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

use mnemra_host::storage::postgres::{PostgresStorage, engine::EmbeddedEngine};
use sqlx::Row;
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
