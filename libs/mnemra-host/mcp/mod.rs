//! MCP server module — rmcp ServerHandler impl for the mnemra host (Task 23).
//!
//! # Module structure
//!
//! - `errors`   — Custom `ErrorCode` constants (AUTH_FAILURE_CODE, PERMISSION_DENIED_CODE).
//! - `server`   — `MnemraMcpServer` implementing `rmcp::ServerHandler`.
//! - `dispatch` — DF-auth-check + WorkspaceCtx construction + verb capability check.

pub mod dispatch;
pub mod errors;
pub mod server;
