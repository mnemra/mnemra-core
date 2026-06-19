// Fixture: seam mnemra_host::plugin::epoch_thread::EpochTickThread::inject_death_for_test
//
// This file MUST NOT compile once the `test-hooks` feature gate is in place.
// It is expected to compile today (seam is pub) — trybuild reports that as the
// red failure ("expected compile-fail but it compiled").
//
// When compiled under `--features test-hooks` the seam is intentionally present;
// the harness test skips fixtures under that flag, so this file is never reached
// then.

use mnemra_host::plugin::epoch_thread::EpochTickThread;

fn main() {
    // Take a function-item reference — compiles iff the method is pub-reachable.
    let _: fn(&EpochTickThread) = EpochTickThread::inject_death_for_test;
}
