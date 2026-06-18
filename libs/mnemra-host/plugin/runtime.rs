//! Plugin runtime — manifest load pipeline and query surface.
//!
//! # Tested surface (manifest_load.rs RED tests)
//!
//! `PluginRuntime` is the single type exercised by the RED integration tests.
//! Its `load` constructor runs the fail-shut pipeline:
//!
//!   1. `verify_plugin(manifest_toml, root_material)` — Ed25519 + Interpretation-B
//!      cross-check. Returns `Err` → `LoadError::VerificationFailed` immediately.
//!   2. TOML parse + `schema_version` branch. Unknown version → `LoadError::UnknownSchemaVersion`.
//!   3. Compile host-fn allowlist from `[host_fns]`.
//!   4. Compile verb capability list from `[verbs].exposed`.
//!
//! Steps 1–4 are PURE MANIFEST PROCESSING. No Wasmtime component is instantiated in
//! `load`. Pool initialisation, fuel/epoch limits, and the epoch-tick supervisor are
//! a separate construct (`pool.rs`, `limits.rs`, `epoch_thread.rs`) wired at host
//! startup, outside this path.
//!
//! # Fail-shut ordering invariant (pinned by RED ordering tests)
//!
//! Verification (step 1) runs BEFORE the schema_version branch (step 2).
//! An attacker-signed manifest with a valid schema_version must be rejected
//! with `VerificationFailed`, NOT `UnknownSchemaVersion`.
//!
//! # Supported schema versions (V0)
//!
//! Only schema_version = 1. `0` and anything ≥ 2 produce `UnknownSchemaVersion`.

use crate::plugin::allowlist::{HostFnAllowlist, VerbAllowlist};
use crate::plugin::manifest::parse_manifest;
use crate::plugin::output;
use crate::signing::verify::{SigningError, verify_plugin};

// ---------------------------------------------------------------------------
// Supported schema versions
// ---------------------------------------------------------------------------

/// The set of schema versions this runtime accepts (V0: only 1).
const SUPPORTED_VERSIONS: &[u64] = &[1];

// ---------------------------------------------------------------------------
// Error types — shapes MUST match what manifest_load.rs destructures
// ---------------------------------------------------------------------------

/// Reasons a manifest can fail to load.
///
/// Variant and field names are pinned by `manifest_load.rs` pattern matches.
/// Do NOT rename without updating that file (which is forbid-scoped — changes
/// there are blocked).
#[derive(Debug)]
pub enum LoadError {
    /// Cryptographic or provenance verification failed.
    ///
    /// `matches!(err, LoadError::VerificationFailed(_))` — the RED tests check
    /// this exact pattern.
    VerificationFailed(SigningError),

    /// The manifest's `schema_version` field names a version this runtime does
    /// not support (including version 0, which was never valid).
    ///
    /// `matches!(err, LoadError::UnknownSchemaVersion { found, supported })` —
    /// the RED tests destructure `found` and `supported` by name.
    UnknownSchemaVersion {
        /// The value found in the manifest.
        found: u64,
        /// The versions this runtime supports.
        supported: &'static [u64],
    },

    /// The manifest TOML is structurally malformed (unparseable or missing
    /// required fields after successful verification).
    MalformedManifest(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VerificationFailed(e) => write!(f, "plugin verification failed: {e}"),
            Self::UnknownSchemaVersion { found, supported } => write!(
                f,
                "unsupported schema_version {found}; supported: {supported:?}"
            ),
            Self::MalformedManifest(msg) => write!(f, "malformed manifest: {msg}"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::VerificationFailed(e) => Some(e),
            _ => None,
        }
    }
}

/// Reasons plugin output validation can fail (R-0003-f).
///
/// Variant and field names are pinned by `manifest_load.rs` pattern matches.
#[derive(Debug)]
pub enum OutputError {
    /// The output bytes do not conform to the WIT-declared schema for the verb.
    ///
    /// `matches!(err, OutputError::SchemaMismatch { .. })` — RED tests check this.
    SchemaMismatch {
        /// The verb that produced the mismatched output.
        verb: String,
        /// Human-readable detail explaining the mismatch.
        detail: String,
    },

    /// An output field exceeds the per-field byte cap. Checked BEFORE schema
    /// validation (ordering pinned by RED tests).
    ///
    /// `matches!(err, OutputError::FieldSizeCap { .. })` — RED tests check this.
    FieldSizeCap {
        /// The field name that exceeded the cap.
        field: String,
        /// The cap in bytes.
        max_bytes: usize,
        /// The actual byte count.
        actual_bytes: usize,
    },
}

impl std::fmt::Display for OutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchemaMismatch { verb, detail } => {
                write!(f, "output schema mismatch for verb '{verb}': {detail}")
            }
            Self::FieldSizeCap {
                field,
                max_bytes,
                actual_bytes,
            } => write!(
                f,
                "output field '{field}' exceeds size cap: {actual_bytes} > {max_bytes} bytes"
            ),
        }
    }
}

