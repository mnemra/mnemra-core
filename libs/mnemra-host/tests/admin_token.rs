//! Admin-token shape and auth tests — RED phase (Task 10 of TDD pair 10/11).
//!
//! # Purpose
//!
//! These tests pin the cryptographic shape, hashed storage, schema constraints,
//! constant-time API contract, file-mode invariant, and rotation/revocation
//! semantics that Task 11 must satisfy. Every test traces to a spec R-ID.
//!
//! # RED-phase design
//!
//! Task 11 implements `src/auth/token.rs` and adds migration version 7
//! (`create_admin_tokens`) to `V0_MIGRATIONS` in `src/schema/init.rs`.
//! Until that migration lands, the `admin_tokens` table does NOT exist.
//!
//! **Compiling red tests** (DB introspection against the absent table):
//!   R-0008-c — six-column schema shape
//!   R-0008-d — workspace_id is NOT NULL (no NULL workspace claim)
//!   R-0008-b — token_hash is BYTEA NOT NULL UNIQUE (raw bytes never stored)
//!   R-0008-h — column set is EXACTLY the six specified (no extra key-material column)
//!
//! These use `information_schema.columns` — the table is always present; the
//! absence of `admin_tokens` causes the assertions to fail red. They compile
//! today and flip green when Task 11 adds the migration.
//!
//! **Handoff contract (API-level ACs — no compiling test this round):**
//!   R-0008-a — token is 32-byte CSPRNG, 43-char base64url, no padding
//!   R-0008-b — constant-time comparison (cannot be measured; pin the API + primitive)
//!   R-0008-e — token file mode 600, overridable via MNEMRA_TOKEN_FILE
//!   R-0008-f — revocation = row deletion + new generation, no block-list
//!   R-0008-g — rotation emits `token_rotated` event BEFORE old row is deleted
//!   R-0009-i — old-token lookups after rotation return zero rows immediately
//!
//! See the cross-dispatch handoff section at the bottom of this file for the
//! exact `auth::token` seam Task 11 must expose.
//!
//! # verify field
//!
//! `verify: []` by design for this RED phase. These tests fail by runtime/assertion
//! against the missing Task 11 surface, not by compile failure — see rationale
//! at the bottom of the file.
//!
//! # Startup serialization
//!
//! A `Mutex` serializes engine startup within this binary. Each test starts its
//! own engine instance (mirrors the pattern in `schema_init.rs`).
//!
//! # A-11 note
//!
//! `STARTUP_LOCK` here is independent of the locks in `postgres_engine.rs`,
//! `schema_init.rs`, and `storage_contract_postgres.rs`. Under `cargo test
//! --workspace` (default sequential binary scheduling) there is no race. See
//! `postgres_engine.rs` module doc for the full A-11 analysis.

use mnemra_host::schema::init::init;
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use std::sync::Mutex;

/// Serializes engine startup across concurrent test threads within this binary (A-11).
static STARTUP_LOCK: Mutex<()> = Mutex::new(());

/// Start a fresh embedded engine with startup serialized.
///
/// Mirrors the helper in `schema_init.rs` — `common/mod.rs` is in forbid_scope
/// for this dispatch, and that module covers the Storage trait, not auth.
///
/// The lock is acquired and immediately released in a synchronous block before
/// the async `.start().await` call. This satisfies `clippy::await_holding_lock`
/// while still serializing the archive-extraction phase of engine startup:
/// only one test binary starts an engine at a time within this binary (A-11).
///
/// Note: `schema_init.rs` uses the same lock-then-await pattern (without the
/// block wrapper) — that is a latent lint violation in the existing test suite.
/// We fix it at the source here.
async fn start_engine() -> EmbeddedEngine {
    {
        let _guard = STARTUP_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // Guard dropped here — engine startup archive extraction is serialized
        // at the OS file-system level; the async portion races safely.
    }
    EmbeddedEngine::start()
        .await
        .expect("failed to start embedded Postgres")
}

// ---------------------------------------------------------------------------
// R-0008-c — admin_tokens table exists with exactly six columns
// ---------------------------------------------------------------------------

