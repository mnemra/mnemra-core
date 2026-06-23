---
title: "P-0013: Plugin Invocation Model (typed per-verb exports)"
summary: "The host↔guest invocation ABI that P-0012 explicitly scoped OUT — the gap on the export/invocation side of the WIT boundary. Locked by the maintainer (2026-06-20): every content plugin exports a fixed typed `content` interface (create/get/list/update/delete) the host invokes per authenticated verb; the universal `run(input: string) -> string` string-dispatch export is RETIRED; symmetric typing both directions, no string-based verb-resolution path on the V0 surface. Two orthogonal distinctions are the ADR's spine: (1) V0 = a FIXED typed `content` interface via plain `wit_bindgen` (no runtime resolution); the manifest-registered/dynamic-resolution mechanism is the DEFERRED domain-verb path, out of V0 scope — these are two different mechanisms. (2) P-0013 governs the EXPORT direction (host→plugin verbs); it does not re-open P-0003's already-typed host-fn IMPORT ABI (plugin→host). Consequences record the TRUE cost as two buckets: Bucket A (ABI-agnostic component-host machinery — the bulk, owed regardless of export shape, storage-gated) and Bucket B (the typed-per-verb DELTA over a string-`run` path — light, bindgen-generated). The 'WIT churn/recompile' framing is overstated; the inflation lives entirely in mis-attributing Bucket A to Bucket B."
primary-audience: agent
---

---
status: "accepted"
date: "2026-06-20"
decision-makers: ["the maintainer"]
consulted: ["the orchestrator"]
informed: []
supersedes: null
superseded_by: null
overrides: null
---

# P-0013: Plugin Invocation Model (typed per-verb exports)

**Project:** mnemra-core

## Status

`accepted`

