//! Component-model host bindings — the `Linker` + host-fn import registration
//! that makes the guest's typed `content` export invocable (T3, R-0012-a/f,
//! R-0019, R-0006-a/b/e).
//!
//! # What this module owns
//!
//! 1. The `component::bindgen!` invocation over the `plugin` WIT world — this
//!    generates the host side of the ABI: typed accessors for the guest's
//!    `content` export (`call_create` / `call_get` / …) and the `Host` traits
//!    for the imports the guest calls back into (`artifact`).
//! 2. `HostState` — the store data threaded through every component instance.
//!    It carries the host-derived `WorkspaceCtx` (the authoritative scoping key,
//!    R-0006-b) and a handle to the fenced in-memory artifact map (T7), and it
//!    implements `ResourceLimiter` so the 64 MiB memory ceiling (R-0007-c)
//!    travels with the store exactly as `PluginResourceLimiter` did before the
//!    component migration.
//! 3. The `Host` trait impls for the wired imports. `artifact-create` /
//!    `artifact-get` delegate to the fenced-map bodies in `abi::host_fns`,
//!    deriving `workspace_id` from `self` (store data) and IGNORING any
//!    guest-supplied `workspace-ctx` value (R-0006-e: no plugin-supplied
//!    scoping key — the guest cannot forge a workspace).
//!
//! # WorkspaceCtx threading (R-0006-b/e)
//!
//! The typed `content` export carries no ctx. The host constructs the single
//! authoritative `WorkspaceCtx` at the dispatch site (R-0006-b), stores its
//! `workspace_id` in `HostState`, and invokes the export. When the guest calls
//! back into `artifact-create`/`artifact-get`, the import body reads
//! `workspace_id` from `HostState` — never from the `workspace-ctx` argument the
//! guest passes (that argument is a host-ignored placeholder, R-0006-e).
//!
//! # Unwired imports (reachability)
//!
//! At slice 1 the echo `content.create`/`content.get` path calls ONLY
//! `artifact-create` / `artifact-get`. The remaining world imports (`echo`,
//! and the WASI imports the `wasm32-wasip2` component carries) are NOT reached
//! on this path; they are satisfied by `Linker::define_unknown_imports_as_traps`
//! — a missing-import call would trap, never silently succeed. This is honest:
//! the slice-1 path that runs is fully real; only genuinely-unreachable imports
//! are trap-stubbed (no `todo!()` on the reachable path).

use std::cell::Cell;

use uuid::Uuid;
use wasmtime::component::{Component, Instance, Linker};
use wasmtime::{ResourceLimiter, Store};

use sqlx::PgPool;

use crate::plugin::limits::MEMORY_MAX_BYTES;

// ---------------------------------------------------------------------------
// R-0020-d — scan-cost statement_timeout backstop: thread-local signal + helpers
// ---------------------------------------------------------------------------

thread_local! {
    /// Per-thread signal that the most recent `artifact_list` keyset SELECT was
    /// canceled by the host-side `statement_timeout` (SQLSTATE 57014, R-0020-d).
    ///
    /// This is the only in-band channel for a DISTINCT caller-facing error code.
    /// A WIT host-fn returns a non-`Result` type, so its sole error channel is
    /// `panic!`, and the `catch_unwind` recovery seam in `trap_recovery` collapses
    /// every host-fn panic to the generic `plugin_invocation_failed` code with the
    /// payload discarded (no-leak). To surface a `query_scan_timeout` code that is
    /// distinct from `plugin_execution_timeout` (the guest epoch -4004) and from
    /// `-32602` (parameter-invalid), the scan-timeout class must ride this
    /// side-channel up to the dispatch site.
    ///
    /// Set in `artifact_list` immediately before the fail-closed panic; read +
    /// cleared by `server.rs::call_tool` within the SAME `spawn_blocking` closure.
    /// Same-thread by construction: `artifact_list` runs synchronously on the
    /// blocking thread via `block_on` (no task migration), so the flag is live in
    /// the closure but would be empty after `.await` (a different runtime thread).
    static SCAN_TIMEOUT: Cell<bool> = const { Cell::new(false) };
}

/// Set the per-thread scan-cost-timeout flag (R-0020-d). Called from
/// `artifact_list` on a 57014 cancellation, immediately before the fail-closed
/// panic that aborts the page.
fn flag_scan_timeout() {
    SCAN_TIMEOUT.with(|c| c.set(true));
}

/// Clear the per-thread scan-cost-timeout flag. The dispatch site calls this at
/// the START of the `spawn_blocking` closure so a flag left by a prior invocation
/// on the same pooled blocking thread cannot leak into this one (R-0020-d).
pub fn clear_scan_timeout() {
    SCAN_TIMEOUT.with(|c| c.set(false));
}

