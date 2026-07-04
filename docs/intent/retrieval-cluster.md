---
spec_type: architecture
frame_relevant: true
---

# Intake: retrieval cluster — get_context_for + provenance envelope + search activation + typed edges + source-roles

**Locked: 2026-07-02 · Amendment A1 locked: 2026-07-04**

**Stakes:** high
**Date:** 2026-07-02
**Status:** locked (intake-exit gate confirmed 2026-07-02; Stage 1c review: Warden round 1, zero blocker/high, six findings resolved, none dismissed). Amendment A1 locked 2026-07-04 — see the A1 section's status line for its gate record.
**Consumer:** agents — MCP-client coding agents consuming the retrieval verbs at runtime; the design and verification pipeline consuming the resulting spec.

> `spec_type: architecture` ratified by the maintainer 2026-07-02. The heuristic was
> ambiguous between `{code, architecture}`; `architecture` chosen because the spec locks
> architectural surface (ABI inference primitives, envelope contract, edge schema),
> matching the signing-to-runnable precedent.

## JTBD

An agent picking up an artifact (a task, a spec, a research brief, a decision record) sits inside a graph — the work it extends, the decisions it feeds, its lifecycle state, prior related sessions. Today that graph is authored but **latent**: edges live in frontmatter relation lists, description free-text, and prose ("extends X"), so every session re-derives context by hand — multiple reads and tool calls per artifact — or risks acting on a stale, incomplete picture. The maintainer needs agents to get **focused, trust-labeled context for a given artifact in one call**: its linked prior work, its graph position, its lifecycle state, with every returned item carrying provenance (how authoritative, how fresh, whether usable at all), shaped to fit the consuming agent's context budget rather than enlarging it.

This is the product's core promise (register entry `1.1.0`, tier `proposed`): the first net-new value over V0's workspace-fidelity baseline.

**Register mapping:** this intake covers four register entries across two tiers — `1.1.0` `get_context_for` (`proposed`), D1 search + indexing activation (`idea`), D2 first-class graph edges + traversal (`idea`), and the G2/G3 cross-artifact authoritativeness + provenance/use-policy substrate fields (`idea`). Locking this intake performs the `idea → proposed` promotion for D1, D2, and G2/G3 as parts of one clustered feature; the register update rides this cluster's docs PR.

## Non-goals

- **No ongoing ingest pipeline** (register `1.2.0`). The cluster serves whatever the substrate already holds (batch-migrated corpus + structured rows); continuous arrival is its own future feature.
- **No memory compaction / consolidation.** Sibling future capability, separately designed.
- **No retrieval eval harness build-out.** The spec names each observable narrowing signal; constructing the eval that reads those signals is separate work.
- **No narrowing decisions on the LLM placements.** Every placement ships with its signal; narrowing is an eval-time, post-live decision per the maintainer's "all of the above for V0, narrow later" ruling (2026-05-27). Firing mechanism (so this deferral is not silently parked): the code-aware retrieval eval — the threshold-setter the retrieval research names — reads the shipped signals; creating that eval is a named deliverable of the committed-tier plan, and until it runs, all placements stay on.
- **No separate vector database, graph database, or search engine.** Locked by the retrieval research: Postgres + pgvector + built-in FTS + an edge table cover dense, sparse, and graph retrieval.
- **No multi-tenant policy-enforcement expansion.** New tables preserve structural workspace scoping; the enforcement posture is unchanged (P-0006, P-0009).
- **No committed-tier content.** No tasks section, no release binding — the output parks at designed tier (locked frame + locked spec).

## Success criteria

Each is an observable outcome a downstream check could verify.