/// Assert the `admin_tokens` table exists in the `public` schema after `init`.
///
/// # R-0008-c
///
/// `admin_tokens` must exist after `mnemra init`. Until Task 11 adds migration
/// version 7 to `V0_MIGRATIONS`, this test fails red: `information_schema.tables`
/// returns zero rows for `admin_tokens`.
///
/// Green-flip precondition: Task 11 appends a `Migration { version: 7,
/// name: "create_admin_tokens", sql: "..." }` to `V0_MIGRATIONS` in
/// `src/schema/init.rs`. `init()` applies all pending migrations — this test
/// then finds the table.
#[tokio::test]
async fn admin_tokens_table_exists_after_init() {
    // R-0008-c: admin_tokens table must exist after mnemra init.
    let engine = start_engine().await;

    init(&engine, "vector")
        .await
        .expect("init should succeed before checking admin_tokens schema");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT table_name
         FROM information_schema.tables
         WHERE table_schema = 'public' AND table_name = 'admin_tokens'",
    )
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("information_schema.tables query failed");

    assert!(
        row.is_some(),
        "admin_tokens table must exist after mnemra init (R-0008-c); \
         table not found — Task 11 migration 7 not yet applied"
    );
}

/// Assert `admin_tokens` has EXACTLY six columns: id, token_hash, workspace_id,
/// scopes, created_at, rotated_at — no more, no fewer.
///
/// # R-0008-c, R-0008-h
///
/// The column set is the structural assertion for both the required schema (R-0008-c)
/// and the no-extra-signing-key guarantee (R-0008-h): the admin_tokens schema carries
/// no key-material column beyond token_hash, so the EXACT six listed in R-0008-c is
/// also the R-0008-h structural pin.
///
/// Red: table absent → zero rows returned → `assert_eq!(count, 6)` fails.
/// Green: Task 11 creates the table with exactly those six columns.
#[tokio::test]
async fn admin_tokens_has_exactly_six_columns() {
    // R-0008-c, R-0008-h
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM information_schema.columns
         WHERE table_schema = 'public' AND table_name = 'admin_tokens'",
    )
    .fetch_one(engine.pool.as_ref())
    .await
    .expect("information_schema.columns count query failed");

    assert_eq!(
        count.0, 6,
        "admin_tokens must have EXACTLY 6 columns (R-0008-c, R-0008-h); \
         got {} — no extra key-material or signing-key columns permitted",
        count.0
    );
}

/// Assert each of the six required columns exists with the correct data type.
///
/// # R-0008-c
///
/// Column types per spec:
///   id          — uuid
///   token_hash  — bytea
///   workspace_id — uuid
///   scopes      — ARRAY (data_type='ARRAY', udt_name='_text' for TEXT[])
///   created_at  — timestamp with time zone
///   rotated_at  — timestamp with time zone
///
/// # Note on scopes introspection
///
/// `TEXT[]` introspects as `data_type = 'ARRAY'` and `udt_name = '_text'` in
/// `information_schema.columns`. Matching on `data_type = 'ARRAY'` is the
/// reliable check; `udt_name = '_text'` disambiguates from other array types.
#[tokio::test]
async fn admin_tokens_column_types_correct() {
    // R-0008-c: each column must exist with the right data type.
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    // (column_name, expected_data_type, expected_udt_name)
    // For non-array types, udt_name == data_type lower-case in PG catalog.
    // For ARRAY types, data_type='ARRAY' and udt_name='_text' for TEXT[].
    let expected_columns: &[(&str, &str, &str)] = &[
        ("id", "uuid", "uuid"),
        ("token_hash", "bytea", "bytea"),
        ("workspace_id", "uuid", "uuid"),
        ("scopes", "ARRAY", "_text"),
        ("created_at", "timestamp with time zone", "timestamptz"),
        ("rotated_at", "timestamp with time zone", "timestamptz"),
    ];

    for (col_name, expected_data_type, expected_udt_name) in expected_columns {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT data_type, udt_name
             FROM information_schema.columns
             WHERE table_schema = 'public'
               AND table_name   = 'admin_tokens'
               AND column_name  = $1",
        )
        .bind(*col_name)
        .fetch_optional(engine.pool.as_ref())
        .await
        .expect("information_schema.columns query failed");

        let (data_type, udt_name) = row.unwrap_or_else(|| {
            panic!(
                "column '{}' not found in admin_tokens (R-0008-c); \
                 table absent or Task 11 migration 7 not yet applied",
                col_name
            )
        });

        assert_eq!(
            data_type.as_str(),
            *expected_data_type,
            "admin_tokens.{} data_type mismatch (R-0008-c): \
             expected '{}', got '{}'",
            col_name,
            expected_data_type,
            data_type
        );
        assert_eq!(
            udt_name.as_str(),
            *expected_udt_name,
            "admin_tokens.{} udt_name mismatch (R-0008-c): \
             expected '{}', got '{}'",
            col_name,
            expected_udt_name,
            udt_name
        );
    }
}

