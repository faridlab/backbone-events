//! The self-arming scheduler pass (hand-written; user-owned; see
//! `metaphor.codegen.yaml`) — ADR-0020.
//!
//! ONE pass over the claim domain (self-arming: registration verbs
//! arm with cheap row updates; the pass NEVER runs inline in a
//! verb). Per scheduler row, in order:
//!
//! 1. the template validity sweep — an empty template ref is the
//!    typed `template_unresolved`, recorded on the row, pass
//!    continues (never a throttle, never a registration blocker);
//! 2. the LAZY receipt materialization (chunked, capped — missing
//!    receipts for eligible registrations get their derived
//!    `scheduled_date` = creation + interval);
//! 3. the receipt walk: per due receipt — the window check (a
//!    finished event visibly DROPS the receipt with the
//!    `dropped_window_closed` outcome, never silently), the render
//!    (host port), the enqueue (host port; `'sent' = queued`), the
//!    idempotent `mail_sent` flip;
//! 4. `mail_done` recompute — the RECEIPT TRUTH (EBB-2): done iff
//!    every eligible registration carries a sent-or-dropped receipt;
//!    a LATE registrant re-opens the row on the next recompute;
//! 5. the overflow re-arm (T3) when work remains beyond the batch
//!    caps; cancellation propagation runs once per pass before the
//!    claims (pending receipts of cancelled registrations are
//!    deleted, audited).
//!
//! Typed failures (template_unresolved / template_renderer_not_
//! composed / render_failed / enqueue_refused / recipient_invalid)
//! are RECORDED (`error_kind`, `error_datetime`) and the pass
//! CONTINUES — the next tick retries; no failure wedges a scheduler.

use std::sync::Arc;

use uuid::Uuid;

use super::event_error::EventResult;
use super::sms_port::{EventSmsQueue, RenderedSms};
use super::template_port::{EventMailQueue, EventTemplateRenderer, RenderContext};
use crate::infrastructure::persistence::event_command_repository::EventCommandRepository;
use crate::infrastructure::persistence::scheduler_repository::{SchedulerRepository, SchedulerRow};

/// `EVENT_MAIL_BATCH` — receipts processed per scheduler per pass
/// (default 50).
pub const DEFAULT_MAIL_BATCH: i64 = 50;

/// `EVENT_MAIL_CRON_LIMIT` — scheduler rows claimed per pass
/// (default 1000).
pub const DEFAULT_CRON_LIMIT: i64 = 1000;

/// The visible outcome vocabulary of a receipt (closed set: queued |
/// dropped_window_closed).
pub const OUTCOME_QUEUED: &str = "queued";
pub const OUTCOME_DROPPED_WINDOW_CLOSED: &str = "dropped_window_closed";

/// One pass's summary (probe + surface shape).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SchedulerRunSummary {
    pub schedulers_claimed: i64,
    pub receipts_materialized: i64,
    pub receipts_queued: i64,
    pub sms_queued: i64,
    pub receipts_dropped_window_closed: i64,
    pub typed_failures_recorded: i64,
    pub cancellation_receipts_deleted: i64,
    pub schedulers_rearmed: i64,
    pub schedulers_completed: i64,
}

/// The scheduler service.
pub struct SchedulerService {
    schedulers: SchedulerRepository,
    events: EventCommandRepository,
    renderer: Arc<dyn EventTemplateRenderer>,
    queue: Arc<dyn EventMailQueue>,
    sms_queue: Arc<dyn EventSmsQueue>,
    batch: i64,
    cron_limit: i64,
}

impl SchedulerService {
    /// Compose with the host-installed ports (the refusing defaults
    /// give the unwired-host typed failures, never silent skips). The
    /// sms queue is its own port: rows whose derived channel is `sms`
    /// enqueue through it (recipient = the registration's phone); an
    /// unconfigured gateway parks the row loudly as
    /// `sms_enqueue_refused` — never silently queued.
    pub fn new(
        schedulers: SchedulerRepository,
        events: EventCommandRepository,
        renderer: Arc<dyn EventTemplateRenderer>,
        queue: Arc<dyn EventMailQueue>,
        sms_queue: Arc<dyn EventSmsQueue>,
    ) -> Self {
        Self {
            schedulers,
            events,
            renderer,
            queue,
            sms_queue,
            batch: DEFAULT_MAIL_BATCH,
            cron_limit: DEFAULT_CRON_LIMIT,
        }
    }

