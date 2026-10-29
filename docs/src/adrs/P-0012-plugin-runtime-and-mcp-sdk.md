---
title: "P-0012: Plugin Runtime (raw Wasmtime) + MCP SDK (`rmcp`)"
summary: "Two implementation decisions for the V0 plugin substrate that an unratified research proposal (ADR-001) had bundled into a single Extism + rmcp + hyper-mcp stack — and that the build reversed on both halves without recording either. Decision A (retroactive, shipped in Tasks 20–22): host plugins on raw Wasmtime, not Extism, because P-0007's per-instance resource-limit mechanism (fuel + epoch + ResourceLimiter) needs direct Wasmtime control that Extism's abstraction fights. Decision B (forward, implemented by Task 23): adopt the official MCP Rust SDK `rmcp` v1.7.0 (Apache-2.0, Green-tier) (Amendment 2026-10-25, #4249: no crate version is a design value; Decision B is the official SDK with a stdio server transport and no HTTP transport, and Decision A is raw Wasmtime with the surface P-0007 drives, at whatever releases `Cargo.toml` pins) for the single stdio MCP server rather than hand-rolling the JSON-RPC/MCP wire layer — the MCP server is the auth+dispatch trust boundary and R-0010-a mandates conformance to MCP 2025-06-18 (Amendment 2026-10-25, MCP protocol revision, approved 2026-10-29 (#4249 activity 9183): the target is MCP 2025-11-25; the host never speaks a draft revision, serves exactly the released 2025-06-18 and 2025-11-25, and answers any other revision requested at `initialize` with 2025-11-25. The host enforces that set itself; tests pin it at `initialize` and before the handshake, and the pin after the handshake is #4326 (open). Conformance also requires a JSON-RPC `-32700` reply with `id` null to a line that is not valid JSON, with the session kept open; the host does not send it yet, and #4323 closes that gap), (refined 2026-10-29, #4249 activity 9258: an empty line is not a message and gets no reply, and each malformed line is logged by its byte length only) the worst place to hand-roll a mandated external wire contract. mnemra's R-0010 handler logic (auth-check, single WorkspaceCtx construction, per-verb capability check, distinguishable JSON-RPC error codes) rides on top as the rmcp ServerHandler, unchanged by the SDK choice (Amendment 2026-10-25, MCP protocol revision: the host also carries an initialize gate, a line-capped reader and an `initialize` override at that boundary, beside its ServerHandler)."
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

Both decisions recorded here were made by the maintainer. They are recorded together because they correct two halves of the **same** unratified proposal — the plugin substrate stack — that the build reversed on both halves without an ADR.

- **Decision A (raw Wasmtime over Extism)** is recorded `accepted`-on-landing rather than `proposed`: it is already in the tree, merged in Tasks 20–22 (commits `8bb2326` "plugin runtime — manifest load, allowlist, wasmtime limits, epoch supervisor (Tasks 20-21)" and `d378118` "resource-limit trap handling — kill-and-replace recovery (Task 22)"), not awaiting a review gate. This ADR ratifies the choice retroactively, in the same spirit as [P-0011](P-0011-logging-facade.md)'s accept-on-landing for the `tracing` scaffold.
- **Decision B (official `rmcp` SDK over hand-rolled MCP)** is `accepted` as a forward decision: the choice is locked by the maintainer; Task 23 (the MCP-server work) implements it. The lock precedes the implementation, which is why this is `accepted`, not `proposed` — there is no open question for a gate to resolve, only the build to follow.

This ADR does **not** `supersede` or `override` any prior record. The proposal it corrects — an earlier, unratified internal research proposal ("ADR-001: Plugin runtime — Extism on Wasmtime, with rmcp 1.5") — carries **Status: Proposed** and was never ratified into this project's accepted ADR series (P-0001…P-0011 contains no plugin-runtime or MCP-SDK ADR). A never-accepted research proposal cannot be superseded; the relationship is narrated in Context, and `supersedes`/`overrides` stay `null`.

## Context and Problem Statement

The V0 plugin substrate involves two implementation choices that are easy to conflate but are **orthogonal**:

1. **The plugin execution runtime** — what hosts and sandboxes plugin WASM (Extism vs raw Wasmtime vs other).
2. **The MCP protocol layer** — what speaks the MCP/JSON-RPC wire contract to agent clients (official `rmcp` SDK vs hand-rolled).

An earlier, unratified internal research proposal ("ADR-001: Plugin runtime — Extism on Wasmtime, with rmcp 1.5", **Status: Proposed**, dated 2026-04-27) bundled both into a single recommended stack: **Extism (on Wasmtime) + `rmcp` 1.5 + a hyper-mcp-shaped host**. That proposal was never ratified into this project's accepted P-ADR series. The actual build then reversed **both** halves, and neither reversal was recorded:

- The plugin runtime shipped on **raw Wasmtime**, not Extism (Tasks 20–22, on `main`).
- The MCP layer was, at the point this ADR is written, about to be **hand-rolled** rather than built on `rmcp`.

The first reversal was a sound engineering call that fell out of P-0007 (see Decision A). The second was **collateral drift, not a decision**: dropping Extism removed the runtime half of the proposed stack, but gave no reason to also drop `rmcp` — the SDK is the *protocol* layer, orthogonal to the *runtime* layer. The proposal's two halves were coupled only by being written in one document, not by any technical dependency. This ADR records the runtime reversal on its merits and corrects the MCP drift back to the official SDK, separating the two decisions that were never actually coupled.

Neither decision below re-opens a settled question. Both were settled by the maintainer; they are recorded here per `P-PreserveDecisionSpace` (the surfaced alternatives the lock chose among), not re-derived.

## Decision Drivers

