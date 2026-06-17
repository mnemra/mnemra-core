//! Authentication builtin: static admin-token bootstrap path (R-0015-d).
//!
//! # V0 scope
//!
//! The `authentication` builtin implements the static admin token bootstrap
//! path per P-0008 and P-0009:
//!
//! - On first run (no token row exists for the workspace): generate a new
//!   admin token, store its BLAKE3 hash in `admin_tokens`, and write the raw
//!   token to the token file.
//! - On subsequent runs: the token row already exists; bootstrap is a no-op
//!   (idempotent).
//!
//! # RFC 9728 config surface (V0 stub)
//!
//! `ProtectedResourceMetadata` is the config struct for the RFC 9728
//! protected-resource-metadata endpoint. At V0 this struct is available
//! and serializable, but it is NOT wired to any HTTP handler or OIDC AS.
//!
//! **V0.1+ boundary:** full OIDC AS integration (discovery, token introspection,
//! JWKS validation) is deferred. The tripwire: when Task 23 (MCP server) lands
//! its HTTP surface, wire `ProtectedResourceMetadata` to `/.well-known/oauth-
//! protected-resource` and populate the `authorization_servers` field from
//! the deployment configuration. See R-0015-d.
//!
//! # bootstrap() mechanics
//!
//! ```text
//! bootstrap(pool, workspace_id, token_file_path)
//!   1. COUNT admin_tokens WHERE workspace_id = $1
//!   2. If count == 0:
//!        token = auth::token::generate()
//!        hash  = auth::token::hash(&token)
//!        INSERT INTO admin_tokens (id, token_hash, workspace_id, scopes)
//!        write_token_file(&token, path)
//!   3. Return Ok(BootstrapResult::Created) or Ok(BootstrapResult::AlreadyExists)
//! ```

use crate::auth::token::{self, AdminToken, TokenFileModeError};
use sqlx::PgPool;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// RFC 9728 config surface
// ---------------------------------------------------------------------------

/// Protected-resource metadata per RFC 9728 §2.
///
/// This struct is available at V0 substrate as a typed config surface, but is
/// NOT wired to any HTTP handler or OIDC AS. A config loader can populate it
/// from environment variables or a config file. The `authorization_servers`
/// field is intentionally left empty at V0.
///
/// # V0.1+ boundary
///
/// Wire to `GET /.well-known/oauth-protected-resource` when the MCP HTTP
/// server (Task 23) lands. Populate `authorization_servers` from deployment
/// config. Add `#[derive(serde::Serialize, serde::Deserialize)]` at that time.
#[derive(Debug, Clone, Default)]
pub struct ProtectedResourceMetadata {
    /// The protected resource identifier (URI). RFC 9728 §2 `resource` claim.
    pub resource: String,

    /// List of AS issuer URIs that can authorize access to this resource.
    ///
    /// **V0:** always empty — no AS is configured at V0.
    ///
    /// **V0.1+:** populate from deployment configuration when OIDC AS
    /// integration lands (R-0015-d full OIDC path).
    pub authorization_servers: Vec<String>,

    /// Bearer token methods supported. RFC 9728 `bearer_methods_supported`.
    ///
    /// **V0:** defaults to `["header"]` (Authorization: Bearer).
    ///
    /// **V0.1+:** may be extended to include `["header", "body"]` if needed.
    pub bearer_methods_supported: Vec<String>,
}

impl ProtectedResourceMetadata {
    /// Construct a minimal V0 stub with defaults.
    ///
    /// - `resource`: the caller supplies the resource URI.
    /// - `authorization_servers`: empty (no AS at V0).
    /// - `bearer_methods_supported`: `["header"]`.
    pub fn v0_stub(resource: impl Into<String>) -> Self {
        Self {
            resource: resource.into(),
            authorization_servers: Vec::new(),
            bearer_methods_supported: vec!["header".to_string()],
        }
    }
}

// ---------------------------------------------------------------------------
// Bootstrap result
// ---------------------------------------------------------------------------

/// Result of `bootstrap()`.
#[derive(Debug, PartialEq, Eq)]
pub enum BootstrapResult {
    /// A new admin token was generated, stored, and written to the token file.
    Created,
    /// A token row already existed for this workspace; no action was taken.
    AlreadyExists,
}

// ---------------------------------------------------------------------------
// Bootstrap error
// ---------------------------------------------------------------------------

