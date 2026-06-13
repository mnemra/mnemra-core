//! Permissions builtin: host-layer plugin verb authorization (R-0015-f).
//!
//! # V0 scope
//!
//! The `permissions` builtin provides a named hook point for plugin verb
//! authorization at the host layer, checked BEFORE plugin dispatch is invoked.
//! At V0 it delegates directly to `auth::permissions::authorize`.
//!
//! The wrapper exists so the plugin dispatch path has a named extension point
//! that future tasks can extend (e.g., per-plugin grant overrides, audit
//! logging) without modifying `authorize` or the permission matrix directly.
//!
//! # Spec requirements traced
//!
//! - R-0015-f: Permission checks for plugin verb access run at the host layer
//!   before plugin dispatch.

use crate::auth::permissions::{PermissionError, Verb, authorize};
use crate::auth::workspace_ctx::WorkspaceCtx;

/// Pre-dispatch host-layer plugin verb authorization (R-0015-f).
///
/// Called by the host dispatch path BEFORE invoking plugin-provided handlers.
/// Returns `Ok(())` if the role in `ctx` is authorized for `verb`, or
/// `Err(PermissionError)` if denied.
///
/// # V0 delegation
///
/// At V0 this delegates to `auth::permissions::authorize`. The builtin wrapper
/// is the stable hook point; future tasks that add per-plugin grant overrides
/// extend here, not inside `authorize`.
pub fn check_plugin_verb(ctx: &WorkspaceCtx, verb: &Verb) -> Result<(), PermissionError> {
    authorize(ctx, verb)
}
