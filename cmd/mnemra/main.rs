//! mnemra V0.01 host — existence-test spike.
//!
//! Loads `mnemra-echo` (a CM component compiled from this workspace's
//! `plugins/mnemra-echo`), instantiates it once, and calls its exported
//! `run` function twice. Verifies that the host-side counter persists
//! across the two calls — i.e. that plugin state survives between
//! invocations on the same component instance.
//!
//! Run the host from the workspace root: paths are relative to that.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Result, bail};
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::error::Context;
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

// Generated bindings for `world plugin` in `wit/echo.wit`.
// `default: async` forces both the host-implemented imports (the
// `echo::Host` impl below) and the plugin exports (`call_run`) to be
// async. Wasmtime 44 dropped the `async: true` shorthand and dropped
// `Config::async_support()` — when an async bindgen world is used, the
// engine runs in async mode automatically.
wasmtime::component::bindgen!({
    path: "../../wit",
    world: "plugin",
    imports: { default: async },
    exports: { default: async },
});

/// Host state shared with the plugin via `wasmtime` Store.
///
/// `counter` is the number the plugin's `increment-counter` host-fn
/// returns. Wrapped in `Arc<Mutex<_>>` so it can be inspected from
/// outside the Store after each `run` call.
struct HostState {
    counter: Arc<Mutex<u32>>,
    wasi: WasiCtx,
    table: ResourceTable,
}

impl WasiView for HostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

// Implement the `echo` interface for the host. The bindgen!-generated
// trait lives at `mnemra::host::echo::Host` (mirroring the WIT
// `package mnemra:host` + `interface echo`).
impl mnemra::host::echo::Host for HostState {
    async fn echo(&mut self, s: String) -> String {
        format!("echo: {s}")
    }

    async fn increment_counter(&mut self) -> u32 {
        let mut guard = self.counter.lock().expect("counter mutex poisoned");
        *guard += 1;
        *guard
    }
}

/// Resolve the plugin .wasm path relative to the workspace root.
fn plugin_path() -> PathBuf {
    PathBuf::from("target/wasm32-wasip2/release/mnemra_echo.wasm")
}

#[tokio::main]
async fn main() -> Result<()> {
    let path = plugin_path();
    println!("Loading plugin from {}", path.display());

    if !path.exists() {
        bail!(
            "plugin not found at {}; build it first with \
             `cargo build --release -p mnemra-echo --target wasm32-wasip2`",
            path.display()
        );
    }

    // Engine: in Wasmtime 44 async support is automatic when an async
    // bindgen world is used; component-model is on by default.
    // (Earlier versions required `Config::async_support(true)`; that
    // method is now deprecated and has no effect.)
    let config = Config::new();
    let engine = Engine::new(&config).context("constructing wasmtime engine")?;

    // Compile the plugin component.
    let component = Component::from_file(&engine, &path)
        .with_context(|| format!("loading component from {}", path.display()))?;

    // Linker: WASI imports must be wired even though our world only
    // declares `import echo`. A std-using `cdylib` compiled to
    // `wasm32-wasip2` pulls in a panic handler etc. that import
    // `wasi:cli/exit`, `wasi:io/streams`, etc. Skipping this gives
    // "unknown import: wasi:..." at instantiation.
    let mut linker: Linker<HostState> = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)
        .context("adding wasmtime-wasi async imports to linker")?;

    // Wire up the `echo` interface from our WIT world. `HasSelf<_>` is
    // a no-op projection: the host state implements `echo::Host`
    // directly.
    Plugin::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)
        .context("adding mnemra:host/echo to linker")?;

    // Store carries the host state. The shared counter handle stays
    // outside so we can read it post-call without going back through
    // the Store.
    let counter = Arc::new(Mutex::new(0u32));
    let mut store = Store::new(
        &engine,
        HostState {
            counter: Arc::clone(&counter),
            wasi: WasiCtx::builder().inherit_stdio().build(),
            table: ResourceTable::new(),
        },
    );

    // Instantiate once. Both `run` calls below operate on this single
    // instance — that's the test for state persistence.
    let plugin = Plugin::instantiate_async(&mut store, &component, &linker)
        .await
        .context("instantiating plugin component")?;
    println!("Plugin instantiated.");

    // First call: counter should advance from 0 to 1.
    let first = plugin
        .call_run(&mut store, "hello")
        .await
        .context("calling plugin.run(\"hello\")")?;
    println!("run(\"hello\") -> {first:?}");

    // Second call: counter should advance from 1 to 2 — proving state
    // persistence on the same component instance.
    let second = plugin
        .call_run(&mut store, "world")
        .await
        .context("calling plugin.run(\"world\")")?;
    println!("run(\"world\") -> {second:?}");

    // Acceptance assertions. Failure here is the kill-switch firing.
    let counter_final = *counter.lock().expect("counter mutex poisoned");
    if !first.contains("echo: hello") {
        bail!("AC5 fail: first result missing \"echo: hello\": {first:?}");
    }
    if !first.contains("counter: 1") {
        bail!("AC5 fail: first result missing \"counter: 1\": {first:?}");
    }
    if !second.contains("echo: world") {
        bail!("AC6 fail: second result missing \"echo: world\": {second:?}");
    }
    if !second.contains("counter: 2") {
        bail!("AC6 fail: second result missing \"counter: 2\": {second:?}");
    }
    if counter_final != 2 {
        bail!("AC6 fail: counter expected 2, got {counter_final}");
    }

    println!("Spike PASS - round-trip works, counter persisted across invocations.");
    Ok(())
}
