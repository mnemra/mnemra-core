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

Each `core: true` plugin declares its identity, content ownership, and required host-fn surface via a manifest. The manifest is signed (per [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md)) and loaded by the plugin runtime before any plugin code executes. The runtime compiles the manifest's declared host-fn surface into a per-instance allowlist; calls outside the allowlist fail at the WIT boundary.

Two axes needed resolution before this ADR could lock:

1. **Universal `content.emit` vs aspect-map-per-type.** Under [P-0001-storage-layout](P-0001-storage-layout.md) C1 (single-document), the artifact is a whole row — there is no per-aspect split. The manifest's verb shape should reflect the single-document model: a universal `content.emit` over JSONB frontmatter + body, not an `aspect_map` per type. Under C2 (composite-with-typed-slots), aspect maps would have been necessary; C1 eliminates that surface.

2. **Host-fn ABI scope.** What categories of host functions can a plugin invoke? The ABI surface must be: wide enough to support the 4 `core: true` plugins declared in [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md); narrow enough that the signed-manifest allowlist meaningfully limits what a plugin can do. Plugins are IO-free — all IO is host-mediated; the ABI names the categories.

The manifest schema must carry an explicit `schema_version` field so V0.1+ manifest format evolution can be backward-compatible without breaking V0 plugins.

## Decision Drivers

- **C1 universal content.emit.** [P-0001-storage-layout](P-0001-storage-layout.md) locked the single-document layout. The manifest verb shape is a propagated consequence: universal artifact CRUD over JSONB, not per-aspect operations.
- **Per-instance allowlist compiled from signed manifest.** [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) requires that the runtime derive per-instance security properties from the signed manifest. The host-fn surface declared in the manifest is the input to the allowlist compilation.
- **ABI forward-compat.** Pre-1.0 ABI freedom (quality attribute scenario) requires that an ABI-change PR causes all `core: true` plugins to recompile and pass tests. The manifest's `schema_version` field makes that recompile surface explicit: V0.1 changes bumping the schema version produce a new manifest version that old V0 plugins can reject gracefully.
- **Security threat coverage.** `P-plugin-instance`/E (Critical): "A plugin requests a host-fn outside its manifest's declared surface; if the host's permission check is at call-time and the plugin's manifest was loaded laxly, the plugin gains a capability it did not declare." The manifest is the structural mitigation: allowlist compiled into the per-instance binding; calls outside fail at WIT boundary, not at host-fn body.
- **`workspace_id` must not be a plugin parameter on write paths.** `P-host-fns`/T (Critical): workspace_id is derived from the session/token in `WorkspaceCtx` ([P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md)); the ABI MUST NOT accept `workspace_id` as a write-path parameter.
- **No direct DB or network from plugin.** Plugin IO-free core is a Hard constraint. Host-fn surface names the exact mediating functions; anything not named is structurally unavailable.

## Considered Options

1. **M1 — Universal `content.emit` with typed host-fn allowlist (V0 shape)** — single manifest verb model; host-fn categories declared per-plugin; allowlist compiled per-instance from signed manifest.
2. **M2 — Aspect-map-per-type manifest** — each plugin declares an `aspect_map` per content type, specifying which aspects (body, state, log, edges) it owns. Rich but only meaningful under C2; does not fit C1.
3. **M3 — Capability-flag-only manifest (no host-fn granularity)** — manifest declares capability flags (e.g., `content_read`, `content_write`, `metrics_write`) without per-fn granularity. Simpler but coarser; does not enable per-fn allowlist compilation.

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

These constraints are architectural, not just stylistic. The runtime enforces them.

