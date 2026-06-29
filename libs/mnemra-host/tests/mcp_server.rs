//! MCP server acceptance tests — RED phase (Task 23 unit-2, dispatch #1052).
//!
//! # Purpose
//!
//! This file pins the contract for `mnemra_host::mcp::*`, the MCP server built
//! on the official `rmcp` Rust SDK. All four behavioral tests reference locked
//! spec R-IDs and exercise the server's external MCP wire interface via an rmcp
//! client over a `tokio::io::duplex` in-memory transport.
//!
//! R-0010-e is in the separate `mcp_feature_guard.rs` binary (see that file).
//!
//! # R-ID mapping
//!
//! | Test function                                                | R-ID(s)                  |
//! |--------------------------------------------------------------|--------------------------|
//! | valid_admin_token_echo_create_returns_ok                     | R-0010-a/b/c/d, R-0006-b |
//! | bogus_token_returns_distinguishable_auth_failure             | R-0010-c/f               |
//! | read_observer_write_denied_permission_error                  | R-0010-d/f, R-0009-e     |
//! | control_plane_verbs_absent_from_tools_list                   | R-0010-g                 |
//! | read_observer_non_read_verb_denied_permission_error (RED)    | R-0009-d/e               |
//! | read_observer_get_verb_not_denied (regression guard)         | R-0009-d                 |
//! | valid_admin_token_echo_update_merges_and_persists (RED)      | R-0019-c                 |
//! | echo_update_cannot_modify_another_workspace_artifact (RED)   | R-0006-d                 |
//! | echo_update_absent_body_preserves_existing_body (RED)        | R-0019-c                 |
//! | valid_admin_token_echo_delete_removes (RED)                  | R-0019-c                 |
//! | echo_delete_cannot_remove_another_workspace_artifact (RED)   | R-0006-d                 |
//! | update_with_malformed_patch_fails_closed (RED-2)             | HIGH-1 (Warden)          |
//! | db_error_message_is_scrubbed_of_sqlx_detail (RED-2)          | MEDIUM-1 (Warden)        |
//!
//! # RED-phase design
//!
//! The behavioral tests exercise the live MCP dispatch path via an rmcp client
//! over the in-memory duplex transport. Tests for wired verbs (create, get, list,
//! update) compile and pass; the delete tests red because `echo.delete` is the
//! remaining unwired arm in `mcp/dispatch.rs` — it returns `NON_DISPATCHABLE -4003`
//! until the T12 delete-slice green phase lands.
//!
//! # rmcp API grounding (Step 0) — verified against rmcp 1.7.0 source
//!
//! Source: context7 `/websites/rs_rmcp_rmcp`, crate source at
//! `~/.cargo/registry/src/.../rmcp-1.7.0/src/`.
//!
//! Verified real rmcp 1.7.0 symbols used:
//! - `rmcp::service::{serve_server, serve_client}` — init server/client from transport
//!   (`serve_server` requires `server` feature; `serve_client` requires `client` feature)
//! - `rmcp::service::RunningService<R, S>` — implements `Deref<Target = Peer<R>>`;
//!   `cancel().await` shuts down; `waiting().await` waits for the task
//! - `Peer<RoleClient>` — exposes `call_tool(CallToolRequestParams)`,
//!   `list_tools(Option<PaginatedRequestParams>)`, `list_all_tools()` (no pagination needed)
//! - `rmcp::model::CallToolRequestParams` — fields: `name`, `meta: Option<Meta>`, `arguments`
//!   (`CallToolRequestParam` is a deprecated alias; the canonical type is `CallToolRequestParams`)
//! - `rmcp::model::Meta` — newtype over `JsonObject`; supports `.insert(key, val)` via
//!   `DerefMut`; used for per-request `_meta.token` auth presentation (open seam #1)
//! - `rmcp::model::{ErrorCode, ErrorData}` — `ErrorData { code: ErrorCode, message, data }`;
//!   `ErrorCode(i32)` with consts `METHOD_NOT_FOUND(-32601)`, `INVALID_PARAMS(-32602)`,
//!   `INVALID_REQUEST(-32600)`, `INTERNAL_ERROR(-32603)`
//! - `rmcp::ServiceError::McpError(ErrorData)` — how the client observes a JSON-RPC error
//! - `()` implements `rmcp::ClientHandler` → implements `Service<RoleClient>`; use as
//!   the minimal client handler: `serve_client((), transport).await`
//! - `tokio::io::duplex(buf_size)` returns `(DuplexStream, DuplexStream)`;
//!   `DuplexStream` implements `AsyncRead + AsyncWrite`, which satisfies the
//!   `IntoTransport` blanket impl via `transport-async-rw` (pulled in by `server` feature)
//!
//! # Open-seam decisions (flagged for Puck to validate before green)
//!
//! See report prose section "Design decisions for Puck to validate before green".
//!
//! # Engine startup
//!
//! Each DB-touching test starts its own embedded Postgres instance. Startup is
//! serialized within this binary via `STARTUP_LOCK` (A-11).
//!
//! # verify: []
//!
//! `verify: []` by design. These tests fail by compile error against the absent
//! `mnemra_host::mcp::*` — there is no just recipe to run against a red binary.
//! Green phase adds the recipe.

use mnemra_host::auth::token::{AdminToken, generate, hash};
use mnemra_host::schema::init::{DEFAULT_WORKSPACE_ID, init};
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use rmcp::model::{CallToolRequestParams, ErrorCode, Meta, RawContent};
use rmcp::service::{RoleClient, RunningService, serve_client, serve_server};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::duplex;
use uuid::Uuid;

use mnemra_host::mcp::server::ECHO_PLUGIN_NAME;
use mnemra_host::plugin::pool::PluginPool;
use wasmtime::component::Component;

// ---------------------------------------------------------------------------
// The absent contract — compile failure IS the valid red.
//
// `mnemra_host::mcp` does not exist. These imports fail to resolve with:
//   error[E0433]: cannot find `mcp` in `mnemra_host`
//
// Failing imports:
//   mnemra_host::mcp::server::MnemraMcpServer — the rmcp ServerHandler impl
//   mnemra_host::mcp::errors::AUTH_FAILURE_CODE — custom auth-failure error code
//   mnemra_host::mcp::errors::PERMISSION_DENIED_CODE — custom permission-denied code
//
// These are the INTENDED public symbols for the green phase (Forge's target).
// Removing these imports and providing real implementations is the green phase.
// ---------------------------------------------------------------------------
use mnemra_host::mcp::errors::{AUTH_FAILURE_CODE, PERMISSION_DENIED_CODE};
use mnemra_host::mcp::server::MnemraMcpServer;

/// Serializes engine startup across concurrent test threads within this binary (A-11).
static STARTUP_LOCK: Mutex<()> = Mutex::new(());

/// Start a fresh embedded engine with startup serialized (mirrors admin_token_behavior.rs).
async fn start_engine() -> EmbeddedEngine {
    {
        let _guard = STARTUP_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // Guard dropped here; async engine start races safely after this point.
    }
    EmbeddedEngine::start()
        .await
        .expect("failed to start embedded Postgres")
}

