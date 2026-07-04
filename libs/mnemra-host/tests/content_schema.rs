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
//! # Engine acquisition
//!
//! Acquisition-migrated onto the shared-engine fixture (T3 sub-run,
//! R-0030/R-0029): each test acquires the binary-wide shared engine via
//! `shared_engine::shared_engine()` and provisions its own fresh, isolated
//! database via `EmbeddedEngine::provision_test_database()` (which already
//! runs the full schema-init sequence — no redundant `init()` call needed).
//! No per-file boot-serialization mutex needed — the fixture's own
//! get-or-init semantics guarantee exactly-once boot.

#[path = "common/shared_engine.rs"]
mod shared_engine;

use mnemra_host::storage::postgres::engine::EmbeddedEngine;

/// The fixture artifact-type name Task 9 must register.
///
/// After schema init completes (now run by
/// `EmbeddedEngine::provision_test_database()`), the schema MUST contain a table
/// named `FIXTURE_TYPE` created by the per-artifact-type table generator.
/// Task 9 wires this registration into the `init()` path.
const FIXTURE_TYPE: &str = "echo_fixture";

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
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

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
        .fetch_optional(&db.pool)
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
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT is_nullable
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = $1
           AND column_name  = 'workspace_id'",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(&db.pool)
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
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT is_nullable
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = $1
           AND column_name  = 'body'",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(&db.pool)
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
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    // pg_indexes exposes the index definition; workspace_id must appear in it.
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT indexname
         FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename  = $1
           AND indexdef   LIKE '%workspace_id%'",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(&db.pool)
    .await
    .expect("pg_indexes workspace_id query failed");

    assert!(
        row.is_some(),
        "R-0001-a: a workspace_id index must exist on table '{}'",
        FIXTURE_TYPE
    );
}

// ---------------------------------------------------------------------------
// T8.1 (R-0001-a) — column TYPES + id-as-PK on `echo_fixture`
//
// The existing R-0001-a tests assert column NAMES and nullability; the AC also
// names concrete column TYPES and that `id` is the primary key. These pin the
// exact types/PK so a DDL type change (e.g. id → uuid, frontmatter → text) or a
// PK change (e.g. composite key) fails the test rather than passing silently.
// ---------------------------------------------------------------------------

/// `id`, `workspace_id`, `frontmatter`, and `frontmatter_version` on
/// `echo_fixture` must have their AC-named SQL types (T8.1, R-0001-a).
///
/// `information_schema.columns.data_type` reports the canonical type name:
/// `text` / `uuid` / `jsonb` / `bigint`. Each is asserted by exact equality;
/// a wrong type (e.g. `character varying` for a `text` column, or `text` for a
/// `jsonb` column) fails the matching assertion.
#[tokio::test]
async fn content_schema_t8_1_artifact_column_types() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    // (column_name, expected information_schema.columns.data_type)
    let expected: &[(&str, &str)] = &[
        ("id", "text"),
        ("workspace_id", "uuid"),
        ("frontmatter", "jsonb"),
        ("frontmatter_version", "bigint"),
    ];

    for (col, expected_type) in expected {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT data_type
             FROM information_schema.columns
             WHERE table_schema = 'public'
               AND table_name   = $1
               AND column_name  = $2",
        )
        .bind(FIXTURE_TYPE)
        .bind(*col)
        .fetch_optional(&db.pool)
        .await
        .expect("information_schema.columns data_type query failed");

        let (data_type,) = row.unwrap_or_else(|| {
            panic!(
                "T8.1 (R-0001-a): column '{}' must exist on table '{}'",
                col, FIXTURE_TYPE
            )
        });

        assert_eq!(
            &data_type, expected_type,
            "T8.1 (R-0001-a): column '{}' on table '{}' must have data_type '{}'; got '{}'",
            col, FIXTURE_TYPE, expected_type, data_type
        );
    }
}

