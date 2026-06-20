# V0 Substrate Plan — Acceptance-Criteria Verification Audit

**Date:** 2026-06-20  
**Verified against:** `main` @ `5c81af9`  
**Scope:** 126 of 132 ACs. Task 23's 6 ACs are excluded — they are governed by the Task 23 V0 execution-path detail plan, not re-verified here.

**Method:** Each AC is verified under a *default-deny* standard — an AC is `met` only with (a) cited implementation `file:line`, (b) a test that *actually asserts the AC's claim* (not merely a test that exists in the area), and (c) that test passing in the green workspace baseline. A second, independent *adversarial* pass attempts to refute every `met` verdict. Assertion-relaxation traps (`#[ignore]`, `is_ok()`-weakened assertions, name-matches-but-body-weaker) block `met`. Only `met` ACs are checked off in the plan.

## Result

| Verdict | Count |
|---|---|
| met (checked off) | 71 |
| partial | 32 |
| not-met | 23 |
| **total in scope** | **126** |

### Per task

| Task | met | partial | not-met |
|---|---|---|---|
| 1 | 5 | 1 | 0 |
| 2 | 3 | 0 | 0 |
| 3 | 4 | 2 | 0 |
| 4 | 3 | 0 | 0 |
| 5 | 4 | 0 | 0 |
| 6 | 3 | 1 | 0 |
| 7 | 4 | 3 | 0 |
| 8 | 2 | 2 | 0 |
| 9 | 3 | 0 | 0 |
| 10 | 5 | 2 | 0 |
| 11 | 1 | 1 | 0 |
| 12 | 6 | 0 | 0 |
| 13 | 4 | 2 | 1 |
| 14 | 1 | 3 | 0 |
| 15 | 3 | 3 | 0 |
| 16 | 6 | 0 | 0 |
| 17 | 4 | 0 | 0 |
| 18 | 2 | 2 | 0 |
| 19 | 1 | 3 | 1 |
| 20 | 2 | 2 | 0 |
| 21 | 1 | 4 | 1 |
| 22 | 4 | 1 | 0 |
| 24 | 0 | 0 | 5 |
| 25 | 0 | 0 | 8 |
| 26 | 0 | 0 | 4 |
| 27 | 0 | 0 | 3 |

## Partial verdicts

_Implementation present, but a sub-clause or the asserting test does not fully establish the AC's claim. Not checked off._

- **T1.3** — Each `verify-*` recipe emits `GATE <name> <PASS|FAIL>` on stdout (`R-0018-f`).
  - Partial held DOWN (not upgraded) — two recipes structurally cannot emit FAIL, and plan lines 53/54 confirm both the PASS-run and the FAIL-stub expectations went uncaptured.
- **T3.4** — Tests assert `@unstable` invocation emits a deprecation log warning and `@deprecated` invocation returns a structured error (`R-0012-e`).
  - PASS-THROUGH partial — downgrade rationale independently confirmed against live tree: abi.rs:106-109 returns a DispatchWarning VALUE, not a log.emit write; the AC's literal '@unstable emits a deprecation LOG warning' is unmet for the @unstable half. @deprecated half fully met. (Both behavioural tests are de-ignored and passing — 0 ignored in baseline — which strengthens the @deprecated half but does not lift the @unstable-log gap.)
- **T3.6** — `sampling.request` accepts `context_ids: [str]`, never artifact bodies; host forwards `context_ids` as opaque refs and does NOT resolve IDs to bodies (`R-0012-b`).
  - PASS-THROUGH partial — confirmed: clause (a) shape fully tested (list<string>, no bodies/body param, abi_contract.rs:664-702); clause (b) 'host does NOT resolve IDs to bodies' is behavioural, host_fns.rs:231 body is todo!(), no assertion exists. Half asserted.
- **T6.4** — No new dependency outside the license-tier model is added without review (`Constraints`: license-tier; Wasmtime/engine pins per `R-0007-i`).
  - Pass-through unchanged: process-claim AC with no asserting test; first-pass reasoning sound.
