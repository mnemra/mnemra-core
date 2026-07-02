---
date: 2026-07-02
status: draft
revision: r2 (Warden Stage-2 review r1 folded — 2 medium, 1 low; 2026-07-02)
intake: ./ci-flake-tier2-shared-engine.md
modulation: brownfield-extension
modulation_override: >
  Literal modulation rule overridden to brownfield-extension per the tier-1
  precedent (./ci-flake-tier1-serialize-frame.md): mnemra-core is a pre-/brief
  mature project, and this work extends the locked tier-1 test-infra surface
  (test harness + lint gates only, no architectural surface). Stage 2a
  elicitation skipped per modulation; inputs are the locked intake, canon, and
  the cited evidence reports — nothing else.
---

# Frame: Tier-2 CI test-infra — shared engine per binary, per-test databases

## Operating constraints

Constraint-graph walk from the validated intent (fast AND deterministic PG
suite). Edge types per `constraint-edges.md`; most-specific applies (per-task >
project ADR > workspace ADR > principle). No `conflicts-with` edge fired — see
Escalated decisions.

- **R-0018-b** (real embedded Postgres + pgvector; serialize/share, never mock
  or externalize) — `refines` the mechanism space: the admissible collapse is
  *sharing* the real engine, never mocking or externalizing it. The fixture
  keeps `postgresql_embedded` in-process. *No conflict.*
- **R-0006** (tenant enforcement; isolation tests see a clean DB per test) —
  `refines` the fixture contract: per-test fresh **DATABASE** on the shared
  engine preserves clean-DB-per-test semantics; the cross-workspace isolation
  tests pass unchanged (F-1, F-2). *No conflict — semantics preserved, only the
  engine multiplicity changes.*
- **P-TrustworthySignal** (`specializes` Observability + Quality) — governs the
  whole change: the measured before/after IS the deliverable; rerun-to-green
  and accept-as-residual-risk are forbidden dispositions (TS1). Every locked
  decision below carries a measured proof obligation. It also motivates F-6:
  a lint gate that cannot see 28 warnings is a signal that under-reports.
- **P-TDDPairs T4** (tests isolate process-global state), general home
  **P-TrustworthySignal** — the shared engine IS deliberately-introduced
  process-global state; isolation is preserved structurally (separate database
  catalogs per test) plus the retained group serialization (F-2). *No conflict:
  T4 requires isolation of the state's effects, which per-test databases
  provide.*
- **R-0022 pattern / R-0018-f** (uniform effect across `verify-test` /
  `verify-test-hooks` / `verify-coverage`; `just ci` sole CI entry;
  code-traveling favored) — `depends-on`: the collapse mechanism must be
  inherited by all three recipes. Resolved by F-1's fixture living in test
  code (`libs/mnemra-host/tests/common/`), not in recipe flags.
- **P-LockContract** — the fixture is the single locked seam for test-side
  engine acquisition; implementations behind it (boot, database provisioning,
  teardown mechanism) vary at implement time. Tier-1's
  constraint-not-mechanism precedent
  (`docs/specs/2026-06-26-ci-flake-tier1-serialize.md` §Purpose) carries
  forward.
- **P-MinBlastRadius** — test-infra-only change; the production containment
  configuration (P-0007: fuel + epoch resource limits, machports
  hardware-trap handling) is NOT touched (F-4).
  A production-config change to fix a test-teardown symptom would be
  blast-radius-disproportionate.
- **P-Defer (DF1)** — every deferral below names what mechanically fires it;
  both deferrals ride on measurements this spec already mandates, so firing is
  structural, not "someone remembers."
- **Membership criterion — transitive-construction-aware** (intake hard
  constraint, from the intake's review r1 Medium) — `refines` R-0021's
  tier-1 criterion: enumeration by *reachable engine construction*, not
  direct-reference grep. Mechanism locked at F-3; the M-1 verification loop
  is closed by F-3's third seeded-violation proof (the transitive can-fail
  case).
- **Tier-1 discriminator tripwire** — SATISFIED 2026-07-02 (Bolt dispatch
  1255, verdict Hyp A: the aborting frame is wasmtime's process-global
  machports handler thread, not PG Drop). This Frame is authored *after* the
  tripwire cleared, as the tier-1 spec required.
