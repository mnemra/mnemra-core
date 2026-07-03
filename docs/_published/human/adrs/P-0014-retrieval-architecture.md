---
title: "P-0014: Retrieval Architecture"
summary: "Resolves the retrieval-cluster Frame's {{P-0014}} slot. Locks the retrieval topology: per-shape chunking policy, collapsed-tree multi-resolution index behind a TreeBuilder seam (authored-first under a named eval), hybrid FTS+dense channels fused by application-side RRF with local cross-encoder rerank, three host-served MCP verbs with disclosure-budget mechanics, the degraded-mode matrix, the internal granularity posture, the instrumentation shape, and the model-config pin+verify contract. This cluster is P-0010 D6's named firing event — the four method-borrows land here and close that deferral. Records the typed-DFD extension for the cluster's new elements and the 4.1 chunk-context ingress control (with P-0015)."
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

# P-0014: Retrieval Architecture

**Project:** mnemra-core

## Status

`accepted`

This ADR was accepted on 2026-07-02, at the retrieval cluster's **spec-exit gate**. That gate is the human checkpoint closing Spec, Stage 3 of the work-shaping pipeline, where agents turn a locked frame into a testable specification. This ADR was part of the Stage-3 package the gate reviewed, alongside the spec ([2026-07-02-retrieval-cluster](../../specs/2026-07-02-retrieval-cluster.md)).

It was authored at Stage 3, resolving the `{{P-0014}}` slot left open by the Frame (Stage 2 of the pipeline, where agents walk the constraint graph from validated intent and propose operating constraints) per [placeholder-resolution](placeholder-resolution.md). The architectural directions rendered here were already ratified by the decomposer (the role that owns intent capture at Stage 1, Intake, and iterates it until it locks) when the [Frame](../../intent/retrieval-cluster-frame.md) itself locked at its frame-exit gate (verdict: Accept, 2026-07-02). The spec-exit gate didn't reopen those directions. It reviewed this document only as their rendering at spec precision.

## Context and Problem Statement

The retrieval cluster, tracked in the feature register at `1.1.0` plus D1, D2, G2, and G3, needs a locked retrieval topology: how the corpus is chunked and indexed, how the search channels combine, where the verbs live, how responses are shaped to a token budget, and how the four generative placements degrade. The locked Frame (blob `65b1d05`) directs the shape through directions R1, R2, R5, R6, R7, R8, R10, and R11 (these are numbered direction identifiers, defined in full in the Frame document itself). This ADR is the slot that locks that shape at ADR precision. It's also the designated landing site for [P-0010](P-0010-storage-substrate-engine.md)'s **D6 method-borrows**. (P-0010, like this document, is a project-scoped ADR, called a P-* ADR because it addresses a decision specific to this project rather than the whole workspace.) P-0010 named this retrieval-layer ADR, when written, as the event that would fire that deferral. The companion contracts live in [P-0015](P-0015-provenance-envelope-source-roles.md) (envelope and policy) and [P-0016](P-0016-edge-schema.md) (edge schema). The binding requirement text sits in the spec's R-0023, R-0024, and R-0028 through R-0034.

## Decision Drivers

