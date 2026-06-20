//! MCP dispatch internals: DF-auth-check, WorkspaceCtx construction, and
//! per-verb capability check (R-0010-c/d, R-0009-e, R-0006-b).
//!
//! # DF-auth-check (R-0010-c)
//!
//! The decision-first auth check runs on EVERY `tools/call` request, before
//! any verb routing. It is NOT applied to `initialize` or `tools/list` — that
//! narrowing is deliberate and locked per the dispatch envelope.
//!
//! Flow for `tools/call`:
//!   1. Extract `token_str` from `params.meta["token"]`.
//!   2. `token::hash_presented(token_str)` — None → AUTH_FAILURE.
//!   3. `token::lookup_by_hash(hash, pool)` — None → AUTH_FAILURE.
//!   4. `auth::resolve::from_token(workspace_id, &scopes, token_id)` → WorkspaceCtx.
//!   5. Classify the verb as `PluginReadVerb` or `PluginWriteVerb`.
//!   6. `builtins::permissions::check_plugin_verb(ctx, verb)` → Ok or PERMISSION_DENIED.
//!
//! # Security note (R-0004-h)
//!
//! The `token_str` MUST NOT appear in any error message, log, metric, or event.
//! This module never formats the token string; it passes it only to
//! `token::hash_presented` which discards the string after hashing.

use rmcp::model::ErrorData;
use sqlx::PgPool;

use crate::auth::permissions::Verb;
use crate::auth::resolve;
use crate::auth::token;
use crate::auth::workspace_ctx::WorkspaceCtx;
use crate::builtins::permissions;
use crate::mcp::errors::{AUTH_FAILURE_CODE, PERMISSION_DENIED_CODE};

// ---------------------------------------------------------------------------
// Verb classification — fail-closed (R-0009-d, SF1, SF2)
// ---------------------------------------------------------------------------

/// Returns `true` if `verb_name` requires write capability (ReadObserver is
/// denied), `false` only for the explicit read allowlist `{get, list}`.
///
/// Fail-closed: every tail NOT in the read allowlist is treated as a write
/// verb. An unclassified or unknown tail therefore DENIES ReadObserver access
/// rather than granting it. This is SF2 (enumerate the permitted set, deny the
/// rest) from `skills/rust.md` `<control-code>`.
///
/// R-0009-d: ReadObserver authorises only read-path MCP verbs
/// (`artifact.get`, `artifact.list`); write verbs and any unrecognised tail
/// are denied at the host-fn boundary.
///
/// The convention is `"<plugin>.<verb>"` where the tail after the last `'.'`
/// is the action name.
fn is_write_verb(verb_name: &str) -> bool {
    let tail = verb_name.rsplit('.').next().unwrap_or(verb_name);
    !matches!(tail, "get" | "list")
}

// ---------------------------------------------------------------------------
// DF-auth-check entry point
// ---------------------------------------------------------------------------

/// Run the full DF-auth-check for a `tools/call` request.
///
/// Returns `Ok(WorkspaceCtx)` if the token resolves and the role is authorized
/// for `verb_name`. Returns `Err(ErrorData)` with the appropriate custom code
/// on any failure.
///
/// The caller is responsible for routing to the plugin after this returns Ok.
///
/// # Security (R-0004-h)
///
/// `token_str` is consumed internally; it MUST NOT be placed in any returned
/// error message, log record, or metric label.
pub async fn auth_and_authorize(
    token_str: &str,
    verb_name: &str,
    pool: &PgPool,
) -> Result<WorkspaceCtx, ErrorData> {
    // Step 1 + 2: hash the presented string. Invalid base64url / wrong length = auth fail.
    let token_hash = token::hash_presented(token_str).ok_or_else(|| ErrorData {
        code: AUTH_FAILURE_CODE,
        message: "authentication failed".into(),
        data: None,
    })?;

    // Step 3: look up the hash in admin_tokens.
    let row = token::lookup_by_hash(&token_hash, pool)
        .await
        .map_err(|_| ErrorData {
            code: AUTH_FAILURE_CODE,
            message: "authentication failed".into(),
            data: None,
        })?
        .ok_or_else(|| ErrorData {
            code: AUTH_FAILURE_CODE,
            message: "authentication failed".into(),
            data: None,
        })?;

    // Step 4: construct WorkspaceCtx at the single production site (R-0006-b).
    let ctx = resolve::from_token(row.workspace_id, &row.scopes, row.id);

    // Step 5: classify verb as read or write for the permission check.
    let verb = if is_write_verb(verb_name) {
        Verb::PluginWriteVerb
    } else {
        Verb::PluginReadVerb
    };

    // Step 6: capability check via builtins.
    permissions::check_plugin_verb(&ctx, &verb).map_err(|_| ErrorData {
        code: PERMISSION_DENIED_CODE,
        message: "permission denied".into(),
        data: None,
    })?;

    Ok(ctx)
}
