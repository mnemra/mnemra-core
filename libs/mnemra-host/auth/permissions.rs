//! Role-based permission matrix at the host-fn boundary (R-0009-c/d/e/g).
//!
//! # Permission matrix
//!
//! The matrix is an **allowlist**: `Admin` may do everything; `ReadObserver`
//! is allowed only the explicitly listed read-path verbs. Every other verb
//! denies with a structured `PermissionError`. Deny is the default; allow is
//! explicit.
//!
//! | Category               | Admin  | ReadObserver |
//! |------------------------|--------|--------------|
//! | ReadPathMcp            | Allow  | Allow        |
//! | WritePathMcp           | Allow  | Deny         |
//! | CliControlPlane        | Allow  | Deny         |
//! | WorkspaceLifecycle     | Allow  | Deny         |
//! | AdminSessionMgmt       | Allow  | Deny         |
//! | PluginVerb (read_path) | Allow  | Allow        |
//! | PluginVerb (write_path)| Allow  | Deny         |
//! | CoordinationWriteVerb  | Allow  | Deny         |
//!
//! # Application-layer enforcement (R-0009-g)
//!
//! Enforcement is at the application layer only. No Postgres RLS policies are
//! activated at V0; no `CREATE POLICY` statements are issued.
//!
//! # Spec requirements traced
//!
//! - R-0009-c: Admin authorizes all MCP verb categories + CLI control-plane +
//!   admin session management.
//! - R-0009-d: ReadObserver authorizes only read-path MCP verbs; write verbs,
//!   CLI control-plane, workspace lifecycle denied at host-fn boundary.
//! - R-0009-e: Workspace lifecycle requires Admin; ReadObserver → structured error.
//! - R-0009-g: No Postgres RLS at V0; application-layer only.
//! - R-0015-f: Permission checks for plugin verb access run at the host layer
//!   before plugin dispatch (see `builtins::permissions`).

use crate::auth::role::Role;
use crate::auth::workspace_ctx::WorkspaceCtx;

// ---------------------------------------------------------------------------
// Verb taxonomy
// ---------------------------------------------------------------------------

/// Verb taxonomy for the permission matrix (R-0009-c/d/e, R-0015-f).
///
/// Every host-fn call is classified as one of these verbs before being
/// dispatched. `authorize` checks the verb against the role in `WorkspaceCtx`
/// and either returns `Ok(())` or a structured `PermissionError`.
///
/// Derive notes:
/// - `Debug`: required for error formatting and test assertions.
/// - `Clone` / `Copy`: verbs are small value types; Copy avoids borrow friction
///   when the verb is passed to `authorize` and also stored in `PermissionError`.
/// - `PartialEq` / `Eq`: required for test equality assertions on `err.verb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    // --- Read-path MCP verbs (R-0009-d allow list) ---
    /// artifact.get (R-0009-d)
    ArtifactGet,
    /// artifact.list (R-0009-d)
    ArtifactList,
    /// projection.query (R-0009-d)
    ProjectionQuery,

    // --- Write-path MCP verbs (R-0009-d deny for ReadObserver) ---
    /// artifact.create (R-0009-d)
    ArtifactCreate,
    /// artifact.update (R-0009-d)
    ArtifactUpdate,
    /// artifact.delete (R-0009-d)
    ArtifactDelete,

    // --- CLI control-plane ops (R-0009-c; R-0009-d deny for ReadObserver) ---
    /// workspace lifecycle via CLI (R-0009-c, R-0009-e)
    WorkspaceLifecycle,
    /// token rotation via CLI (R-0009-c, R-0009-d)
    TokenRotation,
    /// migration trigger via CLI (R-0009-c, R-0009-d)
    MigrationTrigger,
    /// backup trigger via CLI (R-0009-c, R-0009-d)
    BackupTrigger,

    // --- Workspace lifecycle (R-0009-e — named separately for precision) ---
    /// workspace create (R-0009-e)
    WorkspaceCreate,
    /// workspace delete (R-0009-e)
    WorkspaceDelete,

    // --- Admin session management (R-0009-c) ---
    /// admin session list (R-0009-c)
    AdminSessionList,
    /// admin session revoke (R-0009-c)
    AdminSessionRevoke,

    // --- Plugin host-layer verb (R-0015-f) ---
    /// plugin verb at host layer before dispatch; read = allow ReadObserver
    PluginReadVerb,
    /// plugin verb at host layer before dispatch; write = deny ReadObserver
    PluginWriteVerb,

    // --- Coordination host-served verb (R-0073-b) ---
    /// A coordination action (`message`/`claim`: attach/poll, acquire, renew,
    /// release, takeover, send, ack, disposition). Write-category — denied to
    /// `ReadObserver` at the host-fn boundary. Every coordination action —
    /// `list` included — is write-category (each executes under a resolved
    /// attachment, itself a write), so a `read_observer` token cannot
    /// participate in the coordination surface at all (R-0073-b). Not in the
    /// `ReadObserver` allow arm below → auto-denied by the allowlist default.
    CoordinationWriteVerb,
}

// ---------------------------------------------------------------------------
// PermissionError
// ---------------------------------------------------------------------------

/// Structured permission denial: role + attempted verb (R-0009-d, R-0009-e).
///
/// Both fields are public so tests (and callers) can assert on the structured
/// error contents rather than just `.is_err()`. This blocks a vacuous
/// `Err(PermissionError { .. })` stub from satisfying the suite.
///
/// `PermissionError` implements `std::error::Error` so it can be used with
/// `?` and wrapped in host-fn error chains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionError {
    /// The role that was denied.
    pub role: Role,
    /// The verb that was attempted.
    pub verb: Verb,
}

impl std::fmt::Display for PermissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "permission denied: role {:?} is not authorized for verb {:?}",
            self.role, self.verb
        )
    }
}

impl std::error::Error for PermissionError {}

// ---------------------------------------------------------------------------
// authorize — the allowlist gate
// ---------------------------------------------------------------------------

/// Check whether `ctx.role` is authorized to execute `verb`.
///
/// # Permission matrix
///
/// - `Admin`: all verbs authorized — returns `Ok(())`.
/// - `ReadObserver`: only the four read-path verbs are allowed
///   (`ArtifactGet`, `ArtifactList`, `ProjectionQuery`, `PluginReadVerb`).
///   Every other verb returns `Err(PermissionError { role, verb })`.
///
/// The matrix is an allowlist: deny is the default, allow is explicit.
/// Adding a new verb to `Verb` that is NOT added to the `ReadObserver` allow
/// arm below will automatically deny for `ReadObserver` — no "else allow"
/// branch exists.
///
/// # Application-layer enforcement (R-0009-g)
///
/// This function enforces at the application layer only. No Postgres RLS
/// policies are activated at V0.
pub fn authorize(ctx: &WorkspaceCtx, verb: &Verb) -> Result<(), PermissionError> {
    match ctx.role {
        Role::Admin => Ok(()),
        Role::ReadObserver => {
            // Explicit allowlist — everything not listed here denies.
            match verb {
                Verb::ArtifactGet
                | Verb::ArtifactList
                | Verb::ProjectionQuery
                | Verb::PluginReadVerb => Ok(()),
                _ => Err(PermissionError {
                    role: ctx.role.clone(),
                    verb: *verb,
                }),
            }
        }
    }
}
