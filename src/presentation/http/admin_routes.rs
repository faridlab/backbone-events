//! The officer tree (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! The module DOES NOT SELF-MOUNT and DOES NOT SELF-GATE: it
//! exports [`event_admin_routes`], a plain `axum::Router` the host
//! nests under the schema name BEHIND `company_auth`, with
//! `ModuleWriteGate::new(pool, "event")` as the INNERMOST
//! `route_layer` (the website-module pattern verbatim: write gate
//! innermost, company_auth outside). Authority names resolve through
//! the host gate: `write:event` (POST/PATCH), `delete:event`
//! (DELETE), supersets above them.
//!
//! The acting OFFICER id arrives through the [`EventActor`] request
//! extension (the host's company_auth bridge inserts it); without
//! it the verbs run as the system actor — never a public principal.
//!
//! Route table:
//! - GET/POST  /admin/events                    list / create (template-apply once)
//! - GET/PATCH /admin/events/:id                read / typed patch (FENCE: is_published,
//!                                             date_publish refused)
//! - POST /admin/events/:id/publish|unpublish|mark-done
//! - GET  /admin/events/:id/seats               the ONE counter's read (?slot=)
//! - POST /admin/registrations                   the ONE register verb
//! - GET  /admin/registrations?event_id=         the officer list
//! - GET/PATCH /admin/registrations/:id          read / attendee-fields-only patch
//! - POST /admin/registrations/:id/confirm|set-draft|set-done|cancel|sync-from-partner
//! - GET  /admin/mails?event_id=                 the scheduler rows
//! - POST /admin/mails/:id/run                   run one scheduler now
//! - POST /admin/scheduler/run                   run the pass now
//! - POST /admin/sweep/mark-done                 the done sweep
//! - POST /admin/desk/register-attendee          the desk scan (frozen branch order)
//! - GET/POST /admin/booths                      list (?event_id=) / create
//! - GET/PATCH/DELETE /admin/booths/:id          read / patch (fill-if-empty) / fenced delete
//! - POST /admin/booths/:id/release              the human release verb
//! - POST /admin/booths/mark-paid                the one-way paid latch ({line_ids})
//! - GET/POST /admin/booths/:id/bookings         booking intents
//! - GET/DELETE /admin/booth-bookings/:id        read / withdraw a pending intent
//! - POST /admin/booth-bookings/:id/confirm      the explicit confirm (the DB exclusivity wall)
//! - GET/POST /admin/lead-rules                  list / create (closed vocabulary)
//! - GET/PATCH /admin/lead-rules/:id             the rule read model / patch
//! - POST /admin/lead-rules/from-answer          the answer-to-rule bridge
//! - POST /admin/lead-rules/relink               the merge-relink verb
//! - GET  /admin/lead-requests?event_id=         the queue row read
//! - POST /admin/lead-requests/run               the generation pass now
//! - POST /admin/template-cascade/on-template-deleted  the one cascade verb (both channels)
//! - GET  /admin/catalog/event-linked-products   EP-2's exclusion predicate read

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::Extensions,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::application::service::booth_command_service::BoothCommandService;
use crate::application::service::desk_service::DeskService;
use crate::application::service::event_error::{EventError, EventResult};
use crate::application::service::event_service::{EventCommandService, PUBLISH_FENCED_FIELDS};
use crate::application::service::lead_command_service::{
    CreateRuleInput, FromAnswerInput, LeadRuleCommandService, PatchRuleInput, RelinkInput,
};
use crate::application::service::lead_generation_service::LeadGenerationService;
use crate::application::service::lead_sink::EventLeadSink;
use crate::application::service::registration_service::RegistrationCommandService;
use crate::application::service::scheduler_service::SchedulerService;
use crate::application::service::seat_service::SeatService;
use crate::application::service::sms_port::EventSmsQueue;
use crate::application::service::template_port::{EventMailQueue, EventTemplateRenderer};
use crate::infrastructure::persistence::booth_command_repository::BoothCommandRepository;
use crate::infrastructure::persistence::event_command_repository::{
    CreateEventInput, EventCommandRepository, PatchEventInput,
};
use crate::infrastructure::persistence::lead_command_repository::LeadCommandRepository;
use crate::infrastructure::persistence::seat_repository::{RegisterCommand, SeatRepository};
use crate::infrastructure::persistence::scheduler_repository::SchedulerRepository;

