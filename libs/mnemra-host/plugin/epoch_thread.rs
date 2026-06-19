//! Supervised epoch-tick thread (R-0007-b/h).
//!
//! # Purpose
//!
//! Wasmtime's epoch-interruption mechanism requires a host thread that periodically
//! increments the engine's epoch counter. When a Wasmtime store's epoch deadline is
//! reached, it raises an interruption trap. The tick rate (10 ms per tick) and the
//! per-call deadline (`EPOCH_DEADLINE = 500` ticks = 5 s) are defined in `limits.rs`.
//!
//! # Supervisor behaviour (R-0007-h)
//!
//! - The tick thread starts **before any plugin is invoked** (enforced by `pool.rs`
//!   calling `EpochTickThread::start` during pool initialisation).
//! - On crash (the thread panics or exits unexpectedly):
//!   1. The `HealthState` transitions to `Degraded`.
//!   2. An `epoch_tick_thread_died` event is emitted (via `tracing::error!`).
//!   3. New plugin invocations are refused while the thread is dead.
//!   4. One supervised restart is attempted per minute (60 s backoff).
//! - The thread is NOT silently restarted — a crash is visible in health state.
//! - `/health overall = "degraded"` while the thread is dead. The wiring to the
//!   `/health` HTTP route is deferred to Task 23 (MCP server not yet in scope);
//!   `HealthState` is the queryable mechanism the host can read.
//!
//! # R-0007-h: crash semantics + confirm-restart (#1690-a)
//!
//! "Not silently restarted" means the crash is always logged and health-state
//! changes before any restart is attempted. The restart policy (1/min, backoff)
//! is defence-in-depth; the primary signal is the degraded health state.
//!
//! A supervised restart does NOT flip health back to `Ok` optimistically. Health
//! returns to `Ok` only after the restarted thread CONFIRMS a post-restart tick
//! (`tick_confirmed`). Until then `is_healthy()` / `can_invoke()` stay `false`,
//! and invocations are refused — the restart must be confirmed ticking, not merely
//! attempted (#1690-a).
//!
//! # Lock-poison fail-safe (#1690-b)
//!
//! Every access to the health-state mutex recovers from poisoning fail-safe: a
//! poisoned lock degrades to `Degraded` (refuse invocations) rather than panicking
//! the host with `.expect("...poisoned")`. No reachable `.expect()` on a poisoned
//! health lock remains in this module.

use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use wasmtime::Engine;

use crate::plugin::limits::EPOCH_DEADLINE;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Tick interval — 10 ms between epoch increments (R-0007-b).
const TICK_INTERVAL: Duration = Duration::from_millis(10);

/// Minimum interval between supervised restarts — 60 s (R-0007-h backoff).
const RESTART_BACKOFF: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------------------
// HealthState — queryable by the host
// ---------------------------------------------------------------------------

/// The health state of the epoch-tick thread.
///
/// Queryable at any time via `EpochTickThread::health_state`. The host
/// must refuse plugin invocations while state is `Degraded` (R-0007-h).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    /// Thread is running; epoch interruption is active.
    Ok,
    /// Thread has died; epoch interruption is inactive. Plugin invocations
    /// MUST be refused. `/health overall` → `"degraded"` (wired in Task 23).
    Degraded,
}

// ---------------------------------------------------------------------------
// Restart gate — deterministic confirm-restart (#1690-a)
// ---------------------------------------------------------------------------

/// A gate a restarted tick thread waits on before its first tick.
///
/// When `engaged` is `true`, a freshly-restarted tick thread blocks at the gate
/// and does NOT tick / set `tick_confirmed` / flip health to `Ok` until
/// `release()` is called. This makes the "no flip to Ok before a confirmed tick"
/// assertion deterministic (no sleep race): the test engages the gate via
/// `inject_death_for_test`, observes non-Ok immediately after `try_restart`, then
/// releases the gate via `await_tick_confirmation_for_test`.
///
/// In production the gate is never engaged, so a real restart confirms its tick
/// after the first natural 10 ms tick with no added latency.
struct RestartGate {
    engaged: Mutex<bool>,
    cvar: Condvar,
}

impl RestartGate {
    fn new() -> Self {
        Self {
            engaged: Mutex::new(false),
            cvar: Condvar::new(),
        }
    }

