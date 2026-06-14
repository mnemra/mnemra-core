//! Workspaces builtin: workspace lifecycle management (R-0015-a, R-0015-h).
//!
//! # V0 scope
//!
//! The `workspaces` builtin manages workspace lifecycle (create, delete, list).
//! The `default` workspace is created during `schema::init::init()` (Task 7) and
//! is always guaranteed to exist after init. This builtin extends that foundation
//! with the full lifecycle surface.
//!
//! Solo deployment collapses tenancy to `default` (R-0015-h): a single-tenant
//! deployment uses the `default` workspace exclusively. The `default` workspace
//! row MUST always exist after init.
//!
//! # Schema
//!
//! Uses the `workspaces` table created by migration 1 (Task 7):
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS workspaces (
//!     id         UUID        NOT NULL DEFAULT gen_random_uuid(),
//!     name       TEXT        NOT NULL,
//!     created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
//!     CONSTRAINT workspaces_pkey    PRIMARY KEY (id),
//!     CONSTRAINT workspaces_name_uq UNIQUE      (name)
//! )
//! ```

use sqlx::PgPool;
use std::fmt;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Workspace record
// ---------------------------------------------------------------------------

/// A workspace row returned by list/get operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Error returned by workspace operations.
#[derive(Debug)]
pub enum WorkspaceError {
    /// A workspace with the given name already exists.
    AlreadyExists { name: String },
    /// The requested workspace was not found.
    NotFound { id: Uuid },
    /// The `default` workspace cannot be deleted (R-0015-h).
    CannotDeleteDefault,
    /// A database error occurred.
    Db(sqlx::Error),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceError::AlreadyExists { name } => {
                write!(f, "workspace already exists: {name}")
            }
            WorkspaceError::NotFound { id } => {
                write!(f, "workspace not found: {id}")
            }
            WorkspaceError::CannotDeleteDefault => {
                write!(
                    f,
                    "cannot delete the 'default' workspace (R-0015-h): \
                     solo deployment requires this workspace to always exist"
                )
            }
            WorkspaceError::Db(e) => write!(f, "workspace db error: {e}"),
        }
    }
}

impl std::error::Error for WorkspaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WorkspaceError::Db(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for WorkspaceError {
    fn from(e: sqlx::Error) -> Self {
        WorkspaceError::Db(e)
    }
}

// ---------------------------------------------------------------------------
// CRUD operations
// ---------------------------------------------------------------------------

/// Create a new workspace with the given name.
///
/// Returns `WorkspaceError::AlreadyExists` if a workspace with that name already exists.
pub async fn create(pool: &PgPool, name: &str) -> Result<Workspace, WorkspaceError> {
    let id = Uuid::new_v4();
    let result = sqlx::query(
        "INSERT INTO workspaces (id, name)
         VALUES ($1, $2)",
    )
    .bind(id)
    .bind(name)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(Workspace {
            id,
            name: name.to_string(),
        }),
        Err(e) => {
            // Check for unique constraint violation on `name`.
            let msg = e.to_string();
            if msg.contains("workspaces_name_uq") || msg.contains("unique") {
                Err(WorkspaceError::AlreadyExists {
                    name: name.to_string(),
                })
            } else {
                Err(WorkspaceError::Db(e))
            }
        }
    }
}

/// Delete a workspace by ID.
///
/// Returns `WorkspaceError::CannotDeleteDefault` if the target is the default workspace.
/// Returns `WorkspaceError::NotFound` if no workspace with that ID exists.
pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), WorkspaceError> {
    if id == crate::schema::init::DEFAULT_WORKSPACE_ID {
        return Err(WorkspaceError::CannotDeleteDefault);
    }

    let rows = sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();

    if rows == 0 {
        Err(WorkspaceError::NotFound { id })
    } else {
        Ok(())
    }
}

/// List all workspaces, ordered by creation time.
pub async fn list(pool: &PgPool) -> Result<Vec<Workspace>, WorkspaceError> {
    let rows: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, name FROM workspaces ORDER BY created_at")
            .fetch_all(pool)
            .await?;

    Ok(rows
        .into_iter()
        .map(|(id, name)| Workspace { id, name })
        .collect())
}
