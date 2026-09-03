//! The booth probes: the reified booking lifecycle (pending -> confirm
//! -> release), the EBS-1 exclusivity WALL (the partial unique index;
//! the second confirm is the typed loud loser), the one-event-per-line
//! guard (EBS-4), the delete fences, the one-way paid latch, the
//! template-apply whitelist, and SO-cancel never freeing a booth.

use backbone_events::application::service::booth_command_service::BoothCommandService;
use backbone_events::application::service::event_error::EventError;
use backbone_events::application::service::event_service::EventCommandService;
use backbone_events::application::service::sale_seam_service::{OrderCancelled, SaleSeamService};
use backbone_events::infrastructure::persistence::booth_command_repository::BoothCommandRepository;
use backbone_events::infrastructure::persistence::event_command_repository::{
    CreateEventInput, EventCommandRepository,
};
use backbone_events::infrastructure::persistence::sale_seam_repository::SaleSeamRepository;

use super::common::{future_window, make_event, TestDb};

fn booths(db: &TestDb) -> BoothCommandService {
    BoothCommandService::new(BoothCommandRepository::new(db.pool.clone()))
}

async fn make_category(db: &TestDb) -> uuid::Uuid {
    let id = uuid::Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO event.booth_categories (id, name) VALUES ($1, 'probe category')"#,
    )
    .bind(id)
    .execute(&db.pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn booking_lifecycle_and_the_exclusivity_wall() {
    let db = TestDb::new("boothwall").await;
    let event_id = make_event(&db, "probe booths", false, 0, None).await;
    let category = make_category(&db).await;
    let booth = booths(&db)
        .create_booth(event_id, category, "A1", None)
        .await
        .unwrap();
    assert_eq!(booth.state, "available", "a booth is born available");

    // Booking 1 pending -> confirm: the booth's five writes land.
    let b1 = booths(&db)
        .create_booking(
            booth.id,
            None,
            Some(uuid::Uuid::new_v4()),
            Some("Alice"),
            Some("alice@probe.test"),
            Some("+15550001"),
            None,
        )
        .await
        .unwrap();
    assert_eq!(b1.status, "pending", "a booking is born pending");
    let (confirmed_booking, booth_after) = booths(&db).confirm_booking(b1.id, None).await.unwrap();
    assert_eq!(confirmed_booking.status, "confirmed");
    assert_eq!(booth_after.state, "unavailable", "confirm flips the booth");
    assert_eq!(booth_after.contact_name.as_deref(), Some("Alice"), "fill-if-empty contact");

    // Booking 2 on the SAME booth: create is fine (pending rows never
    // collide) — the CONFIRM is the wall.
    let b2 = booths(&db)
        .create_booking(booth.id, None, None, Some("Bob"), None, None, None)
        .await
        .unwrap();
    let loser = booths(&db).confirm_booking(b2.id, None).await;
    match loser {
        Err(EventError::BoothAlreadyConfirmed { event_booth_id }) => {
            assert_eq!(event_booth_id, booth.id, "the loser is LOUD and typed (EBS-1)");
        }
        other => panic!("expected the typed exclusivity refusal, got {other:?}"),
    }
    // The loser's intent row survives as pending — a human decides.
    assert_eq!(booths(&db).find_booking(b2.id).await.unwrap().status, "pending");

    // The DB wall itself (the pre-check demoted, the index holds):
    // a direct confirmed-row insert collides.
    let dup: Result<_, _> = sqlx::query(
        r#"INSERT INTO event.booth_bookings (id, event_booth_id, status)
           VALUES ($1, $2, 'confirmed')"#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(booth.id)
    .execute(&db.pool)
    .await;
    assert!(dup.is_err(), "the partial unique index is the real wall");

    // RELEASE: the human verb — booking rows go, the booth reopens,
    // the operator's contact data... stays (release only reopens).
    let released = booths(&db).release_booth(booth.id, None).await.unwrap();
    assert_eq!(released.state, "available");
    assert!(booths(&db).list_bookings_of_booth(booth.id).await.unwrap().is_empty());
    // After release a NEW booking can confirm (exclusivity re-armed).
    let b3 = booths(&db)
        .create_booking(booth.id, None, None, None, None, None, None)
        .await
        .unwrap();
    booths(&db).confirm_booking(b3.id, None).await.unwrap();
    db.dispose().await;
}

#[tokio::test]
async fn one_event_per_order_line_and_delete_fences() {
    let db = TestDb::new("boothfence").await;
    let event_a = make_event(&db, "probe booths A", false, 0, None).await;
    let event_b = make_event(&db, "probe booths B", false, 0, None).await;
    let category = make_category(&db).await;
    let booth_a = booths(&db).create_booth(event_a, category, "A1", None).await.unwrap();
    let booth_b = booths(&db).create_booth(event_b, category, "B1", None).await.unwrap();

    // EBS-4: one order line books booths of ONE event.
    let line = uuid::Uuid::new_v4();
    booths(&db).create_booking(booth_a.id, Some(line), None, None, None, None, None).await.unwrap();
    match booths(&db)
        .create_booking(booth_b.id, Some(line), None, None, None, None, None)
        .await
    {
        Err(EventError::BoothBookingLineCrossEvent { sale_order_line_id }) => {
            assert_eq!(sale_order_line_id, line);
        }
        other => panic!("expected the cross-event line refusal, got {other:?}"),
    }

    // Delete fence 1: a booth with booking rows refuses (release
    // first).
    match booths(&db).delete_booth(booth_a.id, None).await {
        Err(EventError::Validation(msg)) => {
            assert!(msg.contains("release first"), "the fence says the verb: {msg}");
        }
        other => panic!("expected the booking-row delete fence, got {other:?}"),
    }

    // Delete fence 2: a sale-linked booth refuses even after release
    // (the sale link is history).
    let booth_c = booths(&db).create_booth(event_a, category, "C1", None).await.unwrap();
    let linked_line = uuid::Uuid::new_v4();
    let booking = booths(&db)
        .create_booking(booth_c.id, Some(linked_line), None, None, None, None, None)
        .await
        .unwrap();
    booths(&db).confirm_booking(booking.id, None).await.unwrap();
    booths(&db).release_booth(booth_c.id, None).await.unwrap();
    match booths(&db).delete_booth(booth_c.id, None).await {
        Err(EventError::BoothDeleteRefusedSaleLinked { booth_id }) => {
            assert_eq!(booth_id, booth_c.id, "sale-linked delete refused (EBS-5c)");
        }
        other => panic!("expected the sale-linked delete fence, got {other:?}"),
    }
    db.dispose().await;
}

#[tokio::test]
async fn paid_latch_is_one_way_and_survives_release() {
    let db = TestDb::new("boothlatch").await;
    let event_id = make_event(&db, "probe latch", false, 0, None).await;
    let category = make_category(&db).await;
    let booth = booths(&db).create_booth(event_id, category, "L1", None).await.unwrap();
    let line = uuid::Uuid::new_v4();
    let booking = booths(&db)
        .create_booking(booth.id, Some(line), None, None, None, None, None)
        .await
        .unwrap();
    booths(&db).confirm_booking(booking.id, None).await.unwrap();

    // The paid verb latches.
    let seam = SaleSeamService::new(SaleSeamRepository::new(db.pool.clone()));
    let order = uuid::Uuid::new_v4();
    let paid = seam
        .on_order_paid(backbone_events::application::service::sale_seam_service::OrderPaid {
            delivery_id: None,
            order_id: order,
            line_ids: vec![line],
        })
        .await
        .unwrap();
    assert_eq!(paid.booths_latched_paid, 1);
    assert!(booths(&db).find_booth(booth.id).await.unwrap().is_paid);

    // RELEASE keeps the latch (paid is history, not state).
    let released = booths(&db).release_booth(booth.id, None).await.unwrap();
    assert!(released.is_paid, "release never un-pays");
    db.dispose().await;
}

#[tokio::test]
async fn so_cancel_never_frees_a_booth() {
    // The DELIBERATE semantic change (register row): a
    // SalesOrderCancelled delivery cascades registrations only — the
    // booth keeps its confirmed booking; release stays a human verb.
    let db = TestDb::new("boothsocancel").await;
    let event_id = make_event(&db, "probe so-cancel", false, 0, None).await;
    let category = make_category(&db).await;
    let booth = booths(&db).create_booth(event_id, category, "S1", None).await.unwrap();
    let line = uuid::Uuid::new_v4();
    let booking = booths(&db)
        .create_booking(booth.id, Some(line), None, None, None, None, None)
        .await
        .unwrap();
    booths(&db).confirm_booking(booking.id, None).await.unwrap();

    let seam = SaleSeamService::new(SaleSeamRepository::new(db.pool.clone()));
    let order = uuid::Uuid::new_v4();
    seam.on_order_cancelled(OrderCancelled {
        delivery_id: None,
        order_id: order,
        company_id: None,
        customer_id: None,
    })
    .await
    .unwrap();

    let still = booths(&db).find_booth(booth.id).await.unwrap();
    assert_eq!(still.state, "unavailable", "SO-cancel does NOT free the booth");
    assert_eq!(
        booths(&db).find_booking(booking.id).await.unwrap().status,
        "confirmed"
    );
    db.dispose().await;
}

#[tokio::test]
async fn type_booth_template_applies_at_create_with_whitelist() {
    // The type's booth rows copy into event booths ONCE at create —
    // name + category only (booking state, contacts, and sale links
    // never template).
    let db = TestDb::new("boothapply").await;
    let type_id = uuid::Uuid::new_v4();
    let category = make_category(&db).await;
    sqlx::query("INSERT INTO event.types (id, name) VALUES ($1, 'probe booth type')")
        .bind(type_id)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO event.type_booths (id, event_type_id, name, booth_category_id)
           VALUES ($1, $2, 'T1', $3)"#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(type_id)
    .bind(category)
    .execute(&db.pool)
    .await
    .unwrap();

    let service = EventCommandService::new(EventCommandRepository::new(db.pool.clone()));
    let (begin, end) = future_window();
    let row = service
        .create(
            &CreateEventInput {
                name: "probe apply".into(),
                event_type_id: Some(type_id),
                date_begin: begin,
                date_end: end,
                event_slot_count: 1,
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
    let applied = booths(&db).list_booths_of_event(row.id, 10).await.unwrap();
    assert_eq!(applied.len(), 1, "the type's booth row applied once");
    assert_eq!(applied[0].name, "T1");
    assert_eq!(applied[0].state, "available", "the applied booth is a fresh booth");
    assert!(applied[0].sale_order_line_id.is_none(), "no sale link templates");

    // Never re-propagates: a LATER type booth row does not reach the
    // existing event.
    sqlx::query(
        r#"INSERT INTO event.type_booths (id, event_type_id, name, booth_category_id)
           VALUES ($1, $2, 'T2', $3)"#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(type_id)
    .bind(category)
    .execute(&db.pool)
    .await
    .unwrap();
    let still = booths(&db).list_booths_of_event(row.id, 10).await.unwrap();
    assert_eq!(still.len(), 1, "template-apply runs ONLY at create");
    db.dispose().await;
}
