//! Acceptance tests — resource-limit trap handling, kill-and-replace (Task 22, RED).
//!
//! # What this file pins (the contract)
//!
//! When a plugin invocation breaches a resource limit (epoch deadline or fuel),
//! the host SHALL catch the Wasmtime trap, emit a structured
//! `plugin_limit_violation` event, poison-and-replace the pool slot synchronously,
//! and return a structured error to the caller — without panicking the host
//! process (R-0007-e, R-0007-f, R-0016-c). The epoch supervisor SHALL not flip
//! healthy until a restart is confirmed ticking, and SHALL not panic on a
//! poisoned lock (R-0007-h + #1690 hardening).
//!
//! # Status: RED by design (verify = [])
//!
//! These tests reference a not-yet-existing public `plugin::trap_recovery` module
//! and new `pub` surface on `PluginPool` / `EpochTickThread`. The crate therefore
//! will NOT compile — that compile failure IS the correct TDD red. The GREEN phase
//! implements the proposed seam (see `## PROPOSED PUBLIC API SEAM` in the dispatch
//! report) and these tests then compile and run.
//!
//! # The load-bearing test-design seam (why the tests look the way they do)
//!
//! The production limits are SLOW: `EPOCH_DEADLINE = 500` ticks × 10ms = 5 s wall
//! clock; `FUEL_LIMIT = 10_000_000_000`. A test that waits 5 s or burns 10B fuel
//! is un-fast-able. So each test triggers the trap FAST with a small per-store
//! budget (`ResourceBudget`), while asserting that the EMITTED event reports the
//! POLICY CONSTANT (`limit_value == 500` / `== 10_000_000_000`), decoupled from
//! the trigger budget. A separate policy assertion pins that the production
//! constants ARE the spec values. Two assertions per path:
//!   (mechanism) the event reports the policy constant, decoupled from the trigger;
//!   (policy)    the policy constant equals the spec value.
//! Shrinking the trigger budget for speed therefore cannot hide a wrong production
//! limit.
//!
//! # Epoch ⇆ fuel independence (spec-mandated "tested independently")
//!
//! The two paths are isolated by the STORE-CONFIG SPLIT, not by differing module
//! text:
//!   - epoch path: `ResourceBudget { fuel: <non-binding/large>, epoch_deadline: <small> }`
//!     — fuel must be set NON-binding, because with `consume_fuel(true)` a store
//!     with no fuel defaults to ~0 → it would trap on FUEL and silently violate
//!     independence. The running 10ms tick thread drives the small epoch deadline.
//!   - fuel path: `ResourceBudget { fuel: <small>, epoch_deadline: <large> }`
//!     — a short CPU-burn exhausts the small fuel budget in milliseconds before
//!     the large epoch deadline can fire.
//! Each test asserts its `limit_type` so a wrong-limit trap (e.g. the "epoch"
//! store running out of fuel first) fails the test LOUDLY — fail-shut on fixture
//! correctness (the Task-22 analog of "fixtures must pass verify first").

#[cfg(feature = "test-hooks")]
use mnemra_host::plugin::epoch_thread::{EpochTickThread, HealthState};
#[cfg(feature = "test-hooks")]
use mnemra_host::plugin::limits::build_engine;
use mnemra_host::plugin::limits::{EPOCH_DEADLINE, FUEL_LIMIT};
use mnemra_host::plugin::pool::PluginPool;
use mnemra_host::plugin::trap_recovery::{LimitType, PluginExecError, ResourceBudget};

use wasmtime::component::Component;

