//! Session-plane attach-contract acceptance tests (Task 4 sub-run c — RED).
//!
//! # What these tests contract (the spec guarantees, not the b0 skeleton)
//!
//! b0 ([`938d1d4`]) landed the host-served `message` tool's *skeleton*: the
//! advertisement (`poll` action), the closed-`action` parser, the per-action
//! `read_observer` gate ([`crate::mcp::dispatch::authorize_coordination_action`],
//! already unit-tested — out of scope here), and a **placeholder** `poll` route
//! ([`crate::coordination::session_plane::poll_placeholder`]) that returns
//! `{ status: "not_implemented" }` and **writes nothing to the store**. These
//! tests encode the R-0064 attach contract that the GREEN implementer (Task 4c)
//! must fill, and so they FAIL against the placeholder by design — each for a
//! guarantee-ABSENT reason (no actor minted, no attachment lease, no audit row,
//! no structured refusal), not a compile error and not vacuously.
//!
//! # Test surface — sanctioned black-box + DB observation
//!
//! The coordination surface is host-internal (no CLI/HTTP yet). Per the Task-4c
//! carve-out (identical to Task 3's `coordination_failclosed.rs` precedent), the
//! bind is driven through the **host-served `message` MCP tool** over the real
//! `rmcp` client (`{ action: "poll", role_instance }` via `call_tool`), and
//! outcomes are observed by querying `actors` / `leases` / `coordination_audit`
//! directly. This is not a black-box violation — DB observation of a
//! host-internal surface is sanctioned; the public API is read only for
//! signatures, never for ported logic.
//!
//! # Non-vacuity discipline (the Task-3 trap, held)
//!
//! For every *refusal* AC the non-vacuity anchor is the structured **reason-code**
//! (`invalid_role_instance`, `actor_live_attached`, `wrong_actor_type`), NOT the
//! absent side-effect: "no row minted" / "no attachment" passes vacuously against
//! a placeholder that also writes nothing, so the reason-code is what makes the
//! test red. For the concurrency AC the anchor is "exactly one live attachment
//! `== 1`" (fails at 0 against the placeholder — never the vacuous `<= 1`, which
//! passes when neither session attaches).
//!
//! # No `test-hooks` feature
//!
//! Unlike `coordination_failclosed.rs`, this file needs no fault-injection seam —
//! it drives the real MCP path and reads real rows. It is a plain integration
//! test, active under the default feature set.
//!
//! # STANDING GATE FINDING (green must action before its verify gate is trusted)
//!
//! This binary is NOT in the justfile `PG_TEST_FLAGS` allowlist (line 112). Until
//! `--test coordination_session_plane` is added there, `verify-test` NEVER RUNS
//! this suite and passes anyway — a false-green over the whole Task-4c cycle
//! (silent-failure class, #2004). It is PG-touching → belongs in `PG_TEST_FLAGS`
//! (runs at `--test-threads 1`), not `NONPG_TEST_FLAGS`. The justfile is outside
//! this dispatch's touch-scope, so the registration is the green dispatch's to do.

#[path = "common/shared_engine.rs"]
mod shared_engine;

use std::sync::Arc;
use std::time::Duration;

use mnemra_host::coordination::CoordinationConfig;
use mnemra_host::mcp::server::MnemraMcpServer;
use mnemra_host::plugin::pool::PluginPool;
use mnemra_host::storage::postgres::engine::EmbeddedEngine;

use rmcp::model::{CallToolRequestParams, CallToolResult, Meta};
use rmcp::service::{RoleClient, RunningService, serve_client, serve_server};
use serde_json::json;
use tokio::io::duplex;
use uuid::Uuid;

use mnemra_host::auth::token::{AdminToken, generate, hash};

// ===========================================================================
// Harness
// ===========================================================================

/// Seed an admin-role token into `admin_tokens`, returning the raw token for the
/// MCP `_meta.token` field. `scopes = ["admin"]` → `Role::Admin`, so the poll
/// clears the R-0073-b `read_observer` gate and reaches the attach body.
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

/// Build a `Meta` carrying the auth token (open seam #1 — `_meta.token`).
fn token_meta(token_str: &str) -> Meta {
    let mut meta = Meta::new();
    meta.insert("token".to_owned(), json!(token_str));
    meta
}

/// A bare `PluginPool` with NO echo component registered.
///
/// The host-served coordination `call_tool` branch never touches the plugin pool
/// (host-served, no WASM dispatch — R-0063-a), so a bare pool suffices and
/// decouples this red-run from `just plugin` (no wasm artifact needed).
fn minimal_plugin_pool() -> Arc<PluginPool> {
    Arc::new(PluginPool::new().expect("PluginPool::new"))
}

/// Stand up one `MnemraMcpServer` over an in-memory duplex transport and return
/// its server task handle + a connected `rmcp` client. Multiple pairs may share
/// one provisioned DB `pool` — attachment state is DB-backed (`leases`), so two
/// independent serving paths race over the shared `leases_live_resource_uq`
/// index, which is the shared arbiter (R-0064-c: "uniqueness is the schema
/// constraint, not serving-path filtering").
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
            Err(e) => eprintln!("coordination test server init failed: {e:?}"),
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");
    (handle, client)
}

/// Build the `message` `poll` bind call for `role_instance` under `token_str`.
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

/// True iff the call result surfaces coordination reason-`code` in its structured
/// content (a refusal `{ refused: true, reason_code, detail }`, spec §API
/// Contract) or, defensively, anywhere in its serialized body / protocol error.
///
/// The reason-codes are a **closed machine enum** (`actor_live_attached`,
/// `wrong_actor_type`, `invalid_role_instance`, …) and the result is a machine
/// JSON payload — so a serialized-contains scan is exact here, NOT the
/// over-matching-prose hazard `skills/bdd.md` warns about (that concern is
/// rendered documentation, not a machine result envelope). The b0 placeholder's
/// `not_implemented` marker is disjoint from every coordination code, so this
/// returns `false` against it — the guarantee-absent red anchor for the refusal
/// ACs.
fn result_surfaces_code<E: std::fmt::Debug>(
    result: &Result<CallToolResult, E>,
    code: &str,
) -> bool {
    match result {
        Ok(r) => serde_json::to_string(r)
            .map(|s| s.contains(code))
            .unwrap_or(false),
        Err(e) => format!("{e:?}").contains(code),
    }
}

// ----- DB observers (outcome truth, per the sanctioned carve-out) -----

/// The `actors.id` for `(workspace_id, name)`, if a row exists.
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

/// Count of `actors` rows for `(workspace_id, name)` — the resolve-or-create /
/// no-mint invariant surface.
async fn actor_count_by_name(pool: &sqlx::PgPool, workspace_id: Uuid, name: &str) -> i64 {
    let (n,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM actors WHERE workspace_id = $1 AND name = $2")
            .bind(workspace_id)
            .bind(name)
            .fetch_one(pool)
            .await
            .expect("actors count query must execute");
    n
}

/// Count of LIVE attachment leases held by `actor_id` — the one-live-per-actor
/// surface (R-0064-c). Observed by `holder_actor_id` + reserved `actor:` family
/// + non-terminal, NEVER by an exact `resource` string: spec §Data Model L344
/// says `actor:<role-instance>` while the seam map says `actor:<actor_id>`;
/// R-0064-f makes the qualifier green's choice, so only the family prefix and the
/// holder are asserted.
async fn live_attachment_count(pool: &sqlx::PgPool, workspace_id: Uuid, actor_id: Uuid) -> i64 {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM leases
         WHERE workspace_id = $1
           AND holder_actor_id = $2
           AND resource LIKE 'actor:%'
           AND terminal_state IS NULL",
    )
    .bind(workspace_id)
    .bind(actor_id)
    .fetch_one(pool)
    .await
    .expect("live-attachment count query must execute");
    n
}

/// The `workspace_id` of a registration/attachment audit row for `actor_id`, if
/// any (R-0075-b subset). Returns the row's own `workspace_id` so the tenancy
/// assertion checks the emitted value, not merely a filter.
async fn registration_or_attachment_audit_ws(pool: &sqlx::PgPool, actor_id: Uuid) -> Option<Uuid> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT workspace_id FROM coordination_audit
         WHERE actor_id = $1
           AND event_type IN ('registration', 'attachment')
         LIMIT 1",
    )
    .bind(actor_id)
    .fetch_optional(pool)
    .await
    .expect("coordination_audit read-back query must execute");
    row.map(|(ws,)| ws)
}

// ===========================================================================
// R-0064-a #1 — resolve-or-create identity: same role-instance → same actor
// ===========================================================================

