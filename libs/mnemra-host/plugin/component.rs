//! Component-model host bindings — the `Linker` + host-fn import registration
//! that makes the guest's typed `content` export invocable (T3, R-0012-a/f,
//! R-0019, R-0006-a/b/e).
//!
//! # What this module owns
//!
//! 1. The `component::bindgen!` invocation over the `plugin` WIT world — this
//!    generates the host side of the ABI: typed accessors for the guest's
//!    `content` export (`call_create` / `call_get` / …) and the `Host` traits
//!    for the imports the guest calls back into (`artifact`).
//! 2. `HostState` — the store data threaded through every component instance.
//!    It carries the host-derived `WorkspaceCtx` (the authoritative scoping key,
//!    R-0006-b) and a handle to the fenced in-memory artifact map (T7), and it
//!    implements `ResourceLimiter` so the 64 MiB memory ceiling (R-0007-c)
//!    travels with the store exactly as `PluginResourceLimiter` did before the
//!    component migration.
//! 3. The `Host` trait impls for the wired imports. `artifact-create` /
//!    `artifact-get` delegate to the fenced-map bodies in `abi::host_fns`,
//!    deriving `workspace_id` from `self` (store data) and IGNORING any
//!    guest-supplied `workspace-ctx` value (R-0006-e: no plugin-supplied
//!    scoping key — the guest cannot forge a workspace).
//!
//! # WorkspaceCtx threading (R-0006-b/e)
//!
//! The typed `content` export carries no ctx. The host constructs the single
//! authoritative `WorkspaceCtx` at the dispatch site (R-0006-b), stores its
//! `workspace_id` in `HostState`, and invokes the export. When the guest calls
//! back into `artifact-create`/`artifact-get`, the import body reads
//! `workspace_id` from `HostState` — never from the `workspace-ctx` argument the
//! guest passes (that argument is a host-ignored placeholder, R-0006-e).
//!
//! # Unwired imports (reachability)
//!
//! At slice 1 the echo `content.create`/`content.get` path calls ONLY
//! `artifact-create` / `artifact-get`. The remaining world imports (`echo`,
//! and the WASI imports the `wasm32-wasip2` component carries) are NOT reached
//! on this path; they are satisfied by `Linker::define_unknown_imports_as_traps`
//! — a missing-import call would trap, never silently succeed. This is honest:
//! the slice-1 path that runs is fully real; only genuinely-unreachable imports
//! are trap-stubbed (no `todo!()` on the reachable path).

use uuid::Uuid;
use wasmtime::component::{Component, Instance, Linker};
use wasmtime::{ResourceLimiter, Store};

use crate::abi::host_fns::FencedArtifactStore;
use crate::plugin::limits::MEMORY_MAX_BYTES;

// ---------------------------------------------------------------------------
// Generated host bindings for the `plugin` world
// ---------------------------------------------------------------------------

// `component::bindgen!` generates, against the fixed `plugin` world in
// `wit/echo.wit`:
//   - `Plugin` — the instantiated-world handle with typed export accessors
//     (`plugin.mnemra_host_content().call_create(store, ..)` etc.).
//   - `Plugin::add_to_linker` and per-interface `add_to_linker` helpers.
//   - `mnemra::host::artifact::Host` / `mnemra::host::echo::Host` import traits.
//   - `mnemra::host::types::{WorkspaceCtx, ..}` shared types.
//
// Imports are synchronous and non-trappable: the slice-1 artifact bodies always
// succeed (fenced-map insert/lookup), so the generated `Host` methods return
// plain values rather than `wasmtime::Result`. The export side is synchronous —
// the MCP handler runs the invoke inside a blocking section.
wasmtime::component::bindgen!({
    path: "../../wit",
    world: "plugin",
});

// Re-export the generated guest-import types under stable local names.
pub use mnemra::host::types::WorkspaceCtx as WitWorkspaceCtx;

// ---------------------------------------------------------------------------
// HostState — the per-store host data
// ---------------------------------------------------------------------------

/// The store data threaded through a component instance for one invocation
/// (R-0006-b store-threaded ctx, R-0007-c memory ceiling).
///
/// `workspace_id` is the authoritative, host-derived scoping key set at the
/// dispatch site; the host-fn import bodies read it from here, never from a
/// guest-supplied argument (R-0006-e). `artifacts` is the shared T7 fenced map
/// (process-wide so a `create` on one pooled instance is visible to a later
/// `get` on another). `HostState` implements `ResourceLimiter` so the 64 MiB
/// ceiling rides the store unchanged from the pre-component `PluginResourceLimiter`.
pub struct HostState {
    /// Host-derived workspace scoping key (R-0006-b). Defaults to nil and is set
    /// per-invocation at the dispatch site before the export is called.
    workspace_id: Uuid,
    /// The shared fenced in-memory artifact store (T7, Branch-2 stub).
    artifacts: FencedArtifactStore,
}

