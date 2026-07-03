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

Accepted on 2026-07-02 at the retrieval-cluster **spec-exit gate**, reviewed alongside the spec [2026-07-02-retrieval-cluster](../../specs/2026-07-02-retrieval-cluster.md). This ADR (architecture decision record: it captures a significant technical decision, the context behind it, the alternatives it ruled out, and the expected consequences) was authored during Stage 3 of the retrieval cluster's work-shaping pipeline, the Spec stage where agents turn a frame document into a testable, locked spec. It resolves the `{{P-0015}}` placeholder left by the Frame (Stage 2, where agents synthesize operating constraints from validated intent), following [placeholder-resolution](placeholder-resolution.md).

The two-axis redesign, the eight policy mechanics, and the V0 semantics below (V0 is the first version actually shipped) were already **decomposer-ratified**, meaning Stage 1's intake owner signed off, at the pre-gate maintainer walk and at the frame-exit gate on 2026-07-02. The gate didn't reopen any of that. What this ADR adds at spec precision, and what the gate reviewed, is the finalized labels, tables, dispositions, and the two rendering choices the Frame left for this document to settle.

## Context and Problem Statement

Intake is Stage 1 of the work-shaping pipeline, where the decomposer captures the job-to-be-done, constraints, and success criteria. Its success criterion 2 commits every returned context item to a validator-gated provenance contract that covers authority, freshness, usability, and stable citation. The intake's risk profile states the security fact precisely: the use-policy semantics are an authorization control. A retrieval path that ignores them leaks by design.

The Frame's pre-gate maintainer walk replaced the round-1 single five-role value with two orthogonal axes: policy (a permissions record, enforced structurally) and trust (provenance, served as a label). A single value destroys information. An item can be sensitive and authoritative, or dont-use and background, at the same time.

The maintainer's framing, carried verbatim: "capture the hard to change stuff now: the mechanics; the labels are the easy part." This ADR is the slot that locks that contract. It reads and serves the G2/G3 substrate fields (cross-artifact authoritativeness plus provenance and use-policy, from the knowledge-object survey, 2026-05-15) and renders the freshness and displacement advances from the sources-with-roles finding (maintainer record, 2026-05-21).

## Decision Drivers

