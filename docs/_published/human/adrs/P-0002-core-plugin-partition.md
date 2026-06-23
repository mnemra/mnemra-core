---
title: "P-0002: Core Plugin Partition"
summary: "Cohesion criterion for the V0 core plugin set: verb-on-content plugins vs foundational-substrate builtins. Partitions capability-family increments 0.2.0–0.14.0 into 4 core plugins + 7 builtins."
primary-audience: agent
---

---
status: "accepted"
date: "2026-05-24"
decision-makers: ["the maintainer"]
consulted: ["the orchestrator", "the security reviewer"]
informed: []
supersedes: null
superseded_by: null
---

# P-0002: Core Plugin Partition

## Status

`accepted`

## Context and Problem Statement

The V0 capability-family increments (`0.2.0` through `0.14.0`) have to be sorted into two categories. **Additional builtins** are compiled into the host and run with no WASM sandbox. **`core: true` plugins** are WIT-defined WASM modules that run in Wasmtime, signed, and cannot be uninstalled. Where an increment lands decides its path: a plugin runs on the plugin runtime, a builtin runs as host code.

This decision rides on [P-0001-storage-layout](P-0001-storage-layout.md). The partition criterion under that ADR's C1 (single-document layout) isn't the same one you'd use under C2 (composite-with-typed-slots). Under C1 every plugin works on whole artifact rows, the manifest declares the tables it owns and the host-function surface it calls, and the cohesion boundary is the artifact type family rather than the aspect type.

The brief's Hard constraints commit to three things. First, `core: true` plugins are signed by the mnemra root and structurally cannot be uninstalled. Second, plugins are leaves: no sideways linkage, and any cross-plugin call is mediated by the host. Third, projects and agents are **builtin**, not plugins. That last one is a bootstrap ordering problem: per-project plugin scoping needs project identity to exist before any plugin loads, so the thing that establishes project identity can't itself be a plugin. The [Frame](../intent/mnemra-core-frame.md) (the Stage 2 constraint document, here at Correction 1) sets down the builtin-versus-plugin boundary.

The Frame's Tier-A slot description: "The signed-and-non-uninstallable invariant binds whichever increments are partitioned as plugins."

## Decision Drivers

- **Cohesion criterion under C1.** Under C1's single-document layout, the discriminator falls out naturally. Does the capability work as a verb on content, a function that reads or writes artifact rows through the host-function ABI? If yes, it's a plugin candidate. Does it instead need foundational substrate state (identity, tenancy, session, permissions) that has to be in place before any plugin loads? If yes, it's a builtin.
- **Chicken-and-egg bootstrap constraint.** Any capability whose objects are a prerequisite for plugin scoping or loading can't be a plugin itself. The brief names projects and agents. The same constraint reaches users, sessions, workspaces, auth, and permissions.
- **Signed-and-non-uninstallable invariant.** Partitioning an increment as a `core: true` plugin commits it to the full set of constraints: signed by the mnemra root, loadable only from the signed artifact set, and not uninstallable at runtime. The invariant binds the whole increment, every verb in that family included.
- **ABI surface minimization.** Fewer artifact families in the plugin set means a narrower plugin manifest surface, which means fewer host-function declarations in [P-0003-plugin-manifest](P-0003-plugin-manifest.md). The partition should stay as coarse as cohesion allows.
- **V0.1+ third-party plugin activation.** The V0 partition sets the precedent that third-party plugin authors will read. Load a plugin up with substrate concerns and its ABI contract gets harder to stabilize before third-party install turns on.

## Considered Options

1. **O1 — Verb-on-content discriminator (coarse partition, 4 plugins)** — partition by artifact family; foundational state management stays builtin; work-shaped verbs go into plugins. Results in 4 `core: true` plugins covering the `0.2.0`–`0.14.0` capability families.
2. **O2 — Per-increment plugins (fine partition, up to 13 plugins)** — each capability-family increment is its own plugin. Maximally granular; maximally complex manifest surface.
3. **O3 — Minimal-plugin, builtin-first (0 plugins at V0)** — defer all plugin partitioning to V0.1+; ship everything as builtins at V0. Avoids plugin-partition complexity but forfeits the signed-plugin invariant for work-verb capabilities at V0.

