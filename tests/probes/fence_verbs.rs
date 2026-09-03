//! The fence + verb probes: the publication fence (patch refused,
//! publish/unpublish the only writers, date_publish stamps once),
//! the my-tickets capability (never-expiring deviation, pinned set),
//! the multi-slot refusals, and the sale-window read-time predicate.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use backbone_events::application::service::capability::mint_ticket_report_capability;
use backbone_events::application::service::event_error::EventError;
use backbone_events::application::service::my_tickets_service::MyTicketsService;
use backbone_events::application::service::seat_service::SeatService;
use backbone_events::infrastructure::persistence::event_command_repository::{
    EventCommandRepository, PatchEventInput,
};
use backbone_events::infrastructure::persistence::seat_repository::SeatRepository;
use backbone_events::presentation::http::{event_admin_routes, EventAdminState};

use super::common::{
    make_event, make_multi_slot_event, make_type_with_mail, register_cmd, registrations,
    RefusingLeadSink, RefusingSmsQueue, PROBE_SECRET, RecordingQueue, StubRenderer, TestDb,
};

#[tokio::test]
async fn publish_fence_and_verb_writers() {
    let db = TestDb::new("fence").await;
    let event_id = make_event(&db, "probe fence", false, 0, None).await;
    let repo = EventCommandRepository::new(db.pool.clone());

    // The fence: the repository structurally has no patch arm for the
    // pair; the ROUTE refuses the typed body keys. Route-level:
    let state = EventAdminState::new(
        db.pool.clone(),
        Arc::new(StubRenderer),
        Arc::new(RecordingQueue::default()),
        Arc::new(RefusingSmsQueue),
        Arc::new(RefusingLeadSink),
    );
    let app = event_admin_routes(state);
    let patch = r#"{"name": "renamed", "is_published": true}"#.to_string();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/events/{event_id}"))
                .header("content-type", "application/json")
                .body(Body::from(patch))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "publish_refused", "the fence is typed");

    // The patch did NOT publish (the non-fenced arm was also refused —
    // the whole body goes back, no partial application).
    let row = repo.find(event_id).await.unwrap();
    assert!(!row.is_published, "a refused patch applies NOTHING");

    // date_publish in the body: same typed refusal.
    let patch2 = r#"{"date_publish": "2026-01-01T00:00:00Z"}"#.to_string();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/events/{event_id}"))
                .header("content-type", "application/json")
                .body(Body::from(patch2))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // The legit patch arm (service-level, no fenced keys).
    repo.patch(
        event_id,
        &PatchEventInput {
            name: Some("renamed".into()),
            ..Default::default()
        },
        None,
    )
    .await
    .unwrap();
    let row = repo.find(event_id).await.unwrap();
    assert_eq!(row.name, "renamed");

    // PUBLISH — the only writer; stamps date_publish on first
    // publish only.
    repo.publish(event_id, None).await.unwrap();
    let first = repo.find(event_id).await.unwrap().date_publish;
    assert!(first.is_some());
    // Unpublish + republish: the stamp KEEPS the original.
    repo.unpublish(event_id, None).await.unwrap();
    assert!(!repo.find(event_id).await.unwrap().is_published);
    // The capability gate re-arms live: unpublished means the uniform
    // refusal again.
    let err = repo.find_published(event_id).await.unwrap_err();
    assert_eq!(err.code(), "event_not_published");
    repo.publish(event_id, None).await.unwrap();
    let second = repo.find(event_id).await.unwrap().date_publish;
    assert_eq!(first, second, "date_publish stamps ONCE");
    db.dispose().await;
}

