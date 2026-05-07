use anyhow::Result;
use async_trait::async_trait;

use super::model::{ActivityTaxonomyAssignment, NewActivityTaxonomyAssignment};

#[async_trait]
pub trait ActivityTaxonomyAssignmentRepositoryTrait: Send + Sync {
    /// All assignments for one activity.
    async fn list_for_activity(&self, activity_id: &str)
        -> Result<Vec<ActivityTaxonomyAssignment>>;

    /// All assignments for a batch of activities. Returns rows in arbitrary order;
    /// caller is responsible for grouping by `activity_id`. Used to avoid N+1 fetches
    /// from the cash-activity search endpoint.
    async fn list_for_activities(
        &self,
        activity_ids: &[String],
    ) -> Result<Vec<ActivityTaxonomyAssignment>>;

    /// Create or replace (for single-select taxonomies) the assignment.
    async fn upsert(
        &self,
        new_assignment: NewActivityTaxonomyAssignment,
    ) -> Result<ActivityTaxonomyAssignment>;

    /// Remove a single assignment by id.
    async fn delete(&self, id: &str) -> Result<()>;

    /// Remove all assignments tying `activity_id` to `taxonomy_id`.
    /// Used to clear a single-select taxonomy.
    async fn clear_for_taxonomy(&self, activity_id: &str, taxonomy_id: &str) -> Result<()>;
}
