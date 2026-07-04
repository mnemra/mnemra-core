//! Shared harness for the R-0020 artifact-list keyset-pagination acceptance suite
//! (Task 15 RED; Glitch). Included via `#[path = "common/paging_harness.rs"]` by both
//! the black-box page-contract file and the `test-hooks`-gated white-box file.
//!
//! # The page wire contract lives here (single source — flagged)
//!
//! The spec pins the `artifact-page` WIT record `{ ids, has-more, next-cursor }`
//! (spec lines 224-228 / 548-554) and the MCP error-code table, but it does **NOT**
//! specify how that record maps into the MCP `CallToolResult`. This harness pins the
//! observable wire contract in ONE place — [`extract_page`] — so the choice is cheap
//! to pivot if review prefers another encoding:
//!
//! > The `*.list` `CallToolResult` SHALL carry the page as `structured_content`
//! > (a JSON object) with fields `ids` (array), `has_more` (bool), `next_cursor`
//! > (string or null). `structured_content` is the MCP-native structured-output
//! > surface and mirrors the `artifact-page` record. Key casing is accepted in both
//! > snake_case and kebab-case so GREEN may serialize the WIT record either way.
//!
//! T14's result mapping (`mcp/server.rs:314-324`) returns the ids as **text content**
//! with `structured_content = None` and explicitly defers `has-more` / `next-cursor`
//! "to a later task". Surfacing the page as `structured_content` is therefore the
//! right-reason RED for every walk/boundary test, AND a plan-scope gap: the
//! `server.rs` result-mapping change is in **no** GREEN task's Files list (T16 lists
//! only `component.rs`). Flagged to Puck in the completion report.
//!
//! # Engine acquisition
//!
//! Acquisition-migrated onto the shared-engine fixture (tier-2 T5 sub-run,
//! R-0030/R-0029 — completes the 15-site set): [`setup_in_workspace`] acquires
//! the binary-wide shared engine via `shared_engine::shared_engine()` and
//! provisions its own fresh, isolated database via
//! `EmbeddedEngine::provision_test_database()` (which already runs the full
//! schema-init sequence — no redundant `init()` call needed). No per-file
//! boot-serialization mutex needed — the fixture's own get-or-init semantics
//! guarantee exactly-once boot.

#![allow(dead_code)]

// This file itself lives in `tests/common/` (it is pulled into each consumer
// binary via `#[path = "common/paging_harness.rs"]`), so a `#[path]` written
// HERE resolves relative to `tests/common/` — the bare filename, not
// `common/shared_engine.rs` (that form is correct only from a `tests/*.rs`
// includer's own directory, e.g. `admin_token.rs`).
#[path = "shared_engine.rs"]
mod shared_engine;

use mnemra_host::auth::token::{AdminToken, generate, hash};
use mnemra_host::mcp::server::{ECHO_PLUGIN_NAME, MnemraMcpServer};
use mnemra_host::plugin::pool::PluginPool;
use mnemra_host::storage::postgres::engine::{EmbeddedEngine, TestDatabase};
use rmcp::model::{CallToolRequestParams, CallToolResult, Meta};
use rmcp::service::{RoleClient, RunningService, serve_client, serve_server};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::duplex;
use uuid::Uuid;
use wasmtime::component::Component;

// ---------------------------------------------------------------------------
// MCP wiring (mirrors tests/mcp_server.rs — the proven list harness).
// ---------------------------------------------------------------------------

/// Seed an admin-role token into `admin_tokens`; return the raw token for `_meta`.
pub async fn seed_admin_token(pool: &sqlx::PgPool, workspace_id: Uuid) -> AdminToken {
    let token = generate();
    let token_hash = hash(&token);
    sqlx::query("INSERT INTO admin_tokens (token_hash, workspace_id, scopes) VALUES ($1, $2, $3)")
        .bind(token_hash.as_bytes())
        .bind(workspace_id)
        .bind(&vec!["admin".to_owned()])
        .execute(pool)
        .await
        .expect("INSERT admin token failed");
    token
}

/// Build a `Meta` carrying the auth token in the `token` key (open seam #1).
pub fn token_meta(token_str: &str) -> Meta {
    let mut meta = Meta::new();
    meta.insert("token".to_owned(), json!(token_str));
    meta
}

