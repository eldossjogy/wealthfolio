use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;

use super::model::ActivityEvent;

/// Repository for the `activity_events` join table.
///
/// Reads (`list_for_activities`, `list_for_event`) feed analytics / cash-activity
/// services that need to know which activities are tagged to which events.
/// Writes are routed via `ActivityRepositoryTrait::set_activity_event_id` so
/// the activity row's `updated_at` bumps in the same transaction and device
/// sync picks both changes up atomically.
#[async_trait]
pub trait ActivityEventsRepositoryTrait: Send + Sync {
    /// Returns activity_id → event_id for the requested activity ids.
    /// Untagged activities are absent from the map.
    async fn list_for_activities(&self, ids: &[String]) -> Result<HashMap<String, String>>;

    /// Activity ids currently tagged to a given event.
    async fn list_for_event(&self, event_id: &str) -> Result<Vec<String>>;

    /// Bulk delete by event id. Used when an event is deleted to untag all
    /// its activities atomically. Returns the number of rows removed.
    async fn delete_by_event(&self, event_id: &str) -> Result<usize>;

    /// Full table read — used by device-sync snapshotting.
    async fn list_all(&self) -> Result<Vec<ActivityEvent>>;
}