impl HostState {
    /// Construct host state for an invocation under `workspace_id`, sharing the
    /// given fenced artifact store.
    pub fn new(workspace_id: Uuid, artifacts: FencedArtifactStore) -> Self {
        Self {
            workspace_id,
            artifacts,
        }
    }

    /// Construct host state with no bound workspace (nil UUID). Used at pool
    /// pre-instantiation time, before a request assigns the real workspace at
    /// the dispatch site (R-0006-b).
    pub fn unbound(artifacts: FencedArtifactStore) -> Self {
        Self {
            workspace_id: Uuid::nil(),
            artifacts,
        }
    }

    /// Bind the host-derived workspace scoping key for the upcoming invocation
    /// (R-0006-b: set at the single dispatch site, before the export runs).
    pub fn set_workspace_id(&mut self, workspace_id: Uuid) {
        self.workspace_id = workspace_id;
    }

    /// The host-derived workspace scoping key (R-0006-c accessor discipline).
    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }

    /// A handle to the shared fenced artifact store.
    pub fn artifacts(&self) -> &FencedArtifactStore {
        &self.artifacts
    }
}

impl ResourceLimiter for HostState {
    /// Approve linear-memory growth up to the 64 MiB ceiling (R-0007-c),
    /// identical to the pre-component `PluginResourceLimiter`.
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= MEMORY_MAX_BYTES)
    }

    /// Table growth is unrestricted at V0.
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
// Host-fn import implementations
// ---------------------------------------------------------------------------

impl mnemra::host::types::Host for HostState {}

// TENANT-ISOLATION LINCHPIN (R-0006-e). Every body below derives the workspace
// scoping key from `self.workspace_id` (the host-derived value bound onto this
// `HostState` at the single dispatch site — see `trap_recovery::
// invoke_through_recovery`). The `_ctx: WitWorkspaceCtx` parameter is the value
// the GUEST passes across the import boundary; it is DELIBERATELY IGNORED here
// (prefixed `_`). No host-fn body may read, trust, or branch on `_ctx` — doing
// so would let a plugin choose its own workspace and breach tenant isolation.
// The scoping key is host-derived, never plugin-supplied.
impl mnemra::host::artifact::Host for HostState {
    /// `artifact-create` — persist via the fenced map and return a real ULID.
    ///
    /// The `_ctx` argument is the guest-supplied placeholder and is IGNORED
    /// (R-0006-e); the real scoping key is `self.workspace_id` (R-0006-b/d).
    fn artifact_create(
        &mut self,
        _ctx: WitWorkspaceCtx,
        type_name: String,
        frontmatter: String,
        body: Option<String>,
    ) -> String {
        crate::abi::host_fns::fenced_artifact_create(
            &self.artifacts,
            self.workspace_id,
            &type_name,
            frontmatter,
            body,
        )
    }

    /// `artifact-get` — read back from the fenced map, workspace-scoped on
    /// `self.workspace_id` (R-0006-d). The guest-supplied `_ctx` is ignored.
    fn artifact_get(&mut self, _ctx: WitWorkspaceCtx, id: String) -> Option<String> {
        crate::abi::host_fns::fenced_artifact_get(&self.artifacts, self.workspace_id, &id)
    }

    /// `artifact-update` — merge the frontmatter patch + (optionally) replace the
    /// body in the fenced map, workspace-scoped on `self.workspace_id` (R-0006-d).
    /// The guest-supplied `_ctx` is ignored (R-0006-e); a missing/cross-workspace
    /// target is a silent no-op (the fenced `(workspace_id, id)` lookup misses).
    fn artifact_update(
        &mut self,
        _ctx: WitWorkspaceCtx,
        id: String,
        frontmatter_patch: String,
        body: Option<String>,
    ) {
        crate::abi::host_fns::fenced_artifact_update(
            &self.artifacts,
            self.workspace_id,
            &id,
            frontmatter_patch,
            body,
        )
    }

