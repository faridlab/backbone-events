use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_lead_basis", rename_all = "snake_case")]
pub enum EventLeadBasis {
    Registration,
    Attendee,
}

impl std::fmt::Display for EventLeadBasis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registration => write!(f, "registration"),
            Self::Attendee => write!(f, "attendee"),
        }
    }
}

impl FromStr for EventLeadBasis {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "registration" => Ok(Self::Registration),
            "attendee" => Ok(Self::Attendee),
            _ => Err(format!("Unknown EventLeadBasis variant: {}", s)),
        }
    }
}

impl Default for EventLeadBasis {
    fn default() -> Self {
        Self::Registration
    }
}
