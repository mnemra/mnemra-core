//! The privileged coordination-write seam and its transaction handle.
//!
//! Decision hidden: **the end-to-end shape of a guaranteed coordination write.**
//! [`PgCoordinationStore::run_write`] owns the timeout bound, the outbox-audit
//! flush, and the fail-closed [`Unavailable`] mapping; the caller's body owns
//! only the state transition and audit staging. The scoped-closure shape (the
//! body runs on a machinery-owned txn) is what lets a single
//! `tokio::time::timeout` cancel the whole write — pool acquire included — in
//! green; a two-call begin/commit API could not enforce that end-to-end bound.
//!
//! **Status (Task 3 sub-run c-green):** the guarantees are filled. `run_write`
//! wraps `{ begin → body → flush audit → commit }` in one
//! `tokio::time::timeout(write_timeout, …)` (pool-acquire included in the
//! bound, R-0074-b); a `Commit` body's staged audit rows are flushed into the
//! `coordination_audit` outbox on the SAME txn before the one COMMIT
//! (R-0075-c emit-guarantee — a flush failure rolls the whole txn back); a
//! `Refuse` rolls back with no audit; a body [`StorageFailure`] or an elapsed
//! bound surfaces as a structured [`Unavailable`], never silent (R-0074-a).
//! Every outcome (commit, refusal, unavailable) emits one op-log entry
//! (R-0075-a) via [`log_outcome`], carrying the acting coordination actor
//! (via [`CoordinationTxn::record_acting_actor`], additive seam,
//! maintainer-adjudicated per the decision brief cited on `log_outcome`) when
//! the body resolved one, alongside the retained `token_id` context field.

use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool, Postgres};
use uuid::Uuid;

use crate::auth::workspace_ctx::WorkspaceCtx;
use crate::coordination::COORDINATION_TARGET;
use crate::coordination::CoordinationOp;
use crate::coordination::audit::AuditRecord;

/// A boxed, `Send` future borrowing for `'a` — the return shape of a `run_write`
/// body. Defined locally so the seam needs no `futures` dependency.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Outcome of a coordination write body.
#[derive(Debug, Clone, PartialEq)]
pub enum WriteResult<T> {
    /// State transition + staged audit committed atomically.
    Commit(T),
    /// A structured refusal (R-0065-c etc.): rolled back, op-logged, no audit.
    Refuse(Refusal),
}

/// The closed refusal grammar (spec §API Contract). A refusal is a completed,
/// understood operation the substrate declined — DISTINCT from [`Unavailable`]
/// (a write that could not be verified). Closed by design: an out-of-enum code
/// is a spec amendment, so this enum is intentionally NOT `#[non_exhaustive]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The resource has a live holder — carries the holder + expiry (the
    /// workspace-visible facts, C8 read default; R-0065-c).
    ResourceHeld {
        holder_actor_id: Uuid,
        expires_at: DateTime<Utc>,
    },
    /// `renew`/`release` by a non-holder (R-0065-d).
    NotHolder,
    /// `takeover` against a still-live lease (R-0066-a).
    NotExpired,
    /// No matching live lease (expired/released/never-existed; R-0065-d).
    LeaseNotFound,
    /// `acquire` duration above the policy maximum, or zero/negative (R-0065-d).
    InvalidDuration,
    /// Malformed / out-of-family resource identifier.
    InvalidResource,
    /// The reserved `actor:` family, barred from every `claim` action (R-0067-c).
    ReservedFamily,
    /// The session holds no live attachment (R-0064-e).
    NotAttached,
    /// Attach to an actor that already has a live attachment (R-0064-c).
    ActorLiveAttached,
    /// A re-`poll` with a different `role_instance` than the session bound to
    /// (R-0064-e).
    AttachmentMismatch,
    /// Attach resolved a `human`/`system` actor row (R-0064-c).
    WrongActorType,
    /// The `role_instance` failed the one host-registered identifier rule
    /// (R-0064-a).
    InvalidRoleInstance,
    /// `ack`/`disposition` by a caller who is not the message's addressee
    /// (R-0069-b).
    NotAddressee,
    /// A message state transition out of order (R-0068/-0069).
    InvalidTransition,
    /// A `disposition` value outside the closed set (R-0069).
    InvalidDisposition,
    /// A `send` payload violated its named type + schema-version (R-0070-b).
    SchemaViolation,
    /// A `send` naming an unknown message type / version (R-0070-b).
    UnknownType,
    /// `ack`/`disposition` naming a message id that does not exist (R-0069).
    MessageNotFound,
}