    /// Override the batch/cron caps (probes + host policy).
    pub fn with_caps(mut self, batch: i64, cron_limit: i64) -> Self {
        self.batch = batch.max(1);
        self.cron_limit = cron_limit.max(1);
        self
    }

    /// ONE PASS. Idempotent per receipt (the `mail_sent` flip guards
    /// the enqueue side); safe to run concurrently on two hosts
    /// (SKIP LOCKED claims; the flip is first-writer-wins).
    pub async fn run_due_schedulers(&self) -> EventResult<SchedulerRunSummary> {
        let mut summary = SchedulerRunSummary::default();

        // Cancellation propagation first: pending receipts of
        // cancelled registrations go away, audited.
        summary.cancellation_receipts_deleted = self
            .schedulers
            .propagate_cancellations(self.cron_limit)
            .await?;

        let claimed = self.schedulers.claim_due(self.cron_limit).await?;
        summary.schedulers_claimed = claimed.len() as i64;

        for scheduler in &claimed {
            self.run_one(scheduler, &mut summary).await?;
        }
        Ok(summary)
    }

    async fn run_one(&self, scheduler: &SchedulerRow, summary: &mut SchedulerRunSummary) -> EventResult<()> {
        // 1. The template validity sweep.
        if scheduler.template_ref.is_none() {
            self.schedulers
                .record_failure(scheduler.id, "template_unresolved")
                .await?;
            summary.typed_failures_recorded += 1;
            // Not done: recompute keeps the row open (receipts exist
            // unsent) — the next tick retries after the template is
            // fixed. Continue the pass.
            return Ok(());
        }

        // 2. Lazy receipt materialization (chunked + capped).
        summary.receipts_materialized += self
            .schedulers
            .materialize_receipts(scheduler, self.batch)
            .await?
            .len() as i64;

        // 3. The receipt walk.
        let event = self.events.find(scheduler.event_id).await?;
        loop {
            let due = self.schedulers.due_receipts(scheduler.id, self.batch).await?;
            if due.is_empty() {
                break;
            }
            for receipt in &due {
                // The window check: a finished event visibly drops the
                // receipt (terminal, audited in the outcome column).
                if event.date_end <= chrono::Utc::now() {
                    self.schedulers
                        .mark_receipt_dropped_window_closed(receipt.receipt_id)
                        .await?;
                    summary.receipts_dropped_window_closed += 1;
                    continue;
                }
                // Render through the host port (ONE renderer port for
                // both channels — the renderer receives the row's
                // template_kind and the context carries the phone arm).
                let ctx = RenderContext {
                    event_id: event.id,
                    event_name: event.name.clone(),
                    event_date_begin: event.date_begin,
                    event_date_end: event.date_end,
                    event_date_tz: event.date_tz.clone(),
                    registration_id: receipt.registration_id,
                    attendee_name: receipt.attendee_name.clone(),
                    attendee_email: receipt.attendee_email.clone(),
                    attendee_phone: receipt.attendee_phone.clone(),
                    registration_barcode: receipt.barcode.clone(),
                };
                let rendered = match self
                    .renderer
                    .render(scheduler.template_ref, scheduler.template_kind.as_deref(), &ctx)
                    .await
                {
                    Ok(mail) => mail,
                    Err(failure) => {
                        // Record the typed failure, CONTINUE the pass.
                        // Enqueue-refused class retries next tick.
                        let kind = match failure {
                            super::template_port::RenderFailure::TemplateUnresolved => {
                                "template_unresolved"
                            }
                            super::template_port::RenderFailure::RendererNotComposed => {
                                "template_renderer_not_composed"
                            }
                            super::template_port::RenderFailure::RenderFailed(_) => "render_failed",
                        };
                        self.schedulers.record_failure(scheduler.id, kind).await?;
                        summary.typed_failures_recorded += 1;
                        continue;
                    }
                };
                // THE CHANNEL BRANCH: the row's derived channel picks
                // the port (mail -> email -> EventMailQueue; sms ->
                // phone -> EventSmsQueue). 'sent' = queued on both.
                if scheduler.notification_channel == "sms" {
                    let phone = receipt
                        .attendee_phone
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("");
                    if phone.is_empty() {
                        self.schedulers
                            .record_failure(scheduler.id, "recipient_invalid")
                            .await?;
                        summary.typed_failures_recorded += 1;
                        continue;
                    }
                    let sms = RenderedSms {
                        body_text: rendered.body_text,
                    };
                    match self.sms_queue.enqueue(phone, &sms).await {
                        Ok(()) => {
                            self.schedulers
                                .mark_receipt_queued(receipt.receipt_id)
                                .await?;
                            summary.sms_queued += 1;
                        }
                        Err(_refusal) => {
                            // An unconfigured gateway is a typed, LOUD
                            // park: the receipt stays unsent, retried
                            // next tick (ESM-2 — never silently queued).
                            self.schedulers
                                .record_failure(scheduler.id, "sms_enqueue_refused")
                                .await?;
                            summary.typed_failures_recorded += 1;
                        }
                    }
                    continue;
                }
                if receipt.attendee_email.is_empty() || !receipt.attendee_email.contains('@') {
                    self.schedulers
                        .record_failure(scheduler.id, "recipient_invalid")
                        .await?;
                    summary.typed_failures_recorded += 1;
                    continue;
                }
                // The enqueue ('sent' = queued at success).
                match self.queue.enqueue(&receipt.attendee_email, &rendered).await {
                    Ok(()) => {
                        self.schedulers
                            .mark_receipt_queued(receipt.receipt_id)
                            .await?;
                        summary.receipts_queued += 1;
                    }
                    Err(refusal) => {
                        // Typed enqueue_refused recorded with detail —
                        // retried next tick (at-least-once declared).
                        let _ = refusal;
                        self.schedulers
                            .record_failure(scheduler.id, "enqueue_refused")
                            .await?;
                        summary.typed_failures_recorded += 1;
                    }
                }
            }
            // The loop exits when a full batch drains under the cap;
            // anything beyond re-arms below.
            if (due.len() as i64) < self.batch {
                break;
            }
        }

        // 4. The receipt-truth completion (late registrants re-open).
        let done = self.schedulers.recompute_mail_done(scheduler.id).await?;
        if done {
            summary.schedulers_completed += 1;
            self.schedulers.clear_failure(scheduler.id).await?;
        }

        // 5. The overflow re-arm (work remains beyond the caps).
        let unmaterialized = self.schedulers.unmaterialized_count(scheduler.id).await?;
        let still_due = self
            .schedulers
            .due_receipts(scheduler.id, 1)
            .await?
            .len() as i64;
        if !done && (unmaterialized > 0 || still_due > 0) {
            self.schedulers.rearm(scheduler.id).await?;
            summary.schedulers_rearmed += 1;
        }
        Ok(())
    }

