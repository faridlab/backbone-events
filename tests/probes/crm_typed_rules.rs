//! The CRM probes: the closed predicate vocabulary (ECR-4 — every
//! refusal typed), the answer-to-rule bridge, the self-arming queue's
//! run cycle with a host sink, the grouping strategy (per_order for
//! sale-linked, per_event_day for walk-ins — ECS-1/ECS-2), the
//! provenance idempotence (a re-run walks only NEW rows), the refusing
//! sink's LOUD park, and the merge-relink verb.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backbone_events::application::service::event_error::EventError;
use backbone_events::application::service::lead_command_service::{
    CreateRuleInput, FromAnswerInput, LeadRuleCommandService, PredicateSpec, RelinkInput,
};
use backbone_events::application::service::lead_generation_service::LeadGenerationService;
use backbone_events::application::service::lead_sink::{EventLeadSink, LeadGroup};
use backbone_events::infrastructure::persistence::lead_command_repository::LeadCommandRepository;

use super::common::{make_event, register_cmd, registrations, TestDb};

fn rules(db: &TestDb) -> LeadRuleCommandService {
    LeadRuleCommandService::new(LeadCommandRepository::new(db.pool.clone()))
}

/// The recording sink: create mints a fresh lead id per group, update
/// folds members in — the host adapter's shape.
#[derive(Default)]
struct RecordingSink {
    created: Mutex<Vec<(uuid::Uuid, String)>>, // (lead_id, group_key)
    updated: Mutex<Vec<(uuid::Uuid, usize)>>,  // (lead_id, members)
}

#[async_trait]
impl EventLeadSink for RecordingSink {
    async fn create_lead(&self, group: &LeadGroup) -> Result<uuid::Uuid, String> {
        let id = uuid::Uuid::new_v4();
        self.created
            .lock()
            .unwrap()
            .push((id, group.group_key.clone()));
        Ok(id)
    }
    async fn update_lead(&self, lead_id: uuid::Uuid, group: &LeadGroup) -> Result<(), String> {
        self.updated
            .lock()
            .unwrap()
            .push((lead_id, group.registrations.len()));
        Ok(())
    }
}

fn run_input(name: &str) -> CreateRuleInput {
    CreateRuleInput {
        name: name.to_string(),
        event_id: None,
        basis: None,
        on_create: Some(true),
        on_confirm: None,
        on_done: None,
        predicates: vec![],
    }
}

#[tokio::test]
async fn the_predicate_vocabulary_is_closed() {
    let db = TestDb::new("crmvocab").await;
    let event_id = make_event(&db, "probe vocab", false, 0, None).await;

    // An axis outside the four is refused at parse.
    let bad = CreateRuleInput {
        predicates: vec![PredicateSpec {
            axis: "eval_this".into(),
            question_id: None,
            value_ids: vec![event_id],
        }],
        ..run_input("bad axis")
    };
    match rules(&db).create_rule(bad, None).await {
        Err(EventError::Validation(msg)) => assert!(msg.contains("closed vocabulary")),
        other => panic!("expected the closed-vocabulary refusal, got {other:?}"),
    }

    // question_answer WITHOUT its question ref: refused.
    let no_q = CreateRuleInput {
        predicates: vec![PredicateSpec {
            axis: "question_answer".into(),
            question_id: None,
            value_ids: vec![uuid::Uuid::new_v4()],
        }],
        ..run_input("no question")
    };
    match rules(&db).create_rule(no_q, None).await {
        Err(EventError::Validation(msg)) => assert!(msg.contains("question_id")),
        other => panic!("expected the question-ref refusal, got {other:?}"),
    }

    // A question ref on a NON-question axis: refused.
    let stray_q = CreateRuleInput {
        predicates: vec![PredicateSpec {
            axis: "event".into(),
            question_id: Some(uuid::Uuid::new_v4()),
            value_ids: vec![event_id],
        }],
        ..run_input("stray question")
    };
    match rules(&db).create_rule(stray_q, None).await {
        Err(EventError::Validation(msg)) => assert!(msg.contains("no question_id")),
        other => panic!("expected the stray-question refusal, got {other:?}"),
    }

    // No values: refused.
    let empty_values = CreateRuleInput {
        predicates: vec![PredicateSpec {
            axis: "event".into(),
            question_id: None,
            value_ids: vec![],
        }],
        ..run_input("no values")
    };
    match rules(&db).create_rule(empty_values, None).await {
        Err(EventError::Validation(msg)) => assert!(msg.contains("no values")),
        other => panic!("expected the empty-values refusal, got {other:?}"),
    }

    // No trigger armed: refused.
    let no_trigger = CreateRuleInput {
        on_create: Some(false),
        on_confirm: Some(false),
        on_done: Some(false),
        predicates: vec![PredicateSpec {
            axis: "event".into(),
            question_id: None,
            value_ids: vec![event_id],
        }],
        ..run_input("no trigger")
    };
    match rules(&db).create_rule(no_trigger, None).await {
        Err(EventError::Validation(msg)) => assert!(msg.contains("trigger")),
        other => panic!("expected the trigger-guard refusal, got {other:?}"),
    }
    db.dispose().await;
}

