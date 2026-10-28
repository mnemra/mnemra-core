//! Seeded safety test for `scripts/ci-reap.sh`'s baseline-PID self-reap
//! mechanism, wired into the `ci` justfile recipe (R-9,
//! `brain/specs/2026-07-04-agent-self-verify-long-jobs.md`, baseline-reap
//! mechanism amendment 2026-07-06, #2119; M1/M3 hardening, fix round).
//!
//! # Scope
//!
//! Drives the REAL `ci_reap_capture_baseline` / `ci_reap_own_postmasters`
//! shell functions from `scripts/ci-reap.sh` (not a Rust reimplementation),
//! so a regression in the actual script fails this test. The processes it
//! reaps against are synthetic markers, not real embedded-PG postmasters: a
//! real `sleep` binary invocation with `bash`'s `exec -a` used to rename its
//! visible `argv[0]` to a unique `.../bin/postgres`-shaped path (optionally
//! followed by a synthetic ` -D <data_dir>` argument, mimicking a real
//! postmaster's command line) per test invocation, so `pgrep -f` sees a
//! "postmaster"-shaped command line without booting a real (slow, heavy)
//! `postgresql_embedded` engine. The whole fake command line — including
//! the `-D <data_dir>` — lives in the single renamed `argv[0]` string, not
//! in separate argv elements spawned via an intermediate shell: an earlier
//! draft tried `bash -c 'sleep N' bash -D <data_dir>` to get `-D` into a
//! *separate* argv element, but bash's single-simple-command tail-call
//! optimization silently re-execs into plain `sleep` for that shape,
//! dropping both the `-a` rename and the trailing args; a `sleep N & wait`
//! wrapper avoids that but leaks an orphaned `sleep` child on SIGKILL of the
//! wrapper. Folding the fake `-D <data_dir>` into the `-a` string itself
//! keeps this to one real process with no forked children, so a kill is
//! always clean. The pattern `ci-reap.sh` matches against is overridden per
//! test run to a unique marker (`CI_REAP_PG_PATTERN`, ERE-escaped — see
//! `escape_ere_literal` / N1), so this test can never match — and can never
//! kill — a real postmaster on the host or another concurrently-running
//! instance of this same test.
//!
//! # No live Postgres required (NONPG_TEST_FLAGS)
//!
//! No embedded-Postgres engine is started anywhere in this file — wired into
//! `NONPG_TEST_FLAGS` in the `justfile`, not `PG_TEST_FLAGS`.
//!
//! # The mandatory cases (R-9 dispatch instruction + M1/M3 fix round)
//!
//! `reap_own_postmasters_kills_own_spawned_leaves_others_alive` asserts all
//! three from a single `ci_reap_own_postmasters` call, mirroring how the
//! real recipe uses it — one reap pass must produce all three outcomes:
//!
//! - (a) own-spawned, temp-root data dir (absent from baseline, condition
//!   (ii) satisfied) -> REAPED.
//! - (b) baseline / concurrent, temp-root data dir (present at capture
//!   time) -> LEFT ALIVE — **the original load-bearing safety case.** This
//!   assertion is the one that fails if the baseline-exclusion in
//!   `ci_reap_own_postmasters` is ever dropped (e.g. changed to "kill
//!   everything matching the pattern") — in production that regression
//!   would kill a concurrent agent's live engine or a pre-existing
//!   instance, exactly what R-9 exists to prevent.
//! - (c) own-spawned, NON-temp-root data dir (absent from baseline, but
//!   condition (ii) NOT satisfied — stands in for a developer's system
//!   Postgres or another project's engine) -> LEFT ALIVE — **the M1
//!   load-bearing safety case.** This assertion is the one that fails if
//!   the temp-root check is ever dropped (reverting to "reap everything
//!   absent from the baseline") — in production that regression would kill
//!   a developer's live system Postgres or another project's engine.
//!
//! `reap_refuses_to_run_when_baseline_never_captured` asserts the M3 capture
//! sentinel: calling `ci_reap_own_postmasters` without a prior
//! `ci_reap_capture_baseline` call must reap NOTHING, even a postmaster that
//! satisfies both (i) (trivially — no baseline was ever recorded, so nothing
//! is "in" it) and (ii) (temp-root data dir). This is the assertion that
//! fails if the capture-sentinel check is ever dropped (reverting to
//! treating an uncaptured baseline the same as a legitimately empty one) —
//! in production that regression would let a future caller that forgets to
//! call `ci_reap_capture_baseline` first silently kill everything matching.

