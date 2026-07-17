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
//! ## Slice (b) rows (tests 9–19; Glitch, dispatch 1634)
//!
//! | # | Test | R-ID(s) | RED cause |
//! |---|---|---|---|
//! | 9. First poll delivers + append-once `delivered_at` | `addressees_first_poll_delivers_the_message_and_sets_delivered_at_once` | R-0069-a, R-0068-c | delivery not wired |
//! | 10. Dead role-instance message delivered to successor's bind poll | `message_to_a_role_instance_whose_session_died_is_delivered_to_the_successor_bind_poll` | R-0068-a (AC1) | delivery not wired |
//! | 11. QA-3 full consumption trail (send→poll→ack→disposition) | `qa3_full_consumption_trail_send_poll_ack_disposition` | R-0068-c, R-0069-a, R-0069-c; QA-3 | delivery not wired (fails at the poll step; ack/disposition unreached this run) |
//! | 12. Illegal transition matrix + `message_not_found` | `illegal_transition_matrix_and_message_not_found_are_refused` | R-0069-a (AC4), R-0068-c (AC3 append-once half) | `ack`/`disposition` unrouted |
//! | 13. Each disposition vocabulary member dispositions a fixture | `each_disposition_vocabulary_member_dispositions_an_acknowledged_fixture` | R-0069-c | `disposition` unrouted |
//! | 14. Out-of-set disposition → `invalid_disposition` | `disposition_with_out_of_set_value_is_refused_invalid_disposition` | R-0069-c | `disposition` unrouted |
//! | 15. Disposition `note` round-trips | `disposition_note_round_trips_to_the_stored_row` | R-0069-c | `disposition` unrouted |
//! | 16. Non-addressee `ack`/`disposition` → `not_addressee`, logged | `ack_and_disposition_from_non_addressee_are_refused_not_addressee_and_logged` | R-0069-b; QA-3 | `ack`/`disposition` unrouted |
//! | 17. Unattached session → `not_attached` | `ack_and_disposition_from_unattached_session_are_refused_not_attached` | R-0064-e | `ack`/`disposition` unrouted |
//! | 18. `read_observer` refused pre-dispatch | `ack_and_disposition_from_read_observer_token_are_refused_pre_dispatch` | R-0073-b | `ack`/`disposition` unrouted (parse fails before the per-action gate, mirroring test 7) |
//! | 19. Disposition-note log hygiene | `disposition_note_log_entry_carries_discrete_structured_field_not_spliced` | R-0075-e | `disposition` unrouted |
//! | 20. Disposition emits a `Disposition`-typed audit record (positive emission) | `disposition_emits_exactly_one_disposition_typed_audit_record` | R-0075-b; AC10 | `disposition` unrouted |
//!
//! Disposition audit-fail-closed (R-0075-c) is deliberately NOT a numbered
//! row — see the addendum above: discharged by `coordination_failclosed.rs`
//! (Task 3), not re-authored here. This is DISTINCT from row 20 above
//! (positive emission), which IS tested. FF-4's cross-slice
//! send-then-first-poll registration-duplication concern is likewise NOT a
//! numbered row — assessed and found real, but not authorable here without
//! crossing this dispatch's `poll_bind` attach/audit/succession scope fence
//! (see the addendum's FF-4 section).
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
//!
//! # Slice (b) addendum — lifecycle machine + poll `sent→delivered` + `ack` +
//! `disposition` (Glitch, dispatch 1634)
//!
//! Tests 9+ below continue this file's convention (black-box-adjacent,
//! non-vacuity-first, wire-driven). Two things changed since the header
//! above was written and must be read before trusting a stale mental model:
//!
//! **`send` is no longer red — it is real, merged code (Task 7a, commit
//! `5e5992a`, landed at this dispatch's parent commit `21ca89c`).**
//! [`crate::coordination::messages::send`] is a live wire action: it mints
//! actual `messages` rows. Tests 9+ therefore use `send` as ordinary FIXTURE
//! SETUP (via a `send_real_message` helper) rather than treating it as
//! unrouted — the RED-phase narrative in this file's ORIGINAL header (tests
//! 1–8, "at the parent commit `0fe62fc`") describes the state AT THE TIME
//! dispatch 1593 wrote it and is left as historical record; it does not
//! describe the state this dispatch runs against. Confirm for yourself
//! before assuming otherwise: `coordination/messages.rs` exists and
//! `session_plane::parse_action`'s `(MESSAGE_TOOL, "send")` arm resolves.
//!
//! **This slice's tests are RED for TWO DISTINCT causes, not one — do not
//! collapse them into tests 1–8's single "unsupported action" narrative:**
//!
//! 1. **`ack` and `disposition` are unrouted** — `parse_action` has no
//!    `(MESSAGE_TOOL, "ack")` or `(MESSAGE_TOOL, "disposition")` arm (confirmed
//!    directly: only `poll` and `send` are registered for `MESSAGE_TOOL`; the
//!    existing unit test `parse_action_rejects_unsupported_missing_and_non_string`
//!    already names `ack` as the unsupported case). Every scenario invoking
//!    `ack`/`disposition` collapses to the SAME uniform wire-level cause as
//!    tests 1–8: `ErrorData { code: INVALID_PARAMS, message: "unsupported
//!    coordination action 'ack'/'disposition' for tool 'message'" }`.
//! 2. **`poll`'s delivery call is NOT wired** — `poll` itself IS routed and
//!    green (Task 4); `session_plane::poll_response` is a real, executing
//!    function, but its `messages` field is a hardcoded EMPTY array (its own
//!    doc comment: "`messages` is EMPTY at Task 4 … the delivery half lands
//!    Task 7") and no code path updates a `messages` row's `state`/
//!    `delivered_at` on poll. So a `send` → addressee-`poll` scenario is RED
//!    NOT because an action is unsupported — the call SUCCEEDS — but because
//!    the guarantee-absent positive claim ("the response's `messages` array
//!    contains this message with `state: \"delivered\"`" / "the row's
//!    `delivered_at` is now set") is false today. Anchor every delivery
//!    assertion on that positive claim, never on a bare success/failure of
//!    the `poll` call itself (which already succeeds and always has).
//!
//! Every fixture below that needs a message in a state PAST `sent`
//! (`delivered`/`acknowledged`/`dispositioned`) cannot reach that state via
//! the wire today (poll's delivery gap above), so fixtures force it via a
//! direct-SQL `UPDATE messages SET …` — an extension of this file's own
//! "Sanctioned reads" DB-observation carve-out to a FIXTURE MUTATION of the
//! same table, precisely mirroring `coordination_leases.rs`'s own precedent
//! (`UPDATE leases SET expires_at = now() - interval '1 second' …` to force
//! an expired-lease fixture without a flake-prone real-time wait): "not a
//! shortcut around the [transition] predicate — [the code] reads [the state]
//! fresh from the row at operation time regardless of how it got there."
//! `ack`/`disposition` read `state`/timestamps at call time exactly the same
//! way, so forcing the precondition column(s) directly is sound.
//!
//! # Disposition audit (R-0075-b / R-0075-c / AC10) — TWO obligations, split
//! two ways (revised after a Puck amendment round, same dispatch)
//!
//! The plan carries two SEPARATE disposition-audit obligations for this
//! slice. They are NOT the same claim and are NOT discharged the same way —
//! read them as a pair, not one blended narrative:
//!
//! 1. **Fail-closed (R-0075-c)** — "with audit capture forced to fail,
//!    `disposition`'s state transition does not commit." DISCHARGED by Task
//!    3, NOT separately red-tested here (unchanged from this file's original
//!    stance; independently re-verified during the amendment round and held
//!    — see below).
//! 2. **Positive emission (R-0075-b disposition half / AC10 disposition-audit
//!    half)** — "every `disposition` emits an audit record to the D-SURFACE
//!    outbox" (plan L86, L154). TESTED HERE, per-action, positively — test
//!    20 below, structurally mirroring test 5's
//!    (`registration_audit_fires_iff_addressee_is_newly_minted_by_send`)
//!    per-action positive proof of R-0075-b's REGISTRATION half. Do not read
//!    the fail-closed discharge below as covering this too — it does not.
//!
//! ## Why #1 (fail-closed) stays discharged, not re-tested
//!
//! Two established, merged precedents in THIS SAME codebase disposition the
//! identical privileged-write class (Registration/Attachment/
//! AttachmentSuccession/LeaseTakeover — all, like `Disposition`, R-0075-b
//! subset members) the same way: `coordination_session_plane.rs` states
//! outright "Unlike `coordination_failclosed.rs`, this file needs no
//! fault-injection seam"; `coordination_leases.rs` carries no
//! `AuditEmitFail` test despite `LeaseTakeover` sharing the identical
//! R-0075-b/R-0075-c concern. The generic guarantee — ANY privileged write
//! staged through
//! [`crate::coordination::write_path::PgCoordinationStore::run_write`] rolls
//! back atomically when its staged audit fails to emit — is proven ONCE,
//! exhaustively, in `tests/coordination_failclosed.rs` (Task 3) (confirmed
//! directly: `CoordinationFault::AuditEmitFail` appears only there and in
//! `write_path.rs` itself, consulted generically at `write_path.rs:552`); it
//! is a chokepoint property of `run_write` itself, not a per-action property
//! to re-prove for every audited action.
//!
//! Concretely, under this file's hard constraints — `cargo test --no-run`
//! MUST exit 0 (no new test may reference a not-yet-existing symbol: no
//! `MnemraMcpServer` fault-injection seam exists, and adding one is
//! `mcp/server.rs`, this dispatch's `forbid_scope`) — there is no way to
//! author this as a genuinely-RED wire-driven acceptance test: (a) injecting
//! `CoordinationFault::AuditEmitFail` into a STANDALONE `PgCoordinationStore`
//! with a hand-rolled closure (mirroring `coordination_failclosed.rs`'s own
//! pattern) would compile, but would be GREEN ON ARRIVAL — `run_write`'s
//! fault-consultation is already-merged, already-passing machinery, so the
//! test could not fail today for the right reason (or any reason), which is
//! exactly the vacuous-test anti-pattern `skills/verify.md` warns against;
//! (b) reaching the REAL wire `disposition` action with the fault actually
//! injected into the live server's store has no existing seam and cannot be
//! added from this file alone.
//!
//! ## Why #2 (positive emission) DOES need its own test — the gap #1's
//! discharge does NOT cover
//!
//! `run_write`'s emit-guarantee only ensures a STAGED audit commits-or-
//! rolls-back atomically with the state transition — it guarantees NOTHING
//! about whether `disposition`'s body actually STAGES an `AuditRecord` in
//! the first place. If GREEN's `disposition` body stages no audit,
//! `run_write` has nothing to fail on: the transition commits happily and
//! every OTHER test in this suite still passes — the construction
//! obligation below would have no test net, and GREEN is `forbid_scope`-
//! blocked from this file and could not add one itself. Test 20 is that net.
//! Note the asymmetry with `send`'s registration audit, which is NOT covered
//! by `run_write` alone either — it needed, and got, test 5's per-action
//! positive proof; R-0075-b's registration half already has that net, its
//! disposition half wants the symmetric one.
//!
//! **Disposition to the GREEN implementer + Warden:** `messages.rs`'s
//! `disposition` body MUST route its state transition + staged
//! `AuditRecord { event_type: AuditEventType::Disposition, .. }` through
//! `run_write` (a construction obligation) — `run_write`'s existing,
//! already-proven emit-guarantee then covers the FAIL-CLOSED half for free,
//! exactly as it already covers `send`'s registration audit; test 20 below
//! covers the POSITIVE-EMISSION half, which `run_write` does not. Flagged as
//! a `deviations` entry in the completion report (the fail-closed omission
//! only — the positive-emission half is no longer an omission).
//!
//! ## FF-4's cross-slice half (send-mints, then first-poll-attaches) —
//! assessed, NOT added, and NOT silently dropped
//!
//! Plan FF-4 (L185) also names: "a send-minted-then-polled addressee yields
//! exactly one `Registration` (send) + one `Attachment` (poll), never two
//! registrations." Neither tests 9/10/11 here nor
//! `coordination_session_plane.rs`'s own suite cover this — Task 4's ONLY
//! "no new registration" test
//! (`succession_over_existing_actor_emits_no_new_registration_audit`) covers
//! SUCCESSION over an actor that was ITSELF minted by a PRIOR poll, a
//! different code fork; it predates `send`'s existence and could not have
//! covered the send-then-first-poll ordering. Reading
//! `session_plane.rs::attach_body`'s FRESH-ATTACH arm (the no-live-
//! attachment fork, `stage_audit(AuditRecord::registration(..))` immediately
//! followed by `stage_audit(AuditRecord::attachment(..))`) directly: it
//! stages BOTH unconditionally, with NO "did resolve-or-create actually
//! mint, or merely resolve" check — unlike `send_body`'s own
//! `addressee_existed` pre-check. So a send-minted-then-first-polled
//! addressee WOULD, today, receive a SECOND `registration` audit on its
//! first poll — a genuine, currently-true violation of FF-4's invariant, not
//! a hypothetical.
//!
//! This is NOT added as a new test here despite being real, because fixing
//! it requires an `addressee_existed`-shaped change to `attach_body`'s
//! audit-staging — and this dispatch's OWN brief states, verbatim: "the
//! attach/audit/succession logic inside [`poll_bind`] is forbidden to
//! change — GREEN adds only the delivery call. Do not write tests that would
//! force a change to attach/audit/succession behavior." Authoring this test
//! would do exactly that. This is a genuine conflict between FF-4's ask and
//! this dispatch's own scope fence, surfaced to Puck rather than resolved
//! unilaterally in either direction (silently adding a scope-violating test,
//! or silently dropping a confirmed defect).

