//! Trap-to-kill-and-replace recovery (R-0007-e/f/h, R-0016-c, Task 22).
//!
//! # What this module owns
//!
//! When a plugin invocation breaches a resource limit (epoch deadline or fuel
//! exhaustion), Wasmtime raises a trap on the breaching call. This module is the
//! single seam that:
//!
//!   1. Catches the Wasmtime trap (a `Result::Err` carrying a `wasmtime::Trap`)
//!      — NEVER lets it propagate as a host-process panic (R-0007-f).
//!   2. Classifies the trap as an epoch-deadline or fuel breach.
//!   3. Emits a structured `plugin_limit_violation` event carrying the
//!      `(workspace_id, plugin_id, plugin_version, limit_type, limit_value)`
//!      payload, where `limit_value` is the POLICY constant for the limit type
//!      (`EPOCH_DEADLINE` / `FUEL_LIMIT` / `MEMORY_MAX_BYTES`), decoupled from the
//!      (possibly small) trigger budget used to provoke the trap quickly (R-0007-e).
//!   4. Poisons the pool slot that ran the breaching invocation and replaces it
//!      with a fresh, live instance SYNCHRONOUSLY — before the structured error
//!      returns — so the pool size does not decrease from a kill (R-0016-c).
//!   5. Returns a structured `PluginExecError` to the caller.
//!
//! # Fail-closed posture
//!
//! Every path in this module fails closed: a trap becomes a structured `Err`,
//! never a panic and never a silent `Ok`. There are no reachable `.unwrap()` /
//! `.expect()` / index-panics in the recovery path — lock poisoning is recovered
//! (`lock().unwrap_or_else(|e| e.into_inner())`), slot access uses `.get(..)`, and
//! trap classification is an exhaustive `match` with a fail-closed default.
//!
//! # Store-config split (epoch ⇆ fuel independence)
//!
//! `build_engine()` sets `consume_fuel(true)`, so a `Store` with no explicit fuel
//! defaults to ~0 fuel and traps on fuel instantly. `invoke_with_recovery`
//! therefore applies BOTH `budget.fuel` (via `Store::set_fuel`) and
//! `budget.epoch_deadline` (via `Store::set_epoch_deadline`), plus attaches the
//! `PluginResourceLimiter` for the memory ceiling. Callers choose which limit is
//! binding by sizing the budget: a small epoch deadline + `u64::MAX` fuel makes
//! the epoch deadline bind; a small fuel budget + large epoch deadline makes fuel
//! bind. This is what lets the two trap paths be tested independently.

use crate::plugin::limits::{EPOCH_DEADLINE, FUEL_LIMIT, MEMORY_MAX_BYTES, PluginResourceLimiter};
use crate::plugin::pool::PluginPool;

// ---------------------------------------------------------------------------
// ResourceBudget — per-invocation fuel + epoch deadline
// ---------------------------------------------------------------------------

/// The per-invocation resource budget applied to a `Store` before a verb runs.
///
/// `Default` sources both fields from the production policy constants
/// (`FUEL_LIMIT`, `EPOCH_DEADLINE`). Tests construct a budget with a small
/// binding limit (and a non-binding counterpart) to provoke a trap quickly while
/// the EMITTED `limit_value` still reports the policy constant — the trigger
/// budget and the reported policy value are deliberately decoupled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBudget {
    /// Fuel ticks granted to the store for this invocation (`Store::set_fuel`).
    pub fuel: u64,
    /// Epoch-deadline ticks beyond current granted to the store
    /// (`Store::set_epoch_deadline`). Each tick is 10ms of wall clock, driven by
    /// the supervised epoch-tick thread.
    pub epoch_deadline: u64,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            fuel: FUEL_LIMIT,
            epoch_deadline: EPOCH_DEADLINE,
        }
    }
}

// ---------------------------------------------------------------------------
// LimitType — which resource limit was breached
// ---------------------------------------------------------------------------

/// The class of resource limit a trap breached (R-0007-e).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitType {
    /// Epoch-interruption deadline (wall-clock budget) was reached.
    EpochDeadline,
    /// Fuel (compute budget) was exhausted.
    Fuel,
    /// The linear-memory ceiling was hit (deny from `PluginResourceLimiter`).
    Memory,
}

