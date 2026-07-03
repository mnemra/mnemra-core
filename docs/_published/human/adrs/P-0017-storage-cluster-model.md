---
title: "P-0017: Storage Cluster Model, Source-of-Truth Principle, and Column Promotion Policy"
summary: "Refines P-0001: the content substrate's per-artifact-type tables are classified by a four-shape data-shape taxonomy (state-bearing / narrative / reference / associative); cluster membership — not the table — is what carries the shared index / RLS / retention / projection policy. Locks the source-of-truth principle (the content row is authoritative; every derived surface is non-authoritative and rebuildable; cross-plugin references are soft refs, only core entities are hard FK targets) and the column-promotion policy (JSONB is the default; promotion is additive and non-destructive; the JSONB field stays the source of truth; physical extraction is forbidden at V0)."
primary-audience: agent
---

---
status: "accepted"
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

`accepted`

This document (an ADR, Architecture Decision Record: it captures a decision, the context behind it, and the alternatives it rejected) was authored at the foundational-ADR-cluster stage. It formalizes storage-layout decisions already locked in the working docs: the Round-2 cluster review (its verdict was pressure-tested and accepted on 2026-05-04) and the system-overview walk, which folded in later use-case deltas. It also locks the remaining details, the source-of-truth principle and the column-promotion policy, against canon here.

The maintainer accepted it at the review gate on 2026-07-03. That gate didn't reopen the core cluster-model choice, since this document formalizes a verdict that was already locked. Instead, the gate reviewed this rendering at ADR precision, and confirmed the three explicit reconciliations flagged in "Decision Drivers" and "More Information".

This ADR **refines** [P-0001-storage-layout](P-0001-storage-layout.md) (a project-scoped ADR, prefixed P- rather than G- for workspace-wide decisions); it does not supersede it. P-0001 locked the C1 single-document layout (the whole artifact in one row, JSONB frontmatter plus body plus system fields) and the per-artifact-type-table granularity. Both choices stand. P-0017 adds the layer P-0001 left open: the cluster-shape taxonomy that classifies each per-type table by its data shape, plus the two governing principles, source-of-truth and column-promotion, that a per-shape policy hangs off.

**One P-0001 mechanism does *not* stand, and this ADR names the carve-out so two accepted ADRs don't silently contradict each other.** P-0001 specifies mutation history as "a `tasks_history` shadow table populated by trigger on UPDATE." That trigger-shadow audit mechanism is **superseded** by the Round-2 correction, which reshaped audit into a host-fn-emitted, append-only, artifact-outliving surface (locked as a core-owned emit surface in [P-0018-core-entity-manifest](P-0018-core-entity-manifest.md) D-SURFACE; its storage shape is owned by the [observability baseline](../architecture/overview.md#observability) per [P-0010](P-0010-storage-substrate-engine.md) D8 escalation E1). What stands from P-0001 is **C1 plus per-artifact-type tables**; what's superseded is the `tasks_history` trigger-shadow. The `associative` and state-bearing per-shape policies below emit audit via host-fn, never via a per-type trigger-shadow table.

P-0017 also refines the cohesion framing of [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md) (4 `core: true` plugins, each owning its artifact-family tables). And it sits under the engine-agnostic `Storage` trait locked by [P-0010-storage-substrate-engine](P-0010-storage-substrate-engine.md) (D5: one swappable trait, with Postgres as the only V0 implementation, in `libs/mnemra-host/storage.rs`). That trait is mnemra-core's application of the workspace-general engine-agnostic-seam standard `G-0015` (a G-* ADR: a workspace-wide decision that applies across every project, rather than a single one). G-0015 locks a relational substrate behind a `Storage` seam and defers a second adapter behind a trip-wire; see [DEFAULTS.md](DEFAULTS.md) (the project's frozen snapshot of workspace-wide architecture decisions, captured when the project was created).

**Substrate-independent.** The taxonomy, the source-of-truth principle, and the promotion policy are expressed against the `Storage` trait's record and transaction contract, not against Postgres specifics. Postgres index and RLS mechanics appear only as the V0 implementation illustration under P-0010's Postgres adapter. A conforming second adapter would carry the same taxonomy and principles through a different mechanism.

