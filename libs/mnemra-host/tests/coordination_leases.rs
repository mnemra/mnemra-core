//! `claim` tool acceptance tests — slice b1 (Task 5, R-0065/R-0067/R-0073/R-0075).
//!
//! # Scope: `acquire` only
//!
//! b1 covers the `claim acquire` action alone. `renew`, `release`, `takeover`,
//! and `list` are later slices and are NOT tested here — no scenario below
//! calls those actions.
//!
//! # What this file contracts (the guarantees the parent commit does NOT yet
//! implement)
//!
//! At the parent commit (`7bc3104`, Task 4 "session plane" complete), `message
//! poll` is fully implemented — resolve-or-create, one-live-attachment,
//! succession, TTL renew, the documented poll response body. The `claim` tool
//! does not exist at all: it is neither advertised (`coordination_tools()`
//! returns only the `message` tool), nor routed
//! (`session_plane::is_coordination_tool` checks only `name ==
//! MESSAGE_TOOL`), nor parsed (`CoordinationAction` has exactly one variant,
//! `Poll`). These tests encode the R-0065/-0067/-0073/-0075 `acquire`
//! contract the GREEN implementer (Task 5 b1) must fill.
//!
//! # Why every scenario collapses to ONE wire-level RED cause (read this
//! before touching a failing test)
//!
//! Because `claim` is entirely unrouted, `call_tool("claim", ...)` falls
//! through `MnemraMcpServer::call_tool`'s host-served coordination branch
//! (`is_coordination_tool` returns `false` for `"claim"`) into the **plugin**
//! dispatch path: `auth_and_authorize` classifies the tail `"claim"` (no
//! `.`) as `PluginWriteVerb` (fail-closed default, `is_write_verb`), then —
//! for an `Admin` token — the manifest-verbs membership gate rejects it
//! because `"claim"` is not an echo-plugin verb, returning
//! `VERB_NOT_EXPOSED_CODE` (`-4005`) with message `"verb 'claim' is not in
//! the registered plugin's manifest verbs list"`. Every ADMIN-token scenario
//! below (attached or not, valid or invalid resource, valid or invalid
//! duration) hits this SAME generic `-4005` error today — the scenarios are
//! distinguished only once green wires the action-specific coordination
//! body. The single exception is the `read_observer` scenario, which is
//! denied EARLIER (`PERMISSION_DENIED_CODE`, `-4002`) by the SAME
//! `PluginWriteVerb` classification, before the manifest-verbs gate is even
//! reached — a coincidental green-on-arrival guard, flagged in its own test
//! doc comment.
//!
//! This is why "the tool call errors" is enough to make every guarantee-
//! absent scenario red (skills/tdd.md): none of these tests need a
//! carefully-reproduced red per scenario — the absence of `claim` routing
//! IS the red, uniformly. Non-vacuity is still held per-scenario below: every
//! assertion anchors on a POSITIVE guarantee (a reason code present, a DB row
//! present, a field present) that is false today and becomes true once green
//! lands — never on the accidental `-4005`/`-4002` shape itself.
//!
//! # AC ↔ test map (b1)
//!
//! | AC | Test | R-ID(s) |
//! |---|---|---|
//! | 1. acquire happy path, per resource family | `acquire_happy_path_per_resource_family` | R-0065-b, R-0067-a/-b |
//! | 2. QA-1 atomic contention (repeated-run) | `qa1_concurrent_acquire_yields_exactly_one_live_holder` | R-0065-b/-c |
//! | 3. `not_attached` | `acquire_without_attachment_is_refused_not_attached` | (the new gate) |
//! | 4. `invalid_resource` | `acquire_invalid_resource_is_refused` | R-0067-a |
//! | 5. `reserved_family` (distinct from `invalid_resource`) | `acquire_reserved_family_is_refused_distinctly_from_invalid_resource` | R-0067-c |
//! | 6. `invalid_duration` | `acquire_invalid_duration_is_refused` | R-0065-d |
//! | 7. default duration applied | `acquire_default_duration_applied_when_omitted` | R-0065-d, §Numeric calibrations |
//! | 8. `read_observer` denied pre-dispatch | `read_observer_denied_pre_dispatch_for_claim_acquire` | R-0073-b |
//! | 9. op-log presence (acquire) | `successful_acquire_emits_op_log_entry` | R-0075-a |
//!
//! # Non-vacuity discipline (held)
//!
//! Every refusal-code assertion anchors on the structured `reason_code`
//! being PRESENT (never on a no-row side effect alone, which would pass
//! vacuously against ANY error — including today's unrelated `-4005`). Every
//! "no row created" check is a SECONDARY guard layered on top of the
//! reason-code anchor, never the sole anchor. The QA-1 contention test's
//! primary anchor is `live_lease_count == 1` (never the vacuous `<= 1`,
//! which passes when NEITHER contender acquires — exactly today's case).
//!
//! # `duration_seconds` / lease-duration config note
//!
//! Unlike the attachment TTL (`CoordinationConfig::attachment_ttl`, second-
//! scale overridable for TTL-elapse tests), no scenario in b1 needs to wait
//! out an actual lease expiry — `acquire`'s default/max duration are
//! evaluated by reading the returned `acquired_at`/`expires_at` timestamps
//! immediately, never by sleeping past them (that belongs to a later slice's
//! `renew`/`takeover` tests). `CoordinationConfig` therefore needed no
//! changes for this file, and the default `coordination_server` (no TTL
//! override) is used throughout.
//!
//! # Attach precondition (LANDMINE 1)
//!
//! Every `claim` action requires a live attachment first (R-0064-e's
//! session-identity substrate). Every scenario except #3 (`not_attached`)
//! and #8 (`read_observer`, which cannot attach either — attach is itself
//! write-category) binds via `message poll` before calling `claim`,
//! mirroring `coordination_session_plane.rs`'s established bind sequence.
//!
//! # Slice b2 addendum — `list` (Task 5 b2, R-0073-a/R-0067-c/R-0075-a)
//!
//! b2 appends the `claim list` scenarios below onto the same binary (b1's
//! `acquire`-only scope note above now covers `acquire` only —
//! `renew`/`release`/`takeover` remain untested, later slices c/d; `list` is
//! covered starting at the b2 section further down this file). Slice b1
//! (`claim acquire` + the attachment resolver + `resource_id`) is DONE and
//! GREEN, merged onto this branch (`git log`: `58da1c6` red → `6028ce0`
//! green → `7ea111a` review-hardening, this dispatch's `parent_commit`) —
//! confirmed by running `cargo test -p mnemra-host --test coordination_leases`
//! before writing a line of b2: all 9 b1 tests pass.
//!
//! At this branch's HEAD, `session_plane::parse_action` recognizes exactly
//! `(MESSAGE_TOOL, "poll")` and `(CLAIM_TOOL, "acquire")` (confirmed by
//! reading the current source — this file's established black-box-adjacent
//! convention of observing host-internal surfaces, extended here to reading
//! the dispatch/routing shape itself, the same way b1's own file header
//! reasons about the pre-b1 plugin-fallback path). `(CLAIM_TOOL, "list")` is
//! unrecognized and falls through `parse_action`'s catch-all arm, which
//! returns `ErrorData { code: INVALID_PARAMS, message: "unsupported
//! coordination action 'list' for tool 'claim'" }` — and this happens in
//! `mcp/server.rs::handle_coordination` **before** `authorize_coordination_
//! action` (the per-action token-role gate) or any coordination body ever
//! runs (contract order: authenticate → `parse_action` → per-action gate →
//! route). This is the ONE uniform RED cause for every `list` scenario
//! below, admin-token or read_observer alike — see AC7's doc comment for why
//! this makes the read_observer scenario a GENUINE red this time, unlike
//! b1's AC8 (which passed at the parent commit for the wrong reason, via the
//! then-still-live generic plugin-dispatch fallback).
//!
//! ## AC ↔ test map (b2)
//!
//! | AC | Test | R-ID(s) |
//! |---|---|---|
//! | 1. list returns live non-actor leases, workspace-visible | `list_returns_live_non_actor_leases_workspace_visible` | R-0073-a |
//! | 2. list excludes `actor:` rows | `list_excludes_actor_family_rows` | R-0067-c |
//! | 3. `family` filter | `list_family_filter_returns_only_matching_family` | §API Contract `list` |
//! | 4. `resource_prefix` filter | `list_resource_prefix_filter_returns_only_matching_prefix` | §API Contract `list` |
//! | 5. `family=actor` → `reserved_family` | `list_family_actor_is_refused_reserved_family` | R-0067-c |
//! | 6. `not_attached` | `list_without_attachment_is_refused_not_attached` | R-0064-e |
//! | 7. `read_observer` denied pre-dispatch | `read_observer_denied_pre_dispatch_for_claim_list` | R-0073-b |
//! | 8a. op-log, successful list | `successful_list_emits_op_log_entry` | R-0075-a |
//! | 8b. op-log, refused list | `refused_list_emits_op_log_entry_with_reason_code` | R-0075-a |
//!
//! The dispatch brief's scenario 8 (op-log for list: success + refusal) is
//! split into two single-purpose test functions (8a/8b) rather than one
//! combined test — consistent with this file's one-scenario-per-test
//! convention (b1's AC1–AC9 are each one function).
//!
//! ## Op-log fixture finding (R-0075-d ask — answered, not missing)
//!
//! The dispatch asked whether an op-log-capture fixture exists in this file
//! before b2, and to surface it as a finding if not. It DOES exist:
//! `successful_acquire_emits_op_log_entry` (b1, AC9, same file, above) already
//! establishes the pattern 8a/8b below reuse verbatim —
//! `#[tracing_test::traced_test]` + `logs_contain`/`logs_assert` against the
//! process-global capture channel, gated by a capture-liveness canary
//! (`tracing::info!` + an immediate self-check) before the real op-log
//! assertion is trusted. No new observation seam is needed for `list`'s
//! op-log AC — the fixture question resolves to "already present," which is
//! itself the finding.
//!
//! `CoordinationOp::ClaimList` does not exist yet in `coordination::mod` at
//! this branch's HEAD (confirmed by reading the enum: `AttachBind`/
//! `Acquire`/`Renew`/`Release`/`Takeover`/`Send`/`Poll`/… only — no `List`
//! variant), and `list` never dispatches (see above), so the string
//! `op=ClaimList` cannot appear in ANY log line pre-green — a stronger
//! non-vacuity guarantee than b1 AC9 needed (which had to rule out a
//! co-occurring `AttachBind` line by pairing two conditions on one line;
//! here the op string is outright unproducible, not merely coincidentally
//! absent). 8a/8b still pair `op=ClaimList` with a second condition
//! (actor-attribution / reason-code) on the SAME line, for consistency with
//! the established convention and because the pairing is itself part of the
//! R-0075-a contract (actor attribution; refusal reason code), not just a
//! vacuity guard here.
//!
//! # Slice c addendum — `renew` + `release` (Task 5 c, R-0065-d/R-0067-c)
//!
//! c appends the `claim renew`/`claim release` scenarios below onto the same
//! binary (b1's top-of-file scope note above now covers `acquire` only,
//! superseded here for `renew`/`release` exactly as the b2 addendum
//! superseded it for `list`; `takeover` remains untested, later slice d).
//! Slices b1 (`acquire`) and b2 (`list`) are DONE and GREEN, merged onto
//! this branch (`git log`: b1 `58da1c6`→`6028ce0`→`7ea111a`, b2
//! `ff31734`→`336cd95`→`4cb15d4`, this dispatch's `parent_commit`) —
//! confirmed by running `cargo test -p mnemra-host --test coordination_leases`
//! before writing a line of c: all 18 existing tests pass.
//!
//! At this branch's HEAD, `CoordinationAction` has exactly THREE variants —
//! `Poll`, `ClaimAcquire`, `ClaimList` (confirmed by reading
//! `session_plane.rs`'s enum and `parse_action`'s match arms) — no
//! `ClaimRenew`/`ClaimRelease` exist. `(CLAIM_TOOL, "renew")` and
//! `(CLAIM_TOOL, "release")` both fall through `parse_action`'s catch-all
//! arm, returning `ErrorData { code: INVALID_PARAMS, message: "unsupported
//! coordination action 'renew'/'release' for tool 'claim'" }` in
//! `mcp/server.rs::handle_coordination`, BEFORE `authorize_coordination_
//! action` or any coordination body ever runs (the same contract order b2
//! established: authenticate → `parse_action` → per-action gate → route).
//! This is the ONE uniform RED cause for every scenario below — the
//! `not_holder`/`lease_not_found`/`reserved_family`/`not_attached` reason
//! codes asserted here are each entirely absent today: the underlying
//! `Refusal` arms (`NotHolder`, `LeaseNotFound`, `ReservedFamily`,
//! `NotAttached`) already exist in `write_path.rs` (reused by acquire/list),
//! but no code path in `leases.rs` produces them for `renew`/`release` yet —
//! those actions are not implemented at all.
//!
//! # Deterministic expiry (no wall-sleep)
//!
//! Scenario 4 (renew on an expired-but-untaken lease) needs a LIVE lease to
//! become expired without a flake-prone real-time wait. Rather than
//! `coordination_session_plane.rs`'s `coordination_server_with_ttl` +
//! `tokio::time::sleep` pattern (built for the ATTACHMENT TTL), this file
//! forces the store-clock predicate directly via a sanctioned direct-SQL
//! `UPDATE leases SET expires_at = now() - interval '1 second' WHERE id =
//! $1` — an extension of this file's existing DB-observation carve-out to a
//! single-column mutation of the same table, not a shortcut around the
//! store-clock predicate: R-0065-e reads `expires_at` fresh from the row at
//! operation time regardless of how it got there.
//!
//! # `reserved_family` fixture — the resolved-lease-row arm (R-0067-c)
//!
//! Scenario 9 needs a `lease_id` naming an `actor:`-family row — obtained
//! via direct SQL against the caller's own live attachment lease (created by
//! the `attach_session` precondition every scenario already runs), per
//! R-0067-c's own acceptance-test parenthetical: "obtained via the operator
//! SQL surface in the fixture." This is DISTINCT from `resource`-string
//! reserved-family probes (b1 AC5, `acquire "actor:whatever"`) — here the
//! caller passes a REAL row id that happens to belong to the reserved
//! family, exercising the "resolved lease row" arm of R-0067-c that the
//! acquire/takeover request-side check cannot reach.
//!
//! # Ordering landmine — found+live BEFORE holder (R-0065-d + build plan §3.2)
//!
//! The txn shape resolves in this order: `not_attached` → not-found/expired
//! (`lease_not_found`) → `reserved_family` → holder-only (`not_holder`).
//! Concretely: a non-holder renewing an EXPIRED lease must get
//! `lease_not_found`, NOT `not_holder`. Scenarios 1-2 below therefore use
//! LIVE leases held by someone else (isolating the identity axis), and
//! scenario 4 uses an EXPIRED lease with the SAME holder attempting the
//! renew (isolating the liveness axis) — each test's doc comment restates
//! which axis it isolates and rules out the adjacent wrong code.
//!
//! ## AC ↔ test map (c)
//!
//! | # | Test | Refusal / assertion | R-ID(s) |
//! |---|---|---|---|
//! | 1 | `renew_by_non_holder_on_live_lease_is_refused_not_holder` | `not_holder` | R-0065-d |
//! | 2 | `release_by_non_holder_on_live_lease_is_refused_not_holder` | `not_holder` | R-0065-d |
//! | 3 | `holder_renew_moves_expiry_forward` | `expires_at` strictly advances | R-0065-d |
//! | 4 | `renew_on_expired_untaken_lease_is_refused_lease_not_found` | `lease_not_found` | R-0065-d, R-0065-e |
//! | 5 | `release_then_release_again_is_refused_lease_not_found` | `lease_not_found` | R-0065-d |
//! | 6 | `renew_after_holders_own_release_is_refused_lease_not_found` | `lease_not_found` | R-0065-d |
//! | 7 | `renew_with_fabricated_lease_id_is_refused_lease_not_found` | `lease_not_found` | R-0065-d |
//! | 8 | `release_with_fabricated_lease_id_is_refused_lease_not_found` | `lease_not_found` | R-0065-d |
//! | 9 | `renew_and_release_naming_an_attachment_lease_id_are_refused_reserved_family` | `reserved_family` (both actions) | R-0067-c |
//! | 10 | `renew_and_release_without_attachment_are_refused_not_attached` | `not_attached` (both actions) | R-0064-e |
//!
//! R-0066-c (deposed-holder backstop) is enabled by this slice's found-live-
//! before-holder ordering, but its acceptance test needs `takeover` (slice
//! d, not built yet) — deliberately NOT tested here per the dispatch brief;
//! slice d covers it.
//!
//! # Non-vacuity discipline (held, c)
//!
//! Every refusal-code assertion anchors on the structured `reason_code`
//! being PRESENT (never a no-mutation side effect alone). Where a scenario
//! must rule out a WRONG-but-plausible adjacent code (scenario 1/2 ruling
//! out `lease_not_found`; scenario 4 ruling out `not_holder`; scenario 9
//! ruling out `not_holder`), the negative assertion is explicit — mirroring
//! b1 AC5's `reserved_family`-vs-`invalid_resource` distinctness pattern.
//!
//! # Slice d addendum — `takeover` + reserved-family sweep (Task 5 d,
//! R-0066/R-0067-c)
//!
//! d appends the `claim takeover` scenarios below onto the same binary (b1's
//! top-of-file scope note above now covers `acquire` only, superseded here
//! for `takeover` exactly as the c addendum superseded it for
//! `renew`/`release`; every action `claim` advertises today or will
//! advertise once d lands — `acquire`, `list`, `renew`, `release`,
//! `takeover` — is now covered somewhere in this file). Slices b1
//! (`acquire`), b2 (`list`), and c (`renew`/`release`) are DONE and GREEN,
//! merged onto this branch (`git log`: b1 `58da1c6`→`6028ce0`→`7ea111a`, b2
//! `ff31734`→`336cd95`→`4cb15d4`, c `db5636a`→`7616ff8`, this dispatch's
//! `parent_commit`) — confirmed by running `cargo test -p mnemra-host --test
//! coordination_leases` before writing a line of d: all 28 existing tests
//! pass.
//!
//! At this branch's HEAD, `CoordinationAction` has exactly FIVE variants —
//! `Poll`, `ClaimAcquire`, `ClaimList`, `ClaimRenew`, `ClaimRelease`
//! (confirmed by reading `session_plane.rs`'s enum and `parse_action`'s
//! match arms) — no `ClaimTakeover` exists, and `claim_tool()`'s advertised
//! `action` JSON-schema enum is `["acquire", "list", "renew", "release"]`
//! (no `"takeover"`, and — construction-audit finding, see below — no
//! force-break verb either). `(CLAIM_TOOL, "takeover")` falls through
//! `parse_action`'s catch-all arm, returning `ErrorData { code:
//! INVALID_PARAMS, message: "unsupported coordination action 'takeover' for
//! tool 'claim'" }` in `mcp/server.rs::handle_coordination`, BEFORE
//! `authorize_coordination_action` or any coordination body ever runs (the
//! same contract order b2/c established: authenticate → `parse_action` →
//! per-action gate → route). This is the ONE uniform RED cause for every
//! scenario below — the `not_expired`/`lease_not_found`/`reserved_family`/
//! `not_attached`/`invalid_resource` reason codes asserted here are each
//! entirely absent today: the underlying `Refusal` arms already exist in
//! `write_path.rs` (reused by acquire/list/renew/release), but no code path
//! in `leases.rs` produces a `takeover` body at all yet.
//!
//! # Deterministic expiry + fixture-simulated deposition (no wall-sleep, no
//! self-referential setup)
//!
//! QA-2 (the headline recovery test) reuses c's `force_expire_lease` direct-
//! SQL fixture verbatim to make a live lease expired without a flake-prone
//! real-time wait — the SAME technique, same table, same predicate
//! (R-0065-e's `now() >= expires_at` reads it fresh regardless of how the
//! row got there).
//!
//! The deposed-holder scenario (R-0066-c) has a subtlety the earlier slices
//! didn't: it needs a lease that has ALREADY BEEN taken over — but
//! `takeover` is exactly the feature under red-phase test in THIS slice, so
//! it cannot be used as its own setup (unlike b2/c, which used the
//! already-green `acquire` as fixture setup for their own scenarios). The
//! fixture instead constructs the POST-takeover DB state directly via a new
//! direct-SQL helper, `mark_lease_deposed_by_takeover` — the exact column
//! mutation build plan §3.4 step 4 documents takeover's own green
//! implementation will perform (`terminal_state='taken_over',
//! terminated_at=now(), superseded_by=<id>`). This extends this file's
//! established sanctioned-fixture carve-out (`force_expire_lease`) to the
//! SAME table's terminal-state columns, not a new observation seam.
//!
//! # `reserved_family` — request-side only here (R-0067-c)
//!
//! Unlike renew/release (which needed BOTH the request-side and resolved-
//! lease-row arms of R-0067-c, since they name a `lease_id` that could
//! itself belong to the reserved family), `takeover` takes a `resource`
//! string exactly like `acquire` — so only the request-side probe applies
//! here (mirrors b1 AC5 exactly, substituting `takeover` for `acquire`).
//!
//! # Audit-row observation (R-0066-b) — key names per the build plan's own
//! stated design, not invented here
//!
//! `AuditRecord::lease_takeover(..)` does not exist yet (green's job) —
//! `coordination/audit.rs` already carries the `LeaseTakeover` variant and
//! `as_str() == "lease_takeover"`, but no constructor. Build plan §3.4 step
//! 5 names the payload's four keys explicitly: `prior_holder`, `new_holder`,
//! `expires_at`, `takeover_ts`. This file's audit observer
//! (`lease_takeover_audit_rows`) queries exactly those four JSONB keys,
//! mirroring `coordination_session_plane.rs`'s `succession_audit_evidence`
//! pattern (`payload->>'key'` SQL-side extraction). Unlike that helper
//! (which filters by a single `actor_id`, valid because a succession
//! concerns ONE actor across two sessions), this file's observer filters by
//! `workspace_id` only — a takeover concerns TWO distinct actors (prior ≠
//! new), so there is no single "the actor this event concerns" to key on
//! without assuming which one `AuditRecord`'s own `actor_id` field will be
//! set to (unspecified by the build plan) — workspace + event_type is the
//! least-assumption filter that still isolates each scenario's one
//! takeover.
//!
//! ## Construction-audit findings (R-0066-a) — review items, not runtime
//! tests
//!
//! Per this dispatch's brief, these are surfaced here for Warden/green, not
//! forced into a runtime assertion:
//! - **No force-break verb exists.** `claim_tool()`'s advertised `action`
//!   enum at this branch's HEAD is `["acquire", "list", "renew", "release"]`
//!   — confirmed by reading the tool schema. Green must add exactly
//!   `"takeover"`, never a distinct force/break verb (R-0066-a: "no
//!   force-break verb SHALL exist at V0").
//! - **`acquire` performs no expired-row cleanup.** Confirmed by reading
//!   `leases.rs::acquire_body`'s own doc comment: the expired-but-untaken
//!   collision path is handled via `INSERT ... ON CONFLICT DO NOTHING` + a
//!   follow-up read, never a raised-error `23505` catch that could be
//!   confused with a cleanup/delete — specifically so the expired row is
//!   left exactly as-is and disposed of ONLY by `takeover`. This is ALREADY
//!   b1-green behavior — scenario 4 below
//!   (`acquire_on_expired_untaken_lease_is_refused_resource_held`) is
//!   consequently a REGRESSION GUARD (it may already pass at this branch's
//!   HEAD), not a guarantee-absent red — see that test's own doc comment
//!   and this dispatch's red-confirm evidence for the actual run result.
//!
//! ## AC ↔ test map (d)
//!
//! | # | Test | Refusal / assertion | R-ID(s) |
//! |---|---|---|---|
//! | 1 | `qa2_takeover_recovers_expired_lease_and_emits_audit_row` | succeeds; audit row w/ 4 fields | R-0066-a, R-0066-b, QA-2 |
//! | 2 | `takeover_on_live_lease_is_refused_not_expired` | `not_expired` | R-0066-a |
//! | 3 | `takeover_on_resource_with_no_lease_is_refused_lease_not_found` | `lease_not_found` | R-0066-a |
//! | 4 | `acquire_on_expired_untaken_lease_is_refused_resource_held` | `resource_held` (regression guard) | R-0066-a |
//! | 5 | `takeover_reserved_family_is_refused_distinctly_from_invalid_resource` | `reserved_family` | R-0067-c |
//! | 6 | `deposed_holder_renew_and_release_after_takeover_are_refused` | `not_holder`/`lease_not_found` family (both verbs) | R-0066-c |
//! | 7 | `takeover_without_attachment_is_refused_not_attached` | `not_attached` | R-0064-e |
//! | 8 | `takeover_invalid_resource_is_refused` | `invalid_resource` | R-0067-a (via takeover) |
//!
//! # Non-vacuity discipline (held, d)
//!
//! Every refusal-code assertion anchors on the structured `reason_code`
//! being PRESENT (never a no-mutation side effect alone). Adjacent-code
//! distinctness is ruled out explicitly where it matters: scenario 2 rules
//! out `lease_not_found` (the lease IS found, merely live); scenario 3 rules
//! out `not_expired` (there is no row to be live OR expired); scenario 5
//! rules out `invalid_resource` (mirrors b1 AC5). QA-2's primary anchor is
//! the audit row COUNT (`== 1`, never a bare `>= 1`, which would pass
//! vacuously if an unrelated audit row happened to exist) plus each of the
//! four field VALUES (never row-existence alone).

