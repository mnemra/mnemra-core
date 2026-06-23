---
title: "Placeholder Resolution — Tier-A ADR Slots"
summary: "Maps each Tier-A Frame ADR placeholder to its resolution — an ADR file, a re-altituded non-ADR baseline, or PAUSED status. Updated by WS-E-2 Stage 3."
primary-audience: agent
---

# Placeholder Resolution — Tier-A ADR Slots

This page maps each Tier-A ADR (Architecture Decision Record: the captured context, decision, rejected alternatives, and consequences for one significant choice) placeholder from the Frame to its current resolution state. A Tier-A placeholder is a slot the Frame left open for a decision it knew was needed but hadn't yet made. The implementing developer maintains this page during the Spec stage (Stage 3 of the work-shaping pipeline, where a testable spec is produced). As each slot resolves, the back-references in the Frame and the overview are updated to point at the resolution.

**Source:** [Frame: Mnemra Core](../intent/mnemra-core-frame.md), Tier A table.

## Resolution Table

Each row below is one placeholder. A P-* ADR is an architecture decision scoped to a single project, as opposed to a workspace-wide one. The Status column says how the slot was closed: an ADR was written, the slot was moved out of the ADR layer entirely, or it's still paused. The last column carries a blocking reason only for paused slots.

| Placeholder | Status | File | Blocking reason (if PAUSED) |
|---|---|---|---|
| `{{P-StorageLayout}}` | **RESOLVED** | [P-0001-storage-layout.md](P-0001-storage-layout.md) | n/a |
| `{{P-CorePluginPartition}}` | **RESOLVED** | [P-0002-core-plugin-partition.md](P-0002-core-plugin-partition.md) | n/a |
| `{{P-PluginManifest}}` | **RESOLVED** | [P-0003-plugin-manifest.md](P-0003-plugin-manifest.md) | n/a |
| `{{P-ObservabilityShape}}` | **RE-ALTITUDED (out of the ADR layer)** | [observability baseline](../architecture/overview.md#observability) — a theory-trait baseline in the overview, **not an ADR**; [P-0004](P-0004-observability-shape.md) `deprecated`, no successor ADR | n/a |
| `{{P-V0SigningChain}}` | **RESOLVED** | [P-0005-v0-signing-chain.md](P-0005-v0-signing-chain.md) | n/a |
| `{{P-V0TenantEnforcement}}` | **RESOLVED** | [P-0006-v0-tenant-enforcement.md](P-0006-v0-tenant-enforcement.md) | n/a |
| `{{P-PluginResourceLimits}}` | **RESOLVED** | [P-0007-plugin-resource-limits.md](P-0007-plugin-resource-limits.md) | n/a |
| `{{P-AdminTokenShape}}` | **RESOLVED** | [P-0008-admin-token-shape.md](P-0008-admin-token-shape.md) | n/a |
| `{{P-RLSAdminToken}}` | **RESOLVED** | [P-0009-rls-admin-token.md](P-0009-rls-admin-token.md) | n/a |
| *(no original placeholder — substrate was a hard-lock, not a slot)* | **FOLD-ADDED** | [P-0010-storage-substrate-engine.md](P-0010-storage-substrate-engine.md) | n/a |

## Summary

- **Resolved by an ADR (8):** P-0001, P-0002, P-0003, P-0005, P-0006, P-0007, P-0008, P-0009.
- **Re-altituded out of the ADR layer (1):** `{{P-ObservabilityShape}}`. The 2026-06-09 E1 disposition ruled observability a theory trait plus a chassis mechanism rather than a per-project ADR. So this slot resolves to the [observability baseline](../architecture/overview.md#observability), a theory-trait baseline in the overview, not to an ADR file. The original observability ADR P-0004 is `deprecated`: its storage core was falsified by P-0010 D8, and it has no successor ADR. When a Frame ADR slot resolves to something that isn't an ADR, that's a change in the altitude at which the decision lives. It's marked explicitly both here and in the Frame Tier-A table.
- **Fold-added (1):** P-0010 (storage substrate and engine), authored 2026-06-08 from the post-spec-lock storage-engine evaluation that was ratified 2026-06-07. The Frame had no substrate placeholder because the substrate was treated as a hard-locked carry-forward from the brief. P-0010 is therefore a new Tier-A slot, not the resolution of an existing `{{P-XXX}}`.
- **Paused (0):** none. Every Tier-A slot is dispositioned: 8 resolved by an ADR, and 1 re-altituded out of the ADR layer to the overview observability baseline.
- **Resolution order:** StorageLayout (P-0001) unlocked CorePluginPartition (P-0002), PluginManifest (P-0003), and RLSAdminToken (P-0009) in dependency order. AdminTokenShape (P-0008) was already resolved. RLSAdminToken required both P-0001 and P-0008.

## Number reservation scheme

Numbers are reserved in Frame Tier-A order for paused slots; authored slots fill the non-reserved positions. The Frame's own resolution example (`{{P-StorageLayout}}` resolving to `P-0001-storage-layout.md`) sets the reservation pattern.

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
