use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_audit_event", rename_all = "snake_case")]
pub enum EventAuditEvent {
    EventCreated,
    EventUpdated,
    EventPublished,
    EventUnpublished,
    PublishRefused,
    EventMarkDone,
    RegistrationCreated,
    RegistrationRefused,
    RegistrationStateChanged,
    RegistrationArchived,
    SchedulerCreated,
    SchedulerRun,
    SchedulerFailed,
    ReceiptDroppedWindowClosed,
    CapabilityRefused,
}

impl std::fmt::Display for EventAuditEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EventCreated => write!(f, "event_created"),
            Self::EventUpdated => write!(f, "event_updated"),
            Self::EventPublished => write!(f, "event_published"),
            Self::EventUnpublished => write!(f, "event_unpublished"),
            Self::PublishRefused => write!(f, "publish_refused"),
            Self::EventMarkDone => write!(f, "event_mark_done"),
            Self::RegistrationCreated => write!(f, "registration_created"),
            Self::RegistrationRefused => write!(f, "registration_refused"),
            Self::RegistrationStateChanged => write!(f, "registration_state_changed"),
            Self::RegistrationArchived => write!(f, "registration_archived"),
            Self::SchedulerCreated => write!(f, "scheduler_created"),
            Self::SchedulerRun => write!(f, "scheduler_run"),
            Self::SchedulerFailed => write!(f, "scheduler_failed"),
            Self::ReceiptDroppedWindowClosed => write!(f, "receipt_dropped_window_closed"),
            Self::CapabilityRefused => write!(f, "capability_refused"),
        }
    }
}

impl FromStr for EventAuditEvent {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "event_created" => Ok(Self::EventCreated),
            "event_updated" => Ok(Self::EventUpdated),
            "event_published" => Ok(Self::EventPublished),
            "event_unpublished" => Ok(Self::EventUnpublished),
            "publish_refused" => Ok(Self::PublishRefused),
            "event_mark_done" => Ok(Self::EventMarkDone),
            "registration_created" => Ok(Self::RegistrationCreated),
            "registration_refused" => Ok(Self::RegistrationRefused),
            "registration_state_changed" => Ok(Self::RegistrationStateChanged),
            "registration_archived" => Ok(Self::RegistrationArchived),
            "scheduler_created" => Ok(Self::SchedulerCreated),
            "scheduler_run" => Ok(Self::SchedulerRun),
            "scheduler_failed" => Ok(Self::SchedulerFailed),
            "receipt_dropped_window_closed" => Ok(Self::ReceiptDroppedWindowClosed),
            "capability_refused" => Ok(Self::CapabilityRefused),
            _ => Err(format!("Unknown EventAuditEvent variant: {}", s)),
        }
    }
}

impl Default for EventAuditEvent {
    fn default() -> Self {
        Self::EventCreated
    }
}
