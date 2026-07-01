//! `sign-ceremony` core — the mnemra root signing-ceremony **producer** logic.
//!
//! # Decision hidden
//!
//! The exact byte-layout and signing procedure that produces a plugin manifest
//! the runtime verifier (`mnemra_host::signing::verify::verify_plugin`) accepts
//! — the *producer* side of the P-0005 signing contract. `verify.rs` owns the
//! verify side; this crate owns the matching sign side. The load-bearing
//! property is byte-exactness: the signature is computed over the manifest bytes
//! BEFORE the `\n[signature]` marker (no re-serialization), so the verifier's
//! `extract_signed_payload` recovers exactly what was signed.
//!
//! # What it is / is NOT
//!
//! This is *maintainer tooling* run once at the key ceremony (see
//! `docs/runbooks/signing-ceremony.md`). It never generates or persists a real
//! key on its own beyond the explicit `keygen` step, and it never runs against
//! the committed `plugins/` manifest as part of the build — the maintainer
//! points it at custody paths. The runtime host does not link or call this tool.
//!
//! # Signing format (matches `verify.rs` + the `signing_chain` / `content_hash_binding` fixtures)
//!
//! ```text
//! <manifest body, [signature] and any prior [component] stripped, trailing ws trimmed>
//!
//! [component]
//! hash_alg = "blake3"
//! hash = "<lowercase blake3 hex of the wasm bytes>"
//!
//! [signature]
//! algorithm = "ed25519"
//! public_key = "<lowercase hex of the 32-byte verifying key>"
//! sig_bytes = "<lowercase hex of the 64-byte signature>"
//! signed_at = "<RFC3339>"
//! ```
//!
//! The signed payload is everything above `\n[signature]`, i.e. the body plus
//! the embedded `[component]` block. `public_key` is the hex of the verifying
//! key, which satisfies `verify.rs`'s Interpretation-B cross-check
//! (`public_key == hex(root_material)`) when the same key becomes `ROOT` in
//! round-2.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use ed25519_dalek::{Signer, SigningKey};
use rand::TryRng;

use mnemra_host::signing::verify::{CoreStatus, SIGNATURE_MARKER, verify_plugin};

/// Errors surfaced by the ceremony tool. Rendered to stderr by the bin's `main`;
/// the process exits non-zero on any variant (fail-closed).
#[derive(Debug)]
pub enum CeremonyError {
    /// An I/O failure (open/read/write), with context.
    Io(String),
    /// The manifest bytes are not valid UTF-8.
    ManifestNotUtf8,
    /// The private-key file is not exactly 32 bytes (the raw Ed25519 seed).
    BadKeyLength(usize),
    /// The `keygen` target path already exists — refuse to overwrite a key.
    KeyExists(PathBuf),
    /// The OS CSPRNG could not be read.
    Rng(String),
    /// The produced manifest failed to verify against the real `verify_plugin`
    /// — the tool refuses to write it (fail-closed).
    SelfVerifyFailed(String),
    /// The manifest body contains more than one occurrence of the
    /// `[signature]` boundary marker, or more than one top-level `[component]`
    /// table header — ambiguous input, since both `strip_from_marker` here and
    /// the runtime verifier's `extract_signed_payload` slice at the FIRST
    /// occurrence only (embedded-in-a-value or duplicated-table class).
    EmbeddedMarker(String),
}

impl std::fmt::Display for CeremonyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CeremonyError::Io(msg) => write!(f, "{msg}"),
            CeremonyError::ManifestNotUtf8 => write!(f, "manifest bytes are not valid UTF-8"),
            CeremonyError::BadKeyLength(n) => write!(
                f,
                "private key file must be exactly 32 bytes (the raw Ed25519 seed \
                 produced by `sign-ceremony keygen`); got {n} bytes"
            ),
            CeremonyError::KeyExists(p) => write!(
                f,
                "refusing to overwrite existing file {} — choose a fresh key path",
                p.display()
            ),
            CeremonyError::Rng(msg) => write!(f, "failed to read OS CSPRNG: {msg}"),
            CeremonyError::SelfVerifyFailed(msg) => write!(
                f,
                "produced manifest FAILED self-verification against verify_plugin \
                 ({msg}) — nothing written (fail-closed)"
            ),
            CeremonyError::EmbeddedMarker(msg) => write!(
                f,
                "manifest contains an ambiguous signing-boundary marker ({msg}) — \
                 refusing to sign (fail-closed)"
            ),
        }
    }
}

impl std::error::Error for CeremonyError {}