## Decision Outcome

**O1 — Verb-on-content discriminator, coarse partition: 4 `core: true` plugins + 7 builtins.**

The cohesion criterion: a capability is a plugin when its verbs operate on content rows (CRUD against artifact tables) and carry no foundational substrate dependency. It's a builtin when its state is a prerequisite for plugin scoping, loading, or session management.

### Builtins (7 — host-compiled, not in WASM sandbox)

These are the `P-builtin-*` components named in the architecture overview's data-flow diagram (DFD). Their state has to exist before any plugin loads. By construction they sit above the plugin boundary.

| Builtin | Capability families | Rationale |
|---|---|---|
| `P-builtin-workspaces` | Workspace identity and lifecycle | Workspace scope is the root tenant boundary; scopes all artifact tables; must exist before plugin tables are queryable |
| `P-builtin-users` | User identity | User records are referenced by agent and session state; must exist before per-user session or per-user agent records |
| `P-builtin-agents` | Agent registration | Agents are referenced as `decision-makers` in plugin-owned artifacts (tasks, dispatches); brief Correction 1 names agents as builtin explicitly |
| `P-builtin-auth` | Authentication (OIDC RS + admin token) | Auth token validation gates every MCP request; must be available before any verb routes to a plugin |
| `P-builtin-sessions` | Per-MCP-connection session state | Session context is the source of `WorkspaceCtx` per [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md); must precede any plugin verb |
| `P-builtin-permissions` | Per-plugin permission grants | Permission checks for plugin verb access are host-gate; must be loaded before plugin manifests are evaluated |
| `P-builtin-projects` | Project registry | Per-project plugin scoping makes project identity a prerequisite for plugin loading; brief Correction 1 names projects as builtin explicitly |

### Core plugins (4 — `core: true`, signed, non-uninstallable)

These increments have verbs that work on content rows (CRUD, query, projection) and carry no foundational bootstrap dependency. They're partitioned as plugins.

| Plugin | Capability families | Increments |
|---|---|---|
| `tasks` | Task management; dispatch metrics and lifecycle; skill-run measurement; activity/audit log; collaboration session friction tracking | `0.2.0`, `0.3.0`, `0.4.0`, `0.5.0`, `0.6.0` |
| `repos` | Repo registry; relationships/edges; tags/taggings; dependency-approval state; scope-violation log | `0.7.0`, `0.8.0`, `0.9.0`, `0.10.0`, `0.11.0` |
| `jobs` | Job-search pipeline | `0.12.0` |
| `contacts` | Contacts | `0.13.0` |

`0.14.0` (content-corpus migration) is a **builtin migration handler** (`P-migration-handler` in the DFD). It isn't a plugin: migration is a one-shot destructive control-plane operation gated on the admin token, so it runs under the migration handler builtin.

**Partition judgment calls:**

- `0.5.0` (activity/audit log) goes to the `tasks` plugin because activity records reference task and dispatch artifacts. From the tasks family's point of view, the activity log is a verb-on-content capability. The log shape (a TimescaleDB hypertable) is still host-function-mediated per [P-0001-storage-layout](P-0001-storage-layout.md)'s four-shape model. The `tasks` plugin calls the `log.emit` host-function. It doesn't own the log substrate directly.
- `0.6.0` (collaboration session friction tracking) sits in the `tasks` plugin because session-friction records are associated with dispatch and skill-run artifacts. Same artifact-family reasoning.
- `0.8.0` (relationships/edges) and `0.9.0` (tags/taggings) sit in `repos` because the primary subjects are repo-registry artifacts and content-corpus items. The edge and tag tables reference artifact IDs across plugin families through JSONB foreign keys, which is the C1 pattern. Aggregating across plugins is a projection concern, not a substrate concern.
- This partition is **proposed for maintainer ratification** at Spec stage (the Stage 3 testable spec, here the ratification gate) before the `0.2.0` implementation dispatch. If any increment assignment turns out wrong, an ADR amendment creates a new partition with a supersedes link back here.

