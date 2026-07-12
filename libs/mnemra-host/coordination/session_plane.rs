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
use crate::coordination::write_path::{
    CoordinationTxn, LiveLeaseRow, PgCoordinationStore, Refusal, StorageFailure, WriteResult,
};

/// The host-served coordination tool name for the messaging surface. Task 5
/// adds the sibling `claim` tool through this same host-served branch.
pub(crate) const MESSAGE_TOOL: &str = "message";

/// Returns `true` if `name` is a host-served coordination tool — the ones
/// `call_tool` routes to the coordination branch *before* the echo-verb
/// manifest membership gate (they are not echo verbs, so that gate would
/// otherwise reject them).
pub(crate) fn is_coordination_tool(name: &str) -> bool {
    name == MESSAGE_TOOL
}

/// A coordination action, parsed from a tool call's `action` argument.
///
/// Closed set. At this stage `message` advertises only `poll`; Task 5 (`claim`)
/// and Task 7 (`message send`/`list`/`ack`/`disposition`) extend the set. Every
/// coordination action is write-category under R-0073-b — the classification
/// lives in [`crate::mcp::dispatch::authorize_coordination_action`], which reads
/// the *action*, never the tool-name tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoordinationAction {
    /// `message poll` — the bind call (R-0064-e). The real bind/attach/delivery
    /// body lands in a later sub-run; this stage routes it to a placeholder.
    Poll,
}

/// Parse the closed `action` argument from a coordination tool call.
///
/// The advertised input schema constrains `action` to the closed enum, but the
/// host re-parses defensively — client-supplied structure is never trusted. An
/// absent, non-string, or out-of-set `action` is a malformed request and maps
/// to `INVALID_PARAMS`. (At this stage only `poll` is a valid `message` action;
/// `send`/`list`/`ack`/`disposition` are added by Task 7.)
pub(crate) fn parse_action(
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

    match action {
        "poll" => Ok(CoordinationAction::Poll),
        other => Err(ErrorData {
            code: rmcp::model::ErrorCode::INVALID_PARAMS,
            message: format!("unsupported coordination action '{other}'").into(),
            data: None,
        }),
    }
}

/// The host-served coordination tools advertised by `list_tools`, concatenated
/// with the plugin (echo) verbs. At this stage: `message` with the `poll`
/// action only.
///
/// The input schema declares the closed `action` enum + `role_instance`. It
/// carries **no acting-actor field** (R-0064-b): the acting principal is
/// host-derived from attachment state; a caller-supplied principal does not
/// exist anywhere on the surface.
pub(crate) fn coordination_tools() -> Vec<Tool> {
    vec![message_tool()]
}

/// Build the `message` tool advertisement (poll action, R-0064-b schema).
fn message_tool() -> Tool {
    // `action` is the closed enum argument (poll only at this stage);
    // `role_instance` is the bind identifier. `additionalProperties: false`
    // makes the closed shape explicit; there is deliberately NO acting-actor
    // field (R-0064-b).
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["poll"],
                "description": "The coordination action (closed set)."
            },
            "role_instance": {
                "type": "string",
                "description": "The role-instance identifier the session binds to on poll."
            }
        },
        "required": ["action", "role_instance"],
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
            // Deliver half (R-0072-a): the bind committed; read the workspace's
            // live leases (post-commit, workspace-scoped, `actor:`-family
            // excluded) and render the documented poll response on EVERY
            // non-refused outcome — fresh attach, same-session renew, and
            // audited succession alike. A read failure is fail-closed (R-0074-a),
            // the same posture as a write-path unavailability.
            let live_leases = store
                .live_leases_for_workspace(ctx)
                .await
                .map_err(|_| coordination_unavailable())?;
            Ok(poll_response(actor_id, &role_owned, live_leases))
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
/// `messages` is EMPTY at Task 4: no `send` exists yet, so the polling actor's
/// queue is empty (the delivery half lands Task 7). `live_leases` is already
/// workspace-scoped and `actor:`-family excluded by the read
/// ([`PgCoordinationStore::live_leases_for_workspace`]); each entry renders the
/// documented lease object `{ lease_id, resource, holder: {actor_id,
/// role_instance}, acquired_at, expires_at }`.
fn poll_response(
    actor_id: Uuid,
    role_instance: &str,
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
        "messages": [],
        "live_leases": leases,
    }))
}

