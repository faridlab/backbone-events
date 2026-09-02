//! transition_into_holding_domain_takes_a_seat — the second-path probe.
//!
//! The admin state verbs (confirm/set_done) move a registration INTO the
//! seat-holding domain just like register() does. This probe pins the
//! invariant that the transition path runs the SAME seat head: an
//! admin confirm at a full event is a TYPED refusal (and the row is
//! untouched), a confirm under capacity lands, and the verbs that do
//! NOT take a seat (within-domain open→done, releasing →cancel) never
//! consult the counter.

use backbone_events::application::service::event_error::EventError;

use super::common::{make_event, register_cmd, registrations, TestDb};

/// The counting-domain predicate, one source of truth for the probe's
/// stored-count assertions.
async fn stored_taken(db: &TestDb, event_id: uuid::Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM event.registrations WHERE event_id = $1 AND state IN ('open','done') AND active",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn admin_confirm_at_full_event_is_a_typed_refusal_and_rolls_back() {
    let db = TestDb::new("tr_atcap").await;
    let event_id = make_event(&db, "probe transition cap", true, 2, None).await;
    let service = registrations(&db);

    // Fill the event, park one seated row in draft (releasing its
    // seat), and let the register verb hand that seat to a third
    // attendee — the event is full again with a draft row waiting.
    let first = service.register(register_cmd(event_id, 1)).await.unwrap();
    let second = service.register(register_cmd(event_id, 2)).await.unwrap();
    service.set_draft(first.id, None).await.unwrap();
    let third = service.register(register_cmd(event_id, 3)).await.unwrap();
    assert_eq!(stored_taken(&db, event_id).await, 2);

    // Confirming the draft row at the now-full event: TYPED refusal,
    // never a 500, never a silent overflow.
    let refusal = service
        .confirm(first.id, None)
        .await
        .unwrap_err(); // confirm at capacity must refuse
    match refusal {
        EventError::EventSeatsExhausted { event_id: at } => assert_eq!(at, event_id),
        other => panic!("refusal must be typed seats-exhausted, got {other:?}"),
    }

    // The refusal rolled the whole transition back: the parked row is
    // still draft, the stored count still equals the cap.
    let state: String = sqlx::query_scalar("SELECT state::text FROM event.registrations WHERE id = $1")
        .bind(first.id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(state, "draft", "the refused confirm must not stage any state");
    assert_eq!(stored_taken(&db, event_id).await, 2);

    // The two seated rows are untouched by the refusal.
    for seated in [&second.id, &third.id] {
        let s: String = sqlx::query_scalar("SELECT state::text FROM event.registrations WHERE id = $1")
            .bind(seated)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(s, "open");
    }
    db.dispose().await;
}

#[tokio::test]
async fn admin_confirm_under_capacity_lands_and_takes_the_seat() {
    let db = TestDb::new("tr_room").await;
    let event_id = make_event(&db, "probe transition room", true, 3, None).await;
    let service = registrations(&db);

    let first = service.register(register_cmd(event_id, 1)).await.unwrap();
    let parked = service.register(register_cmd(event_id, 2)).await.unwrap();
    service.set_draft(parked.id, None).await.unwrap();
    assert_eq!(stored_taken(&db, event_id).await, 1);

    // Room for one more: the confirm lands and the count reflects it.
    let landed = service.confirm(parked.id, None).await.unwrap();
    assert_eq!(landed.state, "open");
    assert_eq!(stored_taken(&db, event_id).await, 2);

    // A fresh register takes the last seat; the event is now full at
    // the cap — the transition and register paths hold the same line.
    service.register(register_cmd(event_id, 3)).await.unwrap();
    assert_eq!(stored_taken(&db, event_id).await, 3);
    let refusal = service
        .register(register_cmd(event_id, 4))
        .await
        .unwrap_err(); // register at capacity must refuse
    assert!(
        matches!(refusal, EventError::EventSeatsExhausted { .. }),
        "got {refusal:?}"
    );
    // And re-confirming the already-open row is a no-op, not a refusal.
    let again = service.confirm(first.id, None).await.unwrap();
    assert_eq!(again.state, "open");
    db.dispose().await;
}

#[tokio::test]
async fn verbs_that_hold_or_release_never_consult_the_counter() {
    let db = TestDb::new("tr_holdrel").await;
    let event_id = make_event(&db, "probe hold-release", true, 1, None).await;
    let service = registrations(&db);

    // A one-seat event, full.
    let only = service.register(register_cmd(event_id, 1)).await.unwrap();
    assert_eq!(stored_taken(&db, event_id).await, 1);

    // open -> done stays INSIDE the holding domain (the seat is
    // already held): succeeds at a full event.
    let done = service.set_done(only.id, None).await.unwrap();
    assert_eq!(done.state, "done");
    assert_eq!(stored_taken(&db, event_id).await, 1, "'done' still holds the seat");

    // done -> cancel RELEASES: succeeds at a full event (it frees).
    let cancelled = service.cancel(only.id, None).await.unwrap();
    assert_eq!(cancelled.state, "cancel");
    assert_eq!(stored_taken(&db, event_id).await, 0);

    // cancel -> draft stays outside the domain: no counter consult.
    let draft = service.set_draft(only.id, None).await.unwrap();
    assert_eq!(draft.state, "draft");
    db.dispose().await;
}
