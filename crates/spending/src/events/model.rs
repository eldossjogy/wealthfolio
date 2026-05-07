use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventType {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewEventType {
    pub id: Option<String>,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub event_type_id: String,
    /// RFC3339 string
    pub start_date: String,
    pub end_date: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewEvent {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub event_type_id: String,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEvent {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub event_type_id: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

/// Event with its event_type's name joined in. Returned by `get_events_with_names`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventWithTypeName {
    #[serde(flatten)]
    pub event: Event,
    pub event_type_name: String,
}
