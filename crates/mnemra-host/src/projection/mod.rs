//! Projection refresh machinery (R-0001-f).
//!
//! # What this module provides
//!
//! - [`refresh_queue`] — in-memory refresh queue and drain handle.
//! - [`worker`] — tokio background worker that drains the queue.
//!
//! # Design: in-memory queue
//!
//! The V0 refresh queue is an in-memory `tokio::sync::mpsc` channel rather
//! than a DB-backed queue. This choice:
//!
//! - Avoids a schema_migrations version collision with the parallel
//!   `admin-token` branch (sibling branch awareness).
//! - Keeps the drain deterministic for tests (no timing-dependent polling).
//! - Provides a clear Task 19/23 extension point: swap `MemRefreshQueue` for
//!   a DB-backed queue without changing the worker API.
//!
//! The tradeoff is that in-flight refresh requests are lost on process restart.
//! At V0 the embedded engine restarts with the process so this is acceptable;
//! the V0.1 DB-backed queue is the production graduation path.

pub mod fixture_projection;
pub mod refresh_queue;
pub mod worker;