/// GIVEN two sequential sessions (distinct admin tokens) in one workspace,
/// WHEN each binds `poll(role_instance = R)` for the same identifier R,
/// THEN exactly one `actors` row exists for R — the first bind mints it, the
/// second resolves the same row (actor identity is by role-instance name,
/// independent of session and of the second bind's attach outcome). *(R-0064-a)*
///
/// RED against b0: `poll_placeholder` writes nothing, so ZERO `actors` rows exist
/// after both polls — the "exactly one" assertion fails guarantee-absent.
#[tokio::test]
async fn sequential_same_role_instance_resolves_one_actor() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token_a = seed_admin_token(&pool, workspace_id).await;
    let token_b = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("merger-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;

    // First session binds (mints the actor); second session binds the SAME
    // role-instance (must resolve the same row, mint nothing new).
    let r1 = client
        .call_tool(poll_params(token_a.as_str(), &role_instance))
        .await;
    let r2 = client
        .call_tool(poll_params(token_b.as_str(), &role_instance))
        .await;

    let count = actor_count_by_name(&pool, workspace_id, &role_instance).await;
    assert_eq!(
        count, 1,
        "R-0064-a: two sequential binds of role-instance `{role_instance}` must resolve to \
         exactly ONE actor row (first mints, second reuses); found {count}. Against the b0 \
         placeholder this is 0 — no resolve-or-create happens. r1={r1:?} r2={r2:?}"
    );

    server.abort();
}

// ===========================================================================
// R-0064-a #2 — identifier rule: a bad role-instance is refused, mints no row
// ===========================================================================

/// GIVEN a session presenting a role-instance identifier that fails the one
/// host-registered rule (whitespace — spec R-0064-a bars whitespace/control
/// chars),
/// WHEN it binds `poll(role_instance = "bad name")`,
/// THEN the bind is refused `invalid_role_instance` and NO `actors` row is
/// minted. The validator is shared by bind + send-side (send-side lands Task 7).
/// *(R-0064-a)*
///
/// RED against b0: the placeholder never validates the identifier — it returns
/// `not_implemented`, which does not surface `invalid_role_instance`. The
/// reason-code assertion (not the no-mint side-effect, which passes vacuously) is
/// the guarantee-absent anchor.
#[tokio::test]
async fn invalid_role_instance_is_refused_and_mints_no_actor() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    // Whitespace makes this invalid under the spec-enumerated rule (no whitespace
    // or control characters). The exact grammar is impl-tier, but whitespace is
    // spec-stated, so any conforming validator rejects it.
    let bad_role_instance = "merger lane with spaces";

    let (server, client) = coordination_server(&pool).await;
    let res = client
        .call_tool(poll_params(token.as_str(), bad_role_instance))
        .await;

    // Non-vacuity anchor: the structured refusal reason-code.
    assert!(
        result_surfaces_code(&res, "invalid_role_instance"),
        "R-0064-a: a bind naming an identifier that fails the host rule must be refused \
         `invalid_role_instance`. The b0 placeholder returns not_implemented (no validation), \
         so the code is absent. Got: {res:?}"
    );

    // Secondary invariant: no row minted for the rejected identifier.
    let count = actor_count_by_name(&pool, workspace_id, bad_role_instance).await;
    assert_eq!(
        count, 0,
        "R-0064-a: an identifier-rule refusal mints NO actor row; found {count} for \
         `{bad_role_instance}`."
    );

    server.abort();
}

// ===========================================================================
// R-0064-c — one live attachment per actor (concurrency)
// ===========================================================================

/// GIVEN two concurrent sessions (two independent serving paths, two distinct
/// admin tokens) racing over one shared DB pool,
/// WHEN both bind `poll(role_instance = R)` for the same fresh R, released from a
/// shared start barrier, repeated over several rounds,
/// THEN never two live attachments: exactly ONE live attachment lease exists for
/// the actor, and exactly one racer is refused `actor_live_attached` (the loser).
/// Uniqueness is the `leases_live_resource_uq` schema constraint, not
/// serving-path filtering. *(R-0064-c)*
///
/// RED against b0: `poll_placeholder` writes nothing, so ZERO live attachments
/// exist (the `== 1` assertion fails — never the vacuous `<= 1`) and NEITHER
/// racer surfaces `actor_live_attached`. Both anchors are guarantee-absent.
///
/// Two SEPARATE `serve_server`/`serve_client` pairs (not one client, two tokens)
/// guarantee two distinct sessions under BOTH plausible green session-derivations
/// (token-derived or connection-derived): were they one session, the loser would
/// be a same-session renewal (R-0064-e), not an `actor_live_attached` competitor,
/// and the AC would evaporate.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_same_role_instance_yields_exactly_one_live_attachment() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();

    const ROUNDS: usize = 5;
    for round in 0..ROUNDS {
        // Fresh SESSIONS per round: two fresh admin tokens + two fresh serving
        // paths, so each round races two DISTINCT, UNATTACHED sessions for that
        // round's fresh actor. A session reused across rounds would still hold its
        // prior round's attachment and be refused `attachment_mismatch` (R-0064-e:
        // one session ↔ one role-instance) instead of racing cleanly for
        // `actor_live_attached` — the cross-round session reuse is a fixture
        // artifact, not what R-0064-c contracts.
        let token_a = seed_admin_token(&pool, workspace_id).await;
        let token_b = seed_admin_token(&pool, workspace_id).await;
        let (server_a, client_a) = coordination_server(&pool).await;
        let (server_b, client_b) = coordination_server(&pool).await;

        // Fresh role-instance per round → a fresh actor with no prior attachment,
        // so every round is a clean two-way race (no actor-row cleanup between
        // rounds).
        let role_instance = format!("racer-{}-{round}", Uuid::new_v4());
        let barrier = tokio::sync::Barrier::new(2);

        let fa = async {
            barrier.wait().await;
            client_a
                .call_tool(poll_params(token_a.as_str(), &role_instance))
                .await
        };
        let fb = async {
            barrier.wait().await;
            client_b
                .call_tool(poll_params(token_b.as_str(), &role_instance))
                .await
        };
        let (ra, rb) = tokio::join!(fa, fb);

        // Core invariant: the actor was minted, and it has exactly one live
        // attachment (the schema unique index guarantees never two).
        let actor_id = actor_id_by_name(&pool, workspace_id, &role_instance).await;
        assert!(
            actor_id.is_some(),
            "R-0064-c round {round}: a concurrent bind of `{role_instance}` must \
             resolve/mint the actor — none exists (b0 placeholder writes nothing). \
             a={ra:?} b={rb:?}"
        );
        let actor_id = actor_id.unwrap();
        let live = live_attachment_count(&pool, workspace_id, actor_id).await;
        assert_eq!(
            live, 1,
            "R-0064-c round {round}: exactly ONE live attachment must exist for \
             `{role_instance}` after two concurrent binds; found {live}. Against the b0 \
             placeholder this is 0. a={ra:?} b={rb:?}"
        );

        // The loser is refused `actor_live_attached`; exactly one racer refused.
        let a_refused = result_surfaces_code(&ra, "actor_live_attached");
        let b_refused = result_surfaces_code(&rb, "actor_live_attached");
        assert!(
            a_refused ^ b_refused,
            "R-0064-c round {round}: exactly ONE racer must be refused \
             `actor_live_attached` (the loser); the other attaches. \
             a_refused={a_refused} b_refused={b_refused}. Against the b0 placeholder \
             NEITHER is refused (both return not_implemented). a={ra:?} b={rb:?}"
        );

        // Tear down this round's serving paths so the next round starts from two
        // fresh, unattached sessions.
        server_a.abort();
        server_b.abort();
    }
}

// ===========================================================================
// R-0064-c — wrong actor type: attach refused on a human/system row
// ===========================================================================

/// GIVEN a pre-existing `human`-type actor row (seeded directly),
/// WHEN a session binds `poll(role_instance = R)` naming that row,
/// THEN the bind is refused `wrong_actor_type` and NO attachment is laundered
/// onto the human row — only `agent`-type rows are attachable, so a session
/// cannot pose as host or human action. *(R-0064-c)*
///
/// RED against b0: the placeholder never inspects the resolved `actor_type` — it
/// returns `not_implemented`, which does not surface `wrong_actor_type`. The
/// reason-code (not the no-attachment side-effect, vacuous against the
/// placeholder) is the guarantee-absent anchor.
#[tokio::test]
async fn attaching_to_a_human_actor_is_refused_wrong_actor_type() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;

    // Seed a non-agent (human) actor row directly. resolve-or-create returns the
    // PERSISTED type, so a bind naming this row resolves `human` and must refuse.
    let human_name = format!("person-{}", Uuid::new_v4());
    let (human_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO actors (workspace_id, actor_type, name)
         VALUES ($1, 'human', $2)
         RETURNING id",
    )
    .bind(workspace_id)
    .bind(&human_name)
    .fetch_one(&pool)
    .await
    .expect("seed human actor failed");

    let (server, client) = coordination_server(&pool).await;
    let res = client
        .call_tool(poll_params(token.as_str(), &human_name))
        .await;

    assert!(
        result_surfaces_code(&res, "wrong_actor_type"),
        "R-0064-c: binding to a `human`-type actor must be refused `wrong_actor_type`. \
         The b0 placeholder does not inspect the resolved type, so the code is absent. \
         Got: {res:?}"
    );

    let live = live_attachment_count(&pool, workspace_id, human_id).await;
    assert_eq!(
        live, 0,
        "R-0064-c: no attachment may be laundered onto a `human` actor row; found {live} \
         live attachment(s) for the human actor."
    );

    server.abort();
}

// ===========================================================================
// R-0075-b — attach/registration audit emit
// ===========================================================================

