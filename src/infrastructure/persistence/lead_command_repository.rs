//! The lead command repository (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! The transactional SQL of the CRM arm: the typed rule CRUD (closed
//! vocabulary, ECR-4 — predicates are ROWS over four axes, never a
//! stored domain), the answer-to-rule bridge verb, the per-event
//! queue's arm/claim/complete cycle (ECR-2), the generation engine's
//! typed predicate SQL, the provenance upsert (the idempotence grain)
//! and the merge-relink verb.
//!
//! NOTHING HERE EVALS ANYTHING. Predicate SQL is COMPOSED from typed
//! rows (axis enums + uuid arrays) — an axis outside the closed
//! vocabulary cannot reach this layer (the service refuses it at
//! parse). The generation eligibility domain is always
//! `state IN ('open','done') AND active` — the EVM2-4 pair.

use backbone_orm::{company_scope, org_scope};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::application::service::event_error::EventError;

use super::seat_repository::record_audit;

/// One typed predicate input (the closed vocabulary as TYPES — the
/// service layer refuses anything outside it at parse).
#[derive(Debug, Clone)]
pub struct PredicateInput {
    pub axis: String, // event | event_type | company | question_answer
    pub question_id: Option<Uuid>,
    pub value_ids: Vec<Uuid>,
}

/// The rule row.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct LeadRuleRow {
    pub id: Uuid,
    pub name: String,
    pub event_id: Option<Uuid>,
    pub basis: String,
    pub on_create: bool,
    pub on_confirm: bool,
    pub on_done: bool,
    pub active: bool,
}

/// The predicate row.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct LeadPredicateRow {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub axis: String,
    pub question_id: Option<Uuid>,
    pub value_ids: serde_json::Value,
}

/// One claimed queue row.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct LeadRequestRow {
    pub id: Uuid,
    pub event_id: Uuid,
    pub done: bool,
    pub error_detail: Option<String>,
    /// The lease stamp: set at claim, NULL once the pass finishes. A
    /// fresh (unexpired) stamp marks the row as walked by someone.
    pub claimed_at: Option<DateTime<Utc>>,
}

/// One eligible registration as the engine sees it.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct EligibleRegistration {
    pub id: Uuid,
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub company_name: Option<String>,
    pub partner_id: Option<Uuid>,
    pub sale_order_id: Option<Uuid>,
    pub created_at: Option<DateTime<Utc>>,
}

/// The lead command repository.
pub struct LeadCommandRepository {
    pool: PgPool,
}

