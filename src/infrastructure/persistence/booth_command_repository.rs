//! The booth command repository (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! The transactional SQL of the booth verbs: create/patch/list/delete
//! (the EBS-5c delete fence), the REIFIED booking lifecycle (create
//! pending -> confirm -> release), the EBS-4 one-event-per-line
//! guard, and the one-way paid latch (also reachable through the sale
//! seam's paid verb).
//!
//! THE EXCLUSIVITY WALL IS THE DATABASE'S (EBS-1): `confirm_booking`
//! pre-checks availability only for the nice error — the partial
//! unique index `booths_confirmed_exclusivity` (UNIQUE(event_booth_id)
//! WHERE status='confirmed') is what holds under concurrency; a
//! concurrent loser's unique violation maps to the typed
//! `booth_already_confirmed` refusal (EBS-3's LOUD loser — the
//! collateral competitor-order-cancel is deliberately not ported).
//!
//! CONFIRM IS EXPLICIT (EBT-3): one verb, one transaction — the
//! booking flip, the five booth writes (state unavailable,
//! sale_order_line_id, partner/contact fill-if-empty per EBT-4) and
//! the audit row commit together or not at all.

use sqlx::PgPool;
use uuid::Uuid;

use crate::application::service::event_error::EventError;

use super::seat_repository::record_audit;

/// The booth row shape the verbs return.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct BoothRow {
    pub id: Uuid,
    pub event_id: Uuid,
    pub booth_category_id: Uuid,
    pub name: String,
    pub state: String,
    pub partner_id: Option<Uuid>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub sale_order_line_id: Option<Uuid>,
    pub is_paid: bool,
}

const BOOTH_COLUMNS: &str = "id, event_id, booth_category_id, name, state::text AS state, \
     partner_id, contact_name, contact_email, contact_phone, sale_order_line_id, is_paid";

/// The booking row shape.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct BoothBookingRow {
    pub id: Uuid,
    pub event_booth_id: Uuid,
    pub sale_order_line_id: Option<Uuid>,
    pub status: String,
    pub partner_id: Option<Uuid>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
}

const BOOKING_COLUMNS: &str = "id, event_booth_id, sale_order_line_id, status::text AS status, \
     partner_id, contact_name, contact_email, contact_phone";

/// Map a unique violation on the EBS-1 index to the LOUD typed loser.
fn map_exclusivity(e: sqlx::Error, event_booth_id: Uuid) -> EventError {
    match &e {
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505") => {
            EventError::BoothAlreadyConfirmed { event_booth_id }
        }
        _ => EventError::from(e),
    }
}

/// The booth command repository.
pub struct BoothCommandRepository {
    pool: PgPool,
}

