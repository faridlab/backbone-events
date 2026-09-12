use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::EventRegistrationState;
use super::EventSaleOrderState;
use super::EventSaleStatus;
use super::AuditMetadata;

use crate::domain::state_machine::{registration_stateStateMachine, registration_stateState, StateMachineError};

/// Strongly-typed ID for Registration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RegistrationId(pub Uuid);

impl RegistrationId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for RegistrationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for RegistrationId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for RegistrationId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<RegistrationId> for Uuid {
    fn from(id: RegistrationId) -> Self { id.0 }
}

impl AsRef<Uuid> for RegistrationId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for RegistrationId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Registration {
    pub id: Uuid,
    pub event_id: Uuid,
    pub event_slot_id: Option<Uuid>,
    pub event_ticket_id: Option<Uuid>,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub company_name: Option<String>,
    pub partner_id: Option<Uuid>,
    pub(crate) state: EventRegistrationState,
    pub date_closed: Option<DateTime<Utc>>,
    pub sale_order_id: Option<Uuid>,
    pub sale_order_state: Option<EventSaleOrderState>,
    pub sale_status: Option<EventSaleStatus>,
    pub active: bool,
    pub barcode: String,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Registration {
    /// Create a builder for Registration
    pub fn builder() -> RegistrationBuilder {
        <RegistrationBuilder as Default>::default()
    }

    /// Create a new Registration with required fields
    pub fn new(event_id: Uuid, name: String, email: String, state: EventRegistrationState, active: bool, barcode: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_id,
            event_slot_id: None,
            event_ticket_id: None,
            name,
            email,
            phone: None,
            company_name: None,
            partner_id: None,
            state,
            date_closed: None,
            sale_order_id: None,
            sale_order_state: None,
            sale_status: None,
            active,
            barcode,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> RegistrationId {
        RegistrationId(self.id)
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

    /// Set the event_slot_id field (chainable)
    pub fn with_event_slot_id(mut self, value: Uuid) -> Self {
        self.event_slot_id = Some(value);
        self
    }

    /// Set the event_ticket_id field (chainable)
    pub fn with_event_ticket_id(mut self, value: Uuid) -> Self {
        self.event_ticket_id = Some(value);
        self
    }

    /// Set the phone field (chainable)
    pub fn with_phone(mut self, value: String) -> Self {
        self.phone = Some(value);
        self
    }

    /// Set the company_name field (chainable)
    pub fn with_company_name(mut self, value: String) -> Self {
        self.company_name = Some(value);
        self
    }

    /// Set the partner_id field (chainable)
    pub fn with_partner_id(mut self, value: Uuid) -> Self {
        self.partner_id = Some(value);
        self
    }

    /// Set the date_closed field (chainable)
    pub fn with_date_closed(mut self, value: DateTime<Utc>) -> Self {
        self.date_closed = Some(value);
        self
    }

    /// Set the sale_order_id field (chainable)
    pub fn with_sale_order_id(mut self, value: Uuid) -> Self {
        self.sale_order_id = Some(value);
        self
    }

    /// Set the sale_order_state field (chainable)
    pub fn with_sale_order_state(mut self, value: EventSaleOrderState) -> Self {
        self.sale_order_state = Some(value);
        self
    }

    /// Set the sale_status field (chainable)
    pub fn with_sale_status(mut self, value: EventSaleStatus) -> Self {
        self.sale_status = Some(value);
        self
    }

    // ==========================================================
    // State Machine
    // ==========================================================

    /// Transition to a new state via the state state machine.
    ///
    /// Returns `Err` if the transition is not permitted from the current state.
    /// Use this method instead of assigning `self.state` directly.
    pub fn transition_to(&mut self, new_state: registration_stateState) -> Result<(), StateMachineError> {
        let current = self.state.to_string().parse::<registration_stateState>()?;
        let mut sm = registration_stateStateMachine::from_state(current);
        sm.transition_to_state(new_state)?;
        self.state = new_state.to_string().parse::<EventRegistrationState>()
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
                "event_slot_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.event_slot_id = v; }
                }
                "event_ticket_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.event_ticket_id = v; }
                }
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "email" => {
                    if let Ok(v) = serde_json::from_value(value) { self.email = v; }
                }
                "phone" => {
                    if let Ok(v) = serde_json::from_value(value) { self.phone = v; }
                }
                "company_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_name = v; }
                }
                "partner_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.partner_id = v; }
                }
                "date_closed" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date_closed = v; }
                }
                "sale_order_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sale_order_id = v; }
                }
                "sale_order_state" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sale_order_state = v; }
                }
                "sale_status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sale_status = v; }
                }
                "active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.active = v; }
                }
                "barcode" => {
                    if let Ok(v) = serde_json::from_value(value) { self.barcode = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Registration {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Registration"
    }
}

impl backbone_core::PersistentEntity for Registration {
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

impl backbone_orm::EntityRepoMeta for Registration {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("event_id".to_string(), "uuid".to_string());
        m.insert("event_slot_id".to_string(), "uuid".to_string());
        m.insert("event_ticket_id".to_string(), "uuid".to_string());
        m.insert("partner_id".to_string(), "uuid".to_string());
        m.insert("sale_order_id".to_string(), "uuid".to_string());
        m.insert("state".to_string(), "event_registration_state".to_string());
        m.insert("sale_order_state".to_string(), "event_sale_order_state".to_string());
        m.insert("sale_status".to_string(), "event_sale_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name", "email", "barcode"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("event", "events", "eventId"), ("eventSlot", "slots", "eventSlotId"), ("eventTicket", "tickets", "eventTicketId")]
    }
}

/// Builder for Registration entity
///
/// Provides a fluent API for constructing Registration instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct RegistrationBuilder {
    event_id: Option<Uuid>,
    event_slot_id: Option<Uuid>,
    event_ticket_id: Option<Uuid>,
    name: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    company_name: Option<String>,
    partner_id: Option<Uuid>,
    state: Option<EventRegistrationState>,
    date_closed: Option<DateTime<Utc>>,
    sale_order_id: Option<Uuid>,
    sale_order_state: Option<EventSaleOrderState>,
    sale_status: Option<EventSaleStatus>,
    active: Option<bool>,
    barcode: Option<String>,
}

