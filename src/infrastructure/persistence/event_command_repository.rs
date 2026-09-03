//! The event repository (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! The transactional SQL of the event verbs: create (with the ONE
//! template-apply — the event type's type_mail rows are copied into
//! event-scoped scheduler rows exactly once, at create, and never
//! re-propagate), the PATCH whitelist application (the FENCE itself
//! is enforced in the service — this repo has no arm that can write
//! `is_published`/`date_publish`), publish/unpublish (the ONLY
//! writers of the fence pair), mark_done (writes the first pipe_end
//! stage by sequence), and the reads the capability surface needs.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::application::service::event_error::EventError;

use super::seat_repository::record_audit;

/// The event row shape the verbs return.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct EventRow {
    pub id: Uuid,
    pub name: String,
    pub event_type_id: Option<Uuid>,
    pub stage_id: Uuid,
    pub kanban_state: String,
    pub date_begin: DateTime<Utc>,
    pub date_end: DateTime<Utc>,
    pub date_tz: String,
    pub is_multi_slots: bool,
    pub event_slot_count: i32,
    pub seats_limited: bool,
    pub seats_max: i32,
    pub company_id: Option<Uuid>,
    pub badge_format: String,
    pub is_published: bool,
    pub date_publish: Option<DateTime<Utc>>,
}

/// The create input (officer verbs + probes; every field explicit).
#[derive(Debug, Clone, Default)]
pub struct CreateEventInput {
    pub name: String,
    pub event_type_id: Option<Uuid>,
    pub date_begin: DateTime<Utc>,
    pub date_end: DateTime<Utc>,
    pub date_tz: Option<String>,
    pub is_multi_slots: bool,
    pub event_slot_count: i32,
    pub seats_limited: bool,
    pub seats_max: i32,
    pub company_id: Option<Uuid>,
    pub organizer_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub address_id: Option<Uuid>,
    pub event_url: Option<String>,
    pub badge_format: Option<String>,
}

/// The patch input — every arm OPTIONAL; `None` = leave untouched.
/// The publication fence pair is DELIBERATELY ABSENT: a repo that
/// cannot write the pair cannot be tricked into writing it.
#[derive(Debug, Clone, Default)]
pub struct PatchEventInput {
    pub name: Option<String>,
    pub event_type_id: Option<Uuid>,
    pub stage_id: Option<Uuid>,
    pub date_begin: Option<DateTime<Utc>>,
    pub date_end: Option<DateTime<Utc>>,
    pub date_tz: Option<String>,
    pub is_multi_slots: Option<bool>,
    pub event_slot_count: Option<i32>,
    pub seats_limited: Option<bool>,
    pub seats_max: Option<i32>,
    pub company_id: Option<Uuid>,
    pub organizer_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub address_id: Option<Uuid>,
    pub event_url: Option<String>,
    pub badge_format: Option<String>,
}

const EVENT_COLUMNS: &str = "id, name, event_type_id, stage_id, kanban_state::text AS kanban_state, \
     date_begin, date_end, date_tz, is_multi_slots, event_slot_count, seats_limited, seats_max, \
     company_id, badge_format::text AS badge_format, is_published, date_publish";

pub struct EventCommandRepository {
    pool: PgPool,
}

