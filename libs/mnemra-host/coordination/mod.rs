//! Shared privileged-write machinery for the coordination cluster (Task 3).
//!
//! The decision this module hides: **how a privileged coordination write is
//! made observably atomic** — the end-to-end timeout bound (pool-acquire
//! included, R-0074-b), the in-transaction audit-outbox composition (audit rows
//! staged on the same txn as the state transition, flushed inside the one
//! COMMIT, R-0075-c), and the fail-closed availability contract (a write that
//! cannot be verified written surfaces as a structured [`write_path::Unavailable`],
//! never empty-success, R-0074-a). Callers (the `claim`/`message` handlers,
//! Tasks 4/5/7) hand [`write_path::PgCoordinationStore::run_write`] a body that
//! performs the state transition and stages audit rows; the machinery owns the
//! guarantees.
//!
//! **Status (Task 3 sub-run c-green):** the guarantee behavior is implemented
//! (see [`write_path::PgCoordinationStore::run_write`]) — the timeout wrap,
//! outbox flush, fault check-sites, and `Unavailable` mapping are all live;
//! the fault-injection tests in `tests/coordination_failclosed.rs` pass
//! against this module.

pub mod audit;
pub mod write_path;

/// The R-0075-d unified-stream target tag. Every coordination `tracing` macro
/// carries `target: COORDINATION_TARGET` so the binary-owned subscriber can
/// filter the coordination op-log as one stream — see `write_path`'s private
/// `log_outcome` for the emission sites.
pub const COORDINATION_TARGET: &str = "mnemra::coordination";

/// Operation label for op-log attribution, span naming, and the timeout label
/// (R-0075-a). `#[non_exhaustive]` (F4): Tasks 5/7 add actions without a
/// breaking change.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinationOp {
    /// The bind call (`message poll` carrying a `role_instance`; R-0064-e).
    AttachBind,
    /// `claim acquire`.
    Acquire,
    /// `claim renew`.
    Renew,
    /// `claim release`.
    Release,
    /// `claim takeover`.
    Takeover,
    /// `message send`.
    Send,
    /// `message poll` (the delivery half of the bind call).
    Poll,
    /// `message ack`.
    Ack,
    /// `message disposition`.
    Disposition,
}
