# Plan: Mnemra Core V0 Substrate (`0.1.0`)

> **Spec:** `docs/specs/2026-05-24-mnemra-core-v0-substrate.md` (locked 2026-05-24 at SHA `23e7f83cf770631b7a4db567a1054145cd732a97`)
> **Date:** 2026-06-12
> **Status:** drafted
> **Target release:** `0.1.0`
> **Plan-exit gate:** passed 2026-06-12 — maintainer approved; Task 19 reference/fixture-plugin substitution (evolved `mnemra-echo`) ratified

## Purpose

Decompose the locked V0-substrate spec into a committed-tier task sequence that an implementer team can execute against — the host core, storage trait + embedded-Postgres schema, plugin runtime, host-fn ABI, observability emission, auth, and tenant enforcement that every `0.2.0`–`1.0.0` capability increment rides on.

Per `G-0028` (Designed-tier vs committed-tier amendment, 2026-05-24), this is the committed-tier artifact carrying the task surface for the locked Spec above. Plans are transient — they archive out when the feature lands `live`; git history is the long-term record.

## Reading notes (orientation for the implementer — not new requirements)

These frame how the task list reads against the spec's closed world. They restate decomposition consequences of the spec/ADRs, never extend them.

1. **Repo is a Wasmtime spike, not greenfield-empty.** The worktree carries a Cargo workspace (`Cargo.toml` resolver-3, members `cmd/mnemra`, `plugins/mnemra-echo`), a toy WIT (`wit/echo.wit` — `echo`/`increment-counter`), an existence-test host (`cmd/mnemra/main.rs` — loads one component, asserts a counter persists), and a `justfile` with docs recipes only (no `verify-*` recipes yet). No `src/` runtime crate, no storage, no MCP, no auth, no signing, no manifest exists. Early tasks scaffold the real crates; most runtime files are **to-be-created**.

2. **The reference/fixture plugin (decomposition decision — flagged at plan-exit).** Five scenarios (Scenario 2 signing-load, Scenario 3 MCP dispatch `INSERT INTO tasks`, Scenario 7 resource-limit breach, Scenario 13 fuel exhaustion, Scenario 15 admin-CLI `task.audit`) cite a concrete `tasks` plugin and `tasks/manifest.toml`. The `tasks` plugin **is the `0.2.0` capability family, which is explicitly Out of Scope** for `0.1.0` (spec Out of Scope: "Capability family implementations … out of scope"). The substrate cannot be exercised without *some* signed `core: true` plugin to load, sign-verify, pool, fuel/epoch-limit, and dispatch against. **Resolution: evolve the existing `mnemra-echo` (`plugins/mnemra-echo/`) into the signed reference/fixture plugin** — give it a `manifest.toml`, a fixture content-type table, and the real host-fn ABI surface, so it exercises `artifact.*`, the WHERE-clause path, signing, pooling, and limits **as substrate scaffolding, not as a capability family**. This is in-scope substrate work by the spec's own discriminator (a fixture ≠ a `0.2.0`–`0.14.0` family). Verified: `mnemra-echo` has no `manifest.toml` today and its WIT is the toy `echo` ABI. Where the spec's scenarios say `tasks`/`task.create`/`task.audit`, the V0 acceptance test substitutes the reference plugin's name/verbs (e.g. `mnemra-echo`/`echo.create`/`echo.audit`); the spec's `tasks`-named scenarios are validated against the reference fixture at V0 and re-validated against the real `tasks` plugin at `0.2.0`. **This substitution is surfaced for maintainer ratification at the plan-exit gate** (it is the one place this plan resolves a scenario-vs-out-of-scope tension; if the maintainer prefers a differently-named throwaway fixture, only names change, not task structure).

3. **Concrete tables vs. table machinery.** In-scope concrete tables at V0: `admin_tokens` (R-0008-c) and the seven builtins' state tables (R-0015-a–g). The per-artifact-type table *machinery* (R-0001-d per-type tables, R-0001-e history-trigger shadow tables, expression indexes, R-0001-f refresh-queue + worker) is the **substrate mechanism**, built and exercised against the reference plugin's fixture content-type table — **not** against the real `tasks`/`repos`/`jobs`/`contacts` family tables (those land with `0.2.0`+).

4. **Green-main definition for a foundational layer.** "Green main candidate" (every task independently shippable) means **`just ci` passes — compiles + the task's own tests green** — not standalone user-facing value. A layer (WIT ABI → `Storage` trait → `WorkspaceCtx` → host-fns → MCP/CLI → plugin runtime) merges green because it compiles and its unit/integration tests pass, even before the layer above wires to it. This is the trunk-based-mergeable shape for substrate work; no forced co-landing of siblings.

5. **Storage trait scope (P-0010 D5).** The V0 `Storage`-trait deliverable is the trait exposing a **unit-of-work / transaction boundary + a workspace-isolation contract**, exercised by the design-time **two-adapter test** (in-memory adapter + Postgres adapter). Full keyed supersession (superseded-vector delete + FTS update) rides on `embedding`/`search_tsv` columns that are V0.1+ `ADD COLUMN` (R-0001-g, data model) — the trait must *express* atomic multi-write; V0 does not exercise real supersession. The contract test is scoped to the two invariants, not the retrieval capability.

## Tasks

Tasks are grouped by layer (see **Sequencing**). TDD pairs are split into a red-phase task (acceptance-test author writes failing tests) and a green-phase task (implementer makes them pass) per R-0018-a on security-sensitive / public-ABI / parser / validator surfaces; mechanical surfaces collapse to implementer-self-test. Sizing is **relative (S/M/L)** — this is the first plan in this repo (`docs/plans/archive/` does not exist), so there are **no historical anchors and absolute duration estimates are uncalibrated and deliberately omitted**.

---

### Task 1: CI gate scaffold — `verify-*` recipes + `just ci`

**Files:** `justfile` (extend), `.github/workflows/ci.yml` (to-be-created or extend)
**Type:** infra
**Depends on:** None
**Size:** S

**What:** Add the fixed `verify-*` recipe set and the single `ci` entry point to the `justfile`, each emitting `GATE <name> <PASS|FAIL>` on stdout, wired to pass against the (currently near-empty) workspace. This operationally *defines* "green on main" for every later task, so it lands first.

**Acceptance Criteria:**
- [ ] `justfile` declares recipes `verify-test`, `verify-lint`, `verify-type`, `verify-coverage`, `verify-build`, `verify-smoke`, and `ci` (`R-0018-f`).
- [ ] `just ci` is the sole CI entry point and invokes every `verify-*` recipe (`R-0018-f`).
- [ ] Each `verify-*` recipe emits `GATE <name> <PASS|FAIL>` on stdout (`R-0018-f`).
- [ ] No `verify-*` recipe has `--fix` side effects (`R-0018-f`).
- [ ] `verify-lint` reserves a slot for the WHERE-clause lint check (wired in Task 12) (`R-0018-f`, forward to `R-0018-d`).
- [ ] CI runs on worktree branches; main is protected from direct pushes (`R-0018-c`).

**Test Expectations:**
- `just ci` exits 0 on the empty/scaffold workspace and prints a `GATE … PASS` line per recipe.
- A deliberately failing stub recipe surfaces `GATE <name> FAIL` and non-zero exit (gate-shape smoke).

---

### Task 2: Runtime workspace scaffold — host crate + crate layout

**Files:** `Cargo.toml` (workspace members), `crates/mnemra-host/Cargo.toml` (to-be-created), `crates/mnemra-host/src/lib.rs` (to-be-created), `cmd/mnemra/main.rs` (replace spike entry), `cmd/mnemra/Cargo.toml`
**Type:** infra
**Depends on:** Task 1
**Size:** M

**What:** Establish the runtime crate structure (host library crate(s) + the `mnemra` binary as a thin entry), replacing the existence-test spike `main.rs` with a real startup skeleton. Follow the workspace Rust layout convention (cmd / libs / plugins). No business logic — just the compiling skeleton subsequent layers fill.

**Acceptance Criteria:**
- [ ] A host library crate exists and is a workspace member; `cmd/mnemra` depends on it.
- [ ] The `mnemra` binary builds (`just verify-build` green) and exposes a startup entry that later tasks extend (`R-0018-e` Rust-default).
- [ ] The Wasmtime spike's existence-test assertions are removed or migrated into the plugin-runtime test surface (no orphan spike asserts in `main`).

