---
title: "Frame: Mnemra Core"
summary: "Architectural frame for mnemra-core — system boundary, component map, cross-cutting decisions, and open ADR slots — reconciled to the locked product brief."
primary-audience: agent
---

# Frame — Mnemra Core

**Date:** 2026-05-22 · **Status:** draft (Stage 2 of `/brief`) · **Altitude:** component

> Format note: this is the **Stage 2 (Frame)** output of the `/brief` pipeline. It sits between
> the locked product brief (Stage 1, Intent) and the per-feature specs (Stage 3). The Frame's
> job is to translate the product intent's purpose + hard constraints into an architectural
> shape: system boundary, component map, key cross-cutting decisions, and the open
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
| V0 architecture constraints (high-stakes; constraint inventory, QA tree, DFD, threat scaffold) | "V0 constraints" decision record, draft 2026-05-03 — partially superseded by INCR-1 resolution in the brief (see Framing corrections below) |
| Architecture alignment mental model (round-6) | "Architecture alignment R6" decision record, agreed 2026-04-27 |
| Substrate shape exploration (pre-discovery, background only) | "Substrate shape exploration" exploration note, 2026-05-02 |
| Architecture overview port | [Architecture overview](../architecture/overview.md) — companion artifact, ports constraint inventory + QA utility tree + DFD + threat scaffold |
| Project defaults (G-* baseline) | [Project Defaults](../adrs/DEFAULTS.md) |

This Frame reconciles every architectural claim against the locked brief's Hard constraints
and the staged-increment sequence (`0.1.0` host core → `1.0.0` dogfood cutover) it locked
at intake-exit (2026-05-20).

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

The plugin model (also locked in Hard constraints) is: **WebAssembly Component Model
modules hosted in-process via Wasmtime, with IO-free plugin core logic; all plugin IO MUST
be mediated by host-provided functions; plugins are leaves with no sideways linkage;
cross-plugin calls are host-mediated.** That model does not accommodate identity-bearing
components — by construction they are above the plugin boundary.

**Why the change.** Per-project plugin scoping makes "projects" a plugin a chicken-and-egg
bootstrap problem. The brief's INCR-1 resolution (locked 2026-05-20) records the upstream
resolution: builtin substrate first. The Frame surfaces the consequence on the system
boundary — the DFD nodes for `projects` and `agents` move out of the plugin sandbox into
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
  crate — the CLI wires native IO, the plugin wires host-fn IO.

The "stdio-wrapper" shape does not port to mnemra; the Frame designs for IO-free core +
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
| `1.0.0` | Dogfood cutover (public API defined) — V0 milestone | Proposed tier |

The Frame's component map (below) reconciles to this sequence. Capability families that
land in distinct increments share the substrate `0.1.0` provides; each downstream
increment adds verbs to the MCP server skeleton without changing the host-fn ABI's shape
(the ABI is pre-1.0 within the `0.y.z` band per SemVer §4 — backward-compatible additions
are minor bumps, breaking changes accumulated for `1.0.0`).

## System boundary

### What is in mnemra-core

Mnemra-core is the **single-process server binary** that ships the substrate, the host,
the plugin runtime, and the builtin components. Per the brief's Hard constraints:

- The agent-facing surface is **MCP-native** (MCP specification 2025-06-18). Transport is
  **stdio at V0**; streamable-HTTP is a later-version activation.
- The substrate is a **single-process Postgres** instance with `pgvector` and
  `timescaledb` extensions present.
- Plugins are **WebAssembly Component Model modules** hosted in-process via Wasmtime.
- Deployment posture is **self-hosted-first, single-binary**. The system MUST NOT host a
  language model; it calls out to an external one.

The "single-binary" constraint binds the **server**, not the deployment packaging — an
immutable image or appliance is a valid packaging shape (per brief Hard constraints).
Native compile-time asset embedding is not used (project default `G-0002` — assets are
served at runtime, with packaging-time copy into the deployment image).

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
task store + the prior markdown content corpus) is the **migration source** for the
`0.2.0`–`0.14.0` increments. It is not a runtime adjacency of the running mnemra-core
server; it is one-shot import scope.

### Trust boundaries

The Frame inherits seven trust boundaries from the constraints draft (corrected per
Framing correction 1; the predecessor's `TB-fs-source` and `TB-fs-backup` are present in
the architecture overview's data-flow diagram but are migration-and-backup-scoped and not
part of the steady-state trust topology):

