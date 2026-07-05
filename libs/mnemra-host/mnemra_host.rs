//! mnemra-host — runtime host library.
//!
//! Exposes the startup entry point that the `mnemra` binary calls.
//! This is a skeleton; subsequent tasks extend it:
//!   - Tasks 5–7:  storage initialisation
//!   - Task 23:    MCP server startup
//!   - Tasks 19/20: plugin-runtime integration

pub mod abi;
pub mod auth;
pub mod builtins;
// Task 18: per-deployment config (LLM-key, R-0014-a/b/c).
pub mod config;
// Task 18: outbound hostname allowlist for embedding-call pathway (R-0014-b).
pub mod net;
pub mod projection;
pub mod schema;
// Task 25 / T4-T5 (R-0022-b, R-0004-g): /health loopback HTTP listener —
// wraps schema::init::health_snapshot in a minimal, loopback-only HTTP/1.1
// listener with a readiness gate. Bound first by run() (5a boundary, T5).
pub mod health;
// Task 17: plugin signing-chain verification + embedded root (R-0005).
pub mod signing;
pub mod storage;
// Task 17: host startup invariant checks — file-mode gate before plugin load (R-0005-f).
// Task 18: extended to cover LLM-key file as well (R-0014-d).
pub mod startup;
// Task 23: MCP server — rmcp ServerHandler impl, DF-auth-check, WorkspaceCtx,
// per-verb capability check (R-0010-a/b/c/d/f/g).
pub mod mcp;
// Task 21: plugin runtime — manifest load pipeline, allowlists, wasmtime limits,
// epoch-tick supervisor, and instance pool (R-0003-b/c/f, R-0007, R-0010-d, R-0016-a/b).
pub mod plugin;

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// T5 (R-0022-a/-e) — production startup assembly surface.
//
// RED phase (task #1992): the types below are the cross-dispatch contract the
// startup-ordering acceptance tests (tests/startup_run_ordering.rs,
// tests/startup_run_full.rs) compile against. `run_with` is a `todo!()`
// skeleton; the GREEN dispatch implements the body against the locked
// ordering (spec R-0022-a boundaries 5-pre → accept-last) WITHOUT editing
// the tests or these public signatures.
//
// Execution-order note for GREEN (reconciling the spec's boundary labels
// with the code's data dependencies): `builtins::init_all(pool)` requires a
// live `PgPool`, so storage init necessarily *precedes* builtin init at
// runtime. The spec's boundary constraints all still hold under the order
//   (5-pre) file-mode check → (5a) /health bind → storage + schema init
//   (5c: before server construction) → (5b-i) seven builtins (before any
//   plugin load) → (5b-ii) verified pool → construct server → mark ready →
//   return (accept-last: no serve-loop; T6 owns serving).
// The boundary letters are labels, not a literal execution sequence.
// ---------------------------------------------------------------------------

/// Shared log token for the 5b-i builtin-ordering observable (R-0022-a).
///
/// GREEN's contract: emit a `tracing` event whose message contains this
/// token, on the `run_with` call path (not a detached thread/task),
/// immediately before invoking the verified-load gate. The acceptance tests
/// assert this token's *presence* on a successful startup and its *absence*
/// when a builtin-init failure is injected — the absence is what makes
/// "builtin failure → no plugin load attempted" black-box observable.
pub const PLUGIN_LOAD_LOG_LINE: &str = "plugin load";

/// Failure to inject at a named startup boundary (test seam, R-0022-a).
///
/// Gated behind the `test-hooks` feature so the seam is unreachable in the
/// default/production build (`tests/no_test_seams.rs` enforces this via a
/// trybuild fixture). GREEN's injection-point contract:
///
/// - `StorageInit`: at the storage-init step, *in place of* booting the
///   embedded engine + schema init + `records` bootstrap, fail with
///   `RunError::Storage(..)`. `/health` is already bound (5a) and its
///   listener stays up answering not-ready after `run_with` returns `Err`.
/// - `BuiltinInit`: after real storage init succeeds, at the builtin step,
///   *in place of* `builtins::init_all`'s result, fail with
///   `RunError::BuiltinInit(..)` — before any plugin-load attempt (no
///   [`PLUGIN_LOAD_LOG_LINE`] event, no `PluginPool`).
#[cfg(feature = "test-hooks")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectedFailure {
    /// Fail at the builtin-init boundary (5b-i).
    BuiltinInit,
    /// Fail at the storage-init boundary (5c).
    StorageInit,
}