impl std::error::Error for OutputError {}

// ---------------------------------------------------------------------------
// PluginRuntime
// ---------------------------------------------------------------------------

/// A loaded plugin runtime handle.
///
/// Created by `PluginRuntime::load` after the full fail-shut pipeline completes.
/// Provides allowlist queries and output validation. Does NOT hold a Wasmtime
/// store or component — those live in the pool (`pool.rs`).
#[derive(Debug)]
pub struct PluginRuntime {
    /// The `schema_version` field extracted from the loaded manifest.
    schema_version: u64,
    /// Compiled host-fn allowlist from `[host_fns]`.
    host_fn_allowlist: HostFnAllowlist,
    /// Compiled verb capability list from `[verbs].exposed`.
    verb_allowlist: VerbAllowlist,
}

impl PluginRuntime {
    // -----------------------------------------------------------------------
    // Constructor — fail-shut load pipeline
    // -----------------------------------------------------------------------

    /// Load a plugin from its signed manifest bytes.
    ///
    /// # Fail-shut ordering (test-pinned)
    ///
    /// 1. `verify_plugin(manifest_toml, root_material)` — verification runs FIRST.
    ///    Any failure returns `LoadError::VerificationFailed` immediately, before
    ///    `schema_version` is even inspected.
    /// 2. TOML parse + `schema_version` branch.
    /// 3. Allowlist + capability-list compilation.
    ///
    /// # Parameters
    ///
    /// - `manifest_toml`: full manifest bytes including the `[signature]` table.
    /// - `root_material`: 32-byte Ed25519 root verifying key. Production callers
    ///   pass `signing::root_material::ROOT`; tests inject a per-run generated key.
    pub fn load(manifest_toml: &[u8], root_material: &[u8]) -> Result<Self, LoadError> {
        // Step 1: Verify signature + Interpretation-B cross-check. MUST be first.
        verify_plugin(manifest_toml, root_material).map_err(LoadError::VerificationFailed)?;

        // Step 2: Parse manifest TOML. Verification passed, so the bytes are
        // structurally plausible — but they still need schema_version routing.
        let manifest = parse_manifest(manifest_toml).map_err(LoadError::MalformedManifest)?;

        // Step 3: schema_version branch. Unknown → error with observed + supported.
        let sv = manifest.plugin.schema_version;
        if !SUPPORTED_VERSIONS.contains(&sv) {
            return Err(LoadError::UnknownSchemaVersion {
                found: sv,
                supported: SUPPORTED_VERSIONS,
            });
        }

        // Step 4: Compile allowlist + capability list from manifest fields.
        let host_fn_allowlist = HostFnAllowlist::from_manifest(&manifest.host_fns);
        let verb_allowlist = VerbAllowlist::from_exposed(&manifest.verbs.exposed);

        Ok(Self {
            schema_version: sv,
            host_fn_allowlist,
            verb_allowlist,
        })
    }

    // -----------------------------------------------------------------------
    // Allowlist queries
    // -----------------------------------------------------------------------

    /// Returns `true` iff `fn_name` appears in the manifest's `required` or
    /// `optional` host-fn declarations (R-0003-b).
    ///
    /// This is the per-call check for WIT-boundary enforcement. The actual
    /// boundary rejection (refusing undeclared host-fn calls before they reach
    /// the host-fn body) is wired via the Wasmtime linker in `pool.rs`.
    pub fn is_host_fn_allowed(&self, fn_name: &str) -> bool {
        self.host_fn_allowlist.is_allowed(fn_name)
    }

    /// Returns `true` iff `verb` appears in the manifest's `[verbs].exposed`
    /// list (R-0010-d). Called at dispatch time before the plugin is invoked.
    pub fn is_verb_allowed(&self, verb: &str) -> bool {
        self.verb_allowlist.is_allowed(verb)
    }

    /// The `schema_version` field extracted from the loaded manifest.
    pub fn schema_version(&self) -> u64 {
        self.schema_version
    }

    // -----------------------------------------------------------------------
    // Output validation
    // -----------------------------------------------------------------------

    /// Validate a plugin's output bytes against the WIT-declared schema for `verb`.
    ///
    /// # Ordering (test-pinned)
    ///
    /// Size-cap check runs BEFORE schema check (R-0003-f). Oversized output
    /// returns `Err(OutputError::FieldSizeCap)` without reaching the schema gate.
    ///
    /// # Fail-shut
    ///
    /// Returns `Err` for any bytes that don't conform — never truncates, never
    /// returns `Ok` for structurally invalid output. An unbound verb (no known
    /// WIT output type at V0) is a schema mismatch (R-0003-f, fail-shut default).
    pub fn validate_output(&self, verb: &str, output_bytes: &[u8]) -> Result<(), OutputError> {
        output::validate_output(verb, output_bytes)
    }
}
