//! Token-to-context resolution: the single production `WorkspaceCtx`
//! construction site (R-0006-b).
//!
//! # Contract
//!
//! `from_token` is the ONLY place in production code where `WorkspaceCtx::new`
//! is called. The dispatch path calls it after looking up a raw token string
//! against `admin_tokens`. No other production site constructs a `WorkspaceCtx`.
//!
//! At V0, full MCP verb dispatch (Task 23) is not yet wired, so this function
//! is the reserved construction site. It is called with the token row data
//! returned by `auth::token::lookup_by_hash`.

use uuid::Uuid;

use crate::auth::role::Role;
use crate::auth::workspace_ctx::WorkspaceCtx;

/// Construct a `WorkspaceCtx` from a resolved token row.
///
/// This is the single production construction site for `WorkspaceCtx` (R-0006-b).
/// It is called after `auth::token::lookup_by_hash` succeeds, taking the row's
/// `workspace_id`, `scopes`, and `id` (token_id) as inputs.
///
/// Role is derived from `scopes` using `Role::from_scopes` (R-0009-f).
///
/// # Parameters
///
/// - `workspace_id` — the workspace this token is scoped to.
/// - `scopes` — `admin_tokens.scopes` column value; used to derive the `Role`.
/// - `token_id` — the token row's `id` field; for audit / rotation linking.
pub fn from_token(workspace_id: Uuid, scopes: &[String], token_id: Uuid) -> WorkspaceCtx {
    let role = Role::from_scopes(scopes);
    WorkspaceCtx::new(workspace_id, role, token_id)
}
