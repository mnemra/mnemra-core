//! R-0007-h invoke-path health gate — RED phase (dispatch #1093, Task 1807 / T9).
//!
//! # Purpose
//!
//! Pins the contract for the invoke-path epoch-health gate (R-0007-h). The MCP
//! handler MUST refuse `call_tool` invocations while the epoch-tick supervisor is
//! degraded, returning a structured error with a distinct code. A healthy supervisor
//! MUST NOT block invocations (regression guard).
//!
//! # R-ID mapping
//!
//! | Test function                                        | R-ID(s)    |
//! |------------------------------------------------------|------------|
//! | degraded_supervisor_blocks_call_tool                 | R-0007-h   |
//! | healthy_supervisor_allows_call_tool                  | R-0007-h   |
//!
//! # RED-phase design
//!
//! Test `degraded_supervisor_blocks_call_tool` MUST FAIL against current code.
//! The gap (R-0007-h, half-satisfied): `pool.can_invoke()` and `epoch_health()`
//! exist but nothing in the invoke path calls them. So today `call_tool("echo.create",
//! ...)` with an injected-dead epoch thread runs the invoke and returns `Ok(ULID)`.
//! The test asserts `Err` with code `-4006`, so it fails at `expect_err` — the
//! actual `Ok` response is surfaced in the failure message, proving the gate is absent.
//! That IS the right-reason red.
//!
//! Test `healthy_supervisor_allows_call_tool` is green before and after the fix
//! (regression guard: a gate that unconditionally refuses would satisfy the degraded
//! case but break this one).
//!
//! # Distinct error codes (R-0010-f class)
//!
//! `SUPERVISOR_DEGRADED_CODE` (`-4006`) does not exist yet — Forge adds it in the
//! green phase. Tests assert against the raw `rmcp::model::ErrorCode(-4006)` value
//! to keep the file compilable without importing a named const from `errors.rs`
//! (which is in `forbid_scope`). Existing codes for reference:
//!   -4001: AUTH_FAILURE_CODE
//!   -4002: PERMISSION_DENIED_CODE
//!   -4003: NON_DISPATCHABLE_CODE
//!   -4004: PLUGIN_EXEC_CODE
//!   -4005: VERB_NOT_EXPOSED_CODE
//!   -4006: SUPERVISOR_DEGRADED (new — this gate)
//!
//! # verify: []
//!
//! `verify: []` by design — the suite is red against current code. The recipe is
//! added by the green phase (Forge).
//!
//! # Fault-injection seam
//!
//! `Slice1Harness.plugin_pool.inject_epoch_death_for_test()` (added in this red
//! phase; `#[cfg(feature = "test-hooks")]`) delegates to the existing
//! `EpochTickThread::inject_death_for_test()` seam. The harness now holds an
//! `Arc<PluginPool>` pointing to the SAME pool the server's invoke path reads,
//! so injecting death here degrades the live invoke path.

#[path = "common/slice1_harness.rs"]
mod slice1_harness;

use rmcp::model::CallToolRequestParams;
#[cfg(feature = "test-hooks")]
use rmcp::model::ErrorCode;
use serde_json::json;
use slice1_harness::slice1_echo_harness;
use std::sync::Mutex;

use mnemra_host::schema::init::init;
use mnemra_host::storage::postgres::engine::EmbeddedEngine;

/// Serialises engine startup across concurrent test threads within this binary (A-11).
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
/// Mirrors `token_meta` from `mcp_server.rs` (same open seam #1).
fn token_meta(token_str: &str) -> rmcp::model::Meta {
    let mut meta = rmcp::model::Meta::new();
    meta.insert("token".to_owned(), json!(token_str));
    meta
}

// ===========================================================================
// Test 1 (RED): Degraded supervisor blocks call_tool with -4006
// ===========================================================================