// ---------------------------------------------------------------------------
// CC-POOL-TESTS-REPIN — fixtures converted from core-module WAT to COMPONENT WAT
//
// The pool migrated from core `wasmtime::Module` to `component::Component`
// (Task 23). These trap fixtures are therefore COMPONENTS: each wraps a core
// module whose `run` loops/burns, lifted to a parameterless world-level
// `run: func()` export via `canon lift`. The trap-recovery seam invokes this
// `run` through the SAME component-instance path as the real plugin (real
// `component::Component`, real `Store<HostState>`, real component instantiation
// via the Linker, real component trap) — not the retired core-module seam.
//
// NO ASSERTION CHANGES: the trap behaviour (epoch loop / fuel burn -> Wasmtime
// trap -> classify -> kill-and-replace -> structured error) is identical to the
// core-module fixtures. Only the fixture CONTAINER (core module -> component) and
// the invoke entrypoint (core `run` -> component-lifted `run`) move. Trap
// classification is at the `wasmtime::Trap` level (epoch/fuel), which is
// container-independent, so every R-0007-a/b/c/g assertion below is preserved.
// Fixtures validated independently via `wasm-tools validate`.
// ---------------------------------------------------------------------------

/// Infinite-loop COMPONENT for the EPOCH-deadline trap path.
///
/// The inner core module's `run` enters an unconditional infinite loop and never
/// returns; it is lifted to a world-level `run: func()` component export. With
/// the store configured for a small epoch deadline and a NON-binding fuel budget,
/// the running epoch-tick thread drives the epoch deadline and Wasmtime raises an
/// epoch-interruption trap. Fuel is non-binding so the epoch deadline is the
/// binding limit (independence from the fuel path).
const EPOCH_LOOP_WAT: &str = r#"(component
  (core module $m
    (func (export "run")
      (loop $forever
        br $forever)))
  (core instance $i (instantiate $m))
  (func (export "run") (canon lift (core func $i "run"))))"#;

/// CPU-burn COMPONENT for the FUEL-exhaustion trap path.
///
/// The inner core module's `run` spins a counter upward forever, executing
/// fuel-consuming arithmetic on every iteration WITHOUT sleeping or yielding;
/// it is lifted to a world-level `run: func()` component export. With a small
/// fuel budget and a NON-binding (large) epoch deadline, the fuel budget is
/// exhausted first and Wasmtime raises an `OutOfFuel` trap before the epoch
/// deadline can fire (independence from the epoch path).
const FUEL_BURN_WAT: &str = r#"(component
  (core module $m
    (func (export "run")
      (local $i i64)
      (loop $burn
        (local.set $i (i64.add (local.get $i) (i64.const 1)))
        (br $burn))))
  (core instance $i (instantiate $m))
  (func (export "run") (canon lift (core func $i "run"))))"#;

// Synthetic test plugin identity (anti-silent-fill: the `tasks`/`task.list`/
// `0.2.0`/`W1` strings in the spec scenarios DO NOT EXIST — Task 20 built only
// `mnemra-echo`. What is CONTRACTUAL is the event SHAPE + exact limit_type/
// limit_value + the error code, NOT the literal identity strings. We use a
// synthetic `burn` plugin identity here.)
const TEST_PLUGIN_ID: &str = "burn";
const TEST_PLUGIN_VERSION: &str = "0.0.1";
const TEST_VERB: &str = "loop";

/// Synthetic workspace marker for the event `workspace_id` field. The literal
/// value is not contractual — only that the emitted event echoes the workspace
/// the invocation ran under (the spec scenario's `W1` is a placeholder).
const TEST_WORKSPACE_MARKER: u128 = 0x5701; // mnemonic for "W1"

/// Compile a component-WAT fixture into a `component::Component` on the pool's
/// engine.
///
/// The pool's engine has `consume_fuel(true)` + `epoch_interruption(true)` +
/// `component-model` (per `build_engine`), and default features keep `wat` ON, so
/// component-WAT text compiles directly. A compile failure here means a malformed
/// fixture — not a test of the trap path — so we surface it loudly.
fn compile_fixture(pool: &PluginPool, wat: &str) -> Component {
    Component::new(pool.engine(), wat)
        .expect("pathological component-WAT fixture must compile to a Component")
}

/// Build a fresh pool and register the given fixture as a live, trappable
/// component under the synthetic test identity. Returns the pool.
///
/// This exercises the `register_module` seam — the live-component pool population
/// path (production wires the built echo `.wasm` component in T11; this suite
/// builds component-WAT -> pool -> invoke -> trap -> replace).
fn pool_with_fixture(wat: &str) -> PluginPool {
    let pool = PluginPool::new().expect("pool must initialise");
    let component = compile_fixture(&pool, wat);
    pool.register_module(TEST_PLUGIN_ID, TEST_PLUGIN_VERSION, &component)
        .expect("register_module must populate live slots for the fixture");
    pool
}

