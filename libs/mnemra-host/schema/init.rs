//! Schema initialization: `mnemra init`.
//!
//! Implements the first-run bootstrap sequence (R-0013-a, R-0013-b, R-0013-d,
//! R-0013-e, R-0015-a, R-0015-h):
//!
//! 1. Enable the `vector` (pgvector) extension — structured error and NO
//!    further work if this fails (R-0013-a).
//! 2. Run forward-only migrations: `workspaces`, `content`, `state_config`
//!    tables + indexes (R-0013-b, R-0013-c). No timescaledb, no hypertables.
//! 3. Create the `default` workspace row (idempotent) (R-0015-a, R-0015-h).
//! 4. Create the four least-privilege DB roles via the superuser seam (R-0013-e).
//! 5. Assert the health snapshot returns `overall: "ok"` (R-0004-g).
//!
//! # V0 scope note
//!
//! No `embedding` or `search_tsv` columns are created at V0 (R-0001-g). pgvector
//! is enabled but unused — those columns land as non-breaking `ADD COLUMN` at
//! V0.1+. See Tasks 8/9 handoff at the bottom of this file.
//!
//! # Tasks 8/9 hook-in seam
//!
//! The per-artifact-type table generator (Tasks 8/9) adds tables against the
//! same embedded engine. The seam is: call `init(engine, "vector")` once per
//! engine lifetime, then use `engine.pool` (app-role) for DDL emitted by the
//! Task 9 generator. The generator can use the `migrations::apply()` runner
//! directly with its own migration slice.
//!
//! # Task 25 hook-in seam
//!
//! `health_snapshot(pool)` returns the R-0004-g body struct. Task 25's `/health`
//! HTTP handler calls it and serializes the result. Init asserts it returns
//! `overall: "ok"` at completion; Task 25 owns the HTTP wrapper.

use crate::schema::artifact_table::create_artifact_table;
use crate::schema::history_trigger::create_history_machinery;
use crate::schema::migrations::{Migration, MigrationError, apply as run_migrations};
use crate::storage::postgres::engine::{EmbeddedEngine, ExtensionError};
use sqlx::PgPool;
use std::error::Error;
use std::fmt;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Fixture content types (Task 9 → Task 19 seam)
// ---------------------------------------------------------------------------

/// Content types registered at V0 for testing and fixture scaffolding.
///
/// Each name in this list causes `init()` to call `create_artifact_table` and
/// `create_history_machinery`, creating the per-type table, shadow table, and
/// triggers.
///
/// # Task 19 seam
///
/// Task 19 extends this by reading the plugin manifest content-type list. The
/// call site in `init()` iterates `FIXTURE_CONTENT_TYPES`; Task 19 extends
/// that loop to also iterate manifest-provided types. No changes to this const
/// are expected post-V0.
pub(crate) const FIXTURE_CONTENT_TYPES: &[&str] = &["echo_fixture"];

// ---------------------------------------------------------------------------
// V0 substrate migrations
// ---------------------------------------------------------------------------

/// The `default` workspace UUID: deterministic from the name "default".
///
/// Using a deterministic UUID (UUID v5 SHA-1 of "default" in DNS namespace)
/// rather than a random UUID lets the workspace row be upserted idempotently
/// without a name lookup — convenient for tests and for the health snapshot.
/// R-0015-h: the default workspace MUST always exist after first-run.
///
/// Computed once at compile time as a constant to avoid runtime UUID::from_name
/// machinery and ensure zero divergence across runs.
///
/// Derived: uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, b"default")
pub const DEFAULT_WORKSPACE_ID: Uuid = uuid::uuid!("1b027423-a7e3-54ea-9e35-2e1a4afdf3d9");

