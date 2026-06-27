---
stakes: medium
date: 2026-06-26
status: locked
consumer: mixed   # CI gate + future implementer + maintainer/CI-trust
spec_type: code
frame_relevant: true
---

# Intake: Tier-1 CI-flake fix — serialize the PG/plugin test group

## JTBD
mnemra-core's CI cannot be trusted: the embedded-Postgres / plugin integration
test group flakes on a SIGABRT *teardown* race (#1852, ~15% on `mcp_server`),
forcing build reruns. Make the PG/plugin test group pass deterministically so a
red build signals a real failure, not concurrency noise.

## Non-goals
- Resolving the abort-frame question (Hyp A: wasmtime `PluginPool` / rmcp vs
  Hyp B: PG `Drop`) — that discriminator is separate and does not gate Tier 1.
- The Tier-2 structural refactor (shared postmaster per binary + per-test
  DATABASE, #1703, instance collapse) — separate spec.
- Any change to the per-test DB isolation model (R-0006). Tier 1 changes
  *concurrency only*; the fresh-DB-per-test guarantee is untouched.
- Disk-full / `ci.yml` free-space work — separate operational item.
- Introducing cargo-nextest as the test runner — parked to Tier-2 consideration.

## Success criteria
- SIGABRT count = **0** across a harness re-run of the serialized PG/plugin
  group (N ≥ 20/binary), measured **before/after** with `scratch/flake-runner.sh`
  (baseline: `mcp_server` 3/20 → target 0/N). Per P-TrustworthySignal, the
  measured before/after IS the deliverable, not "green now."
- **CI-runner proof (decided):** at least one Linux GitHub-runner measurement
  (`flake-runner.sh` via a temporary `workflow_dispatch` job) shows SIGABRT = 0
  for the serialized config. Local macOS before/after is the fast loop; the
  CI-runner result is the acceptance proof — the flake lives on
  Linux / 4-core / cold-cache, never previously measured.
- **Scope (decided):** the *whole* PG/plugin test-binary group is serialized
  (precautionary against the macOS→Linux surface divergence), not only
  `mcp_server`.
- The fix holds across all three CI entry points where PG binaries run:
  `verify-test`, `verify-test-hooks`, `verify-coverage`.
- Non-PG tests remain parallel — serialization is scoped to the PG/plugin group,
  not the whole workspace.
- `scratch/flake-runner.sh` reaps leaked embedded-Postgres postmasters between
  runs (it leaked zombie postmasters this session; the leak is the live
  A-10/A-12 mechanism that recurs on any abort/kill).

## Hard constraints
- **R-0018-b**: integration tests run against real embedded Postgres + pgvector
  — serialize/share, never mock or externalize.
- **P-TrustworthySignal**: a flaky gate is a trust defect; the fix is the
  *measured* deterministic proof. A rerun is not an acceptable disposition.
- The serialization mechanism must apply uniformly across `verify-test` /
  `verify-test-hooks` / `verify-coverage` (a code-traveling mechanism is favored
  over per-recipe edits — constraint, not a mechanism mandate; mechanism is
  implement-time).
- Tier 1 is a stopgap; it must not block or complicate the Tier-2 shared-instance
  refactor.

## Evidence
Bolt dispatch 1122 diagnosis (`scratch/dispatch-1122-report.md`):
- Dose-response: `mcp_server` at 1 thread → 0/20 SIGABRT; at 10 threads → 3/20
  (15%). Concurrency is the proven trigger.
- The abort fires *after every test passes* (N× `... ok`, zero `FAILED`, no
  `test result:` line, then `signal: 6`) — a teardown/exit abort, not a test
  failure.
- The existing `STARTUP_LOCK: Mutex` in the PG test binaries serializes *startup
  only*; teardown runs concurrently after the lock releases — which is why it
  does not prevent #1852.
- brain #1852 (the SIGABRT), #1860 (parent), #1864 (this fix).

## Risk profile
No trust boundary touched — test-harness concurrency change; no auth / PII /
network / runtime surface. The one adjacent risk surface (R-0006 isolation
tests) is *not* touched, because Tier 1 leaves the per-test fresh-DB isolation
model unchanged. Frame assessment: none.

## Decisions resolved at intake-exit (2026-06-26)
- Proof surface → **CI-runner + local** (maintainer).
- Serialize scope → **whole PG/plugin group** (maintainer).

## Consultations
- advisor() (2026-06-26): reframed the serialization-mechanism fork as
  implement-time HOW (kept out of spec); carved nextest to Tier 2; surfaced the
  proof-surface gap (macOS-measured vs Linux-CI flake) as the real decision;
  confirmed Tier 1 stays concurrency-only (R-0006 caveat is Tier-2) and
  independent of the Hyp A/B discriminator.

## Dismissed review flags
None. Lightweight path — the advisor pass substituted for a separate Warden
intake review at this stage; the full Warden + Bolt + Glitch review fires at
Stage 3 (spec).
