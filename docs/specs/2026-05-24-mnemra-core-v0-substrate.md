# Feature: Mnemra Core V0 Substrate (`0.1.0`)

> **Locked: 2026-05-24** (spec-exit gate confirmed by the maintainer)
> **Spec for:** task #1166 — WS-E-2 Stage 3, mnemra-core V0 substrate increment
> **Date:** 2026-05-24
> **Design ref:** [Frame](../src/intent/mnemra-core-frame.md), [Product Brief](../src/intent/mnemra-core.md), [Architecture Overview](../src/architecture/overview.md), ADRs [P-0001](../src/adrs/P-0001-storage-layout.md) through [P-0009](../src/adrs/P-0009-rls-admin-token.md)
> **Amended 2026-06-29 — artifact-list keyset (cursor) pagination:** in-place amendment adding **R-0020** (pagination behavior) and amending the `artifact.list` (import) / `content.list` (export) signatures via R-0012-a/-f/-e and R-0019-a (R-0017-a triggered as the breaking-ABI instance). Localized upstreams — the closed world this amendment draws from: [paging intake](../intent/artifact-list-paging.md) and [paging Frame](../intent/artifact-list-paging-frame.md) (Frame-exit gate passed 2026-06-29, blob `31515f1`). The product-tier design refs above are unchanged.
> **Stage-3 review fold (2026-06-29):** this amendment incorporates the Stage-3 review conditions (security/correctness, operational, and testability reviews — all *approve with conditions*, no Critical/High) and the maintainer's spec-exit decisions: the **ids-only** page return is **provisionally accepted** with dogfood instrumentation + a revisit tripwire (R-0020-b; Out of Scope); `statement_timeout = 3000 ms` and `limit = 0` → clamp-to-default are **maintainer-ratified** (R-0020-d/-f); the scan-cost backstop gains an explicit-transaction requirement, a recalibrated (two-part) timeout AC, and a distinct `query_scan_timeout` caller error (R-0020-d; MCP error-code table); the ids-only narrowing is re-anchored to **intake Success criterion 1/2** (Frame D4 had understated the import baseline); and the scenario set + Verify Contract enumeration are reconciled (31 scenarios). *(Puck re-locks `[audit_chain.spec].version` and reconciles the BOM sidecar at spec-exit; the BOM is not edited in this amendment.)*

## Purpose

Establish the foundational substrate — the builtin host core, storage schema, plugin runtime, host-fn ABI, observability, auth, and tenant enforcement — that every subsequent capability-family increment (`0.2.0`–`1.0.0`) rides on, without which no plugin can be loaded, no MCP verb dispatched, and no content stored.

## Requirements

RFC-2119 keywords used throughout. `SHALL`/`MUST` = mandatory. `SHALL NOT`/`MUST NOT` = prohibited. `SHOULD` = preferred. `MAY` = optional.

### R-0001 — Storage layout (anchors [P-0001](../src/adrs/P-0001-storage-layout.md))

