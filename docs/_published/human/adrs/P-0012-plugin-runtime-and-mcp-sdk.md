---
title: "P-0012: Plugin Runtime (raw Wasmtime) + MCP SDK (`rmcp`)"
summary: "Two implementation decisions for the V0 plugin substrate that an unratified research proposal (ADR-001) had bundled into a single Extism + rmcp + hyper-mcp stack — and that the build reversed on both halves without recording either. Decision A (retroactive, shipped in Tasks 20–22): host plugins on raw Wasmtime, not Extism, because P-0007's per-instance resource-limit mechanism (fuel + epoch + ResourceLimiter) needs direct Wasmtime control that Extism's abstraction fights. Decision B (forward, implemented by Task 23): adopt the official MCP Rust SDK `rmcp` v1.7.0 (Apache-2.0, Green-tier) (Amendment 2026-10-25, #4249: no crate version is a design value; Decision B is the official SDK with a stdio server transport and no HTTP transport, and Decision A is raw Wasmtime with the surface P-0007 drives, at whatever releases `Cargo.toml` pins) for the single stdio MCP server rather than hand-rolling the JSON-RPC/MCP wire layer — the MCP server is the auth+dispatch trust boundary and R-0010-a mandates conformance to MCP 2025-06-18 (Amendment 2026-10-25, MCP protocol revision, DRAFT pending approval: the target is MCP 2025-11-25; the host serves exactly the released 2025-06-18 and 2025-11-25 and answers any other revision requested at `initialize` with 2025-11-25. The host enforces that set itself; tests pin it at `initialize` and before the handshake, and the pin after the handshake is #4326 (open)), the worst place to hand-roll a mandated external wire contract. mnemra's R-0010 handler logic (auth-check, single WorkspaceCtx construction, per-verb capability check, distinguishable JSON-RPC error codes) rides on top as the rmcp ServerHandler, unchanged by the SDK choice (Amendment 2026-10-25, MCP protocol revision, DRAFT pending approval: the host also carries an initialize gate, a line-capped reader and an `initialize` override at that boundary, beside its ServerHandler)."
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

## Amendment 2026-10-25 (MCP protocol revision)

**Status: DRAFT, pending the maintainer's approval.** The maintainer ruled on #4249 (activity 8421, an entry on that tracker task) that this amendment would be drafted only if a measured run showed two things: the host negotiates MCP 2025-11-25, and it still serves 2025-06-18 clients. Once drafted, it waits for his approval. The measurement (activity 8589) showed both, on the host as landed at `de62732` (#4295, #4294).

**Scope.** This amendment changes the MCP protocol revision this record cites. It also corrects this record's claims about how little bespoke code sits at the MCP boundary (see Superseded, below). It doesn't touch the crate-version citations, which the amendment above handles. And it doesn't change Decision A or Decision B: the SDK choice stands. Protocol revisions stay in design records, because rulings 8421 and 8435 on #4249 both direct this record to name one. What the #4249 sweep takes out of design records is crate versions and runtime pins.

**Rulings applied.**

- Activity 8421: the condition above.
- Activity 8435 (the maintainer, about 17:35): the record names a pinned set of released revisions, and the host enforces it, so an SDK upgrade can't silently widen it. Its wording also says the set "refuses drafts".
- The `initialize` shape. This is INHERITED (taken from a record and not re-checked) from two sources: #4249 activity 8454, which relays the implementation lane's session (the lane that owns code and tests) and says it wasn't re-verified, and #4294's task description. At about 17:37, two minutes after 8435, the maintainer chose "Answer 2025-11-25". That's the spec-conformant downgrade, not an `initialize` error. It reads as a later refinement of 8435 that applies to `initialize`. On that reading, 8435's "refuses drafts" still holds for a request other than `initialize` or `ping` that names a draft in complete `_meta` of its own. Such a request gets no result (see below). ⚠ The approval should confirm that reading. This record follows the code.

