//! Shared harness: one DISPOSABLE scratch database per probe,
//! FAIL-HARD (the website-module suite's contract, verbatim in
//! shape).
//!
//! The suite never runs against a shared database (and NEVER against
//! the live dev database on 5432): each probe mints
//! `event_seat_<marker>_<hex>` on the local scratch Postgres
//! (127.0.0.1:5433 — the pinned scratch container), applies this
//! module's migrations with a raw SQL file runner, runs, and drops
//! the database.
//!
//! FAIL-HARD CONTRACT: a probe that cannot reach its scratch
//! database PANICS — [`TestDb::new`] refuses to return `None`, and
//! [`skipped`] panics on principle. A green suite means the
//! behaviors were exercised, not that they were unreachable.

use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

/// The scratch Postgres every probe database is born on and dropped
/// from. 127.0.0.1:5433 — the pinned scratch container, NEVER a live
/// service database.
pub const SCRATCH_ADMIN_URL: &str = "postgres://postgres:postgres@127.0.0.1:5433/postgres";

/// The probe capability secret (explicit, never from the environment
/// — probes must not depend on host configuration).
pub const PROBE_SECRET: &str = "event-probe-capability-secret";

fn admin_url() -> String {
    std::env::var("EVENT_TEST_ADMIN_URL").unwrap_or_else(|_| SCRATCH_ADMIN_URL.into())
}

/// The fail-hard skip: reaching this is a FAILURE, never a green
/// tick.
pub fn skipped(reason: &str) -> ! {
    panic!("VACUOUS SKIP IS A FAILURE: {reason}");
}

/// One disposable scratch database, migrations applied. Panics
/// (never returns `None`) when the scratch Postgres is unreachable.
pub struct TestDb {
    pub pool: PgPool,
    name: String,
    admin: PgPool,
}

impl TestDb {
    pub async fn new(marker: &str) -> Self {
        let url = admin_url();
        let admin = match PgPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&url)
            .await
        {
            Ok(a) => a,
            Err(e) => {
                eprintln!("PROBE-FAIL: {marker}: admin connect to {url} failed: {e}");
                skipped(&format!("scratch Postgres unreachable: {e}"));
            }
        };
        let suffix: String = Uuid::new_v4().simple().to_string().chars().take(8).collect();
        let name = format!("event_seat_{marker}_{suffix}");
        // Disposable by construction: a stale DB of the same name goes
        // first.
        if let Err(e) = sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{name}" WITH (FORCE)"#))
            .execute(&admin)
            .await
        {
            eprintln!("PROBE-FAIL: {marker}: pre-drop of {name} failed: {e}");
            skipped(&format!("scratch pre-drop failed: {e}"));
        }
        if let Err(e) = sqlx::query(&format!(r#"CREATE DATABASE "{name}""#))
            .execute(&admin)
            .await
        {
            eprintln!("PROBE-FAIL: {marker}: create database {name} failed: {e}");
            skipped(&format!("scratch create failed: {e}"));
        }
        let db_url = match url.rfind('/') {
            Some(i) => format!("{}{}", &url[..=i], name),
            None => url.clone(),
        };
        let pool = match PgPoolOptions::new()
            .max_connections(12)
            .acquire_timeout(Duration::from_secs(10))
            .connect(&db_url)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                eprintln!("PROBE-FAIL: {marker}: connect to {db_url} failed: {e}");
                skipped(&format!("scratch connect failed: {e}"));
            }
        };
        if let Err(what) = apply_module_migrations(&pool, marker).await {
            skipped(&what);
        }
        Self { pool, name, admin }
    }

    /// Explicit teardown: drop the scratch database entirely.
    pub async fn dispose(self) {
        self.drop_db().await;
    }

    async fn drop_db(&self) {
        // FORCE: the connected probe pool may still hold an idle
        // session.
        let _ = sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, self.name))
            .execute(&self.admin)
            .await;
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        let name = self.name.clone();
        let url = admin_url();
        // Leak-guard teardown for panicking probes; dispose() is the
        // happy path.
        std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() {
                rt.block_on(async move {
                    if let Ok(admin) = sqlx::PgPool::connect(&url).await {
                        let _ = sqlx::query(&format!(
                            r#"DROP DATABASE IF EXISTS "{name}" WITH (FORCE)"#
                        ))
                        .execute(&admin)
                        .await;
                    }
                });
            }
        });
    }
}