// ---------------------------------------------------------------------------
// Core signing logic (pure — driven by the round-trip tests in tests/)
// ---------------------------------------------------------------------------

/// Build a fully-signed manifest from `manifest_toml`.
///
/// Strips any existing `[signature]` and `[component]` sections, embeds a fresh
/// `[component]` block carrying the BLAKE3 hash of `wasm_bytes` inside the
/// signed body, signs the signed payload with `signing_key`, and appends a
/// populated `[signature]` table. The result verifies against `verify_plugin`
/// when checked with `signing_key.verifying_key()` as the root material.
pub fn build_signed_manifest(
    signing_key: &SigningKey,
    wasm_bytes: &[u8],
    manifest_toml: &[u8],
    signed_at: &str,
) -> Result<Vec<u8>, CeremonyError> {
    let manifest_str =
        std::str::from_utf8(manifest_toml).map_err(|_| CeremonyError::ManifestNotUtf8)?;

    // 0. Fail-closed hardening (Warden T3 review, conf 80). `strip_from_marker`
    //    below — and the runtime verifier's `extract_signed_payload` — both
    //    slice at the FIRST occurrence of their boundary marker only. A
    //    manifest body that embeds a decoy `[signature]` marker or a decoy
    //    top-level `[component]` header (e.g. inside a multi-line string
    //    value) ahead of the real one would silently mis-slice rather than
    //    error: not exploitable (producer and verifier slice identically —
    //    see module docs), but this closes the silent-truncation class before
    //    signing ever happens. A legitimate manifest carries AT MOST one of
    //    each (0 when never-signed, 1 when re-signing an already-signed
    //    manifest), so more than one is always ambiguous input.
    let marker_str = std::str::from_utf8(SIGNATURE_MARKER)
        .expect("SIGNATURE_MARKER (mnemra_host::signing::verify) is a static ASCII literal");
    let marker_count = count_occurrences(manifest_str, marker_str);
    if marker_count > 1 {
        return Err(CeremonyError::EmbeddedMarker(format!(
            "found {marker_count} occurrences of the `{marker_str}` signature boundary \
             marker (expected at most 1) — embedded in a value, or duplicated"
        )));
    }
    let component_header_count = count_header_lines(manifest_str, "[component]");
    if component_header_count > 1 {
        return Err(CeremonyError::EmbeddedMarker(format!(
            "found {component_header_count} top-level `[component]` table headers \
             (expected at most 1) — embedded in a value, or duplicated"
        )));
    }

    // 1. Drop any existing `[signature]` — everything from the `\n[signature]`
    //    marker onward. Mirrors `verify::extract_signed_payload`.
    let body = strip_from_marker(manifest_str, "\n[signature]");
    // 2. Drop any existing `[component]` table so re-running is idempotent
    //    (a duplicate table would be a TOML parse error downstream).
    let body = strip_top_level_table(&body, "component");
    // 3. Normalize trailing whitespace so the appended layout is deterministic.
    let body_trimmed = body.trim_end();

    // 4. BLAKE3 over the EXACT committed wasm bytes (not a rebuild).
    let hash_hex = blake3::hash(wasm_bytes).to_hex().to_string();

    // 5. Signed body = body + embedded `[component]`. This whole slice is what
    //    the verifier recovers via `\n[signature]` stripping, so `[component]`
    //    is authenticated.
    let signed_body =
        format!("{body_trimmed}\n\n[component]\nhash_alg = \"blake3\"\nhash = \"{hash_hex}\"\n");

    // 6. Sign the signed payload.
    let signature = signing_key.sign(signed_body.as_bytes());
    let sig_hex = hex_encode(&signature.to_bytes());
    let pubkey_hex = hex_encode(signing_key.verifying_key().as_bytes());

    // 7. Assemble the full manifest. The single `\n` before `[signature]` is the
    //    boundary marker the verifier slices at; `signed_body` already ends with
    //    `\n`, giving `...\n\n[signature]` (matches the known-good fixtures).
    let full = format!(
        "{signed_body}\n[signature]\nalgorithm = \"ed25519\"\npublic_key = \"{pubkey_hex}\"\nsig_bytes = \"{sig_hex}\"\nsigned_at = \"{signed_at}\"\n"
    );
    Ok(full.into_bytes())
}

/// Count all non-overlapping occurrences of `needle` in `haystack`.
///
/// Used by the pre-signing ambiguity check (`build_signed_manifest` step 0)
/// to detect a manifest body carrying more than one `[signature]` boundary
/// marker — the class `strip_from_marker`'s first-occurrence slice cannot
/// itself detect, since it always returns a prefix that is trivially clean.
fn count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut rest = haystack;
    while let Some(pos) = rest.find(needle) {
        count += 1;
        rest = &rest[pos + needle.len()..];
    }
    count
}

