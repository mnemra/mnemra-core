---
title: "P-0008: Admin Token Shape"
summary: "Defines the V0 static admin token structure: opaque random token with server-side workspace-claim lookup. Trivial revocation, no second signing key, workspace claim derived from a DB row."
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

# P-0008: Admin Token Shape

## Status

`accepted`

## Context and Problem Statement

Mnemra-core needs a static admin token at V0 as a bootstrap path for first-run and solo deployments. The token is the credential for the admin CLI and for MCP clients presenting admin-scope requests. Every token carries a workspace claim that scopes storage by workspace.

The product brief's Hard constraints commit to the static admin token with a structural workspace claim. They don't specify the token's internal structure. Two structurally different shapes are available:

1. **Opaque-with-server-side-lookup:** The token is a high-entropy random string. Workspace claim and permissions are looked up in a server-side table on every request. Loss of the token file revokes access immediately on next lookup; rotation deletes the old row. No second signing key required.

2. **Claim-carrying signed (JWT/PASETO/etc.):** The token is a self-describing signed assertion containing the workspace claim and permission scopes. Verification uses a server-held signing key. Rotation requires the old token to be invalidated (e.g., block-listed or version-rotated), which requires server-side state anyway.

The Frame's M4 direction (Stage 2a) split `{{P-RLSAdminToken}}` to surface this decision at Frame altitude. (Frame is Stage 2 of the work-shaping pipeline; see [Frame](../glossary.md#frame).) The split happened because the token structure gates downstream security architecture: loss of the token file, rotation semantics, and workspace-claim binding all differ between the two shapes. This ADR (Architecture Decision Record; see [ADR](../glossary.md#adr)) locks the Tier-A half. `{{P-RLSAdminToken}}` (Tier A, paused pending `{{P-StorageLayout}}`) owns the role model and permission shape downstream.

## Decision Drivers

- **Trivial revocation.** `DS-admin-token`/I (Critical, severity 80): the admin token file can be leaked by backup tooling, accidental world-readable mode, or a host compromise. If the token is opaque, revoking it is a single DB row delete followed by generating a new token. If the token is claim-carrying, revocation requires a block-list or version-rotation mechanism backed by server state, the same server state a purely opaque lookup requires anyway.
- **No second signing key at V0.** V0 already needs a signing-key custody decision for plugin signing (`{{P-V0SigningChain}}`). Adding a second signing key for token minting multiplies the custody surface without compounding security benefit at V0 dogfood scale.
- **Workspace claim must be authoritative.** `P-builtin-auth`/E (Critical, severity 80): "A bug in workspace-claim extraction defaults to `default` workspace when the claim is absent, granting access to the dogfood workspace from any authenticated token." Whether the claim is carried in the token or looked up from the DB, the host must validate it as mandatory. Absence is a hard auth failure. The distinction is where the claim lives, not whether it's validated.
- **V0 is a single-operator single-deployment.** The admin token is created on first run and held by the deploying operator. Sophisticated per-claim cryptographic verification provides no practical benefit over a DB lookup in a single-process single-deployment topology.
- **Downstream: `{{P-RLSAdminToken}}` binds on this choice.** The role model and permission shape downstream of this ADR must be compatible with the chosen token structure. Opaque-with-server-side-lookup makes the permissions table a natural companion to the token row; claim-carrying requires a claim schema decision that feeds into `{{P-RLSAdminToken}}`.

## Considered Options

### Option A — Opaque token with server-side workspace-claim lookup

The token value is a cryptographically random string (32 bytes, hex-encoded or base64url). The host stores a row in a `admin_tokens` table containing `(token_hash, workspace_id, scopes, created_at, rotated_at)`. On every authenticated request, the host looks up the token hash and extracts the workspace claim from the DB row. Token loss leads to deleting the row and generating a new token. No signing key for tokens.

### Option B — Claim-carrying signed token (JWT/PASETO)