/// Apply this module's migrations with a raw SQL file runner (sorted
/// `.up.sql` order — the module's files are self-contained).
async fn apply_module_migrations(pool: &PgPool, marker: &str) -> Result<(), String> {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let dir = format!("{manifest}/migrations");
    let mut files: Vec<std::path::PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".up.sql"))
                    .unwrap_or(false)
            })
            .collect(),
        Err(e) => return Err(format!("PROBE-FAIL: {marker}: cannot read {dir}: {e}")),
    };
    files.sort();
    let mut conn = pool
        .acquire()
        .await
        .map_err(|e| format!("PROBE-FAIL: {marker}: cannot acquire pool conn: {e}"))?;
    for file in files {
        let sql = std::fs::read_to_string(&file)
            .map_err(|e| format!("PROBE-FAIL: {marker}: cannot read {}: {e}", file.display()))?;
        if let Err(e) = sqlx::raw_sql(&sql).execute(&mut *conn).await {
            return Err(format!(
                "PROBE-FAIL: {marker}: migration {} failed: {e}",
                file.display()
            ));
        }
    }
    Ok(())
}

// ── shared fixtures ─────────────────────────────────────────────────────────

use backbone_events::application::service::event_service::EventCommandService;
use backbone_events::application::service::registration_service::RegistrationCommandService;
use backbone_events::application::service::scheduler_service::SchedulerService;
use backbone_events::application::service::sms_port::{EventSmsQueue, RenderedSms};
use backbone_events::application::service::template_port::{
    EventMailQueue, EventTemplateRenderer, RenderContext, RenderFailure, RenderedMail,
};
use backbone_events::infrastructure::persistence::event_command_repository::{
    CreateEventInput, EventCommandRepository,
};
use backbone_events::infrastructure::persistence::seat_repository::{
    RegisterCommand, SeatRepository,
};
use backbone_events::infrastructure::persistence::scheduler_repository::SchedulerRepository;

use async_trait::async_trait;
use std::sync::{Arc, Mutex};

/// The stub renderer: always renders a fixed body (the seam's
/// success arm).
pub struct StubRenderer;

#[async_trait]
impl EventTemplateRenderer for StubRenderer {
    async fn render(
        &self,
        template_ref: Option<uuid::Uuid>,
        _template_kind: Option<&str>,
        _ctx: &RenderContext,
    ) -> Result<RenderedMail, RenderFailure> {
        match template_ref {
            Some(t) => Ok(RenderedMail {
                subject: format!("probe-mail-{t}"),
                body_html: format!("<p>probe {t}</p>"),
                body_text: format!("probe {t}"),
            }),
            None => Err(RenderFailure::TemplateUnresolved),
        }
    }
}

/// The recording queue: captures every enqueue, never refuses.
#[derive(Default)]
pub struct RecordingQueue {
    pub sent: Mutex<Vec<(String, String)>>,
}

#[async_trait]
impl EventMailQueue for RecordingQueue {
    async fn enqueue(&self, recipient: &str, mail: &RenderedMail) -> Result<(), backbone_events::application::service::event_error::EventError> {
        if let Ok(mut guard) = self.sent.lock() {
            guard.push((recipient.to_string(), mail.subject.clone()));
        }
        Ok(())
    }
}

/// A refused-forever queue (the enqueue_refused arm).
pub struct RefusingQueue;

#[async_trait]
impl EventMailQueue for RefusingQueue {
    async fn enqueue(&self, _recipient: &str, _mail: &RenderedMail) -> Result<(), backbone_events::application::service::event_error::EventError> {
        Err(backbone_events::application::service::event_error::EventError::EnqueueRefused(
            "probe refusal".into(),
        ))
    }
}

/// The recording sms queue: captures every enqueue, never refuses.
#[derive(Default)]
pub struct RecordingSmsQueue {
    pub sent: Mutex<Vec<(String, String)>>,
}

#[async_trait]
impl EventSmsQueue for RecordingSmsQueue {
    async fn enqueue(&self, phone: &str, sms: &RenderedSms) -> Result<(), String> {
        if let Ok(mut guard) = self.sent.lock() {
            guard.push((phone.to_string(), sms.body_text.clone()));
        }
        Ok(())
    }
}

/// The module's own refusing sms default (the unconfigured-gateway
/// arm — `sms_enqueue_refused`, parked loudly).
pub use backbone_events::application::service::sms_port::RefusingSmsQueue;

/// The module's own refusing lead sink (the unwired-host arm — the
/// request parks loudly with the reason).
pub use backbone_events::application::service::lead_sink::RefusingLeadSink;

/// An event whose window is comfortably in the future.
pub fn future_window() -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
    use chrono::Duration;
    let begin = chrono::Utc::now() + Duration::days(30);
    let end = begin + Duration::days(1);
    (begin, end)
}

