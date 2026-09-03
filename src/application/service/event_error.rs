//! The module's typed error surface (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! ONE enum for every hand verb, mapped to HTTP once, here. Every
//! refusal a client can distinguish is a NAMED variant with a STABLE
//! `code` string — the wire contract probes and the webapp assert
//! against. Refusals a client must NOT be able to distinguish (the
//! capability-gate family: unpublished, archived, missing, bad
//! signature, expired, cross-event token) all surface as the ONE
//! uniform `event_not_published` 404 — no enumeration oracle.

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// The module's typed error enum.
#[derive(Debug, thiserror::Error)]
pub enum EventError {
    /// The event id does not resolve to a row (or is soft-deleted).
    #[error("event not found")]
    EventNotFound,

    /// Seat domain refusal: the event (or slot) is at capacity under
    /// the counting domain `state IN ('open','done') AND active`.
    #[error("event seats exhausted")]
    EventSeatsExhausted { event_id: uuid::Uuid },

    /// Read-time lazy sale-window refusal (no cron flips anything —
    /// the pure predicate over the ticket's stored instants said no).
    #[error("event sale window closed")]
    EventSaleWindowClosed { event_ticket_id: uuid::Uuid },

    /// A multi-slot event requires a slot on every registration.
    #[error("event slot required")]
    EventSlotRequired { event_id: uuid::Uuid },

    /// The slot does not belong to the named event (EBG-3).
    #[error("event slot not of event")]
    EventSlotNotOfEvent { event_slot_id: uuid::Uuid },

    /// The ticket does not belong to the named event (EBG-4).
    #[error("event ticket not of event")]
    EventTicketNotOfEvent { event_ticket_id: uuid::Uuid },

    /// The registration id does not resolve.
    #[error("registration not found")]
    RegistrationNotFound,

    /// THE UNIFORM CAPABILITY-GATE 404: unpublished, archived,
    /// missing, bad signature, expired, or cross-event token — ONE
    /// answer, no oracle (the /ics + my-tickets refusal family).
    #[error("event not published")]
    EventNotPublished,

    /// `EVENT_CAPABILITY_SECRET` is unset: the capability routes
    /// refuse typed rather than mint under an empty key (fail-closed).
    #[error("event capability secret not configured")]
    EventCapabilitySecretNotConfigured,

    /// Fixed-window throttle refusal (per-identity AND per-IP).
    #[error("event throttled")]
    EventThrottled { retry_after_secs: u32 },

    /// A generic patch tried to write the publication fence pair
    /// (`is_published` / `date_publish`) — publish/unpublish are the
    /// only writers.
    #[error("publish refused: {fields}")]
    PublishRefused { fields: String },

    /// The typed scheduler-failure family (recorded on the scheduler
    /// row, never a registration blocker): the template ref is empty,
    /// the host never installed a renderer, the render call failed,
    /// the queue refused, or the recipient address is invalid.
    #[error("template unresolved")]
    TemplateUnresolved,
    #[error("template renderer not composed")]
    TemplateRendererNotComposed,
    #[error("render failed: {0}")]
    RenderFailed(String),
    #[error("enqueue refused: {0}")]
    EnqueueRefused(String),
    #[error("recipient invalid: {0}")]
    RecipientInvalid(String),

    // ── the booth family (v0.2.0) ──────────────────────────────────────────

    /// The booth id does not resolve.
    #[error("booth not found")]
    BoothNotFound { booth_id: uuid::Uuid },

    /// The booking id does not resolve.
    #[error("booth booking not found")]
    BoothBookingNotFound { booking_id: uuid::Uuid },

    /// THE EBS-1 LOUD LOSER: another confirmed booking holds the booth
    /// (the partial unique index is the wall; this refusal is how a
    /// concurrent loser learns it lost — first-confirm-wins, the
    /// collateral competitor-cancel is deliberately not ported).
    #[error("booth already confirmed")]
    BoothAlreadyConfirmed { event_booth_id: uuid::Uuid },