- **T7.4** — Storage substrate is partitioned into content + state-config, both regular Postgres tables (`R-0013-b`, `R-0013-c`).
  - Downgraded: test asserts existence only; the 'regular Postgres table' half is testable now (pg_class.relkind 'r' vs 'p') and untested. Same defect-class the inherited standard uses to hold T7.AC7 partial.
- **T7.6** — All migrations are forward-only and work against empty and populated DBs; no destructive migration runs without a verified pre-migration backup (`R-0013-d`).
  - Pass-through unchanged: backup sub-clause untested because destructive migrations are structurally unreachable at V0; first-pass reasoning sound. (Verified guard IS wired into apply().)
- **T7.7** — `pgvector` enabled in schema init; V0.1+ vector/full-text columns are non-breaking `ADD COLUMN` (no vector/tsvector columns created at V0) (`R-0001-g`, data model V0.1+ additions).
  - Pass-through unchanged: enabled half passes; column-absence negative only implied by exact-column-set test, not directly asserted.
- **T8.1** — Tests assert the artifact-table column set: `id` (ULID/TEXT PK), `workspace_id` (UUID NOT NULL, indexed), `type`, `frontmatter` (JSONB), `body` (nullable), `frontmatter_version`, `migrated_from`, `migrated_at`, `created_at`, `updated_at` (`R-0001-a`, data model).
  - Downgraded: AC spells out id-as-TEXT-PK, workspace_id UUID, frontmatter JSONB — none of these types nor the PK are asserted (testable now via data_type/table_constraints). Same standard that holds T7.AC7 partial.
- **T8.2** — Tests assert `migrated_from`, `migrated_at`, `frontmatter_version` are NOT inside the `frontmatter` JSONB (dedicated system columns) (`R-0001-b`).
  - Downgraded: existence of a typed column does not establish absence from JSONB, and artifact_table.rs:242-243 CHECK (frontmatter ? 'frontmatter_version') FORCES frontmatter_version INTO the JSONB on every valid row — the AC's negative is untested and partly CONTRADICTED by the impl.
- **T10.2** — Tests assert `BLAKE3(token_bytes)` is stored in `admin_tokens.token_hash`; raw bytes never stored; lookup is constant-time (`R-0008-b`).
  - DOWNGRADED met→partial: BLAKE3-stored/raw-never-stored halves hold by value, but 'lookup is constant-time' is unimplemented at the lookup layer (ct_eq not on the auth path; lookup_by_hash uses SQL equality) — same untested-sub-claim pattern graded partial for T10 AC5 / T13 AC6.
- **T10.5** — Tests assert the token file is written mode 600, owner = host UID, at `~/.config/mnemra/token`, overridable via `MNEMRA_TOKEN_FILE`; startup mode-check resolves the same override (`R-0008-e`).
  - Passed through unchanged: mode-600 write + reject-644 asserted by value; MNEMRA_TOKEN_FILE override, ~/.config path, owner-UID negative, startup-override have no asserting test.
- **T11.2** — The `authentication` builtin implements the static admin-token bootstrap per P-0008/P-0009; per-deployment OIDC AS configuration via RFC 9728 is available at the V0 substrate (full OIDC AS integration is V0.1+) (`R-0015-d`).
  - Passed through unchanged: RFC-9728 config surface asserted; P-0008/P-0009 static-token bootstrap path implemented but uncovered by any test and not invoked (mcp_server seeds via direct INSERT).
- **T13.2** — All read-path host-fns include `workspace_id = ctx.workspace_id` as a WHERE-clause condition derived from the argument, not a post-read filter (`R-0006-d`).
  - Passed through unchanged: WHERE-clause is a non-executed string literal in todo!() stubs; structurally lint-checked only, no runtime/DB assertion (lint file comment :393 confirms it is not yet load-bearing).
- **T13.6** — The RLS column-shape (`workspace_id NOT NULL` on every artifact table) ships; RLS `CREATE POLICY` objects are NOT activated at V0 (`R-0006-g`, `R-0009-g`).
  - Passed through unchanged: column-shape ships+asserted for one fixture table; 'every artifact table' is one table at V0; 'not activated' half has no pg_policies catalog test.
