---
title: "Placeholder Resolution — Frame ADR Slots"
summary: "Maps each Frame ADR placeholder to its resolution — an ADR file, a re-altituded non-ADR baseline, or PAUSED status — and carries the P-number reservation ledger. WS-E-2 Tier-A slots plus per-feature-cluster Frame slots (retrieval cluster added 2026-07-02)."
primary-audience: agent
---

# Placeholder Resolution — Frame ADR Slots

This document maps each ADR placeholder from a locked Frame to its current resolution state, and carries the P-number reservation ledger. Maintained by the implementing developer at Spec stage; back-references are updated in the Frame and overview as each slot resolves. Slots are grouped by originating Frame.

**Sources:** [Frame: Mnemra Core](../intent/mnemra-core-frame.md), Tier A table; [Frame: retrieval cluster](../../intent/retrieval-cluster-frame.md), §8 Open ADR slots.

## Resolution Table

| Placeholder | Status | File | Blocking reason (if PAUSED) |
|---|---|---|---|
| `{{P-StorageLayout}}` | **RESOLVED** | [P-0001-storage-layout.md](P-0001-storage-layout.md) | — |
| `{{P-CorePluginPartition}}` | **RESOLVED** | [P-0002-core-plugin-partition.md](P-0002-core-plugin-partition.md) | — |
| `{{P-PluginManifest}}` | **RESOLVED** | [P-0003-plugin-manifest.md](P-0003-plugin-manifest.md) | — |
| `{{P-ObservabilityShape}}` | **RE-ALTITUDED (out of the ADR layer)** | [observability baseline](../architecture/overview.md#observability) — a theory-trait baseline in the overview, **not an ADR**; [P-0004](P-0004-observability-shape.md) `deprecated`, no successor ADR | — |
| `{{P-V0SigningChain}}` | **RESOLVED** | [P-0005-v0-signing-chain.md](P-0005-v0-signing-chain.md) | — |
| `{{P-V0TenantEnforcement}}` | **RESOLVED** | [P-0006-v0-tenant-enforcement.md](P-0006-v0-tenant-enforcement.md) | — |
| `{{P-PluginResourceLimits}}` | **RESOLVED** | [P-0007-plugin-resource-limits.md](P-0007-plugin-resource-limits.md) | — |
| `{{P-AdminTokenShape}}` | **RESOLVED** | [P-0008-admin-token-shape.md](P-0008-admin-token-shape.md) | — |
| `{{P-RLSAdminToken}}` | **RESOLVED** | [P-0009-rls-admin-token.md](P-0009-rls-admin-token.md) | — |
| *(no original placeholder — substrate was a hard-lock, not a slot)* | **FOLD-ADDED** | [P-0010-storage-substrate-engine.md](P-0010-storage-substrate-engine.md) | — |

## Retrieval-cluster Frame slots (Stage 3, 2026-07-02)

Slots named at [Frame: retrieval cluster](../../intent/retrieval-cluster-frame.md) §8 (locked 2026-07-02), resolved by the cluster's Stage-3 authoring in reservation order (next free numbers after P-0013):

| Placeholder | Status | File | Blocking reason (if PAUSED) |
|---|---|---|---|
| `{{P-0014}}` (retrieval architecture) | **RESOLVED** | [P-0014-retrieval-architecture.md](P-0014-retrieval-architecture.md) | — |
| `{{P-0015}}` (provenance envelope + source-roles contract) | **RESOLVED** | [P-0015-provenance-envelope-source-roles.md](P-0015-provenance-envelope-source-roles.md) | — |
| `{{P-0016}}` (edge schema) | **RESOLVED** | [P-0016-edge-schema.md](P-0016-edge-schema.md) | — |

The three ADRs are authored `proposed` and move to `accepted` at the retrieval-cluster spec-exit gate (they are part of the Stage-3 package the gate reviews, with [the cluster spec](../../specs/2026-07-02-retrieval-cluster.md)). The `{{P-00XX}}` placeholder references inside the locked `docs/intent/` artifacts are historical record and are left as written.

## Summary

- **Resolved by an ADR (8):** P-0001, P-0002, P-0003, P-0005, P-0006, P-0007, P-0008, P-0009.
- **Re-altituded out of the ADR layer (1):** `{{P-ObservabilityShape}}` — the 2026-06-09 E1 disposition ruled observability a theory trait + chassis mechanism, **not a per-project ADR**, so this slot resolves to the [observability baseline](../architecture/overview.md#observability) (a theory-trait baseline in the overview, not an ADR) rather than to an ADR file. The original observability ADR P-0004 is `deprecated` (its storage core falsified by P-0010 D8; no successor ADR). A Frame ADR-slot resolving to a non-ADR is an altitude re-disposition, marked explicitly here and in the Frame Tier-A table.
- **Fold-added (1):** P-0010 (storage substrate/engine) — authored 2026-06-08 from the post-spec-lock storage-engine evaluation (ratified 2026-06-07); the Frame had no substrate placeholder because the substrate was treated as a hard-locked brief carry-forward, so P-0010 is a new Tier-A slot rather than the resolution of an existing `{{P-XXX}}`.
- **Paused (0):** none — every Tier-A slot is dispositioned (8 resolved by an ADR; 1 re-altituded out of the ADR layer to the overview observability baseline).
- **Resolution order:** StorageLayout (P-0001) unlocked CorePluginPartition (P-0002), PluginManifest (P-0003), and RLSAdminToken (P-0009) in dependency order. AdminTokenShape (P-0008) was already resolved; RLSAdminToken required both P-0001 and P-0008.

## Number reservation scheme

Numbers are reserved in Frame Tier-A order for paused slots; authored slots fill non-reserved positions. The Frame's own resolution example (`{{P-StorageLayout}}` → `P-0001-storage-layout.md`) establishes the reservation pattern.

| P-number | Placeholder | State |
|---|---|---|
| P-0001 | `{{P-StorageLayout}}` | authored |
| P-0002 | `{{P-CorePluginPartition}}` | authored |
| P-0003 | `{{P-PluginManifest}}` | authored |
| P-0004 | `{{P-ObservabilityShape}}` | authored, now `deprecated` (slot re-altituded out of the ADR layer to the overview observability baseline; no successor ADR) |
| P-0005 | `{{P-V0SigningChain}}` | authored |
| P-0006 | `{{P-V0TenantEnforcement}}` | authored |
| P-0007 | `{{P-PluginResourceLimits}}` | authored |
| P-0008 | `{{P-AdminTokenShape}}` | authored |
| P-0009 | `{{P-RLSAdminToken}}` | authored |
| P-0010 | *(fold-added; no original placeholder)* | authored |
| P-0011 | *(no placeholder — logging-facade dependency/topology decision)* | authored |
| P-0012 | *(no placeholder — plugin runtime + MCP SDK record)* | authored |
| P-0013 | *(no placeholder — plugin invocation model, forward decision)* | authored |
| P-0014 | `{{P-0014}}` (retrieval-cluster Frame §8) | authored |
| P-0015 | `{{P-0015}}` (retrieval-cluster Frame §8) | authored |
| P-0016 | `{{P-0016}}` (retrieval-cluster Frame §8) | authored |
