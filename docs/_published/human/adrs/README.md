---
title: Architecture Decision Records
summary: "How architecture decisions are structured here (G-* / P-* prefixes, MADR format)."
primary-audience: human
---

# Architecture Decision Records

This section tracks the architectural decisions made in mnemra-core using the [MADR](https://adr.github.io/madr/) (Markdown Architectural Decision Records) format.

## What is an ADR?

An Architecture Decision Record (ADR) captures a single architectural decision along with its context and rationale. ADRs are immutable once accepted — they are not edited to reflect changing opinions, but superseded by newer ADRs when decisions evolve.

## ADR Lifecycle

Each ADR moves through the following statuses:

| Status | Meaning |
|--------|---------|
| `proposed` | Under discussion; not yet accepted |
| `rejected` | Discussed and explicitly declined |
| `accepted` | The current decision in effect |
| `deprecated` | Was accepted; no longer applies (context changed) |
| `superseded` | Replaced by a newer ADR; see `superseded_by` field |

## G/P Prefix Projection Pattern

ADRs in this project use a two-tier identifier convention that reflects where a decision originates and how it propagates.

- **G-NNNN (general)** — workspace-wide architecture decisions. These do not live as individual files in this directory. Instead, they are projected once into [`DEFAULTS.md`](DEFAULTS.md) — a one-time snapshot recording each general standard as a concise entry, without the full rationale chain. DEFAULTS.md is frozen at projection time; upstream changes to a G-* decision do not auto-sync into this file.
- **P-NNNN (project)** — project-specific architecture decisions, stored as individual `P-NNNN-*.md` files in this directory. Two sub-categories:
  - *Project-specific:* a decision with no workspace-wide analog — a constraint or tradeoff unique to mnemra-core.
  - *Override:* a deviation from a `DEFAULTS.md` baseline entry. An override P-ADR names the default it overrides explicitly in its Status field (e.g., `Status: accepted, Overrides G-0017`).

*The default is the starting point; any deviation is a new P-ADR.*

## Using the Template

To record a new project ADR, copy [`template.md`](template.md) to a numbered file (e.g., `adrs/P-0001-topic.md`), fill in all frontmatter fields, and write the body sections. If the decision overrides a `DEFAULTS.md` entry, state that explicitly in the Status section.

## Current ADRs

Nine Tier-A ADRs authored in WS-E-2 Stage 3, plus the storage substrate/engine ADR (P-0010) folded in 2026-06-08 from a post-spec-lock storage-engine evaluation, plus the logging-facade ADR (P-0011) recording the `tracing` facade-from-binary choice merged 2026-06-17, plus the plugin-runtime + MCP-SDK ADR (P-0012) recording the raw-Wasmtime-over-Extism runtime choice (retroactive; shipped Tasks 20–22) and the official-`rmcp`-SDK-over-hand-rolled-MCP protocol choice (forward; implemented by Task 23) — correcting the two halves of the never-ratified `adr-001-plugin-runtime` research proposal, which the build reversed on both halves without an ADR. The 2026-06-09 observability re-derivation (P-0010 D8's E1 disposition — generation⊥storage separation) was **re-altituded out of the project-ADR layer**: observability *shape* is a theory trait + chassis mechanism, not a per-project ADR, so its decisions live in the [observability baseline](../architecture/overview.md#observability) and the original observability ADR P-0004 is `deprecated` (no successor ADR). P-0011 is *not* an observability-shape ADR — it is a dependency-selection + crate-topology decision (P-0010-shaped: a concrete library choice behind a locked contract), and is the plumbing foundation under that baseline. All Tier-A placeholder slots resolved (see [`placeholder-resolution.md`](placeholder-resolution.md) for full status).

| ADR | Summary |
|-----|---------|
| [P-0001-storage-layout](P-0001-storage-layout.md) | C1 single-document layout: whole artifact in one row, JSONB frontmatter + body + system fields, per-artifact-type tables; non-breaking C2 evolution path via the projection layer. Sits under P-0010 (the Postgres implementation under the swap trait). |
| [P-0002-core-plugin-partition](P-0002-core-plugin-partition.md) | Verb-on-content cohesion criterion: 4 `core: true` plugins (tasks / repos / jobs / contacts) covering `0.2.0`–`0.13.0`; 7 builtins covering foundational substrate; `0.14.0` migration as builtin handler. |
| [P-0003-plugin-manifest](P-0003-plugin-manifest.md) | V0 manifest schema: universal `content.emit` verb shape, `schema_version: 1`, typed host-fn allowlist compiled per-instance from signed manifest; `workspace_id` structurally excluded from write-path ABI. |
| [P-0004-observability-shape](P-0004-observability-shape.md) | **`deprecated` — no successor ADR.** (Historical.) Per-verb metrics surface, TimescaleDB retention policies (90d/365d/30d), health-endpoint detail body, and 1-hour continuous-aggregate window — conflated observability storage (TimescaleDB hypertables) into the app's own Postgres substrate. Its storage core was falsified by P-0010 D8; the generation-side decisions were re-altituded to the [observability baseline](../architecture/overview.md#observability) (a theory-trait baseline, not an ADR). |
| [P-0005-v0-signing-chain](P-0005-v0-signing-chain.md) | V0 minimum-viable signing-key custody: build-host-on-disk at mode 600, synchronous fail-closed plugin load verification, multi-deployment trip-wire to `{{P-SigningKeyCustodyHardening}}` (Tier C). |
| [P-0006-v0-tenant-enforcement](P-0006-v0-tenant-enforcement.md) | Typed `WorkspaceCtx` parameter binding at the host-fn boundary as the V0 application-layer workspace isolation mechanism; WHERE-clause-mandatory discipline; upstream of RLS policy enforcement (V0.1+). |
| [P-0007-plugin-resource-limits](P-0007-plugin-resource-limits.md) | Wasmtime per-instance resource limits: fuel (10B ticks), epoch-interruption (5s), memory ceiling (64 MiB), table/instance limits at Wasmtime defaults; both fuel and epoch ON at V0. |
| [P-0008-admin-token-shape](P-0008-admin-token-shape.md) | Static admin token: opaque 32-byte random value, BLAKE3-hashed in DB, workspace claim from server-side lookup row; trivial revocation, no second signing key at V0. |
| [P-0009-rls-admin-token](P-0009-rls-admin-token.md) | Binary admin/read-observer role enum; permission matrix per role for MCP verb categories and CLI control-plane ops; `WorkspaceCtx` carries `token_id` for write attribution; V0 app-layer enforcement with V0.1+ RLS policy hardening path. |
| [P-0010-storage-substrate-engine](P-0010-storage-substrate-engine.md) | PostgreSQL ratified on merits behind an engine-agnostic swappable `Storage` trait (one implementation); V0 embedded Postgres; A1-clean V0 stack (pgvector HNSW + native FTS + recursive CTEs + JSONB); keyword/graph/time-series on named trip-wires (D3/D4/D8 — TimescaleDB demoted); D6 method-borrows deferred to the retrieval ADR; D7 managed-tier skipped. Reframes P-0001/Frame/overview. Escalation E1 (D8 vs observability hypertables) dispositioned 2026-06-09 → re-altituded out of the ADR layer to the [observability baseline](../architecture/overview.md#observability); P-0004 `deprecated`, no successor ADR. |
| [P-0011-logging-facade](P-0011-logging-facade.md) | `tracing` + `tracing-subscriber` (Green/MIT) as the logging facade, wired **facade-from-binary**: the binary (`cmd/mnemra`) owns the one global subscriber (JSON → stdout + `EnvFilter`); library crates use `tracing` macros only and never depend on `tracing-subscriber`. Macros = locked contract, subscriber = binary-owned implementation. Foundation + crate-topology only (impl-crate-only); the full emission layer (semantics, per-verb metrics, OTel export, redaction) is deferred to Task 25 / R-0004. A dependency/topology choice (P-0010-shaped), not an observability *shape* decision (which the 2026-06-09 E1 disposition keeps in the [observability baseline](../architecture/overview.md#observability)). |
| [P-0012-plugin-runtime-and-mcp-sdk](P-0012-plugin-runtime-and-mcp-sdk.md) | Two orthogonal substrate-layer choices the never-ratified `adr-001-plugin-runtime` proposal had bundled, and the build reversed on both halves without an ADR. **Decision A (retroactive; shipped Tasks 20–22):** host plugins on **raw Wasmtime, not Extism** — P-0007's resource-limit mechanism (fuel + epoch + `ResourceLimiter`) needs direct Wasmtime `Config`/`Store` control an Extism abstraction would mediate. **Decision B (forward; implemented by Task 23):** adopt the official MCP Rust SDK **`rmcp` v1.7.0 (Apache-2.0/Green)** for the single stdio MCP server rather than hand-rolling MCP/JSON-RPC — R-0010-a mandates MCP-2025-06-18 conformance and the MCP server is the auth+dispatch trust boundary, the worst place to hand-roll a mandated external wire contract; mnemra's R-0010 handler logic rides on top as the `rmcp` `ServerHandler`, unchanged. Dependency-selection decisions (P-0010/P-0011-shaped), anchored to `P-StackDiscipline` applied per-layer. |