/// Seed an admin-role token into `admin_tokens` and return (token, token_id).
///
/// The token is stored hashed (BLAKE3); the raw `AdminToken` is returned for
/// presentation in the MCP `_meta.token` field (open seam #1).
async fn seed_admin_token(pool: &sqlx::PgPool, workspace_id: Uuid) -> (AdminToken, Uuid) {
    let token = generate();
    let token_hash = hash(&token);

    let (token_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO admin_tokens (token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(token_hash.as_bytes())
    .bind(workspace_id)
    .bind(&vec!["admin".to_owned()])
    .fetch_one(pool)
    .await
    .expect("INSERT admin token failed");

    (token, token_id)
}

/// Seed a read_observer-role token into `admin_tokens`.
async fn seed_read_observer_token(pool: &sqlx::PgPool, workspace_id: Uuid) -> (AdminToken, Uuid) {
    let token = generate();
    let token_hash = hash(&token);

    let (token_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO admin_tokens (token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(token_hash.as_bytes())
    .bind(workspace_id)
    .bind(&vec!["read_observer".to_owned()])
    .fetch_one(pool)
    .await
    .expect("INSERT read_observer token failed");

    (token, token_id)
}

/// Build a `Meta` carrying the auth token in the `token` key.
///
/// Open seam #1: MCP has no standard auth field. Per-request `_meta` is the
/// spec-faithful extension point: it is already defined in the MCP wire format,
/// requires no protocol extension, and is available on every `tools/call` and
/// `tools/list` request. The MCP handler reads `params.meta.get("token")` to
/// perform DF-auth-check (R-0010-c). Decision flagged for Puck validation.
fn token_meta(token_str: &str) -> Meta {
    let mut meta = Meta::new();
    meta.insert("token".to_owned(), json!(token_str));
    meta
}

/// Build a live `PluginPool` with the loaded `mnemra-echo` component registered
/// (R-0016-a). `MnemraMcpServer::new` now takes the plugin pool so `call_tool`
/// can dispatch to the typed `content` export. Tests whose request is rejected
/// pre-dispatch (auth / permission / tools-list) still need a constructed pool,
/// but never reach the invoke.
fn echo_plugin_pool() -> Arc<PluginPool> {
    let pool = PluginPool::new().expect("PluginPool::new");
    let component =
        Component::from_file(pool.engine(), echo_component_path()).expect("load echo component");
    pool.register_module(ECHO_PLUGIN_NAME, "0.0.1", &component)
        .expect("register echo component");
    Arc::new(pool)
}

/// Path to the built `mnemra-echo` component (`wasm32-wasip2`, release), produced
/// by `just plugin`. Resolved relative to the workspace target dir.
fn echo_component_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root from libs/mnemra-host");
    let path = root.join("target/wasm32-wasip2/release/mnemra_echo.wasm");
    assert!(
        path.exists(),
        "echo component not found at {} — run `just plugin` before the e2e tests",
        path.display()
    );
    path
}

/// Extract all text content strings from a `CallToolResult`.
///
/// `CallToolResult.content` is `Vec<Content>` where `Content = Annotated<RawContent>`;
/// text lives at `content.raw` → `RawContent::Text(RawTextContent { text, .. })`.
/// Used only for create-side id DISCOVERY (the returned ULID is a text item).
fn extract_text_content(result: &rmcp::model::CallToolResult) -> Vec<&str> {
    result
        .content
        .iter()
        .filter_map(|c| match &c.raw {
            RawContent::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect()
}

/// True iff `s` is a well-formed ULID: 26 chars, Crockford base32 (excludes I, L, O, U).
///
/// No `ulid`/`regex` crate (neither is in dev-deps; adding one trips the dep-gate).
fn is_valid_ulid(s: &str) -> bool {
    s.len() == 26
        && s.chars()
            .all(|c| "0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(c))
}

/// Create one `echo_fixture` artifact via `echo.create` under `token`, returning its
/// ULID id (discovered from the create result's text content).
///
/// `echo.create` is WIRED at this slice, so this helper succeeds — it exists so the
/// red-phase `echo.list` tests can capture concrete ids to assert on. A panic here is
/// a SETUP failure (wrong-reason red), and the message prints the actual create result
/// so the return shape can be reconciled.
async fn create_echo_fixture(
    client: &RunningService<RoleClient, ()>,
    token: &AdminToken,
    marker: &str,
) -> String {
    let mut params = CallToolRequestParams::new("echo.create");
    params.meta = Some(token_meta(token.as_str()));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert("payload".to_owned(), json!({ "msg": marker }));
        m
    });

    let result = client
        .call_tool(params)
        .await
        .expect("setup: echo.create is wired and must return Ok to seed list fixtures");

    let texts = extract_text_content(&result);
    texts
        .iter()
        .find(|t| is_valid_ulid(t.trim()))
        .map(|t| t.trim().to_owned())
        .unwrap_or_else(|| {
            panic!(
                "setup: echo.create result must carry a well-formed ULID id in text content; \
                 got text content: {texts:?}"
            )
        })
}

// ===========================================================================
// Test 1: Happy path — valid admin token, echo.create returns Ok
// ===========================================================================

/// R-0010-a/b/c/d, R-0006-b — valid admin token dispatches echo.create over MCP wire.
///
/// # Given / When / Then
///
/// GIVEN an MCP client connected over an in-memory duplex transport presenting
///   a valid admin token associated with workspace_id = DEFAULT_WORKSPACE_ID
/// WHEN the client sends a `tools/call` request for verb `echo.create` with
///   valid arguments over the real rmcp MCP wire
/// THEN the response is `Ok` — i.e. no JSON-RPC error is returned.
///
/// This exercises R-0010-a (single MCP server), R-0010-b (verb namespaced
/// `echo.create`), R-0010-c (auth-check before routing), R-0010-d (per-verb
/// capability check), and R-0006-b (WorkspaceCtx constructed at single site).
///
/// # Open seam #2 (verb choice)
///
/// `echo.create` is chosen over `task.create` because the V0 substrate has the
/// mnemra-echo reference plugin with `echo.create` in its `verbs.exposed` list
/// (manifest.toml). `task.create` does not exist at V0. `echo.create` is the
/// concretely-reachable WHERE-scoped-insert path. Flagged for Puck validation.
///
/// # Open seam #3 (metric emission)
///
/// Per-verb metric emission (R-0004) is a Task 25 concern and is not wired yet.
/// This test asserts only the Ok dispatch outcome, not OTel emission. Flagged.
#[tokio::test]
async fn valid_admin_token_echo_create_returns_ok() {
    // R-0010-a/b/c/d: valid admin token → echo.create dispatches successfully.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_admin_token(pool, workspace_id).await;

    // MnemraMcpServer does NOT exist yet — this is the valid red compile failure.
    // Green phase: Forge creates mnemra_host::mcp::server::MnemraMcpServer
    // implementing rmcp::ServerHandler, taking a PgPool and serving all loaded
    // plugin verbs namespaced as "<plugin>.<verb>" (R-0010-a/b).
    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());

    // Wire server and client over an in-process duplex transport.
    // tokio::io::duplex(4096) → (DuplexStream, DuplexStream).
    // DuplexStream: AsyncRead + AsyncWrite → satisfies IntoTransport blanket impl
    // (transport-async-rw, pulled in by rmcp's `server` feature).
    let (server_transport, client_transport) = duplex(4096);

    // Spawn server; drive client in-process.
    let server_handle = tokio::spawn(async move {
        // serve_server initialises the MCP handshake then runs the handler loop.
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });

    // () implements ClientHandler → Service<RoleClient>.
    // serve_client performs the MCP initialize handshake and returns RunningService.
    // RunningService<RoleClient, ()> derefs to Peer<RoleClient>.
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // WHEN — call tools/call for echo.create with a valid admin token in _meta.
    let mut params = CallToolRequestParams::new("echo.create");
    params.meta = Some(token_meta(token.as_str()));
    // Minimal valid arguments for echo.create (content_type + payload).
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert("payload".to_owned(), json!({"msg": "hello"}));
        m
    });

    // Peer::call_tool is available via Deref from RunningService.
    let result = client.call_tool(params).await;

    // THEN — response must be Ok (no JSON-RPC error).
    assert!(
        result.is_ok(),
        "R-0010-a/b/c/d: valid admin token echo.create must return Ok; \
         got error: {:?}",
        result.err()
    );

    // Shutdown.
    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 2: Bogus token → distinguishable auth-failure error code
// ===========================================================================

/// R-0010-c/f — bogus token string returns distinguishable auth-failure error code.
///
/// # Given / When / Then
///
/// GIVEN an MCP client presenting a token string T_BOGUS for which no matching
///   row exists in admin_tokens (BLAKE3(T_BOGUS) hash is not in the table)
/// WHEN the client sends a `tools/call` request for any verb
/// THEN the MCP handler returns a JSON-RPC error with:
///   - a code equal to AUTH_FAILURE_CODE (custom, not a standard JSON-RPC code)
///   - a code NOT equal to -32600 (invalid request)
///   - a code NOT equal to -32601 (method not found)
///   - a code NOT equal to -32602 (invalid params)
///   - a code NOT equal to -32603 (internal error)
///
/// No WorkspaceCtx is constructed; no host-fn is invoked (R-0010-c).
///
/// # Error code contract
///
/// AUTH_FAILURE_CODE is defined in `mnemra_host::mcp::errors`. Its value is
/// a custom i32 that does not overlap with any standard JSON-RPC code class.
/// Green phase: Forge defines `AUTH_FAILURE_CODE` and maps auth-check failures
/// to it in the MCP dispatch path (R-0010-f).
#[tokio::test]
async fn bogus_token_returns_distinguishable_auth_failure() {
    // R-0010-c/f: bogus token → auth-failure code, not a standard JSON-RPC code.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());

    let (server_transport, client_transport) = duplex(4096);

    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });

    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // WHEN — present a bogus token that is not in admin_tokens.
    let mut params = CallToolRequestParams::new("echo.create");
    params.meta = Some(token_meta("BOGUS_TOKEN_NOT_IN_DB_aaabbbcccdddeee"));

    let result = client.call_tool(params).await;

    // THEN — must be a JSON-RPC error with the auth-failure code.
    let err = result.expect_err("R-0010-c/f: bogus token must return Err (JSON-RPC error), not Ok");

    match err {
        rmcp::ServiceError::McpError(ref error_data) => {
            // Assert the code equals AUTH_FAILURE_CODE (R-0010-f).
            assert_eq!(
                error_data.code, AUTH_FAILURE_CODE,
                "R-0010-c/f: bogus token must return AUTH_FAILURE_CODE ({:?}), \
                 got {:?}",
                AUTH_FAILURE_CODE, error_data.code
            );

            // Assert code is not conflated with standard JSON-RPC codes (R-0010-f).
            assert_ne!(
                error_data.code,
                ErrorCode::INVALID_REQUEST,
                "R-0010-f: auth failure MUST NOT use INVALID_REQUEST (-32600)"
            );
            assert_ne!(
                error_data.code,
                ErrorCode::METHOD_NOT_FOUND,
                "R-0010-f: auth failure MUST NOT use METHOD_NOT_FOUND (-32601)"
            );
            assert_ne!(
                error_data.code,
                ErrorCode::INVALID_PARAMS,
                "R-0010-f: auth failure MUST NOT use INVALID_PARAMS (-32602)"
            );
            assert_ne!(
                error_data.code,
                ErrorCode::INTERNAL_ERROR,
                "R-0010-f: auth failure MUST NOT use INTERNAL_ERROR (-32603)"
            );
        }
        other => panic!(
            "R-0010-c/f: expected ServiceError::McpError, got {:?}",
            other
        ),
    }

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 3: Read-observer token attempting write verb → permission-denied code
// ===========================================================================

/// R-0010-d/f, R-0009-e — read-observer token denied on write verb.
///
/// # Given / When / Then
///
/// GIVEN an MCP client presenting a valid read_observer-scoped token
/// WHEN the client sends a `tools/call` request for `echo.create` (a write verb)
/// THEN the MCP handler:
///   - performs DF-auth-check (token resolves to a valid row → auth passes)
///   - constructs WorkspaceCtx with Role::ReadObserver
///   - enforces per-verb capability check: echo.create requires write capability
///   - returns a JSON-RPC error with PERMISSION_DENIED_CODE
///   - NO write is performed
///
/// PERMISSION_DENIED_CODE is distinct from AUTH_FAILURE_CODE:
///   auth failure = bad/missing token; permission denied = valid token, wrong role.
#[tokio::test]
async fn read_observer_write_denied_permission_error() {
    // R-0010-d/f, R-0009-e: read_observer + write verb → PERMISSION_DENIED_CODE.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_read_observer_token(pool, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());

    let (server_transport, client_transport) = duplex(4096);

    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });

    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // WHEN — read_observer presents a valid token but calls a write verb.
    let mut params = CallToolRequestParams::new("echo.create");
    params.meta = Some(token_meta(token.as_str()));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert(
            "payload".to_owned(),
            json!({"msg": "read_observer_attempt"}),
        );
        m
    });

    let result = client.call_tool(params).await;

    // THEN — must be a JSON-RPC error with PERMISSION_DENIED_CODE.
    let err = result
        .expect_err("R-0010-d/f, R-0009-e: read_observer write attempt must return Err, not Ok");

    match err {
        rmcp::ServiceError::McpError(ref error_data) => {
            // Assert PERMISSION_DENIED_CODE (R-0010-d/f, R-0009-e).
            assert_eq!(
                error_data.code, PERMISSION_DENIED_CODE,
                "R-0010-d/f, R-0009-e: read_observer on write verb must return \
                 PERMISSION_DENIED_CODE ({:?}), got {:?}",
                PERMISSION_DENIED_CODE, error_data.code
            );

            // Assert it is NOT the auth-failure code — distinct error classes (R-0010-f).
            assert_ne!(
                error_data.code, AUTH_FAILURE_CODE,
                "R-0009-e/R-0010-f: permission-denied MUST NOT use auth-failure code; \
                 auth failure = bad token; permission denied = valid token, wrong role"
            );
        }
        other => panic!(
            "R-0010-d/f, R-0009-e: expected ServiceError::McpError, got {:?}",
            other
        ),
    }

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 4: Control-plane verbs absent from tools/list (R-0010-g)
// ===========================================================================

/// R-0010-g — control-plane verbs are NOT exposed in the MCP tools list.
///
/// # Given / When / Then
///
/// GIVEN an MCP client connected over an in-memory transport
/// WHEN the client sends a `tools/list` request (all pages, via list_all_tools)
/// THEN the advertised tool name set contains NONE of the control-plane verbs:
///   "workspace create", "workspace.create", "workspace delete", "workspace.delete",
///   "token rotate", "token.rotate", "migrate", "backup"
///
/// Control-plane operations route through the admin CLI only (R-0010-g, R-0011-d);
/// they MUST NOT be exposed as MCP tools visible to agents.
///
/// # Assertion approach (structural, not substring)
///
/// Tool names are extracted as a `HashSet<&str>` and each forbidden name is
/// asserted absent. This is a structural check on the EXACT tool-name set —
/// NOT a bare substring scan over serialized output (which would over-match
/// any payload that legitimately mentions the word in a description).
///
/// # Sanity assertion
///
/// The echo plugin verbs (e.g. "echo.create") must also be present — if the
/// tool list is empty the absence assertion would be vacuously true.
#[tokio::test]
async fn control_plane_verbs_absent_from_tools_list() {
    // R-0010-g: control-plane verbs must not appear in tools/list response.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    // No token needed: tools/list is unauthenticated (R-0010-g only checks the
    // structural content of the advertised tool set, not auth behavior on list).
    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());

    let (server_transport, client_transport) = duplex(4096);

    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });

    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // WHEN — list all advertised MCP tools.
    // Note: list_all_tools handles pagination. If R-0010-c applies to list_tools
    // too (auth-check on every request), the server must accept the request even
    // without a per-request token on the list call — or the implementation must
    // accept the session-level token. This is recorded as an open seam: if the
    // server requires token auth on list_tools calls, the test may need to add a
    // token-bearing list_tools implementation. For now we assert the structural
    // tool-name content (R-0010-g), not the auth behavior on list.
    let tools = client
        .list_all_tools()
        .await
        .expect("R-0010-g: tools/list must succeed");

    // Extract the exact set of tool names.
    let tool_names: std::collections::HashSet<&str> =
        tools.iter().map(|t| t.name.as_ref()).collect();

    // Assert each forbidden control-plane verb is ABSENT (structural check).
    // Forbidden names cover both dot-namespaced and space-namespaced forms
    // to catch any accidentally-exposed variants.
    let forbidden_control_plane = [
        "workspace create",
        "workspace.create",
        "workspace delete",
        "workspace.delete",
        "token rotate",
        "token.rotate",
        "migrate",
        "backup",
    ];

    for forbidden in &forbidden_control_plane {
        assert!(
            !tool_names.contains(*forbidden),
            "R-0010-g: control-plane verb '{}' MUST NOT appear in tools/list; \
             control-plane operations route through admin CLI only (R-0010-g). \
             Advertised tools: {:?}",
            forbidden,
            tool_names
        );
    }

    // Sanity: the tool list must not be empty (vacuous-true guard).
    // At least one echo plugin verb must be present since the echo plugin is loaded.
    let echo_verbs_present = tool_names.iter().any(|n| n.starts_with("echo."));
    assert!(
        echo_verbs_present,
        "R-0010-g sanity: at least one echo.* verb must appear in tools/list; \
         if the list is empty, the control-plane-absent assertion is vacuously true. \
         Advertised tools: {:?}",
        tool_names
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 5 (RED): ReadObserver + non-read verb tail → permission-denied
//   is_write_verb fail-closed (#1728 slice 1)
// ===========================================================================

/// R-0009-d/e — ReadObserver dispatching a verb whose tail is not a known read
/// must receive PERMISSION_DENIED_CODE (fail-closed).
///
/// # The defect being closed
///
/// `mcp/dispatch.rs:is_write_verb` currently uses an EXPLICIT WRITE ALLOWLIST
/// (`create | update | delete`).  Every OTHER tail — including `audit` — falls
/// through to `PluginReadVerb`, granting ReadObserver access.  This is a
/// fail-OPEN default that violates R-0009-d.
///
/// The fix (Forge's green phase) inverts the logic to a fail-CLOSED READ
/// ALLOWLIST: only `get` and `list` are classified as reads; everything else
/// — including unknown tails such as `audit` — is treated as a write and
/// denied to ReadObserver.
///
/// # Given / When / Then
///
/// GIVEN a valid read_observer-scoped token seeded in admin_tokens
/// WHEN the client sends a `tools/call` request for `echo.audit`
///   (tail = "audit", NOT in {get, list})
/// THEN the MCP handler returns a JSON-RPC error with PERMISSION_DENIED_CODE —
///   the same code `read_observer_write_denied_permission_error` asserts for
///   `echo.create` — and NOT an Ok response.
///
/// # Right-reason red
///
/// Against CURRENT code: `is_write_verb("echo.audit")` returns false (tail
/// "audit" is not in {create, update, delete}), so `echo.audit` is classified
/// as `PluginReadVerb` and ReadObserver is permitted.  The test's
/// `assert_eq!(code, PERMISSION_DENIED_CODE)` therefore FAILS today for the
/// correct behavioral reason: the permission check PASSES when it should DENY.
///
/// After Forge's green phase: `is_write_verb` returns true for "audit" (not in
/// read allowlist {get, list}), ReadObserver is denied, and this test passes.
///
/// # verify: []
///
/// The dispatch envelope carries `verify: []` by design.  This test fails
/// against current code (right-reason red); `verify: []` is correct because
/// the recipe runs only after the green phase lands.
#[tokio::test]
async fn read_observer_non_read_verb_denied_permission_error() {
    // R-0009-d/e: ReadObserver + non-read verb tail → PERMISSION_DENIED_CODE.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_read_observer_token(pool, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());

    let (server_transport, client_transport) = duplex(4096);

    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });

    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // WHEN — read_observer presents a valid token and calls echo.audit.
    // echo.audit is in the manifest's exposed list (tail = "audit") but its
    // tail is NOT in the read allowlist {get, list} — it MUST be denied.
    let mut params = CallToolRequestParams::new("echo.audit");
    params.meta = Some(token_meta(token.as_str()));
    // No arguments needed: the permission check runs before dispatch.

    let result = client.call_tool(params).await;

    // THEN — must be a JSON-RPC error with PERMISSION_DENIED_CODE.
    // (Mirrors read_observer_write_denied_permission_error shape exactly.)
    let err = result.expect_err(
        "R-0009-d/e: read_observer on echo.audit (non-read tail) must return Err, not Ok; \
         current is_write_verb fails open — this is the right-reason red",
    );

    match err {
        rmcp::ServiceError::McpError(ref error_data) => {
            // Assert PERMISSION_DENIED_CODE — not merely any error (R-0009-d/e).
            assert_eq!(
                error_data.code, PERMISSION_DENIED_CODE,
                "R-0009-d/e: ReadObserver on non-read verb echo.audit must return \
                 PERMISSION_DENIED_CODE ({:?}), got {:?}",
                PERMISSION_DENIED_CODE, error_data.code
            );

            // Assert it is NOT the auth-failure code — token is valid (R-0010-f).
            assert_ne!(
                error_data.code, AUTH_FAILURE_CODE,
                "R-0009-d/e: echo.audit denial must be a permission error, not an \
                 auth failure — the token is valid; only the role capability is wrong"
            );
        }
        other => panic!(
            "R-0009-d/e: expected ServiceError::McpError for echo.audit denial, got {:?}",
            other
        ),
    }

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 6 (regression guard): ReadObserver + read verb → NOT denied
//   Pins the read-allowlist boundary so Forge's fix cannot over-deny.
// ===========================================================================

