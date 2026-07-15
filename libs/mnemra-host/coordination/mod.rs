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
pub mod leases;
pub mod message_types;
pub mod messages;
pub mod resource_id;
pub mod session_plane;
pub mod write_path;

use std::time::Duration;

/// The R-0075-d unified-stream target tag. Every coordination `tracing` macro
/// carries `target: COORDINATION_TARGET` so the binary-owned subscriber can
/// filter the coordination op-log as one stream — see `write_path`'s private
/// `log_outcome` for the emission sites.
pub const COORDINATION_TARGET: &str = "mnemra::coordination";

/// Coordination-cluster runtime configuration (spec §Numeric calibrations).
///
/// Threaded from [`crate::RunConfig`] through `run_with` into the MCP server.
/// Every field is a config-set default — the *existence and enforcement* are
/// the lock, the numbers are dogfood-scale defaults tunable at startup and, for
/// acceptance tests, overridable to the sanctioned second-scale values (test
/// calibration). No client-settable per-request override exists for these
/// (they stay deployment-scoped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoordinationConfig {
    /// End-to-end coordination-write bound (R-0074-b) — the
    /// [`write_path::PgCoordinationStore::run_write`] timeout, pool-acquire
    /// included. Default 10 s (§Numeric calibrations).
    pub write_timeout: Duration,
    /// Attachment idle-expiry TTL (R-0064-d), security-load-bearing (TB-3): the
    /// ceiling on successor-lockout and the floor on the impersonation-after-
    /// death window. The attach body computes `expires_at = store_now +
    /// attachment_ttl`. Default 10 minutes (§Numeric calibrations). Its
    /// production value is the subject of the sub-run-e startup guard.
    pub attachment_ttl: Duration,
}

impl Default for CoordinationConfig {
    fn default() -> Self {
        CoordinationConfig {
            write_timeout: Duration::from_secs(10),
            attachment_ttl: Duration::from_secs(600),
        }
    }
}

/// `claim acquire` lease duration when `duration_seconds` is omitted
/// (R-0065-d). Default 15 minutes (§Numeric calibrations).
///
/// off-default note: a module const, not a `CoordinationConfig` field. The
/// config-field route (`CoordinationConfig` gaining `lease_default_duration`/
/// `lease_max_duration`) would compile-break every existing
/// `CoordinationConfig { .. }` struct literal that does not use
/// `..Default::default()` — two such literals live in
/// `mnemra_host.rs`'s test module, outside this dispatch's `touch_scope`.
/// The b1 acceptance suite's own header comment confirms the assumption
/// this rides: "no scenario in b1 needs to wait out an actual lease expiry
/// … `CoordinationConfig` therefore needed no changes for this file." Flagged
/// for revisit: fold into `CoordinationConfig` (with the `mnemra_host.rs`
/// literals migrated to `..Default::default()`) when a later slice needs
/// per-test duration override (dogfooder-default: simple + sufficient for
/// b1's V0 usage).
pub const LEASE_DEFAULT_DURATION: Duration = Duration::from_secs(900);

/// `claim acquire` policy-maximum lease duration — a request above this
/// bound is refused `invalid_duration` (R-0065-d). Default 4 hours
/// (§Numeric calibrations). Same off-default rationale as
/// [`LEASE_DEFAULT_DURATION`].
pub const LEASE_MAX_DURATION: Duration = Duration::from_secs(14_400);

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
    /// `claim list` (Task 5 b2) — a READ, never routed through `run_write`;
    /// logged via [`write_path::log_read_outcome`] instead of
    /// [`write_path::PgCoordinationStore::run_write`]'s own emission.
    ClaimList,
    /// `message send`.
    Send,
    /// `message poll` (the delivery half of the bind call).
    Poll,
    /// `message ack`.
    Ack,
    /// `message disposition`.
    Disposition,
}
