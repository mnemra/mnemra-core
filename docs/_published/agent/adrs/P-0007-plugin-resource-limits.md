---
title: "P-0007: Plugin Resource Limits"
summary: "Defines V0 Wasmtime per-instance resource limits: fuel budget (CPU ceiling), epoch-interruption deadline, memory ceiling, and table/instance limits. Both fuel and epoch-interruption are ON at V0."
primary-audience: agent
---

---
status: "accepted"
date: "2026-05-24"
decision-makers: ["the maintainer"]
consulted: ["the orchestrator", "the security reviewer"]
informed: []
supersedes: null
superseded_by: null
---

# P-0007: Plugin Resource Limits

## Status

`accepted`

## Context and Problem Statement

Mnemra-core hosts plugins in-process via Wasmtime (WebAssembly Component Model). Plugins run in a Wasmtime sandbox that is structurally IO-free — they have no ambient network, filesystem, or clock authority. However, without resource limits, a plugin can still damage the host process by:

- **Infinite looping (CPU starvation):** A plugin that never yields blocks the thread servicing a verb, and potentially the entire host process if the plugin pool is exhausted.
- **Unbounded allocation (memory exhaustion):** A plugin that allocates without bound can OOM the host process.
- **WASM table / instance count explosion:** Pathological WASM modules can instantiate large tables or nested instances.

The Frame's quality-attribute tree commits to sandbox security outcomes: "an infinite-loop plugin is killed and replaced from the pool; no single-process-wide DoS via plugin." Achieving these outcomes requires the resource limits to be ON at V0, not deferred.

The Frame's Tier-A table (renamed/promoted from `{{P-PluginPoolMemory}}`) specifies: **fuel (CPU ceiling) + epoch-interruption (deadline-based preemption) + memory ceiling + table/instance limits**. This ADR locks the concrete V0 values for each limit.

`P-StackDiscipline` applies: Wasmtime's resource knobs are stack-aligned and already present in the Wasmtime library at the versions mnemra-core targets. This is a configuration decision, not a build decision.

## Decision Drivers

- **P-StackDiscipline.** Wasmtime's fuel, epoch, and memory-limit APIs are the stack's native DoS-containment mechanism. Using them is not an addition to the design — it is the design.
- **Fail-replace, not fail-hang.** `P-plugin-runtime`/D (Medium, severity 70): "A plugin infinite-loops or allocates without bound; absent kill-and-replace from the pool, the host hangs the verb." The resource-limit mechanism must surface a recoverable error, not a host-process hang or panic.
- **Both fuel AND epoch-interruption ON at V0.** The Frame is explicit: both mechanisms are ON. They are complementary — fuel catches deterministic over-use (CPU ticks consumed), epoch-interruption catches real-time deadline expiry. Using both avoids a single mechanism's failure mode.
- **Values must allow legitimate work.** V0 `core: true` plugins are well-behaved, built-in artifacts. The resource limits must not reject a correctly-written embedding call or a search operation.
- **Values must reject DoS-scale abuse.** A misbehaving or tampered plugin must be terminated before it saturates the host process.

## Considered Options

The structure of the resource-limit mechanism is fixed by the Frame (fuel + epoch + memory + table/instance). The ADR choice is the V0 default values for each limit, and whether the two CPU-containment mechanisms (fuel and epoch) are both ON or only one.

### Option A — Both fuel and epoch ON; conservative V0 floor values

Fuel budget: 10 billion ticks (a rough proxy for ~1–2 seconds of CPU for a typical plugin invocation, depending on WASM instruction mix). Epoch deadline: 5 seconds real-time per verb invocation. Memory ceiling: 64 MiB per instance. Table/instance limits: default Wasmtime maximums (conservative; most V0 `core: true` plugins use negligible table/instance counts).

### Option B — Epoch-interruption only (no fuel)

Epoch-interruption provides a real-time deadline; fuel is disabled. Simpler to reason about (no per-instruction tick accounting), but provides no protection against tight spinning that advances the epoch counter slowly (or if epoch advancement is delayed due to host scheduler pressure).

### Option C — Fuel only (no epoch-interruption)

Fuel provides deterministic tick-budget enforcement but cannot enforce wall-clock deadlines. A plugin that sleeps (via WASM yield points, if available) could game fuel accounting. Also: fuel does not fire if the plugin enters a WASM host-fn call that runs indefinitely on the host side — the epoch interrupt is the backstop for host-fn-induced blocking.

### Option D — All limits higher (lenient floor)

Same mechanism as Option A but with 5× higher fuel (50B ticks) and 256 MiB memory. Reduces false-positive terminations for resource-intensive operations that might be legitimate in future V0.1+ plugins.

## Decision Outcome

**Option A** — both fuel and epoch ON; conservative V0 floor values.

**Rationale:** The two mechanisms are complementary, and both are present in Wasmtime at no additional dependency cost. Fuel catches deterministic over-use; epoch-interruption catches real-time deadline expiry including cases where fuel accounting underestimates real-time cost (e.g., host-fn calls that block). Using both satisfies the Frame commitment without adding complexity.

Conservative floor values are correct for V0: `core: true` plugins are well-behaved. If a well-written V0 plugin exceeds 10B fuel ticks or 5 seconds wall-clock on a real operation, that is a signal the operation should be redesigned (chunked, async, etc.), not a signal to raise the ceiling.

Option B (epoch only) is weaker against deterministic CPU abuse. Option C (fuel only) cannot enforce wall-clock deadlines and has the host-fn blocking gap. Option D (lenient floor) defers the DoS-containment problem to the point where a bad actor could exploit it.

### V0 resource limit values

