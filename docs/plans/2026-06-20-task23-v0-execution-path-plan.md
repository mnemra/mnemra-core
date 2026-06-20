# Plan: Task 23 — V0 artifact.create execution path (walking-skeleton-first)

> **Spec:** `docs/specs/2026-05-24-mnemra-core-v0-substrate.md` (locked 2026-05-24; this plan decomposes against R-0019 at SHA `fbf90b1dcc93865124306eb0cb8132820f67c584`)
> **Date:** 2026-06-20
> **Status:** drafted
> **fork_status:** resolved-B2 (storage-gating fork resolved 2026-06-20 → Branch 2; ready to land)
> **Target release:** `0.1.0`
> **Locked ADRs:** P-0013 (plugin invocation / export ABI), P-0012 (plugin runtime + MCP SDK)

## Purpose

Stand up the **component-host invocation path** so an authenticated MCP `call_tool` request reaches a real typed plugin export and returns a result — implementing the artifact.create execution path (Scenario "MCP verb dispatches to a typed content export") against the locked typed-per-verb export ABI (`R-0019`, anchoring P-0013).

This is a follow-on to the V0-substrate plan's Task 23 (MCP server): the auth/routing/error MCP front and the `is_write_verb` fail-closed guard are merged, but **no invocation path exists yet** — the pool loads core `Module`s with no imports (`libs/mnemra-host/plugin/pool.rs:293`), there is no request-path invoke, the guest `run()` is pure echo (`plugins/mnemra-echo/mnemra_echo.rs:40-50`), and `call_tool` returns `Ok(CallToolResult::default())` (`libs/mnemra-host/mcp/server.rs:168`).

Decomposed **walking-skeleton-first**: slice 1 = `echo.create` end-to-end (MCP → auth → pool invoke → guest `content.create` export → host `artifact-create` import → typed ULID return); subsequent slices add the other CRUD verbs, two reliability carries, startup wiring, and the real-storage swap. This is a **slice-through of an existing, partly-built host** (refactor-favors-slice-through), not a greenfield layer-up — keep `main` green between slices.

## Reading notes (orientation for the implementer — not new requirements)

1. **The host has zero component-model wiring today.** The guest builds as a *component* (`wasm32-wasip2`) but the pool loads core `wasmtime::Module`s with no imports (`pool.rs:293`); the trap-recovery path uses a core-module, parameterless `get_typed_func::<(), ()>("run")` fixture (`libs/mnemra-host/plugin/trap_recovery.rs:394`); the host-fn import bodies are `todo!()` stubs (`libs/mnemra-host/abi/host_fns.rs`); and `call_tool` stops at auth and returns a default result (`libs/mnemra-host/mcp/server.rs:164,168`; "future task" note at `:22-29`). **Standing up the component-host invocation path is the content of this plan** — and it is owed regardless of the export shape (a cost assessment proved the "WIT churn / recompile" framing overstated: the typed-export delta is light; the component-host machinery is the bulk).

2. **Two cost buckets (sizing discipline).** Bucket A — the ABI-agnostic component-host machinery (component Linker + host-fn import bodies + component instantiation in the pool + `call_tool` routing) — is the bulk (T1, T3–T6, T11). Bucket B — the typed-per-verb delta (≈5 WIT lines, the guest export functions, generated host accessors, per-verb result mapping) — is light (T1, T2). The export-shape decision (R-0019) sizes Bucket B only; do not let Bucket A inflate the typed-ABI line.

3. **The invocation model is guest-driven.** The host invokes the guest's typed `content.create` export; the guest body calls *back* into the host `artifact-create` import to persist and obtain the ULID — the call shape sitting dead in `plugins/mnemra-echo/mnemra_echo.rs:66` (`artifact::artifact_create(ctx, "echo_fixture", frontmatter, body)`) becomes the live path. The host import body (`abi/host_fns.rs`) is `todo!()` today; a `todo!()` body traps at runtime → kill-and-replace fires → the caller gets a structured error, not `Ok`. **The slice-1 wired path therefore needs a non-`todo!()` `artifact-create` body even to return `Ok`.**

4. **MCP-verb → `content`-method mapping is a plan-tier concern.** R-0019-c states verb→export resolution is static against the fixed `content` interface and the exact MCP-verb → CRUD-method mapping is "a forthcoming plan/implementation concern"; the API Contract calls the concrete typed WIT `content` signatures "a plan-tier artifact." This plan pins both, minimally and per-slice (see CC-MAPPING), not as spec gaps.

5. **Green-main definition.** "Green main candidate" means `just ci` passes — compiles + the task's own tests green — not standalone user-facing value. Each slice merges green on its own.

## Decision Record — the storage-gating fork (RESOLVED 2026-06-20 → Branch 2)

**Storage-gating fork resolved 2026-06-20 → Branch 2 (fenced in-memory stub).** Branch 1 (fake-ULID wiring-only) rejected — reason: Branch 2 makes slice 1 a genuine walking skeleton at small delta and the fenced map does not re-couple to the in-flight storage design. T7 and T8 are authored to Branch 2; this entry preserves the rejected branch for audit.

The walking-skeleton's end-to-end-ness is **storage-gated**. The invocation *wiring* (component Linker, pool component-instantiation, request-path invoke, `call_tool` routing, typed `content` export) is buildable now. What gates "truly end-to-end" is the `artifact-create` host-fn body (`libs/mnemra-host/abi/host_fns.rs`), which is `todo!()` today (Reading note 3) — a `todo!()` body traps, so the slice-1 wired path needs a real body even to return `Ok`.

