---
title: "Architecture Overview: Mnemra Core"
summary: "Constraint inventory, quality-attribute utility tree, data-flow diagram, and threat-modeling scaffold for mnemra-core. Companion to the Frame doc."
primary-audience: agent
---

# Architecture Overview — Mnemra Core

**Date:** 2026-05-22 · **Status:** draft (companion to Stage 2 Frame) · **Altitude:** component

> This artifact ports the constraint inventory, quality-attribute utility tree, data-flow
> diagram, and threat-modeling scaffold from the V0 architecture constraints record (draft
> 2026-05-03), reconciled to the locked product brief (intake-exit 2026-05-20) and the
> companion Frame document. Where the predecessor framing was stale, this port carries the
> corrected framing — see the companion [Frame](../intent/mnemra-core-frame.md) section
> "Framing corrections" for full provenance.
>
> This overview is **agent-primary**: structured for parsing and machine addressing per
> the workspace-wide agent-primary-source-artifacts decision (locked 2026-05-11).

## Provenance

| Artifact | Reference |
|---|---|
| Companion Frame doc (Stage 2 of `/brief`) | [Frame: Mnemra Core](../intent/mnemra-core-frame.md), draft 2026-05-22 |
| Locked product brief (Stage 1, intake-exit gated) | [Product Brief: Mnemra Core](../intent/mnemra-core.md), locked 2026-05-20 |
| V0 architecture discovery | "V0 discovery" decision record, locked 2026-05-02 |
| V0 architecture constraints (port source) | "V0 constraints" decision record, draft 2026-05-03 |
| Architecture alignment mental model (round-6) | "Architecture alignment R6" decision record, agreed 2026-04-27 |

## Threat-modeling trigger

Mnemra-core V0 hits all four design-time threat-modeling triggers from the workspace
security-layered principle:

- **Authentication.** Per-deployment OIDC AS via RFC 9728 protected-resource-metadata;
  static admin token at V0.
- **Personally-identifiable-information-adjacent data.** Workspace artifact content
  includes operational data and dispatch metadata.
- **Network surface.** MCP transport (stdio at V0; streamable-HTTP V0.1+).
- **Multi-tenancy.** `workspace_id` is structural from V0; Row-Level Security column-shape
  ships at V0, policy enforcement V0.1+.

Threat-modeling fires at Stage 2 terminal review (security-mode reviewer dispatch with the
threat-modeling skill loaded). Threat callouts annotate the data-flow diagram below;
mitigations become Spec-stage ADR proposals. The threats-by-DFD-element and trust-
boundaries sections below are placeholders the terminal-review pass populates.

## Constraint inventory

### Hard-locked

The following constraints are locked by the product brief's Hard constraints section, the
locked V0 discovery, and the workspace architecture canon. They are inputs to the Spec
stage, not under re-derivation.

