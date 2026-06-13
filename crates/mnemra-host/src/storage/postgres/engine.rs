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
// Engine version pin
// ---------------------------------------------------------------------------

/// Pinned Postgres version for the embedded engine.
///
/// Two constraints mandate this exact pin:
///
/// 1. **`pgvector_compiled` asset matrix (portalcorp/pgvector_compiled):** the
///    repository publishes precompiled pgvector packages only for PostgreSQL 16
///    (`pgvector-{aarch64-apple-darwin,x86_64-pc-windows-msvc,x86_64-unknown-linux-gnu}-pg16`).
///    Any Postgres 17+ release produces `AssetNotFound` when `install_extension`
///    attempts to download the matching package, which is the root cause of the
///    CI failure on cold GitHub Ubuntu runners that resolve `VersionReq::STAR` to
///    the latest available release.
///
/// 2. **R-0007-i (engine-pin posture):** the V0 substrate spec requires engine
///    versions to be pinned, not floating, so that test runs are reproducible
///    and the upgrade path is an explicit, tested decision.
///
/// `theseus-rs/postgresql-binaries` publishes `16.4.0` for all three platforms
/// (aarch64-apple-darwin, x86_64-pc-windows-msvc, x86_64-unknown-linux-gnu).
pub const EMBEDDED_PG_VERSION: &str = "=16.4.0";

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
///   bootstrap).  Deliberately `pub(crate)` — privileged access is gated through
///   named methods (e.g. `ensure_pgvector`) rather than direct field access.
///   Task 7's `mnemra init` role-creation (R-0013-e) goes through a new method
///   on this type, not by re-widening the field.
///
/// The server is kept alive as long as this struct is live.
pub struct EmbeddedEngine {
    /// Inner server — kept alive for the engine's lifetime.
    _server: PostgreSQL,
    /// sqlx connection pool authenticated as `mnemra_app` (ordinary role).
    pub pool: Arc<PgPool>,
    /// sqlx connection pool authenticated as the bootstrap superuser.
    /// Narrowed to `pub(crate)` (A-14): privileged ops are exposed as named
    /// methods, not as a raw pool field, so data-path callers cannot accidentally
    /// use superuser credentials.
    pub(crate) superuser_pool: Arc<PgPool>,
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
        // Pin to EMBEDDED_PG_VERSION so cold CI runners don't resolve VersionReq::STAR
        // to a Postgres 17+ release that has no matching pgvector_compiled asset.
        let version = VersionReq::parse(EMBEDDED_PG_VERSION)
            .expect("EMBEDDED_PG_VERSION is a valid semver requirement");
        let settings = SettingsBuilder::new()
            .version(version)
            .temporary(true)
            .build();

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
        // A-09: emit startup timing so cold-runner regressions are visible.
        // Migrates to `log.emit` / structured OTel when Task 25 observability lands.
        eprintln!("engine_startup_ms={elapsed_ms}");

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
        // A-13: idle_timeout + max_lifetime ensure stale connections evict promptly
        // after engine restart (health-probe restart path, Task 25 degraded state).
        // Values are conservative placeholders; revisit for production concurrency
        // when the MCP server (Task 23) has a known concurrency model.
        let app_pool = PgPoolOptions::new()
            .max_connections(20)
            .min_connections(2)
            .acquire_timeout(Duration::from_secs(60))
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(1800))
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