/// The request extension carrying the acting officer id (the host's
/// company_auth bridge inserts it after authentication).
#[derive(Debug, Clone, Copy)]
pub struct EventActor(pub Uuid);

fn actor_of(extensions: &Extensions) -> Option<Uuid> {
    extensions.get::<EventActor>().map(|EventActor(id)| *id)
}

/// The module's admin state — the hand services over one pool.
#[derive(Clone)]
pub struct EventAdminState {
    pub events: Arc<EventCommandService>,
    pub seats: Arc<SeatService>,
    pub registrations: Arc<RegistrationCommandService>,
    pub scheduler: Arc<SchedulerService>,
    pub booths: Arc<BoothCommandService>,
    pub lead_rules: Arc<LeadRuleCommandService>,
    pub leads: Arc<LeadGenerationService>,
    pub desk: Arc<DeskService>,
}

impl EventAdminState {
    /// Compose with the host-installed ports (renderer, mail queue,
    /// sms queue, lead sink — each has a refusing default so an
    /// unwired host gets typed failures, never silent skips).
    pub fn new(
        pool: sqlx::PgPool,
        renderer: Arc<dyn EventTemplateRenderer>,
        queue: Arc<dyn EventMailQueue>,
        sms_queue: Arc<dyn EventSmsQueue>,
        lead_sink: Arc<dyn EventLeadSink>,
    ) -> Self {
        let seat_repo = SeatRepository::new(pool.clone());
        let event_repo = EventCommandRepository::new(pool.clone());
        let scheduler_repo = SchedulerRepository::new(pool.clone());
        Self {
            events: Arc::new(EventCommandService::new(EventCommandRepository::new(
                pool.clone(),
            ))),
            seats: Arc::new(SeatService::new(
                SeatRepository::new(pool.clone()),
                EventCommandRepository::new(pool.clone()),
            )),
            registrations: Arc::new(RegistrationCommandService::new(seat_repo)),
            scheduler: Arc::new(SchedulerService::new(
                scheduler_repo,
                event_repo,
                renderer,
                queue,
                sms_queue,
            )),
            booths: Arc::new(BoothCommandService::new(BoothCommandRepository::new(
                pool.clone(),
            ))),
            lead_rules: Arc::new(LeadRuleCommandService::new(LeadCommandRepository::new(
                pool.clone(),
            ))),
            leads: Arc::new(LeadGenerationService::new(
                LeadCommandRepository::new(pool.clone()),
                lead_sink,
            )),
            desk: Arc::new(DeskService::new(
                SeatRepository::new(pool.clone()),
                EventCommandRepository::new(pool.clone()),
            )),
        }
    }
}

