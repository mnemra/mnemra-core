//! Engine-level tests for the embedded Postgres engine.
//!
//! These tests verify:
//! - **Role shape** (AC#3): the application connection role holds neither
//!   `rolsuper` nor `rolbypassrls`.
//! - **pgvector availability** (AC#1): `pgvector_compiled` is installed so that
//!   `CREATE EXTENSION vector` succeeds and `pg_extension` lists `vector`.
//!
//! A `std::sync::Mutex` serialises engine startup within this test binary so
//! that concurrent archive extraction and pgvector download do not race.
//!
//! # Known residual risks
//!
//! **A-10 — zombie postmaster on test panic:**
//! `postgresql_embedded` uses `Drop` for postmaster cleanup.  Rust runs `Drop`
//! during normal panic unwinding, but the async-stop path in the drop impl is
//! not guaranteed to complete before the runtime exits.  On SIGKILL / OOM the
//! process is killed without any Drop execution, leaving a postmaster zombie.
//! In CI (ephemeral runners) the impact is negligible.  In local development,
//! repeated SIGKILL interruptions can exhaust ephemeral ports.
//! Full fix requires a dep-free watchdog or an OS-level `prctl(PR_SET_PDEATHSIG)`
//! wrapper — neither is achievable without new deps.  Residual risk accepted at
//! Gate A; tracked for revisit when local-dev ergonomics are prioritised.
//!
//! **A-11 — cross-binary archive-extraction race:**
//! `STARTUP_LOCK` is a per-binary `Mutex` — it serialises startup within this
//! binary, but does not coordinate with `storage_contract_postgres.rs`, which is
//! a separate test binary with its own lock.  `cargo test --workspace` (plain)
//! schedules test binaries sequentially unless the host is configured otherwise,
//! so the race does not fire under the standard CI command.  If parallel binary
//! execution is ever enabled (e.g. nextest or `--jobs`), both binaries would race
//! on `~/.theseus` extraction.  The correct no-dep fix is consolidating both
//! Postgres test binaries into one (so one `STARTUP_LOCK` covers all PG tests).
//! Deferred to a follow-up task; the current CI posture is safe.
//!
//! **A-12 — temp data-dir leak on SIGKILL:**
//! `SettingsBuilder::new().temporary(true)` auto-cleans the data dir on
//! `PostgreSQL` drop, but SIGKILL bypasses Drop entirely.  On CI (ephemeral
//! runners) leaked dirs are cleaned with the runner.  Local repeated SIGKILL
//! interruptions accumulate dirs under the system temp path.  Low-priority at V0;
//! tracked for a housekeeping follow-up when local-dev ergonomics are prioritised.

use mnemra_host::storage::postgres::engine::{
    ArtifactPin, KNOWN_GOOD_HASHES, current_platform, verify_pinned_artifacts,
};
use mnemra_host::storage::postgres::{PostgresStorage, engine::EmbeddedEngine};
use sqlx::Row;
use std::path::Path;
use std::sync::Mutex;

/// Serialises engine startup across concurrent test threads.
static STARTUP_LOCK: Mutex<()> = Mutex::new(());

/// Verify the application role (`mnemra_app`) is an ordinary role with neither
/// superuser nor BYPASSRLS privileges.
///
/// This assertion binds AC#3 (P-0010 multi-tenancy precondition) at V0.
#[tokio::test]
async fn app_role_is_not_superuser_and_not_bypassrls() {
    let storage = {
        // Recover from a poisoned mutex so a failing sibling test does not
        // cascade a PoisonError into this test.  The guard is held only for
        // the duration of engine startup to serialise archive extraction and
        // pgvector download; its state carries no invariant across tests.
        let _guard = STARTUP_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        PostgresStorage::start_embedded().await.unwrap()
    };

    // Query pg_roles for the current user's privilege flags.
    let row =
        sqlx::query("SELECT rolsuper, rolbypassrls FROM pg_roles WHERE rolname = current_user")
            .fetch_one(storage.pool())
            .await
            .expect("pg_roles query failed");

    let rolsuper: bool = row.get(0);
    let rolbypassrls: bool = row.get(1);

    assert!(
        !rolsuper,
        "mnemra_app must not be a superuser (P-0010 precondition); got rolsuper=true"
    );
    assert!(
        !rolbypassrls,
        "mnemra_app must not have BYPASSRLS (P-0010 precondition); got rolbypassrls=true"
    );
}