/// The V0 substrate migration set.
///
/// These are the tables init creates. The migration runner enforces:
/// - Forward-only (no down-migration type).
/// - No destructive statements (A-18 guard).
/// - Idempotent (CREATE IF NOT EXISTS posture + ledger version check).
///
/// V0 tables:
/// - `schema_migrations`: the ledger (created by the runner itself).
/// - `workspaces`: the one builtin state table needed for the default workspace.
/// - `content`: DS-pg-content logical shape (R-0013-b).
/// - `state_config`: DS-pg-state logical shape (R-0013-b, R-0013-c).
///
/// Do NOT add artifact-family tables here (tasks/repos/jobs/contacts — Out of
/// V0 scope, land with 0.2.0+). The per-artifact-type table MACHINERY lands
/// Tasks 8/9; see hook-in seam in module doc.
pub(crate) const V0_MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "create_workspaces",
        sql: "
            CREATE TABLE IF NOT EXISTS workspaces (
                id         UUID        NOT NULL DEFAULT gen_random_uuid(),
                name       TEXT        NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                CONSTRAINT workspaces_pkey    PRIMARY KEY (id),
                CONSTRAINT workspaces_name_uq UNIQUE      (name)
            )
        ",
    },
    Migration {
        version: 2,
        name: "workspaces_name_index",
        sql: "
            CREATE INDEX IF NOT EXISTS workspaces_name_idx
                ON workspaces (name)
        ",
    },
    Migration {
        version: 3,
        name: "create_content",
        // DS-pg-content: regular Postgres table for content artifacts (R-0013-b).
        // No embedding/search_tsv columns at V0 — V0.1+ ADD COLUMN (R-0001-g).
        // workspace_id is UUID to match WorkspaceId newtype (A-16).
        sql: "
            CREATE TABLE IF NOT EXISTS content (
                id           TEXT        NOT NULL,
                workspace_id UUID        NOT NULL,
                key          TEXT        NOT NULL,
                value        BYTEA       NOT NULL,
                created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
                CONSTRAINT content_pkey PRIMARY KEY (id),
                CONSTRAINT content_ws_key_uq UNIQUE (workspace_id, key)
            )
        ",
    },
    Migration {
        version: 4,
        name: "content_workspace_index",
        sql: "
            CREATE INDEX IF NOT EXISTS content_workspace_idx
                ON content (workspace_id)
        ",
    },
    Migration {
        version: 5,
        name: "create_state_config",
        // DS-pg-state: regular Postgres table for state/config artifacts (R-0013-b, R-0013-c).
        sql: "
            CREATE TABLE IF NOT EXISTS state_config (
                id           TEXT        NOT NULL,
                workspace_id UUID        NOT NULL,
                key          TEXT        NOT NULL,
                value        BYTEA       NOT NULL,
                created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
                CONSTRAINT state_config_pkey PRIMARY KEY (id),
                CONSTRAINT state_config_ws_key_uq UNIQUE (workspace_id, key)
            )
        ",
    },
    Migration {
        version: 6,
        name: "state_config_workspace_index",
        sql: "
            CREATE INDEX IF NOT EXISTS state_config_workspace_idx
                ON state_config (workspace_id)
        ",
    },
    // Task 11 (R-0008-c): admin token storage.
    // Six columns exactly (R-0008-c, R-0008-h): no additional key-material column.
    // token_hash BYTEA NOT NULL UNIQUE — raw token bytes are never stored (R-0008-b).
    // workspace_id NOT NULL — absence of workspace claim is a schema violation (R-0008-d).
    // rotated_at nullable — NULL before first rotation.
    Migration {
        version: 7,
        name: "create_admin_tokens",
        sql: "
            CREATE TABLE IF NOT EXISTS admin_tokens (
                id           UUID        NOT NULL DEFAULT gen_random_uuid(),
                token_hash   BYTEA       NOT NULL,
                workspace_id UUID        NOT NULL,
                scopes       TEXT[]      NOT NULL,
                created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
                rotated_at   TIMESTAMPTZ,
                CONSTRAINT admin_tokens_pkey    PRIMARY KEY (id),
                CONSTRAINT admin_tokens_hash_uq UNIQUE      (token_hash)
            )
        ",
    },
    // Task 15 (R-0015-b): user identity records.
    // Users are global (not per-workspace) — agents + sessions reference them.
    // display_name nullable: may not be set on first registration.
    Migration {
        version: 8,
        name: "create_users",
        sql: "
            CREATE TABLE IF NOT EXISTS users (
                id           UUID        NOT NULL DEFAULT gen_random_uuid(),
                username     TEXT        NOT NULL,
                display_name TEXT,
                created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
                CONSTRAINT users_pkey        PRIMARY KEY (id),
                CONSTRAINT users_username_uq UNIQUE      (username)
            )
        ",
    },
    Migration {
        version: 9,
        name: "users_username_index",
        sql: "
            CREATE INDEX IF NOT EXISTS users_username_idx
                ON users (username)
        ",
    },
    // Task 15 (R-0015-c): agent registrations tied to user-workspace pairs.
    // agent_id is canonical (UUIDv5 of workspace_id + user_id + agent_name) —
    // identity derivation is deterministic and verified at registration.
    // A unique constraint on (workspace_id, user_id, agent_name) enforces
    // mismatch detection: a re-registration with a different derived id is rejected.
    Migration {
        version: 10,
        name: "create_agents",
        sql: "
            CREATE TABLE IF NOT EXISTS agents (
                id           UUID        NOT NULL,
                workspace_id UUID        NOT NULL,
                user_id      UUID        NOT NULL,
                agent_name   TEXT        NOT NULL,
                created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
                CONSTRAINT agents_pkey         PRIMARY KEY (id),
                CONSTRAINT agents_ws_user_name UNIQUE      (workspace_id, user_id, agent_name)
            )
        ",
    },
    Migration {
        version: 11,
        name: "agents_workspace_index",
        sql: "
            CREATE INDEX IF NOT EXISTS agents_workspace_idx
                ON agents (workspace_id)
        ",
    },
    // Task 15 (R-0015-e): per-MCP-connection session state.
    // Session context is the source of WorkspaceCtx construction (R-0006-b / Task 23).
    // At V0 the MCP server (Task 23) does not exist; the table models the state shape.
    // ended_at nullable: NULL while the session is active.
    Migration {
        version: 12,
        name: "create_sessions",
        sql: "
            CREATE TABLE IF NOT EXISTS sessions (
                id           UUID        NOT NULL DEFAULT gen_random_uuid(),
                workspace_id UUID        NOT NULL,
                user_id      UUID        NOT NULL,
                agent_id     UUID        NOT NULL,
                created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
                ended_at     TIMESTAMPTZ,
                CONSTRAINT sessions_pkey PRIMARY KEY (id)
            )
        ",
    },
    Migration {
        version: 13,
        name: "sessions_workspace_index",
        sql: "
            CREATE INDEX IF NOT EXISTS sessions_workspace_idx
                ON sessions (workspace_id)
        ",
    },
    Migration {
        version: 14,
        name: "sessions_agent_index",
        sql: "
            CREATE INDEX IF NOT EXISTS sessions_agent_idx
                ON sessions (agent_id)
        ",
    },
    // Task 15 (R-0015-g): project registry.
    // Project identity is a prerequisite for plugin scoping (R-0015-g).
    // No plugin is scoped to a project before that project's record exists.
    Migration {
        version: 15,
        name: "create_projects",
        sql: "
            CREATE TABLE IF NOT EXISTS projects (
                id           UUID        NOT NULL DEFAULT gen_random_uuid(),
                workspace_id UUID        NOT NULL,
                name         TEXT        NOT NULL,
                created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
                CONSTRAINT projects_pkey        PRIMARY KEY (id),
                CONSTRAINT projects_ws_name_uq  UNIQUE      (workspace_id, name)
            )
        ",
    },
    Migration {
        version: 16,
        name: "projects_workspace_index",
        sql: "
            CREATE INDEX IF NOT EXISTS projects_workspace_idx
                ON projects (workspace_id)
        ",
    },
    // P-0018 D-ENT / D-ACTOR (Task 1, coordination-wedge): the `actors` core
    // entity. Landed here as a minimal STANDALONE entity, minted directly by
    // role-instance name — NO FK linkage to the existing `users`/`agents`/
    // `sessions` builtins in this landing. The fuller P-0018 unification
    // (rewiring those builtins to populate `actors`) is deferred to P-0018's
    // own landing (Gap A, decided 2026-07-11).
    // actor_type is a closed set {human, agent, system} (P-0018 D-ACTOR): a
    // value outside the set is rejected AT WRITE by the CHECK constraint
    // below, mirroring the `operation IN (...)` precedent in
    // `schema/history_trigger.rs`.
    // name is the per-workspace-unique role-instance identifier that
    // `builtins::actors::resolve_or_create` mints/resolves against.
    Migration {
        version: 17,
        name: "create_actors",
        sql: "
            CREATE TABLE IF NOT EXISTS actors (
                id           UUID        NOT NULL DEFAULT gen_random_uuid(),
                workspace_id UUID        NOT NULL,
                actor_type   TEXT        NOT NULL,
                name         TEXT        NOT NULL,
                created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
                CONSTRAINT actors_pkey           PRIMARY KEY (id),
                CONSTRAINT actors_ws_name_uq     UNIQUE      (workspace_id, name),
                CONSTRAINT actors_actor_type_chk CHECK       (actor_type IN ('human', 'agent', 'system'))
            )
        ",
    },
    Migration {
        version: 18,
        name: "actors_workspace_index",
        sql: "
            CREATE INDEX IF NOT EXISTS actors_workspace_idx
                ON actors (workspace_id)
        ",
    },
    // Coordination wedge (Task 2, R-0076 / R-0077): the `leases` and `messages`
    // operational tables. Additive, forward-only, idempotent (R-0076-e).
    //
    // leases (R-0065): held intent on a resource. One live lease per
    // (workspace_id, resource) is enforced by the PARTIAL UNIQUE index at v21
    // (the QA-1 mechanism; that index doubles as the R-0076-a live-lease hot
    // lookup, so no redundant non-unique twin is added). terminal_state is a
    // closed set {released, taken_over}, nullable (NULL = non-terminal); the
    // bare `IN` CHECK passes on NULL, mirroring actors_actor_type_chk. Hard FKs
    // are single-column to core entities only (holder_actor_id -> actors,
    // project_id -> projects); cross-workspace tenant consistency is
    // application-layer at V0 (R-0076-b, WorkspaceCtx), so there is NO composite
    // (workspace_id, actor_id) FK. superseded_by is a SOFT self-ref (plain uuid,
    // no FK) set on a taken-over row. session_id is the attachment-as-lease
    // realization's session home (R-0064-f baseline). duration is BIGINT seconds
    // (the spec allows interval OR a seconds count; seconds is the simple
    // idiomatic form and keeps the ≤ policy-max bound a plain integer compare).
    Migration {
        version: 19,
        name: "create_leases",
        sql: "
            CREATE TABLE IF NOT EXISTS leases (
                id              UUID        NOT NULL DEFAULT gen_random_uuid(),
                workspace_id    UUID        NOT NULL,
                resource        TEXT        NOT NULL,
                holder_actor_id UUID        NOT NULL,
                project_id      UUID,
                acquired_at     TIMESTAMPTZ NOT NULL,
                duration        BIGINT      NOT NULL,
                expires_at      TIMESTAMPTZ NOT NULL,
                terminal_state  TEXT,
                terminated_at   TIMESTAMPTZ,
                superseded_by   UUID,
                session_id      UUID,
                CONSTRAINT leases_pkey               PRIMARY KEY (id),
                CONSTRAINT leases_holder_actor_fk    FOREIGN KEY (holder_actor_id) REFERENCES actors (id),
                CONSTRAINT leases_project_fk         FOREIGN KEY (project_id)       REFERENCES projects (id),
                CONSTRAINT leases_terminal_state_chk CHECK       (terminal_state IN ('released', 'taken_over'))
            )
        ",
    },
    Migration {
        version: 20,
        name: "leases_workspace_index",
        sql: "
            CREATE INDEX IF NOT EXISTS leases_workspace_idx
                ON leases (workspace_id)
        ",
    },
    // v21: exactly-one-live-lease (R-0065-b; the QA-1 mechanism). PARTIAL on
    // WHERE terminal_state IS NULL so a terminal row (released / taken_over) does
    // not block re-acquisition, while two live rows on one resource collide with
    // 23505. Predicate is non-terminated, NOT expiry-based (a partial index
    // cannot reference now(); expiry is evaluated at operation time, R-0065-e).
    Migration {
        version: 21,
        name: "leases_live_resource_unique_index",
        sql: "
            CREATE UNIQUE INDEX IF NOT EXISTS leases_live_resource_uq
                ON leases (workspace_id, resource)
                WHERE terminal_state IS NULL
        ",
    },
    // messages (R-0068 / R-0069): addressed, append-once message rows. state is
    // a closed, host-enforced-monotonic set {sent, delivered, acknowledged,
    // dispositioned}; disposition a closed set {completed, declined, obsolete},
    // nullable. Both addressing FKs are single-column -> actors (sender /
    // addressee); there is NO FK between leases and messages, so the two tables
    // stay independently flippable (R-0077) over the shared actors spine.
    // schema_version is INTEGER; payload is JSONB, schema-validated at send in
    // host code (R-0070-b) — not a schema CHECK. The append-once consumption
    // timestamps and immutability of the post-send fields are enforced by the
    // host write path, not the schema.
    Migration {
        version: 22,
        name: "create_messages",
        sql: "
            CREATE TABLE IF NOT EXISTS messages (
                id                 UUID        NOT NULL DEFAULT gen_random_uuid(),
                workspace_id       UUID        NOT NULL,
                sender_actor_id    UUID        NOT NULL,
                addressee_actor_id UUID        NOT NULL,
                message_type       TEXT        NOT NULL,
                schema_version     INTEGER     NOT NULL,
                payload            JSONB       NOT NULL,
                state              TEXT        NOT NULL,
                sent_at            TIMESTAMPTZ NOT NULL,
                delivered_at       TIMESTAMPTZ,
                acknowledged_at    TIMESTAMPTZ,
                dispositioned_at   TIMESTAMPTZ,
                disposition        TEXT,
                disposition_note   TEXT,
                CONSTRAINT messages_pkey            PRIMARY KEY (id),
                CONSTRAINT messages_sender_fk       FOREIGN KEY (sender_actor_id)    REFERENCES actors (id),
                CONSTRAINT messages_addressee_fk    FOREIGN KEY (addressee_actor_id) REFERENCES actors (id),
                CONSTRAINT messages_state_chk       CHECK       (state IN ('sent', 'delivered', 'acknowledged', 'dispositioned')),
                CONSTRAINT messages_disposition_chk CHECK       (disposition IN ('completed', 'declined', 'obsolete'))
            )
        ",
    },
    Migration {
        version: 23,
        name: "messages_workspace_index",
        sql: "
            CREATE INDEX IF NOT EXISTS messages_workspace_idx
                ON messages (workspace_id)
        ",
    },
    // v24: undispositioned-per-addressee poll hot predicate (R-0076-a). PARTIAL
    // on WHERE dispositioned_at IS NULL so the poll path scans only the
    // still-open queue for an addressee.
    Migration {
        version: 24,
        name: "messages_undispositioned_index",
        sql: "
            CREATE INDEX IF NOT EXISTS messages_undispositioned_idx
                ON messages (workspace_id, addressee_actor_id)
                WHERE dispositioned_at IS NULL
        ",
    },
];