/// THE OFFICER TREE (see the module doc for the table).
pub fn event_admin_routes(state: EventAdminState) -> Router {
    Router::new()
        .route("/admin/events", get(list_events).post(create_event))
        .route("/admin/events/:id", get(get_event).patch(patch_event))
        .route("/admin/events/:id/publish", post(publish_event))
        .route("/admin/events/:id/unpublish", post(unpublish_event))
        .route("/admin/events/:id/mark-done", post(mark_done_event))
        .route("/admin/events/:id/seats", get(event_seats))
        .route(
            "/admin/registrations",
            get(list_registrations).post(register_registration),
        )
        .route("/admin/registrations/:id", get(get_registration).patch(patch_registration))
        .route("/admin/registrations/:id/confirm", post(confirm_registration))
        .route("/admin/registrations/:id/set-draft", post(set_draft_registration))
        .route("/admin/registrations/:id/set-done", post(set_done_registration))
        .route("/admin/registrations/:id/cancel", post(cancel_registration))
        .route(
            "/admin/registrations/:id/sync-from-partner",
            post(sync_from_partner_registration),
        )
        .route("/admin/mails", get(list_mails))
        .route("/admin/mails/:id/run", post(run_mail))
        .route("/admin/scheduler/run", post(run_scheduler))
        .route("/admin/sweep/mark-done", post(run_sweep))
        .route("/admin/desk/register-attendee", post(desk_register_attendee))
        .route("/admin/booths", get(list_booths).post(create_booth))
        .route(
            "/admin/booths/:id",
            get(get_booth).patch(patch_booth).delete(delete_booth),
        )
        .route("/admin/booths/:id/release", post(release_booth))
        .route("/admin/booths/mark-paid", post(mark_booths_paid))
        .route(
            "/admin/booths/:id/bookings",
            get(list_booth_bookings).post(create_booth_booking),
        )
        .route(
            "/admin/booth-bookings/:id",
            get(get_booth_booking).delete(delete_booth_booking),
        )
        .route("/admin/booth-bookings/:id/confirm", post(confirm_booth_booking))
        .route("/admin/lead-rules", get(list_lead_rules).post(create_lead_rule))
        .route("/admin/lead-rules/from-answer", post(lead_rule_from_answer))
        .route("/admin/lead-rules/relink", post(lead_rule_relink))
        .route("/admin/lead-rules/:id", get(get_lead_rule).patch(patch_lead_rule))
        .route("/admin/lead-requests", get(get_lead_request))
        .route("/admin/lead-requests/run", post(run_lead_requests))
        .route(
            "/admin/template-cascade/on-template-deleted",
            post(on_template_deleted),
        )
        .route("/admin/catalog/event-linked-products", get(event_linked_products))
        .with_state(state)
}

// ── events ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
struct ListEventsQuery {
    limit: Option<i64>,
}

async fn list_events(
    State(state): State<EventAdminState>,
    Query(q): Query<ListEventsQuery>,
) -> Response {
    reply(state.events.list(q.limit.unwrap_or(50).clamp(1, 500)).await.map(Json))
}

async fn create_event(
    State(state): State<EventAdminState>,
    extensions: Extensions,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let input = match parse_create_event(body) {
        Ok(i) => i,
        Err(e) => return e.into_response(),
    };
    reply(
        state
            .events
            .create(&input, actor_of(&extensions))
            .await
            .map(|row| (axum::http::StatusCode::CREATED, Json(row))),
    )
}

fn parse_create_event(body: serde_json::Value) -> EventResult<CreateEventInput> {
    let date_begin = parse_ts(body.get("date_begin"), "date_begin")?;
    let date_end = parse_ts(body.get("date_end"), "date_end")?;
    Ok(CreateEventInput {
        name: string_of(&body, "name")?,
        event_type_id: uuid_of(&body, "event_type_id"),
        date_begin,
        date_end,
        date_tz: opt_string_of(&body, "date_tz"),
        is_multi_slots: bool_of(&body, "is_multi_slots").unwrap_or(false),
        event_slot_count: int_of(&body, "event_slot_count").unwrap_or(1),
        seats_limited: bool_of(&body, "seats_limited").unwrap_or(false),
        seats_max: int_of(&body, "seats_max").unwrap_or(0),
        organizer_id: uuid_of(&body, "organizer_id"),
        user_id: uuid_of(&body, "user_id"),
        address_id: uuid_of(&body, "address_id"),
        event_url: opt_string_of(&body, "event_url"),
        badge_format: opt_string_of(&body, "badge_format"),
    })
}

async fn get_event(State(state): State<EventAdminState>, Path(id): Path<Uuid>) -> Response {
    reply(state.events.get(id).await.map(Json))
}