impl BoothCommandRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a booth (born AVAILABLE — the EBT-1 default; the
    /// confirm verb is the only writer of 'unavailable').
    pub async fn create_booth(
        &self,
        event_id: Uuid,
        booth_category_id: Uuid,
        name: &str,
        actor: Option<Uuid>,
    ) -> Result<BoothRow, EventError> {
        let row = sqlx::query_as::<_, BoothRow>(&format!(
            r#"INSERT INTO event.booths (id, event_id, booth_category_id, name)
               VALUES ($1, $2, $3, $4)
               RETURNING {BOOTH_COLUMNS}"#
        ))
        .bind(Uuid::new_v4())
        .bind(event_id)
        .bind(booth_category_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| match &e {
            // FK violation: event or category does not resolve -> the
            // typed shape refusal, not a 500.
            sqlx::Error::Database(db) if db.code().as_deref() == Some("23503") => {
                EventError::Validation("booth create refused — event or category does not resolve".into())
            }
            _ => EventError::from(e),
        })?
        .ok_or_else(|| {
            EventError::Validation("booth create refused — event or category does not resolve".into())
        })?;
        record_audit(
            &self.pool,
            "booth_created",
            actor,
            "booth",
            row.id,
            serde_json::json!({ "event_id": event_id, "name": name }),
        )
        .await;
        Ok(row)
    }

    /// Patch a booth: name/category set directly; the contact fields
    /// and partner FILL-IF-EMPTY only (EBT-4 — an operator-entered
    /// contact is never clobbered).
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
    ) -> Result<BoothRow, EventError> {
        let row = sqlx::query_as::<_, BoothRow>(&format!(
            r#"UPDATE event.booths SET
                   name             = COALESCE($2, name),
                   booth_category_id = COALESCE($3, booth_category_id),
                   partner_id       = COALESCE(partner_id, $4),
                   contact_name     = COALESCE(contact_name, $5),
                   contact_email    = COALESCE(contact_email, $6),
                   contact_phone    = COALESCE(contact_phone, $7)
                WHERE id = $1
               RETURNING {BOOTH_COLUMNS}"#
        ))
        .bind(booth_id)
        .bind(name)
        .bind(booth_category_id)
        .bind(partner_id)
        .bind(contact_name)
        .bind(contact_email)
        .bind(contact_phone)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(EventError::BoothNotFound { booth_id })?;
        record_audit(
            &self.pool,
            "booth_updated",
            actor,
            "booth",
            booth_id,
            serde_json::json!({ "verb": "patch", "fill_if_empty": ["partner_id","contact_name","contact_email","contact_phone"] }),
        )
        .await;
        Ok(row)
    }

    /// DELETE — refused while sale-linked (EBS-5c) or while ANY booking
    /// row exists (the booking history releases first; the FK would
    /// otherwise be the wall — the verb says it in words).
    pub async fn delete_booth(&self, booth_id: Uuid, actor: Option<Uuid>) -> Result<(), EventError> {
        let mut tx = self.pool.begin().await?;
        let linked = sqlx::query_as::<_, (Option<Uuid>, i64)>(
            r#"SELECT b.sale_order_line_id,
                      (SELECT count(*) FROM event.booth_bookings bb WHERE bb.event_booth_id = b.id)
                 FROM event.booths b WHERE b.id = $1 FOR UPDATE"#,
        )
        .bind(booth_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(EventError::BoothNotFound { booth_id })?;
        let (sale_order_line_id, bookings) = linked;
        if sale_order_line_id.is_some() {
            return Err(EventError::BoothDeleteRefusedSaleLinked { booth_id });
        }
        if bookings > 0 {
            return Err(EventError::Validation(format!(
                "booth delete refused — {bookings} booking rows exist (release first)"
            )));
        }
        sqlx::query("DELETE FROM event.booths WHERE id = $1")
            .bind(booth_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        record_audit(
            &self.pool,
            "booth_deleted",
            actor,
            "booth",
            booth_id,
            serde_json::json!({}),
        )
        .await;
        Ok(())
    }

    /// Officer reads.
    pub async fn find_booth(&self, booth_id: Uuid) -> Result<BoothRow, EventError> {
        sqlx::query_as::<_, BoothRow>(&format!(
            "SELECT {BOOTH_COLUMNS} FROM event.booths WHERE id = $1"
        ))
        .bind(booth_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(EventError::BoothNotFound { booth_id })
    }

    pub async fn list_booths_of_event(
        &self,
        event_id: Uuid,
        limit: i64,
    ) -> Result<Vec<BoothRow>, EventError> {
        sqlx::query_as::<_, BoothRow>(&format!(
            "SELECT {BOOTH_COLUMNS} FROM event.booths WHERE event_id = $1 ORDER BY name, id LIMIT $2"
        ))
        .bind(event_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(EventError::from)
    }

    // ── the booking lifecycle ───────────────────────────────────────────────

    /// Create a PENDING booking intent (no exclusivity — pending rows
    /// never collide). The EBS-4 guard runs when a line ref is given:
    /// one order line books booths of ONE event.
    pub async fn create_booking(
        &self,
        event_booth_id: Uuid,
        sale_order_line_id: Option<Uuid>,
        partner_id: Option<Uuid>,
        contact_name: Option<&str>,
        contact_email: Option<&str>,
        contact_phone: Option<&str>,
        actor: Option<Uuid>,
    ) -> Result<BoothBookingRow, EventError> {
        let mut tx = self.pool.begin().await?;
        let event_id: Uuid = sqlx::query_scalar::<_, Uuid>(
            "SELECT event_id FROM event.booths WHERE id = $1",
        )
        .bind(event_booth_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(EventError::BoothNotFound {
            booth_id: event_booth_id,
        })?;
        if let Some(line) = sale_order_line_id {
            let cross: Option<Uuid> = sqlx::query_scalar::<_, Uuid>(
                r#"SELECT b.event_id FROM event.booth_bookings bb
                     JOIN event.booths b ON b.id = bb.event_booth_id
                    WHERE bb.sale_order_line_id = $1 AND b.event_id <> $2
                    LIMIT 1"#,
            )
            .bind(line)
            .bind(event_id)
            .fetch_optional(&mut *tx)
            .await?;
            if cross.is_some() {
                return Err(EventError::BoothBookingLineCrossEvent {
                    sale_order_line_id: line,
                });
            }
        }
        let row = sqlx::query_as::<_, BoothBookingRow>(&format!(
            r#"INSERT INTO event.booth_bookings
                   (id, event_booth_id, sale_order_line_id, partner_id,
                    contact_name, contact_email, contact_phone)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING {BOOKING_COLUMNS}"#
        ))
        .bind(Uuid::new_v4())
        .bind(event_booth_id)
        .bind(sale_order_line_id)
        .bind(partner_id)
        .bind(contact_name)
        .bind(contact_email)
        .bind(contact_phone)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| map_exclusivity(e, event_booth_id))?;
        tx.commit().await?;
        record_audit(
            &self.pool,
            "booth_booking_created",
            actor,
            "booth",
            event_booth_id,
            serde_json::json!({ "booking_id": row.id, "status": "pending" }),
        )
        .await;
        Ok(row)
    }

    /// CONFIRM (EBT-3) — the one transaction: booking pending ->
    /// confirmed (the EBS-1 wall), then the five booth writes
    /// (unavailable + line/partner/contacts fill-if-empty, EBT-4), and
    /// the audit row. The pre-check is the nice error; the index is
    /// the wall; the concurrent loser hears `booth_already_confirmed`.
    pub async fn confirm_booking(
        &self,
        booking_id: Uuid,
        actor: Option<Uuid>,
    ) -> Result<(BoothBookingRow, BoothRow), EventError> {
        let mut tx = self.pool.begin().await?;

        // Lock the booking + its booth together.
        let locked = sqlx::query_as::<_, (Uuid, String)>(
            r#"SELECT bb.event_booth_id, b.state::text
                 FROM event.booth_bookings bb
                 JOIN event.booths b ON b.id = bb.event_booth_id
                WHERE bb.id = $1
                FOR UPDATE OF bb, b"#,
        )
        .bind(booking_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(EventError::BoothBookingNotFound { booking_id })?;
        let (event_booth_id, booth_state) = locked;

        // The demoted Python pre-check (EBS-1): nice error, not the wall.
        if booth_state != "available" {
            return Err(EventError::BoothAlreadyConfirmed { event_booth_id });
        }

        // The flip — the partial unique index holds the wall.
        let booking = sqlx::query_as::<_, BoothBookingRow>(&format!(
            r#"UPDATE event.booth_bookings SET status = 'confirmed'
                WHERE id = $1 AND status = 'pending'
               RETURNING {BOOKING_COLUMNS}"#
        ))
        .bind(booking_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| map_exclusivity(e, event_booth_id))?;

        // The five booth writes (fill-if-empty for the contact family).
        let booth = sqlx::query_as::<_, BoothRow>(&format!(
            r#"UPDATE event.booths SET
                   state = 'unavailable',
                   sale_order_line_id = COALESCE(sale_order_line_id, $2),
                   partner_id   = COALESCE(partner_id,   $3),
                   contact_name = COALESCE(contact_name, $4),
                   contact_email = COALESCE(contact_email, $5),
                   contact_phone = COALESCE(contact_phone, $6)
                WHERE id = $1
               RETURNING {BOOTH_COLUMNS}"#
        ))
        .bind(event_booth_id)
        .bind(booking.sale_order_line_id)
        .bind(booking.partner_id)
        .bind(&booking.contact_name)
        .bind(&booking.contact_email)
        .bind(&booking.contact_phone)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            r#"INSERT INTO event.event_audit_log (event, actor, subject_type, subject_id, detail)
               VALUES ('booth_confirmed', $1, 'booth', $2, $3)"#,
        )
        .bind(actor)
        .bind(event_booth_id)
        .bind(serde_json::json!({ "booking_id": booking_id }))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((booking, booth))
    }

    /// RELEASE — the human verb (unguarded and silent by design:
    /// SalesOrderCancelled NEVER triggers it). Deletes the booth's
    /// booking rows (the audit row is the durable trace — the receipts
    /// pattern), returns the booth to available. `is_paid` keeps its
    /// one-way latch and the booth delete fence stays armed while any
    /// booking was sale-linked: release reopens BOOKING, it does not
    /// rewrite history.
    pub async fn release_booth(
        &self,
        event_booth_id: Uuid,
        actor: Option<Uuid>,
    ) -> Result<BoothRow, EventError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM event.booth_bookings WHERE event_booth_id = $1")
            .bind(event_booth_id)
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query_as::<_, BoothRow>(&format!(
            r#"UPDATE event.booths SET state = 'available'
                WHERE id = $1 AND state = 'unavailable'
               RETURNING {BOOTH_COLUMNS}"#
        ))
        .bind(event_booth_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(EventError::BoothNotFound {
            booth_id: event_booth_id,
        })?;
        sqlx::query(
            r#"INSERT INTO event.event_audit_log (event, actor, subject_type, subject_id, detail)
               VALUES ('booth_released', $1, 'booth', $2, $3)"#,
        )
        .bind(actor)
        .bind(event_booth_id)
        .bind(serde_json::json!({ "verb": "release" }))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Delete one PENDING booking (an intent withdrawn before confirm;
    /// confirmed rows release through the release verb).
    pub async fn delete_booking(
        &self,
        booking_id: Uuid,
        actor: Option<Uuid>,
    ) -> Result<(), EventError> {
        let outcome = sqlx::query_scalar::<_, Uuid>(
            "DELETE FROM event.booth_bookings WHERE id = $1 AND status = 'pending' RETURNING id",
        )
        .bind(booking_id)
        .fetch_optional(&self.pool)
        .await?;
        if outcome.is_none() {
            // Missing or not pending — both refuse the same way (a
            // confirmed booking releases through the release verb).
            return Err(EventError::BoothBookingNotFound { booking_id });
        }
        record_audit(
            &self.pool,
            "booth_booking_deleted",
            actor,
            "booth_booking",
            booking_id,
            serde_json::json!({ "verb": "withdraw_intent" }),
        )
        .await;
        Ok(())
    }

    pub async fn find_booking(
        &self,
        booking_id: Uuid,
    ) -> Result<BoothBookingRow, EventError> {
        sqlx::query_as::<_, BoothBookingRow>(&format!(
            "SELECT {BOOKING_COLUMNS} FROM event.booth_bookings WHERE id = $1"
        ))
        .bind(booking_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(EventError::BoothBookingNotFound { booking_id })
    }

    pub async fn list_bookings_of_booth(
        &self,
        event_booth_id: Uuid,
    ) -> Result<Vec<BoothBookingRow>, EventError> {
        sqlx::query_as::<_, BoothBookingRow>(&format!(
            "SELECT {BOOKING_COLUMNS} FROM event.booth_bookings WHERE event_booth_id = $1 ORDER BY id"
        ))
        .bind(event_booth_id)
        .fetch_all(&self.pool)
        .await
        .map_err(EventError::from)
    }

    /// The paid-hook latch (also reachable through the seam's paid
    /// verb): one-way, keyed on the order's lines.
    pub async fn mark_booths_paid(&self, line_ids: &[Uuid]) -> Result<usize, EventError> {
        if line_ids.is_empty() {
            return Ok(0);
        }
        let rows = sqlx::query(
            r#"UPDATE event.booths SET is_paid = true
                WHERE sale_order_line_id = ANY($1) AND NOT is_paid
               RETURNING id"#,
        )
        .bind(line_ids)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.len())
    }
}
