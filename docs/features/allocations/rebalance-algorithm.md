# Rebalance Algorithm — Design Notes

Status: Current (M2) Date: 2026-05-31 Audience: Contributors, reviewers, curious
users

---

## 1. What problem are we solving?

A user has a target allocation (e.g. 60 % equities, 30 % bonds, 10 % cash) and a
pool of available cash to deploy. Given the current portfolio, which trades
bring the portfolio as close to target as possible using only that cash?

Constraints:

- **Cash-only by default** — no forced sells (M2). Sell-mode is M3.
- **Sleeve boundaries** — each category gets a proportional share of cash;
  residue does not cross sleeve boundaries.
- **Whole-share mode** — when enabled, only round-lot quantities are suggested.
- **Minimum trade size** — trades below `min_trade_amount` are dropped.

---

## 2. Why not a mathematical optimiser?

Professional rebalancing tools (Tamarac, Orion iRebal) use LP/MILP solvers to
find the globally optimal trade set in one pass. Real costs:

- Heavy dependency (solver library or external service).
- Opaque results — users cannot follow the reasoning.
- Overkill for single-portfolio individual use where "close enough" in
  milliseconds beats "optimal" in seconds.

Our two-phase greedy algorithm achieves near-optimal results for typical inputs
and is easy to audit step by step.

---

## 3. Algorithm overview

```
Input: available_cash, target_profile, current_holdings, drift_report

Phase 1 — Proportional allocation
  Compute shortfall per underweight sleeve.
  Scale sleeve budgets to fit available_cash.
  For each sleeve:
    For each holding (weighted by current market value):
      holding_budget = sleeve_budget × holding_weight
      If whole_shares_only:
        shares = floor(holding_budget / price)
        If shares == 0: track holding as "skipped"
      Else:
        amount = holding_budget    ← fractional, 100% deployed

Phase 2 — Intra-sleeve residue absorption (whole_shares_only only)
  For each sleeve:
    Sort holdings by price ASC
    residue = sleeve_budget - sleeve_deployed
    Loop:
      For each holding (cheapest first):
        If residue ≥ price: buy 1 share, residue -= price
      Break if no holding bought a share this pass

  For each Phase-1-skipped holding still un-funded:
    Emit WholeShareResidue warning with top-up suggestion.

Output: trades[], warnings[], cash_used, cash_remaining
```

---

## 4. Phase 1 — Proportional allocation

### Sleeve budget

Each underweight sleeve receives a budget proportional to its shortfall vs.
target. `nearest_band` aims for the closest band edge; `exact_target` aims for
the exact target percentage. When `total_shortfall > available_cash`, a
`scale_factor = available_cash / total_shortfall` reduces all sleeve budgets
proportionally so every underweight sleeve still gets a share.

Sleeves already at or above target receive zero budget.

### Holding distribution within a sleeve

Within a sleeve, budget is distributed proportional to each holding's current
market value. This keeps the relative composition of the sleeve stable — the
same principle a passive index fund uses when it receives new money.

Example: sleeve holdings = A €4000 (40 %), B €3000 (30 %), C €2000 (20 %), D
€1000 (10 %). Sleeve budget €2000 → A gets €800, B €600, C €400, D €200.

### Fractional vs whole-share mode

| Mode        | Shares                          | Amount                            |
| ----------- | ------------------------------- | --------------------------------- |
| Fractional  | `holding_budget / price`        | `holding_budget` (100 % deployed) |
| Whole-share | `floor(holding_budget / price)` | `shares × price`                  |

Fractional mode deploys 100 % of the sleeve budget. Whole-share mode leaves a
rounding residue that Phase 2 reclaims.

---

## 5. Phase 2 — Intra-sleeve residue absorption (whole-share mode)

### Why this phase exists

With whole-share rounding, each holding may leave a fractional share's worth of
cash undeployed. Across a sleeve with several holdings, this residue easily
totals one or two full share prices — enough to buy at least one more share, but
lost if we stop after Phase 1.

### Algorithm: 1-share-at-a-time, price ascending

1. Sort holdings by price ascending (cheapest first).
2. Loop one pass over all holdings:
   - If `residue ≥ price`, buy exactly **1 share**, reduce residue.
3. Repeat until a full pass buys nothing.

### Why 1-share-at-a-time (not `floor(residue / price)`)

The naive approach `additional = floor(residue / price)` lets the cheapest
holding absorb the entire residue in a single shot, badly skewing sleeve
composition (one cheap ETF receiving 100+ shares while expensive holdings get
zero extra). The 1-share-at-a-time loop preserves balance: each holding can
receive at most one additional share per pass.

### Why price ascending (not proportional weights)

An earlier version used `floor(residue × weight / price)` to preserve
proportions. This failed silently for small residues: when each holding's
proportional slice was below its share price, no holding could buy anything and
the entire residue stayed undeployed. Sorting by price ASC guarantees the
cheapest holding gets the first chance, which is the standard greedy choice for
maximising deployment.

### What it does NOT do

- **No cross-sleeve redistribution.** Residue stays within the sleeve that
  generated it. Crossing sleeve boundaries would silently alter the target
  allocation.
