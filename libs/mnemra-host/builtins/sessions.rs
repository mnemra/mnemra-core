//! Sessions builtin: per-MCP-connection session state (R-0015-e).
//!
//! # V0 scope
//!
//! The `sessions` builtin manages per-MCP-connection session state. Session
//! context is the source of `WorkspaceCtx` construction (R-0015-e, R-0006-b).
//!
//! At V0 the MCP server (Task 23) does not yet exist. This builtin models the
//! session state shape and provides CRUD operations for testing.
//!
//! # Task 23 handoff seam (R-0015-e → R-0006-b)
//!
//! When the MCP server (Task 23) lands, it will:
//!
//! 1. On each incoming MCP connection: call `sessions::open(pool, ctx,
//!    user_id, agent_id)` to record the session start, where `ctx` is the
//!    connection's `WorkspaceCtx`.
//! 2. Construct `WorkspaceCtx` via the single production site in
//!    `auth::resolve::from_token(workspace_id, scopes, token_id)` using data
//!    derived from the session's workspace_id and the token that authenticated
//!    the connection.
//! 3. Thread that `WorkspaceCtx` through every host-fn called during the session.
//! 4. On connection close: call `sessions::close(pool, ctx, session_id)` to
//!    record the ended_at timestamp.
//!
//! The sessions table is the authoritative per-connection state record. The
//! `WorkspaceCtx` is constructed FROM session data but is NOT stored in the
//! table — it is an in-memory ephemeral object created per-request.
//!
//! `WorkspaceCtx` construction belongs exclusively to `auth::resolve::from_token`
//! (R-0006-b). This builtin does NOT construct one.
//!
//! # Schema
//!
//! Uses the `sessions` table created by migration 12 (Task 15):
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS sessions (
//!     id           UUID        NOT NULL DEFAULT gen_random_uuid(),
//!     workspace_id UUID        NOT NULL,
//!     user_id      UUID        NOT NULL,
//!     agent_id     UUID        NOT NULL,
//!     created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
//!     ended_at     TIMESTAMPTZ,
//!     CONSTRAINT sessions_pkey PRIMARY KEY (id)
//! )
//! ```

use crate::auth::workspace_ctx::WorkspaceCtx;
use sqlx::PgPool;
use std::fmt;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Session record
// ---------------------------------------------------------------------------

/// A session row returned by list/get operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub agent_id: Uuid,
    /// `None` while the session is active; `Some(ts)` after `close()`.
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Error returned by session operations.
#[derive(Debug)]
pub enum SessionError {
    /// The requested session was not found.
    NotFound { id: Uuid },
    /// A database error occurred.
    Db(sqlx::Error),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::NotFound { id } => write!(f, "session not found: {id}"),
            SessionError::Db(e) => write!(f, "session db error: {e}"),
        }
    }
}

impl std::error::Error for SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SessionError::Db(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for SessionError {
    fn from(e: sqlx::Error) -> Self {
        SessionError::Db(e)
    }
}

// ---------------------------------------------------------------------------
// Session lifecycle operations
// ---------------------------------------------------------------------------

/// Open a new session for the given workspace, user, and agent.
///
/// Returns the new session with a generated id and `ended_at = None`.
///
/// # Task 23 seam
///
/// This is called at MCP connection open. The returned `session.id` is the
/// session identifier threaded through the connection lifetime.
pub async fn open(
    pool: &PgPool,
    ctx: &WorkspaceCtx,
    user_id: Uuid,
    agent_id: Uuid,
) -> Result<Session, SessionError> {
    let workspace_id = ctx.workspace_id();
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, user_id, agent_id)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(user_id)
    .bind(agent_id)
    .execute(pool)
    .await?;

    Ok(Session {
        id,
        workspace_id,
        user_id,
        agent_id,
        ended_at: None,
    })
}

/// Close a session by recording its end timestamp.
///
/// Returns `SessionError::NotFound` if no session with that ID exists.
///
/// # Task 23 seam
///
/// This is called at MCP connection close.
pub async fn close(
    pool: &PgPool,
    ctx: &WorkspaceCtx,
    session_id: Uuid,
) -> Result<(), SessionError> {
    let rows = sqlx::query(
        "UPDATE sessions SET ended_at = now()
         WHERE id = $1 AND workspace_id = $2 AND ended_at IS NULL",
    )
    .bind(session_id)
    .bind(ctx.workspace_id())
    .execute(pool)
    .await?
    .rows_affected();

    if rows == 0 {
        // Either not found or already closed — treat as NotFound.
        Err(SessionError::NotFound { id: session_id })
    } else {
        Ok(())
    }
}

/// List active (not yet closed) sessions for a workspace.
/// Session row tuple returned from the DB query.
type SessionRow = (
    Uuid,
    Uuid,
    Uuid,
    Uuid,
    Option<chrono::DateTime<chrono::Utc>>,
);

pub async fn list_active_by_workspace(
    pool: &PgPool,
    ctx: &WorkspaceCtx,
) -> Result<Vec<Session>, SessionError> {
    let workspace_id = ctx.workspace_id();
    let rows: Vec<SessionRow> = sqlx::query_as(
        "SELECT id, workspace_id, user_id, agent_id, ended_at
         FROM sessions
         WHERE workspace_id = $1 AND ended_at IS NULL
         ORDER BY created_at",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, workspace_id, user_id, agent_id, ended_at)| Session {
            id,
            workspace_id,
            user_id,
            agent_id,
            ended_at,
        })
        .collect())
}