#[tokio::test]
async fn typed_rules_run_group_and_idempote() {
    let db = TestDb::new("crmrun").await;
    let event_id = make_event(&db, "probe run", false, 0, None).await;
    let reg_service = registrations(&db);

    // Two walk-in registrations BEFORE the rule exists (the queue arms
    // for them too — arming is cheap and set-based).
    let r1 = reg_service.register(register_cmd(event_id, 1)).await.unwrap();
    let r2 = reg_service.register(register_cmd(event_id, 2)).await.unwrap();
    let _ = (r1, r2);

    // The typed rule: event axis, on_create + on_confirm.
    let rule = rules(&db)
        .create_rule(
            CreateRuleInput {
                name: "probe rule".into(),
                event_id: None,
                basis: None,
                on_create: Some(true),
                on_confirm: Some(true),
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
    assert!(rule.active, "a rule is born active and ARMS the queue");

    let sink = Arc::new(RecordingSink::default());
    let generation = LeadGenerationService::new(
        LeadCommandRepository::new(db.pool.clone()),
        sink.clone(),
    );

    // The pass: both walk-ins match, ONE per_event_day group (same
    // event, same day), one lead created.
    let run = generation.run_due_lead_requests().await.unwrap();
    assert_eq!(run.requests_claimed, 1);
    assert_eq!(run.registrations_matched, 2);
    assert_eq!(run.groups_created, 1, "walk-ins group per event-day (ECS-1)");
    assert_eq!(run.leads_created, 1);
    assert_eq!(run.requests_completed, 1);
    {
        let created = sink.created.lock().unwrap();
        assert_eq!(created.len(), 1);
        assert!(created[0].1.starts_with(&format!("{event_id}:")), "group_key is event:day");
    }

    // The rule read model surfaces both grouping grains (ECS-2).
    let read = rules(&db).rule_read_model(rule.id).await.unwrap();
    assert_eq!(read["grouping"]["per_event_day"]["groups"], 1);
    assert_eq!(read["grouping"]["per_event_day"]["with_lead"], 1);
    assert_eq!(read["grouping"]["per_order"]["groups"], 0);

    // A THIRD registration re-arms the queue; the next pass walks ONLY
    // the new row and GROWS the existing group through the sink's
    // update (provenance idempotence).
    reg_service.register(register_cmd(event_id, 3)).await.unwrap();
    let run2 = generation.run_due_lead_requests().await.unwrap();
    assert_eq!(run2.registrations_matched, 1, "provenance anti-join: only NEW rows walk");
    assert_eq!(run2.groups_grown, 1, "the same-day group grew");
    assert_eq!(run2.leads_created, 0);
    {
        let updated = sink.updated.lock().unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].1, 3, "update_lead saw all three members");
    }

    // Re-running a DONE queue with nothing new is a clean no-op.
    let run3 = generation.run_due_lead_requests().await.unwrap();
    assert_eq!(run3.requests_claimed, 0);
    db.dispose().await;
}

