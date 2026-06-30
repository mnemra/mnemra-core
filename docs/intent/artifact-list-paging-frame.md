---
title: "Frame: artifact-list cursor (keyset) pagination"
summary: "Brownfield-extension Frame for adding keyset (cursor) pagination to the artifact-list host-fn import and the guest content.list export — an in-place amendment to the locked mnemra-core V0 substrate spec. Renders the maintainer-locked design as canon-anchored architectural directions, ATAM quality-attribute scenarios, and the open R-ID slots Stage 3 will lock."
primary-audience: agent
modulation: brownfield-extension
intake: docs/intent/artifact-list-paging.md
amends-spec: docs/specs/2026-05-24-mnemra-core-v0-substrate.md
amends-spec-version: 9c617c5bb858b924f75f3b3d96c6f3398ea6c41a
amends-spec-type: architecture
---

# Frame — artifact-list cursor (keyset) pagination

**Date:** 2026-06-29 · **Status:** finalized — Frame-exit gate passed 2026-06-29 (Approve-with-conditions; security-mode review by the reviewer, accepted by the maintainer; conditions A/B/C/D/F folded) · **Altitude:** interface amendment

> **Modulation note.** This is a `brownfield-extension` Frame. An established product Frame
> (`docs/src/intent/mnemra-core-frame.md`) and a locked, verified architecture spec already
> govern mnemra-core; Stage 2a elicitation collapses because the design was locked with the
> maintainer before this run. This Frame is **not** a restatement of the 670-line product
> Frame — it is the architectural shape of the **pagination delta only**, the closed world
> the Spec-stage amendment draws from. It refines the locked spec; it does not contradict it.
> Where a tension would surface between this amendment and the locked spec, this body records
> it explicitly rather than silently choosing. (None surfaced — see §2.)

## 1. Purpose / context

Artifact types in mnemra-core are append-only — they grow without bound and never shrink.
The `artifact-list` read path today issues an **unbounded** query and returns the entire
result set of a type in one response. This Frame designs the addition of **keyset (cursor)
pagination** so a single `list` call returns a bounded, resumable page regardless of how
many artifacts a workspace has accumulated, and a caller can walk the full set page by page.

- **What this designs.** The interface-shape change to the `artifact-list` host-fn (import
  direction) and the guest `content.%list` export (export direction), and the pagination
  *behavior* contract both must satisfy, so the MCP client paginates **end-to-end**
  (client → guest `%list` → host-fn `artifact-list` → Postgres).
- **Intake.** [`docs/intent/artifact-list-paging.md`](artifact-list-paging.md) — locked,
  high-stakes, `spec_type: architecture`. This Frame transcribes the maintainer-locked
  design into canon-anchored directions; it does not re-derive it.
- **Spec being amended.** [`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`](../specs/2026-05-24-mnemra-core-v0-substrate.md)
  (`spec_type: architecture`), at blob SHA `9c617c5`. The amendment is **in-place**: it
  amends existing requirements (R-0012-a/-f, R-0019-a) and adds one new requirement slot
  (≈ R-0020). It does not fork a new spec.
- **Interfaces touched.** [`wit/host.wit`](../../wit/host.wit) — `interface artifact`
  host-fn `artifact-list` (≈ line 103); `interface content` guest export `%list` (≈ line 59).

This Frame is the closed world for the Spec amendment: nothing outside the directions
locked here enters the Spec.

## 2. Constraint-graph walk

Walking `brain/about/constraint-edges.md` from the intent (bounded, resumable reads over an
append-only, multi-tenant store, amending a locked ABI). Traversal rule (G-0013): most-specific
applies when no conflict — per-task > project ADR > workspace ADR > principle. The
most-specific layer here is the mnemra-core project ADRs (P-0006, P-0013, P-0001), which
apply over the workspace principles they specialize.

### Keystone edge — why amending a *locked* contract is legitimate, not a violation

**`P-LockContract` ⇄ `P-PreserveDecisionSpace` — the *when-to-lock* edge** (`constraint-edges.md:140`,
worked instance `:152`), specifically its **re-derive-on-reshape** half:

> "A lock is scoped to the assumptions it was made against; when a later change falsifies
> that world, the contract is re-derived against the new world, not honored as if the world
> had not moved." (`constraint-edges.md:140`; mirrored in `P-LockContract` Anti-example:
> "Honoring a locked freeze literally after a later reshape has falsified the world the
> freeze assumed.")

The original `artifact-list` / `content.%list` contract (`-> list<string>`, no paging
params) was locked under an implicit "no realistic unbounded-growth pressure on the list
path" assumption. The intake's **measured** append-only-growth evidence — ~2,630 artifacts
in three months of single-user work, ~10,500/year extrapolated, the largest type
(`tasks`) append-only and never pruned, multiplied per-workspace across tenants —
**falsifies that world**. The contract is therefore **re-derived** against the new world
(bounded, resumable pages with a hard cap), not honored as frozen. This is the same shape
as the storage-substrate re-derivation recorded at `constraint-edges.md:152` (a prior
no-swap lock reversed once an on-merits re-evaluation falsified its assumption). This edge
is the load-bearing anchor for the whole amendment — it pre-empts the "you are breaking a
locked contract" objection: the lock's scope ended where its assumption failed.

### Other edges bearing on the locked decisions

