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

This document is an ADR, an Architecture Decision Record. It holds the context that prompted one decision, the decision itself, the alternatives that lost, and the consequences that follow. It was authored at Stage 3 of the ingestion-pipeline cluster. Stage 3 is Spec, the last of three work-shaping stages (Intake, then Frame, then Spec), where agents turn a locked frame document into a testable spec. This ADR resolves the Frame's `{{P-ingest-pipeline-shape}}` slot, which is the architecture-overview's ADR-16 slot materialized, per [placeholder-resolution](placeholder-resolution.md).

It moves to `accepted` at the ingestion-pipeline **spec-exit gate**, reviewed alongside the spec [2026-07-16-ingestion-pipeline](../../specs/2026-07-16-ingestion-pipeline.md). That follows the precedent set by the retrieval, reporting, and coordination clusters, where companion ADRs move to `accepted` at the spec-exit gate rather than at authoring time. The subsystem's directions were already locked earlier, at the Frame-exit gate on 2026-07-17. This ADR renders those locked directions at ADR precision and makes **no** fresh decisions.

Binding requirement text is single-sourced to the spec's R-IDs, R-0099 through R-0115. An R-code is a stable identifier for a numbered requirement whose text, constraints, and rationale are defined in the document that owns it; the code carries no meaning on its own. What this ADR carries instead is the shape, the rationale chain, and four renderings the Frame routed specifically here. Those four are the typed data-flow diagram, the ES-6 declared-writer invocation, the staging→jobs orphan-sweep predicate, and the lineage-addressed canonical-id scheme.

## Context and Problem Statement

After `0.14.0`'s one-shot corpus migration, content keeps arriving and nothing ingests it durably or automatically. That leaves in place exactly the per-session re-derivation cost the product exists to kill. The architecture-overview's forward design had already named an ingest subsystem ("ELT shape, external content only… NEW ADR needed") and reserved an ADR-16 slot for its staging schema, its transform-as-job conventions, its re-derivation policy, and its per-source-kind retention rules.