    /// EBS-5c: a sale-linked booth (sale_order_line_id set) is not
    /// deleted — the delete verb refuses.
    #[error("booth delete refused: sale-linked")]
    BoothDeleteRefusedSaleLinked { booth_id: uuid::Uuid },

    /// EBS-4: one order line may book booths of ONE event only.
    #[error("booth booking line crosses events")]
    BoothBookingLineCrossEvent { sale_order_line_id: uuid::Uuid },

    // ── the CRM family (v0.2.0) ────────────────────────────────────────────

    /// The lead rule id does not resolve.
    #[error("lead rule not found")]
    LeadRuleNotFound { rule_id: uuid::Uuid },

    /// The lead sink port refused (unwired host or downstream
    /// refusal): the generation request parks loudly with error_detail
    /// and is retried — never a silent skip.
    #[error("lead sink refused: {0}")]
    LeadSinkRefused(String),

    /// The event's queue row is freshly LEASED — another walker (the
    /// cron pass or another officer run) is generating its leads right
    /// now. One walker per event at a time is the queue's contract;
    /// retry after the walk finishes or the lease expires.
    #[error("lead request busy: another generation walk holds the lease")]
    LeadRequestBusy { event_id: uuid::Uuid },

    /// The event has no queue row: no trigger (registration verb, rule
    /// create/activate) has ever armed it — there is nothing to run.
    #[error("lead request not found: nothing armed for the event")]
    LeadRequestNotFound { event_id: uuid::Uuid },

    // ── the scanner desk family (v0.2.0 — the FROZEN branch order) ─────────

    /// Branch 1: the barcode resolves to no registration (invalid
    /// ticket).
    #[error("desk: invalid ticket")]
    DeskInvalidTicket,

    /// Branch 2: the registration is cancelled.
    #[error("desk: canceled registration")]
    DeskCanceledRegistration,

    /// Branch 3: the registration is draft (unconfirmed — no write).
    #[error("desk: unconfirmed registration")]
    DeskUnconfirmedRegistration,

    /// Branch 4: the event is finished (takes precedence over done —
    /// a done badge at a finished event reports the event, not the
    /// attendee).
    #[error("desk: not ongoing event")]
    DeskNotOngoingEvent,

    /// Branch 5: the barcode belongs to a registration of ANOTHER
    /// event (manual confirmation territory — no write).
    #[error("desk: need manual confirmation")]
    DeskNeedManualConfirmation,

    /// Branch 7: already registered (done) — idempotent-ish, but the
    /// desk hears it loudly rather than re-stamping.
    #[error("desk: already registered")]
    DeskAlreadyRegistered,

    /// Request-shape refusal (allowlist parse failures etc).
    #[error("validation: {0}")]
    Validation(String),

    /// Infrastructure failures (never mapped from a domain refusal).
    #[error("database: {0}")]
    Database(String),

    #[error("internal: {0}")]
    Internal(String),
}

