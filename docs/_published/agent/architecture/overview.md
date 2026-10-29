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
| PostgreSQL substrate ratified on merits, behind an engine-agnostic swappable `Storage` trait (Postgres the only implementation); V0 engine is **embedded Postgres** (`postgresql_embedded` + bundled `pgvector`); V0 stack is A1-clean (pgvector HNSW + native FTS + recursive CTEs + JSONB), TimescaleDB **demoted** off the V0 stack to a trip-wire | [P-0010-storage-substrate-engine](../adrs/P-0010-storage-substrate-engine.md) (folds the 2026-06-07 storage-engine evaluation, ratified after the spec lock); supersedes the prior "single-process Postgres + `pgvector` + `timescaledb` extensions present" hard-lock framing (brief Hard constraints; architecture alignment R4) |
| **(Amended 2026-10-19, #4018, approved 2026-10-19 (#4018 activity 7800) — supersedes the engine clause of the row above.)** V0 engine is a **separate, vanilla PostgreSQL 16 instance the operator installs and runs**, with `pgvector` installed by the operator; the host reaches it by connection configuration, owns no cluster (no download, `initdb`, `pg_ctl`, data directory or `CREATE EXTENSION`), and **verifies its requirements on the instance before serving, failing closed** — server major and minor floor, pgvector floor, serving role not superuser / no `BYPASSRLS` / no `CREATEROLE` / no `CREATEDB` / no `REPLICATION`, `FORCE ROW LEVEL SECURITY` on tenant-scoped tables, `fsync` / `full_page_writes` / `synchronous_commit` / `data_sync_retry` safe on the sessions writers use, `verify-full` TLS on every TCP connection (loopback included) with a password only ever sent over it, peer authentication with a peer-uid check on sockets, `scram-sha-256` password hashing, server-side parameter logging off, UTF8, minor-version floor, schema version, reachability (r2: two roles — owner for `init`, serving for the host; the serving role owns nothing and is a member of nothing); what it cannot verify (data on persistent storage, backups, install supply chain, `max_connections`, the ident map) it names in a list the host emits and the deployment document carries verbatim | [P-0010](../adrs/P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE1, DE2); V0 substrate spec R-0117, R-0119; maintainer rulings 2026-10-16/18 (#4018) |
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
| Stored context survives a host stop and restart, including a hard kill of the host or of the embedded engine; data lives outside the executable and its image at an operator-configured location (OS per-user app-data default for a bare binary, overridable; for a container, a volume the operator mounts at the configured location, which the host can be told to require), never a temporary directory or a container's writable layer; whether the location is local or network-backed is the operator's choice; the host reaches the cluster only over a private unix socket by peer authentication, so no cluster credential is held at rest | Brief Success criteria "Durable across restart" + Hard constraints "Data lives outside the executable and its image" (maintainer ruling 2026-09-15); V0 substrate spec R-0116; the Reliability / durability axis below |
| **(Amended 2026-10-19, #4018, approved 2026-10-19 (#4018 activity 7800) — supersedes the row above.)** Stored context survives a host stop and restart, clean or hard kill (spec R-0121-a, verified out-of-process, R-0121-c); the instance's data directory, placement, locking and version are the operator's and PostgreSQL's — placement on persistent storage (never a container's writable layer or a temporary directory) is a **named operator obligation** (R-0117-j item 1), and the durable-commit settings are **requirements the host verifies on every start and retry** (R-0117-d). Survival across a crash of the instance itself is PostgreSQL's WAL guarantee under those settings; the host names the reliance and does not test it (R-0121-b). The persistence fix is a **correction** of mnemra's own `.temporary(true)` (maintainer 2026-10-18), not a benefit of the move. The credential at rest, where one exists, is a `0600` file under the host uid, never the URL or environment; peer authentication over a socket is the recommended local shape (R-0119) | [P-0010](../adrs/P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE5, DE7); V0 substrate spec R-0117-d/-j, R-0119, R-0121; maintainer ruling 2026-10-18 (#4018 activity 7472) |
| **Startup check, readiness and liveness are three contracts on three surfaces (Amended 2026-10-19, #4018).** An unmet hard requirement on the instance never exits the process: the host stays up, serves nothing, reports why on `GET /ready`, retries with backoff (1 s → 30 s cap) and on `SIGHUP`; `GET /health` reports only process liveness and never depends on the instance; `mnemra check-db` runs the check out-of-process. Probe mapping: liveness → `/health`, readiness → `/ready`, startup → `/health` or none | [P-0010](../adrs/P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE3, DE4); spec R-0118; maintainer ruling 2026-10-19 (#4018 activity 7749); the three-contract split RULED 2026-10-19 (activity 7787; `ready` ⇒ `alive`, never the reverse — AE1); no soft class at V0 (ruling 7771) |
| Workspace cutover from the prior tooling at V0; the prior task-store binary remains available as a rollback path | V0 discovery |
| Rust-default toolchain; ecosystem-aligned tooling (non-Rust adopted only when no viable in-stack path exists) | Brief Hard constraints; workspace stack-discipline principle |
| Test-Driven Development pairs on non-trivial work; worktrees mandatory; main is protected | Workspace test/worktree/main-protection principles |
| Migration scope: the prior structured task-store tables (content / timeseries / log / state partitions) and a fixed subset of prior content-corpus subdirectories | V0 discovery (Migration scope) |
| Workspace-level operational tooling (skill files, team profiles, internal memory store, inboxes, scratch, agent-harness state) is V0.1+ scope (the brief's idea-tier internal-workspace-absorption entry) | V0 discovery (Migration scope); brief idea-tier |
| Architecture MUST NOT be schedule-pressured; marketing-tier dates are not architectural inputs | Brief Hard constraints |
| The system MUST NOT host a **generative** LLM; generative work (query rewrite, chunk-context, tag generation, synthesis) calls out to an external model at V0; local **non-generative** inference (embedding, reranking — encoder models behind the host-fn seam) is permitted host-side *(MODIFIED 2026-07-02 per RC-1, retrieval-cluster intake; was: "MUST NOT host a language model; embeddings call out to an external model")* | Brief Hard constraints + Non-goals, as amended 2026-07-02 (RC-1) |

### Reframed 2026-06-08 — storage substrate re-opened on merits

The storage substrate was originally an unexamined hard-locked carry-forward. A
storage-engine evaluation (ratified 2026-06-07, after the 2026-05-24 substrate spec lock)
re-opened it on merits and is folded in here: **PostgreSQL ratified on merits, behind an
engine-agnostic swappable `Storage` trait** (one implementation), **V0 embedded Postgres**,
**A1-clean V0 stack**, and **TimescaleDB demoted** off the V0 stack to a latency/storage
trip-wire (the V0 time-series *storage shape* is plain timestamped Postgres tables). The
substrate constraint row above is updated; the decision lives in
[P-0010-storage-substrate-engine](../adrs/P-0010-storage-substrate-engine.md).

**Observability re-derived 2026-06-09 (E1 dispositioned = re-derive now).** D8's demote
reached the observability metrics/events surfaces, which P-0004 had committed to TimescaleDB
hypertables in the app's own Postgres. The maintainer dispositioned that collision (escalation
E1 in P-0010) by separating observability **generation** from **storage**: the server EMITS
telemetry (stdout structured logs + OTel metrics/events + health-endpoint-first) from the bare
shell, storage-independently; it does NOT own where telemetry lands. The observability storage
backend is deferred behind the separation (option set {Prometheus, InfluxDB, TimescaleDB, plain
Postgres tables}, named tripwire), not locked; the standalone binary survives (observability
storage is external operator infra). This was **re-altituded out of the project-ADR layer**:
observability is a theory trait (the host emits its event stream for infra; never owns the
service's own observability store) plus chassis mechanics, not a per-project ADR. The decision
lives as the **[observability baseline](#observability) in this overview** (a theory-trait
baseline); the original observability ADR [P-0004](../adrs/P-0004-observability-shape.md) is
`deprecated` (its storage core falsified by D8; no successor ADR). The data-flow diagram (former
`DS-ts-*` / `DS-pg-logs` nodes → a telemetry-egress sink), the quality-attribute observability
scenarios, and the observability threat rows are re-derived around emission below — there is no
in-app observability store at V0.

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

Seven axes. Four of these are workspace core values; one is the workspace supporting value
Reliability, scored as its own axis once the brief defined what "persistent" means
(brief Success criteria "Durable across restart", maintainer ruling 2026-09-15); two are mnemra-specific
non-functional requirements that surfaced as load-bearing in V0 discovery. Reversibility
is folded into conflict-resolution discipline (cutover rollback path preserved per V0
discovery WC.x) rather than scored as its own axis.

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
| Foreign-key preservation | Post-migration, task artifacts reference valid migrated project artifacts across the content and state-config partitions. *(Re-derived 2026-06-09 per the [observability baseline](#observability): the former timeseries/log partitions are observability emission surfaces, not in-app storage; dispatch metrics/events are emitted, not stored as FK-bearing in-app rows.)* |

### Observability

**Observability baseline (theory trait — re-altituded 2026-06-09, E1 dispositioned).** Observability is a base-platform **theory trait plus chassis mechanics, not a per-project ADR**: the host emits its event stream for infra (stdout structured logs + OTel metrics/events + a health endpoint), storage-independently from the bare shell, and **never owns the service's own observability store**. Generation is separated from storage — the server emits; *where* telemetry lands is the operator's choice behind the separation (option set {Prometheus, InfluxDB, TimescaleDB, plain Postgres tables}, named tripwire), deferred and not in the binary at V0. This baseline is the home for the V0 observability generation decisions; the original observability ADR [P-0004](../adrs/P-0004-observability-shape.md) is `deprecated` (no successor ADR — its storage core was falsified by D8). The scenarios below are generation-side — they hold whatever sink, if any, the operator chooses.

> **Storage-backend tripwire (named instrument, P-Defer/DF1).** A persistent observability store is adopted when persistent observability storage becomes load-bearing for a real operator deployment (telemetry must survive process restart / be queried historically beyond what stdout retention + the scrape target already provide). Until then the bare shell carries none. The *generation mechanism* (the emission call sites, the metric/event/log floor) routes to the chassis when `chassis new` lands; until then it is a documented invariant on the build.

> **Capability-manifest invariant (settled — generated-from-WIT, never hand-maintained).** The host's capability manifest — the machine-readable surface describing the host functions and verbs the host exposes — is **generated from the WIT interface definitions, never hand-maintained**, so it cannot drift from the actual ABI. This is a settled invariant at V0, not a live fork. Its *generation mechanism* (the build-time WIT → manifest step) routes to the chassis with a named tripwire: when `chassis new` exists, the manifest-generation step lands there; until then it is a documented invariant on the build. (Distinct from the per-plugin signed manifest at [P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md) — that is a plugin's *declaration* of the surface it requires; this is the host's *generated* description of the surface it offers.)

| Refinement | Scenario |
|---|---|
| Per-verb metrics (emitted) | After a dogfood session, the per-verb metric records emitted to the OTel/stdout surface carry `workspace_id`, `verb`, `outcome`, `duration_ms`; p50 / p95 / p99 per verb are derivable from the emitted `duration_ms` values (computed at the operator-chosen sink — there is no in-app metrics hypertable). |
| Storage-independent emission | With no persistent observability backend configured, N MCP verb calls produce N observable metric emissions on the OTel/stdout surface; `\d+` shows no metrics/events hypertable and `\dx` does not list `timescaledb`. |
| Generation at the bare shell | A structured log line appears on stdout and `GET /health` returns a response *before* the embedded Postgres is initialized (health endpoint is the first API). *(Amended 2026-10-19, #4018: "before the embedded Postgres is initialized" → "before the first connection to the operator's instance is attempted, and regardless of whether that instance is reachable".)* |
| Health-endpoint shape | A request to `/health` returns a structured detail body identifying which dependency failed (Postgres reachable / `pgvector` loaded / workspace=default exists), e.g. 503 when a dependency is down. The listener binds loopback-only (`127.0.0.1`) at V0, so **loopback IS the gate** — every caller is necessarily on loopback and receives the detail body; there is no admin-token gating at V0. *(Named tripwire: if the `/health` listener ever binds non-loopback, admin-token gating on the detail body becomes required.)* *(Amended 2026-10-19, #4018 — superseded by the two rows below; the loopback gate and its tripwire carry to both routes.)* |
| Liveness is independent of the instance (Amended 2026-10-19) | *Stimulus:* the operator's Postgres is stopped for ten minutes while an orchestrator probes `GET /health` every 10 s. *Environment:* the host running under a liveness probe with a restart threshold of three failures. *Response:* every probe returns `200 { "alive": true }`; the handler issues no SQL. *Measure:* zero restarts over the ten minutes; `GET /ready` returns `503` with `reasons[0].reason = "unreachable"` throughout. (Spec R-0118-e/-f.) |
| Readiness carries the reason and recovers without a restart (Amended 2026-10-19) | *Stimulus:* the instance is started after the host. *Environment:* a host that came up against a stopped instance, with an MCP client already connected over stdio that received `host_not_ready` on its first call. *Response:* the host's scheduled check succeeds, readiness flips to `200`, the same client succeeds on its next call. *Measure:* the host pid is unchanged; the gap between instance start and `ready: true` is ≤ the current backoff interval (≤ 30 s) plus one check duration; a `SIGHUP` closes it within 1 s. (Spec R-0118-b/-c/-d/-h.) |
| Measured 2026-10-19 at `57cfeca`: one route serves all three questions | `libs/mnemra-host/health.rs:623-639` answers `503 {"ready": false}` until `mark_ready`, then serves `schema::init::health_snapshot` (`schema/init.rs:757`), which runs `SELECT 1` and reports `overall: "down"` on an unreachable instance — and returns HTTP `200` for it (`health.rs:627-629`). A liveness probe on this route restarts the process on a database outage. Recorded as the measured starting point the amendment's split corrects. |

### Reliability / durability (workspace supporting value — restart survival)

The brief's "Durable across restart" success criterion and "Data lives outside the
executable and its image" hard constraint (maintainer ruling 2026-09-15) are the
product-tier inputs; the V0 substrate spec's R-0116 is the requirement. Scenarios are written as
`[stimulus, environment, response, measure]`. The stated reliability target for this axis
(indicator, objective and window, what a breach obliges): **indicator** — the fraction of
writes acknowledged to a caller before a stop that read back with identical content after
the next start, read from the caller's side over the MCP surface; **objective** — 100% on
every restart, with no window because the property is per-restart, not a rate;
**breach** — the restart acceptance gate (`verify-smoke`) fails and blocks merge, and in
operation a start that cannot honor the invariant refuses to serve (version mismatch,
data directory in use) rather than serving a partial or empty store. No availability
(uptime) target is stated for the host at V0; that absence is recorded here as a
decision, not a blank.

| Refinement | Scenario |
|---|---|
| Clean stop, then restart | *Stimulus:* SIGTERM to a host holding N acknowledged artifact writes. *Environment:* the production binary against an operator-configured data directory (`MNEMRA_DATA_DIR`), not a temporary one. *Response:* the host drains, stops the embedded postmaster, exits; a new host process starts against the same directory without re-initializing the cluster. *Measure:* every one of the N artifacts reads back via `artifact.get` with `frontmatter` and `body` equal to the pre-stop read, and the cluster's `system_identifier` is unchanged across the two starts. |
| Hard kill of the host | *Stimulus:* SIGKILL to the host process. *Environment:* as above; the embedded postmaster, daemonized by `pg_ctl`, survives the host and holds `postmaster.pid` against the data directory. *Response:* the next host start reclaims ownership from the dead same-boot holder recorded in `host.lock`, identifies the orphan as a live postmaster only on a full match (host uid, `-D <data_dir>/pg` in argv, start time matching the pidfile), stops it cleanly, and starts its own; a pid that fails the match is never signalled and the start refuses instead. *Measure:* every acknowledged pre-kill write reads back equal; exactly one postmaster runs against the directory afterwards; the second start does not re-initialize; an unrelated process holding a reused pid survives. |
| Hard kill of the storage engine | *Stimulus:* SIGKILL to the embedded postmaster, leaving a stale `postmaster.pid` (dead PID) and unflushed OS buffers. *Environment:* Postgres durability defaults intact (`fsync`, `synchronous_commit`, `full_page_writes` all `on`). *Response:* the next host start runs WAL crash recovery and serves. *Measure:* every write acknowledged before the kill reads back equal; no write acknowledged is missing. |
| Engine major-version mismatch | *Stimulus:* a host whose pinned engine major differs from the `PG_VERSION` of an existing data directory. *Environment:* an operator upgrading the binary in place. *Response:* the host refuses to start before any postmaster start, with a structured error naming the directory, the found major and the engine major, and touches nothing under the directory. *Measure:* the refusal is the host's own error, the directory's bytes are unchanged, and no second (empty) cluster is initialized beside the old one. |
| Two hosts, one directory | *Stimulus:* a second host started against a data directory a live host already owns — on the same machine, or on another machine sharing a network mount. *Environment:* any deployment shape; on a network mount the lock semantics may be local-only, so the lock alone is not the fence. *Response:* the second host exits with a structured `data_dir_in_use` error — on the same machine because the lock is held, on another machine because the `host.lock` holder record carries a foreign hostname and boot id, which is refused without any liveness test; the first host and its postmaster are undisturbed. *Measure:* the first host's writes continue to read back; exactly one postmaster runs against the directory; the holder record is unchanged. |
| Replace the executable | *Stimulus:* between the stop and the start, the executable is replaced — minimally, the same binary run from a different filesystem path. *Environment:* a bare-binary deployment; the container form (recreate from a different image, same volume) is parked with the image contract in the spec's Out of Scope until an image exists. *Response:* the new host starts against the same data location. *Measure:* every acknowledged pre-stop write reads back equal (spec R-0116-j, locked in the bare-binary form by maintainer ruling 2026-09-17). |

**Amended 2026-10-19 (#4018, approved 2026-10-19 (#4018 activity 7800)) — re-derived against the operator-run instance.** The reliability target above stands with one change to its breach clause: in operation a host that cannot honour the invariant **refuses to serve and reports why on `GET /ready`** rather than refusing to start. Of the six scenarios above: "Clean stop, then restart" and "Hard kill of the host" carry with the response rewritten below; "Hard kill of the storage engine" is **dropped with reason** (the host cannot kill an instance it does not own; the guarantee is PostgreSQL's under settings the host verifies — spec R-0121-b); "Engine major-version mismatch", "Two hosts, one directory" and "Replace the executable" have no object (no host-owned directory; the version guard becomes R-0117-a/-h; two hosts on one database is superseded by the shared-service HTTPS ruling, the next amendment #4127, and unsupported meanwhile — P-0010 §Amendment AE9). The rows above are retained as the embedded-engine record.

| Refinement (Amended 2026-10-19) | Scenario |
|---|---|
| Clean stop, then restart | *Stimulus:* SIGTERM (or stdin EOF) to a host holding N acknowledged artifact writes. *Environment:* the production binary against the operator's instance, `MNEMRA_DATABASE_URL` pointing at one database. *Response:* readiness goes `503 { "draining" }`, the host drains and exits; a new host starts against the same URL, its instance check reports every hard requirement met and no migration pending, and it serves. *Measure:* every one of the N artifacts reads back via `artifact.get` with `frontmatter` semantically equal and `body` byte-equal to the pre-stop read (spec R-0121-a/-c). |
| Hard kill of the host | *Stimulus:* SIGKILL to the host process. *Environment:* as above. *Response:* the next host start connects, checks, and serves; nothing on the instance needed reconciling, because the host owned no process there. *Measure:* every acknowledged pre-kill write reads back equal; an unacknowledged in-flight write may be absent (R-0121-a). |
| Durability setting drifts while serving | *Stimulus:* an operator (or a role-level default) sets `synchronous_commit = off` for the host's role. *Environment:* a serving host with the 30 s drift re-check. *Response:* within one interval the check reports `durability_off`, readiness goes `503` naming the setting and value, every MCP call answers `host_not_ready`; after the setting is restored and `SIGHUP` is sent, the host serves again. *Measure:* no write is acknowledged under the weakened setting for longer than 30 s plus one check duration; the host pid is unchanged throughout (R-0117-d, R-0118-c/-d). |
| Over-privileged role or unforced RLS | *Stimulus:* the configured role is a superuser, holds `BYPASSRLS`, or a tenant-scoped table has `NO FORCE ROW LEVEL SECURITY`. *Environment:* any start or retry. *Response:* not ready with `role_overprivileged { attribute }` or `rls_not_forced { table }`; nothing is served. *Measure:* zero MCP verbs execute while the condition holds; `mnemra check-db` exits `1` naming the clause (R-0117-c/-e; #3591's RLS precondition, verified rather than assumed). |
| Unverifiable is unmet | *Stimulus:* the role's `SHOW fsync` is denied. *Environment:* an instance whose GUC visibility has been restricted. *Response:* the clause reports `unverifiable`, which the host treats as unmet; not ready, reason names the setting and the error. *Measure:* the host never serves on an assumed value; a unit test over injected facts shows one failing case per clause (R-0118-a). |

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
  # Amended 2026-10-19 r2 (#4018): the single P-health-handler is split into liveness and
  # readiness, with the instance check as the one process that talks to the database.
  P-liveness-handler: {
    label: "P-liveness-handler\n(GET /health; non-runtime thread; never touches the DB)"
  }
  P-readiness-handler: {
    label: "P-readiness-handler\n(GET /ready; reads the last completed check)"
  }
  P-instance-check: {
    label: "P-instance-check\n(facts probe + decision; serving role; bounded, supervised)"
  }
  P-cli-check-db: {
    label: "P-cli-check-db\n(mnemra check-db; second credential-holding client)"
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

# Amended 2026-10-19 r2 (#4018): TB-postgres is RETYPED IN PLACE from "the single-process
# cluster the host owns" to "the operator-run instance — the operator's zone". No second
# boundary is added; the five typed flows into it are kept. The data stores are unchanged.
TB-postgres: {
  label: "TB-postgres (operator-run PostgreSQL instance; the operator's zone)"
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
  EE-other-principals: {
    label: "EE-other-principals\n(other roles, apps or tenants on a shared or managed instance)"
    shape: oval
  }
}

# Observability telemetry is EMITTED, not stored in-app (observability baseline; generation⊥storage).
# The host emits structured logs (stdout) + OTel metrics/events; the sink is external,
# operator-chosen, and deferred behind the separation (not in the binary at V0).
TB-obs-sink: {
  label: "TB-obs-sink (external, operator-chosen; deferred — obs baseline)"
  shape: rectangle
  style.stroke-dash: 3

  EE-stdout-otel: {
    label: "EE-stdout-otel\n(stdout logs + OTel metrics/events egress)"
    shape: oval
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
  # Amended 2026-10-19 r2 (#4018): the two database credentials, each a file under the host uid
  # (or a root-owned group-readable Secret mount), fstat-checked on every read (spec R-0119-b/-g).
  DS-db-password-serving: {
    label: "DS-db-password-serving\n(MNEMRA_DATABASE_PASSWORD_FILE)"
    shape: cylinder
  }
  DS-db-password-owner: {
    label: "DS-db-password-owner\n(MNEMRA_DATABASE_OWNER_PASSWORD_FILE; init only)"
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

# Telemetry egress (generation⊥storage): host EMITS metrics/events/logs to the external
# operator-chosen sink (stdout + OTel). No in-app observability store at V0 (obs baseline).
TB-mnemra-host.P-host-fns -> TB-obs-sink.EE-stdout-otel: "DF-telemetry-emit"

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
# Migration emits progress/audit telemetry via the normal emission path (egress, not
# in-app storage). The former DS-ts-* / DS-pg-logs migration-write edges are removed
# (no in-app observability store at V0 — obs baseline).
TB-mnemra-host.P-migration-handler -> TB-obs-sink.EE-stdout-otel: "DF-telemetry-emit"

# Backup handler → backup filesystem
TB-mnemra-host.P-backup-handler -> TB-postgres.DS-pg-content: "DF-backup-read"
TB-mnemra-host.P-backup-handler -> TB-fs-backup.DS-fs-backup: "DF-backup-write"

# Signing (build-time → runtime signature verify)
TB-build-pipeline.EE-mnemra-root -> TB-mnemra-host.P-plugin-runtime: "DF-signing-attest"
TB-build-pipeline.EE-mnemra-root -> TB-fs-secrets.DS-mnemra-root-key: "DF-key-custody\n(open ADR slot)" {
  style.stroke-dash: 3
}
TB-mnemra-host.P-plugin-runtime -> TB-fs-secrets.DS-mnemra-root-key: "DF-signature-verify"

# Instance check (Amended 2026-10-19 r2, #4018): the facts probe is the one process that reaches
# the database for health; readiness reads its last completed result; liveness reaches nothing.
TB-mnemra-host.P-instance-check -> TB-postgres.DS-pg-content: "DF-health-probe"
TB-mnemra-host.P-readiness-handler -> TB-mnemra-host.P-instance-check: "DF-ready-read\n(last completed result; no DB)"
TB-mnemra-host.P-instance-check -> TB-fs-secrets.DS-db-password-serving: "DF-db-password-read"
TB-mnemra-host.P-host-fns -> TB-fs-secrets.DS-db-password-serving: "DF-db-password-read"

# Operator (Amended 2026-10-19 r2, #4018): forced retry by signal; the pre-flight CLI as a second
# credential-holding client; the one-time setup SQL as operator-privileged DDL into the zone;
# mnemra init (P-migration-handler) under the owner credential.
TB-human.EE-operator -> TB-mnemra-host.P-instance-check: "DF-sighup\n(forced retry; password reload)"
TB-human.EE-operator -> TB-mnemra-host.P-cli-check-db: "DF-cli-invoke"
TB-mnemra-host.P-cli-check-db -> TB-postgres.DS-pg-content: "DF-health-probe"
TB-mnemra-host.P-cli-check-db -> TB-fs-secrets.DS-db-password-serving: "DF-db-password-read"
TB-human.EE-operator -> TB-postgres: "DF-setup-sql\n(roles, grants, CREATE EXTENSION; operator's privilege)"
TB-mnemra-host.P-migration-handler -> TB-fs-secrets.DS-db-password-owner: "DF-db-password-read"
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
- **The persisted storage shapes are visible; observability is emitted, not stored in-app.**
  Content (`DS-pg-content`) and state (`DS-pg-state`) substrates are regular Postgres tables —
  the two persisted shapes at V0. *(Re-derived 2026-06-09 per the [observability
  baseline](#observability):
  the former timeseries (`DS-ts-metrics`, `DS-ts-events`) and log (`DS-pg-logs`) shapes are
  observability **emission** surfaces — the host emits structured logs to stdout and OTel
  metrics/events to a configurable export, drawn as the external `TB-obs-sink` telemetry-egress
  node. They are NOT in-app Postgres storage; the storage backend is the operator's choice
  behind the generation⊥storage separation, deferred and not in the binary at V0.)*
- **The trust boundary `TB-build-pipeline` is conceptual.** The signing authority operates
  at build time; runtime sees the signature, not the key. Custody of the actual key
  material (whether it lives on the deployment node, in an HSM, is fetched at runtime, or
  is never on the node at all) is the open ADR slot `{{P-SigningKeyCustodyHardening}}`
  (Tier C; activated by the `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)` multi-deployment trip-wire).
- **External LLM is for generative work only; embeddings and reranking are local
  (MODIFIED 2026-07-02 per RC-1 — the retrieval-cluster intake's model-hosting
  amendment; this bullet is the named lagging copy of the brief's ELT external-embedding
  framing, reconciled here).** Per the amended Hard constraints, mnemra-core MUST NOT
  host a *generative* LLM; local *non-generative* inference (BGE-M3 embedding,
  BGE-reranker reranking — encoder models behind the host-fn seam) runs host-side and
  never egresses. What calls out to the external provider are the four generative
  placements of the retrieval cluster (index-time chunk-context and tag generation;
  query-time HyDE rewrite and optional synthesis), each policy-gated at the model-egress
  gate, individually disable-able, and bounded — the zero-egress configuration (all four
  OFF) is a supported V0 configuration. The API key configuration surface remains folded
  into `0.1.0` (brief T-5 resolution) and now serves the generative call-outs. The drawn
  DFD below still shows the pre-RC-1 `DF-embed-call` flow and the `EE-llm-provider`
  "(embeddings endpoint)" label; the retyped elements for the retrieval cluster — the
  four `DF-egress-4.x` flows crossing `TB-external-llm`, the `EE-model-artifact-source`
  entity, and the new retrieval processes/stores — are recorded in
  [P-0014's typed-DFD extension](../adrs/P-0014-retrieval-architecture.md), and the
  diagram + threat tables are re-drawn against it when that cluster's
  pre-implementation security review updates this overview (the named follow-up; the
  prose in this file is reconciled now). MCP sampling (`DF-sampling-up`) is the separate
  path by which plugins ask the connected agent's MCP client to run an LLM completion;
  the LLM provider that completion uses is external to mnemra-core.

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
| `EE-orchestrator-agent` | S | An MCP client process forges an orchestrator identity by presenting a valid token leaked from another agent's keystore or shell history; the host has no way to bind tokens to a specific process. | High | 75 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (token-scoping + per-agent token discipline); workspace-claim-in-token enforcement at `P-builtin-auth` |
| `EE-orchestrator-agent` | R | An orchestrator dispatches a destructive verb (e.g., bulk delete) and later denies the action; without per-token attribution on every write, the audit trail names the workspace but not the issuing agent. | High | 80 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` + activity-log capability (`0.5.0`) carrying `(workspace_id, agent_id, session_id)` tuple on every write |
| `EE-specialist-agent` | S | A specialist runs under a delegated session and is impersonated by another specialist sharing the same workspace token; sub-agent identity is not distinguishable from the orchestrator's identity at the host. | Medium | 70 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (per-agent token derivation); designed-for at session model |
| `EE-specialist-agent` | R | A specialist's destructive write is not attributable beyond "agent inside workspace X" because the MCP session does not name the specific specialist. | Medium | 70 | Same as `EE-orchestrator-agent`/R |
| `EE-operator` | S | A second user on the deployment host reads the admin token file (file-mode regression, backup-tooling leak, accidental world-readable mode after restore) and impersonates the operator. | Critical | 80 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (token rotation, file-mode invariant check at startup); `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)` informs the V0 secrets-management posture; `{{P-SigningKeyCustodyHardening}}` for production-grade secrets posture |
| `EE-operator` | R | An operator runs a destructive admin operation (`drop-workspace`, force-restore-overwrite) and denies it; the operator's identity is the local UNIX user — no second factor, no audit trail beyond OS audit. | High | 75 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (admin-action audit log; tie destructive ops to a distinct audit event); accepted-risk R-0006 if deferred |
| `EE-mnemra-root` | S | A compromised CI environment forges a "mnemra-root-signed" plugin artifact, and the runtime accepts it because signature verification uses a key the attacker controls. | Critical | 70 | `{{P-SigningKeyCustodyHardening}}` (key isolation; ceremony; offline-root pattern); separate from runtime entirely |
| `EE-mnemra-root` | R | A signing event is non-repudiable only if signing is logged; absent a transparency log of signed plugins, a malicious "core: true" plugin could be denied as ever having been signed. | Medium | 60 | `{{P-SigningKeyCustodyHardening}}` (transparency log; sigsum or rekor-style append-only signing log) |
| `EE-llm-provider` | S | The host fetches embeddings from a TLS endpoint that an attacker has positioned as the configured provider (DNS rebind, typo-squatted hostname, untrusted CA injection); embeddings are tampered or harvested for content-corpus reconstruction. | High | 70 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` + outbound HTTPS pinning policy (cert pin or trusted-CA bundle declared in config); LLM-API-key surface in `0.1.0` ships hostname allowlist |
| `EE-llm-provider` | R | The provider could later deny logging request metadata (artifact bodies / fragments / dispatch traces inferable from embed inputs); the deployment has no independent receipt. | Low | 60 | Accepted-risk R-0005 (telemetry no-leak is dogfood AC, not policy-enforced) |
| `P-mcp-handler` | S | A bogus MCP client presents a forged token; the handler accepts because it does not bind the token to an authenticated session and does not consult `P-builtin-auth` on every request. | Critical | 85 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (mandatory `DF-auth-check` on every request; per-request token verification with replay defense) |
| `P-mcp-handler` | T | A stdin-fed MCP request injects oversized or malformed JSON-RPC that breaks the parser into a non-conforming state; subsequent requests on the same stream are routed incorrectly. | High | 70 | `{{P-MCPWriteSemantics}}` (input-size cap, framing-error fail-shut, structured `parse_error` JSON-RPC code); explicit MCP 2025-06-18 input validation *(Amended 2026-10-29, #4249, ruling #4249 activity 9183: a line that is not valid JSON is answered with JSON-RPC `-32700`, `id` null, and the session stays open, so a framing error does not fail shut; only a line over the input-size cap ends the session. The host does not send that reply yet; #4323 adds it. [P-0012](../adrs/P-0012-plugin-runtime-and-mcp-sdk.md) §Amendment 2026-10-25 (MCP protocol revision).)* *(Refined 2026-10-29, #4249 activities 9244 and 9258: read "a framing error" in the note before this one as "a malformed line", meaning a line that is not valid JSON. A line over the input-size cap is a framing fault too, and it does fail shut. An empty line is not a message and gets no reply.)* |
| `P-mcp-handler` | I | A correctly-authenticated workspace-A token issues a verb whose handler's query was malformed; without RLS policy enforcement at V0, workspace-B rows surface in the response. | Critical | 85 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (RLS policy enforcement at V0.1+; column-shape at V0); pre-mitigation: a workspace-scope-on-every-query lint and integration test in the host-fn surface |
| `P-mcp-handler` | D | An adversarial client floods stdio with verbs that exercise the most-expensive code path (vector search, projection rebuild) — single connection, no rate limit. | High | 75 | `{{P-MCPWriteSemantics}}` (per-session rate limit; cost-aware queueing); observability ships from `0.1.0` so the floor is detectability |
| `P-mcp-handler` | E | An MCP verb invokes a host function that was meant for control-plane only (e.g., a debug-only "dump tokens" verb left exposed by a builtin's manifest mis-declaration); the agent gains operator-level capability. | High | 65 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` (verb-to-capability allowlist; control-plane verbs not exposed via MCP); compile-time enforcement preferred |
| `P-cli-handler` | S | A second process on the host invokes the CLI as the operator user; OS-level checks alone (the UID match) accept it. | Medium | 70 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (admin token mandatory for destructive ops; OS-uid alone is insufficient gate) |
| `P-cli-handler` | T | A CLI argv injection arises when a subcommand interpolates a parameter into a Postgres role-management call (`role-grant`, `workspace-create`) without escaping. | High | 70 | `{{P-MCPWriteSemantics}}` covers MCP write semantics; CLI surface needs its own parameter-binding discipline — surface as `{{P-AdminCLIDiscipline — new}}` |
| `P-cli-handler` | E | A schema-driven dynamic subcommand generated from a malicious or mis-declared plugin manifest exposes a destructive op (`migrate-overwrite`) at the operator CLI without intent. | High | 70 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` (manifest validation; capability tier on subcommands; destructive ops behind an explicit confirmation gate); `core: true` only for the V0 dogfood path |
| `P-host-fns` | T | A host-fn writes content with caller-supplied `workspace_id` rather than deriving it from the session — a plugin (or builtin) supplies an attacker-chosen value and writes into a foreign workspace. | Critical | 90 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` + `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (host derives `workspace_id` from the session/token; ABI MUST NOT accept `workspace_id` as a parameter on write paths); structural invariant |
| `P-host-fns` | I | A read-side host-fn returns rows that pass workspace-scope but include columns the plugin has no manifest declaration for; cross-content-type leak inside one workspace. | High | 70 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` (per-plugin column projection; manifest declares readable columns; host-fn enforces) |
| `P-host-fns` | E | A host-fn intended for a `core: true` plugin (e.g., `mnemra.workspace.add_user`) is callable from a non-core plugin because the ABI does not key on plugin identity. | Critical | 80 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` (per-plugin host-fn allowlist keyed on signed manifest; non-core plugin's allowlist is a strict subset) |
| `P-host-fns` | R | Embedding-call (`DF-embed-call`) traffic carrying artifact-derived inputs lacks a structured trail tying which workspace's content was sent to which external request. | Medium | 65 | [observability baseline](#observability) (per-egress audit event *emitted* with `(workspace_id, artifact_id, provider_hostname)` — generation-side, lands in the operator-chosen sink) |
| `P-migration-handler` | T | Migration writes to substrate tables (`DS-pg-content`, `DS-pg-state`) using a privileged DB role; if migration is re-runnable, an attacker who can trigger it could mass-corrupt content under cover of "resume from checkpoint." *(Re-derived 2026-06-09: the former `DS-ts-*`/`DS-pg-logs` in-app stores are gone — migration emits progress/audit telemetry via the egress path per the observability baseline, not in-app storage writes.)* | High | 65 | `{{P-MigrationID}}` + `{{P-CutoverDualWrite}}` (migration gated on admin-token capability; idempotency keyed on per-record manifest, not "rerun whole import") |
| `P-migration-handler` | I | Migration reads source frontmatter that may contain credentials, private-note content, or secrets embedded in markdown; if the migration log captures source content (for debugging), the log becomes a sink for sensitive data. | High | 70 | `{{P-MigrationID}}` + [observability baseline](#observability) (migration log emits IDs and per-record outcome only, never content; redaction at the emission boundary; policy named in the migration spec) |
| `P-migration-handler` | D | A SIGKILL mid-migration leaves the substrate in a partial state; a re-run that did not consult the per-record progress manifest could re-process records and exhaust connection-pool / disk-space budgets. | Medium | 75 | `{{P-MigrationID}}` (per-record progress manifest mandatory; resume reads manifest first) |
| `P-backup-handler` | I | The backup stream contains every row of every substrate (including tokens, secrets-stored-as-content, dispatch artifacts) — the backup file is a higher-value compromise target than the live DB. | Critical | 85 | `{{P-BackupRestore}}` (backup encryption-at-rest mandatory; key separate from `DS-admin-token`); access-control on `DS-fs-backup` path |
| `P-backup-handler` | T | An attacker with write access to `DS-fs-backup` poisons a backup that a future restore consumes (introducing a malicious admin token, a forged workspace, or rewritten `core: true` plugin metadata). | Critical | 80 | `{{P-BackupRestore}}` (backup signed at write; restore verifies signature; round-trip-verify before destructive operations) |
| `P-backup-handler` | D | An adversary triggers continuous backups (CLI-exposed or scheduled-job-exposed) to exhaust disk space, taking the substrate read-only and the host degraded. | Low | 60 | `{{P-BackupRestore}}` (backup-trigger rate-limit; disk-space precondition check) |
| `P-health-handler` | I | A health-endpoint detail body leaks substrate state to an unauthenticated probe (extension list, table presence, workspace count). | Medium | 70 | [observability baseline](#observability) (health response shape: the listener binds loopback-only at V0, so **loopback IS the gate** — no non-loopback caller can reach the detail body; named tripwire: if the listener ever binds non-loopback, admin-token gating on the detail body becomes required) |
| `P-health-handler` | S | The health endpoint is unauthenticated and shares a transport with MCP; a load-balancer probe could be fabricated and used as an oracle for substrate state. | Low | 55 | [observability baseline](#observability) (separate handler binding for `/health`, the first API on a dedicated loopback-only listener; not on MCP transport at V0 since transport is stdio) |
| *(Amended 2026-10-19 r3, #4018 — the two `P-health-handler` rows above are retargeted, not orphaned: the element is split (see the r2 retype subsection).)* `P-readiness-handler` | I | The same leak as the first row, now on `GET /ready`'s `checks` body (clause statuses, reasons, guidance). | Medium | 70 | Loopback-only bind is still the gate; the MCP surface gets only the coarse `host_not_ready` (spec R-0118-h); the non-loopback tripwire carries (R-0004-g) |
| `P-liveness-handler`, `P-readiness-handler` | S | The same fabricated-probe oracle as the second row, now on two routes. | Low | 55 | Loopback-only listener; `/health` carries no substrate state at all (R-0118-e), so the oracle value of a fabricated liveness probe is nil |
| `P-instance-check` | I | The check's `instance_check` log record carries clause statuses and guidance to the structured-log sink. | Low | 60 | Never carries credentials or the URL's secrets (R-0117-i `detail` rule, R-0119-a logging rule); the sink is the operator's (R-0004-d) |
| `P-plugin-runtime` | T | A WASM module exploits a Wasmtime sandbox-escape primitive (rare, but the load-bearing assumption — sandbox integrity — is exactly what plugin-IO-free buys); host invariants violated. | High | 50 | `[P-0007-plugin-resource-limits](../adrs/P-0007-plugin-resource-limits.md)` + `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)` (Wasmtime version pinning under SBOM; `core: true` only at V0; supply-chain review on Wasmtime upgrades); see also `{{P-SigningKeyCustodyHardening}}` for production-grade supply-chain hardening |
| `P-plugin-runtime` | E | A plugin whose signature verification was skipped under a load-time error path (e.g., "key not loaded yet, defer to background") gets executed before verification completes. | Critical | 70 | `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)` (verification is synchronous on plugin load; load fails closed; no "verify-async" path) |
| `P-plugin-runtime` | D | A plugin infinite-loops or allocates without bound; absent kill-and-replace from the pool, the host hangs the verb. | Medium | 70 | `[P-0007-plugin-resource-limits](../adrs/P-0007-plugin-resource-limits.md)` (per-instance fuel/memory budget; pool kill-and-replace; per-verb timeout) |
| `P-plugin-runtime` | R | A plugin failure (panic, sandbox abort) is not durably attributed to a specific plugin+version+sig — operator cannot tell which artifact misbehaved across deployments. | Medium | 60 | `[P-0007-plugin-resource-limits](../adrs/P-0007-plugin-resource-limits.md)` + [observability baseline](#observability) (per-plugin-instance event stream *emitted* with signed-artifact identity — generation-side; durability is the operator-chosen sink's) |
| `P-plugin-instance` | E | A plugin requests a host-fn (`DF-host-fn-call`) outside its manifest's declared surface; if the host's permission check is at call-time and the plugin's manifest was loaded laxly, the plugin gains a capability it did not declare. | Critical | 80 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` (manifest-declared host-fn surface compiled into the per-instance allowlist; calls outside the allowlist fail at the WIT boundary, not at the host-fn body) |
| `P-plugin-instance` | T | A plugin returns crafted bytes intended to defeat host-side parsing of plugin output (e.g., NUL bytes that desynchronize a downstream JSON serializer, oversized fields that DoS a projection rebuilder). | High | 65 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` (host validates plugin output against the WIT-declared schema; size caps per field; parser is fail-shut on schema mismatch) |
| `P-plugin-instance` | I | A plugin uses `DF-sampling-up` (MCP sampling: plugin → host → MCP client's LLM) to exfiltrate content from one workspace into a prompt the connected agent's LLM provider sees, even though the plugin had no outbound network. | High | 75 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` (manifest declares `sampling: allowed/denied`; if allowed, content fields the plugin can put in prompts are typed-restricted); at V0.1+ with third-party plugins this becomes Critical |
| `P-builtin-auth` | S | The auth builtin accepts a token whose signature is valid against a previously-trusted key whose rotation was incomplete; old-key tokens are still accepted past the rotation. | High | 70 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (per-deployment OIDC AS via RFC 9728; key rotation discipline with explicit cutover; old keys removed, not coexistent) |
| `P-builtin-auth` | E | A bug in workspace-claim extraction defaults to `default` workspace when the claim is absent, granting access to the dogfood workspace from any authenticated token. | Critical | 80 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (workspace claim is mandatory; absence is a hard auth failure, not a default; structural invariant) |
| `P-builtin-workspaces` | E | A workspace-create / workspace-delete verb is exposed to non-admin tokens because the builtin authorizes on workspace-claim presence rather than on a distinct admin scope. | Critical | 75 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (workspace lifecycle ops require admin scope; admin-scope tokens are a strict superset, distinguished by claim) |
| `P-builtin-projects` | I | Project metadata (project names, repo paths, dispatch-time references) is queryable cross-tenant before RLS policy enforcement; the structural `workspace_id` column is present but reads are not policy-filtered. | High | 75 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (RLS policy enforcement is V0.1+; pre-mitigation at V0 = manual host-side scope check on every read path, lint coverage on absence) |
| `P-builtin-agents` | R | An agent registration recorded the wrong principal (mis-mapped from token) and later actions are attributed to the wrong agent identity; the audit log records the action but not the discrepancy. | Medium | 60 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (agent identity derivation is canonical at registration; mismatch surfaces a structured error rather than silent registration) |
| `DS-pg-content` | T | A SQL-injection-style write through a host-fn that interpolates a content field into a query body — content-typed columns are not the obvious vector; metadata-typed (e.g., tag-name) columns are. | Critical | 70 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` + `[P-0001-storage-layout](../adrs/P-0001-storage-layout.md)` (parameterized queries only; metadata fields normalized + length-capped; type-tightening at host-fn boundary) |
| `DS-pg-content` | I | Direct DB access from a process other than `P-host-fns` (a misconfigured backup tool with read-anywhere role, an operator with full Postgres credentials, a future read-replica without RLS) returns cross-workspace rows. | High | 75 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (RLS column-shape ships V0; policy enforcement at V0.1+); deployment-side: separate DB role per host process, least-privilege |
| `DS-pg-content` | R | Content deletions or modifications are not recorded with prior-value snapshots; an operator-issued destructive op is repudiable as "the agent did it." | Medium | 65 | `[P-0001-storage-layout](../adrs/P-0001-storage-layout.md)` + `{{P-FKPreservation}}` (immutable activity log per write; prior-value capture for destructive ops; ties to `0.5.0` activity log) |
| `DS-pg-content` | D | A pathological projection rebuild triggered by a content write storms the substrate, blocking reads on `DS-pg-projections` (and indirectly `DS-pg-content` via FK consistency checks). | Medium | 65 | `{{P-ProjectionRebuild}}` (rebuild queueing; backpressure; out-of-band rebuild path) |
| `DS-pg-state` | T | State writes from a builtin (e.g., agent-session state) interleave with concurrent CLI writes (e.g., admin token rotation), producing torn state that no constraint catches. | Medium | 65 | `{{P-MCPWriteSemantics}}` (OCC or transactional discipline on state shape; constraint-checked writes only) |
| `DS-pg-projections` | I | A projection contains denormalized content from cross-tenant sources (a vector index over content not partitioned by workspace) — a query inadvertently reaches it. | High | 70 | `{{P-ProjectionRebuild}}` + `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (projections partitioned by workspace; vector indexes keyed on `(workspace_id, vector)`) |
| `DS-ts-metrics` (emitted metric records) | I | Emitted metric records include verb names, argument shapes (low-card sketches), and per-tenant counts; a sink that mixes tenants leaks usage patterns cross-tenant. | Medium | 65 | [observability baseline](#observability) (`workspace_id` on every emitted metric record — the tenant dimension is emission-side, so the operator's sink can partition; no in-app hypertable to cross-query); `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (admin-tier visibility distinct) |
| `DS-ts-events` (emitted event records) | T | Event-shape evolution is uncontrolled (new event-types from new capability families) and downstream consumers misread old events when types are reused. | Medium | 60 | [observability baseline](#observability) (versioned event schema; `event_version` field — an emission-side invariant; backward-compatible additions only without explicit migration) |
| `DS-ts-events` (emitted event records) | I | Dispatch events capture content fragments in their description / payload (the dogfood telemetry-no-leak AC is a check, not a structural barrier). | Medium | 70 | Accepted-risk R-0005 at V0 (dogfood AC); [observability baseline](#observability) (typed event payload, content-IDs only, never bodies — emission-side, holds whatever the sink) |
| `DS-pg-logs` (emitted stdout logs) | I | Emitted logs include token fragments, query strings with PII content, or stack traces with workspace identifiers; a log-tail-as-debugging affordance leaks across tenants if logs are not workspace-scoped. | Medium | 70 | [observability baseline](#observability) (`workspace_id` on every tenant-scoped emitted log record; redaction at the emission/log-write boundary for high-entropy strings — emission-side, regardless of sink) |
| `DS-admin-token` | I | The admin token file is read by a backup process whose role has read-everywhere and writes the token into the backup stream in clear; the backup file then carries the admin secret. | Critical | 80 | `{{P-BackupRestore}}` (secrets excluded from backups by path policy; or backups encrypted with a separate key, not the admin token) |
| `DS-admin-token` | T | A second process on the host overwrites the admin token file (e.g., a misbehaving config tool); the host accepts the new token because it is read on-demand. | High | 70 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (token-file inode pinning at startup; modification fail-shut, with explicit rotation operation) |
| `DS-admin-token` | R | Operator-side rotation of the admin token is not logged structurally; rotation events cannot be audited after the fact. | Low | 60 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (rotation is a CLI verb that emits a structured rotation event — destination per the [observability baseline](#observability) — and writes the activity log) |
| `DS-mnemra-root-key` | I | The signing key material lives on the deployment node (the V0 dogfood position); a host-read primitive recovers the key, and an attacker thereafter signs forged `core: true` plugins. | Critical | 85 | `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)` (V0: build-host-on-disk for dogfood only; multi-deployment trip-wire fires `{{P-SigningKeyCustodyHardening}}`); `{{P-SigningKeyCustodyHardening}}` (key NOT on deployment node — offline-root + per-release certificate; or HSM-backed) |
| `DS-mnemra-root-key` | T | Even if custody is offline, a transitively-trusted intermediate signing key (sub-CA pattern) on the build pipeline could be tampered if the pipeline is compromised. | High | 60 | `{{P-SigningKeyCustodyHardening}}` (build-pipeline integrity invariants; signed pipeline configuration; signing-environment attestation) |
| `DS-source-taskdb` | T | The migration source is read-only by intent; if migration writes-back (e.g., to mark records "migrated"), an attacker who can modify the source taskdb during migration can replay or rewrite migrated records. | High | 70 | `{{P-MigrationID}}` (source is strictly read-only; migration manifest is mnemra-side; source-side "migrated" markers are out-of-band, not source-side state) |
| `DS-source-corpus` | I | Source markdown frontmatter contains arbitrary content; if migration logs verbatim source paths and titles, log readers may see internal-project codenames or sensitive identifiers that were never meant to surface in operational telemetry. | Medium | 65 | `{{P-MigrationID}}` + [observability baseline](#observability) (migration log emits derived IDs only; source paths hashed in emitted log lines if needed for resume) |
| `DS-fs-backup` | I | Backup contents include the full substrate (tokens, secrets, content); same compromise surface as the live DB plus offline-accessibility. | Critical | 85 | `{{P-BackupRestore}}` (encryption-at-rest mandatory; key custody separate from substrate; backup file ACLs explicit) |
| `DS-fs-backup` | T | An adversary swaps in a poisoned backup; restore consumes it without integrity verification. | Critical | 80 | `{{P-BackupRestore}}` (backup-manifest hash verified before restore; signed manifest if backup-key custody allows) |
| `DF-mcp-stdio` | T | Stdio framing is non-self-describing; a man-in-the-stream (e.g., a wrapping process the agent invoked) modifies request/response bytes after the handshake. | Medium | 60 | `{{P-MCPWriteSemantics}}` (transport hardening — stdio at V0 trusts the spawning process; streamable-HTTP V0.1+ ships TLS) |
| `DF-mcp-stdio` | I | The same wrapper observes content responses; at V0 stdio there is no transport-level confidentiality. | Medium | 60 | Accepted-risk R-0003 (stdio-only at V0; transport confidentiality lands with `{{P-MCPWriteSemantics}}` and V0.1+ streamable-HTTP) |
| `DF-host-fn-call` | T | Host-fn arguments are passed as raw bytes the host trusts to be well-typed; a plugin crafts a struct-shape that exploits a host-side deserializer (e.g., an enum with an unexpected discriminant). | High | 65 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` (WIT-defined types only; codegen-generated bindings on both sides; size-bounded fields) |
| `DF-host-fn-call` | I | A host-fn returns content (e.g., search results) whose row set was filtered by `workspace_id` after database read rather than as a WHERE-clause condition — a query plan change or bug surfaces foreign rows the filter then drops; an interposed metric on row-count leaks cross-workspace counts. | High | 65 | `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)` (workspace filter is WHERE-clause-mandatory; lint enforces; metric exports redact row-counts to per-workspace buckets) |
| `DF-sampling-up` | T | A plugin shapes prompt content that, when the connected agent's MCP client runs the LLM completion, induces the LLM to emit a destructive verb the orchestrator then executes. (Prompt-injection through the plugin surface.) | High | 70 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` (sampling-allowed plugins are an explicit manifest capability; at V0 all plugins are `core: true` so the surface is contained; V0.1+ third-party plugins escalate this to Critical) |
| `DF-sampling-up` | I | The plugin can include arbitrary content in the sampling prompt, including content from other workspace artifacts the plugin is read-authorized for, which then traverses the agent's MCP client and reaches the agent's LLM provider — content leaves the deployment trust boundary by design. | High | 75 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` (sampling-prompt fields typed; content-IDs only, not bodies, for cross-artifact references; LLM-provider hostname allowlist) |
| `DF-embed-call` | I | Artifact bodies are sent to an external LLM provider for embedding; the provider has full content access by design. | High | 90 | Accepted-risk R-0005 (external-LLM embed-call is the brief's explicit non-goal carve-out; LLM-API-key surface is in `0.1.0` and a hostname allowlist applies); compensating: BYO-provider deployment posture |
| `DF-embed-call` | T | A man-in-the-middle on the embed path could rewrite embedding vectors, poisoning the vector index. | Medium | 55 | `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` (cert pinning on the embed provider; TLS verification mandatory; out-of-band vector-index integrity verification on rebuild) |
| `DF-signing-attest` | T | The signing-attestation flow trusts the build pipeline's identity assertion; a compromised pipeline forges an attestation. | High | 60 | `{{P-SigningKeyCustodyHardening}}` (attestation is signed by the offline root; pipeline-identity claims are inputs to attestation, not the attestation itself) |
| `DF-migration-write` | T | Migration writes interleave with concurrent CLI / MCP writes during cutover — the brief's cutover dual-write window — and content diverges between source and substrate. | Critical | 70 | `{{P-CutoverDualWrite}}` (cutover is single-writer; migration window holds an exclusive lock or operates pre-cutover; explicit "no concurrent writes" gate) |
| `DF-key-custody` | I | If key material flows from the build pipeline to the deployment node at any point, the network transit and the storage at rest are both leak surfaces. | High | 70 | `{{P-SigningKeyCustodyHardening}}` (key NEVER flows to deployment; runtime sees signature only; or HSM-backed where applicable) |

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
core nine-row set. **A tenth row, `TB-obs-sink`, is a *deferred, egress-only* boundary
added 2026-06-09 (the [observability baseline](#observability)):** at V0 the
host EMITS telemetry to stdout + OTel and there is no in-app observability store; the
external sink is operator-chosen and *deferred* behind the generation⊥storage separation.
It is modeled as the telemetry-egress surface (where emission leaves the host), not as a
present V0 storage dependency — the binary does not require a sink to run. The V0 storage
dependency set stays at the nine core boundaries; `TB-obs-sink` materializes as a present
dependency only when the operator wires a sink (the L4 tripwire). **An eleventh row,
`TB-plugin-store`, is added 2026-07-07 by the plugin-distribution extension
([P-0023](../adrs/P-0023-plugin-distribution.md); see the dated subsection after the
boundary notes):** the bundle store zone between `TB-build-pipeline` and
`TB-mnemra-host` — untrusted by design; both crossings are verification surfaces. When
the Frame's TB table and this one disagree, this one wins.

| Trust boundary | Crosses | Direction | Authentication | Authorization | Threats at crossing |
|---|---|---|---|---|---|
| `TB-agent-runtime` ↔ `TB-mnemra-host` | `DF-mcp-stdio` (MCP stdio transport) | bidirectional (request/response) | Bearer-token presented per MCP session; `P-builtin-auth` verifies against OIDC AS or static admin token. **V0:** static admin token suffices; OIDC verification is V0.1+. | Workspace claim in token scopes every operation; per-verb capability check at `P-mcp-handler` against the plugin manifest. Admin scope distinct from user scope. | `EE-orchestrator-agent`/S,R; `EE-specialist-agent`/S,R; `P-mcp-handler`/S,T,I,D,E; `DF-mcp-stdio`/T,I |
| `TB-human` ↔ `TB-mnemra-host` | `DF-cli-invoke` (admin CLI local IPC); `DF-token-read` (token-file read by CLI handler) | bidirectional (CLI invocation; structured responses) | UNIX UID match for CLI invocation; admin token mandatory for destructive operations. | Admin scope only — agent-facing CRUD does not route through the CLI. Schema-driven dynamic subcommands generated from plugin manifests; `core: true` plugins shipped at V0. | `EE-operator`/S,R; `P-cli-handler`/S,T,E; `DS-admin-token`/I,T,R |
| `TB-mnemra-host` ↔ `TB-plugin-sandbox` | `DF-plugin-invoke` (host → plugin); `DF-host-fn-call` (plugin → host) | host-mediated; cross-plugin calls always traverse the host | Plugin identity is the signed-manifest identity (signature verified at load); host derives session context at invocation. Plugin core is IO-free; ambient capability is the empty set. | Per-plugin host-fn allowlist compiled into the per-instance binding from the signed manifest; `workspace_id` is host-derived from the calling session, NEVER a plugin parameter on write paths; sampling, network, and filesystem are manifest-declared. | `P-plugin-instance`/E,T,I; `DF-host-fn-call`/T,I; `DF-sampling-up`/T,I; `P-plugin-runtime`/T,E,D,R |
| `TB-mnemra-host` ↔ `TB-postgres` | `DF-substrate-rw`, `DF-projection-rebuild`, `DF-migration-write`, `DF-backup-read`, `DF-health-probe` | bidirectional (host issues queries; receives result sets) | Host-process DB user; ideally **role-separated per host process** (host-fns role, migration role, backup role, health-probe role) with least-privilege grants. | RLS column-shape ships V0 (`workspace_id` NOT NULL, indexed); RLS **policy enforcement** is V0.1+ — at V0 the host-side WHERE-clause discipline is the structural barrier (lint-enforced). | `P-host-fns`/T,I,E; `DS-pg-content`/T,I,R,D; `DS-pg-state`/T; `DS-pg-projections`/I; `DF-host-fn-call`/I |
| **(Amended 2026-10-19 r2, #4018 — retypes the row above in place; `TB-postgres` is now the operator's zone.)** `TB-mnemra-host` ↔ `TB-postgres` | the same five flows, now issued by two credentials: `DF-substrate-rw`, `DF-projection-rebuild`, `DF-health-probe` (serving role: `P-host-fns`, `P-instance-check`, `P-cli-check-db`); `DF-migration-write` (owner role: `mnemra init` only); `DF-backup-read` (no object — backup is the operator's, spec R-0117-j item 2) | bidirectional; plus `DF-setup-sql` inbound from `EE-operator` (operator-privileged DDL) and `DF-sighup` from `EE-operator` to `P-instance-check` | **Serving role:** peer authentication over a unix socket after a peer-uid and socket-directory check, or SCRAM over `verify-full` TLS (loopback included) with the password from `DS-db-password-serving`; **owner role:** the same two shapes with `DS-db-password-owner`, held only by `init`. Options built from a closed allowlist; `PG*` and `~/.pgpass` inert; cleartext/MD5 auth requests refused (implementation-time mechanism, named). Spec R-0117-f, R-0119. | **Serving role:** DML only, owns nothing, member of nothing, none of `SUPERUSER`/`BYPASSRLS`/`CREATEROLE`/`CREATEDB`/`REPLICATION` — verified on every check, transitively (R-0117-c); **owner role** owns the schema and runs migrations under an advisory lock (R-0117-h). RLS is **in V0** (#3591 ruling 7262; policy template per 7289) with `FORCE ROW LEVEL SECURITY` verified against the manifest and the catalog (R-0117-e). `EE-other-principals` are bounded by the instance's own grants, not by the host (accepted risk `AR-0013`). | `P-host-fns`/T,I,E; `DS-pg-content`/T,I,R,D; `DS-pg-state`/T; `DS-pg-projections`/I; `DF-host-fn-call`/I; **`DF-health-probe`/S (local transports), `DS-db-password-serving`/I, `DS-db-password-owner`/I, `P-instance-check`/D, `DF-setup-sql`/T, `EE-other-principals`/I,E — see the r2 typed-elements subsection** |
| `TB-mnemra-host` → `TB-obs-sink` | `DF-telemetry-emit` (stdout structured logs + OTel metrics/events egress) | outbound from host (emission only; no read-back) | None at the egress itself; the external sink is operator-chosen and operator-secured. The host's emission boundary enforces redaction + no-content. | Emission carries `workspace_id` on every tenant-scoped record and never artifact bodies; the operator's sink partitions/retains. *(Re-derived 2026-06-09 — generation⊥storage, per the [observability baseline](#observability): telemetry is emitted, not stored in-app; the former `DS-ts-*`/`DS-pg-logs` in-app storage threats are now emission-surface threats — `DS-ts-metrics`/I, `DS-ts-events`/T,I, `DS-pg-logs`/I — carried in the threats-by-element table.)* | `P-host-fns`/I (no-leak at emission); `DF-telemetry-emit`/I |
| `TB-mnemra-host` ↔ `TB-fs-secrets` | `DF-token-read` (CLI handler reads admin token); `DF-signature-verify` (plugin-runtime reads signing key/cert) | inbound to host (filesystem reads) | Filesystem ACLs (mode 600) + OS-uid match. **No second factor at V0.** | The host is the sole reader. Secrets are read lazily on-demand; file-mode invariant check at startup; modification fail-shut. | `DS-admin-token`/I,T,R; `DS-mnemra-root-key`/I,T; `P-plugin-runtime`/E (verify-async path) |
| `TB-mnemra-host` ↔ `TB-fs-source` | `DF-migration-read` (one-shot read of prior tooling state) | inbound to host (read-only by intent) | OS-side: read-as-host-process-uid; logical: migration handler is invoked from CLI under admin token. | Migration handler is the sole reader; source is **never** written-back; migration manifest is mnemra-side. | `DS-source-taskdb`/T; `DS-source-corpus`/I; `P-migration-handler`/T,I,D |
| `TB-mnemra-host` ↔ `TB-fs-backup` | `DF-backup-write` (host → backup); restore consumes inbound | outbound from host (write); inbound at restore | OS-side: write-as-backup-process-uid; logical: backup handler is invoked from CLI under admin token. | Backup handler is the sole writer; restore is admin-gated. **Backup contents are higher-value than the live DB by virtue of offline accessibility.** | `P-backup-handler`/I,T,D; `DS-fs-backup`/I,T |
| `TB-mnemra-host` ↔ `TB-external-llm` | `DF-embed-call` (HTTPS to embedding provider) | outbound from host | TLS server identity check; **hostname allowlist** at config; provider API key as a separate secret. | Per-deployment LLM-API-key configuration ships in `0.1.0`; no in-host LLM hosting (Hard constraint). Hostnames pinned via config; cert validation mandatory. | `EE-llm-provider`/S,R; `DF-embed-call`/I,T |
| `TB-build-pipeline` ↔ `TB-mnemra-host` | `DF-signing-attest` (build-time signing); `DF-signature-verify` (runtime verification reads only the signature) | inbound to host (signature only at runtime) | Runtime: signed-artifact signature verified against the mnemra root cert/key (V0 custody per `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)`; production-grade hardening per `{{P-SigningKeyCustodyHardening}}`); build-pipeline identity is a pre-runtime concern. | Only `core: true` plugins at V0 — runtime accepts artifacts whose signature chains to the mnemra root; non-`core` plugin install is V0.1+ scope and requires a separate trust decision. | `EE-mnemra-root`/S,R; `DF-signing-attest`/T; `DF-key-custody`/I; `P-plugin-runtime`/E |

**Notes on the boundary set.** Two boundaries call out particular fragility worth surfacing
inline:

- **`TB-mnemra-host` ↔ `TB-postgres`** is the structural multi-tenancy fence. RLS
  column-shape ships V0 but **policy enforcement is V0.1+** — at V0 the WHERE-clause
  discipline at the host-fn layer is the only enforcement mechanism. A lint or test that
  asserts every read path carries a workspace filter is a Spec-stage mitigation that
  belongs in `[P-0009-rls-admin-token](../adrs/P-0009-rls-admin-token.md)`. *(Amended
  2026-10-19 r2, #4018: "policy enforcement is V0.1+" is superseded — RLS is **in V0** by
  #3591 ruling 7262 (2026-10-16), and the fence is now the two-role model plus forced RLS,
  both **verified on every instance check** (spec R-0117-c/-e) rather than assumed. The
  #3591 batch rewrites this note in full; this parenthetical only stops it reading as
  current.)*
- **`TB-build-pipeline` ↔ `TB-mnemra-host`** is conceptual — the build pipeline is not a
  runtime adjacency — but the key custody choice (`[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)` for V0 dogfood;
  `{{P-SigningKeyCustodyHardening}}` for production-grade posture) determines whether the
  boundary's actual asymmetry favors the attacker or the defender. The V0 decision is
  now locked at Tier A (`[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)`); the production hardening is Tier C
  (`{{P-SigningKeyCustodyHardening}}`), activated by the multi-deployment trip-wire.

### Distribution extension (2026-07-07 — plugin distribution, [P-0023](../adrs/P-0023-plugin-distribution.md))

The plugin-distribution design ([P-0023](../adrs/P-0023-plugin-distribution.md); spec
[`2026-07-07-plugin-distribution.md`](../../specs/2026-07-07-plugin-distribution.md))
adds a store zone and its crossings. The `TB-plugin-store` boundary row:

| Trust boundary | Crosses | Direction | Authentication | Authorization | Threats at crossing |
|---|---|---|---|---|---|
| `TB-build-pipeline` / `TB-mnemra-host` ↔ `TB-plugin-store` | `DF-publish` (builder → store); `DF-fetch` + `DF-referrers` (store → host fetch-verify pipeline) | outbound at publish; inbound at fetch | **None at the store — the store is untrusted by design.** Trust derives entirely from the package signature (P-0005 root, signer-key-pinned, domain-separated) and content-address digests recomputed over received bytes; store-supplied metadata (headers, declared sizes, tags) is never trusted. | Fetch-side: the bounds-first pipeline (`fetch-within-bounds → verify-package-signature → verify-blob-digests → unpack-within-bounds`, fail-closed — [P-0023](../adrs/P-0023-plugin-distribution.md) D4) is the sole path to load-eligibility; digest-pinned resolution only. Publish-side: signing occurs on the build host only (key custody unchanged, [P-0005](../adrs/P-0005-v0-signing-chain.md)). | New typed elements below; STRIDE-per-element rows land at this cluster's pre-implementation security review (the P-0014 typed-DFD-extension precedent) |

Typed elements added by the extension (consumed by [P-0023](../adrs/P-0023-plugin-distribution.md)'s
Threat-references; the drawn DFD and per-element threat rows re-draw against these at the
cluster's pre-implementation security review — same follow-up shape as the retrieval
cluster's P-0014 extension):

| ID | Type | Element |
|---|---|---|
| `EE-plugin-store` | external entity / zone | the store (self-hosted registry or removable-media `oci-layout`) — untrusted by design; trust derives from signatures + digests, never from the store |
| `P-bundle-builder` | process | build-host assemble/sign/publish tool (spec R-0090); inside `TB-build-pipeline` |
| `P-fetch-verify` | process | the bounds-first fetch-verify pipeline behind the `PackageVerifier` seam (spec R-0083); inside `TB-mnemra-host` |
| `DS-oci-store` | data store | bundle content (outer manifest, blobs, signature referrer) at rest in the store zone |
| `DS-bundle-cache` | data store | the local fetched-layout cache the load path reads (load-path invariant #5's re-validation surface) |
| `DF-publish` | data flow | builder → store (crosses `TB-build-pipeline` → `TB-plugin-store`) |
| `DF-fetch` | data flow | store → host pipeline (crosses `TB-plugin-store` → `TB-mnemra-host`; the bounds-first surface) |
| `DF-referrers` | data flow | signature-referrer enumeration (count- and size-bounded, spec R-0084) |

### Operator-instance retype (Amended 2026-10-19 r2, #4018, approved 2026-10-19 (#4018 activity 7800) — [P-0010](../adrs/P-0010-storage-substrate-engine.md) §Amendment 2026-10-19)

**`TB-postgres` is retyped in place** (Warden B1): it was "the single-process cluster the host owns, reached over a host-owned socket by peer authentication" and is now **the operator-run PostgreSQL instance — the operator's zone**, which the host reaches as a client with two credentials. No second boundary is added; the canonical TB set stays at its count, with the `TB-postgres` row above carrying the r2 retype. The five typed flows into the zone are kept (`DF-substrate-rw`, `DF-projection-rebuild`, `DF-migration-write`, `DF-backup-read`, `DF-health-probe`); `DF-health-probe` is now issued by `P-instance-check` and `P-cli-check-db`, never by the liveness handler. The Frame's steady-state TB table row is updated to match. New typed elements, in the style of the distribution extension above:

| ID | Type | Element |
|---|---|---|
| `TB-postgres` (retyped) | trust boundary / zone | the operator-run instance; the operator holds superuser authority inside it; the host holds two non-superuser roles |
| `EE-other-principals` | external entity (inside `TB-postgres`) | other roles, applications or tenants on a shared or managed instance; bounded by the instance's grants, not by the host (accepted risk `AR-0013`) |
| `P-instance-check` | process (inside `TB-mnemra-host`) | the facts probe plus decision (spec R-0118-a): serving role, pool-identical options, 10 s deadline, supervised task, coalesced `SIGHUP`; the **only** health-related process that touches the database |
| `P-liveness-handler` | process | `GET /health` on a non-runtime thread with a 1 s in-process ping; never touches the database or the check results (R-0118-e) |
| `P-readiness-handler` | process | `GET /ready`; reads the last **completed** check result and its age; `503` on any unmet hard clause, staleness (> 70 s), or drain (R-0118-f) |
| `P-cli-check-db` | process (a separate `mnemra` invocation) | the operator's pre-flight: a second credential-holding client running the same check out-of-process; never a probe body (R-0118-d/-g) |
| `DS-db-password-serving` | data store (inside `TB-fs-secrets`) | `MNEMRA_DATABASE_PASSWORD_FILE`: `O_NOFOLLOW` + `fstat` on every read; host-owned `0600` or root-owned group-readable Secret mount; failure is not-ready, never an exit (R-0119-b, ruling 7762) |
| `DS-db-password-owner` | data store (inside `TB-fs-secrets`) | `MNEMRA_DATABASE_OWNER_PASSWORD_FILE`: read by `mnemra init` only; the serving host never reads it (R-0119-g) |
| `DF-sighup` | data flow | `EE-operator` → `P-instance-check`: forced retry, password-file reload, pool rebuild; OS-signal authority only (R-0118-d) |
| `DF-setup-sql` | data flow | `EE-operator` → `TB-postgres`: the one-time, operator-privileged DDL printed by `check-db --setup-sql` (two roles, grants, `CREATE EXTENSION`); no password literal (R-0118-d) |
| `DF-db-password-read` | data flow | host processes → the credential files; descriptor-checked reads |
| `DF-ready-read` | data flow | `P-readiness-handler` → `P-instance-check`: in-process read of the last completed result; no database |

**Threats at the retyped crossing** (STRIDE-per-element; design-time, P-SecurityLayered; a Warden `threat-modeling` pass over this retype is required before the gate and was not run in the authoring session):

| Element | STRIDE | Threat | Severity | Mitigation |
|---|---|---|---|---|
| `DF-health-probe`, `DF-substrate-rw` over a **local** transport | S | Neither loopback TCP nor a unix socket authenticates the server; a same-host process that binds the port or pre-creates the socket receives the host's password or traffic; the client library answers cleartext/MD5 password requests and uses no SCRAM channel binding (Warden, MEASURED sqlx-postgres 0.9.0 `sasl.rs:14`, `establish.rs:74-98`) | High | **Mitigated, not accepted** (ruling 7771): a password only ever travels over `verify-full` TLS, loopback included; a socket is used only with peer authentication after a peer-uid check (uid = socket-directory owner; no group or other write bit on the directory, no sticky exception) — the same-connection-before-first-byte form and the cleartext/MD5 refusal are both implementation-time mechanisms (sqlx 0.9 writes the startup packet with no hook), each pinned as a named expected failure visible in the gate (spec R-0120-h) — spec R-0117-f, R-0119-c/-d |
| the same over **remote** TCP | S, T, I | spoofed or intercepted instance | High | `verify-full` against the operator's CA; `pg_stat_ssl` verified after connect (R-0117-f) |
| `DS-db-password-serving`, `DS-db-password-owner` | I | credential disclosure through a world-readable mount, a symlink swap, the environment, argv, SQL history or logs | High | descriptor-based `fstat` on every read, `O_NOFOLLOW`, no world read; never in the URL or `PG*` (scrubbed), never in `--setup-sql` output (`\password`), never logged; the owner file is read by `init` only (R-0119-a/-b/-g) |
| the serving role (`P-host-fns`, `P-instance-check`) | E | an over-privileged, owning or over-membered role turns a host query bug into a cross-tenant read or a schema change | Critical | attributes, ownership and transitive membership verified on every check against an empty allowlist; forced RLS reconciled against the catalog; the serving role cannot alter tables it does not own (R-0117-c/-e); the first draft's `NO FORCE` residual is dissolved by the two-role model |
| `P-instance-check` | D | a wedged, panicked or silently stopped checker leaves a stale `met` in place, or a slow instance stalls the check | Medium | per-check deadline, supervised task with a synthetic `unverifiable` on restart, staleness withdraws readiness at 70 s, `seconds_since_last_ok` metric (R-0118-a/-f/-j) |
| `TB-postgres` as a dependency | D | instance unavailable at boot or mid-serve | Medium | not ready without a restart loop; bounded backoff; `storage_unavailable` with a 5 s acquire bound; out-of-cycle check on error (R-0118-b/-c/-m) |
| `DF-setup-sql` | T | the operator's DDL grants more than the printed SQL (a `BYPASSRLS` here, a membership there) | High | the host verifies the result rather than trusting the step (R-0117-c); the printed SQL is the documented, CI-exercised path (R-0120-g) |
| `EE-other-principals` | I, E | another principal on a shared or managed instance reads or alters the host's database | High | **out of model** — accepted risk `AR-0013` (ruling 7771); the host's own grants are least-authority (R-0117-c) and the deployment document names the exposure obligations (R-0117-j items 6, 8) |
| `EE-operator` | R | operator actions on the instance | Medium | the operator's own audit (`EE-operator`/R, unchanged) |

Out of model, signed off (ruling 7771): a hostile operator — the operator owns the data by construction (`AR-0013`). The accepted-risk register below gains `AR-0011`–`AR-0013` for this amendment (Warden B2); the register's remaining sweep (R-0003, R-0007, R-0010 triggers) stays inside the #3591 batched amendment (maintainer ruling 2026-10-16, #3591 activity 7321). The operator obligations the host cannot verify are a named delegation (spec R-0117-j), of which items 1–3 are the accepted risk `AR-0011`.

### Test substrate (Amended 2026-10-22, #4119 — threat-modeled test substrate, spec R-0120)

**Why it is here.** The test substrate's typed DFD lives in this file, beside the operator-instance retype, for two reasons. First, `TB-test-postgres` is the same kind of zone as the retyped `TB-postgres`: a server mnemra does not run, here holding a throwaway population. Second, the tier-2 CI Frame (`docs/intent/ci-flake-tier2-shared-engine-frame.md`) carries a banner saying nothing below it is edited, so this file is where later reviews can annotate one diagram. `TB-test-postgres` is a **separate** boundary from `TB-postgres`, so test-only roles never appear inside the production zone.

**Source.** The #4119 threat model (Warden, 2026-10-22, finding F-1; ids per its finding F-3, namespace-disjoint from the `TB-*`/`DF-*` names above). It is drawn here for the design as amended by spec R-0120 (`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`) on 2026-10-22, which changes the threat model's draft in three places:
- the production-store scan cache and its read flow are gone, because the store guard is a name allowlist (R-0120-a(8));
- the superuser location is named (R-0120-a(1));
- the fresh-machine job is a second instance of `TB-ci-job`, with its own container and no test binary (R-0120-g).

```d2
direction: right

TB-github-platform: {
  label: "TB-github-platform (log service, masking, GITHUB_ENV processing)"
  DS-ci-logs: { label: "DS-ci-logs\n(PUBLIC job logs)"; shape: cylinder }
  EE-github-runner: { label: "EE-github-runner\n(hosted VM provider)" }
}

TB-ci-job: {
  label: "TB-ci-job (one runner VM, one uid, docker group: every step is one trust zone, accepted risk AR-0014)"
  P-ci-steps: { label: "P-ci-steps\n(cert gen, container start, password gen,\nadd-mask, setup SQL incl. IS_TEMPLATE,\nsuperuser-location write, GITHUB_ENV export, docker logs)" }
  EE-third-party-actions: { label: "EE-third-party-actions\n(checkout, toolchain, caches, installers;\ntag-pinned, #4153)" }
  DS-github-env: { label: "DS-github-env\n(GITHUB_ENV / GITHUB_OUTPUT files)"; shape: cylinder }
  DS-tls-material: { label: "DS-tls-material\n(CA cert; CA key deleted; server key/cert)"; shape: cylinder }
  DS-su-location: { label: "DS-su-location\n(~/.config/mnemra/test-superuser/, 0700:\nurl + 0600 password)"; shape: cylinder }
  TB-ci-container: {
    label: "TB-ci-container (server uid; reachable by docker exec)"
    EE-container-image: { label: "EE-container-image\n(pgvector/pgvector@sha256)" }
  }
}

TB-contributor-tree: {
  label: "TB-contributor-tree (the just ci process tree, one OS uid; local or in TB-ci-job)"
  P-just-ci: { label: "P-just-ci\n(preflight: harness URL, no superuser or PG* var;\nrun-id export; chain; EXIT trap:\nleftover check, teardown, GATE leaked-objects)" }
  P-cargo-build: { label: "P-cargo-build\n(cargo, rustc, build scripts, proc-macros)" }
  P-teardown-executor: { label: "P-teardown-executor\n(non-default workspace binary, no argument;\nrefuses bad run id, URL or PG* before connecting;\nharness parse + role checks, datdba before drop,\ndir identity before remove; one teardown_report)" }
  P-test-binary: {
    label: "P-test-binary (each Postgres test binary)"
    P-shared-fixture: { label: "P-shared-fixture\n(parse in harness mode, PG* refusal,\nrole-authority allowlist, name-allowlist store guard,\nrun roles + self-grant, pw files, per-binary DB drop)" }
  }
  TB-gated-cell: {
    label: "TB-gated-cell (ENV-INHERITANCE boundary only, not an OS boundary)"
    P-gated-su-recipe: { label: "P-gated-su-recipe\n(reads DS-su-location, builds without,\nexecs with env; never exits 0 on a skip)" }
    P-gated-su-cell: { label: "P-gated-su-cell\n(instance_check_privileged:\naccessor, superuser pool)" }
  }
  TB-host-under-test: {
    label: "TB-host-under-test (env_clear + explicit list)"
    P-spawned-host: { label: "P-spawned-host\n(production binary; serving role,\nor superuser in the R-0117-c case)" }
  }
  DS-procfs: { label: "DS-procfs\n(/proc/<pid>/environ same-uid; cmdline)"; shape: cylinder }
}

TB-shared-tmp: {
  label: "TB-shared-tmp (OS temp dir; on Linux a shared /tmp, INFERRED)"
  DS-run-dir: { label: "DS-run-dir\n(<tmp>/mnemra_test_<run>/, 0700)"; shape: cylinder }
  DS-pw-files: { label: "DS-pw-files\n(run-scoped owner/serving plaintext, 0600)"; shape: cylinder }
}

TB-test-postgres: {
  label: "TB-test-postgres (the contributor's server, or the CI container's)"
  EE-pg-server: { label: "EE-pg-server\n(contributor- or CI-run server, PostgreSQL >= 16)" }
  DS-pg-instance: { label: "DS-pg-instance\n(pg_database, pg_roles, template,\ntest DBs, other runs' objects)"; shape: cylinder }
  DS-pg-server-log: { label: "DS-pg-server-log\n(STATEMENT lines on error;\nSCRAM verifiers, never plaintext)"; shape: cylinder }
  EE-other-lane: { label: "EE-other-lane\n(concurrent run, same server)" }
}

DS-run-dir-record: { label: "DS-run-dir-record\n(location record, the locator's record branch only;\nowner-only, opened without following links;\nwhere it lives is task #4164's)"; shape: cylinder }

EE-contributor: { label: "EE-contributor\n(runs just ci; owns env)" }
EE-other-local-user: { label: "EE-other-local-user\n(multi-user Linux host)" }
EE-pr-author: { label: "EE-pr-author\n(test code under test)" }

EE-contributor -> TB-contributor-tree.P-just-ci: "DF-01 env: harness URL (userinfo pw), MNEMRA_TEST_RUN_ID"
TB-contributor-tree.P-just-ci -> TB-contributor-tree.P-cargo-build: "DF-02 inherited env"
TB-contributor-tree.P-just-ci -> TB-contributor-tree.P-test-binary: "DF-03 inherited env + run id"
TB-contributor-tree.P-cargo-build -> TB-contributor-tree.P-teardown-executor: "DF-23 build, after the preflight and before the trap and the chain"
TB-contributor-tree.P-just-ci -> TB-contributor-tree.P-teardown-executor: "DF-24 EXIT trap spawn: inherited env: harness URL (userinfo pw), run id; no argument"
TB-contributor-tree.P-teardown-executor -> TB-test-postgres.DS-pg-instance: "DF-25 TLS verify-full to postgres db, harness role: role checks, parse + datdba, DROP DATABASE WITH (FORCE), DROP ROLE"
TB-contributor-tree.P-teardown-executor -> TB-shared-tmp.DS-run-dir: "DF-26 locate (the fixture's locator), check identity, remove"
TB-contributor-tree.P-teardown-executor -> TB-github-platform.DS-ci-logs: "DF-27 teardown_report, parsed-field messages"
TB-contributor-tree.P-teardown-executor -> TB-contributor-tree.P-just-ci: "DF-28 stdout and exit status: the teardown_report the trap's closed outcome set decides on"
TB-contributor-tree.P-test-binary.P-shared-fixture -> DS-run-dir-record: "DF-29 run-dir location: written by the first acquirer, read by later acquirers (record branch only)"
DS-run-dir-record -> TB-contributor-tree.P-teardown-executor: "DF-30 run-dir location read (record branch only)"
TB-ci-job.DS-su-location -> TB-contributor-tree.TB-gated-cell.P-gated-su-recipe: "DF-04 su URL + file path (not env)"
TB-contributor-tree.TB-gated-cell.P-gated-su-recipe -> TB-contributor-tree.TB-gated-cell.P-gated-su-cell: "DF-05 exec env: su URL + su pw file path"
TB-contributor-tree.P-test-binary.P-shared-fixture -> TB-shared-tmp.DS-run-dir: "DF-06 mkdir, pw files"
TB-contributor-tree.P-test-binary.P-shared-fixture -> TB-test-postgres.DS-pg-instance: "DF-08 TLS verify-full to postgres db, harness role: name allowlist, sweep, CREATE ROLE ... PASSWORD '<SCRAM verifier>', GRANT ... WITH SET TRUE, CREATE/DROP DATABASE"
TB-contributor-tree.P-test-binary.P-shared-fixture -> TB-contributor-tree.TB-host-under-test.P-spawned-host: "DF-09 env_clear + URL (no pw) + pw file path"
TB-contributor-tree.TB-host-under-test.P-spawned-host -> TB-test-postgres.DS-pg-instance: "DF-10 serving role, verify-full"
TB-contributor-tree.TB-gated-cell.P-gated-su-cell -> TB-test-postgres.DS-pg-instance: "DF-11 superuser, verify-full"
TB-ci-job.P-ci-steps -> TB-ci-job.TB-ci-container: "DF-12 docker run: su password, key mount, -c flags"
TB-ci-job.P-ci-steps -> TB-ci-job.DS-github-env: "DF-13 harness URL + CA path export"
TB-ci-job.DS-github-env -> TB-ci-job.EE-third-party-actions: "DF-14 later steps' env"
TB-test-postgres.DS-pg-server-log -> TB-github-platform.DS-ci-logs: "DF-15 docker logs (on failure)"
TB-contributor-tree.P-test-binary -> TB-github-platform.DS-ci-logs: "DF-16 test stdout/stderr"
TB-ci-job.EE-third-party-actions -> TB-ci-job.TB-ci-container: "DF-17 docker exec (any step)"
EE-other-local-user -> TB-shared-tmp.DS-run-dir: "DF-18 pre-create / plant (Linux)"
EE-other-local-user -> TB-contributor-tree.DS-procfs: "DF-19 cmdline read"
TB-test-postgres.EE-other-lane -> TB-test-postgres.DS-pg-instance: "DF-20 own sweep, age sweep, clone lock"
EE-pr-author -> TB-contributor-tree.P-test-binary: "DF-21 PR code runs as the test"
TB-ci-job.TB-ci-container.EE-container-image -> TB-ci-job.TB-ci-container: "DF-22 image pull by digest"
```

**Elements.** Each element is created by the spec clause named beside it; R-IDs are those of `docs/specs/2026-05-24-mnemra-core-v0-substrate.md` unless a tier-2 spec is named.

| ID | Type | What it is | Source |
|---|---|---|---|
| `P-just-ci` | process | the `ci` recipe: preflight, run-id export, the chain, the `EXIT` trap (leftover check, teardown, `GATE leaked-objects`) | R-0120-a(5), R-0120-c(1) |
| `P-cargo-build` | process | `cargo`, `rustc`, build scripts and proc-macros, inheriting the `just ci` environment | R-0120-a(1) |
| `P-teardown-executor` | process | the teardown executor: built by `P-cargo-build` after the preflight, spawned by `P-just-ci`'s `EXIT` trap, connecting as the harness role (the fixture's equivalent flow is DF-08) | R-0120-a(5) |
| `P-test-binary` / `P-shared-fixture` | process / in-process | each Postgres test binary and its one fixture entry point | tier-2 R-0026; R-0120-a(2)–(9) |
| `P-gated-su-recipe`, `P-gated-su-cell` | process | the `instance_check_privileged` recipe and binary: the accessor and the superuser pool | R-0120-a(1); tier-2 R-0033 (4) |
| `P-spawned-host` | process | the production binary spawned by `smoke_e2e` or by the gated cell | R-0120-e(1), R-0120-a(1) |
| `P-ci-steps` | process | the job steps that start the container and set it up | R-0120-c(1) |
| `DS-su-location` | data store | `~/.config/mnemra/test-superuser/` (`url`, `password`), `0700` | R-0120-a(1), R-0120-c(1), R-0120-d |
| `DS-run-dir`, `DS-pw-files` | data stores | the per-run directory and the run-scoped role password files (no scan cache) | R-0120-a(2), (5), (8) |
| `DS-run-dir-record` | data store | the record the locator reads the per-run directory's location from, present only if task #4164 chooses that branch of R-0120-a(5); owner-only and opened without following a link; drawn outside every boundary because where it lives is task #4164's | R-0120-a(2), (5) |
| `DS-pg-instance`, `DS-pg-server-log` | data stores | the catalog and databases; the server log | R-0120-a; R-0120-a(2) (SCRAM verifiers) |
| `DS-ci-logs`, `DS-github-env`, `DS-tls-material`, `DS-procfs` | data stores | public job logs; `GITHUB_ENV`/`GITHUB_OUTPUT`; the CA, server certificate and key; `/proc/<pid>/environ` and `cmdline` | R-0120-c(1), R-0120-d, R-0120-e(1) |
| `EE-pg-server`, `EE-container-image`, `EE-other-lane` | external entities | the server (PostgreSQL 16 or later), the digest-pinned image, a concurrent run on the same server *(Amended 2026-10-28, #4363, a draft until the maintainer's exit approval of #4363: since ruling #4363 activity 9016 this entity is a concurrent run of the same lane, on that lane's own server. The identifier is kept because the #4119 threat model record cites it six times, and a rename would leave those references naming nothing.)* | R-0120-a(9), R-0120-c(1), R-0120-a |
| `EE-third-party-actions`, `EE-github-runner` | external entities | marketplace actions (tag-pinned, task #4153) and the hosted runner | `.github/workflows/ci.yml` |
| `EE-contributor`, `EE-other-local-user`, `EE-pr-author` | external entities | who runs `just ci`; another uid on a multi-user host; code under test in a pull request | R-0120-d; R-0120-a(2) |

**Trust boundaries.**

| Boundary | Trust direction | Crossing flows | Where the control or the acceptance sits |
|---|---|---|---|
| `TB-github-platform` | the job writes and the world reads; masking is the only filter | DF-15, DF-16, DF-27 | SCRAM verifiers keep plaintext out of DF-15 (R-0120-a(2)); DF-27 carries parsed fields only, never URL text (R-0120-a(2), (5)); residual `AR-0019`; masking controls are task #4163 |
| `TB-ci-job` | every step trusts every other step | DF-12, DF-13, DF-14, DF-17 | accepted risk `AR-0014`; SHA-pinning is task #4153 |
| `TB-ci-container` | trusts any `docker exec`; TCP gated by `pg_hba` only | DF-12, DF-17, DF-22 | digest pin and drift job (R-0120-c); `AR-0014` |
| `TB-contributor-tree` | the tree trusts what the shell exported | DF-01, DF-02, DF-03, DF-19, DF-23, DF-24, DF-28, DF-29, DF-30 | `PG*` refused, harness URL parsed in `harness` mode (R-0120-a(7)), by the executor as by the fixture; the preflight runs before the executor's build (R-0120-a(5)); the trap decides on DF-28 by its closed outcome set (R-0120-a(5)) |
| `TB-gated-cell` | environment-inheritance boundary only | DF-04, DF-05, DF-11 | holder confinement (R-0120-a(1)); same-uid reads accepted, `AR-0015` |
| `TB-host-under-test` | the host trusts only its explicit variable list | DF-09, DF-10 | `env_clear()` (R-0120-a(1)); the spawn-site detector is task #4165 |
| `TB-shared-tmp` | the fixture trusts a directory it created | DF-06, DF-18, DF-26 | no cache under the name allowlist (R-0120-a(8)); the executor finds the directory through the fixture's locator and checks its identity before removing it, and a location record (`DS-run-dir-record`, drawn outside every boundary until task #4164 places it) is owner-only and opened without following a link (R-0120-a(5)); directory ownership, `O_NOFOLLOW` controls, the locator's mechanism and the identity check's mechanism and seeds are task #4164 |
| `TB-test-postgres` | trusted after `verify-full`, the role-authority allowlist and the store guard; between runs and lanes, by parse plus `datdba` under a privilege floor | DF-08, DF-10, DF-11, DF-20, DF-25 | R-0120-a(3), (6), (8), (9), and for DF-25 the same checks run by the executor (R-0120-a(5)); the shared harness role across lanes is accepted, `AR-0016` *(Amended 2026-10-28, #4363, a draft until the maintainer's exit approval of #4363: "between runs and lanes" reads "between runs of one lane", and "the shared harness role across lanes is accepted" reads "the harness role shared by one lane's runs is accepted"; lanes no longer share a server, by the lane claim of spec R-0120-i(a), maintainer ruling #4363 activity 9016.)* |

**Threat rows for the executor flows (INFERRED).** The #4119 threat model (Warden, 2026-10-22) predates the teardown executor and has no rows for DF-23 to DF-30 or `DS-run-dir-record`. Each inherits the rows of the fixture flow or store it duplicates, because R-0120-a(5) has the executor run the fixture's own checks through the fixture's own functions:
- DF-23 inherits DF-02's rows (`T4119-01`).
- DF-24 inherits DF-01's and DF-03's (`T4119-01`, `T4119-16`, `T4119-29`).
- DF-25 inherits DF-08's on the connection and the drops (`T4119-19`, `T4119-21`, `T4119-23`), but not `T4119-41`, the clone lock, which R-0120-a(5) does not give the executor.
- DF-26 inherits `DS-run-dir`'s (`T4119-27`); DF-29, DF-30 and `DS-run-dir-record` inherit `DS-run-dir`'s and `DS-pw-files`' (`T4119-27`, `T4119-28`) until task #4164 places the record.
- DF-27 inherits DF-16's and `DS-ci-logs`' (`T4119-10`).
- DF-28 inherits `P-just-ci`'s (`T4119-38`, `T4119-42`).

No threat-model pass has drawn rows for these flows on their own; the mapping is the design's reading, not a finding.

## Accepted risks

Populated by the Stage 2 terminal security review (2026-05-22). Each entry is a risk
the V0 architecture accepts on the basis of the V0-dogfood-vs-V0.1+-commercial-tier
carve-out the locked product brief authorizes. Entries are numbered `R-NNNN`. Per the
workspace threat-modeling skill convention, every entry carries a **trip-wire** naming
the condition under which the deferred mitigation must be revisited. Owner is the
maintainer at V0; ownership transfers to a named role at commercial-tier transition.
*(Amended 2026-10-19 r3, #4018: the register mixes two prefixes on purpose for now — the
original rows keep `R-0001`–`R-0010`, which collide with the specs' requirement series
(`CLAUDE.md`, #2126), and the entries this amendment adds use the register's own disjoint
prefix `AR-`. Renaming or confirming the old rows is the #3591 batch's register sweep
(ruling 7321), not this amendment's.)*

The placeholder column set is extended with a **Rationale** column to make the
deferral basis legible without external reference. Every entry maps to one or more
threat rows above by Risk ID.

| Risk ID | Description | Owner | Trip-wire | Rationale |
|---|---|---|---|---|
| `R-0001` | RLS policy enforcement deferred to V0.1+; only the column-shape ships at V0. Cross-tenant disclosure on `P-mcp-handler`/I, `P-builtin-projects`/I, `DS-pg-content`/I, `DF-host-fn-call`/I depends on the host-side WHERE-clause discipline rather than database-enforced policy. **(Amended 2026-10-19 r2, #4018 — SUPERSEDED by #3591 ruling 7262, 2026-10-16: RLS is IN V0. Kept as the rejected alternative with the reasons it lost: its trip-wire was human-noticed and never labelled parked; the host-side lint covers a fraction of the SQL files (#3575); retrofitting later means migrations on operator-held data and query paths that fail as silent empty results. The #3591 batch rewrites this row; this note only stops it reading as current.)** | maintainer | First deployment serving more than one workspace (production multi-tenant traffic), OR the column-shape's lint coverage drops below 100% on read paths, OR a third-party plugin is loaded at runtime. | The brief locks tenant scoping key as structural from V0 precisely so policy enforcement can land later without substrate migration. V0 is single-workspace dogfood; the structural barrier is the WHERE-clause discipline, not policy enforcement. Defer-with-trip-wire matches `P-Defer`. |
| `R-0002` | External-authorization-server integration deferred to V0.1+. `P-builtin-auth` at V0 accepts a static admin token; per-deployment OIDC AS via RFC 9728 is in `0.1.0` substrate but federation/SSO is not. Operator-impersonation risk (`EE-operator`/S, `EE-orchestrator-agent`/S) hinges on token file ACLs and rotation discipline. | maintainer | First deployment integrating with an existing identity provider for the operator, OR shared-workstation use of the deployment node, OR external onboarding of agents beyond the maintainer's. | Brief Hard constraints scope mnemra as Resource Server only at V0; AS integration is brief idea-tier (D7). The static admin token bootstrap is the V0 dogfood pattern. |
| `R-0003` | MCP transport at V0 is stdio. Transport confidentiality is process-spawning trust — a wrapping process the agent invokes can observe request/response bytes (`DF-mcp-stdio`/I, T). | maintainer | `{{P-MCPWriteSemantics}}` resolution OR streamable-HTTP transport activates (the microVM-appliance trip-wire from the brief), whichever lands first. | Brief Hard constraints fix stdio at V0; streamable-HTTP is V0.1+ activation gated on the microVM-appliance trip-wire. Stdio confidentiality is "the agent's host process is the trust unit." |
| `R-0004` | Plugin signing-key custody at V0 is dogfood-scoped to the maintainer's single deployment under `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)` (Tier A) — build-host-on-disk, single-instance, maintainer-controlled. `DS-mnemra-root-key`/I and `EE-mnemra-root`/S are Critical-severity threats whose production-grade mitigation depends on `{{P-SigningKeyCustodyHardening}}`. The V0 commitment is made; the hardening is deferred to Tier C. | maintainer | Multi-deployment trip-wire fires — `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)`'s stated condition: the moment mnemra-core is deployed beyond the maintainer's single dogfood instance. `{{P-SigningKeyCustodyHardening}}` (Tier C) must lock before that point. The risk register entry retires when `{{P-SigningKeyCustodyHardening}}` locks. | V0 dogfood scoped to single-operator single-deployment; key-on-build-host is a named and trip-wired V0 decision, not a gap. Production-grade custody (offline-root, HSM, never-on-node) is deferred under `P-Defer` with the multi-deployment trip-wire as the mandatory activation condition. |
| `R-0005` | External-LLM embedding calls (`DF-embed-call`/I) and the brief's "no in-host LLM" Hard constraint together mean **artifact bodies leave the deployment trust boundary** at every embedding call. Telemetry no-leak is a dogfood acceptance criterion (audit script against a known-content corpus), not a structural barrier. | maintainer | First deployment that is not single-operator-only (any deployment with telemetry-sensitive content beyond the maintainer's), OR Bring-Your-Own-Model (BYOM) is not configured and a hosted provider sees production content. | The brief explicitly carves "no in-host LLM" and "calls out to an external model" as Hard constraints. The compensating control is BYOM deployment posture — the LLM-API-key configuration surface is in `0.1.0`. The risk is acknowledged, structurally bounded by deployment posture, not mitigated by code. |
| `R-0006` | Operator-action repudiation at V0 is partially mitigated. CLI destructive ops (`drop-workspace`, force-restore-overwrite) tie to the OS UID; there is no second factor and no admin-action audit log that survives a substrate restore. Repudiation threats on `EE-operator`/R, `DS-admin-token`/R rely on OS-side audit. | maintainer | First deployment with more than one operator on the host, OR the activity-log capability (`0.5.0`) lands and admin actions can be tied to a durable audit event independent of the substrate. | Solo dogfood: the maintainer is sole operator; repudiation is not a meaningful threat in that topology. Activity log lands at `0.5.0`, after the substrate; the dependency is correct, the trip-wire fires on multi-operator topology, not on time. |
| `R-0007` | Plugin sampling (`DF-sampling-up`) at V0 is unrestricted because all plugins at V0 are `core: true` and signed by the mnemra root. The plugin-to-orchestrator prompt-injection surface (`DF-sampling-up`/T, `P-plugin-instance`/I) is High at V0 and **becomes Critical at V0.1+ when third-party plugin install activates** (brief idea-tier D11). | maintainer | Third-party plugin install activates (brief D11), OR `[P-0003-plugin-manifest](../adrs/P-0003-plugin-manifest.md)` locks the sampling capability shape without typed-prompt restrictions. | At V0 the plugin set is trusted by build provenance; signed artifacts are the only execution surface. The mitigation cost is high relative to V0 value; the trip-wire is structural (third-party install), not temporal. |
| `R-0008` | **Rollback/downgrade on the distribution surface** (`EE-plugin-store`, `DF-fetch`): OCI + the keyed package signature cannot prevent the store (or a MITM on the LAN path) from serving an **older, validly-signed** bundle. High severity on the distribution surface; no freshness/monotonicity mechanism exists at this tier — deliberately, per `[P-0023](../adrs/P-0023-plugin-distribution.md)` (a bespoke version-monotonicity check is explicitly not built; TUF is the designed answer). | maintainer | `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)`'s R-0005-e condition fires (deployment beyond the single dogfood instance, or third-party publishers) — the TUF adapter (timestamp/snapshot freshness) lands behind the locked `PackageVerifier` seam, co-requiring the Tier-C offline root for its rotation half. Entry retires when the TUF layer locks. | Single-publisher single-deployment dogfood: the operator controls both store and host, so a stale-serve requires compromising infrastructure the operator owns. The seam is pre-positioned so the mitigation arrives as an adapter, not rework (`P-Defer`, decision content in the Frame's deferral table). |
| `R-0009` | **Single-root compromise-independence** (`DS-mnemra-root-key`/I reach, extended by distribution): the package signature (distribution anchor) and the inner manifest signature (provenance anchor) are both made by the one P-0005 root at this tier (`[P-0023](../adrs/P-0023-plugin-distribution.md)` D6, maintainer-ratified Q5 2026-07-07) — the layers are scope-independent, **not compromise-independent**; one root theft forges both, and detecting a stolen-key forgery needs a transparency-log/TUF witness that is deferred. Critical severity under key theft (as `R-0004`). | maintainer | The same R-0005-e condition — split to a distinct **distribution key** (or TUF delegated roles), restoring compromise-independence; the anchor-independence of the dual load-time checks becomes fully load-bearing at the split. Entry retires when the distribution key splits. | One custody story at single-publisher `core: true` scope (Security ↔ Simplicity, maintainer-resolved); the exposure is recorded in the ADR and here rather than silently accepted. Compensating: the R-0004 custody posture (key never on the deployment node) bounds the theft surface to the build host. |
| `R-0010` | **Build-time dependency confusion** (`P-bundle-builder` reach): `wkg`-pulled WIT/component dependencies are composed into the component **before** signing, so a malicious same-named dependency resolved at build time changes the component bytes and every downstream gate then faithfully binds the poisoned artifact (bytes-run == bytes-signed — integrity, not provenance-of-inputs). | maintainer | Reproducible `wasm32-wasip2` builds land (the reproducible-builds work item, #1942) — SLSA provenance attestations attached as OCI referrers become assertable and are the designed answer. Interim: the lockfile-hash-pin weighing is a named spec-gate item on the distribution spec. Entry retires when provenance attestations land. | The residual is upstream of every signing gate by construction — no load-path mechanism can close it (the gates bind what was built). Named honestly rather than over-claimed; the distribution design neither widens nor narrows it. |
| `AR-0011` *(Amended 2026-10-19 r2, #4018; signed off by the maintainer 2026-10-19, #4018 activity 7771)* | **The operator owns the Postgres and pgvector supply chain and the persistent storage the data lives on.** The host verifies nothing about the binaries it talks to and cannot see the instance's data directory: the embedded design's hash-pinning of the engine binaries (`KNOWN_GOOD_HASHES`) is **lost, not replaced** — a loss, labelled as such (P-0010 §Amendment DE9). Spec R-0117-j items 1–3, emitted by `mnemra check-db --obligations` under "Not verified by mnemra". | maintainer | Retires only if a packaging shape under #4019 lets the host verify what it talks to (a host-verified image digest, for example) — none is designed; otherwise standing. Re-examined at the #4019 gate. | The operator model (ruling 7470) places the instance outside the host's authority by construction; a host that cannot install or run the binaries cannot pin them. Named in the obligations list and the deployment document rather than implied. Rejected: keeping a host-side pin table for an install the host does not perform — it would check a path it never runs. |
| `AR-0012` *(Amended 2026-10-19 r2, #4018; signed off 2026-10-19, activity 7771)* | **Up to one drift interval of writes acknowledged under a weakened durability setting.** An operator- or role-level change to `fsync` / `full_page_writes` / `synchronous_commit` / `data_sync_retry` is observed by the next instance check (30 s cadence plus one check duration, spec R-0118-c); writes acknowledged in that window may be lost on an instance crash. Spec R-0117-d, R-0121-b. | maintainer | **Mechanical trip-wire (r3):** the window only ever opens on a `durability_off` transition, and every such transition is counted (`instance_check_total{clause="R-0117-d",status="unmet"}`, spec R-0118-j) and alerted (`host_ready == 0`); the trip-wire is **the first such transition observed on a production instance**, which re-opens this entry — a lost write itself is not detectable, so the detector is keyed on the event that creates the window, not on the loss. The window shrinks if the drift interval shrinks (AE4, ruled fixed at 30 s on 2026-10-19, activity 7787) and closes only if every writer session re-checked the settings per transaction, a per-call cost this amendment does not take. | The settings are the operator's instance's; the host verifies them on the sessions writers use and rebuilds the pool after a drift, which is the structural mitigation. The residual is the interval, stated as a number rather than hidden. |
| `AR-0013` *(Amended 2026-10-19 r2, #4018; signed off 2026-10-19, activity 7771)* | **A hostile operator, or other principals on a shared or managed instance, are out of model.** The operator holds superuser authority inside `TB-postgres`; `EE-other-principals` are bounded by the instance's grants, not by the host. `EE-other-principals`/I,E. | maintainer | A managed or shared-instance offering (the brief's `idea`-tier managed/appliance entries) — self-announcing through the register: that entry's design is where the model is re-drawn. | The operator owns the data by construction (the brief's self-hostable success criterion); modelling the owner as an adversary would make every control a control against the deployment's own principal. The host's side is least-authority (R-0117-c) and the exposure obligations are named (R-0117-j items 6, 8). **Not** accepted alongside this: local server spoofing, which is mitigated (spec R-0117-f, R-0119-c/-d). |
| `AR-0014` *(Amended 2026-10-22, #4119; accepted by Peter 2026-10-22, #4119 activity 8067; threat model candidate AC4119-1)* | **The CI job is one trust zone.** In the job that runs the Postgres tests (spec R-0120-c(1)), every step can read the superuser location, `docker exec` into the server and hold the harness URL. That includes tag-pinned third-party actions and the pull request's own test code. Threats `T4119-01`, `-02`, `-44`, `-45`, `-47`, `-49` (the #4119 threat model; zone `TB-ci-job` in the test-substrate DFD above). **Cost, as stated in the threat model:** in CI, the superuser "confinement" defends against accidental inheritance only, and a moved action tag can falsify the gate's own verdict. That was already true before #4119; #4119 adds a server and credentials to the zone. | maintainer | **PARKED — human-noticed:** revisited at each change to `.github/workflows/ci.yml` (the review of that change re-reads this row), and at task #4153 (SHA-pin the third-party actions with an update mechanism), whose landing narrows the entry to the pull-request-code half. | The credentials are throwaway: generated per job, for a container that dies with the job, with no repository secret in the job by design and a `pull_request` trigger, so nothing beyond the job is reachable. The alternative is SHA-pinning plus `local all all reject` in the container's `pg_hba` (threat model D4), at a maintenance cost on every action release; it is filed as #4153 rather than taken in #4119. |
| `AR-0015` *(Amended 2026-10-22, #4119; accepted by Peter 2026-10-22, #4119 activity 8067; candidate AC4119-2)* | **Same-uid processes on a development machine can read every test credential**: the gated cell's environment through `/proc/<pid>/environ`, and the `0600` superuser and run-scoped password files. Threat `T4119-07`. **Cost:** any same-uid malware gets superuser on the developer's test server. This matches R-0119-a's own premise for host credentials. The delivered property is "not inherited through any environment" (spec R-0120-a(1); P-0031 §Amendment 2026-10-22), not "held by that process alone". | maintainer | **PARKED — human-noticed:** revisited at each change to the deployment document's local-test section (spec R-0120-d), which is where a supported multi-user development host would first be described. | No cheaper alternative exists: the uid is the boundary the operating system gives a single user's processes. The test server is dedicated to tests (R-0120-a(8)), so the superuser reached is a throwaway one. |
| `AR-0016` *(Amended 2026-10-22, #4119; accepted by Peter 2026-10-22, #4119 activity 8067; candidate AC4119-3, decision D2)* *(Amended 2026-10-28, #4363; maintainer ruling 2026-10-28, #4363 activity 9016; the amendment's text is a draft until the maintainer's exit approval of #4363)* | **Lanes sharing one server and one harness role are mutually trusted.** Between runs and lanes, protection is code (the parse-and-`datdba` rule of spec R-0120-a(3)), not privilege. The harness holds `ADMIN` on every role it created, and `SET` and `INHERIT` on every run role it self-granted (R-0120-a(9)). The 24-hour age sweep assumes synchronised clocks. Threats `T4119-18` (cross-lane part) and `-40`. **Cost:** a buggy, stalled or clock-skewed lane can drop the other lane's live objects, and detection is the other lane's red run. **Amended 2026-10-28 (#4363; maintainer ruling 2026-10-28, #4363 activity 9016, "Separate server per lane"):** the acceptance of **lanes** sharing one server and one harness role is **retired**. Each lane now has its own server and harness role, enforced by the lane claim of spec R-0120-i(a). What stays accepted is the same arrangement for **concurrent runs within one lane**, which share that lane's server and harness role. Between those runs, protection is still code (R-0120-a(3)), not privilege, and the age sweep still assumes synchronised clocks. **Cost, within a lane:** a buggy, stalled or clock-skewed run can drop a concurrent run's live objects in the same lane, and detection is that run's red. The original acceptance above is kept as the record. *Draft until the maintainer's exit approval of #4363.* **The ruling, as recorded** (#4363 activity 9016, Puck's record of his selection; the option wording was Puck's): he selected only "Separate server per lane (Recommended): each lane gets its own Postgres server and port, so lanes cannot interfere". He did not select "one harness role per lane (D2), keep sharing and patch in round 3, cut scope and park". **Why the 2026-10-22 acceptance lost:** the maintainer stated no reason of his own. The record says he was told that sharing had been chosen on 2026-10-22 for a reason Puck did not know, and the option he selected gives its reason as "so lanes cannot interfere". The shared-server class had drawn new findings in two review rounds running (#4363 activity 8988). The lane's identity is a declared lane name (#4363 activity 9037). **Amended again 2026-10-28 (#4363; maintainer ruling 2026-10-28, #4363 activity 9046, O2: "Age sweep skips same-host runs" and "Old run refuses to continue"; Puck's record, and the option wording was Puck's; a draft until the maintainer's exit approval of #4363):** within a lane, the age sweep no longer removes a same-host run, since a same-host name is decided by liveness alone, and a run more than 22 h old refuses to acquire (spec R-0120-i(j)). **The Cost within a lane now reads:** a buggy run, or a run whose liveness is misread as dead, can drop a concurrent run's live objects, and detection is that run's red. A clock-skewed fixture can age out another host's names early, or refuse its own run early. A suspended run is no longer removed by its own lane's age sweep. | maintainer | **PARKED — human-noticed:** revisited on the first red run attributed to a cross-lane drop, and at task #4166 (bounded clone lock and the run-lifecycle hardening), which narrows the window. The fallback is D2: one harness role per lane. **Amended 2026-10-28:** the cadence now reads "revisited on the first red run attributed to a cross-run drop within a lane, and at task #4166". The D2 fallback is no longer the next step, since one server per lane supersedes it. **Amended again 2026-10-28 (ruling 9046; a draft until the maintainer's exit approval of #4363):** the cadence is unchanged. The stalled-run trigger it covered is narrowed by spec R-0120-i(j). | The server holds nothing of value: it is dedicated to tests and its contents are recreated per run. The privilege floor still holds: the harness can drop only databases owned by roles it holds `SET` on, and R-0120-a(6) admits only run-grammar roles, so even a parse bug cannot reach a non-test database. The alternative (D2) costs one more setup step per lane, plus its documentation. |
| `AR-0017` *(Amended 2026-10-22, #4119; accepted by Peter 2026-10-22, #4119 activity 8067; candidate AC4119-4)* | **A local run proves less than CI.** The `/proc/<child>/environ` assertion of spec R-0120-e(1) compiles on Linux only, so a macOS `just ci` does not prove that a spawned host's environment holds no `MNEMRA_TEST_*` variable. Threat `T4119-36`. **Cost:** the required check is the only authority for `env_clear` and for the superuser negative cases. The gated cell itself runs locally (ruling 8067 item 3), so only the environ assertion is CI-only. | maintainer | **PARKED — human-noticed, with a detector for its platform half:** revisited at each change to `.github/workflows/ci.yml` (the review of that change re-reads this row). The platform half is detected: the required "CI gates" job fails unless its output holds the Linux-only marker line of `smoke_e2e` and of the gated binary (spec R-0120-e(4) as amended), so moving the required check off Linux turns it red whatever `CI` is set to. Deleting that step from `ci.yml` is the residual, and the cadence above covers it. The local half stays parked: a macOS `just ci` proves less than CI, and no detector changes that. | A platform limit; no alternative exists short of a Linux-only local workflow. |
| `AR-0018` *(Amended 2026-10-22, #4119; accepted by Peter 2026-10-22, #4119 activity 8067; candidate AC4119-5)* | **Nothing listens on loopback port 1.** The red-phase sentinel pools of spec R-0120-f(ii) target `127.0.0.1:1`; that nothing listens there is INFERRED, and on macOS an unprivileged bind may be possible. Threat `T4119-26`. **Cost:** a local listener makes red-phase tests flake; only dummy credentials are exposed. | maintainer | **Self-announcing:** a listener on port 1 turns the sentinel tests' refusal assertions red (`libs/mnemra-host/tests/coordination_failclosed.rs:292-325` asserts a refused connection), which is a loud red. | The alternative is to measure it (threat model I-17). |
| `AR-0019` *(Amended 2026-10-22, #4119; accepted by Peter 2026-10-22, #4119 activity 8067; candidate AC4119-6)* | **A SCRAM verifier of a 32-byte random secret may appear in a public log on a red run.** Run-scoped passwords are set as client-computed SCRAM-SHA-256 verifiers (spec R-0120-a(2)), so a failing role statement logs the verifier and never the plaintext (MEASURED on PostgreSQL 16.4, #4119 activity 8064, I-18), and `docker logs` publishes the server log on a red CI run. Residual of threat `T4119-03`. **Cost:** offline recovery is infeasible (INFERRED); the role dies with the container. | maintainer | **PARKED — human-noticed:** revisited at each change to `.github/workflows/ci.yml` (the review of that change re-reads this row). The exposure depends on a throwaway CI container, and a change that makes the CI server persistent is a change to R-0120-c(1)'s container steps in that file. | A verifier does not let anyone log in under SCRAM without the plaintext (INFERRED), and brute force against a 256-bit random secret (32 bytes) is infeasible. |

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

- **2026-07-07** — Plugin-distribution extension (rider on the W2-1 Stage-3 docs change;
  [P-0023](../adrs/P-0023-plugin-distribution.md) + spec
  [`2026-07-07-plugin-distribution.md`](../../specs/2026-07-07-plugin-distribution.md)).
  Added the `TB-plugin-store` boundary row + eleventh-row note in the canonical TB
  enumeration, the distribution extension's typed elements (`EE-plugin-store`,
  `P-bundle-builder`, `P-fetch-verify`, `DS-oci-store`, `DS-bundle-cache`, `DF-publish`,
  `DF-fetch`, `DF-referrers`), and accepted risks `R-0008` (rollback/downgrade residual,
  retires at R-0005-e via TUF), `R-0009` (single-root compromise-independence, retires at
  the R-0005-e distribution-key split), `R-0010` (build-time dependency confusion,
  retires at reproducible-builds/SLSA). The drawn DFD and STRIDE-per-element rows
  re-draw against the typed extension at the cluster's pre-implementation security
  review (the P-0014 precedent; named follow-up).
- **2026-07-02** — RC-1 reconciliation (rider on the retrieval-cluster Stage-3 docs
  change; this overview's ELT external-embedding framing was the named lagging copy of
  the brief's model-hosting amendment). Updated the hard-locked constraint-inventory
  model-hosting row and the DFD-notes "External LLM" bullet to the RC-1 posture: MUST NOT
  host a *generative* LLM; local non-generative inference (embedding, reranking) is
  host-side and never egresses; the external provider serves the retrieval cluster's four
  policy-gated, individually-disable-able generative placements; zero-egress is a
  supported V0 configuration. The drawn DFD's `DF-embed-call` flow and `EE-llm-provider`
  "(embeddings endpoint)" label are now pre-RC-1 renderings: the retyped retrieval-cluster
  elements (four `DF-egress-4.x` flows, `EE-model-artifact-source`, new
  processes/stores) are recorded in
  [P-0014's typed-DFD extension](../adrs/P-0014-retrieval-architecture.md); the diagram
  and per-element threat rows re-draw against it at that cluster's pre-implementation
  security review (named follow-up with its firing event; prose reconciled now).
- **2026-06-09** — Observability re-derivation (E1 dispositioned = re-derive now), **re-altituded
  out of the project-ADR layer**. The maintainer dispositioned escalation E1 (D8 vs the accepted
  observability ADR) by separating observability **generation** from **storage**, and ruled that
  observability is a **theory trait + chassis mechanism, not a per-project ADR**. The generation
  decisions land in the [observability baseline](#observability) in this overview (a theory-trait
  baseline); the original observability ADR [P-0004](../adrs/P-0004-observability-shape.md) is
  `deprecated` (its storage core falsified by D8; **no successor ADR**). Re-derived the
  observability surfaces this overview left frozen on 2026-06-08: the "Reframed 2026-06-08"
  subsection's frozen-pending-E1 marker is replaced with the resolved separation; the DFD
  `DS-ts-*` / `DS-pg-logs` in-app storage nodes are replaced with an external `TB-obs-sink`
  telemetry-egress node (host EMITS stdout logs + OTel metrics/events via `DF-telemetry-emit`;
  the in-app `DF-timeseries-write` / `DF-log-write` and migration-to-ts-store edges are removed);
  the QA observability scenarios are generation-side (emitted metrics, p50/p95/p99 derivable at
  the sink; no in-app hypertable); the observability threat rows are re-derived to emission
  surfaces and re-pointed to the observability baseline. The `/health` detail body is gated by
  the loopback-only listener bind (loopback IS the gate at V0; named tripwire if the listener
  ever binds non-loopback). The host capability-manifest is a settled invariant (generated from
  WIT, never hand-maintained; generation mechanism routes to the chassis). The observability
  storage backend is deferred behind the separation (option set + named tripwire); the standalone
  binary survives. No in-app observability store at V0.
- **2026-06-08** — Storage-substrate fold. Reclassified the storage substrate from the
  prior hard-lock framing ("single-process Postgres + `pgvector` + `timescaledb`
  extensions present") to "PostgreSQL ratified on merits, behind an engine-agnostic
  swappable `Storage` trait" per the new
  [P-0010-storage-substrate-engine](../adrs/P-0010-storage-substrate-engine.md) (folding the
  2026-06-07 storage-engine evaluation, ratified after the spec lock). Updated the
  hard-locked constraint-inventory substrate row; added a "Reframed 2026-06-08" subsection.
  TimescaleDB demoted off the V0 stack (D8); the V0 time-series *storage shape* is plain
  timestamped tables. The DFD `DS-ts-*` hypertable nodes, the QA "Per-verb metrics" /
  "Retention discipline" scenarios, and the TimescaleDB threat rows were left frozen pending
  escalation **E1** (D8 vs P-0004) — **since re-derived in the 2026-06-09 entry above** (E1
  dispositioned).
- **2026-05-23** (Stage 3 entry housekeeping) — Slot citation routing pass and R-0004
  rewrite. All 13 stale signing-key-custody citations and 3 stale plugin-pool-memory
  citations in threats-by-element, trust-boundary, accepted-risks tables, DFD notes, and
  TB-notes prose were routed per-row: V0-scoped rows → `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)`; production-
  posture rows → `{{P-SigningKeyCustodyHardening}}`; span-both rows → both with
  cross-reference. All plugin-pool-memory citations → `[P-0007-plugin-resource-limits](../adrs/P-0007-plugin-resource-limits.md)`
  uniformly. R-0004 risk register entry rewritten: shifted from "open ADR slot" framing
  to "V0 commitment + trip-wire to hardening" — description now reflects
  `[P-0005-v0-signing-chain](../adrs/P-0005-v0-signing-chain.md)` (Tier A, single-deployment dogfood decision made) and
  `{{P-SigningKeyCustodyHardening}}` (Tier C, activated by multi-deployment trip-wire).
  The stale "(Tier C in the Frame's Open ADR Slots)" parenthetical was removed as part
  of the rewrite. ELT subsystem description in DFD notes updated to use inline mechanism
  description, consistent with the companion Frame's L1 fix.
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
  Spec-stage ADR slot `[P-0001-storage-layout](../adrs/P-0001-storage-layout.md)`).
