//! Embedded Postgres engine lifecycle.
//!
//! Wraps `postgresql_embedded::PostgreSQL` with the mnemra-specific bootstrap
//! sequence:
//!
//! 1. `setup()` — installs / re-uses cached Postgres binaries.
//! 2. `start()` — starts the server on an ephemeral port.
//! 3. `install_pgvector()` — downloads and installs the `pgvector_compiled`
//!    precompiled package from the portal-corp repository so that
//!    `CREATE EXTENSION vector` succeeds without an OS-installed extension.
//! 4. `create_database("mnemra")` — creates the application database.
//! 5. `create_app_role()` — creates an ordinary (non-superuser, no BYPASSRLS)
//!    application role with a session-local password, then connects through
//!    that role for all subsequent storage operations.
//!
//! # Role split (V0 / V0.1+ precondition)
//!
//! The bootstrap superuser (`postgres`) is used only during the one-time init
//! phase above.  All runtime connections (`PostgresStorage`) authenticate as the
//! `mnemra_app` role, which holds neither superuser nor BYPASSRLS.  This shape
//! satisfies the P-0010 preconditions so that V0.1+ RLS policy activation
//! (`FORCE ROW LEVEL SECURITY`, `CREATE POLICY`) can be layered on top without
//! structural change.
//!
//! # `CREATE EXTENSION vector` and privilege split
//!
//! `CREATE EXTENSION` requires superuser in Postgres ≤14 and the `pg_extension_owner_changer`
//! role in Postgres 15+, or just superuser.  `ensure_pgvector()` therefore uses the
//! superuser pool — it is a privileged init-time operation, not a runtime operation.
//! Task 7's `mnemra init` calls `ensure_pgvector()` via the superuser path
//! (the only place where schema-creation privileges exist), then hands back to
//! the app role for all subsequent data access.
//!
//! # Task 7 seam
//!
//! `EmbeddedEngine::pool` is the application-role connection pool.
//! `EmbeddedEngine::superuser_pool` is the superuser pool (admin-only, for init).
//! Task 7 calls `ensure_pgvector()` (superuser path), then runs schema migrations
//! (`CREATE TABLE`, etc.) which can run as either superuser or mnemra_app (the app
//! role holds USAGE + CREATE on the public schema).

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use postgresql_embedded::{PostgreSQL, SettingsBuilder, VersionReq};
use postgresql_extensions::install as install_extension;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Structured error surfaced when the `vector` extension cannot be enabled.
///
/// Task 7's `mnemra init` checks for `ExtensionError` to display an actionable
/// message rather than a raw Postgres or archive error.
#[derive(Debug)]
pub struct ExtensionError {
    pub extension: String,
    pub cause: String,
}

impl fmt::Display for ExtensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to enable extension '{}': {}",
            self.extension, self.cause
        )
    }
}

impl Error for ExtensionError {}

// ---------------------------------------------------------------------------
// Application role constants
// ---------------------------------------------------------------------------

/// The ordinary (non-superuser) role used for all runtime storage operations.
pub const APP_ROLE: &str = "mnemra_app";

/// The database created for mnemra storage.
pub const APP_DB: &str = "mnemra";

// ---------------------------------------------------------------------------
// EmbeddedEngine
// ---------------------------------------------------------------------------

/// Handle to a running embedded Postgres instance.
///
/// Exposes two pools:
///
/// - `pool`: application-role pool authenticated as `mnemra_app` (ordinary
///   role — no superuser, no BYPASSRLS).  Used by `PostgresStorage` for all
///   data operations.
/// - `superuser_pool`: bootstrap-superuser pool.  Used by `ensure_pgvector()`
///   and Task 7's init gate for privileged operations (CREATE EXTENSION, schema
///   bootstrap).  Not exposed to data-path callers.
///
/// The server is kept alive as long as this struct is live.
pub struct EmbeddedEngine {
    /// Inner server — kept alive for the engine's lifetime.
    _server: PostgreSQL,
    /// sqlx connection pool authenticated as `mnemra_app` (ordinary role).
    pub pool: Arc<PgPool>,
    /// sqlx connection pool authenticated as the bootstrap superuser.
    /// Used only for privileged init operations.
    pub superuser_pool: Arc<PgPool>,
}

impl EmbeddedEngine {
    /// Start the embedded engine, install pgvector, create the application
    /// database and role, and return a ready `EmbeddedEngine`.
    ///
    /// # Errors
    ///
    /// Propagates any engine, extension-install, or connection error.
    pub async fn start() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let t0 = std::time::Instant::now();

