//! The seat-truth repository (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! Services hold no raw sqlx (the DDD boundary): the transactional
//! SQL of the ONE seat-taking path lives here and the services
//! compose it.
//!
//! THE REGISTER TRANSACTION (EBB-1 — the only seat-taking path in
//! the module; a second counter anywhere is the frozen W8 seam
//! refusal):
//!
//! ```text
//! BEGIN;
//!   SELECT * FROM event.events WHERE id = $event FOR UPDATE;   -- lock 1
//!   [multi-slot] SELECT * FROM event.slots
//!       WHERE id = $slot AND event_id = $event FOR UPDATE;     -- lock 2
//!   [ticket] SELECT sale window (read-time lazy predicate);
//!   SELECT count(*) FROM event.registrations
//!       WHERE event_id = $event [AND event_slot_id = $slot]
//!         AND state IN ('open','done') AND active;             -- the count
//!   refuse typed when limited AND count >= cap;                -- then insert
//!   INSERT INTO event.registrations (... state 'open', active, barcode);
//!   UPDATE event.mails SET scheduled_date = now()              -- ARM (cheap
//!       WHERE event_id = $event AND interval_kind='after_sub'  -- rows; the
//!         AND NOT mail_done;                                   -- pass NEVER
//!                                                               -- runs inline)
//!   INSERT audit row;
//! COMMIT;
//! ```
//!
//! Lock order fixed: event → slot → (ticket read) → insert. The
//! counting domain is ALWAYS `state IN ('open','done') AND active`
//! — archived rows leave the seat count and the mail eligibility
//! together (EVM2-4 fixed by construction).
//!
//! Capacity semantics: `seats_limited = false` (or `seats_max = 0`)
//! is UNLIMITED; single-slot events count over the event; multi-slot
//! events count over the slot with the per-slot cap `seats_max`
//! (event total = seats_max x event_slot_count).

use chrono::{DateTime, Utc};
use rand::RngCore;
use sqlx::PgPool;
use uuid::Uuid;

use crate::application::service::event_error::EventError;

/// The command for the ONE register verb.
#[derive(Debug, Clone)]
pub struct RegisterCommand {
    pub event_id: Uuid,
    pub event_slot_id: Option<Uuid>,
    pub event_ticket_id: Option<Uuid>,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub company_name: Option<String>,
    pub partner_id: Option<Uuid>,
    pub actor: Option<Uuid>,
    /// The bulkops import exemption: the registration verbs normally ARM
    /// the per-event lead-generation queue (a cheap row write, never an
    /// inline run); a bulk import sets this to skip the arm (the queue
    /// still catches up on the next non-import trigger or rule change).
    pub lead_rule_skip: bool,
}

/// The sale linkage a minted registration is BORN with (ES-3 — the
/// sale seam's mint rides the SAME register head; this is not a second
/// seat-taking path, it is the one path carrying its birth linkage).
#[derive(Debug, Clone)]
pub struct SaleLink {
    pub sale_order_id: Uuid,
    /// The mirrored order state at mint: 'sale' (confirmed) today.
    pub sale_order_state: String,
    /// The payability pair member: 'free' (zero amount) | 'to_pay'.
    pub sale_status: String,
    /// The birth state: 'open' when free, 'draft' when held for payment.
    pub initial_state: String,
}

/// The row shape the register verb returns (hand-owned projection —
/// the generated entity is not round-tripped through the verb).
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct RegistrationRow {
    pub id: Uuid,
    pub event_id: Uuid,
    pub event_slot_id: Option<Uuid>,
    pub event_ticket_id: Option<Uuid>,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub company_name: Option<String>,
    pub partner_id: Option<Uuid>,
    pub state: String,
    pub date_closed: Option<DateTime<Utc>>,
    pub sale_order_id: Option<Uuid>,
    pub sale_order_state: Option<String>,
    pub sale_status: Option<String>,
    pub active: bool,
    pub barcode: String,
    pub company_id: Option<Uuid>,
}

/// The seat-count read (`seat_availability` over the same domain).
#[derive(Debug, Clone, Copy)]
pub struct SeatCounts {
    pub limited: bool,
    /// 0 = unlimited when `limited` is false.
    pub capacity: i64,
    pub taken: i64,
}