impl RegistrationBuilder {
    /// Set the event_id field (required)
    pub fn event_id(mut self, value: Uuid) -> Self {
        self.event_id = Some(value);
        self
    }

    /// Set the event_slot_id field (optional)
    pub fn event_slot_id(mut self, value: Uuid) -> Self {
        self.event_slot_id = Some(value);
        self
    }

    /// Set the event_ticket_id field (optional)
    pub fn event_ticket_id(mut self, value: Uuid) -> Self {
        self.event_ticket_id = Some(value);
        self
    }

    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the email field (required)
    pub fn email(mut self, value: String) -> Self {
        self.email = Some(value);
        self
    }

    /// Set the phone field (optional)
    pub fn phone(mut self, value: String) -> Self {
        self.phone = Some(value);
        self
    }

    /// Set the company_name field (optional)
    pub fn company_name(mut self, value: String) -> Self {
        self.company_name = Some(value);
        self
    }

    /// Set the partner_id field (optional)
    pub fn partner_id(mut self, value: Uuid) -> Self {
        self.partner_id = Some(value);
        self
    }

    /// Set the state field (default: `EventRegistrationState::default()`)
    pub fn state(mut self, value: EventRegistrationState) -> Self {
        self.state = Some(value);
        self
    }

    /// Set the date_closed field (optional)
    pub fn date_closed(mut self, value: DateTime<Utc>) -> Self {
        self.date_closed = Some(value);
        self
    }

    /// Set the sale_order_id field (optional)
    pub fn sale_order_id(mut self, value: Uuid) -> Self {
        self.sale_order_id = Some(value);
        self
    }

    /// Set the sale_order_state field (optional)
    pub fn sale_order_state(mut self, value: EventSaleOrderState) -> Self {
        self.sale_order_state = Some(value);
        self
    }

    /// Set the sale_status field (optional)
    pub fn sale_status(mut self, value: EventSaleStatus) -> Self {
        self.sale_status = Some(value);
        self
    }

    /// Set the active field (default: `true`)
    pub fn active(mut self, value: bool) -> Self {
        self.active = Some(value);
        self
    }

    /// Set the barcode field (required)
    pub fn barcode(mut self, value: String) -> Self {
        self.barcode = Some(value);
        self
    }

    /// Build the Registration entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Registration, String> {
        let event_id = self.event_id.ok_or_else(|| "event_id is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let email = self.email.ok_or_else(|| "email is required".to_string())?;
        let barcode = self.barcode.ok_or_else(|| "barcode is required".to_string())?;

        Ok(Registration {
            id: Uuid::new_v4(),
            event_id,
            event_slot_id: self.event_slot_id,
            event_ticket_id: self.event_ticket_id,
            name,
            email,
            phone: self.phone,
            company_name: self.company_name,
            partner_id: self.partner_id,
            state: self.state.unwrap_or_default(),
            date_closed: self.date_closed,
            sale_order_id: self.sale_order_id,
            sale_order_state: self.sale_order_state,
            sale_status: self.sale_status,
            active: self.active.unwrap_or(true),
            barcode,
            metadata: AuditMetadata::default(),
        })
    }
}
