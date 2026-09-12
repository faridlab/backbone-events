//! The sale seam repository (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! The transactional SQL of the selling-domain subscription verbs
//! (ES-3/ES-4 + the cancel cascade + the paid-hook family). The module
//! NEVER parses selling's outbox envelopes itself — the HOST bridge
//! does (the buying-consumers precedent) and hands over the TYPED
//! commands defined in `sale_seam_service`.
//!
//! EXACTLY-ONCE PER DELIVERY: every verb first claims its delivery in
//! `event.seam_inbox` (INSERT .. ON CONFLICT DO NOTHING, inside the
//! SAME transaction as its effect — no row claimed, no effect run; a
//! redelivery is a typed no-op that reports `already_applied: true`).
//! The inbox key is the delivery id when the host supplies one, else
//! `"<event-type>:<order-id>"` (the replay-idempotent fallback).
//!
//! THE MINT (ES-3) rides THE ONE REGISTER HEAD (seat_repository's
//! `register_sale_linked`): the locks, the count, the typed capacity
//! refusal and the insert are the one path's — this repo OWNS the
//! transaction (the seat head runs inside it, on this repo's pool
//! handle) so claim + mint + mirrors + audit commit atomically.
//!
//! HEAL-FORWARD-ONLY (ES-2): `open`/`done` rows are NEVER demoted by
//! the driver — recompute arms and lifts, it never pushes a holding
//! row back to draft/cancel (only the explicit cancel cascade does,
//! and only on SalesOrderCancelled).
//!
//! MANUAL OVERRIDE PRESERVED: the four hand verbs stay available on
//! sale-linked rows; the seam only heals FORWARD (an officer's done
//! stamp survives every recompute).

use sqlx::PgPool;
use uuid::Uuid;

use crate::application::service::event_error::EventError;

use super::seat_repository::{record_audit, RegisterCommand, SaleLink, SeatRepository};

/// The outcome of one seam verb call.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SeamOutcome {
    pub already_applied: bool,
    pub registrations_touched: usize,
    pub booths_latched_paid: usize,
}

/// The zero-amount fact of the ES-1 truth table. The carrier's
/// `grand_total` arrives as its raw decimal STRING; this reads only
/// the zero/non-zero magnitude (the ONLY decision the mint makes from
/// it). Unparseable input classifies as PAID — the fail-safe arm holds
/// the seats rather than giving them away.
pub fn grand_total_is_zero(raw: &str) -> bool {
    match raw.trim().parse::<f64>() {
        Ok(v) => v == 0.0,
        Err(_) => false,
    }
}

/// The sale seam repository.
pub struct SaleSeamRepository {
    pool: PgPool,
}