**Chosen — Branch 2: stub storage now, swap real later.** The slice-1 host body (T7) writes `(ULID, type, frontmatter, body)` to a **small in-memory map fenced inside the host-fn layer**, generating a real ULID; `artifact-get` reads it back. Slice 1 is **genuinely end-to-end now** (Test 1 strengthens to assert a well-formed ULID AND a follow-on `echo.get` readback). Cost: a throwaway fenced map + a later swap task (T13).

- **The decisive de-risk (verified):** the throwaway is a **fenced map in the host-fn body**, NOT an extension of the existing `Storage`/`MemStorage` trait (`libs/mnemra-host/storage.rs`, `libs/mnemra-host/storage/memory.rs`). That trait is a transaction/unit-of-work seam over a generic `Record { key, value }` — *not* the artifact-shaped API (`type` / `frontmatter` / `body` / ULID). Extending it to artifact shape would re-couple T7 to **the in-flight storage design** (the storage ADR + adapter work, in progress); the fenced map does not. T7's `forbid_scope` (forbidding edits to the `Storage` trait + impls) is what keeps Branch 2 decoupled from in-flight design — the chosen branch's central fence, enforced by the scope field, not just prose.

**Rejected — Branch 1: wiring-only (constant ULID, no persist).** Preserved for audit. `artifact-create` would return a stub/constant ULID with no persistence; slice 1 would degrade to compile-bind / invoke-returns-without-persisting (the "walking skeleton" framing breaks — it walks the wiring, not a stored artifact), and the real persisting body would couple slice 1's proof-of-end-to-end-ness to the in-flight storage design — a genuine schedule dependency. Rejected because it buys little over Branch 2 at a comparable later cost.

**Common to both (downstream regardless).** Real Postgres persistence against the R-0001 per-artifact-type tables (the `echo_fixture` table machinery already exists via `schema/init.rs`) is downstream substrate work — not built here. The Branch 2 stub body is swapped for the real one in **T13** (the explicit swap task) when real storage lands.

## Cross-task constraints (named, not implicit)

- **CC-RUNVERB-REWRITE** — `trap_recovery.rs::run_verb` currently hard-codes a **core-module, parameterless** `instance.get_typed_func::<(), ()>(&mut *store, "run")` (`libs/mnemra-host/plugin/trap_recovery.rs:394`). Moving to the typed `content` component export rewrites this to the component-typed accessor (`bindings.<content>().call_create(...)` or equivalent). This rewrite **breaks `main` green if left implicit** — it is owned by **T5** (component invoke) and re-pins the existing kill-and-replace tests (see CC-POOL-TESTS-REPIN). It is not a passing note.
- **CC-POOL-TESTS-REPIN** — the existing kill-and-replace tests (`libs/mnemra-host/tests/resource_limits.rs`; pool seams `slot_count` / `take_live_invocation` / `register_module` in `pool.rs`) are pinned to the **core-module `<(),()>("run")` fixture shape**. The component-instantiation swap (T4) and the `run_verb` rewrite (T5) require these tests be re-pinned to the component shape **in the same task that breaks them**, so each task lands green. Owned jointly by T4 (instantiation half) and T5 (invoke half).
- **CC-MAPPING** — the MCP-verb → `content`-method mapping rule is **delegated to this tier** (R-0019-c; API Contract), not pinned by the spec. This plan pins it minimally and per-slice: slice 1 maps `echo.create` → `content.create`, with the MCP `arguments` `{content_type, payload}` (per `libs/mnemra-host/tests/mcp_server.rs:243-248`) mapping to `content.create(type, frontmatter, body)` — `content_type`→`type`, `payload`→`frontmatter` (JSON-as-string), `body`=None for the fixture. Later verbs ride later slices (T12). Pinned with rationale, not assumed silently; not a halt-gap.

## Tasks

Tasks are grouped by slice (see **Sequencing**). TDD pairs split a security-sensitive / public-ABI surface into a red-phase task (the acceptance-test author writes failing tests) and a green-phase task (the implementer makes them pass) per R-0018-a; mechanical surfaces collapse to implementer-self-test. Sizing is **relative (S/M/L)**. Every task maps to a locked R-ID or a named cross-task constraint. The FENCE (no runtime export registry / no dynamic verb→export resolution / no domain verbs — R-0019-c/d, spec Out of Scope) holds across every task. The two reliability carries map to R-0007-h (can_invoke / epoch-health gating) and R-0007-e / R-0016-a / R-0004-b (limit-attach / event / dedup).

---

### Task 1 (RED+GREEN): WIT — typed `content` export interface; retire `run`

**Files:** `wit/echo.wit` (replace `export run: func(input: string) -> string;` with `export content;`), `wit/host.wit` or a new `wit/content.wit` (declare `interface content { create / get / list / update / delete }`), `libs/mnemra-host/tests/abi_contract.rs` (extend the export-side assertions)
**Type:** backend (ABI surface, WIT)
**Depends on:** None (entry task)
**Size:** S

**What:** Declare a fixed typed `content` interface every content plugin exports — `create(type: string, frontmatter: json, body: option<string>) -> string`, `get(id: string) -> option<string>`, `list(type: string, filters: json) -> list<string>`, `update(id: string, frontmatter-patch: json, body: option<string>)`, `delete(id: string)` — mirroring the API Contract export table; retire the `run(input: string) -> string` export. `type json = string` is reused from `wit/host.wit` (JSON-as-string payloads cross the typed boundary; only string-based verb *dispatch* is prohibited).

