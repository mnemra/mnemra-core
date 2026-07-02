---
title: "P-0018: Core Opinionated Entity Manifest"
summary: "Locks the universal cross-plugin reference set mnemra-core defines and every plugin may reference: projects, actors (single table, actor_type ∈ {human, agent, system}), tags, and attachments as the four hard-FK-target core entities, plus audit-log/events as a core-owned emit-target surface (distinct in kind from the FK-target entities; its storage shape is owned by the observability baseline and the event-bus ADR, not re-decided here). Locks the negative space: workflow primitives (tasks, dispatches, skill-runs, specs, comments) are NOT core — they are plugin-introduced."
primary-audience: agent
---

---
status: "proposed"
date: "2026-07-02"
decision-makers: ["the maintainer"]
consulted: ["the architect", "the researcher", "the orchestrator"]
informed: []
supersedes: null
superseded_by: null
overrides: null
---

# P-0018: Core Opinionated Entity Manifest

**Project:** mnemra-core

## Status

`proposed`

Authored at the foundational-ADR-cluster stage alongside [P-0017-storage-cluster-model](P-0017-storage-cluster-model.md), formalizing the core-entity short list locked during the system-overview walk. It flips to `accepted` at the maintainer's review gate; what the gate reviews is this rendering at ADR precision and the two reconciliations flagged in "Decision Drivers" and "More Information" (the unified `actors` entity vs the [P-0002](P-0002-core-plugin-partition.md) builtin split; the audit-log/events surface vs the P-0010 D8 / E1 observability re-altitude).

**Substrate-independent.** This ADR locks *which entities are core* and *how they are referenced* (the reference model), not their storage mechanics. The core entities are laid out per [P-0017](P-0017-storage-cluster-model.md) / [P-0001](P-0001-storage-layout.md) under the engine-agnostic `Storage` trait ([P-0010](P-0010-storage-substrate-engine.md) D5); the audit/events surface's storage shape is owned elsewhere (see D-SURFACE).

## Context and Problem Statement

Every plugin in mnemra-core needs to reference a small set of universal concepts: *which project scopes this artifact*, *who authored or is assigned to it*, *how it is tagged*, *what files hang off it*, and *what happened to it*. If each plugin defined its own project, its own actor model, its own tags, the system would fragment: cross-plugin queries ("everything in project X", "everything assigned to agent Y") would be impossible without reconciling N private definitions, and there would be no stable foreign-key target for the references that hold the workspace together.

