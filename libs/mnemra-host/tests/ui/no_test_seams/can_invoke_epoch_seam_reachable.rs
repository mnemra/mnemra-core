// Fixture: seam mnemra_host::plugin::epoch_thread::EpochTickThread::can_invoke
//
// NOTE: This is EpochTickThread::can_invoke (the doc-hidden test seam), NOT
// PluginPool::can_invoke (a legitimate public health-check surface, in pool.rs).
//
// This file MUST NOT compile once the `test-hooks` feature gate is in place.
// It is expected to compile today (seam is pub) — trybuild reports that as the
// red failure ("expected compile-fail but it compiled").

use mnemra_host::plugin::epoch_thread::EpochTickThread;

fn main() {
    let _: fn(&EpochTickThread) -> bool = EpochTickThread::can_invoke;
}
