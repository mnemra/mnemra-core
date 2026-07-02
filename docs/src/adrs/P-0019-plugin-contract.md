---
title: "P-0019: Plugin Contract — Component Model + WIT ABI, Toolchain, and Lifecycle"
summary: "The foundational plugin-contract decision the substrate ADRs already assumed but none recorded: Path B (WebAssembly Component Model + WIT), locked end of session 4 (2026-05-04) and proven by the V0.01 spike (commit c9018e3). Locks the cross-cutting contract no single sibling owns — the WIT ABI shape (sync WIT both directions; Component Model Layer 1 only: async host feature ON, component-model-async / WIT-async-types OFF; per-concern interface granularity; @since/@unstable/@deprecated stability gates; native WIT structure with opaque JSON-as-string document payloads; host-derived workspace-ctx never a bare write-path parameter), the Wasmtime version floor (>=44.0.x, rationale), the guest toolchain (wasm32-wasip2 target directly via wit-bindgen, NOT cargo-component), the explicit closing of the Extism-migration door, and the composite plugin lifecycle sequence. Records four deferrals with named tripwires: WIT-async-types, cosign-on-wkg third-party distribution verification, the V0.1+ language-support matrix, and the Path-B retreat trigger. This is a capstone that CITES its siblings for the detail each owns (manifest schema -> P-0003; resource limits + kill-and-replace -> P-0007; runtime framework + rmcp SDK + exact Wasmtime pin -> P-0012; typed export/invocation ABI -> P-0013); it does not supersede or override any of them."
primary-audience: agent
---

---
status: "accepted"
date: "2026-07-02"
decision-makers: ["the maintainer"]
consulted: ["the orchestrator", "the researcher", "the security reviewer"]
informed: []
supersedes: null
superseded_by: null
overrides: null
---

# P-0019: Plugin Contract — Component Model + WIT ABI, Toolchain, and Lifecycle

**Project:** mnemra-core

## Status

`accepted`

This ADR records the **foundational plugin-contract decision** for the V0 substrate: the plugin model is **WebAssembly Component Model + WIT** (Path B), locked by the maintainer at the end of session 4 (**2026-05-04**) and validated by the V0.01 existence-test spike (commit `c9018e3`, on `main`, 2026-05-07). It is recorded `accepted`-on-landing in the same spirit as [P-0011-logging-facade](P-0011-logging-facade.md) and [P-0012-plugin-runtime-and-mcp-sdk](P-0012-plugin-runtime-and-mcp-sdk.md): the decision precedes and is already reflected in the tree; this ADR ratifies it, it does not re-open it.

**Why this ADR exists, and why late.** The Path-B decision was a brief Hard-constraint carry-forward. The Frame ([`docs/src/intent/mnemra-core-frame.md`](../intent/mnemra-core-frame.md), §"Correction 2" and §"Plugin runtime") *narrated* it — "Plugins are WebAssembly Component Model modules loaded in-process by Wasmtime, communicating with the host via WIT-defined host functions" — but, unlike the manifest schema and the resource limits, it was never given its own `{{P-XXX}}` open ADR slot. The substrate ADRs authored since (P-0003, P-0007, P-0012, P-0013) each **presuppose** Path B and lock a *piece* of the plugin contract, but the foundational plugin-model decision and the cross-cutting ABI-shape / toolchain / lifecycle / language-reach commitments never got a home. This ADR is that home.

**Capstone that cites — not a parent that overrides.** The sibling ADRs were made independently and earlier, anchored to workspace principles and the Frame, before this ADR existed. This ADR does **not** impose a hierarchy on them: it records the foundational decision they all assumed, and **cites** each as the authority for the detail it owns — the manifest schema and host-fn import allowlist ([P-0003-plugin-manifest](P-0003-plugin-manifest.md)); the per-instance resource limits and kill-and-replace ([P-0007-plugin-resource-limits](P-0007-plugin-resource-limits.md)); the runtime framework, the MCP SDK, and the exact Wasmtime pin ([P-0012-plugin-runtime-and-mcp-sdk](P-0012-plugin-runtime-and-mcp-sdk.md)); the typed export/invocation ABI ([P-0013-plugin-invocation-model](P-0013-plugin-invocation-model.md)); the V0 signing chain ([P-0005-v0-signing-chain](P-0005-v0-signing-chain.md)). `supersedes`, `superseded_by`, and `overrides` all stay `null`. See the **Reconciliation with prior ADRs** section for the explicit division of authority.

**A note on numbering.** This ADR carries the highest number in the plugin-contract cluster (P-0019) yet records the *earliest* and most foundational decision (2026-05-04). ADR numbers reflect recording order, not decision-date order — the same accepted-on-landing pattern P-0012/P-0013 already use. A reader who wants the plugin contract should start here and follow the citations down into the sibling ADRs for detail.

## Context and Problem Statement

Mnemra-core hosts plugins **in-process** via Wasmtime. Two plugin-model families were on the table (the plugin-runtime survey framing):

- **Path A — stdio-MCP-wrappers / reusable substrate (Extism).** Model plugins as external processes wrapped over stdio MCP, or host them via the **Extism** framework (itself Wasmtime-backed), inheriting Extism's multi-language PDK ecosystem and a working host-fn substrate to lift from (as `hyper-mcp` does).
- **Path B — WebAssembly Component Model + WIT.** Host plugins as Component Model components loaded in-process by Wasmtime, communicating with the host via **WIT-defined** host functions, with typed values crossing the boundary natively.

