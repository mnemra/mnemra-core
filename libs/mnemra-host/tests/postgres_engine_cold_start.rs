//! Cold-start acceptance tests for `EmbeddedEngine::start()` (brain #3597).
//!
//! # The requirement
//!
//! The embedded engine must start successfully from an install directory
//! that has never held Postgres binaries or pgvector artifacts (a machine
//! whose `~/.theseus` cache is empty or has expired), and the hash-pin
//! control must still verify BOTH pinned artifacts before `start()` launches
//! the server — reordering, not weakening or skipping, is the only
//! acceptable fix shape. (This is narrower than "before first use": `initdb`
//! runs the Postgres binary inside `setup()`, before any check; see
//! `engine.rs`'s "Known gaps" and brain #3616.)
//!
//! # The defect (fixed — brain #3597, commits cf4f88d + 86b8227)
//!
//! `EmbeddedEngine::start()` (`libs/mnemra-host/storage/postgres/engine.rs`)
//! used to call a single combined `verify_pinned_artifacts()` for BOTH the
//! Postgres binary and the pgvector shared library right after `setup()` —
//! but the pgvector library was only written later, by `install_extension()`,
//! guarded by a `vector.control`-exists check. On a cold install directory
//! this made `start()` fail with `HashPinError { actual: "(read error: No
//! such file or directory ...)" }` for the pgvector artifact and never reach
//! the install step. Production host startup
//! (`libs/mnemra-host/mnemra_host.rs:555`) calls the exact same `start()`,
//! so this affected any fresh machine, not only CI.
//!
//! The fix splits the single call into two, each keyed by a new
//! [`ArtifactKind`] and run at its own point in the bootstrap sequence: the
//! Postgres binary right after `setup()`, the pgvector library right after
//! the install-or-skip decision — both still before `start()` launches a
//! server process. See `engine.rs`'s own module doc for the full sequence.
//!
//! # Sanctioned reads (declared per Glitch convention)
//!
//! - `libs/mnemra-host/storage/postgres/engine.rs`, READ IN FULL against the
//!   fixed code (brain #3597, commits cf4f88d + 86b8227) for this round:
//!   the new `ArtifactKind` enum and its `verification_site` exhaustive
//!   match; `ArtifactPin::kind`; `verify_pinned_artifacts()`'s new `kind`
//!   parameter and per-kind filter; and, specifically for the Warden
//!   findings answered below, the EXACT call-site shape of both
//!   `verify_pinned_artifacts()` calls in `start()` — confirming the
//!   pgvector call runs UNCONDITIONALLY after the install-or-skip decision
//!   (not nested inside the install-taken branch) and that the Postgres call
//!   still exists, unconditionally, right after `setup()`. `HashPinError`'s
//!   fields and `Error` impl (used below only via `downcast_ref` on the
//!   `Box<dyn Error>` `start()` already returns — never called directly).
//!   `KNOWN_GOOD_HASHES` / `ArtifactPin` (a `pub const` the file's own doc
//!   comment says exists "so that tests can construct deliberate-mismatch
//!   entries" — used here read-only, to look up each pin's `rel_path` and
//!   `artifact` name by `kind` rather than hardcoding a second copy of those
//!   strings). `current_platform()` (already `pub`).
//! - `~/.cargo/registry/.../postgresql_embedded-0.20.4/src/settings.rs`:
//!   confirms `Settings::new()` resolves `installation_dir` from
//!   `std::env::home_dir()` (i.e. `$HOME` on Unix) — this is the ONLY
//!   isolation seam available, since `EmbeddedEngine::start()` takes no
//!   parameters and hardcodes `SettingsBuilder::new()`. Empirically
//!   confirmed with a throwaway probe test before writing this file (see the
//!   dispatch report) — not merely inferred from source.
//! - `~/.cargo/registry/.../postgresql_embedded-0.20.4/src/postgresql.rs`:
//!   `PostgreSQL::install()` short-circuits on `installation_dir.exists()`
//!   (line 218) — a plain existence check, not a hash or provenance check.
//!   This is why every mutation scenario below (S2/S3/S4) works from a COPY
//!   of a once-built template install: the crate cannot tell a copied tree
//!   from one it installed itself, so it skips re-download either way. It
//!   is also why a synthetic (not-really-installed) directory doesn't work:
//!   a `bin/postgres` that is anything other than the genuine binary makes
//!   `initdb` fail (a wrong-reason red) before `verify_pinned_artifacts` is
//!   ever reached — confirmed empirically against a scratch copy, not
//!   merely reasoned (see the dispatch report).
//! - `libs/mnemra-host/tests/postgres_engine.rs` and
//!   `libs/mnemra-host/tests/common/shared_engine.rs`: existing conventions
//!   (imports, `HashPinError`/`verify_pinned_artifacts` pure-function tests
//!   already present — this file does NOT duplicate those; it exercises
//!   `EmbeddedEngine::start()` end-to-end instead, which the existing suite
//!   does not). This file deliberately does NOT `#[path]`-include
//!   `common/shared_engine.rs`: that fixture's `OnceCell` boots one engine
//!   per binary against the REAL (unoverridden) `HOME`, which is exactly the
//!   cache this file must never touch.
//! - `libs/mnemra-host/auth/token.rs` (`#[cfg(test)] mod tests`, lines
//!   ~509-575): read only for the `ENV_LOCK` + `EnvVarGuard` convention this
//!   project already uses to serialize process-env-mutating tests. Not
//!   modified, not imported (it is a private module) — the `ENV_LOCK`/
//!   `HomeGuard` below are a fresh implementation of the same pattern,
//!   scoped to `HOME` specifically.
//! - `justfile`: `PG_TEST_FLAGS` / `verify-test` / `verify-smoke` /
//!   `_ci-verify-chain` / `_ci-verify-chain-nocoverage` /
//!   `verify-coverage-membership` / `verify-cold-start`, to decide how this
//!   binary is wired into CI. NOT edited this round — Bolt owns the
//!   `justfile` and `.github/workflows/ci.yml` concurrently in this same
//!   worktree (dispatch coordination, brain #3597); the new scenario count
//!   is reported instead of self-applied to the count-pin.
//!
//! # What S1 alone cannot cover, what S3 closes, and what remains reading-only
//!
//! No black-box test can distinguish a correct reorder-only fix from a "skip
//! verification when the pgvector file is absent" fix on the PRIMARY
//! cold-start scenario (S1) alone: both make a genuinely cold start succeed,
//! because the real downloaded pgvector artifact legitimately matches its
//! pin either way. S2 (tamper) does not close this gap either — a
//! skip-if-absent fix does not special-case "present but wrong".
//!
//! S3 closes it via a different file-system state that neither S1 nor S2
//! reach: `install_extension()` is skipped whenever `vector.control` already
//! exists (`engine.rs`'s own guard, read — see Sanctioned reads above), so a
//! directory with genuine Postgres binaries, a present `vector.control`, but
//! an ABSENT pgvector library is a state a correct reorder-only fix must
//! still refuse. **S3 closes the skip-if-absent shape specifically — this is
//! not a general proof that S1+S2+S3 discriminate every conceivable wrong
//! fix.** Warden's branch review (dispatch #3597, condition C3) named two
//! further wrong fixes that still pass S1, S2, and S3 unchanged:
//!
//! (a) **Verify pgvector ONLY when the install step was SKIPPED** — i.e.
//!     only when `vector.control` already existed, never right after a
//!     fresh `install_extension(...)`. S1 cold-installs, so under this wrong
//!     fix nothing is verified; but S1 only asserts that a genuine cold
//!     start succeeds, and a genuine download would have matched its pin
//!     anyway, so S1 stays green. S2, S3 and S4 all start from a copy where
//!     the install step is skipped, so verification still runs and their
//!     refusals still happen. The consequence: a freshly downloaded pgvector
//!     library (portalcorp publishes no upstream hash) would never be
//!     verified. NOT closable black-box: it needs a cold install whose
//!     downloaded pgvector differs from its pin, i.e. a seam to inject the
//!     install stage, which the current API does not expose. Documented
//!     only, per the dispatch #3597 advisory decision — the seam is not
//!     built. (The opposite wrong fix, verifying ONLY when the install step
//!     was taken, IS caught: S2 and S3 skip the install and would then see
//!     no refusal.)
//! (b) **Delete the Postgres-kind verification call entirely.** No scenario
//!     among S1/S2/S3 tampers `bin/postgres` — all three either use a
//!     genuine binary or never touch it. **S4 closes this one**: it
//!     tampers a copy of the Postgres binary specifically and asserts the
//!     refusal names the Postgres artifact.
//!
//! **The shipped fix (`engine.rs` at brain #3597's resolution, commits
//! cf4f88d + 86b8227) was verified correct on both (a) and (b) BY READING
//! THE CODE, not by a test** (see Sanctioned reads above for exactly what
//! was read): the pgvector verify call (`ArtifactKind::Pgvector`) runs
//! unconditionally after the install-or-skip decision, conditioned on
//! neither branch — `engine.rs`'s own comment states this
//! explicitly ("Runs unconditionally here, not only inside the
//! `if !control_file.exists()` branch above") — which rules out (a) as
//! shipped, though nothing MECHANICALLY re-checks that claim on a future
//! change (hence (a) staying open as a documented, not built, gap). The
//! Postgres-kind verify call (`ArtifactKind::Postgres`) still exists,
//! unconditionally, right after `setup()` — this rules out (b) as shipped.
//! S4 goes further than the reading did: it converts (b) from a one-time
//! reading into an ONGOING black-box regression check that would catch a
//! future removal of that call, not just today's presence of it.
//!
//! # Isolation (HARD CONSTRAINT #1)
//!
//! `postgresql_embedded::Settings::new()` resolves `installation_dir` from
//! `std::env::home_dir()` — there is no other seam, since
//! `EmbeddedEngine::start()` hardcodes `SettingsBuilder::new()` with no
//! caller-supplied `installation_dir`. Every test below therefore overrides
//! the process-global `HOME` env var for a fresh `tempfile::tempdir()`, and:
//!
//! - Never reads, deletes, moves or modifies anything under the REAL
//!   `~/.theseus` — every test's install tree traces back to ONE isolated
//!   template install, built once per binary run in its own isolated
//!   directory (see "Shared install template" below), never the real cache.
//! - Serializes all HOME-mutating tests in this binary behind `ENV_LOCK`,
//!   held for the whole test body (mirrors the `auth::token` convention).
//! - This is its own dedicated test binary specifically so this
//!   serialization never has to extend to `tests/postgres_engine.rs`, whose
//!   shared-engine tests boot against the REAL `HOME` — a process-wide HOME
//!   override in a shared binary would silently redirect them.
//! - `HomeGuard::install` MECHANICALLY asserts (not merely assumes) that
//!   `postgresql_embedded`'s own settings resolution actually followed the
//!   override, immediately after setting it — the isolation guarantee is
//!   checked every run, not just in a one-off manual probe. FIXED this
//!   round (Warden finding): the guard is now constructed BEFORE the
//!   assertion runs, not after — see `HomeGuard::install`'s own doc for why
//!   the old order broke the guard's own "restores on panic" contract.
//!
//! # Shared install template (network-cost reduction, dispatch #3597
//! coordination — Warden advisory, decided and recorded)
//!
//! Every mutation scenario (S2/S3/S4) needs a REAL, genuinely-installed tree
//! to copy and mutate — a synthetic one breaks `initdb` for the wrong reason
//! (see Sanctioned reads above). Each of S2/S3/S4 doing its OWN independent
//! cold install (the shape S2/S3 used in the prior round) would mean 4
//! independent cold installs per full run (S1 plus three), at ~6
//! `api.github.com` calls apiece (Bolt's measurement) against a 60/hr
//! unauthenticated limit — ~24 calls/run, growing with every future
//! scenario added this way.
//!
//! **Decision: S2, S3, and S4 all copy ONE template install, built once per
//! binary run by whichever of the three runs first** (`template_install_dir`,
//! below — a `tokio::sync::OnceCell<TempDir>` get-or-init, same idiom as
//! `tests/common/shared_engine.rs`'s engine singleton). On a passing run
//! this caps network cost at 2 cold installs (S1's own, always independent
//! since its entire point is a directory that has NEVER held anything — it
//! cannot use a template; plus ONE shared template) regardless of how many
//! mutation scenarios exist. On a FAILING template build the cap does not
//! hold: a panicking `get_or_init` leaves the cell empty, so each later
//! scenario retries the cold install (up to 4 in total), and a rate-limit
//! failure cascades.
//!
//! **Tradeoff against test independence, recorded:** S2/S3/S4 no longer each
//! independently prove a fresh cold install can produce the artifacts they
//! mutate — they inherit that proof from whichever one triggers the
//! template build. This does NOT create an ordering dependency between
//! S2/S3/S4 themselves (`OnceCell::get_or_init` means whichever runs first
//! pays the build cost; the other two transparently reuse it regardless of
//! which one that is — swapping declaration order changes nothing). It DOES
//! mean a defect specific to the template-build path would surface
//! identically in whichever of the three happens to run first, rather than
//! independently in all three — accepted, since S1 already independently
//! proves the underlying cold-install path on every run, and all three
//! scenarios already shared that exact code path even before this round's
//! restructuring.

