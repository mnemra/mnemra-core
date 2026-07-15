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
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
    ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use sqlx::PgPool;
use uuid::Uuid;

use crate::coordination::CoordinationConfig;
use crate::coordination::leases;
use crate::coordination::messages;
use crate::coordination::session_plane;
use crate::coordination::write_path::PgCoordinationStore;
use crate::mcp::dispatch::{
    auth_and_authorize, authorize_coordination_action, resolve_content_call, resolve_ctx_from_token,
};
use crate::mcp::errors::{SUPERVISOR_DEGRADED_CODE, VERB_NOT_EXPOSED_CODE};
use crate::plugin::manifest::parse_manifest;
use crate::plugin::pool::PluginPool;
use crate::plugin::trap_recovery::{ContentCall, ContentResult, ResourceBudget, invoke_content};

/// Query scan timeout: the host read-path Postgres `statement_timeout` canceled
/// the keyset query (SQLSTATE 57014) — the R-0020-d scan-cost backstop firing.
///
/// A DISTINCT custom code per R-0010-f no-conflation: it is NOT `PLUGIN_EXEC_CODE`
/// (-4004, the guest epoch-deadline `plugin_execution_timeout`) and NOT
/// `INVALID_PARAMS` (-32602, parameter-invalid). It extends the custom mnemra MCP
/// code range (-4000..-4099, allocated in `crate::mcp::errors`) with -4007.
///
/// Defined here rather than in `errors.rs` to stay within this task's edit scope;
/// a follow-up may relocate it alongside -4001..-4006 for convention consistency.
const QUERY_SCAN_TIMEOUT_CODE: rmcp::model::ErrorCode = rmcp::model::ErrorCode(-4007);

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

/// The registered pool name for the echo content plugin. The pool is populated
/// under this name (T11 startup / the slice-1 harness); `call_tool` borrows a
/// pooled instance by it.
pub const ECHO_PLUGIN_NAME: &str = "mnemra-echo";

/// The mnemra MCP server — single entry point for all MCP tool calls (R-0010-a).
///
/// Holds a `PgPool` for auth lookups, the `PluginPool` of live component
/// instances for dispatch, and a pre-compiled list of echo plugin verbs for tool
/// advertisement.
pub struct MnemraMcpServer {
    pool: PgPool,
    /// The live component-instance pool the dispatch path borrows from (R-0016-a).
    plugin_pool: Arc<PluginPool>,
    /// Plugin verbs parsed from the echo manifest at construction time.
    echo_verbs: Vec<String>,
    /// The host-served coordination write path (`message`/`claim`), constructed
    /// with `coordination_config.write_timeout`. The `message poll` bind body
    /// (`handle_coordination`) drives `run_write` through this store.
    coordination_store: PgCoordinationStore,
    /// Coordination runtime config (attachment TTL + write timeout), threaded
    /// from [`crate::RunConfig`]. `attachment_ttl` is read by the `poll` bind
    /// body to compute the attachment lease `expires_at`.
    coordination_config: CoordinationConfig,
}

impl MnemraMcpServer {
    /// Construct a new `MnemraMcpServer` with the auth pool and the live plugin
    /// pool (R-0010-a, R-0016-a).
    ///
    /// Parses the echo manifest to extract verb names for `list_tools`.
    /// Does NOT start any background tasks or open additional connections.
    ///
    /// # Panics
    ///
    /// Panics if the embedded echo manifest TOML cannot be parsed. This is a
    /// compile-time-embedded file; a parse failure indicates a broken build.
    pub fn new(pool: PgPool, plugin_pool: Arc<PluginPool>) -> Self {
        let manifest =
            parse_manifest(ECHO_MANIFEST_TOML).expect("embedded echo manifest must be valid TOML");
        let echo_verbs = manifest.verbs.exposed;
        // Coordination config defaults to the §Numeric-calibrations dogfood
        // values; `run_with` overrides them from `RunConfig` via
        // `with_coordination_config`. The default keeps the many test
        // constructors (`MnemraMcpServer::new(pool, plugin_pool)`) working
        // without threading config through every call site.
        let coordination_config = CoordinationConfig::default();
        let coordination_store =
            PgCoordinationStore::new(Arc::new(pool.clone()), coordination_config.write_timeout);
        Self {
            pool,
            plugin_pool,
            echo_verbs,
            coordination_store,
            coordination_config,
        }
    }