The maintainer locked **Path B** (2026-05-04). The Frame records the plugin model as a Hard constraint: *"WebAssembly Component Model modules hosted in-process via Wasmtime, with IO-free plugin core logic; all plugin IO MUST be mediated by host-provided functions; plugins are leaves with no sideways linkage; cross-plugin calls are host-mediated"* ([Frame](../intent/mnemra-core-frame.md), §Correction 1). The gating condition for locking the ADR — that the exact shape mnemra wants (long-running stateful in-process CM plugins) actually instantiates on the current toolchain — was **satisfied by the spike**: commit `c9018e3` scaffolds a Cargo workspace with a host (`cmd/mnemra`) and a plugin (`plugins/mnemra-echo/`), compiles the plugin to a Component Model component (`wasm-tools component wit` verified), and passes all 7 acceptance criteria (instantiate-once, `run("hello") -> counter:1`, `run("world") -> counter:2`). The gate is met; this ADR records the evidence.

**What is already decided elsewhere, and what remains.** Since 2026-05-04 the substrate ADRs filled most of the plugin contract's *interior*:

- The **manifest schema** and the typed **host-fn import** ABI (the `content.emit` surface a plugin *calls*) — [P-0003](P-0003-plugin-manifest.md).
- The **per-instance resource limits** (fuel + epoch + memory ceiling) and the **kill-and-replace** invariant — [P-0007](P-0007-plugin-resource-limits.md).
- The **runtime framework** (raw Wasmtime, not Extism), the **MCP SDK** (`rmcp`), and the **exact Wasmtime pin** (`=45.0.2`) — [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md).
- The typed **export/invocation** ABI (the fixed `content` interface a plugin *exports*) — [P-0013](P-0013-plugin-invocation-model.md).

What *no* ADR records is the foundational decision and the cross-cutting contract: **Path B itself**; the **WIT ABI shape invariants** that hold across the whole boundary; the **Component Model async-layer** decision; the **Wasmtime version floor** rationale; the **guest toolchain** choice; the explicit **closing of the Extism-migration door**; the composite **plugin lifecycle sequence**; and the **language-reach** commitments (the V0.1+ language matrix and the Path-B retreat trigger). This ADR locks those, and cites the siblings for everything they already own.

**Bounded input.** The decision draws on the researcher's Component-Model follow-up survey (2026-05-04) — a bounded investigation of WIT patterns, async layering, multi-language tooling, distribution/signing, and production precedents. The survey's concrete "ADR-8 implications" are the direct substrate for the locks below. Where the survey proposed an *illustrative* WIT shape, the implemented shape (`wit/host.wit`, `wit/echo.wit`) and the locked spec requirements are authoritative; the survey is input, not the departure.

## Decision Drivers

- **`P-StackDiscipline`.** Wasmtime, the Component Model, WIT, and `wit-bindgen` are the project's native runtime and toolchain — an in-stack primitive, not an added ecosystem. The async host bridge, the resource knobs, and the typed ABI are all how the runtime the project already targets expresses its contracts. The retreat trigger (below) is where this driver is tested: if the in-stack path cannot cover a hard multi-language requirement, the driver's "unless no in-stack path exists" clause fires.
- **`P-LockContract`.** The plugin contract is what every plugin author writes against and what the host guarantees; locking its shape (WIT interfaces, sync-both-directions, stability gates) while letting the implementation vary (host-fn bodies, pool machinery, the exact runtime version) is the direct application. The contract is the product surface third-party authors will consume — the `P-PerRepoFirst` "published-product" carve-out applies: the contract is shared-from-definition because it *is* the thing shipped.
- **`P-Defer`.** The unstable pieces (WIT-async-types, third-party distribution signing, the wider language matrix) are deferred behind **named tripwires**, not silently omitted. Each deferral below states what it would decide, the canon that supports deferring now, and the mechanical condition that fires it.
- **`P-PreserveDecisionSpace`.** Path A is recorded as the option Path B was chosen against, with the retreat trigger keeping it a live fallback rather than an erased alternative. The maintainer's stated values (strict typing; aversion to non-deterministic string dispatch) are recorded, not paraphrased away.
- **`P-SecurityLayered`.** The sandbox is a trust boundary: IO-free plugin core, host-mediated IO, signature-verified fail-closed load, a per-instance host-fn allowlist, and DoS-containment limits are the layered controls the ABI shape and lifecycle must preserve. The verification-before-execution ordering in the lifecycle is a structural (not advisory) control.
- **Frame Hard constraints and quality-attribute scenarios.** The Frame commits to plugin-sandbox outcomes — *"an infinite-loop plugin is killed and replaced from the pool; no single-process-wide DoS via plugin"* (the DoS-containment QA scenario) and *"a plugin calling an `@unstable` host function emits a deprecation warning"* / *"an ABI-change PR causes all `core: true` plugins to recompile and pass tests"* (the pre-1.0 ABI-evolution QA scenario). The ABI shape and the lifecycle are locked to satisfy these.

## Considered Options

The primary decision is the plugin-model family; four sub-decisions are the cross-cutting contract shape within Path B. Each is recorded with the alternative it was chosen against (`P-PreserveDecisionSpace`); the rejected alternatives are historical, not live options.

**Primary — plugin model family:**