## Context and Problem Statement

[P-0001-storage-layout](P-0001-storage-layout.md) locked *how a single artifact is laid out* (C1: one row, JSONB frontmatter plus body) and *the table granularity* (one per-artifact-type table, for example `tasks`). It deliberately left open the layer above the individual table: how the growing set of per-type tables gets organized, and what policy is shared across tables of the same data shape. Without that layer, index strategy, row-level-security shape, retention, and projection or source-of-truth posture get decided ad hoc per table. That policy has to be restated, and can drift, on every new content type a plugin declares. mnemra-core's V0 migration scope alone spans about 10 artifact types across 4 `core: true` plugins ([P-0002](P-0002-core-plugin-partition.md)), and third-party plugins at V0.1+ multiply that.

The Round-2 storage review found the organizing axis: data shape. Artifacts fall into a small number of shapes. Structured-state-bearing artifacts have frontmatter that dominates, with status, owner, and priority fields that mutate. Narrative artifacts have a body that dominates, with sparse frontmatter. Reference artifacts are read-mostly lookups. Associative artifacts are links or edges between other artifacts. Tables of the same shape want the same index strategy, the same RLS shape, and the same retention and projection posture. The review's insight is that a shape taxonomy is the unit a shared policy attaches to: one policy per shape serves every table of that shape (P-MinBlastRadius, the principle that a change should reach as far as the architecture allows rather than rippling across many files: one policy, N tables, not N policies).

The working docs raised two further questions without pinning them at ADR precision. This ADR has to close both, because every capability plugin gates on them:

1. **Source of truth.** When an artifact's data is also visible through a materialized projection, a promoted index column, a derived "inbox" queue view, or a cross-artifact edge, which representation is authoritative? And what invariant keeps the derived ones honest? Leaving that unanswered is how a review ends up trusting a stale secondary source over the primary one. That's what happened in the 2026-06-08 schema-excavation case: two session tables duplicated one entity and disagreed on 100% of their shared rows, undetected until someone queried them.
2. **Column promotion.** A hot frontmatter field (for example, `status`) wants a real index, and sometimes a typed column, for query performance. When does a field inside the JSONB document graduate to a first-class index or column? How is that done without a table rebuild? And what keeps the promotion from silently moving the source of truth, or breaking the R2.7 requirement (an R-code: a stable identifier for a numbered requirement, defined in full at each place it's cited) that frontmatter round-trips unchanged?

This ADR does **not** re-decide C1 versus C2 versus C3 (that's [P-0001](P-0001-storage-layout.md)), the substrate or engine (that's [P-0010](P-0010-storage-substrate-engine.md)), the plugin manifest or ABI (that's [P-0003-plugin-manifest](P-0003-plugin-manifest.md)), the edge schema (that's [P-0016-edge-schema](P-0016-edge-schema.md), accepted pending merge), or observability storage (moved out of the ADR layer per P-0010 D8 escalation E1, 2026-06-09; see the [observability baseline](../architecture/overview.md#observability)). It composes with all of them.

## Decision Drivers

- **A shared policy needs a unit to attach to (P-MinBlastRadius).** Index, RLS, retention, and projection policy stated per table drifts across tables and ripples on every new content type. A shape taxonomy gives one policy per shape that every same-shape table inherits: a change to a shape's policy lands once.
- **The accepted corpus already fixed the table granularity, and the manifest encodes it.** [P-0003-plugin-manifest](P-0003-plugin-manifest.md) (accepted) declares `[content_types]`, where each content type maps to its own named per-artifact-type table (`task = { table = "tasks", schema_doc = "docs/schemas/task.md" }`; "Under C1, each type is a per-artifact-type Postgres table"). Any cluster model that collapsed those into shared physical tables would contradict two accepted ADRs, P-0001's per-type tables and P-0003's per-type-table manifest declarations, and would force a cascade amendment into the built manifest schema. The taxonomy has to sit over per-type tables instead of replacing them (this reconciliation is flagged for maintainer confirmation).
- **Plugin sandbox isolation is per-plugin-table-shaped (Security).** Under [P-0003](P-0003-plugin-manifest.md), a plugin sees only its own declared tables. The host mediates all DDL and enforces `workspace_id` from `WorkspaceCtx` ([P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md)). Per-plugin-namespaced tables keep that isolation crisp; a shared physical table holding many plugins' rows blurs table-level ownership and RLS attribution.
- **Frontmatter must round-trip byte-equal (R2.7).** Source frontmatter is stored literally in JSONB and serializes back byte-equal, modulo system fields, per [P-0001](P-0001-storage-layout.md). Any promotion mechanism that physically extracted a field out of the JSONB document would break that round-trip. So promotion has to be additive over the JSONB, not destructive.
- **Additions must be non-breaking (R2.4 migration parsimony).** pgvector, full-text, and edge promotion at V0.1+ are all non-breaking column or table additions under C1. Column promotion inherits the same constraint: an index or generated column is an additive migration, never a table rebuild.
- **Defer speculative mechanism until evidence forces it (P-Defer, Simplicity).** P-Defer is the principle that mechanism choice waits until evidence forces it, rather than being adopted for an anticipated need. JSONB with expression indexes is sufficient at V0 dogfood scale, and a promotion is a mechanism added on measured evidence, not anticipated need. The promotion policy locks now; the specific numeric thresholds defer to the query-instrumentation surface behind a named trip-wire.
- **Instrument before the heavy lift, backfillable-first (P-InstrumentBefore / IB1).** P-InstrumentBefore is the principle that every production surface ships with metrics, logs, and traces in place before launch, not bolted on after an incident. The signal that fires a promotion, predicate frequency, planner mis-estimate, or latency over budget, is reconstructable from existing query logs. So the promotion decision reads real baseline evidence instead of waiting for forward-only capture.
- **Honesty about the decision space (P-PreserveDecisionSpace).** P-PreserveDecisionSpace is the principle that rejected alternatives stay visible with their reasons, so a later reader can see what was on the table. The Round-2 review's alternative mechanization, a shared polymorphic table per cluster, is a real, argued option with real advantages: fewer RLS policies, better GIN selectivity at scale, and a non-breaking collapse to a single table. It's recorded below as a rejected alternative with its reasons, not dissolved as "refined away".

## Considered Options

The organizing axis, data-shape clusters, is the locked Round-2 verdict and isn't re-litigated here. The live decision this ADR closes is the mechanization: how the cluster taxonomy relates to the physical tables.

1. **T1: Per-artifact-type tables classified into cluster shapes (chosen).** Each plugin-declared content type keeps its own per-type table ([P-0001](P-0001-storage-layout.md) and [P-0003](P-0003-plugin-manifest.md)). Every table is tagged with exactly one of four data-shape clusters. Cluster membership carries the shared index, RLS, retention, projection, and promotion policy for all tables of that shape. The cluster is a taxonomy label plus a policy carrier, not a physical table.
2. **T2: One shared polymorphic table per cluster (the Round-2 literal mechanization).** Each cluster is a single physical table (`artifacts_state_bearing`, and so on) that discriminates types by a `type` column. Multiple plugins' content types become rows in the same table. This means fewer physical tables, about 4 RLS policies instead of about 10.
3. **T3: Per-type tables with no cluster taxonomy (P-0001 as-is).** Keep the per-type tables, add no organizing layer, and state index, RLS, retention, and promotion policy per table.

## Decision Outcome

**T1: per-artifact-type tables classified into cluster shapes.** Three decisions lock.

### D-CM: The cluster model: a four-shape taxonomy over per-type tables

The content substrate is organized by a closed four-shape taxonomy. Each per-artifact-type table ([P-0001](P-0001-storage-layout.md) C1 layout; [P-0003](P-0003-plugin-manifest.md) manifest declaration) is classified into exactly one cluster shape by its data shape. The cluster is the unit a shared per-shape policy attaches to. It's not a physical table, and it doesn't merge any plugin's tables. *(Anchors: the Round-2 storage review verdict, 2026-05-04, the data-shape organizing axis pressure-tested and accepted at that review; P-MinBlastRadius, one policy per shape with N tables inheriting it; Simplicity, a small closed shape set over the growing type set.)*

| Cluster shape | Data shape | Example content types | Governing per-shape policy |
|---|---|---|---|
| **state-bearing** | Structured frontmatter dominates; status/owner/priority mutate; hot query fields | tasks, dispatches, skill-runs, repos, job-applications, contacts, inbox items | Expression indexes on hot frontmatter fields; recency index on `updated_at`; audit emission on mutation |
| **narrative** | Body dominates; frontmatter sparse; R2.7 round-trip primacy | articles, daily logs, decisions, research briefs, prompts | Body-oriented; full-text promotion path (V0.1+); minimal frontmatter indexing |
| **reference** | Read-mostly shared lookups; long-lived | about, memory, reference, templates | Read-optimized; low write-amplification; cache-friendly |
| **associative** | Links/edges/joins between artifacts | the edge table ([P-0016](P-0016-edge-schema.md)), tag associations | Traversal-oriented; the edge schema and traversal contract are owned by [P-0016](P-0016-edge-schema.md), not re-decided here |

Binding rules:

- **The taxonomy is closed at four shapes at V0.** A new content type gets classified into one of the four; it doesn't mint a fifth shape. Adding a shape would require an amendment to this ADR (closed but extensible at ADR tier). *(Anchor: Simplicity, a bounded shape set is what makes the shared-policy claim hold.)*
- **Classification is a property of the content type, declared once.** The host records each declared content type's cluster at manifest-load time; the plugin doesn't choose storage mechanics ("plugin says what shape; host decides where it lives," the [architecture overview](../architecture/overview.md) Layer-1 storage-contract framing). Where a content type's cluster isn't self-evident from its schema, the assignment is a maintainer call recorded against the type.
- **`associative` defers its schema to [P-0016](P-0016-edge-schema.md).** The edge table (one superset table extending the `0.8.0` relationships substrate, a closed edge-type vocabulary, `edge_class`/`origin` discriminators, and recursive-CTE traversal) is already owned by P-0016 (accepted pending merge). P-0017 places the `associative` cluster shape and the per-shape policy slot; it does not re-decide the edge schema. Double-deciding it is forbidden.
- **Inbox is not a cluster shape; it's a derived view.** The Round-2 predecessor named a fifth `inbox` cluster. The system-overview walk demoted it: inbox items are state-bearing content. They carry lifecycle state and mutate, validated by the inbox-triage use case (2026-05-04), whose own finding is "queue-shape doesn't need a special substrate, it's content-shape with appropriate indexing." The inbox queue itself is a derived view over those state-bearing rows (`ORDER BY arrived_at WHERE triage_state = 'pending'`). This follows directly from D-SoT below. *(Anchor: the use-case delta surfaced in the overview walk; D-SoT: a queue projection is non-authoritative.)*

### D-SoT: Source-of-truth principle

**The artifact content row (its JSONB frontmatter plus body, in its per-type table) is the single source of truth for that artifact's data.** Every other representation of that data is derived and non-authoritative. *(Anchors: Honesty, one authoritative source with everything else labeled derived; P-LockContract, the principle that a stable contract lets implementations vary behind it: here, the content row is the locked contract and derived surfaces vary behind it; P-MinBlastRadius: one place a value changes.)*

Binding rules:

- **Every derived surface is reconstructable from source rows with no external input.** Materialized projections, promoted index columns, derived queue views (the inbox view), and cross-artifact edges of `origin = extracted` ([P-0016](P-0016-edge-schema.md)) are all rebuildable by replaying from their cluster's source rows. *Binary-observable:* dropping and rebuilding any derived surface yields a result equal, modulo row ordering, to the pre-drop surface, given only the source rows as input. A derived surface that can't be reconstructed from source rows is a defect, not a source of truth.
- **A derived surface is never written as if authoritative.** Writes land on the source row; derived surfaces are refreshed from it (host-owned projection refresh per [P-0001](P-0001-storage-layout.md)). No write path treats a projection, a promoted column, or a view as the write target for artifact data.
- **Cross-plugin references are soft refs; only core entities are hard FK targets.** A reference from one plugin's artifact to another plugin's artifact is an opaque ID with **no** database foreign key ([P-0002](P-0002-core-plugin-partition.md): cross-plugin aggregation is a projection concern; the [architecture overview](../architecture/overview.md) Layer-2 API contract: soft refs, host-mediated). The authoritative copy of a referenced entity lives in its owning plugin or core; the soft ref is a pointer, never a duplicated copy. Hard foreign keys are reserved for references to the core opinionated entities locked in [P-0018-core-entity-manifest](P-0018-core-entity-manifest.md) (projects, actors, tags, attachments), which every plugin may FK to. *(Migration delta: the legacy task-store `task.repo_id → repos` foreign key becomes a soft ref, because `repos` is a plugin entity under [P-0002](P-0002-core-plugin-partition.md), not a core entity. `task.project_id → projects` stays a hard FK, because `projects` is core. Surfaced by the schema-context-excavation use case, 2026-06-08.)*
- **Derivation across clusters carries a lineage pointer.** When an artifact is derived from another (an inbox item routed into a task; an ELT transform producing a canonical entity from staged source), the derived artifact carries a `derived_from` soft ref to its source, and the source remains the source-of-record for the derived-from relationship ([architecture overview](../architecture/overview.md) ingest-ELT framing; the inbox-triage use case, 2026-05-04). Source and derived are distinct rows with a known relationship, not the same data restructured.

### D-CP: Column promotion policy

A column promotion graduates a hot frontmatter field from inside the JSONB document to a first-class query mechanism. JSONB is the default home for every frontmatter field; promotion is the exception, taken on measured evidence. *(Anchors: Simplicity and P-Defer: JSONB is the smallest sufficient mechanism, promote only when evidence forces it; P-InstrumentBefore / IB1: the firing signal is backfillable from query logs; R2.4: additive migrations only; R2.7 and D-SoT: the JSONB field stays the source of truth.)*

Binding rules:

- **Promotion is additive and non-destructive; the JSONB field stays the source of truth.** The promotion ladder runs cheapest first. First, an expression index over the JSONB field (`CREATE INDEX ... ((frontmatter->>'status'))`, optionally partial with `WHERE type = 'X'`). Second, a generated column derived from the JSONB field, plus an index on it, when typed comparison or repeated projection warrants it. In every case the promoted artifact is a derivation of the JSONB field, which remains the source of truth (D-SoT). *Binary-observable:* after any promotion, the source frontmatter still round-trips byte-equal (R2.7), and dropping the promoted index or column and re-reading from JSONB returns the same values.
- **Physical extraction is forbidden at V0.** Physically moving a field out of the JSONB document into a standalone column (removing it from `frontmatter`) isn't permitted at V0: it would break the R2.7 round-trip and move the source of truth off the content row. *(Anchor: R2.7 and D-SoT. Trip-wire to reconsider: a field whose write-amplification or storage cost under JSONB is measured, via query or write instrumentation, to exceed budget, and whose promotion to a generated column doesn't recover it. Reconsideration is a maintainer call producing an amendment, not an autonomous extraction.)*
- **Promotion fires on measured evidence, not anticipation.** A field becomes a promotion candidate when instrumentation shows it appears in a `WHERE` or `ORDER BY` predicate above a measured frequency, and the JSONB expression-index plan is empirically insufficient (a planner mis-estimate driving latency over the query budget). The maintainer decides, driven by the instrument. A plugin can't self-promote, and an agent doesn't promote speculatively. *(Anchor: P-InstrumentBefore: the query-instrumentation surface is the input.)*
- **The specific numeric thresholds are deferred, with a named trip-wire.** *Decision content:* the promotion criteria are predicate-frequency threshold, expression-index-selectivity-insufficiency, and latency-over-budget. What defers is only their numeric calibration. *Deferral anchor:* P-Defer / DF1 and IB1: the thresholds get sized from the evidence the instrument surfaces, not guessed now. *Trip-wire:* the query-instrumentation surface (the [observability baseline](../architecture/overview.md#observability) query-latency and predicate-frequency signals) reporting a field over the frequency-and-latency bound. That report is the mechanical firing event that puts a specific field's promotion in front of the maintainer. Until the instrument exists, the item stays parked on the observability-baseline delivery, not silently pending.

### Consequences

**Good:**

- One index, RLS, retention, projection, and promotion policy per shape serves every same-shape table; a policy change lands once (P-MinBlastRadius). New content types inherit their shape's policy at classification time, with no per-table policy restatement.
- Consistent with the accepted corpus: P-0001's per-type tables and P-0003's per-type-table manifest declarations are unchanged, with no manifest-schema cascade.
- Plugin sandbox isolation stays crisp: per-plugin-namespaced tables, a plugin sees only its own, host-mediated DDL, and `workspace_id` enforcement ([P-0003](P-0003-plugin-manifest.md), [P-0006](P-0006-v0-tenant-enforcement.md)).
- The source-of-truth principle makes the "stale secondary source trusted over primary" failure (the schema-context-excavation use case, 2026-06-08) a structural impossibility: derived surfaces are rebuildable and never written as authoritative.
- Column promotion is a bounded, additive, evidence-driven operation that preserves R2.7 and the source of truth: no table rebuild, no round-trip break.

**Bad / Trade-offs:**

- Per-shape RLS runs about 10 policies at V0 (one per table) rather than the roughly 4 a shared-polymorphic-table model (T2) would give. That's accepted: the policy surface is small and auditable at V0 scale, and per-table policies keep RLS attribution aligned with plugin ownership. (The RLS role model and per-(role, table) policy shape are owned by [P-0009-rls-admin-token](P-0009-rls-admin-token.md): binary admin and read-observer roles, about 20 policies at V0.1 (2 roles times about 10 tables), application-layer at V0. P-0017 doesn't re-decide it. "One policy per table" here is the per-shape uniformity claim: every table of a given cluster shape carries the same workspace-isolation policy shape, so the role model applies uniformly within a shape.)
- GIN-on-frontmatter selectivity at large scale is a real T2 advantage this model forgoes. The promotion ladder (expression and generated columns) plus per-table indexes recover most of it at V0 dogfood scale, and the T2 collapse remains available at V0.1+ behind a scale trip-wire (see Alternatives).
- The four-shape taxonomy is a judgment surface: a content type whose shape is ambiguous needs a maintainer classification call. Bounded by the closed shape set and recorded against the type.

## Pros and Cons of the Options

### T1: Per-type tables classified into cluster shapes (chosen)

- Pro: consistent with accepted P-0001 (per-type tables) and P-0003 (per-type-table manifest), so there's no cross-ADR contradiction and no manifest cascade.
- Pro: preserves per-plugin table ownership and sandbox isolation.
- Pro: the cluster is a pure policy carrier: one policy per shape, and every table inherits it (P-MinBlastRadius).
- Con: about 10 RLS policies at V0, not about 4; a larger GIN footprint at extreme scale than a shared table.

### T2: One shared polymorphic table per cluster (Round-2 literal mechanization)

- Pro: about 4 physical tables and about 4 RLS policies; GIN-on-frontmatter selectivity stays shape-coherent (the Round-2 selectivity argument); a non-breaking collapse to a single polymorphic table is available at scale.
- Con: contradicts accepted P-0001 (per-type tables) and P-0003 (per-type-table manifest declarations). Adopting it would force a supersede plus a cascade amendment into the built manifest schema.
- Con: multiple plugins' rows share one physical table, blurring per-plugin table ownership and RLS attribution against the [P-0003](P-0003-plugin-manifest.md) sandbox model (a plugin sees only its own tables).
- Con: its selectivity-at-scale win isn't load-bearing at V0 dogfood scale (under 1k rows per type); the promotion ladder recovers most of it. It's preserved as a V0.1+ option behind a scale trip-wire: if measured GIN size or planner accuracy on per-type tables degrades past budget at production scale, the shape-coherent shared table (or Postgres declarative partitioning by `type`) is the promotion path, a storage-layer refactor that leaves the plugin contract and this ADR's taxonomy and principles intact.

### T3: Per-type tables, no cluster taxonomy (P-0001 as-is)

- Pro: smallest change; no new organizing concept.
- Con: index, RLS, retention, projection, and promotion policy gets restated per table and drifts; a policy change ripples across every table by hand. The taxonomy exists precisely to give that policy a single attachment point (P-MinBlastRadius).

## More Information

**Reconciliations flagged for maintainer confirmation at the review gate:**

1. **Refine versus supersede P-0001.** This ADR reads P-0001's "per-artifact-type tables over polymorphic single-table" as preserved, and adds the cluster taxonomy as a layer P-0001 left open: a refinement, not a supersession. The alternative reading, that the Round-2 verdict intends the literal shared-polymorphic-table collapse (T2), would instead supersede P-0001 and cascade an amendment into accepted [P-0003](P-0003-plugin-manifest.md). The evidence (P-0003's accepted per-type-table manifest declarations; P-MinBlastRadius) points to refine. Confirm the reading.
2. **`associative` cluster and the P-0016 boundary.** P-0017 places the `associative` shape and its policy slot; [P-0016-edge-schema](P-0016-edge-schema.md) owns the edge table, vocabulary, and traversal. Confirm there's no overlap to resolve at merge (P-0014, P-0015, and P-0016 are on an unmerged sibling branch).
3. **Inbox demotion.** The `inbox` cluster from the Round-2 predecessor is dropped in favor of treating inbox items as state-bearing content, with the inbox itself as a derived view. Confirm this fold-in of the overview-walk delta.

**Relationships:**

- Refines: [P-0001-storage-layout](P-0001-storage-layout.md). C1 layout and per-type tables stand; this ADR adds the cluster taxonomy, source-of-truth principle, and promotion policy that P-0001 left open.
- Refines: [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md). Plugin cohesion is per-artifact-family tables classified by shape; cross-plugin references are soft refs.
- Sits under: [P-0010-storage-substrate-engine](P-0010-storage-substrate-engine.md). The taxonomy and principles are expressed against the engine-agnostic `Storage` trait (D5; `libs/mnemra-host/storage.rs`); Postgres mechanics are the V0 implementation illustration.
- Composes with: [P-0003-plugin-manifest](P-0003-plugin-manifest.md) (per-type-table declarations; host-mediated DDL); [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) (`workspace_id` from `WorkspaceCtx`); [P-0016-edge-schema](P-0016-edge-schema.md) (the `associative` cluster's edge table, accepted pending merge); [P-0018-core-entity-manifest](P-0018-core-entity-manifest.md) (the hard-FK-target core entities).
- Source verdict: the Round-2 storage review (2026-05-04), the data-shape organizing axis (locked, pressure-tested at that review). Forward-design companion: the [architecture overview](../architecture/overview.md) storage-layer section, where the four cluster shapes and the per-plugin-namespaced-table framing were folded in here. (These provenance docs are workspace-private working artifacts; their verdicts are stated inline above so this ADR reads self-contained.)
- Validated against use cases (workspace-private working artifacts): the `get_context_for` use case, 2026-06-05 (associative/edge cluster and soft refs across families); the schema-context-excavation use case, 2026-06-08 (core-entity FK targets and source-of-truth over stale secondary sources); the inbox-triage use case, 2026-05-04 (inbox-as-state-bearing-content and inbox-as-derived-view). No use case surfaced a shape the model can't serve.
- Deferred with trip-wire: column-promotion numeric thresholds (parked on the observability-baseline query-instrumentation surface; fires when a field crosses the measured predicate-frequency-and-latency bound). T2 shared-table collapse (parked on a production-scale GIN/planner-degradation trip-wire).
