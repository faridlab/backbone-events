use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use super::AuditMetadata;

/// Strongly-typed ID for TypeBooth
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TypeBoothId(pub Uuid);

impl TypeBoothId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for TypeBoothId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TypeBoothId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for TypeBoothId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<TypeBoothId> for Uuid {
    fn from(id: TypeBoothId) -> Self { id.0 }
}

impl AsRef<Uuid> for TypeBoothId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for TypeBoothId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TypeBooth {
    pub id: Uuid,
    pub event_type_id: Uuid,
    pub booth_category_id: Uuid,
    pub name: String,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl TypeBooth {
    /// Create a builder for TypeBooth
    pub fn builder() -> TypeBoothBuilder {
        <TypeBoothBuilder as Default>::default()
    }

    /// Create a new TypeBooth with required fields
    pub fn new(event_type_id: Uuid, booth_category_id: Uuid, name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type_id,
            booth_category_id,
            name,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> TypeBoothId {
        TypeBoothId(self.id)
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
                "event_type_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.event_type_id = v; }
                }
                "booth_category_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.booth_category_id = v; }
                }
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for TypeBooth {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "TypeBooth"
    }
}

impl backbone_core::PersistentEntity for TypeBooth {
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

impl backbone_orm::EntityRepoMeta for TypeBooth {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("event_type_id".to_string(), "uuid".to_string());
        m.insert("booth_category_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("eventType", "types", "eventTypeId"), ("boothCategory", "booth_categories", "boothCategoryId")]
    }
}

/// Builder for TypeBooth entity
///
/// Provides a fluent API for constructing TypeBooth instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct TypeBoothBuilder {
    event_type_id: Option<Uuid>,
    booth_category_id: Option<Uuid>,
    name: Option<String>,
}

impl TypeBoothBuilder {
    /// Set the event_type_id field (required)
    pub fn event_type_id(mut self, value: Uuid) -> Self {
        self.event_type_id = Some(value);
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

    /// Build the TypeBooth entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<TypeBooth, String> {
        let event_type_id = self.event_type_id.ok_or_else(|| "event_type_id is required".to_string())?;
        let booth_category_id = self.booth_category_id.ok_or_else(|| "booth_category_id is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;

        Ok(TypeBooth {
            id: Uuid::new_v4(),
            event_type_id,
            booth_category_id,
            name,
            metadata: AuditMetadata::default(),
        })
    }
}
