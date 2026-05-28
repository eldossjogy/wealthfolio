# Performance Calculation Model

Wealthfolio uses a single typed performance pipeline for accounts, account
groups, portfolio scopes, and symbol comparisons.

## Result Contract

Performance APIs return `PerformanceResult`:

- `scope`: scope id and result currency.
- `period`: effective start and end dates.
- `mode`: `timeWeighted`, `valueReturn`, `symbolPriceBased`, or `notApplicable`.
- `returns`: `twr`, `annualizedTwr`, `irr`, `annualizedIrr`, `valueReturn`, and
  `annualizedValueReturn`.
- `attribution`: contributions, distributions, income, realized P&L, unrealized
  P&L change, FX effect, fees, taxes, and residual.
- `risk`: annualized volatility, signed max drawdown, peak/trough/recovery
  dates, and drawdown duration.
- `dataQuality`: status, warnings, and not-applicable reasons.
- `series`: dated cumulative return points for charts.

Empty or ineligible scopes return null metrics plus `dataQuality`; they do not
return synthetic zero performance.

## Return Methods

Transaction-mode scopes use daily-linked time-weighted return for comparison.
Daily TWR applies start-of-day external inflows and end-of-day external
outflows. The chain starts only after a positive opening value exists and the
denominator is at least one base currency unit.

Personal cash-flow return is true XIRR on ACT/365 dated cash flows:

- beginning value and contributions are negative cash flows,
- distributions and ending value are positive cash flows,
- insufficient data, no sign change, or solver failure returns null IRR with a
  warning or not-applicable reason.

Holdings-only scopes use value return and total P&L because transaction-level
cash flows are unavailable. Mixed transaction/holdings scopes also use value
return in v1 and mark TWR/IRR as not applicable.

Symbol-only performance uses price/value return and hides IRR unless a user
transaction scope is available.

## Flow Ledger

Valuation rows carry gross `external_inflow_base` and `external_outflow_base`
from activity classification. Same-day deposits and withdrawals remain gross
flows for performance and attribution. Internal transfers are excluded for
portfolio scopes and treated as external only when crossing the selected account
scope boundary.

## Attribution

The attribution identity is:

```txt
delta_total_value_base
  = contributions - distributions
  + income
  + realized_pnl + unrealized_pnl_change
  + fx_effect
  - fees - taxes
  + residual
```

Residuals above the configured tolerance are surfaced as data-quality warnings.
Lot disposal rows preserve deterministic realized P&L slices for FIFO sells,
including partial sells, so historical realized P&L is not lost during
incremental recalculation.

## Risk

Volatility uses log returns from valid market-movement days and annualizes with
`sqrt(252)`. Results include a data-quality note that this is equity-style
annualization.

Max drawdown is returned as a signed negative rate. The risk payload includes
the peak date, trough date, recovery date when recovered, and duration in days.

## Frontend Usage

The frontend formats backend results only. Dashboard hero cards, account rows,
group rows, account detail cards, AI tool output, and addon SDK consumers read
from `PerformanceResult`; they do not recalculate returns from valuation rows.
