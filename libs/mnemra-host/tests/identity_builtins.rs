//! Integration tests for identity builtins + init ordering (Task 15).
//!
//! Tests run against the real embedded Postgres engine (R-0018-b). No mocks.
//!
//! # Coverage
//!
//! - Init order gate (R-0002-c): init_all() fails without schema init (token unproducible).
//! - Init order gate (R-0002-c): plugin-load after init_all() → accepted.
//! - Init order gate (R-0002-c): type-level enforcement — no-token path is compile-time unrepresentable.
//! - Agent identity mismatch (R-0015-c): mismatched id → structured error.
//! - Agent canonical registration: correct id → success.
//! - Agent re-registration (idempotent): AlreadyRegistered.
//! - Workspace lifecycle: create, list, delete; CannotDeleteDefault guard.
//! - User CRUD: register, get, list.
//! - Session lifecycle: open, list active, close.
//! - Project CRUD: create, list, delete; plugin-scoping prerequisite.
//!
//! # Engine acquisition
//!
//! Acquisition-migrated onto the shared-engine fixture (T3 sub-run,
//! R-0030/R-0029): every test acquires the binary-wide shared engine via
//! `shared_engine::shared_engine()` — no per-file boot-serialization mutex
//! needed. Most tests then provision a fresh, isolated database via
//! `EmbeddedEngine::provision_test_database()`. The one test needing the
//! OPPOSITE precondition, `builtin_init_order_fails_without_schema_init`,
//! gets it for free: `shared_engine()` only boots the engine
//! (`EmbeddedEngine::start()`) — it never runs `schema::init::init()` against
//! the shared engine's own APP_DB (only `provision_test_database()` does
//! that, and only for its own freshly-created per-test database). So that
//! test reads `engine.pool` (bound to APP_DB) directly, with no
//! `provision_test_database()` call — `builtins::init_all()` is read-only
//! (`COUNT(*)` probes that fail fast on the first missing table), so this is
//! safe with no pollution risk, and no other test in this binary touches
//! APP_DB.

#[path = "common/shared_engine.rs"]
mod shared_engine;

use mnemra_host::auth::role::Role;
use mnemra_host::auth::workspace_ctx::WorkspaceCtx;
use mnemra_host::builtins;
use mnemra_host::builtins::agents::derive_agent_id;
use mnemra_host::schema::init::DEFAULT_WORKSPACE_ID;
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use uuid::Uuid;

/// Construct an Admin `WorkspaceCtx` scoped to `workspace_id` for tests.
///
/// T13.3 / R-0006-e: the 4 identity-query builtins are threaded by
/// `&WorkspaceCtx` (type-level tenancy), so test call sites build a ctx rather
/// than passing a raw `workspace_id: Uuid`.
///
/// Uses `WorkspaceCtx::new` (the public production constructor). `for_test` is
/// `#[cfg(test)]`-gated inside the library crate and is NOT visible from this
/// integration-test binary (separate crate; the gate applies to the library's
/// own unit tests only) — mirrors the convention already used in
/// `tests/permissions.rs`.
fn ctx_for(workspace_id: Uuid) -> WorkspaceCtx {
    WorkspaceCtx::new(workspace_id, Role::Admin, Uuid::new_v4())
}

// ---------------------------------------------------------------------------
// R-0002-c: plugin-load gate is enforced at compile time + via init failure
// ---------------------------------------------------------------------------

// The "no token, load rejected" direction of R-0002-c is enforced at
// COMPILE TIME: `load_plugin` requires a `&BuiltinsReady`, and `BuiltinsReady`
// has a private constructor only `init_all()` can call. There is no runtime
// rejection path because the type system makes the bad call unrepresentable.
//
// Runtime coverage is provided by two tests below:
// - `builtin_init_order_fails_without_schema_init`: proves `init_all()` refuses
//   to mint the token when prerequisites are absent (so the token is unforgeable
//   and unproducible without a valid substrate).
// - `builtin_init_order_plugin_load_accepted_after_init_all`: proves the happy
//   path — token minted, plugin load proceeds.
//
// Together these three mechanisms (type-level unrepresentability + init guard +
// happy-path acceptance) fully characterize the R-0002-c gate.

