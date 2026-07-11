//! Actors builtin: the P-0018 `actors` core entity (D-ENT / D-ACTOR).
//!
//! # Scope of this landing (Task 1, coordination-wedge)
//!
//! `actors` lands here as a minimal STANDALONE entity, minted directly by
//! role-instance name. There is **no FK linkage** from `actors` to the
//! existing `users`/`agents`/`sessions` builtins in this landing — the
//! fuller P-0018 unification (rewiring those builtins to populate `actors`)
//! is deferred to P-0018's own landing (Gap A, decided 2026-07-11). This
//! module does not read or write any of the identity builtins.
//!
//! # D-ACTOR: one table, closed `actor_type`
//!
//! `actor_type` is a closed set of exactly three members: `human`, `agent`,
//! `system`. A value outside the set is rejected AT WRITE by the
//! `actors_actor_type_chk` CHECK constraint (schema-level enforcement — see
//! `guarantee-by-mechanism` G2: a value invariant enforced by the schema, not
//! by a comment plus the happen-to-be-current set of writers).
//!
//! # Resolve-or-create by role-instance name
//!
//! [`resolve_or_create`] is the entry point: the same `(workspace_id, name)`
//! triple always resolves to the same `actor_id` and mints exactly one row;
//! distinct names (or the same name in a distinct workspace, since
//! uniqueness is workspace-scoped) mint distinct ids.
//!
//! # Schema
//!
//! Uses the `actors` table created by migration 17 (`schema/init.rs`):
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS actors (
//!     id           UUID        NOT NULL DEFAULT gen_random_uuid(),
//!     workspace_id UUID        NOT NULL,
//!     actor_type   TEXT        NOT NULL,
//!     name         TEXT        NOT NULL,
//!     created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
//!     CONSTRAINT actors_pkey           PRIMARY KEY (id),
//!     CONSTRAINT actors_ws_name_uq     UNIQUE      (workspace_id, name),
//!     CONSTRAINT actors_actor_type_chk CHECK       (actor_type IN ('human', 'agent', 'system'))
//! )
//! ```

use crate::auth::workspace_ctx::WorkspaceCtx;
use sqlx::PgPool;
use std::fmt;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// ActorType — the closed identity axis (P-0018 D-ACTOR)
// ---------------------------------------------------------------------------

/// The actor-identity axis: who acted — a person, an AI team member, or the
/// host itself. A closed set of exactly three members (P-0018 D-ACTOR).
///
/// Distinct from, and composing with, the two adjacent "source-role" axes
/// documented in P-0018 (P-0015's trust/source-role axis, P-0016's
/// edge-`origin` axis) — this type is the identity axis only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorType {
    /// A person.
    Human,
    /// An AI team member / automated actor.
    Agent,
    /// Host/system-initiated action (migration handler, host-side extractor,
    /// scheduled job).
    System,
}

impl ActorType {
    /// The canonical DB representation — must match the
    /// `actors_actor_type_chk` CHECK constraint's literal set exactly.
    fn as_db_str(self) -> &'static str {
        match self {
            ActorType::Human => "human",
            ActorType::Agent => "agent",
            ActorType::System => "system",
        }
    }
}

impl fmt::Display for ActorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Error returned when a stored `actor_type` value does not parse into a
/// known [`ActorType`] variant.
///
/// Structurally guarded by the `actors_actor_type_chk` CHECK constraint —
/// unreachable in normal operation (mirrors the "structurally unreachable
/// but guarded" pattern in `builtins/agents.rs::RegisterError`). Retained as
/// a real error path (not a panic) because a row could in principle predate
/// the constraint or be written by a future migration that changes it.
#[derive(Debug)]
pub struct ActorTypeParseError(pub String);

impl fmt::Display for ActorTypeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown actor_type '{}' — expected one of: human, agent, system \
             (structurally guarded by actors_actor_type_chk; this indicates \
             constraint/application drift)",
            self.0
        )
    }
}

impl std::error::Error for ActorTypeParseError {}

