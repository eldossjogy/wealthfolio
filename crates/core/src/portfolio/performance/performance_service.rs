use crate::accounts::TrackingMode;
use crate::activities::{Activity, ActivityRepositoryTrait, ActivityType};
use crate::constants::DECIMAL_PRECISION;
use crate::errors::{self, Result, ValidationError};
use crate::fx::FxServiceTrait;
use crate::lots::{LotDisposal, LotRecord, LotRepositoryTrait};
use crate::performance::ReturnData;
use crate::quotes::QuoteServiceTrait;
use crate::utils::time_utils::{activity_date_in_tz, parse_user_timezone_or_default, user_today};
use crate::valuation::ValuationServiceTrait;

use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
use num_traits::ToPrimitive;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, RwLock};

use log::{debug, warn};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use rust_decimal::MathematicalOps;
use rust_decimal_macros::dec;

use super::{
    DataQualityStatus, PerformanceAttribution, PerformanceDataQuality, PerformancePeriod,
    PerformanceResult, PerformanceReturns, PerformanceRisk, PerformanceScopeDescriptor,
    ReturnMethod, SimplePerformanceMetrics,
};
use crate::portfolio::valuation::{
    DailyAccountValuation, ExternalFlowSource as ValuationExternalFlowSource,
};

#[async_trait]
pub trait PerformanceServiceTrait: Send + Sync {
    async fn calculate_performance_history(
        &self,
        item_type: &str,
        item_id: &str,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
        tracking_mode: Option<TrackingMode>,
    ) -> Result<PerformanceResult>;

