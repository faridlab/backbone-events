//! THE TENANCY POSTURE PROBE (ADR-0029) — the module ships NO tenancy
//! of its own: no tenant column, no tenant predicate, and no RLS
//! policy. What it ships instead is the HALF-FENCE the composing
//! service's tenancy decorator completes: `event.events` and
//! `event.registrations` — the two tables the strip freed of their
//! company axis — keep ENABLE + FORCE ROW LEVEL SECURITY with zero
//! policies (the family pattern, proven on backbone-livechat and the
//! batches before it). This probe pins that posture from below:
//!
//! - the flags stay armed on both stripped tables and the module's
//!   policy set is EMPTY (schema pin — a regen that re-adds a policy
//!   would fight the decorator's org-scoped ones);
//! - the `company_id` columns are GONE from both tables;
//! - a plain NOSUPERUSER NOBYPASSRLS role is default-DENIED — zero
//!   rows, writes refused — no matter what legacy variable is set
//!   (no policy reads `app.company_id` anymore; the decorator's
//!   org-scoped policies will, once composed);
//! - the scratch owner is a superuser and BYPASSES row-level
//!   security, so it still sees its own seeded rows plainly: the
//!   denial is the missing policy, not an empty database.
//!
//! Every fenced assertion runs on a pool connected as a NOSUPERUSER
//! NOBYPASSRLS role (the production posture); a green suite run as
//! the scratch owner proves nothing about the fence, so the fence
//! claims never use it.

use uuid::Uuid;

use super::common::{future_window, TestDb};

/// The non-privileged role every fenced connection authenticates as.
const FENCED_ROLE: &str = "event_probe_app";
const FENCED_PASSWORD: &str = "event_probe_app";

/// The two tables the strip migration freed of their company axis —
/// the only tables in the module that ever carried one, and the only
/// two the composing decorator fences.
const STRIPPED_TABLES: [&str; 2] = ["events", "registrations"];

/// Mint the production-shaped app role on the scratch cluster: LOGIN,
/// DML on the module's tables, NO BYPASSRLS. A role from an aborted
/// run can hold grants in a leaked scratch database (DROP ROLE
/// refuses while any depend on it), so stale denial-marker databases
/// go first — the role then drops clean.
async fn fence_role(db: &TestDb) {
    let stale: Vec<String> = sqlx::query_scalar(
        "SELECT datname FROM pg_database \
         WHERE datname LIKE 'event\\_seat\\_posturedeny\\_%' AND datname <> current_database()",
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
async fn fenced_pool(db: &TestDb) -> sqlx::PgPool {
    let opts = db.pool.connect_options();
    let dsn = format!(
        "postgres://{FENCED_ROLE}:{FENCED_PASSWORD}@{}:{}/{}",
        opts.get_host(),
        opts.get_port(),
        opts.get_database().unwrap(),
    );
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&dsn)
        .await
        .unwrap()
}

/// Seed one event over the owner pool (superuser — the setup path
/// migrations and seeders legitimately use) and return its id.
async fn seed_event(db: &TestDb) -> Uuid {
    let (begin, end) = future_window();
    let stage_id: Uuid = sqlx::query_scalar(
        "INSERT INTO event.stages (name) VALUES ('posture probe stage') RETURNING id",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO event.events (name, stage_id, date_begin, date_end) \
         VALUES ('posture probe event', $1, $2, $3) RETURNING id",
    )
    .bind(stage_id)
    .bind(begin)
    .bind(end)
    .fetch_one(&db.pool)
    .await
    .unwrap()
}

// ── The schema pin: armed flags, empty policy set, gone columns ──────────────

/// Both stripped tables carry ENABLE + FORCE ROW LEVEL SECURITY and
/// the module ships ZERO policies — the decorator's half-fence. If a
/// strip or regen ever drops the flags, an undecorated deployment
/// would silently become readable by any role the host grants; if a
/// policy ever reappears module-side, the decorator's org-scoped
/// policies would fight it.
#[tokio::test]
async fn stripped_tables_carry_rls_flags_and_the_module_ships_no_policy() {
    let db = TestDb::new("postureflags").await;

    for table in STRIPPED_TABLES {
        let (armed, forced): (bool, bool) = sqlx::query_as::<_, (bool, bool)>(
            "SELECT c.relrowsecurity, c.relforcerowsecurity \
             FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = 'event' AND c.relname = $1",
        )
        .bind(table)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert!(armed && forced, "event.{table} must stay ENABLE+FORCE RLS");
    }

    let policies: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_policies WHERE schemaname = 'event'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        policies, 0,
        "the module ships no policy — the composing decorator owns the fence"
    );

    db.dispose().await;
}

