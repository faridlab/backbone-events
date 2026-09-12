//! The sale seam service (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! The TYPED boundary the host bridge calls into. The module never
//! parses selling's outbox envelopes: the host owns the envelope
//! decode, the outbox inbox and the enrichment (the buying-consumers
//! precedent — the bridge maps each event line to a registration spec
//! and calls one verb per delivery). These DTOs are the WHOLE wire
//! contract of that bridge; they are `deny_unknown_fields` so an
//! envelope drift is a typed parse refusal, never a silent partial
//! apply.
//!
//! The consumer key defaults to `event-sale-seam` (the seam's name in
//! the shared inbox); a host running several event consumers passes
//! its own.

use serde::Deserialize;
use uuid::Uuid;

use super::event_error::EventResult;
use crate::infrastructure::persistence::sale_seam_repository::{SaleSeamRepository, SeamOutcome};
use crate::infrastructure::persistence::seat_repository::RegisterCommand;

/// The default consumer key in the seam inbox.
pub const SALE_SEAM_CONSUMER: &str = "event-sale-seam";

/// SalesOrderConfirmed — the typed bridge command (payload per
/// selling.hook.yaml:135-140 + the host's line enrichment).
///
/// `registrations` is the order's attendee specs (one per confirmed
/// event line seat); the carrier itself carries NO line data, so the
/// bridge builds these from the order — this is the declared
/// enrichment point (upstream minted straight off the sale order
/// lines; the port keeps the mint typed and events-owned).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderConfirmed {
    /// The outbox delivery id (exactly-once key); falls back to the
    /// order id when the host relay cannot supply one.
    pub delivery_id: Option<String>,
    pub order_id: Uuid,
    /// The order's org-unit anchor (the composing service's tenancy
    /// axis; the mint itself never reads it — registrations derive
    /// their org from the event row).
    pub org_unit_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    /// The order grand total as its raw decimal string — the mint
    /// reads ONLY its zero/non-zero magnitude (ES-3's free/paid fork).
    pub grand_total: String,
    pub currency: Option<String>,
    /// The attendee specs (one per event line seat; the host enriches).
    pub registrations: Vec<RegistrationSpec>,
}

/// One minted attendee of a confirmed order.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationSpec {
    pub event_id: Uuid,
    pub event_slot_id: Option<Uuid>,
    pub event_ticket_id: Option<Uuid>,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub company_name: Option<String>,
    pub partner_id: Option<Uuid>,
}

/// SalesOrderCancelled — the typed bridge command.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderCancelled {
    /// The outbox delivery id (exactly-once key); falls back to the
    /// order id when the host relay cannot supply one.
    pub delivery_id: Option<String>,
    pub order_id: Uuid,
    /// The order's org-unit anchor (unused by the cancel mirror; kept
    /// so the bridge payload is one shape across the seam family).
    pub org_unit_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
}

/// The paid-hook family — the typed bridge command (the billing seam;
/// no producer exists at this pin, the verb is landed for compose).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderPaid {
    /// The outbox delivery id (exactly-once key); falls back to the
    /// order id when the host relay cannot supply one.
    pub delivery_id: Option<String>,
    pub order_id: Uuid,
    /// The order's line ids — the booth latch's key (bookings carry
    /// the line ref; registrations key on the order id directly).
    pub line_ids: Vec<Uuid>,
}

/// The sale seam service.
pub struct SaleSeamService {
    repo: SaleSeamRepository,
    consumer: String,
}

impl SaleSeamService {
    pub fn new(repo: SaleSeamRepository) -> Self {
        Self {
            repo,
            consumer: SALE_SEAM_CONSUMER.to_string(),
        }
    }

    /// A host running several event consumers names its own.
    pub fn with_consumer(mut self, consumer: impl Into<String>) -> Self {
        self.consumer = consumer.into();
        self
    }

    /// SalesOrderConfirmed -> mint (single paid SO -> born draft held;
    /// free -> born open + armed) or heal an already-linked group.
    /// Idempotent per delivery (the seam inbox).
    pub async fn on_order_confirmed(&self, cmd: OrderConfirmed) -> EventResult<SeamOutcome> {
        if cmd.registrations.is_empty() {
            return Err(super::event_error::EventError::Validation(
                "SalesOrderConfirmed carries no attendee specs — nothing to mint".into(),
            ));
        }
        let specs: Vec<RegisterCommand> = cmd
            .registrations
            .iter()
            .map(|spec| RegisterCommand {
                event_id: spec.event_id,
                event_slot_id: spec.event_slot_id,
                event_ticket_id: spec.event_ticket_id,
                name: spec.name.clone(),
                email: spec.email.clone(),
                phone: spec.phone.clone(),
                company_name: spec.company_name.clone(),
                partner_id: spec.partner_id,
                actor: None, // the system actor — the seam has no officer
                // The mint arms the lead queue like any registration
                // (draft rows are not eligible until the paid heal, and
                // that heal re-arms on its own).
                lead_rule_skip: false,
            })
            .collect();
        self.repo
            .on_order_confirmed(
                &self.consumer,
                cmd.delivery_id.as_deref(),
                cmd.order_id,
                &cmd.grand_total,
                specs,
                None,
            )
            .await
    }

    /// SalesOrderCancelled -> the linked-group cascade (mirror flips,
    /// whole group to cancel; sale_status KEPT; booths untouched).
    pub async fn on_order_cancelled(&self, cmd: OrderCancelled) -> EventResult<SeamOutcome> {
        self.repo
            .on_order_cancelled(&self.consumer, cmd.delivery_id.as_deref(), cmd.order_id, None)
            .await
    }

    /// The paid fact -> the ES-4 heal (to_pay -> sold, draft/cancel ->
    /// open armed) + the booth is_paid one-way latch.
    pub async fn on_order_paid(&self, cmd: OrderPaid) -> EventResult<SeamOutcome> {
        self.repo
            .on_order_paid(
                &self.consumer,
                cmd.delivery_id.as_deref(),
                cmd.order_id,
                &cmd.line_ids,
                None,
            )
            .await
    }
}