impl LimitType {
    /// The stable string tag for this limit type, used in the emitted event.
    pub fn as_str(&self) -> &'static str {
        match self {
            LimitType::EpochDeadline => "epoch_deadline",
            LimitType::Fuel => "fuel",
            LimitType::Memory => "memory",
        }
    }

    /// The POLICY constant for this limit type — the value reported in the
    /// `plugin_limit_violation` event's `limit_value`. This is ALWAYS the
    /// production policy constant, NEVER the (possibly small) trigger budget.
    fn policy_value(&self) -> u64 {
        match self {
            LimitType::EpochDeadline => EPOCH_DEADLINE,
            LimitType::Fuel => FUEL_LIMIT,
            // MEMORY_MAX_BYTES is a usize ceiling; report it as u64 for the event.
            LimitType::Memory => MEMORY_MAX_BYTES as u64,
        }
    }
}

// ---------------------------------------------------------------------------
// PluginLimitViolation — the structured event payload (R-0007-e)
// ---------------------------------------------------------------------------

/// The structured payload emitted on a resource-limit violation (R-0007-e).
///
/// Carried back to the caller (via `PluginExecError::limit_violation`) AND
/// emitted as a `tracing` event named `plugin_limit_violation`. `limit_value` is
/// the POLICY constant for `limit_type` (see `LimitType::policy_value`), decoupled
/// from the trigger budget — shrinking the trigger budget for test speed cannot
/// hide a wrong production limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginLimitViolation {
    /// The workspace the breaching invocation ran under.
    pub workspace_id: uuid::Uuid,
    /// The plugin identity (synthetic in Task-22 tests; real in Task 23).
    pub plugin_id: String,
    /// The plugin version string.
    pub plugin_version: String,
    /// Which resource limit was breached.
    pub limit_type: LimitType,
    /// The POLICY constant for `limit_type` — NEVER the trigger budget.
    pub limit_value: u64,
}

// ---------------------------------------------------------------------------
// PluginExecError — the structured caller error (R-0007-e)
// ---------------------------------------------------------------------------

/// A structured error returned to the caller when a plugin invocation fails on a
/// resource-limit breach (R-0007-e). Surfaces a stable error `code`, the plugin
/// + verb identity, and the optional `PluginLimitViolation` payload.
#[derive(Debug)]
pub struct PluginExecError {
    code: &'static str,
    plugin: String,
    verb: String,
    // Boxed so the `Err` variant of `invoke_with_recovery`'s `Result` stays small
    // (clippy::result_large_err): the violation payload is the heavy field and is
    // absent on non-limit errors. `limit_violation()` still hands out a borrow.
    violation: Option<Box<PluginLimitViolation>>,
}

impl PluginExecError {
    /// The stable error code: `"plugin_execution_timeout"` (epoch deadline) or
    /// `"plugin_resource_exhausted"` (fuel exhaustion).
    pub fn code(&self) -> &str {
        self.code
    }

    /// The plugin identity the failing invocation targeted.
    pub fn plugin(&self) -> &str {
        &self.plugin
    }

    /// The verb the failing invocation targeted.
    pub fn verb(&self) -> &str {
        &self.verb
    }

    /// The structured limit-violation payload, if this error is a limit breach.
    pub fn limit_violation(&self) -> Option<&PluginLimitViolation> {
        self.violation.as_deref()
    }
}

impl std::fmt::Display for PluginExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "plugin execution error [{}] plugin='{}' verb='{}'",
            self.code, self.plugin, self.verb
        )
    }
}

impl std::error::Error for PluginExecError {}

/// The output bytes produced by a successful plugin invocation.
///
/// At V0 the verb fixtures used in Task-22 tests never return successfully (they
/// trap), so the success branch is currently unreachable in tests. `Vec<u8>` is
/// the forward-compatible carrier for real verb output wired in Task 23.
pub type Output = Vec<u8>;

// ---------------------------------------------------------------------------
// Trap classification
// ---------------------------------------------------------------------------