| Constraint | Source |
|---|---|
| Single-process Postgres at V0; `pgvector` + `timescaledb` extensions present | Brief Hard constraints; architecture alignment R4 |
| WebAssembly Component Model plugin runtime via Wasmtime; plugin core logic IO-free; all plugin IO host-mediated; plugins are leaves (no sideways linkage) | Brief Hard constraints; architecture alignment R2 + R4 |
| One MCP server with namespaced plugin verbs; stdio at V0; MCP specification 2025-06-18; streamable-HTTP a V0.1+ activation | Brief Hard constraints; architecture alignment R1 |
| Mnemra is Resource Server only; per-deployment OIDC AS via RFC 9728 protected-resource-metadata | Architecture alignment R5 + R6 |
| Static admin token at V0 (filesystem-stored, mode 600) | V0 discovery; architecture alignment R6 |
| `core: true` plugins signed by mnemra root and structurally non-uninstallable | Architecture alignment R2 + R4 |
| Four storage shapes (content / timeseries / log / state-config); content-first with CQRS-real projections | Architecture alignment R3 + R4 |
| Multi-tenancy structural; `workspace_id` NOT NULL on every artifact table; RLS column-shape at V0 with policy enforcement V0.1+ | Brief Hard constraints; architecture alignment R5 |
| Admin CLI is destructive / control-plane only; agent-facing CRUD on MCP | V0 discovery |
| Schema-driven dynamic admin CLI subcommands generated from plugin manifests | Architecture alignment R1 |
| License: Apache-2.0 with future-relicense clause | Brief Hard constraints (license lock 2026-05-20) |
| Self-hosted-first single-binary deployment posture; immutable image is valid packaging | Brief Hard constraints; architecture alignment R6 |
| Workspace cutover from the prior tooling at V0; the prior task-store binary remains available as a rollback path | V0 discovery |
| Rust-default toolchain; ecosystem-aligned tooling (non-Rust adopted only when no viable in-stack path exists) | Brief Hard constraints; workspace stack-discipline principle |
| Test-Driven Development pairs on non-trivial work; worktrees mandatory; main is protected | Workspace test/worktree/main-protection principles |
| Migration scope: the prior structured task-store tables (content / timeseries / log / state partitions) and a fixed subset of prior content-corpus subdirectories | V0 discovery (Migration scope) |
| Workspace-level operational tooling (skill files, team profiles, internal memory store, inboxes, scratch, agent-harness state) is V0.1+ scope (the brief's idea-tier internal-workspace-absorption entry) | V0 discovery (Migration scope); brief idea-tier |
| Architecture MUST NOT be schedule-pressured; marketing-tier dates are not architectural inputs | Brief Hard constraints |
| The system MUST NOT host a language model; embeddings call out to an external model | Brief Hard constraints + Non-goals |

### Reframed since 2026-05-03

The 2026-05-03 constraints draft listed a separate "External" block containing pre-
announce target dates, an alpha invite-only target date, time-budget figures, a $0 budget
figure, and a kill criterion. These are removed from the constraint inventory here for two
reasons. First, the brief's Hard constraints explicitly state that architecture **MUST
NOT** be schedule-pressured and that dates appearing in marketing or landing material are
not architectural inputs. Second, the kill criterion and time-budget are commercial
inputs — the brief carves commercial validation thresholds to a separate internal record
and explicitly does not inline them. Where any of these surface as architectural inputs at
the Spec stage (e.g., a trip-wire), they will be re-introduced with the trip-wire
specifically named (per the workspace deferral-trip-wire discipline).

### Negotiable

The constraint surface still under Spec-stage decomposition lives in the Frame's "Open ADR
slots" section. Each candidate ADR is named with a `{{P-XXX}}` placeholder; the Spec
stage authors them in tiered order (substrate-unblocking first, then migration mechanics,
then operational hardening).

## Quality-attribute utility tree

Six axes. Four of these are workspace core values; two are mnemra-specific non-functional
requirements that surfaced as load-bearing in V0 discovery. Reversibility is folded into
conflict-resolution discipline (cutover rollback path preserved per V0 discovery WC.x)
rather than scored as its own axis.

### Security (load-bearing — multi-tenancy is structural)

| Refinement | Scenario |
|---|---|
| Tenant isolation | A workspace-A token issuing `task.list` returns zero workspace-B rows even when query construction is buggy. |
| Plugin sandbox | A plugin attempting a direct Postgres connection is denied by the host; a plugin that panics does not crash the core; an infinite-loop plugin is killed and replaced from the pool. |
| Telemetry non-leak | An audit script over a dogfood-day's logs, traces, dispatch events, and session events finds zero artifact-body matches against the known-content corpus. |
| Signed plugins | A `core: true` plugin with a missing or invalid signature fails to load with a structured error naming the plugin. |
| Authentication-failure shape | An invalid-token JSON-RPC error code is distinguishable from "verb not found" and "parameter invalid", even with external-authorization-server integration deferred. |

### Simplicity

| Refinement | Scenario |
|---|---|
| Migration parsimony | A smoke test creates a `pgvector` index against existing artifacts without a table rebuild. |
| Single-binary deployment | `mnemra init` against a fresh Postgres bootstraps a working deployment in one command; `mnemra workspace list` returns `[default]`. |
| Plugin contract | A `projects` plugin manifest declares `content_types`, `state_scopes`, and the host-fn surface it requires; the plugin ships zero SQL and no DB schema. |

### Quality (production-grade — no personal-project discount)

| Refinement | Scenario |
|---|---|
| Migration safety | `mnemra restore --verify <snapshot>` round-trips identity-clean before any destructive operation runs. |
| Idempotency and resumability | SIGKILL mid-migration; rerun consults a per-record progress manifest; completed records are skipped, in-progress records resumed; output IDs match the prior partial run. |
| Round-trip equivalence | Read source frontmatter → write artifact → read artifact → byte-compare equal modulo system-generated fields (`migrated_from`, `migrated_at`, `frontmatter_version`). |
| Foreign-key preservation | Post-migration, task artifacts reference valid migrated project artifacts; dispatch-metric rows reference valid task artifacts across content / timeseries / log partitions. |

### Observability

| Refinement | Scenario |
|---|---|
| Per-verb metrics | A TimescaleDB query against the metrics hypertable returns p50 / p95 / p99 per verb for the last 24 hours of dogfood. |
| Health-endpoint shape | A request to `/health` returns 503 with a structured detail body identifying which dependency failed (Postgres reachable / extensions loaded / workspace=default exists). |
| Retention discipline | Every TimescaleDB hypertable visible in `\d+` introspection has an `add_retention_policy` declared. |

### ABI evolution discipline (mnemra-NFR)

Load-bearing once third-party plugin install activates at V0.1+; the V0 work that hardens
this is the discipline of marking each host function with a stability tier.

| Refinement | Scenario |
|---|---|
| Pre-1.0 freedom | An ABI-change PR causes all `core: true` plugins to recompile against the new ABI and pass their tests. |
| Stability-tier mark (WIT pattern) | A plugin calling an `@unstable` host function emits a deprecation warning to the log; a plugin calling a `@deprecated` host function gets a structured error. |

### Dogfood-cycle correctness (mnemra-NFR — V0 success criterion)

| Refinement | Scenario |
|---|---|
| Functional parity | A workspace-cutover audit cross-reference table shows zero remaining invocations of the prior task-store binary in operational tooling (skill files, slash commands, hook scripts, utility scripts) post-cutover. |
| Canonical-day fixture | A scripted day's worth of dispatches, task CRUD, skill-run lifecycle, and daily-log generation runs end-to-end against V0 mnemra-core; every verb succeeds; no fallback to the prior tooling. |

## Data-flow diagram

The data-flow diagram below uses D2 (the project's chosen graph format per the project
defaults — mdBook + D2 + Mermaid wiring is at `G-0026`). Each typed element ID
(`TB-*` trust boundary, `EE-*` external entity, `P-*` process inside the host,
`DS-*` data store, edge labels `DF-*`) is targetable for STRIDE-per-element annotations
at the terminal-review threat-modeling pass.

```d2
direction: down

TB-agent-runtime: {
  label: "TB-agent-runtime\n(MCP client host process)"
  shape: rectangle
  style.stroke-dash: 3

  EE-orchestrator-agent: {
    label: "EE-orchestrator-agent"
    shape: oval
  }
  EE-specialist-agent: {
    label: "EE-specialist-agent"
    shape: oval
  }
}

TB-human: {
  label: "TB-human"
  shape: rectangle
  style.stroke-dash: 3

  EE-operator: {
    label: "EE-operator"
    shape: oval
  }
}

TB-build-pipeline: {
  label: "TB-build-pipeline (conceptual)"
  shape: rectangle
  style.stroke-dash: 3

  EE-mnemra-root: {
    label: "EE-mnemra-root\n(signing authority)"
    shape: oval
  }
}

TB-mnemra-host: {
  label: "TB-mnemra-host (single-process server)"
  shape: rectangle

  P-mcp-handler: {
    label: "P-mcp-handler (stdio)"
  }
  P-cli-handler: {
    label: "P-cli-handler"
  }
  P-host-fns: {
    label: "P-host-fns"
  }
  P-migration-handler: {
    label: "P-migration-handler"
  }
  P-backup-handler: {
    label: "P-backup-handler"
  }
  P-health-handler: {
    label: "P-health-handler"
  }
  P-plugin-runtime: {
    label: "P-plugin-runtime\n(Wasmtime)"
  }
  P-builtin-projects: {
    label: "P-builtin-projects\n(host-builtin, not plugin)"
  }
  P-builtin-agents: {
    label: "P-builtin-agents\n(host-builtin, not plugin)"
  }
  P-builtin-workspaces: {
    label: "P-builtin-workspaces\n(tenant boundary)"
  }
  P-builtin-auth: {
    label: "P-builtin-auth\n(OIDC RS + admin token)"
  }
  P-builtin-users: {
    label: "P-builtin-users\n(host-builtin, not plugin)"
  }
  P-builtin-sessions: {
    label: "P-builtin-sessions\n(per-MCP-connection)"
  }
  P-builtin-permissions: {
    label: "P-builtin-permissions\n(per-plugin permissions)"
  }
}

TB-plugin-sandbox: {
  label: "TB-plugin-sandbox (WASM, per plugin)"
  shape: rectangle

  P-plugin-instance: {
    label: "P-plugin-instance\n(WIT host-fn-only IO)"
  }
}

TB-postgres: {
  label: "TB-postgres (single-process)"
  shape: rectangle

  DS-pg-content: {
    label: "DS-pg-content"
    shape: cylinder
  }
  DS-pg-state: {
    label: "DS-pg-state"
    shape: cylinder
  }
  DS-pg-projections: {
    label: "DS-pg-projections"
    shape: cylinder
  }
  DS-ts-metrics: {
    label: "DS-ts-metrics\n(TimescaleDB hypertable)"
    shape: cylinder
  }
  DS-ts-events: {
    label: "DS-ts-events\n(TimescaleDB hypertable)"
    shape: cylinder
  }
  DS-pg-logs: {
    label: "DS-pg-logs"
    shape: cylinder
  }
}

TB-fs-secrets: {
  label: "TB-fs-secrets (mode 600)"
  shape: rectangle
  style.stroke-dash: 3

  DS-admin-token: {
    label: "DS-admin-token"
    shape: cylinder
  }
  DS-mnemra-root-key: {
    label: "DS-mnemra-root-key\n(custody: open ADR slot)"
    shape: cylinder
  }
}

TB-fs-source: {
  label: "TB-fs-source (read-only at migration)"
  shape: rectangle
  style.stroke-dash: 3

  DS-source-taskdb: {
    label: "DS-source-taskdb\n(prior task store, SQLite)"
    shape: cylinder
  }
  DS-source-corpus: {
    label: "DS-source-corpus\n(prior content corpus, markdown)"
    shape: cylinder
  }
}

TB-fs-backup: {
  label: "TB-fs-backup"
  shape: rectangle
  style.stroke-dash: 3

  DS-fs-backup: {
    label: "DS-fs-backup"
    shape: cylinder
  }
}

TB-external-llm: {
  label: "TB-external-llm"
  shape: rectangle
  style.stroke-dash: 3

  EE-llm-provider: {
    label: "EE-llm-provider\n(embeddings endpoint)"
    shape: oval
  }
}

# Agent → MCP handler (stdio)
TB-agent-runtime.EE-orchestrator-agent -> TB-mnemra-host.P-mcp-handler: "DF-mcp-stdio"
TB-agent-runtime.EE-specialist-agent -> TB-mnemra-host.P-mcp-handler: "DF-mcp-stdio"

# Operator → CLI handler
TB-human.EE-operator -> TB-mnemra-host.P-cli-handler: "DF-cli-invoke"

# MCP routing
TB-mnemra-host.P-mcp-handler -> TB-mnemra-host.P-builtin-auth: "DF-auth-check"
TB-mnemra-host.P-mcp-handler -> TB-mnemra-host.P-builtin-projects: "DF-builtin-call"
TB-mnemra-host.P-mcp-handler -> TB-mnemra-host.P-builtin-agents: "DF-builtin-call"
TB-mnemra-host.P-mcp-handler -> TB-mnemra-host.P-builtin-workspaces: "DF-builtin-call"
TB-mnemra-host.P-mcp-handler -> TB-mnemra-host.P-plugin-runtime: "DF-verb-dispatch"

# Plugin runtime → plugin instance (sandbox boundary)
TB-mnemra-host.P-plugin-runtime -> TB-plugin-sandbox.P-plugin-instance: "DF-plugin-invoke"

# Plugin → host functions (only path out of sandbox)
TB-plugin-sandbox.P-plugin-instance -> TB-mnemra-host.P-host-fns: "DF-host-fn-call"

# Builtins → host functions (uniform IO path)
TB-mnemra-host.P-builtin-projects -> TB-mnemra-host.P-host-fns: "DF-host-fn-call"
TB-mnemra-host.P-builtin-agents -> TB-mnemra-host.P-host-fns: "DF-host-fn-call"
TB-mnemra-host.P-builtin-workspaces -> TB-mnemra-host.P-host-fns: "DF-host-fn-call"
TB-mnemra-host.P-builtin-users -> TB-mnemra-host.P-host-fns: "DF-host-fn-call"
TB-mnemra-host.P-builtin-sessions -> TB-mnemra-host.P-host-fns: "DF-host-fn-call"
TB-mnemra-host.P-builtin-permissions -> TB-mnemra-host.P-host-fns: "DF-host-fn-call"

# Host functions → Postgres
TB-mnemra-host.P-host-fns -> TB-postgres.DS-pg-content: "DF-substrate-rw"
TB-mnemra-host.P-host-fns -> TB-postgres.DS-pg-state: "DF-substrate-rw"
TB-mnemra-host.P-host-fns -> TB-postgres.DS-pg-projections: "DF-projection-rebuild"
TB-mnemra-host.P-host-fns -> TB-postgres.DS-ts-metrics: "DF-timeseries-write"
TB-mnemra-host.P-host-fns -> TB-postgres.DS-ts-events: "DF-timeseries-write"
TB-mnemra-host.P-host-fns -> TB-postgres.DS-pg-logs: "DF-log-write"

# Host functions → external LLM (embeddings)
TB-mnemra-host.P-host-fns -> TB-external-llm.EE-llm-provider: "DF-embed-call"

# MCP sampling (LLM via agent's MCP client, not bundled)
TB-mnemra-host.P-host-fns -> TB-mnemra-host.P-mcp-handler: "DF-sampling-up"

# CLI handler → admin token + operations
TB-mnemra-host.P-cli-handler -> TB-fs-secrets.DS-admin-token: "DF-token-read"
TB-mnemra-host.P-cli-handler -> TB-mnemra-host.P-migration-handler: "DF-migration-trigger"
TB-mnemra-host.P-cli-handler -> TB-mnemra-host.P-backup-handler: "DF-backup-trigger"

# Migration handler → source filesystems → substrate writes
TB-mnemra-host.P-migration-handler -> TB-fs-source.DS-source-taskdb: "DF-migration-read"
TB-mnemra-host.P-migration-handler -> TB-fs-source.DS-source-corpus: "DF-migration-read"
TB-mnemra-host.P-migration-handler -> TB-postgres.DS-pg-content: "DF-migration-write"
TB-mnemra-host.P-migration-handler -> TB-postgres.DS-pg-state: "DF-migration-write"
TB-mnemra-host.P-migration-handler -> TB-postgres.DS-ts-metrics: "DF-migration-write"
TB-mnemra-host.P-migration-handler -> TB-postgres.DS-ts-events: "DF-migration-write"
TB-mnemra-host.P-migration-handler -> TB-postgres.DS-pg-logs: "DF-migration-write"

# Backup handler → backup filesystem
TB-mnemra-host.P-backup-handler -> TB-postgres.DS-pg-content: "DF-backup-read"
TB-mnemra-host.P-backup-handler -> TB-fs-backup.DS-fs-backup: "DF-backup-write"

# Signing (build-time → runtime signature verify)
TB-build-pipeline.EE-mnemra-root -> TB-mnemra-host.P-plugin-runtime: "DF-signing-attest"
TB-build-pipeline.EE-mnemra-root -> TB-fs-secrets.DS-mnemra-root-key: "DF-key-custody\n(open ADR slot)" {
  style.stroke-dash: 3
}
TB-mnemra-host.P-plugin-runtime -> TB-fs-secrets.DS-mnemra-root-key: "DF-signature-verify"

# Health probe
TB-mnemra-host.P-health-handler -> TB-postgres.DS-pg-content: "DF-health-probe"
```

### Notes on the diagram

- **REST surface is deliberately absent.** V0 has no REST surface; admin operations route
  through the admin CLI's local IPC, agent operations route through MCP. The plugin-
  defined REST routes from the earlier alignment record (round 1 deferred) and any admin
  REST from predecessor specs are not present at V0. A REST surface lands at V0.1+ if and
  when streamable-HTTP MCP transport activates (the microVM appliance trip-wire).
- **External authorization server is absent at V0.** V0 dogfood uses the static admin
  token at the filesystem secrets boundary. OIDC AS integration is V0.1+ (brief idea-tier
  entry).
- **Builtin components are inside the host process, not in the plugin sandbox.** The
  seven `P-builtin-*` nodes (`P-builtin-projects`, `P-builtin-agents`,
  `P-builtin-workspaces`, `P-builtin-auth`, `P-builtin-users`, `P-builtin-sessions`,
  `P-builtin-permissions`) execute as host code. This corrects the predecessor framing
  where projects and agents were drawn as plugins inside `TB-plugin-sandbox`. The
  corrected framing aligns with the locked brief's `0.1.0` substrate description and the
  Frame's enumeration of builtins (Workspace, Users, Agents, Authentication, Agent
  sessions, Per-plugin permissions, Projects).
