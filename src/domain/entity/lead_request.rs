use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use super::AuditMetadata;

/// Strongly-typed ID for LeadRequest
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LeadRequestId(pub Uuid);

impl LeadRequestId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for LeadRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LeadRequestId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for LeadRequestId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<LeadRequestId> for Uuid {
    fn from(id: LeadRequestId) -> Self { id.0 }
}

impl AsRef<Uuid> for LeadRequestId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for LeadRequestId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LeadRequest {
    pub id: Uuid,
    pub event_id: Uuid,
    pub done: bool,
    pub error_detail: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl LeadRequest {
    /// Create a builder for LeadRequest
    pub fn builder() -> LeadRequestBuilder {
        <LeadRequestBuilder as Default>::default()
    }

    /// Create a new LeadRequest with required fields
    pub fn new(event_id: Uuid, done: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_id,
            done,
            error_detail: None,
            claimed_at: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> LeadRequestId {
        LeadRequestId(self.id)
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

    /// Set the error_detail field (chainable)
    pub fn with_error_detail(mut self, value: String) -> Self {
        self.error_detail = Some(value);
        self
    }

    /// Set the claimed_at field (chainable)
    pub fn with_claimed_at(mut self, value: DateTime<Utc>) -> Self {
        self.claimed_at = Some(value);
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
                "done" => {
                    if let Ok(v) = serde_json::from_value(value) { self.done = v; }
                }
                "error_detail" => {
                    if let Ok(v) = serde_json::from_value(value) { self.error_detail = v; }
                }
                "claimed_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.claimed_at = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for LeadRequest {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "LeadRequest"
    }
}

impl backbone_core::PersistentEntity for LeadRequest {
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

impl backbone_orm::EntityRepoMeta for LeadRequest {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("event_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("event", "events", "eventId")]
    }
}

/// Builder for LeadRequest entity
///
/// Provides a fluent API for constructing LeadRequest instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct LeadRequestBuilder {
    event_id: Option<Uuid>,
    done: Option<bool>,
    error_detail: Option<String>,
    claimed_at: Option<DateTime<Utc>>,
}

impl LeadRequestBuilder {
    /// Set the event_id field (required)
    pub fn event_id(mut self, value: Uuid) -> Self {
        self.event_id = Some(value);
        self
    }

    /// Set the done field (default: `false`)
    pub fn done(mut self, value: bool) -> Self {
        self.done = Some(value);
        self
    }

    /// Set the error_detail field (optional)
    pub fn error_detail(mut self, value: String) -> Self {
        self.error_detail = Some(value);
        self
    }

    /// Set the claimed_at field (optional)
    pub fn claimed_at(mut self, value: DateTime<Utc>) -> Self {
        self.claimed_at = Some(value);
        self
    }

    /// Build the LeadRequest entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<LeadRequest, String> {
        let event_id = self.event_id.ok_or_else(|| "event_id is required".to_string())?;

        Ok(LeadRequest {
            id: Uuid::new_v4(),
            event_id,
            done: self.done.unwrap_or(false),
            error_detail: self.error_detail,
            claimed_at: self.claimed_at,
            metadata: AuditMetadata::default(),
        })
    }
}