// A non-binding fuel budget for the epoch path: u64::MAX so the infinite loop
// CANNOT exhaust it before the (small) epoch deadline fires. Maximised on purpose
// to give the epoch deadline a wide margin — on fast hardware a tight `loop br`
// burns a surprising amount of fuel in a few 10ms ticks, and we never want the
// epoch fixture to trap on fuel by accident (the wrong-limit assertion would catch
// it loudly, but a spurious loud failure burns GREEN cycles).
const NON_BINDING_FUEL: u64 = u64::MAX;
// A small epoch deadline for the epoch path: 1 tick → trap on the very next 10ms
// epoch increment, well before any fuel concern.
const FAST_EPOCH_DEADLINE: u64 = 1;
// A small fuel budget for the fuel path: a short CPU-burn exhausts it in ms.
const FAST_FUEL_BUDGET: u64 = 100_000;
// A non-binding epoch deadline for the fuel path: large enough that the 10ms
// tick thread cannot reach it before the small fuel budget is exhausted.
const NON_BINDING_EPOCH_DEADLINE: u64 = 100_000;

// ===========================================================================
// R-0007-e / R-0007-f / R-0016-c — EPOCH-deadline breach → kill-and-replace
// ===========================================================================

/// R-0007-e, R-0007-f, R-0016-c — epoch-deadline breach traps, kills, replaces.
///
/// Given a `burn` plugin instance running an infinite-loop verb, with the store
///   configured so the EPOCH deadline is binding (fuel non-binding),
/// When the epoch-interruption deadline fires,
/// Then the Wasmtime store traps with an epoch-deadline error; the host catches
///   the trap; a structured `plugin_limit_violation` event is emitted carrying
///   `limit_type == "epoch_deadline"` and `limit_value == EPOCH_DEADLINE` (the
///   POLICY constant, decoupled from the small trigger deadline); the pool slot
///   is poisoned and a new instance is created synchronously; the caller receives
///   a structured `PluginExecError` whose `code() == "plugin_execution_timeout"`;
///   and the host process does NOT panic.
#[test]
fn epoch_breach_traps_and_kills_and_replaces() {
    // Given — a pool with the infinite-loop fixture registered live.
    let pool = pool_with_fixture(EPOCH_LOOP_WAT);

    // Epoch is binding; fuel is NON-binding (critical for independence: with
    // consume_fuel(true) a store with ~0 fuel would trap on FUEL, not epoch).
    let budget = ResourceBudget {
        fuel: NON_BINDING_FUEL,
        epoch_deadline: FAST_EPOCH_DEADLINE,
    };
    let ws = uuid::Uuid::from_u128(TEST_WORKSPACE_MARKER);

    // When — invoke through the recovery path; the infinite loop hits the epoch
    // deadline driven by the running 10ms tick thread.
    let result = mnemra_host::plugin::trap_recovery::invoke_with_recovery(
        &pool,
        TEST_PLUGIN_ID,
        TEST_VERB,
        budget,
        ws,
    );

    // Then — the caller receives a structured error, never a panic.
    let err = result.expect_err("epoch breach must surface as a structured Err, not Ok");

    // Wrong-limit guard (fail-shut on fixture correctness): the violation MUST be
    // the epoch deadline. If this is `Fuel`, the "epoch" store ran out of fuel
    // first — a false-green fixture bug, fail loudly.
    let violation = err
        .limit_violation()
        .expect("an epoch breach error must carry a PluginLimitViolation");
    assert_eq!(
        violation.limit_type,
        LimitType::EpochDeadline,
        "epoch fixture must trap on the EPOCH deadline, not fuel — wrong-limit trap is a false green"
    );

    // Mechanism: the event reports the POLICY constant, decoupled from the small
    // FAST_EPOCH_DEADLINE we used to trigger it quickly.
    assert_eq!(
        violation.limit_value, EPOCH_DEADLINE,
        "emitted limit_value must be the policy constant ({EPOCH_DEADLINE}), \
         not the trigger deadline ({FAST_EPOCH_DEADLINE})"
    );
    assert_ne!(
        violation.limit_value, FAST_EPOCH_DEADLINE,
        "limit_value must NOT leak the trigger budget — decoupling is the contract"
    );

    // Event identity fields carry the (synthetic) plugin identity + workspace.
    assert_eq!(violation.plugin_id, TEST_PLUGIN_ID);
    assert_eq!(violation.plugin_version, TEST_PLUGIN_VERSION);
    assert_eq!(violation.workspace_id, ws);

    // Caller error code is the spec-named timeout code for the epoch path.
    assert_eq!(
        err.code(),
        "plugin_execution_timeout",
        "epoch breach caller error code is spec-named 'plugin_execution_timeout'"
    );
    assert_eq!(err.plugin(), TEST_PLUGIN_ID);
    assert_eq!(err.verb(), TEST_VERB);
}