/// Configuration for [`run_with`] — the production startup assembly's inputs.
///
/// Injectable so the startup-ordering tests can lay out temp-dir fixtures
/// without `env::set_var` (workspace canon: TF2). Production (`run()`, GREEN)
/// constructs this from its environment (repo root, R-0008-e token path
/// resolution, `MNEMRA_HEALTH_PORT` via `health::resolve_port`).
#[derive(Debug)]
pub struct RunConfig {
    /// Root directory the integrity-gated load path resolves against
    /// (R-0022-e): the signed manifest is read from
    /// `<root_dir>/plugins/mnemra-echo/manifest.toml` and the signed
    /// component artifact from
    /// `<root_dir>/artifacts/mnemra-echo/mnemra_echo.wasm` — never a
    /// `target/` rebuild.
    pub root_dir: PathBuf,
    /// The admin-token file the 5-pre startup file-mode invariant check
    /// (R-0005-f instantiation) runs over, before any listener binds.
    pub admin_token_path: PathBuf,
    /// Address for the `/health` loopback listener (5a). Tests pass port 0
    /// (OS-assigned) and read the bound address from
    /// [`RunHandle::health_addr`].
    pub health_addr: SocketAddr,
    /// Failure injection (test seam) — absent from the default build.
    #[cfg(feature = "test-hooks")]
    pub inject_failure: Option<InjectedFailure>,
    /// The tracing span current when this config was constructed.
    ///
    /// Standard cross-thread span propagation: `run_with` may be driven on
    /// a different thread/runtime than the context that initiated startup
    /// (a supervisor thread, the acceptance harness's worker thread).
    /// Startup events — notably the [`PLUGIN_LOAD_LOG_LINE`] observable —
    /// are emitted with this span as their explicit parent so they stay
    /// attributed to the initiating context instead of appearing as
    /// orphaned root events on whichever thread happens to drive the
    /// future. Captured by [`RunConfig::new`] via `tracing::Span::current()`
    /// (a no-op disabled span when no subscriber is active).
    caller_span: tracing::Span,
}

impl RunConfig {
    /// Build a config with no failure injection.
    pub fn new(
        root_dir: impl Into<PathBuf>,
        admin_token_path: impl Into<PathBuf>,
        health_addr: SocketAddr,
    ) -> RunConfig {
        RunConfig {
            root_dir: root_dir.into(),
            admin_token_path: admin_token_path.into(),
            health_addr,
            #[cfg(feature = "test-hooks")]
            inject_failure: None,
            caller_span: tracing::Span::current(),
        }
    }

    /// Inject a failure at a named startup boundary (test seam; see
    /// [`InjectedFailure`] for GREEN's injection-point contract).
    #[cfg(feature = "test-hooks")]
    pub fn with_injected_failure(mut self, failure: InjectedFailure) -> RunConfig {
        self.inject_failure = Some(failure);
        self
    }
}

