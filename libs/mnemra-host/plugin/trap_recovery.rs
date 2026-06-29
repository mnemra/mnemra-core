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
//! # Panic-safe seam (T13-g HIGH-2 / MEDIUM-1)
//!
//! WIT host-fn return types are fixed by the interface (e.g. `String`,
//! `Option<String>`, void). The only way a host-fn can signal an error to the
//! caller is to `panic!`. Without protection those panics unwind out of
//! `call(...)` into `invoke_through_recovery`, skipping `repopulate_slot` and
//! permanently draining the pool slot (DoS — HIGH-2). They also escape the
//! `spawn_blocking` boundary and surface raw DB internals in the MCP response
//! via `JoinError::Display` (MEDIUM-1).
//!
//! `invoke_through_recovery` now wraps `call(...)` in
//! `std::panic::catch_unwind(AssertUnwindSafe(...))`. Caught panics are:
//! - Converted to `PluginExecError { code: "plugin_invocation_failed" }` with
//!   the panic payload DISCARDED (no sqlx/Postgres text reaches the client).
//! - Followed unconditionally by `repopulate_slot` (pool size preserved).
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

use wasmtime::Store;
use wasmtime::component::Instance;

use crate::plugin::component::HostState;
use crate::plugin::limits::{EPOCH_DEADLINE, FUEL_LIMIT, MEMORY_MAX_BYTES};
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
// invoke_through_recovery — the SINGLE kill-and-replace seam (R-0007-e/f, R-0016-c)
// ---------------------------------------------------------------------------

// A future `panic = "abort"` would turn the catch_unwind below into dead code,
// silently converting the fail-closed seam into a process abort. Fail the build instead.
const _: () = assert!(cfg!(panic = "unwind"));

