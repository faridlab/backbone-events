//! The scheduler repository (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! The transactional SQL of the self-arming communication scheduler
//! (ADR-0020): the SKIP LOCKED claim domain, the LAZY receipt
//! materialization (receipts are created per pass, chunked and
//! capped, for registrations entering the eligible set), the receipt
//! walk, the receipt-truth completion recompute, the cancellation
//! propagation, and the daily mark-done sweep.
//!
//! `mail_done` is NOT a hand-set flag: it is the RECEIPT-TRUTH
//! recompute — "every eligible registration has a sent (or visibly
//! dropped) receipt" — so a LATE registrant re-opens the scheduler
//! automatically (EBB-2). Eligibility is always the pair
//! `state IN ('open','done') AND active` (EVM2-4).
//!
//! `'sent' = queued`: a successful enqueue sets `mail_sent` from
//! this module's point of view; delivery belongs to the transport.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::application::service::event_error::EventError;

use super::seat_repository::record_audit;

/// One claimed scheduler row.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct SchedulerRow {
    pub id: Uuid,
    pub event_id: Uuid,
    pub interval_nbr: i32,
    pub interval_unit: String,
    pub interval_kind: String,
    pub template_ref: Option<Uuid>,
    pub template_kind: Option<String>,
    pub mail_done: bool,
    pub last_registration_id: Option<Uuid>,
}

/// One due receipt joined to its registration (the render context's
/// arms).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DueReceipt {
    pub receipt_id: Uuid,
    pub registration_id: Uuid,
    pub attendee_name: String,
    pub attendee_email: String,
    pub barcode: String,
    pub scheduled_date: Option<DateTime<Utc>>,
}

/// The scheduler repository.
pub struct SchedulerRepository {
    pool: PgPool,
}

