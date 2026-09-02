use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use super::AuditMetadata;

/// Strongly-typed ID for RegistrationAnswer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RegistrationAnswerId(pub Uuid);

impl RegistrationAnswerId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for RegistrationAnswerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for RegistrationAnswerId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for RegistrationAnswerId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<RegistrationAnswerId> for Uuid {
    fn from(id: RegistrationAnswerId) -> Self { id.0 }
}

impl AsRef<Uuid> for RegistrationAnswerId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for RegistrationAnswerId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RegistrationAnswer {
    pub id: Uuid,
    pub registration_id: Uuid,
    pub question_id: Uuid,
    pub value_text: Option<String>,
    pub value_answer_id: Option<Uuid>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl RegistrationAnswer {
    /// Create a builder for RegistrationAnswer
    pub fn builder() -> RegistrationAnswerBuilder {
        <RegistrationAnswerBuilder as Default>::default()
    }

    /// Create a new RegistrationAnswer with required fields
    pub fn new(registration_id: Uuid, question_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            registration_id,
            question_id,
            value_text: None,
            value_answer_id: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> RegistrationAnswerId {
        RegistrationAnswerId(self.id)
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

    /// Set the value_text field (chainable)
    pub fn with_value_text(mut self, value: String) -> Self {
        self.value_text = Some(value);
        self
    }

    /// Set the value_answer_id field (chainable)
    pub fn with_value_answer_id(mut self, value: Uuid) -> Self {
        self.value_answer_id = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "registration_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.registration_id = v; }
                }
                "question_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.question_id = v; }
                }
                "value_text" => {
                    if let Ok(v) = serde_json::from_value(value) { self.value_text = v; }
                }
                "value_answer_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.value_answer_id = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for RegistrationAnswer {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "RegistrationAnswer"
    }
}

impl backbone_core::PersistentEntity for RegistrationAnswer {
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

impl backbone_orm::EntityRepoMeta for RegistrationAnswer {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("registration_id".to_string(), "uuid".to_string());
        m.insert("question_id".to_string(), "uuid".to_string());
        m.insert("value_answer_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("registration", "registrations", "registrationId"), ("question", "questions", "questionId"), ("valueAnswer", "question_answers", "valueAnswerId")]
    }
}

/// Builder for RegistrationAnswer entity
///
/// Provides a fluent API for constructing RegistrationAnswer instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct RegistrationAnswerBuilder {
    registration_id: Option<Uuid>,
    question_id: Option<Uuid>,
    value_text: Option<String>,
    value_answer_id: Option<Uuid>,
}

impl RegistrationAnswerBuilder {
    /// Set the registration_id field (required)
    pub fn registration_id(mut self, value: Uuid) -> Self {
        self.registration_id = Some(value);
        self
    }

    /// Set the question_id field (required)
    pub fn question_id(mut self, value: Uuid) -> Self {
        self.question_id = Some(value);
        self
    }

    /// Set the value_text field (optional)
    pub fn value_text(mut self, value: String) -> Self {
        self.value_text = Some(value);
        self
    }

    /// Set the value_answer_id field (optional)
    pub fn value_answer_id(mut self, value: Uuid) -> Self {
        self.value_answer_id = Some(value);
        self
    }

    /// Build the RegistrationAnswer entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<RegistrationAnswer, String> {
        let registration_id = self.registration_id.ok_or_else(|| "registration_id is required".to_string())?;
        let question_id = self.question_id.ok_or_else(|| "question_id is required".to_string())?;

        Ok(RegistrationAnswer {
            id: Uuid::new_v4(),
            registration_id,
            question_id,
            value_text: self.value_text,
            value_answer_id: self.value_answer_id,
            metadata: AuditMetadata::default(),
        })
    }
}
