---
title: "Frame: retrieval cluster — get_context_for + provenance envelope + search activation + typed edges + source-roles"
summary: "Cold-start Frame for the retrieval cluster: three host-served MCP verbs (get_context_for / search / get_artifact_by_citation) over a hybrid FTS+dense+RRF retrieval path with local non-generative inference (RC-1), a validator-gated two-axis provenance envelope (trust provenance + a policy permissions record behind a single decision port; freshness + displacement), a single superset edge substrate with discriminating provenance semantics, an authored-first tree-build seam under a named eval, per-placement generative-egress boundaries with zero-egress as a supported V0 configuration, and the D4/D5/D6 deferral instruments placed on the paths this cluster activates. Names the three open ADR slots ({{P-0014}} retrieval architecture, {{P-0015}} provenance envelope + source-roles, {{P-0016}} edge schema) the Stage-3 spec will resolve."
primary-audience: agent
modulation: cold-start
status: locked
date: 2026-07-02
intake: docs/intent/retrieval-cluster.md
spec-type: architecture
---

# Frame — retrieval cluster: get_context_for + provenance envelope + search activation + typed edges + source-roles

**Date:** 2026-07-02 · **Status:** locked (frame-exit gate confirmed 2026-07-02 — verdict Accept; reviews: Stage-2 r1 [9 findings folded] + r2 delta [0 blocker/high; 11 findings folded]; gate outcomes: N3 write-gate confirmed, N4 admin-only visibility added) · **Altitude:** retrieval subsystem architecture (feature cluster, designed tier)

> **Modulation note.** This is a `cold-start` Frame: the cluster is surface-altering (ABI
> inference primitives + storage/edge shape), so the full Stage 2a elicitation ran before this
> synthesis. The validated intake ([`docs/intent/retrieval-cluster.md`](retrieval-cluster.md),
> locked 2026-07-02) and the Stage 2a input record (embedded in §2 below) are the locked inputs;
> the four 2a directions are decomposer-ratified and are **not re-opened here**. This Frame is the
> closed world for the Stage-3 spec: nothing outside the directions locked here enters the spec.

## 1. Purpose / context

An agent picking up an artifact sits inside a graph — the work it extends, the decisions it
feeds, its lifecycle state, prior related sessions. Today that graph is authored but **latent**
(frontmatter relation lists, free-text citations, prose), so every session re-derives context by
hand or acts on a stale picture. The live reference trace (use-case record: get_context_for,
2026-06-05) measured the cost: ≈4 tool calls + 3 reads per task to reconstruct state, and a
near-fired redundant research dispatch measured at 1–4M tokens. This cluster designs the
retrieval feature that collapses that reconstruction to one trust-labeled call.

- **What this designs.** The retrieval subsystem inside the mnemra-core host process: batch
  indexing over the corpus the substrate already holds, hybrid search activation, a typed
  traversable edge substrate, a validator-gated provenance envelope on every returned item, and
  three MCP verbs on the existing stdio server. Register entries covered: `1.1.0`
  `get_context_for` plus D1 (search + indexing), D2 (graph edges + traversal), and G2/G3
  (authoritativeness + provenance/use-policy substrate fields) — the intake's lock performs their
  `idea → proposed` promotion; the register update rides this cluster's docs PR.
- **What this does not design.** No ongoing ingest (register `1.2.0`), no memory compaction, no
  eval-harness build-out, no narrowing decisions on LLM placements, no separate vector/graph/
  search engine, no multi-tenant policy expansion, no committed-tier content (intake Non-goals,
  all seven carried).
- **Intake.** [`docs/intent/retrieval-cluster.md`](retrieval-cluster.md) — locked 2026-07-02,
  high stakes, `spec_type: architecture`, Stage 1c security review round 1 zero blocker/high.
  RC-1 (no *generative* LLM hosted; local non-generative inference permitted host-side), RC-2
  (full cluster), RC-3 (all four LLM placements ship with narrowing signals), and the
  `architecture` spec_type are maintainer-resolved 2026-07-02 and treated as settled.
- **Design substrate.** The locked retrieval research (2026-05-27, r3): progressive-disclosure
  RAG over the heterogeneous corpus — per-shape chunking (§1.1), hierarchical index (§1.2),
  hybrid FTS+dense+RRF (§1.3), over-retrieve→rerank→trim disclosure (§2.3/§2.5), four model
  slots with Green local defaults (§3), four LLM placements each with a named narrowing signal
  (§4), per-layer cloud→local migration paths (§5). The independent canon-free cross-check
  (2026-06-05) corroborates collapsed-tree retrieval and RRF-in-Postgres and **refutes the
  stronger authored-beats-clustered claim** — carried here as an eval hypothesis, not a lock
  (§6 R5). The sources-with-roles finding (maintainer record, 2026-05-21) supplies the freshness
  and displacement semantics rendered into the envelope contract (§6 R3).
- **Substrate base pin (intake success criterion 7).** The cluster designs against
  `main@4e852a1` **plus assumed-shipped surfaces** (the V0 substrate spec's requirement set as
  implemented through the signing→runnable work). The Stage-3 spec SHALL record this pin so
  staleness is detectable at the designed → committed transition. The register V0 sequence is
  `proposed`, not `live`: this cluster designs on a not-yet-cut-over substrate by intent.

## 2. Stage 2a — Elicitation + pre-gate maintainer walk

Elicitation was conducted by the orchestrator with the maintainer, 2026-07-02, one round; the
maintainer locked all four direction forks explicitly. Residual ambiguity fell below threshold —
the remainder is synthesis work within locked canon (§6). This section embeds the Stage 2a input
record (condensed faithfully; the four locked directions verbatim in substance) and, at its end,
the pre-gate maintainer-walk ratification record (2026-07-02) that revision r2 folds.

### Locked directions (maintainer, 2026-07-02 — decomposer-ratified, off-limits to reopen)

1. **Edge substrate — single superset table.** One edge substrate whose vocabulary is the
   superset of the V0 `0.8.0` work-graph set (parent / blocks / depends-on / supersedes /
   dispatched-by) and the retrieval citation set (extends / feeds / cites / supersedes);
   `supersedes` unifies. One traversal + one extraction code path. Design obligation
   acknowledged at elicitation: a discriminating mechanism (column or equivalent) separating
   work-graph vs citation provenance semantics. Resolves the intake's "Open items carried to
   Frame — edge-vocabulary composition" and Stage 1c finding F6.
2. **Egress posture — zero-egress mode is a supported V0 configuration.** Every generative
   placement (4.1 chunk-context, 4.2 HyDE rewrite, 4.3 synthesis, 4.4 tag-gen) is individually
   degradable/disable-able via config. A sovereignty deployment at V0 disables generative
   placements; retrieval remains functional in degraded mode — hybrid search + rerank + envelope
   + traversal depend only on local non-generative inference (RC-1) and MUST work with all four
   generative placements OFF. V0.1+ local generative models arrive through the same host-fn seam
   (research §5.7 migration targets), upgrading degraded mode without ABI change. The
   self-hostable "data never leaving that boundary" promise is satisfiable at V0 **by
   configuration**, not deferred to a future migration. The Frame states the per-placement
   egress boundary for the default (egress-on) configuration (§5). Resolves Stage 1c F2 and the
   intake's required sovereignty-reconciliation constraint.
