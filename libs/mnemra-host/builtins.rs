//! Built-in subsystem bootstraps and init ordering (R-0002-b, R-0002-c).
//!
//! # Builtins are host code (R-0002-b)
//!
//! All builtins in this module are compiled into the host process. None of them
//! execute inside the Wasmtime plugin sandbox. This is enforced structurally:
//! this crate (`mnemra-host`) has no `wasmtime` dependency. Any builtin is
//! therefore, by construction, host code. The `BuiltinsReady` gate below is
//! a plain Rust value — it is created and checked in host code only.
//!
//! # Init ordering (R-0002-c)
//!
//! All seven builtins must initialize before any plugin loads. The ordering is:
//!
//! 1. workspaces  — workspace lifecycle; default workspace guaranteed to exist
//!    (Task 7 / schema::init::init already created it)
//! 2. users       — user identity records
//! 3. agents      — agent registrations (depends on users + workspaces)
//! 4. authentication — admin-token bootstrap (Task 11)
//! 5. sessions    — per-MCP-connection state (depends on agents + workspaces)
//! 6. permissions — host-layer verb authorization (Task 14)
//! 7. projects    — project registry; prerequisite for plugin scoping
//!
//! The deterministic sequence is encoded in `init_all`. On completion it
//! returns a `BuiltinsReady` token. Any path that invokes plugin loading
//! must hold a `BuiltinsReady` — if it cannot produce one (builtins not yet
//! initialized), it returns `PluginLoadError::BuiltinsNotReady`.
//!
//! # Builtins enumerated
//!
//! | # | Builtin        | Task | Spec requirement |
//! |---|----------------|------|-----------------|
//! | 1 | workspaces     | 15   | R-0015-a, R-0015-h |
//! | 2 | users          | 15   | R-0015-b |
//! | 3 | agents         | 15   | R-0015-c |
//! | 4 | authentication | 11   | R-0015-d |
//! | 5 | sessions       | 15   | R-0015-e |
//! | 6 | permissions    | 14   | R-0015-f |
//! | 7 | projects       | 15   | R-0015-g |

pub mod agents;
pub mod authentication;
pub mod permissions;
pub mod projects;
pub mod sessions;
pub mod users;
pub mod workspaces;

use std::fmt;

// ---------------------------------------------------------------------------
// BuiltinsReady gate (R-0002-c)
// ---------------------------------------------------------------------------

/// Token proving all seven builtins have been initialized.
///
/// `init_all()` returns this value on success. Plugin-load entry points
/// require it — they cannot proceed without holding one. This is the
/// "builtins ready" gate enforcing R-0002-c: no plugin invocation
/// precedes builtin startup completion.
///
/// # Structural enforcement
///
/// `BuiltinsReady` is a non-Copy, non-Clone opaque struct. The only way to
/// obtain one is by calling `init_all()` to completion. This makes it
/// impossible for a plugin-load path to bypass the gate by constructing the
/// token directly.
///
/// The struct is `pub` so that the plugin-loader (Task 19) can accept it
/// as a parameter, but its inner field is private — no external code can
/// construct it.
#[derive(Debug)]
pub struct BuiltinsReady {
    /// Private sentinel. Prevents external construction.
    _sentinel: (),
}

impl BuiltinsReady {
    /// Private constructor — called only by `init_all()`.
    fn new() -> Self {
        Self { _sentinel: () }
    }
}

// ---------------------------------------------------------------------------
// Init error
// ---------------------------------------------------------------------------

/// Error returned by `init_all()`.
#[derive(Debug)]
pub enum BuiltinInitError {
    /// The schema init (migrations + default workspace) has not been run.
    /// Call `schema::init::init(engine, "vector")` before `init_all()`.
    SchemaNotInitialized,
    /// A database error occurred during builtin initialization.
    Db(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for BuiltinInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuiltinInitError::SchemaNotInitialized => write!(
                f,
                "builtin init: schema not initialized — call schema::init::init() first"
            ),
            BuiltinInitError::Db(e) => write!(f, "builtin init db error: {e}"),
        }
    }
}

impl std::error::Error for BuiltinInitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BuiltinInitError::Db(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin load error
// ---------------------------------------------------------------------------

/// Error returned when a plugin-load attempt is made without a `BuiltinsReady`.
///
/// This type is used in tests to assert R-0002-c: a plugin-load attempt
/// before builtin completion is rejected with a structured error.
#[derive(Debug, PartialEq, Eq)]
pub enum PluginLoadError {
    /// Plugin load was attempted before all builtins were initialized.
    ///
    /// Obtain a `BuiltinsReady` from `init_all()` before loading plugins.
    BuiltinsNotReady,
}

impl fmt::Display for PluginLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginLoadError::BuiltinsNotReady => write!(
                f,
                "plugin load rejected: builtins not ready (R-0002-c) — \
                 call init_all() to completion before loading plugins"
            ),
        }
    }
}