#[path = "common/shared_engine.rs"]
mod shared_engine;

use std::sync::Arc;
use std::time::Duration;

use rmcp::model::{CallToolRequestParams, CallToolResult, Meta};
use rmcp::service::{RoleClient, RunningService, serve_client, serve_server};
use serde_json::json;
use tokio::io::duplex;
use tracing_test::traced_test;
use uuid::Uuid;

use mnemra_host::auth::token::{AdminToken, generate, hash};
use mnemra_host::coordination::CoordinationConfig;
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
// Slice (b) harness additions (Glitch, dispatch 1634)
// ===========================================================================

/// Stand up an `MnemraMcpServer` with a SECOND-scale `attachment_ttl` (the
/// sanctioned test-calibration config override — mirrors
/// `coordination_session_plane.rs`'s own `coordination_server_with_ttl`,
/// duplicated per this file's established per-file harness convention).
/// `write_timeout` stays at the default 10 s. Needed only by test 10's
/// dead-role-instance/succession scenario (AC1).
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
            Err(e) => eprintln!("coordination_messages (slice b) test server init failed: {e:?}"),
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");
    (handle, client)
}

/// Build a `message` `ack` tool call.
fn ack_params(token_str: &str, message_id: Uuid) -> CallToolRequestParams {
    let mut params = CallToolRequestParams::new("message");
    params.meta = Some(token_meta(token_str));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("action".to_owned(), json!("ack"));
        m.insert("message_id".to_owned(), json!(message_id.to_string()));
        m
    });
    params
}

/// Build a `message` `disposition` tool call. `note` is omitted from the
/// arguments map entirely when `None` (an absent optional field, not a JSON
/// `null`), matching the wire shape a real MCP client sends when the caller
/// doesn't supply one.
fn disposition_params(
    token_str: &str,
    message_id: Uuid,
    disposition: &str,
    note: Option<&str>,
) -> CallToolRequestParams {
    let mut params = CallToolRequestParams::new("message");
    params.meta = Some(token_meta(token_str));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("action".to_owned(), json!("disposition"));
        m.insert("message_id".to_owned(), json!(message_id.to_string()));
        m.insert("disposition".to_owned(), json!(disposition));
        if let Some(n) = note {
            m.insert("note".to_owned(), json!(n));
        }
        m
    });
    params
}

/// Perform a real, GREEN `send` (Task 7a) from `sender_token` (already
/// attached — precondition, panics otherwise) to `to_role_instance`, and
/// return the minted `message_id`. Fixture setup, not a scenario assertion —
/// slice (b)'s tests need real `messages` rows to exercise `ack`/
/// `disposition`/poll-delivery against, and `send` is real code at this
/// dispatch's parent commit (see the module header addendum).
async fn send_real_message(
    client: &RunningService<RoleClient, ()>,
    sender_token: &str,
    to_role_instance: &str,
    subject: &str,
) -> Uuid {
    let res = client
        .call_tool(send_params(
            sender_token,
            to_role_instance,
            HANDOFF_TYPE_NAME,
            HANDOFF_V1,
            conformant_handoff_payload(subject),
        ))
        .await;
    let obj = send_structured_obj(&res).unwrap_or_else(|| {
        panic!(
            "precondition: a conformant `send` (Task 7a, green at this dispatch's parent \
             commit) must succeed; got {res:?}"
        )
    });
    let message_id_str = obj
        .get("message_id")
        .and_then(|v| v.as_str())
        .expect("precondition: the `send` response must carry a string `message_id`");
    Uuid::parse_str(message_id_str)
        .unwrap_or_else(|e| panic!("precondition: `message_id` must be a valid UUID: {e}"))
}

