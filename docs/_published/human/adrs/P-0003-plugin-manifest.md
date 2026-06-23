---
title: "P-0003: Plugin Manifest"
summary: "V0 plugin manifest schema and host-fn ABI surface: universal content.emit verb shape, schema_version: 1 envelope, typed host-fn allowlist compiled per-instance from signed manifest."
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

# P-0003: Plugin Manifest

## Status

`accepted`

## Context and Problem Statement

This is a P-ADR ([P-* ADR](../glossary.md#p--adr): an Architecture Decision Record scoped to a single project, recording context, the decision, rejected alternatives, and consequences). Each `core: true` plugin declares its identity, content ownership, and required host-fn surface through a manifest. The manifest is signed (per [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md)) and loaded by the plugin runtime before any plugin code runs. From the declared host-fn surface, the runtime compiles a per-instance allowlist. Any call outside that allowlist fails at the WIT boundary.

Two axes needed resolution before this ADR could lock.

1. **Universal `content.emit` vs aspect-map-per-type.** Under [P-0001-storage-layout](P-0001-storage-layout.md) C1 (single-document), the artifact is a whole row. There's no per-aspect split. The manifest's verb shape should match that single-document model: a universal `content.emit` over JSONB frontmatter plus body, not an `aspect_map` per type. Aspect maps would have been necessary under C2 (composite-with-typed-slots). C1 removes that surface entirely.

2. **Host-fn ABI scope.** Which categories of host functions can a plugin invoke? The ABI surface has to be wide enough to support the 4 `core: true` plugins declared in [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md), and narrow enough that the signed-manifest allowlist meaningfully limits what a plugin can do. Plugins are IO-free. All IO is host-mediated, and the ABI names the categories.

The manifest schema must carry an explicit `schema_version` field. That way V0.1+ manifest format evolution stays backward-compatible without breaking V0 plugins.

## Decision Drivers

- **C1 universal content.emit.** [P-0001-storage-layout](P-0001-storage-layout.md) locked the single-document layout. The manifest verb shape follows from that as a propagated consequence: universal artifact CRUD over JSONB, not per-aspect operations.
- **Per-instance allowlist compiled from signed manifest.** [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) requires the runtime to derive per-instance security properties from the signed manifest. The host-fn surface declared in the manifest is the input to that allowlist compilation.
- **ABI forward-compat.** Pre-1.0 ABI freedom (a quality attribute scenario) requires that an ABI-change PR forces all `core: true` plugins to recompile and pass tests. The manifest's `schema_version` field makes that recompile surface explicit. A V0.1 change that bumps the schema version produces a new manifest version that old V0 plugins can reject gracefully.
- **Security threat coverage.** `P-plugin-instance`/E (Critical) reads: "A plugin requests a host-fn outside its manifest's declared surface; if the host's permission check is at call-time and the plugin's manifest was loaded laxly, the plugin gains a capability it did not declare." The manifest is the structural mitigation. The allowlist is compiled into the per-instance binding, so calls outside it fail at the WIT boundary, not in the host-fn body.
- **`workspace_id` must not be a plugin parameter on write paths.** `P-host-fns`/T (Critical): `workspace_id` is derived from the session/token in `WorkspaceCtx` ([P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md)). The ABI MUST NOT accept `workspace_id` as a write-path parameter.
- **No direct DB or network from plugin.** An IO-free plugin core is a Hard constraint. The host-fn surface names the exact mediating functions. Anything not named is structurally unavailable.

## Considered Options

1. **M1 — Universal `content.emit` with typed host-fn allowlist (V0 shape).** A single manifest verb model. Host-fn categories are declared per-plugin, and the allowlist is compiled per-instance from the signed manifest.
2. **M2 — Aspect-map-per-type manifest.** Each plugin declares an `aspect_map` per content type, specifying which aspects (body, state, log, edges) it owns. Rich, but only meaningful under C2. It doesn't fit C1.
3. **M3 — Capability-flag-only manifest (no host-fn granularity).** The manifest declares capability flags (`content_read`, `content_write`, `metrics_write`) without per-fn granularity. Simpler but coarser, and it doesn't enable per-fn allowlist compilation.

## Decision Outcome

**M1 — Universal `content.emit` with typed host-fn allowlist.**

### Manifest schema (V0, schema_version: 1)

TOML format. All fields required unless marked optional.

```toml
[plugin]
name          = "tasks"              # unique; lowercase-kebab; max 64 chars
version       = "0.2.0"             # semver; plugin release version
schema_version = 1                  # manifest format version; bumped on breaking manifest changes
core          = true                # core: true = signed, non-uninstallable; false = not valid at V0

[verbs]
# List of MCP verb names this plugin exposes. Verb names are namespaced: "<plugin>.<verb>"
# e.g., ["task.create", "task.update", "task.get", "task.list", "dispatch.create", ...]
exposed = [...]

[content_types]
# plugin-owned artifact table(s). Each key maps to a table name in the content substrate.
# Under C1, each type is a per-artifact-type Postgres table.
# task = { table = "tasks", schema_doc = "docs/schemas/task.md" }

[state_scopes]
# plugin-owned state-shape KV namespaces (optional)
# "skill_run_state" = { description = "per-skill-run state KV" }

[host_fns]
# Required host functions. Declared as category: [fn_name, ...] pairs.
# The runtime compiles this into a per-instance allowlist; undeclared fns fail at WIT boundary.
required = [
  # Content CRUD (universal content.emit shape)
  "artifact.create",     # (type: str, frontmatter: JSON, body: str?) -> id: str
  "artifact.update",     # (id: str, frontmatter_patch: JSON, body: str?) -> ()
  "artifact.get",        # (id: str) -> (frontmatter: JSON, body: str?)
  "artifact.list",       # (type: str, filters: JSON) -> [id: str, ...]
  "artifact.delete",     # (id: str) -> ()  — destructive; requires manifest declaration
  # Observability
  "metrics.record",      # (verb: str, duration_ms: u64, status: str) -> ()
  "log.emit",            # (level: str, message: str, context: JSON?) -> ()
  # Events (timeseries write)
  "event.emit",          # (event_type: str, payload: JSON) -> ()
  # Projection
  "projection.emit",     # (projection_name: str, workspace_id: str, data: JSON) -> ()
]
# Optional host functions (declared but not mandatory for load)
optional = [
  # MCP sampling — only if plugin needs LLM completion via connected agent's MCP client
  "sampling.request",    # (prompt: str, context_ids: [str]) -> completion: str
  # Secrets (read-only; no write path from plugin)
  "secrets.get",         # (key: str) -> value: str
]

[signature]
# Populated at build time by the signing chain per P-0005-v0-signing-chain.
# Runtime verifies this before any plugin code executes.
algorithm   = "ed25519"
public_key  = "..."     # mnemra root public key fingerprint
sig_bytes   = "..."     # signature over canonical(manifest minus [signature])
signed_at   = "..."     # ISO 8601
```

### Host-fn ABI constraints (structural invariants)

These constraints are architectural, not stylistic. The runtime enforces them.

| Constraint | Enforcement | Source |
|---|---|---|
| `workspace_id` is NOT a parameter on any write-path host-fn | Host derives `workspace_id` from `WorkspaceCtx` (session/token); the plugin cannot supply it | [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) plus `P-host-fns`/T mitigation |
| Undeclared host-fn calls fail at WIT boundary | Per-instance allowlist compiled from signed manifest; failure is at binding, not in the host-fn body | `P-plugin-instance`/E mitigation |
| `artifact.delete` requires explicit manifest declaration | The destructive op isn't in the default `required` surface; the plugin must opt in | `P-cli-handler`/E mitigation pattern applied to the plugin surface |
| `sampling.request` content-IDs only (no artifact bodies in prompt) | At V0 all plugins are `core: true`, so the surface is contained; V0.1+ third-party install escalates this to Critical per `R-0007` | `DF-sampling-up`/I mitigation |
| No direct DB access; no network access | Wasmtime sandbox plus IO-free plugin core (Hard constraint) | Brief Hard constraints; architecture overview TB `TB-mnemra-host`↔`TB-plugin-sandbox` |
| `core: true` is the only valid value at V0 | Non-core plugin install is V0.1+ scope; the runtime rejects `core: false` manifests at V0 | [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) plus brief Hard constraints |

### Schema evolution (schema_version field)

`schema_version: 1` locks the V0 manifest format. V0.1+ changes that alter the manifest structure increment this field. The runtime loads a manifest by branching on `schema_version`. A V0 plugin presenting `schema_version: 1` against a V0.1 runtime keeps loading without modification. A V0.1 plugin presenting `schema_version: 2` against a V0 runtime produces a structured load error that names the schema_version mismatch.

This is the primary forward-compat invariant for the ABI evolution discipline quality attribute scenario: "A plugin calling an `@unstable` host function emits a deprecation warning."

### Consequences

**Good:**
- `P-plugin-instance`/E (Critical) mitigation: a per-instance host-fn allowlist compiled from the signed manifest. A call outside the allowlist fails structurally at the WIT boundary.
- The universal `content.emit` ABI is narrow. A typical work-verb plugin declares 5 content CRUD operations plus 4 observability operations, which is 9 required host-fn declarations. That's an auditable surface.
- The `workspace_id` write-path exclusion is structural: host-fn signatures never take `workspace_id` as a parameter, and `WorkspaceCtx` is host-derived.
- `schema_version: 1` gives a forward-compat break surface. ABI evolution is mechanical (recompile all `core: true` plugins) and well-bounded, since the set is small at V0.
- The signing chain in [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) signs the manifest. Verification is synchronous at load time, with no async path.

**Bad / Trade-offs:**
- TOML format for the manifest is a tooling dependency. Plugin authors need a TOML serializer in their build toolchain to produce the signed manifest. It's in the standard library for Rust, and feasible for other compile-to-WASM languages.
- Per-fn granularity in `host_fns.required` means adding a new host function to a plugin requires a manifest update and a re-sign. At V0, with 4 `core: true` plugins and a single signing authority, this is a controlled process. The cost scales with third-party plugin volume at V0.1+.
- `sampling.request` as an optional host-fn creates a two-tier plugin ABI. Plugins that declare it carry a different trust surface than plugins that don't. The manifest makes that split explicit, and the host's allowlist compilation handles it structurally.

## Pros and Cons of the Options

### M1 — Universal `content.emit` + typed host-fn allowlist (accepted)

- Pro: Propagates C1's single-document simplicity straight into the ABI, with no aspect-map complexity.
- Pro: Per-fn allowlist compilation is the structural mitigation for `P-plugin-instance`/E (Critical).
- Pro: `schema_version: 1` gives a clean break surface for ABI evolution.
- Con: A TOML plus signing toolchain dependency in the plugin build pipeline.

### M2 — Aspect-map-per-type manifest

- Pro: Rich per-aspect access control is possible (body, state, log, and edges as separate capability grants).
- Con: Only meaningful under the C2 layout; doesn't fit the C1 single-document model.
- Con: A substantially richer manifest surface, with no third-party plugins to validate it at V0.

### M3 — Capability-flag-only manifest

- Pro: A simpler manifest format with fewer fields to declare.
- Con: A coarse-grained allowlist (category-level) rather than per-fn, which is a weaker `P-plugin-instance`/E mitigation.
- Con: Doesn't accommodate `sampling.request` as an optional per-plugin flag without extending the flag vocabulary toward the same complexity as M1 anyway.

## More Information

- Frame open ADR slot: `{{P-PluginManifest}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table). Frame is Stage 2 of the work-shaping pipeline, where agents synthesize a constraint summary and rationale chain. This ADR resolves that slot.
- Depends on: [P-0001-storage-layout](P-0001-storage-layout.md) (C1 leads to universal `content.emit`); [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md) (4 `core: true` plugins whose manifests follow this schema).
- [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) — the signing chain signs the manifest; the `[signature]` section is populated at build time; the runtime verifies it synchronously at load.
- [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) — `WorkspaceCtx` is the source of `workspace_id` on all host-fn write paths; the host-fn signatures enforce the exclusion.
- [P-0007-plugin-resource-limits](P-0007-plugin-resource-limits.md) — per-instance fuel and memory limits apply to the Wasmtime instance executing the plugin; the manifest is loaded before instance instantiation.
- [P-0008-admin-token-shape](P-0008-admin-token-shape.md) — the admin token's `scopes` array feeds permission checks at the host layer; manifest-declared `verbs` are a separate surface (MCP-facing verb names, not token scopes).
- [P-0009-rls-admin-token](P-0009-rls-admin-token.md) — the verb-to-scope mapping: which admin token scope grants access to which manifest-declared verb categories.
- Threat references: `P-plugin-instance`/E,T,I; `P-host-fns`/T,I,E; `DF-host-fn-call`/T,I; `DF-sampling-up`/T,I; `P-cli-handler`/E; `DS-pg-content`/T. ([Overview](../architecture/overview.md))
- Accepted risk `R-0007`: plugin sampling is unrestricted at V0 because all plugins are `core: true`; it becomes Critical at V0.1+ third-party install.
- V0.1+ ABI follow-up: an `@stable`/`@unstable` annotation per host-fn in the WIT interface definitions; the stability-tier mark scenario from the quality-attribute tree.