use mnemra_host::storage::postgres::engine::{
    ArtifactKind, ArtifactPin, EmbeddedEngine, HashPinError, KNOWN_GOOD_HASHES, current_platform,
};
use std::path::Path;
use std::sync::LazyLock;
use tokio::sync::{Mutex, OnceCell};

// ---------------------------------------------------------------------------
// HOME isolation harness
// ---------------------------------------------------------------------------

/// Serializes every test in this binary that overrides the process-global
/// `HOME` env var. `std::env::set_var`/`remove_var` mutate process state
/// shared by every thread in this process; holding this lock for a whole
/// test body (mirrors `libs/mnemra-host/auth/token.rs`'s `ENV_LOCK`
/// convention) is what makes that safe under this binary's
/// `--test-threads 1` invocation (see the `verify-cold-start` justfile
/// recipe) — belt-and-suspenders, since `--test-threads 1` already means no
/// two test bodies run concurrently. `tokio::sync::Mutex` (not
/// `std::sync::Mutex`) specifically because test bodies below hold this
/// lock across `.await` points — an async-aware mutex is the correct
/// primitive there, not a `#[allow]` on `clippy::await_holding_lock`.
static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// RAII guard: overrides `HOME` for the guard's lifetime, restoring the
/// prior value (or unsetting it if it was unset) on drop — including on
/// panic, so a failed assertion never leaks an overridden `HOME` into
/// whatever test runs next in this binary.
struct HomeGuard {
    prev: Option<String>,
}

