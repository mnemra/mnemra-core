---
title: "Product Brief: Mnemra Core"
summary: "Product brief locking mnemra-core's intent and feature register."
primary-audience: agent
---

# Product Brief: Mnemra Core

**Date:** 2026-05-20 · **Status:** locked (intake-exit gate confirmed) · **Altitude:** product

> Format note: this is a living document. Its structure is a forward-contract with the
> structured-delta tooling that will own its evolution (add idea · promote tier · retire
> feature · adjust scope, applied as labeled `ADDED / MODIFIED / REMOVED` deltas). Do not
> restructure ad hoc. Layer 1 (product-level intent) is stable and changes rarely; Layer 2
> (feature register) grows continuously. A new thought defaults to a Layer-2 register entry
> at the `idea` tier; it escalates to a Layer-1 revision only if it shifts the product's
> fundamental job-to-be-done or scope.

> Register-model note: the feature register uses a five-tier lifecycle
> (`idea → proposed → designed → committed → live`) whose tiers are validated by
> **pipeline artifacts**, not prose judgement. This brief is the forcing instance for a
> pending amendment to the canonical register model (the prior model was four-tier and
> ordered `committed` before `designed`); the amendment is tracked separately and is the
> reason the structure here leads its canonical adoption.

> Scope boundary: this brief is **mnemra-core's product intent and capability roadmap**.
> Sibling components in the mnemra umbrella (a dispatch CLI, a spec-delta/merge tool, a
> markdown review/annotation tool) live in their own repositories with their own forthcoming
> briefs and their own independent versions; they are referenced as external components,
> not absorbed into this brief's register.
>
> **Mnemra-as-a-whole does not carry a unified version.** Components version independently.
> A release-manifest concept may pin specific component versions for a coordinated public
> release; that is distinct from a version.
>
> Commercial validation thresholds, pricing, and go-to-market strategy are maintained as a
> separate internal commercial record and are deliberately **not** inlined here. Where a
> roadmap entry has a commercial dimension, only its product shape is recorded; its
> commercial gating is referenced, not described.
>
> Brief-home: this artifact lives in the mnemra-core repository at
> `docs/src/intent/mnemra-core.md` (relocated 2026-05-20). The brief travels with the component
> it scopes (per-repo-first). The general multi-repo product-brief-home convention question
> remains in APPARATUS-1's scope for future multi-repo briefs in other components.

## Product-level intent  (layer 1 — stable)

### JTBD

Engineering teams that run coding agents (Claude Code, Cursor, Copilot) need their agents —
and the humans working alongside them — to have **persistent, structured, queryable
context** of the team's codebase, decisions, tickets, docs, and prior agent sessions,
available every session without re-explanation, so that context stops being a per-session
tax that drifts and does not scale.

Stated as the need, not the solution: an agent preparing to act on a task sits inside a
graph — parent spec, related decisions, sibling tasks, prior reviews, recent adjacent
commits — and today that graph must be hand-loaded per session by the orchestrating agent,
which drifts and does not scale. Mnemra's job is to make that context a durable,
agent-addressable substrate.

### Non-goals

Each is a concrete not-this:

- Not a retrieval-augmented-generation (RAG) chatbot, and not "a second brain for small
  businesses."