| Limit | V0 value | Mechanism | Rationale |
|---|---|---|---|
| Fuel budget | 10,000,000,000 ticks (10B) | `Store::set_fuel` | Rough proxy for ~1–2s CPU at V0 plugin workloads; deterministic; well-behaved `core: true` plugins complete well within this budget |
| Epoch deadline | 5 seconds wall-clock per verb invocation | `Store::set_epoch_deadline` + host epoch-tick thread | Backstop for host-fn blocking and scheduler jitter; fires regardless of fuel consumption |
| Memory ceiling | 64 MiB per instance | `Config::static_memory_maximum_size` / `ResourceLimiter` | Sufficient for embedding operations + search result buffers; rejects unbounded allocation |
| WASM table size | Wasmtime default maximum | `Config::max_wasm_stack` + table resource limiter | V0 `core: true` plugins use no pathological table patterns; default is appropriate |
| Instance count | 1 per pool slot | Pool architecture (one instance per slot) | Instance count is bounded by the pool size (3–5 at V0); no nested instantiation |
| Plugin pool size | 3–5 per plugin type | Pool architecture | Per architecture alignment record round-5; unchanged here |

**Epoch tick configuration:** The host must advance the epoch counter on a timer (Wasmtime's epoch mechanism requires the host to call `Engine::increment_epoch()` on a schedule). V0 value: advance every 10ms from a host-side background thread. With a 5-second deadline, `set_epoch_deadline(500)` (500 × 10ms = 5s) achieves the wall-clock bound.

### Kill-and-replace invariant

When a resource-limit violation fires (fuel exhaustion, epoch deadline, memory ceiling):

1. The Wasmtime `Store` traps with the limit error.
2. The host catches the trap, logs a structured event to `DS-ts-events` with `(workspace_id, plugin_id, plugin_version, limit_type, limit_value)` for attribution (`P-plugin-runtime`/R mitigation).
3. The pool slot is poisoned and replaced: a new instance is created for the pool slot; the caller receives an error for the current verb invocation.
4. The host does NOT propagate the Wasmtime trap as a process panic. The pool recover is the isolation invariant.

The kill-and-replace path is the primary `P-plugin-runtime`/D mitigation.

### Consequences

**Good:**
- `P-plugin-runtime`/D (Medium) mitigated: fuel + epoch-interruption enforce the kill-and-replace path; unbounded CPU or allocation does not hang the host verb.
- `P-plugin-runtime`/R (Medium) partially mitigated: the structured limit-violation event (with signed-artifact identity) provides attribution for the operator.
- Both mechanisms are on the Wasmtime `Config` + `Store` API surface; no build-system changes required.
- Conservative floor values give signal when a `core: true` plugin is unexpectedly resource-intensive.

**Bad / Trade-offs:**
- Fuel tick counts are WASM-instruction-level, not wall-clock seconds; the fuel budget is approximate. A future V0 plugin that does more complex operations (large embedding contexts) may need tuning. Floor values can be adjusted via configuration without changing the mechanism.
- The host epoch-tick thread must be started before any plugin is invoked. This adds a background task to the host lifecycle that must be tracked (started before first use, not restarted on crash without supervision).
- Memory ceiling of 64 MiB is per-instance; a pool of 5 instances of one plugin type can consume up to 320 MiB of WASM linear memory at peak. This is acceptable for V0 dogfood; it is an input to capacity planning for V0.1+.

## Pros and Cons of the Options

### Option A — Both fuel and epoch ON; conservative V0 floor values (accepted)

- Pro: Complementary mechanisms cover deterministic CPU abuse (fuel) and real-time deadline expiry including host-fn blocking (epoch).
- Pro: Conservative values front-load the DoS-containment bar; tuning upward is always possible; the security argument is valid at conservative values.
- Con: Two mechanisms to reason about (fuel-ticks vs epoch-ticks); mitigated by documenting both in this ADR.

### Option B — Epoch-interruption only

- Con: No defense against deterministic tight CPU spinning that doesn't consume wall-clock time relative to the epoch-tick advance schedule.
- Con: Fuel accounting is zero-cost at the Wasmtime level once enabled; omitting it is a free weakening.

### Option C — Fuel only

- Con: No wall-clock deadline; a plugin blocked inside a host-fn call (I/O that the host issues on the plugin's behalf) is not constrained by fuel.
- Con: Fuel can be gamed by yield-heavy WASM patterns if the plugin author controls compilation.

### Option D — Lenient floor values

- Con: Higher values increase the blast-radius window before a bad plugin is terminated.
- Con: For V0 `core: true` plugins (well-behaved, built-in), the lenient floor provides no practical benefit over conservative values.

## More Information

- Frame doc open ADR slot: `{{P-PluginResourceLimits}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table; promoted from Tier-C `{{P-PluginPoolMemory}}`).
- Stage 2a direction M3: rename `{{P-PluginPoolMemory}}` → `{{P-PluginResourceLimits}}` and promote to Tier A; both fuel and epoch ON at V0.
- Threat references: `P-plugin-runtime`/D,R,T; `P-plugin-instance`/T ([Overview](../architecture/overview.md)).
- Trust boundary `TB-mnemra-host` ↔ `TB-plugin-sandbox`: host-mediated; cross-plugin calls traverse the host; resource limits are the DoS-containment mechanism within this boundary (overview trust-boundary table).
- Pool size 3–5 per plugin type: architecture alignment record round-5 (referenced in Frame reconciliation table).
- Wasmtime fuel API: `Store::set_fuel`, `Store::fuel_consumed`; epoch API: `Engine::increment_epoch`, `Store::set_epoch_deadline`. Both are stable in the Wasmtime version targeted by mnemra-core.
