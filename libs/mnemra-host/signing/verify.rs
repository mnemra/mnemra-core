//! Synchronous plugin signing-chain verification.
//!
//! # Security design
//!
//! The trust anchor is the bytes passed as `root_material` — at runtime this is
//! `signing::root_material::ROOT` (embedded at build time, R-0005-d). No key is
//! ever sourced from the manifest itself; the manifest's `public_key` field is a
//! CROSS-CHECK only (see Interpretation B below).
//!
//! Verification is SYNCHRONOUS and FAIL-SHUT: `verify_plugin` is a plain `fn`,
//! never `async fn`, and every failure path returns `Err` before any plugin
//! instance is created (R-0005-a).
//!
//! # Canonicalization contract
//!
//! The signed payload is the manifest bytes MINUS the `[signature]` section.
//! `verify_plugin` recovers these bytes by finding the `\n[signature]` marker
//! in the raw input and slicing everything before it. This is NOT a re-serialize
//! pass — re-serialization would normalize whitespace and break the byte-exact
//! match against the signature produced by the build pipeline's sign step.
//!
//! This matches the test fixture contract in `signing_chain.rs`:
//!   `manifest_with_signature` appends `\n[signature]\n...` to the exact bytes
//!   of `manifest_bytes_unsigned()`. Slicing at `\n[signature]` recovers the
//!   original unsigned bytes.
//!
//! # Interpretation B (chain-break cross-check) — locked decision 2026-06-13
//!
//! The manifest's `[signature]` table carries a `public_key` field (the hex
//! fingerprint of the key that signed the manifest). Under Interpretation B,
//! the verifier:
//!
//!   1. Verifies `sig_bytes` against the EMBEDDED `root_material` (the trust
//!      anchor). This is the primary cryptographic check.
//!   2. Also checks that `public_key` in the manifest MATCHES the hex encoding
//!      of `root_material`. A mismatch — even when step 1 succeeds — is a
//!      certificate-chain break and REJECTS (R-0005-b).
//!
//! Rationale: the manifest `public_key` field is NEVER used as the verification
//! key (that would be manifest-field trust, forbidden by R-0005-h). It is a
//! defense-in-depth cross-check. An attacker who obtains root-signed bytes but
//! declares a different `public_key` in the manifest is detected here.
//!
//! The alternative (Interpretation A — verify against root only, ignore
//! `public_key` field) was explicitly rejected by the maintainer on 2026-06-13
//! in favor of fail-closed defense-in-depth.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Verification outcome: the plugin's `core` status as determined by signature
/// PROVENANCE, not by the manifest `core` field.
///
/// At V0 there is only one variant — all non-core plugins are rejected at load
/// (R-0005-g). `NonCore` is not a valid outcome; V0 rejects before returning it.
#[derive(Debug, PartialEq)]
pub enum CoreStatus {
    /// The manifest signature chains to the mnemra embedded root. The plugin
    /// is authorised as a core plugin.
    Core,
}

/// The specific reason a signing verification failed.
///
/// Used internally to route error construction. Tests do not assert on the
/// variant — they only inspect `plugin_name()` and `plugin_version()`.
/// The payload fields carry diagnostic detail that surfaces via `Debug` and
/// `Display` in structured logs; the compiler can't see the read-through
/// the derived `Debug` impl, hence the allow.
#[derive(Debug)]
#[allow(dead_code)]
enum SigningErrorKind {
    /// The manifest TOML could not be parsed.
    ParseError(String),
    /// The `[signature]` table is missing or malformed.
    MissingSignature,
    /// The manifest `core` field is not `true` (R-0003-e, R-0005-g).
    CoreFalse,
    /// The manifest `public_key` field does not match the embedded root
    /// material fingerprint (Interpretation B chain-break check).
    FingerprintMismatch,
    /// Cryptographic verification of `sig_bytes` against `root_material` failed.
    VerificationFailed(String),
    /// The `sig_bytes` hex value has the wrong length (not 64 bytes after decode).
    MalformedSignature(String),
}

/// Structured verification failure — names the plugin that was rejected.
///
/// # Required accessor methods
///
/// Tests call `err.plugin_name()` and `err.plugin_version()` to assert that
/// every reject path names the offending plugin.
#[derive(Debug)]
pub struct SigningError {
    plugin_name: String,
    plugin_version: String,
    kind: SigningErrorKind,
}

