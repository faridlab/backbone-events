//! The self-arming lead generation pass (hand-written; user-owned; see
//! `metaphor.codegen.yaml`) — ADR-0020.
//!
//! ONE pass over the claim domain (self_arming + pickup_lock +
//! commit_per_batch — the scheduled job `event-lead-generation`):
//! registration verbs and rule verbs arm with cheap row writes; the
//! pass NEVER runs inline in a verb. Per claimed request (one event):
//!
//! 1. the event's ACTIVE rules (scoped or global);
//! 2. per rule: the eligible registrations (typed predicate SQL,
//!    batch-capped, provenance anti-joined — a re-run walks only NEW
//!    rows);
//! 3. THE GROUPING (ECS-1 as strategy, not column): sale-linked
//!    registrations group per_order (one group per sale order);
//!    walk-ins group per_event_day (one group per event per day);
//! 4. per group: the provenance upsert (the idempotence grain —
//!    (rule_id, group_key) unique), then the SINK over the FULL
//!    membership, then the junction rows: a new group CREATES its
//!    lead through the host port, a grown existing group UPDATES its
//!    lead. Both grouping grains are surfaced in the rule read model
//!    (ECS-2);
//! 5. the request completes (`done`) — unless ANY group's sink call
//!    refused, in which case the request PARKS with the typed reason
//!    (`error_detail`, done stays false): the next tick retries. An
//!    unwired host parks loudly; it never silently skips.
//!
//! Provenance is history: deactivating a rule never un-generates, and
//! the junction's anti-join makes the whole pass idempotent per
//! (rule, registration).

use std::collections::BTreeMap;
use std::sync::Arc;

use uuid::Uuid;

use super::event_error::{EventError, EventResult};
use super::lead_sink::{EventLeadSink, LeadGroup, LeadRegistrationView};
use crate::infrastructure::persistence::lead_command_repository::{
    EligibleRegistration, LeadCommandRepository,
};

/// `EVENT_LEAD_BATCH` — registrations evaluated per rule per pass
/// (default 200).
pub const DEFAULT_LEAD_BATCH: i64 = 200;

/// `EVENT_LEAD_CRON_LIMIT` — queue rows claimed per pass (default
/// 1000).
pub const DEFAULT_LEAD_CRON_LIMIT: i64 = 1000;

/// The generation-pass LEASE (seconds). A walker leases a request row
/// for its whole walk; a walker that dies mid-pass leaves a stale
/// lease that expires here — the next tick re-runs the event
/// at-least-once (idempotent through the provenance unique). The value
/// is the schedule's floor tick (*/15): a stale lease never outlives
/// more than one missed tick.
pub const LEASE_SECONDS: i64 = 900;

/// The grouping strategy's two grains (ECS-1 — a strategy, never a
/// stored column; ECS-2 — both surfaced in the rule read model).
pub const GROUPING_PER_ORDER: &str = "per_order";
pub const GROUPING_PER_EVENT_DAY: &str = "per_event_day";

/// One pass's summary (probe + surface shape).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct LeadRunSummary {
    pub requests_claimed: i64,
    pub rules_evaluated: i64,
    pub registrations_matched: i64,
    pub groups_created: i64,
    pub groups_grown: i64,
    pub leads_created: i64,
    pub leads_updated: i64,
    pub requests_completed: i64,
    pub requests_parked: i64,
}

/// The lead generation service.
pub struct LeadGenerationService {
    leads: LeadCommandRepository,
    sink: Arc<dyn EventLeadSink>,
    batch: i64,
    cron_limit: i64,
}

impl LeadGenerationService {
    /// Compose with the host-installed sink (the refusing default
    /// parks requests loudly — never a silent skip).
    pub fn new(leads: LeadCommandRepository, sink: Arc<dyn EventLeadSink>) -> Self {
        Self {
            leads,
            sink,
            batch: DEFAULT_LEAD_BATCH,
            cron_limit: DEFAULT_LEAD_CRON_LIMIT,
        }
    }

