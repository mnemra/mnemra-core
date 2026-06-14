//! Agents builtin: agent registration tied to user-workspace pairs (R-0015-c).
//!
//! # Agent identity derivation (R-0015-c)
//!
//! Agent identity is canonical at registration. The `agent_id` UUID is derived
//! deterministically as UUIDv5 over the UTF-8 encoding of the canonical tuple:
//!
//! ```text
//! namespace = Uuid::NAMESPACE_OID
//! name      = "<workspace_id>:<user_id>:<agent_name>"
//! ```
//!
//! This makes the id reproducible across registrations: given the same triple,
//! the same id is always derived. On a re-registration attempt, the derived id
//! is compared against the existing row. If they match the tuple is identical
//! (idempotent). If they differ it is a mismatch — a `RegisterError::IdentityMismatch`
//! is returned rather than silently overwriting or silently no-op-ing.
//!
//! # Mismatch semantics
//!
//! A mismatch can only occur if the caller supplies a `workspace_id`, `user_id`,
//! or `agent_name` that does not match the existing registration for that
//! `(workspace_id, user_id, agent_name)` triple. Because the unique constraint is
//! on the triple, and the derived id is a deterministic function of the triple,
//! a mismatch is structurally impossible in normal operation. `IdentityMismatch`
//! is therefore a defensive guard against:
//! - A caller that computes the id externally with a different derivation.
//! - A future schema migration that changes the derivation algorithm.
//!
//! # Schema
//!
//! Uses the `agents` table created by migration 10 (Task 15):
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS agents (
//!     id           UUID        NOT NULL,
//!     workspace_id UUID        NOT NULL,
//!     user_id      UUID        NOT NULL,
//!     agent_name   TEXT        NOT NULL,
//!     created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
//!     CONSTRAINT agents_pkey         PRIMARY KEY (id),
//!     CONSTRAINT agents_ws_user_name UNIQUE      (workspace_id, user_id, agent_name)
//! )
//! ```

use sqlx::PgPool;
use std::fmt;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Agent record
// ---------------------------------------------------------------------------

/// An agent row returned by list/get operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub agent_name: String,
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Error returned by agent operations.
#[derive(Debug)]
pub enum RegisterError {
    /// Agent already registered with this exact triple — idempotent success.
    ///
    /// The caller may treat this as success (same id returned) or inspect the
    /// existing registration.
    AlreadyRegistered { existing_id: Uuid },
    /// The supplied agent_id does not match the canonically derived id.
    ///
    /// This is a hard error: the caller is supplying an identity that does not
    /// match the canonical derivation from the given triple. Registration is
    /// refused; the existing row is returned for diagnostic purposes.
    ///
    /// # R-0015-c
    ///
    /// "agent identity derivation is canonical at registration; structured error
    /// on mismatch, not silent registration"
    IdentityMismatch {
        /// The id the caller attempted to register.
        supplied_id: Uuid,
        /// The canonically derived id for this triple.
        canonical_id: Uuid,
    },
    /// A database error occurred.
    Db(sqlx::Error),
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegisterError::AlreadyRegistered { existing_id } => {
                write!(f, "agent already registered with id {existing_id}")
            }
            RegisterError::IdentityMismatch {
                supplied_id,
                canonical_id,
            } => {
                write!(
                    f,
                    "agent identity mismatch (R-0015-c): supplied id {supplied_id} does not \
                     match the canonical derivation {canonical_id} — registration refused"
                )
            }
            RegisterError::Db(e) => write!(f, "agent registration db error: {e}"),
        }
    }
}

impl std::error::Error for RegisterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RegisterError::Db(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for RegisterError {
    fn from(e: sqlx::Error) -> Self {
        RegisterError::Db(e)
    }
}

/// Error returned by agent read/delete operations.
#[derive(Debug)]
pub enum AgentError {
    /// The requested agent was not found.
    NotFound { id: Uuid },
    /// A database error occurred.
    Db(sqlx::Error),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::NotFound { id } => write!(f, "agent not found: {id}"),
            AgentError::Db(e) => write!(f, "agent db error: {e}"),
        }
    }
}