impl HomeGuard {
    /// Point `HOME` at `dir` and MECHANICALLY verify (not assume) that
    /// `postgresql_embedded`'s own settings resolution followed — panics
    /// otherwise, since continuing would risk touching the real
    /// `~/.theseus` cache.
    ///
    /// FIXED this round (Warden finding): `Self` is constructed BEFORE the
    /// assertion runs, not after. The old order called `set_var` then
    /// asserted before building `Self` — if the assertion fired, `Self`
    /// never existed, so `Drop` never ran and `HOME` was never restored,
    /// directly contradicting this type's own "including on panic" doc
    /// above. Constructing the guard first means the assertion panics
    /// INSIDE an already-live guard's scope, so unwinding still drops it
    /// and still restores `HOME`.
    fn install(dir: &Path) -> Self {
        let prev = std::env::var("HOME").ok();
        // SAFETY: serialized by `ENV_LOCK`, held by the caller for the whole
        // test body — no other thread in this process reads or writes HOME
        // while this override is live.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("HOME", dir);
        }
        let guard = Self { prev };
        let resolved = postgresql_embedded::SettingsBuilder::new()
            .build()
            .installation_dir;
        assert!(
            resolved.starts_with(dir),
            "isolation guarantee failed: postgresql_embedded resolved installation_dir to \
             {resolved:?}, which is NOT under the overridden HOME {dir:?} — refusing to \
             proceed, since continuing could touch the real ~/.theseus cache"
        );
        guard
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        // SAFETY: see `install` — restoration runs under the same
        // serialization (ENV_LOCK outlives this guard within each test).
        #[allow(unsafe_code)]
        unsafe {
            match &self.prev {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
        }
    }
}