/// R-0009-d — ReadObserver dispatching `echo.get` (a known read verb) must NOT
/// receive PERMISSION_DENIED_CODE.
///
/// # Guard intent
///
/// The fail-closed fix inverts `is_write_verb` to an explicit read allowlist
/// ({get, list} → read; everything else → write).  This guard pins the
/// boundary so the inversion cannot accidentally deny legitimate read verbs.
///
/// # Given / When / Then
///
/// GIVEN a valid read_observer-scoped token
/// WHEN the client sends a `tools/call` request for `echo.get`
///   (tail = "get", in the read allowlist)
/// THEN the result does NOT carry PERMISSION_DENIED_CODE —
///   the permission check passes and the call reaches dispatch.
///
/// # Why NOT assert is_ok()
///
/// `echo.get` with no prior `echo.create` may legitimately return a not-found
/// or internal error from the plugin layer — that is a post-permission dispatch
/// outcome, not a permission failure.  We assert only that the code is NOT
/// PERMISSION_DENIED_CODE, which is the invariant R-0009-d guarantees for a
/// read verb.  This test PASSES today (get is currently allowed) and MUST
/// continue to pass after the green phase.
///
/// # verify: []
///
/// This guard passes today and after the fix.  The `verify: []` envelope
/// reflects that the PRIMARY red (#5) fails; this guard is the boundary pin.
#[tokio::test]
async fn read_observer_get_verb_not_denied() {
    // R-0009-d: ReadObserver + echo.get → permission check passes (not denied).
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_read_observer_token(pool, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());

    let (server_transport, client_transport) = duplex(4096);

    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });

    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // WHEN — read_observer presents a valid token and calls echo.get.
    // echo.get tail = "get" is in the read allowlist; ReadObserver MUST be permitted.
    let mut params = CallToolRequestParams::new("echo.get");
    params.meta = Some(token_meta(token.as_str()));
    // No artifact ID argument: any dispatch-layer error is post-permission and fine.

    let result = client.call_tool(params).await;

    // THEN — the result must NOT be PERMISSION_DENIED_CODE.
    // Any other outcome (Ok, not-found error, internal error) is acceptable here
    // because it means the permission check PASSED and dispatch was attempted.
    match &result {
        Err(rmcp::ServiceError::McpError(error_data)) => {
            assert_ne!(
                error_data.code, PERMISSION_DENIED_CODE,
                "R-0009-d: ReadObserver on echo.get (read verb) MUST NOT receive \
                 PERMISSION_DENIED_CODE; got {:?}. \
                 The fail-closed fix must not over-deny the read allowlist.",
                error_data.code
            );
            // Also guard: not an auth failure (token is valid).
            assert_ne!(
                error_data.code, AUTH_FAILURE_CODE,
                "R-0009-d: ReadObserver echo.get must not return auth failure; \
                 the token is valid"
            );
        }
        Ok(_) | Err(_) => {
            // Ok (dispatch succeeded) or a non-McpError transport error:
            // both mean the permission check passed.  Guard holds.
        }
    }

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 7 (RED): echo.list dispatches and returns created ids (T12 list slice)
// ===========================================================================

