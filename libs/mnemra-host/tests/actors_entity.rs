//! Tenancy and write-validation tests for the `actors` core entity (P-0018
//! D-ENT / D-ACTOR, Task 1 of the coordination-wedge build).
//!
//! # Scope of this landing
//!
//! `actors` lands here as a minimal STANDALONE entity, minted directly by
//! role-instance name. There is NO FK linkage from `actors` to the existing
//! `users`/`agents`/`sessions` builtins in this landing — the fuller P-0018
//! unification (rewiring those builtins to populate `actors`) is deferred to
//! P-0018's own landing (Gap A, decided 2026-07-11). These tests therefore
//! exercise `builtins::actors` in isolation, with no cross-builtin setup.
//!
//! # Adversarial style (mirrors `tenancy_isolation.rs`)
//!
//! These are BEHAVIOR tests, not structural ones. The tenancy tests assert
//! the actual cross-workspace leak path — create under workspace A, confirm
//! ABSENT under workspace B, and that every returned row's `workspace_id`
//! equals the querying workspace — never a vacuous `is_ok()`. The write tests
//! assert rejection at the DATABASE (the `actors_actor_type_chk` CHECK
//! constraint), not merely an app-level guard: the rejected write goes
//! through raw SQL, bypassing the typed `ActorType` API entirely, so a
//! CHECK-constraint regression (or its accidental removal) fails this test
//! even though the typed API alone could never construct an invalid value.
//!
//! # Engine acquisition
//!
//! Same shared-engine fixture as `tenancy_isolation.rs`: each test acquires
//! the binary-wide shared engine via `shared_engine::shared_engine()` and
//! provisions its own fresh, isolated database via
//! `EmbeddedEngine::provision_test_database()`, which runs the same
//! schema-init sequence (and therefore the `actors` migration) that
//! `schema::init::init()` runs. Migration idempotency itself is covered
//! generically by `schema_init.rs::init_idempotent` (which re-runs the full
//! `V0_MIGRATIONS` list, including the `actors` migration added here) — not
//! duplicated in this file.
//!
//! # Constructor note (codebase convention)
//!
//! `WorkspaceCtx::for_test` is `#[cfg(test)]`-gated inside the library crate
//! and is NOT visible from this integration-test binary (a separate crate).
//! The public production constructor `WorkspaceCtx::new` is the correct one
//! here — mirrors `tests/tenancy_isolation.rs` / `tests/permissions.rs`.

#[path = "common/shared_engine.rs"]
mod shared_engine;

use mnemra_host::auth::role::Role;
use mnemra_host::auth::workspace_ctx::WorkspaceCtx;
use mnemra_host::builtins;
use mnemra_host::builtins::actors::ActorType;
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use sqlx::PgPool;
use uuid::Uuid;

/// Construct an Admin `WorkspaceCtx` scoped to `workspace_id` for tests.
///
/// `Role::Admin` so the ctx carries full access — these tests probe the
/// *workspace* scoping boundary and the write-validation boundary, not the
/// role/permission boundary.
fn ctx_for(workspace_id: Uuid) -> WorkspaceCtx {
    WorkspaceCtx::new(workspace_id, Role::Admin, Uuid::new_v4())
}

// ===========================================================================
// Write validation — actor_type is a closed set, enforced at the DATABASE
// ===========================================================================

/// P-0018 D-ACTOR (positive write): a write with `actor_type = "agent"`
/// succeeds.
///
/// Goes through raw SQL (not the typed API) so this asserts the SCHEMA
/// accepts the in-set value, independent of any app-level guard.
#[tokio::test]
async fn actors_raw_write_with_valid_actor_type_succeeds() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let workspace_id = Uuid::new_v4();

    let result = sqlx::query(
        "INSERT INTO actors (workspace_id, actor_type, name)
         VALUES ($1, 'agent', 'valid-write-probe')",
    )
    .bind(workspace_id)
    .execute(pool)
    .await;

    assert!(
        result.is_ok(),
        "a write with actor_type = 'agent' must succeed (P-0018 D-ACTOR): {:?}",
        result.err()
    );
}

