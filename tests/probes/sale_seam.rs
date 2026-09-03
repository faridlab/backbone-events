//! The sale seam probes: the order-mint fork (paid -> born draft held,
//! free -> born open armed), delivery idempotence through the seam
//! inbox, the paid-fact forward-only heal, the cancel mirror cascade
//! (sale_status KEPT), and the zero/non-zero magnitude read.

use std::sync::Arc;

use backbone_events::application::service::sale_seam_service::{
    OrderCancelled, OrderConfirmed, OrderPaid, SaleSeamService,
};
use backbone_events::infrastructure::persistence::sale_seam_repository::grand_total_is_zero;
use backbone_events::infrastructure::persistence::sale_seam_repository::SaleSeamRepository;

use super::common::{make_event, make_type_with_mail, scheduler, TestDb, RecordingQueue};

fn seam(db: &TestDb) -> SaleSeamService {
    SaleSeamService::new(SaleSeamRepository::new(db.pool.clone()))
}

fn confirmed(order: uuid::Uuid, total: &str, event_id: uuid::Uuid, n: usize) -> OrderConfirmed {
    OrderConfirmed {
        delivery_id: None,
        order_id: order,
        company_id: None,
        customer_id: None,
        grand_total: total.to_string(),
        currency: None,
        registrations: (1..=n)
            .map(|i| backbone_events::application::service::sale_seam_service::RegistrationSpec {
                event_id,
                event_slot_id: None,
                event_ticket_id: None,
                name: format!("Buyer {i}"),
                email: format!("buyer{i}@probe.test"),
                phone: None,
                company_name: None,
                partner_id: None,
            })
            .collect(),
    }
}

#[tokio::test]
async fn paid_order_mints_draft_held_then_paid_heal() {
    let db = TestDb::new("saleseam").await;
    let (type_id, _t) = make_type_with_mail(&db, "now", 0).await;
    let event_id = make_event(&db, "probe seam paid", false, 0, Some(type_id)).await;
    let order = uuid::Uuid::new_v4();

    // PAID confirm: born DRAFT, mirrors (sale, to_pay), NO arm — the
    // scheduler materializes nothing for held rows.
    let out = seam(&db).on_order_confirmed(confirmed(order, "150.00", event_id, 2)).await.unwrap();
    assert!(!out.already_applied);
    assert_eq!(out.registrations_touched, 2);
    let rows = linked(&db, order).await;
    assert_eq!(rows.len(), 2, "both specs minted");
    for (state, sale_state, sale_status) in &rows {
        assert_eq!(state, "draft", "a paid mint is born DRAFT (held)");
        assert_eq!(sale_state, "sale");
        assert_eq!(sale_status, "to_pay");
    }
    let queue = Arc::new(RecordingQueue::default());
    let held_run = scheduler(&db, queue.clone()).run_due_schedulers().await.unwrap();
    assert_eq!(held_run.receipts_queued, 0, "held rows arm NOTHING (ES-4)");
    assert_eq!(held_run.receipts_materialized, 0, "draft rows are not eligible");

    // Replay: the inbox claim makes the redelivery a typed no-op.
    let replay = seam(&db).on_order_confirmed(confirmed(order, "150.00", event_id, 2)).await.unwrap();
    assert!(replay.already_applied, "the confirmed delivery is exactly-once");
    assert_eq!(replay.registrations_touched, 0);
    assert_eq!(linked(&db, order).await.len(), 2, "no duplicate mint");

    // THE PAID FACT: held rows heal forward (draft -> open + sold),
    // and the heal arms the after_sub engines exactly once.
    let paid = seam(&db)
        .on_order_paid(OrderPaid {
            delivery_id: None,
            order_id: order,
            line_ids: vec![],
        })
        .await
        .unwrap();
    assert_eq!(paid.registrations_touched, 2);
    for (state, sale_state, sale_status) in linked(&db, order).await {
        assert_eq!(state, "open", "the paid fact lifts the hold");
        assert_eq!(sale_state, "sale");
        assert_eq!(sale_status, "sold", "sold lands at PAID");
    }
    let queue2 = Arc::new(RecordingQueue::default());
    let armed_run = scheduler(&db, queue2.clone()).run_due_schedulers().await.unwrap();
    assert_eq!(armed_run.receipts_queued, 2, "the paid heal ARMED the after_sub engines");

    // Paid replay: no-op.
    let paid_replay = seam(&db)
        .on_order_paid(OrderPaid { delivery_id: None, order_id: order, line_ids: vec![] })
        .await
        .unwrap();
    assert!(paid_replay.already_applied);
    assert_eq!(queue2.sent.lock().unwrap().len(), 2, "no double send");
    db.dispose().await;
}

