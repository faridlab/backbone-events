use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::EventKanbanState;
use super::EventBadgeFormat;
use super::AuditMetadata;

/// Strongly-typed ID for Event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(pub Uuid);

impl EventId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for EventId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for EventId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<EventId> for Uuid {
    fn from(id: EventId) -> Self { id.0 }
}

impl AsRef<Uuid> for EventId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for EventId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Event {
    pub id: Uuid,
    pub name: String,
    pub event_type_id: Option<Uuid>,
    pub stage_id: Uuid,
    pub kanban_state: EventKanbanState,
    pub date_begin: DateTime<Utc>,
    pub date_end: DateTime<Utc>,
    pub date_tz: String,
    pub is_multi_slots: bool,
    pub event_slot_count: i32,
    pub seats_limited: bool,
    pub seats_max: i32,
    pub address_id: Option<Uuid>,
    pub organizer_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub event_url: Option<String>,
    pub badge_format: EventBadgeFormat,
    pub is_published: bool,
    pub date_publish: Option<DateTime<Utc>>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Event {
    /// Create a builder for Event
    pub fn builder() -> EventBuilder {
        <EventBuilder as Default>::default()
    }

    /// Create a new Event with required fields
    pub fn new(name: String, stage_id: Uuid, kanban_state: EventKanbanState, date_begin: DateTime<Utc>, date_end: DateTime<Utc>, date_tz: String, is_multi_slots: bool, event_slot_count: i32, seats_limited: bool, seats_max: i32, badge_format: EventBadgeFormat, is_published: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            event_type_id: None,
            stage_id,
            kanban_state,
            date_begin,
            date_end,
            date_tz,
            is_multi_slots,
            event_slot_count,
            seats_limited,
            seats_max,
            address_id: None,
            organizer_id: None,
            user_id: None,
            event_url: None,
            badge_format,
            is_published,
            date_publish: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> EventId {
        EventId(self.id)
    }

    /// Get when this entity was created
    pub fn created_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.created_at.as_ref()
    }

    /// Get when this entity was last updated
    pub fn updated_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.updated_at.as_ref()
    }

    /// Check if this entity is soft deleted
    pub fn is_deleted(&self) -> bool {
        self.metadata.deleted_at.is_some()
    }

    /// Check if this entity is active (not deleted)
    pub fn is_active(&self) -> bool {
        self.metadata.deleted_at.is_none()
    }

    /// Get when this entity was deleted
    pub fn deleted_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.deleted_at.as_ref()
    }

    /// Get who created this entity
    pub fn created_by(&self) -> Option<&Uuid> {
        self.metadata.created_by.as_ref()
    }

    /// Get who last updated this entity
    pub fn updated_by(&self) -> Option<&Uuid> {
        self.metadata.updated_by.as_ref()
    }

    /// Get who deleted this entity
    pub fn deleted_by(&self) -> Option<&Uuid> {
        self.metadata.deleted_by.as_ref()
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the event_type_id field (chainable)
    pub fn with_event_type_id(mut self, value: Uuid) -> Self {
        self.event_type_id = Some(value);
        self
    }

    /// Set the address_id field (chainable)
    pub fn with_address_id(mut self, value: Uuid) -> Self {
        self.address_id = Some(value);
        self
    }

    /// Set the organizer_id field (chainable)
    pub fn with_organizer_id(mut self, value: Uuid) -> Self {
        self.organizer_id = Some(value);
        self
    }

    /// Set the user_id field (chainable)
    pub fn with_user_id(mut self, value: Uuid) -> Self {
        self.user_id = Some(value);
        self
    }

    /// Set the event_url field (chainable)
    pub fn with_event_url(mut self, value: String) -> Self {
        self.event_url = Some(value);
        self
    }

    /// Set the date_publish field (chainable)
    pub fn with_date_publish(mut self, value: DateTime<Utc>) -> Self {
        self.date_publish = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "event_type_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.event_type_id = v; }
                }
                "stage_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.stage_id = v; }
                }
                "kanban_state" => {
                    if let Ok(v) = serde_json::from_value(value) { self.kanban_state = v; }
                }
                "date_begin" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date_begin = v; }
                }
                "date_end" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date_end = v; }
                }
                "date_tz" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date_tz = v; }
                }
                "is_multi_slots" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_multi_slots = v; }
                }
                "event_slot_count" => {
                    if let Ok(v) = serde_json::from_value(value) { self.event_slot_count = v; }
                }
                "seats_limited" => {
                    if let Ok(v) = serde_json::from_value(value) { self.seats_limited = v; }
                }
                "seats_max" => {
                    if let Ok(v) = serde_json::from_value(value) { self.seats_max = v; }
                }
                "address_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.address_id = v; }
                }
                "organizer_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.organizer_id = v; }
                }
                "user_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.user_id = v; }
                }
                "event_url" => {
                    if let Ok(v) = serde_json::from_value(value) { self.event_url = v; }
                }
                "badge_format" => {
                    if let Ok(v) = serde_json::from_value(value) { self.badge_format = v; }
                }
                "is_published" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_published = v; }
                }
                "date_publish" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date_publish = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Event {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Event"
    }
}

impl backbone_core::PersistentEntity for Event {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.created_at
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.created_at = Some(ts);
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.updated_at
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.updated_at = Some(ts);
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.deleted_at
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        self.metadata.deleted_at = ts;
    }
}

