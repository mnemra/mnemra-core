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

This is a P-* ADR (a project-scoped architecture decision record; the P stands for project). [P-0008-admin-token-shape](P-0008-admin-token-shape.md) locked the admin token structure: an opaque 32-byte random value, BLAKE3-hashed in the `admin_tokens` table, with the workspace claim coming from a server-side row lookup. That ADR deferred the role model and permission shape to this slot, noting: "The role model downstream can add scopes and permission shapes, but the workspace claim anchor is [P-0008's] decision."

This ADR locks what the `scopes` array in the `admin_tokens` table means. That covers three things: the role enum, the permission matrix per role across MCP verb categories and CLI control-plane operations, and how the V0 application-layer enforcement (per [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md)) maps roles to `WorkspaceCtx` population.

The decision also depends on [P-0001-storage-layout](P-0001-storage-layout.md). The reason is that the policy-surface count under C1 is small and auditable (roughly 10 artifact tables, one RLS policy per table). Under C2 (composite-with-typed-slots) the policy surface would have been per-aspect, producing a much richer authorization matrix.

The brief sets hard constraints. Multi-tenancy is structural, with `workspace_id` NOT NULL on every artifact table. The RLS column shape ships at V0, and policy enforcement lands at V0.1+. [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) named the V0 enforcement mechanism (typed `WorkspaceCtx` parameter binding at the host-fn boundary). This ADR names the role model that the `WorkspaceCtx` encoding carries.

## Decision Drivers

- **V0 scope: application-layer enforcement only.** RLS policy enforcement is deferred to V0.1+ per accepted risk `R-0001`. The role model has to be implementable entirely at the application layer (host-fn boundary enforcement, plus mandatory WHERE-clause discipline) without Postgres RLS policies active. The V0.1 migration path hardens the same role model at the substrate layer, so the role enum and permission matrix have to stay stable across that migration.
- **Policy-surface count under C1.** Under C1 single-document layout (roughly 10 artifact tables), the RLS policy surface per role stays manageable: one policy per (role, table) pair. The role enum must not explode this surface.
- **Admin token `scopes` array.** [P-0008-admin-token-shape](P-0008-admin-token-shape.md) defines `scopes TEXT[] NOT NULL` in `admin_tokens`. This ADR defines the valid scope strings and what they authorize.
- **Threat coverage.** Multiple `{{P-RLSAdminToken}}` threat entries in the architecture overview are addressed here: workspace-lifecycle ops behind admin scope (`P-builtin-workspaces`/E); workspace-claim extraction defaulting to the `default` workspace (`P-builtin-auth`/E); per-token attribution on writes (`EE-orchestrator-agent`/R); and token-file inode pinning with fail-shut on modification (`DS-admin-token`/T).
- **V0 is single-operator, single-workspace dogfood.** The role model has to support V0 without over-engineering. A minimal role set (admin plus read-observer) covers the V0 use cases. Additional roles, such as per-plugin scope grants or per-team-member roles, are V0.1+ work.
- **Admin scope distinct from user scope.** Workspace lifecycle ops (create, delete) need a scope that ordinary verb-invoking tokens don't carry. The admin scope is a strict superset of user scope, distinguished by claim.

## Considered Options

1. **R1 — Binary admin/read-observer roles + scope strings** — two roles; admin scope authorizes all verbs including control-plane; read-observer scope authorizes read-only MCP verbs. Application-layer enforcement at V0; RLS policy hardening at V0.1+.
2. **R2 — Per-plugin scoped roles (fine-grained)** — one scope string per plugin (e.g., `plugin:tasks:write`, `plugin:repos:read`). Maximum granularity; complexity deferred to when multi-plugin multi-tenant access control is needed.
3. **R3 — Single admin-only role (no read-observer)** — one role, full access. Simplest; provides no read-vs-write separation for auditing or future delegation.

## Decision Outcome

**R1 — Binary admin/read-observer roles with scope strings.**

### Role enum (V0)

| Role | Scope string | Description |
|---|---|---|
| `admin` | `"admin"` | Full access: all MCP verbs across all `core: true` plugin families; all CLI control-plane operations (workspace lifecycle, token rotation, migration trigger, backup trigger); all admin-session management. |
| `read_observer` | `"read_observer"` | Read-only access: MCP read verbs (`*.get`, `*.list`, projection queries); no write verbs; no CLI control-plane access. Intended for monitoring agents and read-only consumers. |

The V0 `admin_tokens` row for the deploying operator carries `scopes = ['admin']`. A read-observer token (if issued) carries `scopes = ['read_observer']`.

### Permission matrix

Enforcement at V0 is application-layer (the host-fn boundary, per [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md)). The matrix below defines what each role authorizes. At V0.1+, these same categories map to Postgres RLS policies (one per artifact table per role).

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

`token_id` is included for audit attribution. Every host-fn write call carries `(workspace_id, role, token_id)` in its context, which lets the system attribute a destructive write back to a specific token after the fact. This addresses `EE-orchestrator-agent`/R (High, 80): "without per-token attribution on every write, the audit trail names the workspace but not the issuing agent."

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

At V0, the permission matrix above is enforced at the application layer only: host-fn boundary checks via `WorkspaceCtx.role`, plus mandatory WHERE-clause discipline on every read path. The RLS column shape (`workspace_id NOT NULL` on every artifact table) ships at V0.

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

