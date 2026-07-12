//! Fault-injection acceptance tests for the privileged coordination write
//! foundation (Task 3, sub-run c — RED). Glitch, dispatch 1506.
//!
//! # What these tests contract (the spec guarantees, not the skeleton)
//!
//! The b1 skeleton
//! ([`mnemra_host::coordination::write_path::PgCoordinationStore::run_write`])
//! is a deliberately non-guaranteeing pass-through: no timeout wrap, staged
//! audit never flushed, the injected fault never consulted, no
//! `Unavailable::Timeout` path. These tests encode the three write guarantees
//! the GREEN implementer (sub-run b) must fill, and so they FAIL against the
//! skeleton by design:
//!
//! - **AC1 — emit-guarantee (R-0075-c).** With audit capture forced to fail
//!   (`CoordinationFault::AuditEmitFail`), the state transition must NOT commit
//!   (a fresh read-back after recovery shows the row absent) and the caller
//!   receives a structured `Unavailable`. Red: the skeleton ignores the fault
//!   and commits the state anyway → the row IS present.
//! - **AC2 — fail-closed availability (R-0074-a/b/c; QA-6).** With the store
//!   forced unavailable via `CoordinationFault::StoreUnavailable` (injected on a
//!   *working* embedded pool — NOT a bare unreachable-pool `matches!`, which the
//!   skeleton would satisfy legitimately), a write returns a structured
//!   `Unavailable`, is provably absent on recovery, and nothing is queued or
//!   retried. Red: the skeleton ignores the fault → the write commits →
//!   `Ok(Commit)`, not `Err(Unavailable)`.
//! - **AC3 — timeout dominates (R-0074-b).** With a short coordination write
//!   bound and a storage pool whose OWN acquire timeout is long, a write blocking
//!   on the unreachable pool returns `Unavailable::Timeout` within the short
//!   coordination bound — proving the coordination path's end-to-end bound fires
//!   ahead of the 60 s pool acquire. Red: the skeleton has no timeout wrap → it
//!   hangs on the pool's long acquire, never returning within the short bound.
//!
//! # Test approach — spec-mandated fault-injection seam (NOT a black-box gap)
//!
//! At Task 3 the coordination machinery is host-internal (no CLI/HTTP/MCP surface
//! exists yet — those land in Tasks 4/5/7). R-0075-c and R-0074-b MANDATE
//! verification "through the host's feature-gated fault-injection seam (the
//! existing `InjectedFailure` test-support pattern, `no_test_seams`-guarded)."
//! So these are Rust integration tests driving the spec-mandated `test-hooks`
//! `CoordinationFault` seam — the correct surface, not an implementation-detail
//! reach. Assertions are written from the spec ACs, never from the skeleton's
//! (absent) behavior.
//!
//! # Feature gating & wiring
//!
//! The whole file is `#![cfg(feature = "test-hooks")]` — it names
//! `CoordinationFault`, which is `test-hooks`-gated. Under the default-feature
//! `verify-test` run the binary is structurally zero-test (exits 0, no false
//! green — the same posture as `artifact_list_paging_whitebox`); it is
//! meaningfully active only under `verify-test-hooks`, which the justfile guards
//! with a scoped non-vacuity check. Registered in `PG_TEST_FLAGS` (AC1/AC2 use
//! the shared embedded engine) so it runs at `--test-threads 1`.
#![cfg(feature = "test-hooks")]

#[path = "common/shared_engine.rs"]
mod shared_engine;

use std::sync::Arc;
use std::time::Duration;

use mnemra_host::auth::role::Role;
use mnemra_host::auth::workspace_ctx::WorkspaceCtx;
use mnemra_host::coordination::CoordinationOp;
use mnemra_host::coordination::audit::AuditRecord;
use mnemra_host::coordination::write_path::{
    CoordinationFault, CoordinationTxn, PgCoordinationStore, StorageFailure, Unavailable,
    WriteResult,
};
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use sqlx::PgPool;
use uuid::Uuid;

/// Read back whether an actor named `name` exists in `workspace_id` — the
/// state-transition observation both AC1 and AC2 make on a *fresh* query after
/// `run_write` has fully resolved (committed or rolled back). `true` = the
/// transition committed; `false` = it is provably absent.
async fn actor_present(pool: &PgPool, workspace_id: Uuid, name: &str) -> bool {
    let found: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM actors WHERE workspace_id = $1 AND name = $2")
            .bind(workspace_id)
            .bind(name)
            .fetch_optional(pool)
            .await
            .expect("actor read-back query must execute");
    found.is_some()
}