/// R-0019-c — `echo.list` dispatches to `content.list` and returns the ids of the
/// caller workspace's artifacts of the given type.
///
/// # Given / When / Then
///
/// GIVEN an admin token in DEFAULT_WORKSPACE_ID with TWO `echo_fixture` artifacts
///   created via `echo.create` (both ids captured)
/// WHEN the client calls `echo.list` for content_type "echo_fixture" with empty `{}` filters
/// THEN the call succeeds AND the returned list contains BOTH captured ids
///   (asserted by exact-id presence — NOT merely "Ok" or "list is non-empty").
///
/// # Red reason
///
/// `echo.list` is UNWIRED at this slice — `mcp/dispatch.rs` rejects "list"
/// (NON_DISPATCHABLE, "verb 'list' is not wired at slice 1"). The `.expect` on the
/// list call therefore panics with that error: right-reason red. The id-presence
/// assertions are the green contract (Forge wires `echo.list -> content.list`).
///
/// # Reconciliation point R-list-args (for Forge)
///
/// The `filters` argument key + `"{}"`-string value map to the WIT signature
/// `content.list(type_name, filters) -> list<string>` (filters is a JSON string,
/// `"{}"` = no filter). The manifest declares the verb but not its per-arg schema;
/// Forge must confirm the `filters` key against the echo verb arg mapping.
#[tokio::test]
async fn valid_admin_token_echo_list_returns_created_ids() {
    // R-0019-c: admin echo.list returns the ids it created.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_admin_token(pool, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // Seed two echo_fixture artifacts; capture both ids (echo.create is wired).
    let id_one = create_echo_fixture(&client, &token, "list_fixture_one").await;
    let id_two = create_echo_fixture(&client, &token, "list_fixture_two").await;

    // WHEN — call echo.list for the echo_fixture type with empty filters.
    let mut list_params = CallToolRequestParams::new("echo.list");
    list_params.meta = Some(token_meta(token.as_str()));
    list_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert("filters".to_owned(), json!("{}"));
        m
    });

    let list_result = client.call_tool(list_params).await.expect(
        "R-0019-c: echo.list must return Ok and dispatch to content.list; \
         red phase: panics because echo.list is unwired (NON_DISPATCHABLE)",
    );

    // THEN — the returned list must contain BOTH created ids.
    // Search the serialized result body so the assertion is robust to the green
    // encoding of `list<string>` (separate text items vs JSON array vs structured
    // content) while remaining strong: a specific 26-char ULID present is no
    // vacuous green.
    let body = serde_json::to_string(&list_result).expect("serialize echo.list result");
    assert!(
        body.contains(&id_one),
        "R-0019-c: echo.list result must contain created id {id_one}; got body: {body}"
    );
    assert!(
        body.contains(&id_two),
        "R-0019-c: echo.list result must contain created id {id_two}; got body: {body}"
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 8 (RED): echo.list is workspace-scoped — tenant isolation
// ===========================================================================

/// R-0006-d — `echo.list` is workspace-scoped: it returns the caller workspace's
/// artifacts and MUST NOT leak artifacts from another workspace.
///
/// # Given / When / Then
///
/// GIVEN one `echo_fixture` created in workspace A AND a different `echo_fixture`
///   created in workspace B (each a fresh Uuid with its own admin token)
/// WHEN `echo.list` is called scoped to workspace A's token
/// THEN the result contains id_A AND does NOT contain id_B.
///
/// # Red reason
///
/// `echo.list` is unwired → NON_DISPATCHABLE; the `.expect` panics for the right
/// reason. The isolation assertions are the green contract.
///
/// # Security note
///
/// This is THE tenant-isolation assertion for `list` (cross-workspace artifacts
/// must not leak). Do not weaken to "Ok" or "id_A present" alone — the id_B-ABSENT
/// half is the security guarantee.
///
/// # Workspace seeding
///
/// `admin_tokens.workspace_id` is UUID NOT NULL with no FK to a `workspaces` row
/// (schema init migration 7). A token for a fresh Uuid4 is valid without a
/// `workspaces` row — same approach as `mcp_slice1_e2e::cross_workspace_get_returns_none`.
#[tokio::test]
async fn echo_list_is_workspace_scoped() {
    // R-0006-d: echo.list does not leak cross-workspace artifacts.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_a = Uuid::new_v4();
    let workspace_b = Uuid::new_v4();
    let (token_a, _a_id) = seed_admin_token(pool, workspace_a).await;
    let (token_b, _b_id) = seed_admin_token(pool, workspace_b).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // Create one artifact in each workspace; capture both ids.
    let id_a = create_echo_fixture(&client, &token_a, "workspace_a_fixture").await;
    let id_b = create_echo_fixture(&client, &token_b, "workspace_b_fixture").await;

    // WHEN — list under workspace A's token.
    let mut list_params = CallToolRequestParams::new("echo.list");
    list_params.meta = Some(token_meta(token_a.as_str()));
    list_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert("filters".to_owned(), json!("{}"));
        m
    });

    let list_result = client.call_tool(list_params).await.expect(
        "R-0006-d: echo.list under workspace A must return Ok; \
         red phase: panics because echo.list is unwired (NON_DISPATCHABLE)",
    );

    // THEN — workspace A's id present; workspace B's id MUST NOT leak.
    let body = serde_json::to_string(&list_result).expect("serialize echo.list result");
    assert!(
        body.contains(&id_a),
        "R-0006-d: workspace A's echo.list must contain its own id {id_a}; got body: {body}"
    );
    assert!(
        !body.contains(&id_b),
        "R-0006-d: tenant isolation violated — workspace A's echo.list leaked \
         workspace B's id {id_b}; got body: {body}"
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 9 (RED): echo.list is reachable by a ReadObserver (list is a read verb)
// ===========================================================================

/// R-0009-d — `echo.list` is a READ verb: a ReadObserver token may list.
///
/// # Given / When / Then
///
/// GIVEN an `echo_fixture` created by an admin token, and a read_observer token in
///   the SAME workspace
/// WHEN `echo.list` is called under the read_observer token
/// THEN the call is NOT denied for role reasons AND returns the created id
///   (a ReadObserver may read; update/delete reachability is deliberately NOT asserted).
///
/// # Red reason
///
/// `list` is classified as a read verb, so the read_observer permission check PASSES
/// and the request reaches dispatch, where `echo.list` is unwired → NON_DISPATCHABLE.
/// The `.expect` therefore panics with the "not wired" error (NOT a permission error):
/// right-reason red. If this instead surfaced PERMISSION_DENIED, current code is
/// mis-classifying `list` as a write — that is a FINDING, not a valid red.
#[tokio::test]
async fn echo_list_reachable_by_read_observer() {
    // R-0009-d: a ReadObserver may echo.list (read verb), not denied.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (admin_token, _admin_id) = seed_admin_token(pool, workspace_id).await;
    let (observer_token, _obs_id) = seed_read_observer_token(pool, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // An admin token seeds the fixture; the read_observer then lists it.
    let id = create_echo_fixture(&client, &admin_token, "read_observer_visible_fixture").await;

    // WHEN — list under the read_observer token.
    let mut list_params = CallToolRequestParams::new("echo.list");
    list_params.meta = Some(token_meta(observer_token.as_str()));
    list_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert("filters".to_owned(), json!("{}"));
        m
    });

    let list_result = client.call_tool(list_params).await.expect(
        "R-0009-d: read_observer echo.list (a read verb) must NOT be denied and must \
         return Ok; red phase: panics because echo.list is unwired (NON_DISPATCHABLE), \
         NOT because of a permission error",
    );

    // THEN — a ReadObserver may list: the created id is returned.
    let body = serde_json::to_string(&list_result).expect("serialize echo.list result");
    assert!(
        body.contains(&id),
        "R-0009-d: read_observer echo.list must return the created id {id}; got body: {body}"
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 10 (RED): echo.update merges frontmatter + replaces body, persists
//   (T12 update slice — R-0019-c happy path, STRONG)
// ===========================================================================

/// R-0019-c — `echo.update` dispatches to `content.update`, MERGES the
/// frontmatter-patch into the existing frontmatter (patch keys overwrite/add;
/// untouched keys are preserved), replaces the body, and persists — observable
/// via a follow-on `echo.get`.
///
/// # Given / When / Then
///
/// GIVEN an admin token and an `echo_fixture` created via `echo.create` with a
///   two-field frontmatter payload `{"title": <orig>, "keep": <kept>}` (id captured)
/// WHEN `echo.update` is called with that id, `frontmatter_patch` = `{"title": <new>}`
///   (a JSON string), and `body` = Some(<new body>)
/// THEN the update call succeeds AND a subsequent `echo.get` of that id shows:
///   - the patched value `<new>` present (the patch was applied),
///   - the untouched field value `<kept>` STILL present (MERGE, not replace),
///   - the new body present (the body was replaced),
///   - and the old `<orig>` value ABSENT (the field was overwritten, not duplicated).
///
/// # Assertion strength
///
/// Asserting `is_ok()` alone would green vacuously. The two-field payload makes
/// MERGE observable: a replace-semantics (wrong) implementation would drop the
/// untouched `keep` field, failing the `survives` assertion; a no-op (wrong)
/// implementation would fail the `newtitle` / `newbody` assertions. Distinctive
/// marker values (`*_t12u` suffix) make the substring checks robust against
/// accidental collisions in the serialized result envelope.
///
/// # Red reason
///
/// `echo.update` is UNWIRED at this slice — `mcp/dispatch.rs` rejects the verb
/// with the structured non-dispatchable error (-4003 NON_DISPATCHABLE,
/// "verb 'echo.update' is not wired at slice 1"). The `.expect` on the update
/// call therefore panics with that error: right-reason red. (A -4005
/// VERB_NOT_EXPOSED here would be a manifest gap, NOT a valid red — see report.)
/// The merge/persist assertions are the green contract (Forge wires
/// `echo.update -> content.update`).
///
/// # Reconciliation point R-update-args (for Forge)
///
/// MCP arg keys are pinned by the T12 dispatch: `id`, `frontmatter_patch`
/// (a JSON string — mirroring the `json`=string WIT type and the `filters`
/// precedent in the list tests), and optional `body`. NOTE the asymmetry:
/// `echo.create` passes its `payload`→`frontmatter` arg as a JSON OBJECT
/// (`json!({...})`), whereas this test passes `frontmatter_patch` as a JSON
/// STRING per the pin. Forge must confirm the host accepts the JSON-string form
/// for `frontmatter_patch` (or reconcile to the object form create uses).
///
/// # verify: []
///
/// `verify: []` by design — this test reds today (echo.update unwired); the
/// just recipe runs after the green phase lands.
#[tokio::test]
async fn valid_admin_token_echo_update_merges_and_persists() {
    // R-0019-c: admin echo.update merges frontmatter, replaces body, persists.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_admin_token(pool, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // Create an echo_fixture with a TWO-FIELD frontmatter so MERGE is observable.
    // (Inline rather than via create_echo_fixture, which only writes one field.)
    let mut create_params = CallToolRequestParams::new("echo.create");
    create_params.meta = Some(token_meta(token.as_str()));
    create_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert(
            "payload".to_owned(),
            json!({"title": "origtitle_t12u", "keep": "keepfield_t12u"}),
        );
        m
    });
    let create_result = client
        .call_tool(create_params)
        .await
        .expect("setup: echo.create (wired) must return Ok to seed the update fixture");
    let texts = extract_text_content(&create_result);
    let id = texts
        .iter()
        .find(|t| is_valid_ulid(t.trim()))
        .map(|t| t.trim().to_owned())
        .unwrap_or_else(|| {
            panic!("setup: echo.create must return a well-formed ULID id; got: {texts:?}")
        });

    // WHEN — echo.update with a frontmatter patch (JSON string) + a replacement body.
    // frontmatter_patch overwrites `title` only; `keep` must survive the merge.
    let patch = json!({"title": "newtitle_t12u"}).to_string();
    let mut update_params = CallToolRequestParams::new("echo.update");
    update_params.meta = Some(token_meta(token.as_str()));
    update_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(id));
        m.insert("frontmatter_patch".to_owned(), json!(patch));
        m.insert("body".to_owned(), json!("newbody_t12u"));
        m
    });

    // The dispatch-success assertion: echo.update must return Ok. While the verb
    // is unwired this panics with -4003 NON_DISPATCHABLE — the right-reason red.
    let _update_result = client.call_tool(update_params).await.expect(
        "R-0019-c: echo.update must return Ok and dispatch to content.update; \
         red phase: panics because echo.update is unwired (NON_DISPATCHABLE -4003)",
    );

    // THEN — read back via echo.get and assert the MERGE + body replacement.
    let mut get_params = CallToolRequestParams::new("echo.get");
    get_params.meta = Some(token_meta(token.as_str()));
    get_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(id));
        m
    });
    let get_result = client
        .call_tool(get_params)
        .await
        .expect("R-0019-c: echo.get of the updated id must return Ok");

    // Search the serialized get result — robust to the green encoding of the
    // artifact (text item / structured content) while remaining strong on values.
    let body = serde_json::to_string(&get_result).expect("serialize echo.get result");
    assert!(
        body.contains("newtitle_t12u"),
        "R-0019-c: echo.update must apply the patch — patched title 'newtitle_t12u' \
         must be present after update; got body: {body}"
    );
    assert!(
        body.contains("keepfield_t12u"),
        "R-0019-c: echo.update must MERGE (not replace) — the untouched field value \
         'keepfield_t12u' must survive the patch; got body: {body}"
    );
    assert!(
        body.contains("newbody_t12u"),
        "R-0019-c: echo.update must replace the body — 'newbody_t12u' must be present; \
         got body: {body}"
    );
    assert!(
        !body.contains("origtitle_t12u"),
        "R-0019-c: echo.update must OVERWRITE the patched field — the old title \
         'origtitle_t12u' must be absent after the patch; got body: {body}"
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 11 (RED): echo.update cannot modify another workspace's artifact
//   (T12 update slice — R-0006-d tenant WRITE isolation — THE security assertion)
// ===========================================================================

/// R-0006-d — `echo.update` is workspace-scoped on WRITE: a token in workspace B
/// MUST NOT modify an artifact owned by workspace A. The fenced `(workspace_id, id)`
/// lookup misses under B, so the update is a silent no-op that returns success;
/// A's artifact is left untouched.
///
/// # Given / When / Then
///
/// GIVEN an `echo_fixture` created in workspace A (fresh Uuid, admin token) with a
///   known payload (id_A captured), and a separate admin token for a different
///   fresh Uuid workspace B
/// WHEN workspace B's token calls `echo.update` on id_A with a `frontmatter_patch`
///   that WOULD change a field (`{"title": "hijacked_t12u"}`)
/// THEN BOTH hold:
///   (a) the update call from B DISPATCHES successfully (Ok — a no-op success,
///       NOT an error), AND
///   (b) an `echo.get` of id_A under WORKSPACE A's token still returns the ORIGINAL
///       content — the `hijacked_t12u` value is ABSENT (B could not modify A).
///
/// # Why the dispatch-success half is required (right-reason red)
///
/// While `echo.update` is unwired it returns -4003 NON_DISPATCHABLE, so the
/// explicit `.expect` on B's update call panics → the test reds for the right
/// reason ("update not wired"). WITHOUT asserting dispatch-success, an unwired
/// update would no-op and leave A unchanged, so the isolation half (b) alone
/// would pass VACUOUSLY at red. The (a) half is therefore load-bearing for a
/// non-vacuous red. (A -4005 VERB_NOT_EXPOSED here is a manifest gap, not a
/// valid red — see report.)
///
/// # Security note
///
/// The security guarantee is carried by the `!contains("hijacked_t12u")` half.
/// The `contains(<original marker>)` half is a vacuity guard only (B's patch
/// touches `title`, so the original `msg` field survives regardless) — do not
/// read it as the isolation assertion.
///
/// # Workspace seeding
///
/// `admin_tokens.workspace_id` is UUID NOT NULL with no FK to a `workspaces` row
/// (schema init migration 7) — a token for a fresh Uuid4 is valid without a
/// `workspaces` row (same approach as `cross_workspace_get_returns_none` and
/// `echo_list_is_workspace_scoped`).
///
/// # verify: []
///
/// `verify: []` by design — this test reds today (echo.update unwired).
#[tokio::test]
async fn echo_update_cannot_modify_another_workspace_artifact() {
    // R-0006-d: echo.update is tenant-isolated on write — B cannot modify A.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_a = Uuid::new_v4();
    let workspace_b = Uuid::new_v4();
    let (token_a, _a_id) = seed_admin_token(pool, workspace_a).await;
    let (token_b, _b_id) = seed_admin_token(pool, workspace_b).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // Create an artifact in workspace A with a known, distinctive marker.
    let id_a = create_echo_fixture(&client, &token_a, "origmsg_ws_a_t12u").await;

    // WHEN — workspace B's token attempts to update workspace A's artifact.
    let patch = json!({"title": "hijacked_t12u"}).to_string();
    let mut update_params = CallToolRequestParams::new("echo.update");
    update_params.meta = Some(token_meta(token_b.as_str()));
    update_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(id_a));
        m.insert("frontmatter_patch".to_owned(), json!(patch));
        m
    });

    // (a) dispatch-success MUST hold explicitly — a cross-workspace update is a
    // no-op that returns Ok (the fenced (workspace_id, id) lookup misses, nothing
    // is modified). While the verb is unwired this panics with -4003
    // NON_DISPATCHABLE — the right-reason red. This half is required so the
    // isolation half below cannot pass vacuously at red.
    let _b_update = client.call_tool(update_params).await.expect(
        "R-0006-d: cross-workspace echo.update must DISPATCH successfully (no-op success, \
         NOT an error); red phase: panics because echo.update is unwired \
         (NON_DISPATCHABLE -4003) — this is the right-reason red",
    );

    // (b) THE security assertion: read id_A back UNDER WORKSPACE A's token; B's
    // patch must NOT have landed on A's artifact.
    let mut get_params = CallToolRequestParams::new("echo.get");
    get_params.meta = Some(token_meta(token_a.as_str()));
    get_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(id_a));
        m
    });
    let get_result = client
        .call_tool(get_params)
        .await
        .expect("R-0006-d: echo.get of id_A under workspace A's token must return Ok");

    let body = serde_json::to_string(&get_result).expect("serialize echo.get result");
    assert!(
        !body.contains("hijacked_t12u"),
        "R-0006-d: tenant WRITE isolation violated — workspace B modified workspace A's \
         artifact (the 'hijacked_t12u' patch value leaked into A); got body: {body}"
    );
    // Vacuity guard only (NOT the security assertion): A's original content survives.
    assert!(
        body.contains("origmsg_ws_a_t12u"),
        "R-0006-d: workspace A's original content must survive a cross-workspace update \
         attempt; expected original marker 'origmsg_ws_a_t12u' present; got body: {body}"
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 12 (RED): echo.update with body absent (body=None) preserves the existing
//   body (T12 update slice — R-0019-c, body=None branch)
// ===========================================================================

/// R-0019-c — `echo.update` with NO `body` argument (WIT `body: option<string>` =
/// None) must LEAVE the artifact's existing body unchanged — it must NOT clear or
/// empty the body. This pins the `body=None` branch so green cannot ship a
/// body-always-overwritten implementation as a silent green.
///
/// # Establishing a non-empty body (why via update, not create)
///
/// At slice 1, `echo.create` maps `{content_type, payload}` → `content.create(type,
/// frontmatter, body=None)` (CC-MAPPING, plan line 64) — create CANNOT set a body.
/// So the known body is established the same way Test 10 establishes one: a first
/// `echo.update` carrying `body = Some("origbody_t12u")`. A second `echo.update`
/// then patches frontmatter only (no `body` arg), and the original body must
/// survive. This also models the real scenario the branch protects: a
/// metadata-only update must not wipe a body written by an earlier write.
///
/// # Given / When / Then
///
/// GIVEN an admin token; an `echo_fixture` whose body has been set to "origbody_t12u"
///   (create → first `echo.update` with body=Some("origbody_t12u")); id captured
/// WHEN `echo.update` is called with that id and a `frontmatter_patch`
///   `{"title":"patched_nobody_t12u"}` and the `body` key OMITTED entirely
///   (absent `body` is how the MCP layer signals `body=None`)
/// THEN the call succeeds AND a subsequent `echo.get` shows BOTH: the patched
///   frontmatter value `patched_nobody_t12u` present, AND the original body
///   `origbody_t12u` STILL present (absent body left the existing body unchanged,
///   not cleared/emptied).
///
/// # Red reason
///
/// `echo.update` is UNWIRED at this slice — the FIRST update (which establishes the
/// body) reds at the update call with -4003 NON_DISPATCHABLE ("verb 'echo.update'
/// is not wired at slice 1"), the same right-reason red as Tests 10 and 11. The
/// body-preservation assertions are the green contract.
///
/// # verify: []
///
/// `verify: []` by design — this test reds today (echo.update unwired).
#[tokio::test]
async fn echo_update_absent_body_preserves_existing_body() {
    // R-0019-c: echo.update with body absent (None) preserves the existing body.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_admin_token(pool, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // Create an echo_fixture (body=None at create per CC-MAPPING).
    let mut create_params = CallToolRequestParams::new("echo.create");
    create_params.meta = Some(token_meta(token.as_str()));
    create_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert("payload".to_owned(), json!({"title": "orig_t12u"}));
        m
    });
    let create_result = client
        .call_tool(create_params)
        .await
        .expect("setup: echo.create (wired) must return Ok to seed the body fixture");
    let texts = extract_text_content(&create_result);
    let id = texts
        .iter()
        .find(|t| is_valid_ulid(t.trim()))
        .map(|t| t.trim().to_owned())
        .unwrap_or_else(|| {
            panic!("setup: echo.create must return a well-formed ULID id; got: {texts:?}")
        });

    // Establish a known, non-empty body via a first echo.update (mirrors Test 10).
    // While echo.update is unwired this reds HERE with -4003 — the right-reason red.
    let seed_patch = json!({"seeded": "yes_t12u"}).to_string();
    let mut seed_body_params = CallToolRequestParams::new("echo.update");
    seed_body_params.meta = Some(token_meta(token.as_str()));
    seed_body_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(id));
        m.insert("frontmatter_patch".to_owned(), json!(seed_patch));
        m.insert("body".to_owned(), json!("origbody_t12u"));
        m
    });
    let _seed_result = client.call_tool(seed_body_params).await.expect(
        "R-0019-c: echo.update (establishing the body) must return Ok and dispatch to \
         content.update; red phase: panics because echo.update is unwired \
         (NON_DISPATCHABLE -4003) — same right-reason red as Tests 10/11",
    );

    // WHEN — echo.update patches frontmatter only, with the `body` key OMITTED
    // (absent body signals body=None: the existing body must be left unchanged).
    let patch = json!({"title": "patched_nobody_t12u"}).to_string();
    let mut update_params = CallToolRequestParams::new("echo.update");
    update_params.meta = Some(token_meta(token.as_str()));
    update_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(id));
        m.insert("frontmatter_patch".to_owned(), json!(patch));
        // NOTE: no `body` key inserted — this is how the MCP layer signals body=None.
        m
    });
    let _update_result = client.call_tool(update_params).await.expect(
        "R-0019-c: echo.update with body absent must return Ok; red phase: \
         panics because echo.update is unwired (NON_DISPATCHABLE -4003)",
    );

    // THEN — read back: patched frontmatter present AND original body preserved.
    let mut get_params = CallToolRequestParams::new("echo.get");
    get_params.meta = Some(token_meta(token.as_str()));
    get_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(id));
        m
    });
    let get_result = client
        .call_tool(get_params)
        .await
        .expect("R-0019-c: echo.get of the updated id must return Ok");

    let body = serde_json::to_string(&get_result).expect("serialize echo.get result");
    assert!(
        body.contains("patched_nobody_t12u"),
        "R-0019-c: echo.update must apply the frontmatter patch even when body is absent — \
         'patched_nobody_t12u' must be present; got body: {body}"
    );
    assert!(
        body.contains("origbody_t12u"),
        "R-0019-c: echo.update with body absent (body=None) must PRESERVE the existing body — \
         'origbody_t12u' must STILL be present (not cleared/emptied); got body: {body}"
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 13 (RED): echo.delete removes the artifact (T12 delete-slice)
//   (R-0019-c happy path, STRONG observable outcome)
// ===========================================================================

/// R-0019-c — `echo.delete` dispatches to `content.delete` and removes the
/// artifact from the caller's workspace — observable via a subsequent `echo.get`
/// that returns absent/None.
///
/// # Given / When / Then
///
/// GIVEN an admin token for DEFAULT_WORKSPACE_ID with one `echo_fixture` created
///   via `echo.create` (id captured, marker "todelete_t12d")
/// WHEN the admin calls `echo.delete` with that id
/// THEN the delete call succeeds AND a subsequent `echo.get` for that id returns
///   absent (the artifact marker "todelete_t12d" is no longer retrievable).
///
/// # Assertion strength
///
/// Asserting `is_ok()` on delete alone would green vacuously. The post-delete
/// `echo.get` assertion pins the OBSERVABLE OUTCOME: the artifact must actually
/// be removed, not merely "delete returned Ok". The marker string `todelete_t12d`
/// in the create payload makes the absent assertion robust against accidental
/// matches. When wired, `echo.delete` returns an empty success
/// (`CallToolResult::success(vec![])`); the green contract is asserted via the
/// absence of the marker in the subsequent get, not via the delete result payload.
///
/// # Right-reason red
///
/// `echo.delete` is UNWIRED at this slice — `mcp/dispatch.rs` rejects "delete"
/// with the structured non-dispatchable error (-4003 NON_DISPATCHABLE,
/// "verb 'echo.delete' is not wired at slice 1 (CC-MAPPING T12)"). The
/// `.expect` on the delete call therefore panics with that error: right-reason
/// red. (A -4005 VERB_NOT_EXPOSED here would be a manifest gap, NOT a valid red.)
/// The post-delete absent assertion is the green contract (Forge wires
/// `echo.delete -> content.delete`).
///
/// # verify: []
///
/// `verify: []` by design — this test reds today (echo.delete unwired); the
/// just recipe runs after the green phase lands.
#[tokio::test]
async fn valid_admin_token_echo_delete_removes() {
    // R-0019-c: admin echo.delete removes the artifact; verified via echo.get absent.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_admin_token(pool, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // Create an echo_fixture with a distinctive marker; capture its id.
    // echo.create is wired — a panic here is a SETUP failure (wrong-reason red).
    let id = create_echo_fixture(&client, &token, "todelete_t12d").await;

    // WHEN — call echo.delete with the captured id.
    let mut delete_params = CallToolRequestParams::new("echo.delete");
    delete_params.meta = Some(token_meta(token.as_str()));
    delete_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(id));
        m
    });

    // The dispatch-success assertion: echo.delete must return Ok. While the verb
    // is unwired this panics with -4003 NON_DISPATCHABLE — the right-reason red.
    let _delete_result = client.call_tool(delete_params).await.expect(
        "R-0019-c: echo.delete must return Ok and dispatch to content.delete; \
         red phase: panics because echo.delete is unwired (NON_DISPATCHABLE -4003)",
    );

    // THEN — the artifact must no longer be retrievable via echo.get.
    // After a successful delete, echo.get for the removed id returns absent/None:
    // either an error response (not-found) or an Ok with empty content.
    // In either case, the original marker must NOT appear in the result.
    let mut get_params = CallToolRequestParams::new("echo.get");
    get_params.meta = Some(token_meta(token.as_str()));
    get_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(id));
        m
    });

    let get_after_delete = client.call_tool(get_params).await;
    match get_after_delete {
        Err(_) => {
            // Any error means the artifact is no longer retrievable — the delete
            // worked. This is an acceptable "absent" outcome (not-found error path).
        }
        Ok(result) => {
            // Ok with content: the artifact marker must be absent (deleted, not returned).
            let body = serde_json::to_string(&result).expect("serialize echo.get result");
            assert!(
                !body.contains("todelete_t12d"),
                "R-0019-c: echo.delete must remove the artifact — the marker \
                 'todelete_t12d' must be absent from echo.get after delete; \
                 got body: {body}"
            );
        }
    }

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 14 (RED): echo.delete cannot remove another workspace's artifact
//   (T12 delete-slice — R-0006-d tenant WRITE isolation — THE security assertion)
// ===========================================================================