#[tokio::test]
async fn my_tickets_pins_the_registration_set() {
    let db = TestDb::new("mytickets").await;
    let (type_id, _t) = make_type_with_mail(&db, "now", 0).await;
    let event_id = make_event(&db, "probe tickets", false, 0, Some(type_id)).await;
    let other_event = make_event(&db, "probe other", false, 0, None).await;
    EventCommandRepository::new(db.pool.clone())
        .publish(event_id, None)
        .await
        .unwrap();
    EventCommandRepository::new(db.pool.clone())
        .publish(other_event, None)
        .await
        .unwrap();

    let service = registrations(&db);
    let r1 = service.register(register_cmd(event_id, 1)).await.unwrap();
    let r2 = service.register(register_cmd(event_id, 2)).await.unwrap();
    let foreign = service.register(register_cmd(other_event, 9)).await.unwrap();

    let tickets = MyTicketsService::new(
        EventCommandRepository::new(db.pool.clone()),
        SeatRepository::new(db.pool.clone()),
    );

    // The token pins (event, sorted registrations).
    let token = tickets
        .mint(PROBE_SECRET, event_id, &[r1.id, r2.id])
        .unwrap();
    let report = tickets.report(PROBE_SECRET, &token).await.unwrap();
    assert_eq!(report.event_id, event_id);
    assert_eq!(report.tickets.len(), 2);
    assert!(report.tickets.iter().all(|t| t.barcode.len() <= 32));

    // The never-expiring deviation: exp = 0 survives any "now".
    let claims = backbone_events::application::service::capability::CapabilityClaims::verify(
        PROBE_SECRET,
        "event-registration-ticket-report-access",
        &token,
        chrono::Utc::now() + chrono::Duration::days(3650),
    )
    .unwrap();
    assert_eq!(claims.exp, 0, "the deviation is real: exp = 0");

    // A foreign registration id in the PINNED SET of another event:
    // the report never returns it (event mismatch = absent).
    let forged = mint_ticket_report_capability(PROBE_SECRET, &event_id, &[foreign.id]).unwrap();
    let report = tickets.report(PROBE_SECRET, &forged).await.unwrap();
    assert!(report.tickets.is_empty(), "foreign rows are never returned");

    // Tampered token: the uniform 404. The replacement char sits
    // OUTSIDE the token alphabet — an in-alphabet char can leave the
    // token valid (a flaky pass).
    let tampered = format!("{}~", &token[..token.len() - 1]);
    let err = tickets.report(PROBE_SECRET, &tampered).await.unwrap_err();
    assert_eq!(err.code(), "event_not_published");

    // Unpublished event: the uniform 404 (the gate is shared with
    // /ics).
    EventCommandRepository::new(db.pool.clone())
        .unpublish(event_id, None)
        .await
        .unwrap();
    let err = tickets.report(PROBE_SECRET, &token).await.unwrap_err();
    assert_eq!(err.code(), "event_not_published");
    db.dispose().await;
}

#[tokio::test]
async fn multi_slot_refusals_are_typed() {
    let db = TestDb::new("multislot").await;
    let event_id = make_multi_slot_event(&db, "probe multi-slot", 2).await;

    // A slot on ANOTHER event (belonging refusal arm).
    let other = make_multi_slot_event(&db, "probe other multi", 2).await;
    let foreign_slot = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO event.slots (id, event_id, name, date_begin) VALUES ($1, $2, 'foreign', now() + interval '30 days')")
        .bind(foreign_slot)
        .bind(other)
        .execute(&db.pool)
        .await
        .unwrap();

    let service = registrations(&db);

    // Multi-slot WITHOUT a slot: typed event_slot_required.
    let mut no_slot = register_cmd(event_id, 1);
    no_slot.event_slot_id = None;
    match service.register(no_slot).await {
        Err(EventError::EventSlotRequired { .. }) => {}
        other_err => panic!("expected slot-required, got {other_err:?}"),
    }

    // Foreign slot: typed event_slot_not_of_event.
    let mut foreign = register_cmd(event_id, 2);
    foreign.event_slot_id = Some(foreign_slot);
    match service.register(foreign).await {
        Err(EventError::EventSlotNotOfEvent { .. }) => {}
        other_err => panic!("expected slot-not-of-event, got {other_err:?}"),
    }

    // A real slot: lands, and the seat read scopes to the slot.
    let slot = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO event.slots (id, event_id, name, date_begin) VALUES ($1, $2, 'real', now() + interval '30 days')")
        .bind(slot)
        .bind(event_id)
        .execute(&db.pool)
        .await
        .unwrap();
    let mut ok = register_cmd(event_id, 3);
    ok.event_slot_id = Some(slot);
    let row = service.register(ok).await.unwrap();
    assert_eq!(row.event_slot_id, Some(slot));

    // The per-slot cap: ONE more lands (the slot's 2nd and last seat),
    // the next is refused typed.
    let mut fill = register_cmd(event_id, 4);
    fill.event_slot_id = Some(slot);
    service.register(fill).await.unwrap();
    let mut over = register_cmd(event_id, 5);
    over.event_slot_id = Some(slot);
    match service.register(over).await {
        Err(EventError::EventSeatsExhausted { .. }) => {}
        other_err => panic!("expected seats-exhausted at the slot cap, got {other_err:?}"),
    }
    // The slot-scoped availability read agrees.
    let seats = SeatService::new(
        SeatRepository::new(db.pool.clone()),
        EventCommandRepository::new(db.pool.clone()),
    );
    let avail = seats.availability(event_id, Some(slot)).await.unwrap();
    assert_eq!(avail.taken, 2);
    assert_eq!(avail.capacity, 2);
    db.dispose().await;
}

