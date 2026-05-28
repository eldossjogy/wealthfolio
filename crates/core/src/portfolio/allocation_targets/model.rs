use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ── Enums ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileStatus {
    Draft,
    Active,
    Archived,
}

impl ProfileStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }
}

impl TryFrom<&str> for ProfileStatus {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "draft" => Ok(Self::Draft),
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            _ => Err(format!("unknown profile status: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeType {
    All,
    Portfolio,
    Account,
}

impl ScopeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Portfolio => "portfolio",
            Self::Account => "account",
        }
    }
}

impl TryFrom<&str> for ScopeType {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "all" => Ok(Self::All),
            "portfolio" => Ok(Self::Portfolio),
            "account" => Ok(Self::Account),
            _ => Err(format!("unknown scope type: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    Manual,
    Threshold,
    Calendar,
    Combined,
}

impl TriggerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Threshold => "threshold",
            Self::Calendar => "calendar",
            Self::Combined => "combined",
        }
    }
}

impl TryFrom<&str> for TriggerType {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "manual" => Ok(Self::Manual),
            "threshold" => Ok(Self::Threshold),
            "calendar" => Ok(Self::Calendar),
            "combined" => Ok(Self::Combined),
            _ => Err(format!("unknown trigger type: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFrequency {
    Monthly,
    Quarterly,
    SemiAnnual,
    Annual,
}

impl ReviewFrequency {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Monthly => "monthly",
            Self::Quarterly => "quarterly",
            Self::SemiAnnual => "semi_annual",
            Self::Annual => "annual",
        }
    }
}

impl TryFrom<&str> for ReviewFrequency {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "monthly" => Ok(Self::Monthly),
            "quarterly" => Ok(Self::Quarterly),
            "semi_annual" => Ok(Self::SemiAnnual),
            "annual" => Ok(Self::Annual),
            _ => Err(format!("unknown review frequency: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebalanceTo {
    NearestBand,
    ExactTarget,
}

impl RebalanceTo {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NearestBand => "nearest_band",
            Self::ExactTarget => "exact_target",
        }
    }
}

impl TryFrom<&str> for RebalanceTo {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "nearest_band" => Ok(Self::NearestBand),
            "exact_target" => Ok(Self::ExactTarget),
            _ => Err(format!("unknown rebalance_to: {s}")),
        }
    }
}

// ── Core domain types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProfile {
    pub id: String,
    pub name: String,
    pub status: ProfileStatus,
    pub scope_type: ScopeType,
    pub scope_id: Option<String>,
    pub taxonomy_id: String,
    pub base_currency: String,
    pub trigger_type: TriggerType,
    pub drift_band_bps: i32,
    pub review_frequency: Option<ReviewFrequency>,
    pub next_review_date: Option<String>,
    pub rebalance_to: RebalanceTo,
    pub allow_sells: bool,
    pub min_trade_amount: String,
    pub whole_shares_only: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTargetProfile {
    pub name: String,
    pub scope_type: ScopeType,
    pub scope_id: Option<String>,
    pub taxonomy_id: String,
    pub base_currency: String,
    pub trigger_type: TriggerType,
    pub drift_band_bps: i32,
    pub review_frequency: Option<ReviewFrequency>,
    pub next_review_date: Option<String>,
    pub rebalance_to: RebalanceTo,
    pub allow_sells: bool,
    pub min_trade_amount: String,
    pub whole_shares_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetAllocationNode {
    pub id: String,
    pub profile_id: String,
    pub category_id: String,
    pub target_bps: i32,
    pub is_locked: bool,
    pub is_required: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTargetAllocationNode {
    pub profile_id: String,
    pub category_id: String,
    pub target_bps: i32,
    pub is_locked: bool,
    pub is_required: bool,
}

// ── Drift types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftStatus {
    InBand,
    Underweight,
    Overweight,
    NotTargeted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftRow {
    pub category_id: String,
    pub category_name: String,
    pub color: String,
    pub current_bps: i32,
    pub target_bps: i32,
    pub drift_bps: i32,
    pub current_value: Decimal,
    pub target_value: Decimal,
    pub value_delta: Decimal,
    pub status: DriftStatus,
    pub is_required: bool,
    pub is_zero_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftReport {
    pub profile_id: String,
    pub scope_type: ScopeType,
    pub scope_id: Option<String>,
    pub total_value: Decimal,
    pub base_currency: String,
    pub max_drift_bps: i32,
    pub out_of_band_count: usize,
    pub rows: Vec<DriftRow>,
}