    /// Override the batch/cron caps (probes + host policy).
    pub fn with_caps(mut self, batch: i64, cron_limit: i64) -> Self {
        self.batch = batch.max(1);
        self.cron_limit = cron_limit.max(1);
        self
    }

    /// ONE PASS over the due queue. Safe to run concurrently on two
    /// hosts: each request is claimed by LEASE (one UPDATE stamping
    /// `claimed_at`), so the second walker cannot take an event the
    /// first is still walking — the claim covers the whole per-event
    /// walk, not just the claim statement. Each row is claimed
    /// immediately before its own walk (never claimed in bulk), so a
    /// lease never expires while its row still waits in a claimed
    /// batch. A parked row's lease is cleared by the park itself; the
    /// parking pass EXCLUDES its own parked events from re-claiming
    /// (the next pass retries them — never a park/retry loop inside
    /// one pass). An error mid-pass leaves the current row's lease to
    /// expire; the loop stops and the next tick resumes at-least-once.
    pub async fn run_due_lead_requests(&self) -> EventResult<LeadRunSummary> {
        let mut summary = LeadRunSummary::default();
        let mut parked: Vec<Uuid> = Vec::new();
        loop {
            if summary.requests_claimed >= self.cron_limit {
                break;
            }
            let Some(request) = self
                .leads
                .claim_next_due_request(LEASE_SECONDS, &parked)
                .await?
            else {
                break;
            };
            summary.requests_claimed += 1;
            if self.run_one(request.event_id, &mut summary).await? {
                parked.push(request.event_id);
            }
        }
        Ok(summary)
    }

    /// Officer verb: run ONE event's queue row now (the admin "run"
    /// button). Serialized against the cron pass through the SAME
    /// lease — a fresh lease held by another walker is the typed
    /// `LeadRequestBusy` refusal, an absent row is
    /// `LeadRequestNotFound` (the verb never runs unserialized).
    pub async fn run_request_of_event(&self, event_id: Uuid) -> EventResult<LeadRunSummary> {
        let mut summary = LeadRunSummary::default();
        let request = self
            .leads
            .try_lease_request_of_event(event_id, LEASE_SECONDS)
            .await?;
        summary.requests_claimed = 1;
        let _parked = self.run_one(request.event_id, &mut summary).await?;
        Ok(summary)
    }

    /// Officer read: the event's queue row (claimed, parked, done).
    pub async fn request_of_event(
        &self,
        event_id: Uuid,
    ) -> EventResult<Option<crate::infrastructure::persistence::lead_command_repository::LeadRequestRow>>
    {
        self.leads.find_request_of_event(event_id).await
    }

    /// Walk ONE event's request. Returns whether the walk PARKED (the
    /// sink refused; the caller's pass excludes the event from
    /// re-claiming — the next pass retries).
    async fn run_one(&self, event_id: Uuid, summary: &mut LeadRunSummary) -> EventResult<bool> {
        let rules = self.leads.active_rules_of_event(event_id).await?;
        let mut park_reason: Option<String> = None;

        for rule in &rules {
            summary.rules_evaluated += 1;
            let predicates = self.leads.predicates_of_rule(rule.id).await?;
            // Walk in capped batches until the rule's domain drains.
            loop {
                let eligible = self
                    .leads
                    .eligible_registrations(event_id, rule, &predicates, self.batch)
                    .await?;
                if eligible.is_empty() {
                    break;
                }
                summary.registrations_matched += eligible.len() as i64;
                let refusal = self
                    .generate_groups(rule.id, event_id, &eligible, summary)
                    .await?;
                if let Some(reason) = refusal {
                    // First refusal parks the request with the reason;
                    // the pass still finishes this batch's completed
                    // writes (each group commits on its own), then the
                    // rule loop stops — the next tick retries from the
                    // anti-join boundary.
                    park_reason = Some(reason);
                    break;
                }
                if (eligible.len() as i64) < self.batch {
                    break;
                }
            }
            if park_reason.is_some() {
                break;
            }
        }

        let parked = park_reason.is_some();
        match park_reason {
            Some(reason) => {
                self.leads.finish_request_by_event(event_id, Some(&reason)).await?;
                summary.requests_parked += 1;
            }
            None => {
                self.leads.finish_request_by_event(event_id, None).await?;
                summary.requests_completed += 1;
            }
        }
        Ok(parked)
    }