/// The `messages` array of a `poll` response, as owned JSON objects — empty
/// if the call errored or carried no `messages` field/array. Used to check
/// whether a specific `message_id` appears in the polling actor's queue and,
/// if so, with what `state`/timestamps (R-0072-a).
fn poll_response_messages<E>(
    result: &Result<CallToolResult, E>,
) -> Vec<serde_json::Map<String, serde_json::Value>> {
    match result {
        Ok(r) => r
            .structured_content
            .as_ref()
            .and_then(|v| v.as_object())
            .and_then(|obj| obj.get("messages"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|m| m.as_object().cloned()).collect())
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Find the entry in a `poll` response's `messages` array whose `message_id`
/// matches `message_id`, if present.
fn find_polled_message(
    messages: &[serde_json::Map<String, serde_json::Value>],
    message_id: Uuid,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let needle = message_id.to_string();
    messages
        .iter()
        .find(|m| m.get("message_id").and_then(|v| v.as_str()) == Some(needle.as_str()))
        .cloned()
}

// ----- Fixture-state forcers (sanctioned direct-SQL mutation — extension of -----
// ----- this file's DB-observation carve-out; mirrors `coordination_leases.rs`'s -----
// ----- own `UPDATE leases SET expires_at = …` expired-lease fixture forcer -----

/// Force a just-`send`t message row (`state = 'sent'`) to `delivered` via
/// direct SQL — bypasses `poll`'s (not-yet-wired) delivery call so
/// `ack`/`disposition` fixtures needing a `delivered` precondition don't
/// depend on the delivery gap under test elsewhere in this file (tests 9/10).
async fn force_message_delivered(pool: &sqlx::PgPool, message_id: Uuid) {
    sqlx::query("UPDATE messages SET state = 'delivered', delivered_at = now() WHERE id = $1")
        .bind(message_id)
        .execute(pool)
        .await
        .expect("force-delivered fixture UPDATE must execute");
}

/// As [`force_message_delivered`], continuing to `acknowledged`.
async fn force_message_acknowledged(pool: &sqlx::PgPool, message_id: Uuid) {
    force_message_delivered(pool, message_id).await;
    sqlx::query(
        "UPDATE messages SET state = 'acknowledged', acknowledged_at = now() WHERE id = $1",
    )
    .bind(message_id)
    .execute(pool)
    .await
    .expect("force-acknowledged fixture UPDATE must execute");
}

/// As [`force_message_acknowledged`], continuing to `dispositioned` with
/// `disposition` (must be one of the closed vocabulary members — the schema
/// CHECK constraint enforces this even for a fixture write) and an optional
/// `note`.
async fn force_message_dispositioned(
    pool: &sqlx::PgPool,
    message_id: Uuid,
    disposition: &str,
    note: Option<&str>,
) {
    force_message_acknowledged(pool, message_id).await;
    sqlx::query(
        "UPDATE messages
         SET state = 'dispositioned', dispositioned_at = now(),
             disposition = $2, disposition_note = $3
         WHERE id = $1",
    )
    .bind(message_id)
    .bind(disposition)
    .bind(note)
    .execute(pool)
    .await
    .expect("force-dispositioned fixture UPDATE must execute");
}

/// Set up a message addressed to a FRESH addressee role-instance, sent from a
/// FRESH sender, forced to `target_state` (`"sent"` leaves the real
/// post-`send` state untouched). Returns `(message_id, addressee_token,
/// addressee_role)` — the addressee token is already attached, so a test can
/// immediately call `ack`/`disposition` as that addressee.
async fn seeded_message_in_state(
    client: &RunningService<RoleClient, ()>,
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    target_state: &str,
) -> (Uuid, AdminToken, String) {
    let sender_token = seed_admin_token(pool, workspace_id).await;
    let sender_role = format!("seed-sender-{}", Uuid::new_v4());
    attach_session(client, sender_token.as_str(), &sender_role).await;

    let addressee_token = seed_admin_token(pool, workspace_id).await;
    let addressee_role = format!("seed-addressee-{}", Uuid::new_v4());
    attach_session(client, addressee_token.as_str(), &addressee_role).await;

    let message_id = send_real_message(
        client,
        sender_token.as_str(),
        &addressee_role,
        "matrix fixture",
    )
    .await;

    match target_state {
        "sent" => {}
        "delivered" => force_message_delivered(pool, message_id).await,
        "acknowledged" => force_message_acknowledged(pool, message_id).await,
        "dispositioned" => force_message_dispositioned(pool, message_id, "completed", None).await,
        other => panic!("test bug: seeded_message_in_state got unknown target_state {other:?}"),
    }

    (message_id, addressee_token, addressee_role)
}

/// The full lifecycle projection of a `messages` row, if it exists:
/// `(state, delivered_at_epoch, acknowledged_at_epoch, dispositioned_at_epoch,
/// disposition, disposition_note)`. Timestamps are read as
/// `EXTRACT(EPOCH FROM …)::float8` (nullable) so the test crate needs no
/// `chrono` dependency and equality checks across re-reads are plain `f64`
/// comparisons — mirrors `coordination_leases.rs`'s own epoch-float
/// convention.
///
/// `#[allow(clippy::type_complexity)]`: a 6-tuple observer return, same
/// precedent as this file's own `message_row_by_id` (8-tuple) and
/// `coordination_session_plane.rs::succession_audit_evidence` (6-tuple) — a
/// named struct would be pure ceremony for a test-local, single-call-site
/// read.
#[allow(clippy::type_complexity)]
async fn message_lifecycle_row(
    pool: &sqlx::PgPool,
    message_id: Uuid,
) -> Option<(
    String,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<String>,
    Option<String>,
)> {
    let row = sqlx::query_as(
        "SELECT state,
                EXTRACT(EPOCH FROM delivered_at)::float8,
                EXTRACT(EPOCH FROM acknowledged_at)::float8,
                EXTRACT(EPOCH FROM dispositioned_at)::float8,
                disposition,
                disposition_note
         FROM messages WHERE id = $1",
    )
    .bind(message_id)
    .fetch_optional(pool)
    .await
    .expect("message lifecycle read-back query must execute");
    row
}

/// The `(actor_id, payload)` of the `disposition`-typed `coordination_audit`
/// row naming `message_id`, if any — the R-0075-b/AC10 disposition-audit-
/// EMISSION surface (distinct from R-0075-c's fail-closed half, discharged
/// by `coordination_failclosed.rs`; see the module header addendum). Queries
/// by `payload->>'message_id'` — this test file's PINNED payload key name
/// for `AuditRecord::disposition(...)` (no plan/spec text fixes one; the
/// natural analogue of `registration`'s `role_instance` key and
/// `lease_takeover`'s `prior_holder`/`new_holder` keys). `actor_id` is
/// nullable at the schema level (`coordination_audit.actor_id UUID`, no NOT
/// NULL), hence `Option<Uuid>` here — test 20 asserts it is `Some` and
/// matches the addressee.
async fn disposition_audit_row_for_message(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    message_id: Uuid,
) -> Option<(Option<Uuid>, serde_json::Value)> {
    let row: Option<(Option<Uuid>, serde_json::Value)> = sqlx::query_as(
        "SELECT actor_id, payload FROM coordination_audit
         WHERE workspace_id = $1 AND event_type = 'disposition'
           AND payload->>'message_id' = $2",
    )
    .bind(workspace_id)
    .bind(message_id.to_string())
    .fetch_optional(pool)
    .await
    .expect("disposition-audit read-back query must execute");
    row
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

// ===========================================================================
// Test 9 — first poll delivers the message; delivered_at is append-once
// (R-0069-a; R-0068-c) — Slice (b), Glitch dispatch 1634
// ===========================================================================

/// GIVEN a `handoff` message `send`t to a role-instance that has NEVER
/// polled,
/// WHEN the addressee's FIRST `poll` (the bind call, R-0064-e) runs,
/// THEN the message appears in the poll response's `messages` array with
/// `state: "delivered"` (R-0069-a) AND the `messages` row's `delivered_at`
/// is now set (read back via operator SQL); a SECOND poll by the same
/// (already-attached) session does not rewrite `delivered_at` — the
/// append-once consumption record (R-0068-c). *(R-0069-a; R-0068-c)*
///
/// # Non-vacuity discipline
///
/// The precondition check (never-polled reads `sent`/`delivered_at` NULL) is
/// ALREADY true today and is asserted here as a SANITY precondition, not the
/// RED anchor — this file's slice (b) addendum flags exactly this hazard.
/// The PRIMARY, guarantee-absent anchor is the POSITIVE claim that the
/// message appears in the poll response with `state: "delivered"` — false
/// today (`poll_response`'s `messages` field is a hardcoded empty array; see
/// the module header addendum). The append-once secondary check is layered
/// on top of the (today-failing) primary anchor, never standing alone.
///
/// RED against this dispatch's parent commit: `poll`'s delivery call is not
/// wired — the response's `messages` array is always empty, so
/// `find_polled_message` returns `None` and the primary assertion fails
/// guarantee-absent (this is NOT the "unsupported action" cause tests 1–8
/// use — `poll` itself succeeds).
#[tokio::test]
async fn addressees_first_poll_delivers_the_message_and_sets_delivered_at_once() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let sender_token = seed_admin_token(&pool, workspace_id).await;
    let sender_role = format!("t9-sender-{}", Uuid::new_v4());
    let addressee_role = format!("t9-addressee-{}", Uuid::new_v4());
    let addressee_token = seed_admin_token(&pool, workspace_id).await;

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, sender_token.as_str(), &sender_role).await;

    let message_id = send_real_message(
        &client,
        sender_token.as_str(),
        &addressee_role,
        "t9 delivery",
    )
    .await;

    // Sanity precondition (already true today — NOT the RED anchor): a
    // never-polled message reads `sent`/`delivered_at` NULL.
    let precondition = message_lifecycle_row(&pool, message_id)
        .await
        .expect("precondition: the sent message row must exist");
    assert_eq!(
        precondition.0, "sent",
        "precondition: a freshly-sent message reads `sent`."
    );
    assert!(
        precondition.1.is_none(),
        "precondition: a never-polled message's `delivered_at` must be NULL."
    );

    // The addressee's FIRST poll — the bind call itself.
    let first_poll = client
        .call_tool(poll_params(addressee_token.as_str(), &addressee_role))
        .await;
    let messages = poll_response_messages(&first_poll);
    let polled = find_polled_message(&messages, message_id);
    assert!(
        polled.is_some(),
        "R-0069-a: the addressee's first `poll` must return this message; `poll`'s delivery \
         call is not wired at this dispatch's parent commit, so the `messages` array is empty. \
         Got messages: {messages:?}"
    );
    assert_eq!(
        polled.unwrap().get("state").and_then(|v| v.as_str()),
        Some("delivered"),
        "R-0069-a: the polled message must carry `state: \"delivered\"`."
    );

    let post = message_lifecycle_row(&pool, message_id)
        .await
        .expect("the message row must still exist after poll");
    assert_eq!(
        post.0, "delivered",
        "R-0069-a: the row's `state` must be `delivered` after the addressee's poll."
    );
    let delivered_epoch = post.1.unwrap_or_else(|| {
        panic!("R-0069-a: `delivered_at` must be set after the addressee's poll delivers it")
    });

    // A SECOND poll (mid-session check, same role_instance) must not
    // rewrite `delivered_at` — append-once (R-0068-c).
    let _second_poll = client
        .call_tool(poll_params(addressee_token.as_str(), &addressee_role))
        .await;
    let repolled = message_lifecycle_row(&pool, message_id)
        .await
        .expect("the message row must still exist after the second poll");
    assert_eq!(
        repolled.1,
        Some(delivered_epoch),
        "R-0068-c: a repeat poll must NOT rewrite `delivered_at` — append-once."
    );

    server.abort();
}

// ===========================================================================
// Test 10 — dead role-instance message delivered to successor's bind poll
// (R-0068-a, AC1)
// ===========================================================================

/// GIVEN a message `send`t to role-instance X while S1 holds X's live
/// attachment, S1 then goes stale (attachment TTL elapses, no release),
/// WHEN a successor session S2 binds via `poll(role_instance = X)` — the
/// SAME single call that both audited-succeeds the stale attachment (Task 4,
/// green) AND is the canonical delivery moment (R-0072-a),
/// THEN S2 resolves the SAME `actor_id` S1 held (succession, unchanged by
/// this slice) AND S2's poll response carries the message with `state:
/// "delivered"` — the queue survives session death exactly as leases do.
/// *(R-0068-a; AC1, completed here per the T7-slice-decomposition plan)*
///
/// RED against this dispatch's parent commit: succession itself is green
/// (Task 4) — S2 DOES resolve the same `actor_id` — but delivery is not
/// wired, so the message is absent from S2's poll response regardless of the
/// (working) succession underneath it.
#[tokio::test]
async fn message_to_a_role_instance_whose_session_died_is_delivered_to_the_successor_bind_poll() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let dead_role = format!("t10-dead-role-{}", Uuid::new_v4());

    let (server, client) = coordination_server_with_ttl(&pool, Duration::from_secs(1)).await;

    let s1_token = seed_admin_token(&pool, workspace_id).await;
    let s1_actor_id = attach_session(&client, s1_token.as_str(), &dead_role).await;

    let sender_token = seed_admin_token(&pool, workspace_id).await;
    let sender_role = format!("t10-sender-{}", Uuid::new_v4());
    attach_session(&client, sender_token.as_str(), &sender_role).await;

    let message_id = send_real_message(
        &client,
        sender_token.as_str(),
        &dead_role,
        "t10 dead-role handoff",
    )
    .await;

    // S1 never renews; wait past the 1 s attachment TTL (sanctioned
    // test-calibration real sleep, mirrors `coordination_session_plane.rs`'s
    // own succession tests — never a wall-clock ten-minute wait).
    tokio::time::sleep(Duration::from_millis(2500)).await;

    let s2_token = seed_admin_token(&pool, workspace_id).await;
    let s2_poll = client
        .call_tool(poll_params(s2_token.as_str(), &dead_role))
        .await;
    let s2_obj = s2_poll
        .as_ref()
        .ok()
        .and_then(|r| r.structured_content.as_ref())
        .and_then(|v| v.as_object())
        .unwrap_or_else(|| panic!("S2's succession bind poll must succeed; got {s2_poll:?}"));
    let s2_actor_id_str = s2_obj
        .get("actor")
        .and_then(|v| v.as_object())
        .and_then(|a| a.get("actor_id"))
        .and_then(|v| v.as_str())
        .expect("S2's poll response must carry `actor.actor_id`");
    assert_eq!(
        Uuid::parse_str(s2_actor_id_str).expect("`actor_id` must be a valid UUID"),
        s1_actor_id,
        "sanity (Task 4, green, unchanged by this slice): the successor must resolve the SAME \
         actor_id S1 held."
    );

    let messages = poll_response_messages(&s2_poll);
    let polled = find_polled_message(&messages, message_id);
    assert!(
        polled.is_some(),
        "R-0068-a (AC1): the message addressed to the dead role-instance must be returned on \
         the successor's bind poll — the queue survives session death exactly as leases do. \
         `poll`'s delivery call is not wired at this dispatch's parent commit, so the \
         `messages` array is empty regardless of succession. Got messages: {messages:?}"
    );
    assert_eq!(
        polled.unwrap().get("state").and_then(|v| v.as_str()),
        Some("delivered"),
        "R-0069-a: the delivered message must carry `state: \"delivered\"`."
    );

    server.abort();
}

