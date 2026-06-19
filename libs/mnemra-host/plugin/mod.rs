//! Plugin runtime module — Task 21 implementation.
//!
//! # Module structure
//!
//! - `runtime` — `PluginRuntime`: manifest-load pipeline (verify → schema_version →
//!   allowlist/caps → output-validation config). The tested surface.
//! - `manifest` — TOML parsing types and deserialization for the plugin manifest.
//! - `allowlist` — Compiled host-fn and verb allowlist extracted from the manifest.
//! - `output` — Output validation: size-cap check, then WIT schema check.
//! - `pool` — Plugin instance pool (3–5 instances per type, init at startup).
//! - `limits` — Wasmtime resource limits: fuel metering + epoch interruption +
//!   memory ceiling.
//! - `epoch_thread` — Supervised epoch-tick thread (10ms tick, crash-detection,
//!   one-restart/min backoff, health-state queryable by the host).
//!
//! # Design split (critical for GREEN phase)
//!
//! `PluginRuntime::load` is PURE MANIFEST PROCESSING — it does not instantiate a
//! Wasmtime component. The RED tests call `load(manifest_bytes, root_material)` with
//! no `.wasm` component and then query the allowlist/caps immediately. Coupling
//! component instantiation into `load` would break all 14 RED tests.
//!
//! The pool (`pool.rs`) and limits (`limits.rs`, `epoch_thread.rs`) are a SEPARATE
//! construct initialized at host startup, outside the `load` path. They satisfy
//! R-0016-a/b and R-0007 but are not reachable from the manifest-load tests.

pub mod allowlist;
pub mod epoch_thread;
pub mod limits;
pub mod manifest;
pub mod output;
pub mod pool;
pub mod runtime;
// Task 22: trap-to-kill-and-replace recovery (R-0007-e/f/h, R-0016-c).
pub mod trap_recovery;
