//! `message send` acceptance suite (Task 7 slice a — RED). Glitch, dispatch 1593.
//!
//! # What this file contracts (the spec guarantees, not the parent commit)
//!
//! At the parent commit (`0fe62fc`, Task 6 `validate_message` merged), the
//! `message` tool advertises and routes exactly ONE action:
//! `(MESSAGE_TOOL, "poll")` — [`mnemra_host::coordination::session_plane::parse_action`]'s
//! match arms cover `poll` for `message` and `acquire`/`list`/`renew`/`release`/
//! `takeover` for `claim`; every other `(tool, action)` pair — including
//! `("message", "send")` — falls through the catch-all and returns
//! `ErrorData { code: INVALID_PARAMS, message: "unsupported coordination
//! action 'send' for tool 'message'" }`. This is confirmed both by reading the
//! match (the sanctioned host-internal-surface reading this file's precedents
//! establish — see "Sanctioned reads" below) and by an existing, currently
//! PASSING unit test in that same module,
//! `parse_action_rejects_unsupported_missing_and_non_string`, which explicitly
//! asserts `send` is rejected today. `send` does not exist as a body at all —
//! no `coordination/messages.rs` module exists yet (Task 7a's own touch_scope
//! creates it).
//!
//! These tests encode the R-0068-a/R-0070-b/R-0075-b/R-0075-e `send` contract
//! the GREEN implementer (Task 7 slice a) must fill.
//!
//! # RED mechanism — a documented DEVIATION from the dispatch's stated
//! expectation (read before trusting `--no-run`)
//!
//! The dispatch brief predicted a COMPILE failure ("the `send` action does not
//! exist yet, so your tests reference the intended API and fail to compile —
//! that compile failure IS the correct red, mirror T6's stance") and told the
//! RED-confirmation step to run `cargo test --no-run` expecting a nonzero
//! compile-fail exit. That prediction does not hold for this slice, and the
//! evidence is direct: `send` is invoked over the **wire MCP tool surface**
//! (`{ action: "send", to_role_instance, type, schema_version, payload }` via
//! `call_tool`) exactly the way `coordination_session_plane.rs`'s `poll` and
//! `coordination_leases.rs`'s `claim` actions are — a dynamically-typed JSON
//! args map, not a Rust-level API this file's own symbols could fail to
//! resolve. T6's compile-fail red was correct FOR T6 because T6 IS a Rust
//! library API (`validate_message`, `MessageValidationError`) called directly
//! from Rust; T7a's `send` is a wire action, like every other coordination
//! action this codebase has red-phased so far. Both established precedents in
//! this exact test family are RUNTIME reds, not compile reds:
//! `coordination_session_plane.rs`'s b0 `poll` suite (red against the
//! `poll_placeholder` skeleton) and `coordination_leases.rs`'s b2 `claim list`
//! suite (red against the SAME unrouted-action catch-all `send` hits here —
//! that file's own header names this "why every scenario collapses to ONE
//! wire-level RED cause": `list` is unrouted on `claim` exactly as `send` is
//! unrouted on `message`, and every scenario is red for the identical
//! `INVALID_PARAMS` reason, distinguished only by which POSITIVE guarantee
//! each assertion anchors on).
//!
//! So: this file compiles cleanly today (confirmed — see the completion
//! report) and its tests fail at RUNTIME, uniformly, because `send` is an
//! unsupported action — the exact `coordination_leases.rs` "list" pattern,
//! not the T6 pattern. `cargo test --no-run` exits 0 (compiles); the RED
//! confirmation instead is a normal test run showing every test below FAILS,
//! each for its own POSITIVE guarantee-absent reason (never a bare
//! `Err`/`-32602` check, which would be vacuous — see Non-vacuity below).
//! Flagged as a `deviations` entry in the completion report, not silently
//! substituted.
//!
//! # Sanctioned reads (this file's black-box-adjacent convention, inherited)
//!
//! Per `coordination_session_plane.rs` ("DB observation of a host-internal
//! surface is sanctioned; the public API is read only for signatures, never
//! for ported logic") and `coordination_leases.rs` ("this file's established
//! black-box-adjacent convention of observing host-internal surfaces,
//! extended here to reading the dispatch/routing shape itself") — this file
//! reads `session_plane::parse_action`'s match arms (routing shape, to reason
//! about the uniform RED cause above), `message_types.rs` (T6's registered
//! `handoff`/`merge-request` schemas — used as payload FIXTURES, never
//! ported/reimplemented), `write_path.rs`'s `Refusal` enum (reason-code
//! strings only — `.reason_code()`'s match arms), and `audit.rs`'s
//! `AuditRecord::registration` (the audit payload SHAPE — `{"role_instance":
//! ...}` — used to query `coordination_audit` by role-instance without
//! needing the actor id ahead of time). No coordination/mcp logic is ported
//! into this file; every assertion is driven through the wire `message` tool
//! and observed via operator SQL / the audit table / the `tracing` stream —
//! never by importing or calling a coordination-internal function directly.
//!
//! # AC ↔ test map
//!
//! | # | Test | R-ID(s) |
//! |---|---|---|
//! | 1. Conformant send lands a row | `conformant_send_lands_a_message_row_with_expected_fields` | R-0068-a |
//! | 2. Undeclared extra field → `schema_violation`, no row | `send_with_undeclared_extra_field_is_refused_schema_violation_and_lands_no_row` | R-0070-b |
//! | 3. Unknown type/version → `unknown_type`, no row | `send_naming_unknown_type_or_version_is_refused_unknown_type_and_lands_no_row` | R-0070-b |
//! | 4. Send-ordering pin (4 refusal reasons → no mint, no registration audit) | `send_refused_for_any_reason_mints_no_addressee_and_emits_no_registration_audit` | §API Contract send-ordering pin |
//! | 5. Registration-audit-iff-minted | `registration_audit_fires_iff_addressee_is_newly_minted_by_send` | R-0075-b, FF-4 |
//! | 6. Unattached session → `not_attached` | `send_from_unattached_session_is_refused_not_attached` | R-0064-e (the new gate, exercisable at Task 7) |
//! | 7. `read_observer` refused pre-dispatch | `send_from_read_observer_token_is_refused_pre_dispatch` | R-0073-b |
//! | 8. Log-field hygiene | `schema_violation_log_entry_carries_discrete_structured_fields_not_display_string` | R-0075-e |
//!
//! Tests 4 and 5 are `#[cfg(feature = "test-hooks")]`-gated per the dispatch's
//! explicit mechanics instruction ("the registration-audit + send-ordering
//! assertions ride the test-hooks seam, like `coordination_failclosed`") —
//! this file therefore compiles and runs a 6-test suite under the DEFAULT
//! feature set (`verify-test`) and the full 8-test suite under
//! `--features test-hooks` (`verify-test-hooks`). The one helper exclusively
//! consumed by those two tests (`registration_audit_count_for_role_instance`)
//! is gated identically so no `dead_code` warning fires under default.
//!
//! # Non-vacuity discipline (held, inherited from both precedent files)
//!
//! Every refusal-code assertion anchors on the structured `reason_code` being
//! PRESENT — never on a no-row/no-mint side effect alone, which would pass
//! vacuously against ANY unrelated error (including today's uniform
//! `unsupported coordination action 'send'`). Every "no row created" / "no
//! actor minted" / "no registration audit emitted" check is a SECONDARY guard
//! layered on top of a reason-code or response-shape anchor, never the sole
//! anchor. Test 4's ordering-pin sub-cases and test 5's registration-audit
//! test are the ones most at risk of accidental vacuity (their headline claim
//! is an ABSENCE), so each is anchored FIRST on a POSITIVE, guarantee-absent
//! assertion (the refusal code being present in test 4; the send actually
//! succeeding — `send_structured_obj(..).is_some()` — in test 5) before the
//! absence checks are layered on. Test 8 mirrors
//! `coordination_leases.rs::successful_acquire_emits_op_log_entry`'s capture-
//! liveness canary so a silently-dead `tracing_test` capture channel cannot
//! masquerade as "no hygiene violation found".
//!
//! # Payload fixture choice
//!
//! Every test below uses the registered `handoff` v1 type
//! (`message_types::{HANDOFF_TYPE_NAME, HANDOFF_V1}`) — its `subject`-only
//! required field is the smallest conformant fixture, keeping fixture
//! construction out of the way of the coordination assertions under test.
//! `merge-request` v1 (the 9-field type) is not needed here — Task 6's own
//! suite already exercises its schema in isolation, and Task 7a is testing
//! `send`'s coordination behavior, not schema-shape coverage a second time.