// ===========================================================================
// Test 11 — QA-3 full consumption trail: send → poll (delivered) → ack
// (acknowledged) → disposition completed (dispositioned)
// ===========================================================================

/// GIVEN a `handoff` message `send`t from actor A to actor B,
/// WHEN B polls (delivered), acks (acknowledged), and dispositions it
/// `completed` (dispositioned),
/// THEN the row carries all three consumption timestamps
/// (`delivered_at`/`acknowledged_at`/`dispositioned_at`) and the disposition
/// member `completed`; a final read-back confirms the immutable fields
/// (sender, addressee, type, schema_version) are unchanged from the
/// original `send`. *(R-0068-c, R-0069-a, R-0069-c; QA-3)*
///
/// # RED cause — NOT one uniform collapse (see module header addendum)
///
/// This test's FIRST failing assertion is the poll-delivery step (`poll`'s
/// delivery call is not wired) — it panics there, so the `ack`/`disposition`
/// calls below are UNREACHED this run. They are written anyway (the intended
/// full trail this test pins) — as GREEN fills in delivery, then `ack`, then
/// `disposition`, this same test progresses further each run without being
/// rewritten, exactly how a trail/integration test is meant to evolve
/// red→green. Do NOT read this test's later assertions as today's RED
/// cause — the module header names the two distinct RED-cause families and
/// this test only ever exercises the FIRST (delivery-not-wired) today.
#[tokio::test]
async fn qa3_full_consumption_trail_send_poll_ack_disposition() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let sender_token = seed_admin_token(&pool, workspace_id).await;
    let sender_role = format!("t11-sender-{}", Uuid::new_v4());
    let addressee_token = seed_admin_token(&pool, workspace_id).await;
    let addressee_role = format!("t11-addressee-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    let sender_actor_id = attach_session(&client, sender_token.as_str(), &sender_role).await;

    let message_id =
        send_real_message(&client, sender_token.as_str(), &addressee_role, "qa3 trail").await;
    let addressee_actor_id = actor_id_by_name(&pool, workspace_id, &addressee_role)
        .await
        .expect("the addressee actor row must exist after `send` (resolve-or-create, Task 7a)");

    // Step 1: addressee's poll must deliver.
    let poll_res = client
        .call_tool(poll_params(addressee_token.as_str(), &addressee_role))
        .await;
    let messages = poll_response_messages(&poll_res);
    let polled = find_polled_message(&messages, message_id);
    assert!(
        polled.is_some(),
        "QA-3 step 1 (poll → delivered): the message must appear in the addressee's poll \
         response; `poll`'s delivery call is not wired at this dispatch's parent commit. Got \
         messages: {messages:?}"
    );
    assert_eq!(
        polled.unwrap().get("state").and_then(|v| v.as_str()),
        Some("delivered"),
        "QA-3 step 1: the polled message must carry `state: \"delivered\"`."
    );

    // Step 2: addressee's ack must acknowledge.
    let ack_res = client
        .call_tool(ack_params(addressee_token.as_str(), message_id))
        .await;
    let ack_obj = send_structured_obj(&ack_res);
    assert_eq!(
        ack_obj
            .as_ref()
            .and_then(|o| o.get("state"))
            .and_then(|v| v.as_str()),
        Some("acknowledged"),
        "QA-3 step 2 (ack → acknowledged): the ack response must carry `state: \
         \"acknowledged\"`. `ack` is unrouted at this dispatch's parent commit, so this \
         collapses to the uniform `INVALID_PARAMS` cause. Got: {ack_res:?}"
    );

    // Step 3: addressee's disposition (completed) must disposition.
    let disp_res = client
        .call_tool(disposition_params(
            addressee_token.as_str(),
            message_id,
            "completed",
            None,
        ))
        .await;
    let disp_obj = send_structured_obj(&disp_res);
    assert_eq!(
        disp_obj
            .as_ref()
            .and_then(|o| o.get("state"))
            .and_then(|v| v.as_str()),
        Some("dispositioned"),
        "QA-3 step 3 (disposition completed → dispositioned): the disposition response must \
         carry `state: \"dispositioned\"`. `disposition` is unrouted at this dispatch's parent \
         commit. Got: {disp_res:?}"
    );
    assert_eq!(
        disp_obj
            .as_ref()
            .and_then(|o| o.get("disposition"))
            .and_then(|v| v.as_str()),
        Some("completed"),
        "QA-3 step 3: the disposition response must carry `disposition: \"completed\"`."
    );

    // Final: all three timestamps + disposition member present; immutable
    // fields unchanged from the original send.
    let final_row = message_lifecycle_row(&pool, message_id)
        .await
        .expect("the message row must still exist after the full trail");
    assert_eq!(
        final_row.0, "dispositioned",
        "QA-3: final state must be `dispositioned`."
    );
    assert!(final_row.1.is_some(), "QA-3: `delivered_at` must be set.");
    assert!(
        final_row.2.is_some(),
        "QA-3: `acknowledged_at` must be set."
    );
    assert!(
        final_row.3.is_some(),
        "QA-3: `dispositioned_at` must be set."
    );
    assert_eq!(
        final_row.4.as_deref(),
        Some("completed"),
        "QA-3: the stored disposition member must be `completed`."
    );

    let immutable_row = message_row_by_id(&pool, message_id)
        .await
        .expect("the message row must exist for the immutability read-back");
    let (_, row_sender, row_addressee, row_type, row_schema_version, _, sent_at_is_set, _) =
        immutable_row;
    assert!(
        sent_at_is_set,
        "QA-3: `sent_at` must remain set post-trail."
    );
    assert_eq!(
        row_sender, sender_actor_id,
        "QA-3: `sender_actor_id` must be unchanged post-trail."
    );
    assert_eq!(
        row_addressee, addressee_actor_id,
        "QA-3: `addressee_actor_id` must be unchanged post-trail."
    );
    assert_eq!(
        row_type, HANDOFF_TYPE_NAME,
        "QA-3: `message_type` must be unchanged post-trail."
    );
    assert_eq!(
        row_schema_version, HANDOFF_V1 as i32,
        "QA-3: `schema_version` must be unchanged post-trail."
    );

    server.abort();
}

