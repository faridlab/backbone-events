//! The sms overlay probes: the dual-channel scheduler (an sms row
//! enqueues through the sms port with the PHONE as recipient, a mail
//! row through the mail queue as before), the unconfigured-gateway
//! LOUD park (sms_enqueue_refused, retried, never silently queued),
//! the recipient-invalid arm, and the ONE template-cascade verb
//! covering both channels.

use std::sync::Arc;

use backbone_events::application::service::registration_service::RegistrationCommandService;
use backbone_events::application::service::scheduler_service::SchedulerService;
use backbone_events::infrastructure::persistence::seat_repository::{RegisterCommand, SeatRepository};

use super::common::{
    make_event, make_type_with_mail, make_type_with_sms, scheduler_with_sms, RecordingQueue,
    RecordingSmsQueue, RefusingSmsQueue, StubRenderer, TestDb,
};

/// A register command WITH a phone (the sms arm's recipient).
fn register_with_phone(event_id: uuid::Uuid, n: usize, phone: Option<&str>) -> RegisterCommand {
    RegisterCommand {
        event_id,
        event_slot_id: None,
        event_ticket_id: None,
        name: format!("Attendee {n}"),
        email: format!("attendee{n}@probe.test"),
        phone: phone.map(str::to_string),
        company_name: None,
        partner_id: None,
        actor: None,
        lead_rule_skip: false,
    }
}

#[tokio::test]
async fn sms_rows_enqueue_through_the_sms_port() {
    let db = TestDb::new("smssend").await;
    let (type_id, _t) = make_type_with_sms(&db, "now", 0).await;
    let event_id = make_event(&db, "probe sms", false, 0, Some(type_id)).await;

    let reg_service = RegistrationCommandService::new(SeatRepository::new(db.pool.clone()));
    reg_service
        .register(register_with_phone(event_id, 1, Some("+15550001")))
        .await
        .unwrap();

    let mail = Arc::new(RecordingQueue::default());
    let sms = Arc::new(RecordingSmsQueue::default());
    let run = scheduler_with_sms(&db, mail.clone(), sms.clone())
        .run_due_schedulers()
        .await
        .unwrap();
    assert_eq!(run.sms_queued, 1, "the sms row went through the sms arm");
    assert_eq!(run.receipts_queued, 0, "the mail queue saw NOTHING (channel branch)");
    {
        let sent = sms.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, "+15550001", "the recipient is the PHONE");
        assert!(sent[0].1.starts_with("probe "), "the body is the rendered body_text");
    }
    assert_eq!(mail.sent.lock().unwrap().len(), 0);
    db.dispose().await;
}

#[tokio::test]
async fn both_channels_in_one_pass() {
    let db = TestDb::new("smsboth").await;
    let (mail_type, _mt) = make_type_with_mail(&db, "now", 0).await;
    let (sms_type, _st) = make_type_with_sms(&db, "now", 0).await;
    // Two events, one per channel.
    let mail_event = make_event(&db, "probe both mail", false, 0, Some(mail_type)).await;
    let sms_event = make_event(&db, "probe both sms", false, 0, Some(sms_type)).await;

    let reg_service = RegistrationCommandService::new(SeatRepository::new(db.pool.clone()));
    reg_service.register(register_with_phone(mail_event, 1, Some("+15550001"))).await.unwrap();
    reg_service.register(register_with_phone(sms_event, 2, Some("+15550002"))).await.unwrap();

    let mail = Arc::new(RecordingQueue::default());
    let sms = Arc::new(RecordingSmsQueue::default());
    let run = scheduler_with_sms(&db, mail.clone(), sms.clone())
        .run_due_schedulers()
        .await
        .unwrap();
    assert_eq!(run.receipts_queued, 1, "the mail row kept its arm");
    assert_eq!(run.sms_queued, 1, "the sms row kept its arm");
    assert_eq!(mail.sent.lock().unwrap()[0].0, "attendee1@probe.test");
    assert_eq!(sms.sent.lock().unwrap()[0].0, "+15550002");
    db.dispose().await;
}