/// R-0006-d — `echo.delete` is workspace-scoped on WRITE: a token in workspace B
/// MUST NOT delete an artifact owned by workspace A. The fenced `(workspace_id, id)`
/// lookup misses under B, so the delete is a silent no-op that returns success;
/// A's artifact is left untouched.
///
/// # Given / When / Then
///
/// GIVEN an `echo_fixture` created in workspace A (fresh Uuid, admin token) with
///   marker "ws_a_artifact_t12d" (id_A captured), and a separate admin token for
///   a different fresh Uuid workspace B
/// WHEN workspace B's token calls `echo.delete` with id_A
/// THEN BOTH hold:
///   (a) the delete call from B DISPATCHES successfully (Ok — a no-op success,
///       NOT an error; `workspace_id` is host-derived from B's ctx, so the lookup
///       `(workspace_B, id_A)` misses → miss=no-op → Ok), AND
///   (b) `echo.get` of id_A under WORKSPACE A's token still returns the artifact
///       with its original marker (A's artifact was NOT deleted by B).
///
/// # Why the dispatch-success half is required (right-reason red)
///
/// While `echo.delete` is unwired it returns -4003 NON_DISPATCHABLE, so the
/// explicit `.expect` on B's delete call panics → the test reds for the right
/// reason ("delete not wired"). WITHOUT asserting dispatch-success, an unwired
/// delete would no-op and leave A unchanged, so the isolation half (b) alone
/// would pass VACUOUSLY at red. The (a) half is therefore load-bearing for a
/// non-vacuous red.
///
/// # Security note
///
/// The security guarantee is carried by the `contains("ws_a_artifact_t12d")` half:
/// A's original marker must still be present after B's delete attempt.
/// A cross-tenant no-op delete MUST NOT silently remove A's artifact.
///
/// # Workspace seeding
///
/// `admin_tokens.workspace_id` is UUID NOT NULL with no FK to a `workspaces` row
/// (schema init migration 7) — a token for a fresh Uuid4 is valid without a
/// `workspaces` row (same approach as `cross_workspace_get_returns_none` and
/// `echo_list_is_workspace_scoped`).
///
/// # verify: []
///
/// `verify: []` by design — this test reds today (echo.delete unwired).
#[tokio::test]
async fn echo_delete_cannot_remove_another_workspace_artifact() {
    // R-0006-d: echo.delete is tenant-isolated on write — B cannot delete A.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_a = Uuid::new_v4();
    let workspace_b = Uuid::new_v4();
    let (token_a, _a_id) = seed_admin_token(pool, workspace_a).await;
    let (token_b, _b_id) = seed_admin_token(pool, workspace_b).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // Create an artifact in workspace A with a known, distinctive marker.
    let id_a = create_echo_fixture(&client, &token_a, "ws_a_artifact_t12d").await;

    // WHEN — workspace B's token attempts to delete workspace A's artifact.
    let mut delete_params = CallToolRequestParams::new("echo.delete");
    delete_params.meta = Some(token_meta(token_b.as_str()));
    delete_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(id_a));
        m
    });

    // (a) dispatch-success MUST hold explicitly — a cross-workspace delete is a
    // no-op that returns Ok (the fenced (workspace_id, id_A) lookup misses under B).
    // While the verb is unwired this panics with -4003 NON_DISPATCHABLE — the
    // right-reason red. This half is required so the isolation half below cannot
    // pass vacuously at red.
    let _b_delete = client.call_tool(delete_params).await.expect(
        "R-0006-d: cross-workspace echo.delete must DISPATCH successfully (no-op success, \
         NOT an error); red phase: panics because echo.delete is unwired \
         (NON_DISPATCHABLE -4003) — this is the right-reason red",
    );

    // (b) THE security assertion: read id_A back UNDER WORKSPACE A's token;
    // A's artifact must still be present — B could not delete it.
    let mut get_params = CallToolRequestParams::new("echo.get");
    get_params.meta = Some(token_meta(token_a.as_str()));
    get_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(id_a));
        m
    });
    let get_result = client
        .call_tool(get_params)
        .await
        .expect("R-0006-d: echo.get of id_A under workspace A's token must return Ok");

    let body = serde_json::to_string(&get_result).expect("serialize echo.get result");
    assert!(
        body.contains("ws_a_artifact_t12d"),
        "R-0006-d: tenant WRITE isolation violated — workspace B deleted workspace A's \
         artifact (the marker 'ws_a_artifact_t12d' is absent from A's get result); \
         got body: {body}"
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// T13 RED tests (dispatch #1150): Swap fenced in-memory stub → real
//   echo_fixture persistence (R-0012-a, R-0001-a, R-0006-d, R-0019-e)
// ===========================================================================
//
// # T13 acceptance criteria (reconciled, committed brain 9c47b97)
//
// These tests assert that artifact host-fn bodies persist to the P-0001
// per-artifact-type `echo_fixture` table (direct sqlx) rather than the T7
// fenced in-memory `FencedArtifactStore`. Every test FAILS against the current
// implementation for the RIGHT reason: a direct sqlx `SELECT FROM echo_fixture`
// finds no row where the fenced map was written, OR a direct echo_fixture row
// (seeded via sqlx) is not touched by the fenced-map host body.
//
// # R-ID mapping (T13 additions)
//
// | Test function                                         | R-ID(s)                        |
// |-------------------------------------------------------|--------------------------------|
// | create_persists_a_real_echo_fixture_row (RED)         | R-0012-a, R-0001-a, R-0019-e   |
// | cross_workspace_isolation_at_echo_fixture_table (RED) | R-0006-d                       |
// | create_survives_fresh_db_query (RED)                  | R-0001-a                       |
// | crud_update_reflected_in_echo_fixture_table (RED)     | R-0019-e, R-0012-a             |
// | crud_delete_removes_echo_fixture_row (RED)            | R-0019-e, R-0012-a             |
//
// # Cross-dispatch context for Forge (green phase)
//
// ## echo_fixture table columns asserted in these tests
//
// | Column              | Type    | Asserted how                                   |
// |---------------------|---------|------------------------------------------------|
// | id                  | TEXT    | SELECT WHERE id = $1 (row presence / absence)  |
// | workspace_id        | UUID    | SELECT WHERE workspace_id = $2 (isolation)     |
// | frontmatter         | JSONB   | frontmatter::text contains marker substring    |
// | body                | TEXT    | not asserted in T13 red (covered by T12 tests) |
//
// ## CHECK constraints Forge's INSERT must satisfy (R-0001-c)
//
// - `frontmatter ? 'id'` — Forge's artifact_create must inject the generated
//   ULID as the `id` key in the frontmatter JSONB BEFORE INSERT. The test
//   payloads include `frontmatter_version` but NOT `id` (the caller cannot
//   know the ULID in advance). Forge must set frontmatter['id'] = ULID.
//   NOTE: the `id` COLUMN and `frontmatter->>'id'` are asserted independently
//   (artifact_machinery.rs uses different values for each); these tests do not
//   assert their equality.
// - `frontmatter ? 'frontmatter_version'` — all test echo.create payloads
//   carry `"frontmatter_version": 1`. Direct-seeded rows use SEEDED_FRONTMATTER
//   which includes both required keys.
//
// ## HostState-PgPool precondition (Forge must address before green)
//
// `HostState` (`plugin/component.rs`) currently holds `FencedArtifactStore` +
// `workspace_id` — NO `PgPool`. Forge must thread a `PgPool` (or a typed
// artifact-store handle wrapping it) into `HostState` so the
// `artifact::Host for HostState` import bodies can run sqlx against
// `echo_fixture`. `MnemraMcpServer` already holds `pool: PgPool`; Forge
// threads it down the dispatch path into `HostState` at `invoke_content`
// call time.

/// Directly query `echo_fixture` for a row with the given id and workspace_id.
/// Returns `(id_text, workspace_id_uuid, frontmatter_as_text)` if found.
///
/// Used by all T13 persistence assertions. The frontmatter is fetched as text
/// (sqlx 0.9 does not enable the `json` feature in this Cargo.toml — JSONB
/// is cast to `::text` and inspected via string operations).
///
/// A panic from this helper is a SETUP error (wrong column name, type mismatch)
/// — NOT the red assertion. Only the subsequent `assert!` call is the red.
async fn fetch_echo_fixture_row(
    pool: &sqlx::PgPool,
    id: &str,
    workspace_id: Uuid,
) -> Option<(String, Uuid, String)> {
    sqlx::query_as(
        "SELECT id, workspace_id, frontmatter::text
         FROM echo_fixture
         WHERE id = $1 AND workspace_id = $2",
    )
    .bind(id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .expect(
        "fetch_echo_fixture_row: SELECT must not error \
         (echo_fixture table exists from schema init step 4)",
    )
}

/// Frontmatter used by direct-seeded echo_fixture rows (tests 18/19).
/// Satisfies both CHECK constraints:
///   - `frontmatter ? 'id'`                   — satisfied
///   - `frontmatter ? 'frontmatter_version'`  — satisfied
const SEEDED_FRONTMATTER: &str =
    r#"{"id": "T13TESTULID000000000000", "frontmatter_version": 1, "status": "open"}"#;

/// Directly INSERT a row into `echo_fixture` (for seeding the update/delete tests).
/// Mirrors the `insert_fixture_row` helper in `artifact_machinery.rs`.
///
/// A panic here is a SETUP error (the test harness is broken, not the
/// implementation). The seeded row is NOT written to the `FencedArtifactStore`
/// — it exists only in Postgres. This lets the update/delete table assertions
/// execute even though the T13 red for `create` would fail first in a joint
/// sequence.
async fn seed_echo_fixture_row(pool: &sqlx::PgPool, id: &str, workspace_id: Uuid) {
    sqlx::query(
        "INSERT INTO echo_fixture (id, workspace_id, type, frontmatter)
         VALUES ($1, $2, 'echo_fixture', $3::jsonb)",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(SEEDED_FRONTMATTER)
    .execute(pool)
    .await
    .unwrap_or_else(|e| {
        panic!(
            "seed_echo_fixture_row: INSERT failed for id={id} — \
             setup error, not a T13 test failure: {e}"
        )
    });
}

// ===========================================================================
// Test 15 (RED): echo.create persists a real echo_fixture row
//   AC#1 (R-0012-a, R-0001-a) + AC#4 happy-path re-target (R-0019-e)
// ===========================================================================

/// R-0012-a, R-0001-a, R-0019-e — echo.create must persist to echo_fixture.
///
/// # Given / When / Then
///
/// GIVEN an admin token for DEFAULT_WORKSPACE_ID
/// WHEN `echo.create` is called with an echo_fixture payload (including
///   `frontmatter_version: 1` required by the `frontmatter ? 'frontmatter_version'`
///   CHECK constraint; Forge must inject `id = <generated ULID>` to satisfy
///   `frontmatter ? 'id'`)
/// THEN (1) the returned ULID identifies a real row in the `echo_fixture` table —
///       `SELECT WHERE id = $ulid AND workspace_id = $ws` finds the row, AND
///      (2) the frontmatter text contains the create payload marker, AND
///      (3) a follow-on `echo.get` returns content containing the marker
///       (re-targeted per AC#4 — reads from Postgres, not the fenced map).
///
/// # Not asserted: frontmatter->>'id' == returned ULID
///
/// The `id` COLUMN must equal the returned ULID (certain). The frontmatter
/// key `'id'` is a CHECK-constraint requirement but its value is Forge's design
/// choice — `artifact_machinery.rs` uses column-id and frontmatter-id
/// independently, so equality is not a schema invariant these tests assert.
///
/// # Why is_ok() alone does not discriminate
///
/// `echo.create` already returns Ok in the current fenced-map impl. The direct
/// sqlx `SELECT` is the discriminating assertion: it passes only after Forge
/// writes an INSERT INTO echo_fixture.
///
/// # Red reason
///
/// `SELECT id, workspace_id, frontmatter::text FROM echo_fixture
///  WHERE id = $ulid AND workspace_id = $ws` finds NO row — the current host
/// body writes to FencedArtifactStore only. The `assert!(row.is_some(), …)`
/// panics: right-reason red.
#[tokio::test]
async fn create_persists_a_real_echo_fixture_row() {
    // R-0012-a, R-0001-a: echo.create must write to the echo_fixture table.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_admin_token(pool, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // WHEN — echo.create with frontmatter_version in the payload.
    // Satisfies CHECK (frontmatter ? 'frontmatter_version').
    // Forge must inject id = <generated ULID> into frontmatter to satisfy
    // CHECK (frontmatter ? 'id') — the caller cannot know the ULID in advance.
    let mut params = CallToolRequestParams::new("echo.create");
    params.meta = Some(token_meta(token.as_str()));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert(
            "payload".to_owned(),
            json!({"frontmatter_version": 1, "msg": "t13_create_real_row"}),
        );
        m
    });

    let create_result = client
        .call_tool(params)
        .await
        .expect("echo.create with valid admin token must return Ok (verb is wired)");

    let texts = extract_text_content(&create_result);
    let ulid = texts
        .iter()
        .find(|t| is_valid_ulid(t.trim()))
        .map(|t| t.trim().to_owned())
        .unwrap_or_else(|| {
            panic!("setup: echo.create must return a well-formed ULID; got texts: {texts:?}")
        });

    // THEN (1) — the ULID must identify a real row in echo_fixture.
    // RIGHT-REASON RED: fenced create wrote to FencedArtifactStore; echo_fixture
    // has no row. The assert panics with "row was absent".
    let row = fetch_echo_fixture_row(pool, &ulid, workspace_id).await;
    assert!(
        row.is_some(),
        "R-0012-a, R-0001-a: create_persists_a_real_echo_fixture_row: \
         echo.create must write to the echo_fixture table. \
         SELECT id, workspace_id, frontmatter FROM echo_fixture \
         WHERE id = '{ulid}' AND workspace_id = '{workspace_id}' found no row. \
         Current impl writes FencedArtifactStore only (not echo_fixture). \
         Right-reason red: Forge must swap artifact_create body to sqlx \
         INSERT INTO echo_fixture."
    );

    let (_id_col, _ws_col, frontmatter_text) = row.unwrap();

    // THEN (2) — the frontmatter must contain the create payload marker.
    assert!(
        frontmatter_text.contains("t13_create_real_row"),
        "R-0019-e: echo_fixture frontmatter must round-trip the create payload; \
         expected 't13_create_real_row' in frontmatter; got: {frontmatter_text}"
    );

    // THEN (3) — echo.get must read back content from Postgres (AC#4 re-target).
    // After Forge's green: echo.get reads from echo_fixture, not the fenced map.
    let mut get_params = CallToolRequestParams::new("echo.get");
    get_params.meta = Some(token_meta(token.as_str()));
    get_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(ulid));
        m
    });
    let get_result = client
        .call_tool(get_params)
        .await
        .expect("R-0019-e: echo.get must return Ok");

    let get_body = serde_json::to_string(&get_result).expect("serialize get result");
    assert!(
        get_body.contains("t13_create_real_row"),
        "R-0019-e: echo.get must return content from Postgres; \
         expected payload marker 't13_create_real_row' in result; got: {get_body}"
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 16 (RED): WHERE-clause workspace scoping at the echo_fixture table (AC#2)
// ===========================================================================

/// R-0006-d — every echo_fixture read path must include
///   `WHERE workspace_id = ctx.workspace_id()` (P-0006 application-layer isolation).
///
/// # Given / When / Then
///
/// GIVEN an `echo_fixture` artifact created under workspace A's admin token
/// WHEN (a) queried directly via sqlx for workspace A → row MUST exist, AND
///      (b) queried directly via sqlx for workspace B → row MUST be absent
///          (WHERE workspace_id = workspace_B finds nothing), AND
///      (c) `echo.get` under workspace B's token → returns empty (not-found)
/// THEN workspace isolation holds at the echo_fixture table level.
///
/// # Why assertion (a) is the right-reason red
///
/// Assertion (a) fails first: the current fenced create does not write to
/// echo_fixture, so `SELECT WHERE workspace_id = workspace_A` finds nothing.
/// Assertion (b) passes vacuously (no rows at all). After Forge's green:
/// (a) passes (row exists), (b) passes (WHERE workspace_id = workspace_B
/// correctly excludes workspace A's row), (c) passes (Postgres read is scoped).
///
/// # Security note
///
/// The (b) half is the table-level isolation guarantee. A cross-workspace id
/// that exists in echo_fixture must NOT be visible to workspace B's queries —
/// enforced by the mandatory `workspace_id` WHERE clause.
#[tokio::test]
async fn cross_workspace_isolation_at_echo_fixture_table() {
    // R-0006-d: echo_fixture reads are workspace-scoped; B cannot see A's row.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_a = Uuid::new_v4();
    let workspace_b = Uuid::new_v4();
    let (token_a, _a_id) = seed_admin_token(pool, workspace_a).await;
    let (token_b, _b_id) = seed_admin_token(pool, workspace_b).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // Create an artifact under workspace A with frontmatter_version and a
    // distinctive marker. Both CHECK-constraint keys present.
    let mut create_params = CallToolRequestParams::new("echo.create");
    create_params.meta = Some(token_meta(token_a.as_str()));
    create_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert(
            "payload".to_owned(),
            json!({"frontmatter_version": 1, "msg": "t13_ws_a_marker"}),
        );
        m
    });
    let create_result = client
        .call_tool(create_params)
        .await
        .expect("setup: echo.create (wired) must return Ok for workspace A");
    let texts = extract_text_content(&create_result);
    let id_a = texts
        .iter()
        .find(|t| is_valid_ulid(t.trim()))
        .map(|t| t.trim().to_owned())
        .unwrap_or_else(|| panic!("setup: echo.create must return a ULID; got: {texts:?}"));

    // THEN (a) — workspace A's echo_fixture row MUST EXIST (RIGHT-REASON RED).
    // Fails now: fenced create did not write to echo_fixture.
    let row_for_a = fetch_echo_fixture_row(pool, &id_a, workspace_a).await;
    assert!(
        row_for_a.is_some(),
        "R-0006-d: cross_workspace_isolation_at_echo_fixture_table: \
         workspace A's echo.create must persist to echo_fixture. \
         SELECT WHERE id = '{id_a}' AND workspace_id = '{workspace_a}' found no row. \
         Current impl writes FencedArtifactStore only. Right-reason red."
    );

    // THEN (b) — the same id is NOT visible for workspace B in echo_fixture.
    // WHERE workspace_id = workspace_B must exclude workspace A's row.
    // Vacuously passes now (no rows exist); correctly enforced after Forge's green.
    let row_for_b = fetch_echo_fixture_row(pool, &id_a, workspace_b).await;
    assert!(
        row_for_b.is_none(),
        "R-0006-d: tenant isolation violated at the echo_fixture table: \
         workspace B's SELECT (WHERE workspace_id = '{workspace_b}') returned \
         workspace A's row (id = '{id_a}'). \
         The artifact_get body must include WHERE workspace_id = ctx.workspace_id()."
    );

    // THEN (c) — echo.get under workspace B's token must not return workspace A's content.
    let mut get_params = CallToolRequestParams::new("echo.get");
    get_params.meta = Some(token_meta(token_b.as_str()));
    get_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(id_a));
        m
    });
    let get_result = client.call_tool(get_params).await;

    let b_sees_content = match &get_result {
        Ok(r) => {
            let body = serde_json::to_string(r).unwrap_or_default();
            body.contains("t13_ws_a_marker")
        }
        Err(_) => false, // not-found error means isolation holds
    };
    assert!(
        !b_sees_content,
        "R-0006-d: workspace B's echo.get must not return workspace A's artifact content; \
         marker 't13_ws_a_marker' must be absent from workspace B's get result"
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 17 (RED): echo.create persists and survives a fresh DB query (AC#3)
// ===========================================================================

/// R-0001-a — created artifacts persist across transactions (real Postgres, not RAM).
///
/// # Given / When / Then
///
/// GIVEN an admin token for DEFAULT_WORKSPACE_ID
/// WHEN `echo.create` is called and the ULID is captured
/// THEN a FRESH sqlx query (a connection/tx distinct from the create dispatch
///   path) finds the row in `echo_fixture` — proving real Postgres persistence,
///   not in-process memory state.
///
/// # What distinguishes this from test 15
///
/// Test 15 queries via the same pool after the MCP create call. Here the
/// emphasis is on provability: the `FencedArtifactStore` is an in-process
/// `Arc<Mutex<HashMap>>` that survives across pooled connections in the same
/// test binary — a direct sqlx `SELECT` is provably NOT the fenced map. If
/// `artifact_create` wrote only to the in-memory store, the SELECT finds
/// nothing. After Forge's committed INSERT, the SELECT finds the row.
///
/// # Red reason
///
/// `SELECT id FROM echo_fixture WHERE id = $ulid` finds NO row — the current
/// host body wrote to FencedArtifactStore only. The `assert!(persisted.is_some())`
/// panics: right-reason red.
#[tokio::test]
async fn create_survives_fresh_db_query() {
    // R-0001-a: created artifacts persist to Postgres, not just in-process memory.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_admin_token(pool, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // WHEN — echo.create; capture the returned ULID.
    let mut params = CallToolRequestParams::new("echo.create");
    params.meta = Some(token_meta(token.as_str()));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert(
            "payload".to_owned(),
            json!({"frontmatter_version": 1, "msg": "t13_fresh_query_test"}),
        );
        m
    });

    let create_result = client
        .call_tool(params)
        .await
        .expect("echo.create with valid admin token must return Ok (verb is wired)");

    let texts = extract_text_content(&create_result);
    let ulid = texts
        .iter()
        .find(|t| is_valid_ulid(t.trim()))
        .map(|t| t.trim().to_owned())
        .unwrap_or_else(|| {
            panic!("setup: echo.create must return a well-formed ULID; got texts: {texts:?}")
        });

    // THEN — query echo_fixture through a fresh pool connection.
    // The pool provides a new connection from the pool; if artifact_create
    // wrote only to the in-memory fenced map, this SELECT finds nothing.
    //
    // RIGHT-REASON RED: finds no row; Forge must commit INSERT INTO echo_fixture.
    let persisted: Option<(String,)> = sqlx::query_as("SELECT id FROM echo_fixture WHERE id = $1")
        .bind(&ulid)
        .fetch_optional(pool)
        .await
        .expect(
            "create_survives_fresh_db_query: SELECT must not error \
                 (echo_fixture table exists from schema init step 4)",
        );

    assert!(
        persisted.is_some(),
        "R-0001-a: create_survives_fresh_db_query: \
         echo.create must persist to Postgres (not just the in-process fenced map). \
         SELECT id FROM echo_fixture WHERE id = '{ulid}' found no row. \
         FencedArtifactStore is in-process memory only. \
         Right-reason red: Forge must commit sqlx INSERT INTO echo_fixture \
         in the artifact_create host body."
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 18 (RED): echo.update patch reflected in echo_fixture table (AC#4 update)
// ===========================================================================

/// R-0019-e, R-0012-a — echo.update must write the patch to the echo_fixture table.
///
/// # Given / When / Then
///
/// GIVEN an echo_fixture row seeded DIRECTLY in the DB via sqlx (the row exists
///   in the table, NOT in the `FencedArtifactStore`), with original frontmatter
///   status = "open" (from SEEDED_FRONTMATTER)
/// WHEN `echo.update` is called with a frontmatter_patch `{"status": "done_t13u"}`
///   for the seeded id under the workspace's admin token
/// THEN the echo_fixture table's frontmatter for that row shows "done_t13u"
///   (the patch was applied) — verified by direct sqlx SELECT.
///
/// # Why direct-seed (advisor guidance)
///
/// Using a directly-seeded row ensures this test's red assertion executes even
/// though the T13 echo.create red (test 15) would fail first in a joint
/// sequence. The seeded row EXISTS in echo_fixture; the fenced-map echo.update
/// with an id NOT in the fenced map returns Ok but does not touch echo_fixture.
/// The table assertion then fails for the right reason.
///
/// # Red reason
///
/// `fenced_artifact_update` performs a `map.get_mut(&(workspace_id, id))` lookup.
/// The seeded id is NOT in the fenced map, so the update is a silent no-op;
/// echo_fixture's frontmatter remains `"status": "open"`. The assertion
/// `frontmatter_text.contains("done_t13u")` FAILS — right-reason red.
/// After Forge's green: `artifact_update` runs
/// `UPDATE echo_fixture SET frontmatter = … WHERE id = $1 AND workspace_id = $2`,
/// the frontmatter reflects the patch, and the assertion passes.
#[tokio::test]
async fn crud_update_reflected_in_echo_fixture_table() {
    // R-0019-e: echo.update must apply the patch to the echo_fixture table.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_admin_token(pool, workspace_id).await;

    // Seed a row directly in echo_fixture (NOT in the fenced map).
    // This lets the update assertion execute independently of the create red (test 15).
    let seeded_id = "T13UPDATETEST0000000000001";
    seed_echo_fixture_row(pool, seeded_id, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // WHEN — echo.update with a frontmatter_patch that should change 'status'.
    // The fenced map has no entry for seeded_id → returns Ok (silent no-op).
    // The echo_fixture row is NOT updated.
    let patch = json!({"status": "done_t13u"}).to_string();
    let mut update_params = CallToolRequestParams::new("echo.update");
    update_params.meta = Some(token_meta(token.as_str()));
    update_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(seeded_id));
        m.insert("frontmatter_patch".to_owned(), json!(patch));
        m
    });

    // The fenced-map update returns Ok even for a no-op (miss = silent no-op).
    let _update_result = client
        .call_tool(update_params)
        .await
        .expect("R-0019-e: echo.update must return Ok");

    // THEN — the echo_fixture table must show the patched frontmatter.
    // RIGHT-REASON RED: fenced-map update no-oped; frontmatter still shows "open".
    let row = fetch_echo_fixture_row(pool, seeded_id, workspace_id).await;
    assert!(
        row.is_some(),
        "crud_update_reflected_in_echo_fixture_table: seeded row must be present \
         (setup check — a panic here means seed_echo_fixture_row failed)"
    );
    let (_id_col, _ws_col, frontmatter_text) = row.unwrap();

    assert!(
        frontmatter_text.contains("done_t13u"),
        "R-0019-e: crud_update_reflected_in_echo_fixture_table: \
         echo.update must apply the frontmatter patch to the echo_fixture table. \
         Expected frontmatter to contain 'done_t13u' after echo.update with \
         patch {{\"status\": \"done_t13u\"}}. \
         Current impl: fenced-map update no-oped (id '{seeded_id}' not in fenced map); \
         echo_fixture frontmatter unchanged from seeded value: {frontmatter_text}. \
         Right-reason red: Forge must run \
         UPDATE echo_fixture SET frontmatter = … WHERE id = $1 AND workspace_id = $2."
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 19 (RED): echo.delete removes the echo_fixture row (AC#4 delete)
// ===========================================================================

/// R-0019-e, R-0012-a — echo.delete must remove the row from the echo_fixture table.
///
/// # Given / When / Then
///
/// GIVEN an echo_fixture row seeded DIRECTLY in the DB via sqlx (row exists in
///   the table, NOT in the `FencedArtifactStore`) for the workspace
/// WHEN `echo.delete` is called with the seeded id under the workspace's token
/// THEN the echo_fixture row is GONE — confirmed by direct sqlx SELECT.
///
/// # Why direct-seed (advisor guidance)
///
/// Same reasoning as test 18: direct seeding lets the delete assertion execute
/// independently of the T13 create red. The fenced-map echo.delete with a
/// seeded id returns Ok but does NOT remove the echo_fixture row.
///
/// # Red reason
///
/// `fenced_artifact_delete` runs `map.remove(&(workspace_id, id))`. The seeded
/// id is NOT in the fenced map, so `map.remove` is a no-op (returns None) —
/// the echo_fixture row is left untouched. The `assert!(after_delete.is_none())`
/// assertion FAILS (row still present) — right-reason red.
/// After Forge's green: `artifact_delete` runs
/// `DELETE FROM echo_fixture WHERE id = $1 AND workspace_id = $2`, the row is
/// removed, and the assertion passes.
#[tokio::test]
async fn crud_delete_removes_echo_fixture_row() {
    // R-0019-e: echo.delete must remove the row from the echo_fixture table.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_admin_token(pool, workspace_id).await;

    // Seed a row directly in echo_fixture (NOT in the fenced map).
    let seeded_id = "T13DELETETEST0000000000001";
    seed_echo_fixture_row(pool, seeded_id, workspace_id).await;

    // Verify the seeded row IS present before the delete (setup guard).
    let before_delete = fetch_echo_fixture_row(pool, seeded_id, workspace_id).await;
    assert!(
        before_delete.is_some(),
        "crud_delete_removes_echo_fixture_row setup: seeded row must be present \
         before echo.delete (a panic here means seed_echo_fixture_row failed)"
    );

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // WHEN — echo.delete with the seeded id.
    // The fenced map has no entry for seeded_id → delete is a silent no-op.
    let mut delete_params = CallToolRequestParams::new("echo.delete");
    delete_params.meta = Some(token_meta(token.as_str()));
    delete_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(seeded_id));
        m
    });

    // The fenced-map delete returns Ok even for a no-op (miss = silent no-op).
    let _delete_result = client
        .call_tool(delete_params)
        .await
        .expect("R-0019-e: echo.delete must return Ok (even a no-op fenced delete returns Ok)");

    // THEN — the echo_fixture row must be GONE (RIGHT-REASON RED).
    // The fenced-map delete no-oped; the echo_fixture row is still present.
    let after_delete = fetch_echo_fixture_row(pool, seeded_id, workspace_id).await;
    assert!(
        after_delete.is_none(),
        "R-0019-e: crud_delete_removes_echo_fixture_row: \
         echo.delete must remove the row from the echo_fixture table. \
         SELECT WHERE id = '{seeded_id}' AND workspace_id = '{workspace_id}' \
         still found a row after echo.delete. \
         Current impl: fenced-map delete no-oped (id not in fenced map); \
         echo_fixture row was not removed. \
         Right-reason red: Forge must run \
         DELETE FROM echo_fixture WHERE id = $1 AND workspace_id = $2."
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// RED-2 tests (dispatch #1153): Fail-closed DB error paths (HIGH-1, MEDIUM-1)
// ===========================================================================
//
// # Coverage
//
// Warden (dispatch #1153 pre-work) found two fail-OPEN error paths in the T13
// artifact host-fn bodies. These tests pin the fail-CLOSED contract so Forge
// (green-2) has a failing test to target.
//
// | Test function                                          | Finding  |
// |--------------------------------------------------------|----------|
// | update_with_malformed_patch_fails_closed (RED-2)       | HIGH-1   |
// | db_error_message_is_scrubbed_of_sqlx_detail (RED-2)    | MEDIUM-1 |
//
// # Delete error path assessment
//
// HIGH-1 also names echo.delete. However, the DELETE path cannot be made to
// error via black-box client input:
//   - `DELETE FROM echo_fixture WHERE id = $1 AND workspace_id = $2` has no JSONB
//     operations or CHECK constraints; all parameters are simple text/UUID binds.
//   - A delete targeting a non-existent (or cross-workspace) id is a silent no-op
//     (0 rows affected), NOT a database error.
//   - workspace_id is host-derived from the authenticated token (not client-
//     controllable).
// Delete's error path is white-box-only (Forge unit test + Warden code review).
// Only echo.update is covered by RED-2 black-box tests; this assessment is the
// cross-dispatch note for Forge.
//
// # Malformed-patch trigger (cross-dispatch contract for Forge)
//
// Both tests use the same input trigger:
//   MCP argument: `frontmatter_patch = json!("not_a_json_object_t13w2")`
//
// Trace through the call stack:
//   1. `dispatch.rs::resolve_content_call` extracts via `.as_str()`:
//      inner string = `not_a_json_object_t13w2` (not valid JSON).
//   2. `artifact_update` (component.rs) binds to `$1::jsonb` in:
//        UPDATE echo_fixture SET frontmatter = frontmatter || $1::jsonb ...
//   3. PostgreSQL CAST: `'not_a_json_object_t13w2'::jsonb` →
//      ERROR: invalid input syntax for type json
//   4. Current impl: `.map_err(|e| { warn!(...); })` swallows the error;
//      `artifact_update` returns `()` (void); WIT content.update returns
//      normally; invoke_content returns Ok(ContentResult::Updated);
//      MCP response is SUCCESS.
//
// After Forge's fix: DB error propagated → MCP response is ERROR.
//
// # Right-reason failure (both RED-2 tests)
//
// Both tests assert the MCP result is Err. Today it is Ok (error swallowed).
// The primary failing assertion in each test is:
//   - Test 20: `assert!(result.is_err(), "HIGH-1: ...")`
//   - Test 21: `result.expect_err("MEDIUM-1: ...")`
// Neither is a compile error; neither is a setup panic. Both fail because the
// current implementation returns a success result when it should return an error.

// ===========================================================================
// Test 20 (RED-2): echo.update with malformed frontmatter_patch fails closed
//   HIGH-1 — fail-open swallow-success error path
// ===========================================================================

/// HIGH-1 — `echo.update` with a malformed `frontmatter_patch` must return an
/// MCP error (fail-closed), NOT a fake success that conceals a rejected write.
///
/// # Given / When / Then
///
/// GIVEN an admin token for DEFAULT_WORKSPACE_ID and an `echo_fixture` row
///   seeded directly in the DB via sqlx (SEEDED_FRONTMATTER: `"status": "open"`)
/// WHEN `echo.update` is called with `frontmatter_patch = json!("not_a_json_object_t13w2")`
///   (a JSON string argument whose inner value is not valid JSON), targeting the
///   seeded id `T13W2UPDATE000000000000001`
/// THEN (a) the MCP call returns an Err result (NOT a success/Updated), AND
///      (b) the echo_fixture row is UNCHANGED — frontmatter still contains "open"
///          (the failed UPDATE is atomic: no partial write from a Postgres CAST error).
///
/// # Malformed-patch trigger
///
/// `json!("not_a_json_object_t13w2")` as the `frontmatter_patch` MCP argument:
///   - The argument VALUE is a JSON string; `.as_str()` in dispatch.rs extracts
///     the inner string `not_a_json_object_t13w2` (not quoted, not valid JSON).
///   - The UPDATE query binds it to `$1::jsonb`:
///       `frontmatter || 'not_a_json_object_t13w2'::jsonb`
///   - PostgreSQL CAST ERROR: "invalid input syntax for type json"
///   - Current impl: `.map_err(warn)` in `artifact_update` swallows the error,
///     returns `()` → the WIT guest sees a successful return → MCP returns Ok.
///   - After Forge fix: error propagated → MCP returns Err.
///
/// # Right-reason red
///
/// `result.is_err()` FAILS because the current impl returns `Ok(success)`.
/// This is the correct right-reason: the error is swallowed, not propagated.
/// The row-unchanged assertion (b) PASSES today (the CAST error prevents any
/// write). Assertion (b) is the atomicity contract for Forge's fix.
///
/// # Cross-dispatch contract for Forge (green-2)
///
/// - Seeded row id: `T13W2UPDATE000000000000001`, workspace_id: DEFAULT_WORKSPACE_ID
/// - Trigger: `frontmatter_patch = json!("not_a_json_object_t13w2")` →
///   Postgres CAST fails at `$1::jsonb`
/// - MCP result must be Err (not Ok/Updated)
/// - Row frontmatter must be unchanged (SEEDED_FRONTMATTER, "open" status present)
/// - Forge must propagate the DB error through artifact_update → WIT → MCP error.
///   The Postgres CAST failure is already atomic (no partial write); Forge only
///   needs to surface the error rather than swallow it.
#[tokio::test]
async fn update_with_malformed_patch_fails_closed() {
    // HIGH-1: echo.update with malformed frontmatter_patch must fail closed.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_admin_token(pool, workspace_id).await;

    // Seed a row directly in echo_fixture (same pattern as T13 tests 18/19).
    // Direct seeding ensures this test executes independently of T13 create reds.
    // SEEDED_FRONTMATTER has "status": "open" — the row-unchanged assertion (b)
    // checks this survives the failed update.
    let seeded_id = "T13W2UPDATE000000000000001";
    seed_echo_fixture_row(pool, seeded_id, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // WHEN — echo.update with a malformed frontmatter_patch.
    //
    // The `frontmatter_patch` MCP argument is a JSON string value whose inner
    // string `not_a_json_object_t13w2` is NOT valid JSON. dispatch.rs extracts
    // the inner string via `.and_then(|v| v.as_str())` and passes it directly
    // to the SQL bind for `$1::jsonb`:
    //   UPDATE echo_fixture SET frontmatter = frontmatter || $1::jsonb ...
    // PostgreSQL: 'not_a_json_object_t13w2'::jsonb → CAST ERROR.
    // Current artifact_update: .map_err(warn) swallows → returns void → SUCCESS.
    let mut update_params = CallToolRequestParams::new("echo.update");
    update_params.meta = Some(token_meta(token.as_str()));
    update_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(seeded_id));
        // NOT a valid JSON object: the inner string is a plain identifier, not JSON.
        // dispatch.rs passes this via .as_str() → binds to $1::jsonb → CAST ERROR.
        m.insert(
            "frontmatter_patch".to_owned(),
            json!("not_a_json_object_t13w2"),
        );
        m
    });

    let result = client.call_tool(update_params).await;

    // THEN (a) — the MCP call must return Err (fail-closed).
    //
    // RIGHT-REASON RED: current impl swallows the Postgres CAST error and returns
    // Ok(success). This assertion FAILS today for the correct behavioral reason.
    // The error path: artifact_update.map_err(warn) → () → WIT void return →
    // invoke_content Ok(Updated) → call_tool Ok(success).
    // Forge must: propagate the DB error so call_tool returns Err.
    assert!(
        result.is_err(),
        "HIGH-1: update_with_malformed_patch_fails_closed: \
         echo.update with a malformed frontmatter_patch must return MCP Err, not Ok. \
         Input: frontmatter_patch = json!(\"not_a_json_object_t13w2\") → \
         dispatch.rs extracts inner string 'not_a_json_object_t13w2' via .as_str() → \
         SQL: frontmatter || 'not_a_json_object_t13w2'::jsonb → \
         Postgres CAST ERROR (invalid input syntax for type json). \
         Current impl: artifact_update swallows via .map_err(warn), returns void; \
         WIT content.update returns normally; invoke_content returns Ok(Updated); \
         MCP response is Ok(success). \
         Right-reason red: error swallowed, not propagated. \
         Forge must surface the DB error as a MCP Err response."
    );

    // THEN (b) — the echo_fixture row must be UNCHANGED (atomicity contract).
    //
    // This assertion PASSES today (the Postgres CAST error aborts the entire UPDATE
    // before any row modification occurs). It pins the atomicity contract: Forge's
    // fail-closed fix must not introduce partial writes.
    let row = fetch_echo_fixture_row(pool, seeded_id, workspace_id).await;
    assert!(
        row.is_some(),
        "HIGH-1 atomicity (setup guard): seeded row must still exist after a failed \
         update — if the row is absent, the setup failed, not the test"
    );
    let (_id_col, _ws_col, frontmatter_text) = row.unwrap();
    assert!(
        frontmatter_text.contains("open"),
        "HIGH-1 atomicity: echo_fixture row frontmatter must be UNCHANGED after the \
         failed update — SEEDED_FRONTMATTER 'open' status must still be present. \
         A partial or committed write would be a data-integrity violation. \
         Got frontmatter: {frontmatter_text}"
    );

    let _ = client.cancel().await;
    let _ = server_handle.await;
}

// ===========================================================================
// Test 21 (RED-2): DB error message is scrubbed of raw sqlx/Postgres detail
//   MEDIUM-1 — internal seam detail must not leak into client-facing MCP error
// ===========================================================================

/// MEDIUM-1 — The MCP error returned for a DB error must NOT contain raw
/// sqlx/Postgres internals (table name, crate name, constraint names,
/// column names, SQL text fragments).
///
/// # Given / When / Then
///
/// GIVEN the same malformed-`frontmatter_patch` trigger as HIGH-1: admin token,
///   an `echo_fixture` row seeded directly in the DB (id `T13W2SCRUB0000000000000001`)
/// WHEN `echo.update` is called with `frontmatter_patch = json!("not_a_json_object_t13w2")`
/// THEN (a) the MCP call returns an Err (same right-reason red as HIGH-1:
///          current impl swallows the DB error and returns success), AND
///      (b) the error message (ErrorData.message + data fields serialized)
///          does NOT contain: `echo_fixture`, `sqlx`, `constraint`, `column`,
///          or SQL text fragments (`UPDATE echo`, `frontmatter ||`).
///
/// # Right-reason red
///
/// The primary failing assertion is `result.expect_err(...)` — which panics
/// because the current impl returns Ok (error swallowed). The scrubbing
/// assertions in the match arm are the GREEN contract: they run only after
/// Forge makes the error propagate to the MCP response.
///
/// # Cross-dispatch contract for Forge (green-2)
///
/// After HIGH-1 is fixed (error propagated), the error message visible to the
/// MCP client MUST be generic/structured — NOT a raw sqlx or Postgres dump.
/// Acceptable error message: `"plugin execution failed: plugin_invocation_failed"`
/// or similar opaque code string.
/// NOT acceptable: any message or data field containing `"echo_fixture"`,
/// `"sqlx"`, `"constraint"`, `"column"`, or SQL fragments from the UPDATE query.
///
/// The structural scan covers `ErrorData.message` and `ErrorData.data` (via
/// full JSON serialization of the ErrorData) so both fields are checked.
#[tokio::test]
async fn db_error_message_is_scrubbed_of_sqlx_detail() {
    // MEDIUM-1: DB error message must not expose raw sqlx/Postgres internals.
    // GIVEN
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref();

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_admin_token(pool, workspace_id).await;

    // Same direct-seed pattern as update_with_malformed_patch_fails_closed.
    // Fresh id to avoid any cross-test state (each test has its own embedded engine).
    let seeded_id = "T13W2SCRUB0000000000000001";
    seed_echo_fixture_row(pool, seeded_id, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => {
                eprintln!("server init failed: {e:?}");
            }
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    // WHEN — same malformed-patch trigger as HIGH-1.
    // `frontmatter_patch = json!("not_a_json_object_t13w2")` →
    // dispatch.rs extracts inner string → Postgres CAST ERROR → swallowed.
    let mut update_params = CallToolRequestParams::new("echo.update");
    update_params.meta = Some(token_meta(token.as_str()));
    update_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(seeded_id));
        // Same non-JSON-object trigger: inner string not valid JSON → CAST ERROR.
        m.insert(
            "frontmatter_patch".to_owned(),
            json!("not_a_json_object_t13w2"),
        );
        m
    });

    let result = client.call_tool(update_params).await;

    // THEN (a) — must be Err.
    //
    // RIGHT-REASON RED: current impl returns Ok (error swallowed — same reason as
    // HIGH-1). `expect_err` panics with the message below.
    // The scrubbing assertions in the match arm are the GREEN contract; they run
    // only after Forge propagates the DB error to the MCP response.
    let err = result.expect_err(
        "MEDIUM-1: db_error_message_is_scrubbed_of_sqlx_detail: \
         echo.update with malformed patch must return MCP Err, not Ok. \
         Right-reason red: current artifact_update swallows the Postgres CAST error \
         via .map_err(warn), returns void; MCP returns Ok(success). \
         Forge must propagate the error before message scrubbing can be verified. \
         (Same root cause as HIGH-1: update_with_malformed_patch_fails_closed.)",
    );

    // THEN (b) — the client-facing error must NOT expose raw DB internals.
    //
    // Only reached after Forge's HIGH-1 fix (error propagated to MCP).
    // Structural check: serialize the full ErrorData and scan for forbidden tokens.
    // Covers both ErrorData.message and ErrorData.data fields.
    match err {
        rmcp::ServiceError::McpError(ref error_data) => {
            let msg = error_data.message.as_ref();

            // Serialize the full ErrorData (message + data) for a comprehensive scan.
            let serialized = serde_json::to_string(error_data)
                .unwrap_or_else(|_| error_data.message.to_string());

            // Table name must not leak (reveals internal schema layout).
            assert!(
                !serialized.contains("echo_fixture"),
                "MEDIUM-1: MCP error must NOT expose table name 'echo_fixture'; \
                 got error message: {msg}"
            );
            // Crate name must not leak (reveals implementation stack).
            assert!(
                !serialized.contains("sqlx"),
                "MEDIUM-1: MCP error must NOT expose crate name 'sqlx'; \
                 got error message: {msg}"
            );
            // Constraint keyword must not leak (reveals DB schema constraints).
            assert!(
                !serialized.contains("constraint"),
                "MEDIUM-1: MCP error must NOT expose 'constraint' \
                 (Postgres constraint name or keyword); got error message: {msg}"
            );
            // Column keyword must not leak (reveals DB column names).
            assert!(
                !serialized.contains("column"),
                "MEDIUM-1: MCP error must NOT expose 'column' \
                 (Postgres column reference); got error message: {msg}"
            );
            // SQL text fragments must not leak (reveal the UPDATE query shape).
            assert!(
                !serialized.contains("UPDATE echo"),
                "MEDIUM-1: MCP error must NOT expose SQL fragment 'UPDATE echo'; \
                 got error message: {msg}"
            );
            assert!(
                !serialized.contains("frontmatter ||"),
                "MEDIUM-1: MCP error must NOT expose SQL fragment 'frontmatter ||'; \
                 got error message: {msg}"
            );
        }
        _other => {
            // A non-McpError (transport / service error): the scrubbing contract
            // applies to the McpError data surface. A non-McpError path here is
            // acceptable for this assertion — the error did not reach the client
            // as structured MCP data, so no internal seam detail leaked via that
            // channel. The HIGH-1 test covers the shape of the returned error code.
        }
    }

    let _ = client.cancel().await;
    let _ = server_handle.await;
}