// ===========================================================================
// Test 12 — illegal transition matrix + `message_not_found` (R-0069-a AC4;
// R-0068-c AC3 append-once half)
// ===========================================================================

/// GIVEN messages fixture-forced into every state from which `ack` or
/// `disposition` is ILLEGAL, plus fabricated `message_id`s naming no row at
/// all,
/// WHEN each illegal call is attempted,
/// THEN every cell refuses `invalid_transition` (the six illegal matrix
/// cells: `ack` on `sent`/`acknowledged`/`dispositioned`, `disposition` on
/// `sent`/`delivered`/`dispositioned` — the second of each pair is the AC3
/// append-once "already-consumed" case) and a fabricated `message_id`
/// refuses `message_not_found` for both actions. The append-once cases
/// additionally assert the pre-forced consumption timestamp (and, for
/// disposition, the stored member) is UNCHANGED after the refused call.
/// *(R-0069-a AC4; R-0068-c AC3)*
///
/// # Non-vacuity discipline
///
/// Every sub-case anchors FIRST on the reason code being PRESENT
/// (guarantee-absent today: `ack`/`disposition` are unrouted, so every case
/// gets `INVALID_PARAMS` instead). The append-once timestamp/member-unchanged
/// checks are SECONDARY, layered on top — never the sole anchor.
///
/// RED against this dispatch's parent commit: `ack`/`disposition` are
/// unrouted — every sub-case hits the SAME `INVALID_PARAMS` catch-all; none
/// of `invalid_transition`/`message_not_found` is ever present.
#[tokio::test]
async fn illegal_transition_matrix_and_message_not_found_are_refused() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    // ---- ack on sent (never delivered) ----
    {
        let workspace_id = Uuid::new_v4();
        let (server, client) = coordination_server(&pool).await;
        let (message_id, addressee_token, _addressee_role) =
            seeded_message_in_state(&client, &pool, workspace_id, "sent").await;

        let res = client
            .call_tool(ack_params(addressee_token.as_str(), message_id))
            .await;
        assert!(
            result_surfaces_code(&res, "invalid_transition"),
            "matrix (ack on sent): must refuse `invalid_transition`. Got: {res:?}"
        );
        server.abort();
    }

    // ---- ack on acknowledged (SECOND ack — AC3 append-once) ----
    {
        let workspace_id = Uuid::new_v4();
        let (server, client) = coordination_server(&pool).await;
        let (message_id, addressee_token, _addressee_role) =
            seeded_message_in_state(&client, &pool, workspace_id, "acknowledged").await;
        let before = message_lifecycle_row(&pool, message_id)
            .await
            .expect("fixture row must exist");

        let res = client
            .call_tool(ack_params(addressee_token.as_str(), message_id))
            .await;
        assert!(
            result_surfaces_code(&res, "invalid_transition"),
            "matrix (ack on already-acknowledged — AC3 second ack): must refuse \
             `invalid_transition`. Got: {res:?}"
        );
        let after = message_lifecycle_row(&pool, message_id)
            .await
            .expect("fixture row must still exist");
        assert_eq!(
            after.2, before.2,
            "R-0068-c: a refused second `ack` must NOT rewrite `acknowledged_at`."
        );
        server.abort();
    }

    // ---- ack on dispositioned ----
    {
        let workspace_id = Uuid::new_v4();
        let (server, client) = coordination_server(&pool).await;
        let (message_id, addressee_token, _addressee_role) =
            seeded_message_in_state(&client, &pool, workspace_id, "dispositioned").await;

        let res = client
            .call_tool(ack_params(addressee_token.as_str(), message_id))
            .await;
        assert!(
            result_surfaces_code(&res, "invalid_transition"),
            "matrix (ack on dispositioned): must refuse `invalid_transition`. Got: {res:?}"
        );
        server.abort();
    }

    // ---- disposition on sent (never delivered) ----
    {
        let workspace_id = Uuid::new_v4();
        let (server, client) = coordination_server(&pool).await;
        let (message_id, addressee_token, _addressee_role) =
            seeded_message_in_state(&client, &pool, workspace_id, "sent").await;

        let res = client
            .call_tool(disposition_params(
                addressee_token.as_str(),
                message_id,
                "completed",
                None,
            ))
            .await;
        assert!(
            result_surfaces_code(&res, "invalid_transition"),
            "matrix (disposition on sent): must refuse `invalid_transition`. Got: {res:?}"
        );
        server.abort();
    }

    // ---- disposition on delivered (un-acked) ----
    {
        let workspace_id = Uuid::new_v4();
        let (server, client) = coordination_server(&pool).await;
        let (message_id, addressee_token, _addressee_role) =
            seeded_message_in_state(&client, &pool, workspace_id, "delivered").await;

        let res = client
            .call_tool(disposition_params(
                addressee_token.as_str(),
                message_id,
                "completed",
                None,
            ))
            .await;
        assert!(
            result_surfaces_code(&res, "invalid_transition"),
            "matrix (disposition on delivered, un-acked): must refuse `invalid_transition`. \
             Got: {res:?}"
        );
        server.abort();
    }

    // ---- disposition on dispositioned (SECOND disposition — AC3 append-once) ----
    {
        let workspace_id = Uuid::new_v4();
        let (server, client) = coordination_server(&pool).await;
        let (message_id, addressee_token, _addressee_role) =
            seeded_message_in_state(&client, &pool, workspace_id, "dispositioned").await;
        let before = message_lifecycle_row(&pool, message_id)
            .await
            .expect("fixture row must exist");

        let res = client
            .call_tool(disposition_params(
                addressee_token.as_str(),
                message_id,
                "declined",
                None,
            ))
            .await;
        assert!(
            result_surfaces_code(&res, "invalid_transition"),
            "matrix (disposition on already-dispositioned — AC3 second disposition): must \
             refuse `invalid_transition`, never overwrite the existing disposition. Got: \
             {res:?}"
        );
        let after = message_lifecycle_row(&pool, message_id)
            .await
            .expect("fixture row must still exist");
        assert_eq!(
            after.3, before.3,
            "R-0068-c: a refused second `disposition` must NOT rewrite `dispositioned_at`."
        );
        assert_eq!(
            after.4, before.4,
            "R-0068-c: a refused second `disposition` must NOT overwrite the stored \
             `disposition` member."
        );
        server.abort();
    }

    // ---- ack on a fabricated message_id → message_not_found ----
    {
        let workspace_id = Uuid::new_v4();
        let (server, client) = coordination_server(&pool).await;
        let token = seed_admin_token(&pool, workspace_id).await;
        let role = format!("t12-notfound-ack-{}", Uuid::new_v4());
        attach_session(&client, token.as_str(), &role).await;

        let res = client
            .call_tool(ack_params(token.as_str(), Uuid::new_v4()))
            .await;
        assert!(
            result_surfaces_code(&res, "message_not_found"),
            "matrix (ack, fabricated message_id): must refuse `message_not_found`. Got: {res:?}"
        );
        server.abort();
    }

    // ---- disposition on a fabricated message_id → message_not_found ----
    {
        let workspace_id = Uuid::new_v4();
        let (server, client) = coordination_server(&pool).await;
        let token = seed_admin_token(&pool, workspace_id).await;
        let role = format!("t12-notfound-disp-{}", Uuid::new_v4());
        attach_session(&client, token.as_str(), &role).await;

        let res = client
            .call_tool(disposition_params(
                token.as_str(),
                Uuid::new_v4(),
                "completed",
                None,
            ))
            .await;
        assert!(
            result_surfaces_code(&res, "message_not_found"),
            "matrix (disposition, fabricated message_id): must refuse `message_not_found`. Got: \
             {res:?}"
        );
        server.abort();
    }
}

