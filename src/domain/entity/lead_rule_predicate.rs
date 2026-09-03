use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::EventLeadPredicateAxis;
use super::AuditMetadata;

/// Strongly-typed ID for LeadRulePredicate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LeadRulePredicateId(pub Uuid);

impl LeadRulePredicateId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for LeadRulePredicateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LeadRulePredicateId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for LeadRulePredicateId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<LeadRulePredicateId> for Uuid {
    fn from(id: LeadRulePredicateId) -> Self { id.0 }
}

impl AsRef<Uuid> for LeadRulePredicateId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for LeadRulePredicateId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LeadRulePredicate {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub axis: EventLeadPredicateAxis,
    pub question_id: Option<Uuid>,
    pub value_ids: serde_json::Value,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl LeadRulePredicate {
    /// Create a builder for LeadRulePredicate
    pub fn builder() -> LeadRulePredicateBuilder {
        <LeadRulePredicateBuilder as Default>::default()
    }

    /// Create a new LeadRulePredicate with required fields
    pub fn new(rule_id: Uuid, axis: EventLeadPredicateAxis, value_ids: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            rule_id,
            axis,
            question_id: None,
            value_ids,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> LeadRulePredicateId {
        LeadRulePredicateId(self.id)
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

    /// Set the question_id field (chainable)
    pub fn with_question_id(mut self, value: Uuid) -> Self {
        self.question_id = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "rule_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rule_id = v; }
                }
                "axis" => {
                    if let Ok(v) = serde_json::from_value(value) { self.axis = v; }
                }
                "question_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.question_id = v; }
                }
                "value_ids" => {
                    if let Ok(v) = serde_json::from_value(value) { self.value_ids = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for LeadRulePredicate {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "LeadRulePredicate"
    }
}

impl backbone_core::PersistentEntity for LeadRulePredicate {
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

impl backbone_orm::EntityRepoMeta for LeadRulePredicate {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("rule_id".to_string(), "uuid".to_string());
        m.insert("question_id".to_string(), "uuid".to_string());
        m.insert("axis".to_string(), "event_lead_predicate_axis".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("rule", "lead_rules", "ruleId"), ("question", "questions", "questionId")]
    }
}

/// Builder for LeadRulePredicate entity
///
/// Provides a fluent API for constructing LeadRulePredicate instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct LeadRulePredicateBuilder {
    rule_id: Option<Uuid>,
    axis: Option<EventLeadPredicateAxis>,
    question_id: Option<Uuid>,
    value_ids: Option<serde_json::Value>,
}

impl LeadRulePredicateBuilder {
    /// Set the rule_id field (required)
    pub fn rule_id(mut self, value: Uuid) -> Self {
        self.rule_id = Some(value);
        self
    }

    /// Set the axis field (required)
    pub fn axis(mut self, value: EventLeadPredicateAxis) -> Self {
        self.axis = Some(value);
        self
    }

    /// Set the question_id field (optional)
    pub fn question_id(mut self, value: Uuid) -> Self {
        self.question_id = Some(value);
        self
    }

    /// Set the value_ids field (required)
    pub fn value_ids(mut self, value: serde_json::Value) -> Self {
        self.value_ids = Some(value);
        self
    }

    /// Build the LeadRulePredicate entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<LeadRulePredicate, String> {
        let rule_id = self.rule_id.ok_or_else(|| "rule_id is required".to_string())?;
        let axis = self.axis.ok_or_else(|| "axis is required".to_string())?;
        let value_ids = self.value_ids.ok_or_else(|| "value_ids is required".to_string())?;

        Ok(LeadRulePredicate {
            id: Uuid::new_v4(),
            rule_id,
            axis,
            question_id: self.question_id,
            value_ids,
            metadata: AuditMetadata::default(),
        })
    }
}
