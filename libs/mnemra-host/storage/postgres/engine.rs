//! Embedded Postgres engine lifecycle.
//!
//! Wraps `postgresql_embedded::PostgreSQL` with the mnemra-specific bootstrap
//! sequence:
//!
//! 1. `setup()` — installs / re-uses cached Postgres binaries. This ALWAYS
//!    runs `initdb` internally: mnemra never overrides `SettingsBuilder`'s
//!    default `data_dir` (a fresh `tempfile::tempdir()` per
//!    `postgresql_embedded` `Settings::new()`), so the crate's own
//!    `is_initialized()` check is always false and `initdb` — which EXECUTES
//!    the just-installed `postgres`-family binaries — runs on every
//!    `start()`, not only a cold one. This happens before step 2's hash
//!    check ever runs; see "Known gaps" below.
//! 2. `verify_pinned_artifacts()` (Postgres binary) — SHA-256 hash-pin of
//!    the installed `postgres` binary (A-04/A-05 interim control, Task 6b),
//!    checked right after `setup()` returns. Fail-shut: unknown platform,
//!    no pin of the requested kind, or a hash mismatch (including a missing
//!    file) → error.
//! 3. `install_extension()` (pgvector) — downloads and installs the
//!    `pgvector_compiled` precompiled package from the portal-corp repository
//!    so that `CREATE EXTENSION vector` succeeds without an OS-installed
//!    extension. Skipped when `share/extension/vector.control` already
//!    exists on the installation. Needs no running server — confirmed from
//!    `postgresql_extensions` crate source: it only shells out to
//!    `postgres --version` and `pg_config` (one-shot subprocess calls) and
//!    extracts an archive — so this step runs BEFORE `start()`.
//! 4. `verify_pinned_artifacts()` (pgvector library) — SHA-256 hash-pin of
//!    the installed pgvector shared library, checked right after step 3
//!    (whichever branch it took) and before any caller can reach
//!    `CREATE EXTENSION` or other use of the extension. Same fail-shut
//!    contract as step 2, also before `start()`.
//! 5. `start()` — starts the server on an ephemeral port. Both pinned
//!    artifacts (steps 2 and 4) have already passed their hash check by
//!    this point — this split (brain #3597) replaces a single combined
//!    check that ran right after step 1, before the pgvector library had
//!    ever been written, which made every cold start (`~/.theseus` never
//!    populated, or its cache expired) fail.
//! 6. `create_database("mnemra")` — creates the application database.
//! 7. `create_app_role()` — creates an ordinary (non-superuser, no BYPASSRLS)
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
//! `verify_pinned_artifacts()` is called TWICE at engine bring-up (brain
//! #3597) — once per [`ArtifactKind`] — verifying the TWO PINNED files this
//! table names (`bin/postgres` and the pgvector shared library), each right
//! before `start()` launches a server process, rather than as one combined
//! gate that ran before either file necessarily existed on disk. "Before
//! `start()`" is a narrower claim than "before the file is first executed
//! at all" — see "Known gaps" below, which names where that narrower claim
//! matters.
//!
//! - [`ArtifactKind::Postgres`] — checked right after `setup()` returns.
//! - [`ArtifactKind::Pgvector`] — checked right after the install-or-skip
//!   decision (`install_extension()` / the `vector.control`-exists guard).
//!
//! [`ArtifactKind::verification_site`] is an exhaustive match with no `_` arm,
//! so adding a variant forces a compile-time touch here. It does NOT force a
//! `verify_pinned_artifacts()` call for that variant: a new arm alone
//! compiles. Wiring the call remains a reviewed step (see the method's own
//! doc). Pins added to an EXISTING kind are verified automatically by the
//! kind filter.
//!
//! It checks the SHA-256 of the `postgres` binary and the `vector` shared library
//! that the crates install into `~/.theseus/`.  Verifying them covers both the
//! fresh-download path (crate extracts the archive and we verify the result) and
//! the warm-cache path (a tampered cache is the realistic local threat).
//!
//! ## Known gaps (brain #3616 tracks closing these)
//!
//! - **The Postgres binary's first execution is not covered.**
//!   `postgresql_embedded::PostgreSQL::setup()` calls the crate's private
//!   `install()` and then `initialize()` — which builds and runs an
//!   `initdb` command using the just-installed binaries (see step 1 above:
//!   mnemra's usage makes this run on EVERY `start()`). That execution
//!   happens BEFORE `verify_pinned_artifacts(..., ArtifactKind::Postgres)`
//!   ever runs, so a tampered cached `postgres` binary set would already
//!   have been run by `initdb` before this control could refuse it. The
//!   warm-cache half looks closable from mnemra's side (the versioned
//!   installation dir is known before `setup()`); the cold path needs
//!   either an upstream change or mnemra-side download, verify and extract.
//!   See the candidate directions on brain #3616.
//! - **Only two files are pinned.** `bin/initdb`, `bin/pg_ctl`,
//!   `bin/pg_config`, and the rest of `bin/` are unpinned executables from
//!   the same archive as `bin/postgres`. On the pgvector side,
//!   `share/extension/vector.control` and the `vector--*.sql` upgrade
//!   scripts — also read by the server at `CREATE EXTENSION` / upgrade time,
//!   as the bootstrap superuser — are unpinned too. Widening the pin table
//!   is brain #3616's scope, not this control's V0 shape.
//! - **Archive-download integrity:** partly addressed by the crate for PG
//!   (theseus releases include `.sha256` files that `postgresql_archive`
//!   fetches and checks); pgvector archives have no upstream hash file —
//!   this control's own post-extraction hash check is what covers that case
//!   today. The remaining TOCTOU window between the crate's internal
//!   download/extract and this control's check, plus full archive
//!   provenance / SBOM, is brain #3616.
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
use sqlx::Connection;
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

