//! The `message` tool's `send` action (Task 7 slice a; R-0068-a, R-0070-b,
//! R-0075-b, R-0075-e).
//!
//! Decision hidden: **a `send` mints its addressee only after every
//! validation gate passes** (the send-ordering pin, §API Contract) — the
//! attach gate, the R-0064-a role-instance rule, and Task 6's closed-schema
//! validation all run BEFORE the resolve-or-create + insert, inside the SAME
//! [`crate::coordination::write_path::PgCoordinationStore::run_write`]
//! transaction, so a refusal at any earlier gate rolls the whole write back
//! — no addressee row, no registration audit. The registration audit itself
//! fires iff [`crate::builtins::actors::resolve_or_create_in_txn`] actually
//! minted a NEW row for the addressee (R-0075-b / FF-4) — an
//! already-existing addressee stages none.
//!
//! The sender is never a caller-supplied argument (R-0064-b) — it is the
//! calling session's live attached actor, resolved the same way every
//! `claim` action resolves its acting actor
//! ([`crate::coordination::leases::resolve_acting_actor`], private to its own
//! module — this file carries its own copy, per this codebase's established
//! per-file duplication convention for such private helpers, e.g.
//! `mcp_verb_gate.rs`'s own inlined `seed_read_observer_token`).
//!
//! The `read_observer` pre-dispatch denial (R-0073-b) is enforced upstream,
//! at [`crate::mcp::dispatch::authorize_coordination_action`], before
//! [`send`] is ever called — nothing to check here.
//!
//! # Slice (b) addendum — lifecycle machine + poll `sent→delivered` + `ack` +
//! `disposition` (Task 7 slice b)
//!
//! This module now also carries: the `sent→delivered` poll-delivery helper
//! ([`deliver_and_list`]/[`deliver_body`], called by
//! [`crate::coordination::session_plane::poll_bind`] as a SEPARATE
//! [`PgCoordinationStore::run_write`] call — `CoordinationOp::Poll`, distinct
//! from the bind's own `CoordinationOp::AttachBind` — AFTER the bind commits,
//! mirroring [`PgCoordinationStore::live_leases_for_workspace`]'s existing
//! post-commit read placement); the addressee-only `ack` action
//! ([`ack`]/[`ack_body`], `delivered → acknowledged`); and the addressee-only
//! `disposition` action ([`disposition`]/[`disposition_body`], `acknowledged
//! → dispositioned`, staging [`AuditRecord::disposition`]). All three reuse
//! [`resolve_acting_actor`] (above) for the shared `not_attached` gate and
//! [`lock_message_row`] for the shared `message_not_found`/row-lock read.
//! Delivery has no refusal grammar of its own (R-0069-a: it is a recorded
//! fact, not a caller-invocable action) — its `run_write` body always
//! commits.

use chrono::{DateTime, Utc};
use rmcp::model::{CallToolResult, ErrorData};
use uuid::Uuid;

use crate::auth::workspace_ctx::WorkspaceCtx;
use crate::builtins::actors::{ActorType, resolve_or_create_in_txn};
use crate::coordination::audit::AuditRecord;
use crate::coordination::message_types::{MessageValidationError, validate_message};
use crate::coordination::session_plane::{
    coordination_unavailable, refusal_result, validate_role_instance,
};
use crate::coordination::write_path::{
    CoordinationTxn, PgCoordinationStore, Refusal, StorageFailure, WriteResult,
};
use crate::coordination::{COORDINATION_TARGET, CoordinationOp};

/// Resolve the calling session's live attached actor (R-0064-e) — the send
/// path's own copy of the same "`actor:`-family lease, non-terminal,
/// unexpired at the store transaction clock" resolution
/// [`crate::coordination::leases::resolve_acting_actor`] performs for every
/// `claim` action; that function is private to its own module, so `send`
/// carries its own copy rather than widening `leases.rs`'s visibility
/// (forbid_scope).
///
/// On success, records the acting actor onto `tx`
/// ([`CoordinationTxn::record_acting_actor`]) so the op-log entry attributes
/// it (R-0075-a) — mirrors `leases::resolve_acting_actor` exactly.
///
/// Returns `Ok(Err(Refusal::NotAttached))` — a refusal the caller maps to
/// `WriteResult::Refuse`, not a storage failure — when no live attachment
/// exists.
async fn resolve_acting_actor(
    tx: &mut CoordinationTxn,
    session_id: Uuid,
) -> Result<Result<(Uuid, String), Refusal>, StorageFailure> {
    let workspace_id = tx.workspace_id();
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT l.holder_actor_id, a.name
           FROM leases l
           JOIN actors a
             ON a.id = l.holder_actor_id
            AND a.workspace_id = l.workspace_id
          WHERE l.workspace_id = $1
            AND l.session_id = $2
            AND l.resource LIKE 'actor:%'
            AND l.terminal_state IS NULL
            AND l.expires_at > now()",
    )
    .bind(workspace_id)
    .bind(session_id)
    .fetch_optional(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    match row {
        Some((actor_id, role_instance)) => {
            tx.record_acting_actor(actor_id);
            Ok(Ok((actor_id, role_instance)))
        }
        None => Ok(Err(Refusal::NotAttached)),
    }
}

