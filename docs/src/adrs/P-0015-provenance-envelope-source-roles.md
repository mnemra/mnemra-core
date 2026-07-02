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

Accepted 2026-07-02 at the retrieval-cluster **spec-exit gate** (reviewed with the spec [2026-07-02-retrieval-cluster](../../specs/2026-07-02-retrieval-cluster.md)). Authored at Stage 3 of the retrieval cluster, resolving the Frame's `{{P-0015}}` slot per [placeholder-resolution](placeholder-resolution.md). The two-axis redesign, the eight policy mechanics, and the V0 semantics below were **decomposer-ratified at the pre-gate maintainer walk and the frame-exit gate (2026-07-02)** and were not re-opened by the gate; what this ADR adds at spec precision — and what the gate reviewed — is the finalized labels, tables, dispositions, and the two rendering picks the Frame delegated here.

## Context and Problem Statement

Intake success criterion 2 commits every returned context item to a validator-gated provenance contract: how authoritative, how fresh, whether usable at all, with a stable citation. The intake's risk profile states the security fact precisely: the use-policy semantics **are an authorization control — a retrieval path that ignores them leaks by design**. The Frame's pre-gate maintainer walk replaced the round-1 single five-role value with **two orthogonal axes** — policy (a permissions record, enforced structurally) and trust (provenance, served as a label) — because a single value destroys information: an item can be sensitive *and* authoritative, or dont-use *and* background. The maintainer's framing, carried verbatim: **"capture the hard to change stuff now — the mechanics; the labels are the easy part."** This ADR is the slot that locks the contract. It reads and serves the G2/G3 substrate fields (cross-artifact authoritativeness + provenance/use-policy — knowledge-object survey, 2026-05-15) and renders the sources-with-roles finding's freshness and displacement advances (maintainer record, 2026-05-21).

## Decision Drivers

