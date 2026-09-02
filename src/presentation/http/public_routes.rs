//! The module's PUBLIC route surface (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! The module DOES NOT SELF-MOUNT: it exports
//! [`event_public_routes`], a plain `axum::Router` the composing host
//! nests BARE of `company_auth` under the schema name —
//! `Router::new().nest("/api/v1/event", event_public_routes(state))`.
//! The capability token + the fixed-window throttle are the fence;
//! there is no session, no company scope, no auth middleware here.
//!
//! The allowlist (exhaustive — the negative-enumeration probe's
//! target):
//! - `GET  /public/ics/{capability}`         the calendar feed
//! - `GET  /public/my-tickets/{capability}`  the attendee ticket report
//!
//! The intake verb's router ([`event_intake_routes`]) is EXPORTED
//! but NOT part of the public tree: the host mounts it only when the
//! funnel arm lands (nothing else answers unauthenticated).
//!
//! The secret: `EVENT_CAPABILITY_SECRET` at compose; unset = BOTH
//! capability routes answer the typed 503
//! `event_capability_secret_not_configured` (fail-closed — the boot
//! WARN is the host's; the secret is never printed).

use std::sync::Arc;

use axum::{
    extract::{ConnectInfo, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};

use crate::application::service::capability::capability_secret_from_env;
use crate::application::service::event_error::EventError;
use crate::application::service::intake_service::{
    FixedWindows, IntakePayload, IntakeService, IntakeThrottles, EVENT_TRUSTED_PROXY_ENV,
};
use crate::application::service::ics_service::IcsService;
use crate::application::service::my_tickets_service::MyTicketsService;
use crate::application::service::registration_service::RegistrationCommandService;
use crate::application::service::template_port::{EventMailQueue, EventTemplateRenderer};
use crate::infrastructure::persistence::event_command_repository::EventCommandRepository;
use crate::infrastructure::persistence::seat_repository::SeatRepository;

/// /ics throttle defaults (per-identity AND per-IP fixed windows):
/// the capability IS the identity arm for anonymous feeds.
pub const ICS_THROTTLE: (u64, u64) = (30, 60);

/// The shared public state (cheap-to-clone service handles).
#[derive(Clone)]
pub struct EventPublicState {
    secret: String,
    trusted_proxy: bool,
    ics: Arc<IcsService>,
    tickets: Arc<MyTicketsService>,
    intake: Arc<IntakeService>,
    windows: Arc<FixedWindows>,
}

impl EventPublicState {
    /// Compose over one pool + the host ports; the secret comes from
    /// [`EVENT_CAPABILITY_SECRET_ENV`] (empty = the typed 503 at the
    /// routes) and the trusted-proxy posture from
    /// [`EVENT_TRUSTED_PROXY_ENV`] (unset = direct connections — the
    /// forwarded header is client-controlled text and never read).
    pub fn compose(
        pool: sqlx::PgPool,
        renderer: Arc<dyn EventTemplateRenderer>,
        queue: Arc<dyn EventMailQueue>,
    ) -> Self {
        Self::with_secret_and_trusted_proxy(
            pool,
            renderer,
            queue,
            capability_secret_from_env(),
            trusted_proxy_from_env(),
        )
    }

    /// [`Self::compose`] with the secret explicit (the probe entry —
    /// tests must not depend on process environment other tests
    /// mutate) under the direct-connection posture.
    pub fn with_secret(
        pool: sqlx::PgPool,
        renderer: Arc<dyn EventTemplateRenderer>,
        queue: Arc<dyn EventMailQueue>,
        secret: String,
    ) -> Self {
        Self::with_secret_and_trusted_proxy(pool, renderer, queue, secret, false)
    }

    /// [`Self::with_secret`] with the trusted-proxy posture explicit
    /// (the rotating-XFF probe entry).
    pub fn with_secret_and_trusted_proxy(
        pool: sqlx::PgPool,
        renderer: Arc<dyn EventTemplateRenderer>,
        queue: Arc<dyn EventMailQueue>,
        secret: String,
        trusted_proxy: bool,
    ) -> Self {
        let events = EventCommandRepository::new(pool.clone());
        let seats = SeatRepository::new(pool.clone());
        let _ = (&renderer, &queue); // intake-side ports unused at core (see event_intake_routes)
        Self {
            secret,
            trusted_proxy,
            ics: Arc::new(IcsService::new(EventCommandRepository::new(pool.clone()))),
            tickets: Arc::new(MyTicketsService::new(events, seats)),
            intake: Arc::new(IntakeService::new(
                RegistrationCommandService::new(SeatRepository::new(pool)),
                IntakeThrottles::default(),
            )),
            windows: Arc::new(FixedWindows::new()),
        }
    }

    /// The compose secret is deliberately not readable (never
    /// printed, never logged); this answers ONLY whether it is set.
    pub fn secret_is_configured(&self) -> bool {
        !self.secret.is_empty()
    }
}

/// THE PUBLIC TREE (the exhaustive allowlist — see the module doc).
pub fn event_public_routes(state: EventPublicState) -> Router {
    Router::new()
        .route("/public/ics/:capability", get(ics_handler))
        .route("/public/my-tickets/:capability", get(my_tickets_handler))
        .with_state(state)
}

/// The intake router — EXPORTED, mounted by the host only when the
/// funnel arms (never part of [`event_public_routes`]).
pub fn event_intake_routes(state: EventPublicState) -> Router {
    Router::new()
        .route("/intake/:event_id", post(intake_handler))
        .with_state(state)
}

/// The trusted-proxy posture (bool-tolerant, fail-closed): `true` /
/// `1` / `yes` / `on` (any case) arm it; unset or anything else
/// keeps the direct-connection posture.
pub fn trusted_proxy_from_env() -> bool {
    matches!(
        std::env::var(EVENT_TRUSTED_PROXY_ENV)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Resolve the caller address for rate shaping and intake identity
/// (never authorization): the RIGHTMOST forwarded hop ONLY under the
/// trusted-proxy posture (the entry the nearest trusted proxy
/// appended; every hop to its left is client-supplied text); every
/// hop is ignored otherwise and the connection's socket IP wins.
/// Falls back to the socket IP when a trusted chain emits no header,
/// and to `"unknown"` when no socket address is available. The socket
/// arm is the bare IP, never the `ip:port` pair — the port is
/// per-connection and would fragment a throttle bucket per reconnect.
pub fn caller_ip(headers: &HeaderMap, socket_ip: Option<&str>, trusted_proxy: bool) -> String {
    if trusted_proxy {
        if let Some(fwd) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(last) = fwd.rsplit(',').next() {
                let trimmed = last.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    socket_ip.unwrap_or("unknown").to_string()
}

async fn ics_handler(
    State(state): State<EventPublicState>,
    Path(capability): Path<String>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<std::net::SocketAddr>>,
) -> Response {
    // Fail-closed secret.
    if !state.secret_is_configured() {
        return EventError::EventCapabilitySecretNotConfigured.into_response();
    }
    // Throttle: per-token (the identity arm for anonymous feeds) AND
    // per-IP fixed windows.
    let ip = caller_ip(
        &headers,
        connect_info.map(|c| c.ip().to_string()).as_deref(),
        state.trusted_proxy,
    );
    let (max, window) = ICS_THROTTLE;
    if !state.windows.allow(&format!("ics-token:{capability}"), max, window)
        || !state.windows.allow(&format!("ics-ip:{ip}"), max, window)
    {
        return EventError::EventThrottled {
            retry_after_secs: window as u32,
        }
        .into_response();
    }
    match state.ics.feed(&state.secret, &capability).await {
        Ok(feed) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/calendar; charset=utf-8")],
            feed.body,
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

async fn my_tickets_handler(
    State(state): State<EventPublicState>,
    Path(capability): Path<String>,
) -> Response {
    if !state.secret_is_configured() {
        return EventError::EventCapabilitySecretNotConfigured.into_response();
    }
    match state.tickets.report(&state.secret, &capability).await {
        Ok(report) => (StatusCode::OK, Json(report)).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn intake_handler(
    State(state): State<EventPublicState>,
    Path(event_id): Path<uuid::Uuid>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<std::net::SocketAddr>>,
    Json(payload): Json<IntakePayload>,
) -> Response {
    let ip = caller_ip(
        &headers,
        connect_info.map(|c| c.ip().to_string()).as_deref(),
        state.trusted_proxy,
    );
    match state.intake.intake(event_id, payload, &ip).await {
        Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
        Err(e) => e.into_response(),
    }
}
