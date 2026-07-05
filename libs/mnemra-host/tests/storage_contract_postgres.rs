//! Two-adapter contract test — Postgres adapter.
//!
//! Exercises the same two invariants from `common` against the Postgres adapter
//! backed by the embedded engine (`postgresql_embedded`).
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
//! # Engine acquisition (Tier-2 T4, R-0037/R-0030/R-0029)
//!
//! Acquisition-migrated onto the shared-engine fixture: each test acquires
//! the binary-wide shared engine via `shared_engine::shared_engine()` and
//! provisions its own fresh, isolated database via
//! `EmbeddedEngine::provision_test_database()` — no per-file
//! boot-serialization mutex needed. The fixture's own get-or-init semantics
//! guarantee exactly-once boot; the per-file boot-serialization mutex
//! previously here is retired as vestigial (R-0029).
//!
//! `PostgresStorage` is now pool-injected (R-0037: `PostgresStorage` is a pure
//! pool adapter and no longer boots or owns an engine): `start()` bootstraps
//! the `records` table on the provisioned database's pool via
//! `postgres::bootstrap_records_table` — the SAME helper the production
//! composition root (`mnemra_host.rs`) calls, so both stay in sync — then
//! hands the pool to `PostgresStorage::new`. Assertions below are unchanged
//! from the pre-migration version.
//!
//! # Known residual risks
//!
//! **A-12 — temp data-dir leak on SIGKILL:**
//! See `postgres_engine.rs` module-level doc for the full analysis. Same
//! posture here: SIGKILL bypasses the shared-engine fixture's `atexit`
//! teardown, which can leave orphaned postmaster processes / temp data dirs.
//! Accepted at Gate A.

mod common;
#[path = "common/shared_engine.rs"]
mod shared_engine;

use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use mnemra_host::storage::postgres::{self, PostgresStorage};
use std::sync::Arc;

/// Start a fresh `PostgresStorage` against a freshly-provisioned, isolated
/// test database on the binary-wide shared engine.
async fn start() -> PostgresStorage {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    postgres::bootstrap_records_table(&db.pool)
        .await
        .expect("bootstrap_records_table should succeed");
    PostgresStorage::new(Arc::new(db.pool))
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
