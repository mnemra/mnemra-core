---
title: "Product Brief: Mnemra Core"
summary: "Product brief locking mnemra-core's intent and feature register."
primary-audience: agent
---

# Product Brief: Mnemra Core

**Date:** 2026-05-20 · **Status:** locked (intake-exit gate confirmed) · **Altitude:** product

> Format note: this is a living document. Its structure is a forward-contract with the
> structured-delta tooling that will own its evolution (add idea · promote tier · retire
> feature · adjust scope, applied as labeled `ADDED / MODIFIED / REMOVED` deltas). Don't
> restructure it ad hoc. Layer 1 (product-level intent) is stable and changes rarely. Layer 2
> (the feature register) grows continuously. A new thought defaults to a Layer-2 register entry
> at the `idea` tier, a captured direction with no design work behind it yet. It escalates to a
> Layer-1 revision only when it shifts the product's fundamental job-to-be-done or scope.

> Register-model note: the feature register uses a five-tier lifecycle
> (`idea → proposed → designed → committed → live`) whose tiers are validated by
> **pipeline artifacts**, not by prose judgement. Each tier's validator asks whether a specific
> artifact exists: `proposed` means a locked intake, `designed` means a locked frame plus a
> locked spec, `committed` adds a release-bound plan, and `live` means built and verified. This
> brief is the forcing instance for a pending amendment to the canonical register model (the
> prior model was four-tier and ordered `committed` before `designed`). The amendment is tracked
> separately, which is why the structure here leads its canonical adoption.

> Scope boundary: this brief is **mnemra-core's product intent and capability roadmap**.
> Sibling components in the mnemra umbrella (a dispatch CLI, a spec-delta/merge tool, a
> markdown review/annotation tool) live in their own repositories with their own forthcoming
> briefs and their own independent versions. They're referenced as external components, not
> absorbed into this brief's register.
>
> **Mnemra-as-a-whole does not carry a unified version.** Components version independently.
> A release-manifest concept may pin specific component versions for a coordinated public
> release; that's distinct from a version.
>
> Commercial validation thresholds, pricing, and go-to-market strategy are maintained as a
> separate internal commercial record and are deliberately **not** inlined here. Where a
> roadmap entry has a commercial dimension, only its product shape is recorded; its
> commercial gating is referenced, not described.
>
> Brief-home: this artifact lives in the mnemra-core repository at
> `docs/src/intent/mnemra-core.md` (relocated 2026-05-20). The brief travels with the component
> it scopes, following the per-repo-first default (keep an artifact with its component rather
> than in a shared home). The general multi-repo product-brief-home convention question stays in
> APPARATUS-1's scope, for future multi-repo briefs in other components.

## Product-level intent (layer 1, stable)

### JTBD

Engineering teams that run coding agents (Claude Code, Cursor, Copilot) need their agents, and
the humans working alongside them, to have **persistent, structured, queryable context** of the
team's codebase, decisions, tickets, docs, and prior agent sessions. That context has to be
available every session without re-explanation, so it stops being a per-session tax that drifts
and doesn't scale.

Stated as the need rather than the solution: an agent preparing to act on a task sits inside a
graph. Parent spec, related decisions, sibling tasks, prior reviews, recent adjacent commits.
Today that graph has to be hand-loaded every session by the orchestrating agent, which drifts
and doesn't scale. Mnemra's job is to make that context a durable, agent-addressable substrate.

### Non-goals

Each item is a concrete not-this:

- Not a retrieval-augmented-generation (RAG) chatbot, and not "a second brain for small
  businesses."
- Not a Notion replacement, knowledge wiki, or general-purpose vector-database wrapper.
- Not a horizontal "AI for every department" tool; not sales/marketing/HR enablement.
- Not an attempt to match an enterprise knowledge-search incumbent's breadth.
- Does not host a **generative** language model. All generative work (query rewrite,
  chunk-context, tag generation, synthesis) calls out to an external model at V0. Local
  **non-generative** inference (embedding, reranking, using small encoder models behind the
  host-fn seam) is permitted host-side. *(MODIFIED 2026-07-02 per RC-1, retrieval-cluster
  intake (locked 2026-07-02); was: "Does not run a language model. Embeddings and
  summaries call out to an external model; the system never hosts one.")*
- Not RAG-as-a-service.
- The open-source core does not pursue multi-tenant isolation as a product goal; tenancy is
  a structural column-shape at V0 with policy enforcement deferred. The boundary between
  OSS-core single-tenant and a future managed multi-tenant offering is commercial, and out of
  scope for this brief.
- **Not a general autonomous-agent framework.** The product *could* generalize toward one
  with work; single-use-case focus (a context layer for coding agents) is a deliberate
  quality choice. The generalization is declined, not absent. It's recorded here so the
  rejected option stays visible.

### Success criteria

Each is an observable outcome a downstream check could verify. **V0 and V0.1 are
marketing-tier labels** denoting product-promise milestones, distinct from Semantic Versioning
(SemVer) release identifiers:

| Marketing tier | SemVer corollary | What it delivers |
|---|---|---|
| V0 | `1.0.0` (dogfood-cutover / MVP) | the maintainer's workspace surface, replicated on mnemra-core without regression |
| V0.1 | `1.1.0`+ (first post-MVP minor sequence) | the core product promise activates: net-new value-add beyond workspace fidelity |

- **V0 (internal-dogfood gate; SemVer `1.0.0`):** the maintainer's own agent-orchestration
  workspace runs on mnemra-core with no functional regression versus its prior
  command-line-and-filesystem tooling, AND the agent-facing surface contract and storage
  substrate contract that later versions depend on are locked. Verifiable: a scripted
  representative-day fixture completes end-to-end against mnemra-core with zero fallback to
  the prior tooling.
- **Core product promise (activates V0.1; SemVer `1.1.0`):** a Model-Context-Protocol (MCP)
  client coding agent can retrieve persistent, typed, cross-session context for a given
  artifact in one call, rather than reconstructing it by hand each session.
- **Self-hostable:** a team can run the full core on its own infrastructure with data never
  leaving that boundary.
- Commercial validation thresholds exist but live in the separate internal commercial
  record; they aren't product success criteria and aren't inlined here.

### Hard constraints

Locked technical and integration boundaries (RFC 2119 keywords like SHALL, MUST, and MUST NOT,
used where the requirement is observable):

- The agent-facing surface SHALL be **MCP-native** (MCP specification 2025-06-18).
  Transport is stdio at V0; streamable-HTTP is a later-version activation.
- **An MCP server is a V0 deliverable.** To be clear about intent: the MCP-native constraint is
  satisfied by a running MCP server inside V0 scope, not by a future protocol posture alone.
- The substrate SHALL be a **single-process Postgres** instance with the `pgvector`
  extension present. TimescaleDB is demoted off the V0 stack (P-0010 decision D8, where P-0010
  is a project-scoped architecture decision record). It's absent by decision, not oversight. At
  V0 only content and state are persisted in in-app Postgres shapes; the former timeseries and
  log shapes are observability emission surfaces, not in-app storage (telemetry is emitted, not
  stored, per the architecture-overview observability baseline). So the time-series engine has
  no V0 store to back. TimescaleDB is held behind a named latency/storage trip-wire for a later
  version.
- Plugins SHALL be **WebAssembly Component Model modules** hosted in-process via Wasmtime;
  plugin core logic MUST be IO-free; all plugin IO MUST be mediated by host-provided
  functions. Plugins are leaves: no direct sideways linkage, and cross-plugin calls are
  host-mediated.
- Deployment posture SHALL be **self-hosted-first, single-binary**. The system MUST NOT
  host a **generative** LLM (large language model); all generative work calls out to an
  external model at V0. Local **non-generative** inference (embedding, reranking) is permitted
  host-side. *(MODIFIED 2026-07-02 per RC-1, retrieval-cluster intake (locked 2026-07-02); was:
  "The system MUST NOT host a language model; it calls out to an external one.")*
- **"Single-binary" constrains the server, not the deployment packaging.** It means one
  process, not a microservice mesh. An immutable image or appliance is a valid packaging
  shape for that single binary and doesn't violate this constraint.
- **Tenancy invariant:** the tenant scoping key (`workspace_id`) is structural from V0:
  NOT NULL, indexed, explicitly passed, and forward-compatible without migration. This is what
  makes deferring tenant-hierarchy/policy enforcement safe. The scoping key ships now; hierarchy
  and enforcement build on top later without a substrate migration.
- Tooling SHALL default to Rust; non-Rust paths are adopted only where no viable in-stack
  path exists (the landing site is an accepted exception).
- License: **Apache-2.0 with a future-relicense clause** (locked 2026-05-20). The
  mnemra-core repository's current LICENSE/README (MIT) is corrected in a separate follow-up
  task; this brief's Hard constraints lock the direction.
- Architecture MUST NOT be schedule-pressured. Dates appearing in marketing or landing
  material are not architectural inputs and don't weight tradeoff analysis.
- **Accessibility is a standing product requirement**, binding on every human-facing UI
  and documentation surface the product ships, current and future: the docs site, any
  dashboard or console, human-readable CLI output. Each such surface's design gate reviews
  it. Machine-facing MCP and agent surfaces are outside this requirement's scope; derivative
  human views inherit it. *(ADDED 2026-07-02, retrieval-cluster frame pre-gate walk item
  13 (locked 2026-07-02): routed here as a product-level standing requirement rather than
  faked into an MCP-verb feature cluster.)*

### Evidence

This brief exists because product intent that lived only in conversation wasn't an
agent-addressable source. Across multiple research-lifecycle reviews, research and
discovery work silently anchored scope to *mnemra-core-as-exists-live*, the only
available ground truth, because no durable product-intent or roadmap artifact existed. The
gap recurred at least three separate times before it was remediated. This document is that
remediation. It's the agent-addressable product-intent source that future research,
discovery, and architecture evaluate scope against, so "the intended product" is a readable
artifact rather than an inferred or imagined one.

Corroborating anchors: a locked V0 architecture discovery and a locked V0
architecture-constraints record (both high-stakes, reviewed to a zero-new-finding stopping
rule); a structural architecture overview (eight named subsystems); and an internal
commercial hypothesis, a set of testable claims maintained separately.

### Consumer

The primary consumer is **agents**: MCP-client coding agents and orchestration tooling that
load this as the agent-addressable product-intent source during research, discovery, and
architecture work. That's consistent with the project's agent-primary source-artifact stance.
The secondary consumer is the maintainer and future contributors evaluating scope.
Human-readable rendered views, when needed, are derivative and generated on demand. This
source is never the rendered view.

### Risk profile

This artifact is documentation. It touches no trust boundary itself. The *product* it
describes carries trust boundaries (multi-tenancy, authentication, plugin sandbox,
telemetry non-leak). Those are owned by the mnemra-core component architecture record
(the threat-modeling trigger is already met there) and are referenced by the register, not
re-assessed here. Required risk assessment for any *implementing* work is deferred to the
component-level frame, where the mechanism is known.

## Feature register (layer 2, grows; each entry has a lifecycle tier)

Each entry carries exactly one tier. Tiers are validated by **pipeline artifacts**, not by
prose judgement. The validator for each tier asks one yes/no question: does this artifact
exist? That keeps the register mechanically checkable and consistent with the intent → frame →
spec pipeline that produces it. (That pipeline has three stages: Intake locks structured
intent, Frame derives the operating constraints, and Spec produces the testable contract
verification consumes.)

| Tier | Mechanical validator | Durability |
|---|---|---|
| `idea` | a captured thought, optionally a provenance pointer to a locked decision | n/a |
| `proposed` | a **locked intake**: the feature has been through intent capture | permanent |
| `designed` | a **locked frame + locked spec**: the permanent "what to build" is complete | permanent (kept) |
| `committed` | `designed` **plus a plan** (the task list to action the build); release-bound | plan is ephemeral (not kept) |
| `live` | built and verified in current code/canon | n/a |

The `designed` / `committed` boundary is the **permanent/ephemeral artifact line**. Once every
permanent design artifact is done, the entry is `designed`; adding the disposable actioning
plan makes it `committed`. The plan's ephemerality is *why* it marks commitment. A throwaway
task list only gets generated once the work is being actioned against a release. `designed`
precedes `committed` because release-fit can't be judged until the design (culminating in the
spec) is complete. The work generates the commitment, not the reverse.