/// Count lines whose trimmed content exactly equals `header` (e.g.
/// `"[component]"`). Mirrors the header-line definition `strip_top_level_table`
/// uses to identify a table boundary, so the duplicate-check and the strip
/// share one definition of "table header line".
fn count_header_lines(body: &str, header: &str) -> usize {
    body.lines().filter(|line| line.trim() == header).count()
}

/// Return everything in `s` before the first occurrence of `marker`, or all of
/// `s` if the marker is absent.
fn strip_from_marker(s: &str, marker: &str) -> String {
    match s.find(marker) {
        Some(idx) => s[..idx].to_string(),
        None => s.to_string(),
    }
}

/// Remove a top-level `[table]` section (the header line plus every line up to
/// the next `[` table header or EOF) from TOML `body`. LF-oriented, matching the
/// canonicalization contract. A no-op when the table is absent.
fn strip_top_level_table(body: &str, table: &str) -> String {
    let header = format!("[{table}]");
    let lines: Vec<&str> = body.lines().collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == header {
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with('[') {
                i += 1;
            }
        } else {
            out.push(lines[i]);
            i += 1;
        }
    }
    out.join("\n")
}

/// Lowercase hex encoding. Mirrors `verify::hex_encode` so the `public_key`
/// fingerprint uses the identical encoding the verifier cross-checks.
pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Key handling
// ---------------------------------------------------------------------------

/// Generate a fresh Ed25519 keypair from the OS CSPRNG. Matches the per-run
/// keypair generation in the signing test fixtures (`rand::rngs::SysRng`).
pub fn generate_keypair() -> Result<SigningKey, CeremonyError> {
    let mut seed = [0u8; 32];
    rand::rngs::SysRng
        .try_fill_bytes(&mut seed)
        .map_err(|e| CeremonyError::Rng(e.to_string()))?;
    Ok(SigningKey::from_bytes(&seed))
}

/// Read a 32-byte raw Ed25519 seed from `path`, IN PLACE, and construct the
/// signing key. The bytes are used only to sign; they are never written back
/// out anywhere by this tool.
fn read_private_key(path: &Path) -> Result<SigningKey, CeremonyError> {
    warn_if_key_readable(path);
    let bytes = std::fs::read(path)
        .map_err(|e| CeremonyError::Io(format!("reading private key {}: {e}", path.display())))?;
    let seed: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| CeremonyError::BadKeyLength(bytes.len()))?;
    Ok(SigningKey::from_bytes(&seed))
}

/// Advisory: warn (stderr) if the private key file is group/world-accessible.
/// The runbook requires mode 600; this is defense-in-depth, not a hard gate
/// (custody is the maintainer's responsibility).
fn warn_if_key_readable(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            eprintln!(
                "sign-ceremony: WARNING: private key {} is mode {:o} — should be 600 \
                 (group/world access detected)",
                path.display(),
                mode
            );
        }
    }
}

