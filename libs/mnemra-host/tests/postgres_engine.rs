//! Engine-level tests for the embedded Postgres engine.
//!
//! These tests verify:
//! - **Role shape** (AC#3): the application connection role holds neither
//!   `rolsuper` nor `rolbypassrls`.
//! - **pgvector availability** (AC#1): `pgvector_compiled` is installed so that
//!   `CREATE EXTENSION vector` succeeds and `pg_extension` lists `vector`.
//! - **Shared-engine fixture contract** (R-0026/R-0027, Task 2 RED): the
//!   fixture boots exactly one engine per binary — including under genuinely
//!   concurrent in-process acquisition — and each test's fixture-provisioned
//!   database is isolated from every other test's.
//!
//! Engine acquisition goes through the shared fixture
//! (`tests/common/shared_engine.rs`, `#[path]`-included below) rather than a
//! per-binary startup-serialising mutex: the fixture's own get-or-init
//! semantics are what guarantee exactly-once boot (R-0026), so a second lock
//! here would be vestigial machinery (R-0029).
//!
//! # Phase
//!
//! RED (test-writer split, Glitch sub-run of dispatch task #2049). The
//! `shared_engine` module this file `#[path]`-includes does not exist yet, and
//! `EmbeddedEngine::provision_test_database` / `EmbeddedEngine::shutdown` /
//! `shared_engine::boot_count` are not implemented — so **every** test below
//! fails to compile, not to assert. That is the expected RED: a missing-module
//! / unresolved-symbol compile failure (contract-not-yet-implemented), not a
//! logic or assertion failure. GREEN (a later Forge sub-run) adds
//! `tests/common/shared_engine.rs` and the two new `EmbeddedEngine` methods to
//! turn this suite green without editing any assertion in this file.
//!
//! # Known residual risks
//!
//! **A-12 — temp data-dir leak on SIGKILL:**
//! `SettingsBuilder::new().temporary(true)` auto-cleans the data dir on
//! `PostgreSQL` drop, but SIGKILL bypasses Drop entirely.  On CI (ephemeral
//! runners) leaked dirs are cleaned with the runner.  Local repeated SIGKILL
//! interruptions accumulate dirs under the system temp path.  Low-priority at V0;
//! tracked for a housekeeping follow-up when local-dev ergonomics are prioritised.

#[path = "common/shared_engine.rs"]
mod shared_engine;

use mnemra_host::storage::postgres::engine::{
    ArtifactPin, EmbeddedEngine, KNOWN_GOOD_HASHES, current_platform, verify_pinned_artifacts,
};
use sqlx::Row;
use std::collections::HashSet;
use std::path::Path;
use uuid::Uuid;

/// Verify the application role (`mnemra_app`) is an ordinary role with neither
/// superuser nor BYPASSRLS privileges.
///
/// This assertion binds AC#3 (P-0010 multi-tenancy precondition) at V0.
///
/// Acquisition-migrated onto the shared-engine fixture (R-0030 site ii) —
/// `current_user` resolves to `mnemra_app` regardless of which database the
/// pool is bound to, so this test needs only a live app-role pool, not a
/// `PostgresStorage`. Assertions below are byte-identical to the pre-migration
/// version.
#[tokio::test]
async fn app_role_is_not_superuser_and_not_bypassrls() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;

    // Query pg_roles for the current user's privilege flags.
    let row =
        sqlx::query("SELECT rolsuper, rolbypassrls FROM pg_roles WHERE rolname = current_user")
            .fetch_one(engine.pool.as_ref())
            .await
            .expect("pg_roles query failed");

    let rolsuper: bool = row.get(0);
    let rolbypassrls: bool = row.get(1);

    assert!(
        !rolsuper,
        "mnemra_app must not be a superuser (P-0010 precondition); got rolsuper=true"
    );
    assert!(
        !rolbypassrls,
        "mnemra_app must not have BYPASSRLS (P-0010 precondition); got rolbypassrls=true"
    );
}

