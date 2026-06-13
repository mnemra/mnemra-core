//! Embedded Postgres engine lifecycle.
//!
//! Wraps `postgresql_embedded::PostgreSQL` with the mnemra-specific bootstrap
//! sequence:
//!
//! 1. `setup()` — installs / re-uses cached Postgres binaries.
//! 2. `verify_pinned_artifacts()` — SHA-256 hash-pin of the installed Postgres
//!    binary and pgvector shared library (A-04/A-05 interim control, Task 6b).
//!    Fail-shut: unknown platform → error; hash mismatch → error.
//! 3. `start()` — starts the server on an ephemeral port.
//! 4. `install_pgvector()` — downloads and installs the `pgvector_compiled`
//!    precompiled package from the portal-corp repository so that
//!    `CREATE EXTENSION vector` succeeds without an OS-installed extension.
//! 5. `create_database("mnemra")` — creates the application database.
//! 6. `create_app_role()` — creates an ordinary (non-superuser, no BYPASSRLS)
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
//!
//! # Hash-pin control (A-04/A-05, Task 6b)
//!
//! `verify_pinned_artifacts()` is called at engine bring-up BEFORE first use.
//! It checks the SHA-256 of the `postgres` binary and the `vector` shared library
//! that the crates install into `~/.theseus/`.  These are the artifacts that actually
//! execute — verifying them covers both the fresh-download path (crate extracts the
//! archive and we verify the result) and the warm-cache path (a tampered cache is
//! the realistic local threat).
//!
//! ## Coverage and known window (Task 26 handoff)
//!
//! - **Covered:** the extracted installed files are verified before the engine starts.
//! - **Not covered by this control:** TOCTOU window between the crate's internal
//!   download/extract and our check.  Full archive integrity during download is
//!   partly addressed by the crate for PG (theseus releases include `.sha256` files
//!   that `postgresql_archive` fetches and checks); pgvector archives have no
//!   upstream hash file — that gap carries to Task 26 (SBOM + full provenance).
//! - **Pin maintenance:** when `EMBEDDED_PG_VERSION` or `PGVECTOR_VERSION` changes,
//!   the `KNOWN_GOOD_HASHES` table MUST be updated.  Task 26 owns the supply-chain
//!   audit; this pin is the V0 interim gate.

use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use postgresql_embedded::{PostgreSQL, SettingsBuilder, VersionReq};
use postgresql_extensions::install as install_extension;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Error types
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

/// Structured error returned when a pinned-artifact SHA-256 check fails.
///
/// The engine refuses to start on any `HashPinError` — fail-shut.
/// No warn-and-continue path exists.
#[derive(Debug)]
pub struct HashPinError {
    /// Human-readable name of the artifact being verified.
    pub artifact: String,
    /// Expected (pinned) SHA-256 hex digest.
    pub expected: String,
    /// Actual SHA-256 hex digest of the file on disk.
    pub actual: String,
}

impl fmt::Display for HashPinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "hash-pin mismatch for '{}': expected {} got {}",
            self.artifact, self.expected, self.actual
        )
    }
}

impl Error for HashPinError {}

// ---------------------------------------------------------------------------
// Engine version pins
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

/// Pinned pgvector_compiled release version.
///
/// Replaces `VersionReq::STAR` (A-04): a floating `STAR` combined with a hash
/// pin creates a latent outage — when portalcorp ships the next release, the new
/// archive fails the hash check and the engine refuses to start without any
/// code change on our side.
///
/// `portalcorp/pgvector_compiled` release `v0.16.105` bundles pgvector 0.8.0
/// (confirmed via `vector.control` default_version) for the three required
/// platforms.  The `v` prefix is stripped by `postgresql_archive`'s tag parser
/// before semver matching.
pub const PGVECTOR_VERSION: &str = "=0.16.105";

// ---------------------------------------------------------------------------
// SHA-256 hash-pin table (A-04/A-05 interim control, Task 6b)
// ---------------------------------------------------------------------------

/// A known-good SHA-256 hash entry for an installed artifact file.
///
/// Public so that tests can construct deliberate-mismatch entries to prove
/// fail-shut behaviour without touching the shared `~/.theseus` cache.
pub struct ArtifactPin {
    /// Identifier for this entry (used in error messages).
    pub artifact: &'static str,
    /// Path of the file relative to the postgresql_embedded installation root
    /// (e.g. `~/.theseus/postgresql/16.4.0/`).
    pub rel_path: &'static str,
    /// Expected SHA-256 hex digest.
    pub sha256: &'static str,
}