3. **ADR slots — three focused ADRs:** `{{P-0014}}` retrieval architecture (P-0010 D6's four
   method-borrows land here — this cluster is D6's named firing event); `{{P-0015}}` provenance
   envelope + source-roles contract (closed, validator-gated role vocabulary; freshness +
   displacement semantics); `{{P-0016}}` edge schema (superset vocabulary, discriminating
   semantics, extraction sources, traversal contract). Scoped in §8.
4. **MCP verb surface — three verbs:** `get_context_for(artifact_id)` + `search(query)` +
   `get_artifact_by_citation(ref)`. First-pass / second-pass / citation-expansion per research
   §2.2/§2.4; constrained tool surface; every verb budget-enforced (no silent truncation).

### Proposed shape presented at elicitation (accepted direction; §4 refines)

Retrieval subsystem inside the mnemra-core host process + storage. Batch indexing over the
corpus the substrate already holds; new verbs extend the existing stdio MCP server; all inference
behind the host-fn seam — local encoders in-host (BGE-M3 embed, BGE-reranker per RC-1),
generative work egressing per direction 2. Component map at architectural altitude: index
builder, edge extractor, storage additions (all workspace-scoped NOT NULL, indexed, explicitly
passed), query path (optional HyDE → FTS + dense → RRF → local rerank → budget trim → envelope
assembly), traversal via recursive CTE (P-0010 D4), host-fn ABI extension
(`embed / rerank / rewrite-query / summarize / classify`, synthesis optional-in-ABI).

### Required Frame constraints carried from 2a (all discharged in this Frame)

| Carried constraint | Discharged at |
|---|---|
| Per-placement egress boundary statement + sovereignty reconciliation | §5 (egress table + zero-egress configuration) |
| D4/D5 trip-wire instruments live on the traversal path this cluster activates | §6 R11 |
| Authored-vs-clustered hierarchy = eval hypothesis, not lock | §6 R5 |
| Sources-with-roles freshness + displacement semantics in the envelope contract | §6 R3 |
| Validator-gated role vocabulary (mechanical validator per role; no prose-judgement roles) | §6 R3 |
| Tenancy invariant on every new table | §5, §6 R4/R6 |
| Disclosure budget: 8k default, per-call override, deliberate reduction never silent truncation | §6 R2 |
| Narrowing signals: every LLM placement ships its A/B-able signal; no narrowing decisions here | §6 R7/R11 |
| Substrate base pin | §1 |

### Left to synthesis (2b work — resolved in §6)

Source-role validator definitions per role (R3); edge-schema discriminating mechanism (R4);
tree-build seam design (R5); per-verb budget mechanics (R2); tag vocabulary (R9);
index-granularity flag posture (R10); host-fn ABI signatures (R8); degraded-mode behavior per
generative placement (R7); where the four narrowing signals physically live (R11).

### Consultations during 2a

None — the four forks were locked directly with the maintainer; no expert consultation fired.

### Pre-gate maintainer walk (2026-07-02 — decomposer-ratified, off-limits to reopen)

Between the round-1 synthesis and the Frame-exit gate, the maintainer walked the round-1 Frame
item by item (explicit per-item decision walk, 2026-07-02) and ratified thirteen items; this
record is the gate's audit trail, and revision r2 folds every one. Condensed here — except the
eight policy mechanics, which are carried **verbatim** (they are the locked word sequence R3
applies).

**Ratifications on the round-1 Frame as written:**

1. **R1 (host-served verbs) pre-ratified.** Maintainer: "yes they should be."
2. **Residual risk (i) accepted as-is for V0** — unlabeled artifacts are egressable at index
   time (opt-in labeling); `{{P-0015}}` owns labeling correctness and the adopter-facing
   polarity revisit; the serving-time fail-closed posture unchanged (§5).
3. **Residual risk (ii) acknowledged** — HyDE query-text egress: the per-placement config
   switch is the control, stated honestly; no design change (§5).

**Envelope redesign (the substantive change — R3 is the landing site):**

4. **Two-axis split.** The single five-role value is replaced by two orthogonal axes:
   **policy** (permissions/security) and **trust** (provenance: `authoritative` / `outdated` /
   `background`). Rationale (maintainer): an item can be sensitive+authoritative or
   dont-use+background; the single-value precedence destroys information. Per-axis single-value
   determinism retained; trust-axis semantics (ranking, labels, point-in-time reach) carry over
   from round-1 R3 unchanged.
5. **The policy side is a permissions RECORD, not a value — eight locked mechanics.** The
   maintainer's framing, verbatim: "capture the hard to change stuff now — the mechanics; the
   labels are the easy part."
   1. **Record shape:** orthogonal, independently-valued permission dimensions per content
      item — never a single enum.
   2. **Validator-gated:** every dimension is a mechanical predicate over stored fields/edges;
      no inference anywhere in assignment.
   3. **Structural fail-closed:** any undecidable check on a security dimension resolves to its
      restrictive pole — including enforceable-in-principle-but-not-at-V0 (see item 7 below,
      owner-only). Corollary: a settable-but-unenforced security value may not exist in the
      schema.
   4. **Named enforcement points:** index-admission gate, model-egress gate (index-time and
      query-time), serving predicate, citation stub. Every dimension declares which point(s)
      enforce it; a dimension without an enforcement point is not a dimension.
   5. **Operation facet:** every dimension declares the operation it governs (serve/egress vs
      write/label/supersede). The read/write permission split is anticipated structurally; the
      write side is NOT designed now (maintainer: "maybe that can wait"). Composes with
      Stage-2 review finding M3 (§5).
   6. **Closed-but-extensible at ADR tier:** adding a dimension is a `{{P-0015}}` amendment
      declaring validator + enforcement point + fail-closed pole + governed operation. Never a
      config knob.
   7. **Permission-field writes are an authorization surface:** label/permission changes are
      recorded, attributable writes. (Stage-2 review finding M3 is the independent
      corroboration; residual design lands in `{{P-0015}}`/`{{P-0016}}`.)
   8. **Single decision port, Cedar PARC shape:** all enforcement points consult one boundary
      `decide(principal, action, resource, context) → allow/deny + reason`. V0 implementation =
      the hardcoded validator predicates behind the port. Conformance invariants (fail-closed;
      attributes-only) are tested AT THE PORT (design-time conformance suite, D5
      two-adapter-test pattern). Row-level serving filters: SQL fragments derived from the
      port's attribute vocabulary in exactly ONE module (single-sourced), so a future engine
      swap replaces one module. Anchors: the Cedar/NGAC permission-model research (maintainer
      record, locked 2026-05-18 — "the engine sits behind mnemra's policy port… port
      substitution, not a rewrite"; cedar-policy 4.x, Apache-2.0/Green); P-LockContract; G-0017
      FlagProvider precedent; P-0010 D5 Storage-trait precedent. Maintainer's stated intent:
      bound the blast radius of the future flexible-policy engine to the PDP module.
6. **Labels demoted to proposed-initial-set** — dimension names and value enums are finalized
   at `{{P-0015}}` and explicitly easy-to-change; the mechanics above are the hard part locked
   now (R3 carries the proposed set).
7. **`visibility: owner-only` at V0 = serve to no one** — fail-closed application of mechanic
   3: caller identity is undecidable under V0's workspace+role context, so owner-only items are
   excluded from all retrieval until identity lands. NOT a settable-but-unenforced value; the
   conservative enforcement IS the V0 semantics (R3).
8. **Temporality: owner-mutable flip only.** Permission changes are explicit, recorded,
   authorized writes (mechanic 7). No auto-expiry at V0; timed embargo recorded as a
   `{{P-0015}}` design option, unbuilt, awaiting a real case (R3, §9).
9. **Per-user identity machinery is OUT** — a named future register entry. Owner columns as an
   identity model, caller identity in context, real owner==caller checks: not built in this
   cluster. Intake Non-goal 6 holds; retrieval-path use-policy controls over stored fields are
   this cluster's blessed scope (intake risk profile (a)); tenant-enforcement machinery
   expansion is what stays fenced (R3, §8).
10. **SC2 interpretation note** recorded verbatim at §11 — the two-axis envelope *implements*
    intake success criterion 2's singular source-role commitment; deliberate interpretation,
    not intake drift; no intake amendment.

**Cross-cutting retrofit preparation** (maintainer posture: "items like tenancy, permissions,
users, accessibility, globalization are really hard to retrofit — do the best we can to prepare
for them"):

11. **Identity-bearing columns on every new table this cluster adds:** `owner`/`created_by` on
    content-bearing rows + actor attribution on permission-write records. Coarse or defaulted
    at V0 single-user; the columns exist and fill from day one so identity arrival is a
    feature, not an excavation (R4, C3).
12. **FTS language config is a single-sourced parameter** (per-corpus, default `english`) —
    never inline `to_tsvector('english', …)` literals scattered across schema/queries/indexes;
    multilingual dense retrieval is already covered (BGE-M3 native) (R6).
13. **Accessibility routed to the product brief at Stage 3** as a standing product requirement
    binding on UI/docs surfaces — not faked into this MCP-verb cluster (§8).

**Also folded in r2:** all nine findings of the Stage-2 security/conflict review, round 1
(2026-07-02; zero blocker/high, four medium, five low) — M1 fixed in §3, M2/M3/M4 named at
§5/R3/R8, L1–L5 fixed or dispositioned in place. **Not reopened:** RC-1/RC-2/RC-3/spec_type;
all four Stage 2a directions; the locked intake in its entirety; round-1 directions R1, R2,
R4–R11 except where an item above explicitly touches them (R3 is the redesign site; R4/R6 gain
the prep columns/parameter; QA scenarios update to match the two-axis design).

### Gate outcomes (Frame-exit gate, 2026-07-02 — decomposer-ratified; folded r3)

The Frame-exit gate returned **Accept, pending the mechanical fold**. Two decomposer decisions
were taken at the gate on the Stage-2 r2 delta review's gate-flagged findings, ratified with
the same authority as the thirteen walk items above; they are recorded here as gate-outcome
additions — the walk record itself is unchanged:

14. **V0 write-gate confirmed (Stage-2 r2 review N3):** at V0, ALL writes to policy/
    trust-affecting fields (permission flips, freshness overrides) ride the **existing
    admin-gated content-mutation path (P-0009), with `read_observer` excluded**. Stated as an
    invariant at §5's write-authorization surface; the authorization precondition added to
    QA-7. With it, the write-side deferral's self-announcing trigger (§13 batched #13) stands
    sound as rendered.
15. **`admin-only` visibility value added (Stage-2 r2 review N4):** the `visibility`
    dimension's V0 value set becomes `workspace` / `admin-only` / `owner-only`. The
    `admin-only` predicate: the caller's `WorkspaceCtx` role is `Admin` — role-gated,
    mechanically enforceable at V0, no identity plumbing. `sensitive`-intent content maps
    faithfully: `model_egress: deny` + `visibility: admin-only` (round-1 read semantics
    restored: admin-readable, observer-withheld). The fail-closed pole for `visibility`
    remains `owner-only`. A label-tier change under walk item 6 (the proposed initial set is
    explicitly easy-to-change); the eight mechanics are untouched. §11(3) updated to the
    now-accurate five-of-five claim; QA-4 restores the admin-sees/observer-withheld measure.

## 3. Constraint-graph walk — operating constraints

Walking `constraint-edges.md` from the validated intent (one-call trust-labeled context;
provenance envelope; search activation; typed edges) plus the 2a record. Traversal rule
(G-0013): most-specific applies when no conflict — per-task > project ADR > workspace ADR >
principle. The most-specific layer here is the mnemra-core project ADR set (P-0002, P-0006,
P-0009, P-0010, P-0012, P-0013), which applies over the workspace principles it specializes.

### Keystone edge — the envelope is an authorization control, not metadata

**`P-SecurityLayered → Security` (specializes, `constraint-edges.md:43`)**, specifically the
value's "each layer independently load-bearing" clause (`architecture-values.md:31`). The intake's
risk profile (a) states it precisely: the `sensitive` / `dont-use` semantics *are* a
use-policy control, and **a retrieval path that ignores them leaks by design**. Under the
ratified two-axis envelope (§2 pre-gate walk; R3) those semantics live as policy-record
dimensions (proposed: `dont_use`, `model_egress`, `visibility`), and the envelope's policy side
is a *security layer* of this cluster, not a decoration on results: every security dimension is
enforced structurally at its named enforcement points (index-admission gate, model-egress gate,
serving predicate, citation stub — §6 R3), behind the single decision port, fail-closed, the
same way P-0006 makes `WorkspaceCtx` structurally unavoidable rather than advisory. The second application of the same edge: the
generative placements open a **design-time data-egress layer** (index-time document content;
query-time query text and retrieved content) that must be stated per placement and gated
policy-aware (§5). Both are load-bearing anchors for R3 and the §5 egress design; neither is a
refinement of the other — losing either leaks.

### Other edges bearing on the locked decisions

| Edge (type) | Bears on | How it applies |
|---|---|---|
| `P-Defer → Simplicity, Reversibility` (specializes, `constraint-edges.md:34–35`) + DF1 | Every deferral this Frame makes | Each deferral names its mechanical firing instrument: the clustered tree-builder (R5, eval signal), the index-granularity flag (R10, storage instrument + self-announcing adopter ask), `pg_textsearch` (P-0010 D3's golden-query regression harness), Apache AGE (P-0010 D4's traversal instrument), the second storage adapter (P-0010 D5, rides D4's signal), local generative migration (research §5.3/§5.5 throughput/sovereignty/latency triggers), TimescaleDB (P-0010 D8). Deferrals without a self-firing instrument are labelled parked with a named cadence. |
| `P-LockContract → Simplicity, Migration cost` (specializes, `constraint-edges.md:38–40`) | The envelope contract; the policy decision port; the host-fn inference ABI; the verb surface; the tree-build seam | Lock what is intrinsic: the two-axis envelope shape, the permission-record mechanics + the single decision-port boundary (R3 mechanics 1–8 — the maintainer's "capture the hard to change stuff now"), the closed per-axis vocabulary *shape* (the labels themselves are demoted to `{{P-0015}}` as deliberately easy-to-change), the three-verb surface, and the inference-primitive ABI shape are intrinsic to what this cluster *is* and lock at this stage (research §5.6/OQ5: consider the ABI shape now to avoid breaking it at activation). Implementations vary behind them: model choice behind `embed`/`rerank`, tree-build strategy behind the `TreeBuilder` seam, generative provider behind the egress config, the policy engine behind the decision port (V0: hardcoded validator predicates; Cedar/NGAC research, locked 2026-05-18 — port substitution, not a rewrite). |
| `P-LockContract ⇄ P-PreserveDecisionSpace` (conflicts-with, when-to-lock axis, `constraint-edges.md:140`) | What locks now vs what stays open | Applied per the edge's discriminator, not escalated: intrinsic invariants (envelope shape, verb surface, edge substrate unification, budget-as-tunable) lock now; separable future options (clustered tree-build, BM25 upgrade, AGE, local generative models, granularity flag) are preserved behind named trip-wires. The authored-first default is explicitly a **lock of the seam + a preserved hypothesis**, not a lock of the winner (§6 R5). |
| `P-InstrumentBefore → Observability` (specializes, `constraint-edges.md:43`) + IB1 | Narrowing signals; extraction coverage; egress volume; traversal latency | The retrieval surface is production (it is meant to keep running; no time-box). Instrumentation ships **with the surface, before first user-touch**: retrieval-run records, per-placement egress counters, extraction-coverage and hierarchy-coverage measures, and the D4 traversal instrument are in-scope deliverables of this cluster, not follow-ups (§6 R11). Backfillable-first: run records retain enough per-query detail to recompute A/B metrics offline. The `P-Defer ⇄ P-InstrumentBefore` tension (`constraint-edges.md:139`) resolves per its stated posture — P-InstrumentBefore wins for production surfaces; the instruments are precisely what makes every other deferral here safe. |
| `P-TrustworthySignal → Observability, Quality` (specializes, `constraint-edges.md:70–71`) | Degraded-mode honesty; budget reporting | A retrieval response that silently omits what the caller is **entitled to** (truncation, disabled tag filters) is a signal that lies. Locked consequences: over-budget reduction is always reported in the response envelope (R2); a tag-filtered query against a tag-gen-disabled corpus returns a structured degraded-mode notice, never silently-empty results (R7); an inference host-fn that is unavailable errors structurally rather than no-op'ing (R8). **Policy-filtered items are categorically different and deliberately NOT in this set** (Stage-2 review M1): policy filtering produces the caller's *entitled view* — the withheld items were never the caller's to see — so their exclusion is not an omission-that-lies. `dont_use`/visibility-restricted exclusion is caller-silent **by security requirement** (a caller-visible "N items hidden" marker would be an existence-disclosure oracle an unauthorized caller could probe), and the operator-side audit event is the observability signal that satisfies this principle there (R3, TB-1, QA-4). |
| `G-0015 (relational substrate default) → P-LockContract` (specializes, `constraint-edges.md:94`) + project ADR **P-0010** (most-specific) | The entire storage shape | Postgres + pgvector behind the engine-agnostic `Storage` trait; A1-clean V0 stack (pgvector HNSW + native FTS + recursive CTEs + JSONB); no new engine, no extension beyond pgvector (D2); keyword leg = native `ts_rank` with the `pg_textsearch` fidelity trip-wire (D3); graph leg = recursive CTEs with the AGE strain trip-wire (D4); the retrieval additions must be expressible through the `Storage` trait's contract, preserving its two co-equal invariants (single-transaction keyed supersession; workspace-scoped isolation). This cluster is **D6's named firing event**: the four method-borrows land in `{{P-0014}}` (§8). |
| Project ADR **P-0002** (core-plugin partition; specializes the brief's plugin constraints) | Where retrieval lives | Retrieval reads across every artifact family, and cross-plugin aggregation is "a projection concern, not a substrate concern" (P-0002 verbatim) — the per-family plugin cohesion model does not fit it; P-0013's domain-verb deferral is the decisive second prong (row below). The inference-substrate dependency (one model serves all consumers — research §3.1) is supporting context, not P-0002's *builtin* criterion — that criterion is bootstrap-ordering (state that must exist before any plugin loads), which retrieval is not (Stage-2 review L4, citation precision). Applied in §6 R1: retrieval is a host subsystem, its verbs host-served. |
| Project ADR **P-0013** (plugin invocation model) | Why the verbs cannot be plugin exports at V0 | P-0013 locks the plugin export surface to the fixed typed `content` CRUD interface and **defers domain/non-CRUD plugin verbs past V0** (named trip-wire: the first domain verb that must dispatch at the V0 surface). `search` / `get_context_for` are non-CRUD domain verbs. Host-serving them (R1) means no plugin domain-verb dispatch is required, so P-0013's trip-wire does **not** fire and the deferred dynamic-resolution machinery stays unbuilt. |
| Project ADRs **P-0006** + **P-0009** (tenant enforcement; RLS/admin token; specialize P-SecurityLayered) | Every new table and every new read path | `WorkspaceCtx` threading applies to every retrieval host-fn and query (first parameter, single construction site, WHERE-clause-mandatory); V0 enforcement is application-layer; every new table carries `workspace_id NOT NULL`, indexed, explicitly passed; the P-0009/P-0010 RLS preconditions (no BYPASSRLS/superuser, `FORCE ROW LEVEL SECURITY`, per-transaction `SET LOCAL`) are carried so V0.1+ policy activation is additive on these tables too (§5). |
| Project ADR **P-0012** (raw Wasmtime + `rmcp`) | The MCP surface the verbs extend | The three verbs register on the existing single stdio MCP server (`rmcp` `ServerHandler`), behind the locked auth/dispatch order (auth check before routing; single `WorkspaceCtx` construction; per-verb capability check). No new transport, no HTTP activation. |
| `P-StackDiscipline → Rust-ecosystem alignment` (specializes, `constraint-edges.md:41`) + S1/S2 | Model stack and reimplementation posture | BGE-M3 + BGE-reranker-v2-m3 via `fastembed-rs` (Rust ONNX, Green/MIT+Apache-2.0); RAPTOR-and-kin are design references **reimplemented** against the substrate, never vendored Python (intake hard constraint; research §5.6). No new ecosystem enters the build. |
| `P-MinBlastRadius → Maintainability` (specializes, `constraint-edges.md:65`) | Vocabulary consistency; seam placement | One edge substrate with one vocabulary rather than two parallel edge stores (2a direction 1; Stage 1c F6's preferred resolution); one budget-counting function; one envelope assembly point; one serving-filter SQL-fragment module derived from the decision port's attribute vocabulary (R3 mechanic 8); the tree-build strategy behind one seam so switching strategies is a bounded change. |
| `P-AgentPrimarySource → Decomposition, Dual-audience` (specializes, `constraint-edges.md:67–68`) | Citation shape; consumer framing | Every envelope item carries a stable citation (artifact ID + block anchor) resolvable by `get_artifact_by_citation` — the ID+name pairing and block-addressability the principle mandates are what make citation-expansion (research §2.4) work. The verbs' primary consumer is agents; no human-view derivative is designed here. |
| `P-PreserveDecisionSpace → Honesty` (specializes, `constraint-edges.md:56`) + PD1 | The RC-1 product-brief amendment; displacement semantics | PD1 obligates the Stage-3 amendment to reconcile **every** falsified canonical copy in the same change: the brief's Non-goals clause ("Embeddings and summaries call out to an external model; the system never hosts one"), its Hard-constraints clause ("MUST NOT host a language model"), and the `0.1.0` substrate entry's external-embedding framing — as labeled MODIFIED deltas per the brief's format contract; the architecture-overview ELT external-embedding framing is the named lagging copy reconciled at the same stage (Stage 1c F1, folded into the intake). Displacement semantics (decisions record their axes; re-eval never auto-invalidation) are this principle applied to the envelope's freshness layer. |

### Conflicts-with findings

**No conflict requires escalation.** Three candidate tensions were walked; each resolves inside
its edge's stated resolution posture, so none is a live `conflicts-with` requiring the
decomposer:

1. **Security ⇄ Simplicity** (`constraint-edges.md:133`): policy-aware egress gating and the
   policy filter add mechanism. Resolution per the edge ("default-to-Security"): both mechanisms
   are minimal *and* load-bearing — a WHERE predicate and an index-admission check behind one
   decision port, no new subsystem. Mutual reinforcement, not a trade-off to escalate.
2. **Observability ⇄ Simplicity** (`constraint-edges.md:134`): the retrieval-run records and
   egress counters are instrumentation cost. Resolution per the edge: default-to-Observability
   for production surfaces — and here the instruments are doubly load-bearing, because the
   maintainer's "all placements ship, narrow later" ruling is only honest if the narrowing
   signals exist (RC-3 is *premised* on the instrumentation).
3. **P-LockContract ⇄ P-PreserveDecisionSpace** (when-to-lock, `constraint-edges.md:140`):
   applied per the edge's discriminator (intrinsic locks now; separable options behind
   trip-wires) — see the edge table row above. The authored-vs-clustered call is the worked
   case: the *seam* is intrinsic (locks), the *winner* is a separable hypothesis (preserved
   under a named eval).

## 4. System boundary + component map

**System boundary.** The retrieval subsystem lives inside the mnemra-core host process and its
Postgres substrate. Inputs: the corpus the substrate already holds after the `0.14.0` batch
migration (files + frontmatter + structured rows) — no ongoing ingest. Outward surface: three
MCP verbs on the existing stdio server. Outbound surface: the external LLM API for the four
generative placements (default configuration only; §5). Local inference (embedding, reranking)
never leaves the host process (RC-1). Plugins remain IO-free; nothing in this cluster runs
inside a plugin sandbox.

**Component map (architectural altitude; refines the 2a shape, does not redirect it):**

| # | Component | Role | Key constraints |
|---|---|---|---|
| C1 | **Index builder** (host subsystem; batch, re-fires on document change) | Per-shape chunking (research §1.1 policy table) → tree build via the `TreeBuilder` seam (authored strategy at V0; R5) → local embed (host `embed`) + `tsvector` + generative enrichment (4.1 chunk-context, 4.4 tag-gen) where egress- and policy-gated (§5) → chunk/summary rows | Policy admission gate: `dont_use` artifacts never enter the index (R3); `model_egress: deny` artifacts index locally, skip generative egress. Emits hierarchy-coverage + egress-volume instruments (R11) |
| C2 | **Edge extractor** (host subsystem; part of the index pipeline) | Frontmatter relation lists + free-text citations → superset edge table rows (`origin = extracted`, with source-span provenance); idempotent re-extraction | Extraction coverage over the migrated corpus is **measured and reported**, not assumed (intake SC5; R4) |
| C3 | **Storage additions** (Postgres, behind the `Storage` trait) | Chunk + summary-node tables (parent pointers), pgvector HNSW over all node levels, FTS columns/GIN, the superset edge table (extends `0.8.0`), envelope substrate fields (G2/G3: sensitivity, use-policy, per-kind lifecycle state, freshness handle/decay columns, decision axes), retrieval-run record tables | Every table: `workspace_id NOT NULL`, indexed, explicitly passed; RLS preconditions carried (§5). Identity-bearing columns on every content-bearing row (`owner`/`created_by`) + actor attribution on permission-write records — defaulted at V0 single-user, filled from day one (R4). Plain timestamped tables — no TimescaleDB (P-0010 D8). A1-clean: no extension beyond pgvector (P-0010 D2) |
| C4 | **Query path** (host) | Optional HyDE rewrite (4.2, config-gated) → dense (pgvector) + lexical (`ts_rank`) channels → RRF fusion (application SQL, D6 borrow 1) → local rerank (host `rerank`) → budget trim (deliberate-reduction ladder, R2) → envelope assembly (R3) | Policy + workspace predicates in the WHERE clause — the earliest structural point (keystone edge); serving-filter SQL fragments derive from the decision port's attribute vocabulary in exactly one module (R3 mechanic 8); collapsed-tree retrieval searches leaf **and** summary nodes in one KNN (D6 borrow 3) |
| C5 | **Traversal engine** (host) | Recursive CTE over the edge table (P-0010 D4); powers `get_context_for`'s graph position (typed relations, lifecycle state, linked-artifact summaries) | Carries the D4 latency-and-expressiveness logging point; D5's re-open rides the same signal (R11) |
| C6 | **MCP verbs** (host-served on the existing stdio server) | `get_context_for(artifact_id, budget?)` · `search(query, budget?, filters?)` · `get_artifact_by_citation(ref, budget?)` | Host-served, not plugin exports (R1); every verb budget-enforced (R2); auth/dispatch order per P-0012/P-0006 |
| C7 | **Host-fn ABI extension** | Inference primitives `embed / rerank / rewrite-query / summarize / classify` — host-side implementations behind the host-fn seam; `summarize` (synthesis) optional-in-ABI (RC-3; research OQ3) | Shape locks now to avoid an ABI break at plugin activation (research §5.6/OQ5); V0 consumer is the host's own retrieval subsystem (R8) |
| C8 | **Config surface** | Per-placement enable/disable (four independent switches), disclosure-budget default, model-egress policy, FTS language config (per-corpus, default `english` — single-sourced parameter, R6), model/provider config (rides the `0.1.0` LLM-API-key surface) | Zero-egress configuration = all four generative placements OFF + local encoders on (§5); capability surface reports per-placement state (R7) |
| C9 | **Instrumentation** | Retrieval-run records (per-query channels, candidates, rerank scores, budget actions, placement states, latency), per-placement egress counters, extraction/hierarchy coverage, D4 traversal log | Ships with the surface (P-InstrumentBefore); backfillable-first (IB1); the narrowing signals' physical home (R11) |

**Data-flow sketch (default configuration):** index-time — corpus → C1 chunk/tree → local
embed + FTS → (policy-gated) 4.1/4.4 egress → C3 rows; C2 edges → C3. Query-time —
verb call → auth + `WorkspaceCtx` → (4.2 egress, if on) → C4 channels → RRF → rerank →
policy-filtered, budget-trimmed envelope → (4.3 egress, if on and no egress-denied item) →
response.
`get_context_for` additionally drives C5 traversal for graph position.

## 5. Trust boundaries + risk profile (resolved)

The intake flagged "may touch trust boundary" on three surfaces and carried them here as
required constraints. The mechanisms are now known; this section is the security-mode review's
input (P-SecurityLayered design-time layer; the change-time review runs per G-0003).

### Trust boundaries

| Boundary | What crosses it | Controls locked here |
|---|---|---|
| **TB-1: MCP client ↔ server** | Retrieval results assembled from workspace-scoped tables, served outward | Policy enforcement as an authorization control on the retrieval path (R3): every security dimension enforced at its named enforcement points, each decision through the single decision port — `dont_use` excluded structurally at index admission *and* query predicates; visibility-restricted items excluded by the serving predicate per value (`admin-only` serves only callers whose `WorkspaceCtx` role is `Admin` — role-gated at V0, no identity plumbing; `owner-only` serves **no one** at V0 — caller identity is undecidable under the workspace+role context, so the restrictive pole is the V0 semantics); exclusions are caller-silent with an operator-side audit event (no existence-disclosure oracle — Stage-2 review M1). Structural fail-closed: an undecidable security dimension resolves to its restrictive pole. Budget enforcement prevents unbounded disclosure per call (R2). Auth precedes routing; per-verb capability check per P-0012's locked handler contract |
| **TB-2: host ↔ external LLM API** (default configuration only) | See per-placement egress table below | Per-placement independent disable (2a direction 2); policy-aware egress gating at the model-egress gate (`model_egress: deny` and `dont_use` content never egressed); bounded egress: index-time runs carry a per-run document-token budget and concurrency bound, query-time calls carry request-size caps (numbers are spec work); per-placement egress-volume counters (tokens/day) — the same instrument that feeds the research §5.5 local-migration trigger (~14M doc tokens/month). Inbound direction (Stage-2 review M2): corpus content flowing back *through* the generative placements is untrusted data — residual risk (iv) below |
| **TB-3: host ↔ Postgres (tenancy)** | Every new retrieval/edge/envelope/run-record table | P-0006 application-layer enforcement at V0: `WorkspaceCtx` first-parameter threading, single construction site, WHERE-clause-mandatory on every read path including all new channel queries and the recursive CTE (the traversal CTE carries the workspace predicate at every recursion level, not only at the anchor). P-0009/P-0010 RLS preconditions carried for V0.1+ activation: application role holds no BYPASSRLS and is not superuser; `FORCE ROW LEVEL SECURITY` where the role owns tables; tenant key per-transaction via `SET LOCAL`, never session-level |
| **TB-4: host ↔ plugin** | Unchanged | Plugins remain IO-free; inference reached only through host-fns; no plugin receives raw retrieval SQL access. The new inference primitives extend the import ABI shape without touching P-0013's export side |
| **TB-5: host ↔ model-artifact source** (Stage-2 review M4) | Encoder model artifacts (BGE-M3, BGE-reranker-v2-m3) fetched and loaded via `fastembed-rs` (RC-1) | Model-artifact supply chain — distinct from the package dependency tier `fastembed-rs` already clears (§3 P-StackDiscipline): the model-config schema SHALL pin each model's **revision** and verify artifact **integrity** (a fixed digest/signature from a trusted source) before load; no unpinned/`latest` model refs — a swapped or tampered encoder silently changes every embedding and rerank score. Exact pin+verify mechanism is R8 spec work, carried to `{{P-0014}}` |

### Per-placement egress boundary (default, egress-on configuration)

| Placement | Fires | Egresses | Payload sensitivity | Degraded mode when OFF |
|---|---|---|---|---|
| 4.1 chunk-context | Index-time, per chunk on document change | **Document content** (whole document cached + chunk) | Corpus content — policy-gated at the model-egress gate: chunks of `model_egress: deny` artifacts are skipped | Chunks embed without situating context; rows marked context-absent so a later enable re-indexes selectively (backfillable); measured retrieval-lift lost, retrieval functional |
| 4.2 HyDE rewrite | Query-time, per search | **Query text** (caller-supplied) | Query content (may itself be sensitive; the config switch is the control) | Query embedded directly; direct-vector + FTS channels only; abstract-query recall degrades, exact-key retrieval unaffected (research §4.2: HyDE is a fused channel, never the sole path) |
| 4.3 synthesis | Query-time, optional, before response shaping | **Retrieved content** (top-k post-rerank) | Corpus content — `model_egress: deny` items are never included in a synthesis payload; responses containing such items skip synthesis for those items | Raw reranked chunks returned — the research-expected default posture for Claude-class consumers (§4.3); no capability lost, one narrowing signal unreadable |
| 4.4 tag-gen | Index-time, co-located with 4.1 | **Chunk content** | Same gating as 4.1 | Rows carry `tags: absent` (distinguishable from empty); tag-filtered queries return a structured degraded-mode notice, never silently-empty results (P-TrustworthySignal) |

### Sovereignty reconciliation (resolves intake risk profile (b))

The product's self-hostable success criterion ("data never leaving that boundary") is satisfied
at V0 **by configuration**: the zero-egress configuration disables all four generative
placements; hybrid search, rerank, envelope, and traversal run entirely on local non-generative
inference (RC-1: BGE-M3 + BGE-reranker host-side) and MUST work in that mode — this is a
binary-observable requirement (QA-5), not a posture. V0.1+ local generative models arrive
through the same host-fn seam (research §5.7 per-layer targets: Qwen-family locals behind
`rewrite-query`/`summarize`/chunk-context), upgrading degraded mode without ABI change. The
default configuration egresses per the table above and is the documented, deliberate posture for
deployments that accept it (research §5.3/§5.5 triggers name when local migration fires on
cost/latency/sovereignty grounds).

### Threat-model items from the Stage 1c review (each addressed)

1. **Egress trust boundary for all four placements, with sovereignty reconciliation and the
   per-layer local-migration path as the mitigation lever** → the egress table + zero-egress
   configuration + §5.7 migration-path reference above.
2. **Retrieval-path enforcement of `sensitive`/`dont-use` role semantics as an authorization
   control** → TB-1 + R3 (structural placement: index admission + query predicates + envelope
   assembly; fail-closed default) + QA-4.
3. **Workspace-scope isolation on every new retrieval/edge/envelope table (P-0006 app-layer;
   P-0009 RLS preconditions)** → TB-3 + the tenancy invariant row in every storage direction
   (R4, R6) + QA-6.

Residual risks carried forward to the spec's threat model:

- **(i)** Index-time egress happens before serving-time policy checks can help — mitigated by
  the index-admission gate, but a mis-labeled-at-ingest artifact (egress-deny flag absent)
  would egress. **Maintainer-accepted as-is for V0** (pre-gate walk, 2026-07-02): labeling is
  opt-in at V0; `{{P-0015}}` owns labeling correctness and the adopter-facing polarity revisit;
  the serving-time fail-closed posture is unchanged.
- **(ii)** HyDE egresses caller query text, which the policy system cannot inspect — the
  per-placement config switch is the control, stated honestly. **Maintainer-acknowledged**
  (pre-gate walk); no design change.
- **(iii)** The external LLM provider is a trusted third party under the standing API-key
  approval; no new trust decision is made here.
- **(iv) Ingress — indirect prompt injection (Stage-2 review M2).** Corpus content is untrusted
  data, and two generative placements consume it and route their output somewhere
  consequential: 4.1 chunk-context output is *stored* and enriches embeddings/retrieval with no
  mechanical post-validator (unlike tag-gen, which has R9's), and 4.3 synthesis folds retrieved
  content into a digest the consuming agent may act on. Bounded at V0: no ongoing ingest
  (Non-goal 1) removes the untrusted-submitter poisoning vector, and the placements are
  single-shot with no tool access or agency — the residual is first-party-but-adversarially-
  shaped corpus text, which this corpus demonstrably contains (security skills, reviews, and
  research briefs quote injection strings as examples). Named controls carried to the
  ADRs/spec: (a) generative output derived from corpus content is untrusted and, where it feeds
  structured decisions, is mechanically validated — tag-gen (4.4) has this control in R9's
  closed-vocabulary post-validator; chunk-context (4.1) output is free-form situating prose
  with no closed vocabulary to validate against, so the tag-validator pattern does not
  transfer (Stage-2 r2 review N5) — 4.1 carries its own named control requirement, distinct
  from the tag path, whose form `{{P-0014}}`/`{{P-0015}}` pick from: bound the influence of
  situating context on retrieval scoring; treat stored chunk-context as untrusted at scoring
  time; constrain the output to an extractive rather than free-generative form; (b) the
  synthesis contract states its output is model-generated and the consumer treats it as
  untrusted (R7). Detail to `{{P-0014}}`/`{{P-0015}}`/spec.

**Write-authorization surface (Stage-2 review M3; ratified mechanics 5/7).** The
policy-dimension fields, the trust-affecting edges (`supersedes`), and the freshness-override
rows are **authorization-bearing inputs**: the read-path enforcement is only as trustworthy as
the fields it reads, so writes to them are a tamper/write-integrity surface requiring
write-authorization + audit — clearing an egress-deny or use-policy flag makes restricted
content servable (a privilege-relevant write), and recording a spurious freshness override
masks staleness. Structurally answered by the ratified mechanics: every label/permission change
is a recorded, attributable, authorized write (mechanic 7), and every dimension declares the
operation it governs (mechanic 5 — the write side is anticipated structurally, not designed
now). **V0 invariant (Frame-exit gate outcome, 2026-07-02 — §2 gate-outcomes item 14):** at
V0, ALL writes to policy/trust-affecting fields — permission flips, freshness overrides —
ride the existing admin-gated content-mutation path (P-0009), with `read_observer` excluded;
no other write path to these fields exists at V0. This closes the V0 authorization window on
authorization-bearing writes (Stage-2 r2 review N3) and is what makes the §9 write-side
deferral's self-announcing trigger sound as rendered (§13 batched #13): no non-owner actor
holds write/label capability until a named surface grants it. Worked case (joins residual
(iv)): R3's `outdated` predicate fires on an incoming
`supersedes` edge, and R4 permits `origin = extracted` edges derived from free text — so
adversarially-shaped text ("…this supersedes P-0010…") could demote a target's trust value via
the extractor. The predicate therefore weights edge `origin` (R3): a `declared`/`system`
supersedes carries authority; an `extracted` one is a weaker signal, never an automatic trust
flip. The `declared`-origin authority weight itself rests on the first-party-corpus assumption
(Non-goal 1 — no ongoing ingest): frontmatter carries authority because only first-party
authors write it. When ongoing ingest opens (register `1.2.0`), an untrusted submitter
controls a document's frontmatter, and `declared` becomes the same authority-laundering vector
one origin-tier up — the weighting re-opens then, with the trip-wire tied to the `1.2.0`
ingest feature (Stage-2 r2 review N10; §9). Residual design (exact predicates, the write-authz
contract, extraction integrity) lands in `{{P-0015}}`/`{{P-0016}}`.

## 6. Locked architectural directions

Each direction is locked with its canon anchor (decide-and-lock posture; anchors at the
decision-and-rationale line). Directions refine the decomposer-ratified 2a shape; none re-opens
it. QA scenarios in §7 carry the binary measures.

### R1 — Retrieval is a host subsystem; the three verbs are host-served, not plugin exports

**Decision:** The retrieval subsystem (index builder, edge extractor, query path, traversal,
envelope assembly) is host code; `get_context_for`, `search`, and `get_artifact_by_citation`
register on the existing stdio MCP server as host-served verbs, exactly as builtin verbs do.
They are not exports of any `core: true` plugin. *(Anchors: P-0002 — retrieval reads
across every artifact family and cross-plugin aggregation is a projection concern, not a
substrate concern (the discriminator's cross-family clause; the inference-substrate dependency
is supporting context, not P-0002's bootstrap-ordering builtin criterion — §3, Stage-2 review
L4); P-0013 — plugin
domain/non-CRUD verbs are deferred past V0, and host placement keeps that trip-wire unfired;
research §3.1/§5.6 — model lifecycle and inference are host-process concerns; P-0012 — the verbs
ride the locked `rmcp` handler contract.)*

Consequences: no P-0002 partition amendment is needed — the cluster's register entries (1.1.0,
D1, D2) sit outside the `0.2.0`–`0.14.0` increments P-0002 enumerates, and the discriminator is
being applied, not changed. The edge table remains manifest-declared under the `repos` plugin's
content family (`0.8.0`); host-side extractor writes and the ownership semantics are named
`{{P-0016}}` scope (§8) rather than silently resolved here.

### R2 — Verb surface + disclosure-budget mechanics

**Decision:** Three verbs, each budget-enforced: `get_context_for(artifact_id, budget?)` returns
the artifact's typed relations, lifecycle state, and linked-artifact summaries with a full
envelope per item; `search(query, budget?, filters?)` is the first-pass verb returning
summary-grain results (research §2.2 — titles + summaries + citations, not full bodies);
`get_artifact_by_citation(ref, budget?)` resolves a stable citation to its artifact/block
(research §2.4). Budget: **8k-token default, per-call override** — the budget being tunable is
the architectural commitment; the default number is eval-calibrated (research OQ4). Over-budget
candidate sets are reduced **deliberately**, in a fixed ladder: (1) rerank-order the candidate
set; (2) substitute coarser hierarchy nodes for leaf chunks (collapsed-tree resolution shift);
(3) drop lowest-ranked whole items. Mid-item truncation is forbidden. Every response carries a
budget report: requested budget, tokens returned, reduction actions taken, and the citations of
omitted candidates — omission is visible and recoverable via citation expansion, never silent.
One budget-counting function host-side (the tokenizer choice is pinned in the spec). *(Anchors:
intake hard constraint — progressive disclosure is load-bearing physics; intake SC3; research
§2.3/§2.5; P-TrustworthySignal — a response that hides its own reduction lies; P-MinBlastRadius —
one counting function, one assembly point.)*

### R3 — Provenance envelope contract: two axes — trust provenance + a policy permissions record; freshness, displacement

**Decision (redesigned at the pre-gate maintainer walk, 2026-07-02 — decomposer-ratified,
§2):** Round 1's single five-role value is replaced by **two orthogonal axes**: **policy**
(permissions/security — a validator-gated permissions *record*, enforced structurally) and
**trust** (provenance: `authoritative` / `outdated` / `background`, served as a label). The
single-value precedence destroyed information — an item can be sensitive **and** authoritative,
or dont-use **and** background; the two axes carry both facts. Per-axis single-value
determinism is retained: the trust axis yields exactly one value per item; each policy
dimension yields exactly one value, mechanically. Every returned context item carries an
envelope `{trust, freshness, citation}`, with `displacement` state on decision-kind artifacts;
the policy record is an **enforcement input**, not a served label — consulted at the
enforcement points, surfaced to callers only where the contract requires it (the `dont_use`
citation stub's policy reason). Every predicate on either axis remains mechanical — stored
fields and edges only; no LLM call, no prose judgment anywhere in assignment. *(Anchors:
pre-gate walk ratification (§2); intake SC2, implemented per the interpretation note recorded
at §11; intake hard constraint — validator-gated vocabulary; P-ShiftLeft D2 — validator before
field; the sources-with-roles finding (2026-05-21) with the maintainer's two advances;
P-SecurityLayered — the keystone edge; G2/G3 substrate fields (knowledge-object survey,
2026-05-15).)*

**Policy axis — a permissions record under eight locked mechanics.** The maintainer's framing,
verbatim: "capture the hard to change stuff now — the mechanics; the labels are the easy
part." The eight mechanics are the intrinsic contract this Frame locks (P-LockContract; the §2
walk record is the ratification audit trail — carried there verbatim, applied here):

1. **Record shape:** orthogonal, independently-valued permission dimensions per content item —
   never a single enum.
2. **Validator-gated:** every dimension is a mechanical predicate over stored fields/edges; no
   inference anywhere in assignment.
3. **Structural fail-closed:** any undecidable check on a security dimension resolves to its
   restrictive pole — including enforceable-in-principle-but-not-at-V0 (the owner-only V0
   semantics below). Corollary: a settable-but-unenforced security value may not exist in the
   schema.
4. **Named enforcement points:** index-admission gate, model-egress gate (index-time and
   query-time), serving predicate, citation stub. Every dimension declares which point(s)
   enforce it; a dimension without an enforcement point is not a dimension.
5. **Operation facet:** every dimension declares the operation it governs (serve/egress vs
   write/label/supersede). The read/write permission split is anticipated structurally; the
   write side is NOT designed now (maintainer: "maybe that can wait"). Composes with the §5
   write-authorization surface (Stage-2 review M3).
6. **Closed-but-extensible at ADR tier:** adding a dimension is a `{{P-0015}}` amendment
   declaring validator + enforcement point + fail-closed pole + governed operation. Never a
   config knob.
7. **Permission-field writes are an authorization surface:** label/permission changes are
   recorded, attributable writes. (Stage-2 review M3 is the independent corroboration;
   residual design lands in `{{P-0015}}`/`{{P-0016}}`.)
8. **Single decision port, Cedar PARC shape:** all enforcement points consult one boundary
   `decide(principal, action, resource, context) → allow/deny + reason`. V0 implementation =
   the hardcoded validator predicates behind the port. Conformance invariants (fail-closed;
   attributes-only) are tested AT THE PORT (design-time conformance suite, D5 two-adapter-test
   pattern). Row-level serving filters: SQL fragments derived from the port's attribute
   vocabulary in exactly ONE module (single-sourced), so a future engine swap replaces one
   module. *(Anchors: the Cedar/NGAC permission-model research (maintainer record, locked
   2026-05-18) — "the engine sits behind mnemra's policy port… port substitution, not a
   rewrite"; cedar-policy 4.x, Apache-2.0/Green; P-LockContract; G-0017 FlagProvider precedent;
   P-0010 D5 Storage-trait precedent.)* Maintainer's stated intent: bound the blast radius of
   the future flexible-policy engine to the PDP module — QA-9 renders this as the port-swap
   scenario.

**Mechanic-8 rendering precisions (Stage-2 r2 review N1/N7/N8; the ratified word sequence
above is untouched).** *Serving-path equivalence (N1):* the serving-filter SQL fragment
executes in Postgres and cannot consult `decide()` per row, so it is a derived enforcement of
the same policy, not a port consultation. The Frame states the binary requirement and
`{{P-0015}}` picks its form: either (a) the fragment is **mechanically generated** from the
port's predicate definitions — no independent serving logic exists to drift — or (b)
`{{P-0015}}` mandates a **differential conformance test**: the fragment and `decide()` return
identical allow/deny over a generated resource set covering every dimension × pole ×
undecidable case. Absent one of these, port-level conformance does not cover the serving path
(QA-9 measures 1 and 4 render this). *Index-time principal (N7):* the index-admission gate and
the index-time model-egress gate fire during batch index builds with no caller principal; they
consult the port under a named **system/service principal** (exact form is `{{P-0015}}` work),
so the PARC shape is well-defined at every named enforcement point. *Citation scope and port
cardinality (N8):* the Cedar/NGAC research's 1:1 PARC-fit finding is scoped to the
host-function capability gate (`principal = plugin`); cited here it validates the port pattern
and PARC-shaped decision boundary generally, not a content-policy-specific fit. The
content-policy decision port is a **distinct per-domain PDP instance** sharing the port
pattern with the host-fn capability gate — unification, if ever, is `{{P-0015}}`/host-fn-gate
ADR territory (the minimal-coupling reading of P-MinBlastRadius; §13 batched #16).

**Proposed initial dimension set — labels demoted; `{{P-0015}}` finalizes.** Dimension names
and value enums are deliberately the easy-to-change part; this table is a **proposed initial
set**, not a lock. Each proposed dimension already conforms to mechanics 2–5 (validator +
enforcement point(s) + fail-closed pole + governed operation) so the demotion is of *names*,
never of mechanics:

| Dimension (proposed) | Values (proposed) | Validator (proposed predicate) | Enforcement point(s) | Fail-closed pole | Governed operation |
|---|---|---|---|---|---|
| `dont_use` | set / not set | Explicit use-policy field = `deny` (G3 substrate field; author/curator-set, never inferred) | Index-admission gate + query predicates + citation stub | Excluded — the curatorial kill switch: never indexed (never chunked, embedded, or egressed), never served; direct citation resolution returns a metadata-only stub carrying the policy reason | serve/egress |
| `model_egress` | `allow` / `deny` | Explicit egress field (G3 sensitivity-family substrate field; never inferred from content) | Model-egress gate — index-time (chunk-context/tag-gen inclusion) and query-time (synthesis payloads) | `deny` | egress |
| `visibility` | `workspace` / `admin-only` / `owner-only` | Explicit visibility field (audience within workspace); `admin-only`: the caller's `WorkspaceCtx` role is `Admin` (role-gated, no identity plumbing — gate outcome, §2 item 15) | Serving predicate + citation stub | `owner-only` (at V0: serve to no one — below) | serve |
| `tenant_share` | `workspace-only` (fixed at V0) | Structural constant — a declarative hook over structural tenancy (P-0006); the future sharing capability must honor per-content holdbacks | Serving/tenancy predicates | `workspace-only` | serve/share |

`sensitive` as a *name* dissolves into `model_egress: deny` + `visibility: admin-only` (gate
outcome, §2 item 15 — the round-1 read semantics restored: admin-readable, observer-withheld)
— whether it survives as a macro is `{{P-0015}}`'s call. Round 1's five validators survive as
predicates: they now write to two axes instead of competing for one value (the dont-use and
sensitivity predicates feed the policy record above; the remaining three feed the trust axis
below). `model_egress: deny` content is still indexed **locally in full** — local embed/rerank
are egress-free per RC-1 — so the policy record and the egress boundary reinforce rather than
fight. `dont_use`, `model_egress`, and `visibility: admin-only` are enforced at V0 and need
no identity — `admin-only` is role-gated through the existing `WorkspaceCtx` role enum.

**V0 semantics — `visibility: owner-only` serves no one** (ratified; the fail-closed
application of mechanic 3): caller identity is undecidable under V0's workspace+role context,
so owner-only items are excluded from all retrieval — for every caller — until identity lands.
This is NOT a settable-but-unenforced value (mechanic 3's corollary forbids that): the
conservative enforcement IS the V0 semantics. Per-user identity machinery is **out** of this
cluster (ratified; a named future register entry — §8): owner columns as an identity model,
caller identity in context, and real owner==caller checks are not built here. Intake Non-goal 6
("enforcement posture unchanged, P-0006/P-0009") holds: retrieval-path use-policy controls over
stored fields are this cluster's blessed scope (intake risk profile (a)); tenant-enforcement
machinery expansion is what stays fenced.

**Enforcement-layer asymmetry, stated deliberately (Stage-2 r2 review N6):** `dont_use` gets
dual-layer defense (index admission + query predicates — a curatorial kill; the content must
not exist in the index at all), while `visibility` enforces at serve time only, with the
content fully indexed — because visibility is an audience restriction *within* the workspace,
not a curatorial kill: the content is legitimately indexed and who may read it varies by
caller, and at V0 the coarse `owner-only` pole serves every caller nothing. When identity
lands and `owner-only` becomes per-user, the serving predicate is the entire control for
audience-restricted content sitting in a shared index — whether that warrants an
index-partition / admission-level control is flagged for `{{P-0015}}`.

**Temporality — owner-mutable flip only** (ratified): permission changes are explicit,
recorded, authorized writes (mechanic 7; the §5 write-authorization surface). No auto-expiry at
V0; a timed embargo is recorded as a `{{P-0015}}` design option, unbuilt, awaiting a real case
(§9).

**Trust axis (semantics carried from round 1 unchanged):** one value per item, assigned by
first-match precedence `outdated` → `authoritative` → `background` (round 1's relative order
with the policy labels now off this axis; round 1's `authoritative` clause "not policy-flagged"
is removed — axis orthogonality is the ratified point):

| Trust value | Mechanical validator (predicate over stored data) | Retrieval-path semantics |
|---|---|---|
| `outdated` | (a) an incoming `supersedes` edge exists in the edge substrate — **weighted by edge `origin`** (§5 write-authorization surface): a `declared`/`system` supersedes carries authority; an `extracted`-from-free-text edge is a weaker signal, never an automatic trust flip (exact weighting is `{{P-0015}}`/`{{P-0016}}` work; the `declared` weight rests on the first-party-corpus assumption and re-opens with `1.2.0` ingest — §5) — OR (b) freshness state is stale — version-handle diff shows the cited source moved, or the decay-class TTL expired without re-validation — AND no recorded freshness override | Served with the trust label (agents may still need superseded context); default ranking demotes — except hard-superseded nodes (an incoming `supersedes` edge with the `superseded-by` pointer), which R6 excludes from default retrieval (reconciliation stated at R6); point-in-time queries reach it deliberately (keyed-supersession pattern, D6 borrow 3) |
| `authoritative` | The artifact kind's lifecycle field is in that kind's closed authoritative-state set (per-kind mapping table in `{{P-0015}}`, e.g. ADR `accepted`, spec/brief/research `locked`) AND not `outdated` | Served, ranked as trust-primary |
| `background` | No other trust predicate fires (the total-function default) | Served with the trust label |

**Freshness (per the maintainer's advance 1, 2026-05-21):** version-handle diff is the primary
staleness check — where an item cites a source with a handle (canon git SHA, dependency version,
model ID, paper version), the envelope stores the handle-at-index and reports
`current | moved | unknown` by diffing against the source's live handle. Decay-class TTL is the
fallback for handle-less sources; the TTL does not measure staleness, it **bounds ignorance**,
with cadence matched to domain volatility (the class table is `{{P-0015}}` content). Time is the
weakest signal — fallback, never primary. Override is **structurally required**: a freshness
override is a recorded row `{by, reason, date}`, never a config toggle; content-based staleness
detection is rejected as circular.

**Displacement ≠ staleness (per the maintainer's advance 2):** handle-diff catches
my-cited-source-changed, not the-world-moved-past-it. Containment, not solution: a **named
displacement-event registry** (enumerable events) fires re-evaluation of decisions whose
recorded axes intersect the event; a fired event sets a `re-eval pending (axis)` flag surfaced
in the envelope — **single-axis displacement triggers re-eval, never auto-invalidation** (the
trust value does not flip on a displacement flag; multi-axis decisions survive a single-axis
shift).
Decision-kind artifacts therefore **record their axes** as substrate data. Honest limit,
carried verbatim: trip-wires only cover enumerable displacement events; truly unforeseen shifts
fall back entirely to the volatility TTL.

**Citation:** every envelope item carries a stable citation — artifact ID (+ block anchor where
the item is a chunk) — resolvable by `get_artifact_by_citation`. *(Anchor: P-AgentPrimarySource —
ID+name pairing, block addressability; V0 content-addressed artifact IDs.)*

### R4 — Edge substrate: one superset table with discriminating provenance semantics

**Decision:** One edge table (extending the `0.8.0` substrate — it is the storage this cluster
extracts into) with the **superset closed vocabulary**
{`parent`, `blocks`, `depends-on`, `dispatched-by`, `extends`, `feeds`, `cites`, `supersedes`};
`supersedes` unifies the two prior sets. The discriminating mechanism is **two closed-enum
columns**: `edge_class` — `work-graph` (parent / blocks / depends-on / dispatched-by) vs
`citation` (extends / feeds / cites), with `supersedes` classed per its subject at write time —
and `origin` — `declared` (authored in structured frontmatter or a DB relation), `extracted`
(derived from free-text by C2, carrying a source-span provenance pointer back to the text it was
read from), or `system` (created by system operations, e.g. `dispatched-by`). Work-graph edges
are system-of-record rows created transactionally with their entities; citation edges are
re-derivable — extraction is idempotent, and **extraction coverage over the migrated corpus is
measured and reported** (per-source counts: how many frontmatter relations and free-text
citations resolved to edges vs failed to resolve), never assumed. One traversal + one extraction
code path. Tenancy invariant: `workspace_id NOT NULL`, indexed, explicitly passed; traversal
CTEs carry the workspace predicate at every level. Identity invariant (retrofit-prep, §2
pre-gate walk item 11): every content-bearing row this cluster adds carries
`owner`/`created_by`, and permission-write records carry actor attribution — coarse or
defaulted at V0 single-user; the columns exist and fill from day one so identity arrival is a
feature, not an excavation. *(Anchors: 2a direction 1
(decomposer-ratified); Stage 1c F6 — superset preferred per vocabulary-consistency
(P-MinBlastRadius); intake SC5; P-0010 D4 — recursive CTEs serve the shallow edge model; D6
borrow 4 — borrow the graph model, not a graph engine; P-0006 tenancy.)*

Exact DDL, uniqueness constraints, the `supersedes` classing rule, the `origin`-weighting of
trust-affecting edges + the extraction-integrity contract (§5 write-authorization surface), and
the plugin-manifest-ownership question (host extractor writing into the `repos`-declared family)
are `{{P-0016}}` scope (§8).

### R5 — Tree-build seam: authored-first as default, under a named eval

**Decision:** Tree construction sits behind a `TreeBuilder` seam — one contract producing one
node shape (leaf chunks → parent-linked summary nodes at section and document grain), with two
strategies designed against it: `authored` (markdown-header hierarchy populates the tree
directly) and `clustered` (RAPTOR-style embed-cluster-summarize). **V0 builds the `authored`
strategy only** for high-structure shapes (ADRs, briefs, specs, skills), with low-structure
shapes (daily logs, code, rows) indexed at their natural single grain (research §1.2); the
`clustered` strategy is a designed-but-unbuilt slot behind the same seam. Authored-first ships
as the default **as an eval hypothesis, not a validated lock** — the canon-free cross-check
(2026-06-05) refuted the stronger authored-beats-clustered claim and found no documented
precedent for the hybrid. *(Anchors: 2a required constraint (decomposer-ratified); research
§2.1 adapted form + cross-check addendum; P-LockContract ⇄ P-PreserveDecisionSpace when-to-lock
discriminator — the seam is intrinsic and locks, the winner is separable and preserved;
P-Defer/DF1.)*

**The named eval — `hierarchy-source eval`:** *hypothesis* — on the migrated corpus, the
authored tree meets or beats a clustered tree on retrieval-failure rate at equal disclosure
budget. *Signal* — (a) per-strategy retrieval-failure rate / nDCG on the golden query set (the
same harness instrument P-0010 D3's `ts_rank`-vs-BM25 trip-wire uses — one harness, two
consumers), and (b) an index-time **hierarchy-coverage measure**: the fraction of corpus tokens
under authored hierarchy at section grain or better, reported per shape at every index run
(backfillable — computable from the existing corpus on day one, IB1). *What fires the eval
read* — the code-aware retrieval eval is a named deliverable of the committed-tier plan (the
intake's non-goal 4 firing mechanism); its first run reads both signals. If coverage is low or
the clustered arm wins, the pre-designed `clustered` strategy is built behind the seam — a
bounded change (P-MinBlastRadius), not a re-architecture.

### R6 — Hybrid search: FTS + dense fused by RRF; the D6 method-borrows land

**Decision:** Search runs two channels at V0 — lexical (Postgres native `tsvector`/`ts_rank`)
and dense (pgvector HNSW over all tree levels, collapsed-tree style) — fused by Reciprocal Rank
Fusion as application-side SQL, followed by local cross-encoder rerank into budget. Exact-key
queries (decision IDs, file paths, function names) resolve via the lexical channel — the query
class pure-vector retrieval loses by mathematics. The keyword leg is deliberately **not BM25**
at V0: P-0010 D3's fidelity trip-wire (golden-query `ts_rank`-vs-BM25 regression) is the
instrument, and `pg_textsearch` the named Green upgrade path. The FTS language configuration is
a **single-sourced per-corpus parameter** (default `english`), read wherever a
`tsvector`/`tsquery` is built — never inline `to_tsvector('english', …)` literals scattered
across schema, queries, and indexes (retrofit-prep, §2 pre-gate walk item 12: multilingual FTS
becomes a parameter change, not an excavation; multilingual *dense* retrieval is already native
to BGE-M3). Additional channels (fact-key,
raw-message, ColBERT late-interaction from BGE-M3's multi-functional output) are known extension
points, added only as their cost is justified. This cluster is **P-0010 D6's named firing
event**; all four method-borrows land in `{{P-0014}}`: (1) single-query BM25+dense+RRF fusion
as the ergonomic reference target for the application-fusion SQL; (2) `pg_textsearch` as the
named Green BM25 upgrade (D3); (3) collapsed-tree / multi-resolution embeddings +
keyed-supersession-via-normalized-topic-key SQL patterns; (4) borrow the graph *model*, not a
graph *engine*. *(Anchors: intake SC4; P-0010 D2/D3/D6; research §1.3 + cross-check
corroboration; storage substrate hard constraint.)* Superseded nodes acquire the
`superseded-by` forward pointer and are excluded from default retrieval, reachable for
point-in-time queries (borrow 3; composes with R3's `outdated` trust value). The
demote-vs-exclude reconciliation, stated once (Stage-2 r2 review N9): a freshness-stale
`outdated` item (R3 trust predicate (b), no superseding node) is served-and-demoted; a
hard-superseded node (an incoming `supersedes` edge with the `superseded-by` forward pointer)
is excluded from default retrieval; both remain point-in-time reachable.

### R7 — Four LLM placements, individually degradable; degraded modes specified

**Decision:** All four generative placements ship as specified surfaces (RC-3; no narrowing
decisions in this artifact), each behind an **independent config switch**, each with the
degraded-mode behavior specified in §5's table (context-absent marking for 4.1; direct-embed
fallback for 4.2; raw-chunks return for 4.3 — the expected steady default for Claude-class
consumers; `tags: absent` with structured degraded-notice for 4.4). Synthesis (4.3) is
**optional-in-ABI** (research OQ3): the `summarize` primitive is declared but its availability
is a capability the caller can probe; absence is a structured state, not an error surprise. The
synthesis contract additionally states its output is **model-generated**: the consuming agent
treats the digest as untrusted content, never as instructions (§5 residual (iv); detail to
spec).
Degraded-mode honesty is contractual: config state is visible on a capability surface, `absent`
is distinguishable from `empty` in every schema this cluster adds, and any query whose semantics
a disabled placement changes returns a structured notice. The zero-egress configuration is these
four switches OFF plus local encoders on, and is a **supported V0 configuration** (2a direction
2), verified by QA-5. *(Anchors: RC-3 + the maintainer's "all of the above for V0, narrow
later" ruling (2026-05-27); 2a direction 2 (decomposer-ratified); research §4; P-TrustworthySignal;
P-LockContract — provider varies behind the config seam.)*

### R8 — Host-fn ABI extension: five inference primitives, host-implemented

**Decision:** The host-fn surface extends with five inference primitives —
`embed(text) -> vector`, `rerank(query, candidates) -> scored-indices`,
`rewrite-query(text) -> text`, `summarize(query, chunks) -> digest` (optional-in-ABI),
`classify(text, closed-vocabulary) -> tags` — implemented host-side (local ONNX via
`fastembed-rs` for embed/rerank; configured external API for the generative three at V0), never
inside a plugin (ONNX is a native dependency the WASM sandbox cannot host; one model serves all
consumers). The **shape locks now** even though the V0 consumer is the host's own retrieval
subsystem, so plugin-facing activation at V0.1+ is additive, not an ABI break. These are
import-direction primitives (what a caller calls *on* the host); P-0013's export-side contract
is untouched. Exact WIT signatures and the model-config schema are spec work; the model-config
schema SHALL pin each model's **revision** and verify artifact **integrity** (a fixed
digest/signature from a trusted source) before load — no unpinned/`latest` model refs (TB-5;
Stage-2 review M4; the pin+verify mechanism carries to `{{P-0014}}`/spec). *(Anchors:
research §3/§5.6 + OQ5; RC-1 — local non-generative inference permitted host-side, generative
never hosted; P-0002/P-0012/P-0013 — IO-free cores, host-mediated IO; P-LockContract — model
choice varies behind the primitive; P-StackDiscipline S1 — `fastembed-rs` is the in-stack path,
all Green tier.)*

### R9 — Tag vocabulary: closed set, prompt-enforced and post-validated

**Decision:** Tag generation (4.4) draws from a **closed vocabulary** enforced twice: the
generation prompt constrains choices to the set, and a mechanical post-generation validator
discards any out-of-set tag and counts the discard (the drift metric — research §4.4's failure
mode made observable). The initial candidate set, worked against the corpus shapes (final set
locked in the spec with worked examples per tag): `decision` (an ADR Decision section),
`requirement` (a spec R-ID block), `rule` (a principle's mechanism rule), `convention` (a
skill/profile operational rule), `research-finding` (a brief's verdict block), `task` (a task
row), `event` (an activity/audit row), `code-symbol` (a function/struct chunk),
`principle-anchor` (a chunk citing P-*/G-* identifiers), `use-case` (a captured-trace record).
Ten tags; eight-to-twelve is the research-plausible envelope. Vocabulary changes are deliberate
spec-governed enum changes, never free-form drift. Per-tag filter precision/recall is the
narrowing signal; tags never used in filters are deadweight and candidates for removal.
*(Anchors: research §4.4 + OQ6 (a genuine novel-design call — zero external evidence, per the
cross-check); P-ShiftLeft D2 — validator before field, applied to each tag slot;
P-MinBlastRadius — one noun per concept.)*

### R10 — Index granularity: per-shape internal policy, no adopter-facing ABI flag at V0

**Decision:** Index granularity (hierarchical for high-structure shapes; single-grain for logs,
code, rows) is an **internal per-shape policy table** in the index builder, not a per-corpus-
shape ABI/config flag. No adopter-facing granularity flag ships at V0. Deferral (research OQ2)
with named firing: (a) *self-announcing* — an adopter asks for storage-constrained single-level
indexing (the need cannot arise unasked); (b) *instrumented* — the index-time storage-overhead
measure (summary-node storage as a multiple of leaf storage, expected ~1.5–2x on high-structure
shapes) crossing a spec-named threshold flags the flag's design. Until either fires, the
smallest mechanism holds. *(Anchors: P-Defer/DF1; Simplicity — every config knob is a mechanism
every reader accounts for; research §1.2 cost bound.)*

### R11 — Instrumentation: narrowing signals' physical home; D4/D5 instruments placed

**Decision:** The narrowing signals live as **structured retrieval-run records in plain
timestamped, workspace-scoped Postgres tables** — product measurement data in the same family as
dispatch metrics, not ephemeral telemetry — because the eval that narrows placements is a
*consumer* of this data (operational telemetry is additionally emitted per the observability
baseline; generation and storage stay separate per P-0010 D8/E1 — these tables are the
content-substrate's own measurement shape, not an observability store). Per-query records
capture: channels fired, candidate sets and rerank scores, budget actions, per-placement on/off
state, latency per stage — enough to recompute A/B metrics offline (IB1 backfillable-first;
where a signal structurally requires index variants — 4.1's contextual-ON/OFF — the record
notes the index build it ran against, and the eval harness owns the variant comparison). The
four placement signals (research §4.5 table) read from these records. **D4's instrument is
placed on C5**: the multi-hop CTE traversal path logs query latency and flags any traversal the
CTE cannot express or serves above the acceptable bound — the logged dogfood incident that fires
Apache AGE adoption; **D5's second-adapter re-open rides the same signal** (if D4 fires, D5
re-evaluates at the same time, per P-0010). Per-placement egress-volume counters (tokens
egressed/day) doubly serve as the §5 boundary instrument and the research §5.5 migration trigger;
volume counters answer "how much," and the **query-time egress content-audit** — which query
text (4.2) or which retrieved chunks (4.3) were sent, keyed for incident response — is named as
a spec/`{{P-0015}}` completeness item (Stage-2 review L5).
*(Anchors: P-InstrumentBefore + IB1; intake SC6; P-0010 D4/D5 (carried deferral instruments —
consumed here, not re-derived); P-0010 D8 — plain tables; RC-3's premise — "narrow later" is
only honest if the signals exist.)*

## 7. Quality-attribute scenarios (ATAM)

Each scenario is `[stimulus · environment · response · measure]`; each measure is a conjunction
of binary checks.

### QA-1 — One-call context bundle (the reference trace)

- **Stimulus:** an agent calls `get_context_for` on a task artifact matching the 2026-06-05
  reference trace (an in-progress research task whose brief, prior briefs, and fed decision
  records exist in the corpus).
- **Environment:** default configuration; migrated corpus indexed; edges extracted.
- **Response:** one call returns the bundle the trace reconstructed by hand.
- **Measure (all must hold):**
  1. The response contains the artifact's typed relations (at minimum the trace's `extends`,
     `feeds`, `cites` edges), its lifecycle state, and linked-artifact summaries — in **one**
     MCP call.
  2. The trace's brief-complete-vs-open-research distinction is answerable from the response
     alone (lifecycle state + graph position), with zero follow-up file reads required.
  3. Every returned item carries a complete envelope (`trust`, `freshness`, `citation`).
  4. The call replaces ≈4 tool calls + 3 reads per task (intake SC1) — the fixture reproduces
     the trace as one call each.

### QA-2 — Disclosure budget enforced; reduction is deliberate and visible

- **Stimulus:** a `search` whose candidate set exceeds the effective budget (default 8k or a
  per-call override).
- **Environment:** any configuration.
- **Response:** the response fits the budget via the reduction ladder; omission is reported.
- **Measure (all must hold):**
  1. Returned payload token-count ≤ the effective budget (single host-side counting function).
  2. No returned item is mid-content truncated (every item is whole at its resolution — leaf or
     summary node).
  3. The budget report names the reduction actions taken and carries citations for omitted
     candidates; each omitted citation resolves via `get_artifact_by_citation`.
  4. A per-call override changes the effective budget for that call only.

### QA-3 — Exact-key retrieval survives hybrid fusion

- **Stimulus:** a `search` for an exact key (a decision ID such as `P-0010`, a file path, a
  function name).
- **Environment:** default configuration; HyDE ON.
- **Response:** the lexical channel resolves the key; fusion does not bury it.
- **Measure (all must hold):**
  1. The artifact/chunk carrying the exact key ranks in the returned set (the lexical channel's
     catch survives RRF + rerank).
  2. The same query with HyDE OFF also resolves (the rewrite is a fused channel, never a
     gating dependency — research §4.2).

### QA-4 — Policy enforcement on the retrieval path (adversarial)

- **Stimulus:** the corpus contains five marked artifacts — one `dont_use`, one
  `model_egress: deny`, one `visibility: owner-only`, one `visibility: admin-only`, and one
  control item (`visibility: workspace`, `model_egress: allow`) — plus two artifacts each
  carrying one undecidable security dimension (the field the validator reads is malformed or
  absent where required): one undecidable-`model_egress`, one undecidable-`visibility`.
  Callers of every available `WorkspaceCtx` role issue `search` and `get_context_for` queries
  matching all seven, plus a direct `get_artifact_by_citation` on each.
- **Environment:** default configuration.
- **Response:** every security dimension enforces at its named enforcement point(s),
  fail-closed and caller-silent; the control item serves normally.
- **Measure (all must hold):**
  1. The `dont_use` artifact appears in **no** result set for any caller and has no rows in the
     chunk/vector/FTS index tables (excluded at admission, not filtered at serve time); its
     citation resolution returns a metadata-only stub (policy reason), no content.
  2. Under the default egress config, no chunk of the `model_egress: deny` artifact was ever
     sent to the external LLM API (index-time and query-time egress logs show zero payloads for
     it) — while the artifact remains locally indexed and retrievable (RC-1 local encoders).
  3. The `visibility: owner-only` artifact is absent from **every** caller's results —
     including `Admin` — at V0 (owner-only serves no one; caller identity undecidable), and the
     withholding is caller-silent (no count, no placeholder in the response) with an
     operator-side audit event emitted.
  4. Citation resolution of the `visibility: owner-only` artifact by any caller discloses no
     content and matches the `{{P-0015}}`-locked disposition (stub vs not-found — the
     existence-disclosure call named for `{{P-0015}}`; Stage-2 review L2).
  5. Each undecidable security dimension resolves to **its** restrictive pole (mechanic 3 is
     per-dimension, never whole-item): the undecidable-`visibility` artifact is withheld from
     serving for every caller (restrictive pole `owner-only` — serve-deny at V0); the
     undecidable-`model_egress` artifact is never egressed (restrictive pole `deny`) while
     remaining locally indexed and served, exactly like measure 2's egress-deny item.
  6. The control item is returned to workspace callers with a complete envelope (the gate can
     fail in the over-blocking direction too).
  7. Every allow/deny above is a `decide(principal, action, resource, context)` decision with a
     recorded reason — no enforcement point bypasses the port (mechanic 8; the serving
     predicate enforces through its derived fragment per the mechanic-8 rendering — QA-9
     measures 1 and 4).
  8. The `visibility: admin-only` artifact is returned to the `Admin`-role caller and absent
     from the `read_observer` caller's results (admin-sees / observer-withheld — the gate
     outcome restoring round-1 read semantics, §2 item 15); the withholding is caller-silent
     with an operator-side audit event, and citation resolution by the withheld caller follows
     the same `{{P-0015}}`-locked stub-vs-not-found disposition as measure 4.

### QA-5 — Zero-egress configuration is functional (sovereignty)

- **Stimulus:** the operator sets the zero-egress configuration (all four generative placements
  OFF); the host indexes the corpus and serves all three verbs; the reference-trace fixture runs.
- **Environment:** network egress denied to everything except loopback (test harness enforces).
- **Response:** retrieval is degraded but fully functional with zero egress.
- **Measure (all must hold):**
  1. Indexing completes: chunking, authored-tree build, local embedding, FTS, edge extraction
     all succeed with no outbound connection attempted.
  2. All three verbs answer: hybrid search + rerank + envelope + traversal work (the QA-1
     fixture passes in this mode).
  3. Degradation is visible: the capability surface reports all four placements OFF; indexed
     rows carry context-absent/tags-absent markers; a tag-filtered query returns the structured
     degraded-mode notice.
  4. Re-enabling a placement later selectively re-enriches (the absent markers drive the
     backfill) without a full re-architecture of the index.

### QA-6 — Tenancy isolation on the new tables

- **Stimulus:** two workspaces exist; workspace B's caller issues every retrieval verb, plus a
  multi-hop `get_context_for` traversal, against artifacts that exist only in workspace A.
- **Environment:** V0 application-layer enforcement (P-0006); new tables carry the RLS
  column shape.
- **Response:** no cross-workspace row is readable through any retrieval path.
- **Measure (all must hold):**
  1. Every new table (chunks, summary nodes, edges, envelope fields, run records) carries
     `workspace_id NOT NULL` with an index, populated on every row.
  2. Workspace B's calls return zero workspace-A items on every channel (FTS, dense, RRF-fused,
     traversal) — the workspace predicate is in each channel's WHERE clause and at every
     recursion level of the traversal CTE.
  3. Every retrieval host-fn takes `WorkspaceCtx` as its first parameter; construction remains
     at the single locked site.
  4. The schema satisfies the P-0009/P-0010 RLS preconditions (no BYPASSRLS/superuser role;
     FORCE RLS applicable; per-transaction `SET LOCAL` compatible) so V0.1+ policy activation
     is additive.

### QA-7 — Freshness: a moved version handle surfaces as outdated

- **Stimulus:** an indexed research artifact cites a canon document at SHA `X`; the canon
  document advances to SHA `Y`; the artifact is then retrieved.
- **Environment:** default configuration; no freshness override recorded.
- **Response:** the envelope reports the staleness mechanically.
- **Measure (all must hold):**
  1. The item's envelope reports freshness mode `version-handle`, stored handle `X`, source
     state `moved`.
  2. The item's trust validator yields `outdated` (predicate (b) of R3's trust table) without
     any LLM or prose judgment in the path.
  3. Recording a freshness override (`{by, reason, date}`) — a write accepted only through
     the admin-gated content-mutation path (P-0009), refused for a `read_observer` token (the
     gate-confirmed V0 write invariant, §5 / §2 item 14) — restores the prior trust value
     **and** the override is itself visible in the envelope.
  4. A displacement-registry event on one axis of a multi-axis decision sets `re-eval pending`
     on that decision's envelope without flipping its trust value (re-eval, never
     auto-invalidation).

### QA-8 — Traversal instrument (D4/D5 carried deferral)

- **Stimulus:** a multi-hop `get_context_for` traversal runs (any depth ≥ 2).
- **Environment:** default configuration.
- **Response:** the D4 instrument observes it.
- **Measure (all must hold):**
  1. The traversal emits a log/run record carrying hop count and latency.
  2. A traversal exceeding the acceptable latency bound, or one the CTE path cannot express, is
     flagged in the record (the D4 firing condition is mechanically detectable from stored
     data, not from someone remembering).
  3. The record is queryable as the shared signal D5's second-adapter re-open reads.

### QA-9 — Decision-port blast radius (engine substitution)

- **Stimulus:** the V0 hardcoded validator predicates behind the policy decision port are
  replaced by a policy engine (the port's designed substitution — a Cedar-class PARC engine).
- **Environment:** the port conformance suite (fail-closed + attributes-only invariants; the
  P-0010 D5 two-adapter-test pattern) exists from V0 and runs against any implementation behind
  the port.
- **Response:** enforcement behavior is preserved; the change is bounded to the
  policy-decision-point module (the maintainer's stated intent for mechanic 8).
- **Measure (all must hold):**
  1. Every host-side enforcement point (index-admission gate, model-egress gate, citation
     stub) reaches its policy decisions only through the single
     `decide(…) → allow/deny + reason` boundary — the index-time gates under the named
     system/service principal (no caller principal exists at batch index time) — and none
     carries its own policy logic (one consulting seam, verifiable by inspection at V0). The
     row-level serving predicate is the deliberate exception: it executes in the database and
     cannot consult the port per row; it enforces through the derived SQL fragment, whose
     equivalence to `decide()` is guaranteed by measure 4.
  2. Row-level serving-filter SQL fragments derive from the port's attribute vocabulary in
     exactly **one** module; the substitution replaces that module and the PDP implementation
     and nothing else (the diff is bounded to those modules).
  3. The port conformance suite — including the seeded undecidable-security-dimension cases
     (QA-4's undecidable-`model_egress` and undecidable-`visibility` artifacts), each of which
     must resolve to **its** restrictive pole through any implementation — passes against the
     V0 predicate implementation, and passes unchanged against a replacement at substitution
     time (the two-adapter pattern).
  4. The serving-path equivalence requirement holds in whichever form `{{P-0015}}` mandates
     (the mechanic-8 rendering states the binary): either the serving-filter SQL fragment is
     mechanically generated from the port's predicate definitions, or the differential
     conformance test passes — the fragment and `decide()` return identical allow/deny over a
     generated resource set covering every dimension × pole × undecidable case.

## 8. Open ADR slots

Three focused ADRs, per the ratified 2a direction 3. Placeholders only — the ADRs are **not**
authored here; they resolve at Stage 3 per the placeholder-resolution convention
(`docs/src/adrs/placeholder-resolution.md`), taking the next free numbers in reservation order.

### `{{P-0014}}` — Retrieval architecture

Locks the retrieval topology this Frame directs: per-shape chunking policy table (research
§1.1), collapsed-tree multi-resolution index, hybrid channel set + RRF fusion shape, rerank
stage, the `TreeBuilder` seam + hierarchy-source eval (R5), disclosure-budget mechanics (R2),
verb surface + host placement (R1/R2), degraded-mode matrix (R7), and the four **P-0010 D6
method-borrows** — this cluster is D6's named firing event, and their landing here closes that
deferral: (1) single-query BM25+dense+RRF as the fusion reference target; (2) `pg_textsearch`
as the named Green BM25 upgrade path (D3's trip-wire instrument); (3) collapsed-tree /
multi-resolution embeddings + keyed-supersession-via-normalized-topic-key SQL patterns; (4)
borrow the graph model, not a graph engine. Also records the R10 granularity posture, the R11
instrumentation shape, the R8 model-config pin+verify contract (TB-5; Stage-2 review M4), and
the typed-DFD extension (Stage-2 review L3): the cluster's new elements typed against the
architecture-overview DFD — new tables as data stores, the three verbs as processes, the
external LLM API and the model-artifact source as external entities, the four egress calls as
flows crossing TB-2 and the model-artifact fetch as a flow crossing TB-5 — so each new element
gets per-element STRIDE coverage rather than the boundary-level treatment §5 gives. Also lands
(with `{{P-0015}}`) the 4.1-specific residual-(iv) control named at §5: chunk-context output
is free-form prose the tag-validator pattern cannot cover, so the ADR picks its control from
the §5 candidate set (Stage-2 r2 review N5).

### `{{P-0015}}` — Provenance envelope + source-roles contract

Locks the two-axis envelope contract this Frame directs (R3). **Policy side:** the final
dimension names and value enums (R3's proposed initial set — `dont_use`, `model_egress`,
`visibility`, `tenant_share` — is deliberately easy-to-change here), each dimension's mechanical
validator, enforcement point(s), fail-closed pole, and governed operation (mechanics 2–6);
whether `sensitive` survives as a macro over `model_egress: deny` + `visibility: admin-only`
(the gate-outcome dissolution, §2 item 15); the decision-port contract — the
`decide(principal, action, resource, context)` boundary, the system/service principal for the
index-time enforcement points (Stage-2 r2 review N7), the port conformance test suite
(fail-closed + attributes-only invariants, D5 two-adapter pattern), the single serving-filter
SQL-fragment module with the serving-path equivalence binary (mechanically generated fragment
or mandated differential conformance test — the mechanic-8 rendering, QA-9.4; Stage-2 r2
review N1), and the port-cardinality disposition (a distinct per-domain PDP at this Frame;
unification with the host-fn capability gate, if ever, is decided there — Stage-2 r2 review
N8) (mechanic 8, QA-9); the owner-only enforcement-layer question — whether V0.1+ per-user
`owner-only` warrants an index-partition / admission-level control rather than the serving
predicate alone (R3's stated serve-time-only rationale; Stage-2 r2 review N6); the
write-authorization
residual (mechanic 7; §5 write-authorization surface): who may set/clear each policy field, the
recorded-attributable-write contract and its audit events; the timed-embargo design option
(recorded, unbuilt — §9); the citation-resolution behavior for a visibility-restricted artifact
under an unauthorized caller (stub vs not-found — existence-disclosure-sensitive; Stage-2
review L2, QA-4.4); and the query-time egress content-audit completeness item (which query text
/ which retrieved chunks egressed, keyed for incident response — Stage-2 review L5, R11).
**Trust side:** the final trust validators and precedence rule, the per-artifact-kind
lifecycle→authoritative mapping table, the `supersedes` origin-weighting in the `outdated`
predicate (with `{{P-0016}}`; including the `declared`-origin/first-party-corpus coupling and
its `1.2.0` ingest trip-wire — §5, §9, Stage-2 r2 review N10), the freshness schema (version-handle kinds, the decay-class
table with volatility-matched TTLs, the override record shape), the displacement-event registry
shape + decision-axes substrate field, and the citation form (artifact ID + block anchor).
**Enforcement semantics:** index admission, model-egress gate, serving predicate, citation
stub, audit events, structural fail-closed — including the labeling-correctness residual and
the adopter-facing polarity revisit named in §5 (maintainer-accepted residual (i)). Reads/serves
the G2/G3 substrate fields.

### `{{P-0016}}` — Edge schema

Locks the superset vocabulary as a closed enum, the discriminating columns (`edge_class`,
`origin`) and the `supersedes` classing rule, the extraction sources + source-span provenance
pointer + idempotency contract + coverage measurement (R4), the traversal contract (recursive
CTE shape, depth/latency bounds, the D4 instrument's field set), uniqueness/DDL, and the
manifest-ownership semantics for the `0.8.0`-family table now written by both the `repos`
plugin's CRUD path (work-graph, `origin ∈ {declared, system}`) and the host-side extractor
(citation, `origin = extracted`).

### Stage-3 obligations that are not ADR slots

- **The RC-1 product-brief amendment** (authored with the Stage-3 spec, riding this cluster's
  docs PR; forbid-scoped here): labeled MODIFIED deltas reconciling every falsified canonical
  copy in the same change (PD1) — the brief's Non-goals model clause, its Hard-constraints
  model-hosting clause, and the `0.1.0` substrate entry's external-embedding framing; the
  architecture-overview ELT subsystem's external-embedding framing is the named lagging copy
  (maintainer-internal record) reconciled at the same stage.
- **The register update**: `idea → proposed` promotion for D1, D2, G2/G3 (the intake performed
  the promotion; the register edit rides the docs PR), with the `0.8.0` edge-table substrate
  noted as what D2's traversal activates — plus the **named future register entry for per-user
  identity machinery** (owner-columns-as-identity, caller identity in context, owner==caller
  checks; §2 pre-gate walk item 9, R3).
- **The accessibility requirement (§2 pre-gate walk item 13):** routed to the product brief as
  a standing product requirement binding on UI/docs surfaces — not faked into this MCP-verb
  cluster; the brief amendment rides the same Stage-3 docs PR as the RC-1 riders.
- **The spec base pin** (`main@4e852a1` + assumed-shipped surfaces) recorded in the spec.

## 9. Deferrals and carried trip-wires (consolidated)

Every deferral names its firing mechanism (P-Defer/DF1). Items owned by P-0010 are consumed
here — their instruments are placed by this Frame (R11), not re-derived.

| Deferred | Decision content when it fires | Firing instrument |
|---|---|---|
| Clustered tree-builder (R5) | Build the `clustered` strategy behind the `TreeBuilder` seam; possibly flip the default | The hierarchy-source eval read (fired by the committed-tier plan's eval deliverable) — per-strategy failure rate + the hierarchy-coverage measure |
| BM25 keyword leg (P-0010 D3, carried) | Adopt `pg_textsearch` (Green) | Golden-query `ts_rank`-vs-BM25 regression harness shows a margin worth the extension cost |
| Apache AGE graph leg (P-0010 D4, carried) | Adopt AGE (Green, openCypher) | The C5 traversal instrument logs a real multi-hop query the CTE path cannot serve at acceptable latency/expressiveness (QA-8) |
| Second storage adapter (P-0010 D5, carried) | Re-evaluate a second `Storage` implementation | Rides D4's signal; or the quarterly license-watch detects a Green relicense |
| Local generative models (research §5.3/§5.5/§5.7) | Migrate rewrite/chunk-context/tag-gen (and synthesis if it survives) to local models behind the same host-fns | Egress-volume counter ≥ ~14M doc tokens/month (index-time) or ~$50/mo rewriter spend or p99 network latency bound or a sovereignty requirement (self-announcing) |
| Index-granularity ABI flag (R10) | Design the per-shape flag | Adopter ask (self-announcing) or the storage-overhead instrument crossing its spec-named threshold |
| Placement narrowing (RC-3) | Keep-or-narrow each of 4.1–4.4 | The code-aware retrieval eval — a named committed-tier plan deliverable — reads the R11 signals; until it runs, all placements stay on (intake non-goal 4's firing mechanism) |
| Extra fusion channels (fact-key, raw-message, ColBERT) (R6) | Add a channel | Retrieval-failure analysis in the run records attributes misses to a class an existing channel cannot serve |
| TimescaleDB (P-0010 D8, carried) | Adopt for metrics/events tables | Logged query-latency-or-storage-cost threshold on those tables |
| Timed embargo — auto-expiring permission values (R3) | Design the embargo option in `{{P-0015}}`'s dimension model (validator + enforcement point + expiry semantics) | Self-announcing — a real embargo case is asked for (maintainer or adopter); until then permission changes are owner-mutable flips only (recorded writes, mechanic 7) |
| Write-side policy dimensions — operation facet: write / label / supersede (R3 mechanic 5) | The write-operation dimension set + validators + enforcement points, added as `{{P-0015}}` amendments (mechanic 6) | Self-announcing — the first surface granting a non-owner actor write/label capability (per-user identity landing, `tenant_share` sharing, plugin-mediated writes); until then the write surface is the owner-mutable flip with recorded attribution, riding the admin-gated content-mutation path (P-0009, `read_observer` excluded — the gate-confirmed V0 invariant, §5) |
| `declared`-origin authority weight on trust-affecting edges (R3, §5) | Re-evaluate the origin-weighting and extend the extraction/ingest-integrity contract to frontmatter-declared edges — `declared` frontmatter stops being first-party-trusted once untrusted submitters can author it (`{{P-0015}}`/`{{P-0016}}`) | Self-announcing — the register `1.2.0` ongoing-ingest feature opening the first untrusted-submitter path into document frontmatter (Stage-2 r2 review N10) |

## 10. Rationale chain

Intent → constraints → decisions, traceable:

- **One-call, trust-labeled, budget-shaped context** (intake JTBD; reference trace) → progressive
  disclosure as physics (intake hard constraint; context-rot finding) + MCP-native surface →
  three constrained verbs with budget mechanics (R2 ← 2a direction 4) on the existing stdio
  server, host-served (R1 ← P-0002/P-0013).
- **"Trust-labeled" requires a contract, and a contract requires validators** (validatability
  lens; intake hard constraint) → the sources-with-roles finding + G2/G3 substrate fields +
  the maintainer's freshness/displacement advances → the validator-gated envelope (R3) →
  `{{P-0015}}` — ratified at the pre-gate walk as a **two-axis** contract: trust provenance +
  a policy permissions record behind one decision port (§2).
- **The envelope's policy dimensions are a security control** (intake risk (a); keystone edge)
  → structural enforcement at the named enforcement points behind the single decision port
  (R3 mechanics 3/4/8, TB-1, QA-4, QA-9).
- **Generative placements egress** (research §4; Stage 1c F2) → per-placement boundary statement
  + policy-aware egress gating + bounded egress (§5) → sovereignty reconciled by the zero-egress
  configuration (2a direction 2, QA-5), with §5.7 local migration as the upgrade path.
- **The latent authored graph must become traversable** (use-case delta 1–3) → typed superset
  edge substrate with provenance discrimination + measured extraction (R4 ← 2a direction 1,
  F6) → CTE traversal carrying the D4/D5 instruments (R11, QA-8) → `{{P-0016}}`.
- **Retrieval quality on this corpus** (research r3) → per-shape chunking + collapsed-tree
  hierarchy + hybrid FTS/dense/RRF + local rerank (R5, R6 ← P-0010 D2/D3/D6) — with the
  authored-hierarchy assumption held as an eval hypothesis (cross-check) behind a seam (R5).
- **"All placements ship, narrow later" is only honest with signals** (RC-3; P-InstrumentBefore)
  → instrumentation as an in-scope deliverable, signals in workspace-scoped run-record tables,
  every deferral wired to an instrument (R7, R11, §9).
- **Everything sits on the locked substrate** (P-0010 A1-clean; P-0006/P-0009 tenancy;
  P-0002/P-0012/P-0013 plugin model) → no new engine, no new extension, tenancy invariant on
  every table, plugins untouched, ABI shape locked forward (R8).

## 11. Intent self-report

**(1) I read the JTBD as:** the maintainer needs an agent that picks up any artifact to get, in
a single MCP call, a focused and honest picture of where that artifact sits — what it extends,
what it feeds, whether it is current, whether it may be used — sized to the agent's context
budget, so that sessions stop paying a re-derivation tax and stop acting on stale or forbidden
context. The provenance envelope is not garnish on retrieval; it is the difference between a
context layer and a search box — a layer that serves stale or policy-violating context
*confidently* is worse than none. The cluster is one feature because the verb without the
envelope is untrusted, the envelope without edges has no graph position to report, and search
without the budget discipline enlarges context instead of shaping it.

**(2) Decisions that strain an enumerated Non-goal or Success criterion:** none diverge; two
are worth flagging as near-boundary. (a) *Non-goal "no retrieval eval harness build-out" vs the
instrumentation in R11*: this Frame ships the **signals** (records, counters, coverage
measures) and holds the line at the harness — the eval that *reads* them stays a committed-tier
plan deliverable. The boundary is deliberate: signals are this cluster's scope (SC6), the
reader is not. (b) *Non-goal "no multi-tenant policy-enforcement expansion" vs QA-6/TB-3*: the
Frame adds tenancy *structure* (columns, predicates, preconditions) on the new tables and
changes no enforcement posture — structural scoping is the intake's own hard constraint, not an
expansion. No other direction touches a Non-goal.

**(3) SC2 interpretation (pre-gate walk item 10; recorded verbatim):** intake success
criterion 2 reads "a source-role from a closed vocabulary (candidate set: …)" — singular. The
two-axis envelope with a permissions record *implements* that commitment (closure,
validator-gating, and all five candidate-role semantics preserved; expressivity strengthened);
the candidate-set framing explicitly left role-selection as Frame work. Recorded as deliberate
interpretation, not intake drift; no intake amendment. Gate-outcome accuracy note (2026-07-02,
§2 item 15; Stage-2 r2 review N4): the five-of-five claim holds at V0 **via the `admin-only`
visibility value added at the gate** — round-1 `sensitive`'s read-authorization granularity
(admin-readable / observer-withheld) maps to `model_egress: deny` + `visibility: admin-only`;
`owner-only` remains serve-none until per-user identity lands and is not what carries the
`sensitive` semantic.

## 12. Consultations

None — no Mode A consultation fired during synthesis. The two places where an expert lens was
considered and judged non-blocking: (i) tokenizer choice for budget counting (a spec-stage
implementation detail behind the single counting function); (ii) egress rate/size limit values
(spec-set numbers; the mechanism is locked here).

## 13. Routine decisions (batched)

Inline decisions within principle, reported at the gate (none reworks done work, none re-opens
a ratified direction):

1. *(Revised r2 — round 1's single-role precedence was replaced by the ratified two-axis
   split, §2.)* Per-axis determinism rendering: the trust axis keeps first-match precedence
   (`outdated` → `authoritative` → `background`, round 1's relative order); each policy
   dimension is a single-valued mechanical predicate. Round 1's `authoritative` clause "not
   policy-flagged" is removed — axis orthogonality is the ratified point.
2. *(Absorbed r2 into ratified mechanic 3.)* Structural fail-closed: each undecidable
   security-dimension check resolves to its restrictive pole; round 1's "treated as `dont-use`
   for serving" rendering survives as that mechanic's serving-side effect — Security
   default-on.
3. *(Restated r2 per Stage-2 review L1; serving semantics updated to the two-axis design.)*
   The policy dimensions are new **use-policy predicates the intake committed to** (risk
   profile (a), SC2), enforced at V0 through the existing P-0009 caller context (workspace +
   role enum) — no new role, no tenant-posture change; Non-goal 6 (specifically multi-tenant
   policy expansion) is not touched. Stated precisely: a new content-level authorization
   predicate layered on the existing enum, not "no policy expansion."
4. The reduction ladder's fixed order (rerank → coarsen → drop) — operationalizes research
   §2.3/§2.5 without inventing new mechanism.
5. `search` returns summary-grain by default (first-pass verb per research §2.2) with full
   bodies reachable by citation expansion — the constrained-tool-surface pattern.
6. Ten-tag initial vocabulary proposal (R9) — a strawman for the spec's worked-examples pass,
   flagged as proposal, not lock.
7. Narrowing-signal storage as product tables rather than emitted-only telemetry (R11) — the
   eval is a data consumer; consistent with the dispatch-metrics precedent and P-0010 D8's
   emit-not-store applying to *observability*, not to measurement-product data.
8. Superseded summary nodes excluded from default retrieval but point-in-time reachable —
   direct application of the carried keyed-supersession borrow.
9. The egress-volume counter doubles as the §5.5 migration-trigger instrument — one instrument,
   two consumers, per IB1's backfillable-first scoring.
10. No new P-0002 partition amendment (R1 consequences) — the discriminator is applied to a
    surface outside P-0002's enumerated increments, not changed.
11. *(r2)* Envelope wire shape rendered as `{trust, freshness, citation}` with the policy
    record as an enforcement input, surfaced only where the contract requires it (the
    `dont_use` stub's policy reason) — final wire schema is `{{P-0015}}` work.
12. *(r2)* QA-9 added to render mechanic 8's blast-radius intent as an ATAM modifiability
    scenario (P-0010 D5 two-adapter pattern) — no new mechanism, an observable measure for a
    ratified lock.
13. *(r2)* The write-side policy-dimension deferral (§9) is given a self-announcing firing
    rendering — the first surface granting a non-owner actor write/label capability — as the
    mechanical reading of the ratified "anticipated structurally, not designed now"; the
    deferral itself is maintainer-ratified. Gate outcome 2026-07-02 (§2 item 14): with the V0
    admin-gate invariant stated at §5, the trigger stands sound as rendered (Stage-2 r2
    review N3 confirmed).
14. *(r2)* Timed embargo recorded as a §9 design-option row with a self-announcing trigger (a
    real case asks) — the DF1 rendering of ratified walk item 8.
15. *(r2)* Verbatim-carriage rendering: the eight mechanics' ratified word sequence is carried
    in the §2 walk record and applied at R3; the adaptations are citation form (source name +
    lock date per this doc's provenance-pointer convention, replacing raw file paths) and
    reviewer references rendered by role, per the doc's conventions.
16. *(r3)* The content-policy decision port is recorded as a **distinct per-domain PDP
    instance** sharing the port pattern with the host-fn capability gate; unification, if
    ever, is `{{P-0015}}`/host-fn-gate ADR territory. The Cedar anchor is correspondingly
    cited as validating the port pattern + PARC-shaped boundary generally, not a
    content-policy-specific fit — the minimal-coupling reading of P-MinBlastRadius (Stage-2
    r2 review N8).

## 14. Escalated decisions

**None.** No `conflicts-with` edge fired beyond its canon-stated resolution posture (§3), no
intent ambiguity survived the locked intake + 2a record, and no novel cross-project precedent
is set (every direction anchors to existing canon or to a maintainer-ratified 2a direction).

## 15. Provenance

- Intake: [`docs/intent/retrieval-cluster.md`](retrieval-cluster.md) (locked 2026-07-02).
- Stage 2a input record: elicitation with the maintainer, 2026-07-02 (embedded at §2).
- Stage 1c review: the security reviewer's round-1 findings (2026-07-02; zero blocker/high;
  Part 5 threat-model handoff discharged in §5).
- Locked retrieval research, revision 3 (2026-05-27) + canon-free cross-check (2026-06-05) +
  use-case record: get_context_for (2026-06-05) — cited by name and lock-date per the
  provenance-pointer convention.
- Sources-with-roles finding + freshness/displacement advances (maintainer record, 2026-05-21).
- Knowledge-object survey substrate findings G2/G3 (2026-05-15).
- Pre-gate maintainer walk (2026-07-02) — thirteen decomposer-ratified items, transcribed at
  §2; with the Stage-2 security/conflict review round 1 (2026-07-02; zero blocker/high, four
  medium, five low — all folded), the r2 revision sources.
- Cedar/NGAC permission-model research (maintainer record, locked 2026-05-18) — the
  decision-port anchor (R3 mechanic 8), with G-0017 (FlagProvider) and P-0010 D5 as
  port-pattern precedents.
- Project ADRs consumed: [P-0002](../src/adrs/P-0002-core-plugin-partition.md),
  [P-0006](../src/adrs/P-0006-v0-tenant-enforcement.md),
  [P-0009](../src/adrs/P-0009-rls-admin-token.md),
  [P-0010](../src/adrs/P-0010-storage-substrate-engine.md) (D2–D8, D6 method-borrows, tenancy
  preconditions), [P-0012](../src/adrs/P-0012-plugin-runtime-and-mcp-sdk.md),
  [P-0013](../src/adrs/P-0013-plugin-invocation-model.md).
- Product brief: [`docs/src/intent/mnemra-core.md`](../src/intent/mnemra-core.md) (register
  entries 1.1.0, D1, D2, G2/G3, 0.8.0; self-hostable criterion; model-hosting clauses RC-1
  amends at Stage 3).
- Shape precedent: [`docs/intent/signing-to-runnable-frame.md`](signing-to-runnable-frame.md)
  (same `architecture` spec_type).

## Changelog

- **2026-07-02** — Initial Stage 2b synthesis (cold-start modulation). Four decomposer-ratified
  2a directions embedded and refined; eleven directions locked (R1–R11); risk profile resolved
  with per-placement egress boundaries and the zero-egress V0 configuration; three ADR slots
  scoped ({{P-0014}}/{{P-0015}}/{{P-0016}}); zero escalations. Frame-exit gate pending.
- **2026-07-02 (r2)** — Stage 2b revision round 2, folding two sources: the **pre-gate
  maintainer walk** (thirteen decomposer-ratified items, §2 record — the two-axis envelope
  redesign with the eight locked policy mechanics + single decision port, labels demoted to a
  proposed initial set, owner-only-serves-no-one V0 semantics, flip-only temporality,
  identity-machinery-out, the SC2 interpretation note, residual-risk (i)/(ii) acceptance, and
  the retrofit-prep items: identity-bearing columns, single-sourced FTS language config,
  accessibility-to-product-brief) and the **Stage-2 security/conflict review round 1** (zero
  blocker/high; M1 fixed in §3, M2/M3/M4 named at §5/R3/R7/R8 with TB-5 and residual (iv)
  added, L1–L5 fixed or dispositioned in place). R3 rewritten to the two-axis +
  permissions-record design; §3/§4/§5/R4/R6/R7/R8/§8/§9/§10/§11/§13 updated; QA-1/QA-4/QA-7
  reworked to the two-axis vocabulary; QA-9 (decision-port blast radius) added. Zero
  escalations; no ratified item reopened. Frame-exit gate pending.
- **2026-07-02 (r3)** — Post-gate mechanical fold + lock stamp. Frame-exit gate confirmed
  (verdict Accept); two gate outcomes folded (§2 gate-outcomes record, items 14–15): the V0
  write-gate invariant (all policy/trust-affecting writes ride the admin-gated P-0009 path,
  `read_observer` excluded — §5, QA-7.3, §9, §13 #13) and the `admin-only` visibility value
  (R3 dimension table + dissolution formula, TB-1, §11(3) accuracy note, QA-4 measure 8 —
  label-tier change under walk item 6; the eight mechanics untouched). Stage-2 r2 delta
  review folded (zero blocker/high; 4 medium + 7 low/nit): N1 serving-path equivalence binary
  (mechanic-8 rendering, QA-9.1/9.4, `{{P-0015}}` slot), N2 per-dimension undecidable
  artifacts (QA-4 stimulus + measure 5; QA-9.3 aligned), N5 4.1-specific residual-(iv)
  control (§5, `{{P-0014}}` slot), N6 enforcement-layer asymmetry rationale + index-partition
  flag (R3, `{{P-0015}}` slot), N7 system/service principal for index-time enforcement points
  (mechanic-8 rendering, QA-9.1, `{{P-0015}}` slot), N8 Cedar-anchor scope + distinct
  per-domain PDP (mechanic-8 rendering, §13 #16), N9 demote-vs-exclude reconciliation (R6, R3
  trust table), N10 `declared`-origin/first-party coupling + `1.2.0` ingest trip-wire (§5,
  R3, §9, `{{P-0015}}` slot), N11 no change (a deliberate forward-prep placeholder —
  disposition recorded in the r3 report). Status: draft → locked.
