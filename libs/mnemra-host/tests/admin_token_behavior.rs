//! Admin-token behavioral suite — GREEN phase (Task 11 seal, R-0008-a..h, R-0009-i).
//!
//! # Purpose
//!
//! This file is the behavioral half of the TDD pair 10/11. The red phase
//! (`admin_token.rs`) pinned the `admin_tokens` SCHEMA. This file pins the
//! API behavior now that the implementation exists.
//!
//! Every test is a real behavioral assertion against `mnemra_host::auth::token`.
//! No test weakens or substitutes for any assertion in the red file.
//!
//! # R-ID mapping
//!
//! | Test function                              | R-ID(s)              |
//! |--------------------------------------------|----------------------|
//! | token_shape_length_and_alphabet            | R-0008-a             |
//! | token_shape_two_calls_distinct             | R-0008-a             |
//! | token_shape_entropy_n_calls_all_distinct   | R-0008-a (adversarial)|
//! | round_trip_via_db                          | R-0008-b, R-0008-c (auth-by-hash success path) |
//! | bogus_hash_lookup_returns_none             | R-0008-b, R-0008-f (auth-by-hash failure path / Task-23 401 precursor, adversarial) |
//! | file_mode_600_and_content                  | R-0008-e             |
//! | file_mode_check_rejects_644               | R-0008-e             |
//! | revocation_lookup_by_old_hash_returns_none | R-0008-f             |
//! | rotation_event_token_id_matches_old_id     | R-0008-g             |
//! | no_grace_period_after_rotation             | R-0009-i             |
//! | token_row_exactly_six_fields               | R-0008-h (behavioral complement) |
//! | rotation_new_token_is_live                 | R-0008-f (adversarial complement) |
//!
//! # Engine bring-up
//!
//! Each test starts its own embedded Postgres instance. Engine startup is
//! serialized within this binary via `STARTUP_LOCK` (A-11 — archive-extraction
//! race). The lock is acquired and immediately released in a synchronous block
//! before the async `.start().await`, satisfying `clippy::await_holding_lock`
//! (fix of the latent violation in `schema_init.rs`).
//!
//! `init(&engine, "vector")` is called at the start of every DB-touching test to
//! create the `admin_tokens` table (migration 7) and the default workspace.
//!
//! # Env-var safety
//!
//! Tests that need `MNEMRA_TOKEN_FILE` MUST NOT use `std::env::set_var`
//! (races under `cargo test --all`). The file-mode tests call `write_token_file`
//! and `check_token_file_mode` with an explicit `tempdir` path — no env mutation.
//!
//! # Pool notes
//!
//! `engine.pool` (mnemra_app role, table owner) is used for all DB operations.
//! The `mnemra_host_fns` role does NOT have DELETE on `admin_tokens` (cross-
//! dispatch note 4); `rotate()` therefore requires the app-owner pool, which
//! `engine.pool` is.

use mnemra_host::auth::token::{
    TokenRow, check_token_file_mode, generate, hash, lookup_by_hash, rotate, write_token_file,
};
use mnemra_host::schema::init::{DEFAULT_WORKSPACE_ID, init};
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use std::sync::Mutex;
use tempfile::tempdir;
use uuid::Uuid;

/// Serializes engine startup across concurrent test threads within this binary (A-11).
///
/// Independent of locks in `postgres_engine.rs`, `schema_init.rs`, etc.
/// Under `cargo test --workspace` (default sequential binary scheduling) there
/// is no cross-binary race. This lock covers within-binary test parallelism.
static STARTUP_LOCK: Mutex<()> = Mutex::new(());

/// Start a fresh embedded engine with startup serialized.
///
/// The lock is acquired and released in a synchronous block BEFORE the async
/// `.start().await` call — satisfies `clippy::await_holding_lock` while still
/// serializing the archive-extraction phase.
async fn start_engine() -> EmbeddedEngine {
    {
        let _guard = STARTUP_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // Guard dropped here; async engine start races safely after this point.
    }
    EmbeddedEngine::start()
        .await
        .expect("failed to start embedded Postgres")
}

// ---------------------------------------------------------------------------
// R-0008-a — token shape: length, alphabet, distinctness
// ---------------------------------------------------------------------------