impl backbone_orm::EntityRepoMeta for Event {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("event_type_id".to_string(), "uuid".to_string());
        m.insert("stage_id".to_string(), "uuid".to_string());
        m.insert("address_id".to_string(), "uuid".to_string());
        m.insert("organizer_id".to_string(), "uuid".to_string());
        m.insert("user_id".to_string(), "uuid".to_string());
        m.insert("kanban_state".to_string(), "event_kanban_state".to_string());
        m.insert("badge_format".to_string(), "event_badge_format".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name", "date_tz"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("eventType", "types", "eventTypeId"), ("stage", "stages", "stageId")]
    }
}

/// Builder for Event entity
///
/// Provides a fluent API for constructing Event instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct EventBuilder {
    name: Option<String>,
    event_type_id: Option<Uuid>,
    stage_id: Option<Uuid>,
    kanban_state: Option<EventKanbanState>,
    date_begin: Option<DateTime<Utc>>,
    date_end: Option<DateTime<Utc>>,
    date_tz: Option<String>,
    is_multi_slots: Option<bool>,
    event_slot_count: Option<i32>,
    seats_limited: Option<bool>,
    seats_max: Option<i32>,
    address_id: Option<Uuid>,
    organizer_id: Option<Uuid>,
    user_id: Option<Uuid>,
    event_url: Option<String>,
    badge_format: Option<EventBadgeFormat>,
    is_published: Option<bool>,
    date_publish: Option<DateTime<Utc>>,
}

impl EventBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the event_type_id field (optional)
    pub fn event_type_id(mut self, value: Uuid) -> Self {
        self.event_type_id = Some(value);
        self
    }

    /// Set the stage_id field (required)
    pub fn stage_id(mut self, value: Uuid) -> Self {
        self.stage_id = Some(value);
        self
    }

    /// Set the kanban_state field (default: `EventKanbanState::default()`)
    pub fn kanban_state(mut self, value: EventKanbanState) -> Self {
        self.kanban_state = Some(value);
        self
    }

    /// Set the date_begin field (required)
    pub fn date_begin(mut self, value: DateTime<Utc>) -> Self {
        self.date_begin = Some(value);
        self
    }

    /// Set the date_end field (required)
    pub fn date_end(mut self, value: DateTime<Utc>) -> Self {
        self.date_end = Some(value);
        self
    }

    /// Set the date_tz field (default: `Default::default()`)
    pub fn date_tz(mut self, value: String) -> Self {
        self.date_tz = Some(value);
        self
    }

    /// Set the is_multi_slots field (default: `false`)
    pub fn is_multi_slots(mut self, value: bool) -> Self {
        self.is_multi_slots = Some(value);
        self
    }

    /// Set the event_slot_count field (default: `1`)
    pub fn event_slot_count(mut self, value: i32) -> Self {
        self.event_slot_count = Some(value);
        self
    }

    /// Set the seats_limited field (default: `false`)
    pub fn seats_limited(mut self, value: bool) -> Self {
        self.seats_limited = Some(value);
        self
    }

    /// Set the seats_max field (default: `0`)
    pub fn seats_max(mut self, value: i32) -> Self {
        self.seats_max = Some(value);
        self
    }

    /// Set the address_id field (optional)
    pub fn address_id(mut self, value: Uuid) -> Self {
        self.address_id = Some(value);
        self
    }

    /// Set the organizer_id field (optional)
    pub fn organizer_id(mut self, value: Uuid) -> Self {
        self.organizer_id = Some(value);
        self
    }

    /// Set the user_id field (optional)
    pub fn user_id(mut self, value: Uuid) -> Self {
        self.user_id = Some(value);
        self
    }

    /// Set the event_url field (optional)
    pub fn event_url(mut self, value: String) -> Self {
        self.event_url = Some(value);
        self
    }

    /// Set the badge_format field (default: `EventBadgeFormat::default()`)
    pub fn badge_format(mut self, value: EventBadgeFormat) -> Self {
        self.badge_format = Some(value);
        self
    }

    /// Set the is_published field (default: `false`)
    pub fn is_published(mut self, value: bool) -> Self {
        self.is_published = Some(value);
        self
    }

    /// Set the date_publish field (optional)
    pub fn date_publish(mut self, value: DateTime<Utc>) -> Self {
        self.date_publish = Some(value);
        self
    }

    /// Build the Event entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Event, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let stage_id = self.stage_id.ok_or_else(|| "stage_id is required".to_string())?;
        let date_begin = self.date_begin.ok_or_else(|| "date_begin is required".to_string())?;
        let date_end = self.date_end.ok_or_else(|| "date_end is required".to_string())?;

        Ok(Event {
            id: Uuid::new_v4(),
            name,
            event_type_id: self.event_type_id,
            stage_id,
            kanban_state: self.kanban_state.unwrap_or_default(),
            date_begin,
            date_end,
            date_tz: self.date_tz.unwrap_or_default(),
            is_multi_slots: self.is_multi_slots.unwrap_or(false),
            event_slot_count: self.event_slot_count.unwrap_or(1),
            seats_limited: self.seats_limited.unwrap_or(false),
            seats_max: self.seats_max.unwrap_or(0),
            address_id: self.address_id,
            organizer_id: self.organizer_id,
            user_id: self.user_id,
            event_url: self.event_url,
            badge_format: self.badge_format.unwrap_or_default(),
            is_published: self.is_published.unwrap_or(false),
            date_publish: self.date_publish,
            metadata: AuditMetadata::default(),
        })
    }
}