The token is a signed assertion (JWT using HMAC-SHA256 or PASETO v4 local) containing `workspace_id`, `scopes`, `iat`, `exp` (long-lived; V0 effectively non-expiring). A symmetric signing key is held by the host. Verification checks the signature against the host key. Revocation requires a block-list table in the DB, which is structurally equivalent to the server-side lookup in Option A, minus the up-front simplicity.

### Option C — Opaque token with in-process cache (no DB lookup on hot path)

Same as Option A but the `admin_tokens` lookup is cached in-process at startup (or on first use) and only re-read on explicit rotation. Avoids DB round-trip per request.

## Decision Outcome

**Option A** — opaque random token with server-side workspace-claim lookup.

**Rationale:**

Option A is strictly simpler than Option B for V0 without a security regression. The canonical advantage of claim-carrying tokens, offline verification without a server round-trip, is irrelevant at V0 single-process deployment: the "verifier" and the "server" are the same process. The canonical advantage of opaque tokens, trivial revocation, is directly load-bearing here. The `DS-admin-token`/I threat (Critical) is a token-leak scenario, and delete-row-plus-generate-new is the most robust revocation path.

Option B introduces a second signing key at V0. V0 already has a signing key custody decision (`{{P-V0SigningChain}}`) for plugin signing. Adding a token-minting key doubles the custody surface. Both keys must be stored at mode 600, both must be rotated coherently, and both are a target for a host-read exploit. For a signing mechanism whose "verification" is a DB lookup anyway (because revocation needs the DB regardless), this added complexity has no payoff.

Option C (in-process cache) is a performance optimization. At V0 dogfood request rates, a DB lookup per authenticated request isn't a bottleneck. The cache introduces a consistency hazard: a rotation event must invalidate the cache, adding a code path that Option A avoids entirely. Deferred to V0.1+ if profiling shows the DB lookup is hot.

### Admin token specification (V0 floor)

| Property | V0 value | Rationale |
|---|---|---|
| Token value | 32 bytes cryptographically random, base64url-encoded (43 characters, no padding) | 256-bit entropy; standard URL-safe alphabet; no structural content |
| Storage form | `BLAKE3(token_bytes)` stored in `admin_tokens.token_hash` | Token bytes never stored; authenticated by unique-hash lookup. No constant-time primitive needed: a 256-bit CSPRNG token compared via its BLAKE3 hash has no applicable comparison-timing channel (R-0008-b) |
| DB table | `admin_tokens(id UUID PK, token_hash BYTEA NOT NULL UNIQUE, workspace_id UUID NOT NULL, scopes TEXT[] NOT NULL, created_at TIMESTAMPTZ NOT NULL, rotated_at TIMESTAMPTZ)` | Minimal schema; scopes array feeds `{{P-RLSAdminToken}}` downstream |
| Workspace claim | `workspace_id` column in the DB row; NOT NULL; absence is a hard auth failure | `P-builtin-auth`/E structural invariant: missing claim → reject, not default |
| File custody | Token value written to filesystem at mode 600, owner = host process UID, on first-run generation | Mirrors the `DS-admin-token` trust-boundary model; file-mode invariant check at startup |
| Revocation | Delete the `admin_tokens` row; generate a new token via the CLI rotation verb; log rotation event to `DS-ts-events` | `DS-admin-token`/R mitigation: rotation is a CLI verb, structurally logged |
| File-mode invariant | Startup: host checks that `DS-admin-token` file is mode 600 and not world-readable; fail-shut otherwise | `EE-operator`/S partial mitigation: removes the "accidental world-readable after restore" silent vector |

### Workspace claim binding semantics

The workspace claim in the token row is the `workspace_id` that scopes all operations authorized by this token. It is:

- Host-derived on every request: the `WorkspaceCtx` (per `{{P-V0TenantEnforcement}}`) is populated from the DB lookup result, not from any client-supplied header.
- NOT client-supplied: a request that attempts to override the workspace claim via a header or query parameter is rejected.
- Mandatory: a token row with a NULL `workspace_id` is a schema violation; the column is NOT NULL.

This binding is the upstream dependency `{{P-RLSAdminToken}}` builds on. The role model downstream can add scopes and permission shapes, but the workspace claim anchor is this ADR's decision.

