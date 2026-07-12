//! MCP dispatch internals: DF-auth-check, WorkspaceCtx construction, and
//! role-based permission check (R-0010-c, R-0009-e, R-0006-b).
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
//!   6. `builtins::permissions::check_plugin_verb(ctx, verb)` → Ok or PERMISSION_DENIED (R-0009).
//!
//! NOTE: The per-verb capability check (R-0010-d, manifest `verbs` membership
//! gate) lives in `server.rs::call_tool`, after `auth_and_authorize` returns
//! and before `resolve_content_call`. This module handles only auth + role.
//!
//! # Security note (R-0004-h)
//!
//! The `token_str` MUST NOT appear in any error message, log, metric, or event.
//! This module never formats the token string; it passes it only to
//! `token::hash_presented` which discards the string after hashing.

use rmcp::model::ErrorData;
use sqlx::PgPool;

use crate::auth::permissions::{Verb, authorize};
use crate::auth::resolve;
use crate::auth::token;
use crate::auth::workspace_ctx::WorkspaceCtx;
use crate::builtins::permissions;
use crate::coordination::session_plane::CoordinationAction;
use crate::mcp::errors::{AUTH_FAILURE_CODE, NON_DISPATCHABLE_CODE, PERMISSION_DENIED_CODE};
use crate::plugin::trap_recovery::ContentCall;

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

/// Run the DF-auth-check and role permission check for a `tools/call` request.
///
/// Covers: token lookup (R-0010-c) + role-based permission check (R-0009-d/e).
/// Does NOT cover: manifest-verbs membership gate (R-0010-d) — that check runs
/// in `server.rs::call_tool` after this function returns Ok, before dispatch.
///
/// Returns `Ok(WorkspaceCtx)` if the token resolves and the role is authorized
/// for `verb_name`. Returns `Err(ErrorData)` with the appropriate custom code
/// on any failure.
///
/// The caller is responsible for the manifest-verbs gate and plugin routing
/// after this returns Ok.
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
    // Resolve the token to a WorkspaceCtx (hash → lookup → the single
    // WorkspaceCtx construction site, R-0006-b).
    let ctx = resolve_ctx_from_token(token_str, pool).await?;

    // Classify the verb as read or write for the permission check.
    let verb = if is_write_verb(verb_name) {
        Verb::PluginWriteVerb
    } else {
        Verb::PluginReadVerb
    };

    // Capability check via the host-layer permission hook.
    permissions::check_plugin_verb(&ctx, &verb).map_err(|_| ErrorData {
        code: PERMISSION_DENIED_CODE,
        message: "permission denied".into(),
        data: None,
    })?;

    Ok(ctx)
}

/// Resolve a presented token string to a [`WorkspaceCtx`] — the auth half
/// shared by the plugin dispatch path ([`auth_and_authorize`]) and the
/// host-served coordination path ([`authorize_coordination_action`]): token
/// hash → `admin_tokens` lookup → the single `WorkspaceCtx` construction site.
///
/// This is the sole caller of [`resolve::from_token`], keeping the single
/// production `WorkspaceCtx` construction-site invariant (R-0006-b). It runs no
/// capability check; the caller applies the per-surface authorization.
///
/// # Security (R-0004-h)
///
/// `token_str` is consumed only via `token::hash_presented` (which discards the
/// string after hashing); it MUST NOT appear in any returned error message.
pub(crate) async fn resolve_ctx_from_token(
    token_str: &str,
    pool: &PgPool,
) -> Result<WorkspaceCtx, ErrorData> {
    // Hash the presented string. Invalid base64url / wrong length = auth fail.
    let token_hash = token::hash_presented(token_str).ok_or_else(|| ErrorData {
        code: AUTH_FAILURE_CODE,
        message: "authentication failed".into(),
        data: None,
    })?;

    // Look up the hash in admin_tokens.
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

    // Construct WorkspaceCtx at the single production site (R-0006-b).
    Ok(resolve::from_token(row.workspace_id, &row.scopes, row.id))
}

