//! The desk probes: the frozen branch order of the scan verb —
//! unknown/inactive -> invalid ticket, cancelled -> canceled, draft ->
//! unconfirmed (no write), finished event -> not ongoing, cross-event
//! -> manual confirmation (no write), already done -> already
//! registered, open -> the ONE write (done through the transition
//! verb). The barcode is EXACT-MATCH (globally unique, no codec).

use backbone_events::application::service::desk_service::DeskService;
use backbone_events::application::service::event_error::EventError;
use backbone_events::application::service::event_service::EventCommandService;
use backbone_events::application::service::sale_seam_service::{OrderConfirmed, SaleSeamService};
use backbone_events::infrastructure::persistence::event_command_repository::{
    CreateEventInput, EventCommandRepository,
};
use backbone_events::infrastructure::persistence::sale_seam_repository::SaleSeamRepository;
use backbone_events::infrastructure::persistence::seat_repository::SeatRepository;

use super::common::{future_window, make_event, register_cmd, registrations, TestDb};

fn desk(db: &TestDb) -> DeskService {
    DeskService::new(
        SeatRepository::new(db.pool.clone()),
        EventCommandRepository::new(db.pool.clone()),
    )
}

#[tokio::test]
async fn the_scan_branches_in_frozen_order() {
    let db = TestDb::new("deskscan").await;
    let event_id = make_event(&db, "probe desk", false, 0, None).await;
    let reg_service = registrations(&db);

    // 1. Unknown barcode -> invalid ticket.
    match desk(&db).register_attendee("000000000000", event_id, None).await {
        Err(EventError::DeskInvalidTicket) => {}
        other => panic!("expected invalid ticket, got {other:?}"),
    }

    // 2. Cancelled -> canceled registration.
    let cancelled = reg_service.register(register_cmd(event_id, 1)).await.unwrap();
    reg_service.cancel(cancelled.id, None).await.unwrap();
    match desk(&db).register_attendee(&cancelled.barcode, event_id, None).await {
        Err(EventError::DeskCanceledRegistration) => {}
        other => panic!("expected canceled, got {other:?}"),
    }

    // 3. Draft (a paid mint not yet healed) -> unconfirmed, NO WRITE.
    let seam = SaleSeamService::new(SaleSeamRepository::new(db.pool.clone()));
    let order = uuid::Uuid::new_v4();
    seam.on_order_confirmed(OrderConfirmed {
        delivery_id: None,
        order_id: order,
        company_id: None,
        customer_id: None,
        grand_total: "50.00".into(),
        currency: None,
        registrations: vec![backbone_events::application::service::sale_seam_service::RegistrationSpec {
            event_id,
            event_slot_id: None,
            event_ticket_id: None,
            name: "Held Buyer".into(),
            email: "held@probe.test".into(),
            phone: None,
            company_name: None,
            partner_id: None,
        }],
    })
    .await
    .unwrap();
    let held: (String, String) = sqlx::query_as(
        "SELECT barcode, state::text FROM event.registrations WHERE sale_order_id = $1",
    )
    .bind(order)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(held.1, "draft");
    match desk(&db).register_attendee(&held.0, event_id, None).await {
        Err(EventError::DeskUnconfirmedRegistration) => {}
        other => panic!("expected unconfirmed, got {other:?}"),
    }
    let still: String = sqlx::query_scalar(
        "SELECT state::text FROM event.registrations WHERE sale_order_id = $1",
    )
    .bind(order)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(still, "draft", "the unconfirmed branch writes NOTHING");

    // 6-before-5 check material: an OPEN row of THIS event scans DONE.
    let open_reg = reg_service.register(register_cmd(event_id, 2)).await.unwrap();
    let (row, before, after) = desk(&db)
        .register_attendee(&open_reg.barcode, event_id, None)
        .await
        .unwrap();
    assert_eq!((before.as_str(), after.as_str()), ("open", "done"));
    assert_eq!(row.state, "done");
    let closed: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT date_closed FROM event.registrations WHERE id = $1",
    )
    .bind(open_reg.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(closed.is_some(), "the desk write stamps date_closed");

    // 7. Re-scan -> already registered (idempotent refusal).
    match desk(&db).register_attendee(&open_reg.barcode, event_id, None).await {
        Err(EventError::DeskAlreadyRegistered) => {}
        other => panic!("expected already registered, got {other:?}"),
    }
    db.dispose().await;
}

