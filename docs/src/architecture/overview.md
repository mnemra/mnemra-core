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
  `P-builtin-projects`, `P-builtin-agents`, `P-builtin-workspaces`, and `P-builtin-auth`
  nodes execute as host code. This corrects the predecessor framing where projects and
  agents were drawn as plugins inside `TB-plugin-sandbox`. The corrected framing aligns
  with the locked brief's `0.1.0` substrate description.
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

*Populated by the security-mode reviewer at Stage 2 terminal review per the workspace
threat-modeling skill. Each entry will key on the typed element ID above
(`TB-*` / `EE-*` / `P-*` / `DS-*` / `DF-*`) and apply STRIDE-per-element discipline
(spoofing, tampering, repudiation, information-disclosure, denial-of-service, elevation-
of-privilege). Empty at port-time.*

| Element ID | STRIDE category | Threat | Mitigation candidate (Spec stage) |
|---|---|---|---|
| *(populated at terminal review)* | | | |

## Trust boundaries

*Populated by the security-mode reviewer at Stage 2 terminal review. Trust direction
across each boundary will be annotated at that pass. Listed below for placeholder
naming; entries fill in at threat-modeling time.*

| Trust boundary | Crosses | Direction | Authentication | Authorization |
|---|---|---|---|---|
| `TB-agent-runtime` ↔ `TB-mnemra-host` | MCP stdio transport | bidirectional (request/response) | *(populated)* | *(populated)* |
| `TB-human` ↔ `TB-mnemra-host` | Admin CLI IPC | bidirectional | *(populated)* | *(populated)* |
| `TB-mnemra-host` ↔ `TB-plugin-sandbox` | host-fn ABI (WIT) | host-mediated | *(populated)* | *(populated)* |
| `TB-mnemra-host` ↔ `TB-postgres` | Postgres connection | bidirectional | *(populated)* | *(populated)* |
| `TB-mnemra-host` ↔ `TB-fs-secrets` | Filesystem read | inbound to host | *(populated)* | *(populated)* |
| `TB-mnemra-host` ↔ `TB-fs-source` | Migration filesystem reads (one-shot) | inbound to host | *(populated)* | *(populated)* |
| `TB-mnemra-host` ↔ `TB-fs-backup` | Backup filesystem writes | outbound from host | *(populated)* | *(populated)* |
| `TB-mnemra-host` ↔ `TB-external-llm` | HTTPS to external LLM provider | outbound from host | *(populated)* | *(populated)* |
| `TB-build-pipeline` ↔ `TB-mnemra-host` | Signed plugin artifact verification | inbound to host (signature only) | *(populated)* | *(populated)* |

## Accepted risks

*Populated if threat-modeling identifies risks the Spec stage explicitly accepts. Each
entry will reference its risk-register ID. Empty at port-time.*

| Risk ID | Description | Owner | Trip-wire |
|---|---|---|---|
| *(populated as risks are accepted)* | | | |

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
