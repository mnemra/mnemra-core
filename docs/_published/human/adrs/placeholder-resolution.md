---
title: "Placeholder Resolution — Frame ADR Slots"
summary: "Maps each Frame ADR placeholder to its resolution — an ADR file, a re-altituded non-ADR baseline, or PAUSED status — and carries the P-number reservation ledger. WS-E-2 Tier-A slots plus per-feature-cluster Frame slots (retrieval cluster added 2026-07-02)."
primary-audience: agent
---

# Placeholder Resolution — Frame ADR Slots

A [Frame](../glossary.md#frame) (the stage-2 document that records a feature's locked architectural constraints) often names an [ADR](../glossary.md#adr) (Architecture Decision Record) it needs before the decision itself is written. It does this with a placeholder such as `{{P-StorageLayout}}`. This page maps each ADR placeholder from a locked Frame to its current resolution state. It also carries the ledger of reserved P-numbers, the sequential IDs given to [P-* ADRs](../glossary.md#p--adr) (project-scoped architecture decisions). The implementing developer maintains it during the [Spec](../glossary.md#spec) stage. As each slot resolves, the back-references in the Frame and in the architecture overview are updated to match. Slots are grouped by the Frame they came from.

**Sources:** [Frame: Mnemra Core](../intent/mnemra-core-frame.md), Tier A table; [Frame: retrieval cluster](../../intent/retrieval-cluster-frame.md), §8 Open ADR slots.

## Resolution Table

These are the Tier A slots from the core Mnemra Frame. A slot is either resolved by an ADR file, moved out of the ADR layer entirely, added later, or paused with a stated blocking reason.

| Placeholder | Status | File | Blocking reason (if PAUSED) |
|---|---|---|---|
| `{{P-StorageLayout}}` | **RESOLVED** | [P-0001-storage-layout.md](P-0001-storage-layout.md) | — |
| `{{P-CorePluginPartition}}` | **RESOLVED** | [P-0002-core-plugin-partition.md](P-0002-core-plugin-partition.md) | — |
| `{{P-PluginManifest}}` | **RESOLVED** | [P-0003-plugin-manifest.md](P-0003-plugin-manifest.md) | — |
| `{{P-ObservabilityShape}}` | **RE-ALTITUDED (out of the ADR layer)** | [observability baseline](../architecture/overview.md#observability): a theory-trait baseline in the overview, **not an ADR**. [P-0004](P-0004-observability-shape.md) is `deprecated`, with no successor ADR | — |
| `{{P-V0SigningChain}}` | **RESOLVED** | [P-0005-v0-signing-chain.md](P-0005-v0-signing-chain.md) | — |
| `{{P-V0TenantEnforcement}}` | **RESOLVED** | [P-0006-v0-tenant-enforcement.md](P-0006-v0-tenant-enforcement.md) | — |
| `{{P-PluginResourceLimits}}` | **RESOLVED** | [P-0007-plugin-resource-limits.md](P-0007-plugin-resource-limits.md) | — |
| `{{P-AdminTokenShape}}` | **RESOLVED** | [P-0008-admin-token-shape.md](P-0008-admin-token-shape.md) | — |
| `{{P-RLSAdminToken}}` | **RESOLVED** | [P-0009-rls-admin-token.md](P-0009-rls-admin-token.md) | — |
| *(no original placeholder; the substrate was a hard lock, not a slot)* | **FOLD-ADDED** | [P-0010-storage-substrate-engine.md](P-0010-storage-substrate-engine.md) | — |

## Retrieval-cluster Frame slots (Stage 3, 2026-07-02)

The retrieval cluster's Frame named its own slots in [Frame: retrieval cluster](../../intent/retrieval-cluster-frame.md) §8, locked 2026-07-02. The cluster's Stage-3 (Spec) authoring resolved them in reservation order, taking the next free numbers after P-0013:

| Placeholder | Status | File | Blocking reason (if PAUSED) |
|---|---|---|---|
| `{{P-0014}}` (retrieval architecture) | **RESOLVED** | [P-0014-retrieval-architecture.md](P-0014-retrieval-architecture.md) | — |
| `{{P-0015}}` (provenance envelope + source-roles contract) | **RESOLVED** | [P-0015-provenance-envelope-source-roles.md](P-0015-provenance-envelope-source-roles.md) | — |
| `{{P-0016}}` (edge schema) | **RESOLVED** | [P-0016-edge-schema.md](P-0016-edge-schema.md) | — |

The three ADRs are authored with status `proposed`. They move to `accepted` at the retrieval cluster's spec-exit gate, because they're part of the Stage-3 package that gate reviews alongside [the cluster spec](../../specs/2026-07-02-retrieval-cluster.md). The `{{P-00XX}}` placeholder references inside the locked `docs/intent/` artifacts are historical record. They're left as written.

## Summary

- **Resolved by an ADR (8):** P-0001, P-0002, P-0003, P-0005, P-0006, P-0007, P-0008, P-0009.
- **Re-altituded out of the ADR layer (1):** `{{P-ObservabilityShape}}`. The E1 disposition of 2026-06-09 ruled that observability is a theory trait plus a chassis mechanism, **not a per-project ADR**. So this slot resolves to the [observability baseline](../architecture/overview.md#observability) (a theory-trait baseline in the overview, not an ADR) rather than to an ADR file. The original observability ADR, P-0004, is `deprecated`: decision D8 of P-0010 falsified its storage core, and it has no successor ADR. When a Frame ADR slot resolves to something that isn't an ADR, that's an altitude re-disposition (the decision moved to a different layer of the documentation). It's marked explicitly here and in the Frame's Tier-A table.
- **Fold-added (1):** P-0010 (storage substrate and engine). It was authored 2026-06-08 from the storage-engine evaluation that followed spec lock (ratified 2026-06-07). The Frame had no substrate placeholder, because it treated the substrate as a hard-locked carry-forward from the product brief. So P-0010 is a new Tier-A slot, not the resolution of an existing `{{P-XXX}}`.
- **Paused (0):** none. Every Tier-A slot is dispositioned: 8 resolved by an ADR, and 1 re-altituded out of the ADR layer to the overview's observability baseline.
- **Resolution order:** StorageLayout (P-0001) unlocked CorePluginPartition (P-0002), PluginManifest (P-0003) and RLSAdminToken (P-0009), in dependency order. AdminTokenShape (P-0008) was already resolved. RLSAdminToken needed both P-0001 and P-0008.

## Number reservation scheme

Numbers are reserved in Frame Tier-A order for paused slots. Authored slots fill the positions that aren't reserved. The Frame's own resolution example (`{{P-StorageLayout}}` → `P-0001-storage-layout.md`) sets the reservation pattern.

| P-number | Placeholder | State |
|---|---|---|
| P-0001 | `{{P-StorageLayout}}` | authored |
| P-0002 | `{{P-CorePluginPartition}}` | authored |
| P-0003 | `{{P-PluginManifest}}` | authored |
| P-0004 | `{{P-ObservabilityShape}}` | authored, now `deprecated` (slot re-altituded out of the ADR layer to the overview's observability baseline; no successor ADR) |
| P-0005 | `{{P-V0SigningChain}}` | authored |
| P-0006 | `{{P-V0TenantEnforcement}}` | authored |
| P-0007 | `{{P-PluginResourceLimits}}` | authored |
| P-0008 | `{{P-AdminTokenShape}}` | authored |
| P-0009 | `{{P-RLSAdminToken}}` | authored |
| P-0010 | *(fold-added; no original placeholder)* | authored |
| P-0011 | *(no placeholder; logging-facade dependency and topology decision)* | authored |
| P-0012 | *(no placeholder; plugin runtime + MCP SDK record)* | authored |
| P-0013 | *(no placeholder; plugin invocation model, a forward decision)* | authored |
| P-0014 | `{{P-0014}}` (retrieval-cluster Frame §8) | authored |
| P-0015 | `{{P-0015}}` (retrieval-cluster Frame §8) | authored |
| P-0016 | `{{P-0016}}` (retrieval-cluster Frame §8) | authored |
| P-0017 to P-0025 | *(storage-cluster model, core-entity manifest, plugin contract, report execution context, report definition type, coordination cluster, plugin distribution, ingest pipeline shape, job queue)* | authored, and the files are present. They were entered in this ledger on 2026-10-19 when the next reservation was made, since the ledger had stopped at P-0016 |
| P-0026 | *(no placeholder; control-class ADR 1, DB connection authentication and transport; #4018 gate ruling 7793)* | authored `proposed` and accepted 2026-10-19: [P-0026-db-connection-authentication-transport.md](P-0026-db-connection-authentication-transport.md) |
| P-0027 | *(control-class ADR 2, DB authorization and role separation)* | authored `proposed` and accepted 2026-10-19: [P-0027-db-authorization-role-separation.md](P-0027-db-authorization-role-separation.md) |
| P-0028 | *(control-class ADR 3, requirements-check integrity)* | authored `proposed` and accepted 2026-10-19: [P-0028-requirements-check-integrity.md](P-0028-requirements-check-integrity.md) |
| P-0029 | *(control-class ADR 4, secrets management for the DB credential files)* | authored `proposed` and accepted 2026-10-19: [P-0029-db-credential-files.md](P-0029-db-credential-files.md) |
| P-0030 | *(control-class ADR 5, delegated-policy inventory and supply chain)* | authored `proposed` and accepted 2026-10-19: [P-0030-delegated-policy-inventory-supply-chain.md](P-0030-delegated-policy-inventory-supply-chain.md) |
| P-0031 | *(control-class ADR 6, test-substrate isolation)* | authored `proposed` and accepted 2026-10-19: [P-0031-test-substrate-isolation.md](P-0031-test-substrate-isolation.md) |
| P-0032 | *(control-class ADR 7, probe and readiness surface)* | **reserved** 2026-10-19. It's to be written with the HTTPS shared-service transport amendment, tracker #4127 (ruling 7793: "Probe surface (with transport)") |

A collision check ran before the 2026-10-19 reservations. `ls docs/src/adrs/ | grep -oE 'P-00[0-9]{2}'` returned P-0001 through P-0025, and `grep -rnoE 'P-00(2[6-9]|3[0-9])' docs/` returned no live reference. (One draft note in P-0010 had named a hypothetical `P-0026` for the supersession alternative. It was removed in the same change.)