**Test Expectations:**
- Workspace compiles; `cargo test` runs (zero or skeleton tests) green.
- No dead `mnemra-echo`-spike coupling remains in `main` (it moves under the plugin-runtime path in Task 19/20).

---

### Task 3 (RED): Host-fn WIT ABI contract tests — `WorkspaceCtx`-first, no write-path `workspace_id`

**Files:** `wit/host.wit` (to-be-created), `crates/mnemra-host/tests/abi_contract.rs` (to-be-created)
**Type:** test
**Depends on:** Task 2
**Size:** M

**What:** Author failing tests + WIT-shape assertions that pin the host-fn ABI structural invariants: every host-fn takes `WorkspaceCtx` first; no write-path host-fn exposes `workspace_id` as a parameter; all params/returns are WIT component types; `@stable`/`@unstable`/`@deprecated` annotations are present and behaviourally distinct.

**Acceptance Criteria (red — these tests MUST exist and fail before Task 4):**
- [ ] A build-time/test assertion proves no WIT-generated write-path function accepts `workspace_id` as a parameter (`R-0012-d`, `R-0003-d`, Scenario: Cross-workspace SQL leak — compile-time ABI prevention).
- [ ] Tests assert each required host-fn signature matches the API Contract table: `artifact.create`, `artifact.update`, `artifact.get`, `artifact.list`, `artifact.delete`, `metrics.record`, `log.emit`, `event.emit`, `projection.emit` (`R-0012-a`).
- [ ] Tests assert opt-in surfaces `sampling.request`, `secrets.get` exist as optional and `artifact.delete` is opt-in (`R-0012-a`, `R-0012-b`, `R-0012-c`, `R-0003-g`).
- [ ] Tests assert `@unstable` invocation emits a deprecation log warning and `@deprecated` invocation returns a structured error (`R-0012-e`).
- [ ] Tests assert all host-fn params/returns are WIT component types; no raw byte buffer with dynamic dispatch (`R-0012-f`).
- [ ] `sampling.request` accepts `context_ids: [str]`, never artifact bodies; host forwards `context_ids` as opaque refs and does NOT resolve IDs to bodies (`R-0012-b`).

**Test Expectations:**
- This is a red-phase task: `verify` is empty by design (tests fail until Task 4 lands the WIT + host-fn skeleton). Rationale recorded so the failing state is not mistaken for a regression.

---

### Task 4 (GREEN): Host-fn WIT ABI definition + binding skeleton

**Files:** `wit/host.wit` (to-be-created), `crates/mnemra-host/src/abi/mod.rs` (to-be-created), `crates/mnemra-host/src/abi/host_fns.rs` (to-be-created)
**Type:** backend
**Depends on:** Task 3
**Size:** L

**What:** Define the real host-fn WIT interface (superseding the toy `echo` ABI for the host surface) and the Rust binding skeleton so the Task 3 contract tests pass. Bodies may be unimplemented stubs that compile and satisfy the type contract; behaviour lands as later layers wire in.

**Acceptance Criteria:**
- [ ] All Task 3 (RED) contract tests pass (`R-0012-a/b/c/d/e/f`, `R-0003-d/g`).
- [ ] The required host-fn set is declared in WIT with `WorkspaceCtx`-first calling convention (host-derived, not WIT-exposed on write paths) (`R-0012-a`, `R-0012-d`).
- [ ] Each host-fn carries an `@stable` or `@unstable` annotation in the WIT (`R-0012-e`).

**Test Expectations:**
- Round-trip: the contract suite from Task 3 goes green.
- Negative: an attempt to declare a write-path host-fn with `workspace_id` fails to compile/bind (the ABI makes the wrong thing inexpressible).

---

### Task 5: `Storage` trait + in-memory adapter + two-adapter contract test

**Files:** `crates/mnemra-host/src/storage/mod.rs` (to-be-created), `crates/mnemra-host/src/storage/memory.rs` (to-be-created), `crates/mnemra-host/tests/storage_contract.rs` (to-be-created)
**Type:** backend
**Depends on:** Task 2
**Size:** L

**What:** Define the engine-agnostic, swappable `Storage` trait exposing a unit-of-work / transaction boundary, plus an in-memory test adapter. Author the design-time two-adapter contract test binding the two co-equal invariants (single-transaction atomic multi-write; workspace-scoped isolation). Postgres adapter lands in Task 6.

**Acceptance Criteria:**
- [ ] `Storage` is engine-agnostic and exposes a unit-of-work/transaction boundary (commit-or-rollback as a whole), not per-write autocommit (`P-0010` D5).
- [ ] An in-memory adapter implements `Storage` for the test/layering seam (`P-0010` D5).
- [ ] The two-adapter contract test asserts (a) atomic multi-write commits or rolls back as a unit, and (b) operations in one workspace context cannot read/mutate another workspace's rows (`P-0010` D5 two co-equal invariants).
- [ ] The trait *expresses* atomic multi-write (the supersession surface) without exercising real keyed supersession (V0.1+) (`R-0001-g`, data model V0.1+ columns).

**Test Expectations:**
- Contract test runs against the in-memory adapter now and against the Postgres adapter (Task 6) — same suite, two adapters.
- Isolation negative: a cross-workspace read returns zero rows under the in-memory adapter.

---

### Task 6: Embedded Postgres engine bring-up + `Storage` Postgres adapter

**Files:** `crates/mnemra-host/src/storage/postgres/mod.rs` (to-be-created), `crates/mnemra-host/src/storage/postgres/engine.rs` (to-be-created), `Cargo.toml` (deps: `postgresql_embedded`, `pgvector_compiled`)
**Type:** backend
**Depends on:** Task 5
**Size:** L

**What:** Bring up the embedded Postgres engine (`postgresql_embedded`) with bundled/compiled `pgvector` (`pgvector_compiled`), and implement the Postgres `Storage` adapter so the Task 5 two-adapter contract test passes against real Postgres. No external server, no OS-installed extensions.

**Acceptance Criteria:**
- [ ] The embedded Postgres engine starts in-process; `pgvector` is bundled/compiled, not OS-installed (`P-0010` V0-engine, Constraints, `R-0013-a`).
- [ ] The Postgres adapter passes the Task 5 two-adapter contract suite (atomicity + isolation) (`P-0010` D5).
- [ ] The application DB role holds neither `BYPASSRLS` nor superuser (`P-0010` multi-tenancy preconditions; forward-context for V0.1+ RLS, enforced application-layer at V0).
- [ ] No new dependency outside the license-tier model is added without review (`Constraints`: license-tier; Wasmtime/engine pins per `R-0007-i`).

**Test Expectations:**
- Integration test runs against the real embedded engine (not a mock) (`R-0018-b`).
- `\dx` introspection lists `pgvector`; `CREATE EXTENSION` failure surfaces a structured error (forward to Task 7 `mnemra init`).

> **Review gate A (operational review):** storage substrate bring-up (Task 6) before schema-init (Task 7) builds on it. See Sequencing.

---

### Task 7: Schema initialization — `mnemra init` (tables, indexes, pgvector, default workspace, health-ok)

**Files:** `crates/mnemra-host/src/schema/init.rs` (to-be-created), `crates/mnemra-host/src/schema/migrations/` (to-be-created), `cmd/mnemra/src/cmd/init.rs` (to-be-created)
**Type:** backend
**Depends on:** Task 6
**Size:** L

**What:** Implement `mnemra init` first-run bootstrap: enable bundled `pgvector`, create all substrate tables + indexes, create the `default` workspace, partition storage into content (`DS-pg-content`) + state-config (`DS-pg-state`) regular tables, create least-privilege DB roles, and emit an `overall: "ok"` health event. Forward-only, idempotent against empty and populated DBs.

**Acceptance Criteria:**
- [ ] `mnemra init` enables `pgvector` against the embedded engine; if `CREATE EXTENSION pgvector` errors, init returns a structured error naming the missing extension and does NOT proceed (`R-0013-a`).
- [ ] `mnemra init` creates substrate tables + indexes, creates the `default` workspace, and emits a health event returning `overall: "ok"` (`R-0013-a`, `R-0015-a`, `R-0015-h`, Scenario: Single-node V0 substrate initialization).
- [ ] After init, `\d+` lists **no** metrics/events hypertable and `\dx` does **not** list `timescaledb` (`R-0004-c`, `R-0013-a`, Scenario: Observability emits during a dogfood session).
- [ ] Storage substrate is partitioned into content + state-config, both regular Postgres tables (`R-0013-b`, `R-0013-c`).
- [ ] Least-privilege DB roles are created: host-fns, migration, backup, health-probe (`R-0013-e`).
- [ ] All migrations are forward-only and work against empty and populated DBs; no destructive migration runs without a verified pre-migration backup (`R-0013-d`).
- [ ] `pgvector` enabled in schema init; V0.1+ vector/full-text columns are non-breaking `ADD COLUMN` (no vector/tsvector columns created at V0) (`R-0001-g`, data model V0.1+ additions).