/// P-0018 D-ACTOR (negative write — the headline schema-level test): an
/// out-of-set `actor_type` is rejected AT WRITE by the database, not merely
/// by an application-level guard.
///
/// This INSERT bypasses `builtins::actors` entirely (raw SQL, out-of-set
/// literal `'robot'`) — a CHECK-constraint regression that only enforced the
/// enum in Rust code would let this succeed. It must not.
#[tokio::test]
async fn actors_raw_write_with_invalid_actor_type_is_rejected_by_check_constraint() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let workspace_id = Uuid::new_v4();

    let result = sqlx::query(
        "INSERT INTO actors (workspace_id, actor_type, name)
         VALUES ($1, 'robot', 'invalid-write-probe')",
    )
    .bind(workspace_id)
    .execute(pool)
    .await;

    assert!(
        result.is_err(),
        "a write with an out-of-set actor_type ('robot') MUST be rejected at \
         write by the actors_actor_type_chk CHECK constraint (P-0018 D-ACTOR) \
         — it was NOT rejected, meaning either the constraint is missing or \
         was weakened"
    );

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.to_lowercase().contains("actors_actor_type_chk")
            || err_msg.to_lowercase().contains("check constraint"),
        "the rejection must come from the actors_actor_type_chk CHECK \
         constraint (a schema-level guard), not some other error — got: {err_msg}"
    );
}

// ===========================================================================
// actors::list_by_workspace — cross-workspace read isolation (both directions)
// ===========================================================================

/// P-0018 D-ENT (actors read isolation): an actor resolved under workspace A
/// is visible in A's list and DOES NOT leak into workspace B's list.
#[tokio::test]
async fn actors_list_by_workspace_excludes_other_workspace_rows() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let ctx_a = ctx_for(ws_a);
    let ctx_b = ctx_for(ws_b);

    let actor_a = builtins::actors::resolve_or_create(pool, &ctx_a, ActorType::Agent, "actor-in-a")
        .await
        .expect("resolve_or_create under workspace A must succeed");
    let actor_b = builtins::actors::resolve_or_create(pool, &ctx_b, ActorType::Human, "actor-in-b")
        .await
        .expect("resolve_or_create under workspace B must succeed");

    let listed_a = builtins::actors::list_by_workspace(pool, &ctx_a)
        .await
        .expect("list_by_workspace under A must succeed");

    assert!(
        listed_a.iter().any(|a| a.id == actor_a.id),
        "workspace A's own actor must appear in A's list (P-0018 D-ENT)"
    );
    assert!(
        !listed_a.iter().any(|a| a.id == actor_b.id),
        "workspace B's actor MUST NOT leak into workspace A's list — \
         tenancy isolation breach (P-0018 D-ENT)"
    );
    assert!(
        listed_a.iter().all(|a| a.workspace_id == ws_a),
        "every row returned under A's ctx must belong to workspace A \
         (P-0018 D-ENT); a foreign workspace_id is a cross-tenant leak"
    );
}

/// P-0018 D-ENT (actors read isolation, symmetric direction): the mirror of
/// the above — listing under workspace B sees only B's actor, never A's.
/// Pins isolation in both directions so a one-sided filter bug (e.g. a
/// hard-coded constant) cannot pass.
#[tokio::test]
async fn actors_list_by_workspace_is_symmetric_isolation() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let ctx_a = ctx_for(ws_a);
    let ctx_b = ctx_for(ws_b);

    let actor_a = builtins::actors::resolve_or_create(pool, &ctx_a, ActorType::System, "sym-a")
        .await
        .expect("resolve_or_create under A must succeed");
    let actor_b = builtins::actors::resolve_or_create(pool, &ctx_b, ActorType::System, "sym-b")
        .await
        .expect("resolve_or_create under B must succeed");

    let listed_b = builtins::actors::list_by_workspace(pool, &ctx_b)
        .await
        .expect("list_by_workspace under B must succeed");

    assert!(
        listed_b.iter().any(|a| a.id == actor_b.id),
        "workspace B's own actor must appear in B's list (P-0018 D-ENT)"
    );
    assert!(
        !listed_b.iter().any(|a| a.id == actor_a.id),
        "workspace A's actor MUST NOT leak into workspace B's list (P-0018 D-ENT)"
    );
    assert!(
        listed_b.iter().all(|a| a.workspace_id == ws_b),
        "every row returned under B's ctx must belong to workspace B \
         (P-0018 D-ENT); a foreign workspace_id is a cross-tenant leak"
    );
}

// ===========================================================================
// actors::resolve_or_create — resolve-or-create-by-role-instance-name
// ===========================================================================