    /// Group the batch by the ECS-1 strategy and drive the sink per
    /// group. Returns the first sink refusal (if any) so the caller can
    /// park the request — writes that already landed stay (idempotent
    /// on the retry through the provenance/junction uniques).
    async fn generate_groups(
        &self,
        rule_id: Uuid,
        event_id: Uuid,
        eligible: &[EligibleRegistration],
        summary: &mut LeadRunSummary,
    ) -> Result<Option<String>, EventError> {
        // (grouping, group_key) -> members, in walk order.
        let mut groups: BTreeMap<(String, String), Vec<&EligibleRegistration>> = BTreeMap::new();
        for r in eligible {
            match r.sale_order_id {
                Some(order_id) => {
                    groups
                        .entry((GROUPING_PER_ORDER.into(), order_id.to_string()))
                        .or_default()
                        .push(r);
                }
                None => {
                    let day = r
                        .created_at
                        .map(|ts| ts.date_naive().to_string())
                        .unwrap_or_else(|| "unknown-day".to_string());
                    groups
                        .entry((
                            GROUPING_PER_EVENT_DAY.into(),
                            format!("{event_id}:{day}"),
                        ))
                        .or_default()
                        .push(r);
                }
            }
        }

        for ((grouping, group_key), members) in groups {
            let (provenance_id, existing_lead) = self
                .leads
                .upsert_provenance(rule_id, event_id, &group_key, &grouping)
                .await?;

            // The sink ALWAYS sees the FULL group membership: the
            // members already junctioned (a prior batch's rows, or a
            // retry after a partial pass) plus this walk's new ones.
            let mut views: Vec<LeadRegistrationView> = self
                .leads
                .group_members(provenance_id)
                .await?
                .iter()
                .map(view_of)
                .collect();
            let known: std::collections::BTreeSet<Uuid> =
                views.iter().map(|v| v.id).collect();
            let mut new_ids = Vec::new();
            for m in &members {
                if !known.contains(&m.id) {
                    new_ids.push(m.id);
                    views.push(view_of(m));
                }
            }
            let group = LeadGroup {
                rule_id,
                event_id,
                group_key: group_key.clone(),
                grouping: grouping.clone(),
                registrations: views,
            };

            // THE SINK BEFORE THE JUNCTION: a refusal leaves the new
            // members UN-junctioned, so the anti-join hands them back
            // on the retry and the sink call re-fires — a parked group
            // is retried, never orphaned. `lead_id IS NULL` is the
            // pending-create marker (a create that never completed
            // re-creates on the next pass).
            match existing_lead {
                Some(lead_id) => {
                    if !new_ids.is_empty() {
                        if let Err(reason) = self.sink.update_lead(lead_id, &group).await {
                            return Ok(Some(reason));
                        }
                        self.leads.junction_add(provenance_id, &new_ids).await?;
                        summary.groups_grown += 1;
                        summary.leads_updated += 1;
                    }
                    // No new members: the re-walk is a no-op.
                }
                None => {
                    let lead_id = match self.sink.create_lead(&group).await {
                        Ok(id) => id,
                        Err(reason) => return Ok(Some(reason)),
                    };
                    self.leads.set_provenance_lead(provenance_id, lead_id).await?;
                    self.leads.junction_add(provenance_id, &new_ids).await?;
                    summary.groups_created += 1;
                    summary.leads_created += 1;
                }
            }
        }
        Ok(None)
    }
}

/// One member row in the sink's payload shape.
fn view_of(m: &EligibleRegistration) -> LeadRegistrationView {
    LeadRegistrationView {
        id: m.id,
        name: m.name.clone(),
        email: m.email.clone(),
        phone: m.phone.clone(),
        company_name: m.company_name.clone(),
        partner_id: m.partner_id,
        sale_order_id: m.sale_order_id,
    }
}
