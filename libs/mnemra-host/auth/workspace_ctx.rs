//! `WorkspaceCtx` — tenant workspace context threaded through every host-fn.
//!
//! # Invariants (R-0006)
//!
//! - **Single construction site (R-0006-b):** `WorkspaceCtx::new` is the sole
//!   production constructor. It is called once, after token validation, at the
//!   auth resolution seam (`crate::auth::resolve::from_token`). There is no
//!   other non-test path that constructs a `WorkspaceCtx`.
//!
//! - **Private field (R-0006-c):** `workspace_id` is a private field. External
//!   code must use the `workspace_id()` accessor; direct field access is not
//!   possible outside this module.
//!
//! - **Test-only constructor (R-0006-f):** `for_test` is gated with
//!   `#[cfg(test)]`. The compiler excludes it from production builds, making it
//!   impossible to accidentally call in production code.
//!
//! # Fields (R-0009-b)
//!
//! - `workspace_id: Uuid` — private; access via `workspace_id()` accessor.
//! - `pub role: Role` — tenant role derived from token scopes at construction.
//! - `pub token_id: Uuid` — the token row's id; for audit / rotation linking.

use uuid::Uuid;

use crate::auth::role::Role;

/// Tenant workspace context — carries workspace identity and role.
///
/// Constructed at a single production site after token validation
/// (`crate::auth::resolve::from_token`). Passed as the first parameter to
/// every host-fn; the compiler enforces this convention (no host-fn can issue
/// a query without receiving a `WorkspaceCtx`).
///
/// # Field visibility
///
/// `workspace_id` is private. Access it via the `workspace_id()` accessor.
/// This prevents accidental bypass of the accessor and makes the constraint
/// structurally enforced rather than relying on convention.
#[derive(Debug, Clone)]
pub struct WorkspaceCtx {
    /// Private: the workspace UUID. Use `workspace_id()` to read it.
    workspace_id: Uuid,
    /// The tenant's role, derived from `admin_tokens.scopes` at construction.
    pub role: Role,
    /// The token row's id — used for audit and rotation linking.
    pub token_id: Uuid,
}

impl WorkspaceCtx {
    /// Production constructor — the single allowed non-test construction site.
    ///
    /// Called at `crate::auth::resolve::from_token` after token lookup and role
    /// derivation. No other production code constructs a `WorkspaceCtx`.
    pub fn new(workspace_id: Uuid, role: Role, token_id: Uuid) -> Self {
        Self {
            workspace_id,
            role,
            token_id,
        }
    }

    /// Public accessor for the private `workspace_id` field (R-0006-c).
    ///
    /// Returns the workspace UUID. This is the only way to read `workspace_id`
    /// from outside this module.
    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }

    /// Test-only constructor — `#[cfg(test)]`-gated (R-0006-f).
    ///
    /// Allows test harnesses to construct a `WorkspaceCtx` with known values.
    /// This function does NOT exist in production builds; the `#[cfg(test)]`
    /// attribute causes the compiler to exclude it from non-test compilation.
    #[cfg(test)]
    pub fn for_test(workspace_id: Uuid, role: Role, token_id: Uuid) -> Self {
        Self {
            workspace_id,
            role,
            token_id,
        }
    }
}
