// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// The shared-audit-trail funnel. Inside the markers so a regeneration
// cannot drop the declaration and leave the file orphaned.
pub mod audit;
// Hand-written repositories carrying the verbs' transactional SQL
// (user-owned; see metaphor.codegen.yaml). The generated CRUD
// repositories keep their own files; these are the verb layer's.
pub mod seat_repository;
pub mod event_command_repository;
pub mod scheduler_repository;
pub mod sale_seam_repository;
pub mod booth_command_repository;
pub mod lead_command_repository;
pub use seat_repository::{RegisterCommand, RegistrationRow, SaleLink, SeatCounts, SeatRepository};
/// Re-bind the ambient org scope onto a connection this module opened
/// itself (a fresh pool transaction). With no ambient scope the
/// transaction stays plain — the module never invents one.
pub(crate) async fn relay_ambient_scope(
    conn: &mut sqlx::PgConnection,
) -> Result<(), sqlx::Error> {
    if let Some(scope) = backbone_orm::org_scope::current_org_scope() {
        backbone_orm::org_scope::bind_org_scope_on(conn, &scope).await?;
    }
    Ok(())
}
pub use event_command_repository::{
    CreateEventInput, EventCommandRepository, EventRow, PatchEventInput,
};
pub use scheduler_repository::{DueReceipt, SchedulerRepository, SchedulerRow};
pub use sale_seam_repository::{SaleSeamRepository, SeamOutcome};
pub use booth_command_repository::{BoothBookingRow, BoothCommandRepository, BoothRow};
pub use lead_command_repository::{
    EligibleRegistration, LeadCommandRepository, LeadPredicateRow, LeadRequestRow, LeadRuleRow,
    PredicateInput,
};
