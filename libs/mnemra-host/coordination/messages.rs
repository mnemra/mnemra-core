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