/// Take (read + clear) the per-thread scan-cost-timeout flag (R-0020-d). The
/// dispatch site calls this immediately after `invoke_content` returns, on the
/// same `spawn_blocking` thread the host-fn ran on, to decide whether to surface
/// the `query_scan_timeout` caller error + `outcome = "timeout"` metric.
pub fn take_scan_timeout() -> bool {
    SCAN_TIMEOUT.with(|c| c.replace(false))
}

/// True iff `e` is a Postgres `statement_timeout` cancellation — SQLSTATE
/// **57014** (`query_canceled`), the scan-cost backstop firing (R-0020-d). Matches
/// ONLY 57014; every other SELECT error keeps the existing fail-closed panic and
/// is never reclassified as a scan timeout.
fn is_query_canceled(e: &sqlx::Error) -> bool {
    e.as_database_error().and_then(|db| db.code()).as_deref() == Some("57014")
}

/// Parse a Postgres `current_setting('statement_timeout')` display string to
/// milliseconds (R-0020-d part-i read-back). Postgres normalizes the value to a
/// human-friendly unit (`3000` ms → `"3s"`, `50` ms → `"50ms"`, `0` → `"0"`), so
/// the read-back converts the unit back to a millisecond count for the white-box
/// assertion (== 3000). Only ever consumes a value the host itself just set, so a
/// best-effort parse (unknown unit → numeric part) is safe. Test-only (the
/// production read path does not read the GUC back).
#[cfg(feature = "test-hooks")]
fn parse_pg_timeout_to_ms(raw: &str) -> i64 {
    let s = raw.trim();
    let split = s
        .find(|c: char| !c.is_ascii_digit() && c != '-')
        .unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let n: i64 = num.parse().unwrap_or(0);
    match unit.trim() {
        "" => n,
        "us" => n / 1000,
        "ms" => n,
        "s" => n * 1000,
        "min" => n * 60_000,
        "h" => n * 3_600_000,
        "d" => n * 86_400_000,
        _ => n,
    }
}

// ---------------------------------------------------------------------------
// Generated host bindings for the `plugin` world
// ---------------------------------------------------------------------------

// `component::bindgen!` generates, against the fixed `plugin` world in
// `wit/echo.wit`:
//   - `Plugin` — the instantiated-world handle with typed export accessors
//     (`plugin.mnemra_host_content().call_create(store, ..)` etc.).
//   - `Plugin::add_to_linker` and per-interface `add_to_linker` helpers.
//   - `mnemra::host::artifact::Host` / `mnemra::host::echo::Host` import traits.
//   - `mnemra::host::types::{WorkspaceCtx, ..}` shared types.
//
// Imports are synchronous and non-trappable: the slice-1 artifact bodies always
// succeed (fenced-map insert/lookup), so the generated `Host` methods return
// plain values rather than `wasmtime::Result`. The export side is synchronous —
// the MCP handler runs the invoke inside a blocking section.
wasmtime::component::bindgen!({
    path: "../../wit",
    world: "plugin",
});

// Re-export the generated guest-import types under stable local names.
pub use mnemra::host::types::ArtifactPage as WitArtifactPage;
pub use mnemra::host::types::WorkspaceCtx as WitWorkspaceCtx;

/// Local `ArtifactPage` — the paged-list result type shared across
/// `content_list` / `trap_recovery` / `server` without exposing bindgen
/// internals to those callers.  The bindgen-generated `WitArtifactPage` is
/// converted into this type inside `content_list` and `artifact_list`.
///
/// R-0020
pub struct ArtifactPage {
    /// Artifact ids visible in the caller's workspace for this page.
    pub ids: Vec<String>,
    /// True when additional pages exist beyond this one.
    pub has_more: bool,
    /// Opaque continuation token; `None` when `has_more` is false.
    pub next_cursor: Option<String>,
}

// ---------------------------------------------------------------------------
// HostState — the per-store host data
// ---------------------------------------------------------------------------

/// The store data threaded through a component instance for one invocation
/// (R-0006-b store-threaded ctx, R-0007-c memory ceiling).
///
/// `workspace_id` is the authoritative, host-derived scoping key set at the
/// dispatch site; the host-fn import bodies read it from here, never from a
/// guest-supplied argument (R-0006-e). `pool` is the Postgres connection pool
/// for artifact persistence (T13), set per-invocation via `set_pool`.
/// `HostState` implements `ResourceLimiter` so the 64 MiB ceiling rides the
/// store unchanged from the pre-component `PluginResourceLimiter`.
pub struct HostState {
    /// Host-derived workspace scoping key (R-0006-b). Defaults to nil and is set
    /// per-invocation at the dispatch site before the export is called.
    workspace_id: Uuid,
    /// Postgres connection pool for artifact persistence (T13). `None` for
    /// resource-limit trap fixture instances that never invoke artifact host-fns.
    pool: Option<PgPool>,
}