impl std::str::FromStr for ActorType {
    type Err = ActorTypeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "human" => Ok(ActorType::Human),
            "agent" => Ok(ActorType::Agent),
            "system" => Ok(ActorType::System),
            other => Err(ActorTypeParseError(other.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// Actor record
// ---------------------------------------------------------------------------

/// An actor row returned by resolve/list operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Actor {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub actor_type: ActorType,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Error returned by actor operations.
#[derive(Debug)]
pub enum ActorError {
    /// The stored `actor_type` failed to parse (see [`ActorTypeParseError`]).
    InvalidActorType(ActorTypeParseError),
    /// A database error occurred.
    Db(sqlx::Error),
}

impl fmt::Display for ActorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActorError::InvalidActorType(e) => write!(f, "actor error: {e}"),
            ActorError::Db(e) => write!(f, "actor db error: {e}"),
        }
    }
}

impl std::error::Error for ActorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ActorError::InvalidActorType(e) => Some(e),
            ActorError::Db(e) => Some(e),
        }
    }
}

impl From<sqlx::Error> for ActorError {
    fn from(e: sqlx::Error) -> Self {
        ActorError::Db(e)
    }
}

impl From<ActorTypeParseError> for ActorError {
    fn from(e: ActorTypeParseError) -> Self {
        ActorError::InvalidActorType(e)
    }
}

// ---------------------------------------------------------------------------
// CRUD operations
// ---------------------------------------------------------------------------

/// Row tuple returned from the actors table for decoding into [`Actor`].
type ActorRow = (Uuid, Uuid, String, String);

fn decode_actor_row(row: ActorRow) -> Result<Actor, ActorError> {
    let (id, workspace_id, actor_type_raw, name) = row;
    let actor_type: ActorType = actor_type_raw.parse()?;
    Ok(Actor {
        id,
        workspace_id,
        actor_type,
        name,
    })
}

/// Resolve an actor by role-instance name within `ctx`'s workspace, minting
/// a new row if none exists yet.
///
/// The same `(workspace_id, name)` triple always resolves to the same
/// `actor_id` and mints exactly one row — repeated calls are idempotent.
/// Distinct names within a workspace mint distinct ids; the same name in a
/// distinct workspace also mints a distinct id (uniqueness is
/// workspace-scoped via `actors_ws_name_uq`).
///
/// If a row already exists for this `(workspace_id, name)`, the row's
/// PERSISTED `actor_type` is returned (not necessarily the `actor_type`
/// passed on this call) — resolution never silently changes an existing
/// actor's type.
///
/// # Race safety
///
/// The insert uses `ON CONFLICT (workspace_id, name) DO NOTHING`, so two
/// concurrent callers resolving the same triple can never mint two rows;
/// whichever insert loses the race falls through to the SELECT and observes
/// the winner's row.
pub async fn resolve_or_create(
    pool: &PgPool,
    ctx: &WorkspaceCtx,
    actor_type: ActorType,
    name: &str,
) -> Result<Actor, ActorError> {
    let workspace_id = ctx.workspace_id();

    sqlx::query(
        "INSERT INTO actors (workspace_id, actor_type, name)
         VALUES ($1, $2, $3)
         ON CONFLICT (workspace_id, name) DO NOTHING",
    )
    .bind(workspace_id)
    .bind(actor_type.as_db_str())
    .bind(name)
    .execute(pool)
    .await?;

    let row: ActorRow = sqlx::query_as(
        "SELECT id, workspace_id, actor_type, name
         FROM actors
         WHERE workspace_id = $1 AND name = $2",
    )
    .bind(workspace_id)
    .bind(name)
    .fetch_one(pool)
    .await?;

    decode_actor_row(row)
}

/// List all actors in a workspace, ordered by creation time.
pub async fn list_by_workspace(
    pool: &PgPool,
    ctx: &WorkspaceCtx,
) -> Result<Vec<Actor>, ActorError> {
    let workspace_id = ctx.workspace_id();
    let rows: Vec<ActorRow> = sqlx::query_as(
        "SELECT id, workspace_id, actor_type, name
         FROM actors
         WHERE workspace_id = $1
         ORDER BY created_at",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(decode_actor_row).collect()
}
