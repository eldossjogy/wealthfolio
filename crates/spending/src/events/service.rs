use std::sync::Arc;

use anyhow::Result;

use super::model::{Event, EventType, EventWithTypeName, NewEvent, NewEventType, UpdateEvent};
use super::traits::{EventTypesRepositoryTrait, EventsRepositoryTrait};

pub struct EventsService {
    types_repo: Arc<dyn EventTypesRepositoryTrait>,
    events_repo: Arc<dyn EventsRepositoryTrait>,
}

impl EventsService {
    pub fn new(
        types_repo: Arc<dyn EventTypesRepositoryTrait>,
        events_repo: Arc<dyn EventsRepositoryTrait>,
    ) -> Self {
        Self {
            types_repo,
            events_repo,
        }
    }

    pub async fn list_types(&self) -> Result<Vec<EventType>> {
        self.types_repo.list().await
    }
    pub async fn create_type(&self, new_type: NewEventType) -> Result<EventType> {
        self.types_repo.create(new_type).await
    }
    pub async fn update_type(
        &self,
        id: &str,
        name: Option<String>,
        color: Option<Option<String>>,
    ) -> Result<EventType> {
        self.types_repo.update(id, name, color).await
    }
    pub async fn delete_type(&self, id: &str) -> Result<()> {
        self.types_repo.delete(id).await
    }

    pub async fn list_events(&self) -> Result<Vec<Event>> {
        self.events_repo.list().await
    }

    /// List events joined with their event_type name for UI display.
    pub async fn list_events_with_names(&self) -> Result<Vec<EventWithTypeName>> {
        let types = self.types_repo.list().await?;
        let type_by_id: std::collections::HashMap<String, String> =
            types.into_iter().map(|t| (t.id, t.name)).collect();
        let events = self.events_repo.list().await?;
        Ok(events
            .into_iter()
            .map(|e| {
                let event_type_name = type_by_id
                    .get(&e.event_type_id)
                    .cloned()
                    .unwrap_or_else(|| e.event_type_id.clone());
                EventWithTypeName {
                    event: e,
                    event_type_name,
                }
            })
            .collect())
    }
    pub async fn get_event(&self, id: &str) -> Result<Option<Event>> {
        self.events_repo.get(id).await
    }
    pub async fn create_event(&self, new_event: NewEvent) -> Result<Event> {
        self.events_repo.create(new_event).await
    }
    pub async fn update_event(&self, id: &str, patch: UpdateEvent) -> Result<Event> {
        self.events_repo.update(id, patch).await
    }
    pub async fn delete_event(&self, id: &str) -> Result<()> {
        self.events_repo.delete(id).await
    }
}