impl HostState {
    /// Construct host state with no bound workspace (nil UUID) and no pool.
    /// Used at pool pre-instantiation time, before a request assigns the real
    /// workspace and pool at the dispatch site (R-0006-b).
    pub fn unbound() -> Self {
        Self {
            workspace_id: Uuid::nil(),
            pool: None,
        }
    }

    /// Bind the host-derived workspace scoping key for the upcoming invocation
    /// (R-0006-b: set at the single dispatch site, before the export runs).
    pub fn set_workspace_id(&mut self, workspace_id: Uuid) {
        self.workspace_id = workspace_id;
    }

    /// Bind the Postgres pool for artifact persistence (T13). Called at the
    /// dispatch site alongside `set_workspace_id`.
    pub fn set_pool(&mut self, pool: PgPool) {
        self.pool = Some(pool);
    }

    /// The host-derived workspace scoping key (R-0006-c accessor discipline).
    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }
}

impl ResourceLimiter for HostState {
    /// Approve linear-memory growth up to the 64 MiB ceiling (R-0007-c),
    /// identical to the pre-component `PluginResourceLimiter`.
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= MEMORY_MAX_BYTES)
    }

    /// Table growth is unrestricted at V0.
    fn table_growing(
        &mut self,
        _current: usize,
        _desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// Host-fn import implementations
// ---------------------------------------------------------------------------

impl mnemra::host::types::Host for HostState {}

// TENANT-ISOLATION LINCHPIN (R-0006-e). Every body below derives the workspace
// scoping key from `self.workspace_id` (the host-derived value bound onto this
// `HostState` at the single dispatch site — see `trap_recovery::
// invoke_through_recovery`). The `_ctx: WitWorkspaceCtx` parameter is the value
// the GUEST passes across the import boundary; it is DELIBERATELY IGNORED here
// (prefixed `_`). No host-fn body may read, trust, or branch on `_ctx` — doing
// so would let a plugin choose its own workspace and breach tenant isolation.
// The scoping key is host-derived, never plugin-supplied.
impl mnemra::host::artifact::Host for HostState {
    /// `artifact-create` — INSERT into `echo_fixture` and return the generated
    /// ULID (T13, AC#1). Injects the ULID into `frontmatter['id']` to satisfy
    /// CHECK (frontmatter ? 'id'). The `_body` param is unused at create
    /// (CC-MAPPING: body=None; body column defaults to NULL).
    ///
    /// Fail-closed (T13-g MEDIUM-2): the None-pool path panics rather than
    /// fabricating a ULID that was never persisted. The panic is caught by the
    /// `catch_unwind` seam in `invoke_through_recovery` and surfaces as a
    /// structured `PluginExecError`, never as a fake-success response.
    fn artifact_create(
        &mut self,
        _ctx: WitWorkspaceCtx,
        type_name: String,
        frontmatter: String,
        _body: Option<String>,
    ) -> String {
        let pool = match self.pool.as_ref() {
            Some(p) => p.clone(),
            None => {
                // PgPool not bound — we cannot persist the artifact. Panicking is
                // the only way to signal failure from a WIT `String` return type.
                // The `catch_unwind` seam in `invoke_through_recovery` catches this
                // panic, repopulates the pool slot, and returns a scrubbed error to
                // the MCP caller (MEDIUM-2, T13-g fail-closed).
                panic!(
                    "artifact_create: PgPool not bound on HostState — \
                     cannot persist artifact, fail closed (T13-g MEDIUM-2)"
                );
            }
        };
        let workspace_id = self.workspace_id;
        let id = ulid::Ulid::new().to_string();
        // Inject the generated ULID as frontmatter['id'] to satisfy
        // CHECK (frontmatter ? 'id') — the caller cannot know the ULID in advance.
        // Build the JSONB-safe frontmatter map from the caller-supplied string.
        //
        // Two cases:
        // 1. `frontmatter` is a valid JSON object string (the common case from
        //    mcp_server tests that pass `json!({...})` payloads) — parse and use
        //    as-is; `id` + `frontmatter_version` are injected below.
        // 2. `frontmatter` is a non-JSON or non-object string (e.g. the e2e test
        //    passes a bare string like "slice1_e2e_payload_..." via dispatch.rs
        //    `json_value_to_payload_string`) — wrap it under the `"content"` key
        //    so it survives the JSONB round-trip. The stored JSON becomes
        //    `{"content":"<string>","id":"...","frontmatter_version":1}` which
        //    satisfies the CHECK constraints AND preserves the payload for `get`.
        let fm_with_id = {
            let mut map: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&frontmatter)
                    .unwrap_or_else(|_| {
                        let mut m = serde_json::Map::new();
                        m.insert(
                            "content".to_owned(),
                            serde_json::Value::String(frontmatter.clone()),
                        );
                        m
                    });
            map.insert("id".to_owned(), serde_json::Value::String(id.clone()));
            map.entry("frontmatter_version".to_owned())
                .or_insert(serde_json::Value::Number(serde_json::Number::from(1u64)));
            serde_json::Value::Object(map).to_string()
        };
        let id2 = id.clone();
        tokio::runtime::Handle::current().block_on(async move {
            sqlx::query(
                "INSERT INTO echo_fixture (id, workspace_id, type, frontmatter) \
                 VALUES ($1, $2, $3, $4::jsonb)",
            )
            .bind(&id2)
            .bind(workspace_id)
            .bind(&type_name)
            .bind(&fm_with_id)
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("artifact_create INSERT failed: {e}"))
        });
        id
    }

    /// `artifact-get` — SELECT from `echo_fixture` WHERE id = $1 AND
    /// workspace_id = $2 (T13, AC#4, R-0006-d). Returns frontmatter text, or
    /// `frontmatter\nbody` when body is present and non-empty.
    ///
    /// Fail-closed (T13-g): a real DB error panics (caught by the seam in
    /// `invoke_through_recovery`) rather than silently returning `None` (which
    /// would be indistinguishable from a legitimate "not found"). A legitimate
    /// `Ok(None)` (row absent or cross-workspace) is returned as `None`.
    fn artifact_get(&mut self, _ctx: WitWorkspaceCtx, id: String) -> Option<String> {
        let pool = match self.pool.as_ref() {
            Some(p) => p.clone(),
            None => {
                tracing::warn!(
                    event = "artifact_get_no_pool",
                    "PgPool not set on HostState — artifact_get returning None"
                );
                return None;
            }
        };
        let workspace_id = self.workspace_id;
        tokio::runtime::Handle::current().block_on(async move {
            let row: Option<(String, Option<String>)> = sqlx::query_as(
                "SELECT frontmatter::text, body \
                 FROM echo_fixture \
                 WHERE id = $1 AND workspace_id = $2",
            )
            .bind(&id)
            .bind(workspace_id)
            .fetch_optional(&pool)
            .await
            // Distinguish a real DB error (panic → seam catches → structured error)
            // from a legitimate not-found/cross-workspace result (Ok(None)).
            .unwrap_or_else(|e| {
                tracing::warn!(
                    event = "artifact_get_error",
                    error = %e,
                    "artifact_get SELECT failed — fail closed (T13-g)"
                );
                panic!("artifact_get SELECT failed: {e}")
            });
            row.map(|(frontmatter_text, body)| match body {
                Some(b) if !b.is_empty() => format!("{frontmatter_text}\n{b}"),
                _ => frontmatter_text,
            })
        })
    }

    /// `artifact-list` — keyset (cursor) page of ids from `echo_fixture` WHERE
    /// workspace_id = $1 AND type = $2, ORDER BY id LIMIT $effective_limit + 1
    /// (T16, R-0020-a/-c/-f, R-0006-d). `_filters` deferred — brain #1846.
    ///
    /// R-0020-d scan-cost backstop (T17): the keyset SELECT runs inside an
    /// EXPLICIT Postgres transaction that first issues `SET LOCAL statement_timeout`
    /// (via the parameterized `set_config(..., is_local := true)` form). The
    /// explicit transaction is mandatory: a `SET LOCAL` outside one binds only to
    /// its own implicit single-statement transaction and is a no-op for the
    /// following query — which would ship the backstop silently disarmed. Setting
    /// the GUC and running the SELECT on the SAME `tx` handle is what arms it.
    ///
    /// On a `statement_timeout` cancellation Postgres raises SQLSTATE 57014
    /// (`query_canceled`); `artifact_list` detects THAT specific error, flags the
    /// per-thread `SCAN_TIMEOUT` side-channel (read by `server.rs` to surface the
    /// distinct `query_scan_timeout` caller code + `outcome = "timeout"` metric),
    /// and fails closed via panic so the page is never a vacuous empty success.
    ///
    /// Fail-closed (T13-g): every other DB error (BEGIN / SET LOCAL / read-back /
    /// non-57014 SELECT / COMMIT) panics (caught by the seam in
    /// `invoke_through_recovery`) rather than silently returning an empty vec
    /// (which would be indistinguishable from a legitimate "no results"). A
    /// legitimate `Ok(vec![])` (no matching rows) is returned as-is.
    fn artifact_list(
        &mut self,
        _ctx: WitWorkspaceCtx,
        type_name: String,
        _filters: String,
        limit: u32,
        cursor: Option<String>,
    ) -> WitArtifactPage {
        let pool = match self.pool.as_ref() {
            Some(p) => p.clone(),
            None => {
                tracing::warn!(
                    event = "artifact_list_no_pool",
                    "PgPool not set on HostState — artifact_list returning empty page"
                );
                return WitArtifactPage {
                    ids: vec![],
                    has_more: false,
                    next_cursor: None,
                };
            }
        };
        let workspace_id = self.workspace_id;
        // R-0020-c: host-side clamp — 0 → default 100, hard cap 500.
        let effective_limit: u32 = if limit == 0 { 100 } else { limit.min(500) };
        // Fetch one extra row to detect has_more (fetch-one-extra idiom, R-0020-a).
        let fetch_limit: u32 = effective_limit + 1;
        // Static SQL strings — two forms so the cursor predicate changes the
        // parameter ordinal ($3 vs shifted $4) with no dynamic string construction.
        // The LIMIT is always a bound parameter (never inlined) to satisfy
        // sqlx::SqlSafeStr. Cursor validation (syntactically valid ULID) has already
        // happened in mcp/server.rs before invoke_content (R-0020-b).
        const SQL_NO_CURSOR: &str = "SELECT id FROM echo_fixture WHERE workspace_id = $1 AND type = $2 \
             ORDER BY id LIMIT $3";
        const SQL_WITH_CURSOR: &str = "SELECT id FROM echo_fixture WHERE workspace_id = $1 AND type = $2 \
             AND id > $3 ORDER BY id LIMIT $4";

        let (ids, has_more, next_cursor) = tokio::runtime::Handle::current().block_on(async move {
            // R-0020-d: open an EXPLICIT transaction so the SET LOCAL
            // statement_timeout binds to the keyset SELECT. begin / set_config /
            // SELECT / commit all run on the same pooled connection inside one
            // BEGIN — a SET LOCAL outside an explicit txn is a no-op (the
            // silent-disarm trap the spec forbids).
            let mut tx = pool.begin().await.unwrap_or_else(|e| {
                tracing::warn!(
                    event = "artifact_list_error",
                    error = %e,
                    "artifact_list BEGIN failed — fail closed (T13-g)"
                );
                panic!("artifact_list BEGIN failed: {e}")
            });

            // The scan-cost backstop ceiling. Production = 3000 ms (the locked
            // value, strictly below the R-0007-b 5 s guest epoch deadline). Under
            // test-hooks the (G1-blocked) cancellation test can force a low value
            // through the knob so the timeout actually fires on a seeded scan.
            let timeout_ms: u32 = {
                #[cfg(feature = "test-hooks")]
                {
                    crate::plugin::sql_observe::effective_statement_timeout_ms(3000)
                }
                #[cfg(not(feature = "test-hooks"))]
                {
                    3000
                }
            };

            // SET LOCAL statement_timeout, parameterized. `SET LOCAL
            // statement_timeout = $1` cannot bind a parameter, but
            // `set_config('statement_timeout', $1, true)` is the equivalent
            // parameterized form — `is_local := true` makes it SET LOCAL (reverts
            // at txn end). statement_timeout reads a bare number as milliseconds,
            // so the value is the ms count as text.
            sqlx::query("SELECT set_config('statement_timeout', $1, true)")
                .bind(timeout_ms.to_string())
                .execute(&mut *tx)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        event = "artifact_list_error",
                        error = %e,
                        "artifact_list SET LOCAL statement_timeout failed — fail closed (T13-g)"
                    );
                    panic!("artifact_list SET LOCAL statement_timeout failed: {e}")
                });

            // R-0020-d part i (test-only): read back current_setting INSIDE the
            // same explicit transaction and record the milliseconds so the
            // white-box GUC-placement assertion (non-zero, == 3000, < 5 s) is
            // observable. The production read path does not pay this round-trip.
            #[cfg(feature = "test-hooks")]
            {
                let setting: (String,) =
                    sqlx::query_as("SELECT current_setting('statement_timeout')")
                        .fetch_one(&mut *tx)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!(
                                event = "artifact_list_error",
                                error = %e,
                                "artifact_list current_setting read-back failed — fail closed (T13-g)"
                            );
                            panic!("artifact_list current_setting read-back failed: {e}")
                        });
                crate::plugin::sql_observe::record_statement_timeout_in_txn(parse_pg_timeout_to_ms(
                    &setting.0,
                ));
            }

            // The keyset SELECT — runs on the SAME `tx` handle, AFTER the SET
            // LOCAL, so the statement_timeout governs it (the load-bearing
            // structure for T22). The SQL is byte-identical to T16's; only the
            // executor changed (&pool -> &mut *tx).
            let select_result: Result<Vec<(String,)>, sqlx::Error> = if let Some(ref c) = cursor {
                #[cfg(feature = "test-hooks")]
                crate::plugin::sql_observe::record_list_sql(
                    SQL_WITH_CURSOR,
                    effective_limit,
                    fetch_limit,
                    true,
                );
                sqlx::query_as(SQL_WITH_CURSOR)
                    .bind(workspace_id)
                    .bind(&type_name)
                    .bind(c)
                    .bind(i64::from(fetch_limit))
                    .fetch_all(&mut *tx)
                    .await
            } else {
                #[cfg(feature = "test-hooks")]
                crate::plugin::sql_observe::record_list_sql(
                    SQL_NO_CURSOR,
                    effective_limit,
                    fetch_limit,
                    false,
                );
                sqlx::query_as(SQL_NO_CURSOR)
                    .bind(workspace_id)
                    .bind(&type_name)
                    .bind(i64::from(fetch_limit))
                    .fetch_all(&mut *tx)
                    .await
            };

            let rows: Vec<(String,)> = match select_result {
                Ok(rows) => rows,
                // R-0020-d part ii: a statement_timeout cancellation raises SQLSTATE
                // 57014 (query_canceled). Detect THAT specific error (not all SELECT
                // errors), flag the per-thread side-channel (read by server.rs to
                // surface the distinct `query_scan_timeout` code + `outcome =
                // "timeout"` metric), and fail closed via panic so the page is never
                // a vacuous empty success. Non-57014 errors keep T16's existing
                // fail-closed panic (→ plugin_invocation_failed); not reclassified.
                Err(e) if is_query_canceled(&e) => {
                    flag_scan_timeout();
                    tracing::warn!(
                        event = "artifact_list_scan_timeout",
                        "artifact_list keyset SELECT canceled by statement_timeout (57014) — scan-cost backstop fired (R-0020-d)"
                    );
                    panic!(
                        "artifact_list keyset SELECT canceled by statement_timeout (R-0020-d)"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        event = "artifact_list_error",
                        error = %e,
                        "artifact_list SELECT failed — fail closed (T13-g)"
                    );
                    panic!("artifact_list SELECT failed: {e}")
                }
            };

            // Commit the explicit transaction (releases the SET LOCAL GUC). The
            // rows are already materialized; commit cannot change them.
            tx.commit().await.unwrap_or_else(|e| {
                tracing::warn!(
                    event = "artifact_list_error",
                    error = %e,
                    "artifact_list COMMIT failed — fail closed (T13-g)"
                );
                panic!("artifact_list COMMIT failed: {e}")
            });

            let mut ids: Vec<String> = rows.into_iter().map(|(id,)| id).collect();
            // Fetch-one-extra (R-0020-a): pop the sentinel row when present.
            // has_more = true iff we received fetch_limit rows (the extra row exists).
            let got_extra = ids.len() == fetch_limit as usize;
            if got_extra {
                ids.pop();
            }
            let has_more = got_extra;
            let next_cursor = if has_more { ids.last().cloned() } else { None };
            (ids, has_more, next_cursor)
        });
        WitArtifactPage {
            ids,
            has_more,
            next_cursor,
        }
    }

    /// `artifact-update` — merge `frontmatter_patch` (jsonb `||`) and COALESCE
    /// `body` WHERE id = $3 AND workspace_id = $4 (T13, R-0006-d). A
    /// missing/cross-workspace target is a no-op (0 rows affected, no error).
    ///
    /// Fail-closed (T13-g HIGH-1): DB errors (e.g. malformed JSONB patch causing
    /// a Postgres CAST error) now panic rather than being swallowed. The panic is
    /// caught by the `catch_unwind` seam in `invoke_through_recovery`, the pool
    /// slot is repopulated, and a structured error is returned to the MCP caller.
    /// The seam discards the panic payload, so no sqlx/table detail leaks.
    fn artifact_update(
        &mut self,
        _ctx: WitWorkspaceCtx,
        id: String,
        frontmatter_patch: String,
        body: Option<String>,
    ) {
        let pool = match self.pool.as_ref() {
            Some(p) => p.clone(),
            None => {
                tracing::warn!(
                    event = "artifact_update_no_pool",
                    "PgPool not set on HostState — artifact_update is a no-op"
                );
                return;
            }
        };
        let workspace_id = self.workspace_id;
        tokio::runtime::Handle::current().block_on(async move {
            sqlx::query(
                "UPDATE echo_fixture \
                 SET frontmatter = frontmatter || $1::jsonb, \
                     body = COALESCE($2, body), \
                     updated_at = now() \
                 WHERE id = $3 AND workspace_id = $4",
            )
            .bind(&frontmatter_patch)
            .bind(body)
            .bind(&id)
            .bind(workspace_id)
            .execute(&pool)
            .await
            // DB error → panic (caught by seam, slot repopulated, scrubbed error
            // to caller). A 0-row result (missing/cross-workspace) is Ok — no-op.
            .unwrap_or_else(|e| {
                tracing::warn!(
                    event = "artifact_update_error",
                    error = %e,
                    "artifact_update UPDATE failed — fail closed (T13-g HIGH-1)"
                );
                panic!("artifact_update UPDATE failed: {e}")
            });
        });
    }

    /// `artifact-delete` — DELETE FROM echo_fixture WHERE id = $1 AND
    /// workspace_id = $2 (T13, R-0006-d). A missing/cross-workspace target is
    /// a no-op (0 rows affected, no error).
    ///
    /// Fail-closed (T13-g): DB errors panic (caught by the seam in
    /// `invoke_through_recovery`, slot repopulated, scrubbed error to MCP caller).
    /// A 0-row result (missing/cross-workspace target) is Ok — no-op.
    fn artifact_delete(&mut self, _ctx: WitWorkspaceCtx, id: String) {
        let pool = match self.pool.as_ref() {
            Some(p) => p.clone(),
            None => {
                tracing::warn!(
                    event = "artifact_delete_no_pool",
                    "PgPool not set on HostState — artifact_delete is a no-op"
                );
                return;
            }
        };
        let workspace_id = self.workspace_id;
        tokio::runtime::Handle::current().block_on(async move {
            sqlx::query("DELETE FROM echo_fixture WHERE id = $1 AND workspace_id = $2")
                .bind(&id)
                .bind(workspace_id)
                .execute(&pool)
                .await
                // DB error → panic (caught by seam, slot repopulated, scrubbed error
                // to caller). A 0-row result (missing/cross-workspace) is Ok — no-op.
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        event = "artifact_delete_error",
                        error = %e,
                        "artifact_delete DELETE failed — fail closed (T13-g)"
                    );
                    panic!("artifact_delete DELETE failed: {e}")
                });
        });
    }
}

