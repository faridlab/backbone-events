use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use super::AuditMetadata;

/// Strongly-typed ID for Slot
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SlotId(pub Uuid);

impl SlotId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for SlotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for SlotId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for SlotId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<SlotId> for Uuid {
    fn from(id: SlotId) -> Self { id.0 }
}

impl AsRef<Uuid> for SlotId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for SlotId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Slot {
    pub id: Uuid,
    pub event_id: Uuid,
    pub name: String,
    pub date_begin: DateTime<Utc>,
    pub date_end: Option<DateTime<Utc>>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Slot {
    /// Create a builder for Slot
    pub fn builder() -> SlotBuilder {
        <SlotBuilder as Default>::default()
    }

    /// Create a new Slot with required fields
    pub fn new(event_id: Uuid, name: String, date_begin: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_id,
            name,
            date_begin,
            date_end: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> SlotId {
        SlotId(self.id)
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

    /// Set the date_end field (chainable)
    pub fn with_date_end(mut self, value: DateTime<Utc>) -> Self {
        self.date_end = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "event_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.event_id = v; }
                }
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "date_begin" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date_begin = v; }
                }
                "date_end" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date_end = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Slot {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Slot"
    }
}

impl backbone_core::PersistentEntity for Slot {
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

impl backbone_orm::EntityRepoMeta for Slot {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("event_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("event", "events", "eventId")]
    }
}

/// Builder for Slot entity
///
/// Provides a fluent API for constructing Slot instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct SlotBuilder {
    event_id: Option<Uuid>,
    name: Option<String>,
    date_begin: Option<DateTime<Utc>>,
    date_end: Option<DateTime<Utc>>,
}

impl SlotBuilder {
    /// Set the event_id field (required)
    pub fn event_id(mut self, value: Uuid) -> Self {
        self.event_id = Some(value);
        self
    }

    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the date_begin field (required)
    pub fn date_begin(mut self, value: DateTime<Utc>) -> Self {
        self.date_begin = Some(value);
        self
    }

    /// Set the date_end field (optional)
    pub fn date_end(mut self, value: DateTime<Utc>) -> Self {
        self.date_end = Some(value);
        self
    }

    /// Build the Slot entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Slot, String> {
        let event_id = self.event_id.ok_or_else(|| "event_id is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let date_begin = self.date_begin.ok_or_else(|| "date_begin is required".to_string())?;

        Ok(Slot {
            id: Uuid::new_v4(),
            event_id,
            name,
            date_begin,
            date_end: self.date_end,
            metadata: AuditMetadata::default(),
        })
    }
}