/// Handle returned by a successful [`run_with`] (accept-last boundary).
///
/// Owns whatever must stay alive for the constructed-but-not-serving host:
/// the embedded storage engine, the pool adapter, the verified pool, and the
/// `/health` listener's liveness are all tied to this value (GREEN chooses
/// the exact private representation — it must remain `Send`; the tests move
/// it across a thread boundary). No serve-loop runs while it is held: T6 owns
/// serving.
///
/// # Field drop order (Tier-2 T4 refactor, R-0037)
///
/// `_storage` and `_server` each hold a clone of `_engine`'s pool. `_engine`
/// owns the embedded Postgres process (moved out of `PostgresStorage`, which
/// is a pure pool adapter as of this refactor); the process must outlive
/// every pool-holding value that might still touch it during teardown. The
/// order is enforced by the explicit `impl Drop` below (`take()`-before-
/// `_engine`), not by field declaration order — reordering the fields listed
/// above can no longer silently regress teardown. A newly added pool-holding
/// field must join the `take()` list to get this guarantee — see the
/// maintenance note on `impl Drop` below.
pub struct RunHandle {
    /// The address the `/health` listener bound (may be OS-assigned).
    health_addr: SocketAddr,
    /// The pool adapter — holds a clone of `_engine.pool`. `Option` so
    /// `impl Drop` can `take()` and drop it explicitly BEFORE `_engine` (see
    /// field-order note above); `Some` from construction until drop.
    _storage: Option<storage::postgres::PostgresStorage>,
    /// The constructed MCP server (accept-last: not yet served — T6 owns
    /// the stdio serve-loop and extends this handle to reach it). Also holds
    /// a pool clone; `Option` for the same reason as `_storage` — dropped
    /// explicitly BEFORE `_engine`. `serve_stdio` also `take()`s this to move
    /// the server out for serving.
    _server: Option<mcp::server::MnemraMcpServer>,
    /// Keeps the embedded Postgres engine — and the server process it
    /// owns — alive for the handle's lifetime. Composition-root-owned as of
    /// the Tier-2 T4 refactor: `PostgresStorage` no longer boots or owns an
    /// engine. Dropped last by `impl Drop`'s explicit ordering.
    _engine: storage::postgres::engine::EmbeddedEngine,
}

/// Enforces the R-0037 teardown order structurally: `_storage` and `_server`
/// are dropped explicitly, before `self`'s remaining fields (`_engine`) drop
/// via the normal end-of-`drop` field teardown. `Option::take` makes the
/// order independent of field declaration order.
///
/// Maintenance obligation: any new field that holds a clone of `_engine`'s
/// pool must be added to this `take()` list. Leave it out and, if the field
/// is declared after `_engine`, it drops too late — after the postmaster has
/// already been torn down.
impl Drop for RunHandle {
    fn drop(&mut self) {
        // Drop the pool-holding fields before `_engine` tears down the
        // embedded postmaster they clone (R-0037). Explicit `take()` makes
        // the order independent of field declaration order — a future
        // reorder can no longer silently regress teardown.
        drop(self._storage.take());
        drop(self._server.take());
    }
}

impl RunHandle {
    /// The address the `/health` listener actually bound — the tests pass
    /// port 0 and discover the OS-assigned port here.
    pub fn health_addr(&self) -> SocketAddr {
        self.health_addr
    }

