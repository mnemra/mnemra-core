//! R-0010-d manifest-verbs capability gate — RED phase (dispatch #1088, Task 1801).
//!
//! # Purpose
//!
//! Pins the contract for the per-verb manifest capability gate (R-0010-d). The MCP
//! handler MUST reject, pre-dispatch, any verb that is not in the registered plugin's
//! manifest `verbs` list, returning a structured error with a distinct code.
//!
//! # R-ID mapping
//!
//! | Test function                                             | R-ID(s)              |
//! |-----------------------------------------------------------|----------------------|
//! | unexposed_verb_rejected_pre_dispatch                      | R-0010-d, R-0019-c   |
//! | exposed_but_undispatchable_verb_still_non_dispatchable    | R-0010-d, R-0019-c   |
//! | exposed_dispatchable_verb_still_succeeds                  | R-0010-d, R-0019-c   |
//! | readobserver_unexposed_read_verb_rejected                 | R-0010-d, R-0009-d   |
//!
//! # RED-phase design
//!
//! Test #1 (`unexposed_verb_rejected_pre_dispatch`) MUST FAIL against current code.
//! Current code has no manifest-verbs gate: `evil.create` (not in the echo manifest)
//! reaches the plugin and currently returns `Ok` with a ULID. The test asserts `Err`
//! with code `-4005`, so it fails at the `expect_err` call — right-reason red.
//!
//! Tests #2 and #3 are green before and after the fix (precision guard + regression
//! guard).
//!
//! # Distinct error codes (R-0010-f class)
//!
//! `VERB_NOT_EXPOSED_CODE` (`-4005`) does not exist yet — Forge adds it in the green
//! phase. Tests assert against the raw `rmcp::model::ErrorCode(-4005)` value to
//! keep the file compilable without importing a named const from `errors.rs`
//! (which is in `forbid_scope`). Existing codes for reference:
//!   -4001: AUTH_FAILURE_CODE
//!   -4002: PERMISSION_DENIED_CODE
//!   -4003: NON_DISPATCHABLE_CODE
//!   -4004: PLUGIN_EXEC_CODE
//!   -4005: VERB_NOT_EXPOSED (new — this gate)
//!
//! # verify: []
//!
//! `verify: []` by design — the suite is red against current code. The recipe is
//! added by the green phase.
//!
//! # Echo manifest declared verbs
//!
//! The `mnemra-echo` manifest declares exactly these exposed verbs:
//!   `echo.create`, `echo.get`, `echo.list`, `echo.update`, `echo.delete`, `echo.audit`
//! `echo.audit` is declared but has no typed export (NON_DISPATCHABLE).
//! `evil.create` is NOT declared — the probe for the new gate.
//!
//! # Engine acquisition
//!
//! Acquisition-migrated onto the shared-engine fixture (T3 sub-run,
//! R-0030/R-0029): each test acquires the binary-wide shared engine via
//! `shared_engine::shared_engine()` and provisions its own fresh, isolated
//! database via `EmbeddedEngine::provision_test_database()` (which already
//! runs the full schema-init sequence — no redundant `init()` call needed).
//! No per-file boot-serialization mutex needed — the fixture's own
//! get-or-init semantics guarantee exactly-once boot.

#[path = "common/shared_engine.rs"]
mod shared_engine;
#[path = "common/slice1_harness.rs"]
mod slice1_harness;

use rmcp::model::{CallToolRequestParams, ErrorCode, Meta, RawContent};
use serde_json::json;
use slice1_harness::slice1_echo_harness;

use mnemra_host::auth::token::{AdminToken, generate, hash};
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use uuid::Uuid;

/// Seed a read_observer-scoped token into `admin_tokens` and return the raw AdminToken.
///
/// Mirrors `seed_read_observer_token` in `mcp_server.rs` — that function is private
/// to its binary and not importable here; inlined to match this file's established
/// pattern for `token_meta`, `is_valid_ulid`, and `extract_text_content`.
async fn seed_read_observer_token(pool: &sqlx::PgPool, workspace_id: Uuid) -> AdminToken {
    let token = generate();
    let token_hash = hash(&token);
    sqlx::query(
        "INSERT INTO admin_tokens (token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3)",
    )
    .bind(token_hash.as_bytes())
    .bind(workspace_id)
    .bind(&vec!["read_observer".to_owned()])
    .execute(pool)
    .await
    .expect("seed read_observer token");
    token
}