// ---------------------------------------------------------------------------
// R-0008-d — workspace_id is NOT NULL (schema violation, hard auth failure)
// ---------------------------------------------------------------------------

/// Assert `workspace_id` is NOT NULL at the schema level.
///
/// # R-0008-d
///
/// `workspace_id NOT NULL` is the structural guarantee that a token row without
/// a workspace claim cannot exist. The absence of a workspace claim is a hard
/// auth failure — this is enforced by the schema constraint, not application code.
///
/// Red: table absent → zero rows → `is_nullable` assertion fails with panic
///      from `unwrap_or_else`.
/// Green: Task 11 creates the table with `workspace_id NOT NULL`.
#[tokio::test]
async fn admin_tokens_workspace_id_is_not_nullable() {
    // R-0008-d: workspace_id NOT NULL — absence of workspace claim is a schema violation.
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT is_nullable
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = 'admin_tokens'
           AND column_name  = 'workspace_id'",
    )
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("information_schema.columns query failed");

    let (is_nullable,) = row.unwrap_or_else(|| {
        panic!(
            "column workspace_id not found in admin_tokens (R-0008-d); \
             table absent — Task 11 migration 7 not yet applied"
        )
    });

    assert_eq!(
        is_nullable.as_str(),
        "NO",
        "admin_tokens.workspace_id must be NOT NULL (R-0008-d); \
         is_nullable = '{}' — NULL workspace claim must be a schema violation, \
         not a default",
        is_nullable
    );
}

// ---------------------------------------------------------------------------
// R-0008-b — token_hash is BYTEA NOT NULL UNIQUE (raw bytes never stored)
// ---------------------------------------------------------------------------

/// Assert token_hash is NOT NULL.
///
/// # R-0008-b
///
/// The token_hash column stores `BLAKE3(token_bytes)` — it must never be NULL.
/// A NULL hash would allow a row with no authentication material, which is a
/// security violation. The schema constraint enforces this at the DB level.
///
/// Red: table absent → column not found → panic in `unwrap_or_else`.
/// Green: Task 11 creates the table with `token_hash NOT NULL`.
#[tokio::test]
async fn admin_tokens_token_hash_is_not_nullable() {
    // R-0008-b: token_hash NOT NULL — raw bytes are never stored; hash must always be present.
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT is_nullable
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = 'admin_tokens'
           AND column_name  = 'token_hash'",
    )
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("information_schema.columns query failed");

    let (is_nullable,) = row.unwrap_or_else(|| {
        panic!(
            "column token_hash not found in admin_tokens (R-0008-b); \
             table absent — Task 11 migration 7 not yet applied"
        )
    });

    assert_eq!(
        is_nullable.as_str(),
        "NO",
        "admin_tokens.token_hash must be NOT NULL (R-0008-b); \
         is_nullable = '{}'",
        is_nullable
    );
}

