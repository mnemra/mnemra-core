//! Forward-only schema migration runner.
//!
//! # A-18: Structural refusal of destructive operations (R-0013-d)
//!
//! V0 migrations are forward-only. The runner STRUCTURALLY REFUSES destructive
//! SQL statements: no down-migration type exists, and a destructive-statement
//! guard (DROP TABLE/COLUMN, TRUNCATE, destructive ALTER) returns a structured
//! error citing R-0013-d before executing anything.
//!
//! This makes the guarded case unreachable at V0. A real backup mechanism fires
//! on the first genuine destructive-migration need — that is the named tripwire:
//! when a migration requires a destructive statement, implement the backup
//! mechanism and loosen this guard rather than bypassing it.
//!
//! # Ledger
//!
//! A `schema_migrations` table records applied versions (integer sequence).
//! `apply()` is idempotent: already-applied versions are skipped. The runner
//! never executes the same version twice, satisfying the empty-and-populated-DB
//! requirement from R-0013-d and the AC idempotency requirement.

use sqlx::PgPool;
use std::fmt;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Error returned when a migration SQL contains a destructive statement.
///
/// R-0013-d: no destructive schema migration SHALL run without a verified
/// pre-migration backup. At V0 the guard makes this case unreachable.
///
/// **Tripwire:** when a migration genuinely needs a destructive statement,
/// implement the backup verification mechanism first, then extend this runner
/// to allow destructive statements under a verified-backup guard. Do not
/// bypass this error by pre-filtering the migration list.
#[derive(Debug)]
pub struct DestructiveMigrationError {
    pub version: u32,
    pub statement: String,
    pub reason: &'static str,
}

impl fmt::Display for DestructiveMigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "migration v{} contains a destructive statement '{}' — \
             R-0013-d requires a verified pre-migration backup before \
             destructive schema changes; implement backup verification first ({})",
            self.version, self.statement, self.reason
        )
    }
}

impl std::error::Error for DestructiveMigrationError {}

/// Error returned by the migration runner.
#[derive(Debug)]
pub enum MigrationError {
    /// A migration SQL was rejected by the destructive-statement guard.
    Destructive(DestructiveMigrationError),
    /// A database operation failed.
    Db(sqlx::Error),
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationError::Destructive(e) => e.fmt(f),
            MigrationError::Db(e) => write!(f, "migration db error: {e}"),
        }
    }
}

impl std::error::Error for MigrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MigrationError::Destructive(e) => Some(e),
            MigrationError::Db(e) => Some(e),
        }
    }
}

impl From<sqlx::Error> for MigrationError {
    fn from(e: sqlx::Error) -> Self {
        MigrationError::Db(e)
    }
}

// ---------------------------------------------------------------------------
// Destructive-statement guard (A-18)
// ---------------------------------------------------------------------------

/// Patterns for SQL tokens that indicate a destructive operation.
///
/// Checked case-insensitively against the migration SQL body. The guard is
/// conservative: if any token matches, the migration is refused.
///
/// NOTE: These are prefix matches on upper-cased token substrings. The guard
/// is not a full SQL parser — it catches the obvious destructive forms. A
/// migration author who needs a destructive operation must implement backup
/// verification first (see module-level doc).
const DESTRUCTIVE_PATTERNS: &[&str] = &[
    "DROP TABLE",
    "DROP COLUMN",
    "DROP INDEX",
    "DROP SCHEMA",
    "TRUNCATE",
    "ALTER TABLE", // ALTER TABLE includes DROP COLUMN, RENAME, TYPE changes
    "DELETE FROM",
];

/// Check `sql` for destructive statements. Returns the first offending pattern
/// if any is found.
fn check_destructive(sql: &str) -> Option<&'static str> {
    let upper = sql.to_uppercase();
    DESTRUCTIVE_PATTERNS
        .iter()
        .find(|&&pattern| upper.contains(pattern))
        .copied()
}

// ---------------------------------------------------------------------------
// Migration descriptor
// ---------------------------------------------------------------------------

