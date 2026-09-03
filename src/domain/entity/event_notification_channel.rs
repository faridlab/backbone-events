use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_notification_channel", rename_all = "snake_case")]
pub enum EventNotificationChannel {
    Mail,
    Sms,
}

impl std::fmt::Display for EventNotificationChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mail => write!(f, "mail"),
            Self::Sms => write!(f, "sms"),
        }
    }
}

impl FromStr for EventNotificationChannel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mail" => Ok(Self::Mail),
            "sms" => Ok(Self::Sms),
            _ => Err(format!("Unknown EventNotificationChannel variant: {}", s)),
        }
    }
}

impl Default for EventNotificationChannel {
    fn default() -> Self {
        Self::Mail
    }
}
