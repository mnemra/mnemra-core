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

Every `core: true` plugin declares its identity, the content it owns, and the host functions it needs through a manifest. (Host functions, the host-fns, are the mediating calls the runtime exposes to a plugin.) The manifest is signed under [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md), one of this project's own architecture decision records (an [ADR](../glossary.md#adr) scoped to a single project, a [P-* ADR](../glossary.md#p--adr)). The plugin runtime loads it before any plugin code runs. The runtime compiles the declared host-fn surface into a per-instance allowlist. A call to anything outside that allowlist fails at the WIT (WebAssembly interface types) boundary.

Two axes needed resolution before this ADR could lock:

1. **Universal `content.emit` vs aspect-map-per-type.** Under [P-0001-storage-layout](P-0001-storage-layout.md), option C1 (the single-document layout), an artifact is a whole row. There's no per-aspect split. So the manifest's verb shape follows the single-document model: a universal `content.emit` over JSONB frontmatter and body, not an `aspect_map` per type. The other storage option, C2 (composite with typed slots), would have needed aspect maps. C1 removes that surface entirely.

2. **Host-fn ABI scope.** Which categories of host functions can a plugin invoke? The ABI (application binary interface) surface has to clear two bars at once. It must be wide enough to support the four `core: true` plugins named in [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md), and narrow enough that the signed-manifest allowlist actually limits what a plugin can do. Plugins are IO-free: every IO call is host-mediated, and the ABI names the categories that are available.

The manifest schema carries an explicit `schema_version` field so the format can evolve at V0.1 and later without breaking V0 plugins.

## Decision Drivers

- **C1 universal `content.emit`.** [P-0001-storage-layout](P-0001-storage-layout.md) locked the single-document layout. The manifest's verb shape follows from that: universal artifact CRUD (create, read, update, delete) over JSONB, not per-aspect operations.
- **Per-instance allowlist compiled from the signed manifest.** [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) requires the runtime to derive each instance's security properties from the signed manifest. The host-fn surface the manifest declares is the input to that allowlist compilation.
- **ABI forward-compat.** Pre-1.0 the ABI stays free to change. A quality-attribute scenario states the rule: any PR that changes the ABI must force all `core: true` plugins to recompile and pass their tests. The `schema_version` field makes that recompile surface visible. A V0.1 change that bumps the schema version produces a new manifest version, and an old V0 plugin can reject it cleanly.
- **Security threat coverage.** Threat `P-plugin-instance`/E (Critical) reads: "A plugin requests a host-fn outside its manifest's declared surface; if the host's permission check is at call-time and the plugin's manifest was loaded laxly, the plugin gains a capability it did not declare." The manifest is the structural fix. The allowlist is compiled into the per-instance binding, so a call outside it fails at the WIT boundary rather than inside a host-fn body.
- **`workspace_id` is never a plugin parameter on a write path.** Threat `P-host-fns`/T (Critical) governs this. The host derives `workspace_id` from the session or token in `WorkspaceCtx` ([P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md)), and the ABI MUST NOT accept `workspace_id` as a write-path parameter.
- **No direct database or network access from a plugin.** An IO-free plugin core is a Hard constraint. The host-fn surface names the exact mediating functions, and anything it doesn't name is structurally unavailable.

## Considered Options

1. **M1, universal `content.emit` with a typed host-fn allowlist (the V0 shape).** One manifest verb model. Host-fn categories are declared per plugin, and the allowlist is compiled per instance from the signed manifest.
2. **M2, aspect-map-per-type manifest.** Each plugin declares an `aspect_map` per content type, naming which aspects it owns (body, state, log, edges). It's richer, but it only means anything under C2. It doesn't fit C1.
3. **M3, capability-flag-only manifest (no host-fn granularity).** The manifest declares capability flags such as `content_read`, `content_write`, and `metrics_write`, with no per-function detail. Simpler, but coarser. It can't drive per-function allowlist compilation.

## Decision Outcome

**M1, universal `content.emit` with a typed host-fn allowlist.**

### Manifest schema (V0, schema_version: 1)