**Test Expectations:**
- Round-trip: `mnemra init` against a fresh embedded engine yields a working schema; re-running is idempotent.
- Negative: `pgvector` unavailable → structured error, no partial schema.
- Introspection: no `timescaledb`, no observability hypertable post-init.

---

### Task 8 (RED): Content-substrate schema tests — C1 layout, system-column separation, CHECK constraints

**Files:** `crates/mnemra-host/tests/content_schema.rs` (to-be-created)
**Type:** test
**Depends on:** Task 7
**Size:** M

**What:** Failing tests pinning the C1 single-document artifact-table layout, the system-column-vs-JSONB separation, and the schema-level CHECK constraints — exercised against the **reference plugin's fixture content-type table** (Task 19), not the real family tables.

**Acceptance Criteria (red):**
- [ ] Tests assert the artifact-table column set: `id` (ULID/TEXT PK), `workspace_id` (UUID NOT NULL, indexed), `type`, `frontmatter` (JSONB), `body` (nullable), `frontmatter_version`, `migrated_from`, `migrated_at`, `created_at`, `updated_at` (`R-0001-a`, data model).
- [ ] Tests assert `migrated_from`, `migrated_at`, `frontmatter_version` are NOT inside the `frontmatter` JSONB (dedicated system columns) (`R-0001-b`).
- [ ] Tests assert `CHECK (frontmatter ? 'id')` and `CHECK (frontmatter ? 'frontmatter_version')` reject violating inserts (`R-0001-c`).
- [ ] Tests assert per-artifact-type tables (not a polymorphic single table) and expression indexes on `(frontmatter->>'status')`, `(frontmatter->>'priority')`, `(frontmatter->>'project_id')`, `(frontmatter->>'parent_id')` (`R-0001-d`, data model).

**Test Expectations:**
- Red-phase: `verify` empty by design until Task 9 lands the schema generator. Rationale recorded.

---

### Task 9 (GREEN): Per-artifact-type table generator + history shadow tables + projection refresh queue

**Files:** `crates/mnemra-host/src/schema/artifact_table.rs` (to-be-created), `crates/mnemra-host/src/schema/history_trigger.rs` (to-be-created), `crates/mnemra-host/src/projection/refresh_queue.rs` (to-be-created), `crates/mnemra-host/src/projection/worker.rs` (to-be-created)
**Type:** backend
**Depends on:** Task 8
**Size:** L

**What:** Implement the per-artifact-type table generation machinery (columns, CHECK constraints, expression indexes), the trigger-based `<artifact>_history` shadow table (UPDATE + DELETE history rows), and the host-owned materialized-projection refresh queue + background drain worker. Exercised against the fixture content-type.

**Acceptance Criteria:**
- [ ] All Task 8 (RED) schema tests pass (`R-0001-a/b/c/d`).
- [ ] On UPDATE, the `<artifact>_history` shadow table preserves the prior `frontmatter` byte-for-byte; on `artifact.delete`, a history row with `operation = 'DELETE'`, `old_frontmatter`, `old_body` is written before the DELETE executes (`R-0001-e`, data model history table).
- [ ] Materialized projection views refresh via a host-owned queue on host-fn write completion using `REFRESH MATERIALIZED VIEW CONCURRENTLY`; a background worker drains the queue (`R-0001-f`).

**Test Expectations:**
- Round-trip: UPDATE writes a byte-exact history row; DELETE writes a `'DELETE'` history row before removing the artifact.
- Refresh: a host-fn write enqueues a refresh; the worker drains it; the projection reflects the write.
- Edge: concurrent refresh does not block reads (`CONCURRENTLY`).

---

### Task 10 (RED): Admin-token shape + auth tests

**Files:** `crates/mnemra-host/tests/admin_token.rs` (to-be-created)
**Type:** test
**Depends on:** Task 7
**Size:** M

**What:** Failing tests pinning the admin-token cryptographic shape, hashed storage, constant-time comparison, NOT-NULL workspace claim, file-mode invariant, and rotation/revocation semantics.

**Acceptance Criteria (red):**
- [ ] Tests assert the token is 32 bytes CSPRNG, base64url-encoded (43 chars, no padding), no structural content in the bytes (`R-0008-a`). No hardcoded token literals in tests; generate per-run.
- [ ] Tests assert `BLAKE3(token_bytes)` is stored in `admin_tokens.token_hash`; raw bytes never stored; lookup is constant-time (`R-0008-b`).
- [ ] Tests assert the `admin_tokens` schema: `id UUID PK, token_hash BYTEA NOT NULL UNIQUE, workspace_id UUID NOT NULL, scopes TEXT[] NOT NULL, created_at TIMESTAMPTZ NOT NULL, rotated_at TIMESTAMPTZ` (`R-0008-c`, data model).
- [ ] Tests assert a NULL `workspace_id` token row is a schema violation; absence of a workspace claim is a hard auth failure, not a default (`R-0008-d`).
- [ ] Tests assert the token file is written mode 600, owner = host UID, at `~/.config/mnemra/token`, overridable via `MNEMRA_TOKEN_FILE`; startup mode-check resolves the same override (`R-0008-e`).
- [ ] Tests assert revocation = row deletion + new generation (no block-list); rotation emits a `token_rotated` event carrying the rotated `token_id` BEFORE the old row is deleted; old-hash lookups then return zero rows with no grace period (`R-0008-f`, `R-0008-g`, `R-0009-i`, Scenario: Token rotation event ordering).
- [ ] Tests assert no second signing key is introduced for admin-token minting (`R-0008-h`).

**Test Expectations:**
- Red-phase: `verify` empty by design until Task 11. Rationale recorded.

---

### Task 11 (GREEN): Admin-token implementation + `authentication` builtin bootstrap

**Files:** `crates/mnemra-host/src/auth/token.rs` (to-be-created), `crates/mnemra-host/src/builtins/authentication.rs` (to-be-created)
**Type:** backend
**Depends on:** Task 10
**Size:** L

**What:** Implement admin-token generation/hashing/comparison/rotation and the `authentication` builtin (static admin-token bootstrap path; RFC 9728 protected-resource-metadata config surface available, full OIDC AS integration V0.1+).

**Acceptance Criteria:**
- [ ] All Task 10 (RED) admin-token tests pass (`R-0008-a/b/c/d/e/f/g/h`, `R-0009-i`).
- [ ] The `authentication` builtin implements the static admin-token bootstrap per P-0008/P-0009; per-deployment OIDC AS configuration via RFC 9728 is available at the V0 substrate (full OIDC AS integration is V0.1+) (`R-0015-d`).

**Test Expectations:**
- Round-trip: generate → store hash → lookup → constant-time match.
- Rotation ordering: event emitted before row deletion (Scenario: Token rotation event ordering).
- Negative: bogus token → zero-row lookup → hard auth failure (forward to Task 23 Scenario: Admin token mismatch — 401).

---

### Task 12 (RED): `WorkspaceCtx` ABI tests + WHERE-clause lint check

**Files:** `crates/mnemra-host/tests/workspace_ctx.rs` (to-be-created), `crates/mnemra-host/tests/lint_workspace_clause.rs` (to-be-created)
**Type:** test
**Depends on:** Task 4, Task 11
**Size:** M

**What:** Failing tests pinning the `WorkspaceCtx` structural invariants (single construction site, private field + accessor, first-typed-parameter, test-only constructor) AND the read-path WHERE-clause lint (`syn` AST parse; planted violation must return non-zero and name the offending function). The lint's red phase needs host-fn source to scan, so it lands with/just after the first host-fn (Task 4) — before host-fns proliferate.

