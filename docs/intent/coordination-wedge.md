# Intake: Coordination wedge — actors · claims-as-locks · messaging

**Locked: 2026-07-06**

**Stakes:** high
**Date:** 2026-07-06
**Status:** locked (intake-exit gate confirmed by the maintainer 2026-07-06; Stage 1c review: security reviewer round 1, approve-with-conditions, zero blocker/high — 2 medium + 3 low + 2 nit; 6 folded, 1 dismissed with rationale, see Dismissed review flags. `spec_type` initially ratified `architecture`, corrected to `code` at the same gate per the verify-contract taxonomy rule: the wedge's end-state includes implementation, and a parked code-destined spec is typed `code` with `--designed-tier` as the sanctioned pre-impl verify mode.)
**Consumer:** agents
**Authorization:** task #2124 (board-placed 2026-07-05 as its own bundle, sequenced ahead of all content bundles; entry condition met 2026-07-05).
**Primary substrate:** the merger-role decision record (2026-07-05), the strangler-migration decision walk (2026-07-06 — migration-spine, availability matrix, cutover ritual, flip rules), the memory-taste decision record (2026-07-05, wedge-core pre-confirm), the captured task direction (2026-07-04).

```
spec_type: code          # ratified at intake-exit 2026-07-06 (corrected from architecture — see Status)
bootstrap: false
frame_relevant: true     # forced for code
```

Expected Stage 2 modulation: **cold-start** — new architectural surface (coordination cluster); full elicitation fires.
Program position: **Frame-park** — this bundle runs Intake → Frame-exit and parks at the designed-Frame tier; the Stage 3 spec is a later pickup on the maintainer's word.

## JTBD

Multiple concurrent instances of the orchestrator operate in one workspace today and cannot coordinate safely. Resource claims are advisory files — no atomic acquisition, no reliable liveness or cleanup binding to the session that wrote them. Handoffs carry no delivery or consumption state — items get consumed without a trace, or sit addressed-but-unarchivable because no consumer may safely disposition them. Cross-session continuity rides hand-rolled stash files whose every claim must be re-verified from scratch by a zero-trust consumer.

The job: make instance coordination a **designed capability of the context layer** — durable actor identity, mutual-exclusion claims, addressed messaging with delivery state — instead of a file convention. This is the system's first live workload by design intent: a coordination plane that runs over **zero migrated content**, defining what "the workspace starts using mnemra" means.

## Non-goals

1. **Content migration.** No knowledge-base, task, or memory corpus rows move with this wedge. The cluster runs over an empty corpus by design; content functions migrate in later strangler steps.
2. **Interim workspace tooling.** No file-convention upgrades, no stopgap CLIs on the current substrate. The existing conventions stay frozen until their cutover flip.
3. **Workflow primitives in core.** Tasks, dispatches, skill-runs stay plugin-tier ([P-0018](../src/adrs/P-0018-core-entity-manifest.md) D-BOUNDARY untouched). The wedge adds no workflow model to the core entity set.
4. **Push transport.** Delivery is poll-shaped at session start (parity with today's session-start injection). Push/streaming rides the streamable-HTTP tripwire — out of scope.
5. **Offline or queued coordination.** No local write queue, ever (reconciliation = split-brain by construction). Orchestration offline mode is deferred with a topology tripwire that fires when the service first runs off-box — a multi-node or disconnected-operation topology becoming a supported deployment (tracked as its own seed task).
6. **Composer/optimization tooling.** The dispatch-composer and spec-ops CLIs are optimizations riding the plugin system on their own pull — not migration steps, not part of this bundle.
7. **Stash-successor as a designed surface.** The session-stash/carrier is a **consumer pattern** of messaging, not a fourth designed surface (maintainer-ratified at intake, 2026-07-06). The cluster designs actors + claims + messages; the Frame notes the stash-successor shape (persistent queryable state shrinks it toward a pointer to live tracker), and its dissolution rides its own later cutover pass per the small-batch flip rule.

## Success criteria

1. **Durable actor identity.** An instance registers as an actor with role-instance addressing that survives session death. *Observable:* register → session dies → a successor session resolves the same actor identity rather than minting a duplicate.
2. **Atomic mutual-exclusion claims (exactly one holder).** Two instances contending for one resource: exactly one acquisition succeeds; the loser receives a mechanical refusal. The mechanism (advisory lock, row lock, compare-and-set) is Frame's to pick. *Observable:* concurrent-acquisition test — never two holders.
3. **Stale-claim semantics.** A claim held by a dead session is detectable and recoverable under defined TTL/takeover rules. *Observable:* kill a holder; a successor acquires per the stated rule — no manual-wipe limbo requiring the maintainer.
4. **Messaging with disposition state.** A message is addressed to a role-instance, persisted durably, acknowledged, and carries consumption/disposition state. *Observable:* the traceless-consumption and unarchivable-item failure modes are structurally impossible — every consumption leaves a record; every item has a disposition path.
5. **Founding message type.** The merge-request particulars (the merger-role input contract: repo, branch/worktree, governing-artifact + marker refs, gate facts, ride-alongs, base-pin, CI expectations) are expressible as a message type. *Observable:* a schema audit finds every locked merge-request particular representable in the message-type schema — no field of the contract falls back to freeform prose.
6. **Standalone cluster.** A fresh deployment with only coordination rows supports full fleet coordination. *Observable:* the cluster's schema and verbs have no dependency on content tables or corpus population.
7. **Fail-closed writes.** A coordination-write failure surfaces as an immediate, observable stop in the client contract — proceeding unclaimed/unacked is not silently possible.
8. **Cutover-compatible.** The design supports the ratified flip mechanics: small-batch flips (claims, messaging, stash-carrier as separate passes), drain-then-flip for in-flight items, scheduled operator-only-live flips, per the cutover ritual (statement → migrate → backup → remove). *Observable:* a dry-run flip plan demonstrates a small-batch flip with drain-then-flip on in-flight items and no dual-authority window at any point in the sequence.

## Hard constraints

1. **Standalone by design.** The coordination cluster (actors + claims + messages) MUST NOT depend on content machinery or corpus population (maintainer-ratified design constraint, 2026-07-04).
2. **Actors are the [P-0018](../src/adrs/P-0018-core-entity-manifest.md) core entity.** Hard-FK target, closed `actor_type ∈ {human, agent, system}`; the identity builtins (P-builtin-users / P-builtin-agents, [P-0002](../src/adrs/P-0002-core-plugin-partition.md)) populate it. The wedge builds on this entity — it does not fork or shadow it.
3. **Sequencing bound.** Available before any content-function migration; it is the first usage — the wedge is migration step 1 (ratified 2026-07-06), sequenced ahead of all content bundles.
4. **Availability matrix (ratified 2026-07-06).** Session-start + service down = no work. Mid-session + down = current unit may finish read-only; ALL writes fail closed; no queue. Coordination-write failure = immediate stop.
5. **No dual-authority window.** During cutover, exactly one claim system is authoritative at any time, governed by the per-function ritual; in-flight coordination state is drained, never migrated.
6. **Standing project canon applies.** Engine-agnostic Storage trait ([P-0010](../src/adrs/P-0010-storage-substrate-engine.md)), workspace scoping ([P-0006](../src/adrs/P-0006-v0-tenant-enforcement.md)), signing chain for whatever residence the design lands ([P-0005](../src/adrs/P-0005-v0-signing-chain.md)), single-digit agent-visible verb budget with content-type-as-parameter (the ratified verb-budget constraint).
7. **Core-vs-plugin residence is a required Frame disposition.** Maintainer lean: core function (precedents: retrieval as host subsystem, host-core-owned content types; wedge-core pre-confirmed at the memory-taste walk). Formal confirmation against the [P-0002](../src/adrs/P-0002-core-plugin-partition.md) verb-on-content criterion happens at Frame — recorded, not assumed.

## Evidence

- **2026-07-04:** four concurrent orchestrator instances (operator / research / governance / design lanes) coordinated on file conventions. One handoff was consumed with no trace; one stale addressed item could not be archived by any instance.
- **2026-07-06:** an instance adopted the live operator's claim file after a context reset — the claim protocol's invariants had to be retrofitted (skill amendment + cleanup hook) the same day. File claims carry no session binding the substrate can enforce.
- **2026-07-06 (same day):** archive-convention drift found during a session-carrier verify — eleven handoffs archived as loose files against a dated-directory convention, despite the convention being documented with exact commands. File conventions decay silently; there is no mechanism to make them hold. (Two concurrent instances then independently caught and filed the same drift within minutes of each other — the coordination gap coordinating about itself.)
- **Merger-lane precedent:** the merger-role decision (2026-07-05) already models the git main-merge lane as a claimed lease and defines merge-request particulars that await a message type — a designed consumer waiting on this capability.
- **Use case (end-to-end walk):** a design lane finishes a batch gate-green → files a merge-request message addressed to the merger role → a merger instance wakes, claims the target repo's main-merge lane (lease), lands the batch, dispositions the message, releases the lease. Every verb in that walk — register, address, append, ack, claim, release, disposition — is a wedge primitive.

## Consumer of resulting work

**Agents (primary, per P-AgentPrimarySource):** the orchestrator fleet — operator, scoped lanes, merger, research instances — is the day-1 user population; the maintainer consumes through fleet behavior. **Downstream:** the strangler-program Frame treats this wedge as migration step 1 (it strangles the file coordination substrate: handoff inbox + instance claims + stash carrier); the merger-lane protocol replaces its file-based input contract with the founding message type.

## Risk profile

**Touches trust boundary — flagged (required Stage-2 constraint; security-mode review fires at Frame where the mechanism is known):**

- Actor registration/authentication — who may register as which role-instance; impersonation is the wedge's version of claim-adoption.
- Claim authority — who may acquire, break, or take over a lease; stale-takeover is a privileged operation, and *which actor class* holds it is a least-authority scoping question (P-LeastAuthority).
- Message addressing — who may send to / read / disposition another actor's queue.
- **Instance-grain isolation — novel to this wedge, distinct from the register's deferred workspace-grain multi-tenancy.** Multiple instances share one workspace runtime, each with its own actor identity, claims, and queue. Default visibility across instances (may A enumerate or read B's queue and claims absent an explicit grant? is the default deny?) is a Frame threat-modeling item; reading this as the already-deferred tenant policy would silently drop it.
- Adjacent substrate: the permissions-model research headline (actor-as-principal spine over four-plane composition) is locked as the auth-bundle Frame substrate; this Frame must state its dependency posture toward it — consume as input now vs. name the overlap and defer.
- Reliability surface: fail-closed stop semantics are load-bearing for correctness (proceeding unclaimed defeats the wedge); the availability matrix is a constraint, not a wish.
- Attribute priority for Frame threat modeling: **integrity** (exactly-one-holder, no traceless consumption) and **availability** (fail-closed, no split-brain) are load-bearing; confidentiality is secondary in a single workspace, with the instance-isolation default as its one edge.
- The five surfaces above (isolation default, takeover authority, registration authenticity, addressing authorization, permissions-substrate posture) are the **Frame threat-modeling handoff set** (Stage 1c review r1).

## Design forks carried to Frame (recorded so intake carries them forward; deliberately unresolved here)

1. **Message shape:** rows (queue semantics, storage-cluster ownership) vs. content-artifacts (provenance envelope, retrievable via the front-door verb). The standalone constraint weighs on this fork — artifact-shape presumes content machinery, row-shape does not.
2. **Claims split:** in-system content coordination may be absorbed by storage semantics (transactions); resources OUTSIDE the system (git repos, config, filesystem) need advisory leases. Refinement carried: the real line may be **duration** (atomic write = transaction; held intent = lease) rather than residence; leases inherit today's stale/TTL/takeover semantics.
3. **Merger-shape tripwire:** the instance-vs-agent fork for the merger role re-opens at THIS Frame (messaging may change the wake calculus) — a required, recorded Frame disposition.
4. **Addressing:** role-instance, not session (survives session death, like claims) — captured as direction, Frame confirms.
5. **Transport:** poll at session start (1:1 with today's injection); push deferred (see Non-goal 4).
6. **Verb shape × residence (added from Stage 1c canon scan):** claim/release/ack/disposition/takeover are state-transitions, not CRUD-on-content. If exposed at the agent surface they fire the invocation model's deferred-domain-verb tripwire ([P-0013](../src/adrs/P-0013-plugin-invocation-model.md)); a host-builtin residence need not export via the plugin ABI at all. Frame resolves residence (Hard constraint 7) and verb shape **together**, under the verb-budget constraint (Hard constraint 6).
7. **Actor-grain confirm (added from Stage 1c canon scan):** each role-instance maps to a distinct durable `actors` row within the [P-0018](../src/adrs/P-0018-core-entity-manifest.md) grain (`actor_type = agent`) — no actor↔session intermediate grain; a successor session resolves the same row (Success criterion 1).

## Consultations

- _none at intake — the substrate is maintainer-ratified decision records (2026-07-04 through 2026-07-06); the Stage 1c review pass was the security reviewer (round 1, zero blocker/high, 6 findings folded, 1 dismissed)._

## Dismissed review flags

- **W7 (nit, reviewer confidence 40):** the JTBD's capability list ("durable actor identity, mutual-exclusion claims, addressed messaging with delivery state") edges toward solution-shape. Dismissed: at bundle altitude a JTBD naming its three sub-capabilities is acceptable; the surrounding problem framing is need-shaped. The reviewer concurred none-required.

## Open items resolved at the intake-exit gate

1. `spec_type` — ratified **code** (corrected from an initially-ratified `architecture` in the same gate conversation; see Status line for the rule).
2. Stash-carrier scope boundary — **consumer pattern** (folded into Non-goal 7).
3. Frame-park — confirmed: this bundle parks at Frame-exit; Stage 3 is a later pickup.
4. Register entry — lands at **Frame-merge time** per the register-promotion convention.
