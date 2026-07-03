---
title: "P-0016: Edge Schema"
summary: "Resolves the retrieval-cluster Frame's {{P-0016}} slot. Locks the single superset edge table extending the 0.8.0 relationships substrate: closed eight-type vocabulary (parent / blocks / depends-on / dispatched-by / extends / feeds / cites / supersedes), two discriminating closed-enum columns (edge_class: work-graph|citation; origin: declared|extracted|system) with the supersedes classing rule, one-logical-edge uniqueness with origin-authority upgrade (system > declared > extracted), the extraction contract (frontmatter + free-text sources, source-span provenance, idempotent re-extraction, per-source coverage measured every run), the recursive-CTE traversal contract carrying the workspace predicate at every recursion level with the D4 instrument's field set, and the dual-writer manifest-ownership semantics (repos-plugin CRUD path for work-graph rows; host-side extractor confined to origin=extracted)."
primary-audience: agent
---

---
status: "accepted"
date: "2026-07-02"
decision-makers: ["the maintainer"]
consulted: ["the orchestrator"]
informed: []
supersedes: null
superseded_by: null
overrides: null
---

# P-0016: Edge Schema

**Project:** mnemra-core

## Status

`accepted`

Accepted 2026-07-02 at the retrieval-cluster **spec-exit gate** (reviewed with the spec [2026-07-02-retrieval-cluster](../../specs/2026-07-02-retrieval-cluster.md)). Authored at Stage 3 of the retrieval cluster, resolving the Frame's `{{P-0016}}` slot per [placeholder-resolution](placeholder-resolution.md). The single-superset-table direction is Stage 2a direction 1, decomposer-ratified — not re-opened by the gate; this ADR locks the schema, classing, extraction, traversal, and ownership renderings the Frame delegated.

## Context and Problem Statement