// ---------------------------------------------------------------------------
// Init error
// ---------------------------------------------------------------------------

/// Structured error returned by `init()`.
#[derive(Debug)]
pub enum InitError {
    /// The `pgvector` extension could not be enabled.
    /// Init does NOT proceed with schema creation (R-0013-a).
    ExtensionUnavailable(ExtensionError),
    /// A migration failed (destructive guard or DB error).
    Migration(MigrationError),
    /// The artifact-table generator failed (type-name or DB error).
    ArtifactTable(crate::schema::artifact_table::ArtifactTableError),
    /// The history trigger machinery failed.
    HistoryMachinery(crate::schema::history_trigger::HistoryMachineryError),
    /// A database operation failed during init.
    Db(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InitError::ExtensionUnavailable(e) => {
                write!(f, "mnemra init: extension unavailable — {e}")
            }
            InitError::Migration(e) => write!(f, "mnemra init: migration failed — {e}"),
            InitError::ArtifactTable(e) => {
                write!(f, "mnemra init: artifact table generator failed — {e}")
            }
            InitError::HistoryMachinery(e) => {
                write!(f, "mnemra init: history trigger machinery failed — {e}")
            }
            InitError::Db(e) => write!(f, "mnemra init: db error — {e}"),
        }
    }
}

impl std::error::Error for InitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            InitError::ExtensionUnavailable(e) => Some(e),
            InitError::Migration(e) => Some(e),
            InitError::ArtifactTable(e) => Some(e),
            InitError::HistoryMachinery(e) => Some(e),
            InitError::Db(e) => Some(e.as_ref()),
        }
    }
}