// ===========================================================================
// Test 13 — each disposition vocabulary member dispositions a fixture
// (R-0069-c)
// ===========================================================================

/// GIVEN three messages, each forced to `acknowledged`,
/// WHEN each is `disposition`ed with a DIFFERENT closed-vocabulary member
/// (`completed`, `declined`, `obsolete`),
/// THEN each call returns `{ state: "dispositioned", disposition: <member>,
/// dispositioned_at }` and the row read-back confirms the same. *(R-0069-c)*
///
/// RED against this dispatch's parent commit: `disposition` is unrouted —
/// every member hits the same `INVALID_PARAMS` catch-all; none of the three
/// successful outcomes is present.
#[tokio::test]
async fn each_disposition_vocabulary_member_dispositions_an_acknowledged_fixture() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    for member in ["completed", "declined", "obsolete"] {
        let workspace_id = Uuid::new_v4();
        let (server, client) = coordination_server(&pool).await;
        let (message_id, addressee_token, _addressee_role) =
            seeded_message_in_state(&client, &pool, workspace_id, "acknowledged").await;

        let res = client
            .call_tool(disposition_params(
                addressee_token.as_str(),
                message_id,
                member,
                None,
            ))
            .await;

        let obj = send_structured_obj(&res);
        assert_eq!(
            obj.as_ref()
                .and_then(|o| o.get("state"))
                .and_then(|v| v.as_str()),
            Some("dispositioned"),
            "R-0069-c: dispositioning `{member}` must return `state: \"dispositioned\"`; \
             `disposition` is unrouted at this dispatch's parent commit. Got: {res:?}"
        );
        assert_eq!(
            obj.as_ref()
                .and_then(|o| o.get("disposition"))
                .and_then(|v| v.as_str()),
            Some(member),
            "R-0069-c: the response must echo `disposition: \"{member}\"`."
        );

        let row = message_lifecycle_row(&pool, message_id)
            .await
            .expect("the message row must exist");
        assert_eq!(
            row.0, "dispositioned",
            "R-0069-c: the row's state must be `dispositioned`."
        );
        assert_eq!(
            row.4.as_deref(),
            Some(member),
            "R-0069-c: the row's stored `disposition` must be `{member}`."
        );

        server.abort();
    }
}

// ===========================================================================
// Test 14 — out-of-set disposition value → `invalid_disposition` (R-0069-c)
// ===========================================================================

/// GIVEN a message forced to `acknowledged`,
/// WHEN it is `disposition`ed with a value OUTSIDE the closed vocabulary
/// (`{completed, declined, obsolete}`),
/// THEN the call is refused `invalid_disposition` and the row remains
/// `acknowledged` (not dispositioned). *(R-0069-c)*
///
/// RED against this dispatch's parent commit: `disposition` is unrouted —
/// the call hits `INVALID_PARAMS`; `invalid_disposition` is absent.
#[tokio::test]
async fn disposition_with_out_of_set_value_is_refused_invalid_disposition() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let (server, client) = coordination_server(&pool).await;
    let (message_id, addressee_token, _addressee_role) =
        seeded_message_in_state(&client, &pool, workspace_id, "acknowledged").await;

    let res = client
        .call_tool(disposition_params(
            addressee_token.as_str(),
            message_id,
            "wontfix",
            None,
        ))
        .await;
    assert!(
        result_surfaces_code(&res, "invalid_disposition"),
        "R-0069-c: an out-of-set disposition value must be refused `invalid_disposition`. Got: \
         {res:?}"
    );

    let row = message_lifecycle_row(&pool, message_id)
        .await
        .expect("the message row must still exist");
    assert_eq!(
        row.0, "acknowledged",
        "R-0069-c: an `invalid_disposition` refusal must leave the row `acknowledged`, not \
         dispositioned."
    );

    server.abort();
}

// ===========================================================================
// Test 15 — disposition `note` round-trips to the stored row (R-0069-c)
// ===========================================================================

/// GIVEN a message forced to `acknowledged`,
/// WHEN it is `disposition`ed `completed` carrying an optional structured
/// `note`,
/// THEN the row's `disposition_note` column stores the note text verbatim.
/// *(R-0069-c)*
///
/// RED against this dispatch's parent commit: `disposition` is unrouted —
/// the call never executes, so `disposition_note` is never written.
#[tokio::test]
async fn disposition_note_round_trips_to_the_stored_row() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let (server, client) = coordination_server(&pool).await;
    let (message_id, addressee_token, _addressee_role) =
        seeded_message_in_state(&client, &pool, workspace_id, "acknowledged").await;

    let note = "handed off to the merger lane per the 2026-07-05 role decision";
    let res = client
        .call_tool(disposition_params(
            addressee_token.as_str(),
            message_id,
            "completed",
            Some(note),
        ))
        .await;
    let obj = send_structured_obj(&res);
    assert_eq!(
        obj.as_ref()
            .and_then(|o| o.get("state"))
            .and_then(|v| v.as_str()),
        Some("dispositioned"),
        "R-0069-c: a conformant `disposition` with a `note` must succeed and return `state: \
         \"dispositioned\"`; `disposition` is unrouted at this dispatch's parent commit. Got: \
         {res:?}"
    );

    let row = message_lifecycle_row(&pool, message_id)
        .await
        .expect("the message row must exist");
    assert_eq!(
        row.5.as_deref(),
        Some(note),
        "R-0069-c: the stored `disposition_note` must round-trip the caller's note verbatim."
    );

    server.abort();
}

