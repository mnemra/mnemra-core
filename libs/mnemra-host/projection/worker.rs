//! Background refresh worker (R-0001-f).
//!
//! # What this module provides
//!
//! [`RefreshWorker`] — a tokio task handle wrapping a background drain loop.
//! The worker receives view names from a [`DrainHandle`] and issues
//! `REFRESH MATERIALIZED VIEW CONCURRENTLY <view_name>` against the pool.
//!
//! # Lifecycle
//!
//! ```text
//! let (queue, drain) = new_refresh_queue();
//! let worker = RefreshWorker::start(drain, pool.clone());
//! // ... use queue to enqueue view names ...
//! worker.stop().await;
//! ```
//!
//! Tests own start/stop. Task 23 wires real startup into the host runtime.
//!
//! # CONCURRENTLY constraint
//!
//! `REFRESH MATERIALIZED VIEW CONCURRENTLY` must NOT run inside a transaction
//! block. The worker uses `pool.execute(...)` directly — no `pool.begin()` —
//! so the refresh runs in autocommit mode.
//!
//! # Non-blocking reads
//!
//! Because the refresh uses `CONCURRENTLY`, it does not take an exclusive lock
//! on the matview and concurrent SELECT queries proceed uninterrupted.
//!
//! # Worker termination
//!
//! The worker loop exits when the `DrainHandle`'s `recv()` returns `None` (all
//! senders dropped). `RefreshWorker::stop()` drops the `RefreshQueue` (closing
//! the sender side) and awaits the task.

use sqlx::PgPool;
use tokio::task::JoinHandle;

use crate::projection::refresh_queue::DrainHandle;

/// Error returned if the worker task panicked.
#[derive(Debug)]
pub struct WorkerPanicError(String);

impl std::fmt::Display for WorkerPanicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "refresh worker panicked: {}", self.0)
    }
}

impl std::error::Error for WorkerPanicError {}

/// Background refresh worker.
///
/// Drains view names from a [`DrainHandle`] and issues
/// `REFRESH MATERIALIZED VIEW CONCURRENTLY` for each.
pub struct RefreshWorker {
    handle: JoinHandle<()>,
}

impl RefreshWorker {
    /// Start the background drain worker.
    ///
    /// Spawns a tokio task that loops on `drain.recv()` until the queue closes.
    /// Each received view name triggers a CONCURRENTLY refresh against `pool`.
    ///
    /// # Parameters
    ///
    /// - `drain` — the receive end of the refresh queue; the worker takes
    ///   ownership.
    /// - `pool` — a sqlx `PgPool` connected as the app role. The worker uses
    ///   this pool for refresh queries.
    pub fn start(drain: DrainHandle, pool: PgPool) -> Self {
        let handle = tokio::spawn(async move {
            while let Some(view_name) = drain.recv().await {
                tracing::debug!(view = %view_name, "refresh worker: dispatching REFRESH MATERIALIZED VIEW CONCURRENTLY");
                // REFRESH MATERIALIZED VIEW CONCURRENTLY must not run inside a
                // transaction — use pool.execute directly (autocommit).
                //
                // Safety: view_name comes from RefreshQueue::enqueue() callers
                // inside this crate. At V0 the only caller is create_fixture_projection
                // which passes a hardcoded constant view name. If this is extended
                // to accept plugin-provided names, apply validate_type_name first.
                let sql = format!("REFRESH MATERIALIZED VIEW CONCURRENTLY {}", view_name);
                if let Err(e) = sqlx::query(sqlx::AssertSqlSafe(sql)).execute(&pool).await {
                    // Log and continue — a refresh failure must not crash the
                    // worker or fail the originating write.
                    tracing::warn!(view = %view_name, error = %e, "REFRESH MATERIALIZED VIEW CONCURRENTLY failed");
                }
            }
        });

        RefreshWorker { handle }
    }

    /// Stop the worker and wait for it to drain.
    ///
    /// Callers should drop all `RefreshQueue` senders before calling this so
    /// the worker exits naturally (recv returns None). This method then awaits
    /// the join handle.
    pub async fn stop(self) -> Result<(), WorkerPanicError> {
        self.handle.await.map_err(|e| {
            WorkerPanicError(if e.is_panic() {
                format!("{:?}", e)
            } else {
                "task cancelled".to_string()
            })
        })
    }
}
