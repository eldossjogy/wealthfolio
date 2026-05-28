use diesel::prelude::*;
use wealthfolio_core::portfolio::allocation_targets::{
    ProfileStatus, ScopeType, TargetAllocationNode, TargetProfile, TriggerType,
};

#[derive(Debug, Clone, Queryable, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::target_profiles)]
pub struct TargetProfileDB {
    pub id: String,
    pub name: String,
    pub status: String,
    pub scope_type: String,
    pub scope_id: Option<String>,
    pub taxonomy_id: String,
    pub trigger_type: String,
    pub drift_band_bps: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<TargetProfile> for TargetProfileDB {
    fn from(p: TargetProfile) -> Self {
        Self {
            id: p.id,
            name: p.name,
            status: p.status.as_str().to_string(),
            scope_type: p.scope_type.as_str().to_string(),
            scope_id: p.scope_id,
            taxonomy_id: p.taxonomy_id,
            trigger_type: p.trigger_type.as_str().to_string(),
            drift_band_bps: p.drift_band_bps,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

impl TryFrom<TargetProfileDB> for TargetProfile {
    type Error = String;
    fn try_from(db: TargetProfileDB) -> Result<Self, Self::Error> {
        Ok(TargetProfile {
            id: db.id,
            name: db.name,
            status: ProfileStatus::try_from(db.status.as_str())?,
            scope_type: ScopeType::try_from(db.scope_type.as_str())?,
            scope_id: db.scope_id,
            taxonomy_id: db.taxonomy_id,
            trigger_type: TriggerType::try_from(db.trigger_type.as_str())?,
            drift_band_bps: db.drift_band_bps,
            created_at: db.created_at,
            updated_at: db.updated_at,
        })
    }
}

#[derive(Debug, Clone, Queryable, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::target_allocation_nodes)]
pub struct TargetAllocationNodeDB {
    pub id: String,
    pub profile_id: String,
    pub category_id: String,
    pub target_bps: i32,
    pub is_locked: i32,
    pub is_required: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<TargetAllocationNode> for TargetAllocationNodeDB {
    fn from(n: TargetAllocationNode) -> Self {
        Self {
            id: n.id,
            profile_id: n.profile_id,
            category_id: n.category_id,
            target_bps: n.target_bps,
            is_locked: n.is_locked as i32,
            is_required: n.is_required as i32,
            created_at: n.created_at,
            updated_at: n.updated_at,
        }
    }
}

impl From<TargetAllocationNodeDB> for TargetAllocationNode {
    fn from(db: TargetAllocationNodeDB) -> Self {
        Self {
            id: db.id,
            profile_id: db.profile_id,
            category_id: db.category_id,
            target_bps: db.target_bps,
            is_locked: db.is_locked != 0,
            is_required: db.is_required != 0,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}