- **The four storage shapes are visible.** Content (`DS-pg-content`) and state
  (`DS-pg-state`) substrates are regular Postgres tables; timeseries
  (`DS-ts-metrics`, `DS-ts-events`) are TimescaleDB hypertables; logs (`DS-pg-logs`) at
  V0 are Postgres-resident (per architecture alignment R4: the log-shape backend at V0 is
  Postgres-or-TimescaleDB; the choice between a regular table and a hypertable for the
  log shape is an open Spec-stage decision under `{{P-ObservabilityShape}}`).
- **The trust boundary `TB-build-pipeline` is conceptual.** The signing authority operates
  at build time; runtime sees the signature, not the key. Custody of the actual key
  material (whether it lives on the deployment node, in an HSM, is fetched at runtime, or
  is never on the node at all) is the open ADR slot `{{P-SigningKeyCustody}}`.
- **External LLM is for embeddings only.** Per the brief's Hard constraints, mnemra-core
  MUST NOT host a language model. The embeddings call out to an external provider via the
  ELT subsystem; the API key configuration surface is folded into `0.1.0` (brief T-5
  resolution). MCP sampling (`DF-sampling-up`) is the path by which plugins ask the
  connected agent's MCP client to run an LLM completion; the LLM provider that completion
  uses is external to mnemra-core.

## Threats by data-flow element

Populated by the Stage 2 terminal security review (2026-05-22) per the workspace
threat-modeling skill. Each row keys on a typed DFD element ID above and applies
STRIDE-per-element discipline. The element-type relevance bar is applied — categories
not applicable to an element type are skipped rather than padded.

| Element type | Applicable STRIDE |
|---|---|
| External entities (`EE-*`) | S, R |
| Processes (`P-*`) | S, T, R, I, D, E |
| Data stores (`DS-*`) | T, R, I, D |
| Data flows (`DF-*`) | T, I, D |

The table below extends the placeholder column set with **Severity** and **Confidence**
(0–100) per the workspace threat-modeling discipline. Severity vocabulary is
Critical/High/Medium/Low. Mitigation column references the Frame's Open ADR Slot IDs
(`{{P-XXX}}`) when one already covers the control class; new slots are flagged
`{{P-XXX — new}}` to surface them as Spec-stage follow-ups.

