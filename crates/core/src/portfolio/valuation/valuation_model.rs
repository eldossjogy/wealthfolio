//! Portfolio valuation domain models.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExternalFlowSource {
    #[default]
    Unknown,
    ActivityDerived,
    StoredGross,
    NetContributionFallback,
    Mixed,
}

impl ExternalFlowSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::ActivityDerived => "ACTIVITY_DERIVED",
            Self::StoredGross => "STORED_GROSS",
            Self::NetContributionFallback => "NET_CONTRIBUTION_FALLBACK",
            Self::Mixed => "MIXED",
        }
    }

    pub fn from_code(value: &str) -> Self {
        match value.trim().to_ascii_uppercase().as_str() {
            "ACTIVITY_DERIVED" => Self::ActivityDerived,
            "STORED_GROSS" => Self::StoredGross,
            "NET_CONTRIBUTION_FALLBACK" => Self::NetContributionFallback,
            "MIXED" => Self::Mixed,
            _ => Self::Unknown,
        }
    }

    pub fn is_explicit_gross(self) -> bool {
        matches!(
            self,
            Self::ActivityDerived | Self::StoredGross | Self::Mixed
        )
    }

    pub fn is_degraded(self) -> bool {
        matches!(
            self,
            Self::Unknown | Self::NetContributionFallback | Self::Mixed
        )
    }
}

/// Details about an account that has a negative total_value in its history.
#[derive(Debug, Clone)]
pub struct NegativeBalanceInfo {
    pub account_id: String,
    /// First date the total_value went negative.
    pub first_negative_date: NaiveDate,
    /// Cash balance on that date (account currency).
    pub cash_balance: Decimal,
    /// Total value on that date (account currency).
    pub total_value: Decimal,
    /// Account currency (e.g. "EUR").
    pub account_currency: String,
}

/// Domain model for daily account valuation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DailyAccountValuation {
    pub id: String,
    pub account_id: String,
    pub valuation_date: NaiveDate,
    pub account_currency: String,
    pub base_currency: String,
    pub fx_rate_to_base: Decimal,
    pub cash_balance: Decimal,
    pub investment_market_value: Decimal,
    pub total_value: Decimal,
    pub cost_basis: Decimal,
    pub net_contribution: Decimal,
    pub cash_balance_base: Decimal,
    pub investment_market_value_base: Decimal,
    pub total_value_base: Decimal,
    pub cost_basis_base: Decimal,
    pub net_contribution_base: Decimal,
    pub external_inflow_base: Decimal,
    pub external_outflow_base: Decimal,
    pub external_flow_source: ExternalFlowSource,
    pub performance_eligible_value_base: Decimal,
    pub calculated_at: DateTime<Utc>,
}
