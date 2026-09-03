use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::EventBoothBookingStatus;
use super::AuditMetadata;

/// Strongly-typed ID for BoothBooking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BoothBookingId(pub Uuid);

impl BoothBookingId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for BoothBookingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for BoothBookingId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for BoothBookingId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<BoothBookingId> for Uuid {
    fn from(id: BoothBookingId) -> Self { id.0 }
}

impl AsRef<Uuid> for BoothBookingId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for BoothBookingId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BoothBooking {
    pub id: Uuid,
    pub event_booth_id: Uuid,
    pub sale_order_line_id: Option<Uuid>,
    pub status: EventBoothBookingStatus,
    pub partner_id: Option<Uuid>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl BoothBooking {
    /// Create a builder for BoothBooking
    pub fn builder() -> BoothBookingBuilder {
        <BoothBookingBuilder as Default>::default()
    }

    /// Create a new BoothBooking with required fields
    pub fn new(event_booth_id: Uuid, status: EventBoothBookingStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_booth_id,
            sale_order_line_id: None,
            status,
            partner_id: None,
            contact_name: None,
            contact_email: None,
            contact_phone: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> BoothBookingId {
        BoothBookingId(self.id)
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

    /// Get the current status
    pub fn status(&self) -> &EventBoothBookingStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the sale_order_line_id field (chainable)
    pub fn with_sale_order_line_id(mut self, value: Uuid) -> Self {
        self.sale_order_line_id = Some(value);
        self
    }

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

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "event_booth_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.event_booth_id = v; }
                }
                "sale_order_line_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sale_order_line_id = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
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
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for BoothBooking {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "BoothBooking"
    }
}

impl backbone_core::PersistentEntity for BoothBooking {
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

impl backbone_orm::EntityRepoMeta for BoothBooking {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("event_booth_id".to_string(), "uuid".to_string());
        m.insert("sale_order_line_id".to_string(), "uuid".to_string());
        m.insert("partner_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "event_booth_booking_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("eventBooth", "booths", "eventBoothId")]
    }
}

/// Builder for BoothBooking entity
///
/// Provides a fluent API for constructing BoothBooking instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct BoothBookingBuilder {
    event_booth_id: Option<Uuid>,
    sale_order_line_id: Option<Uuid>,
    status: Option<EventBoothBookingStatus>,
    partner_id: Option<Uuid>,
    contact_name: Option<String>,
    contact_email: Option<String>,
    contact_phone: Option<String>,
}

impl BoothBookingBuilder {
    /// Set the event_booth_id field (required)
    pub fn event_booth_id(mut self, value: Uuid) -> Self {
        self.event_booth_id = Some(value);
        self
    }

    /// Set the sale_order_line_id field (optional)
    pub fn sale_order_line_id(mut self, value: Uuid) -> Self {
        self.sale_order_line_id = Some(value);
        self
    }

    /// Set the status field (default: `EventBoothBookingStatus::default()`)
    pub fn status(mut self, value: EventBoothBookingStatus) -> Self {
        self.status = Some(value);
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

    /// Build the BoothBooking entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<BoothBooking, String> {
        let event_booth_id = self.event_booth_id.ok_or_else(|| "event_booth_id is required".to_string())?;

        Ok(BoothBooking {
            id: Uuid::new_v4(),
            event_booth_id,
            sale_order_line_id: self.sale_order_line_id,
            status: self.status.unwrap_or_default(),
            partner_id: self.partner_id,
            contact_name: self.contact_name,
            contact_email: self.contact_email,
            contact_phone: self.contact_phone,
            metadata: AuditMetadata::default(),
        })
    }
}
