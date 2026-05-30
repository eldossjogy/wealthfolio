# Rebalance Algorithm — Design Notes

Status: Current (M2)  
Date: 2026-05-30  
Audience: Contributors, curious users

---

## 1. What problem are we solving?

A user has a target allocation (e.g. 60 % equities, 30 % bonds, 10 % cash
equivalents) and a pool of available cash to deploy. Given the current portfolio
state, what trades should be suggested to bring the allocation as close to
target as possible, using only the cash provided?

Constraints the algorithm must respect:

- **Cash-only by default** — no forced sells.
- **Sleeve boundaries** — each category (sleeve) gets a proportional share of
  the available cash; residue does not cross sleeve boundaries.
- **Whole-share mode** — when enabled, only round-lot quantities are suggested.
- **Minimum trade size** — trades below `min_trade_amount` are silently dropped.

---

## 2. Why not use a mathematical optimiser?

Professional rebalancing tools (Tamarac, Orion iRebal) use LP/MILP solvers that
find the globally optimal trade set in one pass. This is mathematically ideal
but comes with real costs:

- Adds a heavy dependency (solver library or external service).
- Results are opaque — users cannot follow the reasoning.
- Overkill for a single-portfolio, individual-use tool where "close enough" in
  milliseconds beats "optimal" in seconds.

Our approach — a two-phase greedy algorithm — achieves near-optimal results for
typical inputs and is easy to audit step by step.

---

## 3. Algorithm overview

```
Input: available_cash, target_profile, current_holdings, drift_report

Phase 1 — Proportional allocation
  For each sleeve:
    budget = available_cash × sleeve_weight
    For each holding in sleeve (weighted by current market value):
      holding_budget = budget × holding_weight
      If whole_shares_only:
        shares = floor(holding_budget / price)
        amount = shares × price          ← may leave rounding residue
      Else:
        amount = holding_budget          ← fractional, 100% deployed

Phase 2 — Intra-sleeve budget optimisation (whole_shares_only only)
  For each sleeve:
    residue = budget − sleeve_deployed
    Sort holdings by price ASC
    Repeat until no more shares affordable:
      For each holding (cheapest first):
        additional = floor(residue / price)
        If additional ≥ 1:
          Merge into existing trade or create new one
          residue -= additional × price

Output: trades[], warnings[], cash_used, cash_remaining
```

---

## 4. Phase 1 — Proportional allocation

### Sleeve budget

Each sleeve receives a budget proportional to its weight gap relative to target.
Specifically, the drift report tells us how much each sleeve is underweight in
absolute terms; those deficits are normalised to sum to `available_cash`.

A sleeve that is already at or above target receives zero budget.

### Holding distribution within a sleeve

Within a sleeve, budget is distributed across holdings proportional to their
current market value. This keeps the relative composition of each sleeve stable
— the same logic a passive index fund uses when it receives new money.

### Fractional vs whole-share mode

| Mode        | Shares                          | Amount                           |
| ----------- | ------------------------------- | -------------------------------- |
| Fractional  | `holding_budget / price`        | `holding_budget` (100% deployed) |
| Whole-share | `floor(holding_budget / price)` | `shares × price`                 |

Fractional mode always deploys 100% of the sleeve budget. Whole-share mode
leaves a rounding residue that Phase 2 reclaims.

---

## 5. Phase 2 — Intra-sleeve budget optimisation

### Why this phase exists

With whole-share rounding, each holding may leave behind a fractional share's
worth of cash. Across a sleeve with ten holdings this can easily total one or
two full share prices — enough to buy at least one more share of the cheapest
holding, but left undeployed if we stop after Phase 1.

### What it does

After Phase 1 completes for a sleeve, the remaining intra-sleeve residue is
redistributed:

1. Sort holdings by price ascending (cheapest first).
2. Loop: for each holding, compute `floor(residue / price)`. If ≥ 1 share is
   affordable, add to the existing trade (or create a new one) and reduce
   residue.
3. Repeat until one full pass over all holdings buys nothing (no improvement
   possible).

Sorting by price ascending maximises the number of holdings touched and
minimises leftover cash.

### What it does NOT do

- **No cross-sleeve redistribution.** Residue stays within the sleeve that
  generated it. Moving cash across sleeve boundaries would silently alter the
  target allocation.
- **No improvement-per-dollar ranking.** That would require holding-level
  targets (not present in V1 which only has category-level targets).
- **No sell suggestions.** Phase 2 only buys.

### Expected outcome

For a typical sleeve with 3–10 holdings, Phase 2 absorbs 80–95% of the rounding
residue. The remaining undeployed cash is genuinely below the price of the
cheapest share in the sleeve.

---

## 6. Warnings emitted

| Warning kind        | Condition                                                 | Action for user                               |
| ------------------- | --------------------------------------------------------- | --------------------------------------------- |
| `MissingQuote`      | Holding has no valid price or quantity (whole-share mode) | Refresh market data                           |
| `WholeShareResidue` | Budget < 1 share price for a holding (after Phase 2)      | Normal; residue goes back as `cash_remaining` |

The residue warning is now per-sleeve (aggregated) rather than per-holding,
since Phase 2 has already absorbed what it can.

---

## 7. Possible future improvements

| Idea                                                   | Complexity | Status                        |
| ------------------------------------------------------ | ---------- | ----------------------------- |
| Sell-to-rebalance (optional)                           | Medium     | Spec'd, not implemented       |
| Tax-lot awareness                                      | High       | Out of V1/M2 scope            |
| Holding-level targets (Phase 1 improvement-per-dollar) | Medium     | Requires V2 data model        |
| LP/MILP solver                                         | High       | Overkill for current use case |
| Multi-account optimisation                             | High       | SOTA spec, future roadmap     |

---

## 8. Implementation reference

- Algorithm: `crates/core/src/portfolio/allocation_targets/rebalance_service.rs`
  — `calculate_plan()` method.
- Types: `crates/core/src/portfolio/allocation_targets/model.rs` —
  `RebalancePlan`, `SuggestedManualTrade`, `RebalanceWarning`.
- UI: `apps/frontend/src/pages/allocation-targets/components/rebalance-tab.tsx`.