**Acceptance Criteria (red):**
- [ ] Tests assert every host-fn takes `WorkspaceCtx` as its first typed parameter; a host-fn issuing a DB query without a `WorkspaceCtx` is not expressible (`R-0006-a`).
- [ ] Tests assert `WorkspaceCtx` is constructed at a single location after token validation; no alternative production construction path (`R-0006-b`).
- [ ] Tests assert `workspace_id` is a private field (`workspace_id: Uuid`) with a public accessor `workspace_id(&self) -> Uuid`; direct field access is private (`R-0006-c`).
- [ ] Tests assert `WorkspaceCtx` carries `workspace_id: Uuid`, `role: Role`, `token_id: Uuid` (`R-0009-b`, data model).
- [ ] Tests assert the test-only constructor is `#[cfg(test)]`-gated and not callable in production paths (`R-0006-f`).
- [ ] The lint (`cargo test --test lint_workspace_clause`, `syn` AST) asserts 100% read-path WHERE-clause `workspace_id = ctx.workspace_id` coverage; a planted read-path host-fn without the clause MUST return non-zero and name the offending function (`R-0006-d`, `R-0018-d`).

**Test Expectations:**
- Red-phase: `verify` empty by design until Task 13. Rationale recorded.
- The planted-violation case is itself a test fixture (the lint must catch it).

---

### Task 13 (GREEN): `WorkspaceCtx` + `Role` + host-fn `WorkspaceCtx` threading + wire WHERE-lint into `verify-lint`

**Files:** `crates/mnemra-host/src/auth/workspace_ctx.rs` (to-be-created), `crates/mnemra-host/src/auth/role.rs` (to-be-created), `crates/mnemra-host/src/abi/host_fns.rs` (extend), `justfile` (wire lint into `verify-lint`)
**Type:** backend
**Depends on:** Task 12
**Size:** L

**What:** Implement `WorkspaceCtx` (private field + accessor, single construction site, test-only constructor), the `Role` enum (`Admin` / `ReadObserver`), `WorkspaceCtx`-threading on all host-fns (builtins use the same threading — no internal bypass), and wire the WHERE-clause lint into `verify-lint` so CI fails on violation.

**Acceptance Criteria:**
- [ ] All Task 12 (RED) `WorkspaceCtx` + lint tests pass (`R-0006-a/b/c/f`, `R-0009-b`, `R-0018-d`).
- [ ] All read-path host-fns include `workspace_id = ctx.workspace_id` as a WHERE-clause condition derived from the argument, not a post-read filter (`R-0006-d`).
- [ ] Builtin components use the same `WorkspaceCtx` threading; no "internal" DB-query bypass without a `WorkspaceCtx` (`R-0006-e`).
- [ ] The `Role` enum is the binary `Admin` / `ReadObserver`; no other roles (`R-0009-a`).
- [ ] `Role` is derived from `admin_tokens.scopes` (`"admin"` / `"read_observer"`) at `WorkspaceCtx` construction (`R-0009-f`).
- [ ] The RLS column-shape (`workspace_id NOT NULL` on every artifact table) ships; RLS `CREATE POLICY` objects are NOT activated at V0 (`R-0006-g`, `R-0009-g`).
- [ ] `verify-lint` runs the WHERE-clause lint and fails the build on a missing read-path clause (`R-0018-d`, `R-0018-f`).

**Test Expectations:**
- The lint goes green on the real host-fns and red on the planted violation.
- Builtins exercise `WorkspaceCtx` threading (no bypass path compiles).

---

### Task 14: Role-based permission enforcement at the host-fn boundary

**Files:** `crates/mnemra-host/src/auth/permissions.rs` (to-be-created), `crates/mnemra-host/src/builtins/permissions.rs` (to-be-created)
**Type:** backend
**Depends on:** Task 13
**Size:** M

**What:** Implement the application-layer permission matrix (`Admin` authorizes all verb categories + CLI control-plane; `ReadObserver` authorizes only read-path MCP verbs) enforced at the host-fn boundary, and the `permissions` builtin (per-plugin grants checked at the host layer before plugin dispatch). No Postgres RLS policies at V0.

**Acceptance Criteria:**
- [ ] `Admin` authorizes all MCP verb categories, all CLI control-plane operations, and admin session management (`R-0009-c`).
- [ ] `ReadObserver` authorizes only read-path MCP verbs (`artifact.get`, `artifact.list`, projection queries); write verbs + CLI control-plane + workspace lifecycle are denied at the host-fn boundary with a structured permission error (`R-0009-d`, `R-0009-e`, Scenario: Read-observer token denied write access).
- [ ] The permission matrix is enforced at the application layer only; no `CREATE POLICY` activated at V0 (`R-0009-g`).
- [ ] The `permissions` builtin checks plugin verb access at the host layer before plugin dispatch (`R-0015-f`).

**Test Expectations:**
- `ReadObserver` + `*.create` → structured permission error; no artifact written; no cross-workspace exposure (Scenario: Read-observer token denied write access).
- `Admin` → all categories authorized.

---

### Task 15: Identity builtins — workspaces, users, agents, sessions, projects

**Files:** `crates/mnemra-host/src/builtins/workspaces.rs`, `users.rs`, `agents.rs`, `sessions.rs`, `projects.rs` (all to-be-created), `crates/mnemra-host/src/builtins/mod.rs` (init ordering)
**Type:** backend
**Depends on:** Task 13
**Size:** L

**What:** Implement the five remaining identity builtins (auth + permissions land in Tasks 11/14) and the deterministic builtin init ordering: all seven builtins initialize before any plugin loads. Mechanical CRUD over state tables — implementer self-tests.

**Acceptance Criteria:**
- [ ] `workspaces` builtin manages workspace lifecycle (create/delete/list); the `default` workspace is created on first-run and always exists after init; solo deployment collapses tenancy to `default` (`R-0015-a`, `R-0015-h`).
- [ ] `users` builtin manages user identity records referenced by agent + session state (`R-0015-b`).
- [ ] `agents` builtin manages agent registration tied to user-workspace pairs; agent identity derivation is canonical at registration and produces a structured error on mismatch (not silent registration) (`R-0015-c`).
- [ ] `sessions` builtin manages per-MCP-connection session state; session context is the source of `WorkspaceCtx` construction (`R-0015-e`, `R-0006-b`).
- [ ] `projects` builtin manages the project registry; project identity is a prerequisite for plugin scoping — no plugin is scoped to a project before that project's record exists (`R-0015-g`).
- [ ] All seven builtins (workspaces, users, agents, authentication, sessions, permissions, projects) initialize before any plugin is loaded; no plugin invocation precedes builtin startup completion (`R-0002-b`, `R-0002-c`).

**Test Expectations:**
- Builtin init-order test: a plugin-load attempt before builtin completion is rejected.
- Agent identity mismatch → structured error.
- Builtins execute as host code, not inside the Wasmtime sandbox (`R-0002-b`).

---

### Task 16 (RED): Signing-chain tests — synchronous verify, provenance-not-field, fail-shut

**Files:** `crates/mnemra-host/tests/signing_chain.rs` (to-be-created)
**Type:** test
**Depends on:** Task 4
**Size:** M

**What:** Failing tests pinning the signing-chain invariants: synchronous verify-on-load, structured rejection on failure, `core` status determined by signature provenance (not manifest-field trust), embedded verification material, and the startup file-mode invariant.

**Acceptance Criteria (red):**
- [ ] Tests assert signature verification is synchronous on load; no instance is created until `verify()` returns `Ok`; no verify-async / defer-to-background path exists (`R-0005-a`).
- [ ] Tests assert a failed verification (malformed sig, unknown key, chain break) rejects the load with a structured error naming the plugin's `name` + `version`; no best-effort load (`R-0005-b`, Scenario: Signing verification failure).
- [ ] Tests assert the root verification material is embedded at build time; no runtime key-fetch path (`R-0005-d`).
- [ ] Tests assert `core` status is determined by signature provenance, NOT manifest-field trust: `core = true` is honored only when the signature chains to the mnemra root; `core = true` signed by any other key is rejected at load (`R-0005-h`, `R-0005-g`).
- [ ] Tests assert at startup that the admin-token file and signing-verification-material file are both mode 600 / not world-readable; the host refuses to start if either check fails (`R-0005-f`).
- [ ] Tests assert any plugin manifest with `core = false` is rejected at load (non-core install is V0.1+) (`R-0003-e`).

**Test Expectations:**
- Red-phase: `verify` empty by design until Task 17. Rationale recorded.
- No hardcoded key material in tests; generate test keypairs per-run.

---

### Task 17 (GREEN): Signing-chain verification + embedded root material + startup file-mode invariant

**Files:** `crates/mnemra-host/src/signing/verify.rs` (to-be-created), `crates/mnemra-host/src/signing/root_material.rs` (to-be-created), `crates/mnemra-host/src/startup/file_mode_check.rs` (to-be-created)
**Type:** backend
**Depends on:** Task 16
**Size:** L

