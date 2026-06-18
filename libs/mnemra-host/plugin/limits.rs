//! Wasmtime resource limits: fuel metering, epoch interruption, memory ceiling.
//!
//! # Task 21 limits (R-0007-a/b/c/g)
//!
//! - **Fuel:** 10 billion ticks per invocation via `Store::set_fuel` /
//!   `Config::consume_fuel(true)`. Exhausted fuel → `Trap::OutOfFuel`.
//! - **Epoch interruption:** `Config::epoch_interruption(true)` +
//!   `store.set_epoch_deadline(500)`. Epoch counter is incremented by the
//!   supervised tick thread every 10ms → 5 s wall-clock deadline.
//! - **Memory ceiling:** 64 MiB via `ResourceLimiter` (R-0007-c). In
//!   wasmtime 45.x, per-instance memory caps on the on-demand allocator are
//!   enforced at the `Store` level via a `ResourceLimiter` implementation.
//!   The `PluginResourceLimiter` struct below implements this.
//!
//! # Task boundary
//!
//! This task activates the limits and wires the supervisor. Catching the
//! fuel/epoch trap and doing kill-and-replace is Task 22 scope.

use wasmtime::{Config, Engine, ResourceLimiter};

// ---------------------------------------------------------------------------
// Constants (R-0007)
// ---------------------------------------------------------------------------

/// Fuel ticks per invocation — 10 billion (R-0007-a).
pub const FUEL_LIMIT: u64 = 10_000_000_000;

/// Epoch deadline (in epoch ticks). Each tick is 10ms → 500 ticks = 5 s
/// (R-0007-b).
pub const EPOCH_DEADLINE: u64 = 500;

/// Memory ceiling per instance — 64 MiB (R-0007-c).
pub const MEMORY_MAX_BYTES: usize = 64 * 1024 * 1024;

// ---------------------------------------------------------------------------
// ResourceLimiter implementation
// ---------------------------------------------------------------------------

/// Per-store resource limiter — enforces the 64 MiB memory ceiling (R-0007-c).
///
/// Attached to each Wasmtime `Store` via `Store::limiter`. Wasmtime calls the
/// `memory_growing` method before any linear-memory growth; returning `false`
/// traps the guest and prevents growth beyond the ceiling.
///
/// This is the correct approach for the on-demand allocator in wasmtime 45.x.
/// The pooling allocator has `PoolingAllocationConfig::max_memory_size`, but
/// mnemra uses the on-demand allocator at V0.
pub struct PluginResourceLimiter;

impl ResourceLimiter for PluginResourceLimiter {
    /// Approve or deny a linear-memory growth request.
    ///
    /// Returns `Ok(true)` if `new_size` ≤ `MEMORY_MAX_BYTES`; `Ok(false)` to
    /// deny (traps the guest) if the requested size exceeds the ceiling.
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= MEMORY_MAX_BYTES)
    }

    /// Table growth is unrestricted at V0 (tables are small in practice).
    fn table_growing(
        &mut self,
        _current: usize,
        _desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// Engine constructor
// ---------------------------------------------------------------------------

/// Build a `wasmtime::Engine` with fuel and epoch interruption active (R-0007).
///
/// Memory ceiling is enforced at the `Store` level via `PluginResourceLimiter`
/// (attached per-store in `pool.rs` when stores are initialised in Task 23).
///
/// # Config choices
///
/// - `consume_fuel(true)` — enables `Store::set_fuel` / `Store::get_fuel`.
/// - `epoch_interruption(true)` — enables epoch-based deadline; the
///   `EpochTickThread` increments the counter on a 10ms interval.
///
/// # R-0007-i: Wasmtime version pinned
///
/// `wasmtime` is pinned to an exact version in `Cargo.toml` (no open caret).
/// Major/minor upgrades require an explicit plan AC (R-0007-i).
pub fn build_engine() -> Result<Engine, wasmtime::Error> {
    let mut config = Config::new();
    config.consume_fuel(true);
    config.epoch_interruption(true);
    Engine::new(&config)
}
