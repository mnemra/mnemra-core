//! Task 9 self-tests: history shadow tables (R-0001-e) and refresh queue
//! (R-0001-f).
//!
//! # What these tests pin
//!
//! - **R-0001-e:** UPDATE writes a byte-exact history row; DELETE writes a
//!   `'DELETE'` history row *before* the artifact is removed.
//! - **R-0001-f:** A host-fn write enqueues a refresh; the worker drains it;
//!   the `echo_fixture_status_counts` matview reflects the write.
//!
//! # Engine acquisition
//!
//! Acquisition-migrated onto the shared-engine fixture (T3 sub-run,
//! R-0030/R-0029): each test acquires the binary-wide shared engine via
//! `shared_engine::shared_engine()` and provisions its own fresh, isolated
//! database via `EmbeddedEngine::provision_test_database()` — which runs the
//! same schema-init sequence `init()` used to run. No per-file
//! boot-serialization mutex needed — the fixture's own get-or-init
//! semantics guarantee exactly-once boot.
//!
//! # Synchronization (R-0001-f)
//!
//! R-0001-f tests use `drop(queue)` + `worker.stop()` for deterministic
//! synchronization: dropping the `RefreshQueue` closes the mpsc sender, which
//! causes the worker's `recv()` loop to return `None` after draining all
//! pending items. `worker.stop()` awaits the task join handle, so the matview
//! assertion runs only after all REFRESH queries have completed.
//!
//! The CONCURRENTLY test additionally uses `tokio::spawn` + `spawn_blocking` +
//! `std::sync::mpsc::recv_timeout` to enforce a 10-second deadline without
//! requiring the tokio `time` feature.

#[path = "common/shared_engine.rs"]
mod shared_engine;

use mnemra_host::projection::fixture_projection::{
    FIXTURE_MATVIEW, create_echo_fixture_projection,
};
use mnemra_host::projection::refresh_queue::new_refresh_queue;
use mnemra_host::projection::worker::RefreshWorker;
use mnemra_host::storage::postgres::engine::EmbeddedEngine;

/// The fixture type used by all tests in this file.
const FIXTURE_TYPE: &str = "echo_fixture";

/// A valid row that satisfies all CHECK constraints.
///
/// `frontmatter` contains both `id` and `frontmatter_version` keys (R-0001-c).
/// Used as the base row in history tests.
const VALID_FRONTMATTER: &str =
    r#"{"id": "TESTULID00000000099", "frontmatter_version": 1, "status": "open"}"#;
const VALID_FRONTMATTER_UPDATED: &str =
    r#"{"id": "TESTULID00000000099", "frontmatter_version": 2, "status": "done"}"#;

// ---------------------------------------------------------------------------
// Helper: insert a row into echo_fixture
// ---------------------------------------------------------------------------

/// Insert a single row into `echo_fixture` and return its id.
///
/// `frontmatter_version` is NOT an explicit column value: it is a
/// `GENERATED ALWAYS … STORED` projection of the JSONB `frontmatter_version`
/// key (R-0001-b), so the value flows from `frontmatter` and an explicit write
/// would be rejected.
async fn insert_fixture_row(pool: &sqlx::PgPool, id: &str, frontmatter: &str) {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {FIXTURE_TYPE} (id, workspace_id, type, frontmatter)
         VALUES (
             '{id}',
             '1b027423-a7e3-54ea-9e35-2e1a4afdf3d9',
             'echo_fixture',
             '{frontmatter}'
         )"
    )))
    .execute(pool)
    .await
    .expect("insert fixture row");
}

// ---------------------------------------------------------------------------
// R-0001-e tests: history shadow table
// ---------------------------------------------------------------------------

