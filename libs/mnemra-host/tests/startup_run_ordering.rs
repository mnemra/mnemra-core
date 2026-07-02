//! Startup-ordering acceptance tests, non-PG group — T5 RED (task #1992,
//! R-0022-a / R-0022-e / R-0005-f, keystone R-0021-e).
//!
//! # Scope and group placement
//!
//! Every scenario here fails **before** (or entirely without) real embedded
//! Postgres: the 5-pre file-mode refusal fires before storage, the injected
//! storage-init failure *replaces* `start_embedded()`, and the keystone
//! load-path pair never touches storage at all. Wired into
//! `NONPG_TEST_FLAGS` in the `justfile` (default thread count). Scenarios
//! that reach real `start_embedded()` live in `startup_run_full.rs`
//! (PG group, `--test-threads 1`).
//!
//! # RED phase — right-reason failures
//!
//! `run_with` and `populate_verified_pool_from_dir` are `todo!()` skeletons
//! (see `mnemra_host.rs` / `startup/pool_population.rs`). Every test below
//! panics at the first skeleton call on the way to its assertions — that IS
//! the red (`skills/tdd.md`: the stub's panic propagates as the failure; no
//! `#[should_panic]` inversion). `verify = []` for this dispatch: red by
//! design until GREEN lands the bodies.
//!
//! # Boundary ↔ test map (plan Task 5 test expectations)
//!
//! | Boundary | Test |
//! |---|---|
//! | 5-pre (R-0005-f)  | `world_readable_admin_token_fails_startup_before_any_listener_binds` |
//! | 5a  (R-0004-g)    | `health_answers_while_mcp_is_not_accepting` (test-hooks) |
//! | 5c  (R-0022-d)    | `storage_init_failure_fails_closed_and_health_reports_not_ready` (test-hooks) |
//! | 5b-ii keystone    | `production_manifest_and_committed_artifact_populate_verified_pool`, `swapped_component_bytes_are_rejected_on_the_hash_mismatch_path` |
//!
//! # Keystone fixture discipline (forbid-scope)
//!
//! The committed `plugins/` and `artifacts/` trees are **read-only inputs**:
//! the keystone tests copy the real signed manifest and the real signed
//! artifact into a temp-dir replica of the repo layout and point
//! `populate_verified_pool_from_dir` at the replica — the committed trees
//! are never modified. The swapped variant tampers only the replica's copy.

use std::net::TcpStream;
use std::time::Duration;

use mnemra_host::startup::{StartupError, populate_verified_pool_from_dir};
use mnemra_host::{RunConfig, RunError, run_with};
// Used only by the test-hooks-gated 5a/5c scenarios; gated so the default
// build carries no unused imports (clippy -D warnings).
#[cfg(feature = "test-hooks")]
use serde_json::Value;

#[path = "common/startup_probe.rs"]
mod startup_probe;
#[cfg(feature = "test-hooks")]
use startup_probe::http_roundtrip;
use startup_probe::{repo_root, reserve_loopback_addr, write_admin_token_file};

// ---------------------------------------------------------------------------
// 5-pre — world-readable admin token: refuse to start, bind nothing
// ---------------------------------------------------------------------------

/// R-0005-f (instantiated over the admin-token file) + R-0022-a 5-pre.
///
/// GIVEN an admin-token file that is world-readable (mode 0644)
/// WHEN `run_with` starts the host
/// THEN it returns `Err(RunError::FileMode(..))` — the file-mode invariant
///      check fires FIRST, before any listener binds
///  AND nothing is listening on the configured `/health` address (the
///      connection is refused — no listener was ever bound).
#[tokio::test]
async fn world_readable_admin_token_fails_startup_before_any_listener_binds() {
    let dir = tempfile::tempdir().expect("tempdir");
    let token_path = write_admin_token_file(dir.path(), 0o644); // world-readable
    let health_addr = reserve_loopback_addr();

    let result = run_with(RunConfig::new(repo_root(), token_path, health_addr)).await;

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!(
            "R-0005-f / 5-pre: a world-readable admin-token file must refuse startup, not proceed"
        ),
    };
    assert!(
        matches!(err, RunError::FileMode(_)),
        "5-pre failure must surface as RunError::FileMode (the file-mode invariant check), \
         not a later boundary's variant; got: {err:?}"
    );

    // No listener bound: the reserved /health address must refuse connections.
    let probe = TcpStream::connect_timeout(&health_addr, Duration::from_millis(500));
    assert!(
        probe.is_err(),
        "R-0022-a 5-pre: after a file-mode refusal NO listener may be bound — \
         but {health_addr} accepted a connection"
    );
}