**Decision: the conformance target is MCP 2025-11-25, and the host serves exactly the released revisions {2025-06-18, 2025-11-25}.** R-0010-a's target moves from 2025-06-18 to 2025-11-25. It's the newest released revision and the one the host answers by default. 2025-06-18 stays served so a client pinned to it keeps working. *(Anchors: the maintainer's rulings 8421 and 8435; `P-GuaranteeByMechanism`, "enforce a guarantee with a mechanism that fails loudly when violated": the set is a host constant, pinned by tests on the paths named below, not the SDK's default.)*

- An `initialize` requesting 2025-06-18 is answered with 2025-06-18.
- An `initialize` requesting 2025-11-25 is answered with 2025-11-25.
- An `initialize` requesting any other revision is answered with 2025-11-25. That covers a draft such as 2026-07-28, an older released revision such as 2025-03-26, and an unknown string such as 2024-01-01. The answer is a downgrade, not an `initialize` error.
- The server's default revision is 2025-11-25.
- A request other than `initialize` whose own `_meta` names a revision outside the set gets `-32022` (unsupported protocol version), and the error data lists the set. Before the handshake there are three exceptions. A `ping` is answered whatever its `_meta`. A request with incomplete `_meta` gets `-32602`. And a `server/discover` gets `-32600` from the initialize gate (see the lifecycle below). After the handshake, every such request gets `-32022`, `ping` included. That answer comes from the SDK's per-request check, and no test pins that path (see below). An `initialize`'s own `_meta` isn't consulted: its revision is the one it requests, as above.
- A session negotiated at 2025-11-25 carries none of the 2026-07-28 draft's result fields (`resultType`, `ttlMs`, `cacheScope`). That holds even when the client asked for the draft.

**Enforcement: the host owns the set.** MEASURED (observed directly) at `de62732` by reading the code; the host line numbers are from that commit. The `rmcp` line numbers come from a source read of rmcp 3.7.0, the release `Cargo.lock` pins at `de62732`.

- `SUPPORTED_PROTOCOL_VERSIONS` in `libs/mnemra-host/mcp/server.rs` (`:77-91`) holds the two revisions.
- `MnemraMcpServer`'s `ServerHandler::supported_protocol_versions` returns that constant (`:555-558`).
- `get_info` names 2025-11-25 explicitly (`:522-529`) rather than inheriting the SDK's default. That default is the SDK's newest known revision, draft included.
- `MnemraMcpServer`'s `ServerHandler::initialize` (`:543-553`) stores the negotiated revision as the session's, not the one the client requested.

**What the tests pin.** On two paths, an SDK release that learns more revisions doesn't change what's served. Tests in `libs/mnemra-host/tests/mcp_server.rs` turn red if it does.

- At `initialize`:
  - `initialize_released_2025_06_18_is_echoed` (`:3087`);
  - `initialize_released_2025_11_25_is_echoed` (`:3095`);
  - `initialize_draft_2026_07_28_is_downgraded_to_2025_11_25` (`:3103`);
  - `initialize_unknown_version_is_downgraded_to_2025_11_25` (`:3111`);
  - `initialize_older_known_version_is_downgraded_to_2025_11_25` (`:3119`);
  - `session_replies_carry_no_draft_result_fields` (`:3407`);
  - `handshake_requesting_the_draft_version_is_served_as_released` (`:3443`). It pins an outcome: a session whose handshake asked for the draft is served as 2025-11-25. Two things produce that outcome, and they're co-equal. The host's `initialize` override stores the negotiated revision. And the SDK's handshake sets it again after the handler returns (rmcp 3.7.0 `src/service/server.rs:692-703`). Either one alone keeps the test green, so on the handshake path neither is pinned by itself. The host override alone is pinned on a re-initialize that reaches an ungated server (`handler_without_the_gate_stores_the_negotiated_version`, `:3503`).
- Before the handshake, on a request's own `_meta`: `pre_init_request_with_draft_inline_meta_does_not_start_stateless_lifecycle` (`:3264`), plus the gate tests in the lifecycle below.