mnemra-core resolves this by **defining a short, opinionated set of core entities itself** — entities that live above the plugin boundary, that every plugin may reference, and that are the only hard foreign-key targets in the system. The set must be *short* (Simplicity — each core entity is a commitment every adopter inherits and every alternative orchestrator must accept) and *opinionated* (it encodes mnemra's stance on what is genuinely universal versus what is a workflow choice a plugin makes).

Three questions this ADR closes, each of which every capability plugin gates on:

1. **Membership.** Which entities are core, and which are deliberately *not*? The negative space is as load-bearing as the positive: making the wrong thing core forces every adopter to inherit a model that should have been a plugin's private choice.
2. **The reference kind.** Core membership is not uniform. Some core concepts are *entities plugins point at* (a task references a project); one is a *surface plugins emit to* (a plugin records an audit event). Conflating them hides a real distinction and drags an undecided storage question into this ADR.
3. **The actor model.** Authorship and assignment currently ride as free strings (`assigned_to: "Puck"`). A single typed `actors` entity with a closed `actor_type` replaces that — and must reconcile with the separate user/agent bootstrap builtins already partitioned in [P-0002](P-0002-core-plugin-partition.md).

This ADR does **not** define the plugin/builtin bootstrap partition ([P-0002](P-0002-core-plugin-partition.md)), the storage layout or cluster taxonomy ([P-0017](P-0017-storage-cluster-model.md) / [P-0001](P-0001-storage-layout.md)), the edge schema for the associative cluster ([P-0016](P-0016-edge-schema.md), accepted pending merge), the observability/audit storage shape (the [observability baseline](../architecture/overview.md#observability) per P-0010 D8 escalation E1), or the event-bus durability/delivery semantics (a forthcoming event-bus ADR). It names the entities and the reference model; those ADRs own the mechanics.

## Decision Drivers

- **Universal cross-plugin references need a stable, shared target (Maintainability, Decomposition).** Project scoping, authorship, taxonomy, and payload are cross-cutting in every plugin; a per-plugin definition of each fragments cross-plugin queries and removes any hard FK target. Defining them once, core-side, is what lets a plugin FK to a project it does not own.
- **Keep the set short and opinionated (Simplicity).** Each core entity is a permanent commitment every adopter and every alternative orchestrator inherits. The bar for core membership is *genuinely universal reference target*, not *frequently used*.
- **The negative space is load-bearing (Migration cost / portability, Honesty).** Workflow primitives (tasks, dispatches, skill-runs, specs, comments) are the shape a *particular* orchestrator imposes. Making tasks core would force every alternative orchestrator built on mnemra to inherit our task model — the wrong shape (the system-overview walk's explicit finding: "Tasks-as-core would force every alternative orchestrator to inherit our task model"). They are plugin-introduced.
- **The reference kind is a real distinction (Honesty, P-LockContract).** Projects/actors/tags/attachments are *entities plugins FK to*; audit-log/events is a *surface plugins emit to*. Locking them as one undifferentiated "core set" would (a) obscure the distinction and (b) pull an undecided storage question — where audit/events physically live — into an ADR whose scope is the reference model. They are locked as core, but as two kinds.
- **The audit/events storage shape is owned elsewhere (P-0010 D8 / E1).** The 2026-06-09 escalation-E1 disposition re-altituded observability *out* of the project-ADR layer ([P-0004](P-0004-observability-shape.md) `deprecated`, no successor ADR; the [observability baseline](../architecture/overview.md#observability) owns the audit surface); the event bus's durable-events table and delivery semantics are a forthcoming event-bus ADR. P-0018 must stay substrate-independent and **not** re-decide either — it locks the *reference/emit semantics*, not the store. *(Reconciliation flagged for confirmation.)*
- **Closed enums over free strings (P-ShiftLeft D2 — validator before field).** `actor_type` is a closed set `{human, agent, system}`; an actor reference is typed, not a free string. A closed enum has a mechanical validator; a free string does not — the same discipline the accepted retrieval corpus already applies ([P-0016](P-0016-edge-schema.md) `edge_class`/`origin`; [P-0015](P-0015-provenance-envelope-source-roles.md) trust axis). The `system` member is already exercised by [P-0016](P-0016-edge-schema.md) (system-actor attribution on host-written edges). *(Reconciliation: a single `actors` entity vs the separate `P-builtin-users` + `P-builtin-agents` bootstrap components in [P-0002](P-0002-core-plugin-partition.md); flagged for confirmation.)*

## Considered Options

1. **E1 — Minimal opinionated core set: projects, actors, tags, attachments (FK targets) + audit-log/events (emit surface); everything else plugin-introduced (chosen).**
2. **E2 — Broad core set: include workflow primitives (tasks, dispatches, skill-runs) as core entities.** Every orchestrator inherits mnemra's workflow model.
3. **E3 — No core entities: everything plugin-owned; all references are soft refs.** No hard FK targets; each plugin re-derives scoping, authorship, and taxonomy.

## Decision Outcome

**E1 — the minimal opinionated core set.** Three decisions lock.

### D-ENT — The core entity set (four FK-target entities)

mnemra-core defines these four entities. They are host-owned (above the plugin boundary — their existence is a prerequisite for plugin scoping and authorship, consistent with the [P-0002](P-0002-core-plugin-partition.md) builtin tier), and they are the **only hard foreign-key targets** in the system. Every plugin may FK to them; all *other* cross-plugin references are soft refs ([P-0017](P-0017-storage-cluster-model.md) D-SoT). *(Anchors: Maintainability + Decomposition — a shared stable reference target; Simplicity — the set is short; [P-0017](P-0017-storage-cluster-model.md) D-SoT — hard FKs are reserved for core entities.)*

| Core entity | Why it is core | Reference role |
|---|---|---|
| **projects** | Universal scoping; the partition key for almost every artifact. The highest-impact core entity — it sets every plugin's data partition. | Hard FK target (`project_id`) |
| **actors** | Universal authorship and assignment target. A single table with a closed `actor_type` (D-ACTOR) replaces free-string `assigned_to`. | Hard FK target (`actor_id`, e.g. `created_by`, `assigned_to`) |
| **tags** | Cross-cutting taxonomy; mnemra-core enforces tag uniqueness per workspace so a tag means one thing across plugins. | Hard FK target (via a tag-association row in the `associative` cluster) |
| **attachments** | Universal payload/file reference; deduplicated. Plugins reference an attachment; mnemra owns blob handling (the blob substrate is not decided here — it is owned by the storage/ingest layer). | Hard FK target (`attachment_id`) |

Binding rules:

- **These four are the only hard-FK targets.** A database foreign key is permitted only to a core entity. Any other cross-plugin reference is a soft ref (opaque ID, no FK — [P-0017](P-0017-storage-cluster-model.md) D-SoT). *Binary-observable:* a schema audit finds a hard foreign key only where the referenced table is one of the four core entities; every cross-plugin reference to a non-core entity is a soft ref.
- **The set is closed at four (plus the D-SURFACE emit surface).** Adding a core entity is an amendment to this ADR, held to the "genuinely universal reference target" bar. *(Anchor: Simplicity — the short list is the commitment.)*
- **Core entities are workspace-scoped like every other row.** They carry `workspace_id` and are reached under `WorkspaceCtx` ([P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md)); "core" means universally-referenceable, not workspace-global.

### D-ACTOR — Actors: one table, closed `actor_type`

Authorship and assignment reference a **single `actors` entity**, discriminated by a closed enum **`actor_type` ∈ `{human, agent, system}`**. This replaces free-string author/assignee fields (`assigned_to: "Puck"` → an `actor_id` FK). *(Anchors: P-ShiftLeft D2 — validator-before-field: a closed enum has a mechanical validator, a free string does not; the in-corpus closed-enum precedent already set by [P-0016](P-0016-edge-schema.md) (`edge_class`, `origin`) and [P-0015](P-0015-provenance-envelope-source-roles.md) (trust axis, displacement-event kinds).)*

**`actor_type` is the actor-*identity* axis — distinct from, and composing with, the two adjacent "source-role" axes in the accepted retrieval corpus.** It answers *who acted* (a person, an AI team member, or the host itself). It is **not** [P-0015](P-0015-provenance-envelope-source-roles.md)'s trust/source-role axis (`authoritative` / `background` / `outdated`), which grades *how trusted a source artifact is*, and it is **not** [P-0016](P-0016-edge-schema.md)'s edge-`origin` axis (`declared` / `extracted` / `system`), which records *how an edge came to exist*. The three compose on the `system` case and must stay aligned: a **`system` actor** (D-ACTOR identity) is the principal a host-initiated write runs under, which is exactly [P-0015](P-0015-provenance-envelope-source-roles.md)'s **system principal** at the decision port (a subsystem principal, not a workspace role) and produces [P-0016](P-0016-edge-schema.md)'s **`origin = system`** edges under system attribution in `created_by`. Naming the three facets keeps the shared `system` token from silently drifting across ADRs.

Binding rules:

- **`actor_type` is a closed set of exactly three members.** `human` (a person), `agent` (an AI team member / automated actor), `system` (host/system-initiated action — migration handler, host-side extractor, scheduled job). A value outside the set is rejected at write. *Binary-observable:* a write with `actor_type = "human"` succeeds; a write with `actor_type = "robot"` is rejected; `system` resolves for a host-initiated write with no human or agent principal.
- **One entity, one FK target.** Authorship (`created_by`), assignment (`assigned_to`), and any other actor reference all FK to the single `actors` table, not to per-type tables. *(Reconciliation with [P-0002](P-0002-core-plugin-partition.md): the `P-builtin-users` and `P-builtin-agents` bootstrap builtins remain the identity-management components that establish human and agent identity respectively; the `actors` table is the unified reference **entity** those components populate. Two lenses — bootstrap component vs FK-target entity — not a contradiction. Flagged for maintainer confirmation, since it refines how P-0002's two identity builtins surface as one reference target.)*

### D-SURFACE — Audit-log / events: a core-owned emit-target surface (not an FK-target entity)

Audit-log/events is **core** — a universal, mnemra-core-owned surface every plugin may record to — but it is core in a **different kind** from the four FK-target entities: plugins *emit to* it (via a host-fn), they do not *FK to* it. This distinction is deliberate and load-bearing. *(Anchors: Honesty — name the reference kind precisely; the [architecture overview](../architecture/overview.md) core-entity framing, which lists audit-log/events among the core cross-plugin surfaces; [P-0002](P-0002-core-plugin-partition.md) — audit is host-fn-mediated, plugins call `log.emit`/`event.emit`, they do not own the substrate.)*

Binding rules:

- **Plugins emit; they do not reference by FK.** A plugin records an audit event or emits a domain event through a host-fn (`log.emit` / `event.emit` / equivalent per [P-0003](P-0003-plugin-manifest.md)); the host writes it. Audit rows reference an `artifact_id` and `actor` without enforcing a database FK back to the artifact — audit must **outlive** the artifacts it describes (soft-delete + audit-of-deletion semantics; the Round-2 verdict placed audit as append-only, artifact-outliving). *Binary-observable:* deleting an artifact does not cascade-delete its audit rows; the audit trail survives.
- **P-0018 does not decide where audit/events physically live.** The audit surface's storage shape is owned by the [observability baseline](../architecture/overview.md#observability) (the 2026-06-09 escalation-E1 disposition re-altituded observability out of the ADR layer — [P-0004](P-0004-observability-shape.md) `deprecated`, no successor; time-series stores use plain timestamped tables per [P-0010](P-0010-storage-substrate-engine.md) D8, TimescaleDB demoted). The durable-events table and delivery/ordering semantics for the *event bus* are owned by a forthcoming event-bus ADR. P-0018 locks only that audit-log/events is a **core-owned emit-target surface with artifact-outliving, append-only reference semantics** — not its substrate. *(Reconciliation flagged: this keeps P-0018 substrate-independent and avoids re-deciding the observability re-altitude.)*

### D-BOUNDARY — The negative space: workflow primitives are not core

**Workflow primitives are plugin-introduced, not core:** tasks, dispatches, skill-runs, specs, comments, and their kin. They are the shape a *particular* orchestrator (Puck's workflow) imposes, not a universal reference target. *(Anchors: Migration cost / portability — a core task model would force every alternative orchestrator built on mnemra to inherit it; the system-overview walk's explicit finding; [P-0002](P-0002-core-plugin-partition.md) — tasks/dispatches/skill-runs are the `tasks` plugin's content types, not builtins.)*

Binding rules:

- **A workflow primitive is a plugin content type.** Tasks and dispatches live in the `tasks` plugin ([P-0002](P-0002-core-plugin-partition.md)); they FK to core entities (a task FKs to its `project` and its `actor`) but are not themselves core. *Binary-observable:* a task row carries a hard FK to `projects` and to `actors`, and no plugin outside its owner carries a hard FK to `tasks` — a reference to a task from another plugin is a soft ref ([P-0017](P-0017-storage-cluster-model.md) D-SoT).
- **The core/plugin line is drawn at universality, not frequency.** That tasks are the most-used artifact in the current workspace does not make them core; universality of *reference target* is the bar, and an alternative orchestrator would not share our task model.

### Consequences

**Good:**

- Cross-plugin queries over the universal axes (by project, by actor, by tag) resolve against one shared definition, not N private ones.
- The four hard-FK targets are the only place referential integrity is DB-enforced; everything else is a soft ref, keeping the plugin sandbox boundary crisp ([P-0017](P-0017-storage-cluster-model.md) D-SoT, [P-0003](P-0003-plugin-manifest.md)).
- A typed `actors` model with a closed `actor_type` replaces drift-prone free strings and gives authorship/assignment a validator; the `system` member cleanly attributes host-initiated writes (already exercised by [P-0016](P-0016-edge-schema.md)).
- The FK-target-vs-emit-surface distinction keeps P-0018 substrate-independent: it locks audit/events reference semantics without re-deciding the observability re-altitude or the event-bus store.
- The negative space keeps mnemra portable: an alternative orchestrator adopts mnemra-core without inheriting Puck's workflow model.

**Bad / Trade-offs:**

- Four core entities are four permanent commitments; a mistaken inclusion is expensive to reverse (every adopter inherits it). Mitigated by the short-list bar and by keeping workflow primitives out.
- The unified `actors` table refines how [P-0002](P-0002-core-plugin-partition.md)'s separate user/agent builtins surface; the reconciliation must be confirmed at the review gate rather than assumed.
- Deferring the audit/events *store* to other owners means P-0018 alone does not make audit runnable — it locks the reference contract; the observability baseline and event-bus ADR complete it.

## Pros and Cons of the Options

### E1 — Minimal opinionated core set (chosen)

- Pro: short, portable, and opinionated; the only hard-FK targets are genuinely universal.
- Pro: the FK-target-vs-emit-surface split keeps the ADR substrate-independent and avoids re-deciding observability storage.
- Con: four permanent commitments; a wrong inclusion is costly. Bounded by the universality bar.

### E2 — Broad core set (workflow primitives as core)

- Pro: tasks/dispatches are heavily used; making them core would give them DB-enforced integrity everywhere.
- Con: forces every alternative orchestrator to inherit mnemra's task/dispatch model — the wrong shape (portability/Migration-cost failure; the overview-walk finding). Rejected.

### E3 — No core entities (all references soft)

- Pro: maximal plugin autonomy; nothing is imposed core-side.
- Con: no hard FK targets anywhere; every plugin re-derives project scoping, authorship, and taxonomy, and cross-plugin queries over those axes require reconciling N private definitions. Loses the universal-reference-target value the core set exists to provide. Rejected.

## More Information

**Reconciliations flagged for maintainer confirmation at the review gate:**

1. **Unified `actors` vs the [P-0002](P-0002-core-plugin-partition.md) builtin split.** P-0018 locks one `actors` entity with `actor_type ∈ {human, agent, system}`; P-0002 partitions `P-builtin-users` and `P-builtin-agents` as separate bootstrap builtins. Read here as two lenses (identity-management component vs unified FK-target entity), not a contradiction — the builtins populate the one entity. Confirm the reading.
2. **Audit-log/events as an emit surface, not an FK-target entity, with its store owned elsewhere.** P-0018 locks the reference/emit semantics (append-only, artifact-outliving, host-fn-mediated) and explicitly defers the storage shape to the [observability baseline](../architecture/overview.md#observability) (per P-0010 D8 / escalation-E1, 2026-06-09) and the forthcoming event-bus ADR. Confirm this keeps P-0018 substrate-independent as intended.

**Relationships:**

- Refines: [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md) — P-0002 partitions the bootstrap components (builtins vs plugins); P-0018 enumerates the core FK-target entity set and the emit surface, overlapping on projects/actors and adding tags/attachments as core entities.
- Composes with: [P-0017-storage-cluster-model](P-0017-storage-cluster-model.md) D-SoT (hard FKs reserved for core entities; everything else soft ref); [P-0001-storage-layout](P-0001-storage-layout.md) (core entities use the C1 layout); [P-0010-storage-substrate-engine](P-0010-storage-substrate-engine.md) (laid out under the `Storage` trait); [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) (`workspace_id` on core-entity rows); [P-0003-plugin-manifest](P-0003-plugin-manifest.md) (the `event.emit`/`log.emit` host-fns plugins use for the emit surface); [P-0016-edge-schema](P-0016-edge-schema.md) (exercises `actor_type = system`; tag associations and edges live in the `associative` cluster — accepted pending merge); [P-0015-provenance-envelope-source-roles](P-0015-provenance-envelope-source-roles.md) (the trust/source-role and system-principal axes D-ACTOR aligns `actor_type` against — accepted pending merge).
- Storage of the audit/events surface: the [observability baseline](../architecture/overview.md#observability) ([P-0004-observability-shape](P-0004-observability-shape.md) `deprecated`, re-altituded per P-0010 D8 escalation-E1) and a forthcoming event-bus ADR — not decided here.
- Source: the system-overview walk's core-entity short list (the [architecture overview](../architecture/overview.md) core-entities section, a workspace-private forward-design companion; its verdict — projects / actors / tags / attachments / audit-log-events, with workflow primitives explicitly non-core — is stated inline above so this ADR reads self-contained).
- Validated against use cases (workspace-private working artifacts): the schema-context-excavation use case, 2026-06-08 (projects as an FK target; the actor-model drift a single `actors` entity closes); the `get_context_for` use case, 2026-06-05 (soft refs across artifact families, with hard FKs only to core entities). No use case required a workflow primitive to be core.