#[path = "common/shared_engine.rs"]
mod shared_engine;

use std::sync::Arc;

use rmcp::model::{CallToolRequestParams, CallToolResult, Meta};
use rmcp::service::{RoleClient, RunningService, serve_client, serve_server};
use serde_json::json;
use tokio::io::duplex;
use tracing_test::traced_test;
use uuid::Uuid;

use mnemra_host::auth::token::{AdminToken, generate, hash};
use mnemra_host::coordination::message_types::{HANDOFF_TYPE_NAME, HANDOFF_V1};
use mnemra_host::mcp::errors::PERMISSION_DENIED_CODE;
use mnemra_host::mcp::server::MnemraMcpServer;
use mnemra_host::plugin::pool::PluginPool;
use mnemra_host::storage::postgres::engine::EmbeddedEngine;

// ===========================================================================
// Harness (duplicated per this codebase's established per-file harness
// convention — `coordination_leases.rs`'s own header names this explicitly:
// "mirrors `coordination_session_plane.rs` — duplicated ... e.g.
// `mcp_verb_gate.rs` inlines `seed_read_observer_token` rather than importing
// it")
// ===========================================================================

/// Seed an admin-role token into `admin_tokens`. `scopes = ["admin"]` →
/// `Role::Admin`, clearing every write-category gate on the path to `send`.
async fn seed_admin_token(pool: &sqlx::PgPool, workspace_id: Uuid) -> AdminToken {
    let token = generate();
    let token_hash = hash(&token);
    let _: (Uuid,) = sqlx::query_as(
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
    token
}

/// Seed a `read_observer`-scoped token into `admin_tokens` (test 7). Mirrors
/// `seed_read_observer_token` in `coordination_leases.rs` / `mcp_verb_gate.rs`
/// — private to their own binaries, not importable here.
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

/// Build a `Meta` carrying the auth token for the MCP `_meta.token` field.
fn token_meta(token_str: &str) -> Meta {
    let mut meta = Meta::new();
    meta.insert("token".to_owned(), json!(token_str));
    meta
}

/// A bare `PluginPool` with no echo component registered — the host-served
/// coordination branch never touches the plugin pool for `message`.
fn minimal_plugin_pool() -> Arc<PluginPool> {
    Arc::new(PluginPool::new().expect("PluginPool::new"))
}

/// Stand up one `MnemraMcpServer` over an in-memory duplex transport and
/// return its server task handle + a connected `rmcp` client. Default
/// coordination config — no scenario here waits past the attachment TTL.
async fn coordination_server(
    pool: &sqlx::PgPool,
) -> (tokio::task::JoinHandle<()>, RunningService<RoleClient, ()>) {
    let server = MnemraMcpServer::new(pool.clone(), minimal_plugin_pool());
    let (server_transport, client_transport) = duplex(4096);
    let handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => eprintln!("coordination_messages test server init failed: {e:?}"),
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");
    (handle, client)
}

/// Build the `message` `poll` bind call for `role_instance` under `token_str`
/// — the attach precondition every non-`poll` `message` action requires
/// (R-0064-e), and the mechanism used to pre-seed an "already existing"
/// addressee in test 5.
fn poll_params(token_str: &str, role_instance: &str) -> CallToolRequestParams {
    let mut params = CallToolRequestParams::new("message");
    params.meta = Some(token_meta(token_str));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("action".to_owned(), json!("poll"));
        m.insert("role_instance".to_owned(), json!(role_instance));
        m
    });
    params
}