/// GIVEN a session that successfully binds a fresh agent role-instance,
/// WHEN the bind commits,
/// THEN a `coordination_audit` row of the registration/attachment kind exists for
/// the actor, and its `workspace_id` is the session's workspace (tenancy). *(the
/// registration/attachment subset of R-0075-b)*
///
/// RED against b0: the placeholder commits no bind, so no actor is minted and no
/// audit row exists — the "audit row present for this actor" assertion fails
/// guarantee-absent.
#[tokio::test]
async fn successful_bind_emits_a_registration_or_attachment_audit_row() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("design-lane-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    let res = client
        .call_tool(poll_params(token.as_str(), &role_instance))
        .await;

    // The bind must have minted the actor (precondition for an audit row).
    let actor_id = actor_id_by_name(&pool, workspace_id, &role_instance).await;
    assert!(
        actor_id.is_some(),
        "R-0075-b: a successful bind must resolve/mint the actor `{role_instance}`; none \
         exists (b0 placeholder commits no bind). Got: {res:?}"
    );
    let actor_id = actor_id.unwrap();

    // The registration/attachment audit row exists AND carries the session's
    // workspace (tenancy — R-0076-b): assert on the emitted value, not a filter.
    let audit_ws = registration_or_attachment_audit_ws(&pool, actor_id).await;
    assert_eq!(
        audit_ws,
        Some(workspace_id),
        "R-0075-b: a successful bind must emit a registration/attachment `coordination_audit` \
         row for the actor, scoped to the session's workspace. Found {audit_ws:?}; expected \
         Some({workspace_id}). Against the b0 placeholder no audit row exists. Got: {res:?}"
    );

    server.abort();
}

// ===========================================================================
// R-0064-b — no acting-actor parameter (green-on-arrival CONTRACT GUARD)
// ===========================================================================

/// GIVEN the advertised MCP tool list over the wire,
/// WHEN a client lists tools,
/// THEN the `message` tool schema declares `action` + `role_instance` and carries
/// NO acting-actor / principal parameter — the principal is host-derived from
/// attachment state, never a caller write input. *(R-0064-b)*
///
/// NOT a guarantee-absent red: b0 already advertises the correct schema (and
/// unit-tests it in `coordination::session_plane`). This is authored as an
/// explicit **green-on-arrival contract guard** — it passes on b0 and STAYS
/// passing when green lands, so it does not trip StuckDetector; it is a plain
/// regression guard over the *wire* `list_tools → coordination_tools` wiring the
/// client actually sees (a distinct surface from the module unit test). The
/// companion "write path reads the principal only from session state" half is a
/// construction/review-audit item for Warden's post-Task-4 review — there is no
/// write path yet (placeholder), so it is not runtime-testable red here.
#[tokio::test]
async fn message_tool_schema_carries_no_acting_actor_parameter() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let (server, client) = coordination_server(&pool).await;
    let tools = client
        .list_all_tools()
        .await
        .expect("list_tools must succeed (unauthenticated)");

    let message = tools
        .iter()
        .find(|t| t.name.as_ref() == "message")
        .expect("R-0063-a: the `message` coordination tool must be advertised over the wire");

    let props = message
        .input_schema
        .get("properties")
        .and_then(|v| v.as_object())
        .expect("R-0064-b: the `message` schema must declare a non-empty `properties` object");

    assert!(
        props.contains_key("action") && props.contains_key("role_instance"),
        "R-0064-b: the `message` schema must declare `action` + `role_instance`; got keys: {:?}",
        props.keys().collect::<Vec<_>>()
    );

    for forbidden in ["actor", "actor_id", "acting_actor", "principal", "sender"] {
        assert!(
            !props.contains_key(forbidden),
            "R-0064-b: the coordination tool schema MUST NOT carry an acting-actor field \
             (found `{forbidden}`) — the principal is host-derived from attachment state."
        );
    }

    server.abort();
}

// ###########################################################################
// # Sub-run d — audited succession over a stale attachment (R-0064-d) + TTL
// # renew-on-activity (plan Test Expectations). RED against c-green (d4dffb1).
// ###########################################################################
//
// # What sub-run d contracts (the succession guarantees c-green does NOT yet
// # implement)
//
// c-green ([`d4dffb1`]) landed the *fresh* attach body: resolve-or-create +
// one-live-per-actor + wrong-actor-type + identifier validation + the
// registration/attachment audit. It does NOT implement succession: an
// expired-but-non-terminal attachment STILL occupies the `leases_live_resource_uq`
// slot (the index predicate is `terminal_state IS NULL`, not expiry), so a
// successor bind collides (SQLSTATE 23505) and is wrongly refused
// `actor_live_attached`. THAT wrong refusal is the guarantee-absent RED reason
// for every succession test below: succession is unimplemented, so the successor
// is refused instead of taking over.
//
// # AC ↔ test map (sub-run d)
//
// | AC / expectation | test |
// |---|---|
// | Succession over stale → takeover + supersede prior (R-0064-d) | `succession_over_stale_attachment_takes_over_and_supersedes_prior` |
// | Refused BEFORE expiry; prior untouched (R-0064-d) — GREEN-ON-ARRIVAL guard | `bind_before_expiry_is_refused_and_prior_lease_untouched` |
// | Audited takeover carries prior + successor session + expiry evidence (R-0064-d / R-0075-b) | `successful_succession_emits_audit_with_prior_successor_and_expiry_evidence` |
// | Same-session idle-gap re-bind rides the SAME audited path; equal sessions = discriminator (R-0064-d) | `same_session_idle_gap_rebind_rides_audited_succession_path` |
// | TTL renew-on-activity keeps the attachment live past the original expiry (plan Test Expectations) | `ttl_renew_on_activity_keeps_attachment_live_across_original_expiry` |
// | No new `registration` audit on succession over a pre-existing actor (c-green forward-constraint) | `succession_over_existing_actor_emits_no_new_registration_audit` |
//
// # Non-vacuity discipline (held)
//
// The RED anchor is NEVER `live_attachment_count == 1`: in RED the prior (A)
// lease stays live and non-terminal, so the count is already 1 — the invariant
// passes vacuously. The guarantee-ABSENT anchors are, per test: (1) the successor
// is NOT refused `actor_live_attached`; (2) a `taken_over` prior lease row EXISTS
// (c-green never marks A terminal); (3) an `attachment_succession` audit row
// EXISTS. Each of these is false against c-green. The `== 1` count and the
// supersession-chain checks are green-correctness guards layered on top.
//
// # TTL calibration (sanctioned)
//
// Every succession/renew test overrides `attachment_ttl` to a SECOND-scale value
// via `MnemraMcpServer::with_coordination_config` (test calibration — a supported
// configuration) and waits out a short REAL sleep past expiry. Never a wall-clock
// 10-minute sleep. Timestamps are read via `EXTRACT(EPOCH FROM …)::float8` and
// staleness is compared SQL-side (`::timestamptz`) so the test crate needs no
// `chrono` dependency.

/// Stand up an `MnemraMcpServer` with a SECOND-scale `attachment_ttl` (the
/// sanctioned test-calibration config override) so an attachment expires within
/// `attachment_ttl`; `write_timeout` stays at the default 10 s. Returns the
/// server task handle + a connected `rmcp` client, exactly like
/// [`coordination_server`].
async fn coordination_server_with_ttl(
    pool: &sqlx::PgPool,
    attachment_ttl: Duration,
) -> (tokio::task::JoinHandle<()>, RunningService<RoleClient, ()>) {
    let server = MnemraMcpServer::new(pool.clone(), minimal_plugin_pool())
        .with_coordination_config(CoordinationConfig {
            write_timeout: Duration::from_secs(10),
            attachment_ttl,
        });
    let (server_transport, client_transport) = duplex(4096);
    let handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => eprintln!("coordination test server init failed: {e:?}"),
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");
    (handle, client)
}

// ----- DB observers for the succession/lease surface (sanctioned carve-out) -----

/// The single LIVE attachment lease for `actor_id`, if any:
/// `(lease_id, session_id, expires_at_epoch_secs)`. Observed by the reserved
/// `actor:` family + non-terminal (`leases_live_resource_uq` guarantees ≤ 1).
/// `expires_at` is read as an epoch float so the test crate needs no `chrono`.
async fn live_attachment_lease(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    actor_id: Uuid,
) -> Option<(Uuid, Option<Uuid>, f64)> {
    let row: Option<(Uuid, Option<Uuid>, f64)> = sqlx::query_as(
        "SELECT id, session_id, EXTRACT(EPOCH FROM expires_at)::float8
         FROM leases
         WHERE workspace_id = $1
           AND holder_actor_id = $2
           AND resource LIKE 'actor:%'
           AND terminal_state IS NULL",
    )
    .bind(workspace_id)
    .bind(actor_id)
    .fetch_optional(pool)
    .await
    .expect("live-attachment lease query must execute");
    row
}

/// The superseded (taken-over) attachment lease for `actor_id`, if any:
/// `(lease_id, session_id, terminated_at_is_set, superseded_by)`. In c-green NO
/// such row exists (succession unimplemented → the prior lease never goes
/// terminal), so this returns `None` — the guarantee-absent RED anchor.
async fn taken_over_attachment_lease(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    actor_id: Uuid,
) -> Option<(Uuid, Option<Uuid>, bool, Option<Uuid>)> {
    let row: Option<(Uuid, Option<Uuid>, bool, Option<Uuid>)> = sqlx::query_as(
        "SELECT id, session_id, (terminated_at IS NOT NULL), superseded_by
         FROM leases
         WHERE workspace_id = $1
           AND holder_actor_id = $2
           AND resource LIKE 'actor:%'
           AND terminal_state = 'taken_over'",
    )
    .bind(workspace_id)
    .bind(actor_id)
    .fetch_optional(pool)
    .await
    .expect("taken-over lease query must execute");
    row
}

