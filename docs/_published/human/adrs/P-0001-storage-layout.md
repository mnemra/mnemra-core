---
title: "P-0001: Storage Layout"
summary: "Single-document layout (C1) for the content substrate: whole artifact in one row with JSONB frontmatter + body + system fields. Non-breaking C2 evolution path designed into the projection layer."
primary-audience: agent
---

---
status: "accepted"
date: "2026-05-24"
decision-makers: ["the maintainer"]
consulted: ["the researcher", "the orchestrator", "the security reviewer"]
informed: []
supersedes: null
superseded_by: null
---

# P-0001: Storage Layout

## Status

`accepted`

This ADR's layout choice (C1) is unchanged. Its Postgres-specifics are now the **Postgres implementation under the swappable `Storage` trait** locked in [P-0010-storage-substrate-engine](P-0010-storage-substrate-engine.md), a project-scoped ADR (D1 substrate + D5 swap trait + V0 embedded engine). That means the substrate is no longer a hard-locked carry-forward. It's a decided artifact, and storage now sits behind an engine-agnostic seam rather than a deliberately Postgres-shaped one. C1 is the content-substrate layout *within* that Postgres implementation. The TimescaleDB references this ADR originally carried are stripped or demoted per P-0010's D8 (TimescaleDB demoted off the V0 stack). They're corrected inline below.

## Context and Problem Statement

Mnemra-core's content substrate has to handle the full lifecycle of a logical artifact without a table rebuild. That lifecycle covers creation, mutation, query, projection, migration from the prior task store, and forward-compatible absorption of V0.1 capabilities (vector search, full-text, graph edges).

A pre-Spec candidate analysis (2026-05-03) explored three structurally distinct layout shapes. Each one varies along a single axis: whether a single logical artifact lives entirely in one content substrate row (C1), spans the four-shape substrate boundary per aspect (C2), or is normalized into typed sidecar tables within the content substrate (C3). This ADR locks the choice.

The decision is the root unblocking dependency for [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md), [P-0003-plugin-manifest](P-0003-plugin-manifest.md), and [P-0009-rls-admin-token](P-0009-rls-admin-token.md). It also gates migration correctness: the `brain.db` row shape maps most directly to one of the three candidates.

## Decision Drivers

R-codes below (`R2.4`, `R2.7`, and so on) are stable identifiers for numbered requirements in mnemra-core's requirements document; each is stated in full at the point of citation.

- **Migration parsimony (R2.4).** Adding pgvector, full-text tsvector, and graph edges at V0.1 has to be a set of non-breaking schema additions, with no table rebuild, so the dogfood timeline keeps its safety margin.
- **Round-trip equivalence (R2.7).** Source frontmatter must round-trip byte-equal, modulo the system-generated fields `migrated_from`, `migrated_at`, and `frontmatter_version`. The layout has to store user frontmatter literally.
- **Dogfood-cycle correctness (NFR).** The layout should match the prior task store's row shape as closely as possible, which minimizes migration cutover risk. `brain.db` is a row-per-task SQLite schema. Task attributes are row columns, not spread across substrates.
- **FK preservation (R6.6.1).** Foreign-key relationships (task to project, dispatch to task) must be single-hop validatable, without cross-substrate joins.
- **Plugin contract simplicity.** The plugin manifest's host-fn ABI surface should be as narrow as possible at V0. A narrower content model produces a narrower ABI.
- **Status-churn write amplification (known C1 weakness).** This is acknowledged at V0 dogfood scale, a few hundred status flips a day. The projection layer absorbs the read-side cost. The numeric budget is deferred to V0.1+ if the write rate grows.

## Considered Options

1. **C1 — Single-document layout** — Whole artifact in one row in the content substrate. JSONB frontmatter + body + system columns. Status, owner, priority are JSONB fields on that same row.
2. **C2 — Composite-with-typed-slots** — Body in the content substrate; status/owner/priority in a state-shape table; audit transitions in the log-shape backend; edges in frontmatter (V0) → graph table (V0.1+). The only candidate that fans a single logical artifact across the four-shape boundary.
3. **C3 — Multi-substrate-with-joins** — Primary content row plus typed sidecar tables (`task_status`, `task_audit`, `task_edges`), but all sidecars live in the content substrate. Joins at query time; projections are views over the joined set.