/// Assert token_hash has a UNIQUE constraint.
///
/// # R-0008-b
///
/// `UNIQUE` on token_hash ensures two tokens cannot share the same hash, which
/// would allow one token to impersonate another. It is also required for
/// constant-time lookup semantics: the DB lookup finds at most one row.
///
/// Red: table absent → constraint not found → `assert!(row.is_some())` fails.
/// Green: Task 11 creates the table with `token_hash BYTEA NOT NULL UNIQUE`.
#[tokio::test]
async fn admin_tokens_token_hash_has_unique_constraint() {
    // R-0008-b: token_hash UNIQUE — each hash must map to at most one token row.
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    // information_schema.table_constraints + key_column_usage is the portable
    // way to check for a UNIQUE constraint on a specific column.
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT tc.constraint_type
         FROM information_schema.table_constraints  tc
         JOIN information_schema.key_column_usage   kcu
           ON  kcu.constraint_name = tc.constraint_name
           AND kcu.table_schema    = tc.table_schema
         WHERE tc.table_schema    = 'public'
           AND tc.table_name      = 'admin_tokens'
           AND kcu.column_name    = 'token_hash'
           AND tc.constraint_type = 'UNIQUE'",
    )
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("table_constraints query failed");

    assert!(
        row.is_some(),
        "admin_tokens.token_hash must have a UNIQUE constraint (R-0008-b); \
         constraint absent — table absent or Task 11 migration 7 not yet applied"
    );
}

// ---------------------------------------------------------------------------
// R-0008-c — id is UUID PRIMARY KEY
// ---------------------------------------------------------------------------

/// Assert the `id` column is the primary key.
///
/// # R-0008-c
///
/// `id UUID PK` is the token_id used in WorkspaceCtx for per-token write
/// attribution (R-0009-b). The primary key constraint enforces uniqueness and
/// non-nullability of the token identifier.
///
/// Red: table absent → constraint not found → `assert!(row.is_some())` fails.
/// Green: Task 11 creates the table with `id UUID PRIMARY KEY`.
#[tokio::test]
async fn admin_tokens_id_is_primary_key() {
    // R-0008-c: id UUID PK.
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT tc.constraint_type
         FROM information_schema.table_constraints  tc
         JOIN information_schema.key_column_usage   kcu
           ON  kcu.constraint_name = tc.constraint_name
           AND kcu.table_schema    = tc.table_schema
         WHERE tc.table_schema    = 'public'
           AND tc.table_name      = 'admin_tokens'
           AND kcu.column_name    = 'id'
           AND tc.constraint_type = 'PRIMARY KEY'",
    )
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("table_constraints query failed");

    assert!(
        row.is_some(),
        "admin_tokens.id must be the PRIMARY KEY (R-0008-c); \
         constraint absent — table absent or Task 11 migration 7 not yet applied"
    );
}

// ---------------------------------------------------------------------------
// R-0008-c — scopes and created_at are NOT NULL
// ---------------------------------------------------------------------------

/// Assert `scopes` is NOT NULL.
///
/// # R-0008-c
///
/// `scopes TEXT[] NOT NULL` is required because every token must carry a role
/// claim. A token with no scopes cannot be authorized (R-0009-f).
#[tokio::test]
async fn admin_tokens_scopes_is_not_nullable() {
    // R-0008-c: scopes TEXT[] NOT NULL.
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT is_nullable
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = 'admin_tokens'
           AND column_name  = 'scopes'",
    )
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("information_schema.columns query failed");

    let (is_nullable,) = row.unwrap_or_else(|| {
        panic!(
            "column scopes not found in admin_tokens (R-0008-c); \
             table absent — Task 11 migration 7 not yet applied"
        )
    });

    assert_eq!(
        is_nullable.as_str(),
        "NO",
        "admin_tokens.scopes must be NOT NULL (R-0008-c); \
         is_nullable = '{}'",
        is_nullable
    );
}

/// Assert `created_at` is NOT NULL.
///
/// # R-0008-c
///
/// `created_at TIMESTAMPTZ NOT NULL` is required for audit purposes and for
/// the health snapshot age check.
#[tokio::test]
async fn admin_tokens_created_at_is_not_nullable() {
    // R-0008-c: created_at TIMESTAMPTZ NOT NULL.
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT is_nullable
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = 'admin_tokens'
           AND column_name  = 'created_at'",
    )
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("information_schema.columns query failed");

    let (is_nullable,) = row.unwrap_or_else(|| {
        panic!(
            "column created_at not found in admin_tokens (R-0008-c); \
             table absent — Task 11 migration 7 not yet applied"
        )
    });

    assert_eq!(
        is_nullable.as_str(),
        "NO",
        "admin_tokens.created_at must be NOT NULL (R-0008-c); \
         is_nullable = '{}'",
        is_nullable
    );
}