- **T14.1** — `Admin` authorizes all MCP verb categories, all CLI control-plane operations, and admin session management (`R-0009-c`).
  - DOWNGRADED met->partial: whole-repo grep shows Verb::{WorkspaceLifecycle,TokenRotation,MigrationTrigger,BackupTrigger,AdminSessionList,AdminSessionRevoke} are constructed ONLY in the matrix module + tests — no production caller builds them at a live boundary (dispatch.rs constructs only Plugin{Read,Write}Verb). The CLI-control-plane + admin-session clause is matrix-entry + unit-test only, the SAME latent condition that made 15-5 partial; rule-consistency forbids differing treatment. Met core: Admin allow-all + plugin-verb wiring.
- **T14.2** — `ReadObserver` authorizes only read-path MCP verbs (`artifact.get`, `artifact.list`, projection queries); write verbs + CLI control-plane + workspace lifecycle are denied at the host-fn boundary with a structured permission error (`R-0009-d`, `R-0009-e`, Scenario: Read-observer token denied write access).
  - DOWNGRADED met->partial: MCP write-path denial clause STANDS (boundary-tested via live tools/call; PluginWriteVerb actually constructed). BUT the 'CLI control-plane + workspace lifecycle denied at the host-fn boundary' clause (incl. R-0009-e) is unsupported at a live boundary — those Verb variants are never constructed in production (grep: matrix module + tests only), identical to 15-5. I had carried forward the first-pass 'matrix fn IS the boundary (advisor-confirmed)' without re-testing it; zero-trust re-check shows it fails for the CLI/workspace-lifecycle categories.
- **T14.3** — The permission matrix is enforced at the application layer only; no `CREATE POLICY` activated at V0 (`R-0009-g`).
  - PASS-THROUGH (partial, unchanged). Independently confirmed: zero real RLS statements anywhere in libs/mnemra-host; the no-RLS claim has no executable test, only code-review enforcement. Self-declared untested 'shall not' invariant.
- **T15.1** — `workspaces` builtin manages workspace lifecycle (create/delete/list); the `default` workspace is created on first-run and always exists after init; solo deployment collapses tenancy to `default` (`R-0015-a`, `R-0015-h`).
  - PASS-THROUGH (partial, unchanged). CRUD + default-persistence tested; tenancy-collapse clause is doc-comment only, untested.
- **T15.4** — `sessions` builtin manages per-MCP-connection session state; session context is the source of `WorkspaceCtx` construction (`R-0015-e`, `R-0006-b`).
  - PASS-THROUGH (partial, unchanged). CRUD clause met+tested; WorkspaceCtx-source clause is a documented Task-23 future seam, untested.
- **T15.5** — `projects` builtin manages the project registry; project identity is a prerequisite for plugin scoping — no plugin is scoped to a project before that project's record exists (`R-0015-g`).
  - PASS-THROUGH (partial, unchanged). Registry CRUD + exists() primitive met+tested; the scoping-enforcement clause has no production caller of projects::exists (only the test) — this is the reference condition I applied to downgrade 14-1/14-2 for consistency.
- **T18.2** — Outbound embedding calls enforce a per-deployment hostname allowlist; a call to a non-allowlisted hostname is blocked (`R-0014-b`).
  - DOWNGRADED met->partial: tests assert the standalone block primitive (AllowList::check returns Err off-list); no test asserts the AC line's 'outbound embedding calls enforce' claim because no outbound call site exists at V0 (grep of whole libs tree: only the signing load pipeline exists, nothing consults AllowList::check from a call flow). Structurally identical to T18 AC3 partial; downgraded for consistency under this recheck's own criterion (test must assert the AC line, not just the underlying primitive). 'Matches plan TDD-pair scope' addresses deliverable scoping, not the AC-line assertion.
- **T18.3** — The system hosts no language model; it does not accept an API key for a hosted model endpoint (`R-0014-c`).
  - Pass-through unchanged (partial confirmed correct, not re-litigated upward): impl genuinely satisfies absence (struct read directly), but the paired test admits in its own doc (449-451, 468-474) it is shape-only and a hidden hosted_endpoint field would still pass; test does not assert the specific AC claim.
