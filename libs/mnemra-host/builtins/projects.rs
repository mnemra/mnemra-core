//! Projects builtin: project registry (R-0015-g).
//!
//! # V0 scope
//!
//! The `projects` builtin manages the project registry. Project identity is a
//! prerequisite for plugin scoping: no plugin is scoped to a project before
//! that project's record exists (R-0015-g).
//!
//! Projects are scoped to a workspace. Plugin scoping at registration-time
//! will reference a `project_id` from this table — if the project does not
//! exist, plugin registration for that scope is refused.
//!
//! # Schema
//!
//! Uses the `projects` table created by migration 15 (Task 15):
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS projects (
//!     id           UUID        NOT NULL DEFAULT gen_random_uuid(),
//!     workspace_id UUID        NOT NULL,
//!     name         TEXT        NOT NULL,
//!     created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
//!     CONSTRAINT projects_pkey        PRIMARY KEY (id),
//!     CONSTRAINT projects_ws_name_uq  UNIQUE      (workspace_id, name)
//! )
//! ```

use sqlx::PgPool;
use std::fmt;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Project record
// ---------------------------------------------------------------------------

/// A project row returned by list/get operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Error returned by project operations.
#[derive(Debug)]
pub enum ProjectError {
    /// A project with the given name already exists in this workspace.
    AlreadyExists { workspace_id: Uuid, name: String },
    /// The requested project was not found.
    NotFound { id: Uuid },
    /// A database error occurred.
    Db(sqlx::Error),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectError::AlreadyExists { workspace_id, name } => {
                write!(
                    f,
                    "project '{name}' already exists in workspace {workspace_id}"
                )
            }
            ProjectError::NotFound { id } => write!(f, "project not found: {id}"),
            ProjectError::Db(e) => write!(f, "project db error: {e}"),
        }
    }
}

impl std::error::Error for ProjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ProjectError::Db(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for ProjectError {
    fn from(e: sqlx::Error) -> Self {
        ProjectError::Db(e)
    }
}

// ---------------------------------------------------------------------------
// CRUD operations
// ---------------------------------------------------------------------------

/// Create a new project in the given workspace.
///
/// Returns `ProjectError::AlreadyExists` if a project with that name already
/// exists in the workspace.
pub async fn create(
    pool: &PgPool,
    workspace_id: Uuid,
    name: &str,
) -> Result<Project, ProjectError> {
    let id = Uuid::new_v4();
    let result = sqlx::query(
        "INSERT INTO projects (id, workspace_id, name)
         VALUES ($1, $2, $3)",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(name)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(Project {
            id,
            workspace_id,
            name: name.to_string(),
        }),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("projects_ws_name_uq") || msg.contains("unique") {
                Err(ProjectError::AlreadyExists {
                    workspace_id,
                    name: name.to_string(),
                })
            } else {
                Err(ProjectError::Db(e))
            }
        }
    }
}

/// Delete a project by ID.
///
/// Returns `ProjectError::NotFound` if no project with that ID exists.
pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), ProjectError> {
    let rows = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();

    if rows == 0 {
        Err(ProjectError::NotFound { id })
    } else {
        Ok(())
    }
}

/// List all projects in a workspace, ordered by creation time.
///
/// # R-0015-g
///
/// No plugin is scoped to a project before that project's record exists. The
/// plugin scoping layer (Task 19) will call this to verify a project exists
/// before accepting a registration scoped to it.
pub async fn list_by_workspace(
    pool: &PgPool,
    workspace_id: Uuid,
) -> Result<Vec<Project>, ProjectError> {
    let rows: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT id, workspace_id, name
         FROM projects
         WHERE workspace_id = $1
         ORDER BY created_at",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, workspace_id, name)| Project {
            id,
            workspace_id,
            name,
        })
        .collect())
}

/// Check whether a project with the given ID exists in the given workspace.
///
/// Returns `true` if the project exists, `false` otherwise.
///
/// # R-0015-g
///
/// This is the prerequisite check for plugin scoping (Task 19). A plugin
/// registration scoped to a project MUST call this first; if it returns
/// `false`, the registration is refused.
pub async fn exists(pool: &PgPool, workspace_id: Uuid, id: Uuid) -> Result<bool, ProjectError> {
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM projects WHERE workspace_id = $1 AND id = $2")
            .bind(workspace_id)
            .bind(id)
            .fetch_one(pool)
            .await?;

    Ok(row.0 > 0)
}
