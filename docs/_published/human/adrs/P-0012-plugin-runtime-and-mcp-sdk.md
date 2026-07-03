---
title: "P-0012: Plugin Runtime (raw Wasmtime) + MCP SDK (`rmcp`)"
summary: "Two implementation decisions for the V0 plugin substrate that an unratified research proposal (ADR-001) had bundled into a single Extism + rmcp + hyper-mcp stack — and that the build reversed on both halves without recording either. Decision A (retroactive, shipped in Tasks 20–22): host plugins on raw Wasmtime, not Extism, because P-0007's per-instance resource-limit mechanism (fuel + epoch + ResourceLimiter) needs direct Wasmtime control that Extism's abstraction fights. Decision B (forward, implemented by Task 23): adopt the official MCP Rust SDK `rmcp` v1.7.0 (Apache-2.0, Green-tier) for the single stdio MCP server rather than hand-rolling the JSON-RPC/MCP wire layer — the MCP server is the auth+dispatch trust boundary and R-0010-a mandates conformance to MCP 2025-06-18, the worst place to hand-roll a mandated external wire contract. mnemra's R-0010 handler logic (auth-check, single WorkspaceCtx construction, per-verb capability check, distinguishable JSON-RPC error codes) rides on top as the rmcp ServerHandler, unchanged by the SDK choice."
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

The maintainer made both decisions recorded here. They're grouped together because they correct two halves of the **same** unratified proposal, the plugin substrate stack, that the build reversed on both halves without an ADR (Architecture Decision Record: a record of a decision, what it ruled out, and why).

- **Decision A (raw Wasmtime over Extism)** is recorded `accepted`-on-landing rather than `proposed`. It's already in the tree, merged in Tasks 20-22 (commits `8bb2326` "plugin runtime: manifest load, allowlist, wasmtime limits, epoch supervisor (Tasks 20-21)" and `d378118` "resource-limit trap handling: kill-and-replace recovery (Task 22)"), and it isn't waiting on a review gate. This ADR ratifies the choice after the fact, the same way [P-0011](P-0011-logging-facade.md), a project-scoped decision record, accepted the `tracing` scaffold on landing.
- **Decision B (official `rmcp` SDK over hand-rolled MCP)** is `accepted` as a forward decision. The maintainer locked the choice, and Task 23 (the MCP-server work) implements it. The lock comes before the implementation, which is why this is `accepted` and not `proposed`. There's no open question left for a review gate to resolve, only the build to follow.

This ADR does **not** `supersede` or `override` any prior record. The proposal it corrects, an earlier, unratified internal research proposal ("ADR-001: Plugin runtime, Extism on Wasmtime, with rmcp 1.5"), carries **Status: Proposed** and was never ratified into this project's accepted ADR series (P-0001…P-0011 contains no plugin-runtime or MCP-SDK ADR). A research proposal that was never accepted can't be superseded, so the relationship is narrated in Context instead, and `supersedes`/`overrides` stay `null`.

## Context and Problem Statement

The V0 plugin substrate involves two implementation choices that are easy to conflate but are **orthogonal**:

1. **The plugin execution runtime**: what hosts and sandboxes plugin WASM (Extism vs. raw Wasmtime vs. other).
2. **The MCP protocol layer**: what speaks the MCP/JSON-RPC wire contract to agent clients (official `rmcp` SDK vs. hand-rolled).

An earlier, unratified internal research proposal ("ADR-001: Plugin runtime, Extism on Wasmtime, with rmcp 1.5", **Status: Proposed**, dated 2026-04-27) bundled both choices into one recommended stack: **Extism (on Wasmtime) + `rmcp` 1.5 + a hyper-mcp-shaped host**. That proposal was never ratified into this project's accepted P-ADR series. The actual build reversed **both** halves, and neither reversal got recorded:

- The plugin runtime shipped on **raw Wasmtime**, not Extism (Tasks 20-22, on `main`).
- The MCP layer was, at the point this ADR was written, about to be **hand-rolled** instead of built on `rmcp`.

