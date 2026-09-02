use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use super::AuditMetadata;

/// Strongly-typed ID for MailSlot
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MailSlotId(pub Uuid);

impl MailSlotId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for MailSlotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for MailSlotId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for MailSlotId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<MailSlotId> for Uuid {
    fn from(id: MailSlotId) -> Self { id.0 }
}

impl AsRef<Uuid> for MailSlotId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for MailSlotId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MailSlot {
    pub id: Uuid,
    pub scheduler_id: Uuid,
    pub slot_id: Uuid,
    pub scheduled_date: Option<DateTime<Utc>>,
    pub mail_done: bool,
    pub last_registration_id: Option<Uuid>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl MailSlot {
    /// Create a builder for MailSlot
    pub fn builder() -> MailSlotBuilder {
        <MailSlotBuilder as Default>::default()
    }

    /// Create a new MailSlot with required fields
    pub fn new(scheduler_id: Uuid, slot_id: Uuid, mail_done: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            scheduler_id,
            slot_id,
            scheduled_date: None,
            mail_done,
            last_registration_id: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> MailSlotId {
        MailSlotId(self.id)
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

    /// Set the scheduled_date field (chainable)
    pub fn with_scheduled_date(mut self, value: DateTime<Utc>) -> Self {
        self.scheduled_date = Some(value);
        self
    }

    /// Set the last_registration_id field (chainable)
    pub fn with_last_registration_id(mut self, value: Uuid) -> Self {
        self.last_registration_id = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "scheduler_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.scheduler_id = v; }
                }
                "slot_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.slot_id = v; }
                }
                "scheduled_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.scheduled_date = v; }
                }
                "mail_done" => {
                    if let Ok(v) = serde_json::from_value(value) { self.mail_done = v; }
                }
                "last_registration_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.last_registration_id = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for MailSlot {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "MailSlot"
    }
}

impl backbone_core::PersistentEntity for MailSlot {
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

impl backbone_orm::EntityRepoMeta for MailSlot {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("scheduler_id".to_string(), "uuid".to_string());
        m.insert("slot_id".to_string(), "uuid".to_string());
        m.insert("last_registration_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("scheduler", "mails", "schedulerId"), ("slot", "slots", "slotId")]
    }
}

/// Builder for MailSlot entity
///
/// Provides a fluent API for constructing MailSlot instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct MailSlotBuilder {
    scheduler_id: Option<Uuid>,
    slot_id: Option<Uuid>,
    scheduled_date: Option<DateTime<Utc>>,
    mail_done: Option<bool>,
    last_registration_id: Option<Uuid>,
}

impl MailSlotBuilder {
    /// Set the scheduler_id field (required)
    pub fn scheduler_id(mut self, value: Uuid) -> Self {
        self.scheduler_id = Some(value);
        self
    }

    /// Set the slot_id field (required)
    pub fn slot_id(mut self, value: Uuid) -> Self {
        self.slot_id = Some(value);
        self
    }

    /// Set the scheduled_date field (optional)
    pub fn scheduled_date(mut self, value: DateTime<Utc>) -> Self {
        self.scheduled_date = Some(value);
        self
    }

    /// Set the mail_done field (default: `false`)
    pub fn mail_done(mut self, value: bool) -> Self {
        self.mail_done = Some(value);
        self
    }

    /// Set the last_registration_id field (optional)
    pub fn last_registration_id(mut self, value: Uuid) -> Self {
        self.last_registration_id = Some(value);
        self
    }

    /// Build the MailSlot entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<MailSlot, String> {
        let scheduler_id = self.scheduler_id.ok_or_else(|| "scheduler_id is required".to_string())?;
        let slot_id = self.slot_id.ok_or_else(|| "slot_id is required".to_string())?;

        Ok(MailSlot {
            id: Uuid::new_v4(),
            scheduler_id,
            slot_id,
            scheduled_date: self.scheduled_date,
            mail_done: self.mail_done.unwrap_or(false),
            last_registration_id: self.last_registration_id,
            metadata: AuditMetadata::default(),
        })
    }
}
