//! Tenancy-isolation behavior tests for the identity query builtins (T13.3 / R-0006-e).
//!
//! # The security property under test
//!
//! AC T13.3 / R-0006-e: "Builtin components use the same `WorkspaceCtx`
//! threading; no internal DB-query bypass without a `WorkspaceCtx`."
//!
//! These are BEHAVIOR tests, not structural ones. They assert the *observable*
//! tenant-isolation outcome: a builtin invoked with workspace A's `WorkspaceCtx`
//! must return / affect ONLY workspace A's rows, never workspace B's. A vacuous
//! `is_ok()` assertion would pass against a stub that ignores the ctx — these
//! assert the actual cross-workspace boundary (B's row is ABSENT from A's view;
//! A's mutation does not touch B).
//!
//! # RED phase (Glitch-first TDD pair)
//!
//! These tests are written against the LOCKED TARGET signatures, where the
//! query/mutation builtins are threaded by `&WorkspaceCtx` rather than a raw
//! `workspace_id: Uuid` (or, for the IDOR surfaces, no workspace predicate at
//! all):
//!
//! ```text
//! agents::register(pool, &WorkspaceCtx, user_id, agent_name, supplied)
//! agents::list_by_workspace(pool, &WorkspaceCtx)
//! sessions::open(pool, &WorkspaceCtx, user_id, agent_id)
//! sessions::list_active_by_workspace(pool, &WorkspaceCtx)
//! // R-0006-e expansion (this cycle) — close the blanket rule:
//! projects::create(pool, &WorkspaceCtx, name)
//! projects::list_by_workspace(pool, &WorkspaceCtx)
//! projects::exists(pool, &WorkspaceCtx, id)
//! projects::delete(pool, &WorkspaceCtx, id)        // was delete(pool, id) — IDOR
//! sessions::close(pool, &WorkspaceCtx, session_id) // was close(pool, session_id) — IDOR
//! ```
//!
//! Today `projects::*` take a raw `workspace_id: Uuid` (and `delete` takes none),
//! and `sessions::close` takes only the `session_id`. So this file is expected to
//! FAIL TO COMPILE until the green phase (Forge) changes those signatures.
//! That compile-failure IS the RED: the tenancy contract cannot be spelled
//! against the current bypass-prone API. Do NOT make these pass by reverting to
//! the raw-`Uuid` / id-only calls — that would invert the gate.
//!
//! # IDOR surfaces (`projects::delete`, `sessions::close`)
//!
//! These two are write-by-id with no workspace predicate today — a caller that
//! knows/guesses a row id mutates it regardless of tenant. The signature swap
//! alone is insufficient: the green phase must add `AND workspace_id = $N` bound
//! to `ctx.workspace_id()` so the predicate reaches the SQL. The behavior tests
//! below prove the predicate is live by asserting the VICTIM ROW SURVIVES a
//! cross-tenant attempt (the load-bearing property), and that the cross-tenant
//! call does not silently report success. A signature-only change that drops the
//! predicate would compile but FAIL the survival assertion — which is the point.
//!
//! # Constructor note (codebase convention)
//!
//! `WorkspaceCtx::for_test` is `#[cfg(test)]`-gated inside the library crate and
//! is NOT visible from this integration-test binary (a separate crate). The
//! public production constructor `WorkspaceCtx::new` is the correct one here —
//! mirrors `tests/permissions.rs`.
//!
//! # No FK to workspaces
//!
//! Neither `agents`, `sessions`, nor `projects` has a foreign key to
//! `workspaces` (`schema/init.rs` declares `workspace_id UUID NOT NULL` with no
//! `REFERENCES`). Two distinct random `Uuid`s for workspace A and B are
//! sufficient — no `workspaces::create` prerequisite, so a failure here is a
//! tenancy failure, never a wrong-reason FK violation.
//!
//! # Engine acquisition
//!
//! Acquisition-migrated onto the shared-engine fixture (T4, R-0037/R-0030/
//! R-0029): each test acquires the binary-wide shared engine via
//! `shared_engine::shared_engine()` and provisions its own fresh, isolated
//! database via `EmbeddedEngine::provision_test_database()` (which runs the
//! same schema-init sequence the old per-file `init(&engine, "vector")` call
//! did) — no per-file boot-serialization mutex needed. The fixture's own
//! get-or-init semantics guarantee exactly-once boot; the per-file
//! boot-serialization mutex previously here is retired as vestigial
//! (R-0029). Assertions below are unchanged from the pre-migration version.

