---
spec_type: architecture
frame_relevant: true
modulation: brownfield-extension
---

# Intake: artifact-list cursor (keyset) pagination

**Stakes:** high
**Date:** 2026-06-29
**Status:** locked
**Consumer:** mixed — primarily the agent-facing MCP client (via the guest plugin export) and host-fn callers; the resulting spec is consumed by the verification, test-authoring, and implementation pipeline.

## JTBD

Artifact types are append-only — they grow without bound and never shrink. A caller listing artifacts of a type needs results delivered in **bounded, resumable pages** so that a single `list` call returns a predictable, safe-sized result regardless of how many artifacts a workspace has accumulated, and the caller can walk the full set page by page. Today the list path issues an unbounded query and returns the entire result set in one response.

## Non-goals

- **No offset/page-number pagination.** Keyset (cursor) only — offset pagination degrades on large append-only tables and is non-resumable across concurrent inserts.
- **No `COUNT` / total-count in the response.** `has-more` is computed by fetching one extra row, not by counting.
- **No change to the storage layout or id scheme.** `id` stays a ULID stored as `text` (single-column primary key); no new column, no index migration beyond what already backs the primary key.
- **No change to filter semantics.** The existing `filters: json` parameter is unchanged; pagination composes with it.
- **No activation of row-level-security policy objects.** The tenant `workspace_id` WHERE-clause condition is preserved exactly as today; RLS policy activation remains a later, additive concern.
- **No retroactive migration of unrelated existing specs or interfaces.** Scope is the artifact-list read path and its guest export only.

## Success criteria

1. The **host-fn** `artifact-list` accepts `limit: u32` and `cursor: option<string>` as explicit typed parameters and returns `record { ids: list<string>, has-more: bool, next-cursor: option<string> }`.
2. The **guest export** `list` accepts the same paging parameters and returns the same record shape, so the MCP client can paginate **end-to-end** (client → guest export → host-fn → storage).
3. The single invariant **`has-more = true` if and only if `next-cursor = some`** holds in every response.
4. **Default page size ≈ 100; hard cap ≈ 500.** A `limit` above the cap is clamped to the cap. The cap bounds the returned result set and therefore doubles as the result-set-size DoS bound, closing the prior unbounded-result-set security finding.
5. The storage query is keyset: `... WHERE workspace_id = $ctx AND type = $type AND id > $cursor ORDER BY id LIMIT $page + 1` — fetch one extra row to compute `has-more` without a `COUNT`; the tenant `workspace_id` condition is retained in the WHERE clause (not applied as a post-read filter).
6. No list path issues an unbounded query after the change; this is asserted by test.
7. All `core: true` plugins recompile against the changed ABI and pass their tests **before** the change merges (ABI-evolution discipline).
8. Existing list scenarios and tests are updated to the new return shape.

## Hard constraints

- **Mechanism — keyset/cursor**, fetch-one-extra-row to derive `has-more` without a `COUNT`. Order is by `id` (ULID-text primary key), which is lexicographically = chronologically sortable, so pages are returned in stable creation order.
- **Interface shape (in):** explicit typed parameters `(ctx, type, filters, limit: u32, cursor: option<string>)` — paging parameters are **not** folded into the `filters` json blob.
- **Interface shape (out):** `record { ids: list<string>, has-more: bool, next-cursor: option<string> }`.
- **Page size:** default ≈ 100, hard cap ≈ 500 (cap = result-set DoS bound).
- **Both interfaces in scope:** the host-fn and the guest export both gain pagination, so the MCP client paginates end-to-end.
- **Breaking ABI change:** the return type changes from `list<string>` to a record. This carries the stability-annotation requirement and the ABI-evolution / recompile-all-core-plugins discipline already established for the host-fn ABI.
- **Tenant isolation preserved:** the keyset SELECT retains the `workspace_id` WHERE-clause condition unchanged.

## Evidence

- **Append-only growth, measured.** Three months of real single-user work produced 2,630 artifacts (721 documents + 1,909 tasks); linear extrapolation ≈ 10,500/year. The largest type — tasks — is append-only (~1,900 now, ~1,330 already in a terminal state and never removed) → ~7,600/year and climbing. Multi-tenancy compounds this: per-workspace tables stay append-only, multiplied across workspaces. No fixed result cap survives this growth, so pagination is structural, not a deferrable stopgap.
- **Security finding (prior review):** the list read path currently issues an unbounded `SELECT` with no result bound — an unbounded-result-set denial-of-service surface. The hard cap in this work closes it.
- **Schema scrutiny (completed at intake):** `echo_fixture.id` is a ULID stored as `text`, the single-column primary key — verified stably orderable and index-backed, with lexicographic = chronological ordering. The keyset mechanism is therefore mechanically valid and yields creation-ordered pages; no schema change is required. (See frame doc.)

## Risk profile

**Security-relevant — carried to Frame.** Two dimensions: (1) the result-set-size DoS bound (the hard cap), and (2) tenant isolation on a read path (the `workspace_id` WHERE-clause condition must be preserved through the keyset rewrite). A security-mode review applies at the Frame and at the implementation review, and a pre-push security review gates the merge. No new trust boundary is introduced; the change hardens an existing read path.

## Consultations

- _none at intake — design was locked with the maintainer prior to this run; intake transcribes the locked design._

## Dismissed review flags

- _none yet._
