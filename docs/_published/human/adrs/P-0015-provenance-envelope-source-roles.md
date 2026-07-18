---
title: "P-0015: Provenance Envelope + Source-Roles Contract"
summary: "Resolves the retrieval-cluster Frame's {{P-0015}} slot. Locks the two-axis envelope contract: a policy permissions RECORD (final V0 dimension set — dont_use / model_egress / visibility / tenant_share — each with a mechanical validator, a permissive not-set DDL default, named enforcement points, an undecidable-only fail-closed pole, and a governed operation, under the eight decomposer-ratified mechanics and a single Cedar-PARC-shaped decision port with a differential serving-path conformance test applied per-channel) and a trust axis (outdated/authoritative/background with mechanical validators, the per-kind lifecycle→authoritative mapping, and origin-weighted supersession). Locks the freshness schema (version-handle kinds, decay classes, structurally-recorded overrides), the displacement-event registry (closed three-kind V0 vocabulary), the citation form, the citation-resolution dispositions (dont_use stub vs visibility not-found), and the V0 write-authorization invariant. 'sensitive' dissolves permanently into model_egress:deny + visibility:admin-only."
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

# P-0015: Provenance Envelope + Source-Roles Contract

**Project:** mnemra-core

## Status

`accepted`

Accepted 2026-07-02 at the retrieval cluster's spec-exit gate, the human review checkpoint that closes [Spec](../glossary.md#spec) (Stage 3 of the work-shaping pipeline, where agents turn a locked frame into a testable spec). The gate reviewed this decision alongside that stage's own spec, [2026-07-02-retrieval-cluster](../../specs/2026-07-02-retrieval-cluster.md).

