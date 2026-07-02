---
stakes: medium
date: 2026-07-02
status: locked
locked: 2026-07-02
consumer: mixed   # CI gate trust + future implementer + tier-1 parked-item closure (#1703/#1852)
spec_type: code
frame_relevant: true
---

# Intake: Tier-2 CI test-infra — shared engine per binary, per-test databases

## JTBD
Tier-1 (865f4dc) bought CI determinism by serializing the 14-binary embedded-PG/plugin test group at `--test-threads 1` over an engine-per-test model. The suite is now structurally slow — each PG binary serially boots one embedded engine per test (~100+ engine boots per full run; full CI ~12-13 min) — and carries vestigial concurrency machinery (per-binary STARTUP_LOCKs, including the early-release variant that never serialized anything) plus a lint blind spot (justfile clippy lacks `--all-targets`; 28 test-target warnings invisible to CI, 9 of them on the vestigial locks). Need: collapse to one shared engine per test binary with a fresh DATABASE per test, so the PG suite is fast AND stays deterministic — closing the tier-1 spec's parked Tier-2 (#1703) without regressing its SIGABRT=0 guarantee (#1852) or R-0006 isolation.

## Non-goals
- Mocking or externalizing Postgres (R-0018-b: real embedded PG + pgvector — locked).
- Changing R-0006 isolation *semantics* — per-test fresh-DATABASE on the shared engine preserves clean-DB-per-test; the isolation tests must pass unchanged.
- Resolving the #1852 abort frame as a deliverable of THIS work — resolved as separate gating evidence (Bolt dispatch 1255, verdict below); any wasmtime-side *remedy* is Frame-evaluable, not an intake commitment.
- The /health CONNECTION_DEADLINE + MNEMRA_ROOT seams (#2005 — separate lightweight /brief).
- smoke_e2e restructuring (boots the engine out-of-process via the real binary's run(); different surface, stays as-is).
- Disk-full / ci.yml free-space work (tier-1 parked operational item, #1860 family).
- cargo-nextest ADOPTION as a mandate — a named tier-2 design input (parked in tier-1 spec), weighed at Frame/spec time, not pre-decided here.

## Success criteria
- Engine-boot collapse observable: a full PG-suite run starts one embedded engine per member binary (~14-16 total; exact membership per the Frame-stage criterion below) instead of one per test (~100+) — count measured before/after (flake-runner or postmaster observation).
- Measured wall-clock before/after for the PG group across `verify-test` / `verify-test-hooks` / `verify-coverage`, recorded in the PR evidence (expected multi-minute CI reduction; the measurement is the deliverable; threshold set at spec).
- SIGABRT stays 0: flake-runner N ≥ 20/binary over every member post-refactor, local + at least one Linux CI-runner run (R-0020/R-0023 pattern reuse).
- R-0006 cross-workspace isolation tests pass UNCHANGED (tier-1 R-0025 hand-off honored: tier-2 re-verifies isolation against the shared-instance shape).
- Vestigial STARTUP_LOCKs retired (grep for the pattern returns nothing).
- `cargo clippy --all-targets --workspace` exits clean (0 warnings; currently 28, all test-target).
- `--all-targets` added to the justfile `verify-lint`/`check` clippy invocations so the warning class stays structurally dead.
- Parallelism restoration — condition RESOLVED (Bolt dispatch 1255, 2026-07-02): the discriminator returned **Hyp A** (wasmtime machports handler-thread abort, not PG Drop), so the PG-fixture refactor alone does NOT license un-serializing wasmtime-constructing binaries. Frame SHALL decide between (a) whole group stays serialized — engine-boot collapse is the win — and (b) group split: un-serialize PG-only members, keep wasmtime-touching members (`mcp_server`, `mcp_slice1_e2e`, `mcp_verb_gate`, any PluginPool constructor) serialized — with flake-runner proof either way. Any wasmtime-side remedy (engine singleton/reuse, `Config::macos_use_mach_ports(false)` vs P-0007 trap-handling trade-offs, wasmtime upgrade, explicit `unload_process_handlers`) is a Frame-stage design question, in scope for evaluation, not pre-decided here.

## Hard constraints
- R-0018-b — real embedded PG + pgvector; serialize/share, never mock or externalize.
- R-0006 — isolation semantics preserved (fresh DB per test).
- P-TrustworthySignal — measured before/after IS the deliverable; rerun-to-green and accept-as-residual-risk are forbidden dispositions.
- Tier-1 spec discriminator tripwire (verbatim): "the frame SHALL be isolated (per-binary peak-concurrency attribution + an abort backtrace on a clean box or the Linux runner) **before** Tier 2 is committed as the #1852 remedy." — SATISFIED 2026-07-02 (Bolt dispatch 1255, Hyp A; see Consultations).
- Uniform effect across `verify-test` / `verify-test-hooks` / `verify-coverage` (R-0022 pattern); code-traveling mechanism favored over per-recipe edits.
- Global R-ID series continuation (the tier-1 spec owns R-0020–R-0025; this spec continues the series).
- **Membership criterion must be transitive-construction-aware** (Warden r1 Medium): tier-1's locked grep (`... | grep -v '/common/'`) misses binaries that obtain engines through shared harnesses (`tests/common/paging_harness.rs` — 2 binaries today) and through the production bootstrap path (`startup_run_full` via `run_with`). Tier-2's membership criterion SHALL enumerate by *reachable engine construction*, not direct-reference grep — mechanism designed at Frame.

## Evidence
- Tier-1 spec `docs/specs/2026-06-26-ci-flake-tier1-serialize.md` (locked, approved 2026-06-26) — parks this exact refactor as Tier-2 with tripwires; inlines the dispatch-1122 diagnosis numbers (dose-response 1-thread 0/20 vs 10-thread 3/20; engine-count confound admin_token 13→0/20 vs mcp_server 14→3/20).
- 2026-07-02 recon (file:line), counts reconciled per Warden r1: tier-1's locked grep returns exactly its 14 known members at HEAD; additionally 2 binaries reach engines via the shared paging harness and `startup_run_full` boots one via the production `run_with` path (≈16 in-process constructors total; smoke_e2e boots one more out-of-process via the real binary). No engine sharing anywhere; no explicit teardown (implicit Drop, `.temporary(true)`); STARTUP_LOCK early-release variant at `admin_token.rs:74-84` / `mcp_server.rs:115-123` / `paging_harness.rs:50-58`; A-10/A-11 residual-risk docs `postgres_engine.rs:14-35`; 60s startup deadline `engine.rs:408`.
- Abort-frame evidence (Bolt dispatch 1255, 2026-07-02): 19 independent macOS crash reports, all showing `wasmtime::runtime::vm::sys::unix::machports::handler_thread` → `abort()`; wasmtime 45.0.2 source-verified (process-global singleton spawned with first Engine; torn down only by explicit unsafe `Engine::unload_process_handlers`, never called by mnemra).
- CI timing: 12m24s / 12m43s full-CI runs post-M2 (2026-07-02).
- clippy `--all-targets`: exit 0 with 28 warnings, all test-target; justfile lint recipes lack `--all-targets` (grep: none).
- Zero deadline-flake CI firings since 865f4dc (tier-1) — determinism currently holds; the residual costs are wall-clock + structural debt.

## Consumer of resulting work
Mixed: the CI gate (trust + speed), future implementers (test-infra they build on), maintainer closure of #1703/#1852.

## Risk profile
No trust boundary (test harness + justfile lint gates; no auth/PII/network/runtime surface). Guarded adjacent surface: R-0006 isolation tests (in-scope regression guard). May-touch-trust-boundary: NO. Warden r1 additionally ruled out a shared-engine multi-tenancy/RLS-analog risk: Postgres binds a connection to one database at connect time (no session-mutable database switch), so cross-test-database isolation under a shared engine is structural, not session-state-dependent.

## Consultations
- Bolt (2026-07-02, dispatch 1255): abort-frame discriminator per the tier-1 tripwire — **verdict Hyp A**, high confidence (evidence above). The tier-1 tripwire is SATISFIED; parallelism consequences recorded in Success criteria. Report: workspace `scratch/dispatch-1255-report.md`.

## Review history
- r1 — Warden (dispatch 1256, 2026-07-02): **pass**, 0 blocker / 0 high / 1 medium / 2 low / 1 nit; all folded into r2 (membership-criterion constraint added; evidence counts reconciled; bundled success criterion split; clippy-figure nit stands, corroborated). Report: workspace `scratch/dispatch-1256-report.md`.

## Decisions resolved at intake-exit (2026-07-02, maintainer)
- Intake locked at **medium stakes** (envelope default confirmed).
- `spec_type=code` ratified; `frame_relevant=true` (forced for code).
- Parallelism direction → **Frame decides, both options open** (whole-group-serialized vs PG-only/wasmtime split), with flake-runner proof obligations either way; engine-boot collapse confirmed as sufficient standalone win.
- Environment unblocked for tier-2 measurement: 24 zombie embedded postmasters (SHMEM exhaustion, three crashed-session clusters) killed post-verification 2026-07-02.