- **Global R-ID series continuation** — tier-1 owns R-0020–R-0025; the tier-2
  spec continues the single global series (next free: R-0026).

## Rationale chain

Validated intent (collapse ~100+ engine boots to one per member binary; keep
SIGABRT = 0 and R-0006 isolation; kill the vestigial locks and the lint blind
spot) → constraints above → six locked decisions F-1..F-6. Each lock cites its
canon anchor; each deferral names its tripwire; every requirement-shaping
observable is binary.

### F-1 — Shared-engine fixture architecture (LOCKED)

**Decision.** One embedded engine per test binary, booted lazily on first use
through a **single shared fixture entry point** in
`libs/mnemra-host/tests/common/` (a new fixture module; name is
implement-time). Each test obtains, from that one entry point: (1) the
binary's shared `EmbeddedEngine` (get-or-init; first caller boots), (2) a
fresh, uniquely-named **DATABASE** created on it, schema-init'd (including
per-database `CREATE EXTENSION vector`), and (3) an application-role pool
bound to that database. *Anchors: R-0018-b (share, never mock/externalize);
R-0006 (clean DB per test preserved); P-LockContract (one seam, varying
implementation); Simplicity (smallest mechanism that collapses boots).*

Locked sub-decisions:

- **Lifecycle/teardown.** The shared engine SHALL have **explicit,
  deterministic teardown at binary exit**. Reliance on `Drop` of a
  `static` is forbidden — Rust statics never drop, so a
  static-without-teardown guarantees one leaked postmaster per binary run
  (~17/full-CI-pass, recreating exactly the zombie backlog that masked the
  abort-frame measurement, per `scratch/dispatch-1255-report.md`
  deviations). Today's per-test implicit Drop
  (`libs/mnemra-host/storage/postgres/engine.rs:375-385` `_server:
  PostgreSQL`, `.temporary(true)` at `:409`) only works because each engine
  is a local. The teardown *mechanism* (e.g. exit-time hook, guardian
  teardown, custom harness) is implement-time HOW per the tier-1
  constraint-not-mechanism precedent, bounded by: no non-Green dependency;
  fires on normal process exit including the all-tests-passed path.
  **Observable:** after every member-binary run that exits normally (pass or
  fail), live `bin/postgres` postmaster count returns to the pre-run
  baseline (`scripts/flake-runner.sh` already measures this as
  `BASE_PM`/`PEAK_DELTA`; R-0024 reaping stays as the measurement backstop,
  not the primary mechanism).
- **Database provisioning is a named privileged method.** Creating the
  per-test database requires the engine's privileged surface;
  `superuser_pool` is deliberately `pub(crate)` with privileged ops exposed
  as named methods (A-14 narrowing,
  `libs/mnemra-host/storage/postgres/engine.rs:380-384`). The fixture SHALL
  obtain databases via a named method on `EmbeddedEngine`, not by re-widening
  the pool field or granting the app role new privileges. **Observable:** the
  role-shape test (`libs/mnemra-host/tests/postgres_engine.rs`
  `app_role_is_not_superuser_and_not_bypassrls`) passes unchanged.
- **No per-test DROP DATABASE.** Test databases live for the binary's
  lifetime and die with the ephemeral engine (`.temporary(true)`); a
  per-test drop is mechanism without evidence of need. *Anchor: P-Defer /
  Simplicity.* Self-announcing tripwire: if a member binary's database count
  ever exhausts engine resources, the failure announces itself in that
  binary's run.
- **STARTUP_LOCK retirement.** Boot serialization is inherent to the
  fixture's get-or-init; all per-binary `STARTUP_LOCK` statics — including
  the early-release variants that never serialized anything
  (`libs/mnemra-host/tests/admin_token.rs:74-84`,
  `libs/mnemra-host/tests/mcp_server.rs:115-123`,
  `libs/mnemra-host/tests/common/paging_harness.rs:50-58`) — retire.
  **Observable:** the intake's grep criterion (pattern returns nothing).