impl SeatCounts {
    pub fn available(&self) -> i64 {
        if !self.limited || self.capacity == 0 {
            i64::MAX
        } else {
            (self.capacity - self.taken).max(0)
        }
    }
}

/// Mint the registration barcode: the decimal of 8 urandom bytes,
/// little-endian (Code128C compact). Globally unique across events —
/// the UNIQUE index on `barcode` deliberately carries no event scope
/// (one scanner desk serves every event).
pub fn mint_barcode() -> String {
    let mut bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut bytes);
    u64::from_le_bytes(bytes).to_string()
}

/// Best-effort audit row (typed refusals are durable facts; an audit
/// write must never mask the original outcome).
pub async fn record_audit(
    pool: &PgPool,
    kind: &str,
    actor: Option<Uuid>,
    subject_type: &str,
    subject_id: Uuid,
    detail: serde_json::Value,
) {
    let _ = sqlx::query(
        r#"INSERT INTO event.event_audit_log (event, actor, subject_type, subject_id, detail)
           VALUES ($1::event_audit_event, $2, $3, $4, $5)"#,
    )
    .bind(kind)
    .bind(actor)
    .bind(subject_type)
    .bind(subject_id)
    .bind(detail)
    .execute(pool)
    .await;
}

/// The seat repository: the ONE register transaction + the seat
/// reads + the four one-liner state verbs.
pub struct SeatRepository {
    pool: PgPool,
}

/// The locked event row (the register transaction's first arm).
#[derive(Debug, Clone, sqlx::FromRow)]
struct LockedEvent {
    #[sqlx(rename = "id")]
    _id: Uuid,
    is_multi_slots: bool,
    seats_limited: bool,
    seats_max: i32,
    kanban_state: String,
    company_id: Option<Uuid>,
}