| Constraint | Enforcement | Source |
|---|---|---|
| `workspace_id` is NOT a parameter on any write-path host-fn | Host derives `workspace_id` from `WorkspaceCtx` (session/token); plugin cannot supply it | [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) + `P-host-fns`/T mitigation |
| Undeclared host-fn calls fail at WIT boundary | Per-instance allowlist compiled from signed manifest; failure is at binding, not at host-fn body | `P-plugin-instance`/E mitigation |
| `artifact.delete` requires explicit manifest declaration | Destructive op not included in default `required` surface; plugin must opt in | `P-cli-handler`/E mitigation pattern applied to plugin surface |
| `sampling.request` content-IDs only (no artifact bodies in prompt) | At V0 all plugins are `core: true` so surface is contained; V0.1+ third-party install escalates this to Critical per `R-0007` | `DF-sampling-up`/I mitigation |
| No direct DB access; no network access | Wasmtime sandbox + IO-free plugin core (Hard constraint) | Brief Hard constraints; architecture overview TB `TB-mnemra-host`↔`TB-plugin-sandbox` |
| `core: true` is the only valid value at V0 | Non-core plugin install is V0.1+ scope; runtime rejects `core: false` manifests at V0 | [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) + brief Hard constraints |

### Schema evolution (schema_version field)

`schema_version: 1` locks the V0 manifest format. V0.1+ changes that alter the manifest structure increment this field. The runtime loads a manifest by `schema_version` branch; a V0 plugin presenting `schema_version: 1` against a V0.1 runtime continues to load without modification. A V0.1 plugin presenting `schema_version: 2` against a V0 runtime produces a structured load error naming the schema_version mismatch.

This is the primary forward-compat invariant for the ABI evolution discipline quality attribute scenario: "A plugin calling an `@unstable` host function emits a deprecation warning."

### Consequences

**Good:**
- `P-plugin-instance`/E (Critical) mitigation: per-instance host-fn allowlist compiled from signed manifest; call outside allowlist fails structurally at WIT boundary.
- Universal `content.emit` ABI is narrow: 5 content CRUD operations + 4 observability operations = 9 required host-fn declarations for a typical work-verb plugin. Auditable surface.
- `workspace_id` write-path exclusion is structural: host-fn signatures never accept `workspace_id` as a parameter; `WorkspaceCtx` is host-derived.
- `schema_version: 1` provides a forward-compat break surface: ABI evolution is mechanical (recompile all `core: true` plugins) and well-bounded (small set at V0).
- Signing chain in [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) signs the manifest; verification is synchronous at load time; no async path.

**Bad / Trade-offs:**
- TOML format for the manifest is a tooling dependency. Plugin authors need a TOML serializer in their build toolchain to produce the signed manifest. Standard library for Rust; feasible for other compile-to-WASM languages.
- Per-fn granularity in `host_fns.required` means adding a new host function to a plugin requires a manifest update and a re-sign. At V0 with 4 `core: true` plugins and a single signing authority, this is a controlled process; the cost scales with third-party plugin volume at V0.1+.
- `sampling.request` as an optional host-fn creates a two-tier plugin ABI: plugins that declare it have a different trust surface than those that do not. The manifest makes this explicit; the host's allowlist compilation handles it structurally.

## Pros and Cons of the Options

### M1 — Universal `content.emit` + typed host-fn allowlist (accepted)

- Pro: Propagates C1's single-document simplicity directly into the ABI — no aspect-map complexity.
- Pro: Per-fn allowlist compilation is the structural mitigation for `P-plugin-instance`/E (Critical).
- Pro: `schema_version: 1` provides a clean break surface for ABI evolution.
- Con: TOML + signing toolchain dependency in the plugin build pipeline.

### M2 — Aspect-map-per-type manifest

- Pro: Rich per-aspect access control possible (body vs state vs log vs edges as separate capability grants).
- Con: Only meaningful under C2 layout; does not fit C1 single-document model.
- Con: Substantially richer manifest surface without third-party plugins to validate it at V0.

### M3 — Capability-flag-only manifest

- Pro: Simpler manifest format; fewer fields to declare.
- Con: Coarse-grained allowlist (category-level) rather than per-fn — weaker `P-plugin-instance`/E mitigation.
- Con: Does not accommodate `sampling.request` as an optional per-plugin flag without extending the flag vocabulary toward the same complexity as M1 anyway.

## Amendment 2026-06-30 — Component content-hash field (supply-chain binding)