#[tokio::test]
async fn sale_linked_registrations_group_per_order() {
    let db = TestDb::new("crmorder").await;
    let event_id = make_event(&db, "probe order groups", false, 0, None).await;
    let reg_service = registrations(&db);

    // One walk-in + two orders' worth of minted registrations.
    reg_service.register(register_cmd(event_id, 1)).await.unwrap();
    let seam = backbone_events::application::service::sale_seam_service::SaleSeamService::new(
        backbone_events::infrastructure::persistence::sale_seam_repository::SaleSeamRepository::new(
            db.pool.clone(),
        ),
    );
    for n in 2..=3 {
        let order = uuid::Uuid::new_v4();
        seam.on_order_confirmed(backbone_events::application::service::sale_seam_service::OrderConfirmed {
            delivery_id: None,
            order_id: order,
            org_unit_id: None,
            customer_id: None,
            grand_total: "0.00".into(), // free: born open + eligible now
            currency: None,
            registrations: vec![backbone_events::application::service::sale_seam_service::RegistrationSpec {
                event_id,
                event_slot_id: None,
                event_ticket_id: None,
                name: format!("Buyer {n}"),
                email: format!("buyer{n}@probe.test"),
                phone: None,
                company_name: None,
                partner_id: None,
            }],
        })
        .await
        .unwrap();
    }

    rules(&db)
        .create_rule(
            CreateRuleInput {
                name: "probe order rule".into(),
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

    let sink = Arc::new(RecordingSink::default());
    let run = LeadGenerationService::new(
        LeadCommandRepository::new(db.pool.clone()),
        sink,
    )
    .run_due_lead_requests()
    .await
    .unwrap();
    assert_eq!(run.registrations_matched, 3);
    // per_order groups for the two mints + one per_event_day group for
    // the walk-in: BOTH grains in one pass, one lead per group.
    assert_eq!(run.groups_created, 3, "2 per_order + 1 per_event_day");
    assert_eq!(run.leads_created, 3);
    db.dispose().await;
}

#[tokio::test]
async fn question_answer_predicate_matches_only_the_answer() {
    let db = TestDb::new("crmanswer").await;
    let event_id = make_event(&db, "probe answers", false, 0, None).await;

    // A question + two suggested answers.
    let question = uuid::Uuid::new_v4();
    let yes = uuid::Uuid::new_v4();
    let no = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO event.questions (id, question) VALUES ($1, 'VIP?')")
        .bind(question)
        .execute(&db.pool)
        .await
        .unwrap();
    for (id, name) in [(yes, "yes"), (no, "no")] {
        sqlx::query("INSERT INTO event.question_answers (id, question_id, name) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(question)
            .bind(name)
            .execute(&db.pool)
            .await
            .unwrap();
    }

    // Two registrations, one answering YES and one NO.
    let vip = registrations(&db).register(register_cmd(event_id, 1)).await.unwrap();
    let plain = registrations(&db).register(register_cmd(event_id, 2)).await.unwrap();
    for (reg, answer) in [(vip.id, yes), (plain.id, no)] {
        sqlx::query(
            r#"INSERT INTO event.registration_answers (id, registration_id, question_id, value_answer_id)
               VALUES ($1, $2, $3, $4)"#,
        )
        .bind(uuid::Uuid::new_v4())
        .bind(reg)
        .bind(question)
        .bind(answer)
        .execute(&db.pool)
        .await
        .unwrap();
    }

    // THE ANSWER-TO-RULE BRIDGE: one rule whose single predicate is the
    // typed (question, [answer]) pair.
    let rule = rules(&db)
        .from_answer(
            FromAnswerInput {
                name: "probe vip".into(),
                event_id: Some(event_id),
                on_create: Some(true),
                on_confirm: None,
                on_done: None,
                question_id: question,
                answer_id: yes,
            },
            None,
        )
        .await
        .unwrap();
    let read = rules(&db).rule_read_model(rule.id).await.unwrap();
    let predicates = read["predicates"].as_array().unwrap();
    assert_eq!(predicates.len(), 1);
    assert_eq!(predicates[0]["axis"], "question_answer", "typed bytes, no stored domain");

    let sink = Arc::new(RecordingSink::default());
    let run = LeadGenerationService::new(LeadCommandRepository::new(db.pool.clone()), sink)
        .run_due_lead_requests()
        .await
        .unwrap();
    assert_eq!(run.registrations_matched, 1, "only the YES answer matches");
    db.dispose().await;
}

#[tokio::test]
async fn refusing_sink_parks_the_request_loudly() {
    let db = TestDb::new("crmpark").await;
    let event_id = make_event(&db, "probe park", false, 0, None).await;
    registrations(&db).register(register_cmd(event_id, 1)).await.unwrap();

    rules(&db)
        .create_rule(
            CreateRuleInput {
                name: "probe park rule".into(),
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

    // The REFUSING default sink: the request parks with the typed
    // reason (done stays false) — never a silent skip.
    let run = LeadGenerationService::new(
        LeadCommandRepository::new(db.pool.clone()),
        Arc::new(backbone_events::application::service::lead_sink::RefusingLeadSink),
    )
    .run_due_lead_requests()
    .await
    .unwrap();
    assert_eq!(run.requests_parked, 1);
    let parked: Option<(bool, Option<String>)> =
        sqlx::query_as("SELECT done, error_detail FROM event.lead_requests WHERE event_id = $1")
            .bind(event_id)
            .fetch_optional(&db.pool)
            .await
            .unwrap();
    let (done, error) = parked.unwrap();
    assert!(!done, "parked is NOT done — retried next tick");
    assert!(error.unwrap().contains("no EventLeadSink"), "the park reason is loud");

    // Wiring a sink on the next tick completes the same request.
    let sink = Arc::new(RecordingSink::default());
    let heal = LeadGenerationService::new(LeadCommandRepository::new(db.pool.clone()), sink)
        .run_due_lead_requests()
        .await
        .unwrap();
    assert_eq!(heal.leads_created, 1, "the retry after wiring completes");
    db.dispose().await;
}

#[tokio::test]
async fn merge_relink_moves_provenance_rows() {
    let db = TestDb::new("crmrelink").await;
    let event_id = make_event(&db, "probe relink", false, 0, None).await;
    registrations(&db).register(register_cmd(event_id, 1)).await.unwrap();
    rules(&db)
        .create_rule(
            CreateRuleInput {
                name: "probe relink rule".into(),
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
    let sink = Arc::new(RecordingSink::default());
    LeadGenerationService::new(LeadCommandRepository::new(db.pool.clone()), sink.clone())
        .run_due_lead_requests()
        .await
        .unwrap();
    let old_lead = sink.created.lock().unwrap()[0].0;

    // The host's lead side merges: every provenance row pointing at the
    // absorbed lead moves to the survivor in ONE update.
    let new_lead = uuid::Uuid::new_v4();
    let moved = rules(&db)
        .relink_lead(
            RelinkInput { old_lead_id: old_lead, new_lead_id: new_lead },
            None,
        )
        .await
        .unwrap();
    assert_eq!(moved, 1);
    let now: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT lead_id FROM event.lead_provenances WHERE lead_id = $1")
            .bind(new_lead)
            .fetch_optional(&db.pool)
            .await
            .unwrap();
    assert!(now.is_some(), "the provenance follows the survivor");
    db.dispose().await;
}