/// Classify a Wasmtime call error into the breached `LimitType`.
///
/// Returns `Some(limit_type)` if the error downcasts to a `wasmtime::Trap` that
/// is one of the recognised resource-limit traps; `None` otherwise (a non-limit
/// trap or a non-trap error). Fail-closed: an unrecognised trap classifies as
/// `None`, which the caller treats as an error path, never as success.
fn classify_trap(err: &wasmtime::Error) -> Option<LimitType> {
    // wasmtime::Error is an anyhow-style error; the trap is downcast by ref.
    let trap = err.downcast_ref::<wasmtime::Trap>()?;
    match trap {
        // Epoch-interruption deadline reached → epoch-deadline breach.
        wasmtime::Trap::Interrupt => Some(LimitType::EpochDeadline),
        // Fuel budget exhausted → fuel breach.
        wasmtime::Trap::OutOfFuel => Some(LimitType::Fuel),
        // NOTE: memory-ceiling denial does NOT surface as a classifiable trap in
        // wasmtime 45. When `PluginResourceLimiter::memory_growing` returns
        // `Ok(false)`, `Memory::grow` returns `Ok(None)` → the `memory.grow` wasm
        // instruction yields `-1` to the guest (the standard grow-failed sentinel);
        // the guest is NOT trapped. `Trap::MemoryOutOfBounds` is raised only on a
        // genuine out-of-bounds load/store (a guest bug), so it must NOT be tagged
        // as a `LimitType::Memory` reliability event — that would be a false
        // signal. Memory-ceiling classification (which needs the guest to trap on
        // the denial, e.g. a host-fn boundary check) is a Task-23 followup;
        // `LimitType::Memory` is retained for that future wiring + the event tag.
        //
        // Any non-fuel/non-epoch trap is therefore NOT a resource-limit breach
        // here; fall through to the generic `plugin_invocation_failed` path.
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// invoke_with_recovery — the public recovery seam
// ---------------------------------------------------------------------------

/// Invoke `verb` on `plugin_name` through the trap-recovery path (R-0007-e/f,
/// R-0016-c).
///
/// Borrows a live slot for `plugin_name`, instantiates a fresh `Store` configured
/// with `budget` (fuel + epoch deadline) and the memory limiter, instantiates the
/// slot's module, and calls the `run` export. If the call traps on a resource
/// limit:
///
///   - the trap is caught (never propagated as a panic, R-0007-f);
///   - a `plugin_limit_violation` event is emitted with the policy `limit_value`;
///   - the breaching slot is poisoned and replaced with a fresh live instance
///     SYNCHRONOUSLY, before this function returns (R-0016-c — pool size is
///     preserved);
///   - a structured `PluginExecError` is returned.
///
/// On a successful (non-trapping) call the `Output` bytes are returned.
pub fn invoke_with_recovery(
    pool: &PluginPool,
    plugin_name: &str,
    verb: &str,
    budget: ResourceBudget,
    workspace_id: uuid::Uuid,
) -> Result<Output, PluginExecError> {
    // Take the live instance OUT of a populated slot. The slot is now empty —
    // genuinely killed pending replacement — so a trap on this instance does not
    // leave a stale live entry behind. A missing plugin is a structured error,
    // never a panic.
    let mut live = match pool.take_live_invocation(plugin_name) {
        Some(l) => l,
        None => {
            return Err(PluginExecError {
                code: "plugin_not_registered",
                plugin: plugin_name.to_owned(),
                verb: verb.to_owned(),
                violation: None,
            });
        }
    };

    let plugin_id = live.plugin_id.clone();
    let plugin_version = live.plugin_version.clone();
    let slot_index = live.slot_index;

    // Apply the per-invocation budget to the live slot's store. `set_fuel` can
    // fail only if fuel metering is disabled on the engine (it is enabled in
    // `build_engine`); treat a failure fail-closed. The slot is repopulated even
    // on this early-return path so the pool size is preserved.
    if let Err(e) = live.store.set_fuel(budget.fuel) {
        pool.repopulate_slot(plugin_name, slot_index);
        return Err(PluginExecError {
            code: "plugin_store_config_failed",
            plugin: plugin_id.clone(),
            verb: verb.to_owned(),
            violation: classify_trap(&e).map(|limit_type| {
                Box::new(PluginLimitViolation {
                    workspace_id,
                    plugin_id: plugin_id.clone(),
                    plugin_version: plugin_version.clone(),
                    limit_type,
                    limit_value: limit_type.policy_value(),
                })
            }),
        });
    }
    live.store.set_epoch_deadline(budget.epoch_deadline);

    // Call the `run` export on the slot's LIVE instance. The instance + store were
    // taken from the slot; a trap here poisons THIS store (the genuine live entry),
    // which is dropped at end of scope. This is the breaching invocation.
    let call_result = run_verb(&mut live.store, &live.instance);

    // GENUINE kill-and-replace (R-0016-c): whatever the outcome, instantiate a
    // fresh live instance back into the slot synchronously — before this function
    // returns. The trapped `live.store`/`live.instance` are dropped when `live`
    // goes out of scope. The pool size is therefore unchanged across the call.
    pool.repopulate_slot(plugin_name, slot_index);

    match call_result {
        Ok(output) => {
            // Successful invocation — return the verb output. (Unreached by
            // Task-22 fixtures, which always trap.)
            Ok(output)
        }
        Err(err) => {
            // A trap (or other call error) occurred. Classify it. (The slot was
            // already repopulated above — kill-and-replace is complete.)
            let limit_type = classify_trap(&err);

            match limit_type {
                Some(limit_type) => {
                    let violation = PluginLimitViolation {
                        workspace_id,
                        plugin_id: plugin_id.clone(),
                        plugin_version: plugin_version.clone(),
                        limit_type,
                        limit_value: limit_type.policy_value(),
                    };

                    // Emit the structured event (R-0007-e). Mirrors the
                    // `auth::token::TokenRotatedEvent` event pattern: a named
                    // `tracing` event carrying the full payload.
                    tracing::warn!(
                        event = "plugin_limit_violation",
                        workspace_id = %violation.workspace_id,
                        plugin_id = %violation.plugin_id,
                        plugin_version = %violation.plugin_version,
                        limit_type = violation.limit_type.as_str(),
                        limit_value = violation.limit_value,
                        "plugin invocation breached a resource limit — slot killed and replaced (R-0007-e)"
                    );

                    let code = match limit_type {
                        LimitType::EpochDeadline => "plugin_execution_timeout",
                        LimitType::Fuel => "plugin_resource_exhausted",
                        LimitType::Memory => "plugin_resource_exhausted",
                    };

                    Err(PluginExecError {
                        code,
                        plugin: plugin_id,
                        verb: verb.to_owned(),
                        violation: Some(Box::new(violation)),
                    })
                }
                None => {
                    // A non-limit trap or other call failure. Still recovered via
                    // kill-and-replace; surface a generic structured error rather
                    // than a panic (R-0007-f).
                    tracing::warn!(
                        event = "plugin_invocation_failed",
                        plugin_id = %plugin_id,
                        verb,
                        error = %err,
                        "plugin invocation failed (non-limit) — slot killed and replaced"
                    );
                    Err(PluginExecError {
                        code: "plugin_invocation_failed",
                        plugin: plugin_id,
                        verb: verb.to_owned(),
                        violation: None,
                    })
                }
            }
        }
    }
}

/// Call the `run` export on a pre-instantiated `instance` bound to `store`.
///
/// The `run` export takes no params and returns no results (per the Task-22
/// fixtures and the V0 verb shape). Any trap raised during the call is returned as
/// the `Err` for the caller to classify — it is NEVER allowed to escape as a panic.
/// The `instance` was instantiated into `store` at slot population time, so this
/// only resolves the export and calls it.
fn run_verb(
    store: &mut wasmtime::Store<PluginResourceLimiter>,
    instance: &wasmtime::Instance,
) -> Result<Output, wasmtime::Error> {
    let run = instance.get_typed_func::<(), ()>(&mut *store, "run")?;
    run.call(&mut *store, ())?;
    // The V0 verb shape returns no bytes through the export; success yields empty
    // output. Real output marshalling is Task 23 scope.
    Ok(Vec::new())
}
