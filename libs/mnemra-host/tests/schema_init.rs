//! Integration tests for `mnemra init` schema bootstrap (Task 7).
//!
//! Tests run against the real embedded Postgres engine — no mocks (R-0018-b).
//!
//! # Test coverage
//!
//! - Round-trip: `init()` against a fresh engine yields a working schema.
//! - Idempotency: re-running `init()` is a success no-op.
//! - Negative: bogus extension name → structured `InitError::ExtensionUnavailable`,
//!   no partial schema created.
//! - Introspection: no `timescaledb`, no observability hypertable post-init.
//! - Roles: all four least-privilege roles exist in `pg_roles`.
//! - Health snapshot: `overall: ok` after init.
//! - Default workspace: exists with name `"default"`.
//! - Partition tables: `content` and `state_config` both exist.
//!
//! # Startup serialization
//!
//! A `std::sync::Mutex` serializes engine startup within this binary (A-11
//! cross-binary archive-extraction race note from `postgres_engine.rs` applies
//! here too). Each test starts its own engine instance.
//!
//! # A-11 note
//!
//! `STARTUP_LOCK` here is independent of the locks in `postgres_engine.rs` and
//! `storage_contract_postgres.rs`. Under `cargo test --workspace` (default
//! sequential binary scheduling) there is no race. See `postgres_engine.rs`
//! module doc for the full A-11 analysis.

use mnemra_host::schema::init::{
    DEFAULT_WORKSPACE_ID, HealthStatus, InitError, ROLE_BACKUP, ROLE_HEALTH, ROLE_HOST_FNS,
    ROLE_MIGRATION, health_snapshot, init,
};
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use std::sync::Mutex;

/// Serializes engine startup across concurrent test threads (A-11).
static STARTUP_LOCK: Mutex<()> = Mutex::new(());

/// Start a fresh engine with startup serialized.
async fn start_engine() -> EmbeddedEngine {
    let _guard = STARTUP_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    EmbeddedEngine::start()
        .await
        .expect("failed to start embedded Postgres")
}

// ---------------------------------------------------------------------------
// Round-trip: init yields working schema, health ok
// ---------------------------------------------------------------------------

#[tokio::test]
async fn init_round_trip_health_ok() {
    let engine = start_engine().await;

    init(&engine, "vector")
        .await
        .expect("init should succeed against a fresh engine");

    let snapshot = health_snapshot(engine.pool.as_ref())
        .await
        .expect("health_snapshot should not error after init");

    assert_eq!(
        snapshot.overall,
        HealthStatus::Ok,
        "health overall must be 'ok' after init; got {:?}",
        snapshot.overall
    );
    assert!(snapshot.postgres, "postgres must be reachable");
    assert!(snapshot.pgvector, "pgvector must be loaded");
    assert!(snapshot.workspace_default, "default workspace must exist");
}

// ---------------------------------------------------------------------------
// Idempotency: re-running init is a success no-op
// ---------------------------------------------------------------------------

#[tokio::test]
async fn init_idempotent() {
    let engine = start_engine().await;

    init(&engine, "vector")
        .await
        .expect("first init should succeed");

    // Second run must also succeed (migrations skip applied versions, workspace
    // upsert uses ON CONFLICT DO NOTHING, roles use IF NOT EXISTS).
    init(&engine, "vector")
        .await
        .expect("second init (idempotency) should succeed");
}

// ---------------------------------------------------------------------------
// Negative: bogus extension → structured error, no partial schema
// ---------------------------------------------------------------------------

