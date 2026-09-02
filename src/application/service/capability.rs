//! Tier A capability tokens (hand-written; user-owned; see
//! `metaphor.codegen.yaml`) — ADR-0018.
//!
//! HMAC-SHA256 over a domain-separated message, base64url-encoded,
//! verified in CONSTANT TIME. The one secret is
//! `EVENT_CAPABILITY_SECRET`; an empty secret is a typed 503 at the
//! routes, never a mint under an empty key (fail-closed — a token
//! minted under "" would be forgeable by anyone with the source).
//!
//! Token shape: `v1.<payload-b64url>.<sig-b64url>` where the payload
//! is compact JSON `{purpose, exp, data}` and the signature is
//! HMAC-SHA256(secret, "event-capability-v1\n" + purpose + "\n" +
//! payload-b64url). Verification recomputes the signature and
//! compares with `subtle` (`ConstantTimeEq`) — never `==`.
//!
//! Expiry: the /ics purpose is `event-ics-access` WITH expiry +
//! rotation (short TTL, remintable); the my-tickets purpose is
//! `event-registration-ticket-report-access` with `exp = 0` — the
//! NEVER-EXPIRING documented deviation (an attendee's ticket link
//! must survive any rotation cadence; the payload pins the exact
//! registration set, so an old token grants nothing beyond the
//! attendee's own already-issued rows).
//!
//! Fail-closed on EVERY malformed arm: a token that fails to parse,
//! carries the wrong purpose, the wrong version, a signature
//! mismatch, or an expired `exp` is the SAME refusal as a missing
//! event — callers map it onto the uniform gate 404 (no oracle).

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use super::event_error::EventError;

/// The /ics capability purpose (expiry + rotation).
pub const PURPOSE_ICS: &str = "event-ics-access";

/// The my-tickets capability purpose (never-expiring deviation).
pub const PURPOSE_TICKET_REPORT: &str = "event-registration-ticket-report-access";

/// The domain-separation label (first arm of every MAC message).
const CAPABILITY_CONTEXT: &str = "event-capability-v1";

type HmacSha256 = Hmac<Sha256>;

/// The env var holding the module's capability secret.
pub const EVENT_CAPABILITY_SECRET_ENV: &str = "EVENT_CAPABILITY_SECRET";

/// Default /ics token lifetime (rotation cadence): 24h.
pub const ICS_TOKEN_TTL_SECS: i64 = 24 * 60 * 60;

/// Read the capability secret from the environment (empty string when
/// unset — the routes turn that into the typed 503).
pub fn capability_secret_from_env() -> String {
    std::env::var(EVENT_CAPABILITY_SECRET_ENV).unwrap_or_default()
}

fn b64url_encode(bytes: &[u8]) -> String {
    // Standard base64 WITHOUT padding, URL-safe alphabet — the
    // path-segment-safe encoding (tokens ride in URL path segments).
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(ALPHA[(n >> 18) as usize & 63] as char);
        out.push(ALPHA[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHA[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHA[n as usize & 63] as char } else { '=' });
    }
    // Trim padding: url-safe no-pad form.
    while out.ends_with('=') {
        out.pop();
    }
    out
}

fn b64url_decode(text: &str) -> Option<Vec<u8>> {
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut bits: u32 = 0;
    let mut nbits: u32 = 0;
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    for ch in text.bytes() {
        let v = ALPHA.iter().position(|&a| a == ch)? as u32;
        bits = (bits << 6) | v;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push(((bits >> nbits) & 0xff) as u8);
        }
    }
    Some(out)
}

fn sign(secret: &str, purpose: &str, payload_b64: &str) -> Result<Vec<u8>, EventError> {
    if secret.is_empty() {
        return Err(EventError::EventCapabilitySecretNotConfigured);
    }
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| EventError::Internal(format!("capability secret rejected by HMAC: {e}")))?;
    mac.update(CAPABILITY_CONTEXT.as_bytes());
    mac.update(b"\n");
    mac.update(purpose.as_bytes());
    mac.update(b"\n");
    mac.update(payload_b64.as_bytes());
    Ok(mac.finalize().into_bytes().to_vec())
}

