//! The `claim` action bodies: acting-actor resolution, `acquire`, `list`,
//! `renew`, `release`, and `takeover` (Task 5 slices b1/b2/c/d;
//! R-0065/R-0066/R-0067/R-0073/R-0075).
//!
//! # Scope (b1 + b2 + c + d)
//!
//! b1 landed the acting-actor resolver every `claim` action shares
//! ([`resolve_acting_actor`], the `not_attached` gate — R-0064-e, never
//! runtime-exercised before that slice) and the `acquire` action body
//! (R-0065-a/-b/-c/-d, R-0067). b2 adds [`list`] (R-0073-a, R-0067-c,
//! R-0075-a) — a READ action, resolved via
//! [`crate::coordination::write_path::PgCoordinationStore::resolve_acting_actor_for_read`]
//! rather than [`resolve_acting_actor`] (list opens no `run_write`
//! transaction; see [`list`]'s own doc). c adds [`renew`] and [`release`]
//! (R-0065-d, R-0067-c) — both writes, sharing the
//! [`resolve_and_check_lease`] prelude (attachment → lease lookup →
//! reserved-family → liveness → holder-only) before diverging into their own
//! mutation. d adds [`takeover`] (R-0066-a/-b/-c, R-0067-c) — the recovery
//! path for an expired lease: supersede the prior row, mint a fresh
//! successor, and ALWAYS stage a `lease_takeover` audit record (mirrors
//! [`crate::coordination::session_plane::succeed_via_takeover`]'s exact
//! supersede-then-insert ordering, the attachment-plane's own takeover).
//!
//! # Why the resolver lives here, not in `session_plane`
//!
//! [`crate::coordination::session_plane::attach_body`] MINTS the acting actor
//! (resolve-or-create); every `claim` action instead RESOLVES an
//! already-attached actor from the caller's session — a read against the
//! reserved `actor:`-family lease, not a write. Distinct enough
//! responsibility to own its module, even though both bottom out in the same
//! `leases` table.

use chrono::{DateTime, Utc};
use rmcp::model::{CallToolResult, ErrorData};
use uuid::Uuid;

use crate::auth::workspace_ctx::WorkspaceCtx;
use crate::coordination::CoordinationOp;
use crate::coordination::audit::{AuditRecord, ExpiryEvidence};
use crate::coordination::resource_id;
use crate::coordination::session_plane::{
    coordination_unavailable, refusal_result, refusal_result_with_detail,
};
use crate::coordination::write_path::{
    CoordinationTxn, LiveLeaseRow, PgCoordinationStore, Refusal, StorageFailure, Unavailable,
    WriteResult, log_read_outcome,
};
use crate::coordination::{LEASE_DEFAULT_DURATION, LEASE_MAX_DURATION};

/// Resolve the calling session's live attached actor (R-0064-e): the
/// `actor:`-family lease where `session_id` matches, non-terminal, and
/// unexpired at the store transaction clock (`expires_at > now()`, evaluated
/// in this query — R-0065-e, never a host-local clock). Every `claim` action
/// resolves this FIRST; this gate has never been runtime-exercised before
/// Task 5 (no non-binding action existed to refuse from).
///
/// Joins `actors` for the role-instance name in the same query — `acquire`'s
/// response renders `holder.role_instance` (§API Contract), so this is the
/// acting actor's ONLY resolution round-trip, not a resolve-then-lookup pair.
///
/// On success, records the acting actor onto `tx`
/// ([`CoordinationTxn::record_acting_actor`]) so the op-log entry attributes
/// it (R-0075-a) — every subsequent refusal in the same write (e.g.
/// `invalid_resource` after a live attachment resolves) still carries the
/// correct `actor_id`.
///
/// Returns `Ok(Err(Refusal::NotAttached))` — a refusal the body maps to
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