    /// Engage the gate — the next restarted thread will block until `release`.
    /// Only used by the `inject_death_for_test` seam; gated with it.
    #[cfg(feature = "test-hooks")]
    fn engage(&self) {
        let mut g = self.engaged.lock().unwrap_or_else(|e| e.into_inner());
        *g = true;
    }

    /// Release the gate, waking a blocked thread.
    fn release(&self) {
        let mut g = self.engaged.lock().unwrap_or_else(|e| e.into_inner());
        *g = false;
        self.cvar.notify_all();
    }

    /// Block the calling (tick) thread while the gate is engaged.
    fn wait_until_released(&self) {
        let mut g = self.engaged.lock().unwrap_or_else(|e| e.into_inner());
        while *g {
            g = self.cvar.wait(g).unwrap_or_else(|e| e.into_inner());
        }
    }
}

// ---------------------------------------------------------------------------
// Health-lock helpers — fail-safe poison recovery (#1690-b)
// ---------------------------------------------------------------------------

/// Read the health state, recovering from a poisoned lock fail-safe.
///
/// A poisoned health mutex is a fail-safe condition: report `Degraded` (refuse
/// invocations) rather than panicking. This replaces the prior
/// `.expect("...poisoned")` at every read site (#1690-b).
fn read_health(state: &Mutex<HealthState>) -> HealthState {
    match state.lock() {
        Ok(guard) => *guard,
        Err(_poisoned) => HealthState::Degraded,
    }
}

/// Write the health state, recovering from a poisoned lock fail-safe.
///
/// Recovers the inner guard from a poisoned lock (`into_inner`) so a single prior
/// panic does not wedge the supervisor; the write still lands.
fn write_health(state: &Mutex<HealthState>, value: HealthState) {
    let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
    *guard = value;
}

// ---------------------------------------------------------------------------
// EpochTickThread
// ---------------------------------------------------------------------------

/// Supervised wrapper around the epoch-tick background thread.
///
/// # Lifecycle
///
/// 1. Call `EpochTickThread::start(engine)` before any plugin is invoked.
/// 2. Check `health_state()` before each plugin dispatch; refuse if `Degraded`.
/// 3. `EpochTickThread` implements `Drop`: the stop signal is set, unblocking
///    the tick loop, allowing the OS thread to exit cleanly.
pub struct EpochTickThread {
    /// Shared health state — written by the supervisor, read by the host.
    state: Arc<Mutex<HealthState>>,
    /// Stop signal — set on `Drop` to unblock the tick loop.
    stop: Arc<AtomicBool>,
    /// Engine handle — the supervisor needs it for restart attempts.
    engine: Engine,
    /// Set `true` once a post-restart tick has been CONFIRMED by the live thread
    /// (R-0007-h, #1690-a). Cleared at the start of every restart. Health does NOT
    /// return to `Ok` until this is `true`.
    tick_confirmed: Arc<AtomicBool>,
    /// Gate a restarted thread waits on before its first tick (deterministic
    /// confirm-restart for tests; never engaged in production).
    restart_gate: Arc<RestartGate>,
}

impl EpochTickThread {
    /// Start the epoch-tick thread for `engine` and return the supervisor handle.
    ///
    /// The thread is started immediately. The caller must call this before
    /// allowing any plugin invocations (R-0007-h: starts before first invocation).
    pub fn start(engine: Engine) -> Self {
        let state = Arc::new(Mutex::new(HealthState::Ok));
        let stop = Arc::new(AtomicBool::new(false));
        // The initial thread is "already confirmed": it starts Ok and ticking.
        let tick_confirmed = Arc::new(AtomicBool::new(true));
        let restart_gate = Arc::new(RestartGate::new());

        let handle = Self {
            state: Arc::clone(&state),
            stop: Arc::clone(&stop),
            engine: engine.clone(),
            tick_confirmed: Arc::clone(&tick_confirmed),
            restart_gate: Arc::clone(&restart_gate),
        };

        spawn_tick_thread(
            engine,
            Arc::clone(&state),
            Arc::clone(&stop),
            Arc::clone(&tick_confirmed),
            Arc::clone(&restart_gate),
        );

        handle
    }

    /// Query the current health state of the tick thread.
    ///
    /// Fail-safe on a poisoned lock: returns `Degraded` rather than panicking
    /// (#1690-b).
    pub fn health_state(&self) -> HealthState {
        read_health(&self.state)
    }

