# Performance Metrics Reference

This document describes the performance metrics Wealthfolio calculates, when
each metric applies, and the calculation conventions used by the backend
performance engine.

The performance engine returns a typed `PerformanceResult` for account,
portfolio, dashboard group, saved portfolio, and symbol scopes. Frontend, Tauri,
web, addon, and AI surfaces should use these backend results directly instead of
rolling up returns in the client.

## Result Shape

`PerformanceResult` contains:

- `scope`: stable scope identifier and display currency.
- `period`: actual start and end valuation dates used for the calculation.
- `mode`: the primary return method for the scope.
- `returns`: TWR, IRR, value return, and annualized variants.
- `attribution`: cash flows and P&L components explaining period value change.
- `risk`: volatility and drawdown metrics.
- `dataQuality`: warnings and reasons for missing or partial metrics.
- `series`: cumulative return points for charting when history is requested.
- `isHoldingsMode` / `isMixedTrackingMode`: scope applicability flags.

All return rates are decimals, not percentages. For example, `0.125` means
`12.5%`.

## Scope Behavior

### Transaction Scopes

Transaction-mode accounts and all-transaction grouped scopes use transaction
cash flows.

- Primary comparison metric: `returns.twr`
- Personal cash-flow metric: `returns.irr`
- Annualized fields: populated in full profile results
- Attribution: contributions, distributions, income, realized P&L, unrealized
  P&L, FX effect, fees, taxes, residual
- Risk: volatility and max drawdown in full profile results

Transaction scopes are the only scopes where TWR and IRR are meaningful.

### Holdings-Only Scopes

Holdings-only accounts do not have full transaction cash-flow history.

- Primary return metric: `returns.valueReturn`
- TWR: `null`
- IRR: `null`
- Risk: calculated from value-return samples in full profile results
- Attribution: limited to available value, cost-basis, and FX facts

For all-time holdings-only returns, the denominator is ending cost basis. For a
selected period, the denominator is starting market value.

### Mixed Tracking Scopes

Mixed scopes contain both transaction-mode and holdings-only accounts.

- Primary return metric: `returns.valueReturn`
- TWR: `null`
- IRR: `null`
- Data quality: warning explaining the mixed tracking-mode limitation
- Risk: calculated in full profile results

Mixed scopes use value return because there is no single complete cash-flow
ledger for TWR or IRR.

### Symbol And Price Scopes

Symbol-only performance uses market price history.

- Primary return metric: `returns.valueReturn`
- TWR: `null`
- IRR: `null`
- Attribution: not portfolio cash-flow based
- Risk: calculated from quoted price returns when enough samples exist

Dividends and distributions are excluded unless the quote series itself is
total-return adjusted.

## Return Metrics

### Time-Weighted Return

Field: `returns.twr`

TWR measures investment performance while neutralizing the size and timing of
external cash flows. Use it to compare manager, account, portfolio, or benchmark
performance.

Method:

- Build daily valuation periods from stored daily valuation rows.
- Treat external inflows as start-of-day flows.
- Treat external outflows as end-of-day flows.
- Link valid daily returns geometrically.
- Start compounding only once opening value is positive and the denominator is
  at least 1 base currency unit.
- Exclude tiny or invalid denominator periods from compounding and report a
  not-applicable reason when the chain cannot start.

TWR is unavailable for holdings-only, mixed tracking, and symbol-only scopes.

### Annualized TWR

Field: `returns.annualizedTwr`

Annualized TWR converts selected-period TWR to an annual rate:

```txt
annualized_twr = (1 + twr)^(365.25 / period_days) - 1
```

The engine caps returns at `-100%` when the compounding base would be zero or
negative. For same-day periods, the selected-period return is returned as-is.

### Internal Rate Of Return

Field: `returns.irr`

IRR is the selected-period money-weighted return. It measures the investor's
return after considering the amount and timing of external cash flows. Use it as
the personal performance metric for transaction scopes.

The backend solves annualized XIRR first, then converts it back to the selected
period:

```txt
irr = (1 + annualized_irr)^(period_days / 365.25) - 1
```

For a one-year period, `irr` and `annualizedIrr` will usually be the same. For
sub-year or multi-year periods, `irr` is the actual selected-period result and
`annualizedIrr` is the yearly equivalent.

IRR is unavailable when cash flows have no sign change, when there are not
enough dated cash flows, or when the solver cannot converge.

### Annualized IRR

Field: `returns.annualizedIrr`

Annualized IRR is the XIRR result using dated cash flows and an ACT/365.25 year
basis.

Cash-flow signs:

- Beginning value: negative
- External contributions/inflows: negative
- External distributions/outflows: positive
- Ending value: positive

This field is useful when comparing money-weighted returns across periods of
different lengths.

### Value Return

Field: `returns.valueReturn`

Value return measures period value growth after adjusting for external cash
flows. It is not time weighted and should not be used as a manager-comparison
metric when transaction cash flows are available.

Transaction and mixed scopes:

```txt
value_return = (ending_value - starting_value - net_external_flow) / starting_value
```

Holdings-only scopes:

- All-time: unrealized P&L divided by ending cost basis.
- Selected period: value change divided by starting market value.

Value return is the primary return metric for holdings-only, mixed tracking, and
symbol price scopes.

### Annualized Value Return

Field: `returns.annualizedValueReturn`

Annualized value return uses the same annualization method as annualized TWR:

```txt
annualized_value_return = (1 + value_return)^(365.25 / period_days) - 1
```

It is populated only when the selected scope and profile calculate annualized
returns.

## Attribution Metrics

Attribution explains how total value changed during the period.

Fields:

- `contributions`: external inflows into the scope.
- `distributions`: external outflows from the scope.
- `income`: dividends, interest, and other income.
- `realizedPnl`: realized gain/loss from lot disposals.
- `unrealizedPnlChange`: change in unrealized gain/loss.
- `fxEffect`: base-currency effect from exchange-rate movement.
- `fees`: fees charged during the period.
- `taxes`: taxes charged during the period.
- `residual`: unexplained amount after known components are applied.

Identity:

```txt
ending_value - starting_value
  = contributions - distributions
  + income
  + realized_pnl
  + unrealized_pnl_change
  + fx_effect
  - fees
  - taxes
  + residual
```

The engine emits a warning when residual is larger than:

```txt
max(1 base currency unit, 0.1% of max(abs(delta), ending_value, 1))
```

Realized P&L comes from persisted lot-disposal slices. This includes partial
sells and split-adjusted lots. Dividends and interest are income, not external
cash flows.

## Risk Metrics

### Volatility

Field: `risk.volatility`

Volatility measures dispersion of daily returns. The engine:

- uses valid daily return samples from the selected scope;
- converts daily simple returns to log returns;
- uses sample variance;
- annualizes with `sqrt(365.25)`;
- returns `null` when fewer than two valid log-return samples exist.

Volatility is available only in full profile results.

### Max Drawdown

Fields:

- `risk.maxDrawdown`
- `risk.peakDate`
- `risk.troughDate`
- `risk.recoveryDate`
- `risk.drawdownDurationDays`

Max drawdown is the largest peak-to-trough percentage decline over the selected
return series. The value is returned as a signed negative decimal. Recovery date
is populated when the series recovers to the prior peak.

Drawdown is available only in full profile results.

## Data Quality

`dataQuality.status` can be:

- `ok`: metrics are complete for the selected scope.
- `partial`: scope or valuation history is incomplete.
- `noData`: there is not enough valuation or quote history.
- `notApplicable`: requested metrics do not apply to the scope.

`warnings` describe reliability concerns, such as fallback flow inference, mixed
tracking mode, incomplete history, or large attribution residuals.

`notApplicableReasons` explain why individual metrics are `null`, such as:

- holdings-only scopes do not have TWR or IRR;
- mixed tracking scopes do not have a complete cash-flow ledger;
- IRR cash flows do not change sign;
- starting value is zero or negative;
- denominator is below the minimum threshold.

Consumers should show `null` metrics as unavailable, not as zero.

## API Usage

### History

Use history APIs for detail pages and charts.

- Tauri: `calculate_performance_history`
- Frontend adapter: `calculatePerformanceHistory`
- Addon API: `ctx.api.performance.calculateHistory`

History responses use the full profile and include return series, annualized
returns, IRR, risk, attribution, and data-quality details.

### Summary

Use summary APIs for cards, tables, dashboard rows, and saved portfolio lists.

- Tauri: `calculate_performance_summary`
- Tauri batch: `get_performance_summaries`
- Frontend adapters: `calculatePerformanceSummary`,
  `calculatePerformanceSummaries`
- Addon API: `ctx.api.performance.calculateSummary`

Tauri, web, and frontend summary APIs support two profiles:

- `full`: rich scalar metrics without chart series.
- `headline`: dashboard-focused metrics that omit unused IRR, annualized
  returns, and risk work.

Use `headline` only when the UI needs headline return/P&L and data-quality
messaging. Use `full` when the UI displays IRR, annualized returns, volatility,
drawdown, or detailed attribution.

Addon `calculateSummary` uses the default full summary profile.

### Simple Account Performance

Use `calculateAccountsSimplePerformance` only for lightweight account lists and
allocation views. It is not a replacement for `PerformanceResult` when the UI
needs TWR, IRR, risk, attribution, or data-quality detail.

## Metric Selection Guidelines

- Use TWR for investment comparison and benchmark comparison.
- Use IRR for personal money-weighted return on transaction scopes.
- Use value return for holdings-only, mixed tracking, and symbol price scopes.
- Use annualized fields only when comparing periods of different lengths.
- Use attribution amounts to explain P&L, not as replacement return metrics.
- Use data-quality messages whenever a metric is unavailable or partially
  reliable.

## Cache And Invalidation

Performance results depend on:

- activity import, edit, and delete;
- quote updates;
- exchange-rate updates;
- account tracking-mode changes;
- base-currency changes;
- portfolio membership changes;
- lot disposal rebuilds.

Any mutation that changes those inputs should invalidate performance queries and
dashboard scoped summary queries.
