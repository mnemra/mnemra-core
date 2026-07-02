//! Startup-ordering acceptance tests, PG group — T5 RED (task #1992,
//! R-0022-a boundaries 5b-i and accept-last).
//!
//! # Scope and group placement
//!
//! Both scenarios here reach real `PostgresStorage::start_embedded()` (the
//! happy path completes it; the builtin-init injection fires only *after*
//! storage init succeeds — see `InjectedFailure`'s injection-point
//! contract), so this file is wired into `PG_TEST_FLAGS` in the `justfile`
//! and runs at `--test-threads 1` (embedded-Postgres teardown-race
//! discipline, #1852).
//!
//! # RED phase — right-reason failures
//!
//! `run_with` is a `todo!()` skeleton; both tests fail at that panic (the
//! happy-path test observes it as the worker thread dying before sending a
//! result). `verify = []` for this dispatch: red by design until GREEN.
//!
//! # Boundary ↔ test map
//!
//! | Boundary | Test |
//! |---|---|
//! | 5b-i (R-0002-c)      | `builtin_init_failure_fails_closed_with_no_plugin_load_attempted` (test-hooks) |
//! | accept-last (R-0022-a) | `run_returns_ok_after_construction_without_serving_and_readiness_flips_ready` |
//!
//! # Log capture
//!
//! `#[traced_test]` (tracing-test, `no-env-filter`) installs a global
//! capturing subscriber, so events from the worker thread and the startup
//! path are captured. The 5b-i observable is the *absence* of the shared
//! `PLUGIN_LOAD_LOG_LINE` token; the happy path asserts its *presence* so
//! the absence assertion cannot go vacuously green.

use std::time::Duration;

use mnemra_host::{PLUGIN_LOAD_LOG_LINE, RunConfig, RunError, RunHandle, run_with};
use serde_json::Value;
use tracing_test::traced_test;

#[path = "common/startup_probe.rs"]
mod startup_probe;
use startup_probe::{http_roundtrip, repo_root, write_admin_token_file};

/// Ceiling for a full production startup (embedded-Postgres bring-up can be
/// slow on a cold runner; the engine's own command timeout is 60s). Reaching
/// this ceiling on the happy path means `run_with` did not return — i.e. it
/// started serving (T6's job) or hung.
const RUN_RETURN_CEILING: Duration = Duration::from_secs(600);

/// Drive `run_with` to completion on a dedicated worker thread with its own
/// current-thread runtime, and wait for the result with a ceiling.
///
/// This is how "returns without serving" is made assertable: a `run_with`
/// that enters a serve-loop never sends a result and the receiver times out.
/// (The dev-dep tokio build here has no `time` feature, so the ceiling lives
/// on the std channel, not in tokio.) The worker panicking — e.g. RED's
/// `todo!()` — disconnects the channel, which also surfaces as `Err`.
fn run_to_completion(config: RunConfig) -> Result<RunHandle, RunError> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("worker runtime build");
        let result = rt.block_on(run_with(config));
        let _ = tx.send(result);
    });
    rx.recv_timeout(RUN_RETURN_CEILING).unwrap_or_else(|e| {
        panic!(
            "accept-last (R-0022-a): run_with must RETURN after construction — it did not \
             ({e}: either it started serving / hung [T6 owns the serve-loop] or it panicked; \
             see the worker thread's panic output above)"
        )
    })
}

// ---------------------------------------------------------------------------
// accept-last — run_with returns Ok after construction, without serving,
// with readiness flipped ready
// ---------------------------------------------------------------------------

