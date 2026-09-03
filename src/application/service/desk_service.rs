//! The registration desk service (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! ONE authenticated verb: `register_attendee(barcode, event_id)` —
//! the desk scan. The barcode is EXACT-MATCH (EVM/EBG-1: barcodes are
//! globally unique in-crate, minted from urandom decimals — no codec,
//! no checksum, no LIKE); the branch order is FROZEN and each refusal
//! is its own typed error so the desk surface can say precisely why a
//! scan refused:
//!
//! 1. unknown barcode (or an inactive row)  -> DeskInvalidTicket
//! 2. cancelled registration               -> DeskCanceledRegistration
//! 3. draft registration                   -> DeskUnconfirmedRegistration (no write — payment has not confirmed the seat)
//! 4. the scanned event is not ongoing      -> DeskNotOngoingEvent (done/cancelled kanban state or past its end)
//! 5. ticket belongs to another event       -> DeskNeedManualConfirmation (no write — a desk never mutates a foreign event's row)
//! 6. already done                          -> DeskAlreadyRegistered
//! 7. open                                  -> the one write: done (through the seat repository's transition — seat head, arm rules, and audit included), the write path of the desk verb.
//!
//! The nomenclature machinery (sequence/prefix badge naming) is
//! deliberately NOT ported — the fence is recorded in
//! docs/spec-overlay.md.

use uuid::Uuid;

use super::event_error::EventResult;
use crate::infrastructure::persistence::event_command_repository::EventCommandRepository;
use crate::infrastructure::persistence::seat_repository::{RegistrationRow, SeatRepository};

/// The desk service.
pub struct DeskService {
    seats: SeatRepository,
    events: EventCommandRepository,
}

impl DeskService {
    pub fn new(seats: SeatRepository, events: EventCommandRepository) -> Self {
        Self { seats, events }
    }

    /// THE DESK VERB (EBG-2): one scan, the frozen branch order, at
    /// most ONE write. Returns the registration row (post-transition)
    /// and the (before, after) state pair.
    pub async fn register_attendee(
        &self,
        barcode: &str,
        event_id: Uuid,
        actor: Option<Uuid>,
    ) -> EventResult<(RegistrationRow, String, String)> {
        // 1. Exact-match lookup — unknown or inactive = invalid ticket.
        let reg = self
            .seats
            .find_by_barcode(barcode.trim())
            .await?
            .filter(|r| r.active)
            .ok_or(super::event_error::EventError::DeskInvalidTicket)?;

        // 2. Cancelled — the ticket was voided with its registration.
        if reg.state == "cancel" {
            return Err(super::event_error::EventError::DeskCanceledRegistration);
        }
        // 3. Draft — no write: the seat is not confirmed yet.
        if reg.state == "draft" {
            return Err(super::event_error::EventError::DeskUnconfirmedRegistration);
        }

        // 4. The scanned event must be ONGOING: not done/cancelled and
        // not past its end.
        let event = self.events.find(event_id).await?;
        if event.kanban_state == "done"
            || event.kanban_state == "cancel"
            || event.date_end <= chrono::Utc::now()
        {
            return Err(super::event_error::EventError::DeskNotOngoingEvent);
        }

        // 5. Cross-event ticket — the desk never mutates another
        // event's row; a human confirms at the ticket's own desk.
        if reg.event_id != event_id {
            return Err(super::event_error::EventError::DeskNeedManualConfirmation);
        }

        // 6. Already registered — idempotent refusal, no second write.
        if reg.state == "done" {
            return Err(super::event_error::EventError::DeskAlreadyRegistered);
        }

        // 7. Open -> done: the one write, through the SAME transition
        // verb everything else uses (seat head, arm rules, audit).
        let (before, after, _active) = self.seats.transition(reg.id, "done", actor).await?;
        let refreshed = self.seats.find_registration(reg.id).await?;
        Ok((refreshed, before, after))
    }
}
