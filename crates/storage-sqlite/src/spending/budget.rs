//! Storage adapter for spending::budget — Diesel impl over budget_config + budget_allocations.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{get_connection, DbPool, WriteHandle};
use crate::errors::StorageError;
use crate::schema::{budget_allocations, budget_config};
use wealthfolio_core::sync::SyncEntity;
use wealthfolio_spending::budget::{
    BudgetAllocation, BudgetConfig, BudgetRepositoryTrait, NewBudgetAllocation, NewBudgetConfig,
    UpdateBudgetConfig,
};

#[derive(Queryable, Identifiable, Selectable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::budget_config)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[serde(rename_all = "camelCase")]
pub struct BudgetConfigDB {
    pub id: String,
    pub monthly_spending_target: String,
    pub monthly_income_target: String,
    pub currency: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Insertable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::budget_config)]
pub struct NewBudgetConfigDB {
    pub id: String,
    pub monthly_spending_target: String,
    pub monthly_income_target: String,
    pub currency: String,
    pub created_at: String,
    pub updated_at: String,
}

impl crate::sync::SyncOutboxModel for BudgetConfigDB {
    const ENTITY: SyncEntity = SyncEntity::BudgetConfig;
    fn sync_entity_id(&self) -> &str {
        &self.id
    }
}

#[derive(Queryable, Identifiable, Selectable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::budget_allocations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[serde(rename_all = "camelCase")]
pub struct BudgetAllocationDB {
    pub id: String,
    pub budget_config_id: String,
    pub taxonomy_id: String,
    pub category_id: String,
    pub amount: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Insertable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::budget_allocations)]
pub struct NewBudgetAllocationDB {
    pub id: String,
    pub budget_config_id: String,
    pub taxonomy_id: String,
    pub category_id: String,
    pub amount: String,
    pub created_at: String,
    pub updated_at: String,
}

impl crate::sync::SyncOutboxModel for BudgetAllocationDB {
    const ENTITY: SyncEntity = SyncEntity::BudgetAllocation;
    fn sync_entity_id(&self) -> &str {
        &self.id
    }
}

fn parse_dt(s: &str) -> NaiveDateTime {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.naive_utc())
        .unwrap_or_else(|_| chrono::Utc::now().naive_utc())
}

impl From<BudgetConfigDB> for BudgetConfig {
    fn from(db: BudgetConfigDB) -> Self {
        Self {
            id: db.id,
            monthly_spending_target: db.monthly_spending_target,
            monthly_income_target: db.monthly_income_target,
            currency: db.currency,
            created_at: parse_dt(&db.created_at),
            updated_at: parse_dt(&db.updated_at),
        }
    }
}

impl From<BudgetAllocationDB> for BudgetAllocation {
    fn from(db: BudgetAllocationDB) -> Self {
        Self {
            id: db.id,
            budget_config_id: db.budget_config_id,
            taxonomy_id: db.taxonomy_id,
            category_id: db.category_id,
            amount: db.amount,
            created_at: parse_dt(&db.created_at),
            updated_at: parse_dt(&db.updated_at),
        }
    }
}

pub struct BudgetRepository {
    pool: Arc<DbPool>,
    writer: WriteHandle,
}

impl BudgetRepository {
    pub fn new(pool: Arc<DbPool>, writer: WriteHandle) -> Self {
        Self { pool, writer }
    }
}

#[async_trait]
impl BudgetRepositoryTrait for BudgetRepository {
    async fn get_config(&self) -> Result<Option<BudgetConfig>> {
        let mut conn = get_connection(&self.pool).map_err(|e| anyhow::anyhow!(e))?;
        // The service maintains a singleton — first-by-id is fine.
        let row = budget_config::table
            .order(budget_config::id.asc())
            .first::<BudgetConfigDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(row.map(Into::into))
    }