// ===========================================================================
// Test 16 — non-addressee `ack`/`disposition` → `not_addressee`, logged
// (R-0069-b; QA-3)
// ===========================================================================

/// GIVEN a message addressed to B, and a THIRD attached actor C (not the
/// addressee),
/// WHEN C attempts `ack` (on a `delivered` fixture) and `disposition` (on an
/// `acknowledged` fixture),
/// THEN both are refused `not_addressee` and the refusal is logged (QA-3's
/// op-log assertion) — B's own consumption state is untouched by C's
/// refused attempts. *(R-0069-b; QA-3)*
///
/// RED against this dispatch's parent commit: `ack`/`disposition` are
/// unrouted — both hit `INVALID_PARAMS`; `not_addressee` is absent and no
/// coordination log line carries it (capture-liveness canary proves the
/// channel is live before trusting the absence).
#[traced_test]
#[tokio::test]
async fn ack_and_disposition_from_non_addressee_are_refused_not_addressee_and_logged() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    tracing::info!("coordination_messages t16 oplog canary");
    assert!(
        logs_contain("coordination_messages t16 oplog canary"),
        "the traced_test capture channel must be live before the log-hygiene assertion is \
         trusted"
    );

    // ---- ack case: message forced to `delivered`; C (not the addressee) acks it ----
    {
        let workspace_id = Uuid::new_v4();
        let (server, client) = coordination_server(&pool).await;
        let (message_id, _addressee_token, _addressee_role) =
            seeded_message_in_state(&client, &pool, workspace_id, "delivered").await;

        let c_token = seed_admin_token(&pool, workspace_id).await;
        let c_role = format!("t16-c-ack-{}", Uuid::new_v4());
        attach_session(&client, c_token.as_str(), &c_role).await;

        let res = client
            .call_tool(ack_params(c_token.as_str(), message_id))
            .await;
        assert!(
            result_surfaces_code(&res, "not_addressee"),
            "R-0069-b: a non-addressee's `ack` must be refused `not_addressee`. Got: {res:?}"
        );

        let row = message_lifecycle_row(&pool, message_id)
            .await
            .expect("the message row must still exist");
        assert_eq!(
            row.0, "delivered",
            "a non-addressee's refused `ack` must not transition B's message."
        );
        server.abort();
    }

    // ---- disposition case: message forced to `acknowledged`; C dispositions it ----
    {
        let workspace_id = Uuid::new_v4();
        let (server, client) = coordination_server(&pool).await;
        let (message_id, _addressee_token, _addressee_role) =
            seeded_message_in_state(&client, &pool, workspace_id, "acknowledged").await;

        let c_token = seed_admin_token(&pool, workspace_id).await;
        let c_role = format!("t16-c-disp-{}", Uuid::new_v4());
        attach_session(&client, c_token.as_str(), &c_role).await;

        let res = client
            .call_tool(disposition_params(
                c_token.as_str(),
                message_id,
                "completed",
                None,
            ))
            .await;
        assert!(
            result_surfaces_code(&res, "not_addressee"),
            "R-0069-b: a non-addressee's `disposition` must be refused `not_addressee`. Got: \
             {res:?}"
        );

        let row = message_lifecycle_row(&pool, message_id)
            .await
            .expect("the message row must still exist");
        assert_eq!(
            row.0, "acknowledged",
            "a non-addressee's refused `disposition` must not transition B's message."
        );
        server.abort();
    }

    // QA-3: the refusal must be logged on the coordination target.
    logs_assert(|lines: &[&str]| {
        let carries_not_addressee = lines
            .iter()
            .any(|l| l.contains("mnemra::coordination") && l.contains("not_addressee"));
        if carries_not_addressee {
            Ok(())
        } else {
            Err(format!(
                "QA-3: a `not_addressee` refusal must be logged on the `mnemra::coordination` \
                 target. Absent today — `ack`/`disposition` are unrouted, so no such log line \
                 is ever emitted. captured lines: {lines:?}"
            ))
        }
    });
}

// ===========================================================================
// Test 17 — `ack`/`disposition` from an unattached session → `not_attached`
// (R-0064-e)
// ===========================================================================

/// GIVEN a message addressed to B (an attached actor), and a SEPARATE
/// session that has NEVER polled,
/// WHEN that unattached session calls `ack` and `disposition` on B's
/// message,
/// THEN both are refused `not_attached`. *(R-0064-e)*
///
/// RED against this dispatch's parent commit: `ack`/`disposition` are
/// unrouted — both hit `INVALID_PARAMS` regardless of attachment state;
/// `not_attached` is absent.
#[tokio::test]
async fn ack_and_disposition_from_unattached_session_are_refused_not_attached() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    // ---- ack case ----
    {
        let workspace_id = Uuid::new_v4();
        let (server, client) = coordination_server(&pool).await;
        let (message_id, _addressee_token, _addressee_role) =
            seeded_message_in_state(&client, &pool, workspace_id, "delivered").await;

        let unattached_token = seed_admin_token(&pool, workspace_id).await;
        let res = client
            .call_tool(ack_params(unattached_token.as_str(), message_id))
            .await;
        assert!(
            result_surfaces_code(&res, "not_attached"),
            "R-0064-e: `ack` from an unattached session must be refused `not_attached`. Got: \
             {res:?}"
        );
        server.abort();
    }

    // ---- disposition case ----
    {
        let workspace_id = Uuid::new_v4();
        let (server, client) = coordination_server(&pool).await;
        let (message_id, _addressee_token, _addressee_role) =
            seeded_message_in_state(&client, &pool, workspace_id, "acknowledged").await;

        let unattached_token = seed_admin_token(&pool, workspace_id).await;
        let res = client
            .call_tool(disposition_params(
                unattached_token.as_str(),
                message_id,
                "completed",
                None,
            ))
            .await;
        assert!(
            result_surfaces_code(&res, "not_attached"),
            "R-0064-e: `disposition` from an unattached session must be refused \
             `not_attached`. Got: {res:?}"
        );
        server.abort();
    }
}

// ===========================================================================
// Test 18 — `ack`/`disposition` from a `read_observer` token → refused
// pre-dispatch (R-0073-b)
// ===========================================================================

/// GIVEN a `read_observer`-scoped token (never attached — attach is itself
/// write-category, R-0073-b),
/// WHEN it calls `ack` and `disposition` on an otherwise-valid `message_id`,
/// THEN both are denied at the host-fn boundary with
/// `PERMISSION_DENIED_CODE` — never a send-body/ack/disposition refusal
/// `reason_code`. *(R-0073-b)*
///
/// Mirrors test 7's `send` scenario exactly: `message` is already
/// coordination-routed, so an `ack`/`disposition` action call hits the SAME
/// `parse_action` catch-all `INVALID_PARAMS` today, never
/// `PERMISSION_DENIED_CODE` — the dedicated per-action gate
/// (`authorize_coordination_action`) is never reached because the action
/// fails to parse first.
///
/// RED against this dispatch's parent commit: the call errors with
/// `INVALID_PARAMS` (parse failure), never `PERMISSION_DENIED_CODE`.
#[tokio::test]
async fn ack_and_disposition_from_read_observer_token_are_refused_pre_dispatch() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let ro_token = seed_read_observer_token(&pool, workspace_id).await;
    let fabricated_message_id = Uuid::new_v4();

    let (server, client) = coordination_server(&pool).await;

    // ---- ack ----
    let ack_res = client
        .call_tool(ack_params(ro_token.as_str(), fabricated_message_id))
        .await;
    let ack_err = ack_res.expect_err(
        "R-0073-b: a `read_observer` token calling `message ack` must be denied at the host-fn \
         boundary (Err), never receive a refusal or success (Ok).",
    );
    match ack_err {
        rmcp::ServiceError::McpError(ref error_data) => {
            assert_eq!(
                error_data.code, PERMISSION_DENIED_CODE,
                "R-0073-b: a `read_observer` calling `message ack` must be denied with \
                 PERMISSION_DENIED_CODE ({PERMISSION_DENIED_CODE:?}); got {:?}. Today the parse \
                 failure (`ack` unsupported) fires first, before the per-action gate.",
                error_data.code
            );
        }
        other => panic!(
            "R-0073-b: expected an `rmcp::ServiceError::McpError` carrying \
             PERMISSION_DENIED_CODE for `ack`; got a different error variant: {other:?}"
        ),
    }

    // ---- disposition ----
    let disp_res = client
        .call_tool(disposition_params(
            ro_token.as_str(),
            fabricated_message_id,
            "completed",
            None,
        ))
        .await;
    let disp_err = disp_res.expect_err(
        "R-0073-b: a `read_observer` token calling `message disposition` must be denied at the \
         host-fn boundary (Err), never receive a refusal or success (Ok).",
    );
    match disp_err {
        rmcp::ServiceError::McpError(ref error_data) => {
            assert_eq!(
                error_data.code, PERMISSION_DENIED_CODE,
                "R-0073-b: a `read_observer` calling `message disposition` must be denied with \
                 PERMISSION_DENIED_CODE ({PERMISSION_DENIED_CODE:?}); got {:?}. Today the parse \
                 failure (`disposition` unsupported) fires first, before the per-action gate.",
                error_data.code
            );
        }
        other => panic!(
            "R-0073-b: expected an `rmcp::ServiceError::McpError` carrying \
             PERMISSION_DENIED_CODE for `disposition`; got a different error variant: {other:?}"
        ),
    }

    server.abort();
}