/// Build a `Meta` carrying the auth token in the `token` key.
/// Mirrors `token_meta` from `mcp_server.rs` (same open seam #1).
fn token_meta(token_str: &str) -> Meta {
    let mut meta = Meta::new();
    meta.insert("token".to_owned(), json!(token_str));
    meta
}

/// Validate that `s` is a well-formed ULID: 26 chars, Crockford base32 alphabet.
/// Copied inline from `mcp_slice1_e2e.rs` — private there, not importable.
fn is_valid_ulid(s: &str) -> bool {
    if s.len() != 26 {
        return false;
    }
    s.chars()
        .all(|c| "0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(c))
}

/// Extract all text strings from a `CallToolResult`'s content vector.
/// Copied inline from `mcp_slice1_e2e.rs` — private there, not importable.
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

// ===========================================================================
// Test 1 (RED): Unexposed verb rejected pre-dispatch with distinct error code
// ===========================================================================

/// R-0010-d, R-0019-c — verb not in manifest `verbs` list is rejected pre-dispatch.
///
/// # Given / When / Then
///
/// GIVEN a valid admin token (admin scope, so this is independent of role-gate ordering)
/// WHEN `call_tool` is invoked with verb `evil.create` (NOT in the echo manifest `verbs`)
/// THEN the result is Err with code ErrorCode(-4005) — distinct from:
///   - -4001 (AUTH_FAILURE)
///   - -4002 (PERMISSION_DENIED)
///   - -4003 (NON_DISPATCHABLE)
///   - -4004 (PLUGIN_EXEC)
/// AND no artifact is created (the call returns Err, no ULID in content).
///
/// # Right-reason red
///
/// Against CURRENT code: no manifest-verbs gate exists. `evil.create` reaches the
/// plugin and the handler currently returns `Ok(CallToolResult { ... ULID ... })`.
/// The `expect_err("…")` call panics showing the actual `Ok` response — that is
/// the right-reason red. An `is_ok()` assertion would NOT discriminate (the call
/// currently succeeds). We assert `is_err()` + the exact distinct code.
///
/// After Forge's green phase: the gate rejects `evil.create` before dispatch with
/// code -4005, and this test passes.
#[tokio::test]
async fn unexposed_verb_rejected_pre_dispatch() {
    // R-0010-d, R-0019-c: verb not in manifest verbs list → rejected pre-dispatch.
    // GIVEN: a slice-1 harness with a valid admin token
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let harness = slice1_echo_harness(db.pool.clone()).await;

    // WHEN: call echo server with a verb NOT in the manifest verbs list.
    // `evil.create` is the probe — not declared in the echo manifest.
    // An admin token is used so the test is independent of role-gate ordering:
    // the manifest gate fires after auth (DF-auth-check) but before dispatch,
    // and an admin token passes auth unconditionally.
    let mut params = CallToolRequestParams::new("evil.create");
    params.meta = Some(token_meta(harness.admin_token.as_str()));
    // Arguments provided (same as echo.create) so the call would succeed if
    // the gate were absent — this ensures the failure is gating, not args.
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert("payload".to_owned(), json!({"msg": "gate_probe"}));
        m
    });

    let result = harness.client.call_tool(params).await;

    // THEN: must be Err — current code returns Ok here (right-reason red).
    // Failure message must show the actual Ok response (proving the gate is absent).
    let err = result.expect_err(
        "R-0010-d: evil.create is NOT in the echo manifest verbs list; \
         the manifest-verbs gate must reject it pre-dispatch with Err. \
         Current code (pre-gate) returns Ok with a ULID — this is the right-reason red.",
    );

    // THEN: error code must be the distinct VERB_NOT_EXPOSED code (-4005).
    // Do NOT import from errors.rs (forbid_scope) — assert raw value.
    match err {
        rmcp::ServiceError::McpError(ref error_data) => {
            // Primary assertion: the gate emits the distinct -4005 code.
            assert_eq!(
                error_data.code,
                ErrorCode(-4005),
                "R-0010-d/R-0010-f: unexposed verb must return ErrorCode(-4005) \
                 (VERB_NOT_EXPOSED); got {:?}. \
                 The code must be DISTINCT from: \
                 -4001 (auth), -4002 (permission), -4003 (non-dispatchable), -4004 (plugin-exec).",
                error_data.code
            );

            // Guard: not an auth failure (token is valid).
            assert_ne!(
                error_data.code,
                ErrorCode(-4001),
                "R-0010-f: verb-not-exposed MUST NOT return auth-failure code (-4001); \
                 token is valid, only the verb is unexposed"
            );

            // Guard: not a permission-denied error (role is admin).
            assert_ne!(
                error_data.code,
                ErrorCode(-4002),
                "R-0010-f: verb-not-exposed MUST NOT return permission-denied code (-4002); \
                 token has admin scope"
            );

            // Guard: not non-dispatchable (that is for declared-but-unexported verbs).
            assert_ne!(
                error_data.code,
                ErrorCode(-4003),
                "R-0010-f: verb-not-exposed MUST NOT return non-dispatchable code (-4003); \
                 -4003 is for verbs declared in the manifest but lacking a typed export; \
                 evil.create is not declared at all"
            );
        }
        other => panic!(
            "R-0010-d: expected ServiceError::McpError for evil.create; got {:?}",
            other
        ),
    }
}

