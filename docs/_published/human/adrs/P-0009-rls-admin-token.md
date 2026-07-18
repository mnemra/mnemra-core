---
title: "P-0009: RLS Role Model and Admin Token Permission Shape"
summary: "V0 role enum (admin / read-observer), permission matrix per role for MCP verb categories and CLI control-plane operations, application-layer enforcement at V0 with RLS-policy hardening path at V0.1+."
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

# P-0009: RLS Role Model and Admin Token Permission Shape

## Status

`accepted`

## Context and Problem Statement

[P-0008-admin-token-shape](P-0008-admin-token-shape.md) locked the admin token structure: an opaque 32-byte random value, BLAKE3-hashed in the `admin_tokens` table, with the workspace claim coming from a server-side row lookup. That ADR (Architecture Decision Record: a document capturing a decision, its context, the alternatives it rejected, and its consequences) deferred the role model and permission shape to this slot, noting: "The role model downstream can add scopes and permission shapes, but the workspace claim anchor is [P-0008's] decision."

This ADR locks what the `scopes` array in the `admin_tokens` table means: the role enum, the permission matrix per role across MCP (Model Context Protocol) verb categories and CLI control-plane operations, and how the V0 application-layer enforcement (per [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md)) maps roles to `WorkspaceCtx` population.

The decision also depends on [P-0001-storage-layout](P-0001-storage-layout.md), because the policy-surface count under C1 is small and auditable: about 10 artifact tables, one RLS (Postgres row-level security) policy per table. Under C2 (composite-with-typed-slots), the policy surface would have been per-aspect, producing a much richer authorization matrix.

The brief sets hard constraints: multi-tenancy is structural, with `workspace_id` NOT NULL on every artifact table. The RLS column shape ships at V0; policy enforcement follows at V0.1+. [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) named the V0 enforcement mechanism: typed `WorkspaceCtx` parameter binding at the host-fn boundary. This ADR names the role model the `WorkspaceCtx` encoding carries.

## Decision Drivers

- **V0 scope: application-layer enforcement only.** RLS policy enforcement is deferred to V0.1+, per accepted risk `R-0001` (a formally accepted risk logged in the architecture overview, not an open bug). The role model must be implementable entirely at the application layer (host-fn boundary enforcement, mandatory WHERE-clause discipline) without any Postgres RLS policies active. The V0.1 migration path hardens the same role model at the substrate layer, so the role enum and permission matrix have to stay stable across that migration.
- **Policy-surface count under C1.** Under the C1 single-document layout (about 10 artifact tables), the RLS policy surface per role stays manageable: one policy per (role, table) pair. The role enum must not blow up this surface.
- **Admin token `scopes` array.** [P-0008-admin-token-shape](P-0008-admin-token-shape.md) defines `scopes TEXT[] NOT NULL` in `admin_tokens`. This ADR defines the valid scope strings and what each one authorizes.
- **Threat coverage.** Multiple `{{P-RLSAdminToken}}` threat entries in the architecture overview address workspace-lifecycle operations behind admin scope (`P-builtin-workspaces`/E), workspace-claim extraction defaulting to the `default` workspace (`P-builtin-auth`/E), per-token attribution on writes (`EE-orchestrator-agent`/R), and token-file inode pinning with modification fail-shut (`DS-admin-token`/T).
- **V0 is single-operator, single-workspace dogfood.** The role model has to support V0 without over-engineering. A minimal role set (admin plus read-observer) covers the V0 use cases; additional roles (e.g., per-plugin scope grants, per-team-member roles) are V0.1+ work.
- **Admin scope is distinct from user scope.** Workspace lifecycle ops (create, delete) require a scope that ordinary verb-invoking tokens don't carry. The admin scope is a strict superset of user scope, distinguished by claim.

## Considered Options

1. **R1 (Binary admin/read-observer roles with scope strings).** Two roles. Admin scope authorizes all verbs, including control-plane operations; read-observer scope authorizes read-only MCP verbs only. Application-layer enforcement at V0, RLS policy hardening at V0.1+.
2. **R2 (Per-plugin scoped roles, fine-grained).** One scope string per plugin (e.g., `plugin:tasks:write`, `plugin:repos:read`). Maximum granularity, but the complexity is deferred until multi-plugin multi-tenant access control is actually needed.
3. **R3 (Single admin-only role, no read-observer).** One role, full access. Simplest option, but it gives no read-vs-write separation for auditing or future delegation.

## Decision Outcome

**R1: Binary admin/read-observer roles with scope strings.**

### Role enum (V0)

