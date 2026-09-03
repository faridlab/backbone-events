//! The exported trait surface (hand-written; user-owned; see
//! `metaphor.codegen.yaml`) — the W8 seam contract.
//!
//! W8 (the funnel arm) consumes THIS and ONLY this: seat
//! availability reads, publication state, the guarded intake verb,
//! the split's pair read, capability minting, the scheduler run
//! verb. It NEVER mounts a second seat counter, NEVER inserts a
//! registration, NEVER writes state directly (the frozen refusal:
//! one seat counter, one register verb — both live in this module).
//!
//! The default implementation composes the hand services; the host
//! may install the trait over its own composition.

use async_trait::async_trait;
use uuid::Uuid;

use super::event_error::EventResult;
use super::intake_service::{IntakePayload, IntakeService};
use super::my_tickets_service::MyTicketsService;
use super::registration_service::RegistrationCommandService;
use super::scheduler_service::{SchedulerRunSummary, SchedulerService};
use super::seat_service::{SeatAvailability, SeatService};
use crate::infrastructure::persistence::event_command_repository::EventCommandRepository;
use crate::infrastructure::persistence::seat_repository::RegistrationRow;
use crate::infrastructure::persistence::{scheduler_repository::SchedulerRepository, seat_repository::SeatRepository};

/// The publication-state read (the capability surface's gate,
/// exported for host-side nav/SEO decisions).
#[derive(Debug, Clone, serde::Serialize)]
pub struct PublicationState {
    pub event_id: Uuid,
    pub is_published: bool,
    pub kanban_state: String,
    pub date_publish: Option<chrono::DateTime<chrono::Utc>>,
}

/// The split's pair read: BOTH the hand-set state arm AND the
/// sale-composed axis in one shape (the sale arm's columns are
/// events-owned mirrors; NULL on the bare branch).
#[derive(Debug, Clone, serde::Serialize)]
pub struct RegistrationStateView {
    pub registration_id: Uuid,
    pub state: String,
    pub sale_order_id: Option<Uuid>,
    pub sale_order_state: Option<String>,
    pub sale_status: Option<String>,
    pub active: bool,
}

/// The capability mint scope: one event, optionally slot-scoped.
#[derive(Debug, Clone)]
pub enum CapabilityScope {
    /// The /ics feed (expiry + rotation ride the TTL arm).
    Ics { event_id: Uuid, slot_id: Option<Uuid>, ttl_secs: i64 },
    /// The attendee ticket report (never-expiring deviation; pins
    /// the registration set).
    TicketReport { event_id: Uuid, registration_ids: Vec<Uuid> },
}

/// THE surface.
#[async_trait]
pub trait EventSurface: Send + Sync {
    /// Seat availability — the ONE counter's read.
    async fn seat_availability(
        &self,
        event_id: Uuid,
        slot_id: Option<Uuid>,
    ) -> EventResult<SeatAvailability>;

    /// Publication state (the capability gate's live read).
    async fn publication_state(&self, event_id: Uuid) -> EventResult<PublicationState>;

    /// THE guarded intake verb — the W8 funnel's ONLY registration
    /// path (throttles + the ONE register verb, inside).
    async fn register_intake(
        &self,
        event_id: Uuid,
        payload: IntakePayload,
        client_ip: &str,
    ) -> EventResult<RegistrationRow>;

    /// The split's pair read.
    async fn registration_state(&self, registration_id: Uuid) -> EventResult<RegistrationStateView>;

    /// Capability minting (Tier A; the secret never crosses this
    /// boundary — the surface holds it).
    async fn mint_capability(&self, scope: CapabilityScope) -> EventResult<String>;

    /// The scheduler run verb (the host's cron/tick entry).
    async fn run_due_schedulers(&self) -> EventResult<SchedulerRunSummary>;
}

