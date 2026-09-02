//! The guarded intake executor (hand-written; user-owned; see
//! `metaphor.codegen.yaml`) — the W8 funnel contract, staged now.
//!
//! `POST /api/v1/event/intake/{event_id_or_capability}` — the ONE
//! public-shaped entry that funnels into the ONE register verb. The
//! router is EXPORTED but the core mounts it NOWHERE: the host arms
//! it only when the W8 funnel lands (the module's public surface at
//! core is /ics + my-tickets, nothing else).
//!
//! The fence, in order:
//! 1. TYPED ALLOWLIST AT PARSE — `deny_unknown_fields`; admitted:
//!    attendee identity + optional slot/ticket ids + answers.
//!    NEVER admitted: `partner_id`, `state`, `sale_*`, `active`,
//!    `barcode`, `company_id` — an unknown key is a typed 422, not a
//!    silently dropped field.
//! 2. NO partner minting — intake creates registrations, not parties.
//! 3. FIXED-WINDOW THROTTLES, per-identity (email) AND per-IP —
//!    in-memory windows (single-host posture; the module gate, not a
//!    fleet gate).
//! 4. THE ONE REGISTER VERB (seat truth lives there, once).
//!
//! The caller-IP posture: direct socket address by default; the
//! RIGHTMOST forwarded hop ONLY under `EVENT_TRUSTED_PROXY`
//! (bool-tolerant, fail-closed — the HTTP layer resolves it).

use std::collections::HashMap;
use std::sync::Mutex;

use uuid::Uuid;

use super::event_error::{EventError, EventResult};
use crate::infrastructure::persistence::seat_repository::{RegisterCommand, RegistrationRow};

/// The env var declaring whether intake traffic arrives through a
/// trusted reverse proxy (rightmost forwarded hop is then the caller
/// address). Unset or any non-true value keeps the direct posture.
pub const EVENT_TRUSTED_PROXY_ENV: &str = "EVENT_TRUSTED_PROXY";

/// The typed allowlist payload — `deny_unknown_fields` is the fence:
/// anything outside this list is a typed 422, never a silent drop.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntakePayload {
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub company_name: Option<String>,
    #[serde(default)]
    pub event_slot_id: Option<Uuid>,
    #[serde(default)]
    pub event_ticket_id: Option<Uuid>,
    #[serde(default)]
    pub answers: Vec<IntakeAnswer>,
}

/// One answer arm (validated against the question tables by the W8
/// funnel; the core records structure only — the register verb does
/// not read them; answers land when the funnel arms).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeAnswer {
    pub question_id: Uuid,
    pub value_text: Option<String>,
    pub value_answer_id: Option<Uuid>,
}

/// In-memory fixed-window throttle (per key). Windows are wall-clock
/// buckets measured from the first hit inside the window.
#[derive(Debug, Default)]
pub struct FixedWindows {
    inner: Mutex<HashMap<String, (u64, u64)>>, // key -> (window_start_unix, count)
}

impl FixedWindows {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a hit; returns false when the key is over budget in the
    /// current window.
    pub fn allow(&self, key: &str, max: u64, window_secs: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let open = match guard.get_mut(key) {
            Some((start, count)) => {
                if now.saturating_sub(*start) < window_secs {
                    true
                } else {
                    // Stale window: reset in place.
                    *start = now;
                    *count = 0;
                    true
                }
            }
            None => {
                guard.insert(key.to_string(), (now, 0));
                true
            }
        };
        if !open {
            return false;
        }
        let Some((_, count)) = guard.get_mut(key) else {
            return false;
        };
        if *count >= max {
            return false;
        }
        *count += 1;
        true
    }
}

/// The intake throttle policy (defaults: 5 per identity / hour, 30
/// per IP / hour — deliberate headroom for group sign-ups behind one
/// NAT; the host can tighten at mount).
#[derive(Debug, Clone)]
pub struct IntakeThrottles {
    pub per_identity_max: u64,
    pub per_identity_window_secs: u64,
    pub per_ip_max: u64,
    pub per_ip_window_secs: u64,
}

impl Default for IntakeThrottles {
    fn default() -> Self {
        Self {
            per_identity_max: 5,
            per_identity_window_secs: 3600,
            per_ip_max: 30,
            per_ip_window_secs: 3600,
        }
    }
}

/// The intake executor: throttles + the ONE register verb.
pub struct IntakeService {
    registrations: super::registration_service::RegistrationCommandService,
    windows: FixedWindows,
    throttles: IntakeThrottles,
}

impl IntakeService {
    pub fn new(
        registrations: super::registration_service::RegistrationCommandService,
        throttles: IntakeThrottles,
    ) -> Self {
        Self {
            registrations,
            windows: FixedWindows::new(),
            throttles,
        }
    }

    /// The guarded intake: throttle (identity AND ip), then the ONE
    /// register verb. `client_ip` is the RESOLVED caller address
    /// (direct socket by default; rightmost forwarded hop only under
    /// the trusted-proxy posture — resolved by the HTTP layer).
    pub async fn intake(
        &self,
        event_id: Uuid,
        payload: IntakePayload,
        client_ip: &str,
    ) -> EventResult<RegistrationRow> {
        // Throttle arms: per-identity AND per-IP fixed windows.
        if !self.windows.allow(
            &format!("identity:{}", payload.email.to_lowercase()),
            self.throttles.per_identity_max,
            self.throttles.per_identity_window_secs,
        ) || !self.windows.allow(
            &format!("ip:{client_ip}"),
            self.throttles.per_ip_max,
            self.throttles.per_ip_window_secs,
        ) {
            let retry = self
                .throttles
                .per_identity_window_secs
                .min(self.throttles.per_ip_window_secs)
                .max(1) as u32;
            return Err(EventError::EventThrottled {
                retry_after_secs: retry,
            });
        }

        // The ONE register verb (seat truth lives there — there is no
        // intake-side seat check to drift from it).
        self.registrations
            .register(RegisterCommand {
                event_id,
                event_slot_id: payload.event_slot_id,
                event_ticket_id: payload.event_ticket_id,
                name: payload.name,
                email: payload.email,
                phone: payload.phone,
                company_name: payload.company_name,
                partner_id: None, // intake NEVER mints or links a party
                actor: None,      // anonymous shape — no officer claim
            })
            .await
    }
}