/// Path to the built `mnemra-echo` component (release `wasm32-wasip2`).
pub fn echo_component_path() -> PathBuf {
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

/// A live `PluginPool` with the loaded `mnemra-echo` component registered.
pub fn echo_plugin_pool() -> Arc<PluginPool> {
    let pool = PluginPool::new().expect("PluginPool::new");
    let component =
        Component::from_file(pool.engine(), echo_component_path()).expect("load echo component");
    pool.register_module(ECHO_PLUGIN_NAME, "0.0.1", &component)
        .expect("register echo component");
    Arc::new(pool)
}

/// A connected fixture: a shared-engine-provisioned per-test database, a running
/// MCP server over an in-process duplex transport, an rmcp client, and one seeded
/// admin token + its workspace.
pub struct Setup {
    pub db: TestDatabase,
    pub client: RunningService<RoleClient, ()>,
    pub server_handle: tokio::task::JoinHandle<()>,
    pub token: AdminToken,
    pub workspace_id: Uuid,
}

impl Setup {
    /// The live `PgPool` (for direct-SQL seeding).
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.db.pool
    }

    /// Graceful shutdown — cancel the client, join the server task.
    pub async fn shutdown(self) {
        let _ = self.client.cancel().await;
        let _ = self.server_handle.await;
    }
}

/// Acquire the shared engine + a fresh per-test database, seed one admin token,
/// and connect a client. `workspace_id` lets tenant-isolation tests pick an
/// explicit workspace.
pub async fn setup_in_workspace(workspace_id: Uuid) -> Setup {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let token = seed_admin_token(&db.pool, workspace_id).await;

    let server = MnemraMcpServer::new(db.pool.clone(), echo_plugin_pool());
    let (server_transport, client_transport) = duplex(8192);
    let server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => eprintln!("server init failed: {e:?}"),
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");

    Setup {
        db,
        client,
        server_handle,
        token,
        workspace_id,
    }
}

/// `setup_in_workspace` against the default workspace.
pub async fn setup() -> Setup {
    setup_in_workspace(mnemra_host::schema::init::DEFAULT_WORKSPACE_ID).await
}

// ---------------------------------------------------------------------------
// Direct-SQL bulk seeding (fast — bypasses N MCP/WASM create round-trips).
//
// The list call under test still traverses the full MCP surface; only the SEED
// is direct SQL (the same INSERT shape `artifact_create` uses, incl. the required
// frontmatter `id` + `frontmatter_version` CHECK keys). The black-box surface is
// the `echo.list` call, not the create path.
// ---------------------------------------------------------------------------

/// Insert `ids` as `echo_fixture` rows under `workspace_id` / `type_name`, in order.
pub async fn seed_artifacts(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    type_name: &str,
    ids: &[String],
) {
    for id in ids {
        let frontmatter = json!({
            "id": id,
            "frontmatter_version": 1,
            "msg": "paging_seed",
        })
        .to_string();
        sqlx::query(
            "INSERT INTO echo_fixture (id, workspace_id, type, frontmatter) \
             VALUES ($1, $2, $3, $4::jsonb)",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(type_name)
        .bind(&frontmatter)
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("seed INSERT failed for id {id}: {e}"));
    }
}

// ---------------------------------------------------------------------------
// Synthetic ascending ULIDs — valid 26-char Crockford base32, in-range.
// ---------------------------------------------------------------------------

const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// The maximum valid ULID — first char `7` (top of the 48-bit-ms range), rest `Z`.
/// Lexicographically greater than any `synthetic_ids` value → an out-of-range
/// (past-the-end) but well-formed cursor (R-0020-e: returns an empty page).
pub const MAX_ULID: &str = "7ZZZZZZZZZZZZZZZZZZZZZZZZZ";

/// A 26-char string of the EXCLUDED Crockford letter `I` — correct length, invalid
/// alphabet (R-0020-e scenario 455: alphabet-and-range validation, not a length check).
pub const WRONG_ALPHABET_ULID: &str = "IIIIIIIIIIIIIIIIIIIIIIIIII";

/// A clearly malformed cursor — not a 26-char ULID (R-0020-e scenario 395-a).
pub const MALFORMED_CURSOR: &str = "not-a-valid-ulid";

fn base32_padded(mut n: u128, width: usize) -> String {
    let mut buf = vec![b'0'; width];
    let mut i = width;
    while n > 0 && i > 0 {
        i -= 1;
        buf[i] = CROCKFORD[(n % 32) as usize];
        n /= 32;
    }
    String::from_utf8(buf).expect("crockford ascii")
}