- **P-StackDiscipline applied correctly to each layer.** `P-StackDiscipline` (workspace `architecture-principles.md`; "reject ecosystem-misaligned tooling unless no in-stack path exists") cuts **differently** for the two decisions, and naming which way it cuts is the spine of this ADR. For the runtime, the in-stack mechanism (raw Wasmtime's own resource knobs, already a direct dependency) covers the requirement better than the abstraction layered on top of it — so the abstraction comes out. For the MCP layer, the SDK *is* in-stack (a Rust crate, no foreign runtime) and the requirement is a **mandated external wire contract**, not a "small mechanism" the hand-roll carve-out covers — so the SDK goes in. Same principle, opposite outcomes, because the layers differ.
- **P-0007's resource-limit mechanism needs direct Wasmtime control.** The V0 per-instance limits — fuel `Store::set_fuel`, `Store::set_epoch_deadline` with a supervised host epoch-tick thread, and a 64 MiB `ResourceLimiter` — are Wasmtime `Config`/`Store`-surface mechanisms. The plugin manifest's typed host-fn allowlist is compiled per-instance. These want the raw `wasmtime::{Engine, Store, Config}` surface, which an Extism wrapper would mediate and partly hide. (Decision A.)
- **The MCP server is the auth + dispatch trust boundary.** It performs `DF-auth-check` on every request before routing and is the single point where a workspace identity is established. Hand-rolling the wire/JSON-RPC framing there maximizes the surface that must be security-reviewed and the drift risk against an external spec, at the worst possible location. (Decision B.)
- **R-0010-a mandates conformance to an external wire contract.** [R-0010-a](../../specs/2026-05-24-mnemra-core-v0-substrate.md) — "The system SHALL run a single MCP server using the MCP specification 2025-06-18 with stdio transport at V0." Conformance to a versioned external protocol is exactly what an official, conformance-tested SDK provides and what a hand-roll silently risks getting subtly wrong. (Decision B.)
- **License tier is clear and additive, not the lead argument.** Both choices are Green-tier (Wasmtime: Apache-2.0; `rmcp`: Apache-2.0, confirmed on crates.io — see below), so neither triggers a license-gate halt. Per `P-StackDiscipline` rule S2, license tier is a *separate, additive* gate and does not by itself justify a dependency; the fit-for-purpose argument leads in both decisions.

## Considered Options

Two independent decisions, each with its own option set. They are recorded in one ADR because they correct the two halves of one proposal; they are decided independently.

### Decision A — Plugin execution runtime

1. **Raw Wasmtime (chosen).** Depend directly on `wasmtime` (Component Model + Cranelift); drive fuel, epoch-interruption, and the memory `ResourceLimiter` from the raw `Config`/`Store` surface; own the pool, the epoch-tick supervisor, and trap recovery in mnemra-host.
2. **Extism on Wasmtime (the proposal's runtime half).** Host plugins via the Extism framework (itself Wasmtime-backed). Gains a multi-PDK plugin-authoring story and a ready host-fn registration model, at the cost of an abstraction layer between mnemra-host and the Wasmtime `Config`/`Store` knobs that P-0007 drives.

### Decision B — MCP protocol layer

1. **Official `rmcp` SDK (chosen).** Depend on `rmcp` v1.7.0 (the official MCP Rust SDK); implement mnemra's MCP server as an `rmcp` `ServerHandler`; enable `transport-io` (stdio) and enable **no** HTTP-transport feature.
2. **Hand-rolled MCP/JSON-RPC layer.** Implement the MCP 2025-06-18 wire contract, JSON-RPC framing, and stdio transport by hand inside mnemra-host. No SDK dependency; full control of the wire layer, and full ownership of its conformance and security surface.

## Decision Outcome

### Decision A — raw Wasmtime (Option 1)

**Chosen: raw Wasmtime**, because P-0007's resource-limit mechanism is defined directly in terms of Wasmtime's `Config`/`Store` surface, and Extism's abstraction would mediate exactly the knobs P-0007 needs to drive.

[P-0007](P-0007-plugin-resource-limits.md) ("Plugin Resource Limits"; fuel 10B + epoch 5s + 64 MiB ceiling, both fuel and epoch ON at V0) locks a mechanism stated in Wasmtime-native terms: `Store::set_fuel`, `Store::set_epoch_deadline(500)` with a host thread advancing the epoch counter every 10 ms, and a per-instance `ResourceLimiter`/`static_memory_maximum_size`. The corresponding requirements [R-0007-a…i](../../specs/2026-05-24-mnemra-core-v0-substrate.md) ("enable fuel metering via `Store::set_fuel`"; "enable epoch-interruption … via `Store::set_epoch_deadline(500)` with a host epoch-tick thread"; "per-instance memory ceiling … via `Config::static_memory_maximum_size` or a `ResourceLimiter`"; "the Wasmtime version SHALL be pinned") name the raw Wasmtime API directly. [R-0016](../../specs/2026-05-24-mnemra-core-v0-substrate.md) ("Plugin pool", anchors P-0007) requires a host-owned 3–5-instance pool with synchronous kill-and-replace — the pool, the epoch supervisor, and the trap-to-recovery path are mnemra-host's own machinery sitting directly on the Wasmtime `Store`. Inserting Extism between mnemra-host and that surface would fight the mechanism P-0007 already locked: the resource knobs, the supervised epoch thread, and the trap-recovery path want unmediated `wasmtime::{Engine, Store, Config}` access. Dropping Extism was the sound call; this ADR ratifies it.

This is consistent with `P-StackDiscipline`'s "How it shows up": when the in-stack primitive (raw Wasmtime, already a direct dependency) covers the requirement, an abstraction layered on top of it is not warranted. Wasmtime is not an *added* ecosystem here — it is the runtime the project already targets; Extism would have been the addition.

### Decision B — official `rmcp` SDK (Option 1)

**Chosen: the official `rmcp` SDK (v1.7.0)**, because the MCP server is both a mandated-external-protocol surface and the auth/dispatch trust boundary — the two properties that most strongly argue against hand-rolling a wire layer — and `rmcp` is the conformant, in-stack, license-clean way to satisfy them.

The maintainer's lock (recorded verbatim per `P-PreserveDecisionSpace`): *"we'll use the official sdk for now unless there becomes an issue with it."*

**Why the hand-roll was the wrong default (collateral drift):** `rmcp` is the MCP *protocol* layer; Wasmtime/Extism is the plugin *execution* runtime. Dropping Extism (Decision A) removed the runtime half of the proposed stack and gave **no** reason to drop the protocol half. The two were coupled only by being recommended in one proposal document — not by any technical dependency. Hand-rolling the MCP layer was drift that followed the runtime reversal by proximity, not a decision on its own merits.

**Why the SDK is right on its merits:**

1. **Mandated external wire contract → conformant official SDK.** [R-0010-a](../../specs/2026-05-24-mnemra-core-v0-substrate.md) ("a single MCP server using the MCP specification 2025-06-18 with stdio transport at V0") makes MCP conformance a hard requirement against a versioned external spec. `rmcp` is the **official** MCP Rust SDK (`github.com/modelcontextprotocol/rust-sdk`), post-1.0, conformance-suite-backed, with 13M+ downloads. A hand-rolled JSON-RPC/MCP layer risks subtle non-conformance to a wire contract the project does not own and cannot unilaterally redefine. This is precisely the case `P-StackDiscipline`'s hand-roll carve-out ("when the in-stack option is pre-1.0 or thin, hand-rolling a small mechanism often beats inheriting churn") does **not** cover: MCP/JSON-RPC at the V0 surface is neither small nor a mechanism the project controls — it is a mandated external protocol, and the in-stack SDK is post-1.0 and mature, not thin.
2. **Trust-boundary minimization.** The MCP server is where `DF-auth-check` runs before any routing ([R-0010-c](../../specs/2026-05-24-mnemra-core-v0-substrate.md): "perform `DF-auth-check` … on every incoming request before routing") and where the single `WorkspaceCtx` is constructed after token validation ([R-0006-b](../../specs/2026-05-24-mnemra-core-v0-substrate.md): "`WorkspaceCtx` SHALL be constructed at a single location in the MCP verb dispatch path, after token validation; there SHALL be no alternative construction path"). Hand-rolling the wire/framing code at this boundary maximizes the attack surface to security-review and the room for framing bugs — the worst place to write bespoke protocol code.
3. **Transport features encode the V0 boundary precisely.** Enabling `rmcp`'s `transport-io` (stdio) satisfies R-0010-a's stdio mandate; enabling **no** HTTP-transport feature is how [R-0010-e](../../specs/2026-05-24-mnemra-core-v0-substrate.md) ("Streamable-HTTP MCP transport SHALL NOT be activated at V0") is satisfied structurally — the capability is simply not compiled in.
4. **mnemra's R-0010 logic is unchanged by the SDK choice — it rides on top.** The SDK supplies transport, framing, and JSON-RPC conformance; mnemra's policy lives in its `ServerHandler` implementation and is identical whether the transport is hand-rolled or `rmcp`-provided: `DF-auth-check` before routing ([R-0010-c](../../specs/2026-05-24-mnemra-core-v0-substrate.md)); single `WorkspaceCtx` construction after validation ([R-0006-b](../../specs/2026-05-24-mnemra-core-v0-substrate.md)); per-verb capability check against the manifest's declared `verbs` before dispatch ([R-0010-d](../../specs/2026-05-24-mnemra-core-v0-substrate.md): "enforce a per-verb capability check against the plugin manifest's declared `verbs` list before dispatching"); and distinguishable JSON-RPC error codes for invalid-token vs verb-not-found vs parameter-invalid, never conflated ([R-0010-f](../../specs/2026-05-24-mnemra-core-v0-substrate.md)). Only the wire transport/framing changes hands — from a hand-rolled implementation to `rmcp`'s `ServerHandler` — while the auth, dispatch, and error-classification logic is mnemra's own.

**`rmcp` facts, confirmed at crates.io (not restated from the proposal):** name `rmcp`; latest version **1.7.0**; license **Apache-2.0** (single, not dual); 13,075,574 downloads; official repository `github.com/modelcontextprotocol/rust-sdk`; description "Rust SDK for Model Context Protocol." Apache-2.0 is **Green-tier** (auto-proceed) under the workspace dependency-approval model — the same tier as mnemra-core's own license — so the dependency clears the license gate without a halt. (The proposal cited `rmcp` 1.5; the version locked here is the current 1.7.0.)

> **Deferral / revisit tripwire (named instrument, `P-Defer`).** Decision B holds *"for now unless there becomes an issue with it."* The named tripwire that fires a revisit is **a concrete `rmcp` limitation that blocks a spec requirement** — for example, an `rmcp` behaviour that cannot satisfy R-0010-a's MCP-2025-06-18 conformance, or that forces a deviation from the R-0010-c/d/f handler contract. Absent such a concrete, requirement-blocking issue, `rmcp` stands; a preference for a different SDK or a hand-roll is not a firing condition.

### Scope — implementation-decision record only; the spec is unchanged

This ADR records *how* the substrate is built; it does not alter *what* the spec requires. The locked, verified spec [`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`](../../specs/2026-05-24-mnemra-core-v0-substrate.md) — at its current content-lock (git blob `9f4695f3a9c5f0906a7ce2e3848eb65bbd47834f`, the live spec on `main`, the state this ADR's R-IDs were read from) — stays locked and is **not** edited by this ADR. In particular:

- **R-0010-a is implementation-agnostic and stays UNCHANGED.** It mandates "a single MCP server using the MCP specification 2025-06-18 with stdio transport" without naming an implementation; `rmcp` is the implementation choice that satisfies it, recorded here, not a change to the requirement.
- The Task-23 plan reshape and the three Task-23 plugin-runtime carries (the limit-attach test, the `can_invoke` invoke-path wiring, the epoch-hook gating) are **plugin-runtime** concerns, unaffected by the `rmcp` choice and out of scope for this ADR.

### Consequences

**Good:**

- **Decision A:** P-0007's resource-limit mechanism (fuel + epoch + `ResourceLimiter`) is driven directly on the raw Wasmtime surface, with no abstraction mediating the knobs the spec names. The pool, epoch supervisor, and trap-recovery path are mnemra-host's own machinery — already shipped and exercised in Tasks 20–22.
- **Decision B:** MCP-2025-06-18 wire conformance comes from the official, conformance-tested SDK rather than from bespoke code the project would have to keep conformant by hand. The auth/dispatch trust boundary carries the minimum bespoke surface — only mnemra's `ServerHandler` policy logic, not the JSON-RPC framing.
- **Both:** the substrate now has a recorded, canon-anchored decision for each of the two layers the proposal had bundled — the drift (an unrecorded reversal on each half) is corrected, and a future reader sees *which* layer each decision governs and *why*.
- The V0 no-HTTP-transport boundary (R-0010-e) is enforced structurally by not compiling the `rmcp` HTTP-transport feature, rather than by a runtime guard.

**Bad / Trade-offs:**

- **Decision A:** raw Wasmtime forgoes Extism's ready-made multi-language plugin-authoring story and its host-fn registration ergonomics; mnemra-host owns the pool, the epoch supervisor, and trap recovery itself (the machinery already built in Tasks 20–22). Accepted: at V0 the plugins are `core: true`, well-behaved, first-party artifacts, and P-0007's direct-control requirement outweighs the authoring-DX convenience.
- **Decision B:** `rmcp` is a new direct dependency and a versioned external API surface to track. Mitigated: it is Green-tier (Apache-2.0), the official SDK for a protocol the project must speak anyway, and is confined to the MCP transport/framing seam behind mnemra's own `ServerHandler` — a version bump touches the transport seam, not the auth/dispatch/error logic.
- Recording two decisions in one ADR risks a future reader treating them as coupled. Mitigated explicitly: the ADR's whole thesis is that the runtime layer and the protocol layer are **orthogonal**, and each decision is decided on its own option set and its own drivers.

## Pros and Cons of the Options

### Decision A — Runtime

#### Raw Wasmtime (chosen)

- Pro: Direct access to the `Config`/`Store` surface P-0007's fuel/epoch/`ResourceLimiter` mechanism is defined in terms of; no abstraction mediating the locked knobs.
- Pro: Wasmtime is already the project's runtime — no added ecosystem; `P-StackDiscipline`-aligned (the in-stack primitive covers the requirement).
- Pro: Already shipped and exercised (Tasks 20–22): pool, epoch supervisor, trap-recovery, kill-and-replace are in the tree.
- Con: No Extism-provided multi-PDK plugin-authoring story; host-fn registration and pool machinery are mnemra-host's to own. Accepted for V0's first-party `core: true` plugins.

#### Extism on Wasmtime

- Pro: Ready multi-language plugin-authoring (multiple PDKs) and a packaged host-fn registration model.
- Con: An abstraction layer between mnemra-host and the Wasmtime `Config`/`Store` knobs that P-0007 (R-0007-a…i) drives directly — it mediates exactly the surface the locked resource-limit mechanism needs.
- Con: Adds the Extism framework as the runtime layer when the underlying Wasmtime (already a direct dependency) covers the V0 requirement on its own.

### Decision B — MCP protocol layer

#### Official `rmcp` SDK (chosen)

- Pro: Conformant to MCP 2025-06-18 (R-0010-a) out of the official, conformance-suite-backed SDK; no hand-maintained wire conformance.
- Pro: Minimizes bespoke code at the auth/dispatch trust boundary (R-0010-c, R-0006-b) — the highest-stakes place to avoid hand-rolled framing.
- Pro: In-stack (a Rust crate, no foreign runtime) and Green-tier (Apache-2.0); clears the license gate as an additive, not leading, factor.
- Pro: V0 transport boundary is structural — `transport-io` on satisfies R-0010-a; no HTTP-transport feature compiled satisfies R-0010-e.
- Con: A new direct dependency and an external API surface to track across versions. Mitigated by Green-tier status, official-SDK maturity (post-1.0, 13M+ downloads), and confinement behind mnemra's own `ServerHandler`.

#### Hand-rolled MCP/JSON-RPC layer

- Con: Risks subtle non-conformance to a **mandated external** wire contract (MCP 2025-06-18, R-0010-a) the project neither owns nor can redefine.
- Con: Maximizes bespoke code precisely at the auth/dispatch trust boundary — the worst location for hand-written framing from a security-review standpoint.
- Con: `P-StackDiscipline`'s hand-roll carve-out does not apply: MCP/JSON-RPC is not a "small mechanism," and the in-stack SDK is post-1.0 and mature (not pre-1.0 or thin), so the "hand-roll a small thing rather than inherit churn" exception is not in play.
- Con: Collateral drift — adopted by proximity to the Extism reversal, not on its own merits; there was no technical dependency coupling the two halves of the proposal.

## More Information

**The proposal being corrected (related precedent, not a superseded ADR).** An earlier, unratified internal research proposal ("ADR-001: Plugin runtime — Extism on Wasmtime, with rmcp 1.5", **Status: Proposed**, 2026-04-27) recommended a single stack — Extism + `rmcp` 1.5 + a hyper-mcp-shaped host. It was never ratified into this project's accepted P-ADR series. This ADR is **not** a `supersede`: a never-accepted research proposal has no `accepted` status to replace, so `supersedes` stays `null` and the relationship is narrated here. The proposal's runtime half was reversed (Decision A); its protocol-SDK half is *re-affirmed* here (Decision B) at the current `rmcp` version — only its bundling with Extism was wrong.

**Why this is a project ADR.** Each decision meets the ADR-vs-design-note criterion ([ADR README](README.md); agent-first: a reversal forces downstream agent rework or external-consumer reference). Reversing Decision A would force the pool, epoch supervisor, and trap-recovery machinery to be rebuilt against a different runtime surface. Reversing Decision B would force every MCP wire/framing site to change and re-establish conformance to an external spec. Both are dependency-selection decisions of the same kind as [P-0010](P-0010-storage-substrate-engine.md) (a concrete engine behind a swappable trait) and [P-0011](P-0011-logging-facade.md) (a concrete logging crate + topology) — concrete library choices under a locked contract.

**Related precedent (plugin-runtime context, not overlap).** [P-0002](P-0002-core-plugin-partition.md) (core/plugin partition), [P-0003](P-0003-plugin-manifest.md) (V0 plugin manifest — the typed host-fn allowlist compiled per-instance), and [P-0007](P-0007-plugin-resource-limits.md) (plugin resource limits) all touch the plugin runtime but decide *different* questions (which verbs are core, the manifest schema, the resource-limit values). None decides the execution-runtime framework (Extism vs raw Wasmtime) or the MCP-SDK question; this ADR fills both gaps. P-0007 is Decision A's primary anchor — its mechanism is *why* raw Wasmtime.

**Canon anchors:**

- `P-StackDiscipline` (workspace `architecture-principles.md`, with rules S1/S2): reject ecosystem-misaligned tooling unless no in-stack path exists; "industry-default" is an installed-base claim, not a fit claim, and license tier is a separate additive gate. Applied per-layer: it removes the Extism abstraction (the in-stack Wasmtime primitive covers the requirement) and admits `rmcp` (in-stack, mandated external protocol, post-1.0 — the hand-roll carve-out does not apply).
- `P-PreserveDecisionSpace`: both decisions are recorded with their surfaced alternatives as the options the lock chose among; the maintainer's lock quote is recorded verbatim.
- `P-Defer`: Decision B is held with a named firing tripwire (a concrete `rmcp` limitation that blocks a spec requirement), not silently.

**References:**

- Decision A — shipped (paths confirmed to resolve in-repo, under `libs/mnemra-host/plugin/`):
  - Runtime + resource limits: `libs/mnemra-host/plugin/runtime.rs`, `libs/mnemra-host/plugin/limits.rs`.
  - Epoch supervisor: `libs/mnemra-host/plugin/epoch_thread.rs`.
  - Pool + kill-and-replace: `libs/mnemra-host/plugin/pool.rs`, `libs/mnemra-host/plugin/trap_recovery.rs`.
  - Manifest + host-fn allowlist: `libs/mnemra-host/plugin/manifest.rs`, `libs/mnemra-host/plugin/allowlist.rs`.
  - Wasmtime dependency pin: `libs/mnemra-host/Cargo.toml` (`wasmtime = "=45.0.2"`, features `cranelift` + `component-model`; no `extism` dependency present). This bullet records the **shipped** feature state only; the runtime-feature *forward contract* — the `async` feature's adoption path and the `component-model-async` off-switch — is owned by [P-0019-plugin-contract](P-0019-plugin-contract.md) D3, the single source for those commitments. (Cross-reference added 2026-07-03 at the P-0019 gate.)
  - Merged in `8bb2326` (Tasks 20–21) and `d378118` (Task 22).
- Decision B — **planned location** (these paths do **not** resolve in-repo yet; Task 23 creates them, following the crate's dominant sibling-file module-entrypoint convention established in `df2d0f0` — `<module>.rs` beside the `<module>/` directory, as `auth.rs`/`auth/`, `storage.rs`/`storage/`, etc.; the one `plugin/mod.rs` is a pre-existing exception, not the pattern to follow): the MCP module entrypoint `libs/mnemra-host/mcp.rs` with `libs/mnemra-host/mcp/server.rs` (the `rmcp` `ServerHandler` replacing the would-be hand-rolled transport/framing), `libs/mnemra-host/mcp/dispatch.rs` (R-0010-c/d auth-check + capability check + single `WorkspaceCtx` construction per R-0006-b), and `libs/mnemra-host/mcp/errors.rs` (R-0010-f distinguishable JSON-RPC error codes). `rmcp` is added to `libs/mnemra-host/Cargo.toml` (features `server`, `transport-io`; no HTTP-transport feature) by Task 23.
- Spec requirements: [R-0006-b](../../specs/2026-05-24-mnemra-core-v0-substrate.md), [R-0007-a…i](../../specs/2026-05-24-mnemra-core-v0-substrate.md), [R-0010-a/c/d/e/f](../../specs/2026-05-24-mnemra-core-v0-substrate.md), [R-0016](../../specs/2026-05-24-mnemra-core-v0-substrate.md), in [`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`](../../specs/2026-05-24-mnemra-core-v0-substrate.md) at content-lock git blob `9f4695f3a9c5f0906a7ce2e3848eb65bbd47834f` (the live spec on `main`).
- `rmcp` on crates.io: v1.7.0, Apache-2.0, official MCP Rust SDK (`github.com/modelcontextprotocol/rust-sdk`).

**Follow-up:** Task 23 implements Decision B (the `rmcp` `ServerHandler` MCP server). The three Task-23 plugin-runtime carries (limit-attach test, `can_invoke` invoke-path wiring, epoch-hook gating) and the Task-23 plan reshape are tracked separately and are out of scope for this ADR.

## Amendment 2026-10-25 — Crate versions are not design values (version citations only)

Applied 2026-10-25 under the maintainer's rulings on #4249: activity 8285 (design records carry no crate or library version), 8303 (confirming 8296: a crate version stays out as a pin or as a floor; a design dependency on a crate names the capability it needs) and 8380 (versioned provenance stays). The reason given is that a version change, for example a bump forced by a security advisory, must never require reopening a locked record. The measured case behind the ruling includes this record: it named `wasmtime = "=45.0.2"` after `libs/mnemra-host/Cargo.toml` had moved past it, with no amendment and no signal.

**Scope.** This amendment touches the crate-version citations only. It does not touch the MCP protocol revision this record cites (2025-06-18, R-0010-a); that is a separate amendment, and a second amendment section with the same date may follow this one.

**What each decision depends on.** These two statements supersede every version clause listed below.

- **Decision A** depends on a Wasmtime release that exposes the raw `Engine` / `Store` / `Config` surface P-0007 drives (fuel metering through `Store::set_fuel`, epoch interruption through `Store::set_epoch_deadline`, a per-instance `ResourceLimiter`), with the `cranelift` and `component-model` features. Which release meets that is an implementation fact held in `libs/mnemra-host/Cargo.toml`, not in this record.
- **Decision B** depends on the official MCP Rust SDK `rmcp` (`github.com/modelcontextprotocol/rust-sdk`) providing a `ServerHandler` server over a stdio transport (the `transport-io` feature), with no HTTP-transport feature enabled. The release in use is the `rmcp` entry in the workspace `Cargo.toml`, not a design value.

The Decision B revisit tripwire is unchanged. It was already keyed on a capability (a concrete `rmcp` limitation that blocks a spec requirement), so an `rmcp` release change that keeps the Decision B capability above does not fire it.

**Superseded (the text above is kept as the record):**

- the frontmatter summary's "`rmcp` v1.7.0", Decision B Option 1's "Depend on `rmcp` v1.7.0" and the Decision B outcome's "(v1.7.0)": the choice is the SDK, not a release of it;
- "the version locked here is the current 1.7.0" at the end of "`rmcp` facts": no version is locked;
- the version quoted in the Decision A reference bullet "Wasmtime dependency pin" (`wasmtime = "=45.0.2"`): the bullet still points at `libs/mnemra-host/Cargo.toml` as where the shipped pin and features live, and the quoted value is that file as recorded on 2026-06-19.

**Kept as provenance (not superseded):**

- the other "`rmcp` facts" (latest version, license, download count) and the References entry "`rmcp` on crates.io: v1.7.0": a reading of crates.io taken when this record was written, 2026-06-19;
- "rmcp 1.5" in the title and description of the unratified ADR-001 proposal (Status, Context, More Information): a historical quotation;
- "post-1.0" in the SDK-maturity argument: the maturity the decision was weighed on in June 2026.

## Amendment 2026-10-25 (MCP protocol revision)

**Status: approved 2026-10-29** (#4249 activity 9183). The maintainer ruled on #4249 (activity 8421) that this amendment is drafted only if a measured run showed the host negotiates MCP 2025-11-25 and still serves 2025-06-18 clients, and that it then waits for his approval. The measurement (activity 8589) showed both, on the host as landed at `de62732` (#4295, #4294).

**Scope.** This amendment changes the MCP protocol revision this record cites, adds the parse-error reply that conformance requires, and corrects this record's claims about how little bespoke code sits at the MCP boundary (Superseded, below). It does not touch the crate-version citations, which the amendment above handles, and it does not change Decision A or Decision B: the SDK choice stands. Protocol revisions stay in design records: rulings 8421 and 8435 on #4249 both direct this record to name one. What the #4249 sweep takes out of design records is crate versions and runtime pins.

**Rulings applied.**

- Activity 8421: the condition above.
- Activity 8435 (the maintainer, 2026-10-25): the record names a pinned set of released revisions, and the host enforces it, so that an SDK upgrade cannot silently widen it. Its wording also says the set "refuses drafts".
- Activity 9183 (the maintainer, 2026-10-29), on the `initialize` shape: *"Yes, a refinement"*. The ruling, as that activity records it: the host never speaks a draft MCP version, and a request naming any other revision is answered with 2025-11-25, per MCP's version negotiation. This refines 8435's "refuses drafts". MCP negotiates the revision at `initialize`, so the downgrade is the `initialize` answer: (this record's reading of 'per MCP negotiation'; 9183's text does not name requests other than `initialize`; label added 2026-10-29, #4249 activity 9270) it is the shape #4294 implemented and the measurement (8589) observed. 8435's "refuses drafts" still holds for a request other than `initialize` or `ping` whose own complete `_meta` names a revision outside the set: that request gets no result (below).
- Activity 9183 (the maintainer, 2026-10-29), on the parse-error reply: *"Reply -32700"*. The ruling, as that activity records it: conformance requires the standard JSON-RPC 2.0 parse-error reply to malformed JSON, and the session stays open.
- Activity 9258 (the maintainer, 2026-10-29), two refinements of 9183 at the edge of the parse-error ruling, each an option the maintainer selected from text Puck wrote. *"Blank line gets no reply"*: an empty input line is not a message and gets no `-32700` reply. *"Log each malformed line"*: one structured record per malformed line at a level that is on by default, carrying the byte length only, never the bytes or the parser's message, rate-limited per session. Both are written into the parse-error criteria below as refinements dated 2026-10-29.

**Decision: the conformance target is MCP 2025-11-25, the host never speaks a draft revision, and it serves exactly the released revisions {2025-06-18, 2025-11-25}.** R-0010-a's target moves from 2025-06-18 to 2025-11-25: it is the newest released revision and the one the host answers by default, and 2025-06-18 stays served so that a client pinned to it keeps working. No session is served at a revision outside the set. A request whose own `_meta` names one gets no result, except a `ping` before the handshake, which gets an empty result whatever its `_meta` (below). *(Anchors: the maintainer's rulings 8421, 8435 and 9183; `P-GuaranteeByMechanism`, "enforce a guarantee with a mechanism that fails loudly when violated": the set is a host constant, pinned by tests on the paths named below, not the SDK's default.)*

- An `initialize` requesting 2025-06-18 is answered with 2025-06-18.
- An `initialize` requesting 2025-11-25 is answered with 2025-11-25.
- An `initialize` requesting any other revision is answered with 2025-11-25. This covers a draft such as 2026-07-28, an older released revision such as 2025-03-26, and an unknown string such as 2024-01-01. The answer is a downgrade, not an `initialize` error.
- The server's default revision is 2025-11-25.
- A request other than `initialize` whose own `_meta` names a revision outside the set gets `-32022` (unsupported protocol version), and the error data lists the set, with three exceptions before the handshake: a `ping` is answered whatever its `_meta`, a request with incomplete `_meta` gets `-32602`, and a `server/discover` gets `-32600` from the initialize gate (lifecycle below). After the handshake every such request gets `-32022`, `ping` included, from the SDK's per-request check, and that path is not pinned by a test (below). An `initialize`'s own `_meta` is not consulted: its revision is the one it requests, as above.
- A session negotiated at 2025-11-25 carries none of the 2026-07-28 draft's result fields (`resultType`, `ttlMs`, `cacheScope`). That holds when the client asked for the draft too.

**Enforcement: the host owns the set.** MEASURED at `de62732` by reading the code; the host line numbers are that commit's. The `rmcp` line numbers are a source read of rmcp 3.7.0, the release `Cargo.lock` pins at `de62732`.

- `SUPPORTED_PROTOCOL_VERSIONS` in `libs/mnemra-host/mcp/server.rs` (`:77-91`) holds the two revisions.
- `MnemraMcpServer`'s `ServerHandler::supported_protocol_versions` returns that constant (`:555-558`).
- `get_info` names 2025-11-25 explicitly (`:522-529`) rather than inheriting the SDK's default, which is the SDK's newest known revision, draft included.
- `MnemraMcpServer`'s `ServerHandler::initialize` (`:543-553`) stores the negotiated revision as the session's, not the one the client requested.

**What the tests pin.** On two paths, an SDK release that learns more revisions does not change what is served, and tests in `libs/mnemra-host/tests/mcp_server.rs` turn red if it does.

- At `initialize`:
  - `initialize_released_2025_06_18_is_echoed` (`:3087`);
  - `initialize_released_2025_11_25_is_echoed` (`:3095`);
  - `initialize_draft_2026_07_28_is_downgraded_to_2025_11_25` (`:3103`);
  - `initialize_unknown_version_is_downgraded_to_2025_11_25` (`:3111`);
  - `initialize_older_known_version_is_downgraded_to_2025_11_25` (`:3119`);
  - `session_replies_carry_no_draft_result_fields` (`:3407`);
  - `handshake_requesting_the_draft_version_is_served_as_released` (`:3443`). It pins an outcome: a session whose handshake asked for the draft is served as 2025-11-25. Two things produce that outcome, and they are co-equal. The host's `initialize` override stores the negotiated revision, and the SDK's handshake sets it again after the handler returns (rmcp 3.7.0 `src/service/server.rs:692-703`). Either one alone keeps the test green, so on the handshake path neither is pinned on its own. The host override alone is pinned on a re-initialize that reaches an ungated server (`handler_without_the_gate_stores_the_negotiated_version`, `:3503`).
- Before the handshake, on a request's own `_meta`: `pre_init_request_with_draft_inline_meta_does_not_start_stateless_lifecycle` (`:3264`), and the gate tests in the lifecycle below.

**Not pinned: a request's own `_meta` after the handshake.** Once the handshake is done, the initialize gate passes every request through (`libs/mnemra-host/mcp/gate.rs:85`), and the SDK takes a request's protocol revision from that request's own `_meta` before the session's (rmcp 3.7.0 `src/service.rs:1220-1226`). On an established session, a request whose `_meta` names the draft is therefore stopped only by the SDK's per-request check against `supported_protocol_versions` (rmcp 3.7.0 `src/handler/server.rs:64-72`), which answers `-32022`. No test at `de62732` sends a request's own `_meta` after the handshake, so an SDK release that narrowed that check would leave every test green while the served set widened. The test that pins this path is filed as #4326 (implementation lane, open). **This record's claim that an SDK upgrade cannot widen the set covers the two pinned paths above only.** It widens to this path when #4326's test lands. Until then #4326 is parked: nothing fires it, and no cadence that resurfaces it is named in this record. *(Refined 2026-10-29, #4249 activity 9270: the parked label in the sentence before this note is withdrawn. A parked item needs a named cadence that resurfaces it, and this one had none, so under `P-Defer` DF1 it was neither parked nor deferred. #4326 is a blocking dependency of #4323 in the tracker, so #4326 is fired by #4323's close. Until #4326's test lands, the claim that an SDK upgrade cannot widen the set still covers only the two pinned paths above.)*

**Lifecycle before the handshake.** This is how the host behaves at `de62732`. The invariant is that **before the `initialize` handshake completes, no request other than `initialize` or `ping` receives a result.** Two layers produce it. The SDK's pre-initialize loop (rmcp 3.7.0 `src/service/server.rs:563-672`) answers some requests itself. The initialize gate `InitializeGated` (`libs/mnemra-host/mcp/gate.rs:63-88`) sees every request the SDK hands to the service. The gate is an allowlist and fails closed, so a request kind a later SDK release adds, and hands to the service, is refused rather than served. Production starts every session through `libs/mnemra-host/mcp/gate.rs`'s `serve` (`:41-61`; called at `libs/mnemra-host/mnemra_host.rs:405`).

Each outcome, with the code that produces it:

- `ping` is answered with an empty result by the SDK's loop, whatever its `_meta` (`src/service/server.rs:566-581`), and an `initialize` then negotiates normally (`pre_init_ping_is_answered_and_initialize_follows`, `mcp_server.rs:3374`).
- A request other than `initialize` or `ping` whose `_meta` is missing or incomplete gets `-32602` from the SDK's loop (`src/service/server.rs:588-607`), and the connection stays before the handshake. This is pinned for `server/discover` (`pre_init_discover_without_meta_is_invalid_params`, `:3282`). For other request kinds it is a source read, untested; #4326 carries that case as optional.
- A `server/discover` with complete `_meta` gets `-32600`, message `initialize required`, from the gate (`gate.rs:82-84`), whichever revision it names. The SDK hands a discover to the service as a bootstrap probe before its own version check (`src/service/server.rs:610-632`). The connection stays before the handshake, and an `initialize` then negotiates normally (`pre_init_discover_naming_draft_version_is_refused`, `:3241`; `pre_init_discover_naming_released_version_is_refused`, `:3356`).
- A request other than `server/discover` that has complete `_meta` naming a revision outside the set, and that reaches the SDK's version check, gets `-32022` (unsupported protocol version) from the SDK (`src/service/server.rs:634-651`). The error data lists the supported set, and the connection stays before the handshake (`pre_init_request_with_draft_inline_meta_does_not_start_stateless_lifecycle`, `:3264`; assertion `:3222-3238`).
- A request other than `server/discover` that has complete `_meta` naming a revision in the set latches the SDK's inline mode for the connection (`src/service/server.rs:653-664`). The gate refuses it with `-32600` `initialize required` (`gate.rs:82-84`). From then on, on that connection:
  - a later `initialize` gets `-32600` `initialize required` (`gate.rs:73-75`);
  - any other request except `ping` gets the same (`gate.rs:82-84`);
  - a `ping`, which the gate allows, gets `-32602` from the SDK's per-request metadata check (`src/handler/server.rs:78-99`).

  The connection stays refused until it is reopened (`pre_init_request_with_released_inline_meta_is_refused_and_so_is_a_late_initialize`, `:3328`; against the real binary, `production_binary_refuses_a_request_before_initialize`, `libs/mnemra-host/tests/smoke_e2e.rs:402`).
- A second `initialize` on an established session is refused with `-32600`, message `already initialized`, and does not change how the session is served (`gate.rs:76-80`; `re_initialize_is_refused_and_does_not_enable_draft_shaping`, `:3470`).
- An incoming line longer than `MAX_INCOMING_LINE_BYTES` (`libs/mnemra-host/mcp/transport.rs:27`, 16 MiB at `de62732`, newline included) ends the session. The client sees end of stream; the line is not dropped with the client left waiting (`transport.rs:1-11`). The tests are `line_one_byte_over_cap_closes_the_stream` (`mcp_server.rs:3637`), `over_cap_line_with_no_newline_closes_the_stream` (`:3649`), `over_cap_line_after_initialize_closes_the_stream` (`:3661`) and `production_binary_ends_the_session_on_an_over_cap_incoming_line` (`smoke_e2e.rs:315`). A line exactly at the cap is served (`line_exactly_at_cap_is_served`, `:3611`).

**Decision: the lifecycle refusals stand as implemented, and they do not conflict with R-0010-f.** R-0010-f's closed list of classes that must stay distinguishable is invalid token, verb not found and parameter invalid. The refusals above use three codes.

- `-32600`, the gate's lifecycle refusals. The invalid-token code is pinned to differ from `-32600` (`bogus_token_returns_distinguishable_auth_failure`, `mcp_server.rs:457-461`), so a lifecycle refusal cannot be read as an authentication failure. The `-32600` code was set by the implementation lane as a default that can be revisited, and it was routed here for this check.
- `-32602`, from the SDK before the handshake and on a latched connection. Each refuses a request that is missing the `_meta` the SDK requires, which is a parameter fault. It falls inside the parameter-invalid class rather than blurring two classes.
- `-32022`, from the SDK for a revision outside the set. It is a code of its own. It differs from the invalid-token code (`-4001`, `libs/mnemra-host/mcp/errors.rs:29`), from the code for a verb outside the manifest (`-4005`, `errors.rs:70`) and from `-32602`. Two codes for neighbouring faults is not conflation; one code for two classes would be.

*(Anchor: R-0010-f's clause text in [`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`](../../specs/2026-05-24-mnemra-core-v0-substrate.md), "error code classes SHALL NOT be conflated", over its three named classes.)*

**Decision: a line that is not valid JSON gets the JSON-RPC 2.0 parse-error reply, and the session stays open.** MCP 2025-11-25 carries its messages as JSON-RPC 2.0, and the maintainer ruled that R-0010-a's conformance target requires the standard parse-error reply. *(Anchors: the maintainer's ruling 9183, "Reply -32700"; `P-GuaranteeByMechanism`, "enforce a guarantee with a mechanism that fails loudly when violated": the reply is pinned by a test, so an SDK release that changes how unparsable input is handled turns a test red.)*

Each criterion applies to a line that ends in a newline, is no longer than `MAX_INCOMING_LINE_BYTES`, and does not parse as JSON, whether it is sent before the `initialize` handshake or after it:

- The host writes exactly one response for the line.
- That response has `error.code` `-32700` and `id` `null`.
- Its `error.message` is byte-identical for any two such lines, so it carries neither the line nor the parser's error detail.
- The session stays open and its lifecycle state does not change. Before the handshake, an `initialize` sent next negotiates normally. After the handshake, a `ping` sent next is answered with a result.

**Refined 2026-10-29** (#4249: the maintainer's ruling 9258, Warden's review 9244, Charon's review 9270). These criteria refine the four above, which are kept as the record. Where a refinement and a criterion above differ, the refinement holds. *(Anchors: the maintainer's rulings 9183 and 9258; `V-DataMinimization`, "collect and retain the minimum data the surface's job requires": a malformed line can carry a token (Warden 9244 M3), so its log record carries its length and nothing of its content; `P-GuaranteeByMechanism`, "enforce a guarantee with a mechanism that fails loudly when violated": each criterion below is stated so a test can turn red on it.)*

- **The per-line bound** (Warden 9244 L2; this record's reading of the bound). "No longer than `MAX_INCOMING_LINE_BYTES`" uses the measure the lifecycle above applies to an over-cap line: the line's length in bytes, its newline included. A line that does not parse as JSON and is exactly `MAX_INCOMING_LINE_BYTES` long, newline included, gets the reply. A line one byte longer gets no `-32700` reply, and the session ends as for any over-cap line.
- **An empty line** (ruling 9258, refining 9183). A line with no bytes before its newline is not a message, and the four criteria above do not apply to it. The host writes no response for it, writes no log record for it (this record's reading: an empty line is not a malformed line), and the session's lifecycle state does not change. This record also reads a line whose only byte before its newline is one carriage return as empty, which matches the SDK: it strips one trailing carriage return and then skips an empty line without parsing it (rmcp 3.7.0 `src/transport/async_rw.rs:276-283`, source read, untested). A line holding anything else that does not parse as JSON, whitespace only included, is not empty under this reading and gets the reply.
- **The whole response line is constant** (Warden 9244 M1; this also closes Charon 9270's Low on the no-oracle condition below). The response the host writes for such a line, every byte from its first through its newline, is byte-identical for any two such lines, in one session or in two, before the handshake or after it. This binds `error.data` and every other field as well as `error.message`, so no part of the reply carries the line or the parser's detail.
- **The reply is written whole and never interleaved** (Warden 9244 M4). No byte of another message the host writes falls between the reply's first byte and its newline, and no byte of the reply falls inside another message's line. Observable: while lines that do not parse as JSON and well-formed requests are sent on one session at the same time, every line the host writes to stdout parses as exactly one complete JSON-RPC message, and the number of `-32700` replies equals the number of such lines sent. The host must own the write side to meet this. The SDK's stdio writer is a private field of its transport (rmcp 3.7.0 `src/transport/async_rw.rs:68`, source read), so host code cannot take the lock the SDK's own writes hold, and a reply written beside that writer could interleave with them. #4323 builds the reply on a write path the host owns.
- **One log record per malformed line** (ruling 9258, refining 9183). For each line that gets the reply, the host writes one structured log record, subject to the per-session limit in the next criterion. The record is emitted at a level the host's default log filter passes, so it appears in the host's stderr log when no filter is configured (the default filter is `info`, `cmd/mnemra/logging.rs:32`, MEASURED at `de62732`). It carries the line's length in bytes as a field. It carries neither the line's bytes nor the parser's error message: a line containing a marker string puts no occurrence of that marker into the log, and no record contains the parser's message text.
- **The log records are rate-limited per session** (ruling 9258). Within one session, the host writes at most a fixed number of these records in a fixed window, and both are constants in host code. Observable: when one session receives more such lines inside one window than the limit, the host writes exactly the limit's number of records for that window, and every line still gets its reply. Lines on one session do not consume another session's limit. **Open, not ruled:** ruling 9258 sets neither the number nor the window, and does not say whether the host reports the records it suppresses. The last bears on `V-Observability`, "an instrument that … stops reporting silently is worse than no instrument". These are the maintainer's call and are not decided here.

Unchanged by this decision: a line over `MAX_INCOMING_LINE_BYTES` still ends the session (lifecycle above), and a line of valid JSON that is not a well-formed message still gets `-32600` (below). `-32700` is a code of its own. It differs from the invalid-token code (`-4001`), the code for a verb outside the manifest (`-4005`) and `-32602`, so R-0010-f's three classes stay distinguishable.

**Not met today.** The host does not send the reply, and a reader must not rely on one until #4323 lands. On the SDK release landed at `de62732` (rmcp 3.7.0), by source read and untested:

- A line that is not valid JSON gets no reply. The SDK drops it, logs it at `debug`, and the stream continues (`src/transport/async_rw.rs:290-299`).
- A line of valid JSON that is not a well-formed message gets `-32600`, message `Invalid request`, with `id: null`, and the stream continues (`async_rw.rs:300-314`).
- Only a line over the cap ends the session (above).

The host's default log filter is `info` (`cmd/mnemra/logging.rs:32`, MEASURED at `de62732`), so a dropped line leaves no trace in the host's log. INHERITED from the #4156 review (activity 8423): the release the host used before the 2.x line (1.7.0) replied `-32700` with a null id. No test at `de62732` pins any of this: a search of `libs/mnemra-host/tests` for `32700`, `PARSE_ERROR`, `parse_error` and `Ignoring unparsable` found nothing, where the same search for `INVALID_PARAMS` did. #4323 (implementation lane, open) closes the gap: it adds the reply in host code and a test in `libs/mnemra-host/tests/mcp_server.rs` that pins the criteria above.

The threat-model row `P-mcp-handler` (tampering) in `docs/src/architecture/overview.md` (`:628` at `0076b25`) lists "framing-error fail-shut" and a structured `parse_error` reply among its mitigation candidates. The parse-error reply is the decided behaviour, not yet met (#4323). "Framing-error fail-shut" is not: a line that is not valid JSON leaves the session open, and only a line over the cap fails shut. The row carries an in-place note saying so.

**Decision: Decision B's revisit tripwire does not fire, on the host's overrides of SDK behaviour or on the parse-error reply.** The tripwire fires on "a concrete `rmcp` limitation that blocks a spec requirement".

1. The parse-error reply is now required, through R-0010-a's conformance target (ruling 9183). The SDK does not send it, but that blocks the requirement only if host code cannot send it on top of the SDK. INFERRED, not tested: host code can, from a wrapper around the SDK `Transport` that sees each parse failure and can write to the stream. That is not the seam the host already holds. `LineCappedReader` (`libs/mnemra-host/mcp/transport.rs:34-97`) wraps the read side, below the SDK's parser, and cannot see a parse result or write a reply.
2. The host already overrides three SDK behaviours at the MCP boundary: the initialize gate `InitializeGated` (`gate.rs:35-105`), the reader `LineCappedReader` (`transport.rs:34-97`) and the `initialize` override (`server.rs:543-553`). Each replaced, in host code, an SDK default the host does not accept, so none blocked a requirement, and the tripwire's text fired on none of them. They do falsify this record's claim about the bespoke surface at the boundary (Superseded, below). The tripwire counts limitations that block a requirement, not overrides. Whether a count of overrides should become a revisit condition is the maintainer's call, and it is not decided here.

The tripwire fires if #4323 finds that the reply cannot be sent from host code on top of the SDK. That finding announces itself: #4323's work is building that reply, so it cannot fail unnoticed. *(Anchors: Decision B's tripwire as written above, "Absent such a concrete, requirement-blocking issue, `rmcp` stands"; `P-Defer` DF1, the self-announcing outcome.)* *(Refined 2026-10-29, #4249 activities 9244 and 9270: the self-announcing outcome holds only while #4323 is worked. #4323 is tracked work in the implementation lane, open, and this record does not schedule it. Warden's review (9244 M4) also weakens the INFERRED premise of point 1, that a wrapper around the SDK `Transport` can write the reply to the stream. The SDK's writer is private (rmcp 3.7.0 `src/transport/async_rw.rs:68`), so a reply written beside it could interleave with the SDK's own writes. Meeting the whole-line write criterion above needs a write path the host owns, which is more than point 1 inferred, and whether host code can build one on top of the SDK is what #4323 finds out.)*

**Rejected alternatives for a line that is not valid JSON:**

- **No reply**, the SDK's behaviour. The SDK states that this matches the other official MCP SDKs, which ignore unparsable input (`async_rw.rs:293-296`; INHERITED, not checked against those SDKs), and the #4156 review recommended it, to avoid reply storms. Rejected: conformance requires the reply (ruling 9183); the reply-storm risk assumes a peer that echoes invalid data back, which stdio does not have (below); and the drop is silent at the host's default log filter.
- **End the session**, as for an over-cap line. Of the three options, only this one frees a client waiting on a lost request, because the client sees end of stream. Rejected: the ruling keeps the session open, and this costs the whole session for one bad line.

Security conditions of the reply, INHERITED from Warden's review of this amendment (2026-10-25), on which ruling 9183 relies:

- There is no amplification on stdio.
- The reply gives no oracle, provided its message is constant and never echoes the line or the parser's detail. The constant-message criterion above carries this condition. *(Refined 2026-10-29, #4249 activities 9244 and 9270: the criterion that the whole response line is constant now carries this condition, and it binds `error.data` and every other field as well as the message.)*
- The client that sent the line is left waiting whether or not it is answered. Invalid JSON carries no id, so no reply can be matched to its pending request. The reply makes the fault visible on the stream; it does not free the request.

**Superseded (the text above is kept as the record):**

- the frontmatter summary's "R-0010-a mandates conformance to MCP 2025-06-18";
- the Decision Drivers bullet "R-0010-a mandates conformance to an external wire contract", which quotes R-0010-a with 2025-06-18;
- item 1 of "Why the SDK is right on its merits", for the same quotation;
- the Decision B tripwire's example "R-0010-a's MCP-2025-06-18 conformance", which now reads 2025-11-25; the tripwire is otherwise unchanged;
- in Scope, "R-0010-a is implementation-agnostic and stays UNCHANGED" and its quotation. R-0010-a's target changes. This record does not edit the spec, and the V0 spec's text still reads 2025-06-18 until that spec's own amendment is made (Follow-ups). Until then, this section is where the move is recorded;
- the Consequences line "MCP-2025-06-18 wire conformance comes from the official, conformance-tested SDK";
- the official-SDK Pro "Conformant to MCP 2025-06-18 (R-0010-a)";
- the Consequences line's second sentence, "The auth/dispatch trust boundary carries the minimum bespoke surface — only mnemra's `ServerHandler` policy logic, not the JSON-RPC framing." At `de62732` the host also carries, at that boundary, the initialize gate, a reader on the framing path and an `initialize` override (point 2 above). Each is small and pinned by tests, but "only … policy logic" no longer holds;
- item 4's closing sentence in "Why the SDK is right on its merits", "Only the wire transport/framing changes hands …", for the same reason.

**Kept (not superseded):**

- Decision B Option 2 ("Implement the MCP 2025-06-18 wire contract …") and the hand-roll Con that cites MCP 2025-06-18. They describe the rejected option as it was weighed on 2026-06-19.
- the first 2026-10-25 amendment's Scope line naming 2025-06-18. It states what that amendment left alone.
- the frontmatter summary's "mnemra's R-0010 handler logic … rides on top as the rmcp ServerHandler, unchanged by the SDK choice". It is still true: the R-0010 logic it lists is unchanged and is still the host's `ServerHandler`. It carries a note because it can be read as the whole of the host's code at the boundary, and the three overrides above sit beside it. The ADR index row's matching phrase carries the same note.

**Follow-ups: PARKED, not deferred.** These records cite 2025-06-18 as the target, or make a claim this amendment corrects. They are amended by #4322, the second phase of #4249. #4119, which held the first two files, has landed (`0e82b3e`), so #4322 is no longer blocked. Nothing fires it, no cadence that resurfaces it is named in this record, and #4249 stays open until #4322 is done. *(Relabelled 2026-10-29, #4249 activity 9270: the PARKED label in this paragraph's heading is withdrawn. #4322's trip-wire, #4119 landing, has fired (`0e82b3e`), so #4322 is unblocked follow-up work, the second phase of #4249, and not a deferral or a parked item.)*

- `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`: R-0010-a; the Protocol line of the "MCP transport" contract; the Constraints line "Must use MCP specification 2025-06-18".
- `docs/src/architecture/overview.md` (line numbers at `0076b25`): the hard-constraints table row that names MCP specification 2025-06-18 (`:62`), and the `P-mcp-handler` tampering row's "explicit MCP 2025-06-18 input validation" (`:628`). That row's parse-error reply and "framing-error fail-shut" already carry this amendment's in-place note.
- `docs/src/adrs/P-0013-plugin-invocation-model.md` (`:100`): its present-tense gloss of R-0010-a, "single MCP server, 2025-06-18, stdio". #4119 does not hold that file, but the sentence describes what the spec currently pins. It stays true until R-0010-a is amended and goes stale at that moment, so it is amended with R-0010-a.