#[path = "common/shared_engine.rs"]
mod shared_engine;

use mnemra_host::auth::role::Role;
use mnemra_host::auth::workspace_ctx::WorkspaceCtx;
use mnemra_host::builtins;
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use uuid::Uuid;

/// Construct an Admin `WorkspaceCtx` scoped to `workspace_id` for tests.
///
/// `Role::Admin` so the ctx carries full access — these tests probe the
/// *workspace* scoping boundary, not the role/permission boundary.
fn ctx_for(workspace_id: Uuid) -> WorkspaceCtx {
    WorkspaceCtx::new(workspace_id, Role::Admin, Uuid::new_v4())
}

// ===========================================================================
// agents::list_by_workspace — cross-workspace read isolation
// ===========================================================================

/// R-0006-e (agents read isolation):
///
/// GIVEN an agent registered under workspace A and a different agent registered
///   under workspace B,
/// WHEN `agents::list_by_workspace` is called with workspace A's `WorkspaceCtx`,
/// THEN the result contains A's agent and DOES NOT contain B's agent.
///
/// This is the load-bearing tenancy assertion: a ctx-ignoring implementation
/// (or a raw cross-tenant query) would leak B's row into A's view.
#[tokio::test]
async fn agents_list_by_workspace_excludes_other_workspace_rows() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    // Two distinct tenants.
    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let ctx_a = ctx_for(ws_a);
    let ctx_b = ctx_for(ws_b);

    let user_a = Uuid::new_v4();
    let user_b = Uuid::new_v4();

    // Register one agent in each workspace.
    let agent_a = builtins::agents::register(pool, &ctx_a, user_a, "agent-in-a", None)
        .await
        .expect("register under workspace A must succeed");
    let agent_b = builtins::agents::register(pool, &ctx_b, user_b, "agent-in-b", None)
        .await
        .expect("register under workspace B must succeed");

    // List under A's ctx — must see A, must NOT see B.
    let listed_a = builtins::agents::list_by_workspace(pool, &ctx_a)
        .await
        .expect("list_by_workspace under A must succeed");

    assert!(
        listed_a.iter().any(|a| a.id == agent_a.id),
        "workspace A's own agent must appear in A's list (R-0006-e)"
    );
    assert!(
        !listed_a.iter().any(|a| a.id == agent_b.id),
        "workspace B's agent MUST NOT leak into workspace A's list — \
         tenancy isolation breach (R-0006-e)"
    );
    assert!(
        listed_a.iter().all(|a| a.workspace_id == ws_a),
        "every row returned under A's ctx must belong to workspace A (R-0006-e); \
         a foreign workspace_id is a cross-tenant leak"
    );
}

/// R-0006-e (agents read isolation, symmetric direction):
///
/// The mirror of the above — listing under workspace B sees only B's agent,
/// never A's. Pins isolation in both directions so a one-sided filter bug
/// (e.g. a hard-coded constant) cannot pass.
#[tokio::test]
async fn agents_list_by_workspace_is_symmetric_isolation() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let ctx_a = ctx_for(ws_a);
    let ctx_b = ctx_for(ws_b);

    let agent_a = builtins::agents::register(pool, &ctx_a, Uuid::new_v4(), "sym-a", None)
        .await
        .expect("register under A must succeed");
    let agent_b = builtins::agents::register(pool, &ctx_b, Uuid::new_v4(), "sym-b", None)
        .await
        .expect("register under B must succeed");

    let listed_b = builtins::agents::list_by_workspace(pool, &ctx_b)
        .await
        .expect("list_by_workspace under B must succeed");

    assert!(
        listed_b.iter().any(|a| a.id == agent_b.id),
        "workspace B's own agent must appear in B's list (R-0006-e)"
    );
    assert!(
        !listed_b.iter().any(|a| a.id == agent_a.id),
        "workspace A's agent MUST NOT leak into workspace B's list (R-0006-e)"
    );
}

// ===========================================================================
// agents::register — write scoped by ctx, not by a caller-supplied workspace_id
// ===========================================================================