1. **Path B — Component Model + WIT (chosen).** In-process CM components, typed WIT host-fn boundary, native typed values, `wkg`/OCI as the eventual distribution path.
2. **Path A — stdio-MCP-wrappers / Extism substrate (rejected).** External-process stdio wrappers, or Extism-on-Wasmtime, inheriting a ~10-language PDK and a lift-from-`hyper-mcp` substrate reference.

**Sub-decision — transition posture (given Path B):**

3. **CM directly at V0 (chosen).** Build on `wasmtime::component` from primitives; no Extism transitional substrate.
4. **Extism-first, migrate to CM later (rejected).** Ship Extism core modules at V0, re-platform to CM at V0.2+.

**Sub-decision — Component Model async layer:**

5. **Layer 1 only: executor-side async host, sync WIT (chosen).** Host runs an async executor and registers `async` host-fn bodies via the Wasmtime `async` feature; all WIT functions (imports and exports) are **sync**; the guest never sees async.
6. **Layer 2: WIT-level async types (rejected for V0).** Declare `future<T>` / `stream<T>` / `error-context` in WIT under the `component-model-async` feature.

**Sub-decision — guest toolchain:**

7. **`wasm32-wasip2` target directly, via `wit-bindgen` (chosen).** Upstream Rust Tier-2 target; `cargo build --target wasm32-wasip2`.
8. **`cargo-component` (rejected).** The older component-authoring wrapper.

**Sub-decision — Extism migration door:**

9. **Close it explicitly (chosen).** No hybrid host, no preserved Extism-compatible ABI path; the door is stated shut.
10. **Leave a hybrid/transitional door open (rejected).** A dual host-fn ABI (Extism-style for core modules, WIT for components) maintained in parallel.

## Decision Outcome

Chosen: **Path B (Component Model + WIT), built directly on CM, with a sync-WIT / Layer-1-async ABI shape, authored via `wasm32-wasip2`, and the Extism door closed.** Six locked decisions and four deferrals follow. Each lock carries its canon anchor at the decision-and-rationale line; acceptance criteria are binary-observable; each deferral carries decision-content + anchor + firing tripwire.

### D1 — Path B: Component Model + WIT, built directly on CM

**Decision:** The plugin model is WebAssembly Component Model components loaded in-process by Wasmtime, communicating with the host via WIT-defined host functions; typed values cross the boundary natively; no Extism transitional substrate is used at V0. *Anchors: Frame Hard constraint (plugin model); `P-StackDiscipline` (CM+WIT is the in-stack primitive); `P-LockContract` (the WIT boundary is the locked contract); the workspace typed-binary-encoding standard, which names CM as the "exceeds-compliance" boundary (no encoding step at all on the host↔plugin boundary).* The transitional Extism-first posture is rejected because it would require maintaining two host-fn ABIs (Extism-allocator/JSON and WIT-typed) and would force an ecosystem-wide plugin re-platform at V0.2+; mnemra has zero legacy plugins, so there is nothing a hybrid would preserve (survey §6).

