---
date: 2026-06-26
status: locked
intake: ./ci-flake-tier1-serialize.md
modulation: brownfield-extension
modulation_override: >
  Literal modulation rule (no project Frame doc → cold-start) overridden to
  brownfield-extension. mnemra-core is a pre-/brief mature project, not
  greenfield, and this work touches no architectural surface (test-harness
  concurrency only). Recorded per the /brief modulation-frontmatter requirement.
---

# Frame: Tier-1 CI-flake fix — serialize the PG/plugin test group

## Operating constraints
- **R-0018-b** (real embedded Postgres + pgvector; serialize/share, never mock or
  externalize) — `refines` the fix's mechanism space: the only admissible
  remedies keep real PG. *No conflict.*
- **P-TrustworthySignal** (a flaky gate is a trust defect; the measured
  before/after IS the deliverable) — `refines` the success criteria: green is not
  acceptance; the harness proof is. *No conflict.*
- **R-0018-f** (Justfile `verify-*` recipes; `just ci` sole CI entry) — `depends-on`:
  the fix must keep `verify-test`, `verify-test-hooks`, and `verify-coverage`
  green; serialization applies within those three recipes.
- **R-0018-c** (changes in worktrees; main protected) — process constraint;
  satisfied (this work is in a worktree off main).

## Rationale chain
Validated intent (deterministic PG/plugin test group) → constraint (R-0018-b: real
PG, cannot mock; Bolt-proven: concurrency is the trigger, teardown is the abort
site) → decision (serialize the whole PG/plugin test-binary group; the
serialization *mechanism* is implement-time HOW, left to the spec's implementer)
→ proof obligation (P-TrustworthySignal: CI-runner + local before/after, SIGABRT
= 0).

## Routine decisions (batched)
- **Modulation override** (above): no-frame-doc → cold-start overridden to
  brownfield-extension. Within-principle; no rework of done work.
- **No novel ADR.** Tier 1 introduces no new architectural decision — it is a
  test-isolation concurrency change fully bounded by existing R-0018-b / -f and
  P-TrustworthySignal. The constraint-graph walk surfaced no novel or
  escalation-triggering edge.

## Escalated decisions
None fired between gates. The two intake decisions (proof surface; serialize
scope) were resolved at the intake-exit gate.

## Risk profile (resolved)
No trust boundary. Test-harness concurrency only; the R-0006 per-test fresh-DB
isolation model is untouched (Tier 1 changes concurrency, not the isolation
mechanism). Warden security-mode review **not** triggered — no auth / PII /
network / multi-tenancy surface change. The standard Warden correctness lens
applies at Stage 3 (spec) review.
