---
title: "P-0028: Requirements-check integrity"
summary: "The instance check is itself a control, so it fails closed: one implementation over injected facts, a fresh dedicated connection per run, a per-check deadline, a supervised task, unverifiable treated as unmet, not_applicable permitted on two named clauses only, readiness withdrawn when the last completed check is stale under a schedule-derived bound, SIGHUP coalesced, the MCP surface given only a coarse code while unauthenticated, and a seeded failing case per clause. Control-class ADR 3 of the operator-Postgres amendment."
primary-audience: agent
---

---
status: "accepted"
date: "2026-10-19"
decision-makers: ["the maintainer"]
consulted: ["Warden (security review)", "Bolt (operations review)", "Charon (canon review)"]
informed: []
supersedes: null
superseded_by: null
---

# P-0028: Requirements-check integrity

## Status

`accepted` — authored `proposed` 2026-10-19 at the #4018 gate (ruling 7793, ADR 3). Accepted 2026-10-19 by the maintainer (#4018 activity 7800) with the operator-run Postgres amendment at commit `08f4b8e`.

## Context and Problem Statement

The operator-Postgres amendment makes every delegated policy a requirement the host verifies before serving (maintainer ruling 7261). That check is therefore the control that stands between a misconfigured instance and a served request, and a control is software: it can be wedged, stale, over-trusting of a value it could not read, or leak what it learned to a caller who has not authenticated. The reviews found each of these in the first draft: a persistent check session would miss a role-level `ALTER ROLE … SET`; a 70-second staleness bound masked the 300-second `auth_failed` backoff; `not_applicable` passed for any clause; an unauthenticated stdio caller received the full reasons list; a dead checker left a stale `met` in place.

## Decision Drivers

- **A control fails closed** — on error, on timeout, on a dead task, on a value it could not read (`P-SecurityLayered`).
- **The signal means what it reports** — a stale `met` is withdrawn; inconclusive is never pass (V-Observability, `P-TrustworthySignal`).
- **The check is keyed on what the claim is about** — durability on a session shaped like a writer's; readiness from the consumer's side (`P-CheckCoversTheClaim`, `P-ReliabilityTarget`).
- **Testable by design** — a pure decision over injected facts, so every clause has one passing and one failing unit case (`P-TestableByDesign`).
- **No configuration detail to an unauthenticated caller** (V-Security).

## Considered Options

1. **One bounded, supervised check over injected facts, fresh connection per run, schedule-derived staleness, coarse MCP error (chosen).**
2. **A persistent check session reused across runs, fixed staleness bound, full reasons on every surface.**
3. **No scheduled re-check — verify at boot only, rely on per-call errors to surface drift.**

## Decision Outcome

Chosen option: **1.** The R-0117 clauses are one decision over an `InstanceFacts` record a probe fills on a dedicated connection opened for that run and closed at its end, with the serving pool's exact options. Results are `met` / `unmet` / `unverifiable` / `not_applicable`; `unverifiable` on a hard clause is `unmet`; `not_applicable` is permitted only on R-0117-e (empty RLS manifest) and the report-role presence check, and is `unmet` anywhere else. A 10-second deadline over a 5-second connect timeout bounds every run; the task is supervised and a restart records a synthetic `unverifiable` for every clause. Readiness requires the most recent completed check to be fresh under 2 × the current scheduled interval + 10 s, so the bound tracks the backoff and never masks `auth_failed`. `SIGHUP` during a run coalesces into one re-run. The RLS manifest check requires found-count = manifest length and reconciles the catalog with opt-out polarity. While not ready, the MCP surface answers with `host_not_ready { kind, next_retry_s }` only; the reasons live on `GET /ready` and `mnemra check-db`. Every clause ships one seeded failing case, and the `/ready` status code is a property test over all clause statuses seeded with the embedded-era 200-on-down defect. *(Anchors: maintainer rulings 7261 (verify, fail closed) and 7749/7761 (stay up, refuse to serve); `P-SecurityLayered`; V-Observability — inconclusive reported as such, a silent stop detected; `P-TrustworthySignal`; `P-CheckCoversTheClaim`; `P-ReliabilityTarget`; `P-TestableByDesign`.)*

**Implemented by** spec R-0118-a (one check, deadline, supervision, fresh connection, coalescing), R-0118-f (staleness bound, `not_applicable` enumeration, property test, `known_gaps`), R-0118-h (coarse MCP error; reconciliation with R-0010-c), R-0117-d (writer-shaped session), R-0117-e (reconciliation), R-0117-i (hysteresis), R-0118-c (backoff), R-0118-m (out-of-cycle check on error) in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`.

### Consequences

**Good:** the check cannot pass by absence, by staleness, by a value it could not read, or by a session that predates a drift; an operator reads one endpoint for the why; an unauthenticated caller learns nothing.

**Bad / Trade-offs:** one short-lived connection per run (a connect every 30 s while serving) — counted in the connection budget; two consecutive successes before ready adds one interval at first start; the two implementation-time transport mechanisms appear as `known_gaps` on an otherwise `met` clause until they land.

## Pros and Cons of the Options

### Option 1 (chosen)

- Pro: every failure class the reviews found has a seeded test and a fail-closed path.
- Con: more moving parts than a boot-only check (a scheduler, a supervisor, a staleness clock).

### Option 2 — persistent session, fixed bound, full reasons everywhere

- Pro: fewer connects; simpler bound.
- Con: a reused session keeps the defaults it was opened with and misses `ALTER ROLE … SET` (Bolt's round-1 H1, writer-session drift, moved onto the check); a fixed bound masks longer backoffs; full reasons on stdio disclose configuration to a caller the host cannot authenticate while the database is down.

### Option 3 — boot-only verification

- Pro: simplest.
- Con: drift after boot is served until a call happens to fail; the stay-up ruling needs a scheduled retry anyway, so the schedule exists and the re-check costs nothing extra.

## More Information

- [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE3, DE4; AE1 and AE4 ruled 2026-10-19, activities 7787); the probe surface itself (`/health`, `/ready`, exec probes, listener) is ADR 7, P-0032, written with #4127.
- Threat rows: overview retype subsection — `P-instance-check` (D, I), `P-readiness-handler` (I).