- **Code-traveling uniformity (R-0022 pattern).** The fixture lives in test
  code, so *any* invocation path — the three `verify-*` recipes and any
  future fourth — inherits the collapse. The `--test-threads 1` directive
  remains recipe-level but single-sourced through the shared `PG_TEST_FLAGS`
  variable (`justfile:96`), as tier-1 landed it. **Observable:** per-recipe
  measured runs (R-0022 AC pattern reused).

**Quality-attribute scenarios (decision unit):**

- *Wall-clock:* [stimulus: full PG-group run under each of the three
  `verify-*` recipes; environment: local + Linux CI runner, warm cache;
  response: one engine boot per member binary (~17 at HEAD, see F-3) instead
  of one per test (~100+); measure: boot count before/after
  (flake-runner/postmaster observation) — pass = one boot per member binary
  — plus wall-clock before/after recorded in PR evidence and compared
  against the spec-set threshold in its F-2 tripwire role only (the
  wall-clock number gates nothing; see F-2's threshold-role lock)].
- *Determinism:* [stimulus: `scripts/flake-runner.sh` N ≥ 20/member over
  every member post-refactor; environment: local macOS + ≥ 1 Linux CI-runner
  run; response: zero ABORT classifications; measure: SIGABRT = 0
  (R-0020/R-0023 pattern reuse)].
- *Isolation:* [stimulus: R-0006 cross-workspace isolation tests on the
  shared-engine fixture; environment: post-refactor; response: assertions
  pass with helpers unedited; measure: green + `git diff` empty on
  `libs/mnemra-host/tests/common/mod.rs` isolation helpers (`:135`, `:177`)].
- *Faithful instrument:* [stimulus: any member run completes; environment:
  local/CI; response: postmaster count returns to baseline; measure: count ==
  `BASE_PM`].

### F-2 — Parallelism: the whole group stays serialized (LOCKED, option a)

**Decision.** The PG/plugin group keeps `--test-threads 1` (tier-1's R-0021
shape) — the engine-boot collapse is the win this spec ships; test-body
parallelism is NOT restored, for any member, in tier-2. *Anchors: Simplicity
(one group, one directive; a PG-only/wasmtime split adds a second maintained
partition); P-TrustworthySignal (un-serializing introduces a new
nondeterminism surface — concurrent `CREATE DATABASE` template contention is
itself a known flake class — on the gated line, for a second-order win);
P-Defer (the maintainer confirmed engine-boot collapse as a sufficient
standalone win; parallel bodies are mechanism ahead of evidence); the Hyp A
evidence (Bolt dispatch 1255).*

Why the split (option b) loses after anchoring: Hyp A makes PG-only members
*structurally immune to the #1852 abort class* (no wasmtime engine → no
machports handler thread), so option b is *safe* in the abort dimension — but
safety is not sufficiency. The split's benefit is second-order wall-clock
(post-collapse, per-test cost is database-create + body, not engine boot),
while its costs are first-order: a maintained wasmtime-reachability partition
on top of F-3's membership mechanism, per-member N ≥ 20 re-proof on the
un-serialized configuration, and fixture-side serialization of `CREATE
DATABASE` becoming load-bearing under concurrency. When the anchor citation
leaves both options canon-admissible, Simplicity + P-Defer break the tie
toward (a). This also collapses the fixture design: under `--test-threads 1`,
database creation is trivially serial (F-1 still guards it inside the fixture
so the invariant does not silently depend on the recipe flag).

**Deferral (option b), all three elements per DF1:** (1) *Decision content:*
un-serialize the PG-only members (those with no reachable wasmtime `Engine`
construction), keep wasmtime-touching members serialized, with per-member
flake-runner proof N ≥ 20 on the new configuration. (2) *Deferral anchor:*
P-Defer — adopt when evidence forces; the evidence does not exist until the
post-collapse wall-clock is measured. (3) *Tripwire (mechanically fired):*
the before/after wall-clock measurement this spec already mandates as its
deliverable. The spec SHALL require the PR evidence to record the
post-collapse PG-group wall-clock against a threshold set at spec-exit; a
measurement above threshold routes the split decision to the maintainer as a
new intake. The measurement is mandatory, so the comparison cannot silently
not-happen.

**Threshold role (LOCKED): split-tripwire only — never an acceptance gate
on the collapse.** The two readings are mutually exclusive on the same
number: an acceptance gate requiring wall-clock ≤ threshold makes the
above-threshold tripwire unfireable, and a fired tripwire would block the
very collapse it exists to follow up on. The spec SHALL therefore use the
wall-clock threshold exclusively in the tripwire role: an above-threshold
measurement ships the collapse AND routes the split decision (option b) to
the maintainer; it does not fail acceptance. Acceptance on the performance
dimension is structural and binary — one engine boot per member binary
(F-1's wall-clock QA measure) — plus the recorded before/after measurement
itself, which is the intake's deliverable ("the measurement is the
deliverable; threshold set at spec": the spec sets the threshold's *value*;
its *role* is locked here). *Anchors: P-TrustworthySignal (gating
acceptance on the number the measurement exists to report would pressure
the instrument; the trustworthy acceptance claim is the boot-count
collapse, which is structural); P-Defer (the threshold routes evidence to
the deferred decision; it does not gate the shipped one).*

**Proof obligation either way (intake lock honored):** flake-runner N ≥
20/binary over every member post-refactor, local + ≥ 1 Linux CI-runner run,
SIGABRT = 0 — the refactor leaves the per-test wasmtime lifecycle untouched
(F-4), so tier-1's measured suppression (1-thread → 0/20) is expected to
carry, and the re-measurement verifies rather than assumes it
(P-TrustworthySignal).

### F-3 — Membership criterion: construction chokepoint + guard lint + partition reconciliation (LOCKED)

**Decision.** Tier-2 replaces tier-1's direct-reference grep with a
**structural criterion**: membership is defined by *reachable engine
construction*, and the refactor itself makes reachability enumerable by
funneling all construction through two named chokepoints. *Anchors: intake
hard constraint (review r1 M-1); P-LockContract (the fixture seam is the
enumeration point); P-ShiftLeft D1 (scope known mechanically, not by audit);
PD3 (the rule and its enforcing tool must agree — the criterion ships with
its checker); P-TrustworthySignal (a membership list that silently omits
binaries is an instrument that under-reports).*

The mechanism, three parts, each binary-observable:

1. **Chokepoint.** All test-side embedded-engine construction goes through
   the F-1 fixture module. The only other engine-construction path reachable
   from a test binary is the production bootstrap
   (`mnemra_host::run_with` → `PostgresStorage::start_embedded()`, exercised
   by `libs/mnemra-host/tests/startup_run_full.rs`; `smoke_e2e.rs` boots
   out-of-process and stays its own gate per the intake Non-goal).
   Membership = binaries that (transitively, through `#[path]`-included
   `tests/common/` modules) reference the fixture module, ∪ the binaries on
   the guard lint's named production-bootstrap caller list (part 2's by-name
   exceptions — the same list, so enumerator and lint cannot drift; PD3).
   The enumerator is **source-derived**: it walks each test binary's sources
   (root file plus its `#[path]`-included `tests/common/` modules) for
   fixture references and unions in the named bootstrap callers. It is never
   derived from, seeded by, or filtered against `PG_TEST_FLAGS` — the flag
   list is the *compared* side of part 3's clause (a); an enumerator echoing
   it would compare the list to itself and could not fail (vacuity).