/// Build a `message send` tool call. `schema_version` is `u16` (matches
/// `message_types`'s registered version type) — serialized as a JSON number,
/// same wire shape a real MCP client sends.
fn send_params(
    token_str: &str,
    to_role_instance: &str,
    type_name: &str,
    schema_version: u16,
    payload: serde_json::Value,
) -> CallToolRequestParams {
    let mut params = CallToolRequestParams::new("message");
    params.meta = Some(token_meta(token_str));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("action".to_owned(), json!("send"));
        m.insert("to_role_instance".to_owned(), json!(to_role_instance));
        m.insert("type".to_owned(), json!(type_name));
        m.insert("schema_version".to_owned(), json!(schema_version));
        m.insert("payload".to_owned(), payload);
        m
    });
    params
}

/// A conformant `handoff` v1 payload — `subject` is the only required field.
fn conformant_handoff_payload(subject: &str) -> serde_json::Value {
    json!({ "subject": subject })
}

/// A `handoff` v1 payload carrying an UNDECLARED extra field — `HandoffV1`
/// derives `#[serde(deny_unknown_fields)]` (`message_types.rs`), so this must
/// deserialize-fail into `MessageValidationError::SchemaViolation` once `send`
/// calls `validate_message` (R-0070-b closed-schema).
fn handoff_payload_with_undeclared_field(subject: &str) -> serde_json::Value {
    json!({ "subject": subject, "not_a_declared_handoff_field": "should be refused" })
}

/// Bind `role_instance` via `message poll` and return its resolved
/// `actor_id`. Panics (a precondition failure, not a scenario assertion) if
/// the bind itself does not succeed — every scenario using this depends on
/// `message poll` (Task 4, already green at the parent commit) working.
async fn attach_session(
    client: &RunningService<RoleClient, ()>,
    token: &str,
    role_instance: &str,
) -> Uuid {
    let res = client
        .call_tool(poll_params(token, role_instance))
        .await
        .expect(
            "precondition: `message poll` (Task 4, already green) must bind the session before any \
         `send` call in this file",
        );
    let obj = res
        .structured_content
        .as_ref()
        .and_then(|v| v.as_object())
        .expect("precondition: the poll response must carry structured content");
    let actor = obj
        .get("actor")
        .and_then(|v| v.as_object())
        .expect("precondition: the poll response must carry an `actor` object");
    let actor_id_str = actor
        .get("actor_id")
        .and_then(|v| v.as_str())
        .expect("precondition: `actor.actor_id` must be a string");
    Uuid::parse_str(actor_id_str).expect("precondition: `actor.actor_id` must be a valid UUID")
}

/// True iff the call result surfaces `needle` anywhere in its serialized
/// structured content or protocol error — the closed `reason_code` enum
/// anchor (spec §API Contract). A machine JSON envelope, so a
/// serialized-contains scan is exact here (not the over-matching-prose hazard
/// `skills/bdd.md` warns about — same precedent as both harness files this
/// suite mirrors).
fn result_surfaces_code<E: std::fmt::Debug>(
    result: &Result<CallToolResult, E>,
    needle: &str,
) -> bool {
    match result {
        Ok(r) => serde_json::to_string(r)
            .map(|s| s.contains(needle))
            .unwrap_or(false),
        Err(e) => format!("{e:?}").contains(needle),
    }
}

/// The structured-content JSON object of a `message` call result, or `None`
/// if the call errored outright (today's `send` case — unsupported action) or
/// carried no structured content.
fn send_structured_obj<E>(
    result: &Result<CallToolResult, E>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    match result {
        Ok(r) => r
            .structured_content
            .as_ref()
            .and_then(|v| v.as_object())
            .cloned(),
        Err(_) => None,
    }
}

// ----- DB observers (sanctioned black-box carve-out — direct SQL, per both precedent files) -----

/// The `actors.id` for `(workspace_id, name)`, if a row exists — the
/// resolve-or-create / no-mint invariant surface (R-0064-a), reused unchanged
/// from `coordination_session_plane.rs`.
async fn actor_id_by_name(pool: &sqlx::PgPool, workspace_id: Uuid, name: &str) -> Option<Uuid> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM actors WHERE workspace_id = $1 AND name = $2")
            .bind(workspace_id)
            .bind(name)
            .fetch_optional(pool)
            .await
            .expect("actors read-back query must execute");
    row.map(|(id,)| id)
}

/// The `messages` row for `message_id`, if any:
/// `(workspace_id, sender_actor_id, addressee_actor_id, message_type,
/// schema_version, state, sent_at_is_set, delivered_at_is_null)`.
///
/// `#[allow(clippy::type_complexity)]`: an 8-tuple observer return, same
/// shape/precedent as `coordination_session_plane.rs::succession_audit_evidence`
/// (a 6-tuple carrying the same `#[allow]`) — a named struct would be pure
/// ceremony for a test-local, single-call-site DB read.
#[allow(clippy::type_complexity)]
async fn message_row_by_id(
    pool: &sqlx::PgPool,
    message_id: Uuid,
) -> Option<(Uuid, Uuid, Uuid, String, i32, String, bool, bool)> {
    let row: Option<(Uuid, Uuid, Uuid, String, i32, String, bool, bool)> = sqlx::query_as(
        "SELECT workspace_id, sender_actor_id, addressee_actor_id, message_type, schema_version,
                state, (sent_at IS NOT NULL), (delivered_at IS NULL)
         FROM messages WHERE id = $1",
    )
    .bind(message_id)
    .fetch_optional(pool)
    .await
    .expect("message read-back query must execute");
    row
}

