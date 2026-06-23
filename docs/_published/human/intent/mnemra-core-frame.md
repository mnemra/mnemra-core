---
title: "Frame: Mnemra Core"
summary: "Architectural frame for mnemra-core — system boundary, component map, cross-cutting decisions, and open ADR slots — reconciled to the locked product brief."
primary-audience: agent
---

# Frame — Mnemra Core

**Date:** 2026-05-22 · **Status:** draft (Stage 2 of `/brief`) · **Altitude:** component

> Format note: this is the **Stage 2 (Frame)** output of the `/brief` pipeline. It sits between
> the locked product brief (Stage 1, Intent) and the per-feature specs (Stage 3). The Frame's
> job is to translate the product intent's purpose and hard constraints into an architectural
> shape: system boundary, component map, the key cross-cutting decisions, and the open
> ADR-shaped questions the spec stage will need to lock. The Frame **refines** the brief; it
> does not contradict it. Where a tension surfaces between Frame and brief, the Frame body
> records the tension explicitly and flags it for resolution rather than silently choosing.
>
> Source-of-truth artifacts (this Frame, the architecture overview, and ADRs) are
> agent-primary per the workspace-wide agent-primary-source-artifacts decision (locked
> 2026-05-11). Human views are derivative.

## Provenance

| Artifact | Reference |
|---|---|
| Locked product brief (Stage 1, intake-exit gated) | [Product Brief: Mnemra Core](mnemra-core.md), locked 2026-05-20 |
| V0 architecture discovery (high-stakes, zero-new-finding stopping rule) | "V0 discovery" decision record, locked 2026-05-02 |
| V0 architecture constraints (high-stakes; constraint inventory, QA tree, DFD, threat scaffold) | "V0 constraints" decision record, draft 2026-05-03, partially superseded by INCR-1 resolution in the brief (see Framing corrections below) |
| Architecture alignment mental model (round-6) | "Architecture alignment R6" decision record, agreed 2026-04-27 |
| Substrate shape exploration (pre-discovery, background only) | "Substrate shape exploration" exploration note, 2026-05-02 |
| Architecture overview port | [Architecture overview](../architecture/overview.md), a companion artifact, ports constraint inventory + QA utility tree + DFD + threat scaffold |
| Project defaults (G-* baseline) | [Project Defaults](../adrs/DEFAULTS.md) |

This Frame reconciles every architectural claim against the locked brief's Hard constraints
and the staged-increment sequence (`0.1.0` host core through `1.0.0` dogfood cutover) it locked
at intake-exit (2026-05-20).

## Stage 2a — Elicitation input record

This section records the maintainer's architectural directions that anchored Stage 2b
synthesis. Stage 2a was run **retroactively** against this Frame on 2026-05-23. The
Frame-exit gate ran on the 2026-05-22 synthesized output (the result of the original lock
under the warm-start two-touchpoint shape), returned a Revise verdict, and the four
architectural directions below were locked through a Stage 2a-shaped walkthrough against
the Warden Stage 2 code-and-security review's findings. The directions feed every revision in
this Frame. Later Spec-stage ADRs anchor on these slot descriptions, not on the original
Warden findings.

Origin: 2026-05-23 G-0028 cold-start amendment (calibration phase, N=5 Frame-exit-cohort
trip-wire). Mnemra-core is the first cold-start exercise of the new shape.

