---
status: locked
spec_type: code
date: 2026-07-15
base-pin: main@5e5992a0
locked: 2026-07-16 — maintainer review + lock (lean track: the maintainer is the review gate; no BOM sidecar, no reviewer panel)
process: lean — direct-authored, reviewed and locked by the maintainer (no separate Frame/intake stage and no multi-reviewer spec panel; the maintainer is the review gate)
---

# Spec: CI coverage sharding — roulette-proof the disk-bound coverage gate

> **Spec for:** the CI coverage gate's disk footprint. Splits the single-job `verify-coverage` work across separate CI jobs (one fresh runner disk each) so no single runner accumulates the whole instrumented binary set, and lands a standing disk-total/headroom probe so a non-deterministic runner-fleet failure becomes observable and provable.
> **Date:** 2026-07-15.
> **Scope note:** this is the **disk tier** fix for the coverage gate. It is a *different root cause* from the shared-engine test-infra work and does not touch it. No product behavior changes — only CI topology, the coverage recipe factoring, and the CI disk probe.

> **Amendment 2026-10-20 — operator-run PostgreSQL instance (#4119).** This spec was locked while the PG shard's binaries ran an embedded Postgres engine; [P-0010](../src/adrs/P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (#4018, on `main` at `65e58e0`) replaced that engine with a separate PostgreSQL instance the operator installs and runs, and **R-0120** in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md` locks how tests obtain it — in CI, as a digest-pinned PostgreSQL container that the job's own steps start with a per-run TLS certificate (R-0120-c(1) as amended; not a `services:` block). The sharding decision (Shape B, the PG-vs-rest partition on `PG_TEST_FLAGS`, R-0093–R-0098) **stands**; what the engine move falsifies is the descriptor "embedded" and the per-shard setup clause that named "the embedded-Postgres binary cache". **R-0094** gains the clause that every CI job executing a `PG_TEST_FLAGS` cell starts that container; **R-0098**'s pin on `scripts/ci-reap.sh` is clarified as binding only the sharding change, and the script's retirement is now required by R-0120-e as amended. Each is marked *(Amended 2026-10-20, #4119)* with the superseded text retained. No new R-ID. This spec has no `.bom.toml` sidecar (lean track).

> **Amendment 2026-10-29 — one image reference per supported PostgreSQL major, and the PG shard on the highest (#4291).** The V0 substrate spec `docs/specs/2026-05-24-mnemra-core-v0-substrate.md` now holds the host's supported PostgreSQL majors as one host code table (its R-0117-a as amended 2026-10-29, maintainer ruling 2026-10-25, "Floor in spec, set in code"), and its R-0120-c (5) as amended has `.github/workflows/ci.yml` hold one digest-pinned image reference per supported major, with the verify chain run once per reference. **R-0094**'s added clause and its 2026-10-20 criterion are read per reference, and `coverage-pg` runs once, on the highest supported major (maintainer ruling 2026-10-29, #4291 activity 9171, item 4, "Highest major only"). The partition, the probe, R-0093–R-0098 and the 2026-10-20 amendment stand. Each note is marked *(Amended 2026-10-29, #4291)* with the superseded text retained, and the decisions are in § Amendment 2026-10-29 below. No new R-ID. This spec has no `.bom.toml` sidecar (lean track).

## Purpose / context

The CI coverage gate intermittently fails with `No space left on device`. The failure is **runner-fleet roulette**, not an unbounded footprint:

- The default hosted Linux runner label is a **mixed fleet** — a smaller-disk image and a larger-disk image — and the workflow cannot select which it draws.
- The existing "Free runner disk space" step (`.github/workflows/ci.yml`, the `Free runner disk space` step) reclaims a **fixed amount** (a static `rm -rf` list plus a docker prune) regardless of runner size. It is already maxed.
- The coverage recipe (`justfile` `verify-coverage`, ~L361–378) runs **four** `cargo llvm-cov --no-report` passes that **accumulate** instrumentation and profile data with no cleanup between them, then one final `cargo llvm-cov report`. `cargo llvm-cov` builds a *separate instrumented copy* of the wasmtime-embedding workspace across the full test-binary set — this is the dominant disk consumer of the whole `just ci` run.
- On the **larger-disk** variant the run finishes with comfortable headroom. On the **smaller-disk** variant, after the fixed free-space reclaim, available space runs out partway through the *second* `--no-report` pass (the non-PG set) — the observed death point. The gate then fails, and a re-run that happens to draw the larger variant passes. That is the roulette.

Larger or deterministic-disk runners are billed and are **out of budget** ($0 constraint). Dropping the embedded-Postgres binaries from instrumentation (a fidelity loss) is **rejected** — full coverage is retained. *(Amended 2026-10-20, #4119: "embedded-Postgres binaries" reads "the `PG_TEST_FLAGS` binaries" — the set is unchanged; what they connect to is the R-0120 server.)*

**The fix (decided):** split the coverage work across separate GitHub **jobs**. Each job gets a fresh runner disk, so each job accumulates only *its* subset of instrumented binaries → each shard's peak disk appetite drops below the smaller-runner floor → the fleet draw stops mattering.

Because the failure is non-deterministic across a fleet that cannot be selected, **a single green run proves nothing** — it may have drawn the larger variant. So the first change (independently landable, ahead of the restructure) is to instrument the roulette: log the runner's total disk size and the true minimum headroom in every coverage job. The fix is proven only when a run that **drew the smaller variant** passes with real margin, and the log evidences both facts.

This spec locks requirements **R-0093–R-0098** (the global R-ID series continues from R-0092). RFC-2119 keywords: `SHALL`/`MUST` mandatory; `SHALL NOT`/`MUST NOT` prohibited; `SHOULD` preferred; `MAY` optional.

### The load-bearing decision: independent shards, not merged shards (Shape B)

Two shard shapes were on the table:

- **Shape A — merge shards.** Each shard uploads its raw profile data as an artifact; a final job downloads all shards and runs one `cargo llvm-cov report` for a **single** coverage number.
- **Shape B — independent shards.** Each shard runs `cargo llvm-cov` fully over its own subset and emits **its own** number. No cross-job merge.

**Decision: Shape B.** *Anchors: Simplicity (choose the least-mechanism option that serves the actual consumer); the coverage gate is presently **emit-only** — the recipe emits a number and does **not** pass/fail on a threshold (`justfile` `verify-coverage` header comment: "emit number; no threshold gate at scaffold stage").*

Rationale:

1. **No consumer needs the union number today.** The gate is emit-only. Nothing reads a single merged coverage figure to pass/fail. The sole benefit Shape A buys — one number instead of N — is a benefit with no current consumer.
2. **Shape A's merge is undocumented and fiddly across jobs.** `cargo llvm-cov`'s supported merge pattern (`cargo llvm-cov clean` → `--no-report` × N → `cargo llvm-cov report`) is a **single-machine** flow: the final `report` reads *both* the profile data *and* the instrumented object files from one shared local `target/llvm-cov-target` directory. There is no supported "upload profraw from job A, download in job B, report" path — a cross-job merge means reconstructing the entire instrumented-target state (profile data **and** the instrumented binaries, whose paths must resolve) in a merge job. That is correctness-critical and brittle. Paying that complexity to produce a number no gate consumes is unjustified.
3. **This surface is revisitable, not lock-in.** The coverage-output shape is an internal CI emit; changing from N per-shard numbers back to one merged number later is cheap CI-only work. On a revisitable, no-external-consumer surface, optimize for the actual consumer (this project's own CI loop) and pick the simple option now.
4. **Shape B adds zero new CI Actions.** Separate jobs / a job matrix are native workflow syntax. Shape A would add `upload-artifact` + `download-artifact` — two new third-party Actions, each an added supply-chain surface (see R-0097). Fewer moving parts, smaller attack surface.

Shape B's cost — a line covered only by *another* shard's tests reads as uncovered in this shard's report, and no union figure exists — is acceptable **only while the gate is emit-only**, and is pinned with a named tripwire (R-0095).

## Requirements

Each requirement carries a **Decision** (with its canon anchor) and **binary-observable acceptance criteria**. Anchors are repo-local: `R-0018` (the CI entry point) and `R-0022` (the PG-serialization directive) are the existing V0-substrate requirements the `justfile` comments already cite; `Simplicity` and `AP1` (unambiguous-resolution / no-silent-drop) are project values.

---

### R-0093 — Roulette-detector probe: log runner disk-total + true headroom, in every coverage job (STEP 1, independently landable)

**Decision:** The CI disk probe SHALL be extended to record, in the live step log, **the total disk size** of the build filesystem(s) and a periodic **available-space** sample for both `/` and the runner scratch mount (`/mnt`). This probe SHALL be present in **every** job that builds under coverage instrumentation. It SHALL land **first**, on the current single-job workflow, before the restructure — so it begins collecting fleet data (which variant each run drew, the true minimum headroom) immediately and captures the failing smaller-variant baseline. *Anchors: R-0018 (CI entry observability); the failure is non-deterministic across an unselectable fleet, so the instrument that identifies the drawn variant is the only thing that makes any pass or fail provable rather than anecdotal.*

The current probe (`.github/workflows/ci.yml`, the `Run CI gates` step) logs `avail` of `/` only — it cannot tell which fleet variant a run drew, so it cannot distinguish "passed because sharded" from "passed because it drew the big runner."

**Acceptance criteria:**
- [ ] Every coverage-instrumented CI job's log contains, at job start, a line stating the **total** size of `/` (and of `/mnt` where the build uses it) — a reader can classify the run as the smaller- or larger-disk variant from the log alone.
- [ ] The same job's log contains **periodic available-space** samples for `/` (and `/mnt`) across the coverage build, from which the run's **minimum** available space is readable.
- [ ] The probe change is committed and green **on the current single-job workflow** in its own change, before any job-restructure change lands (independently landable STEP 1).
- [ ] The probe is a diagnostic log emitter only: it changes **no** gate outcome (a run's pass/fail is identical with and without it).

---

### R-0094 — Coverage instrumentation is decomposed into ≥2 disjoint shards, each proven under the smaller-runner floor (Amended 2026-10-20, #4119: the PG shard's job starts the R-0120-c Postgres container)

**Decision:** The coverage instrumentation set SHALL be partitioned into **two or more disjoint shards**, each run in its **own CI job on a fresh runner**, such that each shard's **peak disk appetite** (the run's minimum available space, from R-0093's probe) stays **below the smaller-runner post-free-space floor with non-trivial margin**. The partition is an **adjustable calibration**, not a lock: the suggested V0 boundary is **{the embedded-Postgres binary set} vs {the non-PG binaries + host lib + remaining workspace crates}**. *Anchors: Simplicity (fresh-disk-per-job is the least-mechanism way to bound accumulation); the observed failure — the first (PG) `--no-report` pass completed and death occurred at the *start* of the second (non-PG) pass — is direct evidence that (a) the failure is the accumulated *sum*, so splitting the sum bounds it, and (b) the natural cut is exactly at the PG-vs-rest boundary, which maps to the observed accumulation point rather than an arbitrary line.*

The embedded-Postgres set is the heavy one (embedded Postgres + wasmtime). The two shards reuse the **same** target selectors as today's recipe — `PG_TEST_FLAGS` (`justfile` ~L125) for the PG shard; `NONPG_TEST_FLAGS` (`justfile` ~L143) + `-p mnemra-host --lib` + `--workspace --exclude mnemra-host` for the rest shard — so the union is the pre-shard set by construction (see R-0096).

*(Amended 2026-10-20, #4119: "The embedded-Postgres set is the heavy one (embedded Postgres + wasmtime)" is superseded as a description, not as a partition: the PG shard is still `PG_TEST_FLAGS` (`justfile:127` at `65e58e0`) and is still the heavy one for the reason that survives — wasmtime — while the engine's own binaries no longer sit in the instrumented tree at all. The partition boundary, the probe-verified margin and the re-partition tripwire stand unchanged. **Added clause:** every CI job that executes a `PG_TEST_FLAGS` cell — the gates job and the PG coverage-shard job — SHALL start the Postgres container exactly as R-0120-c(1) as amended states — by its own job steps, from the one digest-pinned image reference, with the per-run TLS certificate and the conditions listed there, a `pg_isready` wait, and the setup step creating the harness role, the template database and the extension — exporting to the job **only** the harness URL and the CA certificate path (no superuser credential in any job environment), because a container started by a job's steps lives on that job's runner and a shard job is a fresh runner. The rest shard SHALL NOT start one: it has no consumer, and a `PG_TEST_FLAGS` binary that drifted into it would then fail loudly under R-0120-a's fail-not-skip rather than silently find a server. *Anchors: R-0120-c; V-Consistency — one mechanism for every PG-executing job; P-TrustworthySignal — the absent server in the rest shard is a second detector for a mis-partitioned binary, beside R-0096's reconciliation; P-LeastAuthority — the superuser credential stays out of the job environment (maintainer ruling 7867 item 1).* Rejected: one shared server for both coverage jobs — each job is a separate runner, so a server one job's steps start is not reachable from another; a `services:` block per job — cannot present a per-run certificate (R-0120-c(1)); a reusable workflow carrying the steps — permitted as the implementation of the clause, not an alternative to it.)* *(Amended 2026-10-29, #4291: "from the one digest-pinned image reference" reads: from one of the digest-pinned image references, one per key of the host's supported-major table (V0 substrate spec R-0117-a and R-0120-c (5) as amended 2026-10-29). The gates job runs its chain once per reference, as sequential legs, each starting its own container from that reference. The PG coverage-shard job starts its container once, from the reference whose server reports the highest key, and fails `coverage_major_not_highest` otherwise (maintainer ruling 2026-10-29, #4291 activity 9171, item 4; V0 R-0120-c (5) as amended). At V0 the table holds one key, so the clause reads as before. Anchors: the 2026-10-25 and 2026-10-29 rulings (#4291); V-Consistency — one mechanism for every PG-executing job, per key.)*

**Whether two shards suffice is a probe-verified hypothesis, not an assumption.** If any shard lands within margin of the floor on a smaller-variant draw, the partition SHALL be refined (a finer split) until every shard clears the floor with margin — a named, mechanically-fired re-partition condition, not a "revisit later."

**Acceptance criteria:**
- [ ] The coverage work runs across **≥2 separate CI jobs**, each on its own runner (verifiable from the workflow job graph: distinct jobs, each doing its own checkout + free-space + toolchain setup + coverage subset).
- [ ] On a CI run whose R-0093 probe shows a coverage-shard job drew the **smaller-disk variant**, that job's **minimum available space stays ≥ a positive margin floor** (calibrated default: **≥ 10 GB**; the *lock* is "positive non-trivial margin proven by the probe," the specific number is a tunable calibration) — and the shard's coverage gate **passes**.
- [ ] The proof is demonstrated on the **defect surface**: at least one smaller-variant draw per coverage-shard job is observed passing with margin. Because each job draws from the fleet independently (compound roulette), reaching a smaller-variant draw per shard MAY require re-runs; the merge is gated on **observing** it, not on assuming it (see § Verify Contract).
- [ ] Re-partition tripwire (mechanically fired): a shard whose probed minimum available space is **below the margin floor** on any smaller-variant draw SHALL trigger a finer partition before the change is considered done — this is a fail condition of the acceptance run, not a backlog item.
- [ ] *(Added 2026-10-20, #4119; revised after round-1 review)* In `.github/workflows/ci.yml`, the `ci` job ("CI gates") and the `coverage-pg` job ("Coverage — PG shard") each start the Postgres container by a job step from one image reference held once in the workflow and matching `pgvector/pgvector:[^@ ]+@sha256:[0-9a-f]{64}` (repository, tag and digest — R-0120-c as amended), each with the per-run TLS steps and a `pg_isready` wait; the `coverage-rest` job references no Postgres image and starts no container. The job names are the binary key — the recipe a job runs is not visible in the workflow file, so "a job that runs `PG_TEST_FLAGS`" was not computable. *(Amended 2026-10-29, #4291: read per reference. "From one image reference held once in the workflow" reads: each run from one of the image references, each held once in the workflow, one per key of the host's supported-major table (V0 substrate spec R-0120-c (5) as amended 2026-10-29). The chain legs of the `ci` job, one per reference, together cover every reference; `coverage-pg` uses the reference whose server reports the highest key, and fails `coverage_major_not_highest` on any other. At V0 there is one reference and the criterion reads as before.)*

---

### R-0095 — Coverage output: per-shard numbers, union explicitly uncomputed, pinned to the emit-only regime

**Decision:** Under Shape B each shard SHALL emit its **own** coverage number over its subset. The absence of a single union-coverage figure SHALL be **documented explicitly** at the recipe and in the CI output, stated as acceptable **only** while the gate is emit-only. *Anchors: Simplicity + the emit-only regime (no threshold consumer today); AP1 (the output's meaning must be unambiguous — a per-shard number must not be mistakable for a whole-workspace number).*

**Acceptance criteria:**
- [ ] Each coverage-shard job emits a coverage number scoped to **its own subset**, and its log/output labels it as a **per-shard** figure (not a whole-workspace figure).
- [ ] A comment at the coverage recipe(s) states plainly that **union coverage across shards is not computed** under Shape B, and that this is acceptable **only** while the coverage gate is emit-only / has no threshold.
- [ ] Threshold tripwire (mechanically fired, named): **before any coverage *threshold* gate is introduced**, the union-coverage question SHALL be resolved — either by adopting cross-job merge (Shape A) or by defining per-shard thresholds. The condition that fires this is a concrete, detectable event (a PR that adds a pass/fail threshold to the coverage gate), not "later."

---

### R-0096 — Single source of truth for shard membership; no binary silently dropped

**Decision:** There SHALL be exactly **one** definition of each coverage target selector, and every selector in the pre-shard recipe SHALL appear in **exactly one** shard. Any whole-recipe local convenience SHALL **delegate** to the shard recipes rather than re-enumerate the selectors — so local and CI compute coverage over the identical set and the two cannot drift. *Anchors: AP1 (no silent drop; the union of the shards must provably equal the pre-shard set); Simplicity (one source of truth, not two parallel enumerations that diverge unobserved).*

The silent-drop risk is a coverage binary present in the old single recipe but absent from every shard — coverage silently lost with no signal. The delegation structure makes the union identical **by construction**; the reconciliation check below is the binary-observable guard on top of it.

**Acceptance criteria:**
- [ ] The union of the shard recipes' coverage target selectors **equals** the pre-shard `verify-coverage` selector set (`PG_TEST_FLAGS`, `NONPG_TEST_FLAGS`, `-p mnemra-host --lib`, `--workspace --exclude mnemra-host`) with **no selector dropped and none duplicated across shards** — verified by an **enumeration/reconciliation** check, not by eyeballing.
- [ ] The whole-recipe local entry point (`just verify-coverage`) **delegates to** the shard recipes (runs each shard in turn) rather than carrying its own copy of the selectors — there is a single definition of shard membership.
- [ ] A reader can point to the **one** place each selector is defined and confirm it is referenced by exactly one shard.

---

### R-0097 — $0: no billed runners or actions; any new Action SHA-pinned

**Decision:** The fix SHALL introduce **no** paid or larger-disk runners and **no** billed Action. Shape B is chosen partly because it needs **zero** new CI Actions (native jobs/matrix). Should any new third-party Action nonetheless be introduced, it SHALL be **pinned to a full commit SHA** (not a floating tag). *Anchors: the $0 budget constraint; supply-chain integrity (a mutable tag can be repointed at malicious code; a SHA cannot).*

**Acceptance criteria:**
- [ ] No job in the changed workflow uses a larger-disk or otherwise-billed runner label — all coverage-shard jobs use the same free default runner label as today.
- [ ] The change adds **no** `upload-artifact` / `download-artifact` / merge Action (Shape B needs none).
- [ ] If any new third-party Action is added, its `uses:` reference is a **full 40-char commit SHA** (matching the existing SHA-pinned precedent in `.github/workflows/ci.yml`). Existing tag-pinned Actions are **out of scope** — this requirement governs only newly-introduced Actions (no re-pinning sweep).

---

### R-0098 — All other gate semantics preserved; local full-chain coverage retained

**Decision:** Every gate other than coverage, and the coverage gate's non-topology semantics, SHALL be **unchanged**. Specifically: the baseline-PID self-reap safety net (`scripts/ci-reap.sh`, wired at `justfile` `ci` ~L454), the embedded-PG `--test-threads 1` serialization on the PG subset (`justfile` — the `-- --test-threads 1` directive, R-0022), and the other verify gates (type, lint, test, test-hooks, build, smoke, signing-root) SHALL run with identical semantics. Local `just ci` SHALL retain **full-chain** coverage (it runs every shard). *Anchors: R-0018 (CI entry point — its gate set and ordering are the contract); R-0022 (the serialization directive is identical across the three PG-touching recipes and must stay so).*

The only intended changes are: (1) coverage's **CI job topology** (now ≥2 jobs, R-0094) and (2) coverage's **output shape** (per-shard, R-0095). Nothing else moves.

*(Amended 2026-10-20, #4119: two readings corrected. "The embedded-PG `--test-threads 1` serialization" reads "the `--test-threads 1` serialization on the PG subset" — unchanged in substance, R-0120-b states the posture is unchanged by the engine move. The pin "`scripts/ci-reap.sh` unchanged" bound the **sharding change** (this spec's diff) and was discharged at its landing; it does not bar the reap script's retirement, which R-0120-e as amended **requires** — a reap with no postmaster to reap has no object (the same reasoning as tier-1 R-0024 as amended in `docs/specs/2026-06-26-ci-flake-tier1-serialize.md`). The "other seven verify gates" list is as it was at this spec's lock; gates added since (`verify-coverage-membership`, `verify-advisories`, `verify-cold-start`) are outside this requirement, and `verify-cold-start` retires under R-0120-c. *Anchors: R-0120-b/-e; P-PreserveDecisionSpace — the pin is kept with its scope stated, not erased.*)*

**Acceptance criteria:**
- [ ] The PG coverage shard runs its subset with `--test-threads 1` (the R-0022 serialization directive is present and unchanged on the PG subset).
- [ ] The self-reap net still wraps the local `ci` run with identical baseline-capture / fire-on-failure-only semantics (`scripts/ci-reap.sh` unchanged; its invocation in `just ci` intact).
- [ ] The other seven verify gates (type, lint, test, test-hooks, build, smoke, signing-root) run in CI with unchanged commands and outcomes.
- [ ] `just ci` locally still runs the **full** verify chain including **all** coverage shards (local coverage is not narrowed by the CI sharding; local disk has no roulette, so shards may run in one process there — see the illustrative shape).

**Acceptance Criteria (Amended 2026-10-20, #4119).** The second criterion above ("`scripts/ci-reap.sh` unchanged") bound the sharding change and is retained as its record; it is superseded for every later change by R-0120-e as amended (`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`), which **requires** the script's retirement: `grep -rn 'ci-reap' justfile scripts .github` returns nothing and `scripts/ci-reap.sh` does not exist once the fixture re-point lands. The first criterion reads "the PG coverage shard runs its subset with `--test-threads 1`" unchanged.

## Scenarios

**S1 — Smaller-variant coverage run passes with margin (the proof).**
Given a CI run whose R-0093 probe records a coverage-shard job on the **smaller-disk** variant,
When that shard's coverage subset builds and reports,
Then the probe's minimum available space for that job stays **≥ the margin floor** and the shard's coverage gate **passes** — proving the fix on the defect surface (contrast: the pre-fix single job on the same variant hits 0 and fails).

**S2 — No binary silently dropped.**
Given the shard recipes,
When the membership reconciliation runs,
Then the union of shard selectors equals the pre-shard selector set exactly (no drop, no duplication), or the check **fails** the build.

**S3 — Larger-variant run still passes (no regression).**
Given a CI run drawing the **larger-disk** variant for the coverage shards,
When the shards run,
Then all coverage shards pass (the fix does not depend on the small variant; it just no longer *requires* the large one).

**S4 — Threshold introduction fires the union-coverage tripwire.**
Given a future PR that adds a pass/fail **threshold** to the coverage gate,
When it is proposed,
Then the R-0095 tripwire requires resolving union coverage first (Shape A merge or per-shard thresholds) — the per-shard emit-only regime is no longer sufficient.

## Out of scope

- **Cross-job profile-data merge (Shape A) / a single union coverage number.** Deferred, not cut. *Tripwire:* R-0095 — introducing a coverage threshold gate. Until then, per-shard numbers stand.
- **The shared-engine test-infra work (a separate root cause).** Sequenced independently; this spec does not touch it.
- **Re-pinning existing tag-pinned Actions to SHAs.** Out of scope (R-0097 governs only newly-added Actions). Not a deferral — a deliberate boundary to keep this change minimal; a broader supply-chain pin sweep is its own change if wanted.
- **Removing the temporary disk probe.** The probe is the standing roulette-detector for as long as the disk tier is open; its removal is a separate decision once the fleet is no longer a concern.
- **Any product/runtime behavior.** This is CI-topology + coverage-recipe factoring only.

## Constraints

- **Public repository.** Spec, commits, and PR language are generic. **Never force-push.**
- **$0 budget.** No billed runners or Actions (R-0097).
- **Full coverage retained.** No fidelity loss (the PG binaries stay instrumented; they move to their own shard, they are not dropped).
- **Each shard carries its own setup.** A coverage-shard job does its own checkout, free-space reclaim, toolchain install (incl. `llvm-tools-preview` + the wasm target), `cargo-llvm-cov` install, the R-0093 probe, and — where its subset needs it — the embedded-Postgres binary cache and the plugin-build prerequisite. This is inherent to fresh-runner-per-job; it is the cost the fix pays for a fresh disk. *(Amended 2026-10-20, #4119: "the embedded-Postgres binary cache" is superseded by "the R-0120-c(1) Postgres container its own steps start, and its setup step" — the `~/.theseus` cache retires with the engine (R-0120-c). The sentence's point — each shard carries its own setup — is unchanged.)*
- **Clean per-shard slate.** Each shard's emitted number MUST derive only from its own subset's tests (no stale cross-shard profile data). On a fresh runner this is automatic; the local delegated run MUST ensure a shard's report is not polluted by a prior shard's data.

## Illustrative implementation shape (non-normative)

This sketch conveys intent; the build agent decides the exact factoring within the requirements above.

- `justfile`: add `verify-coverage-pg` (the PG subset, `--test-threads 1`) and `verify-coverage-rest` (non-PG + `--lib` + workspace). Redefine `verify-coverage` to **delegate**: run `verify-coverage-pg` then `verify-coverage-rest` (single source of truth for membership; local `just ci` thus runs both and emits two numbers). Each shard recipe reuses the existing `PG_TEST_FLAGS` / `NONPG_TEST_FLAGS` vars verbatim.
- `.github/workflows/ci.yml`: run the coverage shards as **separate jobs** (or a job matrix) — the main gate job runs the rest of the chain (type/lint/test/test-hooks/build/smoke/signing-root); the coverage-shard job(s) each do their own setup + the R-0093 probe + one shard recipe. No `upload-artifact` / `download-artifact`.
- The reap net stays in local `just ci`; ephemeral single-use CI shard runners self-clean on teardown and do not need it re-wired per shard.

## Verify Contract

What a re-validation of this spec must confirm:

1. **Probe (R-0093):** a coverage job log shows disk-total (variant classifiable) + periodic avail for `/` (and `/mnt`); the probe changes no gate outcome; it landed on the single-job workflow first.
2. **Sharding + defect-surface proof (R-0094):** ≥2 separate coverage jobs; **at least one observed smaller-variant draw per coverage-shard job passing with minimum available space ≥ the margin floor** — this is the load-bearing evidence and is gathered from the R-0093 probe on real CI runs (re-run until a smaller-variant draw is observed per shard). A green run that only ever drew the larger variant does **not** satisfy this.
3. **Output shape (R-0095):** per-shard numbers, labeled as such; the union-uncomputed note present; the threshold tripwire recorded.
4. **Membership (R-0096):** the reconciliation check passes — union(shards) == pre-shard set, no drop/dup; whole recipe delegates.
5. **$0 + supply-chain (R-0097):** free runner labels only; no new billed Action; any new Action SHA-pinned.
6. **Preserved semantics (R-0098):** `--test-threads 1` on the PG shard; reap net intact; the other seven gates unchanged; local `just ci` full-chain.

The load-bearing verification is **item 2** — proof on the defect surface (a smaller-variant draw), not "CI went green." The audit expectation is the CI run log(s) evidencing the disk-total and minimum-headroom facts per coverage-shard job.

## Decisions of note

- **Shape B (independent shards), not Shape A (merged).** Locked. Anchors: Simplicity + the emit-only regime + zero-new-Action supply-chain win. The cross-job merge is undocumented in `cargo-llvm-cov` (its merge is single-machine: `report` needs profile data **and** the instrumented object files co-located), so Shape A buys a union number no gate consumes at real complexity cost. Revisit tripwire: a coverage threshold gate (R-0095).
- **Partition is an adjustable calibration, probe-verified — not a locked count (R-0094).** The PG-vs-rest boundary maps to the observed accumulation/death point; whether two shards clear the floor is proven by the probe, and a shard within margin fires a finer split.
- **Single source of truth via delegation (R-0096).** The whole recipe delegates to the shard recipes so local and CI cannot enumerate membership differently. Avoids the second-enumeration silent-drift failure mode.
- **Probe is STEP 1, independently landable (R-0093).** It lands before the restructure so it captures the failing smaller-variant baseline (before/after on the same variant is the honest proof), and it stays as the standing roulette-detector while the disk tier is open.

## Amendment 2026-10-20 — operator-run PostgreSQL instance (#4119)

**Status: amendment to a `locked` (lean-track) spec, authored on the design lane 2026-10-20 against `main` at `65e58e0`, following the in-place dated-amendment precedent of the V0 substrate spec (#4018) and [P-0003](../src/adrs/P-0003-plugin-manifest.md) "Amendment 2026-07-07". Nothing locked is deleted.** Why now: P-0010 §Amendment 2026-10-19 lists `docs/specs/2026-07-15-ci-coverage-shard.md:75` ("the embedded-Postgres set is the heavy one") among the records the engine move falsifies and assigns its correction to the tier-2 amendment task.

| Decision | Resolution | Anchor | Rejected, with reason |
|---|---|---|---|
| The partition (R-0094) | Unchanged: `PG_TEST_FLAGS` vs rest; "embedded" is a superseded descriptor, not a superseded boundary | this spec's R-0094 rationale (b) — the cut maps to the observed accumulation point, which is wasmtime-driven and survives the engine move | Re-partition now because the engine's binaries left the tree — the probe decides partitions (R-0094's own rule), not a guess about what got lighter |
| Postgres per PG-executing job (R-0094, added clause) | The `ci` and `coverage-pg` jobs start the digest-pinned container by a job step with per-run TLS (R-0120-c as amended); `coverage-rest` starts none | R-0120-c; V-Consistency; P-TrustworthySignal; P-LeastAuthority (only the harness URL and CA path reach the job environment) | One shared server across jobs — each job is its own runner; start a server in every job "to be safe" — hides a mis-partitioned PG binary in the rest shard; a `services:` block — cannot present a per-run certificate |
| `scripts/ci-reap.sh` pin (R-0098) | Scoped to the sharding change; the script SHALL retire under R-0120-e as amended (a requirement with a grep, not a hand-off) | R-0120-e as amended; P-GuaranteeByMechanism; P-PreserveDecisionSpace | Read the pin as permanent — would keep kill authority with no object alive by a spec about disk |

**Implementation follow-ups implied:** per-run TLS generation and the digest-pinned container start in the `ci` and `coverage-pg` jobs (R-0120-c as amended); `verify-coverage-pg` exports `MNEMRA_TEST_RUN_ID` and runs the preflight (R-0120-c(4)); the `~/.theseus` cache steps and the `ci` job's `GITHUB_TOKEN` env retire with `verify-cold-start`, in the same change as the R-0120-g job (R-0120-c(3)); `scripts/ci-reap.sh` is retired, and the `ci` recipe's `EXIT` trap runs the leftover check, the end-of-run teardown and the post-teardown check instead, and prints `GATE leaked-objects` (R-0120-a(5) as revised 2026-10-22, R-0120-e; superseded: "the `ci` recipe's `INT`/`TERM` trap runs the end-of-run teardown instead, with `GATE leaked-objects` as the chain's last step"). **Not decided here:** nothing escalated; the coverage output shape, the threshold tripwire (R-0095) and the $0 constraint (R-0097) are untouched.

## Amendment 2026-10-29 — one image reference per supported PostgreSQL major, and the PG shard on the highest (#4291)

**Status: amendment to a `locked` (lean-track) spec, authored on the design lane 2026-10-29 against `main` at `0076b25`, in the in-place shape of this spec's 2026-10-20 amendment. Nothing locked is deleted. The coverage choice is the maintainer's ruling; the rest of each row is the #4291 draft's, until his exit approval of #4291.** Why now: the V0 substrate spec's R-0117-a as amended 2026-10-29 holds the host's supported PostgreSQL majors as one host code table, and its R-0120-c (5) has `ci.yml` hold one image reference per key, so this spec's single-reference wording in R-0094's added clause and its criterion is read per reference.

| Decision | Resolution | Anchor | Rejected, with reason |
|---|---|---|---|
| Postgres per PG-executing job (R-0094, added clause, as amended 2026-10-20) | The `ci` job runs its chain once per reference, as sequential legs, each starting its own container by job steps from that reference (V0 R-0120-c (5) as amended 2026-10-29); `coverage-rest` still starts none | V0 R-0120-c (5), (7); V-Consistency — one mechanism for every PG-executing job, per key; R-0097 — no new Action | A matrix of legs — legs writing one output name overwrite one another (INFERRED), so the per-leg server reports could reach the reconciliation only through a new artifact Action; a matrix keyed on the tag text — the tag is a label, not the server's report |
| `coverage-pg` on the highest key only | `coverage-pg` runs once, from the reference whose server reports the highest key, and fails `coverage_major_not_highest` otherwise; every key still gets its chain leg and fresh-machine job. Premise INFERRED, not measured: the host's line coverage does not vary with the server's major. The V0 R-0117-a (5) key-count pin names the premise, so it is re-read when the key count changes | Maintainer ruling 2026-10-29 (#4291 activity 9171, item 4, "Highest major only"); R-0095 — the coverage number has no consuming gate; V-Quality — a cost override of quality, named as one and ruled | `coverage-pg` once per key — its build and run paid per key for a number nothing reads; the option not selected |

**Not decided here:** the partition, the probe, the threshold tripwire (R-0095) and the $0 constraint (R-0097) are untouched.