/// The `attachment_succession` audit row for `actor_id`, if any:
/// `(workspace_id, prior_session, successor_session, expires_at_evidence,
/// observed_now_evidence, observed_now >= expires_at)`. The two session fields
/// and the staleness boolean are computed from the JSONB payload SQL-side (the
/// `::timestamptz` compare avoids a `chrono` dependency). `None` in c-green (no
/// succession → no row) — the guarantee-absent RED anchor for the audit ACs.
#[allow(clippy::type_complexity)]
async fn succession_audit_evidence(
    pool: &sqlx::PgPool,
    actor_id: Uuid,
) -> Option<(
    Uuid,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<bool>,
)> {
    let row: Option<(
        Uuid,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<bool>,
    )> = sqlx::query_as(
        "SELECT
            workspace_id,
            payload->>'prior_session',
            payload->>'successor_session',
            payload->>'expires_at',
            payload->>'observed_now',
            ((payload->>'observed_now')::timestamptz >= (payload->>'expires_at')::timestamptz)
         FROM coordination_audit
         WHERE actor_id = $1 AND event_type = 'attachment_succession'",
    )
    .bind(actor_id)
    .fetch_optional(pool)
    .await
    .expect("succession-audit read-back query must execute");
    row
}

/// Count of `coordination_audit` rows of `event_type` for `actor_id`.
async fn audit_count(pool: &sqlx::PgPool, actor_id: Uuid, event_type: &str) -> i64 {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM coordination_audit WHERE actor_id = $1 AND event_type = $2",
    )
    .bind(actor_id)
    .bind(event_type)
    .fetch_one(pool)
    .await
    .expect("audit count query must execute");
    n
}

// ===========================================================================
// R-0064-d — succession over a STALE attachment: takeover + supersede prior
// ===========================================================================

/// GIVEN session A has bound `role_instance` and its attachment has EXPIRED
/// (waited past a second-scale `attachment_ttl`),
/// WHEN a DIFFERENT session B binds the SAME `role_instance`,
/// THEN B succeeds by TAKING OVER — it resolves the SAME actor, ends as the ONLY
/// live attachment, and A's prior lease is superseded (`terminal_state='taken_over'`,
/// `terminated_at` set, `superseded_by` = B's lease id). *(R-0064-d)*
///
/// RED against c-green: the expired-but-non-terminal lease still occupies the
/// unique slot, so B collides (23505) and is wrongly refused `actor_live_attached`
/// — succession is unimplemented. The guarantee-absent anchors are (1) B is NOT
/// refused and (2) a `taken_over` prior row exists.
#[tokio::test]
async fn succession_over_stale_attachment_takes_over_and_supersedes_prior() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token_a = seed_admin_token(&pool, workspace_id).await;
    let token_b = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("successor-lane-{}", Uuid::new_v4());

    // Second-scale TTL: A's attachment expires ~1 s after it binds.
    let (server, client) = coordination_server_with_ttl(&pool, Duration::from_secs(1)).await;

    // Session A binds (fresh attach — the c-green path).
    let ra = client
        .call_tool(poll_params(token_a.as_str(), &role_instance))
        .await;
    let actor_id = actor_id_by_name(&pool, workspace_id, &role_instance).await;
    assert!(
        actor_id.is_some(),
        "R-0064-d: A's bind must resolve/mint the actor `{role_instance}`. a={ra:?}"
    );
    let actor_id = actor_id.unwrap();

    // Wait past the attachment TTL so A's attachment is STALE (now >= expires_at).
    tokio::time::sleep(Duration::from_millis(2500)).await;

    // Session B (a DIFFERENT session — distinct token) binds the SAME role-instance.
    let rb = client
        .call_tool(poll_params(token_b.as_str(), &role_instance))
        .await;

    // (RED anchor #1) B must SUCCEED via succession, not be refused. c-green
    // refuses it `actor_live_attached` because the expired lease still holds the
    // slot — succession unimplemented.
    assert!(
        !result_surfaces_code(&rb, "actor_live_attached"),
        "R-0064-d: a successor bind over a STALE (expired) attachment must TAKE OVER, not be \
         refused `actor_live_attached`. c-green refuses it (succession unimplemented). b={rb:?}"
    );

    // (RED anchor #2) The prior (A) lease is superseded: a `taken_over` row exists,
    // terminated_at is set, superseded_by is set.
    let prior = taken_over_attachment_lease(&pool, workspace_id, actor_id).await;
    assert!(
        prior.is_some(),
        "R-0064-d: succession must supersede the prior attachment (terminal_state='taken_over'); \
         c-green leaves A live (no takeover). a={ra:?} b={rb:?}"
    );
    let (prior_id, prior_session, prior_terminated, prior_superseded_by) = prior.unwrap();
    assert!(
        prior_terminated,
        "R-0064-d: the superseded prior lease must carry a `terminated_at` timestamp."
    );
    assert!(
        prior_superseded_by.is_some(),
        "R-0064-d: the superseded prior lease must carry `superseded_by` (the successor lease id)."
    );

    // Exactly ONE live attachment remains, and it is the SUCCESSOR (B), not A.
    let live = live_attachment_lease(&pool, workspace_id, actor_id).await;
    assert!(
        live.is_some(),
        "R-0064-d: exactly one live attachment (the successor) must remain after takeover."
    );
    let (live_id, live_session, _live_exp) = live.unwrap();
    assert_eq!(
        live_attachment_count(&pool, workspace_id, actor_id).await,
        1,
        "R-0064-d/-c: exactly one live attachment may exist for the actor after succession."
    );

    // Supersession chain: the prior row points at the NEW live lease.
    assert_eq!(
        prior_superseded_by,
        Some(live_id),
        "R-0064-d: the prior lease's `superseded_by` must equal the successor (live) lease id. \
         prior={prior_id} live={live_id}"
    );

    // Cross-session succession: the live holder is a DIFFERENT session than the
    // superseded one (a successor session took over the prior holder's slot).
    assert!(
        prior_session.is_some() && live_session.is_some(),
        "R-0064-d: both the prior and successor leases must carry a session_id."
    );
    assert_ne!(
        prior_session, live_session,
        "R-0064-d: succession over a stale attachment hands the actor from the prior session to a \
         DIFFERENT successor session. prior_session={prior_session:?} live_session={live_session:?}"
    );

    server.abort();
}

// ===========================================================================
// R-0064-d — refused BEFORE expiry (green-on-arrival CONTRACT GUARD)
// ===========================================================================

/// GIVEN session A has bound `role_instance` and its attachment is still LIVE
/// (well within TTL — not expired),
/// WHEN a different session B binds the SAME `role_instance`,
/// THEN B is refused `actor_live_attached` and A's lease is UNTOUCHED (not
/// superseded) — succession may not fire before expiry. *(R-0064-d)*
///
/// NOT a guarantee-absent red: c-green already refuses a second LIVE bind via the
/// one-live index, and green MUST keep refusing a successor that arrives before
/// expiry (premature succession would open the TB-3 impersonation window). This
/// is an explicit CONTRACT GUARD — it passes on c-green and STAYS passing under
/// green, so it does not trip StuckDetector. It is anchored on the before-expiry
/// discriminator (a generous TTL keeps A provably LIVE) + A's row staying
/// untouched (no `taken_over`), so it guards green against premature takeover.
#[tokio::test]
async fn bind_before_expiry_is_refused_and_prior_lease_untouched() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token_a = seed_admin_token(&pool, workspace_id).await;
    let token_b = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("live-holder-{}", Uuid::new_v4());

    // Default server → the §Numeric-calibrations 10-minute TTL, so A is provably
    // LIVE (pre-expiry) when B binds immediately.
    let (server, client) = coordination_server(&pool).await;

    let ra = client
        .call_tool(poll_params(token_a.as_str(), &role_instance))
        .await;
    let actor_id = actor_id_by_name(&pool, workspace_id, &role_instance)
        .await
        .expect("R-0064-d: A's bind must resolve/mint the actor");

    // B binds IMMEDIATELY — A is well within TTL (LIVE, not stale).
    let rb = client
        .call_tool(poll_params(token_b.as_str(), &role_instance))
        .await;

    assert!(
        result_surfaces_code(&rb, "actor_live_attached"),
        "R-0064-d: a successor arriving BEFORE expiry must be refused `actor_live_attached` \
         (A is genuinely live). a={ra:?} b={rb:?}"
    );

    // A's lease is untouched: NO taken_over row, exactly one live attachment.
    assert!(
        taken_over_attachment_lease(&pool, workspace_id, actor_id)
            .await
            .is_none(),
        "R-0064-d: a before-expiry refusal must NOT supersede A's lease (no premature takeover)."
    );
    assert_eq!(
        live_attachment_count(&pool, workspace_id, actor_id).await,
        1,
        "R-0064-d: A's single live attachment stays intact through a before-expiry refusal."
    );

    server.abort();
}

// ===========================================================================
// R-0064-d / R-0075-b — audited takeover carries the succession evidence
// ===========================================================================