| Direction | Warden finding addressed | One-liner |
|---|---|---|
| Split `{{P-SigningKeyCustody}}` into Tier-A `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)` (V0 build-host-on-disk for dogfood + multi-deployment trip-wire) and Tier-C `{{P-SigningKeyCustodyHardening}}` (HSM, runtime-fetch, never-on-node) | H1 | V0 ships signed `core: true` plugins under a stated-and-tripwire'd custody decision rather than under a deferred one. Per [P-Defer](../glossary.md#p-defer) (defer mechanism choice until evidence forces it). |
| Add Tier-A `[P-0006-v0-tenant-enforcement](../adrs/P-0006-v0-tenant-enforcement.md)` with **typed `WorkspaceCtx` parameter binding at the host-fn boundary** as the conservative lead, with storage-layer query rewriter and per-host-fn `workspace_id` parameter validation as open variants | H2 | The V0 enforcement layer for workspace isolation is named at Frame altitude; RLS at V0.1+ is the substrate-layer hardening of an enforcement that is already load-bearing at V0. Per `P-SecurityLayered` (each layer is independently load-bearing). |
| Rename Tier-C `{{P-PluginPoolMemory}}` to `[P-0007-plugin-resource-limits](../adrs/P-0007-plugin-resource-limits.md)` and promote to Tier A; V0 turns on Wasmtime fuel + epoch-interruption + memory ceiling + table/instance limits | M3 | The plugin-sandbox DoS-containment outcomes the Frame commits to (kill-and-replace on infinite-loop, no single-process-wide DoS via plugin) require the mechanism named at Frame altitude. Per `P-StackDiscipline` (use the stack's own knobs rather than building new ones). |
| Split `{{P-RLSAdminToken}}` into Tier-A `[P-0008-admin-token-shape](../adrs/P-0008-admin-token-shape.md)` (opaque-vs-claim-carrying token structure + workspace-claim binding mechanism) and Tier-A `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (role model + permission shape, downstream of `[P-0008-admin-token-shape](../adrs/P-0008-admin-token-shape.md)`) | M4 | Token *shape* gates security architecture and belongs at Frame altitude; role + permission is mechanism downstream of shape. |

The four directions above anchored revisions H1, H2, M3, and M4. Three additional Warden
Mediums (M1 trust-boundary reconciliation, M2 DFD-builtin coverage, M5 core-plugin
partition forward-reference) were absorbed as mechanical revisions without further
maintainer input. Their fixes are scoped tightly to Warden's spec.

## Framing corrections

The Frame inherits work from artifacts that predate the brief's intake-exit lock. Three
framings the older inputs carried are stale and corrected here. Each correction cites the
brief section that authorizes it.

### Correction 1 — `projects` and `agents` are builtin, not plugins

**Older framing.** The constraints strawman draft (2026-05-03) drew the data-flow diagram
with `P-core-plugin-projects` and `P-core-plugin-agents` as subgraph nodes inside the WASM
`TB-plugin-sandbox` trust boundary, and the alignment record (2026-04-27, round 2) framed
core identity as `core: true` plugins.

**Corrected framing.** The brief's `0.1.0` substrate description (Proposed tier, locked
2026-05-20) makes `projects` and `agents` **builtin** components of the host, not plugins:

> Projects and agents are **builtin**, not plugins: plugins are scoped per project, so a
> project cannot itself be a plugin (a host bootstrap chicken-and-egg).

The plugin model (also locked in Hard constraints) is this. WebAssembly Component Model
modules hosted in-process via Wasmtime, with IO-free plugin core logic; all plugin IO MUST
be mediated by host-provided functions; plugins are leaves with no sideways linkage;
cross-plugin calls are host-mediated. That model does not accommodate identity-bearing
components. By construction they sit above the plugin boundary.

**Why the change.** Per-project plugin scoping makes "projects" a plugin a chicken-and-egg
bootstrap problem. The brief's INCR-1 resolution (locked 2026-05-20) records the upstream
resolution: builtin substrate first. The Frame surfaces the consequence on the system
boundary. The DFD nodes for `projects` and `agents` move out of the plugin sandbox into
the host.

### Correction 2 — "stdio MCP wrappers around external CLIs" is not the plugin shape

**Older framing.** A class of MCP-server tooling models plugins as stdio wrappers spawning
external CLI processes (an external-process-per-tool pattern that the project context
describes as the framing to reject).

**Corrected framing.** Plugins are **WebAssembly Component Model modules loaded
in-process by Wasmtime**, communicating with the host via WIT-defined host functions.
This shapes downstream decisions:

- **Core logic is IO-free.** Plugin Rust crates declare no `std::fs`, no `std::process`,
  no stdin/stdout. Inputs and outputs are structs over the WIT contract.
- **IO is host-provided.** A plugin declares what IO categories it needs (file reads,
  network, clock, state); the host implements them.
- **A native CLI binary is a separate frontend.** A tool can ship both a native CLI
  (workspace-era ergonomics) and a WASM plugin (mnemra-era), sharing an IO-free core
  crate. The CLI wires native IO; the plugin wires host-fn IO.

The "stdio-wrapper" shape does not port to mnemra. The Frame designs for IO-free core plus
thin host-fn adapters from day one.

### Correction 3 — V0 is a staged increment sequence, not a monolithic release

**Older framing.** The constraints strawman draft (2026-05-03) treated V0 as a single
monolithic deliverable; some constraint phrasing assumed a one-shot cutover.

**Corrected framing.** The brief's INCR-1 resolution (locked 2026-05-20) decomposes V0
into a builtin-substrate-first, one-capability-per-increment staged sequence:

| SemVer | Capability family | Brief reference |
|---|---|---|
| `0.1.0` | Builtin substrate + host core (Postgres + extensions; host-fn ABI; MCP server skeleton; admin CLI; observability minimum; LLM-API-key config; tenancy/identity core) | Proposed tier |
| `0.2.0` | Task management | Proposed tier |
| `0.3.0` | Dispatch metrics & lifecycle | Proposed tier |
| `0.4.0` | Skill-run measurement | Proposed tier |
| `0.5.0` | Activity / audit log | Proposed tier |
| `0.6.0` | Collaboration session friction tracking | Proposed tier |
| `0.7.0` | Repo registry | Proposed tier |
| `0.8.0` | Relationships / edges | Proposed tier |
| `0.9.0` | Tags / taggings | Proposed tier |
| `0.10.0` | Dependency-approval state | Proposed tier |
| `0.11.0` | Scope-violation log | Proposed tier |
| `0.12.0` | Job-search pipeline | Proposed tier |
| `0.13.0` | Contacts | Proposed tier |
| `0.14.0` | Content-corpus migration | Proposed tier |
| `1.0.0` | Dogfood cutover (public API defined), the V0 milestone | Proposed tier |

The Frame's component map (below) reconciles to this sequence. Capability families that
land in distinct increments share the substrate `0.1.0` provides; each downstream
increment adds verbs to the MCP server skeleton without changing the host-fn ABI's shape
(the ABI is pre-1.0 within the `0.y.z` band per SemVer §4: backward-compatible additions
are minor bumps, breaking changes accumulated for `1.0.0`).

## System boundary

### What is in mnemra-core

Mnemra-core is the **single-process server binary** that ships the substrate, the host,
the plugin runtime, and the builtin components. Per the brief's Hard constraints:

- The agent-facing surface is **MCP-native** (MCP specification 2025-06-18). Transport is
  **stdio at V0**; streamable-HTTP is a later-version activation.
- The substrate is a **single-process Postgres** instance with `pgvector` (bundled with
  the embedded engine); TimescaleDB is demoted off the V0 stack (P-0010 D8).
- Plugins are **WebAssembly Component Model modules** hosted in-process via Wasmtime.
- Deployment posture is **self-hosted-first, single-binary**. The system MUST NOT host a
  language model; it calls out to an external one.

The "single-binary" constraint binds the **server**, not the deployment packaging. An
immutable image or appliance is a valid packaging shape (per brief Hard constraints).
Native compile-time asset embedding is not used (project default `G-0002`: assets are
served at runtime, with a packaging-time copy into the deployment image).

### What is adjacent to mnemra-core (referenced, not absorbed)

The brief explicitly carves out four adjacent components that the Frame must name but not
absorb into mnemra-core's component map:

| Adjacent component | Relationship to mnemra-core |
|---|---|
| The dispatch CLI (external component, sibling repository) | Build-time dependency: gates the start of the `0.1.0` build sequence (consumes the intent → frame → spec pipeline output); later runs as a mnemra plugin (a native CLI and a WASM plugin sharing an IO-free core) |
| The spec-delta/merge tool (external component, sibling repository) | Build-time dependency: the structured-delta consumer this brief's living-document format forward-contracts with; later runs as a mnemra plugin |
| The markdown review/annotation tool (sibling product, tentative) | Hosted under the mnemra umbrella when published; relationship to mnemra-core is downstream, not absorbed |
| The landing site (`mnemra.dev`) | Separate repository; not a runtime component of mnemra-core. Marketing-tier dates appearing on the landing **MUST NOT** weight architectural tradeoff analysis (brief Hard constraints) |

The workspace tooling that mnemra-core's `1.0.0` cutover replaces (the prior structured
task store plus the prior markdown content corpus) is the **migration source** for the
`0.2.0` through `0.14.0` increments. It is not a runtime adjacency of the running mnemra-core
server; it is one-shot import scope.

### Trust boundaries

The Frame inherits eight steady-state trust boundaries from the constraints draft
(corrected per Framing correction 1; the predecessor's `TB-fs-source` and `TB-fs-backup`
are present in the architecture overview's data-flow diagram but are migration-and-
backup-scoped and not part of the steady-state trust topology):

| Boundary | What it contains | What crosses it |
|---|---|---|
| `TB-agent-runtime` | MCP-client agents (separate processes hosting MCP clients) | The MCP transport (stdio at V0; streamable-HTTP V0.1+) |
| `TB-human` | The operator invoking the admin CLI | The admin CLI's local IPC to the running mnemra-core server, or a direct admin token over MCP |
| `TB-mnemra-host` | The mnemra-core server process itself | Crosses MCP transport, admin CLI IPC, Postgres connection, filesystem secrets reads |
| `TB-plugin-sandbox` | The Wasmtime sandbox running plugin code | All cross-boundary calls are host-fn-mediated; plugin core is IO-free |
| `TB-postgres` | The Postgres process | SQL connection from mnemra-core host; no agent or plugin code reaches Postgres directly |
| `TB-fs-secrets` | Filesystem-stored secrets (mode 600) | Admin token and (per the open ADR slot for signing-key custody) plugin-signing key material; read by host code only |
| `TB-build-pipeline` | Conceptual, the signing authority at build time | Signed plugin artifacts are what crosses into runtime; runtime sees signatures, not the key |
| `TB-external-llm` | The external LLM provider hosting embeddings (out-of-deployment) | Per-embedding-batch HTTPS call from host functions; the LLM-API-key configuration surface (`0.1.0`) and a hostname allowlist gate the egress |

The host trust boundary (`TB-mnemra-host`) is where policy enforcement happens. Plugins
execute inside `TB-plugin-sandbox` and cannot reach Postgres, the network, or the
filesystem directly; every IO call traverses a host function.

**Canonical TB enumeration.** The architecture overview ([overview](../architecture/overview.md))
is the canonical source for the trust-boundary set, because the data-flow diagram lives
there and the TB table sits adjacent to it. This Frame's table is the steady-state subset
used in Frame-altitude prose; the overview's nine-row TB table additionally carries the
two migration-and-backup-scoped boundaries (`TB-fs-source`, `TB-fs-backup`) plus the
steady-state set above. When the two tables disagree, the overview wins.

## Component map

The six responsibility buckets are (1) memory substrate, (2) inbound protocol surfaces,
(3) plugin runtime, (4) plugin-facing host functions, (5) outbound integrations, and
(6) cross-cutting. They map onto the brief's `0.1.0` substrate description. Each
bucket is a module surface inside the host process, not a microservice.

### 1. Memory substrate

The content and projection layer the host owns directly.

- **Storage substrate.** PostgreSQL ratified on merits, behind an engine-agnostic
  swappable `Storage` trait (one implementation), per [P-0010](../adrs/P-0010-storage-substrate-engine.md)
  D1 + D5. **V0 engine: embedded Postgres** (`postgresql_embedded` + bundled `pgvector`
  for V0.1+ vector retrieval), shipping with the single binary. TimescaleDB is demoted off
  the V0 stack (D8); the V0 timeseries shape is plain timestamped Postgres tables, with
  TimescaleDB held behind a latency/storage trip-wire. The **storage-shape model** is the
  alignment-doc round-4 four-shape model: content / timeseries / log / state-config. It was
  re-derived 2026-06-09 (E1 dispositioned = re-derive now): only content and state-config are
  persisted in-app Postgres shapes; the former timeseries and log shapes are observability
  emission surfaces, not in-app storage at V0 (telemetry is emitted, not stored; see the
  [observability baseline](../architecture/overview.md#observability)).
- **Projections.** Materialized read-side derivatives over content. Refresh strategy and
  dependency tracking is a Frame-level open slot; see the open ADR list below.
- **Indexing pipelines.** Content arrives, projections update reactively. The
  detailed refresh-graph is a per-ADR concern.
- **Ingest.** V0 covers one-shot batch migration of the prior content corpus (brief
  `0.14.0`); ongoing-ingest (watchers, scheduled polls, webhooks) is brief V0.1
  scope (`1.2.0`).

### 2. Inbound protocol surfaces

- **MCP server.** Single MCP server with namespaced plugin verbs (each plugin extends
  the catalog under a namespace prefix; plugins are not separate MCP servers). Stdio
  transport at V0; streamable-HTTP transport is a V0.1+ activation.
- **Admin CLI.** Schema-driven dynamic subcommands generated from plugin manifests at
  startup. Per the brief, the admin CLI handles destructive and control-plane operations
  only; agent-facing CRUD is on MCP. **An MCP server is a V0 deliverable** (brief
  intent-clarity).

### 3. Plugin runtime

Wasmtime-hosted WebAssembly Component Model modules.

- **Lifecycle.** Plugin pool per plugin; instances stateless with respect to tenant (every call
  carries the workspace-scoping key); failure containment via host-managed restart.
- **Sandbox config.** Plugins receive only the host-fn surface their manifest declares;
  no ambient network or filesystem authority. **Wasmtime fuel and epoch-interruption are
  ON at V0**; ceiling values (fuel budget, epoch-interruption deadline, per-instance memory
  ceiling, table/instance limits) are slotted in `[P-0007-plugin-resource-limits](../adrs/P-0007-plugin-resource-limits.md)` (Tier A).
- **Manifest validation.** Plugin manifests declare `content_types` (host-owned tables
  the plugin writes into via host-fns), `state_scopes` (small KV state the plugin owns),
  and the host-fn surface the plugin needs. `core: true` plugins ship signed by the
  mnemra root authority and uninstall is structurally blocked. The manifest schema and
  the host-fn ABI surface are open ADR slots (see below). The capability-family increments
  `0.2.0` through `0.14.0` are partitioned between additional builtins and `core: true` plugins;
  `[P-0002-core-plugin-partition](../adrs/P-0002-core-plugin-partition.md)` (Tier A) determines the partition. The signed-and-non-
  uninstallable invariant binds whichever increments are partitioned as plugins.
- **Signing.** Plugin signature verification; the V0 custody decision lives at
  `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)` (Tier A), the minimum-viable build-host-on-disk for dogfood, with a
  multi-deployment trip-wire to `{{P-SigningKeyCustodyHardening}}` (Tier C).

### 4. Plugin-facing host functions

The host-fn ABI is the contract plugins write against. The following surface was
established during the architectural alignment walk and is enumerated here for
self-containment:

- LLM handoff (MCP sampling: the plugin asks; the agent's MCP client runs the
  completion).
- Content emit and query (universal artifact CRUD at V0; aspect-aware verbs are an open
  evolution path, see open ADR slot for storage layout).
- Projection registration / read.
- Secrets / keyring access (per-plugin permissions).
- HTTP fetch (declared per-plugin; subject to manifest-declared permission).
- Logging and metrics emission.

The ABI is pre-1.0 within `0.y.z`; per-host-fn stability marks (`@stable` / `@unstable`)
are the discipline mechanism.

### 5. Outbound integrations

- **External LLM provider.** mnemra-core calls out to an external model for embeddings
  via the embedding-batch pathway (the Extract-Load-Transform pipeline that routes
  artifact content through host functions to the external provider and writes resulting
  vectors into the projection substrate). The LLM-API-key configuration surface for this
  pathway is folded into `0.1.0` per the brief's T-5 resolution. The detailed pipeline
  shape (batching strategy, retry, provider-level hostname allowlist) is a Spec-stage
  concern; the Frame names only the pathway's existence and its trust-boundary crossing
  (`DF-embed-call` to `TB-external-llm` in the companion overview). The system MUST NOT
  host a language model (brief Hard constraints).
- **Federated MCP servers.** Mnemra-as-MCP-client to upstream MCP servers (re-exposing
  their tools under the mnemra namespace) is forward-context, not V0 scope.
- **Filesystem and HTTP.** Host-fn-mediated; plugins cannot reach these directly.

### 6. Cross-cutting

- **Security / permissions / auth.** Mnemra is **Resource Server only** (no
  authorization-server role at V0). Per-deployment OIDC AS via RFC 9728
  protected-resource-metadata; a static admin token bootstrap path for first-run and
  solo deployments (token shape at `[P-0008-admin-token-shape](../adrs/P-0008-admin-token-shape.md)`, Tier A). The workspace claim is
  structural in every token; the V0 enforcement mechanism for workspace isolation while
  RLS policy enforcement is deferred lives at `[P-0006-v0-tenant-enforcement](../adrs/P-0006-v0-tenant-enforcement.md)` (Tier A).
- **Observability / metrics.** Per-verb metrics, structured logs, a health endpoint, all
  **emitted** (stdout structured logs + OTel metrics/events + health-endpoint-first),
  storage-independently from the bare shell. Re-derived 2026-06-09 (E1 dispositioned) and
  **re-altituded out of the project-ADR layer**: observability is a theory trait plus chassis
  mechanism, not a per-project ADR. The generation decisions live in the
  [observability baseline](../architecture/overview.md#observability) in the companion
  overview, and the original observability ADR [P-0004](../adrs/P-0004-observability-shape.md)
  is `deprecated` (no successor ADR). The observability storage backend is deferred behind the
  generation⊥storage separation (option set plus named tripwire); there is no in-app
  observability store at V0. Observability ships from `0.1.0`, per the workspace-wide observability
  principle: ship instrumented before first user-touch.
- **Configuration.** Per-deployment configuration surface; runtime configuration via
  state-shape storage.
- **Agent sessions.** Per-MCP-connection technical primitive (MCP-protocol-defined),
  distinct from the brief's collaboration-session concept at `0.6.0`.
- **Saga coordination.** Not V0; designed-for as a host-service shape when a first real
  cross-plugin atomicity need lands. Plugins are leaves; the day-1 contract is "no
  cross-plugin atomicity, ever; design for partial failure."

### Builtin components inside the host

Per Framing correction 1, the following are **builtin** to the host process (not
plugins):

- **Workspace** (tenant boundary; solo collapses to `default`)
- **Users** and **Agents** (agents tied to user-workspace pairs)
- **Authentication** (workspace claim in every token; per-deployment OIDC; static
  dev-token first-run bootstrap)
- **Agent sessions** (per-MCP-connection)
- **Per-plugin permissions**
- **Projects**

These compose with the MCP server, the plugin runtime, and the host functions to provide
the substrate every capability-family increment (`0.2.0` onward) rides on. They are not
absorbed into the "plugins" surface; per the brief, plugins are scoped per project, so
projects-as-plugins is a circular bootstrap.

## Key cross-cutting decisions

The Frame inherits these decisions from the locked brief and from the alignment record's
already-locked agreements. Each is a constraint the Spec stage operates within.

### Authentication and authorization

- **Mnemra is Resource Server only.** No authorization-server role at V0.
- **OIDC per deployment.** Each self-hosted deployment configures its own OIDC AS;
  mnemra-core advertises the configured AS via RFC 9728 protected-resource-metadata.
- **Static admin token at V0.** Bootstrap path for first-run and solo deployments; held
  in `~/.config/mnemra/token` mode 600 (per the constraints draft's filesystem secrets
  posture). The token's structure (opaque-with-server-side-lookup versus claim-carrying
  signed) is the Frame-altitude decision at `[P-0008-admin-token-shape](../adrs/P-0008-admin-token-shape.md)` (Tier A); the
  role-and-permission shape downstream of that decision sits at `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)`
  (Tier A).
- **Workspace claim is structural.** Every token carries a `workspace` claim; the host
  scopes storage by it. How the workspace claim is *bound* to the token (host-side
  lookup against a server-side mapping table, or a claim-carrying cryptographic signature)
  is the `[P-0008-admin-token-shape](../adrs/P-0008-admin-token-shape.md)` decision.
- **External authorization server integration** (federated authorization) is brief V0.1+
  scope.

### Multi-tenancy

- **Tenancy invariant.** The `workspace_id` scoping key is **structural from V0**:
  NOT NULL on every artifact table, indexed, explicitly passed, forward-compatible
  without migration (brief Hard constraints).
- **RLS column-shape at V0.** Postgres Row-Level Security column infrastructure ships at
  V0; **policy enforcement activation** is brief V0.1+ scope (the brief's open `idea`
  entry for row-level-security policy enforcement). The V0 enforcement mechanism is named
  at `[P-0006-v0-tenant-enforcement](../adrs/P-0006-v0-tenant-enforcement.md)` (Tier A). RLS at V0.1+ is the substrate-layer hardening
  of an enforcement that is already load-bearing at V0 via an application-layer mechanism
  (per `P-SecurityLayered`, each layer is independently load-bearing).
- **Tenant unit is workspace.** Solo dogfooding collapses to one `default` workspace.
- **Tenant hierarchy** (org and layers above the workspace=tenant boundary) is brief
  idea-tier deferred; safe to defer because the scoping key is structural.

### Plugin runtime

- **WASM Component Model + Wasmtime + WIT-defined host functions** (brief Hard
  constraints; Framing correction 2).
- **Plugins are leaves.** No sideways linkage between plugins; cross-plugin calls are
  host-mediated.
- **Plugin instance pool from V0.** A single mutex breaks under multi-tenant load; pool
  size 3 to 5 per plugin at V0, adaptive sizing is V0.1+ work.
- **`core: true` plugins signed by mnemra root and structurally non-uninstallable.**
  Distribution at V0 is build-time-embedded. V0 custody decision at `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)`
  (Tier A); hardening decision at `{{P-SigningKeyCustodyHardening}}` (Tier C). The set of
  `0.2.0` through `0.14.0` capability families that are `core: true` plugins (versus additional
  builtins) is named at `[P-0002-core-plugin-partition](../adrs/P-0002-core-plugin-partition.md)` (Tier A).

### Storage shape

> **Reframed 2026-06-08 (substrate re-opened on merits, [P-0010-storage-substrate-engine](../adrs/P-0010-storage-substrate-engine.md)).** The original Frame treated the storage substrate as a hard-locked brief carry-forward with no substrate ADR slot. A storage-engine evaluation (ratified 2026-06-07) re-opened it on merits: **PostgreSQL is ratified on merits** (the only license-clean, capable, and mature contender after a license gate), **behind an engine-agnostic, swappable `Storage` trait** with Postgres as the only implementation. This reverses the prior "Postgres-natural / no-swap" treatment. The substrate is now a decided artifact (P-0010), not an unexamined constraint. The points below are updated to that decision; the four-shape model and the layout fork are unchanged in substance.

- **PostgreSQL ratified on merits as the storage substrate**, behind an engine-agnostic
  swappable `Storage` trait (one implementation), per [P-0010](../adrs/P-0010-storage-substrate-engine.md)
  D1 + D5. The **V0 engine is embedded Postgres** (`postgresql_embedded` + bundled
  `pgvector`), shipping with the single self-hosted binary, not an operator-provisioned
  external Postgres server.
- **V0 stack is A1-clean** ([P-0010](../adrs/P-0010-storage-substrate-engine.md) D2):
  pgvector HNSW + native FTS + recursive CTEs + JSONB. Extensions beyond pgvector are
  adopted only on named trip-wires (keyword/BM25-fidelity → D3; graph/AGE → D4;
  time-series/TimescaleDB → D8).
- **Storage shapes at V0: content + state-config (persisted); observability is emitted, not
  stored in-app.** The content shape is `pgvector`-ready tables and state-config is state KV
  tables, the two persisted Postgres shapes. **The former timeseries and log shapes are
  observability emission surfaces** (the [observability baseline](../architecture/overview.md#observability),
  re-derived 2026-06-09; E1 dispositioned = re-derive now): the server emits structured logs
  to stdout and OTel metrics/events; *where* telemetry lands is the operator's choice behind
  the generation⊥storage separation, deferred and not in the binary at V0. The content-substrate's
  own time-series tables (if any beyond observability) are plain timestamped Postgres tables
  ([P-0010](../adrs/P-0010-storage-substrate-engine.md) D8 demotes TimescaleDB off the V0 stack
  to a latency/storage trip-wire).
- **Content-first with CQRS-real projections.** Mutations write content;
  projections rebuild reactively from content; reads hit projections.
- **The detailed layout of a single logical artifact across the four shapes** (whether a
  task lives in one row, in multiple tables in the content substrate, or fans across
  content + state + log substrates) is the central architectural fork at the Spec stage.
  This is resolved in `[P-0001-storage-layout](../adrs/P-0001-storage-layout.md)` (Tier A)
  as the Postgres *implementation* under P-0010's substrate decision; see the Tier A table
  below for context.

### MCP transport

- **Stdio at V0.** Streamable-HTTP is a V0.1+ activation, conditional on the
  microVM-appliance trip-wire (a named-capability condition, not release-gated).
- **MCP specification version 2025-06-18** (brief Hard constraints).

### Observability

- **Ship instrumented from V0.** Metrics, structured logs, and traces in place at first
  user-touch; per-verb metrics on every MCP call, all **emitted** (stdout + OTel) storage-independently
  from the bare shell (a workspace-wide observability discipline). The observability storage
  backend is deferred behind the generation⊥storage separation, not in the binary at V0
  (the [observability baseline](../architecture/overview.md#observability), re-derived 2026-06-09;
  re-altituded out of the project-ADR layer; P-0004 `deprecated`, no successor ADR).
- **Health endpoint shape.** Structured-detail body identifying which dependency failed
  (Postgres reachable / `pgvector` loaded / workspace=default exists). The listener binds
  loopback-only at V0, so loopback IS the gate on the detail body (named tripwire: if the
  listener ever binds non-loopback, admin-token gating becomes required).

### Migration discipline

- **Migration scope at V0.** The prior structured task store's table set and a fixed
  subset of the prior content corpus subdirectories (brief locks the scope; workspace-
  level operational tooling such as skills, team profiles, and inbox surfaces is V0.1+ scope
  per the brief's idea-tier entry for internal-workspace absorption).
- **R6.4 / R6.4a / R6.4b idempotency and resumability.** Migration must support
  resume-from-mid-failure with a per-record progress manifest.
- **R2.7 round-trip equivalence.** Source-frontmatter round-trip MUST be byte-equal
  modulo system-generated migration metadata fields.
- **R6.3 system-field separation.** System-generated fields (e.g., `migrated_from`,
  `migrated_at`, `frontmatter_version`) MUST NOT overwrite source frontmatter.

### Testing and review discipline

- **Test-Driven Development pairs on non-trivial work** (workspace-wide principle:
  TDD pairs). A separate red-phase test task precedes the implementation task on
  security boundaries, public APIs, parsers, validators.
- **Worktrees mandatory.** All code work in worktrees; main is protected.
- **Heterogeneous reviewers.** A code-and-security reviewer on every change; specialists
  join when their surface is touched.

## Open ADR slots

The Spec stage will need locked decisions on the following. Each is named with a
candidate identifier (`{{P-XXX}}` placeholder shape; final IDs are assigned at Spec
stage). The triage tiering follows the constraints strawman's Tier A / B / C ordering,
adapted for the brief's INCR-1 staged-increment shape. Tier A unblocks the `0.1.0`
substrate; Tier B unblocks the migration increments; Tier C unblocks operational
hardening.

**Placeholder-to-ADR resolution mechanism.** At Spec stage, the implementing developer
authors the actual ADR files using MADR format with the `P-` prefix, stored at
`docs/src/adrs/P-NNNN-<slug>.md` (e.g., `{{P-StorageLayout}}` resolves to
`P-0001-storage-layout.md`). Each `{{P-XXX}}` placeholder in this Frame and in the
companion overview is back-updated to cite the resolved ADR file at that point. The
resolution mapping is recorded in a placeholder-resolution table authored at Spec stage
as part of the first ADR dispatch, so the Frame and overview do not carry orphaned
placeholders beyond the Spec authoring window.

### Tier A — unblock `0.1.0` substrate

| Candidate ID | Decision | Notes |
|---|---|---|
| `[P-0010-storage-substrate-engine](../adrs/P-0010-storage-substrate-engine.md)` | The storage **substrate and engine**: which engine (Postgres-on-merits vs unified vs polyglot), the V0 engine shape (embedded vs external), the V0 extension stack, and whether storage sits behind an engine-agnostic swap trait or a Postgres-shaped one | **Fold-added slot (2026-06-08).** The original Frame had *no* substrate slot; the substrate was treated as a hard-locked brief carry-forward. A storage-engine evaluation (ratified 2026-06-07, after the substrate spec lock) re-opened it on merits. **Resolved:** PostgreSQL ratified on merits behind an engine-agnostic swappable `Storage` trait (D5, one implementation); V0 embedded Postgres; A1-clean V0 stack (D2); keyword/graph/time-series on named trip-wires (D3/D4/D8); D6 method-borrows deferred to the retrieval ADR; D7 managed-tier skipped. P-0001 (layout) sits under this decision. Escalation E1 (D8 vs the observability hypertables) dispositioned 2026-06-09 (re-derive now) → re-altituded out of the ADR layer to the [observability baseline](../architecture/overview.md#observability); P-0004 `deprecated`, no successor ADR. |
| `[P-0001-storage-layout](../adrs/P-0001-storage-layout.md)` | The detailed layout of a single logical artifact across the four storage shapes (single-document vs composite-typed-slots vs multi-substrate-with-joins) | Central architectural fork. **Resolved 2026-05-24: C1 single-document layout.** Whole artifact in one row; JSONB frontmatter + body + system fields; non-breaking C2 evolution path designed into the projection layer. Sits under `[P-0010-storage-substrate-engine](../adrs/P-0010-storage-substrate-engine.md)` (the Postgres implementation under the swap trait). |
| `[P-0002-core-plugin-partition](../adrs/P-0002-core-plugin-partition.md)` | Cohesion criterion for the V0 core plugin set; partitions the capability-family increments `0.2.0` through `0.14.0` between additional builtins and `core: true` plugins | Depends-on `[P-0001-storage-layout](../adrs/P-0001-storage-layout.md)`. **Resolved 2026-05-24:** 4 `core: true` plugins (tasks / repos / jobs / contacts) + 7 builtins. Verb-on-content discriminator. |
| `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` | Plugin manifest schema + host-fn ABI surface | Depends-on `[P-0001-storage-layout](../adrs/P-0001-storage-layout.md)` and `[P-0002-core-plugin-partition](../adrs/P-0002-core-plugin-partition.md)`. **Resolved 2026-05-24:** Universal `content.emit` verb shape; `schema_version: 1`; typed host-fn allowlist compiled per-instance from signed manifest. |
| `{{P-ObservabilityShape}}` → **re-altituded out of the ADR layer** to the [observability baseline](../architecture/overview.md#observability) (a theory-trait baseline in the companion overview, **not** an ADR); [P-0004](../adrs/P-0004-observability-shape.md) `deprecated`, **no successor ADR** | Observability shape re-derived around generation⊥storage separation: the server EMITS telemetry (stdout structured logs + OTel metrics/events + health-endpoint-first) storage-independently; the storage backend is deferred behind the separation (option set + named tripwire), not locked. The maintainer ruled observability a theory trait plus chassis mechanism, **not a per-project ADR**, so this Frame ADR-slot resolves to a non-ADR baseline (the altitude re-disposition is marked explicitly). P-0004 (TimescaleDB hypertables + retention + continuous aggregate in the app's own Postgres) is the `deprecated` historical record; its storage core was falsified by P-0010 D8. | Originally drafted as P-0004 in parallel with the Tier A core; re-derived 2026-06-09 (E1 disposition) and re-altituded out of the ADR layer to the overview observability baseline. |
| `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)` | Minimum-viable custody decision that lets V0 ship signed `core: true` plugins. **V0 mechanism:** the build host has the key on disk for dogfood. **Trip-wire:** the moment mnemra-core is deployed beyond the maintainer's single dogfood instance, the trip-wire fires and `{{P-SigningKeyCustodyHardening}}` (Tier C) is authored. | Anchored in [P-Defer](../glossary.md#p-defer) (Open/Deferred mechanism named with a stated trip-wire). Split out of the originally-Tier-C `{{P-SigningKeyCustody}}` because the `0.1.0` build pipeline cannot ship signed artifacts under a deferred custody decision; V0 needs a custody story even if the hardened story comes later. |
| `[P-0006-v0-tenant-enforcement](../adrs/P-0006-v0-tenant-enforcement.md)` | V0 application-layer enforcement mechanism for workspace isolation while RLS policy enforcement is deferred to V0.1+. Conservative pick: **typed `WorkspaceCtx` parameter binding at the host-fn boundary**, where every host-fn signature requires a `WorkspaceCtx` argument that the host populates from the validated request token; queries that don't take it cannot be authored. Open variants: (a) typed parameter binding (lead), (b) storage-layer query rewriter, (c) per-host-fn explicit `workspace_id` parameter validation. | Anchored in `P-SecurityLayered` ("each layer is independently load-bearing; losing any layer weakens the whole"). The V0 enforcement layer is named here so RLS at V0.1+ is the substrate-layer hardening of an enforcement that is already load-bearing at V0 via an application-layer mechanism. |
| `[P-0007-plugin-resource-limits](../adrs/P-0007-plugin-resource-limits.md)` | Plugin sandbox resource limits per Wasmtime instance: **fuel (CPU-time ceiling) + epoch-interruption (deadline-based preemption) + memory ceiling + table/instance limits**. V0 turns on fuel and epoch; values specified in this ADR. | Anchored in `P-StackDiscipline` (Wasmtime's resource knobs are stack-aligned and already present in the library; this is a config decision, not a build decision). Promoted from Tier C `{{P-PluginPoolMemory}}` (renamed) because the sandbox security posture commitments in this Frame depend on these limits being live at V0. |
| `[P-0008-admin-token-shape](../adrs/P-0008-admin-token-shape.md)` | Static admin token structure: opaque-with-server-side-lookup vs claim-carrying signed (JWT/PASETO/etc.). The choice gates security architecture (loss of the token-file means different things in each shape) and the binding mechanism for the workspace claim every token carries. | Split out of the originally-Tier-A `{{P-RLSAdminToken}}`. Upstream of role/permission shape; the role model lives downstream at `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` and presupposes this decision. |
| `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` | RLS role model + admin token permission shape | Depends-on `[P-0001-storage-layout](../adrs/P-0001-storage-layout.md)` (determines policy-surface count) and `[P-0008-admin-token-shape](../adrs/P-0008-admin-token-shape.md)` (determines whether destructive ops bind on a claim-carrying or opaque-lookup token). **Resolved 2026-05-24:** binary admin/read-observer role enum; permission matrix per role; V0 app-layer enforcement; V0.1+ RLS policy hardening path. |

### Tier B — unblock migration increments (`0.2.0`–`0.14.0`)

| Candidate ID | Decision |
|---|---|
| `{{P-MigrationID}}` | Migration ID derivation strategy for the prior task store + content corpus → mnemra-core ULID identity |
| `{{P-FKPreservation}}` | Foreign-key preservation across the migration and inside the content substrate; legacy task-store disposition policy |
| `{{P-BackupRestore}}` | Backup and restore atomicity + role separation; round-trip verify before destructive operations |
| `{{P-CutoverDualWrite}}` | Cutover dual-write gate (during the `1.0.0` cutover window, what guarantees prevent the prior tooling and mnemra-core from diverging) |

### Tier C — operational + concurrency hardening

| Candidate ID | Decision |
|---|---|
| `{{P-PostgresExtDeploy}}` | Postgres + extensions deployment shape (which extensions ship in the appliance, how they upgrade) |
| `{{P-MCPWriteSemantics}}` | MCP write semantics, including concurrent-write conflict resolution, deletion semantics, idempotency keys |
| `{{P-SigningKeyCustodyHardening}}` | Production-grade signing key custody (HSM-backed / runtime-fetch / never-on-deployment-node). Activated by the multi-deployment trip-wire on `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)`. |

### Cross-tier

| Candidate ID | Decision | Notes |
|---|---|---|
| `{{P-ProjectionRebuild}}` | Projection rebuild semantics, including per-substrate source-of-truth declarations, cross-substrate rebuild ordering, refresh-queue dependency tracking | Surfaces under any non-trivial storage layout. The 2026-05-03 strawman called this out as a likely standalone ADR. |

### Forward-context (V0.1+; not Frame-time)

The brief's V0.1 (post-`1.0.0`) immediate roadmap names two committed-to-the-phase
increments: `1.1.0` (`get_context_for(artifact_id)` retrieval verb) and `1.2.0`
(ongoing-ingest pipeline). Neither is in V0 Frame scope. Per the brief, V0.1 entries
lock their own frame plus spec when they're build-actioned.

## Reconciliation to the locked brief

The Frame's reconciliation discipline is direct citation: every claim above is either
(a) directly stated in the locked brief, (b) directly stated in a locked predecessor
decision (the V0 discovery, the architecture alignment record, the project defaults),
or (c) an inference whose chain the Frame body makes explicit. No tensions surfaced that
contradict the brief. The Frame-level refinements (Frame is narrower than the brief, but
consistent) are:

| Refinement | Where the brief permits it |
|---|---|
| The four-shape storage model (content / timeseries / log / state-config), re-derived 2026-06-09 to two persisted shapes (content / state-config) plus observability emission surfaces (former timeseries / log) | Brief Hard constraints fix the substrate (`pgvector`) but do not enumerate the four shapes; the alignment record's round-4 mental model does, and the brief's `0.1.0` substrate description ("content/timeseries/log/state storage-shape partitions") cites it. The substrate is re-opened on merits by [P-0010-storage-substrate-engine](../adrs/P-0010-storage-substrate-engine.md) (TimescaleDB demoted off the V0 stack, D8); the former timeseries/log shapes are observability emission surfaces, not in-app storage at V0 (telemetry is emitted, not stored; see the [observability baseline](../architecture/overview.md#observability)). |
| Plugin instance pool size 3 to 5 at V0 | Brief Hard constraints require multi-tenancy structure; the alignment record's round-5 pool decision operationalizes it. |
| RLS column-shape at V0, policy enforcement at V0.1+, **application-layer enforcement load-bearing at V0** | Brief Tenancy invariant locks "`workspace_id` structural from V0"; the brief's idea-tier entry for row-level-security policy enforcement defers activation. The V0 enforcement mechanism for workspace isolation is named at `[P-0006-v0-tenant-enforcement](../adrs/P-0006-v0-tenant-enforcement.md)` (Tier A). Per `P-SecurityLayered`, the application layer is independently load-bearing at V0; RLS at V0.1+ is the substrate-layer hardening. |
| `core: true` plugins ship signed at V0 under a Tier-A V0 custody decision | Brief Hard constraints commit to signed `core: true` plugins; this Frame names `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)` (Tier A) as the V0 custody decision (build-host-on-disk for dogfood) with the multi-deployment trip-wire to `{{P-SigningKeyCustodyHardening}}` (Tier C). Per [P-Defer](../glossary.md#p-defer), the Open/Deferred mechanism names the V0 mechanism AND its trip-wire. |
| Wasmtime fuel + epoch-interruption ON at V0 | Brief and predecessor specs commit to the plugin-sandbox security outcomes ("an infinite-loop plugin is killed and replaced from the pool"); this Frame names the *mechanism* (fuel, epoch-interruption, memory ceiling, table/instance limits) and slots ceiling values at `[P-0007-plugin-resource-limits](../adrs/P-0007-plugin-resource-limits.md)` (Tier A, renamed/promoted from the prior Tier-C `{{P-PluginPoolMemory}}`). Per `P-StackDiscipline`, Wasmtime's resource knobs are stack-aligned. |
| Static admin token shape is a Frame-altitude decision | Brief Hard constraints commit to the static admin token with a structural workspace claim; the *token structure* (opaque-with-server-side-lookup versus claim-carrying signed) and the workspace-claim *binding mechanism* gate downstream security architecture, so the choice lives at Frame altitude as `[P-0008-admin-token-shape](../adrs/P-0008-admin-token-shape.md)` (Tier A). The role-and-permission shape downstream sits at `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (Tier A). |
| `core: true` plugins partition of `0.2.0`–`0.14.0` is named | Brief enumerates the capability-family increments but does not partition them between additional builtins and `core: true` plugins. `[P-0002-core-plugin-partition](../adrs/P-0002-core-plugin-partition.md)` (Tier A) determines the partition; the signed-and-non-uninstallable invariant binds whichever increments are partitioned as plugins. |

