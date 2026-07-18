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

Authored at Stage 3 of the ingestion-pipeline cluster, resolving the Frame's `{{P-job-queue}}` slot (opened by Stage-2a decision 1) per [placeholder-resolution](placeholder-resolution.md). It moves to `accepted` at the ingestion-pipeline **spec-exit gate**, reviewed with the spec [2026-07-16-ingestion-pipeline](../../specs/2026-07-16-ingestion-pipeline.md), following the companion-ADR precedent. The port was locked at the Frame-exit gate (2026-07-17, Frame IP-5); this ADR renders it at ADR precision and makes **no** fresh decisions. Binding requirement text is single-sourced to spec R-0103.

## Context and Problem Statement

The ingest transform harness ([P-0024](P-0024-ingest-pipeline-shape.md) IPS-5) enqueues one transform job per staged row; job workers claim jobs and apply the registered transform. That needs a **job queue** — a work-distribution primitive with at-least-once delivery and single-claimant semantics — which the system does not yet have and which has no ADR. The architecture-overview's subsystem table named a `JobQueue` row (row 5) with a `FOR UPDATE SKIP LOCKED` sketch, UUID v7 ids, JSONB payloads, and `scheduled_for` delivery, but left it unbuilt. Stage-2a decision 1 (LOCKED) ratified building **a minimal port + Postgres adapter as part of this bundle**, with the harness as its first consumer (dogfood-driven) and the two-adapter test applied at drafting — over the alternatives of specifying the queue in its own spec first, or defining a private job table now and extracting a generic subsystem later.

The queue is a distinct primitive from [P-0022](P-0022-coordination-cluster.md)'s **leases** (actor-held intent across time): a job claim is worker work-distribution. The two share the substrate-atomicity posture and nothing else — stated to prevent a future conflation.

## Decision Drivers

- **The harness needs a queue that does not exist** (Frame IP-5; intake SC6 — a transform consumer writes against the harness conventions without reading pipeline internals): the queue is the substrate under transform-as-job.
- **Contracts lock, implementations vary** (P-LockContract; the two-adapter test as a design-time swap-readiness check, the [P-0010](P-0010-storage-substrate-engine.md) D5 `Storage`-trait pattern): the port's verbs and delivery semantics are the hard-to-change contract; the Postgres adapter is one implementation behind it.
- **At-least-once is only safe over idempotent handlers** (P-GuaranteeByMechanism): the ELT shape already requires idempotent, re-runnable transforms ([P-0024](P-0024-ingest-pipeline-shape.md) IPS-5), so at-least-once delivery composes with the harness contract without adding an exactly-once burden the substrate cannot cheaply provide.
- **Minimal verb set, no premature mechanism** (Simplicity; P-Defer): no priority lanes, cron DSL, or workflow engine at V0.1 — the queue is the smallest thing the harness needs.
- **Portability without a second consumer built now** (P-PerRepoFirst at subsystem scope): the port is shaped so other subsystems can adopt it later without amendment, but no second consumer is designed now.

## Considered Options

1. **Minimal `JobQueue` port + Postgres adapter, in this bundle, harness as first consumer** (chosen — Stage-2a decision 1, LOCKED).
2. **A standalone `JobQueue` spec first, this bundle depends on it** — rejected at 2a: it serializes the harness behind a queue spec with no second consumer to justify the separation; the dogfood consumer (the harness) is the right forcing function, and the two-adapter test at drafting already guards portability.
3. **A private job table in the harness now, generic subsystem extracted later (rule-of-three deferral)** — rejected at 2a: the delivery semantics (at-least-once, single-claimant, lease/visibility, dead-letter) are the hard-to-change contract, and encoding them ad hoc in a private table forfeits the swap-readiness the port + two-adapter test buys for the same drafting cost.

## Decision Outcome

**Chosen: Option 1** — a minimal `JobQueue` port with a Postgres `FOR UPDATE SKIP LOCKED` adapter, the harness its first consumer. Rendered as JQ-1..JQ-5. Binding requirement text: spec R-0103.