#[path = "common/shared_engine.rs"]
mod shared_engine;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use rmcp::model::{CallToolRequestParams, CallToolResult, Meta};
use rmcp::service::{RoleClient, RunningService, serve_client, serve_server};
use serde_json::json;
use tokio::io::duplex;
use tracing_test::traced_test;
use uuid::Uuid;

use mnemra_host::auth::token::{AdminToken, generate, hash};
use mnemra_host::mcp::errors::PERMISSION_DENIED_CODE;
use mnemra_host::mcp::server::MnemraMcpServer;
use mnemra_host::plugin::pool::PluginPool;
use mnemra_host::storage::postgres::engine::EmbeddedEngine;

// ===========================================================================
// Harness (mirrors `coordination_session_plane.rs` — duplicated per this
// codebase's established per-file harness convention, e.g. `mcp_verb_gate.rs`
// inlines `seed_read_observer_token` rather than importing it)
// ===========================================================================

/// Seed an admin-role token into `admin_tokens`. `scopes = ["admin"]` →
/// `Role::Admin`, clearing every write-category gate on the path to `claim`.
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

/// Seed a `read_observer`-scoped token into `admin_tokens` (AC8). Mirrors
/// `seed_read_observer_token` in `mcp_verb_gate.rs` / `mcp_server.rs` — those
/// are private to their own binaries, not importable here.
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
/// coordination branch never touches the plugin pool for `message`, and for
/// `claim` (unrouted today) the pool is touched only by the generic plugin
/// path's health gate, which a bare pool satisfies.
fn minimal_plugin_pool() -> Arc<PluginPool> {
    Arc::new(PluginPool::new().expect("PluginPool::new"))
}

/// Stand up one `MnemraMcpServer` over an in-memory duplex transport and
/// return its server task handle + a connected `rmcp` client. Default
/// coordination config (10 min attachment TTL) — no scenario here waits past
/// it.
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
            Err(e) => eprintln!("coordination_leases test server init failed: {e:?}"),
        }
    });
    let client = serve_client((), client_transport)
        .await
        .expect("client init failed");
    (handle, client)
}

/// Build the `message` `poll` bind call for `role_instance` under `token_str`
/// — the LANDMINE 1 attach precondition every `claim` action requires.
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

/// Build a `claim` tool call. `duration_seconds` is omitted from the
/// arguments map entirely when `None` (distinct from a present `0` — AC6/AC7
/// both rely on this distinction).
fn claim_params(
    token_str: &str,
    action: &str,
    resource: &str,
    duration_seconds: Option<i64>,
) -> CallToolRequestParams {
    let mut params = CallToolRequestParams::new("claim");
    params.meta = Some(token_meta(token_str));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("action".to_owned(), json!(action));
        m.insert("resource".to_owned(), json!(resource));
        if let Some(d) = duration_seconds {
            m.insert("duration_seconds".to_owned(), json!(d));
        }
        m
    });
    params
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
            "precondition: `message poll` (Task 4, already green) must bind the session before \
             any `claim` call in this file",
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
/// structured content or protocol error — used both for the closed
/// `reason_code` enum (spec §API Contract) and, in QA-1, for a concrete
/// actor-id value. A machine JSON envelope, so a serialized-contains scan is
/// exact here (not the over-matching-prose hazard `skills/bdd.md` warns
/// about — see `coordination_session_plane.rs`'s `result_surfaces_code` for
/// the same precedent).
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

/// The structured-content JSON object of a `claim`/`message` call result, or
/// `None` if the call errored outright (today's `claim` case — b1
/// unimplemented) or carried no structured content. Unlike
/// `coordination_session_plane.rs`'s `structured_obj` (which panics — used
/// only after `Ok` is already asserted), this is `Option`-returning so every
/// scenario below can use ONE accessor whether the call is expected to
/// succeed or refuse.
fn claim_structured_obj<E>(
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

/// Parse an RFC3339 timestamp field (the expected wire encoding for
/// `chrono::DateTime<Utc>` via serde).
fn parse_rfc3339(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .unwrap_or_else(|e| panic!("timestamp field `{s}` must be valid RFC3339: {e}"))
        .with_timezone(&Utc)
}

/// Heuristic: does `s` look like an RFC3339 timestamp (digit-led, carries a
/// `T` date/time separator and a `:` time separator)? Used in QA-1 to spot
/// the holder's expiry evidence inside a refusal's `detail` object without
/// pinning its exact field name — the spec (R-0065-c) commits to "carrying…
/// the lease expiry", not a key name.
fn looks_like_timestamp(s: &str) -> bool {
    s.len() >= 10
        && s.contains('T')
        && s.contains(':')
        && s.chars().next().is_some_and(|c| c.is_ascii_digit())
}

// ----- DB observers (sanctioned black-box carve-out — direct SQL on `leases`) -----

/// The live (non-terminal) lease row for `(workspace_id, resource)`, if any:
/// `(lease_id, holder_actor_id, acquired_at_epoch, expires_at_epoch)`. `None`
/// today (no scenario here creates a `leases` row — `claim` never dispatches)
/// — the guarantee-absent anchor every positive-acquire assertion rests on.
async fn live_lease_row(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    resource: &str,
) -> Option<(Uuid, Uuid, f64, f64)> {
    let row: Option<(Uuid, Uuid, f64, f64)> = sqlx::query_as(
        "SELECT id, holder_actor_id,
                EXTRACT(EPOCH FROM acquired_at)::float8,
                EXTRACT(EPOCH FROM expires_at)::float8
         FROM leases
         WHERE workspace_id = $1 AND resource = $2 AND terminal_state IS NULL",
    )
    .bind(workspace_id)
    .bind(resource)
    .fetch_optional(pool)
    .await
    .expect("live lease row query must execute");
    row
}

/// Count of live (non-terminal) lease rows for `(workspace_id, resource)`.
async fn live_lease_count(pool: &sqlx::PgPool, workspace_id: Uuid, resource: &str) -> i64 {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM leases WHERE workspace_id = $1 AND resource = $2 AND terminal_state IS NULL",
    )
    .bind(workspace_id)
    .bind(resource)
    .fetch_one(pool)
    .await
    .expect("live lease count query must execute");
    n
}

// ===========================================================================
// AC1 — acquire happy path, per resource family (R-0065-b, R-0067-a/-b)
// ===========================================================================

/// GIVEN an attached session,
/// WHEN it `acquire`s one resource from each in-family qualifier
/// (`repo-lane:…`, `file:…`, `surface:…`),
/// THEN each response carries the documented §API-Contract lease object —
/// `lease_id`, `holder.actor_id` (matching the attached actor), `acquired_at`,
/// `expires_at` — and `leases` shows one non-terminal row per resource whose
/// `holder_actor_id` matches. *(R-0065-b acquire; R-0067-a/-b family set)*
///
/// RED against the parent commit: `claim` is unrouted (see file header) — the
/// call errors with the generic `-4005`, so `claim_structured_obj` returns
/// `None` and the very first assertion fails guarantee-absent; the DB shows
/// no row for any of the three resources (never vacuously "found" — no code
/// path creates one).
#[tokio::test]
async fn acquire_happy_path_per_resource_family() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("acquirer-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    let actor_id = attach_session(&client, token.as_str(), &role_instance).await;

    let resources = [
        format!("repo-lane:mnemra/happy-lane-{}", Uuid::new_v4()),
        format!("file:src/happy-{}.rs", Uuid::new_v4()),
        format!("surface:happy-build-{}", Uuid::new_v4()),
    ];

    for resource in &resources {
        let res = client
            .call_tool(claim_params(
                token.as_str(),
                "acquire",
                resource,
                Some(3600),
            ))
            .await;

        let obj = claim_structured_obj(&res);
        assert!(
            obj.is_some(),
            "R-0065-b: `acquire {resource}` by an attached actor must return the lease object; \
             `claim` is unrouted at the parent commit so the call errors instead. Got: {res:?}"
        );
        let obj = obj.unwrap();

        assert!(
            obj.get("lease_id").and_then(|v| v.as_str()).is_some(),
            "R-0065-b: the acquire response must carry a string `lease_id`; got {obj:?}"
        );

        let holder = obj
            .get("holder")
            .and_then(|v| v.as_object())
            .unwrap_or_else(|| {
                panic!("§API Contract: the acquire response must carry `holder`; got {obj:?}")
            });
        assert_eq!(
            holder.get("actor_id").and_then(|v| v.as_str()),
            Some(actor_id.to_string().as_str()),
            "§API Contract: `holder.actor_id` must equal the attached session's actor ({actor_id})"
        );

        assert!(
            obj.get("acquired_at").and_then(|v| v.as_str()).is_some(),
            "§API Contract: the acquire response must carry a string `acquired_at`; got {obj:?}"
        );
        assert!(
            obj.get("expires_at").and_then(|v| v.as_str()).is_some(),
            "§API Contract: the acquire response must carry a string `expires_at`; got {obj:?}"
        );

        let row = live_lease_row(&pool, workspace_id, resource).await;
        assert!(
            row.is_some(),
            "R-0065-a: `acquire {resource}` must create a first-class `leases` row; none exists \
             (claim never dispatches at the parent commit)."
        );
        let (_lease_id, holder_db, _acquired, _expires) = row.unwrap();
        assert_eq!(
            holder_db, actor_id,
            "R-0076-c: the `leases.holder_actor_id` row for `{resource}` must equal the acquiring \
             actor ({actor_id}); found {holder_db}"
        );
    }

    server.abort();
}

// ===========================================================================
// AC2 — QA-1 atomic contention, repeated-run (R-0065-b/-c)
// ===========================================================================

