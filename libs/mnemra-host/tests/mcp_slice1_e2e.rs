//! Slice-1 end-to-end MCP acceptance tests — RED phase (dispatch #1084, Task 23).
//!
//! # Purpose
//!
//! Pins the contract for the full slice-1 walking skeleton:
//! MCP `call_tool` → auth (exists) → pool component invoke →
//! guest `content.create` export → host `artifact-create` import
//! (Branch-2 fenced in-memory stub) → typed ULID return →
//! `CallToolResult` → readback via `echo.get`.
//!
//! # R-ID mapping
//!
//! | Test function                                             | R-ID(s)                    |
//! |-----------------------------------------------------------|----------------------------|
//! | echo_create_returns_well_formed_ulid_and_get_round_trips  | R-0019, R-0006-b/d, R-0010 |
//! | cross_workspace_get_returns_none                          | R-0006-d                   |
//!
//! # RED-phase design
//!
//! The helper `slice1_echo_harness` is `unimplemented!()`. Every test panics at
//! the harness boundary — this is the sanctioned red. The tests COMPILE; they fail
//! at runtime for the right reason (helper not yet implemented), not from a typo.
//! The ULID-format and readback assertions are the green-phase contract for Forge;
//! their correctness is verified by compilation and the strong assertion text, not
//! by execution in this phase.
//!
//! # verify: []
//!
//! `verify: []` is intentional for a red-phase dispatch (fails by design).
//! There is no just recipe to run against a helper that panics before dispatch.
//! Green phase adds the recipe.
//!
//! # ULID validation
//!
//! No `regex` or `ulid` crate is used — neither is in dev-deps, and adding them
//! would trigger the dep-gate. Crockford base32 validation is done inline:
//! 26 chars, each in `0-9 A-H J-N P-T V-Z` (excludes I, L, O, U).
//!
//! # `echo.get` argument shape (reconciliation point)
//!
//! CC-MAPPING (the plan) pins `echo.create` → `content.create` with
//! `{content_type, payload}`. The `echo.get` argument shape (`{id: "<ULID>"}`)
//! is NOT separately pinned in CC-MAPPING. This test uses `{id}` as the argument
//! key — Forge must confirm this matches the manifest or amend accordingly
//! (flagged in the completion report as reconciliation point R1).
//!
//! # Content extraction
//!
//! `CallToolResult.content` is `Vec<Content>` where `Content = Annotated<RawContent>`.
//! Text is accessed via `content.raw` → `RawContent::Text(RawTextContent { text, .. })`.
//! This test extracts and asserts on `.text` from the first text content item.
//!
//! # Engine lifetime
//!
//! Each test starts its own embedded Postgres via `start_engine()`. The `engine`
//! binding MUST remain in scope for the entire test — dropping it tears down the
//! embedded Postgres. The pool is cloned (PgPool is Arc-backed) and passed to the
//! harness; the test holds the engine.

// Wire in only the slice1_harness sub-module — avoids compiling the storage
// contract helpers (which assert on types unrelated to MCP) into this binary
// and avoids unused-symbol warnings across test binaries.
#[path = "common/slice1_harness.rs"]
mod slice1_harness;

use mnemra_host::auth::token::{generate, hash};
use mnemra_host::schema::init::init;
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use rmcp::model::{CallToolRequestParams, Meta, RawContent};
use serde_json::json;
use slice1_harness::slice1_echo_harness;
use std::sync::Mutex;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Serialises engine startup across concurrent test threads within this binary.
/// Mirrors the same lock in `mcp_server.rs` (A-11 design decision).
static STARTUP_LOCK: Mutex<()> = Mutex::new(());

/// Start a fresh embedded engine with startup serialised (A-11).
async fn start_engine() -> EmbeddedEngine {
    {
        let _guard = STARTUP_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    }
    EmbeddedEngine::start()
        .await
        .expect("failed to start embedded Postgres")
}

/// Build a `Meta` carrying the auth token in the `token` key.
/// Mirrors `token_meta` from `mcp_server.rs` (same open seam #1 — per-request
/// `_meta` is the spec-faithful extension point for DF-auth-check per R-0010-c).
fn token_meta(token_str: &str) -> Meta {
    let mut meta = Meta::new();
    meta.insert("token".to_owned(), json!(token_str));
    meta
}