/// Count of `messages` rows for `workspace_id` — the "no row lands" surface
/// every refusal test in this file asserts against. Each test uses a FRESH
/// `workspace_id`, so a bare workspace-scoped count is exact (no other
/// scenario's rows can leak in).
async fn message_count(pool: &sqlx::PgPool, workspace_id: Uuid) -> i64 {
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM messages WHERE workspace_id = $1")
        .bind(workspace_id)
        .fetch_one(pool)
        .await
        .expect("message count query must execute");
    n
}

/// Count of `registration`-typed `coordination_audit` rows whose payload
/// names `role_instance` — the FF-4 / send-ordering-pin surface. Queries by
/// role-instance NAME (the audit payload shape `AuditRecord::registration`
/// stages: `{"role_instance": role_instance}`, `audit.rs`), so it works even
/// when no actor was ever minted for that identifier (test 4's refusal
/// cases). `#[cfg(feature = "test-hooks")]`-gated: the exclusive consumer is
/// tests 4 and 5, both gated identically (dispatch mechanics instruction) —
/// gating this helper the same way avoids a `dead_code` warning under the
/// default feature set.
#[cfg(feature = "test-hooks")]
async fn registration_audit_count_for_role_instance(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    role_instance: &str,
) -> i64 {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM coordination_audit
         WHERE workspace_id = $1 AND event_type = 'registration'
           AND payload->>'role_instance' = $2",
    )
    .bind(workspace_id)
    .bind(role_instance)
    .fetch_one(pool)
    .await
    .expect("registration-audit count query must execute");
    n
}

// ===========================================================================
// Test 1 — conformant send lands a row (R-0068-a)
// ===========================================================================

/// GIVEN an attached sender session,
/// WHEN it `send`s a conformant `handoff` v1 payload to a fresh
/// `to_role_instance`,
/// THEN the response carries `{ message_id, state: "sent", sent_at }`
/// (§API Contract) AND the `messages` row (read back by that id, operator
/// SQL) shows `state = 'sent'`, `delivered_at IS NULL`, `sender_actor_id` =
/// the attached sender, `addressee_actor_id` = the resolved addressee id.
/// *(R-0068-a)*
///
/// RED against the parent commit: `send` is an unsupported `message` action
/// (see file header) — the call errors with `INVALID_PARAMS`, so
/// `send_structured_obj` returns `None` and the FIRST assertion fails
/// guarantee-absent; no `messages` row exists for any id (no code path
/// creates one).
#[tokio::test]
async fn conformant_send_lands_a_message_row_with_expected_fields() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let sender_role = format!("sender-{}", Uuid::new_v4());
    let addressee_role = format!("addressee-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    let sender_actor_id = attach_session(&client, token.as_str(), &sender_role).await;

    let res = client
        .call_tool(send_params(
            token.as_str(),
            &addressee_role,
            HANDOFF_TYPE_NAME,
            HANDOFF_V1,
            conformant_handoff_payload("shift-left review"),
        ))
        .await;

    let obj = send_structured_obj(&res);
    assert!(
        obj.is_some(),
        "R-0068-a: a conformant `send` by an attached actor must return the `{{ message_id, \
         state, sent_at }}` object; `send` is an unsupported `message` action at the parent \
         commit, so the call errors instead. Got: {res:?}"
    );
    let obj = obj.unwrap();

    let message_id_str = obj
        .get("message_id")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            panic!(
                "§API Contract: the `send` response must carry a string `message_id`; got {obj:?}"
            )
        });
    let message_id = Uuid::parse_str(message_id_str)
        .unwrap_or_else(|e| panic!("`message_id` must be a valid UUID: {e}"));

    assert_eq!(
        obj.get("state").and_then(|v| v.as_str()),
        Some("sent"),
        "§API Contract: the `send` response must carry `state: \"sent\"`; got {obj:?}"
    );
    assert!(
        obj.get("sent_at").and_then(|v| v.as_str()).is_some(),
        "§API Contract: the `send` response must carry a string `sent_at`; got {obj:?}"
    );

    let addressee_actor_id = actor_id_by_name(&pool, workspace_id, &addressee_role)
        .await
        .unwrap_or_else(|| {
            panic!(
                "R-0064-a: `send` to a novel `to_role_instance` must resolve-or-create the \
                 addressee actor row; none exists for `{addressee_role}`."
            )
        });

    let row = message_row_by_id(&pool, message_id).await;
    assert!(
        row.is_some(),
        "R-0068-a: a `messages` row must exist for the returned `message_id` ({message_id}); \
         none found."
    );
    let (
        row_workspace_id,
        row_sender,
        row_addressee,
        row_type,
        row_schema_version,
        row_state,
        sent_at_is_set,
        delivered_at_is_null,
    ) = row.unwrap();

    assert_eq!(
        row_workspace_id, workspace_id,
        "R-0076-b: the message row must be scoped to the sending session's workspace."
    );
    assert_eq!(
        row_sender, sender_actor_id,
        "§API Contract: `sender_actor_id` must be the attached sender, host-resolved — no \
         sender parameter exists."
    );
    assert_eq!(
        row_addressee, addressee_actor_id,
        "R-0068-a: `addressee_actor_id` must equal the resolved addressee actor id."
    );
    assert_eq!(
        row_type, HANDOFF_TYPE_NAME,
        "the row must carry the sent message-type name."
    );
    assert_eq!(
        row_schema_version, HANDOFF_V1 as i32,
        "the row must carry the sent schema-version."
    );
    assert_eq!(
        row_state, "sent",
        "a freshly-sent message's state must be `sent`."
    );
    assert!(
        sent_at_is_set,
        "a freshly-sent message must carry a `sent_at` timestamp."
    );
    assert!(
        delivered_at_is_null,
        "R-0068-a: `delivered_at` must be NULL until the addressee's own `poll` delivers it \
         (Task 7 slice b) — `send` alone never sets it."
    );

    server.abort();
}