- **T19.1** — The reference plugin ships `manifest.toml` declaring `schema_version = 1`, `core = true`, `name`, `version`, and the `[verbs]`, `[content_types]`, `[state_scopes]`, `[host_fns]`, `[signature]` sections (`R-0003-a`).
  - Passed through unchanged (already partial): impl complete; no test asserts the full required-section set / schema_version=1 / core against the REAL echo manifest file.
- **T19.2** — The manifest does NOT declare `workspace_id` as a parameter on any write-path host-fn (`R-0003-d`).
  - Passed through unchanged (already partial): asserting test is at the WIT layer, not the manifest; claim holds by construction at the manifest level.
- **T19.3** — `artifact.delete` appears in `host_fns.required` only if the plugin opts in; it is not granted by default (`R-0003-g`).
  - Passed through unchanged (already partial): default-deny semantics proven generically on synthetic manifests; the AC's real-manifest opt-in property is untested directly.
- **T20.1** — Tests assert the runtime compiles a per-instance host-fn allowlist from the signed manifest's `[host_fns]` before any instance is created; calls outside the allowlist fail at the WIT boundary, not at the host-fn body (`R-0003-b`).
  - Passed through unchanged (already partial): allowlist-compiled-before-instance met; the 'fail at the WIT boundary, not host-fn body' half is integration-test scope (manifest_load.rs:354-361) and unasserted.
- **T20.4** — Tests assert the MCP handler enforces a per-verb capability check against the manifest's declared `verbs` list before dispatching to the plugin runtime (`R-0010-d`).
  - Passed through unchanged (already partial): capability-list compilation + query met; the AC's 'MCP handler enforces … before dispatching' is Task 23 and unasserted at this task.
- **T21.2** — The plugin pool holds 3–5 instances per plugin type, initialized at host startup before the MCP server accepts requests; instances are tenant-stateless; no cross-call state held (`R-0016-a`, `R-0016-b`, `R-0007-d`).
  - Passed through unchanged (already partial): min-pool size impl + partial test; the <=5 upper bound, pool-before-MCP-accept timing (deferred to Task 23), and tenant-statelessness / no-cross-call-state (R-0007-d) are untested.
- **T21.3** — Wasmtime fuel metering (`Store::set_fuel`, 10B ticks) AND epoch-interruption (`set_epoch_deadline(500)`, 5s, 10ms host epoch-tick thread) are BOTH active simultaneously; memory ceiling 64 MiB via `static_memory_maximum_size` or `ResourceLimiter` (`R-0007-a`, `R-0007-b`, `R-0007-c`, `R-0007-g`).
  - Passed through unchanged (already partial): core 'both fuel+epoch active simultaneously' met; memory ceiling (64 MiB) value+limiter present but NO test exercises it (trap_recovery.rs:215-228: memory denial not a classifiable trap in wasmtime 45).
- **T21.4** — The host epoch-tick thread starts before any plugin is invoked, is supervised, is NOT restarted silently on crash; on crash it emits `epoch_tick_thread_died`, refuses new plugin invocations until restart is confirmed, attempts one supervised restart/min with backoff, and `/health` `overall` reflects `"degraded"` while dead (`R-0007-h`).
  - Passed through unchanged (already partial): re-check CONFIRMS the asserting test is test-hooks-gated and NOT in the baseline (6 tests ran, not 8), and inject_death_for_test bypasses the spawn_tick_thread panic path so epoch_tick_thread_died emission is untested; /health degraded deferred to Task 23.
- **T21.6** — The Wasmtime crate version is pinned in `Cargo.toml` (no wildcard / open constraint); recorded in the `verify-build` SBOM; a major/minor upgrade requires explicit approval, not dependabot auto-merge (`R-0007-i`).
  - Passed through unchanged (already partial): pin present and correct (impl met); no test slot by nature of a Cargo.toml fact; SBOM/upgrade-approval are out-of-band process.
