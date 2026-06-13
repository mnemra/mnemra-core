//! Integration tests for the C1 content-substrate schema (Tasks 8/9 TDD RED phase).
//!
//! # What these tests pin
//!
//! The tests cover exactly R-0001-a through R-0001-d — the per-artifact-type table
//! generator that Task 9 implements.  History shadow tables (R-0001-e) and the
//! refresh queue (R-0001-f) are out-of-scope for this file; Task 9 self-tests those.
//!
//! # How the fixture type is registered (seam contract — read before modifying)
//!
//! These tests call `init(&engine, "vector")` exactly once, then assert the
//! resulting schema state via DB introspection.  **Task 9 must wire a built-in
//! fixture type `echo_fixture` into the `init()` path** so that, after `init()`
//! returns, the public schema contains:
//!
//! - A table named `echo_fixture` with the exact C1 column set (R-0001-a).
//! - The four expression indexes on `(frontmatter->>'status')`,
//!   `(frontmatter->>'priority')`, `(frontmatter->>'project_id')`, and
//!   `(frontmatter->>'parent_id')` (R-0001-d).
//! - The two CHECK constraints: `frontmatter ? 'id'` and
//!   `frontmatter ? 'frontmatter_version'` (R-0001-c).
//!
//! Task 9 implements the generator; these tests MUST NOT be modified to pass.
//! The fixture type name `echo_fixture` is the stable key — Task 9 registers it.
//!
//! # RED-phase rationale
//!
//! `verify` is intentionally empty for this file: tests fail by design until
//! Task 9 creates the artifact-table generator machinery and registers the
//! `echo_fixture` type into the `init()` path.  Failures are runtime assertion
//! failures against a missing `echo_fixture` table — not compile errors in this
//! file.  A compile error in this file is a bug in this file.
//!
//! # Startup serialization
//!
//! A `std::sync::Mutex` serializes engine startup within this binary (A-11
//! cross-binary archive-extraction race note from `postgres_engine.rs` applies
//! here too). Each test starts its own engine instance.

use mnemra_host::schema::init::init;
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use std::sync::Mutex;

/// Serializes engine startup across concurrent test threads (A-11).
static STARTUP_LOCK: Mutex<()> = Mutex::new(());

/// The fixture artifact-type name Task 9 must register.
///
/// After `init(&engine, "vector")` completes, the schema MUST contain a table
/// named `FIXTURE_TYPE` created by the per-artifact-type table generator.
/// Task 9 wires this registration into the `init()` path.
const FIXTURE_TYPE: &str = "echo_fixture";

/// Start a fresh engine with startup serialized.
async fn start_engine() -> EmbeddedEngine {
    let _guard = STARTUP_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    EmbeddedEngine::start()
        .await
        .expect("failed to start embedded Postgres")
}

// ---------------------------------------------------------------------------
// R-0001-a — C1 column set: id, workspace_id, type, frontmatter, body,
//             frontmatter_version, migrated_from, migrated_at,
//             created_at, updated_at
// ---------------------------------------------------------------------------

/// The `echo_fixture` table must expose the full C1 column set (R-0001-a).
///
/// Introspection via `information_schema.columns` after `init()`.
/// RED: table absent → 0 matching rows → assertion fails.
#[tokio::test]
async fn content_schema_r0001a_c1_column_set() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    // Column names required by R-0001-a for every artifact-type table.
    let required_columns: &[&str] = &[
        "id",
        "workspace_id",
        "type",
        "frontmatter",
        "body",
        "frontmatter_version",
        "migrated_from",
        "migrated_at",
        "created_at",
        "updated_at",
    ];

    for col in required_columns {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT column_name
             FROM information_schema.columns
             WHERE table_schema = 'public'
               AND table_name   = $1
               AND column_name  = $2",
        )
        .bind(FIXTURE_TYPE)
        .bind(*col)
        .fetch_optional(engine.pool.as_ref())
        .await
        .expect("information_schema.columns query failed");

        assert!(
            row.is_some(),
            "R-0001-a: column '{}' must exist on table '{}' (C1 column set)",
            col,
            FIXTURE_TYPE
        );
    }
}

