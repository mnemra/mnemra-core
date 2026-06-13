//! RED-phase permission-matrix tests (Task 14 of TDD pair 14-RED/14-GREEN).
//!
//! # Purpose
//!
//! These tests pin the **behavioral** permission matrix that Task 14 (GREEN phase)
//! must satisfy. They import the real `auth::permissions` seam and assert on
//! actual `authorize()` return values — not structural/shape assertions. The
//! file does NOT compile until Task 14 creates `auth/permissions.rs` and
//! `builtins/permissions.rs`; that build failure IS the red phase.
//!
//! # RED-phase rationale (deviation from "compiles + fails behaviorally")
//!
//! The standard red-phase discipline is: compile clean, fail at assertion. This
//! file deviates deliberately. The permission matrix is the authorization boundary;
//! two prior red phases were caught passing vacuously (the seam didn't exist yet,
//! so no assertion ran). A syn-structural test would compile but would pass against
//! a vacuous `fn authorize(..) -> Ok(())` stub, destroying the independence of
//! independent test authorship. The load-bearing value of Glitch-first here is
//! proving the DENY paths at runtime — that requires executing `authorize()`, which
//! requires the real types. Therefore: the build failure is the red signal, not an
//! assertion failure. Task 14 GREEN creates the types; the tests then flip.
//!
//! Documented as a deviation in dispatch-1012-report.md. The maintainer may opt to
//! create a minimal stub of (`Verb`, `PermissionError`, `authorize`) in
//! `auth/permissions.rs` (no logic, just shapes) before GREEN if they prefer
//! compile-clean red. That file is forbid_scope for this dispatch; that choice is
//! theirs.
//!
//! # Coverage design
//!
//! Both directions are pinned so neither a constant-`Ok` nor a constant-`Err` stub
//! survives the suite:
//!   - `ReadObserver` DENY: every write, every CLI control-plane op, every
//!     workspace lifecycle op, admin session management.
//!   - `ReadObserver` ALLOW: the read-path MCP verbs (`artifact.get`,
//!     `artifact.list`, projection queries).
//!   - `Admin` ALLOW: every category.
//!   - Structured error contents asserted (role + verb), not just `.is_err()`.
//!
//! # R-0009-g note
//!
//! R-0009-g ("no Postgres RLS policies at V0") is a "shall not" invariant, not a
//! behavioral check. It is documented here as a non-goal and enforced by code
//! review. No executable test for it is included — manufacturing a `pg_policies`
//! check would require a live DB and is out of scope for the permission-matrix
//! tests. The spec satisfies R-0009-g through the absence of any `CREATE POLICY`
//! in migrations.
//!
//! # Spec requirements traced
//!
//! - R-0009-c: Admin authorizes all MCP verb categories + CLI control-plane +
//!             admin session management.
//! - R-0009-d: ReadObserver authorizes only read-path MCP verbs; write verbs,
//!             CLI control-plane, workspace lifecycle denied at host-fn boundary.
//! - R-0009-e: Workspace lifecycle ops require Admin; ReadObserver → structured
//!             permission error.
//! - R-0009-g: No Postgres RLS at V0; enforcement at application layer only
//!             (non-goal / non-executable — documented above).
//! - R-0015-f: `permissions` builtin checks plugin verb access at the host layer
//!             before plugin dispatch.
//!
//! # Cross-dispatch handoff: seam contract for Task 14 GREEN
//!
//! Task 14 GREEN must implement the following in `libs/mnemra-host/auth/permissions.rs`:
//!
//! ```rust
//! // File: libs/mnemra-host/auth/permissions.rs
//! //
//! // This is NOT the real file — it is the binding contract these tests assume.
//!
//! use crate::auth::workspace_ctx::WorkspaceCtx;
//! use crate::auth::role::Role;
//!
//! /// Verb taxonomy for the permission matrix (R-0009-c/d/e).
//! ///
//! /// PERMISSION MATRIX (canonical — pinned here, enforced in tests):
//! ///
//! /// | Category              | Admin  | ReadObserver |
//! /// |-----------------------|--------|--------------|
//! /// | ReadPathMcp           |  Allow |    Allow     |
//! /// | WritePathMcp          |  Allow |    Deny      |
//! /// | CliControlPlane       |  Allow |    Deny      |
//! /// | WorkspaceLif cycle    |  Allow |    Deny      |
//! /// | AdminSessionMgmt      |  Allow |    Deny      |
//! /// | PluginVerb(read_path) |  Allow |    Allow     |
//! /// | PluginVerb(write_path)|  Allow |    Deny      |
//! ///
//! /// Specific verbs per category (non-exhaustive; tests pin these explicitly):
//! ///
//! ///   ReadPathMcp:       artifact.get, artifact.list, projection.query
//! ///   WritePathMcp:      artifact.create, artifact.update, artifact.delete
//! ///   CliControlPlane:   workspace_lifecycle, token_rotation, migration_trigger,
//! ///                      backup_trigger
//! ///   WorkspaceLifecycle: workspace_create, workspace_delete
//! ///   AdminSessionMgmt:  session_list, session_revoke
//! ///   PluginVerb:        read (allow both roles), write (deny ReadObserver)
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub enum Verb {
//!     // --- Read-path MCP verbs (R-0009-d allow list) ---
//!     /// artifact.get (R-0009-d)
//!     ArtifactGet,
//!     /// artifact.list (R-0009-d)
//!     ArtifactList,
//!     /// projection.query (R-0009-d)
//!     ProjectionQuery,
//!
//!     // --- Write-path MCP verbs (R-0009-d deny for ReadObserver) ---
//!     /// artifact.create (R-0009-d)
//!     ArtifactCreate,
//!     /// artifact.update (R-0009-d)
//!     ArtifactUpdate,
//!     /// artifact.delete (R-0009-d)
//!     ArtifactDelete,
//!
//!     // --- CLI control-plane ops (R-0009-c / R-0009-d deny for ReadObserver) ---
//!     /// workspace lifecycle (create, delete) via CLI (R-0009-c, R-0009-e)
//!     WorkspaceLifecycle,
//!     /// token rotation via CLI (R-0009-c, R-0009-d)
//!     TokenRotation,
//!     /// migration trigger via CLI (R-0009-c, R-0009-d)
//!     MigrationTrigger,
//!     /// backup trigger via CLI (R-0009-c, R-0009-d)
//!     BackupTrigger,
//!
//!     // --- Workspace lifecycle (R-0009-e — named separately for precision) ---
//!     /// workspace create (R-0009-e)
//!     WorkspaceCreate,
//!     /// workspace delete (R-0009-e)
//!     WorkspaceDelete,
//!
//!     // --- Admin session management (R-0009-c) ---
//!     /// admin session list (R-0009-c)
//!     AdminSessionList,
//!     /// admin session revoke (R-0009-c)
//!     AdminSessionRevoke,
//!
//!     // --- Plugin host-layer verb (R-0015-f) ---
//!     /// plugin verb at host layer before dispatch; read = allow ReadObserver
//!     PluginReadVerb,
//!     /// plugin verb at host layer before dispatch; write = deny ReadObserver
//!     PluginWriteVerb,
//! }
//!
//! /// Structured permission error (role + attempted verb).
//! ///
//! /// MUST carry both fields so tests can assert not just `.is_err()` but the
//! /// structured contents, blocking a vacuous-error stub from passing.
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub struct PermissionError {
//!     /// The role that was denied.
//!     pub role: Role,
//!     /// The verb that was attempted.
//!     pub verb: Verb,
//! }
//!
//! impl std::fmt::Display for PermissionError {
//!     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//!         write!(
//!             f,
//!             "permission denied: role {:?} is not authorized for verb {:?}",
//!             self.role, self.verb
//!         )
//!     }
//! }
//!
//! impl std::error::Error for PermissionError {}
//!
//! /// Check whether `ctx.role` is authorized to execute `verb`.
//! ///
//! /// # Permission matrix (R-0009-c/d/e)
//! ///
//! /// Admin: all verbs authorized.
//! /// ReadObserver: only read-path MCP verbs (ArtifactGet, ArtifactList,
//! ///               ProjectionQuery, PluginReadVerb) authorized; everything else
//! ///               returns Err(PermissionError { role: ReadObserver, verb }).
//! ///
//! /// # Application-layer enforcement (R-0009-g)
//! ///
//! /// This function enforces at the application layer only. No Postgres RLS
//! /// policies are activated at V0.
//! pub fn authorize(ctx: &WorkspaceCtx, verb: &Verb) -> Result<(), PermissionError> {
//!     // Task 14 implements this.
//!     todo!()
//! }
//! ```
//!
//! Task 14 must also add `pub mod permissions;` to `auth.rs`.
//!
//! ## Permissions builtin seam (R-0015-f)
//!
//! Task 14 must implement in `libs/mnemra-host/builtins/permissions.rs`:
//!
//! ```rust
//! // File: libs/mnemra-host/builtins/permissions.rs
//! //
//! // This is NOT the real file — it is the binding contract these tests assume.
//!
//! use crate::auth::permissions::{authorize, Verb};
//! use crate::auth::workspace_ctx::WorkspaceCtx;
//!
//! /// Pre-dispatch host-layer plugin verb authorization (R-0015-f).
//! ///
//! /// Called by the host dispatch path BEFORE invoking plugin-provided handlers.
//! /// Returns Ok(()) if the role in `ctx` is authorized for `verb`, or
//! /// Err(PermissionError) if denied.
//! ///
//! /// V0: delegates to `auth::permissions::authorize`. The builtin wrapper exists
//! /// so the plugin dispatch path has a named hook point that future tasks can
//! /// extend (e.g., per-plugin grant overrides) without touching `authorize`.
//! pub fn check_plugin_verb(
//!     ctx: &WorkspaceCtx,
//!     verb: &Verb,
//! ) -> Result<(), crate::auth::permissions::PermissionError> {
//!     authorize(ctx, verb)
//! }
//! ```
//!
//! Task 14 must also add `pub mod permissions;` to `builtins.rs`.

use mnemra_host::auth::{
    permissions::{PermissionError, Verb, authorize},
    role::Role,
    workspace_ctx::WorkspaceCtx,
};
use mnemra_host::builtins::permissions::check_plugin_verb;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Construct an Admin WorkspaceCtx for tests.
///
/// Uses `WorkspaceCtx::new` (the public production constructor). `for_test`
/// is `#[cfg(test)]`-gated inside the library crate — it is NOT visible from
/// integration test binaries (separate crates; the gate applies to the library's
/// unit tests only). `new` is the correct integration-test constructor.
fn admin_ctx() -> WorkspaceCtx {
    WorkspaceCtx::new(Uuid::new_v4(), Role::Admin, Uuid::new_v4())
}

/// Construct a ReadObserver WorkspaceCtx for tests.
///
/// Uses `WorkspaceCtx::new` for the same reason as `admin_ctx`.
fn read_observer_ctx() -> WorkspaceCtx {
    WorkspaceCtx::new(Uuid::new_v4(), Role::ReadObserver, Uuid::new_v4())
}

// ---------------------------------------------------------------------------
// R-0009-c — Admin: all categories authorized
// ---------------------------------------------------------------------------

/// R-0009-c: Admin authorizes all read-path MCP verbs.
#[test]
fn admin_authorized_for_read_path_mcp_verbs() {
    // R-0009-c
    let ctx = admin_ctx();
    assert!(
        authorize(&ctx, &Verb::ArtifactGet).is_ok(),
        "Admin must be authorized for ArtifactGet (R-0009-c)"
    );
    assert!(
        authorize(&ctx, &Verb::ArtifactList).is_ok(),
        "Admin must be authorized for ArtifactList (R-0009-c)"
    );
    assert!(
        authorize(&ctx, &Verb::ProjectionQuery).is_ok(),
        "Admin must be authorized for ProjectionQuery (R-0009-c)"
    );
}

/// R-0009-c: Admin authorizes all write-path MCP verbs.
#[test]
fn admin_authorized_for_write_path_mcp_verbs() {
    // R-0009-c
    let ctx = admin_ctx();
    assert!(
        authorize(&ctx, &Verb::ArtifactCreate).is_ok(),
        "Admin must be authorized for ArtifactCreate (R-0009-c)"
    );
    assert!(
        authorize(&ctx, &Verb::ArtifactUpdate).is_ok(),
        "Admin must be authorized for ArtifactUpdate (R-0009-c)"
    );
    assert!(
        authorize(&ctx, &Verb::ArtifactDelete).is_ok(),
        "Admin must be authorized for ArtifactDelete (R-0009-c)"
    );
}

/// R-0009-c: Admin authorizes all CLI control-plane operations.
///
/// All four named CLI control-plane ops are checked individually (R-0009-c
/// enumerates workspace lifecycle, token rotation, migration trigger, backup
/// trigger — collapsing into one call would under-test the matrix).
#[test]
fn admin_authorized_for_cli_control_plane_ops() {
    // R-0009-c
    let ctx = admin_ctx();
    assert!(
        authorize(&ctx, &Verb::WorkspaceLifecycle).is_ok(),
        "Admin must be authorized for WorkspaceLifecycle via CLI (R-0009-c)"
    );
    assert!(
        authorize(&ctx, &Verb::TokenRotation).is_ok(),
        "Admin must be authorized for TokenRotation via CLI (R-0009-c)"
    );
    assert!(
        authorize(&ctx, &Verb::MigrationTrigger).is_ok(),
        "Admin must be authorized for MigrationTrigger via CLI (R-0009-c)"
    );
    assert!(
        authorize(&ctx, &Verb::BackupTrigger).is_ok(),
        "Admin must be authorized for BackupTrigger via CLI (R-0009-c)"
    );
}

/// R-0009-c / R-0009-e: Admin authorizes workspace lifecycle (create + delete).
#[test]
fn admin_authorized_for_workspace_lifecycle() {
    // R-0009-c, R-0009-e
    let ctx = admin_ctx();
    assert!(
        authorize(&ctx, &Verb::WorkspaceCreate).is_ok(),
        "Admin must be authorized for WorkspaceCreate (R-0009-c, R-0009-e)"
    );
    assert!(
        authorize(&ctx, &Verb::WorkspaceDelete).is_ok(),
        "Admin must be authorized for WorkspaceDelete (R-0009-c, R-0009-e)"
    );
}

/// R-0009-c: Admin authorizes admin session management.
#[test]
fn admin_authorized_for_admin_session_management() {
    // R-0009-c
    let ctx = admin_ctx();
    assert!(
        authorize(&ctx, &Verb::AdminSessionList).is_ok(),
        "Admin must be authorized for AdminSessionList (R-0009-c)"
    );
    assert!(
        authorize(&ctx, &Verb::AdminSessionRevoke).is_ok(),
        "Admin must be authorized for AdminSessionRevoke (R-0009-c)"
    );
}

/// R-0009-c / R-0015-f: Admin authorizes all plugin verb categories.
#[test]
fn admin_authorized_for_plugin_verbs() {
    // R-0009-c, R-0015-f
    let ctx = admin_ctx();
    assert!(
        authorize(&ctx, &Verb::PluginReadVerb).is_ok(),
        "Admin must be authorized for PluginReadVerb (R-0009-c, R-0015-f)"
    );
    assert!(
        authorize(&ctx, &Verb::PluginWriteVerb).is_ok(),
        "Admin must be authorized for PluginWriteVerb (R-0009-c, R-0015-f)"
    );
}

// ---------------------------------------------------------------------------
// R-0009-d — ReadObserver: read-path MCP verbs ALLOWED
// ---------------------------------------------------------------------------

/// R-0009-d: ReadObserver is authorized for read-path MCP verbs.
///
/// This set of allows is the counter-weight that prevents a constant-Err
/// stub from satisfying the suite. Every read-path verb must succeed.
#[test]
fn read_observer_authorized_for_read_path_mcp_verbs() {
    // R-0009-d
    let ctx = read_observer_ctx();
    assert!(
        authorize(&ctx, &Verb::ArtifactGet).is_ok(),
        "ReadObserver must be authorized for ArtifactGet (R-0009-d)"
    );
    assert!(
        authorize(&ctx, &Verb::ArtifactList).is_ok(),
        "ReadObserver must be authorized for ArtifactList (R-0009-d)"
    );
    assert!(
        authorize(&ctx, &Verb::ProjectionQuery).is_ok(),
        "ReadObserver must be authorized for ProjectionQuery (R-0009-d)"
    );
}

// ---------------------------------------------------------------------------
// R-0009-d — ReadObserver: write-path MCP verbs DENIED (structured error)
// ---------------------------------------------------------------------------

/// R-0009-d: ReadObserver is denied ArtifactCreate with a structured error.
///
/// Assertion is on the structured error contents (role + verb), not just
/// `.is_err()`, preventing a vacuous `Err(PermissionError { .. })` stub from
/// passing without encoding the matrix.
#[test]
fn read_observer_denied_artifact_create_structured_error() {
    // R-0009-d
    let ctx = read_observer_ctx();
    let result = authorize(&ctx, &Verb::ArtifactCreate);
    assert!(
        result.is_err(),
        "ReadObserver must NOT be authorized for ArtifactCreate (R-0009-d); got Ok(())"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.role,
        Role::ReadObserver,
        "PermissionError.role must be ReadObserver (R-0009-d); got {:?}",
        err.role
    );
    assert_eq!(
        err.verb,
        Verb::ArtifactCreate,
        "PermissionError.verb must be ArtifactCreate (R-0009-d); got {:?}",
        err.verb
    );
}

/// R-0009-d: ReadObserver is denied ArtifactUpdate.
#[test]
fn read_observer_denied_artifact_update() {
    // R-0009-d
    let ctx = read_observer_ctx();
    let result = authorize(&ctx, &Verb::ArtifactUpdate);
    assert!(
        result.is_err(),
        "ReadObserver must NOT be authorized for ArtifactUpdate (R-0009-d)"
    );
    let err = result.unwrap_err();
    assert_eq!(err.role, Role::ReadObserver);
    assert_eq!(err.verb, Verb::ArtifactUpdate);
}

/// R-0009-d: ReadObserver is denied ArtifactDelete.
#[test]
fn read_observer_denied_artifact_delete() {
    // R-0009-d
    let ctx = read_observer_ctx();
    let result = authorize(&ctx, &Verb::ArtifactDelete);
    assert!(
        result.is_err(),
        "ReadObserver must NOT be authorized for ArtifactDelete (R-0009-d)"
    );
    let err = result.unwrap_err();
    assert_eq!(err.role, Role::ReadObserver);
    assert_eq!(err.verb, Verb::ArtifactDelete);
}

// ---------------------------------------------------------------------------
// R-0009-d — ReadObserver: CLI control-plane DENIED (each op individually)
// ---------------------------------------------------------------------------

/// R-0009-d: ReadObserver is denied WorkspaceLifecycle CLI op.
///
/// Each of the four CLI control-plane ops is tested individually — R-0009-c
/// enumerates them by name, and a single collapsed check would allow a partial
/// matrix to pass.
#[test]
fn read_observer_denied_cli_workspace_lifecycle() {
    // R-0009-c, R-0009-d
    let ctx = read_observer_ctx();
    let result = authorize(&ctx, &Verb::WorkspaceLifecycle);
    assert!(
        result.is_err(),
        "ReadObserver must NOT be authorized for WorkspaceLifecycle CLI op (R-0009-d)"
    );
    let err = result.unwrap_err();
    assert_eq!(err.role, Role::ReadObserver);
    assert_eq!(err.verb, Verb::WorkspaceLifecycle);
}

/// R-0009-d: ReadObserver is denied TokenRotation CLI op.
#[test]
fn read_observer_denied_cli_token_rotation() {
    // R-0009-c, R-0009-d
    let ctx = read_observer_ctx();
    let result = authorize(&ctx, &Verb::TokenRotation);
    assert!(
        result.is_err(),
        "ReadObserver must NOT be authorized for TokenRotation CLI op (R-0009-d)"
    );
    let err = result.unwrap_err();
    assert_eq!(err.role, Role::ReadObserver);
    assert_eq!(err.verb, Verb::TokenRotation);
}

/// R-0009-d: ReadObserver is denied MigrationTrigger CLI op.
#[test]
fn read_observer_denied_cli_migration_trigger() {
    // R-0009-c, R-0009-d
    let ctx = read_observer_ctx();
    let result = authorize(&ctx, &Verb::MigrationTrigger);
    assert!(
        result.is_err(),
        "ReadObserver must NOT be authorized for MigrationTrigger CLI op (R-0009-d)"
    );
    let err = result.unwrap_err();
    assert_eq!(err.role, Role::ReadObserver);
    assert_eq!(err.verb, Verb::MigrationTrigger);
}

/// R-0009-d: ReadObserver is denied BackupTrigger CLI op.
#[test]
fn read_observer_denied_cli_backup_trigger() {
    // R-0009-c, R-0009-d
    let ctx = read_observer_ctx();
    let result = authorize(&ctx, &Verb::BackupTrigger);
    assert!(
        result.is_err(),
        "ReadObserver must NOT be authorized for BackupTrigger CLI op (R-0009-d)"
    );
    let err = result.unwrap_err();
    assert_eq!(err.role, Role::ReadObserver);
    assert_eq!(err.verb, Verb::BackupTrigger);
}

// ---------------------------------------------------------------------------
// R-0009-e — ReadObserver: workspace lifecycle (create + delete) DENIED
// ---------------------------------------------------------------------------

/// R-0009-e: ReadObserver is denied WorkspaceCreate with structured error.
///
/// R-0009-e specifies workspace lifecycle ops require Admin; ReadObserver
/// returns a structured permission error. Both workspace_create and
/// workspace_delete are tested individually.
#[test]
fn read_observer_denied_workspace_create_structured_error() {
    // R-0009-e
    let ctx = read_observer_ctx();
    let result = authorize(&ctx, &Verb::WorkspaceCreate);
    assert!(
        result.is_err(),
        "ReadObserver must NOT be authorized for WorkspaceCreate (R-0009-e); got Ok(())"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.role,
        Role::ReadObserver,
        "PermissionError.role must be ReadObserver for WorkspaceCreate denial (R-0009-e)"
    );
    assert_eq!(
        err.verb,
        Verb::WorkspaceCreate,
        "PermissionError.verb must be WorkspaceCreate (R-0009-e)"
    );
}

/// R-0009-e: ReadObserver is denied WorkspaceDelete with structured error.
#[test]
fn read_observer_denied_workspace_delete_structured_error() {
    // R-0009-e
    let ctx = read_observer_ctx();
    let result = authorize(&ctx, &Verb::WorkspaceDelete);
    assert!(
        result.is_err(),
        "ReadObserver must NOT be authorized for WorkspaceDelete (R-0009-e); got Ok(())"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.role,
        Role::ReadObserver,
        "PermissionError.role must be ReadObserver for WorkspaceDelete denial (R-0009-e)"
    );
    assert_eq!(
        err.verb,
        Verb::WorkspaceDelete,
        "PermissionError.verb must be WorkspaceDelete (R-0009-e)"
    );
}

// ---------------------------------------------------------------------------
// R-0009-d — ReadObserver: admin session management DENIED
// ---------------------------------------------------------------------------

/// R-0009-c / R-0009-d: ReadObserver is denied AdminSessionList.
#[test]
fn read_observer_denied_admin_session_list() {
    // R-0009-c, R-0009-d
    let ctx = read_observer_ctx();
    let result = authorize(&ctx, &Verb::AdminSessionList);
    assert!(
        result.is_err(),
        "ReadObserver must NOT be authorized for AdminSessionList (R-0009-d)"
    );
    let err = result.unwrap_err();
    assert_eq!(err.role, Role::ReadObserver);
    assert_eq!(err.verb, Verb::AdminSessionList);
}

/// R-0009-c / R-0009-d: ReadObserver is denied AdminSessionRevoke.
#[test]
fn read_observer_denied_admin_session_revoke() {
    // R-0009-c, R-0009-d
    let ctx = read_observer_ctx();
    let result = authorize(&ctx, &Verb::AdminSessionRevoke);
    assert!(
        result.is_err(),
        "ReadObserver must NOT be authorized for AdminSessionRevoke (R-0009-d)"
    );
    let err = result.unwrap_err();
    assert_eq!(err.role, Role::ReadObserver);
    assert_eq!(err.verb, Verb::AdminSessionRevoke);
}

// ---------------------------------------------------------------------------
// R-0015-f — Plugin host-layer pre-dispatch verb check
// ---------------------------------------------------------------------------

/// R-0015-f: ReadObserver is authorized for PluginReadVerb at the host layer.
///
/// `check_plugin_verb` is the `builtins::permissions` hook point called before
/// plugin dispatch. At V0 it delegates to `auth::permissions::authorize`. This
/// test confirms read-path plugin verbs pass for ReadObserver.
#[test]
fn plugin_read_verb_authorized_for_read_observer() {
    // R-0015-f
    let ctx = read_observer_ctx();
    assert!(
        check_plugin_verb(&ctx, &Verb::PluginReadVerb).is_ok(),
        "ReadObserver must be authorized for PluginReadVerb at the host layer (R-0015-f)"
    );
}

/// R-0015-f: ReadObserver is denied PluginWriteVerb at the host layer.
///
/// The host-layer pre-dispatch check must block write-path plugin verbs for
/// ReadObserver before plugin dispatch is invoked. Structured error asserted.
#[test]
fn plugin_write_verb_denied_for_read_observer_structured_error() {
    // R-0015-f
    let ctx = read_observer_ctx();
    let result = check_plugin_verb(&ctx, &Verb::PluginWriteVerb);
    assert!(
        result.is_err(),
        "ReadObserver must NOT be authorized for PluginWriteVerb at the host layer (R-0015-f)"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.role,
        Role::ReadObserver,
        "PermissionError.role must be ReadObserver for PluginWriteVerb denial (R-0015-f)"
    );
    assert_eq!(
        err.verb,
        Verb::PluginWriteVerb,
        "PermissionError.verb must be PluginWriteVerb (R-0015-f)"
    );
}

/// R-0015-f: Admin is authorized for all plugin verbs at the host layer.
#[test]
fn plugin_verbs_authorized_for_admin_at_host_layer() {
    // R-0009-c, R-0015-f
    let ctx = admin_ctx();
    assert!(
        check_plugin_verb(&ctx, &Verb::PluginReadVerb).is_ok(),
        "Admin must be authorized for PluginReadVerb at the host layer (R-0015-f)"
    );
    assert!(
        check_plugin_verb(&ctx, &Verb::PluginWriteVerb).is_ok(),
        "Admin must be authorized for PluginWriteVerb at the host layer (R-0015-f)"
    );
}

// ---------------------------------------------------------------------------
// Anti-vacuous-stub matrix proof (structural invariant tests)
// ---------------------------------------------------------------------------

/// Structural guard: PermissionError carries both role and verb fields.
///
/// This test proves that `PermissionError` cannot be satisfied by a unit struct
/// or a structurally vacuous type. If Task 14 GREEN defines `PermissionError`
/// without public `role` and `verb` fields, the field-access expressions above
/// fail to compile — which is itself a red signal at that point.
///
/// Additionally: the deny tests above assert `err.role == Role::ReadObserver`
/// AND `err.verb == <the specific verb>`. A single-variant error enum cannot
/// satisfy both assertions across different verbs. A constant-Err returning
/// `PermissionError { role: ReadObserver, verb: ArtifactCreate }` would pass
/// `artifact_create` but fail `workspace_delete`. The only stub that satisfies
/// all deny tests without encoding the real matrix is the real implementation.
///
/// This test is a documentation anchor; the behavioral guards above are the
/// load-bearing assertions.
#[test]
fn permission_error_is_role_and_verb_structured() {
    // Structural invariant — anti-vacuous
    let ctx = read_observer_ctx();
    let err: PermissionError = authorize(&ctx, &Verb::ArtifactCreate)
        .expect_err("authorize(ReadObserver, ArtifactCreate) must be Err for this test to run");

    // Assert the type has the expected fields by using them:
    let _role: &Role = &err.role;
    let _verb: &Verb = &err.verb;

    // Assert Display is implemented (not just Debug):
    let display = format!("{err}");
    assert!(
        !display.is_empty(),
        "PermissionError must implement Display (used in user-facing error messages)"
    );
}