async fn patch_event(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
    Json(body): Json<serde_json::Value>,
) -> Response {
    // THE FENCE: the publication pair in a patch body is the typed
    // refusal (publish/unpublish are the only writers).
    if let Some(obj) = body.as_object() {
        let hits: Vec<&str> = PUBLISH_FENCED_FIELDS
            .iter()
            .filter(|f| obj.contains_key(**f))
            .copied()
            .collect();
        if !hits.is_empty() {
            let e = EventError::PublishRefused {
                fields: hits.join(", "),
            };
            // The fence refusal is a critical audit fact.
            return e.into_response();
        }
    }
    let patch = match parse_patch_event(&body) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };
    reply(state.events.patch(id, &patch, actor_of(&extensions)).await.map(Json))
}

fn parse_patch_event(body: &serde_json::Value) -> EventResult<PatchEventInput> {
    Ok(PatchEventInput {
        name: opt_string_of(body, "name"),
        event_type_id: uuid_of(body, "event_type_id"),
        stage_id: uuid_of(body, "stage_id"),
        date_begin: opt_ts(body, "date_begin")?,
        date_end: opt_ts(body, "date_end")?,
        date_tz: opt_string_of(body, "date_tz"),
        is_multi_slots: opt_bool_of(body, "is_multi_slots"),
        event_slot_count: opt_int_of(body, "event_slot_count"),
        seats_limited: opt_bool_of(body, "seats_limited"),
        seats_max: opt_int_of(body, "seats_max"),
        organizer_id: uuid_of(body, "organizer_id"),
        user_id: uuid_of(body, "user_id"),
        address_id: uuid_of(body, "address_id"),
        event_url: opt_string_of(body, "event_url"),
        badge_format: opt_string_of(body, "badge_format"),
    })
}

async fn publish_event(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
) -> Response {
    reply(state.events.publish(id, actor_of(&extensions)).await.map(Json))
}

async fn unpublish_event(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
) -> Response {
    reply(state.events.unpublish(id, actor_of(&extensions)).await.map(Json))
}

async fn mark_done_event(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
) -> Response {
    reply(state.events.mark_done(id, actor_of(&extensions)).await.map(Json))
}

#[derive(Debug, Deserialize)]
struct SeatsQuery {
    slot: Option<Uuid>,
}

async fn event_seats(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    Query(q): Query<SeatsQuery>,
) -> Response {
    reply(state.seats.availability(id, q.slot).await.map(Json))
}

// ── registrations ─────────────────────────────────────────────────────────

async fn register_registration(
    State(state): State<EventAdminState>,
    extensions: Extensions,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let cmd = RegisterCommand {
        event_id: match uuid_of(&body, "event_id") {
            Some(id) => id,
            None => {
                return EventError::Validation("event_id is required".into()).into_response()
            }
        },
        event_slot_id: uuid_of(&body, "event_slot_id"),
        event_ticket_id: uuid_of(&body, "event_ticket_id"),
        name: match string_of(&body, "name") {
            Ok(n) => n,
            Err(e) => return e.into_response(),
        },
        email: match string_of(&body, "email") {
            Ok(n) => n,
            Err(e) => return e.into_response(),
        },
        phone: opt_string_of(&body, "phone"),
        company_name: opt_string_of(&body, "company_name"),
        partner_id: uuid_of(&body, "partner_id"),
        actor: actor_of(&extensions),
        // Officer-created registrations arm the lead queue (only the
        // bulkops import path skips the arm).
        lead_rule_skip: false,
    };
    reply(
        state
            .registrations
            .register(cmd)
            .await
            .map(|row| (axum::http::StatusCode::CREATED, Json(row))),
    )
}

#[derive(Debug, Deserialize)]
struct ListRegistrationsQuery {
    event_id: Uuid,
    limit: Option<i64>,
    after: Option<Uuid>,
}