    /// Serve the constructed [`MnemraMcpServer`] over a production **stdio**
    /// MCP transport (R-0022-c).
    ///
    /// Consumes the handle: the `/health` listener thread and the embedded
    /// storage engine this handle was keeping alive (`self._engine`, and the
    /// pool adapter `self._storage`) are no longer owned by anyone once this
    /// returns. Only `self._server` is moved out for serving (via
    /// `Option::take` — `RunHandle` implements `Drop`, so a field can no
    /// longer be moved out of `self` by value); the remaining fields
    /// (notably `_storage` and `_engine`) stay owned by `self` — and
    /// therefore alive — for this call's whole duration, including across the
    /// `.await` below. `self`'s `impl Drop` runs at the end of this method
    /// (or on early return via `?`), after the `.await`s complete.
    ///
    /// Wires `self._server` to `rmcp::transport::io::stdio()` — the
    /// `(tokio::io::Stdin, tokio::io::Stdout)` pair that satisfies the
    /// generic `(R, W)` `IntoTransport` blanket impl (`transport-io`
    /// feature, already declared) — via `rmcp::service::serve_server`, then
    /// awaits the returned `RunningService` to completion. No HTTP MCP
    /// transport is opened (R-0010-e — the `transport-streamable-http-*`
    /// rmcp features are not compiled in, `tests/mcp_feature_guard.rs`).
    ///
    /// `cmd/mnemra/logging.rs` writes its `tracing` JSON logs to **stderr**
    /// (not stdout), so stdout stays reserved exclusively for the JSON-RPC
    /// wire protocol this method serves — required for `run_with`'s startup
    /// events and every `MnemraMcpServer` `verb_metric` event (unconditional
    /// per served tool call) not to corrupt the handshake / stream.
    ///
    /// Returns `Ok(())` once the stdio transport closes (peer EOF on
    /// stdin — `RunningService::waiting()` resolving `Ok(_)`, regardless of
    /// `QuitReason` variant, is this method's definition of a clean
    /// shutdown; there is no separate signal to distinguish). Returns
    /// `Err(ServeError::Mcp)` if the initial MCP handshake fails
    /// (`ServerInitializeError`) or the background serve task panics/is
    /// aborted (`tokio::task::JoinError`) — a per-request protocol error
    /// inside a handler is caught and returned to the peer by `rmcp`
    /// internals and never surfaces here.
    pub async fn serve_stdio(mut self) -> Result<(), ServeError> {
        let server = self
            ._server
            .take()
            .expect("_server is Some from construction until drop");
        let running = rmcp::service::serve_server(server, rmcp::transport::io::stdio())
            .await
            .map_err(|e| ServeError::Mcp(Box::new(e)))?;
        running
            .waiting()
            .await
            .map_err(|e| ServeError::Mcp(Box::new(e)))?;
        Ok(())
    }
}

/// Reasons the production stdio MCP serve-loop (R-0022-c) can fail —
/// GREEN (T6) implements [`RunHandle::serve_stdio`] against this skeleton.
#[derive(Debug)]
#[non_exhaustive]
pub enum ServeError {
    /// The stdio MCP transport failed — during the `initialize` handshake
    /// or at any point while serving (an rmcp transport/protocol error on
    /// the SERVER side). Boxed rather than a concrete rmcp error type: the
    /// exact rmcp error this wraps is GREEN's implementation choice; the
    /// skeleton fixes only the public shape callers (`run()`) see.
    Mcp(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServeError::Mcp(e) => write!(f, "stdio MCP serve loop failed (R-0022-c): {e}"),
        }
    }
}

impl std::error::Error for ServeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ServeError::Mcp(e) => Some(e.as_ref()),
        }
    }
}

/// Reasons production startup can fail — one variant per fail-closed
/// boundary of the locked ordering (R-0022-a).
#[derive(Debug)]
#[non_exhaustive]
pub enum RunError {
    /// 5-pre: the admin-token file failed the startup file-mode invariant
    /// check (R-0005-f) — the host refused to start before binding any
    /// listener.
    FileMode(startup::file_mode_check::FileModeError),
    /// 5a: the `/health` loopback listener could not be bound.
    HealthBind(health::HealthBindError),
    /// 5c: storage init (embedded-engine boot + `records` bootstrap + schema
    /// init) failed — the server is never constructed; `/health` (already
    /// bound) reports not-ready.
    Storage(Box<dyn std::error::Error + Send + Sync>),
    /// 5b-i: a builtin failed to initialize — no plugin load is attempted,
    /// no `PluginPool` is constructed.
    BuiltinInit(builtins::BuiltinInitError),
    /// 5b-ii: the verified pool could not be populated (signature,
    /// content-hash, or component-load failure — R-0021).
    PluginLoad(startup::StartupError),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::FileMode(e) => write!(f, "startup refused (file-mode check, 5-pre): {e}"),
            RunError::HealthBind(e) => write!(f, "startup failed (/health bind, 5a): {e}"),
            RunError::Storage(e) => write!(f, "startup failed closed (storage init, 5c): {e}"),
            RunError::BuiltinInit(e) => {
                write!(f, "startup failed closed (builtin init, 5b-i): {e}")
            }
            RunError::PluginLoad(e) => {
                write!(f, "startup failed closed (verified pool, 5b-ii): {e}")
            }
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RunError::FileMode(e) => Some(e),
            RunError::HealthBind(e) => Some(e),
            RunError::Storage(e) => Some(e.as_ref()),
            RunError::BuiltinInit(e) => Some(e),
            RunError::PluginLoad(e) => Some(e),
        }
    }
}