TOML format. Every field is required unless it's marked optional.

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
| `workspace_id` is NOT a parameter on any write-path host-fn | Host derives `workspace_id` from `WorkspaceCtx` (session/token); plugin cannot supply it | [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) + `P-host-fns`/T mitigation |
| Undeclared host-fn calls fail at WIT boundary | Per-instance allowlist compiled from signed manifest; failure is at binding, not at host-fn body | `P-plugin-instance`/E mitigation |
| `artifact.delete` requires explicit manifest declaration | Destructive op not included in default `required` surface; plugin must opt in | `P-cli-handler`/E mitigation pattern applied to plugin surface |
| `sampling.request` content-IDs only (no artifact bodies in prompt) | At V0 all plugins are `core: true` so surface is contained; V0.1+ third-party install escalates this to Critical per `R-0007` | `DF-sampling-up`/I mitigation |
| No direct DB access; no network access | Wasmtime sandbox + IO-free plugin core (Hard constraint) | Brief Hard constraints; architecture overview TB `TB-mnemra-host`↔`TB-plugin-sandbox` |
| `core: true` is the only valid value at V0 | Non-core plugin install is V0.1+ scope; runtime rejects `core: false` manifests at V0 | [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) + brief Hard constraints |

### Schema evolution (schema_version field)

`schema_version: 1` locks the V0 manifest format. Any V0.1 or later change that alters the manifest structure increments this field. The runtime branches on `schema_version` when it loads a manifest. A V0 plugin carrying `schema_version: 1` keeps loading unmodified against a V0.1 runtime. A V0.1 plugin carrying `schema_version: 2` against a V0 runtime gets a structured load error that names the `schema_version` mismatch.

This is the main forward-compat invariant behind the ABI-evolution-discipline quality-attribute scenario: "A plugin calling an `@unstable` host function emits a deprecation warning."

### Consequences

**Good:**
- `P-plugin-instance`/E (Critical) is mitigated. The per-instance host-fn allowlist is compiled from the signed manifest, so a call outside it fails structurally at the WIT boundary.
- The universal `content.emit` ABI is narrow. A typical work-verb plugin declares five content CRUD operations plus four observability operations, so nine required host-fn declarations in total. That's an auditable surface.
- The `workspace_id` write-path exclusion is structural. Host-fn signatures never take `workspace_id` as a parameter, and `WorkspaceCtx` is host-derived.
- `schema_version: 1` gives ABI evolution a clean break surface. The work is mechanical (recompile every `core: true` plugin) and well-bounded (a small set at V0).
- The signing chain in [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) signs the manifest. Verification is synchronous at load time. There's no async path.

**Bad / Trade-offs:**
- TOML is a tooling dependency. To produce the signed manifest, a plugin author needs a TOML serializer in the build toolchain. That ships in the standard library for Rust, and it's available for other compile-to-WASM languages.
- Per-function granularity in `host_fns.required` has a cost: adding a host function to a plugin means updating the manifest and re-signing it. At V0, with four `core: true` plugins and one signing authority, that's a controlled process. The cost grows with third-party plugin volume at V0.1 and later.
- `sampling.request` is an optional host-fn, which creates a two-tier plugin ABI. A plugin that declares it has a different trust surface from one that doesn't. The manifest states that difference, and the host's allowlist compilation enforces it structurally.

## Pros and Cons of the Options

### M1 — Universal `content.emit` + typed host-fn allowlist (accepted)

- Pro: it carries C1's single-document simplicity straight into the ABI, with no aspect-map complexity.
- Pro: per-function allowlist compilation is the structural mitigation for `P-plugin-instance`/E (Critical).
- Pro: `schema_version: 1` gives ABI evolution a clean break surface.
- Con: it adds a TOML and signing-toolchain dependency to the plugin build pipeline.

### M2 — Aspect-map-per-type manifest

- Pro: rich per-aspect access control is possible, granting body, state, log, and edges as separate capabilities.
- Con: it only means anything under the C2 layout. It doesn't fit the C1 single-document model.
- Con: a much richer manifest surface, with no third-party plugins at V0 to validate it.

### M3 — Capability-flag-only manifest

