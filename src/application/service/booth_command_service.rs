//! The booth verbs service (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! A thin composition over the booth command repository (services
//! hold no raw sqlx): the create/patch/delete fences, the booking
//! lifecycle verbs, and the one-way paid latch. The EBT-3 confirm
//! semantics (pre-check demoted, DB wall, loud loser) live in the
//! repository — this layer is the callable surface the host and the
//! probes compose.

use uuid::Uuid;

use super::event_error::EventResult;
use crate::infrastructure::persistence::booth_command_repository::{
    BoothBookingRow, BoothCommandRepository, BoothRow,
};

pub struct BoothCommandService {
    repo: BoothCommandRepository,
}

impl BoothCommandService {
    pub fn new(repo: BoothCommandRepository) -> Self {
        Self { repo }
    }

    pub async fn create_booth(
        &self,
        event_id: Uuid,
        booth_category_id: Uuid,
        name: &str,
        actor: Option<Uuid>,
    ) -> EventResult<BoothRow> {
        self.repo
            .create_booth(event_id, booth_category_id, name, actor)
            .await
    }

    pub async fn patch_booth(
        &self,
        booth_id: Uuid,
        name: Option<&str>,
        booth_category_id: Option<Uuid>,
        partner_id: Option<Uuid>,
        contact_name: Option<&str>,
        contact_email: Option<&str>,
        contact_phone: Option<&str>,
        actor: Option<Uuid>,
    ) -> EventResult<BoothRow> {
        self.repo
            .patch_booth(
                booth_id,
                name,
                booth_category_id,
                partner_id,
                contact_name,
                contact_email,
                contact_phone,
                actor,
            )
            .await
    }

    pub async fn delete_booth(&self, booth_id: Uuid, actor: Option<Uuid>) -> EventResult<()> {
        self.repo.delete_booth(booth_id, actor).await
    }

    pub async fn find_booth(&self, booth_id: Uuid) -> EventResult<BoothRow> {
        self.repo.find_booth(booth_id).await
    }

    pub async fn list_booths_of_event(
        &self,
        event_id: Uuid,
        limit: i64,
    ) -> EventResult<Vec<BoothRow>> {
        self.repo.list_booths_of_event(event_id, limit).await
    }

    pub async fn create_booking(
        &self,
        event_booth_id: Uuid,
        sale_order_line_id: Option<Uuid>,
        partner_id: Option<Uuid>,
        contact_name: Option<&str>,
        contact_email: Option<&str>,
        contact_phone: Option<&str>,
        actor: Option<Uuid>,
    ) -> EventResult<BoothBookingRow> {
        self.repo
            .create_booking(
                event_booth_id,
                sale_order_line_id,
                partner_id,
                contact_name,
                contact_email,
                contact_phone,
                actor,
            )
            .await
    }

    /// CONFIRM (EBT-3): the explicit verb — never an order-arrival
    /// side effect.
    pub async fn confirm_booking(
        &self,
        booking_id: Uuid,
        actor: Option<Uuid>,
    ) -> EventResult<(BoothBookingRow, BoothRow)> {
        self.repo.confirm_booking(booking_id, actor).await
    }

    /// RELEASE: the human verb — a SalesOrderCancelled delivery never
    /// calls this (register row).
    pub async fn release_booth(
        &self,
        event_booth_id: Uuid,
        actor: Option<Uuid>,
    ) -> EventResult<BoothRow> {
        self.repo.release_booth(event_booth_id, actor).await
    }

    pub async fn delete_booking(&self, booking_id: Uuid, actor: Option<Uuid>) -> EventResult<()> {
        self.repo.delete_booking(booking_id, actor).await
    }

    pub async fn find_booking(&self, booking_id: Uuid) -> EventResult<BoothBookingRow> {
        self.repo.find_booking(booking_id).await
    }

    pub async fn list_bookings_of_booth(
        &self,
        event_booth_id: Uuid,
    ) -> EventResult<Vec<BoothBookingRow>> {
        self.repo.list_bookings_of_booth(event_booth_id).await
    }

    /// The one-way paid latch (WHERE NOT is_paid — nothing clears it).
    pub async fn mark_booths_paid(&self, line_ids: &[Uuid]) -> EventResult<usize> {
        self.repo.mark_booths_paid(line_ids).await
    }
}
