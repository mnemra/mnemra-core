//! Session plane: the host-served `message`/`claim` coordination tools.
//!
//! # Scope (Task 4 foundation slice)
//!
//! This module lands the host-served MCP branch's *skeleton*: the `message`
//! tool advertisement (`poll` action only at this stage), the closed
//! `action`-argument parser, and the placeholder `poll` route. The per-action
//! token-role capability gate lives in [`crate::mcp::dispatch`]
//! (`authorize_coordination_action`, R-0073-b). The attach / audited-succession
//! / real poll-delivery logic (R-0064-c/-d, R-0072-a) lands in later sub-runs,
//! each authored Glitch-red-first.
//!
//! # Why a host-served branch (not the plugin path)
//!
//! Coordination tools are single MCP tools named `message`/`claim` carrying a
//! closed `action` enum *argument* (`{ action: "poll", role_instance }`) — not
//! dotted `plugin.verb` tools. The plugin dispatch path (echo-verb manifest
//! gate → tail-split `resolve_content_call` → WASM `invoke_content`) does not
//! fit, so `call_tool` forks a host-served branch (R-0063-a: host-served, no
//! plugin-ABI export, no P-0013 domain-verb dispatch machinery).

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use rmcp::model::{CallToolResult, ErrorData, Tool};
use uuid::Uuid;

use crate::auth::workspace_ctx::WorkspaceCtx;
use crate::builtins::actors::{ActorType, resolve_or_create_in_txn};
use crate::coordination::CoordinationConfig;
use crate::coordination::CoordinationOp;
use crate::coordination::audit::{AuditRecord, ExpiryEvidence};
use crate::coordination::messages;
use crate::coordination::write_path::{
    CoordinationTxn, LiveLeaseRow, PgCoordinationStore, Refusal, StorageFailure, WriteResult,
};

/// The host-served coordination tool name for the messaging surface. Task 5
/// adds the sibling `claim` tool through this same host-served branch.
pub(crate) const MESSAGE_TOOL: &str = "message";

/// The host-served coordination tool name for the lease-claim surface (Task
/// 5). Advertises `acquire` (slice b1), `list` (slice b2), `renew`/`release`
/// (slice c), and `takeover` (slice d).
pub(crate) const CLAIM_TOOL: &str = "claim";

/// Returns `true` if `name` is a host-served coordination tool — the ones
/// `call_tool` routes to the coordination branch *before* the echo-verb
/// manifest membership gate (they are not echo verbs, so that gate would
/// otherwise reject them).
pub(crate) fn is_coordination_tool(name: &str) -> bool {
    name == MESSAGE_TOOL || name == CLAIM_TOOL
}

/// A coordination action, parsed from a tool call's `action` argument,
/// disambiguated by the tool name ([`parse_action`]'s `tool` parameter) —
/// `claim` and `message` each own their own action vocabulary (and, in later
/// slices, their own `list` action), so the same action string under
/// different tools resolves to different variants (Task 5 b1 Q1 decision:
/// tool-aware `parse_action` + one flat enum with tool-prefixed variant
/// names, rather than splitting `ClaimAction`/`MessageAction`).
///
/// Closed set. `message` advertises `poll` and `send` (Task 7 slice a);
/// `claim` advertises `acquire` (Task 5 slice b1), `list` (Task 5 slice b2),
/// `renew`/`release` (Task 5 slice c), and `takeover` (Task 5 slice d); Task
/// 7 further extends `message` with `list`/`ack`/`disposition` in later
/// slices. Every coordination action is write-category under R-0073-b — the
/// classification lives in
/// [`crate::mcp::dispatch::authorize_coordination_action`], which reads the
/// *action*, never the tool-name tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoordinationAction {
    /// `message poll` — the bind call (R-0064-e): resolve-or-create, attach
    /// or audited succession, then deliver ([`poll_bind`]).
    Poll,
    /// `message send` (R-0068-a, R-0070-b, R-0075-b; Task 7 slice a) —
    /// routed to [`crate::coordination::messages::send`].
    Send,
    /// `message ack` (R-0069-a/-b; Task 7 slice b) — routed to
    /// [`crate::coordination::messages::ack`].
    Ack,
    /// `message disposition` (R-0069-a/-b/-c, R-0075-b; Task 7 slice b) —
    /// routed to [`crate::coordination::messages::disposition`].
    Disposition,
    /// `claim acquire` (R-0065-a/-b/-c/-d, R-0067) — routed to
    /// [`crate::coordination::leases::acquire`].
    ClaimAcquire,
    /// `claim list` (R-0073-a, R-0067-c, R-0075-a) — routed to
    /// [`crate::coordination::leases::list`]. Prefixed `Claim` to pre-empt
    /// the Task-7 `message list` collision (Task 5 b1 Q1 decision).
    ClaimList,
    /// `claim renew` (R-0065-d) — routed to
    /// [`crate::coordination::leases::renew`].
    ClaimRenew,
    /// `claim release` (R-0065-d) — routed to
    /// [`crate::coordination::leases::release`].
    ClaimRelease,
    /// `claim takeover` (R-0066-a/-b/-c, R-0067-c) — routed to
    /// [`crate::coordination::leases::takeover`].
    ClaimTakeover,
}