#[tokio::test]
async fn sale_window_is_a_read_time_predicate() {
    // The ticket window is lazy: shut windows refuse at REGISTER
    // time (no cron flips anything anywhere).
    let db = TestDb::new("salewindow").await;
    let event_id = make_event(&db, "probe window", false, 0, None).await;
    let ticket = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO event.tickets (id, event_id, name, start_sale_datetime, end_sale_datetime) VALUES ($1, $2, 'early-bird', now() - interval '2 days', now() - interval '1 day')",
    )
    .bind(ticket)
    .bind(event_id)
    .execute(&db.pool)
    .await
    .unwrap();

    let service = registrations(&db);
    let mut cmd = register_cmd(event_id, 1);
    cmd.event_ticket_id = Some(ticket);
    match service.register(cmd).await {
        Err(EventError::EventSaleWindowClosed { event_ticket_id }) => {
            assert_eq!(event_ticket_id, ticket);
        }
        other_err => panic!("expected sale-window-closed, got {other_err:?}"),
    }

    // A ticket of another event: typed belonging refusal.
    let other = make_event(&db, "probe other window", false, 0, None).await;
    let _ = other;
    let foreign_ticket = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO event.tickets (id, event_id, name) VALUES ($1, $2, 'foreign-ticket')")
        .bind(foreign_ticket)
        .bind(other)
        .execute(&db.pool)
        .await
        .unwrap();
    let mut cmd2 = register_cmd(event_id, 2);
    cmd2.event_ticket_id = Some(foreign_ticket);
    match service.register(cmd2).await {
        Err(EventError::EventTicketNotOfEvent { .. }) => {}
        other_err => panic!("expected ticket-not-of-event, got {other_err:?}"),
    }
    db.dispose().await;
}

#[tokio::test]
async fn template_applies_once_at_create() {
    // The ONE template-apply: the type's scheduler rows copy at
    // CREATE, exactly once — a later type change never re-propagates
    // (forking a second scheduler row onto an existing event).
    let db = TestDb::new("templateapply").await;
    let (type_id, template_id) = make_type_with_mail(&db, "days", 2).await;
    let event_id = make_event(&db, "probe template", false, 0, Some(type_id)).await;

    let rows: Vec<(uuid::Uuid, Option<uuid::Uuid>)> = sqlx::query_as(
        "SELECT id, template_ref FROM event.mails WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1, "the type's one template applied");
    assert_eq!(rows[0].1, Some(template_id));

    // Add a SECOND type_mail to the SAME type after the event exists:
    // no re-propagation.
    sqlx::query(
        "INSERT INTO event.type_mails (event_type_id, interval_nbr, interval_unit, interval_kind, template_ref) VALUES ($1, 1, 'hours', 'after_sub', $2)",
    )
    .bind(type_id)
    .bind(uuid::Uuid::new_v4())
    .execute(&db.pool)
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM event.mails WHERE event_id = $1")
        .bind(event_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "template-apply never re-propagates");
    db.dispose().await;
}
