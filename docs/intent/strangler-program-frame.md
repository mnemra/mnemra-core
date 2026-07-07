---
title: "Frame: strangler migration program — program mechanics"
summary: "Cold-start, program-altitude Frame for the multi-year brain→mnemra strangler: one migration spine with a dependency-derived sequence rule (core classification governs residence, never order), a reusable per-function cutover ritual (statement → migrate + live-row verify → backup incl. demonstrated restorability → remove) whose preconditions are mechanically fail-closed gates and whose removal step carries standing authorization, the availability ladder per-cutover hosting hardening gates against, and the complete work-list — all 60 + 1-adjacent census functions dispositioned from a closed six-member set, the flag-union of 17 rows reconciled against the census's literal cells and each ruled or routed, and the four structural findings each ruled fix-now vs fix-at-cutover. Metrics ruled plugin at the Frame-exit gate (P-0018 D-BOUNDARY — proposal adopted); the in-repo statements register locked as the progress axis (brain-shrinkage, queryable). Parks at the designed-Frame tier — per-function bundles carry their own intakes, Frames, and specs and reference this Frame for their cutover obligations; no Stage-3 spec, no program ADR slot."
primary-audience: agent
modulation: cold-start
status: locked
date: 2026-07-07
intake: docs/intent/strangler-program.md
intake-blob: 6d58cd29b7e80e595e4492ab396975cbdf7f1de0
spec-type: architecture
---

# Frame — strangler migration program: program mechanics

**Date:** 2026-07-07 · **Status:** locked (Frame-exit gate accepted by the maintainer 2026-07-07; metrics ruled **plugin** at the gate — proposal adopted; the standing removal authorization stands as a locked Frame direction, no governance ADR minted) · **Altitude:** migration-program mechanics (standing program, designed tier) · **Program position:** Frame-park — this bundle runs Intake → Frame-exit and parks at the designed-Frame tier. Unlike a feature cluster whose Stage-3 spec is a later pickup, this program's Frame-park is **terminal**: the per-function bundles carry the code-destined specs, and the program never authors one. This Frame is therefore the durable designed-tier artifact of record for the migration program's mechanics.

> **Modulation note.** This is a `cold-start` Frame: the migration program is a new architectural surface (the first program-altitude Frame — standing mechanics governing every subsequent per-function cutover), so the full Stage 2a elicitation ran before this synthesis. The 2a substrate was substantively pre-run: the ratified strangler-migration decision walk (2026-07-06) is the architectural-direction input record, and 2a confirmed residuals rather than re-eliciting. The validated intake ([`docs/intent/strangler-program.md`](strangler-program.md), locked 2026-07-07, blob `6d58cd29b7e80e595e4492ab396975cbdf7f1de0`) and the Stage 2a input record (§2) are the locked inputs; the ten ratified walk answers and the three 2a ratifications are maintainer-ratified and are **not re-opened here**. This Frame is the closed world for the per-function bundles' cutover obligations: a bundle's cutover mechanics come from here, not from re-litigating sequencing, acceptance, rollback, or availability per function.

## 1. Purpose / context

The migration of the workspace knowledge substrate (the markdown knowledge base and the task/coordination database) into mnemra is a multi-year, function-by-function strangler. It had no standing mechanics: the end-state synthesis's ordering rationale went stale on three axes at once — it was resolved before migration step 1 (the coordination cluster) existed, before memory was ruled core, and before the substrate's build-order realities were visible — so without a program artifact every per-function cutover would re-litigate sequencing, acceptance, rollback, and availability from scratch. The neighbouring evidence base is four instances of convention decay (a handoff consumed with no trace, an addressed item no instance could archive, a live claim adopted after a context reset, an archive convention that decayed despite exact commands): file conventions have no mechanism by which they hold (the constraint the workspace canon names generally as guarantee-by-mechanism).

