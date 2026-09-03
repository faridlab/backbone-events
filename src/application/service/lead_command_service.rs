//! The lead rule command service (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! THE CLOSED VOCABULARY LIVES HERE (ECR-4): every predicate enters as
//! a typed input struct whose `axis` must be one of four strings; the
//! question_answer axis REQUIRES its question ref and nothing else
//! carries one; every value must parse as a uuid. The repository only
//! ever sees validated rows — no stored domain exists anywhere for
//! anything to eval.
//!
//! The input structs are `deny_unknown_fields`: a shape drift from the
//! host is a typed parse refusal, never a silently-ignored field.

use serde::Deserialize;
use uuid::Uuid;

use super::event_error::{EventError, EventResult};
use crate::infrastructure::persistence::lead_command_repository::{
    LeadCommandRepository, LeadPredicateRow, LeadRuleRow, PredicateInput,
};

/// The closed predicate vocabulary (ECR-4 — the whole list).
pub const PREDICATE_AXES: [&str; 4] = ["event", "event_type", "company", "question_answer"];

/// One typed predicate as it crosses the wire.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredicateSpec {
    /// One of: event | event_type | company | question_answer.
    pub axis: String,
    /// REQUIRED on question_answer; refused on every other axis.
    pub question_id: Option<Uuid>,
    /// The matched ids of the axis's own table (event ids, type ids,
    /// company ids, or the question's answer ids).
    pub value_ids: Vec<Uuid>,
}

/// The create-rule input.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateRuleInput {
    pub name: String,
    pub event_id: Option<Uuid>,
    /// registration | attendee — attendee reuses the registration rows
    /// (one lead per registration); the column survives for the read
    /// model, the engine treats both alike at this pin.
    pub basis: Option<String>,
    pub on_create: Option<bool>,
    pub on_confirm: Option<bool>,
    pub on_done: Option<bool>,
    pub predicates: Vec<PredicateSpec>,
}

/// The patch-rule input (name / scope / active — triggers and
/// predicates are immutable; edit by deactivate + recreate, which the
/// provenance history records as separate rules).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchRuleInput {
    pub name: Option<String>,
    pub event_id: Option<Uuid>,
    pub active: Option<bool>,
}

/// The answer-to-rule bridge input (the typed replacement for
/// upstream's stored-domain editor button).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FromAnswerInput {
    pub name: String,
    pub event_id: Option<Uuid>,
    pub on_create: Option<bool>,
    pub on_confirm: Option<bool>,
    pub on_done: Option<bool>,
    pub question_id: Uuid,
    pub answer_id: Uuid,
}

/// The merge-relink input (the host's lead side calls this after a
/// merge; events rows follow in one update).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelinkInput {
    pub old_lead_id: Uuid,
    pub new_lead_id: Uuid,
}

/// The lead rule command service (the hand verb surface — distinct
/// from the generated read-only CRUD alias of the same model).
pub struct LeadRuleCommandService {
    repo: LeadCommandRepository,
}

impl LeadRuleCommandService {
    pub fn new(repo: LeadCommandRepository) -> Self {
        Self { repo }
    }

