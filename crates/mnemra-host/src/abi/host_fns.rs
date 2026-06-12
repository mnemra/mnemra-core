//! Host-fn binding signatures — one function per WIT entry in `wit/host.wit`.
//!
//! Bodies are unimplemented stubs (`todo!()`).  The type signatures enforce the
//! WorkspaceCtx-first calling convention and make the wrong thing inexpressible:
//! every write-path function accepts only `ctx: WorkspaceCtx` plus domain
//! params; there is no `workspace_id: String` parameter anywhere in this module.
//! A caller that tries to pass a bare workspace ID to a write-path function
//! finds no matching parameter slot — the constraint is structural, not enforced
//! by a runtime check.
//!
//! # Why workspace-id is inexpressible as a standalone param
//!
//! The workspace identity lives inside `WorkspaceCtx.workspace_id`.  The host
//! derives it before constructing the `WorkspaceCtx` value; the only way to
//! supply a workspace identity to a host-fn is to construct a `WorkspaceCtx`
//! and pass it as `ctx`.  There is no `workspace_id: String` typed parameter
//! to pass, so R-0012-d / R-0003-d violations cannot be expressed at the Rust
//! type level.

use super::{DispatchError, DispatchOutcome, DispatchWrapper, Stability};

// ---------------------------------------------------------------------------
// Shared types (mirror of wit types)
// ---------------------------------------------------------------------------

/// Mirror of the WIT `workspace-ctx` record.
///
/// Carries the host-derived workspace identity.  Constructed by the host
/// before dispatching into plugin code; plugins receive this value via the
/// host-fn ABI and cannot forge it without a matching host-fn call.
///
/// Note: the WIT model uses a `record` rather than a `resource` for this
/// type because `wit-parser`'s `type_is_named` resolution requires a named
/// typedef at the `Type::Id` level, which records satisfy.  Resource
/// own/borrow handles would provide stronger spoof-resistance; this is
/// flagged in the completion report.
#[derive(Debug, Clone)]
pub struct WorkspaceCtx {
    pub workspace_id: String,
}

/// Opaque JSON value — serialised as a UTF-8 string.
/// Mirrors `type json = string` in WIT (R-0012-f: never list<u8>).
pub type Json = String;

// ---------------------------------------------------------------------------
// artifact interface
// ---------------------------------------------------------------------------

/// Stability constant shared by all `artifact` functions.
const ARTIFACT_STABILITY: Stability = Stability::Stable;

/// `artifact-create` — creates a new artifact and returns its generated id.
///
/// R-0012-a, R-0006-a
pub fn artifact_create(
    _ctx: WorkspaceCtx,
    _type_name: &str,
    _frontmatter: Json,
    _body: Option<String>,
) -> Result<DispatchOutcome<String>, DispatchError> {
    DispatchWrapper::invoke(&ARTIFACT_STABILITY, "artifact-create", || {
        todo!("artifact-create: stub — storage wired in Task 5")
    })
}

/// `artifact-update` — patches frontmatter and/or body of an existing artifact.
///
/// R-0012-a, R-0006-a
pub fn artifact_update(
    _ctx: WorkspaceCtx,
    _id: &str,
    _frontmatter_patch: Json,
    _body: Option<String>,
) -> Result<DispatchOutcome<()>, DispatchError> {
    DispatchWrapper::invoke(&ARTIFACT_STABILITY, "artifact-update", || {
        todo!("artifact-update: stub — storage wired in Task 5")
    })
}

/// `artifact-get` — retrieves a single artifact by id.
///
/// R-0012-a, R-0006-a
pub fn artifact_get(
    _ctx: WorkspaceCtx,
    _id: &str,
) -> Result<DispatchOutcome<Option<String>>, DispatchError> {
    DispatchWrapper::invoke(&ARTIFACT_STABILITY, "artifact-get", || {
        todo!("artifact-get: stub — storage wired in Task 5")
    })
}

/// `artifact-list` — lists artifacts matching the given type and filter criteria.
///
/// R-0012-a, R-0006-a
pub fn artifact_list(
    _ctx: WorkspaceCtx,
    _type_name: &str,
    _filters: Json,
) -> Result<DispatchOutcome<Vec<String>>, DispatchError> {
    DispatchWrapper::invoke(&ARTIFACT_STABILITY, "artifact-list", || {
        todo!("artifact-list: stub — storage wired in Task 5")
    })
}

/// `artifact-delete` — permanently deletes an artifact.
///
/// Opt-in: only available when `host_fns.required` declares this capability
/// (R-0003-g).
///
/// R-0012-a, R-0003-g, R-0006-a
pub fn artifact_delete(
    _ctx: WorkspaceCtx,
    _id: &str,
) -> Result<DispatchOutcome<()>, DispatchError> {
    DispatchWrapper::invoke(&ARTIFACT_STABILITY, "artifact-delete", || {
        todo!("artifact-delete: stub — storage wired in Task 5")
    })
}