**What:** Implement synchronous signature verification (ed25519 per P-0003 manifest `[signature]`), embed the root verification material at build time, determine `core` by provenance, and enforce the startup file-mode invariant (token + signing material both mode 600). Signing-key custody (build-host filesystem mode 600, not on the deployment node) is a build-pipeline + ops invariant — see Task 26 (`verify-build`) and the **Spec gaps surfaced** note on R-0005-c.

**Acceptance Criteria:**
- [ ] All Task 16 (RED) signing tests pass (`R-0005-a/b/d/f/g/h`, `R-0003-e`).
- [ ] `verify()` is synchronous; load is rejected before instance creation on failure (`R-0005-a`, `R-0005-b`).
- [ ] `core` is honored only when the signature chains to the embedded mnemra root material; the binding is structural and not relaxed when V0.1+ non-core install opens (`R-0005-h`).
- [ ] The host refuses to start if the token file or signing-material file is not mode 600 / is world-readable (`R-0005-f`).

**Test Expectations:**
- Scenario: Signing verification failure — invalid sig → structured error naming plugin, no instance, host continues startup, no verbs exposed for the rejected plugin.
- Scenario: Build-host key-on-disk leak trip-wire — see **Spec gaps / intentional gaps** (trip-wire fires on a future deployment event, not a V0 build task).

> **Review gate B (security review):** the security layer (Tasks 11, 13, 14, 16/17, and Task 18 hostname allowlist) completes here, BEFORE MCP dispatch (Task 22) and plugin-load (Tasks 20/21) build on it. See Sequencing.

---

### Task 18 (RED+GREEN): LLM-API-key config + outbound hostname allowlist

**Files:** `crates/mnemra-host/tests/llm_key_allowlist.rs` (to-be-created), `crates/mnemra-host/src/config/llm_key.rs` (to-be-created), `crates/mnemra-host/src/net/hostname_allowlist.rs` (to-be-created)
**Type:** backend
**Depends on:** Task 17
**Size:** M

**What:** Implement the per-deployment LLM-API-key config surface (deploy-time, mode-600 file, separate from the admin token) and the outbound hostname allowlist for embedding calls (block any hostname not on the list). TDD pair on the allowlist (security boundary); the key-config file is mechanical. The system hosts no model.

**Acceptance Criteria:**
- [ ] The LLM-API-key is per-deployment, deploy-time configurable, never hard-coded (`R-0014-a`).
- [ ] Outbound embedding calls enforce a per-deployment hostname allowlist; a call to a non-allowlisted hostname is blocked (`R-0014-b`).
- [ ] The system hosts no language model; it does not accept an API key for a hosted model endpoint (`R-0014-c`).
- [ ] The LLM-API-key file is mode 600, separate from the admin-token file; the startup file-mode invariant check covers both files (`R-0014-d`, extends `R-0005-f`).

**Test Expectations:**
- Allowlist: an outbound call to an off-list hostname is blocked; an on-list call proceeds (red/green pair).
- Startup mode-check fails if the LLM-key file is not mode 600.

---

### Task 19: Reference/fixture plugin manifest + fixture content-type table

**Files:** `plugins/mnemra-echo/manifest.toml` (to-be-created), `wit/host.wit` (reference plugin imports), `plugins/mnemra-echo/mnemra_echo.rs` (extend to call `artifact.*` host-fns), `crates/mnemra-host/src/schema/fixtures/` (fixture content-type table)
**Type:** full-stack
**Depends on:** Task 9, Task 17
**Size:** M

**What:** Evolve `mnemra-echo` into the signed reference/fixture `core: true` plugin: author its `manifest.toml` (schema_version 1, core true, `[verbs]`, `[content_types]`, `[state_scopes]`, `[host_fns]`, `[signature]`), declare a fixture content-type table, and wire the plugin to call the real `artifact.*` host-fns. This is the concrete plugin that Scenarios 2/3/7/13/15 run against at V0 (see Reading note 2). **Maintainer ratifies the fixture substitution at plan-exit.**

**Acceptance Criteria:**
- [ ] The reference plugin ships `manifest.toml` declaring `schema_version = 1`, `core = true`, `name`, `version`, and the `[verbs]`, `[content_types]`, `[state_scopes]`, `[host_fns]`, `[signature]` sections (`R-0003-a`).
- [ ] The manifest does NOT declare `workspace_id` as a parameter on any write-path host-fn (`R-0003-d`).
- [ ] `artifact.delete` appears in `host_fns.required` only if the plugin opts in; it is not granted by default (`R-0003-g`).
- [ ] The fixture content-type table is created via the Task 9 machinery (per-type table, CHECK constraints, expression indexes, history shadow) — exercising R-0001-d/e/f against a fixture, not a capability family (Reading note 3).
- [ ] The plugin's `[signature]` is signed by the mnemra root on the build host (`R-0002-a`, `R-0005-c` build-pipeline).

**Test Expectations:**
- The fixture plugin's manifest loads, signs, and its content-type table exercises `artifact.create`/`get`/`list`/`update`/`delete` and the WHERE-clause path.
- Manifest omitting `artifact.delete` from `required` → `delete` unavailable to the plugin.

---

### Task 20 (RED): Plugin-manifest load pipeline + host-fn allowlist tests

**Files:** `crates/mnemra-host/tests/manifest_load.rs` (to-be-created)
**Type:** test
**Depends on:** Task 17, Task 19
**Size:** M

**What:** Failing tests pinning the manifest load pipeline: `schema_version` branch, per-instance host-fn allowlist compiled from the signed manifest (undeclared calls fail at the WIT boundary), output validation against the WIT schema (fail-shut, size caps), and the per-verb capability check.

**Acceptance Criteria (red):**
- [ ] Tests assert the runtime compiles a per-instance host-fn allowlist from the signed manifest's `[host_fns]` before any instance is created; calls outside the allowlist fail at the WIT boundary, not at the host-fn body (`R-0003-b`).
- [ ] Tests assert manifest loading branches on `schema_version`: `schema_version: 1` loads against a newer runtime; an unknown `schema_version` produces a structured load error (`R-0003-c`, `R-0017-b`).
- [ ] Tests assert plugin output is validated against the WIT-declared schema; per-field size caps enforced; the parser fails shut on schema mismatch rather than truncating (`R-0003-f`).
- [ ] Tests assert the MCP handler enforces a per-verb capability check against the manifest's declared `verbs` list before dispatching to the plugin runtime (`R-0010-d`).

**Test Expectations:**
- Red-phase: `verify` empty by design until Task 21. Rationale recorded.

---

### Task 21 (GREEN): Plugin runtime — manifest load, allowlist compile, pool, resource limits

**Files:** `crates/mnemra-host/src/plugin/runtime.rs` (to-be-created), `crates/mnemra-host/src/plugin/manifest.rs` (to-be-created), `crates/mnemra-host/src/plugin/allowlist.rs` (to-be-created), `crates/mnemra-host/src/plugin/pool.rs` (to-be-created), `crates/mnemra-host/src/plugin/limits.rs` (to-be-created), `crates/mnemra-host/src/plugin/epoch_thread.rs` (to-be-created)
**Type:** backend
**Depends on:** Task 20
**Size:** L

**What:** Implement the plugin runtime: manifest load + `schema_version` branch, per-instance allowlist compilation, fail-shut output validation, the 3–5-instance-per-type pool (initialized before the MCP server accepts requests), and Wasmtime resource limits (fuel 10B, epoch 5s via `set_epoch_deadline(500)` + 10ms host epoch-tick thread, 64 MiB memory). Designate the four `core: true` plugin slots as structurally non-uninstallable.