**Acceptance Criteria:**
- [ ] `wit/echo.wit` no longer declares `export run`; it declares `export content` (`R-0019-a`, `R-0019-e`).
- [ ] The `content` interface declares exactly the five CRUD methods with the typed signatures in the API Contract export table (`R-0019-a`, API Contract §Plugin export / invocation ABI).
- [ ] No runtime export registry and no manifest-verb→export resolution is declared in WIT (FENCE: `R-0019-c`).
- [ ] The WIT parses (named-type resolution passes, as `workspace-ctx` does today).
- [ ] `tests/abi_contract.rs` asserts the built component exports `content` (create/get/list/update/delete) and does NOT export a `run`-shaped function (`R-0019-e`).

**Test Expectations:**
- WIT parses; the contract-shape test pins the export-side ABI (typed `content` present, `run` absent). Coverage target: export-side ABI observability.

---

### Task 2: Guest — implement the typed `content` export (slice 1 = `create`; stub the rest)

**Files:** `plugins/mnemra-echo/mnemra_echo.rs` (replace `impl Guest { fn run }` with the `content` export impl), regenerated `wit_bindgen::generate!` against the new world
**Type:** backend (guest plugin)
**Depends on:** Task 1
**Size:** M

**What:** Implement the guest `content.create` to route to the host `artifact-create` import (the dead `artifact_ops()` call-site shape at `mnemra_echo.rs:66` becomes the live path). For slice 1, `create` is the only fully-wired method; `get`/`list`/`update`/`delete` are minimal stubs returning typed-but-empty values (wired in T12). Remove the `#[allow(dead_code)]` on the create path; the echo round-trip (`echo.echo` / `increment-counter`) is retired or kept as an internal helper, not the export. Mechanical wiring against the generated bindings — implementer self-tests via the build.

**Acceptance Criteria:**
- [ ] The guest exports the typed `content` interface; `content.create` calls `artifact::artifact_create(ctx, type, frontmatter, body)` (`R-0019-a`, guest-driven model per `wit/host.wit`).
- [ ] No string-parse verb-dispatch exists in the guest (FENCE: `R-0019-b` — no `run`-style input parsing).
- [ ] `just plugin` builds the component clean for `wasm32-wasip2`.
- [ ] `just plugin-wit` on the built component shows `export content` and no `run` (`R-0019-e`).

**Test Expectations:**
- `just plugin` green; `just plugin-wit` shows the typed `content` export. Coverage target: guest-side typed export binds and the create path reaches the host import.

---

### Task 3: Host — component `Linker` + host-fn import registration

**Files:** `libs/mnemra-host/plugin/linker.rs` (to-be-created, or extend `pool.rs`), `libs/mnemra-host/abi.rs`, `libs/mnemra-host/abi/host_fns.rs`
**Type:** backend (component-host machinery — Bucket A, ABI-agnostic)
**Depends on:** Task 1
**Size:** L

**What:** Build a `component::Linker<HostState>` (via `component::bindgen!` or a hand-rolled `component::Linker` — either is ABI-agnostic; pick one and record it) that exposes the host-fn import interfaces to the guest instance. At slice 1 only `artifact` (specifically `artifact-create`, plus `artifact-get` for the readback) must be wired; the remaining imports (`metrics` / `log` / `event` / `projection` / `sampling` / `secrets`) may be registered as no-op-or-`todo!()` bodies **only if unreachable from slice 1's path** — any import the guest calls on the slice-1 path must be non-`todo!()` (a `todo!()` traps).

**Acceptance Criteria:**
- [ ] A `component::Linker` registers the host-fn import bodies; the guest instance instantiates *with imports*, closing the `pool.rs:293` "no imports" gap (`R-0012-a`, `R-0012-f`).
- [ ] The per-instance host-fn allowlist (`R-0003-b`) is honored at the linker boundary — undeclared host-fn calls fail at the WIT boundary, not the body (the allowlist already exists: `plugin/allowlist.rs`, `runtime.rs`).
- [ ] `WorkspaceCtx` is threaded as the first parameter to every wired host-fn (`R-0006-a`, `R-0006-e`); no alternative construction path (`R-0006-b`).

**Test Expectations:**
- An integration test instantiates a component with the linker and confirms a guest `artifact-create` call reaches the host body (not a missing-import link error). Coverage target: import-side linker wiring + allowlist enforcement at the boundary.

---

### Task 4: Host — component instantiation in the pool (`Module` → `Component`)

**Files:** `libs/mnemra-host/plugin/pool.rs` (`LiveSlot`, `LiveModuleEntry`, `instantiate_live_slot`, `register_module`), `libs/mnemra-host/tests/resource_limits.rs` (re-pin per CC-POOL-TESTS-REPIN)
**Type:** backend (component-host machinery — Bucket A)
**Depends on:** Task 3
**Size:** L

**What:** Swap the pool's `wasmtime::Module` for `wasmtime::component::Component`; swap `Instance::new(&mut store, module, &[])` (`pool.rs:293`) for `Linker::instantiate`, threading a real `Store<HostState>` carrying the host-fn state. Preserve the kill-and-replace invariant (`R-0016-c`): the slot still holds a live trappable instance; take/repopulate semantics unchanged in shape.

**Acceptance Criteria:**
- [ ] The pool holds `component::Component` + component `Instance`; `instantiate_live_slot` uses the Linker (`R-0016-a`, `R-0016-b`).
- [ ] Kill-and-replace preserved: `slot_count` equal before/after a breaching invocation; replacement synchronous (`R-0016-c`).
- [ ] **CC-POOL-TESTS-REPIN (instantiation half):** the kill-and-replace suite (`tests/resource_limits.rs`) is re-pinned to the component shape and is **green in this task** (`R-0007-e`, `R-0007-f`). No assertion relaxed — the trap→kill→replace→structured-error behavior is unchanged; only the instance/export shape moves.