/// R-0002-c: After `init_all()` succeeds, plugin load is accepted.
///
/// Validates the happy path: builtins ready → plugin load proceeds.
#[tokio::test]
async fn builtin_init_order_plugin_load_accepted_after_init_all() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    // init_all() must succeed — all 7 builtin tables exist post schema init.
    let ready = builtins::init_all(&db.pool)
        .await
        .expect("init_all must succeed after schema init");

    // Plugin load with BuiltinsReady succeeds.
    let result = builtins::load_plugin(&ready, "mnemra-echo");
    assert!(
        result.is_ok(),
        "plugin load must succeed after init_all() (R-0002-c); got: {:?}",
        result
    );
}

/// R-0002-c: init_all() fails with SchemaNotInitialized if schema init was
/// not run (no `default` workspace exists).
#[tokio::test]
async fn builtin_init_order_fails_without_schema_init() {
    // Uses the shared engine's own APP_DB directly (no
    // `provision_test_database()` call) — that is exactly the "schema init
    // never ran" substrate this test needs: `shared_engine()` only boots the
    // engine, it never runs `schema::init::init()` against APP_DB, and no
    // other test in this binary reads or writes through `engine.pool`
    // (every other test uses its own provisioned database). `init_all()`
    // itself never writes (read-only COUNT(*) probes that fail fast on the
    // first missing table), so probing APP_DB directly here is safe.
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;

    let result = builtins::init_all(engine.pool.as_ref()).await;

    // Without schema init, the workspaces table doesn't exist → DB error
    // (or SchemaNotInitialized if we could query but found no default).
    // Either way it must be an error — the gate must not succeed.
    assert!(
        result.is_err(),
        "init_all must fail if schema init has not run (R-0002-c); got Ok"
    );
}

// ---------------------------------------------------------------------------
// R-0002-b: builtins are host code, not sandbox code
// ---------------------------------------------------------------------------

/// R-0002-b: Builtins execute as host code, not inside the Wasmtime sandbox.
///
/// # Enforcement model (Task 21 update)
///
/// Task 21 added `wasmtime` to `mnemra-host` for the plugin runtime (`plugin/`
/// module). The crate-level Cargo.toml dep-check that existed before Task 21
/// is no longer meaningful — `wasmtime` is intentionally present. The durable
/// guard is a directory-scoped source scan: assert that NO file under
/// `libs/mnemra-host/builtins/` contains a `wasmtime` usage (`use wasmtime` or
/// `wasmtime::` path). This fires if a future change smuggles `wasmtime::Store`
/// or any Wasmtime runtime type into a builtin, which would violate R-0002-b.
///
/// The scan uses `use wasmtime` and `wasmtime::` as markers — comment-only
/// mentions are accepted (the match is on import + qualified path usage).
#[test]
fn builtins_are_host_code_not_sandboxed_r0002b() {
    // R-0002-b: builtins SHALL NOT execute inside the Wasmtime plugin sandbox.
    //
    // Proof by construction: scan every .rs file under builtins/ and assert
    // none contains a wasmtime import or qualified path reference. This is a
    // durable guard — it passes today (zero references) and fails the moment
    // a builtin module imports or uses any Wasmtime runtime type.
    let builtins_dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"))).join("builtins");

    let mut violations: Vec<String> = Vec::new();

    for entry in std::fs::read_dir(&builtins_dir).expect("builtins/ directory must be readable") {
        let entry = entry.expect("builtins/ entry must be readable");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

        // Match import statements and qualified path usage.
        // Comment-only mentions are not a violation; these patterns hit real usage.
        if source.contains("use wasmtime") || source.contains("wasmtime::") {
            violations.push(path.display().to_string());
        }
    }

    assert!(
        violations.is_empty(),
        "R-0002-b violation: builtins must not reference the Wasmtime runtime. \
        Found wasmtime usage in: {violations:?}. \
        Builtins are host code (R-0002-b); Wasmtime belongs in plugin/ only."
    );
}

