---
title: "P-0017: Storage Cluster Model, Source-of-Truth Principle, and Column Promotion Policy"
summary: "Refines P-0001: the content substrate's per-artifact-type tables are classified by a four-shape data-shape taxonomy (state-bearing / narrative / reference / associative); cluster membership — not the table — is what carries the shared index / RLS / retention / projection policy. Locks the source-of-truth principle (the content row is authoritative; every derived surface is non-authoritative and rebuildable; cross-plugin references are soft refs, only core entities are hard FK targets) and the column-promotion policy (JSONB is the default; promotion is additive and non-destructive; the JSONB field stays the source of truth; physical extraction is forbidden at V0)."
primary-audience: agent
---

---
status: "proposed"
date: "2026-07-02"
decision-makers: ["the maintainer"]
consulted: ["the architect", "the researcher", "the orchestrator"]
informed: []
supersedes: null
superseded_by: null
overrides: null
---

# P-0017: Storage Cluster Model, Source-of-Truth Principle, and Column Promotion Policy

**Project:** mnemra-core

## Status

`proposed`

Authored at the foundational-ADR-cluster stage as the formalization of the storage-layout verdicts already locked in the working docs — the Round-2 cluster review (verdict pressure-tested and accepted 2026-05-04) and the system-overview walk that folded in the subsequent use-case deltas — plus the remaining details (source-of-truth principle, column-promotion policy) locked against canon here. It flips to `accepted` at the maintainer's review gate; the core cluster-model choice is not re-opened by that gate (it formalizes a locked verdict), what the gate reviews is this rendering at ADR precision and the three explicit reconciliations flagged in "Decision Drivers" and "More Information".

This ADR **refines** — it does not supersede — [P-0001-storage-layout](P-0001-storage-layout.md) (C1 single-document layout: whole artifact in one row, JSONB frontmatter + body + system fields, per-artifact-type tables). P-0001's C1 layout choice **and** its per-artifact-type-table granularity both stand. P-0017 adds the layer P-0001 left open: the **cluster-shape taxonomy** that classifies each per-type table by its data shape, and the two governing principles (source-of-truth, column-promotion) that a per-shape policy hangs off.