/// Parse the closed `action` argument from a coordination tool call,
/// disambiguated by `tool` (`MESSAGE_TOOL` or `CLAIM_TOOL`).
///
/// The advertised input schema constrains `action` to the closed enum, but the
/// host re-parses defensively — client-supplied structure is never trusted. An
/// absent, non-string, out-of-set, or wrong-tool `action` is a malformed
/// request and maps to `INVALID_PARAMS`. (At this stage `message` supports
/// only `poll`; `claim` supports `acquire`, `list`, `renew`, `release`, and
/// `takeover`.)
pub(crate) fn parse_action(
    tool: &str,
    arguments: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Result<CoordinationAction, ErrorData> {
    let action = arguments
        .and_then(|m| m.get("action"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData {
            code: rmcp::model::ErrorCode::INVALID_PARAMS,
            message: "coordination call requires a string `action` argument".into(),
            data: None,
        })?;

    match (tool, action) {
        (MESSAGE_TOOL, "poll") => Ok(CoordinationAction::Poll),
        (MESSAGE_TOOL, "send") => Ok(CoordinationAction::Send),
        (MESSAGE_TOOL, "ack") => Ok(CoordinationAction::Ack),
        (MESSAGE_TOOL, "disposition") => Ok(CoordinationAction::Disposition),
        (CLAIM_TOOL, "acquire") => Ok(CoordinationAction::ClaimAcquire),
        (CLAIM_TOOL, "list") => Ok(CoordinationAction::ClaimList),
        (CLAIM_TOOL, "renew") => Ok(CoordinationAction::ClaimRenew),
        (CLAIM_TOOL, "release") => Ok(CoordinationAction::ClaimRelease),
        (CLAIM_TOOL, "takeover") => Ok(CoordinationAction::ClaimTakeover),
        _ => Err(ErrorData {
            code: rmcp::model::ErrorCode::INVALID_PARAMS,
            message: format!("unsupported coordination action '{action}' for tool '{tool}'").into(),
            data: None,
        }),
    }
}

/// The host-served coordination tools advertised by `list_tools`, concatenated
/// with the plugin (echo) verbs. At this stage: `message` with the `poll`
/// and `send` actions (Task 7 slice a).
///
/// The input schema declares the closed `action` enum + per-action
/// arguments. It carries **no acting-actor field** (R-0064-b): the acting
/// principal is host-derived from attachment state; a caller-supplied
/// principal does not exist anywhere on the surface.
pub(crate) fn coordination_tools() -> Vec<Tool> {
    vec![message_tool(), claim_tool()]
}

/// Build the `message` tool advertisement (`poll`/`send`/`ack`/`disposition`
/// actions, R-0064-b schema).
fn message_tool() -> Tool {
    // `action` is the closed enum argument (`poll`/`send`/`ack`/
    // `disposition` at this stage); `role_instance` is the bind identifier
    // (`poll` only); `to_role_instance`/`type`/`schema_version`/`payload`
    // are `send`'s arguments (§API Contract `send`); `message_id` is the
    // target message identifier (`ack`/`disposition`); `disposition` is the
    // closed terminal-disposition member (`disposition` only, R-0069-c);
    // `note` is the optional structured note (`disposition` only). Only
    // `action` is required at the schema level — every other argument is
    // meaningful to exactly one action, so no single argument is required
    // across all of them (mirrors the `claim` tool's own schema convention,
    // `claim_tool()` below). `additionalProperties: false` makes the closed
    // shape explicit; there is deliberately NO acting-actor/sender field
    // (R-0064-b) — the acting principal is host-derived from attachment
    // state.
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["poll", "send", "ack", "disposition"],
                "description": "The coordination action (closed set)."
            },
            "role_instance": {
                "type": "string",
                "description": "The role-instance identifier the session binds to on poll. \
                                 `poll` only."
            },
            "to_role_instance": {
                "type": "string",
                "description": "The addressee role-instance identifier. `send` only."
            },
            "type": {
                "type": "string",
                "description": "The registered message-type name (R-0070-a). `send` only."
            },
            "schema_version": {
                "type": "integer",
                "description": "The registered message-type's schema version. `send` only."
            },
            "payload": {
                "type": "object",
                "description": "The message payload, schema-validated against the named \
                                 type + schema_version (Task 6). `send` only."
            },
            "message_id": {
                "type": "string",
                "description": "The target message identifier (a UUID). `ack`/`disposition` \
                                 only."
            },
            "disposition": {
                "type": "string",
                "enum": ["completed", "declined", "obsolete"],
                "description": "The closed terminal-disposition member (R-0069-c). \
                                 `disposition` only."
            },
            "note": {
                "type": "string",
                "description": "Optional structured note, stored verbatim. `disposition` only."
            }
        },
        "required": ["action"],
        "additionalProperties": false
    });
    let schema_obj = schema
        .as_object()
        .expect("message tool schema is a JSON object literal")
        .clone();

    Tool::new_with_raw(
        Cow::Borrowed(MESSAGE_TOOL),
        Some(Cow::Borrowed(
            "Coordination messaging tool. The operation is selected by the `action` argument.",
        )),
        Arc::new(schema_obj),
    )
}

/// Build the `claim` tool advertisement (`acquire`/`list`/`renew`/`release`/
/// `takeover` actions at slice d, R-0064-b schema).
fn claim_tool() -> Tool {
    // `action` is the closed enum argument (`acquire`/`list`/`renew`/
    // `release`/`takeover` at this stage); `resource` is the structured
    // `<family>:<qualifier>` identifier (R-0067, `acquire`/`takeover`-only —
    // takeover carries no duration argument, R-0066-a: the recovered lease
    // always uses the configured default, matching an omitted `acquire`
    // duration); `duration_seconds` is optional (`acquire`-only, defaults
    // per R-0065-d); `family`/`resource_prefix` are the optional `list`
    // filters (§API Contract `list`); `lease_id` is the target lease
    // identifier (`renew`/`release`-only, §API Contract). Only `action` is
    // required at the schema level — every other argument is meaningful to
    // exactly one action, so no single argument is required across all of
    // them. `additionalProperties: false` makes the closed shape explicit;
    // there is deliberately NO acting-actor field (R-0064-b) — the acting
    // principal is host-derived from attachment state.
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["acquire", "list", "renew", "release", "takeover"],
                "description": "The coordination claim action (closed set)."
            },
            "resource": {
                "type": "string",
                "description": "The structured resource identifier `<family>:<qualifier>` \
                                 (R-0067). Required for `acquire` and `takeover`."
            },
            "duration_seconds": {
                "type": "integer",
                "description": "Requested lease duration in seconds; omit for the configured \
                                 default, bounded by the policy maximum (R-0065-d). `acquire` only."
            },
            "family": {
                "type": "string",
                "description": "Optional family filter for `list` (§API Contract `list`); \
                                 `family: \"actor\"` is refused `reserved_family` (R-0067-c)."
            },
            "resource_prefix": {
                "type": "string",
                "description": "Optional resource-prefix filter for `list` (§API Contract `list`)."
            },
            "lease_id": {
                "type": "string",
                "description": "The target lease identifier (§API Contract). Required for \
                                 `renew` and `release`."
            }
        },
        "required": ["action"],
        "additionalProperties": false
    });
    let schema_obj = schema
        .as_object()
        .expect("claim tool schema is a JSON object literal")
        .clone();

    Tool::new_with_raw(
        Cow::Borrowed(CLAIM_TOOL),
        Some(Cow::Borrowed(
            "Coordination claim tool (lease acquisition + listing). The operation is selected \
             by the `action` argument.",
        )),
        Arc::new(schema_obj),
    )
}

