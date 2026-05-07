use std::sync::Arc;

use anyhow::Result;

use super::model::{
    BudgetAllocation, BudgetConfig, BudgetSnapshot, NewBudgetAllocation, NewBudgetConfig,
    UpdateBudgetConfig,
};
use super::traits::BudgetRepositoryTrait;

const DEFAULT_CONFIG_ID: &str = "default";

pub struct BudgetService {
    repo: Arc<dyn BudgetRepositoryTrait>,
}

impl BudgetService {
    pub fn new(repo: Arc<dyn BudgetRepositoryTrait>) -> Self {
        Self { repo }
    }

    /// Get the budget snapshot. Lazy-creates the singleton config row on first read.
    pub async fn get(&self, fallback_currency: &str) -> Result<BudgetSnapshot> {
        let config = match self.repo.get_config().await? {
            Some(c) => c,
            None => {
                self.repo
                    .create_config(NewBudgetConfig {
                        id: Some(DEFAULT_CONFIG_ID.to_string()),
                        monthly_spending_target: "0".to_string(),
                        monthly_income_target: "0".to_string(),
                        currency: fallback_currency.to_string(),
                    })
                    .await?
            }
        };
        let allocations = self.repo.list_allocations(&config.id).await?;
        Ok(BudgetSnapshot {
            config,
            allocations,
        })
    }

    pub async fn update_config(
        &self,
        patch: UpdateBudgetConfig,
        fallback_currency: &str,
    ) -> Result<BudgetConfig> {
        let existing = self.repo.get_config().await?;
        let id = match existing {
            Some(c) => c.id,
            None => {
                self.repo
                    .create_config(NewBudgetConfig {
                        id: Some(DEFAULT_CONFIG_ID.to_string()),
                        monthly_spending_target: "0".to_string(),
                        monthly_income_target: "0".to_string(),
                        currency: fallback_currency.to_string(),
                    })
                    .await?
                    .id
            }
        };
        self.repo.update_config(&id, patch).await
    }

    pub async fn upsert_allocation(
        &self,
        taxonomy_id: String,
        category_id: String,
        amount: String,
        fallback_currency: &str,
    ) -> Result<BudgetAllocation> {
        let snapshot = self.get(fallback_currency).await?;
        self.repo
            .upsert_allocation(NewBudgetAllocation {
                id: None,
                budget_config_id: snapshot.config.id,
                taxonomy_id,
                category_id,
                amount,
            })
            .await
    }

    pub async fn delete_allocation(&self, id: &str) -> Result<()> {
        self.repo.delete_allocation(id).await
    }
}