async fn list_registrations(
    State(state): State<EventAdminState>,
    Query(q): Query<ListRegistrationsQuery>,
) -> Response {
    reply(
        state
            .registrations
            .list(q.event_id, q.limit.unwrap_or(50).clamp(1, 500), q.after)
            .await
            .map(Json),
    )
}

async fn get_registration(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
) -> Response {
    reply(state.registrations.get(id).await.map(Json))
}

async fn patch_registration(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
    Json(body): Json<serde_json::Value>,
) -> Response {
    // Attendee fields only: the state axis is NOT patchable here
    // (the four verbs below are the only paths), and neither are the
    // sale-mirror columns or the archive axis.
    if let Some(obj) = body.as_object() {
        const REFUSED: [&str; 8] = [
            "state",
            "sale_order_id",
            "sale_order_state",
            "sale_status",
            "active",
            "barcode",
            "event_id",
            "id",
        ];
        let hits: Vec<&str> = REFUSED.iter().filter(|f| obj.contains_key(**f)).copied().collect();
        if !hits.is_empty() {
            return EventError::Validation(format!(
                "patch refuses non-attendee fields: {}",
                hits.join(", ")
            ))
            .into_response();
        }
    }
    match state
        .registrations
        .sync_from_partner(
            id,
            uuid_of(&body, "partner_id").unwrap_or_default(),
            opt_string_of(&body, "name").as_deref(),
            opt_string_of(&body, "phone").as_deref(),
            opt_string_of(&body, "company_name").as_deref(),
            actor_of(&extensions),
        )
        .await
    {
        Ok(row) => (axum::http::StatusCode::OK, Json(row)).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn confirm_registration(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
) -> Response {
    reply(
        state
            .registrations
            .confirm(id, actor_of(&extensions))
            .await
            .map(Json),
    )
}

async fn set_draft_registration(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
) -> Response {
    reply(
        state
            .registrations
            .set_draft(id, actor_of(&extensions))
            .await
            .map(Json),
    )
}

async fn set_done_registration(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
) -> Response {
    reply(
        state
            .registrations
            .set_done(id, actor_of(&extensions))
            .await
            .map(Json),
    )
}

async fn cancel_registration(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
) -> Response {
    reply(
        state
            .registrations
            .cancel(id, actor_of(&extensions))
            .await
            .map(Json),
    )
}

async fn sync_from_partner_registration(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let partner_id = match uuid_of(&body, "partner_id") {
        Some(id) => id,
        None => {
            return EventError::Validation("partner_id is required".into()).into_response()
        }
    };
    reply(
        state
            .registrations
            .sync_from_partner(
                id,
                partner_id,
                opt_string_of(&body, "name").as_deref(),
                opt_string_of(&body, "phone").as_deref(),
                opt_string_of(&body, "company_name").as_deref(),
                actor_of(&extensions),
            )
            .await
            .map(Json),
    )
}

// ── schedulers / sweeps ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ListMailsQuery {
    event_id: Option<Uuid>,
}

async fn list_mails(
    State(state): State<EventAdminState>,
    Query(q): Query<ListMailsQuery>,
) -> Response {
    match q.event_id {
        Some(event_id) => reply(state.scheduler.list_for_event(event_id).await.map(Json)),
        None => EventError::Validation("event_id is required".into()).into_response(),
    }
}

async fn run_mail(State(state): State<EventAdminState>, Path(id): Path<Uuid>) -> Response {
    reply(state.scheduler.run_scheduler_by_id(id).await.map(Json))
}

async fn run_scheduler(State(state): State<EventAdminState>) -> Response {
    reply(state.scheduler.run_due_schedulers().await.map(Json))
}

async fn run_sweep(State(state): State<EventAdminState>) -> Response {
    reply(
        state
            .scheduler
            .sweep_mark_done(500)
            .await
            .map(|ids| Json(json!({ "swept": ids.len(), "event_ids": ids }))),
    )
}

// ── the desk verb ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DeskScanBody {
    barcode: String,
    event_id: Uuid,
}

