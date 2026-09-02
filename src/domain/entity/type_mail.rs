use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::EventIntervalUnit;
use super::EventIntervalKind;
use super::EventNotificationChannel;
use super::AuditMetadata;

/// Strongly-typed ID for TypeMail
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TypeMailId(pub Uuid);

impl TypeMailId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for TypeMailId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TypeMailId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for TypeMailId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<TypeMailId> for Uuid {
    fn from(id: TypeMailId) -> Self { id.0 }
}

impl AsRef<Uuid> for TypeMailId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for TypeMailId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TypeMail {
    pub id: Uuid,
    pub event_type_id: Uuid,
    pub interval_nbr: i32,
    pub interval_unit: EventIntervalUnit,
    pub interval_kind: EventIntervalKind,
    pub notification_channel: EventNotificationChannel,
    pub template_ref: Option<Uuid>,
    pub template_kind: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl TypeMail {
    /// Create a builder for TypeMail
    pub fn builder() -> TypeMailBuilder {
        <TypeMailBuilder as Default>::default()
    }

    /// Create a new TypeMail with required fields
    pub fn new(event_type_id: Uuid, interval_nbr: i32, interval_unit: EventIntervalUnit, interval_kind: EventIntervalKind, notification_channel: EventNotificationChannel) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type_id,
            interval_nbr,
            interval_unit,
            interval_kind,
            notification_channel,
            template_ref: None,
            template_kind: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> TypeMailId {
        TypeMailId(self.id)
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

    /// Set the template_ref field (chainable)
    pub fn with_template_ref(mut self, value: Uuid) -> Self {
        self.template_ref = Some(value);
        self
    }

    /// Set the template_kind field (chainable)
    pub fn with_template_kind(mut self, value: String) -> Self {
        self.template_kind = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "event_type_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.event_type_id = v; }
                }
                "interval_nbr" => {
                    if let Ok(v) = serde_json::from_value(value) { self.interval_nbr = v; }
                }
                "interval_unit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.interval_unit = v; }
                }
                "interval_kind" => {
                    if let Ok(v) = serde_json::from_value(value) { self.interval_kind = v; }
                }
                "notification_channel" => {
                    if let Ok(v) = serde_json::from_value(value) { self.notification_channel = v; }
                }
                "template_ref" => {
                    if let Ok(v) = serde_json::from_value(value) { self.template_ref = v; }
                }
                "template_kind" => {
                    if let Ok(v) = serde_json::from_value(value) { self.template_kind = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for TypeMail {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "TypeMail"
    }
}

impl backbone_core::PersistentEntity for TypeMail {
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

impl backbone_orm::EntityRepoMeta for TypeMail {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("event_type_id".to_string(), "uuid".to_string());
        m.insert("interval_unit".to_string(), "event_interval_unit".to_string());
        m.insert("interval_kind".to_string(), "event_interval_kind".to_string());
        m.insert("notification_channel".to_string(), "event_notification_channel".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("eventType", "types", "eventTypeId")]
    }
}

/// Builder for TypeMail entity
///
/// Provides a fluent API for constructing TypeMail instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct TypeMailBuilder {
    event_type_id: Option<Uuid>,
    interval_nbr: Option<i32>,
    interval_unit: Option<EventIntervalUnit>,
    interval_kind: Option<EventIntervalKind>,
    notification_channel: Option<EventNotificationChannel>,
    template_ref: Option<Uuid>,
    template_kind: Option<String>,
}

impl TypeMailBuilder {
    /// Set the event_type_id field (required)
    pub fn event_type_id(mut self, value: Uuid) -> Self {
        self.event_type_id = Some(value);
        self
    }

    /// Set the interval_nbr field (default: `1`)
    pub fn interval_nbr(mut self, value: i32) -> Self {
        self.interval_nbr = Some(value);
        self
    }

    /// Set the interval_unit field (default: `Utc::now()`)
    pub fn interval_unit(mut self, value: EventIntervalUnit) -> Self {
        self.interval_unit = Some(value);
        self
    }

    /// Set the interval_kind field (default: `EventIntervalKind::default()`)
    pub fn interval_kind(mut self, value: EventIntervalKind) -> Self {
        self.interval_kind = Some(value);
        self
    }

    /// Set the notification_channel field (default: `EventNotificationChannel::default()`)
    pub fn notification_channel(mut self, value: EventNotificationChannel) -> Self {
        self.notification_channel = Some(value);
        self
    }

    /// Set the template_ref field (optional)
    pub fn template_ref(mut self, value: Uuid) -> Self {
        self.template_ref = Some(value);
        self
    }

    /// Set the template_kind field (optional)
    pub fn template_kind(mut self, value: String) -> Self {
        self.template_kind = Some(value);
        self
    }

    /// Build the TypeMail entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<TypeMail, String> {
        let event_type_id = self.event_type_id.ok_or_else(|| "event_type_id is required".to_string())?;

        Ok(TypeMail {
            id: Uuid::new_v4(),
            event_type_id,
            interval_nbr: self.interval_nbr.unwrap_or(1),
            interval_unit: self.interval_unit.unwrap_or(EventIntervalUnit::Now),
            interval_kind: self.interval_kind.unwrap_or_default(),
            notification_channel: self.notification_channel.unwrap_or_default(),
            template_ref: self.template_ref,
            template_kind: self.template_kind,
            metadata: AuditMetadata::default(),
        })
    }
}
