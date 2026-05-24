//! Storage adapter for spending::categorization_rules.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{get_connection, DbPool, WriteHandle};
use crate::errors::StorageError;
use crate::schema::spending_categorization_rules;
use wealthfolio_core::sync::SyncEntity;
use wealthfolio_spending::categorization_rules::{
    CategorizationRule, CategorizationRulesRepositoryTrait, NewCategorizationRule, RuleMatchType,
    UpdateCategorizationRule,
};

#[derive(Queryable, Identifiable, Selectable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::spending_categorization_rules)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[serde(rename_all = "camelCase")]
pub struct CategorizationRuleDB {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub match_type: String,
    pub taxonomy_id: Option<String>,
    pub category_id: Option<String>,
    pub activity_type: Option<String>,
    pub priority: i32,
    pub is_global: i32,
    pub account_id: Option<String>,
    pub preset_id: Option<String>,
    pub preset_rule_key: Option<String>,
    pub preset_version: Option<String>,
    pub preset_modified: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Insertable, AsChangeset, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::spending_categorization_rules)]
pub struct NewCategorizationRuleDB {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub match_type: String,
    pub taxonomy_id: Option<String>,
    pub category_id: Option<String>,
    pub activity_type: Option<String>,
    pub priority: i32,
    pub is_global: i32,
    pub account_id: Option<String>,
    pub preset_id: Option<String>,
    pub preset_rule_key: Option<String>,
    pub preset_version: Option<String>,
    pub preset_modified: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl crate::sync::SyncOutboxModel for CategorizationRuleDB {
    const ENTITY: SyncEntity = SyncEntity::SpendingCategorizationRule;
    fn sync_entity_id(&self) -> &str {
        &self.id
    }
}

fn parse_dt(s: &str) -> NaiveDateTime {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.naive_utc())
        .unwrap_or_else(|_| chrono::Utc::now().naive_utc())
}

impl From<CategorizationRuleDB> for CategorizationRule {
    fn from(db: CategorizationRuleDB) -> Self {
        Self {
            id: db.id,
            name: db.name,
            pattern: db.pattern,
            match_type: RuleMatchType::parse(&db.match_type),
            taxonomy_id: db.taxonomy_id,
            category_id: db.category_id,
            activity_type: db.activity_type,
            priority: db.priority,
            is_global: db.is_global != 0,
            account_id: db.account_id,
            preset_id: db.preset_id,
            preset_rule_key: db.preset_rule_key,
            preset_version: db.preset_version,
            preset_modified: db.preset_modified != 0,
            created_at: parse_dt(&db.created_at),
            updated_at: parse_dt(&db.updated_at),
        }
    }
}

pub struct CategorizationRulesRepository {
    pool: Arc<DbPool>,
    writer: WriteHandle,
}

impl CategorizationRulesRepository {
    pub fn new(pool: Arc<DbPool>, writer: WriteHandle) -> Self {
        Self { pool, writer }
    }
}