The `mnemra.workspace_id` and `mnemra.role` session settings are set by the host at the request boundary (via `SET LOCAL`) using the same `WorkspaceCtx` values. The application-layer and substrate-layer policies are structurally identical. V0.1 activation is additive, so no schema migration is required.

The trip-wire for V0.1 activation is accepted risk `R-0001`: "first deployment serving more than one workspace (production multi-tenant traffic), OR the column-shape's lint coverage drops below 100% on read paths, OR a third-party plugin is loaded at runtime."

### Consequences

**Good:**
- `P-builtin-auth`/E (Critical): the workspace claim is sourced from a DB row and NOT NULL enforced, so the default-to-default-workspace bug can't arise.
- `P-builtin-workspaces`/E (Critical): workspace lifecycle ops gate on `Role::Admin` at the host-fn boundary; `Role::ReadObserver` tokens can't create or delete workspaces.
- `EE-orchestrator-agent`/R (High): `token_id` in `WorkspaceCtx` enables per-token write attribution; the audit trail names the token, not just the workspace.
- `DS-admin-token`/R (Low): token rotation is a structured CLI verb logged to `DS-ts-events`; rotation events are auditable after the fact.
- V0 → V0.1 RLS hardening is additive: `CREATE POLICY` statements only; no schema migration; no backfill.
- The binary role enum keeps the policy surface manageable: roughly 20 Postgres RLS policies at V0.1 (2 roles × ~10 artifact tables), each one straightforward.

**Bad / Trade-offs:**
- The binary role enum (admin / read_observer) provides no per-plugin granularity. An `admin` token can invoke verbs across all `core: true` plugin families. That's fine at V0 dogfood (single operator), but it'll need extension at V0.1+ when multi-tenant onboarding requires per-plugin scoping.
- The `read_observer` role has no write access at all, not even to its own session metadata. This may be too restrictive for some V0.1+ monitoring use cases; scope extension is an ADR amendment.
- Token rotation doesn't currently invalidate in-flight requests that cached the old token. At V0 there's no in-process cache (per [P-0008-admin-token-shape](P-0008-admin-token-shape.md)), so this isn't a problem: each request does a fresh DB lookup, and rotation is effective immediately on the next request.

## Pros and Cons of the Options

### R1 — Binary admin/read-observer + scope strings (accepted)

- Pro: V0.1 RLS hardening is additive; the same role model holds at the application and substrate layers.
- Pro: `WorkspaceCtx.token_id` adds write attribution without extra complexity.
- Pro: The binary role keeps the policy surface at roughly 20 policies at V0.1.
- Con: No per-plugin granularity at V0; admin scope grants access to all plugin families.

### R2 — Per-plugin scoped roles

- Pro: Fine-grained access control; per-plugin delegation possible.
- Con: Scope string vocabulary explosion; `scopes TEXT[]` would carry up to ~10 values per token.
- Con: The RLS policy surface explodes to (roles × plugins × artifact types) instead of (roles × artifact tables).
- Con: No V0 use case drives this complexity; defer to V0.1+.

### R3 — Single admin-only role

- Pro: Simplest implementation.
- Con: No read-vs-write separation; monitoring agents and read-only consumers get full admin access.
- Con: Doesn't address `EE-specialist-agent`/S (Medium, 70): sub-agent identity separation needs at minimum a read-scoped token.

## More Information

- Frame open ADR slot: `{{P-RLSAdminToken}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table). This ADR resolves that slot.
- Depends on: [P-0001-storage-layout](P-0001-storage-layout.md) (C1 → ~10 artifact tables → manageable RLS policy surface); [P-0008-admin-token-shape](P-0008-admin-token-shape.md) (opaque token with `scopes TEXT[]` — this ADR defines the valid scope strings).
- [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) — `WorkspaceCtx` is the runtime carrier of `(workspace_id, role, token_id)` at V0; this ADR defines what `role` means.
- [P-0003-plugin-manifest](P-0003-plugin-manifest.md) — verb-to-scope mapping: the plugin manifest declares exposed MCP verbs; host routing checks role against verb before dispatching to the plugin.
- Threat references: `EE-orchestrator-agent`/S,R; `EE-specialist-agent`/S; `EE-operator`/S,R; `P-mcp-handler`/S,I; `P-cli-handler`/S,E; `P-builtin-auth`/S,E; `P-builtin-workspaces`/E; `P-builtin-projects`/I; `P-builtin-agents`/R; `DS-admin-token`/I,T,R; `DS-pg-content`/I; `DS-ts-metrics`/I; `DF-host-fn-call`/I. ([Overview](../architecture/overview.md))
- Accepted risk `R-0001` ([Overview](../architecture/overview.md)): RLS policy enforcement deferred to V0.1+; V0.1 activation trip-wire named above.
- Accepted risk `R-0002` ([Overview](../architecture/overview.md)): external-AS integration deferred; the static admin token is the V0 auth path.
- Accepted risk `R-0006` ([Overview](../architecture/overview.md)): operator-action repudiation partially mitigated at V0; the activity log at `0.5.0` is the fuller mitigation.
- Follow-up: per-plugin scope extension for V0.1+ multi-tenant onboarding; per-agent token derivation for `EE-specialist-agent`/S mitigation.
