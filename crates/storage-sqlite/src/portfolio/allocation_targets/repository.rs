use async_trait::async_trait;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use std::sync::Arc;

use super::model::{AllocationTargetDB, AllocationTargetWeightDB};
use crate::db::{get_connection, WriteHandle};
use crate::errors::StorageError;
use crate::schema::{allocation_target_weights, allocation_targets};
use wealthfolio_core::errors::Result;
use wealthfolio_core::portfolio::allocation_targets::{
    AllocationTarget, AllocationTargetRepositoryTrait, AllocationTargetWeight,
    SaveAllocationTargetResult,
};

pub struct AllocationTargetRepository {
    pool: Arc<Pool<ConnectionManager<SqliteConnection>>>,
    writer: WriteHandle,
}

impl AllocationTargetRepository {
    pub fn new(pool: Arc<Pool<ConnectionManager<SqliteConnection>>>, writer: WriteHandle) -> Self {
        Self { pool, writer }
    }

    fn map_targets(rows: Vec<AllocationTargetDB>) -> Result<Vec<AllocationTarget>> {
        rows.into_iter()
            .map(|db| {
                AllocationTarget::try_from(db).map_err(|e| {
                    wealthfolio_core::errors::Error::Validation(
                        wealthfolio_core::errors::ValidationError::InvalidInput(e),
                    )
                })
            })
            .collect()
    }
}

#[async_trait]
impl AllocationTargetRepositoryTrait for AllocationTargetRepository {
    fn get_target(&self, id: &str) -> Result<Option<AllocationTarget>> {
        let conn = &mut get_connection(&self.pool)?;
        let row = allocation_targets::table
            .filter(allocation_targets::id.eq(id))
            .first::<AllocationTargetDB>(conn)
            .optional()
            .map_err(StorageError::from)?;
        row.map(AllocationTarget::try_from)
            .transpose()
            .map_err(|e| {
                wealthfolio_core::errors::Error::Validation(
                    wealthfolio_core::errors::ValidationError::InvalidInput(e),
                )
            })
    }

    fn list_targets(&self) -> Result<Vec<AllocationTarget>> {
        let conn = &mut get_connection(&self.pool)?;
        let rows = allocation_targets::table
            .order(allocation_targets::created_at.asc())
            .load::<AllocationTargetDB>(conn)
            .map_err(StorageError::from)?;
        Self::map_targets(rows)
    }