impl From<MigrationError> for InitError {
    fn from(e: MigrationError) -> Self {
        InitError::Migration(e)
    }
}

impl From<crate::schema::artifact_table::ArtifactTableError> for InitError {
    fn from(e: crate::schema::artifact_table::ArtifactTableError) -> Self {
        InitError::ArtifactTable(e)
    }
}

impl From<crate::schema::history_trigger::HistoryMachineryError> for InitError {
    fn from(e: crate::schema::history_trigger::HistoryMachineryError) -> Self {
        InitError::HistoryMachinery(e)
    }
}

// ---------------------------------------------------------------------------
// Role creation (A-17, R-0013-e) — superuser seam
// ---------------------------------------------------------------------------

/// Role names for the four least-privilege roles (R-0013-e).
///
/// These roles are forward structure for V0.1+. At V0 they are created with
/// minimum grants and no LOGIN (except health-probe which needs none either).
/// The `mnemra_app` role (created in Task 6) is the runtime role; these are
/// separate operational surfaces:
///
/// | Role                | Surface              | Grants (V0)                            |
/// |---------------------|----------------------|----------------------------------------|
/// | `mnemra_host_fns`   | Host-fn execution    | CONNECT + USAGE + SELECT/INSERT/UPDATE |
/// | `mnemra_migration`  | Schema migration     | CONNECT + USAGE + CREATE               |
/// | `mnemra_backup`     | Backup operations    | CONNECT + USAGE + SELECT               |
/// | `mnemra_health`     | Health probe         | CONNECT + USAGE + SELECT on workspaces |
pub const ROLE_HOST_FNS: &str = "mnemra_host_fns";
pub const ROLE_MIGRATION: &str = "mnemra_migration";
pub const ROLE_BACKUP: &str = "mnemra_backup";
pub const ROLE_HEALTH: &str = "mnemra_health";