**One P-0001 mechanism does *not* stand, and this ADR names the carve-out so two accepted ADRs do not silently contradict.** P-0001 specifies mutation history as "a `tasks_history` shadow table populated by trigger on UPDATE." That trigger-shadow audit mechanism is **superseded** — by the Round-2 correction that reshaped audit into a host-fn-emitted, append-only, artifact-outliving surface (locked as a core-owned emit surface in [P-0018-core-entity-manifest](P-0018-core-entity-manifest.md) D-SURFACE; its storage shape owned by the [observability baseline](../architecture/overview.md#observability) per [P-0010](P-0010-storage-substrate-engine.md) D8 escalation E1). What stands from P-0001 is **C1 + per-artifact-type tables**; what is superseded is the `tasks_history` trigger-shadow. The `associative`/state-bearing per-shape policies below emit audit via host-fn, never via a per-type trigger-shadow table.

P-0017 also refines the cohesion framing of [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md) (4 `core: true` plugins, each owning its artifact-family tables) and sits under the engine-agnostic `Storage` trait locked by [P-0010-storage-substrate-engine](P-0010-storage-substrate-engine.md) (D5 — one swappable trait, Postgres the only V0 implementation; `libs/mnemra-host/storage.rs`), which is mnemra-core's application of the workspace-general engine-agnostic-seam standard `G-0015` (relational substrate behind a locked `Storage` seam; second adapter deferred behind a trip-wire — see [DEFAULTS.md](DEFAULTS.md)).

**Substrate-independent.** The taxonomy, the source-of-truth principle, and the promotion policy are expressed against the `Storage` trait's record/transaction contract, not against Postgres specifics. Postgres index/RLS mechanics appear only as the V0 implementation illustration under P-0010's Postgres adapter; a conforming second adapter would carry the same taxonomy and principles by a different mechanism.

## Context and Problem Statement

[P-0001-storage-layout](P-0001-storage-layout.md) locked *how a single artifact is laid out* (C1: one row, JSONB frontmatter + body) and *the table granularity* (one per-artifact-type table, e.g. `tasks`). It deliberately left open the layer above the individual table: **how the growing set of per-type tables is organized, and what policy is shared across tables of the same data shape.** Without that layer, index strategy, row-level-security shape, retention, and projection/source-of-truth posture are decided ad hoc per table — a policy that must be restated (and can drift) on every new content type a plugin declares. mnemra-core's V0 migration scope alone is ~10 artifact types across 4 `core: true` plugins ([P-0002](P-0002-core-plugin-partition.md)), and third-party plugins at V0.1+ multiply that.

The Round-2 storage review found the organizing axis: **data shape.** Artifacts fall into a small number of shapes — structured-state-bearing (status/owner/priority mutate; frontmatter dominates), narrative (body dominates; frontmatter sparse), reference (read-mostly lookups), and associative (links/edges between artifacts). Tables of the same shape want the same index strategy, the same RLS shape, the same retention and projection posture. The review's insight is that a **shape taxonomy is the unit a shared policy attaches to** — one policy per shape serves every table of that shape (P-MinBlastRadius: one policy, N tables, not N policies).

Two further questions the working docs raised but did not pin at ADR precision, and which this ADR must close because every capability plugin gates on them:

1. **Source of truth.** When an artifact's data is also visible through a materialized projection, a promoted index column, a derived "inbox" queue view, or a cross-artifact edge, *which representation is authoritative* — and what invariant keeps the derived ones honest? An unstated answer is how a review ends up trusting a stale secondary source over the primary (the 2026-06-08 schema-excavation case: two session tables that duplicated one entity and disagreed on 100% of shared rows, undetected until queried).
2. **Column promotion.** A hot frontmatter field (e.g. `status`) wants a real index, and sometimes a typed column, for query performance. *When* does a field inside the JSONB document graduate to a first-class index or column, *how* is that done without a table rebuild, and *what keeps the promotion from silently moving the source of truth* or breaking the R2.7 frontmatter round-trip?

This ADR does **not** re-decide C1 vs C2 vs C3 (that is [P-0001](P-0001-storage-layout.md)), the substrate/engine (that is [P-0010](P-0010-storage-substrate-engine.md)), the plugin manifest/ABI (that is [P-0003-plugin-manifest](P-0003-plugin-manifest.md)), the edge schema (that is [P-0016-edge-schema](P-0016-edge-schema.md), accepted pending merge), or observability storage (re-altituded out of the ADR layer per P-0010 D8 escalation E1, 2026-06-09 — the [observability baseline](../architecture/overview.md#observability)). It composes with all of them.

## Decision Drivers

- **A shared policy needs a unit to attach to (P-MinBlastRadius).** Index / RLS / retention / projection policy stated per-table drifts across tables and ripples on every new content type. A shape taxonomy gives one policy per shape that N same-shape tables inherit — a change to a shape's policy lands once.
- **The accepted corpus already fixed the table granularity, and the manifest encodes it.** [P-0003-plugin-manifest](P-0003-plugin-manifest.md) (accepted) declares `[content_types]` where **each content type maps to its own named per-artifact-type table** (`task = { table = "tasks", schema_doc = "docs/schemas/task.md" }`; "Under C1, each type is a per-artifact-type Postgres table"). Any cluster model that collapsed those into shared physical tables would contradict two accepted ADRs (P-0001's per-type tables, P-0003's per-type-table manifest declarations) and force a cascade amendment into the built manifest schema. The taxonomy must therefore sit **over** per-type tables, not replace them (reconciliation flagged for maintainer confirmation).
- **Plugin sandbox isolation is per-plugin-table-shaped (Security).** Under [P-0003](P-0003-plugin-manifest.md) the plugin sees only its own declared tables; the host mediates all DDL and enforces `workspace_id` from `WorkspaceCtx` ([P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md)). Per-plugin-namespaced tables keep that isolation crisp; a shared physical table holding many plugins' rows blurs table-level ownership and RLS attribution.
- **Frontmatter must round-trip byte-equal (R2.7).** Source frontmatter is stored literally in JSONB and serializes back byte-equal (modulo system fields) per [P-0001](P-0001-storage-layout.md). Any promotion mechanism that physically extracted a field out of the JSONB document would break that round-trip — so promotion must be additive over the JSONB, not destructive.
- **Additions must be non-breaking (R2.4 migration parsimony).** pgvector / full-text / edge promotion at V0.1+ are non-breaking column/table additions under C1. Column promotion inherits the same constraint: an index or generated column is an additive migration, never a table rebuild.
- **Defer speculative mechanism until evidence forces it (P-Defer, Simplicity).** JSONB with expression indexes is sufficient at V0 dogfood scale; a promotion is a mechanism added on measured evidence, not anticipated need. The promotion *policy* locks now; specific numeric thresholds defer to the query-instrumentation surface behind a named trip-wire.
- **Instrument before the heavy lift, backfillable-first (P-InstrumentBefore / IB1).** The signal that fires a promotion (predicate frequency, planner mis-estimate, latency over budget) is reconstructable from existing query logs, so the promotion decision reads real baseline evidence rather than waiting for forward-only capture.
- **Honesty about the decision space (P-PreserveDecisionSpace).** The Round-2 review's alternative mechanization — a shared polymorphic table per cluster — is a real, argued option with real wins (RLS-policy-count reduction, GIN-selectivity-at-scale, non-breaking collapse to a single table). It is recorded below as a rejected alternative with its reasons, not dissolved as "refined away".

## Considered Options

The organizing axis (data-shape clusters) is the locked Round-2 verdict and is **not** re-litigated. The live decision this ADR closes is the *mechanization* — how the cluster taxonomy relates to the physical tables:

1. **T1 — Per-artifact-type tables classified into cluster shapes (chosen).** Each plugin-declared content type keeps its own per-type table ([P-0001](P-0001-storage-layout.md) / [P-0003](P-0003-plugin-manifest.md)); every table is tagged with exactly one of four data-shape clusters; cluster membership carries the shared index / RLS / retention / projection / promotion policy for all tables of that shape. The cluster is a **taxonomy label + policy carrier**, not a physical table.
2. **T2 — One shared polymorphic table per cluster (the Round-2 literal mechanization).** Each cluster is a single physical table (`artifacts_state_bearing`, etc.) discriminating types by a `type` column; multiple plugins' content types are rows in the same table. Fewer physical tables; ~4 RLS policies instead of ~10.
3. **T3 — Per-type tables with no cluster taxonomy (P-0001 as-is).** Keep per-type tables; add no organizing layer; state index/RLS/retention/promotion policy per table.

## Decision Outcome

**T1 — per-artifact-type tables classified into cluster shapes.** Three decisions lock.

### D-CM — The cluster model: a four-shape taxonomy over per-type tables

The content substrate is organized by a **closed four-shape taxonomy**. Each per-artifact-type table ([P-0001](P-0001-storage-layout.md) C1 layout; [P-0003](P-0003-plugin-manifest.md) manifest declaration) is classified into **exactly one** cluster shape by its data shape. The cluster is the unit a shared per-shape policy attaches to; it is not a physical table and does not merge any plugin's tables. *(Anchors: the Round-2 storage review verdict, 2026-05-04 — the data-shape organizing axis, pressure-tested and accepted at that review; P-MinBlastRadius — one policy per shape, N tables inherit; Simplicity — a small closed shape set over the growing type set.)*

| Cluster shape | Data shape | Example content types | Governing per-shape policy |
|---|---|---|---|
| **state-bearing** | Structured frontmatter dominates; status/owner/priority mutate; hot query fields | tasks, dispatches, skill-runs, repos, job-applications, contacts, inbox items | Expression indexes on hot frontmatter fields; recency index on `updated_at`; audit emission on mutation |
| **narrative** | Body dominates; frontmatter sparse; R2.7 round-trip primacy | articles, daily logs, decisions, research briefs, prompts | Body-oriented; full-text promotion path (V0.1+); minimal frontmatter indexing |
| **reference** | Read-mostly shared lookups; long-lived | about, memory, reference, templates | Read-optimized; low write-amplification; cache-friendly |
| **associative** | Links/edges/joins between artifacts | the edge table ([P-0016](P-0016-edge-schema.md)), tag associations | Traversal-oriented; the edge schema and traversal contract are owned by [P-0016](P-0016-edge-schema.md), not re-decided here |

Binding rules:

- **The taxonomy is closed at four shapes at V0.** A new content type is classified into one of the four; it does not mint a fifth shape. Adding a shape is an amendment to this ADR (closed-but-extensible at ADR tier). *(Anchor: Simplicity — a bounded shape set is what makes the shared-policy claim hold.)*
- **Classification is a property of the content type, declared once.** The host records each declared content type's cluster at manifest-load time; the plugin does not choose storage mechanics ("plugin says what shape; host decides where it lives" — the [architecture overview](../architecture/overview.md) Layer-1 storage-contract framing). Where a content type's cluster is not self-evident from its schema, the assignment is a maintainer call recorded against the type.
- **`associative` defers its schema to [P-0016](P-0016-edge-schema.md).** The edge table (one superset table extending the `0.8.0` relationships substrate, closed edge-type vocabulary, `edge_class`/`origin` discriminators, recursive-CTE traversal) is **already owned by P-0016** (accepted pending merge). P-0017 places the `associative` cluster *shape* and the per-shape policy slot; it does **not** re-decide the edge schema. Double-deciding it is forbidden.
- **Inbox is not a cluster shape — it is a derived view.** The Round-2 predecessor named a fifth `inbox` cluster; the system-overview walk demoted it: inbox *items* are state-bearing content (they carry lifecycle state and mutate — validated by the inbox-triage use case, 2026-05-04, whose own finding is "queue-shape doesn't need a special substrate — it's content-shape with appropriate indexing"), and the *inbox queue* is a derived view over those state-bearing rows (`ORDER BY arrived_at WHERE triage_state = 'pending'`). This is a direct consequence of D-SoT below. *(Anchor: the use-case delta surfaced in the overview walk; D-SoT — a queue projection is non-authoritative.)*

### D-SoT — Source-of-truth principle

**The artifact content row (its JSONB frontmatter + body, in its per-type table) is the single source of truth for that artifact's data.** Every other representation of that data is *derived* and *non-authoritative*. *(Anchors: Honesty — one authoritative source, everything else labeled derived; P-LockContract — the content row is the locked contract, derived surfaces vary behind it; P-MinBlastRadius — one place a value changes.)*

Binding rules:

- **Every derived surface is reconstructable from source rows with no external input.** Materialized projections, promoted index columns, derived queue views (the inbox view), and cross-artifact edges of `origin = extracted` ([P-0016](P-0016-edge-schema.md)) are all rebuildable by replaying from their cluster's source rows. *Binary-observable:* dropping and rebuilding any derived surface yields a result equal (modulo row ordering) to the pre-drop surface, given only the source rows as input. A derived surface that cannot be reconstructed from source rows is a defect, not a source of truth.
- **A derived surface is never written as if authoritative.** Writes land on the source row; derived surfaces are refreshed from it (host-owned projection refresh per [P-0001](P-0001-storage-layout.md)). No write path treats a projection, a promoted column, or a view as the write target for artifact data.
- **Cross-plugin references are soft refs; only core entities are hard FK targets.** A reference from one plugin's artifact to another plugin's artifact is an opaque ID with **no** database foreign key ([P-0002](P-0002-core-plugin-partition.md) — cross-plugin aggregation is a projection concern; the [architecture overview](../architecture/overview.md) Layer-2 API contract — soft refs, host-mediated). The *authoritative* copy of a referenced entity lives in its owning plugin/core; the soft ref is a pointer, never a duplicated copy. Hard foreign keys are reserved for references to the **core opinionated entities** locked in [P-0018-core-entity-manifest](P-0018-core-entity-manifest.md) (projects, actors, tags, attachments), which every plugin may FK to. *(Migration delta: the legacy task-store `task.repo_id → repos` foreign key becomes a soft ref, because `repos` is a plugin entity under [P-0002](P-0002-core-plugin-partition.md), not a core entity; `task.project_id → projects` stays a hard FK, because `projects` is core. Surfaced by the schema-context-excavation use case, 2026-06-08.)*
- **Derivation across clusters carries a lineage pointer.** When an artifact is derived from another (an inbox item routed into a task; an ELT transform producing a canonical entity from staged source), the derived artifact carries a `derived_from` soft ref to its source, and the source remains the source-of-record for the derived-from relationship ([architecture overview](../architecture/overview.md) ingest-ELT framing; the inbox-triage use case, 2026-05-04). Source and derived are distinct rows with a known relationship, not the same data restructured.

### D-CP — Column promotion policy

A **column promotion** graduates a hot frontmatter field from *inside* the JSONB document to a first-class query mechanism. JSONB is the default home for every frontmatter field; promotion is the exception, taken on measured evidence. *(Anchors: Simplicity + P-Defer — JSONB is the smallest sufficient mechanism, promote only when evidence forces it; P-InstrumentBefore / IB1 — the firing signal is backfillable from query logs; R2.4 — additive migrations only; R2.7 + D-SoT — the JSONB field stays the source of truth.)*

Binding rules:

- **Promotion is additive and non-destructive; the JSONB field stays the source of truth.** The promotion ladder, cheapest first: (1) an **expression index** over the JSONB field (`CREATE INDEX ... ((frontmatter->>'status'))`, optionally partial `WHERE type = 'X'`); (2) a **generated column** derived from the JSONB field, plus an index on it, when typed comparison or repeated projection warrants it. In every case the promoted artifact is a *derivation* of the JSONB field, which remains the source of truth (D-SoT). *Binary-observable:* after any promotion, the source frontmatter still round-trips byte-equal (R2.7), and dropping the promoted index/column and re-reading from JSONB returns the same values.
- **Physical extraction is forbidden at V0.** Physically moving a field *out* of the JSONB document into a standalone column (removing it from `frontmatter`) is not permitted at V0: it would break the R2.7 round-trip and move the source of truth off the content row. *(Anchor: R2.7 + D-SoT. Trip-wire to reconsider: a field whose write-amplification or storage cost under JSONB is measured — via query/write instrumentation — to exceed budget AND whose promotion to a generated column does not recover it; reconsideration is a maintainer call producing an amendment, not an autonomous extraction.)*
- **Promotion fires on measured evidence, not anticipation.** A field is a promotion candidate when instrumentation shows it appears in a `WHERE`/`ORDER BY` predicate above a measured frequency **and** the JSONB expression-index plan is empirically insufficient (planner mis-estimate driving latency over the query budget). The maintainer decides, driven by the instrument; a plugin cannot self-promote, and an agent does not promote speculatively. *(Anchor: P-InstrumentBefore — the query-instrumentation surface is the input.)*
- **The specific numeric thresholds are deferred, with a named trip-wire.** *Decision content:* the promotion criteria are (predicate-frequency threshold, expression-index-selectivity-insufficiency, latency-over-budget); what defers is only their numeric calibration. *Deferral anchor:* P-Defer / DF1 + IB1 — the thresholds are sized from the evidence the instrument surfaces, not guessed now. *Trip-wire:* the query-instrumentation surface (the [observability baseline](../architecture/overview.md#observability) query-latency + predicate-frequency signals) reporting a field over the frequency-and-latency bound; that report is the mechanical firing event that puts a specific field's promotion in front of the maintainer. Until the instrument exists, the item is **parked** on the observability-baseline delivery, not silently pending.

### Consequences

**Good:**

- One index / RLS / retention / projection / promotion policy per shape serves every same-shape table; a policy change lands once (P-MinBlastRadius). New content types inherit their shape's policy at classification time — no per-table policy restatement.
- Consistent with the accepted corpus: P-0001's per-type tables and P-0003's per-type-table manifest declarations are unchanged; no manifest-schema cascade.
- Plugin sandbox isolation stays crisp (per-plugin-namespaced tables; plugin sees only its own; host-mediated DDL and `workspace_id` — [P-0003](P-0003-plugin-manifest.md), [P-0006](P-0006-v0-tenant-enforcement.md)).
- The source-of-truth principle makes the "stale secondary source trusted over primary" failure (the schema-context-excavation use case, 2026-06-08) a structural impossibility: derived surfaces are rebuildable and never written as authoritative.
- Column promotion is a bounded, additive, evidence-driven operation that preserves R2.7 and the source of truth — no table rebuild, no round-trip break.

**Bad / Trade-offs:**

- Per-shape RLS is ~10 policies at V0 (one per table) rather than the ~4 a shared-polymorphic-table model (T2) would give. Accepted: the policy surface is small and auditable at V0 scale, and per-table policies keep RLS attribution aligned with plugin ownership. (The RLS **role model and per-(role, table) policy shape** are owned by [P-0009-rls-admin-token](P-0009-rls-admin-token.md) — binary admin/read-observer roles, ~20 policies at V0.1 = 2 roles × ~10 tables, application-layer at V0; P-0017 does not re-decide it. "One policy per table" here is the per-shape *uniformity* claim: every table of a given cluster shape carries the same workspace-isolation policy shape, so the role model applies uniformly within a shape.)
- GIN-on-frontmatter selectivity at large scale is a real T2 advantage this model forgoes; the promotion ladder (expression + generated columns) plus per-table indexes recover most of it at V0 dogfood scale, and the T2 collapse remains available at V0.1+ behind a scale trip-wire (see Alternatives).
- The four-shape taxonomy is a judgment surface: a content type whose shape is ambiguous needs a maintainer classification call. Bounded by the closed shape set and recorded against the type.

## Pros and Cons of the Options

### T1 — Per-type tables classified into cluster shapes (chosen)

- Pro: consistent with accepted P-0001 (per-type tables) and P-0003 (per-type-table manifest), so no cross-ADR contradiction and no manifest cascade.
- Pro: preserves per-plugin table ownership and sandbox isolation.
- Pro: the cluster is a pure policy carrier — one policy per shape, N tables inherit (P-MinBlastRadius).
- Con: ~10 RLS policies at V0, not ~4; larger GIN footprint at extreme scale than a shared table.

### T2 — One shared polymorphic table per cluster (Round-2 literal mechanization)

- Pro: ~4 physical tables and ~4 RLS policies; GIN-on-frontmatter selectivity stays shape-coherent (the Round-2 selectivity argument); a non-breaking collapse to a single polymorphic table is available at scale.
- Con: **contradicts accepted P-0001 (per-type tables) and P-0003 (per-type-table manifest declarations)** — adopting it forces a supersede + a cascade amendment into the built manifest schema.
- Con: multiple plugins' rows share one physical table, blurring per-plugin table ownership and RLS attribution against the [P-0003](P-0003-plugin-manifest.md) sandbox model (plugin sees only its own tables).
- Con: its selectivity-at-scale win is not load-bearing at V0 dogfood scale (<1k rows/type); the promotion ladder recovers most of it. **Preserved as a V0.1+ option** behind a scale trip-wire: if measured GIN size / planner accuracy on per-type tables degrades past budget at production scale, the shape-coherent shared table (or Postgres declarative partitioning by `type`) is the promotion path — a storage-layer refactor that leaves the plugin contract and this ADR's taxonomy/principles intact.

### T3 — Per-type tables, no cluster taxonomy (P-0001 as-is)

- Pro: smallest change; no new organizing concept.
- Con: index / RLS / retention / projection / promotion policy is restated per table and drifts; a policy change ripples across every table by hand. The taxonomy exists precisely to give that policy a single attachment point (P-MinBlastRadius).

## More Information

**Reconciliations flagged for maintainer confirmation at the review gate:**

1. **Refine vs supersede P-0001.** This ADR reads P-0001's "per-artifact-type tables over polymorphic single-table" as *preserved*, and adds the cluster taxonomy as a layer P-0001 left open — a refinement, not a supersession. The alternative reading (the Round-2 verdict intends the literal shared-polymorphic-table collapse, T2) would instead *supersede* P-0001 and cascade an amendment into accepted [P-0003](P-0003-plugin-manifest.md). The evidence (P-0003's accepted per-type-table manifest declarations; P-MinBlastRadius) points to refine; confirm the reading.
2. **`associative` cluster ↔ P-0016 boundary.** P-0017 places the `associative` shape and its policy slot; [P-0016-edge-schema](P-0016-edge-schema.md) owns the edge table, vocabulary, and traversal. Confirm there is no overlap to resolve at merge (P-0014/P-0015/P-0016 are on an unmerged sibling branch).
3. **Inbox demotion.** The `inbox` cluster of the Round-2 predecessor is dropped in favor of inbox-items-as-state-bearing + inbox-as-derived-view. Confirm this fold-in of the overview-walk delta.

**Relationships:**

- Refines: [P-0001-storage-layout](P-0001-storage-layout.md) — C1 layout + per-type tables stand; this ADR adds the cluster taxonomy, source-of-truth principle, and promotion policy P-0001 left open.
- Refines: [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md) — plugin cohesion is per-artifact-family tables classified by shape; cross-plugin references are soft refs.
- Sits under: [P-0010-storage-substrate-engine](P-0010-storage-substrate-engine.md) — the taxonomy and principles are expressed against the engine-agnostic `Storage` trait (D5; `libs/mnemra-host/storage.rs`); Postgres mechanics are the V0 implementation illustration.
- Composes with: [P-0003-plugin-manifest](P-0003-plugin-manifest.md) (per-type-table declarations; host-mediated DDL); [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) (`workspace_id` from `WorkspaceCtx`); [P-0016-edge-schema](P-0016-edge-schema.md) (the `associative` cluster's edge table — accepted pending merge); [P-0018-core-entity-manifest](P-0018-core-entity-manifest.md) (the hard-FK-target core entities).
- Source verdict: the Round-2 storage review (2026-05-04) — the data-shape organizing axis (locked, pressure-tested at that review). Forward-design companion: the [architecture overview](../architecture/overview.md) storage-layer section — the four cluster shapes and the per-plugin-namespaced-table framing folded in here. (These provenance docs are workspace-private working artifacts; their verdicts are stated inline above so this ADR reads self-contained.)
- Validated against use cases (workspace-private working artifacts): the `get_context_for` use case, 2026-06-05 (associative/edge cluster + soft refs across families); the schema-context-excavation use case, 2026-06-08 (core-entity FK targets + source-of-truth over stale secondary sources); the inbox-triage use case, 2026-05-04 (inbox-as-state-bearing-content + inbox-as-derived-view). No use case surfaced a shape the model cannot serve.
- Deferred with trip-wire: column-promotion numeric thresholds (parked on the observability-baseline query-instrumentation surface; fires when a field crosses the measured predicate-frequency-and-latency bound). T2 shared-table collapse (parked on a production-scale GIN/planner-degradation trip-wire).