**Acceptance Criteria:**
- [ ] All Task 20 (RED) load/allowlist tests pass (`R-0003-b/c/f`, `R-0010-d`, `R-0017-b`).
- [ ] The plugin pool holds 3–5 instances per plugin type, initialized at host startup before the MCP server accepts requests; instances are tenant-stateless; no cross-call state held (`R-0016-a`, `R-0016-b`, `R-0007-d`).
- [ ] Wasmtime fuel metering (`Store::set_fuel`, 10B ticks) AND epoch-interruption (`set_epoch_deadline(500)`, 5s, 10ms host epoch-tick thread) are BOTH active simultaneously; memory ceiling 64 MiB via `static_memory_maximum_size` or `ResourceLimiter` (`R-0007-a`, `R-0007-b`, `R-0007-c`, `R-0007-g`).
- [ ] The host epoch-tick thread starts before any plugin is invoked, is supervised, is NOT restarted silently on crash; on crash it emits `epoch_tick_thread_died`, refuses new plugin invocations until restart is confirmed, attempts one supervised restart/min with backoff, and `/health` `overall` reflects `"degraded"` while dead (`R-0007-h`).
- [ ] The four `core: true` plugins are signed by the mnemra root, structurally non-uninstallable at runtime; the only removal path is a binary rebuild (`R-0002-a`, `R-0002-d`).
- [ ] The Wasmtime crate version is pinned in `Cargo.toml` (no wildcard / open constraint); recorded in the `verify-build` SBOM; a major/minor upgrade requires explicit approval, not dependabot auto-merge (`R-0007-i`).

**Test Expectations:**
- Scenario: Plugin signing chain operates on load — valid manifest → `verify() Ok` → allowlist compiled → 3–5 instances ready.
- Pool init before MCP accept; tenant-stateless instances.
- Epoch-tick-thread-death path: `epoch_tick_thread_died` event + `degraded` health + supervised restart.

---

### Task 22 (RED+GREEN): Resource-limit trap handling — kill-and-replace (fuel + epoch)

**Files:** `crates/mnemra-host/tests/resource_limits.rs` (to-be-created), `crates/mnemra-host/src/plugin/trap_recovery.rs` (to-be-created)
**Type:** backend
**Depends on:** Task 21
**Size:** M

**What:** Implement trap-to-kill-and-replace recovery: catch the Wasmtime trap (epoch deadline or fuel exhaustion), emit a structured `plugin_limit_violation` event, poison + synchronously replace the pool slot, return a structured error to the caller, and never propagate the trap as a host-process panic. TDD pair (security/reliability boundary).

**Acceptance Criteria:**
- [ ] On any resource-limit violation (fuel exhaustion, epoch deadline, memory ceiling): catch the trap; emit a structured event with `(workspace_id, plugin_id, plugin_version, limit_type, limit_value)`; poison the pool slot and replace with a new instance; return a structured error for the current invocation (`R-0007-e`).
- [ ] A Wasmtime trap is NOT propagated as a host-process panic; kill-and-replace is the recovery invariant (`R-0007-f`).
- [ ] The replaced instance is added synchronously before the verb-invocation error is returned; the pool size does not decrease as a result of a kill (`R-0016-c`).
- [ ] Epoch breach: event `plugin_limit_violation` with `limit_type: "epoch_deadline"`, `limit_value: 500`; caller receives `{ code: "plugin_execution_timeout", ... }`; host does not panic (Scenario: Resource limit breach).
- [ ] Fuel exhaustion (independent of epoch): `limit_type: "fuel"`, `limit_value: 10000000000`; same kill-and-replace path; tested with a module that consumes fuel without triggering the epoch deadline (Scenario: Plugin fuel exhaustion mid-verb).

**Test Expectations:**
- Epoch breach (infinite-loop module) and fuel exhaustion (CPU-burn-no-sleep module) tested **independently**; both fire kill-and-replace; host never panics.
- Pool recovers to full size after a kill.

> **Review gate C (security review):** sandbox + resource-limit recovery (Tasks 21, 22) reviewed after this layer. See Sequencing.

---

### Task 23: MCP server — stdio transport, auth-check, dispatch, error codes

**Files:** `crates/mnemra-host/src/mcp/server.rs` (to-be-created), `crates/mnemra-host/src/mcp/dispatch.rs` (to-be-created), `crates/mnemra-host/src/mcp/errors.rs` (to-be-created)
**Type:** backend
**Depends on:** Task 14, Task 17, Task 22
**Size:** L

**What:** Implement the single MCP server (spec 2025-06-18, stdio transport): per-request `DF-auth-check` before any routing, namespaced verb dispatch (`<plugin>.<verb>`), the per-verb capability check, `WorkspaceCtx` construction at the single dispatch-path site after token validation, and distinguishable JSON-RPC error codes. Control-plane operations are NOT exposed as MCP verbs.

**Acceptance Criteria:**
- [ ] A single MCP server runs MCP spec 2025-06-18 over stdio; all verbs from all loaded plugins are served from it; verbs are namespaced `<plugin>.<verb>` (`R-0010-a`, `R-0010-b`).
- [ ] The MCP handler performs `DF-auth-check` (P-builtin-auth token verification) on every request before routing to any handler (`R-0010-c`).
- [ ] `WorkspaceCtx` is constructed at the single dispatch-path site after token validation and passed as the first parameter to host-fns (`R-0006-b`, Scenario: MCP verb dispatches under WorkspaceCtx).
- [ ] Distinguishable JSON-RPC error codes for invalid token (auth failure, NOT -32600/-32601/-32602), verb-not-found (-32601), parameter-invalid (-32602); classes not conflated; plugin-execution-timeout + permission-denied custom codes (`R-0010-f`, API Contract error table, Scenario: Admin token mismatch — 401).
- [ ] Streamable-HTTP transport is NOT activated (V0.1+) (`R-0010-e`).
- [ ] Control-plane operations are NOT exposed as MCP verbs; agent-facing CRUD routes through MCP only (`R-0010-g`).

**Test Expectations:**
- Scenario: MCP verb dispatches under WorkspaceCtx — valid token → `WorkspaceCtx` constructed → `artifact.create` host-fn → WHERE-scoped insert → ULID returned → per-verb metric emitted.
- Scenario: Admin token mismatch — bogus token → distinguishable auth-failure code, no `WorkspaceCtx`, no host-fn invoked, no data accessed.
- Scenario: Read-observer denied write — re-validated end-to-end over MCP.

---

### Task 24: Admin CLI — schema-driven subcommands, control-plane ops, token auth

**Files:** `cmd/mnemra/src/cli/mod.rs` (to-be-created), `cmd/mnemra/src/cli/generate.rs` (to-be-created), `cmd/mnemra/src/cli/control_plane.rs` (to-be-created)
**Type:** backend
**Depends on:** Task 11, Task 15, Task 21
**Size:** L

**What:** Implement the admin CLI: subcommands schema-driven from plugin manifests at startup (a new manifest verb → new subcommand, no CLI code change); destructive/control-plane operations only (no agent-facing CRUD); admin-token authentication required for destructive ops (UID match insufficient); the fixed control-plane verb set.

**Acceptance Criteria:**
- [ ] CLI subcommands are dynamically generated from plugin manifests at startup; a new manifest verb produces a new subcommand without a CLI code change; removing the verb + restarting removes the subcommand (`R-0011-a`, Scenario: Admin CLI schema-driven subcommand generation).
- [ ] The CLI handles destructive/control-plane operations only; agent-facing CRUD is not exposed on the CLI (`R-0011-b`).
- [ ] Destructive CLI operations require admin-token authentication; UNIX UID match alone is insufficient (`R-0011-c`).
- [ ] The CLI exposes `workspace create`, `workspace delete`, `workspace list`, `token rotate`, `migrate` (one-shot trigger), `backup` (trigger), `health` (human-readable `/health` wrapper) (`R-0011-d`).
- [ ] Workspace lifecycle (`workspace create`/`delete`) requires `Role::Admin`; a `ReadObserver` request returns a structured permission error (`R-0009-e`).

**Test Expectations:**
- Scenario: Admin CLI schema-driven generation — fixture plugin verb (`echo.audit` substituting `task.audit`) yields a subcommand; removal removes it.
- Scenario: Admin token authenticates a destructive operation — `mnemra workspace list --token T_ADMIN` → hash lookup → `WorkspaceCtx` → scoped list.
- Negative: destructive op without a valid token → rejected (UID alone insufficient).

---

### Task 25: Observability emission + `/health` endpoint (loopback-only)

**Files:** `crates/mnemra-host/src/observability/emit.rs` (to-be-created), `crates/mnemra-host/src/observability/redaction.rs` (to-be-created), `crates/mnemra-host/src/health/listener.rs` (to-be-created), `crates/mnemra-host/src/health/handler.rs` (to-be-created)
**Type:** backend
**Depends on:** Task 6, Task 13
**Size:** L

**What:** Implement storage-independent observability emission (per-verb metrics + events + structured stdout logs, OTel-exportable; no in-app observability store) and the loopback-only `/health` endpoint (first API, before config load, `127.0.0.1`-bound, `MNEMRA_HEALTH_PORT` default 8877, detail body to every caller it serves). Redaction at the emission boundary. The `metrics.record` / `log.emit` / `event.emit` / `projection.emit` host-fn bodies wire to emission here.