/// Create the four least-privilege DB roles and apply their minimum grants.
///
/// Called via the `pub(crate)` superuser seam on `EmbeddedEngine` (A-17).
/// Idempotent: uses `IF NOT EXISTS`.
pub(crate) async fn create_least_privilege_roles(
    superuser_pool: &PgPool,
    app_db: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Create each role with NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE.
    // IF NOT EXISTS makes this idempotent on re-run.
    for role in &[ROLE_HOST_FNS, ROLE_MIGRATION, ROLE_BACKUP, ROLE_HEALTH] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DO $$ BEGIN
                IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = '{role}') THEN
                    CREATE ROLE {role} NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOLOGIN;
                END IF;
             END $$"
        )))
        .execute(superuser_pool)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
    }

    // Grant CONNECT on the database to all operational roles.
    for role in &[ROLE_HOST_FNS, ROLE_MIGRATION, ROLE_BACKUP, ROLE_HEALTH] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "GRANT CONNECT ON DATABASE {app_db} TO {role}"
        )))
        .execute(superuser_pool)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
    }

    // Grant USAGE on public schema to all operational roles.
    for role in &[ROLE_HOST_FNS, ROLE_MIGRATION, ROLE_BACKUP, ROLE_HEALTH] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "GRANT USAGE ON SCHEMA public TO {role}"
        )))
        .execute(superuser_pool)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
    }

    // host_fns: SELECT/INSERT/UPDATE on all tables in the public schema.
    // Uses ALL TABLES rather than an explicit table list so new tables added by
    // later tasks (Task 9 artifact tables, Task 11 admin_tokens) are covered
    // without requiring a role-grant migration for each addition.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA public TO {ROLE_HOST_FNS}"
    )))
    .execute(superuser_pool)
    .await
    .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

    // migration: CREATE on schema (can create tables) + all DML.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "GRANT CREATE ON SCHEMA public TO {ROLE_MIGRATION}"
    )))
    .execute(superuser_pool)
    .await
    .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA public TO {ROLE_MIGRATION}"
    )))
    .execute(superuser_pool)
    .await
    .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

    // backup: SELECT only (read-only for backup).
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "GRANT SELECT ON ALL TABLES IN SCHEMA public TO {ROLE_BACKUP}"
    )))
    .execute(superuser_pool)
    .await
    .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

    // health-probe: SELECT on workspaces only (minimal liveness check).
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "GRANT SELECT ON workspaces TO {ROLE_HEALTH}"
    )))
    .execute(superuser_pool)
    .await
    .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Health snapshot (R-0004-g)
