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

Accepted 2026-07-02 at the retrieval-cluster **spec-exit gate** (this ADR was part of the Stage-3 package the gate reviewed, with the spec [2026-07-02-retrieval-cluster](../../specs/2026-07-02-retrieval-cluster.md)). Authored at Stage 3 of the retrieval cluster, resolving the Frame's `{{P-0014}}` slot per [placeholder-resolution](placeholder-resolution.md). The architectural directions it renders were already decomposer-ratified — the [Frame](../../intent/retrieval-cluster-frame.md) locked at its frame-exit gate (verdict Accept, 2026-07-02) — and were not re-opened by the gate; what the gate reviewed was this rendering at spec precision.

## Context and Problem Statement

The retrieval cluster (register `1.1.0` + D1 + D2 + G2/G3) needs a locked retrieval topology: how the corpus is chunked and indexed, how search channels combine, where the verbs live, how responses are shaped to a budget, and how the four generative placements degrade. The locked Frame (blob `65b1d05`) directs the shape (directions R1/R2/R5/R6/R7/R8/R10/R11); this ADR is the slot that locks it at ADR precision and the designated landing site for [P-0010](P-0010-storage-substrate-engine.md) **D6's four method-borrows** — the retrieval-layer ADR whose authoring P-0010 named as that deferral's firing event. The companion contracts live in [P-0015](P-0015-provenance-envelope-source-roles.md) (envelope/policy) and [P-0016](P-0016-edge-schema.md) (edge schema); the binding requirement text is the spec's R-0023/R-0024/R-0028–R-0034.

## Decision Drivers

- **One-call, trust-labeled, budget-shaped context** is the product promise (intake JTBD; the 2026-06-05 reference trace measured the re-derivation tax at ≈4 tool calls + 3 reads per task). Progressive disclosure is a hard constraint — context-rot is load-bearing physics, "return more tokens" is not a failure mode.
- **The substrate is locked and A1-clean** (P-0010 D1/D2): Postgres + pgvector behind the `Storage` trait; no new engine, no extension beyond pgvector. Retrieval must be expressible on that stack.
- **Exact-key queries must survive** (decision IDs, file paths, function names) — the query class pure-vector retrieval loses by mathematics (intake SC4).
- **The authored-hierarchy assumption is unvalidated** — the canon-free cross-check (2026-06-05) refuted the stronger authored-beats-clustered claim; the design must preserve the losing arm behind a seam (P-LockContract ⇄ P-PreserveDecisionSpace when-to-lock discriminator).
- **"All placements ship, narrow later" (RC-3) is only honest with signals** — instrumentation is an in-scope deliverable, not a follow-up (P-InstrumentBefore; IB1 backfillable-first).
- **Model artifacts are a supply chain** (Frame TB-5): a swapped or tampered encoder silently changes every embedding and rerank score.

## Considered Options

Closed upstream by the locked Frame, the locked retrieval research (2026-05-27, r3), and P-0010; recorded per P-PreserveDecisionSpace, not re-derived.

1. **Postgres-side hybrid retrieval: collapsed-tree multi-resolution index + FTS/dense channels + application-side RRF + local rerank; three host-served verbs (chosen).**
2. **A dedicated retrieval engine** (vector DB, search engine, or graph engine beside Postgres) — gated out: intake hard constraint ("no separate vector database, graph database, or search engine"); P-0010 D2's A1-clean stack and its named trip-wires (D3/D4/D8) own every upgrade path.
3. **Plugin-exported retrieval verbs** — rejected: retrieval reads across every artifact family (cross-plugin aggregation is a projection concern, not a substrate concern — [P-0002](P-0002-core-plugin-partition.md)); `search`/`get_context_for` are non-CRUD domain verbs whose plugin dispatch [P-0013](P-0013-plugin-invocation-model.md) defers past V0 — host placement keeps that trip-wire unfired.
4. **Clustered-first tree construction (RAPTOR-style) as the V0 default** — rejected as default, preserved as the designed-but-unbuilt strategy behind the `TreeBuilder` seam under the named hierarchy-source eval (the cross-check found no documented precedent for the authored-vs-clustered hybrid; the winner is an eval question, not a lock).

## Decision Outcome

**Chosen: Option 1**, rendered as decisions RA-1..RA-10 below. Binding requirement text lives in the spec ([R-0023, R-0024, R-0028–R-0034](../../specs/2026-07-02-retrieval-cluster.md)); this ADR is the decision record and the D6-borrow landing site.

### RA-1 — Index topology: per-shape chunking + one collapsed tree

The index builder SHALL chunk per shape and build **one collapsed-tree, multi-resolution index**: leaf chunks plus parent-linked summary nodes at section and document grain, all levels embedded into a single pgvector HNSW space and searched in **one KNN** (collapsed-tree style — D6 borrow 3), never staged tree-traversal retrieval. The per-shape chunking policy table (internal — RA-6):

