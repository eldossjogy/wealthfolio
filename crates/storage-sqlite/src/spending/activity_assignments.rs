//! Storage adapter for spending::activity_assignments — Diesel impl over the
//! `activity_taxonomy_assignments` table.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{get_connection, DbPool, WriteHandle};
use crate::errors::StorageError;
use crate::schema::activity_taxonomy_assignments;
use wealthfolio_core::sync::SyncEntity;
use wealthfolio_spending::activity_assignments::{
    ActivityTaxonomyAssignment, ActivityTaxonomyAssignmentRepositoryTrait,
    NewActivityTaxonomyAssignment,
};

#[derive(
    Queryable, Identifiable, AsChangeset, Selectable, Serialize, Deserialize, Debug, Clone,
)]
#[diesel(table_name = crate::schema::activity_taxonomy_assignments)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[serde(rename_all = "camelCase")]
pub struct ActivityTaxonomyAssignmentDB {
    pub id: String,
    pub activity_id: String,
    pub taxonomy_id: String,
    pub category_id: String,
    pub weight: i32,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Insertable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::activity_taxonomy_assignments)]
pub struct NewActivityTaxonomyAssignmentDB {
    pub id: String,
    pub activity_id: String,
    pub taxonomy_id: String,
    pub category_id: String,
    pub weight: i32,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

impl crate::sync::SyncOutboxModel for ActivityTaxonomyAssignmentDB {
    const ENTITY: SyncEntity = SyncEntity::ActivityTaxonomyAssignment;
    fn sync_entity_id(&self) -> &str {
        &self.id
    }
}

fn parse_dt(s: &str) -> NaiveDateTime {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.naive_utc())
        .unwrap_or_else(|_| chrono::Utc::now().naive_utc())
}

impl From<ActivityTaxonomyAssignmentDB> for ActivityTaxonomyAssignment {
    fn from(db: ActivityTaxonomyAssignmentDB) -> Self {
        Self {
            id: db.id,
            activity_id: db.activity_id,
            taxonomy_id: db.taxonomy_id,
            category_id: db.category_id,
            weight: db.weight,
            source: db.source,
            created_at: parse_dt(&db.created_at),
            updated_at: parse_dt(&db.updated_at),
        }
    }
}

pub struct ActivityTaxonomyAssignmentRepository {
    pool: Arc<DbPool>,
    writer: WriteHandle,
}

impl ActivityTaxonomyAssignmentRepository {
    pub fn new(pool: Arc<DbPool>, writer: WriteHandle) -> Self {
        Self { pool, writer }
    }
}

#[async_trait]
impl ActivityTaxonomyAssignmentRepositoryTrait for ActivityTaxonomyAssignmentRepository {
    async fn list_for_activity(
        &self,
        activity_id: &str,
    ) -> Result<Vec<ActivityTaxonomyAssignment>> {
        let mut conn = get_connection(&self.pool).map_err(|e| anyhow::anyhow!(e))?;
        let rows = activity_taxonomy_assignments::table
            .filter(activity_taxonomy_assignments::activity_id.eq(activity_id))
            .load::<ActivityTaxonomyAssignmentDB>(&mut conn)
            .map_err(StorageError::from)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_for_activities(
        &self,
        activity_ids: &[String],
    ) -> Result<Vec<ActivityTaxonomyAssignment>> {
        if activity_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut conn = get_connection(&self.pool).map_err(|e| anyhow::anyhow!(e))?;
        // SQLite has a default 999-parameter cap; chunk to stay safely under.
        const CHUNK: usize = 500;
        let mut out = Vec::new();
        for chunk in activity_ids.chunks(CHUNK) {
            let rows = activity_taxonomy_assignments::table
                .filter(activity_taxonomy_assignments::activity_id.eq_any(chunk))
                .load::<ActivityTaxonomyAssignmentDB>(&mut conn)
                .map_err(StorageError::from)
                .map_err(|e| anyhow::anyhow!(e))?;
            out.extend(rows.into_iter().map(ActivityTaxonomyAssignment::from));
        }
        Ok(out)
    }

    async fn upsert(
        &self,
        new: NewActivityTaxonomyAssignment,
    ) -> Result<ActivityTaxonomyAssignment> {
        let now = chrono::Utc::now().to_rfc3339();
        let row = NewActivityTaxonomyAssignmentDB {
            id: new.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            activity_id: new.activity_id,
            taxonomy_id: new.taxonomy_id,
            category_id: new.category_id,
            weight: new.weight,
            source: new.source,
            created_at: now.clone(),
            updated_at: now,
        };

        self.writer
            .exec_tx(move |tx| {
                let result = diesel::insert_into(activity_taxonomy_assignments::table)
                    .values(&row)
                    .on_conflict((
                        activity_taxonomy_assignments::activity_id,
                        activity_taxonomy_assignments::taxonomy_id,
                        activity_taxonomy_assignments::category_id,
                    ))
                    .do_update()
                    .set((
                        activity_taxonomy_assignments::weight.eq(&row.weight),
                        activity_taxonomy_assignments::source.eq(&row.source),
                        activity_taxonomy_assignments::updated_at.eq(&row.updated_at),
                    ))
                    .returning(ActivityTaxonomyAssignmentDB::as_returning())
                    .get_result(tx.conn())
                    .map_err(StorageError::from)?;

                tx.update(&result)?;
                Ok(result)
            })
            .await
            .map(ActivityTaxonomyAssignment::from)
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.writer
            .exec_tx(move |tx| {
                let affected = diesel::delete(activity_taxonomy_assignments::table.find(&id))
                    .execute(tx.conn())
                    .map_err(StorageError::from)?;
                if affected > 0 {
                    tx.delete::<ActivityTaxonomyAssignmentDB>(id.clone());
                }
                Ok(())
            })
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn clear_for_taxonomy(&self, activity_id: &str, taxonomy_id: &str) -> Result<()> {
        let activity_id = activity_id.to_string();
        let taxonomy_id = taxonomy_id.to_string();
        self.writer
            .exec_tx(move |tx| {
                let existing_ids = activity_taxonomy_assignments::table
                    .filter(activity_taxonomy_assignments::activity_id.eq(&activity_id))
                    .filter(activity_taxonomy_assignments::taxonomy_id.eq(&taxonomy_id))
                    .select(activity_taxonomy_assignments::id)
                    .load::<String>(tx.conn())
                    .map_err(StorageError::from)?;

                diesel::delete(
                    activity_taxonomy_assignments::table
                        .filter(activity_taxonomy_assignments::activity_id.eq(&activity_id))
                        .filter(activity_taxonomy_assignments::taxonomy_id.eq(&taxonomy_id)),
                )
                .execute(tx.conn())
                .map_err(StorageError::from)?;

                for assignment_id in existing_ids {
                    tx.delete::<ActivityTaxonomyAssignmentDB>(assignment_id);
                }
                Ok(())
            })
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }
}
