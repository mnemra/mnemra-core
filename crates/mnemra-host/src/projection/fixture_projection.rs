//! Fixture materialized projection for the refresh-queue exercise (R-0001-f).
//!
//! # What this module provides
//!
//! `create_echo_fixture_projection(pool)` — idempotently creates:
//!
//! - A minimal materialized view `echo_fixture_status_counts` over the
//!   `echo_fixture` table counting rows per `(frontmatter->>'status')` value.
//! - A UNIQUE index on `(status)` — required for
//!   `REFRESH MATERIALIZED VIEW CONCURRENTLY`.
//!
//! This view is **fixture scaffolding** for exercising the refresh queue (Task 9
//! self-test). It is not part of the production schema; a real artifact type
//! would define its own projections.
//!
//! # CONCURRENTLY prerequisites
//!
//! 1. Unique index: `echo_fixture_status_counts_status_uidx` on `(status)`.
//! 2. Populated at creation: `CREATE MATERIALIZED VIEW ... WITH DATA`.
//! 3. REFRESH must not run inside a transaction — the worker uses autocommit.

use sqlx::PgPool;

/// Name of the fixture materialized view.
pub const FIXTURE_MATVIEW: &str = "echo_fixture_status_counts";

/// Create the `echo_fixture_status_counts` materialized view and its unique
/// index.
///
/// Idempotent: uses `CREATE MATERIALIZED VIEW IF NOT EXISTS` and
/// `CREATE UNIQUE INDEX IF NOT EXISTS`.
///
/// Must be called after `create_artifact_table(pool, "echo_fixture")` returns.
pub async fn create_echo_fixture_projection(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Materialized view: status → count of artifacts with that status.
    // Uses `WITH DATA` (the default) so the unique index can be built
    // immediately and CONCURRENTLY refresh works from the first call.
    sqlx::query(
        "CREATE MATERIALIZED VIEW IF NOT EXISTS echo_fixture_status_counts AS
         SELECT
             frontmatter ->> 'status'  AS status,
             COUNT(*)::BIGINT          AS artifact_count
         FROM echo_fixture
         GROUP BY frontmatter ->> 'status'
         WITH DATA",
    )
    .execute(pool)
    .await?;

    // UNIQUE index on (status) — required for REFRESH MATERIALIZED VIEW CONCURRENTLY.
    // The status field may be NULL (artifacts without a status key); NULL values
    // do not conflict in a unique index (each NULL is distinct).
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS echo_fixture_status_counts_status_uidx
         ON echo_fixture_status_counts (status)",
    )
    .execute(pool)
    .await?;

    Ok(())
}
