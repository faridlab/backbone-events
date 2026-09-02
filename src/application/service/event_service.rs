//! The event verbs (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! The generated CRUD alias first (the generator declares this
//! module but emits no file for a read-only model), then the verb
//! layer:
//!
//! - create (with the ONE template-apply: the event type's mail
//!   templates become event-scoped scheduler rows exactly once, at
//!   create, and never re-propagate);
//! - the typed PATCH whitelist — the FENCE is here:
//!   `is_published`/`date_publish` in a patch body is a typed
//!   `publish_refused` (and the repository has no arm that could
//!   write the pair even if a future caller forgot the check);
//! - publish/unpublish — the ONLY writers of the fence pair;
//! - mark_done — writes `done` AND the first pipe_end stage by
//!   sequence (the daily sweep drives the same verb, bounded).

use uuid::Uuid;

use backbone_core::GenericCrudService;
use crate::domain::entity::Event;
use crate::infrastructure::persistence::event_command_repository::{
    CreateEventInput, EventCommandRepository, EventRow, PatchEventInput,
};
use crate::presentation::dto::{CreateEventDto, UpdateEventDto};

/// Generated CRUD alias (keeps lib.rs's wiring compiling).
pub type EventService = GenericCrudService<
    Event,
    CreateEventDto,
    UpdateEventDto,
    crate::infrastructure::persistence::EventRepository,
>;

use super::event_error::{EventError, EventResult};

/// The publication-fence pair: fields a generic PATCH may NEVER
/// write. `publish`/`unpublish` are the only writers.
pub const PUBLISH_FENCED_FIELDS: [&str; 2] = ["is_published", "date_publish"];

/// The event verb service.
pub struct EventCommandService {
    events: EventCommandRepository,
}

impl EventCommandService {
    pub fn new(events: EventCommandRepository) -> Self {
        Self { events }
    }

    /// Create an event (template-apply rides the repository's create
    /// transaction — once, never re-propagates).
    pub async fn create(
        &self,
        input: &CreateEventInput,
        actor: Option<Uuid>,
    ) -> EventResult<EventRow> {
        if input.date_end < input.date_begin {
            return Err(EventError::Validation(
                "date_end precedes date_begin".to_string(),
            ));
        }
        if input.is_multi_slots && input.event_slot_count < 1 {
            return Err(EventError::Validation(
                "multi-slot events declare at least one slot".to_string(),
            ));
        }
        self.events.create(input, actor).await
    }

    /// The typed patch: the fence is checked HERE and the repository
    /// structurally cannot write the pair. The webapp/admin caller
    /// hands a JSON object; the HTTP layer maps it to
    /// [`PatchEventInput`] and REFUSES the fenced keys before this
    /// call (this is the service-side backstop for in-process
    /// callers).
    pub async fn patch(
        &self,
        id: Uuid,
        patch: &PatchEventInput,
        actor: Option<Uuid>,
    ) -> EventResult<EventRow> {
        self.events.patch(id, patch, actor).await
    }

    /// PUBLISH — the only writer that sets the flag. Stamps
    /// `date_publish` on first publish; republish keeps the stamp.
    pub async fn publish(&self, id: Uuid, actor: Option<Uuid>) -> EventResult<EventRow> {
        self.events.publish(id, actor).await
    }

    /// UNPUBLISH — the only writer that clears the flag. The
    /// capability surface reads the flag live: unpublished means the
    /// uniform 404 on the next request.
    pub async fn unpublish(&self, id: Uuid, actor: Option<Uuid>) -> EventResult<EventRow> {
        self.events.unpublish(id, actor).await
    }

    /// MARK DONE — the two-axis close (kanban `done` + first
    /// pipe_end stage by sequence).
    pub async fn mark_done(&self, id: Uuid, actor: Option<Uuid>) -> EventResult<EventRow> {
        self.events.mark_done(id, actor).await
    }

    /// Officer read: one event.
    pub async fn get(&self, id: Uuid) -> EventResult<EventRow> {
        self.events.find(id).await
    }

    /// Officer read: the event list.
    pub async fn list(&self, limit: i64) -> EventResult<Vec<EventRow>> {
        self.events.list(limit).await
    }
}