// ---------------------------------------------------------------------------
// Hash-pin control tests (A-04/A-05, Task 6b)
// ---------------------------------------------------------------------------
//
// These tests exercise `verify_pinned_artifacts` as a pure function — no live
// engine, no shared `~/.theseus` cache touched — so they are fast and safe to
// run in parallel with other tests.

/// NEGATIVE: A deliberately-wrong expected hash produces a structured
/// `HashPinError` naming the artifact and both digest values.
///
/// Proves the control fails SHUT (structured error, not panic) on a hash
/// mismatch.  The install_dir is the real theseus cache so the file IS
/// readable; only the expected sha256 is wrong.
#[test]
fn hash_pin_mismatch_returns_structured_error_not_panic() {
    // Build a pin table with a known-wrong expected hash.
    // We use the real postgres binary path but a bogus expected digest.
    let wrong_pins: &[(&str, &[ArtifactPin])] = &[(
        current_platform(),
        &[ArtifactPin {
            artifact: "postgres-tampered-test",
            rel_path: "bin/postgres",
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        }],
    )];

    // Locate the real installation directory via HOME env var.
    let home = std::env::var("HOME").unwrap_or_default();
    let install_dir = std::path::PathBuf::from(&home).join(".theseus/postgresql/16.4.0");

    if !install_dir.join("bin/postgres").exists() {
        // Skip on a cold runner that has never run the engine; the positive
        // pgvector_extension_is_available test covers the warm-cache path.
        eprintln!("SKIP hash_pin_mismatch_returns_structured_error_not_panic: cache not warm");
        return;
    }

    let result = verify_pinned_artifacts(&install_dir, current_platform(), wrong_pins);
    assert!(
        result.is_err(),
        "verify_pinned_artifacts must return Err on a hash mismatch, got Ok"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.artifact, "postgres-tampered-test",
        "HashPinError must name the artifact"
    );
    assert_eq!(
        err.expected, "0000000000000000000000000000000000000000000000000000000000000000",
        "HashPinError must carry the expected hash"
    );
    assert!(
        !err.actual.is_empty(),
        "HashPinError must carry the actual hash"
    );
    assert_ne!(
        err.actual, err.expected,
        "actual and expected must differ in the error"
    );
    // The error must be displayable without panic (Display is the structured surface
    // Task 7 builds on).
    let msg = format!("{}", err);
    assert!(
        msg.contains("postgres-tampered-test"),
        "Display output must name the artifact: {msg}"
    );
}

/// NEGATIVE: An unknown platform produces a structured `HashPinError` telling
/// the operator to add a pin — not a silent skip, not a panic.
#[test]
fn hash_pin_unknown_platform_returns_structured_error() {
    let result = verify_pinned_artifacts(
        Path::new("/tmp"),
        "unknown-platform-not-in-table",
        KNOWN_GOOD_HASHES,
    );
    assert!(
        result.is_err(),
        "verify_pinned_artifacts must return Err for an unknown platform, got Ok"
    );
    let err = result.unwrap_err();
    assert!(
        err.artifact.contains("platform:"),
        "HashPinError for unknown platform must name the platform in artifact field: {:?}",
        err.artifact
    );
    assert!(
        err.expected.contains("no pin entry"),
        "HashPinError for unknown platform must say 'no pin entry' in expected: {:?}",
        err.expected
    );
}