/// Validate that `s` is a well-formed ULID: 26 chars, Crockford base32 alphabet.
///
/// Crockford base32: digits 0-9 and uppercase letters A-Z excluding I, L, O, U.
/// No `regex` or `ulid` crate — validated inline to avoid the dep-gate.
fn is_valid_ulid(s: &str) -> bool {
    if s.len() != 26 {
        return false;
    }
    s.chars()
        .all(|c| "0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(c))
}

/// Extract all text strings from a `CallToolResult`'s content vector.
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

// ---------------------------------------------------------------------------
// Test A: echo.create returns a well-formed ULID and echo.get round-trips
// ---------------------------------------------------------------------------

/// R-0019, R-0006-b/d, R-0010 — echo.create returns a real ULID; echo.get returns the payload.
///
/// # Given / When / Then
///
/// GIVEN a slice-1 harness (connected MCP client, seeded admin token, workspace_id)
/// WHEN client sends `call_tool` for `echo.create` with a recognisable payload UUID
/// THEN the result carries a well-formed ULID (26 chars, Crockford base32 alphabet)
///   AND it is not empty / not a vacuous default
/// AND WHEN client sends `call_tool` for `echo.get` with the returned ULID
/// THEN the result content round-trips the exact payload string written
///
/// # Assertion strength
///
/// Asserting `is_ok()` alone would green vacuously against a stub returning
/// `CallToolResult::default()` — the same failure mode Task 23 unit-2 hit.
/// This test asserts the REAL ULID FORMAT and the REAL READBACK CONTENT, so it
/// FAILS if create returns a constant/empty ULID or get returns nothing.
///
/// # Red reason
///
/// `slice1_echo_harness` panics with `unimplemented!()` before any assertion runs.
/// This is the right-reason red.
#[tokio::test]
async fn echo_create_returns_well_formed_ulid_and_get_round_trips() {
    // GIVEN: a slice-1 harness
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref().clone();

    // A recognisable test string that round-trips through the create→get path.
    // Using a fresh Uuid ensures the payload is unique per test run; the test
    // asserts the exact string is present in the get response.
    let recognisable_payload = format!("slice1_e2e_payload_{}", Uuid::new_v4());

    let harness = slice1_echo_harness(pool).await;

    // WHEN: call echo.create with the recognisable payload
    let mut create_params = CallToolRequestParams::new("echo.create");
    create_params.meta = Some(token_meta(harness.admin_token.as_str()));
    create_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert("payload".to_owned(), json!(recognisable_payload));
        m
    });

    let create_result = harness
        .client
        .call_tool(create_params)
        .await
        .expect("R-0019, R-0010: echo.create with valid admin token must return Ok");

    // THEN: result must contain a well-formed ULID in its text content
    let texts = extract_text_content(&create_result);
    assert!(
        !texts.is_empty(),
        "R-0019, R-0006-b: echo.create result must have at least one text content item; \
         got content: {:?}",
        create_result.content
    );

    // Find the ULID — it appears as text in the result content
    let ulid_str = texts
        .iter()
        .find(|t| is_valid_ulid(t.trim()))
        .map(|t| t.trim())
        .unwrap_or_else(|| {
            panic!(
                "R-0019: echo.create result must contain a well-formed ULID (26 chars, \
                 Crockford base32 [0-9A-HJKMNP-TV-Z]{{26}}); \
                 got text content: {:?}",
                texts
            )
        });

    // Assert ULID is well-formed (redundant after find, but explicit for clarity)
    assert!(
        is_valid_ulid(ulid_str),
        "R-0019: returned ULID '{}' must be 26 Crockford base32 chars",
        ulid_str
    );

    // Assert not empty string
    assert!(
        !ulid_str.is_empty(),
        "R-0019: echo.create must not return an empty ULID"
    );

    // AND WHEN: call echo.get with the returned ULID
    // Reconciliation point R1: the `{id}` argument key is assumed here; Forge
    // must confirm this matches the echo manifest's echo.get verb schema.
    let mut get_params = CallToolRequestParams::new("echo.get");
    get_params.meta = Some(token_meta(harness.admin_token.as_str()));
    get_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(ulid_str));
        m
    });

    let get_result = harness
        .client
        .call_tool(get_params)
        .await
        .expect("R-0006-d, R-0010: echo.get with valid admin token must return Ok");

    // THEN: result content must contain the recognisable payload
    let get_texts = extract_text_content(&get_result);
    let payload_found = get_texts.iter().any(|t| t.contains(&recognisable_payload));

    assert!(
        payload_found,
        "R-0019, R-0006-d: echo.get must round-trip the written payload; \
         expected text content to contain '{}', got: {:?}",
        recognisable_payload, get_texts
    );
}