- **T22.1** — On any resource-limit violation (fuel exhaustion, epoch deadline, memory ceiling): catch the trap; emit a structured event with `(workspace_id, plugin_id, plugin_version, limit_type, limit_value)`; poison the pool slot and replace with a new instance; return a structured error for the current invocation (`R-0007-e`).
  - Passed through unchanged (already partial): fuel+epoch fully met with value-asserted event payload; the memory-ceiling branch of 'any … violation' is untested (trap_recovery.rs:215-228: memory denial not a classifiable trap in wasmtime 45).

## Not-met verdicts

_Unbuilt, or not implemented as specified. Tasks 24–27 (admin CLI, observability, signed-build pipeline, acceptance smoke) are unbuilt — the build front is at Task 23._

- **T13.3** — Builtin components use the same `WorkspaceCtx` threading; no "internal" DB-query bypass without a `WorkspaceCtx` (`R-0006-e`).
  - Passed through unchanged: builtins issue WHERE workspace_id=$1 scoped by a raw Uuid param — exactly the bypass the AC forbids; neither implemented as specified nor tested.
- **T19.5** — The plugin's `[signature]` is signed by the mnemra root on the build host (`R-0002-a`, `R-0005-c` build-pipeline).
  - Passed through unchanged (not-met): manifest is NOT root-signed; carries documented build-time placeholders, explicitly deferred to Task 26.
- **T21.5** — The four `core: true` plugins are signed by the mnemra root, structurally non-uninstallable at runtime; the only removal path is a binary rebuild (`R-0002-a`, `R-0002-d`).
  - Passed through unchanged (not-met): no affirmative test; 'four' plugins do not exist at V0; signature is placeholder.
- **T24.1** — CLI subcommands are dynamically generated from plugin manifests at startup; a new manifest verb produces a new subcommand without a CLI code change; removing the verb + restarting removes the subcommand (`R-0011-a`, Scenario: Admin CLI schema-driven subcommand generation).
  - Stands: independently confirmed no CLI surface exists at HEAD 5c81af9; Task 24 unbuilt, build front at Task 23.
- **T24.2** — The CLI handles destructive/control-plane operations only; agent-facing CRUD is not exposed on the CLI (`R-0011-b`).
  - Stands: no control_plane source exists; CLI unbuilt.
- **T24.3** — Destructive CLI operations require admin-token authentication; UNIX UID match alone is insufficient (`R-0011-c`).
  - Stands: no CLI to enforce token auth; unbuilt.
- **T24.4** — The CLI exposes `workspace create`, `workspace delete`, `workspace list`, `token rotate`, `migrate` (one-shot trigger), `backup` (trigger), `health` (human-readable `/health` wrapper) (`R-0011-d`).
  - Stands: none of the named verbs exist as CLI subcommands; unbuilt.
- **T24.5** — Workspace lifecycle (`workspace create`/`delete`) requires `Role::Admin`; a `ReadObserver` request returns a structured permission error (`R-0009-e`).
  - Stands: independently verified permissions.rs is host-layer role authorization (Task 14), not CLI enforcement; CLI path the AC requires is unbuilt.
- **T25.1** — Per-verb metric record emitted on every MCP verb dispatch with `workspace_id`, `verb`, `outcome` (`ok`/`error`/`timeout`), `duration_ms`, `recorded_at`, as OTel metric/structured form exportable to a configurable OTLP endpoint; no artifact IDs / content fragments / agent identity in the metric; never written to an in-app hypertable/table (`R-0004-a`, `R-0004-f`, data model emission record).
  - Stands: independently confirmed no emission code at HEAD; Task 25 unbuilt.
- **T25.2** — Event record emitted with `workspace_id`, `event_type`, `event_version` (DEFAULT 1), `token_id` (host-derived from `WorkspaceCtx`, not plugin-supplied; a plugin-supplied `token_id` is rejected at the WIT boundary), `agent_id`, `session_id`, `payload` (never artifact bodies), `recorded_at`; structured/OTel, not an in-app hypertable (`R-0004-b`, `R-0009-b`, data model emission record).
  - Stands: no event emission code exists; unbuilt.