async fn desk_register_attendee(
    State(state): State<EventAdminState>,
    extensions: Extensions,
    Json(body): Json<DeskScanBody>,
) -> Response {
    if body.barcode.trim().is_empty() {
        return EventError::Validation("barcode is required".into()).into_response();
    }
    reply(
        state
            .desk
            .register_attendee(body.barcode.trim(), body.event_id, actor_of(&extensions))
            .await
            .map(|(row, before, after)| {
                Json(json!({ "registration": row, "before_state": before, "after_state": after }))
            }),
    )
}

// ── booths + bookings ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ListBoothsQuery {
    event_id: Option<Uuid>,
    limit: Option<i64>,
}

async fn list_booths(
    State(state): State<EventAdminState>,
    Query(q): Query<ListBoothsQuery>,
) -> Response {
    match q.event_id {
        Some(event_id) => reply(
            state
                .booths
                .list_booths_of_event(event_id, q.limit.unwrap_or(50).clamp(1, 500))
                .await
                .map(Json),
        ),
        None => EventError::Validation("event_id is required".into()).into_response(),
    }
}

async fn create_booth(
    State(state): State<EventAdminState>,
    extensions: Extensions,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let (event_id, booth_category_id, name) = match (
        uuid_of(&body, "event_id"),
        uuid_of(&body, "booth_category_id"),
        string_of(&body, "name"),
    ) {
        (Some(e), Some(c), Ok(n)) => (e, c, n),
        (.., Err(e)) => return e.into_response(),
        _ => {
            return EventError::Validation(
                "event_id and booth_category_id are required".into(),
            )
            .into_response()
        }
    };
    reply(
        state
            .booths
            .create_booth(event_id, booth_category_id, &name, actor_of(&extensions))
            .await
            .map(|row| (axum::http::StatusCode::CREATED, Json(row))),
    )
}

async fn get_booth(State(state): State<EventAdminState>, Path(id): Path<Uuid>) -> Response {
    reply(state.booths.find_booth(id).await.map(Json))
}

async fn patch_booth(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
    Json(body): Json<serde_json::Value>,
) -> Response {
    // Writable surface: name/category set; partner + contacts
    // fill-if-empty. The lifecycle axis and the sale mirrors are NOT
    // patchable (confirm/release/mark-paid are the only writers).
    if let Some(obj) = body.as_object() {
        const REFUSED: [&str; 5] = ["state", "sale_order_line_id", "is_paid", "event_id", "id"];
        let hits: Vec<&str> = REFUSED.iter().filter(|f| obj.contains_key(**f)).copied().collect();
        if !hits.is_empty() {
            return EventError::Validation(format!(
                "booth patch refuses non-writable fields: {}",
                hits.join(", ")
            ))
            .into_response();
        }
    }
    reply(
        state
            .booths
            .patch_booth(
                id,
                opt_string_of(&body, "name").as_deref(),
                uuid_of(&body, "booth_category_id"),
                uuid_of(&body, "partner_id"),
                opt_string_of(&body, "contact_name").as_deref(),
                opt_string_of(&body, "contact_email").as_deref(),
                opt_string_of(&body, "contact_phone").as_deref(),
                actor_of(&extensions),
            )
            .await
            .map(Json),
    )
}

async fn delete_booth(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
) -> Response {
    reply(
        state
            .booths
            .delete_booth(id, actor_of(&extensions))
            .await
            .map(|_| Json(json!({ "deleted": id }))),
    )
}

async fn release_booth(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
) -> Response {
    reply(
        state
            .booths
            .release_booth(id, actor_of(&extensions))
            .await
            .map(Json),
    )
}