/// The company axis is GONE from both stripped tables: the column,
/// and any index or constraint that leaned on it.
#[tokio::test]
async fn company_columns_are_gone() {
    let db = TestDb::new("posturecols").await;

    for table in STRIPPED_TABLES {
        let carries: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM information_schema.columns \
             WHERE table_schema = 'event' AND table_name = $1 \
               AND column_name = 'company_id')",
        )
        .bind(table)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert!(
            !carries,
            "event.{table} must not carry a company_id column after the strip"
        );
    }

    db.dispose().await;
}

// ── The default-DENY pin under the production posture ───────────────────────

/// A NOSUPERUSER NOBYPASSRLS role is denied everything on the armed,
/// policyless tables: reads see zero rows, writes are refused — and
/// setting the legacy `app.company_id` variable changes nothing,
/// because no policy reads it anymore. The owner still sees its
/// seeded row: the denial is the missing policy, not an empty
/// database.
#[tokio::test]
async fn plain_role_is_denied_regardless_of_legacy_variable() {
    let db = TestDb::new("posturedeny").await;
    fence_role(&db).await;
    let fenced = fenced_pool(&db).await;

    let event_id = seed_event(&db).await;

    // Owner sanity: the seeded row IS there.
    let owner_sees: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM event.events WHERE id = $1",
    )
    .bind(event_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(owner_sees, 1, "owner must see its own seeded row");

    // Fail-closed read: zero rows, never the whole table.
    let visible: i64 = sqlx::query_scalar("SELECT count(*) FROM event.events")
        .fetch_one(&fenced)
        .await
        .unwrap();
    assert_eq!(visible, 0, "armed + policyless = zero rows, saw {visible}");

    // The legacy variable grants NOTHING: no policy reads it.
    sqlx::query("SELECT set_config('app.company_id', $1, false)")
        .bind(event_id.to_string())
        .execute(&fenced)
        .await
        .unwrap();
    let with_variable: i64 = sqlx::query_scalar("SELECT count(*) FROM event.events")
        .fetch_one(&fenced)
        .await
        .unwrap();
    assert_eq!(
        with_variable, 0,
        "setting the legacy company variable must not open the fence"
    );

    // Fail-closed write: refused at the fence (42501), the exact
    // failure an unscoped statement produces in production. The stage
    // reference is a fresh UUID on purpose — the row must be REJECTED BY
    // THE FENCE, not by shape, so every NOT NULL column carries a value.
    let (begin, end) = future_window();
    let refused = sqlx::query(
        "INSERT INTO event.events (name, stage_id, date_begin, date_end) \
         VALUES ('denied', $1, $2, $3)",
    )
    .bind(Uuid::new_v4())
    .bind(begin)
    .bind(end)
    .execute(&fenced)
    .await;
    let code = refused
        .err()
        .and_then(|e| {
            e.as_database_error()
                .and_then(|d| d.code().map(|c| c.to_string()))
        })
        .unwrap_or_default();
    assert_eq!(
        code, "42501",
        "unscoped write must be refused by the armed policyless fence"
    );

    fenced.close().await;
    db.dispose().await;
}