/// GIVEN a successful succession (A's attachment expired, B took over),
/// WHEN the takeover commits,
/// THEN a `coordination_audit` row of the `attachment_succession` kind exists for
/// the actor, scoped to the session workspace, carrying prior-holder session,
/// successor session, AND staleness/expiry evidence (`observed_now >= expires_at`).
/// The prior/successor sessions match the actual superseded/live lease rows.
/// *(R-0064-d, R-0075-b)*
///
/// RED against c-green: no succession happens (B refused), so no
/// `attachment_succession` audit row exists — the row-existence assertion fails
/// guarantee-absent.
#[tokio::test]
async fn successful_succession_emits_audit_with_prior_successor_and_expiry_evidence() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token_a = seed_admin_token(&pool, workspace_id).await;
    let token_b = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("audited-lane-{}", Uuid::new_v4());

    let (server, client) = coordination_server_with_ttl(&pool, Duration::from_secs(1)).await;

    let _ra = client
        .call_tool(poll_params(token_a.as_str(), &role_instance))
        .await;
    let actor_id = actor_id_by_name(&pool, workspace_id, &role_instance)
        .await
        .expect("R-0064-d: A's bind must resolve/mint the actor");

    tokio::time::sleep(Duration::from_millis(2500)).await;

    let rb = client
        .call_tool(poll_params(token_b.as_str(), &role_instance))
        .await;

    // (RED anchor) The succession must emit an `attachment_succession` audit row.
    let evidence = succession_audit_evidence(&pool, actor_id).await;
    assert!(
        evidence.is_some(),
        "R-0064-d/R-0075-b: a successful succession must emit an `attachment_succession` audit row; \
         none exists (c-green refuses the successor → no succession → no audit). b={rb:?}"
    );
    let (audit_ws, prior_session, successor_session, ev_expires, ev_observed, stale_ok) =
        evidence.unwrap();

    // Tenancy: the audit is scoped to the session workspace (R-0076-b).
    assert_eq!(
        audit_ws, workspace_id,
        "R-0076-b: the succession audit must be scoped to the session's workspace."
    );

    // The prior/successor session evidence must match the ACTUAL lease rows.
    let (_pid, prior_lease_session, _pt, _ps) =
        taken_over_attachment_lease(&pool, workspace_id, actor_id)
            .await
            .expect("R-0064-d: a superseded (taken_over) lease must exist");
    let (_lid, live_lease_session, _le) = live_attachment_lease(&pool, workspace_id, actor_id)
        .await
        .expect("R-0064-d: a live successor lease must exist");
    assert_eq!(
        prior_session,
        prior_lease_session.map(|u| u.to_string()),
        "R-0064-d: audit `prior_session` must equal the superseded lease's session."
    );
    assert_eq!(
        successor_session,
        live_lease_session.map(|u| u.to_string()),
        "R-0064-d: audit `successor_session` must equal the successor (live) lease's session."
    );
    assert_ne!(
        prior_session, successor_session,
        "R-0064-d: a cross-session succession — prior and successor sessions differ."
    );

    // Staleness / expiry evidence present AND internally consistent.
    assert!(
        ev_expires.as_deref().is_some_and(|s| !s.is_empty()),
        "R-0064-d: the succession audit must carry the prior holder's expiry evidence (`expires_at`)."
    );
    assert!(
        ev_observed.as_deref().is_some_and(|s| !s.is_empty()),
        "R-0064-d: the succession audit must carry the store-clock staleness evidence (`observed_now`)."
    );
    assert_eq!(
        stale_ok,
        Some(true),
        "R-0064-d: the succession's expiry evidence must show `observed_now >= expires_at` \
         (the prior attachment was genuinely stale when taken over)."
    );

    server.abort();
}

// ===========================================================================
// R-0064-d — same-session idle-gap re-bind rides the SAME audited path
// ===========================================================================

/// GIVEN session A bound `role_instance`, then its attachment EXPIRED,
/// WHEN the SAME session (equal `session_id` — same token) re-binds the same
/// `role_instance`,
/// THEN the re-bind rides the SAME audited succession/takeover path (a
/// `attachment_succession` audit row, prior lease superseded), and the
/// discriminator is that the two session identifiers are EQUAL (prior_session ==
/// successor_session). *(R-0064-d)*
///
/// RED against c-green: A's own expired-but-non-terminal lease still holds the
/// slot, so the re-bind collides (23505) and is refused `actor_live_attached` —
/// the same-session succession path is unimplemented.
#[tokio::test]
async fn same_session_idle_gap_rebind_rides_audited_succession_path() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("idle-resume-{}", Uuid::new_v4());

    let (server, client) = coordination_server_with_ttl(&pool, Duration::from_secs(1)).await;

    // A binds (fresh attach).
    let _r1 = client
        .call_tool(poll_params(token.as_str(), &role_instance))
        .await;
    let actor_id = actor_id_by_name(&pool, workspace_id, &role_instance)
        .await
        .expect("R-0064-d: the first bind must resolve/mint the actor");

    // Idle gap: the attachment expires.
    tokio::time::sleep(Duration::from_millis(2500)).await;

    // The SAME session (SAME token → equal session_id) re-binds.
    let r2 = client
        .call_tool(poll_params(token.as_str(), &role_instance))
        .await;

    // (RED anchor #1) The same-session re-bind after expiry must ride the
    // succession path, not be refused.
    assert!(
        !result_surfaces_code(&r2, "actor_live_attached"),
        "R-0064-d: a same-session idle-gap re-bind after expiry must ride the audited succession \
         path, not be refused `actor_live_attached`. c-green refuses it (unimplemented). r2={r2:?}"
    );

    // (RED anchor #2) The expired prior lease is superseded (taken_over).
    let prior = taken_over_attachment_lease(&pool, workspace_id, actor_id).await;
    assert!(
        prior.is_some(),
        "R-0064-d: the idle-gap re-bind must supersede the expired prior lease (taken_over). r2={r2:?}"
    );
    let (_prior_id, prior_session, prior_terminated, prior_superseded_by) = prior.unwrap();
    assert!(
        prior_terminated,
        "R-0064-d: the superseded prior lease must carry a `terminated_at` timestamp."
    );

    let (live_id, live_session, _le) = live_attachment_lease(&pool, workspace_id, actor_id)
        .await
        .expect("R-0064-d: a live successor lease must exist after the re-bind");
    assert_eq!(
        prior_superseded_by,
        Some(live_id),
        "R-0064-d: the prior lease's `superseded_by` must point at the new live lease."
    );

    // The SAME-session discriminator: prior and successor sessions are EQUAL.
    assert!(
        prior_session.is_some() && live_session.is_some(),
        "R-0064-d: both leases must carry a session_id."
    );
    assert_eq!(
        prior_session, live_session,
        "R-0064-d: the idle-gap re-bind discriminator is that the prior and successor sessions are \
         EQUAL (same session resumed). prior={prior_session:?} live={live_session:?}"
    );

    // The audit reflects the same path (attachment_succession) with prior ==
    // successor session (the equality being the idle-resume discriminator).
    let evidence = succession_audit_evidence(&pool, actor_id).await;
    assert!(
        evidence.is_some(),
        "R-0064-d/R-0075-b: the idle-gap re-bind must ride the SAME audited succession path \
         (an `attachment_succession` row); none exists in c-green."
    );
    let (_ws, aud_prior, aud_succ, _e, _o, _stale) = evidence.unwrap();
    assert_eq!(
        aud_prior, aud_succ,
        "R-0064-d: on the same-session idle-gap path the audit's `prior_session` and \
         `successor_session` are EQUAL (the discriminator)."
    );
    assert_eq!(
        aud_prior,
        prior_session.map(|u| u.to_string()),
        "R-0064-d: the audited session equals the re-binding session."
    );

    server.abort();
}

// ===========================================================================
// TTL renew-on-activity (plan Test Expectations) — activity extends expiry
// ===========================================================================