/// The per-action coordination capability gate (R-0073-b, R-0063-c).
///
/// Runs AFTER [`resolve_ctx_from_token`] and BEFORE any coordination routing.
/// It maps the parsed `action` to its verb category and applies the permission
/// matrix. Every coordination action is write-category at V0 (R-0073-b: `list`
/// included — each executes under a resolved attachment, itself a write), so a
/// `read_observer` token is refused `permission denied` **pre-dispatch** and
/// cannot participate in the coordination surface at all.
///
/// This is the action-reading gate the host-served branch requires: it
/// classifies the `action` argument, NOT the `message`/`claim` tool-name tail.
/// (`is_write_verb` classifies on the tail and would mis-read a future `list`
/// action as read; the tool-name tail `"message"` classifying as write today is
/// a coincidence, not the mechanism.) Task 5 extends the match with `claim`'s
/// actions — every arm maps to `CoordinationWriteVerb`.
pub(crate) fn authorize_coordination_action(
    ctx: &WorkspaceCtx,
    action: &CoordinationAction,
) -> Result<(), ErrorData> {
    let verb = match action {
        CoordinationAction::Poll => Verb::CoordinationWriteVerb,
    };

    authorize(ctx, &verb).map_err(|_| ErrorData {
        code: PERMISSION_DENIED_CODE,
        message: "permission denied".into(),
        data: None,
    })
}

// ---------------------------------------------------------------------------
// CC-MAPPING — static verb -> typed `content` export resolution (R-0019-a/c/d)
// ---------------------------------------------------------------------------

/// The resolved dispatch for an authenticated verb: which typed `content` call
/// to make. Verb -> export resolution is STATIC against the fixed `content`
/// interface (FENCE R-0019-c: no runtime export registry).
pub struct ContentDispatch {
    /// The typed `content` call + its marshalled arguments.
    pub call: ContentCall,
}