**Acceptance Criteria:**
- [ ] Per-verb metric record emitted on every MCP verb dispatch with `workspace_id`, `verb`, `outcome` (`ok`/`error`/`timeout`), `duration_ms`, `recorded_at`, as OTel metric/structured form exportable to a configurable OTLP endpoint; no artifact IDs / content fragments / agent identity in the metric; never written to an in-app hypertable/table (`R-0004-a`, `R-0004-f`, data model emission record).
- [ ] Event record emitted with `workspace_id`, `event_type`, `event_version` (DEFAULT 1), `token_id` (host-derived from `WorkspaceCtx`, not plugin-supplied; a plugin-supplied `token_id` is rejected at the WIT boundary), `agent_id`, `session_id`, `payload` (never artifact bodies), `recorded_at`; structured/OTel, not an in-app hypertable (`R-0004-b`, `R-0009-b`, data model emission record).
- [ ] Structured log records emitted to stdout (one JSON line per record) with level, message, timestamp, `workspace_id` on every tenant-scoped record; no in-app `logs` table, no in-app log-retention worker (`R-0004-d`).
- [ ] No in-app observability storage initialized: no metrics/events hypertable, no `add_retention_policy`, no continuous aggregate; emission succeeds and is observable even with no persistent backend configured (`R-0004-c`, `R-0004-e`, `R-0004-f`).
- [ ] No artifact bodies / content fragments / raw query strings in any emitted metric, event, or log; redaction enforced at the emission boundary for high-entropy strings (`R-0004-h`).
- [ ] `event_version` increments on breaking event-type schema changes; backward-compatible additions do not increment it (`R-0004-i`).
- [ ] `GET /health` is the first API, started before config load and before the MCP server accepts requests, on a dedicated loopback-only TCP listener bound to `127.0.0.1` (not `0.0.0.0`/`::`), port via `MNEMRA_HEALTH_PORT` (default 8877), serving only `GET /health` (`R-0004-g`).
- [ ] `/health` returns the structured detail body `{ "postgres", "pgvector", "workspace_default", "overall" }` to every caller it serves (loopback IS the gate; no admin-token gating at V0); `200` healthy / `503` when a dependency is down; `down` mid-shutdown (`R-0004-g`, Scenario: `/health` degraded and down states, API Contract health endpoint).

**Test Expectations:**
- Scenario: Observability emits during a dogfood session — 50 verb calls → 50 observable metric emissions; p50/p95/p99 derivable from `duration_ms`; `\d+` no hypertable; `\dx` no `timescaledb`; emission succeeds with no backend.
- Scenario: `/health` degraded and down — Postgres unreachable → `degraded`/`down` + `503`; mid-shutdown → `down` + `503`.
- Telemetry non-leak: an audit over emitted records finds zero artifact-body matches (redaction).
- **Test infra note:** `/health` integration tests need a loopback TCP bind — see Test surface (sandbox loopback-bind dependency).

---

### Task 26: `verify-build` signed-binary pipeline + SBOM + ABI-recompile gate

**Files:** `justfile` (`verify-build`), `.github/workflows/ci.yml`, `build/sign.rs` or `xtask` (to-be-created), `Cargo.toml` (Wasmtime pin)
**Type:** infra
**Depends on:** Task 17, Task 21
**Size:** M

**What:** Implement the `verify-build` recipe producing the signed binary with embedded root verification material, the SBOM recording the pinned Wasmtime version, and the pre-1.0 ABI-recompile gate (an ABI-change PR recompiles all `core: true` plugins and passes their tests before merge).

**Acceptance Criteria:**
- [ ] `verify-build` produces the signed binary with the embedded root verification material (`R-0018-f`, `R-0005-d`).
- [ ] The SBOM records the pinned Wasmtime version (`R-0007-i`).
- [ ] An ABI-change PR causes all `core: true` plugins to recompile against the new ABI and pass their tests before merge (`R-0017-a`).
- [ ] The signing key resides on the build host's filesystem at mode 600, owner = build-pipeline UID, NOT co-located on the deployment node; the deployment node receives only signed artifacts + verification material (`R-0005-c` — build-pipeline/ops invariant; see Spec gaps note).

**Test Expectations:**
- `verify-build` emits `GATE verify-build PASS` and produces a signed binary; signature verifies against the embedded material.
- ABI-recompile gate: a stub ABI change triggers reference-plugin recompile + test.

---

### Task 27: Acceptance smoke — `verify-smoke` end-to-end init + dispatch

**Files:** `justfile` (`verify-smoke`), `crates/mnemra-host/tests/smoke_e2e.rs` (to-be-created)
**Type:** test
**Depends on:** Task 23, Task 24, Task 25, Task 26
**Size:** M

**What:** Implement the `verify-smoke` end-to-end acceptance test: `mnemra init` → builtins up → reference plugin signed/loaded/pooled → MCP dispatch round-trip → metric emitted → `/health` ok. This is the integration seam that proves the substrate holds together; it is the sealed acceptance test for the V0 increment.

**Acceptance Criteria:**
- [ ] `verify-smoke` runs `mnemra init` against a fresh embedded engine and asserts the Single-node V0 substrate initialization scenario end-to-end (tables, `pgvector`, `default` workspace, seven builtins, pool init, synchronous signature verify, health event, `/health` detail body `overall: "ok"`) (Scenario: Single-node V0 substrate initialization, `R-0013-a`).
- [ ] A round-trip MCP verb dispatch against the reference plugin succeeds and emits a per-verb metric (Scenario: MCP verb dispatches under WorkspaceCtx).
- [ ] Integration tests run against the real surface (HTTP `/health` handler, actual embedded Postgres + `pgvector`), not mocks (`R-0018-b`).

**Test Expectations:**
- Full init → dispatch → emit → health-ok path green against the real embedded engine + signed reference plugin.

---

## Gate A amendments (2026-06-13, maintainer-ratified)

1. **Task 6b (this change, A-04/A-05):** interim SHA-256 hash-pin of the embedded PG + pgvector archives, verified at engine bring-up, fail-shut. Task 26 retains full provenance/SBOM/signature scope; the named gap (TOCTOU window + per-platform pin maintenance) carries to Task 26.
2. **A-15 → Task 7:** a structured `StorageError::EngineUnavailable` degradation seam threads into Task 7's init/schema work (engine crash must not surface as opaque driver errors); Task 25's `/health` "degraded" state consumes it.
3. **A-18 → Task 7 (R-0013-d ruling):** V0 migrations are forward-only; the migration runner **structurally refuses** destructive operations (no destructive path exists), satisfying R-0013-d at V0. A real backup mechanism fires on the first genuine destructive-migration need — named tripwire, not V0 scope.
4. **A-16/A-17 confirmations:** `WorkspaceId` u64→UUID widening and the four R-0013-e least-privilege roles land at Task 7 as already planned.

## Sequencing

**Layer-up order (greenfield substrate — data/ABI up, each layer green-on-merge):**

```
CI scaffold (1) → runtime scaffold (2)
  → [ABI layer]      WIT host-fn ABI: 3(RED) → 4(GREEN)
  → [storage layer]  Storage trait + in-mem (5) → embedded PG adapter (6) → schema init (7)
                       → content-schema: 8(RED) → 9(GREEN)
  → [auth/security]  admin token: 10(RED) → 11(GREEN)
                       → WorkspaceCtx + WHERE-lint: 12(RED) → 13(GREEN)
                       → permissions (14) ; identity builtins (15)
                       → signing: 16(RED) → 17(GREEN) ; LLM key + allowlist (18)
  → [plugin layer]   reference plugin (19) → manifest-load: 20(RED) → 21(GREEN)
                       → trap/kill-replace: 22(RED+GREEN)
  → [surface layer]  MCP server (23) ; admin CLI (24) ; observability + /health (25)
  → [build/accept]   verify-build signed pipeline (26) → verify-smoke E2E (27)
```

**Parallel-safe groups** (no shared file targets, no hard dependency between members):
- After Task 2: **{Task 3/4 ABI}**, **{Task 5 Storage trait}** can start in parallel (different crates/files).
- After Task 7: **{Task 8/9 content schema}**, **{Task 10/11 admin token}** in parallel.
- After Task 13: **{Task 14 permissions}**, **{Task 15 identity builtins}**, **{Task 16/17 signing}** in parallel.
- After Task 21: **{Task 23 MCP}**, **{Task 24 CLI}**, **{Task 25 observability/health}** in parallel (each depends on distinct lower layers; Task 23 also waits on Task 22).

