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

This is a project-scoped [ADR](../glossary.md#adr) (a [P-* ADR](../glossary.md#p--adr)), and it has been accepted.

## Context and Problem Statement

The V0 capability-family increments (`0.2.0` through `0.14.0`) have to be split into two categories. **Additional builtins** are compiled into the host and run without a WASM sandbox. **`core: true` plugins** are WIT-defined WASM modules that run in Wasmtime, are signed, and can't be uninstalled. Which side an increment lands on decides whether it's built on the plugin runtime path or the host-code path.

This decision depends on [P-0001-storage-layout](P-0001-storage-layout.md), because the partition criterion under C1 (single-document layout) is different from what it would be under C2 (composite-with-typed-slots). P-0001 chose C1. Under C1 every plugin operates on whole artifact rows, and the manifest declares the tables the plugin owns and the host functions (host-fns) it calls. So the cohesion boundary is the artifact type family, not the aspect type.

The product brief's Hard constraints commit to three things. First, `core: true` plugins are signed by the mnemra root and are structurally non-uninstallable. Second, plugins are leaves: there's no sideways linkage between them, and any cross-plugin call is mediated by the host. Third, projects and agents are **builtin**, not plugins. That last one is a chicken-and-egg problem: scoping plugins per project means project identity has to exist before any plugin loads. The [Frame](../glossary.md#frame) (the architecture-stage document, in its Correction 1) codifies this builtin-vs-plugin boundary.

The Frame's Tier-A slot description for this decision reads: "The signed-and-non-uninstallable invariant binds whichever increments are partitioned as plugins."

## Decision Drivers

- **Cohesion criterion under C1.** With C1's single-document layout, the natural way to tell the two categories apart is to ask whether the capability **operates as a verb-on-content**, meaning a function that reads and writes artifact rows through the host-fn ABI. If it does, it's a plugin candidate. If it needs foundational substrate state (identity, tenancy, session, permissions) that must exist before any plugin loads, it's a builtin.
- **Chicken-and-egg bootstrap constraint.** A capability whose objects are a prerequisite for scoping or loading plugins can't itself be a plugin. The brief names projects and agents as examples. The same constraint extends to users, sessions, workspaces, auth, and permissions.
- **Signed-and-non-uninstallable invariant.** Partitioning an increment as a `core: true` plugin commits it to being signed by the mnemra root, loadable only from the signed artifact set, and non-uninstallable at runtime. The invariant binds the whole increment, including every verb in that family.
- **ABI surface minimization.** Fewer artifact families in the plugin set means a narrower plugin manifest surface, which means fewer host-fn declarations in [P-0003-plugin-manifest](P-0003-plugin-manifest.md). So the partition should be as coarse as cohesion allows.
- **V0.1+ third-party plugin activation.** The partition chosen at V0 sets the precedent that third-party plugin authors will see. A plugin overloaded with substrate concerns has an ABI contract that's harder to stabilize before third-party install is turned on.

## Considered Options

1. **O1: Verb-on-content discriminator (coarse partition, 4 plugins).** Partition by artifact family. Foundational state management stays builtin, and work-shaped verbs go into plugins. This yields 4 `core: true` plugins covering the `0.2.0` through `0.14.0` capability families.
2. **O2: Per-increment plugins (fine partition, up to 13 plugins).** Each capability-family increment is its own plugin. Maximally granular, and it produces the most complex manifest surface.
3. **O3: Minimal-plugin, builtin-first (0 plugins at V0).** Defer all plugin partitioning to V0.1+ and ship everything as builtins at V0. This avoids partition complexity but gives up the signed-plugin invariant for work-verb capabilities at V0.

## Decision Outcome

**O1: Verb-on-content discriminator, coarse partition: 4 `core: true` plugins + 7 builtins.**

The cohesion criterion works like this. A capability is a plugin if its verbs operate on content rows (CRUD against artifact tables) and it has no foundational substrate dependency. It's a builtin if its state is a prerequisite for plugin scoping, plugin loading, or session management.

### Builtins (7 — host-compiled, not in WASM sandbox)

These are the `P-builtin-*` components named in the data-flow diagram (DFD) in the architecture overview. Their state must exist before any plugin loads, so they sit above the plugin boundary by construction.

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

These increments have verbs that operate on content rows (CRUD, query, projection) and have no foundational bootstrap dependency. They're partitioned as plugins.

| Plugin | Capability families | Increments |
|---|---|---|
| `tasks` | Task management; dispatch metrics and lifecycle; skill-run measurement; activity/audit log; collaboration session friction tracking | `0.2.0`, `0.3.0`, `0.4.0`, `0.5.0`, `0.6.0` |
| `repos` | Repo registry; relationships/edges; tags/taggings; dependency-approval state; scope-violation log | `0.7.0`, `0.8.0`, `0.9.0`, `0.10.0`, `0.11.0` |
| *(a further artifact-family plugin)* | A further artifact family | `0.12.0` |
| *(an additional artifact-family plugin)* | An additional capability family | `0.13.0` |

`0.14.0` (content-corpus migration) is a **builtin migration handler** (`P-migration-handler` in the DFD). It isn't a plugin, because migration is a one-shot, destructive control-plane operation gated on the admin token. It runs under the migration handler builtin.

**Partition judgment calls:**

- `0.5.0` (activity/audit log) goes to the `tasks` plugin because activity records reference task and dispatch artifacts. Seen from the tasks family, the activity log is a verb-on-content capability. The log shape (a TimescaleDB hypertable) is still mediated by host-fns, per the four-shape model in [P-0001-storage-layout](P-0001-storage-layout.md). The `tasks` plugin calls the `log.emit` host-fn; it doesn't own the log substrate directly.
- `0.6.0` (collaboration session friction tracking) is in the `tasks` plugin because session-friction records are associated with dispatch and skill-run artifacts. Same artifact-family reasoning.
- `0.8.0` (relationships/edges) and `0.9.0` (tags/taggings) are in `repos` because their primary subjects are repo-registry artifacts and content-corpus items. The edge and tag tables reference artifact IDs across plugin families through JSONB foreign keys (the C1 pattern). Aggregating across plugins is a projection concern, not a substrate concern.
- This partition is **proposed for maintainer ratification** at the [Spec](../glossary.md#spec) stage, before the `0.2.0` implementation dispatch. If any increment assignment turns out to be wrong, an ADR amendment creates a new partition with a supersedes link back to this record.

### Consequences

**Good:**
- The signed-and-non-uninstallable invariant applies cleanly to all four work-verb capability families from `0.2.0` onward.
- The ABI surface in [P-0003-plugin-manifest](P-0003-plugin-manifest.md) is bounded to 4 manifests × their declared tables and host-fns.
- Bootstrap ordering is deterministic. The 7 builtins are initialized during host startup, and plugins are loaded in the plugin runtime once the host substrate is ready.
- V0.1+ third-party plugin authors see a small, stable set of builtin foundations. The 4 core plugins are the ABI precedent, not the builtins.

**Bad / Trade-offs:**
- Grouping increments within plugin families (especially the `tasks` plugin, which covers 5 increments) gives a larger initial plugin surface than the fine-grained alternatives would. The trade-off: at V0, cohesion outweighs granularity.
- Making `0.14.0` (content-corpus migration) a builtin migration handler means the migration capability can't be plugged in or upgraded independently without rebuilding the host. That's acceptable at V0 dogfood scope. V0.1+ could extract it if the migration surface grows.

## Pros and Cons of the Options

### O1 — Verb-on-content discriminator, 4 plugins (accepted)

- Pro: Cohesion by artifact family produces stable, low-surface plugin contracts.
- Pro: The builtin count (7) is bounded by the bootstrap-ordering constraint, so there's no ambiguity about what must load first.
- Pro: The plugin-manifest surface in P-0003 is bounded and auditable.
- Con: The `tasks` plugin spans 5 increments, which is a larger initial surface. It's manageable because those increments share the same artifact-family tables.

### O2 — Per-increment plugins, up to 13

- Pro: Maximum granularity. Each capability family can be upgraded on its own.
- Con: 13 plugin manifests at V0, with no third-party plugin pressure to justify that complexity.
- Con: The ABI surface explodes. [P-0003-plugin-manifest](P-0003-plugin-manifest.md) would have to manage cross-plugin reference semantics for closely related families (the activity log references task artifacts, and edges reference repo artifacts).

### O3 — Minimal-plugin, 0 plugins at V0

- Pro: Defers all plugin-partition complexity.
- Con: Gives up the signed-plugin invariant for work-verb capabilities, and pushes the `core: true` plugin proof of concept out to V0.1+.
- Con: The plugin ABI (host-fn surface, manifest schema) goes unvalidated against real capability families before third-party install is turned on.

## Amendment 2026-10-16 — Disclosure-wording erratum

A disclosure-wording erratum was applied on 2026-10-16 (#3377) under the standing public-surface disclosure ruling. It rewrites the two single-increment rows of the core-plugin table in generic capability-family wording. Nothing else changed: the partition (4 `core: true` plugins + 7 builtins), the verb-on-content criterion, the increment assignments and every consequence above stay as they were. This is a wording erratum, not a design change, and this record isn't superseded.

## More Information

- Frame open ADR slot: `{{P-CorePluginPartition}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table). This ADR resolves that slot.
- Depends on: [P-0001-storage-layout](P-0001-storage-layout.md), the C1 single-document layout. The cohesion criterion follows from its per-artifact-type tables.
- Downstream: [P-0003-plugin-manifest](P-0003-plugin-manifest.md), which defines the manifest schema and host-fn ABI for the 4 `core: true` plugins locked here.
- Architecture overview DFD nodes: `P-builtin-workspaces`, `P-builtin-users`, `P-builtin-agents`, `P-builtin-auth`, `P-builtin-sessions`, `P-builtin-permissions`, `P-builtin-projects` (builtins); `P-plugin-runtime`, `P-plugin-instance` (plugin execution path). ([Overview](../architecture/overview.md))
- [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md): the signing invariant binds the 4 `core: true` plugins, with build-host-on-disk key custody at V0.
- [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md): the `WorkspaceCtx` binding applies on every plugin host-fn call, and the session builtin provides the context.
- [P-0007-plugin-resource-limits](P-0007-plugin-resource-limits.md): fuel, epoch and memory limits apply to the 4 plugin instances. Builtins are host code and aren't fuel-limited.
- The increment assignment is flagged for maintainer ratification before the `0.2.0` implementation dispatch.