    /// Returns `true` iff the tick thread is running and epoch interruption
    /// is active. Plugin dispatch callers MUST check this before invoking.
    pub fn is_healthy(&self) -> bool {
        self.health_state() == HealthState::Ok
    }

    /// Attempt a supervised restart if the thread is dead and the backoff has
    /// elapsed. Returns `true` if a new thread was started.
    ///
    /// This is intentionally NOT called automatically — the caller controls
    /// when restarts are attempted (R-0007-h: one restart/min with backoff).
    ///
    /// # Confirm-restart (#1690-a)
    ///
    /// Health is NOT flipped to `Ok` here. `tick_confirmed` is cleared and a new
    /// thread is spawned; that thread flips health to `Ok` only AFTER it confirms
    /// a post-restart tick. Until then `is_healthy()` / `can_invoke()` stay false.
    pub fn try_restart(&self, last_restart: &mut Option<Instant>) -> bool {
        if self.is_healthy() {
            return false; // Already running; nothing to do.
        }

        let can_restart = last_restart
            .map(|t| t.elapsed() >= RESTART_BACKOFF)
            .unwrap_or(true);

        if !can_restart {
            return false;
        }

        tracing::warn!("epoch_tick_thread: attempting supervised restart (R-0007-h)");

        // Confirm-restart: clear the confirmed flag and leave health Degraded. The
        // new thread will set Ok only after a confirmed post-restart tick (#1690-a).
        self.tick_confirmed.store(false, Ordering::SeqCst);
        write_health(&self.state, HealthState::Degraded);

        let stop = Arc::clone(&self.stop);
        stop.store(false, Ordering::SeqCst);

        spawn_tick_thread(
            self.engine.clone(),
            Arc::clone(&self.state),
            Arc::clone(&self.stop),
            Arc::clone(&self.tick_confirmed),
            Arc::clone(&self.restart_gate),
        );

        *last_restart = Some(Instant::now());
        true
    }

    // -----------------------------------------------------------------------
    // R-0007-h confirm-restart observation (gated behind `test-hooks` feature)
    // -----------------------------------------------------------------------
    //
    // These hooks are NOT gated behind `#[cfg(test)]`: an integration-test crate
    // links the library compiled WITHOUT `--test`, so cfg(test) items would be
    // invisible. They are gated behind `#[cfg(feature = "test-hooks")]` — a
    // non-default cargo feature — so the default build and the production binary
    // cannot reach them. Once Task 23 wires untrusted MCP dispatch into the same
    // process, these in-process-callable poison/kill methods must not coexist
    // with an untrusted path on the default surface (task #1702, Warden condition 1).

    /// Returns `true` iff invocations may proceed: the tick thread is healthy and
    /// a post-restart tick has been confirmed (R-0007-h).
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn can_invoke(&self) -> bool {
        self.is_healthy() && self.tick_confirmed.load(Ordering::SeqCst)
    }

    /// Returns `true` iff a post-restart tick has been confirmed by the live
    /// thread since the last restart (R-0007-h, #1690-a). Immediately after
    /// `try_restart` and before the new thread ticks, this is `false`.
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn tick_confirmed_since_restart(&self) -> bool {
        self.tick_confirmed.load(Ordering::SeqCst)
    }

    /// Test hook: simulate a tick-thread death.
    ///
    /// Stops the current tick thread, transitions health to `Degraded`, clears the
    /// confirmed flag, and engages the restart gate so the NEXT restart's thread
    /// blocks before its first tick (deterministic confirm-restart). Real deaths
    /// are panics inside the tick loop, not reachable from outside the crate.
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn inject_death_for_test(&self) {
        // Stop the running thread so it cannot keep ticking past the injected death.
        self.stop.store(true, Ordering::SeqCst);
        self.tick_confirmed.store(false, Ordering::SeqCst);
        write_health(&self.state, HealthState::Degraded);
        // Engage the gate: the restarted thread will wait before its first tick.
        self.restart_gate.engage();
    }