// ===========================================================================
// Test 2 (GREEN before and after): Declared but undispatchable verb → -4003
// ===========================================================================

/// R-0010-d, R-0019-c — declared verb with no typed export still returns NON_DISPATCHABLE.
///
/// # Precision guard
///
/// `echo.audit` is declared in the manifest's `verbs` list (it IS exposed).
/// However, it has no corresponding typed export in the echo component.
/// The manifest-verbs gate MUST pass `echo.audit` through (it is declared),
/// and the existing NON_DISPATCHABLE path (-4003) MUST fire as before.
///
/// This guards that the new gate does NOT over-reject: it must only block verbs
/// that are ABSENT from the manifest, not verbs that are present but unimplemented.
///
/// # Given / When / Then
///
/// GIVEN a valid admin token
/// WHEN `call_tool` is invoked with `echo.audit` (declared in manifest, no typed export)
/// THEN the result is Err with code ErrorCode(-4003) — NON_DISPATCHABLE (UNCHANGED).
///
/// # Green before and after
///
/// Today: `echo.audit` reaches the dispatch path and returns -4003 (no export).
/// After Forge's green phase: the manifest gate passes `echo.audit` (it IS declared),
/// and the -4003 path still fires. This test remains green throughout.
#[tokio::test]
async fn exposed_but_undispatchable_verb_still_non_dispatchable() {
    // R-0010-d, R-0019-c: declared verb with no typed export → NON_DISPATCHABLE (-4003).
    // GIVEN: a slice-1 harness with a valid admin token
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let harness = slice1_echo_harness(db.pool.clone()).await;

    // WHEN: call echo.audit — declared in the manifest, but no typed export.
    let mut params = CallToolRequestParams::new("echo.audit");
    params.meta = Some(token_meta(harness.admin_token.as_str()));
    // No arguments — permission check runs before dispatch; any dispatch-layer
    // error is post-permission and acceptable.

    let result = harness.client.call_tool(params).await;

    // THEN: must be Err with NON_DISPATCHABLE code (-4003).
    // The manifest gate passes echo.audit (it IS declared); the dispatch path
    // returns -4003 because there is no typed export for the verb.
    let err = result.expect_err(
        "R-0010-d/R-0019-c: echo.audit is declared in the manifest but has no typed export; \
         the handler must return Err (NON_DISPATCHABLE or similar), not Ok",
    );

    match err {
        rmcp::ServiceError::McpError(ref error_data) => {
            assert_eq!(
                error_data.code,
                ErrorCode(-4003),
                "R-0010-d precision guard: echo.audit (declared, no typed export) must return \
                 ErrorCode(-4003) NON_DISPATCHABLE (UNCHANGED by the new gate); got {:?}. \
                 The manifest-verbs gate must NOT swallow declared-but-undispatchable verbs.",
                error_data.code
            );
        }
        other => panic!(
            "R-0010-d: expected ServiceError::McpError for echo.audit; got {:?}",
            other
        ),
    }
}