- **T25.3** — Structured log records emitted to stdout (one JSON line per record) with level, message, timestamp, `workspace_id` on every tenant-scoped record; no in-app `logs` table, no in-app log-retention worker (`R-0004-d`).
  - Stands: self-documenting deferral in logging.rs:23; the host-fn log.emit emission path is unbuilt.
- **T25.4** — No in-app observability storage initialized: no metrics/events hypertable, no `add_retention_policy`, no continuous aggregate; emission succeeds and is observable even with no persistent backend configured (`R-0004-c`, `R-0004-e`, `R-0004-f`).
  - Stands: the passing no-hypertable test belongs to Task 7 init; the emission-without-backend behavior is unbuilt.
- **T25.5** — No artifact bodies / content fragments / raw query strings in any emitted metric, event, or log; redaction enforced at the emission boundary for high-entropy strings (`R-0004-h`).
  - Stands: no emission boundary exists to redact at; unbuilt (logging.rs:23 defers redaction to Task 25).
- **T25.6** — `event_version` increments on breaking event-type schema changes; backward-compatible additions do not increment it (`R-0004-i`).
  - Stands: no event record type exists to version; unbuilt.
- **T25.7** — `GET /health` is the first API, started before config load and before the MCP server accepts requests, on a dedicated loopback-only TCP listener bound to `127.0.0.1` (not `0.0.0.0`/`::`), port via `MNEMRA_HEALTH_PORT` (default 8877), serving only `GET /health` (`R-0004-g`).
  - Stands: confirmed the only health-named artifact is a Task 21 poison-lock fixture, not a listener; Task 25 unbuilt.
- **T25.8** — `/health` returns the structured detail body `{ "postgres", "pgvector", "workspace_default", "overall" }` to every caller it serves (loopback IS the gate; no admin-token gating at V0); `200` healthy / `503` when a dependency is down; `down` mid-shutdown (`R-0004-g`, Scenario: `/health` degraded and down states, API Contract health endpoint).
  - Stands: no /health handler exists; unbuilt.
- **T26.1** — `verify-build` produces the signed binary with the embedded root verification material (`R-0018-f`, `R-0005-d`).
  - Stands: confirmed verify-build is the Task 1 scaffold; embedded VERIFY material is Task 17; Task 26 SIGN pipeline unbuilt.
- **T26.2** — The SBOM records the pinned Wasmtime version (`R-0007-i`).
  - Stands: Wasmtime pinned at 45.0.2 (Task 21) but no SBOM records it; the AC's subject (the SBOM) is unbuilt.
- **T26.3** — An ABI-change PR causes all `core: true` plugins to recompile against the new ABI and pass their tests before merge (`R-0017-a`).
  - Stands: no ABI-recompile gate exists; unbuilt.
- **T26.4** — The signing key resides on the build host's filesystem at mode 600, owner = build-pipeline UID, NOT co-located on the deployment node; the deployment node receives only signed artifacts + verification material (`R-0005-c` — build-pipeline/ops invariant; see Spec gaps note).
  - Stands: ops/build-pipeline invariant with no runtime code surface; unbuilt regardless.
- **T27.1** — `verify-smoke` runs `mnemra init` against a fresh embedded engine and asserts the Single-node V0 substrate initialization scenario end-to-end (tables, `pgvector`, `default` workspace, seven builtins, pool init, synchronous signature verify, health event, `/health` detail body `overall: "ok"`) (Scenario: Single-node V0 substrate initialization, `R-0013-a`).
  - Stands: confirmed verify-smoke is the Task 1 scaffold echo; schema_init piecewise tests are Task 7; sealed E2E unbuilt.
- **T27.2** — A round-trip MCP verb dispatch against the reference plugin succeeds and emits a per-verb metric (Scenario: MCP verb dispatches under WorkspaceCtx).
  - Stands: confirmed the dispatch test self-documents that metric emission is unwired; metric emission (Task 25) and smoke seal unbuilt.
- **T27.3** — Integration tests run against the real surface (HTTP `/health` handler, actual embedded Postgres + `pgvector`), not mocks (`R-0018-b`).
  - Stands: the /health handler the AC must exercise does not exist; smoke integration unbuilt.

