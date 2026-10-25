//! MCP server module — rmcp ServerHandler impl for the mnemra host (Task 23).
//!
//! # Module structure
//!
//! - `errors`   — Custom `ErrorCode` constants (AUTH_FAILURE_CODE, PERMISSION_DENIED_CODE).
//! - `gate`     — initialize gate: no request is served before the handshake (#4295).
//! - `server`   — `MnemraMcpServer` implementing `rmcp::ServerHandler`.
//! - `dispatch` — DF-auth-check + WorkspaceCtx construction + verb capability check.
//! - `transport` — stdio transport whose over-cap incoming line closes the stream (#4295).

pub mod dispatch;
pub mod errors;
pub mod gate;
pub mod server;
pub mod transport;
