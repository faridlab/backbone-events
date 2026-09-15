//! One place this module records an audited fact.
//!
//! The rows used to land in `event.event_audit_log`, a table only this module
//! could read. They now go to `auditlog.audit_trails`, the shared trail the
//! record-history and activity-feed surfaces read — so an event's changes are
//! visible beside every other module's, instead of in a corner of one schema.
//!
//! Two things are preserved deliberately:
//!
//! - **the actor**, passed explicitly. The trail's default reads `app.actor`
//!   from the session, which would attribute a sweep to `system` and a verb to
//!   whoever's request it rode, and both would be wrong when the caller was
//!   handed an actor.
//! - **the event vocabulary**, carried verbatim into `action`. It used to be
//!   constrained by the `event_audit_event` enum; the shared column is text, so
//!   the values survive but the constraint does not. Keep using the same names.

use uuid::Uuid;

/// Record one audited fact in the caller's transaction.
///
/// `subject_type` is normalised to the schema-qualified table the capture
/// trigger writes (`event.events`), so a row recorded by a verb and a row
/// recorded by a trigger key the same way and a per-record history finds both.
pub async fn record_audit(
    exec: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    action: &str,
    actor: Option<Uuid>,
    subject_type: &str,
    subject_id: Option<Uuid>,
    detail: serde_json::Value,
) -> Result<(), sqlx::Error> {
    backbone_auditlog::application::service::append(
        exec,
        backbone_auditlog::application::service::AuditEvent {
            event_type: backbone_auditlog::domain::entity::AuditEventType::DataChange,
            action: action.to_string(),
            subject_type: Some(if subject_type.contains('.') {
                subject_type.to_string()
            } else {
                format!("event.{subject_type}s")
            }),
            subject_id: subject_id.map(|id| id.to_string()),
            changed: Some(detail),
            reason: None,
            status: backbone_auditlog::domain::entity::AuditStatus::Success,
            actor: actor.map(|id| id.to_string()),
        },
    )
    .await
    .map(|_| ())
}