// ===========================================================================
// Test 2 — undeclared extra field → schema_violation, no row (R-0070-b)
// ===========================================================================

/// GIVEN an attached sender session,
/// WHEN it `send`s a `handoff` v1 payload carrying an UNDECLARED extra field,
/// THEN the call is refused `schema_violation` and NO `messages` row is
/// created (closed-schema validation — R-0070-b). *(R-0070-b)*
///
/// RED against the parent commit: `send` is unsupported — the call errors
/// with `INVALID_PARAMS`; `schema_violation` is absent (never vacuous: the
/// no-row check is layered on the reason-code anchor, never the sole anchor).
#[tokio::test]
async fn send_with_undeclared_extra_field_is_refused_schema_violation_and_lands_no_row() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let sender_role = format!("sender-schema-viol-{}", Uuid::new_v4());
    let addressee_role = format!("addressee-schema-viol-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &sender_role).await;

    let res = client
        .call_tool(send_params(
            token.as_str(),
            &addressee_role,
            HANDOFF_TYPE_NAME,
            HANDOFF_V1,
            handoff_payload_with_undeclared_field("has an extra field"),
        ))
        .await;

    assert!(
        result_surfaces_code(&res, "schema_violation"),
        "R-0070-b: a `send` payload carrying an undeclared field must be refused \
         `schema_violation`. Against the unrouted `send` action the code is absent \
         (`INVALID_PARAMS` instead). Got: {res:?}"
    );

    let count = message_count(&pool, workspace_id).await;
    assert_eq!(
        count, 0,
        "R-0070-b: a `schema_violation` refusal must land no `messages` row; found {count}."
    );

    server.abort();
}

// ===========================================================================
// Test 3 — unknown type/version → unknown_type, no row (R-0070-b)
// ===========================================================================

/// GIVEN an attached sender session,
/// WHEN it `send`s (a) an entirely unregistered `type` name and (b) the
/// registered `handoff` type at an unregistered `schema_version`,
/// THEN each is refused `unknown_type` and creates no `messages` row
/// (version-before-schema ordering — the registry lookup happens BEFORE any
/// payload deserialization). *(R-0070-b)*
///
/// RED against the parent commit: `send` is unsupported — both calls error
/// with `INVALID_PARAMS`; `unknown_type` is absent for either case.
#[tokio::test]
async fn send_naming_unknown_type_or_version_is_refused_unknown_type_and_lands_no_row() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let sender_role = format!("sender-unknown-type-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &sender_role).await;

    for (label, type_name, schema_version) in [
        (
            "unregistered type name",
            "not-a-registered-message-type",
            HANDOFF_V1,
        ),
        (
            "registered type, unregistered version",
            HANDOFF_TYPE_NAME,
            9999u16,
        ),
    ] {
        let addressee_role = format!("addressee-unknown-type-{label}-{}", Uuid::new_v4());
        let res = client
            .call_tool(send_params(
                token.as_str(),
                &addressee_role,
                type_name,
                schema_version,
                conformant_handoff_payload("irrelevant — unknown_type short-circuits first"),
            ))
            .await;

        assert!(
            result_surfaces_code(&res, "unknown_type"),
            "R-0070-b: `send` naming `({type_name}, v{schema_version})` ({label}) must be \
             refused `unknown_type`. Against the unrouted `send` action the code is absent \
             (`INVALID_PARAMS` instead). Got: {res:?}"
        );
    }

    let count = message_count(&pool, workspace_id).await;
    assert_eq!(
        count, 0,
        "R-0070-b: an `unknown_type` refusal must land no `messages` row; found {count}."
    );

    server.abort();
}

// ===========================================================================
// Test 4 — send-ordering pin: any refusal reason mints no addressee, emits
// no registration audit (§API Contract send-ordering pin)
// ===========================================================================