/// Looks up the `ArtifactPin` of the given `kind` for the current platform
/// directly from the real `KNOWN_GOOD_HASHES` table (a sanctioned read — see
/// the file header) so this file never hardcodes a second copy of an
/// artifact's name or relative path; if a pin changes, every assertion built
/// on this helper tracks that change for free.
///
/// FIXED this round (Warden finding): looks up by the real `kind` field
/// (`ArtifactPin::kind`, wired into `verify_pinned_artifacts()` itself) —
/// not by a string-prefix guess (`artifact.starts_with("pgvector-")`) that
/// duplicated, in a test, a naming convention only the production pin table
/// actually enforces.
fn pin_of_kind_for_current_platform(kind: ArtifactKind) -> &'static ArtifactPin {
    KNOWN_GOOD_HASHES
        .iter()
        .find(|(platform, _)| *platform == current_platform())
        .and_then(|(_, pins)| pins.iter().find(|p| p.kind == kind))
        .unwrap_or_else(|| {
            panic!(
                "no {kind:?} pin entry in KNOWN_GOOD_HASHES for platform {:?} — this test \
                 needs KNOWN_GOOD_HASHES extended before it can run on this CI target",
                current_platform()
            )
        })
}

// ---------------------------------------------------------------------------
// Shared install template — see the file header's "Shared install template"
// section for the network-cost tradeoff this records.
// ---------------------------------------------------------------------------

/// Holds the one-time-built template install tree alive for this binary's
/// whole process lifetime, so every `PathBuf` handed out by
/// `template_install_dir()` stays valid for every later copy. Statics are
/// never dropped, so this `TempDir` is NEVER deleted: each run leaves one full
/// Postgres + pgvector install tree in `$TMPDIR` (known leak, tracked as
/// brain #3632 together with the retry cascade on a failing template build).
static TEMPLATE_HOME: LazyLock<OnceCell<tempfile::TempDir>> = LazyLock::new(OnceCell::new);