/// R-0022-a accept-last (+ end-to-end keystone happy path through `run_with`:
/// the production startup loads the real committed manifest + signed
/// artifact from the actual repo root).
///
/// GIVEN a mode-600 admin-token file and the real repo root
/// WHEN `run_with` performs production startup
/// THEN it RETURNS `Ok(handle)` (no serve-loop — T6 owns serving; a hang is
///      the failure mode the ceiling converts into a test failure)
///  AND readiness is flipped ready: `GET /health` on the bound address
///      returns 200 with the R-0004-g detail body reporting `overall: "ok"`
///      against the live initialized storage (R-0013-a schema init ran)
///  AND the plugin-load log token was emitted (non-vacuity partner of the
///      5b-i absence assertion below).
#[traced_test]
#[test]
fn run_returns_ok_after_construction_without_serving_and_readiness_flips_ready() {
    let dir = tempfile::tempdir().expect("tempdir");
    let token_path = write_admin_token_file(dir.path(), 0o600);
    // Port 0: OS-assigned; discovered via the handle (no reservation race).
    let health_addr = "127.0.0.1:0".parse().expect("literal loopback addr");

    let handle = run_to_completion(RunConfig::new(repo_root(), token_path, health_addr))
        .unwrap_or_else(|e| {
            panic!("production startup against the real repo root must succeed; got Err: {e}")
        });

    // Readiness flipped ready at successful construction: /health serves the
    // R-0004-g detail body against the live pool (not the 503 not-ready shape).
    let (status, body) = http_roundtrip(handle.health_addr(), "GET", "/health");
    assert_eq!(
        status, 200,
        "after a successful run_with, /health must report ready (200); body: {body}"
    );
    let json: Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("/health body must be valid JSON: {e}; body: {body}"));
    assert_eq!(
        json.get("postgres"),
        Some(&Value::Bool(true)),
        "storage was initialized (5c) — postgres must be up; body: {json}"
    );
    assert_eq!(
        json.get("workspace_default"),
        Some(&Value::Bool(true)),
        "schema init ran (R-0013-a) — the default workspace must exist; body: {json}"
    );
    assert_eq!(
        json.get("overall"),
        Some(&Value::String("ok".to_string())),
        "the constructed host must be fully healthy; body: {json}"
    );

    // Plugin load happened (5b-ii ran) and announced itself with the shared
    // token — this is what makes the 5b-i absence assertion falsifiable.
    assert!(
        logs_contain(PLUGIN_LOAD_LOG_LINE),
        "a successful startup must emit the '{PLUGIN_LOAD_LOG_LINE}' tracing token on the \
         run_with call path (shared 5b-i observable — see mnemra_host::PLUGIN_LOAD_LOG_LINE)"
    );

    // `handle` stays bound to the end of scope: the host (engine, pool,
    // /health listener) must remain alive through every assertion above.
}

// ---------------------------------------------------------------------------
// 5b-i — builtin-init failure: fail closed, no plugin load attempted
// ---------------------------------------------------------------------------

/// R-0002-c / R-0022-a 5b-i (observed as a startup-failure property, per the
/// spec's round-1 reframing — not a /health body field).
///
/// GIVEN a builtin-init failure injected at the 5b-i boundary (after real
///       storage init succeeds)
/// WHEN `run_with` starts the host
/// THEN it returns `Err(RunError::BuiltinInit(..))` — fail-closed
///  AND no plugin load was attempted: the shared plugin-load log token was
///      never emitted (and hence no `PluginPool` was constructed — the token
///      precedes the verified-load gate on the call path).
#[cfg(feature = "test-hooks")]
#[traced_test]
#[test]
fn builtin_init_failure_fails_closed_with_no_plugin_load_attempted() {
    use mnemra_host::InjectedFailure;

    // Capture-liveness canary: proves the subscriber is recording before any
    // absence assertion is trusted (guards against a vacuously-green absence).
    tracing::info!("t5 capture canary: builtin-init injection test");
    assert!(
        logs_contain("t5 capture canary"),
        "the traced_test capture channel must be live before absence assertions"
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let token_path = write_admin_token_file(dir.path(), 0o600);
    let health_addr = "127.0.0.1:0".parse().expect("literal loopback addr");

    let config = RunConfig::new(repo_root(), token_path, health_addr)
        .with_injected_failure(InjectedFailure::BuiltinInit);
    let err = match run_to_completion(config) {
        Err(e) => e,
        Ok(_) => panic!(
            "R-0022-a 5b-i: an injected builtin-init failure must fail startup closed — got Ok"
        ),
    };

    assert!(
        matches!(err, RunError::BuiltinInit(_)),
        "5b-i failure must surface as RunError::BuiltinInit, not a different boundary's \
         variant; got: {err:?}"
    );

    assert!(
        !logs_contain(PLUGIN_LOAD_LOG_LINE),
        "R-0002-c: after a builtin-init failure NO plugin load may be attempted — the \
         '{PLUGIN_LOAD_LOG_LINE}' token must be absent (no PluginPool, no load log line)"
    );
}
