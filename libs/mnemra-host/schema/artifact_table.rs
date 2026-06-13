//! Per-artifact-type table generator (Tasks 8/9 — GREEN phase).
//!
//! # What this module provides
//!
//! `create_artifact_table(pool, type_name)` — idempotently creates:
//!
//! - A table `<type_name>` with the full C1 column set (R-0001-a, R-0001-b).
//! - Two CHECK constraints: `frontmatter ? 'id'` and
//!   `frontmatter ? 'frontmatter_version'` (R-0001-c).
//! - A `workspace_id` B-tree index (primary access pattern, R-0001-a).
//! - Four expression indexes on `(frontmatter->>'status')`,
//!   `(frontmatter->>'priority')`, `(frontmatter->>'project_id')`, and
//!   `(frontmatter->>'parent_id')` (R-0001-d).
//!
//! This DDL is **generator-executed** (not through the `migrations::apply()`
//! ledger) because each artifact type is independent and the set grows at
//! plugin-registration time (Task 19 seam). The ledger records named,
//! pre-authored migrations; per-type DDL is structural but not authored ahead
//! of time.
//!
//! # Type-name validation
//!
//! `validate_type_name` enforces `[a-z][a-z0-9_]*`, max 63 characters (Postgres
//! identifier limit). Injection through the type name is structurally
//! inexpressible: the validator returns `Err(TypeNameError)` before any SQL is
//! formed.
//!
//! # Task 19 seam
//!
//! `init()` calls `create_artifact_table` for each type in
//! `FIXTURE_CONTENT_TYPES`. Task 19 extends this by reading the plugin manifest
//! content-type list and calling `create_artifact_table` for each registered
//! type. No changes to this module are required; Task 19 calls this function
//! directly.
//!
//! # History shadow table
//!
//! The companion `history_trigger` module creates the `<type_name>_history`
//! shadow table and BEFORE UPDATE / BEFORE DELETE triggers. Call
//! `history_trigger::create_history_machinery(pool, type_name)` after this
//! function returns.

use sqlx::PgPool;
use std::fmt;

// ---------------------------------------------------------------------------
// Type-name validation
// ---------------------------------------------------------------------------

/// Maximum byte length for a validated type name (Postgres identifier limit).
const MAX_TYPE_NAME_LEN: usize = 63;

/// Error returned when a type name fails validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeNameError {
    /// The rejected input (truncated to 80 bytes for safety).
    pub input: String,
    /// Human-readable rejection reason.
    pub reason: &'static str,
}

impl fmt::Display for TypeNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid artifact type name {:?}: {}",
            self.input, self.reason
        )
    }
}

impl std::error::Error for TypeNameError {}

/// Validate `type_name` for use as a Postgres table-name suffix.
///
/// Accepts `[a-z][a-z0-9_]*` up to 63 bytes.  Returns a borrowed `&str` on
/// success (same lifetime as input) so callers can use the validated name
/// directly in format strings without an extra allocation.
///
/// # Security
///
/// Any `type_name` that passes this validator is safe to interpolate directly
/// into DDL statements: the allowed character set contains no SQL metacharacters,
/// comment sequences, or quote characters.
pub fn validate_type_name(type_name: &str) -> Result<&str, TypeNameError> {
    if type_name.is_empty() {
        return Err(TypeNameError {
            input: String::new(),
            reason: "type name must not be empty",
        });
    }

    if type_name.len() > MAX_TYPE_NAME_LEN {
        return Err(TypeNameError {
            input: type_name[..80.min(type_name.len())].to_string(),
            reason: "type name exceeds 63-byte Postgres identifier limit",
        });
    }

    let mut chars = type_name.chars();

    // First character: must be [a-z].
    let first = chars.next().unwrap(); // non-empty checked above
    if !first.is_ascii_lowercase() {
        return Err(TypeNameError {
            input: type_name.to_string(),
            reason: "type name must start with a lowercase ASCII letter [a-z]",
        });
    }

    // Remaining characters: [a-z0-9_].
    for ch in chars {
        if !matches!(ch, 'a'..='z' | '0'..='9' | '_') {
            return Err(TypeNameError {
                input: type_name.to_string(),
                reason: "type name must contain only lowercase ASCII letters, digits, and underscores [a-z0-9_]",
            });
        }
    }

    Ok(type_name)
}