The corpus's graph is authored but latent — frontmatter relation lists, free-text citations, prose. The retrieval cluster makes it traversable (intake SC5), and two edge vocabularies pre-exist the decision: the V0 `0.8.0` work-graph set (`parent / blocks / depends-on / supersedes / dispatched-by` — system-of-record rows created with their entities) and the retrieval citation set (`extends / feeds / cites / supersedes` — re-derivable from authored text). They overlap only on `supersedes`, and `supersedes` is **trust-affecting**: [P-0015](P-0015-provenance-envelope-source-roles.md)'s `outdated` predicate reads it, so where an edge *came from* is an authority question, not bookkeeping (the Frame §5 write-authorization worked case: adversarially-shaped free text must not demote a target's trust through the extractor). The Frame locked one substrate with a discriminating mechanism; this ADR is the slot that locks the schema.

## Decision Drivers

- **One vocabulary, one traversal, one extraction path** (P-MinBlastRadius; Stage 1c F6's preferred resolution) — parallel edge stores mean two query paths for "what relates to X" and a permanent reconciliation burden.
- **Provenance discrimination is load-bearing** (P-0015 PE-7): a trust predicate that cannot distinguish a declared supersession from an extracted one is an authority-laundering vector.
- **The substrate is locked** (P-0010 D4): shallow edge model, recursive CTEs, no graph engine; the traversal path carries D4's instrument.
- **Extraction must be honest** (intake SC5): coverage over the migrated corpus is measured and reported, never assumed.
- **Tenancy + identity are structural from day one** (P-0006; retrofit-prep walk item 11).

## Considered Options

1. **One superset table extending `0.8.0`, discriminated by `edge_class` + `origin` columns (chosen).**
2. **Parallel vocabularies / two edge tables** (work-graph table + citation table) — rejected (Stage 1c F6; the intake carried the composition question to the Frame, which ratified the superset): two traversal code paths, two extraction targets, `supersedes` split across stores exactly where trust reads it.
3. **A graph engine for the citation graph** — rejected upstream (P-0010 D4; intake hard constraint): recursive CTEs serve the shallow model; Apache AGE waits behind D4's strain trip-wire, which this cluster's instrument (ES-5) makes fireable.

## Decision Outcome

**Chosen: Option 1**, rendered as ES-1..ES-7. Binding requirement text: spec [R-0027, R-0034-c, R-0035](../../specs/2026-07-02-retrieval-cluster.md).

### ES-1 — One superset table, closed vocabulary

One edge table — the `0.8.0` relationships substrate, extended — carries the closed vocabulary **`{parent, blocks, depends-on, dispatched-by, extends, feeds, cites, supersedes}`**; `supersedes` unifies the two prior sets. An edge type outside the enum is rejected at write. Vocabulary changes are amendments to this ADR (closed-but-extensible at ADR tier — the P-0015 mechanic-6 discipline applied to edge types). Exactly one traversal code path and one extraction code path operate over the table.

### ES-2 — Discriminating columns + the `supersedes` classing rule

Every row carries two closed-enum columns:

- **`edge_class`** ∈ `{work-graph, citation}` — `parent/blocks/depends-on/dispatched-by` are always `work-graph`; `extends/feeds/cites` are always `citation`; **`supersedes` is classed per its writer**: `work-graph` iff created by the work-graph system-of-record transactional path (entity lifecycle operations — task/dispatch supersession written atomically with its entities), `citation` otherwise (declared in content frontmatter or extracted from free text). The rule is mechanical at write time — the writing path, not a judgment, determines the class.
- **`origin`** ∈ `{declared, extracted, system}` — `declared`: authored in structured frontmatter or a DB relation; `extracted`: derived from free text by the extractor, carrying a **source-span provenance pointer** back to the text it was read from; `system`: created by system operations (e.g. `dispatched-by`). Authority order for trust consumption: **`system` > `declared` > `extracted`** — and per P-0015 PE-7, `extracted` never enters a trust predicate at V0.

**The `superseded-by` forward pointer is the directional view of this edge — not a schema field (r1 fold).** Retrieval's hard-supersession condition (spec R-0029-e; P-0015 PE-7) is: the artifact has an incoming `supersedes` edge of origin `declared` or `system`. "Forward pointer" names that edge read from the superseded artifact's side; there is no separate `superseded_by` column, no normalized-topic-key, and no second supersession mechanism (P-MinBlastRadius — one source of truth where trust reads; a topic-key deriver would put an inference in a trust path, barred by spec R-0026-a). An `origin = extracted` supersedes edge is *not* a forward pointer — it neither excludes nor demotes (PE-7's origin weighting).

Work-graph rows are system-of-record, created transactionally with their entities. Citation rows are re-derivable.

### ES-3 — Uniqueness + origin-authority upgrade

**One logical edge per `(workspace_id, src_artifact_id, dst_artifact_id, edge_type)`.** A single row carries each logical edge so trust predicates and traversal read one source of truth (P-MinBlastRadius). Origin semantics on collision:

- An extractor write finding an existing row at equal-or-higher authority (`declared`/`system`, or an existing `extracted` row) is a **no-op** (idempotent re-extraction refreshes `source_span` only).
- A `declared`/`system` write finding an existing `extracted` row **upgrades the row's origin** (a recorded write — the row's provenance changed, and for `supersedes` that change is trust-affecting, so it rides the P-0015 PE-6 recorded-write discipline).
- Nothing ever downgrades origin authority; deleting a `declared` relation from frontmatter removes the row on re-extraction only if no higher-authority writer owns it.

Extracted rows carry `source_span` (the source artifact/path + span the citation was read from) — required iff `origin = extracted`. Every row carries `workspace_id NOT NULL` (indexed, explicitly passed) and `owner`/`created_by` (defaulted at V0 single-user) per spec R-0035.

### ES-4 — Extraction contract: sources, idempotency, measured coverage, integrity

- **Sources:** (a) frontmatter relation lists → rows with `origin = declared`; (b) free-text citations — markdown links to in-corpus artifacts, decision-record identifiers (`P-NNNN`/`G-NNNN`), requirement IDs, in-corpus path citations → rows with `origin = extracted` + `source_span`.
- **Idempotency:** re-extraction over an unchanged corpus yields byte-identical edge rows (ES-3's upsert semantics); extraction re-fires on document change as part of the index pipeline.
- **Coverage is measured, never assumed (intake SC5):** every index run records, per source class, how many frontmatter relations and free-text citations resolved to edges vs failed to resolve (unresolved citations are counted with their spans in the build record — they are *not* written as dangling edge rows). The count runs against the real migrated corpus, not a fixture.
- **Extraction integrity (with P-0015):** the extractor writes only `origin = extracted` rows; it can never author `declared`/`system` authority. Promotion of an extracted trust-affecting edge to `declared` happens only through the admin-gated recorded-write path (P-0015 PE-6/PE-7). When register `1.2.0` (ongoing ingest) opens an untrusted-submitter path into frontmatter, the `declared` authority weight re-opens — the named trip-wire P-0015 PE-7 carries.

### ES-5 — Traversal contract + the D4 instrument's field set

- **Shape:** graph traversal is a recursive CTE over the edge table (P-0010 D4 — borrow the graph *model*, not a graph *engine*), powering `get_context_for`'s graph position (typed relations, lifecycle state, linked-artifact summaries).
- **Tenancy + policy:** the workspace predicate appears **at every recursion level**, not only at the anchor (QA-6.2); the policy serving predicate rides at the same levels (P-0015's per-channel placement, spec R-0025-g). **Two distinct applications, both required (r2 fold):** the per-level predicate bounds the **reachable-node set** — a restricted node never enters the frontier, so edges beyond it are never reached — and the caller-facing **edge projection** (`get_context_for`'s `relations` bundle) is additionally **endpoint-filtered**: an edge is returned only if the caller passes the serving predicate on both endpoint artifacts (incoming edges' source endpoints included), because the reachable-node set is not the returned edge list; a withheld edge is caller-silent (P-0015 PE-4, spec R-0025-g).
- **Bounds:** traversal depth defaults to **2** for `get_context_for`, per-call overridable up to a configured maximum (default **8**); the per-traversal latency flag bound defaults to **500 ms** (config-tunable — spec R-0034-c).
- **The D4 instrument (field set, locked):** every multi-hop traversal writes a record `{workspace_id, verb, anchor_artifact_id, depth, row_count, latency_ms, flagged: bool, flag_reason: latency | inexpressible, created_at}`. A traversal exceeding the latency bound, or a needed traversal the CTE path cannot express, is flagged with the machine-readable reason — the logged dogfood incident that fires Apache AGE adoption (P-0010 D4), mechanically detectable from stored data. **P-0010 D5's second-adapter re-open rides the same record** (if D4 fires, D5 re-evaluates at the same time).

### ES-6 — Manifest ownership: one table, two writers, origin is the discriminator

The edge table remains **manifest-declared under the `repos` plugin's content family** (the `0.8.0` substrate it extends) — no re-homing, no second table. Two writers operate on it, discriminated by `origin` and bounded by ES-3/ES-4:

- **The `repos` plugin's CRUD path** (host-mediated, via the typed content interface) writes work-graph rows and declared relations — `origin ∈ {declared, system}` — transactionally with their entities.
- **The host-side extractor** writes through the host's storage layer (the same `Storage`-trait surface, under system actor attribution in `created_by`), confined to `origin = extracted` rows and the ES-3 no-op/refresh semantics.

Schema/migration ownership follows the declaring family; the host extractor is a bounded second writer, not a second owner — it cannot mint authority (ES-4) and cannot touch work-graph rows. *(Anchors: P-0002 — cross-plugin aggregation is a projection concern; the extractor is host code by the same discriminator that host-places retrieval (P-0014 RA-4); P-0006 — both writers pass `WorkspaceCtx` structurally.)*

### ES-7 — What this schema deliberately does not carry

- **No edge weights, no confidence scores** — the discriminators are closed enums, not scalars; a "confidence" column without a validator is a prose judgment in disguise (validatability lens). If ranking ever needs edge-strength evidence, that is a run-record analysis first, an amendment second.
- **No dangling-reference rows** — unresolved citations live in the coverage record (ES-4), not as half-edges.
- **No cross-workspace edges** — structurally impossible (`workspace_id` on the row; both endpoints resolve within the workspace).

### Consequences

**Good:**
- "What relates to X" has one answer surface: one table, one traversal, one extraction path; `supersedes` reads consistently wherever trust consumes it.
- Provenance is first-class: every edge knows how it came to exist, and authority is monotone (never silently downgraded/upgraded outside the recorded-write path).
- The D4/D5 deferral instruments are now real stored signals on the exact path they guard.
- Coverage measurement makes extraction honesty checkable per run (SC5), not asserted.

**Bad / Trade-offs:**
- The superset table couples two lifecycles (system-of-record work-graph rows and re-derivable citation rows) in one relation — accepted for the single-path win; `edge_class` + ES-6's writer bounds keep the lifecycles from bleeding.
- Origin-upgrade semantics add write-path logic a two-table design would not need — the price of one-row-per-logical-edge.
- Depth/latency defaults (2/8 hops, 500 ms) are eval-calibrated guesses at V0 — config-tunable; the instrument they gate is the thing that will correct them.

## Pros and Cons of the Options

### One superset table with discriminating columns (chosen)

- Pro: one vocabulary, one traversal, one extraction path (P-MinBlastRadius); `supersedes` unified where trust reads it.
- Pro: extends the existing `0.8.0` substrate — no new store, no migration of prior work-graph rows into a new shape.
- Con: dual-writer discipline needed (ES-6) and upgrade semantics on collision (ES-3).

### Two edge tables / parallel vocabularies

- Pro: each lifecycle owns its table; no collision semantics.
- Con: two traversal paths, two extraction targets, `supersedes` split across stores exactly where the trust predicate reads it; permanent reconciliation burden. (Stage 1c F6 resolved against it; ratified.)

### Graph engine

- Pro: expressive traversal (openCypher), deep-graph performance.
- Con: violates the A1-clean lock for a shallow graph CTEs serve; AGE's maturity cost paid without the named need. D4's trip-wire — now a real instrument (ES-5) — owns the upgrade.

## More Information

- Binding requirement text: [spec R-0027, R-0034-c, R-0035](../../specs/2026-07-02-retrieval-cluster.md); observable measures QA-1/QA-6/QA-8 ([Frame §7](../../intent/retrieval-cluster-frame.md)).
- Companions: [P-0015](P-0015-provenance-envelope-source-roles.md) (the trust predicates that consume `origin`; the recorded-write discipline ES-3's upgrades ride); [P-0014](P-0014-retrieval-architecture.md) (the traversal engine and D6 borrow 4 — graph model, not engine).
- Substrate: [P-0010](P-0010-storage-substrate-engine.md) D4/D5 (the carried deferral instruments this schema places); [P-0006](P-0006-v0-tenant-enforcement.md) (tenancy threading); [P-0002](P-0002-core-plugin-partition.md) (the partition ES-6 applies).
- Upstream locks: [Frame](../../intent/retrieval-cluster-frame.md) R4 + §5 write-authorization surface (blob `65b1d05`); [intake](../../intent/retrieval-cluster.md) SC5 + the edge-vocabulary open item the Frame resolved.