/// Get-or-build the ONE shared template install directory this binary's
/// mutation scenarios (S2/S3/S4) copy from.
///
/// # Caller contract
///
/// The caller MUST already hold `ENV_LOCK` for its whole test body before
/// calling this — it overrides `HOME` internally (to build the template, if
/// not already built) and restores it before returning, but that internal
/// override is not itself synchronized against a second HOME-mutating test
/// running concurrently in this process.
///
/// # Panics
///
/// Panics if the template engine fails to start — matches
/// `tests/common/shared_engine.rs`'s own documented rationale: there is no
/// meaningful fallback for a test binary whose template install never came
/// up.
async fn template_install_dir() -> std::path::PathBuf {
    let temp_dir = TEMPLATE_HOME
        .get_or_init(|| async {
            let home = tempfile::tempdir().expect("tempdir for the shared template HOME");
            {
                // Scoped so the guard drops (restoring HOME) before this
                // closure returns `home` to the OnceCell — the calling
                // test installs its OWN HomeGuard afterward, so only one
                // guard is ever live at a time; no nested overrides.
                let _guard = HomeGuard::install(home.path());
                let engine = EmbeddedEngine::start().await.expect(
                    "template cold start must succeed to build the shared install tree (the \
                     error below says whether it was a hash-pin refusal or a download failure)",
                );
                engine
                    .shutdown()
                    .await
                    .expect("template engine shutdown must succeed");
            }
            home
        })
        .await;
    temp_dir.path().to_path_buf()
}

/// Copies `src_home`'s `.theseus` tree into `dest_home/.theseus`, so a
/// mutation scenario gets its OWN isolated copy to modify without touching
/// the template or any other scenario's copy. Shells out to `cp -R`
/// (test-only helper) rather than reimplementing a recursive copier —
/// correct permission/symlink preservation for free, and the exact command
/// this file's own dispatch-round probe already validated produces the
/// expected `dest_home/.theseus/postgresql/16.4.0/...` shape.
fn copy_theseus_tree(src_home: &Path, dest_home: &Path) {
    let src = src_home.join(".theseus");
    let dest = dest_home.join(".theseus");
    assert!(
        src.exists(),
        "test setup bug: template source {src:?} must exist before copying"
    );
    assert!(
        !dest.exists(),
        "test setup bug: destination {dest:?} must not already exist before copying"
    );
    let status = std::process::Command::new("cp")
        .arg("-R")
        .arg(&src)
        .arg(&dest)
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn `cp -R` {src:?} -> {dest:?}: {e}"));
    assert!(
        status.success(),
        "`cp -R` {src:?} -> {dest:?} failed with status {status:?}"
    );
}

// ---------------------------------------------------------------------------
// S1 — the primary, required scenario (#3597)
// ---------------------------------------------------------------------------

/// Given an install directory that has never held Postgres binaries or
/// pgvector artifacts, when `EmbeddedEngine::start()` runs, then it must
/// succeed — and the resulting engine must be genuinely usable: a live SQL
/// round trip through the app-role pool, and `CREATE EXTENSION vector`
/// succeeding through the superuser pool (proving the pgvector library was
/// not merely downloaded but is actually loadable by Postgres).
///
/// Deliberately NEVER uses the shared template (see the file header): its
/// entire point is a directory that has NEVER held any artifacts, so it is
/// always an independent cold install, on every run.
///
/// THE POSITIVE CONTROL for this suite: on the code as it stood when brain
/// #3597 was filed, this test failed with a `HashPinError` naming the
/// pgvector artifact and an `actual` of `(read error: No such file or
/// directory (os error 2))` — the defect itself, not a network failure, a
/// timeout, or a compile error. See the dispatch report for the exact
/// quoted failure, and for confirmation this now passes for real on the
/// fixed code (brain #3597, commits cf4f88d + 86b8227).
#[tokio::test]
async fn cold_start_succeeds_with_no_prior_theseus_cache() {
    let _serialize = ENV_LOCK.lock().await;
    let home = tempfile::tempdir().expect("tempdir for isolated HOME");
    let _home_guard = HomeGuard::install(home.path());

    // Given: nothing under <home>/.theseus at all.
    assert!(
        !home.path().join(".theseus").exists(),
        "test setup bug: isolated HOME must start with no .theseus directory"
    );

    // When: the engine starts from this genuinely empty install directory.
    let start_result = EmbeddedEngine::start().await;

    // Then: it must succeed — not refuse with a HashPinError read-error on
    // the pgvector artifact (the #3597 defect: verify_pinned_artifacts()
    // checks the pgvector artifact before install_extension() has ever
    // written it).
    let engine = match start_result {
        Ok(engine) => engine,
        Err(e) => {
            if let Some(hash_err) = e.downcast_ref::<HashPinError>() {
                panic!(
                    "cold start must succeed from an empty install directory, but start() \
                     refused with a HashPinError — artifact={:?} expected={:?} actual={:?}. \
                     This is the #3597 defect: verify_pinned_artifacts() checks the pgvector \
                     artifact before install_extension() has written it.",
                    hash_err.artifact, hash_err.expected, hash_err.actual
                );
            }
            panic!(
                "cold start must succeed from an empty install directory, got a non-hash-pin \
                 error instead: {e}"
            );
        }
    };

    // And: both artifacts are not merely present but genuinely usable — a
    // live query through the pool that was hash-verified...
    engine
        .ensure_pgvector()
        .await
        .expect("CREATE EXTENSION vector must succeed after a cold pgvector install");

    // ...confirmed via the catalog, through the APP-ROLE pool specifically
    // (a different connection than ensure_pgvector's superuser path used —
    // proves the app-role pool is independently live too).
    let (extname,): (String,) =
        sqlx::query_as("SELECT extname FROM pg_extension WHERE extname = 'vector'")
            .fetch_one(engine.pool.as_ref())
            .await
            .expect("pg_extension query failed — vector not in catalog after cold install");
    assert_eq!(
        extname, "vector",
        "pg_extension must list 'vector' after a cold-started engine's CREATE EXTENSION"
    );

    engine
        .shutdown()
        .await
        .expect("engine shutdown must succeed");
}