The signing-to-runnable Frame ([`docs/intent/signing-to-runnable-frame.md`](../../intent/signing-to-runnable-frame.md), §5) routed the **content-hash-of-the-component-bytes** field to a P-0003 amendment (the `{{P-ManifestContentHash}}` slot) rather than a spec-local field: the field is a **durable part of the manifest contract every `core: true` plugin must satisfy** — it is recomputed and enforced on **every** plugin load by the verified-load gate (`libs/mnemra-host/startup/pool_population.rs:126`) — and it is a **manifest-schema change** (a new key in the signed canonical body), and P-0003 is the **sole authority** for that schema. A spec-local field would orphan a schema field from its schema authority: a future plugin author reading P-0003 would not see the content-hash requirement, and a future manifest-schema change could silently drop it.

The **runtime behavior** (recompute-and-reject-on-mismatch, fail-closed, single-read load, distinct error variant) is governed by **R-0021** in [`docs/specs/2026-06-30-signing-to-runnable.md`](../../specs/2026-06-30-signing-to-runnable.md); this amendment governs the **schema** (the field, its location, its algorithm constraint, and its presence rule).

### Schema change — `[component]` section in the signed canonical body

The V0 manifest schema gains a `[component]` section carrying a content-hash of the component (`.wasm`) bytes. It is located in the **signed canonical body** — the bytes **before** the `\n[signature]` marker that the signature covers (the slice extracted at `libs/mnemra-host/signing/verify.rs:337-344`). This location is **forced** by [P-0005](P-0005-v0-signing-chain.md) R-0005-h (core-by-provenance): a content-hash in the unsigned `[signature]`-adjacent region would let an attacker swap both the component and its declared hash.

```toml
# ... [plugin], [verbs], [content_types], [state_scopes], [host_fns] (unchanged) ...

[component]
# Content-hash binding of the component (.wasm) bytes. In the SIGNED body
# (before [signature]) so the signature covers it (R-0005-h core-by-provenance).
hash_alg = "blake3"   # one of {blake3, sha256, sha384, sha512}; V0 = blake3; MD5/SHA-1 rejected
hash     = "..."      # mandatory; lowercase-hex digest of the component .wasm bytes under hash_alg

[signature]
# unchanged — covers everything above this marker
```

| Field | Type | Constraints |
|---|---|---|
| `[component].hash_alg` | string | One of `{"blake3","sha256","sha384","sha512"}`; V0 locked value `"blake3"`; `"md5"`/`"sha1"`/any other value SHALL be rejected at load |
| `[component].hash` | string | **Mandatory** (absence is a fail-closed load rejection); lowercase-hex digest of the component `.wasm` bytes under `hash_alg` |

**Named algorithm (banned-weak constraint).** The content-hash algorithm SHALL be **SHA-256 or stronger / BLAKE3**; `MD5` and `SHA-1` are **banned** and SHALL be rejected at load. The V0 locked value is **BLAKE3**, reusing the in-tree BLAKE3 primitive already mandated for admin-token hashing ([P-0008](P-0008-admin-token-shape.md) / R-0008-b) — no new dependency (Simplicity; `P-StackDiscipline`). The strong set is the schema's **forward-allowance**; at V0 the only value any manifest carries is `blake3` (`sha256`/`sha384`/`sha512` are accepted by the schema but not exercised by a V0 manifest).

**Binds bytes, not a path.** The `[component]` section binds the component's **bytes** (via `hash`); it carries **no path or name field**. The runtime resolves which component to load by other means (the single `core: true` plugin at V0); a manifest-declared component path/name is an out-of-scope multi-plugin concern, not part of this schema change.

**Read only from the signed slice (complete-mediation).** The `[component]` table SHALL be parsed **only from the signed canonical body** — the bytes the signature covers, before the `\n[signature]` marker (the slice extracted at `libs/mnemra-host/signing/verify.rs:337-344`). A `[component]` table appearing in the unsigned `[signature]`-adjacent region SHALL NOT satisfy the field-presence requirement and SHALL cause rejection: the enforced value is always the signed value, never an unsigned-region copy. (Behavioral enforcement: spec R-0021-c/-e.)