/// The PRIMARY KEY of `echo_fixture` must be exactly the single column `id`
/// (T8.1, R-0001-a).
///
/// Joins `table_constraints` (constraint_type = 'PRIMARY KEY') to
/// `key_column_usage` and asserts the ordered PK column list equals `["id"]`.
/// Asserting the exact set (not "contains id") fails the test if the PK were a
/// composite key such as `(workspace_id, id)`.
#[tokio::test]
async fn content_schema_t8_1_primary_key_is_id_only() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let pk_columns: Vec<(String,)> = sqlx::query_as(
        "SELECT kcu.column_name
         FROM information_schema.table_constraints tc
         JOIN information_schema.key_column_usage kcu
           ON tc.constraint_name = kcu.constraint_name
          AND tc.table_schema    = kcu.table_schema
         WHERE tc.table_schema    = 'public'
           AND tc.table_name      = $1
           AND tc.constraint_type = 'PRIMARY KEY'
         ORDER BY kcu.ordinal_position",
    )
    .bind(FIXTURE_TYPE)
    .fetch_all(&db.pool)
    .await
    .expect("primary-key column query failed");

    let cols: Vec<String> = pk_columns.into_iter().map(|(c,)| c).collect();

    assert_eq!(
        cols,
        vec!["id".to_string()],
        "T8.1 (R-0001-a): the primary key of '{}' must be exactly {{id}}; got {:?}",
        FIXTURE_TYPE,
        cols
    );
}

// ---------------------------------------------------------------------------
// R-0001-b — Dedicated system columns. `migrated_from` and `migrated_at` are
//             dedicated columns absent from the frontmatter JSONB.
//             `frontmatter_version` is a dedicated GENERATED column that
//             projects the authoritative JSONB `frontmatter_version` key
//             (self-describing interchange format; no-drift projection).
// ---------------------------------------------------------------------------

/// `migrated_from` must be a dedicated column, not stored inside `frontmatter`
/// (R-0001-b).
///
/// The existence of a typed column is the schema-level proof of separation.
/// RED: table absent → 0 rows → assertion fails.
#[tokio::test]
async fn content_schema_r0001b_migrated_from_is_dedicated_column() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT column_name, data_type
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = $1
           AND column_name  = 'migrated_from'",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(&db.pool)
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
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT column_name, data_type
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = $1
           AND column_name  = 'migrated_at'",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(&db.pool)
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
/// Under the locked model it is a dedicated GENERATED column projecting the
/// authoritative JSONB `frontmatter_version` key — its existence as a column is
/// what this test pins (the no-drift / self-describing semantics are covered by
/// `content_schema_r0001b_frontmatter_version_column_is_no_drift_projection`).
///
/// RED: table absent → 0 rows → assertion fails.
#[tokio::test]
async fn content_schema_r0001b_frontmatter_version_is_dedicated_column() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT column_name, data_type
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = $1
           AND column_name  = 'frontmatter_version'",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(&db.pool)
    .await
    .expect("information_schema.columns query for frontmatter_version failed");

    assert!(
        row.is_some(),
        "R-0001-b: 'frontmatter_version' must be a dedicated (generated) column \
         on table '{}'; it is absent",
        FIXTURE_TYPE
    );
}

/// The interchange format is self-describing: an artifact whose `frontmatter`
/// JSONB carries `frontmatter_version` inserts successfully, and the value is
/// queryable from the JSONB itself (R-0001-b, R-0001-c).
///
/// This pins the locked rationale — the JSONB is authoritative and carries the
/// version, so the row is complete without any out-of-band column write.
#[tokio::test]
async fn content_schema_r0001b_frontmatter_carries_version_self_describing() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    // INSERT supplying ONLY the JSONB (no explicit frontmatter_version column);
    // the JSONB is the interchange format and carries the version itself.
    // AssertSqlSafe: FIXTURE_TYPE is a crate constant, not user input.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {table} (id, workspace_id, type, frontmatter)
         VALUES (
             'TESTULID00000000010',
             '1b027423-a7e3-54ea-9e35-2e1a4afdf3d9',
             'echo_fixture',
             '{{\"id\": \"TESTULID00000000010\", \"frontmatter_version\": 7}}'
         )",
        table = FIXTURE_TYPE
    )))
    .execute(pool)
    .await
    .expect("R-0001-b: insert with self-describing frontmatter must succeed");

    // The version is queryable from the JSONB (the interchange format carries it).
    let from_jsonb: (i64,) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT (frontmatter->>'frontmatter_version')::bigint
         FROM {table} WHERE id = 'TESTULID00000000010'",
        table = FIXTURE_TYPE
    )))
    .fetch_one(pool)
    .await
    .expect("query frontmatter_version from JSONB");

    assert_eq!(
        from_jsonb.0, 7,
        "R-0001-b: frontmatter_version must be carried by and queryable from the JSONB"
    );
}