**Strict sequences (highest rework-if-reordered first):**
- Task 1 → everything (CI gate defines "green on main").
- Task 4 (host-fn ABI) → Task 12/13 (`WorkspaceCtx` threading) → Task 23 (MCP dispatch). The WHERE-lint (Task 12/13) lands **with/just after the first host-fn** and before host-fns proliferate — this is the highest-rework tenant-isolation ordering.
- Task 5 → Task 6 → Task 7 → Task 9 (storage substrate before any artifact-table machinery).
- Task 17 (signing) → Task 20/21 (plugin load) — no plugin loads before the signing chain verifies.
- Every RED task immediately precedes its GREEN partner.

**Review gates (placed at layer boundaries — minimum holding cost):**
- **Gate A (operational review):** after Task 6 (embedded-Postgres bring-up), before Task 7 schema init builds on it.
- **Gate B (security review):** after the security layer completes (Tasks 11, 13, 14, 16/17, 18), **before** Task 20/21 (plugin load) and Task 23 (MCP dispatch) build on it. This is the load-bearing gate — the signing/auth/tenant surface is reviewed before downstream layers depend on it.
- **Gate C (security review):** after the plugin sandbox + resource-limit recovery (Tasks 21, 22).
- **Gate D (operational review):** around the control-plane + build pipeline (Tasks 24, 26) and `/health` (Task 25) before the `verify-smoke` seal (Task 27).
- Per the workspace iterate-to-zero discipline, each gate runs up to 3 rounds of routine-finding fixes before escalation; architectural concerns surface as deviation reports, not auto-fired re-rounds.

## Test surface

Cross-task test infrastructure (per-task Test Expectations are scoped above):

- **Unit tests:** `crates/mnemra-host/src/**` inline `#[cfg(test)]` modules — per-component logic (token hashing, allowlist compilation, role derivation, redaction).
- **Integration tests:** `crates/mnemra-host/tests/*.rs` — run against the **real** embedded Postgres + bundled `pgvector` and real HTTP `/health` handler, **not mocks** (`R-0018-b`). The `postgresql_embedded` engine runs in-process; integration tests bring it up per-suite. This is named test infra, not a per-task concern.
- **Contract tests:** the two-adapter `Storage` contract (`tests/storage_contract.rs`) runs the same suite against in-memory + Postgres adapters; the host-fn ABI contract suite (`tests/abi_contract.rs`) pins the WIT structural invariants.
- **Lint test:** `cargo test --test lint_workspace_clause` (`syn` AST over host-fn source) — wired into `verify-lint`; a planted read-path violation must return non-zero and name the offending function (`R-0018-d`).
- **Smoke / E2E:** `just verify-smoke` — `mnemra init` → dispatch → emit → `/health` ok against the real engine + signed reference plugin (Task 27).
- **Known test-infra dependency — loopback bind:** `/health` integration tests (Task 25, Task 27) require a loopback TCP bind (`127.0.0.1`). In a sandboxed CI/test environment this needs local-binding enabled (the workspace has hit this exact constraint before on AI integration tests requiring `sandbox.network.allowLocalBinding=true`). Flag at implementation time so it does not surface mid-task.
- **No hardcoded secrets in tests:** admin tokens and signing keypairs are generated per-run, never literal (`R-0008-a` test discipline; workspace no-hardcoded-test-passwords rule).

## Release binding

- **Target release:** `0.1.0` (per spec title).
- **Branch posture:** trunk-based; one task = one squash-merge to `main`, green on its own (`R-0018-c`, `R-0018-f`). Short-lived worktree branches; main is protected from direct pushes.
- **CI gate:** `just ci` is the sole CI entry point; it invokes every `verify-*` recipe, each emitting `GATE <name> <PASS|FAIL>` (`R-0018-f`). Every task's squash-merge requires `just ci` green.
- **Release notes target:** `0.1.0` release notes generated from the conventional-commit log between the prior tag and the `0.1.0` tag (workspace `/release-notes` convention) at increment close.

## Archival

When the feature lands `live`:
- Mark this plan `archived` at the top.
- Move to `docs/plans/archive/2026-06/` (this repo's first plan; `docs/plans/archive/` is to-be-created at archival).
- Note the live-tier commit SHA / `0.1.0` release tag in the archive entry.
- The spec remains in `docs/specs/` (durable, designed-tier).

## Spec gaps surfaced

Genuine spec-gap findings (an AC not binary-observable, an incomplete scenario data model) are halt-and-escalate items, distinct from the intentional gaps below. **None block decomposition** — each is a flag for the maintainer at the plan-exit gate.

1. **Reference/fixture plugin is a decomposition-forced substitution, not a spec-named artifact (escalate for ratification).** The spec's Scenarios 2/3/7/13/15 are written against the `tasks` plugin / `tasks/manifest.toml` / `task.create` / `task.audit`, but the `tasks` plugin **is** the `0.2.0` capability family the spec's Out of Scope explicitly excludes. The substrate is untestable without *some* signed `core: true` plugin to load/sign/pool/limit/dispatch against. This plan resolves the tension by evolving `mnemra-echo` into a signed reference/fixture plugin (Task 19) and running the `tasks`-named scenarios against it at V0 (Reading note 2). **This is a substitution the maintainer should ratify at plan-exit** — it is the one site where the plan reconciles a scenario against an out-of-scope boundary. If the maintainer prefers a different fixture name or a throwaway plugin distinct from `mnemra-echo`, only task names change, not the task structure. (Not a spec defect — the spec correctly scopes `tasks` out; the gap is that no in-scope fixture plugin is named for the substrate's own acceptance tests.)

2. **R-0005-c (signing-key build-host custody) is an ops/build-pipeline invariant with no V0 code surface.** "The signing key SHALL reside on the build host's filesystem at mode 600 … SHALL NOT be co-located on the deployment node" governs the build pipeline and deployment topology, not a mnemra-core runtime code path. It is carried as an AC on Task 26 (`verify-build` / build pipeline) as the nearest enforcement surface, but its full enforcement is operational (CI secret handling, deployment packaging) and partly outside this repo's source. Flagged so it is not mistaken for a runtime feature; deeper custody hardening is the deliberately-deferred `{{P-SigningKeyCustodyHardening}}` (see Intentional gaps).

### Intentional gaps — transcribed from the spec; NOT defects, NOT plan tasks

These are deliberately-unresolved per the spec and its ADRs. They do **not** become plan tasks by inference.

- **`{{P-SigningKeyCustodyHardening}}` (Tier-C ADR, not yet authored).** Fires on the **multi-deployment trip-wire** — when mnemra-core is deployed to a second node beyond the maintainer's single dogfood instance (`R-0005-e`, Scenario: Build-host key-on-disk leak). The accepted risk `R-0004` is not retired until this ADR locks. This is a future tripwire, not V0 work; the `/health` non-loopback admin-token-gating tripwire (`R-0004-g`) and the streamable-HTTP microVM tripwire (`R-0010-e`) are likewise future-fired, not V0 tasks.
- **Status-churn write-amplification numeric budget** — the C1 write-amplification weakness under high status-flip rates is known; a numeric model and potential C2-influenced partial evolution are deferred to V0.1+ when dogfood data accumulates (spec Out of Scope). Not a plan task.
- **RLS policy enforcement (`CREATE POLICY`)** — the `workspace_id NOT NULL` column-shape ships at V0 (Task 13); the RLS policy objects do not (`R-0006-g`, `R-0009-g`, `R-0009-h`). V0.1+ additive `CREATE POLICY` migration; not a V0 task.
- **V0.1+ non-breaking column additions** — `embedding vector(1536)`, `search_tsv tsvector` are additive `ADD COLUMN` at V0.1+ (`R-0001-g`, data model); the V0 AC covers the single `pgvector` smoke test only. Not V0 tasks.
- **Increment-to-plugin mapping ratification, capability families (`0.2.0`–`0.14.0`), migration mechanics, OIDC AS federation, third-party plugin install, hot-reload, adaptive pool sizing, saga coordination, UI, `get_context_for`, ongoing ingest** — all explicitly Out of Scope (spec Out of Scope, 21 items). The substrate provides the ABI/storage/auth/runtime; the families and operational hardening are later increments.
