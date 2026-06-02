# SOTA Performance Pipeline Plan With Acceptance Criteria

## Summary

Build one typed performance engine for accounts, dashboard groups, saved
portfolios, assets, and the performance page. Drop the legacy Dietz-style
flow-adjusted return completely. Use TWR for comparison, true IRR for personal
cash-flow return, value/P&L for holdings-only scopes, and attribution/risk from
the same ledger.

Public behavior defaults:

- **Transaction scopes:** show TWR, IRR, attribution, volatility, drawdown.
- **Holdings-only scopes:** show value return, total P&L, volatility, drawdown;
  no TWR/IRR.
- **Mixed scopes:** show value return/P&L plus warning; no TWR/IRR in v1.
- **Symbols/assets:** show price/value return and P&L; IRR only when scoped to
  user transactions.
- **Dashboard/account groups/portfolios:** all returns/risk/attribution come
  from backend scoped performance, not frontend rollups.

## API And Model Changes

- Replace the old scalar-first performance result with `PerformanceResult`:
  - `scope`, `period`, `mode`
  - `returns`: `twr`, `annualizedTwr`, `irr`, `annualizedIrr`, `valueReturn`
  - `attribution`: `contributions`, `distributions`, `income`, `realizedPnl`,
    `unrealizedPnlChange`, `fxEffect`, `fees`, `taxes`, `residual`
  - `risk`: `volatility`, `maxDrawdown`, `peakDate`, `troughDate`,
    `recoveryDate`, `drawdownDurationDays`
  - `dataQuality`: `status`, `warnings[]`, `notApplicableReasons[]`
- Remove legacy Dietz-style fields and semantics from Rust, TS, addon SDK, AI
  tool UI, desktop commands, and web API.
- Add batch scoped summary command/API:
  - `get_performance_summaries(scopes[], period)`
  - Scope key is stable and derived from sorted account IDs, not group name.
- Add `lot_disposals` read model for every disposal slice, including partial
  sells.
- Extend lots with dual-currency basis: local cost basis, base cost basis, and
  FX at acquisition.
- Keep settings flat: add `defaultReturnMetric`, no nested setting shape.

## Implementation Checkpoints And Acceptance Criteria

### Checkpoint 1: Contract And Terminology

Implement the new result types and remove legacy Dietz-style terminology from
public UI/API language.

Acceptance criteria:

- No user-facing label, tooltip, enum variant, TS type, addon SDK type, AI
  display, or docs page exposes legacy Dietz-style terminology.
- Old money-weighted aliases are not reused for fake MWR.
- Existing frontend callers compile against `PerformanceResult`.
- Empty/no-data responses return `null` metrics with `dataQuality`, not zero
  returns.
- `pnpm type-check` passes.

### Checkpoint 2: TWR And IRR Engine

Keep GIPS-style daily TWR, add true XIRR, and remove legacy fallback behavior.

Acceptance criteria:

- TWR uses start-of-day external inflows and end-of-day external outflows.
- TWR chain starts only once opening value is positive and denominator is at
  least 1 base currency unit.
- Tiny/zero denominator periods produce `null` TWR with a clear not-applicable
  reason.
- IRR uses ACT/365.25 dated cash flows:
  - beginning value and contributions are negative
  - distributions and ending value are positive
- `returns.annualized_irr` stores annualized XIRR; `returns.irr` stores the
  selected-period money-weighted return derived from that XIRR.
- IRR returns `null` with warning for no sign change, insufficient data, or
  non-convergence.
- Transaction-mode account summary uses TWR as comparison return and IRR as
  personal return.
- Unit tests cover zero-start accounts, early deposits, same-period withdrawals,
  no sign change, and convergence failure.
- `cargo test -p wealthfolio-core performance` passes.

### Checkpoint 3: Typed Daily Flow Ledger

Make account and scoped performance use gross typed external flows.

Acceptance criteria:

- Single-account valuation populates `external_inflow_base` and
  `external_outflow_base` from activity classification, not only net
  contribution deltas.
- Same-day deposit and withdrawal remain two gross flows for IRR/attribution.
- Scoped account, group, and portfolio calculations use the same flow source.
- Internal transfers are excluded from external performance flows.
- Unit tests prove same-day deposit + withdrawal does not collapse to net.
- Existing valuation tests still pass.

### Checkpoint 4: Lot Disposals And Attribution

Persist disposal events and build the attribution identity.

Acceptance criteria:

- Every sell creates deterministic `lot_disposals` rows for each FIFO lot slice.
- Partial sells no longer lose realized P&L history.
- Migration/rebuild path deterministically replays existing activities to
  populate disposals.
- Attribution identity closes:

  ```txt
  delta_total_value_base
    = contributions - distributions
    + income
    + realized_pnl + unrealized_pnl_change
    + fx_effect
    - fees - taxes
    + residual
  ```