/// Write the 32-byte seed to `path`, creating it with mode 600. Fails if the
/// path already exists (never clobber an existing key).
fn write_private_key(path: &Path, seed: &[u8; 32]) -> Result<(), CeremonyError> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| CeremonyError::Io(format!("creating key file {}: {e}", path.display())))?;
    f.write_all(seed)
        .map_err(|e| CeremonyError::Io(format!("writing key file {}: {e}", path.display())))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Subcommands (driven by the bin's arg parsing)
// ---------------------------------------------------------------------------

/// `keygen <key-out-path>`: generate a keypair, persist the private seed
/// (mode 600), print the public key hex to stdout.
pub fn cmd_keygen(out_path: &Path) -> Result<(), CeremonyError> {
    if out_path.exists() {
        return Err(CeremonyError::KeyExists(out_path.to_path_buf()));
    }
    let key = generate_keypair()?;
    write_private_key(out_path, &key.to_bytes())?;
    let pubkey_hex = hex_encode(key.verifying_key().as_bytes());
    println!("{pubkey_hex}");
    eprintln!(
        "sign-ceremony: wrote 32-byte Ed25519 private seed to {} (mode 600)",
        out_path.display()
    );
    eprintln!(
        "sign-ceremony: public key (hex) on stdout — set BOTH ROOT and ROOT_PIN to this in round-2"
    );
    Ok(())
}

/// `sign <key-path> <wasm-path> <manifest-path>`: the ceremony. Reads the key in
/// place, hashes the committed wasm, embeds `[component]`, signs, self-verifies,
/// writes the manifest, and prints the public key hex.
pub fn cmd_sign(
    key_path: &Path,
    wasm_path: &Path,
    manifest_path: &Path,
) -> Result<(), CeremonyError> {
    let key = read_private_key(key_path)?;

    let wasm_bytes = std::fs::read(wasm_path)
        .map_err(|e| CeremonyError::Io(format!("reading wasm {}: {e}", wasm_path.display())))?;
    let manifest_bytes = std::fs::read(manifest_path).map_err(|e| {
        CeremonyError::Io(format!("reading manifest {}: {e}", manifest_path.display()))
    })?;

    let signed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let produced = build_signed_manifest(&key, &wasm_bytes, &manifest_bytes, &signed_at)?;

    // Fail-closed: prove the output verifies against the REAL runtime verifier
    // (with this key as the root) BEFORE persisting anything.
    let root = key.verifying_key().to_bytes();
    match verify_plugin(&produced, &root) {
        Ok(CoreStatus::Core) => {}
        other => return Err(CeremonyError::SelfVerifyFailed(format!("{other:?}"))),
    }

    std::fs::write(manifest_path, &produced).map_err(|e| {
        CeremonyError::Io(format!("writing manifest {}: {e}", manifest_path.display()))
    })?;

    let pubkey_hex = hex_encode(key.verifying_key().as_bytes());
    println!("{pubkey_hex}");
    eprintln!(
        "sign-ceremony: re-signed {} — embedded [component].hash = blake3({})",
        manifest_path.display(),
        wasm_path.display()
    );
    eprintln!(
        "sign-ceremony: public key (hex) on stdout — set BOTH ROOT and ROOT_PIN to this in round-2"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_top_level_table_removes_only_the_named_table() {
        let body = "[plugin]\nname = \"x\"\n\n[component]\nhash_alg = \"blake3\"\nhash = \"deadbeef\"\n\n[host_fns]\nrequired = []\n";
        let stripped = strip_top_level_table(body, "component");
        assert!(!stripped.contains("[component]"));
        assert!(stripped.contains("[plugin]"));
        assert!(stripped.contains("[host_fns]"));
    }

    #[test]
    fn strip_from_marker_returns_all_when_absent() {
        assert_eq!(strip_from_marker("abc", "\n[signature]"), "abc");
        assert_eq!(strip_from_marker("a\n[signature]\nx", "\n[signature]"), "a");
    }

    /// Fix 3 (Warden T3, conf 80): a manifest body carrying more than one
    /// `[signature]` boundary marker — e.g. a decoy occurrence embedded ahead
    /// of the real trailing block — must be rejected fail-closed rather than
    /// silently mis-sliced by `strip_from_marker`'s first-occurrence search.
    #[test]
    fn build_signed_manifest_rejects_manifest_with_duplicate_signature_marker() {
        let key = generate_keypair().expect("keypair generation");
        // Two occurrences of the `\n[signature]` marker in the raw input.
        let manifest = b"[plugin]\nname = \"x\"\ncore = true\n\n\
                          [signature]\ndecoy = \"true\"\n\n\
                          [signature]\nalgorithm = \"ed25519\"\n";
        let err = build_signed_manifest(&key, b"wasm-bytes", manifest, "2026-01-01T00:00:00Z")
            .expect_err("duplicate [signature] marker must be rejected");
        assert!(
            matches!(err, CeremonyError::EmbeddedMarker(_)),
            "expected EmbeddedMarker, got {err:?}"
        );
    }

    /// Same hardening, for the `[component]` table header: more than one
    /// top-level `[component]` line (embedded in a value, or duplicated) is
    /// ambiguous input `strip_top_level_table` cannot safely disambiguate.
    #[test]
    fn build_signed_manifest_rejects_manifest_with_duplicate_component_header() {
        let key = generate_keypair().expect("keypair generation");
        let manifest = b"[plugin]\nname = \"x\"\ncore = true\n\n\
                          [component]\nhash_alg = \"blake3\"\nhash = \"deadbeef\"\n\n\
                          [component]\nhash_alg = \"blake3\"\nhash = \"decafbad\"\n";
        let err = build_signed_manifest(&key, b"wasm-bytes", manifest, "2026-01-01T00:00:00Z")
            .expect_err("duplicate [component] header must be rejected");
        assert!(
            matches!(err, CeremonyError::EmbeddedMarker(_)),
            "expected EmbeddedMarker, got {err:?}"
        );
    }
}