/// UPDATE must write a byte-exact copy of the prior frontmatter into
/// `echo_fixture_history` before the UPDATE is applied (R-0001-e).
///
/// Byte-exact: the `old_frontmatter::text` in the history row must equal the
/// pre-update frontmatter value's `::text` representation.
#[tokio::test]
async fn r0001e_update_writes_byte_exact_history_row() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let artifact_id = "TESTULID00000000091";
    insert_fixture_row(pool, artifact_id, VALID_FRONTMATTER).await;

    // UPDATE the frontmatter — the trigger fires BEFORE the UPDATE.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE {FIXTURE_TYPE}
         SET frontmatter = '{VALID_FRONTMATTER_UPDATED}'::jsonb,
             updated_at = now()
         WHERE id = '{artifact_id}'"
    )))
    .execute(pool)
    .await
    .expect("UPDATE should succeed");

    // The history table must contain exactly one row for this artifact.
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT operation, old_frontmatter::text
         FROM echo_fixture_history
         WHERE artifact_id = $1
         ORDER BY history_id",
    )
    .bind(artifact_id)
    .fetch_all(pool)
    .await
    .expect("history query");

    assert_eq!(
        rows.len(),
        1,
        "R-0001-e: expected exactly one history row after UPDATE, got {}",
        rows.len()
    );

    let (operation, old_fm_text) = &rows[0];
    assert_eq!(
        operation, "UPDATE",
        "R-0001-e: history row operation must be 'UPDATE'"
    );

    // Byte-exact check: compare the stored old_frontmatter (as text) to the
    // original frontmatter value. Postgres normalises JSONB on ingest (key
    // order may change), so we compare the DB-stored text of both values.
    let original_text: (String,) = sqlx::query_as("SELECT $1::jsonb::text")
        .bind(VALID_FRONTMATTER)
        .fetch_one(pool)
        .await
        .expect("normalise original frontmatter");

    assert_eq!(
        old_fm_text, &original_text.0,
        "R-0001-e: old_frontmatter in history row must match original frontmatter byte-for-byte \
         (JSONB text comparison); got {:?}, expected {:?}",
        old_fm_text, original_text.0
    );
}

/// DELETE must write a `'DELETE'` history row with the artifact's frontmatter
/// and body BEFORE the artifact row is removed (R-0001-e).
///
/// After the DELETE, the artifact row must be gone but the history row must
/// be present.
#[tokio::test]
async fn r0001e_delete_writes_history_row_before_removal() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let artifact_id = "TESTULID00000000092";
    insert_fixture_row(pool, artifact_id, VALID_FRONTMATTER).await;

    // DELETE the artifact — the BEFORE DELETE trigger fires first.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM {FIXTURE_TYPE} WHERE id = '{artifact_id}'"
    )))
    .execute(pool)
    .await
    .expect("DELETE should succeed");

    // The artifact must be gone.
    let artifact_row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM echo_fixture WHERE id = $1")
            .bind(artifact_id)
            .fetch_optional(pool)
            .await
            .expect("artifact existence check");

    assert!(
        artifact_row.is_none(),
        "R-0001-e: artifact must be removed after DELETE"
    );

    // The history row must exist with operation = 'DELETE'.
    let history_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT operation, old_frontmatter::text
         FROM echo_fixture_history
         WHERE artifact_id = $1",
    )
    .bind(artifact_id)
    .fetch_all(pool)
    .await
    .expect("history query after DELETE");

    assert_eq!(
        history_rows.len(),
        1,
        "R-0001-e: expected exactly one history row after DELETE, got {}",
        history_rows.len()
    );

    let (operation, old_fm_text) = &history_rows[0];
    assert_eq!(
        operation, "DELETE",
        "R-0001-e: history row operation must be 'DELETE'"
    );

    // Verify old_frontmatter matches (JSONB normalised comparison).
    let original_text: (String,) = sqlx::query_as("SELECT $1::jsonb::text")
        .bind(VALID_FRONTMATTER)
        .fetch_one(pool)
        .await
        .expect("normalise original frontmatter");

    assert_eq!(
        old_fm_text, &original_text.0,
        "R-0001-e: old_frontmatter in DELETE history row must match original frontmatter; \
         got {:?}, expected {:?}",
        old_fm_text, original_text.0
    );
}

