use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::EventQuestionKind;
use super::AuditMetadata;

/// Strongly-typed ID for Question
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QuestionId(pub Uuid);

impl QuestionId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for QuestionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for QuestionId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for QuestionId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<QuestionId> for Uuid {
    fn from(id: QuestionId) -> Self { id.0 }
}

impl AsRef<Uuid> for QuestionId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for QuestionId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Question {
    pub id: Uuid,
    pub question: String,
    pub question_kind: EventQuestionKind,
    pub is_default: bool,
    pub is_reusable: bool,
    pub once_per_order: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Question {
    /// Create a builder for Question
    pub fn builder() -> QuestionBuilder {
        <QuestionBuilder as Default>::default()
    }

    /// Create a new Question with required fields
    pub fn new(question: String, question_kind: EventQuestionKind, is_default: bool, is_reusable: bool, once_per_order: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            question,
            question_kind,
            is_default,
            is_reusable,
            once_per_order,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> QuestionId {
        QuestionId(self.id)
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
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "question" => {
                    if let Ok(v) = serde_json::from_value(value) { self.question = v; }
                }
                "question_kind" => {
                    if let Ok(v) = serde_json::from_value(value) { self.question_kind = v; }
                }
                "is_default" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_default = v; }
                }
                "is_reusable" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_reusable = v; }
                }
                "once_per_order" => {
                    if let Ok(v) = serde_json::from_value(value) { self.once_per_order = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Question {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Question"
    }
}

impl backbone_core::PersistentEntity for Question {
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

impl backbone_orm::EntityRepoMeta for Question {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("question_kind".to_string(), "event_question_kind".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["question"]
    }
}

/// Builder for Question entity
///
/// Provides a fluent API for constructing Question instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct QuestionBuilder {
    question: Option<String>,
    question_kind: Option<EventQuestionKind>,
    is_default: Option<bool>,
    is_reusable: Option<bool>,
    once_per_order: Option<bool>,
}

impl QuestionBuilder {
    /// Set the question field (required)
    pub fn question(mut self, value: String) -> Self {
        self.question = Some(value);
        self
    }

    /// Set the question_kind field (default: `EventQuestionKind::default()`)
    pub fn question_kind(mut self, value: EventQuestionKind) -> Self {
        self.question_kind = Some(value);
        self
    }

    /// Set the is_default field (default: `false`)
    pub fn is_default(mut self, value: bool) -> Self {
        self.is_default = Some(value);
        self
    }

    /// Set the is_reusable field (default: `false`)
    pub fn is_reusable(mut self, value: bool) -> Self {
        self.is_reusable = Some(value);
        self
    }

    /// Set the once_per_order field (default: `false`)
    pub fn once_per_order(mut self, value: bool) -> Self {
        self.once_per_order = Some(value);
        self
    }

    /// Build the Question entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Question, String> {
        let question = self.question.ok_or_else(|| "question is required".to_string())?;

        Ok(Question {
            id: Uuid::new_v4(),
            question,
            question_kind: self.question_kind.unwrap_or_default(),
            is_default: self.is_default.unwrap_or(false),
            is_reusable: self.is_reusable.unwrap_or(false),
            once_per_order: self.once_per_order.unwrap_or(false),
            metadata: AuditMetadata::default(),
        })
    }
}