// ---------------------------------------------------------------------------
// Generator error
// ---------------------------------------------------------------------------

/// Error returned by `create_artifact_table`.
#[derive(Debug)]
pub enum ArtifactTableError {
    /// The type name failed validation.
    InvalidTypeName(TypeNameError),
    /// A database error occurred while creating the table or its indexes.
    Db(sqlx::Error),
}

impl fmt::Display for ArtifactTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtifactTableError::InvalidTypeName(e) => {
                write!(f, "artifact table generator: {e}")
            }
            ArtifactTableError::Db(e) => {
                write!(f, "artifact table generator: db error — {e}")
            }
        }
    }
}

impl std::error::Error for ArtifactTableError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ArtifactTableError::InvalidTypeName(e) => Some(e),
            ArtifactTableError::Db(e) => Some(e),
        }
    }
}

impl From<TypeNameError> for ArtifactTableError {
    fn from(e: TypeNameError) -> Self {
        ArtifactTableError::InvalidTypeName(e)
    }
}

impl From<sqlx::Error> for ArtifactTableError {
    fn from(e: sqlx::Error) -> Self {
        ArtifactTableError::Db(e)
    }
}

// ---------------------------------------------------------------------------
// Generator
// ---------------------------------------------------------------------------

/// Create the per-artifact-type table, CHECK constraints, and all indexes for
/// `type_name`.
///
/// Idempotent: uses `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT
/// EXISTS` throughout.
///
/// # C1 column set (R-0001-a, R-0001-b)
///
/// | Column               | Type        | Nullable | Notes                       |
/// |----------------------|-------------|----------|-----------------------------|
/// | `id`                 | TEXT        | NOT NULL | ULID primary key            |
/// | `workspace_id`       | UUID        | NOT NULL | workspace scoping           |
/// | `type`               | TEXT        | NOT NULL | artifact type discriminator |
/// | `frontmatter`        | JSONB       | NOT NULL | structured metadata         |
/// | `body`               | TEXT        | NULL     | optional free-text content  |
/// | `frontmatter_version`| BIGINT      | NOT NULL | monotonic version counter   |
/// | `migrated_from`      | TEXT        | NULL     | migration provenance        |
/// | `migrated_at`        | TIMESTAMPTZ | NULL     | migration timestamp         |
/// | `created_at`         | TIMESTAMPTZ | NOT NULL | creation timestamp          |
/// | `updated_at`         | TIMESTAMPTZ | NOT NULL | last-update timestamp       |
///
/// `migrated_from`, `migrated_at`, and `frontmatter_version` are dedicated
/// typed columns — not stored inside the `frontmatter` JSONB (R-0001-b).
///
/// # CHECK constraints (R-0001-c)
///
/// - `frontmatter ? 'id'` — frontmatter MUST contain the `id` key.
/// - `frontmatter ? 'frontmatter_version'` — frontmatter MUST contain the
///   `frontmatter_version` key.
///
/// # Indexes
///
/// - `<type_name>_workspace_idx` — B-tree on `workspace_id` (R-0001-a).
/// - `<type_name>_fm_status_idx` — expression on `(frontmatter->>'status')`.
/// - `<type_name>_fm_priority_idx` — expression on `(frontmatter->>'priority')`.
/// - `<type_name>_fm_project_id_idx` — expression on `(frontmatter->>'project_id')`.
/// - `<type_name>_fm_parent_id_idx` — expression on `(frontmatter->>'parent_id')`.
///
/// # Task 19 seam
///
/// Call this function for each type in the plugin manifest's content-type list.
/// The `validate_type_name` call is the injection-prevention boundary.
pub async fn create_artifact_table(
    pool: &PgPool,
    type_name: &str,
) -> Result<(), ArtifactTableError> {
    // Validate before any SQL is formed — injection inexpressible after this point.
    let name = validate_type_name(type_name)?;

    // CREATE TABLE (idempotent).
    // AssertSqlSafe: `name` passed validate_type_name — [a-z][a-z0-9_]*, ≤63 bytes.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE IF NOT EXISTS {name} (
            id                  TEXT        NOT NULL,
            workspace_id        UUID        NOT NULL,
            type                TEXT        NOT NULL,
            frontmatter         JSONB       NOT NULL,
            body                TEXT,
            frontmatter_version BIGINT      NOT NULL,
            migrated_from       TEXT,
            migrated_at         TIMESTAMPTZ,
            created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
            updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
            CONSTRAINT {name}_pkey
                PRIMARY KEY (id),
            CONSTRAINT {name}_frontmatter_has_id
                CHECK (frontmatter ? 'id'),
            CONSTRAINT {name}_frontmatter_has_version
                CHECK (frontmatter ? 'frontmatter_version')
        )"
    )))
    .execute(pool)
    .await?;

    // Workspace-id covering index (R-0001-a).
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE INDEX IF NOT EXISTS {name}_workspace_idx ON {name} (workspace_id)"
    )))
    .execute(pool)
    .await?;

    // Expression indexes on commonly-filtered frontmatter fields (R-0001-d).
    // Standard form `((frontmatter->>'<field>'))` — PG16 deparses to the
    // pattern `(frontmatter ->> '<field>'::text)` matched by the red tests.
    for (suffix, field) in &[
        ("fm_status_idx", "status"),
        ("fm_priority_idx", "priority"),
        ("fm_project_id_idx", "project_id"),
        ("fm_parent_id_idx", "parent_id"),
    ] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE INDEX IF NOT EXISTS {name}_{suffix} ON {name} ((frontmatter->>'{field}'))"
        )))
        .execute(pool)
        .await?;
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
    fn validate_type_name_accepts_simple() {
        assert_eq!(validate_type_name("echo_fixture"), Ok("echo_fixture"));
    }

    #[test]
    fn validate_type_name_accepts_with_digits() {
        assert_eq!(validate_type_name("task2"), Ok("task2"));
    }

    #[test]
    fn validate_type_name_rejects_empty() {
        assert!(validate_type_name("").is_err());
    }

    #[test]
    fn validate_type_name_rejects_starts_with_digit() {
        let e = validate_type_name("1foo").unwrap_err();
        assert!(e.reason.contains("start with a lowercase"));
    }

    #[test]
    fn validate_type_name_rejects_uppercase() {
        let e = validate_type_name("MyType").unwrap_err();
        assert!(e.reason.contains("start with a lowercase"));
    }

    #[test]
    fn validate_type_name_rejects_hyphen() {
        let e = validate_type_name("my-type").unwrap_err();
        assert!(e.reason.contains("only lowercase"), "{}", e.reason);
    }

    #[test]
    fn validate_type_name_rejects_sql_injection_attempt() {
        // Classic injection: `'; DROP TABLE foo; --`
        let e = validate_type_name("foo'; DROP TABLE bar; --").unwrap_err();
        assert!(e.reason.contains("only lowercase"), "{}", e.reason);
    }

    #[test]
    fn validate_type_name_rejects_space() {
        let e = validate_type_name("my type").unwrap_err();
        assert!(e.reason.contains("only lowercase"), "{}", e.reason);
    }

    #[test]
    fn validate_type_name_rejects_too_long() {
        let long = "a".repeat(64);
        let e = validate_type_name(&long).unwrap_err();
        assert!(e.reason.contains("63-byte"), "{}", e.reason);
    }

    #[test]
    fn validate_type_name_accepts_exactly_63() {
        // 63 'a' characters — exactly at the limit.
        let name = "a".repeat(63);
        assert!(validate_type_name(&name).is_ok());
    }
}
