use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_sale_status", rename_all = "snake_case")]
pub enum EventSaleStatus {
    Free,
    Sold,
    ToPay,
}

impl std::fmt::Display for EventSaleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Free => write!(f, "free"),
            Self::Sold => write!(f, "sold"),
            Self::ToPay => write!(f, "to_pay"),
        }
    }
}

impl FromStr for EventSaleStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "free" => Ok(Self::Free),
            "sold" => Ok(Self::Sold),
            "to_pay" => Ok(Self::ToPay),
            _ => Err(format!("Unknown EventSaleStatus variant: {}", s)),
        }
    }
}

impl Default for EventSaleStatus {
    fn default() -> Self {
        Self::Free
    }
}
