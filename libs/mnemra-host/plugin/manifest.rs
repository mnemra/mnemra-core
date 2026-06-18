//! Plugin manifest TOML deserialization types.
//!
//! The manifest format is defined in `docs/src/adrs/P-0003-plugin-manifest.md`.
//! This module owns TOML parsing only; cryptographic verification and schema
//! routing live in `runtime.rs`.
//!
//! # Wire format (schema_version = 1)
//!
//! ```toml
//! [plugin]
//! name = "echo"
//! version = "0.0.1"
//! schema_version = 1
//! core = true
//!
//! [verbs]
//! exposed = ["echo.create", "echo.get"]
//!
//! [host_fns]
//! required = ["artifact.create"]
//! optional = ["metrics.record"]
//!
//! [signature]
//! algorithm = "ed25519"
//! public_key = "<hex>"
//! sig_bytes = "<hex>"
//! signed_at = "2026-06-18T00:00:00Z"
//! ```

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Top-level manifest
// ---------------------------------------------------------------------------

/// Full plugin manifest (parsed from TOML).
///
/// Only `schema_version` is extracted before the schema_version branch; all
/// other fields are read under the assumption of schema_version = 1.
#[derive(Debug, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginMeta,
    #[serde(default)]
    pub verbs: VerbsSection,
    #[serde(default)]
    pub host_fns: HostFnsSection,
    // `signature` is present in the wire bytes but signing/verify.rs handles
    // its extraction directly; we don't need to re-parse it here.
}

// ---------------------------------------------------------------------------
// [plugin] table
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    pub schema_version: u64,
    #[serde(default)]
    pub core: bool,
}

// ---------------------------------------------------------------------------
// [verbs] table
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct VerbsSection {
    #[serde(default)]
    pub exposed: Vec<String>,
}

// ---------------------------------------------------------------------------
// [host_fns] table
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct HostFnsSection {
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub optional: Vec<String>,
}

// ---------------------------------------------------------------------------
// Parsing entry point
// ---------------------------------------------------------------------------

/// Parse manifest TOML bytes into a `PluginManifest`.
///
/// Returns `Err(String)` with a human-readable reason on any parse failure.
pub fn parse_manifest(manifest_toml: &[u8]) -> Result<PluginManifest, String> {
    let s = std::str::from_utf8(manifest_toml)
        .map_err(|e| format!("manifest is not valid UTF-8: {e}"))?;
    toml::from_str(s).map_err(|e| format!("manifest TOML parse error: {e}"))
}