/// P-0018 D-ENT (resolve-or-create idempotency): the same identifier in the
/// same workspace resolves to the same `actor_id` across calls, and mints
/// EXACTLY ONE row — not vacuous `is_ok()` on each call, but a direct row
/// count against the database.
#[tokio::test]
async fn actors_resolve_or_create_same_name_same_workspace_is_idempotent() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let ws = Uuid::new_v4();
    let ctx = ctx_for(ws);

    let first =
        builtins::actors::resolve_or_create(pool, &ctx, ActorType::Agent, "idempotent-actor")
            .await
            .expect("first resolve_or_create must succeed");
    let second =
        builtins::actors::resolve_or_create(pool, &ctx, ActorType::Agent, "idempotent-actor")
            .await
            .expect("second resolve_or_create (same triple) must succeed");

    assert_eq!(
        first.id, second.id,
        "resolving the same (workspace, name) twice must return the same \
         actor_id (P-0018 D-ENT resolve-or-create)"
    );

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM actors WHERE workspace_id = $1 AND name = $2")
            .bind(ws)
            .bind("idempotent-actor")
            .fetch_one(pool)
            .await
            .expect("count query must succeed");

    assert_eq!(
        count.0, 1,
        "resolve_or_create called twice with the identical (workspace, name) \
         triple must mint EXACTLY ONE row, not two (P-0018 D-ENT)"
    );
}

/// P-0018 D-ENT (resolve-or-create distinctness): distinct names in the same
/// workspace resolve to distinct `actor_id`s.
#[tokio::test]
async fn actors_resolve_or_create_distinct_names_yield_distinct_ids() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let ws = Uuid::new_v4();
    let ctx = ctx_for(ws);

    let a = builtins::actors::resolve_or_create(pool, &ctx, ActorType::Human, "distinct-1")
        .await
        .expect("resolve_or_create for distinct-1 must succeed");
    let b = builtins::actors::resolve_or_create(pool, &ctx, ActorType::Human, "distinct-2")
        .await
        .expect("resolve_or_create for distinct-2 must succeed");

    assert_ne!(
        a.id, b.id,
        "distinct role-instance names in the same workspace must mint \
         distinct actor_ids (P-0018 D-ENT)"
    );
}

/// P-0018 D-ENT (resolve-or-create cross-tenant distinctness): the SAME
/// role-instance name in two DIFFERENT workspaces resolves to distinct
/// `actor_id`s — the per-workspace uniqueness is scoped by workspace, not
/// global.
#[tokio::test]
async fn actors_resolve_or_create_same_name_different_workspace_yields_distinct_ids() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let ctx_a = ctx_for(ws_a);
    let ctx_b = ctx_for(ws_b);

    let a = builtins::actors::resolve_or_create(pool, &ctx_a, ActorType::Agent, "shared-name")
        .await
        .expect("resolve_or_create under A must succeed");
    let b = builtins::actors::resolve_or_create(pool, &ctx_b, ActorType::Agent, "shared-name")
        .await
        .expect("resolve_or_create under B must succeed");

    assert_ne!(
        a.id, b.id,
        "the same role-instance name in two different workspaces must mint \
         distinct actor_ids — per-workspace uniqueness, not global (P-0018 D-ENT)"
    );
    assert_eq!(a.workspace_id, ws_a);
    assert_eq!(b.workspace_id, ws_b);
}

/// P-0018 D-ENT (resolve-or-create type stability): re-resolving an existing
/// (workspace, name) with a DIFFERENT actor_type returns the PERSISTED type,
/// never the type passed on the later call. Guards the resolve_or_create doc
/// contract — a regression to `DO UPDATE SET actor_type = EXCLUDED.actor_type`
/// would silently break an identity-relevant invariant with a green suite.
#[tokio::test]
async fn actors_resolve_or_create_does_not_overwrite_existing_actor_type() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let ws = Uuid::new_v4();
    let ctx = ctx_for(ws);

    let first = builtins::actors::resolve_or_create(pool, &ctx, ActorType::Agent, "type-stable")
        .await
        .expect("first resolve_or_create must succeed");
    assert_eq!(
        first.actor_type,
        ActorType::Agent,
        "the minting call sets actor_type = Agent"
    );

    let second = builtins::actors::resolve_or_create(pool, &ctx, ActorType::Human, "type-stable")
        .await
        .expect("second resolve_or_create (different type, same triple) must succeed");

    assert_eq!(
        second.actor_type,
        ActorType::Agent,
        "re-resolving an existing (workspace, name) MUST return the PERSISTED \
         actor_type (Agent), never the type passed on the later call (Human) — \
         resolution must not silently overwrite an existing actor's type"
    );

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM actors WHERE workspace_id = $1 AND name = $2")
            .bind(ws)
            .bind("type-stable")
            .fetch_one(pool)
            .await
            .expect("count query must succeed");
    assert_eq!(
        count.0, 1,
        "re-resolving the same triple must not mint a second row"
    );
}