async fn mark_booths_paid(
    State(state): State<EventAdminState>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let line_ids = match uuid_vec_of(&body, "line_ids") {
        Some(ids) if !ids.is_empty() => ids,
        _ => {
            return EventError::Validation(
                "line_ids must be a non-empty array of uuids".into(),
            )
            .into_response()
        }
    };
    reply(
        state
            .booths
            .mark_booths_paid(&line_ids)
            .await
            .map(|n| Json(json!({ "booths_latched": n }))),
    )
}

async fn create_booth_booking(
    State(state): State<EventAdminState>,
    Path(booth_id): Path<Uuid>,
    extensions: Extensions,
    Json(body): Json<serde_json::Value>,
) -> Response {
    reply(
        state
            .booths
            .create_booking(
                booth_id,
                uuid_of(&body, "sale_order_line_id"),
                uuid_of(&body, "partner_id"),
                opt_string_of(&body, "contact_name").as_deref(),
                opt_string_of(&body, "contact_email").as_deref(),
                opt_string_of(&body, "contact_phone").as_deref(),
                actor_of(&extensions),
            )
            .await
            .map(|row| (axum::http::StatusCode::CREATED, Json(row))),
    )
}

async fn list_booth_bookings(
    State(state): State<EventAdminState>,
    Path(booth_id): Path<Uuid>,
) -> Response {
    reply(state.booths.list_bookings_of_booth(booth_id).await.map(Json))
}

async fn get_booth_booking(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
) -> Response {
    reply(state.booths.find_booking(id).await.map(Json))
}

async fn confirm_booth_booking(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
) -> Response {
    reply(
        state
            .booths
            .confirm_booking(id, actor_of(&extensions))
            .await
            .map(|(booking, booth)| Json(json!({ "booking": booking, "booth": booth }))),
    )
}

async fn delete_booth_booking(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
) -> Response {
    reply(
        state
            .booths
            .delete_booking(id, actor_of(&extensions))
            .await
            .map(|_| Json(json!({ "deleted": id }))),
    )
}

// ── lead rules + the generation queue ─────────────────────────────────────

/// Typed parse for the deny_unknown_fields DTOs (a shape drift is a
/// typed refusal, never a silently-ignored field).
fn parse_typed<T: serde::de::DeserializeOwned>(body: serde_json::Value) -> EventResult<T> {
    serde_json::from_value(body)
        .map_err(|e| EventError::Validation(format!("body parse refused: {e}")))
}

#[derive(Debug, Deserialize, Default)]
struct ListRulesQuery {
    limit: Option<i64>,
}

async fn list_lead_rules(
    State(state): State<EventAdminState>,
    Query(q): Query<ListRulesQuery>,
) -> Response {
    reply(
        state
            .lead_rules
            .list_rules(q.limit.unwrap_or(50).clamp(1, 500))
            .await
            .map(Json),
    )
}

async fn create_lead_rule(
    State(state): State<EventAdminState>,
    extensions: Extensions,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let input: CreateRuleInput = match parse_typed(body) {
        Ok(i) => i,
        Err(e) => return e.into_response(),
    };
    reply(
        state
            .lead_rules
            .create_rule(input, actor_of(&extensions))
            .await
            .map(|row| (axum::http::StatusCode::CREATED, Json(row))),
    )
}

async fn get_lead_rule(State(state): State<EventAdminState>, Path(id): Path<Uuid>) -> Response {
    reply(state.lead_rules.rule_read_model(id).await.map(Json))
}

async fn patch_lead_rule(
    State(state): State<EventAdminState>,
    Path(id): Path<Uuid>,
    extensions: Extensions,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let input: PatchRuleInput = match parse_typed(body) {
        Ok(i) => i,
        Err(e) => return e.into_response(),
    };
    reply(
        state
            .lead_rules
            .patch_rule(id, input, actor_of(&extensions))
            .await
            .map(Json),
    )
}

async fn lead_rule_from_answer(
    State(state): State<EventAdminState>,
    extensions: Extensions,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let input: FromAnswerInput = match parse_typed(body) {
        Ok(i) => i,
        Err(e) => return e.into_response(),
    };
    reply(
        state
            .lead_rules
            .from_answer(input, actor_of(&extensions))
            .await
            .map(|row| (axum::http::StatusCode::CREATED, Json(row))),
    )
}