/// R-0007-h policy counterweight (epoch) — the production EPOCH_DEADLINE constant
/// equals the spec value (500 ticks = 5 s). This is the policy half of the two
/// assertions: shrinking the trigger budget for test speed must NOT be able to
/// hide a wrong production limit.
#[test]
fn epoch_policy_constant_is_spec_value() {
    // The pool's accessor returns the policy constant used to configure stores.
    assert_eq!(
        PluginPool::epoch_deadline(),
        500,
        "production epoch deadline must be 500 ticks (5 s at 10ms/tick) per R-0007-b"
    );
    assert_eq!(
        EPOCH_DEADLINE, 500,
        "EPOCH_DEADLINE constant must equal the spec value (500)"
    );
}

// ===========================================================================
// R-0007-e / R-0007-f / R-0016-c — FUEL exhaustion → kill-and-replace
//   (independent of the epoch path)
// ===========================================================================

/// R-0007-e, R-0007-f, R-0016-c — fuel exhaustion traps, kills, replaces.
///
/// Given a `burn` plugin instance running a CPU-burn verb, with the store
///   configured so the FUEL budget is binding (epoch deadline non-binding),
/// When the fuel ceiling is hit,
/// Then the Wasmtime store traps with a fuel-exhaustion error; the SAME
///   kill-and-replace path fires as the epoch scenario; a structured
///   `plugin_limit_violation` event is emitted carrying `limit_type == "fuel"`
///   and `limit_value == FUEL_LIMIT` (the POLICY constant, decoupled from the
///   small trigger budget); the pool recovers; the caller receives a structured
///   `PluginExecError` whose `code() == "plugin_resource_exhausted"` (a PROPOSED
///   fuel code — the spec under-specifies the fuel caller error; see report
///   spec-gap flag); and the host process does NOT panic.
///
/// Independence: this test sets a small fuel budget and a NON-binding (large)
/// epoch deadline so the epoch deadline cannot fire first.
#[test]
fn fuel_exhaustion_traps_and_kills_and_replaces_independent_of_epoch() {
    // Given — a pool with the CPU-burn fixture registered live.
    let pool = pool_with_fixture(FUEL_BURN_WAT);

    // Fuel is binding; epoch deadline is NON-binding (independence from epoch).
    let budget = ResourceBudget {
        fuel: FAST_FUEL_BUDGET,
        epoch_deadline: NON_BINDING_EPOCH_DEADLINE,
    };
    let ws = uuid::Uuid::from_u128(TEST_WORKSPACE_MARKER);

    // When — invoke; the CPU-burn loop exhausts the small fuel budget in ms,
    // before the large epoch deadline can be reached by the 10ms tick thread.
    let result = mnemra_host::plugin::trap_recovery::invoke_with_recovery(
        &pool,
        TEST_PLUGIN_ID,
        TEST_VERB,
        budget,
        ws,
    );

    // Then — structured error, never a panic.
    let err = result.expect_err("fuel exhaustion must surface as a structured Err, not Ok");

    // Wrong-limit guard (fail-shut on fixture correctness): the violation MUST be
    // fuel. If this is `EpochDeadline`, the "fuel" store hit the epoch deadline
    // first — a false-green fixture bug, fail loudly.
    let violation = err
        .limit_violation()
        .expect("a fuel exhaustion error must carry a PluginLimitViolation");
    assert_eq!(
        violation.limit_type,
        LimitType::Fuel,
        "fuel fixture must trap on FUEL, not the epoch deadline — wrong-limit trap is a false green"
    );

    // Mechanism: the event reports the POLICY constant, decoupled from the small
    // FAST_FUEL_BUDGET we used to trigger it quickly.
    assert_eq!(
        violation.limit_value, FUEL_LIMIT,
        "emitted limit_value must be the policy constant ({FUEL_LIMIT}), \
         not the trigger budget ({FAST_FUEL_BUDGET})"
    );
    assert_ne!(
        violation.limit_value, FAST_FUEL_BUDGET,
        "limit_value must NOT leak the trigger budget — decoupling is the contract"
    );

    assert_eq!(violation.plugin_id, TEST_PLUGIN_ID);
    assert_eq!(violation.plugin_version, TEST_PLUGIN_VERSION);
    assert_eq!(violation.workspace_id, ws);

    // PROPOSED fuel caller error code (spec gap — flagged in report). NOT the
    // timeout code: fuel exhaustion is a compute-budget breach, semantically not
    // a timeout.
    assert_eq!(
        err.code(),
        "plugin_resource_exhausted",
        "fuel breach caller error code is the PROPOSED 'plugin_resource_exhausted' (spec gap)"
    );
    assert_eq!(err.plugin(), TEST_PLUGIN_ID);
    assert_eq!(err.verb(), TEST_VERB);
}