| Edge (type) | Bears on | How it applies |
|---|---|---|
| `P-SecurityLayered → Security` (specializes, `:42`) | The DoS bound (two runtime sub-layers) + tenant-isolation preservation | Each layer independently load-bearing — and within the runtime layer, two distinct sub-layers each load-bearing (the `:105` best-effort/structural/backstop gradient). The hard cap (D5) bounds result-set **size** — it closes the unbounded-`SELECT` *result-set-size* DoS; the host-side `statement_timeout` (D8) bounds query **scan cost** — the deterministic runtime backstop closing the *scan-cost* surface the cap alone leaves open (a `LIMIT` bounds rows returned, never rows scanned). The `workspace_id` WHERE-clause is the **application-layer** tenant enforcement (P-0006) that must survive the keyset rewrite. |
| `P-MinBlastRadius → Maintainability, Reversibility` (specializes, `:65–66`) | The breaking return-type change (`list<string>` → record) | The change ripples to every `core: true` content plugin (the typed export consumers). The **seam that bounds the ripple** is R-0017-a's recompile-all-core-plugins-before-merge discipline — a sequenced landing, not a zero-touch change. (See §3 D6 and §4 QA-4.) |
| `P-Defer → Simplicity, Reversibility` (specializes, `:35–36`) | The Non-goals: no RLS policy activation, no schema/index change, no offset path | The keyset mechanism reuses the existing ULID-text PK index — no new mechanism is adopted speculatively. RLS policy activation stays deferred behind its existing P-0006 `R-0001` trip-wire; this amendment does not fire it. |
| `G-0003 (merge governance) → P-ShiftLeft` (specializes, `:79`) | The security-mode review trigger this Frame carries forward | Substance is reviewed shift-left on this artifact; a mandatory pre-push security review (Warden, security mode) gates the merge per the G-0003 2026-06-25 amendment. §6 makes the security-relevant mechanism explicit so that review can run. |
| Project ADR `P-0006-v0-tenant-enforcement` (specializes `P-SecurityLayered`) | R-0006-d co-constraint | Most-specific layer. The keyset SELECT satisfies P-0006's WHERE-clause-mandatory read-path discipline; **R-0006-d's requirement text does not change.** |
| Project ADR `P-0013-plugin-invocation-model` (specializes `P-StackDiscipline`) | R-0019-a export typing | Most-specific layer. The typed `content` export gains paging params + record return; the symmetric-typing-both-directions invariant (R-0019-b) holds — paging params are explicit typed WIT params, not a string/JSON dispatch hatch. |

### Conflicts-with finding

**None surfaced.** The one candidate tension — `Security ⇄ Simplicity` (`constraint-edges.md:133`,
"each layer load-bearing" vs "smallest mechanism") — does **not** fire here. The DoS bound is
two runtime sub-layers (D5 cap + D8 `statement_timeout`), and **both** are simultaneously the
Security default **and** a minimal mechanism: the cap is a `LIMIT` clamp (no `COUNT`, no
offset-window state); the scan-cost backstop is a one-line session GUC (`SET LOCAL
statement_timeout` on the read path — no schema, index, filter-semantics, or RLS touch). Each
serves both values; it is mutual reinforcement, not a trade-off to escalate. No `conflicts-with`
edge requires maintainer escalation. *(The scan-cost backstop was surfaced by the security-mode
review and is the maintainer's chosen finding-remediation — D8; it is additive defense-in-depth,
not a re-derivation of any locked direction.)*

## 3. Locked architectural directions

Each direction is the maintainer-locked decision, rendered with its canon citation. These are
**not** re-opened — if a reader believes one is wrong, that is a halt-and-escalate to Puck/PA,
not a Frame-internal pivot.

### D1 — Both interfaces gain pagination; the client paginates end-to-end

The host-fn import `artifact-list` **and** the guest export `content.%list` both gain the
paging parameters and the record return, so the MCP client paginates
client → guest `%list` → host-fn `artifact-list` → Postgres.

- **Anchor:** R-0012-a / R-0012-f (host-fn ABI, import side — the governing requirement
  anchor) + `wit/host.wit` `interface artifact` `artifact-list`; R-0019-a (typed `content`
  export, export side) + `wit/host.wit` `interface content` `%list`. Intake Success
  criteria 1 + 2. Workspace anchor: the `P-LockContract` re-derive-on-reshape keystone (§2).
- **Note (anchoring correction — the reason this run exists):** the new behavior anchors to
  **R-0012-a/-f + R-0019-a**, *not* to R-0006-d. R-0006-d is a preserved co-constraint
  (D7), not the home of the new behavior.

### D2 — Mechanism: keyset (cursor), fetch-one-extra-row

`SELECT id FROM <content-store> WHERE workspace_id = $ctx AND type = $type AND <filters> AND id > $cursor ORDER BY id LIMIT $page + 1`
— fetch **one extra row** to derive `has-more` without a `COUNT`. Order is by `id`
(ULID-as-text PK), lexicographic = chronological, so pages are returned in stable creation
order. The keyset predicate `id > $cursor` **ANDs with** the existing `filters: json`
predicate — pagination composes with filtering, it does not replace it. When `has-more`
is true, **`next-cursor` is the `id` of the last row in the returned page** — the value the
next call passes as `cursor` to resume (`id > next-cursor`). This is intrinsic to the keyset
mechanism (the cursor *is* the last-seen key), not a separate decision.

- **Anchor:** Intake Hard constraint (keyset/cursor, fetch-one-extra) + Success criterion 5.
  Workspace anchor: Simplicity (smallest sufficient mechanism — no `COUNT`, no offset state);
  `P-Defer` (reuses the existing ULID-text PK index — no speculative schema/index mechanism).
