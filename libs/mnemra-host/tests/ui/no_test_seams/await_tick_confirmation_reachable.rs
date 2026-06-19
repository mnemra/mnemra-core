// Fixture: seam mnemra_host::plugin::epoch_thread::EpochTickThread::await_tick_confirmation_for_test
//
// This file MUST NOT compile once the `test-hooks` feature gate is in place.
// It is expected to compile today (seam is pub) — trybuild reports that as the
// red failure ("expected compile-fail but it compiled").

use mnemra_host::plugin::epoch_thread::EpochTickThread;

fn main() {
    let _: fn(&EpochTickThread) = EpochTickThread::await_tick_confirmation_for_test;
}
