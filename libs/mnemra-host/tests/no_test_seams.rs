//! Gate test: epoch test-injection seams must NOT be reachable in the default build.
//!
//! # Contract (task #1702, Warden dispatch 1041 condition 1)
//!
//! The five `EpochTickThread` test-injection hooks — `inject_death_for_test`,
//! `poison_health_lock_for_test`, `await_tick_confirmation_for_test`,
//! `tick_confirmed_since_restart`, and `can_invoke` — are currently `pub
//! #[doc(hidden)]` with no cfg gating. Once Task 23 wires untrusted MCP
//! dispatch into the same process, these in-process-callable poison/kill
//! methods must not coexist with an untrusted path. Forge will gate them behind
//! a non-default cargo feature named `test-hooks` (green phase).
//!
//! # Red / Green status
//!
//! **RED NOW (by design).** Each UI fixture calls one seam as a typed
//! function-item reference — the minimal snippet that compiles iff the seam is
//! reachable. Today they compile (seams are still `pub`) so trybuild reports
//! "expected compile-fail but it compiled" for all five → this test FAILS.
//!
//! **GREEN after Forge gates the seams** behind `#[cfg(feature = "test-hooks")]`.
//! The fixtures then fail to compile → trybuild confirms compile-fail →
//! the test PASSES. Forge's green dispatch needs to run
//! `TRYBUILD=overwrite cargo test test_epoch_seams_absent_in_default_build`
//! to capture the `.stderr` snapshots (under `tests/ui/no_test_seams/*.stderr`).
//!
//! # Falsifiability
//!
//! The mechanism is self-enforcing: re-exposing any seam (removing its
//! `#[cfg(feature = "test-hooks")]` guard) causes its fixture to compile again
//! → trybuild reports "expected compile-fail but it compiled" → test fails.
//! No human audit required.
//!
//! # Shared tokens (locked with green phase, do not rename)
//!
//! The feature is named exactly `test-hooks`. The `cfg!(feature = "test-hooks")`
//! guard below is the coordination point — green does not edit this file.

#[test]
fn test_epoch_seams_absent_in_default_build() {
    // When the `test-hooks` feature is explicitly enabled (e.g., running the
    // dedicated resource-limits test suite via `cargo test --features test-hooks`),
    // the seams are intentionally present. Skip this gate — it only enforces
    // the DEFAULT build constraint.
    if cfg!(feature = "test-hooks") {
        return;
    }

    // Each `compile_fail` fixture takes a typed function-item reference to one
    // seam. The reference compiles iff the symbol is reachable at the call site.
    // Today (seams are `pub`): fixtures compile → trybuild fails this test ← RED.
    // After green (seams gated behind `test-hooks`): fixtures fail to compile
    // → trybuild passes this test ← GREEN.
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/no_test_seams/inject_death_reachable.rs");
    t.compile_fail("tests/ui/no_test_seams/poison_health_lock_reachable.rs");
    t.compile_fail("tests/ui/no_test_seams/await_tick_confirmation_reachable.rs");
    t.compile_fail("tests/ui/no_test_seams/tick_confirmed_since_restart_reachable.rs");
    t.compile_fail("tests/ui/no_test_seams/can_invoke_epoch_seam_reachable.rs");
}