impl Refusal {
    /// The closed, machine-readable reason code — spec §API Contract's
    /// `reason_code` enum, verbatim. Carried on every refusal's op-log entry
    /// (R-0075-a) and intended to double as the client-facing response code.
    pub fn reason_code(&self) -> &'static str {
        match self {
            Refusal::ResourceHeld { .. } => "resource_held",
            Refusal::NotHolder => "not_holder",
            Refusal::NotExpired => "not_expired",
            Refusal::LeaseNotFound => "lease_not_found",
            Refusal::InvalidDuration => "invalid_duration",
            Refusal::InvalidResource => "invalid_resource",
            Refusal::ReservedFamily => "reserved_family",
            Refusal::NotAttached => "not_attached",
            Refusal::ActorLiveAttached => "actor_live_attached",
            Refusal::AttachmentMismatch => "attachment_mismatch",
            Refusal::WrongActorType => "wrong_actor_type",
            Refusal::InvalidRoleInstance => "invalid_role_instance",
            Refusal::NotAddressee => "not_addressee",
            Refusal::InvalidTransition => "invalid_transition",
            Refusal::InvalidDisposition => "invalid_disposition",
            Refusal::SchemaViolation => "schema_violation",
            Refusal::UnknownType => "unknown_type",
            Refusal::MessageNotFound => "message_not_found",
        }
    }
}

/// The structured "cannot verify the write" state (R-0074-b) — DISTINCT from a
/// [`Refusal`]. The client surfaces it as a stop (R-0074-a): never
/// empty-success, never silent retry.
#[derive(Debug)]
pub enum Unavailable {
    /// The end-to-end write bound elapsed (pool acquire included).
    Timeout { op: CoordinationOp, bound: Duration },
    /// A storage acquire / begin / commit failure.
    Store(Box<dyn Error + Send + Sync>),
}

impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unavailable::Timeout { op, bound } => write!(
                f,
                "coordination write unavailable: {op:?} exceeded the {bound:?} end-to-end bound"
            ),
            Unavailable::Store(e) => {
                write!(f, "coordination write unavailable: storage failure: {e}")
            }
        }
    }
}

impl Error for Unavailable {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Unavailable::Timeout { .. } => None,
            Unavailable::Store(e) => Some(e.as_ref()),
        }
    }
}

/// Body-internal storage error. The machinery maps it to [`Unavailable::Store`],
/// keeping it DISTINCT from [`Refusal`] so a body cannot launder a real storage
/// failure as a refusal (or vice-versa).
#[derive(Debug)]
pub struct StorageFailure(pub Box<dyn Error + Send + Sync>);

/// Fault injected on the coordination write path (test seam, R-0074-b/R-0075-c).
///
/// Sibling to the startup [`crate::InjectedFailure`] (the startup enum's
/// contract is boundary-specific; these fire on the write path).
/// `test-hooks`-gated so the seam is unreachable in the default build;
/// `tests/no_test_seams.rs` is extended to assert the check-sites do not compile
/// in (sub-run c).
///
/// **Skeleton status:** the variants exist so the red tests can name them, but
/// the machinery does NOT consult `injected_fault` yet (check-sites are green
/// work, sub-run c) — injecting a fault has no effect, so the fault-injection
/// tests fail by design.
#[cfg(feature = "test-hooks")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinationFault {
    /// Force the outbox audit flush to fail (R-0075-c): green rolls the whole
    /// txn back → the state transition is provably absent on recovery.
    AuditEmitFail,
    /// Force the store acquire/begin to fail (R-0074): green returns
    /// [`Unavailable::Store`] within the bound.
    StoreUnavailable,
}

/// The privileged coordination-write seam. V0 realization: Postgres-coupled.
//
// off-default: Postgres-coupled coordination seam for V0; a second-engine
// abstraction lands here. The K/V `Storage` trait cannot express a
// uniqueness-constrained lease insert or the outbox composition, so this seam
// operates on a concrete `sqlx::Transaction<Postgres>` (follows the merged
// Task-2 precedent — leases/messages are raw Postgres DDL in schema/init.rs).
// This is NOT an abstract coordination-store trait; the coupling is deliberate
// and tripwired — when a second engine arrives, the abstraction lands at THIS
// seam. (Named in the deviation report.)
pub struct PgCoordinationStore {
    pool: Arc<PgPool>,
    /// End-to-end write bound (config-set, not a const — two-tier config lock +
    /// CI-speed AC testability). The `tokio::time::timeout` bound `run_write`
    /// wraps `{ begin → body → flush → commit }` in (R-0074-b).
    write_timeout: Duration,
    /// Injected write-path fault (test seam), consulted by `run_write`'s fault
    /// check-sites.
    #[cfg(feature = "test-hooks")]
    injected_fault: Option<CoordinationFault>,
}

impl PgCoordinationStore {
    /// Construct a store over `pool` with the end-to-end write bound
    /// `write_timeout` (default 10 s; §Numeric calibrations).
    pub fn new(pool: Arc<PgPool>, write_timeout: Duration) -> Self {
        PgCoordinationStore {
            pool,
            write_timeout,
            #[cfg(feature = "test-hooks")]
            injected_fault: None,
        }
    }

