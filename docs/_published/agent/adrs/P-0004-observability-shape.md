---
title: "P-0004: Observability Shape"
summary: "Defines the V0 observability deployment shape: per-verb metrics surface, TimescaleDB retention policies, health-endpoint detail body, and continuous-aggregate windows."
primary-audience: agent
---

---
status: "deprecated"
date: "2026-05-24"
decision-makers: ["the maintainer"]
consulted: ["the orchestrator", "the security reviewer"]
informed: []
supersedes: null
superseded_by: null
---

# P-0004: Observability Shape

## Status

`deprecated` — was accepted; no longer applies (the context it locked against changed). 2026-06-09.

This ADR is preserved intact as the historical record; its body is **not edited** (per the [ADR README](README.md) immutability rule and the template's frontmatter reference — an accepted ADR is not rewritten to reflect new thinking). It is `deprecated` rather than `superseded` because there is **no successor *ADR*** to point at: the observability question was re-altituded out of the project-ADR layer entirely. Two later changes falsified the world this lock was made against. First, [P-0010](P-0010-storage-substrate-engine.md) D8 demoted TimescaleDB off the V0 stack (the same `dispatch_metrics`/events surfaces this ADR froze as hypertables), which definitively falsifies this ADR's storage core (hypertables, retention policies, continuous aggregates) — so it cannot return to `accepted`. Second, the maintainer ratified (2026-06-09, the E1 disposition) that observability is a **theory trait + chassis mechanism, not a per-project ADR**: the host EMITS its telemetry storage-independently and never owns the service's own observability store. Under the constraint-graph when-to-lock edge, the falsified freeze is re-derived against the new world, not honored.

The generation-side decisions this ADR got right (the per-verb metrics surface, the health-endpoint body gating, event-schema versioning, telemetry no-leak) live now in the **architecture overview observability baseline** (a theory-trait baseline, not an ADR — see [Architecture overview › Observability](../architecture/overview.md)); the *generation mechanism* routes to the chassis when it lands. This ADR has no `superseded_by` pointer because its content's new home is a non-ADR baseline, not a replacement ADR. The decision content below is the prior, deprecated record.

## Context and Problem Statement

Mnemra-core ships observability from `0.1.0` per the workspace-wide principle: instrument before first user-touch. The Frame commits to per-verb metrics, structured logs, and a health endpoint, all backed by TimescaleDB hypertables with retention policies. The concrete surface — which metrics dimensions, what retention windows, what the health-endpoint response body looks like, and what continuous-aggregate windows are defined — must be locked before the `0.1.0` substrate build begins.

Four decisions are scoped here:

1. **Per-verb metrics surface** — which labels and measurement fields every MCP verb call records.
2. **TimescaleDB retention policies** — per-hypertable data age cap.
3. **Health-endpoint detail body** — shape of the structured response for the `/health` check.
4. **Continuous-aggregate windows** — which aggregate views are materialized over the metrics hypertable.

The Frame's threat model (via the security reviewer) adds a privacy dimension: per-verb metrics must not capture artifact bodies; audit events must not carry content fragments (`P-host-fns`/R, `DS-ts-events`/I, `DS-pg-logs`/I). The observability shape is a mitigation surface for those threats.

## Decision Drivers

- **Instrument-first.** Observability is a V0 deliverable, not V0.1+. The architecture overview quality-attribute tree names "per-verb metrics" and "retention discipline" as acceptance scenarios.
- **Tenant isolation in telemetry.** Every metric and log row must carry `workspace_id`; cross-workspace leakage via metrics is a Medium threat (overview `DS-ts-metrics`/I). The security reviewer confirmed the partition requirement in Stage 2a.
- **Telemetry no-leak.** Audit script over a dogfood-day's logs, traces, dispatch events, and session events must find zero artifact-body matches against the known-content corpus (quality-attribute scenario).
- **Retention discipline.** Every hypertable in `\d+` introspection has an `add_retention_policy` declared. This is a named acceptance scenario, not a nice-to-have.
- **Health-endpoint privacy.** The detail body must not leak substrate state to unauthenticated probes (`P-health-handler`/I, overview threat table).
- **Simplicity.** One metrics hypertable, one events hypertable, one logs table. Add hypertables only when the access pattern clearly warrants it.

## Considered Options

### Metrics surface

1. **Option A** — per-verb, per-workspace, per-outcome label set; measures: `duration_ms` (p50/p95/p99 computable from raw), `request_count`, `error_count`.
2. **Option B** — aggregate-only: only record counts per verb+outcome; no duration data.
3. **Option C** — full distributed-trace style: spans with parent-child, trace IDs, per-call-stack segments.

### Retention windows

1. **Option A** — metrics 90 days, events 365 days, logs 30 days.
2. **Option B** — all hypertables 30 days.
3. **Option C** — no retention (keep forever; operator must manage manually).

### Health-endpoint body

1. **Option A** — structured detail body: `{ dependencies: { postgres: bool, pgvector: bool, timescaledb: bool, workspace_default: bool }, overall: "ok" | "degraded" | "down" }`. Detailed body gated on admin token or loopback origin; public summary is status code only.
2. **Option B** — flat body with all details always public.
3. **Option C** — status code only (no body).

### Continuous-aggregate windows

1. **Option A** — one continuous aggregate over `DS-ts-metrics` at 1-hour resolution for the last 7 days; p50/p95/p99 per verb per workspace per hour. No additional aggregates at V0.
2. **Option B** — multiple windows: 1-hour, 24-hour, 7-day all materialized.
3. **Option C** — no continuous aggregates; all queries run against raw hypertable.

## Decision Outcome

**Metrics surface: Option A** — per-verb, per-workspace, per-outcome with `duration_ms`, `request_count`, `error_count`. The raw-data model lets any percentile window be derived; aggregate-only discards the distribution. Full distributed traces (Option C) are premature for V0 single-process deployment.

**Retention windows: Option A** — 90-day metrics, 365-day events, 30-day logs. Dogfood-cycle correctness requires 90 days of metrics history for the "p50/p95/p99 per verb for the last 24h of dogfood" AC scenario; events carry dispatch and security audit data needed for 365-day retrospectives; logs are operational diagnostics with a shorter useful life. Option B (all 30 days) would truncate events before meaningful audit windows. Option C (no retention) violates the "every hypertable has an `add_retention_policy` declared" acceptance scenario.

**Health-endpoint body: Option A** — structured detail with admin/loopback gating. The full detail body (postgres reachable / extensions loaded / workspace=default exists) is the quality-attribute acceptance scenario verbatim; gating on admin token or loopback origin addresses the `P-health-handler`/I threat (unauthenticated probe oracle for substrate state). Option B leaks. Option C fails the acceptance scenario.

**Continuous-aggregate windows: Option A** — single 1-hour window over the metrics hypertable. One aggregate covers the primary query pattern (24h lookback at hourly granularity); multiple materialized windows (Option B) add maintenance cost for patterns not yet observed in dogfood; raw queries (Option C) add per-request cost for every dashboard probe.

### V0 default values

| Parameter | V0 value | Rationale |
|---|---|---|
| Metrics hypertable chunk interval | 1 day | Default TimescaleDB recommendation for moderate-write workloads |
| Metrics retention | 90 days | Covers the 24h dogfood AC window with comfortable headroom |
| Events hypertable chunk interval | 7 days | Events are lower-frequency than metrics |
| Events retention | 365 days | Dispatch and security audit events have a 1-year look-back requirement |
| Logs table retention (Postgres, not hypertable at V0) | 30 days | Operational logs; diagnostic value drops sharply after 30 days |
| Continuous-aggregate window | 1 hour | Covers `now() - interval '24 hours'` range queries with hour-grain precision |
| Continuous-aggregate lookback | 7 days (materialized range) | Covers typical dogfood debug window; older data falls through to raw hypertable |
| Health detail gate | admin token OR `127.0.0.1/::1` source | Structural in the health handler; fails closed on any non-admin, non-loopback probe |

### Metrics row schema (V0 floor)

```
workspace_id      UUID NOT NULL
verb              TEXT NOT NULL          -- e.g., "task.list", "task.create"
outcome           TEXT NOT NULL          -- "ok" | "error" | "timeout"
duration_ms       INT4 NOT NULL          -- wall-clock milliseconds
recorded_at       TIMESTAMPTZ NOT NULL   -- hypertable partition key
```

No artifact IDs, no content fragments, no agent identity in the metrics row. Per-agent attribution is the `DS-ts-events` surface (dispatch and session events carry `(workspace_id, agent_id, session_id)`), not the metrics surface.

### Events row schema (V0 floor)

```
workspace_id      UUID NOT NULL
event_type        TEXT NOT NULL          -- versioned; see event-type registry at spec stage
event_version     INT2 NOT NULL DEFAULT 1
agent_id          UUID                   -- nullable; not all events are agent-originated
session_id        UUID                   -- nullable; same
payload           JSONB NOT NULL         -- typed per event_type; never contains artifact bodies
recorded_at       TIMESTAMPTZ NOT NULL   -- hypertable partition key
```

`event_version` is the `DS-ts-events`/T mitigation: event-type evolution is controlled; backward-compatible additions are minor bumps; breaking changes version the type.

### Consequences

**Good:**
- The retention-discipline acceptance scenario is satisfied structurally — every hypertable has an `add_retention_policy` declared at schema initialization.
- Telemetry no-leak acceptance criterion is addressable: metrics rows carry no content; events rows use typed payload with content-IDs only.
- Health-endpoint privacy threat (`P-health-handler`/I) mitigated via gating.
- Cross-tenant metrics leak (`DS-ts-metrics`/I) mitigated: `workspace_id` is a required column on every row; query projections must always partition by it.
- Event schema versioning (`DS-ts-events`/T) is built into the schema floor from V0.

**Bad / Trade-offs:**
- Logs at V0 land in a Postgres table, not a TimescaleDB hypertable. The log-shape hypertable option is in the Frame's notes on `DS-pg-logs`; this decision defers the hypertable promotion to `{{P-ObservabilityShape}}` revision if the V0 log write-rate warrants it. Accepted at V0 given the 30-day retention and diagnostic-only use.
- Continuous aggregate at one 1-hour window means sub-hourly queries run against the raw hypertable. For V0 single-deployment dogfood this is acceptable; if query frequency warrants finer granularity it's a non-breaking addition (new continuous aggregate added via migration without changing the retention policy).

## Pros and Cons of the Options

### Metrics surface — Option A (per-verb, per-workspace, per-outcome with `duration_ms`)

- Pro: Any percentile computation is derivable from raw `duration_ms` values; no information is discarded at write time.
- Pro: `workspace_id` + `verb` + `outcome` triple is the minimal label set that answers the dogfood AC query (`p50/p95/p99 per verb for the last 24h`).
- Con: Slightly more write cost per MCP call vs. aggregate-only; acceptable for V0 load envelope.

### Metrics surface — Option B (counts only)

- Con: Percentile queries (p50/p95/p99) cannot be answered from count-only data; fails the dogfood AC.

### Metrics surface — Option C (distributed traces)

- Con: Per-span trace storage is significantly higher write volume; adds trace-propagation complexity to every MCP call; not needed for V0 single-process single-tenant dogfood.

### Retention — Option A (90/365/30)

- Pro: Aligns to the actual useful-life of each data class; metrics have a shorter operational look-back than security audit events.
- Con: Three different retention windows to manage; mitigated by declaring them in schema initialization, not ad-hoc.

### Retention — Option B (all 30 days)

- Con: Truncates events before a meaningful security-audit window (dispatch events, admin-action events).

### Retention — Option C (no retention)

- Con: Violates the "every hypertable has an `add_retention_policy` declared" acceptance scenario; disk growth is unbounded.

### Health body — Option A (structured, admin/loopback-gated)

- Pro: Satisfies the acceptance scenario verbatim; privacy threat mitigated.
- Con: Adds a small auth-check step in the health handler (negligible cost).

### Health body — Option B (all details always public)

- Con: Leaks substrate state to unauthenticated probes; violates `P-health-handler`/I mitigation.

### Health body — Option C (status code only)

- Con: Fails the acceptance scenario ("a request to `/health` returns 503 with a structured detail body identifying which dependency failed").

### Continuous aggregate — Option A (1-hour, 7-day lookback)

- Pro: Matches the primary query pattern; minimal maintenance cost.
- Con: Sub-hourly granularity requires raw-hypertable fallback; not a V0 concern.

### Continuous aggregate — Option B (multiple windows)

- Con: Adds TimescaleDB maintenance overhead (background materialization jobs) for patterns not yet observed.

### Continuous aggregate — Option C (no aggregates)

- Con: Every dashboard or metric-AC query pays full hypertable scan cost; unacceptable as load scales.

## More Information

- Frame doc open ADR slot: `{{P-ObservabilityShape}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table).
- Overview threat references: `P-host-fns`/R, `P-health-handler`/I,S, `DS-ts-metrics`/I, `DS-ts-events`/T,I, `DS-pg-logs`/I ([Overview](../architecture/overview.md)).
- Accepted risk `R-0005` in overview: external-LLM embedding calls; compensating control includes typed `payload` in events schema with no content bodies.
- Accepted risk `R-0001` in overview: RLS policy enforcement deferred; metrics + events rows carry `workspace_id` as a WHERE-clause-mandatory column per the V0 WHERE-clause discipline.
- V0 log-shape backend choice (Postgres table vs TimescaleDB hypertable for `DS-pg-logs`) is a Spec-stage detail; this ADR names the table-for-now posture with an explicit migration path if write rate warrants hypertable promotion.
- `{{P-SigningKeyCustodyHardening}}` (Tier C, deferred): future audit-event shape for signing events will follow the events schema defined here.