| Role | Scope string | Description |
|---|---|---|
| `admin` | `"admin"` | Full access: all MCP verbs across all `core: true` plugin families; all CLI control-plane operations (workspace lifecycle, token rotation, migration trigger, backup trigger); all admin-session management. |
| `read_observer` | `"read_observer"` | Read-only access: MCP read verbs (`*.get`, `*.list`, projection queries); no write verbs; no CLI control-plane access. Intended for monitoring agents and read-only consumers. |

The V0 `admin_tokens` row for the deploying operator carries `scopes = ['admin']`. A read-observer token (if issued) carries `scopes = ['read_observer']`.

### Permission matrix

Enforcement at V0 happens at the application layer (host-fn boundary, per [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md)). The matrix below defines what each role authorizes. At V0.1+, these same categories map onto Postgres RLS policies, one per artifact table per role.

| Verb category | `admin` | `read_observer` | Notes |
|---|---|---|---|
| MCP content read (`artifact.get`, `artifact.list`, projection queries) | Allowed | Allowed | Both roles scoped to workspace_id from token row |
| MCP content write (`artifact.create`, `artifact.update`, `artifact.delete`) | Allowed | Denied | Write path denied for read_observer at host-fn boundary |
| MCP metrics / event / log read | Allowed | Allowed | Observability read is safe for read_observer |
| MCP metrics / event / log write (plugin-internal; not exposed as direct MCP verbs) | Allowed (via plugin host-fn) | N/A | Not a direct MCP verb; plugin-internal write path |
| Plugin verb dispatch (routing MCP requests to `core: true` plugin handlers) | Allowed | Allowed for read verbs only | Plugin routing checks manifest-declared verb list against role |
| CLI workspace lifecycle (`workspace create`, `workspace delete`) | Allowed | Denied | Admin scope only; workspace-delete is irreversible |
| CLI token rotation (`token rotate`) | Allowed | Denied | Self-service rotation; produces audit event in `DS-ts-events` |
| CLI migration trigger (`migrate`) | Allowed | Denied | One-shot destructive control-plane op |
| CLI backup trigger (`backup`) | Allowed | Denied | Reads full substrate; encryption-at-rest per `{{P-BackupRestore}}` |
| Admin session management (creating/invalidating tokens) | Allowed | Denied | `P-builtin-auth` only; admin scope required |

### WorkspaceCtx encoding

The `WorkspaceCtx` constructed per [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) carries:

```rust
pub struct WorkspaceCtx {
    pub workspace_id: Uuid,     // from admin_tokens.workspace_id; NOT NULL
    pub role:         Role,     // derived from admin_tokens.scopes
    pub token_id:     Uuid,     // for audit attribution (EE-orchestrator-agent/R mitigation)
}

pub enum Role {
    Admin,
    ReadObserver,
}
```

`token_id` is included for audit attribution: every host-fn write call carries `(workspace_id, role, token_id)` in its context, enabling post-hoc attribution of destructive writes to a specific token. This addresses `EE-orchestrator-agent`/R (High, 80): "without per-token attribution on every write, the audit trail names the workspace but not the issuing agent."

### Admin scope constraints (structural invariants)

These aren't configuration. They're structural invariants enforced at the host-fn boundary.

| Invariant | Enforcement | Threat |
|---|---|---|
| Workspace-lifecycle ops (`workspace create`, `workspace delete`) require `Role::Admin` | Host-fn body checks role before executing; `Role::ReadObserver` returns a structured permission error | `P-builtin-workspaces`/E (Critical, 75) |
| Workspace claim is mandatory; absence → hard auth failure | `admin_tokens.workspace_id` is NOT NULL; DB row lookup returns the claim; absence means the token row doesn't exist → reject | `P-builtin-auth`/E (Critical, 80) |
| Token-file inode pinning at startup | Host checks `DS-admin-token` file mode = 600, not world-readable; mismatch → fail-shut | `DS-admin-token`/T (High, 70); `EE-operator`/S partial mitigation |
| Token rotation logs to `DS-ts-events` | CLI `token rotate` is a structured operation; event emitted before old row is deleted | `DS-admin-token`/R (Low, 60) |
| Old-key tokens invalidated on rotation | Rotation deletes the old `admin_tokens` row; subsequent lookups against the old hash produce "not found" → reject | `P-builtin-auth`/S (High, 70): key rotation discipline |

### V0 → V0.1+ migration path for RLS policy hardening

At V0, the permission matrix above is enforced only at the application layer: host-fn boundary checks via `WorkspaceCtx.role`, plus mandatory WHERE-clause discipline on every read path. The RLS column shape (`workspace_id NOT NULL` on every artifact table) ships at V0.

At V0.1+, the same role model hardens at the substrate layer:

