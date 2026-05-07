use anyhow::Result;
use async_trait::async_trait;

use super::model::{Event, EventType, NewEvent, NewEventType, UpdateEvent};

#[async_trait]
pub trait EventTypesRepositoryTrait: Send + Sync {
    async fn list(&self) -> Result<Vec<EventType>>;
    async fn create(&self, new_type: NewEventType) -> Result<EventType>;
    async fn update(
        &self,
        id: &str,
        name: Option<String>,
        color: Option<Option<String>>,
    ) -> Result<EventType>;
    async fn delete(&self, id: &str) -> Result<()>;
}

#[async_trait]
pub trait EventsRepositoryTrait: Send + Sync {
    async fn list(&self) -> Result<Vec<Event>>;
    async fn get(&self, id: &str) -> Result<Option<Event>>;
    async fn create(&self, new_event: NewEvent) -> Result<Event>;
    async fn update(&self, id: &str, patch: UpdateEvent) -> Result<Event>;
    async fn delete(&self, id: &str) -> Result<()>;
}
