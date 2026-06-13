//! Two-adapter contract test — Postgres adapter.
//!
//! Exercises the same two invariants from `common` against the Postgres adapter
//! backed by the embedded engine (`postgresql_embedded`).
//!
//! Each test starts its own embedded engine.  A `std::sync::Mutex` serialises
//! the startup phase so concurrent test threads do not race on archive
//! extraction or pgvector download — both of which are non-idempotent under
//! concurrent access.  After startup, each engine runs independently on its
//! own ephemeral port; the unique `WorkspaceId` values per test prevent data
//! interference.
//!
//! # Invariants under test
//!
//! **A — Atomic multi-write:**
//! - Commit direction: all staged writes become visible atomically.
//! - Rollback direction: no staged writes visible after drop-without-commit.
//!
//! **B — Workspace-scoped isolation:**
//! - Cross-workspace read returns zero rows.
//! - Cross-workspace write does not mutate the target workspace's rows.
//!
//! # Known residual risks
//!
//! **A-10 — zombie postmaster on test panic:**
//! See `postgres_engine.rs` module-level doc for the full analysis.  The same
//! residual risk applies here: SIGKILL or async-drop failure can leave orphaned
//! postmaster processes.  Accepted at Gate A.
//!
//! **A-11 — cross-binary archive-extraction race:**
//! `STARTUP_LOCK` here and the one in `postgres_engine.rs` are independent
//! per-binary mutexes.  Under `cargo test --workspace` (sequential binary
//! scheduling by default) they do not race.  If parallel binary execution is
//! enabled, the no-dep fix is consolidating both Postgres test binaries into one.
//! Current CI posture (`cargo test --workspace`) is safe.  Deferred to a
//! follow-up task.
//!
//! **A-12 — temp data-dir leak on SIGKILL:**
//! Same posture as `postgres_engine.rs`.  Accepted at Gate A.

mod common;

use mnemra_host::storage::postgres::PostgresStorage;
use std::sync::Mutex;

/// Serialises engine startup across concurrent test threads.
///
/// `postgresql_embedded` archive extraction and `postgresql_extensions` package
/// download are not safe to run concurrently from the same process.  Holding
/// this lock during `start_embedded()` makes startups sequential while still
/// allowing data-path operations to run fully in parallel once started.
static STARTUP_LOCK: Mutex<()> = Mutex::new(());

/// Start a fresh `PostgresStorage` with serialised engine startup.
async fn start() -> PostgresStorage {
    let _guard = STARTUP_LOCK.lock().expect("startup lock poisoned");
    PostgresStorage::start_embedded()
        .await
        .expect("failed to start embedded Postgres")
    // _guard released here: startup finished, next test may now start its engine.
}

// ---------------------------------------------------------------------------
// Invariant A — Atomicity
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_atomicity_commit() {
    common::assert_atomicity_commit(start().await).await;
}

#[tokio::test]
async fn postgres_atomicity_rollback() {
    common::assert_atomicity_rollback(start().await).await;
}

// ---------------------------------------------------------------------------
// Invariant B — Isolation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_isolation_no_cross_workspace_read() {
    common::assert_isolation_no_cross_workspace_read(start().await).await;
}

#[tokio::test]
async fn postgres_isolation_no_cross_workspace_write() {
    common::assert_isolation_no_cross_workspace_write(start().await).await;
}