- **Schema scrutiny: settled (no flag).** `<content-store>.id` is ULID-as-text, single-column
  PK, fixed-width 26-char, PK-indexed (verified by the orchestrator at intake:
  `component.rs:194`, `content_schema.rs:234,269`). Lexicographic = chronological → keyset is
  valid **and** creation-ordered. No schema change required (intake Non-goal). The illustrative
  SELECT above uses the echo fixture's table as the concrete read path; the real read path is
  the content store under P-0001's single-document layout. *(Illustrative SQL — not normative
  as code.)*

### D3 — WIT in: explicit typed paging parameters

`artifact-list(ctx, %type, filters, limit: u32, cursor: option<string>)` — `limit` and
`cursor` are **explicit typed WIT parameters**, NOT folded into the `filters: json` blob.
The guest export gains the same two params **without** a `ctx` param (the host carries `ctx`
across the export boundary itself — `wit/host.wit:30–33`, R-0019-a):
`content.%list(%type, filters, limit: u32, cursor: option<string>)`.

- **Anchor:** R-0012-f (all host-fn parameters SHALL be WIT component types — no raw byte
  buffers, no dynamic type dispatch); R-0019-b (symmetric typing both directions; no
  string/JSON *dispatch* path — paging params are typed, not a string hatch). P-0013 export
  typing discipline.

*Illustrative WIT (not normative as code):*

```wit
// in interface types — shared by both directions
record artifact-page {
    ids: list<string>,
    has-more: bool,
    next-cursor: option<string>,
}

// interface artifact (host-fn import) — gains limit + cursor, returns the record
artifact-list: func(ctx: workspace-ctx, %type: string, filters: json,
                    limit: u32, cursor: option<string>) -> artifact-page;

// interface content (guest export) — same paging params, NO ctx, same record
%list: func(%type: string, filters: json,
            limit: u32, cursor: option<string>) -> artifact-page;
```

### D4 — WIT out: page record + the single invariant

Return type changes from `list<string>` to
`record { ids: list<string>, has-more: bool, next-cursor: option<string> }`.

> **Invariant (intrinsic — locks at V0): `has-more = true` if and only if `next-cursor = some`.**

