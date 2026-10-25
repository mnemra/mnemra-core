---
title: "P-0012: Plugin Runtime (raw Wasmtime) + MCP SDK (`rmcp`)"
summary: "Two implementation decisions for the V0 plugin substrate that an unratified research proposal (ADR-001) had bundled into a single Extism + rmcp + hyper-mcp stack — and that the build reversed on both halves without recording either. Decision A (retroactive, shipped in Tasks 20–22): host plugins on raw Wasmtime, not Extism, because P-0007's per-instance resource-limit mechanism (fuel + epoch + ResourceLimiter) needs direct Wasmtime control that Extism's abstraction fights. Decision B (forward, implemented by Task 23): adopt the official MCP Rust SDK `rmcp` v1.7.0 (Apache-2.0, Green-tier) (Amendment 2026-10-25, #4249: no crate version is a design value; Decision B is the official SDK with a stdio server transport and no HTTP transport, and Decision A is raw Wasmtime with the surface P-0007 drives, at whatever releases `Cargo.toml` pins) for the single stdio MCP server rather than hand-rolling the JSON-RPC/MCP wire layer — the MCP server is the auth+dispatch trust boundary and R-0010-a mandates conformance to MCP 2025-06-18, the worst place to hand-roll a mandated external wire contract. mnemra's R-0010 handler logic (auth-check, single WorkspaceCtx construction, per-verb capability check, distinguishable JSON-RPC error codes) rides on top as the rmcp ServerHandler, unchanged by the SDK choice."
primary-audience: agent
---

---
status: "accepted"
date: "2026-06-19"
decision-makers: ["the maintainer"]
consulted: ["the orchestrator"]
informed: []
supersedes: null
superseded_by: null
overrides: null
---

# P-0012: Plugin Runtime (raw Wasmtime) + MCP SDK (`rmcp`)

**Project:** mnemra-core

## Status

`accepted`