    /// `artifact-list` — list ids of `type_name` artifacts visible in this
    /// workspace, scoped on `self.workspace_id` (R-0006-d). The guest-supplied
    /// `_ctx` is ignored (R-0006-e); `filters` is threaded but not applied this
    /// slice (predicate logic deferred — brain #1846).
    fn artifact_list(
        &mut self,
        _ctx: WitWorkspaceCtx,
        type_name: String,
        filters: String,
    ) -> Vec<String> {
        crate::abi::host_fns::fenced_artifact_list(
            &self.artifacts,
            self.workspace_id,
            &type_name,
            filters,
        )
    }

    /// `artifact-delete` — remove from the fenced map, workspace-scoped on
    /// `self.workspace_id` (R-0006-d). The guest-supplied `_ctx` is ignored
    /// (R-0006-e); a missing/cross-workspace target is a silent no-op (the fenced
    /// `(workspace_id, id)` lookup misses).
    fn artifact_delete(&mut self, _ctx: WitWorkspaceCtx, id: String) {
        crate::abi::host_fns::fenced_artifact_delete(&self.artifacts, self.workspace_id, &id)
    }
}

impl mnemra::host::echo::Host for HostState {
    /// `echo` — retained import body; NOT on the slice-1 `content` path.
    fn echo(&mut self, s: String) -> String {
        format!("echo: {s}")
    }

    /// `increment-counter` — retained import body; NOT on the slice-1 path.
    /// Per-instance state is not modelled here (the slice-1 path never calls it).
    fn increment_counter(&mut self) -> u32 {
        0
    }
}

// ---------------------------------------------------------------------------
// Linker construction
// ---------------------------------------------------------------------------

/// Build a component `Linker<HostState>` that registers the host-fn import
/// bodies and trap-stubs every other import of `component` (R-0012-a, R-0006-a).
///
/// The wired imports (`artifact`, `echo`) come from the bindgen-generated
/// `add_to_linker`; the remaining imports (WASI) are filled with trapping stubs
/// via `define_unknown_imports_as_traps` — a call to an unreached import traps
/// rather than silently succeeding. The slice-1 `content.create`/`content.get`
/// path calls only `artifact-create`/`artifact-get`, both genuinely wired.
pub fn build_linker(
    engine: &wasmtime::Engine,
    component: &Component,
) -> Result<Linker<HostState>, Box<dyn std::error::Error>> {
    let mut linker: Linker<HostState> = Linker::new(engine);

    // Allow the real host-fn bodies to SHADOW the trap-stubs. Order matters:
    //   1. trap-stub EVERY import (including WASI, `types`, `artifact`, `echo`);
    //   2. add the real host-fn bodies, which shadow the stubs for the mnemra
    //      interfaces, leaving the unreached WASI imports as traps.
    //
    // This ordering is required because `define_unknown_imports_as_traps`
    // unconditionally re-creates *instance* imports (it does not skip already-
    // defined instances), so running it AFTER `add_to_linker` errors with
    // "map entry `mnemra:host/types` defined twice". Stubbing first + shadowing
    // the mnemra interfaces with real bodies second gives: real `artifact` bodies
    // on the reachable path, WASI as traps on the unreachable path.
    linker.allow_shadowing(true);

    // 1. Trap-stub all imports (WASI + the mnemra instances, transiently).
    linker.define_unknown_imports_as_traps(component)?;

    // 2. Register the real host-fn import bodies generated by bindgen, shadowing
    // the stubs for `artifact` / `echo` / `types`. `HasSelf<_>` says the store
    // data IS the host type (no projection).
    Plugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| state)?;

    Ok(linker)
}

/// Instantiate `component` on `linker` with `store`, returning the raw component
/// `Instance` (R-0016-b). The pool holds the raw `Instance` (not the typed
/// `Plugin` world handle) so it can hold BOTH real content plugins (export
/// `content`) and the trap fixtures (export a bare `run`) uniformly — `Plugin`
/// instantiation requires the `content` export, which the trap fixtures lack.
/// The typed `content` invoke is obtained on demand from the raw instance via
/// `content_create` / `content_get`; the trap path uses `call_run`.
pub fn instantiate(
    store: &mut Store<HostState>,
    component: &Component,
    linker: &Linker<HostState>,
) -> Result<Instance, Box<dyn std::error::Error>> {
    let instance = linker.instantiate(&mut *store, component)?;
    Ok(instance)
}

/// Build a `Store<HostState>` carrying the fenced store and the memory limiter,
/// unbound to a workspace until the dispatch site sets it (R-0006-b).
pub fn new_store(engine: &wasmtime::Engine, artifacts: FencedArtifactStore) -> Store<HostState> {
    let mut store = Store::new(engine, HostState::unbound(artifacts));
    store.limiter(|state| state as &mut dyn ResourceLimiter);
    store
}

