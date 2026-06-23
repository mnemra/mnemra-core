---
title: "P-0006: V0 Tenant Enforcement"
summary: "Defines the V0 application-layer workspace isolation mechanism: typed WorkspaceCtx parameter binding at the host-fn boundary as the load-bearing enforcement while RLS policy enforcement is deferred to V0.1+."
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

# P-0006: V0 Tenant Enforcement

## Status

`accepted`

## Context and Problem Statement

Mnemra-core's `workspace_id` scoping key is structural from V0: every artifact table carries `workspace_id NOT NULL`, and the RLS column-shape ships at `0.1.0`. However, Row-Level Security (RLS) **policy enforcement** is deferred to V0.1+ — the Postgres policy objects are not activated at V0, because activating them before the application layer is stable creates a substrate migration cliff at V0.1+.

The accepted-risk register captures `R-0001`: "Cross-tenant disclosure on `P-mcp-handler`/I, `P-builtin-projects`/I, `DS-pg-content`/I, `DF-host-fn-call`/I depends on the host-side WHERE-clause discipline rather than database-enforced policy." The risk-register trip-wire fires at first deployment serving more than one workspace (production multi-tenant traffic), or if lint coverage on read paths drops below 100%, or if a third-party plugin loads.

Per the workspace principle `P-SecurityLayered`, each security layer must be independently load-bearing. This means the application layer at V0 is not a placeholder that RLS "will fix later" — it is the operating enforcement mechanism. Its failure modes must be structurally contained.

Three enforcement postures are in scope. The Frame's Tier-A description calls typed `WorkspaceCtx` parameter binding the conservative lead, with two open variants. This ADR locks the choice.

## Decision Drivers

- **P-SecurityLayered.** Each layer is independently load-bearing; losing any layer weakens the whole. The V0 application layer must enforce workspace isolation robustly even if a future RLS policy is misconfigured or delayed.
- **Structural, not advisory.** The enforcement mechanism must make the wrong thing — writing to a foreign workspace — impossible to author, not just wrong to run. A well-typed host-fn ABI surface achieves this; a lint rule does not.
- **WHERE-clause-mandatory.** The overview's trust-boundary model requires that `workspace_id` is a WHERE-clause condition on every read path, not a post-read filter. The enforcement choice must make WHERE-clause enforcement structurally easy, not a per-author discipline.
- **Plugin ABI boundary.** Plugins are IO-free; host-fns are the only IO surface. `workspace_id` must be **host-derived** from the calling session — it must never be a plugin-supplied parameter on write paths. The enforcement mechanism must structurally prevent a plugin from supplying its own `workspace_id` (`P-host-fns`/T, Critical, severity 90).
- **No runtime overhead per host-fn call.** Workspace claim extraction from the session is O(1); a storage-layer query rewriter adds a query-planning step per call. For V0 dogfood load the difference is negligible, but the design choice sets a precedent.

## Considered Options

### Option A — Typed `WorkspaceCtx` parameter binding at host-fn boundary (lead)

Every host-fn signature takes a `WorkspaceCtx` as its first parameter. The host populates `WorkspaceCtx` from the validated request token at the start of each MCP verb dispatch; the context is passed down the call chain. Host-fn implementations extract `workspace_id` from the context — the ABI makes it impossible to author a host-fn that omits the context entirely without a compile-time failure.

A lint rule (enforced at CI) checks that no host-fn reads `workspace_id` from any source other than the `WorkspaceCtx` argument.

### Option B — Storage-layer query rewriter

A middleware layer intercepts all Postgres queries issued by the host and injects `WHERE workspace_id = ?` based on the active session. Host-fn authors do not need to explicitly thread `workspace_id` through parameters; the rewriter handles it.

### Option C — Per-host-fn explicit `workspace_id` parameter validation

Each host-fn accepts `workspace_id` explicitly as a parameter and validates it against the session's workspace claim before issuing any query. The validation is in-function, not a shared typed context.

## Decision Outcome

**Option A** — typed `WorkspaceCtx` parameter binding at the host-fn boundary.

**Rationale:**

Option A gives the strongest structural guarantee: the host-fn ABI itself enforces context threading. It is impossible to author a host-fn implementation that can issue a database query without having received a `WorkspaceCtx` — the parameter is structurally required, not optionally documented. This directly satisfies `P-host-fns`/T (Critical, severity 90): "`workspace_id` is host-derived from the calling session, NEVER a plugin parameter on write paths."

Option B (query rewriter) provides a weaker guarantee: the rewriter can be misconfigured, disabled in a test context, or bypassed by a direct query path (e.g., a migration script, a backup tool, a health probe). At V0 single-workspace dogfood the risk is low, but the rewriter approach normalizes the premise that workspace enforcement is a database-layer concern — which conflicts with `P-SecurityLayered`'s requirement that the application layer is independently load-bearing. It is also harder to verify: "the rewriter runs on every query" is harder to assert at code review than "every host-fn signature requires `WorkspaceCtx`."

Option C (per-fn explicit parameter) is weaker than Option A because the `workspace_id` parameter on write paths is exactly the attack surface `P-host-fns`/T names. A plugin that supplies `workspace_id` directly on a write call should not even compile against the host-fn ABI. Option C inverts this: it requires plugins to supply `workspace_id` and trusts the validation inside the host-fn to catch mismatches. That is an advisory guard, not a structural one.

