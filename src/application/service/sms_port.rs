//! The sms port (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! The SMS arm of the scheduler (ESM-1/ESM-2): a scheduler row whose
//! derived channel is `sms` renders through the SAME host-installed
//! `EventTemplateRenderer` port (the renderer receives the row's
//! template_kind and returns the body; the sms arm reads `body_text`)
//! and enqueues through THIS port with the registration's PHONE as
//! the recipient.
//!
//! The refusing default is the DECLARED posture of an unconfigured
//! gateway: every enqueue refuses, the scheduler records the typed
//! `sms_enqueue_refused` failure, the receipt stays unsent, and the
//! row parks LOUDLY — parked is never silently queued, and never a
//! registration blocker (ESM-2). The module owns no SMS transport;
//! the host composes mail's gateway-sms-http adapter (or any other)
//! behind this trait at mount time.
//!
//! Process-local trait (not HTTP) — same composition discipline as
//! EventMailQueue.

use async_trait::async_trait;

/// The rendered outbound sms body. The renderer port returns its mail
/// shape; the sms arm reads `body_text` from it, so this view is just
/// that arm — kept as its own type so the queue port speaks sms, not
/// half a mail.
#[derive(Debug, Clone)]
pub struct RenderedSms {
    pub body_text: String,
}

/// The sms enqueue seam. A successful enqueue is `'sent' = queued`
/// (the same contract as the mail arm — EBB-3 holds on both arms).
#[async_trait]
pub trait EventSmsQueue: Send + Sync {
    async fn enqueue(&self, phone: &str, sms: &RenderedSms) -> Result<(), String>;
}

/// The refusing default: an unconfigured gateway is a typed, LOUD
/// failure — `sms_enqueue_refused` recorded on the scheduler row, the
/// receipt retried on the next pass, never silently queued.
pub struct RefusingSmsQueue;

#[async_trait]
impl EventSmsQueue for RefusingSmsQueue {
    async fn enqueue(&self, _phone: &str, _sms: &RenderedSms) -> Result<(), String> {
        Err("no host sms queue composed for the event module (gateway unconfigured)".to_string())
    }
}