| Boundary | What it contains | What crosses it |
|---|---|---|
| `TB-agent-runtime` | MCP-client agents (separate processes hosting MCP clients) | The MCP transport (stdio at V0; streamable-HTTP V0.1+) |
| `TB-human` | The operator invoking the admin CLI | The admin CLI's local IPC to the running mnemra-core server, or a direct admin token over MCP |
| `TB-mnemra-host` | The mnemra-core server process itself | Crosses MCP transport, admin CLI IPC, Postgres connection, filesystem secrets reads |
| `TB-plugin-sandbox` | The Wasmtime sandbox running plugin code | All cross-boundary calls are host-fn-mediated; plugin core is IO-free |
| `TB-postgres` | The Postgres process | SQL connection from mnemra-core host; no agent or plugin code reaches Postgres directly |
| `TB-fs-secrets` | Filesystem-stored secrets (mode 600) | Admin token and (per the open ADR slot for signing-key custody) plugin-signing key material; read by host code only |
| `TB-build-pipeline` | Conceptual — the signing authority at build time | Signed plugin artifacts are what crosses into runtime; runtime sees signatures, not the key |

The host trust boundary (`TB-mnemra-host`) is the locus of policy enforcement. Plugins
execute inside `TB-plugin-sandbox` and cannot reach Postgres, the network, or the
filesystem directly; every IO call traverses a host function.

## Component map

The six responsibility buckets named in the architecture alignment record (round-6) map
naturally onto the brief's `0.1.0` substrate description. Each bucket is a module surface
inside the host process, not a microservice.

### 1. Memory substrate

The content and projection layer the host owns directly.

- **Storage substrate.** Single-process Postgres with `pgvector` (for V0.1+ vector
  retrieval) and `timescaledb` (for V0 timeseries / hypertable shapes) extensions
  installed. Brief Hard constraints lock this; the **storage-shape model** is the
  alignment-doc round-4 four-shape model: content / timeseries / log / state-config.
- **Projections.** Materialized read-side derivatives over content. Refresh strategy and
  dependency tracking is a Frame-level open slot; see the open ADR list below.
- **Indexing pipelines.** Content arrives, projections update reactively. The
  detailed refresh-graph is a per-ADR concern.
- **Ingest.** V0 covers one-shot batch migration of the prior content corpus (brief
  `0.14.0`); ongoing-ingest (watchers / scheduled polls / webhooks) is brief V0.1
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

- **Lifecycle.** Plugin pool per plugin; instances stateless w.r.t. tenant (every call
  carries the workspace-scoping key); failure containment via host-managed restart.
- **Sandbox config.** Plugins receive only the host-fn surface their manifest declares;
  no ambient network or filesystem authority.
- **Manifest validation.** Plugin manifests declare `content_types` (host-owned tables
  the plugin writes into via host-fns), `state_scopes` (small KV state the plugin owns),
  and the host-fn surface the plugin needs. `core: true` plugins ship signed by the
  mnemra root authority and uninstall is structurally blocked. The manifest schema and
  the host-fn ABI surface are open ADR slots (see below).
- **Signing.** Plugin signature verification; key custody is an open ADR slot.

### 4. Plugin-facing host functions

The host-fn ABI is the contract plugins write against. Round-6 of the alignment record
catalogs the surface:

- LLM handoff (MCP sampling — the plugin asks; the agent's MCP client runs the
  completion).
- Content emit and query (universal artifact CRUD at V0; aspect-aware verbs are an open
  evolution path — see open ADR slot for storage layout).
- Projection registration / read.
- Secrets / keyring access (per-plugin permissions).
- HTTP fetch (declared per-plugin; subject to manifest-declared permission).
- Logging and metrics emission.

The ABI is pre-1.0 within `0.y.z`; per-host-fn stability marks (`@stable` / `@unstable`)
are the discipline mechanism.

### 5. Outbound integrations

- **External LLM provider.** mnemra-core calls out to an external model for embeddings
  per the ELT subsystem; an LLM-API-key configuration surface is folded into `0.1.0` per
  the brief's T-5 resolution. The system MUST NOT host a language model (brief Hard
  constraints).
- **Federated MCP servers.** Mnemra-as-MCP-client to upstream MCP servers (re-exposing
  their tools under the mnemra namespace) is forward-context, not V0 scope.