#[tokio::test]
async fn init_extension_unavailable_returns_structured_error() {
    let engine = start_engine().await;

    let result = init(&engine, "nonexistent_extension_xyz").await;

    match result {
        Err(InitError::ExtensionUnavailable(e)) => {
            assert_eq!(
                e.extension, "nonexistent_extension_xyz",
                "error must name the extension"
            );
            assert!(!e.cause.is_empty(), "error must carry a cause string");

            // Assert no substrate tables were created (init halts on extension failure).
            let table_count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM information_schema.tables
                 WHERE table_schema = 'public'
                 AND table_name IN ('workspaces', 'content', 'state_config')",
            )
            .fetch_one(engine.pool.as_ref())
            .await
            .expect("table introspection query failed");

            assert_eq!(
                table_count.0, 0,
                "no substrate tables should exist after a failed init (extension halts before migrations)"
            );
        }
        other => panic!("expected InitError::ExtensionUnavailable, got: {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Introspection: no timescaledb, no observability hypertable
// ---------------------------------------------------------------------------

#[tokio::test]
async fn init_no_timescaledb_no_hypertable() {
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    // \dx equivalent: timescaledb must NOT be listed.
    let ts_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM pg_extension WHERE extname = 'timescaledb'")
            .fetch_one(engine.pool.as_ref())
            .await
            .expect("pg_extension query failed");
    assert_eq!(
        ts_count.0, 0,
        "timescaledb must not be installed (R-0004-c)"
    );

    // No observability-named tables in public schema.
    let obs_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables
         WHERE table_schema = 'public'
         AND table_name IN ('metrics', 'events', 'observations')",
    )
    .fetch_one(engine.pool.as_ref())
    .await
    .expect("table introspection query failed");
    assert_eq!(
        obs_count.0, 0,
        "no observability hypertables should exist post-init (R-0004-c)"
    );
}

// ---------------------------------------------------------------------------
// Content + state-config partition tables exist (R-0013-b)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn init_creates_content_and_state_config_tables() {
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    // Both tables must exist as regular (non-partitioned) Postgres tables.
    for table in &["content", "state_config"] {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT table_name FROM information_schema.tables
             WHERE table_schema = 'public' AND table_name = $1",
        )
        .bind(*table)
        .fetch_optional(engine.pool.as_ref())
        .await
        .expect("table introspection query failed");

        assert!(
            row.is_some(),
            "table '{}' must exist after init (R-0013-b)",
            table
        );
    }
}

// ---------------------------------------------------------------------------
// Workspaces table exists and default workspace is present
// ---------------------------------------------------------------------------

#[tokio::test]
async fn init_creates_default_workspace() {
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    // Look up by name "default" — must exist.
    let row: Option<(String,)> =
        sqlx::query_as("SELECT name FROM workspaces WHERE name = 'default'")
            .fetch_optional(engine.pool.as_ref())
            .await
            .expect("workspaces query failed");

    assert!(
        row.is_some(),
        "the 'default' workspace must exist after init (R-0015-a, R-0015-h)"
    );

    // Also verifiable by the deterministic UUID constant.
    let id_row: Option<(sqlx::types::Uuid,)> =
        sqlx::query_as("SELECT id FROM workspaces WHERE id = $1")
            .bind(DEFAULT_WORKSPACE_ID)
            .fetch_optional(engine.pool.as_ref())
            .await
            .expect("workspaces id query failed");

    assert!(
        id_row.is_some(),
        "the default workspace must be findable by its deterministic UUID (A-16)"
    );
}

// ---------------------------------------------------------------------------
// Four least-privilege roles exist (R-0013-e)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn init_creates_four_least_privilege_roles() {
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    for role in &[ROLE_HOST_FNS, ROLE_MIGRATION, ROLE_BACKUP, ROLE_HEALTH] {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT rolname FROM pg_roles WHERE rolname = $1")
                .bind(*role)
                .fetch_optional(engine.pool.as_ref())
                .await
                .expect("pg_roles query failed");

        assert!(
            row.is_some(),
            "role '{}' must exist after init (R-0013-e)",
            role
        );

        // Each role must be NOLOGIN (no superuser, no BYPASSRLS).
        let flags: (bool, bool, bool) = sqlx::query_as(
            "SELECT rolsuper, rolbypassrls, rolcanlogin
             FROM pg_roles WHERE rolname = $1",
        )
        .bind(*role)
        .fetch_one(engine.pool.as_ref())
        .await
        .expect("pg_roles flags query failed");

        let (superuser, bypassrls, canlogin) = flags;
        assert!(
            !superuser,
            "role '{}' must not be superuser (least-privilege, R-0013-e)",
            role
        );
        assert!(
            !bypassrls,
            "role '{}' must not have BYPASSRLS (least-privilege, R-0013-e)",
            role
        );
        assert!(
            !canlogin,
            "role '{}' must be NOLOGIN (forward structure; runtime uses mnemra_app)",
            role
        );
    }
}

// ---------------------------------------------------------------------------
// pgvector extension is enabled in pg_extension after init
// ---------------------------------------------------------------------------

#[tokio::test]
async fn init_enables_pgvector_extension() {
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pg_extension WHERE extname = 'vector'")
        .fetch_one(engine.pool.as_ref())
        .await
        .expect("pg_extension query failed");

    assert_eq!(
        row.0, 1,
        "pg_extension must list 'vector' after init (R-0013-a)"
    );
}

// ---------------------------------------------------------------------------
// Migration runner: destructive statement guard unit-tested in migrations module
// (pure, no engine needed) — confirmed by cargo test.
// ---------------------------------------------------------------------------