**Acceptance criteria:**
- [ ] A `core: true` plugin compiles to a Component Model **component** (not a bare core module), verifiable via `wasm-tools component wit <plugin>.wasm` (the spike's verification, commit `c9018e3`).
- [ ] The plugin guest crate declares no `std::fs`, no `std::process`, and no stdin/stdout; every IO category it uses is a host-fn import declared in its manifest ([P-0003](P-0003-plugin-manifest.md)).
- [ ] No `extism` dependency appears in `libs/mnemra-host/Cargo.toml` (verifiable by grep; confirmed absent, [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) Decision A).

### D2 — WIT ABI shape invariants

**Decision:** The host↔guest WIT boundary holds the following shape invariants across every interface, current and future. *Anchors: `P-LockContract` (the ABI shape is the locked contract); `P-AgentPrimarySource` (a statically-typed, tool-checkable contract is the agent-primary form the maintainer values); the Frame ABI-evolution QA scenario (`@since`/`@unstable`/`@deprecated` as the discipline mechanism); `P-SecurityLayered` (host-derived context and the manifest allowlist as structural controls).* The concrete signatures live in `wit/host.wit` (imports) and `wit/echo.wit` (the `content` export world) and are pinned by the locked spec (import ABI `R-0012-*`; export/invocation ABI `R-0019-*`); [P-0003](P-0003-plugin-manifest.md) owns the import-side detail and [P-0013](P-0013-plugin-invocation-model.md) owns the export-side detail. This ADR locks the **invariants that hold across the whole boundary**, not the per-function signatures (which its siblings and the WIT files own — restating them here would duplicate a single-source fact and drift against it).

The invariants:

1. **Sync WIT in both directions.** Every WIT function — host-fn imports and content exports alike — is a synchronous `func(...)`. There is no WIT-level `async`, no `future`/`stream` return, on the V0 surface. Verifiable: no function in `wit/host.wit` or `wit/echo.wit` carries an async marker.
2. **Component Model Layer 1 only** (see D3): async is an *executor-side host* mechanism, invisible in the WIT contract. The guest writes sync functions; the host bridges IO-bearing host-fn bodies on its async executor.
3. **Native WIT structure; opaque document payloads as `json = string`.** The ABI *structure* — function signatures, the host-derived `workspace-ctx` record, ids, type discriminators, paging records — crosses as native WIT types with no marshalling envelope. Opaque artifact document content (frontmatter, body) crosses as `type json = string` (a UTF-8 string alias), deliberately **not** `list<u8>` (spec `R-0012-f`). So the boundary is marshalling-free for its typed structure and string-payloaded for opaque documents; it is not "no serialization anywhere."
4. **Per-concern interface granularity.** Host functions are partitioned into concept-focused interfaces (`artifact`, `metrics`, `log`, `event`, `projection`, `sampling`, `secrets`, plus shared `types`), composed into the plugin `world` as imports — not one mega-interface. A plugin that does not need an interface does not import it.
5. **`workspace-ctx` is host-derived and never a bare write-path parameter.** Every host-fn carries `ctx: workspace-ctx` (a host-constructed record) as its first parameter; `workspace-id` never appears as a standalone plugin-supplied parameter on any write path (spec `R-0006-a` / `R-0012-d`; [P-0006](P-0006-v0-tenant-enforcement.md)). Export-side content methods take **no** ctx — the host threads workspace context across the export boundary itself.
6. **Stability gates are the ABI-evolution discipline.** Every WIT item carries a stability annotation: `@since(version = x.y.z)` for stable surface, `@unstable(feature = ...)` for surface that may change (e.g., `sampling-request` is `@unstable(feature = sampling-v0)`), `@deprecated(version = ...)` before removal. This is a first-class WIT/toolchain mechanism, not hand-rolled schema policing, and it is how the pre-1.0 ABI-evolution QA scenario is satisfied.

**Acceptance criteria:**
- [ ] Every function in `wit/host.wit` and `wit/echo.wit` is sync (no async marker present).
- [ ] `type json = string` is declared (not `list<u8>`); a `list<u8>` document payload fails the contract (spec `R-0012-f`).
- [ ] Every host-fn import takes `ctx: workspace-ctx` as its first parameter; no host-fn takes a standalone `workspace-id` parameter.
- [ ] Every WIT interface item carries a stability annotation (`@since` / `@unstable` / `@deprecated`).
- [ ] Host-fn interfaces are per-concern (≥ 2 named interfaces, not a single interface containing all functions).

### D3 — Wasmtime version floor `>= 44.0.x`, and the Component Model async layer

**Decision:** The plugin substrate requires **Wasmtime `>= 44.0.x`** as its ABI-compatibility floor, running with **executor-side async ON** (the Wasmtime `async` crate feature — "Layer 1") and **WIT-level async types OFF** (the `component-model-async` crate feature stays disabled — "Layer 2 deferred", see DEF-1). *Anchors: `P-StackDiscipline` (the async host bridge is Wasmtime-native, no added ecosystem); `P-Defer` (the unstable WIT-async-types track is deferred behind a tripwire); the survey async-layering finding (Layer 1 GA since 2026, Layer 2 feature-gated and a moving target).*

The floor rationale — why `>= 44`: it is the point at which the three properties the contract needs are jointly stable — Component Model support with semver-aware import resolution, executor-side async via `Linker::instantiate_async` / `func_wrap_async` behind the `async` feature, and the `wasm32-wasip2` guest target (D4). Below 44 the async-host story is not jointly stable with CM at the versions the survey verified.

The floor is the **contract-level** commitment; the **exact operational pin** is owned by [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) / [P-0007](P-0007-plugin-resource-limits.md), currently `wasmtime = "=45.0.2"` in `libs/mnemra-host/Cargo.toml`, which **satisfies** this floor. This ADR does not restate or compete with that pin (single-source: the exact version lives in P-0012's domain; this ADR states the floor and defers to P-0012 for the operational value).

**On the `async` feature and the current tree — a forward contract, not a present-tense claim.** This decision locks that the async host feature *will be enabled* when the async host-fn invocation path lands. The current `libs/mnemra-host/Cargo.toml` declares `features = ["cranelift", "component-model"]` and **not yet** `async`, because the component-host invocation path (async host-fn bodies, `instantiate_async`) is not yet built — the host-fn bodies are `todo!()` stubs and the pool still instantiates core modules, per [P-0013](P-0013-plugin-invocation-model.md)'s "Bucket A" analysis. The `async` feature is added together with that path; this ADR does not assert it is enabled today. (This is a reconciliation obligation against P-0012's recorded feature list — see Reconciliation.)

**Acceptance criteria:**
- [ ] The operational Wasmtime pin (P-0012's `=45.0.2`) is `>= 44.0.0`.
- [ ] The `component-model-async` crate feature is NOT enabled at V0 (verifiable in `libs/mnemra-host/Cargo.toml`).
- [ ] When the async host-fn path lands, the `async` crate feature is enabled and host-fn import bodies register as `async` (via `func_wrap_async` or bindgen-generated async imports); no host-fn body blocks the executor thread synchronously on IO.

### D4 — Guest toolchain: `wasm32-wasip2` directly, not `cargo-component`

**Decision:** Rust plugin guests target the upstream **`wasm32-wasip2`** target directly (`cargo build --target wasm32-wasip2`, bindings via `wit-bindgen`), **not** `cargo-component`. *Anchors: `P-StackDiscipline` (the upstream Rust target is the in-stack, first-class path); the survey toolchain finding (`cargo-component` is documented "experimental, not currently stable", last release ~8 months stale, and is being superseded by the upstream `wasm32-wasip2` target).* The workspace `rust-toolchain.toml` declares `targets = ["wasm32-wasip2"]` and `wit-bindgen = "0.57"` is pinned at the workspace root; the spike guest (`plugins/mnemra-echo/`) builds this way.

**Acceptance criteria:**
- [ ] `rust-toolchain.toml` lists `wasm32-wasip2` as a target.
- [ ] The plugin build uses `cargo build --target wasm32-wasip2`; no `cargo-component` invocation appears in the build recipes (`justfile`).
- [ ] Guest bindings are generated by `wit-bindgen` (pinned `0.57`), not by `cargo-component`'s bindings path.

### D5 — The Extism-migration door is closed

**Decision:** There is **no** hybrid host and **no** preserved Extism-compatible migration path. mnemra does not run a dual host-fn ABI (Extism-allocator/JSON for core modules alongside WIT-typed for components), and does not keep an Extism-shaped fallback "in reserve." *Anchors: `P-StackDiscipline` (one substrate, not two ABIs maintained forever); [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) Decision A (raw Wasmtime chosen over Extism at the runtime layer); the survey §6 finding (a Wasmtime Engine *can* load both core modules and CM components, so a hybrid is mechanically possible, but for mnemra it means two host-fn ABIs in parallel forever with zero legacy to justify it).* [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) closed the door at the *runtime-framework* layer (Extism the framework is not a dependency); this ADR closes it at the *ABI-shape* layer: the WIT ABI is not an Extism-PDK-compatible ABI, and no code path preserves one.

This close is **conditional on the retreat trigger (DEF-4) not firing.** If a hard multi-language requirement forces a Path-B reconsideration, re-opening a hybrid Extism-PDK path alongside CM is exactly what that revisit would weigh — so the door is closed for V0 but its re-opening condition is named, not erased (`P-PreserveDecisionSpace`).

**Acceptance criteria:**
- [ ] No `extism` dependency in any workspace `Cargo.toml`.
- [ ] The host exposes exactly one host-fn ABI (the WIT-typed one); there is no second Extism-style registration path.

### D6 — Plugin lifecycle sequence

**Decision:** A `core: true` plugin traverses a fixed lifecycle whose **cross-piece ordering invariants** are locked here; the per-transition mechanics are owned by the sibling ADRs cited at each step. *Anchors: `P-SecurityLayered` (verification precedes execution as a structural control); `P-LockContract` (the lifecycle sequence is part of the plugin contract); the Frame DoS-containment QA scenario (trap → kill → replace).* The ordering invariant is the load-bearing lock: **no later state is reachable without its predecessor**, and verification gates are fail-closed.

The sequence (state → the gate that must pass to advance → the owning ADR for the mechanics):

1. **Discovered** — the component `.wasm` and its signed manifest are located (single `core: true` plugin at V0).
2. **Signature-verified** → the manifest signature chains to the mnemra root key; **synchronous, fail-closed** — no instance exists until `verify()` returns `Ok`; `core` status is honored only by signature provenance. *Owner: [P-0005](P-0005-v0-signing-chain.md) (+ its 2026-05-24 amendment).*
3. **Content-hash-verified** → the component bytes match the manifest's signed `[component].hash` (BLAKE3), recomputed on every load, fail-closed on mismatch or absence. *Owner: [P-0003](P-0003-plugin-manifest.md) §Amendment 2026-06-30.*
4. **Manifest-parsed + allowlist-compiled** → the manifest's declared host-fn surface is compiled into a per-instance allowlist; calls outside it fail at the WIT boundary. *Owner: [P-0003](P-0003-plugin-manifest.md).*
5. **Instantiated into the pool** → the component is instantiated (Component Model, not core module) with its per-instance resource limits attached (fuel, epoch deadline, memory ceiling) into a 3–5-instance host-managed pool. *Owner: [P-0007](P-0007-plugin-resource-limits.md).*
6. **Ready** — the instance is pooled and awaiting invocation; state persists across invocations on the same instance (long-running stateful in-process model).
7. **Invoking** → per authenticated MCP verb, the host runs `DF-auth-check` and the manifest per-verb capability check **before dispatch**, then invokes the **exact typed `content` export** for that verb (static verb→export resolution; no string dispatch), under a per-invocation fuel/epoch budget. *Owners: [P-0013](P-0013-plugin-invocation-model.md) (export invocation), [P-0007](P-0007-plugin-resource-limits.md) (per-invocation budget), the MCP front (spec `R-0010-c/d`).*
8. **Trapped → killed → replaced** → on a resource-limit trap (fuel/epoch/memory) the store traps, the host logs a structured attribution event, poisons the pool slot, and creates a fresh instance; the trap is **never** propagated as a host-process panic. *Owner: [P-0007](P-0007-plugin-resource-limits.md).*
9. **Shutdown** — pool instances are dropped at host shutdown.

**Acceptance criteria:**
- [ ] Signature verification (state 2) and content-hash verification (state 3) both complete `Ok` before any instance is created (state 5); a failure at 2 or 3 yields a load rejection, not a loaded-but-unverified instance.
- [ ] A manifest-declared verb with no matching typed export is non-dispatchable and returns a structured error (not a panic, not a string-dispatch fallback) — [P-0013](P-0013-plugin-invocation-model.md).
- [ ] A resource-limit trap (state 8) results in a replaced pool slot and an error for the current invocation, with the host process surviving (no panic) — [P-0007](P-0007-plugin-resource-limits.md).

### Deferrals

Each deferral states what it would decide, the canon that supports deferring now, and the **named tripwire** that fires it.

#### DEF-1 — WIT-level async types (Layer 2 / `component-model-async`)

**Deferred:** WIT-level async return types (`future<T>`, `stream<T>`, `error-context`) and the concurrent-task scheduling primitives, which would let a host-fn return a stream (e.g., a streaming `read`/query verb) instead of a materialized result. *Deferral anchor: `P-Defer` — the `component-model-async` crate feature is feature-gated and a moving target in Wasmtime 44/45; adopting it now is opting into an unstable ABI surface.* **Tripwire:** a required host-fn or content verb whose contract genuinely needs a streaming/async-typed return **and** the runtime has stabilized `component-model-async` (both conditions). The WIT corpus is designed so such a verb can be added later under a new `@since` version without reshaping the sync surface; a preference for a cleaner async story is not a firing condition.

#### DEF-2 — Third-party plugin distribution + signature verification (cosign-on-`wkg`)

**Deferred:** The distribution-and-verification story for **third-party** (non-`core`) plugins — publishing components to an OCI/`wkg` registry and verifying a per-artifact signature (cosign/sigstore-style, a `application/vnd.mnemra.plugin.v1` artifact type) before instantiation. *Deferral anchor: `P-Defer` — V0 ships only `core: true` plugins embedded in the binary; there is nothing to distribute, so the distribution-verification mechanism has no V0 forcing function.* This is distinct from and does **not** weaken the V0 signing chain: [P-0005](P-0005-v0-signing-chain.md) already locks synchronous fail-closed ed25519 verification of `core: true` plugins at load. The survey §4 finding is that `wkg` (the BA-canonical CM distribution path) does **not** bake in cosign the way `hyper-mcp` does — so this is real work mnemra builds at V0.1+, not a substrate freebie. **Tripwire:** the first third-party (non-`core`) plugin distribution surface opening — i.e., V0.1+ third-party plugin install. The recommended shape when it fires (a sigstore-compatible verifier over `wkg`'s OCI fetch, applied before instantiation) is recorded as the survey's recommendation, not locked here.

#### DEF-3 — V0.1+ language-support matrix

**Deferred:** The supported plugin-authoring language matrix beyond Rust. V0 is **Rust-only** (Rust-first via WIT, per the Frame V0/V1 boundary). The V0.1+ matrix, when third-party authoring opens, is tiered: **Tier 1** — Rust, JS/TS (via `jco`); **Tier 2** — Python (`componentize-py`, build-time-imports caveat), TinyGo (standard Go has documented WIT/GC issues — TinyGo only), C# (Spin-parity gaps); **Tier 3** — C/C++, MoonBit. Languages outside the matrix have no supported on-ramp. *Deferral anchor: `P-Defer` — the matrix is a V0.1+ product-positioning commitment with no V0 forcing function (V0 has no third-party authors); publishing it before authoring opens would lock a maturity claim about tooling that is still moving.* **Tripwire:** V0.1+ third-party plugin authoring opening; the matrix is published in the plugin-author docs at that point, tiered by the then-current tooling maturity. This is an honest narrowing of "plugins in any language" — a planned consequence of Path B, not a stumble.

#### DEF-4 — Path-B retreat trigger

**Deferred (as a reversal condition, not a decision):** Whether to reconsider Path B in favor of a hybrid that reintroduces an Extism-PDK authoring path alongside CM. *Anchor: `P-PreserveDecisionSpace` (Path A stays a documented, reachable fallback) + `P-StackDiscipline`'s "unless no in-stack path exists" clause (the retreat is where that clause would fire).* The Bytecode-Alliance CM language list omits four languages Extism's PDK supports with no CM authoring path in 2026: **F#, Haskell, AssemblyScript, Zig** (Haskell is the genuinely load-bearing loss; the other three are small/long-tail per survey §3). mnemra's actual Tier-1 audience (Rust, JS/TS, Python, Go) is well-covered on CM, so the gap is tolerable for V0. **Tripwire:** a Tier-2 enterprise customer surfaces F#/Haskell/AssemblyScript/Zig plugin authoring as a **hard requirement**. If that fires, this ADR is revisited and a hybrid Extism-PDK-alongside-CM path is weighed against the cost of a dual host-fn ABI (D5). Absent a hard requirement, Path B stands and the Extism door (D5) stays closed; a preference for broader language reach is not a firing condition.

### Consequences

**Good:**

- The foundational plugin-model decision finally has an ADR home; a reader starting here can navigate the whole plugin contract via citations, and a future reversal (Path B → Path A) has a concrete record to reverse against rather than only Frame narrative.
- The ABI shape invariants (D2) give the maintainer's stated values — strict typing, no non-deterministic string dispatch, tool-checkable contracts — a single durable statement, complementing P-0013's export-side typing lock.
- The async-layer decision (D3) commits to the GA, stable executor-side path and quarantines the moving `component-model-async` target behind a tripwire, so the V0 substrate does not track an unstable ABI feature.
- The Extism door (D5) is shut at both the runtime-framework layer (P-0012) and the ABI-shape layer (here), preventing a dual-ABI-forever drift, while the retreat trigger keeps the reversal honest.
- The lifecycle sequence (D6) locks the security-relevant ordering (verify → hash → allowlist → instantiate → invoke; trap → kill → replace) as a cross-piece invariant, so a future refactor cannot reorder verification after instantiation without visibly violating this ADR.
- The four deferrals give the "plugins in any language" and "signed third-party distribution" stories named V0.1+ landing conditions instead of implicit expectations.

**Bad / Trade-offs:**

- **No production substrate exemplar.** Path B is build-on-Wasmtime-from-primitives; no production user runs mnemra's exact shape (long-running stateful in-process CM plugins) on CM in 2026 (survey §5). The runtime is mature; the *plugin-runtime wiring* (pool, lifecycle, host-fn registration) is mnemra's to write, with documentation + WIT spec as reference rather than working code to lift. Accepted as pioneering budget spent on the substrate, consistent with the project's clean-design-over-schedule weighting.
- **Language reach narrows** from ~10 (Extism PDK) to ~8 (BA-CM-blessed), losing F#/Haskell/AssemblyScript/Zig authoring paths (DEF-4). Mitigated by the Tier-1 audience coverage and the retreat trigger; unmitigated for a Haskell-shop plugin author until (if) the trigger fires.
- **Async ceiling.** Sync-WIT / Layer-1 async means the same one-in-flight-call-per-instance concurrency ceiling as Path A, solved by the instance pool; the cleaner WIT-async story is deferred (DEF-1).
- **This ADR carries no field-level ABI or manifest detail of its own** — it is deliberately a cross-cutting-invariants + citations document. A reader who wants a specific host-fn signature or manifest field must follow the citation to `wit/host.wit` / [P-0003](P-0003-plugin-manifest.md) / [P-0013](P-0013-plugin-invocation-model.md). This is the intended division of authority (avoids duplicating single-source facts), but it means P-0019 is not self-sufficient for signature-level questions.

## Pros and Cons of the Options

### Path B — Component Model + WIT (chosen)

- Pro: No marshalling on the typed ABI structure — native WIT values cross the boundary; the cleanest possible answer to the workspace typed-binary-encoding standard for the in-process boundary.
- Pro: WIT stability gates (`@since`/`@unstable`/`@deprecated`) are toolchain-enforced ABI-evolution discipline; Path A would police the same discipline in hand-rolled JSON-schema code.
- Pro: Typed `resource`/record contracts and static verb→export resolution give tool-checkable reason-about-ability the maintainer values; single substrate evolution path (no dual ABI).
- Con: No production exemplar at mnemra's shape; pioneering wiring cost (survey §5). Language reach narrows vs Extism's PDK.

### Path A — stdio-MCP-wrappers / Extism substrate (rejected)

- Pro: Reusable substrate (lift from `hyper-mcp`); ~10-language PDK inherited "for free"; V0 ships sooner.
- Con: Extism-first-then-CM is a re-platform, not a migration — every plugin's host-fn imports change shape at V0.2+; a dual ABI is the alternative, maintained forever. Marshalling (JSON/allocator) at the boundary; weaker typing than native WIT. Starts in violation of the workspace typed-binary-encoding standard for the host↔plugin boundary. The reusable-substrate benefit is largest exactly at V0 — once CM is wired directly, the migration cost is work already paid.

### Layer-1-only async, sync WIT (chosen) vs Layer-2 WIT async (rejected for V0)

- Pro (Layer 1): `Linker::instantiate_async` / `func_wrap_async` are GA behind the stable `async` feature; the host bridges IO-bearing host-fns on its executor while the guest writes plain sync functions; the same pattern Spin/wasmCloud run in production.
- Con (Layer 2): `future`/`stream`/`error-context` are gated behind `component-model-async`, feature-gated and not committed stable in 2026 — building host-fns returning `stream<T>` now tracks a moving target. Deferred with a tripwire (DEF-1).

### `wasm32-wasip2` directly (chosen) vs `cargo-component` (rejected)

- Pro (`wasm32-wasip2`): upstream Rust Tier-2 target, first-class, `cargo build` with no extra wrapper; the direction the ecosystem is consolidating toward.
- Con (`cargo-component`): documented "experimental, not currently stable"; ~8-month-stale release cadence; explicitly being superseded by the upstream target (survey §6). Warranted only for richer non-WASI WIT scenarios V0 does not have.

## Reconciliation with prior ADRs

This ADR is a **capstone that cites**. It records the foundational Path-B decision the sibling ADRs already assumed, and defers to each for the detail it owns. It does **not** supersede or override any of them; `supersedes`/`superseded_by`/`overrides` stay `null`. The division of authority:

| Concern | Authority | This ADR's role |
|---|---|---|
| Plugin model = CM + WIT (Path B) | **P-0019 (here)** + Frame Hard constraint | Records the foundational decision; no prior ADR owned it |
| Manifest schema + host-fn **import** allowlist + component content-hash | [P-0003](P-0003-plugin-manifest.md) | Cites for field-level detail; D2/D6 reference, do not restate |
| Per-instance resource limits + kill-and-replace | [P-0007](P-0007-plugin-resource-limits.md) | Cites for limit values and trap mechanics (D6 states only the ordering) |
| Runtime framework (raw Wasmtime) + MCP SDK (`rmcp`) + **exact** Wasmtime pin | [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) | Cites for the `=45.0.2` pin (D3 states only the `>= 44` floor); D5 extends P-0012's Extism close to the ABI-shape layer |
| Typed **export/invocation** ABI | [P-0013](P-0013-plugin-invocation-model.md) | Cites for the fixed `content` interface + static verb→export resolution (D2/D6 reference) |
| V0 signing chain (ed25519, fail-closed load) | [P-0005](P-0005-v0-signing-chain.md) | Cites in the lifecycle (D6 state 2); DEF-2 explicitly does not weaken it |
| Host-derived `WorkspaceCtx` | [P-0006](P-0006-v0-tenant-enforcement.md) | Cites for the write-path exclusion invariant (D2.5) |

**Two reconciliation obligations recorded (not silently resolved):**

1. **The `async` crate feature vs P-0012's recorded feature list.** D3 locks "async feature ON at V0" as a **forward contract** that enables when the async host-fn path lands; P-0012's References record the *current* `libs/mnemra-host/Cargo.toml` as `features = ["cranelift", "component-model"]` (no `async`), reflecting the pre-async-bridge scaffold. These are consistent — the tree has not yet reached the async host path (host-fn bodies are `todo!()`, per P-0013's Bucket A) — and this ADR does **not** claim the feature is currently enabled. Whether P-0012's feature list should gain a companion note that `async` is added with the async path is a single-source-reconciliation question for the maintainer (this ADR does not edit P-0012).
2. **The illustrative survey WIT shape vs the implemented shape.** The survey proposed an illustrative WIT surface (`list<u8>` bodies, `resource` handles, `result<T,E>` returns). The implemented and locked shape diverged (`type json = string` not `list<u8>`; `workspace-ctx` as a record not a resource, for contract-test resolvability; `option` returns). D2 locks the shape *invariants* and defers to `wit/host.wit` / `wit/echo.wit` + P-0003/P-0013 for the exact signatures; the survey is bounded input, and the implemented WIT is authoritative where they differ.

## More Information

- **Frame relationship.** This ADR does **not** resolve a `{{P-XXX}}` open ADR slot — the Path-B plugin model was a brief Hard-constraint carry-forward, narrated in the Frame ([`docs/src/intent/mnemra-core-frame.md`](../intent/mnemra-core-frame.md), §Correction 1/2 and §"Plugin runtime") but never given its own slot (unlike the manifest schema `{{P-PluginManifest}}` → P-0003 and the resource limits `{{P-PluginPoolMemory}}` → P-0007). It records that narrated Hard constraint plus the cross-cutting ABI-shape/toolchain/lifecycle/language commitments the researcher's survey named as "ADR-8 implications."
- **The gating spike.** Commit `c9018e3` ("scaffold V0.01 + CM existence-test spike", on `main`, 2026-05-07): Cargo workspace with host (`cmd/mnemra`) + plugin (`plugins/mnemra-echo/`), WIT contract, Wasmtime 44 + `wasm32-wasip2`, all 7 acceptance criteria passing. The evidence that Path B instantiates at mnemra's shape.
- **Quality-attribute scenarios anchored.** DoS-containment ("infinite-loop plugin killed and replaced from the pool; no single-process-wide DoS") → D6 states 7–8, owned by [P-0007](P-0007-plugin-resource-limits.md). ABI-evolution ("a plugin calling an `@unstable` host function emits a deprecation warning"; "an ABI-change PR causes all `core: true` plugins to recompile and pass tests") → D2.6 stability gates + D4 `wasm32-wasip2` recompile surface.
- **Threat references** (from the companion [overview](../architecture/overview.md)): `P-plugin-instance`/E (allowlist mitigation, D2/D6-4), `P-plugin-runtime`/D,E,R (kill-and-replace + fail-closed verify, D6), `P-host-fns`/T (workspace-ctx exclusion, D2.5), `DS-mnemra-root-key`/I (signing, D6-2). The ABI shape and lifecycle preserve these structural mitigations.
- **Concrete artifacts (repo-relative, load-bearing):** `wit/host.wit` (host-fn imports + `content` export interface), `wit/echo.wit` (the plugin `world`), `libs/mnemra-host/Cargo.toml` (Wasmtime pin + features), `rust-toolchain.toml` (`wasm32-wasip2` target), `Cargo.toml` (`wit-bindgen = "0.57"`), `plugins/mnemra-echo/` (the spike fixture), `docs/specs/2026-05-24-mnemra-core-v0-substrate.md` (import ABI `R-0012-*`, MCP front `R-0010-*`, tenant ctx `R-0006-*`; export/invocation ABI `R-0019-*`, paging `R-0020`).
- **Bounded input:** the researcher's Component-Model follow-up survey (2026-05-04) — WIT patterns, the two-layer async story, the multi-language matrix, distribution/signing on CM, and production precedents; the direct substrate for D1–D6 and the deferrals.
- **Canon anchors:** `P-StackDiscipline` (CM+WIT + async bridge + `wasm32-wasip2` are in-stack; the retreat clause), `P-LockContract` (the WIT boundary is the product contract), `P-Defer` (four tripwired deferrals), `P-PreserveDecisionSpace` (Path A recorded as the chosen-against option; the maintainer's typing values verbatim), `P-SecurityLayered` (verify-before-execute ordering; host-derived ctx; allowlist), `P-AgentPrimarySource` (tool-checkable typed contract).
- **Follow-ups:**
  - When the async host-fn invocation path lands (P-0013 Bucket A), enable the Wasmtime `async` feature and register host-fn bodies async (D3 AC); confirm whether P-0012's feature-list record wants a companion note (Reconciliation obligation 1 — maintainer's call).
  - At V0.1+ third-party plugin authoring: publish the DEF-3 language matrix in the plugin-author docs, tiered by then-current tooling maturity, and build the DEF-2 distribution-signature verifier over `wkg`/OCI.
- **Related precedent:** [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) and [P-0013](P-0013-plugin-invocation-model.md) established the accepted-on-landing / forward-contract framing this ADR follows for a foundational decision recorded after its dependent detail.