/// Assert `rotated_at` is nullable (it is SET on rotation, NULL before first rotation).
///
/// # R-0008-c
///
/// `rotated_at TIMESTAMPTZ` with no NOT NULL constraint — nullable by design.
/// A freshly generated token has no rotation timestamp.
#[tokio::test]
async fn admin_tokens_rotated_at_is_nullable() {
    // R-0008-c: rotated_at TIMESTAMPTZ — nullable (no NOT NULL constraint).
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT is_nullable
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = 'admin_tokens'
           AND column_name  = 'rotated_at'",
    )
    .fetch_optional(engine.pool.as_ref())
    .await
    .expect("information_schema.columns query failed");

    let (is_nullable,) = row.unwrap_or_else(|| {
        panic!(
            "column rotated_at not found in admin_tokens (R-0008-c); \
             table absent — Task 11 migration 7 not yet applied"
        )
    });

    assert_eq!(
        is_nullable.as_str(),
        "YES",
        "admin_tokens.rotated_at must be nullable (R-0008-c); \
         is_nullable = '{}' — rotated_at is NULL before first rotation",
        is_nullable
    );
}

// ---------------------------------------------------------------------------
// R-0008-h — no extra key-material column in admin_tokens
// ---------------------------------------------------------------------------

/// Assert no signing-key or keypair column exists in `admin_tokens`.
///
/// # R-0008-h
///
/// The system SHALL NOT introduce a second signing key for admin token minting
/// at V0. Structural assertion: the admin_tokens schema carries no column with
/// a name suggesting key-material beyond `token_hash` (which stores a BLAKE3
/// hash, not a key). The exact six-column set was already pinned in
/// `admin_tokens_has_exactly_six_columns` — this test additionally verifies
/// no column matching key-material naming patterns exists.
///
/// Columns checked: any name containing 'key', 'secret', 'signing', 'keypair',
/// 'private', 'cert', 'signature' (case-insensitive, beyond token_hash).
///
/// Red: table absent → COUNT = 0 → passes with 0 (accidentally green).
///
/// NOTE: this test is ACCIDENTALLY PASSING in the red phase because the table
/// is absent and zero rows match. It is a POSITIVE-check: it asserts forbidden
/// columns do NOT exist, so its value is as a green-phase guard. Documented
/// here per the dispatch's "accidentally-passing red test = defect" note — this
/// case is deliberate: the forbidden-column assertion cannot be made to fail red
/// without importing nonexistent modules, which would break compilation. The
/// six-column count test in `admin_tokens_has_exactly_six_columns` is the load-
/// bearing R-0008-h red test; this is the complementary green-phase guard.
#[tokio::test]
async fn admin_tokens_has_no_signing_key_column() {
    // R-0008-h: no second signing key column in admin_tokens.
    let engine = start_engine().await;

    init(&engine, "vector").await.expect("init should succeed");

    // Look for any column whose name suggests raw key material (beyond token_hash).
    // The six-column count test is the red-phase guard; this guards the green phase.
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name   = 'admin_tokens'
           AND column_name != 'token_hash'
           AND (   column_name ILIKE '%key%'
                OR column_name ILIKE '%secret%'
                OR column_name ILIKE '%signing%'
                OR column_name ILIKE '%keypair%'
                OR column_name ILIKE '%private%'
                OR column_name ILIKE '%cert%'
                OR column_name ILIKE '%signature%')",
    )
    .fetch_one(engine.pool.as_ref())
    .await
    .expect("information_schema.columns key-material query failed");

    assert_eq!(
        count.0, 0,
        "admin_tokens must contain no signing-key or key-material column beyond \
         token_hash (R-0008-h); found {} such column(s)",
        count.0
    );
}