/// The one host-registered role-instance identifier rule (R-0064-a), shared by
/// the bind path (here) and the Task-7 send-side.
///
/// The grammar is implementation-tier; the spec-stated constraints are:
/// non-empty, length-bounded, and free of whitespace or control characters. A
/// violation is a [`Refusal::InvalidRoleInstance`] — refused before any actor
/// row is minted.
pub(crate) fn validate_role_instance(role_instance: &str) -> Result<(), Refusal> {
    // Impl-tier upper bound — generous for role-instance names (`design-lane`,
    // `merger`) while still bounding the identifier.
    const MAX_LEN: usize = 256;
    if role_instance.is_empty()
        || role_instance.len() > MAX_LEN
        || role_instance
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(Refusal::InvalidRoleInstance);
    }
    Ok(())
}

/// The `message poll` bind (R-0064-a/-c, R-0075-b): resolve-or-create the actor
/// for `role_instance`, attach it as a lease (one live attachment per actor),
/// and audit the registration + attachment — atomically inside one `run_write`
/// transaction.
///
/// Session derivation (V0): the session identifier is the calling token's id
/// ([`WorkspaceCtx::token_id`]) — the token-derived session the red-test
/// contract sanctions. Two distinct tokens are two distinct sessions, so a
/// second token binding an already-attached actor is a genuine competitor
/// (refused `actor_live_attached`), not a same-session renewal (R-0064-e, whose
/// same-session path is sub-run d).
///
/// Scope (sub-runs c + d + e): fresh attach + one-live enforcement +
/// wrong-actor-type + identifier validation + audit (c); audited succession
/// over a *stale* attachment and same-session TTL renew-on-activity (d); the
/// full poll response shape (`{ actor, messages, live_leases }`, R-0072-a) and
/// the `attachment_mismatch` gate (R-0064-e) land here (e). `not_attached`
/// stays deferred to Task 5 — `poll` is the only advertised action, so no
/// non-binding action exists from which an unattached session could be refused.
pub(crate) async fn poll_bind(
    store: &PgCoordinationStore,
    config: &CoordinationConfig,
    ctx: &WorkspaceCtx,
    role_instance: &str,
) -> Result<CallToolResult, ErrorData> {
    // Session identity is machinery-derived from the authenticated token, never
    // a caller write input (R-0064-b).
    let session_id = ctx.token_id;
    let ttl = config.attachment_ttl;
    // Owned copies: one moved into the `run_write` body (an `FnOnce`), one kept
    // for the success acknowledgement.
    let role_owned = role_instance.to_owned();
    let role_for_body = role_owned.clone();

    let write = store
        .run_write(
            ctx,
            CoordinationOp::AttachBind,
            move |tx: &mut CoordinationTxn| {
                Box::pin(async move { attach_body(tx, &role_for_body, session_id, ttl).await })
            },
        )
        .await;

    match write {
        Ok(WriteResult::Commit(actor_id)) => {
            // Deliver half (R-0072-a): the bind committed; transition the
            // actor's still-`sent` messages to `delivered` and read its
            // non-terminal queue (a SEPARATE `CoordinationOp::Poll` write —
            // `messages::deliver_and_list`, Task 7 slice b), then read the
            // workspace's live leases (post-commit, workspace-scoped,
            // `actor:`-family excluded) and render the documented poll
            // response on EVERY non-refused outcome — fresh attach,
            // same-session renew, and audited succession alike. A read
            // failure is fail-closed (R-0074-a), the same posture as a
            // write-path unavailability.
            let messages = messages::deliver_and_list(store, ctx, actor_id).await?;
            let live_leases = store
                .live_leases_for_workspace(ctx)
                .await
                .map_err(|_| coordination_unavailable())?;
            Ok(poll_response(actor_id, &role_owned, messages, live_leases))
        }
        Ok(WriteResult::Refuse(refusal)) => Ok(refusal_result(&refusal)),
        // Fail-closed (R-0074-a): a write that could not be verified surfaces as
        // a structured stop, never an empty success. `run_write` has already
        // op-logged the unavailability (R-0075-a); no store internals leak here.
        Err(_unavailable) => Err(coordination_unavailable()),
    }
}

/// The live-attachment fork row read by [`attach_body`]: the live `actor:<id>`
/// lease's `(id, session_id, expires_at)` plus the store transaction clock
/// `now()` that evaluates staleness (R-0065-e). `None` (as `Option<_>`) when the
/// actor has no live attachment.
type LiveAttachmentFork = (Uuid, Option<Uuid>, DateTime<Utc>, DateTime<Utc>);

