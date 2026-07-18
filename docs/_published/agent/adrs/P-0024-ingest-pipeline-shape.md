---
title: "P-0024: Ingestion Pipeline Shape"
summary: "Resolves the ingestion-pipeline Frame's {{P-ingest-pipeline-shape}} slot — the ADR-16-materialized shape of the ingest subsystem: core host residence; the typed loader-seam stage chain with the D3 admission insertion point; durable one-superset raw staging (logged, in-substrate blob home, per-source-kind retention); the transform-harness conventions (transform-as-job, update-with-audit re-derivation, derived_from lineage, declared determinism, and the stable lineage-addressed canonical-id scheme with its one-to-many output discriminator); reconciliation-first arrival with the three-join scan (filesystem→staging, the staging→jobs orphan-sweep predicate, and the staging↔dead-letter completion of interrupted terminal-failures); the ingest system principal and the ES-6 sanctioned declared-writer invocation at the canonical write (the harness as a new caller, ES-6's two-writer roster unchanged); BLAKE3 path-scoped identity; recency + load-shedding; the fail-closed failure vocabulary + quarantine bounds; the reliability posture; and the typed DFD. Renders locked Frame content at ADR precision; makes no fresh decisions. Binding requirement text is single-sourced to spec R-0099–R-0115."
primary-audience: agent
---

---
status: "proposed"
date: "2026-07-17"
decision-makers: ["the maintainer"]
consulted: ["the orchestrator"]
informed: []
supersedes: null
superseded_by: null
overrides: null
---

# P-0024: Ingestion Pipeline Shape

**Project:** mnemra-core

## Status

`proposed`

Authored at Stage 3 of the ingestion-pipeline cluster, resolving the Frame's `{{P-ingest-pipeline-shape}}` slot (the architecture-overview's ADR-16 slot, materialized) per [placeholder-resolution](placeholder-resolution.md). It moves to `accepted` at the ingestion-pipeline **spec-exit gate**, reviewed with the spec [2026-07-16-ingestion-pipeline](../../specs/2026-07-16-ingestion-pipeline.md), following the retrieval/reporting/coordination precedent (companion ADRs move to `accepted` at the spec-exit gate). The subsystem's directions were locked at the Frame-exit gate (2026-07-17); this ADR renders them at ADR precision and makes **no** fresh decisions. Binding requirement text is single-sourced to the spec's R-IDs (R-0099–R-0115); this ADR carries the shape, the rationale chain, and the four renderings the Frame routed specifically here (the typed DFD, the ES-6 declared-writer invocation, the staging→jobs orphan-sweep predicate, and the lineage-addressed canonical-id scheme).

## Context and Problem Statement