/// UPDATE then DELETE should produce two history rows: one 'UPDATE' and one
/// 'DELETE' in that order (R-0001-e composite).
#[tokio::test]
async fn r0001e_update_then_delete_produces_two_history_rows() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let artifact_id = "TESTULID00000000093";
    insert_fixture_row(pool, artifact_id, VALID_FRONTMATTER).await;

    // UPDATE first.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE {FIXTURE_TYPE}
         SET frontmatter = '{VALID_FRONTMATTER_UPDATED}'::jsonb
         WHERE id = '{artifact_id}'"
    )))
    .execute(pool)
    .await
    .expect("UPDATE");

    // Then DELETE.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM {FIXTURE_TYPE} WHERE id = '{artifact_id}'"
    )))
    .execute(pool)
    .await
    .expect("DELETE");

    let history: Vec<(String,)> = sqlx::query_as(
        "SELECT operation FROM echo_fixture_history
         WHERE artifact_id = $1
         ORDER BY history_id",
    )
    .bind(artifact_id)
    .fetch_all(pool)
    .await
    .expect("history query");

    let ops: Vec<&str> = history.iter().map(|(op,)| op.as_str()).collect();
    assert_eq!(
        ops,
        vec!["UPDATE", "DELETE"],
        "R-0001-e: expected [UPDATE, DELETE] history in order, got {:?}",
        ops
    );
}

// ---------------------------------------------------------------------------
// R-0001-f tests: refresh queue + worker
// ---------------------------------------------------------------------------

/// A host-fn write enqueues a refresh; the worker drains it; the matview
/// reflects the write (R-0001-f).
///
/// Synchronisation: `drop(queue)` closes the mpsc sender so the worker's
/// `recv()` returns `None` after processing the pending item. `worker.stop()`
/// awaits the task join handle — the REFRESH is complete before this returns.
/// The matview assertion is therefore deterministic.
#[tokio::test]
async fn r0001f_enqueue_drain_matview_reflects_write() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    // Create the fixture matview.
    create_echo_fixture_projection(pool)
        .await
        .expect("create fixture projection");

    // Insert a row with status = "active".
    insert_fixture_row(pool, "TESTULID00000000094", VALID_FRONTMATTER).await;

    // Create the refresh queue and start the worker.
    let (queue, drain) = new_refresh_queue();
    let worker = RefreshWorker::start(drain, db.pool.clone());

    // Enqueue a refresh for the fixture matview.
    queue
        .enqueue(FIXTURE_MATVIEW)
        .await
        .expect("enqueue should succeed");

    // Give the worker time to receive and process the refresh.
    // We drop the queue (closing the sender), which causes the worker's recv()
    // to return None after the pending item is consumed, and then stop.
    drop(queue);
    worker.stop().await.expect("worker stopped cleanly");

    // The matview must now reflect the inserted row.
    let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT status, artifact_count FROM echo_fixture_status_counts ORDER BY status",
    )
    .fetch_all(pool)
    .await
    .expect("matview query");

    // We inserted one row with status = "open".
    assert!(
        !rows.is_empty(),
        "R-0001-f: matview must have rows after worker drains refresh queue"
    );

    let open_row = rows
        .iter()
        .find(|(status, _)| status.as_deref() == Some("open"));
    assert!(
        open_row.is_some(),
        "R-0001-f: matview must contain a row for status='open'; rows: {:?}",
        rows
    );
    let (_, count) = open_row.unwrap();
    assert_eq!(
        *count, 1,
        "R-0001-f: matview must show 1 artifact with status='open', got {}",
        count
    );
}