// ---------------------------------------------------------------------------
// metrics interface
// ---------------------------------------------------------------------------

/// Stability constant for `metrics` functions.
const METRICS_STABILITY: Stability = Stability::Stable;

/// `metrics-record` — records a single metric observation.
///
/// R-0012-a, R-0006-a
pub fn metrics_record(
    _ctx: WorkspaceCtx,
    _verb: &str,
    _duration_ms: u64,
    _outcome: &str,
) -> Result<DispatchOutcome<()>, DispatchError> {
    DispatchWrapper::invoke(&METRICS_STABILITY, "metrics-record", || {
        todo!("metrics-record: stub — metrics sink wired in later task")
    })
}

// ---------------------------------------------------------------------------
// log interface
// ---------------------------------------------------------------------------

/// Stability constant for `log` functions.
const LOG_STABILITY: Stability = Stability::Stable;

/// `log-emit` — emits a structured log line.
///
/// R-0012-a, R-0006-a
pub fn log_emit(
    _ctx: WorkspaceCtx,
    _level: &str,
    _message: &str,
    _context: Option<Json>,
) -> Result<DispatchOutcome<()>, DispatchError> {
    DispatchWrapper::invoke(&LOG_STABILITY, "log-emit", || {
        todo!("log-emit: stub — log sink wired in later task")
    })
}

// ---------------------------------------------------------------------------
// event interface
// ---------------------------------------------------------------------------

/// Stability constant for `event` functions.
const EVENT_STABILITY: Stability = Stability::Stable;

/// `event-emit` — emits a versioned domain event.
///
/// R-0012-a, R-0006-a
pub fn event_emit(
    _ctx: WorkspaceCtx,
    _event_type: &str,
    _event_version: u16,
    _payload: Json,
) -> Result<DispatchOutcome<()>, DispatchError> {
    DispatchWrapper::invoke(&EVENT_STABILITY, "event-emit", || {
        todo!("event-emit: stub — event bus wired in later task")
    })
}

// ---------------------------------------------------------------------------
// projection interface
// ---------------------------------------------------------------------------

/// Stability constant for `projection` functions.
const PROJECTION_STABILITY: Stability = Stability::Stable;

/// `projection-emit` — emits a named projection update.
///
/// Workspace is derived from `ctx`; workspace-id is never an explicit param
/// (R-0003-d).
///
/// R-0012-a, R-0003-d, R-0006-a
pub fn projection_emit(
    _ctx: WorkspaceCtx,
    _projection_name: &str,
    _data: Json,
) -> Result<DispatchOutcome<()>, DispatchError> {
    DispatchWrapper::invoke(&PROJECTION_STABILITY, "projection-emit", || {
        todo!("projection-emit: stub — projection sink wired in later task")
    })
}

// ---------------------------------------------------------------------------
// sampling interface (@unstable)
// ---------------------------------------------------------------------------

/// Stability constant for `sampling` functions — @unstable(feature = sampling-v0).
const SAMPLING_STABILITY: Stability = Stability::Unstable {
    feature: "sampling-v0",
};

/// `sampling-request` — requests an LLM sampling inference.
///
/// `context_ids` carries opaque artifact ID references; the host does not
/// resolve them to bodies inside this call (R-0012-b).
///
/// R-0012-b, R-0006-a
pub fn sampling_request(
    _ctx: WorkspaceCtx,
    _context_ids: Vec<String>,
    _prompt: &str,
) -> Result<DispatchOutcome<Option<String>>, DispatchError> {
    DispatchWrapper::invoke(&SAMPLING_STABILITY, "sampling-request", || {
        todo!("sampling-request: stub — LLM integration wired in later task")
    })
}

// ---------------------------------------------------------------------------
// secrets interface (read-only)
// ---------------------------------------------------------------------------

/// Stability constant for `secrets` functions.
const SECRETS_STABILITY: Stability = Stability::Stable;

/// `secrets-get` — retrieves a secret value by name.
///
/// Read-only; no write path to the secrets store at V0 (R-0012-c).
///
/// R-0012-c, R-0006-a
pub fn secrets_get(
    _ctx: WorkspaceCtx,
    _name: &str,
) -> Result<DispatchOutcome<Option<String>>, DispatchError> {
    DispatchWrapper::invoke(&SECRETS_STABILITY, "secrets-get", || {
        todo!("secrets-get: stub — secrets store wired in later task")
    })
}
