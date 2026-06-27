---
date: 2026-06-26
status: approved
reviewed_by: Peter
reviewed_date: 2026-06-26
locked: 2026-06-26
spec_type: bugfix
severity: medium
intent: ../intent/ci-flake-tier1-serialize.md
frame: ../intent/ci-flake-tier1-serialize-frame.md
frame_version: 2004d97083865629a18e9e998c0da2c880ec7661
supersedes: none
---

# Fix: Tier-1 CI flake — serialize the PG/plugin test group

> **Spec for:** issue #1864 (this fix) — child of #1860; targets #1852 (the SIGABRT)
> **Date:** 2026-06-26
> **Locked:** 2026-06-26 (spec-exit gate, maintainer-approved)
> **Severity:** medium
> **Frame:** [`docs/intent/ci-flake-tier1-serialize-frame.md`](../intent/ci-flake-tier1-serialize-frame.md), locked at blob SHA `2004d97083865629a18e9e998c0da2c880ec7661`
> **Intake:** [`docs/intent/ci-flake-tier1-serialize.md`](../intent/ci-flake-tier1-serialize.md)
> **Evidence:** the embedded-Postgres flake diagnosis (`scratch/dispatch-1122-report.md`, diagnosis-only; gitignored scratch — every load-bearing number is inlined below so this spec stands without it)
> **Deliverable harness:** the measurement instrument is a **tracked** tool at `scripts/flake-runner.sh` (R-0024), derived from the diagnosis-era copy — committed, code-reviewable at PR time, and resolvable from repo root in any worktree or CI runner

## Purpose

mnemra-core's CI cannot be trusted: the embedded-Postgres / plugin integration test group flakes on a SIGABRT *teardown* race (#1852, ~15% on `mcp_server`), forcing build reruns. This fix serializes the whole embedded-Postgres / plugin test-binary group so the group passes deterministically — a red build then signals a real failure, not concurrency noise.