// ---------------------------------------------------------------------------
// Test B: cross-workspace get returns none (workspace isolation)
// ---------------------------------------------------------------------------

/// R-0006-d — artifact created in workspace A is invisible from workspace B.
///
/// # Given / When / Then
///
/// GIVEN an artifact created in workspace A (via the slice-1 harness)
///   AND a second admin token seeded for workspace B (a different UUID)
/// WHEN `echo.get` for that artifact id is issued under workspace B's token
/// THEN the result is not-found / empty (workspace isolation at the fenced-map level)
///
/// # Red reason
///
/// `slice1_echo_harness` panics before any workspace-B seeding occurs.
/// Right-reason red: the harness is not yet implemented.
///
/// # Workspace B seeding
///
/// `admin_tokens.workspace_id` is UUID NOT NULL with no FK to the `workspaces`
/// table (schema init.rs migration 7). Workspace B's token is seeded with a
/// fresh Uuid4 without needing a `workspaces` row — the only constraint is
/// `workspace_id NOT NULL`.
#[tokio::test]
async fn cross_workspace_get_returns_none() {
    // GIVEN: a slice-1 harness for workspace A
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref().clone();

    let harness = slice1_echo_harness(pool.clone()).await;

    // Create an artifact in workspace A
    let recognisable_payload = format!("workspace_isolation_payload_{}", Uuid::new_v4());
    let mut create_params = CallToolRequestParams::new("echo.create");
    create_params.meta = Some(token_meta(harness.admin_token.as_str()));
    create_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert("payload".to_owned(), json!(recognisable_payload));
        m
    });

    let create_result = harness
        .client
        .call_tool(create_params)
        .await
        .expect("R-0006-d setup: echo.create must return Ok for workspace A");

    let texts = extract_text_content(&create_result);
    let ulid_str = texts
        .iter()
        .find(|t| is_valid_ulid(t.trim()))
        .map(|t| t.trim())
        .unwrap_or_else(|| {
            panic!(
                "R-0006-d setup: echo.create must return a well-formed ULID; \
                 got text content: {:?}",
                texts
            )
        });

    // Seed a token for workspace B — a different workspace_id
    let workspace_b_id = Uuid::new_v4();
    let token_b = generate();
    let token_b_hash = hash(&token_b);

    // workspace_id has no FK to workspaces table (migration 7); seeding a token
    // for an arbitrary Uuid is safe without a workspaces row.
    sqlx::query(
        "INSERT INTO admin_tokens (token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3)",
    )
    .bind(token_b_hash.as_bytes())
    .bind(workspace_b_id)
    .bind(&vec!["admin".to_owned()])
    .execute(&pool)
    .await
    .expect("INSERT workspace-B admin token failed");

    // WHEN: echo.get for the workspace-A artifact id, using workspace B's token
    let mut get_params = CallToolRequestParams::new("echo.get");
    get_params.meta = Some(token_meta(token_b.as_str()));
    get_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(ulid_str));
        m
    });

    let get_result = harness.client.call_tool(get_params).await;

    // THEN: the get must NOT return the workspace-A artifact — either an error
    // (not-found MCP error) or an Ok result with empty/no content that does NOT
    // contain the payload.
    //
    // We assert workspace isolation: the recognisable payload must NOT appear.
    match get_result {
        Err(_) => {
            // An error (not-found, auth) confirms isolation — workspace B cannot
            // resolve the artifact.
        }
        Ok(ref result) => {
            let get_texts = extract_text_content(result);
            assert!(
                !get_texts.iter().any(|t| t.contains(&recognisable_payload)),
                "R-0006-d: workspace isolation violated — workspace B's echo.get returned \
                 workspace A's artifact payload '{}'; fenced-map must key by workspace_id. \
                 Got text content: {:?}",
                recognisable_payload,
                get_texts
            );
        }
    }
}