/// GIVEN two ATTACHED sessions (distinct admin tokens, distinct role-
/// instances, distinct serving paths sharing one DB pool),
/// WHEN both `acquire` the SAME fresh resource, released from a shared start
/// barrier, over ≥ 20 independent rounds (a fresh resource id per round —
/// `repo-lane:contend/<round>` — so rounds need no teardown),
/// THEN every round shows exactly ONE live holder row in `leases` for that
/// resource, and exactly one contender is refused `resource_held` — carrying
/// the ACTUAL holder's `actor_id` and a timestamp-shaped expiry value in its
/// refusal `detail` (R-0065-c: "the workspace-visible facts"). *(R-0065-b
/// atomic acquisition; R-0065-c structured refusal; LANDMINE 2)*
///
/// # Non-vacuity discipline (LANDMINE 2, held)
///
/// The primary anchor is `live_lease_count == 1`, NEVER the vacuous `<= 1`
/// (which passes when NEITHER contender acquires — exactly what happens
/// today: `claim` is unrouted, both calls hit the generic `-4005`, and the
/// count is 0 every round). The XOR-refusal anchor is the reason code
/// `resource_held`, never a no-row side effect. The holder-attribution
/// anchor reads the ACTUAL winner from the DB (unavailable in RED — this
/// assertion panics on `row.is_some()` before ever reaching the value
/// comparison, which only becomes meaningful once green creates the row).
///
/// # Mirrors
///
/// The two-serving-paths-per-round structure copies
/// `concurrent_same_role_instance_yields_exactly_one_live_attachment` in
/// `coordination_session_plane.rs` verbatim (fresh sessions + fresh resource
/// per round, `tokio::sync::Barrier`, `multi_thread` flavor) — see that
/// test's doc comment for why two SEPARATE `serve_server`/`serve_client`
/// pairs (not one client, two tokens) are required to guarantee two distinct
/// sessions.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn qa1_concurrent_acquire_yields_exactly_one_live_holder() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();

    const ROUNDS: usize = 20;
    for round in 0..ROUNDS {
        let token_a = seed_admin_token(&pool, workspace_id).await;
        let token_b = seed_admin_token(&pool, workspace_id).await;
        let (server_a, client_a) = coordination_server(&pool).await;
        let (server_b, client_b) = coordination_server(&pool).await;

        // Distinct role-instances so both sessions can attach simultaneously
        // (one live attachment per ACTOR, not per resource) — the resource
        // under contention below is unrelated to either session's own
        // identity.
        let role_a = format!("contender-a-{round}-{}", Uuid::new_v4());
        let role_b = format!("contender-b-{round}-{}", Uuid::new_v4());
        attach_session(&client_a, token_a.as_str(), &role_a).await;
        attach_session(&client_b, token_b.as_str(), &role_b).await;

        let resource = format!("repo-lane:contend/{round}");
        let barrier = tokio::sync::Barrier::new(2);

        let fa = async {
            barrier.wait().await;
            client_a
                .call_tool(claim_params(token_a.as_str(), "acquire", &resource, None))
                .await
        };
        let fb = async {
            barrier.wait().await;
            client_b
                .call_tool(claim_params(token_b.as_str(), "acquire", &resource, None))
                .await
        };
        let (ra, rb) = tokio::join!(fa, fb);

        // Primary anchor: exactly ONE live holder — never the vacuous <= 1.
        let live = live_lease_count(&pool, workspace_id, &resource).await;
        assert_eq!(
            live, 1,
            "QA-1 round {round}: exactly ONE live holder must exist for `{resource}` after two \
             concurrent `acquire` calls; found {live}. Against the unrouted `claim` tool this is \
             0 (right-reason red: claim never dispatches, so no lease is ever created). a={ra:?} \
             b={rb:?}"
        );

        // Exactly one contender refused `resource_held` (the loser).
        let a_refused = result_surfaces_code(&ra, "resource_held");
        let b_refused = result_surfaces_code(&rb, "resource_held");
        assert!(
            a_refused ^ b_refused,
            "QA-1 round {round}: exactly ONE contender must be refused `resource_held` (the \
             loser); the other acquires. a_refused={a_refused} b_refused={b_refused}. Against the \
             unrouted `claim` tool NEITHER is refused this way (both hit the generic dispatch \
             error). a={ra:?} b={rb:?}"
        );

        // Holder-attribution + expiry evidence in the loser's refusal detail.
        let winner_actor_id = live_lease_row(&pool, workspace_id, &resource)
            .await
            .map(|(_id, holder, _acq, _exp)| holder)
            .expect(
                "QA-1: the live holder row must exist once exactly one contender has acquired — \
                 unreachable in RED (asserted above already), reached only once green lands",
            );
        let loser = if a_refused { &ra } else { &rb };

        let loser_obj = claim_structured_obj(loser);
        assert!(
            loser_obj.is_some(),
            "QA-1 round {round}: the loser's `resource_held` refusal must carry structured \
             content (the `detail` object with holder + expiry evidence, R-0065-c). loser={loser:?}"
        );
        let loser_obj = loser_obj.unwrap();
        let detail = loser_obj.get("detail").and_then(|v| v.as_object());
        assert!(
            detail.is_some(),
            "QA-1 round {round}: the refusal response must carry a `detail` object (spec §API \
             Contract error taxonomy: `{{ refused: true, reason_code, detail }}`); got: {loser_obj:?}"
        );
        let detail = detail.unwrap();
        assert!(
            detail
                .values()
                .any(|v| v.as_str() == Some(winner_actor_id.to_string().as_str())),
            "QA-1 round {round}: the refusal `detail` must name the ACTUAL holder's actor_id \
             ({winner_actor_id}) — R-0065-c 'the workspace-visible facts'. detail={detail:?}"
        );
        assert!(
            detail
                .values()
                .any(|v| v.as_str().is_some_and(looks_like_timestamp)),
            "QA-1 round {round}: the refusal `detail` must carry the holder's lease expiry as a \
             timestamp-shaped value (R-0065-c). detail={detail:?}"
        );

        server_a.abort();
        server_b.abort();
    }
}

// ===========================================================================
// AC3 — not_attached (LANDMINE 1: the new gate)
// ===========================================================================

/// GIVEN a fresh session that has NEVER bound via `message poll`,
/// WHEN it calls `claim acquire` on an otherwise-valid resource,
/// THEN it is refused `not_attached` and NO `leases` row is created. *(the
/// new attach-gate every `claim` action requires — LANDMINE 1)*
///
/// RED against the parent commit: `claim` is unrouted, so the call errors
/// with the generic `-4005` regardless of attachment state — `not_attached`
/// is entirely absent (guarantee-absent, not vacuous: the no-row check is
/// layered on top of the reason-code anchor, never the sole anchor).
#[tokio::test]
async fn acquire_without_attachment_is_refused_not_attached() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let resource = format!("repo-lane:mnemra/unattached-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;

    // No `poll` call — this session never attaches.
    let res = client
        .call_tool(claim_params(token.as_str(), "acquire", &resource, None))
        .await;

    assert!(
        result_surfaces_code(&res, "not_attached"),
        "LANDMINE 1: `claim acquire` from a session with no live attachment must be refused \
         `not_attached`. Against the unrouted `claim` tool the code is absent (generic `-4005`). \
         Got: {res:?}"
    );

    let live = live_lease_count(&pool, workspace_id, &resource).await;
    assert_eq!(
        live, 0,
        "a `not_attached` refusal must create no `leases` row; found {live} for `{resource}`."
    );

    server.abort();
}

// ===========================================================================
// AC4 — invalid_resource (R-0067-a)
// ===========================================================================

/// GIVEN an attached session,
/// WHEN it `acquire`s (a) an out-of-family resource (`bogus:x`) and (b) a
/// malformed resource with no `:` qualifier at all (`repo-lane`),
/// THEN each is refused `invalid_resource` and creates no `leases` row.
/// *(R-0067-a)*
///
/// RED against the parent commit: `claim` is unrouted — both calls hit the
/// generic `-4005`; `invalid_resource` is absent for either malformation.
#[tokio::test]
async fn acquire_invalid_resource_is_refused() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("invalid-resource-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    for (label, resource) in [
        ("out-of-family", "bogus:x"),
        ("malformed (no qualifier)", "repo-lane"),
    ] {
        let res = client
            .call_tool(claim_params(token.as_str(), "acquire", resource, None))
            .await;

        assert!(
            result_surfaces_code(&res, "invalid_resource"),
            "R-0067-a: `acquire \"{resource}\"` ({label}) must be refused `invalid_resource`. \
             Against the unrouted `claim` tool the code is absent (generic `-4005`). Got: {res:?}"
        );

        let live = live_lease_count(&pool, workspace_id, resource).await;
        assert_eq!(
            live, 0,
            "an `invalid_resource` refusal must create no `leases` row; found {live} for \
             `{resource}` ({label})."
        );
    }

    server.abort();
}

// ===========================================================================
// AC5 — reserved_family, distinct from invalid_resource (R-0067-c)
// ===========================================================================

/// GIVEN an attached session,
/// WHEN it `acquire`s a well-formed but RESERVED resource (`actor:whatever`
/// — the `actor` family is barred from the entire `claim` surface),
/// THEN it is refused the DEDICATED `reserved_family` code — NOT
/// `invalid_resource` — so a typo and a reserved-family probe are
/// distinguishable to the caller (R-0067-c's explicit distinctness
/// requirement). No `leases` row is created.
///
/// RED against the parent commit: `claim` is unrouted — the call hits the
/// generic `-4005`; `reserved_family` is entirely absent (and so, trivially,
/// is `invalid_resource` — the precision guard below is what makes this a
/// non-vacuous DISTINCTNESS assertion once green lands, not merely "some
/// refusal happened").
#[tokio::test]
async fn acquire_reserved_family_is_refused_distinctly_from_invalid_resource() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("reserved-family-{}", Uuid::new_v4());
    let resource = "actor:whatever";

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    let res = client
        .call_tool(claim_params(token.as_str(), "acquire", resource, None))
        .await;

    assert!(
        result_surfaces_code(&res, "reserved_family"),
        "R-0067-c: `acquire \"{resource}\"` (the reserved `actor:` family) must be refused the \
         dedicated `reserved_family` code. Against the unrouted `claim` tool the code is absent \
         (generic `-4005`). Got: {res:?}"
    );
    assert!(
        !result_surfaces_code(&res, "invalid_resource"),
        "R-0067-c: a reserved-family refusal must be DISTINCT from `invalid_resource` — a typo \
         and a reserved-family probe must be distinguishable. If `invalid_resource` appears \
         instead of/alongside `reserved_family`, the distinctness requirement is violated. \
         Got: {res:?}"
    );

    let live = live_lease_count(&pool, workspace_id, resource).await;
    assert_eq!(
        live, 0,
        "a `reserved_family` refusal must create no `leases` row; found {live} for `{resource}`."
    );

    server.abort();
}

// ===========================================================================
// AC6 — invalid_duration (R-0065-d)
// ===========================================================================

/// GIVEN an attached session,
/// WHEN it `acquire`s a resource with `duration_seconds` (a) far above the
/// policy maximum (4 hours / 14400 s — well above at 100000 s), (b) `0`, and
/// (c) negative (`-1`),
/// THEN each is refused `invalid_duration` and creates no `leases` row.
/// *(R-0065-d)*
///
/// RED against the parent commit: `claim` is unrouted — all three calls hit
/// the generic `-4005`; `invalid_duration` is absent for every case.
#[tokio::test]
async fn acquire_invalid_duration_is_refused() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("invalid-duration-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    for (label, duration_seconds) in [
        ("above policy maximum (4h = 14400s)", 100_000_i64),
        ("zero", 0_i64),
        ("negative", -1_i64),
    ] {
        let resource = format!(
            "repo-lane:mnemra/invalid-duration-{label}-{}",
            Uuid::new_v4()
        );
        let res = client
            .call_tool(claim_params(
                token.as_str(),
                "acquire",
                &resource,
                Some(duration_seconds),
            ))
            .await;

        assert!(
            result_surfaces_code(&res, "invalid_duration"),
            "R-0065-d: `acquire` with `duration_seconds = {duration_seconds}` ({label}) must be \
             refused `invalid_duration`. Against the unrouted `claim` tool the code is absent \
             (generic `-4005`). Got: {res:?}"
        );

        let live = live_lease_count(&pool, workspace_id, &resource).await;
        assert_eq!(
            live, 0,
            "an `invalid_duration` refusal must create no `leases` row; found {live} for \
             `{resource}` ({label})."
        );
    }

    server.abort();
}

// ===========================================================================
// AC7 — default duration applied when omitted (R-0065-d, §Numeric calibrations)
// ===========================================================================

/// GIVEN an attached session,
/// WHEN it `acquire`s a resource with `duration_seconds` OMITTED entirely,
/// THEN the call succeeds and `expires_at - acquired_at` is approximately
/// the configured default lease duration — 15 minutes / 900 s per §Numeric
/// calibrations, asserted within a 5 s tolerance, read directly from the two
/// returned timestamps (no wall-clock wait). *(R-0065-d; §Numeric
/// calibrations)*
///
/// RED against the parent commit: `claim` is unrouted — the call errors with
/// the generic `-4005`, so `claim_structured_obj` returns `None` and the
/// first assertion fails guarantee-absent before any timestamp arithmetic
/// runs.
#[tokio::test]
async fn acquire_default_duration_applied_when_omitted() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let token = seed_admin_token(&pool, Uuid::new_v4()).await;
    let role_instance = format!("default-duration-{}", Uuid::new_v4());
    let resource = format!("repo-lane:mnemra/default-duration-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    // `duration_seconds` OMITTED — None means the key is absent from the
    // arguments map (see `claim_params`), not a present `0`.
    let res = client
        .call_tool(claim_params(token.as_str(), "acquire", &resource, None))
        .await;

    let obj = claim_structured_obj(&res);
    assert!(
        obj.is_some(),
        "§Numeric calibrations: `acquire` with `duration_seconds` omitted must succeed with the \
         configured default (15 min); `claim` is unrouted at the parent commit so the call errors \
         instead. Got: {res:?}"
    );
    let obj = obj.unwrap();

    let acquired_at = obj
        .get("acquired_at")
        .and_then(|v| v.as_str())
        .map(parse_rfc3339)
        .expect("§API Contract: the acquire response must carry a string `acquired_at`");
    let expires_at = obj
        .get("expires_at")
        .and_then(|v| v.as_str())
        .map(parse_rfc3339)
        .expect("§API Contract: the acquire response must carry a string `expires_at`");

    let delta_secs = (expires_at - acquired_at).num_seconds();
    assert!(
        (delta_secs - 900).abs() <= 5,
        "§Numeric calibrations: an omitted `duration_seconds` must default to ~900 s (15 min); \
         got expires_at - acquired_at = {delta_secs}s (acquired_at={acquired_at}, \
         expires_at={expires_at})"
    );

    server.abort();
}

// ===========================================================================
// AC8 — read_observer denied pre-dispatch (R-0073-b)
// ===========================================================================

/// GIVEN a `read_observer`-scoped token (never attached — attach is itself
/// write-category, so a read_observer cannot bind either),
/// WHEN it calls `claim acquire` on an otherwise-valid resource,
/// THEN the call is denied at the host-fn boundary with
/// `PERMISSION_DENIED_CODE` (`-4002`) — an AUTHORIZATION error, distinct from
/// a `claim` refusal `reason_code`; the claim body is never reached (no
/// `leases` row is created). *(R-0073-b)*
///
/// # NOT a guarantee-absent red — a green-on-arrival CONTRACT GUARD, with a
/// mechanism caveat (read before trusting this test post-green)
///
/// Unlike every other scenario in this file, this ALREADY passes against the
/// parent commit — but via the WRONG mechanism. Because `claim` is unrouted
/// (see file header), the call falls through to the GENERIC plugin dispatch
/// path: `is_write_verb("claim")` classifies the tail (no `.`) as
/// `PluginWriteVerb` (fail-closed default), and `authorize` denies
/// `read_observer` for any non-`PluginReadVerb` — producing the SAME
/// `PERMISSION_DENIED_CODE` the INTENDED per-action coordination gate
/// (`authorize_coordination_action` / `Verb::CoordinationWriteVerb`) will
/// produce once Task 5 extends its `match` with `claim`'s actions (per
/// `mcp::dispatch`'s own doc comment: "Task 5 extends the match with
/// `claim`'s actions — every arm maps to `CoordinationWriteVerb`"). This
/// mirrors `mcp::dispatch`'s own remark that the tail-based classification
/// of `"message"` as write today is "a coincidence, not the mechanism" — the
/// same is true of `"claim"` here.
///
/// This test is authored as an explicit CONTRACT GUARD (mirrors
/// `coordination_session_plane.rs`'s `bind_before_expiry_is_refused_and_...`
/// pattern) so it does not trip StuckDetector — it passes now and MUST stay
/// passing once green lands. A black-box client cannot distinguish WHICH
/// internal gate produced `-4002` (both shapes are byte-identical:
/// `ErrorData { code: PERMISSION_DENIED_CODE, message: "permission denied",
/// data: None }`) — confirming the mechanism (not just the code) migrates to
/// the dedicated `authorize_coordination_action` path is a
/// construction/review-audit item, flagged for Warden's post-Task-5 review.
#[tokio::test]
async fn read_observer_denied_pre_dispatch_for_claim_acquire() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let ro_token = seed_read_observer_token(&pool, workspace_id).await;
    let resource = format!("repo-lane:mnemra/ro-denied-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;

    let res = client
        .call_tool(claim_params(ro_token.as_str(), "acquire", &resource, None))
        .await;

    let err = res.expect_err(
        "R-0073-b: a `read_observer` token calling `claim acquire` must be denied at the \
         host-fn boundary (Err), never receive a claim-body refusal or success (Ok).",
    );

    match err {
        rmcp::ServiceError::McpError(ref error_data) => {
            assert_eq!(
                error_data.code, PERMISSION_DENIED_CODE,
                "R-0073-b: a `read_observer` calling `claim acquire` must be denied with \
                 PERMISSION_DENIED_CODE ({PERMISSION_DENIED_CODE:?}); got {:?}. See this test's \
                 doc comment for the mechanism caveat (today's denial is via the generic plugin \
                 write-verb gate, not yet the dedicated coordination action gate).",
                error_data.code
            );
        }
        other => panic!(
            "R-0073-b: expected an `rmcp::ServiceError::McpError` carrying PERMISSION_DENIED_CODE; \
             got a different error variant: {other:?}"
        ),
    }

    let live = live_lease_count(&pool, workspace_id, &resource).await;
    assert_eq!(
        live, 0,
        "a read_observer denial must never reach the claim body; found {live} live lease(s) for \
         `{resource}`."
    );

    server.abort();
}

