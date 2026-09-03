use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_booth_state", rename_all = "snake_case")]
pub enum EventBoothState {
    Available,
    Unavailable,
}

impl std::fmt::Display for EventBoothState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Available => write!(f, "available"),
            Self::Unavailable => write!(f, "unavailable"),
        }
    }
}

impl FromStr for EventBoothState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "available" => Ok(Self::Available),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(format!("Unknown EventBoothState variant: {}", s)),
        }
    }
}

impl Default for EventBoothState {
    fn default() -> Self {
        Self::Available
    }
}