impl LeadCommandRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a rule + its typed predicates in ONE transaction. The
    /// service layer has already validated: the closed vocabulary, the
    /// question_answer axis shape, and the at-least-one-trigger guard.
    /// Creating (or re-activating) a rule ARMS the queue for every
    /// event in its scope (cheap row writes — never an inline run).
    pub async fn create_rule(
        &self,
        name: &str,
        event_id: Option<Uuid>,
        basis: &str,
        on_create: bool,
        on_confirm: bool,
        on_done: bool,
        predicates: &[PredicateInput],
        actor: Option<Uuid>,
    ) -> Result<LeadRuleRow, EventError> {
        let mut tx = self.pool.begin().await?;
        super::relay_ambient_scope(&mut tx).await?;
        let id = Uuid::new_v4();
        let row = sqlx::query_as::<_, LeadRuleRow>(
            r#"INSERT INTO event.lead_rules
                   (id, name, event_id, basis, on_create, on_confirm, on_done, active)
               VALUES ($1, $2, $3, $4::event_lead_basis, $5, $6, $7, true)
               RETURNING id, name, event_id, basis::text AS basis,
                         on_create, on_confirm, on_done, active"#,
        )
        .bind(id)
        .bind(name)
        .bind(event_id)
        .bind(basis)
        .bind(on_create)
        .bind(on_confirm)
        .bind(on_done)
        .fetch_one(&mut *tx)
        .await?;
        for p in predicates {
            sqlx::query(
                r#"INSERT INTO event.lead_rule_predicates
                       (id, rule_id, axis, question_id, value_ids)
                   VALUES ($1, $2, $3::event_lead_predicate_axis, $4, $5::jsonb)"#,
            )
            .bind(Uuid::new_v4())
            .bind(id)
            .bind(&p.axis)
            .bind(p.question_id)
            .bind(serde_json::json!(p
                .value_ids
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()))
            .execute(&mut *tx)
            .await?;
        }
        // Arm the queue for the scope (a global rule arms every event
        // with an eligible-domain registration; a scoped rule arms its
        // event — same cheap write either way).
        sqlx::query(
            r#"INSERT INTO event.lead_requests (event_id)
               SELECT e.id FROM event.events e
                WHERE ($1::uuid IS NULL OR e.id = $1)
               ON CONFLICT (event_id) DO UPDATE SET done = false WHERE lead_requests.done"#,
        )
        .bind(event_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        record_audit(
            &self.pool,
            "lead_rule_created",
            actor,
            "lead_rule",
            id,
            serde_json::json!({ "name": name, "predicates": predicates.len(), "event_id": event_id }),
        )
        .await;
        Ok(row)
    }

    /// Patch a rule (name / scope / axes / active). Deactivating never
    /// un-generates (provenance is history); activating arms the scope.
    pub async fn patch_rule(
        &self,
        rule_id: Uuid,
        name: Option<&str>,
        event_id: Option<Uuid>,
        active: Option<bool>,
        actor: Option<Uuid>,
    ) -> Result<LeadRuleRow, EventError> {
        let row = company_scope::fetch_optional_scoped(
            &self.pool,
            sqlx::query_as::<_, LeadRuleRow>(
                r#"UPDATE event.lead_rules SET
                   name     = COALESCE($2, name),
                   event_id = COALESCE($3, event_id),
                   active   = COALESCE($4, active)
                WHERE id = $1
               RETURNING id, name, event_id, basis::text AS basis,
                         on_create, on_confirm, on_done, active"#,
            )
            .bind(rule_id)
            .bind(name)
            .bind(event_id)
            .bind(active),
        )
        .await?
        .ok_or(EventError::LeadRuleNotFound { rule_id })?;
        if active == Some(true) {
            org_scope::execute_scoped(
                &self.pool,
                sqlx::query(
                    r#"INSERT INTO event.lead_requests (event_id)
                   SELECT e.id FROM event.events e
                    WHERE ($1::uuid IS NULL OR e.id = $1)
                   ON CONFLICT (event_id) DO UPDATE SET done = false WHERE lead_requests.done"#,
                )
                .bind(row.event_id),
            )
            .await?;
        }
        record_audit(
            &self.pool,
            "lead_rule_updated",
            actor,
            "lead_rule",
            rule_id,
            serde_json::json!({ "verb": "patch", "active": active }),
        )
        .await;
        Ok(row)
    }

    pub async fn find_rule(&self, rule_id: Uuid) -> Result<LeadRuleRow, EventError> {
        company_scope::fetch_optional_scoped(&self.pool, sqlx::query_as::<_, LeadRuleRow>(
            r#"SELECT id, name, event_id, basis::text AS basis, on_create, on_confirm, on_done, active
                 FROM event.lead_rules WHERE id = $1"#,
        )
        .bind(rule_id))
        .await?
        .ok_or(EventError::LeadRuleNotFound { rule_id })
    }

    pub async fn list_rules(&self, limit: i64) -> Result<Vec<LeadRuleRow>, EventError> {
        company_scope::fetch_all_scoped(&self.pool, sqlx::query_as::<_, LeadRuleRow>(
            r#"SELECT id, name, event_id, basis::text AS basis, on_create, on_confirm, on_done, active
                 FROM event.lead_rules ORDER BY name, id LIMIT $1"#,
        )
        .bind(limit))
        .await
        .map_err(EventError::from)
    }

    pub async fn predicates_of_rule(
        &self,
        rule_id: Uuid,
    ) -> Result<Vec<LeadPredicateRow>, EventError> {
        company_scope::fetch_all_scoped(
            &self.pool,
            sqlx::query_as::<_, LeadPredicateRow>(
                r#"SELECT id, rule_id, axis::text AS axis, question_id, value_ids
                 FROM event.lead_rule_predicates WHERE rule_id = $1 ORDER BY id"#,
            )
            .bind(rule_id),
        )
        .await
        .map_err(EventError::from)
    }

    /// THE ANSWER-TO-RULE BRIDGE (ECR-4's typed replacement for the
    /// stored-domain editor button): one rule whose single predicate
    /// is (question_answer, question, [answer]) — the same bytes
    /// upstream's eval'd text carried, produced by typed code.
    pub async fn bridge_answer_to_rule(
        &self,
        name: &str,
        event_id: Option<Uuid>,
        on_create: bool,
        on_confirm: bool,
        on_done: bool,
        question_id: Uuid,
        answer_id: Uuid,
        actor: Option<Uuid>,
    ) -> Result<LeadRuleRow, EventError> {
        self.create_rule(
            name,
            event_id,
            "registration",
            on_create,
            on_confirm,
            on_done,
            &[PredicateInput {
                axis: "question_answer".to_string(),
                question_id: Some(question_id),
                value_ids: vec![answer_id],
            }],
            actor,
        )
        .await
    }

    /// The rule READ MODEL (ECS-2 surfaced): rule + predicates + the
    /// per-grouping provenance counters + linked lead count.
    pub async fn rule_read_model(&self, rule_id: Uuid) -> Result<serde_json::Value, EventError> {
        let rule = self.find_rule(rule_id).await?;
        let predicates = self.predicates_of_rule(rule_id).await?;
        let stats = company_scope::fetch_all_scoped(
            &self.pool,
            sqlx::query_as::<_, (String, i64, i64)>(
                r#"SELECT grouping::text AS grouping,
                      count(*) AS groups,
                      count(lead_id) AS with_lead
                 FROM event.lead_provenances WHERE rule_id = $1
                GROUP BY grouping"#,
            )
            .bind(rule_id),
        )
        .await?;
        let per_order = stats
            .iter()
            .find(|(g, _, _)| g == "per_order")
            .map(|s| (s.1, s.2))
            .unwrap_or((0, 0));
        let per_day = stats
            .iter()
            .find(|(g, _, _)| g == "per_event_day")
            .map(|s| (s.1, s.2))
            .unwrap_or((0, 0));
        Ok(serde_json::json!({
            "rule": rule,
            "predicates": predicates,
            "grouping": {
                "per_order": { "groups": per_order.0, "with_lead": per_order.1 },
                "per_event_day": { "groups": per_day.0, "with_lead": per_day.1 },
            },
        }))
    }

    // ── the queue (ECR-2) ───────────────────────────────────────────────────

    /// Arm one event's queue row (public arm: rule created/activated —
    /// the registration verbs arm through the seat repository).
    pub async fn arm_request(&self, event_id: Uuid) -> Result<(), EventError> {
        org_scope::execute_scoped(
            &self.pool,
            sqlx::query(
                r#"INSERT INTO event.lead_requests (event_id) VALUES ($1)
               ON CONFLICT (event_id) DO UPDATE SET done = false WHERE lead_requests.done"#,
            )
            .bind(event_id),
        )
        .await?;
        Ok(())
    }

    /// The claim domain (ECR-2): ONE due request, LEASED. The claim is
    /// a single UPDATE that stamps `claimed_at = now()` — the lease,
    /// not the statement's row locks, is the exclusion: it holds for
    /// the walker's WHOLE pass (the claimer walks after the statement
    /// ends), so a second pass — cron or officer verb — cannot claim
    /// the same event until the walk finishes (the finish clears the
    /// lease) or the lease expires and a later tick re-runs
    /// at-least-once (idempotent through the provenance unique).
    /// SKIP LOCKED keeps two simultaneous claims from waiting on each
    /// other inside the statement; a fresh lease filters the row out
    /// of every later claim. `exclude` holds the CALLING PASS's parked
    /// events (their leases are cleared by the park, so the next pass
    /// — never the parking pass itself — retries them).
    pub async fn claim_next_due_request(
        &self,
        lease_secs: i64,
        exclude: &[Uuid],
    ) -> Result<Option<LeadRequestRow>, EventError> {
        company_scope::fetch_optional_scoped(
            &self.pool,
            sqlx::query_as::<_, LeadRequestRow>(
                r#"WITH due AS (
                   SELECT id FROM event.lead_requests
                    WHERE NOT done
                      AND (claimed_at IS NULL
                           OR claimed_at < now() - make_interval(secs => $1))
                      AND event_id <> ALL($2::uuid[])
                    ORDER BY id
                    LIMIT 1
                    FOR UPDATE SKIP LOCKED
               )
               UPDATE event.lead_requests r
                  SET claimed_at = now()
                 FROM due
                WHERE r.id = due.id
               RETURNING r.id, r.event_id, r.done, r.error_detail, r.claimed_at"#,
            )
            .bind(lease_secs)
            .bind(exclude),
        )
        .await
        .map_err(EventError::from)
    }

    /// The officer verb's claim: lease exactly this event's row. The
    /// verb must never run unserialized against the cron — a fresh
    /// lease is the typed `LeadRequestBusy` refusal (retry after the
    /// walk finishes); an absent row is `LeadRequestNotFound` (nothing
    /// ever armed the event's queue).
    pub async fn try_lease_request_of_event(
        &self,
        event_id: Uuid,
        lease_secs: i64,
    ) -> Result<LeadRequestRow, EventError> {
        let row = company_scope::fetch_optional_scoped(
            &self.pool,
            sqlx::query_as::<_, LeadRequestRow>(
                r#"UPDATE event.lead_requests
                  SET claimed_at = now()
                WHERE event_id = $1
                  AND (claimed_at IS NULL
                       OR claimed_at < now() - make_interval(secs => $2))
               RETURNING id, event_id, done, error_detail, claimed_at"#,
            )
            .bind(event_id)
            .bind(lease_secs),
        )
        .await?;
        match row {
            Some(r) => Ok(r),
            // No lease taken: the row is either absent or freshly
            // held. Which one decides the refusal the officer hears.
            None => {
                let held = company_scope::fetch_one_scalar_scoped(
                    &self.pool,
                    sqlx::query_scalar::<_, i64>(
                        "SELECT count(*) FROM event.lead_requests WHERE event_id = $1",
                    )
                    .bind(event_id),
                )
                .await?;
                if held > 0 {
                    Err(EventError::LeadRequestBusy { event_id })
                } else {
                    Err(EventError::LeadRequestNotFound { event_id })
                }
            }
        }
    }

    /// Complete a request (or park it with the typed failure — done
    /// stays false and the next pass retries). Either way the lease is
    /// RELEASED: the row is claimable again the moment its walk ends.
    pub async fn finish_request(
        &self,
        request_id: Uuid,
        error: Option<&str>,
    ) -> Result<(), EventError> {
        org_scope::execute_scoped(
            &self.pool,
            sqlx::query(
                r#"UPDATE event.lead_requests
                  SET done = $2, error_detail = $3, claimed_at = NULL
                WHERE id = $1"#,
            )
            .bind(request_id)
            .bind(error.is_none())
            .bind(error),
        )
        .await?;
        Ok(())
    }

    /// Same completion keyed on the event (the pass walks one event's
    /// row; the officer verb may not know the row id).
    pub async fn finish_request_by_event(
        &self,
        event_id: Uuid,
        error: Option<&str>,
    ) -> Result<(), EventError> {
        org_scope::execute_scoped(
            &self.pool,
            sqlx::query(
                r#"UPDATE event.lead_requests
                  SET done = $2, error_detail = $3, claimed_at = NULL
                WHERE event_id = $1"#,
            )
            .bind(event_id)
            .bind(error.is_none())
            .bind(error),
        )
        .await?;
        Ok(())
    }

    /// The active rules of an event (scoped or global).
    pub async fn active_rules_of_event(
        &self,
        event_id: Uuid,
    ) -> Result<Vec<LeadRuleRow>, EventError> {
        company_scope::fetch_all_scoped(&self.pool, sqlx::query_as::<_, LeadRuleRow>(
            r#"SELECT id, name, event_id, basis::text AS basis, on_create, on_confirm, on_done, active
                 FROM event.lead_rules
                WHERE active AND (event_id IS NULL OR event_id = $1)
                ORDER BY id"#,
        )
        .bind(event_id))
        .await
        .map_err(EventError::from)
    }

    /// THE ENGINE — the eligible, not-yet-provenanced registrations of
    /// one rule (typed predicate SQL, conjunctive; batch-capped).
    /// Provenanced registrations drop out (the anti-join idempotence
    /// pattern — a re-run walks only NEW rows).
    pub async fn eligible_registrations(
        &self,
        event_id: Uuid,
        rule: &LeadRuleRow,
        predicates: &[LeadPredicateRow],
        batch: i64,
    ) -> Result<Vec<EligibleRegistration>, EventError> {
        // Compose the conjunctive predicate SQL from the typed rows.
        // Parameter numbering: $1 = event_id, $2 = rule_id, $3 = batch,
        // predicates from $4 upward (values first, then the question
        // ref on the question_answer axis — bind order matches).
        let mut clauses = String::new();
        let mut param = 4usize;
        for p in predicates {
            let values =
                format!("SELECT (v #>> '{{}}')::uuid FROM jsonb_array_elements(${param}::jsonb) v");
            param += 1;
            match p.axis.as_str() {
                "event" => {
                    clauses.push_str(&format!(" AND r.event_id IN ({values})"));
                }
                "event_type" => {
                    clauses.push_str(&format!(
                        " AND EXISTS (SELECT 1 FROM event.events e \
                          JOIN event.types t ON t.id = e.event_type_id \
                          WHERE e.id = r.event_id AND t.id IN ({values}))"
                    ));
                }
                "company" => {
                    // The registration's org anchor (decorator-installed
                    // column): the axis matches registrations anchored
                    // at one of the given units. Undecorated module
                    // tests never store a company predicate — the axis
                    // only exists where the composing decorator does.
                    clauses.push_str(&format!(" AND r.org_unit_id IN ({values})"));
                }
                "question_answer" => {
                    clauses.push_str(&format!(
                        " AND EXISTS (SELECT 1 FROM event.registration_answers ra \
                          WHERE ra.registration_id = r.id AND ra.question_id = ${param_q} \
                            AND ra.value_answer_id IN ({values}))",
                        param_q = param
                    ));
                    param += 1;
                }
                // The closed vocabulary is enforced at parse; a bad
                // row here is corruption, not input.
                other => {
                    return Err(EventError::Internal(format!(
                        "predicate axis outside the closed vocabulary: {other}"
                    )))
                }
            }
        }
        let sql = format!(
            r#"SELECT r.id, r.name, r.email, r.phone, r.company_name, r.partner_id,
                      r.sale_order_id, (r.metadata->>'created_at')::timestamptz AS created_at
                 FROM event.registrations r
                WHERE r.event_id = $1
                  AND r.state IN ('open','done') AND r.active
                  AND NOT EXISTS (
                      SELECT 1 FROM event.lead_provenance_registrations j
                        JOIN event.lead_provenances p ON p.id = j.provenance_id
                       WHERE p.rule_id = $2 AND j.registration_id = r.id){clauses}
                ORDER BY r.id
                LIMIT $3"#
        );

        let mut q = sqlx::query_as::<_, EligibleRegistration>(&sql)
            .bind(event_id)
            .bind(rule.id)
            .bind(batch);
        for p in predicates {
            q = q.bind(&p.value_ids);
            if p.axis == "question_answer" {
                q = q.bind(p.question_id);
            }
        }
        company_scope::fetch_all_scoped(&self.pool, q)
            .await
            .map_err(EventError::from)
    }

    /// The provenance upsert: INSERT the (rule, group) row if new
    /// (returns Some(id, None)); an existing group returns its (id,
    /// lead_id) so the caller can grow it through the sink's update.
    pub async fn upsert_provenance(
        &self,
        rule_id: Uuid,
        event_id: Uuid,
        group_key: &str,
        grouping: &str,
    ) -> Result<(Uuid, Option<Uuid>), EventError> {
        company_scope::fetch_one_scoped(
            &self.pool,
            sqlx::query_as::<_, (Uuid, Option<Uuid>)>(
                r#"INSERT INTO event.lead_provenances
                   (id, rule_id, event_id, group_key, grouping)
               VALUES ($1, $2, $3, $4, $5::event_lead_grouping)
               ON CONFLICT (rule_id, group_key) DO UPDATE SET rule_id = EXCLUDED.rule_id
               RETURNING id, lead_id"#,
            )
            .bind(Uuid::new_v4())
            .bind(rule_id)
            .bind(event_id)
            .bind(group_key)
            .bind(grouping),
        )
        .await
        .map_err(EventError::from)
    }

    /// The junctioned members of one provenance group — the FULL-group
    /// payload the sink consumes (a grown group's update carries every
    /// member, not just this walk's new rows).
    pub async fn group_members(
        &self,
        provenance_id: Uuid,
    ) -> Result<Vec<EligibleRegistration>, EventError> {
        company_scope::fetch_all_scoped(
            &self.pool,
            sqlx::query_as::<_, EligibleRegistration>(
                r#"SELECT r.id, r.name, r.email, r.phone, r.company_name, r.partner_id,
                      r.sale_order_id, (r.metadata->>'created_at')::timestamptz AS created_at
                 FROM event.lead_provenance_registrations j
                 JOIN event.registrations r ON r.id = j.registration_id
                WHERE j.provenance_id = $1
                ORDER BY r.id"#,
            )
            .bind(provenance_id),
        )
        .await
        .map_err(EventError::from)
    }

    /// Record the lead id the sink minted for a provenance row.
    pub async fn set_provenance_lead(
        &self,
        provenance_id: Uuid,
        lead_id: Uuid,
    ) -> Result<(), EventError> {
        org_scope::execute_scoped(
            &self.pool,
            sqlx::query("UPDATE event.lead_provenances SET lead_id = $2 WHERE id = $1")
                .bind(provenance_id)
                .bind(lead_id),
        )
        .await?;
        Ok(())
    }

    /// Add the junction rows (idempotent — the pair unique); returns
    /// how many were NEW (the grown-group signal for the sink update).
    pub async fn junction_add(
        &self,
        provenance_id: Uuid,
        registration_ids: &[Uuid],
    ) -> Result<usize, EventError> {
        if registration_ids.is_empty() {
            return Ok(0);
        }
        let rows = company_scope::fetch_all_rows_scoped(&self.pool, sqlx::query(
            r#"INSERT INTO event.lead_provenance_registrations (id, provenance_id, registration_id)
               SELECT gen_random_uuid(), $1, x FROM unnest($2::uuid[]) AS x
               ON CONFLICT (provenance_id, registration_id) DO NOTHING
               RETURNING id"#,
        )
        .bind(provenance_id)
        .bind(registration_ids))
        .await?;
        Ok(rows.len())
    }

    /// THE MERGE-RELINK VERB: when the host's lead side merges two
    /// leads, every provenance row pointing at the absorbed lead moves
    /// to the survivor in ONE update. Returns the rows moved.
    pub async fn relink_lead(
        &self,
        old_lead_id: Uuid,
        new_lead_id: Uuid,
        actor: Option<Uuid>,
    ) -> Result<usize, EventError> {
        let rows = company_scope::fetch_all_rows_scoped(
            &self.pool,
            sqlx::query(
                r#"UPDATE event.lead_provenances SET lead_id = $2
                WHERE lead_id = $1 AND $1 <> $2
               RETURNING id"#,
            )
            .bind(old_lead_id)
            .bind(new_lead_id),
        )
        .await?;
        let n = rows.len();
        record_audit(
            &self.pool,
            "lead_relinked",
            actor,
            "lead",
            new_lead_id,
            serde_json::json!({ "from": old_lead_id, "moved": n }),
        )
        .await;
        Ok(n)
    }

    /// Officer read: an event's queue row state (the lease stamp tells
    /// the officer whether a walk is in flight).
    pub async fn find_request_of_event(
        &self,
        event_id: Uuid,
    ) -> Result<Option<LeadRequestRow>, EventError> {
        company_scope::fetch_optional_scoped(&self.pool, sqlx::query_as::<_, LeadRequestRow>(
            "SELECT id, event_id, done, error_detail, claimed_at FROM event.lead_requests WHERE event_id = $1",
        )
        .bind(event_id))
        .await
        .map_err(EventError::from)
    }
}
