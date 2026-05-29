use async_trait::async_trait;
use log::debug;
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::{Error as CoreError, Result as CoreResult, ValidationError};
use crate::taxonomies::TaxonomyServiceTrait;

use super::model::{
    NewTargetAllocationNode, NewTargetProfile, ProfileStatus, TargetAllocationNode, TargetProfile,
};
use super::validation::{validate_new_profile, validate_nodes_sum};

// ── Repository trait ─────────────────────────────────────────────────────────

#[async_trait]
pub trait TargetProfileRepositoryTrait: Send + Sync {
    fn get_profile(&self, id: &str) -> CoreResult<Option<TargetProfile>>;
    fn list_profiles(&self) -> CoreResult<Vec<TargetProfile>>;
    fn get_active_profile_for_scope(
        &self,
        scope_type: &str,
        scope_id: Option<&str>,
    ) -> CoreResult<Option<TargetProfile>>;
    fn list_nodes_for_profile(&self, profile_id: &str) -> CoreResult<Vec<TargetAllocationNode>>;

    async fn create_profile(&self, profile: TargetProfile) -> CoreResult<TargetProfile>;
    async fn update_profile(&self, profile: TargetProfile) -> CoreResult<TargetProfile>;
    async fn delete_profile(&self, id: &str) -> CoreResult<usize>;
    async fn save_nodes(
        &self,
        profile_id: &str,
        nodes: Vec<TargetAllocationNode>,
    ) -> CoreResult<Vec<TargetAllocationNode>>;
}

// ── Service trait ─────────────────────────────────────────────────────────────

#[async_trait]
pub trait TargetProfileServiceTrait: Send + Sync {
    fn get_profile(&self, id: &str) -> CoreResult<Option<TargetProfile>>;
    fn list_profiles(&self) -> CoreResult<Vec<TargetProfile>>;
    fn get_active_profile_for_scope(
        &self,
        scope_type: &str,
        scope_id: Option<&str>,
    ) -> CoreResult<Option<TargetProfile>>;
    fn list_nodes_for_profile(&self, profile_id: &str) -> CoreResult<Vec<TargetAllocationNode>>;

    async fn create_profile(&self, input: NewTargetProfile) -> CoreResult<TargetProfile>;
    async fn update_profile(&self, id: &str, input: NewTargetProfile) -> CoreResult<TargetProfile>;
    async fn activate_profile(&self, id: &str) -> CoreResult<TargetProfile>;
    async fn archive_profile(&self, id: &str) -> CoreResult<TargetProfile>;
    async fn delete_profile(&self, id: &str) -> CoreResult<()>;
    async fn save_nodes(
        &self,
        profile_id: &str,
        nodes: Vec<NewTargetAllocationNode>,
    ) -> CoreResult<Vec<TargetAllocationNode>>;
}

// ── Implementation ────────────────────────────────────────────────────────────

pub struct TargetProfileService {
    repository: Arc<dyn TargetProfileRepositoryTrait>,
    taxonomy_service: Arc<dyn TaxonomyServiceTrait>,
}

impl TargetProfileService {
    pub fn new(
        repository: Arc<dyn TargetProfileRepositoryTrait>,
        taxonomy_service: Arc<dyn TaxonomyServiceTrait>,
    ) -> Self {
        Self {
            repository,
            taxonomy_service,
        }
    }

    fn now() -> String {
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
    }
}

#[async_trait]
impl TargetProfileServiceTrait for TargetProfileService {
    fn get_profile(&self, id: &str) -> CoreResult<Option<TargetProfile>> {
        self.repository.get_profile(id)
    }

    fn list_profiles(&self) -> CoreResult<Vec<TargetProfile>> {
        self.repository.list_profiles()
    }

    fn get_active_profile_for_scope(
        &self,
        scope_type: &str,
        scope_id: Option<&str>,
    ) -> CoreResult<Option<TargetProfile>> {
        self.repository
            .get_active_profile_for_scope(scope_type, scope_id)
    }

    fn list_nodes_for_profile(&self, profile_id: &str) -> CoreResult<Vec<TargetAllocationNode>> {
        self.repository.list_nodes_for_profile(profile_id)
    }

    async fn create_profile(&self, input: NewTargetProfile) -> CoreResult<TargetProfile> {
        validate_new_profile(&input)?;
        debug!("Creating target profile: {}", input.name);
        let now = Self::now();
        let profile = TargetProfile {
            id: Uuid::new_v4().to_string(),
            name: input.name.trim().to_string(),
            status: ProfileStatus::Draft,
            scope_type: input.scope_type,
            scope_id: input.scope_id,
            taxonomy_id: input.taxonomy_id,
            trigger_type: input.trigger_type,
            drift_band_bps: input.drift_band_bps,
            created_at: now.clone(),
            updated_at: now,
        };
        self.repository.create_profile(profile).await
    }