/// The committed `send` result, rendered into the §API Contract response
/// (`{ message_id, state: "sent", sent_at }`) by [`send_response`].
struct SentMessage {
    message_id: Uuid,
    sent_at: DateTime<Utc>,
}

/// The `message send` action (R-0068-a, R-0070-b, R-0075-b; Task 7 slice a)
/// — routed from
/// [`crate::mcp::server::MnemraMcpServer::handle_coordination`].
///
/// `to_role_instance` is the addressee identifier; `type_name` +
/// `schema_version` name the registered message type (Task 6); `payload` is
/// the caller-supplied JSON payload, schema-validated inside [`send_body`].
/// There is no `sender` argument anywhere on this surface — the sender is
/// host-resolved from the calling session's live attachment (R-0064-b: no
/// caller-supplied acting-actor field).
pub(crate) async fn send(
    store: &PgCoordinationStore,
    ctx: &WorkspaceCtx,
    to_role_instance: &str,
    type_name: &str,
    schema_version: u16,
    payload: serde_json::Value,
) -> Result<CallToolResult, ErrorData> {
    let session_id = ctx.token_id;
    let to_role_instance_owned = to_role_instance.to_owned();
    let type_name_owned = type_name.to_owned();

    let write = store
        .run_write(
            ctx,
            CoordinationOp::Send,
            move |tx: &mut CoordinationTxn| {
                Box::pin(async move {
                    send_body(
                        tx,
                        session_id,
                        &to_role_instance_owned,
                        &type_name_owned,
                        schema_version,
                        &payload,
                    )
                    .await
                })
            },
        )
        .await;

    match write {
        Ok(WriteResult::Commit(sent)) => Ok(send_response(&sent)),
        Ok(WriteResult::Refuse(refusal)) => Ok(refusal_result(&refusal)),
        // Fail-closed (R-0074-a): `run_write` has already op-logged the
        // unavailability (R-0075-a); no store internals leak here.
        Err(_unavailable) => Err(coordination_unavailable()),
    }
}