/// GIVEN four DISTINCT refused-`send` scenarios — one per refusal reason
/// (`not_attached`, `unknown_type`, `schema_violation`, `invalid_role_instance`)
/// — each naming a FRESH `to_role_instance`,
/// WHEN each `send` is refused,
/// THEN NONE mints an `actors` row for its `to_role_instance` AND NONE emits
/// a `registration`-typed `coordination_audit` row naming that role-instance
/// — **the addressee principal is minted only after every validation gate
/// passes** (§API Contract). *(the send-ordering pin)*
///
/// # Non-vacuity discipline
///
/// Each sub-case anchors FIRST on the refusal's reason-code being PRESENT
/// (guarantee-absent today — every case gets `INVALID_PARAMS` instead, since
/// `send` is unsupported). The no-mint / no-audit checks are SECONDARY,
/// layered on top — never the sole anchor (they would pass vacuously against
/// today's uniform unrouted-action error too).
///
/// RED against the parent commit: `send` is unsupported — all four cases hit
/// the SAME `INVALID_PARAMS` catch-all (mirrors `coordination_leases.rs`'s
/// `claim list` collapse); none of the four reason-codes is present.
#[cfg(feature = "test-hooks")]
#[tokio::test]
async fn send_refused_for_any_reason_mints_no_addressee_and_emits_no_registration_audit() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let (server, client) = coordination_server(&pool).await;

    // Case 1: not_attached — no sender attach at all.
    {
        let token = seed_admin_token(&pool, workspace_id).await;
        let addressee_role = format!("ordering-not-attached-{}", Uuid::new_v4());
        let res = client
            .call_tool(send_params(
                token.as_str(),
                &addressee_role,
                HANDOFF_TYPE_NAME,
                HANDOFF_V1,
                conformant_handoff_payload("case 1"),
            ))
            .await;

        assert!(
            result_surfaces_code(&res, "not_attached"),
            "send-ordering pin (not_attached case): the refusal code must be present. Against \
             the unrouted `send` action it is absent (`INVALID_PARAMS` instead). Got: {res:?}"
        );
        assert!(
            actor_id_by_name(&pool, workspace_id, &addressee_role)
                .await
                .is_none(),
            "send-ordering pin: a `not_attached` refusal must mint NO addressee actor row for \
             `{addressee_role}`."
        );
        assert_eq!(
            registration_audit_count_for_role_instance(&pool, workspace_id, &addressee_role).await,
            0,
            "send-ordering pin: a `not_attached` refusal must emit NO registration audit for \
             `{addressee_role}`."
        );
    }

    // Case 2: unknown_type — attached sender, unregistered type name.
    {
        let token = seed_admin_token(&pool, workspace_id).await;
        let sender_role = format!("ordering-sender-unknown-type-{}", Uuid::new_v4());
        attach_session(&client, token.as_str(), &sender_role).await;
        let addressee_role = format!("ordering-unknown-type-{}", Uuid::new_v4());

        let res = client
            .call_tool(send_params(
                token.as_str(),
                &addressee_role,
                "not-a-registered-message-type",
                1,
                conformant_handoff_payload("case 2"),
            ))
            .await;

        assert!(
            result_surfaces_code(&res, "unknown_type"),
            "send-ordering pin (unknown_type case): the refusal code must be present. Against \
             the unrouted `send` action it is absent. Got: {res:?}"
        );
        assert!(
            actor_id_by_name(&pool, workspace_id, &addressee_role)
                .await
                .is_none(),
            "send-ordering pin: an `unknown_type` refusal must mint NO addressee actor row for \
             `{addressee_role}`."
        );
        assert_eq!(
            registration_audit_count_for_role_instance(&pool, workspace_id, &addressee_role).await,
            0,
            "send-ordering pin: an `unknown_type` refusal must emit NO registration audit for \
             `{addressee_role}`."
        );
    }

    // Case 3: schema_violation — attached sender, undeclared extra field.
    {
        let token = seed_admin_token(&pool, workspace_id).await;
        let sender_role = format!("ordering-sender-schema-viol-{}", Uuid::new_v4());
        attach_session(&client, token.as_str(), &sender_role).await;
        let addressee_role = format!("ordering-schema-viol-{}", Uuid::new_v4());

        let res = client
            .call_tool(send_params(
                token.as_str(),
                &addressee_role,
                HANDOFF_TYPE_NAME,
                HANDOFF_V1,
                handoff_payload_with_undeclared_field("case 3"),
            ))
            .await;

        assert!(
            result_surfaces_code(&res, "schema_violation"),
            "send-ordering pin (schema_violation case): the refusal code must be present. \
             Against the unrouted `send` action it is absent. Got: {res:?}"
        );
        assert!(
            actor_id_by_name(&pool, workspace_id, &addressee_role)
                .await
                .is_none(),
            "send-ordering pin: a `schema_violation` refusal must mint NO addressee actor row \
             for `{addressee_role}`."
        );
        assert_eq!(
            registration_audit_count_for_role_instance(&pool, workspace_id, &addressee_role).await,
            0,
            "send-ordering pin: a `schema_violation` refusal must emit NO registration audit \
             for `{addressee_role}`."
        );
    }

    // Case 4: invalid_role_instance — attached sender, whitespace-bearing
    // `to_role_instance` (the same identifier-rule violation
    // `coordination_session_plane.rs` uses for the bind path).
    {
        let token = seed_admin_token(&pool, workspace_id).await;
        let sender_role = format!("ordering-sender-invalid-role-{}", Uuid::new_v4());
        attach_session(&client, token.as_str(), &sender_role).await;
        let bad_addressee_role = "ordering case 4 has spaces";

        let res = client
            .call_tool(send_params(
                token.as_str(),
                bad_addressee_role,
                HANDOFF_TYPE_NAME,
                HANDOFF_V1,
                conformant_handoff_payload("case 4"),
            ))
            .await;

        assert!(
            result_surfaces_code(&res, "invalid_role_instance"),
            "send-ordering pin (invalid_role_instance case): the refusal code must be present. \
             Against the unrouted `send` action it is absent. Got: {res:?}"
        );
        assert!(
            actor_id_by_name(&pool, workspace_id, bad_addressee_role)
                .await
                .is_none(),
            "send-ordering pin: an `invalid_role_instance` refusal must mint NO addressee actor \
             row for `{bad_addressee_role}`."
        );
        assert_eq!(
            registration_audit_count_for_role_instance(&pool, workspace_id, bad_addressee_role)
                .await,
            0,
            "send-ordering pin: an `invalid_role_instance` refusal must emit NO registration \
             audit for `{bad_addressee_role}`."
        );
    }

    server.abort();
}

// ===========================================================================
// Test 5 — registration-audit-iff-minted (R-0075-b, FF-4)
// ===========================================================================

