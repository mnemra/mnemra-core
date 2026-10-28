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

> **Amendment 2026-10-20 — operator-run PostgreSQL instance (#4119).** This fix was locked against the embedded Postgres engine (`postgresql_embedded`, `EmbeddedEngine`), which [P-0010](../src/adrs/P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (#4018, on `main` at `65e58e0`) replaced with a separate PostgreSQL instance the operator installs and runs; **R-0120** in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md` ("Test substrate") locks how test fixtures obtain it. The fix itself landed (5f5470b) and its requirements were discharged; what this amendment corrects is the text a later reader would act on: **R-0021**'s group-membership criterion (a grep for engine symbols that no longer exist), **R-0024**'s postmaster reap (which, with no test spawning a postmaster, can only ever kill the operator's), and the "embedded" descriptors in R-0020 and R-0025. Each is marked *(Amended 2026-10-20, #4119)* with the superseded text retained. The serialization decision this spec owns — `--test-threads 1` on the PG group, uniform across the three recipes — is **unchanged** (R-0120-b states so). No new R-ID. The `.bom.toml` sidecar is not edited (same reason as the tier-2 spec's §Amendment 2026-10-20, "Sidecar"). The intake and Frame carry a superseded-engine banner and are not otherwise edited.

> **Amendment 2026-10-28 — #4361's interim reap (#4363).** #4361 narrows `scripts/flake-runner.sh`'s reap to its own per-worktree temp root, interim until #4161. That departs from R-0024 as amended 2026-10-20 ("The harness SHALL NOT terminate any process"). This amendment records the deviation and its end, which is fired by a check, and tightens R-0024's two 2026-10-20 criteria, which the interim shape would pass vacuously. It is marked *(Amended 2026-10-28, #4363)* at R-0024, with the superseded text retained. The deviation itself was ruled by the maintainer ("Land with the deviation", #4361 activity 8970); the record and the tightened criteria are drafted by #4363, and go to the maintainer's exit gate with the rest of that change, listed at `docs/specs/2026-05-24-mnemra-core-v0-substrate.md` R-0120-i(h). No new R-ID. The `.bom.toml` sidecar is re-pinned at that exit gate, not here.

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

*(Amended 2026-10-20, #4119: discharged at the fix's landing; retained as the record. "Real embedded Postgres" reads "a real Postgres with the real extension" — R-0018-b's text in the V0 substrate spec is unchanged, and the instance is now the operator's or CI's per R-0120. The ABORT classifier (third criterion) is unchanged and is the one the tier-2 spec's R-0035 still uses.)*

**Acceptance Criteria:**
- [ ] A pre-fix "before" is on record: at parallel (default) concurrency, the group produces ≥ 1 SIGABRT across N = 20 runs (recorded: `mcp_server` 3/20, `scratch/dispatch-1122-report.md` §1). The "before" MAY be the recorded baseline or a fresh parallel-config measurement on the implementer's box. (Only `mcp_server`, `admin_token`, `tenancy_isolation`, and `storage_contract_postgres` have a measured before-state; the other 10 members were added precautionarily per R-0021 — their expected before-state is 0/N, and no before-measurement is required for a member with no observed SIGABRT history.)
- [ ] The "after": a re-run of `scripts/flake-runner.sh` over **every** member binary of the PG/plugin group (R-0021), N ≥ 20 runs per binary, reports SIGABRT count = **0**.
- [ ] SIGABRT is counted as runs the harness classifies **ABORT** in its summary table — the classifier matches a log containing `signal: 6`, `SIGABRT`, or `process abort signal` (`scripts/flake-runner.sh`) — **not** by process exit code (a `cargo test` parent masks a signal-6 child as its own exit 101, so exit code is unreliable; see the classifier note in `scratch/dispatch-1122-report.md`).
- [ ] Both the before and after numbers are captured in the fix's regression evidence (a record committed or transcribed with the PR), not asserted.

### R-0021 — Serialization scope = the whole PG/plugin test-binary group; non-PG tests stay parallel (Amended 2026-10-20, #4119: the membership criterion is the tier-2 enumerator's; the engine-symbol grep is retired)

**Decision.** Serialization SHALL cover the *entire* group of test binaries that start an embedded Postgres engine — not only `mcp_server` — and SHALL NOT serialize binaries outside that group. *Anchors: maintainer decision at intake-exit (serialize the whole group, precautionary against the macOS→Linux surface divergence — `scratch/dispatch-1122-report.md` risk notes). R-0018-b — the group is exactly the real-PG integration suite. P-TrustworthySignal — the proof surface is the whole group, not the single binary that happened to flake locally.*

**Group membership (the checkable criterion).** A test binary is a **member** of the PG/plugin group **iff** its compiled test target constructs an embedded Postgres engine — i.e. it references `EmbeddedEngine`, a `start_engine()`-style helper, or the `postgresql_embedded` crate. The criterion is the source of truth; the enumeration below is the known membership at this SHA and SHALL be reconciled against the criterion at implement time.

**Known members (14, in `libs/mnemra-host/tests/`):** `admin_token.rs`, `admin_token_behavior.rs`, `artifact_machinery.rs`, `content_schema.rs`, `identity_builtins.rs`, `invoke_health_gate.rs`, `mcp_server.rs`, `mcp_slice1_e2e.rs`, `mcp_verb_gate.rs`, `postgres_engine.rs`, `schema_init.rs`, `startup_population.rs`, `storage_contract_postgres.rs`, `tenancy_isolation.rs`.

*(Amended 2026-10-20, #4119: the criterion's words — "constructs an embedded Postgres engine", `EmbeddedEngine`, `postgresql_embedded`, and the first criterion's grep — name symbols the engine move removes; a grep for them returns the empty set and reads as "no members", the false-green P-TrustworthySignal names. The decision stands: serialize exactly the group that needs the Postgres server, nothing else. **The criterion, re-stated:** a test binary is a member **iff** it acquires a Postgres through the shared fixture (`libs/mnemra-host/tests/common/shared_engine.rs` or its successor), walked transitively through `#[path]` includes, or is on the named production-bootstrap list (today `startup_run_full`) — which is the tier-2 spec's R-0033 enumerator (`docs/specs/2026-07-02-ci-flake-tier2-shared-engine.md`, as amended 2026-10-20), a CI-gated check. The 14-member enumeration is historical (`PG_TEST_FLAGS` names 23 at `65e58e0`, MEASURED); the serialized set is `PG_TEST_FLAGS`, and R-0033 fails the gate when the computed membership and that list differ. The second criterion (non-members stay parallel) stands unchanged. *Anchors: tier-2 R-0033 (as amended) — one mechanism, source-derived, seeded-proven; R-0120-b — the serialization posture is unchanged by the engine move; P-GuaranteeByMechanism — a grep in a spec is a convention, the CI-gated enumerator is the mechanism.* Rejected: re-keying the grep on `MNEMRA_TEST_DATABASE_URL` — that read is confined to the fixture by tier-2 R-0032 as amended, so a source grep over the members would find nothing.)* **Correction after round-1 review (Warden H1):** the R-0033 enumerator does **not** exist at `65e58e0` — `verify-coverage-membership` (`justfile:418` at `65e58e0`) is the coverage-shard spec's R-0096 selector check, and `tests/common/no_migrate_set.rs:148-151` records T7/R-0033 as undecided (#2409). So this criterion's retirement is **conditional**: until the R-0033 check is green in `just ci`, the serialized set is `PG_TEST_FLAGS` **by convention, named here as unenforced** (a binary omitted from the list runs unserialized and nothing reports it); building the enumerator is a stated prerequisite of the fixture re-point (tier-2 R-0033 as amended), and the day it is green this grep is retired for good.

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

### R-0024 — The measurement instrument reaps leaked embedded-Postgres postmasters between runs (Amended 2026-10-20, #4119: the reap is retired; the instrument's tracked location, classifier and sampler stand)

**Decision.** The measurement instrument SHALL be a **tracked** file at `scripts/flake-runner.sh` (committed, not in gitignored `scratch/`), so the before/after measurement (R-0020), the CI proof (R-0023), and the reaping behavior below are bound to an artifact that is code-reviewable at PR time, diffable, and resolvable from repo root in any worktree or CI runner. It SHALL reap **only the orphaned embedded-Postgres postmasters it itself spawned** — tracked against `BASE_PM`, the pre-run baseline count it records at startup — between binary invocations, so an aborted or killed run's leaked postmasters cannot corrupt the next invocation's baseline, and so it never touches pre-existing postmasters that belong to the developer's environment (the diagnosis found 8 pre-existing zombies at session start). Without reaping, the before/after measurement that *is* the deliverable (R-0020) is invalid: a leaked-instance backlog surfaces as SysV shared-memory exhaustion (macOS) or a startup deadline (Linux) and masks the true SIGABRT rate. *Anchors: P-TrustworthySignal — the instrument must be faithful; an instrument that contaminates its own baseline, or that is uncommittable and so unreviewable, reports an untrustworthy rate (the Observability faithful-instrument clause this principle operationalizes).*

*(Amended 2026-10-20, #4119: the **reap clause** is superseded; the **tracked-location** clause stands. Under R-0120 no test spawns a postmaster, so the harness's own-spawned set is empty by construction and `reap_own_postmasters` (`scripts/flake-runner.sh:107-120` at `65e58e0`, MEASURED: it `kill -KILL`s every `bin/postgres` pid absent from the startup baseline) can only ever kill a postmaster it did not spawn — the operator's server, if it restarted mid-run and so changed pid, or a container-runtime-visible instance of the other lane's. A reap with no object is standing kill authority with no role (P-LeastAuthority), and it is the cross-lane reap trap `CLAUDE.md` records in a new form. **The harness SHALL NOT terminate any process.** Its measurement roles stand: the ABORT classifier (R-0020), the `BASE_PM`/`PEAK` sampler — now the zero-spawn positive control of the tier-2 spec's R-0026 as amended (`PEAK_DELTA = 0`) — and the per-run logs. Leaked **databases** are the successor concern, and R-0120-a's orphan sweep in the fixture is their backstop; the harness does not take that role either. `scripts/ci-reap.sh` (the `just ci` reap, R-0098 of the coverage-shard spec) is the same decision for the implementation lane, as R-0120-e already states. *Anchors: R-0120-a/-e; P-LeastAuthority; P-TrustworthySignal — an instrument must not be able to corrupt the thing it measures.* Rejected: narrowing the reap to "postmasters whose data directory is under `$TMPDIR`" — there are none, so it is dead code with the kill still linked; keeping it "in case the embedded engine returns" — a mechanism ahead of evidence, and the engine decision is locked.)*

**Acceptance Criteria:**
- [ ] **Tracked.** The harness is a committed file at `scripts/flake-runner.sh` and appears in the PR diff (`git ls-files scripts/flake-runner.sh` returns the path). The diagnosis-era copy under gitignored `scratch/` is not the deliverable. Because the script *is* the instrument, its reap behavior is reviewable directly in the diff, and additionally evidenced by the back-to-back run below.
- [ ] **Reap own-spawned excess, between binary invocations.** After each member-binary invocation in which the process aborts or exits non-zero, the harness terminates any `bin/postgres` postmasters it spawned in excess of `BASE_PM` before starting the next binary invocation — at inner-loop (per-invocation) granularity, not only between full N-iterations. The live-postmaster count returns to ≤ `BASE_PM` between invocations (no monotonic accumulation across the run); postmasters at or below `BASE_PM` are never killed.
- [ ] **Two-condition non-contamination (crash precondition stated).** A back-to-back two-condition run (parallel then serialized) does not fail the second condition with shared-memory-exhaustion / startup-deadline caused by the first condition's leaked postmasters. This is only a meaningful test if the first (parallel) condition produces ≥ 1 abort to leak postmasters — the pre-fix 15% rate makes ≥ 1-in-20 likely but not certain; if the parallel condition produces no abort, force the leak with a manual `kill -SIGKILL <postmaster-pid>` mid-run, or assert the stronger invariant directly: after every run (pass or fail), the live-postmaster count equals `BASE_PM`. (Pre-fix evidence of the leak: 8 → 30 zombie postmasters accumulated in a single session, `scratch/dispatch-1122-report.md` §1 and followups.)

**Acceptance Criteria (Amended 2026-10-20, #4119).** The first criterion (tracked at `scripts/flake-runner.sh`) stands; the second and third are retained as the record of the fix and are replaced for the re-point by:
- [ ] `grep -n 'kill' scripts/flake-runner.sh` returns no line that sends a signal to a `bin/postgres` pid (the sampler's own `kill "$SAMPLER"` of its background subshell is not a postmaster and MAY remain); `reap_own_postmasters`, or whatever replaces it, is absent or a no-op that prints nothing.
- [ ] **Seeded survival proof (replaces the manual restart check):** after the harness captures `BASE_PM`, a marker process whose `argv[0]` ends in `/bin/postgres` is started (the `exec -a` technique of `libs/mnemra-host/tests/ci_reap_baseline.rs`, a renamed `sleep`); the harness runs one member to a forced non-zero exit; the marker is still alive afterwards (`kill -0 <pid>` succeeds) — a postmaster the harness did not spawn, started after its baseline, survives. Recorded in the PR evidence with the seed. The retirement of `scripts/ci-reap.sh` is now a requirement of R-0120-e as amended (`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`), not a hand-off.

*(Amended 2026-10-28, #4363 — #4361's interim reap, its limits and its end, and the 2026-10-20 criteria tightened. Drafted by #4363, recording a deviation the maintainer ruled, and put to the maintainer with the rest of that change at `docs/specs/2026-05-24-mnemra-core-v0-substrate.md` R-0120-i(h).)*

**(a) Interim deviation, from the landing of #4361 until #4161 lands.** The 2026-10-20 rule "The harness SHALL NOT terminate any process" was written for the re-pointed tree. There, no test spawns a postmaster. Until the R-0120 fixture re-point (#4161) lands, tests still spawn embedded postmasters, so the harness still needs to reap what it leaked.

**The ruling.** Peter, 2026-10-28: "Land with the deviation" (#4361 activity 8970, Puck's record of his answer). #4361 lands whole, including `scripts/flake-runner.sh`'s narrowed reap, as a recorded interim deviation from this requirement, and this amendment is the record. The alternatives that ruling rejected: landing #4361 without the `scripts/flake-runner.sh` change; holding #4361 until #4363 locks. The direction came from Peter's earlier pick, "Interim fix now" (#3575 activity 8909). That activity records that the per-worktree temp-root shape came from the option text put to him, so it is his pick of that direction, not wording he wrote. The implementation lane's fix-round decision (#4361 activity 8946) rejected a temp root outside the shared temporary base, inside the repository's build directory. That would have protected against a stale reaper at once, but it puts engine data inside the repository tree, which is not worth it for an interim that needs only one rebase.

**The permit.** In that interval the harness MAY terminate a process only when all of these hold. The predicate is stated as the code applies it (MEASURED in `scripts/ci-reap.sh` at the pre-landing commit `74f49dd`, cited as provenance only, since a rebase-merge gives the landed commit another id):
- its full command line contains `bin/postgres` (`pgrep -f`);
- its pid is absent from the baseline the harness captured at startup;
- the first `-D <path>` argument on its command line is under the per-worktree temp root the harness set before capturing that baseline.

It reaps through the functions of `scripts/ci-reap.sh`, which it sources, and through no copy of them. It narrows the reap it replaced, which killed any baseline-absent `bin/postgres` on the machine (`scripts/flake-runner.sh:107-120` at `65e58e0`). Until #4361 lands, `main` still carries that old machine-wide form, which already departs from the 2026-10-20 rule; it is a standing deviation until then, and the permit binds from #4361's landing. This does not contradict the 2026-10-20 rejection of "narrowing the reap to postmasters whose data directory is under `$TMPDIR`". That rejection ("there are none") is about the re-pointed tree; before #4161, such postmasters exist.

**Stated limits, accepted for the interim and not fixed here:**
- **Same worktree.** Two harness or `just ci` invocations in one worktree share one temp root, so a failing one reaps the other's baseline-absent postmaster. `scripts/ci-reap.sh`'s own header says so (INHERITED from the round-2 re-checks).
- **Key collision.** The root's key is a CRC (`cksum`) of the worktree's physical path. Two worktrees whose paths collide share a root. That is negligible for a handful of worktrees, but silent (INFERRED).
- **Nested roots.** A process started with `TMPDIR` already set to another worktree's root gets a root nested inside it, which the other worktree's reap then matches (INFERRED from the root's construction at `74f49dd`). The implementation lane's newer pre-landing commit `fa4a96e` cuts an inherited `/mnemra-ci-` component back to the shared base, so roots there are siblings (MEASURED, `scripts/ci-reap.sh:131` at `fa4a96e`, cited as provenance only). This limit holds until a landed commit carries that cut. Whether it does is **parked**: it is re-read in the review that lands #4361, a human cadence, and in any case the limit ends with the reap at #4161 under (c). Both commits are cited as provenance only: a rebase-merge gives each a new id on `main` and orphans the cited one. *Superseded (round 4 draft):* "is re-read when #4361 lands", an event nothing detects (Charon R4-3, round 4).
- **A stale checkout.** The protection holds only when every checkout that runs a gate includes #4361. One without it still runs the old machine-wide reap, which kills the other lane's engines. This limit ends when every worktree that runs a gate has been rebased onto #4361's landing. Until then, concurrent gate runs stay serialised by convention (#4361 activity 8946).
- **Pid reuse.** A pid read by `ps` and then killed has a reuse window of milliseconds (INFERRED, negligible).

**Interim acceptance:**
- a marker whose command line contains `bin/postgres`, with `-D` under another worktree's temp root and started after the harness's baseline, survives a forced non-zero invocation;
- the same marker with `-D` under the harness's own temp root is reaped (the positive control).

(Anchors: ruling 8970 for the deviation and 8909 for its direction; P-LeastAuthority — an elevation names the event that ends it, and this one is (c); P-CheckCoversTheClaim — the permit states the predicate the code applies, and the limits state what that predicate still reaches.)

**(b) The 2026-10-20 criteria, tightened. Two rows are superseded. Both replacements bind from #4161; during the interim of (a) the first one reports `scripts/ci-reap.sh`'s kill line, which is the expected interim result.**

*Superseded:* "`grep -n 'kill' scripts/flake-runner.sh` returns no line that sends a signal to a `bin/postgres` pid". At `74f49dd` the harness kills through the sourced `scripts/ci-reap.sh`, whose `kill -KILL` (line 228 there) that grep never reads. Its hits in `scripts/flake-runner.sh` are the sampler's own two `kill "$SAMPLER"` lines and comments, so the row passes while the harness still kills (MEASURED). Whether a line signals "a `bin/postgres` pid" also depends on where the pid came from, which a check cannot compute from the text.

*Replacement, an allowlist computed over the files the harness reaches:*
- **Range.** `scripts/flake-runner.sh` and every script it reaches, transitively: a `source` or `.` target, or a file it executes through `bash`, through `sh`, or by a path ending in `.sh`. A target path is resolved by substituting `$SCRIPT_DIR` (the script's own directory) and `$REPO` (the repository root), and nothing else. Any other variable or a command substitution in a target path, and any `eval` in range, fails the check, naming the line. `cargo`, and the binaries it builds and runs, are out of range. So is every other command, and the check's conclusion is no wider than that range.
- **Predicate.** In range, every `kill`, `pkill` or `killall` token outside a comment fails the check, naming the line. A comment is a line whose first non-blank character is `#`, and the allowlist is matched after leading whitespace is stripped. The exception is a line on an exact-line allowlist the check holds, with its count pinned. When this note was written the allowlist was the two lines `trap 'kill "$SAMPLER" 2>/dev/null; true' EXIT` and `kill "$SAMPLER" 2>/dev/null || true` (count 2). The allowlist changes only in the same diff as the line it admits.
- **Positive control, a fixture the check builds.** In a temporary directory the check writes:
  - a stub `flake-runner.sh` that sources `$SCRIPT_DIR/seed.sh`, runs `bash "$SCRIPT_DIR/run.sh"`, and holds one `eval "$x"` line;
  - a `seed.sh` that holds `kill -KILL "$pid"`;
  - a `run.sh` that holds `pkill -f bin/postgres`.

  The check reports all three lines, the third as unresolvable. Over the same stub holding only the two allowlisted lines, it passes.

*Superseded:* the seeded survival proof's marker, "a marker process whose `argv[0]` ends in `/bin/postgres` is started … the marker is still alive afterwards". Under the narrowed reap of (a), a marker with no `-D`, or a `-D` outside the harness's temp root, is excluded by the data-directory condition. It survives whether or not the reap was removed, so the row proves nothing (MEASURED from that condition at `74f49dd`).

*Replacement:*
- The seeded failing member starts the marker itself, so the marker inherits whatever `TMPDIR` the harness exported. The marker's command line contains `bin/postgres` and `-D "$TMPDIR/<name>"`, and it runs a bounded `sleep`, a code constant of 60 s. The harness captured its baseline before invoking the member, so the marker starts after it.
- The member exits non-zero. After the harness's post-invocation step, the marker is alive (`kill -0` succeeds). The test then kills the marker on every exit path, so no leftover marker inflates a later postmaster count or becomes a target for a stale reaper.
- **Positive control, a fixture the test builds:** the same member, run under a stub harness the test writes, which sources a stub reap with the interim's condition (baseline-absent, with `-D` under `$TMPDIR`), shows the marker reaped.

**Where the checks run.** The allowlist check, run over the live tree and over its built fixture, and the two greps of R-0120-e and (c), run as one Rust test binary in `NONPG_TEST_FLAGS`, so they run in both chains on every `just ci`. It lands in the #4161 change, with `NONPG_TEST_FLAGS`'s count pin and `EXPECTED_TEST_FILE_COUNT` bumped in the same commit. It lands with #4161 and not before, because during the interim of (a) the live-tree check reports `scripts/ci-reap.sh`'s kill line. The marker row runs the real harness, which builds the workspace, so it stays landing evidence on #4161's pull request, as the 2026-10-20 criterion already required ("Recorded in the PR evidence with the seed"). *Superseded (round 3 draft):* the checks had no stated home, so the detector for the deviation's end was a reviewer who had to remember to run them (Bolt M-4, round 3).

(Anchors: P-CheckCoversTheClaim — the range includes what the harness reaches and the conclusion is no wider, a zero gets a positive control the check can always rebuild, and a correction is checked by the predicate it corrects, here the narrowed reap's own condition; P-SecurityLayered — the check enumerates the permitted lines instead of matching forbidden shapes.)

**(c) At #4161.** #4161 SHALL remove from `scripts/flake-runner.sh`:
- its `source` of `scripts/ci-reap.sh`;
- its calls of `ci_reap_isolate_tmpdir` and `ci_reap_capture_baseline`;
- `reap_own_postmasters`, with every call of it;
- the `echo "temp root: $TMPDIR"` line;
- every use of a `CI_REAP_` variable. The `baseline postmaster PID count` line uses one. Under the script's `set -u` it would stop the harness at startup once the capture call is gone, so the line is removed, or recomputed from the harness's own `count_postmasters`. It is never defaulted to a value nothing verifies.

The `cd "$REPO"` that #4361 added for the key MAY stay: it reaps nothing, and the harness runs `cargo` by `--manifest-path`. The `BASE_PM`/`PEAK` sampler stands (2026-10-20). The end of (a) is fired mechanically: R-0120-e as amended in the V0 substrate spec requires, of #4161, that `grep -rn 'ci-reap' justfile scripts .github` return nothing and `grep -rniE 'ci_reap_|mnemra-ci-' justfile scripts .github` return nothing. The source line and the `ci_reap_` calls fail both.

*(Binary-observable, added: after #4161, `grep -niE 'ci-reap|ci_reap_|reap_own_postmasters' scripts/flake-runner.sh` returns nothing. The match is case-insensitive, so it also catches `CI_REAP_` variables. The positive control is a fixture the check builds: the same command over a temporary file holding `echo "$CI_REAP_BASELINE_PIDS"` returns that line.)*

(Anchors: V-Simplicity — a mechanism whose object is gone is removed, the reason the 2026-10-20 amendment gave; P-Defer — the deviation's end has a detector, not a reminder.)

### R-0025 — Concurrency-only: the per-test fresh-DB isolation model SHALL NOT change

**Decision.** Tier 1 changes test *concurrency* only. The fix SHALL NOT alter the per-test fresh-database isolation model (each test starts a fresh embedded engine + database and seeds under the same `DEFAULT_WORKSPACE_ID` constant, so isolation derives purely from a separate database per test) that the R-0006 tenant-enforcement tests rely on. This bounds Tier 1 to its stopgap role and keeps the Tier-2 shared-instance refactor unblocked. *Anchors: R-0006 — tenant enforcement; the isolation tests must keep seeing a clean DB per test. R-0018-b — real PG, per-test isolation unchanged. Intake hard constraint — Tier 1 is concurrency-only and must not block or complicate Tier 2.*

*(Amended 2026-10-20, #4119: discharged at the fix's landing; retained as the record. "Each test starts a fresh embedded engine + database" described the pre-tier-2 model; tier-2 R-0027 consumed this parked hand-off (one engine per binary, a database per test) and R-0027 as amended 2026-10-20 carries the isolation model forward (a database per test cloned from a template on the operator's server, dropped at teardown). The R-0006 isolation guarantee is re-verified there (tier-2 R-0037), not here.)*

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

## Amendment 2026-10-20 — operator-run PostgreSQL instance (#4119)

**Status: amendment to an `approved` spec, authored on the design lane 2026-10-20 against `main` at `65e58e0`, following the in-place dated-amendment precedent of the V0 substrate spec (#4018) and [P-0003](../src/adrs/P-0003-plugin-manifest.md) "Amendment 2026-07-07". Nothing locked is deleted.** Why now: P-0010 §Amendment 2026-10-19 lists `docs/specs/2026-06-26-ci-flake-tier1-serialize.md:70-77` among the records the engine move falsifies and assigns its correction to the tier-2 amendment task, which re-defines group membership for the three CI specs at once.

| Decision | Resolution | Anchor | Rejected, with reason |
|---|---|---|---|
| Group-membership criterion (R-0021) | The tier-2 R-0033 enumerator — fixture references walked transitively ∪ the named production-bootstrap list — **once it exists** (it does not at `65e58e0`; building it is a prerequisite of the re-point); until then `PG_TEST_FLAGS` by convention, named as unenforced | tier-2 R-0033 as amended; R-0120-b; P-GuaranteeByMechanism | A new grep keyed on `MNEMRA_TEST_DATABASE_URL` — confined to the fixture by tier-2 R-0032, so it finds nothing |
| The harness reap (R-0024) | Retired: the harness SHALL NOT terminate any process, proven by a seeded marker that survives; classifier, sampler and tracked location stand; `scripts/ci-reap.sh` retires by R-0120-e as amended | R-0120-a/-e; P-LeastAuthority; P-TrustworthySignal | Narrow to `$TMPDIR` data directories — none exist, dead code with the kill still linked; keep for a returning embedded engine — mechanism ahead of evidence against a locked decision |
| Serialization (R-0021 second criterion, R-0022) | Unchanged | R-0120-b, stated there | — |
| Sidecar (`docs/specs/2026-06-26-ci-flake-tier1-serialize.bom.toml`) | Not edited in this revision; **re-pinned at this amendment's exit gate** (maintainer ruling 2026-10-20, #4119 activity 7849 item 3 — Peter's approval is the spec-exit; the re-pin block is drafted in the #4119 report and names the stale intake and Frame pins too) | Same reason as the tier-2 spec's §Amendment 2026-10-20, "Sidecar" | Re-pin `version` alone — a false conformance signal against a Frame that still says "embedded" |

**Implementation follow-ups implied:** remove the reap from `scripts/flake-runner.sh` (keep the sampler) and record the seeded survival proof; retire `scripts/ci-reap.sh`, which R-0120-e as amended now **requires** (`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`), not leaves to the implementation lane. **Not decided here:** nothing; this spec's open items were consumed by tier-2.