impl mnemra::host::echo::Host for HostState {
    /// `echo` — retained import body; NOT on the slice-1 `content` path.
    fn echo(&mut self, s: String) -> String {
        format!("echo: {s}")
    }

    /// `increment-counter` — retained import body; NOT on the slice-1 path.
    /// Per-instance state is not modelled here (the slice-1 path never calls it).
    fn increment_counter(&mut self) -> u32 {
        0
    }
}

// ---------------------------------------------------------------------------
// Linker construction
// ---------------------------------------------------------------------------

/// Build a component `Linker<HostState>` that registers the host-fn import
/// bodies and trap-stubs every other import of `component` (R-0012-a, R-0006-a).
///
/// The wired imports (`artifact`, `echo`) come from the bindgen-generated
/// `add_to_linker`; the remaining imports (WASI) are filled with trapping stubs
/// via `define_unknown_imports_as_traps` — a call to an unreached import traps
/// rather than silently succeeding. The slice-1 `content.create`/`content.get`
/// path calls only `artifact-create`/`artifact-get`, both genuinely wired.
pub fn build_linker(
    engine: &wasmtime::Engine,
    component: &Component,
) -> Result<Linker<HostState>, Box<dyn std::error::Error>> {
    let mut linker: Linker<HostState> = Linker::new(engine);

    // Allow the real host-fn bodies to SHADOW the trap-stubs. Order matters:
    //   1. trap-stub EVERY import (including WASI, `types`, `artifact`, `echo`);
    //   2. add the real host-fn bodies, which shadow the stubs for the mnemra
    //      interfaces, leaving the unreached WASI imports as traps.
    //
    // This ordering is required because `define_unknown_imports_as_traps`
    // unconditionally re-creates *instance* imports (it does not skip already-
    // defined instances), so running it AFTER `add_to_linker` errors with
    // "map entry `mnemra:host/types` defined twice". Stubbing first + shadowing
    // the mnemra interfaces with real bodies second gives: real `artifact` bodies
    // on the reachable path, WASI as traps on the unreachable path.
    linker.allow_shadowing(true);

    // 1. Trap-stub all imports (WASI + the mnemra instances, transiently).
    linker.define_unknown_imports_as_traps(component)?;

    // 2. Register the real host-fn import bodies generated by bindgen, shadowing
    // the stubs for `artifact` / `echo` / `types`. `HasSelf<_>` says the store
    // data IS the host type (no projection).
    Plugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| state)?;

    Ok(linker)
}