**Structural fence (unchanged):** no tier is a build authorization, not even `committed`. A
tier is a readiness and commitment signal. The only build trigger is an explicit
feature-altitude pass on the entry. The tiers track how far a feature's pipeline has
progressed; they don't widen the build trigger.

> Provenance pointers reference decisions by name and lock-date. In a multi-repo project, much
> of the locked provenance lives in a maintainer-internal architecture record that a public
> repository artifact can't cite by path. So pointers name the decision and its lock-date rather
> than an internal path (see Open Decision PRV-1).

### Idea

Captured directions. Either unvalidated, or decision-locked but not yet through their own
pipeline. A provenance pointer, where one exists, records that a decision was taken; it does
**not** promote the tier. Only a dedicated intake does that. **This tier is the scope-anchor
surface: research and discovery read `idea`-and-up so an intended direction is never silently
dropped to what-exists-live.**

- **Search + indexing activation** (full-text + vector) [D1]. **Promoted to `proposed` @ V0.1,
  retrieval cluster** (2026-07-02). See Designed §V0.1 for the live entry; this pointer stays so
  the D1 reference remains resolvable. Provenance: V0 discovery, Deferred section (locked
  2026-05-02).
- **First-class graph edges + traversal** [D2]. **Promoted to `proposed` @ V0.1, retrieval
  cluster** (2026-07-02). See Designed §V0.1 for the live entry; the pointer stays so the D2
  reference remains resolvable. Provenance: as D1.
- **`get_context_for(artifact_id)`, the agent-context bundle composer** [D3]. **Promoted to
  `proposed` @ V0.1 / `1.1.0`** (2026-05-20). See Designed §V0.1 for the live entry; this
  pointer stays so the D3 reference remains resolvable.
- **Bidirectional issue↔code via commit-ref convention** [D4]. Provenance: as D1.
- **New first-class artifact types beyond the migrated set** [D5]. Provenance: as D1.
- **Row-level-security policy enforcement** [D6]. Provenance: as D1.
- **External-authorization-server integration** [D7]. Provenance: as D1.
- **Internal-workspace absorption** (orchestration skills, roles, memory, and inboxes migrate
  or become projections) [D8]. Provenance: as D1.
- **Multi-orchestrator-per-project topology** [D9]. Provenance: as D1.
- **Third-party plugin install** [D11], gated on a documented ABI (application binary
  interface) evolution policy. Provenance: as D1.
- **Host-fn ABI 1.0 stabilization** [D12]. Provenance: as D1.
- **Cross-artifact authoritativeness + provenance/use-policy substrate fields** (G2/G3).
  **Promoted to `proposed` @ V0.1, retrieval cluster** (2026-07-02). See Designed §V0.1 for the
  live entry; the pointer stays so the G2/G3 reference remains resolvable. Provenance:
  knowledge-object survey dogfooding-lens amendment (2026-05-15), reclassified to a substrate
  concern there. The retrieval cluster is the pipeline run it rides.
- **Knowledge-object substrate shape** (a frontmatter shape, not a new artifact type; an OCC
  (optimistic concurrency control) version field; extensible typed audit events). Provenance:
  as G2/G3.
- **Knowledge-object family schemas + judge-extender + review-queue + memory-inspector +
  memory-write-back discipline** (F1/F3/F4/F5/F7). Provenance: as G2/G3 (forward-context, no
  consumer yet).