/// GIVEN session A bound `role_instance` (ttl = T) and its attachment is still
/// LIVE,
/// WHEN A polls again (same session) BEFORE expiry,
/// THEN the attachment is RENEWED — `expires_at` is extended from the renewal
/// moment, the re-poll SUCCEEDS (is not refused), and the attachment stays live;
/// proven by a competitor bind landing AFTER the ORIGINAL expiry but BEFORE the
/// RENEWED expiry still being refused `actor_live_attached`. A renewal is NOT a
/// succession (no `attachment_succession` audit). *(plan Test Expectations —
/// TTL renew-on-activity)*
///
/// RED against c-green: A's re-poll collides with A's own live lease (23505) and
/// is refused `actor_live_attached` — renewal is unimplemented, so `expires_at`
/// is never extended.
#[tokio::test]
async fn ttl_renew_on_activity_keeps_attachment_live_across_original_expiry() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token_a = seed_admin_token(&pool, workspace_id).await;
    let token_b = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("renew-lane-{}", Uuid::new_v4());

    // ttl = 4 s. Schedule: bind @t0 (expiry ~t0+4); renew @~t0+2 (expiry ~t0+6);
    // competitor B @~t0+5 (past ORIGINAL t0+4, before RENEWED t0+6) → still refused.
    let (server, client) = coordination_server_with_ttl(&pool, Duration::from_secs(4)).await;

    let _r1 = client
        .call_tool(poll_params(token_a.as_str(), &role_instance))
        .await;
    let actor_id = actor_id_by_name(&pool, workspace_id, &role_instance)
        .await
        .expect("TTL-renew: A's bind must resolve/mint the actor");
    let (lease_id0, _s0, expires0) = live_attachment_lease(&pool, workspace_id, actor_id)
        .await
        .expect("TTL-renew: A's fresh attachment lease must exist");

    // Before expiry (~2 s into a 4 s TTL), A polls again with the SAME session.
    tokio::time::sleep(Duration::from_millis(2000)).await;
    let r2 = client
        .call_tool(poll_params(token_a.as_str(), &role_instance))
        .await;

    // (RED anchor #1) A's re-poll while LIVE must renew and SUCCEED, not be refused.
    assert!(
        !result_surfaces_code(&r2, "actor_live_attached"),
        "TTL-renew (plan Test Expectations): a same-session re-poll while the attachment is LIVE \
         must RENEW, not be refused. c-green refuses it (renewal unimplemented). r2={r2:?}"
    );

    // (RED anchor #2) Renewal EXTENDS `expires_at` on the existing lease (in place).
    let (lease_id1, _s1, expires1) = live_attachment_lease(&pool, workspace_id, actor_id)
        .await
        .expect("TTL-renew: A's renewed attachment lease must exist");
    assert_eq!(
        live_attachment_count(&pool, workspace_id, actor_id).await,
        1,
        "TTL-renew: renewal keeps exactly one live attachment."
    );
    assert_eq!(
        lease_id1, lease_id0,
        "TTL-renew: renewal extends the EXISTING attachment lease in place (same lease id), not a \
         new row. before={lease_id0} after={lease_id1}"
    );
    assert!(
        expires1 > expires0,
        "TTL-renew: renewal must EXTEND `expires_at` (renewed from the renewal moment). \
         original_epoch={expires0} renewed_epoch={expires1}"
    );

    // Prove the renewal kept the attachment alive PAST the original expiry: a
    // competitor B binding after the ORIGINAL expiry but before the RENEWED expiry
    // is still refused `actor_live_attached` (the renewed attachment is live).
    tokio::time::sleep(Duration::from_millis(3000)).await;
    let rb = client
        .call_tool(poll_params(token_b.as_str(), &role_instance))
        .await;
    assert!(
        result_surfaces_code(&rb, "actor_live_attached"),
        "TTL-renew: after renewal, a competitor bind before the RENEWED expiry must still be \
         refused `actor_live_attached` (the renewed attachment is live). b={rb:?}"
    );

    // A renewal is NOT a succession — no `attachment_succession` audit is emitted.
    assert_eq!(
        audit_count(&pool, actor_id, "attachment_succession").await,
        0,
        "TTL-renew: a renewal is not a succession — it emits no `attachment_succession` audit."
    );

    server.abort();
}

// ===========================================================================
// c-green forward-constraint — succession over a pre-existing actor emits
// NO new registration audit (the succession path forks BEFORE fresh-attach
// registration-staging)
// ===========================================================================

/// GIVEN A's fresh bind minted the actor and emitted exactly one `registration`
/// audit,
/// WHEN a successor B takes over the (now expired) attachment over that SAME
/// pre-existing actor,
/// THEN NO new `registration` audit is emitted — the actor already exists, so the
/// green succession path must fork BEFORE the fresh-attach registration-staging;
/// only succession records accrue. *(c-green forward-constraint on R-0075-b)*
///
/// RED against c-green: the successor is refused (no succession happens), so the
/// `attachment_succession` precondition below is absent — the guarantee-absent
/// anchor. (The no-re-registration invariant is the green-correctness guard this
/// test layers on the succession that green must make happen.)
#[tokio::test]
async fn succession_over_existing_actor_emits_no_new_registration_audit() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token_a = seed_admin_token(&pool, workspace_id).await;
    let token_b = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("no-rereg-lane-{}", Uuid::new_v4());

    let (server, client) = coordination_server_with_ttl(&pool, Duration::from_secs(1)).await;

    // A's fresh bind mints the actor and emits exactly one registration audit.
    let ra = client
        .call_tool(poll_params(token_a.as_str(), &role_instance))
        .await;
    let actor_id = actor_id_by_name(&pool, workspace_id, &role_instance)
        .await
        .expect("A's fresh bind must resolve/mint the actor");
    let reg_after_a = audit_count(&pool, actor_id, "registration").await;
    assert_eq!(
        reg_after_a, 1,
        "R-0075-b baseline: A's fresh bind emits exactly one `registration` audit. a={ra:?}"
    );

    tokio::time::sleep(Duration::from_millis(2500)).await;

    let rb = client
        .call_tool(poll_params(token_b.as_str(), &role_instance))
        .await;

    // (RED anchor) The succession must actually occur — else the forward-constraint
    // below is vacuous. c-green refuses B, so no succession, no audit → RED here.
    assert!(
        succession_audit_evidence(&pool, actor_id).await.is_some(),
        "R-0064-d: the successor bind must TAKE OVER (an `attachment_succession` audit); c-green \
         refuses it → no succession → the forward-constraint guard cannot yet fire. b={rb:?}"
    );

    // (forward-constraint) Succession over a PRE-EXISTING actor emits NO new
    // registration — the green succession path forks BEFORE fresh-attach
    // registration-staging.
    let reg_after_b = audit_count(&pool, actor_id, "registration").await;
    assert_eq!(
        reg_after_b, 1,
        "c-green forward-constraint: succession over an already-registered actor must NOT emit a \
         new `registration` audit — the succession path forks BEFORE fresh-attach \
         registration-staging. reg_after_a={reg_after_a} reg_after_b={reg_after_b}"
    );

    server.abort();
}

// ###########################################################################
// # Sub-run e — poll RESPONSE shape + gates + workspace isolation (the last
// # build slice). RED against e-green's parent (5355d61).
// ###########################################################################
//
// # What sub-run e contracts (the guarantees 5355d61 does NOT yet implement)
//
// At `5355d61` the bind path is complete (fresh attach + succession + TTL
// renew), but `poll_bind` still returns only the MINIMAL attached ack
// (`{ attached, actor_id, role_instance }` — `session_plane::attached_ack`). It
// does NOT yet:
//   - return the documented §API-Contract poll response body
//     (`{ actor, messages, live_leases }`, R-0072-a);
//   - exclude the reserved `actor:`-family attachment rows from the live-lease
//     listing (R-0067-c);
//   - refuse a same-session re-poll under a DIFFERENT role-instance
//     (`attachment_mismatch`, R-0064-e);
//   - workspace-scope the live-lease listing (R-0076-b tenancy).
//
// # AC ↔ test map (sub-run e)
//
// | AC / expectation | test | posture |
// |---|---|---|
// | Poll RESPONSE shape (messages + live_leases) + actor-family exclusion (R-0072-a / R-0067-c) | `poll_response_returns_messages_and_live_leases_excluding_actor_family` | RED (guarantee-absent) |
// | Same-session re-poll with a different role-instance → `attachment_mismatch` (R-0064-e) | `same_session_poll_with_different_role_instance_is_refused_attachment_mismatch` | RED (guarantee-absent) |
// | Workspace-scoped poll — never surfaces another tenant's leases (R-0076-b / R-0072-a / R-0067-c) | `poll_is_workspace_scoped_and_excludes_other_tenants_leases` | RED (guarantee-absent, security-critical) |
// | `not_attached` unreachable at Task 4 (only `poll` binds) — deferred to Task 5 (R-0064-e); deferral retired Task 7 slice a | `message_tool_advertises_poll_and_send_after_task7_slice_a` | green-on-arrival contract guard, updated (not deleted) when the deferral retired |
// | No client-settable per-request attachment-TTL override (§Numeric calibrations) | `message_schema_carries_no_per_request_attachment_ttl_override` | green-on-arrival contract guard |
//
// # Non-vacuity discipline (held)
//
// For the two poll-body RED tests the guarantee-absent anchor is the PRESENCE of
// the documented arrays (`messages` + `live_leases`) — the `5355d61` minimal ack
// carries NEITHER key, so "response has a `live_leases` array" fails
// guarantee-absent (not vacuously, not a compile error). The `actor:`-family
// exclusion and the cross-tenant exclusion are asserted on a state genuinely
// established first (a real `actor:` attachment lease exists from the bind; a real
// foreign-tenant lease is seeded) so those become non-vacuous the instant green
// builds the body. For `attachment_mismatch` the anchor is the structured
// reason-code (a no-mint side-effect would pass vacuously against the current
// path, which fresh-attaches the second role-instance instead of refusing).
//
// # `not_attached` testability judgment (stated, per the brief)
//
// The AC "every other action from an unattached session returns `not_attached`"
// has NO Task-4 trigger: the `message` tool advertises only `poll`, and `poll` IS
// the bind — there is no non-binding coordination action from which an unattached
// session could be refused. `not_attached` becomes runtime-triggerable at Task 5
// (`claim acquire` from a fresh session) and Task 7 (`message send`/`list`/`ack`/
// `disposition` from an unattached session). No vacuous test is forced here; the
// green-on-arrival guard below pins the reasoning (only `poll` is advertised) so a
// future non-binding `message` action landing without a `not_attached` test trips
// it.