**Not pinned: a request's own `_meta` after the handshake.** Once the handshake is done, the initialize gate passes every request through (`libs/mnemra-host/mcp/gate.rs:85`). The SDK takes a request's protocol revision from that request's own `_meta` before the session's (rmcp 3.7.0 `src/service.rs:1220-1226`). So on an established session, a request whose `_meta` names the draft is stopped only by the SDK's per-request check against `supported_protocol_versions` (rmcp 3.7.0 `src/handler/server.rs:64-72`), which answers `-32022`. No test at `de62732` sends a request's own `_meta` after the handshake. An SDK release that narrowed that check would leave every test green while the served set widened. The test that pins this path is filed as #4326 (implementation lane, open). **This record's claim that an SDK upgrade can't widen the set covers the two pinned paths above only.** It widens to this path when #4326's test lands. Until then #4326 is parked: nothing fires it, and no cadence is named here. The approval should either land #4326 first or name the cadence that brings it back up.

**Lifecycle before the handshake.** This is how the host behaves at `de62732`. The invariant is that **before the `initialize` handshake completes, no request other than `initialize` or `ping` receives a result.** Two layers produce it. The SDK's pre-initialize loop (rmcp 3.7.0 `src/service/server.rs:563-672`) answers some requests itself. The initialize gate `InitializeGated` (`libs/mnemra-host/mcp/gate.rs:63-88`) sees every request the SDK hands to the service. The gate is an allowlist and fails closed. So if a later SDK release adds a request kind and hands it to the service, it's refused rather than served. Production starts every session through `libs/mnemra-host/mcp/gate.rs`'s `serve` (`:41-61`; called at `libs/mnemra-host/mnemra_host.rs:405`).

Each outcome, with the code that produces it:

- `ping` is answered with an empty result by the SDK's loop, whatever its `_meta` (`src/service/server.rs:566-581`). An `initialize` then negotiates normally (`pre_init_ping_is_answered_and_initialize_follows`, `mcp_server.rs:3374`).
- A request other than `initialize` or `ping` whose `_meta` is missing or incomplete gets `-32602` from the SDK's loop (`src/service/server.rs:588-607`), and the connection stays before the handshake. This is pinned for `server/discover` (`pre_init_discover_without_meta_is_invalid_params`, `:3282`). For other request kinds it's a source read, untested; #4326 carries that case as optional.
- A `server/discover` with complete `_meta` gets `-32600`, message `initialize required`, from the gate (`gate.rs:82-84`), whichever revision it names. The SDK hands a discover to the service as a bootstrap probe before its own version check (`src/service/server.rs:610-632`). The connection stays before the handshake, and an `initialize` then negotiates normally (`pre_init_discover_naming_draft_version_is_refused`, `:3241`; `pre_init_discover_naming_released_version_is_refused`, `:3356`).
- A request other than `server/discover` that has complete `_meta` naming a revision outside the set, and that reaches the SDK's version check, gets `-32022` (unsupported protocol version) from the SDK (`src/service/server.rs:634-651`). The error data lists the supported set, and the connection stays before the handshake (`pre_init_request_with_draft_inline_meta_does_not_start_stateless_lifecycle`, `:3264`; assertion `:3222-3238`).
- A request other than `server/discover` that has complete `_meta` naming a revision in the set latches the SDK's inline mode for the connection (`src/service/server.rs:653-664`). The gate refuses it with `-32600` `initialize required` (`gate.rs:82-84`). From then on, on that connection:
  - a later `initialize` gets `-32600` `initialize required` (`gate.rs:73-75`);
  - any other request except `ping` gets the same (`gate.rs:82-84`);
  - a `ping`, which the gate allows, gets `-32602` from the SDK's per-request metadata check (`src/handler/server.rs:78-99`).

  The connection stays refused until it's reopened (`pre_init_request_with_released_inline_meta_is_refused_and_so_is_a_late_initialize`, `:3328`; against the real binary, `production_binary_refuses_a_request_before_initialize`, `libs/mnemra-host/tests/smoke_e2e.rs:402`).