- Residual is a loud warning when
  `abs(residual) > max(1 base unit, 0.1% of max(abs(delta), ending value, 1))`.
- `Holding.realized_gain` is wired end-to-end instead of mostly returning
  `None`.
- Unit fixture includes one foreign-currency security, one dividend, one partial
  sale, one FX move, and verifies attribution identity.

### Checkpoint 5: Risk Metrics

Update volatility and drawdown semantics.

Acceptance criteria:

- Volatility uses log returns from valid market-movement days.
- Annualization uses `sqrt(252)`.
- Crypto-only or calendar-daily scopes get a data-quality note that
  annualization is equity-style.
- Max drawdown API returns a signed negative rate.
- Drawdown includes peak date, trough date, recovery date if recovered, and
  duration.
- Unit tests cover unrecovered drawdown, recovered drawdown, flat series, and
  missing values.

### Checkpoint 6: Dashboard, Groups, And Portfolios

Move dashboard performance to backend scoped results.

Acceptance criteria:

- Dashboard hero no longer uses frontend valuation-row return rollups.
- Account rows use account-scope backend results.
- Group rows use backend group-scope results.
- Saved portfolio rows use backend portfolio-scope results.
- Frontend may sum current values, but not returns/risk/attribution.
- Mixed transaction + holdings groups show value return/P&L and warning; TWR/IRR
  are `N/A`.
- Groups with missing valuations show partial-data warning.
- Portfolios with non-performance or deleted/archived accounts show
  excluded-account warning.
- Empty eligible scope shows `N/A`, not zero.
- Multi-currency groups display base-currency performance.
- Dashboard uses batch scoped summaries to avoid N+1 calls.

### Checkpoint 7: Account, Asset, And Performance Pages

Update page-level metric display.

Acceptance criteria:

- Account page transaction-mode cards show TWR, IRR, volatility, max drawdown.
- Account page holdings-mode cards show value return, total P&L, volatility, max
  drawdown.
- Account page mixed/partial-data state shows `N/A` with reason, not silent
  zeroes.
- Asset page separates market value, unrealized P&L, realized P&L, income, FX
  effect, and price return.
- Performance page has metric selection for TWR, IRR, value return, volatility,
  drawdown.
- Performance page benchmark comparison uses TWR only.
- Performance page attribution section explains the period gain/loss by
  components.
- Symbol-only performance hides IRR unless user transaction scope is available.
- Privacy mode hides attribution amounts consistently with hidden balances.

### Checkpoint 8: Addon, AI, Sync, Export, Docs

Update all external and secondary surfaces.

Acceptance criteria:

- Addon SDK exposes `PerformanceResult` and no legacy flow-adjusted return
  fields.
- Addon docs and README examples compile against the new type.
- AI performance tool output and UI show TWR/IRR/value return, not legacy
  flow-adjusted return labels.
- Tauri and web APIs return the same shape.
- Device sync includes `lot_disposals` where app-sync table filters require
  explicit inclusion.
- Backup/export behavior includes the new schema or is verified as
  whole-database safe.
- Query invalidation covers activity import/edit/delete, quote updates, FX
  updates, account tracking mode changes, base currency changes, and portfolio
  membership changes.
- `/Users/aziz/Workspace/wealthfolio-project/wealthfolio-app/wealthfolio/docs/performance-calculation-research.md`
  is updated to describe the new model.

## Validation Plan

Run these at the final checkpoint, and at smaller checkpoints when touching the
relevant layer:

- Rust:
  - `cargo test -p wealthfolio-core performance`
  - `cargo test -p wealthfolio-core valuation`
  - `cargo test -p wealthfolio-core holdings`
  - `cargo test`
- Frontend:
  - `pnpm type-check`
  - `pnpm test`
  - targeted tests for dashboard account groups, account cards, performance page
    selection, AI performance tool UI, and addon type bridge
- E2E:
  - `pnpm test:e2e`
  - specifically validate
    `/Users/aziz/Workspace/wealthfolio-project/wealthfolio-app/wealthfolio/e2e/01-happy-path.spec.ts`
  - specifically validate
    `/Users/aziz/Workspace/wealthfolio-project/wealthfolio-app/wealthfolio/e2e/08-holdings-and-performance.spec.ts`
- Full repo:
  - `pnpm check`
  - desktop and web compile through existing shared command paths

## Assumptions And Defaults

- No backward compatibility for legacy flow-adjusted return fields.
- TWR is the default comparison metric.
- IRR is shown as personal cash-flow return, not used for benchmark comparison.
- Mixed transaction + holdings scopes do not get TWR/IRR in v1.
- All displayed money performance for grouped scopes is in base currency.
- Backend owns performance math; frontend only formats and chooses applicable
  cards.
- Residual warnings are visible enough to tell users the performance result is
  partially unreliable.