/// A privileged Admin ctx scoped to `workspace_id` (coordination writes are
/// privileged; the role is not what these tests probe).
fn ctx_for(workspace_id: Uuid) -> WorkspaceCtx {
    WorkspaceCtx::new(workspace_id, Role::Admin, Uuid::new_v4())
}

// ===========================================================================
// AC1 — emit-guarantee (R-0075-c): audit-capture failure rolls back the state
// ===========================================================================

/// GIVEN a privileged coordination write whose body mints an actor (the state
/// transition) and stages a registration audit row, AND
/// `CoordinationFault::AuditEmitFail` injected,
/// WHEN `run_write` runs,
/// THEN the minted actor is ABSENT on a fresh read-back (the whole txn rolls
/// back when audit capture fails) AND the caller receives a structured
/// `Unavailable` failure.
///
/// RED against the skeleton: `run_write` never consults `injected_fault` and
/// never flushes staged audit — it commits the state transition regardless. So
/// the actor row IS present on read-back and the caller gets `Ok(Commit)`. Both
/// assertions fail because the emit-guarantee is absent.
#[tokio::test]
async fn audit_emit_failure_rolls_back_the_state_transition() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let workspace_id = Uuid::new_v4();
    let actor_name = format!("coord-red-ac1-{}", Uuid::new_v4());
    let body_name = actor_name.clone();
    let ctx = ctx_for(workspace_id);

    let store = PgCoordinationStore::new(Arc::new(db.pool.clone()), Duration::from_secs(10))
        .with_injected_fault(CoordinationFault::AuditEmitFail);

    let result: Result<WriteResult<Uuid>, Unavailable> = store
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
                    Ok(WriteResult::Commit(row.0))
                })
            },
        )
        .await;

    // Primary red signal (the guarantee): the state transition must have rolled
    // back, so the actor is absent.
    let present = actor_present(&db.pool, workspace_id, &actor_name).await;
    assert!(
        !present,
        "AC1 (R-0075-c): with AuditEmitFail injected, the minted actor MUST be absent on \
         read-back — the whole txn rolls back when audit capture fails. The actor was \
         present, so the state committed WITHOUT its audit (emit-guarantee absent). \
         run_write returned: {result:?}"
    );

    // Secondary: the caller sees a structured failure, never a silent success.
    assert!(
        result.is_err(),
        "AC1 (R-0075-c): the caller must receive a structured Unavailable when audit \
         capture fails; got a success: {result:?}"
    );
}

// ===========================================================================
// AC2 — fail-closed availability (R-0074-a/b/c; QA-6): an unavailable store is
// a structured stop, provably absent, with nothing queued or retried
// ===========================================================================

/// GIVEN the coordination store forced unavailable via
/// `CoordinationFault::StoreUnavailable` — injected on a *working* embedded pool
/// (so the guarantee, not a real pool failure, is what the test turns on),
/// WHEN a coordination write (mint actor + stage audit) runs,
/// THEN it returns a structured `Unavailable` error, the write is provably
/// ABSENT from the store on recovery, and nothing is queued or retried (no
/// state row, no audit row).
///
/// RED against the skeleton: `run_write` never consults `injected_fault`, so the
/// working pool lets the body commit — the caller gets `Ok(Commit)` and the
/// actor IS present. The fail-closed guarantee is absent.
///
/// STRENGTH: the fault is injected on a WORKING pool. A bare
/// `matches!(res, Err(Unavailable::Store(_)))` against an *unreachable* pool
/// would pass GREEN against this behavior-absent skeleton (the skeleton maps a
/// real `pool.begin()` failure to `Unavailable::Store` legitimately) — a
/// vacuous, invalid red. Turning the test on the injected fault against a
/// working pool makes it red because the *guarantee* is absent.
#[tokio::test]
async fn injected_store_unavailability_fails_closed_with_nothing_committed() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let workspace_id = Uuid::new_v4();
    let actor_name = format!("coord-red-ac2-{}", Uuid::new_v4());
    let body_name = actor_name.clone();
    let ctx = ctx_for(workspace_id);

    // A WORKING pool (real embedded engine) — the fault, not the pool, drives
    // the outcome.
    let store = PgCoordinationStore::new(Arc::new(db.pool.clone()), Duration::from_secs(10))
        .with_injected_fault(CoordinationFault::StoreUnavailable);

    let result: Result<WriteResult<Uuid>, Unavailable> = store
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
                    Ok(WriteResult::Commit(row.0))
                })
            },
        )
        .await;

    // The write must NOT report success — it is a structured Unavailable stop.
    assert!(
        result.is_err(),
        "AC2 (R-0074-a/b): with StoreUnavailable injected, the write must surface a \
         structured Unavailable stop — never empty-success. Got: {result:?}"
    );

    // Provably absent on recovery — the state transition did not commit.
    let present = actor_present(&db.pool, workspace_id, &actor_name).await;
    assert!(
        !present,
        "AC2 (R-0074-c): the write must be provably absent from the store on recovery; \
         the minted actor was found present. run_write returned: {result:?}"
    );

    // Nothing queued or retried — no audit row was durably landed either.
    let audit_rows: (i64,) =
        sqlx::query_as("SELECT count(*) FROM coordination_audit WHERE workspace_id = $1")
            .bind(workspace_id)
            .fetch_one(&db.pool)
            .await
            .expect("coordination_audit count query must execute");
    assert_eq!(
        audit_rows.0, 0,
        "AC2 (R-0074-a): an unavailable-store write queues/retries nothing — no \
         coordination_audit row must remain for the workspace; found {}",
        audit_rows.0
    );
}

