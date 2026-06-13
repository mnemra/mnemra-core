//! Materialized-projection refresh queue (R-0001-f).
//!
//! # What this module provides
//!
//! - [`RefreshQueue`] — the enqueue handle; callers call `enqueue(view_name)`
//!   after a host-fn write completes.
//! - [`DrainHandle`] — the consumer handle; the worker calls `recv()` in a
//!   loop; tests call `drain_pending()` for a deterministic flush.
//! - [`new_refresh_queue`] — creates a linked `(RefreshQueue, DrainHandle)`
//!   pair backed by a tokio mpsc channel.
//!
//! # Drain semantics
//!
//! The worker calls `DrainHandle::recv()` in a loop; each call blocks until
//! one view name is available, then the worker issues
//! `REFRESH MATERIALIZED VIEW CONCURRENTLY <view_name>` and loops.
//!
//! For tests, `DrainHandle::drain_pending()` returns only after all currently
//! queued items have been received. Because the worker may issue the
//! CONCURRENTLY refresh asynchronously after `recv()` returns, the test must
//! also wait for the matview to reflect the write. The pattern in
//! `tests/artifact_machinery.rs` is:
//!
//! ```text
//! queue.enqueue("echo_fixture_status_counts").await;
//! drain.drain_pending().await;      // all items received by worker
//! // ... now assert matview reflects write
//! ```
//!
//! # CONCURRENTLY requirements (advisory)
//!
//! `REFRESH MATERIALIZED VIEW CONCURRENTLY` requires:
//! (a) a UNIQUE index on the matview.
//! (b) the matview was populated at creation time (`WITH DATA`, the default).
//! (c) the REFRESH must NOT run inside a transaction block — the worker issues
//!     it on a plain pool connection, not via `pool.begin()`.

use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

/// Default channel buffer: enough for typical host-fn write bursts.
const QUEUE_BUFFER: usize = 256;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The enqueue end of the refresh queue.
///
/// Cheaply cloneable (wraps an `Arc`-backed channel sender).
/// Call `enqueue(view_name)` after every host-fn write that modifies an
/// artifact table that has a materialized projection.
#[derive(Clone)]
pub struct RefreshQueue {
    sender: mpsc::Sender<String>,
}

impl RefreshQueue {
    /// Enqueue `view_name` for a `REFRESH MATERIALIZED VIEW CONCURRENTLY`.
    ///
    /// Returns `Err` only if the worker has been stopped (channel closed).
    /// Callers may ignore the error: a refresh failure is not a write failure.
    pub async fn enqueue(&self, view_name: impl Into<String>) -> Result<(), QueueClosedError> {
        self.sender
            .send(view_name.into())
            .await
            .map_err(|_| QueueClosedError)
    }
}

/// Error returned by `enqueue` when the worker has been stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueClosedError;

impl std::fmt::Display for QueueClosedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "refresh queue closed: worker has stopped")
    }
}

impl std::error::Error for QueueClosedError {}

/// The drain end of the refresh queue.
///
/// Owned by the worker. Tests may also hold a `DrainHandle` to call
/// `drain_pending()` for deterministic flush in integration tests.
pub struct DrainHandle {
    receiver: Arc<Mutex<mpsc::Receiver<String>>>,
}

impl DrainHandle {
    /// Receive the next view name queued for refresh.
    ///
    /// Returns `None` when all senders have been dropped (queue closed).
    pub async fn recv(&self) -> Option<String> {
        self.receiver.lock().await.recv().await
    }

    /// Drain all **currently queued** items by reading until the channel buffer
    /// is empty.
    ///
    /// This uses `try_recv` in a loop — it does NOT wait for items enqueued
    /// after this call returns. Callers should enqueue all items before calling
    /// this, then wait for the returned count to confirm drainage.
    ///
    /// Returns the list of view names that were drained.
    ///
    /// # Test usage
    ///
    /// ```rust,ignore
    /// queue.enqueue("my_view").await.unwrap();
    /// let drained = drain.drain_pending().await;
    /// assert!(drained.contains(&"my_view".to_string()));
    /// ```
    pub async fn drain_pending(&self) -> Vec<String> {
        let mut drained = Vec::new();
        let mut rx = self.receiver.lock().await;
        while let Ok(name) = rx.try_recv() {
            drained.push(name);
        }
        drained
    }
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

/// Create a linked `(RefreshQueue, DrainHandle)` pair.
///
/// The `RefreshQueue` is the enqueue end (cloneable, passed to write paths).
/// The `DrainHandle` is owned by the worker.
pub fn new_refresh_queue() -> (RefreshQueue, DrainHandle) {
    let (tx, rx) = mpsc::channel(QUEUE_BUFFER);
    let queue = RefreshQueue { sender: tx };
    let drain = DrainHandle {
        receiver: Arc::new(Mutex::new(rx)),
    };
    (queue, drain)
}

// ---------------------------------------------------------------------------
// Unit tests (pure — no engine)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn enqueue_and_drain_round_trip() {
        let (queue, drain) = new_refresh_queue();
        queue.enqueue("my_view").await.unwrap();
        queue.enqueue("other_view").await.unwrap();
        let drained = drain.drain_pending().await;
        assert_eq!(drained, vec!["my_view", "other_view"]);
    }

    #[tokio::test]
    async fn drain_empty_returns_empty() {
        let (_queue, drain) = new_refresh_queue();
        let drained = drain.drain_pending().await;
        assert!(drained.is_empty());
    }

    #[tokio::test]
    async fn closed_queue_returns_error() {
        let (queue, drain) = new_refresh_queue();
        drop(drain); // drop the receiver end
        let result = queue.enqueue("view").await;
        assert_eq!(result, Err(QueueClosedError));
    }

    #[tokio::test]
    async fn recv_returns_none_when_all_senders_dropped() {
        let (queue, drain) = new_refresh_queue();
        drop(queue);
        let item = drain.recv().await;
        assert!(item.is_none());
    }
}