    /// Test hook: release the restart gate and block until the restarted thread
    /// confirms a post-restart tick.
    ///
    /// Releases the gate engaged by `inject_death_for_test`, then blocks until the
    /// restarted thread has set `tick_confirmed` (and thus flipped health to `Ok`).
    /// Deterministic — uses the confirmed flag, not a fixed sleep.
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn await_tick_confirmation_for_test(&self) {
        self.restart_gate.release();
        // Spin-wait on the confirmed flag. The restarted thread sets it within one
        // tick interval (10 ms) of release; bound the wait generously to avoid a
        // hang if something is wrong, but it confirms near-instantly in practice.
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.tick_confirmed.load(Ordering::SeqCst) {
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Test hook: poison the internal health-state mutex.
    ///
    /// Forces the mutex into a poisoned state by panicking while holding the lock
    /// on a scratch thread. Used to prove `health_state()` degrades fail-safe on a
    /// poisoned lock rather than panicking (#1690-b).
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn poison_health_lock_for_test(&self) {
        let state = Arc::clone(&self.state);
        let _ = std::thread::spawn(move || {
            let _guard = state.lock().unwrap_or_else(|e| e.into_inner());
            // Panic while holding the lock — this poisons the mutex.
            panic!("intentional poison of health-state lock (test hook)");
        })
        .join();
    }
}

impl Drop for EpochTickThread {
    fn drop(&mut self) {
        // Signal the tick loop to exit.
        self.stop.store(true, Ordering::SeqCst);
        // Release the gate so a thread blocked at it can exit cleanly on Drop.
        self.restart_gate.release();
    }
}

// ---------------------------------------------------------------------------
// Tick thread implementation
// ---------------------------------------------------------------------------

/// Spawn the background tick thread.
///
/// The thread first waits at the restart gate (no-op unless engaged), then enters
/// a tight loop: sleep `TICK_INTERVAL`, increment the engine epoch, and on its
/// FIRST post-gate tick set `tick_confirmed` + flip health to `Ok` (confirm-restart,
/// #1690-a). On panic the thread sets health state to `Degraded`, clears the
/// confirmed flag, and emits a tracing error event.
fn spawn_tick_thread(
    engine: Engine,
    state: Arc<Mutex<HealthState>>,
    stop: Arc<AtomicBool>,
    tick_confirmed: Arc<AtomicBool>,
    restart_gate: Arc<RestartGate>,
) {
    // Reference the deadline constant so this module's limit documentation stays
    // wired to the constant.
    let _ = EPOCH_DEADLINE;

    // Keep a handle to the confirmed flag + state for the spawn-failure path; the
    // closure takes its own clones.
    let fail_confirmed = Arc::clone(&tick_confirmed);
    let fail_state = Arc::clone(&state);

    let spawn_result = std::thread::Builder::new()
        .name("mnemra-epoch-tick".to_owned())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // Wait at the gate (production: never engaged → immediate return).
                restart_gate.wait_until_released();
                tick_loop(&engine, &stop, &state, &tick_confirmed);
            }));

            if let Err(panic_payload) = result {
                // The thread panicked — transition to Degraded and emit event.
                let msg = panic_payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panic_payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "(opaque panic payload)".to_owned());

                tracing::error!(
                    event = "epoch_tick_thread_died",
                    reason = %msg,
                    "epoch tick thread panicked — plugin invocations refused until restart (R-0007-h)"
                );

                // Fail-safe write (recovers from a poisoned lock, #1690-b).
                tick_confirmed.store(false, Ordering::SeqCst);
                write_health(&state, HealthState::Degraded);
            }
            // Normal exit (stop flag set via Drop) — no health-state change.
        });

    if let Err(e) = spawn_result {
        // Spawning the supervisor thread failed — fail-safe to Degraded rather
        // than panicking the host (#1690-b spirit: reliability controls fail closed).
        tracing::error!(
            event = "epoch_tick_thread_spawn_failed",
            error = %e,
            "failed to spawn mnemra-epoch-tick thread — health degraded (R-0007-h)"
        );
        fail_confirmed.store(false, Ordering::SeqCst);
        write_health(&fail_state, HealthState::Degraded);
    }
}

/// The tick loop: sleep, increment, confirm on first tick, repeat until stop.
fn tick_loop(
    engine: &Engine,
    stop: &AtomicBool,
    state: &Mutex<HealthState>,
    tick_confirmed: &AtomicBool,
) {
    let mut confirmed = false;
    loop {
        std::thread::sleep(TICK_INTERVAL);
        if stop.load(Ordering::Relaxed) {
            break;
        }
        engine.increment_epoch();

        // Confirm-restart (#1690-a): on the first real post-gate tick, mark the
        // restart confirmed and return health to Ok. Idempotent thereafter.
        if !confirmed {
            tick_confirmed.store(true, Ordering::SeqCst);
            write_health(state, HealthState::Ok);
            confirmed = true;
        }
    }
}