### WorkspaceCtx binding specification (V0 floor)

| Property | V0 value | Rationale |
|---|---|---|
| `WorkspaceCtx` parameter position | First parameter of every host-fn signature | Consistent position; enables mechanical lint |
| `WorkspaceCtx` construction site | Single location in the MCP verb dispatch path, after token validation | One construction site = one audit point; no aliasing |
| `workspace_id` extraction | `WorkspaceCtx::workspace_id()` accessor only; direct field access is private | Prevents accidental use of an unvalidated workspace_id |
| Write-path invariant | `workspace_id` is NEVER a plugin-caller parameter on write paths; host derives it from context | Enforced by ABI signature; lint checks for raw `workspace_id` params on write host-fns |
| Read-path invariant | `workspace_id` is a WHERE-clause condition, extracted from `WorkspaceCtx`, on all read host-fns | WHERE-clause-mandatory discipline; lint enforces |
| Session context for builtins | Same `WorkspaceCtx` threading; builtins are not exempt | Uniformity; no "internal" bypass paths |
| V0 enforcement gap (RLS deferred) | Application-layer enforcement is the sole structural barrier at V0 | `R-0001` documents the residual risk; trip-wire fires at first multi-workspace production deployment |

### Trip-wire to RLS policy enforcement (V0 → V0.1+)

The `R-0001` trip-wire fires at first deployment serving more than one workspace with production traffic. When it fires:

1. RLS policy objects must be defined and activated in a migration before the multi-workspace deployment proceeds.
2. `WorkspaceCtx` threading remains in place after RLS activation — it is the application-layer redundancy that `P-SecurityLayered` requires even after the substrate layer is active.
3. The WHERE-clause lint coverage must be 100% on read paths before the migration lands.

### Consequences

**Good:**
- Structural enforcement: host-fn ABI is typed to require `WorkspaceCtx`; the wrong thing is impossible to compile.
- `P-host-fns`/T (Critical) mitigated: plugin-supplied `workspace_id` on write paths is an ABI-level impossibility, not a runtime check.
- WHERE-clause-mandatory discipline is easy to lint: one `WorkspaceCtx` accessor, one lint pattern.
- Post-RLS, the `WorkspaceCtx` layer remains as the application-layer redundancy required by `P-SecurityLayered`.
- Uniform threading across host-fns and builtins — no "internal" bypass path.

**Bad / Trade-offs:**
- `WorkspaceCtx` must be threaded through every host-fn and downstream call that issues a database query. This is mechanical work but not high-risk; the compiler enforces it.
- Test harnesses must construct a valid `WorkspaceCtx` for every host-fn test. A test-only constructor (bypassing token validation) is required; it must be gated to `#[cfg(test)]` to prevent use in production code.
- The V0 application-layer enforcement has no Postgres-level redundancy until RLS activates. `R-0001` remains in the risk register until the V0.1+ RLS migration lands.

## Pros and Cons of the Options

### Option A — Typed `WorkspaceCtx` parameter binding (accepted)

- Pro: ABI-level structural guarantee; wrong thing is a compile error.
- Pro: Single construction site for `WorkspaceCtx` is a natural audit point.
- Pro: Compatible with future RLS activation — the application layer does not go away, it becomes redundant redundancy (good).
- Con: Mechanical threading work; test harness needs `#[cfg(test)]` constructor.

### Option B — Storage-layer query rewriter

- Con: Rewriter is a middleware that can be bypassed (direct query paths, migration scripts, backup tools, test fixtures that use a raw DB connection).
- Con: "Rewriter runs on all queries" is harder to assert at code-review than a typed parameter.
- Con: Conflicts with `P-SecurityLayered`: if the rewriter is the enforcement, the application layer has no independent load-bearing function.

### Option C — Per-host-fn explicit `workspace_id` parameter

- Con: The `P-host-fns`/T threat is precisely that a plugin supplies `workspace_id` on write paths; Option C enables this pattern rather than preventing it.
- Con: Validation inside each host-fn is advisory (can be omitted in a future fn without a compile error); Option A is structural.
- Con: Harder to lint: each fn's validation logic is different; a shared typed context is a single pattern.

## More Information

- Frame doc open ADR slot: `{{P-V0TenantEnforcement}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table).
- Stage 2a direction H2: add Tier-A `{{P-V0TenantEnforcement}}` with typed `WorkspaceCtx` parameter binding as the conservative lead.
- Threat references: `P-host-fns`/T,I; `P-mcp-handler`/I; `P-builtin-projects`/I; `DS-pg-content`/I; `DF-host-fn-call`/I ([Overview](../architecture/overview.md)).
- Accepted risk `R-0001` in overview: RLS policy enforcement deferred to V0.1+; application-layer enforcement is the V0 structural barrier; trip-wire fires at first multi-workspace production deployment.
- Trust boundary `TB-mnemra-host` ↔ `TB-postgres`: host-side WHERE-clause discipline as the structural barrier at V0 (overview trust-boundary table).
- Downstream: `{{P-RLSAdminToken}}` (Tier A, paused pending `{{P-StorageLayout}}`) extends the workspace claim model into the role/permission shape; `WorkspaceCtx` threading is the upstream mechanism `{{P-RLSAdminToken}}` builds on.
