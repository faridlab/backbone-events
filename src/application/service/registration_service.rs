//! The registration verbs (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! The generated CRUD alias first (keeps lib.rs's wiring compiling —
//! the generator declares this module but emits no file for a
//! read-only model), then the verb layer:
//!
//! - THE ONE REGISTER VERB: lock-first, count-then-insert, one
//!   transaction — `SELECT ... FOR UPDATE` on the event row (slot
//!   row too when multi-slot) INSIDE the verb, via the seat
//!   repository (services hold no raw sqlx).
//! - The four one-liner state verbs (any -> any, no guard, no
//!   monotonicity), with the ARM RULE applied on their outcome:
//!   entering 'open' from draft/cancel arms the after_sub engines;
//!   re-confirming an open row never re-arms; open -> done never
//!   re-arms.
//! - sync_from_partner (fill-only-when-empty).
//!
//! Typed refusals are durable facts: `registration_refused` is
//! audited best-effort after the transaction rolls back.

use uuid::Uuid;

use backbone_core::GenericCrudService;
use crate::domain::entity::Registration;
use crate::infrastructure::persistence::seat_repository::{
    RegisterCommand, RegistrationRow, SeatRepository,
};
use crate::presentation::dto::{CreateRegistrationDto, UpdateRegistrationDto};

/// Generated CRUD alias (the generator declares this module in
/// `application/service/mod.rs` but emits no file for a read-only
/// model; the alias keeps that declaration true).
pub type RegistrationService = GenericCrudService<
    Registration,
    CreateRegistrationDto,
    UpdateRegistrationDto,
    RegistrationRepositoryAlias,
>;

use super::event_error::EventResult;

type RegistrationRepositoryAlias = crate::infrastructure::persistence::RegistrationRepository;

/// The registration verb service.
pub struct RegistrationCommandService {
    seats: SeatRepository,
}

impl RegistrationCommandService {
    pub fn new(seats: SeatRepository) -> Self {
        Self { seats }
    }

    /// THE ONE REGISTER VERB — the only seat-taking path in the
    /// module. Every intake (officer, W8 funnel, future sale
    /// subscription) funnels through here; nothing else inserts a
    /// registration.
    pub async fn register(&self, cmd: RegisterCommand) -> EventResult<RegistrationRow> {
        match self.seats.register(&cmd).await {
            Ok(row) => Ok(row),
            Err(refusal) => {
                // The typed refusal is a durable fact (critical-events
                // list) — best-effort, never masking the refusal.
                crate::infrastructure::persistence::seat_repository::record_audit(
                    self.seats.pool(),
                    "registration_refused",
                    cmd.actor,
                    "event",
                    cmd.event_id,
                    serde_json::json!({ "email": cmd.email, "code": refusal.code() }),
                )
                .await;
                Err(refusal)
            }
        }
    }

    /// confirm — entering 'open' from draft/cancel arms the engines.
    pub async fn confirm(
        &self,
        registration_id: Uuid,
        actor: Option<Uuid>,
    ) -> EventResult<RegistrationRow> {
        self.transition(registration_id, "open", actor).await
    }

    /// set_draft — the sale-flow special case ONLY.
    pub async fn set_draft(
        &self,
        registration_id: Uuid,
        actor: Option<Uuid>,
    ) -> EventResult<RegistrationRow> {
        self.transition(registration_id, "draft", actor).await
    }

    /// set_done — stamps `date_closed` if empty (never overwrites a
    /// manual value).
    pub async fn set_done(
        &self,
        registration_id: Uuid,
        actor: Option<Uuid>,
    ) -> EventResult<RegistrationRow> {
        self.transition(registration_id, "done", actor).await
    }

    /// cancel — the row leaves the seat domain and mail eligibility
    /// together (with `active`, the EVM2-4 pair).
    pub async fn cancel(
        &self,
        registration_id: Uuid,
        actor: Option<Uuid>,
    ) -> EventResult<RegistrationRow> {
        self.transition(registration_id, "cancel", actor).await
    }

    async fn transition(
        &self,
        registration_id: Uuid,
        to: &str,
        actor: Option<Uuid>,
    ) -> EventResult<RegistrationRow> {
        self.seats.transition(registration_id, to, actor).await?;
        self.seats.find_registration(registration_id).await
    }

    /// sync_from_partner — fill-only-when-empty identity fields.
    pub async fn sync_from_partner(
        &self,
        registration_id: Uuid,
        partner_id: Uuid,
        name: Option<&str>,
        phone: Option<&str>,
        company_name: Option<&str>,
        actor: Option<Uuid>,
    ) -> EventResult<RegistrationRow> {
        self.seats
            .sync_from_partner(registration_id, partner_id, name, phone, company_name, actor)
            .await
    }

    /// Officer read: one registration.
    pub async fn get(&self, registration_id: Uuid) -> EventResult<RegistrationRow> {
        self.seats.find_registration(registration_id).await
    }

    /// Officer read: a slice of an event's registrations.
    pub async fn list(
        &self,
        event_id: Uuid,
        limit: i64,
        after: Option<Uuid>,
    ) -> EventResult<Vec<RegistrationRow>> {
        self.seats.list_registrations(event_id, limit, after).await
    }
}
