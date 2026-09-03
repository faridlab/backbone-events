use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::EventBoothState;
use super::AuditMetadata;

use crate::domain::state_machine::{booth_stateStateMachine, booth_stateState, StateMachineError};

/// Strongly-typed ID for Booth
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BoothId(pub Uuid);

impl BoothId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for BoothId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for BoothId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for BoothId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<BoothId> for Uuid {
    fn from(id: BoothId) -> Self { id.0 }
}

impl AsRef<Uuid> for BoothId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for BoothId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Booth {
    pub id: Uuid,
    pub event_id: Uuid,
    pub booth_category_id: Uuid,
    pub name: String,
    pub(crate) state: EventBoothState,
    pub partner_id: Option<Uuid>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub sale_order_line_id: Option<Uuid>,
    pub is_paid: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Booth {
    /// Create a builder for Booth
    pub fn builder() -> BoothBuilder {
        <BoothBuilder as Default>::default()
    }

    /// Create a new Booth with required fields
    pub fn new(event_id: Uuid, booth_category_id: Uuid, name: String, state: EventBoothState, is_paid: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_id,
            booth_category_id,
            name,
            state,
            partner_id: None,
            contact_name: None,
            contact_email: None,
            contact_phone: None,
            sale_order_line_id: None,
            is_paid,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> BoothId {
        BoothId(self.id)
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

    /// Set the partner_id field (chainable)
    pub fn with_partner_id(mut self, value: Uuid) -> Self {
        self.partner_id = Some(value);
        self
    }

    /// Set the contact_name field (chainable)
    pub fn with_contact_name(mut self, value: String) -> Self {
        self.contact_name = Some(value);
        self
    }

    /// Set the contact_email field (chainable)
    pub fn with_contact_email(mut self, value: String) -> Self {
        self.contact_email = Some(value);
        self
    }

    /// Set the contact_phone field (chainable)
    pub fn with_contact_phone(mut self, value: String) -> Self {
        self.contact_phone = Some(value);
        self
    }

    /// Set the sale_order_line_id field (chainable)
    pub fn with_sale_order_line_id(mut self, value: Uuid) -> Self {
        self.sale_order_line_id = Some(value);
        self
    }

    // ==========================================================
    // State Machine
    // ==========================================================

    /// Transition to a new state via the state state machine.
    ///
    /// Returns `Err` if the transition is not permitted from the current state.
    /// Use this method instead of assigning `self.state` directly.
    pub fn transition_to(&mut self, new_state: booth_stateState) -> Result<(), StateMachineError> {
        let current = self.state.to_string().parse::<booth_stateState>()?;
        let mut sm = booth_stateStateMachine::from_state(current);
        sm.transition_to_state(new_state)?;
        self.state = new_state.to_string().parse::<EventBoothState>()
            .map_err(|e| StateMachineError::InvalidState(e.to_string()))?;
        Ok(())
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
                "booth_category_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.booth_category_id = v; }
                }
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "partner_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.partner_id = v; }
                }
                "contact_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.contact_name = v; }
                }
                "contact_email" => {
                    if let Ok(v) = serde_json::from_value(value) { self.contact_email = v; }
                }
                "contact_phone" => {
                    if let Ok(v) = serde_json::from_value(value) { self.contact_phone = v; }
                }
                "sale_order_line_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sale_order_line_id = v; }
                }
                "is_paid" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_paid = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Booth {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Booth"
    }
}

impl backbone_core::PersistentEntity for Booth {
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

impl backbone_orm::EntityRepoMeta for Booth {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("event_id".to_string(), "uuid".to_string());
        m.insert("booth_category_id".to_string(), "uuid".to_string());
        m.insert("partner_id".to_string(), "uuid".to_string());
        m.insert("sale_order_line_id".to_string(), "uuid".to_string());
        m.insert("state".to_string(), "event_booth_state".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("event", "events", "eventId"), ("boothCategory", "booth_categories", "boothCategoryId")]
    }
}

/// Builder for Booth entity
///
/// Provides a fluent API for constructing Booth instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct BoothBuilder {
    event_id: Option<Uuid>,
    booth_category_id: Option<Uuid>,
    name: Option<String>,
    state: Option<EventBoothState>,
    partner_id: Option<Uuid>,
    contact_name: Option<String>,
    contact_email: Option<String>,
    contact_phone: Option<String>,
    sale_order_line_id: Option<Uuid>,
    is_paid: Option<bool>,
}

impl BoothBuilder {
    /// Set the event_id field (required)
    pub fn event_id(mut self, value: Uuid) -> Self {
        self.event_id = Some(value);
        self
    }

    /// Set the booth_category_id field (required)
    pub fn booth_category_id(mut self, value: Uuid) -> Self {
        self.booth_category_id = Some(value);
        self
    }

    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the state field (default: `EventBoothState::default()`)
    pub fn state(mut self, value: EventBoothState) -> Self {
        self.state = Some(value);
        self
    }

    /// Set the partner_id field (optional)
    pub fn partner_id(mut self, value: Uuid) -> Self {
        self.partner_id = Some(value);
        self
    }

    /// Set the contact_name field (optional)
    pub fn contact_name(mut self, value: String) -> Self {
        self.contact_name = Some(value);
        self
    }

    /// Set the contact_email field (optional)
    pub fn contact_email(mut self, value: String) -> Self {
        self.contact_email = Some(value);
        self
    }

    /// Set the contact_phone field (optional)
    pub fn contact_phone(mut self, value: String) -> Self {
        self.contact_phone = Some(value);
        self
    }

    /// Set the sale_order_line_id field (optional)
    pub fn sale_order_line_id(mut self, value: Uuid) -> Self {
        self.sale_order_line_id = Some(value);
        self
    }

    /// Set the is_paid field (default: `false`)
    pub fn is_paid(mut self, value: bool) -> Self {
        self.is_paid = Some(value);
        self
    }

    /// Build the Booth entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Booth, String> {
        let event_id = self.event_id.ok_or_else(|| "event_id is required".to_string())?;
        let booth_category_id = self.booth_category_id.ok_or_else(|| "booth_category_id is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;

        Ok(Booth {
            id: Uuid::new_v4(),
            event_id,
            booth_category_id,
            name,
            state: self.state.unwrap_or_default(),
            partner_id: self.partner_id,
            contact_name: self.contact_name,
            contact_email: self.contact_email,
            contact_phone: self.contact_phone,
            sale_order_line_id: self.sale_order_line_id,
            is_paid: self.is_paid.unwrap_or(false),
            metadata: AuditMetadata::default(),
        })
    }
}
