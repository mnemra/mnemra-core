---
title: "P-0010: Storage Substrate and Engine"
summary: "PostgreSQL ratified on merits as the storage substrate, behind an engine-agnostic swappable Storage trait (one implementation). V0 engine is embedded Postgres; V0 stack is A1-clean (pgvector HNSW + native FTS + recursive CTEs + JSONB); keyword/graph/time-series capabilities adopted on named trip-wires. Supersedes the substrate hard-lock framing of P-0001 and the Frame."
primary-audience: agent
---

---
status: "proposed"
date: "2026-06-08"
decision-makers: ["the maintainer"]
consulted: ["the researcher", "the orchestrator"]
informed: []
supersedes: null
superseded_by: null
---

# P-0010: Storage Substrate and Engine

## Status

`proposed`

The *decisions* this ADR records (D1–D8) were ratified by the maintainer 2026-06-07; escalation E1 (D8's observability-hypertable reach) was dispositioned 2026-06-09 (re-derive now → the [observability baseline](../architecture/overview.md#observability); [P-0004-observability-shape](P-0004-observability-shape.md) is `deprecated`, no successor ADR). The *ADR* is `proposed` until it clears the WS-E-2 review gate (code+security review, canon-conformance review); it moves to `accepted` at that point, consistent with the lifecycle in the [ADR README](README.md) and with how the sibling ADRs were accepted only after their review pass. It folds the ratified decisions into the WS-E-2 designed tier before that increment merges. It reframes [P-0001-storage-layout](P-0001-storage-layout.md) (the layout choice is unchanged; its Postgres-specifics become the Postgres implementation under this substrate decision) and the substrate-hard-lock framing in the [Frame](../intent/mnemra-core-frame.md) and [Architecture Overview](../architecture/overview.md). It does not override a `DEFAULTS.md` baseline entry.

## Context and Problem Statement

The WS-E-2 Frame and the V0 constraints treated the storage substrate (PostgreSQL + extensions) as a **hard-locked brief constraint** — a carry-forward from the legacy pre-discovery spec, recorded as "hard-locked" but never derived against alternatives. The Frame named only a *layout* slot (resolved by [P-0001-storage-layout](P-0001-storage-layout.md)); there was **no substrate/engine ADR slot**, because the substrate was treated as settled input rather than a decision.

A storage-engine evaluation re-opened the substrate on merits: three sibling passes scored Postgres-as-multi-model, unified multi-model engines (SurrealDB flagship), and polyglot best-of-breed against one 15-axis scorecard, applying a license gate first (pass/fail), then comparing survivors on capability. The evaluation also surfaced an engine-choice question (embedded vs external-instance Postgres) and a storage-surface question (engine-agnostic swap trait vs Postgres-shaped trait) that the prior framing had pre-decided in the opposite direction.

The decisions the evaluation produced have **no home** in the current WS-E-2 ADR set. This ADR is that home. It locks the substrate, the V0 engine, the V0 stack, the per-capability adoption posture, and the storage-surface contract; it reframes the layout ADR and the Frame/overview to match; and it records the method-borrows and managed-tier dispositions as forward-context.

## Decision Drivers

- **Survives-on-merits, not incumbency.** The carry-forward must clear the same usefulness-first bar every other substrate would: an option is ruled out only for an intrinsic, timeline-independent reason (architecture, license, mathematics), and a roadmap deferral is a capability condition, never a not-useful verdict. (Honesty — affirmative-merits framing resists the survivor-bias read.)
- **License posture is a hard gate for a redistributed Apache-2.0 product with a managed tier.** The substrate ships inside the single self-hosted binary (embedded-default is the design intent), so any field-of-use restriction propagates downstream. The license is evaluated as a pass/fail gate applied *first*, not as a scored axis. (Honesty; Migration cost.)
- **Multi-tenancy, single-transaction supersession, and operational maturity are the identity-bearing axes.** mnemra's headline guarantees (workspace-scoped isolation; deterministic keyed supersession as one atomic write; a sold managed tier staked on the substrate) turn on these, not on raw capability ceiling. (Security; Quality outranks cost.)
- **Smallest mature mechanism at V0; extensions earn their place on named trip-wires.** Each capability beyond the A1-clean floor is adopted only when a named, mechanically-fired trip-wire shows it is load-bearing. (P-Defer / DF1; Simplicity.)
- **Lock the contract, vary the implementation — and lock only what is intrinsic.** The storage surface is locked as an engine-agnostic trait; the implementation (Postgres) varies behind it. The lock is scoped to the assumptions it is made against. (P-LockContract; constraint-edges P-LockContract ↔ P-PreserveDecisionSpace when-to-lock edge.)
- **Correct sizing for the commercial envelope.** mnemra's regime (10–300-engineer teams, low-thousands of artifacts, ~1M tokens, no stated QPS) sits three orders of magnitude below every best-of-breed crossover point. Capability a polyglot stack would buy is unbankable here. (Simplicity; Dogfooding — actual scale, not imagined scale.)

## Considered Options

The evaluation closed these on merits; they are recorded here per P-PreserveDecisionSpace, not re-derived.

1. **PostgreSQL on merits, behind an engine-agnostic swap trait (chosen).** Postgres is the only license-clean, capability-credible, operationally-mature contender after the gate. Storage sits behind a swappable `Storage` trait; Postgres is the only implementation built.
2. **A unified multi-model engine (SurrealDB / ArangoDB).** The strongest capability columns (native vector + graph + hybrid-RRF in one engine, in-process embeddable). **Gated out:** both ship under BSL 1.1, whose redistributed field-of-use restriction breaks the Apache-2.0-OSS identity and the sold-managed-tier story for every shipped version on a rolling clock. License-clean unified survivors each fail a different intrinsic axis (Gel is Postgres-underneath; Kùzu's repo is archived).
3. **Polyglot best-of-breed (service or embedded composite).** **Gated/killed on two intrinsic grounds:** the service-polyglot half taints the Apache-2.0 composite (Neo4j GPL/AGPL, Memgraph BSL); the embedded composite cannot deliver single-transaction cross-store atomic supersession (mnemra's headline guarantee) and has no viable Apache-aligned embedded graph component (Kùzu archived).

## Decision Outcome

The full ratified-decision text follows. Each decision is locked and anchored. D8's observability-hypertable reconciliation — which collided with the then-accepted [P-0004-observability-shape](P-0004-observability-shape.md) and was carried as escalation E1 — was **dispositioned by the maintainer on 2026-06-09 (re-derive now)**: the observability shape is re-derived around generation⊥storage separation to the [observability baseline](../architecture/overview.md#observability) (generation⊥storage as a theory trait — telemetry emitted, not stored in-app), and P-0004 is `deprecated` with no successor ADR. D8 therefore reads clean here (see D8).

### D1 — PostgreSQL ratified as the storage substrate (on merits)

**The system SHALL use PostgreSQL as its storage substrate.** Postgres survives the license gate as the only clean + capable + mature contender, and wins affirmatively on the identity-bearing axes: multi-tenancy is the reference implementation (workspace-scoped Row-Level Security), single-transaction keyed supersession is deliverable as one `BEGIN…COMMIT`, and decades-deep backup/PITR/observability maturity is a quality requirement for a sold product. The carry-forward survives on merits, not incumbency. *(Anchor: ratified D1; Security, Quality outranks cost; P-StackDiscipline S2 — incumbency graded as fit, not installed-base.)*

### D2 — V0 stack is A1-clean; extensions on named trip-wires (adoption principle)

**The V0 stack SHALL be A1-clean: pgvector HNSW + native full-text search + recursive CTEs + JSONB.** No Postgres extension beyond pgvector is adopted at V0. Each additional capability earns its place against a named, mechanically-fired trip-wire (the legs are owned by D3 keyword, D4 graph, D8 time-series). This is the adoption *principle*; the legs instantiate it and are independently decidable. A2 (full-extension) costs — operational composition, managed-foreclosure, AGE PG-version lag — are paid from day one; its benefits are not needed until the specific trip-wires fire. *(Anchor: ratified D2; P-Defer / DF1; Simplicity.)*

### D3 — Keyword leg: native FTS at V0; `pg_textsearch` on a fidelity trip-wire; AGPL gated out

**The keyword leg SHALL use native Postgres full-text search at V0.** Hybrid retrieval (dense + keyword + reciprocal-rank fusion) is available at V0 with a non-BM25 keyword leg (`ts_rank` is TF + proximity, lacking IDF / TF-saturation / length-normalization). The trip-wire is BM25 *fidelity*, not hybrid itself. **Trip-wire (DF1, named instrument):** a `ts_rank`-vs-BM25 recall/precision-at-k regression test against a golden query set, run periodically; fire — and adopt `pg_textsearch` (Tiger Data, PostgreSQL-licensed / Green, true BM25) — when the BM25 leg measurably beats `ts_rank` by a margin worth the extension's cost. The dependency gate SHALL keep AGPL BM25 (`pg_search` / VectorChord-bm25, Red tier) out by default; adopting it requires explicit Red-tier sign-off. *(Anchor: ratified D3; P-Defer / DF1; license-tier gate per the workspace dependency-approval tiers.)*

### D4 — Graph leg: recursive CTEs at V0; Apache AGE deferred to a strain trip-wire

**The graph leg SHALL use recursive CTEs over the shallow edge model at V0.** mnemra's edge model is shallow (parent-pointer + typed shallow edges; frontmatter at V0); recursive CTEs serve it natively. **Trip-wire (DF1, named instrument):** a query-latency-and-expressiveness logging point on the graph-traversal path — log multi-hop CTE query latency and flag any traversal the CTE path cannot express or serves above an acceptable latency bound; fire — and adopt Apache AGE (Apache-2.0 / Green, openCypher) — on a logged dogfood incident where the CTE path cannot serve a real multi-hop query at acceptable latency/expressiveness. AGE's maturity cost (validated PG13–16, limited PG17, openCypher subset, becomes the laggard gating the instance's PG-upgrade cadence) is only paid against that named need. *(Anchor: ratified D4; P-Defer / DF1.)*

### D5 — Storage surface: engine-agnostic, swappable `Storage` trait (one implementation)

**The storage surface SHALL be an engine-agnostic, swappable `Storage` trait. Postgres SHALL be the only implementation built at V0; no second adapter is built.** This **reverses** the prior "Postgres-natural / no-swap / two-adapter-test-does-not-apply" classification that the workspace-private architecture-overview carried (and which the substrate-hard-lock framing in this repo's Frame and overview reflected): storage is now behind an engine-agnostic seam, not a deliberately Postgres-shaped one. SurrealDB is retained as a **method-borrow** (`search::rrf()` — see D6), not as an engine and not as a second adapter.

The contract is locked because it is intrinsic to the storage layer's identity (P-LockContract: lock what is intrinsic, even when not exposed until later). Building a second adapter now is *not* done (P-Defer): the only mode in which a second engine (SurrealDB) escapes its BSL redistribution kill is an operator-run external instance for internal dogfooding only — a backend that can never become the shipped default or rescue the managed tier, so building it now spends scarce founder-hours on a non-shippable path. **Re-open the second-adapter question (DF1 trip-wire) when EITHER:** (a) the Postgres graph limb measurably strains on a real multi-hop need — *the same logged signal D4's AGE trip-wire fires on* (if D4 fires, re-evaluate this at the same time); **OR** (b) an engine relicenses its core to a Green tier (Apache/MIT) and becomes a *shippable* default candidate — detected by a periodic (quarterly) license-watch over the candidate engines' repos (SurrealDB, ArangoDB, a revived Kùzu fork). Both are mechanically detectable; neither is "someone will remember." A new shippable candidate would slot behind this same trait. *(Anchor: ratified D5; P-LockContract; P-Defer / DF1; constraint-edges P-LockContract ↔ P-PreserveDecisionSpace when-to-lock edge.)*

The `Storage` trait is exercised by Postgres (the production implementation) and an in-memory test adapter — the layering/test seam that motivates having a trait at all even when only one production engine exists. (This trait-for-layering rationale predates the swap-trait decision: a one-implementation-deep `Storage` trait was already part of the substrate design for in-memory testing and host-layer decoupling; D5 widens it from deliberately Postgres-shaped to engine-agnostic.) The design-time two-adapter test for the trait binds **two** co-equal contract invariants, both recorded in the multi-tenancy and transaction note below: single-transaction keyed supersession (the transaction / unit-of-work surface) **and** workspace-scoped tenant isolation. These are the guarantees any swap candidate must preserve; an adapter that cannot express either fails the two-adapter test and does not satisfy the contract.

### V0 engine: embedded Postgres

**The V0 engine SHALL be embedded Postgres** (`postgresql_embedded` with `pgvector` compiled/bundled via `postgresql_extensions` / `pgvector_compiled`), not an operator-provisioned external Postgres server. This was chosen over embedded-SurrealDB on a measured M4 spike: 8× lighter idle / 20× under workload, pgvector clean on Apple Silicon, where SurrealDB v3.1.3 carried a `.bind()`+KNN hang. The embedded engine ships *with* the single self-hosted binary; pgvector is bundled/compiled, not OS-installed. This is the storage-side of the "single self-hosted binary, embedded-default" deployment posture. *(Anchor: ratified V0-engine decision; Dogfooding — measurement overturned the PG-heavier hunch.)*

This engine choice **falsifies the substrate's earlier external-instance deployment framing** — the assumption that an operator provisions a Postgres server and OS-installs extension binaries before `mnemra init`. The storage-side requirements that carried that framing are corrected in the spec (see the spec amendments below): `mnemra init` enables a bundled pgvector against the embedded engine; it does not require an OS-installed `pgvector` binary on an external server, and (per D8) it does not enable TimescaleDB at all at V0.

### D8 — Time-series: plain timestamped tables at V0; TimescaleDB demoted to a trip-wire

**The time-series *storage shape* (the content-substrate's own time-series tables) SHALL use plain timestamped Postgres tables at V0; TimescaleDB is demoted off the V0 stack to a latency/storage trip-wire.** TimescaleDB rode the legacy carry-forward into the V0 stack unexamined — the same un-merits-tested default this study corrected for pgvector. Plain Postgres timestamped tables serve mnemra's actual time-series workload (low-thousands-of-artifacts scale); TimescaleDB's hypertables / columnar compression / continuous aggregates only earn their keep orders of magnitude up. Demoting it keeps the engine choice unchanged (TimescaleDB is a Postgres extension) and keeps the V0 stack on commodity managed Postgres. It is also operationally heavy: `shared_preload_libraries` + instance restart to install (not a hot `CREATE EXTENSION`), and hypertables do not round-trip vanilla `pg_dump`/`pg_restore` (need `timescaledb_pre_restore()`/`post_restore()` + version match). **Trip-wire (DF1, named instrument):** add TimescaleDB when a logged **query-latency-or-storage-cost threshold on the metrics/events tables** is crossed in dogfooding — a concrete logged signal, not "when metrics grow." *(Anchor: ratified D8; P-Defer / DF1; Simplicity.)*

**Scope boundary and observability reconciliation (resolved — was escalation E1).** D8 as ratified is about the *time-series storage shape*. The WS-E-2 designed tier had also committed the **observability** metrics and events surfaces to TimescaleDB **hypertables** in the then-accepted [P-0004-observability-shape](P-0004-observability-shape.md) and the locked spec (R-0004-a/b/c/e, R-0013-c, the health-endpoint `timescaledb` field, and R-0013-a's `CREATE EXTENSION timescaledb`) — the same two surfaces D8 reaches (the brief's D8 rationale names `dispatch_metrics`/events as the workload plain tables serve). Under the constraint-graph when-to-lock edge, P-0004's 2026-05-24 lock is *scoped to the assumptions it was made against*; D8 (ratified 2026-06-07) is a later reshape that falsifies the time-series-backend assumption, so canon re-derives P-0004 against the new world rather than honoring it as if the world had not moved (P-LockContract anti-example; freeze-scope-under-reshape). The *scope/sequencing* of that re-derivation was the maintainer's call, carried as escalation **E1**. **The maintainer dispositioned E1 on 2026-06-09: re-derive now.** The observability shape is re-derived around generation⊥storage separation to the [observability baseline](../architecture/overview.md#observability) (generation⊥storage as a theory trait, emit-not-store), and P-0004 is **`deprecated`** with no successor ADR, and the spec's observability requirements (R-0004-a/b/c/e, R-0013-c, the `CREATE EXTENSION timescaledb` step, the health-body `timescaledb` field) are re-derived to the generation baseline (stdout/OTel/health-first; no in-app observability hypertables) in the same fold. There is no surviving ADR-vs-ADR contradiction: P-0004 is `deprecated` (no successor ADR), and D8's demote now reaches the observability surfaces consistently with the overview observability baseline. *(Anchor: D8; E1 disposition 2026-06-09; the observability baseline.)*

### Multi-tenancy and the single-transaction supersession surface

Postgres RLS *is* the reference implementation for workspace-scoped isolation — but only with its operational preconditions met; omitting any one is a silent cross-tenant-leak risk. These are **requirements**, carried from the ratified evaluation (M4) into this ADR's multi-tenancy surface and cross-referenced to [P-0009-rls-admin-token](P-0009-rls-admin-token.md) (which owns the role model and the V0.1+ RLS-policy activation path):

1. **The application role MUST NOT hold `BYPASSRLS` and MUST NOT be a superuser.** Superusers and `BYPASSRLS` roles bypass every policy by default; mnemra-core connects to the substrate as an ordinary role.
2. **`ALTER TABLE … FORCE ROW LEVEL SECURITY` is required if the application role owns the tables.** Table owners are exempt from their own RLS unless `FORCE` is set.
3. **The tenant key MUST be set per-transaction, not per-session.** Under a transaction-mode connection pooler, use `SET LOCAL app.workspace_id = …` *inside* the transaction; a bare session-level `SET` persists on the physical connection and leaks across pooled checkouts to the next tenant. (P-0009 already specifies `SET LOCAL` for the `mnemra.workspace_id` / `mnemra.role` session settings at the request boundary; these preconditions are the operational guard around that mechanism.)

These preconditions bind at V0.1+ RLS-policy activation (the V0 enforcement is application-layer per [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) and [P-0009-rls-admin-token](P-0009-rls-admin-token.md)); they are stated here so the substrate decision carries them rather than leaving them as implementation trivia.

**The swap-trait's design-time two-adapter test binds two co-equal contract invariants** — single-transaction keyed supersession *and* workspace-scoped tenant isolation. Both bind at the contract level (what any adapter must be *able* to preserve at the storage layer), independent of the V0.1+ RLS-policy-activation timing; V0 isolation enforcement is application-layer per [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md).

1. **Single-transaction keyed supersession.** Engine-managed keyed supersession — new version row + forward-pointer + superseded-vector delete + FTS update, as one atomic operation — is one `BEGIN…COMMIT` in Postgres. This is the guarantee a polyglot stack structurally cannot deliver, and it is the invariant the `Storage` trait's transaction / unit-of-work surface MUST preserve: any future second implementation behind the trait must express this multi-write supersession atomically (a single unit of work that commits or rolls back as a whole), or it does not satisfy the contract. The trait therefore exposes a unit-of-work / transaction boundary, not per-write autocommit calls.

2. **Workspace-scoped tenant isolation.** Per-tenant isolation is a first-class contract requirement, co-equal with supersession atomicity — not a Postgres implementation detail. Any conforming adapter MUST enforce workspace-scoped isolation such that operations executed in one workspace's context cannot read or mutate another workspace's rows, preserving the per-tenant isolation guarantees [P-0001-storage-layout](P-0001-storage-layout.md) and [P-0009-rls-admin-token](P-0009-rls-admin-token.md) establish. The Postgres implementation satisfies this via RLS plus the operational preconditions above (no `BYPASSRLS`/superuser, `FORCE ROW LEVEL SECURITY`, per-transaction `SET LOCAL`, per P-0009); a non-Postgres adapter must provide an equivalent storage-layer guarantee. An adapter that cannot enforce per-tenant isolation **fails the two-adapter test** and does not satisfy the contract — the same binary standing as the atomicity requirement, so the two-adapter test exercises the workspace-isolation invariant (D1's identity-bearing axis), not transactional atomicity alone.

*(Anchor: ratified single-transaction-supersession finding; D1 multi-tenancy identity-bearing axis; P-0001, P-0009; P-LockContract — the contract is what a swap candidate must satisfy.)*

### Consequences

**Good:**
- The substrate is now a decided artifact with an explicit option space (P-PreserveDecisionSpace), not an unexamined carry-forward.
- Storage sits behind an engine-agnostic seam (D5); a future Green-relicensed engine slots behind the same trait without re-architecting call sites (P-LockContract).
- The V0 stack runs on commodity managed Postgres (RDS/Aurora/Cloud SQL all ship pgvector; FTS, CTEs, JSONB are core) — managed-portability is intact at V0 and degrades only when a capability trip-wire fires.
- Smallest mature mechanism at V0 (D2): each extension's operational cost (composition, managed-foreclosure, version-lag, restart-to-install) is paid only against a named capability need.
- The embedded engine ships with the single binary — no operator-provisioned Postgres server, no OS-installed extension binaries at V0.

**Bad / Trade-offs:**
- D5 reverses a prior locked classification; the Frame, overview, and P-0001 must be reframed in the same fold (done — see below), and the workspace-private `architecture-overview.md` carries the same stale classification as a separable follow-up.
- The swap trait carries a one-implementation cost: a trait seam that no second engine exercises yet (justified by the in-memory test adapter and by the trait's role as the re-open seam for the D5 trip-wire).
- **D8's observability reach required re-deriving the accepted observability ADR (was E1, now resolved).** D8 demotes TimescaleDB off the V0 stack, which reaches the observability metrics/events surfaces P-0004 had committed to hypertables. The maintainer dispositioned this (E1) on 2026-06-09 = re-derive now: P-0004 is `deprecated` with no successor ADR, and the observability shape is re-derived around generation⊥storage separation to the [observability baseline](../architecture/overview.md#observability). The time-series-backend decision is now consistent (plain timestamped tables for the storage shape; no in-app observability hypertables — telemetry is emitted, not stored in-app, at V0). No surviving contradiction.
- Hybrid retrieval at V0 ships with a non-BM25 keyword leg (D3); BM25 fidelity is a measured trip-wire, not a V0 guarantee.

## Pros and Cons of the Options

### PostgreSQL on merits, behind a swap trait (accepted)

- Pro: Only license-clean + capable + mature contender after the gate; wins affirmatively on multi-tenancy, single-transaction supersession, and correct sizing.
- Pro: Engine-agnostic seam preserves the relicense/landscape optionality without building a second adapter.
- Pro: V0 stack is managed-Postgres-portable; embedded engine ships with the binary.
- Con: Reverses a prior locked storage classification (handled in this fold).
- Con: Swap trait is one-implementation-deep at V0.

### Unified multi-model engine (SurrealDB / ArangoDB)

- Pro: Strongest capability column — native vector + graph + hybrid-RRF in one engine, in-process embeddable; `search::rrf()` expresses mnemra's hardest retrieval constraint in one query.
- Con: BSL 1.1 core; redistributed field-of-use restriction breaks the Apache-2.0-OSS identity and the sold-managed-tier story, per version, on a rolling clock (intrinsic, timeline-independent kill).
- Con: License-clean unified survivors fail other intrinsic axes (Gel is Postgres-underneath; Kùzu archived).

### Polyglot best-of-breed

- Pro: Best-of-breed capability ceiling (extreme-scale ANN, million-edge graph, monitoring-grade time-series).
- Con: Cannot deliver single-transaction cross-store atomic supersession — mnemra's headline guarantee (structural kill).
- Con: No viable Apache-aligned embedded graph component (Kùzu archived); service-polyglot graph engines taint the Apache-2.0 composite (Neo4j GPL/AGPL, Memgraph BSL).
- Con: The marginal quality it buys is unbankable at mnemra's commercial envelope (three orders of magnitude below every crossover point).

## More Information

**Method-borrows (D6) → retrieval ADR (deferred, not authored here).** The evaluation carried four reusable *methods* forward (engines out, patterns in): (1) single-query BM25 + dense + RRF fusion (`search::rrf()`) as the ergonomic reference target for mnemra's Postgres-side application-fusion retrieval; (2) `pg_textsearch` as the named Green BM25 default (the escape from the AGPL trap); (3) collapsed-tree / multi-resolution embeddings + keyed-supersession-via-normalized-topic-key SQL patterns; (4) borrow the graph *model*, not a graph *engine*. **These belong to the retrieval-layer ADR, which does not exist yet and is NOT authored in this fold.** Decision: defer the four method-borrows to the future retrieval ADR (decision-content: carry all four; anchor: ratified D6; **named tripwire (DF1):** the method-borrows land when the retrieval-layer ADR is authored — that ADR's authoring is the firing event, and this note is its forward-context source). Recorded here so the borrows are not lost between fold and retrieval-ADR authoring.

**Managed-tier note (D7) — skipped.** Commercial is a maybe-never and V0 is embedded; per the ratified D7, no managed-tier content is authored. The managed-portability *fact* (the V0 stack runs on commodity managed Postgres; portability degrades per-extension-trip-wire) is recorded in the Consequences above as a planning fact, not as a managed-tier design.

**Cross-references:**
- [P-0001-storage-layout](P-0001-storage-layout.md) — the C1 single-document layout is the Postgres *implementation* under this substrate decision; layout is unchanged, its Postgres-specifics sit under D1/D5, and its TimescaleDB references are stripped/demoted per D8.
- [P-0004-observability-shape](P-0004-observability-shape.md) (`deprecated`, no successor ADR) → the [observability baseline](../architecture/overview.md#observability) — D8's observability reach was reconciled by re-deriving the observability shape around generation⊥storage separation to the overview baseline (E1 dispositioned 2026-06-09 = re-derive now); P-0004 is deprecated, not superseded.
- [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md), [P-0009-rls-admin-token](P-0009-rls-admin-token.md) — own the V0 application-layer enforcement and the V0.1+ RLS-policy activation path the multi-tenancy preconditions bind to.
- [Frame](../intent/mnemra-core-frame.md) (storage-shape section + Tier-A slot table) and [Architecture Overview](../architecture/overview.md) (storage classification + TimescaleDB references) — reframed in this same fold.
- Source: the locked 2026-06-07 storage-engine evaluation (ratified by the maintainer), which carries the full rationale, the 15-axis matrix, the BSL-redistribution mechanism, the RLS operational-preconditions block, the falsifiable bars, and the ops-gaps subsection.

---

## Escalation E1 — D8 vs P-0004 observability hypertables (RESOLVED 2026-06-09)

D8 demotes TimescaleDB off the V0 stack to plain timestamped tables; the then-accepted [P-0004-observability-shape](P-0004-observability-shape.md) and the locked spec (R-0004-a/b/c/e, R-0013-c, R-0013-a's `CREATE EXTENSION timescaledb`, the health-endpoint `timescaledb` field) committed the metrics/events surfaces to TimescaleDB hypertables — the same surfaces D8 reaches. Canon decided the *principle*: the falsified P-0004 freeze is re-derived against the post-D8 world (P-LockContract when-to-lock edge; freeze-scope-under-reshape). What was open was the *scope/sequencing* — re-derive P-0004 + the R-0004/R-0013-c requirements within the fold, or sequence that re-derivation as a tracked follow-up — the maintainer's call.

**Disposition (maintainer, 2026-06-09): re-derive now**, with a deeper architectural direction — separate observability *generation* from *storage*. The observability shape is re-derived to the [observability baseline](../architecture/overview.md#observability) (generation⊥storage as a theory trait; P-0004 `deprecated`, no successor ADR): the server emits telemetry (stdout structured logs + OTel traces/metrics + health-endpoint-first) storage-independently from the bare shell; the observability *storage* backend is deferred behind the separation (option set {Prometheus, InfluxDB, TimescaleDB, plain Postgres tables}, named tripwire), not locked; the standalone binary survives (observability storage is external operator infra). The spec's observability requirements are re-derived to that baseline in the same fold. The D8 "NOT locked here" carve-out is removed (above); P-0004 is deprecated, with no successor ADR. E1 carries no open content; this record preserves the lineage (P-PreserveDecisionSpace).