/// Instantiate `component` on `linker` with `store`, returning the raw component
/// `Instance` (R-0016-b). The pool holds the raw `Instance` (not the typed
/// `Plugin` world handle) so it can hold BOTH real content plugins (export
/// `content`) and the trap fixtures (export a bare `run`) uniformly — `Plugin`
/// instantiation requires the `content` export, which the trap fixtures lack.
/// The typed `content` invoke is obtained on demand from the raw instance via
/// `content_create` / `content_get`; the trap path uses `call_run`.
pub fn instantiate(
    store: &mut Store<HostState>,
    component: &Component,
    linker: &Linker<HostState>,
) -> Result<Instance, Box<dyn std::error::Error>> {
    let instance = linker.instantiate(&mut *store, component)?;
    Ok(instance)
}

/// Build a `Store<HostState>` with the memory limiter, unbound to a workspace
/// until the dispatch site sets it (R-0006-b). Pool is also unbound until
/// `set_pool` is called at the dispatch site (T13).
pub fn new_store(engine: &wasmtime::Engine) -> Store<HostState> {
    let mut store = Store::new(engine, HostState::unbound());
    store.limiter(|state| state as &mut dyn ResourceLimiter);
    store
}

/// Invoke the typed `content.create` export on a raw component `Instance`
/// (R-0019-a). Wraps the instance in the bindgen `Plugin` handle and calls the
/// generated typed accessor, so marshalling is the bindgen-generated canonical
/// ABI — no hand-rolled lowering. Returns the host-generated id, or any trap.
pub fn content_create(
    store: &mut Store<HostState>,
    instance: &Instance,
    type_name: &str,
    frontmatter: &str,
    body: Option<&str>,
) -> wasmtime::Result<String> {
    let plugin = Plugin::new(&mut *store, instance)?;
    let content = plugin.mnemra_host_content();
    // The bindgen-generated typed accessor takes `&String` for the required
    // string params (not `&str`); `to_owned()` is required to materialize the
    // owned value the borrow points to. clippy's `unnecessary_to_owned` is a
    // false positive here — `&str` does not satisfy the `&String` parameter.
    #[allow(clippy::unnecessary_to_owned)]
    content.call_create(
        &mut *store,
        &type_name.to_owned(),
        &frontmatter.to_owned(),
        body,
    )
}

