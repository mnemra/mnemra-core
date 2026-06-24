//! MCP error codes for auth and permission failures (R-0010-f).
//!
//! Defines custom `ErrorCode` values that do NOT overlap with the standard
//! JSON-RPC error codes (-32600 through -32603). The test contract
//! (`mcp_server.rs`) explicitly asserts these codes are distinct.
//!
//! # Code allocation
//!
//! Custom mnemra MCP error codes occupy the range -4000 to -4099.
//!   -4001 : AUTH_FAILURE_CODE   — bad/missing token; no WorkspaceCtx constructed
//!   -4002 : PERMISSION_DENIED_CODE — valid token, wrong role for the verb
//!   -4003 : NON_DISPATCHABLE_CODE — manifest verb with no typed export (R-0019-d)
//!   -4004 : PLUGIN_EXEC_CODE    — plugin execution error (trap / not-registered)

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
/// # R-0010-d/f, R-0009-e
///
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