| Corpus shape | Chunk unit | Grain | Tree |
|---|---|---|---|
| Decision records (ADRs), specs, briefs/frames, research, skills/profiles | Markdown section (header-bounded) | hierarchical | authored tree: section + document summary nodes |
| Daily logs / activity streams | Entry (day / event) | single | none |
| Code | Symbol (function/struct) | single | none |
| Structured rows (tasks, run records) | Row | single | none |

*(Anchors: the locked retrieval research §1.1/§1.2 + cross-check corroboration of collapsed-tree; P-0010 D2 — pgvector HNSW is the A1-clean dense leg; Simplicity — one index space, one KNN.)*

### RA-2 — Tree construction sits behind the `TreeBuilder` seam; authored-first under the named eval

One `TreeBuilder` contract producing one node shape; two strategies designed against it — `authored` (markdown-header hierarchy) and `clustered` (embed-cluster-summarize, reimplemented, never vendored). **V0 builds `authored` only.** Authored-first is an **eval hypothesis, not a validated lock**: the hierarchy-source eval (spec R-0028-c) reads (a) per-strategy retrieval-failure rate/nDCG on the golden query set — the same harness instrument as P-0010 D3's trip-wire, one harness two consumers — and (b) the per-shape hierarchy-coverage measure reported at every index run. If coverage is low or the clustered arm wins, `clustered` is built behind the seam — a bounded change, not a re-architecture. *(Anchors: Stage 2a required constraint, decomposer-ratified; P-LockContract ⇄ P-PreserveDecisionSpace — the seam is intrinsic and locks, the winner is separable and preserved; P-Defer/DF1 — the eval read is the named firing instrument; P-MinBlastRadius.)*

### RA-3 — Hybrid query path: FTS + dense → application-side RRF → local rerank → budget trim

Two channels at V0 — lexical (native `tsvector`/`ts_rank`; the language config a single-sourced per-corpus parameter, default `english`, never inline literals) and dense (the RA-1 HNSW) — fused by **Reciprocal Rank Fusion as application-side SQL**, then reranked by the local cross-encoder (host `rerank`) into the disclosure budget. Exact-key queries resolve via the lexical channel and survive fusion (spec R-0029-b). The keyword leg is deliberately **not BM25** at V0: P-0010 D3's golden-query regression is the fidelity instrument and `pg_textsearch` the named Green upgrade. Extra channels (fact-key, raw-message, ColBERT late-interaction) are known extension points added only on attributed retrieval-failure evidence. HyDE rewrite (4.2) is an optional, config-gated **fused channel** — never a gating dependency. *(Anchors: intake SC4; P-0010 D2/D3/D6; research §1.3/§4.2 + cross-check corroboration of RRF-in-Postgres.)*

### RA-4 — Three host-served verbs with disclosure-budget mechanics

`get_context_for` / `search` / `get_artifact_by_citation` register on the existing stdio MCP server as host-served verbs (not plugin exports — Considered Option 3); every verb budget-enforced: 8k-token default, per-call override, the fixed reduction ladder (rerank-order → coarsen → drop whole items), no mid-item truncation, a budget report with resolvable omitted citations, **one** host-side counting function with the tokenizer pinned behind it (the host-resident embedding-model tokenizer — spec R-0024-d). `search` returns summary grain (first-pass); citation expansion is the second pass. *(Anchors: P-0002/P-0013/P-0012 — R-0023's anchors; intake SC3; P-TrustworthySignal — reduction is reported, never silent; P-MinBlastRadius — one counting function, one assembly point.)*

### RA-5 — Degraded-mode matrix; zero-egress is a supported V0 configuration

All four generative placements ship behind independent switches with the degraded modes the spec's matrix locks (`context_state: absent` marking; direct-embed fallback; raw-chunks return; `tags_state: absent` + structured degraded notice). `absent` is distinguishable from `empty` — and from `policy-suppressed` (egress-denied under placement-ON, excluded from backfill) — in every new schema; the capability surface reports placement state; synthesis is optional-in-ABI, returned as a single response-level digest object marked model-generated/untrusted (spec R-0030-e). The zero-egress configuration (all four OFF + local encoders on) is a **supported V0 configuration** in which indexing and all three verbs are fully functional with zero outbound connections **at runtime** (model artifacts pre-provisioned per RA-8 — the runtime never fetches) — the sovereignty promise satisfied by configuration plus out-of-band provisioning, upgraded at V0.1+ by local generative models through the same host-fn seam without ABI change. *(Anchors: RC-3 + the maintainer's "all of the above for V0, narrow later" ruling (2026-05-27); Stage 2a direction 2, decomposer-ratified; P-TrustworthySignal; P-LockContract — provider varies behind the config seam.)*