- A second `initialize` on an established session is refused with `-32600`, message `already initialized`. It doesn't change how the session is served (`gate.rs:76-80`; `re_initialize_is_refused_and_does_not_enable_draft_shaping`, `:3470`).
- An incoming line longer than `MAX_INCOMING_LINE_BYTES` (`libs/mnemra-host/mcp/transport.rs:27`, 16 MiB at `de62732`, newline included) ends the session. The client sees end of stream. The line isn't dropped with the client left waiting (`transport.rs:1-11`). The tests are `line_one_byte_over_cap_closes_the_stream` (`mcp_server.rs:3637`), `over_cap_line_with_no_newline_closes_the_stream` (`:3649`), `over_cap_line_after_initialize_closes_the_stream` (`:3661`) and `production_binary_ends_the_session_on_an_over_cap_incoming_line` (`smoke_e2e.rs:315`). A line exactly at the cap is served (`line_exactly_at_cap_is_served`, `:3611`).

**Decision: the lifecycle refusals stand as implemented, and they don't conflict with R-0010-f.** R-0010-f has a closed list of classes that must stay distinguishable: invalid token, verb not found and parameter invalid. The refusals above use three codes.

- `-32600`, the gate's lifecycle refusals. The invalid-token code is pinned to differ from `-32600` (`bogus_token_returns_distinguishable_auth_failure`, `mcp_server.rs:457-461`), so a lifecycle refusal can't be read as an authentication failure. The implementation lane set the `-32600` code as a default that can be revisited, and routed it here for this check.
- `-32602`, from the SDK before the handshake and on a latched connection. Each one refuses a request that's missing the `_meta` the SDK requires, which is a parameter fault. So it falls inside the parameter-invalid class rather than blurring two classes.
- `-32022`, from the SDK for a revision outside the set. It's a code of its own. It differs from the invalid-token code (`-4001`, `libs/mnemra-host/mcp/errors.rs:29`), from the code for a verb outside the manifest (`-4005`, `errors.rs:70`) and from `-32602`. Two codes for neighbouring faults isn't conflation. One code for two classes would be.

