//! The fenced-runtime probe: the repositories must pass the company
//! RLS fence when the pool connects as a NON-superuser role.
//!
//! Every other probe runs on the scratch SUPERUSER, which bypasses
//! row-level security entirely — the exact blind spot that once let
//! unscoped repositories ship green here while the fenced production
//! runtime failed (writes 42501, reads silently empty). This probe
//! recreates the production posture on the scratch database: a LOGIN
//! role with plain DML grants and NO BYPASSRLS, the migrations'
//! FORCE ROW LEVEL SECURITY policies active, and every repository
//! call driven through `with_company_scope`. If a repository ever
//! regresses to a raw unscoped statement, the scoped write below
//! fails the policy's WITH CHECK and this probe goes red.

use backbone_events::application::service::event_error::EventError;
use backbone_events::infrastructure::persistence::event_command_repository::{
    CreateEventInput, EventCommandRepository,
};
use backbone_orm::company_scope::with_company_scope;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::{future_window, TestDb};

/// The non-privileged role every fenced connection authenticates as.
const FENCED_ROLE: &str = "event_probe_app";
const FENCED_PASSWORD: &str = "event_probe_app";

/// Mint the production-shaped app role on the scratch cluster: LOGIN,
/// DML on the module's tables, NO BYPASSRLS. Cluster-wide, so the
/// name is unique to this probe. A role from an aborted run can hold
/// grants in a leaked scratch database (DROP ROLE refuses while any
/// depend on it), so stale fenced-marker databases go first — the
/// role then drops clean.
async fn fence_role(db: &TestDb) {
    let stale: Vec<String> = sqlx::query_scalar(
        "SELECT datname FROM pg_database \
         WHERE datname LIKE 'event\\_seat\\_fenced\\_%' AND datname <> current_database()",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    for name in stale {
        sqlx::query(&format!(r#"DROP DATABASE "{name}" WITH (FORCE)"#))
            .execute(&db.pool)
            .await
            .unwrap_or_else(|e| panic!("stale scratch {name} drop: {e}"));
    }
    sqlx::raw_sql(&format!(
        "DROP ROLE IF EXISTS {FENCED_ROLE}; \
         CREATE ROLE {FENCED_ROLE} LOGIN PASSWORD '{FENCED_PASSWORD}' NOSUPERUSER NOBYPASSRLS; \
         GRANT USAGE ON SCHEMA event TO {FENCED_ROLE}; \
         GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA event TO {FENCED_ROLE}; \
         GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA event TO {FENCED_ROLE};"
    ))
    .execute(&db.pool)
    .await
    .unwrap();
}

/// A pool for the SAME scratch database authenticating as the fenced
/// role — the posture the production runtime pool runs under.
async fn fenced_pool(db: &TestDb) -> PgPool {
    let opts = db.pool.connect_options();
    let dsn = format!(
        "postgres://{FENCED_ROLE}:{FENCED_PASSWORD}@{}:{}/{}",
        opts.get_host(),
        opts.get_port(),
        opts.get_database().unwrap(),
    );
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&dsn)
        .await
        .unwrap()
}

fn input(name: &str, company: Option<Uuid>) -> CreateEventInput {
    let (begin, end) = future_window();
    CreateEventInput {
        name: name.into(),
        date_begin: begin,
        date_end: end,
        event_slot_count: 1,
        company_id: company,
        ..Default::default()
    }
}

#[tokio::test]
async fn repositories_pass_the_fence_under_a_scoped_company() {
    let db = TestDb::new("fenced").await;
    fence_role(&db).await;
    let fenced = fenced_pool(&db).await;

    let company_a = Uuid::new_v4();
    let company_b = Uuid::new_v4();

    // Seed the OTHER company's event over the owner pool (the setup
    // path migrations and seeders legitimately use), inside the
    // owner-side scope so the row lands company-scoped, not NULL.
    let other = EventCommandRepository::new(db.pool.clone());
    let seeded = with_company_scope(
        Some(company_b),
        other.create(&input("company b event", Some(company_b)), None),
    )
    .await
    .unwrap();

    let repo = EventCommandRepository::new(fenced.clone());

    // THE WALL: a write whose company_id claims a tenant but whose
    // scope carries none is refused by the policy's WITH CHECK — the
    // exact failure unscoped repositories produced in the fenced
    // runtime (the row names a company, the connection never did).
    let refused = match repo.create(&input("unscoped", Some(company_a)), None).await {
        Err(EventError::Database(msg)) => msg,
        other => panic!("unscoped fenced write must fail at the database, got {other:?}"),
    };
    assert!(
        refused.contains("row-level security"),
        "refusal must be the RLS policy violation, got: {refused}"
    );

    // The forgery wall: a scoped write claiming ANOTHER company's id
    // is refused too — the scope vouches for its own tenant only.
    let forged = with_company_scope(
        Some(company_a),
        repo.create(&input("forged", Some(company_b)), None),
    )
    .await;
    assert!(
        matches!(forged, Err(EventError::Database(_))),
        "cross-tenant claim must be refused, got {forged:?}"
    );

    // The scoped write passes: the company scope reaches every
    // statement (transaction bind + scoped helpers), so the WITH
    // CHECK admits the row.
    let mine = with_company_scope(
        Some(company_a),
        repo.create(&input("company a event", Some(company_a)), None),
    )
    .await
    .unwrap();

    // Reads stay company-fenced: company A's scope sees ONLY its own
    // event — never the other company's rows.
    let visible: Vec<String> = with_company_scope(Some(company_a), repo.list(10))
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(visible, vec!["company a event".to_string()]);

    // Cross-company reach is refused: the other company's row is
    // indistinguishable from a missing one under company A's scope.
    let cross = with_company_scope(Some(company_a), repo.find(seeded.id)).await;
    assert!(
        matches!(cross, Err(EventError::EventNotFound)),
        "cross-company find must not resolve, got {cross:?}"
    );

    // And find() reaches exactly the scoped row.
    let found = with_company_scope(Some(company_a), repo.find(mine.id))
        .await
        .unwrap();
    assert_eq!(found.id, mine.id);

    // Fail-closed read: with NO company scope the fenced pool sees
    // zero rows (never the whole table).
    let unscoped = repo.list(10).await.unwrap();
    assert!(
        unscoped.is_empty(),
        "no scope = zero rows under the fence, saw {}",
        unscoped.len()
    );

    // The other company's row survives everything untouched.
    let intact = with_company_scope(Some(company_b), repo.find(seeded.id))
        .await
        .unwrap();
    assert_eq!(intact.id, seeded.id);

    fenced.close().await;
    db.dispose().await;
}
