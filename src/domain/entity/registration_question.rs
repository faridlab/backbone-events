use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use super::AuditMetadata;

/// Strongly-typed ID for RegistrationQuestion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RegistrationQuestionId(pub Uuid);

impl RegistrationQuestionId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for RegistrationQuestionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for RegistrationQuestionId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for RegistrationQuestionId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<RegistrationQuestionId> for Uuid {
    fn from(id: RegistrationQuestionId) -> Self { id.0 }
}

impl AsRef<Uuid> for RegistrationQuestionId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for RegistrationQuestionId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RegistrationQuestion {
    pub id: Uuid,
    pub event_id: Option<Uuid>,
    pub event_type_id: Option<Uuid>,
    pub question_id: Uuid,
    pub sequence: i32,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl RegistrationQuestion {
    /// Create a builder for RegistrationQuestion
    pub fn builder() -> RegistrationQuestionBuilder {
        <RegistrationQuestionBuilder as Default>::default()
    }

    /// Create a new RegistrationQuestion with required fields
    pub fn new(question_id: Uuid, sequence: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_id: None,
            event_type_id: None,
            question_id,
            sequence,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> RegistrationQuestionId {
        RegistrationQuestionId(self.id)
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

    /// Set the event_id field (chainable)
    pub fn with_event_id(mut self, value: Uuid) -> Self {
        self.event_id = Some(value);
        self
    }

    /// Set the event_type_id field (chainable)
    pub fn with_event_type_id(mut self, value: Uuid) -> Self {
        self.event_type_id = Some(value);
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
                "event_type_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.event_type_id = v; }
                }
                "question_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.question_id = v; }
                }
                "sequence" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sequence = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for RegistrationQuestion {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "RegistrationQuestion"
    }
}

impl backbone_core::PersistentEntity for RegistrationQuestion {
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

impl backbone_orm::EntityRepoMeta for RegistrationQuestion {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("event_id".to_string(), "uuid".to_string());
        m.insert("event_type_id".to_string(), "uuid".to_string());
        m.insert("question_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("event", "events", "eventId"), ("eventType", "types", "eventTypeId"), ("question", "questions", "questionId")]
    }
}

/// Builder for RegistrationQuestion entity
///
/// Provides a fluent API for constructing RegistrationQuestion instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct RegistrationQuestionBuilder {
    event_id: Option<Uuid>,
    event_type_id: Option<Uuid>,
    question_id: Option<Uuid>,
    sequence: Option<i32>,
}

impl RegistrationQuestionBuilder {
    /// Set the event_id field (optional)
    pub fn event_id(mut self, value: Uuid) -> Self {
        self.event_id = Some(value);
        self
    }

    /// Set the event_type_id field (optional)
    pub fn event_type_id(mut self, value: Uuid) -> Self {
        self.event_type_id = Some(value);
        self
    }

    /// Set the question_id field (required)
    pub fn question_id(mut self, value: Uuid) -> Self {
        self.question_id = Some(value);
        self
    }

    /// Set the sequence field (default: `1`)
    pub fn sequence(mut self, value: i32) -> Self {
        self.sequence = Some(value);
        self
    }

    /// Build the RegistrationQuestion entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<RegistrationQuestion, String> {
        let question_id = self.question_id.ok_or_else(|| "question_id is required".to_string())?;

        Ok(RegistrationQuestion {
            id: Uuid::new_v4(),
            event_id: self.event_id,
            event_type_id: self.event_type_id,
            question_id,
            sequence: self.sequence.unwrap_or(1),
            metadata: AuditMetadata::default(),
        })
    }
}