impl SeatRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// THE ONE REGISTER VERB — lock-first, count-then-insert, one
    /// transaction. See the module doc for the full sequence.
    pub async fn register(&self, cmd: &RegisterCommand) -> Result<RegistrationRow, EventError> {
        let mut tx = self.pool.begin().await?;
        let row = Self::register_core(&mut tx, cmd, None).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// The sale seam's mint (ES-3): the SAME register head carrying its
    /// birth linkage. NOT a second seat-taking path — the locks, the
    /// count, the refusal and the insert are the one path's; only the
    /// born state and the mirror columns differ (free -> born open +
    /// armed; paid -> born draft, held, NOT armed).
    pub async fn register_sale_linked(
        &self,
        cmd: &RegisterCommand,
        link: &SaleLink,
    ) -> Result<RegistrationRow, EventError> {
        let mut tx = self.pool.begin().await?;
        let row = Self::register_core(&mut tx, cmd, Some(link)).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// The one register head over a CALLER-OWNED connection — this is
    /// how the sale seam keeps delivery-claim + mint + mirrors + audit
    /// in ONE transaction (its `on_order_confirmed` opens the
    /// transaction and runs every spec through this core before the
    /// single commit). An `Err` return rolls back everything the caller
    /// staged, inbox claim included.
    pub async fn register_core(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        cmd: &RegisterCommand,
        link: Option<&SaleLink>,
    ) -> Result<RegistrationRow, EventError> {

        // Lock 1: the event row.
        let event = sqlx::query_as::<_, LockedEvent>(
            r#"SELECT id, is_multi_slots, seats_limited, seats_max, kanban_state::text AS kanban_state, company_id
                 FROM event.events WHERE id = $1 FOR UPDATE"#,
        )
        .bind(cmd.event_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(EventError::EventNotFound)?;

        if event.kanban_state == "cancel" {
            return Err(EventError::Validation(
                "event is cancelled — registrations refused".to_string(),
            ));
        }

        // The multi-slot slot: mandatory + belonging-checked + locked.
        let slot_id = if event.is_multi_slots {
            let slot = cmd.event_slot_id.ok_or(EventError::EventSlotRequired {
                event_id: cmd.event_id,
            })?;
            // Lock 2: the slot row (only after the event row — fixed
            // order). Locking the ROW (never count(*) — FOR UPDATE
            // cannot ride an aggregate) also IS the belonging check:
            // no row under this event, typed refusal.
            let belongs = sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM event.slots WHERE id = $1 AND event_id = $2 FOR UPDATE",
            )
            .bind(slot)
            .bind(cmd.event_id)
            .fetch_optional(&mut **tx)
            .await?;
            if belongs.is_none() {
                return Err(EventError::EventSlotNotOfEvent { event_slot_id: slot });
            }
            Some(slot)
        } else {
            cmd.event_slot_id
        };

        // The ticket tier: belonging + the read-time lazy sale window.
        if let Some(ticket) = cmd.event_ticket_id {
            let window = sqlx::query_as::<_, (Option<DateTime<Utc>>, Option<DateTime<Utc>>)>(
                "SELECT start_sale_datetime, end_sale_datetime FROM event.tickets WHERE id = $1 AND event_id = $2",
            )
            .bind(ticket)
            .bind(cmd.event_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(EventError::EventTicketNotOfEvent { event_ticket_id: ticket })?;
            let (start, end) = window;
            let now = Utc::now();
            let shut = start.map(|s| now < s).unwrap_or(false)
                || end.map(|e| now > e).unwrap_or(false);
            if shut {
                return Err(EventError::EventSaleWindowClosed {
                    event_ticket_id: ticket,
                });
            }
        }

        // The ONE count (the counting domain is always the pair).
        let taken: i64 = match slot_id {
            Some(slot) => {
                sqlx::query_scalar::<_, i64>(
                    r#"SELECT count(*) FROM event.registrations
                        WHERE event_id = $1 AND event_slot_id = $2
                          AND state IN ('open','done') AND active"#,
                )
                .bind(cmd.event_id)
                .bind(slot)
                .fetch_one(&mut **tx)
                .await?
            }
            None => {
                sqlx::query_scalar::<_, i64>(
                    r#"SELECT count(*) FROM event.registrations
                        WHERE event_id = $1
                          AND state IN ('open','done') AND active"#,
                )
                .bind(cmd.event_id)
                .fetch_one(&mut **tx)
                .await?
            }
        };

        // Refuse typed at capacity (0 cap while limited = unlimited is
        // a configuration error the count guard catches separately).
        if event.seats_limited && event.seats_max > 0 && taken >= event.seats_max as i64 {
            return Err(EventError::EventSeatsExhausted {
                event_id: cmd.event_id,
            });
        }

        // Then insert (default open — there is NO auto_confirm path;
        // a paid sale mint is born DRAFT and held, never auto-confirmed).
        let born_state = link.map(|l| l.initial_state.as_str()).unwrap_or("open");
        let id = Uuid::new_v4();
        let barcode = mint_barcode();
        let row = sqlx::query_as::<_, RegistrationRow>(
            r#"INSERT INTO event.registrations
                   (id, event_id, event_slot_id, event_ticket_id, name, email, phone,
                    company_name, partner_id, state, active, barcode, company_id,
                    sale_order_id, sale_order_state, sale_status)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $12::event_registration_state,
                       true, $10, $11, $13, $14::event_sale_order_state, $15::event_sale_status)
               RETURNING id, event_id, event_slot_id, event_ticket_id, name, email, phone,
                         company_name, partner_id, state::text AS state, date_closed,
                         sale_order_id, sale_order_state::text AS sale_order_state,
                         sale_status::text AS sale_status, active, barcode, company_id"#,
        )
        .bind(id)
        .bind(cmd.event_id)
        .bind(slot_id)
        .bind(cmd.event_ticket_id)
        .bind(&cmd.name)
        .bind(&cmd.email)
        .bind(&cmd.phone)
        .bind(&cmd.company_name)
        .bind(cmd.partner_id)
        .bind(&barcode)
        .bind(event.company_id)
        .bind(born_state)
        .bind(link.map(|l| l.sale_order_id))
        .bind(link.map(|l| l.sale_order_state.as_str()))
        .bind(link.map(|l| l.sale_status.as_str()))
        .fetch_one(&mut **tx)
        .await?;

        // ARM the after_sub engines — ONLY for a row born INTO the
        // eligible set (born open). A held mint (born draft) does NOT
        // arm: its engines arm at the paid-fact heal, exactly once.
        // Cheap in-transaction row updates only — the scheduler pass
        // NEVER runs inline here. A row entering the eligible set also
        // RE-OPENS any completed scheduler (the receipt-truth recompute
        // inside the pass is what closes it again).
        if born_state == "open" {
            sqlx::query(
                r#"UPDATE event.mails SET scheduled_date = now(), mail_done = false
                    WHERE event_id = $1 AND interval_kind = 'after_sub'"#,
            )
            .bind(cmd.event_id)
            .execute(&mut **tx)
            .await?;
        }

        // ARM the lead-generation queue (the on_create/on_confirm
        // axes): cheap row write, never an inline run; the bulkops
        // import exemption skips it.
        if !cmd.lead_rule_skip {
            sqlx::query(
                r#"INSERT INTO event.lead_requests (event_id)
                   SELECT $1 WHERE EXISTS (
                       SELECT 1 FROM event.lead_rules lr
                        WHERE lr.active
                          AND (lr.event_id IS NULL OR lr.event_id = $1)
                          AND (lr.on_create OR lr.on_confirm))
                   ON CONFLICT (event_id) DO UPDATE SET done = false WHERE lead_requests.done"#,
            )
            .bind(cmd.event_id)
            .execute(&mut **tx)
            .await?;
        }

        // The durable creation fact.
        sqlx::query(
            r#"INSERT INTO event.event_audit_log (event, actor, subject_type, subject_id, detail)
               VALUES ('registration_created', $1, 'registration', $2, $3)"#,
        )
        .bind(cmd.actor)
        .bind(row.id)
        .bind(serde_json::json!({
            "event_id": cmd.event_id,
            "email": cmd.email,
            "state": born_state,
            "sale_minted": link.is_some(),
        }))
        .execute(&mut **tx)
        .await?;

        // NOTE: no commit here — the CALLER owns the transaction (the
        // public wrappers commit their own; the sale seam commits its
        // delivery transaction with every staged mint inside it).
        Ok(row)
    }

    /// The seat-count read over the SAME counting domain the verb
    /// guards (one aggregator, one domain).
    pub async fn seat_counts(
        &self,
        event_id: Uuid,
        slot_id: Option<Uuid>,
    ) -> Result<SeatCounts, EventError> {
        let event = sqlx::query_as::<_, LockedEvent>(
            r#"SELECT id, is_multi_slots, seats_limited, seats_max, kanban_state::text AS kanban_state, company_id
                 FROM event.events WHERE id = $1"#,
        )
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(EventError::EventNotFound)?;

        let taken: i64 = match slot_id {
            Some(slot) => {
                sqlx::query_scalar::<_, i64>(
                    r#"SELECT count(*) FROM event.registrations
                        WHERE event_id = $1 AND event_slot_id = $2
                          AND state IN ('open','done') AND active"#,
                )
                .bind(event_id)
                .bind(slot)
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_scalar::<_, i64>(
                    r#"SELECT count(*) FROM event.registrations
                        WHERE event_id = $1 AND state IN ('open','done') AND active"#,
                )
                .bind(event_id)
                .fetch_one(&self.pool)
                .await?
            }
        };
        Ok(SeatCounts {
            limited: event.seats_limited,
            capacity: event.seats_max as i64,
            taken,
        })
    }

    /// One of the four one-liner state verbs (any -> any, no guard,
    /// no monotonicity). Returns `(before, after, active)` so the
    /// caller applies the ARM RULE: entering 'open' from draft/cancel
    /// arms the after_sub engines; re-confirming an open row never
    /// re-arms; open -> done never re-arms.
    pub async fn transition(
        &self,
        registration_id: Uuid,
        to: &str,
        actor: Option<Uuid>,
    ) -> Result<(String, String, bool), EventError> {
        let mut tx = self.pool.begin().await?;
        let outcome = sqlx::query_as::<_, (String, String, bool)>(
            r#"WITH prev AS (
                   SELECT state FROM event.registrations WHERE id = $1 FOR UPDATE
               )
               UPDATE event.registrations r
                   SET state = $2::event_registration_state,
                       date_closed = CASE
                           WHEN $2 = 'done' AND r.date_closed IS NULL THEN now()
                           ELSE r.date_closed END
               FROM prev
               WHERE r.id = $1
               RETURNING prev.state::text AS before_state,
                         r.state::text AS after_state,
                         r.active"#,
        )
        .bind(registration_id)
        .bind(to)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(EventError::RegistrationNotFound)?;

        // THE SEAT-HEAD — a transition INTO the holding domain
        // ('open'/'done' from 'draft'/'cancel' on an active row) takes
        // a seat, so it runs the SAME lock-first count-then-proceed
        // head that guards register(): lock the event row, count the
        // pair over the seat domain, refuse typed at capacity. There
        // is no second seat-taking path — an admin confirm is the
        // register head wearing an admin verb.
        //
        // Lock order note: this locks the registration row (the CTE
        // above) BEFORE the event row — register() locks the event row
        // first but never locks an existing registration row, so no
        // cycle exists between the two orders.
        let (before, after, active) = &outcome;
        let takes_seat = *active
            && (*after == "open" || *after == "done")
            && (*before == "draft" || *before == "cancel");
        if takes_seat {
            let (event_id, slot_id) = sqlx::query_as::<_, (Uuid, Option<Uuid>)>(
                "SELECT event_id, event_slot_id FROM event.registrations WHERE id = $1",
            )
            .bind(registration_id)
            .fetch_one(&mut *tx)
            .await?;
            let event = sqlx::query_as::<_, LockedEvent>(
                r#"SELECT id, is_multi_slots, seats_limited, seats_max, kanban_state::text AS kanban_state, company_id
                     FROM event.events WHERE id = $1 FOR UPDATE"#,
            )
            .bind(event_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(EventError::EventNotFound)?;

            // The ONE count, same counting domain as register() minus
            // the row under transition (its state is already staged in
            // this transaction).
            let taken: i64 = match slot_id {
                Some(slot) => {
                    sqlx::query_scalar::<_, i64>(
                        r#"SELECT count(*) FROM event.registrations
                            WHERE event_id = $1 AND event_slot_id = $2
                              AND id != $3
                              AND state IN ('open','done') AND active"#,
                    )
                    .bind(event_id)
                    .bind(slot)
                    .bind(registration_id)
                    .fetch_one(&mut *tx)
                    .await?
                }
                None => {
                    sqlx::query_scalar::<_, i64>(
                        r#"SELECT count(*) FROM event.registrations
                            WHERE event_id = $1
                              AND id != $2
                              AND state IN ('open','done') AND active"#,
                    )
                    .bind(event_id)
                    .bind(registration_id)
                    .fetch_one(&mut *tx)
                    .await?
                }
            };
            if event.seats_limited && event.seats_max > 0 && taken >= event.seats_max as i64 {
                // Returning Err drops the transaction — the staged
                // state change rolls back with it.
                return Err(EventError::EventSeatsExhausted { event_id });
            }
        }

        // The arm rule, stated once, here: any transition INTO open
        // from draft/cancel arms AND re-opens (the lazy receipt
        // materialization picks the row up inside the pass); open ->
        // open and open -> done never re-arm.
        if *active && after == "open" && (before == "draft" || before == "cancel") {
            sqlx::query(
                r#"UPDATE event.mails m SET scheduled_date = now(), mail_done = false
                    WHERE m.event_id = (SELECT event_id FROM event.registrations WHERE id = $1)
                      AND m.interval_kind = 'after_sub'"#,
            )
            .bind(registration_id)
            .execute(&mut *tx)
            .await?;
        }

        // The lead-generation queue arms the same way (on_confirm /
        // on_done axes): entering open from draft/cancel is the
        // confirm arm; entering done is the done arm. Cheap row write
        // only — generation NEVER runs inline.
        if *active && (after == "open" || after == "done") && before != after {
            let axis = if after == "done" { "on_done" } else { "on_confirm" };
            sqlx::query(
                r#"INSERT INTO event.lead_requests (event_id)
                   SELECT r.event_id FROM event.registrations r
                    WHERE r.id = $1
                      AND EXISTS (
                          SELECT 1 FROM event.lead_rules lr
                           WHERE lr.active
                             AND (lr.event_id IS NULL OR lr.event_id = r.event_id)
                             AND (CASE WHEN $2 = 'on_done' THEN lr.on_done ELSE lr.on_confirm END))
                   ON CONFLICT (event_id) DO UPDATE SET done = false WHERE lead_requests.done"#,
            )
            .bind(registration_id)
            .bind(axis)
            .execute(&mut *tx)
            .await?;
        }
        if before != after {
            sqlx::query(
                r#"INSERT INTO event.event_audit_log (event, actor, subject_type, subject_id, detail)
                   VALUES ('registration_state_changed', $1, 'registration', $2, $3)"#,
            )
            .bind(actor)
            .bind(registration_id)
            .bind(serde_json::json!({ "before": before, "after": after }))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(outcome)
    }

    /// sync_from_partner: fill-only-when-empty identity fields (never
    /// overwrites a value already on the row).
    pub async fn sync_from_partner(
        &self,
        registration_id: Uuid,
        partner_id: Uuid,
        name: Option<&str>,
        phone: Option<&str>,
        company_name: Option<&str>,
        actor: Option<Uuid>,
    ) -> Result<RegistrationRow, EventError> {
        let row = sqlx::query_as::<_, RegistrationRow>(
            r#"UPDATE event.registrations SET
                   partner_id   = COALESCE(partner_id, $2),
                   name         = COALESCE(name, $3),
                   phone        = COALESCE(phone, $4),
                   company_name = COALESCE(company_name, $5)
               WHERE id = $1 AND active
               RETURNING id, event_id, event_slot_id, event_ticket_id, name, email, phone,
                         company_name, partner_id, state::text AS state, date_closed,
                         sale_order_id, sale_order_state::text AS sale_order_state,
                         sale_status::text AS sale_status, active, barcode, company_id"#,
        )
        .bind(registration_id)
        .bind(partner_id)
        .bind(name)
        .bind(phone)
        .bind(company_name)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(EventError::RegistrationNotFound)?;
        record_audit(
            &self.pool,
            "registration_updated",
            actor,
            "registration",
            registration_id,
            serde_json::json!({ "verb": "sync_from_partner", "partner_id": partner_id }),
        )
        .await;
        Ok(row)
    }

    /// EXACT-match barcode lookup (the desk verb's branch 1 read —
    /// EBG-1: the barcode is globally unique, so one row or none; no
    /// LIKE, no prefix, no event scope).
    pub async fn find_by_barcode(
        &self,
        barcode: &str,
    ) -> Result<Option<RegistrationRow>, EventError> {
        sqlx::query_as::<_, RegistrationRow>(
            r#"SELECT id, event_id, event_slot_id, event_ticket_id, name, email, phone,
                      company_name, partner_id, state::text AS state, date_closed,
                      sale_order_id, sale_order_state::text AS sale_order_state,
                      sale_status::text AS sale_status, active, barcode, company_id
                 FROM event.registrations WHERE barcode = $1"#,
        )
        .bind(barcode)
        .fetch_optional(&self.pool)
        .await
        .map_err(EventError::from)
    }

    /// Fetch one registration row (officer reads).
    pub async fn find_registration(
        &self,
        registration_id: Uuid,
    ) -> Result<RegistrationRow, EventError> {
        sqlx::query_as::<_, RegistrationRow>(
            r#"SELECT id, event_id, event_slot_id, event_ticket_id, name, email, phone,
                      company_name, partner_id, state::text AS state, date_closed,
                      sale_order_id, sale_order_state::text AS sale_order_state,
                      sale_status::text AS sale_status, active, barcode, company_id
                 FROM event.registrations WHERE id = $1"#,
        )
        .bind(registration_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(EventError::RegistrationNotFound)
    }

    /// List a slice of registrations for one event (officer reads).
    pub async fn list_registrations(
        &self,
        event_id: Uuid,
        limit: i64,
        after: Option<Uuid>,
    ) -> Result<Vec<RegistrationRow>, EventError> {
        sqlx::query_as::<_, RegistrationRow>(
            r#"SELECT id, event_id, event_slot_id, event_ticket_id, name, email, phone,
                      company_name, partner_id, state::text AS state, date_closed,
                      sale_order_id, sale_order_state::text AS sale_order_state,
                      sale_status::text AS sale_status, active, barcode, company_id
                 FROM event.registrations
                WHERE event_id = $1 AND ($2::uuid IS NULL OR id > $2)
                ORDER BY id LIMIT $3"#,
        )
        .bind(event_id)
        .bind(after)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(EventError::from)
    }
}