    async fn create_config(&self, new_config: NewBudgetConfig) -> Result<BudgetConfig> {
        let now = chrono::Utc::now().to_rfc3339();
        let row = NewBudgetConfigDB {
            id: new_config.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            monthly_spending_target: new_config.monthly_spending_target,
            monthly_income_target: new_config.monthly_income_target,
            currency: new_config.currency,
            created_at: now.clone(),
            updated_at: now,
        };
        self.writer
            .exec_tx(move |tx| {
                let inserted = diesel::insert_into(budget_config::table)
                    .values(&row)
                    .returning(BudgetConfigDB::as_returning())
                    .get_result(tx.conn())
                    .map_err(StorageError::from)?;
                tx.update(&inserted)?;
                Ok(inserted)
            })
            .await
            .map(BudgetConfig::from)
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn update_config(&self, id: &str, patch: UpdateBudgetConfig) -> Result<BudgetConfig> {
        let id = id.to_string();
        self.writer
            .exec_tx(move |tx| {
                let mut existing: BudgetConfigDB = budget_config::table
                    .find(&id)
                    .first::<BudgetConfigDB>(tx.conn())
                    .map_err(StorageError::from)?;
                if let Some(v) = patch.monthly_spending_target {
                    existing.monthly_spending_target = v;
                }
                if let Some(v) = patch.monthly_income_target {
                    existing.monthly_income_target = v;
                }
                if let Some(v) = patch.currency {
                    existing.currency = v;
                }
                existing.updated_at = chrono::Utc::now().to_rfc3339();

                diesel::update(budget_config::table.find(&id))
                    .set((
                        budget_config::monthly_spending_target
                            .eq(&existing.monthly_spending_target),
                        budget_config::monthly_income_target.eq(&existing.monthly_income_target),
                        budget_config::currency.eq(&existing.currency),
                        budget_config::updated_at.eq(&existing.updated_at),
                    ))
                    .execute(tx.conn())
                    .map_err(StorageError::from)?;
                tx.update(&existing)?;
                Ok(existing)
            })
            .await
            .map(BudgetConfig::from)
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn list_allocations(&self, budget_config_id: &str) -> Result<Vec<BudgetAllocation>> {
        let mut conn = get_connection(&self.pool).map_err(|e| anyhow::anyhow!(e))?;
        let rows = budget_allocations::table
            .filter(budget_allocations::budget_config_id.eq(budget_config_id))
            .load::<BudgetAllocationDB>(&mut conn)
            .map_err(StorageError::from)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn upsert_allocation(&self, new_alloc: NewBudgetAllocation) -> Result<BudgetAllocation> {
        let now = chrono::Utc::now().to_rfc3339();
        let row = NewBudgetAllocationDB {
            id: new_alloc.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            budget_config_id: new_alloc.budget_config_id,
            taxonomy_id: new_alloc.taxonomy_id,
            category_id: new_alloc.category_id,
            amount: new_alloc.amount,
            created_at: now.clone(),
            updated_at: now,
        };
        self.writer
            .exec_tx(move |tx| {
                let result = diesel::insert_into(budget_allocations::table)
                    .values(&row)
                    .on_conflict((
                        budget_allocations::budget_config_id,
                        budget_allocations::taxonomy_id,
                        budget_allocations::category_id,
                    ))
                    .do_update()
                    .set((
                        budget_allocations::amount.eq(&row.amount),
                        budget_allocations::updated_at.eq(&row.updated_at),
                    ))
                    .returning(BudgetAllocationDB::as_returning())
                    .get_result(tx.conn())
                    .map_err(StorageError::from)?;
                tx.update(&result)?;
                Ok(result)
            })
            .await
            .map(BudgetAllocation::from)
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn delete_allocation(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.writer
            .exec_tx(move |tx| {
                let affected = diesel::delete(budget_allocations::table.find(&id))
                    .execute(tx.conn())
                    .map_err(StorageError::from)?;
                if affected > 0 {
                    tx.delete::<BudgetAllocationDB>(id.clone());
                }
                Ok(())
            })
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }
}