/// The `send` state transition, run inside the `run_write` transaction.
///
/// Ordering is load-bearing — the send-ordering pin (§API Contract): a
/// refusal at any step below returns before the addressee is ever resolved
/// or minted.
/// 1. Resolve the acting actor — `not_attached` else the sender.
/// 2. (`read_observer` pre-dispatch denial, R-0073-b, already enforced
///    upstream — not re-checked here.)
/// 3. Closed-schema validation (Task 6) — `schema_violation`/`unknown_type`,
///    logged with DISCRETE structured fields (R-0075-e), else continue.
///    Deliberately BEFORE the role-instance check below: the acceptance
///    suite's own `unknown_type` fixtures (`coordination_messages.rs` test
///    3) embed non-identifier text into `to_role_instance` while asserting
///    `unknown_type`, not `invalid_role_instance` — so the closed-schema
///    gate is the one that must fire first (mirrors R-0070-b's own
///    version-before-schema ordering doctrine, extended one gate further
///    out). A deviation from the dispatch brief's prose ordering (which
///    listed role-instance before schema); the RED test's own assertions
///    are the authoritative contract per this dispatch's instructions —
///    flagged in the completion report.
/// 4. The one host-registered role-instance identifier rule (R-0064-a) on
///    `to_role_instance` — `invalid_role_instance` else continue.
/// 5. ONLY THEN: resolve-or-create the addressee and insert the `messages`
///    row, inside this SAME transaction — staging the registration audit
///    iff the addressee was actually just minted (R-0075-b / FF-4).
async fn send_body(
    tx: &mut CoordinationTxn,
    session_id: Uuid,
    to_role_instance: &str,
    type_name: &str,
    schema_version: u16,
    payload: &serde_json::Value,
) -> Result<WriteResult<SentMessage>, StorageFailure> {
    // (1) Every message action requires a live attachment first (R-0064-e)
    // — the send-ordering pin's first gate.
    let (sender_actor_id, _sender_role_instance) =
        match resolve_acting_actor(tx, session_id).await? {
            Ok(actor) => actor,
            Err(refusal) => return Ok(WriteResult::Refuse(refusal)),
        };

    // (3) Closed-schema validation (Task 6) — version-before-schema
    // ordering lives inside `validate_message` itself. A failure here is
    // refused BEFORE any addressee row is minted (the send-ordering pin)
    // and logged with DISCRETE structured fields (R-0075-e) — never
    // `MessageValidationError`'s `Display` string (a documented
    // log-injection / payload-echo hazard, `message_types.rs:155-163`).
    if let Err(err) = validate_message(type_name, schema_version, payload) {
        log_send_validation_refusal(&err);
        return Ok(WriteResult::Refuse(send_validation_refusal(&err)));
    }

    // (4) The one host-registered role-instance identifier rule (R-0064-a),
    // shared with the bind path — refuse before any addressee row is minted.
    if validate_role_instance(to_role_instance).is_err() {
        return Ok(WriteResult::Refuse(Refusal::InvalidRoleInstance));
    }

    // (5) ONLY THEN: resolve-or-create the addressee and insert the message
    // row, inside this SAME transaction — every refusal above returns
    // before this point, so a refused send mints no addressee and inserts
    // no row (the send-ordering pin, §API Contract).
    let workspace_id = tx.workspace_id();

    // Minted-ness is derived from a pre-existence check on this txn's own
    // connection, sequential with the `resolve_or_create_in_txn` call right
    // below (no race with ITSELF — both statements run on one connection,
    // one after the other). `resolve_or_create_in_txn` is forbid_scope
    // (reused unmodified) and returns no "did I insert" signal of its own,
    // so this is how the send path learns whether it just minted the
    // addressee (R-0075-b / FF-4's registration-audit-iff-minted
    // invariant).
    //
    // off-default note: a genuinely CONCURRENT send/poll racing another
    // send/poll to the SAME brand-new role_instance could both observe
    // "not yet existing" here before either commits, double-staging a
    // registration audit for what `resolve_or_create_in_txn`'s own `ON
    // CONFLICT DO NOTHING` still correctly resolves to exactly ONE actor
    // row. Closing this precisely would require `resolve_or_create_in_txn`
    // itself to report insert-vs-conflict (e.g. via `RETURNING`), which
    // lives in `builtins/actors.rs` — forbid_scope for this slice. Flagged
    // in the completion report as a follow-up.
    let (addressee_existed,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM actors WHERE workspace_id = $1 AND name = $2)",
    )
    .bind(workspace_id)
    .bind(to_role_instance)
    .fetch_one(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    let addressee =
        resolve_or_create_in_txn(tx.conn(), workspace_id, ActorType::Agent, to_role_instance)
            .await
            .map_err(|e| StorageFailure(Box::new(e)))?;

    let now: (DateTime<Utc>,) = sqlx::query_as("SELECT now()")
        .fetch_one(tx.conn())
        .await
        .map_err(|e| StorageFailure(Box::new(e)))?;
    let sent_at = now.0;

    let message_id: (Uuid,) = sqlx::query_as(
        "INSERT INTO messages
             (workspace_id, sender_actor_id, addressee_actor_id, message_type,
              schema_version, payload, state, sent_at)
         VALUES ($1, $2, $3, $4, $5, $6, 'sent', $7)
         RETURNING id",
    )
    .bind(workspace_id)
    .bind(sender_actor_id)
    .bind(addressee.id)
    .bind(type_name)
    .bind(i32::from(schema_version))
    .bind(payload)
    .bind(sent_at)
    .fetch_one(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    // R-0075-b / FF-4: stage the registration audit iff this call actually
    // minted the addressee — an already-existing addressee stages none.
    if !addressee_existed {
        tx.stage_audit(AuditRecord::registration(
            workspace_id,
            addressee.id,
            to_role_instance,
        ));
    }

    Ok(WriteResult::Commit(SentMessage {
        message_id: message_id.0,
        sent_at,
    }))
}

/// R-0075-e: log a `send` validation refusal (`schema_violation`/
/// `unknown_type`) with DISCRETE structured `tracing` fields — never
/// [`MessageValidationError`]'s `Display` impl, which the T6 module doc
/// (`message_types.rs:155-163`) flags as a log-injection / payload-echo
/// hazard that MUST NOT reach any audit/op-log path.
fn log_send_validation_refusal(err: &MessageValidationError) {
    match err {
        MessageValidationError::UnknownType {
            type_name,
            schema_version,
        } => {
            tracing::warn!(
                target: COORDINATION_TARGET,
                op = ?CoordinationOp::Send,
                code = err.code(),
                type_name = %type_name,
                schema_version = %schema_version,
                "coordination message send refused: unknown message type"
            );
        }
        MessageValidationError::SchemaViolation {
            type_name,
            schema_version,
            detail,
        } => {
            tracing::warn!(
                target: COORDINATION_TARGET,
                op = ?CoordinationOp::Send,
                code = err.code(),
                type_name = %type_name,
                schema_version = %schema_version,
                detail = %detail,
                "coordination message send refused: schema violation"
            );
        }
    }
}

/// Map a [`MessageValidationError`] to its [`Refusal`] variant (§API
/// Contract `reason_code`).
fn send_validation_refusal(err: &MessageValidationError) -> Refusal {
    match err {
        MessageValidationError::UnknownType { .. } => Refusal::UnknownType,
        MessageValidationError::SchemaViolation { .. } => Refusal::SchemaViolation,
    }
}

/// Render a committed `send` as the §API Contract response: `{ message_id,
/// state: "sent", sent_at }`.
fn send_response(sent: &SentMessage) -> CallToolResult {
    CallToolResult::structured(serde_json::json!({
        "message_id": sent.message_id.to_string(),
        "state": "sent",
        "sent_at": sent.sent_at.to_rfc3339(),
    }))
}

// ===========================================================================
// Poll delivery (`sent → delivered`; R-0069-a, R-0072-a) — Task 7 slice b
// ===========================================================================

/// One entry of a `poll` response's `messages` array — the §API Contract
/// **message object**: `{ message_id, sender: {actor_id, role_instance},
/// addressee: {actor_id, role_instance}, type, schema_version, payload,
/// state, sent_at, delivered_at?, acknowledged_at?, dispositioned_at?,
/// disposition?, disposition_note? }`.
struct PolledMessage {
    message_id: Uuid,
    sender_actor_id: Uuid,
    sender_role_instance: String,
    addressee_actor_id: Uuid,
    addressee_role_instance: String,
    message_type: String,
    schema_version: i32,
    payload: serde_json::Value,
    state: String,
    sent_at: DateTime<Utc>,
    delivered_at: Option<DateTime<Utc>>,
    acknowledged_at: Option<DateTime<Utc>>,
    dispositioned_at: Option<DateTime<Utc>>,
    disposition: Option<String>,
    disposition_note: Option<String>,
}

/// The raw column tuple read by [`deliver_body`] before mapping into a
/// [`PolledMessage`]. `#[allow(clippy::type_complexity)]`: a 15-tuple
/// observer return, same precedent as this crate's existing
/// `#[allow(clippy::type_complexity)]` tuple observers (e.g.
/// `tests/coordination_messages.rs::message_row_by_id`) — a named
/// `sqlx::FromRow`-deriving struct would be the first of its kind in this
/// codebase (no existing call site uses the derive; every multi-column read
/// here is a plain tuple), so this follows the established convention
/// instead of introducing a new one for a single call site.
#[allow(clippy::type_complexity)]
type PolledMessageColumns = (
    Uuid,
    Uuid,
    String,
    Uuid,
    String,
    String,
    i32,
    serde_json::Value,
    String,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    Option<String>,
    Option<String>,
);

impl From<PolledMessageColumns> for PolledMessage {
    fn from(row: PolledMessageColumns) -> Self {
        PolledMessage {
            message_id: row.0,
            sender_actor_id: row.1,
            sender_role_instance: row.2,
            addressee_actor_id: row.3,
            addressee_role_instance: row.4,
            message_type: row.5,
            schema_version: row.6,
            payload: row.7,
            state: row.8,
            sent_at: row.9,
            delivered_at: row.10,
            acknowledged_at: row.11,
            dispositioned_at: row.12,
            disposition: row.13,
            disposition_note: row.14,
        }
    }
}

impl PolledMessage {
    /// Render as the §API Contract message object. `Option` fields ride
    /// `serde_json::json!`'s standard `Option<T>` handling — `Some` renders
    /// the value, `None` renders JSON `null` — matching the spec's `?`-marked
    /// optional fields.
    fn into_json(self) -> serde_json::Value {
        serde_json::json!({
            "message_id": self.message_id.to_string(),
            "sender": {
                "actor_id": self.sender_actor_id.to_string(),
                "role_instance": self.sender_role_instance,
            },
            "addressee": {
                "actor_id": self.addressee_actor_id.to_string(),
                "role_instance": self.addressee_role_instance,
            },
            "type": self.message_type,
            "schema_version": self.schema_version,
            "payload": self.payload,
            "state": self.state,
            "sent_at": self.sent_at.to_rfc3339(),
            "delivered_at": self.delivered_at.map(|t| t.to_rfc3339()),
            "acknowledged_at": self.acknowledged_at.map(|t| t.to_rfc3339()),
            "dispositioned_at": self.dispositioned_at.map(|t| t.to_rfc3339()),
            "disposition": self.disposition,
            "disposition_note": self.disposition_note,
        })
    }
}

/// The `poll` delivery half (R-0072-a, R-0069-a): transition `actor_id`'s
/// still-`sent` messages to `delivered` (append-once — the `WHERE state =
/// 'sent'` guard means an already-delivered row is left untouched, so a
/// repeat poll never rewrites `delivered_at`), then return the actor's full
/// non-terminal queue (`sent`/`delivered`/`acknowledged` —
/// `dispositioned_at IS NULL`, the same predicate the
/// `messages_undispositioned_idx` partial index carries) as `poll`'s
/// `messages` array.
///
/// Called by [`crate::coordination::session_plane::poll_bind`] AFTER its own
/// `CoordinationOp::AttachBind` write has committed — a SEPARATE
/// `CoordinationOp::Poll` write, mirroring
/// [`PgCoordinationStore::live_leases_for_workspace`]'s existing post-commit
/// read placement in that same function. Delivery carries no refusal grammar
/// (R-0069-a: a recorded fact, not a caller-invocable action) — [`deliver_body`]
/// always returns `WriteResult::Commit`; the `Refuse` arm below exists only
/// because `WriteResult<T>` is a two-variant type.
pub(crate) async fn deliver_and_list(
    store: &PgCoordinationStore,
    ctx: &WorkspaceCtx,
    actor_id: Uuid,
) -> Result<Vec<serde_json::Value>, ErrorData> {
    let write = store
        .run_write(
            ctx,
            CoordinationOp::Poll,
            move |tx: &mut CoordinationTxn| {
                Box::pin(async move { deliver_body(tx, actor_id).await })
            },
        )
        .await;

    match write {
        Ok(WriteResult::Commit(messages)) => {
            Ok(messages.into_iter().map(PolledMessage::into_json).collect())
        }
        Ok(WriteResult::Refuse(_)) => Ok(Vec::new()),
        Err(_unavailable) => Err(coordination_unavailable()),
    }
}

/// The delivery state transition + queue read, run inside the `run_write`
/// transaction.
async fn deliver_body(
    tx: &mut CoordinationTxn,
    actor_id: Uuid,
) -> Result<WriteResult<Vec<PolledMessage>>, StorageFailure> {
    let workspace_id = tx.workspace_id();

    // Append-once: only rows still `sent` transition; an already-`delivered`
    // row (a repeat poll) is left untouched, so `delivered_at` is never
    // rewritten (R-0068-c). `now()` is the store-clock write (never a
    // host-local clock), matching every other consumption timestamp in this
    // cluster.
    sqlx::query(
        "UPDATE messages
            SET state = 'delivered', delivered_at = now()
          WHERE workspace_id = $1
            AND addressee_actor_id = $2
            AND state = 'sent'",
    )
    .bind(workspace_id)
    .bind(actor_id)
    .execute(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    // R-0072-a: the polling actor's non-terminal queue, joined for the
    // sender's and the actor's own role_instance names (the §API Contract
    // message object). `dispositioned_at IS NULL` matches the
    // `messages_undispositioned_idx` partial index predicate exactly.
    let rows: Vec<PolledMessageColumns> = sqlx::query_as(
        "SELECT m.id, m.sender_actor_id, sender.name, m.addressee_actor_id, addressee.name,
                m.message_type, m.schema_version, m.payload, m.state, m.sent_at,
                m.delivered_at, m.acknowledged_at, m.dispositioned_at, m.disposition,
                m.disposition_note
           FROM messages m
           JOIN actors sender    ON sender.id = m.sender_actor_id
           JOIN actors addressee ON addressee.id = m.addressee_actor_id
          WHERE m.workspace_id = $1
            AND m.addressee_actor_id = $2
            AND m.dispositioned_at IS NULL
          ORDER BY m.sent_at",
    )
    .bind(workspace_id)
    .bind(actor_id)
    .fetch_all(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    Ok(WriteResult::Commit(
        rows.into_iter().map(PolledMessage::from).collect(),
    ))
}

// ===========================================================================
// Shared lifecycle-transition read (`ack`/`disposition`) — Task 7 slice b
// ===========================================================================

/// Lock and read the `(addressee_actor_id, state)` fork of the message row
/// named by `message_id`, scoped to this txn's workspace (R-0076-b) — the
/// shared `ack`/`disposition` precondition read. `FOR UPDATE` locks the row
/// so a concurrent transition on the SAME message serializes on it, mirroring
/// `session_plane.rs::attach_body`'s own `SELECT … FOR UPDATE` pattern for
/// the live-attachment fork. `None` when no row matches — the caller maps
/// this to `Refusal::MessageNotFound`.
async fn lock_message_row(
    tx: &mut CoordinationTxn,
    message_id: Uuid,
) -> Result<Option<(Uuid, String)>, StorageFailure> {
    let workspace_id = tx.workspace_id();
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT addressee_actor_id, state FROM messages
          WHERE id = $1 AND workspace_id = $2
          FOR UPDATE",
    )
    .bind(message_id)
    .bind(workspace_id)
    .fetch_optional(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;
    Ok(row)
}

// ===========================================================================
// `ack` (`delivered → acknowledged`; R-0069-a/-b) — Task 7 slice b
// ===========================================================================

/// The committed `ack` result, rendered into the §API Contract response
/// (`{ message_id, state: "acknowledged", acknowledged_at }`) by
/// [`ack_response`].
struct AckedMessage {
    message_id: Uuid,
    acknowledged_at: DateTime<Utc>,
}

/// The `message ack` action (R-0069-a/-b; Task 7 slice b) — routed from
/// [`crate::mcp::server::MnemraMcpServer::handle_coordination`]. Addressee-only
/// (R-0069-b); legal only from `delivered` (`delivered → acknowledged`) —
/// every other state, including a second `ack`, refuses `invalid_transition`
/// (R-0068-c append-once).
pub(crate) async fn ack(
    store: &PgCoordinationStore,
    ctx: &WorkspaceCtx,
    message_id: Uuid,
) -> Result<CallToolResult, ErrorData> {
    let session_id = ctx.token_id;

    let write = store
        .run_write(ctx, CoordinationOp::Ack, move |tx: &mut CoordinationTxn| {
            Box::pin(async move { ack_body(tx, session_id, message_id).await })
        })
        .await;

    match write {
        Ok(WriteResult::Commit(acked)) => Ok(ack_response(&acked)),
        Ok(WriteResult::Refuse(refusal)) => Ok(refusal_result(&refusal)),
        // Fail-closed (R-0074-a): `run_write` has already op-logged the
        // unavailability (R-0075-a); no store internals leak here.
        Err(_unavailable) => Err(coordination_unavailable()),
    }
}

/// The `ack` state transition, run inside the `run_write` transaction.
///
/// Ordering: (1) attachment gate (`not_attached`, shared with `send`); (2)
/// row lookup (`message_not_found`); (3) addressee check (`not_addressee`,
/// R-0069-b); (4) legal-transition check (`invalid_transition` — legal only
/// from `delivered`).
async fn ack_body(
    tx: &mut CoordinationTxn,
    session_id: Uuid,
    message_id: Uuid,
) -> Result<WriteResult<AckedMessage>, StorageFailure> {
    // (1) Every message action requires a live attachment first (R-0064-e).
    let (acting_actor_id, _acting_role_instance) =
        match resolve_acting_actor(tx, session_id).await? {
            Ok(actor) => actor,
            Err(refusal) => return Ok(WriteResult::Refuse(refusal)),
        };

    // (2) A fabricated / nonexistent message_id refuses `message_not_found`
    // before any addressee/transition check runs.
    let Some((addressee_actor_id, state)) = lock_message_row(tx, message_id).await? else {
        return Ok(WriteResult::Refuse(Refusal::MessageNotFound));
    };

    // (3) Addressee-only (R-0069-b).
    if addressee_actor_id != acting_actor_id {
        return Ok(WriteResult::Refuse(Refusal::NotAddressee));
    }

    // (4) Legal only from `delivered` — `sent` (never delivered),
    // `acknowledged` (a second ack — AC3 append-once), and `dispositioned`
    // are all illegal transitions.
    if state != "delivered" {
        return Ok(WriteResult::Refuse(Refusal::InvalidTransition));
    }

    // `WHERE state = 'delivered'` re-affirms the transition guard at the
    // write itself (closes the SELECT-then-UPDATE race despite the `FOR
    // UPDATE` lock above already serializing concurrent callers).
    let acknowledged_at: (DateTime<Utc>,) = sqlx::query_as(
        "UPDATE messages
            SET state = 'acknowledged', acknowledged_at = now()
          WHERE id = $1 AND state = 'delivered'
          RETURNING acknowledged_at",
    )
    .bind(message_id)
    .fetch_one(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    Ok(WriteResult::Commit(AckedMessage {
        message_id,
        acknowledged_at: acknowledged_at.0,
    }))
}

/// Render a committed `ack` as the §API Contract response: `{ message_id,
/// state: "acknowledged", acknowledged_at }`.
fn ack_response(acked: &AckedMessage) -> CallToolResult {
    CallToolResult::structured(serde_json::json!({
        "message_id": acked.message_id.to_string(),
        "state": "acknowledged",
        "acknowledged_at": acked.acknowledged_at.to_rfc3339(),
    }))
}

// ===========================================================================
// `disposition` (`acknowledged → dispositioned`; R-0069-a/-b/-c, R-0075-b) —
// Task 7 slice b
// ===========================================================================

/// The closed disposition vocabulary (R-0069-c): exactly `completed`,
/// `declined`, `obsolete`. An out-of-set value refuses `invalid_disposition`.
fn validate_disposition_member(disposition_member: &str) -> Result<(), Refusal> {
    match disposition_member {
        "completed" | "declined" | "obsolete" => Ok(()),
        _ => Err(Refusal::InvalidDisposition),
    }
}

/// The committed `disposition` result, rendered into the §API Contract
/// response (`{ message_id, state: "dispositioned", disposition,
/// dispositioned_at }`) by [`disposition_response`]. `disposition_note`
/// carries the caller-supplied note through to the outer [`disposition`]
/// wrapper's `Commit` arm — it is NOT part of the API response, only the
/// deferred-until-committed op-log line (see the wrapper's doc comment).
struct DispositionedMessage {
    message_id: Uuid,
    disposition_member: String,
    disposition_note: Option<String>,
    dispositioned_at: DateTime<Utc>,
}

/// The `message disposition` action (R-0069-a/-b/-c, R-0075-b; Task 7 slice
/// b) — routed from
/// [`crate::mcp::server::MnemraMcpServer::handle_coordination`].
/// Addressee-only (R-0069-b); legal only from `acknowledged` (`acknowledged →
/// dispositioned`); `disposition_member` must be one of the closed vocabulary
/// (R-0069-c); `note` is an optional structured note, stored verbatim.
///
/// The `"coordination message disposition"` op-log line is emitted HERE, in
/// the `Ok(WriteResult::Commit(disp))` arm below — deliberately NOT inside
/// [`disposition_body`], which runs inside the `run_write` transaction
/// *before* commit. Logging from inside the transaction would emit the line
/// even when the transaction later rolls back (a storage failure, or a
/// fault-injected audit-emit failure in `test-hooks` builds), leaving a
/// forensic record of a disposition that never actually committed. Emitting
/// only after `run_write` returns `Commit` ties the log to the same
/// committed-or-nothing guarantee the state transition itself has.
pub(crate) async fn disposition(
    store: &PgCoordinationStore,
    ctx: &WorkspaceCtx,
    message_id: Uuid,
    disposition_member: &str,
    note: Option<&str>,
) -> Result<CallToolResult, ErrorData> {
    let session_id = ctx.token_id;
    let disposition_member_owned = disposition_member.to_owned();
    let note_owned = note.map(str::to_owned);

    let write = store
        .run_write(
            ctx,
            CoordinationOp::Disposition,
            move |tx: &mut CoordinationTxn| {
                Box::pin(async move {
                    disposition_body(
                        tx,
                        session_id,
                        message_id,
                        &disposition_member_owned,
                        note_owned.as_deref(),
                    )
                    .await
                })
            },
        )
        .await;

    match write {
        Ok(WriteResult::Commit(disp)) => {
            // R-0075-e log-field hygiene: the caller-supplied `note` rides as
            // a DISCRETE, Debug-formatted (`?note`) field — never Display/
            // string-interpolated. `{:?}` escapes embedded control characters
            // (a raw `\n` becomes the two-character sequence `\n`), so an
            // injected metacharacter cannot fragment this structured log line
            // or masquerade as a second field — mirrors
            // `log_send_validation_refusal`'s discrete-field posture (T6
            // anchor `message_types.rs:155-163`).
            tracing::info!(
                target: COORDINATION_TARGET,
                op = ?CoordinationOp::Disposition,
                message_id = %disp.message_id,
                disposition = %disp.disposition_member,
                note = ?disp.disposition_note,
                "coordination message disposition"
            );
            Ok(disposition_response(&disp))
        }
        Ok(WriteResult::Refuse(refusal)) => Ok(refusal_result(&refusal)),
        // Fail-closed (R-0074-a): `run_write` has already op-logged the
        // unavailability (R-0075-a); no store internals leak here.
        Err(_unavailable) => Err(coordination_unavailable()),
    }
}

/// The `disposition` state transition, run inside the `run_write`
/// transaction.
///
/// Ordering: (1) attachment gate (`not_attached`); (2) the closed-vocabulary
/// check (`invalid_disposition` — a pure, cheap check, run before any row
/// lookup, mirroring `send_body`'s own validate-before-touching-rows
/// posture); (3) row lookup (`message_not_found`); (4) addressee check
/// (`not_addressee`, R-0069-b); (5) legal-transition check
/// (`invalid_transition` — legal only from `acknowledged`). On commit: stage
/// the disposition audit (R-0075-b disposition half / AC10 — the `run_write`
/// machinery then covers the fail-closed half, R-0075-c, for free). The
/// `"coordination message disposition"` op-log line is NOT emitted here —
/// see the outer [`disposition`] wrapper's doc comment for why (it fires
/// only after this transaction actually commits).
async fn disposition_body(
    tx: &mut CoordinationTxn,
    session_id: Uuid,
    message_id: Uuid,
    disposition_member: &str,
    note: Option<&str>,
) -> Result<WriteResult<DispositionedMessage>, StorageFailure> {
    // (1) Every message action requires a live attachment first (R-0064-e).
    let (acting_actor_id, _acting_role_instance) =
        match resolve_acting_actor(tx, session_id).await? {
            Ok(actor) => actor,
            Err(refusal) => return Ok(WriteResult::Refuse(refusal)),
        };

    // (2) Closed disposition vocabulary (R-0069-c) — before any row lookup.
    if validate_disposition_member(disposition_member).is_err() {
        return Ok(WriteResult::Refuse(Refusal::InvalidDisposition));
    }

    // (3) A fabricated / nonexistent message_id refuses `message_not_found`.
    let Some((addressee_actor_id, state)) = lock_message_row(tx, message_id).await? else {
        return Ok(WriteResult::Refuse(Refusal::MessageNotFound));
    };

    // (4) Addressee-only (R-0069-b).
    if addressee_actor_id != acting_actor_id {
        return Ok(WriteResult::Refuse(Refusal::NotAddressee));
    }

    // (5) Legal only from `acknowledged` — `sent`/`delivered` (never
    // acknowledged) and a second `disposition` (AC3 append-once) are all
    // illegal transitions.
    if state != "acknowledged" {
        return Ok(WriteResult::Refuse(Refusal::InvalidTransition));
    }

    let workspace_id = tx.workspace_id();

    // `WHERE state = 'acknowledged'` re-affirms the transition guard at the
    // write itself, mirroring `ack_body`'s own SELECT-then-UPDATE race guard.
    let dispositioned_at: (DateTime<Utc>,) = sqlx::query_as(
        "UPDATE messages
            SET state = 'dispositioned', dispositioned_at = now(),
                disposition = $2, disposition_note = $3
          WHERE id = $1 AND state = 'acknowledged'
          RETURNING dispositioned_at",
    )
    .bind(message_id)
    .bind(disposition_member)
    .bind(note)
    .fetch_one(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    // R-0075-b / AC10 disposition-audit half: every disposition emits an
    // audit record. `run_write`'s existing, already-proven emit-guarantee
    // then covers R-0075-c (audit-emit failure rolls the whole txn back) for
    // free — no separate fail-closed test is authored here (module header
    // addendum).
    tx.stage_audit(AuditRecord::disposition(
        workspace_id,
        acting_actor_id,
        message_id,
    ));

    Ok(WriteResult::Commit(DispositionedMessage {
        message_id,
        disposition_member: disposition_member.to_owned(),
        disposition_note: note.map(str::to_owned),
        dispositioned_at: dispositioned_at.0,
    }))
}

/// Render a committed `disposition` as the §API Contract response: `{
/// message_id, state: "dispositioned", disposition, dispositioned_at }`.
fn disposition_response(disp: &DispositionedMessage) -> CallToolResult {
    CallToolResult::structured(serde_json::json!({
        "message_id": disp.message_id.to_string(),
        "state": "dispositioned",
        "disposition": disp.disposition_member,
        "dispositioned_at": disp.dispositioned_at.to_rfc3339(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_validation_refusal_maps_unknown_type_and_schema_violation() {
        let unknown = MessageValidationError::UnknownType {
            type_name: "not-a-type".to_owned(),
            schema_version: 1,
        };
        assert_eq!(send_validation_refusal(&unknown), Refusal::UnknownType);

        let violation = MessageValidationError::SchemaViolation {
            type_name: "handoff".to_owned(),
            schema_version: 1,
            detail: "missing field `subject`".to_owned(),
        };
        assert_eq!(
            send_validation_refusal(&violation),
            Refusal::SchemaViolation
        );
    }
}