- R-0001-a: The system SHALL persist each logical artifact as a single content-substrate row using the C1 single-document layout: `id` (ULID), `workspace_id` (NOT NULL), `type`, `frontmatter` (JSONB), `body` (nullable text), `frontmatter_version`, `migrated_from` (system field), `migrated_at` (system field), `created_at`, `updated_at`.
- R-0001-b: The system SHALL NOT store `migrated_from` or `migrated_at` inside the `frontmatter` JSONB column; these are dedicated system columns. `frontmatter_version` SHALL live authoritatively inside the `frontmatter` JSONB column (the interchange format is self-describing); the dedicated `frontmatter_version` column SHALL be a `GENERATED ALWAYS AS ((frontmatter->>'frontmatter_version')::bigint) STORED` no-drift projection of that JSONB key, so the typed column cannot diverge from the authoritative JSONB value.
- R-0001-c: The system SHALL enforce `CHECK (frontmatter ? 'id')` and `CHECK (frontmatter ? 'frontmatter_version')` constraints at the schema level on all artifact tables.
- R-0001-d: The system SHALL create per-artifact-type tables (not a polymorphic single table) for each plugin-owned content family; per-type expression indexes SHALL be declared at schema initialization for hot query fields (`status`, `priority`, `project_id`, `parent_id`).
- R-0001-e: The system SHALL implement a trigger-based shadow table (`<artifact>_history`) for mutation history on each artifact table, populated on UPDATE; the shadow table SHALL preserve the prior row's frontmatter value byte-for-byte. On `artifact.delete`, the host SHALL write a history row with `operation = 'DELETE'`, `old_frontmatter` = the artifact's pre-deletion frontmatter value, and `old_body` = the artifact's pre-deletion body value, before executing the DELETE; this row is retained under the shadow table's standard retention schedule.
- R-0001-f: The system SHALL maintain materialized projection views over the artifact tables, refreshed via a host-owned queue on host-fn write completion; the V0 refresh mechanism SHALL use `REFRESH MATERIALIZED VIEW CONCURRENTLY` triggered by the host-fn write path; a background worker SHALL drain the refresh queue.
- R-0001-g: The system SHALL enable `pgvector` (bundled with the embedded Postgres engine — `postgresql_embedded` + `pgvector_compiled`, per [P-0010](../src/adrs/P-0010-storage-substrate-engine.md)) in the schema initialization; V0.1+ activation of vector columns (`embedding vector(1536)`) and full-text columns (`search_tsv tsvector`) SHALL be non-breaking `ALTER TABLE ADD COLUMN` additions to the existing tables. *(Amended 2026-06-08 per [P-0010](../src/adrs/P-0010-storage-substrate-engine.md): `timescaledb` is removed from V0 extension installation — D8 demotes TimescaleDB off the V0 stack. Re-derived 2026-06-09 per [observability baseline](../src/architecture/overview.md#observability) (E1 dispositioned = re-derive now): the observability metrics/events surfaces (R-0004) are re-derived to storage-independent emission (stdout/OTel), not in-app hypertables; there is no in-app observability store at V0. See R-0004 and R-0013-c.)*

### R-0002 — Core plugin partition (anchors [P-0002](../src/adrs/P-0002-core-plugin-partition.md))

- R-0002-a: The system SHALL designate `tasks`, `repos`, `jobs`, and `contacts` as `core: true` plugins, each running as a Wasmtime-hosted WASM Component Model module, signed by the mnemra root, structurally non-uninstallable at runtime.
- R-0002-b: The system SHALL designate workspace, users, agents, authentication, agent sessions, per-plugin permissions, and projects as builtin components compiled into the host process; builtins SHALL NOT execute inside the Wasmtime plugin sandbox.
- R-0002-c: The system SHALL initialize all seven builtins before any plugin is loaded; no plugin invocation SHALL precede builtin startup completion.
- R-0002-d: The system SHALL block runtime uninstallation of any `core: true` plugin; the only path to remove a `core: true` plugin is a binary rebuild.
- R-0002-e: The increment-to-plugin assignment (which of the `0.2.0`–`0.14.0` capability families map to which of the four `core: true` plugins) SHALL be ratified by the maintainer before the `0.2.0` implementation dispatch; the assignment is flagged as a Spec-stage open item in Out of Scope.
- R-0002-f: The content-corpus migration handler (`0.14.0`) SHALL execute as a builtin migration handler, NOT as a `core: true` plugin, because migration is a one-shot destructive control-plane operation gated on the admin token.

### R-0003 — Plugin manifest schema (anchors [P-0003](../src/adrs/P-0003-plugin-manifest.md))

- R-0003-a: Every `core: true` plugin SHALL ship a manifest file at `<plugin-name>/manifest.toml` declaring `schema_version = 1`, `core = true`, `name`, `version`, and the `[verbs]`, `[content_types]`, `[state_scopes]`, `[host_fns]`, and `[signature]` sections as defined in [P-0003](../src/adrs/P-0003-plugin-manifest.md).
- R-0003-b: The runtime SHALL compile a per-instance host-fn allowlist from the signed manifest's `[host_fns]` section before any plugin instance is created; host-fn calls outside the allowlist SHALL fail at the WIT boundary, not at the host-fn body.
- R-0003-c: The runtime SHALL load manifest files using a `schema_version` branch; a V0 plugin presenting `schema_version: 1` to a newer runtime SHALL continue to load; a future manifest presenting an unknown `schema_version` SHALL produce a structured load error.
- R-0003-d: Plugin manifests SHALL NOT include `workspace_id` as a parameter on any write-path host-fn declaration; the host SHALL derive workspace context from the `WorkspaceCtx` bound at request ingress. Note: the [P-0003](../src/adrs/P-0003-plugin-manifest.md) manifest example shows `workspace_id: str` in the inline comment for `projection.emit` — this illustrates the host-fn's internal full signature (including host-derived parameters); it is NOT a plugin-callable parameter. The manifest `[host_fns]` section governs access control (which functions a plugin may call), not the calling convention. The normative calling convention is the API Contract table: `projection.emit(projection_name: str, data: JSON) -> ()`.
- R-0003-e: The runtime SHALL reject at load time any plugin manifest where `core = false`; non-core plugin installation is V0.1+ scope.
- R-0003-f: The runtime SHALL validate plugin output against the WIT-declared schema; size caps per field SHALL be enforced; the parser SHALL fail shut on schema mismatch rather than truncating.
- R-0003-g: The `artifact.delete` host-fn SHALL require explicit declaration in the manifest's `host_fns.required` array; it SHALL NOT be granted by default.

### R-0004 — Observability shape (anchors the [observability baseline](../src/architecture/overview.md#observability); re-altituded out of the [P-0004](../src/adrs/P-0004-observability-shape.md) ADR framing)

*Re-derived 2026-06-09 (E1 disposition: separate observability **generation** from **storage**) and re-altituded out of the project-ADR layer to the [observability baseline](../src/architecture/overview.md#observability) (a theory-trait baseline; observability is a theory trait + chassis mechanism, not a per-project ADR). The server EMITS telemetry storage-independently from the bare shell; it does NOT own where telemetry lands. The original P-0004 framing (TimescaleDB metrics/events hypertables + retention policies + continuous aggregate, all in the app's own Postgres) is removed — it baked observability storage into the substrate; P-0004 is `deprecated` (no successor ADR). The V0 observability storage backend is deferred behind the separation (option set {Prometheus, InfluxDB, TimescaleDB, plain Postgres tables}, named tripwire); the bare shell carries no in-app observability store.*

- R-0004-a: The system SHALL emit a per-verb metric record on every MCP verb dispatch with the V0 emission floor: `workspace_id` (UUID), `verb` (TEXT), `outcome` (TEXT, one of `"ok"`/`"error"`/`"timeout"`), `duration_ms` (INT, wall-clock), `recorded_at` (TIMESTAMPTZ). The emission SHALL be OpenTelemetry (OTel) metric/structured form, exportable to a configurable OTLP endpoint; no artifact IDs, content fragments, or agent identity SHALL appear in the metric record. The metric record SHALL NOT be written to an in-app observability hypertable or table at V0.
- R-0004-b: The system SHALL emit an event record with the V0 emission floor: `workspace_id` (UUID), `event_type` (TEXT), `event_version` (INT, DEFAULT 1), `token_id` (UUID, nullable; derived by host from `WorkspaceCtx`, not plugin-supplied), `agent_id` (UUID, nullable), `session_id` (UUID, nullable), `payload` (JSON; never contains artifact bodies), `recorded_at` (TIMESTAMPTZ). The event SHALL be emitted as a structured/OTel record; it SHALL NOT be written to an in-app observability hypertable at V0.
- R-0004-c: The system SHALL NOT initialize any in-app observability storage at V0 — no metrics/events TimescaleDB hypertable, no observability `add_retention_policy`, and no continuous aggregate. *(Observable: after `mnemra init`, `\d+` lists no metrics/events hypertable and `\dx` does not list `timescaledb`.)* The observability storage backend is deferred behind the generation⊥storage separation (observability baseline, L4 tripwire); telemetry retention at V0 is the operator's stdout/scrape-target retention, not an in-app policy. A persistent observability store is adopted on the named tripwire (when persistent observability storage becomes load-bearing for a real operator deployment).
- R-0004-d: The system SHALL emit structured log records to **stdout** (one structured/JSON line per record), carrying a level, message, timestamp, and `workspace_id` on every tenant-scoped record. Log routing, shipping, and retention are the operator's process-supervisor/log-shipper concern at V0; the system SHALL NOT write logs to an in-app Postgres `logs` table or run an in-app log-retention worker at V0. *(This retires the P-0004 in-app `logs` table + 30-day retention worker — an in-app storage sink the generation⊥storage separation removes.)*
- R-0004-e: *(Retired — re-derived to R-0004-c.)* The P-0004 continuous-aggregate-over-the-metrics-hypertable requirement is removed: there is no in-app metrics hypertable at V0. `p50`/`p95`/`p99` per `(verb, workspace_id, outcome)` remain **derivable** from the emitted `duration_ms` values by the operator's observability backend (R-0004-a guarantees the raw distribution is emitted); the aggregate, if wanted, is computed at the operator-chosen sink, not materialized in the app's substrate.
- R-0004-f: The system SHALL emit a per-verb metric record (per R-0004-a) to the OTel/stdout surface on every MCP verb dispatch, recording `workspace_id`, `verb`, `outcome`, and `duration_ms`. Emission SHALL succeed and the metric SHALL be observable on the emission surface even when no persistent observability backend is configured (storage-independent emission).
- R-0004-g: The system SHALL expose a `GET /health` endpoint as the **first API**, started before config load and before the MCP server begins accepting requests, on a dedicated loopback-only TCP listener. This listener SHALL bind to `127.0.0.1` only (not `0.0.0.0` or `::`) at V0. The listener SHALL NOT be the MCP stdio transport — it is a separate, minimal HTTP server for liveness and readiness probing only, and does NOT constitute an HTTP MCP transport for the purposes of R-0010-e. The listener port SHALL be configurable via `MNEMRA_HEALTH_PORT` env var (default: `8877`). The listener SHALL serve only `GET /health`; it SHALL NOT serve any other HTTP routes at V0. Because the listener binds loopback-only (`127.0.0.1`) at V0, **the loopback bind IS the gate on the detail body** — every caller that can reach the listener is necessarily on loopback, so the `/health` endpoint SHALL return the structured detail body to every caller it can serve: `{ "postgres": bool, "pgvector": bool, "workspace_default": bool, "overall": "ok" | "degraded" | "down" }`. There is no admin-token gating on the detail body at V0. *(Named tripwire: if the `/health` listener ever binds a non-loopback interface, admin-token gating on the detail body becomes required — at that point the detail body SHALL be served only to callers presenting a valid admin token, and unauthenticated callers SHALL receive a status-code-only response.)* The body reports the substrate dependencies the standalone binary owns (the embedded Postgres engine and its bundled `pgvector`); the `timescaledb` field is removed (TimescaleDB is not a V0 substrate dependency per [P-0010](../src/adrs/P-0010-storage-substrate-engine.md) D8).
- R-0004-h: The system SHALL NOT include artifact bodies, content fragments, or raw query strings in any emitted metric, event, or log record; redaction at the emission (log-write) boundary SHALL be enforced for high-entropy strings.
- R-0004-i: The `event_version` field on every event record SHALL be incremented on breaking event-type schema changes; backward-compatible additions SHALL NOT increment `event_version`.

### R-0005 — Signing chain (anchors [P-0005](../src/adrs/P-0005-v0-signing-chain.md))

- R-0005-a: Plugin signature verification SHALL be synchronous on plugin load; the host SHALL NOT create a plugin instance until `verify()` returns `Ok`; no "verify-async" or "defer to background" path SHALL exist in the load pipeline.
- R-0005-b: If signature verification fails (malformed signature, unknown key, certificate-chain break), the plugin load SHALL be rejected with a structured error naming the plugin; no "best-effort load" path is permitted.
- R-0005-c: The signing key SHALL reside on the build host's filesystem at mode 600, owner = build-pipeline process UID; the signing key SHALL NOT be co-located on the deployment node; the deployment node SHALL receive only signed artifacts and verification material.
- R-0005-d: The verification material (root public key / cert) SHALL be embedded in the mnemra-core binary at build time; no runtime key-fetch path is permitted at V0.
- R-0005-e: The system SHALL fire the multi-deployment trip-wire — requiring `{{P-SigningKeyCustodyHardening}}` (Tier-C ADR, not yet authored) to be authored before any deployment beyond the maintainer's single dogfood instance proceeds.
- R-0005-f: On host startup, the system SHALL check that the admin-token file and signing-verification-material file are both mode 600 and not world-readable; if either check fails, the host SHALL refuse to start.
- R-0005-g: The runtime SHALL reject at V0 any plugin whose manifest does not carry `core: true` signed by the mnemra root; non-core plugin installation is blocked at the load path.
- R-0005-h: The runtime SHALL determine `core` status by signature provenance, NOT by manifest-field trust. A manifest carrying `core = true` SHALL be honored as core ONLY when its signature chains to the mnemra root verification material; a manifest with `core = true` signed by any other key SHALL be rejected at load, regardless of whether non-core plugin installation is enabled (V0) or opened in future stages (V0.1+). The core-vs-non-core determination is structural, bound to the mnemra-team-defined identity of the four core plugins; the binding SHALL NOT be relaxed when V0.1+ non-core plugin installation opens.

### R-0006 — Tenant enforcement (anchors [P-0006](../src/adrs/P-0006-v0-tenant-enforcement.md))

- R-0006-a: Every host-fn signature SHALL take a `WorkspaceCtx` as its first typed parameter; the compiler SHALL enforce this; it SHALL NOT be possible to author a host-fn implementation that issues a database query without receiving a `WorkspaceCtx`.
- R-0006-b: `WorkspaceCtx` SHALL be constructed at a single location in the MCP verb dispatch path, after token validation; there SHALL be no alternative construction path in production code.
- R-0006-c: `workspace_id` extraction from `WorkspaceCtx` SHALL use the `WorkspaceCtx::workspace_id()` accessor only; direct field access SHALL be private; raw `workspace_id` parameters on write-path host-fns are prohibited by ABI. Implementation note: [P-0009](../src/adrs/P-0009-rls-admin-token.md)'s `WorkspaceCtx` struct example shows `pub workspace_id` to illustrate the logical field — this is illustrative of field presence, not its Rust visibility. The normative requirement from R-0006-c takes precedence: the field SHALL be private (`workspace_id: Uuid`), with a public accessor `pub fn workspace_id(&self) -> Uuid`.
- R-0006-d: All read-path host-fns SHALL include `workspace_id = ctx.workspace_id` as a WHERE-clause condition, derived from the `WorkspaceCtx` argument, not as a post-read filter; a CI lint check SHALL assert this on all read paths.
- R-0006-e: Builtin components SHALL use the same `WorkspaceCtx` threading as plugins; there SHALL be no "internal" bypass path that issues a database query without a `WorkspaceCtx` argument.
- R-0006-f: Test harnesses SHALL construct `WorkspaceCtx` via a test-only constructor gated to `#[cfg(test)]`; this constructor SHALL NOT be callable in production code paths.
- R-0006-g: The RLS column-shape (`workspace_id NOT NULL` on every artifact table) SHALL ship at `0.1.0`; RLS policy objects (CREATE POLICY statements) SHALL NOT be activated at V0; they are additive V0.1+ additions per the accepted-risk `R-0001` trip-wire.

### R-0007 — Plugin resource limits (anchors [P-0007](../src/adrs/P-0007-plugin-resource-limits.md))

- R-0007-a: The Wasmtime engine configuration SHALL enable fuel metering via `Store::set_fuel`; the V0 fuel ceiling SHALL be 10,000,000,000 ticks (10B) per verb invocation.
- R-0007-b: The Wasmtime engine configuration SHALL enable epoch-interruption; the V0 epoch deadline SHALL be 5 seconds wall-clock per verb invocation, achieved via `Store::set_epoch_deadline(500)` with a host epoch-tick thread advancing the counter every 10ms.
- R-0007-c: The Wasmtime per-instance memory ceiling SHALL be 64 MiB, enforced via `Config::static_memory_maximum_size` or a `ResourceLimiter`.
- R-0007-d: Table and instance limits SHALL use Wasmtime defaults; plugin instance count SHALL be bounded by the pool size (3–5 per plugin type at V0).
- R-0007-e: When any resource-limit violation fires (fuel exhaustion, epoch deadline, memory ceiling), the system SHALL: (1) catch the Wasmtime trap; (2) emit a structured event with `(workspace_id, plugin_id, plugin_version, limit_type, limit_value)` (destination per R-0004); (3) poison the pool slot and replace it with a new instance; (4) return a structured error to the caller for the current verb invocation.
- R-0007-f: The system SHALL NOT propagate a Wasmtime trap as a host-process panic; kill-and-replace is the recovery invariant.
- R-0007-g: Both fuel AND epoch-interruption SHALL be active simultaneously at V0; disabling either is not permitted.
- R-0007-h: The host epoch-tick thread SHALL be started before any plugin is invoked; the thread SHALL be supervised and SHALL NOT be restarted silently on crash. On epoch-tick thread crash, the host SHALL emit a structured event of type `epoch_tick_thread_died` (destination per R-0004); the host SHALL refuse to accept new plugin invocations until the thread is confirmed restarted; the host SHALL attempt one supervised restart per minute with backoff. The `overall` field of the `/health` response SHALL reflect `"degraded"` while the epoch-tick thread is dead.
- R-0007-i: The Wasmtime version SHALL be pinned in `Cargo.toml` (no wildcard `*` or open `>=` version constraints for the Wasmtime crate); the pinned version SHALL be recorded in the SBOM produced by `verify-build`. An upgrade to a new Wasmtime major or minor version SHALL require explicit approval (per the workspace dependency-approval model); it SHALL NOT be auto-merged by dependabot or equivalent.

### R-0008 — Admin token shape (anchors [P-0008](../src/adrs/P-0008-admin-token-shape.md))

- R-0008-a: The admin token value SHALL be 32 bytes cryptographically random, base64url-encoded (43 characters without padding); no structural content SHALL be encoded in the token bytes.
- R-0008-b: The system SHALL store `BLAKE3(token_bytes)` in `admin_tokens.token_hash`; the raw token bytes SHALL NOT be stored in the database. Token authentication SHALL look the row up by `BLAKE3(presented_token_bytes)` against the unique `token_hash` column. Because the admin token is a 256-bit CSPRNG value (R-0008-a) compared via its BLAKE3 hash, comparison-timing attacks are not applicable and no constant-time comparison primitive is required on this path. (Constant-time comparison remains required where it does apply, e.g. the signing-chain verification path per P-0005.)
- R-0008-c: The `admin_tokens` table SHALL have the schema: `id UUID PK, token_hash BYTEA NOT NULL UNIQUE, workspace_id UUID NOT NULL, scopes TEXT[] NOT NULL, created_at TIMESTAMPTZ NOT NULL, rotated_at TIMESTAMPTZ`.
- R-0008-d: The `workspace_id` column in `admin_tokens` SHALL be NOT NULL; a token row with NULL `workspace_id` is a schema violation; absence of a workspace claim SHALL cause a hard auth failure, not a default to any workspace.
- R-0008-e: The token value SHALL be written to the filesystem at mode 600, owner = host process UID, on first-run generation. The default token file path SHALL be `~/.config/mnemra/token`; it SHALL be overridable via the `MNEMRA_TOKEN_FILE` environment variable. The startup file-mode invariant check (R-0005-f) SHALL resolve the path through the same `MNEMRA_TOKEN_FILE` override.
- R-0008-f: Token revocation SHALL be implemented as a single `admin_tokens` row deletion followed by new token generation; no block-list mechanism is required at V0.
- R-0008-g: Token rotation SHALL emit a structured event (destination per R-0004) before the old row is deleted; the rotation event SHALL carry the `token_id` of the rotated token.
- R-0008-h: The system SHALL NOT introduce a second signing key for admin token minting at V0; the only signing key is the plugin signing key per [P-0005](../src/adrs/P-0005-v0-signing-chain.md).

### R-0009 — Role model and permission shape (anchors [P-0009](../src/adrs/P-0009-rls-admin-token.md))

- R-0009-a: The V0 role model SHALL be a binary enum: `Admin` and `ReadObserver`; no other roles are valid at V0.
- R-0009-b: The `WorkspaceCtx` struct SHALL carry `workspace_id: Uuid`, `role: Role`, and `token_id: Uuid`; `token_id` SHALL be used for per-token write attribution in every audit event. The host SHALL derive `token_id` from the calling `WorkspaceCtx` and set it on the `token_id` field of the emitted event record on every `event.emit` call (destination per R-0004); this field is NOT plugin-supplied and SHALL NOT appear in the `event.emit` host-fn parameter list. A plugin-supplied `token_id` SHALL be rejected at the WIT boundary.
- R-0009-c: The `Admin` role SHALL authorize all MCP verb categories (content read and write, event/log emission via plugin host-fn), all CLI control-plane operations (workspace lifecycle, token rotation, migration trigger, backup trigger), and admin session management. *(The "metrics read" verb category granted in the prior framing is removed: under the re-altituded observability baseline there is no in-app observability store to read against — metrics are emitted to the operator-chosen sink, not queried through an MCP verb. Emission via the host-fn surface is unaffected.)*
- R-0009-d: The `ReadObserver` role SHALL authorize only read-path MCP verbs (`artifact.get`, `artifact.list`, projection queries); write verbs, CLI control-plane operations, and workspace lifecycle operations SHALL be denied at the host-fn boundary. *(The "metrics/events read" verb granted in the prior framing is removed: under the re-altituded observability baseline there is no in-app observability store to read against — observability is emitted to the operator-chosen sink, not queried through an MCP verb.)*
- R-0009-e: Workspace lifecycle operations (`workspace create`, `workspace delete`) SHALL require `Role::Admin`; a `Role::ReadObserver` request for these operations SHALL return a structured permission error.
- R-0009-f: The `admin_tokens.scopes` array SHALL use scope strings `"admin"` and `"read_observer"` to encode the role; the host SHALL derive `Role` from the scopes array at `WorkspaceCtx` construction time.
- R-0009-g: The system SHALL NOT activate Postgres RLS policies (`CREATE POLICY` statements) at V0; the permission matrix in R-0009-c and R-0009-d SHALL be enforced at the application layer (host-fn boundary) only; RLS policy objects are reserved for V0.1+ additive `CREATE POLICY` migration.
- R-0009-h: The V0.1+ RLS policy migration path SHALL be additive: `CREATE POLICY` statements use `current_setting('mnemra.workspace_id')` and `current_setting('mnemra.role')` set via `SET LOCAL` from `WorkspaceCtx`; no schema migration is required.
- R-0009-i: Old-key tokens SHALL be invalidated immediately on rotation: subsequent DB lookups against the old hash SHALL produce "not found" → reject; no grace period.

### R-0010 — MCP server (anchors brief Hard constraints, [P-0003](../src/adrs/P-0003-plugin-manifest.md))

- R-0010-a: The system SHALL run a single MCP server using the MCP specification 2025-06-18 with stdio transport at V0.
- R-0010-b: Plugin verbs SHALL be namespaced: `"<plugin>.<verb>"` (e.g., `"task.create"`, `"task.list"`); all verbs from all loaded plugins SHALL be served from the single MCP server.
- R-0010-c: The MCP handler SHALL perform `DF-auth-check` (P-builtin-auth token verification) on every incoming request before routing to any builtin or plugin handler.
- R-0010-d: The MCP handler SHALL enforce a per-verb capability check against the plugin manifest's declared `verbs` list before dispatching to the plugin runtime.
- R-0010-e: Streamable-HTTP MCP transport SHALL NOT be activated at V0; it is a V0.1+ activation conditional on the microVM-appliance trip-wire.
- R-0010-f: The MCP handler SHALL return distinguishable JSON-RPC error codes for: invalid token (authentication failure), verb not found, parameter invalid; error code classes SHALL NOT be conflated.
- R-0010-g: Control-plane operations SHALL NOT be exposed as MCP verbs; agent-facing CRUD routes through MCP; destructive and control-plane operations route through the admin CLI only.

### R-0011 — Admin CLI (anchors brief Hard constraints, [P-0002](../src/adrs/P-0002-core-plugin-partition.md))

- R-0011-a: The system SHALL provide an admin CLI whose subcommands are schema-driven and dynamically generated from plugin manifests at startup; a new plugin whose manifest declares new verbs SHALL produce new CLI subcommands without a CLI code change.
- R-0011-b: The admin CLI SHALL handle destructive and control-plane operations only; agent-facing CRUD operations SHALL NOT be exposed on the admin CLI.
- R-0011-c: Destructive CLI operations SHALL require admin token authentication; UNIX UID match alone SHALL NOT constitute sufficient authorization for destructive operations.
- R-0011-d: The admin CLI SHALL expose: `workspace create`, `workspace delete`, `workspace list`, `token rotate`, `migrate` (one-shot trigger), `backup` (trigger), and `health` (human-readable wrapper over the `/health` endpoint).

### R-0012 — Host-fn ABI structural invariants (anchors [P-0003](../src/adrs/P-0003-plugin-manifest.md), [P-0006](../src/adrs/P-0006-v0-tenant-enforcement.md))

- R-0012-a: The host-fn ABI SHALL declare the universal `content.emit` verb shape over JSONB frontmatter + body; the required host-fns for a typical work-verb plugin SHALL include: `artifact.create`, `artifact.update`, `artifact.get`, `artifact.list`, `artifact.delete` (opt-in only), `metrics.record`, `log.emit`, `event.emit`, `projection.emit`. *(Amended 2026-06-29 — artifact-list keyset pagination, R-0020: the `artifact.list` host-fn signature gains `limit: u32` and `cursor: option<string>` and returns the `artifact-page` record `{ ids: list<string>, has-more: bool, next-cursor: option<string> }` in place of the prior `[{ id, frontmatter }]` shape; the concrete signature is reconciled in the API Contract Host-fn ABI table. The new paging params and record are WIT component types per R-0012-f. The return **narrows to ids-only** — no inline frontmatter — so a caller reads frontmatter via `artifact.get` per id (a deliberate semantic narrowing locked by R-0020-b and **intake Success criterion 1**, which fixes the host-fn return as the `{ ids, has-more, next-cursor }` ids-only record; not a doc fix). *(Anchor correction: the ids-only narrowing anchors to intake SC1, not Frame D4 — D4 understated the import baseline by modeling the prior return as `list<string>`, whereas the host-fn import baseline was `[{ id, frontmatter }]` with frontmatter inline, so the import side genuinely drops inline frontmatter.)* R-0012-a is the import-side governing anchor for the behavior locked in R-0020.)*
- R-0012-b: The `sampling.request` host-fn SHALL be an optional ABI surface declared per-plugin in `host_fns.optional`; at V0, only `core: true` plugins MAY declare it; the prompt arguments to `sampling.request` SHALL accept content IDs, not artifact bodies. The host SHALL forward `context_ids` as opaque references to the agent's MCP client; the host SHALL NOT resolve content IDs to artifact bodies before forwarding — artifact bodies never traverse the `TB-external-llm` trust boundary from within mnemra-core. The MCP client is responsible for any ID-to-body resolution on its side of the boundary. (Path B per Warden M3; mnemra-core does not touch body content for sampling.)
- R-0012-c: The `secrets.get` host-fn SHALL be an optional ABI surface; no write path from a plugin to the secrets store SHALL exist at V0.
- R-0012-d: `workspace_id` SHALL NOT appear as a parameter on any write-path host-fn; the ABI enforces this structurally via the `WorkspaceCtx` first-parameter convention in R-0006-a.
- R-0012-e: Each host-fn in the WIT interface definitions SHALL carry an `@stable` or `@unstable` stability annotation; an `@unstable` host-fn invocation SHALL emit a deprecation warning to the log; a `@deprecated` host-fn invocation SHALL return a structured error. *(Amended 2026-06-29 — R-0020 / R-0017-a: the `artifact.list` return-type change (`[{ id, frontmatter }]`/`list<string>` → `artifact-page` record) is a **breaking** ABI change; the changed `artifact.list` (import) and `content.list` (export) functions SHALL carry the stability annotation reflecting the break — the exact mechanics (version bump vs `@unstable` mark) are a plan-tier concern. Requirement text otherwise unchanged.)*
- R-0012-f: All host-fn parameters and return types SHALL be defined as WIT component types; raw byte buffers with dynamic type dispatch SHALL NOT be used. *(Amended-note 2026-06-29 — R-0020: the artifact-list pagination `limit: u32` and `cursor: option<string>` params and the `artifact-page` return record are WIT component types — this structural invariant is **satisfied by** the R-0020 signature change, not relaxed by it.)*

### R-0013 — Storage substrate initialization (anchors [P-0001](../src/adrs/P-0001-storage-layout.md), [P-0010](../src/adrs/P-0010-storage-substrate-engine.md), [observability baseline](../src/architecture/overview.md#observability))

- R-0013-a: On first-run (`mnemra init`), the system SHALL bootstrap the schema on the **embedded Postgres engine** (`postgresql_embedded`, shipping with the single self-hosted binary — per [P-0010](../src/adrs/P-0010-storage-substrate-engine.md); not an operator-provisioned external Postgres server): invoke `CREATE EXTENSION IF NOT EXISTS pgvector` (pgvector is **bundled/compiled with the embedded engine** via `pgvector_compiled` — `mnemra init` enables it; it does NOT require an OS-installed `pgvector` binary on an external server); create all substrate tables and indexes; create the `default` workspace; and emit a health-check that returns `overall: "ok"`. *(The original "declare all retention policies" step is removed — retention policies were observability-hypertable artifacts; there is no in-app observability store at V0 per R-0004-c.)* If the `pgvector` extension cannot be enabled (i.e., `CREATE EXTENSION` returns an error), `mnemra init` SHALL return a structured error naming the missing extension and SHALL NOT proceed with schema creation. *(Amended 2026-06-08 per [P-0010](../src/adrs/P-0010-storage-substrate-engine.md): the engine is embedded (not external) and pgvector is bundled (not OS-installed). Re-derived 2026-06-09 per [observability baseline](../src/architecture/overview.md#observability) (E1 dispositioned = re-derive now): the original `CREATE EXTENSION IF NOT EXISTS timescaledb` step is removed — TimescaleDB is NOT a V0 extension (D8 demotes it; observability is emitted, not stored in-app per R-0004). `mnemra init` enables only the bundled `pgvector` against the embedded engine; the `timescaledb` health-body field is removed (R-0004-g).)*
- R-0013-b: The system SHALL partition the **storage** substrate into two persisted logical shapes at V0: content (`DS-pg-content`) and state-config (`DS-pg-state`), both regular Postgres tables. *(Re-derived 2026-06-09 per [observability baseline](../src/architecture/overview.md#observability): the former timeseries (`DS-ts-metrics`, `DS-ts-events`) and log (`DS-pg-logs`) shapes are observability **emission** surfaces — stdout/OTel telemetry, not in-app storage partitions — at V0; they land in an operator-chosen sink behind the generation⊥storage separation, not in the app's own substrate.)*
- R-0013-c: Content and state-config data SHALL use regular Postgres tables. Observability telemetry (the former `DS-ts-metrics`/`DS-ts-events` surfaces) is **emitted** (stdout structured logs + OTel metrics/events per R-0004), not stored in an in-app table or hypertable at V0; the observability storage backend is deferred behind the generation⊥storage separation (option set {Prometheus, InfluxDB, TimescaleDB, plain Postgres tables}, named tripwire). Logs are emitted to stdout (R-0004-d), not an in-app `logs` table. *(Re-derived 2026-06-09 per [P-0010](../src/adrs/P-0010-storage-substrate-engine.md) D8 + [observability baseline](../src/architecture/overview.md#observability): E1 dispositioned = re-derive now. TimescaleDB is demoted off the V0 stack; the observability metrics/events surfaces P-0004 had committed to hypertables are re-derived to storage-independent emission. No in-app observability hypertable, retention policy, or continuous aggregate exists at V0.)*
- R-0013-d: All schema migrations SHALL be forward-only and SHALL work against both empty and populated databases; no destructive schema migration SHALL be run without a verified pre-migration backup.
- R-0013-e: The system SHALL create database roles with least-privilege grants per host-process surface: a host-fns role, a migration role, a backup role, and a health-probe role; each role SHALL have only the minimum grants required for its operations.

### R-0014 — LLM-API-key configuration (anchors brief T-5 resolution, product brief `0.1.0`)

- R-0014-a: The system SHALL provide a per-deployment LLM-API-key configuration surface for the embedding-batch pathway (`DF-embed-call`); the API key SHALL be configurable at deploy time, never hard-coded.
- R-0014-b: The system SHALL enforce a hostname allowlist for outbound embedding calls; the allowlist SHALL be configurable per deployment; any outbound call to a hostname not in the allowlist SHALL be blocked.
- R-0014-c: The system SHALL NOT host a language model; embedding generation and MCP sampling SHALL call out to an external provider; the system SHALL NOT accept an API key for a hosted model endpoint.
- R-0014-d: The LLM-API-key SHALL be stored in a file at mode 600, separate from the admin token file; the startup file-mode invariant check SHALL cover both files.

### R-0015 — Builtin identity core (anchors brief `0.1.0`, [P-0002](../src/adrs/P-0002-core-plugin-partition.md))

- R-0015-a: The system SHALL initialize the `workspaces` builtin that manages workspace lifecycle (create, delete, list); the `default` workspace SHALL be created on first-run initialization.
- R-0015-b: The system SHALL initialize the `users` builtin managing user identity records; users SHALL be referenced by agent and session state.
- R-0015-c: The system SHALL initialize the `agents` builtin managing agent registration; agents SHALL be tied to user-workspace pairs; agent identity derivation SHALL be canonical at registration and SHALL produce a structured error on mismatch rather than silent registration.
- R-0015-d: The system SHALL initialize the `authentication` builtin implementing the static admin token bootstrap path per [P-0008](../src/adrs/P-0008-admin-token-shape.md) and [P-0009](../src/adrs/P-0009-rls-admin-token.md); per-deployment OIDC AS configuration via RFC 9728 protected-resource-metadata SHALL be available at V0 substrate (full OIDC AS integration is V0.1+).
- R-0015-e: The system SHALL initialize the `sessions` builtin managing per-MCP-connection session state; session context SHALL be the source of `WorkspaceCtx` construction per R-0006-b.
- R-0015-f: The system SHALL initialize the `permissions` builtin managing per-plugin permission grants; permission checks for plugin verb access SHALL run at the host layer before plugin dispatch.
- R-0015-g: The system SHALL initialize the `projects` builtin managing project registry state; project identity SHALL be a prerequisite for plugin scoping; no plugin SHALL be scoped to a project before that project's record exists.
- R-0015-h: Solo deployment SHALL collapse workspace tenancy to the `default` workspace; the `default` workspace SHALL always exist after first-run initialization.

### R-0016 — Plugin pool (anchors [P-0007](../src/adrs/P-0007-plugin-resource-limits.md), Frame component map)

- R-0016-a: The system SHALL maintain a plugin instance pool of 3–5 instances per plugin type; the pool SHALL be initialized at host startup, before the MCP server begins accepting requests.
- R-0016-b: Plugin instances SHALL be stateless with respect to tenant; every call SHALL carry the workspace-scoping key via `WorkspaceCtx`; no cross-call state SHALL be held in a plugin instance.
- R-0016-c: A killed-and-replaced plugin instance SHALL be replaced synchronously in the pool before the verb invocation error is returned to the caller; the pool size SHALL not decrease as a result of a kill event.
- R-0016-d: Adaptive pool sizing is V0.1+ scope; V0 SHALL use fixed pool size.

### R-0017 — Forward-compatibility and ABI evolution discipline

- R-0017-a: The host-fn ABI operates under pre-1.0 SemVer freedom within `0.y.z`; an ABI-change PR SHALL cause all `core: true` plugins to recompile against the new ABI and pass their tests before merge. *(Triggering instance recorded 2026-06-29: the R-0020 artifact-list keyset-pagination return-type change is a breaking ABI change on both the import (`artifact.list`) and export (`content.list`) directions; this requirement's recompile-all-`core: true`-plugins-and-pass-tests-before-merge discipline fires for the R-0020 PR — asserted at the merge gate, not after. **Rollback (single-binary model):** rolling back this breaking ABI change is a revert of the PR + rebuild — the host and all `core: true` consumers revert **in lockstep** within the single binary, and no partial-ABI state persists (there is no mixed-version fleet to reconcile, so no consumer can observe one direction of the ABI ahead of the other). This lockstep-rollback guarantee holds **only while the single-binary deployment model holds**; it would no longer hold under a multi-binary / independently-deployed-plugin model, where a partial-ABI state could persist across a rollback. Requirement text unchanged.)*
- R-0017-b: The `schema_version` field in plugin manifests SHALL make manifest format evolution mechanical: V0 plugins with `schema_version: 1` SHALL continue to load against a future runtime that supports schema_version > 1.

### R-0018 — Testing and build discipline (anchors brief Hard constraints, overview constraint inventory, [G-0006](../../../../DEFAULTS.md))

- R-0018-a: Non-trivial implementation surfaces (security boundaries, public APIs, host-fn ABI, parsers, validators) SHALL use Test-Driven Development pairs: a red-phase test task precedes the implementation task.
- R-0018-b: Integration tests SHALL run against the real surface (HTTP handlers, actual Postgres + extension instances), not mocks of the database.
- R-0018-c: All code changes SHALL be in worktrees; main SHALL be protected from direct pushes.
- R-0018-d: A CI lint check SHALL assert 100% read-path WHERE-clause coverage before any merge; the lint check SHALL fail the build if a read-path host-fn is added without a `workspace_id` WHERE-clause condition. The lint check SHALL be implemented as a `cargo test --test lint_workspace_clause` integration test (using `syn` AST parsing of host-fn source files); a planted violation (read-path host-fn without `workspace_id` WHERE-clause) MUST return non-zero from the lint check and name the offending function.
- R-0018-e: The binary SHALL be built with Rust; non-Rust paths SHALL be adopted only where no viable in-stack path exists.
- R-0018-f: The project SHALL provide a `Justfile` anchored to [G-0006](../../../../DEFAULTS.md) with the following fixed recipe names: `verify-test`, `verify-lint`, `verify-type`, `verify-coverage`, `verify-build`, `verify-smoke`, and `ci`. `just ci` SHALL be the sole CI entry point; it SHALL invoke all `verify-*` recipes. Each `verify-*` recipe SHALL emit `GATE <name> <PASS|FAIL>` on stdout. No `verify-*` recipe SHALL have `--fix` side effects. The `verify-build` recipe SHALL produce the signed binary with the embedded root verification material (per R-0005-d). The `verify-lint` recipe SHALL include the WHERE-clause lint check from R-0018-d.

### R-0019 — Plugin invocation / export ABI (anchors [P-0013](../src/adrs/P-0013-plugin-invocation-model.md))

*(Added 2026-06-20 per [P-0013](../src/adrs/P-0013-plugin-invocation-model.md) — "Plugin Invocation Model (typed per-verb exports)"; maintainer-approved scoped amendment. P-0013 records the locked invocation ABI: the WIT world declares a typed `content` interface every content plugin exports, and the host invokes the exact typed export per authenticated verb. This requirement pins the **export / invocation** side of the host↔guest boundary; R-0012 ("Host-fn ABI structural invariants") covers the **import** (host-fn) side. P-0013 fills the export gap R-0012 left.)*

- R-0019-a: The plugin invocation ABI SHALL be typed per-verb exports. The WIT world SHALL declare a typed universal `content` interface — `create`, `get`, `list`, `update`, `delete` — that every content plugin exports; the host SHALL invoke the exact typed export corresponding to an authenticated verb. The universal `run(input: string) -> string` string-dispatch export SHALL NOT exist on the V0 surface (it is RETIRED). *(The requirement is stated for every content plugin so it is forward-applicable to any content plugin, not only V0's `core: true` class; at V0 the substrate loads only `core: true` plugins per R-0005-g, so the V0-loadable population this applies to is the `core: true` content plugins.)* *(Amended 2026-06-29 — artifact-list keyset pagination, R-0020: the typed `content.list` export gains `limit: u32` and `cursor: option<string>` (typed WIT params, **no `ctx`** — the host carries `WorkspaceCtx` across the export boundary) and returns the `artifact-page` record in place of `list<...>`; the concrete signature is reconciled in the API Contract Plugin export / invocation ABI table. The symmetric-typing-both-directions invariant (R-0019-b) holds — the paging params are explicit typed params, not a string/JSON dispatch hatch. R-0019-a is the export-side anchor for the behavior locked in R-0020.)*
- R-0019-b: The host↔guest boundary SHALL be typed in both directions: the import (host-fn) side per R-0012-f ("all host-fn parameters and return types SHALL be defined as WIT component types") and the export (invocation) side per R-0019-a. No string-based **verb-dispatch** path — a single export that parses its string input to select which verb to run — SHALL exist on the V0 surface. (This prohibits string-based *verb resolution*; it does NOT prohibit JSON-as-string *payloads*: `type json = string` for frontmatter/body crosses the typed boundary on both directions, per the WIT `json` type and R-0012-a.)
- R-0019-c: At V0 the typed `content` interface SHALL be a **fixed** interface resolved statically (plain `wit_bindgen` against the fixed interface); there SHALL be no runtime export registry and no load-time dynamic verb→export resolution. The plugin manifest's declared `verbs` list (per R-0010-d) SHALL function as the **pre-dispatch capability gate** checked before dispatch, NOT as a runtime export registry; verb→export resolution is static.
- R-0019-d: Domain / non-CRUD verbs (e.g., `echo.audit`) are deferred past V0; their export ABI SHALL be typed too when designed (no string hatch). A manifest-declared verb with no matching typed export SHALL be non-dispatchable at V0 and SHALL return a structured error; this non-dispatchable path SHALL NOT alter the pre-dispatch permission path (the `echo.audit` denial is pre-dispatch and is unaffected — the existing permission tests are not changed by the absence of a typed export).
- R-0019-e: The export ABI SHALL be observable on the contract: a built content plugin component SHALL export the typed `content` interface (`create`/`get`/`list`/`update`/`delete`) and SHALL NOT export a `run`-shaped string-dispatch function; an MCP request for a manifest-declared verb with a corresponding typed export SHALL dispatch to that typed export; an MCP request for a manifest-declared verb with no corresponding typed export SHALL return the R-0019-d structured non-dispatchable error and SHALL leave the pre-dispatch permission outcome unchanged.

### R-0020 — Artifact-list keyset (cursor) pagination behavior (anchors [P-0006](../src/adrs/P-0006-v0-tenant-enforcement.md), [P-0013](../src/adrs/P-0013-plugin-invocation-model.md), the [artifact-list keyset-pagination Frame](../intent/artifact-list-paging-frame.md))

*(Added 2026-06-29 per the [artifact-list keyset-pagination Frame](../intent/artifact-list-paging-frame.md) (Frame-exit gate passed, blob `31515f1`) and its [intake](../intent/artifact-list-paging.md) — a maintainer-locked in-place amendment. R-0020 pins the pagination **behavior** contract both interface directions must satisfy: the import-side `artifact.list` host-fn (R-0012-a/-f) and the export-side `content.list` (R-0019-a); the signature changes themselves are reconciled in the API Contract tables and noted on those requirements. Keystone anchor: `P-LockContract` re-derive-on-reshape — the original `-> list<string>` no-paging contract was locked under a "no unbounded-growth pressure on the list path" assumption that measured append-only growth falsified, so the contract is re-derived against the new world (bounded, resumable, hard-capped pages), not honored as frozen. No new project ADR: keyset-over-offset is recorded as a Non-goal rejection (see Out of Scope), and the amendment is pure application of P-0006 / P-0013 / P-0001 plus the spec R-IDs.)*

- R-0020-a (keyset mechanism — fetch-one-extra): The `artifact.list` read path SHALL paginate by **keyset (cursor)** over the ULID-as-text primary key. It SHALL issue `... WHERE workspace_id = ctx.workspace_id AND type = $type AND <filters> AND id > $cursor ORDER BY id LIMIT $effective_limit + 1`, fetching **one extra row** to derive `has-more` without a `COUNT`. The keyset predicate `id > $cursor` SHALL **AND with** the existing `filters` predicate (R-0012-a) — pagination composes with filtering, it does not replace it — and SHALL retain `workspace_id = ctx.workspace_id` as a WHERE-clause condition (R-0006-d, preserved unchanged). Order is by `id` (ULID-as-text PK; lexicographic = chronological), so pages are returned in stable creation order. When `cursor` is absent (`none`), the read path SHALL omit the `id > $cursor` predicate and return from the first row in `id` order. Concurrent-insert semantics are **forward-non-losing, not snapshot-isolated**: under the spec's `id` ordering (ULID-text PK, lexicographic = chronological — see above), a row created after the cursor's row sorts at `id > $cursor` for any cursor already passed, so it appears on a **later** page of the same walk; an already-emitted row (`id <= $cursor`) is never revisited. The walk therefore neither **loses** already-emitted rows nor **duplicates** them, though it MAY **include** rows created after the walk began — this is the defined keyset behavior (a walk is not a point-in-time snapshot), not a gap. *(Binary-observable on the emitted SQL: it contains `ORDER BY id`, `LIMIT $effective_limit + 1`, the `workspace_id =` condition, and — when `cursor` is `some` — the `id > $cursor` predicate; it issues no `COUNT`.)* [Frame D2]

- R-0020-b (page record + the single invariant): `artifact.list` (and the `content.list` export) SHALL return the WIT record `artifact-page { ids: list<string>, has-more: bool, next-cursor: option<string> }` in place of the prior `list<string>` / `[{ id, frontmatter }]` shape (see the API Contract reconciliation; the record is a WIT component type per R-0012-f). The following invariant is **intrinsic to the page contract and locks at V0**: **`has-more = true` if and only if `next-cursor = some`.** When `has-more = true`, `next-cursor` SHALL be the `id` of the **last row** in the returned page (the value a caller passes as `cursor` to resume — `id > next-cursor`); when `has-more = false`, `next-cursor` SHALL be `none` and the returned `ids` SHALL NOT include a `cap+1`-th row. The return is **ids-only**: it carries no inline frontmatter. This is the shape locked by **intake Success criterion 1** (the host-fn `artifact-list` return record `{ ids: list<string>, has-more, next-cursor }`) and **Success criterion 2** (the symmetric guest export) — not a documentation fix. *(Anchor correction: the upstream lock for the ids-only narrowing is **intake SC1/SC2**, not Frame D4. Frame D4 understated the import baseline, modeling the prior return as `list<string>` for both directions, whereas the host-fn import baseline was `[{ id, frontmatter }]` with frontmatter inline; the import side therefore genuinely **drops inline frontmatter**, a narrowing D4 did not address.)*

  **Accepted V0 characteristic — the ids-only return shifts frontmatter hydration to the caller (N+1):** because the page carries no inline frontmatter, a caller wanting frontmatter for a page resolves it via `artifact.get` per id — up to `effective_limit` `artifact.get` calls for a full page (an N+1 fan-out; `artifact.get` returns frontmatter **and** body, and no frontmatter-only get exists at V0, so each call also over-fetches the body). This is the accepted V0 trade-off (consistent with the data-minimization posture also reflected in R-0012-b); the ids-only shape is **provisionally accepted** by the maintainer 2026-06-29, with the revisit gated on dogfood evidence — see the ids-only-return tripwire in Out of Scope. **V0 dogfood instrumentation SHALL capture** (a) the per-`list` `artifact.get` fan-out count and its latency distribution, and (b) the access-pattern signal — **filter-then-fetch** (a targeted `list` whose handful of ids are individually fetched) versus **list-then-N+1-get-for-metadata** (a `list` followed by a full-page `artifact.get` fan-out purely to read frontmatter) — so reality can (i) validate that the fan-out keeps pool pressure within the fixed 3–5-slot budget (R-0016-a) and (ii) feed the ids-only-vs-frontmatter revisit. *(Binary-observable on every response: the `has-more ⇔ next-cursor = some` biconditional holds, including the last-page case `has-more = false ∧ next-cursor = none`; `next-cursor` equals `ids.last()` whenever `some`; and the page carries no inline frontmatter field.)* [intake SC1/SC2 — record shape + ids-only narrowing; Frame D4 — page-record decision (import baseline understated, corrected here); Frame D2 — keyset mechanics]

- R-0020-c (page-size clamp — host-side; the result-set-size DoS bound): The effective page size SHALL default to **exactly 100** and SHALL be hard-capped at **exactly 500**: `effective_limit = (limit == 0 ? 100 : min(limit, 500))` (the `limit = 0` resolution is R-0020-f). The clamp SHALL be enforced **host-side in the `artifact.list` host-fn** — the SQL `LIMIT` owner and the **sole DB chokepoint**: guests are IO-free under the Wasmtime component-model sandbox (R-0002-a/-b), and a guest reaches the DB only through the host-fn ABI (R-0012-a) — and only through the host-fns on its signed per-instance allowlist (R-0003-b), enforced at the WIT boundary — so the host-fn is the necessary-and-sufficient clamp site, independent of any guest-supplied `limit`; a guest-side clamp is optional defense-in-depth only, never the enforcing layer. **No list path SHALL issue an unbounded (`LIMIT`-less) query.** This is the **result-set-size** DoS bound — it closes the *size* half of the prior unbounded-`SELECT` finding; it does **not** bound query **scan cost** (a `LIMIT` bounds rows *returned*, never rows *scanned* — see R-0020-d). *(Binary-observable: a request with `limit = 100_000` against a type holding > 500 artifacts returns `ids.len() <= 500` (clamp) and rows returned `<= cap + 1` (fetch-one-extra); a test asserts no read path emits a `LIMIT`-less query.)* [Frame D5; intake Success criteria 4 + 6; `P-SecurityLayered` — Security default-on, not opt-in]

- R-0020-d (scan-cost backstop — host-side `statement_timeout`, inside an explicit transaction): Because a `LIMIT` does not bound scan cost, a `filters` predicate on a **non-indexed** frontmatter field (outside the four R-0001-d expression-indexed fields `status` / `priority` / `project_id` / `parent_id`) matching sparsely or zero rows forces the keyset `ORDER BY id LIMIT cap+1` to walk the entire `id`-btree tail. The host read-path SHALL therefore open an **explicit Postgres transaction** (`BEGIN`) enclosing **both** a session **`SET LOCAL statement_timeout`** and the keyset query, so a query that would scan unbounded is **canceled** within a bounded wall-clock budget. The explicit transaction is **mandatory**: a `SET LOCAL` issued outside an explicit transaction binds only to the implicit single-statement transaction that commits immediately, so it is a **no-op** for the following query — a `SET LOCAL` outside an explicit transaction SHALL NOT be accepted as compliance (the backstop would otherwise ship **silently disarmed** while every cap test still passes). This backstop is **additive** to the R-0020-c cap — each runtime sub-layer is independently load-bearing (cap = result-set-size bound; timeout = scan-cost bound). On cancellation the dispatch SHALL surface **both**: (1) the **R-0004-a metric `outcome = "timeout"`** (not `"ok"`) with a bounded `duration_ms`; and (2) a distinguishable caller-facing JSON-RPC error code **`query_scan_timeout`** — **distinct from** `plugin_execution_timeout` (the R-0007-b guest epoch-deadline timeout, a guest-*execution* timeout) and **distinct from** `-32602` parameter-invalid (R-0010-f / R-0020-e, an input fault); error classes SHALL NOT be conflated (R-0010-f). The `query_scan_timeout` error body SHALL carry **no Postgres/schema/DB internals** (the same no-leak posture locked in R-0020-e, applied to a **distinct** error class). The shared `outcome = "timeout"` metric value with the epoch timeout is acceptable because the caller-facing error code is the discriminator; the metric stays shared. The Wasmtime epoch deadline (R-0007-b, 5 s) bounds *guest wasm execution*, **not** this host Postgres query (the host-fn query runs while the guest is blocked in the host call, where the epoch deadline cannot interrupt it) — the `statement_timeout` is the only bound on the host query and SHALL be set strictly below the R-0007-b 5 s guest deadline. **Locked value: `statement_timeout = 3000 ms`** (maintainer-ratified 2026-06-29). Rationale: a backstop *ceiling*, not a target; a healthy keyset page read of `<= cap+1` rows over the PK btree completes in single-digit ms (~100× headroom), so 3000 ms never false-trips a legitimate read; defensible band 1000–5000 ms, strictly below the R-0007-b 5 s guest deadline. *(Binary-observable, two independently-assertable parts: **(i) GUC placement** — the emitted `SET LOCAL statement_timeout` value is non-zero and strictly below the R-0007-b 5 s guest epoch deadline, directly assertable from the emitted value and read back as a non-zero `current_setting('statement_timeout')` inside the same explicit transaction; **(ii) cancellation fires** — a query exceeding the timeout is canceled and surfaces both `outcome = "timeout"` and the `query_scan_timeout` caller error. Because a `<= cap+1`-row PK-btree tail scan completes sub-millisecond and cannot reach the locked 3000 ms, part (ii) SHALL be validated with a **test-only reduced `statement_timeout`** — set deliberately low and seed enough rows to exceed it — NOT the production 3000 ms; the production value is exercised only for part (i) placement. The GUC touches no schema, index, filter semantics, or RLS.)* [Frame D8; `P-SecurityLayered` — the deterministic runtime **backstop** sub-layer]

- R-0020-e (cursor validation at the boundary + error semantics): The `cursor` SHALL be validated as a **well-formed 26-char ULID at the `artifact.list` host-fn boundary, before query construction** — this both fixes the error semantics and **caps the otherwise-unbounded `option<string>` input** (an arbitrary-length client string never reaches query construction). On a **malformed** cursor the host-fn SHALL return the **R-0010-f** structured **"parameter invalid"** error — **not** a silently-empty page, **not** a raw Postgres error echoed to the caller — and the error body SHALL carry **no Postgres/schema/DB internals** (an additive no-leak posture locked here; this is not literal R-0010-f text). A **valid-but-out-of-range** cursor (well-formed ULID past the end) SHALL return an **empty page** (`ids = []`, `has-more = false`, `next-cursor = none`), mechanically derivable from keyset `id > $cursor`. A **foreign-but-valid** ULID is safe: confined to the caller's workspace by `workspace_id = ctx.workspace_id` (R-0006-d), it can at most return an empty or shifted page within the caller's own workspace — never another tenant's rows. Layer note: the *validation* runs at the host-fn boundary (the R-0020-c clamp-site layer); the *JSON-RPC error code* is returned by the MCP handler up-stack (the R-0010-f layer) — linked but distinct layers. *(Binary-observable: malformed cursor → R-0010-f "parameter invalid" with no DB/schema string in the error body; out-of-range valid ULID → empty page; a cross-workspace probe under a foreign cursor returns zero foreign rows.)* [Frame D9; error class R-0010-f]

- R-0020-f (`limit = 0` ≠ unlimited): `limit = 0` SHALL resolve to **clamp-up to the default** (effective limit = 100) — every call returns progress. Independent of that resolution, `0` SHALL **never** disable the `LIMIT`: the read path SHALL NOT emit `LIMIT NULLIF($n, 0)`, a `LIMIT`-less query, or any construction where `0` yields an unbounded result set. **Locked value: clamp-up-to-default** (maintainer-ratified 2026-06-29). Rationale: the recorded Frame lean ("every call returns progress"); least-surprising for a caller passing `0` — a normal default page rather than a silently-empty page that reads as end-of-data; the no-unbounded invariant holds under either resolution. *(Binary-observable against the emitted SQL: `limit = 0` emits `LIMIT 101` (default 100 + 1) and returns `ids.len() <= 100`; it never emits `LIMIT NULL` / `LIMIT NULLIF`-zeroed / a `LIMIT`-less query. The non-monotonicity is intentional and not a bug: `limit = 0` → default 100, `limit = 1` → 1 — `0` is the "unspecified, give me a default page" sentinel, not "fewer than one row".)* [Frame D10; intake Success criterion 6; `P-SecurityLayered`]

- R-0020-g (paging params excluded from telemetry): `cursor` (in) and `next-cursor` (out) **are artifact IDs**; R-0004-a forbids artifact IDs in the per-verb metric record. The paging parameters SHALL be **excluded** from the per-verb metric record **and** from structured-log emission: the dispatch's metric/log carries the R-0004-a floor (`workspace_id` / `verb` / `outcome` / `duration_ms`) but never the `cursor` / `next-cursor` values. *(Binary-observable: the emitted per-verb metric record and the structured-log line for a paginated dispatch contain neither the `cursor` nor the `next-cursor` value.)* [Frame D11; R-0004-a, extended to the structured-log surface by the same data-minimization rationale]

*Illustrative WIT (not normative as code — concrete typed signatures are a plan-tier artifact):*

```wit
// shared by both directions — a WIT component type per R-0012-f
record artifact-page {
    ids: list<string>,
    has-more: bool,
    next-cursor: option<string>,
}

// import (host-fn) — gains limit + cursor, returns the record (R-0012-a/-f)
artifact-list: func(ctx: workspace-ctx, %type: string, filters: json,
                    limit: u32, cursor: option<string>) -> artifact-page;

// export (guest content interface) — same paging params, NO ctx, same record (R-0019-a)
%list: func(%type: string, filters: json,
            limit: u32, cursor: option<string>) -> artifact-page;
```

## Out of Scope

The following are explicitly outside the `0.1.0` substrate increment scope. An implementing developer SHALL NOT build these in the `0.1.0` increment.

- **RLS policy enforcement (Postgres `CREATE POLICY` statements)** — deferred to V0.1+ as additive `CREATE POLICY` additions per accepted risk `R-0001`; the column-shape ships, the policies do not.
- **HSM-backed or runtime-fetch signing key custody** — deferred to `{{P-SigningKeyCustodyHardening}}` (Tier-C ADR, not yet authored); activated by the multi-deployment trip-wire in [P-0005](../src/adrs/P-0005-v0-signing-chain.md).
- **Increment-to-plugin mapping ratification** — the assignment of `0.2.0`–`0.14.0` capability families to the four `core: true` plugins is proposed in [P-0002](../src/adrs/P-0002-core-plugin-partition.md) and flagged for maintainer ratification before the `0.2.0` dispatch; it is not a deliverable of this `0.1.0` spec.
- **Capability family implementations** (`0.2.0` task management through `0.14.0` content-corpus migration) — the substrate provides the ABI, storage, auth, and runtime; actual plugin code for capability families is out of scope for `0.1.0`.
- **Domain / non-CRUD verb export ABI and the dynamic-resolution mechanism** (per [P-0013](../src/adrs/P-0013-plugin-invocation-model.md)) — V0 ships the fixed typed `content` CRUD export interface only (R-0019). Domain / non-CRUD verbs (e.g., `echo.audit`) and the load-time dynamic verb→export resolution mechanism they would need (a runtime registry mapping a manifest verb-name to a typed export handle — a *different* mechanism from V0's static fixed-interface `wit_bindgen`) are deferred past V0; the deferred ABI will be typed too when designed (no string hatch). *(Named tripwire, `P-Defer`: the first domain / non-CRUD verb that must dispatch at the V0 surface — a required verb the fixed `content` CRUD interface cannot express — fires the design of the deferred export ABI and its dynamic-resolution mechanism. A preference for generality is not a firing condition.)*
- **Migration mechanics** (Tier-B ADRs: `{{P-MigrationID}}`, `{{P-FKPreservation}}`, `{{P-BackupRestore}}`, `{{P-CutoverDualWrite}}`) — deferred to migration-increment briefs; the migration handler builtin's execution logic is not in `0.1.0` scope.
- **Tier-C operational hardening** (`{{P-PostgresExtDeploy}}`, `{{P-MCPWriteSemantics}}`, `{{P-SigningKeyCustodyHardening}}`, `{{P-AdminCLIDiscipline}}`) — deferred per [P-0005](../src/adrs/P-0005-v0-signing-chain.md) `P-Defer` principle.
- **Cross-tier `{{P-ProjectionRebuild}}` detailed semantics** — cross-substrate rebuild ordering and refresh-queue dependency tracking is a follow-up Frame extension when the first cross-substrate projection lands.
- **Status-churn write-amplification numeric budget** — the C1 write-amplification weakness under high status-flip rates is known; a numeric model and potential C2-influenced partial evolution are deferred to V0.1+ when dogfood data accumulates.
- **R-0001-g forward-compat Acceptance Criteria expansion** — the AC for the three V0.1 promotion paths (full-text, graph-edge, multiple embedding columns) is flagged in [P-0001](../src/adrs/P-0001-storage-layout.md) as a follow-up; the `0.1.0` AC covers the single pgvector smoke test only.
- **Streamable-HTTP MCP transport** — V0.1+ activation conditional on the microVM-appliance trip-wire.
- **External authorization server integration (OIDC AS federation)** — V0.1+ scope per accepted risk `R-0002`; the static admin token is the V0 auth path.
- **Third-party plugin install** — V0.1+ scope per product brief idea tier D11; the runtime rejects non-`core` plugins at V0.
- **Hot-reload / admin-triggered plugin reload** — V0.1+ scope. Plugin instances are only created at host startup from the fixed `core: true` set. No post-startup plugin loading path exists at V0. Scenario 6 (signing verification failure) is complete for V0 scope: it covers the startup load path only.
- **Per-plugin scope extension** (beyond the binary admin/read-observer roles) — V0.1+ scope per [P-0009](../src/adrs/P-0009-rls-admin-token.md); fine-grained per-plugin scopes are not in V0.
- **Adaptive plugin pool sizing** — V0.1+ scope; V0 uses fixed pool size 3–5.
- **Saga coordination / cross-plugin atomicity** — not V0; the day-1 contract is "no cross-plugin atomicity, ever; design for partial failure."
- **UI behavior** — no user interface at V0 substrate level; UI is post-V0.
- **`get_context_for(artifact_id)` retrieval verb** — V0.1 scope (`1.1.0`), not substrate.
- **Ongoing ingest pipeline** (watchers, scheduled polls, webhooks) — V0.1 scope (`1.2.0`); `0.1.0` covers only the substrate plumbing, not continuous ingest.
- **Artifact-list pagination — out-of-scope mechanisms** (per R-0020 / the [paging Frame](../intent/artifact-list-paging-frame.md)) — the keyset (cursor) pagination added in R-0020 SHALL NOT introduce: **(1) offset / page-number pagination** (offset degrades on large append-only tables and is non-resumable across concurrent inserts; keyset is the locked mechanism per R-0020-a — this is the recorded Alternatives-Considered rejection, so no standalone P-ADR is warranted); **(2) a `COUNT` / total-count surface** (`has-more` is derived by fetch-one-extra per R-0020-a; no total is computed or returned); **(3) any schema or index change** (the keyset reuses the existing ULID-text PK index; R-0001-d's four expression indexes are unchanged and no new index is added); **(4) any change to `filters` semantics** (the keyset `id > $cursor` predicate ANDs with the existing `filters` predicate per R-0020-a; filter parsing/semantics are unchanged); and **(5) RLS policy activation** (the keyset SELECT continues to satisfy the existing R-0006-d application-layer `workspace_id` WHERE-clause — RLS policy objects remain deferred per the **RLS policy enforcement** item above and the accepted-risk `R-0001` trip-wire; this amendment does not fire it). *(Tripwires where these are deferrals, `P-Defer`: a future requirement for a total-count surface, an offset/random-access path, or a non-`id` page order would each re-open the pagination design; a preference for generality is not a firing condition.)*
- **Artifact-list ids-only return — richer-return revisit** (per R-0020-b; provisionally accepted by the maintainer 2026-06-29) — the `artifact.list` / `content.list` page record narrows to **ids-only** (no inline frontmatter); a caller resolves frontmatter via `artifact.get` per id (the accepted N+1 fan-out characterized on R-0020-b). Carrying frontmatter in the page record, adding a `fields` projection, or adding a frontmatter-only `get` are **out of scope for `0.1.0`** — the ids-only shape ships as-is and is validated by dogfood data. *(Named tripwire, `P-Defer`: the ids-only return is revisited — carry frontmatter in the record / add a `fields` projection / add a frontmatter-only get — **if the V0 dogfood instrumentation on R-0020-b shows browse-by-metadata-via-`list` (list-then-N+1-`get`-for-metadata) is a common access pattern**; a preference for a richer return, absent that access-pattern evidence, is not a firing condition. Tracking task #1919.)*

## Scenarios

### Scenario: Single-node V0 substrate initialization — happy path

**Given** a fresh Postgres instance with no mnemra-core schema, and the mnemra-core binary built with signed `core: true` plugin artifacts and embedded verification material  
**When** the operator runs `mnemra init`  
**Then** the system creates all substrate tables and indexes, installs the bundled `pgvector` extension against the embedded engine (no `timescaledb` — re-derived 2026-06-09 per [observability baseline](../src/architecture/overview.md#observability); no in-app observability hypertable, retention policy, or continuous aggregate is created), creates the `default` workspace, initializes all seven builtin components, initializes the plugin pool (3–5 instances per `core: true` plugin type), verifies plugin signatures synchronously before creating any instance, emits a structured health event to the OTel/stdout surface, and responds to a `GET /health` request on the loopback-only listener with the structured detail body `{ "postgres": true, "pgvector": true, "workspace_default": true, "overall": "ok" }` (loopback IS the gate at V0 — no admin token is required on the detail body; the listener binds `127.0.0.1` only, so every caller it serves is on loopback).

### Scenario: Plugin signing chain operates on load — happy path

**Given** a `tasks` plugin manifest at `tasks/manifest.toml` declaring `schema_version = 1`, `core = true`, the valid `[signature]` section signed by the mnemra root key on the build host  
**When** the host loads the `tasks` plugin at startup  
**Then** `verify()` is called synchronously and returns `Ok`; a per-instance host-fn allowlist is compiled from the manifest's `[host_fns]` section; a pool of 3–5 `tasks` plugin instances is created; each instance is ready to service `task.*` MCP verbs.

### Scenario: MCP verb dispatches under `WorkspaceCtx` — happy path

**Given** an MCP client connected over stdio presenting a valid admin token associated with `workspace_id = W1`  
**When** the client sends a JSON-RPC request for verb `task.create` with valid arguments  
**Then** `P-builtin-auth` validates the token; a `WorkspaceCtx { workspace_id: W1, role: Admin, token_id: T1 }` is constructed at a single construction site; the `WorkspaceCtx` is passed as the first parameter to the `artifact.create` host-fn; the host derives `workspace_id` from the context and performs `INSERT INTO tasks (...) WHERE workspace_id = W1`; the response carries the new artifact's ULID; a per-verb metric record is emitted to the OTel/stdout surface for verb `"task.create"`, outcome `"ok"`, and measured `duration_ms` (not written to an in-app hypertable — re-derived per [observability baseline](../src/architecture/overview.md#observability)).

### Scenario: MCP verb dispatches to a typed content export — happy path

**Given** a `tasks` plugin built as a component that exports the typed `content` interface (`create`/`get`/`list`/`update`/`delete`) and exports no `run`-shaped string-dispatch function, with `task.create` declared in its manifest `verbs` list, and an MCP client connected over stdio presenting a valid admin token for `workspace_id = W1`  
**When** the client sends a JSON-RPC request for verb `task.create` with valid arguments  
**Then** after `DF-auth-check` and the per-verb capability check against the manifest `verbs` list (R-0010-d), the host invokes the **typed `content.create` export** on a pooled `tasks` instance — resolved statically against the fixed `content` interface, with no runtime export-registry lookup (R-0019-a, R-0019-c) — passing the typed arguments across the WIT boundary and receiving a typed return; no string-based verb-dispatch path is traversed (R-0019-b); the response carries the new artifact's ULID. *(Companion: a manifest-declared verb with no matching typed export returns the R-0019-d structured non-dispatchable error and leaves the pre-dispatch permission outcome unchanged — covered by R-0019-e, not a separate scenario at V0 since no such verb ships.)*

### Scenario: Observability emits during a dogfood session — happy path

*(Re-derived 2026-06-09 per [observability baseline](../src/architecture/overview.md#observability): observability is emitted, not stored in-app. No metrics/events hypertable or continuous aggregate exists; the percentile distribution is derivable from the emitted `duration_ms` values at the operator-chosen sink.)*

**Given** the substrate is initialized (no in-app observability store; the server emits to stdout/OTel)  
**When** a typical dogfood session generates 50 MCP verb calls across `task.create`, `task.update`, `task.list`, `task.get` over 1 hour, with no persistent observability backend configured  
**Then** 50 per-verb metric records are observable on the OTel/stdout emission surface, each carrying `workspace_id`, `verb`, `outcome`, `duration_ms`; `p50`/`p95`/`p99` per `(verb, workspace_id, outcome)` are derivable from the emitted `duration_ms` values for the window; `\d+` introspection shows **no** metrics/events hypertable and `\dx` does **not** list `timescaledb`; emission succeeds with no observability database present (storage-independent emission, R-0004-c/f).

### Scenario: Admin token authenticates a destructive operation — happy path

**Given** the admin token file at mode 600 with `token_value = T_ADMIN`, and a `admin_tokens` row `{ token_hash: BLAKE3(T_ADMIN), workspace_id: W1, scopes: ["admin"] }`  
**When** the operator runs `mnemra workspace list --token T_ADMIN`  
**Then** the CLI performs a hash lookup `BLAKE3(T_ADMIN)` against `admin_tokens.token_hash`; the lookup returns the row; `WorkspaceCtx { workspace_id: W1, role: Admin, token_id: <row id> }` is constructed; `workspace list` executes and returns the list of workspaces scoped to `W1`.

### Scenario: Signing verification failure — plugin SHALL NOT load

**Given** a plugin artifact whose `[signature]` section contains bytes that do not verify against the embedded mnemra root key  
**When** the host attempts to load this plugin at startup  
**Then** `verify()` returns `Err`; the plugin load is rejected with a structured error naming the plugin's `name` and `version`; no plugin instance is created; the host continues startup (it does not crash); the MCP server does not expose any verbs for the rejected plugin; the error is emitted as a structured event (destination per R-0004).

### Scenario: Resource limit breach — kill-and-replace

**Given** a `tasks` plugin instance running verb `task.list` that enters an infinite loop (pathological implementation)  
**When** the epoch-interruption deadline (5 seconds) fires  
**Then** the Wasmtime `Store` traps with an epoch deadline error; the host catches the trap; a structured event is emitted (destination per R-0004): `{ event_type: "plugin_limit_violation", payload: { workspace_id: W1, plugin_id: "tasks", plugin_version: "0.2.0", limit_type: "epoch_deadline", limit_value: 500 } }`; the pool slot is poisoned; a new `tasks` instance is created for the pool; the caller receives a structured error `{ code: "plugin_execution_timeout", plugin: "tasks", verb: "task.list" }`; the host process does NOT panic.

### Scenario: Admin token mismatch — 401

**Given** an MCP client presenting a token string `T_BOGUS` for which no matching row exists in `admin_tokens`  
**When** the client sends a JSON-RPC request for any verb  
**Then** `P-builtin-auth` performs `SELECT ... WHERE token_hash = BLAKE3(T_BOGUS)`; the query returns zero rows; the MCP handler returns JSON-RPC error code distinguishable as authentication failure (NOT verb-not-found, NOT parameter-invalid); no `WorkspaceCtx` is constructed; no host-fn is invoked; no workspace data is accessed.

### Scenario: Cross-workspace SQL leak — compile-time ABI prevention (build-time test)

**Given** a `core: true` plugin author who attempts to declare a host-fn that accepts `workspace_id: Uuid` as a write-path parameter  
**When** the author compiles the plugin against the WIT-defined host-fn interface  
**Then** the WIT-generated bindings do not include any write-path function that accepts `workspace_id` as a parameter; the compiler rejects the attempt at the type boundary; no runtime check is necessary because the ABI makes the wrong thing impossible to express.

### Scenario: Read-observer token denied write access

**Given** an admin token row with `scopes = ["read_observer"]` associated with `workspace_id W1`  
**When** an MCP client bearing this token sends a `task.create` request  
**Then** the `WorkspaceCtx { workspace_id: W1, role: ReadObserver, token_id: T2 }` is constructed; the host-fn boundary check for `artifact.create` evaluates `Role::ReadObserver` against the permission matrix; the request is denied with a structured permission error; no artifact is written; no `workspace_id` exposure outside W1 occurs.

### Scenario: Build-host key-on-disk leak — trip-wire activation criterion

**Given** the mnemra-core binary is deployed to a second deployment node beyond the maintainer's single dogfood instance  
**When** this deployment event is detected (by the operator, by the CI system, or by any monitoring mechanism)  
**Then** the multi-deployment trip-wire fires; `{{P-SigningKeyCustodyHardening}}` (Tier-C ADR) MUST be authored and locked before the second deployment proceeds; the accepted risk `R-0004` is NOT considered retired until `{{P-SigningKeyCustodyHardening}}` locks.

### Scenario: Admin CLI — schema-driven subcommand generation

**Given** a `tasks` plugin manifest that declares a verb `task.audit` in its `[verbs]` section, and the host has loaded this manifest at startup  
**When** the operator invokes `mnemra admin`  
**Then** a `task.audit` subcommand is available in the admin CLI reflecting the manifest's declared verbs; the subcommand was not hard-coded in the CLI binary; removing the verb from the manifest and restarting removes the subcommand.

### Scenario: `/health` degraded and down states

**Given** the mnemra-core host is running with the loopback TCP listener active  
**When** the Postgres database is unreachable (connection refused) and `GET /health` is invoked on the loopback listener  
**Then** the response body includes `{ "postgres": false, "overall": "degraded" }` if any dependency is degraded but the host is running, or `{ "overall": "down" }` if Postgres is fully unreachable and no structured detail is available; the HTTP status code is `503 Service Unavailable`.  
**And given** the host is mid-shutdown (SIGTERM received, graceful drain in progress)  
**When** `GET /health` is invoked  
**Then** the response reflects `{ "overall": "down" }` with HTTP `503`.

### Scenario: Token rotation event ordering

**Given** a valid admin token `T_OLD` with `workspace_id W1`, and a request to rotate it  
**When** `mnemra token rotate --token T_OLD` is executed  
**Then** the rotation event (type `token_rotated`, carrying `token_id` of `T_OLD`) is emitted (destination per R-0004) BEFORE the old token row is deleted from `admin_tokens`; the new token row is created; subsequent lookups against `T_OLD`'s hash return zero rows; subsequent lookups against the new token succeed.

### Scenario: Plugin fuel exhaustion mid-verb — kill-and-replace (independent of epoch breach)

**Given** a `tasks` plugin instance running verb `task.list` that consumes 10B+ fuel ticks before the epoch deadline (a pathological module constructed to consume CPU instructions without sleeping, so the epoch deadline does not fire first)  
**When** the fuel ceiling is hit  
**Then** the Wasmtime `Store` traps with a fuel-exhaustion error; the same kill-and-replace path fires as in Scenario 7 (pool slot poisoned, new instance created, caller receives structured error); the structured event uses `limit_type: "fuel"` and `limit_value: 10000000000`; the pool recovers; the host does NOT panic. Note: this scenario is tested independently of Scenario 7 — the pathological module must consume fuel without triggering the epoch deadline, confirming the fuel-ceiling path is exercised separately.

### Scenario: Artifact-list keyset pagination walks a type end-to-end — happy path

**Given** a workspace `W1` holding `N` artifacts of type `task` (with `N` greater than one page), and an MCP client connected over stdio presenting a valid admin token for `W1`  
**When** the client walks the type starting from `cursor = none`, following `next-cursor` on each call until `has-more = false` — through the full path client → guest `content.list` export → host-fn `artifact.list` → Postgres  
**Then** a finite sequence of `artifact-page` records is returned in `id`-ascending (creation) order such that: on **every** page `has-more = true` **iff** `next-cursor = some` (R-0020-b); on **every non-final page** `next-cursor = some(ids.last())` — the resume cursor equals the `id` of the last row on the page (R-0020-b); on the **final** page `has-more = false` **and** `next-cursor = none`; the concatenation of all pages' `ids` contains each of the `N` artifacts **exactly once** (no duplicates, no gaps); global order is `id`-ascending across page boundaries; the walk **terminates** (no infinite cursor loop); and the walk is exercised **through the guest `content.list` export**, not only the host-fn (the end-to-end path is the asserted surface). [Frame QA-1]

### Scenario: Artifact-list pagination DoS bounds — size cap and scan-cost timeout

**Given** the keyset read path with the host-side clamp (R-0020-c) and host-side `statement_timeout` (R-0020-d) active  
**When (A — size)** a caller requests `limit = 100_000` against a type holding more than 500 artifacts  
**Then (A)** a single bounded `artifact-page` is returned with `ids.len() <= 500` (clamp) even though the requested `limit` exceeded the cap; rows **returned** are `<= cap + 1` (fetch-one-extra; no `COUNT`); and no list path issues an unbounded (`LIMIT`-less) query — closing the result-set-*size* half of the prior unbounded-`SELECT` finding.  
**And when (B — scan cost, adversarial)** a caller issues a `filters` predicate on a **non-indexed** frontmatter field (outside the four R-0001-d expression-indexed fields) matching sparsely or zero rows, against a type holding more than `cap` artifacts — forcing `ORDER BY id LIMIT cap+1` to walk the entire `id`-btree tail  
**Then (B)** the query is **canceled by the `statement_timeout`** within the bounded budget and surfaces as a structured error — observable as the **R-0004-a metric `outcome = "timeout"`** (not `"ok"`) with a bounded wall-clock `duration_ms` — rather than completing an unbounded full-tail scan, closing the *scan-cost* half the cap leaves open. [Frame QA-2]

### Scenario: Artifact-list tenant isolation preserved across the keyset rewrite

**Given** a paginated `list` issued under workspace `A` while workspace `B` holds artifacts of the same type, with the `workspace_id = ctx.workspace_id` WHERE-clause condition retained on the keyset SELECT (R-0006-d) and the existing R-0018-d read-path lint in CI  
**When** workspace `A` paginates the type across all pages, including under a cursor value that `B`'s rows would sort into  
**Then** only workspace-`A` artifacts are ever returned, on any page, for any cursor; the existing **R-0006-d / R-0018-d read-path CI lint stays green** across the rewrite (the `workspace_id` WHERE-clause condition is present on the new keyset read path — no new mechanism is introduced); and a **cross-workspace probe** returns **zero** `B`-owned `id`s on any page. [Frame QA-3]

### Scenario: Artifact-list breaking return-type change — recompile-all-core-plugins before merge

**Given** the `artifact.list` / `content.list` return type changes from `list<string>` / `[{ id, frontmatter }]` to the `artifact-page` record — a breaking change within the `0.y.z` band  
**When** the R-0020 ABI-change PR is presented at the pre-merge gate with all `core: true` content plugins in the tree  
**Then** the breaking change lands only with all consumers migrated and green: every `core: true` plugin recompiles against the new ABI **and** passes its tests **before** the change merges (R-0017-a, asserted at the merge gate, not after); the changed host-fns carry the correct stability annotation reflecting the breaking change (R-0012-e); and the trap/replace plugin path pinned to the current export shape per P-0013 is re-pinned to the new return shape with its kill-and-replace tests staying green. [Frame QA-4]

### Scenario: Artifact-list cursor and `limit` boundary semantics

**Given** the `artifact.list` host-fn boundary validation (R-0020-e) ahead of query construction and the host-side clamp (R-0020-c/-f)  
**When** a caller supplies (a) a malformed `cursor` (not a 26-char ULID), (b) a well-formed but out-of-range `cursor` (valid ULID past the end), and (c) `limit = 0`  
**Then** each boundary input yields a bounded, defined outcome — never an unbounded query and never a leaked internal error: **(a)** the malformed `cursor` returns the **R-0010-f structured "parameter invalid" error** (not an empty page, not a raw Postgres error), and the error body contains **no DB/schema/Postgres internals** (R-0020-e no-leak posture); **(b)** the out-of-range valid-ULID `cursor` returns an **empty page** (`ids = []`, `has-more = false`, `next-cursor = none`), derivable from keyset `id > $cursor`; **(c)** `limit = 0` returns a **bounded** default-clamped page (`ids.len() <= 100`) and the emitted SQL is `LIMIT 101` — **never** a `LIMIT`-less or `LIMIT NULLIF`-zeroed query (R-0020-f), asserted against the actual emitted SQL. [Frame QA-5]

### Scenario: Artifact-list paging params excluded from telemetry

**Given** any paginated `artifact.list` / `content.list` dispatch carrying a `cursor` in and a `next-cursor` out, with the per-verb metric record (R-0004-a) and structured-log emission on the dispatch  
**When** the dispatch completes and emits its telemetry  
**Then** the emitted per-verb metric record contains `workspace_id` / `verb` / `outcome` / `duration_ms` and **does not** contain the `cursor` or `next-cursor` value (R-0004-a: no artifact IDs in the metric record), and the structured-log line for the dispatch likewise **omits** the `cursor` / `next-cursor` values — assertable by inspecting the emitted record/log (R-0020-g). [Frame QA-6]

### Scenario: Artifact-list empty type — zero artifacts

**Given** a workspace `W1` holding **zero** artifacts of type `task`  
**When** a client calls `artifact.list` for type `task` with `cursor = none`  
**Then** a single `artifact-page` is returned with `ids = []`, `has-more = false`, and `next-cursor = none` — the empty-set case satisfies the R-0020-b biconditional (`has-more = false` ∧ `next-cursor = none`); no error is raised and no `cap+1`-th probe row is implied. [R-0020-b]

### Scenario: Artifact-list exact-last-page boundary — final page exactly full

**Given** a workspace `W1` holding **exactly** `page_size × K` artifacts of type `task` (e.g., 200 at the default page size of 100), and a client walking from `cursor = none`  
**When** the client walks to the last page  
**Then** page 1 returns `ids.len() = 100` with `has-more = true` and `next-cursor = some(ids.last())`, and the 2nd (**final**) page returns `ids.len() = 100` with `has-more = false` and `next-cursor = none` — the final page is **exactly full** yet correctly reports end-of-set. This is the highest-value paging edge: it catches an `ids.len() == limit` end-of-set heuristic (which would wrongly report `has-more = true` on an exactly-full final page) in place of the R-0020-a **fetch-one-extra** mechanism. [R-0020-a, R-0020-b]

### Scenario: Artifact-list page-size cap boundary — limit 499 / 500 / 501

**Given** a workspace holding more than 501 artifacts of a type, and the host-side clamp `effective_limit = (limit == 0 ? 100 : min(limit, 500))` (R-0020-c)  
**When** a client requests `limit = 499`, then `limit = 500`, then `limit = 501`  
**Then** the effective page size is **499, 500, and 500** respectively (`min(limit, 500)`): `limit = 499` returns up to 499 ids; `limit = 500` returns up to 500 ids; `limit = 501` is **clamped** to 500 ids — every page returns `ids.len() <= 500`, and the emitted SQL `LIMIT` is `effective_limit + 1` (500 / 501 / 501) per fetch-one-extra. [R-0020-c]

### Scenario: Artifact-list foreign-but-valid cursor — zero foreign rows

**Given** workspaces `A` and `B` each holding artifacts of the same type, and a **well-formed 26-char ULID** cursor that is the `id` of one of `B`'s artifacts  
**When** workspace `A` calls `artifact.list` passing `B`'s artifact `id` as `cursor`  
**Then** the keyset `id > $cursor` predicate is ANDed with `workspace_id = ctx.workspace_id` (R-0006-d), so the returned page contains only `A`-owned ids whose `id` sorts after the cursor value — **never** any `B`-owned row; the foreign-but-valid cursor is safe (it can at most return an empty or shifted page **within `A`'s own workspace**), and the cross-workspace probe returns **zero** `B`-owned ids on any page. [R-0020-e]

### Scenario: Artifact-list concurrent insert during walk — forward-non-losing keyset

**Given** a client mid-walk of type `task` under `cursor = C`, having already emitted all ids `<= C`  
**When** new artifacts of type `task` are inserted concurrently (each created after the cursor's row, so each sorts at `id > C` under the lexicographic = chronological `id` ordering)  
**Then** the newly-inserted `id > C` rows appear on a **later** page of the same walk (the walk is **forward-non-losing**: it neither loses already-emitted rows nor duplicates them); the walk is **not a point-in-time snapshot** — it MAY include rows created after the walk began, which is the **defined** keyset behavior, not a defect. [R-0020-a]

### Scenario: Artifact-list scan-cost statement_timeout — caller-visible query_scan_timeout

**Given** the keyset read path with the host-side `statement_timeout` (R-0020-d) armed **inside an explicit transaction**, exercised with a **test-only reduced `statement_timeout`** and a seeded row count large enough to exceed it under an adversarial sparse non-indexed `filters` predicate  
**When** the read-path query exceeds the `statement_timeout` and Postgres cancels it  
**Then** the dispatch surfaces **both** the R-0004-a metric `outcome = "timeout"` (not `"ok"`) with a bounded `duration_ms` **and** a caller-facing JSON-RPC **`query_scan_timeout`** error; the error body carries **no** Postgres/schema/DB internals (R-0020-e no-leak posture); and `query_scan_timeout` is **distinct** from `plugin_execution_timeout` (guest epoch) and `-32602` (parameter invalid). This scenario guards **both** the explicit-transaction requirement (a `SET LOCAL` outside a transaction would silently disarm the backstop and this cancellation would never fire) **and** the distinct-error-code requirement. [R-0020-d]

### Scenario: Artifact-list degraded DB connectivity mid-walk — structured error, resumable

**Given** a client mid-walk of a type, having received `next-cursor = C` on the prior page  
**When** DB connectivity degrades (connection refused / dropped) on the next `artifact.list` call  
**Then** the dispatch returns a **structured error** (not a crash, panic, or hang) — the host process does not panic; once connectivity recovers, the client retries the **same call with `cursor = C`** and the walk **resumes from exactly where it left off** (keyset cursors are stateless and idempotent — there is no server-side walk state to lose). [R-0020-a]

### Scenario: Artifact-list concurrent cursor churn under pool pressure

**Given** multiple clients concurrently walking the same and different types, with the fixed 3–5-instance plugin pool (R-0016-a) under contention  
**When** the concurrent walks proceed through the guest export → host-fn → Postgres path under pool pressure  
**Then** **every** walk returns a correct, complete, duplicate-free page sequence (concurrency does not corrupt any individual walk — cursors are per-call and stateless per R-0020-a/-b), and the pool-slot wait latency under contention is **characterized** for the dogfood instrumentation (R-0016-a; the fan-out / pool-pressure validation referenced in R-0020-b). [R-0016-a, R-0020-a]

### Scenario: Artifact-list malformed cursor — invalid base32 alphabet

**Given** the `artifact.list` host-fn boundary validation (R-0020-e), which validates `cursor` as a well-formed 26-char ULID (Crockford base32 alphabet + range) **before** query construction  
**When** a client supplies a 26-character `cursor` that is the correct **length** but contains characters **outside** the Crockford base32 alphabet (e.g., `I`, `L`, `O`, `U`, or other invalid symbols)  
**Then** the host-fn rejects it with the **R-0010-f structured "parameter invalid"** error before any query is constructed — the validation is **alphabet-and-range, not a naive length check** (a 26-char wrong-alphabet string SHALL NOT pass); the error body carries **no** DB/schema/Postgres internals (R-0020-e no-leak posture). [R-0020-e]

## Data Model

### Content substrate — per-artifact-type tables (C1 single-document layout)

**Entity: artifact table (generalized pattern — each plugin owns its concrete table)**

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| `id` | `TEXT` | `PRIMARY KEY` | ULID-from-source-id |
| `workspace_id` | `UUID` | `NOT NULL`, indexed | Tenant scoping key; RLS column-shape at V0. UUID enforced at Postgres layer. See P-0001 amendment 2026-05-24. |
| `type` | `TEXT` | `NOT NULL` | Artifact type constant for this table |
| `frontmatter` | `JSONB` | `NOT NULL`, `CHECK (frontmatter ? 'id')`, `CHECK (frontmatter ? 'frontmatter_version')` | Source frontmatter stored literally; queryable structured fields |
| `body` | `TEXT` | nullable | Narrative content |
| `frontmatter_version` | `BIGINT` | `GENERATED ALWAYS AS ((frontmatter->>'frontmatter_version')::bigint) STORED` | No-drift projection; authoritative value lives in the `frontmatter` JSONB (interchange format is self-describing) |
| `migrated_from` | `TEXT` | nullable | System field; not in frontmatter JSONB |
| `migrated_at` | `TIMESTAMPTZ` | nullable | System field; not in frontmatter JSONB |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL DEFAULT now()` | |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL DEFAULT now()` | |

Expression indexes declared on: `(frontmatter->>'status')`, `(frontmatter->>'priority')`, `(frontmatter->>'project_id')`, `(frontmatter->>'parent_id')`.

**V0.1+ non-breaking column additions (not in `0.1.0`):** `embedding vector(1536)` (pgvector), `search_tsv tsvector GENERATED ALWAYS AS (...) STORED`.

**Entity: artifact history table (shadow table per artifact table)**

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| `id` | `BIGSERIAL` | `PRIMARY KEY` | |
| `artifact_id` | `TEXT` | `NOT NULL` | FK to artifact table's `id` |
| `workspace_id` | `UUID` | `NOT NULL` | Denormalized for isolation. UUID per P-0001 amendment 2026-05-24. |
| `operation` | `TEXT` | `NOT NULL` | `'UPDATE'` only (INSERT is the artifact row; DELETE is separate) |
| `old_frontmatter` | `JSONB` | `NOT NULL` | Prior row frontmatter value, byte-for-byte |
| `old_body` | `TEXT` | nullable | Prior row body value |
| `changed_at` | `TIMESTAMPTZ` | `NOT NULL DEFAULT now()` | |

Populated by trigger on UPDATE; projections do not read from history tables.

**Entity: admin_tokens**

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| `id` | `UUID` | `PRIMARY KEY` | token_id; used in WorkspaceCtx for write attribution |
| `token_hash` | `BYTEA` | `NOT NULL UNIQUE` | `BLAKE3(token_bytes)`; authenticated by unique-hash lookup |
| `workspace_id` | `UUID` | `NOT NULL` | NOT NULL enforced; absence is a schema violation |
| `scopes` | `TEXT[]` | `NOT NULL` | Valid values: `["admin"]`, `["read_observer"]` |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL` | |
| `rotated_at` | `TIMESTAMPTZ` | nullable | Set on rotation; cleared on regeneration |

**Emission record: per-verb metric (OTel/stdout — NOT an in-app table at V0)**

*Re-derived 2026-06-09 per [observability baseline](../src/architecture/overview.md#observability): this is an emission shape, not a stored hypertable. The server emits these records to the OTel/stdout surface; where they land is the operator's choice behind the generation⊥storage separation. No in-app metrics hypertable, partition key, retention policy, or continuous aggregate exists at V0.*

| Field | Type | Notes |
|-------|------|-------|
| `workspace_id` | `UUID` | Tenant dimension; present on every metric record |
| `verb` | `TEXT` | e.g., `"task.list"`, `"task.create"` |
| `outcome` | `TEXT` | `"ok"` or `"error"` or `"timeout"` |
| `duration_ms` | `INT` | Wall-clock milliseconds (p50/p95/p99 derivable at the sink) |
| `recorded_at` | `TIMESTAMPTZ` | Emission timestamp |

**Emission record: event (OTel/structured — NOT an in-app table at V0)**

| Field | Type | Notes |
|-------|------|-------|
| `workspace_id` | `UUID` | Tenant dimension |
| `event_type` | `TEXT` | Versioned; registry at Spec implementation stage |
| `event_version` | `INT` | DEFAULT 1; bumped on breaking event-type schema changes |
| `token_id` | `UUID` | nullable; per-token write attribution; derived by host from `WorkspaceCtx.token_id`; NOT plugin-supplied. NULL only for system-generated events with no token context. |
| `agent_id` | `UUID` | nullable; not all events are agent-originated |
| `session_id` | `UUID` | nullable |
| `payload` | `JSON` | Typed per `event_type`; never contains artifact bodies |
| `recorded_at` | `TIMESTAMPTZ` | Emission timestamp |

**Entity: WorkspaceCtx (runtime struct — Rust)**

| Field | Type | Notes |
|-------|------|-------|
| `workspace_id` | `Uuid` | From `admin_tokens.workspace_id`; NOT NULL |
| `role` | `Role` | Derived from `admin_tokens.scopes`; `Role::Admin` or `Role::ReadObserver` |
| `token_id` | `Uuid` | For per-token write attribution; maps to `admin_tokens.id` |

## API Contract

### Pagination return record (`artifact-page`)

*(Added 2026-06-29 per R-0020.)* The list direction returns a shared WIT record on both the host-fn import (`artifact.list`) and the guest export (`content.list`). It replaces the prior `list<string>` / `[{ id, frontmatter }]` list return — the return **narrows to ids-only** (no inline frontmatter; a caller resolves frontmatter via `artifact.get` per id, an accepted N+1 fan-out characterized on R-0020-b). The ids-only record is the shape locked by **intake Success criterion 1** (host-fn) and **Success criterion 2** (guest export); Frame D4 understated the import baseline (it modeled the prior return as `list<string>`, but the host-fn baseline was `[{ id, frontmatter }]` — so the import side genuinely drops inline frontmatter).

| Field | WIT type | Notes |
|-------|----------|-------|
| `ids` | `list<string>` | The page's artifact ULIDs, in `id`-ascending (creation) order |
| `has-more` | `bool` | `true` iff a further page exists; `true` **iff** `next-cursor = some` (R-0020-b invariant) |
| `next-cursor` | `option<string>` | When `some`: the `id` of the last returned row (the next call's `cursor`). `none` when `has-more = false` |

The paging input parameters added to both directions are `limit: u32` and `cursor: option<string>` (typed WIT params per R-0012-f / R-0019-b — not folded into `filters`). Host-side clamp: `effective_limit = (limit == 0 ? 100 : min(limit, 500))` (R-0020-c/-f).

### Host-fn ABI

The plugin-facing host-fn ABI. All host-fns take `WorkspaceCtx` as the first parameter (not shown in each signature below; it is structurally required). `workspace_id` does not appear as an explicit parameter on write paths.

**Required host-fns (all `core: true` plugins):**

| Function | Parameters | Return | Notes |
|----------|------------|--------|-------|
| `artifact.create` | `type: str, frontmatter: JSON, body: str?` | `id: str` | ULID assigned by host |
| `artifact.update` | `id: str, frontmatter_patch: JSON, body: str?` | `()` | Patch, not replace |
| `artifact.get` | `id: str` | `(frontmatter: JSON, body: str?)` | Scoped to `ctx.workspace_id` |
| `artifact.list` | `type: str, filters: JSON, limit: u32, cursor: str?` | `artifact-page { ids: [str], has-more: bool, next-cursor: str? }` | Keyset (cursor) pagination per R-0020. WHERE-clause mandatory on workspace; host-side clamp default 100 / cap 500; `statement_timeout` scan-cost backstop. *(Amended 2026-06-29 (R-0020): paging params + `artifact-page` return replace the prior `[{ id, frontmatter }]` shape — return narrows to ids-only; resolve frontmatter via `artifact.get`.)* |
| `metrics.record` | `verb: str, duration_ms: u64, outcome: str` | `()` | Emits a per-verb metric record (OTel/stdout); not an in-app table write |
| `log.emit` | `level: str, message: str, context: JSON?` | `()` | Emits a structured log record to stdout |
| `event.emit` | `event_type: str, event_version: u16, payload: JSON` | `()` | Emits a structured event record (OTel/stdout); not an in-app table write |
| `projection.emit` | `projection_name: str, data: JSON` | `()` | workspace_id from ctx |

**Opt-in host-fns (declared in `host_fns.required` or `host_fns.optional` as appropriate):**

| Function | Parameters | Return | Notes |
|----------|------------|--------|-------|
| `artifact.delete` | `id: str` | `()` | Must be explicitly declared; destructive |
| `sampling.request` | `prompt: str, context_ids: [str]` | `completion: str` | Content IDs only in prompt args, not bodies |
| `secrets.get` | `key: str` | `value: str` | Read-only; no write path |

### Plugin export / invocation ABI

The plugin-side export ABI the host invokes per authenticated verb (the **export** direction; the Host-fn ABI above is the **import** direction). Per [P-0013](../src/adrs/P-0013-plugin-invocation-model.md) and R-0019: a fixed typed `content` interface every content plugin exports (at V0 the substrate loads only `core: true` plugins per R-0005-g); resolved statically (plain `wit_bindgen`, no runtime registry). The retired `run(input: string) -> string` export is shown for contrast only — it SHALL NOT exist on the V0 surface.

| Export | Parameters | Return | Notes |
|--------|------------|--------|-------|
| `content.create` | `type: str, frontmatter: JSON, body: str?` | `id: str` | Host invokes per authenticated `*.create` verb |
| `content.get` | `id: str` | `option<...>` | Single artifact by id; `option` for not-found |
| `content.list` | `type: str, filters: JSON, limit: u32, cursor: str?` | `artifact-page` | Keyset pagination per R-0020. Paging params typed (no `ctx` — the host carries `WorkspaceCtx` across the export boundary); workspace-scoped via `WorkspaceCtx`. *(Amended 2026-06-29 (R-0019-a / R-0020): paging params + `artifact-page` return replace the prior `list<...>` shape.)* |
| `content.update` | `id: str, frontmatter_patch: JSON, body: str?` | `()` | Patch, not replace |
| `content.delete` | `id: str` | `()` | Destructive; gated per manifest |
| ~~`run`~~ | ~~`input: str`~~ | ~~`str`~~ | RETIRED (R-0019-a) — string-dispatch export; no string-based verb resolution at V0 |

*(Concrete typed WIT signatures for the `content` interface are a plan-tier artifact, authored with the implementation; this contract pins the interface shape and the export direction. The MCP-verb → `content`-method mapping rule is not yet pinned and is a forthcoming plan/implementation concern, not fixed by this spec — R-0019-c.)*

### Plugin manifest TOML schema (schema_version: 1)

Documented in [P-0003](../src/adrs/P-0003-plugin-manifest.md). The `[signature]` section is populated at build time by the signing chain. The runtime verifies the signature synchronously at load time.

### MCP transport

**Protocol:** MCP specification 2025-06-18  
**Transport at V0:** stdio  
**Verb namespace pattern:** `"<plugin>.<verb>"` (e.g., `"task.create"`)  
**Authentication:** Bearer token in the MCP handshake; verified on every request  
**Error codes:**  

| Error class | JSON-RPC error code | Notes |
|-------------|---------------------|-------|
| Authentication failure | Distinguished code (not -32600 / -32601 / -32602) | Not conflated with parse/method/params errors |
| Verb not found | `-32601` (method not found) | MCP-standard |
| Parameter invalid | `-32602` (invalid params) | MCP-standard |
| Plugin execution timeout (`plugin_execution_timeout`) | Custom code | Returned on epoch-deadline limit breach |
| Plugin resource exhausted (`plugin_resource_exhausted`) | Custom code | Returned on fuel-exhaustion limit breach (distinct from timeout: a compute-budget breach, not a wall-clock timeout) |
| Query scan timeout (`query_scan_timeout`) | Custom code | Returned on host read-path Postgres `statement_timeout` cancellation (R-0020-d) — a **DB scan-cost** timeout; **distinct from** `plugin_execution_timeout` (the guest epoch-deadline timeout) and `-32602` (parameter invalid); error body carries no DB/schema internals (R-0020-e no-leak posture, applied to a distinct class) |
| Permission denied | Custom code | Returned on role check failure |

*(Amended 2026-06-19: §434 split the single plugin-execution error code into distinct epoch (`plugin_execution_timeout`) and fuel (`plugin_resource_exhausted`) caller codes; maintainer-approved, ratifies the Task-22 trap-recovery behavior.)*
*(Amended 2026-06-29 per R-0020-d: added the `query_scan_timeout` row — the host read-path `statement_timeout` cancellation gets a caller-facing code distinct from the guest-execution `plugin_execution_timeout` and from `-32602` parameter-invalid, per R-0010-f no-conflation. Triple-convergent Stage-3 review finding.)*

### Health endpoint

**Path:** `/health`  
**Methods:** GET  
**Transport:** Dedicated loopback-only TCP listener; binds to `127.0.0.1` only; port configurable via `MNEMRA_HEALTH_PORT` env var (default: `8877`). This is NOT the MCP stdio transport.  
**Gate:** The loopback-only bind IS the gate at V0 — every caller that can reach the listener is on loopback, so the detail body is served to every caller the listener can serve; there is no admin-token gating at V0. *(Named tripwire: if the listener ever binds a non-loopback interface, admin-token gating on the detail body becomes required — unauthenticated callers then receive a status-code-only `200`/`503` and only admin-token callers receive the detail body.)*  
**Response (HTTP `200` healthy / `503` when a dependency is down):**

```json
{
  "postgres": true,
  "pgvector": true,
  "workspace_default": true,
  "overall": "ok"
}
```

`"overall"` values: `"ok"` (all deps reachable), `"degraded"` (partial), `"down"` (Postgres unreachable). *(Re-derived 2026-06-09 per [observability baseline](../src/architecture/overview.md#observability): the `timescaledb` field is removed — TimescaleDB is not a V0 substrate dependency (D8). The body reports the substrate dependencies the standalone binary owns. The health endpoint is the first API, started before config load — R-0004-g.)*

## Constraints

- Must use PostgreSQL as the sole substrate, behind an engine-agnostic swappable `Storage` trait (Postgres the only implementation), via the **embedded Postgres engine** with bundled `pgvector` (per [P-0010](../src/adrs/P-0010-storage-substrate-engine.md)). *(Amended 2026-06-08: TimescaleDB is no longer a V0 substrate extension — D8 demotes it off the V0 stack. Re-derived 2026-06-09 per [observability baseline](../src/architecture/overview.md#observability): observability is emitted (stdout/OTel), not stored in-app; there is no in-app observability hypertable at V0. The observability storage backend is deferred behind the generation⊥storage separation — see R-0004, R-0013-c.)*
- Must use Wasmtime for plugin execution; no alternative WASM runtime.
- Must use MCP specification 2025-06-18; no other agent-facing protocol.
- Rust-default toolchain; non-Rust paths require justification.
- License: Apache-2.0 with future-relicense clause.
- No compile-time asset embedding (project default G-0002); assets served at runtime.
- No new dependencies without license-tier review (Green/Yellow/Red tier model).
- Architecture MUST NOT be schedule-pressured; marketing-tier dates are not architectural inputs.
- **Deployment pre-condition (amended 2026-06-08 per [P-0010](../src/adrs/P-0010-storage-substrate-engine.md)):** The V0 engine is **embedded Postgres** (`postgresql_embedded`) shipping with the single self-hosted binary, with **`pgvector` bundled/compiled** (`pgvector_compiled`). There is NO operator-provisioned external Postgres server and NO OS-package-manager extension-install step at V0 — `mnemra init` runs `CREATE EXTENSION IF NOT EXISTS pgvector` against the embedded engine's bundled extension. TimescaleDB is not a V0 extension (D8); observability is emitted (stdout/OTel), not stored in-app, with the storage backend deferred behind the generation⊥storage separation (re-derived 2026-06-09 per [observability baseline](../src/architecture/overview.md#observability); see R-0004, R-0013-c).

---

## Verify Contract

Fields `/verify` consumes from this spec:

- **requirements** — the RFC-2119 requirement list (R-IDs R-0001 through R-0020), including SHALL NOT / MUST NOT prohibitions
- **scenarios** — the Given/When/Then set (31 scenarios: 7 happy path + 17 edge case + 7 failure path). Edge cases (17): cross-workspace ABI prevention (build-time test), read-observer role denial, build-host key-on-disk trip-wire, admin CLI schema-driven generation, token rotation event ordering, artifact-list pagination DoS bounds (size cap + scan-cost timeout), artifact-list tenant isolation under pagination, artifact-list breaking return-type ABI evolution, artifact-list cursor/`limit` boundary semantics, artifact-list paging-param telemetry exclusion, artifact-list empty type (N=0), artifact-list exact-last-page boundary, artifact-list page-size cap boundary (limit 499/500/501), artifact-list foreign-but-valid cursor, artifact-list concurrent-insert-during-walk (forward-non-losing keyset), artifact-list concurrent cursor churn under pool pressure, artifact-list malformed cursor (invalid base32 alphabet). Failure paths (7): signing verification failure, resource limit epoch breach, admin token mismatch 401, `/health` degraded and down states, plugin fuel exhaustion breach, artifact-list scan-cost `statement_timeout` cancellation (caller-visible `query_scan_timeout`), artifact-list degraded-DB-connectivity mid-walk (structured error, resumable). *(The artifact-list keyset-pagination end-to-end walk is the +1 happy-path scenario added 2026-06-29 per R-0020; the 2026-06-29 Stage-3 review fold added 9 scenarios — 7 edge + 2 failure — and corrected the failure-path membership: the prior enumeration named a non-existent "epoch-tick thread death" scenario and omitted the present `/health` degraded/down scenario.)*
- **out-of-scope** — the explicit prohibition boundary (22 items)
- **data_model** — schema tables (artifact rows, admin_tokens, events, metrics) for `/verify` to validate against the running system *(the R-0020 `artifact-page` is a WIT interface return record, carried under api_contract — not a stored table)*
- **api_contract** — host-fn ABI signatures (import direction, including the R-0020 keyset-pagination `artifact.list` signature + `artifact-page` return), the typed `content` export / invocation ABI (export direction, R-0019, including the R-0020 `content.list` paging signature), and MCP transport shape

**White-box AC residual (RED-phase planning note, added 2026-06-29):** R-0020-a, R-0020-c, and R-0020-f carry **white-box** acceptance criteria that assert the *emitted SQL structure* — "the query emits `ORDER BY id` and `LIMIT $effective_limit + 1`" (R-0020-a), "no read path emits a `LIMIT`-less query" (R-0020-c), and "`limit = 0` emits `LIMIT 101`, never `LIMIT NULL` / `LIMIT NULLIF`-zeroed" (R-0020-f). These are **not reachable by a black-box test** of the page contract — they require a **SQL-observation seam** (a query-log capture or a `#[cfg(test)]`-gated SQL-emission hook). The RED phase SHALL provision that seam; the SQL-structure assertions are owned at **code-level review** (the security/correctness reviewer), not by the black-box scenario set. This note flags the seam so the committed-tier plan provisions it.

Per-task acceptance criteria, test expectations, and dependency declarations are **plan-tier** artifacts and live with the committed-tier plan document (authored separately when this feature moves from `designed` → `committed`). They are NOT in scope for `/verify`-at-spec-stage.