This [ADR](../glossary.md#adr) (Architecture Decision Record: the context behind a decision, the decision itself, the alternatives it ruled out, and the expected consequences) was written during that Stage 3 work. It resolves the `{{P-0015}}` placeholder left open by the [Frame](../glossary.md#frame) (Stage 2's constraint-summary output) per [placeholder-resolution](placeholder-resolution.md).

The two-axis redesign, the eight policy mechanics, and the V0 (initial-scope) semantics below were already settled: the decomposer (the intake author) and the maintainer walked through them together before the gate, and they were ratified again at the frame-exit gate on 2026-07-02. The spec-exit gate did not reopen any of that. What this ADR adds at spec-level precision, and what the gate actually reviewed, is the finalized labels, the tables, the dispositions, and two rendering choices the Frame left open for this ADR to settle.

## Context and Problem Statement

[Intake](../glossary.md#intake) (Stage 1 of the pipeline, where structured intent gets captured) sets success criterion 2: every context item returned to a caller must carry a validator-gated provenance contract covering how authoritative it is, how fresh it is, whether it's usable at all, and a stable citation. Intake's risk profile states the security fact directly: the use-policy semantics are an authorization control. A retrieval path that ignores them leaks by design.

The Frame's pre-gate maintainer walkthrough replaced the round-1 design, a single five-role value, with two separate axes: a policy axis (a permissions record, enforced structurally) and a trust axis (provenance, served as a label). A single value destroys information. An item can be sensitive and authoritative at once, or flagged dont-use and background at once, and no single field can hold both facts. The maintainer's framing, kept verbatim: "capture the hard to change stuff now, the mechanics; the labels are the easy part."

This ADR is the slot that locks that contract. It reads and serves the G2/G3 substrate fields (cross-artifact authoritativeness plus provenance/use-policy, from the knowledge-object survey dated 2026-05-15), and it renders the freshness and displacement advances from the sources-with-roles finding (maintainer record, 2026-05-21).

## Decision Drivers

- **The policy side of the envelope is a security layer, not metadata** (a foundational dependency under P-SecurityLayered; intake risk profile item a). Every security dimension needs structural enforcement at a named point, fail-closed, behind one decision boundary.
- **Validator-gated, or there's no slot at all** (intake hard constraint; P-ShiftLeft D2, validator before field). Every dimension and every trust value is a mechanical predicate over stored fields and edges. No inference, no prose judgment, anywhere in how a value gets assigned.
- **Lock the mechanics that are hard to change; keep the labels easy to change** ([P-LockContract](../glossary.md#p-lockcontract): lock the contract, vary the implementation; this is the maintainer's own ratified framing). The record shape, the enforcement discipline, and the port boundary are the hard-to-change contract. Dimension names and enum values are deliberately left open to a later ADR amendment.
- **Bound the blast radius of a future policy engine** (the maintainer's stated intent for mechanic 8). Swapping in a policy engine later has to replace one module, not force a rewrite. The Cedar/NGAC permission-model research (maintainer record, locked 2026-05-18) put it directly: "the engine sits behind mnemra's policy port... port substitution, not a rewrite." Precedents: G-0017's FlagProvider (a workspace-wide decision, this workspace's term for an architecture call that applies across every project) and P-0010 D5's Storage trait (a decision scoped to this project alone).
- **No existence-disclosure oracle** (Stage-2 review finding M1). Withholding a result has to be silent to the caller; the only observability signal lives on the operator side.
- **Retrofit posture** (pre-gate walkthrough item 11). Identity, permissions, and audit are hard to bolt on after the fact, so the columns and write-attribution exist from day one, even while V0 is still single-user.

## Considered Options

1. **Two-axis contract: a policy permissions record plus a trust label, hardcoded validator predicates behind a single decision port. (Chosen.)**
2. **The round-1 single five-role value** (`authoritative / background / outdated / sensitive / dont-use`, resolved by precedence). Replaced at the pre-gate walkthrough and ratified there: a single value with precedence rules destroys information across facts that are actually independent of each other. Recorded here as the option the redesign chose against. Its five validators survive, feeding the two new axes as predicates.
3. **Adopt a policy engine (Cedar-class) at V0.** Rejected for V0: the V0 policy is four dimensions with hardcoded mechanical predicates, and a full engine is more mechanism than the evidence supports ([P-Defer](../glossary.md#p-defer): defer mechanism choice until evidence forces it; also plain simplicity). The decision port (PE-3) is the seam designed for that future substitution; QA-9 is the check that proves the swap stays a bounded change.
4. **Per-dimension config knobs.** Rejected structurally, under mechanic 6: a dimension without a declared validator, enforcement point, fail-closed pole, and governed operation isn't a dimension. Config alone can't declare any of those.

## Decision Outcome

**Chosen: Option 1**, spelled out below as PE-1 through PE-11. The binding requirement text lives in spec [R-0025/R-0026](../../specs/2026-07-02-retrieval-cluster.md) ([R-codes](../glossary.md#r-codes) are stable IDs for numbered requirements; their text lives in the requirements document that defines them). QA-4, QA-7, and QA-9 are the measures used to check this decision actually holds in practice.

### PE-1 — The two-axis contract under the eight locked mechanics

Every item returned to a caller carries an envelope of `{trust, freshness, citation}`, plus a `re-eval pending` flag on decision-kind artifacts (ADRs, specs, and similar). The policy record itself is an enforcement input, not something served back to the caller as a label. The one exception is the `dont_use` stub's policy reason, described in PE-4.

The eight mechanics below are carried over verbatim, in substance, from the ratified walkthrough record ([Frame §2/R3](../../intent/retrieval-cluster-frame.md)), and they bind every dimension, present and future:

1. **Orthogonal record shape.** Never a single enum covering multiple facts.
2. **Validator-gated.** No inference in how a value gets assigned.
3. **Structural fail-closed.** An undecidable security check resolves to that dimension's restrictive pole. "Undecidable" means the validator's stored source field is present but malformed or unreadable, or missing where the schema requires it to be present. That's distinct from "not-set," which is a defined, permissive V0 default per dimension (PE-2) and is never treated as undecidable. A security value that can be set but isn't enforced anywhere may not even exist in the schema.
4. **Named enforcement points.** The index-admission gate, the model-egress gate (both at index time and query time), the serving predicate, and the citation stub (the point where a citation gets resolved; the disposition is per-dimension, a stub for `dont_use` and a not-found for `visibility`, both covered in PE-4). No enforcement point means no dimension.
5. **Operation facet declared per dimension.** The write side is anticipated structurally but not designed yet.
6. **Closed but extensible, at the ADR tier.** A new dimension requires a P-0015 amendment that declares a validator, an enforcement point, a fail-closed pole, and a governed operation. It's never a config knob.
7. **Recorded writes.** Permission-field writes are recorded, attributable, authorized writes.
8. **One decision port.** All enforcement consults a single port, covered in PE-3.

*(Anchor: pre-gate walkthrough ratification, 2026-07-02. Off-limits to reopen.)*

### PE-2 — Final V0 dimension set (labels finalized; mechanics untouched)

The Frame's proposed initial set is adopted as the V0 set. Each dimension already conformed to mechanics 2 through 5, and nothing found since has motivated a rename:

| Dimension | Values | Not-set default (permissive, DDL) | Mechanical validator | Enforcement point(s) | Fail-closed pole (undecidable only) | Governed operation |
|---|---|---|---|---|---|---|
| `dont_use` | set / not set | not set | Explicit use-policy field = `deny` (G3 substrate field; author/curator-set, never inferred) | Index-admission gate + query predicates + citation resolution (metadata-only stub) | Excluded: never chunked, embedded, egressed, or served. Citation resolves to a metadata-only stub with the policy reason | serve/egress |
| `model_egress` | `allow` / `deny` | `allow` | Explicit egress field (G3 sensitivity-family; never inferred from content) | Model-egress gate: index-time (4.1/4.4 inclusion) and query-time (4.3 payloads) | `deny` | egress |
| `visibility` | `workspace` / `admin-only` / `owner-only` | `workspace` | Explicit visibility field; `admin-only` predicate: caller's `WorkspaceCtx` role is `Admin` (role-gated, no identity plumbing; gate outcome, Frame §2 item 15) | Serving predicate + citation resolution (not-found, per PE-4; deliberately not a stub) | `owner-only` (V0: serve to no one, per PE-4) | serve |
| `tenant_share` | `workspace-only` (fixed at V0) | `workspace-only` | Structural constant: a declarative hook over structural tenancy (P-0006). A future sharing capability must honor per-content holdbacks | Serving/tenancy predicates | `workspace-only` | serve/share |

**Not-set is a value, not an absence.** Every policy column carries the permissive DDL (data-definition-language, schema-level) default from the table above. That default is applied at ingest, and the G2/G3 migration applied it retroactively to the unlabeled `0.14.0` corpus. So a stored artifact row always holds a decidable value for every dimension, on the sanctioned write paths. A value written by bypassing the R-0025-f gate directly at the substrate layer can come out malformed, and mechanic 3 resolves that to the dimension's restrictive pole. That means the mechanic-3 fail-closed rule fires only on genuinely undecidable values (present but malformed), never on content that's simply unlabeled. This is PE-11's opt-in-labeling question, rendered structurally: an unlabeled artifact is served to workspace callers and stays egress-eligible at V0. The fail-closed-pole column in the table above governs the undecidable case only.

**`sensitive` doesn't survive, not as a stored value and not as a macro.** Its intent maps onto `model_egress: deny` plus `visibility: admin-only`, which restores the round-1 read semantics: admin-readable, withheld from observers. A macro would just be a second name for the same stored facts (one noun per concept, per [P-MinBlastRadius](../glossary.md#p-minblastradius): a change should reach no further than the architecture allows), and it would give the composed meaning room to drift from the fields actually being enforced. Documentation can explain the mapping; the schema itself carries only the two dimensions. Content marked `model_egress: deny` is still indexed locally in full, since RC-1's local encoders are egress-free. The policy record and the egress boundary reinforce each other instead of fighting.

### PE-3 — The decision port: PARC shape, system principal, conformance, serving-path equivalence

Every host-side enforcement point consults one boundary: `decide(principal, action, resource, context) → allow/deny + reason` (the Cedar PARC shape: principal, action, resource, context). The V0 implementation behind that port is just the hardcoded PE-2 validator predicates. The Frame left four renderings of this port for this ADR to settle:

- **Index-time principal (Stage-2 r2 N7).** The index-admission gate and the index-time model-egress gate fire during batch builds, where there's no caller. They consult the port under a distinguished system principal: a dedicated principal variant that names the invoking subsystem (the index builder, for example), not a workspace role and not a synthetic admin account. That keeps the PARC shape well-defined at every enforcement point, including the ones with no human caller behind them.
- **Conformance is tested at the port.** A design-time conformance suite (fail-closed and attributes-only invariants, using the same two-adapter-test pattern as P-0010 D5) exists from V0 and runs against any implementation sitting behind the port, including seeded undecidable-dimension cases that should resolve to their restrictive poles (QA-9.3).
- **Serving-path placement (r1 fold; Frame C4 keystone).** The serving predicate's SQL fragments apply in every channel's WHERE clause (both full-text search and dense/vector search) and at every recursion level of the traversal CTE, mirroring where the workspace predicate already sits. That keeps a policy-restricted row out of RRF (reciprocal rank fusion), out of reranking, and out of budget accounting entirely: rank order and budget reports are computed only over the set the caller is actually entitled to (spec R-0025-g is the binding text). The same predicate binds the `relations` edge projection too (r2 fold): an edge only surfaces in the caller-facing `relations` bundle if the caller is serve-entitled to both endpoint artifacts, including the source endpoints of incoming edges and both ends of deeper-hop edges. That's necessary because the traversal CTE's reachable-node set isn't the same thing as the returned edge list. A withheld edge stays silent to the caller (PE-4), with an audit row recorded behind the scenes. No caller-facing edge projection is allowed to become an existence oracle for a neighbor withheld by `visibility`.
- **Serving-path equivalence, the pick (Stage-2 r2 N1).** The row-level serving predicate runs inside Postgres and can't call `decide()` per row, so it enforces through SQL fragments, derived from the port's attribute vocabulary, kept in exactly one module. The Frame posed a binary choice here: (a) mechanically generate that fragment from the predicate definitions, or (b) require a differential conformance test. This ADR locks option (b), the differential conformance test. The fragment and `decide()` must return identical allow/deny decisions over a generated resource set covering every dimension, every pole, and every undecidable case (QA-9.4), and the test has to demonstrably fail when a divergence is seeded on purpose. The reasoning: V0's predicates are a handful of hardcoded checks, so a fragment generator would be more machinery than the evidence justifies (plain simplicity, and [P-Defer](../glossary.md#p-defer)), while the differential test reuses a pattern that already exists and directly produces the QA measure. Option (a) stays on record as the alternative a future amendment can adopt if the dimension count grows enough that hand-maintained fragments become the real drift risk. The differential test itself is the instrument that would catch that drift: a failing equivalence run is the signal that fires the reconsideration.
- **Port cardinality (Stage-2 r2 N8).** This content-policy port is its own distinct per-domain PDP (policy decision point) instance, sharing the port pattern with the host-fn capability gate but not sharing an engine with it. Unifying the two, if that ever happens, is a decision for a future amendment to this ADR and to the host-fn gate's own record (reading [P-MinBlastRadius](../glossary.md#p-minblastradius) as minimal coupling between the two). The Cedar/NGAC research anchor validates the port pattern and the PARC-shaped boundary in general; its finding of an exact PARC fit was scoped specifically to the host-fn capability gate and doesn't carry over as a content-policy-specific claim.

*(Anchors: the Cedar/NGAC permission-model research, maintainer record locked 2026-05-18; cedar-policy 4.x is Apache-2.0/Green licensed for whenever the engine substitution happens; [P-LockContract](../glossary.md#p-lockcontract); precedents G-0017's FlagProvider and P-0010 D5.)*

### PE-4 — V0 enforcement semantics + citation-resolution dispositions

- **`visibility: owner-only` serves no one at V0** (ratified; mechanic 3 applied). Caller identity is undecidable under the current workspace-plus-role context, so owner-only items are excluded from all retrieval, for every caller, including `Admin`, until per-user identity lands. That conservative enforcement is the V0 semantics itself, not a value that's settable but happens to go unenforced.
- **`visibility: admin-only`** serves callers whose `WorkspaceCtx` role is `Admin`, and withholds from `read_observer` (a gate outcome, Frame §2 item 15).
- **Withholding is silent to the caller**, no count and no placeholder, but every withheld decision produces an operator-side audit event, stored in the spec Data Model's `policy_decision_audit` table (`{workspace_id, artifact_id, action, dimension, decision, reason, principal, occurred_at}`, also emitted as telemetry, kept independent of storage). That audit event is the signal that makes withholding observable at all, since a caller-visible marker would be an existence-disclosure oracle (Stage-2 finding M1). "Marker" includes an edge (r2 fold): an entry in the `relations` bundle naming a withheld artifact would disclose its existence, its relationship, its edge type and origin, and its citation. So the serving predicate filters the edge projection's endpoints too (PE-3; spec R-0025-g), and a withheld edge is indistinguishable from no edge at all. An edge to a `dont_use` artifact, by contrast, does surface: curatorial policy is meant to announce itself (the stub disposition described next). That asymmetry follows each dimension's own meaning, and it's the same asymmetry that shows up at citation resolution.
- **Citation-resolution dispositions (the Stage-2 L2 call, locked).** Calling `get_artifact_by_citation` on a `dont_use` artifact returns a metadata-only stub carrying the policy reason and no content. That curatorial kill switch is meant to tell the agent: this exists, don't use it. Resolving a visibility-withheld artifact as an unauthorized caller instead returns not-found, indistinguishable from a reference that simply doesn't resolve to anything. Visibility withholds existence itself, so a stub there would be a probeable oracle. The asymmetry is deliberate, and it follows each dimension's meaning: curatorial policy announces itself, audience restriction doesn't. *(Anchors: P-SecurityLayered fail-closed; QA-4 measures 1, 3, 4, and 8.)*

### PE-5 — Enforcement-layer asymmetry; the owner-only index-partition question (deferred, named)

`dont_use` gets dual-layer defense: index admission plus query predicates, meaning the content must not exist in the index at all. `visibility`, by contrast, enforces at serve time only and stays fully indexed: it's an audience restriction within the workspace, not a curatorial kill (Stage-2 r2 N6, stated deliberately as a distinct posture). Once per-user identity lands and `owner-only` becomes genuinely per-user, the serving predicate becomes the entire control for audience-restricted content sitting in a shared index.

Whether that arrangement warrants an index-partition or admission-level control instead is deferred to the per-user identity feature. The decision content is partition-versus-predicate for per-user `owner-only`. The deferral rests on [P-Defer](../glossary.md#p-defer): the shape of that mechanism depends on an identity design that doesn't exist yet. The instrument that fires this reconsideration is self-announcing: the per-user identity register entry (added to the product brief alongside this change) can't be designed at all without resolving owner-only serving semantics, so its own intake raises this question by construction.

### PE-6 — Write authorization: the V0 invariant, the audit record, the deferred write side

- **V0 write gate (frame-exit gate outcome, item 14).** Every write to a policy- or trust-affecting field, whether a permission flip or a freshness override, rides the existing admin-gated content-mutation path ([P-0009](P-0009-rls-admin-token.md)), and `read_observer` is excluded from it. No other write path exists at V0. This closes off the authorization window on writes that themselves carry authorization weight: clearing an egress-deny flag makes previously restricted content servable, and a spurious freshness override could mask real staleness (Frame §5's write-authorization surface, Stage-2 finding M3).
- **Recorded, attributable writes (mechanic 7).** Every label or permission change writes an audit row: `{workspace_id, artifact_id, field, old_value, new_value, actor token_id, occurred_at}` (spec Data Model `policy_write_audit`). Actor attribution is `NOT NULL`, always present.
- **Write-side policy dimensions stay undesigned (mechanic 5).** The split between read and write is anticipated structurally, but the actual set of write-operation dimensions arrives later, as P-0015 amendments under mechanic 6, whenever the first surface granting a non-owner actor write or label capability shows up (per-user identity, `tenant_share` sharing, plugin-mediated writes). That trigger is self-announcing.
- **Timed embargo** (permission values that auto-expire) is recorded here as a design option, not built. If it ever gets built, it's a dimension-model amendment declaring a validator, an enforcement point, and expiry semantics. It fires when a real embargo case is actually asked for, self-announcing again. Until then, permission changes are owner-mutable flips only, and every one is a recorded write.

### PE-7 — Trust axis: validators, precedence, the per-kind mapping, origin weighting

Each item gets exactly one trust value, chosen by first-match precedence: `outdated`, then `authoritative`, then `background`. Every predicate here is mechanical: no LLM involved, no prose judgment, per spec R-0026-a.

- **`outdated`** fires when either (a) an incoming `supersedes` edge of origin `declared` or `system` points at the item, or (b) the item is freshness-stale (PE-8) with no recorded override. Origin weighting is locked here: a supersedes edge of origin `extracted` does not enter the trust predicate at V0. It's still recorded, still traversable, still surfaced in the relations bundle to callers who are serve-entitled to both endpoints (PE-3's projection filter, spec R-0025-g), and it still raises an operator-side curatorial signal. Promoting it to actual trust effect only happens by rewriting it as `declared` through the PE-6 write gate. The reasoning: extraction reads free text, and adversarially shaped corpus text ("...this supersedes P-0010...") must never be able to demote a target's trust automatically (Frame §5's worked case; residual iv). The `declared` weight itself rests on a first-party-corpus assumption (Non-goal 1: no ongoing ingest at V0): frontmatter carries authority because, at V0, only first-party authors write it. There's a named trip-wire here (Stage-2 r2 N10): once register `1.2.0` (ongoing ingest) opens the first untrusted-submitter path into frontmatter, this weighting has to reopen, and the extraction/ingest-integrity contract extends to `declared` edges too. The `1.2.0` feature's own intake is what fires that reconsideration, self-announcing by construction.
- **`authoritative`** fires only if the artifact kind's lifecycle field sits in that kind's closed authoritative-state set, and `outdated` didn't already fire. Extending this V0 mapping table later is a P-0015 amendment, done per-kind and validator-first:

  | Artifact kind | Lifecycle field's authoritative-state set |
  |---|---|
  | Decision record (ADR) | `accepted` |
  | Spec | `approved` |
  | Intake / Frame / product brief | `locked` |
  | Research brief | `locked` |
  | *(any kind not in this table)* | *(none — the kind cannot fire `authoritative`; it resolves `background`)* |

  Kinds without a mechanical lifecycle field (code, logs, tasks, living canon docs) resolve to `background` by the total-function default: served, labeled, and ranked normally. That's deliberately conservative and fail-safe. `background` under-claims authority rather than over-claiming it, and adding a kind to the table later is a cheap, validator-first amendment, done whenever the under-claim demonstrably misranks something (the run records are the instrument that would show it).
- **`background`** is the total-function default: it's what happens when no other predicate fires.
- **Retrieval-path semantics** (per the Frame's trust table; the mechanism was defined at the r1 fold). "Hard-superseded" means the artifact has an incoming `supersedes` edge of origin `declared` or `system`. The "`superseded-by` forward pointer" is exactly that same edge, just read from the superseded item's side (a directional view, not a second schema field and not a topic-key inference: P-0016 ES-2, spec R-0029-e). Every item that fires predicate (a) of `outdated` is therefore hard-superseded: excluded from default retrieval, though still reachable point-in-time. The served-with-label-and-demoted disposition applies specifically to predicate-(b) items, the freshness-stale ones. `authoritative` ranks trust-primary; `background` is served, but labeled as such.

### PE-8 — Freshness schema: handle-diff primary, decay-class fallback, structural overrides

- **Version handles (primary signal).** Where an item cites a source that has a handle, the envelope stores the handle as it stood at index time, and reports `current`, `moved`, or `unknown` by diffing that stored handle against the source's live handle. The closed set of handle kinds at V0: `git-sha` (canon and repo citations, where the live handle is the repo's current state), `semver` (dependency and tool versions), `model-id` (model revisions, diffed against the configured pinned revision), and `doc-version` (explicit version strings on versioned documents). An unreachable live handle reports `unknown`, never a fabricated state (in the zero-egress configuration, external live-handle checks resolve to `unknown` by construction, since they can't be reached at all). Adding a new handle kind is a P-0015 amendment that has to declare its differ first, validator before field.
- **Decay classes (fallback signal; bounds ignorance rather than measuring staleness directly).** Sources without a handle carry a volatility-matched class instead, and TTL expiry without re-validation makes the item freshness-stale, feeding PE-7's predicate (b). The V0 class table:

  | Class | TTL | Domain examples |
  |---|---|---|
  | `volatile` | 30 d | vendor/dependency landscape, pricing, model availability |
  | `moderate` | 90 d | comparative research, tool evaluations |
  | `slow` | 180 d | project conventions, operational skills |
  | `stable` | 365 d | principles, foundational decisions, values |

  Time is the weakest signal here, a fallback, never a primary one. TTL values are class defaults and can be amended per class; the class assignment itself is a stored field.
- **Overrides are structural.** A freshness override is a recorded row, `{by, reason, date}`, accepted only through the PE-6 write gate, never a config toggle. An override restores the prior trust value, and the override itself stays visible in the envelope. Content-based staleness detection was considered and rejected as circular (the maintainer's advance 1, 2026-05-21).

### PE-9 — Displacement ≠ staleness: the event registry

Handle-diff catches the case where a cited source itself changed. It doesn't catch the case where the world moved past that source without the source itself changing. So this is containment, not a full solution: a named displacement-event registry (spec Data Model `displacement_events`), whose fired events set a `re-eval pending (axis)` flag on any decision whose recorded axes intersect the event. The V0 event-kind vocabulary is closed and fully enumerated (an enumerable registry with zero enumerated kinds would have nothing to actually build against). Extending it later is a P-0015 amendment that has to declare the kind's mechanical firing condition, validator-first, under the same mechanic-6 discipline used elsewhere:

| Event kind | Fires (mechanically) |
|---|---|
| `canon-superseded` | A `supersedes` edge of origin `declared`/`system` lands on a canon-kind artifact (ADR, principle, spec). Detected on the edge-write path this cluster builds |
| `dependency-major-change` | A `semver` version-handle diff (PE-8) crosses a major version boundary. Detected by the freshness handle-diff this cluster builds |
| `operator-declared` | The operator records an event through the PE-6 write gate, naming the axis and a description. The catch-all for world-shifts no detector covers |

A single-axis displacement triggers re-evaluation, never auto-invalidation: the trust value doesn't flip just because a displacement flag fired, and a multi-axis decision survives a shift on only one of its axes. Decision-kind artifacts record their own decision axes as substrate data. The honest limit here, carried over verbatim: trip-wires only cover displacement events that are actually enumerable. Genuinely unforeseen shifts fall back entirely on the volatility TTL. *(Anchor: the maintainer's advance 2, 2026-05-21; [P-PreserveDecisionSpace](../glossary.md#p-preservedecisionspace): every ADR keeps its rejected alternatives on record, so later readers can see what was on the table.)*

### PE-10 — Citation form

Every envelope item carries a stable citation: an artifact ID (content-addressed, per the V0 substrate), plus a block anchor when the item is a chunk of a larger artifact. Both resolve through `get_artifact_by_citation`. This ID-plus-name pairing, together with block addressability, is what makes citation-expansion retrieval work at all. *(Anchor: P-AgentPrimarySource.)*

### PE-11 — Residuals owned here

- **Labeling correctness, and the adopter-facing polarity question left open (Frame residual i, maintainer-accepted for V0).** Labeling is opt-in at V0: an unlabeled artifact is egressable at index time. The index-admission gate mitigates that somewhat, and the serving-time posture stays fail-closed regardless. This is rendered structurally as the PE-2 not-set DDL defaults: unlabeled means the permissive default values, stored as such, so the fail-closed rule (mechanic 3) only ever targets genuinely undecidable values and can never silently withhold the unlabeled corpus. This ADR owns the open question of whether that default polarity should flip for adopter deployments (unlabeled meaning egress-deny instead, mechanically achieved by flipping the `model_egress` DDL default for new rows). It fires self-announcing, either at the first deployment beyond the maintainer's own dogfood instance (an adopter standing up an instance is an explicit act) or at register `1.2.0`'s ingest work, whichever comes first.
- **Query-time egress content-audit (Stage-2 L5).** Which query text (4.2) and which retrieved chunk citations (4.3) actually egressed, keyed for incident response, landed as spec R-0034-b. It's recorded here as part of this envelope contract's enforcement observability.

### Consequences

**Good:**
- The hard-to-change parts (record shape, enforcement discipline, port boundary, fail-closed rule, write attribution) are locked, while every label stays a cheap ADR amendment. That's the ratified cost asymmetry, honored in practice.
- Engine substitution stays a bounded change (one PDP module plus one fragment module), proven by a conformance suite that exists from V0 (QA-9).
- There's no existence oracle anywhere on the caller surface. Every withhold is observable, but only to the operator.
- Adversarial corpus text can't flip trust (PE-7's origin weighting) and can't reach served payloads through 4.1 (P-0014 RA-9).

**Bad / Trade-offs:**
- `owner-only` over-blocks at V0: it serves no one, not even the owner. That's the deliberate price of mechanic 3 until identity lands, and the identity feature's own intake is the named point where this gets reopened.
- The differential conformance test has to be maintained as dimensions evolve. That's accepted in place of a fragment generator at V0's current scale, and a failing equivalence run is itself the instrument that would catch drift.
- Kinds outside the PE-7 mapping table under-claim authority, resolving to `background`. That's fail-safe, but it's a ranking cost until the table gets extended on real evidence.
- Freshness `unknown` shows up often in zero-egress deployments, since external handles are unreachable there. That's honest by design, and TTL still bounds the ignorance either way.

## Pros and Cons of the Options

### Two-axis record + port, hardcoded predicates (chosen)

- Pro: carries orthogonal facts without losing information. Each axis keeps its own determinism.
- Pro: enforcement is structural, fail-closed, and sits behind a single boundary, and it's V0-simple: just four predicates.
- Con: two axes is more schema than one enum. That cost is paid once, at the layer that's hardest to retrofit later.

### Round-1 single five-role value (replaced)

- Pro: one field, simple precedence.
- Con: destroys information. Sensitive-and-authoritative, dont-use-and-background, both are real combinations, and precedence rules fight against them. Ratified out at the pre-gate walkthrough; its five validators survive as predicates feeding the two new axes.

### Cedar-class engine at V0 (rejected for V0)

- Pro: an expressive policy language, forward-ready.
- Con: more mechanism than four hardcoded predicates need right now. The port makes later adoption a substitution rather than a rewrite, and QA-9 is the proof obligation for that claim.

### Config-knob dimensions (rejected)

- Pro: no ADR churn needed to add a label.
- Con: violates mechanic 6. Config can't declare a validator, an enforcement point, a fail-closed pole, or a governed operation, so a knob-created "dimension" would be unenforced by construction.

## More Information

- Binding requirement text: [spec R-0025/R-0026](../../specs/2026-07-02-retrieval-cluster.md); observable measures QA-4, QA-7, QA-9 ([Frame §7](../../intent/retrieval-cluster-frame.md)).
- Companions: [P-0014](P-0014-retrieval-architecture.md) (the query path and gates that consume `decide()`; it co-owns the 4.1 control RA-9 with this contract) and [P-0016](P-0016-edge-schema.md) (the edge `origin` column PE-7 weights; the extraction-integrity contract).
- Ratification trail: the pre-gate maintainer walkthrough plus the frame-exit gate outcomes (Frame §2, items 1 through 15: the eight mechanics carried over verbatim; `admin-only` added at the gate; the V0 write invariant confirmed at the gate).
- Sources, by name and lock-date, following the provenance-pointer convention: the sources-with-roles finding plus the freshness/displacement advances (maintainer record, 2026-05-21); the Cedar/NGAC permission-model research (maintainer record, locked 2026-05-18); the knowledge-object survey G2/G3 (2026-05-15).
- Substrate: [P-0006](P-0006-v0-tenant-enforcement.md) and [P-0009](P-0009-rls-admin-token.md), which supply the caller context and the admin-gated mutation path this V0 enforcement rides on.

## Amendment 2026-07-17 — Ingest source trust-class + external-class trust-predicate keying (`{{P-0015-A1-ingest-trust-class}}`)

The ingestion-pipeline Frame ([`docs/intent/ingestion-pipeline-frame.md`](../../intent/ingestion-pipeline-frame.md), blob `f56b3685`, IP-10) routes the trust dimension of the ingest stamp path into a P-0015 amendment, because register `1.2.0` (ongoing ingest) is the named firing event carried by two of this ADR's own trip-wires: PE-7's `declared`-weight reopen ("when register `1.2.0` opens the first untrusted-submitter path into frontmatter, this weighting re-opens and the extraction/ingest-integrity contract extends to `declared` edges," Stage-2 r2 N10), and PE-11's polarity revisit (whose firing condition was "register `1.2.0` ingest, or the first deployment beyond dogfood, whichever comes first"). This amendment fires both trip-wires on their ingest half, using PE-7's own mechanic-6 amendment path (per-kind, validator-first), and it does not reopen a locked mechanic.

The binding requirement text is single-sourced to the ingestion-pipeline spec, R-0108 ([`docs/specs/2026-07-16-ingestion-pipeline.md`](../../specs/2026-07-16-ingestion-pipeline.md)), and the shape is rendered in [P-0024](P-0024-ingest-pipeline-shape.md) IPS-9. This amendment governs the trust-axis semantics; the ingest write path and the registration control-plane are governed by that spec instead.

**This keys the trust axis (PE-7), not the policy record (PE-2).** `trust_class` is not a fifth policy dimension, and it isn't added to the PE-2 table. PE-2's dimensions are permissions, enforced structurally at serving and egress. `trust_class` is a keying input to the trust predicates instead: it grades how far a declaring artifact's source can be trusted, which determines whether that artifact's declared trust-affecting edges get admitted to, or held out of, PE-7's determination. The two axes stay orthogonal, the same orthogonality [P-0016](P-0016-edge-schema.md) already keeps between `origin` (how an edge came to exist) and trust (how far its source is trusted). No PE-2 dimension, DDL default, validator, or enforcement point changes here.

### The mechanism: a stored `trust_class` keying the trust predicates

A per-source `trust_class` field is stored on the ingest source-registration row (spec Data Model `sources.trust_class`; [P-0024](P-0024-ingest-pipeline-shape.md) IPS-9), carried into the provenance stamp at ingest time, and consumed mechanically by the trust predicates. It's rendered under the same mechanic-6 discipline used everywhere else in this ADR, so a reviewer reads it as a structural, validator-first keying input, never as a config knob:

| Facet | Value |
|---|---|
| **Values** | closed enum `{first-party, external}` |
| **Validator** | admin-attested at the audited registration act (spec R-0099-d); `trust_class ∈ enum`, no inference anywhere |
| **Enforcement point** | the **PE-7 trust predicates** (at trust evaluation), keyed on the *declaring* (src) artifact's stamped `trust_class`; and the provenance-stamp write path (where the value is recorded on the artifact) |
| **Fail-closed pole (undecidable only)** | undecidable/missing `trust_class` resolves to **`external`**, the restrictive pole (mechanic 3). Trust-affecting `declared` edges from an artifact whose class cannot be read are held out of the trust predicate |
| **Governed operation** | trust evaluation (which edges may fire `outdated`/hard-supersession); registration/attestation of the class is an admin-gated control-plane write (spec R-0099-c) |

### PE-7 predicate extension (the N10 disposition)

PE-7's `outdated` predicate, branch (a), only reads a `declared`-origin `supersedes` edge (a trust-affecting one) when the declaring (source) artifact's `trust_class` is `first-party`:

- **`first-party`** (V0.1: the operator's own admin-attested corpus roots). Frontmatter keeps its `declared` authority exactly as PE-7 locked it. The "first-party-corpus assumption" PE-7 originally rested on now becomes an explicit per-source attestation, rather than a global assumption applied to everything. A `declared`- or `system`-origin supersession from a first-party artifact enters PE-7's predicate normally: lifecycle fields fire `authoritative` per the PE-7 mapping table, and PE-2's not-set permissive defaults apply as before.
- **`external`** (the parked clipper, webhook, and feed follow-ons; no V0.1 instance exists yet). A `declared`-origin trust-affecting edge whose declaring artifact is `external` does not enter PE-7's `outdated`/hard-supersession predicate. It's still recorded, still traversable, still surfaced in the relations bundle to serve-entitled callers, and it still raises an operator-side curatorial signal (a defined mechanism from ingest spec R-0108-f: a `curatorial_signal_count` run-record counter plus a redacted named-level log line, not just a bare assertion). But it's trust-inert. Promoting it to actual trust effect only happens by rewriting it as a `first-party`-declared or `system` edge, through the PE-6 admin-gated recorded-write path. `system` origin is unchanged by any of this, and so is `extracted`, since `extracted` never entered a trust predicate at V0 in the first place, per PE-7's origin weighting.

Demotion happens by trust-predicate keying, then, never by relabeling `origin`. External frontmatter relations still record honest `origin = declared` edges, because they genuinely are structured frontmatter declarations. So [P-0016](P-0016-edge-schema.md) ES-2's origin definition and ES-3's `source_span`-iff-`extracted` constraint stay untouched, and [P-0016](P-0016-edge-schema.md) ES-6's writer partition stays exactly two (the ingest harness only reaches the edge table as a caller of ES-6's sanctioned declared-writer path, per [P-0024](P-0024-ingest-pipeline-shape.md) IPS-6, spec R-0107-c). This is exactly the extension PE-7's trip-wire text named: "the extraction/ingest-integrity contract extends to `declared` edges." [P-0016](P-0016-edge-schema.md) ES-4's identically-fired trip-wire is resolved by this same keying.

### Artifact-side + the PE-11 ingest-half disposition

Lifecycle fields from `external`-class sources don't enter the `authoritative` predicate at all: their kinds resolve to `background` by PE-7's total-function default. And `model_egress: deny` gets stamped as an explicit value for external-class content. That's the PE-11 ingest-half disposition: there's no DDL-default flip. The polarity keys to source trust class instead, so the dogfood posture PE-11 already accepted stays untouched, while every future untrusted path inherits deny-by-default automatically. New source kinds get mapped this way rather than silently defaulting: binary-extracted content has no mechanical lifecycle field, so it resolves to `background`; watched-file markdown carries its declared kind's mapping under the trust-class rule above.

**PE-11's adopter-deployment half is not closed here.** An adopter running only `first-party` sources still inherits the permissive `model_egress` DDL default. That polarity revisit, whether the unlabeled-means-`model_egress: deny` default should flip for adopter deployments, stays live on PE-11's own firing instrument, the first deployment beyond the maintainer's own dogfood instance. This amendment's ingest-half disposition should not be read as closing that question.

### Scope of this amendment

This amendment adds only the `trust_class` keying input to the trust axis (PE-7), and it records the PE-7 N10 and PE-11 ingest-half firings. It does not add or change any PE-2 policy dimension, DDL default, validator, or enforcement point. It doesn't change the PE-3 decision port, the PE-4 citation-resolution dispositions, the PE-8 freshness schema, the PE-9 displacement registry, the PE-10 citation form, or the PE-6 write gate, through which an external-declared edge is still the only promotion path. `origin` semantics ([P-0016](P-0016-edge-schema.md) ES-2/ES-3) and ES-6's writer roster are unchanged: conformed to, not amended. It rides the ingestion-pipeline spec-exit gate for maintainer ratification, the same gate shape P-0015 itself was locked at.