/// Which class of pinned artifact an [`ArtifactPin`] entry describes.
///
/// `EmbeddedEngine::start()` verifies each kind at its own point in the
/// bootstrap sequence (brain #3597) instead of checking every pin in one
/// combined gate — see the module-level "Hash-pin control" doc section for
/// why a combined gate is the defect this splits away from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    /// The `postgres` server binary — verified before `server.start()`.
    Postgres,
    /// The pgvector shared library — verified after the install-or-skip
    /// decision, before any `CREATE EXTENSION` or other use of the extension.
    Pgvector,
}

impl ArtifactKind {
    /// Where in `EmbeddedEngine::start()` this kind is verified.
    ///
    /// M4 (brain #3597 fix-up): an exhaustive `match` with no `_` arm —
    /// adding a new `ArtifactKind` variant fails to compile HERE until an
    /// arm is added, which is the prompt to also add that variant's real
    /// `verify_pinned_artifacts()` call in `start()`. Without this, a new
    /// variant plus new pins in `KNOWN_GOOD_HASHES` would compile clean and
    /// simply never be checked — the exact silent-coverage gap this guards
    /// against. `start()` calls this at each real call site (see there) so
    /// the match is genuinely exercised, not dead code.
    fn verification_site(self) -> &'static str {
        match self {
            ArtifactKind::Postgres => "right after setup(), before start()",
            ArtifactKind::Pgvector => "right after the install-or-skip decision, before start()",
        }
    }
}

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
    /// Which artifact class this pin belongs to — selects when
    /// `verify_pinned_artifacts()` checks it.
    pub kind: ArtifactKind,
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
                kind: ArtifactKind::Postgres,
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
                kind: ArtifactKind::Pgvector,
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
                kind: ArtifactKind::Postgres,
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
                kind: ArtifactKind::Pgvector,
            },
        ],
    ),
];

// ---------------------------------------------------------------------------
// Artifact hash verification (pure function — testable without a live engine)
// ---------------------------------------------------------------------------