// ===========================================================================
// Test 19 — disposition `note` log hygiene (R-0075-e)
// ===========================================================================

/// GIVEN a message forced to `acknowledged`,
/// WHEN it is `disposition`ed `completed` with a `note` containing a log
/// metacharacter (an embedded newline plus a fake `reason_code=`/`op=`
/// token — the classic log-injection probe),
/// THEN any resulting `mnemra::coordination` op-log line carries the note as
/// a DISCRETE structured field value — the injected fake tokens never appear
/// FREE-STANDING (split onto their own line, as naive text interpolation of
/// an embedded newline would produce) as if they were separate real
/// structured fields. *(R-0075-e)*
///
/// # Non-vacuity discipline
///
/// A capture-liveness canary (mirrors test 8's) proves the `tracing_test`
/// capture channel is genuinely recording before the absence/presence
/// assertion is trusted.
///
/// RED against this dispatch's parent commit: `disposition` is unrouted —
/// the call never reaches any coordination write path, so no log line ever
/// carries the note at all — `carries_note_as_structured_field` is false,
/// failing the primary (positive) half of the assertion.
#[traced_test]
#[tokio::test]
async fn disposition_note_log_entry_carries_discrete_structured_field_not_spliced() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let (server, client) = coordination_server(&pool).await;
    let (message_id, addressee_token, _addressee_role) =
        seeded_message_in_state(&client, &pool, workspace_id, "acknowledged").await;

    tracing::info!("coordination_messages t19 oplog canary");
    assert!(
        logs_contain("coordination_messages t19 oplog canary"),
        "the traced_test capture channel must be live before the log-hygiene assertion is \
         trusted"
    );

    let injected_note = "legitimate note\nreason_code=fake_injected op=Forged";
    let res = client
        .call_tool(disposition_params(
            addressee_token.as_str(),
            message_id,
            "completed",
            Some(injected_note),
        ))
        .await;

    logs_assert(|lines: &[&str]| {
        let coordination_lines: Vec<&&str> = lines
            .iter()
            .filter(|l| l.contains("mnemra::coordination"))
            .collect();

        // Primary (positive, guarantee-absent) anchor: a coordination log
        // line carries the note's content — proving the whole value,
        // metacharacters included, was captured as one field.
        let carries_note_as_structured_field = coordination_lines
            .iter()
            .any(|line| line.contains("legitimate note") && line.contains("fake_injected"));

        // Secondary (negative) guard: the injected fake `reason_code=`/`op=`
        // tokens never appear FREE-STANDING on a line that lacks the
        // "legitimate note" marker — evidence the note text was split across
        // lines by a raw embedded-newline interpolation rather than carried
        // as one discrete field value.
        let fake_token_ever_free_standing = lines.iter().any(|line| {
            (line.contains("reason_code=fake_injected") || line.contains("op=Forged"))
                && !line.contains("legitimate note")
        });

        if carries_note_as_structured_field && !fake_token_ever_free_standing {
            Ok(())
        } else {
            Err(format!(
                "R-0075-e: a `disposition` `note` reaching the op log must ride as a DISCRETE \
                 structured field, never spliced such that its embedded metacharacters could be \
                 mistaken for separate fields. carries_note_as_structured_field=\
                 {carries_note_as_structured_field} fake_token_ever_free_standing=\
                 {fake_token_ever_free_standing}. Absent today — `disposition` is unrouted, so \
                 no such log line is ever emitted. disposition result: {res:?}. captured \
                 `mnemra::coordination` lines: {coordination_lines:?}"
            ))
        }
    });

    server.abort();
}

// ===========================================================================
// Test 20 — disposition emits exactly one Disposition-typed audit record
// (R-0075-b disposition half; AC10 disposition-audit half) — added in a
// Puck amendment round on the same dispatch (1634), same branch, before
// GREEN.
// ===========================================================================

/// GIVEN a message forced to `acknowledged`,
/// WHEN the addressee `disposition`s it `completed`,
/// THEN the call succeeds (`state: "dispositioned"`) AND exactly one
/// `disposition`-typed `coordination_audit` row exists naming this
/// `message_id`, attributed to the addressee's `actor_id` — the R-0075-b /
/// AC10 disposition-audit-EMISSION half. *(R-0075-b; AC10)*
///
/// # Why this test exists, and what it pins (see the module header addendum)
///
/// `run_write`'s emit-guarantee (R-0075-c, discharged generically by
/// `coordination_failclosed.rs`) only ensures a STAGED audit commits-or-
/// rolls-back atomically with the state transition — it proves NOTHING
/// about whether `disposition`'s body actually STAGES an audit row in the
/// first place. If GREEN's `disposition` body stages no `AuditRecord`,
/// `run_write` has nothing to fail on: the transition commits happily and
/// every OTHER test in this suite still passes. This test is the
/// positive-emission net that closes that gap, structurally mirroring test
/// 5's (`registration_audit_fires_iff_addressee_is_newly_minted_by_send`)
/// per-action positive proof of R-0075-b's registration half.
///
/// No plan or spec text fixes an exact JSON payload key for
/// `AuditRecord::disposition(...)` (the plan, L102, only names the factory
/// to add). This test PINS `message_id` as the identifying payload key —
/// the natural analogue of `registration`'s `role_instance` key and
/// `lease_takeover`'s `prior_holder`/`new_holder` keys — and pins `actor_id`
/// as the addressee performing the disposition (mirroring
/// `lease_takeover`'s "the same actor `record_acting_actor` already
/// attributes" convention). Per this file's "your tests are the contract"
/// framing, this is a legitimate RED-phase contract-setting choice, not a
/// guess at an already-fixed shape.
///
/// # Non-vacuity discipline
///
/// Anchored FIRST on the disposition call actually SUCCEEDING (`state:
/// "dispositioned"`) — guarantee-absent today, since `disposition` is
/// unrouted. The audit-row-presence + attribution checks are layered on
/// top, never the sole anchor (a bare "some disposition-typed row exists
/// somewhere" check would pass vacuously against an unrelated row, and an
/// unguarded absence check alone would pass vacuously against today's
/// unrouted action too).
///
/// RED against this dispatch's parent commit: `disposition` is unrouted —
/// the call hits `INVALID_PARAMS`, so the primary (positive, success)
/// anchor fails before the audit-row check is ever meaningfully exercised.
#[tokio::test]
async fn disposition_emits_exactly_one_disposition_typed_audit_record() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let (server, client) = coordination_server(&pool).await;
    let (message_id, addressee_token, addressee_role) =
        seeded_message_in_state(&client, &pool, workspace_id, "acknowledged").await;
    let addressee_actor_id = actor_id_by_name(&pool, workspace_id, &addressee_role)
        .await
        .expect("the addressee actor row must exist");

    let res = client
        .call_tool(disposition_params(
            addressee_token.as_str(),
            message_id,
            "completed",
            None,
        ))
        .await;

    let obj = send_structured_obj(&res);
    assert_eq!(
        obj.as_ref()
            .and_then(|o| o.get("state"))
            .and_then(|v| v.as_str()),
        Some("dispositioned"),
        "primary anchor: `disposition` must succeed and return `state: \"dispositioned\"`; \
         `disposition` is unrouted at this dispatch's parent commit. Got: {res:?}"
    );

    let audit_row = disposition_audit_row_for_message(&pool, workspace_id, message_id).await;
    assert!(
        audit_row.is_some(),
        "R-0075-b/AC10: a successful `disposition` must emit exactly one `disposition`-typed \
         `coordination_audit` row naming this `message_id` (payload key `message_id`, this \
         test's pinned contract). None found — guarantee-absent today since `disposition` \
         never executes."
    );
    let (audit_actor_id, _payload) = audit_row.unwrap();
    assert_eq!(
        audit_actor_id,
        Some(addressee_actor_id),
        "R-0075-b: the disposition audit's `actor_id` must be the addressee performing the \
         disposition."
    );

    server.abort();
}