impl SigningError {
    fn new(plugin_name: &str, plugin_version: &str, kind: SigningErrorKind) -> Self {
        Self {
            plugin_name: plugin_name.to_owned(),
            plugin_version: plugin_version.to_owned(),
            kind,
        }
    }

    /// Returns the name of the plugin that failed verification.
    pub fn plugin_name(&self) -> &str {
        &self.plugin_name
    }

    /// Returns the version of the plugin that failed verification.
    pub fn plugin_version(&self) -> &str {
        &self.plugin_version
    }
}

impl std::fmt::Display for SigningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "plugin signing verification failed for {}@{}: {:?}",
            self.plugin_name, self.plugin_version, self.kind
        )
    }
}

impl std::error::Error for SigningError {}

// ---------------------------------------------------------------------------
// Verification entry point
// ---------------------------------------------------------------------------

/// Verify a plugin manifest's signature synchronously.
///
/// # Parameters
///
/// - `manifest_toml`: full manifest TOML bytes including the `[signature]` table.
/// - `root_material`: 32-byte Ed25519 root verifying key bytes. At runtime this
///   is `signing::root_material::ROOT`; in tests a per-run generated key is
///   injected so tests are not coupled to the build-embedded constant.
///
/// # Algorithm
///
/// 1. Parse `manifest_toml` as TOML to extract `[plugin].name`, `[plugin].version`,
///    `[plugin].core`, and `[signature].sig_bytes` + `[signature].public_key`.
/// 2. If `core != true`, return `Err` (R-0003-e, R-0005-g). Name extraction
///    happens first so the error is always named.
/// 3. Hex-decode `sig_bytes`; reject if not exactly 64 bytes (R-0005-b).
/// 4. Check that `public_key` (hex) matches the hex encoding of `root_material`
///    (Interpretation B cross-check). Reject on mismatch (R-0005-b chain-break).
/// 5. Recover the "signed payload" by stripping `\n[signature]` and everything
///    after it from the raw input bytes. This is a byte-exact slice — NOT a
///    re-serialisation — so whitespace is preserved and the verifier sees exactly
///    the bytes the build pipeline signed.
/// 6. Verify `sig_bytes` over the signed payload using `root_material` as the
///    Ed25519 verifying key (R-0005-d: root_material is the only trust anchor).
/// 7. `Ok(CoreStatus::Core)` on success; `Err(SigningError)` on any failure.
///
/// # R-0005-a: MUST NOT be `async fn`
///
/// This function is a plain synchronous `fn`. No `await`, no `Future`, no
/// thread-defer. If it were made `async`, the compiler would produce an
/// "unused implementor of Future" warning when called without `.await` — that
/// compile artefact IS the test that pins this requirement.
pub fn verify_plugin(
    manifest_toml: &[u8],
    root_material: &[u8],
) -> Result<CoreStatus, SigningError> {
    // Step 1: Parse the full manifest TOML to extract fields.
    let toml_str = std::str::from_utf8(manifest_toml).map_err(|_| {
        // Can't name the plugin yet — use a sentinel value.
        SigningError::new(
            "(unparseable)",
            "(unparseable)",
            SigningErrorKind::ParseError("manifest bytes are not valid UTF-8".to_owned()),
        )
    })?;

    let doc: toml::Value = toml_str.parse().map_err(|e: toml::de::Error| {
        SigningError::new(
            "(unparseable)",
            "(unparseable)",
            SigningErrorKind::ParseError(e.to_string()),
        )
    })?;

    // Extract plugin name and version FIRST — every error path needs them.
    let plugin = doc
        .get("plugin")
        .and_then(|v| v.as_table())
        .ok_or_else(|| {
            SigningError::new(
                "(missing)",
                "(missing)",
                SigningErrorKind::ParseError("missing [plugin] table".to_owned()),
            )
        })?;

    let name = plugin
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("(missing)");

    let version = plugin
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("(missing)");

    // Step 2: Check core = true (R-0003-e, R-0005-g).
    let core = plugin
        .get("core")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !core {
        return Err(SigningError::new(
            name,
            version,
            SigningErrorKind::CoreFalse,
        ));
    }

    // Extract [signature] table fields.
    let signature_table = doc
        .get("signature")
        .and_then(|v| v.as_table())
        .ok_or_else(|| SigningError::new(name, version, SigningErrorKind::MissingSignature))?;

    let sig_hex = signature_table
        .get("sig_bytes")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SigningError::new(name, version, SigningErrorKind::MissingSignature))?;

    let pubkey_hex = signature_table
        .get("public_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SigningError::new(name, version, SigningErrorKind::MissingSignature))?;

    // Step 3: Hex-decode sig_bytes; must be exactly 64 bytes for Ed25519.
    let sig_bytes = hex_decode(sig_hex).map_err(|e| {
        SigningError::new(
            name,
            version,
            SigningErrorKind::MalformedSignature(format!("sig_bytes hex decode failed: {e}")),
        )
    })?;

    if sig_bytes.len() != 64 {
        return Err(SigningError::new(
            name,
            version,
            SigningErrorKind::MalformedSignature(format!(
                "sig_bytes must be 64 bytes, got {}",
                sig_bytes.len()
            )),
        ));
    }

    // Step 4: Interpretation B cross-check — manifest public_key field must
    // match the hex encoding of root_material. This is NOT using the manifest
    // field as a trust source; it is a consistency check that detects chain
    // breaks where sig was made by root but the manifest declares a different key.
    let root_hex = hex_encode(root_material);
    if pubkey_hex != root_hex {
        return Err(SigningError::new(
            name,
            version,
            SigningErrorKind::FingerprintMismatch,
        ));
    }

    // Step 5: Recover the signed payload by byte-exact stripping.
    // `manifest_with_signature` in the test appends "\n[signature]\n..." to the
    // original unsigned bytes. We find that boundary and slice before it.
    // This preserves the exact byte layout (whitespace, ordering) that the
    // build pipeline signed — no re-serialization is safe here.
    let signed_payload = extract_signed_payload(manifest_toml);

    // Step 6: Verify sig_bytes over signed_payload using root_material.
    // root_material is the ONLY trust anchor (R-0005-d, R-0005-h).
    let verifying_key = VerifyingKey::from_bytes(root_material.try_into().map_err(|_| {
        SigningError::new(
            name,
            version,
            SigningErrorKind::VerificationFailed("root_material is not 32 bytes".to_owned()),
        )
    })?)
    .map_err(|e| {
        SigningError::new(
            name,
            version,
            SigningErrorKind::VerificationFailed(format!("invalid root key: {e}")),
        )
    })?;

    let sig_array: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .expect("len already checked to be 64");
    let signature = Signature::from_bytes(&sig_array);

    verifying_key
        .verify(signed_payload, &signature)
        .map_err(|e| {
            SigningError::new(
                name,
                version,
                SigningErrorKind::VerificationFailed(e.to_string()),
            )
        })?;

    // Step 7: Signature verified against the embedded root. Core by provenance.
    Ok(CoreStatus::Core)
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Extract the signed payload from the full manifest bytes.
///
/// The canonical form (per P-0003) is everything BEFORE the `[signature]` table.
/// The test fixture appends `"\n[signature]\n..."` to the unsigned bytes, so
/// searching for `b"\n[signature]"` and slicing before it recovers the exact
/// original bytes.
///
/// If the marker is not found (malformed or already-unsigned manifest), the
/// entire input is returned — which will fail sig verification and produce the
/// correct Err from the caller.
fn extract_signed_payload(manifest_toml: &[u8]) -> &[u8] {
    const MARKER: &[u8] = b"\n[signature]";
    if let Some(idx) = find_subsequence(manifest_toml, MARKER) {
        &manifest_toml[..idx]
    } else {
        manifest_toml
    }
}

/// Find the starting index of `needle` in `haystack`, or `None` if absent.
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Hex-encode `bytes` as a lowercase hex string.
///
/// Mirrors the `hex_encode` helper in `signing_chain.rs` so that the
/// fingerprint comparison in Interpretation B uses the same encoding.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Hex-decode a lowercase hex string to bytes.
///
/// Returns `Err(String)` on invalid input (odd length, non-hex characters).
fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err(format!("odd hex length: {}", s.len()));
    }
    s.as_bytes()
        .chunks(2)
        .map(|chunk| {
            let hi = hex_nibble(chunk[0])?;
            let lo = hex_nibble(chunk[1])?;
            Ok((hi << 4) | lo)
        })
        .collect::<Result<Vec<u8>, String>>()
}

/// Parse a single hex nibble (ASCII digit or a-f/A-F) to its value.
fn hex_nibble(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid hex byte: {b:#x}")),
    }
}