1. **One-call context bundle.** `get_context_for(artifact_id)` returns the artifact's typed relations (extends / feeds / supersedes / cites, composing with the existing edge vocabulary), its lifecycle state, and linked-artifact summaries in one MCP call. The reference trace (two in-progress research tasks, 2026-06-05) reproduces as one call each, replacing ≈4 tool calls + 3 file reads per task.
2. **Provenance envelope on every item.** Each returned context item carries a source-role from a closed vocabulary (candidate set: authoritative / background / outdated / sensitive / dont-use), a freshness signal (version-handle where one exists; decay-class fallback otherwise), and a stable citation back to the source artifact.
3. **Disclosure budget enforced.** Retrieval responses respect a token budget (default 8k, per-call override). Over-budget candidate sets are reduced deliberately (rerank-and-trim, coarser hierarchy nodes) — never silently truncated.
4. **Hybrid search activated.** Search over the indexed corpus runs lexical (Postgres FTS) and dense (pgvector) channels fused by Reciprocal Rank Fusion; exact-key queries (decision IDs, file paths, function names) resolve via the lexical channel — the query class pure-vector retrieval loses.
5. **Typed edges traversable.** Edges are extracted from the existing authored-but-latent sources (frontmatter relation lists, free-text citations) into the edge substrate; extraction coverage over the migrated corpus is measured and reported, not assumed.
6. **Narrowing signals shipped.** Each LLM placement in the retrieval path (index-time chunk-context, query-time rewrite, optional synthesis, tag generation) exposes the A/B-able signal that will decide its keep-or-narrow outcome.
7. **Base pinned.** The spec records its substrate base (main@4e852a1 + assumed-shipped surfaces) so staleness is detectable at the designed → committed transition.

## Hard constraints

- **Substrate:** single-process Postgres + pgvector (P-0010). Full-text search via built-in `tsvector`/`ts_rank` first, with a named upgrade path if it proves inadequate; graph via an edge table joined by artifact id. No new storage engine.
- **Plugin model:** WebAssembly Component Model, IO-free cores (P-0002, P-0012, P-0013). All model inference is reached through host-provided functions; inference implementations live host-side, never inside a plugin.
- **Surface:** MCP-native verbs on the existing stdio server (product hard constraint); new verbs extend that server.
- **Progressive disclosure is a hard constraint,** not a preference — the context-rot finding is load-bearing physics. The system shapes context to a budget; "return more tokens" is not an acceptable failure mode.
- **Dependency tier:** recommended defaults are Green (MIT / Apache-2.0 — BGE-M3, BGE-reranker-v2-m3, fastembed-rs); cloud-LLM use is operational cost under the standing API-key approval. No Yellow or Red dependency without an explicit named trigger.
- **Rust-first; no vendored ports.** External reference implementations (RAPTOR and kin) are design references reimplemented against the substrate.
- **Tenancy invariant:** any new table carries structural workspace scoping (NOT NULL, indexed, explicitly passed).
- **Validator-gated vocabulary:** a source-role earns a slot in the envelope only with a mechanical validator (the validatability lens) — no prose-judgement roles.
- **Carried deferral context (P-0010):** this cluster is the named firing event for P-0010's deferred retrieval method-borrows (D6), and the graph-traversal trip-wires (D4/D5) sit on the path this cluster activates. The Frame walk consumes those deferral records explicitly rather than re-deriving them.
- **Model-hosting boundary (RC-1, resolved 2026-07-02):** the system MUST NOT host a **generative** LLM — all generative work (query rewrite, chunk-context, tag generation, synthesis) calls out to an external model at V0. Local **non-generative** inference (embedding, reranking — small encoder models via the host-fn seam) is permitted host-side. This amends the product brief's model-hosting constraint; **every falsified canonical copy reconciles in the same PR** (single-source discipline): the brief's Non-goals clause ("Embeddings and summaries call out to an external model; the system never hosts one"), its Hard-constraints clause ("The system MUST NOT host a language model; it calls out to an external one"), and the `0.1.0` substrate entry's external-embedding framing — all landing as labeled MODIFIED deltas per the brief's format contract, authored with the Stage 3 spec. The architecture-overview ELT subsystem's external-embedding framing (maintainer-internal record) is a named lagging copy, reconciled at the same stage.

## Evidence

