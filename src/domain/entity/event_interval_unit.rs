use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_interval_unit", rename_all = "snake_case")]
pub enum EventIntervalUnit {
    Now,
    Hours,
    Days,
    Weeks,
    Months,
}

impl std::fmt::Display for EventIntervalUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Now => write!(f, "now"),
            Self::Hours => write!(f, "hours"),
            Self::Days => write!(f, "days"),
            Self::Weeks => write!(f, "weeks"),
            Self::Months => write!(f, "months"),
        }
    }
}

impl FromStr for EventIntervalUnit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "now" => Ok(Self::Now),
            "hours" => Ok(Self::Hours),
            "days" => Ok(Self::Days),
            "weeks" => Ok(Self::Weeks),
            "months" => Ok(Self::Months),
            _ => Err(format!("Unknown EventIntervalUnit variant: {}", s)),
        }
    }
}

impl Default for EventIntervalUnit {
    fn default() -> Self {
        Self::Now
    }
}