/// Verify the SHA-256 hash of every pinned artifact of the given `kind` in
/// `install_dir`.
///
/// This is a pure function that takes the installation directory, the current
/// platform triple, a pin table, and the [`ArtifactKind`] to check.
/// Production code passes [`KNOWN_GOOD_HASHES`], calling this once per kind
/// at its own point in `EmbeddedEngine::start()`'s bootstrap sequence (brain
/// #3597 — see the module-level "Hash-pin control" doc section); tests pass
/// an intentionally-wrong table to prove fail-shut without touching the
/// shared `~/.theseus` cache.
///
/// # Errors
///
/// Returns `HashPinError` on the first mismatch found (including a missing
/// file — surfaced as a read error). Returns a distinct `HashPinError` if
/// the platform is not in the table (`expected` names the missing platform)
/// or if the platform has no pin of the requested `kind` (`expected` names
/// the missing kind) — fail-shut, not silent skip either way.
pub fn verify_pinned_artifacts(
    install_dir: &Path,
    platform: &str,
    pins: &[(&str, &[ArtifactPin])],
    kind: ArtifactKind,
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

    let mut checked_any = false;
    for pin in entries.iter().filter(|p| p.kind == kind) {
        checked_any = true;
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

    // Fail-shut on a platform entry that exists but has no pin of this kind
    // (e.g. a future platform added with only one of the two artifacts) —
    // the same "add the missing entry" refusal as an unknown platform,
    // rather than a silent no-op pass.
    if !checked_any {
        return Err(HashPinError {
            artifact: format!("platform:{platform} kind:{kind:?}"),
            expected: "(no pin entry for this artifact kind — add it to KNOWN_GOOD_HASHES)".into(),
            actual: "(unknown)".into(),
        });
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
// Stale-connection guard (Task 2, R-0026 cross-runtime pool reuse)
// ---------------------------------------------------------------------------

/// `before_acquire` hook (paired with `.test_before_acquire(false)`) for
/// `EmbeddedEngine::pool` and `EmbeddedEngine::superuser_pool`.
///
/// # Why this exists
///
/// `EmbeddedEngine` is a `'static` singleton shared across many
/// `#[tokio::test]` functions once acquired through the shared-engine
/// fixture (`tests/common/shared_engine.rs`) — each `#[tokio::test]` gets
/// its OWN, short-lived Tokio runtime. A pooled connection established while
/// one test's runtime was active has its socket registered with THAT
/// runtime's I/O driver; once that runtime is dropped (at the end of the
/// test that created it), the driver is gone and any I/O on the connection
/// from a LATER test's (different) runtime hangs forever rather than
/// erroring — sqlx's pool then reports `PoolTimedOut` only after the full
/// `acquire_timeout` window, even though a fresh connection under the
/// currently-live runtime would work immediately. Empirically confirmed:
/// `postgres_engine.rs`'s `pgvector_extension_is_available_and_creates_successfully`
/// (the second `#[tokio::test]` in the binary to touch `superuser_pool`)
/// hung for the full 30s `acquire_timeout` before this fix.
///
/// `tokio::runtime::Handle::id()` cannot detect this: per `tokio::runtime::Id`'s
/// own documented behavior, ids are reused across NON-overlapping runtimes,
/// and these test runtimes never overlap — an identity comparison would
/// false-positive as "same runtime."
///
/// sqlx's default `test_before_acquire` ping has the SAME hazard (unbounded
/// I/O on a possibly-stale connection, consuming the same `acquire_timeout`
/// budget) — this hook replaces it with a SHORT-timeout-bounded ping: a
/// healthy connection under a live runtime answers near-instantly; a
/// connection whose runtime is gone never answers, so the bound fires fast
/// and sqlx discards + reconnects fresh under the CURRENTLY active (caller's)
/// runtime instead of hanging.
fn ping_within_timeout(
    conn: &mut sqlx::postgres::PgConnection,
    _meta: sqlx::pool::PoolConnectionMetadata,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, sqlx::Error>> + Send + '_>> {
    Box::pin(async move {
        Ok(
            tokio::time::timeout(Duration::from_millis(200), conn.ping())
                .await
                .map(|r| r.is_ok())
                .unwrap_or(false),
        )
    })
}

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

// ---------------------------------------------------------------------------
// Test-fixture provisioning (Task 2, R-0026/R-0027/R-0028)
// ---------------------------------------------------------------------------
//
// The two items below exist so `libs/mnemra-host/tests/common/shared_engine.rs`
// (a separate crate that consumes `mnemra-host` as an ordinary external
// dependency, per `#[path]` inclusion into integration-test binaries) can
// provision per-test databases and tear the shared engine down deterministically.
// `pub(crate)` is unreachable across that crate boundary — R-0027's spec text
// amendment (2026-07-02) widens both to `#[doc(hidden)] pub`, keeping them out
// of the published rustdoc surface while remaining callable from test crates.

/// A fresh, schema-initialized per-test database plus its bound
/// application-role pool.
///
/// Returned by value — owned by the caller (the test), scope-dropped at test
/// end (SO-1). The database itself is NOT dropped: it lives for the binary's
/// lifetime and dies with the `.temporary(true)` engine (R-0027) — there is
/// no per-test `DROP DATABASE`.
#[doc(hidden)]
pub struct TestDatabase {
    pub name: String,
    pub pool: PgPool,
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
            // Bump the PG command/startup timeout from the crate default (5s): under CI
            // concurrency many embedded engines start at once and 5s blows the deadline
            // (DatabaseInitializationError "deadline has elapsed"). Mirrors the
            // acquire_timeout bump below — same concurrent-CI rationale.
            .timeout(Some(Duration::from_secs(60)))
            .temporary(true)
            .build();

        // The bootstrap password is generated by SettingsBuilder::new() and stored
        // in settings.password.  We derive the app-role password from it so it stays
        // within the ephemeral session and requires no separate secret management.
        let app_password = format!("{}_{}", settings.password, APP_ROLE);

        let mut server = PostgreSQL::new(settings);

        // setup() downloads (first run) or reuses cached Postgres binaries from
        // ~/.theseus/postgresql/. This ALWAYS runs initdb internally (mnemra
        // never overrides the default fresh-tempdir data_dir — see the
        // module doc's step 1), which EXECUTES the just-installed postgres
        // binaries before the hash check below ever runs. See the module
        // doc's "Known gaps" and the candidate directions on brain #3616.
        tracing::debug!(
            version = EMBEDDED_PG_VERSION,
            "postgres engine setup: download or cache reuse"
        );
        server
            .setup()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Hash-pin verification, Postgres binary (A-04/A-05, Task 6b, split
        // per brain #3597) — right after setup() returns: this is the exact
        // pinned file setup() just installed or reused. It does not mean
        // "before this binary's first execution" — see the comment above
        // setup() and the module doc's "Known gaps".
        //
        // Covers both the fresh-download path (crate extracts archive → we
        // verify the result) and the warm-cache path (tampered local cache
        // is the realistic threat model).
        //
        // Fail-shut: unknown platform, no pin of the requested kind, or a
        // hash mismatch (including a missing file) → structured error,
        // engine refuses to start. No warn-and-continue mode.
        let install_dir = server.settings().installation_dir.clone();
        tracing::debug!(
            kind = ?ArtifactKind::Postgres,
            site = ArtifactKind::Postgres.verification_site(),
            "verifying pinned artifact"
        );
        verify_pinned_artifacts(
            &install_dir,
            current_platform(),
            KNOWN_GOOD_HASHES,
            ArtifactKind::Postgres,
        )
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Install the pgvector_compiled precompiled extension package, BEFORE
        // start() — no running server is required: postgresql_extensions'
        // install() (crate source, extensions.rs) only shells out to
        // `postgres --version` and `pg_config` (one-shot subprocesses against
        // the installed binaries) and extracts an archive to disk. Moving
        // this ahead of start() means both pinned artifacts are verified
        // before any server process exists (see below).
        //
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

        // Hash-pin verification, pgvector library (A-04/A-05, Task 6b, split
        // per brain #3597) — right after the install-or-skip decision above,
        // still before start() and before any caller can reach
        // `CREATE EXTENSION` or other use of the extension
        // (`ensure_pgvector`/`ensure_extension` are only callable once
        // `start()` has returned `Ok`). Runs unconditionally here, not only
        // inside the `if !control_file.exists()` branch above: a directory
        // where `vector.control` is already present (so the install step is
        // skipped) but the pinned library itself is missing or tampered
        // must still be refused, not silently started against a broken
        // install. Same fail-shut contract as the Postgres binary check
        // above — no warn-and-continue mode.
        tracing::debug!(
            kind = ?ArtifactKind::Pgvector,
            site = ArtifactKind::Pgvector.verification_site(),
            "verifying pinned artifact"
        );
        verify_pinned_artifacts(
            &install_dir,
            current_platform(),
            KNOWN_GOOD_HASHES,
            ArtifactKind::Pgvector,
        )
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // start() assigns an ephemeral port and launches the server. Both
        // pinned artifacts have already passed their hash check above — no
        // server process exists until they have.
        server
            .start()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        let elapsed_ms = t0.elapsed().as_millis() as u64;
        // A-09: emit startup timing so cold-runner regressions are visible.
        // Scope note: this measures from function entry (t0), so it now
        // includes the pgvector install-or-skip step above (moved ahead of
        // start() by this fix-up) as well as setup() and both hash checks —
        // it always included setup() and the Postgres check; the pgvector
        // step is the addition. Still fit for A-09's purpose (surfacing
        // cold-runner regressions in cold-start wall-clock), and arguably
        // more representative of it, since a slow cold pgvector download is
        // exactly the kind of regression a cold runner would show.
        // Migrates to structured OTel emission when Task 25 observability lands.
        tracing::info!(engine_startup_ms = elapsed_ms, "postgres engine started");

        // Create the application database as the bootstrap superuser.
        tracing::debug!(
            db = APP_DB,
            role = APP_ROLE,
            "creating application database and role"
        );
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
            // R-0026: this pool is reused across many #[tokio::test] runtimes
            // via the shared-engine fixture — see ping_within_timeout's doc.
            .test_before_acquire(false)
            .before_acquire(ping_within_timeout)
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
            // R-0026: this pool is reused across many #[tokio::test] runtimes
            // via the shared-engine fixture — see ping_within_timeout's doc.
            .test_before_acquire(false)
            .before_acquire(ping_within_timeout)
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
        self.ensure_extension("vector").await
    }

    /// Enable a named Postgres extension via the superuser path.
    ///
    /// Parameterized so Task 7's `init()` can inject a bogus extension name
    /// for the pgvector-unavailable negative test path (fidelity note: this
    /// exercises the structured-error refusal code, not a genuinely-missing
    /// bundled extension). Production callers use `ensure_pgvector()`.
    pub async fn ensure_extension(&self, extension_name: &str) -> Result<(), ExtensionError> {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE EXTENSION IF NOT EXISTS \"{extension_name}\""
        )))
        .execute(self.superuser_pool.as_ref())
        .await
        .map_err(|e| ExtensionError {
            extension: extension_name.into(),
            cause: e.to_string(),
        })?;
        Ok(())
    }

    /// Create the four least-privilege DB roles (R-0013-e, A-17).
    ///
    /// This is a `pub(crate)` named method on the superuser seam. Task 7's
    /// `init()` calls it; data-path callers cannot access the superuser pool
    /// directly (field is `pub(crate)` but not exposed on this method).
    pub(crate) async fn create_least_privilege_roles(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        crate::schema::init::create_least_privilege_roles(self.superuser_pool.as_ref(), APP_DB)
            .await
    }

    /// Provision one fresh, uniquely-named database on THIS running engine:
    /// `CREATE DATABASE`, enable `vector`, run the same schema-init sequence
    /// [`crate::schema::init::init`] runs for `APP_DB` (migrations, default
    /// workspace, per-type fixture tables, least-privilege roles), grant the
    /// existing `mnemra_app` role the SAME privilege shape already granted on
    /// `APP_DB` (CONNECT + schema USAGE/CREATE — nothing wider, no new
    /// privilege class), and return an app-role pool bound to it.
    ///
    /// The database name is generated INTERNALLY (UUID-derived) — never taken
    /// as caller-supplied text — so two concurrent callers can never collide
    /// and no external string reaches DDL (DDL identifiers cannot be
    /// bind-parameterized, so this is the injection boundary in place of
    /// parameterization).
    ///
    /// `#[doc(hidden)] pub` (not `pub(crate)`): this is called from the
    /// `tests/common/shared_engine.rs` fixture, which compiles as a separate
    /// crate — `pub(crate)` visibility does not cross that boundary.
    ///
    /// # Errors
    ///
    /// Propagates any Postgres/connection error. No partial-provisioning
    /// state is left callable — treat `Err` as "this test's database is
    /// unusable," not as a retryable half-built state.
    #[doc(hidden)]
    pub async fn provision_test_database(
        &self,
    ) -> Result<TestDatabase, Box<dyn Error + Send + Sync>> {
        let db_name = format!("mnemra_test_{}", uuid::Uuid::new_v4().simple());
        let settings = self._server.settings();

        // 1. CREATE DATABASE — reuses the same call start() already makes for
        //    APP_DB.
        self._server
            .create_database(&db_name)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // 2. Short-lived superuser connection scoped to the new database —
        //    NOT stored anywhere; closed before this fn returns.
        //    `settings.url()` bakes in the bootstrap superuser username/
        //    password (Settings::new() defaults `username` to
        //    BOOTSTRAP_SUPERUSER), matching how create_database() itself
        //    connects.
        let su_url = settings.url(&db_name);
        let su_pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(30))
            .connect(&su_url)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // 3. CREATE EXTENSION vector — per-database (R-0027 AC2). Mirrors
        //    ensure_extension's DDL, which can't be reused directly because
        //    it's hardwired to self.superuser_pool == APP_DB.
        sqlx::query(sqlx::AssertSqlSafe(
            "CREATE EXTENSION IF NOT EXISTS \"vector\"".to_string(),
        ))
        .execute(&su_pool)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // 4. Grant mnemra_app the SAME shape already granted on APP_DB
        //    (start(), above) — CONNECT + schema USAGE/CREATE, scoped to the
        //    new db. Same privilege class, different target database — not a
        //    widening.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "GRANT CONNECT ON DATABASE {db_name} TO {APP_ROLE}"
        )))
        .execute(&su_pool)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "GRANT USAGE, CREATE ON SCHEMA public TO {APP_ROLE}"
        )))
        .execute(&su_pool)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // 5. App-role pool bound to the new database — same URL shape start()
        //    builds for APP_DB, substituting db_name. app_password is
        //    RECOMPUTED here (not stored as a new struct field) — same
        //    deterministic derivation start() uses, so no duplicate secret
        //    material lives on EmbeddedEngine.
        let app_password = format!("{}_{}", settings.password, APP_ROLE);
        let app_url = format!(
            "postgresql://{APP_ROLE}:{app_password}@{}:{}/{db_name}",
            settings.host, settings.port,
        );
        let pool = PgPoolOptions::new()
            .max_connections(5) // per-test scale; start()'s shared app_pool uses 20
            .min_connections(1)
            .acquire_timeout(Duration::from_secs(30))
            .connect(&app_url)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // 6. Schema init on the new database — reuse the SAME pool/db-name-
        //    parameterized helpers schema::init::init() itself calls, NOT the
        //    EmbeddedEngine-shaped init() wrapper (hardwired to
        //    self.pool/self.superuser_pool/APP_DB). Zero edits to
        //    schema/init.rs: all four are already pub/pub(crate) and
        //    pool-parameterized.
        crate::schema::migrations::apply(&pool, crate::schema::init::V0_MIGRATIONS)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        sqlx::query(
            "INSERT INTO workspaces (id, name) VALUES ($1, 'default') ON CONFLICT (name) DO NOTHING",
        )
        .bind(crate::schema::init::DEFAULT_WORKSPACE_ID)
        .execute(&pool)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        for type_name in crate::schema::init::FIXTURE_CONTENT_TYPES {
            crate::schema::artifact_table::create_artifact_table(&pool, type_name)
                .await
                .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
            crate::schema::history_trigger::create_history_machinery(&pool, type_name)
                .await
                .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        }
        // create_least_privilege_roles already returns Box<dyn Error + Send +
        // Sync> — no map_err needed, matches the existing
        // EmbeddedEngine::create_least_privilege_roles delegation above.
        crate::schema::init::create_least_privilege_roles(&su_pool, &db_name).await?;

        su_pool.close().await;
        Ok(TestDatabase {
            name: db_name,
            pool,
        })
    }

    /// Explicit, deterministic shutdown of the underlying postmaster — never
    /// relies on `Drop` of a `static` (R-0028: Rust statics never drop).
    ///
    /// Takes `&self` (shared reference), matching
    /// `postgresql_embedded::PostgreSQL::stop(&self)` — so the fixture's
    /// `atexit` callback can call this through the `OnceCell`'s `&'static`
    /// handle with no ownership transfer, no interior mutability.
    ///
    /// `PostgreSQL::stop()` stops the postmaster but does NOT also perform
    /// the `.temporary(true)` data-dir/password-file/socket-dir cleanup that
    /// `PostgreSQL`'s own `Drop` impl does (vendored source:
    /// postgresql-0.20.4/src/postgresql.rs:520-542) — since this engine lives
    /// in a `'static` `OnceCell` and never drops, `Drop` never runs here
    /// either. Unlike the accepted A-12 SIGKILL residual (a rare
    /// interruption case), EVERY graceful exit of every fixture-consuming
    /// binary would leak a full temp Postgres data directory without this —
    /// so the cleanup half is replicated here, best-effort (mirrors Drop's
    /// own `let _ = ...` swallow for the identical operation), with a
    /// `tracing::warn!` on failure rather than a fully silent swallow.
    ///
    /// `#[doc(hidden)] pub`: called from the `tests/common/shared_engine.rs`
    /// fixture, a separate crate — same visibility rationale as
    /// `provision_test_database`.
    #[doc(hidden)]
    pub async fn shutdown(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self._server
            .stop()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        let settings = self._server.settings();
        if settings.temporary {
            // `eprintln!`, not `tracing::warn!` (L1 hardening, #2049): this
            // runs from the `libc::atexit` teardown path
            // (`tests/common/shared_engine.rs`), which has no installed
            // subscriber — `tracing::warn!` there is a silent no-op, so a
            // cleanup failure was invisible. Same rationale as the
            // `FIXTURE_BOOT`/`FIXTURE_TEARDOWN` markers already used on that
            // path. Best-effort cleanup semantics and the `settings.temporary`
            // gate are unchanged — only the logging call.
            if let Err(e) = fs::remove_dir_all(&settings.data_dir) {
                eprintln!(
                    "FIXTURE_TEARDOWN shutdown: failed to remove temp data dir (non-fatal): \
                     path={} error={e}",
                    settings.data_dir.display()
                );
            }
            if let Err(e) = fs::remove_file(&settings.password_file) {
                eprintln!(
                    "FIXTURE_TEARDOWN shutdown: failed to remove password file (non-fatal): \
                     path={} error={e}",
                    settings.password_file.display()
                );
            }
            if let Some(ref socket_dir) = settings.socket_dir
                && let Err(e) = fs::remove_dir_all(socket_dir)
            {
                eprintln!(
                    "FIXTURE_TEARDOWN shutdown: failed to remove socket dir (non-fatal): \
                     path={} error={e}",
                    socket_dir.display()
                );
            }
        }

        Ok(())
    }
}
