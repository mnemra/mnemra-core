//! Outbound hostname allowlist — complete mediation (SF2, R-0014-b).
//!
//! # Security contract
//!
//! The allowlist enumerates PERMITTED hostnames and denies the rest.
//! An empty allowlist blocks everything (deny-by-default).
//! No wildcard or subdomain matching at V0 — exact match only.
//! Hostname comparison is case-insensitive (normalised to lowercase on insert).

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error returned when a hostname is not on the allowlist.
///
/// Implements [`std::error::Error`] and [`std::fmt::Display`].
#[derive(Debug)]
pub struct AllowListError {
    /// The hostname that was blocked.
    blocked: String,
}

impl std::fmt::Display for AllowListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "hostname '{}' is not on the outbound allowlist (R-0014-b)",
            self.blocked,
        )
    }
}

impl std::error::Error for AllowListError {}

// ---------------------------------------------------------------------------
// AllowList
// ---------------------------------------------------------------------------

/// An immutable, per-deployment hostname allowlist.
///
/// Constructed once from configuration; cloneable for embedding in config
/// structs.
///
/// # Complete mediation (SF2 / R-0014-b)
///
/// The implementation enumerates PERMITTED hostnames and denies the rest.
/// The deny-unknown default is the contract; this is NOT a blocklist.
///
/// # Exact-match semantics at V0
///
/// V0 uses exact hostname match, case-insensitive. If `"api.openai.com"` is
/// listed, `"subdomain.api.openai.com"` is blocked. No wildcard or CIDR
/// matching. Upgrade to wildcard is a V0.1+ scope item.
///
/// # Empty allowlist
///
/// An empty slice blocks everything — deny-by-default is correct behaviour.
#[derive(Debug, Clone)]
pub struct AllowList {
    /// Permitted hostnames, stored in lowercase for case-insensitive matching.
    permitted: Vec<String>,
}

impl AllowList {
    /// Construct an `AllowList` from a slice of allowed hostnames.
    ///
    /// Hostnames are stored case-insensitively (normalised to lowercase on
    /// insert). An empty slice produces an allowlist that blocks everything.
    ///
    /// # Parameters
    ///
    /// - `hostnames`: the permitted hostname strings (bare hostname, no port
    ///   or scheme).
    pub fn new(hostnames: &[&str]) -> Self {
        let permitted = hostnames.iter().map(|h| h.to_lowercase()).collect();
        Self { permitted }
    }

    /// Check whether `hostname` is on the allowlist.
    ///
    /// Returns `Ok(())` if permitted, `Err(AllowListError)` if blocked.
    ///
    /// The check is case-insensitive. The caller passes the bare hostname
    /// (no port, no scheme). At V0 the embedding call path always calls a
    /// fixed provider hostname; port is a separate concern.
    pub fn check(&self, hostname: &str) -> Result<(), AllowListError> {
        let normalised = hostname.to_lowercase();
        if self.permitted.contains(&normalised) {
            Ok(())
        } else {
            Err(AllowListError {
                blocked: normalised,
            })
        }
    }
}
