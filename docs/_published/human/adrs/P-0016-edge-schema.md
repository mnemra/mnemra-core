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

This decision was accepted on 2026-07-02 at the retrieval cluster's spec-exit gate, the human checkpoint that closes [Spec](../glossary.md#spec), Stage 3 of the project's work-shaping pipeline. It was reviewed together with the spec itself: [2026-07-02-retrieval-cluster](../../specs/2026-07-02-retrieval-cluster.md).

It was authored during that Spec stage, and it resolves a placeholder that the [Frame](../glossary.md#frame) document, Stage 2's output, left open and marked `{{P-0016}}`. See [placeholder-resolution](placeholder-resolution.md) for how that resolution works. This is an [ADR](../glossary.md#adr): a record of the decision, the context behind it, and the alternatives it ruled out.

The core direction, one superset table instead of two separate ones, isn't new here. It's Frame direction 1 from Stage 2a (Frame's early elicitation phase), and the decomposer (the role that captures and locks intent during [Intake](../glossary.md#intake), Stage 1) already ratified it. The spec-exit gate didn't reopen that direction. What this ADR locks is what the Frame left open: the schema, the classing rules, extraction, traversal, and ownership.

## Context and Problem Statement

The corpus already has a graph inside it, but that graph is latent: it isn't queryable yet. It lives in frontmatter relation lists, in free-text citations, in prose. The retrieval cluster's job is to make that graph traversable (intake SC5).

Two edge vocabularies exist ahead of this decision. The first is the V0 (initial-release) `0.8.0` work-graph set: `parent`, `blocks`, `depends-on`, `supersedes`, `dispatched-by`. These are system-of-record rows, created at the same time as the entities they describe. The second is the retrieval citation set: `extends`, `feeds`, `cites`, `supersedes`. These are re-derivable: they can be reconstructed from the authored text itself.

The two sets overlap on exactly one type, `supersedes`, and that overlap matters because `supersedes` is trust-affecting. [P-0015](P-0015-provenance-envelope-source-roles.md)'s `outdated` predicate reads it directly, so where a `supersedes` edge came from isn't bookkeeping. It's an authority question. The Frame's section 5 write-authorization scenario makes the stakes concrete: adversarially-shaped free text must never be able to demote a target artifact's trust just because the extractor picked it up.

The Frame already locked the direction: one substrate, with a mechanism that discriminates between edge kinds. This ADR fills in the schema that mechanism runs on.

## Decision Drivers

- **One vocabulary, one traversal, one extraction path** ([P-MinBlastRadius](../glossary.md#p-minblastradius): keep a change's reach as small as the architecture allows, ideally isolated to one module. Stage 1c's finding F6, from Intake's review pass, already preferred this resolution). Parallel edge stores would mean two query paths for "what relates to X," and a permanent reconciliation burden.
- **Provenance discrimination is load-bearing** (P-0015 PE-7): a trust predicate that can't distinguish a declared supersession from an extracted one is an authority-laundering vector.
- **The substrate is locked** (P-0010 D4): shallow edge model, recursive CTEs, no graph engine. The traversal path carries D4's instrument (the record described in ES-5 below).
- **Extraction must be honest** (intake SC5): coverage over the migrated corpus is measured and reported, never assumed.
- **Tenancy and identity are structural from day one** (P-0006; retrofit-prep walk item 11).

## Considered Options

1. **One superset table extending `0.8.0`, discriminated by `edge_class` and `origin` columns (chosen).**
2. **Parallel vocabularies, or two edge tables** (a work-graph table plus a citation table). Rejected at Stage 1c (finding F6): the intake document carried this composition question forward to the Frame, which ratified the superset approach instead. Two tables would mean two traversal code paths, two extraction targets, and `supersedes` split across stores at exactly the point where trust reads it.
3. **A graph engine for the citation graph.** Rejected upstream, at Intake, as a hard constraint (P-0010 D4): recursive CTEs already serve the shallow graph model this project uses. Apache AGE stays parked behind D4's strain trip-wire, and this cluster's instrument (ES-5, below) is what will make that trip-wire fireable.

## Decision Outcome

**Chosen: Option 1**, rendered below as ES-1 through ES-7. The binding requirement text lives in the spec: [R-0027, R-0034-c, R-0035](../../specs/2026-07-02-retrieval-cluster.md) ([R-codes](../glossary.md#r-codes) are stable identifiers for specific requirement entries in that document).

### ES-1 — One superset table, closed vocabulary

One edge table, the `0.8.0` relationships substrate extended, carries the closed vocabulary **`{parent, blocks, depends-on, dispatched-by, extends, feeds, cites, supersedes}`**. `supersedes` is what unifies the two prior sets. An edge type outside this enum is rejected at write time.

Vocabulary changes require an amendment to this ADR. The vocabulary is closed at the schema level but extensible at the ADR tier (the same discipline P-0015's mechanic 6 applies elsewhere, here applied to edge types). Exactly one traversal code path and one extraction code path operate over the table.

### ES-2 — Discriminating columns + the `supersedes` classing rule

Every row carries two closed-enum columns.

- **`edge_class`** is one of `{work-graph, citation}`. `parent`, `blocks`, `depends-on`, and `dispatched-by` are always `work-graph`. `extends`, `feeds`, and `cites` are always `citation`. `supersedes` is classed by who wrote it: it's `work-graph` only if it was created by the work-graph system-of-record transactional path (entity lifecycle operations, such as task or dispatch supersession, written atomically with the entities they describe), and `citation` otherwise (declared in content frontmatter, or extracted from free text). The rule is mechanical at write time: the writing path decides the class, not a judgment call.
- **`origin`** is one of `{declared, extracted, system}`. `declared` means authored in structured frontmatter or a database relation. `extracted` means derived from free text by the extractor, and it always carries a source-span provenance pointer back to the text it was read from. `system` means created by system operations, such as `dispatched-by`. The authority order for anything that consumes trust is `system` > `declared` > `extracted`, and per P-0015 PE-7, an `extracted` edge never enters a trust predicate at V0 (the initial release).

The `superseded-by` forward pointer is a name for a directional read of the `supersedes` edge, not a schema field of its own (this was folded away in revision r1). Retrieval's hard-supersession condition (spec R-0029-e; P-0015 PE-7) is that the artifact has an incoming `supersedes` edge with origin `declared` or `system`. "Forward pointer" is the name for reading that edge from the superseded artifact's side. There is no separate `superseded_by` column, no normalized topic key, and no second supersession mechanism: one source of truth is where trust reads, per P-MinBlastRadius, and a topic-key deriver would put an inference inside a trust path, which spec R-0026-a bars outright. An `origin = extracted` `supersedes` edge is not a forward pointer. It neither excludes nor demotes anything (that's PE-7's origin weighting at work).

Work-graph rows are system-of-record: they're created transactionally with their entities. Citation rows are re-derivable.

### ES-3 — Uniqueness + origin-authority upgrade

There is one logical edge per `(workspace_id, src_artifact_id, dst_artifact_id, edge_type)`. A single row carries each logical edge, so trust predicates and traversal always read from one source of truth (P-MinBlastRadius). On collision, origin decides what happens:

- An extractor write that finds an existing row at equal or higher authority (`declared`, `system`, or an existing `extracted` row) is a no-op. Idempotent re-extraction only refreshes `source_span`.
- A `declared` or `system` write that finds an existing `extracted` row upgrades that row's origin. This is a recorded write, since the row's provenance changed, and for `supersedes` that change is trust-affecting, so it follows the P-0015 PE-6 recorded-write discipline.
- Nothing ever downgrades origin authority. Deleting a `declared` relation from frontmatter removes the row on re-extraction only if no higher-authority writer owns it.

Extracted rows carry `source_span` (the source artifact or path, plus the span the citation was read from), required only when `origin = extracted`. Every row carries `workspace_id NOT NULL` (indexed, explicitly passed) and `owner`/`created_by` (defaulted at V0, since V0 is single-user), per spec R-0035.

### ES-4 — Extraction contract: sources, idempotency, measured coverage, integrity

- **Sources:** frontmatter relation lists produce rows with `origin = declared`. Free-text citations, meaning markdown links to in-corpus artifacts, decision-record identifiers (`P-NNNN`/`G-NNNN`), requirement IDs, and in-corpus path citations, produce rows with `origin = extracted` plus `source_span`.
- **Idempotency:** re-extraction over an unchanged corpus produces byte-identical edge rows, using ES-3's upsert semantics. Extraction re-fires on document change as part of the index pipeline.
- **Coverage is measured, never assumed (intake SC5).** Every index run records, per source class, how many frontmatter relations and free-text citations resolved to edges versus failed to resolve. Unresolved citations are counted with their spans in the build record; they're not written as dangling edge rows. The count runs against the real migrated corpus, not a fixture.
- **Extraction integrity (together with P-0015):** the extractor writes only `origin = extracted` rows. It can never author `declared` or `system` authority. Promoting an extracted, trust-affecting edge to `declared` happens only through the admin-gated recorded-write path (P-0015 PE-6/PE-7). When register `1.2.0` (ongoing ingest) opens an untrusted-submitter path into frontmatter, the weight given to `declared` authority reopens as a question, and the trip-wire named in P-0015 PE-7 is what carries that reopening.

### ES-5 — Traversal contract + the D4 instrument's field set

- **Shape:** graph traversal is a recursive CTE (common table expression) over the edge table. P-0010 D4 already settled this: borrow the graph *model*, not a graph *engine*. This is what powers `get_context_for`'s graph position: typed relations, lifecycle state, and linked-artifact summaries.
- **Tenancy and policy:** the workspace predicate appears at every recursion level, not just at the anchor (QA-6.2), and the policy-serving predicate rides at those same levels (P-0015's per-channel placement, spec R-0025-g). There are two distinct applications here, and both are required (this was folded together in revision r2). The per-level predicate bounds the reachable-node set: a restricted node never enters the traversal frontier, so edges beyond it are never reached at all. Separately, the caller-facing edge projection, meaning `get_context_for`'s `relations` bundle, is endpoint-filtered on top of that: an edge is returned only if the caller passes the serving predicate on both endpoint artifacts, including the source endpoints of incoming edges. That's necessary because the reachable-node set isn't the same thing as the returned edge list. A withheld edge fails silently from the caller's point of view (P-0015 PE-4, spec R-0025-g).
- **Bounds:** traversal depth defaults to 2 for `get_context_for`, and it's overridable per call up to a configured maximum (default 8). The per-traversal latency flag bound defaults to 500 ms, and it's config-tunable (spec R-0034-c).
- **The D4 instrument, with its field set locked:** every multi-hop traversal writes a record: `{workspace_id, verb, anchor_artifact_id, depth, row_count, latency_ms, flagged: bool, flag_reason: latency | inexpressible, created_at}`. A traversal that exceeds the latency bound, or a needed traversal the CTE path can't express, gets flagged with a machine-readable reason. That's the logged incident, encountered through the project's own use of the system, that would trigger adoption of Apache AGE (P-0010 D4), and it's mechanically detectable straight from stored data. P-0010 D5's decision to reopen the question of a second storage adapter rides the same record: if D4 fires, D5 gets re-evaluated at the same time.

### ES-6 — Manifest ownership: one table, two writers, origin is the discriminator

The edge table stays manifest-declared under the `repos` plugin's content family, the same family that owns the `0.8.0` substrate it extends. There's no re-homing and no second table. Two writers operate on it, discriminated by `origin` and bounded by the rules in ES-3 and ES-4:

- **The `repos` plugin's CRUD path** (host-mediated, through the typed content interface) writes work-graph rows and declared relations, meaning `origin ∈ {declared, system}`, transactionally with their entities.
- **The host-side extractor** writes through the host's storage layer (the same `Storage`-trait surface, under system-actor attribution in `created_by`), confined to `origin = extracted` rows and the no-op/refresh semantics from ES-3.

Schema and migration ownership follows the declaring family. The host extractor is a bounded second writer, not a second owner: it can't mint authority (ES-4) and can't touch work-graph rows. A few things anchor this. Cross-plugin aggregation is a projection concern (P-0002). The extractor counts as host code, by the same discriminator that places retrieval on the host side (P-0014 RA-4). And both writers pass `WorkspaceCtx` structurally (P-0006).

### ES-7 — What this schema deliberately does not carry

- **No edge weights, no confidence scores.** The discriminators here are closed enums, not scalars. A "confidence" column without a validator is really just a prose judgment wearing a number (the validatability lens catches this). If ranking ever needs edge-strength evidence, that's a run-record analysis first, and an ADR amendment second.
- **No dangling-reference rows.** Unresolved citations live in the coverage record (ES-4), not as half-edges sitting in the table.
- **No cross-workspace edges.** This is structurally impossible: `workspace_id` sits on the row, and both endpoints have to resolve within that same workspace.

### Consequences

**Good:**
- "What relates to X" has exactly one answer surface: one table, one traversal, one extraction path. `supersedes` reads consistently wherever trust consumes it.
- Provenance is first-class. Every edge knows how it came to exist, and authority only ever moves one direction: nothing gets silently downgraded or upgraded outside the recorded-write path.
- The D4 and D5 deferral instruments (the trip-wire mechanisms from P-0010) are now real stored signals sitting on the exact path they're meant to guard.
- Coverage measurement makes extraction honesty checkable on every run (SC5), instead of just asserted.

**Bad / Trade-offs:**
- The superset table couples two different lifecycles, system-of-record work-graph rows and re-derivable citation rows, into one relation. That's accepted in exchange for the single-path win, and `edge_class` plus ES-6's writer bounds keep the two lifecycles from bleeding into each other.
- Origin-upgrade semantics add write-path logic that a two-table design wouldn't need. That's the price of keeping one row per logical edge.
- The depth and latency defaults (2 and 8 hops, 500 ms) are eval-calibrated guesses at V0. They're config-tunable, and the instrument that gates them is exactly what will correct them over time.

## Pros and Cons of the Options

### One superset table with discriminating columns (chosen)

- Pro: one vocabulary, one traversal, one extraction path (P-MinBlastRadius). `supersedes` is unified exactly where trust reads it.
- Pro: extends the existing `0.8.0` substrate. No new store, no migrating prior work-graph rows into a new shape.
- Con: needs dual-writer discipline (ES-6) and upgrade semantics on collision (ES-3).

### Two edge tables / parallel vocabularies

- Pro: each lifecycle owns its own table, so there's no collision semantics to design.
- Con: two traversal paths, two extraction targets, and `supersedes` split across stores exactly where the trust predicate reads it. That's a permanent reconciliation burden. Stage 1c's finding F6 resolved against this option, and the Frame ratified that call.

### Graph engine

- Pro: expressive traversal (openCypher), and strong performance on deep graphs.
- Con: violates the A1-clean lock that a shallow graph, already well served by CTEs, doesn't need. It would mean paying Apache AGE's maturity cost without a demonstrated need for it. D4's trip-wire, now a real instrument via ES-5, is what owns the decision to upgrade later.

## More Information

- Binding requirement text: [spec R-0027, R-0034-c, R-0035](../../specs/2026-07-02-retrieval-cluster.md); observable measures QA-1/QA-6/QA-8 ([Frame section 7](../../intent/retrieval-cluster-frame.md)).
- Companions: [P-0015](P-0015-provenance-envelope-source-roles.md), which defines the trust predicates that consume `origin` and the recorded-write discipline that ES-3's upgrades follow; [P-0014](P-0014-retrieval-architecture.md), which covers the traversal engine, where D6 borrows the same idea as D4: graph model, not graph engine.
- Substrate: [P-0010](P-0010-storage-substrate-engine.md) D4/D5, the carried deferral instruments this schema puts in place; [P-0006](P-0006-v0-tenant-enforcement.md), tenancy threading; [P-0002](P-0002-core-plugin-partition.md), the partition ES-6 applies.
- Upstream locks: [Frame](../../intent/retrieval-cluster-frame.md) R4 and the section 5 write-authorization surface (blob `65b1d05`); [intake](../../intent/retrieval-cluster.md) SC5 and the edge-vocabulary open item the Frame resolved.