/// Verify that `pgvector_compiled` is installed and `CREATE EXTENSION vector`
/// succeeds.  Also checks `pg_extension` lists `vector`.
///
/// Task 7 builds its init-gate (`ensure_pgvector`) on this seam.
///
/// Acquisition-migrated onto the shared-engine fixture (R-0030 site ii) —
/// `engine.ensure_pgvector()`'s `CREATE EXTENSION IF NOT EXISTS` stays
/// meaningful on the shared instance because it is idempotent. Assertions
/// below are byte-identical to the pre-migration version.
#[tokio::test]
async fn pgvector_extension_is_available_and_creates_successfully() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;

    // ensure_pgvector calls CREATE EXTENSION IF NOT EXISTS vector (as superuser)
    // and returns a structured ExtensionError on failure.
    engine
        .ensure_pgvector()
        .await
        .expect("CREATE EXTENSION vector should succeed with bundled pgvector_compiled");

    // Confirm via catalog that the extension is loaded.
    let row: (String,) =
        sqlx::query_as("SELECT extname FROM pg_extension WHERE extname = 'vector'")
            .fetch_one(engine.pool.as_ref())
            .await
            .expect("pg_extension query failed — vector not in catalog");

    assert_eq!(
        row.0, "vector",
        "pg_extension should list 'vector' after CREATE EXTENSION"
    );
}

// ---------------------------------------------------------------------------
// Shared-engine fixture contract tests (R-0026 AC1/AC3, R-0027; Task 2 RED)
// ---------------------------------------------------------------------------
//
// These are the new Glitch-authored RED tests the T2 sub-run adds. They bind
// directly to the fixture API sketch (scratch/tier2-t2-fixture-api-sketch.md
// §1, §2) — `shared_engine::shared_engine()`, `shared_engine::boot_count()`,
// and `EmbeddedEngine::provision_test_database()`.

/// R-0026 AC3 — the fixture's once-semantics are backstopped mechanically: two
/// **genuinely concurrent** in-process acquisitions (joined via `tokio::join!`
/// within one test body, not two sequential `.await`s — intra-process
/// concurrency exists even under `--test-threads 1`, per the spec's own
/// framing of this AC) must both observe the same engine, with exactly one
/// boot. Deliberately NOT `tokio::spawn`-ed onto separate tasks: spawning
/// would additionally require the `shared_engine()` future (and its
/// `&'static EmbeddedEngine` output) to be `Send`, a bound this RED phase has
/// no way to verify (compilation halts at the unresolved `mod
/// shared_engine;` before any test body is type-checked) and GREEN — forbid-
/// scoped from editing test files — would have no way to repair if it didn't
/// hold. Plain `tokio::join!` of two un-spawned futures needs no such bound
/// and satisfies this AC identically.
///
/// The `boot_count() == 1` assertion is the load-bearing one — pointer
/// equality on the two `&'static` returns only proves "same instance" (an
/// `OnceCell` implementation detail), not "booted exactly once". `boot_count`
/// is an independent `AtomicUsize` bumped once inside the fixture's init
/// closure, so it is not a proxy (sketch §6.2).
#[tokio::test]
async fn shared_engine_concurrent_acquisition_boots_exactly_once() {
    // Given: no assumption about acquisition order — two callers race.
    // When: both acquire the shared engine concurrently, polled together in
    // one test body (not two sequential calls).
    let (engine_a, engine_b) = tokio::join!(
        shared_engine::shared_engine(),
        shared_engine::shared_engine(),
    );

    // Then: both observe the SAME `&'static EmbeddedEngine` instance...
    assert!(
        std::ptr::eq(engine_a, engine_b),
        "concurrent shared_engine() callers must observe the same engine instance"
    );

    // ...AND the engine booted exactly once — independent of how many other
    // tests in this binary have already called shared_engine(), since the
    // fixture boots at most once for the whole binary run.
    assert_eq!(
        shared_engine::boot_count(),
        1,
        "engine must boot exactly once even when two callers race concurrently"
    );
}

/// R-0026 mechanical — a second, sequential `shared_engine()` call reuses the
/// already-booted engine: same instance, and `boot_count()` stays at 1.
#[tokio::test]
async fn shared_engine_second_sequential_call_reuses_and_boot_count_stays_one() {
    // Given: the engine has already been acquired at least once in this
    // binary (by this test or an earlier one — the fixture is a process-wide
    // singleton, so a fresh acquisition here still exercises "get" not "init"
    // whenever it isn't the very first call of the run).
    // When: shared_engine() is called twice, sequentially.
    let first = shared_engine::shared_engine().await;
    let second = shared_engine::shared_engine().await;

    // Then: both calls return the same engine instance...
    assert!(
        std::ptr::eq(first, second),
        "sequential shared_engine() calls must return the same engine instance"
    );
    // ...and the reuse did not trigger a second boot.
    assert_eq!(
        shared_engine::boot_count(),
        1,
        "a second sequential acquisition must not trigger a second boot"
    );
}