#[tokio::test]
async fn finished_event_and_cross_event_branches() {
    let db = TestDb::new("deskbounds").await;
    let reg_service = registrations(&db);

    // A FINISHED event (kanban done): the scan refuses at the
    // not-ongoing branch even for a done row.
    let service = EventCommandService::new(EventCommandRepository::new(db.pool.clone()));
    let (begin, end) = future_window();
    let finished = service
        .create(
            &CreateEventInput {
                name: "probe desk finished".into(),
                date_begin: begin,
                date_end: end,
                event_slot_count: 1,
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
    service.mark_done(finished.id, None).await.unwrap();
    let done_reg = reg_service.register(register_cmd(finished.id, 1)).await.unwrap();
    match desk(&db).register_attendee(&done_reg.barcode, finished.id, None).await {
        Err(EventError::DeskNotOngoingEvent) => {}
        other => panic!("expected not-ongoing, got {other:?}"),
    }

    // A PAST event (date_end gone by): same branch.
    let past_begin = chrono::Utc::now() - chrono::Duration::days(2);
    let past_end = past_begin + chrono::Duration::days(1);
    let past = service
        .create(
            &CreateEventInput {
                name: "probe desk past".into(),
                date_begin: past_begin,
                date_end: past_end,
                event_slot_count: 1,
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
    let past_reg = reg_service.register(register_cmd(past.id, 2)).await.unwrap();
    match desk(&db).register_attendee(&past_reg.barcode, past.id, None).await {
        Err(EventError::DeskNotOngoingEvent) => {}
        other => panic!("expected not-ongoing for a past window, got {other:?}"),
    }

    // CROSS-EVENT: a valid open ticket scanned at ANOTHER ongoing
    // event -> manual confirmation, NO WRITE on either side.
    let event_a = make_event(&db, "probe desk A", false, 0, None).await;
    let event_b = make_event(&db, "probe desk B", false, 0, None).await;
    let a_reg = reg_service.register(register_cmd(event_a, 3)).await.unwrap();
    match desk(&db).register_attendee(&a_reg.barcode, event_b, None).await {
        Err(EventError::DeskNeedManualConfirmation) => {}
        other => panic!("expected manual confirmation, got {other:?}"),
    }
    let untouched: String = sqlx::query_scalar(
        "SELECT state::text FROM event.registrations WHERE id = $1",
    )
    .bind(a_reg.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(untouched, "open", "the cross-event branch writes NOTHING");
    db.dispose().await;
}

#[tokio::test]
async fn barcode_is_exact_match_and_globally_unique() {
    // EBG-1: the lookup is exact-match over a globally unique barcode
    // — a PREFIX or NEAR-MISS scans as invalid, never a LIKE hit.
    let db = TestDb::new("deskbarcode").await;
    let event_a = make_event(&db, "probe barcode A", false, 0, None).await;
    let event_b = make_event(&db, "probe barcode B", false, 0, None).await;
    let reg = registrations(&db).register(register_cmd(event_a, 1)).await.unwrap();

    // A near-miss (dropped digit, appended digit) is EXACT-miss:
    // invalid, never a LIKE hit. The verb TRIMS surrounding
    // whitespace before the exact match — a padded scan resolves to
    // the same row (the documented carrier behavior), so at ANOTHER
    // event's desk it lands on the cross-event branch.
    let trimmed = reg.barcode[..reg.barcode.len() - 1].to_string();
    for miss in [&trimmed, &format!("{}0", reg.barcode)] {
        match desk(&db).register_attendee(miss, event_b, None).await {
            Err(EventError::DeskInvalidTicket) => {}
            other => panic!("a near-miss barcode must be invalid, got {other:?}"),
        }
    }
    match desk(&db).register_attendee(&format!(" {} ", reg.barcode), event_b, None).await {
        Err(EventError::DeskNeedManualConfirmation) => {
            // trimmed → resolved → the cross-event branch decides
        }
        other => panic!("a padded scan resolves (trim) then crosses events, got {other:?}"),
    }
    db.dispose().await;
}