/// Invoke the typed `content.get` export on a raw component `Instance`
/// (R-0019-a). Returns the stored content (round-tripping the payload) or `None`.
pub fn content_get(
    store: &mut Store<HostState>,
    instance: &Instance,
    id: &str,
) -> wasmtime::Result<Option<String>> {
    let plugin = Plugin::new(&mut *store, instance)?;
    let content = plugin.mnemra_host_content();
    // See `content_create`: the accessor takes `&String`, not `&str`.
    #[allow(clippy::unnecessary_to_owned)]
    content.call_get(&mut *store, &id.to_owned())
}

/// Invoke the typed `content.list` export on a raw component `Instance`
/// (R-0019-a, R-0020). Returns an `ArtifactPage` with the ids of `type_name`
/// artifacts visible in the caller's workspace (the guest body calls back into
/// the host `artifact-list` import, which scopes on the host-derived
/// `workspace_id`). `filters` is threaded but not applied this slice (predicate
/// logic deferred — brain #1846). `limit` and `cursor` are forwarded to the
/// guest; T14 host body returns placeholder paging only (has-more=false,
/// next-cursor=none) — no keyset/clamp/cursor logic at this layer.
pub fn content_list(
    store: &mut Store<HostState>,
    instance: &Instance,
    type_name: &str,
    filters: &str,
    limit: u32,
    cursor: Option<&str>,
) -> wasmtime::Result<ArtifactPage> {
    let plugin = Plugin::new(&mut *store, instance)?;
    let content = plugin.mnemra_host_content();
    // See `content_create`: the accessor takes `&String`, not `&str`.
    #[allow(clippy::unnecessary_to_owned)]
    let wit_page = content.call_list(
        &mut *store,
        &type_name.to_owned(),
        &filters.to_owned(),
        limit,
        cursor,
    )?;
    Ok(ArtifactPage {
        ids: wit_page.ids,
        has_more: wit_page.has_more,
        next_cursor: wit_page.next_cursor,
    })
}