impl std::error::Error for AgentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AgentError::Db(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for AgentError {
    fn from(e: sqlx::Error) -> Self {
        AgentError::Db(e)
    }
}

// ---------------------------------------------------------------------------
// Identity derivation
// ---------------------------------------------------------------------------

/// UUIDv5 namespace for agent identity derivation.
///
/// Using `Uuid::NAMESPACE_OID` as the fixed namespace for canonical derivation.
/// The name is the colon-separated tuple `<workspace_id>:<user_id>:<agent_name>`.
const AGENT_ID_NAMESPACE: Uuid = Uuid::NAMESPACE_OID;

/// Derive the canonical agent id from the registration triple.
///
/// The derivation is: `UUIDv5(NAMESPACE_OID, "<workspace_id>:<user_id>:<agent_name>")`.
///
/// This function is the single source of truth for agent identity derivation.
/// Both `register` and the test harness call it to verify canonical registration.
pub fn derive_agent_id(workspace_id: Uuid, user_id: Uuid, agent_name: &str) -> Uuid {
    let name = format!("{workspace_id}:{user_id}:{agent_name}");
    Uuid::new_v5(&AGENT_ID_NAMESPACE, name.as_bytes())
}

// ---------------------------------------------------------------------------
// CRUD operations
// ---------------------------------------------------------------------------

/// Register an agent for the given user-workspace pair.
///
/// The `agent_id` is always derived canonically via `derive_agent_id`. Any
/// supplied `agent_id` is checked against the canonical derivation — a mismatch
/// returns `RegisterError::IdentityMismatch` (R-0015-c).
///
/// If the agent is already registered with the identical triple, returns
/// `RegisterError::AlreadyRegistered` with the existing id (idempotent).
///
/// # Registration flow
///
/// 1. Derive the canonical id = `UUIDv5(NAMESPACE_OID, "<workspace_id>:<user_id>:<agent_name>")`.
/// 2. If the caller supplied an `agent_id` that differs from the canonical id →
///    `IdentityMismatch`.
/// 3. Attempt INSERT. If the unique constraint fires (triple already exists):
///    a. Look up the existing row's id.
///    b. If it matches the canonical id → `AlreadyRegistered`.
///    c. If it differs → `IdentityMismatch` (should be unreachable given step 2,
///    but is structurally guarded for forward-safety).
/// 4. Return the new `Agent`.
pub async fn register(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
    agent_name: &str,
    supplied_agent_id: Option<Uuid>,
) -> Result<Agent, RegisterError> {
    let canonical_id = derive_agent_id(workspace_id, user_id, agent_name);

    // Step 2: mismatch guard — if the caller supplied a specific id, verify it.
    if let Some(supplied) = supplied_agent_id
        && supplied != canonical_id
    {
        return Err(RegisterError::IdentityMismatch {
            supplied_id: supplied,
            canonical_id,
        });
    }

    // Step 3: attempt insertion.
    let result = sqlx::query(
        "INSERT INTO agents (id, workspace_id, user_id, agent_name)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(canonical_id)
    .bind(workspace_id)
    .bind(user_id)
    .bind(agent_name)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(Agent {
            id: canonical_id,
            workspace_id,
            user_id,
            agent_name: agent_name.to_string(),
        }),
        Err(e) => {
            // Check for unique constraint violation on the triple.
            let msg = e.to_string();
            if msg.contains("agents_ws_user_name") || msg.contains("unique") {
                // The triple already exists. Look up the existing row id.
                let existing: Option<(Uuid,)> = sqlx::query_as(
                    "SELECT id FROM agents
                     WHERE workspace_id = $1 AND user_id = $2 AND agent_name = $3",
                )
                .bind(workspace_id)
                .bind(user_id)
                .bind(agent_name)
                .fetch_optional(pool)
                .await
                .map_err(RegisterError::Db)?;

                if let Some((existing_id,)) = existing {
                    if existing_id == canonical_id {
                        Err(RegisterError::AlreadyRegistered { existing_id })
                    } else {
                        // Structurally unreachable: the derivation is deterministic,
                        // and the mismatch guard above already caught any supplied-id
                        // divergence. This guard is for forward-safety.
                        Err(RegisterError::IdentityMismatch {
                            supplied_id: existing_id,
                            canonical_id,
                        })
                    }
                } else {
                    // Constraint fired but row vanished — race; surface as Db.
                    Err(RegisterError::Db(e))
                }
            } else {
                Err(RegisterError::Db(e))
            }
        }
    }
}

/// List all agents in a workspace, ordered by creation time.
pub async fn list_by_workspace(
    pool: &PgPool,
    workspace_id: Uuid,
) -> Result<Vec<Agent>, AgentError> {
    let rows: Vec<(Uuid, Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT id, workspace_id, user_id, agent_name
         FROM agents
         WHERE workspace_id = $1
         ORDER BY created_at",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, workspace_id, user_id, agent_name)| Agent {
            id,
            workspace_id,
            user_id,
            agent_name,
        })
        .collect())
}
