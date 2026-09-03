//! The lead sink port (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! Lead creation goes through a HOST-INSTALLED process-local port with
//! a REFUSING default — an unwired host gets a typed refusal that
//! parks the generation request loudly (error_detail, retried), never
//! a silent skip. This is the module's established port discipline
//! (EventTemplateRenderer / EventMailQueue / EventSmsQueue).
//!
//! Upstream's SIX sudo call sites into crm.lead collapse into this ONE
//! declared seam. The host adapter owns the mapping (docs/spec-overlay.md
//! §C carries the table); the module never touches the lead module's
//! schema and backbone-lead gains no events edge at this pin.

use async_trait::async_trait;
use uuid::Uuid;

/// One registration as the sink sees it (the provenance feed).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LeadRegistrationView {
    pub id: Uuid,
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub company_name: Option<String>,
    pub partner_id: Option<Uuid>,
    pub sale_order_id: Option<Uuid>,
}

/// One generated group (ECS-1's realized strategy — per_order for
/// sale-linked registrations, per_event_day for walk-ins; both
/// surfaced in the rule read model, ECS-2).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LeadGroup {
    pub rule_id: Uuid,
    pub event_id: Uuid,
    pub group_key: String,
    pub grouping: String,
    pub registrations: Vec<LeadRegistrationView>,
}

/// The sink port. `create_lead` returns the new logical lead id (the
/// provenance row stores it); `update_lead` folds newly-joined members
/// into an existing group's lead. Both refuse with a REASON (the
/// request parks with it).
#[async_trait]
pub trait EventLeadSink: Send + Sync {
    async fn create_lead(&self, group: &LeadGroup) -> Result<Uuid, String>;
    async fn update_lead(&self, lead_id: Uuid, group: &LeadGroup) -> Result<(), String>;
}

/// The refusing default: every call refuses, the request parks.
pub struct RefusingLeadSink;

#[async_trait]
impl EventLeadSink for RefusingLeadSink {
    async fn create_lead(&self, _group: &LeadGroup) -> Result<Uuid, String> {
        Err("host installed no EventLeadSink adapter — lead generation parks".to_string())
    }
    async fn update_lead(&self, _lead_id: Uuid, _group: &LeadGroup) -> Result<(), String> {
        Err("host installed no EventLeadSink adapter — lead generation parks".to_string())
    }
}