/// No-drift: the `frontmatter_version` column is a `GENERATED ALWAYS … STORED`
/// projection of the JSONB key, so it cannot diverge from the JSONB (R-0001-b).
///
/// Two facts pin this:
/// 1. After insert, the column equals `(frontmatter->>'frontmatter_version')::bigint`.
/// 2. An attempt to write a conflicting column value is rejected by Postgres
///    with SQLSTATE 428C9 (cannot insert a non-DEFAULT value into a generated
///    column) — the column structurally cannot be set independently of the JSONB.
#[tokio::test]
async fn content_schema_r0001b_frontmatter_version_column_is_no_drift_projection() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    // Insert a row carrying version 5 in the JSONB.
    // AssertSqlSafe: FIXTURE_TYPE is a crate constant, not user input.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {table} (id, workspace_id, type, frontmatter)
         VALUES (
             'TESTULID00000000011',
             '1b027423-a7e3-54ea-9e35-2e1a4afdf3d9',
             'echo_fixture',
             '{{\"id\": \"TESTULID00000000011\", \"frontmatter_version\": 5}}'
         )",
        table = FIXTURE_TYPE
    )))
    .execute(pool)
    .await
    .expect("insert row with version 5");

    // (1) The generated column tracks the JSONB value exactly.
    let projected: (i64, i64) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT frontmatter_version, (frontmatter->>'frontmatter_version')::bigint
         FROM {table} WHERE id = 'TESTULID00000000011'",
        table = FIXTURE_TYPE
    )))
    .fetch_one(pool)
    .await
    .expect("query projected column and JSONB value");

    assert_eq!(
        projected.0, 5,
        "R-0001-b: generated frontmatter_version column must equal the JSONB version"
    );
    assert_eq!(
        projected.0, projected.1,
        "R-0001-b: generated column must equal (frontmatter->>'frontmatter_version')::bigint"
    );

    // (2) An explicit write to the generated column is structurally rejected —
    // the column cannot be set to a value that diverges from the JSONB.
    let conflicting: Result<_, sqlx::Error> = sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {table} (id, workspace_id, type, frontmatter, frontmatter_version)
         VALUES (
             'TESTULID00000000012',
             '1b027423-a7e3-54ea-9e35-2e1a4afdf3d9',
             'echo_fixture',
             '{{\"id\": \"TESTULID00000000012\", \"frontmatter_version\": 1}}',
             999
         )",
        table = FIXTURE_TYPE
    )))
    .execute(pool)
    .await;

    let code = conflicting
        .expect_err("R-0001-b: explicit write to generated column must be rejected")
        .as_database_error()
        .and_then(|e| e.code())
        .map(|c| c.to_string());

    assert_eq!(
        code.as_deref(),
        Some("428C9"),
        "R-0001-b: explicit non-DEFAULT write to the GENERATED frontmatter_version \
         column must fail with SQLSTATE 428C9 (generated-column write); got {:?}",
        code
    );
}