    /// Inject a coordination fault (test seam). Mirrors
    /// [`crate::RunConfig::with_injected_failure`].
    #[cfg(feature = "test-hooks")]
    pub fn with_injected_fault(mut self, fault: CoordinationFault) -> Self {
        self.injected_fault = Some(fault);
        self
    }

    /// Run a privileged coordination write end-to-end.
    ///
    /// `op` names the operation (op-log + timeout label). `body` performs the
    /// state transition on the handed-in txn and returns a [`WriteResult`],
    /// staging any audit rows via [`CoordinationTxn::stage_audit`].
    ///
    /// Shape: `{ begin → body → flush staged audit → commit }`, the whole
    /// thing wrapped in one `tokio::time::timeout(self.write_timeout, …)` —
    /// pool-acquire (`begin`) is INSIDE the bound, so the coordination path's
    /// own timeout dominates a slow/unreachable storage pool rather than
    /// inheriting the pool's own (possibly much longer) acquire timeout
    /// (R-0074-b). Outcomes:
    /// - `Commit`: staged audit flushed into the `coordination_audit` outbox
    ///   on the SAME txn, then ONE commit (R-0075-c outbox composition). A
    ///   flush failure (real, or the injected `AuditEmitFail` fault) means the
    ///   txn is never committed — it rolls back on drop, so the state
    ///   transition and the audit are both absent (the emit-guarantee).
    /// - `Refuse`: rolled back (txn drop), no audit — a refusal is a
    ///   completed, understood decision, distinct from [`Unavailable`].
    /// - A body [`StorageFailure`], a `begin`/commit/flush storage error, the
    ///   injected `StoreUnavailable` fault (short-circuits before `begin` —
    ///   nothing is ever committed), or an elapsed bound: surfaces as a
    ///   structured [`Unavailable`], never silent (R-0074-a). The elapsed-bound
    ///   case is the only source of `Unavailable::Timeout`; every other path
    ///   maps to `Unavailable::Store`.
    ///
    /// Every outcome (commit, refusal, unavailable) emits one op-log entry via
    /// [`log_outcome`] before returning (R-0075-a, refusals included).
    pub async fn run_write<T, F>(
        &self,
        ctx: &WorkspaceCtx,
        op: CoordinationOp,
        body: F,
    ) -> Result<WriteResult<T>, Unavailable>
    where
        F: for<'t> FnOnce(
            &'t mut CoordinationTxn,
        ) -> BoxFuture<'t, Result<WriteResult<T>, StorageFailure>>,
    {
        let workspace_id = ctx.workspace_id();
        let token_id = ctx.token_id;

        // Fault check-site: force the store unavailable BEFORE any txn begins
        // — nothing is ever committed, no audit row lands. Mirrors the
        // `RunConfig`/`InjectedFailure` check-site style in `mnemra_host.rs`
        // (an early-return `Box<dyn Error + Send + Sync>` built from a `&str`,
        // gated identically to the seam it consults).
        #[cfg(feature = "test-hooks")]
        if self.injected_fault == Some(CoordinationFault::StoreUnavailable) {
            let injected: Box<dyn Error + Send + Sync> =
                "test-hooks: injected coordination store-unavailable fault \
                 (CoordinationFault::StoreUnavailable) — no txn was opened"
                    .into();
            let result = Err(Unavailable::Store(injected));
            // No `tx` was ever opened here — nothing for a body to have
            // recorded an actor into.
            log_outcome(op, workspace_id, token_id, None, &result);
            return result;
        }

        #[cfg(feature = "test-hooks")]
        let audit_fail_injected = self.injected_fault == Some(CoordinationFault::AuditEmitFail);

        // Alongside the write's Result, the wrapped future also surfaces the
        // acting actor the body recorded via `CoordinationTxn::record_acting_actor`
        // (R-0075-a) — read back from `tx` after the body returns, before `tx`
        // is (possibly) partially consumed by `flush_staged_audit`/`commit`.
        // `None` when the body never recorded one (pre-resolution refusal) OR
        // when the bound elapsed / the store was unavailable before `begin`
        // ever produced a `tx` to record into.
        let attempt = tokio::time::timeout(self.write_timeout, async move {
            let mut tx = match self.pool.begin().await {
                Ok(txn) => CoordinationTxn {
                    txn,
                    workspace_id,
                    staged_audit: Vec::new(),
                    acting_actor: None,
                },
                Err(e) => return (Err(Unavailable::Store(Box::new(e))), None),
            };

            match body(&mut tx).await {
                Ok(WriteResult::Commit(value)) => {
                    let acting_actor = tx.acting_actor;
                    // Fault check-site: force the outbox flush to fail — the
                    // whole txn (state transition included) rolls back on
                    // drop, never flushed, never committed (R-0075-c
                    // emit-guarantee).
                    #[cfg(feature = "test-hooks")]
                    if audit_fail_injected {
                        let injected: Box<dyn Error + Send + Sync> =
                            "test-hooks: injected coordination audit-emit failure \
                             (CoordinationFault::AuditEmitFail) — outbox flush forced \
                             to fail, whole txn rolled back"
                                .into();
                        return (Err(Unavailable::Store(injected)), acting_actor);
                    }
                    match tx.flush_staged_audit().await {
                        Ok(_flushed) => match tx.txn.commit().await {
                            Ok(()) => (Ok(WriteResult::Commit(value)), acting_actor),
                            Err(e) => (Err(Unavailable::Store(Box::new(e))), acting_actor),
                        },
                        // Flush failed: DO NOT commit. `tx` drops here — the
                        // txn rolls back, taking the state transition with it.
                        Err(e) => (Err(Unavailable::Store(Box::new(e))), acting_actor),
                    }
                }
                // A refusal is a completed, understood decision the substrate
                // declined — rollback (implicit txn drop), no audit (refusals
                // are not in the R-0075-b privileged subset), NOT
                // `Unavailable`. Still surfaces `acting_actor` if the body
                // resolved one before refusing (R-0075-a: refusals logged).
                Ok(WriteResult::Refuse(refusal)) => {
                    (Ok(WriteResult::Refuse(refusal)), tx.acting_actor)
                }
                // A body storage failure maps to `Unavailable::Store` —
                // rollback (implicit txn drop).
                Err(failure) => (Err(Unavailable::Store(failure.0)), tx.acting_actor),
            }
        })
        .await;

        let (result, acting_actor) = match attempt {
            Ok(inner) => inner,
            // The end-to-end bound elapsed (pool-acquire included, since
            // `begin` runs inside the wrapped future) — the coordination
            // path's own timeout, never the storage pool's. The whole
            // in-flight future (any `tx` it held) was cancelled, so whatever
            // the body may have recorded is unrecoverable — `None` is the
            // honest attribution here.
            Err(_elapsed) => (
                Err(Unavailable::Timeout {
                    op,
                    bound: self.write_timeout,
                }),
                None,
            ),
        };

        log_outcome(op, workspace_id, token_id, acting_actor, &result);
        result
    }
}

