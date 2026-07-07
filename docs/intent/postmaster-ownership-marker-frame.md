---
date: 2026-07-07
status: locked
intake: ./postmaster-ownership-marker.md
modulation: brownfield-extension
modulation_override: >
  Literal modulation rule (no project Frame doc → cold-start) overridden to
  brownfield-extension, per the sibling CI-flake precedent
  (./ci-flake-tier1-serialize-frame.md, ./ci-flake-tier2-shared-engine-frame.md):
  mnemra-core is a pre-/brief mature project, and this work extends an already-
  locked test-infra surface (scripts/ci-reap.sh + its seeded seam) with no
  architectural surface. Stage 2a elicitation skipped per modulation; inputs are
  the locked intake, the cited code (ci-reap.sh, ci_reap_baseline.rs, engine.rs,
  and the postgresql_embedded 0.20.4 crate source), the two sibling Frames, and
  the Stage-0 canon baseline.
---

# Frame: Postmaster ownership marker (concurrent ci-reap safety) — #2170

Mechanism evaluation for the concurrent-own-vs-other reap window that
`scripts/ci-reap.sh:27-38` documents and names #2170 to close. This is a
**Frame-only, brownfield-extension** run: #2170 closes at Frame-accept; the
implementation is a separate downstream task (intake Non-goal #5). The Frame's
job is therefore to **land one locked mechanism decision** the implementation
task builds against, not to enumerate open trade-offs.

## Operating constraints

Constraint-graph walk from the validated intent (reap own leaks only; never a
concurrent run's live postmaster). Edge types per `constraint-edges.md`;
most-specific applies. No `conflicts-with` edge fired. The locked-intake
constraints are treated as fixed and worked *within*.

**From the locked intake (fixed — not re-litigated):**

- **HC-1 — marker lives in the process command line or on the filesystem, never
  in engine in-memory state.** The reap runs post-hoc in bash (`pgrep` /
  `ps -o args`), possibly after the spawning Rust process has exited. `refines`
  the admissible-mechanism space: the marker MUST be observable from
  `-D <data_dir>` (the postmaster's only stable self-describing argv element) or
  from the filesystem. Any mechanism that needs the spawning process alive at
  reap time is inadmissible.
- **HC-2 — PPID is not a discriminator.** `pg_ctl` daemonizes every postmaster
  to PPID 1, live or leaked (falsified before build; `ci-reap.sh:57-65` "WHY NOT
  PPID"). Removes the whole PPID branch of the option space.
- **HC-3 — MUST NOT break the test seam.** `CI_REAP_PG_PATTERN` /
  `CI_REAP_BASELINE_FILE` are driven by `libs/mnemra-host/tests/ci_reap_baseline.rs`
  against the real script. `depends-on`: the chosen mechanism must keep those two
  seam variables and the three seeded assertions (own→reaped, baseline→alive,
  non-temp-root→alive) green.
- **HC-4 — refines #2119, does not replace it.** The baseline-PID-diff
  (condition (i)) + temp-root narrowing (condition (ii)) mechanism stays; the new
  marker refines condition (ii). The fails-closed capture sentinel (M3,
  `ci-reap.sh:174-177`) is preserved.

**From workspace canon (Stage-0 baseline):**

- **P-MinBlastRadius** (`architecture-principles.md:61`) — bounds how far the
  change reaches. The reap already reads `$TMPDIR`; a fix that changes *what
  `$TMPDIR` points to* rather than adding a new marker-plumbing surface, and that
  touches no production/Rust runtime path, is the minimum-blast-radius option.
  Governs the mechanism-1-vs-3 tie.
- **Simplicity** (value; anchors P-Defer, P-LockContract) — fewest moving parts
  that close the window; do not add a positive-marker plumbing channel if
  narrowing an existing check suffices.
- **P-TrustworthySignal** (`architecture-principles.md:156`) — this is a
  test-infra correctness change on the constantly-run `just ci` path; a
  mis-reap of a concurrent run's live postmaster is a false-red *in another run*
  (the faithful-instrument clause, cross-run). Its **discriminating-power /
  seeded-reintroduction** clause (2026-07-04 (c)) governs the proof: the fix is
  proven only when the seam test still fails on a seeded reintroduction of the
  window.
- **P-GuaranteeByMechanism** + **P-TrustworthySignal TS2** (silently-narrowed-
  coverage, 2026-07-02; workspace `feedback_silent_failure_structuralize_first_sighting`)
  — the load-bearing constraint on the recommendation: the chosen mechanism's
  correctness rests on an implicit crate behavior (data_dir derives from
  `env::temp_dir()`); a future change that breaks that coupling would fail
  **silent** (scoping defeated, seam still green). A silent-failure class
  structuralizes on first sighting — so the mechanism ships with a guard that
  fails loudly, not a prose note.
- **P-LockContract** (`architecture-principles.md:51`) — the two seam variables
  are the locked test-side contract; the mechanism varies condition (ii)'s
  *meaning*, not the seam.
- **P-0007 plugin-resource-limits** (`docs/src/adrs/P-0007-*.md`) — untouched
  (as tier-2 F-4 held); no production containment surface is in scope.

## Rationale chain

Validated intent (reap own-spawned leaks; never a concurrent `just ci`'s live
postmaster on this same codebase) → the residual window is that a concurrent
run's data_dir is under the **same global `$TMPDIR`** as this run's, so
condition (ii) cannot tell them apart (`ci-reap.sh:27-38`) → the discriminator
the window needs is a **start-time-scoped ownership marker** observable from the
`-D` argument (HC-1) → resolve which marker is *feasible* against the crate
source (the backend-feasibility question below) → among the feasible mechanisms,
P-MinBlastRadius + Simplicity break the tie toward the one that narrows the
*existing* `$TMPDIR` check rather than adding a new marker channel → lock it with
a P-GuaranteeByMechanism guard against its one silent-failure coupling, and a
P-TrustworthySignal seeded-reintroduction proof.

## Backend-feasibility resolution (the discriminator)

Resolved against the pinned crate source, `postgresql_embedded` **0.20.4**
(`Cargo.lock:2007-2010`), at
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/postgresql_embedded-0.20.4/`.
Both parts resolve **from source alone** — no runtime probe required, so this
Frame carries **no source-ambiguous open item** on the timing question.

**(a) Does the crate expose a native per-instance `data_dir` override?**
**YES.** `SettingsBuilder::data_dir<P: Into<PathBuf>>(mut self, dir: P) -> Self`
at **`src/settings.rs:347-351`**, backed by the public field
`Settings.data_dir: PathBuf` at **`src/settings.rs:47`** (also settable via a
`data_dir` URL query param, `src/settings.rs:194-196`). An explicit per-instance
data directory can be set at build time. → **Mechanism 1 is feasible.**

**(b) Is `temp_dir()` read live at engine init, or cached at `Settings` build?**
**Cached at `Settings` construction — but from the live process environment, and
NOT re-read afterward.** Trace:

- `Settings::new()` computes `data_dir` by calling `tempfile::tempdir()` (which
  reads `std::env::temp_dir()` = `$TMPDIR` internally) then `.keep()` to store the
  resulting concrete `PathBuf` — **`src/settings.rs:81`**. This is **not** the only
  temp read: `Settings::new()` calls `tempfile::tempdir()` **twice** — `password_file`
  at **`src/settings.rs:75`** and `data_dir` at **`:81`**, both under
  `env::temp_dir()`. The `data_dir` read is the one condition (ii) keys off; the
  `password_file` read means a per-run `$TMPDIR` relocates the password file too
  (folded into R-3).
- `PostgreSQL::new(settings)` takes `Settings` by value and mutates only
  `installation_dir` (version pinning); it **never touches `data_dir`** —
  **`src/postgresql.rs:67-86`**.
- `setup()` (`src/postgresql.rs:175-189`), `initialize()`
  (`.pgdata(&self.settings.data_dir)`, `src/postgresql.rs:266`), and `start()`
  (`.pgdata(&self.settings.data_dir)`, `src/postgresql.rs:333`) all consume the
  **stored** `data_dir` verbatim — none re-derive it from `temp_dir()`.
- `Drop` removes `self.settings.data_dir` when `temporary` — `src/postgresql.rs:534-535`.

**What this means for mechanism 3 (the trap, resolved).** The intake framed the
dichotomy as "live at engine init → mech 3 works / cached at construction → mech
3 dead," where "dead" rests on "setting `$TMPDIR` *afterward* is inert." The
literal source answer is the *cached-at-construction* branch — **but it does not
kill mechanism 3.** `Settings::new()` reads `env::temp_dir()` **live from the
process environment at construction time** (it is a plain `tempfile::tempdir()`
call, not a `static`/`LazyLock` global cache), so a `$TMPDIR` exported
process-wide **before the test process starts** is honored by every
`Settings::new()` in every test thread. mnemra-core's own construction path
confirms the precondition: `EmbeddedEngine::start()` builds settings with
`SettingsBuilder::new().version(..).timeout(..).temporary(true).build()` and
**no explicit `.data_dir()`** (`libs/mnemra-host/storage/postgres/engine.rs:478-486`),
so its data_dir *is* derived from `env::temp_dir()`. The "inert afterward"
concern applies only to setting `$TMPDIR` *after* a Settings is built (e.g.
programmatically mid-process) — which mechanism 3 does not do; it sets `$TMPDIR`
at the recipe level, before any Settings construction. → **Mechanism 3 is
feasible**, with a named silent-failure coupling (see the recommendation's
guard): if a future construction site adds an explicit `.data_dir()` outside
`$TMPDIR`, scoping breaks silently.

## The three candidate mechanisms (hypotheses tested)

### Candidate 1 — run-id-tagged explicit `data_dir` — FEASIBLE, not recommended

Tag each spawned postmaster's `-D <data_dir>` with this run's id via
`SettingsBuilder::data_dir(<per-run-root>/<unique>)`; the reap matches the run-id
substring in the `-D` argument.

- **Feasibility:** confirmed by (a) — the builder exists.
- **Cost:** requires a **Rust change at every engine-construction chokepoint**
  (`EmbeddedEngine::start()`, the tier-2 F-1 shared-engine fixture, any
  production-bootstrap path), threading a run-id from the environment into the
  path. It also **loses `tempfile::tempdir()`'s per-instance uniqueness** — a
  fixed `.data_dir()` collides when a run boots multiple engines — so it must
  itself become "run-id-tagged *root* + per-instance unique suffix," i.e. it
  re-implements a per-run temp root in Rust. And the reap needs a **new** run-id
  match condition (new seam surface) rather than reusing the `$TMPDIR` check.
- **Hypothesis verdict — the maintainer's #1 "tag the data_dir":** *Feasible but
  subsumed.* It reaches the same start-time scoping as candidate 3 with strictly
  more machinery (Rust + reaper + seam changes), so P-MinBlastRadius + Simplicity
  rank it below candidate 3. Confirms the maintainer's parenthetical hunch that #3
  subsumes #1.

### Candidate 2 — spawn-time record + compare — OVERTURNED (weak)

Record each spawned postmaster at spawn time; reap only recorded ones.

- **Post-hoc constraint (HC-1):** an in-memory record is inadmissible (the
  reaper runs in bash after the Rust process may have exited). The only
  admissible form is a **persisted per-run PID file** the bash reaper reads —
  i.e. a positive-ownership list, the inverse of today's baseline-diff.
- **Why it stays weak:** it needs the engine to expose each postmaster's PID
  (which `pg_ctl` daemonizes; the PID lives in `postmaster.pid` *inside* the
  data_dir) and to write it **reliably even on SIGKILL/interrupt** — the exact
  failure mode the reap exists for. A run killed before it records a PID leaves
  that leak unrecorded → not reaped (fails to clean own; safe for concurrency but
  defeats the JTBD's own-cleanup half). It leverages nothing the crate offers and
  adds a new write-path failure surface.
- **Hypothesis verdict — the maintainer's #2 "likely weak":** *Confirmed weak; overturned
  as a candidate.* Post-hoc attribution from a spawn-time record can't be made
  robust against the interrupt case without re-deriving a filesystem marker that
  candidate 3 gets for free.

### Candidate 3 — per-run `$TMPDIR` subdir — FEASIBLE, RECOMMENDED

The `ci` recipe creates a fresh per-run temp dir (`mktemp -d` under the system
temp root) and **exports `TMPDIR=<per-run dir>`** around the verify chain. Every
embedded engine the run spawns derives its data_dir under that per-run root
(via (b): `Settings::new()` → `tempfile::tempdir()` → `env::temp_dir()`, read
live from the inherited environment). The reaper reads the **same** `$TMPDIR`
(the recipe set it) and its existing condition (ii) now means "under *this run's*
temp root."

- **Feasibility:** confirmed by (b) — mnemra-core's construction path derives
  data_dir from `env::temp_dir()`, and the recipe sets `$TMPDIR` before the test
  process starts.
- **Cost:** **no Rust change; the reaper's core baseline-diff/temp-root logic is
  unchanged.** `ci-reap.sh` already reads `${TMPDIR:-/tmp}` (`:134`) — mechanism 3
  changes what `$TMPDIR` resolves to (per-run, not global). A concurrent run's
  data_dir is under **its** per-run `$TMPDIR` (a different `mktemp -d` dir), so it
  does not match this run's `$TMPDIR` prefix → condition (ii) fails → **not
  reaped.** The window closes by turning condition (ii) from a global-temp proxy
  into a true start-time-scoped ownership check. It carries **one small
  deliberate reaper addition** — the fails-closed scope sentinel (obligation 4) —
  which is a conscious trade against a raw zero-change reading of P-MinBlastRadius
  (see the recommendation).
- **Hypothesis verdict — the maintainer's #3 "likely subsumes #1 via `$TMPDIR`-scoping":**
  *Confirmed.* It achieves candidate 1's start-time scoping recipe-side, with the
  smallest surface.

## Recommended mechanism (the lock)

**LOCK: Candidate 3 — per-run `$TMPDIR` subdir**, with **two**
P-GuaranteeByMechanism guards against its two silent-failure couplings (a
construction-site explicit `.data_dir()`, and a `ci`-recipe scope regression) and
a P-TrustworthySignal seeded-reintroduction proof — plus **one accepted,
unguardable residual** (a fixed/shared `$TMPDIR`, closed only by a hard recipe
invariant; see R-1).

The implementation task (separate; Non-goal #5) builds four things — the
*mechanism* details (exact recipe wiring, guard placement, sentinel form, test
shape) are implement-time HOW, bounded here:

1. **Recipe wiring.** The `ci` justfile recipe creates a fresh per-run temp dir
   and exports `TMPDIR` to it for the verify chain (so both the spawning Rust
   test processes and the sourced `ci-reap.sh` inherit the same per-run value).
   Bounded: the per-run dir is under the system temp root so nothing else about
   the temp-root check changes; the export scope covers baseline capture, the
   verify chain, and the reap.
2. **Silent-failure guard (P-GuaranteeByMechanism / TS2).** A loud-failing check
   that the embedded engine's resolved `data_dir` actually lands under `$TMPDIR`
   — because correctness couples to the *implicit* crate behavior that data_dir
   derives from `env::temp_dir()` (verified for 0.20.4 above, but not contractual).
   If a future construction site adds an explicit `.data_dir()` outside `$TMPDIR`,
   scoping is silently defeated while the seam test stays green — a silent class,
   so it structuralizes on this first sighting rather than as a prose caveat.
   Placement (a fixture-chokepoint assertion in the tier-2 F-1 shared-engine
   module, or a startup debug-assert on the resolved data_dir prefix) is
   implement-time; the binding property is that the coupling fails loudly, not
   silently.
3. **Seam extension (P-TrustworthySignal seeded-reintroduction).** Add a seeded
   case to `ci_reap_baseline.rs`: run `ci_reap_own_postmasters` with
   `TMPDIR=<run-A dir>` present, and a marker whose data_dir is under a
   **different** per-run dir `<run-B dir>` (both under the system temp root),
   asserting it is **LEFT ALIVE** despite being absent from the baseline — the
   concurrent-other-run stand-in. (Structurally this rides the same "not under
   *this* `$TMPDIR`" path the existing non-temp-root case (c) already exercises;
   the new case pins the per-run distinction specifically.) **Non-simplification
   (load-bearing):** the reaper's `$TMPDIR` in this seeded case MUST be the narrow
   per-run `<run-A dir>`, **never** the system temp root — if an implementer
   "simplifies" it to the system temp root, `<run-B dir>` falls *under* it,
   condition (ii) passes, and the LEFT-ALIVE assertion stops firing for the right
   reason (a gate that gates nothing). The proof is complete only if reverting the
   per-run `$TMPDIR` export makes this new case fail.
4. **Fails-closed scope sentinel (P-GuaranteeByMechanism / TS2) — guards the
   *second* silent-failure coupling.** Mechanism 3 has two silent ways to revert
   to the pre-#2170 window: (i) a construction site sets an explicit `.data_dir()`
   outside `$TMPDIR` (obligation 2), and (ii) **the `ci` recipe stops exporting a
   per-run `$TMPDIR`** — a justfile edit or a runner that scrubs env — so
   data_dirs fall back under the global default, the reaper reads the same global
   default, and condition (ii) silently reverts to "under the global temp root"
   (concurrent runs reaped again). Obligation 2's guard ("data_dir under
   `$TMPDIR`") is **trivially true on the global default** and cannot see coupling
   (ii); obligation 3's seam test **sets `TMPDIR` in its own harness** and so
   cannot see a *recipe* regression. Coupling (ii) therefore needs its own
   loud-failing guard. Lock the **property**: the recipe sets a **value-carrying**
   per-run-scoping sentinel — it records the **concrete per-run `$TMPDIR` value**,
   written **atomically with the export** (the analogue of M3's `.captured`
   sentinel, but carrying a *value* rather than a bare flag); the reaper scopes
   condition (ii) off the **recorded** value and trusts it as an *ownership*
   narrowing only when that value is present **and is not equal to a known global
   default** — otherwise it **fails closed**, refusing to reap on the temp-root
   check alone, exactly as "no baseline → don't reap" already does
   (`ci-reap.sh:174-177`).

   **Why value-carrying, not a bare boolean (partial fix — be honest about the
   residual).** A bare boolean certifies "intent to scope," not "actually scoped
   per-run." Three ways the sentinel could be present yet scoping defeated, and how
   the value-carrying form handles each:
   - **Export dropped but sentinel kept** → data_dirs and reaper both fall back to
     the global default; a *bare* boolean is still set → concurrent reaped. The
     value-carrying form records the value **atomically with the export**, so a
     dropped export leaves an empty/absent recorded value → **fails closed.**
   - **`mktemp -d` fails → empty `$TMPDIR`** → both sides fall back to `/tmp`; a
     bare boolean is still set. The value-carrying form records an empty value →
     **fails closed** (and the recorded `/tmp` equals a known global default → the
     crude refusal catches this sub-case too).
   - **Fixed/shared `$TMPDIR`** (e.g. `export TMPDIR=/tmp/mnemra-ci` reused across
     runs) + sentinel → both concurrent runs land under the **same** recorded root
     → condition (ii) passes for a concurrent run → the #2170 window is silently
     back. This is **NOT runtime-closeable by any sentinel** — telling "fresh
     unique per-run path" from "fixed path reused" from a value alone would require
     re-deriving candidate 1's run-id tag or candidate 2's PID file. It is an
     **accepted residual + recipe invariant** (see R-1), not a guardable runtime
     property; the crude "equals a known global default" refusal catches the
     bare-`/tmp` sub-case but not `/tmp/mnemra-ci`.

   Design constraints (bounding the implementation): the refusal MUST key off the
   **recorded** sentinel value (the value the recipe deliberately wrote), never a
   live read of the reaper's ambient `$TMPDIR` — a "`$TMPDIR != /tmp`" heuristic
   read from the environment is fragile and would mis-handle the seam's
   ambient-temp cases; the "equals a known global default" test is **value
   equality**, not path containment, so a narrow per-run `mktemp -d` dir (a subdir
   of the system temp root, equal to no global default) still reaps. The seam
   harness **records the sentinel value** so the existing reap-path assertions
   (a)/(b)/(c) still exercise a reap — an **added (value-carrying) env var**, not a
   change to the two seam variables (`CI_REAP_PG_PATTERN` / `CI_REAP_BASELINE_FILE`)
   or the three assertions, so HC-3 holds. **Path-form (implementation note):** the
   implementation MUST confirm the recorded per-run `$TMPDIR` matches the
   postmaster's `-D data_dir` string form — canonicalize both sides, or record the
   same form the `-D` argument carries — so symlink/path-form divergence (e.g.
   macOS `/tmp` → `/private/tmp`) does not silently defeat own-leak reaping (SC-2).
   A tiny reaper change that fails **closed** beats zero change that fails **open**
   (P-TrustworthySignal + P-GuaranteeByMechanism over a raw P-MinBlastRadius
   optimization).

**How the lock satisfies each Success criterion:**

- *Concurrent run's post-baseline postmaster NOT reaped* → its data_dir is under
  **its** per-run `$TMPDIR`, not this run's; condition (ii)'s prefix test fails;
  left alive. Demonstrable through the existing `CI_REAP_PG_PATTERN` /
  `CI_REAP_BASELINE_FILE` seam plus the new seeded per-run case (obligation 3).
- *This run's own leaked postmasters ARE reaped* → their data_dir is under this
  run's `$TMPDIR` (condition (ii) satisfied) and absent from the baseline
  (condition (i)) → reaped, unchanged from #2119.
- *Fails closed on an uncaptured baseline* → the M3 capture sentinel
  (`ci-reap.sh:174-177`) is untouched; no baseline → no reap.
- *Marker observable by post-hoc bash, no live spawning process* → the marker is
  the data_dir path in the `-D` argument, parsed by the existing
  `_ci_reap_data_dir_for_pid` (`ci-reap.sh:123-129`) +
  `_ci_reap_path_under_temp_root` (`:133-141`); it depends on nothing but the
  postmaster's own command line.

**Test-seam preservation (HC-3):** the two seam variables
(`CI_REAP_PG_PATTERN` / `CI_REAP_BASELINE_FILE`) and all three existing
assertions (own→reaped, baseline→alive, non-temp-root→alive) stay green.
Mechanism 3's per-run `$TMPDIR` is orthogonal to those two variables. Obligation 4
**records one value-carrying env var** in the seam harness (the scope sentinel,
carrying the harness's narrow per-run temp dir — a subdir of the system temp root,
equal to no known global default) so the existing reap-path assertions still
exercise a reap under the new fails-closed reaper — a harness addition, not a
change to the two seam variables or the three assertions; and obligation 3 *adds*
a case. The seam contract is preserved; the seam *test file* gains a value-carrying
sentinel env and one case, both within the implementation task's scope (Success
criterion 1 explicitly anticipates extending the seam).

## Consultations

None (source-only synthesis under `modulation: brownfield-extension`; Stage 2a
elicitation skipped). The single question the intake carried forward to the Frame
— whether the embedded-Postgres crate exposes a native per-instance data-dir
option and reads the process temp dir live vs. cached — is resolved in-Frame
against the crate source (see **Backend-feasibility resolution**), with no
runtime-probe or operator consultation needed.

## Escalated decisions

None fired. Both candidates 1 and 3 are canon-admissible after the feasibility
resolution; the tie broke by canon anchoring (P-MinBlastRadius + Simplicity),
not by novel judgment — no pause-and-escalate trigger and no canon amendment. The
one novel-adjacent surface (coupling correctness to an implicit crate behavior)
is handled *within* existing canon (P-GuaranteeByMechanism / TS2), not by a new
principle.

## Routine decisions (batched)

- **Modulation override** (frontmatter): brownfield-extension per the sibling
  precedent. Within-principle.
- **No novel ADR.** #2170 introduces no architectural decision — it refines an
  existing test-infra mechanism (#2119) inside R-9/R-11 + P-TrustworthySignal;
  reversal re-opens this Frame, not project canon. The constraint-graph walk
  surfaced no novel or escalation-triggering edge.
- **ci-reap.sh core logic minimally changed.** The recommendation narrows
  condition (ii)'s *input* (per-run `$TMPDIR`) rather than adding a new ownership
  condition, and adds exactly **one** deliberate reaper check — the fails-closed,
  value-carrying scope sentinel (obligation 4), mirroring the fail-closed *shape*
  of the existing M3 baseline sentinel (carrying a value rather than a bare flag). Any
  comment update to the `ci-reap.sh` "residual window" note (`:27-38`) at
  implementation time is documentation, not logic.

## Risk profile (resolved)

No trust boundary, PII, auth, network, or multi-tenancy surface (intake
confirmed; unchanged by the now-known mechanism). Residual correctness risks:

Mechanism 3 has **two silent-failure couplings**, both fail *open* (revert to the
pre-#2170 window) and are the primary risks; each is mitigated **in-scope** by a
loud-failing guard, not carried residual:

- **R-1 (primary) — `ci`-recipe scope regression + the unguardable fixed-`$TMPDIR`
  residual.** Two sub-cases:
  - *Guardable (fails closed).* If the recipe stops exporting a per-run `$TMPDIR`
    (a justfile edit, or a runner that scrubs env), or `mktemp -d` fails leaving it
    empty, data_dirs and the reaper both fall back to the global temp root and
    condition (ii) would silently revert to a global proxy. Neither obligation 2
    (trivially true on the global default) nor obligation 3 (sets `TMPDIR` in its
    own harness) can see this — so it is guarded by **obligation 4's value-carrying
    scope sentinel**: the recorded value is empty/absent (or equals a known global
    default), so the reaper refuses to reap on the temp-root check. Tripwire: the
    recorded sentinel value is empty or equals a known global default on any run
    whose recipe did not scope `$TMPDIR`.
  - *Accepted residual — NOT runtime-guardable; a hard recipe invariant.* If the
    recipe exports a **fixed/shared** `$TMPDIR` (e.g. `TMPDIR=/tmp/mnemra-ci`)
    reused across runs, both concurrent runs land under the same recorded root,
    condition (ii) passes for a concurrent run, and the #2170 window silently
    reopens. No sentinel can distinguish "fresh unique per-run path" from "fixed
    path reused" from a value alone — that would require re-deriving candidate 1's
    run-id tag or candidate 2's PID file. This is a **permanent, unguardable
    property of Candidate 3 that the maintainer accepted at the Frame-exit gate.** It
    therefore binds the implementation as a **hard recipe invariant, not a runtime
    check**: the per-run `$TMPDIR` MUST be a **fresh, unique directory per run**
    (e.g. `mktemp -d` under the system temp root); a fixed or shared value silently
    reopens the #2170 window and is not runtime-detectable. The crude "equals a
    known global default" refusal catches the bare-`/tmp` sub-case only, never
    `/tmp/mnemra-ci`.
- **R-2 (primary) — construction-site explicit `data_dir`.** Scoping holds only
  while embedded-engine construction derives data_dir from `env::temp_dir()`.
  Verified true for 0.20.4 and mnemra-core's current call site
  (`engine.rs:478-486`), but not contractual. Guarded by **obligation 2's**
  loud-failing data_dir-under-`$TMPDIR` check. Tripwire: the guard fires on any
  construction site that sets an explicit non-`$TMPDIR` data_dir; a crate upgrade
  that changes the temp-dir derivation is caught by the same guard on the next
  run.
- **R-3 — broader temp blast radius.** A per-run `$TMPDIR` relocates *all* of the
  run's `tempfile` usage under the per-run root, not just PG data_dirs — including,
  within the embedded engine itself, **both** temp reads `Settings::new()` makes:
  the `data_dir` (`settings.rs:81`) **and** the `password_file` (`settings.rs:75`)
  move under the per-run root, not just the data_dir. This is the intended
  semantics of per-run temp isolation and is low-risk, but the implementation
  should confirm no gate depends on a stable global `$TMPDIR` path. Bounded, not
  blocking.

Standard code+security review applies at the Frame gate (intake concentrated
review here) and again at the implementation task's review; no security-mode
trigger.

## Intent self-report

**(a) JTBD as read:** I read the JTBD as — *give the failure/interrupt reap a
true start-time ownership discriminator so it kills only the postmasters this
`just ci` run spawned, never a concurrent run's live engine on the same
codebase, while keeping the #2119 baseline-diff + temp-root mechanism, the
fails-closed sentinel, and the seeded seam intact.*

**(b) Decisions that strain an enumerated Non-goal or Success criterion —
declared:**

1. **The recommendation adds two guards and a seam case that are technically
   *implementation* work, and #2170 is Frame-only (Non-goal #5).** I lock them as
   **bounded obligations on the implementation task**, not as work done here — the
   Frame names *what must hold* (two loud-failing coupling guards — the
   construction-site data_dir check and the fails-closed scope sentinel — plus the
   seeded per-run proof), the implementation decides *how*. This respects the
   Frame/implementation boundary while satisfying the
   silent-failure-first-sighting discipline; the alternative (leaving either
   coupling as a prose caveat) would violate P-GuaranteeByMechanism.
2. **The chosen marker is a per-run temp-root *scoping*, not a literal run-id
   string in the `-D` argument** — a reasonable reading of "ownership marker"
   (intake title / Success criterion 4) that some might expect to be an explicit
   id token. It is still a marker observable from the command line (the data_dir
   *prefix*), and it satisfies Success criterion 4 literally; I flag it because a
   reader anticipating a visible run-id in `ps` output will instead see a per-run
   temp path. Candidate 1 offers the literal-id reading at higher cost; the lock
   chose scoping per P-MinBlastRadius + Simplicity.
3. **Otherwise none** — no locked-intake Non-goal, Hard constraint, or Success
   criterion is contradicted; #2119's mechanism and the seam are preserved, not
   replaced.