// ===========================================================================
// actors::resolve_or_create_in_txn — the txn-scoped resolve/mint (Q3 fix)
//
// The coordination write path (`run_write` bodies, Tasks 4/5/7) mint the
// actor row AND stage its registration audit atomically inside one COMMIT.
// That requires resolve-or-create to run on a BORROWED connection inside the
// caller's transaction, not to self-commit like the `&PgPool` entry point.
// These prove the txn-scoped variant resolves/mints correctly on a borrowed
// `&mut PgConnection` and leaves the commit to the caller.
// ===========================================================================

/// The txn-scoped variant resolves-or-mints on a borrowed connection: two calls
/// with the same triple in one transaction return the SAME `actor_id` and, on
/// commit, exactly one row exists. Mirrors the `resolve_or_create` idempotency
/// test, but feeds a borrowed `&mut PgConnection` (the coordination-body shape)
/// and takes `workspace_id` directly (fed `CoordinationTxn::workspace_id()` in
/// production).
#[tokio::test]
async fn actors_resolve_or_create_in_txn_is_idempotent_on_borrowed_conn() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let ws = Uuid::new_v4();

    let mut tx = pool.begin().await.expect("begin txn must succeed");

    let first =
        builtins::actors::resolve_or_create_in_txn(&mut tx, ws, ActorType::Agent, "txn-idempotent")
            .await
            .expect("first in-txn resolve_or_create must succeed");
    let second =
        builtins::actors::resolve_or_create_in_txn(&mut tx, ws, ActorType::Agent, "txn-idempotent")
            .await
            .expect("second in-txn resolve_or_create (same triple) must succeed");

    assert_eq!(
        first.id, second.id,
        "resolving the same (workspace, name) twice on the borrowed connection \
         must return the same actor_id — no duplicate row"
    );
    assert_eq!(
        first.workspace_id, ws,
        "the resolved row must carry the workspace_id fed directly to the txn \
         variant (the tenant enforcement point — fed CoordinationTxn::workspace_id())"
    );

    tx.commit().await.expect("commit must succeed");

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM actors WHERE workspace_id = $1 AND name = $2")
            .bind(ws)
            .bind("txn-idempotent")
            .fetch_one(pool)
            .await
            .expect("count query must succeed");
    assert_eq!(
        count.0, 1,
        "two in-txn resolves of the identical triple must mint EXACTLY ONE row"
    );
}

/// The txn-scoped variant does NOT self-commit — the caller controls the
/// commit. Mint a row inside a transaction, ROLL BACK, and confirm the row is
/// absent on the pool: if the function committed on its own, the row would
/// survive the rollback. This is the clean-room proof of the Q3 property the
/// coordination write path depends on (the actor mint and its audit share the
/// body's single COMMIT — or share its rollback).
#[tokio::test]
async fn actors_resolve_or_create_in_txn_caller_controls_commit() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let ws = Uuid::new_v4();

    let mut tx = pool.begin().await.expect("begin txn must succeed");
    let minted =
        builtins::actors::resolve_or_create_in_txn(&mut tx, ws, ActorType::Agent, "txn-rollback")
            .await
            .expect("in-txn resolve_or_create must succeed");
    assert_eq!(minted.workspace_id, ws);

    // Roll the transaction back — the function must not have committed.
    tx.rollback().await.expect("rollback must succeed");

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM actors WHERE workspace_id = $1 AND name = $2")
            .bind(ws)
            .bind("txn-rollback")
            .fetch_one(pool)
            .await
            .expect("count query must succeed");
    assert_eq!(
        count.0, 0,
        "resolve_or_create_in_txn MUST NOT self-commit: after the caller rolls \
         back, the minted row must be absent (the caller controls the commit)"
    );
}