/// GIVEN an attached sender session,
/// WHEN it `send`s to (a) a NOVEL `to_role_instance` (never bound before) and
/// (b) an ALREADY-EXISTING addressee (bound via its own `poll` beforehand),
/// THEN (a) succeeds, mints the addressee, and emits EXACTLY ONE
/// `registration` audit naming that role-instance; (b) also succeeds
/// (resolves, not mints) and emits NO additional `registration` audit — the
/// count for the pre-existing addressee stays at the ONE its own bind already
/// produced. *(R-0075-b; FF-4)*
///
/// # Non-vacuity discipline
///
/// Both halves anchor FIRST on the send itself SUCCEEDING
/// (`send_structured_obj(..).is_some()`) — guarantee-absent today, since
/// `send` never dispatches. The audit-count assertions are layered on top.
///
/// RED against the parent commit: `send` is unsupported — both calls error
/// with `INVALID_PARAMS`; `send_structured_obj` returns `None` for each,
/// failing the primary anchor before the audit-count checks are ever
/// meaningfully exercised.
#[cfg(feature = "test-hooks")]
#[tokio::test]
async fn registration_audit_fires_iff_addressee_is_newly_minted_by_send() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let (server, client) = coordination_server(&pool).await;

    let sender_token = seed_admin_token(&pool, workspace_id).await;
    let sender_role = format!("reg-audit-sender-{}", Uuid::new_v4());
    attach_session(&client, sender_token.as_str(), &sender_role).await;

    // --- Part A: NOVEL addressee — never bound before. ---
    let novel_addressee = format!("novel-addressee-{}", Uuid::new_v4());
    let res_a = client
        .call_tool(send_params(
            sender_token.as_str(),
            &novel_addressee,
            HANDOFF_TYPE_NAME,
            HANDOFF_V1,
            conformant_handoff_payload("part a — novel"),
        ))
        .await;

    let obj_a = send_structured_obj(&res_a);
    assert!(
        obj_a.is_some(),
        "R-0075-b/FF-4: a `send` to a NOVEL `to_role_instance` must succeed (mint the addressee \
         and land the message); guarantee-absent today (`send` is unrouted). Got: {res_a:?}"
    );

    let novel_registration_count =
        registration_audit_count_for_role_instance(&pool, workspace_id, &novel_addressee).await;
    assert_eq!(
        novel_registration_count, 1,
        "FF-4: a send minting a NOVEL addressee must emit EXACTLY ONE `registration` audit \
         naming `{novel_addressee}`; found {novel_registration_count}."
    );

    // --- Part B: ALREADY-EXISTING addressee — bound via its own poll first. ---
    let existing_addressee = format!("existing-addressee-{}", Uuid::new_v4());
    let existing_token = seed_admin_token(&pool, workspace_id).await;
    attach_session(&client, existing_token.as_str(), &existing_addressee).await;

    let pre_send_registration_count =
        registration_audit_count_for_role_instance(&pool, workspace_id, &existing_addressee).await;
    assert_eq!(
        pre_send_registration_count, 1,
        "precondition: the addressee's own `poll` bind must already have emitted its \
         `registration` audit (Task 4, green) before this test's `send` runs; found \
         {pre_send_registration_count}."
    );

    let res_b = client
        .call_tool(send_params(
            sender_token.as_str(),
            &existing_addressee,
            HANDOFF_TYPE_NAME,
            HANDOFF_V1,
            conformant_handoff_payload("part b — existing"),
        ))
        .await;

    let obj_b = send_structured_obj(&res_b);
    assert!(
        obj_b.is_some(),
        "R-0075-b/FF-4: a `send` to an ALREADY-EXISTING addressee must still succeed (resolve, \
         not mint); guarantee-absent today (`send` is unrouted). Got: {res_b:?}"
    );

    let post_send_registration_count =
        registration_audit_count_for_role_instance(&pool, workspace_id, &existing_addressee).await;
    assert_eq!(
        post_send_registration_count, 1,
        "FF-4: a send to an ALREADY-EXISTING addressee must emit NO additional `registration` \
         audit — the count for `{existing_addressee}` must stay at 1 (from its own bind); found \
         {post_send_registration_count}."
    );

    server.abort();
}

// ===========================================================================
// Test 6 — unattached session → not_attached (R-0064-e, the new gate)
// ===========================================================================

/// GIVEN a fresh session that has NEVER bound via `message poll`,
/// WHEN it calls `message send` on an otherwise-valid request,
/// THEN it is refused `not_attached` and NO `messages` row is created. *(the
/// attach gate, exercisable at Task 7 per the plan's cross-slice obligation —
/// distinct from test 4's ordering-pin sub-case: this test's headline claim
/// is the refusal code alone, over a simple fixture, not the mint/audit
/// side-effect.)*
///
/// RED against the parent commit: `send` is unsupported — the call errors
/// with `INVALID_PARAMS` regardless of attachment state; `not_attached` is
/// entirely absent.
#[tokio::test]
async fn send_from_unattached_session_is_refused_not_attached() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let addressee_role = format!("unattached-target-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;

    // No `poll` call — this session never attaches.
    let res = client
        .call_tool(send_params(
            token.as_str(),
            &addressee_role,
            HANDOFF_TYPE_NAME,
            HANDOFF_V1,
            conformant_handoff_payload("unattached"),
        ))
        .await;

    assert!(
        result_surfaces_code(&res, "not_attached"),
        "R-0064-e: `message send` from a session with no live attachment must be refused \
         `not_attached`. Against the unrouted `send` action the code is absent \
         (`INVALID_PARAMS` instead). Got: {res:?}"
    );

    let count = message_count(&pool, workspace_id).await;
    assert_eq!(
        count, 0,
        "a `not_attached` refusal must create no `messages` row; found {count}."
    );

    server.abort();
}

// ===========================================================================
// Test 7 — read_observer refused pre-dispatch (R-0073-b)
// ===========================================================================