/// `workspace_id` on `echo_fixture` must be NOT NULL (R-0001-a).
///
/// RED: table absent → 0 rows → assertion fails.
#[tokio::test]
async fn content_schema_r0001a_workspace_id_not_null() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT is_nullable
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = $1
           AND column_name  = 'workspace_id'",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("information_schema.columns nullability query failed");

    assert!(
        row.is_some(),
        "R-0001-a: column 'workspace_id' must exist on table '{}'",
        FIXTURE_TYPE
    );
    let (is_nullable,) = row.unwrap();
    assert_eq!(
        is_nullable, "NO",
        "R-0001-a: 'workspace_id' must be NOT NULL on table '{}'",
        FIXTURE_TYPE
    );
}

/// `body` on `echo_fixture` must be nullable (R-0001-a).
///
/// RED: table absent → 0 rows → assertion fails.
#[tokio::test]
async fn content_schema_r0001a_body_nullable() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT is_nullable
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = $1
           AND column_name  = 'body'",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("information_schema.columns nullability query failed");

    assert!(
        row.is_some(),
        "R-0001-a: column 'body' must exist on table '{}'",
        FIXTURE_TYPE
    );
    let (is_nullable,) = row.unwrap();
    assert_eq!(
        is_nullable, "YES",
        "R-0001-a: 'body' must be nullable on table '{}'",
        FIXTURE_TYPE
    );
}

/// `workspace_id` on `echo_fixture` must have a covering index (R-0001-a).
///
/// Workspace-scoped queries are a primary access pattern; the index is
/// structural, not advisory.
///
/// RED: table absent → 0 matching indexes → assertion fails.
#[tokio::test]
async fn content_schema_r0001a_workspace_id_indexed() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    // pg_indexes exposes the index definition; workspace_id must appear in it.
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT indexname
         FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename  = $1
           AND indexdef   LIKE '%workspace_id%'",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("pg_indexes workspace_id query failed");

    assert!(
        row.is_some(),
        "R-0001-a: a workspace_id index must exist on table '{}'",
        FIXTURE_TYPE
    );
}

// ---------------------------------------------------------------------------
// R-0001-b — Dedicated system columns: migrated_from, migrated_at,
//             frontmatter_version are NOT inside the frontmatter JSONB
// ---------------------------------------------------------------------------

/// `migrated_from` must be a dedicated column, not stored inside `frontmatter`
/// (R-0001-b).
///
/// The existence of a typed column is the schema-level proof of separation.
/// RED: table absent → 0 rows → assertion fails.
#[tokio::test]
async fn content_schema_r0001b_migrated_from_is_dedicated_column() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT column_name, data_type
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = $1
           AND column_name  = 'migrated_from'",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("information_schema.columns query for migrated_from failed");

    assert!(
        row.is_some(),
        "R-0001-b: 'migrated_from' must be a dedicated column (not JSONB-embedded) \
         on table '{}'; it is absent",
        FIXTURE_TYPE
    );
}

/// `migrated_at` must be a dedicated column (R-0001-b).
///
/// RED: table absent → 0 rows → assertion fails.
#[tokio::test]
async fn content_schema_r0001b_migrated_at_is_dedicated_column() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT column_name, data_type
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = $1
           AND column_name  = 'migrated_at'",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("information_schema.columns query for migrated_at failed");

    assert!(
        row.is_some(),
        "R-0001-b: 'migrated_at' must be a dedicated column (not JSONB-embedded) \
         on table '{}'; it is absent",
        FIXTURE_TYPE
    );
}