/// R-0027 / Scenario S2 — two tests' provisioned databases are isolated: they
/// have distinct names, and a row written through one's pool is invisible
/// through the other's (and vice versa — checked both directions).
///
/// Uses the `workspaces` table (schema-init creates it and seeds a `default`
/// row per provisioned database — sketch §2 step 6), since it is guaranteed
/// to exist after fixture provisioning and has a `name` column with a unique
/// constraint, ample for a distinctly-named marker row.
#[tokio::test]
async fn provisioned_databases_are_isolated_from_each_other() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;

    // Given: two tests' worth of fixture-provisioned databases...
    let db_a = engine
        .provision_test_database()
        .await
        .expect("provision_test_database (db A) should succeed");
    let db_b = engine
        .provision_test_database()
        .await
        .expect("provision_test_database (db B) should succeed");

    assert_ne!(
        db_a.name, db_b.name,
        "two provisioned test databases must have distinct names"
    );

    // When: test A writes and commits a row through its own pool...
    let marker_name = format!("isolation-marker-{}", Uuid::new_v4());
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(Uuid::new_v4())
        .bind(&marker_name)
        .execute(&db_a.pool)
        .await
        .expect("insert into db A's workspaces should succeed");

    // Then: test B — reading through its own pool — observes none of test A's
    // rows...
    let seen_in_b: Option<(String,)> =
        sqlx::query_as("SELECT name FROM workspaces WHERE name = $1")
            .bind(&marker_name)
            .fetch_optional(&db_b.pool)
            .await
            .expect("select from db B's workspaces should succeed");
    assert!(
        seen_in_b.is_none(),
        "a row written to db A must be invisible through db B's pool"
    );

    // ...and, checked in the reverse direction, db A DOES observe the row it
    // wrote itself (the isolation is database-scoped, not a broken write).
    let seen_in_a: Option<(String,)> =
        sqlx::query_as("SELECT name FROM workspaces WHERE name = $1")
            .bind(&marker_name)
            .fetch_optional(&db_a.pool)
            .await
            .expect("select from db A's workspaces should succeed");
    assert_eq!(
        seen_in_a.map(|(n,)| n),
        Some(marker_name),
        "db A must observe the row it wrote to its own database"
    );
}

/// R-0027 mechanical — N (>= 3) provisions on the shared engine yield N
/// pairwise-distinct database names.
#[tokio::test]
async fn provisioned_databases_are_pairwise_distinct_across_n_provisions() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;

    let mut names: HashSet<String> = HashSet::new();
    for _ in 0..3 {
        let db = engine
            .provision_test_database()
            .await
            .expect("provision_test_database should succeed");
        assert!(
            names.insert(db.name.clone()),
            "provisioned database name {} was not unique across provisions",
            db.name
        );
    }
    assert_eq!(
        names.len(),
        3,
        "expected 3 pairwise-distinct provisioned database names"
    );
}

/// R-0027 AC2 — a freshly-provisioned database has the `vector` extension
/// available (schema-init runs per-database `CREATE EXTENSION vector` —
/// sketch §2 step 3), queried through the provisioned database's own pool.
#[tokio::test]
async fn provisioned_database_has_vector_extension_available() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;

    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");

    let row: (String,) =
        sqlx::query_as("SELECT extname FROM pg_extension WHERE extname = 'vector'")
            .fetch_one(&db.pool)
            .await
            .expect("pg_extension query failed on provisioned database — vector not in catalog");

    assert_eq!(
        row.0, "vector",
        "provisioned database's pg_extension should list 'vector'"
    );
}