/// The one take -> bind-ws -> budget -> invoke -> repopulate -> classify -> emit
/// seam, generic over the export call. BOTH the production typed-`content`
/// dispatch path (`invoke_content`) and the resource-limit trap-recovery path
/// (`invoke_with_recovery`) go through this function — so the R-0007 trap tests
/// in `resource_limits.rs` exercise the EXACT machinery the request path runs
/// (no duplicated copy that production never executes).
///
/// Steps:
///   - borrow a live slot for `plugin_name` (missing plugin -> structured error);
///   - bind the host-derived `workspace_id` onto the store's `HostState` at the
///     single dispatch site (R-0006-b); trap fixtures ignore it harmlessly;
///   - apply the per-invocation budget (fuel + epoch, R-0007-g), fail-closed;
///   - run `call(store, instance)` — the breaching invocation;
///   - SYNCHRONOUSLY repopulate the slot before returning (R-0016-c — pool size
///     preserved), whatever the outcome;
///   - on a trap: classify (epoch/fuel), emit the `plugin_limit_violation` event
///     with the POLICY `limit_value`, and return a structured `PluginExecError`
///     (never a panic, R-0007-f).
fn invoke_through_recovery<R>(
    pool: &PluginPool,
    plugin_name: &str,
    verb: &str,
    budget: ResourceBudget,
    workspace_id: uuid::Uuid,
    pg_pool_opt: Option<&sqlx::PgPool>,
    call: impl FnOnce(&mut Store<HostState>, &Instance) -> Result<R, wasmtime::Error>,
) -> Result<R, PluginExecError> {
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

    // ========================= TENANT-ISOLATION LINCHPIN =====================
    // R-0006-b / R-0006-e — THE workspace scoping key derivation site.
    //
    // `workspace_id` here is HOST-DERIVED: it came from the authenticated
    // `WorkspaceCtx` constructed at the single dispatch site in `call_tool`
    // (auth_and_authorize -> ctx.workspace_id()). We bind it onto the store's
    // `HostState` HERE, BEFORE the guest export runs. The `artifact-*` host-fn
    // import bodies read `self.workspace_id` from this `HostState`
    // (libs/mnemra-host/plugin/component.rs) — they NEVER read the
    // `workspace-ctx` value the guest passes across the import boundary. The
    // guest cannot supply, forge, or influence the scoping key (R-0006-e: no
    // plugin-supplied scoping key). This is what makes cross-workspace access
    // structurally impossible — verified observably by Glitch's Test B
    // (cross_workspace_get_returns_none) and enforced at the fenced-map key
    // `(workspace_id, id)` (R-0006-d). Changing this binding, or making any
    // host-fn body trust a guest-supplied workspace value, is a tenant-isolation
    // breach. (Trap fixtures never read it; binding here is harmless for them.)
    // ========================================================================
    live.store.data_mut().set_workspace_id(workspace_id);
    // T13: bind the Postgres pool for artifact host-fn persistence. `None` for
    // resource-limit trap fixtures (`invoke_with_recovery` path) that never call
    // artifact host-fns. The pool is not stored across invocations (pool is set
    // fresh per-dispatch from `invoke_content`'s caller-supplied reference).
    if let Some(p) = pg_pool_opt {
        live.store.data_mut().set_pool(p.clone());
    }

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

    // Run the export through a panic-safe seam (HIGH-2, MEDIUM-1, T13-g).
    //
    // Host-fn bodies (artifact_create/update/delete/get/list in component.rs)
    // panic on DB errors to signal failure from within a WIT return-type that
    // provides no other error channel. Without `catch_unwind`, that panic
    // propagates out of `call(...)`, skips `repopulate_slot`, permanently
    // drains the pool slot (DoS — HIGH-2), and leaks the raw sqlx/Postgres
    // error string into the MCP response via JoinError::Display (MEDIUM-1).
    //
    // `catch_unwind` catches those panics before they escape this function.
    // The slot is then repopulated regardless of outcome (R-0016-c). The caught
    // panic payload is intentionally DISCARDED — it is never embedded in the
    // returned `PluginExecError`, so no internal DB detail reaches the client.
    //
    // `AssertUnwindSafe` is correct here: `live.store`/`live.instance` are
    // dropped at end of scope after the catch (poisoned stores are never reused),
    // and no partially-mutated state is observed after the catch.
    let call_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        call(&mut live.store, &live.instance)
    }));

    // GENUINE kill-and-replace (R-0016-c): whatever the outcome — success, trap,
    // OR host-fn panic — instantiate a fresh live instance back into the slot
    // synchronously before this function returns. The old store/instance are
    // dropped when `live` goes out of scope. Pool size is unchanged.
    pool.repopulate_slot(plugin_name, slot_index);

    // Unwrap the two-layer result: outer = panic/no-panic, inner = wasmtime trap.
    let call_result = match call_result {
        Ok(inner) => inner,
        Err(_panic_payload) => {
            // A host-fn panicked (e.g. DB error in artifact_create/update/delete).
            // The panic payload is dropped here — it is NOT included in the error
            // returned to the caller (scrubbing, MEDIUM-1). The slot is already
            // repopulated above (HIGH-2 / R-0016-c).
            tracing::warn!(
                event = "plugin_host_fn_panic",
                plugin_id = %plugin_id,
                verb,
                "plugin host-fn panicked — slot killed and replaced, panic payload scrubbed"
            );
            return Err(PluginExecError {
                code: "plugin_invocation_failed",
                plugin: plugin_id,
                verb: verb.to_owned(),
                violation: None,
            });
        }
    };

    match call_result {
        Ok(value) => Ok(value),
        Err(err) => {
            // A Wasmtime trap (or other call error) occurred. Classify it. (The
            // slot was already repopulated above — kill-and-replace is complete.)
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

// ---------------------------------------------------------------------------
// invoke_with_recovery — the trap-recovery probe (resource-limit suite)
// ---------------------------------------------------------------------------

/// Invoke the world-level `run` export on `plugin_name` through the shared
/// kill-and-replace seam (R-0007-e/f, R-0016-c).
///
/// Used by the resource-limit suite: the trap fixtures are components exporting a
/// parameterless `run` that loops/burns. This is a thin caller over
/// `invoke_through_recovery` with the `run_verb` closure — the SAME seam the real
/// `content` dispatch path uses, so the R-0007 trap assertions cover production
/// machinery. Public signature unchanged (the suite is untouched).
pub fn invoke_with_recovery(
    pool: &PluginPool,
    plugin_name: &str,
    verb: &str,
    budget: ResourceBudget,
    workspace_id: uuid::Uuid,
) -> Result<Output, PluginExecError> {
    // Pass `None` for the pool — resource-limit trap fixtures never invoke
    // artifact host-fns, so no PgPool is needed on this path (T13).
    invoke_through_recovery(
        pool,
        plugin_name,
        verb,
        budget,
        workspace_id,
        None,
        run_verb,
    )
}

// ---------------------------------------------------------------------------
// Typed `content` invoke — the REAL dispatch path (T5/T6, R-0019-a)
// ---------------------------------------------------------------------------

/// Which typed `content` method to invoke, plus its marshalled arguments
/// (CC-MAPPING: `echo.create` -> `Create`, `echo.get` -> `Get`,
/// `echo.list` -> `List`).
#[derive(Debug, Clone)]
pub enum ContentCall {
    /// `content.create(type, frontmatter, body)` — returns the generated ULID.
    Create {
        /// The artifact type name (from the MCP `content_type` argument).
        type_name: String,
        /// The frontmatter JSON-as-string (from the MCP `payload` argument).
        frontmatter: String,
        /// The optional body.
        body: Option<String>,
    },
    /// `content.get(id)` — returns the stored content, or `None`.
    Get {
        /// The artifact id (from the MCP `id` argument).
        id: String,
    },
    /// `content.list(type, filters)` — returns the ids visible in the workspace.
    List {
        /// The artifact type name (from the MCP `content_type` argument).
        type_name: String,
        /// The filter JSON-as-string (from the MCP `filters` argument). Threaded
        /// through but not applied this slice (predicate logic deferred — #1846).
        filters: String,
    },
    /// `content.update(id, frontmatter_patch, body)` — merge the frontmatter
    /// patch into the stored frontmatter and (when `body` is `Some`) replace the
    /// body; returns nothing (the WIT `update` is void).
    Update {
        /// The artifact id (from the MCP `id` argument).
        id: String,
        /// The frontmatter patch JSON-as-string (from the MCP `frontmatter_patch`
        /// argument). JSON-merged into the stored frontmatter: patch keys
        /// overwrite/add; keys absent from the patch are preserved.
        frontmatter_patch: String,
        /// The optional new body. `Some` replaces the stored body; `None` (the MCP
        /// `body` key ABSENT) leaves the stored body unchanged.
        body: Option<String>,
    },
    /// `content.delete(id)` — remove the artifact; a missing or cross-workspace
    /// artifact is a silent no-op (the fenced-map key misses). Returns nothing.
    Delete {
        /// The artifact id (from the MCP `id` argument).
        id: String,
    },
}

/// The typed result of a successful `content` invoke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentResult {
    /// `content.create` returned the generated id.
    Created(String),
    /// `content.get` returned the stored content (or `None`).
    Got(Option<String>),
    /// `content.list` returned the ids visible in the workspace.
    Listed(Vec<String>),
    /// `content.update` completed (the WIT `update` is void — no payload).
    Updated,
    /// `content.delete` completed (the WIT `delete` is void — no payload).
    Deleted,
}

/// Invoke the typed `content` export on a pooled component instance through the
/// shared kill-and-replace seam, scoped to `workspace_id` (R-0019-a, R-0006-b,
/// R-0007).
///
/// This is the REAL request dispatch path. It is a thin caller over
/// `invoke_through_recovery` with the typed-`content` closure — so a trap during
/// a `content` invoke gets the IDENTICAL classify -> emit -> kill-and-replace ->
/// structured-error treatment the resource-limit suite asserts, on the same code.
pub fn invoke_content(
    pool: &PluginPool,
    plugin_name: &str,
    verb: &str,
    call: ContentCall,
    budget: ResourceBudget,
    workspace_id: uuid::Uuid,
    pg_pool: &sqlx::PgPool,
) -> Result<ContentResult, PluginExecError> {
    invoke_through_recovery(
        pool,
        plugin_name,
        verb,
        budget,
        workspace_id,
        Some(pg_pool),
        |store, instance| match &call {
            ContentCall::Create {
                type_name,
                frontmatter,
                body,
            } => crate::plugin::component::content_create(
                store,
                instance,
                type_name,
                frontmatter,
                body.as_deref(),
            )
            .map(ContentResult::Created),
            ContentCall::Get { id } => {
                crate::plugin::component::content_get(store, instance, id).map(ContentResult::Got)
            }
            ContentCall::List { type_name, filters } => {
                crate::plugin::component::content_list(store, instance, type_name, filters)
                    .map(ContentResult::Listed)
            }
            ContentCall::Update {
                id,
                frontmatter_patch,
                body,
            } => crate::plugin::component::content_update(
                store,
                instance,
                id,
                frontmatter_patch,
                body.as_deref(),
            )
            .map(|()| ContentResult::Updated),
            ContentCall::Delete { id } => {
                crate::plugin::component::content_delete(store, instance, id)
                    .map(|()| ContentResult::Deleted)
            }
        },
    )
}

// ---------------------------------------------------------------------------
// White-box unit tests — panic-safe seam (HIGH-2 / MEDIUM-1, T13-g)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::pool::{POOL_MAX, PluginPool};
    use wasmtime::component::Component;

    /// Plugin ID used only in this test module — must not collide with integration
    /// test fixtures ("burn", "echo").
    const PANIC_TEST_PLUGIN: &str = "panic_seam_test";
    const PANIC_TEST_VERSION: &str = "0.0.1";
    /// Arbitrary verb label; the seam does not inspect it.
    const PANIC_TEST_VERB: &str = "create";

    /// A minimal no-op component: exports the bare `run: func()` symbol used by
    /// `invoke_with_recovery`. No WIT imports — the host linker satisfies only what
    /// the component actually imports, so this compiles and instantiates cleanly.
    const NOOP_WAT: &str = r#"(component
  (core module $m
    (func (export "run")))
  (core instance $i (instantiate $m))
  (func (export "run") (canon lift (core func $i "run"))))"#;

    fn pool_with_noop() -> PluginPool {
        let pool = PluginPool::new().expect("pool must initialise");
        let component =
            Component::new(pool.engine(), NOOP_WAT).expect("noop WAT component must compile");
        pool.register_module(PANIC_TEST_PLUGIN, PANIC_TEST_VERSION, &component)
            .expect("register_module must succeed for noop component");
        pool
    }

    /// HIGH-2 + MEDIUM-1 white-box: a host-fn panic leaves the pool slot
    /// replenished and the returned error is scrubbed of DB internals.
    ///
    /// The `call` closure simulates an `artifact_update` panicking with a payload
    /// that contains raw DB text — exactly the strings that would surface in the
    /// MCP response before this fix. Asserts:
    ///
    /// 1. `slot_count` equals `POOL_MAX` after the invoke (kill-and-replace ran
    ///    correctly — HIGH-2 / R-0016-c).
    /// 2. `invoke_through_recovery` returned `Err` (panic did not become Ok).
    /// 3. The error code is `"plugin_invocation_failed"` (generic scrubbed code).
    /// 4. The error's `Display` does NOT contain any of the forbidden DB tokens
    ///    (`echo_fixture`, `sqlx`, `constraint`, `column`) — MEDIUM-1 scrub.
    #[test]
    fn host_fn_panic_repopulates_slot_and_scrubs_error() {
        let pool = pool_with_noop();
        assert_eq!(
            pool.slot_count(PANIC_TEST_PLUGIN),
            POOL_MAX,
            "pool should be fully populated before invoke"
        );

        // The panic payload contains raw DB internals — exactly the strings
        // Warden flagged as MEDIUM-1 forbidden content.
        let result: Result<(), PluginExecError> = invoke_through_recovery(
            &pool,
            PANIC_TEST_PLUGIN,
            PANIC_TEST_VERB,
            ResourceBudget::default(),
            uuid::Uuid::nil(),
            None,
            |_store, _instance| {
                panic!(
                    "artifact_update UPDATE failed: sqlx DB error — relation \
                     \"echo_fixture\" violates constraint; column frontmatter \
                     invalid input syntax for type json"
                )
            },
        );

        // 1. Slot was repopulated (HIGH-2 / R-0016-c): pool size unchanged.
        assert_eq!(
            pool.slot_count(PANIC_TEST_PLUGIN),
            POOL_MAX,
            "HIGH-2: pool slot must be repopulated after a host-fn panic"
        );

        // 2. A panicking host-fn surfaces as Err, never Ok.
        let err = result.expect_err("panicking host-fn must surface as Err");

        // 3. Error code is the generic scrubbed code, not the raw panic payload.
        assert_eq!(
            err.code(),
            "plugin_invocation_failed",
            "HIGH-2/MEDIUM-1: error code must be the scrubbed generic string"
        );

        // 4. Error Display must not expose any of the forbidden DB token strings.
        let display = err.to_string();
        for forbidden in &["echo_fixture", "sqlx", "constraint", "column"] {
            assert!(
                !display.contains(forbidden),
                "MEDIUM-1: error Display must not expose '{}'; got: {display}",
                forbidden
            );
        }
    }
}