/// Op-log emission (R-0075-a): every coordination operation — including every
/// refusal — emits one structured event on the [`COORDINATION_TARGET`] stream
/// carrying `op`, `workspace_id`, `token_id`, `actor_id`, `outcome`, and (for
/// refusals and unavailability) a machine-readable `reason_code`. The event's
/// timestamp rides the subscriber layer, per the rest of this crate's
/// `tracing` call sites (P-0011 facade; no manual timestamp field).
///
/// `token_id` is the calling admin token's id ([`WorkspaceCtx::token_id`]) —
/// the session/token context, retained on every entry regardless of actor
/// resolution. `actor_id` is the **acting coordination actor**
/// (maintainer-adjudicated R-0075-a reading, decision brief
/// `brain/projects/mnemra/2026-07-12-r0075a-oplog-actor-attribution-decision.md`):
/// an `Option<Uuid>`, `Some` when the body recorded one via
/// [`CoordinationTxn::record_acting_actor`] before this call, `None` when it
/// did not (a body often MINTS the coordination-specific actor mid-write,
/// e.g. registration, so that identity cannot be known before the body runs
/// — and a pre-resolution refusal never resolves one at all). Per-actor
/// attribution for the R-0075-b privileged subset ALSO rides the
/// [`AuditRecord`] staged via [`CoordinationTxn::stage_audit`] — a DIFFERENT,
/// narrower mechanism (privileged writes only, in-txn outbox) from this
/// op-log (every operation, fire-and-forget after the txn resolves, R-0075-a
/// vs R-0075-b/-c).
///
/// Every call site below uses a static string literal as the `tracing`
/// message and carries all dynamic content as separate `field = value`
/// arguments (R-0075-e log-field hygiene) — never `format!`/positional
/// interpolation of payload or error content into the message text.
fn log_outcome<T>(
    op: CoordinationOp,
    workspace_id: Uuid,
    token_id: Uuid,
    acting_actor: Option<Uuid>,
    result: &Result<WriteResult<T>, Unavailable>,
) {
    match result {
        Ok(WriteResult::Commit(_)) => {
            tracing::info!(
                target: COORDINATION_TARGET,
                op = ?op,
                workspace_id = %workspace_id,
                token_id = %token_id,
                actor_id = ?acting_actor,
                outcome = "commit",
                "coordination write committed"
            );
        }
        Ok(WriteResult::Refuse(refusal)) => {
            tracing::info!(
                target: COORDINATION_TARGET,
                op = ?op,
                workspace_id = %workspace_id,
                token_id = %token_id,
                actor_id = ?acting_actor,
                outcome = "refused",
                reason_code = refusal.reason_code(),
                "coordination write refused"
            );
        }
        Err(Unavailable::Timeout { bound, .. }) => {
            tracing::warn!(
                target: COORDINATION_TARGET,
                op = ?op,
                workspace_id = %workspace_id,
                token_id = %token_id,
                actor_id = ?acting_actor,
                outcome = "unavailable",
                reason_code = "timeout",
                bound = ?bound,
                "coordination write unavailable: end-to-end bound elapsed"
            );
        }
        Err(Unavailable::Store(e)) => {
            tracing::warn!(
                target: COORDINATION_TARGET,
                op = ?op,
                workspace_id = %workspace_id,
                token_id = %token_id,
                actor_id = ?acting_actor,
                outcome = "unavailable",
                reason_code = "store",
                error = %e,
                "coordination write unavailable: storage failure"
            );
        }
    }
}