// ---------------------------------------------------------------------------
// R-0015-c: agent identity derivation + mismatch → structured error
// ---------------------------------------------------------------------------

/// R-0015-c (canonical registration): register an agent with the correctly
/// derived id — succeeds and returns the agent row.
#[tokio::test]
async fn agent_canonical_registration_succeeds() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let user_id = Uuid::new_v4();
    let agent_name = "test-agent-canonical";

    let canonical_id = derive_agent_id(workspace_id, user_id, agent_name);

    // T13.3 / R-0006-e: register is threaded by &WorkspaceCtx (tenancy at the
    // type level), no longer a raw workspace_id param.
    let ctx = ctx_for(workspace_id);

    // Supply the canonically derived id — registration must succeed.
    let result =
        builtins::agents::register(pool, &ctx, user_id, agent_name, Some(canonical_id)).await;

    let agent = result.expect("canonical agent registration must succeed (R-0015-c)");
    assert_eq!(
        agent.id, canonical_id,
        "returned id must match canonical derivation"
    );
    assert_eq!(agent.workspace_id, workspace_id);
    assert_eq!(agent.user_id, user_id);
    assert_eq!(agent.agent_name, agent_name);
}

/// R-0015-c: registration without a supplied id uses canonical derivation.
#[tokio::test]
async fn agent_registration_without_supplied_id_uses_canonical() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let user_id = Uuid::new_v4();
    let agent_name = "test-agent-no-id";

    let expected_id = derive_agent_id(workspace_id, user_id, agent_name);

    // T13.3 / R-0006-e: register is threaded by &WorkspaceCtx.
    let ctx = ctx_for(workspace_id);

    // No supplied_agent_id — the builtin derives the id canonically.
    let agent = builtins::agents::register(pool, &ctx, user_id, agent_name, None)
        .await
        .expect("registration without supplied id must succeed");

    assert_eq!(
        agent.id, expected_id,
        "agent id must match canonical derivation even when not supplied"
    );
}

/// R-0015-c: supplying a NON-canonical id → IdentityMismatch structured error.
///
/// This is the must-test correctness assertion for R-0015-c.
#[tokio::test]
async fn agent_identity_mismatch_returns_structured_error() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let user_id = Uuid::new_v4();
    let agent_name = "test-agent-mismatch";

    let canonical_id = derive_agent_id(workspace_id, user_id, agent_name);
    // Construct a wrong id that is definitely not the canonical derivation.
    let wrong_id = Uuid::new_v4();
    assert_ne!(
        wrong_id, canonical_id,
        "test setup: wrong_id must differ from canonical"
    );

    // T13.3 / R-0006-e: register is threaded by &WorkspaceCtx.
    let ctx = ctx_for(workspace_id);

    let result = builtins::agents::register(pool, &ctx, user_id, agent_name, Some(wrong_id)).await;

    match result {
        Err(builtins::agents::RegisterError::IdentityMismatch {
            supplied_id,
            canonical_id: returned_canonical,
        }) => {
            assert_eq!(
                supplied_id, wrong_id,
                "mismatch error must name the supplied id"
            );
            assert_eq!(
                returned_canonical, canonical_id,
                "mismatch error must name the canonical id"
            );
        }
        other => panic!(
            "expected RegisterError::IdentityMismatch (R-0015-c), got: {:?}",
            other
        ),
    }
}

/// R-0015-c: re-registration with the identical canonical triple →
/// AlreadyRegistered (idempotent, not silent overwrite).
#[tokio::test]
async fn agent_re_registration_returns_already_registered() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let user_id = Uuid::new_v4();
    let agent_name = "test-agent-idempotent";

    // T13.3 / R-0006-e: register is threaded by &WorkspaceCtx.
    let ctx = ctx_for(workspace_id);

    // First registration succeeds.
    let first = builtins::agents::register(pool, &ctx, user_id, agent_name, None)
        .await
        .expect("first registration must succeed");

    // Second registration with the same triple → AlreadyRegistered.
    let result = builtins::agents::register(pool, &ctx, user_id, agent_name, None).await;

    match result {
        Err(builtins::agents::RegisterError::AlreadyRegistered { existing_id }) => {
            assert_eq!(
                existing_id, first.id,
                "AlreadyRegistered must return the original id"
            );
        }
        other => panic!(
            "expected RegisterError::AlreadyRegistered on re-registration, got: {:?}",
            other
        ),
    }
}