/// A single forward-only schema migration.
#[derive(Debug, Clone)]
pub struct Migration {
    /// Monotonically increasing version number (1-based).
    pub version: u32,
    /// Human-readable name for log messages.
    pub name: &'static str,
    /// SQL to execute. Must not contain destructive statements (A-18).
    pub sql: &'static str,
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

/// Apply all pending migrations in `migrations` to `pool`.
///
/// Creates the `schema_migrations` ledger table if it does not exist (that
/// CREATE is itself idempotent). Skips versions already recorded in the ledger.
/// Executes each pending migration in a transaction so a failure leaves the
/// ledger consistent.
///
/// # Errors
///
/// - `MigrationError::Destructive` if any pending migration SQL contains a
///   destructive statement (A-18 structural refusal, R-0013-d).
/// - `MigrationError::Db` on any database error.
pub async fn apply(pool: &PgPool, migrations: &[Migration]) -> Result<(), MigrationError> {
    // Ensure the ledger table exists.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
             version      INTEGER     PRIMARY KEY,
             name         TEXT        NOT NULL,
             applied_at   TIMESTAMPTZ NOT NULL DEFAULT now()
         )",
    )
    .execute(pool)
    .await?;

    // Load already-applied versions.
    let applied: Vec<(i32,)> =
        sqlx::query_as("SELECT version FROM schema_migrations ORDER BY version")
            .fetch_all(pool)
            .await?;
    let applied_set: std::collections::HashSet<u32> =
        applied.into_iter().map(|(v,)| v as u32).collect();

    for mig in migrations {
        if applied_set.contains(&mig.version) {
            continue;
        }

        // A-18: refuse destructive statements before executing anything.
        if let Some(pattern) = check_destructive(mig.sql) {
            return Err(MigrationError::Destructive(DestructiveMigrationError {
                version: mig.version,
                statement: pattern.to_string(),
                reason: "R-0013-d",
            }));
        }

        // Execute in a transaction for rollback on failure.
        let mut txn = pool.begin().await?;
        sqlx::query(mig.sql).execute(&mut *txn).await?;
        sqlx::query("INSERT INTO schema_migrations (version, name) VALUES ($1, $2)")
            .bind(mig.version as i32)
            .bind(mig.name)
            .execute(&mut *txn)
            .await?;
        txn.commit().await?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests (pure — no engine)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_destructive_catches_drop_table() {
        assert_eq!(
            check_destructive("DROP TABLE IF EXISTS foo"),
            Some("DROP TABLE")
        );
    }

    #[test]
    fn check_destructive_catches_truncate() {
        assert_eq!(check_destructive("TRUNCATE workspaces"), Some("TRUNCATE"));
    }

    #[test]
    fn check_destructive_catches_alter_table() {
        assert_eq!(
            check_destructive("ALTER TABLE workspaces ADD COLUMN foo TEXT"),
            Some("ALTER TABLE")
        );
    }

    #[test]
    fn check_destructive_catches_drop_column() {
        // "DROP COLUMN" pattern is checked before "ALTER TABLE" in the list,
        // so a statement containing both returns "DROP COLUMN" (first match).
        assert_eq!(
            check_destructive("ALTER TABLE foo DROP COLUMN bar"),
            Some("DROP COLUMN")
        );
    }

    #[test]
    fn check_destructive_allows_create_table() {
        assert_eq!(
            check_destructive("CREATE TABLE IF NOT EXISTS foo (id UUID PRIMARY KEY)"),
            None
        );
    }

    #[test]
    fn check_destructive_allows_create_index() {
        assert_eq!(
            check_destructive("CREATE INDEX IF NOT EXISTS idx ON foo (bar)"),
            None
        );
    }

    #[test]
    fn check_destructive_allows_insert() {
        assert_eq!(
            check_destructive("INSERT INTO workspaces (id, name) VALUES ($1, $2)"),
            None
        );
    }

    #[test]
    fn check_destructive_case_insensitive() {
        assert_eq!(
            check_destructive("drop table IF EXISTS foo"),
            Some("DROP TABLE")
        );
    }
}