/// REFRESH MATERIALIZED VIEW CONCURRENTLY must not block concurrent reads
/// (R-0001-f non-blocking requirement).
///
/// # How this test distinguishes CONCURRENTLY from plain REFRESH
///
/// We hold an open PostgreSQL transaction with an `AccessShareLock` on the
/// matview (taken by a plain `SELECT` inside `BEGIN`).  PostgreSQL lock
/// compatibility for materialized views:
///
/// - `REFRESH MATERIALIZED VIEW CONCURRENTLY` → `ShareUpdateExclusiveLock`
///   (compatible with `AccessShareLock` — refresh proceeds)
/// - `REFRESH MATERIALIZED VIEW` (plain) → `AccessExclusiveLock`
///   (conflicts with `AccessShareLock` — refresh blocks until we release)
///
/// Note: `SELECT ... FOR SHARE` is invalid on materialized views (PG error
/// 42809); plain `SELECT` inside an open transaction is the correct mechanism.
///
/// We enqueue the refresh, then ask the worker to stop using a 10-second
/// deadline via `std::sync::mpsc::recv_timeout` on a signal channel driven by
/// `tokio::task::spawn_blocking`.  If the worker stops before the deadline, the
/// refresh used CONCURRENTLY.  If the deadline fires, plain REFRESH is blocking
/// on the held `AccessShareLock` — the test fails.
///
/// After the worker stops, we release the held transaction and verify the
/// matview reflects the write (value-level R-0001-f assertion).
///
/// # Why `spawn_blocking` + `std::sync::mpsc::recv_timeout`?
///
/// `tokio::time` is not enabled in the dev-dependency feature set and
/// `libs/mnemra-host/Cargo.toml` is out of this agent's edit scope.
/// `std::sync::mpsc::recv_timeout` provides a deadline on a OS thread without
/// requiring the tokio `time` feature.
#[tokio::test]
async fn r0001f_concurrent_refresh_does_not_block_reads() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    // Create projection.
    create_echo_fixture_projection(pool)
        .await
        .expect("create fixture projection");

    // Insert a row — the matview will need a refresh to reflect it.
    insert_fixture_row(pool, "TESTULID00000000095", VALID_FRONTMATTER).await;

    // Acquire AccessShareLock on the matview by opening a transaction with a
    // plain SELECT.  This lock is held until we ROLLBACK after the worker
    // completes.  CONCURRENTLY (ShareUpdateExclusiveLock) is compatible with
    // AccessShareLock; plain REFRESH (AccessExclusiveLock) is not.
    //
    // Note: SELECT ... FOR SHARE is invalid on materialized views (PG error
    // 42809), so we use a plain SELECT inside an open transaction.
    let mut hold_conn = pool.acquire().await.expect("acquire hold connection");
    sqlx::query("BEGIN")
        .execute(&mut *hold_conn)
        .await
        .expect("BEGIN on hold connection");
    sqlx::query("SELECT status, artifact_count FROM echo_fixture_status_counts")
        .fetch_all(&mut *hold_conn)
        .await
        .expect("SELECT to establish AccessShareLock on matview");

    // Start the worker and enqueue the refresh.
    let (queue, drain) = new_refresh_queue();
    let worker = RefreshWorker::start(drain, db.pool.clone());
    queue.enqueue(FIXTURE_MATVIEW).await.expect("enqueue");

    // Drop the queue (close sender) so the worker exits after processing the
    // pending refresh item.
    drop(queue);

    // Verify the worker stops within 10 seconds.  If the refresh used plain
    // REFRESH (no CONCURRENTLY), it blocks on our held RowShareLock and the
    // deadline fires.
    //
    // tokio::time is not available (not in dev-dep feature set and Cargo.toml
    // is out of scope).  Pattern: tokio::spawn drives the async worker.stop();
    // spawn_blocking awaits the result with a std::sync::mpsc deadline.
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    tokio::spawn(async move {
        let r = worker.stop().await.map_err(|e| e.to_string());
        tx.send(r).ok();
    });

    let stop_result =
        tokio::task::spawn_blocking(move || rx.recv_timeout(std::time::Duration::from_secs(10)))
            .await
            .expect("spawn_blocking completed");

    match stop_result {
        Ok(Ok(())) => { /* worker stopped cleanly and in time */ }
        Ok(Err(e)) => panic!("R-0001-f: worker stopped with error: {e}"),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            panic!(
                "R-0001-f: worker did not stop within 10 s — \
                 REFRESH MATERIALIZED VIEW CONCURRENTLY likely missing; \
                 plain REFRESH blocks on held RowShareLock"
            );
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            panic!("R-0001-f: stop signal channel disconnected unexpectedly");
        }
    }

    // Release the RowShareLock transaction.
    sqlx::query("ROLLBACK")
        .execute(&mut *hold_conn)
        .await
        .expect("ROLLBACK hold transaction");
    drop(hold_conn);

    // Value-level verification: matview must reflect the inserted row.
    let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT status, artifact_count FROM echo_fixture_status_counts ORDER BY status",
    )
    .fetch_all(pool)
    .await
    .expect("matview query after concurrent refresh");

    let open_row = rows
        .iter()
        .find(|(status, _)| status.as_deref() == Some("open"));
    assert!(
        open_row.is_some(),
        "R-0001-f: matview must contain a row for status='open' after CONCURRENTLY refresh; rows: {:?}",
        rows
    );
    let (_, count) = open_row.unwrap();
    assert_eq!(
        *count, 1,
        "R-0001-f: matview must show 1 artifact with status='open', got {}",
        count
    );
}