// ---------------------------------------------------------------------------
// Hash-pin control tests (A-04/A-05, Task 6b)
// ---------------------------------------------------------------------------
//
// These tests exercise `verify_pinned_artifacts` as a pure function — no live
// engine, no shared `~/.theseus` cache touched — so they are fast and safe to
// run in parallel with other tests.

/// NEGATIVE: A deliberately-wrong expected hash produces a structured
/// `HashPinError` naming the artifact and both digest values.
///
/// Proves the control fails SHUT (structured error, not panic) on a hash
/// mismatch.  The install_dir is the real theseus cache so the file IS
/// readable; only the expected sha256 is wrong.
#[test]
fn hash_pin_mismatch_returns_structured_error_not_panic() {
    // Build a pin table with a known-wrong expected hash.
    // We use the real postgres binary path but a bogus expected digest.
    let wrong_pins: &[(&str, &[ArtifactPin])] = &[(
        current_platform(),
        &[ArtifactPin {
            artifact: "postgres-tampered-test",
            rel_path: "bin/postgres",
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        }],
    )];

    // Locate the real installation directory via HOME env var.
    let home = std::env::var("HOME").unwrap_or_default();
    let install_dir = std::path::PathBuf::from(&home).join(".theseus/postgresql/16.4.0");

    if !install_dir.join("bin/postgres").exists() {
        // Skip on a cold runner that has never run the engine; the positive
        // pgvector_extension_is_available test covers the warm-cache path.
        eprintln!("SKIP hash_pin_mismatch_returns_structured_error_not_panic: cache not warm");
        return;
    }

    let result = verify_pinned_artifacts(&install_dir, current_platform(), wrong_pins);
    assert!(
        result.is_err(),
        "verify_pinned_artifacts must return Err on a hash mismatch, got Ok"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.artifact, "postgres-tampered-test",
        "HashPinError must name the artifact"
    );
    assert_eq!(
        err.expected, "0000000000000000000000000000000000000000000000000000000000000000",
        "HashPinError must carry the expected hash"
    );
    assert!(
        !err.actual.is_empty(),
        "HashPinError must carry the actual hash"
    );
    assert_ne!(
        err.actual, err.expected,
        "actual and expected must differ in the error"
    );
    // The error must be displayable without panic (Display is the structured surface
    // Task 7 builds on).
    let msg = format!("{}", err);
    assert!(
        msg.contains("postgres-tampered-test"),
        "Display output must name the artifact: {msg}"
    );
}

/// NEGATIVE: An unknown platform produces a structured `HashPinError` telling
/// the operator to add a pin — not a silent skip, not a panic.
#[test]
fn hash_pin_unknown_platform_returns_structured_error() {
    let result = verify_pinned_artifacts(
        Path::new("/tmp"),
        "unknown-platform-not-in-table",
        KNOWN_GOOD_HASHES,
    );
    assert!(
        result.is_err(),
        "verify_pinned_artifacts must return Err for an unknown platform, got Ok"
    );
    let err = result.unwrap_err();
    assert!(
        err.artifact.contains("platform:"),
        "HashPinError for unknown platform must name the platform in artifact field: {:?}",
        err.artifact
    );
    assert!(
        err.expected.contains("no pin entry"),
        "HashPinError for unknown platform must say 'no pin entry' in expected: {:?}",
        err.expected
    );
}

/// Verify that `pgvector_compiled` is installed and `CREATE EXTENSION vector`
/// succeeds.  Also checks `pg_extension` lists `vector`.
///
/// Task 7 builds its init-gate (`ensure_pgvector`) on this seam.
#[tokio::test]
async fn pgvector_extension_is_available_and_creates_successfully() {
    let engine = {
        // Recover from a poisoned mutex so a failing sibling test does not
        // cascade a PoisonError into this test.  The guard is held only for
        // the duration of engine startup to serialise archive extraction and
        // pgvector download; its state carries no invariant across tests.
        let _guard = STARTUP_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        EmbeddedEngine::start().await.unwrap()
    };

    // ensure_pgvector calls CREATE EXTENSION IF NOT EXISTS vector (as superuser)
    // and returns a structured ExtensionError on failure.
    engine
        .ensure_pgvector()
        .await
        .expect("CREATE EXTENSION vector should succeed with bundled pgvector_compiled");

    // Confirm via catalog that the extension is loaded.
    let row: (String,) =
        sqlx::query_as("SELECT extname FROM pg_extension WHERE extname = 'vector'")
            .fetch_one(engine.pool.as_ref())
            .await
            .expect("pg_extension query failed — vector not in catalog");

    assert_eq!(
        row.0, "vector",
        "pg_extension should list 'vector' after CREATE EXTENSION"
    );
}