/// Invoke the typed `content.update` export on a raw component `Instance`
/// (R-0019-a). The guest body calls back into the host `artifact-update` import,
/// which merges the frontmatter patch and (when `body` is `Some`) replaces the
/// body in the fenced map, scoped on the host-derived `workspace_id` (R-0006-d).
/// `update` is void; this returns `()` on success (or any trap).
pub fn content_update(
    store: &mut Store<HostState>,
    instance: &Instance,
    id: &str,
    frontmatter_patch: &str,
    body: Option<&str>,
) -> wasmtime::Result<()> {
    let plugin = Plugin::new(&mut *store, instance)?;
    let content = plugin.mnemra_host_content();
    // See `content_create`: the accessor takes `&String`, not `&str`.
    #[allow(clippy::unnecessary_to_owned)]
    content.call_update(
        &mut *store,
        &id.to_owned(),
        &frontmatter_patch.to_owned(),
        body,
    )
}

/// Invoke the typed `content.delete` export on a raw component `Instance`
/// (R-0019-a). The guest body calls back into the host `artifact-delete` import,
/// which removes the artifact keyed `(workspace_id, id)` from the fenced map;
/// a missing/cross-workspace target is a silent no-op (R-0006-d).
/// `delete` is void; this returns `()` on success (or any trap).
pub fn content_delete(
    store: &mut Store<HostState>,
    instance: &Instance,
    id: &str,
) -> wasmtime::Result<()> {
    let plugin = Plugin::new(&mut *store, instance)?;
    let content = plugin.mnemra_host_content();
    // See `content_create`: the accessor takes `&String`, not `&str`.
    #[allow(clippy::unnecessary_to_owned)]
    content.call_delete(&mut *store, &id.to_owned())
}