**Test Expectations:**
- The kill-and-replace tests are green against the component shape (epoch + fuel breach paths still trap→kill→replace per Scenarios "Resource limit breach" and "Plugin fuel exhaustion"). Coverage target: pool component-instantiation + preserved kill-and-replace.

---

### Task 5: Host — request-path component invoke + `run_verb` rewrite (CC-RUNVERB-REWRITE)

**Files:** `libs/mnemra-host/plugin/runtime.rs` or `pool.rs` (new invoke method), `libs/mnemra-host/plugin/trap_recovery.rs` (`run_verb`, `invoke_with_recovery`), `libs/mnemra-host/tests/resource_limits.rs` (re-pin)
**Type:** backend (component-host machinery — Bucket A)
**Depends on:** Task 4
**Size:** L

**What:** Add a pool invoke method that borrows a slot and calls the typed `content` export through the trap-recovery seam. **CC-RUNVERB-REWRITE:** rewrite `run_verb` (`trap_recovery.rs:394`) from the core-module `get_typed_func::<(),()>("run")` to the component-typed accessor for the resolved `content` method (slice 1: `content.create`). The per-invocation fuel/epoch budget and memory limiter continue to apply (`R-0007-a/b/c/g`). Output marshalling (`trap_recovery.rs:397` "Real output marshalling is Task 23 scope") now carries the typed return.

**Acceptance Criteria:**
- [ ] A pool invoke method calls the typed `content` export via the trap-recovery seam (`R-0007-e`, `R-0007-f`).
- [ ] **CC-RUNVERB-REWRITE:** `run_verb` calls the component-typed `content` accessor, not the core-module `run` fixture; the success branch returns the typed export value (`R-0019-a`).
- [ ] Budget applied per invocation (fuel + epoch + memory), both active simultaneously (`R-0007-g`).
- [ ] **CC-POOL-TESTS-REPIN (invoke half):** the trap-recovery tests are green against the component invoke path; trap→kill→replace→structured-error behavior unchanged (`R-0007-e`, `R-0007-f`, `R-0016-c`).

**Test Expectations:**
- Invoke happy path returns the typed export value; both breach paths (epoch, fuel) still trap→kill→replace per the two limit scenarios. Coverage target: request-path invoke + trap recovery against the component export.

---

### Task 6: Host — `call_tool` → export routing + result mapping (replace the stub)

**Files:** `libs/mnemra-host/mcp/server.rs` (`call_tool`, replace `Ok(CallToolResult::default())` at `:168`), `libs/mnemra-host/mcp/dispatch.rs` (routing), `libs/mnemra-host/plugin/output.rs` (typed-return → `CallToolResult` mapping; output validation already present)
**Type:** backend (component-host machinery — Bucket A, the routing seam)
**Depends on:** Task 5, Task 2
**Size:** M

**What:** After `auth_and_authorize` returns the `WorkspaceCtx` (`server.rs:164`), map the authenticated verb to its `content` method (CC-MAPPING: slice 1 `echo.create`→`content.create`, `{content_type, payload}`→`content.create(type, frontmatter, body)`), borrow a slot, invoke via T5's method, and map the typed return into `CallToolResult`. The per-verb capability check against the manifest `verbs` list (`R-0010-d`) remains the **pre-dispatch capability gate**, NOT a runtime export registry (FENCE: `R-0019-c`). The pre-dispatch permission path (`is_write_verb`) is unchanged.

**Acceptance Criteria:**
- [ ] `call_tool` no longer returns `CallToolResult::default()`; it routes the authenticated verb to the typed `content` export and maps the typed return into `CallToolResult` (`R-0010-c`, `R-0010-d`, `R-0019-a`, `R-0019-e`).
- [ ] Verb→export resolution is **static** against the fixed `content` interface; the manifest `verbs` list is the pre-dispatch gate only (FENCE: `R-0019-c`).
- [ ] A manifest-declared verb with no matching typed export returns the `R-0019-d` structured non-dispatchable error AND leaves the pre-dispatch permission outcome unchanged (`R-0019-d`, `R-0019-e`). (At V0 no such verb ships through `content`; the path is covered by `R-0019-e`, not a runtime registry.)
- [ ] Error classes stay distinguishable (auth / verb-not-found / param-invalid / permission-denied / exec-timeout / resource-exhausted) (`R-0010-f`, API Contract §MCP transport error codes).
- [ ] Output validation applied before mapping (`R-0003-f`; `plugin/output.rs`).

**Test Expectations:**
- The four merged `mcp_server.rs` behavioral tests stay green (auth-failure, read-observer-write-denied, control-plane-absent, read-observer-get-not-denied); the happy-path test (`valid_admin_token_echo_create_returns_ok`) now exercises the real routing (its strengthening to real readback is T8). Coverage target: routing seam + result mapping + error-class distinctness.

---

### Task 7: Host — `artifact-create` + `artifact-get` host-fn body (Branch 2 fenced in-memory stub)

**Files:** `libs/mnemra-host/abi/host_fns.rs` (`artifact_create`, `artifact_get`)
**Type:** backend (host-fn body)
**Depends on:** Task 3
**Size:** M