/// The committed `acquire` result, rendered into the §API Contract lease
/// object by [`acquire_response`].
struct AcquiredLease {
    lease_id: Uuid,
    resource: String,
    holder_actor_id: Uuid,
    holder_role_instance: String,
    acquired_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

/// The `claim acquire` action (R-0065-a/-b/-c/-d, R-0067) — routed from
/// [`crate::mcp::server::MnemraMcpServer::handle_coordination`].
///
/// `resource` is the caller-supplied identifier string; `duration_seconds` is
/// `None` when the request omitted it (defaults to [`LEASE_DEFAULT_DURATION`])
/// — an absent key, never a present `0` (the caller distinguishes the two
/// before this point, at the MCP argument-parsing boundary).
pub(crate) async fn acquire(
    store: &PgCoordinationStore,
    ctx: &WorkspaceCtx,
    resource: &str,
    duration_seconds: Option<i64>,
) -> Result<CallToolResult, ErrorData> {
    let session_id = ctx.token_id;
    let resource_owned = resource.to_owned();

    let write = store
        .run_write(
            ctx,
            CoordinationOp::Acquire,
            move |tx: &mut CoordinationTxn| {
                Box::pin(async move {
                    acquire_body(tx, session_id, &resource_owned, duration_seconds).await
                })
            },
        )
        .await;

    match write {
        Ok(WriteResult::Commit(lease)) => Ok(acquire_response(&lease)),
        Ok(WriteResult::Refuse(Refusal::InvalidResource)) => {
            // Enrich `invalid_resource`'s detail with the specific
            // `resource_id::parse` failure (R-0067-a). `acquire_body`
            // already made the refusal decision inside the transaction;
            // `Refusal` is a closed, unparameterized enum (write_path.rs) so
            // the specific `ResourceIdError` cannot be threaded through
            // `WriteResult` — instead, re-derive it here via a SECOND, pure
            // `resource_id::parse` call on the same input. `parse` is
            // IO-free/deterministic (module doc), so this cannot diverge
            // from the transaction's decision.
            let rule = resource_id::parse(resource)
                .err()
                .map(|e| e.detail_rule())
                .unwrap_or_else(|| "malformed resource identifier".to_string());
            Ok(refusal_result_with_detail(
                &Refusal::InvalidResource,
                serde_json::json!({ "rule": rule }),
            ))
        }
        Ok(WriteResult::Refuse(refusal)) => Ok(refusal_result(&refusal)),
        // Fail-closed (R-0074-a): `run_write` has already op-logged the
        // unavailability (R-0075-a); no store internals leak here.
        Err(_unavailable) => Err(coordination_unavailable()),
    }
}

/// The `acquire` state transition, run inside the `run_write` transaction.
///
/// Ordering is load-bearing (mirrors `session_plane::attach_body`'s ordering
/// discipline):
/// 1. Resolve the acting actor — `not_attached` else the holder.
/// 2. Parse + validate the resource identifier (`invalid_resource`); bar the
///    reserved `actor` family request-side (`reserved_family` — R-0067-c).
/// 3. Bound the duration — default when omitted, `invalid_duration` on
///    `<= 0` or `> max`.
/// 4. Read the store transaction clock (`now()`) — the lease window is
///    computed from it, never a host-local clock (R-0065-e).
/// 5. Atomic `INSERT ... ON CONFLICT ... DO NOTHING`. `leases_live_resource_uq`
///    (R-0065-b) is the mutual-exclusion mechanism, named directly as the
///    `ON CONFLICT` target; a conflict means a live holder already exists —
///    read its holder + expiry and refuse `resource_held` (never a
///    `SELECT`-then-`INSERT` check-then-act; this includes the
///    expired-but-untaken case, R-0066-a — an expired row is still
///    non-terminal, so it still occupies the unique slot). `ON CONFLICT ...
///    DO NOTHING`, not a plain insert caught for `23505` (a raised error
///    aborts the whole Postgres transaction, poisoning it for this step's
///    follow-up read — see the inline comment at the insert site). No audit
///    (`acquire` is not in the R-0075-b privileged subset; op-log only,
///    emitted by `run_write`).
async fn acquire_body(
    tx: &mut CoordinationTxn,
    session_id: Uuid,
    resource: &str,
    duration_seconds: Option<i64>,
) -> Result<WriteResult<AcquiredLease>, StorageFailure> {
    // (1) Every `claim` action requires a live attachment first (R-0064-e).
    let (holder_actor_id, holder_role_instance) = match resolve_acting_actor(tx, session_id).await?
    {
        Ok(actor) => actor,
        Err(refusal) => return Ok(WriteResult::Refuse(refusal)),
    };

    // (2) Parse + validate the resource identifier (R-0067-a); bar the
    // reserved `actor` family request-side (R-0067-c) — distinct from
    // `invalid_resource` so a typo and a reserved-family probe are
    // distinguishable to the caller. The specific `ResourceIdError` is not
    // threaded through here (`Refusal` is closed/unparameterized) — `acquire`
    // (the caller) re-derives it via a second, pure `resource_id::parse` call
    // to render the `invalid_resource` detail's specific rule (R-0067-a).
    let resource_id = match resource_id::parse(resource) {
        Ok(id) => id,
        Err(_) => return Ok(WriteResult::Refuse(Refusal::InvalidResource)),
    };
    if resource_id.family().is_reserved() {
        return Ok(WriteResult::Refuse(Refusal::ReservedFamily));
    }

    // (3) Duration: default when omitted, bound-checked otherwise (R-0065-d).
    let duration_secs = match duration_seconds {
        None => LEASE_DEFAULT_DURATION.as_secs() as i64,
        Some(requested) => {
            let max = LEASE_MAX_DURATION.as_secs() as i64;
            if requested <= 0 || requested > max {
                return Ok(WriteResult::Refuse(Refusal::InvalidDuration));
            }
            requested
        }
    };

    let workspace_id = tx.workspace_id();

    // (4) Store-clock "now" — the lease window is computed from it, read
    // inside this deciding transaction (R-0065-e).
    let now: (DateTime<Utc>,) = sqlx::query_as("SELECT now()")
        .fetch_one(tx.conn())
        .await
        .map_err(|e| StorageFailure(Box::new(e)))?;
    let acquired_at = now.0;
    let expires_at = acquired_at + chrono::TimeDelta::seconds(duration_secs);
    let resource_str = resource_id.to_string();

    // (5) Atomic insert-or-detect-conflict. `leases_live_resource_uq` (the
    // partial unique index on `(workspace_id, resource) WHERE terminal_state
    // IS NULL`) is the mutual-exclusion mechanism (R-0065-b) — the `ON
    // CONFLICT` target names it exactly. `ON CONFLICT ... DO NOTHING`
    // (never a plain `INSERT` + catch-`23505`): Postgres aborts the WHOLE
    // transaction on ANY raised error, so a `23505` caught off a plain
    // INSERT poisons this txn for the follow-up read R-0065-c requires (the
    // winner's holder + expiry) — every later statement on that connection
    // fails "current transaction is aborted" (confirmed empirically: QA-1
    // red under the plain-INSERT-catch-`23505` shape, the pattern
    // `attach_body`'s fresh-attach path uses — its own `23505` refusal
    // carries no extra data and never needs a follow-up read on the same
    // txn, so that shape never hits this). `DO NOTHING` is a controlled
    // no-op, not an error, so the txn stays healthy for the follow-up SELECT
    // below. Mirrors the same `ON CONFLICT ... DO NOTHING` + follow-up
    // SELECT idiom `builtins::actors::resolve_or_create_in_txn` already
    // establishes in this codebase. NO `session_id` bound (that column is
    // attachment-only, R-0064-f).
    let inserted: Option<(Uuid,)> = sqlx::query_as(
        "INSERT INTO leases
             (workspace_id, resource, holder_actor_id, acquired_at, duration, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (workspace_id, resource) WHERE terminal_state IS NULL DO NOTHING
         RETURNING id",
    )
    .bind(workspace_id)
    .bind(&resource_str)
    .bind(holder_actor_id)
    .bind(acquired_at)
    .bind(duration_secs)
    .bind(expires_at)
    .fetch_optional(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    let lease_id = match inserted {
        Some((id,)) => id,
        None => {
            // A live holder already exists — read its holder + expiry (the
            // workspace-visible facts, R-0065-c) and refuse `resource_held`.
            let holder: (Uuid, DateTime<Utc>) = sqlx::query_as(
                "SELECT holder_actor_id, expires_at
                   FROM leases
                  WHERE workspace_id = $1 AND resource = $2 AND terminal_state IS NULL",
            )
            .bind(workspace_id)
            .bind(&resource_str)
            .fetch_one(tx.conn())
            .await
            .map_err(|e| StorageFailure(Box::new(e)))?;
            return Ok(WriteResult::Refuse(Refusal::ResourceHeld {
                holder_actor_id: holder.0,
                expires_at: holder.1,
            }));
        }
    };

    Ok(WriteResult::Commit(AcquiredLease {
        lease_id,
        resource: resource_str,
        holder_actor_id,
        holder_role_instance,
        acquired_at,
        expires_at,
    }))
}

/// Render an [`AcquiredLease`] as the §API Contract lease object: `{
/// lease_id, resource, holder: {actor_id, role_instance}, acquired_at,
/// expires_at }`. Shared by [`acquire_response`] and [`takeover_response`]
/// (Task 5 d) — `takeover`'s success response is the SAME lease-object shape
/// plus a `superseded` field (§API Contract `takeover`), so the object body
/// is factored out rather than duplicated.
fn lease_object(lease: &AcquiredLease) -> serde_json::Value {
    serde_json::json!({
        "lease_id": lease.lease_id.to_string(),
        "resource": lease.resource,
        "holder": {
            "actor_id": lease.holder_actor_id.to_string(),
            "role_instance": lease.holder_role_instance,
        },
        "acquired_at": lease.acquired_at.to_rfc3339(),
        "expires_at": lease.expires_at.to_rfc3339(),
    })
}

/// Render a committed `acquire` as the §API Contract lease object.
fn acquire_response(lease: &AcquiredLease) -> CallToolResult {
    CallToolResult::structured(lease_object(lease))
}

/// The `claim list` action (R-0073-a, R-0067-c, R-0075-a) — routed from
/// [`crate::mcp::server::MnemraMcpServer::handle_coordination`].
///
/// `family` and `resource_prefix` are the optional §API Contract filters,
/// `None` when the request omitted them (an absent key, never a present
/// empty string — the caller distinguishes the two at the MCP
/// argument-parsing boundary, mirroring [`acquire`]'s `duration_seconds`
/// convention).
///
/// # Why this is a READ, not a `run_write` body (cross-dispatch item 3)
///
/// `list` performs no state transition — no row is written, no audit is
/// staged — so it does not open a [`PgCoordinationStore::run_write`]
/// transaction the way [`acquire`] does. Attachment resolution instead runs
/// against a short-lived pool read
/// ([`PgCoordinationStore::resolve_acting_actor_for_read`]), and the lease
/// read against [`PgCoordinationStore::list_leases`] — both direct pool
/// reads, mirroring the existing read/write split
/// [`crate::coordination::session_plane::poll_bind`] already establishes
/// for its own post-commit `live_leases_for_workspace` read.
///
/// # Op-log (R-0075-a, Q2 decision)
///
/// Every outcome below — success, `not_attached`, `reserved_family` — emits
/// one op-log entry via [`log_read_outcome`] (the `run_write`-free sibling
/// of [`crate::coordination::write_path::PgCoordinationStore::run_write`]'s
/// own emission), carrying `op = CoordinationOp::ClaimList` and, once
/// resolved, the acting actor (R-0075-a attribution). `not_attached` never
/// resolves an actor, so its entry honestly carries `actor_id = None` —
/// the same convention [`acquire_body`]'s pre-resolution refusals follow.
pub(crate) async fn list(
    store: &PgCoordinationStore,
    ctx: &WorkspaceCtx,
    family: Option<&str>,
    resource_prefix: Option<&str>,
) -> Result<CallToolResult, ErrorData> {
    let session_id = ctx.token_id;

    // (1) Resolve the caller's live attachment on a READ path (R-0064-e) —
    // never through `run_write` (see this fn's doc, cross-dispatch item 3).
    let (actor_id, _role_instance) =
        match store.resolve_acting_actor_for_read(ctx, session_id).await {
            Ok(Ok(actor)) => actor,
            Ok(Err(refusal)) => {
                let response = refusal_result(&refusal);
                let log_result: Result<WriteResult<()>, Unavailable> =
                    Ok(WriteResult::Refuse(refusal));
                log_read_outcome(
                    CoordinationOp::ClaimList,
                    ctx.workspace_id(),
                    ctx.token_id,
                    None,
                    &log_result,
                );
                return Ok(response);
            }
            // Fail-closed (R-0074-a): a read that could not be verified surfaces
            // as a structured stop, never an empty success or an empty list.
            // Op-logged too (R-0075-a: refusals SHALL be logged, extended here
            // to the read-side unavailable case — b2 review M-1): no actor was
            // ever resolved at this point, so the entry honestly carries
            // `actor_id = None`, mirroring `run_write`'s own pre-`begin`
            // unavailable attribution.
            Err(unavailable) => {
                let log_result: Result<WriteResult<()>, Unavailable> = Err(unavailable);
                log_read_outcome(
                    CoordinationOp::ClaimList,
                    ctx.workspace_id(),
                    ctx.token_id,
                    None,
                    &log_result,
                );
                return Err(coordination_unavailable());
            }
        };

    // (2) Reserved-family bar on the filter itself (R-0067-c): `family ==
    // "actor"` is refused before any lease read runs — distinct from
    // `invalid_resource` (a malformed/out-of-family value; `list` never
    // rejects an unrecognized-but-non-reserved family, see
    // `PgCoordinationStore::list_leases`'s doc for the dogfooder-default
    // rationale). Every read path below ALSO excludes `actor:` rows
    // unconditionally, so this is a request-side refusal on top of a
    // structural backstop, not the only exclusion mechanism.
    if family == Some("actor") {
        let log_result: Result<WriteResult<()>, Unavailable> =
            Ok(WriteResult::Refuse(Refusal::ReservedFamily));
        log_read_outcome(
            CoordinationOp::ClaimList,
            ctx.workspace_id(),
            ctx.token_id,
            Some(actor_id),
            &log_result,
        );
        return Ok(refusal_result(&Refusal::ReservedFamily));
    }

    // (3) The filtered, workspace-visible, actor-excluded live leases
    // (R-0073-a/R-0067-c). A storage failure here is fail-closed AND
    // op-logged (R-0075-a — b2 review M-1: this path previously discarded
    // the store error and emitted no op-log entry at all, unlike the write
    // path's own `run_write`, which always logs an `unavailable` outcome on
    // storage failure). The acting actor is already resolved by this point
    // (step 1), so the entry carries real attribution.
    let leases = match store.list_leases(ctx, family, resource_prefix).await {
        Ok(leases) => leases,
        Err(unavailable) => {
            let log_result: Result<WriteResult<()>, Unavailable> = Err(unavailable);
            log_read_outcome(
                CoordinationOp::ClaimList,
                ctx.workspace_id(),
                ctx.token_id,
                Some(actor_id),
                &log_result,
            );
            return Err(coordination_unavailable());
        }
    };

    let log_result: Result<WriteResult<()>, Unavailable> = Ok(WriteResult::Commit(()));
    log_read_outcome(
        CoordinationOp::ClaimList,
        ctx.workspace_id(),
        ctx.token_id,
        Some(actor_id),
        &log_result,
    );

    Ok(list_response(&leases))
}

/// Render a `list` result as the §API Contract response: `{ leases: [
/// { resource, lease_id, holder: {actor_id, role_instance}, acquired_at,
/// expires_at }, ... ] }`.
fn list_response(leases: &[LiveLeaseRow]) -> CallToolResult {
    let entries: Vec<serde_json::Value> = leases
        .iter()
        .map(|l| {
            serde_json::json!({
                "resource": l.resource,
                "lease_id": l.lease_id.to_string(),
                "holder": {
                    "actor_id": l.holder_actor_id.to_string(),
                    "role_instance": l.holder_role_instance,
                },
                "acquired_at": l.acquired_at.to_rfc3339(),
                "expires_at": l.expires_at.to_rfc3339(),
            })
        })
        .collect();

    CallToolResult::structured(serde_json::json!({ "leases": entries }))
}

// ---------------------------------------------------------------------------
// `renew` / `release` (Task 5 slice c, R-0065-d/R-0067-c)
// ---------------------------------------------------------------------------

/// The lease row read by [`resolve_and_check_lease`]: `(id, holder_actor_id,
/// resource, terminal_state, expires_at, duration, now())`. Aliased to
/// satisfy `clippy::type_complexity` at the query site (mirrors
/// `write_path::LiveLeaseColumns`).
type RenewOrReleaseRow = (
    Uuid,
    Uuid,
    String,
    Option<String>,
    DateTime<Utc>,
    i64,
    DateTime<Utc>,
);

/// A `renew`/`release` target that has passed every refusal check (found,
/// non-reserved-family, live, held by the acting actor) — the shared prelude
/// [`resolve_and_check_lease`] hands this to [`renew_body`] and
/// [`release_body`], which each perform their own distinct mutation from
/// here.
struct CheckedLease {
    /// The lease row id (equals the caller's `lease_id` — carried through so
    /// callers don't re-thread the argument).
    lease_id: Uuid,
    /// The lease's own stored duration (seconds). `renew` extends
    /// `expires_at` from the store clock by this amount (R-0065-d: the
    /// lease's OWN duration, never a request-supplied value — the `renew`
    /// request carries no duration argument at all).
    duration_secs: i64,
    /// The store transaction clock, read in the SAME query as the row
    /// lookup (R-0065-e). `renew` computes its new `expires_at` from this
    /// instant rather than a second `SELECT now()` — Postgres's `now()` is
    /// constant for the lifetime of one transaction, so a second read would
    /// return the identical value; reusing it avoids a redundant round-trip.
    store_now: DateTime<Utc>,
}

/// The shared `renew`/`release` prelude (R-0065-d, R-0067-c): resolve the
/// caller's live attachment, locate the named lease, and apply every
/// refusal check EXCEPT the mutation itself — [`renew_body`] and
/// [`release_body`] each perform their own mutation after this returns
/// `Ok`.
///
/// Ordering is load-bearing (mirrors [`acquire_body`]'s ordering discipline;
/// pinned by the dispatch's cross-dispatch context and the acceptance
/// suite's ordering-landmine tests, `tests/coordination_leases.rs`'s c
/// addendum):
/// 1. Resolve the acting actor — `not_attached` else the acting actor id.
/// 2. `SELECT ... FOR UPDATE` the named lease by `(id, workspace_id)`. No
///    matching row (a fabricated id, or a row in a different workspace) ⇒
///    `lease_not_found`.
/// 3. Reserved-family bar on the RESOLVED ROW's `resource` column (R-0067-c
///    — distinct from `acquire`'s request-side check on the resource
///    STRING): an `actor:`-family row ⇒ `reserved_family`. Checked BEFORE
///    liveness/holder, so a caller naming its own attachment lease id
///    (which it genuinely holds, and which is live) is refused
///    `reserved_family`, never `not_holder`.
/// 4. **Liveness:** "found" for renew/release means non-terminal
///    (`terminal_state IS NULL`) AND unexpired (`store_now < expires_at`,
///    R-0065-e — read at the store transaction clock, never a host-local
///    clock). Expired-but-untaken, `released`, or `taken_over` all collapse
///    to `lease_not_found` — an expired hold is not revivable; the path
///    back is the explicit, audited `takeover`. Checked BEFORE the holder
///    check, so a non-holder renewing/releasing an EXPIRED lease is
///    `lease_not_found`, never `not_holder` (the ordering landmine the
///    acceptance suite pins).
/// 5. **Holder-only:** `holder_actor_id != acting_actor` ⇒ `not_holder`.
async fn resolve_and_check_lease(
    tx: &mut CoordinationTxn,
    session_id: Uuid,
    lease_id: Uuid,
) -> Result<Result<CheckedLease, Refusal>, StorageFailure> {
    // (1) Every `claim` action requires a live attachment first (R-0064-e).
    let (acting_actor, _role_instance) = match resolve_acting_actor(tx, session_id).await? {
        Ok(actor) => actor,
        Err(refusal) => return Ok(Err(refusal)),
    };

    let workspace_id = tx.workspace_id();

    // (2) Locate the named lease, workspace-scoped. `FOR UPDATE` locks the
    // row for the duration of this write (mirrors `attach_body`'s fork
    // read).
    let row: Option<RenewOrReleaseRow> = sqlx::query_as(
        "SELECT id, holder_actor_id, resource, terminal_state, expires_at, duration, now()
           FROM leases
          WHERE id = $1 AND workspace_id = $2
          FOR UPDATE",
    )
    .bind(lease_id)
    .bind(workspace_id)
    .fetch_optional(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    let (id, holder_actor_id, resource, terminal_state, expires_at, duration_secs, store_now) =
        match row {
            Some(r) => r,
            None => return Ok(Err(Refusal::LeaseNotFound)),
        };

    // (3) Reserved-family bar on the RESOLVED ROW (R-0067-c) — before
    // liveness/holder. A row whose `resource` fails to parse would be a
    // storage anomaly (every INSERT path validates it via
    // `resource_id::parse` first, R-0067-a) — treated as non-reserved here
    // rather than surfacing a refusal vocabulary this fn does not own; the
    // liveness/holder checks below still apply to it unaffected.
    if resource_id::parse(&resource).is_ok_and(|parsed| parsed.family().is_reserved()) {
        return Ok(Err(Refusal::ReservedFamily));
    }

    // (4) Liveness — non-terminal AND unexpired at the store clock
    // (R-0065-e). Expired/released/taken-over all collapse to
    // `lease_not_found`.
    if terminal_state.is_some() || store_now >= expires_at {
        return Ok(Err(Refusal::LeaseNotFound));
    }

    // (5) Holder-only.
    if holder_actor_id != acting_actor {
        return Ok(Err(Refusal::NotHolder));
    }

    Ok(Ok(CheckedLease {
        lease_id: id,
        duration_secs,
        store_now,
    }))
}

/// The committed `renew` result, rendered into the §API Contract response
/// (`{ lease_id, expires_at }`) by [`renew_response`].
struct RenewedLease {
    lease_id: Uuid,
    expires_at: DateTime<Utc>,
}

/// The `claim renew` action (R-0065-d, R-0067-c) — routed from
/// [`crate::mcp::server::MnemraMcpServer::handle_coordination`].
///
/// `lease_id` is the caller-supplied lease identifier (§API Contract) —
/// distinct from `acquire`'s `resource` argument; `renew` carries no
/// duration argument (it always extends by the lease's own stored
/// duration, R-0065-d).
pub(crate) async fn renew(
    store: &PgCoordinationStore,
    ctx: &WorkspaceCtx,
    lease_id: Uuid,
) -> Result<CallToolResult, ErrorData> {
    let session_id = ctx.token_id;

    let write = store
        .run_write(
            ctx,
            CoordinationOp::Renew,
            move |tx: &mut CoordinationTxn| {
                Box::pin(async move { renew_body(tx, session_id, lease_id).await })
            },
        )
        .await;

    match write {
        Ok(WriteResult::Commit(renewed)) => Ok(renew_response(&renewed)),
        Ok(WriteResult::Refuse(refusal)) => Ok(refusal_result(&refusal)),
        // Fail-closed (R-0074-a): `run_write` has already op-logged the
        // unavailability (R-0075-a); no store internals leak here.
        Err(_unavailable) => Err(coordination_unavailable()),
    }
}

/// The `renew` state transition, run inside the `run_write` transaction —
/// the shared checks in [`resolve_and_check_lease`], then the extension
/// itself: `expires_at = store_now + the lease's own stored duration`
/// (R-0065-d: extend from the renewal moment, using the lease's OWN
/// duration). No audit staged (`renew` is not in the R-0075-b privileged
/// subset; op-log only, emitted by `run_write`).
async fn renew_body(
    tx: &mut CoordinationTxn,
    session_id: Uuid,
    lease_id: Uuid,
) -> Result<WriteResult<RenewedLease>, StorageFailure> {
    let checked = match resolve_and_check_lease(tx, session_id, lease_id).await? {
        Ok(c) => c,
        Err(refusal) => return Ok(WriteResult::Refuse(refusal)),
    };

    let new_expires_at = checked.store_now + chrono::TimeDelta::seconds(checked.duration_secs);

    sqlx::query("UPDATE leases SET expires_at = $1 WHERE id = $2")
        .bind(new_expires_at)
        .bind(checked.lease_id)
        .execute(tx.conn())
        .await
        .map_err(|e| StorageFailure(Box::new(e)))?;

    Ok(WriteResult::Commit(RenewedLease {
        lease_id: checked.lease_id,
        expires_at: new_expires_at,
    }))
}

/// Render a committed `renew` as the §API Contract response: `{ lease_id,
/// expires_at }` — the SAME `lease_id`, the NEW `expires_at`.
fn renew_response(renewed: &RenewedLease) -> CallToolResult {
    CallToolResult::structured(serde_json::json!({
        "lease_id": renewed.lease_id.to_string(),
        "expires_at": renewed.expires_at.to_rfc3339(),
    }))
}

/// The committed `release` result, rendered into the §API Contract response
/// (`{ lease_id, released: true }`) by [`release_response`].
struct ReleasedLease {
    lease_id: Uuid,
}

/// The `claim release` action (R-0065-d, R-0067-c) — routed from
/// [`crate::mcp::server::MnemraMcpServer::handle_coordination`].
///
/// `lease_id` is the caller-supplied lease identifier (§API Contract) —
/// same argument contract as [`renew`].
pub(crate) async fn release(
    store: &PgCoordinationStore,
    ctx: &WorkspaceCtx,
    lease_id: Uuid,
) -> Result<CallToolResult, ErrorData> {
    let session_id = ctx.token_id;

    let write = store
        .run_write(
            ctx,
            CoordinationOp::Release,
            move |tx: &mut CoordinationTxn| {
                Box::pin(async move { release_body(tx, session_id, lease_id).await })
            },
        )
        .await;

    match write {
        Ok(WriteResult::Commit(released)) => Ok(release_response(&released)),
        Ok(WriteResult::Refuse(refusal)) => Ok(refusal_result(&refusal)),
        Err(_unavailable) => Err(coordination_unavailable()),
    }
}

/// The `release` state transition, run inside the `run_write` transaction —
/// the shared checks in [`resolve_and_check_lease`], then terminate the
/// lease: `terminal_state = 'released', terminated_at = now()`. No audit
/// staged (same rationale as [`renew_body`]).
async fn release_body(
    tx: &mut CoordinationTxn,
    session_id: Uuid,
    lease_id: Uuid,
) -> Result<WriteResult<ReleasedLease>, StorageFailure> {
    let checked = match resolve_and_check_lease(tx, session_id, lease_id).await? {
        Ok(c) => c,
        Err(refusal) => return Ok(WriteResult::Refuse(refusal)),
    };

    sqlx::query(
        "UPDATE leases SET terminal_state = 'released', terminated_at = now() WHERE id = $1",
    )
    .bind(checked.lease_id)
    .execute(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    Ok(WriteResult::Commit(ReleasedLease {
        lease_id: checked.lease_id,
    }))
}

/// Render a committed `release` as the §API Contract response: `{
/// lease_id, released: true }`.
fn release_response(released: &ReleasedLease) -> CallToolResult {
    CallToolResult::structured(serde_json::json!({
        "lease_id": released.lease_id.to_string(),
        "released": true,
    }))
}

// ---------------------------------------------------------------------------
// `takeover` (Task 5 slice d, R-0066-a/-b/-c, R-0067-c)
// ---------------------------------------------------------------------------

/// The committed `takeover` result: the fresh successor lease (the same
/// shape [`AcquiredLease`] renders) plus the superseded prior holder's
/// evidence (§API Contract `takeover`: `{ ...lease, superseded: {
/// prior_holder, expired_at } }`).
struct TakenOverLease {
    lease: AcquiredLease,
    prior_holder_actor_id: Uuid,
    prior_expires_at: DateTime<Utc>,
}

/// The `claim takeover` action (R-0066-a/-b/-c, R-0067-c) — routed from
/// [`crate::mcp::server::MnemraMcpServer::handle_coordination`].
///
/// `resource` is the caller-supplied identifier string — the SAME argument
/// contract as [`acquire`]'s `resource` (§API Contract `takeover`: `{
/// action: "takeover", resource }`). `takeover` carries no
/// `duration_seconds` argument — the fresh lease it mints always uses
/// [`LEASE_DEFAULT_DURATION`] (dogfooder-default: spec-silent on the
/// recovered lease's duration; the request schema has no field to carry an
/// override, so the same default an omitted `acquire` duration resolves to
/// is the only sensible V0 choice — revisit if a real consumer needs a
/// takeover-specific duration).
pub(crate) async fn takeover(
    store: &PgCoordinationStore,
    ctx: &WorkspaceCtx,
    resource: &str,
) -> Result<CallToolResult, ErrorData> {
    let session_id = ctx.token_id;
    let resource_owned = resource.to_owned();

    let write = store
        .run_write(
            ctx,
            CoordinationOp::Takeover,
            move |tx: &mut CoordinationTxn| {
                Box::pin(async move { takeover_body(tx, session_id, &resource_owned).await })
            },
        )
        .await;

    match write {
        Ok(WriteResult::Commit(taken_over)) => Ok(takeover_response(&taken_over)),
        Ok(WriteResult::Refuse(Refusal::InvalidResource)) => {
            // Same enrichment [`acquire`] performs (R-0067-a) — re-derive the
            // specific `resource_id::parse` failure via a second, pure parse
            // call (see `acquire`'s own doc comment for the full rationale;
            // identical here since `takeover` takes the same request-side
            // `resource` string).
            let rule = resource_id::parse(resource)
                .err()
                .map(|e| e.detail_rule())
                .unwrap_or_else(|| "malformed resource identifier".to_string());
            Ok(refusal_result_with_detail(
                &Refusal::InvalidResource,
                serde_json::json!({ "rule": rule }),
            ))
        }
        Ok(WriteResult::Refuse(refusal)) => Ok(refusal_result(&refusal)),
        // Fail-closed (R-0074-a): `run_write` has already op-logged the
        // unavailability (R-0075-a); no store internals leak here.
        Err(_unavailable) => Err(coordination_unavailable()),
    }
}

/// The `takeover` state transition, run inside the `run_write` transaction.
///
/// Ordering is load-bearing (build plan §3.4; mirrors `acquire_body`'s
/// discipline):
/// 1. Resolve the acting actor — `not_attached` else the NEW holder.
/// 2. Parse + validate the resource identifier (`invalid_resource`); bar the
///    reserved `actor` family request-side (`reserved_family` — R-0067-c) —
///    the same request-side check [`acquire_body`] performs; `takeover`
///    takes the same `resource` string argument.
/// 3. Lock the resource's current live row `FOR UPDATE`, reading the store
///    transaction clock (`now()`) in the SAME query (R-0065-e). No live row
///    ⇒ `lease_not_found` ("use `acquire`", R-0066-a). Live and NOT expired
///    (`store_now < expires_at`) ⇒ `not_expired`.
/// 4. Expired (`store_now >= expires_at`): supersede the prior row FIRST —
///    `terminal_state='taken_over', terminated_at=store_now,
///    superseded_by=<pre-generated new id>` — which drops it out of the
///    partial unique index (`leases_live_resource_uq`), freeing the slot
///    (mirrors [`crate::coordination::session_plane::succeed_via_takeover`]'s
///    exact supersede-then-insert ordering). Insert the successor lease
///    SECOND with the pre-generated id, the acting actor as holder, a fresh
///    window from `store_now` sized [`LEASE_DEFAULT_DURATION`] (`takeover`
///    carries no duration argument). `ON CONFLICT ... DO NOTHING RETURNING
///    id` — never a plain `INSERT` + catch-`23505` (the SAME rationale
///    `acquire_body`'s own doc comment states: a raised error aborts the
///    WHOLE Postgres transaction, poisoning it for the follow-up read the
///    `ResourceHeld` refusal below needs). A `NULL` RETURNING (a concurrent
///    successor grabbed the freed slot) reads the new occupant's holder +
///    expiry and refuses `ResourceHeld` — Q3 decision, matching the
///    acquire-loser grammar rather than a bespoke takeover-race code. A
///    refusal here rolls the WHOLE transaction back (`WriteResult::Refuse`'s
///    own contract), so the supersede UPDATE above is undone along with it —
///    the prior row is left exactly as it was on any refused path.
/// 5. **Always stage the audit on success (R-0066-b):** the four pinned
///    evidence fields, via [`AuditRecord::lease_takeover`].
async fn takeover_body(
    tx: &mut CoordinationTxn,
    session_id: Uuid,
    resource: &str,
) -> Result<WriteResult<TakenOverLease>, StorageFailure> {
    // (1) Every `claim` action requires a live attachment first (R-0064-e).
    let (new_holder_actor_id, new_holder_role_instance) =
        match resolve_acting_actor(tx, session_id).await? {
            Ok(actor) => actor,
            Err(refusal) => return Ok(WriteResult::Refuse(refusal)),
        };

    // (2) Parse + validate the resource identifier (R-0067-a); bar the
    // reserved `actor` family request-side (R-0067-c).
    let resource_id = match resource_id::parse(resource) {
        Ok(id) => id,
        Err(_) => return Ok(WriteResult::Refuse(Refusal::InvalidResource)),
    };
    if resource_id.family().is_reserved() {
        return Ok(WriteResult::Refuse(Refusal::ReservedFamily));
    }

    let workspace_id = tx.workspace_id();
    let resource_str = resource_id.to_string();

    // (3) Lock the resource's current live row; read the store clock in the
    // same query (R-0065-e).
    let live: Option<(Uuid, Uuid, DateTime<Utc>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, holder_actor_id, expires_at, now()
           FROM leases
          WHERE workspace_id = $1 AND resource = $2 AND terminal_state IS NULL
          FOR UPDATE",
    )
    .bind(workspace_id)
    .bind(&resource_str)
    .fetch_optional(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    let (prior_lease_id, prior_holder_actor_id, prior_expires_at, store_now) = match live {
        Some(row) => row,
        None => return Ok(WriteResult::Refuse(Refusal::LeaseNotFound)),
    };

    if store_now < prior_expires_at {
        return Ok(WriteResult::Refuse(Refusal::NotExpired));
    }

    // (4) Expired — supersede the prior row FIRST (frees the unique slot),
    // then insert the successor with a pre-generated id so `superseded_by`
    // is set in this one UPDATE.
    let new_lease_id = Uuid::new_v4();
    sqlx::query(
        "UPDATE leases
            SET terminal_state = 'taken_over',
                terminated_at  = $1,
                superseded_by  = $2
          WHERE id = $3",
    )
    .bind(store_now)
    .bind(new_lease_id)
    .bind(prior_lease_id)
    .execute(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    let new_expires_at =
        store_now + chrono::TimeDelta::seconds(LEASE_DEFAULT_DURATION.as_secs() as i64);

    let inserted: Option<(Uuid,)> = sqlx::query_as(
        "INSERT INTO leases
             (id, workspace_id, resource, holder_actor_id, acquired_at, duration, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (workspace_id, resource) WHERE terminal_state IS NULL DO NOTHING
         RETURNING id",
    )
    .bind(new_lease_id)
    .bind(workspace_id)
    .bind(&resource_str)
    .bind(new_holder_actor_id)
    .bind(store_now)
    .bind(LEASE_DEFAULT_DURATION.as_secs() as i64)
    .bind(new_expires_at)
    .fetch_optional(tx.conn())
    .await
    .map_err(|e| StorageFailure(Box::new(e)))?;

    if inserted.is_none() {
        // (Q3) A concurrent successor grabbed the freed slot — read its
        // holder + expiry (the workspace-visible facts, mirrors `acquire`'s
        // own `ResourceHeld` construction) and refuse. This branch rolls the
        // whole transaction back (including the supersede UPDATE above), so
        // the prior row is left untouched on this path.
        let holder: (Uuid, DateTime<Utc>) = sqlx::query_as(
            "SELECT holder_actor_id, expires_at
               FROM leases
              WHERE workspace_id = $1 AND resource = $2 AND terminal_state IS NULL",
        )
        .bind(workspace_id)
        .bind(&resource_str)
        .fetch_one(tx.conn())
        .await
        .map_err(|e| StorageFailure(Box::new(e)))?;
        return Ok(WriteResult::Refuse(Refusal::ResourceHeld {
            holder_actor_id: holder.0,
            expires_at: holder.1,
        }));
    }

    // (5) Always audit on success (R-0066-b) — the four pinned evidence
    // fields.
    tx.stage_audit(AuditRecord::lease_takeover(
        workspace_id,
        prior_holder_actor_id,
        new_holder_actor_id,
        ExpiryEvidence {
            expires_at: prior_expires_at,
            observed_now: store_now,
        },
    ));

    Ok(WriteResult::Commit(TakenOverLease {
        lease: AcquiredLease {
            lease_id: new_lease_id,
            resource: resource_str,
            holder_actor_id: new_holder_actor_id,
            holder_role_instance: new_holder_role_instance,
            acquired_at: store_now,
            expires_at: new_expires_at,
        },
        prior_holder_actor_id,
        prior_expires_at,
    }))
}

/// Render a committed `takeover` as the §API Contract response: the same
/// lease object [`acquire_response`] renders, PLUS `superseded: {
/// prior_holder, expired_at }` naming the deposed prior holder and the
/// (past) expiry that authorized the recovery.
fn takeover_response(taken_over: &TakenOverLease) -> CallToolResult {
    let mut obj = lease_object(&taken_over.lease);
    if let Some(map) = obj.as_object_mut() {
        map.insert(
            "superseded".to_owned(),
            serde_json::json!({
                "prior_holder": taken_over.prior_holder_actor_id.to_string(),
                "expired_at": taken_over.prior_expires_at.to_rfc3339(),
            }),
        );
    }
    CallToolResult::structured(obj)
}