// ===========================================================================
// AC3 — timeout dominates (R-0074-b): the end-to-end coordination bound fires
// ahead of the storage pool's own (long) acquire timeout
// ===========================================================================

/// GIVEN a coordination store with a SHORT write bound (1 s) over a storage pool
/// whose OWN acquire timeout is LONG (60 s) against an unreachable address,
/// WHEN a write blocks on the unreachable pool,
/// THEN `run_write` returns `Unavailable::Timeout` within the short coordination
/// bound — proving the coordination path's end-to-end bound fires ahead of the
/// 60 s pool acquire, not the pool's.
///
/// RED against the skeleton: `run_write` has no timeout wrap — it awaits
/// `pool.begin()` directly, which blocks on the pool's 60 s acquire. So it does
/// not return within the 5 s test bound (the outer `tokio::time::timeout`
/// elapses) and can never produce `Unavailable::Timeout`. The dominance
/// guarantee is absent.
///
/// The pool's `acquire_timeout` is deliberately 60 s (NOT the 500 ms of
/// `health_listener.rs`'s `unreachable_lazy_pool`): a short pool timeout would
/// let `begin()` fast-fail as `Unavailable::Store`, which cannot prove the
/// coordination bound dominates. The 60 s acquire is what makes dominance
/// observable.
#[tokio::test]
async fn coordination_bound_dominates_the_pool_acquire_timeout() {
    // Unreachable lazy pool: parses the URL but never connects; the first query
    // (here, `begin()`) retries connection-refused with backoff until the pool's
    // own `acquire_timeout` (60 s) — far above the 1 s coordination bound. The
    // `max_lifetime(None).idle_timeout(None)` overrides mirror the vetted
    // `health_listener.rs::unreachable_lazy_pool` helper.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_lifetime(None)
        .idle_timeout(None)
        .acquire_timeout(Duration::from_secs(60))
        .connect_lazy("postgres://coord_red_ac3:coord_red_ac3@127.0.0.1:1/coord_red_ac3")
        .expect("connect_lazy must not eagerly connect (defers to first query)");

    let workspace_id = Uuid::new_v4();
    let body_name = format!("coord-red-ac3-{}", Uuid::new_v4());
    let ctx = ctx_for(workspace_id);

    // SHORT coordination write bound; NO fault injected — the timeout must fire
    // on its own.
    let store = PgCoordinationStore::new(Arc::new(pool), Duration::from_secs(1));

    // Bound the whole test at 5 s: comfortably above the 1 s coordination bound
    // (green returns well within this) and far below the 60 s pool acquire (the
    // skeleton, with no wrap, cannot return within this).
    let outer = tokio::time::timeout(
        Duration::from_secs(5),
        store.run_write(
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
                    Ok(WriteResult::Commit(row.0))
                })
            },
        ),
    )
    .await;

    // The coordination bound must dominate: run_write returns before the 5 s
    // test bound. (Skeleton: no wrap → blocks on the 60 s acquire → this
    // elapses → RED here.)
    let inner: Result<WriteResult<Uuid>, Unavailable> = outer.expect(
        "AC3 (R-0074-b): run_write MUST return within the 5 s test bound — the 1 s \
         coordination write bound is meant to dominate the 60 s pool acquire. It did not \
         return, so the coordination path has no end-to-end timeout wrap and hung on the \
         pool's long acquire.",
    );

    assert!(
        matches!(inner, Err(Unavailable::Timeout { .. })),
        "AC3 (R-0074-b): a write blocking on the unreachable pool must surface as \
         Unavailable::Timeout within the short coordination bound (proving the \
         coordination bound, not the 60 s pool acquire, fired); got {inner:?}"
    );
}
