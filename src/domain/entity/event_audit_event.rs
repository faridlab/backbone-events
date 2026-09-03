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
    RegistrationUpdated,
    SaleSeamConfirmed,
    SaleSeamCancelled,
    SaleSeamPaid,
    BoothCreated,
    BoothUpdated,
    BoothDeleted,
    BoothConfirmed,
    BoothReleased,
    BoothBookingCreated,
    BoothBookingDeleted,
    LeadRuleCreated,
    LeadRuleUpdated,
    LeadRelinked,
    TemplateCascade,
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
            Self::RegistrationUpdated => write!(f, "registration_updated"),
            Self::SaleSeamConfirmed => write!(f, "sale_seam_confirmed"),
            Self::SaleSeamCancelled => write!(f, "sale_seam_cancelled"),
            Self::SaleSeamPaid => write!(f, "sale_seam_paid"),
            Self::BoothCreated => write!(f, "booth_created"),
            Self::BoothUpdated => write!(f, "booth_updated"),
            Self::BoothDeleted => write!(f, "booth_deleted"),
            Self::BoothConfirmed => write!(f, "booth_confirmed"),
            Self::BoothReleased => write!(f, "booth_released"),
            Self::BoothBookingCreated => write!(f, "booth_booking_created"),
            Self::BoothBookingDeleted => write!(f, "booth_booking_deleted"),
            Self::LeadRuleCreated => write!(f, "lead_rule_created"),
            Self::LeadRuleUpdated => write!(f, "lead_rule_updated"),
            Self::LeadRelinked => write!(f, "lead_relinked"),
            Self::TemplateCascade => write!(f, "template_cascade"),
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
            "registration_updated" => Ok(Self::RegistrationUpdated),
            "sale_seam_confirmed" => Ok(Self::SaleSeamConfirmed),
            "sale_seam_cancelled" => Ok(Self::SaleSeamCancelled),
            "sale_seam_paid" => Ok(Self::SaleSeamPaid),
            "booth_created" => Ok(Self::BoothCreated),
            "booth_updated" => Ok(Self::BoothUpdated),
            "booth_deleted" => Ok(Self::BoothDeleted),
            "booth_confirmed" => Ok(Self::BoothConfirmed),
            "booth_released" => Ok(Self::BoothReleased),
            "booth_booking_created" => Ok(Self::BoothBookingCreated),
            "booth_booking_deleted" => Ok(Self::BoothBookingDeleted),
            "lead_rule_created" => Ok(Self::LeadRuleCreated),
            "lead_rule_updated" => Ok(Self::LeadRuleUpdated),
            "lead_relinked" => Ok(Self::LeadRelinked),
            "template_cascade" => Ok(Self::TemplateCascade),
            _ => Err(format!("Unknown EventAuditEvent variant: {}", s)),
        }
    }
}

impl Default for EventAuditEvent {
    fn default() -> Self {
        Self::EventCreated
    }
}