    async fn update_profile(&self, id: &str, input: NewTargetProfile) -> CoreResult<TargetProfile> {
        validate_new_profile(&input)?;
        let existing = self.repository.get_profile(id)?.ok_or_else(|| {
            crate::errors::Error::Database(crate::errors::DatabaseError::NotFound(format!(
                "TargetProfile {} not found",
                id
            )))
        })?;
        debug!("Updating target profile: {}", id);
        let updated = TargetProfile {
            id: existing.id,
            name: input.name.trim().to_string(),
            status: existing.status,
            scope_type: input.scope_type,
            scope_id: input.scope_id,
            taxonomy_id: input.taxonomy_id,
            trigger_type: input.trigger_type,
            drift_band_bps: input.drift_band_bps,
            created_at: existing.created_at,
            updated_at: Self::now(),
        };
        self.repository.update_profile(updated).await
    }

    async fn activate_profile(&self, id: &str) -> CoreResult<TargetProfile> {
        let existing = self.repository.get_profile(id)?.ok_or_else(|| {
            crate::errors::Error::Database(crate::errors::DatabaseError::NotFound(format!(
                "TargetProfile {} not found",
                id
            )))
        })?;
        debug!("Activating target profile: {}", id);

        // Archive any other active profile for the same scope before activating this one.
        let scope_type = existing.scope_type.as_str();
        let scope_id = existing.scope_id.as_deref();
        if let Some(currently_active) = self
            .repository
            .get_active_profile_for_scope(scope_type, scope_id)?
        {
            if currently_active.id != id {
                let archived = TargetProfile {
                    status: ProfileStatus::Archived,
                    updated_at: Self::now(),
                    ..currently_active
                };
                self.repository.update_profile(archived).await?;
            }
        }

        let updated = TargetProfile {
            status: ProfileStatus::Active,
            updated_at: Self::now(),
            ..existing
        };
        self.repository.update_profile(updated).await
    }

    async fn archive_profile(&self, id: &str) -> CoreResult<TargetProfile> {
        let existing = self.repository.get_profile(id)?.ok_or_else(|| {
            crate::errors::Error::Database(crate::errors::DatabaseError::NotFound(format!(
                "TargetProfile {} not found",
                id
            )))
        })?;
        debug!("Archiving target profile: {}", id);
        let updated = TargetProfile {
            status: ProfileStatus::Archived,
            updated_at: Self::now(),
            ..existing
        };
        self.repository.update_profile(updated).await
    }

    async fn delete_profile(&self, id: &str) -> CoreResult<()> {
        debug!("Deleting target profile: {}", id);
        self.repository.delete_profile(id).await?;
        Ok(())
    }

    async fn save_nodes(
        &self,
        profile_id: &str,
        nodes: Vec<NewTargetAllocationNode>,
    ) -> CoreResult<Vec<TargetAllocationNode>> {
        validate_nodes_sum(&nodes)?;

        let profile = self.repository.get_profile(profile_id)?.ok_or_else(|| {
            CoreError::Database(crate::errors::DatabaseError::NotFound(format!(
                "TargetProfile {} not found",
                profile_id
            )))
        })?;
        if let Some(taxonomy) = self.taxonomy_service.get_taxonomy(&profile.taxonomy_id)? {
            let valid_ids: std::collections::HashSet<&str> =
                taxonomy.categories.iter().map(|c| c.id.as_str()).collect();
            for node in &nodes {
                if !valid_ids.contains(node.category_id.as_str()) {
                    return Err(CoreError::Validation(ValidationError::InvalidInput(
                        format!(
                            "category_id '{}' does not belong to taxonomy '{}'",
                            node.category_id, profile.taxonomy_id
                        ),
                    )));
                }
            }
        }

        debug!("Saving {} nodes for profile {}", nodes.len(), profile_id);
        let now = Self::now();
        let domain_nodes: Vec<TargetAllocationNode> = nodes
            .into_iter()
            .map(|n| TargetAllocationNode {
                id: Uuid::new_v4().to_string(),
                profile_id: profile_id.to_string(),
                category_id: n.category_id,
                target_bps: n.target_bps,
                is_locked: n.is_locked,
                is_required: n.is_required,
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .collect();
        self.repository.save_nodes(profile_id, domain_nodes).await
    }
}