/// R-0006-e (agents write scoping):
///
/// GIVEN a `WorkspaceCtx` for workspace A,
/// WHEN `agents::register` is called with that ctx,
/// THEN the persisted row's `workspace_id` is A's id — the write is scoped by
///   the ctx, and the row is invisible from workspace B's list.
///
/// A register that scoped to anything other than `ctx.workspace_id()` would
/// place the row in the wrong tenant; this asserts the mutation lands in A and
/// only A.
#[tokio::test]
async fn agents_register_scopes_write_to_ctx_workspace() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let ctx_a = ctx_for(ws_a);
    let ctx_b = ctx_for(ws_b);

    let registered = builtins::agents::register(pool, &ctx_a, Uuid::new_v4(), "scoped-write", None)
        .await
        .expect("register under A must succeed");

    // The persisted row must carry A's workspace_id, derived from the ctx.
    assert_eq!(
        registered.workspace_id, ws_a,
        "register must scope the new row to ctx.workspace_id (workspace A) (R-0006-e)"
    );

    // And it must not be visible from workspace B.
    let listed_b = builtins::agents::list_by_workspace(pool, &ctx_b)
        .await
        .expect("list under B must succeed");
    assert!(
        !listed_b.iter().any(|a| a.id == registered.id),
        "an agent registered under A's ctx MUST NOT appear under B (R-0006-e)"
    );
}

// ===========================================================================
// sessions::list_active_by_workspace — cross-workspace read isolation
// ===========================================================================

/// R-0006-e (sessions read isolation):
///
/// GIVEN an active session opened under workspace A and one under workspace B,
/// WHEN `sessions::list_active_by_workspace` is called with A's `WorkspaceCtx`,
/// THEN the result contains A's session and DOES NOT contain B's session.
#[tokio::test]
async fn sessions_list_active_excludes_other_workspace_rows() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let ctx_a = ctx_for(ws_a);
    let ctx_b = ctx_for(ws_b);

    let session_a = builtins::sessions::open(pool, &ctx_a, Uuid::new_v4(), Uuid::new_v4())
        .await
        .expect("open session under A must succeed");
    let session_b = builtins::sessions::open(pool, &ctx_b, Uuid::new_v4(), Uuid::new_v4())
        .await
        .expect("open session under B must succeed");

    let active_a = builtins::sessions::list_active_by_workspace(pool, &ctx_a)
        .await
        .expect("list active under A must succeed");

    assert!(
        active_a.iter().any(|s| s.id == session_a.id),
        "workspace A's own session must appear in A's active list (R-0006-e)"
    );
    assert!(
        !active_a.iter().any(|s| s.id == session_b.id),
        "workspace B's session MUST NOT leak into workspace A's active list — \
         tenancy isolation breach (R-0006-e)"
    );
    assert!(
        active_a.iter().all(|s| s.workspace_id == ws_a),
        "every active session returned under A's ctx must belong to workspace A (R-0006-e)"
    );
}

// ===========================================================================
// sessions::open — write scoped by ctx
// ===========================================================================

/// R-0006-e (sessions write scoping):
///
/// GIVEN a `WorkspaceCtx` for workspace A,
/// WHEN `sessions::open` is called with that ctx,
/// THEN the opened session's `workspace_id` is A's id, and it does not appear in
///   workspace B's active list.
#[tokio::test]
async fn sessions_open_scopes_write_to_ctx_workspace() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let ctx_a = ctx_for(ws_a);
    let ctx_b = ctx_for(ws_b);

    let opened = builtins::sessions::open(pool, &ctx_a, Uuid::new_v4(), Uuid::new_v4())
        .await
        .expect("open under A must succeed");

    assert_eq!(
        opened.workspace_id, ws_a,
        "open must scope the new session to ctx.workspace_id (workspace A) (R-0006-e)"
    );

    let active_b = builtins::sessions::list_active_by_workspace(pool, &ctx_b)
        .await
        .expect("list active under B must succeed");
    assert!(
        !active_b.iter().any(|s| s.id == opened.id),
        "a session opened under A's ctx MUST NOT appear under B (R-0006-e)"
    );
}

// ===========================================================================
// R-0006-e EXPANSION — projects (create / list_by_workspace / exists / delete)
//
// `projects` is a full sibling-bypass surface flagged by the security audit:
// create/list/exists take a raw `workspace_id: Uuid` and `delete` takes only the
// row id (zero workspace predicate — a Class-2 / IDOR cross-tenant delete). The
// LOCKED target threads `&WorkspaceCtx` through all four and binds
// `ctx.workspace_id()` into the WHERE clause. These tests assert the observable
// tenancy boundary against those target signatures (RED until Forge threads them).
// ===========================================================================