This [ADR](../glossary.md#adr) (Architecture Decision Record, the structured record of a technical decision and the options it was chosen against) records a decision the maintainer has already made (lock 2026-06-20). It is recorded `accepted`-as-a-forward-decision, in the same spirit as [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md)'s Decision B: the lock comes before the implementation, Task 23 implements it, so there's no open question for a gate to resolve. Only the build to follow. It's `accepted`, not `proposed`.

This ADR does **not** `supersede` or `override` any prior record. It's the sibling that fills the gap [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) explicitly scoped out. P-0012 decided the plugin *runtime* (raw Wasmtime + component model) and the *MCP SDK* (`rmcp`), and it stated that "plugin-runtime concerns … [are] out of scope for this ADR." The host↔guest *invocation contract* was left unrecorded. P-0013 records it. `supersedes` and `overrides` stay `null`.

## Context and Problem Statement

Until now the V0 plugin substrate has had no recorded host↔guest invocation contract. P-0012 decided what runs the plugin (raw Wasmtime) and what speaks MCP to clients (`rmcp`). [P-0003](P-0003-plugin-manifest.md) decided the manifest schema and the **host-fn import** ABI: the typed functions a plugin *calls*, such as `artifact-create` and `metrics-record`. Neither decided the **export/invocation** side: how the host invokes a verb *on* the plugin. The starting state in the tree is a single universal export, `export run: func(input: string) -> string` (`wit/echo.wit`), which would force the guest to parse its string input to decide which verb to run. This ADR records the decision that replaces that string-dispatch export with typed per-verb exports.

### The locked decision (verbatim — recorded, not re-opened)

The maintainer locked the invocation ABI on 2026-06-20 (activity 2402). It's recorded verbatim per [P-PreserveDecisionSpace](../glossary.md#p-preservedecisionspace) (every ADR keeps its rejected options and rationale on the record so future readers can see what was weighed), exactly as P-0012 records its own maintainer lock:

> Invocation ABI LOCKED (maintainer, 2026-06-20 — activity 2402): typed per-verb exports, manifest-registered at load. `run(input: string) -> string` RETIRED.
> - WIT world declares a **typed universal content interface** (create/get/list/update/delete) every content plugin exports; host resolves manifest-declared verbs → named typed exports at load, invokes the **exact typed export** per authenticated verb.
> - Symmetric typing both directions; **no string/JSON dispatch path** on the V0 surface.
> - Domain/non-CRUD verbs (`echo.audit`) **deferred past V0** (their ABI typed too when designed — no string hatch). Manifest-declared verb without a typed export = non-dispatchable at V0 (structured error); does **not** affect existing permission tests (`echo.audit` denial is pre-dispatch).
> - Rationale: the maintainer values strict typing for reason-about-ability + AI-tool error prevention; dislikes non-deterministic string paths; typed-as-needed (build CRUD, defer domain verbs, no per-plugin codegen).

"No string/JSON dispatch path" kills the **string-based verb-resolution** path: the single `run` dispatcher that parses its input to decide which verb to run. It does **not** mean string-typed payloads vanish from the ABI. The host-fn import surface already carries `type json = string` for frontmatter and body (`wit/host.wit`), and the typed `content` exports carry the same JSON-as-string payloads. What's removed is the guest-side string/JSON *dispatch*, the verb-selection step. Not the JSON values.

### The crux distinction this ADR exists to pin: fixed interface vs dynamic resolution

The locked decision's phrase "host resolves manifest-declared verbs → named typed exports at load" reads like runtime resolution. **It is not, at V0.** There are two different mechanisms here, and conflating them is what inflates the cost estimate:

> The V0 CRUD surface is a **fixed typed `content` interface** every plugin exports → plain `wit_bindgen`, **no runtime resolution** (the light path). The "host resolves manifest-declared verbs → named exports at load" / **dynamic-lookup** path only matters for the **deferred domain verbs** — its cost does **not** belong in the V0 estimate. Fixed-interface-bindgen and "manifest-registered/dynamic-resolution" are two different mechanisms; V0 = the former.

Stated plainly: **V0 is a fixed typed `content` interface (plain `wit_bindgen`, no registry, no runtime resolution); the manifest-registered/dynamic-resolution mechanism is the DEFERRED domain-verb path, out of V0 scope.** At V0 the manifest's `verbs` list is the **pre-dispatch capability gate** ([R-0010-d](../../specs/2026-05-24-mnemra-core-v0-substrate.md): "enforce a per-verb capability check against the plugin manifest's declared `verbs` list before dispatching"), **not** a runtime export registry. Verb→export resolution is static (compile-time `wit_bindgen` against the fixed `content` interface). Getting a reader to see fixed-interface and dynamic-resolution as two mechanisms is this ADR's central clarification.

### Scope boundary: this ADR governs the EXPORT direction, not P-0003's IMPORT ABI

The two WIT directions carry the **same CRUD shape in opposite directions**, which is easy to conflate:

- **Import direction (plugin → host), already typed, NOT re-opened here.** [P-0003](P-0003-plugin-manifest.md) and [R-0012-a](../../specs/2026-05-24-mnemra-core-v0-substrate.md) lock the universal `content.emit` host-fn ABI — the typed functions a plugin *calls*: `artifact-create: func(ctx: workspace-ctx, %type: string, frontmatter: json, body: option<string>) -> string`, `artifact-get`, `artifact-list`, `artifact-update`, `artifact-delete` (`wit/host.wit`). These are already typed and already locked. P-0013 does **not** touch them.
- **Export direction (host → plugin), the gap, governed here.** The `run(input: string) -> string` export (`wit/echo.wit`) is the string hatch on this side. P-0013 replaces it with a typed `content` interface every content plugin *exports* and the host *invokes* per authenticated verb.

P-0013 governs the **export/invocation** side only. A reader who asks "doesn't P-0003 already lock the content ABI?" is looking at the import side. This ADR is the export side P-0003 didn't cover.

## Decision Drivers

- **`P-StackDiscipline` — the typed component model is in-stack, not an added ecosystem.** Wasmtime's `component-model` feature is already a direct dependency (locked in [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) Decision A; `wasmtime = "=45.0.2"`, features `cranelift` + `component-model`, in `libs/mnemra-host/Cargo.toml`). Typed per-verb exports use `wit_bindgen` / `component::bindgen!`, already in the toolchain (`wit-bindgen` pinned `0.57` → `0.57.1`). Typed exports aren't an ecosystem addition. They're how the runtime the project already targets expresses its contracts. `P-StackDiscipline` (`brain/about/architecture-principles.md`; "reject ecosystem-misaligned tooling unless no in-stack path exists") is satisfied: the in-stack primitive covers the requirement.
- **The maintainer's stated values: strict typing for reason-about-ability and AI-tool error prevention; aversion to non-deterministic string paths.** Recorded as the rationale in the lock quote above. A typed export contract is statically checkable by the AI tooling that authors and calls plugins. A `run(string)` dispatcher pushes verb selection into runtime string parsing the toolchain can't reason about. This is the decision's leading driver, and [P-PreserveDecisionSpace](../glossary.md#p-preservedecisionspace) records it verbatim, not paraphrased.
- **Symmetric typing closes the one remaining string hatch.** The import direction is already typed (P-0003 / `wit/host.wit`). Retiring the `run(string)` export makes the boundary typed in **both** directions, so no string-based verb-resolution path survives on the V0 surface. The asymmetry (typed imports, string-dispatch export) is exactly what the lock removes.
- **Typed-as-needed, no per-plugin codegen.** The decision builds the fixed `content` CRUD interface now and defers domain/non-CRUD verbs (their ABI typed too when designed). The fixed interface is plain `wit_bindgen`: `bindgen` generates the marshaling, and there's no hand-written per-plugin codegen. This bounds the V0 cost (see Consequences) and is why the typed surface is light, not heavy.

## Considered Options

A single decision, recorded with the rejected alternative as the option the lock chose against ([P-PreserveDecisionSpace](../glossary.md#p-preservedecisionspace)). The rejected option is **not** presented as a live choice. It's recorded so a future reader sees what was weighed.

1. **Typed per-verb exports — a fixed typed `content` interface (chosen).** The WIT world declares a fixed `content` interface (create/get/list/update/delete) every content plugin exports. The host invokes the exact typed export per authenticated verb. Plain `wit_bindgen` against a fixed interface, no runtime resolution at V0. Both ABI directions typed, no string-based verb-resolution path. Domain/non-CRUD verbs deferred (typed when designed).

2. **Universal `run(input: string) -> string` string-dispatch export (rejected — the starting state).** Every plugin exports one `run` function; the host passes a string; the guest parses the string (or a JSON envelope inside it) to decide which verb to run and what to do. One export, full guest-side flexibility, at the cost of a non-deterministic string-dispatch path the host and the AI toolchain can't reason about statically, and a verb-selection step the type system doesn't check. This is the export currently in `wit/echo.wit`. The lock retires it.

## Decision Outcome

**Chosen: typed per-verb exports — a fixed typed `content` interface (Option 1)**, because strict typing on the export side (matching the already-typed import side) gives the reason-about-ability and AI-tool error-prevention the maintainer values, and the fixed-interface form makes that typing **light**: plain `wit_bindgen`, no runtime resolution, no per-plugin codegen.

The maintainer's lock and its rationale are recorded verbatim in Context above ([P-PreserveDecisionSpace](../glossary.md#p-preservedecisionspace)). The `run(string)` string-dispatch export is the rejected alternative, not an open option.

### What V0 is, precisely

- **A fixed typed `content` interface**, declared once in the WIT world, exported by every content plugin: `create` / `get` / `list` / `update` / `delete`. Plain `wit_bindgen` on the guest; `component::bindgen!` (or a hand-rolled `component::Linker`) generates the typed export accessors on the host. **No runtime resolution** — the export set is fixed at compile time.
- **The manifest `verbs` list is the pre-dispatch capability gate, not a runtime export registry.** [R-0010-d](../../specs/2026-05-24-mnemra-core-v0-substrate.md) checks the authenticated verb against the manifest's declared `verbs` *before* dispatch; the verb→export resolution itself is static (the fixed `content` interface), not a load-time registry lookup. The exact MCP-verb → CRUD-method mapping rule is **not** pinned by this ADR (it isn't yet specified in the locked spec) and is a forthcoming spec-amendment concern (see Scope, below), not an invention recorded here.

### Deferred: domain/non-CRUD verbs (named tripwire, `P-Defer`)

Domain/non-CRUD verbs (`echo.audit` is the named example) are **deferred past V0** under [P-Defer](../glossary.md#p-defer) (don't choose a mechanism until evidence forces it; a named tripwire fires the choice). Their ABI will be typed too when designed; there's no string hatch for them either. Two consequences of the deferral are recorded so it isn't silent:

1. **Non-dispatchable at V0, as a structured error.** A manifest-declared verb with no matching typed export is non-dispatchable at V0 and surfaces a structured error. This is **independent of the permission path**: the `echo.audit` denial is **pre-dispatch** — `is_write_verb` / `auth_and_authorize` run before any export dispatch (`libs/mnemra-host/mcp/dispatch.rs:52`, `:73`). The deferral does **not** reshape auth and does **not** affect the existing permission tests.
2. **The dynamic-resolution mechanism is the deferred path, not V0.** The "host resolves manifest-declared verbs → named typed exports at load" / dynamic-lookup mechanism (a runtime registry mapping manifest verb-name → a typed export handle) is what the deferred domain verbs would need. It's a *different mechanism* from the fixed-interface bindgen V0 uses. It's out of V0 scope.

> **Deferral / revisit tripwire (named instrument, `P-Defer`).** The named tripwire that fires the design of the deferred domain-verb ABI (and the dynamic-resolution mechanism it needs) is **the first domain/non-CRUD verb that must dispatch at the V0 surface** — i.e., a required verb that the fixed `content` CRUD interface cannot express. Absent such a verb, the fixed interface stands and the dynamic-resolution path is not built. A preference for generality is not a firing condition.

### Scope — implementation-decision record; the spec is changed by a separate amendment, not by this ADR

This ADR records *what the invocation contract is* and *why*. It does **not** alter the spec, in the same way [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) records *how* the substrate is built without editing the spec. The locked, verified spec [`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`](../../specs/2026-05-24-mnemra-core-v0-substrate.md) currently pins the **import** side (the host-fn ABI — [R-0012-a](../../specs/2026-05-24-mnemra-core-v0-substrate.md): "the host-fn ABI SHALL declare the universal `content.emit` verb shape … `artifact.create`, `artifact.update`, `artifact.get`, `artifact.list`, `artifact.delete` …") and the **MCP front** ([R-0010-a](../../specs/2026-05-24-mnemra-core-v0-substrate.md): single MCP server, 2025-06-18, stdio; [R-0010-c](../../specs/2026-05-24-mnemra-core-v0-substrate.md): `DF-auth-check` before routing; [R-0010-d](../../specs/2026-05-24-mnemra-core-v0-substrate.md): per-verb capability check against the manifest's `verbs`; [R-0006-b](../../specs/2026-05-24-mnemra-core-v0-substrate.md): single `WorkspaceCtx` construction after token validation). It does **not** currently pin the guest `run()` / invocation envelope (the export-side contract). That gap is closed by a **separate, named spec amendment**, the next step in the sequence, **not** this dispatch. P-0013 notes the gap and references the forthcoming amendment as follow-up. The spec is read-only here.

### Consequences

The "WIT change = churn/recompile" framing for this decision is **overstated**, proved against the installed toolchain (`wit-bindgen` 0.57.1, `wasmtime` =45.0.2, guest `wasm32-wasip2`) by the Forge cost assessment (dispatch 1060, `scratch/dispatch-1060-wit-cost-assessment.md`). The cost is recorded as **two explicitly-labelled buckets** so canon reflects reality and the decomposition (the Task-23 re-decompose) and the spec amendment are sized correctly. The decisive fact: **the host has zero component-model wiring today**, and standing that up is owed regardless of export shape.

**Good:**

- **Symmetric typing.** With the `run(string)` export retired, the host↔guest boundary is typed in both directions — typed host-fn imports (P-0003) and typed `content` exports — and no string-based verb-resolution path survives on the V0 surface. This is the property the lock buys: reason-about-ability and static checkability for the AI tooling that authors and calls plugins.
- **The typed surface is light (Bucket B).** Going from one `run(string)->string` export to five typed `content` exports adds ~5 WIT lines + one interface, four extra guest functions, and four extra **generated** host accessors. `bindgen` generates the marshaling: no hand-written marshaling, no per-plugin codegen. The toolchain already marshals typed values across the WIT boundary today. `wit/host.wit` declares `artifact-create` as a **typed host-fn import** that already crosses the boundary, so going 1→5 typed exports just adds generated accessors of the same kind.
- **The recorded contract closes the gap P-0012 named.** A future reader sees which side of the WIT boundary each decision governs — P-0003/R-0012-a the import side, P-0013 the export side — and that the V0 mechanism is fixed-interface bindgen, not a runtime registry.

**Bad / Trade-offs:**

- **Bucket A — the ABI-agnostic component-host machinery — is the real V0 weight, and it is owed regardless of export shape.** The host today has **no** component-model invocation path: the pool instantiates **core** `wasmtime::Module`s with no imports (`libs/mnemra-host/plugin/pool.rs:293`: `Instance::new(&mut store, module, &[])`; the pool holds `wasmtime::Module`, not `wasmtime::component::Component`, per `:84`, `:232`), not `component::Component`; the trap-recovery path uses a core-module, parameterless `get_typed_func::<(), ()>(&mut *store, "run")` fixture (`libs/mnemra-host/plugin/trap_recovery.rs:394`, `:385`); the host-fn import bodies are `todo!()` stubs; and `mcp/server.rs`'s `call_tool` returns `Ok(CallToolResult::default())` after `auth_and_authorize` with no verb→export routing (`libs/mnemra-host/mcp/server.rs:164`, `:168`; "future task" at `:25`, `:29`, `:133`). The guest builds as a *component* but the pool can't load it, an impedance mismatch. Standing up the component-host invocation path (component `Linker` + host-fn import bodies + component instantiation in the pool + `call_tool` → export routing) is the true content of V0. **A `run(string)` path would owe nearly all of it too**, so it is **not** the cost of the typed-export decision. The host-fn import bodies are **storage-gated** (Task 5/21: the `artifact-*` bodies need real storage, not `todo!()`), which is the single largest driver of V0 timing and is orthogonal to export shape.
- **The "WIT churn/recompile" inflation lives entirely in mis-attributing Bucket A to Bucket B.** The verdict to record in canon: typed-per-verb is light *given the component-host machinery exists*; that machinery doesn't exist yet and is owed regardless of export shape. Charging Bucket A to "typed exports" reproduces the inflated framing in a new costume. The decomposition (and the spec amendment) must size Bucket B (the typed delta) only, and must not let Bucket A inflate the typed-ABI line.
- **The trap/replace path is pinned to the current core-module `run` shape.** `trap_recovery.rs:394` hard-codes `get_typed_func::<(),()>("run")`, and the kill-and-replace tests are pinned to the current core-module shape; moving to component exports rewrites this to the component-typed accessor (counted in Bucket A — component instantiation) and re-pins those tests. Flagged so the re-decompose budgets the re-pin.

## Pros and Cons of the Options

### Typed per-verb exports — fixed `content` interface (chosen)

- Pro: Typed both directions — matches the already-typed import ABI (P-0003); no string-based verb-resolution path on the V0 surface; statically checkable by the AI tooling that authors and calls plugins.
- Pro: In-stack — `wasmtime` `component-model` and `wit-bindgen` are already direct dependencies (P-0012); `P-StackDiscipline`-aligned (the in-stack primitive covers the requirement; no added ecosystem).
- Pro: Light (Bucket B) — `bindgen` generates the export accessors and marshaling; the 1→5 export swap is ~5 WIT lines + 4 guest functions + 4 generated accessors; no per-plugin codegen.
- Pro: Fixed interface, no runtime resolution at V0 — the manifest `verbs` list is a pre-dispatch capability gate, not a runtime export registry; the dynamic-resolution mechanism is deferred with the domain verbs.
- Con: Domain/non-CRUD verbs are deferred — the fixed `content` interface expresses CRUD only; the first domain verb that must dispatch at the V0 surface fires the deferred design (named `P-Defer` tripwire). Accepted: V0 content plugins are CRUD-shaped; typed-as-needed avoids building the dynamic-resolution mechanism before a verb requires it.

### Universal `run(input: string) -> string` string-dispatch export (rejected)

- Pro: One export per plugin; maximal guest-side flexibility (any verb dispatched by parsing the input).
- Con: A non-deterministic string-dispatch path on the export side — verb selection happens in runtime string parsing the host and the AI toolchain can't reason about or statically check. Directly against the maintainer's stated values (strict typing, no non-deterministic string paths).
- Con: Asymmetric — the import direction is already typed (P-0003 / `wit/host.wit`); a `run(string)` export leaves one untyped string hatch on the boundary.
- Con: Doesn't avoid Bucket A. A `run(string)` path needs essentially the same component-host machinery (Linker, host-fn bodies, component instantiation, `call_tool` routing); it saves only the light Bucket B delta. So the rejected option is *not* meaningfully cheaper, while it forfeits the typing the decision is for.

## More Information

**The ADR P-0013 extends (related precedent, not a supersession).** [P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) ("Plugin Runtime (raw Wasmtime) + MCP SDK (`rmcp`)") decided the plugin execution runtime and the MCP SDK and **explicitly scoped the invocation ABI out** ("plugin-runtime concerns … out of scope for this ADR"). P-0013 fills that gap, the host↔guest invocation contract, and mirrors P-0012's `accepted`-as-forward-decision framing (the lock precedes the implementation; Task 23 implements). It's **not** a supersession and **not** an override: a sibling that completes the substrate. `supersedes`/`overrides` stay `null`.

**Related precedent — the manifest and the import ABI.** [P-0003](P-0003-plugin-manifest.md) ("Plugin Manifest") locks the manifest schema and the typed **host-fn import** ABI (the universal `content.emit` surface a plugin *calls*: `artifact-create`, `artifact-get`, …). The manifest's declared `verbs` list is what [R-0010-d](../../specs/2026-05-24-mnemra-core-v0-substrate.md) checks pre-dispatch, the precedent the invocation model builds on. P-0013 governs the export side P-0003 didn't cover. No contradiction: opposite WIT directions of the same CRUD shape.

**Related context (minor).** [P-0007](P-0007-plugin-resource-limits.md) ("Plugin Resource Limits") — the per-invocation fuel/epoch budget is applied on the invocation path this ADR governs (one budget per typed-export call, as it would be per `run` call). [P-0002](P-0002-core-plugin-partition.md) ("Core Plugin Partition") — which verbs are `core: true` content verbs (the plugins that export the typed `content` interface) vs builtins. Neither decides the invocation ABI; both are context.

**Canon anchors:**

- `P-StackDiscipline` (`brain/about/architecture-principles.md`): the typed component model is in-stack — `wasmtime`'s `component-model` feature and `wit-bindgen` are already direct dependencies (P-0012). Typed exports are how the existing runtime expresses contracts, not an added ecosystem.
- `P-PreserveDecisionSpace` (`brain/about/architecture-principles.md`): the rejected `run(string)->string` string-dispatch export is recorded as the option the lock chose against, with the maintainer's verbatim rationale; it isn't presented as a live choice.
- `P-Defer` (`brain/about/architecture-principles.md`): the deferred domain/non-CRUD verbs (and the dynamic-resolution mechanism they would need) carry a named firing tripwire — the first domain verb that must dispatch at the V0 surface — not a silent deferral.

**The cost source.** The two-bucket cost shape recorded in Consequences is the Forge cost assessment (dispatch 1060, Task 1737, `scratch/dispatch-1060-wit-cost-assessment.md`), which priced the locked ABI against the installed toolchain and proved the "churn/recompile" framing overstated. It's the source of the Bucket A / Bucket B split and of the code-state facts cited above.

**References (paths confirmed to resolve in-repo this session):**

- The export string hatch being retired: `wit/echo.wit` (`export run: func(input: string) -> string`).
- The already-typed import ABI (not re-opened): `wit/host.wit` (`interface artifact { artifact-create: func(ctx: workspace-ctx, %type: string, frontmatter: json, body: option<string>) -> string; … }`; `type json = string`).
- Component-host machinery gap (Bucket A): `libs/mnemra-host/plugin/pool.rs:293` (core-module `Instance::new(&mut store, module, &[])`, no imports; `wasmtime::Module` per `:84`, `:232`); `libs/mnemra-host/plugin/trap_recovery.rs:394`, `:385` (core-module `get_typed_func::<(), ()>(&mut *store, "run")` fixture); `libs/mnemra-host/mcp/server.rs:164`, `:168` (`call_tool` returns `Ok(CallToolResult::default())`; "future task" at `:25`, `:29`, `:133`).
- Pre-dispatch permission path (unaffected by the deferral): `libs/mnemra-host/mcp/dispatch.rs:52` (`is_write_verb`), `:73` (`auth_and_authorize`).
- Toolchain pins: `wit-bindgen` `0.57` → `0.57.1`; `wasmtime = "=45.0.2"` (features `cranelift` + `component-model`, `libs/mnemra-host/Cargo.toml`); guest target `wasm32-wasip2`.
- Spec requirements (read-only; the surrounding contract): [R-0006-b](../../specs/2026-05-24-mnemra-core-v0-substrate.md), [R-0010-a](../../specs/2026-05-24-mnemra-core-v0-substrate.md), [R-0010-c](../../specs/2026-05-24-mnemra-core-v0-substrate.md), [R-0010-d](../../specs/2026-05-24-mnemra-core-v0-substrate.md), [R-0012-a](../../specs/2026-05-24-mnemra-core-v0-substrate.md), in [`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`](../../specs/2026-05-24-mnemra-core-v0-substrate.md).

**Follow-up:**

- **A separate spec amendment** adds the guest `run()` / invocation-envelope requirement (the export-side contract) to the locked spec — the spec currently pins the import-side host-fn ABI (R-0012-a) and the MCP front (R-0010-a/c/d, R-0006-b) but not the export/invocation envelope. That amendment is the next step in the sequence; this ADR only records the decision it will encode.
- **The Task-23 re-decompose** sizes Bucket B (the typed-export delta) only; Bucket A (the component-host machinery + host-fn import bodies) is the ABI-agnostic bulk, storage-gated (Task 5/21), and must not inflate the typed-ABI line. Confirm with the re-decompose which task owns the host-fn bodies vs the invocation wiring, and budget the trap/replace re-pin.