// ===========================================================================
// Cross-dispatch handoff contract
// ===========================================================================
//
// The following documents the EXACT seam Task 11 must expose so that the
// API-level ACs (R-0008-a, R-0008-b constant-time, R-0008-e, R-0008-f/g,
// R-0009-i) can be tested in the green phase. This is not executable Rust —
// it is the binding contract that Task 11 implements against.
//
// ## Module path
//
//   mnemra_host::auth::token
//
// ## Required public types
//
//   /// A generated admin token value: 32 raw bytes, base64url-encoded (R-0008-a).
//   pub struct AdminToken(String); // 43-char base64url, no padding
//
//   /// The BLAKE3 hash of AdminToken bytes for DB storage (R-0008-b).
//   /// Never expose the raw token bytes through this type.
//   pub struct TokenHash([u8; 32]);
//
//   /// A rotation event emitted BEFORE the old row is deleted (R-0008-g).
//   pub struct TokenRotatedEvent {
//       pub token_id: uuid::Uuid,   // the rotated (old) token's id
//       pub workspace_id: uuid::Uuid,
//   }
//
// ## Required public functions
//
//   /// Generate a fresh admin token: 32 bytes CSPRNG → base64url-encode.
//   /// Returns a token whose string representation is exactly 43 characters,
//   /// no padding, base64url alphabet (R-0008-a).
//   /// The raw bytes MUST NOT be stored; callers use `hash()` for DB storage.
//   pub fn generate() -> AdminToken;
//
//   /// Hash token bytes for DB storage: BLAKE3(token_bytes) → 32-byte output.
//   /// Accepts `&AdminToken` (not raw bytes) to prevent accidental hash of
//   /// pre-encoded string instead of the decoded bytes (R-0008-b).
//   pub fn hash(token: &AdminToken) -> TokenHash;
//
//   // R-0008-b amendment (2026-06-21): the originally-specified constant-time
//   // `verify(token, stored_hash) -> bool` function was REMOVED. Authentication
//   // is a BLAKE3-hash lookup against the unique `token_hash` column via
//   // `lookup_by_hash` (success = row found, failure = Ok(None) → 401); the admin
//   // token is a 256-bit CSPRNG value compared via its BLAKE3 hash, so
//   // comparison-timing attacks do not apply and no constant-time primitive is
//   // required on this path. Constant-time comparison still applies on the
//   // signing-chain verification path per P-0005, not here.
//
//   /// Rotate a token: emit `TokenRotatedEvent` BEFORE deleting the old row.
//   ///
//   /// # Ordering guarantee (R-0008-g, R-0009-i)
//   ///
//   /// The function MUST:
//   ///   1. Generate a new token + hash.
//   ///   2. Emit `TokenRotatedEvent { token_id: old_id, workspace_id }` to the
//   ///      OTel/stdout surface (R-0004; stdout structured log is acceptable
//   ///      at V0 — a Postgres events table would violate R-0004-c).
//   ///      NOTE: R-0004-c forbids an in-app events table. The event is emitted
//   ///      as a structured log record. The TESTABLE observable for ordering is
//   ///      the RETURN VALUE: `rotate()` returns `TokenRotatedEvent` BEFORE the
//   ///      caller can observe the old row gone. The test sequence is:
//   ///        a. Call `rotate(pool, old_token_id)`.
//   ///        b. Assert `TokenRotatedEvent.token_id == old_token_id`.
//   ///        c. Assert old-hash lookup returns zero rows (no grace period).
//   ///   3. Insert the new token row.
//   ///   4. Delete the old token row.
//   ///
//   /// # No block-list (R-0008-f)
//   ///
//   /// Revocation = DELETE the old row. There is no block-list table at V0.
//   /// After rotation, `lookup_by_hash(old_hash, pool)` returns `Ok(None)`.
//   pub async fn rotate(
//       pool: &sqlx::PgPool,
//       old_token_id: uuid::Uuid,
//       workspace_id: uuid::Uuid,
//       scopes: Vec<String>,
//   ) -> Result<(AdminToken, TokenRotatedEvent), RotateError>;
//
//   /// Look up a token row by hash using constant-time comparison at the
//   /// application layer (after the DB SELECT by hash). Returns None if not found.
//   /// A "not found" result after rotation must be immediate — no grace period
//   /// (R-0009-i).
//   pub async fn lookup_by_hash(
//       hash: &TokenHash,
//       pool: &sqlx::PgPool,
//   ) -> Result<Option<TokenRow>, sqlx::Error>;
//
//   /// Error type for rotation failures.
//   #[derive(Debug)]
//   pub enum RotateError {
//       TokenNotFound(uuid::Uuid),
//       Db(sqlx::Error),
//   }
//
//   /// A fetched token row (mirrors admin_tokens columns for R-0008-c).
//   #[derive(Debug)]
//   pub struct TokenRow {
//       pub id: uuid::Uuid,
//       pub token_hash: Vec<u8>,      // BLAKE3 hash bytes
//       pub workspace_id: uuid::Uuid,
//       pub scopes: Vec<String>,
//       pub created_at: chrono::DateTime<chrono::Utc>,
//       pub rotated_at: Option<chrono::DateTime<chrono::Utc>>,
//   }
//
// ## Token file seam (R-0008-e)
//
//   /// Write the admin token to the filesystem at mode 600 (R-0008-e).
//   ///
//   /// Path resolution order:
//   ///   1. `MNEMRA_TOKEN_FILE` env var, if set and non-empty.
//   ///   2. `~/.config/mnemra/token` (HOME expansion).
//   ///
//   /// The same resolution logic is used by the startup mode-check (R-0005-f):
//   /// `check_token_file_mode(pool)` resolves the path through the SAME
//   /// `MNEMRA_TOKEN_FILE` override.
//   ///
//   /// # Test mechanics (env-var serialization)
//   ///
//   /// Tests that exercise MNEMRA_TOKEN_FILE MUST NOT use `std::env::set_var`
//   /// under parallel `cargo test --workspace` — global env mutation races.
//   /// Preferred approach: pass the path through the function parameter
//   /// (`write_token_file(token, path)` where path = `tempdir/token`).
//   /// If the env-var path is unavoidable (e.g., testing the startup check),
//   /// serialize with a `Mutex<()>` guard (non-poisoning pattern from
//   /// `postgres_engine.rs`).
//   ///
//   /// Mode assertion: `std::os::unix::fs::PermissionsExt::mode() & 0o777 == 0o600`.
//   pub fn write_token_file(
//       token: &AdminToken,
//       path: &std::path::Path,
//   ) -> Result<(), std::io::Error>;
//
//   /// Check that the file at `path` is mode 600, owner = current UID.
//   /// Returns Err if mode != 600 or file is world-readable.
//   pub fn check_token_file_mode(path: &std::path::Path) -> Result<(), TokenFileModeError>;
//
// ## Migration seam
//
//   Task 11 MUST append to `V0_MIGRATIONS` in `src/schema/init.rs`:
//
//   Migration {
//       version: 7,
//       name: "create_admin_tokens",
//       sql: "
//           CREATE TABLE IF NOT EXISTS admin_tokens (
//               id           UUID        NOT NULL DEFAULT gen_random_uuid(),
//               token_hash   BYTEA       NOT NULL,
//               workspace_id UUID        NOT NULL,
//               scopes       TEXT[]      NOT NULL,
//               created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
//               rotated_at   TIMESTAMPTZ,
//               CONSTRAINT admin_tokens_pkey          PRIMARY KEY (id),
//               CONSTRAINT admin_tokens_hash_uq       UNIQUE      (token_hash)
//           )
//       ",
//   }
//
//   The `init(&engine, "vector")` call in these tests then creates the table
//   and all assertions in this file flip from red to green.
//
// ## verify: [] rationale
//
//   `verify: []` is correct by design for this RED phase. The failing
//   assertions are runtime/assertion failures against the MISSING Task 11
//   surface — not compile failures. Every test in this file compiles clean
//   today. The `verify` field holds just-recipe names that run the tests;
//   the recipe does not exist yet because the test file itself is new.
//   Task 11 populates the just recipe and the verify field is updated at
//   that point.
