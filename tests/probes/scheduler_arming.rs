//! The scheduler probes: the ARM RULE + `mail_done`-as-receipt-truth
//! (EBB-2) + the EVM2-4 archive pair.
//!
//! - registration arms the after_sub engines (cheap row updates;
//!   nothing executes inline);
//! - the pass materializes receipts lazily, enqueues them
//!   ('sent' = queued), and completes by RECEIPT TRUTH;
//! - a LATE registrant re-opens a completed scheduler (receipt truth,
//!   not a hand-set flag);
//! - re-confirming an already-open row never re-arms (no duplicate
//!   receipt, no second send);
//! - open -> done never re-arms;
//! - ARCHIVED rows (active = false) leave the seat count AND the
//!   mail eligibility together (EVM2-4);
//! - the refusing ports record their typed failures (renderer not
//!   composed, enqueue refused) and CONTINUE.

use std::sync::Arc;

use backbone_events::application::service::scheduler_service::SchedulerService;
use backbone_events::application::service::template_port::{
    EventTemplateRenderer, RenderContext, RenderFailure, RenderedMail,
};
use backbone_events::infrastructure::persistence::event_command_repository::EventCommandRepository;
use backbone_events::infrastructure::persistence::scheduler_repository::SchedulerRepository;

use super::common::{
    make_event, make_type_with_mail, register_cmd, registrations, scheduler, RecordingQueue,
    RefusingQueue, RefusingSmsQueue, StubRenderer, TestDb,
};
use async_trait::async_trait;