/// Error returned by `bootstrap()`.
#[derive(Debug)]
pub enum BootstrapError {
    /// A database error occurred.
    Db(sqlx::Error),
    /// The token file could not be written.
    TokenFileWrite(std::io::Error),
}

impl std::fmt::Display for BootstrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BootstrapError::Db(e) => write!(f, "authentication bootstrap db error: {e}"),
            BootstrapError::TokenFileWrite(e) => {
                write!(f, "authentication bootstrap token file write error: {e}")
            }
        }
    }
}

impl std::error::Error for BootstrapError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BootstrapError::Db(e) => Some(e),
            BootstrapError::TokenFileWrite(e) => Some(e),
        }
    }
}

impl From<sqlx::Error> for BootstrapError {
    fn from(e: sqlx::Error) -> Self {
        BootstrapError::Db(e)
    }
}

// ---------------------------------------------------------------------------
// bootstrap() — first-run ensure-token path
// ---------------------------------------------------------------------------

/// Bootstrap the admin token for `workspace_id`.
///
/// # Idempotent
///
/// If a token row already exists for `workspace_id`, returns
/// `Ok(BootstrapResult::AlreadyExists)` without touching the DB or token file.
///
/// # First-run path
///
/// When no token row exists:
/// 1. Generates a new token (32-byte CSPRNG, base64url).
/// 2. Stores its BLAKE3 hash in `admin_tokens` with the given workspace and
///    scopes `["admin"]`.
/// 3. Writes the raw token to `token_file_path` at mode 0600.
///
/// # Token file path
///
/// The caller supplies the path. In production this is
/// `token::default_token_file_path()`. In tests this is a tempdir path.
///
/// # Default scopes
///
/// The bootstrap path grants `["admin"]` — the full-access bootstrap scope.
/// Additional scopes are not configurable at V0; the rotation path preserves
/// the rotated token's scopes exactly — scope adjustment is not a V0 capability
/// (spec:205) and would be a distinct future Admin-gated operation, not a
/// rotation parameter.
pub async fn bootstrap(
    pool: &PgPool,
    workspace_id: Uuid,
    token_file_path: &std::path::Path,
) -> Result<(BootstrapResult, Option<AdminToken>), BootstrapError> {
    // Check if a token row already exists for this workspace.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM admin_tokens WHERE workspace_id = $1")
        .bind(workspace_id)
        .fetch_one(pool)
        .await?;

    if count.0 > 0 {
        return Ok((BootstrapResult::AlreadyExists, None));
    }

    // First run: generate, store, write.
    let new_token = token::generate();
    let new_hash = token::hash(&new_token);
    let new_id = Uuid::new_v4();
    let scopes: Vec<String> = vec!["admin".to_string()];

    sqlx::query(
        "INSERT INTO admin_tokens (id, token_hash, workspace_id, scopes)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(new_id)
    .bind(new_hash.as_bytes())
    .bind(workspace_id)
    .bind(&scopes)
    .execute(pool)
    .await?;

    token::write_token_file(&new_token, token_file_path).map_err(BootstrapError::TokenFileWrite)?;

    Ok((BootstrapResult::Created, Some(new_token)))
}

// ---------------------------------------------------------------------------
// Startup token file mode check
// ---------------------------------------------------------------------------

/// Check the token file's mode at startup (R-0008-e startup check).
///
/// Resolves the path through `token::default_token_file_path()` then calls
/// `token::check_token_file_mode(path)`. Returns `Ok(())` if the file is
/// correctly protected, or the structured error from `check_token_file_mode`.
///
/// A missing file (`NotFound` IO error) is treated as `Ok(())` — no token
/// file yet means first-run; the mode check only applies to an existing file.
pub fn startup_check_token_file_mode() -> Result<(), TokenFileModeError> {
    let Some(path) = token::default_token_file_path() else {
        // No path resolvable (no HOME, no env var) — skip check.
        return Ok(());
    };

    match token::check_token_file_mode(&path) {
        Ok(()) => Ok(()),
        Err(TokenFileModeError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            // File not yet created — first run; skip mode check.
            Ok(())
        }
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_resource_metadata_v0_stub_defaults() {
        let meta = ProtectedResourceMetadata::v0_stub("https://mnemra.example/");
        assert_eq!(meta.resource, "https://mnemra.example/");
        assert!(
            meta.authorization_servers.is_empty(),
            "V0 stub must have no authorization_servers"
        );
        assert_eq!(
            meta.bearer_methods_supported,
            vec!["header".to_string()],
            "V0 stub must support header bearer method"
        );
    }
}
