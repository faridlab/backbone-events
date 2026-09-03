//! The lead-pass EXCLUSIVITY probes (ECR-2's lease claim): two
//! concurrent passes claim each event's queue row exactly once (one
//! create_lead per group, one lead id — even with a sink whose create
//! outlives the claim statement by far, which is exactly the
//! double-claim window), the officer run verb refuses TYPED while
//! another walk holds the lease, an unarmed event refuses TYPED on
//! the verb, and a dead walker's stale lease is reclaimed by the next
//! tick (at-least-once).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use backbone_events::application::service::event_error::EventError;
use backbone_events::application::service::lead_command_service::{
    CreateRuleInput, LeadRuleCommandService, PredicateSpec,
};
use backbone_events::application::service::lead_generation_service::{
    LeadGenerationService, LEASE_SECONDS,
};
use backbone_events::application::service::lead_sink::{EventLeadSink, LeadGroup};
use backbone_events::infrastructure::persistence::lead_command_repository::LeadCommandRepository;

use super::common::{make_event, register_cmd, registrations, TestDb};

fn rules(db: &TestDb) -> LeadRuleCommandService {
    LeadRuleCommandService::new(LeadCommandRepository::new(db.pool.clone()))
}

/// Arm the queue: one walk-in registration + one active rule matching
/// it (the registration verb and the rule create both arm).
async fn armed_event(db: &TestDb) -> uuid::Uuid {
    let event_id = make_event(db, "probe exclusivity", false, 0, None).await;
    registrations(db)
        .register(register_cmd(event_id, 1))
        .await
        .unwrap();
    rules(db)
        .create_rule(
            CreateRuleInput {
                name: "probe exclusivity rule".into(),
                event_id: None,
                basis: None,
                on_create: Some(true),
                on_confirm: None,
                on_done: None,
                predicates: vec![PredicateSpec {
                    axis: "event".into(),
                    question_id: None,
                    value_ids: vec![event_id],
                }],
            },
            None,
        )
        .await
        .unwrap();
    event_id
}

/// The slow sink: every create outlives any claim statement by far —
/// the create window the lease must cover.
#[derive(Default)]
struct SlowSink {
    created: Mutex<Vec<uuid::Uuid>>,
}

#[async_trait]
impl EventLeadSink for SlowSink {
    async fn create_lead(&self, _group: &LeadGroup) -> Result<uuid::Uuid, String> {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let id = uuid::Uuid::new_v4();
        self.created.lock().unwrap().push(id);
        Ok(id)
    }
    async fn update_lead(&self, _lead_id: uuid::Uuid, _group: &LeadGroup) -> Result<(), String> {
        Ok(())
    }
}

/// The gated sink: create signals it started, then waits for the
/// release — the officer verb runs against a walk that is provably
/// mid-flight (inside create_lead).
struct GatedSink {
    entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    release: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

impl GatedSink {
    fn new(
        entered: tokio::sync::oneshot::Sender<()>,
        release: tokio::sync::oneshot::Receiver<()>,
    ) -> Self {
        Self {
            entered: Mutex::new(Some(entered)),
            release: Mutex::new(Some(release)),
        }
    }
}

#[async_trait]
impl EventLeadSink for GatedSink {
    async fn create_lead(&self, _group: &LeadGroup) -> Result<uuid::Uuid, String> {
        if let Some(tx) = self.entered.lock().unwrap().take() {
            let _ = tx.send(());
        }
        let release = self.release.lock().unwrap().take();
        if let Some(rx) = release {
            let _ = rx.await;
        }
        Ok(uuid::Uuid::new_v4())
    }
    async fn update_lead(&self, _lead_id: uuid::Uuid, _group: &LeadGroup) -> Result<(), String> {
        Ok(())
    }
}

#[tokio::test]
async fn concurrent_passes_create_each_lead_once() {
    let db = TestDb::new("leadexcl").await;
    let event_id = armed_event(&db).await;

    // Two independent walkers (two service instances, one shared
    // pool), both started at once; the create takes 300ms — far past
    // the claim statement, squarely inside the old double-claim
    // window.
    let sink = Arc::new(SlowSink::default());
    let a = LeadGenerationService::new(
        LeadCommandRepository::new(db.pool.clone()),
        sink.clone(),
    );
    let b = LeadGenerationService::new(
        LeadCommandRepository::new(db.pool.clone()),
        sink.clone(),
    );
    let (ra, rb) = tokio::join!(a.run_due_lead_requests(), b.run_due_lead_requests());
    let (ra, rb) = (ra.unwrap(), rb.unwrap());

    // THE LEASE: exactly one walker claimed the row, no matter which.
    assert_eq!(
        ra.requests_claimed + rb.requests_claimed,
        1,
        "the lease gives the row to ONE pass"
    );
    // One group, one create_lead, one lead id.
    assert_eq!(
        sink.created.lock().unwrap().len(),
        1,
        "exactly one durable create per group"
    );
    assert_eq!(ra.leads_created + rb.leads_created, 1);

    // The row completed and the lease was RELEASED (a later pass or
    // the officer verb can take it immediately).
    let row = LeadCommandRepository::new(db.pool.clone())
        .find_request_of_event(event_id)
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("the queue row of {event_id} must exist"));
    assert!(row.done);
    assert!(row.claimed_at.is_none(), "finish clears the lease");