The maintainer made both decisions recorded in this [ADR](../glossary.md#adr) (architecture decision record). They're recorded together because they correct two halves of the **same** unratified proposal, the plugin substrate stack. The build reversed both halves, and no ADR recorded either reversal.

- **Decision A (raw Wasmtime over Extism)** is recorded as `accepted` on landing rather than `proposed`. It's already in the tree, merged in Tasks 20-22 (commits `8bb2326` "plugin runtime — manifest load, allowlist, wasmtime limits, epoch supervisor (Tasks 20-21)" and `d378118` "resource-limit trap handling — kill-and-replace recovery (Task 22)"), so it isn't waiting on a review gate. This ADR ratifies the choice after the fact, the same way [P-0011](P-0011-logging-facade.md) accepted the `tracing` scaffold on landing.
- **Decision B (official `rmcp` SDK over hand-rolled MCP)** is `accepted` as a forward decision. The maintainer has locked the choice, and Task 23 (the MCP server work) implements it. The lock comes before the implementation. That's why the status is `accepted` and not `proposed`: no open question remains for a gate to resolve, only the build to follow.

This ADR doesn't `supersede` or `override` any prior record. The proposal it corrects is an earlier internal research proposal ("ADR-001: Plugin runtime — Extism on Wasmtime, with rmcp 1.5"). It carries **Status: Proposed** and was never ratified into this project's accepted ADR series (P-0001…P-0011 contains no plugin-runtime or MCP-SDK ADR). A research proposal that was never accepted can't be superseded. So the relationship is described in Context instead, and `supersedes`/`overrides` stay `null`.

## Context and Problem Statement

The V0 plugin substrate (the first release's plugin foundation) involves two implementation choices. They're easy to conflate, but they're **orthogonal**:

1. **The plugin execution runtime:** what hosts and sandboxes plugin WASM (Extism vs raw Wasmtime vs other).
2. **The MCP protocol layer:** what speaks the MCP/JSON-RPC wire contract to agent clients (official `rmcp` SDK vs hand-rolled).

An earlier internal research proposal ("ADR-001: Plugin runtime — Extism on Wasmtime, with rmcp 1.5", **Status: Proposed**, dated 2026-04-27) bundled both into one recommended stack: **Extism (on Wasmtime) + `rmcp` 1.5 + a hyper-mcp-shaped host**. It was never ratified into this project's accepted [P-ADR](../glossary.md#p--adr) series (project-scoped decisions). The actual build then reversed **both** halves, and neither reversal was recorded:

- The plugin runtime shipped on **raw Wasmtime**, not Extism (Tasks 20-22, on `main`).
- The MCP layer was, when this ADR was written, about to be **hand-rolled** rather than built on `rmcp`.

The first reversal was a sound engineering call that followed from P-0007 (see Decision A). The second was **collateral drift, not a decision**. Dropping Extism removed the runtime half of the proposed stack. But it gave no reason to drop `rmcp` as well, because the SDK is the *protocol* layer and is orthogonal to the *runtime* layer. The only thing coupling the proposal's two halves was that they were written in one document. No technical dependency tied them. This ADR records the runtime reversal on its merits and corrects the MCP drift back to the official SDK. It separates two decisions that were never really coupled.

Neither decision below reopens a settled question. The maintainer settled both. They're recorded here under [`P-PreserveDecisionSpace`](../glossary.md#p-preservedecisionspace) (keep the alternatives a decision was made against), which means recording the surfaced alternatives the lock chose among, not deriving the decisions again.

## Decision Drivers

- **P-StackDiscipline applied correctly to each layer.** `P-StackDiscipline` (workspace `architecture-principles.md`; "reject ecosystem-misaligned tooling unless no in-stack path exists") cuts **differently** for the two decisions. Naming which way it cuts is the spine of this ADR. For the runtime, the in-stack mechanism (raw Wasmtime's own resource knobs, already a direct dependency) covers the requirement better than the abstraction layered on top of it. So the abstraction comes out. For the MCP layer, the SDK *is* in-stack (a Rust crate, no foreign runtime), and the requirement is a **mandated external wire contract**. That isn't the kind of "small mechanism" the hand-roll carve-out covers, so the SDK goes in. Same principle, opposite outcomes, because the layers differ.
- **P-0007's resource-limit mechanism needs direct Wasmtime control.** The V0 per-instance limits are fuel through `Store::set_fuel`, `Store::set_epoch_deadline` with a supervised host epoch-tick thread, and a 64 MiB `ResourceLimiter`. All of them are mechanisms on Wasmtime's `Config`/`Store` surface. The plugin manifest's typed host-fn allowlist is compiled per instance. These need the raw `wasmtime::{Engine, Store, Config}` surface, which an Extism wrapper would mediate and partly hide. (Decision A.)
- **The MCP server is the auth + dispatch trust boundary.** It runs `DF-auth-check` (the authentication check) on every request before routing, and it's the single point where a workspace identity is established. Hand-rolling the wire and JSON-RPC framing there maximizes both the surface that needs security review and the risk of drifting from an external spec. And it does so at the worst possible location. (Decision B.)
- **R-0010-a mandates conformance to an external wire contract.** [R-0010-a](../../specs/2026-05-24-mnemra-core-v0-substrate.md) (a numbered [requirement](../glossary.md#r-codes) in the V0 substrate spec) reads: "The system SHALL run a single MCP server using the MCP specification 2025-06-18 with stdio transport at V0." Conformance to a versioned external protocol is exactly what an official, conformance-tested SDK provides. A hand-roll risks getting it subtly wrong, with nothing to signal the mistake. (Decision B.)
- **License tier is clear and additive, not the lead argument.** Both choices are Green-tier, the license tier that proceeds without a halt under the workspace dependency-approval model (Wasmtime: Apache-2.0; `rmcp`: Apache-2.0, confirmed on crates.io, see below). So neither triggers a license-gate halt. Under `P-StackDiscipline` rule S2, license tier is a *separate, additive* gate and doesn't justify a dependency on its own. In both decisions, the fit-for-purpose argument leads.

## Considered Options

There are two independent decisions here, each with its own option set. They share one ADR because they correct the two halves of one proposal. Each is decided on its own.

### Decision A — Plugin execution runtime

1. **Raw Wasmtime (chosen).** Depend directly on `wasmtime` (Component Model + Cranelift). Drive fuel, epoch interruption and the memory `ResourceLimiter` from the raw `Config`/`Store` surface. mnemra-host owns the pool, the epoch-tick supervisor and trap recovery.
2. **Extism on Wasmtime (the proposal's runtime half).** Host plugins through the Extism framework, which is itself backed by Wasmtime. This gains a multi-PDK plugin-authoring story and a ready host-fn registration model. The cost is an abstraction layer between mnemra-host and the Wasmtime `Config`/`Store` knobs that P-0007 drives.

### Decision B — MCP protocol layer

1. **Official `rmcp` SDK (chosen).** Depend on `rmcp` v1.7.0 (the official MCP Rust SDK). Implement mnemra's MCP server as an `rmcp` `ServerHandler`. Enable `transport-io` (stdio), and enable **no** HTTP-transport feature.
2. **Hand-rolled MCP/JSON-RPC layer.** Implement the MCP 2025-06-18 wire contract, JSON-RPC framing and stdio transport by hand inside mnemra-host. There's no SDK dependency and full control of the wire layer. There's also full ownership of its conformance and its security surface.

## Decision Outcome

### Decision A — raw Wasmtime (Option 1)

**Chosen: raw Wasmtime.** P-0007's resource-limit mechanism is defined directly in terms of Wasmtime's `Config`/`Store` surface, and Extism's abstraction would mediate exactly the knobs P-0007 needs to drive.

[P-0007](P-0007-plugin-resource-limits.md) ("Plugin Resource Limits"; fuel 10B + epoch 5s + 64 MiB ceiling, with both fuel and epoch ON at V0) locks a mechanism stated in Wasmtime-native terms. It uses `Store::set_fuel`, `Store::set_epoch_deadline(500)` with a host thread advancing the epoch counter every 10 ms, and a per-instance `ResourceLimiter`/`static_memory_maximum_size`. The matching requirements [R-0007-a…i](../../specs/2026-05-24-mnemra-core-v0-substrate.md) name the raw Wasmtime API directly: "enable fuel metering via `Store::set_fuel`"; "enable epoch-interruption … via `Store::set_epoch_deadline(500)` with a host epoch-tick thread"; "per-instance memory ceiling … via `Config::static_memory_maximum_size` or a `ResourceLimiter`"; "the Wasmtime version SHALL be pinned". [R-0016](../../specs/2026-05-24-mnemra-core-v0-substrate.md) ("Plugin pool", anchored to P-0007) requires a host-owned pool of 3 to 5 instances with synchronous kill-and-replace. The pool, the epoch supervisor and the trap-to-recovery path are mnemra-host's own machinery, sitting directly on the Wasmtime `Store`. Putting Extism between mnemra-host and that surface would fight the mechanism P-0007 already locked. The resource knobs, the supervised epoch thread and the trap-recovery path all need unmediated `wasmtime::{Engine, Store, Config}` access. Dropping Extism was the sound call, and this ADR ratifies it.

This matches how `P-StackDiscipline` is meant to show up in practice. When the in-stack primitive (raw Wasmtime, already a direct dependency) covers the requirement, an abstraction layered on top isn't warranted. Wasmtime isn't an *added* ecosystem here. It's the runtime the project already targets; Extism would have been the addition.

### Decision B — official `rmcp` SDK (Option 1)

**Chosen: the official `rmcp` SDK (v1.7.0).** The MCP server is both a surface for a mandated external protocol and the auth/dispatch trust boundary. Those are the two properties that argue hardest against hand-rolling a wire layer. `rmcp` is the conformant, in-stack, license-clean way to satisfy both.

The maintainer's lock, recorded verbatim per `P-PreserveDecisionSpace`: *"we'll use the official sdk for now unless there becomes an issue with it."*

**Why the hand-roll was the wrong default (collateral drift):** `rmcp` is the MCP *protocol* layer, while Wasmtime/Extism is the plugin *execution* runtime. Dropping Extism (Decision A) removed the runtime half of the proposed stack and gave **no** reason to drop the protocol half. The two were coupled only by being recommended in one proposal document, not by any technical dependency. Hand-rolling the MCP layer was drift that followed the runtime reversal because it sat next to it. It wasn't a decision made on its own merits.

**Why the SDK is right on its merits:**

1. **A mandated external wire contract calls for a conformant official SDK.** [R-0010-a](../../specs/2026-05-24-mnemra-core-v0-substrate.md) ("a single MCP server using the MCP specification 2025-06-18 with stdio transport at V0") makes MCP conformance a hard requirement against a versioned external spec. `rmcp` is the **official** MCP Rust SDK (`github.com/modelcontextprotocol/rust-sdk`). It's post-1.0, backed by a conformance suite, and has 13M+ downloads. A hand-rolled JSON-RPC/MCP layer risks subtle non-conformance to a wire contract the project doesn't own and can't redefine on its own. `P-StackDiscipline` has a hand-roll carve-out: "when the in-stack option is pre-1.0 or thin, hand-rolling a small mechanism often beats inheriting churn". This is precisely the case that carve-out does **not** cover. MCP/JSON-RPC at the V0 surface isn't small, and it isn't a mechanism the project controls; it's a mandated external protocol. And the in-stack SDK is post-1.0 and mature, not thin.
2. **Trust-boundary minimization.** The MCP server is where `DF-auth-check` runs before any routing ([R-0010-c](../../specs/2026-05-24-mnemra-core-v0-substrate.md): "perform `DF-auth-check` … on every incoming request before routing"). It's also where the single `WorkspaceCtx` is constructed after token validation ([R-0006-b](../../specs/2026-05-24-mnemra-core-v0-substrate.md): "`WorkspaceCtx` SHALL be constructed at a single location in the MCP verb dispatch path, after token validation; there SHALL be no alternative construction path"). Hand-rolling the wire and framing code at this boundary maximizes the attack surface to security-review and the room for framing bugs. It's the worst place to write bespoke protocol code.
3. **Transport features encode the V0 boundary precisely.** Enabling `rmcp`'s `transport-io` (stdio) satisfies R-0010-a's stdio mandate. Enabling **no** HTTP-transport feature is how [R-0010-e](../../specs/2026-05-24-mnemra-core-v0-substrate.md) ("Streamable-HTTP MCP transport SHALL NOT be activated at V0") is satisfied structurally: the capability simply isn't compiled in.
4. **mnemra's R-0010 logic is unchanged by the SDK choice; it rides on top.** The SDK supplies transport, framing and JSON-RPC conformance. mnemra's policy lives in its `ServerHandler` implementation, and it's identical whether the transport is hand-rolled or provided by `rmcp`. That policy is `DF-auth-check` before routing ([R-0010-c](../../specs/2026-05-24-mnemra-core-v0-substrate.md)); a single `WorkspaceCtx` construction after validation ([R-0006-b](../../specs/2026-05-24-mnemra-core-v0-substrate.md)); a per-verb capability check against the manifest's declared `verbs` before dispatch ([R-0010-d](../../specs/2026-05-24-mnemra-core-v0-substrate.md): "enforce a per-verb capability check against the plugin manifest's declared `verbs` list before dispatching"); and distinguishable JSON-RPC error codes for invalid-token vs verb-not-found vs parameter-invalid, never conflated ([R-0010-f](../../specs/2026-05-24-mnemra-core-v0-substrate.md)). Only the wire transport and framing change hands, from a hand-rolled implementation to `rmcp`'s `ServerHandler`. The auth, dispatch and error-classification logic stays mnemra's own.

**`rmcp` facts, confirmed at crates.io (not restated from the proposal):** name `rmcp`; latest version **1.7.0**; license **Apache-2.0** (single, not dual); 13,075,574 downloads; official repository `github.com/modelcontextprotocol/rust-sdk`; description "Rust SDK for Model Context Protocol." Apache-2.0 is **Green-tier** (auto-proceed) under the workspace dependency-approval model. That's the same tier as mnemra-core's own license, so the dependency clears the license gate without a halt. (The proposal cited `rmcp` 1.5; the version locked here is the current 1.7.0.)

> **Deferral / revisit tripwire (named instrument, [`P-Defer`](../glossary.md#p-defer), defer a mechanism choice until evidence forces it).** Decision B holds *"for now unless there becomes an issue with it."* The named tripwire that fires a revisit is **a concrete `rmcp` limitation that blocks a spec requirement**. One example is an `rmcp` behaviour that can't satisfy R-0010-a's MCP-2025-06-18 conformance. Another is one that forces a deviation from the R-0010-c/d/f handler contract. Without a concrete issue like that, one that blocks a requirement, `rmcp` stands. Preferring a different SDK or a hand-roll isn't a firing condition.

### Scope — implementation-decision record only; the spec is unchanged

This ADR records *how* the substrate is built. It doesn't change *what* the spec requires. The locked, verified spec [`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`](../../specs/2026-05-24-mnemra-core-v0-substrate.md) stays locked at its current content-lock (git blob `9f4695f3a9c5f0906a7ce2e3848eb65bbd47834f`, the live spec on `main`, the state this ADR's R-IDs were read from). This ADR does **not** edit it. In particular:

- **R-0010-a is implementation-agnostic and stays UNCHANGED.** It mandates "a single MCP server using the MCP specification 2025-06-18 with stdio transport" without naming an implementation. `rmcp` is the implementation choice that satisfies it. That choice is recorded here; it isn't a change to the requirement.
- The Task-23 plan reshape and the three Task-23 plugin-runtime carries (the limit-attach test, the `can_invoke` invoke-path wiring, the epoch-hook gating) are **plugin-runtime** concerns. The `rmcp` choice doesn't affect them, and they're out of scope for this ADR.

### Consequences

**Good:**

- **Decision A:** P-0007's resource-limit mechanism (fuel + epoch + `ResourceLimiter`) is driven directly on the raw Wasmtime surface. No abstraction sits between it and the knobs the spec names. The pool, epoch supervisor and trap-recovery path are mnemra-host's own machinery, already shipped and exercised in Tasks 20-22.
- **Decision B:** MCP-2025-06-18 wire conformance comes from the official, conformance-tested SDK. The project doesn't have to keep bespoke code conformant by hand. The auth/dispatch trust boundary carries the minimum bespoke surface: only mnemra's `ServerHandler` policy logic, not the JSON-RPC framing.
- **Both:** the substrate now has a recorded, canon-anchored decision for each of the two layers the proposal had bundled. The drift (an unrecorded reversal on each half) is corrected. A future reader can see *which* layer each decision governs and *why*.
- The V0 no-HTTP-transport boundary (R-0010-e) is enforced structurally, by not compiling the `rmcp` HTTP-transport feature, rather than by a runtime guard.

**Bad / Trade-offs:**

- **Decision A:** raw Wasmtime gives up Extism's ready-made multi-language plugin-authoring story and its host-fn registration ergonomics. mnemra-host owns the pool, the epoch supervisor and trap recovery itself (the machinery already built in Tasks 20-22). Accepted: at V0 the plugins are `core: true`, well-behaved, first-party artifacts, and P-0007's direct-control requirement outweighs the convenience for plugin authors.
- **Decision B:** `rmcp` is a new direct dependency and a versioned external API surface to track. Mitigated: it's Green-tier (Apache-2.0) and the official SDK for a protocol the project has to speak anyway. It's also confined to the MCP transport/framing seam behind mnemra's own `ServerHandler`, so a version bump touches the transport seam, not the auth, dispatch or error logic.
- Recording two decisions in one ADR risks a future reader treating them as coupled. Mitigated explicitly: the ADR's whole thesis is that the runtime layer and the protocol layer are **orthogonal**. Each decision rests on its own option set and its own drivers.

## Pros and Cons of the Options

### Decision A — Runtime

#### Raw Wasmtime (chosen)

- Pro: Direct access to the `Config`/`Store` surface that P-0007's fuel/epoch/`ResourceLimiter` mechanism is defined in terms of. No abstraction mediates the locked knobs.
- Pro: Wasmtime is already the project's runtime, so there's no added ecosystem. It's consistent with `P-StackDiscipline` (the in-stack primitive covers the requirement).
- Pro: Already shipped and exercised (Tasks 20-22). The pool, epoch supervisor, trap recovery and kill-and-replace are in the tree.
- Con: No Extism-provided multi-PDK plugin-authoring story. Host-fn registration and the pool machinery are mnemra-host's to own. Accepted for V0's first-party `core: true` plugins.

#### Extism on Wasmtime

- Pro: Ready multi-language plugin authoring (multiple PDKs) and a packaged host-fn registration model.
- Con: An abstraction layer between mnemra-host and the Wasmtime `Config`/`Store` knobs that P-0007 (R-0007-a…i) drives directly. It mediates exactly the surface the locked resource-limit mechanism needs.
- Con: Adds the Extism framework as the runtime layer when the underlying Wasmtime (already a direct dependency) covers the V0 requirement by itself.

### Decision B — MCP protocol layer

#### Official `rmcp` SDK (chosen)

- Pro: Conformance to MCP 2025-06-18 (R-0010-a) comes from the official SDK, which is backed by a conformance suite. No wire conformance to maintain by hand.
- Pro: Minimizes bespoke code at the auth/dispatch trust boundary (R-0010-c, R-0006-b), the highest-stakes place to avoid hand-rolled framing.
- Pro: In-stack (a Rust crate, no foreign runtime) and Green-tier (Apache-2.0). It clears the license gate, and that's an additive factor, not the leading one.
- Pro: The V0 transport boundary is structural. Turning `transport-io` on satisfies R-0010-a, and compiling no HTTP-transport feature satisfies R-0010-e.
- Con: A new direct dependency and an external API surface to track across versions. Mitigated by Green-tier status, the official SDK's maturity (post-1.0, 13M+ downloads), and confinement behind mnemra's own `ServerHandler`.

#### Hand-rolled MCP/JSON-RPC layer

- Con: Risks subtle non-conformance to a **mandated external** wire contract (MCP 2025-06-18, R-0010-a) that the project neither owns nor can redefine.
- Con: Maximizes bespoke code precisely at the auth/dispatch trust boundary. From a security-review standpoint, that's the worst location for hand-written framing.
- Con: `P-StackDiscipline`'s hand-roll carve-out doesn't apply. MCP/JSON-RPC isn't a "small mechanism", and the in-stack SDK is post-1.0 and mature (not pre-1.0 or thin). So the "hand-roll a small thing rather than inherit churn" exception isn't in play.
- Con: Collateral drift. It was adopted because it sat next to the Extism reversal, not on its own merits; no technical dependency coupled the two halves of the proposal.

## More Information

**The proposal being corrected (related precedent, not a superseded ADR).** An earlier internal research proposal ("ADR-001: Plugin runtime — Extism on Wasmtime, with rmcp 1.5", **Status: Proposed**, 2026-04-27) recommended a single stack: Extism + `rmcp` 1.5 + a hyper-mcp-shaped host. It was never ratified into this project's accepted P-ADR series. This ADR is **not** a `supersede`. A research proposal that was never accepted has no `accepted` status to replace, so `supersedes` stays `null` and the relationship is described here instead. The proposal's runtime half was reversed (Decision A). Its protocol-SDK half is *re-affirmed* here (Decision B) at the current `rmcp` version. Only its bundling with Extism was wrong.

**Why this is a project ADR.** Each decision meets the criterion for an ADR rather than a design note ([ADR README](README.md); judged agent-first: a reversal forces rework by downstream agents, or changes what external consumers reference). Reversing Decision A would force the pool, epoch supervisor and trap-recovery machinery to be rebuilt against a different runtime surface. Reversing Decision B would force every MCP wire and framing site to change and conformance to an external spec to be established again. Both are dependency-selection decisions of the same kind as [P-0010](P-0010-storage-substrate-engine.md) (a concrete engine behind a swappable trait) and [P-0011](P-0011-logging-facade.md) (a concrete logging crate + topology). Each is a concrete library choice under a locked contract.

**Related precedent (plugin-runtime context, not overlap).** Three earlier ADRs touch the plugin runtime: [P-0002](P-0002-core-plugin-partition.md) (core/plugin partition), [P-0003](P-0003-plugin-manifest.md) (V0 plugin manifest, including the typed host-fn allowlist compiled per instance) and [P-0007](P-0007-plugin-resource-limits.md) (plugin resource limits). But they decide *different* questions: which verbs are core, the manifest schema, and the resource-limit values. None of them decides the execution-runtime framework (Extism vs raw Wasmtime) or the MCP-SDK question. This ADR fills both gaps. P-0007 is Decision A's primary anchor, since its mechanism is *why* raw Wasmtime.

**Canon anchors:**

- `P-StackDiscipline` (workspace `architecture-principles.md`, with rules S1/S2): reject ecosystem-misaligned tooling unless no in-stack path exists. "Industry-default" is a claim about installed base, not about fit, and license tier is a separate additive gate. Applied per layer, it removes the Extism abstraction (the in-stack Wasmtime primitive covers the requirement) and admits `rmcp` (in-stack, a mandated external protocol, post-1.0, so the hand-roll carve-out doesn't apply).
- `P-PreserveDecisionSpace`: both decisions are recorded with their surfaced alternatives as the options the lock chose among, and the maintainer's lock quote is recorded verbatim.
- `P-Defer`: Decision B is held with a named tripwire that fires a revisit (a concrete `rmcp` limitation that blocks a spec requirement), not held silently.

**References:**

- Decision A, shipped (paths confirmed to resolve in the repo, under `libs/mnemra-host/plugin/`):
  - Runtime + resource limits: `libs/mnemra-host/plugin/runtime.rs`, `libs/mnemra-host/plugin/limits.rs`.
  - Epoch supervisor: `libs/mnemra-host/plugin/epoch_thread.rs`.
  - Pool + kill-and-replace: `libs/mnemra-host/plugin/pool.rs`, `libs/mnemra-host/plugin/trap_recovery.rs`.
  - Manifest + host-fn allowlist: `libs/mnemra-host/plugin/manifest.rs`, `libs/mnemra-host/plugin/allowlist.rs`.
  - Wasmtime dependency pin: `libs/mnemra-host/Cargo.toml` (`wasmtime = "=45.0.2"`, features `cranelift` + `component-model`; no `extism` dependency present). This bullet records the **shipped** feature state only. The *forward contract* for runtime features (the adoption path for the `async` feature, and the `component-model-async` off-switch) is owned by [P-0019-plugin-contract](P-0019-plugin-contract.md) D3, which is the single source for those commitments. (Cross-reference added 2026-07-03 at the P-0019 gate.)
  - Merged in `8bb2326` (Tasks 20-21) and `d378118` (Task 22).
- Decision B, **planned location**. These paths do **not** resolve in the repo yet; Task 23 creates them. They follow the crate's dominant convention for module entrypoints, established in `df2d0f0`: a `<module>.rs` file beside the `<module>/` directory, as with `auth.rs`/`auth/`, `storage.rs`/`storage/`, etc. The one `plugin/mod.rs` is a pre-existing exception, not the pattern to follow. The planned files are the MCP module entrypoint `libs/mnemra-host/mcp.rs` with `libs/mnemra-host/mcp/server.rs` (the `rmcp` `ServerHandler`, replacing the would-be hand-rolled transport and framing), `libs/mnemra-host/mcp/dispatch.rs` (the R-0010-c/d auth check and capability check, plus single `WorkspaceCtx` construction per R-0006-b) and `libs/mnemra-host/mcp/errors.rs` (the R-0010-f distinguishable JSON-RPC error codes). Task 23 adds `rmcp` to `libs/mnemra-host/Cargo.toml` (features `server`, `transport-io`; no HTTP-transport feature).
- Spec requirements: [R-0006-b](../../specs/2026-05-24-mnemra-core-v0-substrate.md), [R-0007-a…i](../../specs/2026-05-24-mnemra-core-v0-substrate.md), [R-0010-a/c/d/e/f](../../specs/2026-05-24-mnemra-core-v0-substrate.md), [R-0016](../../specs/2026-05-24-mnemra-core-v0-substrate.md), in [`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`](../../specs/2026-05-24-mnemra-core-v0-substrate.md) at content-lock git blob `9f4695f3a9c5f0906a7ce2e3848eb65bbd47834f` (the live spec on `main`).
- `rmcp` on crates.io: v1.7.0, Apache-2.0, official MCP Rust SDK (`github.com/modelcontextprotocol/rust-sdk`).

**Follow-up:** Task 23 implements Decision B (the `rmcp` `ServerHandler` MCP server). The three Task-23 plugin-runtime carries (limit-attach test, `can_invoke` invoke-path wiring, epoch-hook gating) and the Task-23 plan reshape are tracked separately and are out of scope for this ADR.

## Amendment 2026-10-25 — Crate versions are not design values (version citations only)

Applied 2026-10-25 under the maintainer's rulings on #4249. There are three: activity 8285 (design records carry no crate or library version), 8303 (confirming 8296: a crate version stays out, whether as a pin or as a floor, and a design dependency on a crate names the capability it needs) and 8380 (versioned provenance stays). The stated reason is that a version change, for example a bump forced by a security advisory, must never require reopening a locked record. This record is part of the measured case behind the ruling. It named `wasmtime = "=45.0.2"` after `libs/mnemra-host/Cargo.toml` had moved past it, with no amendment and no signal.

**Scope.** This amendment touches the crate-version citations only. It doesn't touch the MCP protocol revision this record cites (2025-06-18, R-0010-a). That's a separate amendment, and a second amendment section with the same date may follow this one.

**What each decision depends on.** These two statements supersede every version clause listed below.

- **Decision A** depends on a Wasmtime release that exposes the raw `Engine` / `Store` / `Config` surface P-0007 drives (fuel metering through `Store::set_fuel`, epoch interruption through `Store::set_epoch_deadline`, a per-instance `ResourceLimiter`), with the `cranelift` and `component-model` features. Which release meets that is an implementation fact held in `libs/mnemra-host/Cargo.toml`, not in this record.
- **Decision B** depends on the official MCP Rust SDK `rmcp` (`github.com/modelcontextprotocol/rust-sdk`) providing a `ServerHandler` server over a stdio transport (the `transport-io` feature), with no HTTP-transport feature enabled. The release in use is the `rmcp` entry in the workspace `Cargo.toml`, not a design value.

The Decision B revisit tripwire is unchanged. It was already keyed on a capability (a concrete `rmcp` limitation that blocks a spec requirement). So an `rmcp` release change that keeps the Decision B capability above doesn't fire it.

**Superseded (the text above is kept as the record):**

- the frontmatter summary's "`rmcp` v1.7.0", Decision B Option 1's "Depend on `rmcp` v1.7.0" and the Decision B outcome's "(v1.7.0)": the choice is the SDK, not a release of it;
- "the version locked here is the current 1.7.0" at the end of "`rmcp` facts": no version is locked;
- the version quoted in the Decision A reference bullet "Wasmtime dependency pin" (`wasmtime = "=45.0.2"`): the bullet still points at `libs/mnemra-host/Cargo.toml` as where the shipped pin and features live, and the quoted value is that file as recorded on 2026-06-19.

**Kept as provenance (not superseded):**

- the other "`rmcp` facts" (latest version, license, download count) and the References entry "`rmcp` on crates.io: v1.7.0": a reading of crates.io taken when this record was written, 2026-06-19;
- "rmcp 1.5" in the title and description of the unratified ADR-001 proposal (Status, Context, More Information): a historical quotation;
- "post-1.0" in the SDK-maturity argument: the maturity the decision was weighed on in June 2026.