/// R-0006-e (projects write scoping):
///
/// GIVEN a `WorkspaceCtx` for workspace A,
/// WHEN `projects::create` is called with that ctx,
/// THEN the persisted project's `workspace_id` is A's id (the write is scoped by
///   the ctx), and the project is invisible from workspace B's list.
///
/// A create that scoped to anything other than `ctx.workspace_id()` would place
/// the row in the wrong tenant.
#[tokio::test]
async fn projects_create_scopes_write_to_ctx_workspace() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let ctx_a = ctx_for(ws_a);
    let ctx_b = ctx_for(ws_b);

    let project = builtins::projects::create(pool, &ctx_a, "proj-scoped-write")
        .await
        .expect("create under A must succeed");

    assert_eq!(
        project.workspace_id, ws_a,
        "create must scope the new project to ctx.workspace_id (workspace A) (R-0006-e)"
    );

    // The project must not be visible from workspace B.
    let listed_b = builtins::projects::list_by_workspace(pool, &ctx_b)
        .await
        .expect("list under B must succeed");
    assert!(
        !listed_b.iter().any(|p| p.id == project.id),
        "a project created under A's ctx MUST NOT appear under B (R-0006-e)"
    );
}

/// R-0006-e (projects read isolation):
///
/// GIVEN a project created under workspace A and one under workspace B,
/// WHEN `projects::list_by_workspace` is called with A's `WorkspaceCtx`,
/// THEN the result contains A's project and DOES NOT contain B's project, and
///   every returned row belongs to workspace A.
///
/// A ctx-ignoring (or raw cross-tenant) query would leak B's row into A's view.
#[tokio::test]
async fn projects_list_by_workspace_excludes_other_workspace_rows() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let ctx_a = ctx_for(ws_a);
    let ctx_b = ctx_for(ws_b);

    let proj_a = builtins::projects::create(pool, &ctx_a, "proj-in-a")
        .await
        .expect("create under A must succeed");
    let proj_b = builtins::projects::create(pool, &ctx_b, "proj-in-b")
        .await
        .expect("create under B must succeed");

    let listed_a = builtins::projects::list_by_workspace(pool, &ctx_a)
        .await
        .expect("list under A must succeed");

    assert!(
        listed_a.iter().any(|p| p.id == proj_a.id),
        "workspace A's own project must appear in A's list (R-0006-e)"
    );
    assert!(
        !listed_a.iter().any(|p| p.id == proj_b.id),
        "workspace B's project MUST NOT leak into workspace A's list — \
         tenancy isolation breach (R-0006-e)"
    );
    assert!(
        listed_a.iter().all(|p| p.workspace_id == ws_a),
        "every project returned under A's ctx must belong to workspace A (R-0006-e); \
         a foreign workspace_id is a cross-tenant leak"
    );
}

/// R-0006-e (projects exists cross-tenant isolation):
///
/// GIVEN a project created under workspace A,
/// WHEN `projects::exists` is called with workspace B's `WorkspaceCtx` on A's
///   project id,
/// THEN it reports `false` — B cannot probe the existence of A's row by id.
///
/// `exists` scoped only by id (ignoring the ctx) would report `true` and leak the
/// existence of A's project to a foreign tenant.
#[tokio::test]
async fn projects_exists_is_false_cross_tenant() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let ctx_a = ctx_for(ws_a);
    let ctx_b = ctx_for(ws_b);

    let proj_a = builtins::projects::create(pool, &ctx_a, "proj-exists-probe")
        .await
        .expect("create under A must succeed");

    // Sanity: the owner (A) sees it.
    let exists_for_owner = builtins::projects::exists(pool, &ctx_a, proj_a.id)
        .await
        .expect("exists under A must succeed");
    assert!(
        exists_for_owner,
        "the owning workspace A must see its own project as existing (R-0006-e)"
    );

    // The attacker (B) must NOT — even though it holds A's project id.
    let exists_for_other = builtins::projects::exists(pool, &ctx_b, proj_a.id)
        .await
        .expect("exists under B must succeed (returns false, not error)");
    assert!(
        !exists_for_other,
        "workspace B MUST NOT see workspace A's project as existing — \
         cross-tenant existence leak (R-0006-e)"
    );
}