- **One-call, trust-labeled, budget-shaped context is the product promise** (this is the job-to-be-done, JTBD, captured at Intake, Stage 1 of the pipeline where the decomposer writes and agents review; the 2026-06-05 reference trace measured the re-derivation tax at roughly 4 tool calls plus 3 reads per task). Progressive disclosure is a hard constraint: context-rot is load-bearing physics here, and "return more tokens" doesn't count as a fix.
- **The substrate is locked and A1-clean** (P-0010's decisions D1 and D2): Postgres plus pgvector behind the `Storage` trait, no new engine, no extension beyond pgvector. Retrieval has to be expressible on that stack.
- **Exact-key queries have to survive** (decision IDs, file paths, function names). Pure-vector retrieval loses this query class by mathematics, not by tuning (intake success criterion SC4).
- **The authored-hierarchy assumption is unvalidated.** The canon-free cross-check run on 2026-06-05 refuted the stronger claim that authored hierarchy beats clustered hierarchy outright. So the design has to preserve the losing arm behind a seam: this is the P-LockContract (lock the contract, vary the implementation) versus P-PreserveDecisionSpace (keep rejected alternatives visible rather than discard them) discriminator for when to lock a decision and when to leave it open.
- **"All placements ship, narrow later" (an intake decision, RC-3) is only honest if it comes with signals.** Instrumentation is an in-scope deliverable for this cluster, not a follow-up: P-InstrumentBefore (every production surface ships instrumented before launch) and its IB1 sub-rule (make metrics backfillable first) both apply.
- **Model artifacts are a supply chain** (Frame trust boundary 5, TB-5): a swapped or tampered encoder silently changes every embedding and rerank score.

## Considered Options

These options were closed upstream, by the locked Frame, the locked retrieval research (2026-05-27, r3), and P-0010. They're recorded here per P-PreserveDecisionSpace, not re-derived from scratch.

1. **Postgres-side hybrid retrieval: collapsed-tree multi-resolution index + FTS (full-text search) and dense channels + application-side RRF + local rerank; three host-served verbs (chosen).**
2. **A dedicated retrieval engine** (vector DB, search engine, or graph engine beside Postgres): gated out. Intake's hard constraint rules it out directly ("no separate vector database, graph database, or search engine"), and P-0010's D2 A1-clean stack, with its own named trip-wires (D3, D4, D8: specific evidence thresholds that, if crossed, trigger revisiting a deferred choice), already owns every upgrade path.
3. **Plugin-exported retrieval verbs**: rejected. Retrieval reads across every artifact family, and cross-plugin aggregation is a projection concern, not a substrate concern (see [P-0002](P-0002-core-plugin-partition.md)). `search` and `get_context_for` are non-CRUD domain verbs, and [P-0013](P-0013-plugin-invocation-model.md) defers plugin dispatch for that verb class past V0 (the initial release). Placing them on the host instead keeps that trip-wire unfired.
4. **Clustered-first tree construction (RAPTOR-style) as the V0 default**: rejected as the default, but preserved as a designed-but-unbuilt strategy behind the `TreeBuilder` seam, under the named hierarchy-source eval. The cross-check found no documented precedent for the authored-versus-clustered hybrid, so which one wins is an eval question, not something to lock in advance.

## Decision Outcome

**Chosen: Option 1**, rendered as decisions RA-1 through RA-10 below (RA numbers each individual decision this ADR locks). Binding requirement text lives in the spec ([R-0023, R-0024, R-0028 through R-0034](../../specs/2026-07-02-retrieval-cluster.md)); this ADR is the decision record and the landing site for the D6 borrows.

### RA-1 — Index topology: per-shape chunking + one collapsed tree

The index builder shall chunk content per corpus shape and build **one collapsed-tree, multi-resolution index**: leaf chunks plus parent-linked summary nodes at section and document grain, with every level embedded into a single pgvector HNSW (hierarchical navigable small world) space and searched in **one KNN** (k-nearest-neighbor) query (this is the collapsed-tree style, D6 borrow 3, explained fully in RA-10). It never does staged tree-traversal retrieval. The per-shape chunking policy table is internal (see RA-6):

| Corpus shape | Chunk unit | Grain | Tree |
|---|---|---|---|
| Decision records (ADRs), specs, briefs/frames, research, skills/profiles | Markdown section (header-bounded) | hierarchical | authored tree: section + document summary nodes |
| Daily logs / activity streams | Entry (day / event) | single | none |
| Code | Symbol (function/struct) | single | none |
| Structured rows (tasks, run records) | Row | single | none |

*(Anchors: the locked retrieval research §1.1/§1.2 and the cross-check's corroboration of collapsed-tree; P-0010 D2, where pgvector HNSW is the A1-clean dense leg; Simplicity: one index space, one KNN.)*

### RA-2 — Tree construction sits behind the `TreeBuilder` seam; authored-first under the named eval

There's one `TreeBuilder` contract producing one node shape, with two strategies designed against it: `authored` (markdown-header hierarchy) and `clustered` (embed-cluster-summarize, reimplemented rather than vendored from an existing library). **V0 builds `authored` only.** Authored-first is an **eval hypothesis, not a validated lock**: the hierarchy-source eval (spec R-0028-c) reads two things. First, the per-strategy retrieval-failure rate and nDCG (a ranking-quality metric) on the golden query set (the same harness instrument as P-0010's D3 trip-wire; one harness, two consumers). Second, the per-shape hierarchy-coverage measure reported at every index run. If coverage comes back low, or the clustered arm wins the eval, `clustered` gets built behind the seam. That's a bounded change, not a re-architecture. *(Anchors: the Stage 2a required constraint, ratified by the decomposer; P-LockContract and P-PreserveDecisionSpace together: the seam itself is intrinsic and locks, while the winning strategy stays separable and preserved; P-Defer's DF1, since the eval read is the named firing instrument; P-MinBlastRadius.)*