/// Call the world-level `run` export on a pre-instantiated component `instance`
/// bound to `store` (CC-RUNVERB-REWRITE).
///
/// This is the trap-recovery probe used by the resource-limit suite: the trap
/// fixtures are COMPONENTS exporting a parameterless world-level `run: func()`
/// that loops (epoch) or burns fuel. A trap raised during the call is returned
/// as the `Err` for the caller to classify — NEVER allowed to escape as a panic
/// (R-0007-f). The component-typed `content` invoke for the REAL dispatch path
/// lives in `crate::plugin::component::content_create` / `content_get`; both the
/// trap path here and the real path go through this same kill-and-replace seam
/// (`invoke_with_recovery` for the trap path; the dispatch site applies the same
/// take→budget→invoke→repopulate discipline for the real path).
///
/// The move from the core-module `get_typed_func::<(),()>("run")` to the
/// component `Instance::get_typed_func` is the CC-RUNVERB-REWRITE: the instance
/// is a `component::Instance`, not a core `wasmtime::Instance`, but the trap
/// classification (epoch/fuel, at the `wasmtime::Trap` level) is unchanged.
fn run_verb(store: &mut Store<HostState>, instance: &Instance) -> Result<Output, wasmtime::Error> {
    let run = instance.get_typed_func::<(), ()>(&mut *store, "run")?;
    run.call(&mut *store, ())?;
    // The trap fixtures' `run` returns no values; success yields empty output.
    // (The real dispatch path returns the typed `content` value, not via here.)
    Ok(Vec::new())
}