/// GIVEN a `read_observer`-scoped token (never attached — attach is itself
/// write-category, so a read_observer cannot bind either),
/// WHEN it calls `message send` on an otherwise-valid request,
/// THEN the call is denied at the host-fn boundary with
/// `PERMISSION_DENIED_CODE` — an AUTHORIZATION error, distinct from a `send`
/// refusal `reason_code`; the send body is never reached (no `messages` row
/// is created). *(R-0073-b)*
///
/// Unlike `coordination_leases.rs`'s analogous `claim acquire` scenario
/// (which coincidentally passes today via the generic plugin-dispatch
/// fallback, since `claim` is entirely unrouted as a *tool*), `message` IS
/// already coordination-routed (`session_plane::is_coordination_tool`
/// recognizes it) — so a `send` action call for EITHER token role hits the
/// SAME `parse_action` catch-all `INVALID_PARAMS`, never
/// `PERMISSION_DENIED_CODE`, today. This is therefore a genuine
/// guarantee-absent RED here (not a green-on-arrival contract guard): the
/// dedicated per-action gate (`authorize_coordination_action`) is never
/// reached because `send` fails to parse before the gate is consulted.
///
/// RED against the parent commit: the call errors with `INVALID_PARAMS`
/// (parse failure), never `PERMISSION_DENIED_CODE` — the assertion below
/// fails on the code-value comparison.
#[tokio::test]
async fn send_from_read_observer_token_is_refused_pre_dispatch() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let ro_token = seed_read_observer_token(&pool, workspace_id).await;
    let addressee_role = format!("ro-denied-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;

    let res = client
        .call_tool(send_params(
            ro_token.as_str(),
            &addressee_role,
            HANDOFF_TYPE_NAME,
            HANDOFF_V1,
            conformant_handoff_payload("read_observer probe"),
        ))
        .await;

    let err = res.expect_err(
        "R-0073-b: a `read_observer` token calling `message send` must be denied at the host-fn \
         boundary (Err), never receive a send-body refusal or success (Ok).",
    );

    match err {
        rmcp::ServiceError::McpError(ref error_data) => {
            assert_eq!(
                error_data.code, PERMISSION_DENIED_CODE,
                "R-0073-b: a `read_observer` calling `message send` must be denied with \
                 PERMISSION_DENIED_CODE ({PERMISSION_DENIED_CODE:?}); got {:?}. Today the parse \
                 failure (`send` unsupported) fires FIRST, before the per-action gate — see this \
                 test's doc comment.",
                error_data.code
            );
        }
        other => panic!(
            "R-0073-b: expected an `rmcp::ServiceError::McpError` carrying PERMISSION_DENIED_CODE; \
             got a different error variant: {other:?}"
        ),
    }

    let count = message_count(&pool, workspace_id).await;
    assert_eq!(
        count, 0,
        "a read_observer denial must never reach the send body; found {count} message(s)."
    );

    server.abort();
}

// ===========================================================================
// Test 8 — log-field hygiene (R-0075-e)
// ===========================================================================

/// GIVEN an attached sender session,
/// WHEN it `send`s a payload that violates the schema (undeclared field),
/// THEN the resulting `mnemra::coordination` op-log line(s) carry the
/// violation's `code`/`type_name`/`schema_version`/`detail` as DISCRETE
/// structured `tracing` fields — NEVER the `MessageValidationError` `Display`
/// string ("schema violation for (...): ...", the anchor named at
/// `message_types.rs:155-163`). *(R-0075-e)*
///
/// # Non-vacuity discipline
///
/// A capture-liveness canary (mirrors
/// `coordination_leases.rs::successful_acquire_emits_op_log_entry`) proves
/// the `tracing_test` capture channel is genuinely recording before the
/// absence/presence assertion is trusted — otherwise "no hygiene violation
/// found" could mean "the channel captured nothing at all", not "the
/// violating form never appears".
///
/// RED against the parent commit: `send` is unsupported — the call never
/// reaches any coordination write/validation path, so NO log line ever
/// carries the discrete fields (nor, trivially, the Display prose) —
/// `carries_discrete_fields` is false, failing the primary (positive) half
/// of the assertion.
#[traced_test]
#[tokio::test]
async fn schema_violation_log_entry_carries_discrete_structured_fields_not_display_string() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let token = seed_admin_token(&pool, Uuid::new_v4()).await;
    let sender_role = format!("hygiene-sender-{}", Uuid::new_v4());
    let addressee_role = format!("hygiene-addressee-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &sender_role).await;

    // Capture-liveness canary.
    tracing::info!("coordination_messages oplog canary");
    assert!(
        logs_contain("coordination_messages oplog canary"),
        "the traced_test capture channel must be live before the log-hygiene assertion is trusted"
    );

    let res = client
        .call_tool(send_params(
            token.as_str(),
            &addressee_role,
            HANDOFF_TYPE_NAME,
            HANDOFF_V1,
            handoff_payload_with_undeclared_field("hygiene probe"),
        ))
        .await;

    let schema_version_str = HANDOFF_V1.to_string();

    logs_assert(|lines: &[&str]| {
        let coordination_lines: Vec<&&str> = lines
            .iter()
            .filter(|l| l.contains("mnemra::coordination"))
            .collect();

        // Primary (positive, guarantee-absent) anchor: a coordination log line
        // carries the refusal code AND the violated type's name AND its
        // schema-version, as separate tokens on one line — the discrete-field
        // shape.
        let carries_discrete_fields = coordination_lines.iter().any(|line| {
            line.contains("schema_violation")
                && line.contains(HANDOFF_TYPE_NAME)
                && line.contains(&schema_version_str)
        });

        // Secondary (negative) guard: the interpolated `Display` prose from
        // `MessageValidationError` (message_types.rs:164-184) never appears —
        // "schema violation for (...): ..." or the `UnknownType` sibling
        // "... is not registered".
        let carries_display_prose = lines.iter().any(|line| {
            line.contains("schema violation for (") || line.contains("is not registered")
        });

        if carries_discrete_fields && !carries_display_prose {
            Ok(())
        } else {
            Err(format!(
                "R-0075-e: a `schema_violation` op-log entry must carry `code`/`type_name`/\
                 `schema_version`/`detail` as DISCRETE structured fields, never the \
                 `MessageValidationError` `Display` string (anchor: message_types.rs:155-163). \
                 carries_discrete_fields={carries_discrete_fields} \
                 carries_display_prose={carries_display_prose}. Absent today — `send` is \
                 unrouted, so no such log line is ever emitted. send result: {res:?}. captured \
                 `mnemra::coordination` lines: {coordination_lines:?}"
            ))
        }
    });

    server.abort();
}