#[tokio::test]
async fn unconfigured_gateway_parks_loudly_then_retries() {
    let db = TestDb::new("smspark").await;
    let (type_id, _t) = make_type_with_sms(&db, "now", 0).await;
    let event_id = make_event(&db, "probe sms park", false, 0, Some(type_id)).await;

    let reg_service = RegistrationCommandService::new(SeatRepository::new(db.pool.clone()));
    reg_service
        .register(register_with_phone(event_id, 1, Some("+15550001")))
        .await
        .unwrap();

    // The refusing queue (an unconfigured gateway): the receipt stays
    // unsent, the row records the typed failure — parked is NOT
    // silently queued.
    let run = scheduler_with_sms(&db, Arc::new(RecordingQueue::default()), Arc::new(RefusingSmsQueue))
        .run_due_schedulers()
        .await
        .unwrap();
    assert_eq!(run.typed_failures_recorded, 1);
    let kind: Option<String> = sqlx::query_scalar(
        "SELECT error_kind::text FROM event.mails WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(kind.as_deref(), Some("sms_enqueue_refused"));
    let unsent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM event.mail_registrations mr JOIN event.mails m ON m.id = mr.scheduler_id WHERE m.event_id = $1 AND NOT mr.mail_sent",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(unsent, 1, "the receipt stays unsent (ESM-2)");
    let done: bool = sqlx::query_scalar("SELECT mail_done FROM event.mails WHERE event_id = $1")
        .bind(event_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(!done, "the scheduler stays open and retries");

    // Wiring the gateway on the next tick delivers the parked receipt.
    let sms = Arc::new(RecordingSmsQueue::default());
    let heal = scheduler_with_sms(&db, Arc::new(RecordingQueue::default()), sms.clone())
        .run_due_schedulers()
        .await
        .unwrap();
    assert_eq!(heal.sms_queued, 1, "the retry delivers");
    assert_eq!(sms.sent.lock().unwrap().len(), 1);
    db.dispose().await;
}

#[tokio::test]
async fn phoneless_registration_is_recipient_invalid() {
    let db = TestDb::new("smsnophone").await;
    let (type_id, _t) = make_type_with_sms(&db, "now", 0).await;
    let event_id = make_event(&db, "probe sms no-phone", false, 0, Some(type_id)).await;

    let reg_service = RegistrationCommandService::new(SeatRepository::new(db.pool.clone()));
    reg_service.register(register_with_phone(event_id, 1, None)).await.unwrap();

    let run = scheduler_with_sms(&db, Arc::new(RecordingQueue::default()), Arc::new(RefusingSmsQueue))
        .run_due_schedulers()
        .await
        .unwrap();
    assert_eq!(run.sms_queued, 0);
    assert_eq!(run.typed_failures_recorded, 1);
    let kind: Option<String> = sqlx::query_scalar(
        "SELECT error_kind::text FROM event.mails WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(kind.as_deref(), Some("recipient_invalid"), "no phone = typed recipient_invalid");
    db.dispose().await;
}

#[tokio::test]
async fn one_template_cascade_covers_both_channels() {
    let db = TestDb::new("smscascade").await;
    let (mail_type, mail_tpl) = make_type_with_mail(&db, "now", 0).await;
    let (sms_type, sms_tpl) = make_type_with_sms(&db, "now", 0).await;
    let mail_event = make_event(&db, "probe cascade mail", false, 0, Some(mail_type)).await;
    let sms_event = make_event(&db, "probe cascade sms", false, 0, Some(sms_type)).await;
    let _ = (mail_event, sms_event);

    // The scheduler rows exist (template-apply landed both).
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM event.mails")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(rows, 2);

    // Cascade the SMS template: its scheduler row AND its type-level
    // row go; the mail pair is untouched (the kind discriminates).
    let scheduler = SchedulerService::new(
        backbone_events::infrastructure::persistence::scheduler_repository::SchedulerRepository::new(
            db.pool.clone(),
        ),
        backbone_events::infrastructure::persistence::event_command_repository::EventCommandRepository::new(
            db.pool.clone(),
        ),
        Arc::new(StubRenderer),
        Arc::new(RecordingQueue::default()),
        Arc::new(RefusingSmsQueue),
    );
    let (mails_deleted, type_mails_deleted) = scheduler
        .on_template_deleted("sms", sms_tpl, None)
        .await
        .unwrap();
    assert_eq!(mails_deleted, 1, "the sms scheduler row fell");
    assert_eq!(type_mails_deleted, 1, "the type-level sms template row fell");
    let remaining_kind: Option<String> = sqlx::query_scalar(
        "SELECT template_kind FROM event.mails LIMIT 1",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(remaining_kind.as_deref(), Some("mail"), "the mail row survived");

    // Cascading the mail template clears the rest.
    let (m, tm) = scheduler.on_template_deleted("mail", mail_tpl, None).await.unwrap();
    assert_eq!((m, tm), (1, 1));
    let empty: i64 = sqlx::query_scalar("SELECT count(*) FROM event.mails")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(empty, 0);
    db.dispose().await;
}
