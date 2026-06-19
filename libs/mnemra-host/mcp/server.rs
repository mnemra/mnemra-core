//! MCP server — `MnemraMcpServer` implementing `rmcp::ServerHandler` (R-0010-a/b/c/d).
//!
//! # Design
//!
//! `MnemraMcpServer` is the single MCP entry point for the mnemra host (R-0010-a).
//! It implements `rmcp::ServerHandler` and is served over a duplex in-process
//! transport in tests, and over stdio in production (R-0010-a/b).
//!
//! # Verb advertisement (R-0010-b/g)
//!
//! Plugin verbs are advertised as MCP tools via `list_tools`. The echo plugin's
//! verbs are read from the manifest at construction time via `parse_manifest`
//! (TOML-only parse, no signature verification required for the verb list).
//! Control-plane verbs (workspace create/delete, token rotation, migration,
//! backup) are NEVER advertised (R-0010-g).
//!
//! # Auth-check scope (resolved decision)
//!
//! DF-auth-check runs on `call_tool` ONLY — NOT on `initialize` or `list_tools`.
//! The `list_tools` handler returns the tool list without requiring a token.
//!
//! # Task 5 wiring note (Test 1 gap)
//!
//! After auth and permission checks pass, the actual plugin dispatch is a stub
//! that returns `Ok(CallToolResult::default())`. Full plugin dispatch (invoking
//! the WASM module + routing to artifact host-fns) requires Task 5 storage
//! wiring (`artifact_create` is `todo!()`) and Task 22 PluginPool wiring.
//! Test 1 passes because it asserts only `result.is_ok()`, not content.
//! The dispatch stub is replaced in a future task.

use std::borrow::Cow;
use std::sync::Arc;

use rmcp::RoleServer;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, ServerInfo,
    Tool,
};
use rmcp::service::RequestContext;
use sqlx::PgPool;

use crate::mcp::dispatch::auth_and_authorize;
use crate::plugin::manifest::parse_manifest;

// ---------------------------------------------------------------------------
// Echo manifest bytes (embedded at compile time)
// ---------------------------------------------------------------------------

/// Embedded echo manifest TOML. Used ONLY for verb-list extraction via
/// `parse_manifest` (TOML parse, no signature gate). The `[signature]` section
/// is present but ignored by `parse_manifest`.
///
/// At V0 the echo plugin ships a placeholder signature (public_key = "ROOT"),
/// so `PluginRuntime::load` (signature-verifying) would reject it. Verb
/// advertisement does NOT need a verified runtime; it needs only the
/// `[verbs].exposed` list, which is safe to extract from TOML alone.
const ECHO_MANIFEST_TOML: &[u8] = include_bytes!("../../../plugins/mnemra-echo/manifest.toml");

// ---------------------------------------------------------------------------
// MnemraMcpServer
// ---------------------------------------------------------------------------

/// The mnemra MCP server — single entry point for all MCP tool calls (R-0010-a).
///
/// Holds a `PgPool` for auth lookups and a pre-compiled list of echo plugin
/// verbs for tool advertisement.
pub struct MnemraMcpServer {
    pool: PgPool,
    /// Plugin verbs parsed from the echo manifest at construction time.
    echo_verbs: Vec<String>,
}

impl MnemraMcpServer {
    /// Construct a new `MnemraMcpServer` with the given pool.
    ///
    /// Parses the echo manifest to extract verb names for `list_tools`.
    /// Does NOT start any background tasks or open additional connections.
    ///
    /// # Panics
    ///
    /// Panics if the embedded echo manifest TOML cannot be parsed. This is a
    /// compile-time-embedded file; a parse failure indicates a broken build.
    pub fn new(pool: PgPool) -> Self {
        let manifest =
            parse_manifest(ECHO_MANIFEST_TOML).expect("embedded echo manifest must be valid TOML");
        let echo_verbs = manifest.verbs.exposed;
        Self { pool, echo_verbs }
    }
}

// ---------------------------------------------------------------------------
// ServerHandler implementation
// ---------------------------------------------------------------------------

impl ServerHandler for MnemraMcpServer {
    /// Return server metadata for the MCP initialize handshake.
    fn get_info(&self) -> ServerInfo {
        ServerInfo::default()
    }

    /// List all advertised MCP tools (R-0010-b/g).
    ///
    /// Returns echo plugin verbs as `Tool` entries with a minimal empty
    /// input_schema. Control-plane verbs are never included (R-0010-g).
    ///
    /// No auth check — `list_tools` is unauthenticated per the resolved
    /// decision (dispatch envelope §RESOLVED DECISIONS).
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::model::ErrorData> {
        let empty_schema: Arc<rmcp::model::JsonObject> = Arc::new(Default::default());
        let tools: Vec<Tool> = self
            .echo_verbs
            .iter()
            .map(|verb_name| {
                Tool::new_with_raw(
                    Cow::Owned(verb_name.clone()),
                    None,
                    Arc::clone(&empty_schema),
                )
            })
            .collect();
        Ok(ListToolsResult::with_all_items(tools))
    }

    /// Call a tool — DF-auth-check + permission check + stub dispatch (R-0010-c/d).
    ///
    /// Auth and permission checks run before any routing (R-0010-c). On success,
    /// returns a minimal `Ok(CallToolResult)`. Full plugin dispatch is wired in
    /// a future task (Task 5 storage + Task 22 PluginPool integration).
    ///
    /// Returns `Err(ErrorData { code: AUTH_FAILURE_CODE, .. })` if the token
    /// is missing, invalid base64url, or not found in `admin_tokens`.
    ///
    /// Returns `Err(ErrorData { code: PERMISSION_DENIED_CODE, .. })` if the
    /// token resolves but the role is not authorized for the verb.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        // Extract token string from _meta.token (R-0010-c open seam #1).
        //
        // rmcp deserialization moves `_meta` out of `params` and into
        // `RequestContext.meta` (see rmcp serde_impl.rs). The token is
        // therefore in `context.meta`, not `request.meta`.
        // Missing or non-string "token" key = auth failure.
        let token_str_owned: String = context
            .meta
            .get("token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .ok_or_else(|| rmcp::model::ErrorData {
                code: crate::mcp::errors::AUTH_FAILURE_CODE,
                message: "authentication failed".into(),
                data: None,
            })?;

        // DF-auth-check + per-verb capability check (R-0010-c/d).
        // token_str is consumed; it does not appear in any error payload.
        let _ctx = auth_and_authorize(&token_str_owned, &request.name, &self.pool).await?;

        // Auth and permission passed. Return a minimal Ok result.
        // Full plugin dispatch is out of scope until Task 5 + Task 22 wiring lands.
        Ok(CallToolResult::default())
    }
}