    /// Officer verb: run ONE scheduler row now (the admin "run"
    /// button). The idempotent per-receipt guards make the bypass of
    /// the claim domain safe (worst case it duplicates nothing; the
    /// `mail_sent` flip is first-writer-wins).
    pub async fn run_scheduler_by_id(
        &self,
        scheduler_id: Uuid,
    ) -> EventResult<SchedulerRunSummary> {
        let mut summary = SchedulerRunSummary::default();
        let row = self.schedulers.find(scheduler_id).await?;
        summary.schedulers_claimed = 1;
        self.run_one(&row, &mut summary).await?;
        Ok(summary)
    }

    /// Officer read: an event's scheduler rows.
    pub async fn list_for_event(
        &self,
        event_id: Uuid,
    ) -> EventResult<Vec<crate::infrastructure::persistence::scheduler_repository::SchedulerRow>>
    {
        self.schedulers.list_for_event(event_id).await
    }

    /// The daily done sweep (bounded batches, SKIP LOCKED claims —
    /// the verb run as a sweep). Returns the swept event ids.
    pub async fn sweep_mark_done(&self, limit: i64) -> EventResult<Vec<Uuid>> {
        self.schedulers.sweep_mark_done(limit).await
    }

    /// THE TEMPLATE CASCADE (one verb, BOTH channels — EVM2-2 + ESM-1
    /// collapsed): the template store's delete declares this seam and
    /// calls it with the deleted (kind, ref) pair; dependent scheduler
    /// rows and type-level template rows of that pair go in one
    /// set-based transaction. Returns (scheduler rows deleted, type
    /// template rows deleted).
    pub async fn on_template_deleted(
        &self,
        template_kind: &str,
        template_ref: Uuid,
        actor: Option<Uuid>,
    ) -> EventResult<(i64, i64)> {
        if template_kind.trim().is_empty() {
            return Err(super::event_error::EventError::Validation(
                "template cascade requires a template_kind".into(),
            ));
        }
        self.schedulers
            .cascade_delete_template_dependents(template_kind, template_ref, actor)
            .await
    }
}