    /// Validate the closed vocabulary + the axis shape + the trigger
    /// guard, then persist. Creating a rule ARMS the queue for its
    /// whole scope (the repository does that in the same transaction).
    pub async fn create_rule(&self, input: CreateRuleInput, actor: Option<Uuid>) -> EventResult<LeadRuleRow> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(EventError::Validation("rule name must not be empty".into()));
        }
        if input.predicates.is_empty() {
            return Err(EventError::Validation(
                "a rule carries at least one predicate — an unconstrained rule is refused".into(),
            ));
        }
        let on_create = input.on_create.unwrap_or(false);
        let on_confirm = input.on_confirm.unwrap_or(false);
        let on_done = input.on_done.unwrap_or(false);
        if !(on_create || on_confirm || on_done) {
            return Err(EventError::Validation(
                "at least one trigger (on_create / on_confirm / on_done) must be armed".into(),
            ));
        }
        let basis = input.basis.unwrap_or_else(|| "registration".to_string());
        if basis != "registration" && basis != "attendee" {
            return Err(EventError::Validation(format!(
                "basis must be registration or attendee (got {basis})"
            )));
        }
        let predicates = validate_predicates(&input.predicates)?;
        self.repo
            .create_rule(name, input.event_id, &basis, on_create, on_confirm, on_done, &predicates, actor)
            .await
    }

    pub async fn patch_rule(&self, rule_id: Uuid, input: PatchRuleInput, actor: Option<Uuid>) -> EventResult<LeadRuleRow> {
        if let Some(name) = input.name.as_deref() {
            if name.trim().is_empty() {
                return Err(EventError::Validation("rule name must not be empty".into()));
            }
        }
        self.repo
            .patch_rule(rule_id, input.name.as_deref().map(str::trim), input.event_id, input.active, actor)
            .await
    }

    /// The answer-to-rule bridge (ECR-4): one rule, one question_answer
    /// predicate — typed bytes where upstream eval'd a stored domain.
    pub async fn from_answer(&self, input: FromAnswerInput, actor: Option<Uuid>) -> EventResult<LeadRuleRow> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(EventError::Validation("rule name must not be empty".into()));
        }
        let on_create = input.on_create.unwrap_or(false);
        let on_confirm = input.on_confirm.unwrap_or(false);
        let on_done = input.on_done.unwrap_or(false);
        if !(on_create || on_confirm || on_done) {
            return Err(EventError::Validation(
                "at least one trigger (on_create / on_confirm / on_done) must be armed".into(),
            ));
        }
        self.repo
            .bridge_answer_to_rule(name, input.event_id, on_create, on_confirm, on_done, input.question_id, input.answer_id, actor)
            .await
    }

    /// The rule read model (ECS-2): rule + predicates + both grouping
    /// counters side by side.
    pub async fn rule_read_model(&self, rule_id: Uuid) -> EventResult<serde_json::Value> {
        self.repo.rule_read_model(rule_id).await
    }

    pub async fn find_rule(&self, rule_id: Uuid) -> EventResult<LeadRuleRow> {
        self.repo.find_rule(rule_id).await
    }

    pub async fn list_rules(&self, limit: i64) -> EventResult<Vec<LeadRuleRow>> {
        self.repo.list_rules(limit).await
    }

    pub async fn predicates_of_rule(&self, rule_id: Uuid) -> EventResult<Vec<LeadPredicateRow>> {
        self.repo.predicates_of_rule(rule_id).await
    }

    /// The merge-relink verb (declared seam — the host's lead module
    /// calls it after a merge; no lead schema is touched from here).
    pub async fn relink_lead(&self, input: RelinkInput, actor: Option<Uuid>) -> EventResult<usize> {
        if input.old_lead_id == input.new_lead_id {
            return Ok(0);
        }
        self.repo.relink_lead(input.old_lead_id, input.new_lead_id, actor).await
    }
}

/// The closed-vocabulary gate: axis membership, the axis/question
/// shape, and non-empty value lists. What passes here is the ONLY
/// predicate shape the engine can ever compose SQL from.
fn validate_predicates(specs: &[PredicateSpec]) -> Result<Vec<PredicateInput>, EventError> {
    let mut out = Vec::with_capacity(specs.len());
    for spec in specs {
        if !PREDICATE_AXES.contains(&spec.axis.as_str()) {
            return Err(EventError::Validation(format!(
                "predicate axis '{}' is outside the closed vocabulary {PREDICATE_AXES:?}",
                spec.axis
            )));
        }
        if spec.value_ids.is_empty() {
            return Err(EventError::Validation(format!(
                "predicate axis '{}' carries no values",
                spec.axis
            )));
        }
        match spec.axis.as_str() {
            "question_answer" => {
                let question_id = spec.question_id.ok_or_else(|| {
                    EventError::Validation(
                        "the question_answer axis requires question_id (refused)".into(),
                    )
                })?;
                out.push(PredicateInput {
                    axis: spec.axis.clone(),
                    question_id: Some(question_id),
                    value_ids: spec.value_ids.clone(),
                });
            }
            axis => {
                if spec.question_id.is_some() {
                    return Err(EventError::Validation(format!(
                        "the {axis} axis carries no question_id (refused)"
                    )));
                }
                out.push(PredicateInput {
                    axis: spec.axis.clone(),
                    question_id: None,
                    value_ids: spec.value_ids.clone(),
                });
            }
        }
    }
    Ok(out)
}