/// R-0007-a policy counterweight (fuel) — the production FUEL_LIMIT constant
/// equals the spec value (10_000_000_000). Policy half of the two assertions.
#[test]
fn fuel_policy_constant_is_spec_value() {
    assert_eq!(
        PluginPool::fuel_limit(),
        10_000_000_000,
        "production fuel limit must be 10 billion ticks per R-0007-a"
    );
    assert_eq!(
        FUEL_LIMIT, 10_000_000_000,
        "FUEL_LIMIT constant must equal the spec value (10_000_000_000)"
    );
}

// ===========================================================================
// R-0016-c — pool size is preserved across a kill-and-replace
// ===========================================================================

/// R-0016-c — a killed-and-replaced instance is replaced synchronously; the pool
/// size does not decrease as a result of a kill event.
///
/// Given a pool registered with the epoch fixture (POOL_MIN live slots),
/// When a verb invocation breaches a limit and the slot is poisoned-and-replaced,
/// Then the pool size for that plugin is EQUAL before and after — by the time the
///   caller's structured error returns, the slot is already replaced (synchronous
///   replacement, R-0016-c).
#[test]
fn pool_size_preserved_across_kill_and_replace() {
    // Given — a pool with live slots for the fixture.
    let pool = pool_with_fixture(EPOCH_LOOP_WAT);
    let size_before = pool.slot_count(TEST_PLUGIN_ID);
    assert!(
        size_before >= 3,
        "pool must be initialised with at least POOL_MIN (3) slots; got {size_before}"
    );

    // When — a limit breach triggers kill-and-replace.
    let budget = ResourceBudget {
        fuel: NON_BINDING_FUEL,
        epoch_deadline: FAST_EPOCH_DEADLINE,
    };
    let ws = uuid::Uuid::from_u128(TEST_WORKSPACE_MARKER);
    let _ = mnemra_host::plugin::trap_recovery::invoke_with_recovery(
        &pool,
        TEST_PLUGIN_ID,
        TEST_VERB,
        budget,
        ws,
    )
    .expect_err("the breach must return an Err");

    // Then — pool size is preserved (the replacement is synchronous, already done
    // by the time the error returned).
    let size_after = pool.slot_count(TEST_PLUGIN_ID);
    assert_eq!(
        size_before, size_after,
        "pool size must not decrease as a result of a kill event (R-0016-c): \
         before={size_before} after={size_after}"
    );
}

