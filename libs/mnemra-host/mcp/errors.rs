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