```sql
-- One policy per (role, artifact table) pair; ~20 policies for the V0 artifact set
CREATE POLICY tasks_admin_read ON tasks
  FOR SELECT USING (workspace_id = current_setting('mnemra.workspace_id')::uuid);

CREATE POLICY tasks_admin_write ON tasks
  FOR ALL USING (workspace_id = current_setting('mnemra.workspace_id')::uuid)
      WITH CHECK (workspace_id = current_setting('mnemra.workspace_id')::uuid);

CREATE POLICY tasks_read_observer ON tasks
  FOR SELECT USING (
    workspace_id = current_setting('mnemra.workspace_id')::uuid
    AND current_setting('mnemra.role') = 'read_observer'
  );
```

The `mnemra.workspace_id` and `mnemra.role` session settings are set by the host at the request boundary (via `SET LOCAL`), using the same `WorkspaceCtx` values. The application-layer and substrate-layer policies are structurally identical. V0.1 activation is additive: no schema migration is required.

The trip-wire for V0.1 activation is accepted risk `R-0001`: "first deployment serving more than one workspace (production multi-tenant traffic), OR the column-shape's lint coverage drops below 100% on read paths, OR a third-party plugin is loaded at runtime."

### Consequences

**Good:**
- `P-builtin-auth`/E (Critical): the workspace claim is DB-row-sourced and NOT NULL enforced, so the default-to-`default`-workspace bug can't happen.
- `P-builtin-workspaces`/E (Critical): workspace lifecycle ops gate on `Role::Admin` at the host-fn boundary; `Role::ReadObserver` tokens can't create or delete workspaces.
- `EE-orchestrator-agent`/R (High): `token_id` in `WorkspaceCtx` enables per-token write attribution; the audit trail names the token, not just the workspace.
- `DS-admin-token`/R (Low): token rotation is a structured CLI verb logged to `DS-ts-events`; rotation events stay auditable after the fact.
- V0 → V0.1 RLS hardening is additive: `CREATE POLICY` statements only, no schema migration, no backfill.
- Binary role enum keeps the policy surface manageable: about 20 Postgres RLS policies at V0.1 (2 roles × ~10 artifact tables), each straightforward.

**Bad / Trade-offs:**
- Binary role enum (admin / read_observer) gives no per-plugin granularity. An `admin` token can invoke verbs across all `core: true` plugin families. That's fine at V0 dogfood (single operator), but it'll need extension at V0.1+ once multi-tenant onboarding requires per-plugin scoping.
- `read_observer` has no write access at all, not even to its own session metadata. That may be too restrictive for some V0.1+ monitoring use cases; scope extension would be an ADR amendment.
- Token rotation doesn't currently invalidate in-flight requests that cached the old token. At V0, with no in-process cache (per [P-0008-admin-token-shape](P-0008-admin-token-shape.md)), this isn't a problem: each request does a fresh DB lookup, so rotation takes effect immediately on the next request.

## Pros and Cons of the Options

### R1 — Binary admin/read-observer + scope strings (accepted)

- Pro: V0.1 RLS hardening is additive; the same role model works at both the application and substrate layer.
- Pro: `WorkspaceCtx.token_id` adds write attribution without extra complexity.
- Pro: the binary role keeps the policy surface to about 20 policies at V0.1.
- Con: no per-plugin granularity at V0; admin scope grants access to all plugin families.

### R2 — Per-plugin scoped roles

- Pro: fine-grained access control; per-plugin delegation is possible.
- Con: scope string vocabulary explosion; `scopes TEXT[]` would carry up to about 10 values per token.
- Con: the RLS policy surface explodes: roles × plugins × artifact types, instead of roles × artifact tables.
- Con: no V0 use case drives this complexity; defer it to V0.1+.

### R3 — Single admin-only role

- Pro: simplest implementation.
- Con: no read-vs-write separation; monitoring agents and read-only consumers get full admin access.
- Con: doesn't address `EE-specialist-agent`/S (Medium, 70): sub-agent identity separation needs at minimum a read-scoped token.

## More Information

