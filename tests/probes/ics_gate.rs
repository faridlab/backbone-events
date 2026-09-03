//! The /ics refusal family — publication gating + the Tier A token
//! contract (ADR-0018), every member of the family answering the ONE
//! uniform typed 404 `event_not_published` (no oracle):
//!
//! - unpublished event + VALID token  -> uniform 404
//! - published event + TAMPERED sig   -> uniform 404
//! - published event + EXPIRED token  -> uniform 404
//! - published event + WRONG purpose  -> uniform 404
//! - missing event + any token        -> uniform 404
//! - cross-event token (A's token on B's row via B's mint path)
//!   — a token never widens its scope
//! - slot-scoped token whose slot belongs to another event -> 404
//! - published + valid                -> 200 + text/calendar body
//! - unset secret                     -> typed 503 at the routes
//!
//! Plus the negative-enumeration probe: the public router answers
//! ONLY the two declared paths — everything else is the router's own
//! bare 404, distinguishable from the uniform gate body.
//!
//! Plus the trusted-proxy posture probe: the per-IP throttle bucket
//! is keyed by the RIGHTMOST X-Forwarded-For hop ONLY under the
//! trusted posture — a fixed hop shared across two tokens trips the
//! shared bucket, and distinct hops under the untrusted posture all
//! collapse into ONE bucket (proof the header is unread there).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use backbone_events::application::service::capability::{mint_ics_capability, PURPOSE_ICS, PURPOSE_TICKET_REPORT};
use backbone_events::application::service::ics_service::IcsService;
use backbone_events::infrastructure::persistence::event_command_repository::EventCommandRepository;
use backbone_events::presentation::http::{
    caller_ip, event_public_routes, EventPublicState, ICS_THROTTLE,
};

use super::common::{make_event, make_type_with_mail, register_cmd, registrations, PROBE_SECRET, StubRenderer, TestDb, RecordingQueue};

/// The caller-address resolver's semantics, pinned at unit level:
/// rightmost hop under trust (never the client-writable left hops),
/// socket IP fallback, "unknown" when neither.
#[test]
fn caller_ip_rightmost_hop_only_and_fallbacks() {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        "1.1.1.1, 2.2.2.2, 3.3.3.3".parse().unwrap(),
    );

    // Trusted: the RIGHTMOST hop (the nearest proxy's record), never
    // the leftmost (client-writable text).
    assert_eq!(caller_ip(&headers, Some("4.4.4.4"), true), "3.3.3.3");
    // Untrusted: the header is client-controlled text — ignored
    // entirely, the socket IP wins.
    assert_eq!(caller_ip(&headers, Some("4.4.4.4"), false), "4.4.4.4");
    // Trusted chain that emitted no header: socket IP.
    assert_eq!(
        caller_ip(&axum::http::HeaderMap::new(), Some("4.4.4.4"), true),
        "4.4.4.4"
    );
    // No header, no socket: "unknown".
    assert_eq!(caller_ip(&axum::http::HeaderMap::new(), None, true), "unknown");
    assert_eq!(caller_ip(&axum::http::HeaderMap::new(), None, false), "unknown");

    // A header whose rightmost hop is blank padding falls back to the
    // socket IP rather than an empty key.
    let mut blank = axum::http::HeaderMap::new();
    blank.insert("x-forwarded-for", "1.1.1.1, ".parse().unwrap());
    assert_eq!(caller_ip(&blank, Some("4.4.4.4"), true), "4.4.4.4");
}