- **No sell suggestions.** Phase 2 only buys.
- **No expensive-holding rescue.** When a cheaper holding shares the sleeve,
  Phase 2 ASC always satisfies the cheaper holding first. Expensive holdings
  that were skipped in Phase 1 (because their proportional budget < price)
  cannot be rescued — see the structural-starvation limitation below.

---

## 6. Known limitation — structural starvation of expensive holdings

### The problem

When a holding's proportional budget is less than its share price, Phase 1 skips
it. Phase 2 cannot rescue it if any cheaper holding exists in the same sleeve —
the cheaper holding will absorb the residue first, leaving less than one
expensive share's worth behind.

**Concrete example:** sleeve has holding X (price €240, 10 % weight). User
deploys €2000/month. X's proportional budget each month is 10 % × €2000 = €200,
which is less than the €240 share price. X never receives any cash, and over
time its portfolio weight drifts toward zero while other holdings grow.

### Why this matters

Without holding-level targets, the algorithm assumes "maintain current
proportions" is the right default. Structurally expensive holdings break that
assumption: their proportion silently decays each rebalance run.

### Options considered

| Option                      | Mechanism                                                           | Status                             |
| --------------------------- | ------------------------------------------------------------------- | ---------------------------------- |
| **A — 1-at-a-time Phase 2** | Absorbs cheap residue, no expensive rescue                          | ✅ Implemented                     |
| **B — Robin Hood rescue**   | Reduce served holdings by 1 share to fund expensive starved holding | ⏳ Open design question for Afadil |
| **C — Top-up warning**      | Emit explicit warning with suggested top-up cash amount             | ✅ Implemented                     |
| **D — HoldingTarget**       | Per-ticker explicit weights override "maintain proportions"         | ⏳ Future (V2 data model)          |

### Current behaviour (A + C)

Phase 2 absorbs cheap residue. For any holding still starved after Phase 2, a
`WholeShareResidue` warning is emitted with the top-up amount needed to fund one
share. Example:

> EXPENSIVE: proportional budget $200.00 short of 1 share at $240.00. Add
> ~$40.00 more cash to fund 1 share, or EXPENSIVE will drift below target over
> time.

The user can then choose to add more cash or accept the drift.

### Why we deferred Robin Hood (Option B)

Reducing one holding's allocation to fund another is a design decision, not a
pure algorithmic improvement. It alters the proportional contract Phase 1
established. We want explicit sign-off from Afadil before changing this default
— see the M2 PR open questions.

### Why we deferred HoldingTarget (Option D)

`HoldingTarget` is in the SOTA spec but not in V1. It requires a new data model,
UI for per-ticker target weights, and downstream changes to drift calculation.
Out of scope for V1/M2.

---

## 7. Warnings emitted

| Warning kind        | Condition                                                                          | User action                                 |
| ------------------- | ---------------------------------------------------------------------------------- | ------------------------------------------- |
| `MissingQuote`      | Holding has no valid price or quantity (whole-share mode)                          | Refresh market data                         |
| `WholeShareResidue` | Holding skipped because proportional budget < price, even after Phase 2 absorption | Top-up suggestion included (price − budget) |
| `NoBuyCandidate`    | Sleeve has no holdings — sleeve-level dollar suggestion emitted                    | User picks a ticker manually                |

The `WholeShareResidue` message names the symbol, original budget, share price,
and exact top-up required. Aggregated at sleeve level: one warning per starved
holding, emitted once per plan.

---

## 8. Possible future improvements

| Idea                                              | Complexity | Status                                                                             |
| ------------------------------------------------- | ---------- | ---------------------------------------------------------------------------------- |
| Robin Hood rescue (Option B above)                | Medium     | Open design question for Afadil                                                    |
| HoldingTarget — per-ticker allocations (Option D) | High       | Requires V2 data model (SOTA Phase 2)                                              |
| Sell-to-rebalance (`allow_sells`)                 | Medium     | Spec'd in V1, scoped for M3                                                        |
| Tax-lot awareness                                 | High       | Out of V1 scope                                                                    |
| Cross-sleeve residue redistribution               | High       | Would break target allocation contract                                             |
| Persistent "owed" credit across runs              | High       | Solves expensive-holding drift more cleanly than Robin Hood, but needs schema + UX |
| LP/MILP solver                                    | High       | Overkill for current use case                                                      |
| Multi-account optimisation                        | High       | SOTA spec, future roadmap                                                          |

---

## 9. Implementation reference

- Algorithm: `crates/core/src/portfolio/allocation_targets/rebalance_service.rs`
  — `calculate_plan()` method, Phase 1 lines ~241–333, Phase 2 lines ~335–460.
- Types: `crates/core/src/portfolio/allocation_targets/model.rs` —
  `RebalancePlan`, `SuggestedManualTrade`, `RebalanceWarning`.
- Tests: same file, `mod tests` — 10 tests covering Phase 1 proportional split,
  Phase 2 absorption + non-skew, scaling, warning emission, min-trade filter.
- UI: `apps/frontend/src/pages/allocation-targets/components/rebalance-tab.tsx`.