After `0.14.0`'s one-shot corpus migration, content keeps arriving and nothing ingests it durably or automatically — the exact per-session re-derivation cost the product exists to kill. The architecture-overview's forward design named an ingest subsystem ("ELT shape, external content only… NEW ADR needed") with an ADR-16 slot for its staging schema, transform-as-job conventions, re-derivation policy, and per-source-kind retention. This ADR is that slot, materialized: the shape of a core host subsystem — source registry, reconciliation-first arrival, a shared loader seam two drivers ride, durable raw staging, a transform harness on a minimal `JobQueue` port, egress-denied binary extraction (#324), recency instrumentation, and the quarantine/failure path — under the maintainer's LOCKED ELT-over-ETL ruling (raw = truth; derive as a re-runnable stage) and the LOCKED loader-seam share (one seam, two drivers).

The shape is constrained by the locked corpus canon it must conform to rather than amend: [P-0016](P-0016-edge-schema.md) ES-6's two-writer edge partition (ingest writes no edge rows), [P-0015](P-0015-provenance-envelope-source-roles.md)'s provenance envelope and trust axis (the fired PE-7/PE-11 trip-wires dispositioned in the `{{P-0015-A1-ingest-trust-class}}` amendment), [P-0017](P-0017-storage-cluster-model.md) D-SoT (canonical rows are content with `derived_from` lineage; operational tables sit outside the four-shape taxonomy), and [P-0009](P-0009-rls-admin-token.md)/[P-0006](P-0006-v0-tenant-enforcement.md) on the write path.

## Decision Drivers

- **ELT: raw = truth, derived = disposable** (intake Hard constraint 2, LOCKED — the strangler-migration walk 2026-07-06): staging is the re-derivation source-of-record and must be durable; derivations re-run over it with zero re-fetch.
- **One seam, two drivers** (intake Hard constraint 1, LOCKED): the loader seam is the intrinsic contract this bundle exists to lock; driver differences confine to drivers (P-LockContract, P-MinBlastRadius).
- **Conform to locked corpus canon, do not amend it** (P-VerifyInheritedState; the Frame-exit gate ratification 2026-07-17): ingest writes no edge rows and reaches the edge table only through ES-6's sanctioned declared-writer path; the trust demotion re-keys onto the stored `trust_class` at the trust predicates, never by relabeling `origin`.
- **Security guarantees are mechanism-carried, never convention-carried** (P-GuaranteeByMechanism): egress-deny is a capability/enclosure property; no-partial-write is transactional; dedup is a schema constraint; trust demotion is a stored-field rule.
- **Instrument in the same increment** (P-InstrumentBefore; Observability | Simplicity default-to-Observability): the subsystem is a production surface at first run and ships with recency, reconciliation, quarantine, and backlog measures.

## Considered Options

The shape decisions this ADR renders, each with the alternative the Frame recorded and decided against. (The edge-writer question — how declared frontmatter relations reach the edge table — was decided at the Frame-exit gate, and its losing arm would have amended [P-0016](P-0016-edge-schema.md), not this ADR; it is therefore recorded as settled in [§ IPS-6](#ips-6--the-ingest-system-principal-and-the-es-6-declared-writer-invocation), not enumerated as an option here.)

1. **Residence — core host subsystem** (chosen) vs a plugin (WASM host-fn surface for fs-watch would need designing; disqualifying at this tier) vs a separate ingest-worker binary (a second deployable; deferred behind the residence-move trip-wire).
2. **Staging durability — logged** (chosen) vs `UNLOGGED` (the pre-ADR overview's note, falsified by the ELT ruling — a raw store Postgres truncates on crash cannot be the ELT source-of-record).
3. **Staging shape — one superset table** (chosen; per-kind variance as data, the [P-0016](P-0016-edge-schema.md) ES-1 precedent) vs per-source-kind table proliferation.
4. **Blob home — in-substrate `raw_bytes` (TOAST-backed)** (chosen; the staging row and its payload commit in one transaction, backup/PITR for free) vs a filesystem blob store (fs/db two-phase residue, which SC9/SC10 would have to reason about).
5. **Re-derivation policy — update-with-audit** (chosen; overview open question 8 dispositioned) vs new-rows-append (would mint a new artifact identity per derive-run, breaking [P-0015](P-0015-provenance-envelope-source-roles.md) PE-10 citation stability).

## Decision Outcome

Rendered as IPS-1..IPS-11. Binding requirement text lives in the spec (R-0099–R-0115); the shape and the four routed renderings are below.

### IPS-1 — Residence: core host subsystem; zero new MCP tools; registration is admin CLI control-plane

The subsystem is host code inside the mnemra-core host process, beside the retrieval and coordination subsystems (the [P-0002](P-0002-core-plugin-partition.md) verb-on-content walk: arrival detection, staging, and job execution are operational substrate, not CRUD on content — the subsystem *produces* content rows, its own machinery is not content). Nothing exports through the plugin ABI; [P-0005](P-0005-v0-signing-chain.md)'s plugin-load surface is unchanged (vacuously-by-residence). Zero agent-visible MCP tools ship — the verb budget's reserved slot ([P-0022](P-0022-coordination-cluster.md) D-VERBS) is untouched; ingested content is served through the existing retrieval verbs once indexed. Source register/list/retire and quarantine list/inspect are admin-scoped CLI control-plane operations in the [P-0009](P-0009-rls-admin-token.md) admin family, with the matrix rows added by `{{P-0009-A1-ingest-control-plane}}` (this ADR's sibling amendment). *(Spec: R-0099.)*

### IPS-2 — The loader seam: a typed stage chain; the admission chain is the D3 insertion point

The seam (intake Hard constraint 1) is a typed stage chain with one contract — **bytes + source-metadata in → a durably-staged raw artifact with [P-0015](P-0015-provenance-envelope-source-roles.md)-envelope provenance out, a transform job enqueued** — over ordered stages `parse/normalize → frontmatter-map → provenance-stamp → admission chain → write-through`. `write-through` is a transactional staging insert; the transform-job enqueue follows **post-commit, best-effort** (the durable staged row is the guarantee; IPS-4's orphan sweep re-enqueues any staged-but-untransformed row, so the `JobQueue` port carries no SQL-transactional coupling). The **admission chain is the D3 classify-at-write insertion point**: a classification stage inserts by registering one more admission stage with the typed contract `(candidate artifact + provenance) → admit | quarantine(reason) | annotate(policy fields)`, where `annotate` writes only existing PE-2 policy columns — no schema change, no path reshape. The stage itself stays undesigned (Frame Non-goal 7; Tier-2-gated per D3). Driver differences stay in drivers: the `0.14.0` migrator owns kill-and-resume/byte-equivalence/source-read-only; the `1.2.0` watcher owns arrival detection/dedup/recency. *(Spec: R-0100.)*

### IPS-3 — Durable raw staging: logged, one superset table, in-substrate blob, per-source-kind retention

Staging is **ordinary logged tables — never `UNLOGGED`** (raw = truth). It is **one superset table** ([spec § Data Model](../../specs/2026-07-16-ingestion-pipeline.md#data-model)): per-kind variance is data (registry fields + `source_kind` + payload polarity), not schema forks (the [P-0016](P-0016-edge-schema.md) ES-1 precedent); the payload is `raw_text | raw_bytes` with exactly-one-non-null (CHECK-enforced). Retained binaries live in-substrate (`raw_bytes`, TOAST-backed), bounded by the per-file size cap, so the staging row and its payload commit in one transaction. Retention is a per-source-kind closed enum `{long-lived, transient}`; every V0.1 kind is `long-lived` (SC3 requires re-derivation with the arrival source absent, so the fs copy is never the raw). [P-0018](P-0018-core-entity-manifest.md)'s note that blob handling "is owned by the storage/ingest layer" is thereby given its V0.1 answer: the staging raw store is the blob substrate. *(Spec: R-0102.)*

### IPS-4 — Arrival: reconciliation-first; three joins; the staging→jobs orphan-sweep and staging↔dead-letter predicates

The **periodic/startup reconciliation scan is the primary arrival mechanism**; filesystem events accelerate latency only (a missed event costs latency, never correctness — P-GuaranteeByMechanism). Startup reconciliation is the same scan, so the crash-recovery path runs on every startup and cannot silently rot (P-TrustworthySignal). The size/mtime pre-filter is a fast-path heuristic bounded by a periodic full-content-rehash cadence (a size-preserving edit with reset mtime is caught within that window; the pass is I/O-paced so it does not contend with the `arrival→staged` critical path).

The scan reconciles **three joins**:

1. **filesystem → staging** — observed `(path, size/mtime, content hash on candidates)` against staging identity (IPS-8); an unstaged or changed file is staged.
2. **staging → jobs (the orphan sweep)** — a staging row whose post-commit best-effort enqueue (IPS-2) never landed a transform job is re-enqueued. The orphan predicate, SQL-expressible (the [FIV-1 fold](../../intent/ingestion-pipeline-frame.md) obligation, rendered here):

   > A `staging` row is a **transform orphan** when `status = 'staged'` **and** `staged_at < now() − enqueue_orphan_window` **and** no `jobs` row exists with `kind = 'transform'`, `payload ->> 'staging_id' = staging.id`, and `state ∈ {'pending','claimed','completed','dead_letter'}`. Each orphan is re-enqueued (one transform job per orphan row); a staged row that already has a live, completed, **or dead-lettered** transform job is never re-enqueued.

   The `enqueue_orphan_window` (default 2× the scan interval — [spec § Numeric calibrations](../../specs/2026-07-16-ingestion-pipeline.md#numeric-calibrations)) is wide enough that a normally-enqueued job is never falsely swept. `dead_letter` is **inside** the guard set so a poison-message row that has already dead-lettered is never re-enqueued into a fresh retry cycle — the crash-window it leaves is closed by join 3, not by re-enqueue. This is the mechanism behind the no-partial-write / delivery guarantee (IPS-10): staged-durability-plus-reconciliation, not a cross-resource staging+enqueue transaction.
3. **staging ↔ dead-letter (interrupted terminal-failure completion)** — because the R-0104-f staging-quarantine write and the job's terminal `dead_letter` transition are **separate resource writes** with no transactional coupling (JQ-3 forbids it), a crash between them leaves a `staged` row whose only job is `dead_letter`. Such a row is **not** an orphan to re-enqueue but an interrupted terminal-failure to **complete**, idempotently, to its designed terminal state:

   > A `staging` row is an **interrupted terminal-failure** when `status = 'staged'` **and** a `jobs` row exists with `kind = 'transform'`, `payload ->> 'staging_id' = staging.id`, `state = 'dead_letter'`, **and** no `jobs` row for it exists in `state ∈ {'pending','claimed','completed'}`. The scan drives it to `status = 'quarantined'`, `rejection_class = 'transform-failure'` — idempotently (a row already `quarantined` is a no-op; no canonical row is written).

   This closes the poison-loop gap Bolt B1 ≡ Warden M2 identified in the R-0112-c working-state → working-state guarantee, using the same one-scan reconciliation mechanism as joins 1–2 — **not** same-transaction atomicity of the `fail()`/quarantine writes, which would put SQL-transactional coupling in the port and break on an SQS/Temporal adapter (JQ-3). *(Spec: R-0101, R-0104-f, R-0112-b/-c.)*

### IPS-5 — Transform-harness conventions: transform-as-job, update-with-audit, lineage, the lineage-addressed canonical-id scheme

A transform is a registered, versioned unit `(source_kind, transform_id, version, declared_deterministic: bool)` with signature `staged raw → canonical candidate rows`, holding no capability beyond bytes-in/content-out (extraction transforms additionally run sandboxed, IPS-7). Outputs land as ordinary content rows ([P-0001](P-0001-storage-layout.md) C1) each carrying a `derived_from` soft ref to the staging row ([P-0017](P-0017-storage-cluster-model.md) D-SoT). Re-derivation policy is **update-with-audit**: a re-run updates the existing canonical row at the same identity, with the [P-0001](P-0001-storage-layout.md) system-versioned history pattern as the audit trail. Re-run is first-class (delete-derived → re-run repopulates from staging alone, no arrival-source read); the harness requires **idempotent convergence**, not byte-identity, with per-transform determinism a declared, validator-backed property (`declared_deterministic`, seeded byte-compare where `true`).

**The lineage-addressed canonical-id scheme (the one-to-many discriminator, rendered here):**

> The canonical id is a **deterministic function of `(workspace_id, source_id, source_path, transform_id, output_key)`** — the IPS-8 staging identity **minus its `content_hash`**, plus the transform identity, plus a transform-assigned **`output_key`** that discriminates each logical output where **one transform emits many canonical rows** (a PDF or DOCX extracting into several artifacts). One staging lineage therefore maps to **N** canonical ids — one per `output_key`.
>
> `content_hash` and `transform_version` are **NOT** part of the identity: including `content_hash` would mint a new identity on every content change (contradicting update-with-audit and breaking [P-0015](P-0015-provenance-envelope-source-roles.md) PE-10 citation stability), and `transform_version` belongs to the audit history, not the identity. The `output_key` is stable per logical output (a transform assigns the same `output_key` to the same logical output across re-runs), so each id is **stable across content-changing re-derivations** and **reconstructable from staging alone** after delete-derived (SC3, including for a `declared_deterministic: false` transform, whose *content* may vary while its *identity set* does not).

**Retire-on-shrink (the one-to-many update-with-audit completion, rendered here):** update-with-audit updates the surviving ids in place, but a re-run must also **retire** each canonical row whose `output_key` is present in the prior set but **absent from the new set** — the non-empty set-difference `prior − new` — **regardless of whether the new run also adds `output_key`s the prior set lacked**, so a **drop-and-add** re-run (prior `{a,b,c}` → new `{a,b,d}`, which is not a subset) retires `c` and mints `d`; otherwise the vanished output's stale canonical row would serve indefinitely (only the full delete-derived → re-run path drops it, by reconstructing the id set from staging alone). An empty new set is disambiguated by the transform's **success/failure signal, not the output count**: the retire diff is computed only on the success path, so a re-run that *completes* emitting nothing retires **all** prior rows for the lineage, whereas a transform **failure** diverts to R-0104-f quarantine (`transform-failure`) before any retire is computed and retires nothing. Retirement is a **soft-delete / mark-superseded through the [P-0001](P-0001-storage-layout.md) system-versioned history**, in the **same re-derivation transaction** that updates the survivors — never a hard delete and never a re-minted identity, so PE-10 citation stability holds for the retired id (a citation resolves to a superseded-terminal, not a dangling reference). *(Spec: R-0104-g.)*

This is the reading under which [P-0015](P-0015-provenance-envelope-source-roles.md) **PE-10's** "content-addressed, per the V0 substrate" citation wording and [P-0001](P-0001-storage-layout.md)'s system-versioned history point the same way: PE-10 citation stability is preserved by the stable-id-with-history model, not by content-hash addressing of canonical rows. *(Spec: R-0104.)*

### IPS-6 — The ingest system principal and the ES-6 declared-writer invocation

All ingest-path writes run under a distinguished **system principal naming the ingest subsystem** (the [P-0015](P-0015-provenance-envelope-source-roles.md) PE-3 pattern; not a workspace role, not a synthetic admin token), bound to a concrete `workspace_id` from the source registration row at a single [P-0006](P-0006-v0-tenant-enforcement.md) context-construction site. Because the principal is cross-workspace-write-capable by design, that construction site is the audited chokepoint and the invariant is *the principal cannot commit a row whose `workspace_id` differs from its source's registered workspace* (the spec's negative test, R-0107-b). `created_by` resolves to the `system` actor ([P-0018](P-0018-core-entity-manifest.md) D-ACTOR — "host-side extractor, scheduled job").

**The ES-6 declared-writer invocation (the harness as a new caller, rendered here):**

> The loader and harness hold **no edge-write capability of their own** — no code path in the ingest subsystem writes an edge row directly. A relation declared in ingested frontmatter reaches the edge table **only** through [P-0016](P-0016-edge-schema.md) **ES-6's sanctioned declared-writer path** — the typed, entity-transactional content write path the `repos`-plugin CRUD family drives (writer 1 of ES-6's two-writer partition, `origin ∈ {declared, system}`). At the transform's canonical entity write, for each declared frontmatter relation, the harness **invokes that existing path** — passing the [P-0006](P-0006-v0-tenant-enforcement.md) `WorkspaceCtx` (the ingest system principal) and the relation as `origin = declared` — so the declared edge is written **transactionally with its entity** by the path ES-6 already sanctions.
>
> The harness is a **new caller** of writer 1's path, **not a third writer**: ES-6's writer roster stays exactly two (the `repos`-plugin CRUD path for `origin ∈ {declared, system}`; the host extractor confined to `origin = extracted`), ES-2/ES-3's origin and `source_span`-iff-`extracted` semantics hold untouched, and ES-4's extractor-integrity rule (the extractor never mints `declared`/`system` authority) is unaffected. The two-writer split is extractor-confinement, not a per-subsystem pattern; the harness's declared edges are writer 1's charter ("declared relations transactional with their entities"), so the harness needs no writer of its own, and the trust boundary for external-class sources is handled entirely by the trust-predicate keying in `{{P-0015-A1-ingest-trust-class}}` (IPS-9), never by an edge-writer change.

This conforming shape was **ratified at the Frame-exit gate (2026-07-17; Frame §15 item 3)**; it is rendered here as settled. *(Spec: R-0107.)*

### IPS-7 — Extraction sandbox: egress-denied by construction; per-format hybrid lanes; fail-closed terminal

Binary extraction runs egress-denied. **Lane A** (default) is a `wasm32-wasip2` component ([P-0019](P-0019-plugin-contract.md) D4) on the host Wasmtime engine ([P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) Decision A) whose world imports no network and no filesystem — egress is structurally impossible — with resource bounds on [P-0007](P-0007-plugin-resource-limits.md) mechanisms (fuel + epoch + memory, kill-and-replace) at **extraction-calibrated** numbers (P-0007's plugin values are not inherited). **Lane B** (native, for crates that cannot run in WASM) runs a sandboxed subprocess inside a structural egress-denying enclosure (Linux netns/seccomp + landlock; macOS a container boundary), and is a **legal lane on a host iff its enclosure is verifiably present**, checked by a startup/registration lane-availability check. The **lane-selection terminal is fail-closed**: a format runs Lane A iff its crate compiles to `wasm32-wasip2` and performs within budget (verified by an implementation-start build+perf spike); a failing spike moves it to Lane B, never to an unsandboxed path; a format that clears **no** lane whose egress-deny precondition holds on the running host quarantines `unextractable-here` — there is no unsandboxed fallback anywhere in the selection function. The XML parser families in every viable crate perform no entity resolution at all, so XXE is absent by construction *and* the sandbox denies egress regardless (defense in depth). The dogfood consequence — PPTX (and DOCX if its spike fails) unavailable on a native macOS host until a container boundary is provided — is deployment posture accepted at the Frame-exit gate (Frame §15 item 4), not a design change. *(Spec: R-0105, R-0115.)*

### IPS-8 — Content identity + dedup: BLAKE3, path-scoped, observable no-ops

Staging identity is `(workspace_id, source_id, source_path, content_hash)` with `content_hash = BLAKE3(raw payload)`. BLAKE3's 256-bit collision resistance is its own documented design property and is the direct security ground of this identity boundary; [P-0008](P-0008-admin-token-shape.md) is the in-corpus algorithm-choice precedent only. Identical re-arrival is a no-op recorded as an arrival event (suppression never silent); changed content at an existing path is a new staging row; equal content at a different path is a distinct row (path-scoping bounds dedup poisoning). Cross-source global dedup is deliberately not performed at V0.1 (integrity-first). *(Spec: R-0109.)*

### IPS-9 — Trust-class mapping for the new source kinds

Per-source `trust_class` (`{first-party, external}`, admin-attested at registration, undecidable → `external`) is consumed mechanically at provenance-stamp time. `first-party` frontmatter keeps its `declared` authority; `external`-class frontmatter relations still record honest `origin = declared` edges via the IPS-6 sanctioned path, but the **trust predicates are keyed on the stored `trust_class`** — a `declared` trust-affecting edge enters PE-7's `outdated`/hard-supersession predicate only when its declaring artifact's trust class is `first-party` (`{{P-0015-A1-ingest-trust-class}}`). Demotion is thus by trust-predicate keying, never by relabeling `origin`. When such an external-class `declared` trust-affecting edge is held out of a trust predicate, the **curatorial signal that records the withholding is a defined mechanism, not an assertion** (R-0108-f): a monotonic `curatorial_signal_count` on the ingest run/job records **and** a redacted named-level log line at edge-record time (P-TrustworthySignal — the withholding is operator-visible, never silent; the [P-0015](P-0015-provenance-envelope-source-roles.md) operator-side-signal precedent). The fired PE-7 (N10) and PE-11 (ingest-half) trip-wires are dispositioned in that amendment; PE-11's adopter-deployment half stays live on its own instrument. *(Spec: R-0108, R-0108-f; ADR: `{{P-0015-A1-ingest-trust-class}}` in [P-0015](P-0015-provenance-envelope-source-roles.md).)*

### IPS-10 — Recency, load-shedding, and the failure path

Two ingest-owned recency spans (`arrival→staged`, `staged→canonical-committed`) are emitted per source kind on plain workspace-scoped run/job records (the RA-7 family; [P-0010](P-0010-storage-substrate-engine.md) D8); the composite `arrival→indexed` span is computed by join against the retrieval cluster's index-build records, not by coupling (both sides backfillable — IB1). Emission is redacted (derived IDs only; source paths hashed; no frontmatter bodies/titles). Load-shedding: the staging insert is the protected critical path; enqueue and transform execution are off it; **queue depth is the unbounded, measured absorption buffer, but transform execution concurrency is bounded** by a fixed-size **host-owned transform-worker pool** (R-0111-b — not P-0007's plugin pool, and distinct from P-0007's per-instance kill-and-replace inside a single Lane-A extraction), so a burst absorbs into queue depth rather than into an unbounded number of simultaneously-executing extraction instances; the aggregate execution-resource ceiling under any burst is `worker_pool_size × per-lane budget`, which is what makes QA-7 falsifiable. The failure path is fail-closed over a **closed rejection vocabulary** (`malformed-file`, `unparseable-binary`, `oversized-input`, `decompression-bomb`, `path-traversal-name`, `unregistered-source`, `transform-failure`, `unextractable-here`); no partial canonical write (the canonical write is transactional per staging row); every operation moves working-state → working-state; quarantine is inert, metadata-only-inspectable, and aggregate-bounded. *(Spec: R-0110, R-0111, R-0112, R-0115.)*

### IPS-11 — Reliability posture

No numeric availability/recovery target is adopted at V0.1; contractual recovery behavior binds instead — SC11 reconcile-on-restart and SC9 working-state → working-state, both tested-binary — with the recency metric as the future target's calibration baseline (P-InstrumentBefore). The deferred numeric target fires on the recency/run-record breach of the spec bound, or on the first adopter deployment. *(Spec: R-0113.)*

## Typed DFD

The formal typed data-flow diagram the Stage-3 obligations require (Frame §9 routes the artifact here), so a downstream threat-model refresh has typed anchors. **Trust boundaries:** (TB-ws) the workspace boundary — everything below sits inside it except the external entities, which cross it; (TB-adm) the admission-control boundary — nothing outside a registered source is ever staged; (TB-sbx) the extraction-sandbox egress-denying enclosure — Lane A's capability world and Lane B's verified per-host enclosure.

| Kind | Element | Notes / owning control |
|---|---|---|
| External entity | `EE-watched-root` — the watched-root filesystem (partially-untrusted content) | crosses TB-ws/TB-adm; TOCTOU-safe open, `S_ISREG`-only (R-0106-c) |
| External entity | `EE-operator` — the admin operator (CLI control-plane) | crosses TB-ws; admin-token auth, [P-0009](P-0009-rls-admin-token.md) + `{{P-0009-A1-ingest-control-plane}}` (R-0099-c) |
| Process | `P-scanner` — reconciliation scanner / watcher (supervised task) | least authority, read-only roots (R-0106-b) |
| Process | `P-seam` — loader seam (`parse → frontmatter-map → provenance-stamp → admission chain → write-through`) | typed stage chain; D3 insertion point (R-0100) |
| Process | `P-harness` — transform harness / workers | idempotent handlers; canonical write transactional per row (R-0104) |
| Process | `P-laneA` — WASM extraction component | no network/fs imports — egress structurally impossible (R-0105-a) |
| Process | `P-laneB` — sandboxed subprocess extractor | enclosure verified per host; fail-closed terminal (R-0105-b/-c) |
| Process | `P-queue` — `JobQueue` port + Postgres SKIP LOCKED adapter | at-least-once, single-claimant (R-0103; [P-0025](P-0025-job-queue.md)) |
| Data store | `DS-sources` — source registry (root, kind, `trust_class`, retention) | admin-attested; allow-list-validated (R-0099-d) |
| Data store | `DS-staging` — durable raw staging (logged; `raw_bytes` TOAST) | BLAKE3 path-scoped identity; `UNLOGGED` forbidden (R-0102/R-0109) |
| Data store | `DS-jobs` — the queue table | UUID v7; closed state enum |
| Data store | `DS-canonical` — canonical content rows (`derived_from` lineage) | ordinary content ([P-0001](P-0001-storage-layout.md) C1); update-with-audit (R-0104) |
| Data store | `DS-runrec` — ingest run/recency records | RA-7 family; redacted emission (R-0110) |
| Data store | `DS-edges` — the edge table (**written only via the ES-6 sanctioned path — not owned by ingest**) | [P-0016](P-0016-edge-schema.md) ES-6; harness is a caller, roster unchanged (IPS-6) |
| Flow | `DF-read` — `EE-watched-root → P-scanner` (bytes in) | TOCTOU-safe open at TB-adm |
| Flow | `DF-stage` — `P-seam → DS-staging` (transactional write-through) | no partial write |
| Flow | `DF-enqueue` — `P-seam → P-queue` (post-commit best-effort) | orphan sweep re-enqueues (IPS-4) |
| Flow | `DF-claim` — `P-queue → P-harness` (at-least-once claim) | single live claimant; lease expiry re-delivers |
| Flow | `DF-extract` — `P-harness → P-laneA`/`P-laneB` (bytes in, content out) | crosses TB-sbx; egress-denied both lanes |
| Flow | `DF-canon` — `P-harness → DS-canonical` (transactional per row) | update-with-audit; lineage-addressed id |
| Flow | `DF-edge` — `P-harness → DS-edges` **via the ES-6 declared-writer path** | `origin = declared`, transactional with the entity; harness holds no edge-write capability (IPS-6) |
| Flow | `DF-telemetry` — `P-scanner`/`P-harness → DS-runrec` (redacted) | derived IDs only; paths hashed (R-0110-c) |

**Attack surface → owning control:** extraction outbound fetch / XXE (`DF-extract` structural egress-deny + XXE-safe-by-omission parsers — R-0105-a/-e); resource exhaustion (`DF-read`/`DF-extract` size cap + decompression counters + per-lane budgets — R-0115); path traversal / symlink escape (`DF-read` open-time containment + `S_ISREG`-only — R-0106-c); silent write-through on a watched root (`TB-adm` admin-attested registration + attested `trust_class` — R-0099-c/-d); adversarial trust flip (`DF-edge`/`DS-edges` trust-predicate keying; `extracted` never enters a trust predicate — R-0108-c); standing-service authority (`P-scanner`/`P-harness` least-authority system principal, no edge-write capability — R-0106-b/R-0107); dedup poisoning (`DS-staging` path-scoped BLAKE3 identity + observable no-ops — R-0109); cross-workspace write (`DF-stage`/`DF-canon` single construction-site invariant + negative test — R-0107-b); quarantine as attacker-controlled unbounded write (aggregate bound + metadata-only inspection — R-0112-d); ingested-content-as-injection-to-downstream-agents (named, deferred to the D3 admission hook — R-0100-c). Residual risks are the Frame §5 set, accepted at single-operator dogfood scope with named re-open conditions.

### Consequences

**Good:**
- Raw = truth is durable and single-transaction (staging row + `raw_bytes` commit together), which is what makes SC9 working-state→working-state and SC10 no-partial-write cheap to guarantee.
- The subsystem conforms to locked corpus canon rather than amending it: ES-6's roster stays two, PE-7's origin semantics stay orthogonal to trust, P-0017's four-shape taxonomy is untouched (operational tables sit outside it).
- Every security-relevant guarantee is mechanism-carried (egress-deny capability/enclosure; transactional no-partial-write; schema-constraint dedup; stored-field trust demotion), so a regression is structurally visible rather than convention-dependent.
- The delivery guarantee lives on staged-durability-plus-reconciliation (the orphan sweep), so the `JobQueue` port keeps its portability (no SQL-transactional coupling) and the two-adapter test holds.

**Bad / Trade-offs:**
- Lane B's sandbox strength varies by OS where Lane A's is uniform, and a host without the enclosure loses Lane-B formats to `unextractable-here` — an availability cost accepted to keep egress-deny a verified property, never a convention.
- The lineage-addressed canonical-id scheme requires a transform to assign a stable `output_key` per logical output — a harness-contract obligation a content-hash scheme would not need — paid to preserve citation stability under update-with-audit.
- In-substrate blobs put binary bytes in the substrate (TOAST), bounded by the size cap and deferred-externalizable on a measured threshold — a storage-footprint cost accepted for single-transaction durability and free backup/PITR at V0.1.

## Pros and Cons of the Options

### Residence: core host subsystem (chosen)
- Pro: foundational substrate (staging, queue, standing service) must exist before any transform consumer and binds at host startup; no plugin ABI change; the [P-0013](P-0013-plugin-invocation-model.md) domain-verb trip-wire stays unfired.
- Con: the host binary grows a standing-service surface — contained by privilege scope (R-0106) rather than a process boundary at V0.1.

### Staging durability: logged (chosen)
- Pro: raw survives crash recovery — the precondition for ELT's re-derivation source-of-record.
- Con: none at the ELT lock; `UNLOGGED` stays cache-only in this system.

### Staging shape: one superset table (chosen)
- Pro: one vocabulary, one write path; per-kind variance as data (ES-1 precedent).
- Con: a wide table with a payload XOR and nullable per-kind columns — the price of no per-kind proliferation.

### Blob home: in-substrate (chosen)
- Pro: staging row + payload commit in one transaction (no fs/db two-phase residue); backup/PITR over blobs for free on the single self-hosted binary.
- Con: substrate storage footprint — bounded by the size cap, externalization deferred on a measured threshold.

### Re-derivation: update-with-audit (chosen)
- Pro: stable artifact identity + citation stability (PE-10); history is audit, not identity.
- Con: an update path with system-versioned history rather than append-only inserts — the write-side logic a new-rows scheme would not carry, paid to keep identity stable.

## More Information

- Binding requirement text: [spec R-0099–R-0115](../../specs/2026-07-16-ingestion-pipeline.md).
- Sibling ADRs in this Stage-3 package: [P-0025](P-0025-job-queue.md) (`{{P-job-queue}}`); the `{{P-0015-A1-ingest-trust-class}}` amendment ([P-0015](P-0015-provenance-envelope-source-roles.md) § Amendment) and the `{{P-0009-A1-ingest-control-plane}}` amendment ([P-0009](P-0009-rls-admin-token.md) § Amendment).
- Conformed-to canon: [P-0016](P-0016-edge-schema.md) ES-2/ES-3/ES-4/ES-6 (the edge writer partition ingest calls, never extends); [P-0015](P-0015-provenance-envelope-source-roles.md) PE-2/PE-3/PE-7/PE-10/PE-11 (provenance envelope, system principal, trust axis, citation stability); [P-0017](P-0017-storage-cluster-model.md) D-SoT (canonical lineage; operational tables outside the four-shape taxonomy); [P-0018](P-0018-core-entity-manifest.md) D-ACTOR (`system` actor); [P-0009](P-0009-rls-admin-token.md)/[P-0006](P-0006-v0-tenant-enforcement.md) (the write path); [P-0007](P-0007-plugin-resource-limits.md)/[P-0012](P-0012-plugin-runtime-and-mcp-sdk.md)/[P-0019](P-0019-plugin-contract.md) (the sandbox substrate).
- Frame: [`docs/intent/ingestion-pipeline-frame.md`](../../intent/ingestion-pipeline-frame.md) (blob `f56b3685`), directions IP-1..IP-16; §9 open ADR slots; §15 escalated items (item 3 = the conforming ES-6 edge-writer shape, gate-ratified; the ES-6-amendment alternative declined).
- Placeholder ledger: [`placeholder-resolution.md`](placeholder-resolution.md) updates `{{P-ingest-pipeline-shape}}` → P-0024 when this lands.
