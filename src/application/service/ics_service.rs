//! The /ics body builder + publication gate (hand-written;
//! user-owned; see `metaphor.codegen.yaml`).
//!
//! `GET /api/v1/event/public/ics/{capability}` — a Tier A token
//! (ADR-0018) pins ONE event (optionally slot-scoped). The gate:
//! unpublished, cancelled, missing, bad-signature, expired, and
//! cross-event tokens ALL answer the ONE uniform typed 404
//! `event_not_published` — no oracle distinguishing members. The
//! body is a minimal RFC 5545 VCALENDAR (the calendar glue; the
//! webapp renders, the desk prints badges).
//!
//! Throttling (per-identity AND per-IP fixed windows) lives in the
//! HTTP layer; this service is the token verify + gate + body. All
//! SQL rides the repository (services hold no raw sqlx).

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::capability::{mint_ics_capability, CapabilityClaims, PURPOSE_ICS};
use super::event_error::{EventError, EventResult};
use crate::infrastructure::persistence::event_command_repository::{
    EventCommandRepository, EventRow,
};

/// The ics service.
pub struct IcsService {
    events: EventCommandRepository,
}

impl IcsService {
    pub fn new(events: EventCommandRepository) -> Self {
        Self { events }
    }

    /// Mint an /ics capability for one event (optionally
    /// slot-scoped), TTL-bounded (expiry + rotation).
    pub fn mint(
        &self,
        secret: &str,
        event_id: Uuid,
        slot_id: Option<Uuid>,
        ttl_secs: i64,
    ) -> EventResult<String> {
        mint_ics_capability(secret, &event_id, slot_id.as_ref(), Utc::now(), ttl_secs)
    }

    /// THE GATED READ: verify the token (constant-time, purpose
    /// pinned), then the slot scope, then the publication state, then
    /// build the body. The arms are ordered so every failure is the
    /// same uniform refusal — a forged token is indistinguishable
    /// from a missing event.
    pub async fn feed(&self, secret: &str, token: &str) -> EventResult<IcsFeed> {
        let claims = CapabilityClaims::verify(secret, PURPOSE_ICS, token, Utc::now())?;
        let event_id = claims
            .data
            .first()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or(EventError::EventNotPublished)?;

        // Slot scope: the token's second arm must belong to the event
        // (else the uniform refusal — cross-scope probing yields
        // nothing).
        let slot_window = match claims.data.get(1) {
            Some(raw) => {
                let slot = Uuid::parse_str(raw)
                    .ok()
                    .ok_or(EventError::EventNotPublished)?;
                Some(
                    self.events
                        .slot_window_of_event(slot, event_id)
                        .await?
                        .ok_or(EventError::EventNotPublished)?,
                )
            }
            None => None,
        };

        // The publication gate (unpublished / cancelled / missing —
        // the same uniform refusal).
        let event = self.events.find_published(event_id).await?;

        Ok(IcsFeed {
            event_id,
            slot_id: slot_window.map(|(slot_id, _, _)| slot_id),
            body: build_ics(&event, slot_window),
        })
    }
}

/// The gated feed payload (the HTTP layer serves `body` as
/// text/calendar).
#[derive(Debug, Clone)]
pub struct IcsFeed {
    pub event_id: Uuid,
    pub slot_id: Option<Uuid>,
    pub body: String,
}

/// Escape one TEXT property value per RFC 5545 (backslash, semicolon,
/// comma, newline).
fn ics_escape(raw: &str) -> String {
    raw.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

/// Fold long lines at 75 octets per RFC 5545 (space-indent
/// continuation).
fn ics_fold(line: &str) -> String {
    if line.len() <= 75 {
        return line.to_string();
    }
    let mut out = String::new();
    let mut count = 0;
    for ch in line.chars() {
        if count >= 74 {
            out.push_str("\r\n ");
            count = 0;
        }
        out.push(ch);
        count += 1;
    }
    out
}

fn ics_stamp(t: DateTime<Utc>) -> String {
    t.format("%Y%m%dT%H%M%SZ").to_string()
}

/// Build the minimal VCALENDAR: one VEVENT for the event's window
/// (narrowed to the slot's hours when the token is slot-scoped).
pub fn build_ics(event: &EventRow, slot_window: Option<(Uuid, DateTime<Utc>, DateTime<Utc>)>) -> String {
    let (begin, end) = match slot_window {
        Some((_, slot_begin, slot_end)) => (slot_begin, slot_end),
        None => (event.date_begin, event.date_end),
    };
    let slot_id = slot_window.map(|(id, _, _)| id);
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//backbone-events//core//EN".to_string(),
        "CALSCALE:GREGORIAN".to_string(),
        "BEGIN:VEVENT".to_string(),
        format!(
            "UID:{}",
            match slot_id {
                Some(slot) => format!("event-{}-slot-{}@event.module", event.id, slot),
                None => format!("event-{}@event.module", event.id),
            }
        ),
        format!("DTSTAMP:{}", ics_stamp(Utc::now())),
        format!("DTSTART:{}", ics_stamp(begin)),
        format!("DTEND:{}", ics_stamp(end)),
        format!("SUMMARY:{}", ics_escape(&event.name)),
        "END:VEVENT".to_string(),
        "END:VCALENDAR".to_string(),
    ];
    lines.iter().map(|l| ics_fold(l)).collect::<Vec<_>>().join("\r\n")
}