/// `frontmatter_version` must be a dedicated column (R-0001-b).
///
/// RED: table absent → 0 rows → assertion fails.
#[tokio::test]
async fn content_schema_r0001b_frontmatter_version_is_dedicated_column() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT column_name, data_type
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = $1
           AND column_name  = 'frontmatter_version'",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("information_schema.columns query for frontmatter_version failed");

    assert!(
        row.is_some(),
        "R-0001-b: 'frontmatter_version' must be a dedicated column (not JSONB-embedded) \
         on table '{}'; it is absent",
        FIXTURE_TYPE
    );
}

// ---------------------------------------------------------------------------
// R-0001-c — CHECK constraints: frontmatter must contain 'id' and
//             'frontmatter_version' keys; violating inserts must be rejected
// ---------------------------------------------------------------------------

/// INSERT with frontmatter missing the `id` key must be rejected with a CHECK
/// violation (SQLSTATE 23514) (R-0001-c).
///
/// RED: table absent → INSERT fails with SQLSTATE 42P01 (undefined_table) →
/// the assertion that the error code is 23514 fails.
/// GREEN: table present with CHECK constraint → INSERT fails with 23514 → pass.
///
/// Checking for SQLSTATE 23514 is load-bearing: it distinguishes CHECK-rejection
/// (green semantic) from table-absent (red semantic).  A bare `.is_err()` would
/// accidentally pass today because `42P01` is also an error.
#[tokio::test]
async fn content_schema_r0001c_check_rejects_frontmatter_missing_id() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    // Frontmatter JSONB has `frontmatter_version` but NOT `id` — must be rejected.
    // AssertSqlSafe: FIXTURE_TYPE is a crate constant, not user input.
    let result: Result<_, sqlx::Error> = sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {table} (id, workspace_id, type, frontmatter, frontmatter_version)
         VALUES (
             'TESTULID00000000001',
             '1b027423-a7e3-54ea-9e35-2e1a4afdf3d9',
             'echo_fixture',
             '{{\"frontmatter_version\": 1}}',
             1
         )",
        table = FIXTURE_TYPE
    )))
    .execute(engine.pool.as_ref())
    .await;

    assert!(
        result.is_err(),
        "R-0001-c: INSERT with frontmatter missing 'id' must fail"
    );

    let err = result.unwrap_err();
    let code = err
        .as_database_error()
        .and_then(|e| e.code())
        .map(|c| c.to_string());

    assert_eq!(
        code.as_deref(),
        Some("23514"),
        "R-0001-c: INSERT with frontmatter missing 'id' must fail with SQLSTATE 23514 \
         (check_violation); got {:?} — if this is 42P01 the table does not exist yet \
         (expected in RED phase)",
        code
    );
}

/// INSERT with frontmatter missing the `frontmatter_version` key must be
/// rejected with SQLSTATE 23514 (R-0001-c).
///
/// RED: table absent → INSERT fails with SQLSTATE 42P01 → assertion for 23514 fails.
/// GREEN: table present with CHECK constraint → 23514 → pass.
#[tokio::test]
async fn content_schema_r0001c_check_rejects_frontmatter_missing_frontmatter_version() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    // Frontmatter JSONB has `id` but NOT `frontmatter_version` — must be rejected.
    // AssertSqlSafe: FIXTURE_TYPE is a crate constant, not user input.
    let result: Result<_, sqlx::Error> = sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {table} (id, workspace_id, type, frontmatter, frontmatter_version)
         VALUES (
             'TESTULID00000000002',
             '1b027423-a7e3-54ea-9e35-2e1a4afdf3d9',
             'echo_fixture',
             '{{\"id\": \"TESTULID00000000002\"}}',
             1
         )",
        table = FIXTURE_TYPE
    )))
    .execute(engine.pool.as_ref())
    .await;

    assert!(
        result.is_err(),
        "R-0001-c: INSERT with frontmatter missing 'frontmatter_version' must fail"
    );

    let code = result
        .unwrap_err()
        .as_database_error()
        .and_then(|e| e.code())
        .map(|c| c.to_string());

    assert_eq!(
        code.as_deref(),
        Some("23514"),
        "R-0001-c: INSERT with frontmatter missing 'frontmatter_version' must fail with \
         SQLSTATE 23514 (check_violation); got {:?} — if this is 42P01 the table does not \
         exist yet (expected in RED phase)",
        code
    );
}