/// The attach state transition, run inside the `run_write` transaction. Returns
/// a [`WriteResult`] so the machinery owns commit/rollback, the audit-outbox
/// flush, and op-logging.
///
/// Ordering is load-bearing:
/// 1. Validate the identifier — refuse `invalid_role_instance` before any mint
///    (a rejected identifier mints no actor row; the refusal rolls the txn back).
/// 2. Resolve-or-create the actor on THIS txn. `workspace_id` is the
///    machinery-authoritative tenant scope ([`CoordinationTxn::workspace_id`]),
///    never caller input.
/// 3. Refuse `wrong_actor_type` when the resolved row is not an `agent` — a
///    session cannot attach to a `human`/`system` identity. `resolve_or_create_in_txn`
///    returns the PERSISTED type, so a pre-existing non-agent row is caught here
///    with no redundant pre-query. Then, still before the fork, refuse
///    `attachment_mismatch` (R-0064-e) when THIS session already holds a LIVE
///    (non-terminal, unexpired) attachment to a DIFFERENT actor — one session,
///    one role-instance. A same-session re-poll of the SAME role-instance
///    resolves the same actor (`holder_actor_id = actor.id`) and is excluded, so
///    it falls through to the renew/succession fork below (never regressed).
/// 4. **Fork on the actor's current attachment state.** `SELECT … FOR UPDATE`
///    the live `actor:<id>` lease (if any) and read the store transaction clock
///    (`now()`) in the SAME query — staleness is evaluated against the store
///    clock (R-0065-e), never a host-local clock, and a concurrent successor
///    serializes on the locked row. The discriminator:
///    - **No live lease** → FRESH ATTACH (step 5): the mint's first attachment,
///      registration + attachment audit.
///    - **Live but EXPIRED** (`store_now >= expires_at`, ANY session — a
///      same-session idle-gap re-bind rides this SAME audited path; the session
///      equality is a discriminator, not a skip signal) → SUCCESSION
///      ([`succeed_via_takeover`]): supersede the prior lease, insert the
///      successor, emit `attachment_succession` — NO new `registration` (the
///      actor pre-exists; the fork happens BEFORE fresh-attach
///      registration-staging). R-0064-d.
///    - **Live and NOT expired, SAME session** → RENEW in place: extend the
///      existing lease's `expires_at` from the store clock (same lease id, no
///      new row, no succession audit). R-0064-e / plan TTL renew-on-activity.
///    - **Live and NOT expired, DIFFERENT session** → refuse
///      `actor_live_attached` — succession may not fire before expiry (the TB-3
///      impersonation-window guard). R-0064-c/-d.
/// 5. FRESH ATTACH: compute the lease window from the store clock, insert the
///    attachment-as-lease under the reserved `actor` family
///    (`leases_live_resource_uq` makes one live attachment per actor structural;
///    a lost concurrent race collides with SQLSTATE 23505 → refuse
///    `actor_live_attached`), and stage the registration + attachment audit.
async fn attach_body(
    tx: &mut CoordinationTxn,
    role_instance: &str,
    session_id: Uuid,
    ttl: Duration,
) -> Result<WriteResult<Uuid>, StorageFailure> {
    // (1) Identifier rule — refuse before minting anything (R-0064-a).
    if validate_role_instance(role_instance).is_err() {
        return Ok(WriteResult::Refuse(Refusal::InvalidRoleInstance));
    }

    let workspace_id = tx.workspace_id();

    // (2) Resolve-or-create the actor on this txn.
    let actor = resolve_or_create_in_txn(tx.conn(), workspace_id, ActorType::Agent, role_instance)
        .await
        .map_err(|e| StorageFailure(Box::new(e)))?;

    // (3) Only agent rows are attachable (R-0064-c).
    if actor.actor_type != ActorType::Agent {
        return Ok(WriteResult::Refuse(Refusal::WrongActorType));
    }

    // (3a) attachment_mismatch (R-0064-e): one session, one role-instance. If
    // THIS session already holds a LIVE attachment to a DIFFERENT actor (a
    // different role-instance), refuse `attachment_mismatch` rather than
    // fresh-attach the new identity. "Live" is non-terminal AND unexpired at the
    // store clock — an expired prior attachment is stale (its slot is up for
    // succession), so it does not lock the session out of re-declaring. The
    // same-session same-role-instance re-poll resolves the SAME actor
    // (`holder_actor_id = actor.id`), is excluded by the `<>` predicate, and
    // falls through to the renew/succession fork below — d-green is not regressed.
    let (session_bound_elsewhere,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (
            SELECT 1 FROM leases
             WHERE workspace_id = $1
               AND session_id = $2
               AND holder_actor_id <> $3
               AND resource LIKE 'actor:%'
               AND terminal_state IS NULL
               AND expires_at > now()
         )",
    )
    .bind(workspace_id)
    .bind(session_id)
    .bind(actor.id)
    .fetch_one(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;
    if session_bound_elsewhere {
        return Ok(WriteResult::Refuse(Refusal::AttachmentMismatch));
    }

    // R-0075-a op-log attribution: the acting actor is resolved now — every fork
    // below attributes to it.
    tx.record_acting_actor(actor.id);

    let resource = format!("actor:{}", actor.id);
    let duration_secs = ttl.as_secs() as i64;

    // (4) Fork on the actor's current attachment. `FOR UPDATE` locks the live
    // lease so a concurrent successor serializes on it; `now()` is read in the
    // same query as the store transaction clock evaluating staleness (R-0065-e).
    let live: Option<LiveAttachmentFork> = sqlx::query_as(
        "SELECT id, session_id, expires_at, now()
           FROM leases
          WHERE workspace_id = $1
            AND resource = $2
            AND terminal_state IS NULL
          FOR UPDATE",
    )
    .bind(workspace_id)
    .bind(&resource)
    .fetch_optional(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    if let Some((prior_lease_id, prior_session, prior_expires_at, store_now)) = live {
        if store_now >= prior_expires_at {
            // (4a) SUCCESSION over a STALE attachment (any session — a
            // same-session idle-gap re-bind rides this SAME audited path).
            return succeed_via_takeover(
                tx,
                SuccessionCtx {
                    workspace_id,
                    actor_id: actor.id,
                    resource: &resource,
                    prior_lease_id,
                    prior_session,
                    prior_expires_at,
                    store_now,
                    successor_session: session_id,
                    duration_secs,
                },
            )
            .await;
        }
        // Prior attachment is still LIVE (not expired).
        if prior_session == Some(session_id) {
            // (4b) RENEW in place — a same-session re-poll before expiry. Extend
            // the existing lease's expiry from the store clock: same lease id, no
            // new row, no succession audit (R-0064-e / plan TTL renew).
            let renewed_expires = store_now + chrono::TimeDelta::seconds(duration_secs);
            sqlx::query("UPDATE leases SET expires_at = $1 WHERE id = $2")
                .bind(renewed_expires)
                .bind(prior_lease_id)
                .execute(tx.conn())
                .await
                .map_err(|e| StorageFailure(Box::new(e)))?;
            return Ok(WriteResult::Commit(actor.id));
        }
        // (4c) A DIFFERENT session while the attachment is LIVE → refuse
        // `actor_live_attached` (succession may not fire before expiry).
        return Ok(WriteResult::Refuse(Refusal::ActorLiveAttached));
    }

    // (5) No live attachment → FRESH ATTACH (the mint's first attachment). Lease
    // window from the store transaction clock (R-0065-e — no host-local clock).
    let now: (DateTime<Utc>,) = sqlx::query_as("SELECT now()")
        .fetch_one(tx.conn())
        .await
        .map_err(|e| StorageFailure(Box::new(e)))?;
    let acquired_at = now.0;
    let expires_at = acquired_at + chrono::TimeDelta::seconds(duration_secs);

    // Insert the attachment-as-lease. A 23505 on `leases_live_resource_uq` means
    // a live attachment already exists for this actor (a lost concurrent race) →
    // refuse `actor_live_attached`; any other error is a genuine storage failure.
    let insert = sqlx::query(
        "INSERT INTO leases
             (workspace_id, resource, holder_actor_id, acquired_at, duration, expires_at, session_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(workspace_id)
    .bind(&resource)
    .bind(actor.id)
    .bind(acquired_at)
    .bind(duration_secs)
    .bind(expires_at)
    .bind(session_id)
    .execute(tx.conn())
    .await;

    if let Err(e) = insert {
        let unique_violation =
            e.as_database_error().and_then(|db| db.code()).as_deref() == Some("23505");
        if unique_violation {
            return Ok(WriteResult::Refuse(Refusal::ActorLiveAttached));
        }
        return Err(StorageFailure(Box::new(e)));
    }

    // (6) Audit registration + attachment, staged for the in-COMMIT outbox flush
    // (R-0075-b/-c). The audit `workspace_id` is the machinery-authoritative
    // tenant scope, never body-supplied.
    tx.stage_audit(AuditRecord::registration(
        workspace_id,
        actor.id,
        role_instance,
    ));
    tx.stage_audit(AuditRecord::attachment(workspace_id, actor.id, session_id));

    Ok(WriteResult::Commit(actor.id))
}

/// The inputs to a [`succeed_via_takeover`] succession, grouped so the helper's
/// signature stays readable (and clear of `clippy::too_many_arguments`).
struct SuccessionCtx<'a> {
    /// Machinery-authoritative tenant scope (R-0076-b).
    workspace_id: Uuid,
    /// The actor being handed from the prior holder to the successor.
    actor_id: Uuid,
    /// The reserved `actor:<id>` lease resource (shared by both lease rows).
    resource: &'a str,
    /// The expired prior lease being superseded.
    prior_lease_id: Uuid,
    /// The prior lease's session (the superseded holder — becomes the audit's
    /// `prior_session`; equals `successor_session` on the same-session idle-gap
    /// path).
    prior_session: Option<Uuid>,
    /// The prior lease's declared expiry (succession evidence).
    prior_expires_at: DateTime<Utc>,
    /// The store-clock instant that observed the prior lease past expiry
    /// (`store_now >= prior_expires_at`; the successor window's start).
    store_now: DateTime<Utc>,
    /// The successor (binding) session taking over.
    successor_session: Uuid,
    /// The attachment TTL, in seconds, for the successor lease window.
    duration_secs: i64,
}

/// Take over a STALE attachment: supersede the prior lease and insert the
/// successor in one txn, staging an `attachment_succession` audit (R-0064-d).
///
/// Ordering is load-bearing. The prior lease is marked `taken_over` FIRST —
/// which drops it out of the `terminal_state IS NULL` partial unique index,
/// freeing the `leases_live_resource_uq` slot — and the successor is inserted
/// SECOND. `superseded_by` is the pre-generated successor lease id, so the
/// supersession chain is set in the one UPDATE (no second pass). A 23505 on the
/// successor insert means a concurrent successor already occupied the freed slot
/// → refuse `actor_live_attached` (a lost succession race is a refusal, never
/// `Unavailable`). NO `registration` audit is staged: the actor pre-exists, so
/// this path forks BEFORE fresh-attach registration-staging.
async fn succeed_via_takeover(
    tx: &mut CoordinationTxn,
    ctx: SuccessionCtx<'_>,
) -> Result<WriteResult<Uuid>, StorageFailure> {
    let prior_session = match ctx.prior_session {
        Some(s) => s,
        // A live attachment lease is always inserted with a session_id (both the
        // fresh-attach and succession inserts bind it), so a NULL here is a
        // storage anomaly — fail closed rather than fabricate succession evidence.
        None => {
            return Err(StorageFailure(
                "live attachment lease missing session_id on succession".into(),
            ));
        }
    };

    let successor_lease_id = Uuid::new_v4();
    let successor_expires = ctx.store_now + chrono::TimeDelta::seconds(ctx.duration_secs);

    // Supersede the prior lease FIRST — `taken_over` drops it out of the partial
    // unique index, freeing the slot for the successor insert. `superseded_by` is
    // the pre-generated successor id, so the chain is set in this one UPDATE.
    sqlx::query(
        "UPDATE leases
            SET terminal_state = 'taken_over',
                terminated_at  = $1,
                superseded_by  = $2
          WHERE id = $3",
    )
    .bind(ctx.store_now)
    .bind(successor_lease_id)
    .bind(ctx.prior_lease_id)
    .execute(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    // Insert the successor lease with the pre-generated id. A 23505 means a
    // concurrent successor already occupied the freed slot → refuse
    // `actor_live_attached` (same catch as the fresh-attach race).
    let insert = sqlx::query(
        "INSERT INTO leases
             (id, workspace_id, resource, holder_actor_id, acquired_at, duration, expires_at, session_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(successor_lease_id)
    .bind(ctx.workspace_id)
    .bind(ctx.resource)
    .bind(ctx.actor_id)
    .bind(ctx.store_now)
    .bind(ctx.duration_secs)
    .bind(successor_expires)
    .bind(ctx.successor_session)
    .execute(tx.conn())
    .await;

    if let Err(e) = insert {
        let unique_violation =
            e.as_database_error().and_then(|db| db.code()).as_deref() == Some("23505");
        if unique_violation {
            return Ok(WriteResult::Refuse(Refusal::ActorLiveAttached));
        }
        return Err(StorageFailure(Box::new(e)));
    }

    // Succession audit ONLY — no `registration` (the actor pre-exists). Carries
    // the prior + successor sessions (equal on the same-session idle-gap path)
    // and the store-clock expiry evidence that authorized the takeover.
    tx.stage_audit(AuditRecord::attachment_succession(
        ctx.workspace_id,
        ctx.actor_id,
        prior_session,
        ctx.successor_session,
        ExpiryEvidence {
            expires_at: ctx.prior_expires_at,
            observed_now: ctx.store_now,
        },
    ));

    Ok(WriteResult::Commit(ctx.actor_id))
}

/// The `poll` response body (R-0072-a, spec §API Contract): the bound actor,
/// the polling actor's non-terminal messages, and the workspace's live leases.
/// Returned on EVERY non-refused poll outcome — fresh attach, same-session
/// renew, and audited succession alike.
///
/// `messages` is the actor's non-terminal queue (`sent → delivered` already
/// applied), read by [`crate::coordination::messages::deliver_and_list`]
/// (Task 7 slice b) — each entry is the §API Contract message object.
/// `live_leases` is already workspace-scoped and `actor:`-family excluded by
/// the read ([`PgCoordinationStore::live_leases_for_workspace`]); each entry
/// renders the documented lease object `{ lease_id, resource, holder:
/// {actor_id, role_instance}, acquired_at, expires_at }`.
fn poll_response(
    actor_id: Uuid,
    role_instance: &str,
    messages: Vec<serde_json::Value>,
    live_leases: Vec<LiveLeaseRow>,
) -> CallToolResult {
    let leases: Vec<serde_json::Value> = live_leases
        .iter()
        .map(|l| {
            serde_json::json!({
                "lease_id": l.lease_id.to_string(),
                "resource": l.resource,
                "holder": {
                    "actor_id": l.holder_actor_id.to_string(),
                    "role_instance": l.holder_role_instance,
                },
                "acquired_at": l.acquired_at.to_rfc3339(),
                "expires_at": l.expires_at.to_rfc3339(),
            })
        })
        .collect();

    CallToolResult::structured(serde_json::json!({
        "actor": {
            "actor_id": actor_id.to_string(),
            "role_instance": role_instance,
        },
        "messages": messages,
        "live_leases": leases,
    }))
}

/// Render a [`Refusal`] as the spec §API Contract structured refusal envelope
/// (`{ refused: true, reason_code, detail }`). `reason_code` is the closed
/// machine code; `detail` carries no store internals. `pub(crate)`: every
/// `claim`/`message` action body (e.g. `coordination::leases::acquire`)
/// shares this one renderer.
pub(crate) fn refusal_result(refusal: &Refusal) -> CallToolResult {
    refusal_result_with_detail(refusal, refusal_detail(refusal))
}

/// As [`refusal_result`], but the caller supplies `detail` directly instead
/// of letting [`refusal_detail`]'s generic, Refusal-variant-only rendering
/// produce it — used when the caller holds more specific information than
/// the closed [`Refusal`] enum can carry (e.g. `claim acquire`'s
/// `invalid_resource` detail naming the specific `resource_id::parse`
/// failure, R-0067-a; see `coordination::leases::acquire`). `reason_code` is
/// unaffected either way — always the closed, machine
/// [`Refusal::reason_code`].
pub(crate) fn refusal_result_with_detail(
    refusal: &Refusal,
    detail: serde_json::Value,
) -> CallToolResult {
    CallToolResult::structured(serde_json::json!({
        "refused": true,
        "reason_code": refusal.reason_code(),
        "detail": detail,
    }))
}

/// A refusal's `detail` (§API Contract error taxonomy). Usually a
/// human-facing explanation string; `ResourceHeld` (R-0065-c) instead carries
/// a structured object — the holder's `actor_id` and lease `expires_at` (the
/// "workspace-visible facts" the spec requires the loser to receive) — so a
/// caller can read the contended resource's holder and expiry
/// programmatically, not just prose.
fn refusal_detail(refusal: &Refusal) -> serde_json::Value {
    match refusal {
        Refusal::InvalidRoleInstance => serde_json::json!(
            "the role_instance failed the identifier rule (must be non-empty, \
             length-bounded, and free of whitespace and control characters)"
        ),
        Refusal::WrongActorType => serde_json::json!(
            "the named identity is not an agent actor; only agent actors are attachable"
        ),
        Refusal::ActorLiveAttached => {
            serde_json::json!("the actor already has a live attachment")
        }
        Refusal::AttachmentMismatch => serde_json::json!(
            "the session is already attached as a different role-instance; \
             one session binds one role-instance"
        ),
        Refusal::NotAttached => {
            serde_json::json!("the session holds no live attachment; bind via `message poll` first")
        }
        Refusal::InvalidResource => serde_json::json!(
            "the resource identifier is malformed, or names a family outside the closed set"
        ),
        Refusal::ReservedFamily => serde_json::json!(
            "the `actor` resource family is reserved and barred from the entire claim surface"
        ),
        Refusal::InvalidDuration => {
            serde_json::json!("duration_seconds must be a positive value within the policy maximum")
        }
        Refusal::ResourceHeld {
            holder_actor_id,
            expires_at,
        } => serde_json::json!({
            "holder_actor_id": holder_actor_id.to_string(),
            "expires_at": expires_at.to_rfc3339(),
        }),
        Refusal::NotHolder => serde_json::json!(
            "the caller does not hold this lease; only the current holder may renew or \
             release it"
        ),
        Refusal::LeaseNotFound => serde_json::json!(
            "no live lease matches the given lease_id (it never existed, or is expired, \
             released, or taken over — an expired hold is not revivable; use `takeover`)"
        ),
        Refusal::NotExpired => serde_json::json!(
            "the lease has not expired yet; a live lease cannot be taken over — no \
             force-break verb exists at V0 (R-0066-a); wait out the TTL"
        ),
        // `send` (Task 7 slice a) makes these two reachable — split out of
        // the "not yet wired" group below with their own accurate detail.
        Refusal::SchemaViolation => serde_json::json!(
            "the send payload failed to validate against its named type + schema_version's \
             schema (an undeclared field, a wrong-typed value, or a missing required field)"
        ),
        Refusal::UnknownType => serde_json::json!(
            "the send named a (type, schema_version) pair that is not a registered message type"
        ),
        // Message-family refusals (`ack`/`disposition` — R-0069; Task 7
        // slice b makes these reachable) — each split out with its own
        // accurate detail, same posture as `send`'s `SchemaViolation`/
        // `UnknownType` arm above.
        Refusal::NotAddressee => serde_json::json!(
            "the caller's attached actor is not this message's addressee; `ack`/`disposition` \
             are addressee-only"
        ),
        Refusal::InvalidTransition => serde_json::json!(
            "the message is not in a state from which this action is a legal transition (the \
             lifecycle is monotonic — no reverse or re-transition; `ack` requires `delivered`, \
             `disposition` requires `acknowledged`)"
        ),
        Refusal::InvalidDisposition => serde_json::json!(
            "the disposition value is outside the closed vocabulary {completed, declined, \
             obsolete}"
        ),
        Refusal::MessageNotFound => {
            serde_json::json!("no message matches the given message_id in this workspace")
        } // Deliberately NO wildcard arm: `Refusal` is a closed enum
          // (write_path.rs), not `#[non_exhaustive]`, so this match is
          // exhaustive over every current variant. A later slice that makes
          // one of the arms above reachable — or the enum gaining a new
          // variant — is a compile error here, not a silently generic detail.
    }
}

/// The fail-closed stop surfaced when a coordination write cannot be verified
/// (R-0074-a). Carries no store internals (no-leak posture). `pub(crate)`:
/// shared by every coordination action body.
pub(crate) fn coordination_unavailable() -> ErrorData {
    ErrorData {
        code: rmcp::model::ErrorCode::INTERNAL_ERROR,
        message: "coordination write unavailable".into(),
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------
    // Advertisement — `message` is advertised with the closed `action` +
    // `role_instance` schema and NO acting-actor field (R-0064-b).
    // -----------------------------------------------------------------

    #[test]
    fn message_tool_advertised_with_action_role_instance_and_no_acting_actor() {
        let tools = coordination_tools();
        let msg = tools
            .iter()
            .find(|t| t.name.as_ref() == "message")
            .expect("coordination_tools must advertise the `message` tool");

        let props = msg
            .input_schema
            .get("properties")
            .and_then(|v| v.as_object())
            .expect("the message tool schema must declare a non-empty `properties` object");

        assert!(
            props.contains_key("action"),
            "the message tool schema must declare the closed `action` argument"
        );
        assert!(
            props.contains_key("role_instance"),
            "the message tool schema must declare the `role_instance` argument"
        );
        for send_arg in ["to_role_instance", "type", "schema_version", "payload"] {
            assert!(
                props.contains_key(send_arg),
                "the message tool schema must declare `send`'s `{send_arg}` argument \
                 (Task 7 slice a)"
            );
        }
        for ack_disposition_arg in ["message_id", "disposition", "note"] {
            assert!(
                props.contains_key(ack_disposition_arg),
                "the message tool schema must declare `ack`/`disposition`'s \
                 `{ack_disposition_arg}` argument (Task 7 slice b)"
            );
        }

        // R-0064-b: the schema carries NO acting-actor field — the principal is
        // host-derived from attachment state, never a caller write input.
        for forbidden in ["actor", "actor_id", "acting_actor", "principal", "sender"] {
            assert!(
                !props.contains_key(forbidden),
                "R-0064-b: the coordination tool schema MUST NOT carry an \
                 acting-actor field (found `{forbidden}`)"
            );
        }

        // `poll`, `send`, `ack`, and `disposition` are advertised at this
        // stage (closed enum; Task 7 slice b adds `ack`/`disposition`).
        let action_enum = props
            .get("action")
            .and_then(|a| a.get("enum"))
            .and_then(|e| e.as_array())
            .expect("the `action` argument must declare a closed enum");
        assert_eq!(
            action_enum,
            &vec![
                serde_json::json!("poll"),
                serde_json::json!("send"),
                serde_json::json!("ack"),
                serde_json::json!("disposition"),
            ],
            "the advertised `message` actions at this stage are `poll`, `send`, `ack`, and \
             `disposition`"
        );
    }

    #[test]
    fn claim_tool_advertised_with_action_resource_duration_and_no_acting_actor() {
        let tools = coordination_tools();
        let claim = tools
            .iter()
            .find(|t| t.name.as_ref() == "claim")
            .expect("coordination_tools must advertise the `claim` tool");

        let props = claim
            .input_schema
            .get("properties")
            .and_then(|v| v.as_object())
            .expect("the claim tool schema must declare a non-empty `properties` object");

        assert!(
            props.contains_key("action"),
            "the claim tool schema must declare the closed `action` argument"
        );
        assert!(
            props.contains_key("resource"),
            "the claim tool schema must declare the `resource` argument"
        );
        assert!(
            props.contains_key("duration_seconds"),
            "the claim tool schema must declare the optional `duration_seconds` argument"
        );
        assert!(
            props.contains_key("family"),
            "the claim tool schema must declare the optional `family` list-filter argument"
        );
        assert!(
            props.contains_key("resource_prefix"),
            "the claim tool schema must declare the optional `resource_prefix` list-filter \
             argument"
        );
        assert!(
            props.contains_key("lease_id"),
            "the claim tool schema must declare the `lease_id` argument (`renew`/`release`, \
             Task 5 slice c)"
        );

        // R-0064-b: the schema carries NO acting-actor field — the principal is
        // host-derived from attachment state, never a caller write input.
        for forbidden in [
            "actor",
            "actor_id",
            "acting_actor",
            "principal",
            "sender",
            "holder",
        ] {
            assert!(
                !props.contains_key(forbidden),
                "R-0064-b: the coordination tool schema MUST NOT carry an \
                 acting-actor field (found `{forbidden}`)"
            );
        }

        // `acquire`, `list`, `renew`, `release`, and `takeover` are
        // advertised at slice d (closed enum).
        let action_enum = props
            .get("action")
            .and_then(|a| a.get("enum"))
            .and_then(|e| e.as_array())
            .expect("the `action` argument must declare a closed enum");
        assert_eq!(
            action_enum,
            &vec![
                serde_json::json!("acquire"),
                serde_json::json!("list"),
                serde_json::json!("renew"),
                serde_json::json!("release"),
                serde_json::json!("takeover"),
            ],
            "the advertised `claim` actions at slice d are `acquire`, `list`, `renew`, \
             `release`, and `takeover`"
        );
    }

    // -----------------------------------------------------------------
    // Action parsing — closed set; defensive against client-supplied shape;
    // tool-aware (Task 5 b1 Q1 decision).
    // -----------------------------------------------------------------

    #[test]
    fn parse_action_accepts_message_poll() {
        let mut args = serde_json::Map::new();
        args.insert("action".to_string(), serde_json::json!("poll"));
        assert_eq!(
            parse_action(MESSAGE_TOOL, Some(&args)).expect("`poll` is a valid `message` action"),
            CoordinationAction::Poll
        );
    }

    #[test]
    fn parse_action_accepts_message_send() {
        let mut args = serde_json::Map::new();
        args.insert("action".to_string(), serde_json::json!("send"));
        assert_eq!(
            parse_action(MESSAGE_TOOL, Some(&args)).expect("`send` is a valid `message` action"),
            CoordinationAction::Send
        );
    }

    #[test]
    fn parse_action_accepts_message_ack() {
        let mut args = serde_json::Map::new();
        args.insert("action".to_string(), serde_json::json!("ack"));
        assert_eq!(
            parse_action(MESSAGE_TOOL, Some(&args)).expect("`ack` is a valid `message` action"),
            CoordinationAction::Ack
        );
    }

    #[test]
    fn parse_action_accepts_message_disposition() {
        let mut args = serde_json::Map::new();
        args.insert("action".to_string(), serde_json::json!("disposition"));
        assert_eq!(
            parse_action(MESSAGE_TOOL, Some(&args))
                .expect("`disposition` is a valid `message` action"),
            CoordinationAction::Disposition
        );
    }

    #[test]
    fn parse_action_accepts_claim_acquire() {
        let mut args = serde_json::Map::new();
        args.insert("action".to_string(), serde_json::json!("acquire"));
        assert_eq!(
            parse_action(CLAIM_TOOL, Some(&args)).expect("`acquire` is a valid `claim` action"),
            CoordinationAction::ClaimAcquire
        );
    }

    #[test]
    fn parse_action_accepts_claim_list() {
        let mut args = serde_json::Map::new();
        args.insert("action".to_string(), serde_json::json!("list"));
        assert_eq!(
            parse_action(CLAIM_TOOL, Some(&args)).expect("`list` is a valid `claim` action"),
            CoordinationAction::ClaimList
        );
    }

    #[test]
    fn parse_action_accepts_claim_renew() {
        let mut args = serde_json::Map::new();
        args.insert("action".to_string(), serde_json::json!("renew"));
        assert_eq!(
            parse_action(CLAIM_TOOL, Some(&args)).expect("`renew` is a valid `claim` action"),
            CoordinationAction::ClaimRenew
        );
    }

    #[test]
    fn parse_action_accepts_claim_release() {
        let mut args = serde_json::Map::new();
        args.insert("action".to_string(), serde_json::json!("release"));
        assert_eq!(
            parse_action(CLAIM_TOOL, Some(&args)).expect("`release` is a valid `claim` action"),
            CoordinationAction::ClaimRelease
        );
    }

    #[test]
    fn parse_action_accepts_claim_takeover() {
        let mut args = serde_json::Map::new();
        args.insert("action".to_string(), serde_json::json!("takeover"));
        assert_eq!(
            parse_action(CLAIM_TOOL, Some(&args)).expect("`takeover` is a valid `claim` action"),
            CoordinationAction::ClaimTakeover
        );
    }

    #[test]
    fn parse_action_rejects_action_under_the_wrong_tool() {
        // `acquire` is a `claim` action, not a `message` action, and vice
        // versa — tool-aware parsing rejects a valid action name paired with
        // the wrong tool.
        let mut acquire = serde_json::Map::new();
        acquire.insert("action".to_string(), serde_json::json!("acquire"));
        assert!(
            parse_action(MESSAGE_TOOL, Some(&acquire)).is_err(),
            "`acquire` is not a `message` action"
        );

        let mut poll = serde_json::Map::new();
        poll.insert("action".to_string(), serde_json::json!("poll"));
        assert!(
            parse_action(CLAIM_TOOL, Some(&poll)).is_err(),
            "`poll` is not a `claim` action"
        );
    }

    #[test]
    fn parse_action_rejects_unsupported_missing_and_non_string() {
        // An action not yet supported at this stage. `ack`/`disposition`
        // became supported in Task 7 slice b (see the dedicated
        // `parse_action_accepts_message_ack`/`_disposition` tests below), so
        // this test now anchors on `list` — the next `message` action,
        // landing in Task 7 slice c — as the still-genuinely-unsupported
        // case.
        let mut list = serde_json::Map::new();
        list.insert("action".to_string(), serde_json::json!("list"));
        assert!(
            parse_action(MESSAGE_TOOL, Some(&list)).is_err(),
            "an unsupported action must be refused, not silently accepted"
        );

        // Absent arguments.
        assert!(
            parse_action(MESSAGE_TOOL, None).is_err(),
            "a call with no arguments has no `action` and must be refused"
        );

        // Present arguments, absent `action`.
        assert!(
            parse_action(MESSAGE_TOOL, Some(&serde_json::Map::new())).is_err(),
            "a missing `action` argument must be refused"
        );

        // Non-string `action`.
        let mut wrong_type = serde_json::Map::new();
        wrong_type.insert("action".to_string(), serde_json::json!(7));
        assert!(
            parse_action(MESSAGE_TOOL, Some(&wrong_type)).is_err(),
            "a non-string `action` argument must be refused"
        );
    }

    // -----------------------------------------------------------------
    // Identifier rule — the one host-registered role-instance validator
    // (R-0064-a), shared by bind + send-side. Pure function → unit-tested here.
    // -----------------------------------------------------------------

    #[test]
    fn validate_role_instance_accepts_conforming_and_rejects_bad() {
        // Conforming identifiers (the shapes the bind/send paths mint against).
        for good in ["merger", "design-lane", "merger-42", "reviewer_1"] {
            assert!(
                validate_role_instance(good).is_ok(),
                "`{good}` conforms to the identifier rule"
            );
        }
        // Empty, whitespace, and control characters are refused
        // `invalid_role_instance` (R-0064-a) — before any actor row is minted.
        for bad in ["", "merger lane", "line\nbreak", "tab\tted"] {
            assert_eq!(
                validate_role_instance(bad),
                Err(Refusal::InvalidRoleInstance),
                "`{bad:?}` must be refused invalid_role_instance"
            );
        }
    }
}