/// The default surface over the hand services.
pub struct DefaultEventSurface {
    secret: String,
    seats: SeatService,
    events: EventCommandRepository,
    registrations: RegistrationCommandService,
    intake: IntakeService,
    ics: IcsAlias,
    tickets: MyTicketsService,
    scheduler: SchedulerService,
}

type IcsAlias = super::ics_service::IcsService;

impl DefaultEventSurface {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        secret: String,
        seats: SeatService,
        events: EventCommandRepository,
        registrations: RegistrationCommandService,
        intake: IntakeService,
        ics: IcsAlias,
        tickets: MyTicketsService,
        scheduler: SchedulerService,
    ) -> Self {
        Self {
            secret,
            seats,
            events,
            registrations,
            intake,
            ics,
            tickets,
            scheduler,
        }
    }

    /// Compose the whole surface over one pool + the host ports (the
    /// host's one-call wiring; refusing ports give typed failures).
    pub fn compose(
        pool: sqlx::PgPool,
        secret: String,
        renderer: std::sync::Arc<dyn super::template_port::EventTemplateRenderer>,
        queue: std::sync::Arc<dyn super::template_port::EventMailQueue>,
        sms_queue: std::sync::Arc<dyn super::sms_port::EventSmsQueue>,
    ) -> Self {
        let registrations = RegistrationCommandService::new(SeatRepository::new(pool.clone()));
        let intake = IntakeService::new(
            RegistrationCommandService::new(SeatRepository::new(pool.clone())),
            Default::default(),
        );
        Self::new(
            secret,
            SeatService::new(
                SeatRepository::new(pool.clone()),
                EventCommandRepository::new(pool.clone()),
            ),
            EventCommandRepository::new(pool.clone()),
            registrations,
            intake,
            IcsAlias::new(EventCommandRepository::new(pool.clone())),
            MyTicketsService::new(
                EventCommandRepository::new(pool.clone()),
                SeatRepository::new(pool.clone()),
            ),
            SchedulerService::new(
                SchedulerRepository::new(pool.clone()),
                EventCommandRepository::new(pool.clone()),
                renderer,
                queue,
                sms_queue,
            ),
        )
    }
}

#[async_trait]
impl EventSurface for DefaultEventSurface {
    async fn seat_availability(
        &self,
        event_id: Uuid,
        slot_id: Option<Uuid>,
    ) -> EventResult<SeatAvailability> {
        self.seats.availability(event_id, slot_id).await
    }

    async fn publication_state(&self, event_id: Uuid) -> EventResult<PublicationState> {
        let row = self.events.find(event_id).await?;
        Ok(PublicationState {
            event_id,
            is_published: row.is_published,
            kanban_state: row.kanban_state,
            date_publish: row.date_publish,
        })
    }

    async fn register_intake(
        &self,
        event_id: Uuid,
        payload: IntakePayload,
        client_ip: &str,
    ) -> EventResult<RegistrationRow> {
        self.intake.intake(event_id, payload, client_ip).await
    }

    async fn registration_state(&self, registration_id: Uuid) -> EventResult<RegistrationStateView> {
        let row = self.registrations.get(registration_id).await?;
        Ok(RegistrationStateView {
            registration_id: row.id,
            state: row.state,
            sale_order_id: row.sale_order_id,
            sale_order_state: row.sale_order_state,
            sale_status: row.sale_status,
            active: row.active,
        })
    }

    async fn mint_capability(&self, scope: CapabilityScope) -> EventResult<String> {
        match scope {
            CapabilityScope::Ics {
                event_id,
                slot_id,
                ttl_secs,
            } => self.ics.mint(&self.secret, event_id, slot_id, ttl_secs),
            CapabilityScope::TicketReport {
                event_id,
                registration_ids,
            } => self.tickets.mint(&self.secret, event_id, &registration_ids),
        }
    }

    async fn run_due_schedulers(&self) -> EventResult<SchedulerRunSummary> {
        self.scheduler.run_due_schedulers().await
    }
}