### JQ-1 — The port operation set

The port exposes, expressed in delivery semantics and never in engine idioms:

- `enqueue(kind, payload, scheduled_for?)` — register a job; a `kind` with no registered handler does not enqueue (validator-first, closed job-kind vocabulary).
- `claim(kinds, lease)` — atomically claim one available job of a listed kind, taking a lease/visibility window.
- `complete(job)` — mark a claimed job done (terminal `completed`).
- `fail(job, retryable?)` — record a failed attempt (incrementing `attempts`): a **retryable** fail returns the job to `pending` for backoff re-delivery (as a lease expiry does), so there is no non-terminal `failed` resting state; a **non-retryable** fail, or an attempts-exhausted retry, moves the job to the terminal dead-letter state.
- dead-letter disposition — a job whose retries are exhausted (or a non-retryable failure) moves to a terminal `dead_letter` state, not silent loss.
- `schedule` via `scheduled_for` — delayed/future delivery.

Job identity is UUID v7; payloads are JSONB; job kinds are a code-registered closed vocabulary. *(Spec: R-0103-a.)*

### JQ-2 — Delivery semantics: at-least-once, single live claimant

Delivery is **at-least-once** with **a single live claimant per job**: `claim` carries a lease/visibility window (its duration a [spec § Numeric calibrations](../../specs/2026-07-16-ingestion-pipeline.md#numeric-calibrations) default, sized to **exceed the longest lane's wall-clock bound plus margin** so a still-running job is never re-delivered to a second claimant while the first is working), and an expired claim re-delivers the job. The consequence is bound into the harness contract — **handlers are idempotent** ([P-0024](P-0024-ingest-pipeline-shape.md) IPS-5): at-least-once is only safe over idempotent transforms, which the ELT shape already requires (re-run is first-class). Exactly-once delivery is **not** provided (it would push distributed-transaction machinery the substrate cannot cheaply guarantee into the port); idempotent-convergence at the handler is the deliberate, canon-anchored alternative. *(Spec: R-0103-b.)*

### JQ-3 — Portability discipline: no `LISTEN/NOTIFY`, no SQL-transactional coupling in the port

The port surface carries **no `LISTEN/NOTIFY`** and **no SQL-transactional coupling**: a low-latency wake hint is an optional *adapter* capability, not a port guarantee, and **polling is the portable baseline**. The port's verbs and semantics are satisfiable by an SQS-class or Temporal-class adapter — this is the two-adapter test (JQ-5) as a design-time swap-readiness check. Consequently the enqueue from the loader seam is **post-commit, best-effort** ([P-0024](P-0024-ingest-pipeline-shape.md) IPS-2): a cross-resource staging+enqueue transaction would put SQL-transactional coupling in the port surface, falsifying portability. The Postgres adapter **MAY** enqueue inside the staging transaction as an adapter-local optimization, but never as a port guarantee. *(Spec: R-0103-c, R-0100-b.)*

### JQ-4 — The delivery guarantee: post-commit best-effort enqueue + reconciliation re-enqueue

Because the enqueue is post-commit best-effort (JQ-3), the delivery guarantee does not live on the enqueue call. It lives on **staged-durability-plus-reconciliation**: the staging write is transactional and durable, and a staging row whose enqueue never landed a transform job (a **transform orphan**) is re-enqueued by the reconciliation scan's staging→jobs join — the orphan predicate rendered in [P-0024](P-0024-ingest-pipeline-shape.md) IPS-4. Orphan latency is bounded by the scan cadence and the `enqueue_orphan_window` ([spec § Numeric calibrations](../../specs/2026-07-16-ingestion-pipeline.md#numeric-calibrations)). This is what lets the no-partial-write / delivery guarantee (Frame IP-2/IP-14) hold without SQL-transactional coupling in the port. *(Spec: R-0100-b, R-0101-d, R-0112-b.)*

### JQ-5 — The Postgres adapter and the two-adapter conformance test

The V0.1 adapter implements `claim` with **`FOR UPDATE SKIP LOCKED`** (the overview row-5 sketch). Queue tables are **host-owned operational tables** under the engine-agnostic `Storage` trait ([P-0010](P-0010-storage-substrate-engine.md) D5), **outside** the [P-0017](P-0017-storage-cluster-model.md) four-shape content taxonomy (the [P-0022](P-0022-coordination-cluster.md) D-STORAGE precedent) — no P-0017 amendment. A **design-time two-adapter conformance test** demonstrates the port's operations and delivery semantics over the Postgres adapter and a second (in-memory or stub) adapter, so the port's portability is a checked property from V0.1, not an assertion. The port is shaped so other subsystems (event-bus consumers, projection refresh) can adopt it later without amendment; no second consumer is designed now. *(Spec: R-0103-c.)*

### Consequences

**Good:**
- The harness gets its substrate primitive with the delivery semantics locked as a contract, so a future adapter swap (SQS/Temporal class) is a one-module change proven by a conformance suite that exists from V0.1.
- At-least-once composes with the ELT harness's already-required idempotent transforms — no exactly-once machinery, no distributed transaction.
- The post-commit best-effort enqueue keeps the port free of SQL-transactional coupling, and the reconciliation orphan sweep carries the delivery guarantee instead — one mechanism (the scan) covers both missed-arrival and orphan recovery.

**Bad / Trade-offs:**
- Polling as the portable baseline trades a little latency for portability; a low-latency wake hint is available as an adapter capability but is never contractual.
- At-least-once means a handler can see a job twice (lease expiry re-delivery); the idempotence obligation is real harness-contract work, accepted because the ELT shape requires it anyway.
- A minimal verb set forecloses priority lanes / cron DSL / workflow orchestration at V0.1 — deferred, re-openable by a real second-consumer need, not built speculatively.

## Pros and Cons of the Options

### Minimal port + Postgres adapter in this bundle (chosen)
- Pro: the harness (dogfood consumer) is the forcing function; the two-adapter test guards portability at drafting cost.
- Pro: delivery semantics locked as a contract; adapter swap is a substitution, not a rewrite.
- Con: a generic port with one consumer is slightly more structure than a private table — paid once, at the swap-readiness layer.

### Standalone `JobQueue` spec first
- Pro: cleanest separation of concerns.
- Con: serializes the harness behind a queue spec with no second consumer to justify it; the dogfood consumer is the better forcing function.

### Private job table now, extract later
- Pro: least structure at V0.1.
- Con: forfeits swap-readiness for the same drafting cost; encodes the hard-to-change delivery semantics ad hoc, exactly where P-LockContract says to lock a contract.

## More Information

- Binding requirement text: [spec R-0103](../../specs/2026-07-16-ingestion-pipeline.md) (and R-0100-b / R-0101-d / R-0112-b for the enqueue + orphan-sweep delivery guarantee).
- Companion ADR: [P-0024](P-0024-ingest-pipeline-shape.md) (the pipeline shape; IPS-2/IPS-4/IPS-5 render the seam, the orphan-sweep predicate, and the harness the queue serves).
- Substrate: [P-0010](P-0010-storage-substrate-engine.md) D5 (the engine-agnostic `Storage` trait + two-adapter-test pattern); [P-0022](P-0022-coordination-cluster.md) D-STORAGE (the operational-tables-outside-the-content-taxonomy precedent, and the leases-vs-jobs distinction); the architecture-overview subsystem row 5 (the `SKIP LOCKED` / UUID v7 / JSONB / `scheduled_for` sketch, verified current in Frame §8).
- Frame: [`docs/intent/ingestion-pipeline-frame.md`](../../intent/ingestion-pipeline-frame.md) (blob `f56b3685`), IP-5; §9 open ADR slots (opened by Stage-2a decision 1).
- Placeholder ledger: [`placeholder-resolution.md`](placeholder-resolution.md) updates `{{P-job-queue}}` → P-0025 when this lands.