- Frame open ADR slot: `{{P-RLSAdminToken}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table). (Frame is Stage 2 of the intent-to-spec pipeline: agents synthesize operating constraints from validated intent into a frame document.) This ADR resolves that slot.
- Depends on: [P-0001-storage-layout](P-0001-storage-layout.md) (C1 → about 10 artifact tables → a manageable RLS policy surface); [P-0008-admin-token-shape](P-0008-admin-token-shape.md) (opaque token with `scopes TEXT[]`; this ADR defines the valid scope strings).
- [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md): `WorkspaceCtx` is the runtime carrier of `(workspace_id, role, token_id)` at V0; this ADR defines what `role` means.
- [P-0003-plugin-manifest](P-0003-plugin-manifest.md): verb-to-scope mapping. The plugin manifest declares exposed MCP verbs; host routing checks role against verb before dispatching to the plugin.
- Threat references: `EE-orchestrator-agent`/S,R; `EE-specialist-agent`/S; `EE-operator`/S,R; `P-mcp-handler`/S,I; `P-cli-handler`/S,E; `P-builtin-auth`/S,E; `P-builtin-workspaces`/E; `P-builtin-projects`/I; `P-builtin-agents`/R; `DS-admin-token`/I,T,R; `DS-pg-content`/I; `DS-ts-metrics`/I; `DF-host-fn-call`/I. ([Overview](../architecture/overview.md))
- Accepted risk `R-0001` ([Overview](../architecture/overview.md)): RLS policy enforcement is deferred to V0.1+; the V0.1 activation trip-wire is named above.
- Accepted risk `R-0002` ([Overview](../architecture/overview.md)): external-AS integration is deferred; the static admin token is the V0 auth path.
- Accepted risk `R-0006` ([Overview](../architecture/overview.md)): operator-action repudiation is partially mitigated at V0; the activity log at `0.5.0` is the fuller mitigation.
- Follow-up: per-plugin scope extension for V0.1+ multi-tenant onboarding; per-agent token derivation for `EE-specialist-agent`/S mitigation.

## Amendment 2026-07-17 — Ingest control-plane permission-matrix rows (`{{P-0009-A1-ingest-control-plane}}`)

The ingestion-pipeline Frame ([`docs/intent/ingestion-pipeline-frame.md`](../../intent/ingestion-pipeline-frame.md), blob `f56b3685`, IP-1) places source registration and quarantine inspection as **admin CLI control-plane operations** in this ADR's admin family, alongside the workspace-lifecycle, migrate, and backup siblings. But the permission matrix above **doesn't yet enumerate** them. Rather than stretch the existing rows to imply coverage the matrix text doesn't actually carry (the Frame's honesty note), this amendment **adds their rows explicitly**. Binding requirement text is single-sourced to the ingestion-pipeline spec, requirement **R-0099** ([`docs/specs/2026-07-16-ingestion-pipeline.md`](../../specs/2026-07-16-ingestion-pipeline.md); the registration-input allow-list validation rides at R-0099-d). (Spec is Stage 3 of the intent-to-spec pipeline, the locked, testable contract verification consumes. R-0099 is an R-code: a stable requirement identifier whose full text lives in that spec document, not restated here.) The shape itself is rendered in [P-0024](P-0024-ingest-pipeline-shape.md) IPS-1. This amendment governs the permission matrix, meaning which role may invoke each operation; the operations' own behavior is governed by that spec.

### Added permission-matrix rows

These add to the Permission matrix under Decision Outcome above, with the same enforcement discipline as the existing control-plane rows: application-layer at V0 per [P-0006](P-0006-v0-tenant-enforcement.md), RLS-hardening path at V0.1+ unchanged.

| Verb category | `admin` | `read_observer` | Notes |
|---|---|---|---|
| CLI ingest source registration (`source register`, `source retire`) | Allowed | Denied | Admin scope only. Registration is the admission-control boundary: registering a root authorizes write-through from it under the attested `trust_class`. Input is validated against allow-lists (`trust_class` ∈ enum, `retention_policy` ∈ enum, `source_kind` ∈ enum, root canonicalized, must be a directory, and within an allowed prefix, per spec R-0099-d), not mere well-formedness. Audited like the `migrate`/`backup` siblings. |
| CLI ingest source listing (`source list`) | Allowed | Denied | Admin scope only, consistent with the other control-plane listings; no read-observer control-plane access. |
| CLI quarantine inspection (`quarantine list`, `quarantine inspect`) | Allowed | Denied | Admin scope only. **Metadata-only** (path, `rejection_class`, diagnostic; never a payload render; per spec R-0112-d). Payload retrieval, if ever offered, is an explicit operator action that re-enters the sandboxed extraction path. Audited. |

The ingest write path itself runs under a distinguished **system principal** (the [P-0015](P-0015-provenance-envelope-source-roles.md) PE-3 pattern; the [P-0018](P-0018-core-entity-manifest.md) `system` actor), **not** an admin token and **not** a workspace role. The system principal can't register or retire sources, and it can't touch these admin control-plane operations either (spec R-0106-b/R-0107-a). The rows above govern the **operator** control-plane surface; the standing service's authority is separately least-scoped.

### Scope of this amendment

This amendment adds **only** the three ingest control-plane rows above to the permission matrix. It does **not** change the role enum (`admin` / `read_observer` stay as they were), the `WorkspaceCtx` encoding, the admin-scope structural invariants, the V0→V0.1 RLS hardening path, or any existing matrix row. The V0.1 RLS activation trip-wire (accepted risk `R-0001`) is unchanged. This amendment rides the ingestion-pipeline spec-exit gate (the human review checkpoint at the end of Stage 3, before the spec locks) for maintainer ratification.