// ---------------------------------------------------------------------------
// R-0015-a/h: workspace lifecycle
// ---------------------------------------------------------------------------

/// R-0015-a/h: create, list, and delete workspace lifecycle.
#[tokio::test]
async fn workspace_lifecycle_create_list_delete() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    // Create a new workspace.
    let ws = builtins::workspaces::create(pool, "test-ws-lifecycle")
        .await
        .expect("workspace create must succeed");

    assert_eq!(ws.name, "test-ws-lifecycle");

    // List must include it.
    let list = builtins::workspaces::list(pool)
        .await
        .expect("workspace list must succeed");

    assert!(
        list.iter().any(|w| w.id == ws.id),
        "created workspace must appear in list"
    );

    // Delete it.
    builtins::workspaces::delete(pool, ws.id)
        .await
        .expect("workspace delete must succeed");

    // Must no longer appear in list.
    let list_after = builtins::workspaces::list(pool)
        .await
        .expect("workspace list after delete must succeed");

    assert!(
        !list_after.iter().any(|w| w.id == ws.id),
        "deleted workspace must not appear in list"
    );
}

/// R-0015-h: the `default` workspace cannot be deleted.
#[tokio::test]
async fn workspace_cannot_delete_default() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let result = builtins::workspaces::delete(pool, DEFAULT_WORKSPACE_ID).await;

    match result {
        Err(builtins::workspaces::WorkspaceError::CannotDeleteDefault) => {
            // Expected.
        }
        other => panic!(
            "expected WorkspaceError::CannotDeleteDefault for default workspace, got: {:?}",
            other
        ),
    }
}

// ---------------------------------------------------------------------------
// R-0015-b: user CRUD
// ---------------------------------------------------------------------------

/// R-0015-b: register, get, and list users.
#[tokio::test]
async fn user_crud_round_trip() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    // Register a user.
    let user = builtins::users::register(pool, "alice", Some("Alice Wonderland"))
        .await
        .expect("user register must succeed");

    assert_eq!(user.username, "alice");
    assert_eq!(user.display_name, Some("Alice Wonderland".to_string()));

    // Get by id.
    let fetched = builtins::users::get(pool, user.id)
        .await
        .expect("user get must succeed");

    assert_eq!(fetched.id, user.id);
    assert_eq!(fetched.username, "alice");

    // List must include the user.
    let list = builtins::users::list(pool)
        .await
        .expect("user list must succeed");

    assert!(
        list.iter().any(|u| u.id == user.id),
        "registered user must appear in list"
    );
}

/// R-0015-b: registering a duplicate username returns AlreadyExists.
#[tokio::test]
async fn user_duplicate_username_returns_error() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    builtins::users::register(pool, "bob", None)
        .await
        .expect("first user register must succeed");

    let result = builtins::users::register(pool, "bob", None).await;

    match result {
        Err(builtins::users::UserError::AlreadyExists { username }) => {
            assert_eq!(username, "bob");
        }
        other => panic!(
            "expected UserError::AlreadyExists on duplicate username, got: {:?}",
            other
        ),
    }
}

// ---------------------------------------------------------------------------
// R-0015-e: session lifecycle
// ---------------------------------------------------------------------------