- **Filesystem and HTTP.** Host-fn-mediated; plugins cannot reach these directly.

### 6. Cross-cutting

- **Security / permissions / auth.** Mnemra is **Resource Server only** (no
  authorization-server role at V0). Per-deployment OIDC AS via RFC 9728
  protected-resource-metadata; a static admin token bootstrap path for first-run and
  solo deployments. Workspace claim is structural in every token.
- **Observability / metrics.** Per-verb metrics, structured logs, a health endpoint;
  TimescaleDB hypertables with retention policies for metrics and events. Observability
  ships from `0.1.0` — workspace-wide observability principle: ship instrumented before
  first user-touch.
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
  posture).
- **Workspace claim is structural.** Every token carries a `workspace` claim; the host
  scopes storage by it.
- **External authorization server integration** (federated authorization) is brief V0.1+
  scope.

### Multi-tenancy

- **Tenancy invariant.** The `workspace_id` scoping key is **structural from V0** —
  NOT NULL on every artifact table, indexed, explicitly passed, forward-compatible
  without migration (brief Hard constraints).
- **RLS column-shape at V0.** Postgres Row-Level Security column infrastructure ships at
  V0; **policy enforcement activation** is brief V0.1+ scope (the brief's open `idea`
  entry for row-level-security policy enforcement).
- **Tenant unit is workspace.** Solo dogfooding collapses to one `default` workspace.
- **Tenant hierarchy** (org / + layers above the workspace=tenant boundary) is brief
  idea-tier deferred; safe to defer because the scoping key is structural.

### Plugin runtime

- **WASM Component Model + Wasmtime + WIT-defined host functions** (brief Hard
  constraints; Framing correction 2).
- **Plugins are leaves.** No sideways linkage between plugins; cross-plugin calls are
  host-mediated.
- **Plugin instance pool from V0.** Single-mutex breaks under multi-tenant load; pool
  size 3–5 per plugin at V0, adaptive sizing is V0.1+ work.
- **`core: true` plugins signed by mnemra root and structurally non-uninstallable.**
  Distribution at V0 is build-time-embedded.

### Storage shape

- **Single-process Postgres with `pgvector` + `timescaledb` extensions** (brief Hard
  constraints).
- **Four storage shapes** (content / timeseries / log / state-config), with the right
  Postgres-resident backend per shape (`pgvector`-enabled tables / TimescaleDB
  hypertables / log tables / state KV tables).
- **Content-first with CQRS-real projections.** Mutations write content;
  projections rebuild reactively from content; reads hit projections.
- **The detailed layout of a single logical artifact across the four shapes** (whether a
  task lives in one row, in multiple tables in the content substrate, or fans across
  content + state + log substrates) is the central architectural fork at the Spec stage.
  This is an open ADR slot; see the candidate `{{P-StorageLayout}}` below.

### MCP transport

- **Stdio at V0.** Streamable-HTTP is a V0.1+ activation, conditional on the
  microVM-appliance trip-wire (a named-capability condition, not release-gated).
- **MCP specification version 2025-06-18** (brief Hard constraints).

### Observability

- **Ship instrumented from V0.** Metrics, structured logs, and traces in place at first
  user-touch; per-verb metrics on every MCP call; TimescaleDB hypertables with retention
  policies (a workspace-wide observability discipline).
- **Health endpoint shape.** Structured-detail body identifying which dependency failed
  (Postgres reachable / extensions loaded / workspace=default exists).

### Migration discipline