### `schema_version` stays `1` — and the mandatory-field rule (one decision)

The "Schema evolution" section above states: *"`schema_version: 1` locks the V0 manifest format. **V0.1+ changes that alter the manifest structure increment this field.**"* Adding a `[component]` section **does** alter the manifest structure, so on the literal face of that rule it would increment. It does **not** increment, and the reasons are recorded here so this ADR is not silently self-contradicted:

1. **V0 is still being *defined*, not evolved post-freeze.** No external plugin has shipped against a frozen V0 schema. The increment rule governs **V0.1+** changes — changes that alter the format *after* V0 is fixed — so a third party loading an older V0 plugin can detect the mismatch. There is no such older population: the single `core: true` plugin (`mnemra-echo`) is **re-signed with the field** as part of the same M1 change. The `[component]` section is therefore part of the **V0 schema definition**, not a post-freeze evolution.
2. **The increment surface's purpose is not engaged.** `schema_version`'s job (Decision Outcome, "Schema evolution") is to give a *future* runtime a branch point for an *older* manifest. Bumping to `2` now would imply a `schema_version: 1` population that this change must remain load-compatible with — but that population does not exist; every V0 manifest carries the field.

**Paired rule — mandatory field, enforced by the parser, not by the version number.** Because `schema_version` stays `1`, the version number **cannot** signal the `[component].hash` field's presence. The parser SHALL therefore **require the field directly**: a signed manifest body lacking `[component].hash` SHALL be **rejected fail-closed** at load — never loaded with the content-hash check skipped. This mandatory-field rule and the `schema_version: 1` disposition are **one decision**: holding the version at `1` is only safe *because* the parser enforces presence rather than inferring it from the version. (Behavioral enforcement: spec R-0021-c.)

### Scope of this amendment

This amendment adds **only** the `[component]` content-hash section to the signed body. It does **not** change `schema_version`, the host-fn ABI, the `[verbs]` / `[content_types]` / `[state_scopes]` / `[host_fns]` sections, the `[signature]` section, or any structural invariant in the table above. The custody / signing mechanism is unchanged ([P-0005](P-0005-v0-signing-chain.md)). The Tier-C signing-key custody hardening (`{{P-SigningKeyCustodyHardening}}`) remains deferred behind the R-0005-e multi-deployment trip-wire — out of scope here.

## More Information

- Frame open ADR slot: `{{P-PluginManifest}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table). This ADR resolves that slot.
- Depends on: [P-0001-storage-layout](P-0001-storage-layout.md) (C1 → universal `content.emit`); [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md) (4 `core: true` plugins whose manifests follow this schema).
- [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) — signing chain signs the manifest; `[signature]` section populated at build time; runtime verifies synchronously at load.
- [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) — `WorkspaceCtx` is the source of `workspace_id` on all host-fn write paths; host-fn signatures enforce the exclusion.
- [P-0007-plugin-resource-limits](P-0007-plugin-resource-limits.md) — per-instance fuel/memory limits apply to the Wasmtime instance executing the plugin; manifest is loaded before instance instantiation.
- [P-0008-admin-token-shape](P-0008-admin-token-shape.md) — admin token's `scopes` array feeds permission checks at the host layer; manifest-declared `verbs` are a separate surface (MCP-facing verb names, not token scopes).
- [P-0009-rls-admin-token](P-0009-rls-admin-token.md) — verb-to-scope mapping: which admin token scope grants access to which manifest-declared verb categories.
- Threat references: `P-plugin-instance`/E,T,I; `P-host-fns`/T,I,E; `DF-host-fn-call`/T,I; `DF-sampling-up`/T,I; `P-cli-handler`/E; `DS-pg-content`/T. ([Overview](../architecture/overview.md))
- Accepted risk `R-0007`: plugin sampling is unrestricted at V0 because all plugins are `core: true`; becomes Critical at V0.1+ third-party install.
- V0.1+ ABI follow-up: `@stable`/`@unstable` annotation per host-fn in the WIT interface definitions; stability-tier mark scenario from quality-attribute tree.