### Consequences

**Good:**
- The signed-and-non-uninstallable invariant applies cleanly to all four work-verb capability families from `0.2.0` onward.
- The ABI surface in [P-0003-plugin-manifest](P-0003-plugin-manifest.md) is bounded to 4 manifests and their declared tables and host-functions.
- Bootstrap ordering is deterministic. The 7 builtins initialize during host startup; plugins load in the plugin runtime once the host substrate is ready.
- V0.1+ third-party plugin authors see a small, stable set of builtin foundations. The 4 core plugins are the ABI precedent, not the builtins.

**Bad / Trade-offs:**
- Assigning increments inside a plugin family, especially the `tasks` plugin that covers 5 of them, makes for a larger initial plugin surface than the fine-grained alternatives. The trade-off: at V0, cohesion wins over granularity.
- `0.14.0` (content-corpus migration) as a builtin migration handler means migration capability can't be plugged in or upgraded on its own without a host rebuild. That's fine at V0 dogfood scope. V0.1+ could extract it if the migration surface grows.

## Pros and Cons of the Options

### O1 — Verb-on-content discriminator, 4 plugins (accepted)

- Pro: Cohesion by artifact family produces stable, low-surface plugin contracts.
- Pro: Builtin count (7) is bounded by the bootstrap-ordering constraint; no ambiguity about what must load first.
- Pro: Plugin-manifest surface in P-0003 is bounded and auditable.
- Con: `tasks` plugin spans 5 increments — larger initial surface. Manageable because increments share the same artifact-family tables.

### O2 — Per-increment plugins, up to 13

- Pro: Maximum granularity; each capability family independently upgradeable.
- Con: 13 plugin manifests at V0 without third-party plugin pressure to warrant the complexity.
- Con: ABI surface explosion; [P-0003-plugin-manifest](P-0003-plugin-manifest.md) would need to manage cross-plugin reference semantics for closely-related families (activity log references task artifacts, edges reference repo artifacts).

### O3 — Minimal-plugin, 0 plugins at V0

- Pro: Defers all plugin-partition complexity.
- Con: Forfeits the signed-plugin invariant for work-verb capabilities; `core: true` plugin proof-of-concept deferred to V0.1+.
- Con: The plugin ABI (host-fn surface, manifest schema) is unvalidated against real capability families before third-party install activates.

## More Information

- Frame open ADR slot: `{{P-CorePluginPartition}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table). This ADR resolves that slot.
- Depends on: [P-0001-storage-layout](P-0001-storage-layout.md) — C1 single-document layout; cohesion criterion follows from per-artifact-type tables.
- Downstream: [P-0003-plugin-manifest](P-0003-plugin-manifest.md) — manifest schema and host-fn ABI for the 4 `core: true` plugins locked here.
- Architecture overview DFD nodes: `P-builtin-workspaces`, `P-builtin-users`, `P-builtin-agents`, `P-builtin-auth`, `P-builtin-sessions`, `P-builtin-permissions`, `P-builtin-projects` (builtins); `P-plugin-runtime`, `P-plugin-instance` (plugin execution path). ([Overview](../architecture/overview.md))
- [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) — signing invariant binds the 4 `core: true` plugins; build-host-on-disk custody at V0.
- [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) — `WorkspaceCtx` binding applies on every plugin host-fn call; the session builtin provides the context.
- [P-0007-plugin-resource-limits](P-0007-plugin-resource-limits.md) — fuel/epoch/memory limits apply to the 4 plugin instances; builtins are host-code and not fuel-limited.
- Increment assignment is flagged for maintainer ratification before the `0.2.0` implementation dispatch.
