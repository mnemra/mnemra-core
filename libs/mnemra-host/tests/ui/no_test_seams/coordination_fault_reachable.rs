// Fixture: the Task 3 coordination-write fault-injection seam
// mnemra_host::coordination::write_path::{CoordinationFault,
// PgCoordinationStore::with_injected_fault} (R-0074-b / R-0075-c).
//
// This file MUST NOT compile in the default build: the seam is born gated
// behind `#[cfg(feature = "test-hooks")]` in the Task 3 RED phase (like the T5
// startup `InjectedFailure` seam), so trybuild confirms compile-fail from day
// one. Re-exposing the seam (removing its cfg guard) makes this fixture compile
// again and fails the gate — a coordination fault seam leaking into the
// production build is a silent-failure class (no runtime signal when it fires
// in prod), so it is structuralized here rather than trusted to review.
//
// When compiled under `--features test-hooks` the seam is intentionally
// present; the harness test skips fixtures under that flag, so this file is
// never reached then.

use mnemra_host::coordination::write_path::{CoordinationFault, PgCoordinationStore};

fn main() {
    // Name the gated enum — compiles iff `CoordinationFault` is reachable.
    let _fault: CoordinationFault = CoordinationFault::AuditEmitFail;
    // Take a function-item reference to the injector setter — compiles iff the
    // seam is pub-reachable.
    let _: fn(PgCoordinationStore, CoordinationFault) -> PgCoordinationStore =
        PgCoordinationStore::with_injected_fault;
}