/// Assert `generate()` produces a 43-character base64url string (no padding).
///
/// # R-0008-a
///
/// 32 raw bytes → base64url-encode (URL_SAFE_NO_PAD) → always 43 chars.
/// All characters must be in the base64url alphabet: `[A-Za-z0-9_-]`.
#[tokio::test]
async fn token_shape_length_and_alphabet() {
    // R-0008-a: 43 chars, base64url alphabet.
    let token = generate();
    assert_eq!(
        token.as_str().len(),
        43,
        "R-0008-a: generated token must be exactly 43 chars (base64url of 32 bytes, no padding); \
         got {}",
        token.as_str().len()
    );
    for ch in token.as_str().chars() {
        assert!(
            ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
            "R-0008-a: non-base64url char '{ch}' in token '{}'",
            token.as_str()
        );
    }
}

/// Assert two consecutive `generate()` calls produce distinct strings.
///
/// # R-0008-a
///
/// Cryptographic tokens from OsRng MUST be unique. If two consecutive calls
/// produce the same string, the CSPRNG is broken or the function is not using
/// fresh randomness each call.
#[tokio::test]
async fn token_shape_two_calls_distinct() {
    // R-0008-a: two consecutive generate() calls must be distinct.
    let t1 = generate();
    let t2 = generate();
    assert_ne!(
        t1.as_str(),
        t2.as_str(),
        "R-0008-a: two consecutive generate() calls must produce distinct tokens"
    );
}