/// Invoke the typed `content.create` export on a raw component `Instance`
/// (R-0019-a). Wraps the instance in the bindgen `Plugin` handle and calls the
/// generated typed accessor, so marshalling is the bindgen-generated canonical
/// ABI — no hand-rolled lowering. Returns the host-generated id, or any trap.
pub fn content_create(
    store: &mut Store<HostState>,
    instance: &Instance,
    type_name: &str,
    frontmatter: &str,
    body: Option<&str>,
) -> wasmtime::Result<String> {
    let plugin = Plugin::new(&mut *store, instance)?;
    let content = plugin.mnemra_host_content();
    // The bindgen-generated typed accessor takes `&String` for the required
    // string params (not `&str`); `to_owned()` is required to materialize the
    // owned value the borrow points to. clippy's `unnecessary_to_owned` is a
    // false positive here — `&str` does not satisfy the `&String` parameter.
    #[allow(clippy::unnecessary_to_owned)]
    content.call_create(
        &mut *store,
        &type_name.to_owned(),
        &frontmatter.to_owned(),
        body,
    )
}

/// Invoke the typed `content.get` export on a raw component `Instance`
/// (R-0019-a). Returns the stored content (round-tripping the payload) or `None`.
pub fn content_get(
    store: &mut Store<HostState>,
    instance: &Instance,
    id: &str,
) -> wasmtime::Result<Option<String>> {
    let plugin = Plugin::new(&mut *store, instance)?;
    let content = plugin.mnemra_host_content();
    // See `content_create`: the accessor takes `&String`, not `&str`.
    #[allow(clippy::unnecessary_to_owned)]
    content.call_get(&mut *store, &id.to_owned())
}

/// Invoke the typed `content.list` export on a raw component `Instance`
/// (R-0019-a). Returns the ids of `type_name` artifacts visible in the caller's
/// workspace (the guest body calls back into the host `artifact-list` import,
/// which scopes on the host-derived `workspace_id`). `filters` is threaded but
/// not applied this slice (predicate logic deferred — brain #1846).
pub fn content_list(
    store: &mut Store<HostState>,
    instance: &Instance,
    type_name: &str,
    filters: &str,
) -> wasmtime::Result<Vec<String>> {
    let plugin = Plugin::new(&mut *store, instance)?;
    let content = plugin.mnemra_host_content();
    // See `content_create`: the accessor takes `&String`, not `&str`.
    #[allow(clippy::unnecessary_to_owned)]
    content.call_list(&mut *store, &type_name.to_owned(), &filters.to_owned())
}

/// Invoke the typed `content.update` export on a raw component `Instance`
/// (R-0019-a). The guest body calls back into the host `artifact-update` import,
/// which merges the frontmatter patch and (when `body` is `Some`) replaces the
/// body in the fenced map, scoped on the host-derived `workspace_id` (R-0006-d).
/// `update` is void; this returns `()` on success (or any trap).
pub fn content_update(
    store: &mut Store<HostState>,
    instance: &Instance,
    id: &str,
    frontmatter_patch: &str,
    body: Option<&str>,
) -> wasmtime::Result<()> {
    let plugin = Plugin::new(&mut *store, instance)?;
    let content = plugin.mnemra_host_content();
    // See `content_create`: the accessor takes `&String`, not `&str`.
    #[allow(clippy::unnecessary_to_owned)]
    content.call_update(
        &mut *store,
        &id.to_owned(),
        &frontmatter_patch.to_owned(),
        body,
    )
}

/// Invoke the typed `content.delete` export on a raw component `Instance`
/// (R-0019-a). The guest body calls back into the host `artifact-delete` import,
/// which removes the artifact keyed `(workspace_id, id)` from the fenced map;
/// a missing/cross-workspace target is a silent no-op (R-0006-d).
/// `delete` is void; this returns `()` on success (or any trap).
pub fn content_delete(
    store: &mut Store<HostState>,
    instance: &Instance,
    id: &str,
) -> wasmtime::Result<()> {
    let plugin = Plugin::new(&mut *store, instance)?;
    let content = plugin.mnemra_host_content();
    // See `content_create`: the accessor takes `&String`, not `&str`.
    #[allow(clippy::unnecessary_to_owned)]
    content.call_delete(&mut *store, &id.to_owned())
}