impl EventCommandRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create an event + apply the type's mail templates EXACTLY ONCE
    /// (never re-propagates — a later type change does not fork
    /// scheduler rows onto existing events).
    pub async fn create(
        &self,
        input: &CreateEventInput,
        actor: Option<Uuid>,
    ) -> Result<EventRow, EventError> {
        let mut tx = self.pool.begin().await?;
        let stage_id = match sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM event.stages ORDER BY sequence, id LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?
        {
            Some(id) => id,
            None => {
                sqlx::query_scalar::<_, Uuid>(
                    "INSERT INTO event.stages (name, sequence) VALUES ('New', 1) RETURNING id",
                )
                .fetch_one(&mut *tx)
                .await?
            }
        };

        let id = Uuid::new_v4();
        let row = sqlx::query_as::<_, EventRow>(&format!(
            r#"INSERT INTO event.events
                   (id, name, event_type_id, stage_id, date_begin, date_end, date_tz,
                    is_multi_slots, event_slot_count, seats_limited, seats_max,
                    company_id, organizer_id, user_id, address_id, event_url, badge_format)
               VALUES ($1, $2, $3, $4, $5, $6, COALESCE($7, 'UTC'), $8, $9, $10, $11,
                       $12, $13, $14, $15, $16, COALESCE($17::event_badge_format, 'a4_french_fold'))
               RETURNING {EVENT_COLUMNS}"#
        ))
        .bind(id)
        .bind(&input.name)
        .bind(input.event_type_id)
        .bind(stage_id)
        .bind(input.date_begin)
        .bind(input.date_end)
        .bind(input.date_tz.as_deref())
        .bind(input.is_multi_slots)
        .bind(input.event_slot_count)
        .bind(input.seats_limited)
        .bind(input.seats_max)
        .bind(input.company_id)
        .bind(input.organizer_id)
        .bind(input.user_id)
        .bind(input.address_id)
        .bind(&input.event_url)
        .bind(input.badge_format.as_deref())
        .fetch_one(&mut *tx)
        .await?;

        // THE ONE TEMPLATE-APPLY: copy the type's scheduler templates
        // as event-scoped scheduler rows, and the type's booth rows as
        // event booths (WHITELIST: name + booth_category_id only — the
        // type never templates booking state, contacts, or sale
        // links). Runs ONLY here; never re-propagates.
        if let Some(type_id) = input.event_type_id {
            sqlx::query(
                r#"INSERT INTO event.mails
                       (event_id, interval_nbr, interval_unit, interval_kind,
                        notification_channel, scheduled_date, template_ref, template_kind)
                   SELECT $1, tm.interval_nbr, tm.interval_unit, tm.interval_kind,
                          tm.notification_channel, now(), tm.template_ref, tm.template_kind
                     FROM event.type_mails tm WHERE tm.event_type_id = $2"#,
            )
            .bind(id)
            .bind(type_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"INSERT INTO event.booths (id, event_id, booth_category_id, name)
                   SELECT gen_random_uuid(), $1, tb.booth_category_id, tb.name
                     FROM event.type_booths tb WHERE tb.event_type_id = $2"#,
            )
            .bind(id)
            .bind(type_id)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            r#"INSERT INTO event.event_audit_log (event, actor, subject_type, subject_id, detail)
               VALUES ('event_created', $1, 'event', $2, $3)"#,
        )
        .bind(actor)
        .bind(id)
        .bind(serde_json::json!({ "name": input.name, "event_type_id": input.event_type_id }))
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(row)
    }

    /// Apply a whitelisted patch (COALESCE semantics: absent arms
    /// leave the stored value). The fence pair is not patchable —
    /// not by this repo, not by any caller of it.
    pub async fn patch(
        &self,
        id: Uuid,
        patch: &PatchEventInput,
        actor: Option<Uuid>,
    ) -> Result<EventRow, EventError> {
        let row = sqlx::query_as::<_, EventRow>(&format!(
            r#"UPDATE event.events SET
                   name             = COALESCE($2, name),
                   event_type_id    = COALESCE($3, event_type_id),
                   stage_id         = COALESCE($4, stage_id),
                   date_begin       = COALESCE($5, date_begin),
                   date_end         = COALESCE($6, date_end),
                   date_tz          = COALESCE($7, date_tz),
                   is_multi_slots   = COALESCE($8, is_multi_slots),
                   event_slot_count = COALESCE($9, event_slot_count),
                   seats_limited    = COALESCE($10, seats_limited),
                   seats_max        = COALESCE($11, seats_max),
                   company_id       = COALESCE($12, company_id),
                   organizer_id     = COALESCE($13, organizer_id),
                   user_id          = COALESCE($14, user_id),
                   address_id       = COALESCE($15, address_id),
                   event_url        = COALESCE($16, event_url),
                   badge_format     = COALESCE($17::event_badge_format, badge_format)
                WHERE id = $1
               RETURNING {EVENT_COLUMNS}"#
        ))
        .bind(id)
        .bind(&patch.name)
        .bind(patch.event_type_id)
        .bind(patch.stage_id)
        .bind(patch.date_begin)
        .bind(patch.date_end)
        .bind(patch.date_tz.as_deref())
        .bind(patch.is_multi_slots)
        .bind(patch.event_slot_count)
        .bind(patch.seats_limited)
        .bind(patch.seats_max)
        .bind(patch.company_id)
        .bind(patch.organizer_id)
        .bind(patch.user_id)
        .bind(patch.address_id)
        .bind(&patch.event_url)
        .bind(patch.badge_format.as_deref())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(EventError::EventNotFound)?;
        record_audit(
            &self.pool,
            "event_updated",
            actor,
            "event",
            id,
            serde_json::json!({ "verb": "patch", "fenced": ["is_published", "date_publish"] }),
        )
        .await;
        Ok(row)
    }

    /// PUBLISH — the only writer that sets `is_published = true`.
    /// `date_publish` stamps now() on FIRST publish and is never
    /// rewritten on republish.
    pub async fn publish(&self, id: Uuid, actor: Option<Uuid>) -> Result<EventRow, EventError> {
        let row = sqlx::query_as::<_, EventRow>(&format!(
            r#"UPDATE event.events
                  SET is_published = true,
                      date_publish = COALESCE(date_publish, now())
                WHERE id = $1
               RETURNING {EVENT_COLUMNS}"#
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(EventError::EventNotFound)?;
        record_audit(&self.pool, "event_published", actor, "event", id, serde_json::json!({}))
            .await;
        Ok(row)
    }

    /// UNPUBLISH — the only writer that clears `is_published`
    /// (`date_publish` keeps the historical first-publish stamp).
    pub async fn unpublish(&self, id: Uuid, actor: Option<Uuid>) -> Result<EventRow, EventError> {
        let row = sqlx::query_as::<_, EventRow>(&format!(
            r#"UPDATE event.events SET is_published = false
                WHERE id = $1
               RETURNING {EVENT_COLUMNS}"#
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(EventError::EventNotFound)?;
        record_audit(&self.pool, "event_unpublished", actor, "event", id, serde_json::json!({}))
            .await;
        Ok(row)
    }

    /// MARK DONE — the verb: sets `done` AND moves the row to the
    /// FIRST pipe_end stage by sequence (upstream's two-axis close).
    pub async fn mark_done(&self, id: Uuid, actor: Option<Uuid>) -> Result<EventRow, EventError> {
        let mut tx = self.pool.begin().await?;
        let pipe_end = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM event.stages WHERE pipe_end ORDER BY sequence, id LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?;
        let row = sqlx::query_as::<_, EventRow>(&format!(
            r#"UPDATE event.events
                  SET kanban_state = 'done',
                      stage_id = COALESCE($2, stage_id)
                WHERE id = $1
               RETURNING {EVENT_COLUMNS}"#
        ))
        .bind(id)
        .bind(pipe_end)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(EventError::EventNotFound)?;
        sqlx::query(
            r#"INSERT INTO event.event_audit_log (event, actor, subject_type, subject_id, detail)
               VALUES ('event_mark_done', $1, 'event', $2, $3)"#,
        )
        .bind(actor)
        .bind(id)
        .bind(serde_json::json!({ "verb": "mark_done" }))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Fetch one event row.
    pub async fn find(&self, id: Uuid) -> Result<EventRow, EventError> {
        sqlx::query_as::<_, EventRow>(&format!(
            "SELECT {EVENT_COLUMNS} FROM event.events WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(EventError::EventNotFound)
    }

    /// List events (newest first), capped.
    pub async fn list(&self, limit: i64) -> Result<Vec<EventRow>, EventError> {
        sqlx::query_as::<_, EventRow>(&format!(
            "SELECT {EVENT_COLUMNS} FROM event.events ORDER BY date_begin DESC, id LIMIT $1"
        ))
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(EventError::from)
    }

    /// THE CATALOG READ (EP-2): the distinct product ids linked across
    /// the event family (tickets + booth categories), through the
    /// `event_linked_products` view. Events owns the linkage; the
    /// catalog consumes only this.
    pub async fn list_linked_products(&self) -> Result<Vec<Uuid>, EventError> {
        sqlx::query_scalar::<_, Uuid>("SELECT product_id FROM event.event_linked_products ORDER BY product_id")
            .fetch_all(&self.pool)
            .await
            .map_err(EventError::from)
    }

    /// The publication-checked event read for the capability surface:
    /// only a PUBLISHED, non-cancelled row resolves; unpublished,
    /// cancelled AND missing are all the ONE uniform not-published
    /// refusal (no oracle distinguishing members).
    pub async fn find_published(&self, id: Uuid) -> Result<EventRow, EventError> {
        let row = match self.find(id).await {
            Ok(row) => row,
            Err(EventError::EventNotFound) => return Err(EventError::EventNotPublished),
            Err(other) => return Err(other),
        };
        if !row.is_published || row.kanban_state == "cancel" {
            return Err(EventError::EventNotPublished);
        }
        Ok(row)
    }

    /// A slot's hours, but ONLY when the slot belongs to the named
    /// event (the ics slot-scope check + window read in one).
    pub async fn slot_window_of_event(
        &self,
        slot_id: Uuid,
        event_id: Uuid,
    ) -> Result<Option<(Uuid, DateTime<Utc>, DateTime<Utc>)>, EventError> {
        sqlx::query_as::<_, (Uuid, DateTime<Utc>, Option<DateTime<Utc>>)>(
            "SELECT id, date_begin, date_end FROM event.slots WHERE id = $1 AND event_id = $2",
        )
        .bind(slot_id)
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(EventError::from)
        .map(|row| {
            row.map(|(id, begin, end)| {
                (id, begin, end.unwrap_or(begin))
            })
        })
    }
}