// ===========================================================================
// AC9 — op-log presence on a successful acquire (R-0075-a)
// ===========================================================================

/// GIVEN an attached session,
/// WHEN it `acquire`s a resource successfully,
/// THEN a coordination op-log entry SPECIFIC TO THE ACQUIRE OPERATION is
/// emitted on the unified `tracing` stream — a single line carrying both the
/// static commit message `write_path::log_outcome` emits on every `Commit`
/// outcome ("coordination write committed", target `COORDINATION_TARGET`)
/// AND the operation field `op=Acquire` (the `CoordinationOp::Acquire`
/// Debug-formatted `op = ?op` field `log_outcome` attaches). *(R-0075-a)*
///
/// # Observability surface (found per the dispatch's ask)
///
/// `coordination_session_plane.rs` tests R-0075-b (the DB-backed
/// `coordination_audit` subset) but never R-0075-a (the `tracing`-based op
/// log) — R-0075-a is genuinely tracing-only, not DB-observable. The
/// established in-repo pattern for asserting op-log presence is
/// `#[tracing_test::traced_test]` + `logs_contain`/`logs_assert`, used
/// directly against `write_path::log_outcome` in `coordination/write_path.
/// rs`'s own `#[cfg(test)]` module (e.g. `log_outcome_commit_carries_
/// target_and_outcome_field`) and, for a full end-to-end MCP path with a
/// `tokio::spawn`-ed server exactly like this file's harness, in `tests/
/// startup_run_full.rs` (`run_returns_ok_after_construction_...`:
/// `logs_contain(PLUGIN_LOAD_LOG_LINE)` after driving the server through its
/// real call path). `tracing-test` installs a PROCESS-GLOBAL subscriber
/// (`set_global_default`, not thread-local), so it captures the spawned
/// server task's events; the `#[traced_test]`-injected span uses the default
/// single-threaded `#[tokio::test]` flavor here (NOT `multi_thread`, unlike
/// AC2) so the span-enter guard's thread-local scope covers the spawned
/// server task (current-thread runtime keeps every task, including
/// `tokio::spawn`ed ones, on the one OS thread the guard is entered on).
///
/// # Non-vacuity discipline — a real trap this test's first draft fell into
///
/// The static message "coordination write committed" is NOT operation-
/// specific: `log_outcome` emits the SAME literal text for every successful
/// `CoordinationOp` — including `AttachBind`, which the `attach_session`
/// precondition call (`message poll`, already green at the parent commit)
/// ALSO commits successfully. A `logs_contain("coordination write
/// committed")` check alone is satisfied by the ATTACH step's own op-log
/// line and passes VACUOUSLY today, before `claim acquire` ever runs —
/// caught in this dispatch's own dry-run (the first draft reported this test
/// green against the parent commit, which is wrong for a guarantee-absent
/// scenario). The fix: anchor on a single LINE carrying BOTH the commit
/// message AND `op=Acquire` (`logs_assert`, not `logs_contain`, since the
/// latter only proves the two substrings exist SOMEWHERE in the buffer, not
/// on the same line — the attach line contributes `op=AttachBind` and a
/// *different* line would contribute the acquire attempt's own commit text
/// only once green wires it through `run_write`).
///
/// RED against the parent commit: `claim` is unrouted, so the call never
/// reaches `PgCoordinationStore::run_write` / `log_outcome` at all — no log
/// line ever carries `op=Acquire`, so no line satisfies both conditions
/// (the attach step's own commit line carries `op=AttachBind` instead).
#[traced_test]
#[tokio::test]
async fn successful_acquire_emits_op_log_entry() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let token = seed_admin_token(&pool, Uuid::new_v4()).await;
    let role_instance = format!("oplog-{}", Uuid::new_v4());
    let resource = format!("repo-lane:mnemra/oplog-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    // Capture-liveness canary (mirrors `startup_run_full.rs`): proves the
    // subscriber is genuinely recording before the absence/presence
    // assertion below is trusted.
    tracing::info!("coordination_leases oplog canary");
    assert!(
        logs_contain("coordination_leases oplog canary"),
        "the traced_test capture channel must be live before the op-log assertion is trusted"
    );

    let res = client
        .call_tool(claim_params(
            token.as_str(),
            "acquire",
            &resource,
            Some(3600),
        ))
        .await;

    logs_assert(|lines: &[&str]| {
        let hit = lines.iter().any(|line| {
            line.contains("coordination write committed") && line.contains("op=Acquire")
        });
        if hit {
            Ok(())
        } else {
            // `no-env-filter` captures every trace-level line (sqlx queries,
            // embedded-Postgres setup, …) for this test's scope — dumping
            // `lines` in full would bury the diagnostic. Narrow to the
            // coordination op-log's own lines (the target tag every
            // `log_outcome` emission carries) so the failure message stays
            // readable.
            let coordination_lines: Vec<&&str> = lines
                .iter()
                .filter(|l| l.contains("mnemra::coordination"))
                .collect();
            Err(format!(
                "R-0075-a: a successful `claim acquire` must emit ONE op-log line carrying BOTH \
                 the commit message (\"coordination write committed\") AND `op=Acquire` — \
                 distinguishing it from the attach precondition's OWN commit line (which carries \
                 `op=AttachBind` instead; see this test's doc comment for why a bare \
                 `logs_contain(\"coordination write committed\")` passes vacuously here). Absent \
                 against the parent commit — `claim` never reaches `run_write`/`log_outcome` \
                 because the tool is unrouted, so no line ever carries `op=Acquire`. acquire \
                 result: {res:?}. captured `mnemra::coordination` lines: {coordination_lines:?}"
            ))
        }
    });

    server.abort();
}

// ===========================================================================
// b2 helpers — `claim list` (Task 5 b2, R-0073-a/R-0067-c/R-0075-a)
// ===========================================================================

/// Build a `claim list` tool call. `family` and `resource_prefix` are each
/// omitted from the arguments map entirely when `None` (mirrors
/// `claim_params`'s `duration_seconds` omission convention above).
fn claim_list_params(
    token_str: &str,
    family: Option<&str>,
    resource_prefix: Option<&str>,
) -> CallToolRequestParams {
    let mut params = CallToolRequestParams::new("claim");
    params.meta = Some(token_meta(token_str));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("action".to_owned(), json!("list"));
        if let Some(f) = family {
            m.insert("family".to_owned(), json!(f));
        }
        if let Some(p) = resource_prefix {
            m.insert("resource_prefix".to_owned(), json!(p));
        }
        m
    });
    params
}

/// Acquire a lease as scenario SETUP (not the assertion under test) — panics
/// with a precondition message on failure. `claim acquire` is GREEN at this
/// branch's HEAD (Task 5 slice b1, merged onto this branch — confirmed by
/// running this file's own b1 suite before writing b2), so every `list`
/// scenario below may rely on it to seed fixture leases.
async fn acquire_lease_for_setup(
    client: &RunningService<RoleClient, ()>,
    token: &str,
    resource: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let res = client
        .call_tool(claim_params(token, "acquire", resource, Some(3600)))
        .await;
    claim_structured_obj(&res).unwrap_or_else(|| {
        panic!(
            "precondition: `claim acquire {resource}` (Task 5 slice b1, already green on this \
             branch) must succeed to seed this scenario's fixture lease. Got: {res:?}"
        )
    })
}

/// The `leases` array from a `claim list` response's structured content, or
/// `None` if the call errored/refused (no `leases` array on a refusal) or
/// carried no structured content at all (today's guarantee-absent case —
/// `list` is unrecognized, see file header addendum).
fn list_leases_array<E>(result: &Result<CallToolResult, E>) -> Option<Vec<serde_json::Value>> {
    claim_structured_obj(result)
        .and_then(|obj| obj.get("leases").and_then(|v| v.as_array()).cloned())
}

/// `true` iff some entry in `leases` carries `resource == needle`.
fn leases_array_contains_resource(leases: &[serde_json::Value], needle: &str) -> bool {
    leases
        .iter()
        .any(|l| l.get("resource").and_then(|v| v.as_str()) == Some(needle))
}

/// `true` iff some entry in `leases` carries an `actor:`-family `resource`
/// (the R-0067-c exclusion AC2 asserts is NEVER true).
fn leases_array_has_any_actor_family_resource(leases: &[serde_json::Value]) -> bool {
    leases.iter().any(|l| {
        l.get("resource")
            .and_then(|v| v.as_str())
            .is_some_and(|r| r.starts_with("actor:"))
    })
}

/// `true` iff `obj[field]` is present and is a JSON string (the §API
/// Contract "carries a string `<field>`" shape check, reused across AC1's
/// per-field assertions).
fn obj_has_string_field(obj: &serde_json::Value, field: &str) -> bool {
    obj.get(field).and_then(|v| v.as_str()).is_some()
}

// ===========================================================================
// b2 AC1 — list returns live non-actor leases, workspace-visible (R-0073-a)
// ===========================================================================

/// GIVEN two attached actors A and B (distinct role-instances, distinct
/// admin tokens) where A has acquired two leases across two resource
/// families,
/// WHEN B (who acquired nothing itself) calls `claim list` with no filters,
/// THEN B's response carries both of A's leases with the documented
/// §API-Contract lease-object fields (`lease_id`, `resource`,
/// `holder.actor_id`, `holder.role_instance`, `acquired_at`, `expires_at`) —
/// proving the read is workspace-visible, not self-scoped (R-0073-a: "any
/// resolved actor").
///
/// RED against this branch's HEAD: `(CLAIM_TOOL, "list")` is unrecognized by
/// `parse_action` (see file header addendum) — B's call errors
/// `INVALID_PARAMS` before any coordination body runs, so `list_leases_array`
/// returns `None` and the first assertion fails guarantee-absent.
#[tokio::test]
async fn list_returns_live_non_actor_leases_workspace_visible() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token_a = seed_admin_token(&pool, workspace_id).await;
    let token_b = seed_admin_token(&pool, workspace_id).await;
    let role_a = format!("list-holder-{}", Uuid::new_v4());
    let role_b = format!("list-reader-{}", Uuid::new_v4());

    // Two SEPARATE serve_server/serve_client pairs — mirrors the QA-1
    // rationale (coordination_session_plane.rs / this file's AC2 above): the
    // safe choice under either plausible session-derivation, even though
    // this scenario needs no concurrency, only two genuinely distinct
    // attachments.
    let (server_a, client_a) = coordination_server(&pool).await;
    let (server_b, client_b) = coordination_server(&pool).await;
    let actor_a = attach_session(&client_a, token_a.as_str(), &role_a).await;
    attach_session(&client_b, token_b.as_str(), &role_b).await;

    let resource_repo = format!("repo-lane:mnemra/list-happy-{}", Uuid::new_v4());
    let resource_file = format!("file:src/list-happy-{}.rs", Uuid::new_v4());
    acquire_lease_for_setup(&client_a, token_a.as_str(), &resource_repo).await;
    acquire_lease_for_setup(&client_a, token_a.as_str(), &resource_file).await;

    // B — who acquired nothing — lists.
    let res = client_b
        .call_tool(claim_list_params(token_b.as_str(), None, None))
        .await;

    let leases = list_leases_array(&res);
    assert!(
        leases.is_some(),
        "R-0073-a: `claim list` by an attached actor (B) must return the `{{ leases: [...] }}` \
         response, even though B holds no leases of its own; `list` is unrecognized at this \
         branch's HEAD so the call errors instead. Got: {res:?}"
    );
    let leases = leases.unwrap();

    assert!(
        leases_array_contains_resource(&leases, &resource_repo),
        "R-0073-a: B's `list` must surface A's `{resource_repo}` lease (workspace-visible read, \
         not self-scoped). leases={leases:?}"
    );
    assert!(
        leases_array_contains_resource(&leases, &resource_file),
        "R-0073-a: B's `list` must surface A's `{resource_file}` lease. leases={leases:?}"
    );

    let repo_entry = leases
        .iter()
        .find(|l| l.get("resource").and_then(|v| v.as_str()) == Some(resource_repo.as_str()))
        .expect("asserted present above");
    assert!(
        obj_has_string_field(repo_entry, "lease_id"),
        "§API Contract: each list entry must carry a string `lease_id`; got {repo_entry:?}"
    );
    let holder = repo_entry
        .get("holder")
        .and_then(|v| v.as_object())
        .unwrap_or_else(|| {
            panic!("§API Contract: each list entry must carry `holder`; got {repo_entry:?}")
        });
    assert_eq!(
        holder.get("actor_id").and_then(|v| v.as_str()),
        Some(actor_a.to_string().as_str()),
        "§API Contract: the `{resource_repo}` entry's `holder.actor_id` must equal A's actor id \
         ({actor_a})"
    );
    assert_eq!(
        holder.get("role_instance").and_then(|v| v.as_str()),
        Some(role_a.as_str()),
        "§API Contract: the `{resource_repo}` entry's `holder.role_instance` must equal A's \
         bound role-instance (`{role_a}`)"
    );
    assert!(
        obj_has_string_field(repo_entry, "acquired_at"),
        "§API Contract: each list entry must carry a string `acquired_at`; got {repo_entry:?}"
    );
    assert!(
        obj_has_string_field(repo_entry, "expires_at"),
        "§API Contract: each list entry must carry a string `expires_at`; got {repo_entry:?}"
    );

    server_a.abort();
    server_b.abort();
}

// ===========================================================================
// b2 AC2 — list excludes `actor:` rows (R-0067-c)
// ===========================================================================

/// GIVEN an attached actor (attaching itself creates a live `actor:<id>`
/// lease row under the R-0064-f attachment-as-lease realization — confirmed
/// live on this branch by reading `session_plane.rs::attach_body`) that has
/// ALSO acquired one ordinary resource,
/// WHEN it calls `claim list` with no filters,
/// THEN the response includes the ordinary resource but contains NO entry
/// whose `resource` is `actor:`-family — even though the calling actor's OWN
/// attachment row is live and would be the first candidate a naive
/// implementation forgot to exclude. *(R-0067-c)*
///
/// RED against this branch's HEAD: `list` is unrecognized (see file header
/// addendum) — the call errors `INVALID_PARAMS`, so `list_leases_array`
/// returns `None` and the first assertion fails guarantee-absent.
#[tokio::test]
async fn list_excludes_actor_family_rows() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("list-excl-actor-{}", Uuid::new_v4());
    let resource = format!("surface:list-excl-actor-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;
    acquire_lease_for_setup(&client, token.as_str(), &resource).await;

    // Precondition sanity: the attachment DID create a live `actor:` row —
    // the exclusion below is a real exclusion, not a coincidental absence.
    let (actor_rows,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM leases WHERE workspace_id = $1 AND resource LIKE 'actor:%' \
         AND terminal_state IS NULL",
    )
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .expect("actor-row sanity query must execute");
    assert_eq!(
        actor_rows, 1,
        "precondition: attaching must create exactly one live `actor:`-family lease row \
         (R-0064-f) — found {actor_rows}; the exclusion this test asserts would be vacuous \
         without a real `actor:` row present."
    );

    let res = client
        .call_tool(claim_list_params(token.as_str(), None, None))
        .await;

    let leases = list_leases_array(&res);
    assert!(
        leases.is_some(),
        "R-0067-c: `claim list` by an attached actor must return the `{{ leases: [...] }}` \
         response; `list` is unrecognized at this branch's HEAD so the call errors instead. \
         Got: {res:?}"
    );
    let leases = leases.unwrap();

    assert!(
        leases_array_contains_resource(&leases, &resource),
        "sanity: the ordinary `{resource}` lease must appear in `list`. leases={leases:?}"
    );
    assert!(
        !leases_array_has_any_actor_family_resource(&leases),
        "R-0067-c: `claim list` SHALL NOT return `actor:`-family rows in any response — even \
         the CALLING actor's own live attachment row must be excluded. leases={leases:?}"
    );

    server.abort();
}

// ===========================================================================
// b2 AC3 — `family` filter (§API Contract `list`)
// ===========================================================================

/// GIVEN an attached actor holding leases in two DIFFERENT families (`file`
/// and `surface`),
/// WHEN it calls `claim list` with `family: "file"`,
/// THEN the response includes only the `file:` lease and excludes the
/// `surface:` lease. *(§API Contract `list` request shape; R-0073-a read
/// default)*
///
/// RED against this branch's HEAD: `list` is unrecognized — the call errors
/// `INVALID_PARAMS`, so `list_leases_array` returns `None` and the first
/// assertion fails guarantee-absent.
#[tokio::test]
async fn list_family_filter_returns_only_matching_family() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let token = seed_admin_token(&pool, Uuid::new_v4()).await;
    let role_instance = format!("list-family-filter-{}", Uuid::new_v4());
    let resource_file = format!("file:src/family-filter-{}.rs", Uuid::new_v4());
    let resource_surface = format!("surface:family-filter-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;
    acquire_lease_for_setup(&client, token.as_str(), &resource_file).await;
    acquire_lease_for_setup(&client, token.as_str(), &resource_surface).await;

    let res = client
        .call_tool(claim_list_params(token.as_str(), Some("file"), None))
        .await;

    let leases = list_leases_array(&res);
    assert!(
        leases.is_some(),
        "§API Contract: `claim list {{family: \"file\"}}` must return the `{{ leases: [...] }}` \
         response; `list` is unrecognized at this branch's HEAD so the call errors instead. \
         Got: {res:?}"
    );
    let leases = leases.unwrap();

    assert!(
        leases_array_contains_resource(&leases, &resource_file),
        "the `family: \"file\"` filter must include the `file:` lease. leases={leases:?}"
    );
    assert!(
        !leases_array_contains_resource(&leases, &resource_surface),
        "the `family: \"file\"` filter must EXCLUDE the `surface:` lease. leases={leases:?}"
    );

    server.abort();
}