## Decision Outcome

**C1 — Single-document layout.**

C1 is the correct V0 choice. It's closest to `brain.db`'s current row shape. It satisfies R2.7 frontmatter round-trip trivially: JSONB stores source frontmatter literally, and you serialize it back with deterministic key order. And it absorbs V0.1 search, pgvector, full-text, and edges without an ALTER TABLE on existing rows. The dogfood-cycle migration risk is the lowest of the three. At V0 dogfood scale (a few hundred status flips a day), the write-amplification weakness is bounded and not a bottleneck.

The projection layer is designed as the seam that allows a future non-breaking C2 evolution. Per-plugin typed projections over the content rows can emit to the state and log shapes at V0.1+ without altering the content substrate schema. The content row is the source of truth. Projection-layer consumers can evolve their aspect maps independently.

**Schema pattern (V0 floor — per-artifact-type tables):**

```sql
CREATE TABLE tasks (
  id                  text PRIMARY KEY,          -- ULID-from-source-id
  workspace_id        text NOT NULL,             -- R2.5; indexed; NOT NULL enforced
  type                text NOT NULL,             -- 'task' (constant for this table)
  frontmatter         jsonb NOT NULL,            -- queryable structured fields; source frontmatter stored literally
  body                text,                      -- nullable narrative
  frontmatter_version text NOT NULL,             -- R2.6
  migrated_from       text,                      -- system field; NOT in frontmatter
  migrated_at         timestamptz,               -- system field; NOT in frontmatter
  created_at          timestamptz NOT NULL DEFAULT now(),
  updated_at          timestamptz NOT NULL DEFAULT now(),
  -- V0.1+ non-breaking additions:
  -- embedding   vector(1536)                    -- pgvector when D1 activates
  -- search_tsv  tsvector GENERATED ALWAYS AS (...) STORED

  CHECK (frontmatter ? 'id'),
  CHECK (frontmatter ? 'frontmatter_version')
);

CREATE INDEX tasks_workspace ON tasks (workspace_id);
CREATE INDEX tasks_status    ON tasks ((frontmatter->>'status'));
CREATE INDEX tasks_priority  ON tasks ((frontmatter->>'priority'));
CREATE INDEX tasks_project   ON tasks ((frontmatter->>'project_id'));
CREATE INDEX tasks_parent    ON tasks ((frontmatter->>'parent_id'));
```

**Per-artifact-type tables over polymorphic single-table.** Each plugin owns its table family. That buys per-type indexes, per-type RLS policy clarity, and per-type CHECK constraints encoding plugin-declared invariants. The polymorphic alternative adds dispatch-table generality at the cost of per-type index optimization. It isn't worth it at V0 dogfood scale (roughly 10 artifact types in migration scope).

**Mutation history under C1.** This uses a system-versioned pattern: a `tasks_history` shadow table populated by a trigger on UPDATE. You get an audit trail without changing the read path. Round-trip byte-equal frontmatter is preserved on the current row. Projections never read from history.

**Projections.** These are materialized views over the artifact tables, refreshed via a host-owned queue on host-fn write completion. The V0 refresh strategy is `REFRESH MATERIALIZED VIEW CONCURRENTLY` triggered by a host-fn write, with a background worker draining the queue. That's the seam for a V0.1+ incremental-maintenance upgrade without a contract change.

### C2 evolution path

The projection layer is the seam. At V0.1+, if aspect-specific access control, per-aspect state-shape writes, or cross-substrate projection rebuild semantics earn their keep, the projection worker can fan per-artifact writes into state and log shapes derived from the C1 content row. None of that alters the content substrate schema. The content row remains the source of truth. The evolution is additive.

### Consequences