        // Build settings: ephemeral port, temporary data dir (auto-cleaned on drop).
        let settings = SettingsBuilder::new().temporary(true).build();

        // The bootstrap password is generated by SettingsBuilder::new() and stored
        // in settings.password.  We derive the app-role password from it so it stays
        // within the ephemeral session and requires no separate secret management.
        let app_password = format!("{}_{}", settings.password, APP_ROLE);

        let mut server = PostgreSQL::new(settings);

        // setup() downloads (first run) or reuses cached Postgres binaries from
        // ~/.theseus/postgresql/.
        server
            .setup()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // start() assigns an ephemeral port and launches the server.
        server
            .start()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        let elapsed_ms = t0.elapsed().as_millis();
        // elapsed_ms is preserved for caller inspection; not yet wired to tracing.
        let _ = elapsed_ms;

        // Install the pgvector_compiled precompiled extension package.
        // portal-corp provides the precompiled .so + .control + .sql files.
        // No OS-level pgvector installation is required.
        //
        // Guard: skip the download + install if the control file is already present
        // on the shared Postgres installation.  This avoids hitting the GitHub API
        // on every engine startup (which causes rate-limit 403s in repeated test
        // runs) and prevents the destructive uninstall-then-fail pattern that would
        // leave the cache empty on a transient network error.
        //
        // The control file lives at <pg_install>/share/extension/vector.control.
        // We derive the path from the binary dir (same as postgresql_extensions
        // does internally) rather than depending on an exported helper.
        let settings_ref = server.settings();
        let extension_dir = settings_ref
            .installation_dir
            .join("share")
            .join("extension");
        let control_file = extension_dir.join("vector.control");
        if !control_file.exists() {
            install_extension(
                settings_ref,
                "portal-corp",
                "pgvector_compiled",
                &VersionReq::STAR,
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        }

        // Create the application database as the bootstrap superuser.
        server
            .create_database(APP_DB)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Connect to the app database as superuser to perform role bootstrap.
        // Use a generous acquire timeout: in CI, multiple test engines start
        // concurrently and the connection may need time to stabilise.
        let s = server.settings();
        let superuser_url = s.url(APP_DB);
        let superuser_pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_secs(30))
            .connect(&superuser_url)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Create the app role as an ordinary role (no superuser, no BYPASSRLS).
        // P-0010 AC#3 binds here: the runtime role must never hold these flags.
        // The role is created with a password so it can authenticate over TCP
        // (the embedded cluster uses md5/scram for all local connections).
        //
        // Safety: APP_ROLE and app_password are generated by this crate
        // (constants + per-session random suffix); no user input is interpolated.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE ROLE {APP_ROLE} WITH LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE \
             NOINHERIT PASSWORD '{app_password}'"
        )))
        .execute(&superuser_pool)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Grant database access and schema CREATE to the app role.
        // Task 7 migrations run as mnemra_app so it needs CREATE on public schema.
        //
        // Safety: APP_DB and APP_ROLE are internal string constants.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "GRANT CONNECT ON DATABASE {APP_DB} TO {APP_ROLE}"
        )))
        .execute(&superuser_pool)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        sqlx::query(sqlx::AssertSqlSafe(format!(
            "GRANT USAGE, CREATE ON SCHEMA public TO {APP_ROLE}"
        )))
        .execute(&superuser_pool)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Connect as the app role for all subsequent storage operations.
        let app_url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            APP_ROLE, app_password, s.host, s.port, APP_DB,
        );

        // max_connections: generous for test parallelism; enough for concurrent
        // queries across multiple test cases sharing this pool.
        let app_pool = PgPoolOptions::new()
            .max_connections(20)
            .min_connections(2)
            .acquire_timeout(Duration::from_secs(60))
            .connect(&app_url)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        Ok(EmbeddedEngine {
            _server: server,
            pool: Arc::new(app_pool),
            superuser_pool: Arc::new(superuser_pool),
        })
    }

    /// Verify that the `vector` extension is available and enable it.
    ///
    /// Runs as the bootstrap superuser because `CREATE EXTENSION` requires
    /// elevated privilege.  Returns `Err(ExtensionError)` if the extension
    /// cannot be created — this is the structured error surface Task 7 builds
    /// its `mnemra init` gate on.
    ///
    /// After a successful call, the `vector` extension is available on the
    /// `mnemra` database for the app role to use.
    pub async fn ensure_pgvector(&self) -> Result<(), ExtensionError> {
        sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(self.superuser_pool.as_ref())
            .await
            .map_err(|e| ExtensionError {
                extension: "vector".into(),
                cause: e.to_string(),
            })?;
        Ok(())
    }
}
