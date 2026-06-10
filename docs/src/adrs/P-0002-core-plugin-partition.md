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

The V0 capability-family increments (`0.2.0`–`0.14.0`) must be partitioned into two categories: **additional builtins** (compiled into the host, no WASM sandbox) and **`core: true` plugins** (WIT-defined WASM modules running in Wasmtime, signed, non-uninstallable). The partition determines which increments get the plugin runtime path and which get the host-code path.

This decision depends on [P-0001-storage-layout](P-0001-storage-layout.md) because the partition criterion under C1 (single-document layout) differs from what it would be under C2 (composite-with-typed-slots). Under C1 every plugin operates on whole artifact rows; the manifest declares owned tables and host-fn surface; the cohesion boundary is the artifact type family, not the aspect type.

The brief's Hard constraints commit to: (1) `core: true` plugins are signed by the mnemra root and structurally non-uninstallable; (2) plugins are leaves (no sideways linkage; cross-plugin calls are host-mediated); (3) projects and agents are **builtin**, not plugins (chicken-and-egg: per-project plugin scoping requires project identity to be established before any plugin loads). The Frame (Correction 1) codifies the builtin-vs-plugin boundary.

The Frame's Tier-A slot description: "The signed-and-non-uninstallable invariant binds whichever increments are partitioned as plugins."

## Decision Drivers

- **Cohesion criterion under C1.** Under C1's single-document layout, the natural partition discriminator is: **does the capability operate as a verb-on-content** (a function that reads/writes artifact rows via the host-fn ABI)? If yes — plugin candidate. Does it require foundational substrate state (identity, tenancy, session, permissions) that must exist before any plugin loads? If yes — builtin.
- **Chicken-and-egg bootstrap constraint.** Any capability whose objects are a prerequisite for plugin scoping or loading cannot itself be a plugin. Projects and agents are the named examples in the brief; the same constraint extends to users, sessions, workspaces, auth, and permissions.
- **Signed-and-non-uninstallable invariant.** Partitioning an increment as a `core: true` plugin commits it to: signed by mnemra root, loadable only from the signed artifact set, non-uninstallable at runtime. The invariant binds the whole increment — including all verbs in that family.
- **ABI surface minimization.** Fewer artifact families in the plugin set → narrower plugin manifest surface → fewer host-fn declarations in [P-0003-plugin-manifest](P-0003-plugin-manifest.md). The partition should be as coarse as cohesion allows.
- **V0.1+ third-party plugin activation.** The partition decision at V0 defines the precedent surface for third-party plugin authors. Overloading a plugin with substrate concerns makes its ABI contract harder to stabilize before third-party install activates.

## Considered Options

1. **O1 — Verb-on-content discriminator (coarse partition, 4 plugins)** — partition by artifact family; foundational state management stays builtin; work-shaped verbs go into plugins. Results in 4 `core: true` plugins covering the `0.2.0`–`0.14.0` capability families.
2. **O2 — Per-increment plugins (fine partition, up to 13 plugins)** — each capability-family increment is its own plugin. Maximally granular; maximally complex manifest surface.
3. **O3 — Minimal-plugin, builtin-first (0 plugins at V0)** — defer all plugin partitioning to V0.1+; ship everything as builtins at V0. Avoids plugin-partition complexity but forfeits the signed-plugin invariant for work-verb capabilities at V0.

## Decision Outcome

**O1 — Verb-on-content discriminator, coarse partition: 4 `core: true` plugins + 7 builtins.**

The cohesion criterion is: a capability is a plugin if its verbs operate on content rows (CRUD against artifact tables) and carry no foundational substrate dependency. It is a builtin if its state is a prerequisite for plugin scoping, loading, or session management.

### Builtins (7 — host-compiled, not in WASM sandbox)

These are the `P-builtin-*` components named in the architecture overview's DFD. Their state must exist before any plugin loads; they are above the plugin boundary by construction.

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

These increments have verbs that operate on content rows (CRUD, query, projection) and carry no foundational bootstrap dependency. They are partitioned as plugins.

| Plugin | Capability families | Increments |
|---|---|---|
| `tasks` | Task management; dispatch metrics and lifecycle; skill-run measurement; activity/audit log; collaboration session friction tracking | `0.2.0`, `0.3.0`, `0.4.0`, `0.5.0`, `0.6.0` |
| `repos` | Repo registry; relationships/edges; tags/taggings; dependency-approval state; scope-violation log | `0.7.0`, `0.8.0`, `0.9.0`, `0.10.0`, `0.11.0` |
| `jobs` | Job-search pipeline | `0.12.0` |
| `contacts` | Contacts | `0.13.0` |

`0.14.0` (content-corpus migration) is a **builtin migration handler** (`P-migration-handler` in the DFD) — it is not a plugin because migration is a one-shot destructive control-plane operation gated on the admin token. It executes under the migration handler builtin.

**Partition judgment calls:**

- `0.5.0` (activity/audit log) is assigned to the `tasks` plugin because activity records reference task and dispatch artifacts; the activity log is a verb-on-content capability from the tasks family perspective. The log shape (TimescaleDB hypertable) is still host-fn-mediated per [P-0001-storage-layout](P-0001-storage-layout.md)'s four-shape model — the `tasks` plugin calls `log.emit` host-fn; it does not own the log substrate directly.
- `0.6.0` (collaboration session friction tracking) is in the `tasks` plugin because session-friction records are associated with dispatch/skill-run artifacts; same artifact family reasoning.
- `0.8.0` (relationships/edges) and `0.9.0` (tags/taggings) are in `repos` because the primary subjects are repo-registry artifacts and content-corpus items; the edge and tag tables reference artifact IDs across plugin families via JSONB foreign keys (C1 pattern). Cross-plugin aggregation is a projection concern, not a substrate concern.
- This partition is **proposed for maintainer ratification** at Spec stage before the `0.2.0` implementation dispatch. If any increment assignment is wrong, an ADR amendment creates a new partition with a supersedes link here.

### Consequences

**Good:**
- Signed-and-non-uninstallable invariant applies cleanly to all four work-verb capability families from `0.2.0` onward.
- ABI surface in [P-0003-plugin-manifest](P-0003-plugin-manifest.md) is bounded to 4 manifests × their declared tables and host-fns.
- Bootstrap ordering is deterministic: 7 builtins initialized in host startup; plugins loaded in plugin-runtime after host substrate is ready.
- V0.1+ third-party plugin authors see a small, stable set of builtin foundations; the 4 core plugins are the ABI precedent, not the builtins.

**Bad / Trade-offs:**
- Increment assignment within plugin families (especially the `tasks` plugin covering 5 increments) creates a larger initial plugin surface than fine-grained alternatives. Trade-off: cohesion outweighs granularity at V0.
- `0.14.0` (content-corpus migration) as a builtin migration handler means migration capability is not independently pluggable or upgradeable without a host rebuild. Acceptable at V0 dogfood scope; V0.1+ could extract it if the migration surface grows.

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