**Good:**
- R2.7 round-trip equivalence: trivial — JSONB stores source frontmatter literally; system fields `migrated_from`, `migrated_at`, `frontmatter_version` live in dedicated columns, not in frontmatter JSONB.
- R2.4 migration parsimony: pgvector (`ALTER TABLE ADD COLUMN embedding vector(1536)`), full-text (generated tsvector column), graph edges (new edges table with FK) are all non-breaking additions.
- FK preservation (R6.6.1): FKs are JSONB references resolved at write time; single-hop validation; no cross-substrate joins.
- Lowest dogfood-cycle migration risk: `brain.db` row to C1 content row is a 1:1 column-to-frontmatter field mapping.
- RLS policy surface is small and auditable: one `workspace_id` column per artifact table, one RLS policy per table. Roughly 10 policies at V0. See [P-0009-rls-admin-token](P-0009-rls-admin-token.md) for the role model.
- Plugin contract narrow: a universal `content.emit` ABI over JSONB frontmatter + body. See [P-0003-plugin-manifest](P-0003-plugin-manifest.md).
- [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) is storage-orthogonal; no interaction with the layout choice.
- [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) `WorkspaceCtx` binding is simpler under C1: one `workspace_id` column per table, a WHERE-clause mandatory on every read path, lint-enforced.

**Bad / Trade-offs:**
- Status-churn write amplification: each status flip writes a new history row under the trigger-based history pattern. At V0 dogfood scale (a few hundred flips a day), this is negligible (under 1 MB/day). If the write rate grows, the V0.1+ work item is a numeric write budget and a potential migration to a state-shape status column (a C2-influenced partial evolution).
- JSONB query planner mis-estimates: Postgres JSONB expression indexes are mature, but the planner sometimes mis-estimates JSONB selectivity. Mitigation: explicit expression indexes per hot field (status, priority, project_id, parent_id).
- R2.4 forward-compat acceptance criterion needs strengthening: the current AC ("smoke test creates a pgvector index") proves one index works but doesn't exercise all three V0.1 promotion paths (full-text plus graph-edge promotion plus multiple embedding columns). Flagged as a follow-up: expand the AC to a fixture set covering all three paths.

## Pros and Cons of the Options

### C1 — Single-document layout (accepted)

- Pro: R2.7 trivially satisfied; JSONB stores source frontmatter literally.
- Pro: R2.4 V0.1 additions are non-breaking column additions to existing tables.
- Pro: FK preservation is single-hop; no cross-substrate join surfaces.
- Pro: Closest to `brain.db` row shape; lowest migration risk.
- Pro: Narrow plugin ABI — universal `content.emit`; the manifest declares tables + host-fns.
- Pro: RLS policy surface is small (~10 policies, one per artifact table); auditable.
- Con: Status-churn writes a new history row on each status flip. Bounded at V0 dogfood scale; numeric budget deferred to V0.1+.
- Con: JSONB planner selectivity estimates can be suboptimal without explicit expression indexes.

### C2 — Composite-with-typed-slots

- Pro: Status as a state-shape UPDATE rather than a history row per flip; better at scale.
- Pro: Per-aspect access control and per-aspect audit log possible at V0.
- Con: R2.7 frontmatter round-trip requires structural work: split frontmatter to multi-substrate, then recompose YAML byte-equal, is non-trivial.
- Con: Projection rebuild story (R2.8) requires per-aspect source-of-truth declarations not yet pinned in requirements.
- Con: Migration cutover risk is the highest: every artifact type's aspect map must land before the WC.5 canonical-day fixture passes.
- Con (now moot under [P-0010](P-0010-storage-substrate-engine.md) D8): the original con read "Postgres cannot enforce a foreign key from a TimescaleDB hypertable to a regular table; cross-substrate FK preservation is not guaranteed structurally (R6.6.1)." With TimescaleDB demoted off the V0 stack (D8, time-series uses plain timestamped Postgres tables), there's no hypertable-to-regular-table FK boundary at V0, so this specific cross-substrate FK obstacle no longer applies. The con is recorded struck-through rather than deleted, following [P-PreserveDecisionSpace](../glossary.md#p-preservedecisionspace) (keep rejected alternatives visible). It was a real C2 trade-off against the TimescaleDB-in-stack world the decision was originally made in; it stopped binding when D8 reshaped that world. It didn't affect the C1 outcome either way.
- Con: Plugin manifest schema (`aspect_map` per type) becomes substantially richer without third-party plugins to validate the surface at V0.

### C3 — Multi-substrate-with-joins

- Pro: Status-churn writes to a state-shape column rather than a history row.
- Con: Pays C2's join complexity and sidecar-table maintenance cost without C2's architectural payoff, since no actual four-shape boundary is crossed. C3 is premature extraction: sidecar tables belong when query load demonstrates the need, not before.
- Con: FK surface is multi-hop (content row to sidecar row); each join hop is a potential integrity gap.
- Con: Projection rebuild requires per-sidecar source-of-truth declarations; no simplification over C2 on rebuild semantics.