// ===========================================================================
// b2 AC4 — `resource_prefix` filter (§API Contract `list`)
// ===========================================================================

/// GIVEN an attached actor holding two `repo-lane:` leases under the SAME
/// repo (different lanes) and one `repo-lane:` lease under a DIFFERENT repo,
/// WHEN it calls `claim list` with `resource_prefix: "repo-lane:<repo>/"`
/// (the R-0067-a example shape),
/// THEN the response includes both same-repo leases and excludes the
/// different-repo one. *(§API Contract `list` request shape)*
///
/// RED against this branch's HEAD: `list` is unrecognized — the call errors
/// `INVALID_PARAMS`, so `list_leases_array` returns `None` and the first
/// assertion fails guarantee-absent.
#[tokio::test]
async fn list_resource_prefix_filter_returns_only_matching_prefix() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let token = seed_admin_token(&pool, Uuid::new_v4()).await;
    let role_instance = format!("list-prefix-filter-{}", Uuid::new_v4());
    let repo = format!("proj-{}", Uuid::new_v4());
    let other_repo = format!("other-{}", Uuid::new_v4());
    let resource_a = format!("repo-lane:{repo}/lane-a");
    let resource_b = format!("repo-lane:{repo}/lane-b");
    let resource_other = format!("repo-lane:{other_repo}/lane-a");
    let prefix = format!("repo-lane:{repo}/");

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;
    acquire_lease_for_setup(&client, token.as_str(), &resource_a).await;
    acquire_lease_for_setup(&client, token.as_str(), &resource_b).await;
    acquire_lease_for_setup(&client, token.as_str(), &resource_other).await;

    let res = client
        .call_tool(claim_list_params(token.as_str(), None, Some(&prefix)))
        .await;

    let leases = list_leases_array(&res);
    assert!(
        leases.is_some(),
        "§API Contract: `claim list {{resource_prefix: \"{prefix}\"}}` must return the \
         `{{ leases: [...] }}` response; `list` is unrecognized at this branch's HEAD so the \
         call errors instead. Got: {res:?}"
    );
    let leases = leases.unwrap();

    assert!(
        leases_array_contains_resource(&leases, &resource_a),
        "the `resource_prefix: \"{prefix}\"` filter must include `{resource_a}`. \
         leases={leases:?}"
    );
    assert!(
        leases_array_contains_resource(&leases, &resource_b),
        "the `resource_prefix: \"{prefix}\"` filter must include `{resource_b}`. \
         leases={leases:?}"
    );
    assert!(
        !leases_array_contains_resource(&leases, &resource_other),
        "the `resource_prefix: \"{prefix}\"` filter must EXCLUDE `{resource_other}` (a \
         different repo). leases={leases:?}"
    );

    server.abort();
}

// ===========================================================================
// b2 AC5 — `family=actor` is refused `reserved_family` (R-0067-c)
// ===========================================================================

/// GIVEN an attached actor,
/// WHEN it calls `claim list` with `family: "actor"` (the reserved family,
/// barred from the entire `claim` surface),
/// THEN it is refused the DEDICATED `reserved_family` code — NOT
/// `invalid_resource` (the family token IS well-formed and in the closed
/// set; it is RESERVED, a distinct refusal per R-0067-c's explicit
/// distinctness requirement). *(R-0067-c)*
///
/// RED against this branch's HEAD: `list` is unrecognized (see file header
/// addendum) — the call errors `INVALID_PARAMS`; `reserved_family` is
/// entirely absent.
#[tokio::test]
async fn list_family_actor_is_refused_reserved_family() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let token = seed_admin_token(&pool, Uuid::new_v4()).await;
    let role_instance = format!("list-family-actor-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    let res = client
        .call_tool(claim_list_params(token.as_str(), Some("actor"), None))
        .await;

    assert!(
        result_surfaces_code(&res, "reserved_family"),
        "R-0067-c: `claim list {{family: \"actor\"}}` must be refused the dedicated \
         `reserved_family` code. Against `list`'s unrecognized-action state at this branch's \
         HEAD the code is absent (`INVALID_PARAMS` instead). Got: {res:?}"
    );
    assert!(
        !result_surfaces_code(&res, "invalid_resource"),
        "R-0067-c: a reserved-family refusal must be DISTINCT from `invalid_resource`. \
         Got: {res:?}"
    );

    server.abort();
}

// ===========================================================================
// b2 AC6 — not_attached (R-0064-e)
// ===========================================================================

/// GIVEN a fresh session that has NEVER bound via `message poll`,
/// WHEN it calls `claim list`,
/// THEN it is refused `not_attached` — `list` is a coordination action like
/// any other and executes under a resolved attachment (R-0064-e's bind
/// requirement; R-0073-b's own premise states it explicitly: "every
/// coordination action — list included — executes under a resolved
/// attachment"). *(R-0064-e)*
///
/// RED against this branch's HEAD: `list` is unrecognized — the call errors
/// `INVALID_PARAMS` regardless of attachment state, so `not_attached` is
/// entirely absent.
#[tokio::test]
async fn list_without_attachment_is_refused_not_attached() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let token = seed_admin_token(&pool, Uuid::new_v4()).await;

    let (server, client) = coordination_server(&pool).await;

    // No `poll` call — this session never attaches.
    let res = client
        .call_tool(claim_list_params(token.as_str(), None, None))
        .await;

    assert!(
        result_surfaces_code(&res, "not_attached"),
        "R-0064-e: `claim list` from a session with no live attachment must be refused \
         `not_attached`. Against `list`'s unrecognized-action state at this branch's HEAD the \
         code is absent (`INVALID_PARAMS` instead). Got: {res:?}"
    );

    server.abort();
}

// ===========================================================================
// b2 AC7 — read_observer denied pre-dispatch (R-0073-b)
// ===========================================================================

/// GIVEN a `read_observer`-scoped token (never attached — attach is itself
/// write-category, so a read_observer cannot bind either),
/// WHEN it calls `claim list`,
/// THEN the call is denied at the host-fn boundary with
/// `PERMISSION_DENIED_CODE` (`-4002`) — an AUTHORIZATION error, never a
/// claim-body refusal `reason_code`; the coordination body is never reached.
/// *(R-0073-b: "list included" is the spec's own explicit phrase)*
///
/// # A GENUINE red this time — not a coincidental green (contrast with b1's AC8)
///
/// Unlike b1's `read_observer_denied_pre_dispatch_for_claim_acquire` (which
/// passed against the parent commit for the WRONG reason — the generic
/// plugin-dispatch fallback happened to also deny with `-4002` before
/// `acquire` was routed at all), this scenario does NOT coincidentally pass
/// today. `claim` IS routed now (b1 merged) and `handle_coordination`'s
/// contract order is authenticate → **`parse_action`** → per-action gate
/// (`authorize_coordination_action`) → route (confirmed by reading the
/// current `mcp/server.rs` source, this file's established black-box-
/// adjacent observation convention). Because `(CLAIM_TOOL, "list")` fails to
/// PARSE (see file header addendum), the call errors `INVALID_PARAMS` at the
/// parse step — BEFORE `authorize_coordination_action` ever runs, for ANY
/// token role, admin or read_observer alike. So this scenario is guarantee-
/// absent exactly like the others: the assertion below pins the INTENDED
/// mechanism (`PERMISSION_DENIED_CODE`, the coordination gate), which is
/// false today (`INVALID_PARAMS` instead) and becomes true only once `list`
/// is parsed AND the per-action gate denies it.
#[tokio::test]
async fn read_observer_denied_pre_dispatch_for_claim_list() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let ro_token = seed_read_observer_token(&pool, Uuid::new_v4()).await;

    let (server, client) = coordination_server(&pool).await;

    let res = client
        .call_tool(claim_list_params(ro_token.as_str(), None, None))
        .await;

    let err = res.expect_err(
        "R-0073-b: a `read_observer` token calling `claim list` must be denied at the host-fn \
         boundary (Err), never receive a claim-body refusal or success (Ok).",
    );

    match err {
        rmcp::ServiceError::McpError(ref error_data) => {
            assert_eq!(
                error_data.code, PERMISSION_DENIED_CODE,
                "R-0073-b: a `read_observer` calling `claim list` must be denied with \
                 PERMISSION_DENIED_CODE ({PERMISSION_DENIED_CODE:?}); got {:?}. At this branch's \
                 HEAD `list` fails to parse before the per-action gate runs, so today's actual \
                 code is `INVALID_PARAMS` — see this test's doc comment.",
                error_data.code
            );
        }
        other => panic!(
            "R-0073-b: expected an `rmcp::ServiceError::McpError` carrying PERMISSION_DENIED_CODE; \
             got a different error variant: {other:?}"
        ),
    }

    server.abort();
}

// ===========================================================================
// b2 AC8a — op-log on a successful list (R-0075-a)
// ===========================================================================

/// GIVEN an attached session that has acquired one lease,
/// WHEN it calls `claim list` successfully,
/// THEN a coordination op-log entry SPECIFIC TO THE LIST OPERATION is
/// emitted on the unified `tracing` stream — a single line carrying BOTH
/// `op=ClaimList` (the `CoordinationOp::ClaimList` Debug-formatted `op = ?op`
/// field, following the exact convention this file's b1 AC9 established for
/// `op=Acquire`) AND the resolved actor's id (R-0075-a "actor attribution").
/// *(R-0075-a)*
///
/// # Op-log fixture — already exists, reused verbatim (see file header addendum)
///
/// `successful_acquire_emits_op_log_entry` (b1, AC9, above) already
/// establishes the `#[tracing_test::traced_test]` + `logs_contain`/
/// `logs_assert` capture mechanism this test reuses — no new observation
/// seam is needed for `list`'s op-log AC.
///
/// # Non-vacuity discipline
///
/// `op=ClaimList` cannot appear in ANY log line before green: no
/// `CoordinationOp::ClaimList` variant exists in `coordination::mod` at this
/// branch's HEAD (confirmed by reading the enum), so the string is not
/// merely absent today, it is UNPRODUCIBLE — a strictly stronger guarantee
/// than b1 AC9 needed (which had to rule out a co-occurring `AttachBind`
/// line by pairing two conditions on one line). The pairing with actor
/// attribution below is kept anyway, both for convention consistency and
/// because it is itself part of the R-0075-a contract under test, not merely
/// a vacuity guard here.
///
/// RED against this branch's HEAD: `list` never dispatches (see file header
/// addendum) — no coordination body runs for it at all, so no line ever
/// carries `op=ClaimList`.
#[traced_test]
#[tokio::test]
async fn successful_list_emits_op_log_entry() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let token = seed_admin_token(&pool, Uuid::new_v4()).await;
    let role_instance = format!("list-oplog-{}", Uuid::new_v4());
    let resource = format!("surface:list-oplog-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    let actor_id = attach_session(&client, token.as_str(), &role_instance).await;
    acquire_lease_for_setup(&client, token.as_str(), &resource).await;

    // Capture-liveness canary (mirrors b1 AC9 / `startup_run_full.rs`).
    tracing::info!("coordination_leases list oplog canary");
    assert!(
        logs_contain("coordination_leases list oplog canary"),
        "the traced_test capture channel must be live before the op-log assertion is trusted"
    );

    let res = client
        .call_tool(claim_list_params(token.as_str(), None, None))
        .await;

    let actor_id_str = actor_id.to_string();
    logs_assert(|lines: &[&str]| {
        let hit = lines
            .iter()
            .any(|line| line.contains("op=ClaimList") && line.contains(&actor_id_str));
        if hit {
            Ok(())
        } else {
            let coordination_lines: Vec<&&str> = lines
                .iter()
                .filter(|l| l.contains("mnemra::coordination"))
                .collect();
            Err(format!(
                "R-0075-a: a successful `claim list` must emit ONE op-log line carrying BOTH \
                 `op=ClaimList` AND the resolved actor's id ({actor_id_str}, actor attribution). \
                 Absent at this branch's HEAD — `list` never dispatches (unrecognized action), \
                 so no line ever carries `op=ClaimList`. list result: {res:?}. captured \
                 `mnemra::coordination` lines: {coordination_lines:?}"
            ))
        }
    });

    server.abort();
}

// ===========================================================================
// b2 AC8b — op-log on a refused list, reason code included (R-0075-a)
// ===========================================================================

/// GIVEN an attached session,
/// WHEN it calls `claim list` with `family: "actor"` (refused
/// `reserved_family` — AC5),
/// THEN a coordination op-log entry is emitted carrying BOTH `op=ClaimList`
/// AND the reason code `reserved_family` on the SAME line — R-0075-a's
/// explicit "refusals SHALL be logged... with their machine-readable reason
/// code". *(R-0075-a)*
///
/// RED against this branch's HEAD: `list` never dispatches (see file header
/// addendum) — no coordination body runs for it at all, so no line ever
/// carries `op=ClaimList`, and `reserved_family` (AC5) is itself absent
/// today too.
#[traced_test]
#[tokio::test]
async fn refused_list_emits_op_log_entry_with_reason_code() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let token = seed_admin_token(&pool, Uuid::new_v4()).await;
    let role_instance = format!("list-oplog-refused-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    tracing::info!("coordination_leases list refused oplog canary");
    assert!(
        logs_contain("coordination_leases list refused oplog canary"),
        "the traced_test capture channel must be live before the op-log assertion is trusted"
    );

    let res = client
        .call_tool(claim_list_params(token.as_str(), Some("actor"), None))
        .await;

    logs_assert(|lines: &[&str]| {
        let hit = lines
            .iter()
            .any(|line| line.contains("op=ClaimList") && line.contains("reserved_family"));
        if hit {
            Ok(())
        } else {
            let coordination_lines: Vec<&&str> = lines
                .iter()
                .filter(|l| l.contains("mnemra::coordination"))
                .collect();
            Err(format!(
                "R-0075-a: a refused `claim list` (family=actor → reserved_family) must emit \
                 ONE op-log line carrying BOTH `op=ClaimList` AND the reason code \
                 `reserved_family`. Absent at this branch's HEAD — `list` never dispatches, so \
                 no line ever carries `op=ClaimList`. list result: {res:?}. captured \
                 `mnemra::coordination` lines: {coordination_lines:?}"
            ))
        }
    });

    server.abort();
}

// ===========================================================================
// c helpers — `claim renew` / `claim release` (Task 5 c, R-0065-d/R-0067-c)
// ===========================================================================

/// Build a `claim renew` or `claim release` tool call — the request names a
/// `lease_id`, not a `resource` (§API Contract). Mirrors `claim_params`'s /
/// `claim_list_params`'s builder-per-action convention above.
fn claim_lease_action_params(
    token_str: &str,
    action: &str,
    lease_id: Uuid,
) -> CallToolRequestParams {
    let mut params = CallToolRequestParams::new("claim");
    params.meta = Some(token_meta(token_str));
    params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("action".to_owned(), json!(action));
        m.insert("lease_id".to_owned(), json!(lease_id.to_string()));
        m
    });
    params
}

/// The caller's own live `actor:`-family lease id (the attachment
/// realization's own row, R-0064-f) — obtained via the sanctioned direct-SQL
/// operator surface per R-0067-c's acceptance-test parenthetical ("obtained
/// via the operator SQL surface in the fixture"). Panics (a precondition
/// failure, not a scenario assertion) if no such row is live — every
/// scenario using this attaches first via `attach_session`.
async fn own_actor_lease_id(pool: &sqlx::PgPool, workspace_id: Uuid, actor_id: Uuid) -> Uuid {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM leases
         WHERE workspace_id = $1 AND holder_actor_id = $2
           AND resource LIKE 'actor:%' AND terminal_state IS NULL",
    )
    .bind(workspace_id)
    .bind(actor_id)
    .fetch_optional(pool)
    .await
    .expect("own-actor-lease-id query must execute");
    row.map(|(id,)| id).unwrap_or_else(|| {
        panic!(
            "precondition: attaching must create exactly one live `actor:`-family lease row \
             (R-0064-f) for actor {actor_id} in workspace {workspace_id}; found none."
        )
    })
}