/// Production startup assembly (R-0022-a) — T5 GREEN implements.
///
/// Performs the locked startup ordering (see the module-level execution-order
/// note) and returns a [`RunHandle`] after construction **without serving**
/// (accept-last; the stdio serve-loop is T6). On failure, returns the
/// boundary's [`RunError`] variant; a failure after the 5a bind leaves the
/// `/health` listener up and answering not-ready (503 + `{"ready": false}`,
/// the sanctioned T4 contract).
pub async fn run_with(config: RunConfig) -> Result<RunHandle, RunError> {
    // 5-pre (R-0005-f instantiation): the admin-token file-mode invariant
    // check runs FIRST — before any listener binds.
    startup::file_mode_check::check_admin_token(&config.admin_token_path)
        .map_err(RunError::FileMode)?;

    // 5a (R-0004-g / R-0022-b): bind /health before config load and before
    // MCP accept. The listener is spawned onto a dedicated OS thread right
    // after binding, so it outlives any failure below (failure-path
    // contract: /health keeps answering not-ready after `run_with` returns
    // `Err`).
    let (readiness_handle, readiness_signal) = health::ReadinessHandle::new();
    let pool_cell = health::PoolCell::empty();
    let listener =
        health::HealthListener::bind(config.health_addr, readiness_signal, pool_cell.clone())
            .map_err(RunError::HealthBind)?;
    let health_addr = listener.local_addr();
    std::thread::spawn(move || {
        if let Err(err) = listener.serve() {
            tracing::warn!(error = %err, "/health listener accept loop exited");
        }
    });

    // 5c (R-0022-d): boot the embedded engine, bootstrap the `records`
    // table, then run schema init — all before builtins and before the
    // server is constructed. Composition-root-owned as of the Tier-2 T4
    // refactor (R-0037): `PostgresStorage` is a pure pool adapter and no
    // longer boots or owns an engine, so the engine is booted directly HERE
    // and the resulting pool is injected into `PostgresStorage::new`. Schema
    // init needs `&EmbeddedEngine` directly (the superuser path for
    // `ensure_extension` / `create_least_privilege_roles`).
    #[cfg(feature = "test-hooks")]
    if config.inject_failure == Some(InjectedFailure::StorageInit) {
        let injected: Box<dyn std::error::Error + Send + Sync> =
            "test-hooks: injected storage-init failure (InjectedFailure::StorageInit)".into();
        return Err(RunError::Storage(injected));
    }
    let engine = storage::postgres::engine::EmbeddedEngine::start()
        .await
        .map_err(RunError::Storage)?;
    storage::postgres::bootstrap_records_table(engine.pool.as_ref())
        .await
        .map_err(RunError::Storage)?;
    schema::init::init(&engine, "vector")
        .await
        .map_err(|e| RunError::Storage(Box::new(e)))?;
    let pg_pool = engine.pool.as_ref().clone();
    let storage = storage::postgres::PostgresStorage::new(Arc::clone(&engine.pool));

    // 5b-i (R-0002-c): all seven builtins before any plugin load.
    #[cfg(feature = "test-hooks")]
    if config.inject_failure == Some(InjectedFailure::BuiltinInit) {
        let injected: Box<dyn std::error::Error + Send + Sync> =
            "test-hooks: injected builtin-init failure (InjectedFailure::BuiltinInit)".into();
        return Err(RunError::BuiltinInit(builtins::BuiltinInitError::Db(
            injected,
        )));
    }
    let _builtins_ready = builtins::init_all(&pg_pool)
        .await
        .map_err(RunError::BuiltinInit)?;

    // Shared 5b-i observable: emitted on THIS call path, immediately before
    // the verified-load gate. Its *absence* is what makes "a builtin-init
    // failure means no plugin load was attempted" black-box observable.
    // Parented to the initiating context's span (see `RunConfig.caller_span`)
    // so the event stays attributed to that context even when this future is
    // driven on another thread.
    tracing::info!(parent: &config.caller_span, "{}", PLUGIN_LOAD_LOG_LINE);

    // 5b-ii (R-0016-a / R-0022-e): the verified plugin pool, before MCP
    // accept. Feeds the real on-disk manifest + the signed component
    // artifact under `config.root_dir` (decoupling-reversal) against the
    // real embedded root.
    let plugin_pool =
        startup::populate_verified_pool_from_dir(&config.root_dir, signing::root_material::ROOT)
            .map_err(RunError::PluginLoad)?;

    // Construct the server — 5c precedes construction; 5b-i/5b-ii precede
    // any plugin load. Accept-last: no MCP transport is opened here (T6).
    let server = mcp::server::MnemraMcpServer::new(pg_pool.clone(), plugin_pool);

    // Mark ready: supply the pool for /health's detail body, then flip the
    // readiness flag.
    //
    // `PoolCell` gets a SEPARATE lazily-connecting pool, not `pg_pool`
    // itself: `health::HealthListener::serve()` drives its queries on its
    // own dedicated tokio runtime (health.rs doc comment — a different
    // runtime than the one driving this function, and in a worker-thread
    // harness that runtime may already be gone by the time a request
    // lands). A pool whose connections were eagerly established under THIS
    // runtime is bound to it; a lazy pool built from the same connect
    // options defers connection establishment to first use, so it connects
    // under whichever runtime actually queries it.
    // `max_lifetime(None)`/`idle_timeout(None)` avoid `PoolInner::new_arc`'s
    // unconditional maintenance-task spawn at construction time (sqlx-core
    // 0.9.0 `spawn_maintenance_tasks`) — the same recipe
    // `tests/health_listener.rs::unreachable_lazy_pool` documents and uses.
    let health_pool = sqlx::postgres::PgPoolOptions::new()
        .max_lifetime(None)
        .idle_timeout(None)
        .connect_lazy_with((*pg_pool.connect_options()).clone());
    pool_cell.set(health_pool);
    readiness_handle.mark_ready();

    Ok(RunHandle {
        health_addr,
        _storage: Some(storage),
        _server: Some(server),
        _engine: engine,
    })
}

