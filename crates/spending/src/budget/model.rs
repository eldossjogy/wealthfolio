use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetConfig {
    pub id: String,
    /// Stored as decimal string ("0", "500.00")
    pub monthly_spending_target: String,
    pub monthly_income_target: String,
    pub currency: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewBudgetConfig {
    pub id: Option<String>,
    pub monthly_spending_target: String,
    pub monthly_income_target: String,
    pub currency: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBudgetConfig {
    pub monthly_spending_target: Option<String>,
    pub monthly_income_target: Option<String>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetAllocation {
    pub id: String,
    pub budget_config_id: String,
    pub taxonomy_id: String,
    pub category_id: String,
    pub amount: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewBudgetAllocation {
    pub id: Option<String>,
    pub budget_config_id: String,
    pub taxonomy_id: String,
    pub category_id: String,
    pub amount: String,
}

/// Convenience snapshot for the UI: full config + allocations in one shot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetSnapshot {
    pub config: BudgetConfig,
    pub allocations: Vec<BudgetAllocation>,
}