impl SchedulerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// The claim domain: scheduler rows NOT done, DUE, on non-cancel
    /// events, with WORK REMAINING (an eligible registration without
    /// a sent receipt). `FOR UPDATE SKIP LOCKED` — concurrent hosts
    /// never double-walk a row.
    pub async fn claim_due(&self, limit: i64) -> Result<Vec<SchedulerRow>, EventError> {
        sqlx::query_as::<_, SchedulerRow>(
            r#"WITH claimed AS (
                   SELECT m.id FROM event.mails m
                     JOIN event.events e ON e.id = m.event_id
                    WHERE NOT m.mail_done
                      AND m.scheduled_date <= now()
                      AND e.kanban_state::text <> 'cancel'
                      AND EXISTS (
                          SELECT 1 FROM event.registrations r
                           WHERE r.event_id = m.event_id
                             AND r.state IN ('open','done') AND r.active
                             AND NOT EXISTS (
                                 SELECT 1 FROM event.mail_registrations mr
                                  WHERE mr.scheduler_id = m.id
                                    AND mr.registration_id = r.id
                                    AND mr.mail_sent))
                    ORDER BY m.scheduled_date
                    LIMIT $1
                    FOR UPDATE SKIP LOCKED
               )
               SELECT m.id, m.event_id, m.interval_nbr, m.interval_unit::text AS interval_unit,
                      m.interval_kind::text AS interval_kind, m.template_ref, m.template_kind,
                      m.mail_done, m.last_registration_id
                 FROM event.mails m JOIN claimed c ON c.id = m.id"#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(EventError::from)
    }

    /// LAZY receipt materialization: create the MISSING receipts for
    /// eligible registrations, capped per pass. The idempotence
    /// predicate is the ANTI-JOIN (`NOT EXISTS` a receipt for the
    /// pair) backed by the UNIQUE(scheduler_id, registration_id)
    /// constraint — materialized rows drop out of the missing set, so
    /// the anti-join self-paginates under the cap. The
    /// `last_registration_id` column is kept as a WATERMARK of the
    /// highest materialized id and is NEVER a filter: registration ids
    /// are random UUIDs, so a LATE registrant can sort below any
    /// watermark (a cursor filter would strand it forever).
    /// `scheduled_date` derives from the registration's creation stamp
    /// + the row's interval (`now` = due immediately).
    pub async fn materialize_receipts(
        &self,
        scheduler: &SchedulerRow,
        cap: i64,
    ) -> Result<Vec<Uuid>, EventError> {
        let mut tx = self.pool.begin().await?;
        let ids = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO event.mail_registrations (scheduler_id, registration_id, scheduled_date)
               SELECT m.id, r.id,
                      (r.metadata->>'created_at')::timestamptz +
                        CASE WHEN m.interval_unit::text = 'now'
                             THEN '0 seconds'::interval
                             ELSE (m.interval_nbr::text || ' ' || m.interval_unit::text)::interval
                        END
                 FROM event.mails m
                 JOIN event.registrations r ON r.event_id = m.event_id
                WHERE m.id = $1
                  AND r.state IN ('open','done') AND r.active
                  AND NOT EXISTS (
                      SELECT 1 FROM event.mail_registrations x
                       WHERE x.scheduler_id = m.id AND x.registration_id = r.id)
                ORDER BY r.id
                LIMIT $2
               RETURNING registration_id"#,
        )
        .bind(scheduler.id)
        .bind(cap)
        .fetch_all(&mut *tx)
        .await?;
        if let Some(last) = ids.last() {
            sqlx::query(
                "UPDATE event.mails SET last_registration_id = GREATEST(last_registration_id, $2) WHERE id = $1",
            )
            .bind(scheduler.id)
            .bind(last)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(ids)
    }

    /// How many eligible registrations still lack ANY receipt for
    /// this scheduler (the overflow signal for the re-arm rule).
    pub async fn unmaterialized_count(&self, scheduler_id: Uuid) -> Result<i64, EventError> {
        sqlx::query_scalar::<_, i64>(
            r#"SELECT count(*) FROM event.registrations r
                WHERE r.event_id = (SELECT event_id FROM event.mails WHERE id = $1)
                  AND r.state IN ('open','done') AND r.active
                  AND NOT EXISTS (
                      SELECT 1 FROM event.mail_registrations x
                       WHERE x.scheduler_id = $1 AND x.registration_id = r.id)"#,
        )
        .bind(scheduler_id)
        .fetch_one(&self.pool)
        .await
        .map_err(EventError::from)
    }

    /// The due receipts (scheduled, unsent, eligible). A NULL
    /// scheduled_date counts as due (the `now` unit).
    pub async fn due_receipts(
        &self,
        scheduler_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DueReceipt>, EventError> {
        sqlx::query_as::<_, DueReceipt>(
            r#"SELECT mr.id AS receipt_id, mr.registration_id, r.name AS attendee_name,
                      r.email AS attendee_email, r.barcode, mr.scheduled_date
                 FROM event.mail_registrations mr
                 JOIN event.registrations r ON r.id = mr.registration_id
                WHERE mr.scheduler_id = $1
                  AND NOT mr.mail_sent
                  AND (mr.scheduled_date IS NULL OR mr.scheduled_date <= now())
                  AND r.state IN ('open','done') AND r.active
                ORDER BY mr.scheduled_date NULLS FIRST, mr.registration_id
                LIMIT $2"#,
        )
        .bind(scheduler_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(EventError::from)
    }

    /// Mark one receipt `'sent' = queued` (the idempotent send guard:
    /// only the first writer flips the flag).
    pub async fn mark_receipt_queued(&self, receipt_id: Uuid) -> Result<(), EventError> {
        sqlx::query(
            "UPDATE event.mail_registrations SET mail_sent = true, outcome = 'queued' WHERE id = $1 AND NOT mail_sent",
        )
        .bind(receipt_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The VISIBLE drop (the window closed before the send): the
    /// receipt closes with the drop outcome rather than silently
    /// disappearing. Terminal — never retried.
    pub async fn mark_receipt_dropped_window_closed(
        &self,
        receipt_id: Uuid,
    ) -> Result<(), EventError> {
        sqlx::query(
            "UPDATE event.mail_registrations SET mail_sent = true, outcome = 'dropped_window_closed' WHERE id = $1 AND NOT mail_sent",
        )
        .bind(receipt_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record a typed scheduler failure (the family: template
    /// unresolved / renderer not composed / render failed / enqueue
    /// refused / recipient invalid). Recorded and CONTINUED — never a
    /// registration blocker, never a throttle.
    pub async fn record_failure(&self, scheduler_id: Uuid, kind: &str) -> Result<(), EventError> {
        sqlx::query(
            "UPDATE event.mails SET error_kind = $2::event_mail_error_kind, error_datetime = now() WHERE id = $1",
        )
        .bind(scheduler_id)
        .bind(kind)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Clear the failure marker after a fully successful pass.
    pub async fn clear_failure(&self, scheduler_id: Uuid) -> Result<(), EventError> {
        sqlx::query(
            "UPDATE event.mails SET error_kind = NULL, error_datetime = NULL WHERE id = $1",
        )
        .bind(scheduler_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// THE RECEIPT-TRUTH COMPLETION: `mail_done` is true iff every
    /// eligible registration carries a sent (or dropped) receipt. A
    /// late registrant re-opens the row automatically.
    pub async fn recompute_mail_done(&self, scheduler_id: Uuid) -> Result<bool, EventError> {
        let done: bool = sqlx::query_scalar::<_, bool>(
            r#"UPDATE event.mails m
                  SET mail_done = NOT EXISTS (
                          SELECT 1 FROM event.registrations r
                           WHERE r.event_id = m.event_id
                             AND r.state IN ('open','done') AND r.active
                             AND NOT EXISTS (
                                 SELECT 1 FROM event.mail_registrations mr
                                  WHERE mr.scheduler_id = m.id
                                    AND mr.registration_id = r.id
                                    AND mr.mail_sent))
                WHERE m.id = $1
               RETURNING mail_done"#,
        )
        .bind(scheduler_id)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(true);
        Ok(done)
    }

    /// The overflow re-arm: a pass hit its cap with work left.
    pub async fn rearm(&self, scheduler_id: Uuid) -> Result<(), EventError> {
        sqlx::query("UPDATE event.mails SET scheduled_date = now() WHERE id = $1 AND NOT mail_done")
            .bind(scheduler_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Cancellation propagation: pending (unsent) receipts of
    /// cancelled registrations are deleted on the next pass — the
    /// audit row is the durable trace.
    pub async fn propagate_cancellations(&self, limit: i64) -> Result<i64, EventError> {
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query_scalar::<_, Uuid>(
            r#"DELETE FROM event.mail_registrations mr
                WHERE mr.id IN (
                    SELECT x.id FROM event.mail_registrations x
                      JOIN event.registrations r ON r.id = x.registration_id
                     WHERE r.state = 'cancel' AND NOT x.mail_sent
                     LIMIT $1)
               RETURNING mr.scheduler_id"#,
        )
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?;
        let n = deleted.len() as i64;
        if n > 0 {
            record_audit(
                &self.pool,
                "scheduler_run",
                None,
                "mail_scheduler",
                Uuid::nil(),
                serde_json::json!({
                    "verb": "cancellation_propagation",
                    "deleted_pending_receipts": n,
                }),
            )
            .await;
        }
        tx.commit().await?;
        Ok(n)
    }

    /// Officer read: one scheduler row.
    pub async fn find(&self, scheduler_id: Uuid) -> Result<SchedulerRow, EventError> {
        sqlx::query_as::<_, SchedulerRow>(
            r#"SELECT id, event_id, interval_nbr, interval_unit::text AS interval_unit,
                      interval_kind::text AS interval_kind, template_ref, template_kind,
                      mail_done, last_registration_id
                 FROM event.mails WHERE id = $1"#,
        )
        .bind(scheduler_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(EventError::RegistrationNotFound)
        .map_err(|e| match e {
            EventError::RegistrationNotFound => EventError::EventNotFound,
            other => other,
        })
    }

    /// Officer read: an event's scheduler rows.
    pub async fn list_for_event(
        &self,
        event_id: Uuid,
    ) -> Result<Vec<SchedulerRow>, EventError> {
        sqlx::query_as::<_, SchedulerRow>(
            r#"SELECT id, event_id, interval_nbr, interval_unit::text AS interval_unit,
                      interval_kind::text AS interval_kind, template_ref, template_kind,
                      mail_done, last_registration_id
                 FROM event.mails WHERE event_id = $1 ORDER BY id"#,
        )
        .bind(event_id)
        .fetch_all(&self.pool)
        .await
        .map_err(EventError::from)
    }

    /// The daily done sweep (bounded batches, FOR UPDATE SKIP LOCKED
    /// claims): past events not already done/cancelled and not
    /// resting in a pipe_end stage move to `done` — the verb, run as
    /// a sweep. Returns the swept ids.
    pub async fn sweep_mark_done(&self, limit: i64) -> Result<Vec<Uuid>, EventError> {
        let mut tx = self.pool.begin().await?;
        let ids = sqlx::query_scalar::<_, Uuid>(
            r#"WITH claimed AS (
                   SELECT e.id FROM event.events e
                    WHERE e.kanban_state::text NOT IN ('done','cancel')
                      AND e.date_end < now()
                      AND NOT EXISTS (
                          SELECT 1 FROM event.stages s
                           WHERE s.id = e.stage_id AND s.pipe_end)
                    ORDER BY e.date_end
                    LIMIT $1
                    FOR UPDATE SKIP LOCKED
               )
               UPDATE event.events ev SET kanban_state = 'done'
                FROM claimed c WHERE ev.id = c.id
               RETURNING ev.id"#,
        )
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?;
        for id in &ids {
            sqlx::query(
                r#"INSERT INTO event.event_audit_log (event, actor, subject_type, subject_id, detail)
                   VALUES ('event_mark_done', NULL, 'event', $1, $2)"#,
            )
            .bind(id)
            .bind(serde_json::json!({ "verb": "done_sweep" }))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(ids)
    }
}
