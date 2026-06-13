//! Postgres `Storage` adapter.
//!
//! Implements [`Storage`] using the embedded Postgres engine from
//! [`engine::EmbeddedEngine`].  The adapter satisfies the same two invariants
//! as the in-memory adapter:
//!
//! 1. **Atomic multi-write** — writes are buffered in memory; `commit()`
//!    opens one Postgres transaction, applies all buffered writes, and commits
//!    atomically.  Drop-without-commit discards the buffer; nothing is sent to
//!    the database.
//!
//! 2. **Workspace-scoped isolation** — every query is parameterized on
//!    `workspace_id` (a `i64` column).  Cross-workspace reads return zero rows
//!    because the `WHERE workspace_id = $n` predicate is always applied.
//!
//! # Buffer-first design
//!
//! Holding a live sqlx `Transaction` across multiple `async-trait` `&mut self`
//! methods fights the borrow checker and makes rollback rely on async drop
//! (which is unreliable).  Mirroring the in-memory adapter — stage in memory,
//! flush in one DB transaction at `commit()` — avoids both problems cleanly.
//!
//! # Schema
//!
//! Task 6 creates a single table (A-16: workspace_id widened to UUID at Task 7):
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS records (
//!     workspace_id UUID    NOT NULL,
//!     key          TEXT    NOT NULL,
//!     value        BYTEA   NOT NULL,
//!     PRIMARY KEY (workspace_id, key)
//! );
//! ```
//!
//! Real artifact columns (`embedding`, `search_tsv`, etc.) land in Tasks 7–9.
//! The application role (`mnemra_app`) owns no BYPASSRLS / superuser privilege,
//! satisfying P-0010's V0.1+ RLS preconditions.

pub mod engine;

use super::{Record, Storage, Transaction, WorkspaceId};
use async_trait::async_trait;
use engine::EmbeddedEngine;
use sqlx::PgPool;
use std::{collections::HashMap, error::Error, sync::Arc};

// ---------------------------------------------------------------------------
// PostgresStorage
// ---------------------------------------------------------------------------

/// Postgres-backed storage backend using the embedded engine.
///
/// The `pool` is authenticated as the `mnemra_app` ordinary role.
/// The engine is wrapped in an `Arc` so `PostgresStorage` is cheaply
/// `Clone` — sharing the engine and pool across the clone boundary.
/// The embedded Postgres server stays alive as long as any clone exists.
#[derive(Clone)]
pub struct PostgresStorage {
    /// Embedded engine wrapped in Arc — kept alive and shared across clones.
    _engine: Arc<EmbeddedEngine>,
    pool: Arc<PgPool>,
}

impl PostgresStorage {
    /// Return a reference to the underlying connection pool.
    ///
    /// Primarily used by tests that need to run raw SQL against the
    /// application-role connection (e.g. the role-shape AC assertion).
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Start the embedded Postgres engine and return a ready `PostgresStorage`.
    ///
    /// First invocation downloads Postgres binaries (~cold, several seconds).
    /// Subsequent runs reuse the cached installation (~warm, sub-second startup).
    ///
    /// # Errors
    ///
    /// Propagates engine bring-up, extension-install, or connection errors.
    pub async fn start_embedded() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let engine = EmbeddedEngine::start().await?;
        let pool = Arc::clone(&engine.pool);

        // Bootstrap the records table (idempotent).
        // A-16: workspace_id widened from BIGINT to UUID.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS records (
                workspace_id UUID    NOT NULL,
                key          TEXT    NOT NULL,
                value        BYTEA   NOT NULL,
                PRIMARY KEY (workspace_id, key)
            )
            "#,
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        Ok(PostgresStorage {
            _engine: Arc::new(engine),
            pool,
        })
    }
}

#[async_trait]
impl Storage for PostgresStorage {
    async fn begin(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Box<dyn Transaction>, Box<dyn Error + Send + Sync>> {
        Ok(Box::new(PostgresTransaction {
            workspace,
            pool: Arc::clone(&self.pool),
            staged: HashMap::new(),
        }))
    }
}

// ---------------------------------------------------------------------------
// PostgresTransaction
// ---------------------------------------------------------------------------

/// An in-flight Postgres transaction.
///
/// Writes are buffered in `staged` until `commit()`.  Dropping without commit
/// discards the buffer; no SQL is sent to the database.
pub struct PostgresTransaction {
    workspace: WorkspaceId,
    pool: Arc<PgPool>,
    /// Writes buffered in this transaction, not yet persisted.
    staged: HashMap<String, Record>,
}

#[async_trait]
impl Transaction for PostgresTransaction {
    async fn put(&mut self, record: Record) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.staged.insert(record.key.clone(), record);
        Ok(())
    }

    async fn get(&mut self, key: &str) -> Result<Option<Record>, Box<dyn Error + Send + Sync>> {
        // Read-your-own-writes: staged buffer takes priority.
        if let Some(r) = self.staged.get(key) {
            return Ok(Some(r.clone()));
        }

        let ws_id = self.workspace.0; // A-16: UUID, no cast needed
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT value FROM records WHERE workspace_id = $1 AND key = $2")
                .bind(ws_id)
                .bind(key)
                .fetch_optional(self.pool.as_ref())
                .await
                .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        Ok(row.map(|(value,)| Record {
            key: key.to_string(),
            value,
        }))
    }

    async fn list(&mut self) -> Result<Vec<Record>, Box<dyn Error + Send + Sync>> {
        let ws_id = self.workspace.0; // A-16: UUID, no cast needed

        // Fetch committed rows for this workspace.
        let rows: Vec<(String, Vec<u8>)> =
            sqlx::query_as("SELECT key, value FROM records WHERE workspace_id = $1")
                .bind(ws_id)
                .fetch_all(self.pool.as_ref())
                .await
                .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Merge committed + staged (staged wins on key collision).
        let mut merged: HashMap<String, Record> = rows
            .into_iter()
            .map(|(key, value)| (key.clone(), Record { key, value }))
            .collect();

        for (k, v) in &self.staged {
            merged.insert(k.clone(), v.clone());
        }

        Ok(merged.into_values().collect())
    }

    async fn commit(self: Box<Self>) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.staged.is_empty() {
            return Ok(());
        }

        let ws_id = self.workspace.0; // A-16: UUID, no cast needed

        // Open one real Postgres transaction, flush all staged writes, commit.
        let mut txn = self
            .pool
            .begin()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        for (_, record) in self.staged {
            sqlx::query(
                r#"
                INSERT INTO records (workspace_id, key, value)
                VALUES ($1, $2, $3)
                ON CONFLICT (workspace_id, key) DO UPDATE SET value = EXCLUDED.value
                "#,
            )
            .bind(ws_id)
            .bind(&record.key)
            .bind(&record.value)
            .execute(&mut *txn)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        }

        txn.commit()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        Ok(())
    }
}