// ---------------------------------------------------------------------------
// S2 — tampered pgvector is refused, isolated from the real cache entirely
// (HARD CONSTRAINT: "if you can also assert ... do")
// ---------------------------------------------------------------------------

/// Given a copy of the shared template install (real, correctly-hashed
/// artifacts — see the file header's "Shared install template" section) in
/// its own isolated directory, when the copied pgvector library is
/// corrupted and `EmbeddedEngine::start()` runs against that copy, then it
/// must refuse: `Err`, downcasting to `HashPinError`, naming the pgvector
/// artifact specifically, with a genuine hash MISMATCH (`actual !=
/// expected`, and NOT a read-error string) — proving the hash-pin control
/// still runs its real per-file comparison against the pgvector artifact
/// and does not silently accept a compromised cache.
///
/// RESTRUCTURED this round (dispatch #3597 coordination, network-cost
/// advisory): previously did its own independent cold install, then a
/// SECOND `start()` call against the same (now-tampered) directory. Now
/// copies the shared template instead of installing independently — same
/// underlying mechanism (`PostgreSQL::install()`'s `installation_dir.exists()`
/// short-circuit doesn't care whether the directory got there via a live
/// crate install or a `cp -R`, see Sanctioned reads), one fewer `start()`
/// call, no second network-cost contribution.
#[tokio::test]
async fn tampered_pgvector_library_is_refused_in_isolated_install_dir() {
    let _serialize = ENV_LOCK.lock().await;

    let template = template_install_dir().await;
    let home = tempfile::tempdir().expect("tempdir for isolated HOME");
    copy_theseus_tree(&template, home.path());
    let _home_guard = HomeGuard::install(home.path());

    // Given: a copy of the template's genuine, correctly-hashed pgvector
    // library...
    let pin = pin_of_kind_for_current_platform(ArtifactKind::Pgvector);
    let install_dir = home.path().join(".theseus/postgresql/16.4.0");
    let pgvector_path = install_dir.join(pin.rel_path);
    let before = std::fs::read(&pgvector_path).expect("read the copied pgvector library");
    assert!(
        !before.is_empty(),
        "test setup bug: copied pgvector library must be non-empty before tampering"
    );

    // When: it is corrupted...
    std::fs::write(&pgvector_path, b"TAMPERED-NOT-THE-REAL-PGVECTOR-LIBRARY")
        .expect("tamper the copied pgvector library");

    // ...and an engine attempts to start against this isolated copy.
    let start_result = EmbeddedEngine::start().await;

    // Then: refused, not silently accepted.
    //
    // `EmbeddedEngine` does not implement `Debug` (no test-only Debug derive
    // on a production type), so this is a `match` rather than `expect_err`.
    let err = match start_result {
        Ok(_) => panic!(
            "start() must refuse when the installed pgvector library no longer matches its \
             pinned hash — a tampered cache must never be silently accepted, but start() \
             succeeded"
        ),
        Err(e) => e,
    };
    let hash_err = err.downcast_ref::<HashPinError>().unwrap_or_else(|| {
        panic!("expected a hash-pin refusal, got a different error type: {err}")
    });

    assert_eq!(
        hash_err.artifact, pin.artifact,
        "HashPinError must name the pgvector artifact specifically, not the postgres binary \
         or a different platform's pin"
    );
    assert_ne!(
        hash_err.actual, hash_err.expected,
        "must be a genuine hash mismatch, not a coincidental equal value"
    );
    assert!(
        !hash_err.actual.contains("read error"),
        "expected a hash MISMATCH on the tampered (but present) file, not a read-error/absence \
         (actual={:?}) — a read-error here would mean the file was unexpectedly missing, not \
         that tamper detection ran against its bytes",
        hash_err.actual
    );
}