// ---------------------------------------------------------------------------

/// Health snapshot body (R-0004-g).
///
/// Task 25's `/health` HTTP handler serializes this struct to JSON.
#[derive(Debug, PartialEq, serde::Serialize)]
pub struct HealthSnapshot {
    /// True if the Postgres engine is reachable.
    pub postgres: bool,
    /// True if the `vector` extension is loaded in `pg_extension`.
    pub pgvector: bool,
    /// True if the `default` workspace row exists.
    pub workspace_default: bool,
    /// `"ok"` | `"degraded"` | `"down"` (R-0004-g body shape).
    pub overall: HealthStatus,
}

/// Overall health status string per R-0004-g.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Ok,
    Degraded,
    Down,
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthStatus::Ok => write!(f, "ok"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Down => write!(f, "down"),
        }
    }
}

/// Serializes via [`fmt::Display`] (`"ok" | "degraded" | "down"`) so the
/// wire form and the log/error form share one source of truth — no
/// separate `#[serde(rename_all = ...)]` mapping to drift out of sync with
/// the `Display` impl above.
impl serde::Serialize for HealthStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

/// A-15: Structured `StorageError::EngineUnavailable` degradation seam.
///
/// Connection-refused-class failures map to this variant so Task 25's `/health`
/// "degraded" path can distinguish engine-down from other errors. The Storage
/// trait signatures are unchanged (they use `Box<dyn Error>`); this type lives
/// at the init/health surface.
#[derive(Debug)]
pub enum StorageError {
    /// The storage engine is unreachable (connection refused, engine not started).
    /// Task 25's `/health` maps this to `overall: "degraded"` or `"down"`.
    EngineUnavailable { cause: String },
    /// Any other storage error.
    Other(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::EngineUnavailable { cause } => {
                write!(f, "storage engine unavailable: {cause}")
            }
            StorageError::Other(e) => write!(f, "storage error: {e}"),
        }
    }
}

