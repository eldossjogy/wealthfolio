use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ── Enums ────────────────────────────────────────────────────────────────────

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
}

impl TriggerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Threshold => "threshold",
        }
    }
}

impl TryFrom<&str> for TriggerType {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "manual" => Ok(Self::Manual),
            "threshold" => Ok(Self::Threshold),
            _ => Err(format!("unknown trigger type: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebalanceGoal {
    NearestBand,
    ExactTarget,
}

impl RebalanceGoal {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NearestBand => "nearest_band",
            Self::ExactTarget => "exact_target",
        }
    }
}

impl TryFrom<&str> for RebalanceGoal {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "nearest_band" => Ok(Self::NearestBand),
            "exact_target" => Ok(Self::ExactTarget),
            _ => Err(format!("unknown rebalance goal: {s}")),
        }
    }
}

// ── Core domain types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllocationTarget {
    pub id: String,
    pub name: String,
    pub scope_type: ScopeType,
    pub scope_id: Option<String>,
    pub taxonomy_id: String,
    pub trigger_type: TriggerType,
    pub drift_band_bps: i32,
    pub rebalance_goal: RebalanceGoal,
    pub min_trade_amount: String,
    pub whole_shares_only: bool,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAllocationTarget {
    pub name: String,
    pub scope_type: ScopeType,
    pub scope_id: Option<String>,
    pub taxonomy_id: String,
    pub trigger_type: TriggerType,
    pub drift_band_bps: i32,
    pub rebalance_goal: Option<RebalanceGoal>,
    pub min_trade_amount: Option<String>,
    pub whole_shares_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllocationTargetWeight {
    pub id: String,
    pub target_id: String,
    pub taxonomy_id: String,
    pub category_id: String,
    pub target_bps: i32,
    pub is_locked: bool,
    pub is_required: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAllocationTargetWeight {
    pub category_id: String,
    pub target_bps: i32,
    pub is_locked: bool,
    pub is_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAllocationTargetResult {
    pub target: AllocationTarget,
    pub weights: Vec<AllocationTargetWeight>,
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
    /// True when this category holds only cash. The rebalance planner reduces
    /// this row by the deployed cash when estimating after-drift.
    #[serde(default)]
    pub is_cash: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftReport {
    pub target_id: String,
    pub scope_type: ScopeType,
    pub scope_id: Option<String>,
    pub total_value: Decimal,
    pub base_currency: String,
    pub max_drift_bps: i32,
    pub out_of_band_count: usize,
    pub rows: Vec<DriftRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holdings: Option<DriftHoldingsReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftHoldingRow {
    pub id: String,
    pub holding_id: String,
    pub asset_id: String,
    pub account_id: String,
    #[serde(default)]
    pub source_account_ids: Vec<String>,
    pub symbol: String,
    pub name: String,
    pub category_id: String,
    pub category_name: String,
    pub category_color: Option<String>,
    pub value: Decimal,
    pub current_pct: Decimal,
    pub target_pct: Option<Decimal>,
    pub drift_bps: Option<i32>,
    pub is_unknown_category: bool,
    pub is_cash: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftHoldingsReport {
    pub target_id: String,
    pub total_value: Decimal,
    pub base_currency: String,
    pub rows: Vec<DriftHoldingRow>,
}

// ── Rebalance types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalculateRebalancePlanInput {
    pub target_id: String,
    pub available_cash: Decimal,
    pub account_ids: Vec<String>,
    pub base_currency: String,
    pub aggregated_account_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebalanceWarningKind {
    MissingQuote,
    NoBuyCandidate,
    WholeShareResidue,
    /// Asset has no taxonomy assignments for the active taxonomy — skipped as buy candidate.
    UnclassifiedAsset,
    /// Asset has partial taxonomy weights (<100%) — known exposure used, remainder ignored.
    PartialClassification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebalanceWarning {
    pub kind: RebalanceWarningKind,
    pub category_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedManualTrade {
    pub action: String,
    pub category_id: String,
    pub category_name: String,
    pub asset_id: Option<String>,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub quantity: Option<Decimal>,
    pub estimated_price: Option<Decimal>,
    pub estimated_amount: Decimal,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebalancePlan {
    pub target_id: String,
    pub available_cash: Decimal,
    pub cash_used: Decimal,
    pub cash_remaining: Decimal,
    pub max_drift_bps_before: i32,
    pub max_drift_bps_after: i32,
    pub trades: Vec<SuggestedManualTrade>,
    pub warnings: Vec<RebalanceWarning>,
}