/// R-0015-e: open, list active, and close a session.
#[tokio::test]
async fn session_lifecycle_open_list_close() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let user_id = Uuid::new_v4();
    let agent_id = Uuid::new_v4();

    // T13.3 / R-0006-e: open + list_active_by_workspace are threaded by
    // &WorkspaceCtx (tenancy at the type level), no longer raw workspace_id.
    let ctx = ctx_for(workspace_id);

    // Open a session.
    let session = builtins::sessions::open(pool, &ctx, user_id, agent_id)
        .await
        .expect("session open must succeed");

    assert_eq!(session.workspace_id, workspace_id);
    assert_eq!(session.user_id, user_id);
    assert_eq!(session.agent_id, agent_id);
    assert!(session.ended_at.is_none(), "new session must be active");

    // List active sessions for the workspace — must include it.
    let active = builtins::sessions::list_active_by_workspace(pool, &ctx)
        .await
        .expect("list active sessions must succeed");

    assert!(
        active.iter().any(|s| s.id == session.id),
        "opened session must appear in active list"
    );

    // Close the session. R-0006-e expansion: `close` is now threaded by
    // `&WorkspaceCtx` (was `close(pool, session_id)` — a cross-tenant IDOR). The
    // owner's ctx closes the owner's session.
    builtins::sessions::close(pool, &ctx, session.id)
        .await
        .expect("session close must succeed");

    // Must no longer appear in active list.
    let active_after = builtins::sessions::list_active_by_workspace(pool, &ctx)
        .await
        .expect("list active sessions after close must succeed");

    assert!(
        !active_after.iter().any(|s| s.id == session.id),
        "closed session must not appear in active list"
    );
}

// ---------------------------------------------------------------------------
// R-0015-g: project CRUD + plugin scoping prerequisite
// ---------------------------------------------------------------------------

/// R-0015-g: create, list, and delete projects; plugin scoping prerequisite.
#[tokio::test]
async fn project_crud_round_trip_and_exists_check() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let workspace_id = DEFAULT_WORKSPACE_ID;

    // R-0006-e expansion: projects builtins are now threaded by `&WorkspaceCtx`
    // (create/list_by_workspace/exists/delete), no longer a raw `workspace_id`
    // (and `delete` previously took none — a cross-tenant IDOR).
    let ctx = ctx_for(workspace_id);

    // Create a project.
    let project = builtins::projects::create(pool, &ctx, "my-project")
        .await
        .expect("project create must succeed");

    assert_eq!(project.name, "my-project");
    assert_eq!(project.workspace_id, workspace_id);

    // Exists check — the plugin scoping prerequisite (R-0015-g).
    let exists = builtins::projects::exists(pool, &ctx, project.id)
        .await
        .expect("project exists check must succeed");
    assert!(exists, "created project must report as existing");

    // A random id must not exist.
    let missing = builtins::projects::exists(pool, &ctx, Uuid::new_v4())
        .await
        .expect("project exists check for missing id must succeed");
    assert!(!missing, "random id must not report as existing");

    // List must include it.
    let list = builtins::projects::list_by_workspace(pool, &ctx)
        .await
        .expect("project list must succeed");

    assert!(
        list.iter().any(|p| p.id == project.id),
        "created project must appear in list"
    );

    // Delete it (the owner's ctx deletes the owner's project).
    builtins::projects::delete(pool, &ctx, project.id)
        .await
        .expect("project delete must succeed");

    // Must no longer exist.
    let exists_after = builtins::projects::exists(pool, &ctx, project.id)
        .await
        .expect("project exists check after delete must succeed");
    assert!(!exists_after, "deleted project must not report as existing");
}

/// R-0015-g: duplicate project name in same workspace returns AlreadyExists.
#[tokio::test]
async fn project_duplicate_name_in_workspace_returns_error() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = &db.pool;

    let workspace_id = DEFAULT_WORKSPACE_ID;

    // R-0006-e expansion: projects::create is threaded by `&WorkspaceCtx`.
    let ctx = ctx_for(workspace_id);

    builtins::projects::create(pool, &ctx, "dup-project")
        .await
        .expect("first project create must succeed");

    let result = builtins::projects::create(pool, &ctx, "dup-project").await;

    match result {
        Err(builtins::projects::ProjectError::AlreadyExists {
            workspace_id: ws,
            name,
        }) => {
            assert_eq!(ws, workspace_id);
            assert_eq!(name, "dup-project");
        }
        other => panic!(
            "expected ProjectError::AlreadyExists on duplicate name, got: {:?}",
            other
        ),
    }
}