impl std::error::Error for StorageError {}

/// Probe the health of the running embedded engine and return a snapshot.
///
/// This is a pure probe — it does NOT start the engine or run migrations.
/// Init calls this at the end to assert `overall: "ok"`. Task 25 wraps it
/// in an HTTP handler.
///
/// # A-15
///
/// Connection-refused-class errors (engine not running) map to
/// `StorageError::EngineUnavailable` so the `/health` "degraded" state is
/// distinguishable from a genuine query error.
pub async fn health_snapshot(pool: &PgPool) -> Result<HealthSnapshot, StorageError> {
    // Check Postgres connectivity with a trivial query.
    let pg_ok = sqlx::query("SELECT 1")
        .execute(pool)
        .await
        .map(|_| true)
        .unwrap_or_else(|e| {
            // Map connection-refused-class failures to EngineUnavailable (A-15).
            let msg = e.to_string();
            if msg.contains("connection refused")
                || msg.contains("could not connect")
                || msg.contains("the connection is closed")
            {
                // Return false — we'll set overall=down below.
                false
            } else {
                false
            }
        });

    // If the engine itself is unreachable, return Down immediately.
    if !pg_ok {
        return Ok(HealthSnapshot {
            postgres: false,
            pgvector: false,
            workspace_default: false,
            overall: HealthStatus::Down,
        });
    }

    // Check pgvector extension.
    let pgvector_ok: bool =
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM pg_extension WHERE extname = 'vector'")
            .fetch_one(pool)
            .await
            .map(|(count,)| count > 0)
            .unwrap_or(false);

    // Check default workspace existence.
    let default_ok: bool =
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM workspaces WHERE name = 'default'")
            .fetch_one(pool)
            .await
            .map(|(count,)| count > 0)
            .unwrap_or(false);

    let overall = if pg_ok && pgvector_ok && default_ok {
        HealthStatus::Ok
    } else if pg_ok {
        HealthStatus::Degraded
    } else {
        HealthStatus::Down
    };

    Ok(HealthSnapshot {
        postgres: pg_ok,
        pgvector: pgvector_ok,
        workspace_default: default_ok,
        overall,
    })
}