use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// The repo root (where `scripts/` and the `justfile` live) is two
/// directories up from this crate's manifest dir (`libs/mnemra-host`).
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("mnemra-host manifest dir has two ancestors: libs/, repo root")
        .to_path_buf()
}

/// A unique `.../bin/postgres`-shaped marker string for this test
/// invocation. Backed by a real `tempfile::TempDir` purely for its
/// OS-guaranteed-unique, auto-cleaned path — the `bin/postgres` suffix is
/// never actually created on disk; `exec -a` only needs the string, not a
/// file at that path.
fn unique_marker() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("create tempdir for marker uniqueness");
    let marker = dir
        .path()
        .join("bin")
        .join("postgres")
        .to_string_lossy()
        .to_string();
    (dir, marker)
}

/// Escape ERE metacharacters (N1) so `marker` can be used as a `pgrep -f`
/// pattern matching only its literal text. Without this, a tempdir path
/// containing `.` (matches "any character" in an ERE) carries a negligible
/// but real over-match risk; production's default pattern ("bin/postgres")
/// has no metacharacters and needs no escaping, but the test's tempdir-
/// derived marker does.
fn escape_ere_literal(marker: &str) -> String {
    let mut out = String::with_capacity(marker.len());
    for c in marker.chars() {
        if matches!(
            c,
            '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' | '\\'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// RAII guard that force-kills and reaps a spawned marker process when it
/// goes out of scope — including on an early test-assertion panic (N2).
/// Without this, a failed `assert!` earlier in a test unwinds past any
/// manual end-of-test cleanup, leaking a real `sleep`-backed marker process
/// for up to its full spawned duration. Double-kill (once here, once at any
/// remaining explicit call site) is harmless: killing an already-dead PID
/// just fails quietly.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Deref for KillOnDrop {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}

impl DerefMut for KillOnDrop {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

/// Spawn a real, long-lived process whose visible command line is
/// `<marker> -D <data_dir> <duration>` (or just `<marker> <duration>` when
/// `data_dir` is `None`) — a stand-in "postmaster" for `pgrep -f` to find
/// and for `ci-reap.sh`'s `_ci_reap_data_dir_for_pid` to parse. Runs the
/// real `sleep` (PATH-resolved, so this works identically on macOS and
/// Linux) with its argv[0] renamed via bash's `exec -a` to the full fake
/// command-line string (marker path plus the synthetic `-D` argument, all
/// folded into the single renamed argv[0] — see the module doc for why:
/// this keeps the marker to one real process with no forked children, so a
/// kill is always clean and immediate). The marker path never has to exist
/// on disk and no binary is copied (copying a platform-signed binary to a
/// new path gets it SIGKILLed by the OS on modern macOS — renaming argv[0]
/// in place avoids that entirely).
fn spawn_marker_process(marker: &str, data_dir: Option<&str>) -> KillOnDrop {
    let argv0 = match data_dir {
        Some(dir) => format!("{marker} -D {dir}"),
        None => marker.to_string(),
    };
    let child = Command::new("bash")
        .args(["-c", "exec -a \"$1\" sleep \"$2\"", "bash", &argv0, "300"])
        .spawn()
        .expect("spawn fake postmaster marker process (argv0-renamed sleep)");
    KillOnDrop(child)
}

fn is_alive(child: &mut Child) -> bool {
    matches!(child.try_wait(), Ok(None))
}

/// Run one `ci-reap.sh` function against `marker`/`baseline_file`, exactly
/// as the `ci` justfile recipe would (`source scripts/ci-reap.sh && <fn>`),
/// from the repo root.
fn run_ci_reap_function(
    function_name: &str,
    marker: &str,
    baseline_file: &std::path::Path,
    repo_root: &std::path::Path,
) {
    let output = Command::new("bash")
        .arg("-c")
        .arg(format!("source scripts/ci-reap.sh && {function_name}"))
        .current_dir(repo_root)
        .env("CI_REAP_PG_PATTERN", escape_ere_literal(marker))
        .env("CI_REAP_BASELINE_FILE", baseline_file)
        .output()
        .unwrap_or_else(|e| panic!("failed to run ci-reap.sh {function_name}: {e}"));
    assert!(
        output.status.success(),
        "ci-reap.sh {function_name} exited non-zero: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Poll briefly instead of a fixed sleep, to avoid flaking under load.
fn wait_until_dead(child: &mut Child, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while is_alive(child) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn reap_own_postmasters_kills_own_spawned_leaves_others_alive() {
    let root = repo_root();
    let (_marker_dir, marker) = unique_marker();
    let temp_root = std::env::temp_dir();

    // (b) Baseline process: alive BEFORE ci_reap_capture_baseline runs —
    // stands in for a concurrent agent's live embedded-PG engine, or a
    // pre-existing leaked instance. Its data dir is under the temp root
    // (condition (ii) would be satisfied), but presence in the baseline
    // (condition (i)) is what must protect it. Must survive the reap below.
    let mut baseline_child = spawn_marker_process(
        &marker,
        Some(&temp_root.join("ci-reap-test-baseline").to_string_lossy()),
    );
    assert!(
        is_alive(&mut baseline_child),
        "baseline marker process failed to start"
    );

    let baseline_file = tempfile::NamedTempFile::new().expect("create baseline file");

    run_ci_reap_function(
        "ci_reap_capture_baseline",
        &marker,
        baseline_file.path(),
        &root,
    );
    let captured = fs::read_to_string(baseline_file.path()).expect("read baseline file");
    assert!(
        captured
            .lines()
            .any(|l| l.trim() == baseline_child.id().to_string()),
        "ci_reap_capture_baseline did not record the pre-existing marker PID {} \
         (captured: {captured:?})",
        baseline_child.id()
    );

    // (a) Own-spawned process, temp-root data dir: started AFTER the
    // baseline snapshot, data dir under the embedded-PG temp root — stands
    // in for this ci run's own leaked postmaster. Must be reaped.
    let mut own_child = spawn_marker_process(
        &marker,
        Some(&temp_root.join("ci-reap-test-own").to_string_lossy()),
    );
    assert!(
        is_alive(&mut own_child),
        "own-spawned marker process failed to start"
    );

    // (c) Own-spawned process, NON-temp-root data dir: also started AFTER
    // the baseline snapshot (absent from it, same as (a)), but its data dir
    // is NOT under the embedded-PG temp root — stands in for a developer's
    // system Postgres, or another project's embedded PG, that happened to
    // start after this run's baseline capture. M1 load-bearing case: must
    // be LEFT ALIVE despite being absent from the baseline.
    let mut system_child = spawn_marker_process(&marker, Some("/var/lib/postgresql/16/main"));
    assert!(
        is_alive(&mut system_child),
        "system-postgres-stand-in marker process failed to start"
    );

    // Simulated ci failure/interrupt -> reap.
    run_ci_reap_function(
        "ci_reap_own_postmasters",
        &marker,
        baseline_file.path(),
        &root,
    );

    // SIGKILL delivery + zombie reap via try_wait is near-instant; poll
    // briefly instead of a fixed sleep to avoid flaking under load.
    wait_until_dead(&mut own_child, Duration::from_secs(5));

    // (a) own-spawned, temp-root data dir -> REAPED.
    assert!(
        !is_alive(&mut own_child),
        "own-spawned marker (absent from baseline, temp-root data dir) was NOT reaped"
    );
    // (b) baseline/concurrent, temp-root data dir -> LEFT ALIVE. See module
    // doc — this is the original load-bearing safety assertion.
    assert!(
        is_alive(&mut baseline_child),
        "baseline marker (present at capture time) was reaped — the \
         baseline-exclusion is broken; in production this would kill a \
         concurrent agent's live engine or a pre-existing instance"
    );
    // (c) own-spawned, NON-temp-root data dir -> LEFT ALIVE. See module doc
    // — this is the M1 load-bearing safety assertion.
    assert!(
        is_alive(&mut system_child),
        "system-postgres stand-in (absent from baseline, non-temp-root data \
         dir) was reaped — the temp-root narrowing (M1) is broken; in \
         production this would kill a developer's live system Postgres or \
         another project's engine"
    );

    // Explicit cleanup on the normal (non-panic) path; KillOnDrop (N2) also
    // covers the early-panic case for all three markers.
    let _ = baseline_child.kill();
    let _ = system_child.kill();
}

/// Run `source scripts/ci-reap.sh && ci_reap_isolate_tmpdir && <script_tail>`
/// from `cwd` with the ambient `TMPDIR` set to `base_tmp`, exactly the way the
/// `ci` recipe orders it (#4361). Returns trimmed stdout.
fn run_isolated(
    script_tail: &str,
    marker: &str,
    baseline_file: &std::path::Path,
    base_tmp: &std::path::Path,
    cwd: &std::path::Path,
    repo_root: &std::path::Path,
) -> String {
    let script = repo_root.join("scripts/ci-reap.sh");
    let output = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "source '{}' && ci_reap_isolate_tmpdir && {script_tail}",
            script.display()
        ))
        .current_dir(cwd)
        .env("TMPDIR", base_tmp)
        .env("CI_REAP_PG_PATTERN", escape_ere_literal(marker))
        .env("CI_REAP_BASELINE_FILE", baseline_file)
        .output()
        .unwrap_or_else(|e| panic!("failed to run ci-reap.sh `{script_tail}`: {e}"));
    assert!(
        output.status.success(),
        "ci-reap.sh `{script_tail}` exited non-zero: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// #4361: a failing run's reap must not kill a postmaster whose `-D` is under
/// ANOTHER worktree's temp root, and must still reap one under its own root
/// (positive control). The recipe keeps one exported TMPDIR across capture and
/// reap; here each bash call re-derives it, which is a pure function of the
/// cwd and the base TMPDIR, so both calls see the same root.
#[test]
fn reap_spares_other_worktrees_temp_root_and_reaps_own() {
    let root = repo_root();
    let (_marker_dir, marker) = unique_marker();
    let base = tempfile::tempdir().expect("create base temp dir");
    let baseline_file = tempfile::NamedTempFile::new().expect("create baseline file");

    let own_root = run_isolated(
        "printf %s \"$TMPDIR\"",
        &marker,
        baseline_file.path(),
        base.path(),
        &root,
        &root,
    );
    // Another worktree's root, spelled literally (not derived by the code
    // under test) so the survival assertion below cannot be satisfied by the
    // derivation agreeing with itself.
    let other_root = format!("{}/mnemra-ci-other-worktree", base.path().display());

    run_isolated(
        "ci_reap_capture_baseline",
        &marker,
        baseline_file.path(),
        base.path(),
        &root,
        &root,
    );

    // Both started AFTER the baseline: only the temp root tells them apart.
    let mut own_child = spawn_marker_process(&marker, Some(&format!("{own_root}/.tmpOWN")));
    let mut other_child = spawn_marker_process(&marker, Some(&format!("{other_root}/.tmpOTHER")));
    assert!(is_alive(&mut own_child), "own marker failed to start");
    assert!(
        is_alive(&mut other_child),
        "other-worktree marker failed to start"
    );

    run_isolated(
        "ci_reap_own_postmasters",
        &marker,
        baseline_file.path(),
        base.path(),
        &root,
        &root,
    );

    wait_until_dead(&mut own_child, Duration::from_secs(5));
    // Positive control: without it, a reap that never fires would pass the
    // survival assertion below.
    assert!(
        !is_alive(&mut own_child),
        "marker under THIS run's temp root was not reaped (positive control)"
    );
    assert!(
        is_alive(&mut other_child),
        "marker under ANOTHER worktree's temp root was reaped — the per-worktree \
         TMPDIR isolation is broken; in production a failing run would kill the \
         other lane's live embedded Postgres (#4361)"
    );

    // Distinct worktree paths must derive distinct roots, and each root sits
    // under the ambient TMPDIR.
    let other_cwd = tempfile::tempdir().expect("create stand-in for another worktree");
    let derived_other = run_isolated(
        "printf %s \"$TMPDIR\"",
        &marker,
        baseline_file.path(),
        base.path(),
        other_cwd.path(),
        &root,
    );
    assert_ne!(
        own_root, derived_other,
        "two worktrees derived the same temp root; isolation is a no-op"
    );
    let base_str = base.path().to_string_lossy().to_string();
    assert!(
        own_root.starts_with(&base_str),
        "own temp root {own_root:?} is not under the base TMPDIR {base_str:?}"
    );
}

/// macOS `$TMPDIR` ends in `/` and pg_ctl prints `-D` canonicalised
/// (`/x//y/` -> `/x/y`). With a trailing-slash (and doubled-slash) ambient
/// TMPDIR, a marker whose `-D` is in the canonical form must still be reaped:
/// if the isolated root is not canonical, the reap's prefix match silently
/// stops matching the run's own leaks (no error, no signal).
#[test]
fn reap_matches_canonical_data_dir_under_trailing_slash_tmpdir() {
    let root = repo_root();
    let (_marker_dir, marker) = unique_marker();
    let base = tempfile::tempdir().expect("create base temp dir");
    let sloppy_base = std::path::PathBuf::from(format!("{}//", base.path().display()));
    let baseline_file = tempfile::NamedTempFile::new().expect("create baseline file");

    let own_root = run_isolated(
        "printf %s \"$TMPDIR\"",
        &marker,
        baseline_file.path(),
        &sloppy_base,
        &root,
        &root,
    );
    assert!(
        !own_root.ends_with('/') && !own_root.contains("//"),
        "isolated temp root {own_root:?} is not canonical (trailing or doubled slash)"
    );

    run_isolated(
        "ci_reap_capture_baseline",
        &marker,
        baseline_file.path(),
        &sloppy_base,
        &root,
        &root,
    );
    // The `-D` pg_ctl would print for a data dir created under this root.
    let mut own_child = spawn_marker_process(&marker, Some(&format!("{own_root}/.tmpCANON")));
    assert!(is_alive(&mut own_child), "own marker failed to start");

    run_isolated(
        "ci_reap_own_postmasters",
        &marker,
        baseline_file.path(),
        &sloppy_base,
        &root,
        &root,
    );
    wait_until_dead(&mut own_child, Duration::from_secs(5));
    assert!(
        !is_alive(&mut own_child),
        "own marker with a canonical -D was NOT reaped under a trailing-slash TMPDIR — \
         the run's own leaked postmasters would silently survive"
    );
}

/// The isolation only holds if the `ci` recipe calls it before the baseline
/// capture. A prose check on the recipe body: removing or reordering the call
/// fails here rather than silently restoring the shared temp root.
#[test]
fn ci_recipe_isolates_tmpdir_before_capturing_baseline() {
    let justfile = fs::read_to_string(repo_root().join("justfile")).expect("read justfile");
    let recipe = justfile
        .split("\nci chain='_ci-verify-chain':")
        .nth(1)
        .expect("`ci` recipe header present in justfile")
        .split("\nci-full:")
        .next()
        .expect("`ci` recipe ends at ci-full");
    let isolate = recipe
        .find("\n    ci_reap_isolate_tmpdir\n")
        .expect("`ci` recipe calls ci_reap_isolate_tmpdir");
    let capture = recipe
        .find("\n    ci_reap_capture_baseline\n")
        .expect("`ci` recipe calls ci_reap_capture_baseline");
    assert!(
        isolate < capture,
        "ci_reap_isolate_tmpdir must run before ci_reap_capture_baseline"
    );
}

#[test]
fn reap_refuses_to_run_when_baseline_never_captured() {
    let root = repo_root();
    let (_marker_dir, marker) = unique_marker();
    let temp_root = std::env::temp_dir();

    // A postmaster absent from any baseline (none was ever captured) AND
    // with a temp-root data dir — satisfies condition (ii) and, trivially,
    // "absent from the baseline" (there is no baseline). If the M3 sentinel
    // were dropped, this would be indistinguishable from a legitimately
    // empty baseline and would be reaped.
    let mut marker_child = spawn_marker_process(
        &marker,
        Some(
            &temp_root
                .join("ci-reap-test-never-captured")
                .to_string_lossy(),
        ),
    );
    assert!(
        is_alive(&mut marker_child),
        "marker process failed to start"
    );

    // Deliberately never call ci_reap_capture_baseline. Point
    // CI_REAP_BASELINE_FILE at a path that has never been written (no
    // baseline file, no `.captured` sentinel file) — exactly what a future
    // caller that forgets to call ci_reap_capture_baseline first would
    // produce.
    let baseline_dir = tempfile::tempdir().expect("create scratch dir for baseline path");
    let never_captured_baseline_file = baseline_dir.path().join("never-written-baseline");

    run_ci_reap_function(
        "ci_reap_own_postmasters",
        &marker,
        &never_captured_baseline_file,
        &root,
    );

    // Give a regressed implementation a moment to have acted, the same way
    // the other test polls for a real kill.
    std::thread::sleep(Duration::from_millis(200));

    // M3 load-bearing assertion: LEFT ALIVE. A capture-sentinel regression
    // (treating "never captured" the same as "captured empty") would reap
    // this marker even though it's a perfectly legitimate pre-existing
    // postmaster this run never saw at any baseline snapshot.
    assert!(
        is_alive(&mut marker_child),
        "marker was reaped even though ci_reap_capture_baseline was never \
         called — the M3 capture sentinel is broken; in production a \
         caller of ci_reap_own_postmasters that forgets to capture a \
         baseline first would silently kill everything matching"
    );

    let _ = marker_child.kill();
}
