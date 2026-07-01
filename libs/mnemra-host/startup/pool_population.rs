//! Verify-gated plugin pool population — `populate_verified_pool`.
//!
//! # Security design (SF1 / R-0005-a / R-0021)
//!
//! `populate_verified_pool` is a fail-closed gate with two ordered layers:
//!
//! 1. **Signature verification** (SF1 / R-0005-a): verifies the manifest's
//!    Ed25519 signature against `root_material` — proves provenance of the
//!    declaration.  Runs FIRST; any failure returns `Err` before anything is
//!    constructed.
//!
//! 2. **Content-hash binding** (R-0021): reads the component bytes once,
//!    recomputes the declared `[component].hash`, and compares before any pool
//!    or instance is created — proves integrity of the artifact.  Fail-closed
//!    on absence of `[component].hash`.  Only a hash in the **signed** slice
//!    satisfies presence (complete mediation / R-0021-c).
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
use crate::signing::verify::{SigningError, extract_signed_payload, verify_plugin};

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

    /// The declared `[component].hash` in the signed manifest body did not
    /// match the recomputed digest of the loaded component bytes, or was
    /// absent from the signed slice, or named a weak algorithm.  The
    /// content-hash binding gate rejected the load before any pool or instance
    /// was constructed (R-0021-c / R-0021-e / R-0021-f).
    ComponentHashMismatch(String),
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
            Self::ComponentHashMismatch(msg) => write!(f, "{msg}"),
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

/// Verify a plugin manifest's signature and content hash, then populate a
/// `PluginPool` with the mnemra-echo component.
///
/// # Behavioral contract (SF1 fail-closed, R-0005-a, R-0021)
///
/// 1. Calls `verify_plugin(manifest_toml, root_material)` **FIRST** (synchronous).
///    If it returns `Err` → returns `Err(StartupError::SignatureVerification(..))`
///    and builds / populates **nothing**.
/// 2. Parses `[component].hash_alg` and `[component].hash` from the **signed
///    slice only** (R-0021-c); a `[component]` table in the unsigned region
///    does not satisfy presence.
/// 3. Single-read (R-0021-d): reads the component bytes with `std::fs::read`
///    once.  No `Component::from_file` — the same buffer feeds both the hash
///    comparison and `Component::from_binary`.
/// 4. Validates `hash_alg` against `{blake3, sha256, sha384, sha512}` (R-0021-a)
///    — any other value (including `md5`/`sha1`) is rejected before a digest is
///    computed — then recomputes that algorithm's digest over the bytes read in
///    step 3 and compares against the declared hash value.
/// 5. Only on hash match → builds a `PluginPool`, compiles the component from
///    the same in-memory buffer, registers it, and returns `Ok(Arc::new(pool))`.
///    On mismatch → `Err(StartupError::ComponentHashMismatch(..))` before any
///    pool or instance is constructed (R-0021-e).
///
/// # Manifest/component decoupling (V0 intentional design)
///
/// Verifies the **`manifest_toml` parameter it is passed** and loads the real
/// echo wasm **separately from the workspace target directory**.  It does NOT
/// read or verify the on-disk `plugins/mnemra-echo/manifest.toml`.  That
/// on-disk manifest carries placeholder signature bytes — real signing is
/// deferred to Task 26.  Do not "fix" this decoupling.
///
/// # Parameters
///
/// - `manifest_toml`: full signed manifest TOML bytes (including `[signature]`
///   table). Verified against `root_material` using Interpretation-B fingerprint
///   cross-check + Ed25519 math.
/// - `root_material`: 32-byte Ed25519 root verifying key bytes.
pub fn populate_verified_pool(
    manifest_toml: &[u8],
    root_material: &[u8],
) -> Result<Arc<PluginPool>, StartupError> {
    // Layer 1: verify signature FIRST — fail-closed (SF1, R-0005-a).
    verify_plugin(manifest_toml, root_material).map_err(StartupError::SignatureVerification)?;

    // Layer 2a: extract the signed slice; parse [component] from it ONLY (R-0021-c).
    let signed_slice = extract_signed_payload(manifest_toml);
    let signed_str = std::str::from_utf8(signed_slice).map_err(|_| {
        StartupError::ComponentHashMismatch(
            "mnemra-echo: signed manifest slice is not valid UTF-8".to_owned(),
        )
    })?;
    let signed_doc: toml::Value = signed_str.parse().map_err(|e: toml::de::Error| {
        StartupError::ComponentHashMismatch(format!(
            "mnemra-echo: failed to parse signed manifest slice: {e}"
        ))
    })?;

    let component_table = signed_doc
        .get("component")
        .and_then(|v| v.as_table())
        .ok_or_else(|| {
            StartupError::ComponentHashMismatch(
                "mnemra-echo: [component] section absent from signed body — \
                 component hash required (R-0021-c)"
                    .to_owned(),
            )
        })?;

    let hash_alg = component_table
        .get("hash_alg")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let declared_hash = component_table
        .get("hash")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            StartupError::ComponentHashMismatch(
                "mnemra-echo: [component].hash absent from signed body — \
                 component hash required (R-0021-c)"
                    .to_owned(),
            )
        })?;

    // Layer 2b: single-read (R-0021-d) — read bytes once, then hash, compare, from_binary.
    let component_path = echo_component_path();
    let component_bytes =
        std::fs::read(&component_path).map_err(|e| StartupError::ComponentLoad(e.to_string()))?;

    // Layer 2c: compute hash and compare — rejects weak alg and mismatch (R-0021-a / R-0021-e).
    let recomputed = compute_hash(hash_alg, &component_bytes)?;
    if recomputed != declared_hash {
        return Err(StartupError::ComponentHashMismatch(format!(
            "mnemra-echo: component hash mismatch — integrity check failed (tamper signal): \
             declared={declared_hash}, recomputed={recomputed}"
        )));
    }

    // Both layers passed — construct pool and compile component from same buffer (R-0021-d).
    let pool = PluginPool::new().map_err(|e| StartupError::PoolPopulation(e.to_string()))?;
    let component = Component::from_binary(pool.engine(), &component_bytes)
        .map_err(|e| StartupError::ComponentLoad(e.to_string()))?;
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
/// error from `std::fs::read` (no panic in library code).
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

/// Compute a content-hash over `bytes` using `alg`.
///
/// Returns `Err(ComponentHashMismatch)` if `alg` is outside the strong set
/// `{blake3, sha256, sha384, sha512}` — this is the single arbiter for the
/// algorithm allowlist (R-0021-a).  `md5` and `sha1` are structurally rejected
/// here before any digest computation.  The `match` and the allowed set are
/// one list — no dual-list drift (no separate `STRONG_ALGS` constant).
fn compute_hash(alg: &str, bytes: &[u8]) -> Result<String, StartupError> {
    use sha2::Digest;
    match alg {
        "blake3" => Ok(blake3::hash(bytes).to_hex().to_string()),
        "sha256" => {
            let mut h = sha2::Sha256::new();
            h.update(bytes);
            Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
        }
        "sha384" => {
            let mut h = sha2::Sha384::new();
            h.update(bytes);
            Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
        }
        "sha512" => {
            let mut h = sha2::Sha512::new();
            h.update(bytes);
            Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
        }
        other => Err(StartupError::ComponentHashMismatch(format!(
            "mnemra-echo: hash_alg '{other}' not in strong set \
             {{blake3,sha256,sha384,sha512}} — rejected (R-0021-a)"
        ))),
    }
}
