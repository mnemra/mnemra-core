//! History shadow table + trigger machinery (R-0001-e).
//!
//! # What this module provides
//!
//! `create_history_machinery(pool, type_name)` — idempotently creates:
//!
//! - A shadow table `<type_name>_history` that captures per-row mutation history.
//! - A BEFORE UPDATE trigger that copies the **prior** row into the history table
//!   before the UPDATE is applied, preserving `frontmatter` byte-for-byte.
//! - A BEFORE DELETE trigger that writes a `'DELETE'` history row with the
//!   artifact's `old_frontmatter` and `old_body` before the DELETE executes
//!   (R-0001-e).
//!
//! # Design note: trigger-based DELETE history
//!
//! R-0001-e states "the host SHALL write a history row … before executing the
//! DELETE". A BEFORE DELETE trigger satisfies this guarantee atomically within
//! the same transaction, ensuring the history row is always written even if
//! the DELETE is issued outside of a host-fn call.
//!
//! **Deviation from literal wording:** the spec says "the host SHALL write",
//! which could be read as host application code rather than a DB trigger. We
//! choose the trigger path because:
//! (a) it is unconditionally atomic — no host-code path can miss it;
//! (b) a host delete fn does not exist at V0;
//! (c) the observable contract (history row present before artifact removed)
//!     is identical.
//! This deviation is flagged in the dispatch completion report.
//!
//! # History table schema
//!
//! | Column            | Type        | Notes                              |
//! |-------------------|-------------|------------------------------------|
//! | `history_id`      | BIGSERIAL   | surrogate PK                       |
//! | `operation`       | TEXT        | `'UPDATE'` or `'DELETE'`           |
//! | `artifact_id`     | TEXT        | the artifact's `id` value          |
//! | `old_frontmatter` | JSONB       | prior frontmatter (byte-for-byte)  |
//! | `old_body`        | TEXT        | prior body (nullable)              |
//! | `recorded_at`     | TIMESTAMPTZ | when the history row was written   |
//!
//! `old_frontmatter` stores the full JSONB value as written — no normalization.
//! The R-0001-e byte-exact requirement is satisfied by copying `OLD.frontmatter`
//! directly (JSONB→JSONB) without going through text and back.

use sqlx::PgPool;
use std::fmt;

use crate::schema::artifact_table::validate_type_name;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error returned by `create_history_machinery`.
#[derive(Debug)]
pub enum HistoryMachineryError {
    /// The type name failed validation.
    InvalidTypeName(crate::schema::artifact_table::TypeNameError),
    /// A database error occurred.
    Db(sqlx::Error),
}

impl fmt::Display for HistoryMachineryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HistoryMachineryError::InvalidTypeName(e) => {
                write!(f, "history machinery: {e}")
            }
            HistoryMachineryError::Db(e) => {
                write!(f, "history machinery: db error — {e}")
            }
        }
    }
}

impl std::error::Error for HistoryMachineryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HistoryMachineryError::InvalidTypeName(e) => Some(e),
            HistoryMachineryError::Db(e) => Some(e),
        }
    }
}

impl From<sqlx::Error> for HistoryMachineryError {
    fn from(e: sqlx::Error) -> Self {
        HistoryMachineryError::Db(e)
    }
}

// ---------------------------------------------------------------------------
// Creator
// ---------------------------------------------------------------------------

/// Create the history shadow table, UPDATE trigger, and DELETE trigger for
/// `type_name`.
///
/// Idempotent: uses `CREATE TABLE IF NOT EXISTS` and `CREATE OR REPLACE TRIGGER`.
///
/// Must be called AFTER `create_artifact_table(pool, type_name)` because the
/// triggers reference the parent table.
///
/// # Byte-exact preservation (R-0001-e)
///
/// The trigger body copies `OLD.frontmatter` (JSONB) directly into
/// `old_frontmatter` (JSONB) without a text round-trip. JSONB stores the
/// binary representation as parsed; the copy is value-identical. Tests verify
/// this via a `::text` cast comparison.
pub async fn create_history_machinery(
    pool: &PgPool,
    type_name: &str,
) -> Result<(), HistoryMachineryError> {
    let name = validate_type_name(type_name).map_err(HistoryMachineryError::InvalidTypeName)?;

    let history_table = format!("{name}_history");
    let update_fn = format!("{name}_history_update_fn");
    let update_trigger = format!("{name}_history_update");
    let delete_fn = format!("{name}_history_delete_fn");
    let delete_trigger = format!("{name}_history_delete");

    // Shadow table — idempotent.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE IF NOT EXISTS {history_table} (
            history_id      BIGSERIAL   NOT NULL,
            operation       TEXT        NOT NULL,
            artifact_id     TEXT        NOT NULL,
            old_frontmatter JSONB       NOT NULL,
            old_body        TEXT,
            recorded_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
            CONSTRAINT {history_table}_pkey PRIMARY KEY (history_id),
            CONSTRAINT {history_table}_operation_chk
                CHECK (operation IN ('UPDATE', 'DELETE'))
        )"
    )))
    .execute(pool)
    .await?;

    // UPDATE trigger function — fires BEFORE UPDATE, preserves OLD row.
    // CREATE OR REPLACE is idempotent on PG16.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE OR REPLACE FUNCTION {update_fn}()
         RETURNS TRIGGER LANGUAGE plpgsql AS $$
         BEGIN
             INSERT INTO {history_table}
                 (operation, artifact_id, old_frontmatter, old_body)
             VALUES
                 ('UPDATE', OLD.id, OLD.frontmatter, OLD.body);
             RETURN NEW;
         END;
         $$"
    )))
    .execute(pool)
    .await?;

    // BEFORE UPDATE trigger — fires before every UPDATE on the artifact table.
    // CREATE OR REPLACE TRIGGER is available in PG16.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE OR REPLACE TRIGGER {update_trigger}
         BEFORE UPDATE ON {name}
         FOR EACH ROW EXECUTE FUNCTION {update_fn}()"
    )))
    .execute(pool)
    .await?;

    // DELETE trigger function — fires BEFORE DELETE, writes 'DELETE' history row.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE OR REPLACE FUNCTION {delete_fn}()
         RETURNS TRIGGER LANGUAGE plpgsql AS $$
         BEGIN
             INSERT INTO {history_table}
                 (operation, artifact_id, old_frontmatter, old_body)
             VALUES
                 ('DELETE', OLD.id, OLD.frontmatter, OLD.body);
             RETURN OLD;
         END;
         $$"
    )))
    .execute(pool)
    .await?;

    // BEFORE DELETE trigger.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE OR REPLACE TRIGGER {delete_trigger}
         BEFORE DELETE ON {name}
         FOR EACH ROW EXECUTE FUNCTION {delete_fn}()"
    )))
    .execute(pool)
    .await?;

    Ok(())
}