/// Fail-closed: a non-numeric `frontmatter_version` JSONB value (e.g. `"abc"`)
/// is rejected at insert — the row never lands (R-0001-b projection is total).
///
/// SQLSTATE is `22P02` (invalid_text_representation), NOT the `23514` of a CHECK
/// violation. This is structural, not a choice: a STORED generated column is
/// evaluated BEFORE CHECK constraints, so the locked
/// `(frontmatter->>'frontmatter_version')::bigint` cast throws on the
/// non-numeric value before any CHECK can run (empirically confirmed against
/// the embedded engine). The locked DDL expression is kept verbatim; a
/// friendlier-error form (a regex-guarded CASE that projects NULL so the
/// presence CHECK fires with 23514) is deferred to the input-validation
/// hardening tracked in #1752. This test pins the fail-closed guarantee so a
/// future DDL change cannot silently weaken it (e.g. let a malformed row land).
#[tokio::test]
async fn content_schema_r0001b_nonnumeric_version_is_rejected_fail_closed() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    // `frontmatter_version` is a non-numeric string; the generated ::bigint cast
    // rejects it at insert. (No explicit frontmatter_version column — it is
    // GENERATED.) AssertSqlSafe: FIXTURE_TYPE is a crate constant, not user input.
    let result: Result<_, sqlx::Error> = sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {table} (id, workspace_id, type, frontmatter)
         VALUES (
             'TESTULID00000000013',
             '1b027423-a7e3-54ea-9e35-2e1a4afdf3d9',
             'echo_fixture',
             '{{\"id\": \"TESTULID00000000013\", \"frontmatter_version\": \"abc\"}}'
         )",
        table = FIXTURE_TYPE
    )))
    .execute(pool)
    .await;

    let code = result
        .expect_err("R-0001-b: non-numeric frontmatter_version must be rejected (fail-closed)")
        .as_database_error()
        .and_then(|e| e.code())
        .map(|c| c.to_string());

    assert_eq!(
        code.as_deref(),
        Some("22P02"),
        "R-0001-b: non-numeric frontmatter_version must be rejected at insert with \
         SQLSTATE 22P02 (the GENERATED ::bigint cast throws before any CHECK runs); \
         got {:?}. A different code here means the fail-closed guarantee shifted — \
         friendlier-error hardening is tracked in #1752, not a silent DDL change.",
        code
    );

    // And the row must not have landed.
    let landed: Option<(String,)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT id FROM {table} WHERE id = 'TESTULID00000000013'",
        table = FIXTURE_TYPE
    )))
    .fetch_optional(pool)
    .await
    .expect("existence check after rejected insert");

    assert!(
        landed.is_none(),
        "R-0001-b: a row with a non-numeric frontmatter_version must not be persisted"
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
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    // Frontmatter JSONB has `frontmatter_version` but NOT `id` — must be rejected.
    // `frontmatter_version` is a GENERATED column (R-0001-b projection of the
    // JSONB key), so it is not an explicit INSERT target.
    // AssertSqlSafe: FIXTURE_TYPE is a crate constant, not user input.
    let result: Result<_, sqlx::Error> = sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {table} (id, workspace_id, type, frontmatter)
         VALUES (
             'TESTULID00000000001',
             '1b027423-a7e3-54ea-9e35-2e1a4afdf3d9',
             'echo_fixture',
             '{{\"frontmatter_version\": 1}}'
         )",
        table = FIXTURE_TYPE
    )))
    .execute(&db.pool)
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
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    // Frontmatter JSONB has `id` but NOT `frontmatter_version` — must be rejected
    // by the CHECK (23514). The design relies on there being NO NOT NULL
    // constraint on the generated `frontmatter_version` column: a missing key
    // projects to NULL via the generated expression, so
    // `CHECK (frontmatter ? 'frontmatter_version')` is the sole gate and rejects
    // the row with 23514. (A NOT NULL here would introduce a second possible
    // rejection — 23502 — for the same row; omitting it keeps the CHECK the only
    // gate, which is what this test pins.)
    // AssertSqlSafe: FIXTURE_TYPE is a crate constant, not user input.
    let result: Result<_, sqlx::Error> = sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {table} (id, workspace_id, type, frontmatter)
         VALUES (
             'TESTULID00000000002',
             '1b027423-a7e3-54ea-9e35-2e1a4afdf3d9',
             'echo_fixture',
             '{{\"id\": \"TESTULID00000000002\"}}'
         )",
        table = FIXTURE_TYPE
    )))
    .execute(&db.pool)
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
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT table_name
         FROM information_schema.tables
         WHERE table_schema = 'public'
           AND table_name   = $1",
    )
    .bind(FIXTURE_TYPE)
    .fetch_optional(&db.pool)
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
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT indexname
         FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename  = $1
           AND indexdef   LIKE $2",
    )
    .bind(FIXTURE_TYPE)
    .bind("%(frontmatter ->> 'status'::text)%")
    .fetch_optional(&db.pool)
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
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT indexname
         FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename  = $1
           AND indexdef   LIKE $2",
    )
    .bind(FIXTURE_TYPE)
    .bind("%(frontmatter ->> 'priority'::text)%")
    .fetch_optional(&db.pool)
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
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT indexname
         FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename  = $1
           AND indexdef   LIKE $2",
    )
    .bind(FIXTURE_TYPE)
    .bind("%(frontmatter ->> 'project_id'::text)%")
    .fetch_optional(&db.pool)
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
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT indexname
         FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename  = $1
           AND indexdef   LIKE $2",
    )
    .bind(FIXTURE_TYPE)
    .bind("%(frontmatter ->> 'parent_id'::text)%")
    .fetch_optional(&db.pool)
    .await
    .expect("pg_indexes parent_id expression index query failed");

    assert!(
        row.is_some(),
        "R-0001-d: expression index on (frontmatter->>'parent_id') must exist on table '{}'",
        FIXTURE_TYPE
    );
}
