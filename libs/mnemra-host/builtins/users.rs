//! Users builtin: user identity record management (R-0015-b).
//!
//! # V0 scope
//!
//! The `users` builtin manages user identity records. Users are global (not
//! per-workspace) — they are referenced by agent registrations and session state.
//!
//! # Schema
//!
//! Uses the `users` table created by migration 8 (Task 15):
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS users (
//!     id           UUID        NOT NULL DEFAULT gen_random_uuid(),
//!     username     TEXT        NOT NULL,
//!     display_name TEXT,
//!     created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
//!     CONSTRAINT users_pkey        PRIMARY KEY (id),
//!     CONSTRAINT users_username_uq UNIQUE      (username)
//! )
//! ```

use sqlx::PgPool;
use std::fmt;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// User record
// ---------------------------------------------------------------------------

/// A user row returned by list/get operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Error returned by user operations.
#[derive(Debug)]
pub enum UserError {
    /// A user with the given username already exists.
    AlreadyExists { username: String },
    /// The requested user was not found.
    NotFound { id: Uuid },
    /// A database error occurred.
    Db(sqlx::Error),
}

impl fmt::Display for UserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserError::AlreadyExists { username } => {
                write!(f, "user already exists: {username}")
            }
            UserError::NotFound { id } => {
                write!(f, "user not found: {id}")
            }
            UserError::Db(e) => write!(f, "user db error: {e}"),
        }
    }
}

impl std::error::Error for UserError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            UserError::Db(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for UserError {
    fn from(e: sqlx::Error) -> Self {
        UserError::Db(e)
    }
}

// ---------------------------------------------------------------------------
// CRUD operations
// ---------------------------------------------------------------------------

/// Register a new user with the given username and optional display name.
///
/// Returns `UserError::AlreadyExists` if a user with that username already exists.
pub async fn register(
    pool: &PgPool,
    username: &str,
    display_name: Option<&str>,
) -> Result<User, UserError> {
    let id = Uuid::new_v4();
    let result = sqlx::query(
        "INSERT INTO users (id, username, display_name)
         VALUES ($1, $2, $3)",
    )
    .bind(id)
    .bind(username)
    .bind(display_name)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(User {
            id,
            username: username.to_string(),
            display_name: display_name.map(str::to_string),
        }),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("users_username_uq") || msg.contains("unique") {
                Err(UserError::AlreadyExists {
                    username: username.to_string(),
                })
            } else {
                Err(UserError::Db(e))
            }
        }
    }
}

/// Look up a user by ID.
///
/// Returns `UserError::NotFound` if no user with that ID exists.
pub async fn get(pool: &PgPool, id: Uuid) -> Result<User, UserError> {
    let row: Option<(Uuid, String, Option<String>)> =
        sqlx::query_as("SELECT id, username, display_name FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

    row.map(|(id, username, display_name)| User {
        id,
        username,
        display_name,
    })
    .ok_or(UserError::NotFound { id })
}

/// List all users, ordered by creation time.
pub async fn list(pool: &PgPool) -> Result<Vec<User>, UserError> {
    let rows: Vec<(Uuid, String, Option<String>)> =
        sqlx::query_as("SELECT id, username, display_name FROM users ORDER BY created_at")
            .fetch_all(pool)
            .await?;

    Ok(rows
        .into_iter()
        .map(|(id, username, display_name)| User {
            id,
            username,
            display_name,
        })
        .collect())
}
