use diesel::prelude::*;
use wealthfolio_core::portfolio::allocation_targets::{
    ProfileStatus, RebalanceTo, ReviewFrequency, ScopeType, TargetAllocationNode, TargetProfile,
    TriggerType,
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
    pub base_currency: String,
    pub trigger_type: String,
    pub drift_band_bps: i32,
    pub review_frequency: Option<String>,
    pub next_review_date: Option<String>,
    pub rebalance_to: String,
    pub allow_sells: i32,
    pub min_trade_amount: String,
    pub whole_shares_only: i32,
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
            base_currency: p.base_currency,
            trigger_type: p.trigger_type.as_str().to_string(),
            drift_band_bps: p.drift_band_bps,
            review_frequency: p.review_frequency.map(|f| f.as_str().to_string()),
            next_review_date: p.next_review_date,
            rebalance_to: p.rebalance_to.as_str().to_string(),
            allow_sells: p.allow_sells as i32,
            min_trade_amount: p.min_trade_amount,
            whole_shares_only: p.whole_shares_only as i32,
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
            base_currency: db.base_currency,
            trigger_type: TriggerType::try_from(db.trigger_type.as_str())?,
            drift_band_bps: db.drift_band_bps,
            review_frequency: db
                .review_frequency
                .as_deref()
                .map(ReviewFrequency::try_from)
                .transpose()?,
            next_review_date: db.next_review_date,
            rebalance_to: RebalanceTo::try_from(db.rebalance_to.as_str())?,
            allow_sells: db.allow_sells != 0,
            min_trade_amount: db.min_trade_amount,
            whole_shares_only: db.whole_shares_only != 0,
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
