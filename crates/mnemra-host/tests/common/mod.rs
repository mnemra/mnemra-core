//! Shared contract harness for `Storage` adapters.
//!
//! # Extension point for Task 6
//!
//! Task 6 adds a Postgres adapter test by creating
//! `tests/storage_contract_postgres.rs` with:
//!
//! ```rust,ignore
//! mod common;
//! use mnemra_host::storage::postgres::PostgresStorage;
//!
//! #[tokio::test]
//! async fn postgres_atomicity_commit() {
//!     let s = PostgresStorage::start_embedded().await.unwrap();
//!     common::assert_atomicity_commit(s).await;
//! }
//!
//! #[tokio::test]
//! async fn postgres_atomicity_rollback() {
//!     let s = PostgresStorage::start_embedded().await.unwrap();
//!     common::assert_atomicity_rollback(s).await;
//! }
//!
//! #[tokio::test]
//! async fn postgres_isolation_no_cross_workspace_read() {
//!     let s = PostgresStorage::start_embedded().await.unwrap();
//!     common::assert_isolation_no_cross_workspace_read(s).await;
//! }
//!
//! #[tokio::test]
//! async fn postgres_isolation_no_cross_workspace_write() {
//!     let s = PostgresStorage::start_embedded().await.unwrap();
//!     common::assert_isolation_no_cross_workspace_write(s).await;
//! }
//! ```
//!
//! Each function below is one self-contained invariant assertion. Keep them
//! fine-grained so a partial failure points directly at the broken invariant.

use mnemra_host::storage::{Record, Storage, WorkspaceId};

// ---------------------------------------------------------------------------
// Invariant A: Atomic multi-write — commit direction
// ---------------------------------------------------------------------------

/// A committed transaction makes all its writes visible atomically.
///
/// Sequence:
/// 1. Open txn, write two records, commit.
/// 2. Open a second txn in the same workspace, list all records.
/// 3. Assert both records are present (commit-or-none — at least "all" half).
pub async fn assert_atomicity_commit<S: Storage>(storage: S) {
    let ws = WorkspaceId(1);

    let mut txn = storage.begin(ws).await.expect("begin failed");
    txn.put(Record {
        key: "k1".into(),
        value: b"v1".to_vec(),
    })
    .await
    .expect("put k1 failed");
    txn.put(Record {
        key: "k2".into(),
        value: b"v2".to_vec(),
    })
    .await
    .expect("put k2 failed");
    txn.commit().await.expect("commit failed");

    // Read back via a fresh transaction.
    let mut reader = storage.begin(ws).await.expect("begin reader failed");
    let records = reader.list().await.expect("list failed");

    let keys: Vec<&str> = records.iter().map(|r| r.key.as_str()).collect();
    assert!(
        keys.contains(&"k1"),
        "k1 should be visible after commit; got {keys:?}"
    );
    assert!(
        keys.contains(&"k2"),
        "k2 should be visible after commit; got {keys:?}"
    );
}

// ---------------------------------------------------------------------------
// Invariant A: Atomic multi-write — rollback direction
// ---------------------------------------------------------------------------

/// An abandoned (dropped without commit) transaction leaves no writes visible.
///
/// Sequence:
/// 1. Open txn, stage two writes — do NOT commit, drop instead.
/// 2. Open a second txn in the same workspace, list all records.
/// 3. Assert neither staged write is visible (rollback — "all-or-none").
pub async fn assert_atomicity_rollback<S: Storage>(storage: S) {
    let ws = WorkspaceId(2);

    {
        let mut txn = storage.begin(ws).await.expect("begin failed");
        txn.put(Record {
            key: "should_not_appear_1".into(),
            value: b"x".to_vec(),
        })
        .await
        .expect("put failed");
        txn.put(Record {
            key: "should_not_appear_2".into(),
            value: b"y".to_vec(),
        })
        .await
        .expect("put failed");
        // Drop without calling commit — this IS the rollback.
    }

    let mut reader = storage.begin(ws).await.expect("begin reader failed");
    let records = reader.list().await.expect("list failed");

    assert!(
        records.is_empty(),
        "rolled-back writes must not be visible; got {records:?}"
    );
}

// ---------------------------------------------------------------------------
// Invariant B: Workspace isolation — cross-workspace read returns zero rows
// ---------------------------------------------------------------------------

/// Rows written to workspace A are invisible from workspace B.
///
/// Sequence:
/// 1. Write and commit a record in workspace A.
/// 2. Open a transaction in workspace B, list records.
/// 3. Assert zero rows returned from workspace B's perspective.
pub async fn assert_isolation_no_cross_workspace_read<S: Storage>(storage: S) {
    let ws_a = WorkspaceId(10);
    let ws_b = WorkspaceId(11);

    // Write into workspace A.
    let mut txn_a = storage.begin(ws_a).await.expect("begin ws_a failed");
    txn_a
        .put(Record {
            key: "secret".into(),
            value: b"workspace_a_data".to_vec(),
        })
        .await
        .expect("put failed");
    txn_a.commit().await.expect("commit ws_a failed");

    // Read from workspace B — must see nothing.
    let mut txn_b = storage.begin(ws_b).await.expect("begin ws_b failed");
    let rows_b = txn_b.list().await.expect("list ws_b failed");

    assert!(
        rows_b.is_empty(),
        "workspace B must see zero rows from workspace A; got {rows_b:?}"
    );

    // Also verify direct get by key returns None from workspace B.
    let fetched = txn_b.get("secret").await.expect("get failed");
    assert!(
        fetched.is_none(),
        "workspace B must not be able to fetch workspace A's key by name"
    );
}

// ---------------------------------------------------------------------------
// Invariant B: Workspace isolation — cross-workspace write has no effect
// ---------------------------------------------------------------------------

/// A write in workspace B cannot mutate workspace A's committed rows.
///
/// Sequence:
/// 1. Write and commit a record (key="shared") in workspace A.
/// 2. Write and commit a different value for key="shared" from workspace B.
/// 3. Read key="shared" from workspace A — assert original value is intact.
pub async fn assert_isolation_no_cross_workspace_write<S: Storage>(storage: S) {
    let ws_a = WorkspaceId(20);
    let ws_b = WorkspaceId(21);

    // Establish initial state in workspace A.
    let mut txn_a = storage.begin(ws_a).await.expect("begin ws_a failed");
    txn_a
        .put(Record {
            key: "shared".into(),
            value: b"original".to_vec(),
        })
        .await
        .expect("put ws_a failed");
    txn_a.commit().await.expect("commit ws_a failed");

    // Workspace B writes a different value for the same key.
    let mut txn_b = storage.begin(ws_b).await.expect("begin ws_b failed");
    txn_b
        .put(Record {
            key: "shared".into(),
            value: b"tampered".to_vec(),
        })
        .await
        .expect("put ws_b failed");
    txn_b.commit().await.expect("commit ws_b failed");

    // Workspace A must still see the original value.
    let mut verify_a = storage.begin(ws_a).await.expect("begin verify_a failed");
    let record = verify_a
        .get("shared")
        .await
        .expect("get failed")
        .expect("key must still exist in workspace A");

    assert_eq!(
        record.value, b"original",
        "workspace A's row must not be mutated by workspace B's write"
    );
}