This is **Tier 1**: a concurrency-only stopgap. It changes test concurrency and nothing else — the per-test fresh-database isolation model (R-0006) and the embedded-Postgres-vs-mock decision (R-0018-b) are untouched. The structural shared-instance refactor (#1703) is a separate spec (see Out of Scope). The proof obligation is a *measured before/after* on both the local fast loop and a Linux CI runner; per P-TrustworthySignal, the measurement is the deliverable, not "green now."

The serialization **mechanism** (e.g. a per-test serial gate vs a single-thread harness setting vs a runner test-group) is implement-time HOW and is deliberately left to the implementer. This spec states the *constraints* the mechanism must satisfy (R-0021, R-0022, R-0025), not the mechanism.

RFC-2119 keywords are used throughout. `SHALL`/`MUST` = mandatory; `SHALL NOT`/`MUST NOT` = prohibited; `SHOULD` = preferred; `MAY` = optional.

## Bug

**Observed.** After every test in `mcp_server` passes (the run logs show N× `... ok`, **zero** `FAILED`, **no** `test result:` summary line), the test binary aborts with `process didn't exit successfully: ... (signal: 6, SIGABRT)`. It is a teardown/exit abort, not a test failure. Rate: ~15% (3/20) at default parallelism. CI treats it as a red build and reruns are budgeted to clear it.

**Expected.** The embedded-Postgres / plugin test group passes deterministically: SIGABRT count = 0 across repeated runs, so a red build means a real failure.

**Impact.** The CI verification signal (`just ci`, the gated merge line) is nondeterministic — every green is acted upon, and a gate that flips trains the team to distrust red (P-TrustworthySignal, on the P-MainProtected line). Build reruns are an unfiled defect, not an accepted disposition.

**Root-cause class (from the diagnosis, `scratch/dispatch-1122-report.md` §2).** A *concurrency-gated teardown race*. Dose-response is the proof: `mcp_server` at 1 thread → 0/20 SIGABRT; at 10 threads → 3/20 (15%); the abort rate tracks the concurrent live-postmaster count. The existing per-binary `STARTUP_LOCK: Mutex` (`libs/mnemra-host/tests/postgres_engine.rs:52`, `libs/mnemra-host/tests/mcp_server.rs:106`) serialises *startup only*; teardown runs concurrently after the lock releases, which is why it does not prevent #1852.

**The confound (diagnosis §2 — the load-bearing evidence).** `admin_token` constructs **13** embedded-Postgres engines per binary run and aborts **0/20**; `mcp_server` constructs **14** and aborts **3/20 (15%)**. Equal engine *count* with a different abort rate refutes "`mcp_server` aborts because it starts *more* PG engines" — it does **not** refute "PG `Drop` is the aborting resource." `mcp_server` uniquely constructs a `wasmtime PluginPool` and an `rmcp RunningService` per test, either of which could be the aborting frame. So the exact aborting frame — **Hyp A** (a non-PG `wasmtime PluginPool` / `rmcp` drop → a PG-only Tier-2 fix would *not* stop #1852) vs **Hyp B** (PG `Drop`, with `mcp_server` simply reaching higher concurrent teardown overlap → Tier 2 would) — is *genuinely* unresolved, not merely unexamined. It **does not gate this fix**: removing concurrent teardown eliminates the abort regardless of frame. The frame-discriminator therefore stays Out of Scope as a real safety check, load-bearing *before* Tier 2 is relied on as the #1852 remedy — not a formality.

## Reproduction

**Given** the embedded-Postgres / plugin test group at this SHA, on a multi-core host, with the test binaries run at default (parallel) test-thread concurrency.
**When** `mcp_server` is run N = 20 times via the harness at default threads (`bash scripts/flake-runner.sh 20 default mcp_server`, invoked from repo root).
**Then** ≥ 1 run aborts with `signal: 6` after all tests pass (recorded: 3/20 at default threads, `scratch/dispatch-1122-report.md` §1).

## Requirements

These requirements continue the project's global `R-NNNN` series (the V0 substrate spec, `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`, owns `R-0001`–`R-0019`); this spec owns `R-0020`–`R-0025`.

### R-0020 — Deterministic pass: serialization drives SIGABRT to zero, proven before/after

**Decision.** Serializing the embedded-Postgres / plugin test group (R-0021) SHALL eliminate the #1852 SIGABRT teardown race, and the fix SHALL be accepted on a *measured before/after*, not on a single green run. *Anchors: P-TrustworthySignal — the measured before/after is the deliverable; rerun-to-green and accept-as-residual-risk are the two forbidden dispositions. R-0018-b — the proof runs against real embedded Postgres, never a mock.*

**Acceptance Criteria:**
- [ ] A pre-fix "before" is on record: at parallel (default) concurrency, the group produces ≥ 1 SIGABRT across N = 20 runs (recorded: `mcp_server` 3/20, `scratch/dispatch-1122-report.md` §1). The "before" MAY be the recorded baseline or a fresh parallel-config measurement on the implementer's box. (Only `mcp_server`, `admin_token`, `tenancy_isolation`, and `storage_contract_postgres` have a measured before-state; the other 10 members were added precautionarily per R-0021 — their expected before-state is 0/N, and no before-measurement is required for a member with no observed SIGABRT history.)
- [ ] The "after": a re-run of `scripts/flake-runner.sh` over **every** member binary of the PG/plugin group (R-0021), N ≥ 20 runs per binary, reports SIGABRT count = **0**.
- [ ] SIGABRT is counted as runs the harness classifies **ABORT** in its summary table — the classifier matches a log containing `signal: 6`, `SIGABRT`, or `process abort signal` (`scripts/flake-runner.sh`) — **not** by process exit code (a `cargo test` parent masks a signal-6 child as its own exit 101, so exit code is unreliable; see the classifier note in `scratch/dispatch-1122-report.md`).
- [ ] Both the before and after numbers are captured in the fix's regression evidence (a record committed or transcribed with the PR), not asserted.

### R-0021 — Serialization scope = the whole PG/plugin test-binary group; non-PG tests stay parallel

**Decision.** Serialization SHALL cover the *entire* group of test binaries that start an embedded Postgres engine — not only `mcp_server` — and SHALL NOT serialize binaries outside that group. *Anchors: maintainer decision at intake-exit (serialize the whole group, precautionary against the macOS→Linux surface divergence — `scratch/dispatch-1122-report.md` risk notes). R-0018-b — the group is exactly the real-PG integration suite. P-TrustworthySignal — the proof surface is the whole group, not the single binary that happened to flake locally.*

**Group membership (the checkable criterion).** A test binary is a **member** of the PG/plugin group **iff** its compiled test target constructs an embedded Postgres engine — i.e. it references `EmbeddedEngine`, a `start_engine()`-style helper, or the `postgresql_embedded` crate. The criterion is the source of truth; the enumeration below is the known membership at this SHA and SHALL be reconciled against the criterion at implement time.

**Known members (14, in `libs/mnemra-host/tests/`):** `admin_token.rs`, `admin_token_behavior.rs`, `artifact_machinery.rs`, `content_schema.rs`, `identity_builtins.rs`, `invoke_health_gate.rs`, `mcp_server.rs`, `mcp_slice1_e2e.rs`, `mcp_verb_gate.rs`, `postgres_engine.rs`, `schema_init.rs`, `startup_population.rs`, `storage_contract_postgres.rs`, `tenancy_isolation.rs`.

**Acceptance Criteria:**
- [ ] **Coverage (binary gate = SIGABRT 0).** The membership criterion enumerates exactly the serialized set: `grep -rln 'EmbeddedEngine\|postgresql_embedded\|start_engine' libs/mnemra-host/tests/ | grep -v '/common/'` lists the members, and the serialization mechanism covers every one of them. The acceptance gate per member binary is **SIGABRT = 0** (R-0020). *(Diagnostic, not a gate: the harness's `peak-above-baseline PEAK_DELTA` should drop to a small single-digit count — `PEAK_DELTA ≤ 2` is consistent with one active engine plus one tearing down in the serialized transition window, contrast `mcp_server`'s pre-fix `+24` at default threads, `scratch/dispatch-1122-report.md` §1. A `PEAK_DELTA` above ~4 with SIGABRT = 0 warrants investigation but does not block acceptance.)*
- [ ] **Scoping (serialization is real, not workspace-global) — empirical check is PRIMARY.** The primary gate: a designated non-member binary (e.g. the 39-test non-PG `abi_contract.rs`) runs its tests with parallelism > 1 under the post-fix config (observed peak > 1 concurrent test, or its worker-thread budget unchanged from the cargo default). Secondary (code-review grep backstop): no *workspace-global* single-thread/serial directive exists — `grep -E 'test-threads|RUST_TEST_THREADS' justfile` finds no directive applied to a whole-workspace `cargo test` / `cargo llvm-cov` run (catches both `--test-threads=1`/`--test-threads 1` and `RUST_TEST_THREADS=1`), and `.cargo/config.toml` either does not exist or carries no `[build]/[test]` thread override (`test -f .cargo/config.toml && grep -E 'threads?' .cargo/config.toml || true` is empty).

### R-0022 — Serialization holds uniformly across all three CI entry points that run PG binaries

**Decision.** Serialization SHALL be in force wherever the PG/plugin binaries execute under CI — `verify-test`, `verify-test-hooks`, and `verify-coverage` — and SHALL NOT be silently absent from any of the three. A *code-traveling* mechanism (one that travels with the test code so any invocation path inherits it) is favored over per-recipe flag edits; this is a constraint on the mechanism's *reach*, not a mandate of any specific mechanism. *Anchors: R-0018-f — the `verify-*` recipes; `just ci` is the sole CI entry. Intake hard constraint — uniform across the three recipes; code-traveling favored.*

**Acceptance Criteria:**
- [ ] `just verify-test`, `just verify-test-hooks`, and `just verify-coverage` each complete post-fix with SIGABRT = 0 for all member binaries (zero ABORT classifications, per R-0020's classifier, in any of the three recipes' output).
- [ ] Uniformity is proven by the three measured runs above, regardless of mechanism type — the per-recipe SIGABRT = 0 measurement is the verification, and is required even when the mechanism is claimed "code-traveling." A code-traveling construction is a forward-guarantee that a future 4th invocation path inherits the serialization; it is **not** a substitute for running and observing all three recipes. If the mechanism is recipe-level, all three recipes additionally carry the identical directive (grep shows the same directive in each of `verify-test`, `verify-test-hooks`, `verify-coverage` in the `justfile`).

### R-0023 — CI-runner proof obligation (Linux GitHub runner, SIGABRT = 0)

**Decision.** Acceptance REQUIRES at least one **Linux GitHub-runner** measurement showing SIGABRT = 0 for the serialized configuration. The local before/after (R-0020) is the fast loop; the CI-runner result is the acceptance proof — the flake lives on Linux / 4-core / cold-cache / coverage-instrumented startup, never previously measured directly. (macOS surfaces startup-under-load as SysV shared-memory exhaustion under a low `SHMMNI`, while Linux surfaces the *same* driver as the 60s startup deadline — the two platforms look qualitatively different, not merely different-rate — and the SIGABRT itself has only ever been observed on macOS, so the Linux-true SIGABRT rate is exactly what this proof establishes; a local measurement is one-OS / one-machine and cannot be read as the CI rate.) *Anchors: P-TrustworthySignal — prove the fix where the signal is consumed (the gated CI line). P-IterateToZero — prove-by-measurement. Maintainer decision at intake-exit (proof surface = CI-runner + local).*

**Acceptance Criteria:**
- [ ] A temporary `workflow_dispatch` job named **`flake-proof-tier1`** runs the harness (`scripts/flake-runner.sh`, or an equivalent in-CI loop) over the PG/plugin group on a Linux GitHub runner, and the job log shows SIGABRT count = **0** (zero ABORT classifications, per R-0020's classifier) across every member binary.
- [ ] **Runtime tiering (to fit GitHub's 360-minute per-job cap; ~280 sequential `cargo test` runs at full N ≈ 3.5–5h, tight against the cap).** The 4 members with a measured before-baseline (`mcp_server`, `admin_token`, `tenancy_isolation`, `storage_contract_postgres`) run at **N ≥ 20**; the 10 precautionary members MAY run at a documented reduced floor of **N ≥ 5** (the exact N recorded in the proof evidence). The job SHALL set an explicit `timeout-minutes` (raised above the 360-min default if the chosen N requires it) and SHOULD cache the Rust build so cold-build time does not consume the budget. *(This adjusts the maintainer's original "N ≥ 20/binary" CI bar for the 10 precautionary members — flagged in Decisions of note for spec-exit confirmation.)*
- [ ] The CI measurement is captured as acceptance evidence (job log or run artifact retained, or transcribed into the regression record), not asserted.
- [ ] The temporary proof job is **removed** after the measurement — it is a one-shot proof harness, not a standing CI gate. (Binary check: `grep -rl 'flake-proof-tier1' .github/workflows/` returns nothing; equivalently `git diff <parent_commit> HEAD -- .github/workflows/` shows no net-added proof workflow.)

### R-0024 — The measurement instrument reaps leaked embedded-Postgres postmasters between runs

**Decision.** The measurement instrument SHALL be a **tracked** file at `scripts/flake-runner.sh` (committed, not in gitignored `scratch/`), so the before/after measurement (R-0020), the CI proof (R-0023), and the reaping behavior below are bound to an artifact that is code-reviewable at PR time, diffable, and resolvable from repo root in any worktree or CI runner. It SHALL reap **only the orphaned embedded-Postgres postmasters it itself spawned** — tracked against `BASE_PM`, the pre-run baseline count it records at startup — between binary invocations, so an aborted or killed run's leaked postmasters cannot corrupt the next invocation's baseline, and so it never touches pre-existing postmasters that belong to the developer's environment (the diagnosis found 8 pre-existing zombies at session start). Without reaping, the before/after measurement that *is* the deliverable (R-0020) is invalid: a leaked-instance backlog surfaces as SysV shared-memory exhaustion (macOS) or a startup deadline (Linux) and masks the true SIGABRT rate. *Anchors: P-TrustworthySignal — the instrument must be faithful; an instrument that contaminates its own baseline, or that is uncommittable and so unreviewable, reports an untrustworthy rate (the Observability faithful-instrument clause this principle operationalizes).*

**Acceptance Criteria:**
- [ ] **Tracked.** The harness is a committed file at `scripts/flake-runner.sh` and appears in the PR diff (`git ls-files scripts/flake-runner.sh` returns the path). The diagnosis-era copy under gitignored `scratch/` is not the deliverable. Because the script *is* the instrument, its reap behavior is reviewable directly in the diff, and additionally evidenced by the back-to-back run below.
- [ ] **Reap own-spawned excess, between binary invocations.** After each member-binary invocation in which the process aborts or exits non-zero, the harness terminates any `bin/postgres` postmasters it spawned in excess of `BASE_PM` before starting the next binary invocation — at inner-loop (per-invocation) granularity, not only between full N-iterations. The live-postmaster count returns to ≤ `BASE_PM` between invocations (no monotonic accumulation across the run); postmasters at or below `BASE_PM` are never killed.
- [ ] **Two-condition non-contamination (crash precondition stated).** A back-to-back two-condition run (parallel then serialized) does not fail the second condition with shared-memory-exhaustion / startup-deadline caused by the first condition's leaked postmasters. This is only a meaningful test if the first (parallel) condition produces ≥ 1 abort to leak postmasters — the pre-fix 15% rate makes ≥ 1-in-20 likely but not certain; if the parallel condition produces no abort, force the leak with a manual `kill -SIGKILL <postmaster-pid>` mid-run, or assert the stronger invariant directly: after every run (pass or fail), the live-postmaster count equals `BASE_PM`. (Pre-fix evidence of the leak: 8 → 30 zombie postmasters accumulated in a single session, `scratch/dispatch-1122-report.md` §1 and followups.)

### R-0025 — Concurrency-only: the per-test fresh-DB isolation model SHALL NOT change

**Decision.** Tier 1 changes test *concurrency* only. The fix SHALL NOT alter the per-test fresh-database isolation model (each test starts a fresh embedded engine + database and seeds under the same `DEFAULT_WORKSPACE_ID` constant, so isolation derives purely from a separate database per test) that the R-0006 tenant-enforcement tests rely on. This bounds Tier 1 to its stopgap role and keeps the Tier-2 shared-instance refactor unblocked. *Anchors: R-0006 — tenant enforcement; the isolation tests must keep seeing a clean DB per test. R-0018-b — real PG, per-test isolation unchanged. Intake hard constraint — Tier 1 is concurrency-only and must not block or complicate Tier 2.*

**Acceptance Criteria:**
- [ ] The diff introduces no change to the per-test database/engine construction: the `start_engine()` / fresh-DB-per-test path is unmodified except for whatever the serialization mechanism gates; there is no shared-database or shared-`workspace_id` substitution.
- [ ] The R-0006 cross-workspace isolation tests pass unchanged post-fix: the `assert_isolation_no_cross_workspace_read` / `assert_isolation_no_cross_workspace_write` helpers (`libs/mnemra-host/tests/common/mod.rs:135`, `:177`) and their **PG call site** (`libs/mnemra-host/tests/storage_contract_postgres.rs`, a group member) are green and unedited. (The same helpers are also invoked from `libs/mnemra-host/tests/storage_contract.rs`, which exercises `MemStorage` and is a non-member unaffected by this fix; it too remains green.) Verified via `git diff <parent_commit> -- libs/mnemra-host/tests/common/mod.rs libs/mnemra-host/tests/storage_contract_postgres.rs` showing no changes to those files.
- [ ] `just verify-lint` (which includes the R-0006-d read-path WHERE-clause lint) remains green post-fix.

## Scenarios

**S1 — The bug (baseline reproduction).**
*Given* the PG/plugin group at parallel concurrency on a multi-core host,
*When* `mcp_server` runs N = 20 times at default threads,
*Then* ≥ 1 run aborts with `signal: 6` after all tests pass (3/20 recorded). [R-0020 "before"]

**S2 — Serialized group passes deterministically (local after).**
*Given* the post-fix serialization in force over the whole PG/plugin group,
*When* `scripts/flake-runner.sh` runs each member binary N ≥ 20 times,
*Then* SIGABRT count = 0 across the group (the binary acceptance gate); the harness's `peak-above-baseline PEAK_DELTA` per member drops to a small single-digit count (`≤ 2`, diagnostic, not a gate). [R-0020, R-0021]

**S3 — CI-runner proof (Linux).**
*Given* the temporary `workflow_dispatch` job `flake-proof-tier1` running the harness on a Linux GitHub runner,
*When* the serialized PG/plugin group runs at the R-0023 tiered N (≥ 20 for the 4 measured members, ≥ 5 for the 10 precautionary) within an explicit `timeout-minutes`,
*Then* the job log shows SIGABRT = 0 for every member binary, and the job is removed afterward. [R-0023]

**S4 — Non-PG tests stay parallel (scoping holds).**
*Given* the post-fix config,
*When* a non-member binary (e.g. `abi_contract.rs`) runs,
*Then* its tests execute with parallelism > 1, and no workspace-global single-thread directive is present in any of the three recipes. [R-0021]

**S5 — Uniform across the three recipes.**
*Given* the post-fix config,
*When* `just verify-test`, `just verify-test-hooks`, and `just verify-coverage` each run,
*Then* each completes with SIGABRT = 0 for all member binaries. [R-0022]

**S6 — Instrument reaps leaked postmasters.**
*Given* a harness run in which a member binary aborts and leaks postmasters,
*When* the next run begins,
*Then* the leaked `bin/postgres` postmasters have been reaped and the live-postmaster count is back at baseline; the next run does not fail on shared-memory exhaustion / startup deadline caused by the prior run. [R-0024]

**S7 — Isolation model unchanged (regression guard).**
*Given* the post-fix diff,
*When* the R-0006 cross-workspace isolation tests and `verify-lint` run,
*Then* they pass unchanged, and the per-test fresh-DB construction is unmodified. [R-0025]

## Out of Scope

Each item below is excluded from this fix. Deferrals carry a firing tripwire; items with no mechanical firing condition are labelled **parked** with the named cadence that resurfaces them (per the decide-and-lock anti-hedge: a tripwire that depends on "someone remembers" is a parked item, not a deferral).

- **The abort-frame discriminator** (Hyp A: a non-PG `wasmtime PluginPool` / `rmcp` drop aborts → Tier 2 alone would not fix #1852; vs Hyp B: PG `Drop` aborts → Tier 2 would). Excluded because Tier-1 serialization removes all concurrent teardown and eliminates the abort *regardless* of frame. **Deferred** — *tripwire:* the frame SHALL be isolated (per-binary peak-concurrency attribution + an abort backtrace on a clean box or the Linux runner, `scratch/dispatch-1122-report.md` §6) **before** Tier 2 is committed as the #1852 remedy. Firing gate: the Tier-2 spec's decision gate, tracked under #1703.

- **The Tier-2 structural refactor** — shared postmaster per binary + per-test `DATABASE`, ordered async teardown, instance collapse (#1703). Excluded as a separate, multi-file spec; Tier 1 must not block it (R-0025). **Parked** — cadence: authored as its own spec, tracked under #1703.

- **Any change to the R-0006 per-test fresh-DB isolation model.** This is a permanent cut *for Tier 1* (Tier 1 is concurrency-only; R-0025 is the in-scope guard that the fix does not regress it). The isolation model is re-opened only by the Tier-2 per-test-`DATABASE` design. **Parked** — cadence: the Tier-2 spec, which SHALL re-verify the R-0006 isolation tests against its shared-instance shape (#1703).

- **Disk-full / `ci.yml` free-disk-space work** (free-space reclaim step, `df -h` measurement, dual cargo-target-tree footprint). A separate operational footprint problem that will not dissolve under serialization or instance collapse. **Deferred** — *tripwire:* a separate operational `ci.yml` task (`scratch/dispatch-1122-report.md` §4); fired when CI hits disk pressure or at Tier-2 scheduling, tracked as its own item under #1860.

- **cargo-nextest adoption** (process-per-test isolation; would surface a teardown abort as a single failed test instead of a binary-wide abort). Excluded as a runner change with its own cost. **Parked** — cadence: considered at Tier-2 design time (it is a candidate Tier-1-alternative / Tier-2 input, not a Tier-1 deliverable), tracked under #1703.

## Decisions of note

| Decision | Resolution | Anchor |
|----------|-----------|--------|
| Proof surface | CI-runner **and** local before/after (R-0020, R-0023) — local is the fast loop, the Linux runner is the acceptance proof | P-TrustworthySignal; maintainer (intake-exit) |
| Serialize scope | The **whole** PG/plugin test-binary group, defined by a membership criterion (R-0021), not only `mcp_server` | maintainer (intake-exit); R-0018-b |
| Serialization mechanism | **Implement-time HOW — out of this spec.** The spec states constraints (uniform across the three recipes, code-traveling favored, scoped so non-PG stays parallel, isolation untouched), not the mechanism | intake hard constraint; decide-and-lock (mechanism is the implementer's) |
| Tier boundary | Tier 1 is concurrency-only; isolation model (R-0006) untouched and Tier 2 (#1703) unblocked (R-0025) | R-0006; intake hard constraint |
| Abort frame | Unresolved and **not gating** this fix — serialization removes concurrent teardown regardless of frame; the confound (admin_token 13 engines→0/20 vs mcp_server 14→3/20) is inlined in the Bug section | `scratch/dispatch-1122-report.md` §2 |
| Harness location | Promote the measurement instrument from gitignored `scratch/` to the tracked path `scripts/flake-runner.sh` (R-0024), so the proof obligation binds to a reviewable, diffable, CI-resolvable artifact | P-TrustworthySignal (faithful, reviewable instrument); review trio (Warden M-1, Glitch F1/F2) |

**Confirmed at spec-exit (2026-06-26, maintainer-approved).** Three items surfaced for the maintainer were confirmed:

1. **CI proof-job N-tiering — CONFIRMED.** R-0023's tiered CI proof N (N ≥ 20 for the 4 members with a measured before-baseline — `mcp_server`, `admin_token`, `tenancy_isolation`, `storage_contract_postgres`; N ≥ 5 for the 10 precautionary members, with a raised `timeout-minutes`) is the accepted CI bar. Local before/after (R-0020) is unchanged at N ≥ 20/binary. *(Raised by Bolt F-02 runtime analysis: ~280 sequential runs ≈ 3.5–5h at full N, against GitHub's 360-minute per-job cap.)*
2. **R-0025 kept as a separate requirement — CONFIRMED.** Retained as its own `SHALL NOT` requirement (the review trio confirmed it is correctly scoped — Warden, Glitch), not folded into R-0021.
3. **R-ID global-series continuation — CONFIRMED.** This spec owns R-0020–R-0025 in the project's single global R-namespace (the V0 substrate spec owns R-0001–R-0019; nothing else uses the series).

**No novel ADR.** Tier 1 introduces no new architectural decision — it is a test-harness concurrency change fully bounded by existing R-0018-b / R-0018-f / R-0006 and P-TrustworthySignal. The Frame's constraint-graph walk surfaced no novel or escalation-triggering edge (`docs/intent/ci-flake-tier1-serialize-frame.md`). No decision in this spec required Principal-Architect escalation; the two maintainer decisions (proof surface, serialize scope) were resolved at the intake-exit gate and are transcribed here, not re-litigated.