// ---------------------------------------------------------------------------
// 5a — /health answers while MCP is not accepting (deterministic: startup
// is halted at the storage boundary, so MCP never begins accepting)
// ---------------------------------------------------------------------------

/// R-0004-g / R-0022-a 5a.
///
/// GIVEN startup deterministically halted after the 5a bind (injected
///       storage-init failure — the deterministic not-yet-accepting state
///       the spec requires instead of a wall-clock race)
/// WHEN a client issues `GET /health` on the configured address
/// THEN the listener answers (503 not-ready is fine) while the MCP server
///      was never constructed and never accepted a request.
#[cfg(feature = "test-hooks")]
#[tokio::test]
async fn health_answers_while_mcp_is_not_accepting() {
    use mnemra_host::InjectedFailure;

    let dir = tempfile::tempdir().expect("tempdir");
    let token_path = write_admin_token_file(dir.path(), 0o600);
    let health_addr = reserve_loopback_addr();

    let config = RunConfig::new(repo_root(), token_path, health_addr)
        .with_injected_failure(InjectedFailure::StorageInit);
    let result = run_with(config).await;
    assert!(
        result.is_err(),
        "the injected storage-init failure must halt startup (5c fail-closed)"
    );

    // /health was bound at 5a — before the halted boundary — so it answers
    // even though startup failed and MCP is not (and never will be) accepting.
    let (status, body) = http_roundtrip(health_addr, "GET", "/health");
    assert_eq!(
        status, 503,
        "R-0022-a 5a: /health must answer (not-ready) while MCP is not accepting; body: {body}"
    );
}

// ---------------------------------------------------------------------------
// 5c — storage-init failure: fail closed, server never constructed,
// /health reports not-ready
// ---------------------------------------------------------------------------

/// R-0022-d / R-0022-a 5c.
///
/// GIVEN a storage-init failure injected at the 5c boundary
/// WHEN `run_with` starts the host
/// THEN it returns `Err(RunError::Storage(..))` — fail-closed, the
///      `MnemraMcpServer` is never constructed
///  AND `/health` (bound at 5a, still up after the failure) reports
///      not-ready: 503 + `{"ready": false}` (the sanctioned T4 contract).
#[cfg(feature = "test-hooks")]
#[tokio::test]
async fn storage_init_failure_fails_closed_and_health_reports_not_ready() {
    use mnemra_host::InjectedFailure;

    let dir = tempfile::tempdir().expect("tempdir");
    let token_path = write_admin_token_file(dir.path(), 0o600);
    let health_addr = reserve_loopback_addr();

    let config = RunConfig::new(repo_root(), token_path, health_addr)
        .with_injected_failure(InjectedFailure::StorageInit);
    let err = match run_with(config).await {
        Err(e) => e,
        Ok(_) => panic!("R-0022-a 5c: a storage-init failure must fail startup closed — got Ok"),
    };

    assert!(
        matches!(err, RunError::Storage(_)),
        "5c failure must surface as RunError::Storage, not a different boundary's \
         variant; got: {err:?}"
    );

    // The host did not proceed to serving: /health reports not-ready.
    let (status, body) = http_roundtrip(health_addr, "GET", "/health");
    assert_eq!(
        status, 503,
        "R-0022-a 5c: /health must report not-ready after a storage-init failure; body: {body}"
    );
    let json: Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("not-ready body must be valid JSON: {e}; body: {body}"));
    assert_eq!(
        json.get("ready"),
        Some(&Value::Bool(false)),
        "not-ready body must be the sanctioned T4 contract {{\"ready\": false}}; body: {json}"
    );
}

// ---------------------------------------------------------------------------
// 5b-ii KEYSTONE — production load path: real manifest + committed signed
// artifact accepted; swapped component bytes rejected (R-0022-e / R-0021-e)
// ---------------------------------------------------------------------------

