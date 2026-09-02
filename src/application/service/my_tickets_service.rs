//! The attendee ticket-report read (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! `GET /api/v1/event/public/my-tickets/{capability}` — a Tier A
//! token (ADR-0018) pins ONE event plus the SORTED registration id
//! set. The never-expiring deviation is documented at the mint: an
//! attendee's ticket link must survive any rotation cadence, and the
//! payload pins the exact rows — an old token grants nothing beyond
//! the attendee's own already-issued registrations. The publication
//! gate is the SAME uniform 404 family as /ics (unpublished event =
//! no ticket feed, no oracle).
//!
//! The payload is responsive (the webapp renders badges/tickets; the
//! module never renders HTML).

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::capability::{mint_ticket_report_capability, CapabilityClaims, PURPOSE_TICKET_REPORT};
use super::event_error::{EventError, EventResult};
use crate::infrastructure::persistence::event_command_repository::EventCommandRepository;
use crate::infrastructure::persistence::seat_repository::{RegistrationRow, SeatRepository};

/// One attendee's ticket row (the responsive payload).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TicketView {
    pub registration_id: Uuid,
    pub event_id: Uuid,
    pub event_name: String,
    pub event_date_begin: DateTime<Utc>,
    pub event_date_end: DateTime<Utc>,
    pub event_date_tz: String,
    pub attendee_name: String,
    pub state: String,
    pub barcode: String,
}

/// The whole feed (event header + the pinned rows).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TicketReport {
    pub event_id: Uuid,
    pub event_name: String,
    pub event_date_begin: DateTime<Utc>,
    pub event_date_end: DateTime<Utc>,
    pub event_date_tz: String,
    pub badge_format: String,
    pub tickets: Vec<TicketView>,
}

/// The my-tickets service.
pub struct MyTicketsService {
    events: EventCommandRepository,
    seats: SeatRepository,
}

impl MyTicketsService {
    pub fn new(events: EventCommandRepository, seats: SeatRepository) -> Self {
        Self { events, seats }
    }

    /// Mint a ticket-report capability (never-expiring deviation;
    /// payload pins event + sorted registration ids).
    pub fn mint(
        &self,
        secret: &str,
        event_id: Uuid,
        registration_ids: &[Uuid],
    ) -> EventResult<String> {
        mint_ticket_report_capability(secret, &event_id, registration_ids)
    }

    /// THE GATED READ: verify the token, gate the event's publication
    /// state (uniform refusal family), then return ONLY the pinned
    /// rows — a token naming another event's registrations yields
    /// that event's uniform refusal or nothing; unpinned rows are
    /// never returned.
    pub async fn report(&self, secret: &str, token: &str) -> EventResult<TicketReport> {
        let claims = CapabilityClaims::verify(secret, PURPOSE_TICKET_REPORT, token, Utc::now())?;
        let event_id = claims
            .data
            .first()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or(EventError::EventNotPublished)?;
        let pinned: Vec<Uuid> = claims
            .data
            .iter()
            .skip(1)
            .filter_map(|s| Uuid::parse_str(s).ok())
            .collect();

        let event = self.events.find_published(event_id).await?;

        let mut tickets = Vec::with_capacity(pinned.len());
        for reg_id in pinned {
            // A pinned id that does not resolve (or belongs to another
            // event) is silently absent from the feed — the token's
            // word is the only grant, and absence is not an oracle.
            if let Ok(row) = self.seats.find_registration(reg_id).await {
                if row.event_id == event_id {
                    tickets.push(ticket_of(&event, &row));
                }
            }
        }
        Ok(TicketReport {
            event_id: event.id,
            event_name: event.name.clone(),
            event_date_begin: event.date_begin,
            event_date_end: event.date_end,
            event_date_tz: event.date_tz.clone(),
            badge_format: event.badge_format.clone(),
            tickets,
        })
    }
}

fn ticket_of(
    event: &crate::infrastructure::persistence::event_command_repository::EventRow,
    row: &RegistrationRow,
) -> TicketView {
    TicketView {
        registration_id: row.id,
        event_id: event.id,
        event_name: event.name.clone(),
        event_date_begin: event.date_begin,
        event_date_end: event.date_end,
        event_date_tz: event.date_tz.clone(),
        attendee_name: row.name.clone(),
        state: row.state.clone(),
        barcode: row.barcode.clone(),
    }
}