### Consequences

**Good:**
- `DS-admin-token`/I (Critical) mitigation: revocation is one DB row delete; no block-list required; no second-key rotation entanglement.
- No second signing key at V0: custody surface for signing material is bounded to `{{P-V0SigningChain}}`'s build-host key.
- `P-builtin-auth`/E (Critical) mitigation: workspace claim is DB-row-sourced; absence is structurally impossible (NOT NULL column); the default-to-default-workspace bug can't arise from a NULL row field.
- Downstream `{{P-RLSAdminToken}}` has a clean upstream: the `scopes` array in the token row is the hook for the role/permission model.

**Bad / Trade-offs:**
- Every authenticated request incurs a DB lookup for the token hash. At V0 dogfood rates this isn't a bottleneck; if it becomes one, Option C (in-process cache with invalidation) is the natural V0.1+ optimization.
- BLAKE3 is a fast non-cryptographic hash for the token hash. Using a password-hashing function (Argon2, bcrypt) would be more resistant to offline brute-force if the DB row is leaked. But the token is 256-bit random. Brute-force is computationally infeasible regardless of the hash speed. BLAKE3 is the correct choice for a high-entropy token; password-KDFs are for low-entropy user passwords.
- The `admin_tokens` table is a substrate table that `{{P-StorageLayout}}` will govern. The table schema above is the V0 floor; `{{P-RLSAdminToken}}` may extend it with scope-specific columns once the role model is locked.

## Pros and Cons of the Options

### Option A — Opaque with server-side lookup (accepted)

- Pro: Trivial revocation; no second signing key; workspace claim is authoritative DB-row-sourced.
- Pro: Simpler implementation; no JWT/PASETO library dependency; no signing key rotation entanglement.
- Pro: Compatible with future claim expansion: the `scopes` array grows without changing the token structure.
- Con: DB round-trip per authenticated request; mitigated by dogfood load profile at V0.

### Option B — Claim-carrying signed (JWT/PASETO)

- Con: Requires a second signing key at V0; doubles the custody surface alongside `{{P-V0SigningChain}}`.
- Con: Revocation requires a block-list or version-rotation backed by DB state, the same DB infrastructure Option A uses more directly.
- Con: The claim-verification advantage (offline, no DB round-trip) is irrelevant in a single-process deployment.
- Con: Adds a JWT/PASETO library dependency and a claim-schema design decision that precedes `{{P-RLSAdminToken}}`.

### Option C — Opaque with in-process cache

- Con: Cache-invalidation on rotation adds a code path (and a potential bug surface) that Option A avoids.
- Con: V0 dogfood load doesn't warrant the optimization; premature complexity.
- Con: A stale cache on a leased token is a silent window between leak and revocation. Option A has no such window.

## More Information

- Frame doc open ADR slot: `{{P-AdminTokenShape}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table).
- Stage 2a direction M4: split `{{P-RLSAdminToken}}` → Tier-A `{{P-AdminTokenShape}}` + Tier-A `{{P-RLSAdminToken}}`. This ADR locks the upstream (token shape); `{{P-RLSAdminToken}}` (paused, depends on `{{P-StorageLayout}}`) owns the downstream (role model + permission shape).
- Threat references: `DS-admin-token`/I,T,R; `EE-operator`/S; `P-builtin-auth`/E,S; `P-mcp-handler`/S; `TB-mnemra-host`↔`TB-fs-secrets` trust boundary ([Overview](../architecture/overview.md)).
- Accepted risk `R-0002` in overview: external-AS integration deferred to V0.1+; static admin token is the V0 dogfood auth path.
- Accepted risk `R-0006` in overview: operator-action repudiation partially mitigated; admin-action audit log lands at `0.5.0`.
- Downstream: `{{P-RLSAdminToken}}` (Tier A, paused) — role model and permission shape; builds on the `scopes` array and `workspace_id` column defined here.
- `{{P-V0TenantEnforcement}}` (P-0006): workspace claim from the DB row is the source for `WorkspaceCtx` construction on every authenticated request.