// ===========================================================================
// R-0007-f — the host process does NOT panic on a trap; recovers + serves again
// ===========================================================================

/// R-0007-f — a Wasmtime trap is NEVER propagated as a host-process panic;
/// kill-and-replace is the recovery invariant, and the host survives to serve a
/// subsequent invocation.
///
/// Given a pool with the epoch fixture,
/// When a limit-breaching invocation runs inside `catch_unwind`,
/// Then no panic escapes (the closure returns `Ok`); the call yielded a
///   structured `Err`; AND a SECOND invocation on the same pool also returns a
///   structured `Err` (the host did not abort and the pool still serves).
#[test]
fn trap_does_not_panic_host_and_pool_serves_again() {
    let pool = pool_with_fixture(EPOCH_LOOP_WAT);
    let budget = ResourceBudget {
        fuel: NON_BINDING_FUEL,
        epoch_deadline: FAST_EPOCH_DEADLINE,
    };
    let ws = uuid::Uuid::from_u128(TEST_WORKSPACE_MARKER);

    // When — wrap the invocation in catch_unwind to PROVE no panic escaped the
    // recovery path. A trap that propagated as a panic would make this `Err`.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        mnemra_host::plugin::trap_recovery::invoke_with_recovery(
            &pool,
            TEST_PLUGIN_ID,
            TEST_VERB,
            budget,
            ws,
        )
    }));

    // Then — no panic escaped; the inner result is a structured Err.
    let first = outcome.expect("a Wasmtime trap must NOT propagate as a host panic (R-0007-f)");
    assert!(
        first.is_err(),
        "the breaching invocation must return a structured Err"
    );

    // And — the host survived; a second invocation still returns a structured Err
    // (the pool was not left in an aborted/unusable state).
    let second = mnemra_host::plugin::trap_recovery::invoke_with_recovery(
        &pool,
        TEST_PLUGIN_ID,
        TEST_VERB,
        ResourceBudget {
            fuel: NON_BINDING_FUEL,
            epoch_deadline: FAST_EPOCH_DEADLINE,
        },
        ws,
    );
    // The second invocation returns the structured `PluginExecError` type (not a
    // panic, not a different error). Asserting `code()` is accessible proves the
    // returned value is the proposed structured type.
    let second_err: PluginExecError =
        second.expect_err("the host must still serve after a trap recovery (pool not aborted)");
    assert_eq!(
        second_err.code(),
        "plugin_execution_timeout",
        "second epoch breach returns the same structured PluginExecError type + code"
    );
}

// ===========================================================================
// #1690 (a) — R-0007-h confirm-restart: health flips to Ok only AFTER a
//   post-restart tick is confirmed, not optimistically before the restart
//   succeeds.
// ===========================================================================
//
// BLACK-BOX NOTE: provoking a tick-thread DEATH and OBSERVING a confirmed tick
// is not reachable through today's public surface (the thread death and the
// optimistic flip are internal). This test is written against the PROPOSED
// public observation seam (see report): a `pub` death-injection hook and a `pub`
// post-restart confirmed-tick observation. If GREEN chooses a different seam, it
// updates this test; the CONTRACT pinned here is "no flip-to-Ok before a
// confirmed post-restart tick."