- **Migration scope at V0.** The prior structured task store's table set and a fixed
  subset of the prior content corpus subdirectories (brief locks the scope; workspace-
  level operational tooling — skills, team profiles, inbox surfaces — is V0.1+ scope
  per the brief's idea-tier entry for internal-workspace absorption).
- **R6.4 / R6.4a / R6.4b idempotency and resumability.** Migration must support
  resume-from-mid-failure with per-record progress manifest.
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
adapted for the brief's INCR-1 staged-increment shape — Tier A unblocks the `0.1.0`
substrate; Tier B unblocks the migration increments; Tier C unblocks operational
hardening.

### Tier A — unblock `0.1.0` substrate

| Candidate ID | Decision | Notes |
|---|---|---|
| `{{P-StorageLayout}}` | The detailed layout of a single logical artifact across the four storage shapes (single-document vs composite-typed-slots vs multi-substrate-with-joins) | Central architectural fork. A Stage-1 strawman (2026-05-03) explored three candidates and recommended single-document for V0 with a non-breaking evolution path to composite-typed-slots; the recommendation is **not** locked here — the Spec stage owns the ADR. |
| `{{P-CorePluginPartition}}` | Cohesion criterion for the V0 core plugin set | Depends-on `{{P-StorageLayout}}`. The plugin partition follows once the artifact-layout shape is fixed; under different storage layouts, plugins partition differently. |
| `{{P-PluginManifest}}` | Plugin manifest schema + host-fn ABI surface | Depends-on `{{P-StorageLayout}}` and `{{P-CorePluginPartition}}`. ABI shape differs between single-document and composite-typed-slots layouts. |
| `{{P-ObservabilityShape}}` | Observability deployment shape — per-verb metrics surface, TimescaleDB retention policies, health-endpoint detail body, continuous-aggregate windows | Drafted in parallel with the Tier A core. |
| `{{P-RLSAdminToken}}` | RLS role model + admin token storage location + permission shape | Depends-on `{{P-StorageLayout}}`. Determines policy-surface count. |

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
| `{{P-PluginPoolMemory}}` | Plugin pool sizing + memory budget per plugin |
| `{{P-MCPWriteSemantics}}` | MCP write semantics — concurrent-write conflict resolution, deletion semantics, idempotency keys |
| `{{P-SigningKeyCustody}}` | Mnemra root signing key custody (deployment-node, HSM, runtime-fetch, never-on-node) |

### Cross-tier

| Candidate ID | Decision | Notes |
|---|---|---|
| `{{P-ProjectionRebuild}}` | Projection rebuild semantics — per-substrate source-of-truth declarations, cross-substrate rebuild ordering, refresh-queue dependency tracking | Surfaces under any non-trivial storage layout. The 2026-05-03 strawman called this out as a likely standalone ADR. |

### Forward-context (V0.1+; not Frame-time)

The brief's V0.1 (post-`1.0.0`) immediate roadmap names two committed-to-the-phase
increments — `1.1.0` (`get_context_for(artifact_id)` retrieval verb) and `1.2.0`
(ongoing-ingest pipeline). Neither is in V0 Frame scope. Per the brief, V0.1 entries
lock their own frame + spec when they're build-actioned.

## Reconciliation to the locked brief

The Frame's reconciliation discipline is direct citation: every claim above is either
(a) directly stated in the locked brief, (b) directly stated in a locked predecessor
decision (the V0 discovery, the architecture alignment record, the project defaults),
or (c) an inference whose chain the Frame body makes explicit. No tensions surfaced that
contradict the brief. Three are worth naming as Frame-level refinements (Frame is
narrower than brief, but consistent):

| Refinement | Where the brief permits it |
|---|---|
| The four-shape storage model (content / timeseries / log / state-config) | Brief Hard constraints fix the substrate (`pgvector` + `timescaledb`) but do not enumerate the four shapes; the alignment record's round-4 mental model does, and the brief's `0.1.0` substrate description ("content/timeseries/log/state storage-shape partitions") cites it. |
| Plugin instance pool size 3–5 at V0 | Brief Hard constraints require multi-tenancy structure; the alignment record's round-5 pool decision operationalizes it. |
| RLS column-shape at V0, policy enforcement at V0.1+ | Brief Tenancy invariant locks "`workspace_id` structural from V0"; the brief's idea-tier entry for row-level-security policy enforcement defers activation. |

## Out of Frame scope

The following are explicitly **not** Frame-stage decisions and are recorded here so the
boundary is unambiguous:

- **Detailed ADR authoring** — Spec stage owns this. Frame names candidate ADRs and
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

- **2026-05-22** — Frame doc initial draft. Stage 2 of `/brief` for mnemra-core; first
  real Stage-2 → Stage-3 dogfood of the new agent-first workflow shape. Reconciles to
  the locked brief (intake-exit 2026-05-20). Applies three framing corrections from
  predecessor artifacts: (1) `projects` and `agents` are builtin not plugins; (2) plugin
  shape is WASM Component Model loaded in-process, not stdio MCP wrappers around
  external CLIs; (3) V0 is a staged increment sequence (`0.1.0` → `1.0.0`), not a
  monolithic release. Companion artifact: `architecture/overview.md`. Open ADR slots
  named with `{{P-XXX}}` placeholders for Spec-stage authorship.