// ===========================================================================
// Test 3 (GREEN before and after): Declared dispatchable verb still succeeds
// ===========================================================================

/// R-0010-d, R-0019-c — declared and dispatchable verb succeeds through the gate.
///
/// # Regression guard
///
/// `echo.create` is declared in the manifest and has a typed export. After the
/// new manifest-verbs gate is added, an admin caller invoking `echo.create` must
/// still receive Ok with a well-formed ULID — the gate must not block declared,
/// dispatchable verbs.
///
/// # Given / When / Then
///
/// GIVEN a valid admin token
/// WHEN `call_tool` is invoked with `echo.create` + valid `{content_type, payload}` args
/// THEN the result is Ok containing a well-formed ULID (26-char Crockford base32).
///
/// # Green before and after
///
/// Today and after the green phase: `echo.create` is declared and dispatchable;
/// the gate (when added) passes it through; the happy path succeeds.
///
/// # Thin vs `mcp_slice1_e2e.rs`
///
/// `mcp_slice1_e2e.rs::echo_create_returns_well_formed_ulid_and_get_round_trips`
/// covers the same happy path with full ULID + round-trip assertions. This test
/// repeats the minimum shape here so the gate-boundary tests are self-contained
/// in this file and the gate's "pass-through" invariant is explicitly documented
/// alongside the "reject" invariant.
#[tokio::test]
async fn exposed_dispatchable_verb_still_succeeds() {
    // R-0010-d, R-0019-c: declared + dispatchable verb → Ok with ULID (UNCHANGED).
    // GIVEN: a slice-1 harness with a valid admin token
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let harness = slice1_echo_harness(db.pool.clone()).await;

    // WHEN: call echo.create — declared in manifest AND has a typed export.
    let mut params = CallToolRequestParams::new("echo.create");
    params.meta = Some(token_meta(harness.admin_token.as_str()));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert(
            "payload".to_owned(),
            json!({"msg": "gate_regression_probe"}),
        );
        m
    });

    let result = harness.client.call_tool(params).await;

    // THEN: must be Ok — the gate passes declared dispatchable verbs through.
    let call_result = result.expect(
        "R-0010-d regression guard: echo.create is declared in the manifest and has a typed \
         export; the manifest-verbs gate must NOT block it — Ok with a ULID is required.",
    );

    // Assert at least one text content item is present.
    let texts = extract_text_content(&call_result);
    assert!(
        !texts.is_empty(),
        "R-0010-d regression guard: echo.create must return at least one text content item; \
         got content: {:?}",
        call_result.content
    );

    // Assert the text content contains a well-formed ULID.
    let ulid_found = texts.iter().any(|t| is_valid_ulid(t.trim()));
    assert!(
        ulid_found,
        "R-0010-d regression guard: echo.create must return a well-formed ULID \
         (26 Crockford base32 chars) in its text content; got: {:?}",
        texts
    );
}

// ===========================================================================
// Test 4 (GREEN immediately — regression guard):
//   ReadObserver + unexposed read-tailed verb → membership gate fires (-4005)
// ===========================================================================