This ADR is that slot, materialized. It fixes the shape of a core host subsystem: a source registry, reconciliation-first arrival, a shared loader seam that two drivers ride, durable raw staging, a transform harness on a minimal `JobQueue` port, egress-denied binary extraction (#324), recency instrumentation, and the quarantine and failure path. Two rulings from the maintainer bound that shape, and both are LOCKED. First, ELT over ETL: raw is truth, and derivation is a re-runnable stage. Second, the loader-seam share: one seam, two drivers.

The shape is further constrained by locked corpus canon it must conform to rather than amend. Each of the P-NNNN documents cited below is a project-scoped ADR living in this repository under `docs/src/adrs/`, as distinct from workspace-wide decisions that projects inherit. [P-0016](P-0016-edge-schema.md) ES-6 partitions edge writes across exactly two writers, and ingest writes no edge rows of its own. [P-0015](P-0015-provenance-envelope-source-roles.md) sets the provenance envelope and the trust axis; its fired PE-7 and PE-11 trip-wires are dispositioned in the `{{P-0015-A1-ingest-trust-class}}` amendment. [P-0017](P-0017-storage-cluster-model.md) D-SoT holds that canonical rows are content with `derived_from` lineage, and that operational tables sit outside the four-shape taxonomy. [P-0009](P-0009-rls-admin-token.md) and [P-0006](P-0006-v0-tenant-enforcement.md) govern the write path.

## Decision Drivers

- **ELT: raw = truth, derived = disposable** (intake Hard constraint 2, LOCKED, from the strangler-migration walk on 2026-07-06): staging is the re-derivation source-of-record and must be durable. Derivations re-run over it with zero re-fetch.
- **One seam, two drivers** (intake Hard constraint 1, LOCKED): the loader seam is the intrinsic contract this bundle exists to lock, and driver differences confine to drivers. Two principles apply here. P-LockContract says lock the contract and vary the implementation, so callers keep working while implementations swap. P-MinBlastRadius says a change reaches only as far as the architecture allows, and a change that forces many files to move in lock-step is the architecture reporting structural debt.
- **Conform to locked corpus canon, do not amend it** (P-VerifyInheritedState, meaning inherited state is checked against its source rather than assumed; ratified at the Frame-exit gate on 2026-07-17): ingest writes no edge rows and reaches the edge table only through ES-6's sanctioned declared-writer path. The trust demotion re-keys onto the stored `trust_class` at the trust predicates, never by relabeling `origin`.
- **Security guarantees are mechanism-carried, never convention-carried** (P-GuaranteeByMechanism, meaning a guarantee holds because a mechanism enforces it, not because callers agree to behave). Egress-deny is a capability or enclosure property. No-partial-write is transactional. Dedup is a schema constraint. Trust demotion is a stored-field rule.
- **Instrument in the same increment** (P-InstrumentBefore, meaning every production surface ships instrumented before launch rather than after the first incident; the Observability-versus-Simplicity tiebreak defaults to Observability): the subsystem is a production surface at first run, so it ships with recency, reconciliation, quarantine, and backlog measures already in place.

## Considered Options

Below are the shape decisions this ADR renders, each paired with the alternative the Frame recorded and decided against. One question is missing from the list on purpose. How declared frontmatter relations reach the edge table was decided at the Frame-exit gate, and its losing arm would have amended [P-0016](P-0016-edge-schema.md) rather than this ADR. It's therefore recorded as settled in [§ IPS-6](#ips-6--the-ingest-system-principal-and-the-es-6-declared-writer-invocation), not enumerated as an option here.

1. **Residence — core host subsystem** (chosen) vs a plugin (WASM host-fn surface for fs-watch would need designing; disqualifying at this tier) vs a separate ingest-worker binary (a second deployable; deferred behind the residence-move trip-wire).
2. **Staging durability — logged** (chosen) vs `UNLOGGED` (the pre-ADR overview's note, falsified by the ELT ruling — a raw store Postgres truncates on crash cannot be the ELT source-of-record).
3. **Staging shape — one superset table** (chosen; per-kind variance as data, the [P-0016](P-0016-edge-schema.md) ES-1 precedent) vs per-source-kind table proliferation.
4. **Blob home — in-substrate `raw_bytes` (TOAST-backed)** (chosen; the staging row and its payload commit in one transaction, backup/PITR for free) vs a filesystem blob store (fs/db two-phase residue, which SC9/SC10 would have to reason about).
5. **Re-derivation policy — update-with-audit** (chosen; overview open question 8 dispositioned) vs new-rows-append (would mint a new artifact identity per derive-run, breaking [P-0015](P-0015-provenance-envelope-source-roles.md) PE-10 citation stability).

## Decision Outcome

The outcome is rendered as IPS-1 through IPS-11. Binding requirement text lives in the spec (R-0099 through R-0115). What follows is the shape plus the four renderings the Frame routed here.

### IPS-1 — Residence: core host subsystem; zero new MCP tools; registration is admin CLI control-plane

The subsystem is host code inside the mnemra-core host process, sitting beside the retrieval and coordination subsystems. That placement comes from the [P-0002](P-0002-core-plugin-partition.md) verb-on-content walk: arrival detection, staging, and job execution are operational substrate rather than CRUD on content. The subsystem *produces* content rows; its own machinery is not content.

Nothing exports through the plugin ABI, so [P-0005](P-0005-v0-signing-chain.md)'s plugin-load surface is unchanged, vacuously, by residence alone. Zero agent-visible MCP tools ship. The verb budget's reserved slot ([P-0022](P-0022-coordination-cluster.md) D-VERBS) stays untouched, and ingested content is served through the existing retrieval verbs once indexed. Source register, list, and retire, along with quarantine list and inspect, are admin-scoped CLI control-plane operations in the [P-0009](P-0009-rls-admin-token.md) admin family. The matrix rows for them are added by `{{P-0009-A1-ingest-control-plane}}`, this ADR's sibling amendment. *(Spec: R-0099.)*

### IPS-2 — The loader seam: a typed stage chain; the admission chain is the D3 insertion point

The seam named in intake Hard constraint 1 is a typed stage chain with one contract: **bytes plus source-metadata in, a durably-staged raw artifact with [P-0015](P-0015-provenance-envelope-source-roles.md)-envelope provenance out, and a transform job enqueued**. The ordered stages are `parse/normalize → frontmatter-map → provenance-stamp → admission chain → write-through`.

`write-through` is a transactional staging insert. The transform-job enqueue follows it **post-commit, best-effort**. The durable staged row is the guarantee, and IPS-4's orphan sweep re-enqueues any staged-but-untransformed row, so the `JobQueue` port carries no SQL-transactional coupling.

The **admission chain is the D3 classify-at-write insertion point**. A classification stage inserts itself by registering one more admission stage against the typed contract `(candidate artifact + provenance) → admit | quarantine(reason) | annotate(policy fields)`, where `annotate` writes only existing PE-2 policy columns. No schema change, no path reshape. The stage itself stays undesigned (Frame Non-goal 7; Tier-2-gated per D3).

Driver differences stay in drivers. The `0.14.0` migrator owns kill-and-resume, byte-equivalence, and source-read-only. The `1.2.0` watcher owns arrival detection, dedup, and recency. *(Spec: R-0100.)*

### IPS-3 — Durable raw staging: logged, one superset table, in-substrate blob, per-source-kind retention

Staging uses **ordinary logged tables and never `UNLOGGED`**, because raw is truth. It is **one superset table** ([spec § Data Model](../../specs/2026-07-16-ingestion-pipeline.md#data-model)). Per-kind variance is carried as data (registry fields plus `source_kind` plus payload polarity) rather than as schema forks, following the [P-0016](P-0016-edge-schema.md) ES-1 precedent. The payload is `raw_text | raw_bytes` with exactly one non-null, enforced by a CHECK constraint.

Retained binaries live in-substrate as `raw_bytes`, TOAST-backed, bounded by the per-file size cap, so the staging row and its payload commit in one transaction. Retention is a per-source-kind closed enum, `{long-lived, transient}`, and every V0.1 kind is `long-lived`. SC3 requires re-derivation to work with the arrival source absent, which means the filesystem copy is never the raw. [P-0018](P-0018-core-entity-manifest.md)'s note that blob handling "is owned by the storage/ingest layer" gets its V0.1 answer here: the staging raw store is the blob substrate. *(Spec: R-0102.)*

### IPS-4 — Arrival: reconciliation-first; three joins; the staging→jobs orphan-sweep and staging↔dead-letter predicates

The **periodic and startup reconciliation scan is the primary arrival mechanism**. Filesystem events only accelerate latency, so a missed event costs latency and never correctness (P-GuaranteeByMechanism). Startup reconciliation runs the same scan, which means the crash-recovery path executes on every startup and can't silently rot. That's P-TrustworthySignal: a signal only counts if something exercises it.

The size and mtime pre-filter is a fast-path heuristic, bounded by a periodic full-content-rehash cadence. A size-preserving edit with a reset mtime is caught within that window. The rehash pass is I/O-paced so it doesn't contend with the `arrival→staged` critical path.

The scan reconciles **three joins**:

1. **filesystem → staging** — observed `(path, size/mtime, content hash on candidates)` against staging identity (IPS-8); an unstaged or changed file is staged.
2. **staging → jobs (the orphan sweep)** — a staging row whose post-commit best-effort enqueue (IPS-2) never landed a transform job is re-enqueued. The orphan predicate, SQL-expressible (the [FIV-1 fold](../../intent/ingestion-pipeline-frame.md) obligation, rendered here):

   > A `staging` row is a **transform orphan** when `status = 'staged'` **and** `staged_at < now() − enqueue_orphan_window` **and** no `jobs` row exists with `kind = 'transform'`, `payload ->> 'staging_id' = staging.id`, and `state ∈ {'pending','claimed','completed','dead_letter'}`. Each orphan is re-enqueued (one transform job per orphan row); a staged row that already has a live, completed, **or dead-lettered** transform job is never re-enqueued.

   The `enqueue_orphan_window` defaults to 2× the scan interval ([spec § Numeric calibrations](../../specs/2026-07-16-ingestion-pipeline.md#numeric-calibrations)), wide enough that a normally-enqueued job is never falsely swept. `dead_letter` sits **inside** the guard set so a poison-message row that has already dead-lettered is never re-enqueued into a fresh retry cycle. The crash-window that leaves behind is closed by join 3, not by re-enqueue. This is the mechanism behind the no-partial-write and delivery guarantee in IPS-10: staged-durability plus reconciliation, not a cross-resource staging-and-enqueue transaction.
3. **staging ↔ dead-letter (interrupted terminal-failure completion)** — the R-0104-f staging-quarantine write and the job's terminal `dead_letter` transition are **separate resource writes** with no transactional coupling, because JQ-3 forbids it. A crash between them leaves a `staged` row whose only job is `dead_letter`. Such a row is **not** an orphan to re-enqueue. It's an interrupted terminal-failure to **complete**, idempotently, to its designed terminal state:

   > A `staging` row is an **interrupted terminal-failure** when `status = 'staged'` **and** a `jobs` row exists with `kind = 'transform'`, `payload ->> 'staging_id' = staging.id`, `state = 'dead_letter'`, **and** no `jobs` row for it exists in `state ∈ {'pending','claimed','completed'}`. The scan drives it to `status = 'quarantined'`, `rejection_class = 'transform-failure'` — idempotently (a row already `quarantined` is a no-op; no canonical row is written).

   This closes the poison-loop gap that Bolt B1 and Warden M2 both identified in the R-0112-c working-state → working-state guarantee. It closes it with the same one-scan reconciliation mechanism as joins 1 and 2. The rejected alternative was same-transaction atomicity of the `fail()` and quarantine writes, which would put SQL-transactional coupling in the port and break on an SQS or Temporal adapter (JQ-3). *(Spec: R-0101, R-0104-f, R-0112-b/-c.)*

### IPS-5 — Transform-harness conventions: transform-as-job, update-with-audit, lineage, the lineage-addressed canonical-id scheme

A transform is a registered, versioned unit `(source_kind, transform_id, version, declared_deterministic: bool)` with the signature `staged raw → canonical candidate rows`. It holds no capability beyond bytes-in and content-out. Extraction transforms additionally run sandboxed, per IPS-7.

Outputs land as ordinary content rows ([P-0001](P-0001-storage-layout.md) C1), each carrying a `derived_from` soft ref back to the staging row ([P-0017](P-0017-storage-cluster-model.md) D-SoT). Re-derivation policy is **update-with-audit**: a re-run updates the existing canonical row at the same identity, and the [P-0001](P-0001-storage-layout.md) system-versioned history pattern is the audit trail. Re-run is first-class, so deleting derived rows and re-running repopulates from staging alone with no arrival-source read. The harness requires **idempotent convergence**, not byte-identity. Per-transform determinism is a declared, validator-backed property (`declared_deterministic`), with a seeded byte-compare where it's `true`.

**The lineage-addressed canonical-id scheme (the one-to-many discriminator, rendered here):**

> The canonical id is a **deterministic function of `(workspace_id, source_id, source_path, transform_id, output_key)`**. That's the IPS-8 staging identity **minus its `content_hash`**, plus the transform identity, plus a transform-assigned **`output_key`** that discriminates each logical output where **one transform emits many canonical rows** (a PDF or DOCX extracting into several artifacts). One staging lineage therefore maps to **N** canonical ids, one per `output_key`.
>
> `content_hash` and `transform_version` are **NOT** part of the identity. Including `content_hash` would mint a new identity on every content change, contradicting update-with-audit and breaking [P-0015](P-0015-provenance-envelope-source-roles.md) PE-10 citation stability. `transform_version` belongs to the audit history, not the identity. The `output_key` is stable per logical output (a transform assigns the same `output_key` to the same logical output across re-runs), so each id is **stable across content-changing re-derivations** and **reconstructable from staging alone** after delete-derived (SC3). That holds even for a `declared_deterministic: false` transform, whose *content* may vary while its *identity set* does not.

**Retire-on-shrink (the one-to-many update-with-audit completion, rendered here):** update-with-audit updates the surviving ids in place. But a re-run must also **retire** each canonical row whose `output_key` is present in the prior set and **absent from the new set**, meaning the non-empty set-difference `prior − new`. That retirement happens **regardless of whether the new run also adds `output_key`s the prior set lacked**. So a **drop-and-add** re-run (prior `{a,b,c}` becoming new `{a,b,d}`, which is not a subset) retires `c` and mints `d`. Without it, the vanished output's stale canonical row would serve indefinitely, and only the full delete-derived and re-run path would drop it by reconstructing the id set from staging alone.

An empty new set is disambiguated by the transform's **success or failure signal, not by the output count**. The retire diff is computed only on the success path. A re-run that *completes* while emitting nothing retires **all** prior rows for the lineage. A transform **failure** instead diverts to R-0104-f quarantine (`transform-failure`) before any retire is computed, and retires nothing.

Retirement is a **soft-delete or mark-superseded through the [P-0001](P-0001-storage-layout.md) system-versioned history**, in the **same re-derivation transaction** that updates the survivors. It's never a hard delete and never a re-minted identity, so PE-10 citation stability holds for the retired id: a citation resolves to a superseded-terminal rather than a dangling reference. *(Spec: R-0104-g.)*

This is the reading under which [P-0015](P-0015-provenance-envelope-source-roles.md) **PE-10's** "content-addressed, per the V0 substrate" citation wording and [P-0001](P-0001-storage-layout.md)'s system-versioned history point the same way. PE-10 citation stability is preserved by the stable-id-with-history model, not by content-hash addressing of canonical rows. *(Spec: R-0104.)*

### IPS-6 — The ingest system principal and the ES-6 declared-writer invocation

All ingest-path writes run under a distinguished **system principal naming the ingest subsystem**, following the [P-0015](P-0015-provenance-envelope-source-roles.md) PE-3 pattern. It's not a workspace role and not a synthetic admin token. The principal is bound to a concrete `workspace_id` taken from the source registration row, at a single [P-0006](P-0006-v0-tenant-enforcement.md) context-construction site.

That single site matters because the principal is cross-workspace-write-capable by design. The construction site is the audited chokepoint, and the invariant is that *the principal cannot commit a row whose `workspace_id` differs from its source's registered workspace*. The spec covers it with a negative test, R-0107-b. `created_by` resolves to the `system` actor ([P-0018](P-0018-core-entity-manifest.md) D-ACTOR, "host-side extractor, scheduled job").

**The ES-6 declared-writer invocation (the harness as a new caller, rendered here):**

> The loader and harness hold **no edge-write capability of their own**. No code path in the ingest subsystem writes an edge row directly. A relation declared in ingested frontmatter reaches the edge table **only** through [P-0016](P-0016-edge-schema.md) **ES-6's sanctioned declared-writer path**, the typed, entity-transactional content write path that the `repos`-plugin CRUD family drives (writer 1 of ES-6's two-writer partition, `origin ∈ {declared, system}`). At the transform's canonical entity write, for each declared frontmatter relation, the harness **invokes that existing path**, passing the [P-0006](P-0006-v0-tenant-enforcement.md) `WorkspaceCtx` (the ingest system principal) and the relation as `origin = declared`. The declared edge is written **transactionally with its entity** by the path ES-6 already sanctions.
>
> The harness is a **new caller** of writer 1's path, **not a third writer**. ES-6's writer roster stays exactly two: the `repos`-plugin CRUD path for `origin ∈ {declared, system}`, and the host extractor confined to `origin = extracted`. ES-2 and ES-3's origin and `source_span`-iff-`extracted` semantics hold untouched, and ES-4's extractor-integrity rule (the extractor never mints `declared` or `system` authority) is unaffected. The two-writer split is extractor-confinement, not a per-subsystem pattern. The harness's declared edges fall inside writer 1's charter ("declared relations transactional with their entities"), so the harness needs no writer of its own. The trust boundary for external-class sources is handled entirely by the trust-predicate keying in `{{P-0015-A1-ingest-trust-class}}` (IPS-9), never by an edge-writer change.

This conforming shape was **ratified at the Frame-exit gate (2026-07-17; Frame §15 item 3)**, and it's rendered here as settled. *(Spec: R-0107.)*

### IPS-7 — Extraction sandbox: egress-denied by construction; per-format hybrid lanes; fail-closed terminal

Binary extraction runs egress-denied, across two lanes.

**Lane A** (the default) is a `wasm32-wasip2` component ([P-0019](P-0019-plugin-contract.md) D4) running on the host Wasmtime engine ([P-0012](P-0012-plugin-runtime-and-mcp-sdk.md) Decision A). Its world imports no network and no filesystem, so egress is structurally impossible. Resource bounds ride [P-0007](P-0007-plugin-resource-limits.md) mechanisms (fuel, epoch, memory, kill-and-replace) at **extraction-calibrated** numbers. P-0007's plugin values are not inherited.

**Lane B** (native, for crates that can't run in WASM) runs a sandboxed subprocess inside a structural egress-denying enclosure: Linux netns, seccomp, and landlock; on macOS, a container boundary. Lane B is a **legal lane on a host only if its enclosure is verifiably present**, which a startup and registration lane-availability check confirms.

The **lane-selection terminal is fail-closed**. A format runs Lane A only if its crate compiles to `wasm32-wasip2` and performs within budget, verified by an implementation-start build-and-perf spike. A failing spike moves the format to Lane B, never to an unsandboxed path. A format that clears **no** lane whose egress-deny precondition holds on the running host quarantines as `unextractable-here`. There is no unsandboxed fallback anywhere in the selection function.

On XXE specifically: the XML parser families in every viable crate perform no entity resolution at all, so XXE is absent by construction, and the sandbox denies egress regardless. Defense in depth. The dogfood consequence is that PPTX, and DOCX if its spike fails, are unavailable on a native macOS host until a container boundary is provided. That's deployment posture accepted at the Frame-exit gate (Frame §15 item 4), not a design change. *(Spec: R-0105, R-0115.)*

### IPS-8 — Content identity + dedup: BLAKE3, path-scoped, observable no-ops

Staging identity is `(workspace_id, source_id, source_path, content_hash)`, where `content_hash = BLAKE3(raw payload)`. BLAKE3's 256-bit collision resistance is its own documented design property and is the direct security ground of this identity boundary. [P-0008](P-0008-admin-token-shape.md) is the in-corpus algorithm-choice precedent only, nothing more.

Identical re-arrival is a no-op, recorded as an arrival event, so suppression is never silent. Changed content at an existing path produces a new staging row. Equal content at a different path is a distinct row, and that path-scoping is what bounds dedup poisoning. Cross-source global dedup is deliberately not performed at V0.1, on integrity-first grounds. *(Spec: R-0109.)*

### IPS-9 — Trust-class mapping for the new source kinds

Per-source `trust_class` is a two-value enum, `{first-party, external}`, admin-attested at registration, with anything undecidable resolving to `external`. It's consumed mechanically at provenance-stamp time.

`first-party` frontmatter keeps its `declared` authority. `external`-class frontmatter relations still record honest `origin = declared` edges via the IPS-6 sanctioned path, but the **trust predicates are keyed on the stored `trust_class`**. A `declared` trust-affecting edge enters PE-7's `outdated` and hard-supersession predicate only when its declaring artifact's trust class is `first-party` (`{{P-0015-A1-ingest-trust-class}}`). Demotion is therefore by trust-predicate keying, never by relabeling `origin`.

When such an external-class `declared` trust-affecting edge is held out of a trust predicate, the **curatorial signal that records the withholding is a defined mechanism, not an assertion** (R-0108-f). Two things fire: a monotonic `curatorial_signal_count` on the ingest run and job records, **and** a redacted named-level log line at edge-record time. That's P-TrustworthySignal in practice, following the [P-0015](P-0015-provenance-envelope-source-roles.md) operator-side-signal precedent. The withholding is operator-visible, never silent.

The fired PE-7 (N10) and PE-11 (ingest-half) trip-wires are dispositioned in that amendment. PE-11's adopter-deployment half stays live on its own instrument. *(Spec: R-0108, R-0108-f; ADR: `{{P-0015-A1-ingest-trust-class}}` in [P-0015](P-0015-provenance-envelope-source-roles.md).)*

### IPS-10 — Recency, load-shedding, and the failure path

Two ingest-owned recency spans, `arrival→staged` and `staged→canonical-committed`, are emitted per source kind on plain workspace-scoped run and job records (the RA-7 family; [P-0010](P-0010-storage-substrate-engine.md) D8). The composite `arrival→indexed` span is computed by joining against the retrieval cluster's index-build records rather than by coupling the two subsystems, and both sides are backfillable (IB1). Emission is redacted: derived IDs only, source paths hashed, no frontmatter bodies or titles.

Load-shedding puts the staging insert on the protected critical path and keeps enqueue and transform execution off it. **Queue depth is the unbounded, measured absorption buffer, but transform execution concurrency is bounded** by a fixed-size **host-owned transform-worker pool** (R-0111-b). That pool is not P-0007's plugin pool, and it's distinct from P-0007's per-instance kill-and-replace inside a single Lane-A extraction. A burst therefore absorbs into queue depth rather than into an unbounded number of simultaneously-executing extraction instances. The aggregate execution-resource ceiling under any burst is `worker_pool_size × per-lane budget`, which is what makes QA-7 falsifiable.

The failure path is fail-closed over a **closed rejection vocabulary**: `malformed-file`, `unparseable-binary`, `oversized-input`, `decompression-bomb`, `path-traversal-name`, `unregistered-source`, `transform-failure`, `unextractable-here`. There's no partial canonical write, because the canonical write is transactional per staging row. Every operation moves working-state → working-state. Quarantine is inert, metadata-only-inspectable, and aggregate-bounded. *(Spec: R-0110, R-0111, R-0112, R-0115.)*

### IPS-11 — Reliability posture

No numeric availability or recovery target is adopted at V0.1. Contractual recovery behavior binds instead: SC11 reconcile-on-restart and SC9 working-state → working-state, both tested-binary. The recency metric is the calibration baseline for the future numeric target (P-InstrumentBefore). The deferred numeric target fires on a recency or run-record breach of the spec bound, or on the first adopter deployment. *(Spec: R-0113.)*

## Typed DFD

This is the formal typed data-flow diagram the Stage-3 obligations require, and Frame §9 routes the artifact here so a downstream threat-model refresh has typed anchors to work against.

**Trust boundaries:** (TB-ws) the workspace boundary, where everything below sits inside it except the external entities, which cross it; (TB-adm) the admission-control boundary, where nothing outside a registered source is ever staged; (TB-sbx) the extraction-sandbox egress-denying enclosure, meaning Lane A's capability world and Lane B's verified per-host enclosure.

| Kind | Element | Notes / owning control |
|---|---|---|
| External entity | `EE-watched-root`: the watched-root filesystem (partially-untrusted content) | crosses TB-ws/TB-adm; TOCTOU-safe open, `S_ISREG`-only (R-0106-c) |
| External entity | `EE-operator`: the admin operator (CLI control-plane) | crosses TB-ws; admin-token auth, [P-0009](P-0009-rls-admin-token.md) + `{{P-0009-A1-ingest-control-plane}}` (R-0099-c) |
| Process | `P-scanner`: reconciliation scanner / watcher (supervised task) | least authority, read-only roots (R-0106-b) |
| Process | `P-seam`: loader seam (`parse → frontmatter-map → provenance-stamp → admission chain → write-through`) | typed stage chain; D3 insertion point (R-0100) |
| Process | `P-harness`: transform harness / workers | idempotent handlers; canonical write transactional per row (R-0104) |
| Process | `P-laneA`: WASM extraction component | no network or fs imports, so egress is structurally impossible (R-0105-a) |
| Process | `P-laneB`: sandboxed subprocess extractor | enclosure verified per host; fail-closed terminal (R-0105-b/-c) |
| Process | `P-queue`: `JobQueue` port + Postgres SKIP LOCKED adapter | at-least-once, single-claimant (R-0103; [P-0025](P-0025-job-queue.md)) |
| Data store | `DS-sources`: source registry (root, kind, `trust_class`, retention) | admin-attested; allow-list-validated (R-0099-d) |
| Data store | `DS-staging`: durable raw staging (logged; `raw_bytes` TOAST) | BLAKE3 path-scoped identity; `UNLOGGED` forbidden (R-0102/R-0109) |
| Data store | `DS-jobs`: the queue table | UUID v7; closed state enum |
| Data store | `DS-canonical`: canonical content rows (`derived_from` lineage) | ordinary content ([P-0001](P-0001-storage-layout.md) C1); update-with-audit (R-0104) |
| Data store | `DS-runrec`: ingest run/recency records | RA-7 family; redacted emission (R-0110) |
| Data store | `DS-edges`: the edge table (**written only via the ES-6 sanctioned path, not owned by ingest**) | [P-0016](P-0016-edge-schema.md) ES-6; harness is a caller, roster unchanged (IPS-6) |
| Flow | `DF-read`: `EE-watched-root → P-scanner` (bytes in) | TOCTOU-safe open at TB-adm |
| Flow | `DF-stage`: `P-seam → DS-staging` (transactional write-through) | no partial write |
| Flow | `DF-enqueue`: `P-seam → P-queue` (post-commit best-effort) | orphan sweep re-enqueues (IPS-4) |
| Flow | `DF-claim`: `P-queue → P-harness` (at-least-once claim) | single live claimant; lease expiry re-delivers |
| Flow | `DF-extract`: `P-harness → P-laneA`/`P-laneB` (bytes in, content out) | crosses TB-sbx; egress-denied both lanes |
| Flow | `DF-canon`: `P-harness → DS-canonical` (transactional per row) | update-with-audit; lineage-addressed id |
| Flow | `DF-edge`: `P-harness → DS-edges` **via the ES-6 declared-writer path** | `origin = declared`, transactional with the entity; harness holds no edge-write capability (IPS-6) |
| Flow | `DF-telemetry`: `P-scanner`/`P-harness → DS-runrec` (redacted) | derived IDs only; paths hashed (R-0110-c) |

**Attack surface → owning control.** Extraction outbound fetch and XXE are owned by `DF-extract`'s structural egress-deny plus XXE-safe-by-omission parsers (R-0105-a/-e). Resource exhaustion is owned by `DF-read` and `DF-extract` size caps, decompression counters, and per-lane budgets (R-0115). Path traversal and symlink escape are owned by `DF-read` open-time containment and `S_ISREG`-only (R-0106-c). Silent write-through on a watched root is owned by `TB-adm` admin-attested registration and attested `trust_class` (R-0099-c/-d). Adversarial trust flip is owned by `DF-edge` and `DS-edges` trust-predicate keying, where `extracted` never enters a trust predicate (R-0108-c). Standing-service authority is owned by the `P-scanner` and `P-harness` least-authority system principal with no edge-write capability (R-0106-b, R-0107). Dedup poisoning is owned by `DS-staging`'s path-scoped BLAKE3 identity and observable no-ops (R-0109). Cross-workspace write is owned by the `DF-stage` and `DF-canon` single construction-site invariant with a negative test (R-0107-b). Quarantine as attacker-controlled unbounded write is owned by the aggregate bound and metadata-only inspection (R-0112-d). Ingested-content-as-injection-to-downstream-agents is named and deferred to the D3 admission hook (R-0100-c). Residual risks are the Frame §5 set, accepted at single-operator dogfood scope with named re-open conditions.

### Consequences

**Good:**
- Raw = truth is durable and single-transaction (the staging row and `raw_bytes` commit together), which is what makes SC9 working-state→working-state and SC10 no-partial-write cheap to guarantee.
- The subsystem conforms to locked corpus canon rather than amending it. ES-6's roster stays two, PE-7's origin semantics stay orthogonal to trust, and P-0017's four-shape taxonomy is untouched because operational tables sit outside it.
- Every security-relevant guarantee is mechanism-carried: egress-deny by capability or enclosure, no-partial-write by transaction, dedup by schema constraint, trust demotion by stored field. A regression is therefore structurally visible rather than convention-dependent.
- The delivery guarantee lives on staged-durability plus reconciliation (the orphan sweep), so the `JobQueue` port keeps its portability with no SQL-transactional coupling, and the two-adapter test holds.

**Bad / Trade-offs:**
- Lane B's sandbox strength varies by OS where Lane A's is uniform, and a host without the enclosure loses Lane-B formats to `unextractable-here`. That's an availability cost accepted to keep egress-deny a verified property rather than a convention.
- The lineage-addressed canonical-id scheme requires a transform to assign a stable `output_key` per logical output. That's a harness-contract obligation a content-hash scheme wouldn't need, paid to preserve citation stability under update-with-audit.
- In-substrate blobs put binary bytes in the substrate (TOAST), bounded by the size cap and deferred-externalizable on a measured threshold. That storage-footprint cost buys single-transaction durability and free backup/PITR at V0.1.

## Pros and Cons of the Options

### Residence: core host subsystem (chosen)
- Pro: foundational substrate (staging, queue, standing service) must exist before any transform consumer and binds at host startup; no plugin ABI change; the [P-0013](P-0013-plugin-invocation-model.md) domain-verb trip-wire stays unfired.
- Con: the host binary grows a standing-service surface, contained by privilege scope (R-0106) rather than by a process boundary at V0.1.

### Staging durability: logged (chosen)
- Pro: raw survives crash recovery, which is the precondition for ELT's re-derivation source-of-record.
- Con: none at the ELT lock. `UNLOGGED` stays cache-only in this system.

### Staging shape: one superset table (chosen)
- Pro: one vocabulary, one write path, with per-kind variance carried as data (the ES-1 precedent).
- Con: a wide table with a payload XOR and nullable per-kind columns. That's the price of no per-kind proliferation.

### Blob home: in-substrate (chosen)
- Pro: the staging row and its payload commit in one transaction, with no fs/db two-phase residue, and backup/PITR over blobs comes free on the single self-hosted binary.
- Con: substrate storage footprint, bounded by the size cap, with externalization deferred on a measured threshold.

### Re-derivation: update-with-audit (chosen)
- Pro: stable artifact identity and citation stability (PE-10); history is audit, not identity.
- Con: an update path with system-versioned history rather than append-only inserts. That's write-side logic a new-rows scheme wouldn't carry, paid to keep identity stable.

## More Information

- Binding requirement text: [spec R-0099–R-0115](../../specs/2026-07-16-ingestion-pipeline.md).
- Sibling ADRs in this Stage-3 package: [P-0025](P-0025-job-queue.md) (`{{P-job-queue}}`); the `{{P-0015-A1-ingest-trust-class}}` amendment ([P-0015](P-0015-provenance-envelope-source-roles.md) § Amendment) and the `{{P-0009-A1-ingest-control-plane}}` amendment ([P-0009](P-0009-rls-admin-token.md) § Amendment).
- Conformed-to canon: [P-0016](P-0016-edge-schema.md) ES-2/ES-3/ES-4/ES-6 (the edge writer partition ingest calls, never extends); [P-0015](P-0015-provenance-envelope-source-roles.md) PE-2/PE-3/PE-7/PE-10/PE-11 (provenance envelope, system principal, trust axis, citation stability); [P-0017](P-0017-storage-cluster-model.md) D-SoT (canonical lineage; operational tables outside the four-shape taxonomy); [P-0018](P-0018-core-entity-manifest.md) D-ACTOR (the `system` actor); [P-0009](P-0009-rls-admin-token.md)/[P-0006](P-0006-v0-tenant-enforcement.md) (the write path); [P-0007](P-0007-plugin-resource-limits.md)/[P-0012](P-0012-plugin-runtime-and-mcp-sdk.md)/[P-0019](P-0019-plugin-contract.md) (the sandbox substrate).
- Frame: [`docs/intent/ingestion-pipeline-frame.md`](../../intent/ingestion-pipeline-frame.md) (blob `f56b3685`), directions IP-1..IP-16; §9 open ADR slots; §15 escalated items (item 3 is the conforming ES-6 edge-writer shape, gate-ratified, with the ES-6-amendment alternative declined).
- Placeholder ledger: [`placeholder-resolution.md`](placeholder-resolution.md) updates `{{P-ingest-pipeline-shape}}` → P-0024 when this lands.
