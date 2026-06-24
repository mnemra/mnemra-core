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

use wasmtime::component::{Component, Instance, Linker};
use wasmtime::{Engine, Store};

use crate::abi::host_fns::FencedArtifactStore;
use crate::plugin::component::{self, HostState};
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
// LiveSlot — a Task-22 live, trappable instance slot
// ---------------------------------------------------------------------------

/// A live, pre-instantiated, trappable plugin instance held by the pool
/// (R-0016-a/b, R-0007-e/f).
///
/// Unlike the Task-21 `PluginSlot` (a manifest handle reserved for Task-23 store
/// wiring), a `LiveSlot` holds an ACTUAL Wasmtime `Store` + `Instance` ready to
/// run a verb. This is what makes kill-and-replace GENUINE: the breaching
/// invocation runs on the live instance taken from a slot, and that store (now
/// trapped/poisoned) is dropped while a fresh instance is instantiated back into
/// the slot synchronously (R-0016-c).
///
/// Task 23 migrates this population path to the component model: the slot holds
/// a `component::Component` instantiated via the host `Linker<HostState>`, and a
/// `Plugin` typed-export world handle for the typed `content` invoke.
struct LiveSlot {
    /// The live store for this slot's instance — its data is the `HostState`
    /// (workspace ctx + fenced map + memory limiter). `None` only transiently
    /// while an invocation has taken the slot out and before the replacement lands.
    store: Store<HostState>,
    /// The live, instantiated raw component `Instance` bound to `store`. The raw
    /// instance (not the typed `Plugin` world handle) is held so the pool can
    /// carry both real content plugins and the trap fixtures (which export a bare
    /// `run`, not `content`) uniformly.
    instance: Instance,
}

/// The live, registered component + identity for a plugin, plus its live slots.
///
/// Holds the compiled `Component` and the host `Linker<HostState>` used to
/// (re)instantiate slots. `LiveModuleEntry` is the trappable population path.
struct LiveModuleEntry {
    plugin_name: String,
    plugin_version: String,
    component: Arc<Component>,
    /// The host Linker for this component (host-fn imports + WASI trap-stubs).
    linker: Arc<Linker<HostState>>,
    /// Live slots. Each is `Some` when populated and `None` only transiently
    /// during a take→repopulate cycle. `slot_count` counts the `Some` entries.
    slots: Vec<Option<LiveSlot>>,
}