- **What this designs.** The standing mechanics of the migration program: (a) the single migration spine and its dependency-derived sequence rule; (b) the reusable per-function cutover ritual, with mechanically fail-closed preconditions and a standing removal authorization; (c) the availability ladder that per-cutover hosting hardening gates against; (d) the complete function work-list, every census function dispositioned from a closed set with the judgment calls ruled or routed and the structural findings dispositioned; (e) the program constraints every bundle inherits; (f) the progress axis and the durable statements register that makes brain-shrinkage queryable. It designs **program mechanics only** — deliberately small.
- **What this does not design.** No per-function design. Migration step 1 (the coordination cluster, locked) and the ingestion/derived-context, memory, tasks, and every other function keep their own bundles. This Frame owns mechanics; per-function bundles own function internals (memory compaction, ingestion ELT, coordination design), and this Frame stops at the boundary — if a section starts designing a function, it belongs in that function's bundle. (Intake Non-goals 1–9, all carried.)
- **Intake.** [`docs/intent/strangler-program.md`](strangler-program.md) — locked 2026-07-07, high stakes, `spec_type: architecture` (no implementation ships from this bundle; per-function bundles carry the code-destined specs), Stage 1c security review round 1 approve-with-conditions, zero blocker/high.
- **Design substrate.** The ratified strangler-migration decision walk (2026-07-06 — ten maintainer-ratified program-mechanics answers; where it differs from earlier material, the walk supersedes); the brain-function census (2026-07-06, task #2166 — the complete work-list, consumed zero-trust); the workspace end-state synthesis (2026-06-19, not locked — the migration-subject architecture the walk amends); the locked migration step 1 Frame ([`docs/intent/coordination-wedge-frame.md`](coordination-wedge-frame.md), 2026-07-06 — whose §8 cutover obligations this program owns the template for, composed with, never re-designed).

## 2. Stage 2a — Elicitation

Elicitation was substantively pre-run: the ratified strangler-migration decision walk (2026-07-06) is the architectural-direction input record. This section embeds the walk as the elicitation substrate and records the three 2a ratifications confirmed this run (2026-07-07). All are maintainer-ratified and off-limits to re-open; §§4–9 lock each at Frame precision.

### 2.1 The ratified decision walk (2026-07-06 — maintainer-ratified program mechanics, not re-decided)

Ten questions were walked one-by-one and ratified. Rendered at Frame precision (the walk record is the substrate; this is the render):

1. **One spine, not two (Q1).** Migration is function-by-function from the workspace perspective. Coordination is a workspace function whose current substrate is files; migration step 1 strangles that substrate — it is the first migration step, not a separate adoption track. Extension: the file-based instance-communication surfaces (handoffs, instance claims, session-carrier, the carrier protocol) all move; claims/messaging are replaced by mnemra primitives, the session-carrier partially dissolves, and the zero-trust consumer discipline survives the substrate change. Extension 2: claims split by duration, not residence — atomic in-system writes are storage transactions; held intent (inside or outside mnemra) is an advisory lease. (Migration step 1's Frame resolved this fork; §7 records it closed.)
2. **Core governs residence, not ordering (Q2).** Core/plugin is a functionality-and-residence classification (core-essential vs plugin-optional); migration order is dependency-derived. The end-state synthesis's "core first, then plugins" phrasing was never a sequence rule. This Frame pins one sentence to that effect (§4); a full vocabulary rename would ripple the core/plugin ADR set and is its own walk (Non-goal 8). Carve-out: a separate discussion on taste (task #2164), off this Frame.
3. **Availability matrix (Q3).** Session-start + service down = no work (as designed). Mid-session + down = the current unit may finish read-only against the materialized snapshot; all writes fail closed — no local write queue, ever (a queue is split-brain by construction); the session then stops. Coordination-write failure = immediate stop. Offline mode is a planned choice only, never an automatic crash fallback. This matrix is the availability ladder the program hands the per-cutover hosting hardening (task #1056) — §6.
4. **Forward-only + backups; the cutover ritual (Q4).** Changes are forward-facing — issues found post-cutover are fixed forward in mnemra (dogfooding is the acceptance test); no dual-run, no shadow-read, no parity machinery at any point (Non-goal 7). Safety net: a snapshot of the old copy at each cutover (kept until confidence, then deleted) plus a standing daily backup of mnemra (task #2165). The per-function ritual: **statement → migrate + live-row verify → backup → remove**. Removal-after-backup is maintainer-sanctioned as part of the ritual; the snapshot is the archive; future sessions do not re-litigate the delete step against the never-hard-delete convention — the ritual is the standing authorization.
5. **Derived context is built, not migrated (Q5).** Core data migrates via the ritual; the derived layer (graph, embeddings, anchors) is computed over already-migrated raw content on its own bundle timeline — it never blocks the migration queue. ELT over ETL: load/persist raw content first (with provenance), derive as a separate re-runnable stage. That direction is the ingestion bundle's intake, not this Frame's content (Non-goal 3).
6. **Optimizations are removed from the sequence (Q6).** The composer and spec-ops tools strangle nothing; they are new capability on mnemra's plugin system, scheduled on their own pull — removed from the migration sequence entirely (Non-goal 2). Relocated dependency: the plugin-invocation Bucket A machinery (P-0013) is owed before the first plugin-shaped **migration** step — the census names it (tasks), which becomes the honest forcing function for that build.
7. **No dual-authority window (Q7).** Small-batch flips; drain-then-flip for in-flight items; scheduled operator-only-live statements. The program owns the flip-plan template; per-pass flip plans belong to the cutover statements.
8. **Offline mode deferred with a topology tripwire (Q8).** An orchestration offline mode is needed eventually — but not until the service first runs off-box, where remote access becomes a failure path. Local-first dogfooding: local instance down = temporary, restartable from backup; risk acknowledged, accepted, low. Seed task #2167 carries the firing condition.
9. **Memory's governance moves upstream (Q9).** mnemra removes the need for most of the flat-file memory index and its byte caps; the end-state render is a minimal bootstrap stub, most memory served on demand via retrieval. The governance gate moves to mnemra's memory write path; the filesystem render is a regenerable, shrunken build artifact. Evidence: the flat-file index hits its caps at near-idle activity — it fails at N=1 dogfood scale, which raises memory's by-evidence priority on the work-list. Memory's cutover rides its dependencies (compaction design + the render bridge), carried as a constraint on the memory bundle, not designed here (Non-goal 4).
10. **Program-mechanics-only, small Frame (Q10).** The walk record + the census are the primary intake; per-function design stays in the bundles. Ratified alongside: **small incremental improvements** as the program's (and the workspace's) standing operating mode — coherent with small-batch flips, the trunk-based ritual, and forward-only-with-backups. Extends the incremental-slicing operating mode beyond greenfield.

**The walk's sharpest finding:** the synthesis's sequence was resolved before migration step 1, before memory was ruled core, and before build-order realities were visible — coherent shape, ordering rationale stale on three axes at once. This program exists to stop that class of drift from recurring per-cutover.

### 2.2 The three 2a ratifications (2026-07-07 — this run, off-limits to re-open)

1. **Ordering — pure dependency cut.** The Architect derives the order from census dependencies alone; memory slots where its dependency chain allows. Evidence-priority (memory's cap pressure) is noted as a pull, not a sequence override — consistent with ratified Q2. (§4.)
2. **Metrics core-vs-plugin — Architect proposes.** 2b weighs the census evidence under the plugin-default rule (default plugin, promote deliberately) and proposes; the maintainer rules at Frame-exit with the full work-list in view. The proposal is expected, flagged as proposal-for-gate, not silently locked. (§7.3, item 2; §16.)
3. **Statement register — in-repo statements register.** The dated cutover statements live in a docs file in this repo alongside the program Frame ([`docs/intent/strangler-program-statements.md`](strangler-program-statements.md)); statements are program artifacts that survive the adopter workspace's own dissolution and land via the same docs PR lane. This Frame locks the register's *shape*; the file is created at first cutover (§9, §15).

## 3. Constraint-graph walk — operating constraints

Walked from the intent through the workspace constraint graph (values → principles → edges → project ADRs), most-specific-wins. Edges bearing on the program's mechanics:

| Edge / constraint | What it binds here |
|---|---|
| **P-GuaranteeByMechanism → Quality, Observability** (specializes) | The program's load-bearing constraint. A removal-precondition check that can silently pass is a convention, and the program's own evidence base is four instances of convention decay (intake Evidence). Every removal precondition (migration verified, backup present-and-restorable) is a mechanism that fails **closed** on an unconfirmed check — never a convention a future session must recall. The ritual's removal authorization is safe *only because* its gates are mechanical (§5). |
| **P-VerifyInheritedState → Honesty, Reversibility** (specializes) | Writer + migration ≠ capturing: a migration's "done" is a producer's assertion, and an assertion is not evidence. The verify step counts **live rows** at source and destination, and for a zero-corpus function verifies drain-to-empty — the consumer re-verifies against the source of truth before the removal step may fire (§5). |
| **P-Defer → Simplicity, Reversibility** (specializes; DF1 firing-mechanism discipline) | Every deferral in this Frame names its firing mechanism (§11): offline mode (topology tripwire, task #2167 — self-announcing on off-box deploy), taste model (parked, task #2164 — a named discussion, not a trip-wire), the metrics-vocabulary rename (self-announcing — only if a walk is opened), memory's cutover (rides its dependency chain — self-announcing when compaction + render-bridge land). A named-but-unfireable trip-wire is parked, labelled as such. |
| **P-MinBlastRadius → Maintainability** (specializes) | One function per cutover; small-batch flips; the flip unit, the disposition unit, and the blast-radius unit coincide. A cutover reaches exactly one function's substrate; a defect found post-cutover is fixed forward in that function's home, not rippled across the queue (MB5 — no lock-step multi-function flips). |
| **P-LeastAuthority → Security** (specializes) | The removal authorization is scoped to the ritual's recovery job, not standing delete authority: removal fires only after the fail-closed gates pass, only at an operator-only-live statement (no other instance mid-session), never as a side effect of migration. Authority is scoped to the flip, and drops when it closes. |
| **P-SecurityLayered → Security** (specializes; design-time layer) | The intake flags the removal surface as a trust boundary; §10 is the threat-model pass over the four-item handoff set. Fail-closed is the control-integrity posture: a removal whose precondition cannot be verified is a stop, not a warning. The design-time threat model fires here because the mechanism (fail-closed gates) is now known. |
| **P-LockContract ⇄ P-PreserveDecisionSpace** (conflicts-with, when-to-lock axis) | Applied per the edge's discriminator, not escalated. Intrinsic to the program's identity and locked now: the sequence rule (core≠order), the cutover ritual and its fail-closed preconditions, the standing removal authorization, the availability ladder's rung contract, the closed disposition set, the progress axis, the no-dual-authority guarantee. Separable and left to the per-function bundles: each function's design, its live-row-count particulars, its TTL/backup calibration, its flip plan. A lock is scoped to its assumptions: migration step 1's Frame re-derived the residence/duration fork the walk left open — this Frame renders that as closed, not re-opened (§7.3, #57). |
| **Reversibility (value) → forward or backward** | The program's mitigation path is *forward* (fix in mnemra) plus *restore-from-backup*, named before any removal commits. Reversibility lowers the cost of a cutover error; it never lowers the bar to remove — the fail-closed gates are the bar, and "the backup exists" is not authorization to skip a gate. |
| **Dogfooding (value) → forcing function** | Forward-only is the dogfooding posture: real fleet usage is the acceptance test, not parity choreography. The verify half stays lightweight (live-row counts + real usage), not a shadow-read reconciliation the workspace would have to build and maintain. |
| **Decomposition (value) — vertical-slices clause** | Small, low-blast-radius increments as the program operating mode (SC9): the strangler runs as a sequence of single-function vertical slices, each a complete cutover, rather than a big-bang layered migration. Extends the incremental-slicing operating mode beyond greenfield (walk Q10). |
| **P-0002 (core/plugin partition)** — most-specific on residence | Core = residence/essentiality classification via the verb-on-content criterion, never ordering. The census applied it as the residence rubric; this Frame pins core≠order as one sentence (§4) and otherwise leaves the vocabulary untouched (Non-goal 8). |
| **P-0018 (core entity manifest)** — most-specific on what is core | `projects` and `actors` are core entities; `tags`/`attachments` complete the four hard-FK targets. **Workflow primitives — tasks, dispatches, skill-runs, specs, comments — are explicitly NOT core (D-BOUNDARY, plugin-introduced).** This is the decisive anchor for the metrics proposal (§7.3, item 2): dispatch/skill-run measurement is workflow-primitive data, named in the negative space. |
| **P-0013 (plugin invocation model)** — most-specific on the Bucket A forcing function | Bucket A (host-fn bodies, component instantiation, call_tool routing) is owed before the first plugin-shaped migration step. The census names tasks as that step; the sequence records the edge (§4). |
| **P-0017 / P-0010 (storage)** — most-specific on placement | Migrated functions land under the engine-agnostic `Storage` trait (P-0010) with the four-shape content taxonomy or the operational-table precedent (P-0017); additive-migration discipline applies. Placement is per-function-bundle detail; the program only names that every migrated function inherits it. |
| **G-0003 (merge governance)** — most-specific on the docs PR lane | The program Frame and the statements register are docs-only artifacts landing via the reviewed docs PR lane; the migration program mints no new autonomous merge authority — the removal authorization is a data-integrity authorization on the substrate, distinct from merge authority. |

## 4. The spine + the dependency-derived sequence re-cut

### 4.1 The one spine (SC1)

There is **one migration spine**: function-by-function from the workspace perspective, brain shrinking until the old substrate is gone (walk Q1). Migration step 1 is the coordination cluster (locked, [`docs/intent/coordination-wedge-frame.md`](coordination-wedge-frame.md)) — it strangles the file coordination substrate. The content-function order follows.

**The one-sentence rule (pinned, SC1):** *core/plugin classification governs where a function lives (residence/essentiality), never when it migrates (sequence); every step's position is derived from a named dependency, not from a classification label* (walk Q1/Q2; anchors P-0002 — residence criterion; the plugin-default rule — core is a high bar, not an ordering key). Consequence, stated so it is not a silent rule-break: memory is **core** yet migrates **late** (§4.2, step 5), because its dependency chain (render bridge + compaction) gates it — "core first" as a sequence rule is dead; dependency order is the only sequencer.

### 4.2 The dependency-derived sequence (SC1 observable — every position traces to a named dependency)

The spine is a **dependency partial order** (a DAG), not a strict line: where two functions share no dependency edge they may cutover in either order or in parallel, at the operator's small-batch cadence. The named edges fix each position:

| Step | Function(s) | Position derived from | Residence |
|---|---|---|---|
| **1** | Coordination cluster (migration step 1, locked) | Ratified as step 1 (usage-defining) **and** zero-corpus by design — no upstream data dependency; it strangles the file substrate every later cutover's statements and messaging ride on | core (host subsystem; builds the `actors` core entity) |
| **2** | Project definitions + raw content/artifacts (the ingestion/context substrate) | `projects` is the fan-in root (five census tables hard-FK to it) — it must exist before any dependent; raw artifacts are the ELT load foundation the derived layer computes over (walk Q5), so the content substrate must exist before any content-derived function | core (`projects` per P-0018; content substrate is the ingestion bundle's build) |
| **3** | Tasks | Hard-FK-depends on `projects` (+ `repos`), so it follows step 2; **first plugin-shaped migration step → trips P-0013 Bucket A**, the honest forcing function for the component-host build (walk Q6; SC6) | plugin |
| **4** | Metrics cluster (dispatch lifecycle, events, session/skill-run measurement, reporting) | Hard-FK-depends on `tasks` (dispatch/event rows key off tasks) → follows step 3 | plugin (ruled at Frame-exit 2026-07-07 — §7.3 item 2) |
| **5** | Memory (native memory → mnemra) | Rides its dependency chain: the render bridge + compaction design must land first (walk Q2/Q9). By-evidence highest priority (cap pressure at N=1) — a **pull that advances it within its chain**, never a jump ahead of the render bridge (2a ratification 1). A different source (native memory, not the brain substrate) | core |
| **n** | Remaining plugin verticals (job-search, latent functionality) | Independent verticals — no cross-function FK; slot at the operator's cadence wherever their own dependencies (their FK anchors already migrated) allow | plugin |

**The Bucket A forcing function, pinned (SC6):** tasks (step 3) is the first plugin-shaped migration step; the plugin-invocation Bucket A machinery (P-0013) is owed before it. The dependency edge — *tasks-cutover depends-on Bucket A build* — is recorded on the sequence, so the honest cost of the first plugin migration is the component-host build, not a task-table copy (walk Q6).

Excluded from the sequence entirely (Non-goal 2): the composer and spec-ops optimizations — they strangle nothing and ride their own pull (walk Q6). Excluded from the migration queue (built, not sequenced, Non-goal 3): the derived-context layer — computed over migrated raw content on the ingestion bundle's timeline (walk Q5).

## 5. The cutover ritual template

The ritual is the reusable per-function cutover mechanic (walk Q4; SC4). It instantiates per function; the per-pass particulars (live-row counts, backup location, flip schedule) belong to that function's cutover statement, not here.

**The four steps:**

1. **Statement.** An explicit, dated cutover statement is written to the statements register (§9) before anything moves, naming the function, the target availability rung (§6), and the flip plan (small-batch, drain-then-flip for in-flight items, operator-only-live). The statement is the first step because authority transfers at the statement: from the statement forward, mnemra is the authority for that function and the old substrate is frozen (no new writes), guaranteeing no dual-authority window (Hard constraint 5; walk Q7).
2. **Migrate + live-row verify.** The function's data migrates to mnemra; migration is **verified by live-row count**, not by the writer's assertion (P-VerifyInheritedState). The verification is one of two shapes: for a data-bearing function, source live-row count equals destination live-row count; for a zero-corpus / drain-not-migrate function, the migrate step is a **drain** and the verification is drain-to-empty (the old substrate is provably empty, in-flight items processed before the flip). Either shape produces a mechanical yes/no.
3. **Backup.** A snapshot of the old copy is taken and its **restorability is demonstrated** — restore-rehearsal or integrity verification, not mere presence (intake risk profile: existence *and* demonstrated restorability). The snapshot is the archive; it is kept until confidence, then deleted. **Snapshot-deletion has a mechanical floor:** the snapshot MUST NOT be deleted before the standing mnemra daily backup (task #2165) has *demonstrably captured this function's data* — a machine-checkable, fail-closed condition (unconfirmed capture ⇒ deletion blocked), enforced by the same *mechanize the removal gate* obligation (§8) that gates the old-copy removal. The floor makes an early snapshot-delete *unreachable*; "kept until confidence" is the maintainer's timing *above* that floor, never a licence to drop below it. Without it, deleting the last local archive before the daily backup has captured the function would leave live mnemra the sole copy on a shadowless subject (F1). That standing daily backup is the ongoing restore path once the old copy is gone.
4. **Remove.** The old copy is removed — one home, no stale shadow. Removal fires **only** when the preconditions below all pass.

**Preconditions as mechanically fail-closed gates (SC7; the intake's data-integrity requirement):** before the remove step may execute, each precondition is checked by a mechanism that **fails closed** — an unconfirmed or unverifiable check is treated as *not satisfied*, and removal is blocked. A precondition check that could silently pass is a convention, and the program's evidence base is four instances of convention decay (P-GuaranteeByMechanism):

- **G-verify:** the step-2 live-row verification passed (counts match, or drain-to-empty confirmed). Unconfirmed ⇒ blocked.
- **G-backup:** the step-3 backup exists **and** its restorability was demonstrated. Present-but-unrehearsed ⇒ blocked.
- **G-standing-backup:** the program precondition (the mnemra daily backup path, §6) is established — required before the **first** sole-home removal, and standing thereafter. Absent ⇒ blocked.

**Standing removal authorization (Hard constraint 5; walk Q4 refinement):** removal-after-backup is maintainer-sanctioned as part of the ritual. The ritual **is** the standing authorization for the delete — future sessions do not re-litigate the delete step against the workspace's never-hard-delete convention, and the authorization is safe precisely because it is conditioned on the fail-closed gates above — structurally enforced, not merely checked (§8, *mechanize the removal gate*) — and the operator-only-live statement (P-LeastAuthority — scoped to the ritual, not a standing delete power). The delete is agent-executable without a per-instance human gate; the gates carry the integrity weight the human gate would otherwise carry.

**Back-check against migration step 1 (SC4 observable — the template is satisfied by the locked step-1 Frame *as written*, without amending it):** migration step 1's cutover-obligations section ([`coordination-wedge-frame.md`](coordination-wedge-frame.md) §8) instantiates this ritual for coordination as *statement → drain → backup (archive snapshot of the file substrate) → remove (files + conventions retired)*, with the migrate step vacuous by rule (zero-corpus; drain-then-flip; no dual-authority window). Running the back-check: step-1's statement/drain/backup/remove maps one-to-one onto steps 1–4; its drain-to-empty is the zero-corpus shape of G-verify; its archive snapshot is step 3; its no-dual-authority window is the statement-freezes-authority guarantee. One term does not map tightly: the intake-sharpened "demonstrated restorability" (step 3) postdates migration step 1's §8, whose plain "archive snapshot of the file substrate" predates the sharpening. This does not break the back-check — restorability binds *data-bearing* cutovers, and for a zero-corpus file-*drain* un-archiving files is inherent, so demonstrated restorability is near-vacuous (no migrated corpus exists whose restore could fail). The sharpening lands on the first data-bearing cutover; migration step 1 inherits it at its own Stage-3 spec only if a data-bearing residue appears. **The back-check passes; migration step 1's §8 satisfies this template as written and needs no amendment.**

## 6. The availability ladder

The ratified availability matrix (walk Q3; intake Hard constraint 4) is the program **ladder** the per-cutover hosting hardening (task #1056) gates against. Each rung is the availability bar that must hold **before** a function's sole-home statement — the bar rises as each function reaches sole-home (SC5).

**The rung contract (the matrix, per function reaching sole-home):**

| Condition | Bar the rung must hold |
|---|---|
| Session start + service down | **No work** for that function — no fallback substrate exists after its cutover (the old copy is gone). |
| Mid-session + service down | The current unit may finish **read-only** against the materialized snapshot; **all writes for that function fail closed**; **no local write queue, ever** (a queue is split-brain by construction); the session then stops. |
| Any write failure for that function | **Immediate, observable stop** — a structured error the client surfaces as a stop, never empty-success or silent proceed. |

**How the ladder rises.** While mnemra is *additive* for a function (the old substrate still holds it), mnemra being down is tolerable — the old copy serves. The **moment a function reaches sole-home** (its old copy removed), "mnemra unreachable" becomes a real outage for that function, and its rung's bar goes live. So the hardening task does not harden everything at once: it gates per cutover, raising the availability bar to the new rung before the sole-home statement that makes that rung load-bearing (walk Q3; end-state synthesis: hosting hardening is gated on strangler progress).

**Backup preconditions positioned (SC7).** The **first** sole-home cutover is the point where unreplicated-data risk becomes real (the migration subject has no shadow — the knowledge base is filesystem + local git, the task database has zero version history, census structural finding 1). Therefore the standing mnemra daily backup (task #2165) and a resolution of the zero-version-history gap are **program preconditions**, positioned **before the first sole-home removal** and standing thereafter (G-standing-backup, §5). The Frame states the ordering: **no removal step may execute until the standing backup path exists.**

## 7. The work-list

The complete function work-list. Every census function (60 + 1 adjacent = 61) receives a disposition from a closed set (§7.1, §7.2); the flag-carrying rows are enumerated authoritatively and each ruled or routed (§7.3); the four structural findings are each ruled fix-now vs fix-at-cutover (§7.4).

### 7.1 The closed disposition set (six members)

Every function lands in exactly one:

1. **migrate-with-dependency** — data migrates via the ritual, positioned by its dependency edge.
2. **build-new** — the function is rebuilt in mnemra (its derived/computed nature means there is nothing in the old substrate to migrate; raw inputs may migrate as content).
3. **dissolve-into-substrate** — the function is reborn as an mnemra primitive; the old file substrate is drained-and-retired, not migrated (the migrate step is vacuous).
4. **retire-with-ritual** — vestigial/dead weight; the backup-then-remove ritual applies as cleanup, but the function is *not* in the migration queue.
5. **dissolves-with-substrate-at-decommission** — self-referential tooling (the substrate's own schema-migration machinery, mechanical git artifacts) that stays live until the old substrate is decommissioned and goes with it; neither migrates nor retires early.
6. **out-of-scope** — not a workspace/agent function this program migrates; a named alternative home is recorded.

*(Routing is orthogonal: a function may be routed to a later bundle that applies one of these dispositions at its cutover — SC3's "ruled or routed." A routed item names its owner and firing point.)*

### 7.2 The full work-list (all 60 + 1 — SC2 observable: zero unaccounted)

Census numbering preserved for traceability. `†` marks a flag-carrying row (enumerated in §7.3). Disposition = the §7.1 member.

| # | Function (terse) | Disposition | Dependency / routing |
|---|---|---|---|
| 1 | Task CRUD + lifecycle | migrate-with-dependency | step 3; first plugin-shaped step, trips Bucket A; dep projects+repos |
| 2 | Activity log | migrate-with-dependency | step 4; dep tasks |
| 3† | Scope-violation tracking | retire-with-ritual | superseded by dispatch_metrics columns; fold + retire at metrics cutover (fix-at-cutover) |
| 4 | Dispatch lifecycle record | migrate-with-dependency | step 4; dep tasks; plugin (proposed) |
| 5 | Dispatch event logging | migrate-with-dependency | step 4; dep #4 |
| 6 | Dispatch metrics capture | migrate-with-dependency | step 4; dep #4 |
| 7 | Experience-sample verdict | migrate-with-dependency | step 4; dep #4/#5 |
| 8 | Dispatch lifecycle automation (reap/finalize) | migrate-with-dependency | step 4; automation atop the CLI, dep #4 |
| 9 | Per-dispatch scope/verify drift check | migrate-with-dependency | step 4; dep #4 + spec sidecars (#44) |
| 10 | Metrics reporting (read-only aggregates) | migrate-with-dependency | step 4; dep #4/#11; pure read surface |
| 11† | Session cost/token aggregation (CLI path) | migrate-with-dependency | step 4; canonical session-agg path (lean, §7.4-F3) |
| 12† | Session cost/token aggregation (direct-SQL path) | retire-with-ritual | retires when the canonical path is chosen (fix-at-cutover, §7.4-F3) |
| 13 | Session event narration | migrate-with-dependency | step 4; dep #14 |
| 14 | Skill-run measurement | migrate-with-dependency | step 4; independent within cluster |
| 15 | Skill-run retro | migrate-with-dependency | step 4; dep #14 |
| 16 | Knowledge-extraction capture | migrate-with-dependency | step 4; dep #14; consumer of memory (#59*) |
| 17† | Project registry | migrate-with-dependency **+ core** | step 2; fan-in root (5 FKs); **promote to core** (P-0018 core entity) |
| 18† | Repo registry | migrate-with-dependency | step 2; FK target for tasks; stays plugin (not in P-0018 set); design-clean at cutover |
| 19† | Content/artifact registry | build-new | step 2; becomes the mnemra ingestion/content substrate (core); raw artifacts migrate as content; routed to ingestion bundle |
| 20 | Legacy content (Cowork-era) | retire-with-ritual | superseded by #19 + filesystem |
| 21 | Schema/data migration tooling | dissolves-with-substrate-at-decommission | self-referential; retires with the task database |
| 22† | Dependency-approval tracking | migrate-with-dependency | plugin; **write-path gap fix-now** (§7.4-F2) — identify the undocumented writer before migration |
| 23† | Contacts registry | retire-with-ritual | dead (1 row); the *concept* is a plugin in mnemra (build-new), not this dead table |
| 24† | Generic key-value context | retire-with-ritual | dead (2 rows), dormant since seed |
| 25† | Generic typed relationships | retire-with-ritual | dead (3 rows); the *concept* is the mnemra edge model (build-new), not this table |
| 26† | Tags + taggings | retire-with-ritual | dead brain tables; the `tags` core entity is build-new in mnemra (P-0018), not migrated from these |
| 27 | Job listing ingestion + scoring | migrate-with-dependency | independent vertical; routed to job-search bundle |
| 28 | Job application tracking | migrate-with-dependency | independent vertical |
| 29† | Job search run logging | routed (migrate-or-retire) | owner: job-search bundle; firing point: its cutover confirms live/dead (§7.3) |
| 30 | Job listings backup snapshot | retire-with-ritual | one-off backup snapshot |
| 31† | Cognitive/architecture canon (`about/`) | migrate-with-dependency | step 2; migrates as raw content; early candidate (small, static, high-value) |
| 32 | Daily logs | migrate-with-dependency | content + generation fn; dep #2/#1 |
| 33 | Weekly rollups | migrate-with-dependency | content + generation fn; dep #32 |
| 34 | Per-project status docs | migrate-with-dependency | step 2; highest-value artifacts-as-content |
| 35 | Research briefs + papers | migrate-with-dependency | content |
| 36 | Prompt-engineering docs | migrate-with-dependency | content |
| 37† | Editor/terminal config | out-of-scope | personal dev-environment config, not a workspace/agent function; confirmed excluded |
| 38 | Governance/ADR corpus | migrate-with-dependency | content; the citation-resolution register carries its own never-delete constraint |
| 39 | Blog drafts / published content | migrate-with-dependency | content |
| 40 | Spec/plan templates | migrate-with-dependency | content; supports the intake/Frame/spec pipeline |
| 41 | Completed-work archive | migrate-with-dependency | content, bulk + deferred priority; some contents are retire-candidates handled at bulk |
| 42 | Intake + Frame documents | migrate-with-dependency | content; dep #40 |
| 43 | Committed plans | migrate-with-dependency | content; committed tier |
| 44 | Locked specs + BOM sidecars | migrate-with-dependency | content; **load-bearing for the verify pipeline** (audit-chain); dep #42 |
| 45 | Misc reference docs | migrate-with-dependency | content |
| 46 | Anthropic API/pricing delta tracking | migrate-with-dependency | content; actively growing |
| 47 | Security advisory tracking | migrate-with-dependency | content |
| 48 | Model-behavior eval corpus | migrate-with-dependency | content; static research artifact, low priority |
| 49 | Schema-migration history + rollback SQL | dissolves-with-substrate-at-decommission | self-referential; retires with the task database |
| 50 | Git-worktree parking directory | dissolves-with-substrate-at-decommission | mechanical git artifact, not a function |
| 51 | Vestigial `.claude/skills` | retire-with-ritual | dead, orphaned (reference non-existent dirs) |
| 52 | Root-level legacy files | retire-with-ritual | Cowork-era, superseded by the task database + `about/` |
| 53 | Legacy `memory/` directory | retire-with-ritual | near-empty legacy; **do not conflate with #59*** |
| 54† | Rule-provenance register | out-of-scope | rides the harness→plugin path, not brain→mnemra; explicit non-migration per its own header; named alternative home = the installable plugin |
| 55† | Static workspace dashboard | retire-with-ritual | stale since workspace creation, no regenerator; confirm-then-retire |
| 56 | Handoff inbox | dissolve-into-substrate | step 1; reborn as messages |
| 57† | Live-instance claim registry | dissolve-into-substrate | step 1; reborn as leases; **residence/duration fork closed by step 1** (duration line) |
| 58 | Session-boundary carrier (stash) | dissolve-into-substrate | step 1; file substrate dissolves, shrinks toward a pointer to live tracker |
| 59 | Carrier protocol (`/handoff`) | dissolve-into-substrate | step 1; zero-trust consumer invariant absorbed |
| 60 | Git main-merge lease | dissolve-into-substrate | step 1; reborn as a lease |
| 59* | Native memory (`MEMORY.md` + topics) | migrate-with-dependency | step 5; native→mnemra (not brain→mnemra); **core**; rides render bridge + compaction; by-evidence highest, dependency-gated; routed to memory bundle |

**Reconciliation (SC2 observable — zero unaccounted, one authoritative enumeration):** 61 rows, each with exactly one disposition (or a named routing that resolves to one at a later bundle). Class tallies: migrate-with-dependency (37), dissolve-into-substrate (5: #56–60), retire-with-ritual (12: #3, #12, #20, #23, #24, #25, #26, #30, #51, #52, #53, #55), dissolves-with-substrate-at-decommission (3: #21, #49, #50), build-new (1: #19), out-of-scope (2: #37, #54), routed (1: #29). 37 + 5 + 12 + 3 + 1 + 2 + 1 = 61.

### 7.3 The flag-union enumeration + the 17-vs-11 reconciliation (SC2, SC3)

The census carries a flagged-count discrepancy (17 vs 11). Reconciled against the census's **literal Classification cells**, not its headline arithmetic (which, the intake notes, does not self-reconcile):

**Cell-verified correction (surfaced explicitly, per the brief's zero-trust requirement):** the intake's candidate mapping reads "17 = 11 explicitly FLAGGED rows + 4 dormant + unclear-write-path + residence-split." Against the literal cells, only **10** rows carry a `FLAGGED` marker in the Classification column (#3, #11, #12, #17, #18, #19, #22, #31, #37, #54). The 11th row the intake lumped into "FLAGGED" is the dashboard (#55), whose cell reads *"CENSUS-DISCOVERED, likely stale"* — a distinct **confirm-live/dead** flag-type, not `FLAGGED`. The union of 17 still holds; the sub-attribution is corrected below. This is the off-by-one the intake anticipated ("the headline's own arithmetic does not reconcile").

**The authoritative flag-union — 17 rows, each ruled or routed:**

| Flag-type | Rows | Count | Ruling / routing (per row, §7.2) |
|---|---|---|---|
| Literal `FLAGGED` cell | #3, #11, #12, #17, #18, #19, #22, #31, #37, #54 | 10 | ruled in §7.2 (retire/migrate/build-new/out-of-scope as marked) |
| Likely-stale / confirm-live-dead | #55 | 1 | ruled: retire-with-ritual (confirm-then-retire) |
| Dormant confirm-before-exclude | #23, #24, #25, #26 | 4 | ruled: retire-with-ritual (dead tables; concepts are build-new in mnemra) |
| Unclear-write-path | #29 | 1 | routed: job-search bundle confirms live/dead at its cutover |
| Residence-split | #57 | 1 | ruled: **closed by migration step 1** (duration line, not residence) |
| **Union** | | **17** | none silently dropped |

**The 17 → 11 consolidation (the census's "consolidated judgment-call list"):** 17 minus {#54, which carries its *own* explicit non-migration ruling and needs no fresh judgment} minus {the 4 dormant rows #23–26, batched as one confirm-before-exclude call} minus 1 {#11 and #12 merge into one metrics session-aggregation call} = **11**: {#3, metrics (#11+#12), #17, #18, #19, #22, #29, #31, #37, #55, #57}. 17 − 1 − 4 − 1 = 11. The flag enumeration maps one-to-one to rulings/routings; none is silently dropped (SC3).

**The two rulings that carry program weight (rendered here, mechanics routed to bundles):**

1. **`projects` → core (#17).** Ruled core, not defaulted plugin. Anchor: it is the `projects` core entity in the P-0018 manifest, and it is the fan-in root (five census tables hard-FK to it) — the demonstrated need the plugin-default rule requires for deliberate promotion. *(Anchors: P-0018 D-ENT; the plugin-default rule's promote-on-demonstrated-need clause.)*

2. **Metrics → plugin (proposal for the Frame-exit gate — 2a ratification 2).** The Architect proposes; the maintainer rules at Frame-exit. **Proposal: the metrics cluster (#4–16) is plugin.** *Decision-and-rationale:* dispatch and skill-run measurement is workflow-primitive data, and workflow primitives are named in the negative space of the core entity manifest — tasks, dispatches, skill-runs are explicitly NOT core (P-0018 D-BOUNDARY). The census's promotion signals do not overturn this: the wide fan-out is fan-out *among other plugins* (the reporting/automation/knowledge-extraction consumers), which is a consumer shape, not a foundation shape — core promotion is for foundational substrate every plugin references (fan-*in*), and nothing references metrics as a foundation (fan-*out*). The internal duplication (two session-aggregation paths) is a structural finding to resolve (§7.4-F3), not a core signal. *(Anchors: P-0018 D-BOUNDARY — the decisive anchor; the plugin-default rule — default plugin, promote only on demonstrated foundational need; fan-in-vs-fan-out as the promotion discriminator.)* **RULED at the Frame-exit gate (2026-07-07): plugin** — the maintainer adopted the proposal; the disposition is locked and the work-list carries no open item.

### 7.4 The four structural findings (SC8 — each ruled fix-now vs fix-at-cutover)

| # | Finding | Ruling |
|---|---|---|
| **F1** | The task database has **zero version history** (excluded from git; no remote shadow) | **fix-now.** The standing mnemra daily backup (task #2165) and a resolution of the no-history gap are **program preconditions**, positioned before the first sole-home removal and standing thereafter (§5 G-standing-backup, §6). This is the load-bearing finding — it sets the removal gate's third precondition. |
| **F2** | **Six CLI-less tables** — `approved_packages` (live, undocumented writer) + five dormant | **split.** The undocumented `approved_packages` writer is **fix-now** (identify and name the write mechanism before #22 migrates — the sole-writer discipline has a live exception, cheap to find now). The five **dormant** tables (`contacts`, `context`, `relationships`, `tags`, `taggings` — five tables occupying four census rows #23–26, since row #26 covers both `tags` and `taggings`) are **fix-at-cutover** — retire-with-ritual as cleanup, confirmed-before-excluded. |
| **F3** | **Two parallel session-aggregation paths** (CLI-written `puck_sessions` #11; direct-SQL `session_metrics` #12), unreconciled | **fix-at-cutover.** Resolve at the metrics cutover — pick one canonical path before either migrates; porting both perpetuates the split. Non-binding lean (a tiebreaker, not a lock; the mechanism is the metrics bundle's): the CLI-written path (#11) honours the sole-writer discipline, so it is the natural canonical, with the direct-SQL path (#12) retiring. |
| **F4** | **Three direct-SQL bypasses** of the CLI-sole-writer discipline (two read-only generators; one read+write session script) | **fix-at-cutover.** These scripts retire or convert when their functions migrate (daily/weekly generation → the content bundle; the write bypass → folds with F3's `session_metrics` retirement). The **inventory is captured now** (this Frame) so each cutover is not a blind "what does this do" investigation; the write bypass (the session script's INSERT) is the one to watch, tied to F3. |

## 8. Program constraints (the standing mode bundles inherit)

Stated once here; every per-function bundle inherits them (SC9):

- **Small-increments operating mode.** Small, low-blast-radius increments are the program's standing operating mode (walk Q10): one function per cutover, small-batch flips, no big-bang. Coheres with the incremental-slicing operating mode, extended beyond greenfield. *(Anchor: Decomposition value's vertical-slices clause; P-MinBlastRadius.)*
- **Forward-only.** Issues found post-cutover are fixed forward in mnemra; no dual-run, no shadow-read, no parity/reconciliation machinery at any point (walk Q4; Non-goal 7). *(Anchor: Dogfooding value; Reversibility value — the mitigation path is forward-fix + restore-from-backup.)*
- **No dual-authority window.** Exactly one authority per function at any instant; authority transfers at the statement; the old substrate is frozen between statement and removal; in-flight state is drained, never migrated (walk Q7; Hard constraint 5). *(Anchor: P-GuaranteeByMechanism — the drain precondition and the statement-freeze are mechanisms, not conventions.)*
- **Drain-then-flip.** In-flight coordination/work state is processed to empty before its flip; never migrate in-flight state (walk Q7).
- **Operator-only-live flips.** Only the operator instance is live at a cutover statement; flips are scheduled, not rolling (walk Q7). *(Anchor: P-LeastAuthority — the removal authority is scoped to the operator-only-live window.)*
- **Mechanize the removal gate.** Each per-function removal is *structurally* gated on the three fail-closed checks (§5 G-verify / G-backup / G-standing-backup), not merely conditioned on them: the mechanism shape is an atomic check-and-delete, or a shared removal tool that refuses to execute absent machine-readable pass-evidence for all three gates. A checklist step ("verified — yes") does not satisfy fail-closed — a check a future session must *recall to run* is a convention, and the program's evidence base is four convention-decay instances. Every bundle inherits *mechanize the gate* as an obligation: a per-function spec may not discharge fail-closed with a prose or checklist precondition, and the same obligation gates snapshot-deletion (§5 step 3). *(Anchor: P-GuaranteeByMechanism — the removal gate is a mechanism, not a policy conditioned on checks; P-SecurityLayered — control-integrity fail-closed.)*

## 9. Progress axis + the statements register shape

**Progress axis (SC10):** program progress is measured against **brain-shrinkage** — the count of functions at sole-home in mnemra, against the work-list total (§7.2). "How far along is the strangler" has one answer: *the number of work-list functions whose cutover statement records a completed removal.*

**The statements register (2a ratification 3 — shape locked here, file created at first cutover):** the dated cutover statements live in an in-repo docs file ([`docs/intent/strangler-program-statements.md`](strangler-program-statements.md)), landing via the docs PR lane, surviving the adopter workspace's own dissolution (statements are program artifacts, not workspace-private state). Each entry is one cutover statement, carrying:

- **function** — the work-list # + name (§7.2), so the register joins to the work-list.
- **statement date** — when authority transferred (ritual step 1).
- **target availability rung** — the §6 rung the function reaches at sole-home.
- **migrate-verify result** — the step-2 live-row counts (or drain-to-empty confirmation) — the G-verify evidence.
- **backup reference + restorability-demonstrated** — the step-3 archive location and the restore-rehearsal outcome — the G-backup evidence.
- **removal date** — when the old copy was removed (blank until removal fires) — the sole-home marker.

**Queryability (SC10 observable):** a function's sole-home status is *recorded* as the presence of a removal date on its register entry, and *queried* by counting entries with a removal date against the work-list total. Brain-shrinkage is queryable rather than narrated. The register's per-entry fields are also the fail-closed gates' evidence **record** (§5) — the same entry proves the gates fired. The register **records** that the gates fired; it is not itself the gate. An evidence record is written by the same agent that runs the delete, so it cannot *prevent* an ungated removal — the enforcement lives in the removal mechanism (§8, *mechanize the removal gate*), which structurally refuses the delete absent the pass-evidence. The register is the audit surface; the mechanism is the gate.

## 10. Threat-model pass (integrity + availability lens)

The intake hands a four-surface threat-modeling set with the attribute priority **integrity** (no data loss at removal, no split-brain at flips) and **availability** (fail-closed, the ladder) load-bearing; confidentiality is not materially changed at single-team dogfood scope (hosted/multi-team concerns ride their own bundles). Each surface gets a resolution:

**TS-1 — Removal preconditions as fail-closed mechanism, incl. backup restorability (resolved).** The load-bearing integrity surface: removal is standing, un-gated by a per-instance human, agent-executed. The control is the three fail-closed gates (§5 G-verify / G-backup / G-standing-backup), each failing *closed* on an unconfirmed check — a removal whose preconditions cannot be verified is blocked, not warned. Backup adequacy is *existence + demonstrated restorability*, not presence alone (the intake's explicit sharpening). The gates carry the integrity weight the absent human gate would carry, and they are mechanisms, not conventions — the program's evidence base is four convention-decay instances, so a convention check here would decay identically. *(Anchors: P-GuaranteeByMechanism; P-VerifyInheritedState — live-row verify; P-SecurityLayered fail-closed; intake risk profile.)*

**TS-2 — The no-shadow window (resolved with stated residual).** The migration subject has no shadow: the knowledge base is filesystem + local git, the task database has zero version history (F1). Between statement and backup-establishment the program operates on unreplicated data. Control: the ritual takes the backup (step 3, with restorability demonstrated) *before* the remove step (step 4), and the standing mnemra daily backup (F1 fix-now) is a program precondition before the first sole-home removal — so the unreplicated window is bounded to *within a single cutover's statement→backup span*, never spanning a removal. **A second no-shadow sub-window sits at snapshot-deletion:** after the old copy is removed, the per-cutover archive snapshot is the only local copy until the daily backup first captures the function; deleting it early would reopen the no-shadow window with no local fallback (live mnemra alone). Control: the §5 step-3 mechanical floor — the snapshot delete is *unreachable* until the standing backup has demonstrably captured the function (fail-closed, enforced by *mechanize the removal gate*, §8) — so the archive cannot predecease its replacement. Residual, stated: within that span, a failure loses at most the in-flight delta of one function's cutover (bounded by small-batch — one function, drained-then-flipped); accepted at dogfood scope, its re-open condition is the off-box topology tripwire (§11). *(Anchors: intake risk profile — no-shadow; P-MinBlastRadius — one function per cutover bounds the exposure.)*

**TS-3 — Split-brain at flips (resolved).** Two authorities over one function during a transition is split-brain by construction. Controls, all mechanical: authority transfers at the statement (the old substrate is frozen from that instant — no dual-authority window, §8); drain-then-flip guarantees no in-flight item straddles the boundary; operator-only-live guarantees no other instance is mid-session at the statement. The old substrate's freeze is enforced by writer-repointing at the flip plus operator-only-live plus drain — *no code path targets the old substrate post-flip* — not by §6: §6's fail-closed governs **mnemra** writes when mnemra is down, so it backstops a mnemra-side race (a write against a deposed authority fails closed and stops the session), not writes to the old substrate. Two authorities cannot both proceed. *(Anchors: P-GuaranteeByMechanism; walk Q7; the availability contract.)*

**TS-4 — Fail-closed availability (resolved).** Proceeding on a failed write defeats the program (an unmigrated write against a sole-home function is data loss; a proceed-unclaimed at a coordination flip is split-brain). The control is the availability ladder's rung contract (§6): all writes fail closed, no local queue ever, coordination-write failure is an immediate observable stop. The matrix is a constraint, not a wish; the hosting hardening (task #1056) gates each rung against it before the sole-home statement that makes it load-bearing. *(Anchors: intake HC4; walk Q3; P-SecurityLayered fail-closed; P-TrustworthySignal — a write failure renders as a structured stop, never empty-success.)*

## 11. Deferrals + carried tripwires (DF1 firing mechanisms)

Every deferral names its firing mechanism; "self-announcing" = the need cannot arise without someone explicitly asking, so no detector is required; "parked" = human-noticed via a named cadence, not a mechanical trip-wire (P-Defer/DF1).

| Deferred / parked item | Decision content when fired | Firing mechanism |
|---|---|---|
| Orchestration offline mode (Non-goal 5; walk Q8) | Offline-mode topology + remote-access failure handling | **Trip-wire:** the service first runs off-box (a multi-node/disconnected-operation deployment). Tracked as seed task #2167; the topology change is the firing event. |
| The taste model (Non-goal 6; walk Q2 carve-out) | What "taste" means beyond the provenance-tiered-subset note | **Parked** (not trip-wired): a named discussion, task #2164. Cadence = the maintainer opens the discussion; nothing mechanical fires it. Labelled parked, not deferred. |
| Core/plugin vocabulary rename (Non-goal 8; walk Q2) | Renaming residence/essentiality vocabulary across the core/plugin ADR set | **Self-announcing:** only if a rename walk is deliberately opened; the one-sentence core≠order pin (§4.1) is the cheap fix that removes the pressure until then. |
| Memory's cutover (Non-goal 4; walk Q9; §4.2 step 5) | Memory's migration design (rides compaction + the render bridge) | **Self-announcing:** the memory bundle fires when its dependency chain (compaction design + render bridge) lands; the program only positions it (step 5), the bundle designs it. |
| Derived-context build (Non-goal 3; walk Q5) | Graph/embeddings/anchors over migrated raw content | **Self-announcing:** the ingestion bundle designs it on its own timeline; it never blocks the migration queue (built, not migrated). |
| Hosting/availability hardening design (Non-goal 9; task #1056) | The hardening mechanisms per rung | **Trip-wire:** each sole-home cutover raises the bar to a new rung; the hardening task gates against §6 per cutover. The program hands the ladder; the task designs the mechanisms. |

## 12. Open ADR slots

**Decision: the program mints no ADR slot. (Split decision recorded either way, per the intake's Design fork on ADR minting.)**

*Rationale (decide-and-record):* migration step 1's Frame minted a slot ({{P-CoordinationCluster}}) because its directions (entities, contracts) await a Stage-3 spec that will fill it — its Frame-park is *temporary*. This program's Frame-park is **terminal**: per-function bundles carry the specs, and the program never authors one, so there is no downstream spec-authoring step to fill a program slot. The Frame itself is the durable designed-tier artifact every per-function bundle references for its cutover obligations (as migration step 1's §8 already does by anticipation) — a program ADR slot would duplicate the Frame, not add a fillable contract. *(Anchors: the ADR-vs-design-note criterion — an ADR earns its slot when a Stage-3 spec or an external consumer will reference the *decision* separately from the artifact; here the artifact is the reference. P-MinBlastRadius — no artifact whose reversal the Frame's own reversal does not already carry.)*

**One ADR-shaped question surfaced for the Principal Architect (not locked here — §16):** the standing removal authorization (§5) — agent-executed deletion of the old copy without a per-instance human gate, conditioned on mechanical fail-closed gates — has a **governance** character paralleling the merge-governance ADR (which authorizes agent-executed merges under conditions). Whether that authorization warrants a *governance*-tier ADR is a canon-amendment shape, which is the Principal Architect's call, not the Architect's to mint. Surfaced as a shape with reasoning (§16), not resolved.

## 13. Intent self-report

**(a) I read the JTBD as:** scope the multi-year brain→mnemra strangler as a *standing program* — one dependency-derived migration spine, one reusable fail-closed cutover ritual, one availability ladder the hosting hardening gates against, and one complete work-list with every function dispositioned and every judgment call ruled or routed — so that each per-function cutover executes as routine, small-batch mechanics referencing this Frame rather than re-litigating sequencing, acceptance, rollback, and availability from scratch, and program progress is measured against the one right axis (brain-shrinkage, function by function, until the old substrate is gone). The deeper intent I read: the program is the guarantee-by-mechanism discipline applied to the migration itself — the ordering rationale went stale silently three times, and a standing program artifact is the mechanism that stops that drift from recurring per-cutover.

**(b) Decisions that strain an enumerated Non-goal / Success criterion, named:**

1. **Metrics is a proposal, not a lock (2a ratification 2 / SC-decide-and-lock).** The decide-and-lock posture wants locked decisions; the metrics core-vs-plugin call is deliberately handed to the maintainer at Frame-exit. The strain is real and sanctioned: the 2a input reserved this one call for the gate, so it arrives as a proposal-for-gate (§7.3 item 2, §16), not a hedge — it carries decision content (plugin), a canon anchor (P-0018 D-BOUNDARY), and a named resolver (the Frame-exit gate). It is the *only* work-list disposition left to the gate.
2. **The sequence is a partial order, not the strict line SC1 might be read to want (§4.2).** SC1's observable — "every step's position derives from a named dependency" — is satisfied, but where two functions share no dependency edge the Frame deliberately does *not* invent an order (that would sequence by preference, violating Hard constraint 6). The strain: a reader expecting a single numbered list finds a DAG with parallelism at the leaves. Named so it is not read as an omission.
3. **`content` registry ruled build-new, not migrate (#19).** The census lists it among functions; ruling it build-new (it becomes the mnemra ingestion substrate) edges toward designing the ingestion function this Frame must not design. Kept at disposition-grain: the ruling is *that* it is build-new and routed to the ingestion bundle; the ELT mechanics are the bundle's (Non-goal 3). If the gate reads even the build-new ruling as function-design, it narrows to "routed to the ingestion bundle" without the build-new label.
4. **Findings F2/F3 carry pre-rulings the metrics bundle owns.** The "identify the writer" (F2) and "CLI path is the natural canonical" (F3 lean) edge toward mechanism. Held non-binding: F2 is a fix-now *investigation* (find, don't design), F3's canonical choice is an explicit non-binding lean (§7.4), the lock staying with the metrics bundle.

## 14. Consultations

None. The substrate was maintainer-ratified decision records (the strangler-migration decision walk, 2026-07-06) plus the census deliverable and the locked migration step 1 Frame; no principle conflict required a canon consult, and the one advisor pass this synthesis took (folded before draft) sharpened the flag-union cell-verification and the coordination-item disposition, not a canon question.

## 15. Routine decisions (batched)

Within-principle calls decided inline; none alters a ratified direction:

1. The statements register file is *created at first cutover*, not at Frame-merge — there is no statement to record until the first cutover fires, and an empty file is not a program artifact (Simplicity). Its shape is locked here (§9); the register-promotion convention (migration step 1's precedent) lands the Frame's own register entry at Frame-merge time.
2. The sequence is expressed as a dependency partial order (§4.2), not forced into a strict line — parallelism at the dependency-free leaves is the operator's small-batch call (P-MinBlastRadius; Hard constraint 6 — no preference-sequencing).
3. The four dormant tables (#23–26) are dispositioned retire-with-ritual as one batched confirm-before-exclude call, not four separate judgments — they share the same evidence (dead since seed, no CLI, no code) and the same ruling (their concepts are build-new in mnemra).
4. `repos` (#18) stays plugin rather than escalating a P-0018 amendment: its fan-in is one FK-in (small), the plugin-default rule applies, and no demonstrated foundational need overturns it — promotion would be a P-0018 amendment shape, not warranted here.
5. Self-referential tooling (#21, #49, #50) is dispositioned dissolves-with-substrate-at-decommission uniformly — it evolves the old substrate's own schema/mechanics and has no mnemra analog, so it neither migrates nor retires early; it goes when the old substrate is decommissioned.
6. `#59*` (native memory) keeps the census's out-of-`brain/`-boundary marker in the work-list rather than a separate appendix — burying the single highest-by-evidence item would repeat the exact omission the census flags.

## 16. Escalated decisions

Two items are handed to the Principal Architect — neither is an Architect lock:

1. **Metrics core-vs-plugin — the Frame-exit gate decision (2a ratification 2).** *Proposal:* plugin (§7.3 item 2). *Options considered (bounded):* (a) plugin — dispatch/skill-run measurement is workflow-primitive data, named in P-0018's negative space; the fan-out is a consumer shape, not a foundation; (b) core — the wide fan-out and the reporting/automation/knowledge-extraction dependency web argue foundational status. *Canon:* P-0018 D-BOUNDARY decides for (a) — the promotion discriminator is fan-*in* (foundational substrate every plugin references), and metrics is fan-*out* (a consumer). *Recommended framing:* the maintainer rules metrics plugin-or-core at Frame-exit with the full work-list in view; the proposal is plugin, anchored to P-0018 D-BOUNDARY. This is a reserved gate decision, not a canon gap. **Resolved (Frame-exit gate, 2026-07-07): ruled plugin — proposal adopted.**

2. **Whether the standing removal authorization warrants a governance-tier ADR (canon-amendment shape).** *The shape:* the ritual authorizes agent-executed deletion of the old copy without a per-instance human gate, conditioned on mechanical fail-closed gates — structurally parallel to the merge-governance ADR (agent-executed merges under conditions). *Reasoning for surfacing:* a standing authorization for un-gated destructive action on unreplicated data is the kind of durable governance decision the workspace records as a G-tier ADR, and every future cutover references it — but minting a governance ADR is canon stewardship (the Principal Architect's), not the Architect's to fiat. *Canon gap:* none exists yet; this would be a new governance decision, not an amendment to an existing one. *Recommended framing:* the Principal Architect decides whether the removal authorization is recorded as a governance ADR or stands as a locked direction in this Frame; the Architect's default (§12) is no program ADR slot, with this one governance question surfaced rather than resolved. **Resolved (Frame-exit gate, 2026-07-07): the Frame direction suffices — no governance ADR minted; the question re-opens if a second program-shaped standing authorization appears (the deferred separation-of-privilege rule names the next standing autonomous capability as its own re-examination point).**

## 17. Provenance

- **Intake:** [`docs/intent/strangler-program.md`](strangler-program.md), locked 2026-07-07, blob `6d58cd29b7e80e595e4492ab396975cbdf7f1de0`.
- **Elicitation record:** the ratified strangler-migration decision walk (2026-07-06 — ten maintainer-ratified program-mechanics answers), embedded in §2.1; the three 2a ratifications (2026-07-07), §2.2.
- **Decision-record + census substrate:** the brain-function census (2026-07-06, task #2166 — 60 + 1-adjacent functions, four structural findings; consumed zero-trust, reconciled against its literal cells); the workspace end-state synthesis (2026-06-19, not locked — superseded by the walk where they differ); the locked migration step 1 Frame ([`docs/intent/coordination-wedge-frame.md`](coordination-wedge-frame.md), 2026-07-06 — composed with at §5, back-check passed).
- **Baseline:** workspace architecture values, principles, and constraint edges as of 2026-07-07 (the §3 constraint-graph walk); project ADR corpus P-0001…P-0021 in this repo at the Frame branch's parent commit (P-0002, P-0013, P-0017, P-0018, P-0010 bearing directly).
- **Seed tasks referenced:** #1056 (per-cutover hosting hardening), #2164 (taste discussion), #2165 (mnemra backup requirement), #2166 (census), #2167 (offline-mode topology tripwire).

## Changelog

- **2026-07-07** — Frame authored (Stage 2b, cold-start, Frame-park terminal) from the locked intake and the ratified decision walk, discharging intake success criteria SC1–SC10: the one spine + the core≠order rule + the dependency-derived sequence with the Bucket A forcing function pinned (§4); the cutover ritual template with fail-closed removal preconditions and standing authorization, back-checked against migration step 1's §8 (§5); the availability ladder with backup preconditions positioned before the first sole-home removal (§6); the complete 60 + 1 work-list dispositioned from the closed six-member set, the flag-union of 17 reconciled against the census's literal cells (10 literal-FLAGGED + 1 likely-stale + 4 dormant + 1 unclear-write-path + 1 residence-split; the intake's "11 FLAGGED" corrected as off-by-one) and each ruled or routed, the four structural findings ruled fix-now/fix-at-cutover (§7); program constraints (§8); the progress axis + statements-register shape (§9); the four-surface threat pass under integrity+availability (§10); deferrals with DF1 firing mechanisms (§11); no program ADR slot (§12, split decision recorded); the intent self-report with four named strains (§13); two items escalated — the metrics plugin proposal for the Frame-exit gate and the removal-authorization governance-ADR shape (§16). Status: draft — Frame-exit gate pending.
- **2026-07-07 (Frame-exit lock)** — Frame-exit gate accepted by the maintainer: **metrics ruled plugin** (proposal adopted — §7.3 item 2, §16.1 resolved; the work-list's last open disposition closes) and **the standing removal authorization stands as a locked Frame direction** (no governance ADR minted — §16.2 resolved; re-opens on a second program-shaped standing authorization). Review status at acceptance: security-review r1 approve-with-conditions, zero blocker/high, one medium + three polish items folded (the r1-fold entry below), fold conformance-checked; r2 delta waived at the gate as proportionate. Status flipped locked; BOM `[audit_chain.frame]` appended (internal_conformance pass, generated_against intent `6d58cd2…`).
- **2026-07-07 (Frame-review r1 fold)** — folded Warden's Frame-review round 1 (dispatch 1425, approve-with-conditions). Operationalized the one Medium: added the §8 *mechanize the removal gate* inherited constraint (removal is structurally gated on machine-readable pass-evidence, not merely conditioned on it — a checklist does not satisfy fail-closed), gave snapshot-deletion a mechanical floor (§5 step 3 — undeletable until the standing daily backup has demonstrably captured the function; the ratified "kept until confidence" timing sits *above* that floor), rewrote the §9 register line (evidence record, not the gate — enforcement lives in the removal mechanism), and carried the snapshot sub-window into the §10 TS-2 threat pass. Three polish folds: repaired §7.4-F2 tables-vs-rows phrasing (six CLI-less tables = one live + five dormant tables occupying four census rows #23–26), added the §5 back-check restorability caveat (near-vacuous for a zero-corpus drain — restorability binds data-bearing cutovers), de-conflated §10 TS-3 frozen-substrate wording (§6 governs mnemra writes; the old-substrate freeze is writer-repoint + operator-only-live + drain). Closed the §5 authorization A1 seam with a §8 pointer. No locked direction re-opened. Status: draft — Frame-exit gate pending.