    async fn calculate_performance_history_for_accounts(
        &self,
        scope_id: &str,
        account_ids: &[String],
        base_currency: &str,
        account_tracking_modes: &HashMap<String, TrackingMode>,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<PerformanceResult>;

    async fn calculate_performance_summary(
        &self,
        item_type: &str,
        item_id: &str,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
        tracking_mode: Option<TrackingMode>,
    ) -> Result<PerformanceResult>;

    async fn calculate_performance_summary_for_accounts(
        &self,
        scope_id: &str,
        account_ids: &[String],
        base_currency: &str,
        account_tracking_modes: &HashMap<String, TrackingMode>,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<PerformanceResult>;

    /// Calculates lightweight account performance metrics (cumulative returns and portfolio weights) for multiple accounts.
    /// This method efficiently fetches the latest and previous day's valuations in bulk to minimize database queries.
    /// Can be used for a single account by passing a slice with one ID.
    fn calculate_accounts_simple_performance(
        &self,
        account_ids: &[String],
    ) -> Result<Vec<SimplePerformanceMetrics>>;
}

pub struct PerformanceService {
    valuation_service: Arc<dyn ValuationServiceTrait + Send + Sync>,
    quote_service: Arc<dyn QuoteServiceTrait + Send + Sync>,
    timezone: Arc<RwLock<String>>,
    lot_repository: Option<Arc<dyn LotRepositoryTrait>>,
    activity_repository: Option<Arc<dyn ActivityRepositoryTrait>>,
    fx_service: Option<Arc<dyn FxServiceTrait>>,
}

const TRADING_DAYS_PER_YEAR: u32 = 252;
const DAYS_PER_YEAR_DECIMAL: Decimal = dec!(365.25);
const SQRT_TRADING_DAYS_APPROX: Decimal = dec!(15.874507866); // sqrt(252)

fn parse_decimal_lossy(value: &str) -> Decimal {
    value.parse::<Decimal>().unwrap_or(Decimal::ZERO)
}

#[derive(Clone, Copy, Debug)]
struct DailyReturnSample {
    twr: Decimal,
    cumulative_twr_to_date: Decimal,
    excluded_from_compounding: bool,
}

#[derive(Clone, Copy, Debug)]
struct RiskSample {
    date: NaiveDate,
    simple_return: Decimal,
}

#[derive(Clone, Debug)]
struct TwrComputation {
    cumulative_twr: Option<Decimal>,
    samples: Vec<(NaiveDate, DailyReturnSample)>,
    warnings: Vec<String>,
    not_applicable_reasons: Vec<String>,
}

#[derive(Clone, Debug)]
struct IrrComputation {
    irr: Option<Decimal>,
    warnings: Vec<String>,
    not_applicable_reasons: Vec<String>,
}

#[derive(Clone, Debug)]
struct DrawdownComputation {
    max_drawdown: Option<Decimal>,
    peak_date: Option<NaiveDate>,
    trough_date: Option<NaiveDate>,
    recovery_date: Option<NaiveDate>,
    duration_days: Option<i64>,
}

#[derive(Clone, Debug)]
struct ScopedUnrealizedAttribution {
    unrealized_pnl_change: Decimal,
    fx_effect: Decimal,
    warnings: Vec<String>,
    complete: bool,
}

struct ScopedPerformanceRequest<'a> {
    scope_id: &'a str,
    account_ids: &'a [String],
    base_currency: &'a str,
    account_tracking_modes: &'a HashMap<String, TrackingMode>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    include_returns_series: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedTrackingComposition {
    TransactionsOnly,
    HoldingsOnly,
    Mixed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
enum ExternalFlowBasis {
    AccountCurrency,
    BaseCurrency,
}

#[derive(Clone, Copy, Debug)]
struct DailyExternalFlow {
    date: NaiveDate,
    inflow: Decimal,
    outflow: Decimal,
    source: ValuationExternalFlowSource,
}

impl DailyExternalFlow {
    fn net(self) -> Decimal {
        self.inflow - self.outflow
    }
}

impl PerformanceService {
    pub fn new(
        valuation_service: Arc<dyn ValuationServiceTrait + Send + Sync>,
        quote_service: Arc<dyn QuoteServiceTrait + Send + Sync>,
    ) -> Self {
        Self::new_with_timezone(
            valuation_service,
            quote_service,
            Arc::new(RwLock::new(String::new())),
        )
    }

    pub fn new_with_timezone(
        valuation_service: Arc<dyn ValuationServiceTrait + Send + Sync>,
        quote_service: Arc<dyn QuoteServiceTrait + Send + Sync>,
        timezone: Arc<RwLock<String>>,
    ) -> Self {
        Self {
            valuation_service,
            quote_service,
            timezone,
            lot_repository: None,
            activity_repository: None,
            fx_service: None,
        }
    }

    pub fn with_lot_repository(mut self, lot_repository: Arc<dyn LotRepositoryTrait>) -> Self {
        self.lot_repository = Some(lot_repository);
        self
    }

    pub fn with_activity_repository(
        mut self,
        activity_repository: Arc<dyn ActivityRepositoryTrait>,
        fx_service: Arc<dyn FxServiceTrait>,
    ) -> Self {
        self.activity_repository = Some(activity_repository);
        self.fx_service = Some(fx_service);
        self
    }

    fn today_in_user_timezone(&self) -> NaiveDate {
        let tz = parse_user_timezone_or_default(&self.timezone.read().unwrap());
        user_today(tz)
    }

    fn activity_local_date(&self, activity: &Activity) -> NaiveDate {
        let tz = parse_user_timezone_or_default(&self.timezone.read().unwrap());
        activity_date_in_tz(activity.activity_date, tz)
    }

    // =========================================================================
    // Shared performance math
    //
    // These helpers are the single source of truth for the formulas used by
    // both the "full" and "summary" account-performance paths. Having two
    // slightly-diverging copies of this math was the root cause of the
    // dashboard-vs-account-page percentage mismatch — keep them consolidated.
    // =========================================================================

    fn compute_time_weighted_returns(
        history: &[DailyAccountValuation],
        daily_flows: &[DailyExternalFlow],
        flow_basis: ExternalFlowBasis,
    ) -> Result<TwrComputation> {
        let mut cumulative_twr_factor = Decimal::ONE;
        let mut samples = Vec::new();
        let warnings = Vec::new();
        let mut not_applicable_reasons = Vec::new();
        let mut chain_started = false;

        for (window, flow) in history.windows(2).zip(daily_flows.iter()) {
            let prev_point = &window[0];
            let curr_point = &window[1];

            let prev_value = Self::return_total_value(prev_point, flow_basis);
            let curr_value = Self::return_total_value(curr_point, flow_basis);

            if prev_value.is_sign_negative() || curr_value.is_sign_negative() {
                return Err(errors::Error::Validation(ValidationError::InvalidInput(
                    "Account has negative portfolio value in its history. This may be caused by missing buy activities. Please review your transactions on the Activities page.".to_string(),
                )));
            }

            let twr_denominator = prev_value + flow.inflow;
            if !chain_started && (prev_value <= Decimal::ZERO || twr_denominator < Decimal::ONE) {
                if prev_value > Decimal::ZERO && twr_denominator < Decimal::ONE {
                    not_applicable_reasons.push(format!(
                        "TWR unavailable for {}: denominator {} is below 1 base currency unit before the return chain starts.",
                        curr_point.valuation_date, twr_denominator
                    ));
                }
                let sample = DailyReturnSample {
                    twr: Decimal::ZERO,
                    cumulative_twr_to_date: Decimal::ZERO,
                    excluded_from_compounding: true,
                };
                samples.push((curr_point.valuation_date, sample));
                continue;
            }

            chain_started = true;

            let excluded_from_compounding = twr_denominator < Decimal::ONE;
            let twr = if excluded_from_compounding {
                let reason = format!(
                    "TWR unavailable for {}: denominator {} is below 1 base currency unit.",
                    curr_point.valuation_date, twr_denominator
                );
                warn!("{}", reason);
                not_applicable_reasons.push(reason);
                Decimal::ZERO
            } else {
                let numerator = curr_value + flow.outflow - prev_value - flow.inflow;
                numerator / twr_denominator
            };

            if !excluded_from_compounding {
                cumulative_twr_factor *= Decimal::ONE + twr;
            }

            let sample = DailyReturnSample {
                twr,
                cumulative_twr_to_date: cumulative_twr_factor - Decimal::ONE,
                excluded_from_compounding,
            };
            samples.push((curr_point.valuation_date, sample));
        }

        let cumulative_twr = if !chain_started {
            not_applicable_reasons.push(
                "TWR unavailable: no period starts with positive opening value and denominator of at least 1 base currency unit.".to_string(),
            );
            None
        } else if !not_applicable_reasons.is_empty() {
            None
        } else {
            Some(cumulative_twr_factor - Decimal::ONE)
        };

        Ok(TwrComputation {
            cumulative_twr,
            samples,
            warnings,
            not_applicable_reasons,
        })
    }

    fn daily_external_flows(
        prev_point: &DailyAccountValuation,
        curr_point: &DailyAccountValuation,
        flow_basis: ExternalFlowBasis,
    ) -> DailyExternalFlow {
        let date = curr_point.valuation_date;
        let cash_flow = match flow_basis {
            ExternalFlowBasis::AccountCurrency => {
                curr_point.net_contribution - prev_point.net_contribution
            }
            ExternalFlowBasis::BaseCurrency => {
                if curr_point.external_flow_source.is_explicit_gross()
                    || curr_point.external_flow_source
                        == ValuationExternalFlowSource::NetContributionFallback
                {
                    return DailyExternalFlow {
                        date,
                        inflow: curr_point.external_inflow_base,
                        outflow: curr_point.external_outflow_base,
                        source: curr_point.external_flow_source,
                    };
                }

                if !curr_point.external_inflow_base.is_zero()
                    || !curr_point.external_outflow_base.is_zero()
                {
                    return DailyExternalFlow {
                        date,
                        inflow: curr_point.external_inflow_base,
                        outflow: curr_point.external_outflow_base,
                        source: ValuationExternalFlowSource::StoredGross,
                    };
                }

                curr_point.net_contribution_base - prev_point.net_contribution_base
            }
        };
        let (inflow, outflow) = if cash_flow.is_sign_negative() {
            (Decimal::ZERO, -cash_flow)
        } else {
            (cash_flow, Decimal::ZERO)
        };
        DailyExternalFlow {
            date,
            inflow,
            outflow,
            source: ValuationExternalFlowSource::NetContributionFallback,
        }
    }

    fn daily_external_flow_series(
        history: &[DailyAccountValuation],
        flow_basis: ExternalFlowBasis,
    ) -> Vec<DailyExternalFlow> {
        history
            .windows(2)
            .map(|window| Self::daily_external_flows(&window[0], &window[1], flow_basis))
            .collect()
    }

    fn external_flow_quality_warnings(daily_flows: &[DailyExternalFlow]) -> Vec<String> {
        let used_net_fallback = daily_flows
            .iter()
            .any(|flow| flow.source == ValuationExternalFlowSource::NetContributionFallback);
        let used_degraded_gross = daily_flows.iter().any(|flow| {
            matches!(
                flow.source,
                ValuationExternalFlowSource::Unknown | ValuationExternalFlowSource::Mixed
            )
        });

        let mut warnings = Vec::new();
        if used_net_fallback {
            warnings.push(
                "External cash flows were inferred from net contribution deltas for part of this period because gross daily flow data was unavailable; same-day deposits and withdrawals may be netted.".to_string(),
            );
        }
        if used_degraded_gross {
            warnings.push(
                "External cash flow provenance is incomplete for part of this period; return and attribution results may include degraded flow data.".to_string(),
            );
        }
        warnings
    }

    fn return_total_value(point: &DailyAccountValuation, flow_basis: ExternalFlowBasis) -> Decimal {
        match flow_basis {
            ExternalFlowBasis::AccountCurrency => point.total_value,
            ExternalFlowBasis::BaseCurrency => point.total_value_base,
        }
    }

    fn return_investment_market_value(
        point: &DailyAccountValuation,
        flow_basis: ExternalFlowBasis,
    ) -> Decimal {
        match flow_basis {
            ExternalFlowBasis::AccountCurrency => point.investment_market_value,
            ExternalFlowBasis::BaseCurrency => point.investment_market_value_base,
        }
    }

    fn return_cost_basis(point: &DailyAccountValuation, flow_basis: ExternalFlowBasis) -> Decimal {
        match flow_basis {
            ExternalFlowBasis::AccountCurrency => point.cost_basis,
            ExternalFlowBasis::BaseCurrency => point.cost_basis_base,
        }
    }

    fn compute_simple_value_return(
        full_history: &[DailyAccountValuation],
        daily_flows: &[DailyExternalFlow],
        flow_basis: ExternalFlowBasis,
    ) -> Option<Decimal> {
        let start_point = full_history.first()?;
        let end_point = full_history.last()?;
        let start_value = Self::return_total_value(start_point, flow_basis);
        if start_value <= Decimal::ZERO {
            return None;
        }

        let net_cash_flow: Decimal = daily_flows.iter().map(|flow| flow.net()).sum();
        let end_value = Self::return_total_value(end_point, flow_basis);

        Some((end_value - start_value - net_cash_flow) / start_value)
    }

    fn total_external_flows(daily_flows: &[DailyExternalFlow]) -> (Decimal, Decimal) {
        daily_flows.iter().fold(
            (Decimal::ZERO, Decimal::ZERO),
            |(inflows, outflows), flow| (inflows + flow.inflow, outflows + flow.outflow),
        )
    }

    fn calculate_xirr(
        history: &[DailyAccountValuation],
        daily_flows: &[DailyExternalFlow],
        flow_basis: ExternalFlowBasis,
    ) -> IrrComputation {
        if history.len() < 2 {
            return IrrComputation {
                irr: None,
                warnings: Vec::new(),
                not_applicable_reasons: vec![
                    "IRR unavailable: at least two valuation points are required.".to_string(),
                ],
            };
        }

        let start = history.first().expect("len checked");
        let end = history.last().expect("len checked");
        let mut cash_flows: Vec<(NaiveDate, f64)> = Vec::new();

        if let Some(start_value) = Self::return_total_value(start, flow_basis).to_f64() {
            if start_value > 0.0 {
                cash_flows.push((start.valuation_date, -start_value));
            }
        }

        for flow in daily_flows {
            if let Some(inflow) = flow.inflow.to_f64() {
                if inflow > 0.0 {
                    cash_flows.push((flow.date, -inflow));
                }
            }
            if let Some(outflow) = flow.outflow.to_f64() {
                if outflow > 0.0 {
                    cash_flows.push((flow.date, outflow));
                }
            }
        }

        if let Some(end_value) = Self::return_total_value(end, flow_basis).to_f64() {
            if end_value > 0.0 {
                cash_flows.push((end.valuation_date, end_value));
            }
        }

        if cash_flows.len() < 2 {
            return IrrComputation {
                irr: None,
                warnings: Vec::new(),
                not_applicable_reasons: vec![
                    "IRR unavailable: insufficient dated cash flows.".to_string()
                ],
            };
        }

        let has_positive = cash_flows.iter().any(|(_, amount)| *amount > 0.0);
        let has_negative = cash_flows.iter().any(|(_, amount)| *amount < 0.0);
        if !has_positive || !has_negative {
            return IrrComputation {
                irr: None,
                warnings: vec!["IRR unavailable: cash flows do not change sign.".to_string()],
                not_applicable_reasons: Vec::new(),
            };
        }

        let origin = cash_flows[0].0;
        let npv = |rate: f64| -> Option<f64> {
            if rate <= -0.999_999_999 {
                return None;
            }
            let base = 1.0 + rate;
            let mut total = 0.0;
            for (date, amount) in &cash_flows {
                let years = (*date - origin).num_days() as f64 / 365.0;
                total += amount / base.powf(years);
            }
            if total.is_finite() {
                Some(total)
            } else {
                None
            }
        };

        let mut low = -0.999_999;
        let mut high = 10.0;
        let mut npv_low = match npv(low) {
            Some(value) => value,
            None => {
                return IrrComputation {
                    irr: None,
                    warnings: vec![
                        "IRR unavailable: solver could not evaluate cash flows.".to_string()
                    ],
                    not_applicable_reasons: Vec::new(),
                }
            }
        };
        let mut npv_high = npv(high).unwrap_or(f64::NAN);

        let mut expanded = 0;
        while npv_low.signum() == npv_high.signum() && expanded < 16 {
            high *= 2.0;
            npv_high = npv(high).unwrap_or(f64::NAN);
            if !npv_high.is_finite() {
                break;
            }
            expanded += 1;
        }

        if !npv_high.is_finite() || npv_low.signum() == npv_high.signum() {
            return IrrComputation {
                irr: None,
                warnings: vec!["IRR unavailable: solver did not converge.".to_string()],
                not_applicable_reasons: Vec::new(),
            };
        }

        for _ in 0..128 {
            let mid = (low + high) / 2.0;
            let Some(npv_mid) = npv(mid) else {
                return IrrComputation {
                    irr: None,
                    warnings: vec!["IRR unavailable: solver did not converge.".to_string()],
                    not_applicable_reasons: Vec::new(),
                };
            };
            if npv_mid.abs() < 1e-7 || (high - low).abs() < 1e-10 {
                return IrrComputation {
                    irr: Decimal::from_f64(mid).map(|value| value.round_dp(DECIMAL_PRECISION)),
                    warnings: Vec::new(),
                    not_applicable_reasons: Vec::new(),
                };
            }

            if npv_low.signum() == npv_mid.signum() {
                low = mid;
                npv_low = npv_mid;
            } else {
                high = mid;
            }
        }

        IrrComputation {
            irr: None,
            warnings: vec!["IRR unavailable: solver did not converge.".to_string()],
            not_applicable_reasons: Vec::new(),
        }
    }

    fn annualize_optional_return(
        start_date: NaiveDate,
        end_date: NaiveDate,
        value: Option<Decimal>,
    ) -> Option<Decimal> {
        value.map(|return_value| {
            Self::calculate_annualized_return(start_date, end_date, return_value)
                .round_dp(DECIMAL_PRECISION)
        })
    }

    fn data_quality(
        warnings: Vec<String>,
        not_applicable_reasons: Vec<String>,
        no_data: bool,
    ) -> PerformanceDataQuality {
        let status = if no_data {
            DataQualityStatus::NoData
        } else if !warnings.is_empty() || !not_applicable_reasons.is_empty() {
            DataQualityStatus::Partial
        } else {
            DataQualityStatus::Ok
        };
        PerformanceDataQuality {
            status,
            warnings,
            not_applicable_reasons,
        }
    }

    fn refresh_data_quality_status(data_quality: &mut PerformanceDataQuality) {
        if matches!(
            data_quality.status,
            DataQualityStatus::NoData | DataQualityStatus::NotApplicable
        ) {
            return;
        }

        data_quality.status =
            if data_quality.warnings.is_empty() && data_quality.not_applicable_reasons.is_empty() {
                DataQualityStatus::Ok
            } else {
                DataQualityStatus::Partial
            };
    }

    fn risk_from_samples(
        samples: &[RiskSample],
        opening_date: Option<NaiveDate>,
    ) -> PerformanceRisk {
        let returns: Vec<Decimal> = samples.iter().map(|sample| sample.simple_return).collect();
        let drawdown = Self::calculate_max_drawdown(samples, opening_date);
        PerformanceRisk {
            volatility: Self::calculate_volatility(&returns),
            max_drawdown: drawdown.max_drawdown,
            peak_date: drawdown.peak_date,
            trough_date: drawdown.trough_date,
            recovery_date: drawdown.recovery_date,
            drawdown_duration_days: drawdown.duration_days,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_result(
        id: String,
        currency: String,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
        mode: ReturnMethod,
        returns: PerformanceReturns,
        attribution: PerformanceAttribution,
        risk: PerformanceRisk,
        data_quality: PerformanceDataQuality,
        series: Vec<ReturnData>,
        is_holdings_mode: bool,
        is_mixed_tracking_mode: bool,
    ) -> PerformanceResult {
        PerformanceResult {
            scope: PerformanceScopeDescriptor { id, currency },
            period: PerformancePeriod {
                start_date,
                end_date,
            },
            mode,
            returns,
            attribution,
            risk,
            data_quality,
            series,
            is_holdings_mode,
            is_mixed_tracking_mode,
        }
    }

    async fn apply_external_attribution_best_effort(
        &self,
        result: &mut PerformanceResult,
        account_ids: &[String],
        history: &[DailyAccountValuation],
    ) {
        self.apply_activity_attribution_best_effort(result, account_ids, history)
            .await;
        self.apply_realized_attribution_best_effort(result, account_ids, history)
            .await;
        self.apply_trade_fee_pnl_gross_up_best_effort(result, account_ids, history)
            .await;
    }

    async fn apply_scoped_unrealized_attribution_best_effort(
        &self,
        result: &mut PerformanceResult,
        account_ids: &[String],
        aggregate_history: &[DailyAccountValuation],
    ) {
        let Some(start_date) = result.period.start_date else {
            return;
        };
        let Some(end_date) = result.period.end_date else {
            return;
        };
        if account_ids.is_empty() {
            return;
        }

        let histories_by_account = match self
            .valuation_service
            .get_historical_valuations_by_account(account_ids, Some(start_date), Some(end_date))
        {
            Ok(histories) => histories,
            Err(e) => {
                result.data_quality.warnings.push(format!(
                    "Scoped FX attribution skipped because valuation history failed: {}",
                    e
                ));
                Self::refresh_data_quality_status(&mut result.data_quality);
                return;
            }
        };
        let account_histories: Vec<Vec<DailyAccountValuation>> = account_ids
            .iter()
            .map(|account_id| {
                histories_by_account
                    .get(account_id)
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();

        let attribution = Self::scoped_unrealized_attribution_components(
            &account_histories,
            start_date,
            end_date,
        );
        result.data_quality.warnings.extend(attribution.warnings);
        if !attribution.complete {
            Self::refresh_data_quality_status(&mut result.data_quality);
            return;
        }

        result.attribution.unrealized_pnl_change = attribution.unrealized_pnl_change;
        result.attribution.fx_effect = attribution.fx_effect;
        Self::recompute_attribution_residual(
            result,
            aggregate_history,
            ExternalFlowBasis::BaseCurrency,
        );
    }

    async fn apply_activity_attribution_best_effort(
        &self,
        result: &mut PerformanceResult,
        account_ids: &[String],
        history: &[DailyAccountValuation],
    ) {
        let Some(activity_repository) = &self.activity_repository else {
            return;
        };
        let Some(start_date) = result.period.start_date else {
            return;
        };
        let Some(end_date) = result.period.end_date else {
            return;
        };
        if account_ids.is_empty() {
            return;
        }

        let start_utc = (start_date - Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let end_utc = (end_date + Duration::days(1))
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_utc();

        let activities = match activity_repository.get_activities_by_account_ids_in_date_range(
            account_ids,
            start_utc,
            end_utc,
        ) {
            Ok(activities) => activities,
            Err(e) => {
                warn!(
                    "Failed to load activities for performance attribution scope {}: {}",
                    result.scope.id, e
                );
                return;
            }
        };

        let mut income = Decimal::ZERO;
        let mut fees = Decimal::ZERO;
        let mut taxes = Decimal::ZERO;
        let mut warnings = Vec::new();

        for activity in activities {
            if !activity.is_posted() {
                continue;
            }

            let activity_date = self.activity_local_date(&activity);
            if activity_date <= start_date || activity_date > end_date {
                continue;
            }

            let Ok(activity_type) = ActivityType::from_str(activity.effective_type()) else {
                continue;
            };
            let (raw_income, raw_fees, raw_taxes) =
                Self::activity_attribution_components(&activity, &activity_type);

            if !raw_income.is_zero() {
                match self.convert_activity_amount_for_attribution(
                    &activity,
                    raw_income,
                    &result.scope.currency,
                    activity_date,
                ) {
                    Some(amount) => income += amount,
                    None => warnings.push(format!(
                        "Income attribution skipped for activity {} because FX conversion failed.",
                        activity.id
                    )),
                }
            }

            if !raw_fees.is_zero() {
                match self.convert_activity_amount_for_attribution(
                    &activity,
                    raw_fees,
                    &result.scope.currency,
                    activity_date,
                ) {
                    Some(amount) => fees += amount,
                    None => warnings.push(format!(
                        "Fee attribution skipped for activity {} because FX conversion failed.",
                        activity.id
                    )),
                }
            }

            if !raw_taxes.is_zero() {
                match self.convert_activity_amount_for_attribution(
                    &activity,
                    raw_taxes,
                    &result.scope.currency,
                    activity_date,
                ) {
                    Some(amount) => taxes += amount,
                    None => warnings.push(format!(
                        "Tax attribution skipped for activity {} because FX conversion failed.",
                        activity.id
                    )),
                }
            }
        }

        let changed = !income.is_zero() || !fees.is_zero() || !taxes.is_zero();
        if changed {
            result.attribution.income = income.round_dp(DECIMAL_PRECISION);
            result.attribution.fees = fees.round_dp(DECIMAL_PRECISION);
            result.attribution.taxes = taxes.round_dp(DECIMAL_PRECISION);
        }
        if !warnings.is_empty() {
            result.data_quality.warnings.extend(warnings);
        }
        if changed || !result.data_quality.warnings.is_empty() {
            Self::recompute_attribution_residual(result, history, ExternalFlowBasis::BaseCurrency);
        }
    }

    fn activity_attribution_components(
        activity: &Activity,
        activity_type: &ActivityType,
    ) -> (Decimal, Decimal, Decimal) {
        match activity_type {
            ActivityType::Dividend | ActivityType::Interest => {
                (activity.amt(), activity.fee_amt(), Decimal::ZERO)
            }
            ActivityType::Fee => (
                Decimal::ZERO,
                Self::activity_charge_amount(activity),
                Decimal::ZERO,
            ),
            ActivityType::Buy | ActivityType::Sell => {
                (Decimal::ZERO, activity.fee_amt(), Decimal::ZERO)
            }
            ActivityType::Tax => (
                Decimal::ZERO,
                Decimal::ZERO,
                Self::activity_charge_amount(activity),
            ),
            _ => (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO),
        }
    }

    fn activity_charge_amount(activity: &Activity) -> Decimal {
        if activity.fee_amt().is_zero() {
            activity.amt()
        } else {
            activity.fee_amt()
        }
    }

    fn convert_activity_amount_for_attribution(
        &self,
        activity: &Activity,
        amount: Decimal,
        target_currency: &str,
        activity_date: NaiveDate,
    ) -> Option<Decimal> {
        if activity.currency == target_currency {
            return Some(amount);
        }

        let Some(fx_service) = &self.fx_service else {
            warn!(
                "Missing FX service for performance attribution conversion {} -> {} on activity {}",
                activity.currency, target_currency, activity.id
            );
            return None;
        };

        match fx_service.convert_currency_for_date(
            amount,
            &activity.currency,
            target_currency,
            activity_date,
        ) {
            Ok(converted) => Some(converted),
            Err(e) => {
                warn!(
                    "Failed performance attribution FX conversion for activity {}: {} {} -> {} on {}: {}",
                    activity.id, amount, activity.currency, target_currency, activity_date, e
                );
                None
            }
        }
    }

    fn realized_pnl_base_from_disposal(
        disposal: &LotDisposal,
    ) -> std::result::Result<Decimal, String> {
        let fx_rate_to_base = parse_decimal_lossy(&disposal.fx_rate_to_base);
        if disposal.currency != disposal.base_currency && fx_rate_to_base <= Decimal::ZERO {
            return Err(format!(
                "Realized P&L attribution skipped for disposal {} because FX conversion was unavailable.",
                disposal.id
            ));
        }

        let cost_basis = parse_decimal_lossy(&disposal.cost_basis);
        let cost_basis_base = parse_decimal_lossy(&disposal.cost_basis_base);
        if disposal.currency != disposal.base_currency
            && cost_basis > Decimal::ZERO
            && cost_basis_base <= Decimal::ZERO
        {
            return Err(format!(
                "Realized P&L attribution skipped for disposal {} because acquisition FX conversion was unavailable.",
                disposal.id
            ));
        }

        Ok(parse_decimal_lossy(&disposal.realized_pnl_base))
    }

    async fn apply_realized_attribution_best_effort(
        &self,
        result: &mut PerformanceResult,
        account_ids: &[String],
        history: &[DailyAccountValuation],
    ) {
        let Some(lot_repository) = &self.lot_repository else {
            return;
        };
        let Some(start_date) = result.period.start_date else {
            return;
        };
        let Some(end_date) = result.period.end_date else {
            return;
        };

        let mut realized_pnl_base = Decimal::ZERO;
        let initial_warning_count = result.data_quality.warnings.len();
        for account_id in account_ids {
            let disposals = match lot_repository
                .get_lot_disposals_for_account(account_id)
                .await
            {
                Ok(disposals) => disposals,
                Err(e) => {
                    warn!(
                        "Failed to load lot disposals for performance attribution account {}: {}",
                        account_id, e
                    );
                    continue;
                }
            };

            for disposal in disposals {
                let Ok(disposal_date) =
                    NaiveDate::parse_from_str(&disposal.disposal_date, "%Y-%m-%d")
                else {
                    continue;
                };
                if disposal_date <= start_date || disposal_date > end_date {
                    continue;
                }
                match Self::realized_pnl_base_from_disposal(&disposal) {
                    Ok(amount) => realized_pnl_base += amount,
                    Err(warning) => result.data_quality.warnings.push(warning),
                }
            }
        }

        if realized_pnl_base.is_zero() {
            if result.data_quality.warnings.len() != initial_warning_count {
                Self::refresh_data_quality_status(&mut result.data_quality);
            }
            return;
        }

        result.attribution.realized_pnl = realized_pnl_base.round_dp(DECIMAL_PRECISION);
        Self::recompute_attribution_residual(result, history, ExternalFlowBasis::BaseCurrency);
    }

    async fn apply_trade_fee_pnl_gross_up_best_effort(
        &self,
        result: &mut PerformanceResult,
        account_ids: &[String],
        history: &[DailyAccountValuation],
    ) {
        let Some(activity_repository) = &self.activity_repository else {
            return;
        };
        let Some(lot_repository) = &self.lot_repository else {
            return;
        };
        let Some(start_date) = result.period.start_date else {
            return;
        };
        let Some(end_date) = result.period.end_date else {
            return;
        };
        if account_ids.is_empty() {
            return;
        }

        let start_utc = (start_date - Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let end_utc = (end_date + Duration::days(1))
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_utc();

        let activities = match activity_repository.get_activities_by_account_ids_in_date_range(
            account_ids,
            start_utc,
            end_utc,
        ) {
            Ok(activities) => activities,
            Err(e) => {
                warn!(
                    "Failed to load activities for trade-fee performance attribution scope {}: {}",
                    result.scope.id, e
                );
                return;
            }
        };

        let mut buy_fee_by_activity = HashMap::<String, Decimal>::new();
        let mut sell_fee_by_activity = HashMap::<String, Decimal>::new();
        for activity in activities {
            if !activity.is_posted() || activity.fee_amt().is_zero() {
                continue;
            }

            let activity_date = self.activity_local_date(&activity);
            if activity_date <= start_date || activity_date > end_date {
                continue;
            }

            let Ok(activity_type) = ActivityType::from_str(activity.effective_type()) else {
                continue;
            };
            if !matches!(activity_type, ActivityType::Buy | ActivityType::Sell) {
                continue;
            }

            let Some(fee) = self.convert_activity_amount_for_attribution(
                &activity,
                activity.fee_amt(),
                &result.scope.currency,
                activity_date,
            ) else {
                continue;
            };

            match activity_type {
                ActivityType::Buy => {
                    buy_fee_by_activity.insert(activity.id, fee);
                }
                ActivityType::Sell => {
                    sell_fee_by_activity.insert(activity.id, fee);
                }
                _ => {}
            }
        }

        if buy_fee_by_activity.is_empty() && sell_fee_by_activity.is_empty() {
            return;
        }

        let mut lot_by_account_and_id = HashMap::<(String, String), LotRecord>::new();
        for account_id in account_ids {
            match lot_repository.get_all_lots_for_account(account_id).await {
                Ok(lots) => {
                    for lot in lots {
                        lot_by_account_and_id.insert((account_id.clone(), lot.id.clone()), lot);
                    }
                }
                Err(e) => warn!(
                    "Failed to load lots for trade-fee performance attribution account {}: {}",
                    account_id, e
                ),
            }
        }

        let mut acquisition_fees_disposed = Decimal::ZERO;
        let mut disposal_activity_ids = HashSet::<String>::new();
        for account_id in account_ids {
            let disposals = match lot_repository
                .get_lot_disposals_for_account(account_id)
                .await
            {
                Ok(disposals) => disposals,
                Err(e) => {
                    warn!(
                        "Failed to load lot disposals for trade-fee attribution account {}: {}",
                        account_id, e
                    );
                    continue;
                }
            };

            for disposal in disposals {
                let Ok(disposal_date) =
                    NaiveDate::parse_from_str(&disposal.disposal_date, "%Y-%m-%d")
                else {
                    continue;
                };
                if disposal_date <= start_date || disposal_date > end_date {
                    continue;
                }

                disposal_activity_ids.insert(disposal.disposal_activity_id.clone());

                let Some(lot) =
                    lot_by_account_and_id.get(&(account_id.clone(), disposal.lot_id.clone()))
                else {
                    continue;
                };
                let Ok(open_date) = NaiveDate::parse_from_str(&lot.open_date, "%Y-%m-%d") else {
                    continue;
                };
                if open_date <= start_date || open_date > end_date {
                    continue;
                }

                let original_cost_basis_base = parse_decimal_lossy(&lot.original_cost_basis_base);
                let fee_allocated_base = parse_decimal_lossy(&lot.fee_allocated_base);
                let disposal_cost_basis_base = parse_decimal_lossy(&disposal.cost_basis_base);
                if original_cost_basis_base <= Decimal::ZERO || fee_allocated_base.is_zero() {
                    continue;
                }

                acquisition_fees_disposed +=
                    disposal_cost_basis_base * fee_allocated_base / original_cost_basis_base;
            }
        }

        let period_buy_fees = buy_fee_by_activity
            .values()
            .copied()
            .sum::<Decimal>()
            .round_dp(DECIMAL_PRECISION);
        let period_sell_fees = disposal_activity_ids
            .iter()
            .filter_map(|activity_id| sell_fee_by_activity.get(activity_id))
            .copied()
            .sum::<Decimal>()
            .round_dp(DECIMAL_PRECISION);
        let acquisition_fees_disposed = acquisition_fees_disposed
            .min(period_buy_fees)
            .round_dp(DECIMAL_PRECISION);
        let remaining_period_buy_fees =
            (period_buy_fees - acquisition_fees_disposed).round_dp(DECIMAL_PRECISION);

        if period_sell_fees.is_zero()
            && acquisition_fees_disposed.is_zero()
            && remaining_period_buy_fees.is_zero()
        {
            return;
        }

        result.attribution.realized_pnl =
            (result.attribution.realized_pnl + period_sell_fees + acquisition_fees_disposed)
                .round_dp(DECIMAL_PRECISION);
        result.attribution.unrealized_pnl_change = (result.attribution.unrealized_pnl_change
            + remaining_period_buy_fees)
            .round_dp(DECIMAL_PRECISION);
        Self::recompute_attribution_residual(result, history, ExternalFlowBasis::BaseCurrency);
    }

    fn recompute_attribution_residual(
        result: &mut PerformanceResult,
        history: &[DailyAccountValuation],
        flow_basis: ExternalFlowBasis,
    ) {
        let Some(start_point) = history.first() else {
            return;
        };
        let Some(end_point) = history.last() else {
            return;
        };

        let start_value = Self::return_total_value(start_point, flow_basis);
        let end_value = Self::return_total_value(end_point, flow_basis);
        let delta_total_value = end_value - start_value;
        result.attribution.residual = (delta_total_value
            - (result.attribution.contributions - result.attribution.distributions
                + result.attribution.income
                + result.attribution.realized_pnl
                + result.attribution.unrealized_pnl_change
                + result.attribution.fx_effect
                - result.attribution.fees
                - result.attribution.taxes))
            .round_dp(DECIMAL_PRECISION);

        result
            .data_quality
            .warnings
            .retain(|warning| !warning.starts_with("Attribution residual "));
        let residual_threshold = Decimal::ONE.max(
            (delta_total_value
                .abs()
                .max(end_value.abs())
                .max(Decimal::ONE))
                * dec!(0.001),
        );
        if result.attribution.residual.abs() > residual_threshold {
            result.data_quality.warnings.push(format!(
                "Attribution residual {} exceeds tolerance {}; result is partially unreliable.",
                result.attribution.residual.round_dp(DECIMAL_PRECISION),
                residual_threshold.round_dp(DECIMAL_PRECISION)
            ));
        }
        Self::refresh_data_quality_status(&mut result.data_quality);
    }

    /// HOLDINGS-mode period gain and return.
    ///
    /// HOLDINGS mode doesn't track cash flows at the transaction level, so
    /// TWR/IRR aren't meaningful — we measure unrealized P&L growth instead.
    ///
    /// * `is_all_time` — when `true`, divides by ending `cost_basis` (the full
    ///   amount invested). When `false`, divides by `investment_market_value`
    ///   at the period start. Non-positive denominators make the percentage
    ///   undefined, so the return is omitted rather than reported as 0%.
    fn compute_holdings_value_return(
        start_point: &DailyAccountValuation,
        end_point: &DailyAccountValuation,
        is_all_time: bool,
        flow_basis: ExternalFlowBasis,
    ) -> (Decimal, Option<Decimal>) {
        let start_unrealized_pnl = Self::return_investment_market_value(start_point, flow_basis)
            - Self::return_cost_basis(start_point, flow_basis);
        let end_unrealized_pnl = Self::return_investment_market_value(end_point, flow_basis)
            - Self::return_cost_basis(end_point, flow_basis);
        let pnl_change = end_unrealized_pnl - start_unrealized_pnl;

        let value_return = if is_all_time {
            let end_cost_basis = Self::return_cost_basis(end_point, flow_basis);
            if end_cost_basis <= Decimal::ZERO {
                None
            } else {
                Some(end_unrealized_pnl / end_cost_basis)
            }
        } else {
            let start_market_value = Self::return_investment_market_value(start_point, flow_basis);
            if start_market_value <= Decimal::ZERO {
                None
            } else {
                Some(pnl_change / start_market_value)
            }
        };

        (pnl_change, value_return)
    }

    fn unrealized_attribution_components(
        start_point: &DailyAccountValuation,
        end_point: &DailyAccountValuation,
        flow_basis: ExternalFlowBasis,
    ) -> (Decimal, Decimal) {
        let base_unrealized_change = (Self::return_investment_market_value(end_point, flow_basis)
            - Self::return_cost_basis(end_point, flow_basis))
            - (Self::return_investment_market_value(start_point, flow_basis)
                - Self::return_cost_basis(start_point, flow_basis));

        if !matches!(flow_basis, ExternalFlowBasis::BaseCurrency)
            || start_point.account_currency == start_point.base_currency
        {
            return (base_unrealized_change, Decimal::ZERO);
        }

        let local_unrealized_change = (end_point.investment_market_value - end_point.cost_basis)
            - (start_point.investment_market_value - start_point.cost_basis);
        let local_change_at_end_fx = local_unrealized_change * end_point.fx_rate_to_base;
        (
            local_change_at_end_fx.round_dp(DECIMAL_PRECISION),
            (base_unrealized_change - local_change_at_end_fx).round_dp(DECIMAL_PRECISION),
        )
    }

    fn account_unrealized_local(point: &DailyAccountValuation) -> Decimal {
        point.investment_market_value - point.cost_basis
    }

    fn account_unrealized_base(point: &DailyAccountValuation) -> Decimal {
        point.investment_market_value_base - point.cost_basis_base
    }

    fn scoped_unrealized_attribution_components(
        account_histories: &[Vec<DailyAccountValuation>],
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> ScopedUnrealizedAttribution {
        let mut unrealized_pnl_change = Decimal::ZERO;
        let mut fx_effect = Decimal::ZERO;
        let mut warnings = Vec::new();
        let mut saw_account = false;
        let mut complete = true;

        for history in account_histories {
            if history.is_empty() {
                continue;
            }

            let start_point = history
                .iter()
                .filter(|point| point.valuation_date <= start_date)
                .max_by_key(|point| point.valuation_date);
            let Some(end_point) = history
                .iter()
                .filter(|point| point.valuation_date <= end_date)
                .max_by_key(|point| point.valuation_date)
            else {
                continue;
            };

            let end_fx_rate = if end_point.account_currency == end_point.base_currency {
                Decimal::ONE
            } else {
                end_point.fx_rate_to_base
            };
            if end_fx_rate <= Decimal::ZERO {
                complete = false;
                warnings.push(format!(
                    "Scoped FX attribution skipped for account {} because its end-date FX rate is unavailable.",
                    end_point.account_id
                ));
                continue;
            }

            let start_unrealized_local =
                start_point.map_or(Decimal::ZERO, Self::account_unrealized_local);
            let start_unrealized_base =
                start_point.map_or(Decimal::ZERO, Self::account_unrealized_base);
            let local_unrealized_change =
                Self::account_unrealized_local(end_point) - start_unrealized_local;
            let base_unrealized_change =
                Self::account_unrealized_base(end_point) - start_unrealized_base;
            let local_change_at_end_fx = local_unrealized_change * end_fx_rate;

            unrealized_pnl_change += local_change_at_end_fx;
            fx_effect += base_unrealized_change - local_change_at_end_fx;
            saw_account = true;
        }

        ScopedUnrealizedAttribution {
            unrealized_pnl_change: unrealized_pnl_change.round_dp(DECIMAL_PRECISION),
            fx_effect: fx_effect.round_dp(DECIMAL_PRECISION),
            warnings,
            complete: complete && saw_account,
        }
    }

    /// Full account performance calculation including per-day `returns[]`,
    /// volatility, and max-drawdown. Used by the account-detail page.
    async fn calculate_account_performance(
        &self,
        account_id: &str,
        start_date_opt: Option<NaiveDate>,
        end_date_opt: Option<NaiveDate>,
        tracking_mode: Option<TrackingMode>,
    ) -> Result<PerformanceResult> {
        if let (Some(start), Some(end)) = (start_date_opt, end_date_opt) {
            if start > end {
                return Err(errors::Error::Validation(ValidationError::InvalidInput(
                    "Start date must be before end date".to_string(),
                )));
            }
        }

        let full_history = self.valuation_service.get_historical_valuations(
            account_id,
            start_date_opt,
            end_date_opt,
        )?;

        if full_history.len() < 2 {
            warn!("Performance calculation for account '{}': Not enough valuation data ({} points). Returning empty response.", account_id, full_history.len());
            let currency = full_history
                .first()
                .map(|point| point.base_currency.as_str())
                .unwrap_or("");
            let start_date = full_history
                .first()
                .map(|point| point.valuation_date)
                .or(start_date_opt);
            let end_date = full_history
                .last()
                .map(|point| point.valuation_date)
                .or(end_date_opt);
            return Ok(PerformanceService::empty_response_with_context(
                account_id,
                currency,
                start_date,
                end_date,
                "Performance unavailable: at least two valuation points are required.",
            ));
        }

        let mut metrics =
            Self::compute_account_performance(&full_history, tracking_mode, start_date_opt, true)?;
        metrics.scope.id = account_id.to_string();
        self.apply_external_attribution_best_effort(
            &mut metrics,
            &[account_id.to_string()],
            &full_history,
        )
        .await;
        Ok(metrics)
    }

    /// Summary account performance calculation (no `returns[]`, no risk
    /// metrics). Used by the dashboard card. Shares the same TWR/IRR chain as
    /// the full path so percentages match the account-detail page.
    async fn calculate_account_performance_summary(
        &self,
        account_id: &str,
        start_date_opt: Option<NaiveDate>,
        end_date_opt: Option<NaiveDate>,
        tracking_mode: Option<TrackingMode>,
    ) -> Result<PerformanceResult> {
        if let (Some(start), Some(end)) = (start_date_opt, end_date_opt) {
            if start > end {
                return Err(errors::Error::Validation(ValidationError::InvalidInput(
                    "Start date must be before end date".to_string(),
                )));
            }
        }

        let full_history = self.valuation_service.get_historical_valuations(
            account_id,
            start_date_opt,
            end_date_opt,
        )?;

        if full_history.len() < 2 {
            warn!(
                "Account '{}': Not enough history data ({} points). Returning empty performance response.",
                account_id,
                full_history.len()
            );
            let currency = full_history
                .first()
                .map(|point| point.base_currency.as_str())
                .unwrap_or("");
            let start_date = full_history
                .first()
                .map(|point| point.valuation_date)
                .or(start_date_opt);
            let end_date = full_history
                .last()
                .map(|point| point.valuation_date)
                .or(end_date_opt);
            return Ok(PerformanceService::empty_response_with_context(
                account_id,
                currency,
                start_date,
                end_date,
                "Performance unavailable: at least two valuation points are required.",
            ));
        }

        let mut metrics =
            Self::compute_account_performance(&full_history, tracking_mode, start_date_opt, false)?;
        metrics.scope.id = account_id.to_string();
        self.apply_external_attribution_best_effort(
            &mut metrics,
            &[account_id.to_string()],
            &full_history,
        )
        .await;
        Ok(metrics)
    }

    async fn calculate_scoped_performance(
        &self,
        request: ScopedPerformanceRequest<'_>,
    ) -> Result<PerformanceResult> {
        let ScopedPerformanceRequest {
            scope_id,
            account_ids,
            base_currency,
            account_tracking_modes,
            start_date: start_date_opt,
            end_date: end_date_opt,
            include_returns_series,
        } = request;

        if let (Some(start), Some(end)) = (start_date_opt, end_date_opt) {
            if start > end {
                return Err(errors::Error::Validation(ValidationError::InvalidInput(
                    "Start date must be before end date".to_string(),
                )));
            }
        }

        if account_ids.is_empty() {
            return Ok(PerformanceService::empty_response_with_context(
                scope_id,
                base_currency,
                start_date_opt,
                end_date_opt,
                "Performance unavailable: no accounts selected.",
            ));
        }
        let scoped_tracking_composition =
            Self::scoped_tracking_composition(account_ids, account_tracking_modes);

        let full_history = match self
            .valuation_service
            .get_historical_valuations_for_accounts(
                scope_id,
                account_ids,
                base_currency,
                start_date_opt,
                end_date_opt,
            ) {
            Ok(history) => history,
            Err(errors::Error::Calculation(error)) => {
                let warning = format!(
                    "Performance is partially unavailable for this scope because valuation history is incomplete: {}",
                    error
                );
                warn!("{}", warning);
                return Ok(PerformanceService::partial_response(
                    scope_id,
                    base_currency,
                    start_date_opt,
                    end_date_opt,
                    warning,
                ));
            }
            Err(error) => return Err(error),
        };

        if full_history.len() < 2 {
            let start_date = full_history
                .first()
                .map(|point| point.valuation_date)
                .or(start_date_opt);
            let end_date = full_history
                .last()
                .map(|point| point.valuation_date)
                .or(end_date_opt);
            return Ok(PerformanceService::empty_response_with_context(
                scope_id,
                base_currency,
                start_date,
                end_date,
                "Performance unavailable: at least two valuation points are required.",
            ));
        }

        let mut metrics = match scoped_tracking_composition {
            ScopedTrackingComposition::TransactionsOnly => {
                Self::compute_scoped_account_performance(
                    &full_history,
                    Some(TrackingMode::Transactions),
                    start_date_opt,
                    include_returns_series,
                )?
            }
            ScopedTrackingComposition::HoldingsOnly => Self::compute_scoped_account_performance(
                &full_history,
                Some(TrackingMode::Holdings),
                start_date_opt,
                include_returns_series,
            )?,
            ScopedTrackingComposition::Mixed => {
                Self::compute_mixed_scope_performance(&full_history, include_returns_series)?
            }
        };

        metrics.scope.id = scope_id.to_string();
        self.apply_scoped_unrealized_attribution_best_effort(
            &mut metrics,
            account_ids,
            &full_history,
        )
        .await;
        self.apply_external_attribution_best_effort(&mut metrics, account_ids, &full_history)
            .await;
        Ok(metrics)
    }

    fn scoped_tracking_composition(
        account_ids: &[String],
        account_tracking_modes: &HashMap<String, TrackingMode>,
    ) -> ScopedTrackingComposition {
        let mut has_holdings = false;
        let mut has_transactions = false;

        for account_id in account_ids {
            if matches!(
                account_tracking_modes.get(account_id),
                Some(TrackingMode::Holdings)
            ) {
                has_holdings = true;
            } else {
                has_transactions = true;
            }
        }

        match (has_transactions, has_holdings) {
            (true, true) => ScopedTrackingComposition::Mixed,
            (false, true) => ScopedTrackingComposition::HoldingsOnly,
            _ => ScopedTrackingComposition::TransactionsOnly,
        }
    }

    /// Pure computation shared by the full and summary paths. Takes a
    /// pre-fetched valuation history and produces the same `PerformanceResult`
    /// both call sites need.
    ///
    /// * `include_returns_series` — when `true`, populates `returns[]` with a
    ///   per-day cumulative TWR and computes volatility/max-drawdown. The full
    ///   path sets this; the summary doesn't to save allocation on dashboards
    ///   with many accounts.
    ///
    /// `id` is left empty — callers set it after.
    ///
    /// # Precondition
    /// `full_history.len() >= 2`. Callers check this first so they can respond
    /// differently to insufficient history (empty response vs. error).
    fn compute_account_performance(
        full_history: &[DailyAccountValuation],
        tracking_mode: Option<TrackingMode>,
        start_date_opt: Option<NaiveDate>,
        include_returns_series: bool,
    ) -> Result<PerformanceResult> {
        Self::compute_account_performance_with_flow_basis(
            full_history,
            tracking_mode,
            start_date_opt,
            include_returns_series,
            ExternalFlowBasis::BaseCurrency,
        )
    }

    fn compute_scoped_account_performance(
        full_history: &[DailyAccountValuation],
        tracking_mode: Option<TrackingMode>,
        start_date_opt: Option<NaiveDate>,
        include_returns_series: bool,
    ) -> Result<PerformanceResult> {
        Self::compute_account_performance_with_flow_basis(
            full_history,
            tracking_mode,
            start_date_opt,
            include_returns_series,
            ExternalFlowBasis::BaseCurrency,
        )
    }

    fn compute_account_performance_with_flow_basis(
        full_history: &[DailyAccountValuation],
        tracking_mode: Option<TrackingMode>,
        start_date_opt: Option<NaiveDate>,
        include_returns_series: bool,
        flow_basis: ExternalFlowBasis,
    ) -> Result<PerformanceResult> {
        debug_assert!(full_history.len() >= 2);

        let start_point = full_history.first().unwrap();
        let end_point = full_history.last().unwrap();
        let actual_start_date = start_point.valuation_date;
        let actual_end_date = end_point.valuation_date;
        let currency = match flow_basis {
            ExternalFlowBasis::AccountCurrency => start_point.account_currency.clone(),
            ExternalFlowBasis::BaseCurrency => start_point.base_currency.clone(),
        };
        let is_holdings_mode = matches!(tracking_mode, Some(TrackingMode::Holdings));

        let start_value = Self::return_total_value(start_point, flow_basis);
        let end_value = Self::return_total_value(end_point, flow_basis);
        let daily_flows = Self::daily_external_flow_series(full_history, flow_basis);

        let twr = if is_holdings_mode {
            TwrComputation {
                cumulative_twr: None,
                samples: Vec::new(),
                warnings: Vec::new(),
                not_applicable_reasons: vec![
                    "TWR unavailable for holdings-only scopes because transaction cash flows are not tracked.".to_string(),
                ],
            }
        } else {
            Self::compute_time_weighted_returns(full_history, &daily_flows, flow_basis)?
        };
        let irr = if is_holdings_mode {
            IrrComputation {
                irr: None,
                warnings: Vec::new(),
                not_applicable_reasons: vec![
                    "IRR unavailable for holdings-only scopes because transaction cash flows are not tracked.".to_string(),
                ],
            }
        } else {
            Self::calculate_xirr(full_history, &daily_flows, flow_basis)
        };

        let mut risk_samples = Vec::new();
        let mut series = Vec::new();
        if include_returns_series {
            series.push(ReturnData {
                date: actual_start_date,
                value: Decimal::ZERO,
            });
        }

        if is_holdings_mode {
            let mut cumulative_value_factor = Decimal::ONE;
            for (index, window) in full_history.windows(2).enumerate() {
                let prev = &window[0];
                let curr = &window[1];
                let prev_value = Self::return_total_value(prev, flow_basis);
                let curr_value = Self::return_total_value(curr, flow_basis);
                let flow = daily_flows[index];
                let day_gain = curr_value + flow.outflow - prev_value - flow.inflow;
                if prev_value > Decimal::ZERO {
                    let daily_return = day_gain / prev_value;
                    cumulative_value_factor *= Decimal::ONE + daily_return;
                    risk_samples.push(RiskSample {
                        date: curr.valuation_date,
                        simple_return: daily_return,
                    });
                    if include_returns_series {
                        series.push(ReturnData {
                            date: curr.valuation_date,
                            value: (cumulative_value_factor - Decimal::ONE)
                                .round_dp(DECIMAL_PRECISION),
                        });
                    }
                } else if include_returns_series {
                    series.push(ReturnData {
                        date: curr.valuation_date,
                        value: Decimal::ZERO,
                    });
                }
            }
        } else {
            for (date, sample) in &twr.samples {
                if !sample.excluded_from_compounding {
                    risk_samples.push(RiskSample {
                        date: *date,
                        simple_return: sample.twr,
                    });
                }
                if include_returns_series {
                    series.push(ReturnData {
                        date: *date,
                        value: sample.cumulative_twr_to_date.round_dp(DECIMAL_PRECISION),
                    });
                }
            }
        }

        let risk = Self::risk_from_samples(&risk_samples, Some(actual_start_date));

        let (mode, value_return, value_return_not_applicable_reason) = if is_holdings_mode {
            let (_pnl_change, ret) = Self::compute_holdings_value_return(
                start_point,
                end_point,
                start_date_opt.is_none(),
                flow_basis,
            );
            let reason = if ret.is_none() {
                Some(if start_date_opt.is_none() {
                    "Value return unavailable for holdings-only scope because ending cost basis is zero or negative."
                        .to_string()
                } else {
                    "Value return unavailable for holdings-only scope because starting market value is zero or negative."
                        .to_string()
                })
            } else {
                None
            };
            (ReturnMethod::ValueReturn, ret, reason)
        } else {
            let value_return =
                Self::compute_simple_value_return(full_history, &daily_flows, flow_basis);
            let reason = if value_return.is_none() {
                Some(
                    "Value return unavailable for transaction-mode scope because starting value is zero or negative."
                        .to_string(),
                )
            } else {
                None
            };
            (ReturnMethod::TimeWeighted, value_return, reason)
        };

        let (contributions, distributions) = Self::total_external_flows(&daily_flows);
        let (unrealized_pnl_change, fx_effect) =
            Self::unrealized_attribution_components(start_point, end_point, flow_basis);
        let delta_total_value = end_value - start_value;
        let mut attribution = PerformanceAttribution {
            contributions,
            distributions,
            unrealized_pnl_change,
            fx_effect,
            ..PerformanceAttribution::default()
        };
        attribution.residual = delta_total_value
            - (attribution.contributions - attribution.distributions
                + attribution.income
                + attribution.realized_pnl
                + attribution.unrealized_pnl_change
                + attribution.fx_effect
                - attribution.fees
                - attribution.taxes);

        let mut warnings = Self::external_flow_quality_warnings(&daily_flows);
        warnings.extend(twr.warnings);
        warnings.extend(irr.warnings);
        let residual_threshold = Decimal::ONE.max(
            (delta_total_value
                .abs()
                .max(end_value.abs())
                .max(Decimal::ONE))
                * dec!(0.001),
        );
        if attribution.residual.abs() > residual_threshold {
            warnings.push(format!(
                "Attribution residual {} exceeds tolerance {}; result is partially unreliable.",
                attribution.residual.round_dp(DECIMAL_PRECISION),
                residual_threshold.round_dp(DECIMAL_PRECISION)
            ));
        }
        if risk.volatility.is_some() {
            warnings
                .push("Volatility is annualized with equity-style sqrt(252) scaling.".to_string());
        }

        let mut not_applicable_reasons = twr.not_applicable_reasons;
        not_applicable_reasons.extend(irr.not_applicable_reasons);
        if let Some(reason) = value_return_not_applicable_reason {
            not_applicable_reasons.push(reason);
        }

        Ok(Self::build_result(
            String::new(),
            currency,
            Some(actual_start_date),
            Some(actual_end_date),
            mode,
            PerformanceReturns {
                twr: twr
                    .cumulative_twr
                    .map(|value| value.round_dp(DECIMAL_PRECISION)),
                annualized_twr: Self::annualize_optional_return(
                    actual_start_date,
                    actual_end_date,
                    twr.cumulative_twr,
                ),
                irr: irr.irr,
                annualized_irr: irr.irr,
                value_return: value_return.map(|value| value.round_dp(DECIMAL_PRECISION)),
                annualized_value_return: Self::annualize_optional_return(
                    actual_start_date,
                    actual_end_date,
                    value_return,
                ),
            },
            attribution,
            risk,
            Self::data_quality(warnings, not_applicable_reasons, false),
            series,
            is_holdings_mode,
            false,
        ))
    }

    fn compute_mixed_scope_performance(
        full_history: &[DailyAccountValuation],
        include_returns_series: bool,
    ) -> Result<PerformanceResult> {
        debug_assert!(full_history.len() >= 2);

        let start_point = full_history.first().unwrap();
        let end_point = full_history.last().unwrap();
        let actual_start_date = start_point.valuation_date;
        let actual_end_date = end_point.valuation_date;
        let currency = start_point.base_currency.clone();
        let flow_basis = ExternalFlowBasis::BaseCurrency;
        let start_value = Self::return_total_value(start_point, flow_basis);
        let end_value = Self::return_total_value(end_point, flow_basis);
        let daily_flows = Self::daily_external_flow_series(full_history, flow_basis);
        let net_cash_flow: Decimal = daily_flows.iter().map(|flow| flow.net()).sum();
        let gain_loss_amount = end_value - start_value - net_cash_flow;
        let value_return = if start_value > Decimal::ZERO {
            Some(gain_loss_amount / start_value)
        } else {
            None
        };
        if full_history
            .iter()
            .any(|point| Self::return_total_value(point, flow_basis).is_sign_negative())
        {
            return Err(errors::Error::Validation(ValidationError::InvalidInput(
                "Account scope has negative portfolio value in its history. Please review the underlying transactions and holdings.".to_string(),
            )));
        }

        let mut returns = Vec::new();
        let mut risk_samples = Vec::new();

        if include_returns_series && value_return.is_some() {
            returns.reserve(full_history.len());
            returns.push(ReturnData {
                date: actual_start_date,
                value: Decimal::ZERO,
            });
        }

        let mut cumulative_external_flow = Decimal::ZERO;
        for (window, flow) in full_history.windows(2).zip(daily_flows.iter()) {
            let prev_point = &window[0];
            let curr_point = &window[1];
            let prev_value = Self::return_total_value(prev_point, flow_basis);
            let curr_value = Self::return_total_value(curr_point, flow_basis);
            if prev_value.is_sign_negative() || curr_value.is_sign_negative() {
                return Err(errors::Error::Validation(ValidationError::InvalidInput(
                    "Account scope has negative portfolio value in its history. Please review the underlying transactions and holdings.".to_string(),
                )));
            }

            cumulative_external_flow += flow.net();
            let cumulative_return = if start_value > Decimal::ZERO {
                let cumulative_gain = curr_value - start_value - cumulative_external_flow;
                Some(cumulative_gain / start_value)
            } else {
                None
            };

            let day_gain = curr_value + flow.outflow - prev_value - flow.inflow;
            if prev_value > Decimal::ZERO {
                let daily_return = day_gain / prev_value;
                risk_samples.push(RiskSample {
                    date: curr_point.valuation_date,
                    simple_return: daily_return,
                });
            }

            if include_returns_series {
                let Some(cumulative_return) = cumulative_return else {
                    continue;
                };
                returns.push(ReturnData {
                    date: curr_point.valuation_date,
                    value: cumulative_return.round_dp(DECIMAL_PRECISION),
                });
            }
        }

        let (contributions, distributions) = Self::total_external_flows(&daily_flows);
        let (unrealized_pnl_change, fx_effect) =
            Self::unrealized_attribution_components(start_point, end_point, flow_basis);
        let delta_total_value = end_value - start_value;
        let mut attribution = PerformanceAttribution {
            contributions,
            distributions,
            unrealized_pnl_change,
            fx_effect,
            ..PerformanceAttribution::default()
        };
        attribution.residual = delta_total_value
            - (attribution.contributions - attribution.distributions
                + attribution.income
                + attribution.realized_pnl
                + attribution.unrealized_pnl_change
                + attribution.fx_effect
                - attribution.fees
                - attribution.taxes);
        let risk = Self::risk_from_samples(&risk_samples, Some(actual_start_date));
        let mut warnings = vec![
            "This scope mixes transaction-mode and holdings-mode accounts, so TWR and IRR are unavailable. The return is a value return over the selected scope.".to_string(),
        ];
        warnings.extend(Self::external_flow_quality_warnings(&daily_flows));
        if risk.volatility.is_some() {
            warnings
                .push("Volatility is annualized with equity-style sqrt(252) scaling.".to_string());
        }
        let mut not_applicable_reasons = vec![
            "TWR unavailable for mixed transaction and holdings scopes.".to_string(),
            "IRR unavailable for mixed transaction and holdings scopes.".to_string(),
        ];
        if value_return.is_none() {
            not_applicable_reasons.push(
                "Value return unavailable for mixed scope because starting value is zero or negative."
                    .to_string(),
            );
        }

        Ok(Self::build_result(
            String::new(),
            currency,
            Some(actual_start_date),
            Some(actual_end_date),
            ReturnMethod::ValueReturn,
            PerformanceReturns {
                twr: None,
                annualized_twr: None,
                irr: None,
                annualized_irr: None,
                value_return: value_return.map(|value| value.round_dp(DECIMAL_PRECISION)),
                annualized_value_return: Self::annualize_optional_return(
                    actual_start_date,
                    actual_end_date,
                    value_return,
                ),
            },
            attribution,
            risk,
            Self::data_quality(warnings, not_applicable_reasons, false),
            returns,
            false,
            true,
        ))
    }

    /// Internal function for calculating symbol/benchmark performance (Full)
    /// asset_id can be a canonical ID like "SEC:^GSPC:INDEX" or a raw symbol
    async fn calculate_symbol_performance(
        &self,
        asset_id: &str,
        start_date_opt: Option<NaiveDate>,
        end_date_opt: Option<NaiveDate>,
    ) -> Result<PerformanceResult> {
        let effective_end_date = end_date_opt.unwrap_or_else(|| self.today_in_user_timezone());
        let effective_start_date =
            start_date_opt.unwrap_or_else(|| effective_end_date - chrono::Duration::days(365));

        if effective_start_date > effective_end_date {
            return Err(errors::Error::Validation(ValidationError::InvalidInput(
                format!(
                    "Effective start date {} must be before effective end date {}",
                    effective_start_date, effective_end_date
                ),
            )));
        }

        // Use fetch_quotes_for_symbol which handles both existing assets and canonical IDs
        let quote_history = self
            .quote_service
            .fetch_quotes_for_symbol(asset_id, "USD", effective_start_date, effective_end_date)
            .await?;

        if quote_history.is_empty() {
            warn!(
                "Asset '{}': No quote data found between {} and {}. Returning empty response.",
                asset_id, effective_start_date, effective_end_date
            );
            return Ok(PerformanceService::empty_response_with_context(
                asset_id,
                "USD",
                Some(effective_start_date),
                Some(effective_end_date),
                "Performance unavailable: no quote data found for the selected period.",
            ));
        }

        let currency = quote_history.first().unwrap().currency.clone();
        let mut quote_points: Vec<(NaiveDate, Decimal)> = quote_history
            .into_iter()
            .map(|quote| {
                (
                    quote.timestamp.date_naive(),
                    quote.close.round_dp(DECIMAL_PRECISION),
                )
            })
            .collect();
        quote_points.sort_by_key(|(date, _)| *date);
        quote_points.dedup_by_key(|(date, _)| *date);

        if quote_points.len() < 2 {
            warn!(
                "Asset '{}': Only one quote data point found between {} and {}. Returning empty response.",
                asset_id, effective_start_date, effective_end_date
            );
            return Ok(PerformanceService::empty_response_with_context(
                asset_id,
                &currency,
                Some(effective_start_date),
                Some(effective_end_date),
                "Performance unavailable: at least two quote points are required.",
            ));
        }

        let Some((actual_start_date, start_price)) = quote_points.first().copied() else {
            return Ok(PerformanceService::empty_response_with_context(
                asset_id,
                &currency,
                Some(effective_start_date),
                Some(effective_end_date),
                "Performance unavailable: no quote data found for the selected period.",
            ));
        };
        let Some((actual_end_date, end_price)) = quote_points.last().copied() else {
            return Ok(PerformanceService::empty_response_with_context(
                asset_id,
                &currency,
                Some(effective_start_date),
                Some(effective_end_date),
                "Performance unavailable: no quote data found for the selected period.",
            ));
        };
        if start_price <= Decimal::ZERO {
            warn!(
                "Asset '{}': starting quote price is non-positive on {}. Returning empty response.",
                asset_id, actual_start_date
            );
            return Ok(PerformanceService::empty_response_with_context(
                asset_id,
                &currency,
                Some(actual_start_date),
                Some(actual_end_date),
                "Performance unavailable: starting quote price is non-positive.",
            ));
        }
        let mut returns = Vec::with_capacity(quote_points.len());
        let mut risk_samples = Vec::with_capacity(quote_points.len().saturating_sub(1));
        let mut cumulative_value = Decimal::ONE;
        let mut prev_price = start_price;
        returns.push(ReturnData {
            date: actual_start_date,
            value: Decimal::ZERO,
        });

        for (date, price) in quote_points.iter().copied().skip(1) {
            if price <= Decimal::ZERO || prev_price <= Decimal::ZERO {
                prev_price = price;
                continue;
            }
            let daily_return = (price / prev_price) - Decimal::ONE;
            risk_samples.push(RiskSample {
                date,
                simple_return: daily_return,
            });
            cumulative_value *= Decimal::ONE + daily_return;
            returns.push(ReturnData {
                date,
                value: (cumulative_value - Decimal::ONE).round_dp(DECIMAL_PRECISION),
            });
            prev_price = price;
        }

        let total_return = if start_price.is_zero() {
            Decimal::ZERO
        } else {
            (end_price / start_price) - Decimal::ONE
        };
        let annualized_return =
            Self::calculate_annualized_return(actual_start_date, actual_end_date, total_return);
        let result = Self::build_result(
            asset_id.to_string(),
            currency,
            Some(actual_start_date),
            Some(actual_end_date),
            ReturnMethod::SymbolPriceBased,
            PerformanceReturns {
                twr: None,
                annualized_twr: None,
                irr: None,
                annualized_irr: None,
                value_return: Some(total_return.round_dp(DECIMAL_PRECISION)),
                annualized_value_return: Some(annualized_return.round_dp(DECIMAL_PRECISION)),
            },
            PerformanceAttribution::default(),
            Self::risk_from_samples(&risk_samples, Some(actual_start_date)),
            Self::data_quality(
                vec![
                    "Symbol-only performance uses price quotes only; dividends and distributions are excluded unless the quote series is total-return adjusted.".to_string(),
                    "Volatility is annualized with equity-style sqrt(252) scaling and uses quoted market-movement days only.".to_string(),
                ],
                vec![
                    "TWR unavailable for symbol-only price performance because there is no portfolio cash-flow scope.".to_string(),
                    "IRR unavailable for symbol-only price performance because there are no user cash flows.".to_string(),
                ],
                false,
            ),
            returns,
            false,
            false,
        );

        Ok(result)
    }

    fn empty_response_with_context(
        id: &str,
        currency: &str,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
        reason: impl Into<String>,
    ) -> PerformanceResult {
        Self::build_result(
            id.to_string(),
            currency.to_string(),
            start_date,
            end_date,
            ReturnMethod::NotApplicable,
            PerformanceReturns {
                twr: None,
                annualized_twr: None,
                irr: None,
                annualized_irr: None,
                value_return: None,
                annualized_value_return: None,
            },
            PerformanceAttribution::default(),
            PerformanceRisk {
                volatility: None,
                max_drawdown: None,
                peak_date: None,
                trough_date: None,
                recovery_date: None,
                drawdown_duration_days: None,
            },
            PerformanceDataQuality::no_data(reason),
            Vec::new(),
            false,
            false,
        )
    }

    fn partial_response(
        id: &str,
        currency: &str,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
        warning: String,
    ) -> PerformanceResult {
        Self::build_result(
            id.to_string(),
            currency.to_string(),
            start_date,
            end_date,
            ReturnMethod::NotApplicable,
            PerformanceReturns {
                twr: None,
                annualized_twr: None,
                irr: None,
                annualized_irr: None,
                value_return: None,
                annualized_value_return: None,
            },
            PerformanceAttribution::default(),
            PerformanceRisk {
                volatility: None,
                max_drawdown: None,
                peak_date: None,
                trough_date: None,
                recovery_date: None,
                drawdown_duration_days: None,
            },
            PerformanceDataQuality {
                status: DataQualityStatus::Partial,
                warnings: vec![warning],
                not_applicable_reasons: vec![
                    "Performance metrics unavailable because scoped valuation history is incomplete."
                        .to_string(),
                ],
            },
            Vec::new(),
            false,
            false,
        )
    }

    fn calculate_annualized_return(
        start_date: NaiveDate,
        end_date: NaiveDate,
        total_return: Decimal,
    ) -> Decimal {
        if start_date > end_date {
            return Decimal::ZERO;
        }

        // If total_return is -100% or less, base would be 0 or negative.
        // powd might handle base = 0, but negative base for non-integer exponent is problematic.
        // Directly returning -1.0 (i.e., -100% loss) is a sensible cap.
        if total_return <= dec!(-1.0) {
            return dec!(-1.0);
        }

        let days = (end_date - start_date).num_days();

        if days <= 0 {
            return total_return;
        }

        let years = Decimal::from(days) / DAYS_PER_YEAR_DECIMAL;

        let base = Decimal::ONE + total_return;

        // This check is theoretically covered by `total_return <= dec!(-1.0)`,
        // but as a safeguard if `total_return` was just slightly above -1.0,
        // leading to `base` being zero or negative due to precision.
        if base <= Decimal::ZERO {
            return dec!(-1.0);
        }

        let exponent = Decimal::ONE / years;

        base.powd(exponent) - Decimal::ONE
    }

    fn calculate_volatility(daily_returns: &[Decimal]) -> Option<Decimal> {
        if daily_returns.len() < 2 {
            return None;
        }

        let log_returns: Vec<Decimal> = daily_returns
            .iter()
            .filter_map(|daily_return| {
                let factor = Decimal::ONE + *daily_return;
                if factor <= Decimal::ZERO {
                    return None;
                }
                factor
                    .to_f64()
                    .and_then(|factor| Decimal::from_f64(factor.ln()))
            })
            .collect();

        if log_returns.len() < 2 {
            return None;
        }

        let count = Decimal::from(log_returns.len());
        let sum: Decimal = log_returns.iter().sum();
        let mean = sum / count;

        let sum_squared_diff: Decimal = log_returns
            .iter()
            .map(|&r| {
                let diff = r - mean;
                diff * diff
            })
            .sum();

        let variance = sum_squared_diff / (count - Decimal::ONE);
        if variance.is_sign_negative() {
            return None;
        }

        let daily_volatility = variance.sqrt().unwrap_or(Decimal::ZERO);

        let annualization_factor = Decimal::from(TRADING_DAYS_PER_YEAR)
            .sqrt()
            .unwrap_or(SQRT_TRADING_DAYS_APPROX);

        Some((daily_volatility * annualization_factor).round_dp(DECIMAL_PRECISION))
    }

    fn calculate_max_drawdown(
        samples: &[RiskSample],
        opening_date: Option<NaiveDate>,
    ) -> DrawdownComputation {
        if samples.is_empty() {
            return DrawdownComputation {
                max_drawdown: None,
                peak_date: None,
                trough_date: None,
                recovery_date: None,
                duration_days: None,
            };
        }

        let mut cumulative_value = Decimal::ONE;
        let mut peak_value = Decimal::ONE;
        let mut peak_date = opening_date.unwrap_or(samples[0].date);
        let mut max_drawdown = Decimal::ZERO;
        let mut max_peak_date = peak_date;
        let mut trough_date = samples[0].date;
        let mut recovery_date = None;
        let mut in_max_drawdown = false;

        for sample in samples {
            cumulative_value *= Decimal::ONE + sample.simple_return;
            if cumulative_value >= peak_value {
                peak_value = cumulative_value;
                peak_date = sample.date;
                if in_max_drawdown && recovery_date.is_none() {
                    recovery_date = Some(sample.date);
                }
            }

            if peak_value > Decimal::ZERO {
                let drawdown = (cumulative_value - peak_value) / peak_value;
                if drawdown < max_drawdown {
                    max_drawdown = drawdown;
                    max_peak_date = peak_date;
                    trough_date = sample.date;
                    recovery_date = None;
                    in_max_drawdown = true;
                }
            }
        }

        let duration_end = recovery_date.unwrap_or(trough_date);
        DrawdownComputation {
            max_drawdown: Some(max_drawdown.round_dp(DECIMAL_PRECISION)),
            peak_date: Some(max_peak_date),
            trough_date: Some(trough_date),
            recovery_date,
            duration_days: Some((duration_end - max_peak_date).num_days()),
        }
    }

    pub fn calculate_simple_performance(
        current: &DailyAccountValuation,
        _previous: Option<&DailyAccountValuation>,
        total_portfolio_value_base: Option<Decimal>,
    ) -> SimplePerformanceMetrics {
        // Use self for the current valuation data
        let total_gain_loss_amount = current.total_value - current.net_contribution;
        let denominator_cumulative_return = current.net_contribution;
        let cumulative_return_percent = if !denominator_cumulative_return.is_zero() {
            Some((total_gain_loss_amount / denominator_cumulative_return).round_dp(4))
        } else if total_gain_loss_amount.is_zero() {
            Some(Decimal::ZERO)
        } else {
            None
        };

        let total_value_base = current.total_value_base;
        let portfolio_weight = if let Some(total_portfolio) = total_portfolio_value_base {
            if !total_portfolio.is_zero() {
                Some(
                    (total_value_base / total_portfolio)
                        .max(Decimal::ZERO)
                        .min(Decimal::ONE)
                        .round_dp(4),
                )
            } else if total_value_base.is_zero() {
                Some(Decimal::ZERO)
            } else {
                None
            }
        } else {
            None
        };

        SimplePerformanceMetrics {
            account_id: current.account_id.clone(),
            total_value: Some(current.total_value),
            account_currency: Some(current.account_currency.clone()),
            base_currency: Some(current.base_currency.clone()),
            fx_rate_to_base: Some(current.fx_rate_to_base),
            total_gain_loss_amount: Some(total_gain_loss_amount.round_dp(2)),
            cumulative_return_percent,
            portfolio_weight,
        }
    }
}

#[async_trait::async_trait]
impl PerformanceServiceTrait for PerformanceService {
    /// Calculates cumulative returns for a given item (account or symbol)
    async fn calculate_performance_history(
        &self,
        item_type: &str,
        item_id: &str,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
        tracking_mode: Option<TrackingMode>,
    ) -> Result<PerformanceResult> {
        match item_type {
            "account" => {
                self.calculate_account_performance(item_id, start_date, end_date, tracking_mode)
                    .await
            }
            "symbol" => {
                self.calculate_symbol_performance(item_id, start_date, end_date)
                    .await
            }
            _ => Err(errors::Error::Validation(ValidationError::InvalidInput(
                "Invalid item type".to_string(),
            ))),
        }
    }

    async fn calculate_performance_history_for_accounts(
        &self,
        scope_id: &str,
        account_ids: &[String],
        base_currency: &str,
        account_tracking_modes: &HashMap<String, TrackingMode>,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<PerformanceResult> {
        self.calculate_scoped_performance(ScopedPerformanceRequest {
            scope_id,
            account_ids,
            base_currency,
            account_tracking_modes,
            start_date,
            end_date,
            include_returns_series: true,
        })
        .await
    }

    /// Calculates summary performance metrics only (no returns array, vol, maxDD)
    /// Currently only implemented for item_type = "account"
    async fn calculate_performance_summary(
        &self,
        item_type: &str,
        item_id: &str,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
        tracking_mode: Option<TrackingMode>,
    ) -> Result<PerformanceResult> {
        match item_type {
            "account" => {
                self.calculate_account_performance_summary(
                    item_id,
                    start_date,
                    end_date,
                    tracking_mode,
                )
                .await
            }
            "symbol" => {
                self.calculate_symbol_performance(item_id, start_date, end_date)
                    .await
            }
            _ => Err(errors::Error::Validation(ValidationError::InvalidInput(
                "Invalid item type".to_string(),
            ))),
        }
    }

    async fn calculate_performance_summary_for_accounts(
        &self,
        scope_id: &str,
        account_ids: &[String],
        base_currency: &str,
        account_tracking_modes: &HashMap<String, TrackingMode>,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<PerformanceResult> {
        self.calculate_scoped_performance(ScopedPerformanceRequest {
            scope_id,
            account_ids,
            base_currency,
            account_tracking_modes,
            start_date,
            end_date,
            include_returns_series: false,
        })
        .await
    }

    fn calculate_accounts_simple_performance(
        &self,
        account_ids: &[String],
    ) -> Result<Vec<SimplePerformanceMetrics>> {
        if account_ids.is_empty() {
            return Ok(Vec::new());
        }

        // 1. Fetch the *absolute* latest record for each account
        let latest_daily_valuations = self.valuation_service.get_latest_valuations(account_ids)?;

        let latest_daily_map: HashMap<String, DailyAccountValuation> = latest_daily_valuations
            .into_iter()
            .map(|d| (d.account_id.clone(), d))
            .collect();

        // 2. Determine the previous date for each account based on its absolute latest found date
        //    and group accounts by the previous date needed.
        let mut prev_dates_needed: HashMap<NaiveDate, Vec<String>> = HashMap::new();
        for account_id in account_ids {
            // Iterate over original requested IDs
            if let Some(latest_record) = latest_daily_map.get(account_id) {
                let prev_date = latest_record.valuation_date - Duration::days(1);
                prev_dates_needed
                    .entry(prev_date)
                    .or_default()
                    .push(account_id.clone());
            }
        }

        // 3. Fetch the previous day's records in bulk for all needed dates
        let mut previous_daily_map: HashMap<String, DailyAccountValuation> = HashMap::new();
        for (prev_date, ids) in prev_dates_needed {
            match self
                .valuation_service
                .get_valuations_on_date(&ids, prev_date)
            {
                Ok(records) => {
                    for record in records {
                        previous_daily_map.insert(record.account_id.clone(), record);
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to fetch valuation data for date {}: {}",
                        prev_date, e
                    );
                }
            }
        }

        // 4. Calculate total portfolio value by summing real-account base values.
        let total_portfolio_value_base: Decimal = account_ids
            .iter()
            .filter_map(|account_id| latest_daily_map.get(account_id))
            .map(|p| p.total_value_base)
            .sum();
        let total_portfolio_value_base = Some(total_portfolio_value_base);

        // 5. Construct results using the absolute latest and previous-to-latest records
        let mut results = Vec::with_capacity(account_ids.len());
        for account_id in account_ids {
            if let Some(current) = latest_daily_map.get(account_id) {
                let previous = previous_daily_map.get(account_id);
                let performance_metrics = Self::calculate_simple_performance(
                    current,
                    previous,
                    total_portfolio_value_base,
                );
                results.push(performance_metrics);
            } else {
                // This case might happen if history calculation failed or account is new
                debug!(
                    "No DailyAccountValuation found for account '{}' when fetching latest",
                    account_id
                );
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portfolio::valuation::ExternalFlowSource;
    use chrono::{DateTime, Utc};

    fn attribution_pnl(result: &PerformanceResult) -> Decimal {
        result.attribution.income
            + result.attribution.realized_pnl
            + result.attribution.unrealized_pnl_change
            + result.attribution.fx_effect
            - result.attribution.fees
            - result.attribution.taxes
            + result.attribution.residual
    }

    fn valuation(
        date: &str,
        total_value: Decimal,
        net_contribution: Decimal,
        investment_market_value: Decimal,
        cost_basis: Decimal,
    ) -> DailyAccountValuation {
        DailyAccountValuation {
            id: format!("acct-{}", date),
            account_id: "acct".to_string(),
            valuation_date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            account_currency: "CAD".to_string(),
            base_currency: "CAD".to_string(),
            fx_rate_to_base: Decimal::ONE,
            cash_balance: total_value - investment_market_value,
            investment_market_value,
            total_value,
            cost_basis,
            net_contribution,
            cash_balance_base: total_value - investment_market_value,
            investment_market_value_base: investment_market_value,
            total_value_base: total_value,
            cost_basis_base: cost_basis,
            net_contribution_base: net_contribution,
            external_inflow_base: Decimal::ZERO,
            external_outflow_base: Decimal::ZERO,
            external_flow_source: ValuationExternalFlowSource::Unknown,
            performance_eligible_value_base: total_value,
            calculated_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
        }
    }

    fn lot_disposal(
        currency: &str,
        base_currency: &str,
        fx_rate_to_base: &str,
        cost_basis: &str,
        cost_basis_base: &str,
        realized_pnl_base: &str,
    ) -> LotDisposal {
        LotDisposal {
            id: "disposal-1".to_string(),
            lot_id: "lot-1".to_string(),
            account_id: "acct".to_string(),
            asset_id: "asset".to_string(),
            disposal_activity_id: "sell-1".to_string(),
            disposal_date: "2026-05-02".to_string(),
            quantity: "1".to_string(),
            proceeds: "120".to_string(),
            cost_basis: cost_basis.to_string(),
            realized_pnl: "20".to_string(),
            proceeds_base: "132".to_string(),
            cost_basis_base: cost_basis_base.to_string(),
            realized_pnl_base: realized_pnl_base.to_string(),
            currency: currency.to_string(),
            base_currency: base_currency.to_string(),
            fx_rate_to_base: fx_rate_to_base.to_string(),
            cost_basis_method: "FIFO".to_string(),
            created_at: "2026-05-02T00:00:00.000Z".to_string(),
        }
    }

    /// Build the fixture used by the divergence / invariant tests: Feb 15 seed
    /// of 100 CAD, Mar 15 deposit of 2000 CAD + buy of 7 × 260, then a synthetic
    /// linear drift in holdings value to 1809.16 by Apr 14. Mirrors the shape
    /// of the user's Reproduce account that originally surfaced the bug.
    fn fixture_small_seed_then_large_deposit() -> Vec<DailyAccountValuation> {
        let mut history = Vec::new();

        // Feb 15 → Mar 14: $100 cash, no activity.
        let mut d = NaiveDate::parse_from_str("2026-02-15", "%Y-%m-%d").unwrap();
        let pre_deposit_end = NaiveDate::parse_from_str("2026-03-14", "%Y-%m-%d").unwrap();
        while d <= pre_deposit_end {
            history.push(valuation(
                &d.format("%Y-%m-%d").to_string(),
                dec!(100),
                dec!(100),
                Decimal::ZERO,
                Decimal::ZERO,
            ));
            d = d.succ_opt().unwrap();
        }

        // Mar 15: deposit 2000, buy 7 × 260 = 1820 same day. Net contribution
        // 2100, total_value 2100 (cash 280 + holdings at cost 1820).
        let mut deposit_day =
            valuation("2026-03-15", dec!(2100), dec!(2100), dec!(1820), dec!(1820));
        deposit_day.external_inflow_base = dec!(2000);
        history.push(deposit_day);

        // Mar 16 → Apr 13: holdings drift down by ~0.7/day (~$20 total over ~29 days).
        let mut d = NaiveDate::parse_from_str("2026-03-16", "%Y-%m-%d").unwrap();
        let drift_end = NaiveDate::parse_from_str("2026-04-13", "%Y-%m-%d").unwrap();
        let mut imv = dec!(1820);
        while d <= drift_end {
            imv -= dec!(0.7);
            history.push(valuation(
                &d.format("%Y-%m-%d").to_string(),
                dec!(280) + imv,
                dec!(2100),
                imv,
                dec!(1820),
            ));
            d = d.succ_opt().unwrap();
        }

        // Apr 14: final row — value matches the dashboard screenshot.
        history.push(valuation(
            "2026-04-14",
            dec!(2089.16),
            dec!(2100),
            dec!(1809.16),
            dec!(1820),
        ));

        history
    }

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn activity_fixture(activity_type: ActivityType, amount: Decimal, fee: Decimal) -> Activity {
        let now = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        Activity {
            id: format!("activity-{}", activity_type.as_str()),
            account_id: "acct".to_string(),
            asset_id: None,
            activity_type: activity_type.as_str().to_string(),
            activity_type_override: None,
            source_type: None,
            subtype: None,
            status: crate::activities::ActivityStatus::Posted,
            activity_date: now,
            settlement_date: None,
            quantity: None,
            unit_price: None,
            amount: Some(amount),
            fee: Some(fee),
            currency: "CAD".to_string(),
            fx_rate: None,
            notes: None,
            metadata: None,
            source_system: None,
            source_record_id: None,
            source_group_id: None,
            idempotency_key: None,
            import_run_id: None,
            is_user_modified: false,
            needs_review: false,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn activity_attribution_components_separate_income_fees_and_taxes() {
        let dividend = activity_fixture(ActivityType::Dividend, dec!(50), dec!(2));
        assert_eq!(
            PerformanceService::activity_attribution_components(&dividend, &ActivityType::Dividend),
            (dec!(50), dec!(2), Decimal::ZERO)
        );

        let explicit_fee = activity_fixture(ActivityType::Fee, dec!(4), Decimal::ZERO);
        assert_eq!(
            PerformanceService::activity_attribution_components(&explicit_fee, &ActivityType::Fee),
            (Decimal::ZERO, dec!(4), Decimal::ZERO)
        );

        let tax = activity_fixture(ActivityType::Tax, dec!(7), Decimal::ZERO);
        assert_eq!(
            PerformanceService::activity_attribution_components(&tax, &ActivityType::Tax),
            (Decimal::ZERO, Decimal::ZERO, dec!(7))
        );

        let buy = activity_fixture(ActivityType::Buy, dec!(100), dec!(1));
        assert_eq!(
            PerformanceService::activity_attribution_components(&buy, &ActivityType::Buy),
            (Decimal::ZERO, dec!(1), Decimal::ZERO)
        );
    }

    /// Regression test for the reporter's bug. Pre-fix, the headline return was
    /// `gain / start_value` = -10.84/100 = -10.84%. Post-fix, it's daily-linked
    /// TWR — should end up near zero, dominated by the synthetic ~1.1% AAPL
    /// drift between Mar 15 and Apr 14.
    #[test]
    fn perf_does_not_explode_when_start_value_tiny_vs_cash_flow() {
        let history = fixture_small_seed_then_large_deposit();

        let result = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            Some(date("2026-01-01")),
            false, // summary path — matches the dashboard
        )
        .expect("summary should compute");

        let twr = result.returns.twr.expect("TWR should be Some");

        // Old formula: -0.1084. New: small (market-drift-dominated). Bounds are
        // wide — the fixture uses synthetic linear drift and exact precision
        // isn't what we're testing; we're testing that the percentage is sane.
        assert!(
            twr > dec!(-0.05),
            "TWR = {} should be > -5% (was -10.84% with the old formula)",
            twr
        );
        assert!(
            twr < dec!(0.01),
            "TWR = {} should be < 1% (asset drifted down slightly)",
            twr
        );

        // $ gain is unchanged — end - start - cash_flow = 2089.16 - 100 - 2000.
        assert_eq!(attribution_pnl(&result), dec!(-10.84));
        assert_eq!(
            result.returns.value_return.unwrap().round_dp(4),
            dec!(-0.1084)
        );
        assert!(!result
            .data_quality
            .not_applicable_reasons
            .iter()
            .any(|reason| reason.contains("transaction-mode")));
        assert!(result.returns.twr.is_some());
    }

    /// Invariant: summary and full paths must agree on headline returns. This is
    /// the core guarantee the refactor is meant to enforce — the dashboard card
    /// and account-detail page showing different percentages for the same
    /// account / range was the original user complaint.
    #[test]
    fn perf_full_and_summary_paths_agree_on_headline_return() {
        let history = fixture_small_seed_then_large_deposit();
        let start = Some(date("2026-01-01"));

        let full = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            start,
            true,
        )
        .expect("full should compute");

        let summary = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            start,
            false,
        )
        .expect("summary should compute");

        // Headline percentage must match exactly — that's the user-visible
        // invariant. Everything else (returns series, risk metrics) is summary
        // vs full differentiation.
        assert_eq!(full.returns.irr, summary.returns.irr);
        assert_eq!(full.mode, ReturnMethod::TimeWeighted);
        assert_eq!(summary.mode, ReturnMethod::TimeWeighted);
        assert_eq!(full.returns.twr, summary.returns.twr);
        assert_eq!(attribution_pnl(&full), attribution_pnl(&summary));
        assert_eq!(full.returns.value_return, summary.returns.value_return);

        // Differentiation: full path populates returns[] and risk metrics;
        // summary stays empty/zero to save allocation on the dashboard.
        assert!(!full.series.is_empty());
        assert!(summary.series.is_empty());
        assert!(full.risk.volatility.unwrap() > Decimal::ZERO);
        assert!(summary.risk.volatility.is_some());
    }

    /// Well-formed account (`start_value == net_contribution`) stays sane —
    /// the common case shouldn't regress.
    #[test]
    fn perf_well_formed_account_remains_sane() {
        let history = vec![
            valuation(
                "2026-02-15",
                dec!(1000),
                dec!(1000),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation(
                "2026-02-16",
                dec!(1000),
                dec!(1000),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation("2026-04-14", dec!(999.48), dec!(1000), dec!(259), dec!(260)),
        ];

        let result = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            Some(date("2026-01-01")),
            false,
        )
        .expect("summary should compute");

        let twr = result.returns.twr.expect("TWR should be Some");
        assert!(
            twr.abs() < dec!(0.01),
            "TWR = {} should be small for well-formed account",
            twr
        );
        assert_eq!(attribution_pnl(&result).round_dp(2), dec!(-0.52));
    }

    #[test]
    fn twr_uses_start_of_day_inflow_convention() {
        let history = vec![
            valuation(
                "2026-05-01",
                dec!(100),
                dec!(100),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation(
                "2026-05-02",
                dec!(210),
                dec!(200),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
        ];

        let result = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            None,
            true,
        )
        .expect("performance should compute");

        assert_eq!(result.returns.twr.unwrap().round_dp(4), dec!(0.05));
    }

    #[test]
    fn twr_uses_end_of_day_outflow_convention() {
        let history = vec![
            valuation(
                "2026-05-01",
                dec!(200),
                dec!(200),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation(
                "2026-05-02",
                dec!(110),
                dec!(100),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
        ];

        let result = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            None,
            true,
        )
        .expect("performance should compute");

        assert_eq!(result.returns.twr.unwrap().round_dp(4), dec!(0.05));
    }

    #[test]
    fn twr_and_irr_handle_zero_start_then_early_deposit() {
        let mut history = vec![
            valuation(
                "2026-05-01",
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation(
                "2026-05-02",
                dec!(100),
                dec!(100),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation(
                "2027-05-02",
                dec!(110),
                dec!(100),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
        ];
        history[1].external_inflow_base = dec!(100);

        let result = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            None,
            true,
        )
        .expect("performance should compute");

        assert_eq!(result.returns.twr.unwrap().round_dp(4), dec!(0.1));
        assert!(result.returns.irr.is_some());
        assert!(result.returns.value_return.is_none());
        assert!(result
            .data_quality
            .not_applicable_reasons
            .iter()
            .any(|reason| reason.contains("starting value is zero or negative")));
        assert_eq!(result.series.last().unwrap().value.round_dp(4), dec!(0.1));
    }

    #[test]
    fn twr_and_irr_keep_same_day_deposit_and_withdrawal_gross() {
        let mut history = vec![
            valuation(
                "2026-05-01",
                dec!(100),
                dec!(100),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation(
                "2027-05-01",
                dec!(168),
                dec!(160),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
        ];
        history[1].external_inflow_base = dec!(100);
        history[1].external_outflow_base = dec!(40);

        let result = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            None,
            true,
        )
        .expect("performance should compute");

        let daily_flows = PerformanceService::daily_external_flow_series(
            &history,
            ExternalFlowBasis::BaseCurrency,
        );
        let (contributions, distributions) = PerformanceService::total_external_flows(&daily_flows);
        assert_eq!(contributions, dec!(100));
        assert_eq!(distributions, dec!(40));
        assert_eq!(result.returns.twr.unwrap().round_dp(4), dec!(0.04));
        assert!(result.returns.irr.is_some());
        assert!(!result
            .data_quality
            .warnings
            .iter()
            .any(|warning| warning.contains("inferred from net contribution")));
    }

    #[test]
    fn net_flow_fallback_is_shared_by_twr_irr_value_return_and_attribution() {
        let history = vec![
            valuation("2026-05-01", dec!(100), dec!(100), dec!(100), dec!(100)),
            valuation("2027-05-01", dec!(160), dec!(150), dec!(160), dec!(150)),
        ];

        let result = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            None,
            true,
        )
        .expect("performance should compute");

        assert_eq!(result.attribution.contributions, dec!(50));
        assert_eq!(result.attribution.distributions, Decimal::ZERO);
        assert_eq!(result.attribution.unrealized_pnl_change, dec!(10));
        assert_eq!(result.attribution.residual, Decimal::ZERO);
        assert_eq!(result.returns.twr.unwrap().round_dp(4), dec!(0.0667));
        assert_eq!(result.returns.value_return.unwrap().round_dp(4), dec!(0.1));
        assert!(result.returns.irr.is_some());
        assert!(result
            .data_quality
            .warnings
            .iter()
            .any(|warning| warning.contains("inferred from net contribution")));
    }

    #[test]
    fn zero_net_fallback_flow_warns_across_account_return_paths() {
        let mut history = vec![
            valuation("2026-05-01", dec!(100), dec!(100), dec!(100), dec!(100)),
            valuation("2027-05-01", dec!(110), dec!(100), dec!(110), dec!(100)),
        ];
        history[1].external_flow_source = ExternalFlowSource::NetContributionFallback;

        let result = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            None,
            true,
        )
        .expect("performance should compute");

        assert_eq!(result.attribution.contributions, Decimal::ZERO);
        assert_eq!(result.attribution.distributions, Decimal::ZERO);
        assert_eq!(result.attribution.unrealized_pnl_change, dec!(10));
        assert_eq!(result.attribution.residual, Decimal::ZERO);
        assert_eq!(result.returns.twr.unwrap().round_dp(4), dec!(0.1));
        assert_eq!(result.returns.value_return.unwrap().round_dp(4), dec!(0.1));
        assert!(result.returns.irr.is_some());
        assert!(result
            .data_quality
            .warnings
            .iter()
            .any(|warning| warning.contains("inferred from net contribution")));
    }

    #[test]
    fn explicit_gross_same_day_flows_feed_all_account_return_paths() {
        let mut history = vec![
            valuation("2026-05-01", dec!(100), dec!(100), dec!(100), dec!(100)),
            valuation("2027-05-01", dec!(120), dec!(100), dec!(120), dec!(100)),
        ];
        history[1].external_inflow_base = dec!(100);
        history[1].external_outflow_base = dec!(100);
        history[1].external_flow_source = ExternalFlowSource::ActivityDerived;

        let result = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            None,
            true,
        )
        .expect("performance should compute");

        assert_eq!(result.attribution.contributions, dec!(100));
        assert_eq!(result.attribution.distributions, dec!(100));
        assert_eq!(result.attribution.unrealized_pnl_change, dec!(20));
        assert_eq!(result.attribution.residual, Decimal::ZERO);
        assert_eq!(result.returns.twr.unwrap().round_dp(4), dec!(0.1));
        assert_eq!(result.returns.value_return.unwrap().round_dp(4), dec!(0.2));
        assert!(result.returns.irr.is_some());
        assert!(!result
            .data_quality
            .warnings
            .iter()
            .any(|warning| warning.contains("inferred from net contribution")));
    }

    #[test]
    fn realized_pnl_attribution_warns_when_acquisition_fx_is_missing() {
        let disposal = lot_disposal("USD", "CAD", "1.1", "100", "0", "0");

        let warning = PerformanceService::realized_pnl_base_from_disposal(&disposal)
            .expect_err("missing acquisition FX should make base realized P&L unusable");

        assert!(warning.contains("acquisition FX conversion was unavailable"));
    }

    #[test]
    fn realized_pnl_attribution_accepts_complete_base_disposal() {
        let disposal = lot_disposal("USD", "CAD", "1.1", "100", "110", "22");

        let realized_pnl_base = PerformanceService::realized_pnl_base_from_disposal(&disposal)
            .expect("complete FX facts should be usable");

        assert_eq!(realized_pnl_base, dec!(22));
    }

    #[test]
    fn irr_returns_none_when_cash_flows_have_no_sign_change() {
        let mut history = vec![
            valuation(
                "2026-05-01",
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation(
                "2026-05-02",
                dec!(10),
                dec!(-10),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
        ];
        history[1].external_outflow_base = dec!(10);

        let daily_flows = PerformanceService::daily_external_flow_series(
            &history,
            ExternalFlowBasis::BaseCurrency,
        );
        let irr = PerformanceService::calculate_xirr(
            &history,
            &daily_flows,
            ExternalFlowBasis::BaseCurrency,
        );

        assert!(irr.irr.is_none());
        assert!(irr
            .warnings
            .iter()
            .any(|warning| warning.contains("do not change sign")));
    }

    #[test]
    fn irr_returns_none_when_solver_does_not_converge() {
        let history = vec![
            valuation("2026-05-01", dec!(1), dec!(1), Decimal::ZERO, Decimal::ZERO),
            valuation(
                "2026-05-02",
                dec!(1000000000000),
                dec!(1),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
        ];

        let daily_flows = PerformanceService::daily_external_flow_series(
            &history,
            ExternalFlowBasis::BaseCurrency,
        );
        let irr = PerformanceService::calculate_xirr(
            &history,
            &daily_flows,
            ExternalFlowBasis::BaseCurrency,
        );

        assert!(irr.irr.is_none());
        assert!(irr
            .warnings
            .iter()
            .any(|warning| warning.contains("did not converge")));
    }

    #[test]
    fn return_period_with_non_positive_denominator_is_excluded() {
        let history = vec![
            valuation(
                "2026-05-01",
                dec!(0.5),
                dec!(0.5),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation(
                "2026-05-02",
                dec!(0.5),
                dec!(0.5),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
        ];

        let result = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            None,
            true,
        )
        .expect("performance should compute");

        assert!(result.returns.twr.is_none());
        assert_eq!(result.series.last().unwrap().value, Decimal::ZERO);
        assert!(!result.data_quality.not_applicable_reasons.is_empty());
    }

    #[test]
    fn twr_tiny_denominator_before_chain_makes_result_not_applicable() {
        let mut history = vec![
            valuation(
                "2026-05-01",
                dec!(0.5),
                dec!(0.5),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation(
                "2026-05-02",
                dec!(0.5),
                dec!(0.5),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation("2026-05-03", dec!(2), dec!(2), Decimal::ZERO, Decimal::ZERO),
        ];
        history[2].external_inflow_base = dec!(1.5);

        let result = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            None,
            true,
        )
        .expect("performance should compute");

        assert!(result.returns.twr.is_none());
        assert!(result
            .data_quality
            .not_applicable_reasons
            .iter()
            .any(|reason| reason.contains("below 1 base currency unit")));
    }

    #[test]
    fn attribution_separates_account_currency_fx_from_unrealized_pnl() {
        let mut start = valuation("2026-05-01", dec!(100), dec!(100), dec!(100), dec!(100));
        start.account_currency = "USD".to_string();
        start.base_currency = "CAD".to_string();
        start.fx_rate_to_base = dec!(1.3);
        start.total_value_base = dec!(130);
        start.investment_market_value_base = dec!(130);
        start.cost_basis_base = dec!(130);
        start.net_contribution_base = dec!(130);

        let mut end = valuation("2026-05-02", dec!(100), dec!(100), dec!(100), dec!(100));
        end.account_currency = "USD".to_string();
        end.base_currency = "CAD".to_string();
        end.fx_rate_to_base = dec!(1.4);
        end.total_value_base = dec!(140);
        end.investment_market_value_base = dec!(140);
        end.cost_basis_base = dec!(130);
        end.net_contribution_base = dec!(130);

        let result = PerformanceService::compute_account_performance(
            &[start, end],
            Some(TrackingMode::Transactions),
            None,
            false,
        )
        .expect("performance should compute");

        assert_eq!(result.attribution.unrealized_pnl_change, Decimal::ZERO);
        assert_eq!(result.attribution.fx_effect, dec!(10));
        assert_eq!(result.attribution.residual, Decimal::ZERO);
    }

    #[test]
    fn scoped_attribution_separates_fx_per_account_before_aggregation() {
        let mut usd_start = valuation("2026-05-01", dec!(100), dec!(100), dec!(100), dec!(100));
        usd_start.account_id = "usd".to_string();
        usd_start.account_currency = "USD".to_string();
        usd_start.base_currency = "CAD".to_string();
        usd_start.fx_rate_to_base = dec!(1.3);
        usd_start.total_value_base = dec!(130);
        usd_start.investment_market_value_base = dec!(130);
        usd_start.cost_basis_base = dec!(130);
        usd_start.net_contribution_base = dec!(130);

        let mut usd_end = valuation("2026-05-02", dec!(110), dec!(100), dec!(110), dec!(100));
        usd_end.account_id = "usd".to_string();
        usd_end.account_currency = "USD".to_string();
        usd_end.base_currency = "CAD".to_string();
        usd_end.fx_rate_to_base = dec!(1.4);
        usd_end.total_value_base = dec!(154);
        usd_end.investment_market_value_base = dec!(154);
        usd_end.cost_basis_base = dec!(130);
        usd_end.net_contribution_base = dec!(130);

        let mut cad_start = valuation("2026-05-01", dec!(100), dec!(100), dec!(100), dec!(100));
        cad_start.account_id = "cad".to_string();
        let mut cad_end = valuation("2026-05-02", dec!(120), dec!(100), dec!(120), dec!(100));
        cad_end.account_id = "cad".to_string();

        let attribution = PerformanceService::scoped_unrealized_attribution_components(
            &[vec![usd_start, usd_end], vec![cad_start, cad_end]],
            date("2026-05-01"),
            date("2026-05-02"),
        );

        assert!(attribution.complete);
        assert_eq!(attribution.unrealized_pnl_change, dec!(34));
        assert_eq!(attribution.fx_effect, dec!(10));
    }

    /// Negative portfolio value (like TEST's unfunded-BUY shape) surfaces as a
    /// validation error in both paths — downstream percentages are meaningless
    /// when the underlying data is broken.
    #[test]
    fn perf_rejects_negative_portfolio_value() {
        let history = vec![
            valuation(
                "2026-04-01",
                dec!(100),
                dec!(100),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation("2026-04-02", dec!(-50), dec!(100), dec!(-50), Decimal::ZERO),
        ];

        for include_series in [true, false] {
            let err = PerformanceService::compute_account_performance(
                &history,
                Some(TrackingMode::Transactions),
                None,
                include_series,
            )
            .expect_err("should error on negative portfolio value");

            assert!(
                format!("{}", err).contains("negative portfolio value"),
                "expected 'negative portfolio value' in error (include_series={}), got: {}",
                include_series,
                err
            );
        }
    }

    #[test]
    fn scoped_perf_uses_explicit_base_flows_for_foreign_currency_accounts() {
        let mut prev = valuation("2026-04-01", dec!(100), dec!(100), dec!(100), dec!(100));
        let mut curr = valuation("2026-04-02", dec!(210), dec!(200), dec!(210), dec!(200));
        prev.account_currency = "EUR".to_string();
        prev.base_currency = "USD".to_string();
        prev.fx_rate_to_base = dec!(1.1);
        prev.total_value_base = dec!(110);
        prev.net_contribution_base = dec!(110);
        curr.account_currency = "EUR".to_string();
        curr.base_currency = "USD".to_string();
        curr.fx_rate_to_base = dec!(1.1);
        curr.total_value_base = dec!(231);
        curr.net_contribution_base = dec!(220);
        curr.external_inflow_base = dec!(110);

        let flow =
            PerformanceService::daily_external_flows(&prev, &curr, ExternalFlowBasis::BaseCurrency);

        assert_eq!(flow.inflow, dec!(110));
        assert_eq!(flow.outflow, Decimal::ZERO);
        assert_eq!(flow.source, ExternalFlowSource::StoredGross);
    }

    #[test]
    fn account_perf_uses_account_currency_flows_for_foreign_currency_accounts() {
        let mut prev = valuation("2026-04-01", dec!(100), dec!(100), dec!(100), dec!(100));
        let mut curr = valuation("2026-04-02", dec!(210), dec!(200), dec!(210), dec!(200));
        prev.account_currency = "EUR".to_string();
        prev.base_currency = "USD".to_string();
        prev.fx_rate_to_base = dec!(1.1);
        prev.total_value_base = dec!(110);
        prev.net_contribution_base = dec!(110);
        curr.account_currency = "EUR".to_string();
        curr.base_currency = "USD".to_string();
        curr.fx_rate_to_base = dec!(1.1);
        curr.total_value_base = dec!(231);
        curr.net_contribution_base = dec!(220);
        curr.external_inflow_base = dec!(110);

        let result = PerformanceService::compute_account_performance(
            &[prev, curr],
            Some(TrackingMode::Transactions),
            None,
            false,
        )
        .expect("foreign-currency account performance should compute");

        assert_eq!(result.scope.currency, "USD");
        assert_eq!(attribution_pnl(&result), dec!(11));
        assert_eq!(result.returns.twr.unwrap().round_dp(4), dec!(0.05));
    }

    /// HOLDINGS mode uses the cost-basis formula in both paths. TWR/IRR are
    /// returned as `None` because they aren't meaningful without per-transaction
    /// cash-flow tracking.
    #[test]
    fn perf_holdings_mode_uses_cost_basis_formula() {
        let history = vec![
            valuation("2026-02-15", dec!(1000), dec!(1000), dec!(1000), dec!(1000)),
            valuation("2026-04-14", dec!(900), dec!(1000), dec!(900), dec!(1000)),
        ];

        let result = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Holdings),
            None, // ALL-time branch
            false,
        )
        .expect("holdings should compute");

        // end_unrealized_pnl = 900 - 1000 = -100; return = -100 / 1000 = -0.10.
        assert_eq!(result.returns.value_return.unwrap().round_dp(4), dec!(-0.1));
        assert!(result.returns.twr.is_none());
        assert!(result.returns.irr.is_none());
        assert!(result.is_holdings_mode);
    }

    #[test]
    fn perf_holdings_mode_omits_value_return_when_denominator_is_undefined() {
        let history = vec![
            valuation(
                "2026-02-15",
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation(
                "2026-04-14",
                dec!(50),
                Decimal::ZERO,
                dec!(50),
                Decimal::ZERO,
            ),
        ];

        let all_time = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Holdings),
            None,
            false,
        )
        .expect("holdings should compute");

        assert!(all_time.returns.value_return.is_none());
        assert!(all_time
            .data_quality
            .not_applicable_reasons
            .iter()
            .any(|reason| reason.contains("ending cost basis")));

        let period = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Holdings),
            Some(date("2026-02-01")),
            false,
        )
        .expect("holdings should compute");

        assert!(period.returns.value_return.is_none());
        assert!(period
            .data_quality
            .not_applicable_reasons
            .iter()
            .any(|reason| reason.contains("starting market value")));
    }

    #[test]
    fn scoped_performance_uses_mixed_mode_without_dropping_holdings_accounts() {
        let account_ids = vec!["tx".to_string(), "holdings".to_string()];
        let mut modes = HashMap::new();
        modes.insert("tx".to_string(), TrackingMode::Transactions);
        modes.insert("holdings".to_string(), TrackingMode::Holdings);

        let composition = PerformanceService::scoped_tracking_composition(&account_ids, &modes);

        assert_eq!(composition, ScopedTrackingComposition::Mixed);
    }

    #[test]
    fn scoped_performance_uses_holdings_mode_when_all_accounts_are_holdings_mode() {
        let account_ids = vec!["holdings-a".to_string(), "holdings-b".to_string()];
        let mut modes = HashMap::new();
        modes.insert("holdings-a".to_string(), TrackingMode::Holdings);
        modes.insert("holdings-b".to_string(), TrackingMode::Holdings);

        let composition = PerformanceService::scoped_tracking_composition(&account_ids, &modes);

        assert_eq!(composition, ScopedTrackingComposition::HoldingsOnly);
    }

    #[test]
    fn mixed_scope_performance_is_value_complete_simple_return() {
        let mut history = vec![
            valuation("2026-05-01", dec!(1500), dec!(1500), dec!(1500), dec!(1500)),
            valuation("2026-05-02", dec!(1660), dec!(1600), dec!(1660), dec!(1600)),
        ];
        history[1].external_inflow_base = dec!(100);

        let result = PerformanceService::compute_mixed_scope_performance(&history, true)
            .expect("mixed scope should compute");

        assert!(result.is_mixed_tracking_mode);
        assert_eq!(result.mode, ReturnMethod::ValueReturn);
        assert_eq!(attribution_pnl(&result), dec!(60));
        assert_eq!(result.returns.value_return.unwrap().round_dp(4), dec!(0.04));
        assert_eq!(result.series.last().unwrap().value.round_dp(4), dec!(0.04));
        assert!(result.returns.twr.is_none());
        assert!(result.returns.irr.is_none());
        assert!(!result.data_quality.warnings.is_empty());
    }

    #[test]
    fn mixed_scope_performance_uses_base_currency_values() {
        let mut history = vec![
            valuation("2026-05-01", dec!(50), dec!(50), dec!(50), dec!(50)),
            valuation("2027-05-01", dec!(80), dec!(60), dec!(80), dec!(60)),
        ];
        history[0].base_currency = "CAD".to_string();
        history[0].total_value_base = dec!(100);
        history[0].investment_market_value_base = dec!(100);
        history[0].cost_basis_base = dec!(100);
        history[0].net_contribution_base = dec!(100);
        history[1].base_currency = "CAD".to_string();
        history[1].total_value_base = dec!(130);
        history[1].investment_market_value_base = dec!(130);
        history[1].cost_basis_base = dec!(110);
        history[1].net_contribution_base = dec!(110);
        history[1].external_inflow_base = dec!(10);

        let result = PerformanceService::compute_mixed_scope_performance(&history, true)
            .expect("mixed scope should compute from base values");

        assert_eq!(result.scope.currency, "CAD");
        assert_eq!(result.returns.value_return.unwrap().round_dp(4), dec!(0.2));
        assert_eq!(result.series.last().unwrap().value.round_dp(4), dec!(0.2));
        assert_eq!(result.attribution.contributions, dec!(10));
    }

    #[test]
    fn mixed_scope_rejects_negative_portfolio_value_without_series() {
        let history = vec![
            valuation("2026-05-01", dec!(100), dec!(100), dec!(100), dec!(100)),
            valuation("2026-05-02", dec!(-50), dec!(100), dec!(-50), dec!(100)),
        ];

        for include_series in [true, false] {
            let err = PerformanceService::compute_mixed_scope_performance(&history, include_series)
                .expect_err("mixed scope should reject negative portfolio value");

            assert!(
                format!("{}", err).contains("negative portfolio value"),
                "expected 'negative portfolio value' in error (include_series={}), got: {}",
                include_series,
                err
            );
        }
    }

    #[test]
    fn mixed_scope_with_zero_start_value_returns_not_applicable_value_return() {
        let mut history = vec![
            valuation(
                "2026-05-01",
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            valuation("2026-05-02", dec!(100), dec!(100), dec!(100), dec!(100)),
        ];
        history[1].external_inflow_base = dec!(100);

        let result = PerformanceService::compute_mixed_scope_performance(&history, true)
            .expect("mixed scope should compute P&L without a percentage denominator");

        assert!(result.returns.value_return.is_none());
        assert!(result.returns.annualized_value_return.is_none());
        assert!(result.series.is_empty());
        assert!(result
            .data_quality
            .not_applicable_reasons
            .iter()
            .any(|reason| reason.contains("starting value is zero or negative")));
    }

    #[test]
    fn drawdown_reports_unrecovered_decline_with_dates() {
        let samples = vec![
            RiskSample {
                date: date("2026-05-01"),
                simple_return: dec!(0.1),
            },
            RiskSample {
                date: date("2026-05-02"),
                simple_return: dec!(-0.2),
            },
            RiskSample {
                date: date("2026-05-03"),
                simple_return: dec!(-0.1),
            },
        ];

        let drawdown =
            PerformanceService::calculate_max_drawdown(&samples, Some(date("2026-04-30")));

        assert_eq!(drawdown.max_drawdown.unwrap().round_dp(4), dec!(-0.28));
        assert_eq!(drawdown.peak_date, Some(date("2026-05-01")));
        assert_eq!(drawdown.trough_date, Some(date("2026-05-03")));
        assert_eq!(drawdown.recovery_date, None);
        assert_eq!(drawdown.duration_days, Some(2));
    }

    #[test]
    fn drawdown_reports_recovery_date() {
        let samples = vec![
            RiskSample {
                date: date("2026-05-01"),
                simple_return: dec!(0.1),
            },
            RiskSample {
                date: date("2026-05-02"),
                simple_return: dec!(-0.1),
            },
            RiskSample {
                date: date("2026-05-03"),
                simple_return: dec!(0.12),
            },
        ];

        let drawdown =
            PerformanceService::calculate_max_drawdown(&samples, Some(date("2026-05-01")));

        assert_eq!(drawdown.max_drawdown.unwrap().round_dp(4), dec!(-0.1));
        assert_eq!(drawdown.peak_date, Some(date("2026-05-01")));
        assert_eq!(drawdown.trough_date, Some(date("2026-05-02")));
        assert_eq!(drawdown.recovery_date, Some(date("2026-05-03")));
        assert_eq!(drawdown.duration_days, Some(2));
    }

    #[test]
    fn drawdown_uses_opening_date_when_first_sample_declines() {
        let samples = vec![
            RiskSample {
                date: date("2026-05-02"),
                simple_return: dec!(-0.1),
            },
            RiskSample {
                date: date("2026-05-03"),
                simple_return: dec!(0.05),
            },
        ];

        let drawdown =
            PerformanceService::calculate_max_drawdown(&samples, Some(date("2026-05-01")));

        assert_eq!(drawdown.max_drawdown.unwrap().round_dp(4), dec!(-0.1));
        assert_eq!(drawdown.peak_date, Some(date("2026-05-01")));
        assert_eq!(drawdown.trough_date, Some(date("2026-05-02")));
        assert_eq!(drawdown.duration_days, Some(1));
    }

    #[test]
    fn risk_handles_flat_and_missing_series() {
        let empty_risk = PerformanceService::risk_from_samples(&[], None);
        assert!(empty_risk.volatility.is_none());
        assert!(empty_risk.max_drawdown.is_none());

        let flat_samples = vec![
            RiskSample {
                date: date("2026-05-01"),
                simple_return: Decimal::ZERO,
            },
            RiskSample {
                date: date("2026-05-02"),
                simple_return: Decimal::ZERO,
            },
        ];
        let flat_risk =
            PerformanceService::risk_from_samples(&flat_samples, Some(date("2026-05-01")));
        assert_eq!(flat_risk.volatility, Some(Decimal::ZERO));
        assert_eq!(flat_risk.max_drawdown, Some(Decimal::ZERO));
    }

    #[test]
    fn account_performance_keeps_zero_return_days_for_risk() {
        let history = vec![
            valuation("2026-05-01", dec!(100), dec!(100), dec!(100), dec!(100)),
            valuation("2026-05-02", dec!(100), dec!(100), dec!(100), dec!(100)),
            valuation("2026-05-03", dec!(100), dec!(100), dec!(100), dec!(100)),
        ];

        let result = PerformanceService::compute_account_performance(
            &history,
            Some(TrackingMode::Transactions),
            None,
            true,
        )
        .expect("flat performance should compute");

        assert_eq!(result.risk.volatility, Some(Decimal::ZERO));
        assert_eq!(result.risk.max_drawdown, Some(Decimal::ZERO));
    }
}