impl EventError {
    /// The stable wire code (the string probes and clients match on).
    pub fn code(&self) -> &'static str {
        match self {
            Self::EventNotFound => "event_not_found",
            Self::EventSeatsExhausted { .. } => "event_seats_exhausted",
            Self::EventSaleWindowClosed { .. } => "event_sale_window_closed",
            Self::EventSlotRequired { .. } => "event_slot_required",
            Self::EventSlotNotOfEvent { .. } => "event_slot_not_of_event",
            Self::EventTicketNotOfEvent { .. } => "event_ticket_not_of_event",
            Self::RegistrationNotFound => "registration_not_found",
            Self::EventNotPublished => "event_not_published",
            Self::EventCapabilitySecretNotConfigured => "event_capability_secret_not_configured",
            Self::EventThrottled { .. } => "event_throttled",
            Self::PublishRefused { .. } => "publish_refused",
            Self::TemplateUnresolved => "template_unresolved",
            Self::TemplateRendererNotComposed => "template_renderer_not_composed",
            Self::RenderFailed(_) => "render_failed",
            Self::EnqueueRefused(_) => "enqueue_refused",
            Self::RecipientInvalid(_) => "recipient_invalid",
            Self::BoothNotFound { .. } => "booth_not_found",
            Self::BoothBookingNotFound { .. } => "booth_booking_not_found",
            Self::BoothAlreadyConfirmed { .. } => "booth_already_confirmed",
            Self::BoothDeleteRefusedSaleLinked { .. } => "booth_delete_refused_sale_linked",
            Self::BoothBookingLineCrossEvent { .. } => "booth_booking_line_cross_event",
            Self::LeadRuleNotFound { .. } => "lead_rule_not_found",
            Self::LeadSinkRefused(_) => "lead_sink_refused",
            Self::LeadRequestBusy { .. } => "lead_request_busy",
            Self::LeadRequestNotFound { .. } => "lead_request_not_found",
            Self::DeskInvalidTicket => "desk_invalid_ticket",
            Self::DeskCanceledRegistration => "desk_canceled_registration",
            Self::DeskUnconfirmedRegistration => "desk_unconfirmed_registration",
            Self::DeskNotOngoingEvent => "desk_not_ongoing_event",
            Self::DeskNeedManualConfirmation => "desk_need_manual_confirmation",
            Self::DeskAlreadyRegistered => "desk_already_registered",
            Self::Validation(_) => "event_validation_failed",
            Self::Database(_) => "event_database_error",
            Self::Internal(_) => "event_internal_error",
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::EventNotFound
            | Self::RegistrationNotFound
            | Self::EventNotPublished
            | Self::BoothNotFound { .. }
            | Self::BoothBookingNotFound { .. }
            | Self::LeadRuleNotFound { .. }
            | Self::LeadRequestNotFound { .. } => StatusCode::NOT_FOUND,
            Self::EventSeatsExhausted { .. }
            | Self::EventSaleWindowClosed { .. }
            | Self::EventSlotRequired { .. }
            | Self::EventSlotNotOfEvent { .. }
            | Self::EventTicketNotOfEvent { .. }
            | Self::PublishRefused { .. }
            | Self::BoothDeleteRefusedSaleLinked { .. }
            | Self::BoothBookingLineCrossEvent { .. }
            | Self::DeskInvalidTicket
            | Self::DeskCanceledRegistration
            | Self::DeskUnconfirmedRegistration
            | Self::DeskNotOngoingEvent
            | Self::DeskNeedManualConfirmation
            | Self::DeskAlreadyRegistered
            | Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::EventThrottled { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::EventCapabilitySecretNotConfigured => StatusCode::SERVICE_UNAVAILABLE,
            Self::TemplateUnresolved
            | Self::TemplateRendererNotComposed
            | Self::RenderFailed(_)
            | Self::EnqueueRefused(_)
            | Self::RecipientInvalid(_)
            | Self::BoothAlreadyConfirmed { .. }
            | Self::LeadSinkRefused(_)
            | Self::LeadRequestBusy { .. } => StatusCode::CONFLICT,
            Self::Database(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for EventError {
    fn into_response(self) -> Response {
        let status = self.status();
        let mut headers = HeaderMap::new();
        if let Self::EventThrottled { retry_after_secs } = &self {
            if let Ok(v) = HeaderValue::from_str(&retry_after_secs.to_string()) {
                headers.insert(header::RETRY_AFTER, v);
            }
        }
        // The refusal detail the caller may see. The capability-gate
        // family deliberately carries NO detail: the body is the same
        // for every member (uniform `event_not_published`).
        let message = match &self {
            Self::EventNotPublished | Self::EventCapabilitySecretNotConfigured => {
                self.to_string()
            }
            _ => self.to_string(),
        };
        let body = Json(json!({
            "error": { "code": self.code(), "message": message }
        }));
        (status, headers, body).into_response()
    }
}

impl From<sqlx::Error> for EventError {
    fn from(e: sqlx::Error) -> Self {
        Self::Database(e.to_string())
    }
}

impl From<anyhow::Error> for EventError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

/// Result alias over [`EventError`].
pub type EventResult<T> = Result<T, EventError>;