*(Anchor: R-0010-f's clause text in [`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`](../../specs/2026-05-24-mnemra-core-v0-substrate.md), "error code classes SHALL NOT be conflated", over its three named classes.)*

**Not claimed: a parse-error reply.** This record doesn't claim that the host answers a line that isn't valid JSON with `-32700` (parse error), and a reader mustn't rely on one. Here's what happens on the SDK release landed at `de62732` (rmcp 3.7.0), by source read and untested:

- A line that isn't valid JSON gets no reply. The SDK drops it, logs it at `debug`, and the stream continues (`src/transport/async_rw.rs:290-299`).
- A line of valid JSON that isn't a well-formed message gets `-32600`, message `Invalid request`, with `id: null`, and the stream continues (`async_rw.rs:300-314`).
- Only a line over the cap ends the session (above).

The host's default log filter is `info` (`cmd/mnemra/logging.rs:32`, MEASURED at `de62732`), so a dropped line leaves no trace in the host's log. INHERITED from the #4156 review (activity 8423): the release the host used before the 2.x line (1.7.0) replied `-32700` with a null id. No test at `de62732` pins any of this. A search of `libs/mnemra-host/tests` for `32700`, `PARSE_ERROR`, `parse_error` and `Ignoring unparsable` found nothing, while the same search for `INVALID_PARAMS` did find hits. The mechanism that closes that gap is the characterization test #4323 adds. It pins today's no-reply and `-32600` behaviour, so an SDK release that changes either one turns a test red. #4323 is filed and open (implementation lane), and parked like #4326: nothing fires it, and no cadence is named here.

The threat-model row `P-mcp-handler` (tampering) in `docs/src/architecture/overview.md` (`:628`) lists "framing-error fail-shut" and a structured `parse_error` reply as mitigations. Neither holds at `de62732`. Only an over-cap line fails shut. A line that isn't valid JSON is dropped, a wrong-shape message gets `-32600`, and the stream continues after both. #4322 corrects that row (see Follow-ups).

**Decision: Decision B's revisit tripwire doesn't fire on the host's overrides of SDK behaviour. It doesn't fire on the parse-error change either, unless the open question below resolves that conformance requires a `-32700` reply.** The tripwire fires on "a concrete `rmcp` limitation that blocks a spec requirement".

1. No requirement in the V0 spec names a parse-error reply. A parse error isn't one of R-0010-f's three classes, and R-0010-c and R-0010-d are untouched. The one route by which a requirement could call for the reply is R-0010-a's conformance target, if MCP 2025-11-25 or JSON-RPC 2.0 requires it. That's the open question below.
2. INFERRED (reasoned, not observed), not tested: even if the reply is required, the host could send it from its own wrapper around the SDK `Transport`, so the SDK wouldn't block the requirement. That's not the seam the host already holds. `LineCappedReader` (`transport.rs:34-97`) wraps the read side, below the SDK's parser, and can't see a parse result or write a reply. A `-32700` reply needs a host wrapper that sees each parse failure and can write to the stream. Whether such a wrapper works hasn't been measured.
3. The host already overrides three SDK behaviours at the MCP boundary: the initialize gate `InitializeGated` (`gate.rs:35-105`), the reader `LineCappedReader` (`transport.rs:34-97`) and the `initialize` override (`server.rs:543-553`). Each one replaced, in host code, an SDK default the host doesn't accept. So none blocked a requirement, and the tripwire's text fired on none of them. They do falsify this record's claim about the bespoke surface at the boundary (see Superseded, below). The tripwire counts limitations that block a requirement, not overrides. Whether a count of overrides should become a revisit condition is the maintainer's call, and it isn't decided here.

If the open question resolves that the reply is required, and the wrapper in point 2 proves infeasible, the tripwire fires. *(Anchor: Decision B's tripwire as written above, "Absent such a concrete, requirement-blocking issue, `rmcp` stands".)*

**Open question, not decided here: must a line that isn't valid JSON get a `-32700` reply?** MCP 2025-11-25 carries its messages as JSON-RPC 2.0. Whether either specification requires the reply isn't established in this record. Neither text was read for this amendment, and the design lane (the lane that owns specs and ADRs) has no web access. The in-repo attempts didn't settle it. The SDK's error-code definitions cite JSON-RPC 2.0 for the code values (rmcp 3.7.0 `src/model.rs:624-643`), but they say nothing about when a reply is due. A search of the project's research notes found no reading of either specification. INHERITED from Warden's recollection during the review of this amendment, not read: JSON-RPC 2.0 answers invalid JSON with `-32700` and `id: null`, and a server must reply to every call except a notification.

The options:

- (a) Send no reply, which is the SDK's behaviour. The SDK states that this matches the other official MCP SDKs, which ignore unparsable input (`async_rw.rs:293-296`; INHERITED, not checked against those SDKs). The #4156 review recommended it, to avoid reply storms. It costs conformance if JSON-RPC requires the reply. And the drop is silent today.
- (b) Reply `-32700` with `id: null` from a host wrapper around the SDK `Transport` (point 2 above). The reply carries a constant message and never echoes the line or the parser's detail. It buys conformance if the reply is required, and it keeps the session open.
- (c) End the session, as for an over-cap line. Of the three, only this one frees a client waiting on a lost request, because the client sees end of stream. It costs the session for one bad line, and it costs conformance too if a reply is required.

Security findings, INHERITED from Warden's review of this amendment (2026-10-25), with one part MEASURED here:

- There's no amplification on stdio. The reply-storm risk behind (a) assumes a peer that echoes invalid data back.
- Under (a) or (b), the client that sent the line hangs either way. Invalid JSON carries no id, so no reply can be matched to its pending request.
- (b) gives no oracle, as long as the reply has a constant message and never echoes the line or the parser's detail.
- Under (a) as shipped, a dropped line is a silent failure. It's logged only at `debug`, and the host's default filter is `info`; both facts are MEASURED above.

The values in conflict come from the workspace's architecture values. V-Consistency (the same problem gets the same shape) points three ways. The internal precedent is #4249 activity 8460: an over-cap line must fail visibly, because a silent drop hangs the sender. That favours a visible outcome, though its hang reason transfers only to (c). External convention, as the SDK reports it, favours (a). JSON-RPC 2.0, as recalled, favours (b). V-Observability (a surface that stops reporting silently is a defect) counts against (a) as shipped. But that part is separable from the wire choice: the host can make a dropped line visible in its own log under any option. V-Simplicity (the smallest mechanism that solves the problem) favours (a), which adds no mechanism. Core values trade off here, so this record doesn't lock it.

**Recommendation (DaVinci's, the author's; not a ruling):**

1. Read JSON-RPC 2.0 and MCP 2025-11-25 on this point before ruling. That's a research input the design lane can't fetch.
2. If JSON-RPC's reply rule holds as recalled, take (b). R-0010-a makes conformance the requirement, Warden's findings remove the security reason for (a) on stdio, and (b) keeps the session.
3. If the rule doesn't hold, take (a).
4. Under either, make a dropped line visible in the host's log at the default filter. A failure that leaves no signal is a defect on first sighting (`P-GuaranteeByMechanism`, the silent-failure clause; V-Observability, "stops reporting silently").
5. If (b) is chosen, #4323's test pins `-32700` in place of no reply.

**Superseded (the text above is kept as the record):**

- the frontmatter summary's "R-0010-a mandates conformance to MCP 2025-06-18";
- the Decision Drivers bullet "R-0010-a mandates conformance to an external wire contract", which quotes R-0010-a with 2025-06-18;
- item 1 of "Why the SDK is right on its merits", for the same quotation;
- the Decision B tripwire's example "R-0010-a's MCP-2025-06-18 conformance", which now reads 2025-11-25; the tripwire is otherwise unchanged;
- in Scope, "R-0010-a is implementation-agnostic and stays UNCHANGED" and its quotation. R-0010-a's target changes. This record doesn't edit the spec, and the V0 spec's text still reads 2025-06-18 until that spec's own amendment is made (see Follow-ups). Until then, this section is where the move is recorded;
- the Consequences line "MCP-2025-06-18 wire conformance comes from the official, conformance-tested SDK";
- the official-SDK Pro "Conformance to MCP 2025-06-18 (R-0010-a)";
- the Consequences line's closing sentence, "The auth/dispatch trust boundary carries the minimum bespoke surface: only mnemra's `ServerHandler` policy logic, not the JSON-RPC framing." At `de62732` the host also carries three things at that boundary: the initialize gate, a reader on the framing path and an `initialize` override (point 3 above). Each is small and pinned by tests, but "only … policy logic" no longer holds;
- the closing sentences of item 4 in "Why the SDK is right on its merits", "Only the wire transport and framing change hands …", for the same reason.

**Kept (not superseded):**

- Decision B Option 2 ("Implement the MCP 2025-06-18 wire contract …") and the hand-roll Con that cites MCP 2025-06-18. They describe the rejected option as it was weighed on 2026-06-19.
- the first 2026-10-25 amendment's Scope line naming 2025-06-18. It states what that amendment left alone.
- the frontmatter summary's "mnemra's R-0010 handler logic … rides on top as the rmcp ServerHandler, unchanged by the SDK choice". It's still true: the R-0010 logic it lists is unchanged and is still the host's `ServerHandler`. It carries a note because it can be read as the whole of the host's code at the boundary, and the three overrides above sit beside it. The ADR index row's matching phrase carries the same note.

**Follow-ups: PARKED, not deferred.** The records below cite 2025-06-18 as the target, or make a claim this amendment corrects. They're amended by #4322, the second phase of #4249. That work is blocked on #4119 (PR #10), because that open change holds the first two files. Nothing detects PR #10's merge. Instead, the design lane checks PR #10's state at each session start, and #4249 stays open until #4322 is done.

- `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`: R-0010-a; the Protocol line of the "MCP transport" contract; the Constraints line "Must use MCP specification 2025-06-18".
- `docs/src/architecture/overview.md` (line numbers as they stand before #4119 lands): the hard-constraints table row that names MCP specification 2025-06-18 (`:62`), and the `P-mcp-handler` tampering row (`:628`), for its structured `parse_error` reply, its "framing-error fail-shut", and its "explicit MCP 2025-06-18 input validation".
- `docs/src/adrs/P-0013-plugin-invocation-model.md` (`:100`): its present-tense gloss of R-0010-a, "single MCP server, 2025-06-18, stdio". #4119 doesn't hold that file, but the sentence describes what the spec currently pins. It stays true until R-0010-a is amended and goes stale at that moment, so it's amended together with R-0010-a.
