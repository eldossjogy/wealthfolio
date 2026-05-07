use std::sync::Arc;

use anyhow::Result;

use super::model::{ActivityTaxonomyAssignment, NewActivityTaxonomyAssignment};
use super::traits::ActivityTaxonomyAssignmentRepositoryTrait;

pub struct ActivityTaxonomyAssignmentService {
    repo: Arc<dyn ActivityTaxonomyAssignmentRepositoryTrait>,
}

impl ActivityTaxonomyAssignmentService {
    pub fn new(repo: Arc<dyn ActivityTaxonomyAssignmentRepositoryTrait>) -> Self {
        Self { repo }
    }

    pub async fn list_for_activity(
        &self,
        activity_id: &str,
    ) -> Result<Vec<ActivityTaxonomyAssignment>> {
        self.repo.list_for_activity(activity_id).await
    }

    pub async fn list_for_activities(
        &self,
        activity_ids: &[String],
    ) -> Result<Vec<ActivityTaxonomyAssignment>> {
        self.repo.list_for_activities(activity_ids).await
    }

    /// Set the (single) category for `activity_id` in `taxonomy_id`.
    /// Clears any prior assignments tying that activity to that taxonomy.
    pub async fn assign_single(
        &self,
        activity_id: &str,
        taxonomy_id: &str,
        category_id: &str,
    ) -> Result<ActivityTaxonomyAssignment> {
        self.repo
            .clear_for_taxonomy(activity_id, taxonomy_id)
            .await?;
        self.repo
            .upsert(NewActivityTaxonomyAssignment {
                id: None,
                activity_id: activity_id.to_string(),
                taxonomy_id: taxonomy_id.to_string(),
                category_id: category_id.to_string(),
                weight: 10_000,
                source: "manual".to_string(),
            })
            .await
    }

    pub async fn unassign(&self, activity_id: &str, taxonomy_id: &str) -> Result<()> {
        self.repo.clear_for_taxonomy(activity_id, taxonomy_id).await
    }

    /// Direct upsert (used by Activity Rules to apply rule-sourced assignments).
    pub async fn upsert(
        &self,
        new_assignment: NewActivityTaxonomyAssignment,
    ) -> Result<ActivityTaxonomyAssignment> {
        self.repo.upsert(new_assignment).await
    }
}
