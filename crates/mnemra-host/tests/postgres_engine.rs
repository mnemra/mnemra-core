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