#[async_trait]
impl CategorizationRulesRepositoryTrait for CategorizationRulesRepository {
    async fn list(&self) -> Result<Vec<CategorizationRule>> {
        let mut conn = get_connection(&self.pool).map_err(|e| anyhow::anyhow!(e))?;
        let rows = spending_categorization_rules::table
            .order(spending_categorization_rules::priority.desc())
            .load::<CategorizationRuleDB>(&mut conn)
            .map_err(StorageError::from)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn get(&self, id: &str) -> Result<Option<CategorizationRule>> {
        let mut conn = get_connection(&self.pool).map_err(|e| anyhow::anyhow!(e))?;
        let row = spending_categorization_rules::table
            .find(id)
            .first::<CategorizationRuleDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(row.map(Into::into))
    }

    async fn create(&self, new_rule: NewCategorizationRule) -> Result<CategorizationRule> {
        let now = chrono::Utc::now().to_rfc3339();
        let row = NewCategorizationRuleDB {
            id: new_rule.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            name: new_rule.name,
            pattern: new_rule.pattern,
            match_type: new_rule.match_type.as_str().to_string(),
            taxonomy_id: new_rule.taxonomy_id,
            category_id: new_rule.category_id,
            activity_type: new_rule.activity_type,
            priority: new_rule.priority,
            is_global: if new_rule.is_global { 1 } else { 0 },
            account_id: new_rule.account_id,
            preset_id: new_rule.preset_id,
            preset_rule_key: new_rule.preset_rule_key,
            preset_version: new_rule.preset_version,
            preset_modified: 0,
            created_at: now.clone(),
            updated_at: now,
        };
        self.writer
            .exec_tx(move |tx| {
                let inserted = diesel::insert_into(spending_categorization_rules::table)
                    .values(&row)
                    .returning(CategorizationRuleDB::as_returning())
                    .get_result(tx.conn())
                    .map_err(StorageError::from)?;
                tx.update(&inserted)?;
                Ok(inserted)
            })
            .await
            .map(CategorizationRule::from)
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn update(
        &self,
        id: &str,
        patch: UpdateCategorizationRule,
    ) -> Result<CategorizationRule> {
        let id = id.to_string();
        self.writer
            .exec_tx(move |tx| {
                let mut existing: CategorizationRuleDB = spending_categorization_rules::table
                    .find(&id)
                    .first::<CategorizationRuleDB>(tx.conn())
                    .map_err(StorageError::from)?;
                if let Some(v) = patch.name {
                    existing.name = v;
                }
                if let Some(v) = patch.pattern {
                    existing.pattern = v;
                }
                if let Some(v) = patch.match_type {
                    existing.match_type = v.as_str().to_string();
                }
                if let Some(v) = patch.taxonomy_id {
                    existing.taxonomy_id = v;
                }
                if let Some(v) = patch.category_id {
                    existing.category_id = v;
                }
                if let Some(v) = patch.activity_type {
                    existing.activity_type = v;
                }
                if let Some(v) = patch.priority {
                    existing.priority = v;
                }
                if let Some(v) = patch.is_global {
                    existing.is_global = if v { 1 } else { 0 };
                }
                if let Some(v) = patch.account_id {
                    existing.account_id = v;
                }
                // If this rule came from a preset, mark it as user-modified so
                // future preset updates can ask before overwriting.
                if existing.preset_id.is_some() {
                    existing.preset_modified = 1;
                }
                existing.updated_at = chrono::Utc::now().to_rfc3339();

                diesel::update(spending_categorization_rules::table.find(&id))
                    .set((
                        spending_categorization_rules::name.eq(&existing.name),
                        spending_categorization_rules::pattern.eq(&existing.pattern),
                        spending_categorization_rules::match_type.eq(&existing.match_type),
                        spending_categorization_rules::taxonomy_id.eq(&existing.taxonomy_id),
                        spending_categorization_rules::category_id.eq(&existing.category_id),
                        spending_categorization_rules::activity_type.eq(&existing.activity_type),
                        spending_categorization_rules::priority.eq(existing.priority),
                        spending_categorization_rules::is_global.eq(existing.is_global),
                        spending_categorization_rules::account_id.eq(&existing.account_id),
                        spending_categorization_rules::preset_modified.eq(existing.preset_modified),
                        spending_categorization_rules::updated_at.eq(&existing.updated_at),
                    ))
                    .execute(tx.conn())
                    .map_err(StorageError::from)?;

                tx.update(&existing)?;
                Ok(existing)
            })
            .await
            .map(CategorizationRule::from)
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.writer
            .exec_tx(move |tx| {
                let affected = diesel::delete(spending_categorization_rules::table.find(&id))
                    .execute(tx.conn())
                    .map_err(StorageError::from)?;
                if affected > 0 {
                    tx.delete::<CategorizationRuleDB>(id.clone());
                }
                Ok(())
            })
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn remove_preset(&self, preset_id: &str) -> Result<(usize, usize)> {
        let preset_id = preset_id.to_string();
        self.writer
            .exec_tx(move |tx| {
                let rows: Vec<CategorizationRuleDB> = spending_categorization_rules::table
                    .filter(spending_categorization_rules::preset_id.eq(&preset_id))
                    .load::<CategorizationRuleDB>(tx.conn())
                    .map_err(StorageError::from)?;

                let mut removed = 0usize;
                let mut kept = 0usize;
                let now = chrono::Utc::now().to_rfc3339();

                for row in rows {
                    if row.preset_modified != 0 {
                        // Detach: clear preset metadata, keep the rule as user-owned.
                        diesel::update(spending_categorization_rules::table.find(&row.id))
                            .set((
                                spending_categorization_rules::preset_id.eq::<Option<String>>(None),
                                spending_categorization_rules::preset_rule_key
                                    .eq::<Option<String>>(None),
                                spending_categorization_rules::preset_version
                                    .eq::<Option<String>>(None),
                                spending_categorization_rules::preset_modified.eq(0),
                                spending_categorization_rules::updated_at.eq(&now),
                            ))
                            .execute(tx.conn())
                            .map_err(StorageError::from)?;
                        let mut detached = row.clone();
                        detached.preset_id = None;
                        detached.preset_rule_key = None;
                        detached.preset_version = None;
                        detached.preset_modified = 0;
                        detached.updated_at = now.clone();
                        tx.update(&detached)?;
                        kept += 1;
                    } else {
                        diesel::delete(spending_categorization_rules::table.find(&row.id))
                            .execute(tx.conn())
                            .map_err(StorageError::from)?;
                        tx.delete::<CategorizationRuleDB>(row.id.clone());
                        removed += 1;
                    }
                }
                Ok((removed, kept))
            })
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }
}