/// #1690-a (R-0007-h) — supervised restart confirms a tick before reporting Ok.
///
/// Given an epoch-tick thread that has died (health == Degraded),
/// When a supervised restart is attempted,
/// Then `is_healthy()` / `can_invoke()` returns `false` until a post-restart tick
///   is CONFIRMED observed — NOT optimistically flipped to Ok before the new
///   thread is confirmed ticking. Health becomes Ok only after the confirmed tick.
#[cfg(feature = "test-hooks")]
#[test]
fn supervised_restart_confirms_tick_before_reporting_healthy() {
    let engine = build_engine().expect("engine must build");
    let thread = EpochTickThread::start(engine);

    // Given — the thread has died (PROPOSED pub death-injection hook for tests;
    // real deaths are panics inside the tick loop, not reachable from outside).
    thread.inject_death_for_test();
    // Health must reflect the death.
    assert_eq!(
        thread.health_state(),
        HealthState::Degraded,
        "a dead tick thread must report Degraded"
    );
    assert!(
        !thread.can_invoke(),
        "invocations must be refused while the thread is dead (R-0007-h)"
    );

    // When — a supervised restart is attempted.
    let mut last_restart = None;
    let restarted = thread.try_restart(&mut last_restart);
    assert!(
        restarted,
        "restart should be attempted when degraded + backoff ok"
    );

    // Then — health must NOT be Ok yet: the spec requires confirmation that the
    // thread is ticking again before accepting invocations. The current
    // implementation flips to Ok OPTIMISTICALLY inside try_restart — this
    // assertion pins that that optimistic flip is WRONG.
    assert!(
        !thread.tick_confirmed_since_restart(),
        "no post-restart tick can be confirmed synchronously at the instant of restart"
    );
    assert!(
        !thread.is_healthy(),
        "R-0007-h: health must stay non-Ok until a post-restart tick is CONFIRMED, \
         not flip to Ok optimistically before the restart is confirmed ticking"
    );

    // After a confirmed tick is observed, health becomes Ok and invocations resume.
    thread.await_tick_confirmation_for_test();
    assert!(
        thread.tick_confirmed_since_restart(),
        "a post-restart tick must eventually be confirmed by the live thread"
    );
    assert!(
        thread.is_healthy(),
        "once a post-restart tick is confirmed, health returns to Ok (R-0007-h)"
    );
    assert!(
        thread.can_invoke(),
        "invocations resume only after the restart is confirmed (R-0007-h)"
    );
}

// ===========================================================================
// #1690 (b) — lock-poison recovery (hardening nit, lower priority): a poisoned
//   health-state mutex must recover (return Degraded / not panic) rather than
//   the current `.expect(...)` panic.
// ===========================================================================
//
// BLACK-BOX NOTE: poisoning the internal health-state mutex is not reachable
// from outside the crate. This test is written against a PROPOSED pub poison-
// injection hook (see report). If GREEN cannot expose poison injection safely,
// it may instead prove non-panic via the death path; the CONTRACT pinned here is
// "health_state() does not panic on a poisoned lock — it degrades."

/// #1690-b — `health_state()` does not panic on a poisoned health-state lock.
///
/// Given an epoch-tick thread whose internal health-state mutex has been poisoned,
/// When `health_state()` is queried,
/// Then it returns a value (degraded, fail-safe) and does NOT panic — replacing
///   the current `.expect("...poisoned")` which aborts the host on a poisoned
///   lock.
#[cfg(feature = "test-hooks")]
#[test]
fn health_state_does_not_panic_on_poisoned_lock() {
    let engine = build_engine().expect("engine must build");
    let thread = EpochTickThread::start(engine);

    // Given — the internal health-state mutex is poisoned (PROPOSED pub hook).
    thread.poison_health_lock_for_test();

    // When/Then — querying health must not panic; wrap in catch_unwind to prove it.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| thread.health_state()));
    let state =
        outcome.expect("health_state() must NOT panic on a poisoned lock (#1690-b hardening)");

    // A poisoned lock is a fail-safe condition: report Degraded, refuse invocations.
    assert_eq!(
        state,
        HealthState::Degraded,
        "a poisoned health lock must degrade (fail-safe), not be reported as Ok"
    );
}

// ===========================================================================
// T10 AC(c) — POOL_MAX pre-instantiation + reject-duplicate-register (R-0016-a)
// ===========================================================================