    // A re-run with nothing new is a clean no-op.
    let again = LeadGenerationService::new(
        LeadCommandRepository::new(db.pool.clone()),
        sink.clone(),
    )
    .run_due_lead_requests()
    .await
    .unwrap();
    assert_eq!(again.requests_claimed, 0);
    db.dispose().await;
}

#[tokio::test]
async fn the_run_verb_refuses_typed_while_a_walk_holds_the_lease() {
    let db = TestDb::new("leadbusy").await;
    let event_id = armed_event(&db).await;

    let (enter_tx, enter_rx) = tokio::sync::oneshot::channel::<()>();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
    let sink = Arc::new(GatedSink::new(enter_tx, release_rx));

    let walker = LeadGenerationService::new(
        LeadCommandRepository::new(db.pool.clone()),
        sink.clone(),
    );
    let handle = tokio::spawn(async move { walker.run_due_lead_requests().await.unwrap() });

    // The walker is now INSIDE create_lead — mid-walk, lease held.
    enter_rx.await.unwrap();

    // The officer verb (its own service instance) must refuse TYPED,
    // never run unserialized against the cron's walk.
    let verb = LeadGenerationService::new(
        LeadCommandRepository::new(db.pool.clone()),
        sink,
    );
    match verb.run_request_of_event(event_id).await {
        Err(EventError::LeadRequestBusy { event_id: busy }) => assert_eq!(busy, event_id),
        other => panic!("expected LeadRequestBusy mid-walk, got {other:?}"),
    }

    // Release the walk; it completes cleanly exactly once.
    release_tx.send(()).unwrap();
    let run = handle.await.unwrap();
    assert_eq!(run.requests_claimed, 1);
    assert_eq!(run.leads_created, 1);

    // With the walk finished, the SAME verb runs (nothing new — but
    // the lease is free, so no refusal).
    let repo = LeadCommandRepository::new(db.pool.clone());
    let verb = LeadGenerationService::new(repo, Arc::new(backbone_events::application::service::lead_sink::RefusingLeadSink));
    let rerun = verb.run_request_of_event(event_id).await.unwrap();
    assert_eq!(rerun.requests_claimed, 1);
    assert_eq!(rerun.leads_created, 0, "nothing new to walk");
    db.dispose().await;
}

#[tokio::test]
async fn the_run_verb_on_an_unarmed_event_is_typed_not_found() {
    let db = TestDb::new("leadabsent").await;
    let verb = LeadGenerationService::new(
        LeadCommandRepository::new(db.pool.clone()),
        Arc::new(backbone_events::application::service::lead_sink::RefusingLeadSink),
    );
    // No registration, no rule, no queue row — nothing ever armed.
    match verb.run_request_of_event(uuid::Uuid::new_v4()).await {
        Err(EventError::LeadRequestNotFound { .. }) => {}
        other => panic!("expected LeadRequestNotFound on the unarmed event, got {other:?}"),
    }
    db.dispose().await;
}

#[tokio::test]
async fn a_dead_walkers_stale_lease_is_reclaimed() {
    let db = TestDb::new("leadstale").await;
    let event_id = armed_event(&db).await;

    let repo = LeadCommandRepository::new(db.pool.clone());
    // A walker claims and DIES (no finish — the lease is left stale).
    let claimed = repo
        .claim_next_due_request(LEASE_SECONDS, &[])
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("the armed row of {event_id} must claim"));
    assert!(claimed.claimed_at.is_some(), "the claim leases the row");

    // A second walker inside the lease window: nothing to claim.
    let blocked = repo
        .claim_next_due_request(LEASE_SECONDS, &[])
        .await
        .unwrap();
    assert!(blocked.is_none(), "a fresh lease holds the row");

    // The lease expires (backdated past the window — a dead walker's
    // 15-minute-old lease): the next tick re-claims at-least-once.
    sqlx::query(
        "UPDATE event.lead_requests SET claimed_at = now() - interval '1 hour' WHERE event_id = $1",
    )
    .bind(event_id)
    .execute(&db.pool)
    .await
    .unwrap();
    let reclaimed = repo
        .claim_next_due_request(LEASE_SECONDS, &[])
        .await
        .unwrap();
    assert!(reclaimed.is_some(), "an expired lease is reclaimable");
    db.dispose().await;
}