### RA-3 — Hybrid query path: FTS + dense → application-side RRF → local rerank → budget trim

There are two channels at V0: lexical, using native `tsvector`/`ts_rank` (the language config is a single-sourced per-corpus parameter, defaulting to `english`, never an inline literal), and dense, using the RA-1 HNSW index. The two are fused by **Reciprocal Rank Fusion as application-side SQL**, then reranked by the local cross-encoder (host `rerank`) into the disclosure budget. Exact-key queries resolve through the lexical channel and survive fusion (spec R-0029-b). The keyword leg is deliberately **not BM25** (a standard keyword-ranking algorithm that accounts for term frequency and document length) at V0: P-0010's D3 golden-query regression is the fidelity instrument, and `pg_textsearch` is the named Green upgrade path once that instrument calls for it. Extra channels (fact-key, raw-message, ColBERT late-interaction) are known extension points, but they only get added on attributed retrieval-failure evidence, not speculatively. HyDE rewrite (section 4.2) is an optional, config-gated **fused channel**. It's never a gating dependency. *(Anchors: intake success criterion SC4; P-0010's D2, D3, and D6; research sections 1.3 and 4.2, plus the cross-check's corroboration of RRF-in-Postgres.)*

### RA-4 — Three host-served verbs with disclosure-budget mechanics

`get_context_for`, `search`, and `get_artifact_by_citation` register on the existing stdio MCP (Model Context Protocol) server as host-served verbs, not as plugin exports (that's Considered Option 3, rejected above). Every verb is budget-enforced: an 8,000-token default, a per-call override, a fixed reduction ladder (rerank-order, then coarsen, then drop whole items), no mid-item truncation, and a budget report that lists resolvable omitted citations. There is exactly **one** host-side counting function, with the tokenizer pinned behind it (the host-resident embedding-model tokenizer, spec R-0024-d). `search` returns summary grain on the first pass; citation expansion is a separate second pass. *(Anchors: P-0002, P-0013, and P-0012, the same anchors R-0023 cites; intake success criterion SC3; P-TrustworthySignal (reduction is reported, never silent); P-MinBlastRadius, since there's one counting function and one assembly point.)*

### RA-5 — Degraded-mode matrix; zero-egress is a supported V0 configuration

All four generative placements ship behind independent switches, with the degraded modes locked in the spec's matrix: `context_state: absent` marking, direct-embed fallback, raw-chunks return, and `tags_state: absent` plus a structured degraded notice. `absent` is distinguishable from `empty`, and from `policy-suppressed` (egress-denied while the placement is on, excluded from backfill), in every new schema. The capability surface reports placement state. Synthesis is optional in the ABI (application binary interface), returned as a single response-level digest object marked model-generated and untrusted (spec R-0030-e). The zero-egress configuration (all four placements off, local encoders on) is a **supported V0 configuration**: indexing and all three verbs are fully functional with zero outbound connections **at runtime**, because model artifacts are pre-provisioned per RA-8 and the runtime never fetches anything. The sovereignty promise is satisfied by configuration plus out-of-band provisioning, and it gets upgraded at V0.1 and later by local generative models through the same host-function seam, with no ABI change required. *(Anchors: RC-3, the intake decision behind the maintainer's "all of the above for V0, narrow later" ruling from 2026-05-27; Stage 2a direction 2, ratified by the decomposer; P-TrustworthySignal; P-LockContract, since the provider varies behind the config seam.)*