**touch_scope:** `["libs/mnemra-host/abi/host_fns.rs"]` (plus the host-fn body's own module if a fenced-map type is added under `abi/`).
**forbid_scope:** `["libs/mnemra-host/storage.rs", "libs/mnemra-host/storage/memory.rs", "libs/mnemra-host/storage/postgres.rs", "libs/mnemra-host/storage/postgres/engine.rs"]` — **load-bearing de-risk.** The fenced map lives in the host-fn body, NOT in the `Storage`/`MemStorage` trait. That trait is a generic-`Record { key, value }` transaction seam; artifact-shaping it (`type`/`frontmatter`/`body`/ULID) would re-couple this task to the in-flight storage design. Forbidding edits to the `Storage` trait + impls is what keeps Branch 2 decoupled from in-flight design. This is the chosen branch's central fence — enforced by the scope field, not just prose.

**What:** `artifact_create` generates a **real ULID** and writes `(id, type, frontmatter, body)` to a **small fenced in-memory map** that lives inside the host-fn layer (`abi/host_fns.rs`), keyed/scoped by `ctx.workspace_id()` (the `R-0006-d` WHERE-clause-shape discriminator is already present at `host_fns.rs:80`). `artifact_get` reads back from the same fenced map. The map is a deliberate throwaway — swapped for real `Storage`/Postgres in **T13** when storage lands. The body must NOT `todo!()` on the slice-1 reachable path (a `todo!()` traps → the happy-path test cannot return `Ok`).

**Acceptance Criteria:**
- [ ] `artifact_create` generates a real ULID and persists `(id, type, frontmatter, body)` to a fenced in-memory map keyed within `ctx.workspace_id()` (`R-0012-a`, `R-0006-a`, `R-0006-d`).
- [ ] `artifact_get` returns the just-created artifact for the same workspace; a cross-workspace get returns `None` (`R-0006-d` isolation, enforced at the fenced-map level).
- [ ] **FENCE on coupling (forbid_scope):** no edit to `libs/mnemra-host/storage.rs`, `storage/memory.rs`, or the Postgres adapter; the fenced map does not touch the `Storage` trait. (Keeps the task decoupled from the in-flight storage design.)
- [ ] No `todo!()` reachable from the `echo.create` / `echo.get` slice-1 path.

**Test Expectations:**
- Create→get readback round-trips within a workspace; cross-workspace get returns `None` (workspace isolation at the fenced-map level). Coverage target: the slice-1 host body (fenced in-memory stub).

---

### Task 8: Slice-1 end-to-end acceptance — strengthen the happy-path MCP test (real readback)

**Files:** `libs/mnemra-host/tests/mcp_server.rs` (the `valid_admin_token_echo_create_returns_ok` test)
**Type:** test
**Depends on:** Task 6, Task 7
**Size:** S

**What:** The happy-path test today asserts only `result.is_ok()` (`mcp_server.rs:254`) and is the explicitly-flagged strengthening target (the `call_tool` Task-5/Task-22-wiring note at `libs/mnemra-host/mcp/server.rs:22-29`). Strengthen it to assert `Ok` + a well-formed ULID in the `CallToolResult` content, AND add a follow-on `echo.get` (admin token) that returns the just-created artifact — **real end-to-end readback**, the genuine walking-skeleton proof Branch 2 enables.

**Acceptance Criteria:**
- [ ] The happy-path test asserts `Ok` + a well-formed ULID is returned (not merely `is_ok()`) (`R-0019-e`, Scenario "MCP verb dispatches to a typed content export").
- [ ] The test adds a follow-on `echo.get` that returns the created artifact (real readback) (`R-0012-a`, Scenario "MCP verb dispatches under WorkspaceCtx").

**Test Expectations:**
- The slice-1 end-to-end path (create → readback) is asserted against the fenced in-memory stub. Coverage target: slice-1 end-to-end acceptance. (When T13 swaps in real storage, this test's persistence assertion re-targets real `Storage` — see T13.)

> **Slice 1 closes here.** Tasks 1–8 = the walking skeleton: MCP `call_tool` → auth (exists) → pool invoke → guest `content.create` → host `artifact-create` (Branch-2 fenced in-memory stub) → typed ULID return → `CallToolResult` → readback via `echo.get`. Slice 1 lands on `main` green and is **genuinely end-to-end** against the stub. Each task below is a subsequent slice; each lands green on its own.

---

### Task 9: Carry (R-0007-h) — wire `can_invoke()` / epoch-health gating into the invoke path

**Files:** `libs/mnemra-host/plugin/trap_recovery.rs` (`invoke_with_recovery`) or the T5 invoke method, `libs/mnemra-host/mcp/dispatch.rs` / `server.rs` (request path)
**Type:** backend (security/reliability carry)
**Depends on:** Task 5
**Size:** M

**What:** The mechanism is built and tested (`pool.can_invoke()`, `epoch_health()` at `pool.rs:203-211`); the **request-path wiring is the open work**. The invoke path MUST call `pool.can_invoke()` / epoch-health and **REFUSE** invocations while the epoch-tick supervisor is degraded, returning a structured error; the `/health` `overall` field reflects `"degraded"` while the thread is dead (already partly wired).

**Acceptance Criteria:**
- [ ] The invoke path calls `pool.can_invoke()` before dispatching and refuses with a structured error when the epoch-tick thread is degraded (`R-0007-h`).
- [ ] A degraded supervisor actually blocks invokes (closes the `R-0007-h` half-satisfied gap: the mechanism exists but is not yet wired to the request path).

**Test Expectations:**
- With the epoch-tick thread injected-dead (the existing test seam, `tests/ui/no_test_seams/can_invoke_epoch_seam_reachable.rs`), an invoke is refused with the structured error; healthy → invoke proceeds. Coverage target: invoke-path health gating. (Red phase: `verify` empty by design — rationale recorded — until the green phase lands the recipe.)

---

### Task 10: Carry (R-0007-e / R-0016-a / R-0004-b) — per-invocation limit-attach + event emission + dedup

**Files:** `libs/mnemra-host/plugin/trap_recovery.rs` / invoke path (limit-attach tripwire), `libs/mnemra-host/plugin/pool.rs` (POOL_MAX growth + reject-duplicate-register), `libs/mnemra-host/auth/token.rs` (the `TokenRotatedEvent` pattern for `epoch_tick_thread_died`)
**Type:** backend (reliability carry)
**Depends on:** Task 5, Task 9
**Size:** M

**What:** Three sub-items: (a) **blocking** — a limit-attachment tripwire test against a live `Store` asserting fuel = 10B set, epoch deadline set, the 64 MiB limiter traps on >64 MiB grow, and `can_invoke()` is gated before dispatch; (b) `epoch_tick_thread_died` → structured event via the `TokenRotatedEvent` pattern (`R-0004-b`, `R-0007-h`); (c) pool `POOL_MAX` growth + reject duplicate `register` (`R-0016-a`).

**Acceptance Criteria:**
- [ ] Limit-attach tripwire: a live `Store` on the invoke path has fuel = `FUEL_LIMIT` (10B), epoch deadline set, and the 64 MiB limiter denies >64 MiB grow (`R-0007-a/b/c/g`).
- [ ] `epoch_tick_thread_died` emits a structured event (`R-0004-b`, `R-0007-h`).
- [ ] The pool grows to `POOL_MAX` and rejects a duplicate `register` for an already-registered plugin (`R-0016-a`).

**Test Expectations:**
- The limit-attach tripwire test is green against the live invoke `Store`; duplicate-register rejected; thread-death event observable. Coverage target: per-invocation limit attach + event + dedup. (Red phase: `verify` empty by design — rationale recorded.)

---

### Task 11: Host startup — pool population from the built `echo` component + MCP server start

**Files:** `libs/mnemra-host/mnemra_host.rs` / `startup.rs`, `libs/mnemra-host/plugin/pool.rs` (`register_module` from real component bytes), `libs/mnemra-host/mcp/server.rs` (wire the pool into `MnemraMcpServer`)
**Type:** backend (startup wiring)
**Depends on:** Task 4, Task 6
**Size:** M

**What:** At host startup, load the built `mnemra-echo` component (`target/wasm32-wasip2/release/mnemra_echo.wasm`, produced by `just plugin`), verify its signature synchronously (`R-0005-a`, `R-0005-b`; the in-memory test-key path exists), populate the pool (3–5 instances, `R-0016-a`) **before** the MCP server accepts requests, and hand the pool to `MnemraMcpServer`. Closes the `pool.rs` "population from actual `.wasm` bytes is deferred to Task 23" note.

**Acceptance Criteria:**
- [ ] The pool is populated with 3–5 verified `echo` component instances at startup, before the MCP server accepts requests (`R-0016-a`, `R-0005-a`, `R-0005-b`).
- [ ] `MnemraMcpServer` holds the populated pool and routes `call_tool` through it (`R-0010-a`).
- [ ] No plugin invocation precedes pool readiness (`R-0002-c`, `R-0016-a`).

**Test Expectations:**
- A startup integration test confirms the pool is populated and a `call_tool` dispatches through a pooled instance. Coverage target: startup population + server wiring. (The real smoke gate is the substrate plan's Task 27 scope — `verify-smoke` is a scaffold today.)

---

### Task 12: Slices 2–5 — wire the remaining CRUD verbs (`get` / `list` / `update` / `delete`)

**Files:** `plugins/mnemra-echo/mnemra_echo.rs` (guest `content.get/list/update/delete`), `libs/mnemra-host/abi/host_fns.rs` (remaining bodies against the T7 fenced map), `libs/mnemra-host/mcp/server.rs` (verb→method mapping for the four verbs), `libs/mnemra-host/tests/mcp_server.rs` (per-verb tests)
**Type:** full-stack (guest + host + test) — author as **per-verb slices** (one verb = one slice = one green-main commit), NOT one batch
**Depends on:** Slice 1 (Tasks 1–8) landed; Task 11 (startup) for end-to-end
**Size:** L

**forbid_scope:** inherits T7's fence — `["libs/mnemra-host/storage.rs", "libs/mnemra-host/storage/memory.rs", ...]`. The remaining verb bodies operate on the T7 fenced in-memory map, NOT the `Storage` trait (decoupled from the in-flight storage design until T13).

**What:** Extend CC-MAPPING per verb: `echo.get`→`content.get`, `echo.list`→`content.list`, `echo.update`→`content.update`, `echo.delete`→`content.delete`. Each verb's host body operates on the T7 fenced in-memory map (read / list / patch / delete). `artifact.delete` remains opt-in per manifest (`R-0003-g`; already declared in the `echo` manifest).

**Acceptance Criteria:**
- [ ] Each verb dispatches to its typed `content` method statically (FENCE: `R-0019-c`); `delete` is honored only when manifest-declared (`R-0003-g`).
- [ ] Read verbs (`get` / `list`) are reachable by `ReadObserver`; write verbs (`update` / `delete`) are denied to `ReadObserver` (`R-0009-d`, already enforced by the merged `is_write_verb` guard).
- [ ] Each verb's per-verb MCP test is green (`R-0019-e`).

**Test Expectations:**
- One behavioral test per verb mirroring slice 1's shape; the full CRUD surface dispatches through the typed exports. Coverage target: the remaining four typed CRUD verbs.

---

### Task 13: Swap the fenced in-memory stub for real `Storage`/Postgres persistence

**Files:** `libs/mnemra-host/abi/host_fns.rs` (replace the fenced map in `artifact_create`/`get`/`list`/`update`/`delete` with calls into the real `Storage` trait), `libs/mnemra-host/tests/mcp_server.rs` (re-target the happy-path persistence assertion against real storage)
**Type:** backend (storage swap)
**Depends on:** **the substrate storage seam (the real `Storage`/Postgres artifact-persistence work — the in-flight storage ADR + adapter) MUST land first**; Task 7; Task 12
**Size:** M

**What:** The explicit retirement of the Branch-2 throwaway. Replace the fenced in-memory map (introduced in T7, used by T12) with calls into the real artifact-persistence `Storage` API once it lands. The host-fn bodies (`artifact_create` etc.) now persist through `Storage` (`R-0012-a` host-fn ABI; `R-0019` invocation path unchanged — only the body's storage backend changes), WHERE-scoped on `ctx.workspace_id()` (`R-0006-d`). Removes the fenced map entirely (no dual-write, no lingering stub) and lifts T7's `Storage`-trait `forbid_scope` (that fence existed only to keep the stub decoupled from in-flight design; once storage lands the fence is retired). This task does NOT build the `Storage` seam — it consumes it.

**Acceptance Criteria:**
- [ ] The fenced in-memory map from T7 is **removed**; `artifact_create`/`get`/`list`/`update`/`delete` host-fn bodies persist through the real `Storage` trait (`R-0012-a`).
- [ ] WHERE-clause workspace scoping holds against real storage (`R-0006-d`; the `verify-lint` WHERE-clause lint `R-0018-d` stays green).
- [ ] Real artifacts persist against the R-0001 per-artifact-type table (`echo_fixture`) — created rows survive across transactions (`R-0001-a`).
- [ ] The happy-path test's persistence assertion is **re-targeted against real storage** — create → real persisted row → `echo.get` readback from the real backend (`R-0012-a`, `R-0019-e`; strengthens the T8 stub-level assertion).
- [ ] No invocation-path change: the typed `content` export contract (`R-0019`) and the trap-recovery seam are untouched; only the host-fn body's backend swaps.

**Test Expectations:**
- The slice-1 (and CRUD) end-to-end paths now persist to real Postgres; the happy-path test asserts real-backend readback; full CRUD round-trips against the real table. Coverage target: the storage swap + real-backend persistence assertions.

---

## Sequencing

**Slice-through order (each slice green-on-merge):**

```
Slice 1 (walking skeleton) — strict spine, mostly sequential:
  T1 (WIT typed content + retire run)
   ├─> T2 (guest export)            ── parallel-safe with T3 after T1
   └─> T3 (Linker / import bodies)
          └─> T4 (pool component-instantiation + CC-POOL-TESTS-REPIN)
                 └─> T5 (invoke + CC-RUNVERB-REWRITE + CC-POOL-TESTS-REPIN)
                        └─> T6 (call_tool routing)            ── needs T2 + T5
                               └─> T7 (host body — Branch-2 fenced stub)  ── needs T3
                                      └─> T8 (slice-1 e2e acceptance — real readback)  ── needs T6 + T7

Post-skeleton (each a green-main slice; ordering by dependency, not want):
  T9  (carry R-0007-h can_invoke gating)     ── needs T5
  T10 (carry limits/events/dedup)            ── needs T5, T9
  T11 (startup population + server)          ── needs T4, T6
  T12 (slices 2–5: get/list/update/delete)   ── needs slice 1 + T11; author per-verb

Storage swap (gated on substrate storage — orders AFTER storage lands):
  T13 (swap fenced stub → real Storage/Postgres)  ── needs the substrate storage seam + T7 + T12
```

**Parallel-safe groups** (no shared file targets, no hard dependency between members):
- After Task 1: **{Task 2 guest export}** and **{Task 3 host Linker}** start in parallel (independent surfaces).
- After the spine lands: **{Task 9 → Task 10}** and **{Task 11}** run concurrently (Task 10 depends on Task 9 for the health-gating tie-in; Task 11 depends on Task 4 + Task 6), then Task 12.

**Strict sequences (highest rework-if-reordered first):**
- Task 1 (WIT) → Task 3 → Task 4 → Task 5 → Task 6 → Task 7 → Task 8 — the slice-1 spine.
- Task 4 and Task 5 carry the test-re-pin (CC-POOL-TESTS-REPIN) and the `run_verb` rewrite (CC-RUNVERB-REWRITE) — the highest-rework-if-reordered tasks, anchored mid-spine where the component shape stabilizes.
- The substrate storage seam → Task 13 — the stub swap never runs before real storage lands.

**Critical path (the chain whose slip slips the slice-1 ship):** Task 1 → Task 3 → Task 4 → Task 5 → Task 6 → Task 7 → Task 8.

**Review gates (placed at slice boundaries — minimum holding cost):** this is a **security-critical write path** (auth → invoke → persist).
- **Per implementation task (Tasks 2–7, 9–13):** the test-author-first triplet — the **acceptance-test author writes failing tests → the implementer makes them pass → the test author re-reviews the suite** (no assertion relaxation, the red discriminates, the suite is green). Red-phase tasks carry `verify` empty by design (they fail against absent/wrong code — right-reason red); the green phase adds the recipe. Task 1 (WIT-only) and Task 8 (test-strengthen) may collapse the pair (Task 1 is a declarative ABI edit verified by Task 2's build; Task 8 *is* the test).
- **Security review — per slice, not per task** (the auth→invoke→persist surface is only assessable as a whole slice): slice 1 (Tasks 1–8) gets one security review over the full invocation path; the carries (Task 9, Task 10) each get a security review (health-gating and limit-attach are security/reliability surfaces); Task 11 (startup signature-verify path) and Task 12 (write verbs `update` / `delete`) each get a security review; **Task 13 (storage swap) gets a security review — it moves the write path onto a real persistence backend, a security-relevant surface change.**
- **Merge:** **every merge on this path escalates to the maintainer** (security-critical write path). One branch per slice, one PR per slice; the security review's approval precedes the escalation.
- Per the iterate-to-zero discipline, each gate runs up to 3 rounds of routine-finding fixes before escalation; architectural concerns surface as deviation reports, not auto-fired re-rounds.

Gate order per slice: **failing tests → implementation → test re-review → security review → maintainer merge.**

## Test surface

Cross-task test infrastructure (per-task Test Expectations are scoped above):

- **Unit / integration (host):** the merged `tests/mcp_server.rs` suite (6 tests) stays green throughout; the happy-path test strengthens to real readback at Task 8 (against the fenced stub) and re-targets real storage at Task 13. The existing `tests/resource_limits.rs` kill-and-replace suite is re-pinned at Task 4 / Task 5 (CC-POOL-TESTS-REPIN) and stays green. `tests/abi_contract.rs` extends at Task 1 to assert the `content` export / no-`run` (`R-0019-e`). Integration tests run against the real embedded Postgres + bundled `pgvector`, not mocks (`R-0018-b`).
- **Component build:** `just plugin` (guest `wasm32-wasip2` component) green from Task 2 onward; `just plugin-wit` shows the typed `content` export.
- **Test-seam gating (holds):** the no-test-seams gate (`tests/no_test_seams.rs`, the `verify-test-hooks` recipe) must stay green — invoke-path seams (e.g. the `can_invoke` epoch seam) remain behind the `test-hooks` feature and are not reachable in production builds.
- **Property / edge:** workspace isolation at the fenced-map level (Task 7): a cross-workspace get returns `None`. Trap independence (epoch vs fuel) preserved through the component shape (Task 4 / Task 5).
- **Smoke / E2E:** the real smoke gate is the substrate plan's Task 27 scope (`verify-smoke` is a scaffold today); Task 11 adds a startup-population integration test as the interim end-to-end check.
- **CI entry point:** `just ci` (`R-0018-f`) — `verify-type`, `verify-lint` (incl. the WHERE-clause lint `R-0018-d`), `verify-test`, `verify-test-hooks`, `verify-coverage`, `verify-build`, `verify-smoke`. Every slice's green-main claim is `just ci` green.
- **No hardcoded secrets in tests:** admin tokens and signing keypairs are generated per-run, never literal.

## Release binding

- **Target release:** `0.1.0` (V0 substrate).
- **Branch posture:** trunk-based; one slice = one squash-merge to `main`, green on its own (`R-0018-c`, `R-0018-f`). Short-lived branches per slice; main is protected from direct pushes.
- **CI gate:** `just ci` is the sole CI entry point; every slice's squash-merge requires `just ci` green. **Every merge on this path escalates to the maintainer** (security-critical write path).
- **Release notes target:** `0.1.0` — "typed `content` invocation path (R-0019 / P-0013): MCP `call_tool` → pooled component → typed `content` export → host `artifact` import; `run(string)` retired."

## Archival

When this feature lands `live` (slice 1 + carries + slices 2–5 + the Task 13 storage swap):
- Mark this plan `archived` at the top.
- Move to `docs/plans/archive/2026-06/`.
- Note the live-tier commit SHA(s) / `0.1.0` release tag. The storage fork was resolved to **Branch 2** (fenced in-memory stub, Task 7) and retired by **Task 13** (swap to real `Storage`/Postgres); record the SHA at which the fenced map was removed.
- The spec remains in `docs/specs/` (durable, designed-tier).

## Spec gaps surfaced

None that block decomposition. Two items are **delegated to this tier by the locked spec** (not gaps — handled in-plan, recorded for the maintainer's awareness):

1. **MCP-verb → `content`-method mapping rule (CC-MAPPING).** R-0019-c and the API Contract explicitly state this is "a forthcoming plan/implementation concern" / "a plan-tier artifact," not pinned by the spec. Pinned here minimally, per-slice, with rationale (slice 1: `echo.create`→`content.create`, `{content_type, payload}`→`(type, frontmatter, body)`). If the maintainer wants the mapping rule promoted to a spec amendment (parallel to how R-0019 amended the export shape), that is a separate spec-tier decision — this plan does not require it, but flags the option.
2. **Concrete typed WIT `content` signatures.** API Contract: "Concrete typed WIT signatures for the `content` interface are a plan-tier artifact, authored with the implementation." Pinned at Task 1 against the export-table shapes. Not a gap.

One observation (not a gap): P-0013's "Follow-up" notes "a separate spec amendment adds the guest invocation-envelope requirement." **R-0019 is that amendment** — it pins the export/invocation side at the spec SHA this plan decomposes against. This plan treats R-0019 as the landed, locked export-side requirement, not a pending blocker. If a *further* envelope amendment beyond R-0019 was intended, it should be named; absent that, R-0019 is sufficient for this plan.
