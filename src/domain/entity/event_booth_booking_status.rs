use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_booth_booking_status", rename_all = "snake_case")]
pub enum EventBoothBookingStatus {
    Pending,
    Confirmed,
}

impl std::fmt::Display for EventBoothBookingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Confirmed => write!(f, "confirmed"),
        }
    }
}

impl FromStr for EventBoothBookingStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "confirmed" => Ok(Self::Confirmed),
            _ => Err(format!("Unknown EventBoothBookingStatus variant: {}", s)),
        }
    }
}

impl Default for EventBoothBookingStatus {
    fn default() -> Self {
        Self::Pending
    }
}
