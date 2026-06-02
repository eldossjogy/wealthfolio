use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CumulativeReturn {
    pub date: NaiveDate,
    pub value: Decimal,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TotalReturn {
    pub rate: Decimal,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum ReturnMethod {
    #[default]
    TimeWeighted,
    ValueReturn,
    SymbolPriceBased,
    NotApplicable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReturnData {
    pub date: NaiveDate,
    pub value: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceScopeDescriptor {
    pub id: String,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerformancePeriod {
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceReturns {
    pub twr: Option<Decimal>,
    pub annualized_twr: Option<Decimal>,
    /// Selected-period money-weighted return derived from annualized XIRR.
    pub irr: Option<Decimal>,
    /// Annualized XIRR using dated cash flows.
    pub annualized_irr: Option<Decimal>,
    pub value_return: Option<Decimal>,
    pub annualized_value_return: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PerformanceSummaryProfile {
    #[default]
    Full,
    Headline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceAttribution {
    pub contributions: Decimal,
    pub distributions: Decimal,
    pub income: Decimal,
    pub realized_pnl: Decimal,
    pub unrealized_pnl_change: Decimal,
    pub fx_effect: Decimal,
    pub fees: Decimal,
    pub taxes: Decimal,
    pub residual: Decimal,
}

impl Default for PerformanceAttribution {
    fn default() -> Self {
        Self {
            contributions: Decimal::ZERO,
            distributions: Decimal::ZERO,
            income: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            unrealized_pnl_change: Decimal::ZERO,
            fx_effect: Decimal::ZERO,
            fees: Decimal::ZERO,
            taxes: Decimal::ZERO,
            residual: Decimal::ZERO,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceRisk {
    pub volatility: Option<Decimal>,
    pub max_drawdown: Option<Decimal>,
    pub peak_date: Option<NaiveDate>,
    pub trough_date: Option<NaiveDate>,
    pub recovery_date: Option<NaiveDate>,
    pub drawdown_duration_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DataQualityStatus {
    Ok,
    Partial,
    NoData,
    NotApplicable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceDataQuality {
    pub status: DataQualityStatus,
    pub warnings: Vec<String>,
    pub not_applicable_reasons: Vec<String>,
}

impl PerformanceDataQuality {
    pub fn ok() -> Self {
        Self {
            status: DataQualityStatus::Ok,
            warnings: Vec::new(),
            not_applicable_reasons: Vec::new(),
        }
    }

    pub fn no_data(reason: impl Into<String>) -> Self {
        Self {
            status: DataQualityStatus::NoData,
            warnings: Vec::new(),
            not_applicable_reasons: vec![reason.into()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceResult {
    pub scope: PerformanceScopeDescriptor,
    pub period: PerformancePeriod,
    pub mode: ReturnMethod,
    pub returns: PerformanceReturns,
    pub attribution: PerformanceAttribution,
    pub risk: PerformanceRisk,
    pub data_quality: PerformanceDataQuality,
    pub series: Vec<ReturnData>,
    #[serde(default)]
    pub is_holdings_mode: bool,
    #[serde(default)]
    pub is_mixed_tracking_mode: bool,
}

// This struct now only holds the calculated performance metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SimplePerformanceMetrics {
    pub account_id: String,
    pub account_currency: Option<String>,
    pub base_currency: Option<String>,
    pub fx_rate_to_base: Option<Decimal>,
    pub total_value: Option<Decimal>,
    pub total_gain_loss_amount: Option<Decimal>,
    pub cumulative_return_percent: Option<Decimal>,
    pub portfolio_weight: Option<Decimal>,
}