/// R-0007-h — degraded epoch-tick supervisor blocks `call_tool` with distinct code.
///
/// # Given / When / Then
///
/// GIVEN a slice-1 harness with a valid admin token
///   AND the pool's epoch-tick thread is injected-dead (Degraded)
/// WHEN `call_tool("echo.create", {content_type, payload})` is invoked with the admin token
/// THEN the result is Err with code ErrorCode(-4006) — SUPERVISOR_DEGRADED — distinct from:
///   - -4001 (AUTH_FAILURE)
///   - -4002 (PERMISSION_DENIED)
///   - -4003 (NON_DISPATCHABLE)
///   - -4004 (PLUGIN_EXEC)
///   - -4005 (VERB_NOT_EXPOSED)
///
/// # Right-reason red
///
/// Against CURRENT code: no invoke-path health gate exists. With an injected-dead
/// epoch thread, `echo.create` reaches the plugin and returns `Ok(CallToolResult {
/// ... ULID ... })`. The `expect_err("…")` call panics showing the actual `Ok`
/// response — that is the right-reason red. An `is_ok()` assertion would NOT
/// discriminate (the call currently succeeds). We assert `is_err()` + the exact
/// distinct code.
///
/// After Forge's green phase: the gate calls `pool.can_invoke()` before dispatch,
/// finds it returning `false`, and returns Err(-4006). This test passes.
///
/// # Why echo.create with valid args
///
/// Using a valid, dispatchable verb with correct args ensures the failure can ONLY
/// be the health gate — not bad verb, not permission, not arg parsing. If the dead
/// epoch thread broke dispatch for a different reason (e.g. epoch deadline fires,
/// returning -4004), that is a wrong-reason red and must be investigated. The
/// `expect_err` message surfaces the actual Ok so the failure message proves the
/// gate is absent today.
#[cfg(feature = "test-hooks")]
#[tokio::test]
async fn degraded_supervisor_blocks_call_tool() {
    // R-0007-h: degraded epoch-tick supervisor must block call_tool.
    // GIVEN: a slice-1 harness with a valid admin token
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref().clone();

    let harness = slice1_echo_harness(pool).await;

    // GIVEN: the epoch-tick thread is injected-dead (Degraded).
    // This calls EpochTickThread::inject_death_for_test() on the same pool the
    // MnemraMcpServer's invoke path reads — same Arc.
    harness.plugin_pool.inject_epoch_death_for_test();

    // WHEN: call echo.create with valid args and the admin token.
    // Valid dispatchable verb + correct args + admin token: any Err here is the gate.
    let mut params = CallToolRequestParams::new("echo.create");
    params.meta = Some(token_meta(harness.admin_token.as_str()));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert("payload".to_owned(), json!({"msg": "health_gate_probe"}));
        m
    });

    let result = harness.client.call_tool(params).await;

    // THEN: must be Err — current code returns Ok here (right-reason red).
    // Failure message must show the actual Ok response (proving the gate is absent).
    let err = result.expect_err(
        "R-0007-h: epoch-tick supervisor is Degraded (injected-dead); \
         the invoke-path health gate must refuse call_tool with Err. \
         Current code (no gate) returns Ok with a ULID — this is the right-reason red. \
         After Forge's green phase: pool.can_invoke() returns false → Err(-4006).",
    );

    // THEN: error code must be the distinct SUPERVISOR_DEGRADED code (-4006).
    // Do NOT import from errors.rs (forbid_scope) — assert raw value.
    match err {
        rmcp::ServiceError::McpError(ref error_data) => {
            // Primary assertion: the gate emits the distinct -4006 code.
            assert_eq!(
                error_data.code,
                ErrorCode(-4006),
                "R-0007-h: degraded supervisor must return ErrorCode(-4006) \
                 (SUPERVISOR_DEGRADED); got {:?}. \
                 The code must be DISTINCT from: \
                 -4001 (auth), -4002 (permission), -4003 (non-dispatchable), \
                 -4004 (plugin-exec), -4005 (verb-not-exposed).",
                error_data.code
            );

            // Guard: not an auth failure — token is valid.
            assert_ne!(
                error_data.code,
                ErrorCode(-4001),
                "R-0007-h: supervisor-degraded MUST NOT return auth-failure code (-4001); \
                 the admin token is valid."
            );

            // Guard: not permission-denied — role is admin.
            assert_ne!(
                error_data.code,
                ErrorCode(-4002),
                "R-0007-h: supervisor-degraded MUST NOT return permission-denied code (-4002); \
                 the token has admin scope."
            );

            // Guard: not non-dispatchable — echo.create has a typed export.
            assert_ne!(
                error_data.code,
                ErrorCode(-4003),
                "R-0007-h: supervisor-degraded MUST NOT return non-dispatchable code (-4003); \
                 echo.create is declared in the manifest and has a typed export."
            );

            // Guard: not plugin-exec — the gate fires before dispatch.
            assert_ne!(
                error_data.code,
                ErrorCode(-4004),
                "R-0007-h: supervisor-degraded MUST NOT return plugin-exec code (-4004); \
                 the health gate fires before the plugin is invoked."
            );

            // Guard: not verb-not-exposed — echo.create is in the manifest verbs list.
            assert_ne!(
                error_data.code,
                ErrorCode(-4005),
                "R-0007-h: supervisor-degraded MUST NOT return verb-not-exposed code (-4005); \
                 echo.create is declared in the echo manifest verbs list."
            );
        }
        other => panic!(
            "R-0007-h: expected ServiceError::McpError for echo.create with degraded supervisor; \
             got {:?}",
            other
        ),
    }
}