/// Create a plain single-slot event through the verb service.
pub async fn make_event(
    db: &TestDb,
    name: &str,
    seats_limited: bool,
    seats_max: i32,
    event_type_id: Option<uuid::Uuid>,
) -> uuid::Uuid {
    let service = EventCommandService::new(EventCommandRepository::new(db.pool.clone()));
    let (begin, end) = future_window();
    let row = service
        .create(
            &CreateEventInput {
                name: name.to_string(),
                event_type_id,
                date_begin: begin,
                date_end: end,
                seats_limited,
                seats_max,
                event_slot_count: 1,
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap_or_else(|e| panic!("probe fixture: event create failed: {e:?}"));
    row.id
}

/// Create a MULTI-slot event (the slot-mandatory arm: registrations
/// without a slot are refused typed).
pub async fn make_multi_slot_event(
    db: &TestDb,
    name: &str,
    seats_max_per_slot: i32,
) -> uuid::Uuid {
    let service = EventCommandService::new(EventCommandRepository::new(db.pool.clone()));
    let (begin, end) = future_window();
    let row = service
        .create(
            &CreateEventInput {
                name: name.to_string(),
                date_begin: begin,
                date_end: end,
                is_multi_slots: true,
                event_slot_count: 2,
                seats_limited: true,
                seats_max: seats_max_per_slot,
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap_or_else(|e| panic!("probe fixture: multi-slot event create failed: {e:?}"));
    row.id
}

/// The registration command service over the probe pool.
pub fn registrations(db: &TestDb) -> RegistrationCommandService {
    RegistrationCommandService::new(SeatRepository::new(db.pool.clone()))
}

/// A register command for the given event.
pub fn register_cmd(event_id: uuid::Uuid, n: usize) -> RegisterCommand {
    RegisterCommand {
        event_id,
        event_slot_id: None,
        event_ticket_id: None,
        name: format!("Attendee {n}"),
        email: format!("attendee{n}@probe.test"),
        phone: None,
        company_name: None,
        partner_id: None,
        actor: None,
        lead_rule_skip: false,
    }
}

/// The scheduler service with the stub renderer + the two queue ports
/// (the sms queue defaults to the refusing default — the mail probes
/// never hit it).
pub fn scheduler(db: &TestDb, queue: Arc<dyn EventMailQueue>) -> SchedulerService {
    SchedulerService::new(
        SchedulerRepository::new(db.pool.clone()),
        EventCommandRepository::new(db.pool.clone()),
        Arc::new(StubRenderer),
        queue,
        Arc::new(RefusingSmsQueue),
    )
}

/// The scheduler service with an explicit sms queue (the sms probe).
pub fn scheduler_with_sms(
    db: &TestDb,
    queue: Arc<dyn EventMailQueue>,
    sms_queue: Arc<dyn EventSmsQueue>,
) -> SchedulerService {
    SchedulerService::new(
        SchedulerRepository::new(db.pool.clone()),
        EventCommandRepository::new(db.pool.clone()),
        Arc::new(StubRenderer),
        queue,
        sms_queue,
    )
}

/// A type carrying one after_sub type_mail row (the template-apply
/// source); returns (type_id, template_id).
pub async fn make_type_with_mail(
    db: &TestDb,
    interval_unit: &str,
    interval_nbr: i32,
) -> (uuid::Uuid, uuid::Uuid) {
    let type_id = uuid::Uuid::new_v4();
    let template_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO event.types (id, name) VALUES ($1, 'probe-type')")
        .bind(type_id)
        .execute(&db.pool)
        .await
        .unwrap_or_else(|e| panic!("probe fixture: type insert: {e}"));
    sqlx::query(
        r#"INSERT INTO event.type_mails
               (event_type_id, interval_nbr, interval_unit, interval_kind,
                notification_channel, template_ref, template_kind)
           VALUES ($1, $2, $3::event_interval_unit, 'after_sub', 'mail', $4, 'mail')"#,
    )
    .bind(type_id)
    .bind(interval_nbr)
    .bind(interval_unit)
    .bind(template_id)
    .execute(&db.pool)
    .await
    .unwrap_or_else(|e| panic!("probe fixture: type_mail insert: {e}"));
    (type_id, template_id)
}

/// A type carrying one after_sub SMS type_mail row (the sms arm's
/// template-apply source); returns (type_id, template_id).
pub async fn make_type_with_sms(
    db: &TestDb,
    interval_unit: &str,
    interval_nbr: i32,
) -> (uuid::Uuid, uuid::Uuid) {
    let type_id = uuid::Uuid::new_v4();
    let template_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO event.types (id, name) VALUES ($1, 'probe-type-sms')")
        .bind(type_id)
        .execute(&db.pool)
        .await
        .unwrap_or_else(|e| panic!("probe fixture: type insert: {e}"));
    sqlx::query(
        r#"INSERT INTO event.type_mails
               (event_type_id, interval_nbr, interval_unit, interval_kind,
                notification_channel, template_ref, template_kind)
           VALUES ($1, $2, $3::event_interval_unit, 'after_sub', 'sms', $4, 'sms')"#,
    )
    .bind(type_id)
    .bind(interval_nbr)
    .bind(interval_unit)
    .bind(template_id)
    .execute(&db.pool)
    .await
    .unwrap_or_else(|e| panic!("probe fixture: sms type_mail insert: {e}"));
    (type_id, template_id)
}