/// Known-good SHA-256 hashes of installed artifact files, keyed by Rust
/// `target_arch`-`target_os`-`target_env` triple components.
///
/// How each hash was obtained:
///
/// 1. `gh release download 16.4.0 --repo theseus-rs/postgresql-binaries` →
///    `shasum -a 256` on archive, then cross-checked against the upstream
///    `.sha256` sibling file (matches confirmed).
/// 2. Archive extracted to `/tmp`; the `postgres` binary was hashed.
/// 3. Cross-checked against the locally-cached `~/.theseus/postgresql/16.4.0/bin/postgres`
///    (both aarch64 hashes matched — local cache is clean).
/// 4. `gh release download v0.16.105 --repo portalcorp/pgvector_compiled` →
///    `shasum -a 256` on `.zip` (portalcorp has no upstream `.sha256` sibling —
///    computed-only; gap carries to Task 26).
/// 5. Zip extracted to `/tmp`; the `vector.{dylib,so}` library was hashed.
/// 6. Cross-checked against locally-cached `~/.theseus/...lib/vector.*`
///    (aarch64 confirmed; linux cross-check not possible on darwin host).
///
/// **Pin maintenance:** when `EMBEDDED_PG_VERSION` or `PGVECTOR_VERSION` changes,
/// update this table.  Task 26 owns the supply-chain audit and full SBOM.
pub const KNOWN_GOOD_HASHES: &[(&str, &[ArtifactPin])] = &[
    (
        // Local dev: Apple Silicon Mac
        "aarch64-apple-darwin",
        &[
            ArtifactPin {
                artifact: "postgres-16.4.0-aarch64-apple-darwin",
                rel_path: "bin/postgres",
                // From: theseus-rs/postgresql-binaries release 16.4.0
                // Archive: postgresql-16.4.0-aarch64-apple-darwin.tar.gz
                // Archive SHA-256 (upstream .sha256 file confirmed):
                //   0ec91e77eff381e43e3963f012aff3acb9de12ad3739a625e57cce9671b28b0f
                // Installed binary SHA-256 (extracted from archive, cross-checked
                // against local ~/.theseus cache — match confirmed):
                sha256: "a245e44bebf13f9b61ef3855b085476cdd71ac59d22e4d99cc7f879c30d48ef3",
            },
            ArtifactPin {
                artifact: "pgvector-0.8.0(v0.16.105)-aarch64-apple-darwin",
                rel_path: "lib/vector.dylib",
                // From: portalcorp/pgvector_compiled release v0.16.105
                // Archive: pgvector-aarch64-apple-darwin-pg16.zip
                // Archive SHA-256 (computed; portalcorp has no upstream .sha256):
                //   66b8af9e38510007a7041f5e25e7975685cd219c9fd1459d779a0762abc5600b
                // Installed library SHA-256 (extracted from archive, cross-checked
                // against local ~/.theseus cache — match confirmed):
                sha256: "de2fd49fcc0602f90c5b8821dd744f21f2a933a1eab0e600c944b683e8d3b65a",
            },
        ],
    ),
    (
        // CI runner: x86_64 Linux glibc
        "x86_64-unknown-linux-gnu",
        &[
            ArtifactPin {
                artifact: "postgres-16.4.0-x86_64-unknown-linux-gnu",
                rel_path: "bin/postgres",
                // From: theseus-rs/postgresql-binaries release 16.4.0
                // Archive: postgresql-16.4.0-x86_64-unknown-linux-gnu.tar.gz
                // Archive SHA-256 (upstream .sha256 file confirmed):
                //   1059350056c24e6dd3974af7582199c2a4d06078ecb2beb9f4b26b6debea6d37
                // Installed binary SHA-256 (extracted from archive; cross-check
                // against remote cache not possible from darwin host):
                sha256: "3ad9bf317793480fc50a0467b5da7ccbade48c41a0b234e1a27552c855945f8e",
            },
            ArtifactPin {
                artifact: "pgvector-0.8.0(v0.16.105)-x86_64-unknown-linux-gnu",
                rel_path: "lib/vector.so",
                // From: portalcorp/pgvector_compiled release v0.16.105
                // Archive: pgvector-x86_64-unknown-linux-gnu-pg16.zip
                // Archive SHA-256 (computed; portalcorp has no upstream .sha256):
                //   ffa189a117ad2e3a3b2f5d73ef3f8130db3e062d2e82063350dee5201798e922
                // Installed library SHA-256 (extracted from archive; cross-check
                // against remote cache not possible from darwin host):
                sha256: "d464f84c02e13744ad80a3d8316ec77e59759063a31077ef4798b51fa5daf33d",
            },
        ],
    ),
];