impl SaleSeamRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Claim a delivery exactly once. Returns false when the delivery
    /// was already consumed (the caller returns the no-op outcome).
    async fn claim(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        consumer: &str,
        external_id: &str,
    ) -> Result<bool, EventError> {
        let claimed = sqlx::query_scalar::<_, i64>(
            r#"INSERT INTO event.seam_inbox (consumer, external_id)
               VALUES ($1, $2)
               ON CONFLICT (consumer, external_id) DO NOTHING
               RETURNING 1::int8"#,
        )
        .bind(consumer)
        .bind(external_id)
        .fetch_optional(&mut **tx)
        .await?
        .is_some();
        Ok(claimed)
    }

    /// The exactly-once key of one delivery: the host's delivery id
    /// when the bridge supplies one, else the (verb, order) pair — the
    /// replay-idempotent fallback. The delivery id is the TRUE key: a
    /// re-delivered fact re-consumes its own row whatever the order
    /// has been through since, while the fallback would swallow a
    /// second fact of the same verb+order (e.g. a cancelled order
    /// re-confirmed) as already applied.
    fn claim_key(verb: &str, delivery_id: Option<&str>, order_id: Uuid) -> String {
        match delivery_id {
            Some(d) => format!("{verb}:{d}"),
            None => format!("{verb}:{order_id}"),
        }
    }

    /// SalesOrderConfirmed — THE MINT + THE HEAL (ES-3).
    ///
    /// No linked registrations yet -> MINT the order's attendee specs
    /// through THE ONE register head: `grand_total` of (numerically)
    /// zero -> born `open` + mirrors (`sale`, `free`) + arms; anything
    /// else -> born `draft` + mirrors (`sale`, `to_pay`), HELD, no arm
    /// (ES-4: the draft -> open heal fires on the PAID fact, never at
    /// confirm).
    ///
    /// Already-linked registrations -> heal only (the mint replays as
    /// nothing; the idempotence is the linked-group read itself):
    /// free -> draft/cancel heal to open + arm; paid -> rows are held
    /// (draft stays draft; a cancelled row of an unpaid order stays
    /// cancelled — coherent holding), mirrors set to (sale, to_pay).
    pub async fn on_order_confirmed(
        &self,
        consumer_key: &str,
        delivery_id: Option<&str>,
        order_id: Uuid,
        grand_total: &str,
        specs: Vec<RegisterCommand>,
        actor: Option<Uuid>,
    ) -> Result<SeamOutcome, EventError> {
        let free = grand_total_is_zero(grand_total);
        let mut tx = self.pool.begin().await?;
        super::relay_ambient_scope(&mut tx).await?;

        if !Self::claim(
            &mut tx,
            consumer_key,
            &Self::claim_key("confirmed", delivery_id, order_id),
        )
        .await?
        {
            tx.commit().await?;
            return Ok(SeamOutcome {
                already_applied: true,
                registrations_touched: 0,
                booths_latched_paid: 0,
            });
        }

        // Does this order already own registrations? (The replay
        // question — decided by the rows, never by the caller.)
        let linked: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM event.registrations WHERE sale_order_id = $1",
        )
        .bind(order_id)
        .fetch_one(&mut *tx)
        .await?;

        let mut touched = 0usize;
        if linked == 0 {
            // THE MINT — every spec through the ONE seat head, carrying
            // its birth linkage, INSIDE this delivery transaction: a
            // typed capacity refusal on ANY spec aborts the whole
            // delivery (the inbox claim rolls back with it, so a later
            // retry after seats free up can land cleanly).
            let link = SaleLink {
                sale_order_id: order_id,
                sale_order_state: "sale".to_string(),
                sale_status: if free { "free" } else { "to_pay" }.to_string(),
                initial_state: if free { "open" } else { "draft" }.to_string(),
            };
            for spec in &specs {
                SeatRepository::register_core(&mut tx, spec, Some(&link)).await?;
                touched += 1;
            }
            tx.commit().await?;
            record_audit(
                &self.pool,
                "sale_seam_confirmed",
                actor,
                "sale_order",
                order_id,
                serde_json::json!({
                    "verb": "mint",
                    "free": free,
                    "minted": touched,
                    "sale_status": link.sale_status,
                }),
            )
            .await;
        } else {
            // THE HEAL — forward-only, group semantics. Free orders
            // lift draft/cancel rows into open (arming them); paid
            // orders HOLD (the paid fact will do the lifting).
            let detail = if free {
                let rows = sqlx::query(
                    r#"UPDATE event.registrations
                          SET state = 'open',
                              sale_order_state = 'sale',
                              sale_status = 'free'
                        WHERE sale_order_id = $1
                          AND state IN ('draft','cancel')
                          AND active
                       RETURNING id"#,
                )
                .bind(order_id)
                .fetch_all(&mut *tx)
                .await?;
                touched = rows.len();
                // The arm rule for every healed row's event (the heal
                // enters open — DRAFT-HEAL fires).
                sqlx::query(
                    r#"UPDATE event.mails m SET scheduled_date = now(), mail_done = false
                        WHERE m.interval_kind = 'after_sub'
                          AND m.event_id IN (
                              SELECT DISTINCT event_id FROM event.registrations
                               WHERE sale_order_id = $1)"#,
                )
                .bind(order_id)
                .execute(&mut *tx)
                .await?;
                serde_json::json!({
                    "verb": "heal",
                    "free": true,
                    "healed_to_open": touched,
                })
            } else {
                let rows = sqlx::query(
                    r#"UPDATE event.registrations
                          SET sale_order_state = 'sale',
                              sale_status = 'to_pay'
                        WHERE sale_order_id = $1 AND active
                       RETURNING id"#,
                )
                .bind(order_id)
                .fetch_all(&mut *tx)
                .await?;
                touched = rows.len();
                serde_json::json!({
                    "verb": "heal",
                    "free": false,
                    "held": touched,
                    "note": "held at draft until the paid fact",
                })
            };
            record_audit(
                &self.pool,
                "sale_seam_confirmed",
                actor,
                "sale_order",
                order_id,
                detail,
            )
            .await;
            tx.commit().await?;
        }

        Ok(SeamOutcome {
            already_applied: false,
            registrations_touched: touched,
            booths_latched_paid: 0,
        })
    }

    /// SalesOrderCancelled — the CASCADE: the whole linked group moves
    /// to `cancel`; the mirror flips to `cancel`; `sale_status` is
    /// KEPT (the payability axis is history, not state). Booths are
    /// NOT touched (SO-cancel does not free a booth — register row;
    /// release stays a human verb).
    pub async fn on_order_cancelled(
        &self,
        consumer_key: &str,
        delivery_id: Option<&str>,
        order_id: Uuid,
        actor: Option<Uuid>,
    ) -> Result<SeamOutcome, EventError> {
        let mut tx = self.pool.begin().await?;
        super::relay_ambient_scope(&mut tx).await?;
        if !Self::claim(
            &mut tx,
            consumer_key,
            &Self::claim_key("cancelled", delivery_id, order_id),
        )
        .await?
        {
            tx.commit().await?;
            return Ok(SeamOutcome {
                already_applied: true,
                registrations_touched: 0,
                booths_latched_paid: 0,
            });
        }
        let rows = sqlx::query(
            r#"UPDATE event.registrations
                  SET state = 'cancel',
                      sale_order_state = 'cancel'
                WHERE sale_order_id = $1 AND active
               RETURNING id"#,
        )
        .bind(order_id)
        .fetch_all(&mut *tx)
        .await?;
        let touched = rows.len();
        record_audit(
            &self.pool,
            "sale_seam_cancelled",
            actor,
            "sale_order",
            order_id,
            serde_json::json!({ "verb": "cancel_cascade", "cancelled": touched }),
        )
        .await;
        tx.commit().await?;
        Ok(SeamOutcome {
            already_applied: false,
            registrations_touched: touched,
            booths_latched_paid: 0,
        })
    }

    /// The paid-hook family — THE ES-4 HEAL + THE BOOTH LATCH.
    ///
    /// Registrations: `to_pay` -> `sold` and the held rows heal
    /// forward (draft/cancel -> open, armed). Booths: every confirmed
    /// booking's booth of the order's lines latches `is_paid`
    /// one-way. No carrier exists at this pin (backbone-billing
    /// declares no events edge) — the verb is landed for the host to
    /// wire at compose; the line ids arrive typed from the bridge.
    pub async fn on_order_paid(
        &self,
        consumer_key: &str,
        delivery_id: Option<&str>,
        order_id: Uuid,
        line_ids: &[Uuid],
        actor: Option<Uuid>,
    ) -> Result<SeamOutcome, EventError> {
        let mut tx = self.pool.begin().await?;
        super::relay_ambient_scope(&mut tx).await?;
        if !Self::claim(
            &mut tx,
            consumer_key,
            &Self::claim_key("paid", delivery_id, order_id),
        )
        .await?
        {
            tx.commit().await?;
            return Ok(SeamOutcome {
                already_applied: true,
                registrations_touched: 0,
                booths_latched_paid: 0,
            });
        }

        // The registrations of the order: to_pay -> sold, held rows
        // heal forward (armed).
        let healed = sqlx::query(
            r#"UPDATE event.registrations
                  SET state = 'open',
                      sale_order_state = 'sale',
                      sale_status = 'sold'
                WHERE sale_order_id = $1
                  AND state IN ('draft','cancel')
                  AND sale_status = 'to_pay'
                  AND active
               RETURNING id"#,
        )
        .bind(order_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut touched = healed.len();
        sqlx::query(
            r#"UPDATE event.registrations
                  SET sale_status = 'sold'
                WHERE sale_order_id = $1 AND sale_status = 'to_pay' AND active"#,
        )
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
        // The arm rule (the heal entered open — DRAFT-HEAL fires).
        sqlx::query(
            r#"UPDATE event.mails m SET scheduled_date = now(), mail_done = false
                WHERE m.interval_kind = 'after_sub'
                  AND m.event_id IN (
                      SELECT DISTINCT event_id FROM event.registrations
                       WHERE sale_order_id = $1)"#,
        )
        .bind(order_id)
        .execute(&mut *tx)
        .await?;

        // The booth latch: one-way (WHERE NOT is_paid), keyed on the
        // order's lines.
        let mut latched = 0usize;
        if !line_ids.is_empty() {
            let rows = sqlx::query(
                r#"UPDATE event.booths SET is_paid = true
                    WHERE sale_order_line_id = ANY($1) AND NOT is_paid
                   RETURNING id"#,
            )
            .bind(line_ids)
            .fetch_all(&mut *tx)
            .await?;
            latched = rows.len();
        }

        record_audit(
            &self.pool,
            "sale_seam_paid",
            actor,
            "sale_order",
            order_id,
            serde_json::json!({
                "verb": "paid_heal",
                "healed": touched,
                "booths_latched": latched,
            }),
        )
        .await;
        tx.commit().await?;
        Ok(SeamOutcome {
            already_applied: false,
            registrations_touched: touched,
            booths_latched_paid: latched,
        })
    }
}
