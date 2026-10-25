---
title: "P-0019: Plugin Contract — Component Model + WIT ABI, Toolchain, and Lifecycle"
summary: "The foundational plugin-contract decision the substrate ADRs already assumed but none recorded: Path B (WebAssembly Component Model + WIT), locked end of session 4 (2026-05-04) and proven by the V0.01 spike (commit c9018e3). Locks the cross-cutting contract no single sibling owns — the WIT ABI shape (sync WIT both directions; Component Model Layer 1 only: async host feature ON, component-model-async / WIT-async-types OFF; per-concern interface granularity; @since/@unstable/@deprecated stability gates; native WIT structure with opaque JSON-as-string document payloads; host-derived workspace-ctx never a bare write-path parameter), the Wasmtime version floor (>=44.0.x, rationale) (Amendment 2026-10-25, #4249: the floor is restated as the three Wasmtime capabilities it stood for, and no record owns an exact Wasmtime or wit-bindgen version), the guest toolchain (wasm32-wasip2 target directly via wit-bindgen, NOT cargo-component), the explicit closing of the Extism-migration door, and the composite plugin lifecycle sequence. Records four deferrals with named tripwires: WIT-async-types, cosign-on-wkg third-party distribution verification, the V0.1+ language-support matrix, and the Path-B retreat trigger. This is a capstone that CITES its siblings for the detail each owns (manifest schema -> P-0003; resource limits + kill-and-replace -> P-0007; runtime framework + rmcp SDK + exact Wasmtime pin -> P-0012; typed export/invocation ABI -> P-0013); it does not supersede or override any of them."
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

