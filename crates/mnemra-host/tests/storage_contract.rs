//! Two-adapter contract test — in-memory adapter.
//!
//! Exercises the two co-equal invariants from P-0010 D5 against the in-memory
//! adapter. The same invariant functions in `common` are re-invoked by Task 6
//! against the Postgres adapter (`tests/storage_contract_postgres.rs`).
//!
//! # Invariants under test
//!
//! **A — Atomic multi-write:**
//! - Commit direction: all staged writes become visible atomically.
//! - Rollback direction: no staged writes are visible after drop-without-commit.
//!
//! **B — Workspace-scoped isolation:**
//! - Cross-workspace read returns zero rows.
//! - Cross-workspace write does not mutate the target workspace's rows.

mod common;

use mnemra_host::storage::memory::MemStorage;

// ---------------------------------------------------------------------------
// Invariant A — Atomicity
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mem_atomicity_commit() {
    common::assert_atomicity_commit(MemStorage::new()).await;
}

#[tokio::test]
async fn mem_atomicity_rollback() {
    common::assert_atomicity_rollback(MemStorage::new()).await;
}

// ---------------------------------------------------------------------------
// Invariant B — Isolation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mem_isolation_no_cross_workspace_read() {
    common::assert_isolation_no_cross_workspace_read(MemStorage::new()).await;
}

#[tokio::test]
async fn mem_isolation_no_cross_workspace_write() {
    common::assert_isolation_no_cross_workspace_write(MemStorage::new()).await;
}
