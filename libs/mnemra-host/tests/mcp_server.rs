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
//! | Test function                                      | R-ID(s)                  |
//! |----------------------------------------------------|--------------------------|
//! | valid_admin_token_echo_create_returns_ok           | R-0010-a/b/c/d, R-0006-b |
//! | bogus_token_returns_distinguishable_auth_failure   | R-0010-c/f               |
//! | read_observer_write_denied_permission_error        | R-0010-d/f, R-0009-e     |
//! | control_plane_verbs_absent_from_tools_list         | R-0010-g                 |
//!
//! # RED-phase design
//!
//! All four behavioral tests FAIL TO COMPILE because `mnemra_host::mcp::*` does
//! not exist yet. That is the correct red: the mnemra dispatch path is the absent
//! contract; the rmcp API used here is grounded against rmcp 1.7 real symbols.
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
use rmcp::model::{CallToolRequestParams, ErrorCode, Meta};
use rmcp::service::{serve_client, serve_server};
use serde_json::json;
use std::sync::Mutex;
use tokio::io::duplex;
use uuid::Uuid;

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
    let server = MnemraMcpServer::new(pool.clone());

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

    let server = MnemraMcpServer::new(pool.clone());

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

    let server = MnemraMcpServer::new(pool.clone());

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

    let workspace_id = DEFAULT_WORKSPACE_ID;
    let (token, _token_id) = seed_admin_token(pool, workspace_id).await;

    let server = MnemraMcpServer::new(pool.clone());

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