| Element ID | STRIDE | Threat | Severity | Confidence | Mitigation candidate |
|---|---|---|---|---|---|
| `EE-orchestrator-agent` | S | An MCP client process forges an orchestrator identity by presenting a valid token leaked from another agent's keystore or shell history; the host has no way to bind tokens to a specific process. | High | 75 | `{{P-RLSAdminToken}}` (token-scoping + per-agent token discipline); workspace-claim-in-token enforcement at `P-builtin-auth` |
| `EE-orchestrator-agent` | R | An orchestrator dispatches a destructive verb (e.g., bulk delete) and later denies the action; without per-token attribution on every write, the audit trail names the workspace but not the issuing agent. | High | 80 | `{{P-PluginManifest}}` + activity-log capability (`0.5.0`) carrying `(workspace_id, agent_id, session_id)` tuple on every write |
| `EE-specialist-agent` | S | A specialist runs under a delegated session and is impersonated by another specialist sharing the same workspace token; sub-agent identity is not distinguishable from the orchestrator's identity at the host. | Medium | 70 | `{{P-RLSAdminToken}}` (per-agent token derivation); designed-for at session model |
| `EE-specialist-agent` | R | A specialist's destructive write is not attributable beyond "agent inside workspace X" because the MCP session does not name the specific specialist. | Medium | 70 | Same as `EE-orchestrator-agent`/R |
| `EE-operator` | S | A second user on the deployment host reads the admin token file (file-mode regression, backup-tooling leak, accidental world-readable mode after restore) and impersonates the operator. | Critical | 80 | `{{P-RLSAdminToken}}` (token rotation, file-mode invariant check at startup); `{{P-SigningKeyCustody}}` informs the secrets-management posture |
| `EE-operator` | R | An operator runs a destructive admin operation (`drop-workspace`, force-restore-overwrite) and denies it; the operator's identity is the local UNIX user — no second factor, no audit trail beyond OS audit. | High | 75 | `{{P-RLSAdminToken}}` (admin-action audit log; tie destructive ops to a distinct audit event); accepted-risk R-0006 if deferred |
| `EE-mnemra-root` | S | A compromised CI environment forges a "mnemra-root-signed" plugin artifact, and the runtime accepts it because signature verification uses a key the attacker controls. | Critical | 70 | `{{P-SigningKeyCustody}}` (key isolation; ceremony; offline-root pattern); separate from runtime entirely |
| `EE-mnemra-root` | R | A signing event is non-repudiable only if signing is logged; absent a transparency log of signed plugins, a malicious "core: true" plugin could be denied as ever having been signed. | Medium | 60 | `{{P-SigningKeyCustody}}` (transparency log; sigsum or rekor-style append-only signing log) |
| `EE-llm-provider` | S | The host fetches embeddings from a TLS endpoint that an attacker has positioned as the configured provider (DNS rebind, typo-squatted hostname, untrusted CA injection); embeddings are tampered or harvested for content-corpus reconstruction. | High | 70 | `{{P-PluginManifest}}` + outbound HTTPS pinning policy (cert pin or trusted-CA bundle declared in config); LLM-API-key surface in `0.1.0` ships hostname allowlist |
| `EE-llm-provider` | R | The provider could later deny logging request metadata (artifact bodies / fragments / dispatch traces inferable from embed inputs); the deployment has no independent receipt. | Low | 60 | Accepted-risk R-0005 (telemetry no-leak is dogfood AC, not policy-enforced) |
| `P-mcp-handler` | S | A bogus MCP client presents a forged token; the handler accepts because it does not bind the token to an authenticated session and does not consult `P-builtin-auth` on every request. | Critical | 85 | `{{P-RLSAdminToken}}` (mandatory `DF-auth-check` on every request; per-request token verification with replay defense) |
| `P-mcp-handler` | T | A stdin-fed MCP request injects oversized or malformed JSON-RPC that breaks the parser into a non-conforming state; subsequent requests on the same stream are routed incorrectly. | High | 70 | `{{P-MCPWriteSemantics}}` (input-size cap, framing-error fail-shut, structured `parse_error` JSON-RPC code); explicit MCP 2025-06-18 input validation |
| `P-mcp-handler` | I | A correctly-authenticated workspace-A token issues a verb whose handler's query was malformed; without RLS policy enforcement at V0, workspace-B rows surface in the response. | Critical | 85 | `{{P-RLSAdminToken}}` (RLS policy enforcement at V0.1+; column-shape at V0); pre-mitigation: a workspace-scope-on-every-query lint and integration test in the host-fn surface |
| `P-mcp-handler` | D | An adversarial client floods stdio with verbs that exercise the most-expensive code path (vector search, projection rebuild) — single connection, no rate limit. | High | 75 | `{{P-MCPWriteSemantics}}` (per-session rate limit; cost-aware queueing); observability ships from `0.1.0` so the floor is detectability |
| `P-mcp-handler` | E | An MCP verb invokes a host function that was meant for control-plane only (e.g., a debug-only "dump tokens" verb left exposed by a builtin's manifest mis-declaration); the agent gains operator-level capability. | High | 65 | `{{P-PluginManifest}}` (verb-to-capability allowlist; control-plane verbs not exposed via MCP); compile-time enforcement preferred |
| `P-cli-handler` | S | A second process on the host invokes the CLI as the operator user; OS-level checks alone (the UID match) accept it. | Medium | 70 | `{{P-RLSAdminToken}}` (admin token mandatory for destructive ops; OS-uid alone is insufficient gate) |
| `P-cli-handler` | T | A CLI argv injection arises when a subcommand interpolates a parameter into a Postgres role-management call (`role-grant`, `workspace-create`) without escaping. | High | 70 | `{{P-MCPWriteSemantics}}` covers MCP write semantics; CLI surface needs its own parameter-binding discipline — surface as `{{P-AdminCLIDiscipline — new}}` |
| `P-cli-handler` | E | A schema-driven dynamic subcommand generated from a malicious or mis-declared plugin manifest exposes a destructive op (`migrate-overwrite`) at the operator CLI without intent. | High | 70 | `{{P-PluginManifest}}` (manifest validation; capability tier on subcommands; destructive ops behind an explicit confirmation gate); `core: true` only for the V0 dogfood path |
| `P-host-fns` | T | A host-fn writes content with caller-supplied `workspace_id` rather than deriving it from the session — a plugin (or builtin) supplies an attacker-chosen value and writes into a foreign workspace. | Critical | 90 | `{{P-PluginManifest}}` + `{{P-RLSAdminToken}}` (host derives `workspace_id` from the session/token; ABI MUST NOT accept `workspace_id` as a parameter on write paths); structural invariant |
| `P-host-fns` | I | A read-side host-fn returns rows that pass workspace-scope but include columns the plugin has no manifest declaration for; cross-content-type leak inside one workspace. | High | 70 | `{{P-PluginManifest}}` (per-plugin column projection; manifest declares readable columns; host-fn enforces) |
| `P-host-fns` | E | A host-fn intended for a `core: true` plugin (e.g., `mnemra.workspace.add_user`) is callable from a non-core plugin because the ABI does not key on plugin identity. | Critical | 80 | `{{P-PluginManifest}}` (per-plugin host-fn allowlist keyed on signed manifest; non-core plugin's allowlist is a strict subset) |
| `P-host-fns` | R | Embedding-call (`DF-embed-call`) traffic carrying artifact-derived inputs lacks a structured trail tying which workspace's content was sent to which external request. | Medium | 65 | `{{P-ObservabilityShape}}` (per-egress audit event in `DS-ts-events` with `(workspace_id, artifact_id, provider_hostname)`) |
| `P-migration-handler` | T | Migration writes to substrate tables (`DS-pg-content`, `DS-pg-state`, `DS-ts-*`, `DS-pg-logs`) using a privileged DB role; if migration is re-runnable, an attacker who can trigger it could mass-corrupt content under cover of "resume from checkpoint." | High | 65 | `{{P-MigrationID}}` + `{{P-CutoverDualWrite}}` (migration gated on admin-token capability; idempotency keyed on per-record manifest, not "rerun whole import") |
| `P-migration-handler` | I | Migration reads source frontmatter that may contain credentials, private-note content, or secrets embedded in markdown; if the migration log captures source content (for debugging), the log becomes a sink for sensitive data. | High | 70 | `{{P-MigrationID}}` + `{{P-ObservabilityShape}}` (migration log captures IDs and per-record outcome only, never content; redaction policy named in the migration spec) |
| `P-migration-handler` | D | A SIGKILL mid-migration leaves the substrate in a partial state; a re-run that did not consult the per-record progress manifest could re-process records and exhaust connection-pool / disk-space budgets. | Medium | 75 | `{{P-MigrationID}}` (per-record progress manifest mandatory; resume reads manifest first) |
| `P-backup-handler` | I | The backup stream contains every row of every substrate (including tokens, secrets-stored-as-content, dispatch artifacts) — the backup file is a higher-value compromise target than the live DB. | Critical | 85 | `{{P-BackupRestore}}` (backup encryption-at-rest mandatory; key separate from `DS-admin-token`); access-control on `DS-fs-backup` path |
| `P-backup-handler` | T | An attacker with write access to `DS-fs-backup` poisons a backup that a future restore consumes (introducing a malicious admin token, a forged workspace, or rewritten `core: true` plugin metadata). | Critical | 80 | `{{P-BackupRestore}}` (backup signed at write; restore verifies signature; round-trip-verify before destructive operations) |
| `P-backup-handler` | D | An adversary triggers continuous backups (CLI-exposed or scheduled-job-exposed) to exhaust disk space, taking the substrate read-only and the host degraded. | Low | 60 | `{{P-BackupRestore}}` (backup-trigger rate-limit; disk-space precondition check) |
| `P-health-handler` | I | A health-endpoint detail body leaks substrate state to an unauthenticated probe (extension list, table presence, workspace count). | Medium | 70 | `{{P-ObservabilityShape}}` (health response shape: minimal public summary; detailed body gated on admin token or loopback origin) |
| `P-health-handler` | S | The health endpoint is unauthenticated and shares a transport with MCP; a load-balancer probe could be fabricated and used as an oracle for substrate state. | Low | 55 | `{{P-ObservabilityShape}}` (separate handler binding for `/health`; not on MCP transport at V0 since transport is stdio) |
| `P-plugin-runtime` | T | A WASM module exploits a Wasmtime sandbox-escape primitive (rare, but the load-bearing assumption — sandbox integrity — is exactly what plugin-IO-free buys); host invariants violated. | High | 50 | `{{P-PluginPoolMemory}}` + `{{P-SigningKeyCustody}}` (Wasmtime version pinning under SBOM; `core: true` only at V0; supply-chain review on Wasmtime upgrades) |
| `P-plugin-runtime` | E | A plugin whose signature verification was skipped under a load-time error path (e.g., "key not loaded yet, defer to background") gets executed before verification completes. | Critical | 70 | `{{P-SigningKeyCustody}}` (verification is synchronous on plugin load; load fails closed; no "verify-async" path) |
| `P-plugin-runtime` | D | A plugin infinite-loops or allocates without bound; absent kill-and-replace from the pool, the host hangs the verb. | Medium | 70 | `{{P-PluginPoolMemory}}` (per-instance fuel/memory budget; pool kill-and-replace; per-verb timeout) |
| `P-plugin-runtime` | R | A plugin failure (panic, sandbox abort) is not durably attributed to a specific plugin+version+sig — operator cannot tell which artifact misbehaved across deployments. | Medium | 60 | `{{P-PluginPoolMemory}}` + `{{P-ObservabilityShape}}` (per-plugin-instance event stream with signed-artifact identity in `DS-ts-events`) |
| `P-plugin-instance` | E | A plugin requests a host-fn (`DF-host-fn-call`) outside its manifest's declared surface; if the host's permission check is at call-time and the plugin's manifest was loaded laxly, the plugin gains a capability it did not declare. | Critical | 80 | `{{P-PluginManifest}}` (manifest-declared host-fn surface compiled into the per-instance allowlist; calls outside the allowlist fail at the WIT boundary, not at the host-fn body) |
| `P-plugin-instance` | T | A plugin returns crafted bytes intended to defeat host-side parsing of plugin output (e.g., NUL bytes that desynchronize a downstream JSON serializer, oversized fields that DoS a projection rebuilder). | High | 65 | `{{P-PluginManifest}}` (host validates plugin output against the WIT-declared schema; size caps per field; parser is fail-shut on schema mismatch) |
| `P-plugin-instance` | I | A plugin uses `DF-sampling-up` (MCP sampling: plugin → host → MCP client's LLM) to exfiltrate content from one workspace into a prompt the connected agent's LLM provider sees, even though the plugin had no outbound network. | High | 75 | `{{P-PluginManifest}}` (manifest declares `sampling: allowed/denied`; if allowed, content fields the plugin can put in prompts are typed-restricted); at V0.1+ with third-party plugins this becomes Critical |
| `P-builtin-auth` | S | The auth builtin accepts a token whose signature is valid against a previously-trusted key whose rotation was incomplete; old-key tokens are still accepted past the rotation. | High | 70 | `{{P-RLSAdminToken}}` (per-deployment OIDC AS via RFC 9728; key rotation discipline with explicit cutover; old keys removed, not coexistent) |
| `P-builtin-auth` | E | A bug in workspace-claim extraction defaults to `default` workspace when the claim is absent, granting access to the dogfood workspace from any authenticated token. | Critical | 80 | `{{P-RLSAdminToken}}` (workspace claim is mandatory; absence is a hard auth failure, not a default; structural invariant) |
| `P-builtin-workspaces` | E | A workspace-create / workspace-delete verb is exposed to non-admin tokens because the builtin authorizes on workspace-claim presence rather than on a distinct admin scope. | Critical | 75 | `{{P-RLSAdminToken}}` (workspace lifecycle ops require admin scope; admin-scope tokens are a strict superset, distinguished by claim) |
| `P-builtin-projects` | I | Project metadata (project names, repo paths, dispatch-time references) is queryable cross-tenant before RLS policy enforcement; the structural `workspace_id` column is present but reads are not policy-filtered. | High | 75 | `{{P-RLSAdminToken}}` (RLS policy enforcement is V0.1+; pre-mitigation at V0 = manual host-side scope check on every read path, lint coverage on absence) |
| `P-builtin-agents` | R | An agent registration recorded the wrong principal (mis-mapped from token) and later actions are attributed to the wrong agent identity; the audit log records the action but not the discrepancy. | Medium | 60 | `{{P-RLSAdminToken}}` (agent identity derivation is canonical at registration; mismatch surfaces a structured error rather than silent registration) |
| `DS-pg-content` | T | A SQL-injection-style write through a host-fn that interpolates a content field into a query body — content-typed columns are not the obvious vector; metadata-typed (e.g., tag-name) columns are. | Critical | 70 | `{{P-PluginManifest}}` + `{{P-StorageLayout}}` (parameterized queries only; metadata fields normalized + length-capped; type-tightening at host-fn boundary) |
| `DS-pg-content` | I | Direct DB access from a process other than `P-host-fns` (a misconfigured backup tool with read-anywhere role, an operator with full Postgres credentials, a future read-replica without RLS) returns cross-workspace rows. | High | 75 | `{{P-RLSAdminToken}}` (RLS column-shape ships V0; policy enforcement at V0.1+); deployment-side: separate DB role per host process, least-privilege |
| `DS-pg-content` | R | Content deletions or modifications are not recorded with prior-value snapshots; an operator-issued destructive op is repudiable as "the agent did it." | Medium | 65 | `{{P-StorageLayout}}` + `{{P-FKPreservation}}` (immutable activity log per write; prior-value capture for destructive ops; ties to `0.5.0` activity log) |
| `DS-pg-content` | D | A pathological projection rebuild triggered by a content write storms the substrate, blocking reads on `DS-pg-projections` (and indirectly `DS-pg-content` via FK consistency checks). | Medium | 65 | `{{P-ProjectionRebuild}}` (rebuild queueing; backpressure; out-of-band rebuild path) |
| `DS-pg-state` | T | State writes from a builtin (e.g., agent-session state) interleave with concurrent CLI writes (e.g., admin token rotation), producing torn state that no constraint catches. | Medium | 65 | `{{P-MCPWriteSemantics}}` (OCC or transactional discipline on state shape; constraint-checked writes only) |
| `DS-pg-projections` | I | A projection contains denormalized content from cross-tenant sources (a vector index over content not partitioned by workspace) — a query inadvertently reaches it. | High | 70 | `{{P-ProjectionRebuild}}` + `{{P-RLSAdminToken}}` (projections partitioned by workspace; vector indexes keyed on `(workspace_id, vector)`) |
| `DS-ts-metrics` | I | TimescaleDB hypertable rows include verb names, argument shapes (low-card sketches), and per-tenant counts; cross-tenant query of the hypertable leaks usage patterns. | Medium | 65 | `{{P-RLSAdminToken}}` (metrics partitioned by `workspace_id`; per-workspace retention policies); admin-tier visibility distinct |
| `DS-ts-events` | T | Event-shape evolution is uncontrolled (new event-types from new capability families) and downstream consumers misread old events when types are reused. | Medium | 60 | `{{P-ObservabilityShape}}` (versioned event schema; `event_version` field; backward-compatible additions only without explicit migration) |
| `DS-ts-events` | I | Dispatch events capture content fragments in their description / payload (the dogfood telemetry-no-leak AC is a check, not a structural barrier). | Medium | 70 | Accepted-risk R-0005 at V0 (dogfood AC); `{{P-ObservabilityShape}}` ships event-payload typed schema in V0.1+ |
| `DS-pg-logs` | I | Operational logs include token fragments, query strings with PII content, or stack traces with workspace identifiers; a log-tail-as-debugging affordance leaks across tenants if logs are not workspace-partitioned. | Medium | 70 | `{{P-ObservabilityShape}}` (logs partitioned by `workspace_id`; redaction at log-write for high-entropy strings) |
| `DS-admin-token` | I | The admin token file is read by a backup process whose role has read-everywhere and writes the token into the backup stream in clear; the backup file then carries the admin secret. | Critical | 80 | `{{P-BackupRestore}}` (secrets excluded from backups by path policy; or backups encrypted with a separate key, not the admin token) |
| `DS-admin-token` | T | A second process on the host overwrites the admin token file (e.g., a misbehaving config tool); the host accepts the new token because it is read on-demand. | High | 70 | `{{P-RLSAdminToken}}` (token-file inode pinning at startup; modification fail-shut, with explicit rotation operation) |
| `DS-admin-token` | R | Operator-side rotation of the admin token is not logged structurally; rotation events cannot be audited after the fact. | Low | 60 | `{{P-RLSAdminToken}}` (rotation is a CLI verb that logs to `DS-ts-events` and to the activity log) |
| `DS-mnemra-root-key` | I | The signing key material lives on the deployment node (the default open-ADR position); a host-read primitive recovers the key, and an attacker thereafter signs forged `core: true` plugins. | Critical | 85 | `{{P-SigningKeyCustody}}` (key NOT on deployment node — offline-root + per-release certificate; or HSM-backed); structural posture, locked at Spec stage |
| `DS-mnemra-root-key` | T | Even if custody is offline, a transitively-trusted intermediate signing key (sub-CA pattern) on the build pipeline could be tampered if the pipeline is compromised. | High | 60 | `{{P-SigningKeyCustody}}` (build-pipeline integrity invariants; signed pipeline configuration; signing-environment attestation) |
| `DS-source-taskdb` | T | The migration source is read-only by intent; if migration writes-back (e.g., to mark records "migrated"), an attacker who can modify the source taskdb during migration can replay or rewrite migrated records. | High | 70 | `{{P-MigrationID}}` (source is strictly read-only; migration manifest is mnemra-side; source-side "migrated" markers are out-of-band, not source-side state) |
| `DS-source-corpus` | I | Source markdown frontmatter contains arbitrary content; if migration logs verbatim source paths and titles, log readers may see internal-project codenames or sensitive identifiers that were never meant to surface in operational telemetry. | Medium | 65 | `{{P-MigrationID}}` + `{{P-ObservabilityShape}}` (migration log uses derived IDs only; source paths hashed in log lines if needed for resume) |
| `DS-fs-backup` | I | Backup contents include the full substrate (tokens, secrets, content); same compromise surface as the live DB plus offline-accessibility. | Critical | 85 | `{{P-BackupRestore}}` (encryption-at-rest mandatory; key custody separate from substrate; backup file ACLs explicit) |
| `DS-fs-backup` | T | An adversary swaps in a poisoned backup; restore consumes it without integrity verification. | Critical | 80 | `{{P-BackupRestore}}` (backup-manifest hash verified before restore; signed manifest if backup-key custody allows) |
| `DF-mcp-stdio` | T | Stdio framing is non-self-describing; a man-in-the-stream (e.g., a wrapping process the agent invoked) modifies request/response bytes after the handshake. | Medium | 60 | `{{P-MCPWriteSemantics}}` (transport hardening — stdio at V0 trusts the spawning process; streamable-HTTP V0.1+ ships TLS) |
| `DF-mcp-stdio` | I | The same wrapper observes content responses; at V0 stdio there is no transport-level confidentiality. | Medium | 60 | Accepted-risk R-0003 (stdio-only at V0; transport confidentiality lands with `{{P-MCPWriteSemantics}}` and V0.1+ streamable-HTTP) |
| `DF-host-fn-call` | T | Host-fn arguments are passed as raw bytes the host trusts to be well-typed; a plugin crafts a struct-shape that exploits a host-side deserializer (e.g., an enum with an unexpected discriminant). | High | 65 | `{{P-PluginManifest}}` (WIT-defined types only; codegen-generated bindings on both sides; size-bounded fields) |
| `DF-host-fn-call` | I | A host-fn returns content (e.g., search results) whose row set was filtered by `workspace_id` after database read rather than as a WHERE-clause condition — a query plan change or bug surfaces foreign rows the filter then drops; an interposed metric on row-count leaks cross-workspace counts. | High | 65 | `{{P-RLSAdminToken}}` (workspace filter is WHERE-clause-mandatory; lint enforces; metric exports redact row-counts to per-workspace buckets) |
| `DF-sampling-up` | T | A plugin shapes prompt content that, when the connected agent's MCP client runs the LLM completion, induces the LLM to emit a destructive verb the orchestrator then executes. (Prompt-injection through the plugin surface.) | High | 70 | `{{P-PluginManifest}}` (sampling-allowed plugins are an explicit manifest capability; at V0 all plugins are `core: true` so the surface is contained; V0.1+ third-party plugins escalate this to Critical) |
| `DF-sampling-up` | I | The plugin can include arbitrary content in the sampling prompt, including content from other workspace artifacts the plugin is read-authorized for, which then traverses the agent's MCP client and reaches the agent's LLM provider — content leaves the deployment trust boundary by design. | High | 75 | `{{P-PluginManifest}}` (sampling-prompt fields typed; content-IDs only, not bodies, for cross-artifact references; LLM-provider hostname allowlist) |
| `DF-embed-call` | I | Artifact bodies are sent to an external LLM provider for embedding; the provider has full content access by design. | High | 90 | Accepted-risk R-0005 (external-LLM embed-call is the brief's explicit non-goal carve-out; LLM-API-key surface is in `0.1.0` and a hostname allowlist applies); compensating: BYO-provider deployment posture |
| `DF-embed-call` | T | A man-in-the-middle on the embed path could rewrite embedding vectors, poisoning the vector index. | Medium | 55 | `{{P-PluginManifest}}` (cert pinning on the embed provider; TLS verification mandatory; out-of-band vector-index integrity verification on rebuild) |
| `DF-signing-attest` | T | The signing-attestation flow trusts the build pipeline's identity assertion; a compromised pipeline forges an attestation. | High | 60 | `{{P-SigningKeyCustody}}` (attestation is signed by the offline root; pipeline-identity claims are inputs to attestation, not the attestation itself) |
| `DF-migration-write` | T | Migration writes interleave with concurrent CLI / MCP writes during cutover — the brief's cutover dual-write window — and content diverges between source and substrate. | Critical | 70 | `{{P-CutoverDualWrite}}` (cutover is single-writer; migration window holds an exclusive lock or operates pre-cutover; explicit "no concurrent writes" gate) |
| `DF-key-custody` | I | If key material flows from the build pipeline to the deployment node at any point, the network transit and the storage at rest are both leak surfaces. | High | 70 | `{{P-SigningKeyCustody}}` (key NEVER flows to deployment; runtime sees signature only; or HSM-backed where applicable) |

## Trust boundaries

Populated by the Stage 2 terminal security review (2026-05-22). The placeholder column
set is extended with **Threats at crossing** — element-ID + STRIDE references that key
into the threats-by-element table. Trust assumption changes at each crossing are named
under Authentication and Authorization.

**Canonical TB enumeration.** This table is the canonical source for the mnemra-core
trust-boundary set. The DFD lives in this document and the TB table sits adjacent to it,
so this artifact (not the Frame) owns the enumeration. The Frame doc carries a
steady-state subset (eight boundaries) used in Frame-altitude prose; this overview adds
the two migration-and-backup-scoped boundaries (`TB-fs-source`, `TB-fs-backup`) for the
full nine-row set. When the Frame's TB table and this one disagree, this one wins.

| Trust boundary | Crosses | Direction | Authentication | Authorization | Threats at crossing |
|---|---|---|---|---|---|
| `TB-agent-runtime` ↔ `TB-mnemra-host` | `DF-mcp-stdio` (MCP stdio transport) | bidirectional (request/response) | Bearer-token presented per MCP session; `P-builtin-auth` verifies against OIDC AS or static admin token. **V0:** static admin token suffices; OIDC verification is V0.1+. | Workspace claim in token scopes every operation; per-verb capability check at `P-mcp-handler` against the plugin manifest. Admin scope distinct from user scope. | `EE-orchestrator-agent`/S,R; `EE-specialist-agent`/S,R; `P-mcp-handler`/S,T,I,D,E; `DF-mcp-stdio`/T,I |
| `TB-human` ↔ `TB-mnemra-host` | `DF-cli-invoke` (admin CLI local IPC); `DF-token-read` (token-file read by CLI handler) | bidirectional (CLI invocation; structured responses) | UNIX UID match for CLI invocation; admin token mandatory for destructive operations. | Admin scope only — agent-facing CRUD does not route through the CLI. Schema-driven dynamic subcommands generated from plugin manifests; `core: true` plugins shipped at V0. | `EE-operator`/S,R; `P-cli-handler`/S,T,E; `DS-admin-token`/I,T,R |
| `TB-mnemra-host` ↔ `TB-plugin-sandbox` | `DF-plugin-invoke` (host → plugin); `DF-host-fn-call` (plugin → host) | host-mediated; cross-plugin calls always traverse the host | Plugin identity is the signed-manifest identity (signature verified at load); host derives session context at invocation. Plugin core is IO-free; ambient capability is the empty set. | Per-plugin host-fn allowlist compiled into the per-instance binding from the signed manifest; `workspace_id` is host-derived from the calling session, NEVER a plugin parameter on write paths; sampling, network, and filesystem are manifest-declared. | `P-plugin-instance`/E,T,I; `DF-host-fn-call`/T,I; `DF-sampling-up`/T,I; `P-plugin-runtime`/T,E,D,R |
| `TB-mnemra-host` ↔ `TB-postgres` | `DF-substrate-rw`, `DF-projection-rebuild`, `DF-timeseries-write`, `DF-log-write`, `DF-migration-write`, `DF-backup-read`, `DF-health-probe` | bidirectional (host issues queries; receives result sets) | Host-process DB user; ideally **role-separated per host process** (host-fns role, migration role, backup role, health-probe role) with least-privilege grants. | RLS column-shape ships V0 (`workspace_id` NOT NULL, indexed); RLS **policy enforcement** is V0.1+ — at V0 the host-side WHERE-clause discipline is the structural barrier (lint-enforced). | `P-host-fns`/T,I,E; `DS-pg-content`/T,I,R,D; `DS-pg-state`/T; `DS-pg-projections`/I; `DS-ts-metrics`/I; `DS-ts-events`/T,I; `DS-pg-logs`/I; `DF-host-fn-call`/I |
| `TB-mnemra-host` ↔ `TB-fs-secrets` | `DF-token-read` (CLI handler reads admin token); `DF-signature-verify` (plugin-runtime reads signing key/cert) | inbound to host (filesystem reads) | Filesystem ACLs (mode 600) + OS-uid match. **No second factor at V0.** | The host is the sole reader. Secrets are read lazily on-demand; file-mode invariant check at startup; modification fail-shut. | `DS-admin-token`/I,T,R; `DS-mnemra-root-key`/I,T; `P-plugin-runtime`/E (verify-async path) |
| `TB-mnemra-host` ↔ `TB-fs-source` | `DF-migration-read` (one-shot read of prior tooling state) | inbound to host (read-only by intent) | OS-side: read-as-host-process-uid; logical: migration handler is invoked from CLI under admin token. | Migration handler is the sole reader; source is **never** written-back; migration manifest is mnemra-side. | `DS-source-taskdb`/T; `DS-source-corpus`/I; `P-migration-handler`/T,I,D |
| `TB-mnemra-host` ↔ `TB-fs-backup` | `DF-backup-write` (host → backup); restore consumes inbound | outbound from host (write); inbound at restore | OS-side: write-as-backup-process-uid; logical: backup handler is invoked from CLI under admin token. | Backup handler is the sole writer; restore is admin-gated. **Backup contents are higher-value than the live DB by virtue of offline accessibility.** | `P-backup-handler`/I,T,D; `DS-fs-backup`/I,T |
| `TB-mnemra-host` ↔ `TB-external-llm` | `DF-embed-call` (HTTPS to embedding provider) | outbound from host | TLS server identity check; **hostname allowlist** at config; provider API key as a separate secret. | Per-deployment LLM-API-key configuration ships in `0.1.0`; no in-host LLM hosting (Hard constraint). Hostnames pinned via config; cert validation mandatory. | `EE-llm-provider`/S,R; `DF-embed-call`/I,T |
| `TB-build-pipeline` ↔ `TB-mnemra-host` | `DF-signing-attest` (build-time signing); `DF-signature-verify` (runtime verification reads only the signature) | inbound to host (signature only at runtime) | Runtime: signed-artifact signature verified against the mnemra root cert/key (custody open per `{{P-SigningKeyCustody}}`); build-pipeline identity is a pre-runtime concern. | Only `core: true` plugins at V0 — runtime accepts artifacts whose signature chains to the mnemra root; non-`core` plugin install is V0.1+ scope and requires a separate trust decision. | `EE-mnemra-root`/S,R; `DF-signing-attest`/T; `DF-key-custody`/I; `P-plugin-runtime`/E |

**Notes on the boundary set.** Two boundaries call out particular fragility worth surfacing
inline:

- **`TB-mnemra-host` ↔ `TB-postgres`** is the structural multi-tenancy fence. RLS
  column-shape ships V0 but **policy enforcement is V0.1+** — at V0 the WHERE-clause
  discipline at the host-fn layer is the only enforcement mechanism. A lint or test that
  asserts every read path carries a workspace filter is a Spec-stage mitigation that
  belongs in `{{P-RLSAdminToken}}`.
- **`TB-build-pipeline` ↔ `TB-mnemra-host`** is conceptual — the build pipeline is not a
  runtime adjacency — but the key custody choice (`{{P-SigningKeyCustody}}`) determines
  whether the boundary's actual asymmetry favors the attacker or the defender. The default
  open-ADR posture (key custody undecided) is a Critical-severity gap until that ADR
  locks.

## Accepted risks

Populated by the Stage 2 terminal security review (2026-05-22). Each entry is a risk
the V0 architecture accepts on the basis of the V0-dogfood-vs-V0.1+-commercial-tier
carve-out the locked product brief authorizes. Entries are numbered `R-NNNN`. Per the
workspace threat-modeling skill convention, every entry carries a **trip-wire** naming
the condition under which the deferred mitigation must be revisited. Owner is the
maintainer at V0; ownership transfers to a named role at commercial-tier transition.

The placeholder column set is extended with a **Rationale** column to make the
deferral basis legible without external reference. Every entry maps to one or more
threat rows above by Risk ID.

| Risk ID | Description | Owner | Trip-wire | Rationale |
|---|---|---|---|---|
| `R-0001` | RLS policy enforcement deferred to V0.1+; only the column-shape ships at V0. Cross-tenant disclosure on `P-mcp-handler`/I, `P-builtin-projects`/I, `DS-pg-content`/I, `DF-host-fn-call`/I depends on the host-side WHERE-clause discipline rather than database-enforced policy. | maintainer | First deployment serving more than one workspace (production multi-tenant traffic), OR the column-shape's lint coverage drops below 100% on read paths, OR a third-party plugin is loaded at runtime. | The brief locks tenant scoping key as structural from V0 precisely so policy enforcement can land later without substrate migration. V0 is single-workspace dogfood; the structural barrier is the WHERE-clause discipline, not policy enforcement. Defer-with-trip-wire matches `P-Defer`. |
| `R-0002` | External-authorization-server integration deferred to V0.1+. `P-builtin-auth` at V0 accepts a static admin token; per-deployment OIDC AS via RFC 9728 is in `0.1.0` substrate but federation/SSO is not. Operator-impersonation risk (`EE-operator`/S, `EE-orchestrator-agent`/S) hinges on token file ACLs and rotation discipline. | maintainer | First deployment integrating with an existing identity provider for the operator, OR shared-workstation use of the deployment node, OR external onboarding of agents beyond the maintainer's. | Brief Hard constraints scope mnemra as Resource Server only at V0; AS integration is brief idea-tier (D7). The static admin token bootstrap is the V0 dogfood pattern. |
| `R-0003` | MCP transport at V0 is stdio. Transport confidentiality is process-spawning trust — a wrapping process the agent invokes can observe request/response bytes (`DF-mcp-stdio`/I, T). | maintainer | `{{P-MCPWriteSemantics}}` resolution OR streamable-HTTP transport activates (the microVM-appliance trip-wire from the brief), whichever lands first. | Brief Hard constraints fix stdio at V0; streamable-HTTP is V0.1+ activation gated on the microVM-appliance trip-wire. Stdio confidentiality is "the agent's host process is the trust unit." |
| `R-0004` | Plugin signing-key custody is an open ADR slot (`{{P-SigningKeyCustody}}`). Until the ADR locks, the default open-position is that key material could land on the deployment node; `DS-mnemra-root-key`/I and `EE-mnemra-root`/S are Critical-severity threats whose mitigation depends on this ADR. | maintainer | `{{P-SigningKeyCustody}}` ADR locks (Tier C in the Frame's Open ADR Slots) — **mandatory before V0 ships any `core: true` plugin externally**. The risk register entry retires when the ADR locks. | The custody decision is structural and depends on the deployment posture (self-hosted-first single-binary vs immutable-image appliance vs HSM-backed). Spec stage owns this. |
| `R-0005` | External-LLM embedding calls (`DF-embed-call`/I) and the brief's "no in-host LLM" Hard constraint together mean **artifact bodies leave the deployment trust boundary** at every embedding call. Telemetry no-leak is a dogfood acceptance criterion (audit script against a known-content corpus), not a structural barrier. | maintainer | First deployment that is not single-operator-only (any deployment with telemetry-sensitive content beyond the maintainer's), OR Bring-Your-Own-Model (BYOM) is not configured and a hosted provider sees production content. | The brief explicitly carves "no in-host LLM" and "calls out to an external model" as Hard constraints. The compensating control is BYOM deployment posture — the LLM-API-key configuration surface is in `0.1.0`. The risk is acknowledged, structurally bounded by deployment posture, not mitigated by code. |
| `R-0006` | Operator-action repudiation at V0 is partially mitigated. CLI destructive ops (`drop-workspace`, force-restore-overwrite) tie to the OS UID; there is no second factor and no admin-action audit log that survives a substrate restore. Repudiation threats on `EE-operator`/R, `DS-admin-token`/R rely on OS-side audit. | maintainer | First deployment with more than one operator on the host, OR the activity-log capability (`0.5.0`) lands and admin actions can be tied to a durable audit event independent of the substrate. | Solo dogfood: the maintainer is sole operator; repudiation is not a meaningful threat in that topology. Activity log lands at `0.5.0`, after the substrate; the dependency is correct, the trip-wire fires on multi-operator topology, not on time. |
| `R-0007` | Plugin sampling (`DF-sampling-up`) at V0 is unrestricted because all plugins at V0 are `core: true` and signed by the mnemra root. The plugin-to-orchestrator prompt-injection surface (`DF-sampling-up`/T, `P-plugin-instance`/I) is High at V0 and **becomes Critical at V0.1+ when third-party plugin install activates** (brief idea-tier D11). | maintainer | Third-party plugin install activates (brief D11), OR `{{P-PluginManifest}}` locks the sampling capability shape without typed-prompt restrictions. | At V0 the plugin set is trusted by build provenance; signed artifacts are the only execution surface. The mitigation cost is high relative to V0 value; the trip-wire is structural (third-party install), not temporal. |

**Cross-reference.** Each accepted risk maps to one or more threat rows in the threats-
by-element table. A future risk-register file (workspace canon names `RISK-REGISTER.md`
as the storage shape, but the durable register lives with the project per `P-PerRepoFirst`)
holds the long-form per-entry record with review dates; this table is the per-Frame
snapshot the Spec stage operates against.

## Consultations

Mid-Frame expert dispatches that affect the architectural shape. This section grows as
the Frame matures.

| Date | Agent | Question | Outcome | Affects ADR slot |
|---|---|---|---|---|
| *(none yet — port pass is reconciliation against locked predecessors)* | | | | |

## Feedback to product brief

*Populated if Frame surfaces issues that would change the locked product brief. Each entry
names the brief section, the tension, and the proposed amendment for human-decomposer
resolution. Empty at port-time — Frame work did not surface a brief-level tension.*

| Brief section | Tension | Proposed amendment |
|---|---|---|
| *(none surfaced)* | | |

## Session log

- **2026-05-23** — Frame-exit gate revision. The companion Frame doc returned a Revise
  verdict at Frame-exit (G-0028 cold-start amendment, 2026-05-23). Overview revisions:
  (M1) the trust-boundaries table above is designated canonical for the mnemra-core TB
  set, with the Frame's eight-row steady-state table as a subset; (M2) the DFD's
  builtin component set extended from four (`P-builtin-projects`, `P-builtin-agents`,
  `P-builtin-workspaces`, `P-builtin-auth`) to seven (adds `P-builtin-users`,
  `P-builtin-sessions`, `P-builtin-permissions`) with the uniform host-fn edge pattern.
  Threats-by-element / trust-boundary-crossings / accepted-risks tables from the
  2026-05-22 terminal security review are preserved unchanged. Cross-reference: Frame
  doc Changelog 2026-05-23 entry for the full revision tally; Warden Stage 2
  code+security review of 2026-05-22 supplies the original findings.
- **2026-05-22** — Stage 2 terminal security review (security-mode reviewer pass with
  the threat-modeling skill loaded). The three threat-scaffold tables (threats by data-
  flow element, trust boundaries, accepted risks) populated by STRIDE-per-element walk
  of the DFD. The element-type relevance bar was applied — S/R for external entities;
  full STRIDE for processes; T/R/I/D for data stores; T/I/D for data flows. **72 threat
  rows recorded across 35 DFD elements; 9 trust boundaries annotated with crossings,
  trust-direction, authentication, authorization, and threats-at-crossing references;
  7 accepted risks recorded with rationale and named trip-wires.** Severity distribution:
  18 Critical, 29 High, 21 Medium, 4 Low. Mitigation candidates map to the Frame's 14
  Open ADR Slots wherever the control class fits; one new candidate slot surfaced
  (`{{P-AdminCLIDiscipline — new}}` — CLI parameter-binding hygiene, distinct from
  `{{P-MCPWriteSemantics}}`). Material risks naming themselves at the architectural
  layer: signing-key custody (`R-0004`), RLS policy enforcement deferral
  (`R-0001`), plugin sampling at V0.1+ third-party install (`R-0007`),
  workspace-id-supplied-by-plugin host-fn shape (`P-host-fns`/T Critical). Followups:
  the 14 `{{P-XXX}}` slots' Spec-stage authorship is the mitigation surface this pass
  produced; the new `{{P-AdminCLIDiscipline}}` candidate joins the Tier-C operational
  hardening set; the V0 dogfood scope underwrites every accepted risk and its
  trip-wire.
- **2026-05-22** — Port of the 2026-05-03 constraints draft into the project repo's
  agent-primary architecture overview. Reconciliation to the locked product brief
  (intake-exit 2026-05-20) applied:
  - Constraint inventory updated to the brief's Hard constraints (Apache-2.0 with
    future-relicense clause; license-lock 2026-05-20) and `0.1.0` substrate description
    (LLM-API-key configuration surface folded into substrate; builtin tenancy/identity
    core).
  - External-constraints rows from the 2026-05-03 draft removed (pre-announce dates,
    alpha invite-only target date, time budget, kill criterion) — the brief explicitly
    de-anchors architecture from marketing-tier dates and carves commercial inputs to
    a separate internal record.
  - DFD redrawn in D2 (project default `G-0026`) with `P-builtin-projects`,
    `P-builtin-agents`, `P-builtin-workspaces`, and `P-builtin-auth` moved out of
    `TB-plugin-sandbox` and into `TB-mnemra-host` (Frame correction 1: builtin not
    plugins).
  - DFD typed-element-ID convention preserved (`TB-*` / `EE-*` / `P-*` / `DS-*` / `DF-*`)
    so STRIDE-per-element annotation by the security-mode reviewer at Stage 2 terminal
    review can target each element directly.
  - Threats-by-element, trust-boundaries, accepted-risks tables present as placeholders
    the terminal-review pass populates.
- **2026-05-03 (port source)** — Original constraints draft authored as an
  orchestrator-direct elicitation session against the V0 discovery + architecture
  alignment R6 mental model. Constraint inventory, 6-axis QA tree, 35-domain-question
  triage, DFD draft. A storage-layout strawman was forked off this draft by a researcher
  dispatch (recommended single-document storage layout for V0, kept open here as the
  Spec-stage ADR slot `{{P-StorageLayout}}`).