/// Resolve an authenticated MCP verb to its typed `content` call, reading the
/// MCP `arguments` map (CC-MAPPING, R-0019-a/c).
///
/// Slice-1 / V0 mapping (the `<plugin>.<verb>` tail selects the method):
///   - `*.create` -> `content.create`, `{content_type -> type, payload -> frontmatter}`
///   - `*.get`    -> `content.get`,    `{id -> id}`  (R1: `{id}` argument key)
///   - `*.list`   -> `content.list`,   `{content_type -> type, filters -> filters}`
///     (T12 list slice; `filters` defaults to `"{}"`, parsed-not-applied)
///   - `*.update` -> `content.update`, `{id -> id, frontmatter_patch -> frontmatter-patch,
///     body -> body}` (T12 update slice; all three read via `.as_str()`,
///     `frontmatter_patch` defaults to `"{}"`, `body` is `Some` iff the key is present)
///   - `*.delete` -> `content.delete`, `{id -> id}` (T12 delete slice).
///
/// A verb whose tail has no matching typed `content` method (e.g. a future
/// `*.audit`) returns the R-0019-d structured non-dispatchable error. This runs
/// AFTER the pre-dispatch permission check, so the permission outcome is
/// unchanged (R-0019-e).
///
/// Argument typing: the MCP `payload` argument is mapped to the `frontmatter`
/// JSON-as-string. A JSON string payload is carried through as its raw inner
/// string (so it round-trips exactly on `get`); any non-string JSON value is
/// serialized to its JSON text. The `content_type` argument defaults to
/// `"echo_fixture"` when absent (the V0 fixture content type).
pub fn resolve_content_call(
    verb_name: &str,
    arguments: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Result<ContentDispatch, ErrorData> {
    let tail = verb_name.rsplit('.').next().unwrap_or(verb_name);

    let call = match tail {
        "create" => {
            let type_name = arguments
                .and_then(|m| m.get("content_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("echo_fixture")
                .to_owned();
            let frontmatter = arguments
                .and_then(|m| m.get("payload"))
                .map(json_value_to_payload_string)
                .unwrap_or_default();
            ContentCall::Create {
                type_name,
                frontmatter,
                body: None,
            }
        }
        "get" => {
            // R1: the `echo.get` argument key is `{id}` (CC-MAPPING slice-1 extension).
            let id = arguments
                .and_then(|m| m.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned();
            ContentCall::Get { id }
        }
        "list" => {
            // T12 list slice: `content_type` -> WIT `type` (default `"echo_fixture"`,
            // exactly like create) and `filters` -> WIT `filters` (a JSON string,
            // default `"{}"`). The host-fn body scopes by workspace + type only;
            // `filters` is threaded but not applied this slice (brain #1846).
            // T14 (R-0020): `limit` and `cursor` forwarded for paging; T14 host
            // returns placeholder paging only — no keyset/clamp/cursor logic here.
            let type_name = arguments
                .and_then(|m| m.get("content_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("echo_fixture")
                .to_owned();
            let filters = arguments
                .and_then(|m| m.get("filters"))
                .map(json_value_to_payload_string)
                .unwrap_or_else(|| "{}".to_owned());
            let limit = arguments
                .and_then(|m| m.get("limit"))
                .and_then(|v| v.as_u64())
                .map(|n| n as u32)
                .unwrap_or(0);
            let cursor = arguments
                .and_then(|m| m.get("cursor"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned());
            ContentCall::List {
                type_name,
                filters,
                limit,
                cursor,
            }
        }
        "update" => {
            // T12 update slice: `id` -> WIT id; `frontmatter_patch` -> WIT
            // frontmatter-patch (a JSON-text string, default `"{}"`); `body` ->
            // WIT body=option<string>. All three are read via `.as_str()` (the MCP
            // layer passes them as JSON *string* values, NOT objects — so they are
            // NOT routed through `json_value_to_payload_string` the way create's
            // object `payload` is). `body` is `Some` when the key is PRESENT and
            // `None` when the key is ABSENT — that absence is how the MCP layer
            // signals the WIT `body=None` (leave the existing body unchanged).
            let id = arguments
                .and_then(|m| m.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned();
            let frontmatter_patch = arguments
                .and_then(|m| m.get("frontmatter_patch"))
                .and_then(|v| v.as_str())
                .unwrap_or("{}")
                .to_owned();
            let body = arguments
                .and_then(|m| m.get("body"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned());
            ContentCall::Update {
                id,
                frontmatter_patch,
                body,
            }
        }
        // `delete` -> `content.delete`, `{id -> id}` (T12 delete slice).
        // Argument shape mirrors `get`: `id` is the only argument, read via
        // `.as_str()` (the MCP layer passes it as a JSON string value).
        "delete" => {
            let id = arguments
                .and_then(|m| m.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned();
            ContentCall::Delete { id }
        }
        _ => {
            // No matching typed `content` method (e.g. `*.audit`) — R-0019-d.
            return Err(ErrorData {
                code: NON_DISPATCHABLE_CODE,
                message: format!("verb '{verb_name}' has no matching typed content export").into(),
                data: None,
            });
        }
    };

    Ok(ContentDispatch { call })
}

/// Map an MCP `payload` JSON value to the `frontmatter` JSON-as-string.
///
/// A JSON string is carried through as its raw inner string so the created
/// payload round-trips exactly on `get`; any other JSON value is serialized to
/// its compact JSON text.
fn json_value_to_payload_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests: the per-action coordination capability gate (R-0073-b).
//
// `authorize_coordination_action` is a pure function over `(WorkspaceCtx,
// CoordinationAction)`, so the token-role gate is unit-testable directly — no
// DB, no transport. This is the b0-owned coverage for the R-0073-b control the
// host-served coordination branch lands (poll refused pre-dispatch for a
// read_observer; admitted for admin). Attach/succession/poll-delivery behavior
// is NOT tested here — it is authored Glitch-red-first in a later sub-run.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::role::Role;
    use crate::auth::workspace_ctx::WorkspaceCtx;
    use uuid::Uuid;

    #[test]
    fn coordination_poll_refused_for_read_observer_predispatch() {
        // R-0073-b: `poll` is write-category; a `read_observer` token is refused
        // at the token-role gate before any attach/actor resolution.
        let ctx = WorkspaceCtx::new(Uuid::new_v4(), Role::ReadObserver, Uuid::new_v4());
        let err = authorize_coordination_action(&ctx, &CoordinationAction::Poll)
            .expect_err("R-0073-b: a read_observer `poll` must be refused pre-dispatch");
        assert_eq!(
            err.code, PERMISSION_DENIED_CODE,
            "the refusal must carry the permission-denied code — distinct from \
             an auth failure (valid token, wrong role)"
        );
    }

    #[test]
    fn coordination_poll_admitted_for_admin() {
        // The gate is a deny-for-read_observer control, not a deny-all: an Admin
        // token passes it (the attach/actor logic runs downstream in a later
        // sub-run). Guards against the gate degrading into a blanket refusal.
        let ctx = WorkspaceCtx::new(Uuid::new_v4(), Role::Admin, Uuid::new_v4());
        authorize_coordination_action(&ctx, &CoordinationAction::Poll)
            .expect("an Admin token must pass the coordination capability gate");
    }
}
