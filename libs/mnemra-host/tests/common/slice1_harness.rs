//! Slice-1 e2e harness — green phase (Forge).
//!
//! # Contract
//!
//! `slice1_echo_harness` returns a connected MCP client whose `echo.create`
//! dispatches through a loaded mnemra-echo component (real `MnemraMcpServer`
//! path), plus a seeded admin token and its workspace_id.
//!
//! The `Slice1Harness` struct shape and `slice1_echo_harness(pg_pool)` signature
//! are **pinned** by the red phase — the body is filled here; the public shape is
//! unchanged.
//!
//! # What it drives (no shortcut)
//!
//! It builds the REAL `MnemraMcpServer` over a `PluginPool` holding the loaded
//! `mnemra-echo` component, serves it over an in-process duplex transport, and
//! returns the connected rmcp client. `echo.create` / `echo.get` therefore
//! traverse the full slice-1 path: MCP `call_tool` -> auth -> pool component
//! invoke -> guest `content` export -> host `artifact` import (fenced map) ->
//! typed return -> `CallToolResult`. There is no mock and no bypass.

use std::path::PathBuf;
use std::sync::Arc;

use mnemra_host::auth::token::{AdminToken, generate, hash};
use mnemra_host::mcp::server::{ECHO_PLUGIN_NAME, MnemraMcpServer};
use mnemra_host::plugin::pool::PluginPool;
use rmcp::service::{RoleClient, RunningService, serve_client, serve_server};
use tokio::io::duplex;
use uuid::Uuid;
use wasmtime::component::Component;

/// Running slice-1 e2e harness: connected client, seeded admin token, workspace.
///
/// Fields mirror the types returned by the real `serve_client((), transport).await`
/// call in `mcp_server.rs` (RunningService<RoleClient, ()>) and the token type
/// used in `token_meta` in the same file (AdminToken).
///
/// `#[allow(dead_code)]` on `workspace_id`: the field is part of the pinned contract
/// for Forge's green phase; not every red-phase test reads it directly.
#[allow(dead_code)]
pub struct Slice1Harness {
    /// MCP client connected to a running `MnemraMcpServer` over an in-memory duplex.
    /// Type is `RunningService<RoleClient, ()>` — identical to what `serve_client((), t).await`
    /// returns in `mcp_server.rs`. Implements `Deref<Target = Peer<RoleClient>>` so
    /// `call_tool` is available directly.
    pub client: RunningService<RoleClient, ()>,
    /// Admin-scoped token seeded into `admin_tokens` for `workspace_id`.
    pub admin_token: AdminToken,
    /// The workspace this harness operates against.
    pub workspace_id: Uuid,
    /// The live PluginPool shared with the running `MnemraMcpServer`.
    ///
    /// The server holds one `Arc` clone; this field holds another pointing to the
    /// same `PluginPool`. Tests that need fault-injection (R-0007-h) call
    /// `plugin_pool.inject_epoch_death_for_test()` on this handle to degrade the
    /// same pool the server's invoke path reads from.
    ///
    /// Available under `#[cfg(feature = "test-hooks")]` only (the accessor is
    /// feature-gated); storing the `Arc` here is unconditional — the field is always
    /// present so `Slice1Harness` is a stable struct shape across features.
    pub plugin_pool: Arc<PluginPool>,
    /// Server task — keeps the `MnemraMcpServer` alive for the duration of the test.
    pub(crate) _server_handle: tokio::task::JoinHandle<()>,
}

/// Build a slice-1 e2e harness.
///
/// CONTRACT: returns a connected MCP client whose `echo.create` dispatches
/// through a loaded mnemra-echo component (real `MnemraMcpServer` path), plus
/// a seeded admin token and its `workspace_id`.
///
/// The `PgPool` is passed in from the test so each test owns its own engine
/// (embedded Postgres lifetime is caller-managed).
pub async fn slice1_echo_harness(pg_pool: sqlx::PgPool) -> Slice1Harness {
    // The harness operates against the schema's default workspace; the e2e test's
    // workspace-isolation check seeds a SECOND workspace token itself.
    let workspace_id = mnemra_host::schema::init::DEFAULT_WORKSPACE_ID;

    // Seed an admin-scoped token for this workspace (generated per-run, never a
    // literal — no hardcoded secrets in tests).
    let admin_token = generate();
    let token_hash = hash(&admin_token);
    sqlx::query(
        "INSERT INTO admin_tokens (token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3)",
    )
    .bind(token_hash.as_bytes())
    .bind(workspace_id)
    .bind(&vec!["admin".to_owned()])
    .execute(&pg_pool)
    .await
    .expect("seed admin token");

    // Build the live PluginPool and register the loaded mnemra-echo component
    // under its pool name (R-0016-a). This is the REAL component the host
    // invokes — built by `just plugin` to `wasm32-wasip2`.
    //
    // The pool is wrapped in an Arc here so we can keep a clone in Slice1Harness
    // (for fault-injection tests, R-0007-h) AND hand a clone to MnemraMcpServer.
    // Both clones point to the same PluginPool — injecting death via the harness
    // field degrades the same pool the server's invoke path reads from.
    let plugin_pool_inner = PluginPool::new().expect("PluginPool::new");
    let component = Component::from_file(plugin_pool_inner.engine(), echo_component_path())
        .expect("load echo component");
    plugin_pool_inner
        .register_module(ECHO_PLUGIN_NAME, "0.0.1", &component)
        .expect("register echo component into the pool");
    let plugin_pool = Arc::new(plugin_pool_inner);

    // Build the real MnemraMcpServer over the auth pool + the live plugin pool.
    let server = MnemraMcpServer::new(pg_pool, Arc::clone(&plugin_pool));

    // Serve over an in-process duplex transport; drive the client in-process.
    let (server_transport, client_transport) = duplex(8192);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => eprintln!("slice1 harness server init failed: {e:?}"),
        }
    });

    let client = serve_client((), client_transport)
        .await
        .expect("slice1 harness client init");

    Slice1Harness {
        client,
        admin_token,
        workspace_id,
        plugin_pool,
        _server_handle: server_handle,
    }
}

/// Locate the built `mnemra-echo` component (`wasm32-wasip2`, release). Produced
/// by `just plugin`. Resolved relative to the workspace target dir; the CI gate
/// builds it before the test run.
fn echo_component_path() -> PathBuf {
    // mnemra-host manifest dir is `<root>/libs/mnemra-host`; the workspace target
    // is `<root>/target`.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root from libs/mnemra-host");
    let path = root.join("target/wasm32-wasip2/release/mnemra_echo.wasm");
    assert!(
        path.exists(),
        "echo component not found at {} — run `just plugin` (cargo build --release \
         -p mnemra-echo --target wasm32-wasip2) before the e2e tests",
        path.display()
    );
    path
}