/// T10 AC(c), R-0016-a — `register_module` pre-instantiates `POOL_MAX` (5)
/// live instances, not just `POOL_MIN` (3).
///
/// Given a component registered via `register_module`,
/// When the pool is inspected via `slot_count`,
/// Then the live instance count for that plugin == `POOL_MAX` (5).
///
/// Puck-locked interpretation (V0.1+ scope note): adaptive/runtime pool growth is
/// V0.1+ scope; V0 "pool grows to POOL_MAX" means registration pre-instantiates
/// the full ceiling (5 instances), not the floor (3). The spec range is 3–5; V0
/// pins the ceiling at registration time, not at runtime.
///
/// RED: current code instantiates only `POOL_MIN` (3) slots. This test FAILS
/// with `slot_count == 3`, not 5.
#[test]
fn register_module_pre_instantiates_pool_max_live_instances() {
    use mnemra_host::plugin::pool::POOL_MAX;

    // Given — a pool with the fixture registered via register_module (POOL_MIN
    // slots populated today; the test asserts the spec-required POOL_MAX).
    let pool = pool_with_fixture(EPOCH_LOOP_WAT);

    // When — inspect the live slot count for the registered plugin.
    let count = pool.slot_count(TEST_PLUGIN_ID);

    // Then — exactly POOL_MAX (5) live instances. Do NOT use >= 3: that passes
    // vacuously with the current broken floor. The contract is the ceiling.
    // Right-reason red: count is 3 (POOL_MIN), asserted value is 5 (POOL_MAX).
    assert_eq!(
        count, POOL_MAX,
        "register_module must pre-instantiate POOL_MAX ({POOL_MAX}) live instances \
         (R-0016-a, T10 AC(c)); got {count} — current impl instantiates only POOL_MIN (3)"
    );
}

/// T10 AC(c), R-0016-a — `register_module` rejects a duplicate `plugin_name`
/// and leaves the pool state intact.
///
/// Given a plugin already registered via `register_module`,
/// When `register_module` is called again with the SAME `plugin_name`,
/// Then the second call returns `Err(...)` AND the pool's slot count is
///   unchanged (original instances intact, no duplicate entry created).
///
/// # Missing-accessor note (flagged for green phase)
///
/// `pool.slot_count()` uses `.find()` and returns the FIRST matching entry's
/// slot count. It cannot distinguish "one entry with N slots" from "two entries
/// with the same name, N slots each." The count assertion below (`slots_after ==
/// slots_before`) is therefore the strongest check possible against the current
/// public surface — it catches a doubled slot count but NOT a duplicate
/// zero-count entry. The green phase implementer should add:
///
/// ```text
/// pub fn registered_entry_count(&self, plugin_name: &str) -> usize
/// ```
///
/// returning the number of `LiveModuleEntry` records with that name, so a test
/// can assert `== 1` (exactly one entry) after a rejected duplicate.
///
/// RED: current code returns `Ok` unconditionally on the second call and pushes
/// a second `LiveModuleEntry` for the same name. The primary failing assertion
/// is `result.is_err()`.
#[test]
fn register_module_rejects_duplicate_plugin_name() {
    // Given — first registration succeeds (pool holds one entry, POOL_MIN slots).
    let pool = pool_with_fixture(EPOCH_LOOP_WAT);
    let slots_before = pool.slot_count(TEST_PLUGIN_ID);
    assert!(
        slots_before >= 3,
        "precondition: first registration must yield at least POOL_MIN (3) slots; \
         got {slots_before}"
    );

    // Compile a second Component instance from the same WAT (same plugin identity).
    let component = compile_fixture(&pool, EPOCH_LOOP_WAT);

    // When — attempt to register the same plugin_name a second time.
    let result = pool.register_module(TEST_PLUGIN_ID, TEST_PLUGIN_VERSION, &component);

    // Then — second call must return Err (duplicate is rejected, R-0016-a).
    // Right-reason red: current impl returns Ok unconditionally and pushes a
    // second LiveModuleEntry, accepting the duplicate.
    assert!(
        result.is_err(),
        "register_module must return Err when plugin '{TEST_PLUGIN_ID}' is already \
         registered; got Ok — current impl accepts duplicates unconditionally (R-0016-a)"
    );

    // Then — slot count is unchanged (original instances intact, no doubled allocation).
    // Limitation: slot_count cannot distinguish 1 vs 2 entries — see doc above.
    // This assertion catches a doubling but not an empty-count duplicate entry.
    let slots_after = pool.slot_count(TEST_PLUGIN_ID);
    assert_eq!(
        slots_before, slots_after,
        "pool slot count must not change after a rejected duplicate register: \
         before={slots_before}, after={slots_after}"
    );
}