/// The transaction handle a [`PgCoordinationStore::run_write`] body writes
/// through. Owns the live Postgres txn; audit rows staged here are flushed by
/// the machinery inside the commit txn (green) and discarded on rollback.
///
/// The txn is `'static` because `PgPool::begin` hands back an owned pooled
/// connection — so the handle needs no lifetime parameter.
pub struct CoordinationTxn {
    // §Q2: Postgres-coupled at V0 (tripwired at the store seam above).
    txn: sqlx::Transaction<'static, Postgres>,
    workspace_id: Uuid,
    staged_audit: Vec<AuditRecord>,
    /// The acting coordination actor, recorded by the body once
    /// resolved/minted (via [`Self::record_acting_actor`]). Read by
    /// [`PgCoordinationStore::run_write`] and surfaced on the op-log entry
    /// (R-0075-a actor attribution). `None` when the body never resolved an
    /// actor (e.g. a pre-resolution refusal) — see `record_acting_actor` doc.
    acting_actor: Option<Uuid>,
}

impl CoordinationTxn {
    /// Buffer an audit row for the machinery to flush inside the commit txn
    /// (outbox composition, R-0075-c). Staged rows are discarded on rollback.
    pub fn stage_audit(&mut self, record: AuditRecord) {
        self.staged_audit.push(record);
    }

    /// Record the acting coordination actor for this write, once the body has
    /// resolved or minted it (R-0075-a actor attribution, maintainer-adjudicated
    /// — decision brief `brain/projects/mnemra/2026-07-12-r0075a-oplog-actor-attribution-decision.md`).
    ///
    /// The machinery (`run_write`) cannot know the acting actor at op-log time
    /// — the body often mints it mid-write (e.g. registration) — so this is an
    /// additive seam: the body calls it once the actor is known, and
    /// `run_write` reads it back after the body returns to include it on the
    /// [`log_outcome`] entry. DISTINCT from [`Self::stage_audit`]'s
    /// `AuditRecord.actor_id`: that is the R-0075-b/-c privileged-subset
    /// outbox; this is the R-0075-a op-log surface, required on every
    /// operation (successes AND refusals).
    ///
    /// Not called (actor stays `None`) for ops with no resolved acting actor —
    /// pre-resolution refusals (e.g. `invalid_role_instance` before any row is
    /// minted, `not_attached` with no attached actor). R-0075-a cannot require
    /// an actor that does not exist; `None` plus the machinery-retained
    /// `token_id` is the honest attribution for those. Last call wins if
    /// invoked more than once in one write.
    pub fn record_acting_actor(&mut self, actor_id: Uuid) {
        self.acting_actor = Some(actor_id);
    }

    /// Raw parameterized-SQL access for the state transition (the body's
    /// lease/message/attachment INSERT/UPDATE — Tasks 4/5/7 own the SQL).
    pub fn conn(&mut self) -> &mut PgConnection {
        &mut self.txn
    }