/// The token payload. `exp` is unix seconds; `0` = never (the
/// my-tickets deviation). `data` is the purpose-scoped scope (the
/// ics token pins one event id and an optional slot id; the
/// ticket-report token pins one event id plus the SORTED registration
/// id set).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityClaims {
    pub purpose: String,
    pub exp: i64,
    pub data: Vec<String>,
}

impl CapabilityClaims {
    /// Mint a token for these claims (fails closed on an empty
    /// secret).
    pub fn mint(&self, secret: &str) -> Result<String, EventError> {
        let payload = serde_json::to_vec(self)
            .map_err(|e| EventError::Internal(format!("capability payload encode: {e}")))?;
        let payload_b64 = b64url_encode(&payload);
        let sig = sign(secret, &self.purpose, &payload_b64)?;
        Ok(format!("v1.{payload_b64}.{}", b64url_encode(&sig)))
    }

    /// Verify a token against the expected purpose (constant-time
    /// signature compare; expiry checked AFTER the signature so a
    /// forged expiry is not distinguishable from a forged anything).
    /// `exp = 0` never expires.
    pub fn verify(
        secret: &str,
        expected_purpose: &str,
        token: &str,
        now: DateTime<Utc>,
    ) -> Result<Self, EventError> {
        if secret.is_empty() {
            return Err(EventError::EventCapabilitySecretNotConfigured);
        }
        let bad = || EventError::EventNotPublished;
        let mut parts = token.splitn(3, '.');
        let version = parts.next().unwrap_or_default();
        let payload_b64 = parts.next().unwrap_or_default();
        let sig_b64 = parts.next().unwrap_or_default();
        if version != "v1" || payload_b64.is_empty() || sig_b64.is_empty() {
            return Err(bad());
        }
        let expected_sig = sign(secret, expected_purpose, payload_b64)?;
        let given_sig = b64url_decode(sig_b64).ok_or_else(bad)?;
        // Constant-time compare (length included): never `==`.
        if expected_sig.len() != given_sig.len() || expected_sig.ct_eq(&given_sig).unwrap_u8() == 0
        {
            return Err(bad());
        }
        let payload = b64url_decode(payload_b64).ok_or_else(bad)?;
        let claims: Self =
            serde_json::from_slice(&payload).map_err(|_| bad())?;
        if claims.purpose != expected_purpose {
            return Err(bad());
        }
        if claims.exp != 0 && now.timestamp() >= claims.exp {
            return Err(bad());
        }
        Ok(claims)
    }
}

/// Mint an /ics capability: one event, optionally slot-scoped, TTL
/// from `now`.
pub fn mint_ics_capability(
    secret: &str,
    event_id: &uuid::Uuid,
    slot_id: Option<&uuid::Uuid>,
    now: DateTime<Utc>,
    ttl_secs: i64,
) -> Result<String, EventError> {
    let mut data = vec![event_id.to_string()];
    if let Some(slot) = slot_id {
        data.push(slot.to_string());
    }
    CapabilityClaims {
        purpose: PURPOSE_ICS.to_string(),
        exp: now.timestamp() + ttl_secs,
        data,
    }
    .mint(secret)
}

/// Mint a my-tickets capability: one event + the SORTED registration
/// id set, never-expiring (`exp = 0` — the documented deviation).
pub fn mint_ticket_report_capability(
    secret: &str,
    event_id: &uuid::Uuid,
    registration_ids: &[uuid::Uuid],
) -> Result<String, EventError> {
    let mut ids: Vec<String> = registration_ids.iter().map(|id| id.to_string()).collect();
    ids.sort();
    let mut data = vec![event_id.to_string()];
    data.append(&mut ids);
    CapabilityClaims {
        purpose: PURPOSE_TICKET_REPORT.to_string(),
        exp: 0,
        data,
    }
    .mint(secret)
}
