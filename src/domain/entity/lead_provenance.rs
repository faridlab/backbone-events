use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::EventLeadGrouping;
use super::AuditMetadata;

/// Strongly-typed ID for LeadProvenance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LeadProvenanceId(pub Uuid);

impl LeadProvenanceId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for LeadProvenanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LeadProvenanceId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for LeadProvenanceId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<LeadProvenanceId> for Uuid {
    fn from(id: LeadProvenanceId) -> Self { id.0 }
}

impl AsRef<Uuid> for LeadProvenanceId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for LeadProvenanceId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LeadProvenance {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub event_id: Uuid,
    pub lead_id: Option<Uuid>,
    pub group_key: String,
    pub grouping: EventLeadGrouping,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl LeadProvenance {
    /// Create a builder for LeadProvenance
    pub fn builder() -> LeadProvenanceBuilder {
        <LeadProvenanceBuilder as Default>::default()
    }

    /// Create a new LeadProvenance with required fields
    pub fn new(rule_id: Uuid, event_id: Uuid, group_key: String, grouping: EventLeadGrouping) -> Self {
        Self {
            id: Uuid::new_v4(),
            rule_id,
            event_id,
            lead_id: None,
            group_key,
            grouping,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> LeadProvenanceId {
        LeadProvenanceId(self.id)
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

    /// Set the lead_id field (chainable)
    pub fn with_lead_id(mut self, value: Uuid) -> Self {
        self.lead_id = Some(value);
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
                "event_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.event_id = v; }
                }
                "lead_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.lead_id = v; }
                }
                "group_key" => {
                    if let Ok(v) = serde_json::from_value(value) { self.group_key = v; }
                }
                "grouping" => {
                    if let Ok(v) = serde_json::from_value(value) { self.grouping = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for LeadProvenance {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "LeadProvenance"
    }
}

impl backbone_core::PersistentEntity for LeadProvenance {
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

impl backbone_orm::EntityRepoMeta for LeadProvenance {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("rule_id".to_string(), "uuid".to_string());
        m.insert("event_id".to_string(), "uuid".to_string());
        m.insert("lead_id".to_string(), "uuid".to_string());
        m.insert("grouping".to_string(), "event_lead_grouping".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["group_key"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("rule", "lead_rules", "ruleId"), ("event", "events", "eventId")]
    }
}

/// Builder for LeadProvenance entity
///
/// Provides a fluent API for constructing LeadProvenance instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct LeadProvenanceBuilder {
    rule_id: Option<Uuid>,
    event_id: Option<Uuid>,
    lead_id: Option<Uuid>,
    group_key: Option<String>,
    grouping: Option<EventLeadGrouping>,
}

impl LeadProvenanceBuilder {
    /// Set the rule_id field (required)
    pub fn rule_id(mut self, value: Uuid) -> Self {
        self.rule_id = Some(value);
        self
    }

    /// Set the event_id field (required)
    pub fn event_id(mut self, value: Uuid) -> Self {
        self.event_id = Some(value);
        self
    }

    /// Set the lead_id field (optional)
    pub fn lead_id(mut self, value: Uuid) -> Self {
        self.lead_id = Some(value);
        self
    }

    /// Set the group_key field (required)
    pub fn group_key(mut self, value: String) -> Self {
        self.group_key = Some(value);
        self
    }

    /// Set the grouping field (required)
    pub fn grouping(mut self, value: EventLeadGrouping) -> Self {
        self.grouping = Some(value);
        self
    }

    /// Build the LeadProvenance entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<LeadProvenance, String> {
        let rule_id = self.rule_id.ok_or_else(|| "rule_id is required".to_string())?;
        let event_id = self.event_id.ok_or_else(|| "event_id is required".to_string())?;
        let group_key = self.group_key.ok_or_else(|| "group_key is required".to_string())?;
        let grouping = self.grouping.ok_or_else(|| "grouping is required".to_string())?;

        Ok(LeadProvenance {
            id: Uuid::new_v4(),
            rule_id,
            event_id,
            lead_id: self.lead_id,
            group_key,
            grouping,
            metadata: AuditMetadata::default(),
        })
    }
}