    /// The owning workspace (tenant scope, R-0076-b).
    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }

    /// Flush every staged audit row into the `coordination_audit` outbox
    /// table on THIS txn's own connection (R-0075-c outbox composition — the
    /// same txn as the state transition, so a flush failure and a state-write
    /// failure share one rollback). Drains `staged_audit`; returns the
    /// flushed count. Called by [`PgCoordinationStore::run_write`]
    /// immediately before the one COMMIT — an `Err` here means `run_write`
    /// does NOT commit (the txn is dropped instead, rolling back the state
    /// transition along with the failed audit).
    async fn flush_staged_audit(&mut self) -> Result<usize, sqlx::Error> {
        let staged = std::mem::take(&mut self.staged_audit);
        let count = staged.len();
        for record in &staged {
            debug_assert_eq!(
                record.workspace_id, self.workspace_id,
                "AuditRecord.workspace_id must match the machinery-authoritative \
                 CoordinationTxn.workspace_id (tenant scope) — a mismatch here \
                 means a body staged an audit row for the wrong workspace"
            );
            sqlx::query(
                "INSERT INTO coordination_audit \
                 (workspace_id, event_type, event_version, actor_id, payload) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(self.workspace_id)
            .bind(record.event_type.as_str())
            .bind(i32::from(record.event_version))
            .bind(record.actor_id)
            .bind(&record.payload)
            .execute(&mut *self.txn)
            .await?;
        }
        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// Tests (Task 3 sub-run c-green): the invariants the red fault-injection
// tests in `tests/coordination_failclosed.rs` do not cover.
// ---------------------------------------------------------------------------
//
// Scope note (this dispatch is forbid-scoped from `libs/mnemra-host/tests/`):
// everything below that needs NO live Postgres (reason-code mapping, the
// op-log shape incl. refusals, the target tag) is self-tested here as an
// in-module `#[cfg(test)]` unit test. ONE exception boots a real embedded
// engine directly (`crate::storage::postgres::engine::EmbeddedEngine`, always
// compiled — not a `tests/common/` fixture) for the happy-path commit + audit
// round-trip: it is the ONLY thing that proves the `INSERT INTO
// coordination_audit` in `flush_staged_audit` above actually runs and is
// well-formed — none of AC1 (skips the flush by design)/AC2 (short-circuits
// before `begin`, body never runs)/AC3 (times out on `begin`) execute it. A
// single, non-concurrent engine boot added to `cargo test -p mnemra-host
// --lib` does not interact with the `--test-threads 1` PG-group race
// (#1852) that motivates that directive — no other `--lib` test boots an
// engine, so there is nothing for it to race with.
//
// NOT self-tested here, flagged for Warden's #2290 review: the refusal path's
// rollback (`WriteResult::Refuse` → no state, no audit) against a REAL txn —
// unlike the happy path above, this is pure control-flow already visible by
// construction in `run_write` above (the `Ok(WriteResult::Refuse(refusal)) =>
// Ok(WriteResult::Refuse(refusal))` arm never calls `flush_staged_audit` or
// `commit`, so the txn only ever drops), so the residual risk is lower than
// the untested-INSERT risk the happy-path test closes.
#[cfg(test)]
mod tests {
    use super::*;

    fn ws() -> Uuid {
        Uuid::new_v4()
    }

    fn some_actor() -> Uuid {
        Uuid::new_v4()
    }

    // -----------------------------------------------------------------
    // Refusal::reason_code — exhaustive, pinned to the spec's closed
    // `reason_code` enum (§API Contract) verbatim.
    // -----------------------------------------------------------------

    #[test]
    fn refusal_reason_code_matches_spec_closed_enum_verbatim() {
        let now = Utc::now();
        let cases: &[(Refusal, &str)] = &[
            (
                Refusal::ResourceHeld {
                    holder_actor_id: some_actor(),
                    expires_at: now,
                },
                "resource_held",
            ),
            (Refusal::NotHolder, "not_holder"),
            (Refusal::NotExpired, "not_expired"),
            (Refusal::LeaseNotFound, "lease_not_found"),
            (Refusal::InvalidDuration, "invalid_duration"),
            (Refusal::InvalidResource, "invalid_resource"),
            (Refusal::ReservedFamily, "reserved_family"),
            (Refusal::NotAttached, "not_attached"),
            (Refusal::ActorLiveAttached, "actor_live_attached"),
            (Refusal::AttachmentMismatch, "attachment_mismatch"),
            (Refusal::WrongActorType, "wrong_actor_type"),
            (Refusal::InvalidRoleInstance, "invalid_role_instance"),
            (Refusal::NotAddressee, "not_addressee"),
            (Refusal::InvalidTransition, "invalid_transition"),
            (Refusal::InvalidDisposition, "invalid_disposition"),
            (Refusal::SchemaViolation, "schema_violation"),
            (Refusal::UnknownType, "unknown_type"),
            (Refusal::MessageNotFound, "message_not_found"),
        ];
        assert_eq!(
            cases.len(),
            18,
            "spec §API Contract's reason_code enum has exactly 18 codes — a case was \
             added/removed without updating this pin"
        );
        for (refusal, expected) in cases {
            assert_eq!(refusal.reason_code(), *expected);
        }
    }

    // -----------------------------------------------------------------
    // log_outcome — AC5 (R-0075-d target tag), R-0075-a (op-log incl.
    // refusals, machine-readable reason code), R-0075-e (structured fields,
    // never string-interpolated into the message). No live DB — `log_outcome`
    // takes an already-computed `Result`, decoupled from `run_write`'s
    // Postgres-coupled orchestration.
    // -----------------------------------------------------------------

    #[tracing_test::traced_test]
    #[test]
    fn log_outcome_commit_carries_target_and_outcome_field() {
        let result: Result<WriteResult<()>, Unavailable> = Ok(WriteResult::Commit(()));
        log_outcome(
            CoordinationOp::AttachBind,
            ws(),
            some_actor(),
            None,
            &result,
        );

        assert!(
            logs_contain("coordination write committed"),
            "commit must emit the static op-log message"
        );
        assert!(
            logs_contain(COORDINATION_TARGET),
            "AC5 / R-0075-d: the coordination op-log entry must carry the \
             {COORDINATION_TARGET} target tag on the unified stream"
        );
    }

    #[tracing_test::traced_test]
    #[test]
    fn log_outcome_refusal_carries_reason_code_and_outcome_refused() {
        // Spot-check a family spanning the grammar (lease / attachment /
        // message) rather than all 18 — reason_code's own mapping is already
        // pinned exhaustively above; this test is about the op-log SHAPE.
        for refusal in [
            Refusal::NotHolder,
            Refusal::AttachmentMismatch,
            Refusal::MessageNotFound,
        ] {
            let expected_code = refusal.reason_code();
            let result: Result<WriteResult<()>, Unavailable> = Ok(WriteResult::Refuse(refusal));
            log_outcome(CoordinationOp::Renew, ws(), some_actor(), None, &result);

            assert!(
                logs_contain("coordination write refused"),
                "refusal must emit the static op-log message (R-0075-a: refusals logged)"
            );
            assert!(
                logs_contain(expected_code),
                "refusal op-log entry must carry its machine-readable reason_code \
                 ({expected_code})"
            );
        }
    }

    #[tracing_test::traced_test]
    #[test]
    fn log_outcome_unavailable_variants_carry_outcome_and_reason_code() {
        let timeout_result: Result<WriteResult<()>, Unavailable> = Err(Unavailable::Timeout {
            op: CoordinationOp::Acquire,
            bound: Duration::from_secs(1),
        });
        log_outcome(
            CoordinationOp::Acquire,
            ws(),
            some_actor(),
            None,
            &timeout_result,
        );
        assert!(logs_contain(
            "coordination write unavailable: end-to-end bound elapsed"
        ));
        assert!(logs_contain("timeout"));

        let store_err: Box<dyn Error + Send + Sync> = "synthetic store failure".into();
        let store_result: Result<WriteResult<()>, Unavailable> = Err(Unavailable::Store(store_err));
        log_outcome(
            CoordinationOp::Release,
            ws(),
            some_actor(),
            None,
            &store_result,
        );
        assert!(logs_contain(
            "coordination write unavailable: storage failure"
        ));
        // The error's own Display text is the field value (`error = %e`), not
        // spliced into the message — confirms R-0075-e hygiene: dynamic
        // content rides a field, and the static message text above is
        // unchanged regardless of the error's content.
        assert!(logs_contain("synthetic store failure"));
    }

    // -----------------------------------------------------------------
    // log_outcome — actor_id attribution (R-0075-a, maintainer-adjudicated:
    // decision brief
    // `brain/projects/mnemra/2026-07-12-r0075a-oplog-actor-attribution-decision.md`).
    // `token_id` is the session/token context, always retained; `actor_id` is
    // the additive, body-recorded acting-actor field — `Some` when
    // `CoordinationTxn::record_acting_actor` was called, `None` otherwise
    // (honest attribution for pre-resolution refusals / no-actor ops). These
    // assert the actual field VALUE (not just line presence), per each of the
    // three required shapes: recorded on commit, absent-with-token-retained,
    // and recorded on a refusal.
    // -----------------------------------------------------------------

    #[tracing_test::traced_test]
    #[test]
    fn log_outcome_commit_carries_actor_id_when_body_recorded_it() {
        let actor = some_actor();
        let result: Result<WriteResult<()>, Unavailable> = Ok(WriteResult::Commit(()));
        log_outcome(
            CoordinationOp::AttachBind,
            ws(),
            some_actor(),
            Some(actor),
            &result,
        );

        assert!(
            logs_contain(&format!("actor_id=Some({actor})")),
            "R-0075-a: when the body recorded an acting actor, the op-log entry's \
             actor_id field must carry that exact id — got a log line missing \
             'actor_id=Some({actor})'"
        );
    }

    #[tracing_test::traced_test]
    #[test]
    fn log_outcome_actor_id_absent_when_not_recorded_but_token_id_retained() {
        let token = some_actor();
        let result: Result<WriteResult<()>, Unavailable> = Ok(WriteResult::Commit(()));
        log_outcome(CoordinationOp::AttachBind, ws(), token, None, &result);

        assert!(
            logs_contain("actor_id=None"),
            "when the body never called record_acting_actor, the op-log entry's \
             actor_id field must be None, not defaulted to some other value"
        );
        assert!(
            logs_contain(&format!("token_id={token}")),
            "token_id must remain on the entry as the retained session/token \
             context field even when no actor was recorded (option 1: additive, \
             token_id NOT removed)"
        );
    }

    #[tracing_test::traced_test]
    #[test]
    fn log_outcome_refusal_also_carries_actor_id_field() {
        // R-0075-a: refusals are logged too, and a refusal fired AFTER the
        // body resolved an actor (e.g. NotHolder — the actor exists, it's
        // just not the lease holder) must still attribute that actor.
        let actor = some_actor();
        let result: Result<WriteResult<()>, Unavailable> =
            Ok(WriteResult::Refuse(Refusal::NotHolder));
        log_outcome(
            CoordinationOp::Renew,
            ws(),
            some_actor(),
            Some(actor),
            &result,
        );

        assert!(
            logs_contain("coordination write refused"),
            "must still emit the refusal op-log message"
        );
        assert!(
            logs_contain(&format!("actor_id=Some({actor})")),
            "R-0075-a: a refusal op-log entry must also carry the actor_id field \
             when the body had resolved an actor before refusing"
        );
    }

    // -----------------------------------------------------------------
    // Happy path (mirrors AC2's shape, no fault injected) — the ONLY test
    // (self- or red-) that exercises `flush_staged_audit`'s real INSERT.
    // Boots its own embedded engine directly (not `tests/common/`) — see the
    // module-level scope note above for why this one live-DB test lives here.
    // (Deliberately still the ONLY `--lib` test that boots an engine — see
    // that same scope note re: the #1852 PG-group race — so the R-0075-a
    // `record_acting_actor` end-to-end wiring check below is folded into
    // THIS test rather than added as a second live-DB test.)
    // -----------------------------------------------------------------

    #[tracing_test::traced_test]
    #[tokio::test]
    async fn clean_write_commits_state_and_flushes_exactly_one_audit_row() {
        use crate::auth::role::Role;
        use crate::storage::postgres::engine::EmbeddedEngine;

        let engine = EmbeddedEngine::start()
            .await
            .expect("embedded engine must start");
        let db = engine
            .provision_test_database()
            .await
            .expect("provision_test_database should succeed");

        let workspace_id = Uuid::new_v4();
        let actor_name = format!("coord-green-happy-{}", Uuid::new_v4());
        let body_name = actor_name.clone();
        let ctx = WorkspaceCtx::new(workspace_id, Role::Admin, Uuid::new_v4());

        let store = PgCoordinationStore::new(Arc::new(db.pool.clone()), Duration::from_secs(10));

        let result = store
            .run_write(
                &ctx,
                CoordinationOp::AttachBind,
                move |tx: &mut CoordinationTxn| {
                    Box::pin(async move {
                        let ws = tx.workspace_id();
                        let row: (Uuid,) = sqlx::query_as(
                            "INSERT INTO actors (workspace_id, actor_type, name) \
                             VALUES ($1, 'agent', $2) RETURNING id",
                        )
                        .bind(ws)
                        .bind(&body_name)
                        .fetch_one(tx.conn())
                        .await
                        .map_err(|e| StorageFailure(Box::new(e)))?;
                        tx.stage_audit(AuditRecord::registration(ws, row.0, &body_name));
                        // R-0075-a: record the freshly-minted actor as the
                        // acting actor for this write — this is the real
                        // `run_write` wiring under test below (not just
                        // `log_outcome`'s own field-emission behavior, which
                        // the decoupled unit tests above already cover).
                        tx.record_acting_actor(row.0);
                        Ok(WriteResult::Commit(row.0))
                    })
                },
            )
            .await;

        assert!(
            matches!(result, Ok(WriteResult::Commit(_))),
            "a clean write with no fault injected must commit: {result:?}"
        );
        let minted_actor_id = match &result {
            Ok(WriteResult::Commit(id)) => *id,
            _ => unreachable!("checked above"),
        };

        assert!(
            logs_contain(&format!("actor_id=Some({minted_actor_id})")),
            "R-0075-a end-to-end: `run_write` must read back the actor the body \
             recorded via `record_acting_actor` and surface it on the op-log \
             entry as actor_id — not just when `log_outcome` is called directly, \
             but through the machinery's own tx-read-after-body wiring"
        );

        let present: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM actors WHERE workspace_id = $1 AND name = $2")
                .bind(workspace_id)
                .bind(&actor_name)
                .fetch_optional(&db.pool)
                .await
                .expect("actor read-back query must execute");
        assert!(
            present.is_some(),
            "the committed actor must be present on read-back"
        );

        let audit_rows: (i64,) =
            sqlx::query_as("SELECT count(*) FROM coordination_audit WHERE workspace_id = $1")
                .bind(workspace_id)
                .fetch_one(&db.pool)
                .await
                .expect("coordination_audit count query must execute");
        assert_eq!(
            audit_rows.0, 1,
            "a clean privileged write must flush exactly the one staged audit row \
             (R-0075-c: audited-or-failed — this is the audited-and-succeeded half)"
        );
    }
}