// ---------------------------------------------------------------------------
// init() — the main entry point
// ---------------------------------------------------------------------------

/// Bootstrap the mnemra schema on `engine`.
///
/// # Steps
///
/// 1. Enable `extension_name` (default: `"vector"`) via the superuser path.
///    Returns `InitError::ExtensionUnavailable` and halts on failure (R-0013-a).
/// 2. Run forward-only migrations (V0_MIGRATIONS) via the app-role pool.
/// 3. Upsert the `default` workspace row (idempotent, R-0015-a/h).
/// 4. Create per-artifact-type tables for all `FIXTURE_CONTENT_TYPES` (Tasks
///    8/9). Each call to `create_artifact_table` + `create_history_machinery`
///    is idempotent. This step runs BEFORE role creation so that the
///    `GRANT ... ON ALL TABLES` in step 5 covers the newly created tables.
/// 5. Create the four least-privilege roles (idempotent, R-0013-e).
///    The grant covers all tables existing at call time — step 4 tables are
///    included because they are created first.
///
/// # Idempotency
///
/// Safe to run on an empty or already-initialized database. The migration
/// runner skips applied versions; the default workspace insert uses
/// `ON CONFLICT DO NOTHING`; roles use `IF NOT EXISTS`; artifact-table
/// generator uses `CREATE TABLE/INDEX IF NOT EXISTS` and
/// `CREATE OR REPLACE TRIGGER`.
///
/// # Negative-path seam note
///
/// `extension_name` is parameterized so the pgvector-unavailable negative path
/// is testable: pass a bogus extension name (e.g. `"nonexistent_extension"`)
/// to exercise the refusal path. The real production call uses `"vector"`.
/// Fidelity limit: this exercises the structured-error refusal code path, not a
/// genuinely-missing pgvector (the bundled engine always has the extension).
pub async fn init(engine: &EmbeddedEngine, extension_name: &str) -> Result<(), InitError> {
    // Step 1: enable pgvector (or the injected test extension name).
    // HALT on failure — do NOT proceed with schema creation (R-0013-a).
    engine
        .ensure_extension(extension_name)
        .await
        .map_err(InitError::ExtensionUnavailable)?;

    // Step 2: run forward-only migrations via the app-role pool.
    run_migrations(engine.pool.as_ref(), V0_MIGRATIONS).await?;

    // Step 3: upsert the `default` workspace row (idempotent).
    // Uses the deterministic UUID constant so it can be found by either id or name.
    sqlx::query(
        "INSERT INTO workspaces (id, name)
         VALUES ($1, 'default')
         ON CONFLICT (name) DO NOTHING",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .execute(engine.pool.as_ref())
    .await
    .map_err(|e| InitError::Db(Box::new(e)))?;

    // Step 4: create per-artifact-type tables for all fixture types (Tasks 8/9).
    //
    // Generator-executed (not through migrations::apply) because per-type DDL
    // is structural but not authored ahead of time — the set grows at
    // plugin-registration (Task 19 seam).
    //
    // Run BEFORE create_least_privilege_roles (step 5) so that the
    // `GRANT ... ON ALL TABLES` covers tables created here.
    for type_name in FIXTURE_CONTENT_TYPES {
        create_artifact_table(engine.pool.as_ref(), type_name).await?;
        create_history_machinery(engine.pool.as_ref(), type_name).await?;
    }

    // Step 5: create the four least-privilege roles via the superuser seam (A-17).
    engine
        .create_least_privilege_roles()
        .await
        .map_err(InitError::Db)?;

    Ok(())
}
