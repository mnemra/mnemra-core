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

use crate::auth::permissions::Verb;
use crate::auth::resolve;
use crate::auth::token;
use crate::auth::workspace_ctx::WorkspaceCtx;
use crate::builtins::permissions;
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
///   - `*.update`/`*.delete` -> NON_DISPATCHABLE until their per-verb CC-MAPPING
///     is pinned (a later T12 slice).
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
            let type_name = arguments
                .and_then(|m| m.get("content_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("echo_fixture")
                .to_owned();
            let filters = arguments
                .and_then(|m| m.get("filters"))
                .map(json_value_to_payload_string)
                .unwrap_or_else(|| "{}".to_owned());
            ContentCall::List { type_name, filters }
        }
        // update/delete have typed `content` exports (slice-1 guest stubs) but
        // their MCP-verb -> method argument shapes are pinned per-verb in a later
        // T12 slice, not here. No test dispatches them through `call_tool` yet;
        // surfacing the R-0019-d non-dispatchable error is the honest behavior
        // (their CC-MAPPING is not yet pinned), and it leaves the pre-dispatch
        // permission outcome unchanged (R-0019-e).
        "update" | "delete" => {
            return Err(ErrorData {
                code: NON_DISPATCHABLE_CODE,
                message: format!("verb '{verb_name}' is not wired at slice 1 (CC-MAPPING T12)")
                    .into(),
                data: None,
            });
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