    /// Override the coordination runtime config (write timeout + attachment
    /// TTL). `MnemraMcpServer::new` seeds the §Numeric-calibrations defaults for
    /// the test constructors; `run_with` threads the deployment/config values
    /// here. Rebuilds the coordination store so the (possibly overridden) write
    /// timeout is applied.
    pub fn with_coordination_config(mut self, config: CoordinationConfig) -> Self {
        self.coordination_store =
            PgCoordinationStore::new(Arc::new(self.pool.clone()), config.write_timeout);
        self.coordination_config = config;
        self
    }

    /// Host-served coordination dispatch (`message`/`claim`) — R-0063-a/-c.
    ///
    /// Contract order (R-0063-c): authenticate (→ `WorkspaceCtx` via the single
    /// construction site) → parse the closed `action` argument → per-action
    /// token-role capability gate (R-0073-b) → route. At this foundation stage
    /// `poll` routes to a placeholder; the real bind/attach/delivery body lands
    /// in a later sub-run, Glitch-red-first.
    ///
    /// This branch deliberately does NOT gate on `PluginPool::can_invoke()`:
    /// that health gate protects the plugin (WASM) runtime, and coordination is
    /// host-served with no plugin dispatch (R-0063-a).
    async fn handle_coordination(
        &self,
        token_str: &str,
        request: &CallToolRequestParams,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        // Authenticate before routing (R-0063-c): resolve the token to a
        // WorkspaceCtx through the single construction site (R-0006-b).
        let ctx = resolve_ctx_from_token(token_str, &self.pool).await?;

        // Parse the closed `action` argument (defensive — never trust the
        // client-supplied shape, even though the advertised schema constrains
        // it). Tool-aware (`request.name`): `claim` and `message` each own
        // their own action vocabulary.
        let action =
            session_plane::parse_action(request.name.as_ref(), request.arguments.as_ref())?;

        // Per-action token-role capability gate (R-0073-b): a `read_observer`
        // token is refused pre-dispatch — `poll` is write-category and every
        // coordination action executes under an attachment (itself a write), so
        // a read-observer cannot participate in the coordination surface at all.
        authorize_coordination_action(&ctx, &action)?;

        // Route the action to the session plane. `poll` is the bind call
        // (R-0064-e): resolve-or-create + attach + audit through the
        // coordination write path.
        match action {
            session_plane::CoordinationAction::Poll => {
                // `poll` carries the bind `role_instance` argument (the advertised
                // schema marks it required). An absent/non-string value is a
                // malformed request — a protocol error (INVALID_PARAMS), distinct
                // from a present-but-invalid identifier (which the bind body
                // refuses `invalid_role_instance`).
                let role_instance = request
                    .arguments
                    .as_ref()
                    .and_then(|m| m.get("role_instance"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "coordination `poll` requires a string `role_instance` argument"
                            .into(),
                        data: None,
                    })?;
                session_plane::poll_bind(
                    &self.coordination_store,
                    &self.coordination_config,
                    &ctx,
                    role_instance,
                )
                .await
            }
            session_plane::CoordinationAction::Send => {
                // `send` carries four required arguments: `to_role_instance`
                // (string), `type` (string — the registered message-type
                // name), `schema_version` (a JSON number, matching
                // `message_types`'s `u16` version type), and `payload` (the
                // JSON value forwarded verbatim to `validate_message`). Each
                // absent/wrong-type argument is a malformed request
                // (INVALID_PARAMS) — distinct from a present-but-invalid
                // value, which the send body refuses via its own structured
                // reason code (`invalid_role_instance`/`schema_violation`/
                // `unknown_type`).
                let to_role_instance = request
                    .arguments
                    .as_ref()
                    .and_then(|m| m.get("to_role_instance"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "coordination `send` requires a string `to_role_instance` \
                                   argument"
                            .into(),
                        data: None,
                    })?;
                let type_name = request
                    .arguments
                    .as_ref()
                    .and_then(|m| m.get("type"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "coordination `send` requires a string `type` argument".into(),
                        data: None,
                    })?;
                let schema_version_raw = request
                    .arguments
                    .as_ref()
                    .and_then(|m| m.get("schema_version"))
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "coordination `send` requires an integer `schema_version` \
                                   argument"
                            .into(),
                        data: None,
                    })?;
                let schema_version: u16 =
                    schema_version_raw
                        .try_into()
                        .map_err(|_| rmcp::model::ErrorData {
                            code: rmcp::model::ErrorCode::INVALID_PARAMS,
                            message: "coordination `send`'s `schema_version` argument is out \
                                       of range"
                                .into(),
                            data: None,
                        })?;
                let payload = request
                    .arguments
                    .as_ref()
                    .and_then(|m| m.get("payload"))
                    .cloned()
                    .ok_or_else(|| rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "coordination `send` requires a `payload` argument".into(),
                        data: None,
                    })?;
                messages::send(
                    &self.coordination_store,
                    &ctx,
                    to_role_instance,
                    type_name,
                    schema_version,
                    payload,
                )
                .await
            }
            session_plane::CoordinationAction::ClaimAcquire => {
                // `acquire` carries the required `resource` argument and the
                // optional `duration_seconds` argument. `resource` absent/
                // non-string is a malformed request (INVALID_PARAMS), distinct
                // from a present-but-invalid resource (which the acquire body
                // refuses `invalid_resource`/`reserved_family`). `duration_seconds`
                // absent is `None` (defaults, R-0065-d); present-but-non-integer
                // is also a malformed request — never silently treated as absent.
                let resource = request
                    .arguments
                    .as_ref()
                    .and_then(|m| m.get("resource"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "coordination `acquire` requires a string `resource` argument"
                            .into(),
                        data: None,
                    })?;
                let duration_seconds = match request
                    .arguments
                    .as_ref()
                    .and_then(|m| m.get("duration_seconds"))
                {
                    None => None,
                    Some(v) => Some(v.as_i64().ok_or_else(|| {
                        rmcp::model::ErrorData {
                            code: rmcp::model::ErrorCode::INVALID_PARAMS,
                            message:
                                "coordination `acquire`'s `duration_seconds` argument must be \
                                   an integer"
                                    .into(),
                            data: None,
                        }
                    })?),
                };
                leases::acquire(&self.coordination_store, &ctx, resource, duration_seconds).await
            }
            session_plane::CoordinationAction::ClaimList => {
                // `list` carries the optional `family` and `resource_prefix`
                // filter arguments (§API Contract `list`). Absent is `None`
                // (no filter on that axis); present-but-non-string is a
                // malformed request (INVALID_PARAMS), never silently treated
                // as absent — mirrors `acquire`'s `duration_seconds` handling
                // above.
                let family = match request.arguments.as_ref().and_then(|m| m.get("family")) {
                    None => None,
                    Some(v) => Some(v.as_str().ok_or_else(|| rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "coordination `list`'s `family` argument must be a string".into(),
                        data: None,
                    })?),
                };
                let resource_prefix = match request
                    .arguments
                    .as_ref()
                    .and_then(|m| m.get("resource_prefix"))
                {
                    None => None,
                    Some(v) => Some(v.as_str().ok_or_else(|| {
                        rmcp::model::ErrorData {
                            code: rmcp::model::ErrorCode::INVALID_PARAMS,
                            message: "coordination `list`'s `resource_prefix` argument must be a \
                                   string"
                                .into(),
                            data: None,
                        }
                    })?),
                };
                leases::list(&self.coordination_store, &ctx, family, resource_prefix).await
            }
            session_plane::CoordinationAction::ClaimRenew => {
                // `renew` carries the required `lease_id` argument (§API
                // Contract) — a string UUID, distinct from `acquire`'s
                // `resource` argument. Absent/non-string is a malformed
                // request (INVALID_PARAMS); a present-but-unparseable UUID
                // is ALSO malformed — never silently treated as
                // `lease_not_found` (that refusal is reserved for a
                // well-formed id the claim body cannot find/reach).
                let lease_id_str = request
                    .arguments
                    .as_ref()
                    .and_then(|m| m.get("lease_id"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "coordination `renew` requires a string `lease_id` argument"
                            .into(),
                        data: None,
                    })?;
                let lease_id =
                    Uuid::parse_str(lease_id_str).map_err(|_| rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "coordination `lease_id` argument must be a valid UUID".into(),
                        data: None,
                    })?;
                leases::renew(&self.coordination_store, &ctx, lease_id).await
            }
            session_plane::CoordinationAction::ClaimRelease => {
                // Same `lease_id` argument contract as `renew` above.
                let lease_id_str = request
                    .arguments
                    .as_ref()
                    .and_then(|m| m.get("lease_id"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "coordination `release` requires a string `lease_id` argument"
                            .into(),
                        data: None,
                    })?;
                let lease_id =
                    Uuid::parse_str(lease_id_str).map_err(|_| rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "coordination `lease_id` argument must be a valid UUID".into(),
                        data: None,
                    })?;
                leases::release(&self.coordination_store, &ctx, lease_id).await
            }
            session_plane::CoordinationAction::ClaimTakeover => {
                // `takeover` carries the required `resource` argument — the
                // SAME argument contract as `acquire` above (§API Contract
                // `takeover`: `{ action: "takeover", resource }`); it takes
                // no `duration_seconds` (the recovered lease always uses the
                // configured default, `leases::takeover`'s own doc comment).
                let resource = request
                    .arguments
                    .as_ref()
                    .and_then(|m| m.get("resource"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "coordination `takeover` requires a string `resource` argument"
                            .into(),
                        data: None,
                    })?;
                leases::takeover(&self.coordination_store, &ctx, resource).await
            }
        }
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
        let mut tools: Vec<Tool> = self
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
        // Host-served coordination tools (R-0063-a), advertised in addition to
        // the plugin (echo) verbs. Control-plane verbs stay absent (R-0010-g);
        // `message`/`claim` are not control-plane and carry no forbidden name.
        tools.extend(session_plane::coordination_tools());
        Ok(ListToolsResult::with_all_items(tools))
    }

    /// Call a tool — DF-auth-check + role check + manifest-verbs gate + health gate + dispatch (R-0010-c/d, R-0007-h).
    ///
    /// Checks run in order before any plugin routing (R-0010-c/d):
    ///   1. Auth: token lookup (R-0010-c). Failure → AUTH_FAILURE_CODE (-4001).
    ///   2. Role: permission check (R-0009-d/e). Failure → PERMISSION_DENIED_CODE (-4002).
    ///   3. Manifest-verbs gate (R-0010-d): verb must be in manifest `verbs` list.
    ///      Failure → VERB_NOT_EXPOSED_CODE (-4005). NOT the same as NON_DISPATCHABLE.
    ///   4. Health gate (R-0007-h): epoch-tick supervisor must be healthy.
    ///      Failure → SUPERVISOR_DEGRADED_CODE (-4006). Fails closed; never passes through.
    ///   5. Dispatch: resolve verb to typed `content` call (R-0019-c/d).
    ///      Declared-but-no-export → NON_DISPATCHABLE_CODE (-4003).
    ///
    /// Returns `Err(ErrorData { code: AUTH_FAILURE_CODE, .. })` if the token
    /// is missing, invalid base64url, or not found in `admin_tokens`.
    ///
    /// Returns `Err(ErrorData { code: PERMISSION_DENIED_CODE, .. })` if the
    /// token resolves but the role is not authorized for the verb.
    ///
    /// Returns `Err(ErrorData { code: VERB_NOT_EXPOSED_CODE, .. })` if the
    /// verb is absent from the plugin manifest's `verbs` list (R-0010-d).
    ///
    /// Returns `Err(ErrorData { code: SUPERVISOR_DEGRADED_CODE, .. })` if the
    /// epoch-tick supervisor is degraded (R-0007-h).
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

        // Host-served coordination branch (R-0063-a). Taken BEFORE the
        // echo-verb manifest membership gate below: `message`/`claim` are not
        // echo verbs, so that gate would reject them. The coordination handler
        // runs its own auth + per-action capability gate (R-0073-b) and routes
        // to the session plane; it never touches `resolve_content_call` /
        // `invoke_content` (no plugin ABI, no WIT surface — R-0063-a). The
        // existing plugin path stays the `else`.
        if session_plane::is_coordination_tool(request.name.as_ref()) {
            return self.handle_coordination(&token_str_owned, &request).await;
        }

        // DF-auth-check + role permission check (R-0010-c, R-0009-d/e).
        // token_str is consumed; it does not appear in any error payload.
        let ctx = auth_and_authorize(&token_str_owned, &request.name, &self.pool).await?;
        let workspace_id = ctx.workspace_id();

        // Manifest-verbs membership gate (R-0010-d, R-0019-c).
        //
        // Rejects any verb NOT present in the registered plugin's manifest
        // `verbs` list before routing to the plugin. This is the pre-dispatch
        // capability check: "is this verb exposed by the plugin at all?"
        //
        // Placement rationale: after auth_and_authorize (DF-auth-check passes
        // first) and before resolve_content_call (no dispatch attempt for
        // undeclared verbs). Placement is unbypassable — every authenticated
        // call_tool path passes through this check regardless of token role.
        //
        // Membership-only: this gate answers "is the verb declared in the
        // manifest?" It does NOT subsume resolve_content_call. A verb that IS
        // declared but has no typed export (e.g. echo.audit) passes this gate
        // and then receives NON_DISPATCHABLE (-4003) from resolve_content_call
        // exactly as before (R-0019-d, precision guard — test #2).
        if !self.echo_verbs.iter().any(|v| v == request.name.as_ref()) {
            return Err(rmcp::model::ErrorData {
                code: VERB_NOT_EXPOSED_CODE,
                message: format!(
                    "verb '{}' is not in the registered plugin's manifest verbs list",
                    request.name
                )
                .into(),
                data: None,
            });
        }

        // R-0007-h: refuse invocation while the epoch-tick supervisor is degraded.
        //
        // Security control — fails closed: a degraded supervisor blocks dispatch.
        // Reads from the SAME Arc<PluginPool> the harness degrades in tests, so the
        // health state is live (not a snapshot). A false return from can_invoke()
        // means the epoch-tick thread has died; forwarding to invoke_content would
        // run unticked epochs, voiding the resource-limit contract (R-0007-e/f).
        //
        // Placement: after auth + permission + manifest-verbs checks, before
        // resolve_content_call. A valid, authorized, dispatchable request receives
        // the distinct SUPERVISOR_DEGRADED code (-4006), not auth/permission/verb/
        // plugin-exec — which is what the distinctness guards in the test assert.
        //
        // TRIPWIRE: V0 is MCP-only — server.rs is the sole invoke_content caller.
        // When a second content-dispatching frontend (CLI/HTTP, T11+) lands, hoist
        // this can_invoke() check into the shared invoke_content path so every
        // frontend is gated (task #1702 scope).
        if !self.plugin_pool.can_invoke() {
            return Err(rmcp::model::ErrorData {
                code: SUPERVISOR_DEGRADED_CODE,
                message: "epoch-tick supervisor is degraded; plugin dispatch is unsafe".into(),
                data: None,
            });
        }

        // Resolve the authenticated verb to its typed `content` call, reading the
        // MCP arguments (CC-MAPPING + R1). A manifest-declared verb with no typed
        // export returns the R-0019-d structured non-dispatchable error; the
        // pre-dispatch permission outcome above is unchanged (R-0019-e). Verb
        // -> export resolution is STATIC against the fixed `content` interface —
        // no runtime export registry (FENCE R-0019-c).
        let dispatch = resolve_content_call(&request.name, request.arguments.as_ref())?;

        // Cursor ULID validation — performed here at the MCP boundary, BEFORE
        // invoke_content. Reason: artifact_list returns WitArtifactPage (not a
        // Result); panics inside the host-fn are caught by the catch_unwind seam
        // in invoke_through_recovery and surface as PLUGIN_EXEC_CODE (-4004), NOT
        // INVALID_PARAMS (-32602). Validating here is the only way to return the
        // spec-required -32602 code for a malformed cursor (R-0020-b).
        if let ContentCall::List {
            cursor: Some(ref c),
            ..
        } = dispatch.call
            && !is_valid_ulid(c)
        {
            return Err(rmcp::model::ErrorData {
                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                message: "cursor is not a valid ULID".into(),
                data: None,
            });
        }

        // Invoke the typed `content` export on a pooled component instance. The
        // pool invoke is synchronous (wasmtime `Store` is not async) and may run
        // until the epoch deadline on a trapping plugin, so it runs on a blocking
        // thread to keep the async runtime free. The host-derived `workspace_id`
        // is bound onto the store inside `invoke_content` at the single dispatch
        // site (R-0006-b); the guest never supplies it (R-0006-e).
        let plugin_pool = Arc::clone(&self.plugin_pool);
        let pg_pool = self.pool.clone();
        let verb = request.name.to_string();
        // Time the dispatch for the bounded R-0004-a `duration_ms` carried on the
        // scan-timeout metric (R-0020-d). The host-side statement_timeout caps the
        // read-path query, so this duration is bounded.
        let dispatch_started = std::time::Instant::now();
        // R-0020-d part ii: the scan-cost `statement_timeout` (SQLSTATE 57014) flag
        // is a thread-local set inside the host-fn `artifact_list`. It MUST be read
        // on the same blocking thread the host-fn ran on — `artifact_list` executes
        // synchronously on this `spawn_blocking` thread via `block_on`, so the flag
        // is live in the closure but would be empty after `.await` (a different
        // runtime thread). Clear on entry (guard against a prior invocation on this
        // pooled thread), take immediately after `invoke_content`, and carry the
        // bool out of the closure as a plain value.
        let (invoke_result, scan_timeout) = tokio::task::spawn_blocking(move || {
            crate::plugin::component::clear_scan_timeout();
            let result = invoke_content(
                &plugin_pool,
                ECHO_PLUGIN_NAME,
                &verb,
                dispatch.call,
                ResourceBudget::default(),
                workspace_id,
                &pg_pool,
            );
            let scan_timeout = crate::plugin::component::take_scan_timeout();
            (result, scan_timeout)
        })
        .await
        .map_err(|join_err| {
            // A join error means the blocking task panicked with an exception
            // outside the `catch_unwind` seam in `invoke_through_recovery`.
            // Host-fn panics (DB errors, None-pool) are caught by that seam and
            // never reach here; this path fires only for infrastructure panics.
            // Log the raw join_err server-side for diagnostics; do NOT embed it
            // in the client-facing message — it may contain internal detail
            // (MEDIUM-1 / scrub discipline).
            tracing::error!(
                event = "plugin_dispatch_task_panic",
                error = %join_err,
                "plugin dispatch blocking task panicked outside the recovery seam"
            );
            rmcp::model::ErrorData {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: "plugin dispatch task failed: internal error".into(),
                data: None,
            }
        })?;

        // R-0020-d part ii: a host-side `statement_timeout` (SQLSTATE 57014)
        // cancellation on the read-path keyset query. The host-fn flagged it on the
        // side-channel and failed closed (its panic was collapsed to a generic exec
        // error by the recovery seam); surface the DISTINCT caller-facing
        // `query_scan_timeout` code here — distinct from `plugin_execution_timeout`
        // (the guest epoch -4004) and from `-32602` (parameter-invalid) — plus the
        // R-0004-a `outcome = "timeout"` metric (workspace_id / verb / outcome /
        // bounded duration_ms; cursor + next-cursor excluded, R-0020-g). The error
        // body carries NO Postgres/schema/DB internals (R-0020-e no-leak posture).
        if scan_timeout {
            let duration_ms = dispatch_started.elapsed().as_millis() as u64;
            tracing::warn!(
                event = "verb_metric",
                workspace_id = %workspace_id,
                verb = %request.name,
                outcome = "timeout",
                duration_ms,
                "artifact.list read-path scan-cost statement_timeout cancellation (R-0020-d)"
            );
            return Err(rmcp::model::ErrorData {
                code: QUERY_SCAN_TIMEOUT_CODE,
                message: "query scan timeout".into(),
                data: None,
            });
        }

        // R-0020-b / R-0016-a: invoke-inclusive duration_ms for all verbs.
        //
        // dispatch_started was set before spawn_blocking, so this elapsed span
        // covers pool-slot acquire wait + guest execution (R-0016-a). Computing
        // it once here (rather than per-arm) gives a consistent measurement point
        // for the backfillable browse-by-metadata signal (#1919 tripwire): offline
        // queries correlate verb_metric events by workspace_id across the full verb
        // set (list + get + others), reconstructing the list-then-N-gets pattern.
        let duration_ms = dispatch_started.elapsed().as_millis() as u64;

        // Map the typed return into a `CallToolResult` (R2). On a plugin
        // execution error (trap / not-registered), surface a structured MCP error
        // with a distinguishable code (R-0010-f) — never a vacuous `Ok`.
        //
        // R-0004-a / R-0020-g: every arm emits a `verb_metric` structured-log event
        // carrying the R-0004-a floor (workspace_id / verb / outcome / duration_ms).
        // Artifact IDs and content values are never emitted (R-0020-g exclusion).
        // The timeout path early-returns above with outcome="timeout"; exactly one
        // verb_metric fires per dispatch (invariant maintained across all arms).
        match invoke_result {
            Ok(ContentResult::Created(id)) => {
                // R-0004-a: per-verb metric. `id` is NOT emitted (artifact-id, R-0020-g).
                tracing::info!(
                    event = "verb_metric",
                    workspace_id = %workspace_id,
                    verb = %request.name,
                    outcome = "ok",
                    duration_ms,
                    "artifact.create dispatch completed (R-0004-a)"
                );
                // `echo.create` -> the generated ULID as a text content item.
                Ok(CallToolResult::success(vec![Content::text(id)]))
            }
            Ok(ContentResult::Got(Some(content))) => {
                // R-0004-a: per-verb metric. `content` is NOT emitted (artifact data, R-0020-g).
                // This arm is the primary `get` success path; offline queries correlating
                // list-then-get sequences per workspace_id fire against this event (#1919).
                tracing::info!(
                    event = "verb_metric",
                    workspace_id = %workspace_id,
                    verb = %request.name,
                    outcome = "ok",
                    duration_ms,
                    "artifact.get dispatch completed (R-0004-a)"
                );
                // `echo.get` -> the stored content (round-trips the payload).
                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
            Ok(ContentResult::Got(None)) => {
                // R-0004-a: per-verb metric. Not-found is a valid response (outcome "ok").
                // Counted separately from Got(Some) by offline queries — both are get verbs.
                tracing::info!(
                    event = "verb_metric",
                    workspace_id = %workspace_id,
                    verb = %request.name,
                    outcome = "ok",
                    duration_ms,
                    "artifact.get dispatch completed (not found) (R-0004-a)"
                );
                // Not found / not visible in this workspace — an empty-content Ok
                // result (the readback path; cross-workspace get lands here).
                Ok(CallToolResult::success(vec![]))
            }
            Ok(ContentResult::Listed(page)) => {
                // `echo.list` -> artifact-page as MCP structured_content (R-0020 T16).
                // The ids are already workspace-scoped in the fenced-list host body
                // (R-0006-d); a cross-workspace id cannot appear here.
                // CallToolResult::structured() populates both structured_content (the
                // JSON object) and content[0].text (the JSON string) so that clients
                // which read only the text content still see all ids.
                //
                // R-0004-a / R-0020-g: per-verb metric for the list success path.
                // cursor (in) and next-cursor (out) are intentionally excluded — both
                // are artifact IDs and must not appear in the metric or structured log
                // (R-0020-g). duration_ms is invoke-inclusive (computed above, R-0016-a).
                tracing::info!(
                    event = "verb_metric",
                    workspace_id = %workspace_id,
                    verb = %request.name,
                    outcome = "ok",
                    duration_ms,
                    "artifact.list dispatch completed (R-0004-a; cursor+next-cursor excluded R-0020-g)"
                );
                Ok(CallToolResult::structured(serde_json::json!({
                    "ids": page.ids,
                    "has_more": page.has_more,
                    "next_cursor": page.next_cursor,
                })))
            }
            Ok(ContentResult::Updated) => {
                // R-0004-a: per-verb metric.
                tracing::info!(
                    event = "verb_metric",
                    workspace_id = %workspace_id,
                    verb = %request.name,
                    outcome = "ok",
                    duration_ms,
                    "artifact.update dispatch completed (R-0004-a)"
                );
                // `echo.update` is void — an empty-content success result (like the
                // `Got(None)` readback). The merge/persist happened host-side; a
                // missing/cross-workspace target was a silent no-op (R-0006-d).
                Ok(CallToolResult::success(vec![]))
            }
            Ok(ContentResult::Deleted) => {
                // R-0004-a: per-verb metric.
                tracing::info!(
                    event = "verb_metric",
                    workspace_id = %workspace_id,
                    verb = %request.name,
                    outcome = "ok",
                    duration_ms,
                    "artifact.delete dispatch completed (R-0004-a)"
                );
                // `echo.delete` is void — an empty-content success result. The
                // delete happened host-side; a missing/cross-workspace target was a
                // silent no-op (R-0006-d, miss=no-op).
                Ok(CallToolResult::success(vec![]))
            }
            Err(exec_err) => {
                // R-0004-a: per-verb metric for plugin execution failures (trap / not-registered).
                tracing::warn!(
                    event = "verb_metric",
                    workspace_id = %workspace_id,
                    verb = %request.name,
                    outcome = "error",
                    duration_ms,
                    "plugin execution failed (R-0004-a)"
                );
                Err(rmcp::model::ErrorData {
                    code: crate::mcp::errors::PLUGIN_EXEC_CODE,
                    message: format!("plugin execution failed: {}", exec_err.code()).into(),
                    data: None,
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ULID validation helper (R-0020-b)
// ---------------------------------------------------------------------------

/// Return `true` iff `s` is a syntactically valid Crockford base32-encoded ULID.
///
/// Rules:
/// - Exactly 26 characters.
/// - First character in `[0-7]` — the top 3 bits of the 130-bit encoding must be
///   zero so the value fits in 128 bits.
/// - All characters from the Crockford base32 alphabet:
///   `0-9`, `A-H`, `J-K`, `M-N`, `P-T`, `V-Z` (excludes `I`, `L`, `O`, `U`).
fn is_valid_ulid(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 26 {
        return false;
    }
    // First character must be in [0-7] to keep the 128-bit ULID value in range.
    if !matches!(b[0], b'0'..=b'7') {
        return false;
    }
    b.iter().all(|&c| {
        matches!(
            c,
            b'0'..=b'9'
                | b'A'..=b'H'
                | b'J'..=b'K'
                | b'M'..=b'N'
                | b'P'..=b'T'
                | b'V'..=b'Z'
        )
    })
}