This [ADR](../glossary.md#adr) (architecture decision record) records the **foundational plugin-contract decision** for the V0 substrate. The plugin model is **WebAssembly Component Model + WIT** (Path B). The maintainer locked it at the end of session 4 (**2026-05-04**), and the V0.01 existence-test spike validated it (commit `c9018e3`, on `main`, 2026-05-07). It's recorded as `accepted` on landing, in the same way as [P-0011-logging-facade](P-0011-logging-facade.md) and [P-0012-plugin-runtime-and-mcp-sdk](P-0012-plugin-runtime-and-mcp-sdk.md): the decision came first and the tree already reflects it. This ADR ratifies the decision. It doesn't reopen it.

**Why this ADR exists, and why late.** Path B came forward from the product brief as a Hard constraint. The [Frame](../glossary.md#frame) (the architecture-stage document, [`docs/src/intent/mnemra-core-frame.md`](../intent/mnemra-core-frame.md), §"Correction 2" and §"Plugin runtime") *narrated* it: "Plugins are WebAssembly Component Model modules loaded in-process by Wasmtime, communicating with the host via WIT-defined host functions". But the manifest schema and the resource limits each got their own `{{P-XXX}}` open ADR slot, and Path B never did. The substrate ADRs written since then (P-0003, P-0007, P-0012, P-0013) all **presuppose** Path B, and each locks one *piece* of the plugin contract. Nobody gave a home to the foundational plugin-model decision, or to the commitments that cut across all of them: ABI shape, toolchain, lifecycle and language reach. This ADR is that home.

**A capstone that cites, not a parent that overrides.** The sibling ADRs were decided independently and earlier, anchored to workspace principles and the Frame, before this ADR existed. So this ADR does **not** put them under a hierarchy. It records the foundational decision they all assumed, and **cites** each one as the authority for the detail it owns:

- the manifest schema and the host-fn (host function) import allowlist, in [P-0003-plugin-manifest](P-0003-plugin-manifest.md);
- the per-instance resource limits and kill-and-replace, in [P-0007-plugin-resource-limits](P-0007-plugin-resource-limits.md);
- the runtime framework, the MCP SDK and the exact Wasmtime pin, in [P-0012-plugin-runtime-and-mcp-sdk](P-0012-plugin-runtime-and-mcp-sdk.md);
- the typed export/invocation ABI, in [P-0013-plugin-invocation-model](P-0013-plugin-invocation-model.md);
- the V0 signing chain, in [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md).

`supersedes`, `superseded_by` and `overrides` all stay `null`. The **Reconciliation with prior ADRs** section spells out the division of authority.

**A note on numbering.** This ADR has the highest number in the plugin-contract cluster (P-0019), yet it records the *earliest* and most foundational decision (2026-05-04). ADR numbers follow recording order, not decision date. P-0012 and P-0013 already use the same accepted-on-landing pattern. A reader who wants the plugin contract should start here and follow the citations down into the sibling ADRs for detail.

## Context and Problem Statement

Mnemra-core hosts plugins **in-process** through Wasmtime. The plugin-runtime survey framed two plugin-model families:

- **Path A, stdio-MCP-wrappers / reusable substrate (Extism).** Plugins run as external processes wrapped over stdio MCP, or the host runs them through the **Extism** framework (which itself runs on Wasmtime). This path inherits Extism's multi-language PDK (plugin development kit) ecosystem and a working host-fn substrate to lift from, as `hyper-mcp` does.
- **Path B, WebAssembly Component Model + WIT.** Plugins are Component Model components that Wasmtime loads in-process. They talk to the host through **WIT-defined** host functions, and typed values cross the boundary natively.

The maintainer locked **Path B** on 2026-05-04. The Frame records the plugin model as a Hard constraint: *"WebAssembly Component Model modules hosted in-process via Wasmtime, with IO-free plugin core logic; all plugin IO MUST be mediated by host-provided functions; plugins are leaves with no sideways linkage; cross-plugin calls are host-mediated"* ([Frame](../intent/mnemra-core-frame.md), §Correction 1). Locking the ADR depended on one condition: the exact shape mnemra wants (long-running, stateful, in-process CM plugins) had to instantiate on the current toolchain. **The spike satisfied it.** Commit `c9018e3` scaffolds a Cargo workspace with a host (`cmd/mnemra`) and a plugin (`plugins/mnemra-echo/`). It compiles the plugin to a Component Model component (checked with `wasm-tools component wit`) and passes all 7 acceptance criteria (instantiate-once, `run("hello") -> counter:1`, `run("world") -> counter:2`). The gate is met, and this ADR records the evidence.

**What is already decided elsewhere, and what remains.** Since 2026-05-04 the substrate ADRs have filled in most of the plugin contract's *interior*:

- The **manifest schema** and the typed **host-fn import** ABI (the `content.emit` surface a plugin *calls*): [P-0003](P-0003-plugin-manifest.md).
- The **per-instance resource limits** (fuel, epoch and memory ceiling) and the **kill-and-replace** invariant: [P-0007](P-0007-plugin-resource-limits.md).
- The **runtime framework** (raw Wasmtime, not Extism), the **MCP SDK** (`rmcp`) and the **exact Wasmtime pin** (`=45.0.2`): [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md).
- The typed **export/invocation** ABI (the fixed `content` interface a plugin *exports*): [P-0013](P-0013-plugin-invocation-model.md).

*No* ADR records the foundational decision or the contract that cuts across all of these. That gap covers:

- **Path B itself**;
- the **WIT ABI shape invariants** that hold across the whole boundary;
- the **Component Model async-layer** decision;
- the rationale for the **Wasmtime version floor**;
- the **guest toolchain** choice;
- the explicit **closing of the Extism-migration door**;
- the composite **plugin lifecycle sequence**;
- the **language-reach** commitments (the V0.1+ language matrix and the Path-B retreat trigger).

This ADR locks those, and cites the siblings for everything they already own.

**Bounded input.** The decision draws on the researcher's Component Model follow-up survey (2026-05-04). That was a bounded investigation of WIT patterns, async layering, multi-language tooling, distribution and signing, and production precedents. The survey's concrete "ADR-8 implications" are the direct basis for the locks below. Where the survey proposed an *illustrative* WIT shape, the implemented shape (`wit/host.wit`, `wit/echo.wit`) and the locked spec requirements are authoritative. The survey is input, not the starting point the design departs from.

## Decision Drivers

- **`P-StackDiscipline`** (stay in the project's native stack unless no in-stack path exists). Wasmtime, the Component Model, WIT and `wit-bindgen` are the project's native runtime and toolchain. They're an in-stack primitive, not an added ecosystem. The async host bridge, the resource knobs and the typed ABI are all the way the runtime the project already targets expresses its contracts. The retreat trigger (below) is where this driver gets tested: if the in-stack path can't cover a hard multi-language requirement, the driver's "unless no in-stack path exists" clause fires.
- **[`P-LockContract`](../glossary.md#p-lockcontract)** (lock the contract; vary the implementation). The plugin contract is what every plugin author writes against and what the host guarantees. Locking its shape (WIT interfaces, sync in both directions, stability gates) while the implementation varies (host-fn bodies, pool machinery, the exact runtime version) is a direct application of the principle. Third-party authors will consume this contract as a product surface, so the "published-product" carve-out in [`P-PerRepoFirst`](../glossary.md#p-perrepofirst) (per-repo first; extract on rule-of-three) applies. The contract is shared from the moment it's defined because it *is* the thing being shipped.
- **[`P-Defer`](../glossary.md#p-defer)** (defer mechanism choice until evidence forces it). The unstable pieces (WIT async types, third-party distribution signing, the wider language matrix) sit behind **named tripwires**. They aren't silently left out. Each deferral below states what it would decide, the canon that supports deferring it now, and the mechanical condition that fires it.
- **[`P-PreserveDecisionSpace`](../glossary.md#p-preservedecisionspace)** (keep rejected options and their reasons on the record). Path A is recorded as the option Path B was chosen against. The retreat trigger keeps it a live fallback instead of an erased alternative. The maintainer's stated values (strict typing; an aversion to non-deterministic string dispatch) are recorded as stated, not paraphrased away.
- **`P-SecurityLayered`** (security controls compose in independent layers). The sandbox is a trust boundary. Its layered controls are an IO-free plugin core, host-mediated IO, a signature-verified load that fails closed, a per-instance host-fn allowlist, and limits that contain denial of service. The ABI shape and the lifecycle must preserve all of them. The lifecycle's ordering of verification before execution is a structural control, not an advisory one.
- **Frame Hard constraints and quality-attribute scenarios.** The Frame commits to two kinds of plugin-sandbox outcome. One is DoS containment: *"an infinite-loop plugin is killed and replaced from the pool; no single-process-wide DoS via plugin"*. The other is pre-1.0 ABI evolution: *"a plugin calling an `@unstable` host function emits a deprecation warning"* and *"an ABI-change PR causes all `core: true` plugins to recompile and pass tests"*. The ABI shape and the lifecycle are locked to satisfy these scenarios.

## Considered Options

The primary decision is the plugin-model family. Four sub-decisions then set the cross-cutting contract shape inside Path B. Each one is recorded with the alternative it was chosen against (`P-PreserveDecisionSpace`). The rejected alternatives are history, not live options.

**Primary: plugin model family.**

1. **Path B, Component Model + WIT (chosen).** In-process CM components, a typed WIT host-fn boundary, native typed values, and `wkg`/OCI as the eventual distribution path.
2. **Path A, stdio-MCP-wrappers / Extism substrate (rejected).** External-process stdio wrappers, or Extism on Wasmtime, inheriting a PDK covering about 10 languages and `hyper-mcp` as a substrate reference to lift from.

**Sub-decision: transition posture (given Path B).**

3. **CM directly at V0 (chosen).** Build on `wasmtime::component` from primitives, with no Extism transitional substrate.
4. **Extism-first, migrate to CM later (rejected).** Ship Extism core modules at V0, then re-platform to CM at V0.2+.

**Sub-decision: Component Model async layer.**

5. **Layer 1 only: executor-side async host, sync WIT (chosen).** The host runs an async executor and registers `async` host-fn bodies through the Wasmtime `async` feature. Every WIT function (imports and exports) is **sync**. The guest never sees async.
6. **Layer 2: WIT-level async types (rejected for V0).** Declare `future<T>` / `stream<T>` / `error-context` in WIT under the `component-model-async` feature.

**Sub-decision: guest toolchain.**

7. **`wasm32-wasip2` target directly, via `wit-bindgen` (chosen).** The upstream Rust Tier-2 target, built with `cargo build --target wasm32-wasip2`.
8. **`cargo-component` (rejected).** The older wrapper for authoring components.

**Sub-decision: Extism migration door.**

9. **Close it explicitly (chosen).** No hybrid host and no preserved Extism-compatible ABI path. The door is stated shut.
10. **Leave a hybrid/transitional door open (rejected).** Maintain two host-fn ABIs in parallel: an Extism-style one for core modules and a WIT one for components.

## Decision Outcome

Chosen: **Path B (Component Model + WIT), built directly on CM, with a sync-WIT / Layer-1-async ABI shape, authored via `wasm32-wasip2`, and the Extism door closed.** Six locked decisions and four deferrals follow. Each lock names its canon anchor on the decision-and-rationale line. Acceptance criteria are binary and observable. Each deferral carries its decision content, an anchor and a tripwire that fires it.

### D1 — Path B: Component Model + WIT, built directly on CM

**Decision:** The plugin model is WebAssembly Component Model components that Wasmtime loads in-process. They talk to the host through WIT-defined host functions, and typed values cross the boundary natively. No Extism transitional substrate is used at V0. *Anchors: Frame Hard constraint (plugin model); `P-StackDiscipline` (CM+WIT is the in-stack primitive); `P-LockContract` (the WIT boundary is the locked contract); the workspace typed-binary-encoding standard, which names CM as the "exceeds-compliance" boundary (no encoding step at all on the host↔plugin boundary).* The Extism-first transitional posture is rejected for two reasons. It would mean maintaining two host-fn ABIs (Extism-allocator/JSON and WIT-typed), and it would force every plugin in the ecosystem to re-platform at V0.2+. Mnemra has zero legacy plugins, so a hybrid would preserve nothing (survey §6).

**Acceptance criteria:**
- [ ] A `core: true` plugin compiles to a Component Model **component** (not a bare core module), verifiable via `wasm-tools component wit <plugin>.wasm` (the spike's verification, commit `c9018e3`).
- [ ] The plugin guest crate declares no `std::fs`, no `std::process`, and no stdin/stdout; every IO category it uses is a host-fn import declared in its manifest ([P-0003](P-0003-plugin-manifest.md)).
- [ ] No `extism` dependency appears in `libs/mnemra-host/Cargo.toml` (verifiable by grep; confirmed absent, [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) Decision A).

### D2 — WIT ABI shape invariants

**Decision:** The host↔guest WIT boundary holds the shape invariants below across every interface, current and future. *Anchors: `P-LockContract` (the ABI shape is the locked contract); `P-AgentPrimarySource` (source artifacts are written for agents first; a statically typed, tool-checkable contract is the agent-primary form the maintainer values); the Frame ABI-evolution QA scenario (`@since`/`@unstable`/`@deprecated` as the discipline mechanism); `P-SecurityLayered` (host-derived context and the manifest allowlist as structural controls).* The concrete signatures live in `wit/host.wit` (imports) and `wit/echo.wit` (the `content` export world). The locked spec pins them, with the import ABI under `R-0012-*` and the export/invocation ABI under `R-0019-*` (these are [R-codes](../glossary.md#r-codes), requirement IDs defined in that spec). [P-0003](P-0003-plugin-manifest.md) owns the import-side detail and [P-0013](P-0013-plugin-invocation-model.md) owns the export-side detail. This ADR locks the **invariants that hold across the whole boundary**, not the per-function signatures. Those belong to the siblings and the WIT files, and restating them here would duplicate a single-source fact and let it drift.

The invariants:

1. **Sync WIT in both directions.** Every WIT function, host-fn imports and content exports alike, is a synchronous `func(...)`. The V0 surface has no WIT-level `async` and no `future`/`stream` return. Verifiable: no function in `wit/host.wit` or `wit/echo.wit` carries an async marker.
2. **Component Model Layer 1 only** (see D3). Async is a mechanism on the *executor side of the host*, and it's invisible in the WIT contract. The guest writes sync functions. The host bridges the IO-bearing host-fn bodies on its async executor.
3. **Native WIT structure; opaque document payloads as `json = string`.** The ABI *structure* crosses as native WIT types with no marshalling envelope. That structure is the function signatures, the host-derived `workspace-ctx` record, ids, type discriminators and paging records. Opaque artifact document content (frontmatter, body) crosses as `type json = string`, a UTF-8 string alias, and deliberately **not** as `list<u8>` (spec `R-0012-f`). So the boundary is marshalling-free for its typed structure and carries string payloads for opaque documents. It isn't "no serialization anywhere."
4. **Per-concern interface granularity.** Host functions are split into interfaces focused on one concept each (`artifact`, `metrics`, `log`, `event`, `projection`, `sampling`, `secrets`, plus a shared `types`). They're composed into the plugin `world` as imports, not merged into one mega-interface. A plugin that doesn't need an interface doesn't import it.
5. **`workspace-ctx` is host-derived and never a bare write-path parameter.** Every host-fn takes `ctx: workspace-ctx` (a record the host constructs) as its first parameter. `workspace-id` never shows up as a standalone, plugin-supplied parameter on any write path (spec `R-0006-a` / `R-0012-d`; [P-0006](P-0006-v0-tenant-enforcement.md)). The export-side content methods take **no** ctx, because the host threads workspace context across the export boundary itself.
6. **Stability gates are the ABI-evolution discipline.** Every WIT item carries a stability annotation. `@since(version = x.y.z)` marks stable surface. `@unstable(feature = ...)` marks surface that may change (for example, `sampling-request` is `@unstable(feature = sampling-v0)`). `@deprecated(version = ...)` comes before removal. This is a first-class WIT and toolchain mechanism, not schema policing written by hand, and it's how the pre-1.0 ABI-evolution QA scenario is met.

**Acceptance criteria:**
- [ ] Every function in `wit/host.wit` and `wit/echo.wit` is sync (no async marker present).
- [ ] `type json = string` is declared (not `list<u8>`); a `list<u8>` document payload fails the contract (spec `R-0012-f`).
- [ ] Every host-fn import takes `ctx: workspace-ctx` as its first parameter; no host-fn takes a standalone `workspace-id` parameter.
- [ ] Every WIT interface item carries a stability annotation (`@since` / `@unstable` / `@deprecated`).
- [ ] Host-fn interfaces are per-concern (≥ 2 named interfaces, not a single interface containing all functions).

### D3 — Wasmtime version floor `>= 44.0.x`, and the Component Model async layer

**Decision:** The plugin substrate requires **Wasmtime `>= 44.0.x`** as its ABI-compatibility floor. It runs with **executor-side async ON** (the Wasmtime `async` crate feature, "Layer 1") and **WIT-level async types OFF** (the `component-model-async` crate feature stays disabled, "Layer 2 deferred"; see DEF-1). *Anchors: `P-StackDiscipline` (the async host bridge is native to Wasmtime, with no added ecosystem); `P-Defer` (the unstable WIT-async-types track waits behind a tripwire); the survey's async-layering finding (Layer 1 GA since 2026, Layer 2 feature-gated and a moving target).*

Why `>= 44`: it's the point where the three properties the contract needs are stable together. Those are Component Model support with semver-aware import resolution; executor-side async through `Linker::instantiate_async` / `func_wrap_async` behind the `async` feature; and the `wasm32-wasip2` guest target (D4). Below 44, at the versions the survey checked, the async-host story isn't stable together with CM.

The floor is the **contract-level** commitment. The **exact operational pin** belongs to [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) / [P-0007](P-0007-plugin-resource-limits.md). It's currently `wasmtime = "=45.0.2"` in `libs/mnemra-host/Cargo.toml`, which **satisfies** this floor. This ADR doesn't restate that pin or compete with it. The exact version is a single-source fact in P-0012's domain, so this ADR states the floor and defers to P-0012 for the operational value.

**On the `async` feature and the current tree: a forward contract, not a present-tense claim.** This decision locks that the async host feature *will be enabled* when the async host-fn invocation path lands. Today, `libs/mnemra-host/Cargo.toml` declares `features = ["cranelift", "component-model"]` and **not yet** `async`. That's because the component-host invocation path (async host-fn bodies, `instantiate_async`) isn't built. The host-fn bodies are `todo!()` stubs and the pool still instantiates core modules, as [P-0013](P-0013-plugin-invocation-model.md)'s "Bucket A" analysis describes. The `async` feature gets added together with that path. This ADR doesn't claim it's enabled today. (This creates a reconciliation obligation against the feature list P-0012 records; see Reconciliation.)

**Acceptance criteria:**
- [ ] The operational Wasmtime pin (P-0012's `=45.0.2`) is `>= 44.0.0`.
- [ ] The `component-model-async` crate feature is NOT enabled at V0 (verifiable in `libs/mnemra-host/Cargo.toml`).
- [ ] When the async host-fn path lands, the `async` crate feature is enabled and host-fn import bodies register as `async` (via `func_wrap_async` or bindgen-generated async imports); no host-fn body blocks the executor thread synchronously on IO.

### D4 — Guest toolchain: `wasm32-wasip2` directly, not `cargo-component`

**Decision:** Rust plugin guests target the upstream **`wasm32-wasip2`** target directly (`cargo build --target wasm32-wasip2`, with bindings from `wit-bindgen`). They **don't** use `cargo-component`. *Anchors: `P-StackDiscipline` (the upstream Rust target is the in-stack, first-class path); the survey's toolchain finding (`cargo-component` is documented as "experimental, not currently stable", its last release was about 8 months stale, and the upstream `wasm32-wasip2` target is replacing it).* The workspace `rust-toolchain.toml` declares `targets = ["wasm32-wasip2"]`, and `wit-bindgen = "0.57"` is pinned at the workspace root. The spike guest (`plugins/mnemra-echo/`) builds this way.

**Acceptance criteria:**
- [ ] `rust-toolchain.toml` lists `wasm32-wasip2` as a target.
- [ ] The plugin build uses `cargo build --target wasm32-wasip2`; no `cargo-component` invocation appears in the build recipes (`justfile`).
- [ ] Guest bindings are generated by `wit-bindgen` (pinned `0.57`), not by `cargo-component`'s bindings path.

### D5 — The Extism-migration door is closed

**Decision:** There's **no** hybrid host and **no** preserved migration path that stays compatible with Extism. Mnemra doesn't run two host-fn ABIs side by side (Extism-allocator/JSON for core modules next to WIT-typed for components), and it doesn't keep an Extism-shaped fallback "in reserve." *Anchors: `P-StackDiscipline` (one substrate, not two ABIs maintained forever); [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) Decision A (raw Wasmtime chosen over Extism at the runtime layer); the survey §6 finding (a Wasmtime Engine *can* load both core modules and CM components, so a hybrid is mechanically possible, but for mnemra it means two host-fn ABIs in parallel forever with zero legacy to justify it).* [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) closed the door at the *runtime-framework* layer: the Extism framework isn't a dependency. This ADR closes it at the *ABI-shape* layer. The WIT ABI isn't compatible with the Extism PDK, and no code path keeps such an ABI alive.

The close is **conditional on the retreat trigger (DEF-4) not firing.** If a hard multi-language requirement forces Path B to be reconsidered, reopening a hybrid Extism-PDK path next to CM is exactly what that review would weigh. So the door is closed for V0, but the condition for reopening it is named rather than erased (`P-PreserveDecisionSpace`).

**Acceptance criteria:**
- [ ] No `extism` dependency in any workspace `Cargo.toml`.
- [ ] The host exposes exactly one host-fn ABI (the WIT-typed one); there is no second Extism-style registration path.

### D6 — Plugin lifecycle sequence

**Decision:** A `core: true` plugin goes through a fixed lifecycle. This ADR locks the **ordering invariants that cut across the pieces**; the sibling ADRs cited at each step own the mechanics of each transition. *Anchors: `P-SecurityLayered` (verification before execution is a structural control); `P-LockContract` (the lifecycle sequence is part of the plugin contract); the Frame DoS-containment QA scenario (trap → kill → replace).* The ordering invariant is the lock that carries the weight here: **no later state is reachable without its predecessor**, and the verification gates fail closed.

Each step below gives the state, the gate that must pass to advance, and the ADR that owns the mechanics:

> *Cross-reference (2026-07-07): for plugins delivered as a bundle, a **distribution pre-phase** (verified fetch and unpack) comes before state 1. [P-0023-plugin-distribution](P-0023-plugin-distribution.md) D4 owns its canonical bounds-first ordering and its mechanics. It's a pre-phase, not a tenth state here.*

1. **Discovered.** The component `.wasm` and its signed manifest are located (a single `core: true` plugin at V0).
2. **Signature-verified.** The manifest signature chains to the mnemra root key. This step is **synchronous and fails closed**: no instance exists until `verify()` returns `Ok`, and `core` status is honored only on signature provenance. *Owner: [P-0005](P-0005-v0-signing-chain.md) (plus its 2026-05-24 amendment).*
3. **Content-hash-verified.** The component bytes must match the manifest's signed content-hash binding (BLAKE3). The hash is recomputed on every load, and a mismatch or a missing binding fails closed. *(After the 2026-07-07 cutover, the binding source is `[[artifacts]]` entry #1, per [P-0003](P-0003-plugin-manifest.md) §Amendment 2026-07-07, which supersedes `[component]`. A body that carries `[component]` is itself rejected. The primitive is unchanged.)* *Owner: [P-0003](P-0003-plugin-manifest.md) §Amendment 2026-07-07 (the historical single-component form is in §Amendment 2026-06-30).*
4. **Manifest-parsed + allowlist-compiled.** The host-fn surface the manifest declares is compiled into a per-instance allowlist. Calls outside it fail at the WIT boundary. *Owner: [P-0003](P-0003-plugin-manifest.md).*
5. **Instantiated into the pool.** The component is instantiated (as a Component Model component, not a core module) with its per-instance resource limits attached (fuel, epoch deadline, memory ceiling), into a host-managed pool of 3 to 5 instances. *Owner: [P-0007](P-0007-plugin-resource-limits.md).*
6. **Ready.** The instance sits in the pool waiting for invocation. State persists across invocations on the same instance (the long-running, stateful, in-process model).
7. **Invoking.** For each authenticated MCP verb, the host runs `DF-auth-check` and the manifest's per-verb capability check **before dispatch**. It then invokes the **exact typed `content` export** for that verb, under a per-invocation fuel and epoch budget. Verb-to-export resolution is static, with no string dispatch. *Owners: [P-0013](P-0013-plugin-invocation-model.md) (export invocation), [P-0007](P-0007-plugin-resource-limits.md) (per-invocation budget), the MCP front (spec `R-0010-c/d`).*
8. **Trapped → killed → replaced.** On a resource-limit trap (fuel, epoch or memory), the store traps. The host logs a structured attribution event, poisons the pool slot and creates a fresh instance. The trap is **never** propagated as a panic in the host process. *Owner: [P-0007](P-0007-plugin-resource-limits.md).*
9. **Shutdown.** Pool instances are dropped when the host shuts down.

**Acceptance criteria:**
- [ ] Signature verification (state 2) and content-hash verification (state 3) both complete `Ok` before any instance is created (state 5); a failure at 2 or 3 yields a load rejection, not a loaded-but-unverified instance.
- [ ] A manifest-declared verb with no matching typed export is non-dispatchable and returns a structured error (not a panic, not a string-dispatch fallback); see [P-0013](P-0013-plugin-invocation-model.md).
- [ ] A resource-limit trap (state 8) results in a replaced pool slot and an error for the current invocation, with the host process surviving (no panic); see [P-0007](P-0007-plugin-resource-limits.md).

### Deferrals

Each deferral states what it would decide, the canon that supports deferring it now, and the **named tripwire** that fires it.

#### DEF-1 — WIT-level async types (Layer 2 / `component-model-async`)

**Deferred:** WIT-level async return types (`future<T>`, `stream<T>`, `error-context`) and the primitives for scheduling concurrent tasks. These would let a host-fn return a stream (a streaming `read` or query verb, say) instead of a fully materialized result. *Deferral anchor: `P-Defer`. The `component-model-async` crate feature is feature-gated and a moving target in Wasmtime 44/45, so adopting it now means opting into an unstable ABI surface.* **Tripwire:** both of two conditions must hold. A required host-fn or content verb must have a contract that genuinely needs a streaming or async-typed return, **and** the runtime must have stabilized `component-model-async`. The WIT corpus is designed so that such a verb can be added later under a new `@since` version without reshaping the sync surface. Preferring a cleaner async story isn't a firing condition.

#### DEF-2 — Third-party plugin distribution + signature verification (cosign-on-`wkg`)

**Deferred:** How **third-party** (non-`core`) plugins get distributed and verified. That means publishing components to an OCI/`wkg` registry and verifying a per-artifact signature (cosign/sigstore-style, with an `application/vnd.mnemra.plugin.v1` artifact type) before instantiation. *Deferral anchor: `P-Defer`. V0 ships only `core: true` plugins embedded in the binary. There's nothing to distribute, so nothing at V0 forces the distribution-verification mechanism.* This is separate from the V0 signing chain and does **not** weaken it: [P-0005](P-0005-v0-signing-chain.md) already locks synchronous, fail-closed ed25519 verification of `core: true` plugins at load. The survey's §4 finding is that `wkg` (the Bytecode Alliance's canonical CM distribution path) does **not** build in cosign the way `hyper-mcp` does. So this is real work mnemra builds at V0.1+, not something the substrate hands over for free. **Tripwire:** the first surface for distributing third-party (non-`core`) plugins opens, meaning third-party plugin install at V0.1+. The survey recommended a shape for when it fires (a sigstore-compatible verifier over `wkg`'s OCI fetch, applied before instantiation). That recommendation is recorded as the survey's, not locked here.

**Disposition (2026-07-07, W2-1 plugin distribution, [P-0023](P-0023-plugin-distribution.md)):** there were two moves, and one of them is partial, so this deferral is *not* fully retired.

**(a) The recorded shape is superseded.** The locked 2026-07-01 distribution research replaces cosign-on-`wkg` with **keyed-in-tree signing**: the P-0005 root signs over the OCI manifest digest, domain-separated, as an OCI 1.1 referrer. (`cosign` signs only as a Go CLI, which is a stack-discipline cost.) It also replaces `wkg`'s one-component-per-package convention with **direct-OCI bundle authoring**. The `application/vnd.mnemra.plugin.v1` artifact type and the verify-before-instantiate ordering recorded above are **retained** as designed.

**(b) The single-publisher scope was deliberately pulled ahead of the tripwire** (the maintainer's board placement, 2026-07-05). The `core: true` distribution story is designed now ([P-0023](P-0023-plugin-distribution.md); spec R-0078 to R-0091).

**What stays deferred, with unchanged force:** the **third-party-publisher** half. That covers non-`core` distribution, key splitting for external publishers, and delegated roles. It stays behind the same tripwire, which is now also expressed as [P-0005](P-0005-v0-signing-chain.md)'s R-0005-e condition.

#### DEF-3 — V0.1+ language-support matrix

**Deferred:** The matrix of supported plugin-authoring languages beyond Rust. V0 is **Rust-only** (Rust first through WIT, per the Frame's V0/V1 boundary). When third-party authoring opens, the V0.1+ matrix is tiered:

- **Tier 1:** Rust, and JS/TS (via `jco`).
- **Tier 2:** Python (`componentize-py`, with a caveat about build-time imports), TinyGo (standard Go has documented WIT/GC issues, so TinyGo only), and C# (gaps relative to Spin).
- **Tier 3:** C/C++ and MoonBit.

Languages outside the matrix have no supported on-ramp. *Deferral anchor: `P-Defer`. The matrix is a V0.1+ product-positioning commitment, and nothing at V0 forces it (V0 has no third-party authors). Publishing it before authoring opens would lock in a maturity claim about tooling that's still moving.* **Tripwire:** third-party plugin authoring opens at V0.1+. At that point the matrix is published in the plugin-author docs, tiered by how mature the tooling is then. This is an honest narrowing of "plugins in any language." It's a planned consequence of Path B, not a stumble.

#### DEF-4 — Path-B retreat trigger

**Deferred (as a reversal condition, not a decision):** whether to reconsider Path B in favor of a hybrid that brings back an Extism-PDK authoring path next to CM. *Anchor: `P-PreserveDecisionSpace` (Path A stays a documented, reachable fallback) plus `P-StackDiscipline`'s "unless no in-stack path exists" clause (the retreat is where that clause would fire).* The Bytecode Alliance's CM language list leaves out four languages that Extism's PDK supports and that have no CM authoring path in 2026: **F#, Haskell, AssemblyScript, Zig**. Haskell is the loss that really matters; the other three are small or long-tail, per survey §3. Mnemra's actual Tier-1 audience (Rust, JS/TS, Python, Go) is well covered on CM, so the gap is tolerable for V0. **Tripwire:** a Tier-2 enterprise customer raises plugin authoring in F#, Haskell, AssemblyScript or Zig as a **hard requirement**. If that fires, this ADR is revisited, and a hybrid Extism-PDK path next to CM is weighed against the cost of two host-fn ABIs (D5). Without a hard requirement, Path B stands and the Extism door (D5) stays closed. Wanting broader language reach isn't a firing condition.

### Consequences

**Good:**

- The foundational plugin-model decision finally has an ADR. A reader starting here can find the whole plugin contract by following citations. And a future reversal (Path B → Path A) has a concrete record to reverse against, instead of only the Frame's narrative.
- The ABI shape invariants (D2) give the maintainer's stated values a single durable statement. Those values are strict typing, no non-deterministic string dispatch, and contracts tools can check. D2 complements P-0013's lock on export-side typing.
- The async-layer decision (D3) commits to the GA, stable executor-side path. It quarantines the moving `component-model-async` target behind a tripwire, so the V0 substrate doesn't track an unstable ABI feature.
- The Extism door (D5) is shut at both the runtime-framework layer (P-0012) and the ABI-shape layer (here). That prevents a drift into two ABIs forever, while the retreat trigger keeps a reversal honest.
- The lifecycle sequence (D6) locks the security-relevant ordering (verify → hash → allowlist → instantiate → invoke; trap → kill → replace) as an invariant across the pieces. A future refactor can't move verification after instantiation without visibly breaking this ADR.
- The four deferrals give the "plugins in any language" and "signed third-party distribution" stories named V0.1+ landing conditions, in place of implicit expectations.

**Bad / Trade-offs:**

- **No production substrate exemplar.** Path B means building on Wasmtime from primitives. In 2026 no production user runs mnemra's exact shape (long-running, stateful, in-process CM plugins) on CM (survey §5). The runtime is mature. But the *plugin-runtime wiring* (pool, lifecycle, host-fn registration) is mnemra's to write, with documentation and the WIT spec as reference instead of working code to lift. This is accepted as the cost of building the substrate without a precedent, consistent with the project weighting clean design over schedule.
- **Language reach narrows** from about 10 (Extism PDK) to about 8 (the languages the Bytecode Alliance supports for CM). Authoring paths for F#, Haskell, AssemblyScript and Zig are lost (DEF-4). Tier-1 audience coverage and the retreat trigger mitigate this. For a plugin author in a Haskell shop, it stays unmitigated until the trigger fires, if it ever does.
- **Async ceiling.** Sync WIT with Layer-1 async gives the same concurrency ceiling as Path A: one call in flight per instance, which the instance pool solves. The cleaner WIT-async story is deferred (DEF-1).
- **This ADR carries no field-level ABI or manifest detail of its own.** It's deliberately a document of cross-cutting invariants and citations. A reader who wants a specific host-fn signature or manifest field has to follow the citation to `wit/host.wit`, [P-0003](P-0003-plugin-manifest.md) or [P-0013](P-0013-plugin-invocation-model.md). That's the intended division of authority, since it avoids duplicating single-source facts. But it means P-0019 can't answer signature-level questions on its own.

## Pros and Cons of the Options

### Path B — Component Model + WIT (chosen)

- Pro: No marshalling on the typed ABI structure, because native WIT values cross the boundary. For the in-process boundary, it's the cleanest possible answer to the workspace typed-binary-encoding standard.
- Pro: WIT stability gates (`@since`/`@unstable`/`@deprecated`) are ABI-evolution discipline that the toolchain enforces. Path A would have to police the same discipline in hand-written JSON-schema code.
- Pro: Typed `resource` and record contracts plus static verb-to-export resolution make the system something tools can check and people can reason about, which the maintainer values. There's also a single path for evolving the substrate (no dual ABI).
- Con: No production exemplar at mnemra's shape, so the wiring has to be built without a precedent (survey §5). Language reach narrows compared with Extism's PDK.

### Path A — stdio-MCP-wrappers / Extism substrate (rejected)

- Pro: A reusable substrate (lift from `hyper-mcp`), a PDK covering about 10 languages inherited "for free", and V0 ships sooner.
- Con: Going Extism-first and then CM is a re-platform, not a migration. Every plugin's host-fn imports change shape at V0.2+, and the only alternative is a dual ABI maintained forever. There's marshalling (JSON/allocator) at the boundary, and the typing is weaker than native WIT. It starts out violating the workspace typed-binary-encoding standard for the host↔plugin boundary. And the reusable-substrate benefit is biggest exactly at V0: once CM is wired directly, the migration cost is work already paid.

### Layer-1-only async, sync WIT (chosen) vs Layer-2 WIT async (rejected for V0)

- Pro (Layer 1): `Linker::instantiate_async` / `func_wrap_async` are GA behind the stable `async` feature. The host bridges IO-bearing host-fns on its executor while the guest writes plain sync functions. Spin and wasmCloud run the same pattern in production.
- Con (Layer 2): `future`/`stream`/`error-context` sit behind `component-model-async`, which is feature-gated and not committed as stable in 2026. Building host-fns that return `stream<T>` now would track a moving target. Deferred with a tripwire (DEF-1).

### `wasm32-wasip2` directly (chosen) vs `cargo-component` (rejected)

- Pro (`wasm32-wasip2`): the upstream Rust Tier-2 target. It's first-class and builds with plain `cargo build`, no extra wrapper. It's the direction the ecosystem is consolidating toward.
- Con (`cargo-component`): documented as "experimental, not currently stable", with a release cadence about 8 months stale, and explicitly being replaced by the upstream target (survey §6). It's only warranted for richer non-WASI WIT scenarios, which V0 doesn't have.

## Reconciliation with prior ADRs

This ADR is a **capstone that cites**. It records the foundational Path-B decision the sibling ADRs already assumed, and defers to each of them for the detail it owns. It does **not** supersede or override any of them, so `supersedes`/`superseded_by`/`overrides` stay `null`. The division of authority:

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

1. **The `async` crate feature vs P-0012's recorded feature list.** D3 locks "async feature ON at V0" as a **forward contract** that takes effect when the async host-fn path lands. P-0012's References record the *current* `libs/mnemra-host/Cargo.toml` as `features = ["cranelift", "component-model"]` (no `async`), which reflects the scaffold before the async bridge exists. The two are consistent, because the tree hasn't reached the async host path yet (the host-fn bodies are `todo!()`, per P-0013's Bucket A). This ADR does **not** claim the feature is enabled now. Whether P-0012's feature list should get a companion note saying `async` arrives with the async path is a single-source reconciliation question for the maintainer. This ADR doesn't edit P-0012.
2. **The illustrative survey WIT shape vs the implemented shape.** The survey proposed an illustrative WIT surface with `list<u8>` bodies, `resource` handles and `result<T,E>` returns. The implemented and locked shape diverged: `type json = string` instead of `list<u8>`; `workspace-ctx` as a record instead of a resource, so contract tests can resolve it; and `option` returns. D2 locks the shape *invariants* and defers to `wit/host.wit` / `wit/echo.wit` plus P-0003/P-0013 for the exact signatures. The survey is bounded input, and where the two differ the implemented WIT is authoritative.

## More Information

- **Frame relationship.** This ADR does **not** resolve a `{{P-XXX}}` open ADR slot. The Path-B plugin model came forward from the brief as a Hard constraint and was narrated in the Frame ([`docs/src/intent/mnemra-core-frame.md`](../intent/mnemra-core-frame.md), §Correction 1/2 and §"Plugin runtime"), but it never got its own slot. Compare the manifest schema (`{{P-PluginManifest}}` → P-0003) and the resource limits (`{{P-PluginPoolMemory}}` → P-0007), which did. This ADR records that narrated Hard constraint, plus the cross-cutting commitments on ABI shape, toolchain, lifecycle and language that the researcher's survey named as "ADR-8 implications."
- **The gating spike.** Commit `c9018e3` ("scaffold V0.01 + CM existence-test spike", on `main`, 2026-05-07). It contains a Cargo workspace with a host (`cmd/mnemra`) and a plugin (`plugins/mnemra-echo/`), the WIT contract, and Wasmtime 44 + `wasm32-wasip2`, with all 7 acceptance criteria passing. It's the evidence that Path B instantiates at mnemra's shape.
- **Quality-attribute scenarios anchored.** DoS containment ("infinite-loop plugin killed and replaced from the pool; no single-process-wide DoS") maps to D6 states 7 and 8, owned by [P-0007](P-0007-plugin-resource-limits.md). ABI evolution ("a plugin calling an `@unstable` host function emits a deprecation warning"; "an ABI-change PR causes all `core: true` plugins to recompile and pass tests") maps to the D2.6 stability gates and the D4 `wasm32-wasip2` recompile surface.
- **Threat references** (from the companion [overview](../architecture/overview.md)): `P-plugin-instance`/E (allowlist mitigation, D2/D6-4), `P-plugin-runtime`/D,E,R (kill-and-replace + fail-closed verify, D6), `P-host-fns`/T (workspace-ctx exclusion, D2.5), `DS-mnemra-root-key`/I (signing, D6-2). The ABI shape and the lifecycle preserve these structural mitigations.
- **Concrete artifacts (repo-relative, load-bearing):** `wit/host.wit` (host-fn imports + `content` export interface), `wit/echo.wit` (the plugin `world`), `libs/mnemra-host/Cargo.toml` (Wasmtime pin + features), `rust-toolchain.toml` (`wasm32-wasip2` target), `Cargo.toml` (`wit-bindgen = "0.57"`), `plugins/mnemra-echo/` (the spike fixture), `docs/specs/2026-05-24-mnemra-core-v0-substrate.md` (import ABI `R-0012-*`, MCP front `R-0010-*`, tenant ctx `R-0006-*`; export/invocation ABI `R-0019-*`, paging `R-0020`).
- **Bounded input:** the researcher's Component Model follow-up survey (2026-05-04). It covered WIT patterns, the two-layer async story, the multi-language matrix, distribution and signing on CM, and production precedents, and it's the direct basis for D1 to D6 and the deferrals.
- **Canon anchors:** `P-StackDiscipline` (CM+WIT, the async bridge and `wasm32-wasip2` are in-stack; the retreat clause), `P-LockContract` (the WIT boundary is the product contract), `P-Defer` (four deferrals with tripwires), `P-PreserveDecisionSpace` (Path A recorded as the option chosen against; the maintainer's typing values kept verbatim), `P-SecurityLayered` (verify-before-execute ordering; host-derived ctx; allowlist), `P-AgentPrimarySource` (a typed contract that tools can check).
- **Follow-ups:**
  - When the async host-fn invocation path lands (P-0013 Bucket A), enable the Wasmtime `async` feature and register host-fn bodies as async (the D3 acceptance criterion). Confirm whether P-0012's feature-list record needs a companion note (Reconciliation obligation 1, the maintainer's call).
  - When third-party plugin authoring opens at V0.1+, publish the DEF-3 language matrix in the plugin-author docs, tiered by the tooling maturity at that time, and build the DEF-2 distribution-signature verifier over `wkg`/OCI.
- **Related precedent:** [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) and [P-0013](P-0013-plugin-invocation-model.md) set up the accepted-on-landing and forward-contract framing that this ADR follows for a foundational decision recorded after the detail that depends on it.

## Amendment 2026-10-25 — The Wasmtime floor restated as capabilities; crate versions are not design values

Applied on 2026-10-25 under the maintainer's rulings on #4249. Three activities on that task carry the rulings. Activity 8285 rules that design records carry no crate or library version. Activity 8303 confirms 8296: a crate version stays out whether as a pin or as a floor, and a design dependency on a crate names the capability it needs. Activity 8380 rules that versioned provenance stays. The stated reason is that a version change, such as a bump forced by a security advisory, must never require reopening a locked record. The measured case behind the ruling includes this record. It named `=45.0.2` as the operational pin after `libs/mnemra-host/Cargo.toml` had already moved past it, with no amendment and no signal.

### D3, restated

**Decision:** The plugin substrate requires a Wasmtime release that provides, together, the three properties D3's floor rationale names:

1. Component Model support with semver-aware import resolution.
2. Executor-side async through `Linker::instantiate_async` / `func_wrap_async` behind the `async` crate feature.
3. Instantiation of components built for the `wasm32-wasip2` guest target (D4).

The async layering doesn't change: executor-side async is ON when the async host-fn path lands, and `component-model-async` is OFF (DEF-1). Which release meets these properties is an implementation fact, held in `libs/mnemra-host/Cargo.toml`. No design record owns an exact Wasmtime pin, and that includes [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) (see its Amendment 2026-10-25). *Anchors: maintainer rulings #4249 activities 8285, 8303; `P-StackDiscipline` and `P-Defer` as in D3.*

**Acceptance criteria.** These two criteria supersede the first D3 criterion ("The operational Wasmtime pin (P-0012's `=45.0.2`) is `>= 44.0.0`"). The other two D3 criteria stand unchanged.

- [ ] `libs/mnemra-host/Cargo.toml` enables the Wasmtime `component-model` feature.
- [ ] `libs/mnemra-host/tests/smoke_e2e.rs`, which loads the committed `wasm32-wasip2` component `artifacts/mnemra-echo/mnemra_echo.wasm` (built from `plugins/mnemra-echo/`), passes against the release `libs/mnemra-host/Cargo.toml` pins.

The existing third D3 criterion observes property 2 once the async host-fn path lands.

**Open, for the maintainer.** No criterion observes the semver-aware half of property 1, which is a guest built against an older `mnemra:host` minor version linking against the host's newer one. The superseded `>= 44.0.0` criterion only stood in for it, and no test in `libs/mnemra-host/tests/` exercises it today. This amendment adds no criterion for it, since a new criterion would need a new test. Whether to add one, with the test routed to the implementation lane, is open on #4249.

### D4

**Superseded:** "`wit-bindgen = "0.57"` is pinned at the workspace root" and the criterion's "(pinned `0.57`)". `wit-bindgen` is a workspace dependency declared in the root `Cargo.toml`, and its release isn't a design value. The D4 criterion now reads: "Guest bindings are generated by `wit-bindgen`, not by `cargo-component`'s bindings path."

### Other citations

- **Superseded:** four phrases that tie this record to an exact Wasmtime version. They're "the exact Wasmtime pin" in the list of what P-0012 owns (frontmatter summary and Status), the Context bullet giving P-0012 "the **exact Wasmtime pin** (`=45.0.2`)", D3's "currently `wasmtime = "=45.0.2"` … which **satisfies** this floor", and the Reconciliation row "Cites for the `=45.0.2` pin (D3 states only the `>= 44` floor)". No record owns the exact pin, and D3 is restated above.
- **Superseded in part:** the More Information entry "`Cargo.toml` (`wit-bindgen = "0.57"`)". The pointer stands, and the quoted value is the file as it was recorded on 2026-07-02.
- **Kept as provenance:** DEF-1's "a moving target in Wasmtime 44/45" stays. It's the assessment made when this record was written, against those releases, and the DEF-1 tripwire was already keyed on the runtime stabilising `component-model-async`, not on a version. "Wasmtime 44 + `wasm32-wasip2`" in the gating-spike entry also stays, because it's what commit `c9018e3` ran on.