- **Live trace (2026-06-05 — captured, not hypothetical).** An orchestration session picking up two in-progress research tasks re-derived their true state by hand (task row → free-text path citation → frontmatter relations → prose) at ≈4 tool calls + 3 reads per task, and **near-fired a redundant research dispatch measured at 1–4M tokens** against the same session's comparable runs. The verb collapses that reconstruction to one call. (Use-case record: get_context_for, 2026-06-05.)
- **Locked retrieval research (2026-05-27, revision 3).** Progressive-disclosure RAG over the heterogeneous corpus: per-shape chunking, hierarchical index, over-retrieve-rerank-into-budget disclosure, iterative and citation-expansion access patterns, four model slots with Green local defaults, and four LLM placements each with a named narrowing signal.
- **Independent cross-check (2026-06-05, canon-free).** Corroborates hierarchical / collapsed-tree retrieval and hybrid BM25 + dense + RRF-in-Postgres. Flags the research's "use authored hierarchy, skip clustering" recommendation (its open question 1) as an **unvalidated assumption to eval, not a settled lock**; confirms synthesis-narrowing, budget sizing, and tag vocabulary as genuine novel-design decisions.
- **Sources-with-roles finding (maintainer record, 2026-05-21).** Source-roles as a context contract a context layer serves to agents, extended by two maintainer advances: freshness (version-handle diff as the primary staleness check; decay-class TTL as the fallback that bounds ignorance; override structurally required) and displacement (named trip-wire events fire re-evaluation; single-axis displacement triggers re-eval, never auto-invalidation; decisions record the axes they were made on).
- **Substrate findings (2026-05-15 knowledge-object survey).** Cross-artifact authoritativeness (G2) and provenance / use-policy (G3) are substrate concerns — the fields this cluster's envelope reads and serves.
- **No exploratory items.** Source-roles enters as a firm commitment to *define and lock* the envelope contract with a validator-gated vocabulary; evaluating which roles earn slots is Frame work against that commitment, not an open hunch.

## Risk profile

**May touch trust boundary — flagged; carried to Frame as a required constraint.** Three surfaces: (a) retrieval reads span workspace-scoped tables and serve content outward through MCP — the `sensitive` / `dont-use` role semantics are themselves a use-policy control, and a retrieval path that ignores them leaks by design; (b) the generative LLM placements egress content to an external API at **index time** (chunk-context, tag-gen: document content) **and at query time** (HyDE rewrite: query text; synthesis: retrieved content) — the Frame SHALL state the egress boundary per placement and reconcile it with the product's self-hostable promise ("data never leaving that boundary"), including the local-migration posture that preserves it (required Frame constraint, not advisory); (c) local-model hosting (RC-1) changes the deployment surface. The security-mode review fires at Frame, where the mechanisms are known.

## Open decisions (all resolved by the maintainer, 2026-07-02)

- **RC-1 — resolved: amend to "no generative LLM."** The conflict: the product brief locks "calls out to an external model; never hosts one," while the locked retrieval research recommends local BGE-M3 embedding + BGE-reranker in the host process (zero marginal cost, zero egress, lower latency). Rejected option preserved: holding the constraint literally would force a proprietary (Red-tier) embedding API as a dependency — no Green-tier cloud embedding path exists, and the maintainer's cloud instinct (Haiku call-out) is already the V0 default for every *generative* slot. Decision basis: encoder memory footprint verified trivial on the dev machine (~1.5–2.5GB warm on 24GB); the host-fn seam keeps model choice swappable. The product-brief amendment ("MUST NOT host a *generative* LLM; local non-generative inference permitted host-side") rides this cluster's docs PR.
- **RC-2 — resolved: full cluster.** Search activation (register idea D1: indexing pipeline over the existing corpus + agent-facing search verb(s)) is in scope alongside `get_context_for`; the end state is a fully working retrieval feature. Staging/sequencing is a committed-tier plan concern, deliberately out of this designed-tier artifact.
- **RC-3 — resolved: all four placements.** Chunk-context, query rewrite, synthesis, and tag-gen all ship as specified surfaces with narrowing signals, per the maintainer's standing "all of the above for V0, narrow later" ruling (2026-05-27). Synthesis is marked optional-in-ABI per the research's narrowing anticipation (its open question 3).
- **spec_type — ratified: `architecture`.** Heuristic was ambiguous ({code, architecture}); `architecture` chosen because the spec locks architectural surface (ABI inference primitives, envelope contract, edge schema), matching the signing-to-runnable precedent.

## Open items carried to Frame

Firm-to-defer: committed work whose design choice the Frame constraint-walk resolves — not unresolved intake questions.

- **Edge-vocabulary composition:** the cluster's typed edges (extends / feeds / cites / supersedes) overlap the V0 `0.8.0` relationship vocabulary (parent / blocks / depends-on / supersedes / dispatched-by) only on `supersedes`. The Frame decides: one edge substrate with a superset vocabulary, or parallel vocabularies with distinct semantics. The intake commits only to typed, traversable edges.
- **Generative-egress reconciliation:** per-placement egress boundary vs the self-hostable sovereignty promise — required Frame constraint (see Risk profile).

## Consultations