- **The envelope's policy side is a security layer, not metadata** (P-SecurityLayered keystone edge; intake risk profile item a). Every security dimension needs structural enforcement at a named point, fails closed, and sits behind one decision boundary.
- **Validator-gated or no slot** (intake hard constraint; P-ShiftLeft D2, validator before field). Every dimension and every trust value is a mechanical predicate over stored fields and edges. No inference, no prose judgment, anywhere in assignment.
- **Lock the intrinsic mechanics; keep the labels easy to change** (P-LockContract: lock the contract, vary the implementation; the maintainer's ratified framing). The record shape, enforcement discipline, and port boundary are the hard-to-change contract. Dimension names and enums are deliberately open to change through a later ADR.
- **Bound the blast radius of the future policy engine** (the maintainer's stated intent for mechanic 8). Engine substitution must replace one module, not force a rewrite. The Cedar/NGAC permission-model research (maintainer record, locked 2026-05-18) put it this way: "the engine sits behind mnemra's policy port… port substitution, not a rewrite." Precedents: G-0017 FlagProvider (a workspace-wide architecture decision, per the G-* naming convention) and P-0010 D5 Storage trait (a project-scoped architecture decision, per the P-* naming convention).
- **No existence-disclosure oracle** (Stage-2 review finding M1). Withholding must stay silent to the caller; the observability signal lives on the operator side.
- **Retrofit posture** (pre-gate walk item 11). Identity, permissions, and audit are hard to retrofit after the fact, so the columns and write-attribution exist from day one even while V0 is single-user.

## Considered Options

1. **Two-axis contract: policy permissions record plus trust label, hardcoded validator predicates behind a single decision port (chosen).**
2. **The round-1 single five-role value** (`authoritative / background / outdated / sensitive / dont-use` with precedence). Replaced at the pre-gate walk and ratified there: single-value precedence destroys information across facts that are actually independent of each other. Recorded as the option the redesign chose against; its five validators survive as predicates feeding the two axes.
3. **Adopt a policy engine (Cedar-class) at V0.** Rejected for V0: the V0 policy is four dimensions with hardcoded mechanical predicates, and an engine is mechanism ahead of evidence (P-Defer: defer a mechanism choice until evidence forces it; Simplicity). The port (PE-3, defined below) is the designed substitution seam; QA-9 renders the swap as a bounded change.
4. **Per-dimension config knobs.** Rejected structurally (mechanic 6): a dimension without a declared validator, enforcement point, fail-closed pole, and governed operation isn't a dimension. Config can't declare those.

## Decision Outcome

**Chosen: Option 1**, rendered below as PE-1 through PE-11. Binding requirement text lives in spec [R-0025/R-0026](../../specs/2026-07-02-retrieval-cluster.md) (R-codes are stable identifiers for numbered requirements; the full text of each lives in the requirements document that defines it). QA-4, QA-7, and QA-9 are the observable measures used to check the requirement was met.

### PE-1 — The two-axis contract under the eight locked mechanics

Every returned item carries an envelope `{trust, freshness, citation}` (plus a `re-eval pending` flag on decision-kind artifacts). The **policy record** is an enforcement input, not a label served to the caller. The sole exception is the `dont_use` stub's policy reason, which is shown.

The eight mechanics below are carried from the ratified walk record ([Frame §2/R3](../../intent/retrieval-cluster-frame.md)) verbatim in substance, and they bind every dimension, present and future:

1. Orthogonal record shape. Never a single enum.
2. Validator-gated. No inference in assignment.
3. Structural fail-closed. An **undecidable** security check resolves to its restrictive pole, per dimension. Undecidable means the validator's stored source field is present but malformed or unreadable, or missing where the schema requires it to be present. This is distinct from **not-set**, which is a defined permissive V0 default per dimension (PE-2) and is never undecidable. A settable-but-unenforced security value isn't allowed to exist in the schema at all.
4. Named enforcement points: the index-admission gate, the model-egress gate (both index-time and query-time), the serving predicate, and the citation stub (the citation-resolution point). The disposition is per-dimension: a stub for `dont_use`, not-found for `visibility` (PE-4). No enforcement point, no dimension.
5. An operation facet declared per dimension. The write side is anticipated structurally but not designed yet.
6. Closed but extensible at the ADR tier. A new dimension requires a P-0015 amendment declaring a validator, an enforcement point, a fail-closed pole, and a governed operation. It's never a config knob.
7. Permission-field writes are recorded, attributable, authorized writes.
8. One decision port (PE-3, below).

*(Anchor: pre-gate walk ratification, 2026-07-02. Off-limits to reopen.)*

### PE-2 — Final V0 dimension set (labels finalized; mechanics untouched)

The Frame's proposed initial set is **adopted as the V0 set**. Each dimension already conformed to mechanics 2 through 5, and nothing found since has motivated a rename:

| Dimension | Values | Not-set default (permissive, DDL) | Mechanical validator | Enforcement point(s) | Fail-closed pole (undecidable only) | Governed operation |
|---|---|---|---|---|---|---|
| `dont_use` | set / not set | not set | Explicit use-policy field = `deny` (G3 substrate field; author/curator-set, never inferred) | Index-admission gate + query predicates + citation resolution (metadata-only stub) | Excluded — never chunked, embedded, egressed, or served; citation resolves to a metadata-only stub with the policy reason | serve/egress |
| `model_egress` | `allow` / `deny` | `allow` | Explicit egress field (G3 sensitivity-family; never inferred from content) | Model-egress gate — index-time (4.1/4.4 inclusion) and query-time (4.3 payloads) | `deny` | egress |
| `visibility` | `workspace` / `admin-only` / `owner-only` | `workspace` | Explicit visibility field; `admin-only` predicate: caller's `WorkspaceCtx` role is `Admin` (role-gated, no identity plumbing — gate outcome, Frame §2 item 15) | Serving predicate + citation resolution (not-found — PE-4; deliberately NOT a stub) | `owner-only` (V0: serve to no one — PE-4) | serve |
| `tenant_share` | `workspace-only` (fixed at V0) | `workspace-only` | Structural constant — a declarative hook over structural tenancy (P-0006); a future sharing capability must honor per-content holdbacks | Serving/tenancy predicates | `workspace-only` | serve/share |

**Not-set is a value, not an absence (r1 fold):** every policy column carries the permissive DDL default (data definition language default; the value a database column takes when nothing overrides it) shown above, applied at ingest and by the G2/G3 migration to the unlabeled `0.14.0` corpus. A stored artifact row always holds a decidable value for every dimension on those sanctioned write paths. A value written through a direct substrate-layer bypass of the R-0025-f gate can be malformed, and mechanic 3 resolves that to the dimension's restrictive pole. So the mechanic-3 fail-closed rule fires only on genuinely undecidable (present-but-malformed) values, never on unlabeled content.

This is PE-11's opt-in-labeling residual, rendered structurally: an unlabeled artifact is served to workspace callers and stays egress-eligible at V0. The fail-closed-pole column governs the undecidable case only.

**`sensitive` doesn't survive, not as a stored value and not as a macro.** Its intent maps to `model_egress: deny` plus `visibility: admin-only` (this restores the round-1 read semantics: admin-readable, withheld from observers). A macro would just be a second name for the same stored facts. That violates P-MinBlastRadius (a change should reach only as far as the architecture allows; here, one noun per concept), and it creates a place for the composed meaning to drift from the enforced fields. Documentation can explain the mapping, but the schema carries only the two dimensions.

`model_egress: deny` content is still indexed locally in full, because RC-1's local encoders are egress-free. The policy record and the egress boundary reinforce each other instead of fighting.

### PE-3 — The decision port: PARC shape, system principal, conformance, serving-path equivalence

All host-side enforcement points consult one boundary: **`decide(principal, action, resource, context) → allow/deny + reason`** (the Cedar PARC shape, a request format naming Principal, Action, Resource, and Context). The V0 implementation behind that port is just the hardcoded PE-2 validator predicates. Below are the renderings the Frame left for this document to decide:

- **Index-time principal (Stage-2 r2 N7).** The index-admission gate and the index-time model-egress gate fire during batch builds, with no caller present. They consult the port under a distinguished **system principal**: a dedicated principal variant naming the invoking subsystem (the index builder, for example), not a workspace role and not a synthetic admin account. That keeps the PARC shape well-defined at every enforcement point.
- **Conformance is tested at the port.** A design-time conformance suite, covering fail-closed and attributes-only invariants (the P-0010 D5 two-adapter-test pattern), exists from V0 and runs against any implementation behind the port. That includes seeded undecidable-dimension cases resolving to their restrictive poles (QA-9.3).
- **Serving-path placement (r1 fold; Frame C4 keystone).** The serving predicate's fragments are applied in every channel's WHERE clause (full-text search, dense) and at every recursion level of the traversal CTE (the recursive query that walks the graph), mirroring where the workspace predicate already sits. That means a policy-restricted row never enters RRF fusion, reranking, or budget accounting, and rank order and budget reports are computed over the caller's entitled set only (spec R-0025-g is the binding text). The predicate equally binds the relations edge projection (r2 fold). An edge surfaces in the caller-facing `relations` bundle only if the caller is serve-entitled to both endpoint artifacts: incoming edges' source endpoints, and both ends of deeper-hop edges. That's because the traversal CTE's reachable-node set isn't the same thing as the returned edge list. A withheld edge stays caller-silent (PE-4) but generates an audit row. No caller-facing edge projection may act as an existence oracle for a neighbor withheld by `visibility`.
- **Serving-path equivalence, the pick (Stage-2 r2 N1).** The row-level serving predicate executes inside Postgres and can't call `decide()` per row, so it enforces through SQL fragments derived from the port's attribute vocabulary, kept in exactly one module. The Frame left a binary choice here: (a) mechanically generate the fragment from the predicate definitions, or (b) mandate a differential conformance test. This ADR locks option (b), the differential conformance test. The fragment and `decide()` must return identical allow/deny decisions over a generated resource set covering every dimension, pole, and undecidable case (QA-9.4), and the test has to demonstrably fail on a seeded divergence. The reasoning: V0's predicates are a handful of hardcoded checks, so a fragment generator would be more mechanism than the evidence supports (Simplicity; P-Defer). The differential test, by contrast, reuses an existing test pattern and directly produces the QA measure. Option (a) stays on record as the alternative a future amendment can adopt if the dimension count grows enough that hand-maintained fragments become the drift risk; the differential test itself is the instrument that would catch that drift, since a failing equivalence run is the signal that fires the amendment.
- **Port cardinality (Stage-2 r2 N8).** This content-policy port is a distinct per-domain PDP (policy decision point) instance. It shares the port pattern with the host-fn capability gate, but it isn't a shared engine. Unifying the two, if that ever happens, is a decision for a future amendment of this ADR together with the host-fn gate's own record, reading P-MinBlastRadius as calling for minimal coupling here. The Cedar/NGAC research anchor validates the port pattern and the PARC-shaped boundary in general; its finding that PARC fits 1:1 was scoped to the host-fn capability gate specifically and isn't read as a content-policy-specific fit.

*(Anchors: the Cedar/NGAC permission-model research, maintainer record locked 2026-05-18; cedar-policy 4.x is Apache-2.0/Green when the engine substitution fires; P-LockContract; the G-0017 FlagProvider and P-0010 D5 precedents.)*

### PE-4 — V0 enforcement semantics + citation-resolution dispositions

- **`visibility: owner-only` serves no one at V0** (ratified; mechanic 3 applied). Caller identity is undecidable under the current workspace-plus-role context, so owner-only items are excluded from all retrieval, for every caller including `Admin`, until identity lands as a feature. This conservative enforcement is the V0 semantics, not a settable-but-unenforced value sitting unused in the schema.
- **`visibility: admin-only`** serves callers whose `WorkspaceCtx` role is `Admin`, and is withheld from `read_observer` (a gate outcome, Frame §2 item 15).
- **Withholding is caller-silent.** No count, no placeholder. Instead there's an operator-side audit event per withheld decision, stored in the spec Data Model's `policy_decision_audit` table (`{workspace_id, artifact_id, action, dimension, decision, reason, principal, occurred_at}`), also emitted as telemetry, generated independently of storage. That's the signal that satisfies P-TrustworthySignal; a caller-visible marker would be an existence-disclosure oracle (Stage-2 finding M1). "Marker" includes an **edge** (r2 fold): a `relations`-bundle entry naming a withheld artifact would disclose its existence, its relationship, its edge type and origin, and its citation. So the serving predicate filters the edge projection's endpoints too (PE-3; spec R-0025-g), and a withheld edge reads exactly like no edge at all. An edge to a `dont_use` artifact does surface, though: curatorial policy announces itself (see the stub disposition below). The asymmetry follows each dimension's meaning exactly the way it does at citation resolution.
- **Citation-resolution dispositions (the Stage-2 L2 call, locked):** `get_artifact_by_citation` on a `dont_use` artifact returns a **metadata-only stub** carrying the policy reason and no content. The curatorial kill switch is supposed to tell the agent "this exists, don't use it." Resolving a **visibility-withheld** artifact for an unauthorized caller returns **not-found**, indistinguishable from a genuinely unresolvable reference. Visibility withholds existence itself, and a stub there would be a probe-able oracle. The asymmetry is deliberate and follows each dimension's meaning: curatorial policy announces itself, audience restriction doesn't. *(Anchors: P-SecurityLayered fail-closed; QA-4 measures 1, 3, 4, and 8.)*

### PE-5 — Enforcement-layer asymmetry; the owner-only index-partition question (deferred, named)

`dont_use` gets dual-layer defense: index admission plus query predicates, because the content must not exist in the index at all. `visibility` enforces at serve time only and stays fully indexed: it's an audience restriction within the workspace, not a curatorial kill (Stage-2 r2 N6, stated deliberately).

When per-user identity lands and `owner-only` becomes genuinely per-user,the serving predicate becomes the entire control for audience-restricted content in a shared index. Whether that warrants an index-partition or admission-level control is deferred to the per-user identity feature. Decision content: partition versus predicate for per-user owner-only. Deferral anchor: P-Defer, since the mechanism's shape depends on an identity design that doesn't exist yet. Firing instrument: self-announcing. The per-user identity machinery's register entry (added to the product brief with this change) can't be designed without resolving owner-only serving semantics, so its intake fires this question by construction.

### PE-6 — Write authorization: the V0 invariant, the audit record, the deferred write side

- **V0 write gate (frame-exit gate outcome, item 14).** Every write to a policy- or trust-affecting field, whether a permission flip or a freshness override, rides the existing admin-gated content-mutation path ([P-0009](P-0009-rls-admin-token.md)), with `read_observer` excluded. No other write path exists at V0. That closes the V0 authorization window on authorization-bearing writes: clearing an egress-deny flag makes restricted content servable, and a spurious freshness override masks staleness (Frame §5 write-authorization surface, Stage-2 finding M3).
- **Recorded, attributable writes (mechanic 7).** Every label or permission change writes an audit row `{workspace_id, artifact_id, field, old_value, new_value, actor token_id, occurred_at}` (spec Data Model `policy_write_audit`). Actor attribution is `NOT NULL`.
- **Write-side policy dimensions stay undesigned** (mechanic 5). The read/write split is anticipated structurally, but the write-operation dimension set arrives as P-0015 amendments (mechanic 6) only when the first surface granting a non-owner actor write or label capability shows up: per-user identity, `tenant_share` sharing, or plugin-mediated writes. Self-announcing.
- **Timed embargo** (auto-expiring permission values) is a recorded design option that isn't built. If it's ever built, it's a dimension-model amendment declaring a validator, an enforcement point, and expiry semantics. Firing: self-announcing, when a real embargo case is asked for. Until then, permission changes are owner-mutable flips only, recorded as writes.

### PE-7 — Trust axis: validators, precedence, the per-kind mapping, origin weighting

One trust value per item, chosen by first-match precedence: `outdated`, then `authoritative`, then `background`. Every predicate is mechanical: no LLM, no prose judgment (spec R-0026-a).

- **`outdated`** fires on (a) an incoming `supersedes` edge of origin `declared` or `system`, or (b) freshness-stale (PE-8) with no recorded override.

  Origin weighting is locked here: an `origin = extracted` supersedes edge does not enter the trust predicate at V0. It's recorded, it's traversable, it surfaces in the relations bundle for callers serve-entitled to both endpoints (PE-3's projection filter, spec R-0025-g), and it raises an operator-side curatorial signal. Promotion to trust effect happens only by re-writing the edge as `declared` through the PE-6 gate. The reasoning: extraction reads free text, and adversarially-shaped corpus text like "…this supersedes P-0010…" must never demote a target's trust automatically (Frame §5 worked case; residual (iv)).

  The `declared` weight itself rests on a first-party-corpus assumption (Non-goal 1: no ongoing ingest). Frontmatter carries authority because only first-party authors write it. Named trip-wire (Stage-2 r2 N10): when register `1.2.0` (ongoing ingest) opens the first untrusted-submitter path into frontmatter, this weighting reopens, and the extraction and ingest-integrity contract extends to `declared` edges too. The `1.2.0` feature's own intake is the self-announcing event that fires this.

- **`authoritative`** fires only if the artifact kind's lifecycle field sits in that kind's closed authoritative-state set, and `outdated` didn't already fire. The V0 mapping table follows (extending it is a P-0015 amendment, done per kind, validator-first):

  | Artifact kind | Lifecycle field's authoritative-state set |
  |---|---|
  | Decision record (ADR) | `accepted` |
  | Spec | `approved` |
  | Intake / Frame / product brief | `locked` |
  | Research brief | `locked` |
  | *(any kind not in this table)* | *(none — the kind cannot fire `authoritative`; it resolves `background`)* |

  Kinds without a mechanical lifecycle field (code, logs, tasks, living canon docs) resolve to `background` by default, since the predicate always has to return something. They're served, labeled, and ranked normally. This is deliberately conservative and fail-safe: `background` under-claims authority rather than over-claiming it. Adding a kind to the table is a cheap, validator-first amendment, done when the under-claim demonstrably misranks results (the run records are the instrument that would show that).

- **`background`** is the default when no other predicate fires.

- **Retrieval-path semantics** (per the Frame's trust table; the mechanism was defined at the r1 fold). Hard-superseded means the artifact has an incoming `supersedes` edge of origin `declared` or `system`. The "`superseded-by` forward pointer" is exactly that same edge, read from the superseded side: it's a directional view, not a second schema field or a topic-key inference (P-0016 ES-2, spec R-0029-e). Every predicate-(a) `outdated` item is therefore hard-superseded: excluded from default retrieval, but still reachable point-in-time. The served-with-label-and-demoted disposition applies specifically to predicate-(b) items, the freshness-stale ones. `authoritative` ranks trust-primary; `background` serves labeled.

### PE-8 — Freshness schema: handle-diff primary, decay-class fallback, structural overrides

- **Version handles (primary).** Where an item cites a source that has a handle, the envelope stores the handle at index time and reports `current`, `moved`, or `unknown` by diffing against the source's live handle. The closed handle-kind set at V0: `git-sha` for canon and repo citations (the live handle is the repo's current state), `semver` for dependency and tool versions, `model-id` for model revisions (diffed against the configured pinned revision), and `doc-version` for explicit version strings on versioned documents. An unreachable live handle reports `unknown`, never a fabricated state. In a zero-egress configuration, external live-handle checks resolve to `unknown` by construction. Adding a handle kind is a P-0015 amendment declaring its differ, validator before field.
- **Decay classes (fallback; these bound ignorance, they don't measure staleness).** Handle-less sources carry a volatility-matched class. TTL (time-to-live) expiry without re-validation makes the item freshness-stale, feeding PE-7's predicate (b). The V0 class table:

  | Class | TTL | Domain examples |
  |---|---|---|
  | `volatile` | 30 d | vendor/dependency landscape, pricing, model availability |
  | `moderate` | 90 d | comparative research, tool evaluations |
  | `slow` | 180 d | project conventions, operational skills |
  | `stable` | 365 d | principles, foundational decisions, values |

  Time is the weakest signal here. It's a fallback, never primary. TTL values are class defaults and can be amended per class; the class assignment itself is a stored field.
- **Overrides are structural.** A freshness override is a recorded row `{by, reason, date}`, accepted only through the PE-6 write gate. It's never a config toggle. An override restores the prior trust value and is itself visible in the envelope. Content-based staleness detection was rejected as circular reasoning (the maintainer's advance 1, 2026-05-21).

### PE-9 — Displacement ≠ staleness: the event registry

Handle-diff catches "my cited source changed," not "the world moved past it." This is containment, not a full solution: a named displacement-event registry (spec Data Model `displacement_events`) whose fired events set a `re-eval pending (axis)` envelope flag on decisions whose recorded axes intersect the event. The V0 event-kind vocabulary is closed and enumerated (r1 fold: an enumerable registry with zero enumerated kinds would have nothing to build a test fixture against). Extending it is a P-0015 amendment declaring the kind's mechanical firing condition, validator-first, following the mechanic-6 discipline:

| Event kind | Fires (mechanically) |
|---|---|
| `canon-superseded` | A `supersedes` edge of origin `declared`/`system` lands on a canon-kind artifact (ADR, principle, spec) — detected on the edge-write path this cluster builds |
| `dependency-major-change` | A `semver` version-handle diff (PE-8) crosses a major version boundary — detected by the freshness handle-diff this cluster builds |
| `operator-declared` | The operator records an event through the PE-6 write gate, naming the axis + description — the catch-all for world-shifts no detector covers |

Single-axis displacement triggers re-evaluation, never auto-invalidation. The trust value doesn't flip just because a displacement flag fired; multi-axis decisions survive a single-axis shift. Decision-kind artifacts record their decision axes as substrate data.

Here's the honest limit, carried verbatim: trip-wires only cover enumerable displacement events. Truly unforeseen shifts fall back entirely to the volatility TTL. *(Anchor: the maintainer's advance 2, 2026-05-21; P-PreserveDecisionSpace: every ADR keeps its rejected alternatives on record so the option space stays visible to future readers.)*

### PE-10 — Citation form

Every envelope item carries a stable citation: an artifact ID (content-addressed, per the V0 substrate) plus a block anchor when the item is a chunk, resolvable by `get_artifact_by_citation`. The ID-and-name pairing plus block addressability are what make citation-expansion retrieval work. *(Anchor: P-AgentPrimarySource.)*

### PE-11 — Residuals owned here

- **Labeling correctness and the adopter-facing polarity revisit (Frame residual (i), maintainer-accepted for V0).** Labeling is opt-in at V0: an unlabeled artifact is egressable at index time, mitigated by the index-admission gate, while the serving-time posture stays fail-closed. This is rendered structurally as the PE-2 not-set DDL defaults: unlabeled means the permissive default values get stored. So the fail-closed rule (mechanic 3) targets only genuinely undecidable values and can never silently withhold the unlabeled corpus. This ADR owns the revisit question: should the default polarity flip (unlabeled implies egress-deny, mechanically done by flipping the `model_egress` DDL default for new rows) for adopter deployments? Firing: self-announcing, at the first deployment beyond the maintainer's own dogfood instance (an adopter standing up an instance is an explicit act), or at register `1.2.0` ingest, whichever comes first.
- **Query-time egress content-audit (Stage-2 L5).** Which query text (4.2) and which retrieved chunk citations (4.3) egressed, keyed for incident response. This landed as spec R-0034-b, and is recorded here as part of the envelope contract's enforcement observability.

## Consequences

**Good:**
- The hard-to-change parts (record shape, enforcement discipline, port boundary, fail-closed rule, write attribution) are locked, while every label stays a cheap ADR amendment. That honors the cost asymmetry the walk ratified.
- Engine substitution is a bounded change (one PDP module plus one fragment module), proven by a conformance suite that exists from V0 (QA-9).
- There's no existence oracle anywhere on the caller surface. Every withhold is operator-observable.
- Adversarial corpus text can't flip trust (PE-7's origin weighting) and can't reach served payloads through control point 4.1 (P-0014 RA-9).

**Bad / Trade-offs:**
- `owner-only` over-blocks at V0: it serves no one, not even the owner. That's the deliberate price of mechanic 3 until identity lands; the identity feature's intake is the named point where this reopens.
- The differential conformance test has to be maintained as dimensions evolve. That's accepted over a fragment generator at V0 scale; a failing equivalence run is itself the instrument that would catch drift.
- Kinds outside the PE-7 mapping under-claim authority by resolving to `background`. Fail-safe, but it's a ranking cost until the table gets extended on evidence.
- Freshness `unknown` is common in zero-egress deployments, since external handles are unreachable there. That's honest by design; TTL still bounds the ignorance.

## Pros and Cons of the Options

### Two-axis record + port, hardcoded predicates (chosen)

- Pro: carries independent facts without losing information; per-axis determinism is retained.
- Pro: enforcement is structural, fail-closed, and single-boundary, and it's simple enough for V0 (four predicates).
- Con: two axes are more schema than one enum. That cost is paid once, at the layer that's hardest to retrofit later.

### Round-1 single five-role value (replaced)

- Pro: one field, simple precedence.
- Con: destroys information, since sensitive-and-authoritative or dont-use-and-background are both real combinations, and precedence ends up fighting itself. Ratified out at the pre-gate walk; its five validators survive as predicates in the new design.

### Cedar-class engine at V0 (rejected for V0)

- Pro: expressive policy language, forward-ready.
- Con: mechanism ahead of evidence, for what's currently four hardcoded predicates. The port makes later adoption a substitution rather than a rewrite (QA-9 is the proof obligation).

### Config-knob dimensions (rejected)

- Pro: no ADR churn needed to add a label.
- Con: violates mechanic 6. Config can't declare a validator, an enforcement point, a fail-closed pole, or a governed operation, so a knob-created "dimension" is unenforced by construction.

## More Information

- Binding requirement text: [spec R-0025/R-0026](../../specs/2026-07-02-retrieval-cluster.md). Observable measures QA-4, QA-7, and QA-9 are defined in [Frame §7](../../intent/retrieval-cluster-frame.md).
- Companions: [P-0014](P-0014-retrieval-architecture.md), which covers the query path and the gates that consume `decide()` (the 4.1 control RA-9 is co-owned with this contract), and [P-0016](P-0016-edge-schema.md), which owns the edge `origin` column that PE-7 weights and the extraction-integrity contract.
- Ratification trail: the pre-gate maintainer walk and frame-exit gate outcomes (Frame §2, items 1 through 15). The eight mechanics are carried verbatim from there; `admin-only` was added at the gate; the V0 write invariant was confirmed at the gate.
- Sources by name and lock-date: the sources-with-roles finding plus freshness and displacement advances (maintainer record, 2026-05-21); the Cedar/NGAC permission-model research (maintainer record, locked 2026-05-18); the knowledge-object survey G2/G3 (2026-05-15).
- Substrate: [P-0006](P-0006-v0-tenant-enforcement.md) and [P-0009](P-0009-rls-admin-token.md), which supply the caller context and the admin-gated mutation path that V0 enforcement rides on.
