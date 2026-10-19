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

`accepted`. This [ADR](../glossary.md#adr) (Architecture Decision Record) was authored as `proposed` on 2026-10-19 at the #4018 gate (ruling 7793, where it was listed as ADR 3). The maintainer accepted it the same day (#4018 activity 7800), together with the operator-run Postgres amendment at commit `08f4b8e`.

## Context and Problem Statement

The operator-Postgres amendment turns every delegated policy into a requirement that the host verifies before it serves anything (maintainer ruling 7261). So that check is the control standing between a misconfigured instance and a served request. And a control is software. It can wedge. It can go stale. It can over-trust a value it couldn't read, or leak what it learned to a caller who hasn't authenticated.

The reviews found every one of these in the first draft:

- A persistent check session would miss a role-level `ALTER ROLE … SET`.
- A 70-second staleness bound masked the 300-second `auth_failed` backoff.
- `not_applicable` passed for any clause.
- An unauthenticated stdio caller received the full reasons list.
- A dead checker left a stale `met` in place.

## Decision Drivers

- **A control fails closed.** It fails closed on error, on timeout, on a dead task, and on a value it couldn't read (`P-SecurityLayered`: security is layered, and each control denies rather than passes through when it fails).
- **The signal means what it reports.** A stale `met` is withdrawn, and an inconclusive result is never a pass (V-Observability, the value that an instrument must report what actually happened; `P-TrustworthySignal`, a verification signal that doesn't mean what it reports is a defect).
- **The check is keyed on what the claim is about.** Durability is checked on a session shaped like a writer's session. Readiness is judged from the consumer's side (`P-CheckCoversTheClaim`, no conclusion wider than what the check ranged over; `P-ReliabilityTarget`, a running surface is measured against the reliability it claims).
- **Testable by design.** The check is a pure decision over injected facts, so every clause has one passing and one failing unit case (`P-TestableByDesign`).
- **No configuration detail goes to an unauthenticated caller** (V-Security, the security value).

## Considered Options

1. **One bounded, supervised check over injected facts, fresh connection per run, schedule-derived staleness, coarse MCP error (chosen).**
2. **A persistent check session reused across runs, fixed staleness bound, full reasons on every surface.**
3. **No scheduled re-check: verify at boot only, rely on per-call errors to surface drift.**

## Decision Outcome

Chosen option: **1.**

The R-0117 clauses (requirement R-0117 in the V0 substrate spec named below) are evaluated as one decision over an `InstanceFacts` record. A probe fills that record on a dedicated connection that's opened for the run and closed at its end, using the serving pool's exact options.

Each clause resolves to one of four results: `met`, `unmet`, `unverifiable` or `not_applicable`. On a hard clause, `unverifiable` counts as `unmet`. `not_applicable` is permitted on exactly two clauses: R-0117-e (when the RLS manifest is empty) and the report-role presence check. Anywhere else it's `unmet`.

Every run is bounded by a 10-second deadline that sits over a 5-second connect timeout. The task is supervised, and a restart records a synthetic `unverifiable` for every clause. Readiness requires the most recent completed check to be fresh within 2 × the current scheduled interval + 10 s. Because the bound is derived from the schedule, it tracks the backoff and never masks `auth_failed`. A `SIGHUP` that arrives during a run coalesces into a single re-run.

The RLS manifest check requires the found count to equal the manifest length. It reconciles the catalog with opt-out polarity, so anything not explicitly excluded is checked.

While the host isn't ready, the MCP surface answers with `host_not_ready { kind, next_retry_s }` and nothing more. The reasons live on `GET /ready` and in `mnemra check-db`.

Every clause ships one seeded failing case. The `/ready` status code is a property test over all clause statuses, seeded with the embedded-era defect in which `/ready` returned 200 while the database was down. *(Anchors: maintainer rulings 7261 (verify, fail closed) and 7749/7761 (stay up, refuse to serve); `P-SecurityLayered`; V-Observability, meaning an inconclusive result is reported as such and a silent stop is detected; `P-TrustworthySignal`; `P-CheckCoversTheClaim`; `P-ReliabilityTarget`; `P-TestableByDesign`.)*

**Implemented by** these requirements in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`: spec R-0118-a (one check, deadline, supervision, fresh connection, coalescing), R-0118-f (staleness bound, `not_applicable` enumeration, property test, `known_gaps`), R-0118-h (coarse MCP error; reconciliation with R-0010-c), R-0117-d (writer-shaped session), R-0117-e (reconciliation), R-0117-i (hysteresis), R-0118-c (backoff) and R-0118-m (out-of-cycle check on error).

### Consequences

**Good:** the check can't pass by absence, by staleness, by a value it couldn't read, or by a session that predates a drift. An operator reads one endpoint to learn why the host isn't ready. An unauthenticated caller learns nothing.

**Bad / Trade-offs:** each run opens one short-lived connection (a connect every 30 s while serving), and that's counted in the connection budget. Requiring two consecutive successes before ready adds one interval at first start. And the two transport mechanisms deferred to implementation time show up as `known_gaps` on an otherwise `met` clause until they land.

## Pros and Cons of the Options

### Option 1 (chosen)

- Pro: every failure class the reviews found has a seeded test and a fail-closed path.
- Con: it has more moving parts than a boot-only check (a scheduler, a supervisor, a staleness clock).

### Option 2 — persistent session, fixed bound, full reasons everywhere

- Pro: fewer connects; a simpler bound.
- Con: a reused session keeps the defaults it was opened with, so it misses `ALTER ROLE … SET`. (This is Bolt's round-1 finding H1, writer-session drift, carried over onto the check.) A fixed bound masks longer backoffs. And full reasons on stdio disclose configuration to a caller the host can't authenticate while the database is down.

### Option 3 — boot-only verification

- Pro: simplest.
- Con: drift after boot gets served until some call happens to fail. The stay-up ruling needs a scheduled retry anyway, so the schedule already exists and the re-check costs nothing extra.

## More Information

- [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE3, DE4; AE1 and AE4 ruled 2026-10-19, activities 7787). The probe surface itself (`/health`, `/ready`, exec probes, listener) is ADR 7, P-0032, written with #4127.
- Threat rows: the overview's retype subsection carries `P-instance-check` (D, I) and `P-readiness-handler` (I).