/// R-0010-d, R-0009-d — manifest-verbs membership gate is the sole defense when
/// a ReadObserver calls an unexposed read-tailed verb.
///
/// # The path being pinned
///
/// The role-gate classifies verb tails: `get` is read-classified, so a ReadObserver
/// calling `evil.get` passes the role check.  Only the manifest-verbs membership
/// gate (R-0010-d) stands between the caller and dispatch.
///
/// Today the gate rejects `evil.get` (not in the echo manifest → ErrorCode(-4005))
/// regardless of the caller's role — the gate is role-agnostic.  This test exists
/// so a future role-conditional refactor that accidentally makes the gate skip for
/// read-classified verbs would break this test while leaving all admin-token tests
/// green.
///
/// # Given / When / Then
///
/// GIVEN a valid ReadObserver token (read_observer scope, NOT admin),
///   a read-tailed verb `evil.get` that is NOT in the echo manifest's verbs list
/// WHEN `call_tool` is invoked with `evil.get` using that ReadObserver token
/// THEN the result is Err with ErrorCode(-4005) — the membership gate —
///   AND NOT ErrorCode(-4002) (PERMISSION_DENIED), which would indicate the
///   role check blocked it instead of the membership gate (masking the gate's absence).
///
/// # Green on arrival
///
/// The gate is currently role-agnostic and fires pre-dispatch for every caller.
/// `evil.get` is not in the echo manifest, so the gate rejects it with -4005
/// regardless of whether the caller is admin or ReadObserver.  If this test does
/// NOT pass green, the gate is role-conditional — a real security defect.
///
/// # Why -4002 is explicitly ruled out
///
/// A `get` tail passes the ReadObserver role check (R-0009-d allows read verbs).
/// If the result is -4002 (PERMISSION_DENIED), it means the role check fired —
/// not the membership gate — which would mean the gate is NOT the sole defense on
/// this path (it was bypassed or the role classification changed).  The `assert_ne`
/// on -4002 is the load-bearing guard: it proves the role passed and the gate rejected.
#[tokio::test]
async fn readobserver_unexposed_read_verb_rejected() {
    // R-0010-d, R-0009-d: ReadObserver + evil.get (not in manifest) → -4005 from gate.
    // GIVEN: a slice-1 harness (real MnemraMcpServer with echo plugin loaded)
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    // The harness seeds an admin token and exposes the connected client.
    // We seed a ReadObserver token separately against the same pool + workspace.
    let harness = slice1_echo_harness(db.pool.clone()).await;

    // Seed a ReadObserver token for the same workspace as the harness.
    // Uses the harness's workspace_id (DEFAULT_WORKSPACE_ID) so the token
    // is valid for this server's auth lookup.
    let ro_token = seed_read_observer_token(&db.pool, harness.workspace_id).await;

    // WHEN: call the server via the harness client, presenting the ReadObserver token.
    // Verb: evil.get — tail "get" is read-classified (passes role check);
    // "evil.get" is NOT in the echo manifest verbs list (triggers membership gate).
    let mut params = CallToolRequestParams::new("evil.get");
    params.meta = Some(token_meta(ro_token.as_str()));
    // No arguments needed: the membership gate fires pre-dispatch (before args are used).

    let result = harness.client.call_tool(params).await;

    // THEN: must be Err — the membership gate rejects evil.get regardless of role.
    let err = result.expect_err(
        "R-0010-d: evil.get is NOT in the echo manifest verbs list; \
         the membership gate must reject it pre-dispatch with Err even for a ReadObserver. \
         If Ok is returned, the gate is either absent or role-conditional — both are defects.",
    );

    match err {
        rmcp::ServiceError::McpError(ref error_data) => {
            // Primary assertion: membership gate fires with the distinct -4005 code.
            // This proves the gate runs and rejects unexposed verbs for all callers.
            assert_eq!(
                error_data.code,
                ErrorCode(-4005),
                "R-0010-d: evil.get (not in manifest) must return ErrorCode(-4005) \
                 (VERB_NOT_EXPOSED) for a ReadObserver; got {:?}. \
                 The membership gate must be role-agnostic — it fires after auth, \
                 before dispatch, for every role.",
                error_data.code
            );

            // Regression guard: NOT permission-denied (-4002).
            // A get-tailed verb passes the read-role check (R-0009-d).
            // If -4002 is returned instead of -4005, the role check blocked the call —
            // meaning the membership gate is not the sole defense on this path, which
            // is a security regression: a future fix to allow ReadObserver more read
            // verbs could inadvertently expose undeclared verbs.
            assert_ne!(
                error_data.code,
                ErrorCode(-4002),
                "R-0010-d/R-0009-d: evil.get rejection MUST NOT be ErrorCode(-4002) \
                 (PERMISSION_DENIED); a get-tailed verb passes the read-role check, \
                 so -4002 would mean the membership gate is not the sole defense. \
                 Expected -4005 (VERB_NOT_EXPOSED) — the gate.",
            );

            // Guard: not an auth failure — the token is valid (just ReadObserver-scoped).
            assert_ne!(
                error_data.code,
                ErrorCode(-4001),
                "R-0010-d: ReadObserver token is valid; evil.get must NOT return \
                 ErrorCode(-4001) (AUTH_FAILURE) — the token resolves correctly.",
            );
        }
        other => panic!(
            "R-0010-d: expected ServiceError::McpError for evil.get with ReadObserver token; \
             got {:?}",
            other
        ),
    }
}
