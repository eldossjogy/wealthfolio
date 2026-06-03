# Rebalance Algorithm — Design Notes

Status: Current (PR-B) Date: 2026-06-03 Audience: Contributors, reviewers,
curious users

---

## 1. What problem are we solving?

A user has a target allocation (e.g. 60 % equities, 30 % bonds, 10 % cash) and a
pool of available cash to deploy. Given the current portfolio, which trades
bring the portfolio as close to target as possible using only that cash?

Constraints:

- **Cash-only** — no forced sells (M2). Sell-mode is M3.
- **Exposure-aware** — an asset may span multiple taxonomy categories (e.g. a
  global ETF classified 60 % US equity / 40 % international). One buy must
  update all affected category exposures simultaneously.
- **Whole-share mode** — when enabled, only round-lot quantities are suggested.
- **Minimum trade size** — trades below `min_trade_amount` are dropped from the
  final output.

---

## 2. Why not a mathematical optimiser?

Professional rebalancing tools (Tamarac, Orion iRebal) use LP/MILP solvers to
find the globally optimal trade set in one pass. Real costs:

- Heavy dependency (solver library or external service).
- Opaque results — users cannot follow the reasoning.
- Overkill for single-portfolio individual use where "close enough" in
  milliseconds beats "optimal" in seconds.

The greedy algorithm below achieves near-optimal results for typical inputs and
is easy to audit step by step. The `RebalanceOptimizer` trait is
solver-compatible: a future `MilpOptimizer` can replace `DriftPriorityOptimizer`
behind a feature flag without changing `RebalanceService`.

---

## 3. Algorithm overview

```
Input: available_cash, target_profile, current_holdings, drift_report

Build exposure vectors
  For each non-cash holding with taxonomy assignments:
    exposure_per_share[category] = contribution.value / quantity
    (multi-category ETF → multiple entries summing to ≤ price)
    Skip holding if all categories are __UNKNOWN__ (warn UnclassifiedAsset)
    Include with partial exposure if some categories are __UNKNOWN__ (warn PartialClassification)

Greedy loop
  while cash > 0:
    drift_before = Σ |current_bps[c] − target_bps[c]|  for required non-cash categories
    for each candidate asset:
      if cash < price: skip
      simulate buy 1 share → new_bps[c] = (current_value[c] + exposure_per_share[c]) / total_value
      drift_after = Σ |new_bps[c] − target_bps[c]|
      score = (drift_before − drift_after) / price
    pick candidate with highest score > 0
    tie-break: price ASC (deterministic; lower-price assets preferred)
    if no candidate improves drift: stop
    apply buy: update current_value[c] for each category, cash -= price

Post-processing
  Aggregate shares by asset → SuggestedManualTrade (1 per asset)
  Drop trades where estimated_amount < min_trade_amount
  cash_used = sum of kept trade amounts
  after_bps_by_category = recompute from initial values + kept trades only

Output: trades[], warnings[], cash_used, cash_remaining, after_bps_by_category
```

---

## 4. Exposure vectors

Each asset is represented as a vector of per-share exposures across taxonomy
categories, derived from
`AllocationService::get_holding_contributions_for_taxonomy_for_accounts`.

```
              US equity   Intl equity   Bonds
VT              $60          $40          $0     (price $100 — fully classified)
AAPL           $100           $0          $0     (price $100 — single category)
XYZ             $70           $0          $0     (price $100 — partial: 70% known, 30% __UNKNOWN__)
```

For VT: buying 1 share adds $60 to US equity and $40 to international equity
simultaneously, improving drift in both categories in a single step.

`__UNKNOWN__` exposure is excluded from the vector. Partial exposure means the
greedy can improve drift for the known categories but not for the unclassified
remainder.

### Classification edge cases

| Situation                                     | Behaviour                                                                    |
| --------------------------------------------- | ---------------------------------------------------------------------------- |
| All exposure in `__UNKNOWN__`                 | Skip as candidate. Warn `UnclassifiedAsset`.                                 |
| Partial exposure (<100% classified)           | Include with known exposure. Warn `PartialClassification`. Do not normalise. |
| Weights >100%                                 | `AllocationService` normalises to 100% (consistent with drift view).         |
| Cash holdings (`CASH` / `CASH_BANK_DEPOSITS`) | Excluded from candidates.                                                    |
| No price available (whole-share mode)         | Skip. Warn `MissingQuote`.                                                   |

---

## 5. Scoring

**Score = drift_improvement / price**

Where `drift_improvement = Σ|drift_bps[c]| before − Σ|drift_bps[c]| after`
across all required non-cash categories.

Key properties:

- **Multi-category benefit.** An ETF that simultaneously reduces drift in two
  categories scores higher than a single-category asset at the same price — the
  numerator captures the combined improvement.
- **Scale-invariant for same-category assets.** Two assets 100% in the same
  underweight category have identical scores regardless of price (buying more of
  a cheaper asset per dollar improves the category by the same bps as buying
  less of an expensive one). The tie-break resolves them by price ASC.
- **Stops when no improvement.** Once the portfolio reaches target (or no
  candidate can further reduce drift without overshooting), the loop terminates.
  Remaining cash stays as `cash_remaining`.

---

## 6. Whole-share vs fractional mode

| Mode        | Step                  | Quantity |
| ----------- | --------------------- | -------- |
| Whole-share | 1 share per iteration | Integer  |
| Fractional  | 1 share per iteration | Decimal  |

Both modes share the same greedy whole-share loop. The difference is what
happens with the cash left over once no whole share fits:

- **Whole-share mode** stops there. The leftover (< cheapest candidate price) is
  reported as `cash_remaining`.
- **Fractional mode** deploys a final fractional slice: it picks the candidate
  whose sub-share buy reduces drift the most and sizes it `cash / price`, capped
  so no category overshoots its desired value (the exact target, or the band
  edge under `NearestBand`). Any residue below that cap stays as
  `cash_remaining`.

---

## 7. After-drift computation

After filtering trades by `min_trade_amount`, `after_bps_by_category` is
recomputed from the initial portfolio state plus the kept trades only (not the
full greedy state). This keeps `cash_used`, `cash_remaining`, and
`after_bps_by_category` mutually consistent with what the user will actually
execute.

The frontend `BeforeAfterStack` visualisation uses `after_bps_by_category`
directly from the plan (not re-derived from trade amounts), which gives correct
results for multi-category ETFs.

---

## 8. Warnings emitted

| Warning kind            | Condition                                                    | User action                         |
| ----------------------- | ------------------------------------------------------------ | ----------------------------------- |
| `UnclassifiedAsset`     | Holding has no taxonomy assignments for the active taxonomy  | Classify the asset                  |
| `PartialClassification` | Holding has partial weights (<100%); known exposure used     | Complete classification if possible |
| `MissingQuote`          | No valid price in whole-share mode                           | Refresh market data                 |
| `NoBuyCandidate`        | Required underweight category has no candidate with exposure | Allocate that category manually     |

`NoBuyCandidate` emits a sleeve-level dollar trade (no ticker) so the user sees
the suggested amount even when no holding covers that category.

---

## 9. Open question for Afadil (PR-B)

`AllocationService::contribution_shares_for_holding` normalises weights >10000
bps silently (line 247: `weight_divisor = total.max(10000)`). This is consistent
with how drift reports and allocation views already handle over-allocated
assets. For the rebalance planner we align with this behaviour rather than
blocking on >100% weights.

If a hard block is desired, the right place is write-time validation on
`AssetTaxonomyAssignment` (not in the planner). Flagged for Afadil's input in
the PR.

---

## 10. Possible future improvements

| Idea                                                | Complexity | Status                                |
| --------------------------------------------------- | ---------- | ------------------------------------- |
| Sell-to-rebalance (`allow_sells`)                   | Medium     | M3                                    |
| `HoldingTarget` — per-ticker allocations            | High       | V2 data model (SOTA Phase 2)          |
| Tax-lot awareness                                   | High       | Out of V1 scope                       |
| `TaxAwareOptimizer` (greedy + tax penalty in score) | Medium     | M3/M4                                 |
| `MilpOptimizer` behind `--features milp`            | High       | When tax-aware / lot selection needed |
| Multi-account optimisation                          | High       | SOTA spec, future roadmap             |

---

## 11. Implementation reference

- **Trait + types:** `crates/core/src/portfolio/allocation_targets/optimizer.rs`
  — `RebalanceOptimizer`, `DriftPriorityOptimizer`, `RebalanceInput`,
  `AssetCandidate`.
- **Orchestration:**
  `crates/core/src/portfolio/allocation_targets/rebalance_service.rs` —
  `RebalanceService::calculate_plan()` fetches holdings + contributions, builds
  candidates, calls optimizer.
- **Types:** `crates/core/src/portfolio/allocation_targets/model.rs` —
  `RebalancePlan`, `SuggestedManualTrade`, `RebalanceWarning`,
  `RebalanceWarningKind`.
- **Tests:** `rebalance_service.rs` `mod tests` — 50 tests covering cash
  enforcement, greedy selection, multi-category ETF exposure, classification
  edge cases, whole-share, nearest-band, min-trade filter.
- **UI:**
  `apps/frontend/src/pages/allocation-targets/components/rebalance-tab.tsx`.