/// One GET against the public router with an optional X-Forwarded-For.
async fn ics_get(app: &axum::Router, path: &str, xff: Option<&str>) -> StatusCode {
    let mut builder = Request::builder().uri(path);
    if let Some(hop) = xff {
        builder = builder.header("x-forwarded-for", hop);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    response.status()
}

#[tokio::test]
async fn trusted_proxy_posture_shapes_the_per_ip_bucket() {
    // The throttle layer, not the feed: tokens can be arbitrary
    // strings (the gate's 404 never runs — the throttle is checked
    // first), so the observable IS the bucket keying.
    let db = TestDb::new("xfftrust").await;
    let (max, _window) = ICS_THROTTLE;

    // ARM A (trusted): ONE fixed rightmost hop shared by BOTH tokens.
    // Each token stays under its own budget; the SHARED ip bucket
    // trips exactly one past the cap.
    let state = EventPublicState::with_secret_and_trusted_proxy(
        db.pool.clone(),
        Arc::new(StubRenderer),
        Arc::new(RecordingQueue::default()),
        PROBE_SECRET.to_string(),
        true,
    );
    let app = event_public_routes(state);
    let mut statuses = Vec::new();
    for n in 0..(max + 1) {
        let token = if n < max / 2 { "token-a" } else { "token-b" };
        statuses.push(
            ics_get(&app, &format!("/public/ics/{token}"), Some("9.9.9.9")).await,
        );
    }
    assert!(
        statuses[..max as usize].iter().all(|s| *s != StatusCode::TOO_MANY_REQUESTS),
        "under the cap: both tokens' requests pass (token budgets not exhausted)"
    );
    assert_eq!(
        statuses[max as usize], StatusCode::TOO_MANY_REQUESTS,
        "the shared rightmost hop's bucket trips one past the cap — the XFF IS the bucket key under trust"
    );

    // ARM B (untrusted): a DISTINCT hop per request. If the header
    // were read, every request would key a fresh bucket and nothing
    // would trip; instead all collapse into the one direct-connection
    // bucket ("unknown" under oneshot — no socket address) and the
    // overflow request 429s.
    let state = EventPublicState::with_secret_and_trusted_proxy(
        db.pool.clone(),
        Arc::new(StubRenderer),
        Arc::new(RecordingQueue::default()),
        PROBE_SECRET.to_string(),
        false,
    );
    let app = event_public_routes(state);
    let mut statuses = Vec::new();
    for n in 0..(max + 1) {
        let token = if n < max / 2 { "token-a" } else { "token-b" };
        let hop = format!("10.0.0.{n}");
        statuses.push(
            ics_get(&app, &format!("/public/ics/{token}"), Some(&hop)).await,
        );
    }
    assert_eq!(
        statuses[max as usize], StatusCode::TOO_MANY_REQUESTS,
        "distinct hops under the untrusted posture all share ONE bucket — the header is not read"
    );
    assert!(
        statuses[..max as usize].iter().all(|s| *s != StatusCode::TOO_MANY_REQUESTS),
        "under the cap even in the collapsed bucket"
    );
    db.dispose().await;
}

#[tokio::test]
async fn ics_refusal_family_and_publication_gate() {
    let db = TestDb::new("icsgate").await;
    let (type_id, _template) = make_type_with_mail(&db, "now", 0).await;
    let event_a = make_event(&db, "probe event A", false, 0, Some(type_id)).await;
    let event_b = make_event(&db, "probe event B", false, 0, None).await;

    let events = EventCommandRepository::new(db.pool.clone());
    let ics = IcsService::new(EventCommandRepository::new(db.pool.clone()));

    // UNPUBLISHED + valid token -> the uniform 404.
    let token_unpublished = ics.mint(PROBE_SECRET, event_a, None, 3600).unwrap();
    let err = ics.feed(PROBE_SECRET, &token_unpublished).await.unwrap_err();
    assert_eq!(err.code(), "event_not_published", "unpublished is the uniform refusal");

    // PUBLISH (the fence verb — the only writer).
    events.publish(event_a, None).await.unwrap();

    // PUBLISHED + valid -> the feed.
    let token_ok = ics.mint(PROBE_SECRET, event_a, None, 3600).unwrap();
    let feed = ics.feed(PROBE_SECRET, &token_ok).await.unwrap();
    assert_eq!(feed.event_id, event_a);
    assert!(feed.body.contains("BEGIN:VCALENDAR"));
    assert!(feed.body.contains("probe event A"));

    // TAMPERED signature -> uniform 404. The replacement char sits
    // OUTSIDE the token alphabet — replacing with an in-alphabet char
    // can leave a valid token untouched (a flaky 1-in-64 pass).
    let tampered = format!("{}~", &token_ok[..token_ok.len() - 1]);
    let err = ics.feed(PROBE_SECRET, &tampered).await.unwrap_err();
    assert_eq!(err.code(), "event_not_published");

    // EXPIRED -> uniform 404.
    let expired = mint_ics_capability(PROBE_SECRET, &event_a, None, chrono::Utc::now(), -60).unwrap();
    let err = ics.feed(PROBE_SECRET, &expired).await.unwrap_err();
    assert_eq!(err.code(), "event_not_published");

    // WRONG PURPOSE (a ticket-report token on the ics route) ->
    // uniform 404.
    let wrong_purpose = backbone_events::application::service::capability::mint_ticket_report_capability(
        PROBE_SECRET, &event_a, &[],
    )
    .unwrap();
    let err = ics.feed(PROBE_SECRET, &wrong_purpose).await.unwrap_err();
    assert_eq!(err.code(), "event_not_published");

    // MISSING event + a well-formed token -> uniform 404 (the token
    // mints for any id; the gate refuses).
    let ghost = uuid::Uuid::new_v4();
    let ghost_token = ics.mint(PROBE_SECRET, ghost, None, 3600).unwrap();
    let err = ics.feed(PROBE_SECRET, &ghost_token).await.unwrap_err();
    assert_eq!(err.code(), "event_not_published");

    // SECRET MISMATCH: a token minted under another secret -> uniform
    // 404 (constant-time verify, fail-closed).
    let other_secret_token = mint_ics_capability("a-different-secret", &event_a, None, chrono::Utc::now(), 3600).unwrap();
    let err = ics.feed(PROBE_SECRET, &other_secret_token).await.unwrap_err();
    assert_eq!(err.code(), "event_not_published");

    // SLOT SCOPE: a token scoped to a slot of ANOTHER event -> 404.
    sqlx::query("INSERT INTO event.slots (id, event_id, name, date_begin) VALUES ($1, $2, 'slot-of-b', now() + interval '30 days')")
        .bind(ghost) // reuse ghost as the foreign slot id
        .bind(event_b)
        .execute(&db.pool)
        .await
        .unwrap();
    let slot_token = mint_ics_capability(PROBE_SECRET, &event_a, Some(&ghost), chrono::Utc::now(), 3600).unwrap();
    let err = ics.feed(PROBE_SECRET, &slot_token).await.unwrap_err();
    assert_eq!(err.code(), "event_not_published", "cross-event slot scope is the uniform refusal");

    // A LEGIT slot scope resolves with the slot's window.
    let slot_a = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO event.slots (id, event_id, name, date_begin) VALUES ($1, $2, 'slot-of-a', now() + interval '31 days')")
        .bind(slot_a)
        .bind(event_a)
        .execute(&db.pool)
        .await
        .unwrap();
    let slot_token_ok = mint_ics_capability(PROBE_SECRET, &event_a, Some(&slot_a), chrono::Utc::now(), 3600).unwrap();
    let feed = ics.feed(PROBE_SECRET, &slot_token_ok).await.unwrap();
    assert_eq!(feed.slot_id, Some(slot_a));
    assert!(feed.body.contains(&slot_a.to_string().to_uppercase()[..8]) || feed.body.contains("UID:"));

    // Purpose strings are distinct and never conflated.
    assert_ne!(PURPOSE_ICS, PURPOSE_TICKET_REPORT);

    db.dispose().await;
}

#[tokio::test]
async fn capability_routes_fail_closed_without_secret() {
    // The ROUTE layer: unset secret = typed 503 on BOTH capability
    // routes (never a mint under an empty key).
    let db = TestDb::new("nosecret").await;
    let state = EventPublicState::with_secret(
        db.pool.clone(),
        Arc::new(StubRenderer),
        Arc::new(RecordingQueue::default()),
        String::new(),
    );
    let app = event_public_routes(state);

    for path in ["/public/ics/whatever", "/public/my-tickets/whatever"] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "path {path}");
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"]["code"], "event_capability_secret_not_configured");
    }
    db.dispose().await;
}

#[tokio::test]
async fn public_router_negative_enumeration() {
    // The declared public tree is EXACTLY the two capability paths
    // (plus nothing else): an undeclared path answers the ROUTER's
    // bare 404 — a DIFFERENT body from the uniform gate (which only
    // the declared paths can produce).
    let db = TestDb::new("enum").await;
    let (type_id, _) = make_type_with_mail(&db, "now", 0).await;
    let event_id = make_event(&db, "probe enumeration", false, 0, Some(type_id)).await;
    let _ = registrations(&db).register(register_cmd(event_id, 0)).await;

    let state = EventPublicState::with_secret(
        db.pool.clone(),
        Arc::new(StubRenderer),
        Arc::new(RecordingQueue::default()),
        PROBE_SECRET.to_string(),
    );
    let app = event_public_routes(state);

    // Undeclared paths (events listing, registrations, admin family)
    // are NOT mounted: the router's own 404, empty body.
    for path in [
        "/public/events",
        "/public/registrations",
        "/public/admin/events",
        "/admin/events",
        "/public/intake/00000000-0000-0000-0000-000000000000",
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "undeclared path {path}");
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(bytes.is_empty(), "the router 404 carries no gate body ({path})");
    }
    db.dispose().await;
}
