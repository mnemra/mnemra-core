//! Verify-gated plugin pool population — `populate_verified_pool`.
//!
//! # Security design (SF1 / R-0005-a)
//!
//! `populate_verified_pool` is a fail-closed gate: verification runs FIRST
//! and ANY failure returns `Err` before any pool or component is constructed.
//! The ordering invariant is structural — verify before build — never bypassed.
//!
//! # V0 manifest/component decoupling (INTENTIONAL — do NOT "fix")
//!
//! This function verifies the **manifest_toml parameter it is passed** and loads
//! the real built mnemra-echo component **from the workspace target directory
//! separately**. It does NOT read or verify the on-disk `plugins/mnemra-echo/
//! manifest.toml`. That on-disk manifest carries placeholder signature bytes
//! (`public_key="ROOT"`, `sig_bytes="PLACEHOLDER_SIG"`) — real signing is
//! deferred to Task 26. If the on-disk manifest were verified here, the function
//! would return `Err` for every caller in the V0 build. The decoupling is
//! intentional V0 design: callers (including tests) supply a verifiable manifest
//! while the component binary is loaded from the fixed build path.
//!
//! Not yet called by `run()`; production startup wiring is deferred to Task 26
//! (manifest signing) and the serve-loop/storage follow-up. Verifies the manifest
//! passed in, NOT the on-disk placeholder manifest.

use std::path::PathBuf;
use std::sync::Arc;

use wasmtime::component::Component;

use crate::mcp::server::ECHO_PLUGIN_NAME;
use crate::plugin::pool::PluginPool;
use crate::signing::verify::{SigningError, verify_plugin};

// ---------------------------------------------------------------------------
// StartupError — public error type exported at mnemra_host::startup
// ---------------------------------------------------------------------------

/// Reasons that `populate_verified_pool` can fail.
///
/// Variants are `#[non_exhaustive]` — callers must match with a wildcard arm.
/// Tests assert only `.is_err()` / `.is_ok()`; variant names exist for
/// diagnostic clarity in structured logs and `Display` output.
#[derive(Debug)]
#[non_exhaustive]
pub enum StartupError {
    /// The manifest's signature did not verify against the provided root
    /// material (Interpretation-B chain-break or Ed25519 rejection). The verify
    /// gate fired before any pool or component was constructed (SF1, R-0005-a).
    SignatureVerification(SigningError),

    /// The compiled echo WASM component could not be loaded from the build
    /// target directory. Check that `just plugin` has been run (`cargo build
    /// --release -p mnemra-echo --target wasm32-wasip2`).
    ComponentLoad(String),

    /// The pool could not be constructed or the component could not be
    /// registered into it.
    PoolPopulation(String),
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SignatureVerification(e) => {
                write!(f, "startup: plugin signature verification failed: {e}")
            }
            Self::ComponentLoad(msg) => {
                write!(f, "startup: failed to load echo component: {msg}")
            }
            Self::PoolPopulation(msg) => {
                write!(f, "startup: failed to populate plugin pool: {msg}")
            }
        }
    }
}

impl std::error::Error for StartupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SignatureVerification(e) => Some(e),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// populate_verified_pool
// ---------------------------------------------------------------------------

/// Verify a plugin manifest's signature then populate a `PluginPool` with the
/// mnemra-echo component.
///
/// # Behavioral contract (SF1 fail-closed, R-0005-a)
///
/// 1. Calls `verify_plugin(manifest_toml, root_material)` **FIRST** (synchronous).
///    If it returns `Err` → returns `Err(StartupError::SignatureVerification(..))`
///    and builds / populates **nothing**.
/// 2. Only on verify `Ok` → builds a `PluginPool`, loads the real built
///    mnemra-echo component, registers it via `register_module`, and returns
///    `Ok(Arc::new(pool))`.
///
/// # Manifest/component decoupling (V0 intentional design)
///
/// This function verifies the **`manifest_toml` parameter it is passed** and
/// loads the real echo wasm **separately from the workspace target directory**.
/// It does NOT read or verify the on-disk `plugins/mnemra-echo/manifest.toml`.
/// That on-disk manifest carries placeholder signature bytes (`public_key="ROOT"`,
/// `sig_bytes="PLACEHOLDER_SIG"`) — real signing is deferred to Task 26. Reading
/// and verifying the on-disk manifest here would cause every call to return `Err`.
/// This decoupling is intentional V0 design; do not "fix" it.
///
/// # Deferred production wiring
///
/// Not yet called by `run()`. Production startup wiring — wiring this function
/// into `run()` with a real root-signed manifest — is deferred to Task 26
/// (manifest signing) and the serve-loop/storage follow-up.
///
/// # Parameters
///
/// - `manifest_toml`: full signed manifest TOML bytes (including `[signature]`
///   table). Verified against `root_material` using Interpretation-B fingerprint
///   cross-check + Ed25519 math.
/// - `root_material`: 32-byte Ed25519 root verifying key bytes. At production
///   runtime this is `signing::root_material::ROOT`; callers (including tests)
///   inject a per-run generated key so tests are not coupled to the build constant.
pub fn populate_verified_pool(
    manifest_toml: &[u8],
    root_material: &[u8],
) -> Result<Arc<PluginPool>, StartupError> {
    // Step 1: Verify signature FIRST — fail-closed (SF1, R-0005-a).
    // Any error returns immediately before pool or component construction.
    verify_plugin(manifest_toml, root_material).map_err(StartupError::SignatureVerification)?;

    // Step 2: Verification passed — build the pool.
    let pool = PluginPool::new().map_err(|e| StartupError::PoolPopulation(e.to_string()))?;

    // Step 3: Load the built mnemra-echo component from the workspace target dir.
    // Replicates the path logic from tests/common/slice1_harness.rs::echo_component_path()
    // without importing the test helper (tests/ is a separate crate, forbid_scope).
    // CARGO_MANIFEST_DIR = <root>/libs/mnemra-host; workspace root = ../..
    let component_path = echo_component_path();
    let component = Component::from_file(pool.engine(), &component_path)
        .map_err(|e| StartupError::ComponentLoad(e.to_string()))?;

    // Step 4: Register the echo component into the pool.
    pool.register_module(ECHO_PLUGIN_NAME, "0.0.1", &component)
        .map_err(|e| StartupError::PoolPopulation(e.to_string()))?;

    Ok(Arc::new(pool))
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Locate the built `mnemra-echo` component (`wasm32-wasip2`, release).
///
/// Resolved relative to this crate's manifest directory. Returns the path
/// without asserting existence — a missing file produces a `ComponentLoad`
/// error from `Component::from_file` (no panic in library code).
///
/// Path: `<workspace_root>/target/wasm32-wasip2/release/mnemra_echo.wasm`
/// where workspace root is two directories above `CARGO_MANIFEST_DIR`
/// (`libs/mnemra-host`).
fn echo_component_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root resolution from libs/mnemra-host — layout must not change");
    root.join("target/wasm32-wasip2/release/mnemra_echo.wasm")
}
