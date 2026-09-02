use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_sale_order_state", rename_all = "snake_case")]
pub enum EventSaleOrderState {
    Draft,
    Sent,
    Sale,
    Cancel,
}

impl std::fmt::Display for EventSaleOrderState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Sent => write!(f, "sent"),
            Self::Sale => write!(f, "sale"),
            Self::Cancel => write!(f, "cancel"),
        }
    }
}

impl FromStr for EventSaleOrderState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "sent" => Ok(Self::Sent),
            "sale" => Ok(Self::Sale),
            "cancel" => Ok(Self::Cancel),
            _ => Err(format!("Unknown EventSaleOrderState variant: {}", s)),
        }
    }
}

impl Default for EventSaleOrderState {
    fn default() -> Self {
        Self::Draft
    }
}