#[tokio::test]
async fn free_order_mints_open_and_arms() {
    let db = TestDb::new("salefree").await;
    let (type_id, _t) = make_type_with_mail(&db, "now", 0).await;
    let event_id = make_event(&db, "probe seam free", false, 0, Some(type_id)).await;
    let order = uuid::Uuid::new_v4();

    // FREE confirm: born OPEN + free + armed — the truth table's free
    // arm (no paid fact will ever come).
    let out = seam(&db).on_order_confirmed(confirmed(order, "0.00", event_id, 1)).await.unwrap();
    assert_eq!(out.registrations_touched, 1);
    let rows = linked(&db, order).await;
    assert_eq!(rows[0].0, "open");
    assert_eq!(rows[0].2, "free");
    let queue = Arc::new(RecordingQueue::default());
    let run = scheduler(&db, queue.clone()).run_due_schedulers().await.unwrap();
    assert_eq!(run.receipts_queued, 1, "the free mint armed at birth");
    db.dispose().await;
}

#[tokio::test]
async fn cancel_cascade_keeps_sale_status_and_paid_heals_forward() {
    let db = TestDb::new("salecancel").await;
    let (type_id, _t) = make_type_with_mail(&db, "now", 0).await;
    let event_id = make_event(&db, "probe seam cancel", false, 0, Some(type_id)).await;
    let order = uuid::Uuid::new_v4();

    seam(&db).on_order_confirmed(confirmed(order, "90.00", event_id, 1)).await.unwrap();
    let cancelled = seam(&db)
        .on_order_cancelled(OrderCancelled {
            delivery_id: None,
            order_id: order,
            company_id: None,
            customer_id: None,
        })
        .await
        .unwrap();
    assert_eq!(cancelled.registrations_touched, 1);
    let rows = linked(&db, order).await;
    assert_eq!(rows[0].0, "cancel", "the whole linked group cascades");
    assert_eq!(rows[0].1, "cancel", "the mirror flips");
    assert_eq!(rows[0].2, "to_pay", "sale_status is KEPT (payability is history)");

    // Cancel replay: no-op.
    let replay = seam(&db)
        .on_order_cancelled(OrderCancelled {
            delivery_id: None,
            order_id: order,
            company_id: None,
            customer_id: None,
        })
        .await
        .unwrap();
    assert!(replay.already_applied);

    // A late paid fact on a cancelled order heals FORWARD (cancel ->
    // open + sold): the recompute never strands a paid attendee.
    seam(&db)
        .on_order_paid(OrderPaid { delivery_id: None, order_id: order, line_ids: vec![] })
        .await
        .unwrap();
    let healed = linked(&db, order).await;
    assert_eq!(healed[0].0, "open", "the paid fact heals even a cancelled row");
    assert_eq!(healed[0].2, "sold");
    db.dispose().await;
}

#[tokio::test]
async fn grand_total_magnitude_reads_zero_only() {
    // The mint reads ONLY the zero/non-zero magnitude; unparseable
    // input classifies PAID (the fail-safe holds seats).
    assert!(grand_total_is_zero("0"));
    assert!(grand_total_is_zero("0.00"));
    assert!(grand_total_is_zero(" 0.0 "));
    assert!(!grand_total_is_zero("10.00"));
    assert!(!grand_total_is_zero("0.01"));
    assert!(!grand_total_is_zero("not-a-number"), "unparseable = paid fail-safe");
}

async fn linked(db: &TestDb, order: uuid::Uuid) -> Vec<(String, String, String)> {
    sqlx::query_as::<_, (String, String, String)>(
        r#"SELECT state::text, sale_order_state::text, sale_status::text
             FROM event.registrations WHERE sale_order_id = $1 ORDER BY id"#,
    )
    .bind(order)
    .fetch_all(&db.pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn officer_register_arms_lead_queue_and_mint_arms_alike() {
    // The lead queue arming (on_create axis) fires for BOTH mint paths
    // and the officer verb — proven by the queue row's existence, not
    // by running generation here (the crm probes run the pass).
    let db = TestDb::new("salearm").await;
    let event_id = make_event(&db, "probe seam arm", false, 0, None).await;
    // A rule must exist for the arm to fire (the EXISTS guard).
    sqlx::query(
        r#"INSERT INTO event.lead_rules (id, name, basis, on_create, on_confirm, on_done, active)
           VALUES ($1, 'probe arm rule', 'registration', true, false, false, true)"#,
    )
    .bind(uuid::Uuid::new_v4())
    .execute(&db.pool)
    .await
    .unwrap();

    let order = uuid::Uuid::new_v4();
    seam(&db).on_order_confirmed(confirmed(order, "25.00", event_id, 1)).await.unwrap();
    let armed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM event.lead_requests WHERE event_id = $1 AND NOT done)",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(armed, "the mint armed the lead queue (on_create axis)");
    db.dispose().await;
}
