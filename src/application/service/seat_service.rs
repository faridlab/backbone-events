//! THE ONE seat aggregation service (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! EBB-1/EVM-3 replaced: there is ONE seat counter in this module —
//! the counting domain `state IN ('open','done') AND active`, read
//! through the SAME repository the register verb guards with. A
//! second counter anywhere (a cache, a denormalized column, a
//! different filter) is the frozen W8 seam refusal.
//!
//! Capacity semantics: `seats_limited = false` (or `seats_max = 0`)
//! is UNLIMITED; multi-slot events report per-slot capacity (the
//! event total is `seats_max x event_slot_count`).

use uuid::Uuid;

use super::event_error::EventResult;
use crate::infrastructure::persistence::event_command_repository::EventCommandRepository;
use crate::infrastructure::persistence::seat_repository::{SeatCounts, SeatRepository};

/// The seat availability read (the exported surface's shape).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SeatAvailability {
    pub event_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_slot_id: Option<Uuid>,
    pub limited: bool,
    /// 0 = unlimited when `limited` is false.
    pub capacity: i64,
    pub taken: i64,
    pub available: Option<i64>,
}

impl SeatAvailability {
    pub fn from_counts(event_id: Uuid, slot_id: Option<Uuid>, c: SeatCounts) -> Self {
        let available = if c.limited && c.capacity > 0 {
            Some(c.available())
        } else {
            None
        };
        Self {
            event_id,
            event_slot_id: slot_id,
            limited: c.limited,
            capacity: c.capacity,
            taken: c.taken,
            available,
        }
    }
}

/// The seat service: availability reads composed over the seat
/// repository (which is also the register verb's guard — one domain,
/// one aggregator).
pub struct SeatService {
    seats: SeatRepository,
    events: EventCommandRepository,
}

impl SeatService {
    pub fn new(seats: SeatRepository, events: EventCommandRepository) -> Self {
        Self { seats, events }
    }

    /// Seat availability for an event (or one slot of it). Refuses
    /// `event_not_found` for a missing row; a cancelled event still
    /// READS (officers see the numbers; only registration is
    /// refused).
    pub async fn availability(
        &self,
        event_id: Uuid,
        slot_id: Option<Uuid>,
    ) -> EventResult<SeatAvailability> {
        // Existence + shape come from the event row (typed not-found).
        self.events.find(event_id).await?;
        let counts = self.seats.seat_counts(event_id, slot_id).await?;
        Ok(SeatAvailability::from_counts(event_id, slot_id, counts))
    }
}
