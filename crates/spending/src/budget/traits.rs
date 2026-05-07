use anyhow::Result;
use async_trait::async_trait;

use super::model::{
    BudgetAllocation, BudgetConfig, NewBudgetAllocation, NewBudgetConfig, UpdateBudgetConfig,
};

#[async_trait]
pub trait BudgetRepositoryTrait: Send + Sync {
    /// Get the singleton budget config (id="default"). Returns None if not yet created.
    async fn get_config(&self) -> Result<Option<BudgetConfig>>;

    /// Create the budget config (called once on first save).
    async fn create_config(&self, new_config: NewBudgetConfig) -> Result<BudgetConfig>;

    /// Update the budget config in place.
    async fn update_config(&self, id: &str, patch: UpdateBudgetConfig) -> Result<BudgetConfig>;

    /// List allocations for the given budget_config_id.
    async fn list_allocations(&self, budget_config_id: &str) -> Result<Vec<BudgetAllocation>>;

    /// Upsert an allocation (UNIQUE on (budget_config_id, taxonomy_id, category_id)).
    async fn upsert_allocation(&self, new_alloc: NewBudgetAllocation) -> Result<BudgetAllocation>;

    /// Remove an allocation by id.
    async fn delete_allocation(&self, id: &str) -> Result<()>;
}