## Out of Frame scope

The following are explicitly **not** Frame-stage decisions and are recorded here so the
boundary is unambiguous:

- **Detailed ADR authoring.** Spec stage owns this. Frame names candidate ADRs and
  surfaces the relationships between them, but does not lock decisions.
- **Threat-modeling output** (STRIDE-per-element, risk-register entries, accepted-risks
  list). The architecture overview port contains a threat-scaffold placeholder; the
  terminal-review threat-modeling pass populates it at Stage 2 fire (security-mode
  review with the threat-modeling skill loaded).
- **Test-suite design.** Test discipline is named (TDD pairs, heterogeneous reviewers);
  test-fixture authoring is Spec-stage and Stage-3 implementation work.
- **Plan / sequencing within the staged-increment sequence.** The brief locks ordering
  rationale per-entry; per-increment plans are ephemeral, generated when each increment
  is build-actioned.
- **Commercial gating.** The brief explicitly carves commercial validation thresholds,
  pricing, and go-to-market to a separate internal record; Frame does not absorb them.

## Pointers

- For the **constraint inventory**, the **6-axis quality-attribute utility tree**, the
  **data-flow diagram**, and the **threat-scaffold placeholder**, see the companion
  artifact: [Architecture overview](../architecture/overview.md).
- For the workspace-wide project defaults (G-* ADR baseline projected at project
  creation), see [Project Defaults](../adrs/DEFAULTS.md).