/// `n` strictly-ascending, well-formed, in-range 26-char ULIDs. Each is `'0'`
/// (lowest Crockford char, safely in ULID range) + a 25-char zero-padded base32
/// counter, so lexicographic order == numeric order == seed order == keyset order.
pub fn synthetic_ids(n: usize) -> Vec<String> {
    (1..=n as u128)
        .map(|c| format!("0{}", base32_padded(c, 25)))
        .collect()
}

// ---------------------------------------------------------------------------
// The page wire contract — extraction + the MCP list call.
// ---------------------------------------------------------------------------

/// The decoded `artifact-page` record as observed at the MCP `CallToolResult`.
#[derive(Debug, Clone)]
pub struct Page {
    pub ids: Vec<String>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

/// Decode the `artifact-page` from the `CallToolResult.structured_content` (the
/// single pinned wire contract — see the module header). `Err` carries a
/// right-reason-RED message when the page is not surfaced as `structured_content`.
pub fn extract_page(result: &CallToolResult) -> Result<Page, String> {
    let sc = result.structured_content.as_ref().ok_or_else(|| {
        "R-0020 page contract not surfaced: CallToolResult.structured_content is None. \
         The list result must carry the artifact-page record as structured_content \
         { ids, has_more, next_cursor }. T14 returns ids-only text content with \
         structured_content = None — GREEN (server.rs:314-324 result mapping, see \
         handoff) must populate it."
            .to_owned()
    })?;
    let obj = sc
        .as_object()
        .ok_or_else(|| format!("structured_content is not a JSON object: {sc}"))?;
    let ids = obj
        .get("ids")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("page.ids missing/not an array in {sc}"))?
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_owned())
        .collect();
    let has_more = obj
        .get("has_more")
        .or_else(|| obj.get("has-more"))
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("page.has_more missing/not a bool in {sc}"))?;
    let next_cursor = obj
        .get("next_cursor")
        .or_else(|| obj.get("next-cursor"))
        .and_then(|v| {
            if v.is_null() {
                None
            } else {
                v.as_str().map(str::to_owned)
            }
        });
    Ok(Page {
        ids,
        has_more,
        next_cursor,
    })
}

/// Call `echo.list` for `content_type` with empty `filters`, optionally carrying
/// `limit` and/or `cursor`. Returns the raw MCP result (Ok) or JSON-RPC error (Err).
pub async fn list_call(
    setup: &Setup,
    content_type: &str,
    limit: Option<u32>,
    cursor: Option<&str>,
) -> Result<CallToolResult, rmcp::ServiceError> {
    let mut params = CallToolRequestParams::new("echo.list");
    params.meta = Some(token_meta(setup.token.as_str()));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!(content_type));
        m.insert("filters".to_owned(), json!("{}"));
        if let Some(l) = limit {
            m.insert("limit".to_owned(), json!(l));
        }
        if let Some(c) = cursor {
            m.insert("cursor".to_owned(), json!(c));
        }
        m
    });
    setup.client.call_tool(params).await
}

/// Walk `content_type` from `cursor = none` following `next_cursor` until
/// `has_more = false`. Returns the concatenated ids and the per-page records. Panics
/// (right-reason) if the page is not surfaced, or guards against an infinite cursor
/// loop. `max_pages` bounds the walk so a non-terminating bug fails loudly.
pub async fn walk_all(
    setup: &Setup,
    content_type: &str,
    max_pages: usize,
) -> (Vec<String>, Vec<Page>) {
    let mut cursor: Option<String> = None;
    let mut all_ids: Vec<String> = Vec::new();
    let mut pages: Vec<Page> = Vec::new();
    for _ in 0..max_pages {
        let result = list_call(setup, content_type, None, cursor.as_deref())
            .await
            .expect("echo.list must return Ok during a keyset walk");
        let page = extract_page(&result).unwrap_or_else(|e| panic!("{e}"));
        all_ids.extend(page.ids.iter().cloned());
        let has_more = page.has_more;
        let next = page.next_cursor.clone();
        pages.push(page);
        if !has_more {
            return (all_ids, pages);
        }
        cursor = Some(next.expect("R-0020-b: has_more = true requires next_cursor = some"));
    }
    panic!(
        "keyset walk did not terminate within {max_pages} pages — possible infinite cursor loop"
    );
}