### RA-6 — Granularity is an internal per-shape policy; no adopter-facing flag

The RA-1 policy table is internal to the index builder. No adopter-facing granularity ABI or config flag ships at V0. The instruments watching for that deferral to end are the self-announcing adopter ask and the storage-overhead measure (summary-node storage as a multiple of leaf storage, tracked per index run) crossing the spec-named **3.0x** threshold. *(Anchors: P-Defer's DF1; Simplicity, since every config knob is a mechanism every reader has to account for.)*

### RA-7 — Instrumentation shape: product measurement tables, backfillable-first

Narrowing signals live as structured retrieval-run records, in plain timestamped, workspace-scoped Postgres tables. This is product measurement data (the dispatch-metrics family), not an observability store: generation and storage are kept separate, per the [observability baseline](../architecture/overview.md#observability) and P-0010's D8/E1. Per-query records capture channels, candidates plus rerank scores, budget actions, placement states, per-stage latency, and the index build. That's enough to recompute A/B metrics offline. **The D4 traversal instrument sits on the traversal engine**: hop count, latency, and a flag on the 500 ms bound or an inexpressible traversal (this is the same logged dogfood incident that fires Apache AGE adoption). **D5's second-adapter re-open rides the same signal.** Per-placement egress-volume counters double as the TB-2 boundary instrument and the local-migration trigger (roughly 14 million document-tokens per month). The query-time egress content-audit records which query text and which chunk citations egressed, keyed for incident response. *(Anchors: P-InstrumentBefore plus its IB1 sub-rule; intake success criterion SC6; P-0010's D4, D5, and D8, carried instruments consumed here rather than re-derived; RC-3's premise.)*

### RA-8 — Model-config pin + verify (TB-5); provisioning + footprint

The model-config schema pins each local model's **revision** and verifies artifact **integrity** (a fixed digest from a trusted source) before loading it. There are no unpinned or `latest` model references. A failed check fails the load closed, with a structured error. The model-artifact source is a distinct trust boundary (TB-5) from the package-dependency tier that `fastembed-rs` itself clears.

**Provisioning** (folded in during r1 review, to reconcile TB-5 with the RA-5 zero-egress claim): the `DF-model-fetch` flow runs **only at deploy or provision time. The runtime itself never fetches model artifacts.** In egress-permitted environments, a deploy or first-provision step fetches into the local model cache, pinned and verified. In a zero-egress environment, the artifacts are placed out-of-band instead: a pre-provisioned cache, or baked into the deployment artifact. A host that starts with a required artifact missing fails the load closed, with no fetch attempt, and reports `/health` as `overall: "degraded"` while that encoder is unloaded (the same precedent as the substrate's R-0007-h). Retrieval verbs return the structured unavailable state in that case (spec R-0030-c and R-0031-d are the binding text).

**Footprint disclosure (always-on encoders):** BGE-M3 and BGE-reranker-v2-m3 load into the single host process in every deployment. The order-of-magnitude sizing basis here is an eval-calibrated placeholder, to be measured for real at implementation: on-disk footprint of roughly **0.5 to 2.5 GB combined** (the range depends on which quantized ONNX artifacts, a model-runtime format, are used), resident memory of the same order (roughly **1 to 3 GB combined**), and cold-start model-load time on the order of **seconds** (single digits to low tens). This is the first sizing guideline for the single-binary posture. The actual measured values land in the deployment docs once implementation happens. *(Anchors: Frame trust boundary 5 and Stage-2 review comment M4; P-SecurityLayered, the supply-chain layer of that principle; RC-1, since sovereignty is satisfiable without runtime egress.)*

### RA-9 — The 4.1 chunk-context ingress control (residual (iv); with P-0015)

Chunk-context output is model-generated text, derived from untrusted corpus content, that gets stored and feeds retrieval scoring. There's no closed vocabulary to post-validate it against (the tag-validator pattern used elsewhere doesn't transfer to free-form prose). **The control chosen, from the Frame section 5 candidate set, is bounded influence, composite.** The output is stored in a dedicated column marked model-generated. It's length-capped (200-token default; anything over cap is rejected and the row marked context-absent). It's used **only** as dense-embedding input for its own chunk, and it's **excluded from the FTS index and from every served payload**. Its maximum possible influence is a dense-ranking shift on its own chunk; it can never inject served content or instructions. Two alternatives from the candidate set were rejected: an extractive-only output form, which would give up the situating value the placement exists for, and an unbounded treat-as-untrusted annotation, since an annotation without an enforcing mechanism isn't a control. *(Anchors: Frame section 5 residual item (iv) and Stage-2 r2 review comment N5; P-SecurityLayered (untrusted model output never reaches a served payload); spec R-0030-f is the binding text.)*

### RA-10 — The P-0010 D6 method-borrows land here (deferral closed)

This cluster is D6's named firing event. The four borrows are adopted as design methods: the engines they came from are out, but the patterns are in.

1. **Single-query BM25-plus-dense-plus-RRF fusion** (`search::rrf()`), used as the ergonomic reference target for the RA-3 application-fusion SQL.
2. **`pg_textsearch`** as the named Green BM25 upgrade path. D3's trip-wire instrument is what decides whether or when to take it.
3. **Collapsed-tree and multi-resolution embeddings, plus keyed-supersession-via-normalized-topic-key SQL patterns.** This is RA-1's one-KNN shape: superseded nodes are excluded from default retrieval but remain reachable point-in-time (spec R-0029-e). The borrow adapts the *shape* of that pattern (default-excluded, point-in-time-reachable SQL), not its key. The source pattern keys supersession on a normalized topic key inferred over content. Mnemra's corpus declares supersession explicitly instead, so the key here is the origin-weighted `supersedes` edge itself. The "`superseded-by` forward pointer" is just that same edge, read from the superseded side (P-0016's edge-schema decision ES-2), and no topic-key inference ever enters the trust path (spec R-0026-a).
4. **Borrow the graph *model*, not a graph *engine*.** Traversal stays recursive CTEs (common table expressions) over the P-0016 edge table (P-0010's D4).

With this ADR authored, P-0010's D6 forward-context note is discharged. *(Anchor: P-0010's D6, ratified.)*

### Typed-DFD extension (Stage-2 review L3)

The cluster's new elements are typed against the [architecture-overview DFD](../architecture/overview.md)'s (data flow diagram) element convention, so each one gets **per-element STRIDE coverage** (STRIDE being the standard threat-modeling category set) rather than the Frame section 5's boundary-level treatment, once the overview's DFD and threat tables are updated at this cluster's pre-implementation security review:

| Element | Type | Notes |
|---|---|---|
| `P-index-builder`, `P-edge-extractor`, `P-query-path`, `P-traversal-engine` | Process | host subsystems (Frame C1/C2/C4/C5) |
| `get_context_for` / `search` / `get_artifact_by_citation` | Process (verbs on `P-mcp-handler`) | host-served (RA-4) |
| `DS-retrieval-nodes`, `DS-edges`, `DS-citation-handles`, `DS-policy-write-audit`, `DS-retrieval-runs`, `DS-traversal-log`, `DS-egress-events`, `DS-index-builds` | Data store | workspace-scoped tables (spec Data Model) |
| `EE-llm-provider` | External entity (existing) | now the **generative-placement** endpoint (RC-1: embeddings no longer egress) |
| `EE-model-artifact-source` | External entity (new) | encoder-model fetch (TB-5, RA-8) |
| `DF-egress-4.1` / `DF-egress-4.2` / `DF-egress-4.3` / `DF-egress-4.4` | Data flow | the four generative egress calls crossing TB-2 (policy-gated, bounded) |
| `DF-model-fetch` | Data flow | crossing TB-5 (pin+verify); **deploy/provision-time only — no runtime fetch** (RA-8) |

### Consequences

**Good:**
- One index space, one KNN, one fusion point, one counting function, one traversal path: this is the smallest topology that serves the intake's success criteria on the locked substrate.
- Every deferral this topology makes (clustered builder, BM25, AGE, extra channels, local generative models, the granularity flag) is wired to a named instrument that the cluster itself ships (RA-7).
- The seam placement (TreeBuilder, config switches, the counting function, host-function primitives) bounds every anticipated change to one module.
- P-0010's D6 deferral closes without adopting any engine.

**Bad / trade-offs:**
- V0's hybrid ships a non-BM25 keyword leg (`ts_rank` lacks IDF, inverse document frequency, and TF-saturation, term-frequency saturation). Fidelity is a measured trip-wire here, not a V0 guarantee (P-0010's D3).
- The authored-first default might lose its own eval. That's accepted deliberately: the clustered strategy is pre-designed behind the seam, so the flip stays bounded if it happens.
- Summary nodes cost extra storage, expected at roughly 1.5 to 2 times leaf storage on high-structure shapes. It's measured per run and flagged if it crosses 3.0x (RA-6).
- Chunk-context's embed-only bounding (RA-9) trades away a slice of the placement's potential lift (no lexical contribution) in exchange for a structural injection bound. The security default wins that trade: Security versus Simplicity resolves in Security's favor by default.

## Pros and Cons of the Options

### Postgres-side hybrid + collapsed tree + host-served verbs (chosen)

- Pro: expressible entirely on the A1-clean, locked substrate. Managed-portability stays intact.
- Pro: exact-key and semantic recall arrive in one budget-shaped call. The collapsed tree serves multi-resolution results without staged traversal.
- Pro: keeps P-0013's domain-verb trip-wire unfired and P-0002's partition untouched.
- Con: application-side RRF and rerank are host code that has to be maintained. Accepted, because the D6 borrow-1 reference shape bounds the maintenance burden.

### Dedicated retrieval engine

- Pro: richer native retrieval features (engine-native fusion, approximate nearest-neighbor search at scale).
- Con: violates the intake hard constraint and P-0010's A1-clean lock. The capability isn't bankable at the product's scale envelope anyway, and license and operations costs would be paid from day one.

### Plugin-exported retrieval verbs

- Pro: uniform verb surface with content plugins.
- Con: fires P-0013's deferred domain-verb machinery for no gain. Retrieval is cross-family by nature (P-0002), and inference itself is host-side anyway: ONNX can't live in the plugin sandbox.

### Clustered-first tree construction

- Pro: corpus-shape-independent hierarchy. It's the default outside this workspace's own research.
- Con: unvalidated *here* too (the cross-check refutes certainty in both directions). It costs generative egress at index time, and authored structure is already present in the corpus's high-structure shapes anyway. It's preserved behind the seam, not discarded.

## More Information

- Binding requirement text: [spec R-0023, R-0024, R-0028 through R-0034](../../specs/2026-07-02-retrieval-cluster.md).
- Companion contracts: [P-0015](P-0015-provenance-envelope-source-roles.md) (envelope, policy record, and decision port; the query path's policy predicates and envelope assembly consume it); [P-0016](P-0016-edge-schema.md) (edge schema plus the traversal contract the traversal engine executes).
- Upstream locks consumed: the [Frame](../../intent/retrieval-cluster-frame.md) (blob `65b1d05`), sections 4, 5, and 6, directions R1, R2, R5 through R8, R10, and R11; the [intake](../../intent/retrieval-cluster.md) document's RC-1, RC-2, and RC-3; [P-0010](P-0010-storage-substrate-engine.md)'s D2, D3, D4, D5, D6, and D8; the locked retrieval research (2026-05-27, r3); the canon-free cross-check (2026-06-05); and the use-case record for get_context_for (2026-06-05), all cited by name and lock-date per the provenance-pointer convention.
- The deferral instruments this ADR relies on are spec deliverables: R-0034 (run records, egress counters and audit, traversal log, coverage and overhead measures).
