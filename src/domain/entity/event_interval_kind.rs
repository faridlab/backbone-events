use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_interval_kind", rename_all = "snake_case")]
pub enum EventIntervalKind {
    AfterSub,
    BeforeEvent,
    AfterEventStart,
    AfterEvent,
    BeforeEventEnd,
}

impl std::fmt::Display for EventIntervalKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AfterSub => write!(f, "after_sub"),
            Self::BeforeEvent => write!(f, "before_event"),
            Self::AfterEventStart => write!(f, "after_event_start"),
            Self::AfterEvent => write!(f, "after_event"),
            Self::BeforeEventEnd => write!(f, "before_event_end"),
        }
    }
}

impl FromStr for EventIntervalKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "after_sub" => Ok(Self::AfterSub),
            "before_event" => Ok(Self::BeforeEvent),
            "after_event_start" => Ok(Self::AfterEventStart),
            "after_event" => Ok(Self::AfterEvent),
            "before_event_end" => Ok(Self::BeforeEventEnd),
            _ => Err(format!("Unknown EventIntervalKind variant: {}", s)),
        }
    }
}

impl Default for EventIntervalKind {
    fn default() -> Self {
        Self::AfterSub
    }
}
