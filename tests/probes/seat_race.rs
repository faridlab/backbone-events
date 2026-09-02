//! concurrent_registration_zero_oversell — THE DoD probe (spec gate
//! names 8+ parallel register calls at a 4-seat event: exactly 4
//! rows land, the rest are TYPED refusals, the count never exceeds
//! the cap).
//!
//! The racing arms hit the ONE register verb from parallel tasks
//! over one pool — the FOR UPDATE inside the verb serializes them;
//! count-then-insert in one transaction makes oversell impossible.

use futures::future::join_all;

use backbone_events::application::service::event_error::EventError;
use backbone_events::application::service::seat_service::{SeatAvailability, SeatService};
use backbone_events::infrastructure::persistence::event_command_repository::EventCommandRepository;
use backbone_events::infrastructure::persistence::seat_repository::SeatRepository;

use super::common::{make_event, register_cmd, registrations, TestDb};

#[tokio::test]
async fn concurrent_registration_zero_oversell() {
    let db = TestDb::new("seatrace").await;
    let event_id = make_event(&db, "probe four-seater", true, 4, None).await;
    let service = registrations(&db);

    // 8 parallel register calls against the 4 seats.
    let mut tasks = Vec::new();
    for n in 0..8 {
        let service_ref = &service;
        tasks.push(async move {
            let cmd = register_cmd(event_id, n);
            service_ref.register(cmd).await
        });
    }
    let results = join_all(tasks).await;

    let wins = results.iter().filter(|r| r.is_ok()).count();
    let refusals = results.iter().filter(|r| r.is_err()).count();

    // Exactly 4 win (the cap), exactly 4 are refused.
    assert_eq!(wins, 4, "exactly the cap wins — got {wins} winners of 8");
    assert_eq!(refusals, 4, "the overflow is refused — got {refusals} refusals");

    // EVERY refusal is the TYPED event_seats_exhausted (never a 500,
    // never a generic error).
    for r in results.iter().filter(|r| r.is_err()) {
        match r {
            Err(EventError::EventSeatsExhausted { event_id: refused_at }) => {
                assert_eq!(*refused_at, event_id);
            }
            other => panic!("refusal must be typed seats-exhausted, got {other:?}"),
        }
    }

    // The stored count matches the cap exactly — no oversell row.
    let stored: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM event.registrations WHERE event_id = $1 AND state IN ('open','done') AND active",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(stored, 4, "stored rows == cap (zero oversell), got {stored}");

    // The ONE counter's read agrees with the stored count.
    let seats = SeatService::new(
        SeatRepository::new(db.pool.clone()),
        EventCommandRepository::new(db.pool.clone()),
    );
    let SeatAvailability { taken, capacity, limited, .. } = seats.availability(event_id, None).await.unwrap();
    assert!(limited, "the fixture event is limited");
    assert_eq!(capacity, 4);
    assert_eq!(taken, 4, "the seat read == the stored count");

    // Every winner carries a distinct minted barcode (global
    // uniqueness would trip a unique violation otherwise; assert the
    // count of distinct values too).
    let barcodes: Vec<String> = sqlx::query_scalar(
        "SELECT barcode FROM event.registrations WHERE event_id = $1 ORDER BY barcode",
    )
    .bind(event_id)
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(barcodes.len(), 4);
    let mut distinct = barcodes.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 4, "barcodes are distinct");
    for b in &barcodes {
        assert!(!b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()), "barcode is decimal: {b}");
    }

    db.dispose().await;
}

#[tokio::test]
async fn unlimited_event_never_refuses_on_count() {
    // The complement arm: seats_limited = false (cap 0 = unlimited) —
    // 8 racing registrations ALL land.
    let db = TestDb::new("unlimited").await;
    let event_id = make_event(&db, "probe unlimited", false, 0, None).await;
    let service = registrations(&db);

    let mut tasks = Vec::new();
    for n in 0..8 {
        let service_ref = &service;
        tasks.push(async move {
            let cmd = register_cmd(event_id, n);
            service_ref.register(cmd).await
        });
    }
    let results = join_all(tasks).await;
    assert!(
        results.iter().all(|r| r.is_ok()),
        "unlimited events refuse nothing on seat count"
    );
    let stored: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM event.registrations WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(stored, 8);
    db.dispose().await;
}