- Not a Notion replacement, knowledge wiki, or general-purpose vector-database wrapper.
- Not a horizontal "AI for every department" tool; not sales/marketing/HR enablement.
- Not an attempt to match an enterprise knowledge-search incumbent's breadth.
- Does not host a **generative** language model. All generative work (query rewrite,
  chunk-context, tag generation, synthesis) calls out to an external model at V0; local
  **non-generative** inference (embedding, reranking — small encoder models behind the
  host-fn seam) is permitted host-side. *(MODIFIED 2026-07-02 per RC-1, retrieval-cluster
  intake (locked 2026-07-02); was: "Does not run a language model. Embeddings and
  summaries call out to an external model; the system never hosts one.")*
- Not RAG-as-a-service.
- The open-source core does not pursue multi-tenant isolation as a product goal; tenancy is
  a structural column-shape at V0 with policy enforcement deferred. The boundary between
  OSS-core single-tenant and a future managed multi-tenant offering is commercial — out of
  scope for this brief.
- **Not a general autonomous-agent framework.** The product *could* generalize toward one
  with work; single-use-case focus (a context layer for coding agents) is a deliberate
  quality choice. The generalization is declined, not absent — recorded so the rejected
  option is preserved.

### Success criteria

Each is an observable outcome a downstream check could verify. **V0 and V0.1 are
marketing-tier labels** denoting product-promise milestones, distinct from SemVer release
identifiers:

| Marketing tier | SemVer corollary | What it delivers |
|---|---|---|
| V0 | `1.0.0` (dogfood-cutover / MVP) | the maintainer's workspace surface, replicated on mnemra-core without regression |
| V0.1 | `1.1.0`+ (first post-MVP minor sequence) | the core product promise activates — net-new value-add beyond workspace fidelity |

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
  record; they are not product success criteria and are not inlined here.

### Hard constraints

Locked technical and integration boundaries (RFC-2119 keywords where observable):

- The agent-facing surface SHALL be **MCP-native** (MCP specification 2025-06-18).
  Transport is stdio at V0; streamable-HTTP is a later-version activation.
- **An MCP server is a V0 deliverable** — intent-clarity: the MCP-native constraint is
  satisfied by a running MCP server in V0 scope, not merely a future protocol posture.
- The substrate SHALL be a **single-process Postgres** instance with the `pgvector`
  extension present. TimescaleDB is demoted off the V0 stack (P-0010 D8) — it is absent by
  decision, not oversight, because at V0 only content and state are persisted in-app
  Postgres shapes; the former timeseries and log shapes are observability emission surfaces,
  not in-app storage (telemetry is emitted, not stored — per the architecture-overview
  observability baseline), so the time-series engine has no V0 store to back. TimescaleDB is
  held behind a named latency/storage trip-wire for a later version.
- Plugins SHALL be **WebAssembly Component Model modules** hosted in-process via Wasmtime;
  plugin core logic MUST be IO-free; all plugin IO MUST be mediated by host-provided
  functions. Plugins are leaves — no direct sideways linkage; cross-plugin calls are
  host-mediated.
- Deployment posture SHALL be **self-hosted-first, single-binary**. The system MUST NOT
  host a **generative** LLM — all generative work calls out to an external model at V0;
  local **non-generative** inference (embedding, reranking) is permitted host-side.
  *(MODIFIED 2026-07-02 per RC-1, retrieval-cluster intake (locked 2026-07-02); was: "The
  system MUST NOT host a language model; it calls out to an external one.")*
- **"Single-binary" constrains the server, not the deployment packaging.** It means one
  process (not a microservice mesh) — an immutable image/appliance is a valid packaging
  shape for that single binary and does not violate this constraint.
- **Tenancy invariant:** the tenant scoping key (`workspace_id`) is structural from V0 —
  NOT NULL, indexed, explicitly passed, forward-compatible without migration. This is what
  makes deferring tenant-hierarchy/policy enforcement safe: the scoping key ships now;
  hierarchy and enforcement build on top later without a substrate migration.
- Tooling SHALL default to Rust; non-Rust paths are adopted only where no viable in-stack
  path exists (the landing site is an accepted exception).
- License: **Apache-2.0 with a future-relicense clause** (locked 2026-05-20). The
  mnemra-core repository's current LICENSE/README (MIT) is corrected in a separate
  follow-up task; this brief's Hard constraints lock the direction.
- Architecture MUST NOT be schedule-pressured. Dates appearing in marketing or landing
  material are not architectural inputs and do not weight tradeoff analysis.
- **Accessibility is a standing product requirement**, binding on every human-facing UI
  and documentation surface the product ships (current and future: the docs site, any
  dashboard/console, human-readable CLI output); each such surface's design gate reviews
  it. Machine-facing MCP/agent surfaces are outside this requirement's scope; derivative
  human views inherit it. *(ADDED 2026-07-02 — retrieval-cluster frame pre-gate walk item
  13 (locked 2026-07-02): routed here as a product-level standing requirement rather than
  faked into an MCP-verb feature cluster.)*

### Evidence

This brief exists because product intent that lived only in conversation was not an
agent-addressable source. Across multiple research-lifecycle reviews, research and
discovery work silently anchored scope to *mnemra-core-as-exists-live* — the only
available ground truth — because no durable product-intent/roadmap artifact existed. The
gap recurred at least three separate times before being remediated. This document is that
remediation: the agent-addressable product-intent source against which future research,
discovery, and architecture evaluate scope, so "the intended product" is a readable
artifact rather than an inferred or imagined one.

Corroborating anchors: a locked V0 architecture discovery and a locked V0
architecture-constraints record (both high-stakes, reviewed to a zero-new-finding stopping
rule); a structural architecture overview (eight named subsystems); and an internal
commercial hypothesis (a set of testable claims, maintained separately).

### Consumer

Primary consumer is **agents**: MCP-client coding agents and orchestration tooling that
load this as the agent-addressable product-intent source during research, discovery, and
architecture work — consistent with the project's agent-primary source-artifact stance.
Secondary consumer is the maintainer and future contributors evaluating scope.
Human-readable rendered views, if needed, are derivative and generated on demand; this
source is never the rendered view.

### Risk profile

This artifact is documentation; it touches no trust boundary itself. The *product* it
describes carries trust boundaries (multi-tenancy, authentication, plugin sandbox,
telemetry non-leak) — these are owned by the mnemra-core component architecture record
(threat-modeling trigger already met there) and are referenced by the register, not
re-assessed here. Required risk assessment for any *implementing* work is deferred to the
component-level frame where the mechanism is known.

## Feature register  (layer 2 — grows; each entry has a lifecycle tier)

Each entry carries exactly one tier. Tiers are validated by **pipeline artifacts** — the
validator for each tier is "does this artifact exist?", which makes the register
mechanically checkable and self-consistent with the intent → frame → spec pipeline that
produces it:

| Tier | Mechanical validator | Durability |
|---|---|---|
| `idea` | a captured thought; optionally a provenance pointer to a locked decision | — |
| `proposed` | a **locked intake** — the feature has been through intent capture | permanent |
| `designed` | a **locked frame + locked spec** — the permanent "what to build" is complete | permanent (kept) |
| `committed` | `designed` **plus a plan** (the task list to action the build); release-bound | plan is ephemeral (not kept) |
| `live` | built and verified in current code/canon | — |

The `designed`|`committed` boundary is the **permanent/ephemeral artifact line**: every
permanent design artifact done = `designed`; add the disposable actioning plan = `committed`.
The plan's ephemerality is *why* it marks commitment — a throwaway task list is only
generated once the work is being actioned against a release. `designed` precedes
`committed` because release-fit cannot be judged until the design (culminating in the spec)
is complete: the work generates the commitment, not the reverse.

**Structural fence (unchanged):** no tier is a build authorization — not even `committed`.
A tier is a readiness/commitment signal; the only build trigger is an explicit
feature-altitude pass on the entry. The tiers track how far a feature's pipeline has
progressed; they do not widen the build trigger.

> Provenance pointers reference decisions by name and lock-date. For a multi-repo project
> much of the locked provenance lives in a maintainer-internal architecture record a public
> repository artifact cannot cite by path; pointers name the decision and lock-date rather
> than an internal path (see Open Decision PRV-1).

### Idea

Captured directions — unvalidated, or decision-locked but not yet through their own
pipeline. A provenance pointer (where one exists) records that a decision was taken; it
does **not** promote the tier — only a dedicated intake does. **This tier is the
scope-anchor surface: research and discovery read `idea`-and-up so intended direction is
never silently dropped to what-exists-live.**

- **Search + indexing activation** (full-text + vector) [D1] — **promoted to `proposed` @ V0.1, retrieval cluster** (2026-07-02). See Designed §V0.1 for the live entry; this pointer is retained so the D1 reference stays resolvable. Provenance: V0 discovery, Deferred section (locked 2026-05-02).
- **First-class graph edges + traversal** [D2] — **promoted to `proposed` @ V0.1, retrieval cluster** (2026-07-02). See Designed §V0.1 for the live entry; pointer retained so the D2 reference stays resolvable. Provenance: as D1.
- **`get_context_for(artifact_id)` — the agent-context bundle composer** [D3] — **promoted to `proposed` @ V0.1 / `1.1.0`** (2026-05-20). See Designed §V0.1 for the live entry; this pointer is retained so the D3 reference stays resolvable.
- **Bidirectional issue↔code via commit-ref convention** [D4] — provenance: as D1.
- **New first-class artifact types beyond the migrated set** [D5] — provenance: as D1.
- **Row-level-security policy enforcement** [D6] — provenance: as D1.
- **External-authorization-server integration** [D7] — provenance: as D1.
- **Internal-workspace absorption** (orchestration skills/roles/memory/inboxes migrate or become projections) [D8] — provenance: as D1.
- **Multi-orchestrator-per-project topology** [D9] — provenance: as D1.
- **Third-party plugin install** [D11], gated on a documented ABI evolution policy — provenance: as D1.
- **Host-fn ABI 1.0 stabilization** [D12] — provenance: as D1.
- **Cross-artifact authoritativeness + provenance/use-policy substrate fields** (G2/G3) — **promoted to `proposed` @ V0.1, retrieval cluster** (2026-07-02). See Designed §V0.1 for the live entry; pointer retained so the G2/G3 reference stays resolvable. Provenance: knowledge-object survey dogfooding-lens amendment (2026-05-15); reclassified to a substrate concern there — the retrieval cluster is the pipeline run it rides.
- **Knowledge-object substrate shape** (frontmatter-shape not new artifact-type; OCC version field; extensible typed audit events) — provenance: as G2/G3.
- **Knowledge-object family schemas + judge-extender + review-queue + memory-inspector + memory-write-back discipline** (F1/F3/F4/F5/F7) — provenance: as G2/G3 (forward-context, no consumer yet).
- **Multi-language plugin authoring** (Rust-first now; JS/TS, Python, TinyGo; later C#) — provenance: architecture overview V0/V1 boundary.
- **microVM appliance posture for self-host** — conditional; trip-wire = streamable-HTTP becomes the active transport (a named-capability condition, not release-gated); substrate shortlist recorded in the hosting research. Provenance: microVM hosting research (2026-05-18).
- **Per-tenant microVM isolation (managed tier)** — conditional on a multi-tenant managed audience existing; Firecracker the substrate; pairs with row-level security, not instead of it. Provenance: as above (Lens B).
- **Tenant hierarchy** (org/+ layers above the workspace=tenant boundary) for full multi-tenancy — deferred; safe to defer because the tenant scoping key is structural from V0 (see Tenancy invariant). Provenance: knowledge-object-survey scope sketch (`visibility: …|workspace|org`).
- **A dispatch CLI (external component, separate brief forthcoming).** A sibling component in the mnemra umbrella with its own repository, its own brief, and its own independent version. Operationally required before mnemra-core V0 build begins (mechanical tasks heavily consume the premium model tier; this CLI routes and optimizes them). Built *using* the completed intent → frame → spec pipeline; later runs as a mnemra plugin (a workspace-era CLI and a mnemra-era WASM plugin sharing an IO-free core). Provenance: maintainer sequencing decision 2026-05-18. Referenced here as a build-time dependency, not absorbed into this brief's register.
- **A spec-delta/merge tool (external component, separate brief forthcoming).** A sibling component with its own repository, brief, and version. Operationally required before mnemra-core V0 build begins: the structured-delta consumer this brief's format forward-contracts with, needed for living-document updates. Built *using* the completed intent → frame → spec pipeline; later runs as a mnemra plugin. Provenance: as above. Referenced as a build-time dependency, not absorbed.
- **A markdown review/annotation tool, hosted under the mnemra umbrella when published** — a sibling product; tentative. Provenance: maintainer note 2026-05-18.
- **Context-intelligence plugin** — project-aware code understanding for reviewers (decisions + language-server composite, sidecar). Conditional on such a plugin surface existing. Provenance: an external algorithms-research review.
- **Byte-level provenance-tracing reference** — a provenance-deficit (not correctness) hallucination check; reimplementation-reference for a future verification plugin; keys on the G2/G3 provenance direction; reimplement-not-port. Provenance: an external algorithms-research review (reimplementation-feasibility follow-up pending).
- **Managed/Cloud tier and Enterprise tier** — product expansion beyond OSS core; commercial gating in a separate internal record, not here.
- **Connectors/ingestion beyond the dev-adjacent wedge** — opinionated, demand-driven; which and when unvalidated.
- **External context/memory vocabulary adoption** — shape convergence evidenced; label lock deferred until a second consumer joins.
- **Dashboard interface** — a maintainer-side console product surface (a review/annotation tool may host here).
- **End-user CLI/TUI** distinct from the admin control CLI; **plugin registry/marketplace + signing/distribution as a product surface**; **hosted web console**; **onboarding/docs-as-product**; **agent-framework-specific integrations** beyond generic MCP — all valid under the product umbrella, unvalidated.
- **A newsfeed capability** (working name) — pure thought, no provenance.
- **A permissions model** — pure thought; explicitly needs research into the approach (see Open Decision OD-B).
- **Per-user identity machinery** — owner-columns-as-identity, caller identity in the request context, real owner==caller checks. Activates per-user `visibility: owner-only` serving semantics (at V0 owner-only serves no one — the fail-closed enforcement is the V0 semantics) and is the first write/label-capability surface that fires the deferred write-side policy-dimension design. The retrieval cluster's new tables carry `owner`/`created_by` columns from day one so this lands as a feature, not an excavation. Provenance: retrieval-cluster frame pre-gate walk item 9 (locked 2026-07-02). *(ADDED 2026-07-02.)*

### Proposed

Has a locked intake (through intent capture); not yet frame+spec complete; not release-bound.

**V0 is delivered as a staged increment sequence, not a monolithic release**, and the
register extends beyond V0 into the V0.1 (post-`1.0.0`) immediate roadmap. "V0" denotes
the dogfood-replacement milestone, "V0.1" the very-next-update phase (see Success criteria
for the marketing-tier vs SemVer mapping); each increment below is its own register entry
carrying its own tier. The intake is by retrofit: a locked, high-stakes, reviewed-to-zero-
findings V0 discovery (locked 2026-05-02) scopes the V0 contents; the V0.1 entries lock at
this brief's altitude per maintainer ruling 2026-05-20. The increment decomposition was
captured in the 2026-05-18 product-intake refine and extended 2026-05-20. Every increment
is therefore `proposed` — none is `designed` (no per-increment frame+spec exists) nor
`committed` (no plan, no release-bound date); the honest empty `designed`/`committed`
tiers below are preserved, not papered.

**Versioning scheme (resolves INCR-1) — Semantic Versioning 2.0.0, applied without abuse:**
- Each increment delivers backward-compatible **functionality**, so each is a **minor**
  bump within initial development: `0.1.0` (host core) → `0.2.0` → … (SemVer §7; §4 —
  `0.y.z` is initial development, the API is not yet stable, which is exactly the V0 build
  period).
- Backward-compatible fixes within an increment are **patch** bumps (`0.N.1`, `0.N.2`; §6).
- The commit identifier is **build metadata** appended with `+` (`0.2.0+a1b2c3d`) — ignored
  in precedence (§10, §11). It is **not** a `-` pre-release identifier; pre-release has
  *lower* precedence than the release (§11.3, `1.0.0-alpha < 1.0.0`), which would
  misrepresent a delivered increment. Trunk commits advance the `+sha`; the version number
  bumps only when an increment's functionality is realized.
- **`1.0.0` is the dogfood-cutover / MVP.** SemVer §5: `1.0.0` defines the public API —
  precisely the V0 success criterion (the agent-facing surface and storage-substrate
  contracts are locked and the maintainer's workspace runs fully on mnemra-core). The road
  to `1.0.0` is the `0.y.z` increment sequence below.

The sequence is **builtin-substrate-first, then one capability family per increment**,
ordered by dogfood value and dependency with the maintainer's stated priority (tasks first
after substrate). A one-clause ordering rationale accompanies each entry so the sequence
can be reordered cheaply at the intake-exit gate without restructuring entries.

- **`0.1.0` — Builtin substrate + host core.** Single-process Postgres (pgvector); the
  content and state storage-shape partitions persisted in-app, with the former timeseries
  and log shapes emitted to the observability minimum rather than stored (P-0010 D8); the pre-1.0
  host-fn ABI; an MCP server skeleton (stdio) onto which each capability increment adds its
  verbs; the admin/destructive control CLI; an observability minimum; an **LLM-API-key
  configuration surface** (mnemra-core calls out to an external model for *generative*
  work; embeddings and reranking run host-side on local non-generative encoder models per
  RC-1; the key is configured per deployment, never hard-coded, and never used to host a
  model) *(MODIFIED 2026-07-02 per RC-1; was: "calls out to an external model for
  embeddings per the architecture-overview ELT subsystem")*; **and the builtin tenancy/identity core —
  workspace (tenant boundary; solo collapses to `default`), users, agents (tied to
  user–workspace pairs), authentication (a workspace claim in every token; per-deployment
  OIDC via RFC 9728; a static dev-token first-run bootstrap), agent sessions, per-plugin
  permissions, projects.** Projects and agents are *builtin*, not
  plugins: plugins are scoped per project, so a project cannot itself be a plugin (a host
  bootstrap chicken-and-egg). *Order: nothing runs without the spine, and every capability
  family is scoped within workspace + project.* Tier: `proposed`. Provenance: V0 discovery
  (locked 2026-05-02) + architecture-alignment record (2026-04-27, tenant/substrate
  rounds); decomposition — 2026-05-18 product-intake refine.
- **`0.2.0` — Task management.** Task CRUD, subtasks/parent-id, status lifecycle, project +
  priority; migration of the prior structured task data. *Order: maintainer-stated first
  priority after substrate; the operational spine the workspace dogfoods earliest; its
  project/agent references are satisfied by the builtin substrate.* Tier: `proposed`.
  Provenance: as `0.1.0`.
- **`0.3.0` — Dispatch metrics & lifecycle.** Dispatch start/event/record/finalize;
  per-dispatch tokens/duration/cost/tool-uses; the dispatch-event stream; the scope
  envelope. *Order: maintainer-stated next priority; core to the orchestration workflow.*
  Tier: `proposed`. Provenance: as `0.1.0`.
- **`0.4.0` — Skill-run measurement.** Tracks runs of a *skill* (a named, reusable agent
  capability — e.g., a specific dispatch shape, a structured review protocol, an
  elicitation loop). Each run is measured: start/end timestamps, per-run
  consultations / review-rounds / flags tallies, knowledge-extraction capture, and a
  structured *retro* — the formal "review-after" capture in a trust-then-review workflow,
  where after a run the operator selectively reviews what the agent decided, flags
  divergences, and records what was learned; the structure makes findings aggregable
  across runs. Skill-run measurement operates at the substrate level and does not depend
  on a separate decision to migrate workspace skill definitions into mnemra-core (D8) —
  the measurement substrate works whether skill definitions live in mnemra-core or remain
  as external files. *Order: sibling of dispatch metrics — the same measurement family.*
  Tier: `proposed`. Provenance: as `0.1.0`.
- **`0.5.0` — Activity / audit log.** The append-only actor/action/target/summary stream.
  *Order: low-complexity, high-leverage; underpins traceability across every later
  capability.* Tier: `proposed`. Provenance: as `0.1.0`.
- **`0.6.0` — Collaboration session friction tracking.** A *collaboration session* is the
  operator-with-team conversation container (distinct from `0.1.0`'s per-MCP-connection
  agent session — that is the technical auth/state primitive, MCP-protocol-defined). One
  collaboration session may span many per-MCP-connection agent sessions as the orchestrator
  dispatches to sibling agents. Friction events surface within a collaboration session
  along two axes:
  - **Event type (the friction *kind*):** `clarification` (operator needed to ask before
    acting), `revision` (operator changed something after delivery), `course-correction`
    (operator redirected the approach mid-task).
  - **Dimension (the friction *axis*):** `scope`, `Acceptance Criteria (AC)`, `context`,
    `routing`, `priority`.
  Each event row records the (event-type × dimension) tuple plus context. Aggregated over
  time the rows surface friction patterns per collaboration session and across sessions —
  the measurement substrate for trust-then-review iteration. *Order: completes the
  measurement/audit triad with dispatch + skill-run + activity.* Tier: `proposed`.
  Provenance: as `0.1.0`.
- **`0.7.0` — Repo registry.** Repos per project (path / visibility / default-branch /
  remote). *Order: rides on builtin projects; pairs with the structural families.* Tier:
  `proposed`. Provenance: as `0.1.0`.
- **`0.8.0` — Relationships / edges.** Typed edges
  (parent / blocks / depends-on / supersedes / dispatched-by). *Order: the cross-cutting
  graph; valuable once tasks and projects exist to link.* Tier: `proposed`. Provenance: as
  `0.1.0`.
- **`0.9.0` — Tags / taggings.** Cross-cutting taxonomy. *Order: light; rides on the
  entities above.* Tier: `proposed`. Provenance: as `0.1.0`.
- **`0.10.0` — Dependency-approval state.** The approved-package register (the
  green/yellow/red license-tiering state). *Order: self-contained governance state.* Tier:
  `proposed`. Provenance: as `0.1.0`.
- **`0.11.0` — Scope-violation log.** The append-only scope-denial stream. *Order:
  self-contained, low-complexity.* Tier: `proposed`. Provenance: as `0.1.0`.
- **`0.12.0` — Job-search pipeline.** Applications / listings / search-runs; stale
  auto-reject. *Order: a distinct domain, lower coupling to the orchestration core.* Tier:
  `proposed`. Provenance: as `0.1.0`.
- **`0.13.0` — Contacts.** *Order: smallest and most isolated of the capability families.*
  Tier: `proposed`. Provenance: as `0.1.0`.
- **`0.14.0` — Content-corpus migration.** The prior markdown content corpus (the
  maintained knowledge subdirectories) → stored as files with frontmatter metadata, limited
  indexing (no full-text/vector — that is `idea` D1). *Order: placed after the
  structured-capability families per the maintainer's tasks/metrics-first priority; flagged
  as a reorder candidate at the intake-exit gate — for a context-layer product the corpus
  is arguably core-value-early.* Tier: `proposed`. Provenance: V0 discovery §Migration
  scope (locked 2026-05-02); decomposition — 2026-05-18 refine.
- **`1.0.0` — Dogfood cutover (public API defined).** The maintainer's workspace runs fully
  on mnemra-core with zero fallback to the prior tooling and the agent-facing +
  storage-substrate contracts are locked. SemVer §5: this is where the public API is
  defined — the V0/MVP milestone. *Order: last in V0 by definition — it is the milestone
  gate.* Tier: `proposed`. Provenance: as `0.1.0`.

#### V0.1 (post-`1.0.0` immediate roadmap)

The very-next-update phase after MVP cutover — net-new value beyond V0 workspace-fidelity.
Each V0.1 entry starts `proposed` at this product altitude (this brief's intake locks the
phase placement); each entry's own feature-altitude intake locks its frame+spec before
build, promoting it to `designed` (see Designed §V0.1 for the entries that have — the
retrieval cluster's four plus the extensible reporting engine). Maintainer ruling
2026-05-20: V0 = workspace-replacement (no
regression); V0.1 = the core product promise activates plus operational follow-ups V0
deliberately did not promise.

- **`1.1.0` — `get_context_for(artifact_id)` retrieval verb (core product promise)** —
  **promoted to `designed` @ V0.1 / `1.1.0`** (2026-07-02). See Designed §V0.1 for the
  live entry; pointer retained so the `1.1.0` reference stays resolvable. Provenance:
  retrieval-cluster spec (`docs/specs/2026-07-02-retrieval-cluster.md`, locked
  2026-07-02).
- **Search + indexing activation [D1] (retrieval cluster)** — **promoted to `designed`
  @ V0.1, retrieval cluster** (2026-07-02). See Designed §V0.1 for the live entry;
  pointer retained so the D1 reference stays resolvable. Provenance: retrieval-cluster
  spec (`docs/specs/2026-07-02-retrieval-cluster.md`, locked 2026-07-02).
- **First-class graph edges + traversal [D2] (retrieval cluster)** — **promoted to
  `designed` @ V0.1, retrieval cluster** (2026-07-02). See Designed §V0.1 for the live
  entry; pointer retained so the D2 reference stays resolvable. Provenance:
  retrieval-cluster spec (`docs/specs/2026-07-02-retrieval-cluster.md`, locked
  2026-07-02).
- **Cross-artifact authoritativeness + provenance/use-policy substrate fields [G2/G3]
  (retrieval cluster)** — **promoted to `designed` @ V0.1, retrieval cluster**
  (2026-07-02). See Designed §V0.1 for the live entry; pointer retained so the G2/G3
  reference stays resolvable. Provenance: retrieval-cluster spec
  (`docs/specs/2026-07-02-retrieval-cluster.md`, locked 2026-07-02).
- **`1.2.0` — Ongoing ingest pipeline.** Watchers, scheduled polls, or webhooks that
  auto-detect and ingest new content arriving in the brain corpus after V0's one-shot batch
  migration (`0.14.0`). Distinct from `0.14.0`: `0.14.0` is a one-shot move-existing-corpus;
  `1.2.0` is a continuous-arrival pipeline. *Order: operational follow-up to V0.1's
  headline; V0 covers batch only, V0.1 adds continuous ingest.* Tier: `proposed`.
  Provenance: architecture-overview ELT subsystem (ADR-16) + product-intake refine 2026-05-20
  (OD-A resolved: distinct from `0.14.0`, deferred to V0.1).

- **`1.3.0`+ (candidate) — Extensible reporting engine** — **promoted to `designed`
  @ V0.1 / `1.3.0`+ (candidate)** (2026-07-04). See Designed §V0.1 for the live entry;
  pointer retained so the `1.3.0`+ reporting-engine reference stays resolvable.
  Provenance: reporting-engine spec (`docs/specs/2026-07-03-reporting-engine.md`, locked
  2026-07-04; verify verdict passed_with_concerns).

Future V0.1 increments (`1.3.0`+) land here as the "very-next-update" trigger fires for
new capabilities.

**Build prerequisites (sequence, unchanged):** the V0 increment sequence's build is gated
on three external predecessors — the intent → frame → spec pipeline being complete (it is
being exercised and amended now), then the spec-delta/merge tool and the dispatch CLI being
operational. Both prerequisite tools are **external components** with their own forthcoming
briefs and their own independent versions (see Idea section pointers); this brief
references them as build-time dependencies, does not absorb them into its register.
`0.1.0` work begins only after those exist; the prerequisites gate the V0 sequence's
*start*, not its contents.

### Designed

A locked frame + locked spec exists. Two tenants: the retrieval cluster — its spec
(`docs/specs/2026-07-02-retrieval-cluster.md`) locked 2026-07-02, promoting its four
constituent entries below from `proposed` — and the extensible reporting engine — its
spec (`docs/specs/2026-07-03-reporting-engine.md`) locked 2026-07-04, promoting the
`1.3.0`+ (candidate) entry below from `proposed`. Stated explicitly: the register does not
infer design completion beyond what a locked spec actually covers.

#### V0.1 (post-`1.0.0` immediate roadmap)

- **`1.1.0` — `get_context_for(artifact_id)` retrieval verb (core product promise).** A
  one-call MCP retrieval of persistent, typed, cross-session context for a given artifact,
  rather than reconstructing it by hand each session. The headline V0.1 capability — the
  first net-new value-add over V0's workspace-fidelity baseline. *Order: V0.1's headline
  promise; first net-new value over V0.* Tier: `designed`. Provenance: V0 discovery (D3 —
  locked 2026-05-02) + product-intake refine 2026-05-20 (scheduled at V0.1 / `1.1.0`) +
  retrieval-cluster spec (`docs/specs/2026-07-02-retrieval-cluster.md`, locked
  2026-07-02). **Promoted 2026-07-02 (MODIFIED):** covered by the retrieval-cluster
  feature-altitude intake (locked 2026-07-02) as one clustered feature with D1, D2, and
  G2/G3 below — one intake/frame/spec pipeline; the frame locked 2026-07-02 and the spec
  locked 2026-07-02, satisfying the `designed` validator (a locked frame **plus** a locked
  spec).
- **Search + indexing activation [D1] (retrieval cluster).** Batch indexing pipeline over
  the corpus the substrate already holds (per-shape chunking, authored-tree summary
  nodes, local embeddings, full-text search) plus the agent-facing `search` verb — hybrid
  FTS+dense retrieval fused by Reciprocal Rank Fusion with local rerank, budget-shaped.
  Part of the retrieval cluster (one clustered feature with `1.1.0`, D2, G2/G3). *Order:
  rides with `1.1.0` — the headline verb needs the index.* Tier: `designed`. Provenance:
  V0 discovery D1 (locked 2026-05-02); retrieval-cluster intake (locked 2026-07-02) —
  the intake lock performed the `idea → proposed` promotion; retrieval-cluster spec
  (`docs/specs/2026-07-02-retrieval-cluster.md`, locked 2026-07-02) performed the
  `proposed → designed` promotion. *(ADDED 2026-07-02.)*
- **First-class graph edges + traversal [D2] (retrieval cluster).** Typed, traversable
  edges extracted from the authored-but-latent sources (frontmatter relation lists,
  free-text citations) with provenance discrimination; **the `0.8.0` edge-table substrate
  is what this traversal activates** — one superset vocabulary, one traversal path
  (recursive CTEs, per the storage-substrate decision). Part of the retrieval cluster.
  *Order: as D1.* Tier: `designed`. Provenance: V0 discovery D2 (locked 2026-05-02);
  retrieval-cluster intake (locked 2026-07-02) — the intake lock performed the
  `idea → proposed` promotion; retrieval-cluster spec
  (`docs/specs/2026-07-02-retrieval-cluster.md`, locked 2026-07-02) performed the
  `proposed → designed` promotion. *(ADDED 2026-07-02.)*
- **Cross-artifact authoritativeness + provenance/use-policy substrate fields [G2/G3]
  (retrieval cluster).** The substrate fields the retrieval envelope reads and serves:
  trust provenance (authoritative/outdated/background), the policy permissions record
  (dont-use, model-egress, visibility, tenant-share), freshness handles + decay classes,
  and decision axes. Part of the retrieval cluster. *Order: as D1 — the envelope's
  substrate.* Tier: `designed`. Provenance: knowledge-object survey (2026-05-15);
  retrieval-cluster intake (locked 2026-07-02) — the intake lock performed the
  `idea → proposed` promotion; retrieval-cluster spec
  (`docs/specs/2026-07-02-retrieval-cluster.md`, locked 2026-07-02) performed the
  `proposed → designed` promotion. *(ADDED 2026-07-02.)*
- **`1.3.0`+ (candidate) — Extensible reporting engine.** One report surface backed by a
  registry: canonical built-in reports (which ride their V0 capability-family increments
  as workspace-fidelity content) plus declarative, runtime-added, **read-only** user
  reports invoked via MCP (admin-CLI convenience secondary). Read-only execution is the
  identity invariant (defense-in-depth, re-derived for the Postgres substrate); because
  report queries are user-authored, the guard graduates from operator-mistake to
  adversary — workspace isolation, the role matrix, and the provenance/policy-envelope
  predicates must hold on a caller-written query, which is the Frame threat model's
  headline boundary. Deliberately sequenced after `1.1.0`: the policy-envelope
  enforcement machinery this surface must honor lands with the retrieval feature.
  *Order: value activates once the measurement families hold data and the envelope
  machinery exists.* Tier: `designed`. Provenance: reporting-engine intake (locked
  2026-07-03) placed the entry at `proposed`; reporting-engine spec
  (`docs/specs/2026-07-03-reporting-engine.md`, locked 2026-07-04) performed the
  `proposed → designed` promotion (verify verdict passed_with_concerns). *(ADDED 2026-07-04.)*

### Committed

`designed` plus a plan, release-bound. **Empty.** The retrieval cluster and the extensible
reporting engine are now `designed`, but no feature has yet moved `designed → committed`
(no committed-tier plan exists, and no
release has a committed date) — consistent with the product's stated posture that a phase
commits a date only when work is far enough along. Stated explicitly: the register does
not over-claim commitment. An empty `committed` tier early in a project is the register
working, not a gap.

### Live

Built and verified in current code/canon.

- **mnemra-core pre-`0.1.0` substrate spike** — a host instantiates a WebAssembly Component
  Model plugin over a typed contract, with host-fn round-trips and host-side state
  persisting across invocations on the `wasm32-wasip2` toolchain. Verifiable: the
  mnemra-core repository (host crate, first plugin crate, contract package; spike commit on
  `main`).
- **Landing site** — `mnemra.dev`, Astro on Cloudflare Pages; deployed.
- **GitHub organization** — `github.com/mnemra`, README published.
- **Email waitlist + social presence.**
- **Developer-docs scaffolding** — mdBook site with an ADR section and template.

## Open Decisions (resolve at the intake-exit gate)

Surfaced for the decomposer; not resolved in this draft — source conflicts, unknowns, and
unsettled scope are named, not papered.

- **APPARATUS-1 — confirmed (tracked separately as a register-model amendment task).** The
  canonical product-brief register model (previously four-tier, `committed` before
  `designed`) is mis-ordered and missing `proposed` for release-commitment semantics. The
  amendment: five tiers `idea → proposed → designed → committed → live`, each validated by
  a pipeline artifact, with the permanent/ephemeral boundary at `designed`|`committed`;
  plus the spec-is-permanent / plan-is-ephemeral distinction promoted to general workspace
  canon; plus the multi-repo product-brief-home gap (DEFER-1). Do now while the
  structured-delta consumer does not yet exist (zero forward-contract migration; deferring
  = a contract break later). Tracked as a separate amendment task/ADR — not a mid-run edit.
- **INCR-1 — resolved.** V0 decomposed into a builtin-substrate-first,
  one-capability-per-increment staged sequence; versioning is Semantic Versioning 2.0.0
  applied without abuse — each feature increment a **minor** bump within `0.y.z` initial
  development (`0.1.0` host core → `0.14.0`), backward-compatible fixes as patch, the commit
  pinned as `+build` metadata (not a lower-precedence `-pre-release`), and **`1.0.0`** the
  dogfood-cutover/MVP where the public API is defined (SemVer §5). Applied this round — see
  the Proposed section; the per-entry ordering rationale supports cheap reordering at this
  gate. The `{projects, agents}`-as-core-plugins question is resolved upstream (builtin
  substrate; per-project plugin chicken-and-egg) — the stale architecture-alignment-record
  framing is flagged in the maintainer-internal intake record for a separate downstream
  amendment, not corrected here. The apparatus-relevant residue (the *canonical* register
  model expressing staged/incremental delivery) folds into APPARATUS-1; no longer an open
  question for this brief.
- **LIC-1 — resolved.** Apache-2.0 + future-relicense clause locked 2026-05-20. The
  mnemra-core repository's current LICENSE/README (MIT) is corrected in a separate
  follow-up task. Stronger contributor IP grant; future-relicense clause preserves the
  future commercial-managed-tier option.
- **BIPT-1 — resolved.** Split: the committed provenance direction (G2/G3) is an
  `idea`-with-provenance entry; the byte-level provenance-tracing technique is an
  `idea` reimplementation-reference entry. No longer an open tiering question.
- **PRV-1 — resolved.** Decision-name + lock-date confirmed as the provenance-pointer
  convention (locked 2026-05-20). Pattern already in use throughout the brief; a public-
  repo artifact can cite internal-record decisions by name and date without exposing
  internal paths.
- **DEFER-1 — resolved (relocated).** Brief moved from the landing-site repository to
  the mnemra-core repository at `docs/src/intent/mnemra-core.md` (2026-05-20). The brief
  lives with the component it scopes (per-repo-first). APPARATUS-1's broader multi-repo
  product-brief-home convention question still applies for future multi-repo briefs in
  other components.
- **AMEND-1 — confirmed routed to retrospective.** Both drift items in the mnemra-core
  project context file — (a) "bare mnemra = mnemra-core" shorthand wrong at product
  altitude, (b) landing-site framework migration listed as "not started" though shipped —
  batch into a retrospective doc for corrective action (locked 2026-05-20).
- **T-5 — resolved (split).** (a) **Categorization-via-LLM-API:** `idea` tier — V0 is
  workspace-replacement (no auto-categorization today); net-new value-add candidate for
  V0.1+ when the use-case shape solidifies. (b) **LLM-API-key configuration surface:** V0,
  folded into `0.1.0` substrate description (mnemra-core calls LLM for embeddings at V0
  per the architecture-overview ELT subsystem; config surface required from substrate
  onward). Locked 2026-05-20.
- **OD-A — resolved.** Content-import / ongoing ingest pipeline is distinct from
  `0.14.0`'s one-shot batch migration. Deferred to V0.1 (placed at `1.2.0` — see Proposed
  §V0.1) per maintainer ruling 2026-05-20. V0 covers batch migration only.
- **OD-B — resolved.** Permissions model: a separate scoped research dispatch is queued
  for after intake-exit (non-blocking on this gate). The capability stays `idea` until
  research informs its shape; result feeds the future permissions-model intake. Locked
  2026-05-20.
- **T-7 — resolved.** "Team" == workspace/tenant (aligned; one self-hosted instance per
  tenant). "Team" is an informal user-grouping inside a workspace, not a distinct layer.
  Deferred hierarchy is **1-layer** (org above the workspace=tenant boundary, when
  multi-tenancy lands). Locked 2026-05-20.
- **µVM-OQ1..4 — confirmed parked.** libkrun copyleft-tier acceptance; the GPL
  process-boundary stance for an appliance; the appliance trip-wire wording; managed-tier
  Postgres shape (per-tenant VM ≠ per-tenant Postgres). All deferred-until-trip-wire
  (streamable-HTTP becomes the active MCP transport), non-blocking. Locked 2026-05-20.
- *Most earlier per-item tier-ambiguities dissolved under the v4 model:* items without
  their own pipeline run are uniformly `idea`-with-provenance, so no per-item tiering
  ruling is needed. The model removing those decisions is a model-quality signal.

## Changelog

- **2026-07-04** — Reporting-engine spec locked
  (`docs/specs/2026-07-03-reporting-engine.md`; verify verdict passed_with_concerns).
  Register: `proposed → designed` promotion for the `1.3.0`+ (candidate) extensible
  reporting engine — moved from Proposed §V0.1 into Designed §V0.1 (the tier's second
  tenant, after the retrieval cluster), with a pointer-stub retained in Proposed §V0.1.
  No Idea-section pointer is retargeted: the entry was born at `proposed` via the
  2026-07-03 intake (not promoted from a Deferred D-item), so it never had one.
  Register-promotion deferred to merge per the f0f570d direct+stubs convention (the
  design branch's base predated main's post-retrieval-cluster register state).
- **2026-07-02** — Retrieval-cluster spec locked
  (`docs/specs/2026-07-02-retrieval-cluster.md`). Register: `proposed → designed`
  promotion for the four clustered entries — `1.1.0` (`get_context_for` retrieval verb),
  D1 (search + indexing), D2 (graph edges + traversal), and G2/G3 (authoritativeness +
  provenance/use-policy substrate fields) — moved from Proposed §V0.1 into a new Designed
  §V0.1 subsection (the tier's first tenant), each with a pointer-stub retained in
  Proposed §V0.1 and the corresponding Idea-section pointers (D1, D2, D3, G2/G3)
  retargeted from "See Proposed §V0.1" to "See Designed §V0.1."
- **2026-07-02** — Retrieval-cluster riders (labeled deltas; authored with the
  retrieval-cluster Stage-3 spec, riding that cluster's docs change). **RC-1
  model-hosting amendment** (retrieval-cluster intake, locked 2026-07-02) — every
  falsified canonical copy reconciled in the same change (single-source discipline):
  MODIFIED the Non-goals model clause ("never hosts one" → MUST NOT host a *generative*
  LLM; local non-generative inference — embedding, reranking — permitted host-side);
  MODIFIED the Hard-constraints model-hosting clause to match; MODIFIED the `0.1.0`
  substrate entry's external-embedding framing (embeddings/reranking now local
  non-generative; the LLM-API-key surface serves the generative call-outs). The
  architecture-overview ELT external-embedding framing is the named lagging copy,
  reconciled in the same change. **Register:** `idea → proposed` promotions performed by
  the retrieval-cluster intake lock — D1 (search + indexing), D2 (graph edges +
  traversal, noting the `0.8.0` edge-table substrate as what its traversal activates),
  and G2/G3 (authoritativeness + provenance/use-policy substrate fields) — added as live
  Proposed §V0.1 entries with idea-tier pointers retained; `1.1.0` re-confirmed as the
  cluster's headline entry (tier stays `proposed` until the spec locks). ADDED idea-tier
  entry: per-user identity machinery (frame pre-gate walk item 9). **Hard constraints:**
  ADDED accessibility as a standing product requirement binding on human-facing UI/docs
  surfaces (frame pre-gate walk item 13 — routed here, not faked into the MCP-verb
  cluster).
- **2026-05-20** — Intake-exit gate confirmed (Stage 1 lock). All open decisions resolved:
  LIC-1 (Apache-2.0 + future-relicense locked; mnemra-core repo LICENSE/README correction
  is a follow-up task); T-5 split (categorization-via-LLM-API → `idea`; LLM-API-key config
  surface → V0 substrate, folded into `0.1.0`); T-7 (team == workspace/tenant; 1-layer
  deferred hierarchy); OD-B (permissions-model research queued for separate dispatch,
  non-blocking); PRV-1 (decision-name + lock-date provenance form confirmed); AMEND-1
  (project-context-file drift routed to retrospective); DEFER-1 (brief-home trip-wire
  wording confirmed); µVM-OQ1..4 (all confirmed parked until streamable-HTTP active);
  APPARATUS-1 (confirmed tracked separately, no mid-brief absorption); alignment-doc
  framing flag confirmed as a separate downstream amendment candidate. Brief relocated
  from `mnemra.dev/docs/intent/mnemra.md` to `mnemra-core/docs/src/intent/mnemra-core.md` —
  the brief travels with the component it scopes (per-repo-first); DEFER-1 resolves to
  relocation rather than parking. Hard constraints
  updated for the license lock; `0.1.0` substrate updated for the LLM-API-key config
  surface. Two suppressed-r2 tweaks applied (provenance line "scheduled at" instead of
  "committed to"; changelog vocabulary tightened). Intake-exit gate (Stage 1d, human
  touchpoint 1) is closed; brief enters lock state. Stage 2 (Frame) is the next pipeline
  stage when re-engaged.
- **2026-05-20** — Intake review pass reconciled (external review-mode pass, six findings,
  all resolved). Brief retitled "Mnemra Core" — focus correction: this brief is mnemra-core's
  product intent and roadmap; sibling components (dispatch CLI, spec-delta/merge tool,
  markdown review/annotation tool) are external with their own forthcoming briefs and
  their own independent versions. No-meta-version decision recorded for mnemra-as-a-whole.
  F1 resolved: V0/V0.1 made explicit as marketing-tier labels with `1.0.0`/`1.1.0`+ SemVer
  corollaries — D3 (`get_context_for`) promoted from `idea` to `proposed` @ `1.1.0`; new
  Proposed §V0.1 (post-`1.0.0` immediate roadmap) added with `1.1.0` retrieval verb and
  `1.2.0` ongoing-ingest entries. F2: retro defined at intent level (the "review-after"
  capture in a trust-then-review workflow). F3a: `0.6.0` renamed to "Collaboration session
  friction tracking" — disambiguates the operator-with-team layer from `0.1.0`'s
  per-MCP-connection agent session (MCP-protocol-defined); intent-level definitions added
  for event-type 3-tuple and dimension 5-tuple. F3b: "AC" expanded to "Acceptance Criteria
  (AC)" on first use. F4: `0.4.0` "skill" defined at intent level + clarified that
  skill-run measurement does not depend on D8 migration. F5: pre-resolution `V0.01` label
  retired (Live tier entry renamed to "pre-`0.1.0` substrate spike"). F6 / OD-A resolved:
  ongoing-ingest pipeline distinct from `0.14.0`, placed at V0.1 / `1.2.0`.
  dispatch CLI + spec-delta/merge tool Idea entries reframed: external components (operationally required
  before mnemra-core V0 build, but live in their own repositories / briefs / versions),
  not items in this brief's register. Build prerequisites paragraph updated accordingly.
- **2026-05-18** — V0 remodeled from a single monolithic `proposed` entry into a
  builtin-substrate-first, one-capability-per-increment staged sequence (`0.1.0` host core →
  `0.14.0`, then `1.0.0` cutover), each increment its own `proposed` entry. Versioning
  resolved to Semantic Versioning 2.0.0 applied without abuse (feature = minor within
  `0.y.z`; fix = patch; commit = `+build` metadata, not `-pre-release`; `1.0.0` = public
  API defined = the dogfood-cutover/MVP, SemVer §5). Substrate boundary corrected against
  the architecture-alignment record: workspace/users/auth/session/perms/projects/agents are
  builtin (projects/agents are not plugins — per-project plugin chicken-and-egg). Resolves
  INCR-1. Empty `designed`/`committed` tiers preserved. Canonical-model residue folded into
  APPARATUS-1; the stale alignment-record core-plugin framing flagged in the
  maintainer-internal intake record for separate amendment.
- **2026-05-18** — Re-tiered to the v4 five-tier model (`idea → proposed → designed →
  committed → live`, pipeline-artifact validators, permanent/ephemeral boundary) across
  six refine rounds with the decomposer. This brief is the forcing instance for the
  canonical register-model amendment (APPARATUS-1). Honest state recorded: `committed` and
  `designed` are empty (no locked spec for any feature; no committed release date) — the
  register declining to over-claim is the mirror of the under-capture gap it remediates.
  Elicited in-head intent folded in (the prior-tooling capability families, MCP-server-as-
  V0-deliverable, deliberate single-use-case focus, tenant-hierarchy deferral + invariant,
  sibling-tool-as-plugin entries, microVM hosting posture). Scope: whole-product intent +
  roadmap; commercial/GTM strategy held in a separate internal record by deliberate seam.
- **2026-05-18** — Initial draft. Stage 1 (Intake) of a product-altitude structured
  product-intent authoring pass. Home and scope set by the decomposer across the intake
  conversation. (Superseded by the re-tier above; retained per decision-space preservation.)