/// Lay out a temp-dir replica of the repo's `plugins/` + `artifacts/` layout:
/// the real committed manifest, and `component_bytes` as the component
/// artifact. Returns the replica root (keep the TempDir alive).
fn replica_with_component(component_bytes: &[u8]) -> tempfile::TempDir {
    let replica = tempfile::tempdir().expect("tempdir for replica");
    let plugin_dir = replica.path().join("plugins/mnemra-echo");
    let artifact_dir = replica.path().join("artifacts/mnemra-echo");
    std::fs::create_dir_all(&plugin_dir).expect("create replica plugins dir");
    std::fs::create_dir_all(&artifact_dir).expect("create replica artifacts dir");
    std::fs::copy(
        repo_root().join("plugins/mnemra-echo/manifest.toml"),
        plugin_dir.join("manifest.toml"),
    )
    .expect("copy the real signed manifest into the replica (read-only source)");
    std::fs::write(artifact_dir.join("mnemra_echo.wasm"), component_bytes)
        .expect("write the replica component artifact");
    replica
}

/// The committed signed artifact's bytes (read-only input).
fn committed_artifact_bytes() -> Vec<u8> {
    std::fs::read(repo_root().join("artifacts/mnemra-echo/mnemra_echo.wasm"))
        .expect("read the committed signed artifact (artifacts/mnemra-echo/mnemra_echo.wasm)")
}

/// R-0022-e (decoupling-reversal) + R-0021-e accept side — the keystone.
///
/// GIVEN the real on-disk signed manifest and the committed signed artifact
///       (byte-identical copies in a replica root) and the REAL embedded root
/// WHEN the production disk-driven gate loads the plugin
/// THEN it returns `Ok(pool)` — signature verifies against the real root AND
///      the recomputed BLAKE3 of the loaded bytes equals the signed
///      `[component].hash`
///  AND the pool holds the R-0016-a instance range (3–5) for `mnemra-echo`
///      (a real populated pool, not a stub `Ok`).
#[test]
fn production_manifest_and_committed_artifact_populate_verified_pool() {
    use mnemra_host::mcp::server::ECHO_PLUGIN_NAME;
    use mnemra_host::plugin::pool::{POOL_MAX, POOL_MIN};

    let replica = replica_with_component(&committed_artifact_bytes());

    let pool =
        populate_verified_pool_from_dir(replica.path(), mnemra_host::signing::root_material::ROOT)
            .unwrap_or_else(|e| {
                panic!(
                    "R-0022-e keystone: the real signed manifest + committed signed artifact must \
                 load Ok against the real embedded root; got Err: {e}"
                )
            });

    let slots = pool.slot_count(ECHO_PLUGIN_NAME);
    assert!(
        (POOL_MIN..=POOL_MAX).contains(&slots),
        "R-0016-a: the verified pool must hold {POOL_MIN}–{POOL_MAX} instances of \
         '{ECHO_PLUGIN_NAME}'; got {slots}"
    );
}

/// R-0021-e reject side — the supply-chain swap (keystone measure 2/3/4).
///
/// GIVEN the same real signed manifest but component bytes that differ from
///       the signed artifact (a post-signing swap)
/// WHEN the production disk-driven gate loads the plugin
/// THEN it returns `Err` on the HASH-MISMATCH path — the distinct
///      `ComponentHashMismatch` variant (R-0021-f), NOT the signature path
///      (the manifest signature still verifies) and NOT the benign
///      `ComponentLoad` class
///  AND the error names the plugin with no pool or instance constructed
///      (`Err` return IS the no-construction observable — R-0021-e).
#[test]
fn swapped_component_bytes_are_rejected_on_the_hash_mismatch_path() {
    // Different bytes, same length class: flip the last byte of the real
    // artifact. The hash gate fires before any wasm decode, so validity of
    // the tampered bytes as a component is irrelevant.
    let mut tampered = committed_artifact_bytes();
    let last = tampered.len() - 1;
    tampered[last] ^= 0xFF;

    let replica = replica_with_component(&tampered);

    let err = populate_verified_pool_from_dir(
        replica.path(),
        mnemra_host::signing::root_material::ROOT,
    )
    .err()
    .expect("R-0021-e: swapped component bytes must be rejected — got Ok (gate absent/bypassed)");

    match &err {
        StartupError::ComponentHashMismatch(msg) => {
            assert!(
                msg.contains("mnemra-echo"),
                "R-0005-b: the hash-mismatch error must name the plugin; got: {msg}"
            );
        }
        other => panic!(
            "R-0021-f: a post-signing swap must surface the distinct ComponentHashMismatch \
             variant (tamper signal), not the signature or benign load class; got: {other:?}"
        ),
    }
}