- **The envelope's policy side is a security layer, not metadata** (P-SecurityLayered keystone edge; intake risk profile (a)) — every security dimension needs structural enforcement at a named point, fail-closed, behind one decision boundary.
- **Validator-gated or no slot** (intake hard constraint; P-ShiftLeft D2 — validator before field): every dimension and every trust value is a mechanical predicate over stored fields/edges; no inference, no prose judgment, anywhere in assignment.
- **Lock the intrinsic mechanics; keep the labels easy to change** (P-LockContract; the maintainer's ratified framing): the record shape, enforcement discipline, and port boundary are the hard-to-change contract; dimension names and enums are deliberately ADR-amendable.
- **Bound the blast radius of the future policy engine** (the maintainer's stated intent for mechanic 8): engine substitution must replace one module, not a rewrite — the Cedar/NGAC permission-model research (maintainer record, locked 2026-05-18): "the engine sits behind mnemra's policy port… port substitution, not a rewrite"; precedents G-0017 FlagProvider, P-0010 D5 Storage trait.
- **No existence-disclosure oracle** (Stage-2 review M1): withholding must be caller-silent; the observability signal is operator-side.
- **Retrofit posture** (pre-gate walk item 11): identity, permissions, and audit are hard to retrofit — the columns and write-attribution exist from day one even while V0 is single-user.

## Considered Options

1. **Two-axis contract: policy permissions record + trust label, hardcoded validator predicates behind a single decision port (chosen).**
2. **The round-1 single five-role value** (`authoritative / background / outdated / sensitive / dont-use` with precedence) — **replaced at the pre-gate walk (ratified)**: single-value precedence destroys information across orthogonal facts. Recorded as the option the redesign chose against; its five validators survive as predicates feeding the two axes.
3. **Adopt a policy engine (Cedar-class) at V0** — rejected for V0: the V0 policy is four dimensions with hardcoded mechanical predicates; an engine is mechanism ahead of evidence (P-Defer, Simplicity). The port (PE-3) is the designed substitution seam; QA-9 renders the swap as a bounded change.
4. **Per-dimension config knobs** — rejected structurally (mechanic 6): a dimension without a declared validator + enforcement point + fail-closed pole + governed operation is not a dimension; config cannot declare those.

## Decision Outcome

**Chosen: Option 1**, rendered as PE-1..PE-11. Binding requirement text: spec [R-0025/R-0026](../../specs/2026-07-02-retrieval-cluster.md); QA-4/QA-7/QA-9 are the observable measures.

### PE-1 — The two-axis contract under the eight locked mechanics

Every returned item carries an envelope `{trust, freshness, citation}` (+ `re-eval pending` on decision-kind artifacts). The **policy record** is an enforcement input, not a served label (sole exception: the `dont_use` stub's policy reason). The eight mechanics are carried from the ratified walk record ([Frame §2/R3](../../intent/retrieval-cluster-frame.md)) verbatim in substance and bind every dimension, present and future: (1) orthogonal record shape, never a single enum; (2) validator-gated, no inference in assignment; (3) structural fail-closed — an **undecidable** security check resolves to **its** restrictive pole, per-dimension (undecidable = the validator's stored source field is present but malformed/unreadable, or missing where the schema requires presence — distinct from **not-set**, which is a defined permissive V0 default per dimension, PE-2, and never undecidable); a settable-but-unenforced security value may not exist in the schema; (4) named enforcement points — index-admission gate, model-egress gate (index- and query-time), serving predicate, citation stub (the citation-resolution point; the disposition is per-dimension — stub for `dont_use`, not-found for `visibility`, PE-4); no point, no dimension; (5) operation facet declared per dimension; write side anticipated structurally, not designed now; (6) closed-but-extensible at ADR tier — a new dimension is a P-0015 amendment declaring validator + enforcement point + fail-closed pole + governed operation, never a config knob; (7) permission-field writes are recorded, attributable, authorized writes; (8) one decision port (PE-3). *(Anchor: pre-gate walk ratification, 2026-07-02 — off-limits to reopen.)*

### PE-2 — Final V0 dimension set (labels finalized; mechanics untouched)

The Frame's proposed initial set is **adopted as the V0 set** — each dimension already conformed to mechanics 2–5, and no finding since has motivated a rename:

| Dimension | Values | Not-set default (permissive, DDL) | Mechanical validator | Enforcement point(s) | Fail-closed pole (undecidable only) | Governed operation |
|---|---|---|---|---|---|---|
| `dont_use` | set / not set | not set | Explicit use-policy field = `deny` (G3 substrate field; author/curator-set, never inferred) | Index-admission gate + query predicates + citation resolution (metadata-only stub) | Excluded — never chunked, embedded, egressed, or served; citation resolves to a metadata-only stub with the policy reason | serve/egress |
| `model_egress` | `allow` / `deny` | `allow` | Explicit egress field (G3 sensitivity-family; never inferred from content) | Model-egress gate — index-time (4.1/4.4 inclusion) and query-time (4.3 payloads) | `deny` | egress |
| `visibility` | `workspace` / `admin-only` / `owner-only` | `workspace` | Explicit visibility field; `admin-only` predicate: caller's `WorkspaceCtx` role is `Admin` (role-gated, no identity plumbing — gate outcome, Frame §2 item 15) | Serving predicate + citation resolution (not-found — PE-4; deliberately NOT a stub) | `owner-only` (V0: serve to no one — PE-4) | serve |
| `tenant_share` | `workspace-only` (fixed at V0) | `workspace-only` | Structural constant — a declarative hook over structural tenancy (P-0006); a future sharing capability must honor per-content holdbacks | Serving/tenancy predicates | `workspace-only` | serve/share |

**Not-set is a value, not an absence (r1 fold):** every policy column carries the permissive DDL default above, applied at ingest and by the G2/G3 migration to the (unlabeled) `0.14.0` corpus — a stored artifact row always holds a decidable value for every dimension **on those sanctioned write paths**; a value written via a direct substrate-layer bypass of the R-0025-f gate can be malformed, which mechanic 3 resolves to that dimension's restrictive pole — so the mechanic-3 fail-closed law fires **only** on genuinely undecidable (present-but-malformed) values, never on unlabeled content. This is PE-11's opt-in-labeling residual rendered structurally: an unlabeled artifact is served to workspace callers and egress-eligible at V0. The fail-closed-pole column governs the undecidable case only.

**`sensitive` does not survive — not as a stored value and not as a macro.** Its intent maps to `model_egress: deny` + `visibility: admin-only` (round-1 read semantics restored: admin-readable, observer-withheld). A macro would be a second name for the same stored facts (P-MinBlastRadius: one noun per concept) and a place for the composed meaning to drift from the enforced fields; documentation may explain the mapping, the schema carries only the two dimensions. `model_egress: deny` content is indexed **locally in full** (RC-1 local encoders are egress-free) — the policy record and the egress boundary reinforce rather than fight.

### PE-3 — The decision port: PARC shape, system principal, conformance, serving-path equivalence

All host-side enforcement points consult one boundary **`decide(principal, action, resource, context) → allow/deny + reason`** (Cedar PARC shape). V0 implementation: the hardcoded PE-2 validator predicates behind the port. Renderings the Frame delegated here:

- **Index-time principal (Stage-2 r2 N7):** the index-admission gate and index-time model-egress gate fire during batch builds with no caller; they consult the port under a distinguished **system principal** — a dedicated principal variant naming the invoking subsystem (e.g. the index builder), *not* a workspace role and *not* a synthetic admin. The PARC shape is thereby well-defined at every enforcement point.
- **Conformance is tested AT THE PORT:** a design-time conformance suite (fail-closed + attributes-only invariants; the P-0010 D5 two-adapter-test pattern) exists from V0 and runs against any implementation behind the port, including the seeded undecidable-dimension cases resolving to their restrictive poles (QA-9.3).
- **Serving-path placement (r1 fold; Frame C4 keystone):** the serving predicate's fragments are applied **in every channel's WHERE clause (FTS, dense) and at every recursion level of the traversal CTE** — mirroring the workspace predicate's placement — so a policy-restricted row never enters RRF fusion, rerank, or budget accounting, and rank order + budget reports are computed over the caller's entitled set only (spec R-0025-g is the binding text). **The predicate equally binds the relations edge projection (r2 fold):** an edge surfaces in the caller-facing `relations` bundle only if the caller is serve-entitled to **both** endpoint artifacts — incoming edges' source endpoints and both ends of deeper-hop edges included — because the traversal CTE's reachable-node set is not the returned edge list; a withheld edge is caller-silent (PE-4) with the audit row. No caller-facing edge projection may be an existence oracle for a `visibility`-withheld neighbor.
- **Serving-path equivalence — the pick (Stage-2 r2 N1):** the row-level serving predicate executes in Postgres and cannot consult `decide()` per row; it enforces through SQL fragments derived from the port's attribute vocabulary in **exactly one module**. Of the Frame's stated binary — (a) mechanically generate the fragment from the predicate definitions, or (b) mandate a differential conformance test — **this ADR locks (b): the differential conformance test.** The fragment and `decide()` must return identical allow/deny over a generated resource set covering every dimension × pole × undecidable case (QA-9.4), and the test demonstrably fails on a seeded divergence. Rationale: V0's predicates are a handful of hardcoded checks — a fragment *generator* is more mechanism than the evidence supports (Simplicity; P-Defer), while the differential test reuses an existing test pattern and directly renders the QA measure. Option (a) remains the recorded alternative a future amendment adopts if the dimension count grows enough that hand-maintained fragments become the drift risk — the differential test itself is the instrument that would catch that drift (a failing equivalence run is the firing signal).
- **Port cardinality (Stage-2 r2 N8):** this content-policy port is a **distinct per-domain PDP instance** sharing the port *pattern* with the host-fn capability gate — not a shared engine. Unification, if ever, is a decision for a future amendment of this ADR together with the host-fn gate's record (minimal-coupling reading of P-MinBlastRadius). The Cedar/NGAC research anchor validates the port pattern and PARC-shaped boundary generally; its 1:1 PARC-fit finding was scoped to the host-fn capability gate and is not read as a content-policy-specific fit.

*(Anchors: the Cedar/NGAC permission-model research (maintainer record, locked 2026-05-18); cedar-policy 4.x is Apache-2.0/Green when the engine substitution fires; P-LockContract; G-0017 FlagProvider + P-0010 D5 precedents.)*

### PE-4 — V0 enforcement semantics + citation-resolution dispositions

- **`visibility: owner-only` serves no one at V0** (ratified; mechanic 3 applied): caller identity is undecidable under the workspace+role context, so owner-only items are excluded from all retrieval, for every caller including `Admin`, until identity lands. The conservative enforcement IS the V0 semantics — not a settable-but-unenforced value.
- **`visibility: admin-only`** serves callers whose `WorkspaceCtx` role is `Admin`; withheld from `read_observer` (gate outcome, Frame §2 item 15).
- **Withholding is caller-silent** — no count, no placeholder — with an **operator-side audit event** per withheld decision, stored in the spec Data Model's `policy_decision_audit` table (`{workspace_id, artifact_id, action, dimension, decision, reason, principal, occurred_at}`; additionally emitted as telemetry, generation⊥storage) — the P-TrustworthySignal-satisfying signal; a caller-visible marker is an existence-disclosure oracle (Stage-2 M1). "Marker" includes an **edge** (r2 fold): a `relations`-bundle entry naming a withheld artifact would disclose its existence, relationship, edge type/origin, and citation, so the serving predicate filters the edge projection's endpoints (PE-3; spec R-0025-g) and a withheld edge is indistinguishable from no-edge. An edge to a `dont_use` artifact does surface — curatorial policy announces itself (the stub disposition below); the asymmetry follows each dimension's meaning exactly as at citation resolution.
- **Citation-resolution dispositions (the Stage-2 L2 call, locked):** `get_artifact_by_citation` on a `dont_use` artifact returns a **metadata-only stub** carrying the policy reason and no content — the curatorial kill switch is *supposed* to tell the agent "this exists; do not use it." Resolution of a **visibility-withheld** artifact by an unauthorized caller returns **not-found, indistinguishable from a genuinely unresolvable reference** — visibility withholds *existence*, and a stub would be a probe-able oracle. The asymmetry is deliberate and follows each dimension's meaning: curatorial policy announces itself; audience restriction does not. *(Anchors: P-SecurityLayered fail-closed; QA-4 measures 1/3/4/8.)*

### PE-5 — Enforcement-layer asymmetry; the owner-only index-partition question (deferred, named)

`dont_use` gets dual-layer defense (index admission + query predicates — the content must not exist in the index at all); `visibility` enforces at serve time only, fully indexed — an audience restriction *within* the workspace, not a curatorial kill (Stage-2 r2 N6, stated deliberately). When per-user identity lands and `owner-only` becomes per-user, the serving predicate becomes the entire control for audience-restricted content in a shared index. **Whether that warrants an index-partition / admission-level control is deferred to the per-user identity feature** — decision content: partition-vs-predicate for per-user owner-only; deferral anchor: P-Defer (the mechanism's shape depends on the identity design, which does not exist); firing instrument: **self-announcing** — the per-user identity machinery register entry (added to the product brief with this change) cannot be designed without resolving owner-only serving semantics, so its intake fires this question by construction.

### PE-6 — Write authorization: the V0 invariant, the audit record, the deferred write side

- **V0 write gate (frame-exit gate outcome, item 14):** ALL writes to policy/trust-affecting fields — permission flips, freshness overrides — ride the existing **admin-gated content-mutation path** ([P-0009](P-0009-rls-admin-token.md)), with `read_observer` excluded; no other write path exists at V0. This closes the V0 authorization window on authorization-bearing writes: clearing an egress-deny flag makes restricted content servable; a spurious freshness override masks staleness (Frame §5 write-authorization surface, Stage-2 M3).
- **Recorded, attributable writes (mechanic 7):** every label/permission change writes an audit row `{workspace_id, artifact_id, field, old_value, new_value, actor token_id, occurred_at}` (spec Data Model `policy_write_audit`); actor attribution is NOT NULL.
- **Write-side policy dimensions stay undesigned** (mechanic 5): the read/write split is anticipated structurally; the write-operation dimension set arrives as P-0015 amendments (mechanic 6) when the **first surface granting a non-owner actor write/label capability** appears (per-user identity, `tenant_share` sharing, plugin-mediated writes) — self-announcing.
- **Timed embargo** (auto-expiring permission values) is a recorded **design option, unbuilt**: if built, it is a dimension-model amendment declaring validator + enforcement point + expiry semantics. Firing: self-announcing — a real embargo case is asked for. Until then, permission changes are owner-mutable flips only (recorded writes).

### PE-7 — Trust axis: validators, precedence, the per-kind mapping, origin weighting

One value per item by first-match precedence **`outdated` → `authoritative` → `background`**; every predicate mechanical (no LLM, no prose judgment — spec R-0026-a).

- **`outdated`** fires on (a) an incoming `supersedes` edge of origin `declared` or `system`, or (b) freshness-stale (PE-8) with no recorded override. **Origin weighting, locked:** an `origin = extracted` supersedes edge does **not** enter the trust predicate at V0 — it is recorded, traversable, surfaced in the relations bundle (to callers serve-entitled to both endpoints — PE-3's projection filter, spec R-0025-g), and raises an operator-side curatorial signal; promotion to trust effect happens only by re-writing it as `declared` through the PE-6 gate. Rationale: extraction reads free text, and adversarially-shaped corpus text ("…this supersedes P-0010…") must never demote a target's trust automatically (Frame §5 worked case; residual (iv)). **The `declared` weight itself rests on the first-party-corpus assumption** (Non-goal 1 — no ongoing ingest): frontmatter carries authority because only first-party authors write it. **Named trip-wire (Stage-2 r2 N10):** when register `1.2.0` (ongoing ingest) opens the first untrusted-submitter path into frontmatter, this weighting re-opens and the extraction/ingest-integrity contract extends to `declared` edges — the `1.2.0` feature's own intake is the self-announcing firing event.
- **`authoritative`** fires iff the artifact kind's lifecycle field is in that kind's closed authoritative-state set AND `outdated` did not fire. The V0 mapping table (extending it is a P-0015 amendment, per-kind, validator-first):

  | Artifact kind | Lifecycle field's authoritative-state set |
  |---|---|
  | Decision record (ADR) | `accepted` |
  | Spec | `approved` |
  | Intake / Frame / product brief | `locked` |
  | Research brief | `locked` |
  | *(any kind not in this table)* | *(none — the kind cannot fire `authoritative`; it resolves `background`)* |

  Kinds without a mechanical lifecycle field (code, logs, tasks, living canon docs) resolve `background` by the total-function default — served, labeled, ranked normally. This is deliberately conservative and fail-safe: `background` under-claims rather than over-claims authority; adding a kind to the table is a cheap, validator-first amendment when the under-claim demonstrably misranks (the run records are the instrument that would show it).
- **`background`** is the total-function default — no other predicate fires.
- **Retrieval-path semantics** (per the Frame's trust table; mechanism defined at the r1 fold): **hard-superseded** means the artifact has an incoming `supersedes` edge of origin `declared`/`system` — the "`superseded-by` forward pointer" is exactly that edge read from the superseded side (the directional view; no second schema field, no topic-key inference — P-0016 ES-2, spec R-0029-e). Every predicate-(a) `outdated` item is therefore hard-superseded: **excluded** from default retrieval, point-in-time reachable. The served-with-label-and-demoted disposition applies exactly to **predicate-(b)** (freshness-stale) items. `authoritative` ranks trust-primary; `background` serves labeled.

### PE-8 — Freshness schema: handle-diff primary, decay-class fallback, structural overrides

- **Version handles (primary):** where an item cites a source with a handle, the envelope stores the handle-at-index and reports `current | moved | unknown` by diffing against the source's live handle. **Closed handle-kind set at V0:** `git-sha` (canon/repo citations — live handle is the repo's current state), `semver` (dependency/tool versions), `model-id` (model revisions — diffed against the configured pinned revision), `doc-version` (explicit version strings on versioned documents). An unreachable live handle reports `unknown` — never a fabricated state (in the zero-egress configuration, external live-handle checks resolve `unknown` by construction). Adding a handle kind is a P-0015 amendment declaring its differ (validator before field).
- **Decay classes (fallback; bounds ignorance, does not measure staleness):** handle-less sources carry a volatility-matched class; TTL expiry without re-validation makes the item freshness-stale (feeding PE-7's predicate (b)). The V0 class table:

  | Class | TTL | Domain examples |
  |---|---|---|
  | `volatile` | 30 d | vendor/dependency landscape, pricing, model availability |
  | `moderate` | 90 d | comparative research, tool evaluations |
  | `slow` | 180 d | project conventions, operational skills |
  | `stable` | 365 d | principles, foundational decisions, values |

  Time is the weakest signal — fallback, never primary. TTL values are class defaults, amendable per class; the class assignment is a stored field.
- **Overrides are structural:** a freshness override is a recorded row `{by, reason, date}` accepted only through the PE-6 write gate — never a config toggle. An override restores the prior trust value and is itself visible in the envelope. Content-based staleness detection is rejected as circular (the maintainer's advance 1, 2026-05-21).

### PE-9 — Displacement ≠ staleness: the event registry

Handle-diff catches my-cited-source-changed, not the-world-moved-past-it. Containment, not solution: a **named displacement-event registry** (spec Data Model `displacement_events`) whose fired events set a `re-eval pending (axis)` envelope flag on decisions whose **recorded axes** intersect the event. The V0 event-kind vocabulary is **closed and enumerated** (r1 fold — an enumerable registry with zero enumerated kinds would have no buildable fixture); extending it is a P-0015 amendment declaring the kind's mechanical firing condition (validator-first, the mechanic-6 discipline):

| Event kind | Fires (mechanically) |
|---|---|
| `canon-superseded` | A `supersedes` edge of origin `declared`/`system` lands on a canon-kind artifact (ADR, principle, spec) — detected on the edge-write path this cluster builds |
| `dependency-major-change` | A `semver` version-handle diff (PE-8) crosses a major version boundary — detected by the freshness handle-diff this cluster builds |
| `operator-declared` | The operator records an event through the PE-6 write gate, naming the axis + description — the catch-all for world-shifts no detector covers |

**Single-axis displacement triggers re-evaluation, never auto-invalidation** — the trust value does not flip on a displacement flag; multi-axis decisions survive a single-axis shift. Decision-kind artifacts record their decision axes as substrate data. Honest limit, carried verbatim: trip-wires only cover enumerable displacement events; truly unforeseen shifts fall back entirely to the volatility TTL. *(Anchor: the maintainer's advance 2, 2026-05-21; P-PreserveDecisionSpace.)*

### PE-10 — Citation form

Every envelope item carries a stable citation: **artifact ID (content-addressed, per the V0 substrate) + block anchor where the item is a chunk**, resolvable by `get_artifact_by_citation`. The ID+name pairing and block addressability are what make citation-expansion retrieval work. *(Anchor: P-AgentPrimarySource.)*

### PE-11 — Residuals owned here

- **Labeling correctness + the adopter-facing polarity revisit (Frame residual (i), maintainer-accepted for V0):** labeling is opt-in at V0 — an unlabeled artifact is egressable at index time; the index-admission gate mitigates, the serving-time posture stays fail-closed. Rendered structurally as the PE-2 not-set DDL defaults: unlabeled = the permissive default values, stored — so the fail-closed law (mechanic 3) targets only genuinely undecidable values and can never silently withhold the unlabeled corpus. This ADR owns the revisit: whether the default polarity should flip (unlabeled ⇒ egress-deny — mechanically, flipping the `model_egress` DDL default for new rows) for adopter deployments. Firing: self-announcing — the first deployment beyond the maintainer's dogfood instance (an adopter standing up an instance is an explicit act), or register `1.2.0` ingest, whichever first.
- **Query-time egress content-audit (Stage-2 L5):** which query text (4.2) / which retrieved chunk citations (4.3) egressed, keyed for incident response — landed as spec R-0034-b; recorded here as part of the envelope contract's enforcement observability.

### Consequences

**Good:**
- The hard-to-change parts (record shape, enforcement discipline, port boundary, fail-closed law, write attribution) are locked while every label stays a cheap ADR amendment — the ratified cost asymmetry, honored.
- Engine substitution is a bounded change (one PDP module + one fragment module), proven by a conformance suite that exists from V0 (QA-9).
- No existence oracle anywhere on the caller surface; every withhold is operator-observable.
- Adversarial corpus text cannot flip trust (PE-7's origin weighting) and cannot reach served payloads through 4.1 (P-0014 RA-9).

**Bad / Trade-offs:**
- `owner-only` over-blocks at V0 (serves no one, even the owner) — the deliberate price of mechanic 3 until identity lands; the identity feature's intake is the named re-open.
- The differential conformance test must be maintained as dimensions evolve — accepted over a fragment generator at V0 scale; a failing equivalence run is itself the drift instrument.
- Kinds outside the PE-7 mapping under-claim authority (`background`) — fail-safe, but a ranking cost until the table is extended on evidence.
- Freshness `unknown` is common in zero-egress deployments (external handles unreachable) — honest by design; TTL still bounds ignorance.

## Pros and Cons of the Options

### Two-axis record + port, hardcoded predicates (chosen)

- Pro: carries orthogonal facts without information loss; per-axis determinism retained.
- Pro: enforcement is structural, fail-closed, single-boundary — and V0-simple (four predicates).
- Con: two axes are more schema than one enum — paid once, at the layer that is hardest to retrofit.

### Round-1 single five-role value (replaced)

- Pro: one field, simple precedence.
- Con: destroys information (sensitive+authoritative, dont-use+background are real); precedence fights itself. Ratified out at the pre-gate walk; its five validators survive as predicates.

### Cedar-class engine at V0 (rejected for V0)

- Pro: expressive policy language, forward-ready.
- Con: mechanism ahead of evidence for four hardcoded predicates; the port makes later adoption a substitution, not a rewrite (QA-9 is the proof obligation).

### Config-knob dimensions (rejected)

- Pro: no ADR churn to add a label.
- Con: violates mechanic 6 — config cannot declare a validator, enforcement point, fail-closed pole, or governed operation; a knob-created "dimension" is unenforced by construction.

## More Information

- Binding requirement text: [spec R-0025/R-0026](../../specs/2026-07-02-retrieval-cluster.md); observable measures QA-4/QA-7/QA-9 ([Frame §7](../../intent/retrieval-cluster-frame.md)).
- Companions: [P-0014](P-0014-retrieval-architecture.md) (the query path and gates that consume `decide()`; the 4.1 control RA-9 co-owned with this contract); [P-0016](P-0016-edge-schema.md) (the edge `origin` column PE-7 weights; the extraction-integrity contract).
- Ratification trail: the pre-gate maintainer walk + frame-exit gate outcomes (Frame §2, items 1–15 — the eight mechanics verbatim; `admin-only` added at the gate; the V0 write invariant confirmed at the gate).
- Sources by name + lock-date (provenance-pointer convention): sources-with-roles finding + freshness/displacement advances (maintainer record, 2026-05-21); Cedar/NGAC permission-model research (maintainer record, locked 2026-05-18); knowledge-object survey G2/G3 (2026-05-15).
- Substrate: [P-0006](P-0006-v0-tenant-enforcement.md) / [P-0009](P-0009-rls-admin-token.md) (the caller context and admin-gated mutation path the V0 enforcement rides).