- Pro: a simpler manifest format, with fewer fields to declare.
- Con: the allowlist is coarse-grained, at the category level rather than per function, so the `P-plugin-instance`/E mitigation is weaker.
- Con: it can't treat `sampling.request` as an optional per-plugin flag without growing the flag vocabulary toward M1's complexity anyway.

## Amendment 2026-06-30 — Component content-hash field (supply-chain binding)

The signing-to-runnable Frame ([`docs/intent/signing-to-runnable-frame.md`](../../intent/signing-to-runnable-frame.md), §5), the constraint-shaping stage that precedes a spec ([Frame](../glossary.md#frame)), routed the **content-hash-of-the-component-bytes** field to a P-0003 amendment (the `{{P-ManifestContentHash}}` slot) instead of a spec-local field. Two reasons. The field is a **durable part of the manifest contract every `core: true` plugin must satisfy**: it's recomputed and enforced on **every** plugin load by the verified-load gate (`libs/mnemra-host/startup/pool_population.rs:126`). And it's a **manifest-schema change**, a new key in the signed canonical body, for which P-0003 is the **sole authority**. A spec-local field would orphan a schema field from its schema authority. A future plugin author reading P-0003 wouldn't see the content-hash requirement, and a later manifest-schema change could silently drop it.

The **runtime behavior** (recompute and reject on mismatch, fail closed, single-read load, a distinct error variant) is governed by **R-0021**, a numbered requirement ([R-codes](../glossary.md#r-codes)), in the project's [spec](../glossary.md#spec) document [`docs/specs/2026-06-30-signing-to-runnable.md`](../../specs/2026-06-30-signing-to-runnable.md). This amendment governs the **schema** itself: the field, where it lives, its algorithm constraint, and its presence rule.

### Schema change — `[component]` section in the signed canonical body

The V0 manifest schema gains a `[component]` section that carries a content-hash of the component (`.wasm`) bytes. It sits in the **signed canonical body**, the bytes **before** the `\n[signature]` marker that the signature covers (the slice extracted at `libs/mnemra-host/signing/verify.rs:337-344`). [P-0005](P-0005-v0-signing-chain.md) R-0005-h (core-by-provenance) **forces** that location. A content-hash in the unsigned region next to `[signature]` would let an attacker swap both the component and its declared hash.

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

**Named algorithm (the banned-weak constraint).** The content-hash algorithm SHALL be **SHA-256 or stronger, or BLAKE3**. `MD5` and `SHA-1` are **banned** and SHALL be rejected at load. The V0 locked value is **BLAKE3**, reusing the in-tree BLAKE3 primitive already required for admin-token hashing ([P-0008](P-0008-admin-token-shape.md) / R-0008-b), so no new dependency (Simplicity, `P-StackDiscipline`). The strong set is the schema's **forward-allowance**. At V0 the only value any manifest carries is `blake3`. The schema accepts `sha256`, `sha384`, and `sha512`, but no V0 manifest exercises them.

**Binds bytes, not a path.** The `[component]` section binds the component's **bytes** through `hash`. It carries **no path or name field**. The runtime works out which component to load by other means (there's a single `core: true` plugin at V0). A manifest-declared component path or name is a multi-plugin concern that's out of scope here, not part of this schema change.

**Read only from the signed slice (complete mediation).** The `[component]` table SHALL be parsed **only from the signed canonical body**, the bytes the signature covers before the `\n[signature]` marker (again, the slice extracted at `libs/mnemra-host/signing/verify.rs:337-344`). A `[component]` table that appears in the unsigned region next to `[signature]` SHALL NOT satisfy the field-presence requirement and SHALL cause rejection. The enforced value is always the signed value, never a copy from the unsigned region. (Behavioral enforcement lives in spec R-0021-c and R-0021-e.)

### `schema_version` stays `1` — and the mandatory-field rule (one decision)

The "Schema evolution" section above states: *"`schema_version: 1` locks the V0 manifest format. **V0.1+ changes that alter the manifest structure increment this field.**"* Adding a `[component]` section **does** alter the manifest structure, so read literally, that rule would make the version increment. It doesn't. The reasons are recorded here so the ADR isn't silently self-contradictory.

1. **V0 is still being *defined*, not evolved after a freeze.** No external plugin has shipped against a frozen V0 schema. The increment rule governs **V0.1+** changes, the ones that alter the format *after* V0 is fixed, so that a third party loading an older V0 plugin can detect the mismatch. No such older population exists. The single `core: true` plugin (`mnemra-echo`) is **re-signed with the field** as part of the same M1 change. So the `[component]` section is part of the **V0 schema definition**, not a post-freeze evolution.
2. **The reason to increment isn't in play.** The job of `schema_version` (see Decision Outcome, "Schema evolution") is to give a *future* runtime a branch point for an *older* manifest. Bumping it to `2` now would imply there's a `schema_version: 1` population this change has to stay load-compatible with. That population doesn't exist. Every V0 manifest carries the field.

**Paired rule: the field is mandatory, enforced by the parser rather than by the version number.** Because `schema_version` stays `1`, the version number **can't** signal whether `[component].hash` is present. So the parser SHALL **require the field directly**. A signed manifest body that lacks `[component].hash` SHALL be **rejected fail-closed** at load, never loaded with the content-hash check skipped. This mandatory-field rule and the `schema_version: 1` decision are **one decision**. Holding the version at `1` is only safe *because* the parser enforces presence instead of inferring it from the version. (Behavioral enforcement lives in spec R-0021-c.)

### Scope of this amendment

This amendment adds **only** the `[component]` content-hash section to the signed body. It does **not** change `schema_version`, the host-fn ABI, the `[verbs]`, `[content_types]`, `[state_scopes]`, or `[host_fns]` sections, the `[signature]` section, or any structural invariant in the table above. The custody and signing mechanism is unchanged ([P-0005](P-0005-v0-signing-chain.md)). The Tier-C signing-key custody hardening (`{{P-SigningKeyCustodyHardening}}`) stays deferred behind the R-0005-e multi-deployment trip-wire, and it's out of scope here.

## Amendment 2026-07-07 — `[[artifacts]]` N≥1 binding (plugin distribution; supersedes the `[component]` section)

The plugin-distribution design (W2-1: [P-0023](P-0023-plugin-distribution.md); spec [`docs/specs/2026-07-07-plugin-distribution.md`](../../specs/2026-07-07-plugin-distribution.md), R-0086) extends this manifest's supply-chain binding from a single component hash to **all N≥1 bundle artifacts**. That follows the maintainer's Decision A (2026-07-01, locked): the inner signed manifest binds every blob through an `[[artifacts]]`-style N≥1 list, and each entry is verified with R-0021's single-read, complete-mediation, fail-closed discipline at the provenance anchor. As with the 2026-06-30 amendment, this one governs the **schema**. The runtime behavior (the complete-mediation loop, the dual-digest single read, the designed rejections) is governed by the distribution spec (R-0086 and R-0087).

### Schema change — `[[artifacts]]` replaces `[component]` in the signed canonical body

The signed canonical body (the bytes before the `\n[signature]` marker, the slice produced by the canonical-body splitter in `libs/mnemra-host/signing/verify.rs`) gains an **`[[artifacts]]` array of tables, N≥1, with entry #1 being the component `.wasm`**, and **loses the `[component]` section**, which this amendment **supersedes**. The full detail is locked in spec **R-0086-a and R-0086-b**, single-sourced there rather than restated here: the field-by-field schema, the `id` grammar (`^[a-z0-9][a-z0-9._-]{0,63}$`, which makes path traversal unconstructible by construction), the `hash_alg` constraint (**`blake3` is the only value accepted at V0**; `sha256`, `sha384`, and `sha512` are reserved vocabulary for a future amendment and rejected on sight as `hash_alg_unsupported`, alongside the banned-weak `md5` and `sha1`, so no non-blake3 verification path is reachable at V0), the mandatory lowercase-hex `hash`, and the `media_type` cross-anchor consistency rule.

Two rules from the 2026-06-30 amendment are explicitly superseded or carried forward:

1. **"Binds bytes, not a path": the earlier scope-out is superseded.** The 2026-06-30 amendment carried no per-artifact identifier because the single `[component]` section needed none. Under an N≥1 binding, a per-artifact **`id`** is now necessary for unambiguous, collision-free resolution of each entry to its blob (the outer OCI layer joins on the `vnd.mnemra.artifact.id` annotation, ratified 2026-07-07). Per **R-0005-h (core-by-provenance)**, every per-artifact identifier lives **in the signed canonical body**. An identifier in the unsigned region would let an attacker swap both an artifact and its declared identity. The binding still binds *bytes*, through `hash`. The `id` names the binding; it doesn't weaken it to a path reference.
2. **Read only from the signed slice, carried unchanged.** The `[[artifacts]]` array SHALL be parsed only from the signed canonical body. An array in the unsigned region next to `[signature]` SHALL NOT satisfy presence and SHALL cause rejection (spec R-0086-d).

### `schema_version` stays `1` — the same one-decision rule, re-applied

The 2026-06-30 reasoning applies verbatim. V0 is still being *defined*: the `core: true` set is re-signed with `[[artifacts]]` in the same hard-cutover change ([P-0023](P-0023-plugin-distribution.md) D5), and no older population exists to branch on. So the version doesn't increment, **and the parser enforces presence directly**. A signed body that lacks `[[artifacts]]`, or carries zero entries, is rejected fail-closed (spec R-0086-d). **Strict supersession (maintainer-ratified 2026-07-07):** after the cutover, a signed body that still carries a `[component]` section is **rejected** (`legacy_manifest_rejected`, spec R-0086-e), not ignored, so no manifest can carry two binding representations that might disagree. Holding the version at `1` is only safe because the parser enforces both the presence of the new table and the absence of the old one.

### Scope of this amendment

This amendment changes **only** the supply-chain binding section of the signed body (`[component]` becomes `[[artifacts]]`). The host-fn ABI, the `[verbs]`, `[content_types]`, `[state_scopes]`, and `[host_fns]` sections, `[signature]`, `schema_version`, and every structural invariant in the V0 table above are unchanged. The custody and signing mechanism is unchanged ([P-0005](P-0005-v0-signing-chain.md)). The distribution-layer packaging, the package signature, and the fetch pipeline are [P-0023](P-0023-plugin-distribution.md)'s domain, not this schema's. The 2026-06-30 amendment text above stays as the historical record of the single-component binding it defined (amend, don't erase). Its schema is superseded by this section as of the hard cutover.

## More Information

- Frame open-ADR slot: `{{P-PluginManifest}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table). This ADR resolves that slot.
- Depends on [P-0001-storage-layout](P-0001-storage-layout.md) (C1 gives the universal `content.emit`) and [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md) (the four `core: true` plugins whose manifests follow this schema).
- [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md): the signing chain signs the manifest, the `[signature]` section is populated at build time, and the runtime verifies it synchronously at load.
- [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md): `WorkspaceCtx` is the source of `workspace_id` on every host-fn write path, and the host-fn signatures enforce the exclusion.
- [P-0007-plugin-resource-limits](P-0007-plugin-resource-limits.md): per-instance fuel and memory limits apply to the Wasmtime instance running the plugin, and the manifest is loaded before that instance is created.
- [P-0008-admin-token-shape](P-0008-admin-token-shape.md): the admin token's `scopes` array feeds permission checks at the host layer. The manifest-declared `verbs` are a separate surface, the MCP-facing verb names, not token scopes.
- [P-0009-rls-admin-token](P-0009-rls-admin-token.md): the verb-to-scope mapping, which admin-token scope grants access to which manifest-declared verb categories.
- Threat references: `P-plugin-instance`/E,T,I; `P-host-fns`/T,I,E; `DF-host-fn-call`/T,I; `DF-sampling-up`/T,I; `P-cli-handler`/E; `DS-pg-content`/T. ([Overview](../architecture/overview.md))
- Accepted risk `R-0007`: plugin sampling is unrestricted at V0 because every plugin is `core: true`. It becomes Critical once V0.1 and later allow third-party install.
- V0.1+ ABI follow-up: a `@stable` or `@unstable` annotation per host-fn in the WIT interface definitions, the stability-tier mark scenario from the quality-attribute tree.
