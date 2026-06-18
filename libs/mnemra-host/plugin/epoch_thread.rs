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
//! # R-0007-h: crash semantics
//!
//! "Not silently restarted" means the crash is always logged and health-state
//! changes before any restart is attempted. The restart policy (1/min, backoff)
//! is defence-in-depth; the primary signal is the degraded health state.

use std::sync::{
    Arc, Mutex,
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
}

impl EpochTickThread {
    /// Start the epoch-tick thread for `engine` and return the supervisor handle.
    ///
    /// The thread is started immediately. The caller must call this before
    /// allowing any plugin invocations (R-0007-h: starts before first invocation).
    pub fn start(engine: Engine) -> Self {
        let state = Arc::new(Mutex::new(HealthState::Ok));
        let stop = Arc::new(AtomicBool::new(false));

        let handle = Self {
            state: Arc::clone(&state),
            stop: Arc::clone(&stop),
            engine: engine.clone(),
        };

        spawn_tick_thread(engine, Arc::clone(&state), Arc::clone(&stop));

        handle
    }

    /// Query the current health state of the tick thread.
    pub fn health_state(&self) -> HealthState {
        *self.state.lock().expect("epoch health state lock poisoned")
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

        // Transition health state back to Ok optimistically; the thread will
        // set it to Degraded again if it immediately panics.
        {
            let mut guard = self.state.lock().expect("health state lock poisoned");
            *guard = HealthState::Ok;
        }

        let stop = Arc::clone(&self.stop);
        stop.store(false, Ordering::SeqCst);

        spawn_tick_thread(
            self.engine.clone(),
            Arc::clone(&self.state),
            Arc::clone(&self.stop),
        );

        *last_restart = Some(Instant::now());
        true
    }
}

impl Drop for EpochTickThread {
    fn drop(&mut self) {
        // Signal the tick loop to exit.
        self.stop.store(true, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------------------
// Tick thread implementation
// ---------------------------------------------------------------------------

/// Spawn the background tick thread.
///
/// The thread runs a tight loop: sleep `TICK_INTERVAL`, increment the engine
/// epoch, check the stop flag. On panic the thread sets health state to
/// `Degraded` and emits a tracing error event.
fn spawn_tick_thread(engine: Engine, state: Arc<Mutex<HealthState>>, stop: Arc<AtomicBool>) {
    // Capture `deadline` for the doc comment; actual usage is via EPOCH_DEADLINE.
    let _ = EPOCH_DEADLINE; // ensure the constant is referenced in this module.

    std::thread::Builder::new()
        .name("mnemra-epoch-tick".to_owned())
        .spawn(move || {
            // The panic hook is set before entering the loop. On panic, the
            // standard `take_hook` fires, then Rust's default handler unwinds
            // and the thread exits — we detect this via the JoinHandle::join in
            // a monitoring wrapper below. However, for simplicity at V0 we use
            // a catch_unwind to detect panics inside the loop and transition
            // health state without a monitoring wrapper.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                tick_loop(&engine, &stop);
            }));

            if let Err(panic_payload) = result {
                // The thread panicked — transition to Degraded and emit event.
                let msg = panic_payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| {
                        panic_payload
                            .downcast_ref::<String>()
                            .cloned()
                    })
                    .unwrap_or_else(|| "(opaque panic payload)".to_owned());

                tracing::error!(
                    event = "epoch_tick_thread_died",
                    reason = %msg,
                    "epoch tick thread panicked — plugin invocations refused until restart (R-0007-h)"
                );

                let mut guard = state.lock().expect("health state lock on panic path");
                *guard = HealthState::Degraded;
            }
            // Normal exit (stop flag set via Drop) — no health-state change.
        })
        .expect("failed to spawn mnemra-epoch-tick thread");
}

/// The tick loop: sleep, increment, repeat until stop.
fn tick_loop(engine: &Engine, stop: &AtomicBool) {
    loop {
        std::thread::sleep(TICK_INTERVAL);
        if stop.load(Ordering::Relaxed) {
            break;
        }
        engine.increment_epoch();
    }
}
