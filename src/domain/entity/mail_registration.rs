use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use super::AuditMetadata;

/// Strongly-typed ID for MailRegistration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MailRegistrationId(pub Uuid);

impl MailRegistrationId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for MailRegistrationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for MailRegistrationId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for MailRegistrationId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<MailRegistrationId> for Uuid {
    fn from(id: MailRegistrationId) -> Self { id.0 }
}

impl AsRef<Uuid> for MailRegistrationId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for MailRegistrationId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MailRegistration {
    pub id: Uuid,
    pub scheduler_id: Uuid,
    pub registration_id: Uuid,
    pub scheduled_date: Option<DateTime<Utc>>,
    pub mail_sent: bool,
    pub outcome: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl MailRegistration {
    /// Create a builder for MailRegistration
    pub fn builder() -> MailRegistrationBuilder {
        <MailRegistrationBuilder as Default>::default()
    }

    /// Create a new MailRegistration with required fields
    pub fn new(scheduler_id: Uuid, registration_id: Uuid, mail_sent: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            scheduler_id,
            registration_id,
            scheduled_date: None,
            mail_sent,
            outcome: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> MailRegistrationId {
        MailRegistrationId(self.id)
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

    /// Set the outcome field (chainable)
    pub fn with_outcome(mut self, value: String) -> Self {
        self.outcome = Some(value);
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
                "registration_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.registration_id = v; }
                }
                "scheduled_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.scheduled_date = v; }
                }
                "mail_sent" => {
                    if let Ok(v) = serde_json::from_value(value) { self.mail_sent = v; }
                }
                "outcome" => {
                    if let Ok(v) = serde_json::from_value(value) { self.outcome = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for MailRegistration {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "MailRegistration"
    }
}

impl backbone_core::PersistentEntity for MailRegistration {
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

impl backbone_orm::EntityRepoMeta for MailRegistration {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("scheduler_id".to_string(), "uuid".to_string());
        m.insert("registration_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("scheduler", "mails", "schedulerId"), ("registration", "registrations", "registrationId")]
    }
}

/// Builder for MailRegistration entity
///
/// Provides a fluent API for constructing MailRegistration instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct MailRegistrationBuilder {
    scheduler_id: Option<Uuid>,
    registration_id: Option<Uuid>,
    scheduled_date: Option<DateTime<Utc>>,
    mail_sent: Option<bool>,
    outcome: Option<String>,
}

impl MailRegistrationBuilder {
    /// Set the scheduler_id field (required)
    pub fn scheduler_id(mut self, value: Uuid) -> Self {
        self.scheduler_id = Some(value);
        self
    }

    /// Set the registration_id field (required)
    pub fn registration_id(mut self, value: Uuid) -> Self {
        self.registration_id = Some(value);
        self
    }

    /// Set the scheduled_date field (optional)
    pub fn scheduled_date(mut self, value: DateTime<Utc>) -> Self {
        self.scheduled_date = Some(value);
        self
    }

    /// Set the mail_sent field (default: `false`)
    pub fn mail_sent(mut self, value: bool) -> Self {
        self.mail_sent = Some(value);
        self
    }

    /// Set the outcome field (optional)
    pub fn outcome(mut self, value: String) -> Self {
        self.outcome = Some(value);
        self
    }

    /// Build the MailRegistration entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<MailRegistration, String> {
        let scheduler_id = self.scheduler_id.ok_or_else(|| "scheduler_id is required".to_string())?;
        let registration_id = self.registration_id.ok_or_else(|| "registration_id is required".to_string())?;

        Ok(MailRegistration {
            id: Uuid::new_v4(),
            scheduler_id,
            registration_id,
            scheduled_date: self.scheduled_date,
            mail_sent: self.mail_sent.unwrap_or(false),
            outcome: self.outcome,
            metadata: AuditMetadata::default(),
        })
    }
}
