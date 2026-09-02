use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use super::AuditMetadata;

/// Strongly-typed ID for EventType
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventTypeId(pub Uuid);

impl EventTypeId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for EventTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for EventTypeId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for EventTypeId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<EventTypeId> for Uuid {
    fn from(id: EventTypeId) -> Self { id.0 }
}

impl AsRef<Uuid> for EventTypeId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for EventTypeId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EventType {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub seats_limited: bool,
    pub seats_max: i32,
    pub default_date_tz: String,
    pub is_multi_slots: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl EventType {
    /// Create a builder for EventType
    pub fn builder() -> EventTypeBuilder {
        <EventTypeBuilder as Default>::default()
    }

    /// Create a new EventType with required fields
    pub fn new(name: String, seats_limited: bool, seats_max: i32, default_date_tz: String, is_multi_slots: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description: None,
            seats_limited,
            seats_max,
            default_date_tz,
            is_multi_slots,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> EventTypeId {
        EventTypeId(self.id)
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

    /// Set the description field (chainable)
    pub fn with_description(mut self, value: String) -> Self {
        self.description = Some(value);
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
                "description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.description = v; }
                }
                "seats_limited" => {
                    if let Ok(v) = serde_json::from_value(value) { self.seats_limited = v; }
                }
                "seats_max" => {
                    if let Ok(v) = serde_json::from_value(value) { self.seats_max = v; }
                }
                "default_date_tz" => {
                    if let Ok(v) = serde_json::from_value(value) { self.default_date_tz = v; }
                }
                "is_multi_slots" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_multi_slots = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for EventType {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "EventType"
    }
}

impl backbone_core::PersistentEntity for EventType {
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

impl backbone_orm::EntityRepoMeta for EventType {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name", "default_date_tz"]
    }
}

/// Builder for EventType entity
///
/// Provides a fluent API for constructing EventType instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct EventTypeBuilder {
    name: Option<String>,
    description: Option<String>,
    seats_limited: Option<bool>,
    seats_max: Option<i32>,
    default_date_tz: Option<String>,
    is_multi_slots: Option<bool>,
}

impl EventTypeBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the description field (optional)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
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

    /// Set the default_date_tz field (default: `Default::default()`)
    pub fn default_date_tz(mut self, value: String) -> Self {
        self.default_date_tz = Some(value);
        self
    }

    /// Set the is_multi_slots field (default: `false`)
    pub fn is_multi_slots(mut self, value: bool) -> Self {
        self.is_multi_slots = Some(value);
        self
    }

    /// Build the EventType entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<EventType, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;

        Ok(EventType {
            id: Uuid::new_v4(),
            name,
            description: self.description,
            seats_limited: self.seats_limited.unwrap_or(false),
            seats_max: self.seats_max.unwrap_or(0),
            default_date_tz: self.default_date_tz.unwrap_or_default(),
            is_multi_slots: self.is_multi_slots.unwrap_or(false),
            metadata: AuditMetadata::default(),
        })
    }
}
