//! Per-deployment LLM-API-key configuration (R-0014-a, R-0014-b, R-0014-c).
//!
//! Loaded from a TOML file at deploy time. The file format is:
//!
//! ```toml
//! api_key = "sk-..."
//! hostname_allowlist = ["api.openai.com"]
//! ```
//!
//! # Security properties
//!
//! - R-0014-a: the API key comes from the config file, never from a compiled-in
//!   default.
//! - R-0014-b: the hostname allowlist is configurable per deployment.
//! - R-0014-c: `LlmKeyConfig` has NO `hosted_model_endpoint` field or accessor.
//!   The config surface accepts external-provider keys only; there is no path
//!   to configure a self-hosted model endpoint.
//!
//! # File-mode gate
//!
//! `LlmKeyConfig::load()` does NOT check file permissions. The file-mode gate
//! (`startup::file_mode_check::check()`) MUST be called before this function
//! is invoked (R-0014-d).

use std::path::Path;

use crate::net::hostname_allowlist::AllowList;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error returned when the LLM-key config file cannot be loaded.
///
/// Implements [`std::error::Error`] and [`std::fmt::Display`].
#[derive(Debug)]
pub struct LlmKeyConfigError {
    /// Informative message describing the failure.
    message: String,
}

impl LlmKeyConfigError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for LlmKeyConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LLM-key config error: {}", self.message)
    }
}

impl std::error::Error for LlmKeyConfigError {}

// ---------------------------------------------------------------------------
// LlmKeyConfig
// ---------------------------------------------------------------------------

/// Per-deployment LLM-API-key configuration.
///
/// Loaded from a TOML file at deploy time via [`LlmKeyConfig::load()`].
///
/// # Public API (complete — no additional public methods)
///
/// - [`api_key()`](LlmKeyConfig::api_key) — the API key string (from file)
/// - [`hostname_allowlist()`](LlmKeyConfig::hostname_allowlist) — the
///   configured [`AllowList`] (from file)
///
/// There is NO `hosted_model_endpoint()` method or `hosted_endpoint` field
/// (R-0014-c: external providers only; structural absence is the enforcement).
pub struct LlmKeyConfig {
    /// The API key, as read from the `api_key` TOML field.
    api_key: String,
    /// The hostname allowlist, constructed from the `hostname_allowlist` field.
    hostname_allowlist: AllowList,
}

impl LlmKeyConfig {
    /// Load a `LlmKeyConfig` from a TOML file at `path`.
    ///
    /// Reads the file, parses TOML, constructs and returns the config.
    ///
    /// # Errors
    ///
    /// Returns `Err(LlmKeyConfigError)` if:
    /// - the file cannot be read (`io::Error`)
    /// - the content is not valid TOML
    /// - the required `api_key` field is absent or not a string
    /// - the required `hostname_allowlist` field is absent or not an array of
    ///   strings
    ///
    /// # File-mode gate
    ///
    /// Does NOT check file permissions. The caller MUST invoke
    /// `startup::file_mode_check::check()` before calling this (R-0014-d).
    pub fn load(path: &Path) -> Result<Self, LlmKeyConfigError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            LlmKeyConfigError::new(format!("cannot read {}: {}", path.display(), e))
        })?;

        let doc: toml::Value = content.parse().map_err(|e| {
            LlmKeyConfigError::new(format!("TOML parse error in {}: {}", path.display(), e))
        })?;

        // Extract api_key field.
        let api_key = doc
            .get("api_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                LlmKeyConfigError::new(format!(
                    "missing or non-string `api_key` field in {}",
                    path.display()
                ))
            })?
            .to_owned();

        // Extract hostname_allowlist field — array of strings.
        let raw_list = doc
            .get("hostname_allowlist")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                LlmKeyConfigError::new(format!(
                    "missing or non-array `hostname_allowlist` field in {}",
                    path.display()
                ))
            })?;

        // Collect the array elements as &str; reject any non-string entries.
        let mut hostnames: Vec<String> = Vec::with_capacity(raw_list.len());
        for (i, entry) in raw_list.iter().enumerate() {
            match entry.as_str() {
                Some(s) => hostnames.push(s.to_owned()),
                None => {
                    return Err(LlmKeyConfigError::new(format!(
                        "hostname_allowlist[{}] is not a string in {}",
                        i,
                        path.display()
                    )));
                }
            }
        }

        // Build the AllowList from owned strings.
        let hostname_refs: Vec<&str> = hostnames.iter().map(String::as_str).collect();
        let hostname_allowlist = AllowList::new(&hostname_refs);

        Ok(Self {
            api_key,
            hostname_allowlist,
        })
    }

    /// The API key string, as loaded from the config file.
    ///
    /// Returns a `&str` reference into the loaded config.
    /// The string is the literal value from the `api_key` TOML field.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// The hostname allowlist loaded from the config file.
    ///
    /// Returns a reference to the [`AllowList`] constructed from the
    /// `hostname_allowlist` TOML field.
    pub fn hostname_allowlist(&self) -> &AllowList {
        &self.hostname_allowlist
    }
}