The first reversal was a sound engineering call that fell out of P-0007 (see Decision A). The second was **collateral drift, not a decision**. Dropping Extism removed the runtime half of the proposed stack, but that gave no reason to also drop `rmcp`: the SDK is the *protocol* layer, orthogonal to the *runtime* layer. The proposal's two halves were coupled only by sitting in one document, not by any technical dependency. This ADR records the runtime reversal on its own merits and corrects the MCP drift back to the official SDK, separating two decisions that were never actually coupled.

Neither decision below re-opens a settled question. The maintainer settled both, and they're recorded here per `P-PreserveDecisionSpace` (preserve the option space: the alternatives a decision was chosen among stay on record, not just the winner), not re-derived.

## Decision Drivers

- **P-StackDiscipline applied correctly to each layer.** `P-StackDiscipline` (workspace `architecture-principles.md`; "reject ecosystem-misaligned tooling unless no in-stack path exists") cuts **differently** for the two decisions, and naming which way it cuts is the spine of this ADR. For the runtime, the in-stack mechanism (raw Wasmtime's own resource knobs, already a direct dependency) covers the requirement better than the abstraction layered on top of it, so the abstraction comes out. For the MCP layer, the SDK *is* in-stack (a Rust crate, no foreign runtime), and the requirement is a **mandated external wire contract**, not a "small mechanism" the hand-roll carve-out covers, so the SDK goes in. Same principle, opposite outcomes, because the layers differ.
- **P-0007's resource-limit mechanism needs direct Wasmtime control.** The V0 per-instance limits (fuel `Store::set_fuel`, `Store::set_epoch_deadline` with a supervised host epoch-tick thread, and a 64 MiB `ResourceLimiter`) are Wasmtime `Config`/`Store`-surface mechanisms. The plugin manifest's typed host-fn allowlist is compiled per-instance. These need the raw `wasmtime::{Engine, Store, Config}` surface, which an Extism wrapper would mediate and partly hide. (Decision A.)
- **The MCP server is the auth and dispatch trust boundary.** It performs `DF-auth-check` on every request before routing, and it's the single point where a workspace identity gets established. Hand-rolling the wire and JSON-RPC framing there maximizes both the surface that needs security review and the risk of drifting from an external spec, at the worst possible location. (Decision B.)
- **R-0010-a mandates conformance to an external wire contract.** [R-0010-a](../../specs/2026-05-24-mnemra-core-v0-substrate.md) (a numbered requirement defined in the project's spec) states: "The system SHALL run a single MCP server using the MCP specification 2025-06-18 with stdio transport at V0." Conformance to a versioned external protocol is exactly what an official, conformance-tested SDK provides, and exactly what a hand-roll risks getting subtly wrong without anyone noticing. (Decision B.)
- **License tier is clear and additive, not the lead argument.** Both choices are Green-tier (Wasmtime: Apache-2.0; `rmcp`: Apache-2.0, confirmed on crates.io, see below), so neither triggers a license-gate halt. Per `P-StackDiscipline` rule S2, license tier is a *separate, additive* gate and doesn't by itself justify a dependency. The fit-for-purpose argument leads in both decisions.

## Considered Options

Two independent decisions, each with its own option set. They're recorded in one ADR because they correct the two halves of one proposal, but they're decided independently.

### Decision A — Plugin execution runtime

1. **Raw Wasmtime (chosen).** Depend directly on `wasmtime` (Component Model + Cranelift). Drive fuel, epoch-interruption, and the memory `ResourceLimiter` from the raw `Config`/`Store` surface. Own the pool, the epoch-tick supervisor, and trap recovery in mnemra-host.
2. **Extism on Wasmtime (the proposal's runtime half).** Host plugins through the Extism framework (itself Wasmtime-backed). This gains a multi-PDK plugin-authoring story and a ready host-fn registration model, at the cost of an abstraction layer sitting between mnemra-host and the Wasmtime `Config`/`Store` knobs that P-0007 drives.

### Decision B — MCP protocol layer

1. **Official `rmcp` SDK (chosen).** Depend on `rmcp` v1.7.0 (the official MCP Rust SDK). Implement mnemra's MCP server as an `rmcp` `ServerHandler`. Enable `transport-io` (stdio) and enable **no** HTTP-transport feature.
2. **Hand-rolled MCP/JSON-RPC layer.** Implement the MCP 2025-06-18 wire contract, JSON-RPC framing, and stdio transport by hand inside mnemra-host. No SDK dependency, full control of the wire layer, and full ownership of its conformance and security surface.

## Decision Outcome

### Decision A — raw Wasmtime (Option 1)

**Chosen: raw Wasmtime**, because P-0007's resource-limit mechanism is defined directly in terms of Wasmtime's `Config`/`Store` surface, and Extism's abstraction would mediate exactly the knobs P-0007 needs to drive.

[P-0007](P-0007-plugin-resource-limits.md) ("Plugin Resource Limits"; fuel 10B + epoch 5s + 64 MiB ceiling, both fuel and epoch ON at V0) locks a mechanism stated in Wasmtime-native terms: `Store::set_fuel`, `Store::set_epoch_deadline(500)` with a host thread advancing the epoch counter every 10 ms, and a per-instance `ResourceLimiter`/`static_memory_maximum_size`. The corresponding requirements [R-0007-a…i](../../specs/2026-05-24-mnemra-core-v0-substrate.md) ("enable fuel metering via `Store::set_fuel`"; "enable epoch-interruption … via `Store::set_epoch_deadline(500)` with a host epoch-tick thread"; "per-instance memory ceiling … via `Config::static_memory_maximum_size` or a `ResourceLimiter`"; "the Wasmtime version SHALL be pinned") name the raw Wasmtime API directly. [R-0016](../../specs/2026-05-24-mnemra-core-v0-substrate.md) ("Plugin pool", anchors P-0007) requires a host-owned 3-5-instance pool with synchronous kill-and-replace. The pool, the epoch supervisor, and the trap-to-recovery path are mnemra-host's own machinery, sitting directly on the Wasmtime `Store`. Inserting Extism between mnemra-host and that surface would fight the mechanism P-0007 already locked: the resource knobs, the supervised epoch thread, and the trap-recovery path all want unmediated `wasmtime::{Engine, Store, Config}` access. Dropping Extism was the sound call. This ADR ratifies it.

This is consistent with `P-StackDiscipline`'s "How it shows up": when the in-stack primitive (raw Wasmtime, already a direct dependency) covers the requirement, an abstraction layered on top of it isn't warranted. Wasmtime isn't an *added* ecosystem here. It's the runtime the project already targets. Extism would have been the addition.

### Decision B — official `rmcp` SDK (Option 1)

**Chosen: the official `rmcp` SDK (v1.7.0)**, because the MCP server is both a mandated-external-protocol surface and the auth/dispatch trust boundary. Those are the two properties that most strongly argue against hand-rolling a wire layer, and `rmcp` is the conformant, in-stack, license-clean way to satisfy both.

The maintainer's lock (recorded verbatim per `P-PreserveDecisionSpace`): *"we'll use the official sdk for now unless there becomes an issue with it."*

**Why the hand-roll was the wrong default (collateral drift):** `rmcp` is the MCP *protocol* layer; Wasmtime/Extism is the plugin *execution* runtime. Dropping Extism (Decision A) removed the runtime half of the proposed stack and gave **no** reason to drop the protocol half too. The two were coupled only by being recommended in one proposal document, not by any technical dependency. Hand-rolling the MCP layer was drift that followed the runtime reversal by proximity, not a decision made on its own merits.

**Why the SDK is right on its merits:**

1. **Mandated external wire contract calls for a conformant official SDK.** [R-0010-a](../../specs/2026-05-24-mnemra-core-v0-substrate.md) ("a single MCP server using the MCP specification 2025-06-18 with stdio transport at V0") makes MCP conformance a hard requirement against a versioned external spec. `rmcp` is the **official** MCP Rust SDK (`github.com/modelcontextprotocol/rust-sdk`), post-1.0, conformance-suite-backed, with 13M+ downloads. A hand-rolled JSON-RPC/MCP layer risks subtle non-conformance to a wire contract the project doesn't own and can't unilaterally redefine. This is exactly the case `P-StackDiscipline`'s hand-roll carve-out ("when the in-stack option is pre-1.0 or thin, hand-rolling a small mechanism often beats inheriting churn") does **not** cover: MCP/JSON-RPC at the V0 surface is neither small nor a mechanism the project controls. It's a mandated external protocol, and the in-stack SDK is post-1.0 and mature, not thin.
2. **Trust-boundary minimization.** The MCP server is where `DF-auth-check` runs before any routing ([R-0010-c](../../specs/2026-05-24-mnemra-core-v0-substrate.md): "perform `DF-auth-check` … on every incoming request before routing") and where the single `WorkspaceCtx` gets constructed after token validation ([R-0006-b](../../specs/2026-05-24-mnemra-core-v0-substrate.md): "`WorkspaceCtx` SHALL be constructed at a single location in the MCP verb dispatch path, after token validation; there SHALL be no alternative construction path"). Hand-rolling the wire and framing code at this boundary maximizes both the attack surface a security review has to cover and the room for framing bugs: the worst place to write bespoke protocol code.
3. **Transport features encode the V0 boundary precisely.** Enabling `rmcp`'s `transport-io` (stdio) satisfies R-0010-a's stdio mandate. Enabling **no** HTTP-transport feature is how [R-0010-e](../../specs/2026-05-24-mnemra-core-v0-substrate.md) ("Streamable-HTTP MCP transport SHALL NOT be activated at V0") gets satisfied structurally: the capability is simply not compiled in.
4. **mnemra's R-0010 logic doesn't change with the SDK choice; it rides on top.** The SDK supplies transport, framing, and JSON-RPC conformance. mnemra's policy lives in its `ServerHandler` implementation and is identical whether the transport is hand-rolled or `rmcp`-provided: `DF-auth-check` before routing ([R-0010-c](../../specs/2026-05-24-mnemra-core-v0-substrate.md)); single `WorkspaceCtx` construction after validation ([R-0006-b](../../specs/2026-05-24-mnemra-core-v0-substrate.md)); a per-verb capability check against the manifest's declared `verbs` before dispatch ([R-0010-d](../../specs/2026-05-24-mnemra-core-v0-substrate.md): "enforce a per-verb capability check against the plugin manifest's declared `verbs` list before dispatching"); and distinguishable JSON-RPC error codes for invalid-token vs. verb-not-found vs. parameter-invalid, never conflated ([R-0010-f](../../specs/2026-05-24-mnemra-core-v0-substrate.md)). Only the wire transport and framing change hands, from a hand-rolled implementation to `rmcp`'s `ServerHandler`, while the auth, dispatch, and error-classification logic stays mnemra's own.

**`rmcp` facts, confirmed at crates.io (not restated from the proposal):** the crate is named `rmcp`, currently at version **1.7.0**, licensed **Apache-2.0** (single, not dual), with 13,075,574 downloads. Its official repository is `github.com/modelcontextprotocol/rust-sdk`, and crates.io describes it as "Rust SDK for Model Context Protocol." Apache-2.0 is **Green-tier** (auto-proceed) under the workspace dependency-approval model, the same tier as mnemra-core's own license, so the dependency clears the license gate without a halt. (The proposal cited `rmcp` 1.5; the version locked here is the current 1.7.0.)

> **Deferral / revisit tripwire (named instrument, `P-Defer`: defer the mechanism choice until evidence forces a change).** Decision B holds *"for now unless there becomes an issue with it."* The named tripwire that fires a revisit is **a concrete `rmcp` limitation that blocks a spec requirement**: for example, an `rmcp` behaviour that can't satisfy R-0010-a's MCP-2025-06-18 conformance, or that forces a deviation from the R-0010-c/d/f handler contract. Absent such a concrete, requirement-blocking issue, `rmcp` stands. A preference for a different SDK, or for a hand-roll, is not a firing condition.

### Scope — implementation-decision record only; the spec is unchanged

This ADR records *how* the substrate is built. It doesn't alter *what* the spec requires. The locked, verified spec [`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`](../../specs/2026-05-24-mnemra-core-v0-substrate.md) (at its current content-lock, git blob `9f4695f3a9c5f0906a7ce2e3848eb65bbd47834f`, the live spec on `main`, the state this ADR's R-IDs were read from) stays locked and is **not** edited by this ADR. In particular:

- **R-0010-a is implementation-agnostic and stays UNCHANGED.** It mandates "a single MCP server using the MCP specification 2025-06-18 with stdio transport" without naming an implementation. `rmcp` is the implementation choice that satisfies it, recorded here, not a change to the requirement.
- The Task-23 plan reshape and the three Task-23 plugin-runtime carries (the limit-attach test, the `can_invoke` invoke-path wiring, the epoch-hook gating) are **plugin-runtime** concerns. They're unaffected by the `rmcp` choice and out of scope for this ADR.

### Consequences

**Good:**

- **Decision A:** P-0007's resource-limit mechanism (fuel, epoch, and `ResourceLimiter`) runs directly on the raw Wasmtime surface, with no abstraction mediating the knobs the spec names. The pool, epoch supervisor, and trap-recovery path are mnemra-host's own machinery, already shipped and exercised in Tasks 20-22.
- **Decision B:** MCP-2025-06-18 wire conformance comes from the official, conformance-tested SDK instead of from bespoke code the project would have to keep conformant by hand. The auth/dispatch trust boundary carries the minimum bespoke surface: only mnemra's `ServerHandler` policy logic, not the JSON-RPC framing.
- **Both:** the substrate now has a recorded, canon-anchored decision for each of the two layers the proposal had bundled. The drift (an unrecorded reversal on each half) is corrected, and a future reader can see *which* layer each decision governs and *why*.
- The V0 no-HTTP-transport boundary (R-0010-e) is enforced structurally, by not compiling the `rmcp` HTTP-transport feature, rather than by a runtime guard.

**Bad / Trade-offs:**

- **Decision A:** raw Wasmtime gives up Extism's ready-made multi-language plugin-authoring story and its host-fn registration ergonomics. mnemra-host owns the pool, the epoch supervisor, and trap recovery itself (the machinery already built in Tasks 20-22). Accepted, because at V0 the plugins are `core: true`, well-behaved, first-party artifacts, and P-0007's direct-control requirement outweighs the authoring-DX convenience.
- **Decision B:** `rmcp` is a new direct dependency and a versioned external API surface to track. Mitigated: it's Green-tier (Apache-2.0), it's the official SDK for a protocol the project has to speak anyway, and it's confined to the MCP transport/framing seam behind mnemra's own `ServerHandler`. A version bump touches the transport seam, not the auth, dispatch, or error logic.
- Recording two decisions in one ADR risks a future reader treating them as coupled. Mitigated explicitly: the ADR's whole point is that the runtime layer and the protocol layer are **orthogonal**, and each decision is decided on its own option set and its own drivers.

## Pros and Cons of the Options

### Decision A — Runtime

#### Raw Wasmtime (chosen)

- Pro: Direct access to the `Config`/`Store` surface that P-0007's fuel/epoch/`ResourceLimiter` mechanism is defined in terms of. No abstraction mediates the locked knobs.
- Pro: Wasmtime is already the project's runtime, so there's no added ecosystem. `P-StackDiscipline`-aligned: the in-stack primitive covers the requirement.
- Pro: Already shipped and exercised (Tasks 20-22). Pool, epoch supervisor, trap-recovery, and kill-and-replace are in the tree.
- Con: No Extism-provided multi-PDK plugin-authoring story. Host-fn registration and pool machinery are mnemra-host's to own. Accepted for V0's first-party `core: true` plugins.

#### Extism on Wasmtime

- Pro: Ready multi-language plugin-authoring (multiple PDKs) and a packaged host-fn registration model.
- Con: An abstraction layer sits between mnemra-host and the Wasmtime `Config`/`Store` knobs that P-0007 (R-0007-a…i) drives directly. It mediates exactly the surface the locked resource-limit mechanism needs.
- Con: Adds the Extism framework as the runtime layer when the underlying Wasmtime (already a direct dependency) covers the V0 requirement on its own.

### Decision B — MCP protocol layer

#### Official `rmcp` SDK (chosen)

- Pro: Conformant to MCP 2025-06-18 (R-0010-a) straight out of the official, conformance-suite-backed SDK. No hand-maintained wire conformance to keep up.
- Pro: Minimizes bespoke code at the auth/dispatch trust boundary (R-0010-c, R-0006-b), the highest-stakes place to avoid hand-rolled framing.
- Pro: In-stack (a Rust crate, no foreign runtime) and Green-tier (Apache-2.0). Clears the license gate as an additive factor, not the leading one.
- Pro: The V0 transport boundary is structural. `transport-io` on satisfies R-0010-a; no HTTP-transport feature compiled satisfies R-0010-e.
- Con: A new direct dependency and an external API surface to track across versions. Mitigated by Green-tier status, the official SDK's maturity (post-1.0, 13M+ downloads), and confinement behind mnemra's own `ServerHandler`.

#### Hand-rolled MCP/JSON-RPC layer

- Con: Risks subtle non-conformance to a **mandated external** wire contract (MCP 2025-06-18, R-0010-a) that the project neither owns nor can redefine.
- Con: Maximizes bespoke code at precisely the auth/dispatch trust boundary, the worst location for hand-written framing from a security-review standpoint.
- Con: `P-StackDiscipline`'s hand-roll carve-out doesn't apply. MCP/JSON-RPC isn't a "small mechanism," and the in-stack SDK is post-1.0 and mature (not pre-1.0 or thin), so the "hand-roll a small thing rather than inherit churn" exception doesn't fire.
- Con: Collateral drift. It was adopted by proximity to the Extism reversal, not on its own merits; nothing technical coupled the two halves of the proposal.

## More Information

**The proposal being corrected (related precedent, not a superseded ADR).** An earlier, unratified internal research proposal ("ADR-001: Plugin runtime, Extism on Wasmtime, with rmcp 1.5", **Status: Proposed**, 2026-04-27) recommended a single stack: Extism + `rmcp` 1.5 + a hyper-mcp-shaped host. It was never ratified into this project's accepted P-ADR series. This ADR is **not** a `supersede`. A never-accepted research proposal has no `accepted` status to replace, so `supersedes` stays `null` and the relationship is narrated here instead. The proposal's runtime half was reversed (Decision A). Its protocol-SDK half is *re-affirmed* here (Decision B) at the current `rmcp` version. Only its bundling with Extism was wrong.

**Why this is a project ADR.** Each decision meets the ADR-vs-design-note criterion ([ADR README](README.md); agent-first: a reversal forces downstream agent rework or external-consumer reference). Reversing Decision A would force the pool, epoch supervisor, and trap-recovery machinery to be rebuilt against a different runtime surface. Reversing Decision B would force every MCP wire and framing site to change and re-establish conformance to an external spec. Both are dependency-selection decisions of the same kind as [P-0010](P-0010-storage-substrate-engine.md) (a concrete engine behind a swappable trait) and [P-0011](P-0011-logging-facade.md) (a concrete logging crate plus topology): concrete library choices under a locked contract.

**Related precedent (plugin-runtime context, not overlap).** [P-0002](P-0002-core-plugin-partition.md) (core/plugin partition), [P-0003](P-0003-plugin-manifest.md) (V0 plugin manifest: the typed host-fn allowlist compiled per-instance), and [P-0007](P-0007-plugin-resource-limits.md) (plugin resource limits) all touch the plugin runtime but decide *different* questions (which verbs are core, the manifest schema, the resource-limit values). None of them decides the execution-runtime framework (Extism vs. raw Wasmtime) or the MCP-SDK question. This ADR fills both gaps. P-0007 is Decision A's primary anchor: its mechanism is *why* raw Wasmtime.

**Canon anchors:**

- `P-StackDiscipline` (workspace `architecture-principles.md`, with rules S1/S2): reject ecosystem-misaligned tooling unless no in-stack path exists. "Industry-default" is an installed-base claim, not a fit claim, and license tier is a separate, additive gate. Applied per layer, it removes the Extism abstraction (the in-stack Wasmtime primitive covers the requirement) and admits `rmcp` (in-stack, mandated external protocol, post-1.0: the hand-roll carve-out doesn't apply).
- `P-PreserveDecisionSpace`: both decisions are recorded with their surfaced alternatives as the options the lock chose among. The maintainer's lock quote is recorded verbatim.
- `P-Defer`: Decision B is held with a named firing tripwire (a concrete `rmcp` limitation that blocks a spec requirement), not silently.

**References:**

- Decision A, shipped (paths confirmed to resolve in-repo, under `libs/mnemra-host/plugin/`):
  - Runtime and resource limits: `libs/mnemra-host/plugin/runtime.rs`, `libs/mnemra-host/plugin/limits.rs`.
  - Epoch supervisor: `libs/mnemra-host/plugin/epoch_thread.rs`.
  - Pool and kill-and-replace: `libs/mnemra-host/plugin/pool.rs`, `libs/mnemra-host/plugin/trap_recovery.rs`.
  - Manifest and host-fn allowlist: `libs/mnemra-host/plugin/manifest.rs`, `libs/mnemra-host/plugin/allowlist.rs`.
  - Wasmtime dependency pin: `libs/mnemra-host/Cargo.toml` (`wasmtime = "=45.0.2"`, features `cranelift` and `component-model`; no `extism` dependency present). This bullet records the **shipped** feature state only. The runtime-feature *forward contract* (the `async` feature's adoption path and the `component-model-async` off-switch) is owned by [P-0019-plugin-contract](P-0019-plugin-contract.md) D3, the single source for those commitments. (Cross-reference added 2026-07-03 at the P-0019 gate.)
  - Merged in `8bb2326` (Tasks 20-21) and `d378118` (Task 22).
- Decision B, **planned location** (these paths do **not** resolve in-repo yet; Task 23 creates them, following the crate's dominant sibling-file module-entrypoint convention established in `df2d0f0`: `<module>.rs` beside the `<module>/` directory, as in `auth.rs`/`auth/`, `storage.rs`/`storage/`, and so on; the one `plugin/mod.rs` is a pre-existing exception, not the pattern to follow): the MCP module entrypoint `libs/mnemra-host/mcp.rs` with `libs/mnemra-host/mcp/server.rs` (the `rmcp` `ServerHandler` replacing the would-be hand-rolled transport/framing), `libs/mnemra-host/mcp/dispatch.rs` (R-0010-c/d auth-check, capability check, and single `WorkspaceCtx` construction per R-0006-b), and `libs/mnemra-host/mcp/errors.rs` (R-0010-f distinguishable JSON-RPC error codes). `rmcp` is added to `libs/mnemra-host/Cargo.toml` (features `server`, `transport-io`; no HTTP-transport feature) by Task 23.
- Spec requirements: [R-0006-b](../../specs/2026-05-24-mnemra-core-v0-substrate.md), [R-0007-a…i](../../specs/2026-05-24-mnemra-core-v0-substrate.md), [R-0010-a/c/d/e/f](../../specs/2026-05-24-mnemra-core-v0-substrate.md), [R-0016](../../specs/2026-05-24-mnemra-core-v0-substrate.md), in [`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`](../../specs/2026-05-24-mnemra-core-v0-substrate.md) at content-lock git blob `9f4695f3a9c5f0906a7ce2e3848eb65bbd47834f` (the live spec on `main`).
- `rmcp` on crates.io: v1.7.0, Apache-2.0, official MCP Rust SDK (`github.com/modelcontextprotocol/rust-sdk`).

**Follow-up:** Task 23 implements Decision B (the `rmcp` `ServerHandler` MCP server). The three Task-23 plugin-runtime carries (the limit-attach test, the `can_invoke` invoke-path wiring, and the epoch-hook gating) and the Task-23 plan reshape are tracked separately and are out of scope for this ADR.