/// Render a [`Refusal`] as the spec §API Contract structured refusal envelope
/// (`{ refused: true, reason_code, detail }`). `reason_code` is the closed
/// machine code; `detail` is human-facing and carries no store internals.
fn refusal_result(refusal: &Refusal) -> CallToolResult {
    CallToolResult::structured(serde_json::json!({
        "refused": true,
        "reason_code": refusal.reason_code(),
        "detail": refusal_detail(refusal),
    }))
}

/// A human-facing explanation for each refusal reachable on the bind path.
fn refusal_detail(refusal: &Refusal) -> &'static str {
    match refusal {
        Refusal::InvalidRoleInstance => {
            "the role_instance failed the identifier rule (must be non-empty, \
             length-bounded, and free of whitespace and control characters)"
        }
        Refusal::WrongActorType => {
            "the named identity is not an agent actor; only agent actors are attachable"
        }
        Refusal::ActorLiveAttached => "the actor already has a live attachment",
        Refusal::AttachmentMismatch => {
            "the session is already attached as a different role-instance; \
             one session binds one role-instance"
        }
        // Refusals not produced on the bind path; the machine reason_code
        // still carries the precise code for any caller.
        _ => "the coordination bind was refused",
    }
}

/// The fail-closed stop surfaced when a coordination write cannot be verified
/// (R-0074-a). Carries no store internals (no-leak posture).
fn coordination_unavailable() -> ErrorData {
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

        // R-0064-b: the schema carries NO acting-actor field — the principal is
        // host-derived from attachment state, never a caller write input.
        for forbidden in ["actor", "actor_id", "acting_actor", "principal", "sender"] {
            assert!(
                !props.contains_key(forbidden),
                "R-0064-b: the coordination tool schema MUST NOT carry an \
                 acting-actor field (found `{forbidden}`)"
            );
        }

        // Only `poll` is advertised at this stage (closed enum).
        let action_enum = props
            .get("action")
            .and_then(|a| a.get("enum"))
            .and_then(|e| e.as_array())
            .expect("the `action` argument must declare a closed enum");
        assert_eq!(
            action_enum,
            &vec![serde_json::json!("poll")],
            "the only advertised `message` action at this stage is `poll`"
        );
    }

    // -----------------------------------------------------------------
    // Action parsing — closed set; defensive against client-supplied shape.
    // -----------------------------------------------------------------

    #[test]
    fn parse_action_accepts_poll() {
        let mut args = serde_json::Map::new();
        args.insert("action".to_string(), serde_json::json!("poll"));
        assert_eq!(
            parse_action(Some(&args)).expect("`poll` is a valid action"),
            CoordinationAction::Poll
        );
    }

    #[test]
    fn parse_action_rejects_unsupported_missing_and_non_string() {
        // An action not yet supported at this stage (Task 7's `send`).
        let mut send = serde_json::Map::new();
        send.insert("action".to_string(), serde_json::json!("send"));
        assert!(
            parse_action(Some(&send)).is_err(),
            "an unsupported action must be refused, not silently accepted"
        );

        // Absent arguments.
        assert!(
            parse_action(None).is_err(),
            "a call with no arguments has no `action` and must be refused"
        );

        // Present arguments, absent `action`.
        assert!(
            parse_action(Some(&serde_json::Map::new())).is_err(),
            "a missing `action` argument must be refused"
        );

        // Non-string `action`.
        let mut wrong_type = serde_json::Map::new();
        wrong_type.insert("action".to_string(), serde_json::json!(7));
        assert!(
            parse_action(Some(&wrong_type)).is_err(),
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
