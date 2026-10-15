//! Component-model async is OFF in the host build (brain #3646; RUSTSEC-2026-0327).
//!
//! RUSTSEC-2026-0327 (async-lifted callback result count unvalidated) is
//! reachable only when a component with an async lift is accepted. The host
//! therefore refuses such components at load. Async is off by the cargo
//! feature (`default-features = false` in `Cargo.toml`) AND by
//! `concurrency_support(false)` in `build_engine`.
//!
//! What this test pins: the async-lift fixture is rejected with the
//! validator's feature-gate message, not for some other fixture fault. The
//! fixture's function type is itself async, so with the gate lifted the
//! validator accepts it (see the mutation result in the #3646 report: enabling
//! the `component-model-async` feature plus `concurrency_support(true)` turns
//! the rejection test red). Measured per mechanism: with the cargo feature on
//! but `concurrency_support(false)`, the component is still rejected, so that
//! line alone holds; with the feature off and `concurrency_support(true)`,
//! `Engine::new` itself errors. The test catches a regression that turns async
//! on in both; it does not attribute the rejection to one mechanism.
//!
//! The sync-lift control shows the engine loads components at all and that
//! the fixture's shape is valid when synchronous; it does not rule out other
//! fixture faults in the async fixture. The exact gate message does that: any
//! other fault would produce a different message. A stackful (`async` without
//! `callback`) fixture is deliberately absent: its rejection would carry the
//! stackful gate message, a different code path. RUSTSEC-2026-0327 is the
//! callback path, so there is no coverage loss for it.

use mnemra_host::plugin::limits::build_engine;
use wasmtime::component::Component;

/// One core function lifted to a parameterless `run` export. `func_type` is
/// spliced into the exported function's type (`async` makes it an async
/// function type); `lift_options` into the `canon lift` options.
fn fixture(core_func: &str, func_type: &str, lift_options: &str) -> String {
    format!(
        r#"(component
  (core module $m
    (func (export "run") {core_func})
    (func (export "cb") (param i32 i32 i32) (result i32) i32.const 0))
  (core instance $i (instantiate $m))
  (func (export "run") {func_type} (canon lift (core func $i "run") {lift_options})))"#
    )
}

/// The validator's gate message for an async function type when the
/// component-model async feature is off (wasmparser 0.254.2). wasmtime 48.0.6
/// requires `wasmparser ^0.254.1`, so it is Cargo.lock that pins the validator
/// version, not wasmtime; re-read this message when the lock moves it.
const ASYNC_GATE_MESSAGE: &str =
    "async component functions require the component model async feature";

#[test]
fn sync_lift_control_loads_on_the_production_engine() {
    let engine = build_engine().expect("engine must build");
    Component::new(&engine, fixture("", "", "")).expect(
        "control: a sync-lift component must load, or the async rejection below proves nothing",
    );
}

#[test]
fn async_lift_with_callback_is_rejected_at_load() {
    let engine = build_engine().expect("engine must build");
    let wat = fixture(
        "(result i32) i32.const 0",
        "async",
        r#"async (callback (core func $i "cb"))"#,
    );
    let err = Component::new(&engine, wat)
        .expect_err("a component with an async lift must be rejected at load");
    let msg = format!("{err:?}");
    assert!(
        msg.contains(ASYNC_GATE_MESSAGE),
        "rejection must be the async feature gate, not another fixture fault; got: {msg}"
    );
}