// ---------------------------------------------------------------------------
// Artifact hash verification (pure function — testable without a live engine)
// ---------------------------------------------------------------------------

/// Verify the SHA-256 hash of every pinned artifact in `install_dir`.
///
/// This is a pure function that takes the installation directory, the current
/// platform triple, and a pin table.  Production code passes
/// [`KNOWN_GOOD_HASHES`]; tests pass an intentionally-wrong table to prove
/// fail-shut without touching the shared `~/.theseus` cache.
///
/// # Errors
///
/// Returns `HashPinError` on the first mismatch found.  Returns a distinct
/// `HashPinError` with `expected = "(no pin entry)"` if the platform is not in
/// the table — fail-shut, not silent skip.
pub fn verify_pinned_artifacts(
    install_dir: &Path,
    platform: &str,
    pins: &[(&str, &[ArtifactPin])],
) -> Result<(), HashPinError> {
    // Look up the pin entries for this platform.
    let entries = pins
        .iter()
        .find(|(p, _)| *p == platform)
        .map(|(_, e)| *e)
        .ok_or_else(|| HashPinError {
            artifact: format!("platform:{}", platform),
            expected: "(no pin entry — add this platform to KNOWN_GOOD_HASHES)".into(),
            actual: "(unknown)".into(),
        })?;

    for pin in entries {
        let path = install_dir.join(pin.rel_path);
        let bytes = fs::read(&path).map_err(|e| HashPinError {
            artifact: pin.artifact.into(),
            expected: pin.sha256.into(),
            actual: format!("(read error: {})", e),
        })?;

        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let digest = hasher.finalize();
        let actual: String = digest.iter().map(|b| format!("{b:02x}")).collect();

        if actual != pin.sha256 {
            return Err(HashPinError {
                artifact: pin.artifact.into(),
                expected: pin.sha256.into(),
                actual,
            });
        }
    }
    Ok(())
}

/// Returns the current target platform triple used as the key in
/// [`KNOWN_GOOD_HASHES`].
///
/// Determined at compile time via `cfg` attributes so it is a zero-cost
/// constant expression at runtime.
pub fn current_platform() -> &'static str {
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(not(any(
        all(target_arch = "aarch64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "linux", target_env = "gnu")
    )))]
    {
        // Deliberately not a constant we match — ensures the platform check in
        // verify_pinned_artifacts returns the "no pin entry" structured error.
        "unknown-platform"
    }
}

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

        // Hash-pin verification (A-04/A-05, Task 6b) — BEFORE first use.
        //
        // Verifies the installed postgres binary and pgvector shared library
        // against known-good SHA-256 hashes.  Covers both the fresh-download
        // path (crate extracts archive → we verify the result) and the warm-cache
        // path (tampered local cache is the realistic threat model).
        //
        // Fail-shut: any mismatch or unknown platform → structured error, engine
        // refuses to start.  No warn-and-continue mode.
        //
        // TOCTOU note: there is a window between the crate's download/extract and
        // this check; closing that window requires intercepting the crate's internal
        // download path, which is not possible without upstream changes.  That gap
        // carries to Task 26 (full provenance / SBOM).
        let install_dir = server.settings().installation_dir.clone();
        verify_pinned_artifacts(&install_dir, current_platform(), KNOWN_GOOD_HASHES)
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
            let pgvector_version = VersionReq::parse(PGVECTOR_VERSION)
                .expect("PGVECTOR_VERSION is a valid semver requirement");
            install_extension(
                settings_ref,
                "portal-corp",
                "pgvector_compiled",
                &pgvector_version,
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