## Amendment 2026-05-24 — workspace_id type normalization

**Trigger:** Warden review d661 H3 (2026-05-24) identified a type drift. Artifact tables declared `workspace_id text NOT NULL` (per the schema pattern above), while `admin_tokens`, the `metrics` hypertable, the `events` hypertable, and `WorkspaceCtx` all use `UUID`. The drift creates cross-table join failures and removes Postgres's RFC-4122 enforcement on artifact rows.

> **Note ([P-0010](P-0010-storage-substrate-engine.md) D8 / escalation E1 — dispositioned 2026-06-09):** this amendment names the `metrics`/`events` surfaces as *hypertables*, which was the [P-0004-observability-shape](P-0004-observability-shape.md) terminology in force when it was written. P-0010's D8 demotes TimescaleDB; escalation E1 was **dispositioned 2026-06-09**: observability was re-altituded out of the project-ADR layer to the [observability baseline](../architecture/overview.md#observability) (P-0004 `deprecated`, no successor ADR), and the `metrics`/`events` surfaces are now **emitted** (stdout/OTel), not in-app hypertables. There's no in-app observability store at V0. The `workspace_id UUID NOT NULL` decision below was always independent of the hypertable-vs-table question and holds regardless; the "hypertable" wording above is the historical P-0004 terminology and no longer describes a live V0 surface.

**Decision (maintainer-locked 2026-05-24):**

- `workspace_id` SHALL be `UUID NOT NULL` in every table that references it, including artifact content tables, `_history` shadow tables, `admin_tokens`, `metrics`, `events`, `audit`, and any other table the V0 substrate defines.
- This supersedes the `workspace_id text NOT NULL` declaration in the schema pattern in "Decision Outcome" above. The SQL block is preserved for historical audit; the UUID type is the normative requirement from this amendment date forward.
- This is a downstream consequence of the C1 layout: the C1 choice is unchanged; this amendment narrows a previously-underspecified type that wasn't resolved when the ADR was authored.

**Scope of this amendment:** type normalization only. The C1 vs C2 vs C3 layout decision isn't re-opened. The amendment doesn't change which tables exist, their structure, or the RLS posture. It changes only that `workspace_id` uses `UUID` everywhere, validated at the Postgres layer.

---

## More Information

- Storage-layout candidate analysis (2026-05-03), three-candidate scoring against the V0 quality-attribute utility tree; recommended C1 for V0.
- Substrate/engine decision: [P-0010-storage-substrate-engine](P-0010-storage-substrate-engine.md) — C1 is the content-substrate layout under P-0010's Postgres implementation (D1 substrate, D5 swap trait, V0 embedded engine, D8 time-series demote). Layout sits *under* the substrate decision; this ADR doesn't re-decide the substrate.
- Frame open ADR slot: `{{P-StorageLayout}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table). Frame is Stage 2 of the work-shaping pipeline, where operating constraints are proposed and routine architecture decisions are batched. This ADR resolves that slot.
- Architecture overview ([Overview](../architecture/overview.md)): threat table entries `DS-pg-content`/T,I,R,D; trust boundary `TB-mnemra-host` to `TB-postgres`; accepted risk `R-0001` (RLS policy enforcement deferred to V0.1+).
- Downstream ADRs unlocked by this decision:
  - [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md) — cohesion criterion shaped by the C1 single-row layout.
  - [P-0003-plugin-manifest](P-0003-plugin-manifest.md) — universal `content.emit` ABI validated by C1.
  - [P-0009-rls-admin-token](P-0009-rls-admin-token.md) — RLS policy surface (one policy per artifact table) is C1-shaped.
- Cross-references: [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) (storage-orthogonal); [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) (WHERE-clause discipline on C1 tables); [P-0007-plugin-resource-limits](P-0007-plugin-resource-limits.md) (storage-orthogonal); [P-0008-admin-token-shape](P-0008-admin-token-shape.md) (the `admin_tokens` table is a C1-shaped state substrate table, not a content artifact table).
- Known weaknesses (flagged for follow-up): status-churn write-amplification budget (numeric model deferred to V0.1+); R2.4 forward-compat AC strengthening (three V0.1 promotion paths need fixture coverage).