/// Force `lease_id` into the past (`expires_at = now() - 1s`) via direct SQL
/// — the deterministic no-wall-sleep expiry technique this file's c
/// addendum documents, so R-0065-e's store-clock predicate (`now >=
/// expires_at`) fires without a flake-prone real-time wait.
async fn force_expire_lease(pool: &sqlx::PgPool, lease_id: Uuid) {
    sqlx::query("UPDATE leases SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(lease_id)
        .execute(pool)
        .await
        .expect("force-expire UPDATE must execute");
}

// ===========================================================================
// c AC1 — non-holder renew on a LIVE lease is refused `not_holder` (R-0065-d)
// ===========================================================================

/// GIVEN actor A holds a live lease and actor B is attached but does NOT
/// hold it,
/// WHEN B calls `claim renew` naming A's `lease_id`,
/// THEN B is refused the dedicated `not_holder` code — NOT `lease_not_found`
/// (the lease IS found and IS live; the only thing wrong is identity) — and
/// A's lease is left live and untouched. *(R-0065-d; ordering landmine:
/// isolates the identity axis from the liveness axis)*
///
/// RED against this branch's HEAD: `(CLAIM_TOOL, "renew")` is unrecognized
/// by `parse_action` (see this file's c addendum) — B's call errors
/// `INVALID_PARAMS` before any coordination body runs, so `not_holder` is
/// entirely absent.
#[tokio::test]
async fn renew_by_non_holder_on_live_lease_is_refused_not_holder() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token_a = seed_admin_token(&pool, workspace_id).await;
    let token_b = seed_admin_token(&pool, workspace_id).await;
    let role_a = format!("renew-holder-{}", Uuid::new_v4());
    let role_b = format!("renew-nonholder-{}", Uuid::new_v4());

    let (server_a, client_a) = coordination_server(&pool).await;
    let (server_b, client_b) = coordination_server(&pool).await;
    attach_session(&client_a, token_a.as_str(), &role_a).await;
    attach_session(&client_b, token_b.as_str(), &role_b).await;

    let resource = format!("repo-lane:mnemra/nonholder-renew-{}", Uuid::new_v4());
    let acquired = acquire_lease_for_setup(&client_a, token_a.as_str(), &resource).await;
    let lease_id = acquired
        .get("lease_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("precondition: acquire response must carry a valid `lease_id`");

    let res = client_b
        .call_tool(claim_lease_action_params(
            token_b.as_str(),
            "renew",
            lease_id,
        ))
        .await;

    assert!(
        result_surfaces_code(&res, "not_holder"),
        "R-0065-d: a non-holder `renew` on a LIVE lease must be refused `not_holder`. Against \
         the unrecognized `renew` action at this branch's HEAD the code is absent \
         (`INVALID_PARAMS` instead). Got: {res:?}"
    );
    assert!(
        !result_surfaces_code(&res, "lease_not_found"),
        "R-0065-d ordering landmine: the lease IS found and IS live — a non-holder renewing a \
         LIVE lease must NOT be conflated with `lease_not_found`. Got: {res:?}"
    );

    let live = live_lease_count(&pool, workspace_id, &resource).await;
    assert_eq!(
        live, 1,
        "a `not_holder` refusal must leave A's lease live and untouched; found {live} for \
         `{resource}`."
    );

    server_a.abort();
    server_b.abort();
}

// ===========================================================================
// c AC2 — non-holder release on a LIVE lease is refused `not_holder` (R-0065-d)
// ===========================================================================

/// GIVEN actor A holds a live lease and actor B is attached but does NOT
/// hold it,
/// WHEN B calls `claim release` naming A's `lease_id`,
/// THEN B is refused `not_holder` and A's lease remains live (non-terminal —
/// B's release attempt must not release SOMEONE ELSE's lease). *(R-0065-d)*
///
/// RED against this branch's HEAD: `(CLAIM_TOOL, "release")` is unrecognized
/// — B's call errors `INVALID_PARAMS`; `not_holder` is absent.
#[tokio::test]
async fn release_by_non_holder_on_live_lease_is_refused_not_holder() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token_a = seed_admin_token(&pool, workspace_id).await;
    let token_b = seed_admin_token(&pool, workspace_id).await;
    let role_a = format!("release-holder-{}", Uuid::new_v4());
    let role_b = format!("release-nonholder-{}", Uuid::new_v4());

    let (server_a, client_a) = coordination_server(&pool).await;
    let (server_b, client_b) = coordination_server(&pool).await;
    attach_session(&client_a, token_a.as_str(), &role_a).await;
    attach_session(&client_b, token_b.as_str(), &role_b).await;

    let resource = format!("repo-lane:mnemra/nonholder-release-{}", Uuid::new_v4());
    let acquired = acquire_lease_for_setup(&client_a, token_a.as_str(), &resource).await;
    let lease_id = acquired
        .get("lease_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("precondition: acquire response must carry a valid `lease_id`");

    let res = client_b
        .call_tool(claim_lease_action_params(
            token_b.as_str(),
            "release",
            lease_id,
        ))
        .await;

    assert!(
        result_surfaces_code(&res, "not_holder"),
        "R-0065-d: a non-holder `release` on a LIVE lease must be refused `not_holder`. Against \
         the unrecognized `release` action at this branch's HEAD the code is absent \
         (`INVALID_PARAMS` instead). Got: {res:?}"
    );
    assert!(
        !result_surfaces_code(&res, "lease_not_found"),
        "R-0065-d ordering landmine: the lease IS found and IS live — a non-holder releasing a \
         LIVE lease must NOT be conflated with `lease_not_found`. Got: {res:?}"
    );

    let live = live_lease_count(&pool, workspace_id, &resource).await;
    assert_eq!(
        live, 1,
        "a `not_holder` refusal must leave A's lease live (NOT released); found {live} for \
         `{resource}`."
    );

    server_a.abort();
    server_b.abort();
}

// ===========================================================================
// c AC3 — holder renew moves `expires_at` forward (R-0065-d)
// ===========================================================================

/// GIVEN an attached actor holding its own live lease,
/// WHEN the HOLDER calls `claim renew` naming its own `lease_id`,
/// THEN the call succeeds, the response names the SAME `lease_id`, its
/// `expires_at` is STRICTLY LATER than the lease's original `expires_at`
/// (renewal extends from the renewal moment by the lease's duration —
/// R-0065-d), and the `leases` row reflects the same advanced value.
/// *(R-0065-d)*
///
/// RED against this branch's HEAD: `renew` is unrecognized — the call
/// errors `INVALID_PARAMS`, so `claim_structured_obj` returns `None` and the
/// first assertion fails guarantee-absent before any timestamp comparison
/// runs.
#[tokio::test]
async fn holder_renew_moves_expiry_forward() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("renew-holder-self-{}", Uuid::new_v4());
    let resource = format!("repo-lane:mnemra/holder-renew-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    let acquired = acquire_lease_for_setup(&client, token.as_str(), &resource).await;
    let lease_id = acquired
        .get("lease_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("precondition: acquire response must carry a valid `lease_id`");
    let original_expires_at = acquired
        .get("expires_at")
        .and_then(|v| v.as_str())
        .map(parse_rfc3339)
        .expect("precondition: acquire response must carry `expires_at`");

    // A real-time gap long enough that a renewal "from the renewal moment"
    // is guaranteed distinguishable from the original value even at
    // second-granularity RFC3339 timestamps (floor(x+d) - floor(x) >= d - 1,
    // so d > 2s guarantees a strictly later whole-second value regardless of
    // where `original_expires_at`'s fractional part fell). Not a wait for
    // expiry — the lease is acquired with a 3600s duration (via
    // `acquire_lease_for_setup`) and never comes close to expiring here.
    tokio::time::sleep(std::time::Duration::from_millis(2200)).await;

    let res = client
        .call_tool(claim_lease_action_params(token.as_str(), "renew", lease_id))
        .await;

    let obj = claim_structured_obj(&res);
    assert!(
        obj.is_some(),
        "R-0065-d: the HOLDER's `renew` on its own live lease must succeed; `renew` is \
         unrecognized at this branch's HEAD so the call errors instead. Got: {res:?}"
    );
    let obj = obj.unwrap();

    assert_eq!(
        obj.get("lease_id").and_then(|v| v.as_str()),
        Some(lease_id.to_string().as_str()),
        "§API Contract: the renew response must name the SAME `lease_id`; got {obj:?}"
    );

    let renewed_expires_at = obj
        .get("expires_at")
        .and_then(|v| v.as_str())
        .map(parse_rfc3339)
        .expect("§API Contract: the renew response must carry a string `expires_at`");
    assert!(
        renewed_expires_at > original_expires_at,
        "R-0065-d: renewal must extend `expires_at` STRICTLY forward from its original value; \
         original={original_expires_at} renewed={renewed_expires_at}"
    );

    let row = live_lease_row(&pool, workspace_id, &resource).await;
    assert!(
        row.is_some(),
        "the renewed lease must remain a live row; found none for `{resource}`."
    );
    let (_id, _holder, _acquired_epoch, db_expires_epoch) = row.unwrap();
    assert!(
        (db_expires_epoch - renewed_expires_at.timestamp() as f64).abs() < 2.0,
        "the `leases` row's `expires_at` must match the renew response's value; \
         response={renewed_expires_at} db_epoch={db_expires_epoch}"
    );

    server.abort();
}

// ===========================================================================
// c AC4 — renew on an expired (untaken) lease is refused `lease_not_found`
// (R-0065-d, R-0065-e)
// ===========================================================================

/// GIVEN an attached actor holding a lease that has been forced expired (via
/// direct SQL — R-0065-e's store-clock predicate; no takeover has happened,
/// the row is still non-terminal, merely past its `expires_at`),
/// WHEN the SAME actor (still the holder) calls `claim renew` naming it,
/// THEN it is refused `lease_not_found` — an expired hold is not revivable;
/// the path back is the explicit, audited `takeover` (R-0065-d). Isolates
/// the liveness axis: the caller IS the holder, so a `not_holder` result
/// would be wrong. *(R-0065-d, R-0065-e)*
///
/// RED against this branch's HEAD: `renew` is unrecognized — the call
/// errors `INVALID_PARAMS`; `lease_not_found` is absent.
#[tokio::test]
async fn renew_on_expired_untaken_lease_is_refused_lease_not_found() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("renew-expired-{}", Uuid::new_v4());
    let resource = format!("repo-lane:mnemra/renew-expired-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    let acquired = acquire_lease_for_setup(&client, token.as_str(), &resource).await;
    let lease_id = acquired
        .get("lease_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("precondition: acquire response must carry a valid `lease_id`");

    force_expire_lease(&pool, lease_id).await;

    let res = client
        .call_tool(claim_lease_action_params(token.as_str(), "renew", lease_id))
        .await;

    assert!(
        result_surfaces_code(&res, "lease_not_found"),
        "R-0065-d/-e: `renew` on an expired-but-untaken lease must be refused `lease_not_found` \
         — an expired hold is not revivable. Against the unrecognized `renew` action at this \
         branch's HEAD the code is absent (`INVALID_PARAMS` instead). Got: {res:?}"
    );
    assert!(
        !result_surfaces_code(&res, "not_holder"),
        "R-0065-d ordering landmine: the caller IS the holder — an expired-lease renew must NOT \
         be conflated with `not_holder`. Got: {res:?}"
    );

    server.abort();
}

// ===========================================================================
// c AC5 — release then release again is refused `lease_not_found` (R-0065-d)
// ===========================================================================

/// GIVEN an attached actor holding a live lease,
/// WHEN it `release`s that lease successfully, then calls `release` a SECOND
/// time naming the SAME `lease_id`,
/// THEN the first call succeeds (`released: true`, and the lease is no
/// longer a live row) and the second is refused `lease_not_found` — a
/// released lease is terminal, so it is "not found" for the liveness check
/// exactly like an expired one. *(R-0065-d)*
///
/// RED against this branch's HEAD: `release` is unrecognized — the FIRST
/// call already errors `INVALID_PARAMS`, so `claim_structured_obj` returns
/// `None` and the first assertion fails guarantee-absent before the second
/// call is even attempted.
#[tokio::test]
async fn release_then_release_again_is_refused_lease_not_found() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("release-twice-{}", Uuid::new_v4());
    let resource = format!("repo-lane:mnemra/release-twice-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    let acquired = acquire_lease_for_setup(&client, token.as_str(), &resource).await;
    let lease_id = acquired
        .get("lease_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("precondition: acquire response must carry a valid `lease_id`");

    let first = client
        .call_tool(claim_lease_action_params(
            token.as_str(),
            "release",
            lease_id,
        ))
        .await;
    let first_obj = claim_structured_obj(&first);
    assert!(
        first_obj.is_some(),
        "R-0065-d: the FIRST `release` of a held live lease must succeed; `release` is \
         unrecognized at this branch's HEAD so the call errors instead. Got: {first:?}"
    );
    let first_obj = first_obj.unwrap();
    assert_eq!(
        first_obj.get("lease_id").and_then(|v| v.as_str()),
        Some(lease_id.to_string().as_str()),
        "§API Contract: the release response must name the SAME `lease_id`; got {first_obj:?}"
    );
    assert_eq!(
        first_obj.get("released").and_then(|v| v.as_bool()),
        Some(true),
        "§API Contract: a successful release response must carry `released: true`; got \
         {first_obj:?}"
    );

    let live_after_first = live_lease_count(&pool, workspace_id, &resource).await;
    assert_eq!(
        live_after_first, 0,
        "the first release must terminate the lease (no longer a LIVE row); found \
         {live_after_first} for `{resource}`."
    );

    let second = client
        .call_tool(claim_lease_action_params(
            token.as_str(),
            "release",
            lease_id,
        ))
        .await;
    assert!(
        result_surfaces_code(&second, "lease_not_found"),
        "R-0065-d: a SECOND `release` naming an already-released `lease_id` must be refused \
         `lease_not_found`. Got: {second:?}"
    );

    server.abort();
}

// ===========================================================================
// c AC6 — renew after the holder's own release is refused `lease_not_found`
// (R-0065-d)
// ===========================================================================

/// GIVEN an attached actor holding a live lease,
/// WHEN it `release`s that lease successfully, then calls `renew` naming the
/// SAME `lease_id`,
/// THEN the release succeeds and the subsequent renew is refused
/// `lease_not_found` — a released lease is terminal, not renewable; the
/// caller IS the (former) holder, so a `not_holder` result would be wrong.
/// *(R-0065-d)*
///
/// RED against this branch's HEAD: BOTH `release` and `renew` are
/// unrecognized — the release call already errors `INVALID_PARAMS`, so the
/// first assertion fails guarantee-absent before renew is even attempted.
#[tokio::test]
async fn renew_after_holders_own_release_is_refused_lease_not_found() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let token = seed_admin_token(&pool, Uuid::new_v4()).await;
    let role_instance = format!("renew-after-release-{}", Uuid::new_v4());
    let resource = format!("repo-lane:mnemra/renew-after-release-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    let acquired = acquire_lease_for_setup(&client, token.as_str(), &resource).await;
    let lease_id = acquired
        .get("lease_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("precondition: acquire response must carry a valid `lease_id`");

    let release_res = client
        .call_tool(claim_lease_action_params(
            token.as_str(),
            "release",
            lease_id,
        ))
        .await;
    let release_obj = claim_structured_obj(&release_res);
    assert!(
        release_obj.is_some(),
        "precondition: `release` of a held live lease must succeed before this test's real \
         assertion (the subsequent renew); `release` is unrecognized at this branch's HEAD so \
         the call errors instead. Got: {release_res:?}"
    );
    assert_eq!(
        release_obj
            .unwrap()
            .get("released")
            .and_then(|v| v.as_bool()),
        Some(true),
        "precondition: the release response must carry `released: true`"
    );

    let renew_res = client
        .call_tool(claim_lease_action_params(token.as_str(), "renew", lease_id))
        .await;
    assert!(
        result_surfaces_code(&renew_res, "lease_not_found"),
        "R-0065-d: `renew` naming a `lease_id` the SAME actor already released must be refused \
         `lease_not_found`. Against the unrecognized `renew` action at this branch's HEAD the \
         code is absent (`INVALID_PARAMS` instead). Got: {renew_res:?}"
    );
    assert!(
        !result_surfaces_code(&renew_res, "not_holder"),
        "R-0065-d ordering landmine: the caller IS the (former) holder — a renew after one's \
         own release must NOT be conflated with `not_holder`. Got: {renew_res:?}"
    );

    server.abort();
}

// ===========================================================================
// c AC7 — renew naming a fabricated `lease_id` is refused `lease_not_found`
// (R-0065-d)
// ===========================================================================

/// GIVEN an attached actor,
/// WHEN it calls `claim renew` naming a `lease_id` that never existed (a
/// fresh random UUID),
/// THEN it is refused `lease_not_found`. *(R-0065-d)*
///
/// RED against this branch's HEAD: `renew` is unrecognized — the call
/// errors `INVALID_PARAMS`; `lease_not_found` is absent.
#[tokio::test]
async fn renew_with_fabricated_lease_id_is_refused_lease_not_found() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let token = seed_admin_token(&pool, Uuid::new_v4()).await;
    let role_instance = format!("renew-fabricated-{}", Uuid::new_v4());
    let fabricated_lease_id = Uuid::new_v4();

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    let res = client
        .call_tool(claim_lease_action_params(
            token.as_str(),
            "renew",
            fabricated_lease_id,
        ))
        .await;

    assert!(
        result_surfaces_code(&res, "lease_not_found"),
        "R-0065-d: `renew` naming a `lease_id` that never existed must be refused \
         `lease_not_found`. Against the unrecognized `renew` action at this branch's HEAD the \
         code is absent (`INVALID_PARAMS` instead). Got: {res:?}"
    );

    server.abort();
}

// ===========================================================================
// c AC8 — release naming a fabricated `lease_id` is refused `lease_not_found`
// (R-0065-d)
// ===========================================================================

/// GIVEN an attached actor,
/// WHEN it calls `claim release` naming a `lease_id` that never existed,
/// THEN it is refused `lease_not_found`. *(R-0065-d)*
///
/// RED against this branch's HEAD: `release` is unrecognized — the call
/// errors `INVALID_PARAMS`; `lease_not_found` is absent.
#[tokio::test]
async fn release_with_fabricated_lease_id_is_refused_lease_not_found() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let token = seed_admin_token(&pool, Uuid::new_v4()).await;
    let role_instance = format!("release-fabricated-{}", Uuid::new_v4());
    let fabricated_lease_id = Uuid::new_v4();

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    let res = client
        .call_tool(claim_lease_action_params(
            token.as_str(),
            "release",
            fabricated_lease_id,
        ))
        .await;

    assert!(
        result_surfaces_code(&res, "lease_not_found"),
        "R-0065-d: `release` naming a `lease_id` that never existed must be refused \
         `lease_not_found`. Against the unrecognized `release` action at this branch's HEAD the \
         code is absent (`INVALID_PARAMS` instead). Got: {res:?}"
    );

    server.abort();
}

// ===========================================================================
// c AC9 — renew/release naming an attachment `actor:` lease id are refused
// `reserved_family` (R-0067-c, the resolved-lease-row arm)
// ===========================================================================

/// GIVEN an attached actor (attaching creates a live `actor:`-family lease
/// row under the R-0064-f attachment-as-lease realization — confirmed live
/// on this branch, same precondition sanity b2 AC2 established),
/// WHEN it calls `claim renew` and separately `claim release`, each naming
/// its OWN attachment row's `lease_id` (obtained via the sanctioned direct-
/// SQL operator surface — R-0067-c's own acceptance-test parenthetical),
/// THEN each is refused the DEDICATED `reserved_family` code — NOT
/// `not_holder` (the caller genuinely IS `holder_actor_id` on that row),
/// because the reserved-family bar is checked BEFORE the holder/liveness
/// checks (build plan §3.2 step 3, ahead of steps 4-5). The attachment row
/// itself is left untouched — a `renew`/`release` refusal creates no side
/// effect on the identity substrate. *(R-0067-c)*
///
/// RED against this branch's HEAD: both actions are unrecognized —
/// `reserved_family` is absent for either call.
#[tokio::test]
async fn renew_and_release_naming_an_attachment_lease_id_are_refused_reserved_family() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("reserved-family-renew-release-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    let actor_id = attach_session(&client, token.as_str(), &role_instance).await;

    let attachment_lease_id = own_actor_lease_id(&pool, workspace_id, actor_id).await;

    let renew_res = client
        .call_tool(claim_lease_action_params(
            token.as_str(),
            "renew",
            attachment_lease_id,
        ))
        .await;
    assert!(
        result_surfaces_code(&renew_res, "reserved_family"),
        "R-0067-c: `renew` naming an attachment row's `lease_id` must be refused the dedicated \
         `reserved_family` code. Against the unrecognized `renew` action at this branch's HEAD \
         the code is absent (`INVALID_PARAMS` instead). Got: {renew_res:?}"
    );
    assert!(
        !result_surfaces_code(&renew_res, "not_holder"),
        "R-0067-c: the caller genuinely IS `holder_actor_id` on its own attachment row — the \
         reserved-family bar must fire BEFORE the holder check, not be conflated with \
         `not_holder`. Got: {renew_res:?}"
    );

    let release_res = client
        .call_tool(claim_lease_action_params(
            token.as_str(),
            "release",
            attachment_lease_id,
        ))
        .await;
    assert!(
        result_surfaces_code(&release_res, "reserved_family"),
        "R-0067-c: `release` naming an attachment row's `lease_id` must ALSO be refused \
         `reserved_family`. Got: {release_res:?}"
    );

    // The attachment row itself is left untouched by either refusal — same
    // id, still live.
    let still_live = own_actor_lease_id(&pool, workspace_id, actor_id).await;
    assert_eq!(
        still_live, attachment_lease_id,
        "a `reserved_family` refusal on renew/release must leave the attachment row untouched \
         (same id, still live) — the identity substrate must be unaffected by claim-surface \
         probes against it."
    );

    server.abort();
}

// ===========================================================================
// c AC10 — renew/release without attachment are refused `not_attached`
// (R-0064-e)
// ===========================================================================

/// GIVEN a fresh session that has NEVER bound via `message poll`,
/// WHEN it calls `claim renew` and separately `claim release`, each naming
/// an arbitrary (fabricated) `lease_id`,
/// THEN each is refused `not_attached` — the attachment gate resolves
/// BEFORE the lease lookup (build plan §3.2 step 1, ahead of step 2), so an
/// unattached caller never reaches the found/live/holder checks regardless
/// of what `lease_id` it names. *(R-0064-e)*
///
/// RED against this branch's HEAD: both actions are unrecognized —
/// `not_attached` is absent for either call (the generic `INVALID_PARAMS`
/// fires regardless of attachment state, exactly like b2 AC6's `list`
/// precedent).
#[tokio::test]
async fn renew_and_release_without_attachment_are_refused_not_attached() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let token = seed_admin_token(&pool, Uuid::new_v4()).await;
    let fabricated_lease_id = Uuid::new_v4();

    let (server, client) = coordination_server(&pool).await;

    // No `poll` call — this session never attaches.
    let renew_res = client
        .call_tool(claim_lease_action_params(
            token.as_str(),
            "renew",
            fabricated_lease_id,
        ))
        .await;
    assert!(
        result_surfaces_code(&renew_res, "not_attached"),
        "R-0064-e: `claim renew` from a session with no live attachment must be refused \
         `not_attached`, regardless of the named `lease_id`. Against the unrecognized `renew` \
         action at this branch's HEAD the code is absent (`INVALID_PARAMS` instead). \
         Got: {renew_res:?}"
    );

    let release_res = client
        .call_tool(claim_lease_action_params(
            token.as_str(),
            "release",
            fabricated_lease_id,
        ))
        .await;
    assert!(
        result_surfaces_code(&release_res, "not_attached"),
        "R-0064-e: `claim release` from a session with no live attachment must ALSO be refused \
         `not_attached`. Got: {release_res:?}"
    );

    server.abort();
}

// ===========================================================================
// d helpers — `claim takeover` (Task 5 d, R-0066/R-0067-c)
// ===========================================================================

/// The `taken_over` (terminal, superseded) lease row for `(workspace_id,
/// resource)`, if any: `(lease_id, holder_actor_id, terminated_at IS NOT
/// NULL, superseded_by)`. Mirrors `coordination_session_plane.rs`'s
/// `taken_over_attachment_lease`, scoped by `resource` instead of `actor_id`
/// (a RESOURCE lease's prior-holder identity is not fixed to one actor the
/// way an attachment's succession chain is). `None` today (RED) — no code
/// path ever marks a `leases` row `taken_over` because `takeover` is
/// unrecognized.
async fn taken_over_lease_row(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    resource: &str,
) -> Option<(Uuid, Uuid, bool, Option<Uuid>)> {
    let row: Option<(Uuid, Uuid, bool, Option<Uuid>)> = sqlx::query_as(
        "SELECT id, holder_actor_id, (terminated_at IS NOT NULL), superseded_by
         FROM leases
         WHERE workspace_id = $1 AND resource = $2 AND terminal_state = 'taken_over'",
    )
    .bind(workspace_id)
    .bind(resource)
    .fetch_optional(pool)
    .await
    .expect("taken-over lease row query must execute");
    row
}

/// The `terminal_state` column of `lease_id`, or `None` if the row doesn't
/// exist or the column is NULL (non-terminal/live). Used by the deposed-
/// holder scenario to prove a refused `renew`/`release` left the fixture's
/// already-deposed row untouched.
async fn lease_terminal_state(pool: &sqlx::PgPool, lease_id: Uuid) -> Option<String> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT terminal_state FROM leases WHERE id = $1")
            .bind(lease_id)
            .fetch_optional(pool)
            .await
            .expect("terminal_state read-back query must execute");
    row.and_then(|(s,)| s)
}

/// Force `lease_id` into the DEPOSED state (`terminal_state='taken_over'`,
/// `terminated_at=now()`, `superseded_by=successor`) via direct SQL — a
/// fixture-only construction of R-0066-c's "after a takeover" precondition,
/// independent of the `takeover` action itself (the feature under red-phase
/// test in THIS slice, and therefore unusable as its own setup). Mutates the
/// exact columns build plan §3.4 step 4 documents takeover's own green
/// implementation will set — an extension of this file's established
/// `force_expire_lease` direct-SQL fixture carve-out to the same table's
/// terminal-state columns.
async fn mark_lease_deposed_by_takeover(pool: &sqlx::PgPool, lease_id: Uuid, successor: Uuid) {
    sqlx::query(
        "UPDATE leases
         SET terminal_state = 'taken_over', terminated_at = now(), superseded_by = $2
         WHERE id = $1",
    )
    .bind(lease_id)
    .bind(successor)
    .execute(pool)
    .await
    .expect("mark-deposed UPDATE must execute");
}

/// The `lease_takeover` audit row(s) for `workspace_id`: each tuple is
/// `(actor_id, payload->>'prior_holder', payload->>'new_holder',
/// payload->>'expires_at', payload->>'takeover_ts')`. Workspace-scoped, not
/// actor-scoped (unlike `coordination_session_plane.rs`'s
/// `succession_audit_evidence`) — see this file's slice-d addendum doc
/// comment for why (a takeover names TWO distinct actors, so there is no
/// single-actor filter to key on without assuming `AuditRecord`'s own
/// `actor_id` field, which the build plan leaves unspecified). `vec![]`
/// today (RED) — no takeover ever fires, so no `lease_takeover` audit row is
/// ever staged.
#[allow(clippy::type_complexity)]
async fn lease_takeover_audit_rows(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
) -> Vec<(
    Option<Uuid>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
)> {
    sqlx::query_as(
        "SELECT actor_id,
                payload->>'prior_holder',
                payload->>'new_holder',
                payload->>'expires_at',
                payload->>'takeover_ts'
         FROM coordination_audit
         WHERE workspace_id = $1 AND event_type = 'lease_takeover'",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await
    .expect("lease_takeover audit read-back query must execute")
}

// ===========================================================================
// d AC1 — QA-2 recovery: takeover succeeds on an expired lease, audited with
// all four R-0066-b fields (R-0066-a, R-0066-b)
// ===========================================================================

/// GIVEN actor A holds a lease that has been forced expired (direct SQL, no
/// wall-sleep — mirrors c's `force_expire_lease`; the row is still
/// non-terminal, merely past `expires_at`),
/// WHEN a DIFFERENT attached actor B calls `claim takeover` naming the SAME
/// resource,
/// THEN the call SUCCEEDS — B becomes the new holder on a FRESH lease id
/// (not a mutation of A's row) — A's prior row becomes `taken_over`
/// (terminated, `superseded_by` = B's new lease id), and a `coordination_
/// audit` row of the `lease_takeover` kind exists carrying all FOUR R-0066-b
/// fields: prior_holder = A, new_holder = B, the lease's (forced) expiry
/// timestamp, and a takeover timestamp that is itself >= that expiry (the
/// same store-clock ordering `now() >= expires_at` that authorized the
/// takeover in the first place). *(R-0066-a recovery path; R-0066-b audit;
/// QA-2)*
///
/// RED against this branch's HEAD: `takeover` is unrecognized by
/// `parse_action` (see this file's slice-d addendum) — B's call errors
/// `INVALID_PARAMS` before any coordination body runs, so `claim_structured_
/// obj` returns `None` and the first assertion fails guarantee-absent; no
/// `leases` row is ever marked `taken_over`, and no `lease_takeover` audit
/// row is ever staged (the later assertions are consequently unreachable in
/// RED — they become meaningful only once green lands).
#[tokio::test]
async fn qa2_takeover_recovers_expired_lease_and_emits_audit_row() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token_a = seed_admin_token(&pool, workspace_id).await;
    let token_b = seed_admin_token(&pool, workspace_id).await;
    let role_a = format!("qa2-prior-holder-{}", Uuid::new_v4());
    let role_b = format!("qa2-successor-{}", Uuid::new_v4());

    let (server_a, client_a) = coordination_server(&pool).await;
    let (server_b, client_b) = coordination_server(&pool).await;
    let actor_a = attach_session(&client_a, token_a.as_str(), &role_a).await;
    let actor_b = attach_session(&client_b, token_b.as_str(), &role_b).await;

    let resource = format!("repo-lane:mnemra/qa2-takeover-{}", Uuid::new_v4());
    let acquired = acquire_lease_for_setup(&client_a, token_a.as_str(), &resource).await;
    let lease_id_a = acquired
        .get("lease_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("precondition: acquire response must carry a valid `lease_id`");

    force_expire_lease(&pool, lease_id_a).await;
    let forced = live_lease_row(&pool, workspace_id, &resource).await.expect(
        "precondition: force-expiring A's lease must leave it non-terminal (still occupying \
             the unique slot) with the forced `expires_at`",
    );
    let (_forced_id, _forced_holder, _forced_acquired_epoch, forced_expires_epoch) = forced;

    let res = client_b
        .call_tool(claim_params(token_b.as_str(), "takeover", &resource, None))
        .await;

    let obj = claim_structured_obj(&res);
    assert!(
        obj.is_some(),
        "R-0066-a: `takeover` on an EXPIRED lease by a different attached actor must succeed. \
         `takeover` is unrecognized at this branch's HEAD so the call errors instead. Got: {res:?}"
    );
    let obj = obj.unwrap();

    let new_lease_id = obj
        .get("lease_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("§API Contract: the takeover response must carry a valid string `lease_id`");
    assert_ne!(
        new_lease_id, lease_id_a,
        "QA-2: a successful takeover mints a FRESH lease (fresh window), never mutates A's prior \
         row id in place."
    );

    let holder = obj
        .get("holder")
        .and_then(|v| v.as_object())
        .unwrap_or_else(|| {
            panic!("§API Contract: the takeover response must carry `holder`; got {obj:?}")
        });
    assert_eq!(
        holder.get("actor_id").and_then(|v| v.as_str()),
        Some(actor_b.to_string().as_str()),
        "QA-2: the takeover response's `holder.actor_id` must equal B's actor ({actor_b}) — B is \
         the new holder."
    );

    assert!(
        obj.get("acquired_at").and_then(|v| v.as_str()).is_some(),
        "§API Contract: the takeover response must carry a string `acquired_at`; got {obj:?}"
    );
    let new_expires_at = obj
        .get("expires_at")
        .and_then(|v| v.as_str())
        .map(parse_rfc3339)
        .expect("§API Contract: the takeover response must carry a string `expires_at`");
    assert!(
        new_expires_at.timestamp() as f64 > forced_expires_epoch,
        "QA-2: the fresh lease's `expires_at` must be strictly beyond A's forced (past) expiry — \
         a genuinely fresh window, not a copy of the expired value. new={new_expires_at} \
         forced_epoch={forced_expires_epoch}"
    );

    // The `leases` table shows exactly one LIVE row for the resource, held by B.
    let live = live_lease_row(&pool, workspace_id, &resource).await;
    assert!(
        live.is_some(),
        "QA-2: exactly one live lease must exist for `{resource}` after a successful takeover; \
         unreachable in RED (asserted above already), reached only once green lands."
    );
    let (_live_id, live_holder, _live_acq, _live_exp) = live.unwrap();
    assert_eq!(
        live_holder, actor_b,
        "QA-2: the post-takeover live row's holder must be B ({actor_b}); found {live_holder}."
    );
    assert_eq!(
        live_lease_count(&pool, workspace_id, &resource).await,
        1,
        "QA-2: exactly one live lease must exist for `{resource}` — never two (A's row must have \
         been marked terminal, not merely superseded-in-place with both rows live)."
    );

    // A's prior row is superseded: taken_over, terminated, pointing at B's new lease.
    let prior = taken_over_lease_row(&pool, workspace_id, &resource).await;
    assert!(
        prior.is_some(),
        "R-0066-a: takeover must mark A's prior lease `terminal_state='taken_over'`; none found \
         (RED: takeover never dispatches, so no row is ever mutated)."
    );
    let (prior_id, prior_holder_db, prior_terminated, prior_superseded_by) = prior.unwrap();
    assert_eq!(
        prior_id, lease_id_a,
        "the taken_over row must be A's original lease id"
    );
    assert_eq!(
        prior_holder_db, actor_a,
        "the taken_over row's holder must still read A"
    );
    assert!(
        prior_terminated,
        "the taken_over row must carry a `terminated_at` timestamp"
    );
    assert_eq!(
        prior_superseded_by,
        Some(new_lease_id),
        "the taken_over row's `superseded_by` must equal B's new lease id"
    );

    // Audit: exactly ONE `lease_takeover` row for this workspace, carrying all
    // four R-0066-b fields (never row-existence alone).
    let rows = lease_takeover_audit_rows(&pool, workspace_id).await;
    assert_eq!(
        rows.len(),
        1,
        "R-0066-b: exactly ONE `lease_takeover` audit row must exist for this workspace after one \
         takeover — never zero (RED: no takeover ever fires) and never more than one. rows={rows:?}"
    );
    let (_audit_actor, prior_holder_field, new_holder_field, expires_at_field, takeover_ts_field) =
        rows[0].clone();

    assert_eq!(
        prior_holder_field,
        Some(actor_a.to_string()),
        "R-0066-b: the audit row's `prior_holder` field must equal A's actor id"
    );
    assert_eq!(
        new_holder_field,
        Some(actor_b.to_string()),
        "R-0066-b: the audit row's `new_holder` field must equal B's actor id"
    );

    let expires_at_val = expires_at_field
        .as_deref()
        .map(parse_rfc3339)
        .expect("R-0066-b: the audit row must carry an `expires_at` field parseable as RFC3339");
    assert!(
        (expires_at_val.timestamp() as f64 - forced_expires_epoch).abs() < 2.0,
        "R-0066-b: the audit row's `expires_at` must equal A's lease's (forced) expiry — \
         audit={expires_at_val} forced_epoch={forced_expires_epoch}"
    );

    let takeover_ts_val = takeover_ts_field
        .as_deref()
        .map(parse_rfc3339)
        .expect("R-0066-b: the audit row must carry a `takeover_ts` field parseable as RFC3339");
    assert!(
        takeover_ts_val >= expires_at_val,
        "R-0066-a/-b: the takeover timestamp must be >= the lease's expiry — the same store-clock \
         ordering that authorized the takeover. takeover_ts={takeover_ts_val} \
         expires_at={expires_at_val}"
    );

    server_a.abort();
    server_b.abort();
}

// ===========================================================================
// d AC2 — takeover on a LIVE lease is refused `not_expired` (R-0066-a)
// ===========================================================================

/// GIVEN actor A holds a LIVE (unexpired) lease,
/// WHEN a different attached actor B calls `claim takeover` naming the SAME
/// resource,
/// THEN B is refused the dedicated `not_expired` code — NOT `lease_not_
/// found` (the lease IS found; it is simply not yet eligible for recovery)
/// — and A's lease is left live and untouched (still the sole live row,
/// same holder). *(R-0066-a: "a live (unexpired) lease SHALL NOT be
/// takeover-able")*
///
/// RED against this branch's HEAD: `takeover` is unrecognized — the call
/// errors `INVALID_PARAMS`; `not_expired` is absent.
#[tokio::test]
async fn takeover_on_live_lease_is_refused_not_expired() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token_a = seed_admin_token(&pool, workspace_id).await;
    let token_b = seed_admin_token(&pool, workspace_id).await;
    let role_a = format!("live-holder-{}", Uuid::new_v4());
    let role_b = format!("live-would-be-successor-{}", Uuid::new_v4());

    let (server_a, client_a) = coordination_server(&pool).await;
    let (server_b, client_b) = coordination_server(&pool).await;
    let actor_a = attach_session(&client_a, token_a.as_str(), &role_a).await;
    attach_session(&client_b, token_b.as_str(), &role_b).await;

    let resource = format!("repo-lane:mnemra/live-no-takeover-{}", Uuid::new_v4());
    // 3600s duration via `acquire_lease_for_setup` — provably live, nowhere
    // near expiry.
    acquire_lease_for_setup(&client_a, token_a.as_str(), &resource).await;

    let res = client_b
        .call_tool(claim_params(token_b.as_str(), "takeover", &resource, None))
        .await;

    assert!(
        result_surfaces_code(&res, "not_expired"),
        "R-0066-a: `takeover` on a LIVE lease must be refused `not_expired`. Against the \
         unrecognized `takeover` action at this branch's HEAD the code is absent \
         (`INVALID_PARAMS` instead). Got: {res:?}"
    );
    assert!(
        !result_surfaces_code(&res, "lease_not_found"),
        "R-0066-a: the lease IS found and IS live — a takeover attempt on it must NOT be \
         conflated with `lease_not_found`. Got: {res:?}"
    );

    let live = live_lease_row(&pool, workspace_id, &resource).await;
    assert!(
        live.is_some(),
        "a `not_expired` refusal must leave A's lease live; found none."
    );
    let (_id, holder_db, _acq, _exp) = live.unwrap();
    assert_eq!(
        holder_db, actor_a,
        "a `not_expired` refusal must leave A as the untouched holder; found {holder_db}."
    );
    assert_eq!(
        live_lease_count(&pool, workspace_id, &resource).await,
        1,
        "a `not_expired` refusal must create no second row."
    );

    server_a.abort();
    server_b.abort();
}

// ===========================================================================
// d AC3 — takeover on a resource with NO lease row is refused
// `lease_not_found` (R-0066-a)
// ===========================================================================

/// GIVEN an attached actor and a resource that NO ONE has ever acquired,
/// WHEN it calls `claim takeover` naming that resource,
/// THEN it is refused `lease_not_found` — NOT `not_expired` (there is no row
/// to be live OR expired) — per R-0066-a's explicit "no lease row at all"
/// case ("use `acquire`"). No `leases` row is created. *(R-0066-a)*
///
/// RED against this branch's HEAD: `takeover` is unrecognized — the call
/// errors `INVALID_PARAMS`; `lease_not_found` is absent.
#[tokio::test]
async fn takeover_on_resource_with_no_lease_is_refused_lease_not_found() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("takeover-no-row-{}", Uuid::new_v4());
    let resource = format!("repo-lane:mnemra/never-acquired-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    let res = client
        .call_tool(claim_params(token.as_str(), "takeover", &resource, None))
        .await;

    assert!(
        result_surfaces_code(&res, "lease_not_found"),
        "R-0066-a: `takeover` on a resource with no lease row at all must be refused \
         `lease_not_found`. Against the unrecognized `takeover` action at this branch's HEAD the \
         code is absent (`INVALID_PARAMS` instead). Got: {res:?}"
    );
    assert!(
        !result_surfaces_code(&res, "not_expired"),
        "R-0066-a: with no row at all there is nothing to be live OR expired — must not be \
         conflated with `not_expired`. Got: {res:?}"
    );

    let live = live_lease_count(&pool, workspace_id, &resource).await;
    assert_eq!(
        live, 0,
        "a `lease_not_found` refusal must create no `leases` row; found {live} for `{resource}`."
    );

    server.abort();
}

// ===========================================================================
// d AC4 — acquire on an expired-but-untaken lease is refused `resource_held`
// (R-0066-a) — REGRESSION GUARD, may already pass at this branch's HEAD
// ===========================================================================

/// GIVEN actor A holds a lease that has been forced expired (direct SQL — no
/// takeover has ever run against it, so the row is still non-terminal),
/// WHEN a different attached actor C calls `claim acquire` on the SAME
/// resource,
/// THEN C is refused `resource_held` — carrying A's actor_id and the
/// ALREADY-PAST expiry: the expired row is dispositioned ONLY by
/// `takeover`, never folded into `acquire`'s own transaction. *(R-0066-a)*
///
/// # NOT a guarantee-absent red for THIS slice — a regression guard on
/// ALREADY-GREEN b1 behavior (read before trusting this test's red-ness)
///
/// Unlike every other scenario in this file's d addendum, `acquire` is
/// ALREADY implemented (b1, green on this branch) and its own construction
/// — confirmed by reading `leases.rs::acquire_body`'s doc comment — already
/// handles the expired-but-untaken collision via `INSERT ... ON CONFLICT DO
/// NOTHING` + a follow-up read, never a cleanup/delete. This test may
/// therefore ALREADY PASS at this branch's HEAD (before any d-slice code
/// exists) — this dispatch's own red-confirm run states whether it does.
/// Kept in this file regardless: if it passes today, it is a REGRESSION
/// GUARD locking in b1's already-correct behavior against a future `acquire`
/// change that folds in cleanup (which R-0066-a explicitly forbids); if it
/// somehow fails, that is itself a genuine finding about b1, not d.
#[tokio::test]
async fn acquire_on_expired_untaken_lease_is_refused_resource_held() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token_a = seed_admin_token(&pool, workspace_id).await;
    let token_c = seed_admin_token(&pool, workspace_id).await;
    let role_a = format!("expired-untaken-holder-{}", Uuid::new_v4());
    let role_c = format!("expired-untaken-reacquirer-{}", Uuid::new_v4());

    let (server_a, client_a) = coordination_server(&pool).await;
    let (server_c, client_c) = coordination_server(&pool).await;
    let actor_a = attach_session(&client_a, token_a.as_str(), &role_a).await;
    attach_session(&client_c, token_c.as_str(), &role_c).await;

    let resource = format!("repo-lane:mnemra/expired-untaken-{}", Uuid::new_v4());
    let acquired = acquire_lease_for_setup(&client_a, token_a.as_str(), &resource).await;
    let lease_id_a = acquired
        .get("lease_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("precondition: acquire response must carry a valid `lease_id`");
    force_expire_lease(&pool, lease_id_a).await;

    let res = client_c
        .call_tool(claim_params(token_c.as_str(), "acquire", &resource, None))
        .await;

    assert!(
        result_surfaces_code(&res, "resource_held"),
        "R-0066-a: `acquire` on a resource whose lease is EXPIRED-BUT-UNTAKEN must still be \
         refused `resource_held` (the expired row is dispositioned only by `takeover`, never \
         folded into `acquire`). Got: {res:?}"
    );

    let obj = claim_structured_obj(&res).unwrap_or_else(|| {
        panic!("R-0065-c: the `resource_held` refusal must carry structured content. Got: {res:?}")
    });
    let detail = obj
        .get("detail")
        .and_then(|v| v.as_object())
        .unwrap_or_else(|| panic!("the refusal must carry a `detail` object; got {obj:?}"));
    assert!(
        detail
            .values()
            .any(|v| v.as_str() == Some(actor_a.to_string().as_str())),
        "R-0066-a/R-0065-c: the refusal `detail` must name A ({actor_a}) as the (still) holder. \
         detail={detail:?}"
    );
    assert!(
        detail
            .values()
            .any(|v| v.as_str().is_some_and(looks_like_timestamp)),
        "R-0066-a/R-0065-c: the refusal `detail` must carry the ALREADY-PAST expiry as a \
         timestamp-shaped value. detail={detail:?}"
    );

    // No cleanup, no second row — still exactly the one (now-expired, still
    // non-terminal) row.
    assert_eq!(
        live_lease_count(&pool, workspace_id, &resource).await,
        1,
        "R-0066-a: an `acquire` collision on an expired-but-untaken lease must leave EXACTLY the \
         original row live (non-terminal) — no cleanup, no second row."
    );

    server_a.abort();
    server_c.abort();
}

// ===========================================================================
// d AC5 — takeover `reserved_family`, distinct from `invalid_resource`
// (R-0067-c, request-side)
// ===========================================================================

/// GIVEN an attached session,
/// WHEN it calls `claim takeover` naming a well-formed but RESERVED resource
/// (`actor:whatever`),
/// THEN it is refused the DEDICATED `reserved_family` code — NOT
/// `invalid_resource` (mirrors b1 AC5's acquire probe, substituting
/// `takeover`; `takeover` takes a `resource` string exactly like `acquire`,
/// so only the request-side arm of R-0067-c applies here — unlike renew/
/// release, which name a `lease_id` and need the resolved-lease-row arm
/// instead, tested in slice c). No `leases` row is created or mutated.
/// *(R-0067-c)*
///
/// RED against this branch's HEAD: `takeover` is unrecognized — the call
/// errors `INVALID_PARAMS`; `reserved_family` is absent (and so, trivially,
/// is `invalid_resource` — the precision guard below is what makes this a
/// non-vacuous DISTINCTNESS assertion once green lands).
#[tokio::test]
async fn takeover_reserved_family_is_refused_distinctly_from_invalid_resource() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("takeover-reserved-family-{}", Uuid::new_v4());
    let resource = "actor:whatever";

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    let res = client
        .call_tool(claim_params(token.as_str(), "takeover", resource, None))
        .await;

    assert!(
        result_surfaces_code(&res, "reserved_family"),
        "R-0067-c: `takeover \"{resource}\"` (the reserved `actor:` family) must be refused the \
         dedicated `reserved_family` code. Against the unrecognized `takeover` action at this \
         branch's HEAD the code is absent (`INVALID_PARAMS` instead). Got: {res:?}"
    );
    assert!(
        !result_surfaces_code(&res, "invalid_resource"),
        "R-0067-c: a reserved-family refusal must be DISTINCT from `invalid_resource`. Got: {res:?}"
    );

    let live = live_lease_count(&pool, workspace_id, resource).await;
    assert_eq!(
        live, 0,
        "a `reserved_family` refusal must create no `leases` row for the literal probe string; \
         found {live}."
    );

    server.abort();
}

// ===========================================================================
// d AC6 — deposed-holder backstop: renew AND release both refused after a
// (fixture-simulated) takeover (R-0066-c)
// ===========================================================================

/// GIVEN actor A's lease has been DEPOSED (fixture-simulated post-takeover
/// state — `terminal_state='taken_over'`; see this file's slice-d addendum
/// for why a fixture, not the real `takeover` action, constructs this state
/// here),
/// WHEN A calls `claim renew` and separately `claim release`, each naming
/// its OWN (now-deposed) `lease_id`,
/// THEN each is refused a structured code in the `not_holder`/
/// `lease_not_found` family — R-0066-c's own LOCKED wording ("a structured
/// refusal (`not_holder`/`lease_not_found` family)") — and NEVER succeeds;
/// the deposed row itself is left untouched by either refusal attempt
/// (`terminal_state` stays `'taken_over'`, unchanged). *(R-0066-c)*
///
/// # Fixture note + expected mechanism (read before trusting this test's
/// red-ness)
///
/// This scenario constructs its precondition via DIRECT SQL
/// (`mark_lease_deposed_by_takeover`), not the real `takeover` action (which
/// is this slice's own feature under red-phase test and therefore cannot be
/// its own setup — see this file's slice-d addendum). Because the resulting
/// row differs from an ordinary expired-untaken row ONLY in which non-NULL
/// `terminal_state` value it carries (`'taken_over'` here vs `'released'` in
/// c's `release_then_release_again_is_refused_lease_not_found`), and c's
/// `renew`/`release` liveness check (already GREEN, slice c) excludes ANY
/// non-NULL `terminal_state` uniformly, this scenario may ALREADY PASS at
/// this branch's HEAD — independent of whether `takeover` itself is
/// implemented. That would make it a REGRESSION GUARD for R-0066-c
/// specifically (the c-slice mechanism already satisfies the LOCKED
/// requirement for this exact precondition, by construction, not by
/// coincidence) rather than a guarantee-absent d-slice red — this
/// dispatch's own red-confirm run states which. Kept in this file's d
/// addendum regardless, because R-0066-c is a d-slice requirement and its
/// acceptance test belongs with the requirement it locks, not with c's
/// unrelated (expired-but-never-taken-over) scenarios.
#[tokio::test]
async fn deposed_holder_renew_and_release_after_takeover_are_refused() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token_a = seed_admin_token(&pool, workspace_id).await;
    let role_a = format!("deposed-holder-{}", Uuid::new_v4());

    let (server_a, client_a) = coordination_server(&pool).await;
    attach_session(&client_a, token_a.as_str(), &role_a).await;

    let resource = format!("repo-lane:mnemra/deposed-{}", Uuid::new_v4());
    let acquired = acquire_lease_for_setup(&client_a, token_a.as_str(), &resource).await;
    let lease_id_a = acquired
        .get("lease_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("precondition: acquire response must carry a valid `lease_id`");

    // Fixture-simulate the post-takeover deposed state (see doc comment above
    // for why the real `takeover` action cannot be its own setup here).
    let fake_successor_lease_id = Uuid::new_v4();
    mark_lease_deposed_by_takeover(&pool, lease_id_a, fake_successor_lease_id).await;
    assert_eq!(
        lease_terminal_state(&pool, lease_id_a).await,
        Some("taken_over".to_owned()),
        "precondition: the fixture must leave A's lease `terminal_state='taken_over'`"
    );

    let renew_res = client_a
        .call_tool(claim_lease_action_params(
            token_a.as_str(),
            "renew",
            lease_id_a,
        ))
        .await;
    assert!(
        result_surfaces_code(&renew_res, "lease_not_found")
            || result_surfaces_code(&renew_res, "not_holder"),
        "R-0066-c: A's `renew` on its own DEPOSED lease must return a structured refusal in the \
         `not_holder`/`lease_not_found` family — never succeed. Got: {renew_res:?}"
    );

    let release_res = client_a
        .call_tool(claim_lease_action_params(
            token_a.as_str(),
            "release",
            lease_id_a,
        ))
        .await;
    assert!(
        result_surfaces_code(&release_res, "lease_not_found")
            || result_surfaces_code(&release_res, "not_holder"),
        "R-0066-c: A's `release` on its own DEPOSED lease must ALSO return a structured refusal \
         in the `not_holder`/`lease_not_found` family — never succeed. Got: {release_res:?}"
    );

    // Neither refusal attempt mutated the deposed row.
    assert_eq!(
        lease_terminal_state(&pool, lease_id_a).await,
        Some("taken_over".to_owned()),
        "R-0066-c: a refused renew/release on a deposed lease must leave `terminal_state` \
         unchanged (still `'taken_over'`) — no side effect."
    );

    server_a.abort();
}

// ===========================================================================
// d AC7 — takeover without attachment is refused `not_attached` (R-0064-e)
// ===========================================================================

/// GIVEN a fresh session that has NEVER bound via `message poll`,
/// WHEN it calls `claim takeover` on an otherwise-valid resource,
/// THEN it is refused `not_attached` and no `leases` row is created or
/// mutated — the attachment gate resolves before any resource/lease lookup
/// (mirrors b1 AC3 / c AC10's precedent, extended to `takeover`).
/// *(R-0064-e)*
///
/// RED against this branch's HEAD: `takeover` is unrecognized — the call
/// errors `INVALID_PARAMS` regardless of attachment state; `not_attached` is
/// absent.
#[tokio::test]
async fn takeover_without_attachment_is_refused_not_attached() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let resource = format!("repo-lane:mnemra/takeover-unattached-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;

    // No `poll` call — this session never attaches.
    let res = client
        .call_tool(claim_params(token.as_str(), "takeover", &resource, None))
        .await;

    assert!(
        result_surfaces_code(&res, "not_attached"),
        "R-0064-e: `claim takeover` from a session with no live attachment must be refused \
         `not_attached`. Against the unrecognized `takeover` action at this branch's HEAD the \
         code is absent (`INVALID_PARAMS` instead). Got: {res:?}"
    );

    let live = live_lease_count(&pool, workspace_id, &resource).await;
    assert_eq!(
        live, 0,
        "a `not_attached` refusal must create no `leases` row; found {live} for `{resource}`."
    );

    server.abort();
}

// ===========================================================================
// d AC8 — takeover `invalid_resource` (R-0067-a, via takeover)
// ===========================================================================

/// GIVEN an attached session,
/// WHEN it calls `claim takeover` on (a) an out-of-family resource
/// (`bogus:x`) and (b) a malformed resource with no `:` qualifier at all
/// (`repo-lane`),
/// THEN each is refused `invalid_resource` and creates no `leases` row.
/// Mirrors b1 AC4's acquire probe, substituting `takeover`. *(R-0067-a)*
///
/// RED against this branch's HEAD: `takeover` is unrecognized — both calls
/// hit `INVALID_PARAMS`; `invalid_resource` is absent for either
/// malformation.
#[tokio::test]
async fn takeover_invalid_resource_is_refused() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool = db.pool.clone();

    let workspace_id = Uuid::new_v4();
    let token = seed_admin_token(&pool, workspace_id).await;
    let role_instance = format!("takeover-invalid-resource-{}", Uuid::new_v4());

    let (server, client) = coordination_server(&pool).await;
    attach_session(&client, token.as_str(), &role_instance).await;

    for (label, resource) in [
        ("out-of-family", "bogus:x"),
        ("malformed (no qualifier)", "repo-lane"),
    ] {
        let res = client
            .call_tool(claim_params(token.as_str(), "takeover", resource, None))
            .await;

        assert!(
            result_surfaces_code(&res, "invalid_resource"),
            "R-0067-a: `takeover \"{resource}\"` ({label}) must be refused `invalid_resource`. \
             Against the unrecognized `takeover` action at this branch's HEAD the code is absent \
             (`INVALID_PARAMS` instead). Got: {res:?}"
        );

        let live = live_lease_count(&pool, workspace_id, resource).await;
        assert_eq!(
            live, 0,
            "an `invalid_resource` refusal must create no `leases` row; found {live} for \
             `{resource}` ({label})."
        );
    }

    server.abort();
}