// ---------------------------------------------------------------------------
// S3 — a partial install (control file present, library absent) is refused,
// not started. Closes the "skip when absent" shape of the gap S1 alone
// leaves open (dispatcher decision, brain #3597: a stderr verification-event
// observable was considered and rejected in favor of this scenario, which
// needs no new production surface).
// ---------------------------------------------------------------------------

/// Given a copy of the shared template install (genuine Postgres binaries,
/// genuine `vector.control`) with the pinned pgvector shared library
/// removed from the copy, when `EmbeddedEngine::start()` runs against that
/// copy, then it must refuse: `Err`, downcasting to `HashPinError`, naming
/// the pgvector artifact.
///
/// # Why this state, specifically
///
/// `install_extension()` is skipped whenever `vector.control` already exists
/// (`engine.rs`'s own guard: `if !control_file.exists() { install_extension
/// (...) }`). So on a CORRECT reorder-only fix, verification still runs
/// after that install-or-skip decision and refuses on the missing file — but
/// a "skip verification when absent" fix, or a fix that only verifies
/// pgvector INSIDE the install-taken branch (so it never runs when the
/// branch is skipped, exactly as it is here), lets `start()` succeed despite
/// a broken/missing pgvector library.
///
/// # What this scenario closes, and what it does not (Warden finding, C3)
///
/// This scenario closes the "skip verification when the file is absent"
/// wrong-fix shape SPECIFICALLY — it is not a general proof that S1+S2+S3
/// discriminate every conceivable wrong fix. See the file header's "What S1
/// alone cannot cover" section for the two further wrong-fix shapes Warden's
/// branch review found still pass S1/S2/S3 unchanged, and how (a) remains
/// reading-only while (b) is closed by S4.
///
/// RESTRUCTURED this round (dispatch #3597 coordination, network-cost
/// advisory) — same mechanism change as S2: copies the shared template
/// instead of doing its own independent cold install.
#[tokio::test]
async fn missing_pgvector_library_with_present_control_file_is_refused() {
    let _serialize = ENV_LOCK.lock().await;

    let template = template_install_dir().await;
    let home = tempfile::tempdir().expect("tempdir for isolated HOME");
    copy_theseus_tree(&template, home.path());
    let _home_guard = HomeGuard::install(home.path());

    // Given: a copy of the template, with the pgvector library removed but
    // vector.control left in place — the exact state install_extension()'s
    // own skip-guard treats as "already installed, nothing to do".
    let pin = pin_of_kind_for_current_platform(ArtifactKind::Pgvector);
    let install_dir = home.path().join(".theseus/postgresql/16.4.0");
    let pgvector_path = install_dir.join(pin.rel_path);
    let control_path = install_dir.join("share/extension/vector.control");
    assert!(
        control_path.exists(),
        "test setup bug: vector.control must exist in the copied template"
    );
    assert!(
        pgvector_path.exists(),
        "test setup bug: the pgvector library must exist in the copy before it is removed"
    );
    std::fs::remove_file(&pgvector_path).expect("remove the copied pgvector library");
    assert!(
        control_path.exists(),
        "test setup bug: removing the library must not remove vector.control"
    );
    assert!(
        !pgvector_path.exists(),
        "test setup bug: the pgvector library must be genuinely absent after removal"
    );

    // When: an engine attempts to start against this isolated copy, where
    // install_extension() will be skipped (control file present) but the
    // pinned library is missing.
    let start_result = EmbeddedEngine::start().await;

    // Then: refused, not silently started against a broken install.
    let err = match start_result {
        Ok(_) => panic!(
            "start() must refuse when the pgvector library is missing even though \
             vector.control is present (install_extension() would be skipped, so nothing \
             would ever write the library back) — a fix that only verifies inside the \
             install-taken branch, or skips verification when the file is absent, must not \
             silently start"
        ),
        Err(e) => e,
    };
    let hash_err = err.downcast_ref::<HashPinError>().unwrap_or_else(|| {
        panic!("expected a hash-pin refusal, got a different error type: {err}")
    });

    assert_eq!(
        hash_err.artifact, pin.artifact,
        "HashPinError must name the pgvector artifact specifically, not the postgres binary \
         or a different platform's pin"
    );
    assert_eq!(
        hash_err.expected, pin.sha256,
        "HashPinError must carry the real pinned expected hash for this artifact"
    );
    assert!(
        hash_err.actual.contains("read error"),
        "expected a read-error/absence (the library is genuinely missing, not merely wrong) — \
         actual={:?}",
        hash_err.actual
    );
}

