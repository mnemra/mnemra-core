//! MCP error codes for auth and permission failures (R-0010-f).
//!
//! Defines custom `ErrorCode` values that do NOT overlap with the standard
//! JSON-RPC error codes (-32600 through -32603). The test contract
//! (`mcp_server.rs`) explicitly asserts these codes are distinct.
//!
//! # Code allocation
//!
//! Custom mnemra MCP error codes occupy the range -4000 to -4099.
//!   -4001 : AUTH_FAILURE_CODE         — bad/missing token; no WorkspaceCtx constructed
//!   -4002 : PERMISSION_DENIED_CODE    — valid token, wrong role for the verb (R-0009)
//!   -4003 : NON_DISPATCHABLE_CODE     — manifest verb with no typed export (R-0019-d)
//!   -4004 : PLUGIN_EXEC_CODE          — plugin execution error (trap / not-registered)
//!   -4005 : VERB_NOT_EXPOSED_CODE     — verb not in manifest verbs list (R-0010-d)
//!   -4006 : SUPERVISOR_DEGRADED_CODE  — epoch-tick supervisor degraded; dispatch unsafe (R-0007-h)

use rmcp::model::ErrorCode;

/// Auth failure: the presented token was not found in `admin_tokens`.
///
/// Returned from `call_tool` when the BLAKE3 hash of the presented token
/// string does not match any row in the DB. No `WorkspaceCtx` is constructed.
///
/// # R-0010-f
///
/// Code is custom (-4001) and does NOT overlap with standard JSON-RPC codes
/// (-32600 INVALID_REQUEST, -32601 METHOD_NOT_FOUND, -32602 INVALID_PARAMS,
/// -32603 INTERNAL_ERROR). The test asserts `!=` for each standard code.
pub const AUTH_FAILURE_CODE: ErrorCode = ErrorCode(-4001);

/// Permission denied: valid token, but the role is not authorized for the verb.
///
/// Returned from `call_tool` when auth passes (token resolves to a DB row)
/// but `auth::permissions::authorize` denies the role+verb combination.
///
/// # R-0009-e/f
///
/// This is the role-based permission gate (R-0009), NOT the manifest-verbs
/// membership gate (R-0010-d). R-0010-d is `VERB_NOT_EXPOSED_CODE` (-4005).
/// Code is custom (-4002) and is distinct from `AUTH_FAILURE_CODE` (-4001)
/// so callers can distinguish "bad token" from "good token, wrong role".
pub const PERMISSION_DENIED_CODE: ErrorCode = ErrorCode(-4002);

/// Non-dispatchable: a manifest-declared verb with no matching typed `content`
/// export (R-0019-d). Returned AFTER the pre-dispatch permission check passes,
/// so the permission outcome is unchanged (R-0019-e). At V0 no shipped verb
/// reaches this — the `content` CRUD set covers `echo.{create,get,list,update,
/// delete}`; a non-CRUD verb (e.g. a future `echo.audit` with no typed export)
/// would land here.
pub const NON_DISPATCHABLE_CODE: ErrorCode = ErrorCode(-4003);

/// Plugin execution error: the typed export trapped (resource-limit breach /
/// non-limit trap) or the plugin was not registered in the pool. Distinct from
/// auth/permission/non-dispatchable so the caller can tell a runtime execution
/// failure from a pre-dispatch rejection (R-0010-f).
pub const PLUGIN_EXEC_CODE: ErrorCode = ErrorCode(-4004);

/// Verb not exposed: the requested verb is absent from the plugin manifest's
/// declared `verbs` list (R-0010-d, R-0010-f class verb-not-found).
///
/// This is the **manifest-verbs membership gate** (R-0010-d / R-0019-c).
/// It fires pre-dispatch, after DF-auth-check (R-0010-c) passes, when the
/// requested verb is NOT in the registered plugin's manifest `verbs` list.
///
/// Distinct from:
///   -4001 AUTH_FAILURE_CODE      — bad/missing token
///   -4002 PERMISSION_DENIED_CODE — valid token, wrong role (R-0009)
///   -4003 NON_DISPATCHABLE_CODE  — verb declared in manifest, no typed export
///   -4004 PLUGIN_EXEC_CODE       — execution-time trap / pool miss
pub const VERB_NOT_EXPOSED_CODE: ErrorCode = ErrorCode(-4005);

/// Supervisor degraded: the epoch-tick thread is not healthy; plugin dispatch
/// is unsafe (R-0007-h).
///
/// Returned from `call_tool` when `pool.can_invoke()` returns `false` — the
/// epoch-tick supervisor has died or been degraded. This is a security control:
/// the gate FAILS CLOSED (refuses, never passes through on error).
///
/// Placed after the manifest-verbs membership gate (-4005) and before
/// `invoke_content` — a degraded supervisor blocks a valid, authorized,
/// dispatchable request at the pre-dispatch chokepoint.
///
/// Distinct from:
///   -4001 AUTH_FAILURE_CODE         — bad/missing token
///   -4002 PERMISSION_DENIED_CODE    — valid token, wrong role
///   -4003 NON_DISPATCHABLE_CODE     — verb declared, no typed export
///   -4004 PLUGIN_EXEC_CODE          — execution-time trap / pool miss
///   -4005 VERB_NOT_EXPOSED_CODE     — verb absent from manifest verbs list
pub const SUPERVISOR_DEGRADED_CODE: ErrorCode = ErrorCode(-4006);