- **Multi-language plugin authoring** (Rust-first now; JS/TS, Python, TinyGo; later C#).
  Provenance: architecture overview V0/V1 boundary.
- **microVM appliance posture for self-host.** Conditional; the trip-wire is streamable-HTTP
  becoming the active transport (a named-capability condition, not release-gated). The substrate
  shortlist is recorded in the hosting research. Provenance: microVM hosting research
  (2026-05-18).
- **Per-tenant microVM isolation (managed tier).** Conditional on a multi-tenant managed
  audience existing; Firecracker is the substrate; it pairs with row-level security rather than
  replacing it. Provenance: as above (Lens B).
- **Tenant hierarchy** (org and higher layers above the workspace=tenant boundary) for full
  multi-tenancy. Deferred, and safe to defer because the tenant scoping key is structural from
  V0 (see the Tenancy invariant). Provenance: knowledge-object-survey scope sketch
  (`visibility: …|workspace|org`).
- **A dispatch CLI (external component, separate brief forthcoming).** A sibling component in
  the mnemra umbrella with its own repository, its own brief, and its own independent version.
  It's operationally required before mnemra-core V0 build begins: mechanical tasks heavily
  consume the premium model tier, and this CLI routes them and drives that cost down. Built
  *using* the completed intent → frame → spec pipeline, it later runs as a mnemra plugin (a
  workspace-era CLI and a mnemra-era WASM plugin sharing an IO-free core). Provenance: maintainer
  sequencing decision 2026-05-18. Referenced here as a build-time dependency, not absorbed into
  this brief's register.
- **A spec-delta/merge tool (external component, separate brief forthcoming).** A sibling
  component with its own repository, brief, and version. It's operationally required before
  mnemra-core V0 build begins: it's the structured-delta consumer this brief's format
  forward-contracts with, needed for living-document updates. Built *using* the completed
  intent → frame → spec pipeline, it later runs as a mnemra plugin. Provenance: as above.
  Referenced as a build-time dependency, not absorbed.
- **A markdown review/annotation tool, hosted under the mnemra umbrella when published.** A
  sibling product; tentative. Provenance: maintainer note 2026-05-18.
- **Context-intelligence plugin.** Project-aware code understanding for reviewers (a decisions +
  language-server composite, sidecar). Conditional on such a plugin surface existing. Provenance:
  an external algorithms-research review.
- **Byte-level provenance-tracing reference.** A provenance-deficit (not correctness)
  hallucination check. It's a reimplementation reference for a future verification plugin, keyed
  on the G2/G3 provenance direction; reimplement, don't port. Provenance: an external
  algorithms-research review (reimplementation-feasibility follow-up pending).
- **Managed/Cloud tier and Enterprise tier.** Product expansion beyond the OSS core; commercial
  gating lives in a separate internal record, not here.
- **Connectors/ingestion beyond the dev-adjacent wedge.** Opinionated and demand-driven; which
  ones, and when, are unvalidated.
- **External context/memory vocabulary adoption.** Shape convergence is evidenced; the label
  lock is deferred until a second consumer joins.
- **Dashboard interface.** A maintainer-side console product surface (a review/annotation tool
  may host here).
- **End-user CLI/TUI** distinct from the admin control CLI; **plugin registry/marketplace +
  signing/distribution as a product surface**; **hosted web console**;
  **onboarding/docs-as-product**; **agent-framework-specific integrations** beyond generic MCP.
  All valid under the product umbrella, all unvalidated.
- **A newsfeed capability** (working name). Pure thought, no provenance.
- **A permissions model.** Pure thought; it explicitly needs research into the approach (see
  Open Decision OD-B).
- **Per-user identity machinery.** Owner-columns-as-identity, caller identity in the request
  context, real owner==caller checks. It activates per-user `visibility: owner-only` serving
  semantics (at V0 owner-only serves no one; the fail-closed enforcement is the V0 semantics)
  and is the first write/label-capability surface that fires the deferred write-side
  policy-dimension design. The retrieval cluster's new tables carry `owner`/`created_by` columns
  from day one, so this lands as a feature rather than an excavation. Provenance:
  retrieval-cluster frame pre-gate walk item 9 (locked 2026-07-02). *(ADDED 2026-07-02.)*

### Proposed

Has a locked intake (through intent capture). Not yet frame+spec complete, and not
release-bound.

**V0 is delivered as a staged increment sequence, not a monolithic release**, and the
register extends beyond V0 into the V0.1 (post-`1.0.0`) immediate roadmap. "V0" denotes
the dogfood-replacement milestone; "V0.1" denotes the very-next-update phase (see Success
criteria for the marketing-tier vs SemVer mapping). Each increment below is its own register
entry carrying its own tier. The intake here is by retrofit: a locked, high-stakes,
reviewed-to-zero-findings V0 discovery (locked 2026-05-02) scopes the V0 contents, and the
V0.1 entries lock at this brief's altitude per maintainer ruling 2026-05-20. The increment
decomposition was captured in the 2026-05-18 product-intake refine and extended 2026-05-20.
Every increment is therefore `proposed`. None is `designed` (no per-increment frame+spec
exists) or `committed` (no plan, no release-bound date). The honest empty `designed`/`committed`
tiers below are preserved, not papered over.

**Versioning scheme (resolves INCR-1): Semantic Versioning 2.0.0, applied without abuse:**
- Each increment delivers backward-compatible **functionality**, so each is a **minor**
  bump within initial development: `0.1.0` (host core) → `0.2.0` → … (SemVer §7; §4:
  `0.y.z` is initial development, where the API isn't yet stable, which is exactly the V0 build
  period).
- Backward-compatible fixes within an increment are **patch** bumps (`0.N.1`, `0.N.2`; §6).
- The commit identifier is **build metadata** appended with `+` (`0.2.0+a1b2c3d`), ignored
  in precedence (§10, §11). It is **not** a `-` pre-release identifier. Pre-release has
  *lower* precedence than the release (§11.3, `1.0.0-alpha < 1.0.0`), which would
  misrepresent a delivered increment. Trunk commits advance the `+sha`; the version number
  bumps only when an increment's functionality is realized.
- **`1.0.0` is the dogfood-cutover / MVP.** SemVer §5: `1.0.0` defines the public API, which is
  precisely the V0 success criterion (the agent-facing surface and storage-substrate contracts
  are locked, and the maintainer's workspace runs fully on mnemra-core). The road to `1.0.0` is
  the `0.y.z` increment sequence below.

The sequence is **builtin-substrate-first, then one capability family per increment**,
ordered by dogfood value and dependency, with the maintainer's stated priority (tasks first
after substrate). A one-clause ordering rationale accompanies each entry, so the sequence
can be reordered cheaply at the intake-exit gate without restructuring entries.

- **`0.1.0`: Builtin substrate + host core.** Single-process Postgres (pgvector); the
  content and state storage-shape partitions persisted in-app, with the former timeseries
  and log shapes emitted to the observability minimum rather than stored (P-0010 D8); the pre-1.0
  host-fn ABI; an MCP server skeleton (stdio) onto which each capability increment adds its
  verbs; the admin/destructive control CLI; an observability minimum; an **LLM-API-key
  configuration surface** (mnemra-core calls out to an external model for *generative* work;
  embeddings and reranking run host-side on local non-generative encoder models per RC-1; the
  key is configured per deployment, never hard-coded, and never used to host a model)
  *(MODIFIED 2026-07-02 per RC-1; was: "calls out to an external model for embeddings per the
  architecture-overview ELT subsystem")*; **and the builtin tenancy/identity core: workspace
  (the tenant boundary; solo collapses to `default`), users, agents (tied to user-workspace
  pairs), authentication (a workspace claim in every token; per-deployment OIDC (OpenID Connect)
  via RFC 9728; a static dev-token first-run bootstrap), agent sessions, per-plugin permissions,
  projects.** Projects and agents are *builtin*, not plugins: plugins are scoped per project, so
  a project can't itself be a plugin (a host-bootstrap chicken-and-egg). *Order: nothing runs
  without the spine, and every capability family is scoped within workspace + project.* Tier:
  `proposed`. Provenance: V0 discovery (locked 2026-05-02) + architecture-alignment record
  (2026-04-27, tenant/substrate rounds); decomposition from the 2026-05-18 product-intake refine.
- **`0.2.0`: Task management.** Task CRUD, subtasks/parent-id, status lifecycle, project +
  priority; migration of the prior structured task data. *Order: the maintainer-stated first
  priority after substrate; the operational spine the workspace dogfoods earliest; its project
  and agent references are satisfied by the builtin substrate.* Tier: `proposed`. Provenance: as
  `0.1.0`.
- **`0.3.0`: Dispatch metrics & lifecycle.** Dispatch start/event/record/finalize;
  per-dispatch tokens/duration/cost/tool-uses; the dispatch-event stream; the scope
  envelope. *Order: the maintainer-stated next priority; core to the orchestration workflow.*
  Tier: `proposed`. Provenance: as `0.1.0`.
- **`0.4.0`: Skill-run measurement.** Tracks runs of a *skill* (a named, reusable agent
  capability: a specific dispatch shape, a structured review protocol, or an elicitation loop).
  Each run is measured: start/end timestamps, per-run consultations / review-rounds / flags
  tallies, knowledge-extraction capture, and a structured *retro*. The retro is the formal
  "review-after" capture in a trust-then-review workflow, where after a run the operator
  selectively reviews what the agent decided, flags divergences, and records what was learned;
  the structure makes findings aggregable across runs. Skill-run measurement operates at the
  substrate level and doesn't depend on a separate decision to migrate workspace skill
  definitions into mnemra-core (D8). The measurement substrate works whether skill definitions
  live in mnemra-core or stay as external files. *Order: sibling of dispatch metrics, the same
  measurement family.* Tier: `proposed`. Provenance: as `0.1.0`.
- **`0.5.0`: Activity / audit log.** The append-only actor/action/target/summary stream.
  *Order: low-complexity, high-value; underpins traceability across every later capability.*
  Tier: `proposed`. Provenance: as `0.1.0`.
- **`0.6.0`: Collaboration session friction tracking.** A *collaboration session* is the
  operator-with-team conversation container (distinct from `0.1.0`'s per-MCP-connection
  agent session, which is the technical auth/state primitive, MCP-protocol-defined). One
  collaboration session may span many per-MCP-connection agent sessions as the orchestrator
  dispatches to sibling agents. Friction events surface within a collaboration session
  along two axes:
  - **Event type (the friction *kind*):** `clarification` (operator needed to ask before
    acting), `revision` (operator changed something after delivery), `course-correction`
    (operator redirected the approach mid-task).
  - **Dimension (the friction *axis*):** `scope`, `Acceptance Criteria (AC)`, `context`,
    `routing`, `priority`.
  Each event row records the (event-type × dimension) tuple plus context. Aggregated over
  time, the rows surface friction patterns per collaboration session and across sessions, which
  is the measurement substrate for trust-then-review iteration. *Order: completes the
  measurement/audit triad with dispatch + skill-run + activity.* Tier: `proposed`.
  Provenance: as `0.1.0`.
- **`0.7.0`: Repo registry.** Repos per project (path / visibility / default-branch /
  remote). *Order: rides on builtin projects; pairs with the structural families.* Tier:
  `proposed`. Provenance: as `0.1.0`.
- **`0.8.0`: Relationships / edges.** Typed edges
  (parent / blocks / depends-on / supersedes / dispatched-by). *Order: the cross-cutting
  graph; valuable once tasks and projects exist to link.* Tier: `proposed`. Provenance: as
  `0.1.0`.
- **`0.9.0`: Tags / taggings.** Cross-cutting taxonomy. *Order: light; rides on the
  entities above.* Tier: `proposed`. Provenance: as `0.1.0`.
- **`0.10.0`: Dependency-approval state.** The approved-package register (the
  green/yellow/red license-tiering state). *Order: self-contained governance state.* Tier:
  `proposed`. Provenance: as `0.1.0`.
- **`0.11.0`: Scope-violation log.** The append-only scope-denial stream. *Order:
  self-contained, low-complexity.* Tier: `proposed`. Provenance: as `0.1.0`.
- **`0.12.0`: Job-search pipeline.** Applications / listings / search-runs; stale
  auto-reject. *Order: a distinct domain, lower coupling to the orchestration core.* Tier:
  `proposed`. Provenance: as `0.1.0`.
- **`0.13.0`: Contacts.** *Order: smallest and most isolated of the capability families.*
  Tier: `proposed`. Provenance: as `0.1.0`.
- **`0.14.0`: Content-corpus migration.** The prior markdown content corpus (the
  maintained knowledge subdirectories) → stored as files with frontmatter metadata, limited
  indexing (no full-text/vector; that's `idea` D1). *Order: placed after the
  structured-capability families per the maintainer's tasks/metrics-first priority; flagged
  as a reorder candidate at the intake-exit gate, since for a context-layer product a fair
  argument puts the corpus early as core value.* Tier: `proposed`. Provenance: V0 discovery
  §Migration scope (locked 2026-05-02); decomposition from the 2026-05-18 refine.
- **`1.0.0`: Dogfood cutover (public API defined).** The maintainer's workspace runs fully
  on mnemra-core with zero fallback to the prior tooling, and the agent-facing +
  storage-substrate contracts are locked. SemVer §5: this is where the public API is
  defined, the V0/MVP milestone. *Order: last in V0 by definition; it's the milestone gate.*
  Tier: `proposed`. Provenance: as `0.1.0`.

#### V0.1 (post-`1.0.0` immediate roadmap)

The very-next-update phase after MVP cutover: net-new value beyond V0 workspace-fidelity.
Each V0.1 entry starts `proposed` at this product altitude (this brief's intake locks the
phase placement); each entry's own feature-altitude intake locks its frame+spec before
build, promoting it to `designed` (see Designed §V0.1 for the entries that have reached it:
the retrieval cluster's four plus the extensible reporting engine). Maintainer ruling
2026-05-20: V0 means workspace-replacement (no regression); V0.1 means the core product
promise activates, plus operational follow-ups V0 deliberately didn't promise.

- **`1.1.0`: `get_context_for(artifact_id)` retrieval verb (core product promise).**
  **Promoted to `designed` @ V0.1 / `1.1.0`** (2026-07-02). See Designed §V0.1 for the live
  entry; the pointer stays so the `1.1.0` reference remains resolvable. Provenance:
  retrieval-cluster spec (`docs/specs/2026-07-02-retrieval-cluster.md`, locked 2026-07-02).
- **Search + indexing activation [D1] (retrieval cluster).** **Promoted to `designed` @ V0.1,
  retrieval cluster** (2026-07-02). See Designed §V0.1 for the live entry; the pointer stays so
  the D1 reference remains resolvable. Provenance: retrieval-cluster spec
  (`docs/specs/2026-07-02-retrieval-cluster.md`, locked 2026-07-02).
- **First-class graph edges + traversal [D2] (retrieval cluster).** **Promoted to
  `designed` @ V0.1, retrieval cluster** (2026-07-02). See Designed §V0.1 for the live
  entry; the pointer stays so the D2 reference remains resolvable. Provenance:
  retrieval-cluster spec (`docs/specs/2026-07-02-retrieval-cluster.md`, locked 2026-07-02).
- **Cross-artifact authoritativeness + provenance/use-policy substrate fields [G2/G3]
  (retrieval cluster).** **Promoted to `designed` @ V0.1, retrieval cluster** (2026-07-02).
  See Designed §V0.1 for the live entry; the pointer stays so the G2/G3 reference remains
  resolvable. Provenance: retrieval-cluster spec
  (`docs/specs/2026-07-02-retrieval-cluster.md`, locked 2026-07-02).
- **`1.2.0`: Ongoing ingest pipeline.** Watchers, scheduled polls, or webhooks that
  auto-detect and ingest new content arriving in the brain corpus after V0's one-shot batch
  migration (`0.14.0`). It's distinct from `0.14.0`: `0.14.0` is a one-shot move of the existing
  corpus; `1.2.0` is a continuous-arrival pipeline. *Order: operational follow-up to V0.1's
  headline; V0 covers batch only, V0.1 adds continuous ingest.* Tier: `proposed`.
  Provenance: architecture-overview ELT subsystem (ADR-16, an architecture decision record) +
  product-intake refine 2026-05-20 (OD-A resolved: distinct from `0.14.0`, deferred to V0.1).

- **`1.3.0`+ (candidate): Extensible reporting engine.** **Promoted to `designed`
  @ V0.1 / `1.3.0`+ (candidate)** (2026-07-04). See Designed §V0.1 for the live entry;
  the pointer stays so the `1.3.0`+ reporting-engine reference remains resolvable.
  Provenance: reporting-engine spec (`docs/specs/2026-07-03-reporting-engine.md`, locked
  2026-07-04; verify verdict passed_with_concerns).

Future V0.1 increments (`1.3.0`+) land here as the "very-next-update" trigger fires for
new capabilities.

**Build prerequisites (sequence, unchanged):** the V0 increment sequence's build is gated
on three external predecessors: the intent → frame → spec pipeline being complete (it's
being exercised and amended now), then the spec-delta/merge tool and the dispatch CLI being
operational. Both prerequisite tools are **external components** with their own forthcoming
briefs and their own independent versions (see the Idea section pointers). This brief
references them as build-time dependencies; it doesn't absorb them into its register.
`0.1.0` work begins only after those exist. The prerequisites gate the V0 sequence's
*start*, not its contents.

### Designed

A locked frame + locked spec exists. **There's also a maintainer-ratified exception the
`designed` tier admits on a locked Frame alone: the Frame-park, in two kinds.** Spec-*deferred*
is a Frame-park whose Stage-3 spec is a later maintainer pickup; the coordination wedge was
admitted this way and has since discharged its deferral, its spec now locked, so it stands as a
completed pickup rather than a live example of the kind. Spec-*terminal* is the strangler
migration program, where no Stage-3 spec will ever follow because the per-function bundles carry
the code-destined specs (see the V0 subsection below).

There are five tenants. The retrieval cluster: its spec
(`docs/specs/2026-07-02-retrieval-cluster.md`) locked 2026-07-02, promoting its four constituent
entries below from `proposed`. The extensible reporting engine: its spec
(`docs/specs/2026-07-03-reporting-engine.md`) locked 2026-07-04, promoting the `1.3.0`+
(candidate) entry below from `proposed`. The coordination wedge cluster: its Frame
(`docs/intent/coordination-wedge-frame.md`) locked 2026-07-07 and its spec
(`docs/specs/2026-07-06-coordination-wedge.md`) locked 2026-07-07 (companion ADR P-0022
accepted), a completed Frame-park pickup where the deferred Stage-3 spec has landed, so it's now
a full spec-locked designed tenant rather than a Frame-park. The plugin-distribution layer: its
spec (`docs/specs/2026-07-07-plugin-distribution.md`) locked 2026-07-11 (companion ADR P-0023
accepted), born net-new at `designed`. And the strangler migration program: its Frame
(`docs/intent/strangler-program-frame.md`) locked 2026-07-07 as a **terminal** Frame-park
designed-tier artifact of record, with no spec and none to follow. Stated explicitly: the
register doesn't infer design completion beyond what a locked spec actually covers, or, for a
ratified Frame-park entry, beyond what a locked Frame covers.

#### V0 (pre-`1.0.0` cutover substrate)

- **Coordination wedge cluster: actors + claims-as-leases + messaging (the workspace's
  first live workload).** The coordination plane inside the mnemra-core host. Durable actor
  identity: each role-instance is one durable `actors` row at the P-0018 grain (the core-entity
  manifest); sessions are ephemeral attachments that resolve-or-create the row at start, never
  minting a duplicate. Mutual-exclusion claims on the duration line as first-class **leases**:
  holder, structured resource identifier, TTL (time to live), and stale/takeover/release
  semantics; acquisition is substrate-enforced atomic, so exactly one live lease per resource
  can exist and the loser gets a structured refusal. And addressed **messaging** as rows with
  queue semantics and a closed disposition state machine (delivered → acknowledged →
  dispositioned, immutable after send, every consumption recorded). Two agent-visible MCP tools
  carry the surface, `claim` and `message` (actions as closed-enum parameters), inside the
  single-digit verb budget. The founding message type is `merge-request` (the merger-lane input
  contract rendered as schema fields), with a minimal generic `handoff` (subject + body +
  artifact refs) as a second type. Residence is the core host subsystem (the P-0002
  core-vs-plugin walk); observability is one unified target-tagged tracing stream, with the
  privileged subset also emitting to the P-0018 audit surface; the availability contract is
  fail-closed (no local write queue, no split-brain). It runs over **zero migrated content** by
  design: the system's first live workload, defining what "the workspace starts using mnemra"
  means. *Order: migration step 1 (ratified 2026-07-06), sequenced ahead of all content bundles;
  the first usage.* Tier: `designed` (full spec-locked: a locked Frame **and** a locked Stage-3
  spec; the Frame-park deferral that admitted this entry on a locked Frame alone is now
  discharged, per the Designed preamble's completed-pickup framing). Provenance:
  coordination-wedge intake (`docs/intent/coordination-wedge.md`, locked 2026-07-06, blob
  `a4931ef85789639f4a5e451a945f43901be17343`) + coordination-wedge Frame
  (`docs/intent/coordination-wedge-frame.md`, locked 2026-07-07, blob
  `4d1a8c211b11b984406249beef9e1156c0d7773d`) + coordination-wedge spec
  (`docs/specs/2026-07-06-coordination-wedge.md`, locked 2026-07-07, blob
  `81dc71953c9bb0de2b269b4da15ab394aed20f9c`), with companion ADR **P-0022**
  (`docs/src/adrs/P-0022-coordination-cluster.md`, accepted). The Stage-3 spec landing
  discharges the Frame-park deferral, so the entry now satisfies the full `designed` validator
  (a locked frame **plus** a locked spec). Born net-new at `designed`: the intake deferred the
  whole register entry to Frame-merge (open-item 4), so it was never at `proposed`. No Proposed
  pointer-stub is retained (nothing references it there, and it's not a SemVer increment), and no
  Idea-section pointer is retargeted (it wasn't promoted from a Deferred D-item).
  *(ADDED 2026-07-07; spec-promoted 2026-07-07.)*

- **Strangler migration program: the standing mechanics governing every brain→mnemra
  cutover (the coordination wedge is migration step 1).** The program scopes the
  multi-year migration of the workspace's knowledge substrate (the `brain/` tree + the
  `puck.db` task database) into mnemra as a standing, function-by-function strangler, so each
  per-function cutover runs as routine small-batch mechanics that reference this program
  rather than re-litigating sequencing, acceptance, rollback, and availability from scratch.
  Here's what it fixes. **One migration spine**: a dependency partial order (a DAG, a directed
  acyclic graph) whose every step's position derives from a named dependency, pinning the
  one-sentence rule that **core/plugin classification governs where a function lives (residence),
  never when it migrates (sequence)**. So memory is core yet migrates late, because its
  dependency chain (render bridge + compaction) gates it; "core first" as a sequence rule is
  dead. **A reusable cutover ritual**: statement (explicit, dated; authority transfers here and
  the old substrate freezes, so there's no dual-authority window) → migrate + live-row verify →
  backup (a snapshot with *demonstrated restorability*, undeletable until the standing daily
  backup has captured the function) → remove (one home, no stale shadow). Its preconditions are
  **mechanically fail-closed gates** (G-verify / G-backup / G-standing-backup: an unconfirmed
  check is treated as not-satisfied and removal is blocked), and its removal step carries a
  **standing authorization**: agent-executed deletion of the old copy without a per-instance
  human gate, safe precisely because it's structurally gated on the fail-closed checks rather
  than merely conditioned on them. **The availability ladder** the per-cutover hosting hardening
  (task #1056) gates against: the availability bar rises as each function reaches sole-home, any
  coordination/content write failure is an immediate observable stop, and there's no local write
  queue ever (a queue is split-brain by construction). **The complete work-list**: all 60 +
  1-adjacent census functions dispositioned from a closed six-member set (migrate-with-dependency
  / build-new / dissolve-into-substrate / retire-with-ritual /
  dissolves-with-substrate-at-decommission / out-of-scope), the flag-union of 17 rows reconciled
  against the census's literal cells and each ruled or routed, the four structural findings each
  ruled fix-now vs fix-at-cutover. **The program constraints** every per-function bundle inherits
  (small-increments operating mode; forward-only, no dual-run; no dual-authority window;
  drain-then-flip; operator-only-live flips; mechanize-the-removal-gate). And **the progress
  axis**: brain-shrinkage (the count of functions at sole-home in mnemra), made queryable rather
  than narrated via an in-repo **statements register**
  (`docs/intent/strangler-program-statements.md`, its shape locked in the Frame, the file created
  at the first cutover). Two dispositions were ruled at the Frame-exit gate (2026-07-07). The
  metrics cluster is **plugin** (dispatch/skill-run measurement is workflow-primitive data, named
  in the negative space of the P-0018 core-entity manifest: a fan-*out* consumer shape, not the
  fan-*in* a core promotion needs). And `projects` is the core fan-in root that every dependent
  hard-FKs to (a required foreign-key reference), so it precedes tasks (plugin); tasks are the
  first plugin-shaped migration step and so the honest forcing function for the P-0013 Bucket A
  component-host build. *Order: the program owns the mechanics for migration step 1 (the
  coordination wedge cluster, above) and every cutover after it; it sequences nothing on its own
  beyond the spine's dependency edges.* Tier: `designed` (**Frame-park, TERMINAL**: unlike a
  feature cluster whose Stage-3 spec is a later pickup, this program's Frame-park is terminal.
  The per-function bundles carry the code-destined specs, and the program never authors one; this
  Frame is the durable designed-tier artifact of record for the migration program's mechanics,
  per the Designed preamble's exception). Provenance: strangler-program intake
  (`docs/intent/strangler-program.md`, locked 2026-07-07, blob
  `6d58cd29b7e80e595e4492ab396975cbdf7f1de0`) + strangler-program Frame
  (`docs/intent/strangler-program-frame.md`, locked 2026-07-07, blob
  `805756411db1bca920c0a87889ec5ce8edf6eb3e`); NO spec (Frame-park, terminal: no Stage-3 will
  ever follow). Born net-new at `designed`: the intake deferred the register entry to Frame-merge
  (intake open-item 3), so it was never at `proposed`. No Proposed pointer-stub is retained
  (nothing references it there, and it's not a SemVer increment), and no Idea-section pointer is
  retargeted (it wasn't promoted from a Deferred D-item). No program ADR slot (Frame §12: a
  terminal Frame-park mints none, because there's no downstream spec-authoring step to fill a
  program slot, and the Frame is itself the reference every per-function bundle cites; the
  ADR-slot ledger is left untouched). *(ADDED 2026-07-07.)*

- **Plugin distribution + package signing: the distribution layer (W2-1).** The layer that
  makes a plugin a distributable unit whose whole-bundle provenance and integrity are
  established **before unpacking**. Every plugin ships as one uniform multi-artifact OCI (Open
  Container Initiative) bundle (N≥1 digest-addressed blobs, the component `.wasm` always artifact
  #1, the signed inner TOML manifest as the config descriptor, uncompressed-only at V0). The
  package signature is the P-0005 Ed25519 root over the domain-separated outer-manifest digest,
  attached as an OCI 1.1 referrer and verified on fetch under signer-key-pinning (never
  trust-on-first-use, no unsigned fall-through). Two transports satisfy R-NoExternalHost (R-codes
  are stable requirement identifiers defined in the spec): an OCI image-layout filesystem for
  true air-gap, and a self-hosted distribution-API registry for restricted-egress LAN (local area
  network), both behind one store contract with digest-pinned resolution recomputed over received
  bytes. The bounds-first verified-fetch pipeline (fetch-within-bounds → verify-package-signature
  → verify-blob-digests → unpack-within-bounds, fail-closed at every stage) sits behind the locked
  `PackageVerifier` seam (the TUF (The Update Framework) adapter's slot, fires at R-0005-e). The
  inner `[[artifacts]]` N≥1 binding complete-mediates every blob at the provenance anchor (P-0003
  §Amendment 2026-07-07, strictly superseding the single `[component]` hash). A hard cutover
  retires the bare load path with no legacy accept window. And install is a **working-state →
  working-state** transition: no failure, crash, or kill point leaves the host's serving or
  load-eligible state indeterminate, and residue from a failed attempt is inert (R-0092, the
  maintainer's 2026-07-11 spec-exit-gate ruling; update is forward-bound by the same invariant,
  its flow deferred). Single-root custody exposure is recorded, never silent (accepted risks
  R-0008 rollback residual / R-0009 single-root / R-0010 build-time dependency confusion; all
  tripwired). *Order: Wave-2 design-lane bundle W2-1 (board order); implementation is a
  committed-tier pickup, gated by base-pin freshness.* Tier: `designed` (full spec-locked).
  Provenance: plugin-distribution intake (`docs/intent/plugin-distribution.md`, locked
  2026-07-07, blob `9c8e1577ed345cbcef546ba51d252a0df4db1144`) + plugin-distribution Frame
  (`docs/intent/plugin-distribution-frame.md`, locked 2026-07-08Z, blob
  `60c437c5e569eacbf00329e92cae2e7c42cebba6`) + plugin-distribution spec
  (`docs/specs/2026-07-07-plugin-distribution.md`, locked 2026-07-11, blob
  `10554ccdd6ae91731086b6fa6cba3de281a4fd49`, R-0078 through R-0092 with the R-0082 tombstone),
  with companion ADR **P-0023** (`docs/src/adrs/P-0023-plugin-distribution.md`, accepted) + the
  P-0003 `[[artifacts]]` amendment + the P-0019 DEF-2 disposition (that deferral was pulled
  deliberately ahead of its third-party tripwire, at single-publisher `core: true` scope). Born
  net-new at `designed`: no Proposed pointer-stub is retained and no Idea-section pointer is
  retargeted. The Idea-tier "plugin registry/marketplace + signing/distribution as a product
  surface" umbrella is a distinct, unvalidated product surface and stays where it is; this layer
  is substrate infrastructure at single-publisher scope, not that product. *(ADDED 2026-07-11.)*

#### V0.1 (post-`1.0.0` immediate roadmap)

- **`1.1.0`: `get_context_for(artifact_id)` retrieval verb (core product promise).** A
  one-call MCP retrieval of persistent, typed, cross-session context for a given artifact,
  rather than reconstructing it by hand each session. The headline V0.1 capability: the
  first net-new value-add over V0's workspace-fidelity baseline. *Order: V0.1's headline
  promise; first net-new value over V0.* Tier: `designed`. Provenance: V0 discovery (D3,
  locked 2026-05-02) + product-intake refine 2026-05-20 (scheduled at V0.1 / `1.1.0`) +
  retrieval-cluster spec (`docs/specs/2026-07-02-retrieval-cluster.md`, locked
  2026-07-02). **Promoted 2026-07-02 (MODIFIED):** covered by the retrieval-cluster
  feature-altitude intake (locked 2026-07-02) as one clustered feature with D1, D2, and
  G2/G3 below, a single intake/frame/spec pipeline; the frame locked 2026-07-02 and the spec
  locked 2026-07-02, satisfying the `designed` validator (a locked frame **plus** a locked
  spec).
- **Search + indexing activation [D1] (retrieval cluster).** Batch indexing pipeline over
  the corpus the substrate already holds (per-shape chunking, authored-tree summary
  nodes, local embeddings, full-text search) plus the agent-facing `search` verb: hybrid
  FTS (full-text search) and dense retrieval fused by Reciprocal Rank Fusion with local rerank,
  budget-shaped. Part of the retrieval cluster (one clustered feature with `1.1.0`, D2, G2/G3).
  *Order: rides with `1.1.0`; the headline verb needs the index.* Tier: `designed`. Provenance:
  V0 discovery D1 (locked 2026-05-02); retrieval-cluster intake (locked 2026-07-02), whose lock
  performed the `idea → proposed` promotion; retrieval-cluster spec
  (`docs/specs/2026-07-02-retrieval-cluster.md`, locked 2026-07-02) performed the
  `proposed → designed` promotion. *(ADDED 2026-07-02.)*
- **First-class graph edges + traversal [D2] (retrieval cluster).** Typed, traversable
  edges extracted from the authored-but-latent sources (frontmatter relation lists,
  free-text citations) with provenance discrimination; **the `0.8.0` edge-table substrate
  is what this traversal activates**: one superset vocabulary, one traversal path
  (recursive CTEs, common table expressions, per the storage-substrate decision). Part of the
  retrieval cluster. *Order: as D1.* Tier: `designed`. Provenance: V0 discovery D2 (locked
  2026-05-02); retrieval-cluster intake (locked 2026-07-02), whose lock performed the
  `idea → proposed` promotion; retrieval-cluster spec
  (`docs/specs/2026-07-02-retrieval-cluster.md`, locked 2026-07-02) performed the
  `proposed → designed` promotion. *(ADDED 2026-07-02.)*
- **Cross-artifact authoritativeness + provenance/use-policy substrate fields [G2/G3]
  (retrieval cluster).** The substrate fields the retrieval envelope reads and serves:
  trust provenance (authoritative/outdated/background), the policy permissions record
  (dont-use, model-egress, visibility, tenant-share), freshness handles + decay classes,
  and decision axes. Part of the retrieval cluster. *Order: as D1; the envelope's
  substrate.* Tier: `designed`. Provenance: knowledge-object survey (2026-05-15);
  retrieval-cluster intake (locked 2026-07-02), whose lock performed the `idea → proposed`
  promotion; retrieval-cluster spec (`docs/specs/2026-07-02-retrieval-cluster.md`, locked
  2026-07-02) performed the `proposed → designed` promotion. *(ADDED 2026-07-02.)*
- **`1.3.0`+ (candidate): Extensible reporting engine.** One report surface backed by a
  registry: canonical built-in reports (which ride their V0 capability-family increments
  as workspace-fidelity content) plus declarative, runtime-added, **read-only** user
  reports invoked via MCP (admin-CLI convenience is secondary). Read-only execution is the
  identity invariant (defense-in-depth, re-derived for the Postgres substrate). Because
  report queries are user-authored, the guard graduates from operator-mistake to
  adversary: workspace isolation, the role matrix, and the provenance/policy-envelope
  predicates must hold on a caller-written query, which is the Frame threat model's
  headline boundary. It's deliberately sequenced after `1.1.0`, because the policy-envelope
  enforcement machinery this surface must honor lands with the retrieval feature.
  *Order: value activates once the measurement families hold data and the envelope
  machinery exists.* Tier: `designed`. Provenance: reporting-engine intake (locked
  2026-07-03) placed the entry at `proposed`; reporting-engine spec
  (`docs/specs/2026-07-03-reporting-engine.md`, locked 2026-07-04) performed the
  `proposed → designed` promotion (verify verdict passed_with_concerns). *(ADDED 2026-07-04.)*

### Committed

`designed` plus a plan, release-bound. **Empty.** The retrieval cluster and the extensible
reporting engine are now `designed`, but no feature has yet moved `designed → committed`
(no committed-tier plan exists, and no release has a committed date). That's consistent with
the product's stated posture that a phase commits a date only when work is far enough along.
Stated explicitly: the register doesn't over-claim commitment. An empty `committed` tier
early in a project is the register working, not a gap.

### Live

Built and verified in current code/canon.

- **mnemra-core pre-`0.1.0` substrate spike.** A host instantiates a WebAssembly Component
  Model plugin over a typed contract, with host-fn round-trips and host-side state
  persisting across invocations on the `wasm32-wasip2` toolchain. Verifiable: the
  mnemra-core repository (host crate, first plugin crate, contract package; spike commit on
  `main`).
- **Landing site.** `mnemra.dev`, Astro on Cloudflare Pages; deployed.
- **GitHub organization.** `github.com/mnemra`, README published.
- **Email waitlist + social presence.**
- **Developer-docs scaffolding.** mdBook site with an ADR section and template.

## Open Decisions (resolve at the intake-exit gate)

Surfaced for the decomposer; not resolved in this draft. Source conflicts, unknowns, and
unsettled scope are named, not papered over.

- **APPARATUS-1: confirmed (tracked separately as a register-model amendment task).** The
  canonical product-brief register model (previously four-tier, `committed` before
  `designed`) is mis-ordered and missing `proposed` for release-commitment semantics. The
  amendment is: five tiers `idea → proposed → designed → committed → live`, each validated by
  a pipeline artifact, with the permanent/ephemeral boundary at `designed`|`committed`;
  plus the spec-is-permanent / plan-is-ephemeral distinction promoted to general workspace
  canon; plus the multi-repo product-brief-home gap (DEFER-1). Do it now while the
  structured-delta consumer doesn't yet exist (zero forward-contract migration; deferring
  means a contract break later). Tracked as a separate amendment task/ADR, not a mid-run edit.
- **INCR-1: resolved.** V0 decomposed into a builtin-substrate-first,
  one-capability-per-increment staged sequence. Versioning is Semantic Versioning 2.0.0
  applied without abuse: each feature increment is a **minor** bump within `0.y.z` initial
  development (`0.1.0` host core → `0.14.0`), backward-compatible fixes are patch, the commit
  is pinned as `+build` metadata (not a lower-precedence `-pre-release`), and **`1.0.0`** is the
  dogfood-cutover/MVP where the public API is defined (SemVer §5). Applied this round; see
  the Proposed section, where the per-entry ordering rationale supports cheap reordering at this
  gate. The `{projects, agents}`-as-core-plugins question is resolved upstream (builtin
  substrate; per-project plugin chicken-and-egg). The stale architecture-alignment-record
  framing is flagged in the maintainer-internal intake record for a separate downstream
  amendment, not corrected here. The apparatus-relevant residue (the *canonical* register
  model expressing staged/incremental delivery) folds into APPARATUS-1, and is no longer an
  open question for this brief.
- **LIC-1: resolved.** Apache-2.0 + future-relicense clause locked 2026-05-20. The
  mnemra-core repository's current LICENSE/README (MIT) is corrected in a separate
  follow-up task. Stronger contributor IP grant; the future-relicense clause preserves the
  future commercial-managed-tier option.
- **BIPT-1: resolved.** Split into two: the committed provenance direction (G2/G3) is an
  `idea`-with-provenance entry, and the byte-level provenance-tracing technique is an
  `idea` reimplementation-reference entry. No longer an open tiering question.
- **PRV-1: resolved.** Decision-name + lock-date confirmed as the provenance-pointer
  convention (locked 2026-05-20). The pattern is already in use throughout the brief; a public-
  repo artifact can cite internal-record decisions by name and date without exposing
  internal paths.
- **DEFER-1: resolved (relocated).** Brief moved from the landing-site repository to
  the mnemra-core repository at `docs/src/intent/mnemra-core.md` (2026-05-20). The brief
  lives with the component it scopes (per-repo-first). APPARATUS-1's broader multi-repo
  product-brief-home convention question still applies for future multi-repo briefs in
  other components.
- **AMEND-1: confirmed routed to retrospective.** Both drift items in the mnemra-core
  project context file, (a) the "bare mnemra = mnemra-core" shorthand being wrong at product
  altitude and (b) the landing-site framework migration listed as "not started" though it
  shipped, batch into a retrospective doc for corrective action (locked 2026-05-20).
- **T-5: resolved (split).** (a) **Categorization-via-LLM-API:** `idea` tier, because V0 is
  workspace-replacement (no auto-categorization today); a net-new value-add candidate for
  V0.1+ once the use-case shape solidifies. (b) **LLM-API-key configuration surface:** V0,
  folded into the `0.1.0` substrate description (mnemra-core calls an LLM for embeddings at V0
  per the architecture-overview ELT subsystem; the config surface is required from substrate
  onward). Locked 2026-05-20.
- **OD-A: resolved.** Content-import / ongoing ingest pipeline is distinct from
  `0.14.0`'s one-shot batch migration. Deferred to V0.1 (placed at `1.2.0`; see Proposed
  §V0.1) per maintainer ruling 2026-05-20. V0 covers batch migration only.
- **OD-B: resolved.** Permissions model: a separate scoped research dispatch is queued
  for after intake-exit (non-blocking on this gate). The capability stays `idea` until
  research informs its shape; the result feeds the future permissions-model intake. Locked
  2026-05-20.
- **T-7: resolved.** "Team" == workspace/tenant (aligned; one self-hosted instance per
  tenant). "Team" is an informal user-grouping inside a workspace, not a distinct layer.
  The deferred hierarchy is **1-layer** (org above the workspace=tenant boundary, when
  multi-tenancy lands). Locked 2026-05-20.
- **µVM-OQ1..4: confirmed parked.** libkrun copyleft-tier acceptance; the GPL
  process-boundary stance for an appliance; the appliance trip-wire wording; the managed-tier
  Postgres shape (per-tenant VM ≠ per-tenant Postgres). All deferred-until-trip-wire
  (streamable-HTTP becoming the active MCP transport), non-blocking. Locked 2026-05-20.
- *Most earlier per-item tier-ambiguities dissolved under the v4 model:* items without
  their own pipeline run are uniformly `idea`-with-provenance, so no per-item tiering
  ruling is needed. The model removing those decisions is a model-quality signal.

## Changelog

- **2026-07-11**: Plugin-distribution Stage-3 spec locked
  (`docs/specs/2026-07-07-plugin-distribution.md`, blob
  `10554ccdd6ae91731086b6fa6cba3de281a4fd49`; spec-exit gate accepted 2026-07-11) over the
  locked Frame (`docs/intent/plugin-distribution-frame.md`, blob
  `60c437c5e569eacbf00329e92cae2e7c42cebba6`, locked 2026-07-08Z) and intake
  (`docs/intent/plugin-distribution.md`, blob `9c8e1577ed345cbcef546ba51d252a0df4db1144`,
  locked 2026-07-07), with companion ADR **P-0023**
  (`docs/src/adrs/P-0023-plugin-distribution.md`) status **accepted**, the P-0003
  `[[artifacts]]` §Amendment 2026-07-07, and the P-0019 DEF-2 disposition. Register: a new
  V0 tenant **added, born net-new at `designed`** (full spec-locked: locked frame + locked
  spec + accepted companion ADR): the plugin-distribution layer (uniform multi-artifact OCI
  bundle; keyed-in-tree package signature verified on fetch; two R-NoExternalHost transports
  behind one store contract; the bounds-first `PackageVerifier` pipeline; the `[[artifacts]]`
  complete-mediation binding; hard cutover retiring the bare load path; install atomicity
  R-0092 per the maintainer's 2026-07-11 gate ruling, with update forward-bound). Designed
  preamble updated four → five tenants. No Proposed pointer-stub and no Idea retarget (the
  registry/marketplace product-surface idea is distinct and unmoved).
- **2026-07-07**: Coordination-wedge Stage-3 spec locked
  (`docs/specs/2026-07-06-coordination-wedge.md`, blob
  `81dc71953c9bb0de2b269b4da15ab394aed20f9c`; spec-exit gate accepted 2026-07-07) over the
  locked Frame (`docs/intent/coordination-wedge-frame.md`, blob
  `4d1a8c211b11b984406249beef9e1156c0d7773d`, locked 2026-07-07), with companion ADR
  **P-0022** (`docs/src/adrs/P-0022-coordination-cluster.md`) status **accepted**. Register:
  **Frame-park deferral discharged**. The coordination-wedge V0 tenant, admitted at
  `designed` on a locked Frame alone with its Stage-3 spec deliberately deferred to a later
  maintainer pickup, now carries the spec that deferral promised, so it satisfies the full
  `designed` validator (locked frame **plus** locked spec) and reads as a completed
  Frame-park pickup, no longer a Frame-park. The V0 tenant entry is rewritten from the
  Frame-park framing ("NO spec, Stage-3 deferred") to the full spec-locked record (spec path
  + blob, companion ADR P-0022 accepted; the intake + Frame chain retained), and its trailing
  marker records the spec-promotion. The Designed preamble is reworded so the two-kinds
  Frame-park taxonomy (spec-*deferred* / spec-*terminal*) is unchanged, the strangler
  migration program stays the sole live Frame-park exemplar (terminal), and the coordination
  wedge reads as a completed pickup of the spec-*deferred* kind rather than its live example,
  with no third category introduced. In-place within-tier promotion: no new register entry, no
  Proposed pointer-stub or Idea-section retarget (there were none, since the entry was born
  net-new at `designed`), and the tier stays `designed`. The Layer-2 tier validator ("locked
  frame + locked spec") is untouched: the entry now meets it directly, so the named-instance
  Frame-park exception no longer applies to the wedge.
- **2026-07-07**: Strangler-program Frame locked
  (`docs/intent/strangler-program-frame.md`, blob
  `805756411db1bca920c0a87889ec5ce8edf6eb3e`; Frame-exit gate accepted 2026-07-07) over the
  locked intake (`docs/intent/strangler-program.md`, blob
  `6d58cd29b7e80e595e4492ab396975cbdf7f1de0`, locked 2026-07-07). Register: **net-new
  `designed`-tier entry** for the strangler migration program, the standing mechanics governing
  every brain→mnemra cutover (one dependency-derived migration spine with the core≠order rule;
  the fail-closed cutover ritual with a standing removal authorization; the availability ladder;
  the complete 60 + 1 work-list dispositioned from a closed six-member set; the program
  constraints every bundle inherits; the brain-shrinkage progress axis via an in-repo statements
  register). It's appended to Designed §V0 (pre-`1.0.0` cutover substrate) after the
  coordination-wedge entry (migration step 1), the tier's fourth tenant. **Frame-park promotion,
  TERMINAL:** the entry lands at `designed` on a locked Frame alone, and, unlike the coordination
  wedge's spec-*deferred* Frame-park, **no Stage-3 spec will ever follow**: the per-function
  bundles carry the code-destined specs, and the program never authors one. The Designed preamble
  is broadened to name the two kinds of Frame-park exception (spec-deferred/temporary vs
  spec-terminal). Two dispositions ruled at the Frame-exit gate: the metrics cluster is
  **plugin** (P-0018 D-BOUNDARY: dispatch/skill-run measurement is workflow-primitive, named in
  the core-entity manifest's negative space; fan-out, not the fan-in a core promotion needs), and
  the standing removal authorization stands as a locked Frame direction (no governance ADR minted;
  re-opens on a second program-shaped standing authorization). Born net-new at `designed` (the
  intake deferred the whole register entry to Frame-merge per open-item 3, so it was never at
  `proposed`): no Proposed pointer-stub is retained and no Idea-section pointer is retargeted.
  Register-promotion performed at Frame-merge time per the register-promotion convention (the
  wedge's precedent; Frame §15), riding the docs PR orchestrator-side. **No program ADR slot**
  (Frame §12): a terminal Frame-park mints none, because per-function bundles carry the specs, so
  no downstream spec-authoring step fills a program slot, and the Frame is itself the reference
  every per-function bundle cites; the ADR-slot ledger (`docs/src/adrs/placeholder-resolution.md`)
  is left untouched. The Layer-2 tier validator ("locked frame + locked spec") is left unamended:
  a named instance exception, not a register-model change (the model-level question is
  APPARATUS-1's).
- **2026-07-07**: Coordination-wedge Frame locked
  (`docs/intent/coordination-wedge-frame.md`, blob
  `4d1a8c211b11b984406249beef9e1156c0d7773d`; Frame-exit gate accepted 2026-07-07) over the
  locked intake (`docs/intent/coordination-wedge.md`, blob
  `a4931ef85789639f4a5e451a945f43901be17343`, locked 2026-07-06). Register: **net-new
  `designed`-tier entry** for the coordination wedge cluster (durable actor identity,
  claims-as-leases with TTL/takeover, addressed messaging with a closed disposition state
  machine, two agent-visible MCP tools, the `merge-request` founding type; the workspace's
  first live workload over zero migrated content, migration step 1 ahead of all content
  bundles). It's added under a new Designed §V0 (pre-`1.0.0` cutover substrate) subsection, the
  tier's third tenant after the retrieval cluster and the reporting engine. **Frame-park
  promotion:** the entry lands at `designed` on a locked Frame **alone**, its Stage-3 spec
  deliberately deferred to a later maintainer pickup: a maintainer-ratified exception to the
  `designed` validator's "locked frame + locked spec", named in the Designed preamble and the
  entry's provenance rather than papered over. Born net-new at `designed` (the intake deferred
  the whole register entry to Frame-merge per open-item 4, so it was never at `proposed`): no
  Proposed pointer-stub is retained (nothing in Proposed references it and it's not a SemVer
  increment) and no Idea-section pointer is retargeted (not promoted from a Deferred D-item).
  Register-promotion performed at Frame-merge time per the register-promotion convention
  (intake open-item 4; Frame §14.7), riding the docs PR orchestrator-side. The Layer-2 tier
  validator ("locked frame + locked spec") is left unamended: this is a named instance
  exception, not a register-model change (the model-level question is APPARATUS-1's).
- **2026-07-04**: Reporting-engine spec locked
  (`docs/specs/2026-07-03-reporting-engine.md`; verify verdict passed_with_concerns).
  Register: `proposed → designed` promotion for the `1.3.0`+ (candidate) extensible
  reporting engine, moved from Proposed §V0.1 into Designed §V0.1 (the tier's second
  tenant, after the retrieval cluster), with a pointer-stub retained in Proposed §V0.1.
  No Idea-section pointer is retargeted: the entry was born at `proposed` via the
  2026-07-03 intake (not promoted from a Deferred D-item), so it never had one.
  Register-promotion deferred to merge per the f0f570d direct+stubs convention (the
  design branch's base predated main's post-retrieval-cluster register state).
- **2026-07-02**: Retrieval-cluster spec locked
  (`docs/specs/2026-07-02-retrieval-cluster.md`). Register: `proposed → designed`
  promotion for the four clustered entries: `1.1.0` (`get_context_for` retrieval verb),
  D1 (search + indexing), D2 (graph edges + traversal), and G2/G3 (authoritativeness +
  provenance/use-policy substrate fields). All four moved from Proposed §V0.1 into a new
  Designed §V0.1 subsection (the tier's first tenant), each with a pointer-stub retained in
  Proposed §V0.1 and the corresponding Idea-section pointers (D1, D2, D3, G2/G3)
  retargeted from "See Proposed §V0.1" to "See Designed §V0.1."
- **2026-07-02**: Retrieval-cluster riders (labeled deltas; authored with the
  retrieval-cluster Stage-3 spec, riding that cluster's docs change). **RC-1
  model-hosting amendment** (retrieval-cluster intake, locked 2026-07-02): every
  falsified canonical copy reconciled in the same change (single-source discipline).
  MODIFIED the Non-goals model clause ("never hosts one" → MUST NOT host a *generative*
  LLM; local non-generative inference (embedding, reranking) permitted host-side);
  MODIFIED the Hard-constraints model-hosting clause to match; MODIFIED the `0.1.0`
  substrate entry's external-embedding framing (embeddings/reranking now local
  non-generative; the LLM-API-key surface serves the generative call-outs). The
  architecture-overview ELT external-embedding framing is the named lagging copy,
  reconciled in the same change. **Register:** `idea → proposed` promotions performed by
  the retrieval-cluster intake lock for D1 (search + indexing), D2 (graph edges +
  traversal, noting the `0.8.0` edge-table substrate as what its traversal activates),
  and G2/G3 (authoritativeness + provenance/use-policy substrate fields), all added as live
  Proposed §V0.1 entries with idea-tier pointers retained; `1.1.0` re-confirmed as the
  cluster's headline entry (tier stays `proposed` until the spec locks). ADDED idea-tier
  entry: per-user identity machinery (frame pre-gate walk item 9). **Hard constraints:**
  ADDED accessibility as a standing product requirement binding on human-facing UI/docs
  surfaces (frame pre-gate walk item 13: routed here, not faked into the MCP-verb cluster).
- **2026-05-20**: Intake-exit gate confirmed (Stage 1 lock). All open decisions resolved:
  LIC-1 (Apache-2.0 + future-relicense locked; mnemra-core repo LICENSE/README correction
  is a follow-up task); T-5 split (categorization-via-LLM-API → `idea`; LLM-API-key config
  surface → V0 substrate, folded into `0.1.0`); T-7 (team == workspace/tenant; 1-layer
  deferred hierarchy); OD-B (permissions-model research queued for separate dispatch,
  non-blocking); PRV-1 (decision-name + lock-date provenance form confirmed); AMEND-1
  (project-context-file drift routed to retrospective); DEFER-1 (brief-home trip-wire
  wording confirmed); µVM-OQ1..4 (all confirmed parked until streamable-HTTP active);
  APPARATUS-1 (confirmed tracked separately, no mid-brief absorption); alignment-doc
  framing flag confirmed as a separate downstream amendment candidate. Brief relocated
  from `mnemra.dev/docs/intent/mnemra.md` to `mnemra-core/docs/src/intent/mnemra-core.md`.
  The brief travels with the component it scopes (per-repo-first); DEFER-1 resolves to
  relocation rather than parking. Hard constraints updated for the license lock; `0.1.0`
  substrate updated for the LLM-API-key config surface. Two suppressed-r2 tweaks applied
  (provenance line "scheduled at" instead of "committed to"; changelog vocabulary tightened).
  Intake-exit gate (Stage 1d, human touchpoint 1) is closed; brief enters lock state. Stage 2
  (Frame) is the next pipeline stage when re-engaged.
- **2026-05-20**: Intake review pass reconciled (external review-mode pass, six findings,
  all resolved). Brief retitled "Mnemra Core", a focus correction: this brief is mnemra-core's
  product intent and roadmap; sibling components (dispatch CLI, spec-delta/merge tool,
  markdown review/annotation tool) are external with their own forthcoming briefs and
  their own independent versions. No-meta-version decision recorded for mnemra-as-a-whole.
  F1 resolved: V0/V0.1 made explicit as marketing-tier labels with `1.0.0`/`1.1.0`+ SemVer
  corollaries; D3 (`get_context_for`) promoted from `idea` to `proposed` @ `1.1.0`; new
  Proposed §V0.1 (post-`1.0.0` immediate roadmap) added with `1.1.0` retrieval verb and
  `1.2.0` ongoing-ingest entries. F2: retro defined at intent level (the "review-after"
  capture in a trust-then-review workflow). F3a: `0.6.0` renamed to "Collaboration session
  friction tracking", which disambiguates the operator-with-team layer from `0.1.0`'s
  per-MCP-connection agent session (MCP-protocol-defined); intent-level definitions added
  for event-type 3-tuple and dimension 5-tuple. F3b: "AC" expanded to "Acceptance Criteria
  (AC)" on first use. F4: `0.4.0` "skill" defined at intent level + clarified that
  skill-run measurement does not depend on D8 migration. F5: pre-resolution `V0.01` label
  retired (Live tier entry renamed to "pre-`0.1.0` substrate spike"). F6 / OD-A resolved:
  ongoing-ingest pipeline distinct from `0.14.0`, placed at V0.1 / `1.2.0`.
  dispatch CLI + spec-delta/merge tool Idea entries reframed: external components (operationally
  required before mnemra-core V0 build, but live in their own repositories / briefs / versions),
  not items in this brief's register. Build prerequisites paragraph updated accordingly.
- **2026-05-18**: V0 remodeled from a single monolithic `proposed` entry into a
  builtin-substrate-first, one-capability-per-increment staged sequence (`0.1.0` host core →
  `0.14.0`, then `1.0.0` cutover), each increment its own `proposed` entry. Versioning
  resolved to Semantic Versioning 2.0.0 applied without abuse (feature = minor within
  `0.y.z`; fix = patch; commit = `+build` metadata, not `-pre-release`; `1.0.0` = public
  API defined = the dogfood-cutover/MVP, SemVer §5). Substrate boundary corrected against
  the architecture-alignment record: workspace/users/auth/session/perms/projects/agents are
  builtin (projects/agents are not plugins, a per-project plugin chicken-and-egg). Resolves
  INCR-1. Empty `designed`/`committed` tiers preserved. Canonical-model residue folded into
  APPARATUS-1; the stale alignment-record core-plugin framing flagged in the
  maintainer-internal intake record for separate amendment.
- **2026-05-18**: Re-tiered to the v4 five-tier model (`idea → proposed → designed →
  committed → live`, pipeline-artifact validators, permanent/ephemeral boundary) across
  six refine rounds with the decomposer. This brief is the forcing instance for the
  canonical register-model amendment (APPARATUS-1). Honest state recorded: `committed` and
  `designed` are empty (no locked spec for any feature; no committed release date). The
  register declining to over-claim is the mirror of the under-capture gap it remediates.
  Elicited in-head intent folded in (the prior-tooling capability families, MCP-server-as-
  V0-deliverable, deliberate single-use-case focus, tenant-hierarchy deferral + invariant,
  sibling-tool-as-plugin entries, microVM hosting posture). Scope: whole-product intent +
  roadmap; commercial and go-to-market strategy held in a separate internal record by
  deliberate seam.
- **2026-05-18**: Initial draft. Stage 1 (Intake) of a product-altitude structured
  product-intent authoring pass. Home and scope set by the decomposer across the intake
  conversation. (Superseded by the re-tier above; retained per decision-space preservation.)
