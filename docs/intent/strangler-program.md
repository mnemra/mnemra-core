# Intake: Strangler migration program — standing program mechanics

**Locked: 2026-07-07**

**Stakes:** high
**Date:** 2026-07-07
**Status:** locked (intake-exit gate confirmed by the maintainer 2026-07-07; Stage 1c review: security reviewer round 1, approve-with-conditions, zero blocker/high — 2 medium folded, 4 sub-threshold notes folded opportunistically, 0 dismissed. `spec_type` ratified **architecture** at the gate: no implementation ships from the program bundle itself — per-function bundles carry the code-destined specs.)
**Consumer:** agents
**Authorization:** task #1721 (entry condition met 2026-07-06 — spar pass + full decision walk, all ten questions maintainer-ratified).
**Primary substrate:** the strangler-migration decision walk (2026-07-06, ratified — ten maintainer-ratified program-mechanics answers; where it differs from earlier material, the walk supersedes), the brain-function census (2026-07-06, task #2166 — the complete work-list over the migration subject, consumed zero-trust), the workspace end-state synthesis (2026-06-19, not locked — the migration-subject architecture the walk amends), the locked coordination-wedge Frame (2026-07-06 — migration step 1's designed artifact, whose cutover obligations this program owns the template for).

```
spec_type: architecture   # ratified at intake-exit 2026-07-07
bootstrap: false
frame_relevant: true      # forced for architecture
```

Expected Stage 2 modulation: **cold-start** — first program-altitude Frame (a new architectural surface: program mechanics governing every subsequent cutover), with the 2a elicitation substantively pre-run: the ratified decision walk is the architectural-direction input record; 2a confirms residuals rather than re-eliciting.
Program position: **Frame-park** — this bundle runs Intake → Frame-exit and parks at the designed-Frame tier. The program Frame is the deliverable; per-function bundles carry their own intakes, Frames, and specs, referencing the program Frame for their cutover obligations.

## JTBD

The migration of the workspace's knowledge substrate (the `brain/` tree and the `puck.db` task database) into mnemra is a multi-year, function-by-function strangler — and it currently has no standing mechanics. The end-state synthesis's ordering rationale went stale on three axes at once (it was resolved before the coordination wedge existed, before memory was ruled core, and before the substrate's build-order realities were visible), and without a program artifact every per-function cutover would re-litigate sequencing, acceptance, rollback, and availability from scratch.

The job: scope the strangler as a **standing program** — one migration spine with a dependency-derived sequence rule, a reusable per-function cutover ritual, the availability ladder that hosting hardening gates against, and the complete function work-list with every judgment call ruled or routed — so that each per-function bundle executes its cutover as routine, small-batch mechanics referencing the program Frame, and program progress is measured against the right axis: brain-shrinkage, function by function, until the old substrate is gone.

## Non-goals

1. **Per-function designs.** The wedge (locked), ingestion/derived-context, memory, tasks, and every other function keep their own bundles. The program Frame owns mechanics only (ratified Q10: program-mechanics-only, small Frame).
2. **Optimization scheduling.** The dispatch-composer and spec-ops CLIs are optimizations riding the plugin system on their own value/pull — removed from the migration sequence entirely (ratified Q6). This program does not sequence them.
3. **Derived-context build design.** The derived layer (graph, embeddings, anchors) is built, not migrated — ELT over ETL, a re-runnable derive stage over persisted raw content (ratified Q5). That direction is ingestion-bundle intake, not program-Frame content.
4. **Memory compaction + render-bridge design.** Memory's cutover rides its dependencies (compaction design + the render bridge); the ratified Q9 direction (gate moves upstream to the memory write path; filesystem render is a regenerable, shrunken artifact) is carried as a constraint on that bundle, not designed here.
5. **Orchestration offline mode.** Deferred with a topology tripwire — fires when the service first runs off-box (tracked as its own seed task, #2167).
6. **The taste model.** Carved out to its own discussion (task #2164); nothing here pins what taste means beyond the existing provenance-tiered-subset note.
7. **Dual-run / parity machinery.** Changes are forward-facing; no shadow-read, parity choreography, or reconciliation tooling is designed at any point in the program (ratified Q4).
8. **Core/plugin vocabulary rename.** Core = residence/essentiality classification, never ordering (ratified Q2). The Frame pins that as one sentence; the established core/plugin ADR vocabulary is otherwise untouched — a rename would be its own walk.
9. **Hosting/availability hardening design.** The program Frame hands the availability ladder to the per-cutover hardening work (task #1056); the hardening mechanisms themselves are that work's to design.

## Success criteria

1. **One spine, stated.** The Frame states the single migration spine — function-by-function from the workspace perspective, the coordination wedge as migration step 1 — and pins the one-sentence rule that core/plugin classification governs residence, never sequence (ratified Q1/Q2). *Observable:* every step's position in the Frame's sequence derives from a named dependency, not from a classification label.
2. **Complete work-list, every function dispositioned.** Every census function (60 + 1 adjacent) appears with a disposition from a closed set (migrate-with-dependency / build-new / dissolve-into-substrate / retire-with-ritual / dissolves-with-substrate-at-decommission / out-of-scope — the fifth member covers self-referential tooling such as the substrate's own schema-migration machinery, which stays live until the old substrate is decommissioned and goes with it, neither migrating nor retiring early), and the census's flagged-count discrepancy (17 flagged classifications vs 11 consolidated judgment calls) is reconciled explicitly. *Observable:* a census→Frame reconciliation finds zero unaccounted functions and one authoritative flag enumeration.
3. **Every judgment call ruled or routed.** Each census flag gets a Frame ruling, or a named owner and firing point if it genuinely belongs to a later bundle. *Observable:* the flag enumeration maps one-to-one to rulings/routings; none is silently dropped.
4. **The cutover ritual, as a template.** Statement (explicit, dated) → migrate (with live-row verification counts) → backup (zip snapshot, kept until confidence, then deleted) → remove (the old copy — one home, no stale shadow), instantiable per function (ratified Q4). *Observable:* the locked wedge Frame's cutover-obligations section already satisfies the template as written — the back-check passes without amending the wedge.
5. **The availability ladder.** The ratified matrix (session-start vs mid-session × read vs write; coordination-write failure = immediate stop; no local write queue ever) rendered as the program ladder that per-cutover hosting hardening (task #1056) gates against — the availability bar rising as each function reaches sole-home (ratified Q3). *Observable:* each rung names the bar that must hold before that function's sole-home statement.
6. **Component-host forcing function pinned.** The Frame names which migration step first requires the plugin-invocation Bucket A machinery (per the census: tasks is the likely first plugin-shaped step), making it the honest forcing function for that build (ratified Q6). *Observable:* the dependency edge is recorded on the sequence.
7. **Backup preconditions positioned.** The daily-backup requirement (task #2165) and the census's zero-version-history finding on the task database are positioned as program preconditions with explicit ordering relative to the first sole-home cutover. *Observable:* the Frame states what must exist before any removal step may execute.
8. **Structural findings dispositioned.** The census's four structural findings (task-database zero git history; six CLI-less tables; two parallel session-cost aggregation paths; three direct-SQL bypasses of the CLI-sole-writer discipline) each get a fix-now vs fix-at-cutover ruling. *Observable:* four of four carry a ruling.
9. **Small-increments operating principle stated.** Small, low-blast-radius increments as the program's standing operating mode — coherent with small-batch flips, drain-then-flip, and forward-only-with-backups (ratified Q10/Q7). *Observable:* stated as a program constraint bundles inherit.
10. **Progress axis defined.** Program progress is measured against brain-shrinkage (functions at sole-home in mnemra), with the tracking mechanism named. *Observable:* the Frame states how a function's sole-home status is recorded, so "how far along is the strangler" has one answer.

## Hard constraints

1. **Program-mechanics-only, SMALL** (ratified Q10). Per-function design content is structurally excluded; if a section starts designing a function, it belongs in that function's bundle.
2. **The ten ratified walk answers are locked program mechanics** — off-limits to re-open at this Frame. The Frame renders them at Frame precision; it does not re-decide them.
3. **The locked coordination-wedge Frame is migration step 1's designed artifact.** The program Frame composes with its cutover-obligations section (which instantiates the ritual for coordination) — it never re-designs or contradicts the wedge's locked directions.
4. **Forward-only.** Issues found post-cutover are fixed forward in mnemra; no local write queue, ever; no dual-run (ratified Q3/Q4).
5. **The ritual's removal step is standing authorization** (ratified Q4 refinement). Statement → migrate → backup → remove: removal-after-backup is maintainer-sanctioned as part of the ritual; the zip snapshot is the archive. Future sessions do not re-litigate the delete step against the workspace's never-hard-delete conventions — the ritual is the authorization.
6. **Dependency-derived ordering only** (ratified Q2). No step is sequenced by classification, want, or stakeholder preference; each position traces to a dependency.
7. **No dual-authority window at any flip** (ratified Q7): small-batch flips, drain-then-flip for in-flight items, scheduled operator-only-live statements. The program Frame owns the flip-plan template; per-pass flip plans belong to the cutover statements.
8. **Standing project canon applies.** Docs-only bundle in this repo; the agent-first workflow's BOM/audit-chain protocol governs the stage locks.

## Evidence

- **The ratified decision walk (2026-07-06):** ten program-mechanics questions walked one-by-one and ratified — the spar's sharpest overall finding being that the synthesis's ordering rationale was stale on three axes at once (resolved before the wedge, before memory was ruled core, before build-order realities were visible). The program artifact exists to stop that class of drift from recurring per-cutover.
- **The brain-function census (2026-07-06, task #2166):** 60 functions + 1 adjacent enumerated; 13 census-discovered beyond the seed list (notably the intent/plans/specs pipeline storage — load-bearing for the verify pipeline's audit chain); four structural findings that are live discipline gaps, not just inventory.
- **The task database has zero version history** (census structural finding 1): it is excluded from the knowledge base's local git; the markdown corpus has local history, the database has none — no remote shadow either. Sharpens the backup requirement (#2165) beyond the walk's original finding.
- **Memory cap pressure at near-idle** (ratified Q9 evidence): the flat-file memory index hits its byte caps constantly at near-idle activity — the flat-file system fails at N=1 dogfood scale. Raises memory's practical priority on the work-list; the census carries this signal on the adjacent item.
- **Use case (end-to-end walk of one cutover under the program mechanics):** a function's bundle locks its design referencing the program Frame → the operator schedules the flip (operator-only-live) → statement (explicit, dated) → data migrates with live-row verification counts → zip snapshot of the old copy → the old copy is removed → the availability bar rises for that function → the hardening task's gate check runs against the ladder rung. Every step in that walk is a program-Frame mechanic; none is per-function design.

## Consumer of resulting work

**Agents (primary):** every per-function bundle's Frame and spec references the program Frame for its cutover obligations — the locked wedge Frame already does this by anticipation (its cutover-obligations section names the program frame as owner of the flip plans). The operator lane executes cutover statements against the ritual template. **Downstream tasks:** per-cutover hosting hardening (#1056) gates against the availability ladder; the daily-backup requirement (#2165) takes its position from the Frame's precondition ordering.

## Risk profile

**Data-integrity risk — flagged (required Stage-2 constraint; security-mode review fires at Frame where the mechanism is known):**

- **Removal is part of the ritual — standing, un-gated, agent-executed.** The program's standing mechanics authorize deleting the old copy of each migrated function without a per-instance human gate, so the mechanical preconditions carry the full integrity weight. The load-bearing risk surfaces: backup adequacy *before* removal — existence **and demonstrated restorability**, not presence alone — plus what "verified migrated" means via live-row counts, and the ordering of preconditions relative to the first sole-home cutover. The Frame renders these preconditions as **mechanically fail-closed gates, not convention checks** (a removal step whose precondition check can silently pass is a convention, and the program's neighboring evidence base is four instances of convention decay): the precondition check itself fails closed.
- **The migration subject has no shadow.** The knowledge substrate is filesystem + local git only; the task database has no version history at all. Between statement and backup-establishment, the program is operating on unreplicated data.
- **Split-brain at flips.** Two authorities over one function during a transition window is split-brain by construction; the ritual + drain-then-flip + operator-only-live rules are the control. The Frame's threat pass should pressure-test the no-dual-authority guarantee at the template level.
- **Availability as correctness.** Fail-closed write semantics are load-bearing (proceeding on a failed coordination or content write defeats the program); the availability matrix is a constraint, not a wish.
- Attribute priority for Frame threat modeling: **integrity** (no data loss at removal, no split-brain at flips) and **availability** (fail-closed, the ladder) are load-bearing; confidentiality is not materially changed at single-team dogfood scope — hosted/multi-team confidentiality concerns ride their own bundles (the auth bundle; the hardening task), not this program's mechanics.
- The four surfaces above — **removal preconditions as fail-closed mechanism (incl. backup restorability) · the no-shadow window · split-brain at flips · fail-closed availability** — are the **Frame threat-modeling handoff set**, with the attribute priority as its lens.

## Design forks carried to Frame (deliberately unresolved here)

1. **The sequence re-cut itself** — the actual dependency-derived order after step 1 (the wedge), including where memory rides given its dependency chain (compaction + render bridge) and its by-evidence priority (Q9 cap pressure). The intake carries the inputs; the Frame cuts the order.
2. **The census judgment calls** — the consolidated flag list (scope-violations table liveness; metrics core-vs-plugin, explicitly left open with evidence; projects/repos/content registry promotion questions; the undocumented write paths; the core-adjacent canon directory; the config and dashboard exclusions; the instance-claim residence split). These are exactly Frame dispositions, per success criterion 3.
3. **Flag-count reconciliation** (17 vs 11): candidate mapping identified at intake — 17 = every census row carrying any flag-type marker (11 explicitly FLAGGED rows + 4 dormant-table confirm-before-exclude rows + the unclear-write-path row + the residence-split row); 11 = the consolidated judgment-call list (which merges the two session-metrics rows into one call, and omits the four dormant-confirms and the row that already carries an explicit non-migration ruling). The Frame enumerates the union authoritatively — reconciling against the census's **table cells**, not its headline counts (the headline's own arithmetic does not reconcile) — and dispositions each member.
4. **Progress-axis rendering** — how sole-home status is recorded per function (a statement register? the program plan's tracker? a per-function status field), so brain-shrinkage is queryable rather than narrated.
5. **Statement residence** — where the dated cutover statements live (the declaration is the ritual's first step; its durable home and shape are Frame decisions).
6. **Dead-weight disposition class** — the census's vestigial items (pre-existing legacy files, frozen snapshot tables, orphaned skills) need the ritual applied as cleanup separate from the strangler queue; the Frame decides whether that is a program lane (a "retire" class on the work-list) or explicitly out-of-program.

## Consultations

- _none at intake — the substrate is maintainer-ratified decision records (2026-07-05 through 2026-07-06) plus the census deliverable; the Stage 1c review pass is the security reviewer._

## Dismissed review flags

- _none — Stage 1c round 1 reported two medium findings (closed disposition set missing the self-referential-terminal class; removal preconditions as fail-closed mechanism), both folded. One reviewer-suppressed nit (JTBD capability list edging solution-shaped) sits under the wedge intake's W7 precedent: at bundle altitude a JTBD naming its sub-capabilities is acceptable when the surrounding framing is need-shaped._

## Open items resolved at the intake-exit gate (2026-07-07)

1. `spec_type` — ratified **architecture** (consumer = agents; success criteria are program-mechanics artifacts; no implementation ships from this bundle — the per-function bundles carry the code-destined specs).
2. **Frame-park** — confirmed: the program parks at the designed-Frame tier; no Stage-3 spec for this bundle.
3. **Register entry** — confirmed: lands at Frame-merge time per the register-promotion convention (the wedge's precedent).