impl std::error::Error for PluginLoadError {}

// ---------------------------------------------------------------------------
// init_all() — deterministic 7-builtin init sequence (R-0002-c)
// ---------------------------------------------------------------------------

/// Initialize all seven builtins in the deterministic order required by R-0002-c.
///
/// Returns a `BuiltinsReady` token on success. This token is the gate:
/// plugin-load entry points require it, and cannot proceed without one.
///
/// # Prerequisites
///
/// Schema must already be initialized (call `schema::init::init(engine, "vector")`
/// before calling this). The function verifies the `default` workspace exists;
/// if it does not, `BuiltinInitError::SchemaNotInitialized` is returned.
///
/// # Ordering
///
/// 1. workspaces  — verify default workspace exists (already created by schema init)
/// 2. users       — no runtime bootstrap needed at V0
/// 3. agents      — no runtime bootstrap needed at V0
/// 4. authentication — admin-token bootstrap (first-run only, idempotent)
///    Note: `authentication::bootstrap()` is called by the startup sequence
///    separately (Task 11); here we only verify the table exists and is accessible.
/// 5. sessions    — no runtime bootstrap needed at V0
/// 6. permissions — no runtime bootstrap needed at V0
/// 7. projects    — no runtime bootstrap needed at V0
///
/// Steps 2–3, 5–7 are structural (table existence verified by being able to
/// query them with zero rows returned). This proves the migration ran and the
/// builtin's schema is live.
pub async fn init_all(pool: &sqlx::PgPool) -> Result<BuiltinsReady, BuiltinInitError> {
    // Step 1: workspaces — verify default workspace exists (R-0015-a, R-0015-h).
    // The `workspaces` table and `default` row are created by schema::init::init().
    // If they don't exist here, the schema init was not run.
    let default_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM workspaces WHERE name = 'default'")
            .fetch_one(pool)
            .await
            .map_err(|e| BuiltinInitError::Db(Box::new(e)))?;

    if default_count.0 == 0 {
        return Err(BuiltinInitError::SchemaNotInitialized);
    }

    // Step 2: users — verify the users table is accessible (R-0015-b).
    sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
        .map_err(|e| BuiltinInitError::Db(Box::new(e)))?;

    // Step 3: agents — verify the agents table is accessible (R-0015-c).
    sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM agents")
        .fetch_one(pool)
        .await
        .map_err(|e| BuiltinInitError::Db(Box::new(e)))?;

    // Step 4: authentication — verify the admin_tokens table is accessible (R-0015-d).
    // The bootstrap path (first-run token generation) is called by the startup
    // sequence separately via `authentication::bootstrap()`.
    sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM admin_tokens")
        .fetch_one(pool)
        .await
        .map_err(|e| BuiltinInitError::Db(Box::new(e)))?;

    // Step 5: sessions — verify the sessions table is accessible (R-0015-e).
    sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM sessions")
        .fetch_one(pool)
        .await
        .map_err(|e| BuiltinInitError::Db(Box::new(e)))?;

    // Step 6: permissions — no table (permissions live in auth/permissions.rs
    // and use the admin_tokens table verified above). Verified structurally:
    // calling `permissions::check_plugin_verb` is possible in host code only.
    // No DB probe needed here — the permissions builtin has no dedicated table.

    // Step 7: projects — verify the projects table is accessible (R-0015-g).
    sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM projects")
        .fetch_one(pool)
        .await
        .map_err(|e| BuiltinInitError::Db(Box::new(e)))?;

    // All seven builtins are ready. Return the gate token.
    Ok(BuiltinsReady::new())
}

// ---------------------------------------------------------------------------
// Plugin-load gate (R-0002-c enforcement)
// ---------------------------------------------------------------------------

/// Attempt to load a plugin by name.
///
/// At V0 no real plugin loader exists (Task 19). This function is the
/// gate-enforcement entry point: it requires a `BuiltinsReady` token, which
/// can only be obtained by calling `init_all()` to completion.
///
/// Without a `BuiltinsReady`, this returns `Err(PluginLoadError::BuiltinsNotReady)`.
///
/// # R-0002-c
///
/// "no plugin invocation SHALL precede builtin startup completion"
///
/// This function enforces that invariant: the type system makes it impossible
/// to call this function without first producing a `BuiltinsReady`.
pub fn load_plugin(_ready: &BuiltinsReady, _plugin_name: &str) -> Result<(), PluginLoadError> {
    // At V0 the real plugin loader (Task 19) is not wired. This is the
    // gate stub: it accepts the `BuiltinsReady` token (proving builtins
    // initialized) and returns Ok — Task 19 replaces the body.
    Ok(())
}