2. **Guard lint.** A CI check FAILS if engine-construction symbols
   (`EmbeddedEngine::start`, `start_embedded`, `postgresql_embedded`) appear
   in `libs/mnemra-host/tests/` outside the fixture module (production-
   bootstrap callers excepted by name). Same enforcement family as the
   repo's existing grep-shape lints (`lint_workspace_clause.rs`,
   `no_test_seams.rs`) — a structural fix, not a prose rule. This is what
   makes the criterion *transitive-construction-aware going forward*: a new
   binary cannot construct an engine except through a chokepoint the
   enumerator sees.
3. **Partition reconciliation.** The partition is defined over
   **full-binary execution cells** — invocation sites that run a binary's
   complete test set: `PG_TEST_FLAGS` (`justfile:96`), `NONPG_TEST_FLAGS`
   (`justfile:107`), and the named full-binary standalone gates (today
   exactly one: `smoke_e2e` via `verify-smoke`, `justfile:229`). **Filtered
   supplementary invocations** — a name-filtered subset of a binary run as
   an additional gate (today exactly one: `verify-signing-root`'s `--exact
   root_pin_gate_matches_embedded` run of `build_gate`, `justfile:266`) —
   are NOT cells; the check carries them as a separately-enumerated
   supplementary list, outside the partition. What the check computes: for
   every `libs/mnemra-host/tests/*.rs` integration binary, the count of
   cells that execute its full set. It FAILS iff (a) the computed
   membership (part 1's source-derived enumeration) differs from the
   `PG_TEST_FLAGS` contents in either direction, or (b) any binary's cell
   count ≠ 1 — count 0 is a never-run binary (a filtered supplementary
   invocation does not rescue it: its full set still never runs), count ≥ 2
   is a genuinely twice-executed one. Under these semantics `build_gate` as
   it exists today PASSES (one cell, `NONPG_TEST_FLAGS`; its signing-root
   invocation is listed as supplementary, not counted), and a binary present
   in both flag lists, or in neither, FAILS.
   Clause (b) exists because the blind spot has already produced a live
   defect stronger than un-collapsed boots: **`artifact_list_paging.rs` and
   `artifact_list_paging_whitebox.rs` are in NEITHER flag list and are
   therefore never executed by any `just ci` recipe today** (verified at
   this worktree's HEAD: commit `079f520` landed the binaries and the GREEN
   implementation but did not touch the `justfile`). The reconciliation at
   implement time wires both into membership.

**Membership at HEAD under this criterion (17 in-process members):** the 14
tier-1 grep members + `startup_run_full` (production bootstrap; already in
`PG_TEST_FLAGS`) + `artifact_list_paging` + `artifact_list_paging_whitebox`
(paging-harness-mediated; currently unlisted — reconciled in). The intake's
"~14-16" bracket is superseded by this enumeration per the intake's own
deferral ("exact membership per the Frame-stage criterion").

**Observables:** the guard lint and reconciliation check exist as CI-gated
checks (run under `verify-lint` or a sibling recipe — placement is
implement-time) and each demonstrably fails on a seeded violation — the
can-fail proof per P-ShiftLeft's spec-falsifiability mirror. Three seeded
violations, one per failure surface: (1) *guard lint* — a test constructing
an engine directly, outside the fixture module; (2) *partition* — a binary
present in neither flag list; (3) *transitive enumeration* — a member that
reaches the fixture only through a `#[path]`-included `tests/common/` shim
appears in the computed membership, and clause (a) flags it when
`PG_TEST_FLAGS` omits it. Proof (3) closes the M-1 verification loop
(intake review r1): the chokepoint design discharges M-1, and the seeded
transitive case proves the enumerator fires on `/common/`-mediated
construction rather than inheriting the grep blind spot.

### F-4 — Wasmtime-side remedy: none in tier-2 scope (LOCKED: no change; remedy DEFERRED with tripwire)

**Decision.** Tier-2 makes **no wasmtime-side change**: no engine
singleton/reuse, no `Config::macos_use_mach_ports(false)`, no wasmtime
upgrade, no `Engine::unload_process_handlers`. The per-test
`PluginPool`/wasmtime lifecycle is untouched. *Anchors: P-MinBlastRadius (the
abort is suppressed by the retained serialization — tier-1 measured 0/20 at 1
thread — so a production-config change would widen blast radius to fix an
already-suppressed test symptom); P-0007 (machports backs wasmtime's
hardware-trap handling path — guest hardware faults become recoverable
traps — foundational to the sandbox's fail-safe posture; the fuel/epoch
kill-and-replace paths are software interruption mechanisms that bypass
machports entirely, per Bolt 1255; `macos_use_mach_ports(false)` re-routes
production hardware-trap handling to fix a test-only teardown race —
disproportionate and security-adjacent); P-Defer (a wasmtime upgrade on the
hope of an upstream fix is unverified, evidence-free adoption); Honesty /
verify discipline (Bolt could not pin which of the two `machports.rs` abort
branches fires — `scratch/dispatch-1255-report.md` risks — so any targeted
remedy would be aimed at an unconfirmed internal branch).*

The option space was evaluated as the intake required: *engine
singleton/reuse* would also collapse per-test wasm engine boots but is a
speed optimization outside the JTBD (PG-suite collapse) and adds a second
shared-fixture surface for no determinism gain under F-2; *mach-ports-off*
trades a P-0007-adjacent production trap-handling property against a test
symptom; *upgrade* is unverified; *unload_process_handlers* is `unsafe`,
last-Engine-asserting, and ordering-fragile across tests. All four lose to
"no change" while serialization holds.

**Deferral, all three DF1 elements:** (1) *Decision content:* pick among the
four remedies (or an upstream fix) for the machports handler-thread abort.
(2) *Deferral anchor:* P-Defer — under F-2 the abort has no firing surface;
remedy shape should be chosen against the evidence that re-exposes it.
(3) *Tripwire (mechanically fired):* either (i) the F-2 split tripwire fires
and option b is pursued — its mandated N ≥ 20 proof on un-serialized
wasmtime-adjacent members is exactly the measurement that would re-surface
the abort — or (ii) this spec's own mandatory post-refactor flake-runner
measurement reports any ABORT classification (SIGABRT > 0 fails the spec's
acceptance gate outright, forcing the remedy conversation). Bolt's follow-up
(filing upstream against wasmtime 45.0.2 `machports.rs`) is recorded in the
completion report as an out-of-repo action for the orchestrator, not spec
scope.

### F-5 — cargo-nextest: DON'T ADOPT (LOCKED)

**Decision.** cargo-nextest is not adopted in tier-2. *Anchors: F-1 (the
locked fixture architecture) — nextest's core execution model is
process-per-test, and a shared-engine-per-binary fixture relies on in-process
sharing across tests within one binary: under nextest every test is a fresh
process, so the "shared" engine boots once per test again and the collapse
this spec exists to ship is structurally defeated; Simplicity (a new runner +
config surface with negative interaction); S2 (installed-base popularity is
not a fit claim — the fit test fails here on the execution model itself).*
The tier-1 parking noted nextest would convert a binary-wide abort into a
single failed test; under F-1+F-2 the abort is eliminated-by-measurement
rather than cosmetically contained, which is the stronger property
(P-TrustworthySignal).

This is a **don't-adopt, not a deferral** — there is no tripwire to name
because the conflict is with a locked decision, not with missing evidence.
Re-opening is self-announcing per DF1: adopting nextest would require
re-deriving F-1's fixture architecture, which re-opens this Frame through
the normal amendment path (P-LockContract: a lock is scoped to the world it
was made against — if the fixture architecture is ever re-derived, the
nextest question re-derives with it).

### F-6 — clippy `--all-targets` gate (LOCKED, routine)

**Decision.** The `justfile` clippy invocations (`verify-lint`, `check` —
`justfile:4-5, 128-129`) gain `--all-targets` for the host-workspace
invocation, and the 28 currently-invisible test-target warnings are driven to
zero (9 die with F-1's STARTUP_LOCK retirement; the remainder are fixed, not
allowed). *Anchors: P-TrustworthySignal (a lint gate blind to an entire
target class under-reports — the faithful-instrument clause); intake success
criteria (verbatim commitments).* Whether `--all-targets` also applies to the
`wasm32-wasip2` `mnemra-echo` invocation is implement-time (building test
targets for a wasm component may not be meaningful); the binding observables
are the intake's: `cargo clippy --all-targets --workspace` exits clean with 0
warnings, and the flag is present in the justfile lint recipes so the warning
class stays structurally dead. Batched as routine — within principles, no
rework of done work.

## Spec obligations (named)

Spec-tier items this Frame binds Stage 3 to disposition explicitly — each a
named obligation the spec addresses on its face (design the mechanism, or
state why none is needed), inherited here rather than from review
archaeology. Warden's Stage-2 review r1 domain questions Q2–Q4 route here;
Q1 (threshold role) is answered in-Frame at F-2's threshold-role lock.

- **SO-1 — Per-test pool lifecycle (F-1).** State whether each test's
  app-role pool is scope-dropped at test end or cached for the binary's
  lifetime, so connections do not silently accumulate on the shared engine
  across a binary's run. Under the per-test-engine model connections died
  with each engine; under the shared engine they persist iff pools are
  cached. The no-DROP-DATABASE tripwire self-announces on any resource
  exhaustion (connections included) — the obligation is to state the
  mechanism, not to add one.
- **SO-2 — Teardown vs. abort exit-window ordering (F-1/F-4).** One spec
  note on whether the chosen exit-time teardown mechanism alters
  process-exit *ordering* relative to wasmtime's machports handler-thread
  shutdown: F-1 introduces a new exit-time operation in the same
  exit-adjacent window where the #1852 abort fires, so "wasmtime lifecycle
  untouched → tier-1 suppression carries" (F-2's proof paragraph) needs the
  interaction stated, not assumed. Low risk — the dose-response evidence
  tracked *concurrent* teardown, F-2 keeps 1-thread, and the mandatory
  N ≥ 20 measurement backstops it — but the statement must exist.
- **SO-3 — R-0006 unedited byte-set (declared strain #1).** Pin the precise
  byte-set the "isolation tests pass UNCHANGED" guard covers: only the
  `tests/common/mod.rs:135/:177` helper functions, or also the isolation
  assertion-call bodies inside member files (e.g.
  `storage_contract_postgres.rs`) whose engine-acquisition line migrates to
  the F-1 fixture. The guard must be checkable (`git diff` empty on a named
  set), not interpretive.

## Consultations

None needed at Frame — no question surfaced that requires an operational,
feasibility-depth, or testability consultation before spec authoring.
Evidence consulted in written form:

- Bolt dispatch 1255 (abort-frame discriminator, verdict Hyp A) — workspace
  `scratch/dispatch-1255-report.md`; load-bearing for F-2 and F-4.
- Warden dispatch 1256 (intake review r1; membership blind spot M-1;
  shared-engine RLS-analog ruled out) — workspace
  `scratch/dispatch-1256-report.md`; load-bearing for F-3 and the risk
  profile.
- Warden Stage-2 Frame review r1 (2 medium / 1 low, folded in this r2
  revision) — workspace `scratch/ci-flake-tier2-frame-review-r1.md`;
  load-bearing for F-3's partition-cell and enumerator semantics, F-4's
  machports attribution, F-2's threshold-role lock, and spec obligations
  SO-1..SO-3.
- Tier-1 locked spec `docs/specs/2026-06-26-ci-flake-tier1-serialize.md`
  (R-0020–R-0025 patterns reused; parked items consumed as designed).
- Code recon at this worktree's HEAD: `justfile:88-107, 120-242`;
  `libs/mnemra-host/tests/common/paging_harness.rs`;
  `libs/mnemra-host/tests/postgres_engine.rs:12-52` (A-10/A-11/A-12);
  `libs/mnemra-host/storage/postgres/engine.rs:375-489`; commit `079f520`
  (paging binaries landed without justfile wiring).

## Routine decisions (batched)

- **Modulation override** (frontmatter): brownfield-extension per the tier-1
  precedent. Within-principle.
- **F-6 clippy `--all-targets`** — routine per above.
- **No per-test DROP DATABASE; databases die with the ephemeral engine**
  (F-1 sub-decision). Within P-Defer/Simplicity.
- **STARTUP_LOCK retirement mechanics** fall out of F-1's get-or-init; no
  separate decision surface.
- **No novel ADR.** Tier-2 introduces no new *architectural* decision — it
  is a test-infrastructure restructure fully bounded by existing R-0018-b /
  R-0006 / R-0022-pattern constraints, P-TrustworthySignal, P-LockContract,
  and P-0007 (untouched). The constraint-graph walk surfaced no novel or
  escalation-triggering edge. The fixture design is spec-tier content
  (reversal re-opens the spec, not project canon).
- **R-ID continuation:** the tier-2 spec opens at **R-0026**.

## Escalated decisions

None fired. The one decision the maintainer explicitly delegated with both
options open (parallelism) locked at F-2 by canon anchoring
(Simplicity + P-Defer + P-TrustworthySignal breaking a canon-admissible tie);
no `conflicts-with` edge required a pause-and-escalate, and no canon
amendment is proposed. The live finding that two test binaries are absent
from every CI list (F-3) is a downstream inconsistency *given* tier-1's
locked text — surfaced here and in the completion report for maintainer
visibility, repaired in-scope by F-3's reconciliation; tier-1's text itself
is not re-opened.

## Risk profile (resolved)

**No trust boundary** — confirmed against the now-known mechanism. The change
is test harness + justfile lint gates; no auth / PII / network / production
runtime surface. Mechanism-specific re-checks:

- **Cross-test isolation under a shared engine is structural, not
  session-state-dependent:** Postgres binds a connection to one database at
  connect time (no in-session database switch analogous to a mutable GUC), so
  the P-0010-style pooled-connection RLS-leak analog does not apply — ruled
  out at intake review r1 and unchanged by the F-1 fixture shape (each test's
  pool is constructed against its own database's catalog).
- **Privilege surface:** per-test database provisioning uses the engine's
  privileged surface behind a named method (A-14 discipline, F-1); the
  application role gains no privilege; the role-shape test remains green and
  unedited.
- **Guarded adjacent surface:** the R-0006 isolation tests are the in-scope
  regression guard (F-1 isolation QA scenario; helpers unedited).
- **Residual risks A-10/A-12** (zombie postmaster / temp-dir leak on
  SIGKILL, `libs/mnemra-host/tests/postgres_engine.rs:12-41`): unchanged in
  kind, reduced in magnitude by F-1 (at most one engine per binary instead of
  one per test, and explicit exit-time teardown handles all normal exits).
  These are not gate-signal defects (not a flake disposition on the merge
  line — the P-TrustworthySignal TS1 boundary); measurement fidelity against
  leak-masking remains guarded by R-0024's reaping harness. A-11
  (cross-binary extraction race) loses its within-binary half structurally
  (one boot per binary) and keeps its cross-binary posture (cargo runs test
  binaries sequentially; F-5 keeps it that way).
- Standard code+security review applies at the spec gate; no security-mode
  trigger.

## Intent self-report

**(a) JTBD as read:** Tier-1 bought CI determinism by brute-force
serialization and left the suite structurally slow (~100+ serial engine boots
per full run) with vestigial lock machinery and a lint blind spot. This work
collapses engine boots to one per member binary with a fresh database per
test — keeping the determinism guarantee (SIGABRT = 0, re-proven by
measurement, never asserted) and the R-0006 isolation semantics exactly as
they are — and retires the vestigial locks and the `--all-targets` blind spot
so the gate reports everything it should. Speed AND trust, proven
before/after; the measurement is the deliverable.

**(b) Decisions that strain or diverge from an enumerated Non-goal or
Success criterion — declared:**

1. **"R-0006 isolation tests pass UNCHANGED" is read as: assertions,
   helpers, and isolation semantics unchanged** (`tests/common/mod.rs:135,
   :177` unedited; tests green) — while the engine-*acquisition* line in
   member test files necessarily migrates to the F-1 fixture. A strictly
   file-byte-identical reading of "unchanged" for `tenancy_isolation.rs` /
   `storage_contract_postgres.rs` is unsatisfiable under any shared-engine
   refactor; the spec will pin the precise unedited set so the guard is
   binary (named obligation SO-3).
2. **Membership = 17, outside the intake's "~14-16" bracket** — superseded
   per the intake's own deferral to the Frame-stage criterion; declared
   because the bracket was in a Success criterion.
3. **F-3's partition-reconciliation clause (b)** (every integration binary
   in exactly one full-binary execution cell) extends the membership
   mechanism to the complement set. Declared as potential scope-strain: it is the smallest
   shape that makes the enumeration self-verifying at both boundaries, and
   it is what catches the already-live defect (two binaries CI never runs);
   it adds no new tooling class beyond the check F-3 requires anyway.
4. **Wiring `artifact_list_paging(_whitebox)` into the CI lists** is
   membership reconciliation under Success criterion #1, not new scope — but
   it will surface as new CI execution time and (if those suites are not
   green at HEAD) as newly-visible failures; the spec should sequence the
   wiring with a fresh local run of both binaries so any latent red is
   attributed to the wiring, not to the fixture refactor
   (P-TrustworthySignal: attribute the signal before trusting it).
