use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use super::AuditMetadata;

/// Strongly-typed ID for Ticket
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TicketId(pub Uuid);

impl TicketId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for TicketId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TicketId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for TicketId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<TicketId> for Uuid {
    fn from(id: TicketId) -> Self { id.0 }
}

impl AsRef<Uuid> for TicketId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for TicketId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Ticket {
    pub id: Uuid,
    pub event_id: Uuid,
    pub name: String,
    pub seats_max: i32,
    pub seats_max_per_order: i32,
    pub start_sale_datetime: Option<DateTime<Utc>>,
    pub end_sale_datetime: Option<DateTime<Utc>>,
    pub product_id: Option<Uuid>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Ticket {
    /// Create a builder for Ticket
    pub fn builder() -> TicketBuilder {
        <TicketBuilder as Default>::default()
    }

    /// Create a new Ticket with required fields
    pub fn new(event_id: Uuid, name: String, seats_max: i32, seats_max_per_order: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_id,
            name,
            seats_max,
            seats_max_per_order,
            start_sale_datetime: None,
            end_sale_datetime: None,
            product_id: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> TicketId {
        TicketId(self.id)
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

    /// Set the start_sale_datetime field (chainable)
    pub fn with_start_sale_datetime(mut self, value: DateTime<Utc>) -> Self {
        self.start_sale_datetime = Some(value);
        self
    }

    /// Set the end_sale_datetime field (chainable)
    pub fn with_end_sale_datetime(mut self, value: DateTime<Utc>) -> Self {
        self.end_sale_datetime = Some(value);
        self
    }

    /// Set the product_id field (chainable)
    pub fn with_product_id(mut self, value: Uuid) -> Self {
        self.product_id = Some(value);
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
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "seats_max" => {
                    if let Ok(v) = serde_json::from_value(value) { self.seats_max = v; }
                }
                "seats_max_per_order" => {
                    if let Ok(v) = serde_json::from_value(value) { self.seats_max_per_order = v; }
                }
                "start_sale_datetime" => {
                    if let Ok(v) = serde_json::from_value(value) { self.start_sale_datetime = v; }
                }
                "end_sale_datetime" => {
                    if let Ok(v) = serde_json::from_value(value) { self.end_sale_datetime = v; }
                }
                "product_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.product_id = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Ticket {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Ticket"
    }
}

impl backbone_core::PersistentEntity for Ticket {
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

impl backbone_orm::EntityRepoMeta for Ticket {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("event_id".to_string(), "uuid".to_string());
        m.insert("product_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("event", "events", "eventId")]
    }
}

/// Builder for Ticket entity
///
/// Provides a fluent API for constructing Ticket instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct TicketBuilder {
    event_id: Option<Uuid>,
    name: Option<String>,
    seats_max: Option<i32>,
    seats_max_per_order: Option<i32>,
    start_sale_datetime: Option<DateTime<Utc>>,
    end_sale_datetime: Option<DateTime<Utc>>,
    product_id: Option<Uuid>,
}

impl TicketBuilder {
    /// Set the event_id field (required)
    pub fn event_id(mut self, value: Uuid) -> Self {
        self.event_id = Some(value);
        self
    }

    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the seats_max field (default: `0`)
    pub fn seats_max(mut self, value: i32) -> Self {
        self.seats_max = Some(value);
        self
    }

    /// Set the seats_max_per_order field (default: `0`)
    pub fn seats_max_per_order(mut self, value: i32) -> Self {
        self.seats_max_per_order = Some(value);
        self
    }

    /// Set the start_sale_datetime field (optional)
    pub fn start_sale_datetime(mut self, value: DateTime<Utc>) -> Self {
        self.start_sale_datetime = Some(value);
        self
    }

    /// Set the end_sale_datetime field (optional)
    pub fn end_sale_datetime(mut self, value: DateTime<Utc>) -> Self {
        self.end_sale_datetime = Some(value);
        self
    }

    /// Set the product_id field (optional)
    pub fn product_id(mut self, value: Uuid) -> Self {
        self.product_id = Some(value);
        self
    }

    /// Build the Ticket entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Ticket, String> {
        let event_id = self.event_id.ok_or_else(|| "event_id is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;

        Ok(Ticket {
            id: Uuid::new_v4(),
            event_id,
            name,
            seats_max: self.seats_max.unwrap_or(0),
            seats_max_per_order: self.seats_max_per_order.unwrap_or(0),
            start_sale_datetime: self.start_sale_datetime,
            end_sale_datetime: self.end_sale_datetime,
            product_id: self.product_id,
            metadata: AuditMetadata::default(),
        })
    }
}