// ===========================================================================
// Test 2 (GREEN before and after): Healthy supervisor allows call_tool
// ===========================================================================

/// R-0007-h — healthy epoch-tick supervisor allows `call_tool` to proceed.
///
/// # Regression guard
///
/// With no fault injection, `echo.create` with valid args and an admin token must
/// return `Ok` containing a well-formed ULID. This guards against an unconditional
/// block (a gate that always refuses would satisfy the degraded case but fail here).
///
/// # Given / When / Then
///
/// GIVEN a slice-1 harness with a valid admin token
///   AND the epoch-tick thread is in its default healthy state (no injection)
/// WHEN `call_tool("echo.create", {content_type, payload})` is invoked with the admin token
/// THEN the result is Ok (the health gate must NOT block a healthy invoke path).
///
/// # Green before and after
///
/// Today and after the green phase: the epoch thread is healthy; the gate (when added)
/// calls `pool.can_invoke()` → returns `true` → dispatch proceeds → Ok with ULID.
/// This test remains green throughout.
///
/// # Relationship to mcp_verb_gate.rs test 3
///
/// `exposed_dispatchable_verb_still_succeeds` in `mcp_verb_gate.rs` covers the same
/// happy-path surface from the verb-gate angle. This test is kept here so the health-
/// gate boundary tests are self-contained: the healthy-supervisor positive case is
/// documented alongside the degraded-supervisor negative case.
#[tokio::test]
async fn healthy_supervisor_allows_call_tool() {
    // R-0007-h: healthy epoch-tick supervisor must NOT block call_tool.
    // GIVEN: a slice-1 harness with a valid admin token (no fault injection)
    let engine = start_engine().await;
    init(&engine, "vector").await.expect("init should succeed");
    let pool = engine.pool.as_ref().clone();

    let harness = slice1_echo_harness(pool).await;

    // WHEN: call echo.create — declared in manifest, has typed export, healthy pool.
    let mut params = CallToolRequestParams::new("echo.create");
    params.meta = Some(token_meta(harness.admin_token.as_str()));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert("payload".to_owned(), json!({"msg": "healthy_gate_probe"}));
        m
    });

    let result = harness.client.call_tool(params).await;

    // THEN: must be Ok — the health gate must NOT block a healthy invoke path.
    // If Err is returned here, the gate is unconditional (a bug, not a feature).
    result.expect(
        "R-0007-h regression guard: epoch-tick supervisor is healthy (no injection); \
         the invoke-path health gate MUST NOT block call_tool when pool.can_invoke() is true. \
         An unconditional block would satisfy the degraded test but is a correctness bug.",
    );
}
