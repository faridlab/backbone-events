//! The template seam ports (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! TWO host-installed ports, each with a REFUSING default so an
//! unwired host is a TYPED failure — never a silent skip, never a
//! registration blocker:
//!
//! - [`EventTemplateRenderer`]: turns a scheduler row's template ref
//!   + a registration's render context into the outbound mail body.
//!   The refusing default fails `template_renderer_not_composed` (the
//!   seam is host policy; this module owns no template engine).
//!
//! - [`EventMailQueue`]: the enqueue substrate (backbone-mail's queue
//!   behind the host adapter). `'sent' = queued`: a successful enqueue
//!   marks the receipt mailed from this module's point of view —
//!   delivery is the transport's concern. The refusing default fails
//!   `enqueue_refused`, which the scheduler RECORDS and retries next
//!   pass (at-least-once, idempotent consumers: the receipt UNIQUE +
//!   mail_sent flags).
//!
//! Both ports are process-local traits (not HTTP): the host composes
//! them into the scheduler service at mount time.

use async_trait::async_trait;
use uuid::Uuid;

use super::event_error::EventError;

/// What a scheduler render needs (the registration-side arms of the
/// upstream template context, flattened).
#[derive(Debug, Clone)]
pub struct RenderContext {
    pub event_id: Uuid,
    pub event_name: String,
    pub event_date_begin: chrono::DateTime<chrono::Utc>,
    pub event_date_end: chrono::DateTime<chrono::Utc>,
    pub event_date_tz: String,
    pub registration_id: Uuid,
    pub attendee_name: String,
    pub attendee_email: String,
    pub registration_barcode: String,
}

/// The rendered outbound body.
#[derive(Debug, Clone)]
pub struct RenderedMail {
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
}

/// Why a render refused — the typed family the scheduler records.
#[derive(Debug, Clone)]
pub enum RenderFailure {
    /// The scheduler row carries no template ref.
    TemplateUnresolved,
    /// No host renderer was installed for this module.
    RendererNotComposed,
    /// The renderer ran and failed.
    RenderFailed(String),
}

/// The template seam. The refusing default is the safe hostless
/// posture.
#[async_trait]
pub trait EventTemplateRenderer: Send + Sync {
    async fn render(
        &self,
        template_ref: Option<Uuid>,
        template_kind: Option<&str>,
        ctx: &RenderContext,
    ) -> Result<RenderedMail, RenderFailure>;
}

/// The refusing default: an unwired host is a typed failure, not a
/// silent skip.
pub struct RefusingTemplateRenderer;

#[async_trait]
impl EventTemplateRenderer for RefusingTemplateRenderer {
    async fn render(
        &self,
        template_ref: Option<Uuid>,
        _template_kind: Option<&str>,
        _ctx: &RenderContext,
    ) -> Result<RenderedMail, RenderFailure> {
        if template_ref.is_none() {
            return Err(RenderFailure::TemplateUnresolved);
        }
        Err(RenderFailure::RendererNotComposed)
    }
}

impl RenderFailure {
    /// Map to the typed scheduler-failure error.
    pub fn into_error(self) -> EventError {
        match self {
            Self::TemplateUnresolved => EventError::TemplateUnresolved,
            Self::RendererNotComposed => EventError::TemplateRendererNotComposed,
            Self::RenderFailed(d) => EventError::RenderFailed(d),
        }
    }
}

/// The enqueue seam. A successful enqueue is `'sent' = queued`.
#[async_trait]
pub trait EventMailQueue: Send + Sync {
    async fn enqueue(&self, recipient: &str, mail: &RenderedMail) -> Result<(), EventError>;
}

/// The refusing default: recorded `enqueue_refused`, retried next
/// pass.
pub struct RefusingMailQueue;

#[async_trait]
impl EventMailQueue for RefusingMailQueue {
    async fn enqueue(&self, _recipient: &str, _mail: &RenderedMail) -> Result<(), EventError> {
        Err(EventError::EnqueueRefused(
            "no host mail queue composed for the event module".to_string(),
        ))
    }
}
