//! Role enum: binary tenant-role type (R-0009-a).
//!
//! The role is derived from `admin_tokens.scopes` at `WorkspaceCtx`
//! construction time (R-0009-f). Only two roles exist at V0:
//!
//! - `Admin` — full access; derived from scope `"admin"`.
//! - `ReadObserver` — read-only access; derived from scope `"read_observer"`.
//!
//! No other roles are defined (R-0009-a). The permission matrix is enforced
//! at application layer; Postgres RLS policies are NOT activated at V0
//! (R-0009-g).

/// Binary tenant role (R-0009-a).
///
/// Derived from `admin_tokens.scopes` at `WorkspaceCtx` construction:
/// - scope `"admin"` → `Role::Admin`
/// - scope `"read_observer"` → `Role::ReadObserver`
/// - any other / empty → `Role::ReadObserver` (safe default)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// Full-access role; derived from scope `"admin"` (R-0009-f).
    Admin,
    /// Read-only observer role; derived from scope `"read_observer"` (R-0009-f).
    ReadObserver,
}

impl Role {
    /// Derive a `Role` from a slice of scope strings (R-0009-f).
    ///
    /// If the scopes contain `"admin"`, returns `Role::Admin`.
    /// Otherwise returns `Role::ReadObserver` (safe fallback).
    ///
    /// This is called at `WorkspaceCtx` construction after token lookup.
    pub fn from_scopes(scopes: &[String]) -> Self {
        if scopes.iter().any(|s| s == "admin") {
            Role::Admin
        } else {
            Role::ReadObserver
        }
    }
}
