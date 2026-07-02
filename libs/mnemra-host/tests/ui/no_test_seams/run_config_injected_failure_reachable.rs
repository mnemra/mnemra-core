// Fixture: seam mnemra_host::RunConfig::with_injected_failure (+ the
// InjectedFailure enum) — the T5 startup failure-injection seam (R-0022-a).
//
// This file MUST NOT compile in the default build: unlike the epoch seams
// (which started `pub` and were gated at green), this seam was born gated
// behind `#[cfg(feature = "test-hooks")]` in the T5 RED phase, so trybuild
// confirms compile-fail from day one. Re-exposing the seam (removing its
// cfg guard) makes this fixture compile again and fails the gate.
//
// When compiled under `--features test-hooks` the seam is intentionally
// present; the harness test skips fixtures under that flag, so this file is
// never reached then.

use mnemra_host::{InjectedFailure, RunConfig};

fn main() {
    // Take a function-item reference — compiles iff the seam is pub-reachable.
    let _: fn(RunConfig, InjectedFailure) -> RunConfig = RunConfig::with_injected_failure;
}