/// A live invocation borrowed out of the pool: the slot's store + component world
/// handle moved out for the duration of the call, plus the identity and slot
/// index needed to emit the event and repopulate the slot afterward.
///
/// The `store` + `plugin` are the GENUINE live entry taken from the slot — the
/// slot is left empty (`None`) until `repopulate_slot` instantiates a fresh one.
/// On a trap the store is poisoned; dropping this struct kills it.
pub struct LiveInvocation {
    /// The live store taken from the slot — the caller applies the budget here.
    pub store: Store<HostState>,
    /// The live raw component `Instance` to invoke an export on (the typed
    /// `content` export for the real path; a bare `run` for the trap fixtures).
    pub instance: Instance,
    /// The plugin identity (for the emitted event).
    pub plugin_id: String,
    /// The plugin version (for the emitted event).
    pub plugin_version: String,
    /// The slot index the live entry was taken from (for repopulation).
    pub slot_index: usize,
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
    /// Per-plugin-name slot pools (Task-21 manifest-handle path).
    slots: Mutex<Vec<PluginEntry>>,
    /// Per-plugin-name LIVE component entries (trappable path). Each holds a
    /// compiled `Component` + host `Linker` + POOL_MIN live instances usable by
    /// `invoke_with_recovery`.
    live_modules: Mutex<Vec<LiveModuleEntry>>,
    /// The shared, process-wide fenced artifact store (T7, Branch-2 stub). All
    /// instances share it via their `HostState` so a `create` on one instance is
    /// visible to a later `get` on another. Swapped for real `Storage` in T13.
    artifacts: FencedArtifactStore,
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
            live_modules: Mutex::new(Vec::new()),
            artifacts: FencedArtifactStore::new(),
        })
    }

    /// A handle to the pool's shared fenced artifact store (T7). All pooled
    /// instances share this via their `HostState`.
    pub fn artifacts(&self) -> FencedArtifactStore {
        self.artifacts.clone()
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

    // -----------------------------------------------------------------------
    // Task-22 live-module path (trappable instances + kill-and-replace)
    // -----------------------------------------------------------------------

    /// Register a compiled `component::Component` as a live, trappable plugin and
    /// populate `POOL_MIN` live instances (R-0016-a, R-0007-e/f, R-0016-b).
    ///
    /// Each live slot is a pre-instantiated `Store<HostState>` + `Plugin` world
    /// handle ready to run the typed `content` export. Unlike `register` (the
    /// Task-21 manifest-handle path), these slots are genuinely trappable: a
    /// resource-limit breach during a verb call kills the live instance and a
    /// fresh one is instantiated synchronously (R-0016-c).
    ///
    /// The host `Linker<HostState>` (host-fn imports + WASI trap-stubs) is built
    /// once per registered component and reused for every (re)instantiation.
    pub fn register_module(
        &self,
        plugin_name: &str,
        plugin_version: &str,
        component: &Component,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let component = Arc::new(component.clone());
        // Build the host Linker once for this component (host-fn imports wired,
        // remaining imports trap-stubbed). Reused for every slot (re)instantiation.
        let linker = Arc::new(component::build_linker(&self.engine, &component)?);

        // Pre-instantiate POOL_MIN live slots. A failure to instantiate the
        // fixture is a registration error, surfaced to the caller (fail-closed).
        let mut slots: Vec<Option<LiveSlot>> = Vec::with_capacity(POOL_MIN);
        for _ in 0..POOL_MIN {
            let slot = self.instantiate_live_slot(&component, &linker)?;
            slots.push(Some(slot));
        }

        let mut live = self.live_modules.lock().unwrap_or_else(|e| e.into_inner());
        live.push(LiveModuleEntry {
            plugin_name: plugin_name.to_owned(),
            plugin_version: plugin_version.to_owned(),
            component,
            linker,
            slots,
        });

        Ok(())
    }

    /// The number of currently-populated live slots for `plugin_name`.
    ///
    /// Used to assert pool-size preservation across a kill-and-replace (R-0016-c):
    /// because the replacement is synchronous, the count is equal before and after
    /// a breaching invocation that returns an error.
    pub fn slot_count(&self, plugin_name: &str) -> usize {
        let live = self.live_modules.lock().unwrap_or_else(|e| e.into_inner());
        live.iter()
            .find(|e| e.plugin_name == plugin_name)
            .map(|e| e.slots.iter().filter(|s| s.is_some()).count())
            .unwrap_or(0)
    }

    /// Instantiate a fresh live slot for `component` on this pool's engine using
    /// `linker` (R-0016-b component instantiation via the Linker).
    ///
    /// Builds a `Store<HostState>` whose data carries the shared fenced artifact
    /// store + the memory limiter (unbound to a workspace until the dispatch site
    /// sets it, R-0006-b), then instantiates the component world. Budget
    /// (fuel/epoch) is applied per-invocation, not here.
    fn instantiate_live_slot(
        &self,
        component: &Component,
        linker: &Linker<HostState>,
    ) -> Result<LiveSlot, Box<dyn std::error::Error>> {
        let mut store = component::new_store(&self.engine, self.artifacts.clone());
        let instance = component::instantiate(&mut store, component, linker)?;
        Ok(LiveSlot { store, instance })
    }

    /// Take the live instance out of the first populated slot for `plugin_name`
    /// for an invocation (R-0016-c kill-and-replace step 1).
    ///
    /// The slot is left EMPTY (`None`) — the caller runs the verb on the returned
    /// `LiveInvocation`, then calls `repopulate_slot` to instantiate a fresh
    /// instance back into the slot before returning to its own caller. Returns
    /// `None` if the plugin is not registered or has no populated slots.
    pub fn take_live_invocation(&self, plugin_name: &str) -> Option<LiveInvocation> {
        let mut live = self.live_modules.lock().unwrap_or_else(|e| e.into_inner());
        let entry = live.iter_mut().find(|e| e.plugin_name == plugin_name)?;

        // Find the first populated slot index.
        let slot_index = entry.slots.iter().position(|s| s.is_some())?;
        // `take` the live entry out — the slot is now genuinely empty (killed
        // pending replacement). `.get_mut` is used (not indexing) but the index
        // came from `position` so it is in-bounds; still, be defensive.
        let live_slot = entry.slots.get_mut(slot_index).and_then(Option::take)?;

        Some(LiveInvocation {
            store: live_slot.store,
            instance: live_slot.instance,
            plugin_id: entry.plugin_name.clone(),
            plugin_version: entry.plugin_version.clone(),
            slot_index,
        })
    }

    /// Instantiate a fresh live instance back into `slot_index` for `plugin_name`
    /// (R-0016-c kill-and-replace step 2 — synchronous replacement).
    ///
    /// Called after a verb invocation (whether it trapped or returned) to restore
    /// the slot to a live, ready state. The old (possibly trapped) store was moved
    /// into the `LiveInvocation` and is dropped by the caller — this method creates
    /// its replacement so the pool size is preserved.
    ///
    /// Fail-closed: if re-instantiation fails (it should not for a component that
    /// already instantiated at registration), the slot is left empty and the
    /// failure is logged; the pool does not panic.
    pub fn repopulate_slot(&self, plugin_name: &str, slot_index: usize) {
        // Resolve the component + linker under the lock, then instantiate, then
        // store back. Cloning the Arcs releases the lock before instantiation.
        let (component, linker) = {
            let live = self.live_modules.lock().unwrap_or_else(|e| e.into_inner());
            match live.iter().find(|e| e.plugin_name == plugin_name) {
                Some(entry) => (Arc::clone(&entry.component), Arc::clone(&entry.linker)),
                None => return,
            }
        };

        let fresh = match self.instantiate_live_slot(&component, &linker) {
            Ok(slot) => slot,
            Err(e) => {
                tracing::error!(
                    event = "plugin_slot_replacement_failed",
                    plugin_id = %plugin_name,
                    error = %e,
                    "failed to re-instantiate plugin slot after kill — slot left empty (fail-closed)"
                );
                return;
            }
        };

        let mut live = self.live_modules.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = live.iter_mut().find(|e| e.plugin_name == plugin_name)
            && let Some(slot) = entry.slots.get_mut(slot_index)
        {
            *slot = Some(fresh);
        }
    }
}
