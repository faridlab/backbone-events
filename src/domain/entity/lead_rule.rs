use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::EventLeadBasis;
use super::AuditMetadata;

/// Strongly-typed ID for LeadRule
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LeadRuleId(pub Uuid);

impl LeadRuleId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for LeadRuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LeadRuleId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for LeadRuleId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<LeadRuleId> for Uuid {
    fn from(id: LeadRuleId) -> Self { id.0 }
}

impl AsRef<Uuid> for LeadRuleId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for LeadRuleId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LeadRule {
    pub id: Uuid,
    pub name: String,
    pub event_id: Option<Uuid>,
    pub basis: EventLeadBasis,
    pub on_create: bool,
    pub on_confirm: bool,
    pub on_done: bool,
    pub active: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl LeadRule {
    /// Create a builder for LeadRule
    pub fn builder() -> LeadRuleBuilder {
        <LeadRuleBuilder as Default>::default()
    }

    /// Create a new LeadRule with required fields
    pub fn new(name: String, basis: EventLeadBasis, on_create: bool, on_confirm: bool, on_done: bool, active: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            event_id: None,
            basis,
            on_create,
            on_confirm,
            on_done,
            active,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> LeadRuleId {
        LeadRuleId(self.id)
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

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "event_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.event_id = v; }
                }
                "basis" => {
                    if let Ok(v) = serde_json::from_value(value) { self.basis = v; }
                }
                "on_create" => {
                    if let Ok(v) = serde_json::from_value(value) { self.on_create = v; }
                }
                "on_confirm" => {
                    if let Ok(v) = serde_json::from_value(value) { self.on_confirm = v; }
                }
                "on_done" => {
                    if let Ok(v) = serde_json::from_value(value) { self.on_done = v; }
                }
                "active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.active = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for LeadRule {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "LeadRule"
    }
}

impl backbone_core::PersistentEntity for LeadRule {
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

impl backbone_orm::EntityRepoMeta for LeadRule {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("event_id".to_string(), "uuid".to_string());
        m.insert("basis".to_string(), "event_lead_basis".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("event", "events", "eventId")]
    }
}

/// Builder for LeadRule entity
///
/// Provides a fluent API for constructing LeadRule instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct LeadRuleBuilder {
    name: Option<String>,
    event_id: Option<Uuid>,
    basis: Option<EventLeadBasis>,
    on_create: Option<bool>,
    on_confirm: Option<bool>,
    on_done: Option<bool>,
    active: Option<bool>,
}

impl LeadRuleBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the event_id field (optional)
    pub fn event_id(mut self, value: Uuid) -> Self {
        self.event_id = Some(value);
        self
    }

    /// Set the basis field (default: `EventLeadBasis::default()`)
    pub fn basis(mut self, value: EventLeadBasis) -> Self {
        self.basis = Some(value);
        self
    }

    /// Set the on_create field (default: `false`)
    pub fn on_create(mut self, value: bool) -> Self {
        self.on_create = Some(value);
        self
    }

    /// Set the on_confirm field (default: `false`)
    pub fn on_confirm(mut self, value: bool) -> Self {
        self.on_confirm = Some(value);
        self
    }

    /// Set the on_done field (default: `false`)
    pub fn on_done(mut self, value: bool) -> Self {
        self.on_done = Some(value);
        self
    }

    /// Set the active field (default: `true`)
    pub fn active(mut self, value: bool) -> Self {
        self.active = Some(value);
        self
    }

    /// Build the LeadRule entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<LeadRule, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;

        Ok(LeadRule {
            id: Uuid::new_v4(),
            name,
            event_id: self.event_id,
            basis: self.basis.unwrap_or_default(),
            on_create: self.on_create.unwrap_or(false),
            on_confirm: self.on_confirm.unwrap_or(false),
            on_done: self.on_done.unwrap_or(false),
            active: self.active.unwrap_or(true),
            metadata: AuditMetadata::default(),
        })
    }
}