// ----- Sub-run e observers/seeders (sanctioned carve-out) -----

/// Seed an `agent`-type actor row directly, returning its id. A holder for the
/// seeded non-actor leases below (an `agent` actor is a valid `holder_actor_id`
/// FK target; the family under test is the *lease resource* family, not the
/// holder's actor type).
async fn seed_agent_actor(pool: &sqlx::PgPool, workspace_id: Uuid, name: &str) -> Uuid {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO actors (workspace_id, actor_type, name)
         VALUES ($1, 'agent', $2)
         RETURNING id",
    )
    .bind(workspace_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("seed agent actor failed");
    id
}

/// Seed a LIVE, non-terminal, non-`actor:`-family lease (a `repo-lane:` resource)
/// held by `holder_actor_id`, expiring an hour out (so it is genuinely live at
/// operation time). This is the workspace-visible lease a poll's `live_leases`
/// listing MUST surface (R-0072-a) — the non-vacuity floor for the exclusion
/// assertions (proves the listing is populated, not vacuously empty).
async fn seed_live_repo_lane_lease(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    holder_actor_id: Uuid,
    resource: &str,
) -> Uuid {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO leases
             (workspace_id, resource, holder_actor_id, acquired_at, duration, expires_at)
         VALUES ($1, $2, $3, now(), 3600, now() + interval '1 hour')
         RETURNING id",
    )
    .bind(workspace_id)
    .bind(resource)
    .bind(holder_actor_id)
    .fetch_one(pool)
    .await
    .expect("seed live repo-lane lease failed");
    id
}

/// The structured-content JSON object of a `CallToolResult`. Passes on the
/// `5355d61` minimal ack too (it IS a JSON object) — so the RED anchors below
/// land on the ABSENT keys, never on this accessor.
fn structured_obj(result: &CallToolResult) -> &serde_json::Map<String, serde_json::Value> {
    result
        .structured_content
        .as_ref()
        .and_then(|v| v.as_object())
        .expect("a poll response must carry a structured-content JSON object")
}

/// The `resource` strings of a poll response's `live_leases` array. `None` when
/// the response carries no `live_leases` array at all — the `5355d61`
/// minimal-ack case (the guarantee-absent RED surface). `Some(vec)` once green
/// builds the body.
fn live_lease_resources(result: &CallToolResult) -> Option<Vec<String>> {
    let arr = result
        .structured_content
        .as_ref()?
        .as_object()?
        .get("live_leases")?
        .as_array()?;
    Some(
        arr.iter()
            .filter_map(|l| {
                l.get("resource")
                    .and_then(|r| r.as_str())
                    .map(str::to_owned)
            })
            .collect(),
    )
}

// ===========================================================================
// R-0072-a / R-0067-c — poll RESPONSE shape: messages + live_leases, actor
// family EXCLUDED
// ===========================================================================

/// GIVEN a workspace holding one pre-existing live non-`actor:` lease
/// (`repo-lane:visible-…`),
/// WHEN a session binds `poll(role_instance)` (a fresh attach — the bind also
/// mints the actor's own `actor:<id>` attachment lease),
/// THEN the ONE response carries the documented §API-Contract poll body — a
/// `messages` array (empty at this stage; send is Task 7) AND a `live_leases`
/// array — and the live-lease listing SURFACES the `repo-lane:` lease while
/// EXCLUDING the reserved `actor:`-family attachment row. *(R-0072-a poll-shape;
/// R-0067-c exclusion)*
///
/// RED against `5355d61`: `poll_bind` returns the minimal ack
/// (`{ attached, actor_id, role_instance }`) — it carries NEITHER a `messages`
/// nor a `live_leases` key, so the "response has a `live_leases` array" anchor
/// fails guarantee-absent. The exclusion/inclusion assertions are non-vacuous the
/// instant green builds the body (a real `actor:` lease and a real `repo-lane:`
/// lease both exist in `leases`).
#[tokio::test]
async fn poll_response_returns_messages_and_live_leases_excluding_actor_family() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;

    // A pre-existing workspace-visible NON-actor lease the poll MUST surface — the
    // non-vacuity floor (proves live_leases is populated, not vacuously empty).
    let holder = seed_agent_actor(&pool, workspace_id, &format!("holder-{}", Uuid::new_v4())).await;
    let visible_resource = format!("repo-lane:visible-{}", Uuid::new_v4());
    seed_live_repo_lane_lease(&pool, workspace_id, holder, &visible_resource).await;

    let role_instance = format!("poller-{}", Uuid::new_v4());
    let (server, client) = coordination_server(&pool).await;

    let res = client
        .call_tool(poll_params(token.as_str(), &role_instance))
        .await
        .expect("the poll bind call must return a result");

    // (RED anchor) the documented poll shape: BOTH arrays present.
    let obj = structured_obj(&res);
    let messages = obj.get("messages").and_then(|v| v.as_array());
    assert!(
        messages.is_some(),
        "R-0072-a: the poll response must carry a `messages` array (empty at this stage — send is \
         Task 7); the 5355d61 minimal ack does not. got keys: {:?}",
        obj.keys().collect::<Vec<_>>()
    );
    assert!(
        messages.expect("messages array present").is_empty(),
        "R-0072-a: at Task 4 no `send` exists, so the polling actor's `messages` array is empty."
    );
    assert!(
        obj.get("live_leases").and_then(|v| v.as_array()).is_some(),
        "R-0072-a: the poll response must carry a `live_leases` array; the 5355d61 minimal ack does \
         not. got keys: {:?}",
        obj.keys().collect::<Vec<_>>()
    );

    // (green correctness) the workspace's live non-actor lease appears...
    let resources = live_lease_resources(&res).unwrap_or_default();
    assert!(
        resources.iter().any(|r| r == &visible_resource),
        "R-0072-a: the workspace's live `{visible_resource}` lease must appear in the poll's \
         live_leases; got {resources:?}"
    );
    // ...AND the reserved `actor:`-family attachment lease (which now exists in
    // `leases` from THIS bind) is EXCLUDED — the realization stays invisible on
    // the tool surface (R-0067-c).
    assert!(
        !resources.iter().any(|r| r.starts_with("actor:")),
        "R-0067-c: the reserved `actor:`-family attachment lease MUST be excluded from the poll's \
         live_leases; got {resources:?}"
    );

    server.abort();
}

// ===========================================================================
// R-0064-e — same-session re-poll with a DIFFERENT role-instance →
// attachment_mismatch
// ===========================================================================

/// GIVEN a session (one token → one session) that has bound `role_instance` A,
/// WHEN the SAME session polls again with a DIFFERENT `role_instance` B,
/// THEN it is refused `attachment_mismatch` — one session, one role-instance
/// (R-0064-e). *(R-0064-e)*
///
/// RED against `5355d61`: there is no attachment-mismatch gate. The bind path
/// keys attachment off the *actor* (`actor:<actor_id>` resource), not off a
/// session→role-instance binding, so B (a different role-instance → a different
/// actor → a free `actor:<B>` slot) simply FRESH-ATTACHES and returns an attached
/// ack. `attachment_mismatch` is absent — the guarantee-absent anchor (the
/// structured reason-code, not a no-mint side-effect which would pass vacuously).
#[tokio::test]
async fn same_session_poll_with_different_role_instance_is_refused_attachment_mismatch() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    // ONE token → ONE session for both polls (the same-session discriminator).
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_a = format!("merger-{}", Uuid::new_v4());
    let role_b = format!("design-lane-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;

    // Bind as A (a fresh attach — establishes the session's attachment to A).
    let ra = client.call_tool(poll_params(token.as_str(), &role_a)).await;
    assert!(
        !result_surfaces_code(&ra, "attachment_mismatch")
            && !result_surfaces_code(&ra, "actor_live_attached"),
        "precondition: A's fresh bind must SUCCEED (establish the session's attachment). a={ra:?}"
    );
    assert!(
        actor_id_by_name(&pool, workspace_id, &role_a)
            .await
            .is_some(),
        "precondition: A's bind must resolve/mint the actor. a={ra:?}"
    );

    // The SAME session (SAME token) polls a DIFFERENT role-instance.
    let rb = client.call_tool(poll_params(token.as_str(), &role_b)).await;

    // Non-vacuity anchor: the structured refusal reason-code.
    assert!(
        result_surfaces_code(&rb, "attachment_mismatch"),
        "R-0064-e: a session attached as `{role_a}` polling with a DIFFERENT role-instance \
         `{role_b}` must be refused `attachment_mismatch` (one session, one role-instance). \
         5355d61 has no mismatch gate — it fresh-attaches B instead. b={rb:?}"
    );

    server.abort();
}

// ===========================================================================
// R-0076-b / R-0072-a / R-0067-c — poll live-lease listing is WORKSPACE-SCOPED
// (the cross-tenant leak surface — security-critical)
// ===========================================================================