// ---------------------------------------------------------------------------
// R-0001-d — Per-artifact-type tables (non-polymorphic) + expression indexes
// ---------------------------------------------------------------------------

/// A table named `echo_fixture` must exist after `init()` (R-0001-d).
///
/// The table's existence under its type name (not `content`) is the schema-level
/// proof of the per-artifact-type (non-polymorphic) layout requirement.
///
/// RED: table absent → 0 rows → assertion fails.
#[tokio::test]
async fn content_schema_r0001d_per_type_table_exists() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT table_name
         FROM information_schema.tables
         WHERE table_schema = 'public'
           AND table_name   = $1",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("information_schema.tables query failed");

    assert!(
        row.is_some(),
        "R-0001-d: per-artifact-type table '{}' must exist after init() \
         (non-polymorphic layout; this table is created by the Task 9 generator)",
        FIXTURE_TYPE
    );
}

/// Expression index on `(frontmatter->>'status')` must exist on `echo_fixture`
/// (R-0001-d).
///
/// RED: table or index absent → 0 rows → assertion fails.
#[tokio::test]
async fn content_schema_r0001d_expression_index_status() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT indexname
         FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename  = $1
           AND indexdef   LIKE $2",
    )
    .bind(FIXTURE_TYPE)
    .bind("%(frontmatter ->> 'status'::text)%")
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("pg_indexes status expression index query failed");

    assert!(
        row.is_some(),
        "R-0001-d: expression index on (frontmatter->>'status') must exist on table '{}'",
        FIXTURE_TYPE
    );
}

/// Expression index on `(frontmatter->>'priority')` must exist on `echo_fixture`
/// (R-0001-d).
///
/// RED: table or index absent → 0 rows → assertion fails.
#[tokio::test]
async fn content_schema_r0001d_expression_index_priority() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT indexname
         FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename  = $1
           AND indexdef   LIKE $2",
    )
    .bind(FIXTURE_TYPE)
    .bind("%(frontmatter ->> 'priority'::text)%")
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("pg_indexes priority expression index query failed");

    assert!(
        row.is_some(),
        "R-0001-d: expression index on (frontmatter->>'priority') must exist on table '{}'",
        FIXTURE_TYPE
    );
}

/// Expression index on `(frontmatter->>'project_id')` must exist on `echo_fixture`
/// (R-0001-d).
///
/// RED: table or index absent → 0 rows → assertion fails.
#[tokio::test]
async fn content_schema_r0001d_expression_index_project_id() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT indexname
         FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename  = $1
           AND indexdef   LIKE $2",
    )
    .bind(FIXTURE_TYPE)
    .bind("%(frontmatter ->> 'project_id'::text)%")
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("pg_indexes project_id expression index query failed");

    assert!(
        row.is_some(),
        "R-0001-d: expression index on (frontmatter->>'project_id') must exist on table '{}'",
        FIXTURE_TYPE
    );
}

/// Expression index on `(frontmatter->>'parent_id')` must exist on `echo_fixture`
/// (R-0001-d).
///
/// RED: table or index absent → 0 rows → assertion fails.
#[tokio::test]
async fn content_schema_r0001d_expression_index_parent_id() {
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT indexname
         FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename  = $1
           AND indexdef   LIKE $2",
    )
    .bind(FIXTURE_TYPE)
    .bind("%(frontmatter ->> 'parent_id'::text)%")
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("pg_indexes parent_id expression index query failed");

    assert!(
        row.is_some(),
        "R-0001-d: expression index on (frontmatter->>'parent_id') must exist on table '{}'",
        FIXTURE_TYPE
    );
}