/// R-0006-e (projects delete IDOR — the headline security test):
///
/// GIVEN a project created under workspace A,
/// WHEN workspace B calls `projects::delete` with B's `WorkspaceCtx` on A's
///   project id,
/// THEN A's project is NOT deleted — the cross-tenant delete is refused and the
///   victim row survives.
///
/// `delete` today is `delete(pool, id)` with ZERO workspace predicate: any caller
/// that knows/guesses a project id deletes it regardless of tenant. The locked
/// target adds `AND workspace_id = ctx.workspace_id()`, so B's delete matches 0
/// rows. The load-bearing assertion is the SURVIVAL of A's project (proves the
/// predicate reached the SQL); we also assert B's call did not silently report
/// success, so a predicate-less signature swap fails here.
#[tokio::test]
async fn workspace_b_cannot_delete_workspace_a_project() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let ctx_a = ctx_for(ws_a);
    let ctx_b = ctx_for(ws_b);

    // Workspace A creates a project.
    let victim = builtins::projects::create(pool, &ctx_a, "a-victim-project")
        .await
        .expect("create under A must succeed");

    // Workspace B attempts to delete A's project using A's id but B's ctx.
    let attack = builtins::projects::delete(pool, &ctx_b, victim.id).await;

    // The cross-tenant delete must NOT report success. With the target predicate,
    // B matches 0 rows (the current impl maps 0 affected rows to NotFound). A
    // predicate-less signature swap that returned Ok(()) here is a live IDOR.
    assert!(
        attack.is_err(),
        "workspace B's cross-tenant delete of A's project MUST be refused, not \
         reported as success — silent Ok(()) is the IDOR (R-0006-e)"
    );

    // The load-bearing property: A's project still exists.
    let still_exists = builtins::projects::exists(pool, &ctx_a, victim.id)
        .await
        .expect("exists under A must succeed");
    assert!(
        still_exists,
        "workspace A's project MUST survive a cross-tenant delete by B — \
         cross-tenant destructive IDOR (R-0006-e)"
    );

    // And A's delete of its OWN project still works (the predicate must not block
    // the legitimate owner — proves the scope is correct, not just restrictive).
    builtins::projects::delete(pool, &ctx_a, victim.id)
        .await
        .expect("workspace A deleting its OWN project must succeed (R-0006-e)");
    let gone = builtins::projects::exists(pool, &ctx_a, victim.id)
        .await
        .expect("exists under A after owner delete must succeed");
    assert!(
        !gone,
        "after the owner (A) deletes its own project it must no longer exist (R-0006-e)"
    );
}

// ===========================================================================
// R-0006-e EXPANSION — sessions::close (IDOR)
//
// `close` today is `close(pool, session_id)` with `WHERE id = $1` only — a
// cross-tenant session-close IDOR. The session id is v4-random but is RETURNED
// to the client by open/list_active, so it is not an authorization boundary. The
// locked target adds `AND workspace_id = ctx.workspace_id()`.
// ===========================================================================

/// R-0006-e (sessions::close IDOR):
///
/// GIVEN an active session opened under workspace A,
/// WHEN workspace B calls `sessions::close` with B's `WorkspaceCtx` on A's
///   session id,
/// THEN A's session is NOT closed — it remains active.
///
/// The load-bearing assertion is that A's session is STILL in A's active list
/// after B's attempt (proves the workspace predicate reached the UPDATE). We also
/// assert B's call did not silently report success, so a predicate-less signature
/// swap fails here.
#[tokio::test]
async fn workspace_b_cannot_close_workspace_a_session() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let ctx_a = ctx_for(ws_a);
    let ctx_b = ctx_for(ws_b);

    // Workspace A opens a session.
    let session = builtins::sessions::open(pool, &ctx_a, Uuid::new_v4(), Uuid::new_v4())
        .await
        .expect("open under A must succeed");

    // Workspace B attempts to close A's session using A's id but B's ctx.
    let attack = builtins::sessions::close(pool, &ctx_b, session.id).await;

    assert!(
        attack.is_err(),
        "workspace B's cross-tenant close of A's session MUST be refused, not \
         reported as success — silent Ok(()) is the IDOR (R-0006-e)"
    );

    // The load-bearing property: A's session is still active.
    let active_a = builtins::sessions::list_active_by_workspace(pool, &ctx_a)
        .await
        .expect("list active under A must succeed");
    assert!(
        active_a.iter().any(|s| s.id == session.id),
        "workspace A's session MUST remain active after a cross-tenant close by B — \
         cross-tenant session-close IDOR (R-0006-e)"
    );

    // And A's close of its OWN session still works (legitimate owner not blocked).
    builtins::sessions::close(pool, &ctx_a, session.id)
        .await
        .expect("workspace A closing its OWN session must succeed (R-0006-e)");
    let active_after = builtins::sessions::list_active_by_workspace(pool, &ctx_a)
        .await
        .expect("list active under A after owner close must succeed");
    assert!(
        !active_after.iter().any(|s| s.id == session.id),
        "after the owner (A) closes its own session it must leave the active list (R-0006-e)"
    );
}
