---
title: "P-0025: JobQueue Port + Postgres Adapter"
summary: "Resolves the ingestion-pipeline Frame's {{P-job-queue}} slot — a minimal JobQueue port with a Postgres SKIP LOCKED adapter, specified as part of the ingest harness (the harness is its first consumer, dogfood-driven). Locks the port's operation set (enqueue / claim / complete / fail-with-retry / dead-letter / scheduled delivery), its delivery semantics (at-least-once, single live claimant per job, lease/visibility window), the portability discipline (no LISTEN/NOTIFY and no SQL-transactional coupling in the port surface; polling is the portable baseline, a wake hint is an optional adapter capability), the post-commit best-effort enqueue plus reconciliation-re-enqueue delivery guarantee, and the design-time two-adapter conformance test. Queue tables are host-owned operational tables outside the P-0017 content taxonomy. Renders locked Frame content; makes no fresh decisions. Binding requirement text is single-sourced to spec R-0103."
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

# P-0025: JobQueue Port + Postgres Adapter

**Project:** mnemra-core

## Status

`proposed`

This ADR ([Architecture Decision Record](../glossary.md#adr): it captures a decision, the context behind it, rejected alternatives, and expected consequences) was authored at Stage 3 of the ingestion-pipeline cluster, the pipeline's [Spec stage](../glossary.md#spec), where agents synthesize a testable spec from an already-locked frame document. It resolves the [Frame](../glossary.md#frame)'s `{{P-job-queue}}` slot, a placeholder left open in the Stage 2 frame document to mark an architecture decision still pending. That placeholder was opened by Stage-2a decision 1, reached during Frame stage, per [placeholder-resolution](placeholder-resolution.md).

Status moves to `accepted` at the ingestion-pipeline **spec-exit gate**, the checkpoint that closes the Spec stage, reviewed alongside the spec, [2026-07-16-ingestion-pipeline](../../specs/2026-07-16-ingestion-pipeline.md), following the companion-ADR precedent.

The port itself was already locked at the **Frame-exit gate** (2026-07-17, Frame IP-5), the checkpoint that closes Frame stage. This ADR renders that decision at ADR precision. It makes **no** fresh decisions. Binding requirement text is single-sourced to spec R-0103, an [R-code](../glossary.md#r-codes): a stable identifier for a numbered requirement, whose full text and rationale live in the spec document that defines it.

## Context and Problem Statement

The ingest transform harness (defined in [P-0024](P-0024-ingest-pipeline-shape.md), a [project-scoped ADR](../glossary.md#p--adr), at requirement IPS-5) enqueues one transform job per staged row. Job workers claim jobs and apply the registered transform. That needs a **job queue**: a work-distribution primitive with at-least-once delivery and single-claimant semantics. The system doesn't have one yet, and no ADR covers it.

The architecture-overview's subsystem table already named a `JobQueue` row (row 5), with a `FOR UPDATE SKIP LOCKED` sketch, UUID v7 ids, JSONB payloads, and `scheduled_for` delivery. But it was left unbuilt. Stage-2a decision 1 (LOCKED) ratified building **a minimal port plus Postgres adapter as part of this bundle**, with the harness as its first consumer (dogfood-driven: the harness's own build becomes the queue's first real user) and the two-adapter test (a conformance check run against two independent adapters, detailed below in JQ-5) applied at drafting. The alternatives were specifying the queue in its own spec first, or defining a private job table now and extracting a generic subsystem later.

The queue is a distinct primitive from [P-0022](P-0022-coordination-cluster.md)'s **leases** (actor-held intent across time). A job claim is worker work-distribution, not intent-holding. The two share the substrate-atomicity posture and nothing else. That distinction is stated here to head off a future conflation between the two.

## Decision Drivers

- **The harness needs a queue that doesn't exist** (Frame IP-5; intake SC6, the criterion that a transform consumer writes against the harness's conventions without reading pipeline internals). The queue is the substrate under transform-as-job.
- **Contracts lock, implementations vary** ([P-LockContract](../glossary.md#p-lockcontract): lock the contract, vary the implementation; the two-adapter test as a design-time swap-readiness check; the [P-0010](P-0010-storage-substrate-engine.md) D5 `Storage`-trait pattern). The port's verbs and delivery semantics are the hard-to-change contract. The Postgres adapter is one implementation behind it.
- **At-least-once is only safe over idempotent handlers** (P-GuaranteeByMechanism: a guarantee has to be backed by the mechanism that actually enforces it, not just asserted). The ELT shape already requires idempotent, re-runnable transforms ([P-0024](P-0024-ingest-pipeline-shape.md) IPS-5), so at-least-once delivery composes with the harness contract without adding an exactly-once burden the substrate can't cheaply provide.
- **Minimal verb set, no premature mechanism** (Simplicity; [P-Defer](../glossary.md#p-defer): defer a mechanism choice until evidence forces it). No priority lanes, cron DSL, or workflow engine at V0.1. The queue is the smallest thing the harness needs.
- **Portability without a second consumer built now** ([P-PerRepoFirst](../glossary.md#p-perrepofirst) at subsystem scope: build per-repo first, extract shared abstractions only once real reuse shows up). The port is shaped so other subsystems can adopt it later without amendment. But no second consumer is designed now.

## Considered Options

1. **Minimal `JobQueue` port plus Postgres adapter, in this bundle, harness as first consumer** (chosen: Stage-2a decision 1, LOCKED).
2. **A standalone `JobQueue` spec first, this bundle depends on it.** Rejected at Stage 2a. It serializes the harness behind a queue spec with no second consumer to justify the separation. The dogfood consumer (the harness) is the right forcing function, and the two-adapter test at drafting already guards portability.
3. **A private job table in the harness now, with a generic subsystem extracted later** (rule-of-three deferral, per P-PerRepoFirst). Rejected at Stage 2a. The delivery semantics (at-least-once, single-claimant, lease/visibility, dead-letter) are the hard-to-change contract. Encoding them ad hoc in a private table forfeits the swap-readiness that the port plus two-adapter test buys, for the same drafting cost.

## Decision Outcome

**Chosen: Option 1.** A minimal `JobQueue` port with a Postgres `FOR UPDATE SKIP LOCKED` adapter, with the harness as its first consumer. Rendered below as JQ-1 through JQ-5. Binding requirement text: spec R-0103.

### JQ-1 — The port operation set

The port exposes the following, always expressed in delivery semantics and never in engine idioms:

- `enqueue(kind, payload, scheduled_for?)`: registers a job. A `kind` with no registered handler doesn't enqueue at all (validator-first, closed job-kind vocabulary).
- `claim(kinds, lease)`: atomically claims one available job of a listed kind, taking a lease, a visibility window during which no other claimant can take the same job.
- `complete(job)`: marks a claimed job done, the terminal `completed` state.
- `fail(job, retryable?)`: records a failed attempt, incrementing `attempts`. A **retryable** fail returns the job to `pending` for backoff re-delivery, the same as a lease expiry does, so there's no non-terminal `failed` resting state. A **non-retryable** fail, or an attempts-exhausted retry, moves the job to the terminal dead-letter state.
- Dead-letter disposition: a job whose retries are exhausted, or that hit a non-retryable failure, moves to a terminal `dead_letter` state. It isn't silently lost.
- `schedule`, via `scheduled_for`: delayed or future delivery.

Job identity is UUID v7. Payloads are JSONB. Job kinds are a code-registered closed vocabulary. *(Spec: R-0103-a.)*

### JQ-2 — Delivery semantics: at-least-once, single live claimant

Delivery is **at-least-once**, with **a single live claimant per job**. `claim` carries a lease (a visibility window) whose duration is a [spec § Numeric calibrations](../../specs/2026-07-16-ingestion-pipeline.md#numeric-calibrations) default, sized to **exceed the longest lane's wall-clock bound plus margin**. That sizing means a still-running job is never re-delivered to a second claimant while the first is still working. An expired claim re-delivers the job.

That consequence is bound directly into the harness contract: **handlers are idempotent** ([P-0024](P-0024-ingest-pipeline-shape.md) IPS-5). At-least-once delivery is only safe over idempotent transforms, and the ELT shape already requires those (re-run is first-class there). Exactly-once delivery is **not** provided. Providing it would push distributed-transaction machinery into the port that the substrate can't cheaply guarantee. Idempotent convergence at the handler is the deliberate alternative, grounded in the same guarantee-by-mechanism principle cited above. *(Spec: R-0103-b.)*

### JQ-3 — Portability discipline: no `LISTEN/NOTIFY`, no SQL-transactional coupling in the port

The port surface carries **no `LISTEN/NOTIFY`** and **no SQL-transactional coupling**. A low-latency wake hint is an optional *adapter* capability, not a port guarantee. **Polling is the portable baseline.** The port's verbs and semantics have to be satisfiable by an SQS-class or a Temporal-class adapter too, not just Postgres. That's what the two-adapter test (JQ-5) checks for, as a design-time swap-readiness check.

As a result, the enqueue from the loader seam is **post-commit, best-effort** ([P-0024](P-0024-ingest-pipeline-shape.md) IPS-2). A cross-resource staging-plus-enqueue transaction would put SQL-transactional coupling into the port surface, which would falsify the portability claim. The Postgres adapter **MAY** enqueue inside the staging transaction as an adapter-local optimization, but that's never a port guarantee. *(Spec: R-0103-c, R-0100-b.)*

### JQ-4 — The delivery guarantee: post-commit best-effort enqueue + reconciliation re-enqueue

Because the enqueue is post-commit best-effort (JQ-3), the delivery guarantee doesn't live on the enqueue call itself. It lives on **staged durability plus reconciliation**. The staging write is transactional and durable. A staging row whose enqueue never landed a transform job (a **transform orphan**) gets re-enqueued by the reconciliation scan's staging-to-jobs join, using the orphan predicate rendered in [P-0024](P-0024-ingest-pipeline-shape.md) IPS-4. Orphan latency is bounded by the scan cadence and by the `enqueue_orphan_window` ([spec § Numeric calibrations](../../specs/2026-07-16-ingestion-pipeline.md#numeric-calibrations)).

This is what lets the no-partial-write / delivery guarantee (Frame IP-2/IP-14) hold without SQL-transactional coupling in the port. *(Spec: R-0100-b, R-0101-d, R-0112-b.)*

### JQ-5 — The Postgres adapter and the two-adapter conformance test

The V0.1 adapter implements `claim` with **`FOR UPDATE SKIP LOCKED`**, the sketch already named in the architecture overview's row 5. Queue tables are **host-owned operational tables** under the engine-agnostic `Storage` trait ([P-0010](P-0010-storage-substrate-engine.md) D5), sitting **outside** the [P-0017](P-0017-storage-cluster-model.md) four-shape content taxonomy. That follows the [P-0022](P-0022-coordination-cluster.md) D-STORAGE precedent, and it requires no P-0017 amendment.

A **design-time two-adapter conformance test** demonstrates the port's operations and delivery semantics over the Postgres adapter and a second, in-memory or stub, adapter. That makes the port's portability a checked property from V0.1, not just an assertion. The port is shaped so other subsystems (event-bus consumers, projection refresh) can adopt it later without amendment. No second consumer is designed now. *(Spec: R-0103-c.)*

### Consequences

**Good:**
- The harness gets its substrate primitive with the delivery semantics locked as a contract. A future adapter swap (SQS or Temporal class) becomes a one-module change, proven by a conformance suite that exists from V0.1.
- At-least-once composes with the ELT harness's already-required idempotent transforms. No exactly-once machinery, no distributed transaction.
- The post-commit best-effort enqueue keeps the port free of SQL-transactional coupling. The reconciliation orphan sweep carries the delivery guarantee instead: one mechanism, the scan, covers both missed-arrival and orphan recovery.

**Bad / Trade-offs:**
- Polling as the portable baseline trades a little latency for portability. A low-latency wake hint is available as an adapter capability, but it's never contractual.
- At-least-once means a handler can see a job twice, from lease-expiry re-delivery. The idempotence obligation is real harness-contract work, accepted because the ELT shape requires it anyway.
- A minimal verb set forecloses priority lanes, a cron DSL, or workflow orchestration at V0.1. That's deferred, re-openable by a real second-consumer need, not built speculatively.

## Pros and Cons of the Options

### Minimal port + Postgres adapter in this bundle (chosen)
- Pro: the harness (the dogfood consumer) is the forcing function. The two-adapter test guards portability, at drafting cost only.
- Pro: delivery semantics are locked as a contract. An adapter swap becomes a substitution, not a rewrite.
- Con: a generic port with one consumer is slightly more structure than a private table would be. That cost is paid once, at the swap-readiness layer.

### Standalone `JobQueue` spec first
- Pro: cleanest separation of concerns.
- Con: serializes the harness behind a queue spec with no second consumer to justify it. The dogfood consumer is the better forcing function.

### Private job table now, extract later
- Pro: least structure at V0.1.
- Con: forfeits swap-readiness for the same drafting cost. It encodes the hard-to-change delivery semantics ad hoc, exactly where P-LockContract says to lock a contract instead.

## More Information

- Binding requirement text: [spec R-0103](../../specs/2026-07-16-ingestion-pipeline.md) (plus R-0100-b, R-0101-d, and R-0112-b, for the enqueue and orphan-sweep delivery guarantee).
- Companion ADR: [P-0024](P-0024-ingest-pipeline-shape.md), covering the pipeline shape. IPS-2, IPS-4, and IPS-5 render the seam, the orphan-sweep predicate, and the harness the queue serves.
- Substrate: [P-0010](P-0010-storage-substrate-engine.md) D5 (the engine-agnostic `Storage` trait and two-adapter-test pattern); [P-0022](P-0022-coordination-cluster.md) D-STORAGE (the operational-tables-outside-the-content-taxonomy precedent, and the leases-vs-jobs distinction); the architecture-overview's subsystem row 5 (the `SKIP LOCKED`, UUID v7, JSONB, and `scheduled_for` sketch, verified current in Frame §8).
- Frame: [`docs/intent/ingestion-pipeline-frame.md`](../../intent/ingestion-pipeline-frame.md) (blob `f56b3685`), IP-5; §9 open ADR slots, opened by Stage-2a decision 1.
- Placeholder ledger: [`placeholder-resolution.md`](placeholder-resolution.md) updates `{{P-job-queue}}` to P-0025 when this lands.