- _none at intake — scope, the four RC resolutions, and the model-hosting boundary were locked directly with the maintainer during elicitation (2026-07-02); the Stage 1c review pass was Warden (round 1, zero blocker/high, six findings folded in)._

## Dismissed review flags

- _none — all six round-1 findings (F1–F6) were resolved by edits; none dismissed._

## Amendment A1 (2026-07-04) — ANIMUS anti-phantom instruments

**Status:** locked 2026-07-04 (A1 intake-exit gate confirmed by the maintainer 2026-07-04. Stage 1c review: Warden r1 approve-with-conditions — 1 High + 1 Low + 1 Nit folded in-loop, r2 delta re-verify verified-clean 3/3 folds, zero new findings; the 1 Medium — a pre-existing cross-spec R-NNNN numbering overlap, not an A1 defect — was escalated at the gate and dispositioned defer-with-named-trigger, task #2126, firing at this cluster's designed→committed pickup.)
**Authorization:** task #2104. Maintainer-ratified decision walk 2026-07-04 (task #2102, all four OQs decided) — decision record in `brain/projects/mnemra/2026-07-04-animus-v4-applicability-research.md` §Decision record (brief locked 2026-07-04, Ink terminal review r1+r2 complete). Re-opening this locked intake is the conscious unlock that record authorizes; scope is strictly the additions below. The original sections above are untouched — A1 is additive.

### JTBD (A1)

The locked cluster's honesty instruments *measure* but do not *enforce*: ES-4/R-0034-d record coverage and population counts per index run, and the P-0015 admission gate is specified per write path, but nothing structurally fails when a measurement diverges or a write path bypasses the gate. The ANIMUS v4.0 primary source demonstrates the failure class this leaves open — a system whose specified guarantee (dedup, honesty filter) was silently narrowed by one unguarded write path, inflating its reported metrics 2× until an external audit caught it. The maintainer needs the retrieval cluster's integrity guarantees to be **self-enforcing at build and test time, before implementation starts**, so a specified-but-unrealized guarantee (a phantom) cannot survive an index build or a test run.

### Success criteria (A1 additions — SC-8 to SC-11)

Each is an observable outcome a downstream check could verify; numbering continues from SC-7.

8. **Build-failing integrity gate (ratified OQ2).** The index build fails — not logs — when either assertion of the pair diverges: (a) reported distinct-key count == live distinct-key count; (b) provenance/coverage population counted against **live rows** (not "the migration ran"). ES-4's existing measurement becomes an assertion pair. *Observable: a seeded divergence (fixture corpus with a mismatched count) causes a non-zero index-build exit.*
9. **Write-path admission-gate enumeration test (ratified OQ3, spec-tier AC).** A conformance test enumerates every write path to the content/edge store and asserts each passes the P-0015 admission gate + envelope validators; adding a write path that bypasses the gate makes the test fail. In-family precedent: the reporting-engine R-0044 reconciliation pattern (locked 2026-07-04; the per-table matrix itself lives at its R-0050, referenced from R-0044). *Observable: a seeded bypassing write path turns the test red.*
10. **Content-addressed idempotent ingest (ratified OQ1-b).** Every ingest path this cluster owns computes a SHA-256 content address and is idempotent on it: re-ingesting byte-identical content is a no-op (no duplicate row, no spurious new version). The requirement is forward-binding on register `1.2.0` (ongoing ingest) when that feature is built. *Observable: double-ingest of identical content leaves row counts unchanged.*
11. **Named near-dup tripwire (ratified OQ1-a).** The near-dup rate on `origin = extracted` edges and chunk-grain retrieval is instrumented, riding the ES-4/R-0034-d measurement surface; the spec names the instrument, the measured rate, and a **numeric threshold** whose crossing re-opens the write-time-dedup question (OQ1). Firing is a threshold crossing in stored data — not prose review. *Observable: the per-run build record carries the near-dup measure; a fixture run seeded above threshold produces the fired flag.*

### Non-goals (A1 boundary)

- **The no-ongoing-ingest non-goal stands.** SC-10 constrains the write/ingest paths this cluster already owns (batch corpus load, host-side extractor re-runs) and binds forward onto `1.2.0`; it does not pull the ingest pipeline into scope.
- **No write-time semantic dedup.** Ratified OQ1 keeps the current bet (exact-key PK + explicit supersession + retrieval-time consolidation). The tripwire instruments the bet; it does not hedge it into write-time dedup.
- **No reshaping of the reporting-engine consumed contract.** R-0025, R-0026, and R-0025-g are pinned by the reporting-engine BOM (`0b948ea2`); A1 does not alter their shape (see Hard constraints).

### Hard constraints (A1 additions)

- **Consumed-contract stability:** the amendment SHALL NOT reshape R-0025 / R-0026 / R-0025-g (the reporting-engine BOM `[consumed_contracts.retrieval-cluster]` pin scope). This keeps the ride-along re-pin mechanical (no D4 re-derivation). If a review finding forces reshaping any pinned R-ID, pause-and-escalate — that converts the ride-along into a reporting-engine re-derivation.
- **Tripwire completeness:** SC-11 lands only with instrument + measured rate + numeric threshold + named re-open action, per the standing firing-mechanism discipline. A tripwire without a firing mechanism is prose intent and does not pass the gate.
- **Additive amendment:** no locked requirement is removed or weakened. New requirements SHOULD slot as lettered sub-items of this spec's existing R-IDs (no new number allocation) — but never under the pinned IDs (R-0025 / R-0026, incl. R-0025-g). If a new number is unavoidable, the spec author SHALL verify global uniqueness across every locked spec in `docs/specs/` before allocating — R-0036–R-0039 are **not** free (owned by the ci-flake-tier2 spec, which claims R-0026–R-0039 in the same global series), and the corpus already carries a pre-existing cross-spec numbering overlap (retrieval R-0023–R-0035 vs tier1 R-0020–R-0025 / tier2 R-0026–R-0039) — escalated at the A1 intake-exit gate; if its resolution renumbers, the amendment follows it.

### Evidence (A1)

- **ANIMUS v4.0 applicability brief** (`brain/projects/mnemra/2026-07-04-animus-v4-applicability-research.md`, locked 2026-07-04, Ink-reviewed, committed `daa8ac0`). Primary source: the paper's own v3.3→v4.0 confession — a dedup guarantee silently narrowed by one unguarded write path (`integrate_knowledge`), reported metrics inflated ~2×, caught only by an external line-level audit. The brief maps the class onto this cluster: every guarantee here that matches an ANIMUS theme is currently specified-only.
- **In-family workspace precedents:** the live-data-not-just-schema discipline (a writer + migration ≠ capturing; count live rows) and silent-failure-classes-structuralize-at-first-sighting (ratified 2026-07-02) — both cited in the decision record's rationale; the reporting-engine R-0044 reconciliation pattern (matrix at its R-0050) as the same pattern at its second use.

### Consumer (A1)

Unchanged — agents consuming the retrieval verbs; the design/verification pipeline consuming the amended spec. The new instruments additionally serve the implementation-time CI surface (the build and test gates are consumed by the committed-tier pipeline when implementation starts).

### Risk profile (A1)

**No new trust surface.** The additions are build-time assertions, a conformance test, ingest idempotency, and instrumentation — no new egress, no auth or policy-semantics change, no new schema authority. The original risk profile's surfaces are untouched. Standard (non-security-mode) review applies.

### Open items carried to Frame/Spec (A1)

- **Frame mapping (brownfield):** SC-8 → Frame R4/R11 (edge substrate + instrumentation home); SC-9 → Frame R3 (envelope/admission gate); SC-10 → Frame R4 (extraction/ingest contract, ES-3/ES-4 idempotency); SC-11 → Frame R11 (instrumentation) — recorded as a dated Frame addendum, no new architectural surface.
- **Near-dup threshold value:** the spec author proposes the numeric threshold + rationale (and what "near-dup" is measured against at chunk grain); maintainer ratifies at the A1 spec-exit gate.
- **SC-8(a) counter mapping:** which existing measure the distinct-key assertion extends — ES-4/R-0034-d record per-source resolved/unresolved *edge-coverage* counts, not a distinct-key count per se — the spec author confirms the mapping (or introduces the distinct-key counter as part of the gate) at spec precision. The locked OQ2 assertion-pair language stands verbatim.
- **Content-address mechanics:** key space (what content is hashed), storage location (column/constraint), and interaction with ES-3 upsert byte-idempotency — spec precision against P-0001/P-0016.
- **Write-path enumeration source:** the test's authoritative write-path list starts from ES-6's two named writers (repos-plugin CRUD path, host-side extractor) plus any batch-load path the spec names; the enumeration mechanism (how a new path is forced into the list) is spec work.