async fn lead_rule_relink(
    State(state): State<EventAdminState>,
    extensions: Extensions,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let input: RelinkInput = match parse_typed(body) {
        Ok(i) => i,
        Err(e) => return e.into_response(),
    };
    reply(
        state
            .lead_rules
            .relink_lead(input, actor_of(&extensions))
            .await
            .map(|n| Json(json!({ "provenance_rows_moved": n }))),
    )
}

#[derive(Debug, Deserialize)]
struct LeadRequestQuery {
    event_id: Uuid,
}

async fn get_lead_request(
    State(state): State<EventAdminState>,
    Query(q): Query<LeadRequestQuery>,
) -> Response {
    reply(
        state
            .leads
            .request_of_event(q.event_id)
            .await
            .map(|row| Json(json!({ "request": row }))),
    )
}

async fn run_lead_requests(State(state): State<EventAdminState>) -> Response {
    reply(state.leads.run_due_lead_requests().await.map(Json))
}

// ── the template cascade + the catalog read ───────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateDeletedBody {
    template_kind: String,
    template_ref: Uuid,
}

async fn on_template_deleted(
    State(state): State<EventAdminState>,
    extensions: Extensions,
    Json(body): Json<TemplateDeletedBody>,
) -> Response {
    reply(
        state
            .scheduler
            .on_template_deleted(
                body.template_kind.trim(),
                body.template_ref,
                actor_of(&extensions),
            )
            .await
            .map(|(mails, type_mails)| {
                Json(json!({ "mails_deleted": mails, "type_mails_deleted": type_mails }))
            }),
    )
}

async fn event_linked_products(State(state): State<EventAdminState>) -> Response {
    reply(
        state
            .events
            .linked_products()
            .await
            .map(|ids| Json(json!({ "product_ids": ids }))),
    )
}

// ── tiny JSON arms (the typed parse layer) ────────────────────────────────

fn string_of(body: &serde_json::Value, key: &str) -> EventResult<String> {
    body.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| EventError::Validation(format!("{key} is required")))
}

fn opt_string_of(body: &serde_json::Value, key: &str) -> Option<String> {
    body.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn bool_of(body: &serde_json::Value, key: &str) -> Option<bool> {
    body.get(key).and_then(|v| v.as_bool())
}

fn opt_bool_of(body: &serde_json::Value, key: &str) -> Option<bool> {
    bool_of(body, key)
}

fn int_of(body: &serde_json::Value, key: &str) -> Option<i32> {
    body.get(key).and_then(|v| v.as_i64()).map(|v| v as i32)
}

fn opt_int_of(body: &serde_json::Value, key: &str) -> Option<i32> {
    int_of(body, key)
}

fn uuid_of(body: &serde_json::Value, key: &str) -> Option<Uuid> {
    body.get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

fn uuid_vec_of(body: &serde_json::Value, key: &str) -> Option<Vec<Uuid>> {
    body.get(key)?
        .as_array()?
        .iter()
        .map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
        .collect()
}

fn parse_ts(value: Option<&serde_json::Value>, key: &str) -> EventResult<chrono::DateTime<chrono::Utc>> {
    value
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc))
        .ok_or_else(|| EventError::Validation(format!("{key} must be an RFC 3339 timestamp")))
}

fn opt_ts(
    body: &serde_json::Value,
    key: &str,
) -> EventResult<Option<chrono::DateTime<chrono::Utc>>> {
    if body.get(key).is_none() {
        return Ok(None);
    }
    parse_ts(body.get(key), key).map(Some)
}

fn reply<T: IntoResponse>(result: EventResult<T>) -> Response {
    match result {
        Ok(payload) => payload.into_response(),
        Err(e) => e.into_response(),
    }
}
