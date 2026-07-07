---
spec_type: architecture
frame_relevant: true
---

# Intake: Postmaster ownership marker (concurrent ci-reap safety)

**Stakes:** medium
**Date:** 2026-07-07
**Status:** locked
**Consumer:** mixed (agent-primary — the `ci` self-verify + parallel agent-ci harness; maintainer secondary)

## JTBD
When a `just ci` run fails or is interrupted and self-reaps its leaked embedded-Postgres postmasters, it must kill **only** the postmasters that *this* run spawned — so that a concurrent `just ci` on the same codebase (parallel agent-ci is permitted with no serialization, R-11) never has its live postmaster killed.

## Non-goals
- Not changing the reap **trigger** — reaping stays failure/interrupt-only, never on a clean all-gates-pass completion (the engine self-cleans on drop there).
- Not serializing agent-ci runs — R-11 parallelism is preserved.
- Not modifying `scripts/flake-runner.sh`'s separate `BASELINE_PM_PIDS` / `reap_own_postmasters` mechanism — this work is `ci`-scoped only.
- Not re-solving protection of a system or other-project Postgres — the existing temp-root check (#2119) already leaves those alone; unchanged.
- Not the implementation itself — this is a design evaluation; the chosen mechanism is built under a separate implementation task.

## Success criteria
- A concurrent run's postmaster that started **after** this run's baseline capture is **not** reaped when this run fails — demonstrable with a seeded two-run scenario driving `scripts/ci-reap.sh` through the existing `CI_REAP_PG_PATTERN` / `CI_REAP_BASELINE_FILE` test seam (`tests/ci_reap_baseline.rs`).
- This run's own leaked postmasters **are** still reaped on failure/interrupt.
- The fails-closed property is preserved: with no baseline captured, nothing is reaped.
- The ownership marker is observable by the **post-hoc bash** reap (`pgrep` / `ps -o args`), with no dependency on the spawning process still being alive at reap time.

## Hard constraints
- The reap runs **post-hoc in bash**, potentially after the spawning process has exited — so the marker MUST live in the postmaster's process command line (the `-D <data_dir>` argument) or on the filesystem, **not** in engine in-memory state.
- Postmasters are spawned by the embedded-Postgres engine via `pg_ctl`, which daemonizes every instance to PPID 1 (live or leaked) — PPID is not a usable discriminator (falsified before build; see `scripts/ci-reap.sh` "WHY NOT PPID").
- The change extends `scripts/ci-reap.sh`; it MUST NOT break the test seam (`CI_REAP_PG_PATTERN`, `CI_REAP_BASELINE_FILE`) that `tests/ci_reap_baseline.rs` drives against the real script.
- Builds on #2119 (landed): the baseline-PID-diff + temp-root-narrowing mechanism. The new marker refines condition (ii); it does not replace the mechanism.

## Evidence
- `scripts/ci-reap.sh` lines 27–35 explicitly document the residual concurrent-own-vs-other window and name #2170 as the design change that closes it.
- Surfaced by the #2119 review + engine self-flag: the concurrent-window limitation now sits on the constantly-run `just ci` path.
- R-11 permits parallel agent-ci with no serialization, so the window is live, not hypothetical.

## Risk profile
No trust boundary, PII, auth, network, or multi-tenancy surface. The only hazard is a **correctness** one in test infrastructure: mis-reaping a concurrent run's live postmaster produces a false CI failure for that other run. No security-mode review required.

## Consultations
None at intake. One backend-feasibility question is carried forward as a Frame-stage input: does the embedded-Postgres crate expose a native per-instance data-directory (or data-dir root) option, and does it read the process temp directory *live* at engine init (vs. cached at process start)? The answer bears on whether a per-run marker can be set cleanly without relocating all of the run's temp usage.

## Dismissed review flags
None — intake taken as a terse, all-firm brownfield-extension capture; review concentrated at the Frame stage.
