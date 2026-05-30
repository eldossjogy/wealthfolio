use async_trait::async_trait;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use std::sync::Arc;

use super::model::{TargetAllocationNodeDB, TargetProfileDB};
use crate::db::{get_connection, WriteHandle};
use crate::errors::StorageError;
use crate::schema::{target_allocation_nodes, target_profiles};
use wealthfolio_core::errors::Result;
use wealthfolio_core::portfolio::allocation_targets::{
    TargetAllocationNode, TargetProfile, TargetProfileRepositoryTrait,
};

pub struct TargetProfileRepository {
    pool: Arc<Pool<ConnectionManager<SqliteConnection>>>,
    writer: WriteHandle,
}

impl TargetProfileRepository {
    pub fn new(pool: Arc<Pool<ConnectionManager<SqliteConnection>>>, writer: WriteHandle) -> Self {
        Self { pool, writer }
    }

    fn map_profiles(rows: Vec<TargetProfileDB>) -> Result<Vec<TargetProfile>> {
        rows.into_iter()
            .map(|db| {
                TargetProfile::try_from(db).map_err(|e| {
                    wealthfolio_core::errors::Error::Validation(
                        wealthfolio_core::errors::ValidationError::InvalidInput(e),
                    )
                })
            })
            .collect()
    }
}

#[async_trait]
impl TargetProfileRepositoryTrait for TargetProfileRepository {
    fn get_profile(&self, id: &str) -> Result<Option<TargetProfile>> {
        let conn = &mut get_connection(&self.pool)?;
        let row = target_profiles::table
            .filter(target_profiles::id.eq(id))
            .first::<TargetProfileDB>(conn)
            .optional()
            .map_err(StorageError::from)?;
        row.map(TargetProfile::try_from).transpose().map_err(|e| {
            wealthfolio_core::errors::Error::Validation(
                wealthfolio_core::errors::ValidationError::InvalidInput(e),
            )
        })
    }

    fn list_profiles(&self) -> Result<Vec<TargetProfile>> {
        let conn = &mut get_connection(&self.pool)?;
        let rows = target_profiles::table
            .order(target_profiles::created_at.asc())
            .load::<TargetProfileDB>(conn)
            .map_err(StorageError::from)?;
        Self::map_profiles(rows)
    }

    fn get_active_profile_for_scope(
        &self,
        scope_type: &str,
        scope_id: Option<&str>,
    ) -> Result<Option<TargetProfile>> {
        let conn = &mut get_connection(&self.pool)?;
        let row = match scope_id {
            Some(sid) => target_profiles::table
                .filter(target_profiles::scope_type.eq(scope_type))
                .filter(target_profiles::scope_id.eq(sid))
                .filter(target_profiles::status.eq("active"))
                .first::<TargetProfileDB>(conn)
                .optional()
                .map_err(StorageError::from)?,
            None => target_profiles::table
                .filter(target_profiles::scope_type.eq(scope_type))
                .filter(target_profiles::scope_id.is_null())
                .filter(target_profiles::status.eq("active"))
                .first::<TargetProfileDB>(conn)
                .optional()
                .map_err(StorageError::from)?,
        };
        row.map(TargetProfile::try_from).transpose().map_err(|e| {
            wealthfolio_core::errors::Error::Validation(
                wealthfolio_core::errors::ValidationError::InvalidInput(e),
            )
        })
    }

    fn list_nodes_for_profile(&self, profile_id: &str) -> Result<Vec<TargetAllocationNode>> {
        let conn = &mut get_connection(&self.pool)?;
        let rows = target_allocation_nodes::table
            .filter(target_allocation_nodes::profile_id.eq(profile_id))
            .order(target_allocation_nodes::created_at.asc())
            .load::<TargetAllocationNodeDB>(conn)
            .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(TargetAllocationNode::from).collect())
    }

    async fn create_profile(&self, profile: TargetProfile) -> Result<TargetProfile> {
        let db = TargetProfileDB::from(profile);
        let id = db.id.clone();
        self.writer
            .exec(move |conn| {
                diesel::insert_into(target_profiles::table)
                    .values(&db)
                    .execute(conn)
                    .map_err(StorageError::from)?;
                Ok(())
            })
            .await?;
        self.get_profile(&id)?.ok_or_else(|| {
            wealthfolio_core::errors::Error::Database(
                wealthfolio_core::errors::DatabaseError::NotFound(format!(
                    "TargetProfile {} not found",
                    id
                )),
            )
        })
    }

    async fn update_profile(&self, profile: TargetProfile) -> Result<TargetProfile> {
        let id = profile.id.clone();
        let db = TargetProfileDB::from(profile);
        self.writer
            .exec(move |conn| {
                diesel::update(target_profiles::table.filter(target_profiles::id.eq(&db.id)))
                    .set(&db)
                    .execute(conn)
                    .map_err(StorageError::from)?;
                Ok(())
            })
            .await?;
        self.get_profile(&id)?.ok_or_else(|| {
            wealthfolio_core::errors::Error::Database(
                wealthfolio_core::errors::DatabaseError::NotFound(format!(
                    "TargetProfile {} not found",
                    id
                )),
            )
        })
    }

    async fn delete_profile(&self, id: &str) -> Result<usize> {
        let id_owned = id.to_string();
        self.writer
            .exec(move |conn| {
                let n = diesel::delete(
                    target_profiles::table.filter(target_profiles::id.eq(&id_owned)),
                )
                .execute(conn)
                .map_err(StorageError::from)?;
                Ok(n)
            })
            .await
    }

    async fn save_nodes(
        &self,
        profile_id: &str,
        nodes: Vec<TargetAllocationNode>,
    ) -> Result<Vec<TargetAllocationNode>> {
        let profile_id_owned = profile_id.to_string();
        let db_nodes: Vec<TargetAllocationNodeDB> = nodes
            .into_iter()
            .map(TargetAllocationNodeDB::from)
            .collect();

        self.writer
            .exec(move |conn| {
                // Replace all nodes for this profile atomically
                diesel::delete(
                    target_allocation_nodes::table
                        .filter(target_allocation_nodes::profile_id.eq(&profile_id_owned)),
                )
                .execute(conn)
                .map_err(StorageError::from)?;

                diesel::insert_into(target_allocation_nodes::table)
                    .values(&db_nodes)
                    .execute(conn)
                    .map_err(StorageError::from)?;
                Ok(())
            })
            .await?;

        self.list_nodes_for_profile(profile_id)
    }
}