### RA-6 — Granularity is an internal per-shape policy; no adopter-facing flag

The RA-1 policy table is internal to the index builder. No adopter-facing granularity ABI/config flag ships at V0; the deferral's instruments are the self-announcing adopter ask and the storage-overhead measure (summary-node storage as a multiple of leaf storage, per index run) crossing the spec-named **3.0×** threshold. *(Anchors: P-Defer/DF1; Simplicity — every config knob is a mechanism every reader accounts for.)*

### RA-7 — Instrumentation shape: product measurement tables, backfillable-first

Narrowing signals live as structured retrieval-run records in plain timestamped, workspace-scoped Postgres tables — product measurement data (the dispatch-metrics family), not an observability store (generation⊥storage per the [observability baseline](../architecture/overview.md#observability); P-0010 D8/E1). Per-query records capture channels, candidates + rerank scores, budget actions, placement states, per-stage latency, and the index build — enough to recompute A/B metrics offline. **The D4 traversal instrument is placed on the traversal engine** (hop count, latency, flag on the 500 ms bound or an inexpressible traversal — the logged dogfood incident that fires Apache AGE adoption); **D5's second-adapter re-open rides the same signal.** Per-placement egress-volume counters double as the TB-2 boundary instrument and the local-migration trigger (~14M doc-tokens/month); the query-time egress content-audit records which query text / which chunk citations egressed, keyed for incident response. *(Anchors: P-InstrumentBefore + IB1; intake SC6; P-0010 D4/D5/D8 — carried instruments consumed here, not re-derived; RC-3's premise.)*

### RA-8 — Model-config pin + verify (TB-5); provisioning + footprint

The model-config schema pins each local model's **revision** and verifies artifact **integrity** (a fixed digest from a trusted source) before load; no unpinned/`latest` model refs; a failed check fails the load closed with a structured error. The model-artifact source is a distinct trust boundary (TB-5) from the package-dependency tier `fastembed-rs` clears.

**Provisioning (r1 fold — reconciles TB-5 with the RA-5 zero-egress claim):** the `DF-model-fetch` flow is **deploy/provision-time only — the runtime never fetches model artifacts**. In egress-permitted environments a deploy or first-provision step fetches into the local model cache (pin+verified); in a zero-egress environment the artifacts are placed out-of-band (pre-provisioned cache or baked into the deployment artifact). A host starting with a required artifact absent fails the load closed — no fetch attempt — and reports `/health` `overall: "degraded"` while an encoder is unloaded (the substrate's R-0007-h precedent), with retrieval verbs returning the structured unavailable state (spec R-0030-c/R-0031-d are the binding text).

**Footprint disclosure (always-on encoders):** BGE-M3 + BGE-reranker-v2-m3 load into the single host process in every deployment. Order-of-magnitude sizing basis (eval-calibrated placeholder, measured at implementation): on-disk **~0.5–2.5 GB combined** (quantization-dependent ONNX artifacts), resident memory of the same order (**~1–3 GB combined**), cold-start model-load time **seconds-order** (single-digit to low tens). This is the first sizing guideline for the single-binary posture; the measured values land in the deployment docs at implementation. *(Anchors: Frame TB-5 / Stage-2 review M4; P-SecurityLayered supply-chain layer; RC-1 — sovereignty satisfiable without runtime egress.)*

### RA-9 — The 4.1 chunk-context ingress control (residual (iv); with P-0015)

Chunk-context output is model-generated text derived from untrusted corpus content, stored and feeding retrieval scoring with no closed vocabulary to post-validate (the tag-validator pattern does not transfer to free-form prose). **Control chosen from the Frame §5 candidate set: bounded influence, composite** — the output is stored in a dedicated column marked model-generated; length-capped (200-token default; over-cap rejected, row marked context-absent); used **only** as dense-embedding input for its own chunk; **excluded from the FTS index and from every served payload**. Its maximum influence is a dense-ranking shift on its own chunk — it can never inject served content or instructions. Rejected from the candidate set: an extractive-only output form (would forfeit the situating value the placement exists for) and an unbounded treat-as-untrusted annotation (an annotation without a mechanism is not a control). *(Anchors: Frame §5 residual (iv) + Stage-2 r2 review N5; P-SecurityLayered — untrusted model output never reaches a served payload; spec R-0030-f is the binding text.)*

### RA-10 — The P-0010 D6 method-borrows land here (deferral closed)

This cluster is D6's named firing event; the four borrows are adopted as design methods (engines out, patterns in):

1. **Single-query BM25+dense+RRF fusion** (`search::rrf()`) as the ergonomic reference target for the RA-3 application-fusion SQL.
2. **`pg_textsearch`** as the named Green BM25 upgrade path — D3's trip-wire instrument decides.
3. **Collapsed-tree / multi-resolution embeddings + keyed-supersession-via-normalized-topic-key SQL patterns** — RA-1's one-KNN shape; superseded nodes are excluded from default retrieval and remain point-in-time reachable (spec R-0029-e). The borrow adapts the *shape* (default-excluded, point-in-time-reachable SQL), not the key: the source pattern keys supersession on a normalized topic key inferred over content; mnemra's corpus declares supersession explicitly, so the key is the origin-weighted `supersedes` edge itself — the "`superseded-by` forward pointer" is that edge read from the superseded side (P-0016 ES-2), and no topic-key inference enters the trust path (spec R-0026-a).
4. **Borrow the graph *model*, not a graph *engine*** — traversal stays recursive CTEs over the P-0016 edge table (P-0010 D4).

With this ADR authored, P-0010's D6 forward-context note is discharged. *(Anchor: P-0010 D6, ratified.)*

### Typed-DFD extension (Stage-2 review L3)

The cluster's new elements, typed against the [architecture-overview DFD](../architecture/overview.md)'s element convention so each gets **per-element STRIDE coverage** (rather than the Frame §5's boundary-level treatment) when the overview's DFD and threat tables are updated at this cluster's pre-implementation security review:

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
- One index space, one KNN, one fusion point, one counting function, one traversal path — the smallest topology that serves the intake's success criteria on the locked substrate.
- Every deferral this topology makes (clustered builder, BM25, AGE, extra channels, local generative, granularity flag) is wired to a named instrument the cluster itself ships (RA-7).
- The seam placement (TreeBuilder, config switches, counting function, host-fn primitives) bounds every anticipated change to one module.
- P-0010's D6 deferral is closed without adopting any engine.

**Bad / Trade-offs:**
- V0 hybrid ships a non-BM25 keyword leg (`ts_rank` lacks IDF/TF-saturation); fidelity is a measured trip-wire, not a V0 guarantee (P-0010 D3).
- The authored-first default may lose its eval — accepted deliberately; the clustered strategy is pre-designed behind the seam so the flip is bounded.
- Summary nodes cost storage (~1.5–2× expected on high-structure shapes) — measured per run, flagged at 3.0× (RA-6).
- Chunk-context's embed-only bounding (RA-9) trades a slice of the placement's potential lift (no lexical contribution) for a structural injection bound — the security default wins (Security ⇄ Simplicity resolves default-to-Security).

## Pros and Cons of the Options

### Postgres-side hybrid + collapsed tree + host-served verbs (chosen)

- Pro: expressible entirely on the A1-clean locked substrate; managed-portability intact.
- Pro: exact-key + semantic recall in one budget-shaped call; collapsed tree serves multi-resolution without staged traversal.
- Pro: keeps P-0013's domain-verb trip-wire unfired and P-0002's partition untouched.
- Con: application-side RRF and rerank are host code to maintain (accepted: the D6 borrow-1 reference shape bounds it).

### Dedicated retrieval engine

- Pro: richer native retrieval features (engine-native fusion, ANN at scale).
- Con: violates the intake hard constraint and P-0010's A1-clean lock; capability unbankable at the product's scale envelope; license/ops costs paid from day one.

### Plugin-exported retrieval verbs

- Pro: uniform verb surface with content plugins.
- Con: fires P-0013's deferred domain-verb machinery for no gain; retrieval is cross-family by nature (P-0002); inference is host-side (ONNX cannot live in the sandbox).

### Clustered-first tree construction

- Pro: corpus-shape-independent hierarchy; the research-external default.
- Con: unvalidated *here* too (the cross-check refutes certainty in both directions); costs generative egress at index time; authored structure is already present in the corpus's high-structure shapes. Preserved behind the seam, not discarded.

## More Information

- Binding requirement text: [spec R-0023, R-0024, R-0028–R-0034](../../specs/2026-07-02-retrieval-cluster.md).
- Companion contracts: [P-0015](P-0015-provenance-envelope-source-roles.md) (envelope, policy record, decision port — the query path's policy predicates and envelope assembly consume it); [P-0016](P-0016-edge-schema.md) (edge schema + traversal contract the traversal engine executes).
- Upstream locks consumed: [Frame](../../intent/retrieval-cluster-frame.md) (blob `65b1d05`) §4/§5/§6 R1/R2/R5–R8/R10/R11; [intake](../../intent/retrieval-cluster.md) RC-1/RC-2/RC-3; [P-0010](P-0010-storage-substrate-engine.md) D2/D3/D4/D5/D6/D8; the locked retrieval research (2026-05-27, r3) + canon-free cross-check (2026-06-05) + use-case record: get_context_for (2026-06-05), cited by name and lock-date per the provenance-pointer convention.
- Deferral instruments this ADR relies on are spec deliverables: R-0034 (run records, egress counters + audit, traversal log, coverage + overhead measures).
