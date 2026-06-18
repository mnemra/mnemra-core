//! Plugin instance pool — 3–5 instances per plugin type (R-0016-a/b, R-0007-d).
//!
//! # Pool design
//!
//! The pool holds pre-initialised, tenant-stateless Wasmtime component instances.
//! At host startup, before the MCP server accepts requests (Task 23), the pool
//! is populated with 3–5 instances per registered plugin type. Each invocation
//! borrows a slot, executes, and returns it to the pool — no cross-call state
//! is retained in the instance (R-0007-d: instances are reset or replaced, not
//! re-used across tenant boundaries).
//!
//! # Task 21 scope
//!
//! At V0 the pool mechanism is compiled and the struct is constructible. A live
//! component binary (`.wasm`) is required to populate slots; the `mnemra-echo`
//! plugin is built by the workspace's `plugins/mnemra-echo` crate but is not
//! embedded at compile time (R-0005-d applies to manifests, not component bytes
//! at this stage). Population from actual `.wasm` bytes is deferred to Task 23
//! (MCP server startup wiring).
//!
//! The RED tests do not exercise the pool — they only exercise the `PluginRuntime`
//! manifest-load surface. Pool correctness is validated by integration/smoke tests
//! added when Task 23 wires the pool into the startup sequence.
//!
//! # Core plugin non-uninstallability (R-0002-a/d)
//!
//! The four `core: true` plugins are signed by the mnemra root. They are
//! structurally non-uninstallable at runtime: the pool holds the only live
//! references and there is no API to remove a registered plugin entry from a
//! running pool. The only removal path is a binary rebuild (R-0002-d).
//!
//! At V0 this is structural: `PluginPool` has no `remove` or `unregister` method.
//! A future task that adds dynamic plugin support must go through an ADR gate.

use std::sync::{Arc, Mutex};

use wasmtime::Engine;

use crate::plugin::epoch_thread::{EpochTickThread, HealthState};
use crate::plugin::limits::{EPOCH_DEADLINE, FUEL_LIMIT, build_engine};
use crate::plugin::runtime::PluginRuntime;

// ---------------------------------------------------------------------------
// Pool constants
// ---------------------------------------------------------------------------

/// Minimum number of pre-initialised instances per plugin type (R-0016-a).
pub const POOL_MIN: usize = 3;

/// Maximum number of pre-initialised instances per plugin type (R-0016-a).
pub const POOL_MAX: usize = 5;

// ---------------------------------------------------------------------------
// PluginSlot — one pre-initialised instance slot
// ---------------------------------------------------------------------------

/// A pre-initialised, tenant-stateless plugin instance slot.
///
/// At V0, holds the plugin's `PluginRuntime` (manifest metadata + allowlists)
/// and a placeholder for the Wasmtime component store (populated in Task 23).
/// `tenant_state` is always reset between invocations (R-0007-d).
pub struct PluginSlot {
    /// Manifest metadata for this slot — allowlists, schema_version, etc.
    pub runtime: Arc<PluginRuntime>,
    // `component_store` is task-23 scope (wasmtime::Store<HostState>).
    // At V0 the slot is a manifest handle + reserve for the store.
}

// ---------------------------------------------------------------------------
// PluginPool
// ---------------------------------------------------------------------------

/// The host-level plugin instance pool.
///
/// Holds a pool of slots per registered plugin. Populated at host startup
/// before the MCP server accepts requests (Task 23 wiring). Thread-safe via
/// internal `Mutex`.
pub struct PluginPool {
    /// The shared Wasmtime engine — configured with fuel, epoch, memory limits.
    engine: Engine,
    /// The supervised epoch-tick thread. Must be healthy before any invocation.
    epoch_thread: EpochTickThread,
    /// Per-plugin-name slot pools.
    slots: Mutex<Vec<PluginEntry>>,
}

// Fields are written during `register` and will be read when Task 23 wires
// pool dispatch (component invocation by plugin name + slot borrowing).
#[allow(dead_code)]
struct PluginEntry {
    plugin_name: String,
    slots: Vec<PluginSlot>,
}

impl PluginPool {
    /// Initialise the pool: build the engine, start the epoch-tick thread, and
    /// prepare empty slot vectors.
    ///
    /// Call this at host startup, before the MCP server accepts any requests
    /// (R-0016-a: pool is ready before first invocation).
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let engine = build_engine()?;
        let epoch_thread = EpochTickThread::start(engine.clone());

        Ok(Self {
            engine,
            epoch_thread,
            slots: Mutex::new(Vec::new()),
        })
    }

    /// Register a plugin by its manifest runtime and pre-initialise POOL_MIN slots.
    ///
    /// In Task 23 this will also instantiate POOL_MIN Wasmtime component stores.
    /// At V0 each slot holds the manifest runtime as a lightweight placeholder.
    pub fn register(
        &self,
        plugin_name: &str,
        runtime: Arc<PluginRuntime>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut slots = self.slots.lock().expect("pool slot lock poisoned");

        // Build POOL_MIN slots.
        let slot_vec: Vec<PluginSlot> = (0..POOL_MIN)
            .map(|_| PluginSlot {
                runtime: Arc::clone(&runtime),
            })
            .collect();

        slots.push(PluginEntry {
            plugin_name: plugin_name.to_owned(),
            slots: slot_vec,
        });

        Ok(())
    }

    /// Check the epoch-tick thread health state.
    ///
    /// Callers MUST check this before dispatching to a plugin (R-0007-h).
    pub fn epoch_health(&self) -> HealthState {
        self.epoch_thread.health_state()
    }

    /// Returns `true` iff the epoch-tick thread is healthy and plugin dispatch
    /// is safe.
    pub fn can_invoke(&self) -> bool {
        self.epoch_thread.is_healthy()
    }

    /// Returns a reference to the Wasmtime engine (for Task 23 component loading).
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Returns the fuel limit constant for configuring Stores.
    pub fn fuel_limit() -> u64 {
        FUEL_LIMIT
    }

    /// Returns the epoch deadline constant for configuring Stores.
    pub fn epoch_deadline() -> u64 {
        EPOCH_DEADLINE
    }
}