/// Start the mnemra host runtime.
///
/// Delegates to [`run_with`] over a production-derived [`RunConfig`]:
/// `root_dir` resolves to the workspace root (same-machine build==run V0
/// convention — mirrors `startup::populate_verified_pool`'s
/// `echo_component_path()` resolution); `admin_token_path` resolves via
/// `auth::token::default_token_file_path()` (R-0008-e); `health_addr` binds
/// loopback on the port `health::resolve_port` resolves from
/// `MNEMRA_HEALTH_PORT`.
///
/// Construction (`run_with`) completes, then this serves the production
/// stdio MCP loop (R-0022-c) via [`RunHandle::serve_stdio`].
///
/// Returns once the serve loop ends (peer EOF on stdio — R-0010-a; no
/// serve-loop error) or propagates the boundary's error.
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root_dir = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root resolution from libs/mnemra-host — layout must not change")
        .to_path_buf();

    let admin_token_path = auth::token::default_token_file_path().ok_or(
        "admin-token path could not be resolved: HOME is not set and MNEMRA_TOKEN_FILE is not set",
    )?;

    let health_port =
        health::resolve_port(std::env::var(health::HEALTH_PORT_ENV_VAR).ok().as_deref());
    let health_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, health_port));

    let handle = run_with(RunConfig::new(root_dir, admin_token_path, health_addr)).await?;
    handle.serve_stdio().await?;
    Ok(())
}