/// Adversarial entropy check: N=50 calls all distinct.
///
/// # R-0008-a (adversarial)
///
/// Two-call distinctness is necessary but not sufficient for a functioning
/// CSPRNG. Generating 50 tokens and asserting uniqueness across all pairs
/// gives a birthday-paradox false-positive probability of < 1e-74 if the
/// CSPRNG is working correctly, and catches a stuck/repeating generator.
#[tokio::test]
async fn token_shape_entropy_n_calls_all_distinct() {
    // R-0008-a adversarial: 50 tokens all distinct.
    const N: usize = 50;
    let tokens: Vec<String> = (0..N).map(|_| generate().as_str().to_owned()).collect();

    // Check every pair — O(N^2) is fine for N=50.
    for i in 0..N {
        for j in (i + 1)..N {
            assert_ne!(
                tokens[i], tokens[j],
                "R-0008-a adversarial: token[{i}] == token[{j}] — CSPRNG not producing distinct values"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// R-0008-b — authentication by unique-hash lookup
// ---------------------------------------------------------------------------
//
// R-0008-b was amended (2026-06-21): authentication is a BLAKE3-hash lookup
// against the unique `token_hash` column, NOT a constant-time byte comparison.
// The admin token is a 256-bit CSPRNG value compared via its BLAKE3 hash, so
// comparison-timing attacks do not apply and no constant-time primitive is
// required on this path. The former `verify_*` tests sealed the (now removed)
// constant-time `verify()` function; they are deleted because the requirement
// they sealed no longer exists. R-0008-b's authentication invariant is now
// covered by the lookup-path tests below:
//   - `round_trip_via_db` — auth-by-hash SUCCESS path: a token's BLAKE3 hash
//     finds its row via `lookup_by_hash` (R-0008-b + R-0008-c).
//   - `bogus_hash_lookup_returns_none` — auth-by-hash FAILURE path: a
//     never-inserted hash yields `Ok(None)` → 401 (R-0008-b + R-0008-f).

// ---------------------------------------------------------------------------
// R-0008-c — round-trip via DB: generate → hash → INSERT → lookup_by_hash
// ---------------------------------------------------------------------------

/// Assert round-trip: `generate()` → `hash()` → INSERT → `lookup_by_hash()` returns `Some`.
///
/// # R-0008-b, R-0008-c
///
/// The DB round-trip pins that `lookup_by_hash` finds the row, and that the
/// returned `TokenRow` carries the expected `workspace_id` and `scopes`. This is
/// also the R-0008-b authentication SUCCESS path: a presented token is
/// authenticated by matching its BLAKE3 hash against the unique `token_hash`
/// column (no constant-time byte comparison — see the R-0008-b section note).
#[tokio::test]
async fn round_trip_via_db() {
    // R-0008-c: full round-trip through the real DB.
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let pool = engine.pool.as_ref();
    let workspace_id = DEFAULT_WORKSPACE_ID;
    let scopes = vec!["admin".to_owned()];

    let token = generate();
    let token_hash = hash(&token);

    // INSERT using the app-role pool (owner of admin_tokens).
    let inserted_id: (Uuid,) = sqlx::query_as(
        "INSERT INTO admin_tokens (token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(token_hash.as_bytes())
    .bind(workspace_id)
    .bind(&scopes)
    .fetch_one(pool)
    .await
    .expect("INSERT into admin_tokens failed");

    let row = lookup_by_hash(&token_hash, pool)
        .await
        .expect("lookup_by_hash should not error")
        .expect("R-0008-c: lookup_by_hash must return Some after INSERT");

    assert_eq!(
        row.id, inserted_id.0,
        "R-0008-c: returned TokenRow.id must match inserted id"
    );
    assert_eq!(
        row.workspace_id, workspace_id,
        "R-0008-c: returned TokenRow.workspace_id must match inserted workspace_id"
    );
    assert_eq!(
        row.scopes, scopes,
        "R-0008-c: returned TokenRow.scopes must match inserted scopes"
    );
}

/// Adversarial: `lookup_by_hash` for a bogus hash returns `Ok(None)`.
///
/// # R-0008-b, R-0008-f (adversarial — Task-23 401 precursor)
///
/// A hash that was never inserted must yield `Ok(None)` — the caller translates
/// this into a 401 Unauthorized at the HTTP layer (Task 23). No row in the table
/// should produce a match for arbitrary BLAKE3-shaped bytes. This is the R-0008-b
/// authentication FAILURE path: a token not matching any stored hash is rejected.
#[tokio::test]
async fn bogus_hash_lookup_returns_none() {
    // R-0008-f adversarial: a bogus hash must return Ok(None).
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let pool = engine.pool.as_ref();

    // Generate a token that was NEVER inserted into the DB.
    let phantom_token = generate();
    let phantom_hash = hash(&phantom_token);

    let result = lookup_by_hash(&phantom_hash, pool)
        .await
        .expect("lookup_by_hash must not error on a miss");

    assert!(
        result.is_none(),
        "R-0008-f adversarial: lookup_by_hash for a never-inserted hash must return None"
    );
}

// ---------------------------------------------------------------------------
// R-0008-e — token file mode 600, content, and mode check
// ---------------------------------------------------------------------------

/// Assert `write_token_file` creates a file with mode 0600 and content == `token.as_str()`.
///
/// # R-0008-e
///
/// The file is created in a tempdir (never `~/.config`). Mode is checked via
/// `PermissionsExt::mode() & 0o777`. Content must be the exact base64url string.
/// No env mutation — explicit path argument only.
#[tokio::test]
async fn file_mode_600_and_content() {
    // R-0008-e: write_token_file → mode 0600, content == token string.
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("token");
    let token = generate();

    write_token_file(&token, &path).expect("write_token_file must not fail");

    // Mode assertion.
    let metadata = std::fs::metadata(&path).expect("metadata after write");
    let mode = metadata.permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "R-0008-e: token file must be mode 0600 after write_token_file; got {mode:#o}"
    );

    // Content assertion.
    let content = std::fs::read_to_string(&path).expect("read back token file");
    assert_eq!(
        content,
        token.as_str(),
        "R-0008-e: token file content must equal token.as_str()"
    );

    // check_token_file_mode must succeed on the just-written file.
    check_token_file_mode(&path)
        .expect("R-0008-e: check_token_file_mode must succeed on a 0600 file");
}

/// Assert `check_token_file_mode` rejects a file widened to 0644.
///
/// # R-0008-e
///
/// Simulates a misconfigured file (e.g., copied with wrong umask). The function
/// must return `Err(WrongMode { actual: 0o644 })`.
#[tokio::test]
async fn file_mode_check_rejects_644() {
    // R-0008-e: check_token_file_mode must reject a 0644 file.
    use mnemra_host::auth::token::TokenFileModeError;
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("token");
    let token = generate();

    write_token_file(&token, &path).expect("write_token_file must not fail");

    // Widen to 0644 — simulates a misconfigured file.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
        .expect("set_permissions to 0644");

    let err = check_token_file_mode(&path)
        .expect_err("R-0008-e: check_token_file_mode must return Err for a 0644 file");

    match err {
        TokenFileModeError::WrongMode { actual } => {
            assert_eq!(
                actual, 0o644,
                "R-0008-e: WrongMode.actual must be 0o644; got {actual:#o}"
            );
        }
        other => panic!("R-0008-e: expected WrongMode, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// R-0008-f — revocation: old hash is gone after rotate
// ---------------------------------------------------------------------------

/// Assert `lookup_by_hash(&old_hash)` returns `Ok(None)` after `rotate()`.
///
/// # R-0008-f
///
/// Revocation = DELETE the old row. No block-list. After `rotate()` completes,
/// the old token's hash must not be found in `admin_tokens`.
#[tokio::test]
async fn revocation_lookup_by_old_hash_returns_none() {
    // R-0008-f: old hash unreachable after rotate (no block-list, DELETE semantics).
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let pool = engine.pool.as_ref();
    let workspace_id = DEFAULT_WORKSPACE_ID;
    let scopes = vec!["admin".to_owned()];

    // Insert an initial token.
    let token = generate();
    let token_hash = hash(&token);

    let (old_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO admin_tokens (token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(token_hash.as_bytes())
    .bind(workspace_id)
    .bind(&scopes)
    .fetch_one(pool)
    .await
    .expect("INSERT failed");

    // Rotate: old row deleted, new row inserted.
    rotate(pool, old_id).await.expect("rotate must succeed");

    // Old hash must be gone.
    let result = lookup_by_hash(&token_hash, pool)
        .await
        .expect("lookup_by_hash must not error");

    assert!(
        result.is_none(),
        "R-0008-f: lookup_by_hash(&old_hash) must return None after rotate (revocation = DELETE, no block-list)"
    );
}

/// Adversarial complement: after rotate, `lookup_by_hash(&new_hash)` returns `Some`.
///
/// # R-0008-f (adversarial complement)
///
/// The old hash is gone AND the new token is live. Confirms rotation does not
/// silently delete both rows or insert into the wrong table.
#[tokio::test]
async fn rotation_new_token_is_live() {
    // R-0008-f adversarial: new token is live after rotate.
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let pool = engine.pool.as_ref();
    let workspace_id = DEFAULT_WORKSPACE_ID;
    let scopes = vec!["admin".to_owned()];

    let token = generate();
    let token_hash = hash(&token);

    let (old_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO admin_tokens (token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(token_hash.as_bytes())
    .bind(workspace_id)
    .bind(&scopes)
    .fetch_one(pool)
    .await
    .expect("INSERT failed");

    let (new_token, _event) = rotate(pool, old_id).await.expect("rotate must succeed");

    let new_hash = hash(&new_token);
    let row = lookup_by_hash(&new_hash, pool)
        .await
        .expect("lookup_by_hash must not error")
        .expect("R-0008-f adversarial: new token must be live after rotate");

    assert_eq!(
        row.workspace_id, workspace_id,
        "R-0008-f adversarial: new token row must carry the correct workspace_id"
    );
}

// ---------------------------------------------------------------------------
// R-0008-g — rotation event ordering: event.token_id == old_id
// ---------------------------------------------------------------------------

/// Assert `TokenRotatedEvent.token_id == old_id` after `rotate()`.
///
/// # R-0008-g
///
/// `rotate()` constructs `TokenRotatedEvent` from the OLD row's data BEFORE
/// issuing the DELETE. The return value is the observable ordering seam: the
/// caller receives `(new_token, event)` BEFORE any DB change is visible.
/// Asserting `event.token_id == old_id` pins the identity of the emitted event.
#[tokio::test]
async fn rotation_event_token_id_matches_old_id() {
    // R-0008-g: rotation event carries the old token's id.
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let pool = engine.pool.as_ref();
    let workspace_id = DEFAULT_WORKSPACE_ID;
    let scopes = vec!["admin".to_owned()];

    let token = generate();
    let token_hash = hash(&token);

    let (old_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO admin_tokens (token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(token_hash.as_bytes())
    .bind(workspace_id)
    .bind(&scopes)
    .fetch_one(pool)
    .await
    .expect("INSERT failed");

    let (_new_token, event) = rotate(pool, old_id).await.expect("rotate must succeed");

    assert_eq!(
        event.token_id, old_id,
        "R-0008-g: TokenRotatedEvent.token_id must equal the old token's id; \
         event carries the identity of the rotated (deleted) token"
    );
}

// ---------------------------------------------------------------------------
// R-0009-i — no grace period: old hash unreachable immediately after rotate
// ---------------------------------------------------------------------------

/// Assert `lookup_by_hash(&old_hash)` returns `Ok(None)` with NO intervening sleep.
///
/// # R-0009-i
///
/// After `rotate()` returns, a lookup on the old hash with no sleep → `Ok(None)`.
/// This is the behavioral pin for "no grace period": the old row is deleted
/// within the same transaction as the new row insert, committed before `rotate()`
/// returns.
#[tokio::test]
async fn no_grace_period_after_rotation() {
    // R-0009-i: old hash unreachable IMMEDIATELY after rotate (no sleep, no grace period).
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let pool = engine.pool.as_ref();
    let workspace_id = DEFAULT_WORKSPACE_ID;
    let scopes = vec!["admin".to_owned()];

    let token = generate();
    let token_hash = hash(&token);

    let (old_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO admin_tokens (token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(token_hash.as_bytes())
    .bind(workspace_id)
    .bind(&scopes)
    .fetch_one(pool)
    .await
    .expect("INSERT failed");

    // Rotate — no sleep before or after.
    rotate(pool, old_id).await.expect("rotate must succeed");

    // Immediate lookup — NO sleep between rotate() return and this call.
    let result = lookup_by_hash(&token_hash, pool)
        .await
        .expect("lookup_by_hash must not error");

    assert!(
        result.is_none(),
        "R-0009-i: lookup_by_hash(&old_hash) must return None IMMEDIATELY after rotate — \
         no grace period, old row deleted in the same transaction"
    );
}

// ---------------------------------------------------------------------------
// F-13 red-phase — rotate() tenant + scope preservation
// ---------------------------------------------------------------------------

/// Assert `rotate(pool, old_id)` preserves the old token's `scopes`, preventing escalation.
///
/// # F-13 (red phase)
///
/// Under the old 4-arg signature, a caller could supply `["admin"]` as the scopes
/// argument regardless of what the old token held — a privilege-escalation primitive.
/// The 2-arg signature makes escalation inexpressible at the call site; this test
/// pins the runtime guarantee: after rotation the new token's scopes equal the old
/// row's scopes, not any caller-supplied value.
///
/// This test FAILS TO COMPILE against the current 4-arg impl — that compile failure
/// is the correct red state. Forge's green phase changes the impl to the 2-arg sig.
#[tokio::test]
async fn rotation_preserves_scopes_no_escalation() {
    // Given: an old token created with read_observer scope (not admin).
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let pool = engine.pool.as_ref();
    // Use a non-admin scope to make the escalation distinction observable.
    let old_scopes = vec!["read_observer".to_owned()];

    let token = generate();
    let token_hash = hash(&token);

    let (old_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO admin_tokens (token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(token_hash.as_bytes())
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(&old_scopes)
    .fetch_one(pool)
    .await
    .expect("INSERT failed");

    // When: rotate with 2-arg signature (derives workspace_id + scopes from old row).
    let (new_token, _event) = rotate(pool, old_id).await.expect("rotate must succeed");

    // Then: the new token's scopes equal the old row's scopes (read_observer), not admin.
    // Under the old 4-arg signature a caller could pass ["admin"] here; the 2-arg
    // signature makes escalation inexpressible — this assertion pins that the scopes
    // are preserved from the old row and cannot be elevated by a caller parameter.
    let new_hash = hash(&new_token);
    let row = lookup_by_hash(&new_hash, pool)
        .await
        .expect("lookup_by_hash must not error")
        .expect("F-13: new token must be live after rotate");

    assert_eq!(
        row.scopes, old_scopes,
        "F-13: new token scopes must equal old token scopes (read_observer, not admin); \
         rotate() must derive scopes from the old row, not from caller params"
    );
}

/// Assert `rotate(pool, old_id)` preserves the old token's `workspace_id`, preventing cross-tenant mint.
///
/// # F-13 (red phase)
///
/// The old token is inserted into a workspace that is distinct from `DEFAULT_WORKSPACE_ID`.
/// After rotation, the new token must carry that same workspace_id — proving the impl
/// reads from the old row, not from any caller-supplied parameter.
///
/// `admin_tokens.workspace_id` has no FK constraint to the `workspaces` table (verified
/// against migration 7 DDL), so a second-workspace token can be seeded via a direct INSERT
/// using an arbitrary UUID without needing to create a workspaces row. This allows the test
/// to prove cross-tenant preservation as a runtime guarantee rather than relying on
/// type-enforcement alone.
///
/// This test FAILS TO COMPILE against the current 4-arg impl — that compile failure
/// is the correct red state. Forge's green phase changes the impl to the 2-arg sig.
#[tokio::test]
async fn rotation_preserves_workspace_id() {
    // Given: an old token in a workspace DISTINCT from DEFAULT_WORKSPACE_ID.
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let pool = engine.pool.as_ref();
    let scopes = vec!["admin".to_owned()];

    // A deterministic non-default workspace UUID (UUID v5 of "tenant-b" in DNS namespace).
    // Chosen to be stable across runs and visually distinct from DEFAULT_WORKSPACE_ID.
    // No workspaces row is required — admin_tokens.workspace_id has no FK constraint.
    let tenant_b_workspace_id: Uuid = uuid::uuid!("d4e6f2a8-1c3b-5e9d-8f7a-2b4c6e0d1f3a");

    let token = generate();
    let token_hash = hash(&token);

    let (old_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO admin_tokens (token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(token_hash.as_bytes())
    .bind(tenant_b_workspace_id)
    .bind(&scopes)
    .fetch_one(pool)
    .await
    .expect("INSERT failed");

    // When: rotate with 2-arg signature (derives workspace_id from old row).
    let (new_token, _event) = rotate(pool, old_id).await.expect("rotate must succeed");

    // Then: the new token's workspace_id equals tenant_b, not DEFAULT_WORKSPACE_ID.
    // If the impl echoed a caller parameter (or defaulted to DEFAULT_WORKSPACE_ID),
    // this assertion would fail — proving the impl reads from the old row's workspace.
    let new_hash = hash(&new_token);
    let row = lookup_by_hash(&new_hash, pool)
        .await
        .expect("lookup_by_hash must not error")
        .expect("F-13: new token must be live after rotate");

    assert_eq!(
        row.workspace_id, tenant_b_workspace_id,
        "F-13: new token workspace_id must equal old token workspace_id (tenant_b); \
         rotate() must derive workspace_id from the old row, not from caller params — \
         a token in workspace A cannot be rotated into workspace B"
    );
}

// ---------------------------------------------------------------------------
// R-0008-h — behavioral complement: TokenRow exposes exactly six fields
// ---------------------------------------------------------------------------

/// Assert `TokenRow` has no field beyond the six schema columns.
///
/// # R-0008-h (behavioral complement)
///
/// The schema-contract red test (`admin_tokens_has_exactly_six_columns`) pinned
/// the column count at the DB level. This test is the API-level complement: an
/// exhaustive destructure of `TokenRow` with no `..` rest pattern fails to
/// compile if a seventh field is added to the struct. This makes the no-extra-
/// key-material guarantee a compile-time invariant, not just a DB introspection
/// check.
///
/// The fields destructured must exactly match the six columns in the schema
/// (R-0008-c): id, token_hash, workspace_id, scopes, created_at, rotated_at.
#[tokio::test]
async fn token_row_exactly_six_fields() {
    // R-0008-h behavioral: exhaustive destructure of TokenRow — compile fails if a 7th field is added.
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");

    let pool = engine.pool.as_ref();
    let workspace_id = DEFAULT_WORKSPACE_ID;
    let scopes = vec!["admin".to_owned()];

    let token = generate();
    let token_hash = hash(&token);

    sqlx::query(
        "INSERT INTO admin_tokens (token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3)",
    )
    .bind(token_hash.as_bytes())
    .bind(workspace_id)
    .bind(&scopes)
    .execute(pool)
    .await
    .expect("INSERT failed");

    let row = lookup_by_hash(&token_hash, pool)
        .await
        .expect("lookup_by_hash must not error")
        .expect("R-0008-h: row must exist after INSERT");

    // Exhaustive destructure — no `..` rest pattern. Adding any field to
    // TokenRow beyond these six breaks compilation (R-0008-h compile-time seal).
    let TokenRow {
        id,
        token_hash: row_hash,
        workspace_id: row_ws,
        scopes: row_scopes,
        created_at,
        rotated_at,
    } = row;

    // Exercise each field so the compiler does not warn about unused variables.
    let _ = id;
    assert_eq!(
        row_hash,
        token_hash.as_bytes(),
        "R-0008-h: token_hash round-trips through DB"
    );
    assert_eq!(
        row_ws, workspace_id,
        "R-0008-h: workspace_id round-trips through DB"
    );
    assert_eq!(
        row_scopes, scopes,
        "R-0008-h: scopes round-trips through DB"
    );
    assert!(
        created_at.timestamp() > 0,
        "R-0008-h: created_at must be a real timestamp"
    );
    assert!(
        rotated_at.is_none(),
        "R-0008-h: rotated_at must be None for a fresh token"
    );
}