// ---------------------------------------------------------------------------
// S4 — a tampered Postgres binary is refused. Closes wrong-fix (b) from
// Warden's branch review (C3 / advisory): "delete the Postgres-kind
// verification call entirely" still passes S1/S2/S3 unchanged, since none
// of them tampers `bin/postgres`.
// ---------------------------------------------------------------------------

/// Given a copy of the shared template install with ONE byte appended to
/// the copied `postgres` binary, when `EmbeddedEngine::start()` runs
/// against that copy, then it must refuse: `Err`, downcasting to
/// `HashPinError`, naming the POSTGRES artifact specifically, with a
/// genuine hash MISMATCH (not a read error).
///
/// # Why an appended byte, and why it is reachable black-box
///
/// A CORRUPTED binary (garbage bytes) makes `initdb` fail — its internal
/// `postgres --boot` call cannot exec it — before `verify_pinned_artifacts`
/// is ever reached (a wrong-reason red; confirmed empirically in an earlier
/// dispatch round, see the file header's Sanctioned reads). A single
/// appended byte is different: EMPIRICALLY CONFIRMED this round (not
/// assumed — the coordinator specifically flagged the risk that macOS code
/// signing could reject even a one-byte change) — a real `postgres` binary
/// from this machine's cache, copied to scratch with one byte appended,
/// still ran `--version` successfully AND still let a real `initdb` run
/// its full bootstrap to completion against it (see the dispatch report for
/// the exact commands and output). So this scenario reaches
/// `verify_pinned_artifacts(..., ArtifactKind::Postgres)` as a genuine hash
/// MISMATCH, not an exec failure — the scenario Warden's advisory asked for
/// is black-box reachable.
///
/// Positive control: a temporary, reverted-before-commit mutation to
/// `engine.rs` deleting the `ArtifactKind::Postgres` verification call
/// entirely must make this scenario fail (`start()` wrongly succeeds against
/// the tampered binary) — see the dispatch report for the mutation, the
/// failing run it produces, and the revert evidence.
#[tokio::test]
async fn tampered_postgres_binary_is_refused() {
    let _serialize = ENV_LOCK.lock().await;

    let template = template_install_dir().await;
    let home = tempfile::tempdir().expect("tempdir for isolated HOME");
    copy_theseus_tree(&template, home.path());
    let _home_guard = HomeGuard::install(home.path());

    // Given: a copy of the template's genuine, correctly-hashed postgres
    // binary...
    let pin = pin_of_kind_for_current_platform(ArtifactKind::Postgres);
    let install_dir = home.path().join(".theseus/postgresql/16.4.0");
    let postgres_path = install_dir.join(pin.rel_path);
    let mut bytes = std::fs::read(&postgres_path).expect("read the copied postgres binary");
    assert!(
        !bytes.is_empty(),
        "test setup bug: copied postgres binary must be non-empty before tampering"
    );

    // When: ONE byte is appended (still executes — see this fn's doc)...
    bytes.push(b'X');
    std::fs::write(&postgres_path, &bytes).expect("append one byte to the copied postgres binary");
    // `std::fs::write` truncates+rewrites an EXISTING file in place on
    // Unix, preserving its existing mode bits (the `mode` argument to the
    // underlying `open()` only applies when O_CREAT actually creates a NEW
    // file) — so this is defensive redundancy, not a correction of an
    // observed loss, but it costs nothing and documents the requirement
    // explicitly rather than silently assuming it holds.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&postgres_path)
            .expect("stat the tampered postgres binary")
            .permissions();
        perms.set_mode(perms.mode() | 0o111);
        std::fs::set_permissions(&postgres_path, perms)
            .expect("ensure the tampered postgres binary is still executable");
    }

    // ...and an engine attempts to start against this isolated copy.
    let start_result = EmbeddedEngine::start().await;

    // Then: refused, not silently started against a tampered binary.
    let err = match start_result {
        Ok(_) => panic!(
            "start() must refuse when the postgres binary no longer matches its pinned hash — \
             a tampered binary must never be silently accepted, but start() succeeded"
        ),
        Err(e) => e,
    };
    let hash_err = err.downcast_ref::<HashPinError>().unwrap_or_else(|| {
        panic!("expected a hash-pin refusal, got a different error type: {err}")
    });

    assert_eq!(
        hash_err.artifact, pin.artifact,
        "HashPinError must name the POSTGRES artifact specifically, not pgvector or a \
         different platform's pin"
    );
    assert_ne!(
        hash_err.actual, hash_err.expected,
        "must be a genuine hash mismatch, not a coincidental equal value"
    );
    assert!(
        !hash_err.actual.contains("read error"),
        "expected a hash MISMATCH on the tampered (but present, still-executable) binary, not \
         a read-error/absence (actual={:?})",
        hash_err.actual
    );
}