/// GIVEN two DISTINCT workspaces W and V (distinct `WorkspaceCtx` from distinct
/// tokens), each with an established attachment AND a live `repo-lane:` lease,
/// WHEN a session in W polls,
/// THEN W's poll `live_leases` surfaces ONLY W-scoped leases — it contains W's
/// `repo-lane:w-visible-…` lease and NEVER V's `repo-lane:v-secret-…` lease
/// (nor any `actor:`-family row) — the poll listing filters by the session ctx's
/// `workspace_id`. *(R-0076-b tenancy; R-0072-a poll scope; R-0067-c exclusion)*
///
/// > Brief-vs-spec note: the dispatch brief labels this "R-0065-e ws-scoping",
/// > but R-0065-e is lease *stale semantics*; the ws-scoping lock is R-0076-b
/// > ("a two-workspace fixture shows zero cross-workspace rows on every read and
/// > write path"), which is what this test anchors.
///
/// RED against `5355d61`: no workspace-scoped lease listing exists — the poll
/// body isn't built, so the minimal ack carries no `live_leases` at all. The
/// "W's poll surfaces W's lease" anchor fails guarantee-absent; V's-state and
/// W's-state are BOTH genuinely established so the cross-tenant EXCLUSION becomes
/// a real security assertion the instant green builds the body.
#[tokio::test]
async fn poll_is_workspace_scoped_and_excludes_other_tenants_leases() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let ws_w = Uuid::new_v4();
    let ws_v = Uuid::new_v4();
    let token_w = seed_admin_token(&pool, ws_w).await;
    let token_v = seed_admin_token(&pool, ws_v).await;

    let (server, client) = coordination_server(&pool).await;

    // --- Establish tenant V's state: an attachment + a live foreign-tenant lease
    // (the lease that MUST NOT leak into W's poll). ---
    let role_v = format!("v-poller-{}", Uuid::new_v4());
    let _rv = client
        .call_tool(poll_params(token_v.as_str(), &role_v))
        .await;
    assert!(
        actor_id_by_name(&pool, ws_v, &role_v).await.is_some(),
        "precondition: V's bind must establish V's attachment state. rv={_rv:?}"
    );
    let holder_v = seed_agent_actor(&pool, ws_v, &format!("v-holder-{}", Uuid::new_v4())).await;
    let v_secret_resource = format!("repo-lane:v-secret-{}", Uuid::new_v4());
    seed_live_repo_lane_lease(&pool, ws_v, holder_v, &v_secret_resource).await;

    // --- Establish tenant W's state: a live W-scoped lease that its poll MUST
    // surface (the non-vacuity floor / RED anchor). Seeded BEFORE W binds so it is
    // present in W's bind-poll response. ---
    let holder_w = seed_agent_actor(&pool, ws_w, &format!("w-holder-{}", Uuid::new_v4())).await;
    let w_visible_resource = format!("repo-lane:w-visible-{}", Uuid::new_v4());
    seed_live_repo_lane_lease(&pool, ws_w, holder_w, &w_visible_resource).await;

    // W's session polls — at this point BOTH tenants' leases exist in `leases`.
    let rw = client
        .call_tool(poll_params(token_w.as_str(), &role_from("w-poller")))
        .await
        .expect("W's poll bind call must return a result");

    // (RED anchor) W's poll carries the workspace-scoped live-lease listing.
    assert!(
        structured_obj(&rw)
            .get("live_leases")
            .and_then(|v| v.as_array())
            .is_some(),
        "R-0072-a: W's poll response must carry a `live_leases` array; the 5355d61 minimal ack does \
         not. rw keys: {:?}",
        structured_obj(&rw).keys().collect::<Vec<_>>()
    );

    let w_resources = live_lease_resources(&rw).unwrap_or_default();

    // W's OWN visible lease is present (proves the listing is populated + scoped,
    // not vacuously empty).
    assert!(
        w_resources.iter().any(|r| r == &w_visible_resource),
        "R-0072-a/R-0076-b: W's poll must surface W's own `{w_visible_resource}` lease; got \
         {w_resources:?}"
    );

    // (SECURITY) V's foreign-tenant lease NEVER appears in W's poll.
    assert!(
        !w_resources.iter().any(|r| r == &v_secret_resource),
        "R-0076-b (cross-tenant leak): W's poll live_leases MUST NOT contain tenant V's \
         `{v_secret_resource}` lease — the listing filters by the session ctx's workspace_id. got \
         {w_resources:?}"
    );
    // No `actor:`-family row leaks either (R-0067-c), from EITHER tenant.
    assert!(
        !w_resources.iter().any(|r| r.starts_with("actor:")),
        "R-0067-c: no `actor:`-family attachment row may appear in a poll's live_leases; got \
         {w_resources:?}"
    );

    server.abort();
}

/// A fresh, rule-conforming role-instance name from a stable prefix (avoids a
/// stray inline `format!` in the poll call above).
fn role_from(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4())
}

// ===========================================================================
// R-0064-e — `not_attached` is unreachable at Task 4 (deferral guard, GREEN
// on arrival)
// ===========================================================================

/// GIVEN the advertised `message` tool over the wire,
/// WHEN a client lists tools,
/// THEN the `message` tool advertises EXACTLY `[poll, send]` now that Task 7
/// slice a has landed `send` (R-0068-a). *(R-0064-e — the deferral this test
/// used to pin is now fulfilled)*
///
/// # Deferral retired (Task 7 slice a)
///
/// This test was originally
/// `message_tool_advertises_only_poll_so_not_attached_is_deferred_to_task5` — a
/// green-on-arrival CONTRACT GUARD pinning the reasoning behind the Task-4
/// absence of a `not_attached` test (no non-binding `message` action existed to
/// refuse from). Its own doc comment named the exact firing condition: "When a
/// future non-binding `message` action lands (Task 7) without a `not_attached`
/// test, this guard trips, surfacing the deferral." Task 7 slice a's `send`
/// action landed EXACTLY that non-binding action, and it ships with its OWN
/// `not_attached` coverage
/// (`tests/coordination_messages.rs::send_from_unattached_session_is_refused_not_attached`)
/// — so the guard tripped as designed, and this test is updated (not deleted)
/// to pin the NEW closed enum rather than assert the retired one. `list`/`ack`/
/// `disposition` (Task 7 slices b/c) still land later; each carries its own
/// `not_attached` coverage per slice (the plan's cross-slice obligation), so no
/// replacement deferral guard is needed here.
#[tokio::test]
async fn message_tool_advertises_poll_and_send_after_task7_slice_a() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let (server, client) = coordination_server(&pool).await;
    let tools = client
        .list_all_tools()
        .await
        .expect("list_tools must succeed (unauthenticated)");

    let message = tools
        .iter()
        .find(|t| t.name.as_ref() == "message")
        .expect("R-0063-a: the `message` coordination tool must be advertised over the wire");

    let action_enum = message
        .input_schema
        .get("properties")
        .and_then(|v| v.as_object())
        .and_then(|props| props.get("action"))
        .and_then(|a| a.get("enum"))
        .and_then(|e| e.as_array())
        .expect("R-0063-b: the `message` `action` argument must declare a closed enum");

    assert_eq!(
        action_enum,
        &vec![json!("poll"), json!("send")],
        "R-0064-e: after Task 7 slice a the `message` tool advertises exactly `[poll, send]` — \
         `send` is the non-binding action whose arrival was expected to retire this guard, and it \
         ships with its own `not_attached` coverage (`coordination_messages.rs`'s \
         `send_from_unattached_session_is_refused_not_attached`). got: {action_enum:?}"
    );

    server.abort();
}

// ===========================================================================
// §Numeric calibrations — no client-settable per-request attachment-TTL
// override (schema guard, GREEN on arrival)
// ===========================================================================

/// GIVEN the advertised `message` tool schema over the wire,
/// WHEN a client lists tools,
/// THEN the schema carries NO per-request attachment-TTL parameter and is closed
/// (`additionalProperties: false`) — the attachment TTL is host-config only, the
/// security-load-bearing knob stays deployment-scoped. *(§Numeric calibrations —
/// "no client-settable per-request override exists for the attachment TTL")*
///
/// NOT a guarantee-absent red: `5355d61`'s schema is already closed and TTL-free.
/// This is a green-on-arrival CONTRACT GUARD (same posture as the acting-actor
/// schema guard) — it passes now and STAYS passing when green builds the poll
/// body, guarding against a helpfully-added TTL knob.
#[tokio::test]
async fn message_schema_carries_no_per_request_attachment_ttl_override() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let (server, client) = coordination_server(&pool).await;
    let tools = client
        .list_all_tools()
        .await
        .expect("list_tools must succeed (unauthenticated)");

    let message = tools
        .iter()
        .find(|t| t.name.as_ref() == "message")
        .expect("R-0063-a: the `message` coordination tool must be advertised over the wire");

    // Closed schema — no arbitrary field (a TTL among them) can be injected.
    assert_eq!(
        message.input_schema.get("additionalProperties"),
        Some(&json!(false)),
        "§Numeric calibrations: the `message` schema must be closed (`additionalProperties: false`) \
         so no per-request attachment-TTL field can be smuggled in."
    );

    let props = message
        .input_schema
        .get("properties")
        .and_then(|v| v.as_object())
        .expect("the `message` schema must declare a `properties` object");

    for forbidden in [
        "ttl",
        "attachment_ttl",
        "attachment_ttl_seconds",
        "ttl_seconds",
        "expiry",
        "expires_in",
    ] {
        assert!(
            !props.contains_key(forbidden),
            "§Numeric calibrations: the `message` schema MUST NOT carry a per-request \
             attachment-TTL override (found `{forbidden}`) — TTL is host-config only, deployment-\
             scoped."
        );
    }

    server.abort();
}