async fn scheduler_row(db: &TestDb, event_id: uuid::Uuid) -> (uuid::Uuid, bool) {
    let row = sqlx::query_as::<_, (uuid::Uuid, bool)>(
        "SELECT id, mail_done FROM event.mails WHERE event_id = $1 ORDER BY id LIMIT 1",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    row
}

async fn receipts(db: &TestDb, scheduler_id: uuid::Uuid) -> Vec<(uuid::Uuid, bool, Option<String>)> {
    sqlx::query_as(
        "SELECT registration_id, mail_sent, outcome FROM event.mail_registrations WHERE scheduler_id = $1 ORDER BY registration_id",
    )
    .bind(scheduler_id)
    .fetch_all(&db.pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn arming_receipt_truth_and_late_reopen() {
    let db = TestDb::new("arming").await;
    let (type_id, _template_id) = make_type_with_mail(&db, "now", 0).await;
    let event_id = make_event(&db, "probe arming", false, 0, Some(type_id)).await;
    let queue = Arc::new(RecordingQueue::default());
    let service = registrations(&db);

    // TWO registrations arm the engines (scheduled_date set at arm).
    let r1 = service.register(register_cmd(event_id, 1)).await.unwrap();
    let r2 = service.register(register_cmd(event_id, 2)).await.unwrap();
    let (mail_id, _) = scheduler_row(&db, event_id).await;
    let armed: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT scheduled_date FROM event.mails WHERE id = $1")
            .bind(mail_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(armed.is_some(), "registering ARMED the after_sub scheduler row");

    // The pass: both receipts materialize lazily + enqueue + the
    // receipt-truth completion flips mail_done.
    let run = SchedulerService::new(
        SchedulerRepository::new(db.pool.clone()),
        EventCommandRepository::new(db.pool.clone()),
        Arc::new(StubRenderer),
        queue.clone(),
        Arc::new(RefusingSmsQueue),
    )
    .run_due_schedulers()
    .await
    .unwrap();
    assert_eq!(run.receipts_queued, 2, "both registrants got their mail");
    assert_eq!(queue.sent.lock().unwrap().len(), 2);

    let (mail_id, done) = scheduler_row(&db, event_id).await;
    assert!(done, "completion is the receipt truth: all receipts sent");

    // The LATE registrant re-opens the completed scheduler.
    let r3 = service.register(register_cmd(event_id, 3)).await.unwrap();
    let _ = r3;
    let (_, done_after_late) = scheduler_row(&db, event_id).await;
    assert!(!done_after_late, "a late registrant RE-OPENS the scheduler (receipt truth)");

    // The next pass closes the gap.
    let run2 = scheduler(&db, queue.clone()).run_due_schedulers().await.unwrap();
    assert_eq!(run2.receipts_queued, 1, "the late registrant's receipt is new work");
    let (_, done_again) = scheduler_row(&db, event_id).await;
    assert!(done_again);

    // RE-CONFIRM NEVER RE-ARMS: confirm an already-open row — no new
    // receipt, no duplicate send, no state churn.
    let before = receipts(&db, mail_id).await;
    service.confirm(r1.id, None).await.unwrap();
    let run3 = scheduler(&db, queue.clone()).run_due_schedulers().await.unwrap();
    let after = receipts(&db, mail_id).await;
    assert_eq!(before.len(), after.len(), "re-confirm creates no receipt");
    assert_eq!(run3.receipts_queued, 0, "re-confirm sends nothing");
    assert_eq!(queue.sent.lock().unwrap().len(), 3, "total sends stay at three");

    // OPEN -> DONE never re-arms either (the row stays counted; no
    // mail consequence).
    service.set_done(r2.id, None).await.unwrap();
    let date_closed: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT date_closed FROM event.registrations WHERE id = $1")
            .bind(r2.id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(date_closed.is_some(), "set_done stamps date_closed when empty");
    let run4 = scheduler(&db, queue.clone()).run_due_schedulers().await.unwrap();
    assert_eq!(run4.receipts_queued, 0, "open -> done never re-arms");

    // CANCELLED rows: PENDING receipts are deleted on the next pass
    // (cancellation propagation, audited). Build the pending condition
    // honestly — r4's enqueue is refused, so its receipt materialized
    // but was never sent.
    let r4 = service.register(register_cmd(event_id, 4)).await.unwrap();
    let refused_run = SchedulerService::new(
        SchedulerRepository::new(db.pool.clone()),
        EventCommandRepository::new(db.pool.clone()),
        Arc::new(StubRenderer),
        Arc::new(RefusingQueue),
        Arc::new(RefusingSmsQueue),
    )
    .run_due_schedulers()
    .await
    .unwrap();
    assert_eq!(refused_run.typed_failures_recorded, 1, "r4's enqueue was refused");
    let pending_snapshot = receipts(&db, mail_id).await;
    assert!(
        pending_snapshot
            .iter()
            .any(|(rid, sent, _)| *rid == r4.id && !*sent),
        "the refused receipt stays PENDING (the retry posture)"
    );

    // Cancel r4: the next pass drops the PENDING receipt; the SENT
    // receipts of every other row survive (a sent receipt is history —
    // cancellation cannot unsend it).
    service.cancel(r4.id, None).await.unwrap();
    let run5 = scheduler(&db, queue.clone()).run_due_schedulers().await.unwrap();
    assert_eq!(
        run5.cancellation_receipts_deleted, 1,
        "the cancelled row's pending receipt is deleted"
    );
    let remaining = receipts(&db, mail_id).await;
    assert!(
        remaining.iter().all(|(rid, _, _)| *rid != r4.id),
        "the cancelled row's pending receipt is gone"
    );
    assert!(
        remaining.iter().any(|(rid, sent, _)| *rid == r1.id && *sent),
        "a SENT receipt is history: the cancelled row's sent receipt stays"
    );
    db.dispose().await;
}

/// The refusing-renderer: the typed renderer_not_composed failure is
/// RECORDED on the row and the pass continues (no throttle, no
/// crash).
struct NoRenderer;

#[async_trait]
impl EventTemplateRenderer for NoRenderer {
    async fn render(
        &self,
        _template_ref: Option<uuid::Uuid>,
        _template_kind: Option<&str>,
        _ctx: &RenderContext,
    ) -> Result<RenderedMail, RenderFailure> {
        Err(RenderFailure::RendererNotComposed)
    }
}

#[tokio::test]
async fn typed_failures_recorded_and_continued() {
    let db = TestDb::new("typedfail").await;
    let (type_id, _t) = make_type_with_mail(&db, "now", 0).await;
    let event_id = make_event(&db, "probe typed failures", false, 0, Some(type_id)).await;
    let service = registrations(&db);
    service.register(register_cmd(event_id, 1)).await.unwrap();

    // Renderer not composed: recorded, continued, retried next tick.
    let run = SchedulerService::new(
        SchedulerRepository::new(db.pool.clone()),
        EventCommandRepository::new(db.pool.clone()),
        Arc::new(NoRenderer),
        Arc::new(RecordingQueue::default()),
        Arc::new(RefusingSmsQueue),
    )
    .run_due_schedulers()
    .await
    .unwrap();
    assert_eq!(run.typed_failures_recorded, 1);
    let (_, done) = scheduler_row(&db, event_id).await;
    assert!(!done, "a typed failure leaves the scheduler open (retry)");
    let error_kind: Option<String> = sqlx::query_scalar(
        "SELECT error_kind::text FROM event.mails WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(error_kind.as_deref(), Some("template_renderer_not_composed"));

    // Enqueue refused: same posture (recorded, open, retried).
    let run2 = scheduler(&db, Arc::new(RefusingQueue)).run_due_schedulers().await.unwrap();
    assert_eq!(run2.typed_failures_recorded, 1);
    let error_kind: Option<String> = sqlx::query_scalar(
        "SELECT error_kind::text FROM event.mails WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(error_kind.as_deref(), Some("enqueue_refused"));
    db.dispose().await;
}

#[tokio::test]
async fn template_unresolved_when_no_template_ref() {
    // A scheduler row with NO template ref is the typed
    // template_unresolved (the validity sweep), never a silent skip.
    let db = TestDb::new("notemplate").await;
    let event_id = make_event(&db, "probe no-template", false, 0, None).await;
    sqlx::query(
        "INSERT INTO event.mails (event_id, interval_nbr, interval_unit, interval_kind, scheduled_date) VALUES ($1, 0, 'now', 'after_sub', now())",
    )
    .bind(event_id)
    .execute(&db.pool)
    .await
    .unwrap();
    registrations(&db).register(register_cmd(event_id, 1)).await.unwrap();

    let run = scheduler(&db, Arc::new(RecordingQueue::default()))
        .run_due_schedulers()
        .await
        .unwrap();
    assert_eq!(run.typed_failures_recorded, 1, "the empty template ref is typed");
    assert_eq!(run.receipts_queued, 0);
    let error_kind: Option<String> = sqlx::query_scalar(
        "SELECT error_kind::text FROM event.mails WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(error_kind.as_deref(), Some("template_unresolved"));
    db.dispose().await;
}

#[tokio::test]
async fn archived_rows_leave_seats_and_mail_together() {
    // EVM2-4: archived (active = false) rows leave the seat count AND
    // the mail eligibility TOGETHER — every filter reads the pair.
    let db = TestDb::new("evm24").await;
    let (type_id, _t) = make_type_with_mail(&db, "now", 0).await;
    let event_id = make_event(&db, "probe archive", true, 4, Some(type_id)).await;
    let service = registrations(&db);
    service.register(register_cmd(event_id, 1)).await.unwrap();
    let r2 = service.register(register_cmd(event_id, 2)).await.unwrap();

    // Archive r2 (the Odoo axis): active = false.
    sqlx::query("UPDATE event.registrations SET active = false WHERE id = $1")
        .bind(r2.id)
        .execute(&db.pool)
        .await
        .unwrap();

    // Seats: the archived row left the count (one seat free again).
    let counted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM event.registrations WHERE event_id = $1 AND state IN ('open','done') AND active",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(counted, 1, "archived rows do not hold seats");

    // Mail: the pass materializes NO receipt for the archived row.
    let run = scheduler(&db, Arc::new(RecordingQueue::default()))
        .run_due_schedulers()
        .await
        .unwrap();
    assert_eq!(run.receipts_materialized as usize, 1, "only the active row gets a receipt");
    assert_eq!(run.receipts_queued, 1);
    let receipts: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT registration_id FROM event.mail_registrations mr JOIN event.mails m ON m.id = mr.scheduler_id WHERE m.event_id = $1",
    )
    .bind(event_id)
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(receipts.len(), 1);
    assert_ne!(receipts[0], r2.id, "the archived row is ineligible for mail");
    db.dispose().await;
}

#[tokio::test]
async fn finished_event_drops_receipts_visibly() {
    // The window rule: once the event's date_end has passed, due
    // receipts DROP with the visible outcome (never silently).
    let db = TestDb::new("windowdrop").await;
    let (type_id, _t) = make_type_with_mail(&db, "now", 0).await;
    let event_id = make_event(&db, "probe window", false, 0, Some(type_id)).await;
    // Push the event into the past (the fixture made it future; the
    // hardening CHECK only demands ordering).
    sqlx::query("UPDATE event.events SET date_begin = now() - interval '2 days', date_end = now() - interval '1 day' WHERE id = $1")
        .bind(event_id)
        .execute(&db.pool)
        .await
        .unwrap();
    registrations(&db).register(register_cmd(event_id, 1)).await.unwrap();

    let run = scheduler(&db, Arc::new(RecordingQueue::default()))
        .run_due_schedulers()
        .await
        .unwrap();
    assert_eq!(run.receipts_dropped_window_closed, 1, "the past window drops visibly");
    let outcome: String = sqlx::query_scalar(
        "SELECT outcome FROM event.mail_registrations mr JOIN event.mails m ON m.id = mr.scheduler_id WHERE m.event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(outcome, "dropped_window_closed");
    db.dispose().await;
}

#[tokio::test]
async fn done_sweep_moves_past_events_to_done() {
    // The daily sweep: past events not already done/cancelled and
    // not resting in a pipe_end stage move to done (bounded batches).
    let db = TestDb::new("sweep").await;
    let event_id = make_event(&db, "probe past", false, 0, None).await;
    // BOTH window arms go past (the ordering CHECK reads the pair).
    sqlx::query(
        "UPDATE event.events SET date_begin = now() - interval '2 days', date_end = now() - interval '1 hour' WHERE id = $1",
    )
        .bind(event_id)
        .execute(&db.pool)
        .await
        .unwrap();

    let swept = scheduler(&db, Arc::new(RecordingQueue::default()))
        .sweep_mark_done(100)
        .await
        .unwrap();
    assert_eq!(swept, vec![event_id], "the past event is swept to done");

    // Idempotent: the second sweep finds nothing.
    let swept2 = scheduler(&db, Arc::new(RecordingQueue::default()))
        .sweep_mark_done(100)
        .await
        .unwrap();
    assert!(swept2.is_empty(), "done is terminal for the sweep");

    // A pipe_end-resting event is NOT swept (it already rests). The
    // stage INSERT is its own statement — a DML sub-statement cannot
    // ride inside an UPDATE expression.
    let resting = make_event(&db, "probe resting", false, 0, None).await;
    let pipe_end_stage: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO event.stages (name, sequence, pipe_end) VALUES ('Done', 99, true) RETURNING id",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE event.events SET date_begin = now() - interval '2 days', date_end = now() - interval '1 hour', stage_id = $2 WHERE id = $1",
    )
    .bind(resting)
    .bind(pipe_end_stage)
        .execute(&db.pool)
        .await
        .unwrap();
    let swept3 = scheduler(&db, Arc::new(RecordingQueue::default()))
        .sweep_mark_done(100)
        .await
        .unwrap();
    assert!(
        !swept3.contains(&resting),
        "a pipe_end-resting past event is left alone"
    );
    db.dispose().await;
}