    fn list_weights_for_target(&self, target_id: &str) -> Result<Vec<AllocationTargetWeight>> {
        let conn = &mut get_connection(&self.pool)?;
        let rows = allocation_target_weights::table
            .filter(allocation_target_weights::target_id.eq(target_id))
            .order(allocation_target_weights::created_at.asc())
            .load::<AllocationTargetWeightDB>(conn)
            .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(AllocationTargetWeight::from).collect())
    }

    async fn create_target(&self, target: AllocationTarget) -> Result<AllocationTarget> {
        let db = AllocationTargetDB::from(target);
        let id = db.id.clone();
        self.writer
            .exec(move |conn| {
                diesel::insert_into(allocation_targets::table)
                    .values(&db)
                    .execute(conn)
                    .map_err(StorageError::from)?;
                Ok(())
            })
            .await?;
        self.get_target(&id)?.ok_or_else(|| {
            wealthfolio_core::errors::Error::Database(
                wealthfolio_core::errors::DatabaseError::NotFound(format!(
                    "AllocationTarget {} not found",
                    id
                )),
            )
        })
    }

    async fn update_target(&self, target: AllocationTarget) -> Result<AllocationTarget> {
        let id = target.id.clone();
        let db = AllocationTargetDB::from(target);
        self.writer
            .exec(move |conn| {
                diesel::update(allocation_targets::table.filter(allocation_targets::id.eq(&db.id)))
                    .set(&db)
                    .execute(conn)
                    .map_err(StorageError::from)?;
                Ok(())
            })
            .await?;
        self.get_target(&id)?.ok_or_else(|| {
            wealthfolio_core::errors::Error::Database(
                wealthfolio_core::errors::DatabaseError::NotFound(format!(
                    "AllocationTarget {} not found",
                    id
                )),
            )
        })
    }

    async fn delete_target(&self, id: &str) -> Result<usize> {
        let id_owned = id.to_string();
        self.writer
            .exec(move |conn| {
                let n = diesel::delete(
                    allocation_targets::table.filter(allocation_targets::id.eq(&id_owned)),
                )
                .execute(conn)
                .map_err(StorageError::from)?;
                Ok(n)
            })
            .await
    }

    async fn save_weights(
        &self,
        target_id: &str,
        weights: Vec<AllocationTargetWeight>,
    ) -> Result<Vec<AllocationTargetWeight>> {
        let target_id_owned = target_id.to_string();
        let db_weights: Vec<AllocationTargetWeightDB> = weights
            .into_iter()
            .map(AllocationTargetWeightDB::from)
            .collect();

        self.writer
            .exec(move |conn| {
                // Replace all weights for this target atomically.
                diesel::delete(
                    allocation_target_weights::table
                        .filter(allocation_target_weights::target_id.eq(&target_id_owned)),
                )
                .execute(conn)
                .map_err(StorageError::from)?;

                if !db_weights.is_empty() {
                    diesel::insert_into(allocation_target_weights::table)
                        .values(&db_weights)
                        .execute(conn)
                        .map_err(StorageError::from)?;
                }
                Ok(())
            })
            .await?;

        self.list_weights_for_target(target_id)
    }

    async fn save_target_with_weights(
        &self,
        target: AllocationTarget,
        weights: Vec<AllocationTargetWeight>,
    ) -> Result<SaveAllocationTargetResult> {
        let target_db = AllocationTargetDB::from(target);
        let target_id = target_db.id.clone();
        let db_weights: Vec<AllocationTargetWeightDB> = weights
            .into_iter()
            .map(AllocationTargetWeightDB::from)
            .collect();

        self.writer
            .exec({
                let target_id = target_id.clone();
                move |conn| {
                    diesel::delete(
                        allocation_target_weights::table
                            .filter(allocation_target_weights::target_id.eq(&target_id)),
                    )
                    .execute(conn)
                    .map_err(StorageError::from)?;

                    let updated = diesel::update(
                        allocation_targets::table.filter(allocation_targets::id.eq(&target_id)),
                    )
                    .set(&target_db)
                    .execute(conn)
                    .map_err(StorageError::from)?;

                    if updated == 0 {
                        diesel::insert_into(allocation_targets::table)
                            .values(&target_db)
                            .execute(conn)
                            .map_err(StorageError::from)?;
                    }

                    if !db_weights.is_empty() {
                        diesel::insert_into(allocation_target_weights::table)
                            .values(&db_weights)
                            .execute(conn)
                            .map_err(StorageError::from)?;
                    }
                    Ok(())
                }
            })
            .await?;

        let target = self.get_target(&target_id)?.ok_or_else(|| {
            wealthfolio_core::errors::Error::Database(
                wealthfolio_core::errors::DatabaseError::NotFound(format!(
                    "AllocationTarget {} not found",
                    target_id
                )),
            )
        })?;
        let weights = self.list_weights_for_target(&target_id)?;
        Ok(SaveAllocationTargetResult { target, weights })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_pool, init, run_migrations, write_actor::spawn_writer};
    use crate::taxonomies::TaxonomyRepository;
    use tempfile::tempdir;
    use wealthfolio_core::portfolio::allocation_targets::{RebalanceGoal, ScopeType, TriggerType};
    use wealthfolio_core::taxonomies::TaxonomyRepositoryTrait;

    fn setup_repos() -> (AllocationTargetRepository, TaxonomyRepository) {
        std::env::set_var("CONNECT_API_URL", "http://test.local");
        let app_data = tempdir()
            .expect("tempdir")
            .keep()
            .to_string_lossy()
            .to_string();
        let db_path = init(&app_data).expect("init db");
        run_migrations(&db_path).expect("migrate db");
        let pool = create_pool(&db_path).expect("create pool");
        let writer = spawn_writer(pool.as_ref().clone()).expect("spawn writer");
        (
            AllocationTargetRepository::new(pool.clone(), writer.clone()),
            TaxonomyRepository::new(pool, writer),
        )
    }

    fn setup_repo() -> AllocationTargetRepository {
        setup_repos().0
    }

    fn target(taxonomy_id: &str) -> AllocationTarget {
        AllocationTarget {
            id: "target-1".to_string(),
            name: "Target".to_string(),
            scope_type: ScopeType::All,
            scope_id: None,
            taxonomy_id: taxonomy_id.to_string(),
            trigger_type: TriggerType::Threshold,
            drift_band_bps: 500,
            rebalance_goal: RebalanceGoal::NearestBand,
            min_trade_amount: "0".to_string(),
            whole_shares_only: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            archived_at: None,
        }
    }

    fn weight(taxonomy_id: &str, category_id: &str) -> AllocationTargetWeight {
        AllocationTargetWeight {
            id: format!("weight-{taxonomy_id}-{category_id}"),
            target_id: "target-1".to_string(),
            taxonomy_id: taxonomy_id.to_string(),
            category_id: category_id.to_string(),
            target_bps: 10000,
            is_locked: false,
            is_required: true,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn save_weights_persists_weight_taxonomy_id() {
        let repo = setup_repo();
        repo.create_target(target("asset_classes")).await.unwrap();

        let saved = repo
            .save_weights("target-1", vec![weight("asset_classes", "CASH")])
            .await
            .unwrap();

        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].taxonomy_id, "asset_classes");
        assert_eq!(saved[0].category_id, "CASH");
    }

    #[tokio::test]
    async fn save_weights_rejects_category_from_another_taxonomy() {
        let repo = setup_repo();
        repo.create_target(target("asset_classes")).await.unwrap();

        let err = repo
            .save_weights("target-1", vec![weight("regions", "R10")])
            .await
            .unwrap_err();

        assert!(err
            .to_string()
            .contains("allocation_target_weights.taxonomy_id must match"));
    }

    #[tokio::test]
    async fn update_target_rejects_taxonomy_change_when_weights_exist() {
        let repo = setup_repo();
        repo.create_target(target("asset_classes")).await.unwrap();
        repo.save_weights("target-1", vec![weight("asset_classes", "CASH")])
            .await
            .unwrap();

        let err = repo.update_target(target("regions")).await.unwrap_err();

        assert!(err
            .to_string()
            .contains("allocation_targets.taxonomy_id cannot change while weights exist"));
    }

    #[tokio::test]
    async fn save_target_with_weights_allows_taxonomy_change_with_replacement_weights() {
        let repo = setup_repo();
        repo.create_target(target("asset_classes")).await.unwrap();
        repo.save_weights("target-1", vec![weight("asset_classes", "CASH")])
            .await
            .unwrap();

        let saved = repo
            .save_target_with_weights(target("regions"), vec![weight("regions", "R10")])
            .await
            .unwrap();

        assert_eq!(saved.target.taxonomy_id, "regions");
        assert_eq!(saved.weights.len(), 1);
        assert_eq!(saved.weights[0].taxonomy_id, "regions");
        assert_eq!(saved.weights[0].category_id, "R10");
    }

    #[tokio::test]
    async fn taxonomy_reference_count_includes_allocation_target_weights() {
        let (target_repo, taxonomy_repo) = setup_repos();
        target_repo
            .create_target(target("asset_classes"))
            .await
            .unwrap();
        target_repo
            .save_weights("target-1", vec![weight("asset_classes", "CASH")])
            .await
            .unwrap();

        let count = taxonomy_repo
            .get_category_allocation_target_weight_count("asset_classes", "CASH")
            .unwrap();

        assert_eq!(count, 1);
    }
}