Per `P-LockContract` ("an invariant intrinsic to what the artifact *is* locks at the stage it
is defined"), this biconditional is intrinsic to the page contract and locks now. It is the
spine of the correctness scenario (§4 QA-1) and is binary-observable on every response.

- **Anchor:** R-0012-f (return types are WIT component types); intake Success criterion 3 +
  Hard constraint (interface shape out).

### D5 — Page size: exact `default = 100`, hard cap `= 500` (clamp); the cap is the result-set-*size* DoS bound

Default page size **exactly 100**; hard cap **exactly 500**. A `limit` above the cap is
**clamped** to the cap. The cap bounds the returned result-set **size** and therefore doubles
as the **result-set-size** DoS bound, closing the *size* half of the prior unbounded-`SELECT`
finding. It does **not** bound query **scan cost** — a `LIMIT` bounds rows *returned*, never
rows *scanned*; the scan-cost half is closed by the host-side `statement_timeout` backstop
(**D8**), an additive runtime sub-layer.

- **Clamp site (host-side — Condition B, locked):** the cap clamp is enforced in the **host-fn
  `artifact-list`** — the SQL `LIMIT` owner and the **sole DB chokepoint** — independent of any
  guest-supplied `limit`. Rationale: guests execute inside the Wasmtime component-model sandbox
  (R-0002-a/-b) and have no ambient IO; the host-fn ABI (R-0012-a, which declares `artifact.list`
  as the list operation) is the only path a guest reaches the DB. The host-fn is therefore the
  **necessary-and-sufficient** clamp site, and both interfaces (host import + guest `%list`
  export) funnel through it. A guest-side clamp is **optional** defense-in-depth only, never the
  enforcing layer.
- **Anchor:** Intake Success criterion 4 + Hard constraint + Evidence (prior unbounded-result-set
  DoS finding). Workspace anchor: `P-SecurityLayered` (Security, the change-time/runtime layer);
  Security's "default-on, never opt-in" — the bound is structural, not opt-in.
- **Sizes ratified at the Frame-exit gate (2026-06-29):** the intake's "default ≈ 100; hard cap
  ≈ 500" `~` wording was not binary-observable; this Frame's pin to exact `default = 100` /
  `cap = 500` was **ratified by the maintainer at the gate**. These are now the locked,
  binary-observable values — no longer a provisional reading.

### D6 — Breaking ABI change: stability annotation + recompile-all-core-plugins

The return-type change (`list<string>` → record) is a **breaking** ABI change on both
directions. Under pre-1.0 SemVer (R-0017's `0.y.z` freedom) breaking changes are permitted
within the band, but the discipline is mandatory.

- **Co-anchors:** **R-0012-e** — the changed host-fns SHALL carry the correct
  `@stable`/`@unstable` stability annotation reflecting the change (the exact annotation
  mechanics — version bump vs mark — are a Spec/plan concern). **R-0017-a** — the ABI-change
  PR SHALL cause **all `core: true` plugins to recompile against the new ABI and pass their
  tests before merge.**
- **Workspace anchor:** `P-MinBlastRadius` — the blast radius is **not zero**; the return-type
  change propagates to every `core: true` content plugin (the typed `content` export
  consumers). R-0017-a's recompile-and-green-before-merge **is the seam that bounds the
  ripple** — a sequenced landing (change the ABI, recompile consumers, green before merge),
  not a lock-step N-module edit and not a zero-touch claim.

### D7 — Tenant isolation preserved (co-constraint; text unchanged)

The keyset SELECT **retains** `workspace_id = $ctx` as a WHERE-clause condition (not a
post-read filter), exactly as today. The keyset rewrite must not weaken tenant isolation.

- **Anchor (preserved co-constraint, NOT the behavior's home):** **R-0006-d** — "All
  read-path host-fns SHALL include `workspace_id = ctx.workspace_id` as a WHERE-clause
  condition … a CI lint check SHALL assert this on all read paths." R-0006-d's **requirement
  text does not change**; the keyset SELECT simply continues to satisfy it. P-0006 / P-SecurityLayered.
- This direction is cited so the Spec records tenant isolation as a *preserved* invariant of
  the rewrite, and so QA-3 can assert it — it is **not** the anchor for the new pagination
  behavior (that is D1).

### D8 — Scan-cost backstop: host-side `statement_timeout` (new direction; security-review remediation)

The cap (D5) bounds result-set **size**, not query **scan cost**. A `LIMIT` bounds rows
*returned*, never rows *scanned*. Under a `filters` predicate on a **non-indexed** frontmatter
field — anything outside the four expression-indexed fields `status` / `priority` /
`project_id` / `parent_id` (**R-0001-d**) — the keyset `ORDER BY id LIMIT cap+1` walks the `id`
btree applying the filter row-by-row, scanning the **entire tail** when matches are sparse or
zero. The substrate has no `statement_timeout` today, and the Wasmtime epoch deadline
(**R-0007-b**, 5s) bounds *guest wasm execution*, **not** the host's Postgres query — so the
adversarial scan path is unbounded with the cap alone.

The locked remediation (maintainer's gate choice — the `statement_timeout` option, **not** the
"restrict filters to indexed fields" alternative): the host read path SHALL set a host-side
**`statement_timeout`** (a Postgres session GUC, e.g. `SET LOCAL statement_timeout` on the
read-path transaction) so a query that would scan unbounded is **canceled** within a bounded
wall-clock budget. The exact timeout value is a Spec/plan concern (R-0020); the *direction* —
a host-side query-time backstop on the read path — locks here.

- **Anchor:** `P-SecurityLayered` — the **runtime** layer, specifically the deterministic
  **backstop** sub-layer of the `:105` best-effort/structural/backstop gradient. It is
  **additive** to the D5 cap (each runtime sub-layer independently load-bearing: cap = size
  bound, timeout = scan-cost bound), not a replacement. The cancellation outcome is observable
  on the **R-0004-a** metric `outcome` enum as `"timeout"`.
- **Clears every intake Non-goal:** a session GUC touches **no schema**, **no index** (intake
  Non-goal: no index migration), **no filter semantics** (Non-goal: filters unchanged), and
  **no RLS** (Non-goal: no RLS activation). It is purely a runtime/defense-in-depth addition —
  which is why it is the chosen remediation over altering the filter/index surface.

### D9 — Malformed cursor → validate-at-boundary, return the structured "parameter invalid" error (locked; was an open R-0020 slot)

The `cursor` is validated as a well-formed **26-char ULID at the host-fn `artifact-list`
boundary, before query construction**. This both fixes the error semantics and **caps the
otherwise-unbounded `option<string>` input** (an arbitrary-length client string never reaches
query construction). On a malformed cursor the host-fn returns the **R-0010-f** structured
**"parameter invalid"** error — **not** a silently-empty page, **not** a raw Postgres error
echoed to the caller. A **valid-but-out-of-range** cursor (well-formed ULID, past the end)
returns an **empty page** (mechanically derivable from keyset `id > $cursor`); a
**foreign-but-valid** ULID is safe — confined to the caller's workspace by `workspace_id = $ctx`
(QA-3).

- **Anchor (two distinct pieces, kept separate):** **R-0010-f** supplies the structured
  **error class** — "The MCP handler SHALL return distinguishable JSON-RPC error codes for …
  parameter invalid; error code classes SHALL NOT be conflated." The **"no DB/schema detail
  leaked"** posture is an **additive** security constraint locked *here* (it is not literal
  R-0010-f text): the structured error SHALL carry no Postgres/schema internals. Layer note:
  the *validation* runs at the host-fn boundary (D5/Condition B layer); the *JSON-RPC error
  code* is returned by the MCP handler up-stack (R-0010-f layer) — linked, distinct layers.
- This moves the malformed-cursor semantics from the §5 open slot to a **locked** direction. The
  injection-safe / tenant-scoped posture (cursor is a bound parameter ANDed with `workspace_id`)
  was confirmed sound by the security-mode review and is unchanged (§6.3).

### D10 — `limit = 0` ≠ unlimited (locked; was an open R-0020 slot)

`limit = 0` is locked to **clamp-up to the default** (the recorded lean — every call returns
progress) **or** an empty page. The **binding constraint**, independent of which of those two
the Spec pins: `0` MUST **never** disable the `LIMIT` — e.g. the read path SHALL NOT emit
`LIMIT NULLIF($n, 0)` or any construction where `0` yields an unbounded query. A zero `limit`
is a bounded outcome, never an unbounded-result-set escape hatch.

- **Anchor:** `P-SecurityLayered` (the bound is structural / default-on — the unbounded-result
  surface stays closed for every `limit` value, including `0`); intake Success criterion 6 ("no
  list path issues an unbounded query"). The choice between clamp-up-to-default and empty page is
  the only residual; the no-unbounded invariant is locked. This moves `limit = 0` from the §5
  open slot to a **locked** direction.

### D11 — Paging params excluded from metric / structured-log emission (locked)

`cursor` (in) and `next-cursor` (out) **are artifact IDs**. **R-0004-a** forbids artifact IDs
in the per-verb metric record ("no artifact IDs, content fragments, or agent identity SHALL
appear in the metric record"). The paging parameters SHALL therefore be **excluded** from the
per-verb metric record **and** from structured-log emission — the verb's metric/log carries
`workspace_id` / `verb` / `outcome` / `duration_ms` (R-0004-a floor) but never the `cursor` /
`next-cursor` values.

- **Anchor:** **R-0004-a** (artifact-ID exclusion from the metric record), extended to the
  structured-log surface by the same data-minimization rationale. Telemetry data-minimization,
  not a behavioral change — observable by inspecting the emitted record (QA-6).

## 4. Quality-attribute scenarios (ATAM)

Each scenario is `[stimulus · environment · response · measure]`, the unit of architectural
decision. Each **measure is a conjunction of binary checks**, not a single assertion — a lone
`has-more ⇔ next-cursor` iff does not catch a `next-cursor` wrongly set on the last page.

### QA-1 — Pagination correctness (end-to-end, both interfaces)

- **Stimulus:** an MCP client walks a type with N artifacts from `cursor = none`, following
  `next-cursor` each call, until `has-more = false`.
- **Environment:** the full path client → guest `content.%list` → host-fn `artifact-list` →
  Postgres; a workspace with N > one page of artifacts of the target type.
- **Response:** a finite sequence of pages covering the full set in `id`-ascending order.
- **Measure (all must hold):**
  1. On **every** page: `has-more = true` **iff** `next-cursor = some` (D4 invariant).
  2. On the **final** page: `has-more = false` **and** `next-cursor = none` (the last-page
     trap — `next-cursor` MUST be `none` when `has-more = false`).
  3. The concatenation of all pages' `ids` contains **each** of the N artifacts **exactly
     once** — no duplicates, no gaps.
  4. Global order is **`id`-ascending** (= creation order) across page boundaries.
  5. The walk **terminates** (no infinite cursor loop).
  6. The walk is exercised through the **guest `%list` export**, not only the host-fn — the
     end-to-end path is the asserted surface (the mis-anchor's lesson).

### QA-2 — DoS bound: result-set size (cap) **and** scan cost (`statement_timeout`)

Two stimuli, two runtime sub-layers (D5 cap + D8 backstop) — each independently load-bearing.

- **Stimulus A (size — the cap):** a caller requests `limit` greater than the hard cap
  (e.g. `limit = 100_000`) against a type with more than `cap` artifacts.
- **Environment:** the keyset read path with the host-side clamp active (D5 / Condition B).
- **Response:** a single bounded page; the request cannot drain an unbounded result set.
- **Stimulus B (scan cost — the backstop, adversarial):** a caller issues a `filters` predicate
  on a **non-indexed** frontmatter field (outside the four R-0001-d expression-indexed fields)
  that matches **sparsely or zero** rows, against a type holding **more than `cap`** artifacts —
  forcing the keyset `ORDER BY id LIMIT cap+1` to walk the entire `id`-btree tail.
- **Environment:** the keyset read path with the host-side `statement_timeout` set (D8).
- **Response:** the unbounded tail-scan is **canceled** within the bounded timeout budget; the
  request cannot pin the DB on a full-tail walk.
- **Measure (all must hold):**
  1. **(A)** `ids.len() <= 500` (the cap) **even though** the requested `limit` exceeded it (clamp).
  2. **(A)** Rows **returned** are `<= cap + 1` (fetch-one-extra; no `COUNT`). *(This bounds rows
     returned only — a `LIMIT` does not bound rows **scanned**; the scan bound is measure 4.)*
  3. **(A)** No list path issues an unbounded (`LIMIT`-less) query — asserted by test (intake
     Success criterion 6); closes the *result-set-size* half of the prior unbounded-`SELECT`
     finding (intake Evidence).
  4. **(B, adversarial)** Under Stimulus B the query is **canceled by the `statement_timeout`**
     within the bounded budget and surfaces as a structured error — observable as the **R-0004-a
     metric `outcome = "timeout"`** (not `"ok"`) and a bounded wall-clock `duration_ms` — rather
     than completing an unbounded full-tail scan. Closes the *scan-cost* half the cap leaves open.

### QA-3 — Tenant-isolation preservation across the rewrite

- **Stimulus:** a paginated `list` is issued under workspace A while workspace B holds
  artifacts of the same type.
- **Environment:** the keyset SELECT with the `workspace_id = $ctx` WHERE-clause retained
  (D7); the existing R-0006-d read-path lint in CI.
- **Response:** only workspace-A artifacts are ever returned, on any page, for any cursor.
- **Measure (all must hold):**
  1. The existing **R-0006-d read-path CI lint stays green** across the rewrite (the
     `workspace_id` WHERE-clause condition is present on the new keyset read path) — the
     existing mechanism is preserved, no new mechanism is introduced.
  2. A **cross-workspace probe** (paginate under A; assert no B-owned `id` appears on any
     page, including under a cursor that B's rows would sort into) returns **zero** foreign rows.

### QA-4 — ABI evolution (breaking return-type change, pre-1.0)

- **Stimulus:** the return type of `artifact-list` / `content.%list` changes from
  `list<string>` to the page record — a breaking change within the `0.y.z` band.
- **Environment:** the pre-merge gate; all `core: true` content plugins in the tree.
- **Response:** the breaking change lands only with all consumers migrated and green.
- **Measure (all must hold):**
  1. **R-0017-a:** every `core: true` plugin recompiles against the new ABI **and** passes
     its tests **before** the change merges (asserted at the merge gate, not after).
  2. **R-0012-e:** the changed host-fns carry the correct stability annotation reflecting the
     breaking change.
  3. The trap/replace plugin path (pinned to the current export shape per P-0013 Consequences)
     is re-pinned to the new return shape and its kill-and-replace tests stay green.

### QA-5 — Input validation & boundary semantics (cursor + `limit = 0`)

- **Stimulus:** a caller supplies (a) a malformed `cursor` (not a 26-char ULID), (b) a
  well-formed but out-of-range `cursor` (valid ULID past the end), and (c) `limit = 0`.
- **Environment:** the host-fn `artifact-list` boundary validation (D9) ahead of query
  construction; the host-side clamp (D5).
- **Response:** each boundary input yields a **bounded, defined** outcome — never an unbounded
  query and never a leaked internal error.
- **Measure (all must hold):**
  1. **(a)** A malformed `cursor` returns the **R-0010-f structured "parameter invalid" error**
     (not an empty page, not a raw Postgres error), and the error body contains **no
     DB/schema/Postgres internals** (D9 no-leak posture).
  2. **(b)** An out-of-range valid-ULID `cursor` returns an **empty page** (`ids = []`,
     `has-more = false`, `next-cursor = none`) — derivable from keyset `id > $cursor`.
  3. **(c)** `limit = 0` returns a **bounded** result (default-clamped page or empty page per the
     Spec's R-0020 pin) and **never** issues an unbounded (`LIMIT`-less or `LIMIT NULLIF`-zeroed)
     query (D10). Asserted against the actual emitted SQL.

### QA-6 — Telemetry data-minimization (paging params excluded)

- **Stimulus:** any paginated `artifact-list` / `content.%list` dispatch carrying a `cursor` in
  and a `next-cursor` out.
- **Environment:** the per-verb metric record (R-0004-a) and structured-log emission on the
  dispatch.
- **Response:** the telemetry records the R-0004-a floor without the paging-param artifact IDs.
- **Measure (all must hold):**
  1. The emitted per-verb metric record contains `workspace_id` / `verb` / `outcome` /
     `duration_ms` and **does not** contain the `cursor` or `next-cursor` value (R-0004-a:
     no artifact IDs in the metric record) — D11.
  2. The structured-log line for the dispatch likewise **omits** the `cursor` / `next-cursor`
     values — assertable by inspecting the emitted record/log.

## 5. Open ADR / R-ID slots

What Stage 3 (Spec amendment) will lock. Names the slots; assigns no IDs beyond the confirmed-next R-0020.

### New requirement — pagination behavior

| Slot | Locks | Status |
|---|---|---|
| **R-0020** (confirmed next — spec runs R-0001..R-0019; R-0020 is unused) | The pagination **behavior** contract: the `(limit, cursor) → page` shape; the `has-more ⇔ next-cursor=some` invariant (D4); `next-cursor` = `id` of the last returned row (D2); the exact `default=100 / cap=500` clamp, enforced host-side in `artifact-list` (D5 / Condition B); the keyset mechanism + fetch-one-extra + `workspace_id`-WHERE retention + filter composition (D2); the "no list path issues an unbounded query" assertion (intake SC 6); the host-side `statement_timeout` scan-cost backstop (D8); cursor ULID-validation-at-boundary → R-0010-f structured "parameter invalid" error, no leak (D9); `limit = 0` ≠ unlimited (D10); paging-params excluded from metric/log emission (D11). The pagination behavior has **no existing home** — this is its slot. The actual id is assigned when the Spec is authored at Stage 3. **Spec residuals (boundary semantics now locked by direction; only these two values remain to pin):** (i) the exact `statement_timeout` value (D8), and (ii) the `limit = 0` resolution — clamp-up-to-default vs empty page (D10, the no-unbounded invariant is already locked). | **Open — Stage 3 assigns + locks the requirement; boundary semantics locked by D8–D11** |

### Amended-in-place requirements (no new slot — existing R-IDs change)

| R-ID | Amendment |
|---|---|
| **R-0012-a / R-0012-f** | Host-fn `artifact-list` signature: add `limit: u32`, `cursor: option<string>`; return type → `artifact-page` record. Import direction. |
| **R-0019-a** | Guest `content.%list` export signature: add `limit: u32`, `cursor: option<string>` (no `ctx`); return type → `artifact-page` record. Export direction. |
| **R-0012-e** | The changed fns carry the correct stability annotation reflecting the breaking change. |
| **R-0017-a** | The ABI-change discipline (recompile-all-`core:true`-plugins + green before merge) fires for this change. Requirement text unchanged; this amendment is an instance that triggers it. |
| **R-0006-d** | **Preserved, NOT amended** — requirement text unchanged; the keyset SELECT continues to satisfy it. Listed for completeness so the Spec records it as a preserved co-constraint. |

### Boundary semantics — now LOCKED at the Frame-exit gate (were open R-0020 slots)

The two boundary behaviors this Frame originally surfaced as open binary-observability gaps
(`limit = 0`; malformed-cursor error semantics) were **closed at the Frame-exit gate** by the
folded security-review conditions — they are now **locked directions**, not open slots:

- **`limit = 0` semantics → locked (D10).** `0` MUST never disable the `LIMIT` (no unbounded
  query); it resolves to clamp-up-to-default or empty page. The no-unbounded invariant is locked;
  the only residual is the clamp-up-vs-empty choice, carried to R-0020 as a value to pin (not an
  open binary-observability gap — both options are bounded and testable).
- **Malformed / foreign / out-of-range cursor → locked (D9).** Malformed cursor → R-0010-f
  structured "parameter invalid" error, validated as a 26-char ULID at the host-fn boundary, no
  DB/schema detail leaked. Valid-but-out-of-range → empty page (derivable). Foreign-but-valid
  ULID → safe, confined by `workspace_id` (QA-3). The injection-safe / tenant-scoped posture was
  confirmed sound by the security-mode review (§6.3).

**No open binary-observability gaps remain.** R-0020's boundary semantics are locked by D8–D11;
Stage 3 assigns the requirement id and pins the two residual *values* named in the R-0020 row
above (the `statement_timeout` value and the `limit = 0` resolution).

**Mechanically-derivable boundaries — locked now (consequences of D2/D4, not new decisions):**
- **Empty result** (type has zero artifacts of that type in the workspace): `ids = []`,
  `has-more = false`, `next-cursor = none` (falls out of fetch-one-extra + the D4 invariant).
- **Exact-last-page** (the final page is exactly full, no extra row): `has-more = false`,
  `next-cursor = none` (fetch-one-extra finds no `cap+1`-th row).

### New project ADR? — decision: **no new P-ADR.**

Keyset-over-offset is a real decision with a rejected alternative (offset/page-number
pagination), which Honesty's Alternatives-Considered discipline says must be recorded. It
**is** recorded — in the intake's Non-goals ("No offset/page-number pagination … offset
degrades on large append-only tables and is non-resumable across concurrent inserts") and it
will be carried into R-0020. This amendment is **pure application** of existing project canon
(P-0006 tenant WHERE-clause, P-0013 typed export, P-0001 single-document ULID-PK layout) plus
the spec R-IDs — no novel architectural decision warrants a standalone `P-NNNN` ADR. Decided
and stated per the designed-tier "decide, don't manufacture" discipline.

## 6. Risk profile (security-relevant — carried to the security-mode review)

Security dimensions below. No new trust boundary is introduced — the change **hardens an
existing read path**. The mandatory Frame-stage security-mode review **ran and passed**
(Approve-with-conditions; the conditions are folded into this finalized Frame); the
implementation-time pre-push security review remains in scope per the G-0003 2026-06-25
amendment.

1. **Result-set-*size* DoS bound (the hard cap).** Before: the list path issues an unbounded
   `SELECT` — an unbounded-result-set denial-of-service surface (intake Evidence; ex-finding
   "Warden 1113 #3"). After: the `cap = 500` clamp (host-side, D5) bounds the **size** of every
   response (QA-2 measure 1). The cap bounds rows **returned**, not rows **scanned** — see the
   scan-cost dimension next.
2. **Result-set-*scan-cost* DoS bound (the `statement_timeout` backstop — surfaced by the
   security review).** A `LIMIT` does not bound scan cost: under a `filters` predicate on a
   **non-indexed** field (outside the four R-0001-d expression-indexed fields) that matches
   sparsely or zero rows, the keyset `ORDER BY id LIMIT cap+1` walks the entire `id`-btree tail.
   The Wasmtime epoch deadline (R-0007-b) bounds guest wasm, **not** the host Postgres query, and
   there is no `statement_timeout` in the substrate today. The host-side `statement_timeout`
   backstop (D8) closes this surface — the deterministic runtime backstop sub-layer additive to
   the cap (P-SecurityLayered). Measure: QA-2 measure 4 (`outcome = "timeout"`, bounded
   wall-clock).
3. **Tenant isolation on a read path.** The `workspace_id = $ctx` WHERE-clause condition
   (R-0006-d, P-0006; CI lint R-0018-d) must survive the keyset rewrite intact — a keyset rewrite
   that dropped or post-filtered the workspace condition would be a cross-tenant disclosure. The
   measure is QA-3 (R-0006-d/R-0018-d lint green + zero-foreign-row cross-workspace probe).
   **Confirmed sound** by the security-mode review — unchanged.
4. **Cursor injection surface (sub-note — confirmed sound).** The `cursor` is an opaque
   client-supplied string. It is consumed as a **bound query parameter** ANDed with
   `workspace_id = $ctx` and `type = $type` — never string-interpolated into SQL — so it is
   injection-safe and tenant-scoped by construction. A foreign/out-of-range cursor can at worst
   return an empty or shifted page **within the caller's own workspace**; it cannot read another
   tenant's rows or escape the cap. The injection/isolation posture was **confirmed sound** by
   the security review. The **error/return** semantics for a malformed cursor — previously an
   open R-0020 slot — are now **locked (D9)**: ULID-validate at the host-fn boundary, return the
   R-0010-f structured error, no DB/schema detail leaked (this also caps the otherwise-unbounded
   `option<string>` input).
5. **Telemetry data-minimization (locked, D11).** The paging params `cursor` / `next-cursor`
   **are artifact IDs**; R-0004-a forbids artifact IDs in the per-verb metric record. They are
   excluded from metric **and** structured-log emission (QA-6) — a read path must not leak
   artifact IDs into telemetry.

**Security-mode review trigger — carried forward to implementation:** this Frame changes a read
path with a DoS-bound (two sub-layers) and a tenant-isolation co-constraint. The pre-push
security review (security mode) is in scope for the implementation; a Critical/High finding is
fixed in-loop and escalates to Peter only if unresolvable (G-0003 amendment 2026-06-25).

## 7. Intent self-report (preventive)

- **JTBD reading.** I read the job-to-be-done as: *"A caller listing an append-only artifact
  type needs results in bounded, resumable, creation-ordered pages, so a single `list` call is
  predictable and safe-sized no matter how large the workspace has grown, and the caller can
  walk the full set page by page — end-to-end through the MCP client."* The dual driver is
  **operational** (bounded predictable responses over unbounded growth) and **security** (the
  cap closes the unbounded-`SELECT` DoS surface). I do not read this as a general "add
  pagination everywhere" mandate — intake Non-goal scopes it to the `artifact-list` read path
  and its guest export only.
- **Gate decisions folded (Frame-exit, 2026-06-29):**
  - **Page sizes ratified exact (D5).** The intake's `≈100 / ≈500` is now pinned to exact
    `default = 100` / `cap = 500`, **ratified by the maintainer at the gate** — no longer a
    provisional reading. The `~`→exact wording is exact throughout.
  - **`limit = 0` and malformed-cursor semantics — now LOCKED, not open (D10, D9).** These two
    boundaries, surfaced as open R-0020 slots in the draft, were **closed at the gate** by the
    folded security-review conditions: `limit = 0` never disables the `LIMIT` (D10);
    malformed cursor → R-0010-f structured error, ULID-validated at the boundary, no leak (D9).
    No open binary-observability gaps remain (§5).
  - **Scan-cost backstop added (D8).** The maintainer chose the host-side `statement_timeout`
    remediation (over the "restrict filters to indexed fields" alternative) for the scan-cost
    surface the security review found the cap leaves open.
  - **No strain** against any Non-goal: no offset path, no `COUNT`, no schema/index change, no
    filter-semantics change, no RLS activation, no out-of-scope spec/interface touched — every
    locked direction, **including the new D8 `statement_timeout`** (a session GUC), stays inside
    the intake's Non-goals.
- **Finalize status.** With the conditions folded and all cited requirement anchors verified
  against the spec, this Frame is **finalized** — the gate passed and the closed-world the
  Stage-3 Spec draws from is corrected. (The original draft's `done_with_concerns` was driven
  by the two now-closed boundary gaps.)

## 8. Provenance

This is an **in-place amendment** to an existing architecture spec. The BOM chain stays
anchored to the **product-tier** upstreams (`docs/src/intent/mnemra-core.md` /
`mnemra-core-frame.md`, **unchanged**). The paging intake
([`docs/intent/artifact-list-paging.md`](artifact-list-paging.md)) and this Frame
([`docs/intent/artifact-list-paging-frame.md`](artifact-list-paging-frame.md)) are the
**localized design records** for the amendment — the localized upstreams the Spec amendment
draws from.

- The product-tier `[audit_chain.intent]` / `[audit_chain.frame]` BOM blocks are **not**
  touched by this Frame.
- The BOM (`…-substrate.bom.toml`) is **not** edited here. Puck performs the Frame-exit BOM
  lock after the maintainer checkpoint.

## 9. Consultations

- **none.** The design was locked with the maintainer prior to this run; Stage 2a elicitation
  collapsed (brownfield-extension). The `advisor()` reviewer pass that sharpened the QA-measure
  conjunctions, the keystone constraint edge, and the open-slot discipline is recorded as
  authoring-internal, not a maintainer consultation.

## Changelog

- **2026-06-29** — **Frame finalized (Frame-exit gate passed — Approve-with-conditions).** The
  security-mode review accepted the Frame with no Critical/High and confirmed tenant isolation
  (D7/§6.3/QA-3) and cursor injection-safety (§6.4) sound. Folded the maintainer's three gate
  decisions (accept + proceed; scan backstop = host-side `statement_timeout`; page sizes exact
  `100`/`500`) and the five security-review conditions: **A** — corrected QA-2 (rows *returned*
  ≤ `cap+1`, dropped the false rows-*scanned* claim) and added the host-side `statement_timeout`
  scan-cost backstop as new direction **D8** (anchor `P-SecurityLayered` runtime-backstop layer;
  `outcome = "timeout"` per R-0004-a) with adversarial QA-2 measure 4; **B** — pinned the cap
  clamp host-side in `artifact-list` (D5; rationale: guests IO-free under the Wasmtime sandbox
  R-0002 + host-fn ABI R-0012-a → sole DB chokepoint); **C** — malformed-cursor → ULID-validate
  at boundary + R-0010-f structured error, no leak (new direction **D9**, ex-§5 open slot); **D**
  — `limit = 0` never disables `LIMIT` (new direction **D10**, ex-§5 open slot); **F** — paging
  params excluded from metric/log emission per R-0004-a (new direction **D11**). Swept §2
  (P-SecurityLayered row + Conflicts-with finding) for the size-vs-scan split; added QA-5
  (input-validation/boundary) and QA-6 (telemetry); reframed §5 (no open binary-observability
  gaps remain). All cited R-IDs verified against the spec (one sharpening noted: the "guests
  IO-free" basis is R-0002 sandbox + R-0012-a, not a literal R-0012 sentence). No BOM edit; no
  commit; task #1847 stays `in_progress` (Spec amend is the next stage).
- **2026-06-29** — Frame authored (Stage 2b, brownfield-extension). Renders the
  maintainer-locked artifact-list keyset-pagination design as canon-anchored directions
  (D1–D7), ATAM QA scenarios (QA-1..QA-4), and open R-ID slots. Keystone anchor:
  `P-LockContract` re-derive-on-reshape (`constraint-edges.md:140/:152`). Anchoring corrected
  to R-0012-a/-f + R-0019-a (not R-0006-d, which is a preserved co-constraint). Two
  binary-observability gaps surfaced as R-0020 Spec slots (`limit=0`, malformed-cursor
  semantics) → status `done_with_concerns`. No BOM edit; no commit; task #1847 stays
  `in_progress`.