- For glossary entries (ADR, MADR format, the two-tier G-/P- ADR system), see
  [Glossary](../glossary.md).

## Changelog

- **2026-06-09** — Observability re-derivation (E1 dispositioned = re-derive now), **re-altituded
  out of the project-ADR layer**. The maintainer dispositioned escalation E1 by separating
  observability **generation** from **storage**: the server EMITS telemetry (stdout structured
  logs + OTel metrics/events + health-endpoint-first) storage-independently from the bare shell;
  the observability storage backend is deferred behind the separation (option set {Prometheus,
  InfluxDB, TimescaleDB, plain Postgres tables}, named tripwire), not locked; the standalone
  binary survives (observability storage is external operator infra). The maintainer further
  ruled observability a **theory trait + chassis mechanism, not a per-project ADR**: the
  generation decisions land in the [observability baseline](../architecture/overview.md#observability)
  in the companion overview (a theory-trait baseline, not an ADR), and the original observability
  ADR `[P-0004-observability-shape](../adrs/P-0004-observability-shape.md)` is `deprecated`
  (its storage core falsified by P-0010 D8; **no successor ADR**). The intermediate
  observability ADR (P-0011) drafted earlier this day is **dissolved**; it
  over-built the altitude (re-asserting at project-ADR altitude a thing that is a theory baseline
  plus chassis wiring). Reframed the Frame's two Observability sections, the storage-shape model
  (the former timeseries/log shapes are emission surfaces, not in-app storage), and the Tier-A
  slot table (the `{{P-ObservabilityShape}}` slot is marked re-altituded out of the ADR layer to
  the overview baseline; P-0010's E1 tail re-targeted). The `/health` detail body is gated by the
  loopback-only listener bind (loopback IS the gate at V0; named tripwire if it ever binds
  non-loopback). The host capability-manifest is a settled invariant (generated from WIT, never
  hand-maintained; generation mechanism routes to the chassis). Non-observability Frame content
  unchanged.
- **2026-06-08** — Storage-substrate fold. A storage-engine evaluation (ratified
  2026-06-07, *after* the 2026-05-24 substrate spec lock) re-opened the storage substrate
  on merits and was folded into the WS-E-2 designed tier before merge. The Frame's
  storage treatment is reframed from "hard-locked brief constraint, no substrate slot" to
  "PostgreSQL ratified on merits, behind an engine-agnostic swappable `Storage` trait"
  (new `[P-0010-storage-substrate-engine](../adrs/P-0010-storage-substrate-engine.md)`,
  added to the Tier-A slot table). Reframed: the Storage-shape cross-cutting block, the
  Memory-substrate component bullet, and the four-shape model's timeseries shape (plain
  timestamped Postgres tables at V0; TimescaleDB demoted off the V0 stack to a trip-wire,
  D8). The V0 engine is embedded Postgres (not an external server). Carried escalation
  **E1** (D8 vs the then-accepted `[P-0004-observability-shape](../adrs/P-0004-observability-shape.md)`
  hypertables, a maintainer scope/sequencing call, pending at fold time), **dispositioned
  2026-06-09 (re-derive now); see the 2026-06-09 entry above.** Non-storage Frame content is
  unchanged.
- **2026-05-23** — Frame revision per Frame-exit gate Revise verdict. Frame-exit gate
  ran retroactively on the 2026-05-22 synthesized Frame after the 2026-05-23 G-0028
  cold-start amendment landed (Stage 2 split into 2a elicitation + 2b synthesis +
  Frame-exit gate). Warden's Stage 2 code-and-security review (target_commit
  `0fafbf39c2a5412bc99de0ecf499cebc7524ec63`, dispatch_id 652, dated 2026-05-22) returned
  Approve-with-conditions; the gate read that as Revise. Four architectural directions
  were locked via Stage 2a-shaped walkthrough (recorded at the new Stage 2a section
  above): (H1) split `{{P-SigningKeyCustody}}` → Tier-A `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)` + Tier-C
  `{{P-SigningKeyCustodyHardening}}`; (H2) add Tier-A `[P-0006-v0-tenant-enforcement](../adrs/P-0006-v0-tenant-enforcement.md)`
  application-layer enforcement backstop; (M3) rename `{{P-PluginPoolMemory}}` →
  `[P-0007-plugin-resource-limits](../adrs/P-0007-plugin-resource-limits.md)` and promote to Tier A; (M4) split `{{P-RLSAdminToken}}`
  → Tier-A `[P-0008-admin-token-shape](../adrs/P-0008-admin-token-shape.md)` + Tier-A `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (narrowed). Three
  Mediums absorbed mechanically: (M1) `TB-external-llm` row added to Frame TB table,
  overview designated canonical TB enumeration; (M2) overview DFD extended with
  `P-builtin-users`, `P-builtin-sessions`, `P-builtin-permissions`; (M5) inline forward-
  reference to `[P-0002-core-plugin-partition](../adrs/P-0002-core-plugin-partition.md)` for the `0.2.0`–`0.14.0` partition. L1, L2,
  N1 deferred (housekeeping). Net ADR-slot change: +3 Tier A slots
  (`[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)`, `[P-0006-v0-tenant-enforcement](../adrs/P-0006-v0-tenant-enforcement.md)`, `[P-0008-admin-token-shape](../adrs/P-0008-admin-token-shape.md)`),
  +1 promoted Tier A slot (`[P-0007-plugin-resource-limits](../adrs/P-0007-plugin-resource-limits.md)`, renamed from
  `{{P-PluginPoolMemory}}`), Tier C gains `{{P-SigningKeyCustodyHardening}}`, Tier C
  loses `{{P-PluginPoolMemory}}` and `{{P-SigningKeyCustody}}`. Final tally: Tier A 9
  (was 5), Tier B 4 (unchanged), Tier C 3 (was 4), Cross-tier 1 (unchanged).
- **2026-05-23** (Stage 3 entry housekeeping) — Resolved three deferred housekeeping
  items from the Stage 2 code-and-security review (d652/d655): (L1) ELT subsystem forward
  reference resolved, removed undefined "ELT subsystem" term; replaced with an inline
  description of the embedding-batch pathway (Extract-Load-Transform pipeline routing
  artifact content through host functions to external provider; Spec-stage detail deferred
  with explicit callout to `DF-embed-call`/`TB-external-llm`); (L2) workspace-internal
  citation resolved, "Round-6 of the alignment record" citations removed from component
  map intro and host-fn surface intro; six bucket names inlined directly; host-fn surface
  enumeration already present inline so only the citation header changed; (N1) placeholder
  resolution mechanism added, an explicit paragraph inserted at the open-ADR-slots section
  (before Tier A table) specifying that the implementing developer at Spec stage authors
  `P-NNNN-<slug>.md` ADR files in MADR format; each placeholder resolves to a numbered
  filename; resolution recorded in a placeholder-resolution table at Spec stage. No
  architectural content changes; Frame remains locked at acc511f baseline. Companion
  overview updated in the same commit for slot citation routing and R-0004 rewrite.
- **2026-05-22** — Frame doc initial draft. Stage 2 of `/brief` for mnemra-core; first
  real Stage-2 → Stage-3 dogfood of the new agent-first workflow shape. Reconciles to
  the locked brief (intake-exit 2026-05-20). Applies three framing corrections from
  predecessor artifacts: (1) `projects` and `agents` are builtin not plugins; (2) plugin
  shape is WASM Component Model loaded in-process, not stdio MCP wrappers around
  external CLIs; (3) V0 is a staged increment sequence (`0.1.0` → `1.0.0`), not a
  monolithic release. Companion artifact: `architecture/overview.md`. Open ADR slots
  named with `{{P-XXX}}` placeholders for Spec-stage authorship.
