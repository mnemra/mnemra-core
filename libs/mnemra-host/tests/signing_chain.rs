//! Signing-chain invariant tests — RED phase (Task 16 of TDD pair 16/17).
//!
//! # Purpose
//!
//! These tests pin the signing-chain security invariants that Task 17 must
//! implement. Every test traces to a spec R-ID. This is the adversarial
//! surface: the load-bearing tests are the REJECT paths, not the happy path.
//!
//! # RED-phase design
//!
//! Task 17 implements:
//!   - `mnemra_host::signing::verify` — `verify_plugin()` + `CoreStatus` + `SigningError`
//!   - `mnemra_host::signing::root_material` — `ROOT` embedded at build time
//!   - `mnemra_host::startup::file_mode_check` — `check()` for admin-token + signing material
//!
//! Until those modules land, this file does NOT compile. That compile-fail
//! IS the red signal — the right-reason failure for this phase.
//!
//! **`rustfmt` parses this file without type resolution; the file is fmt-clean.**
//! **`clippy` requires compilation; it cannot run until Task 17 lands.**
//!
//! See the deviation note below for the explicit accounting.
//!
//! # RED-phase deviation: compile-fail as red signal
//!
//! Unlike `admin_token.rs` (which uses `information_schema` — a compile-clean
//! seam that fails at runtime), there is no analogous seam for signing.
//! Every reject-path test must call `verify_plugin()` / `check()`, which do
//! not exist yet. The options are:
//!
//!   A. No real tests (comment-block handoff only) — satisfies clippy but
//!      delivers nothing assertable; the reject matrix is the deliverable.
//!   B. Genuine tests against the documented seam; compile-fail = red.
//!
//! Option B is correct per the dispatch ("a happy-path-only test is
//! worthless"). The compile error names exactly the missing Task-17 surface:
//!   `error[E0433]: failed to resolve: could not find 'signing' in 'mnemra_host'`
//!
//! Task 17 resolving that error IS the green-flip condition.
//!
//! # verify: [] rationale
//!
//! `verify: []` is correct by design. The test binary cannot be linked
//! (missing modules) until Task 17 lands. Recipes that compile the test
//! binary do not exist yet for this test. Task 17 populates `verify`.
//!
//! # No hardcoded key material
//!
//! All Ed25519 keypairs are generated per-run via `SigningKey::generate()`.
//! No key bytes, no seed bytes, no DER blobs appear literally in this file.
//!
//! # Manifest canonicalization contract
//!
//! The spec (P-0003) states: signatures are over `canonical(manifest minus
//! [signature])`. The canonical form for V0 is defined here as the binding
//! contract for Task 17:
//!
//!   1. Parse the manifest TOML.
//!   2. Remove the `[signature]` table entirely (all fields under it).
//!   3. Re-serialize to TOML bytes using deterministic (alphabetically sorted
//!      key) order. At V0 with fixed-shape manifests the order is fixed by
//!      table insertion order in the spec's example; Task 17 must match.
//!   4. Sign/verify those bytes.
//!
//! For test fixtures, the signed bytes are generated via a helper that
//! strips the `[signature]` section from the TOML literal and returns the
//! remaining UTF-8 bytes. Task 17's `verify_plugin()` must apply the same
//! strip-then-verify logic.
//!
//! The test helpers below implement the "sign" side of this contract so that
//! the test fixtures produce signatures the real verifier will accept (once
//! Task 17 implements the matching "verify" side).
//!
//! # Cross-dispatch handoff: exact API Task 17 must expose
//!
//! See the handoff section at the bottom of this file.

// ---------------------------------------------------------------------------
// Imports — the missing Task-17 modules are the red signal.
// ---------------------------------------------------------------------------

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use mnemra_host::signing::root_material::ROOT;
use mnemra_host::signing::verify::{CoreStatus, SigningError, verify_plugin};
use mnemra_host::startup::file_mode_check::check as check_file_mode;
use rand::TryRng;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Test keypair generation
// ---------------------------------------------------------------------------

/// Generate a fresh Ed25519 signing keypair using the OS CSPRNG.
///
/// No hardcoded key material — this is called fresh in every test that
/// needs a keypair. Both the "mnemra root" test key and the "attacker"
/// key are generated this way.
///
/// rand 0.10: `OsRng` was renamed to `SysRng` (re-exported from getrandom).
/// `rand::rngs::SysRng::default()` is the CSPRNG at rand 0.10.
fn generate_keypair() -> SigningKey {
    let mut seed = [0u8; 32];
    rand::rngs::SysRng
        .try_fill_bytes(&mut seed)
        .expect("SysRng fill failed");
    SigningKey::from_bytes(&seed)
}

// ---------------------------------------------------------------------------
// Manifest fixture helpers
// ---------------------------------------------------------------------------

/// Build a minimal valid `core = true` manifest TOML, ready to be signed.
///
/// The `[signature]` section is NOT included — this is the canonical form
/// (manifest-minus-signature) that the signing contract operates over.
///
/// Task 17's `verify_plugin()` must strip the `[signature]` table from the
/// full manifest before verifying — the bytes must match what `sign_manifest`
/// signs.
fn manifest_bytes_unsigned(name: &str, version: &str, core: bool) -> Vec<u8> {
    // Deterministic TOML layout — matches P-0003 canonical form.
    // The [signature] table is absent; this is the signed payload.
    format!(
        r#"[plugin]
name = "{name}"
version = "{version}"
schema_version = 1
core = {core}

[verbs]
exposed = ["task.create", "task.update", "task.get", "task.list"]

[content_types]
task = {{ table = "tasks", schema_doc = "docs/schemas/task.md" }}

[state_scopes]

[host_fns]
required = [
  "artifact.create",
  "artifact.update",
  "artifact.get",
  "artifact.list",
  "artifact.delete",
  "metrics.record",
  "log.emit",
  "event.emit",
  "projection.emit",
]
optional = []
"#
    )
    .into_bytes()
}

/// Sign the manifest's canonical bytes with the given signing key and return
/// the detached Ed25519 signature bytes (64 bytes).
///
/// This is the "build-pipeline" side of the signing contract. Task 17
/// implements the matching "verify" side.
fn sign_manifest(signing_key: &SigningKey, manifest_bytes: &[u8]) -> Vec<u8> {
    signing_key.sign(manifest_bytes).to_bytes().to_vec()
}

/// Build the full manifest TOML including the `[signature]` section.
///
/// The verifier (Task 17) must strip the `[signature]` table before
/// verifying — the bytes passed to `ed25519_dalek::verify` must be
/// identical to the output of `manifest_bytes_unsigned()`.
fn manifest_with_signature(
    name: &str,
    version: &str,
    core: bool,
    verifying_key: &VerifyingKey,
    sig_bytes: &[u8],
) -> Vec<u8> {
    let unsigned = manifest_bytes_unsigned(name, version, core);
    let unsigned_str = String::from_utf8(unsigned).unwrap();

    // Public key as hex for the manifest field (fingerprint convention).
    let pubkey_hex = hex_encode(verifying_key.as_bytes());
    // Signature as hex.
    let sig_hex = hex_encode(sig_bytes);

    let full = format!(
        r#"{unsigned_str}
[signature]
algorithm = "ed25519"
public_key = "{pubkey_hex}"
sig_bytes = "{sig_hex}"
signed_at = "2026-06-13T00:00:00Z"
"#
    );
    full.into_bytes()
}

/// Minimal hex encoding for key/signature material in manifest fixtures.
///
/// This avoids pulling in a hex crate; the hex encoding is only used in
/// manifest TOML fixtures, not as a security primitive.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// File-mode fixture helpers
// ---------------------------------------------------------------------------

/// Create a file in `dir` with the given Unix permission bits.
fn create_file_with_mode(dir: &TempDir, name: &str, mode: u32) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, b"fixture content").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    path
}

// ---------------------------------------------------------------------------
// R-0005-a — Synchronous verify: API shape is a synchronous Result
// ---------------------------------------------------------------------------

/// Assert the verify API is synchronous: `verify_plugin` returns `Result`,
/// not `Future`. The seam signature `fn verify_plugin(...) -> Result<...>`
/// (non-async) is the structural pin.
///
/// # R-0005-a
///
/// "Plugin signature verification SHALL be synchronous on plugin load; the
/// host SHALL NOT create a plugin instance until `verify()` returns `Ok`;
/// no 'verify-async' or 'defer to background' path SHALL exist."
///
/// This test pins the synchronous shape: the return type is a concrete
/// `Result`, not `impl Future<Output = Result<...>>`. Task 17 MUST NOT
/// make `verify_plugin` async. The compile-time type check IS the test —
/// calling `verify_plugin(...)` without `.await` would fail to compile if
/// Task 17 made it async.
///
/// Red: `mnemra_host::signing::verify` does not exist — compile fail.
/// Green: `verify_plugin` returns `Result<CoreStatus, SigningError>` synchronously.
#[test]
fn verify_api_is_synchronous() {
    // R-0005-a: verify_plugin must be a non-async fn returning Result.
    // If Task 17 mistakenly made it async, this call without .await would
    // produce a compile error "unused implementor of Future". That error IS
    // the test. A sync return type makes the call produce a concrete Result
    // that we can match immediately.
    let signing_key = generate_keypair();
    let verifying_key = signing_key.verifying_key();
    let manifest = manifest_bytes_unsigned("tasks", "0.2.0", true);
    let sig = sign_manifest(&signing_key, &manifest);
    let full_manifest = manifest_with_signature("tasks", "0.2.0", true, &verifying_key, &sig);

    // This call MUST be usable without .await. If it were Future-returning,
    // the absence of .await would make the result an unconsumed Future, which
    // clippy::unused_must_use would catch. The type assertion here is:
    // verify_plugin returns Result, not Future<Output=Result>.
    let result: Result<CoreStatus, SigningError> =
        verify_plugin(&full_manifest, &verifying_key.to_bytes());

    // With a valid root key and valid signature, it must be Ok(Core).
    // (This also serves as the happy-path control case.)
    assert!(
        matches!(result, Ok(CoreStatus::Core)),
        "valid root-signed core=true manifest must verify as Ok(Core) (R-0005-a); \
         got {result:?}"
    );
}

// ---------------------------------------------------------------------------
// R-0005-b — Malformed signature → structured reject naming plugin name+version
// ---------------------------------------------------------------------------

/// Assert a malformed (truncated) signature rejects the load with a structured
/// error that names the plugin's `name` and `version`.
///
/// # R-0005-b
///
/// "If signature verification fails (malformed signature, unknown key,
/// certificate-chain break), the plugin load SHALL be rejected with a
/// structured error naming the plugin; no 'best-effort load' path is permitted."
///
/// Red: `verify_plugin` does not exist — compile fail.
/// Green: returns `Err(SigningError { plugin_name: "tasks", plugin_version: "0.2.0", .. })`.
#[test]
fn malformed_signature_rejects_with_named_error() {
    // R-0005-b: malformed sig → Err, error names plugin name+version.
    let signing_key = generate_keypair();
    let verifying_key = signing_key.verifying_key();

    let manifest = manifest_bytes_unsigned("tasks", "0.2.0", true);
    let mut sig = sign_manifest(&signing_key, &manifest);
    // Corrupt the signature: flip every byte — guaranteed invalid.
    for byte in sig.iter_mut() {
        *byte ^= 0xFF;
    }
    let full_manifest = manifest_with_signature("tasks", "0.2.0", true, &verifying_key, &sig);

    let result: Result<CoreStatus, SigningError> =
        verify_plugin(&full_manifest, &verifying_key.to_bytes());

    assert!(
        result.is_err(),
        "malformed signature must be rejected (R-0005-b); got Ok"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.plugin_name(),
        "tasks",
        "error must name the plugin name 'tasks' (R-0005-b); got '{}'",
        err.plugin_name()
    );
    assert_eq!(
        err.plugin_version(),
        "0.2.0",
        "error must name the plugin version '0.2.0' (R-0005-b); got '{}'",
        err.plugin_version()
    );
}

/// Assert a zero-length signature rejects the load with a named error.
///
/// # R-0005-b
///
/// A zero-length signature is the most malformed possible input; it should
/// be rejected before any cryptographic operation is attempted.
#[test]
fn empty_signature_rejects_with_named_error() {
    // R-0005-b: empty/zero-length sig → Err, error names plugin.
    let signing_key = generate_keypair();
    let verifying_key = signing_key.verifying_key();
    let empty_sig: Vec<u8> = vec![];
    let full_manifest =
        manifest_with_signature("contacts", "0.2.0", true, &verifying_key, &empty_sig);

    let result: Result<CoreStatus, SigningError> =
        verify_plugin(&full_manifest, &verifying_key.to_bytes());

    assert!(
        result.is_err(),
        "empty/zero-length signature must be rejected (R-0005-b)"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.plugin_name(),
        "contacts",
        "error must name the plugin (R-0005-b); got '{}'",
        err.plugin_name()
    );
}

// ---------------------------------------------------------------------------
// R-0005-b — Unknown key → structured reject
// ---------------------------------------------------------------------------

/// Assert a signature from an unknown (attacker) key rejects the load.
///
/// # R-0005-b
///
/// An attacker generates their own keypair and signs a manifest. The runtime
/// verifies against the embedded root material — the attacker's key is not
/// the root key, so the signature does not verify. Reject with named error.
///
/// Red: `verify_plugin` does not exist — compile fail.
/// Green: `Err(SigningError { .. })` with plugin name+version.
#[test]
fn unknown_key_rejects_with_named_error() {
    // R-0005-b: unknown/attacker key → Err, error names plugin.
    let root_key = generate_keypair();
    let attacker_key = generate_keypair(); // Unknown — not the root.

    // Signed by attacker but presented with attacker's verifying key
    // embedded in the manifest. The verify call uses the ROOT key bytes,
    // not the key declared in the manifest field.
    let manifest = manifest_bytes_unsigned("repos", "0.2.0", true);
    let sig = sign_manifest(&attacker_key, &manifest);
    let full_manifest =
        manifest_with_signature("repos", "0.2.0", true, &attacker_key.verifying_key(), &sig);

    // Verify against the ROOT key (not the attacker's key).
    let result: Result<CoreStatus, SigningError> =
        verify_plugin(&full_manifest, &root_key.verifying_key().to_bytes());

    assert!(
        result.is_err(),
        "signature from unknown/attacker key must be rejected (R-0005-b); got Ok"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.plugin_name(),
        "repos",
        "error must name the plugin (R-0005-b); got '{}'",
        err.plugin_name()
    );
    assert_eq!(
        err.plugin_version(),
        "0.2.0",
        "error must name the version (R-0005-b); got '{}'",
        err.plugin_version()
    );
}

/// Assert a signature that was made with the root key but presented under
/// a different (attacker-controlled) verifying key in the manifest is rejected.
///
/// # R-0005-b — chain break variant
///
/// This is a certificate-chain break: the manifest's `public_key` field
/// does not match the key that actually signed the bytes. The verifier
/// must detect the mismatch and reject.
#[test]
fn chain_break_rejects_with_named_error() {
    // R-0005-b: cert-chain break → Err (valid sig, wrong public_key field).
    let root_key = generate_keypair();
    let attacker_key = generate_keypair();

    let manifest = manifest_bytes_unsigned("jobs", "0.2.0", true);
    // Signed by root key...
    let sig = sign_manifest(&root_key, &manifest);
    // ...but manifest records attacker's public key as the signer.
    let full_manifest = manifest_with_signature(
        "jobs",
        "0.2.0",
        true,
        &attacker_key.verifying_key(), // chain break: wrong key in manifest
        &sig,
    );

    // Verify against root material. The sig was made by root so it WOULD
    // verify against the root key, but the manifest declares a different
    // public key — the verifier must check consistency and reject.
    // NOTE: Task 17 must either:
    //   (a) verify sig against ROOT directly (ignoring manifest public_key field
    //       as a trust signal), OR
    //   (b) verify sig against manifest public_key AND check that key chains
    //       to ROOT.
    // Either way this specific case (root-signed, attacker-declared-key,
    // verified against root) must produce Err, because the manifest field
    // and the root are inconsistent. Task 17 must document which model it uses.
    let result: Result<CoreStatus, SigningError> =
        verify_plugin(&full_manifest, &root_key.verifying_key().to_bytes());

    assert!(
        result.is_err(),
        "chain break (valid sig but wrong public_key in manifest) must be rejected (R-0005-b); \
         got Ok"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.plugin_name(),
        "jobs",
        "error must name the plugin (R-0005-b)"
    );
}

// ---------------------------------------------------------------------------
// R-0005-h — core=true signed by attacker key → reject (provenance-not-field)
// ---------------------------------------------------------------------------

/// THE CENTERPIECE TEST: core=true manifest signed by an attacker key must be
/// rejected even though the manifest field says `core = true`.
///
/// # R-0005-h (load-bearing)
///
/// "The runtime SHALL determine `core` status by signature provenance, NOT by
/// manifest-field trust. A manifest carrying `core = true` SHALL be honored as
/// core ONLY when its signature chains to the mnemra root verification material;
/// a manifest with `core = true` signed by any other key SHALL be rejected at
/// load, regardless of whether non-core plugin installation is enabled."
///
/// This test makes it structurally impossible to satisfy with an always-Err
/// stub (because the R-0005-a happy path MUST return Ok(Core)) and impossible
/// to satisfy with a manifest-field-trust implementation (because this test
/// proves field trust is wrong).
///
/// Control: root-signed core=true → Ok(Core).
/// Attack:  attacker-signed core=true → Err (not Ok(NonCore), not Ok(Core)).
///
/// A manifest-field-trust implementation would return Ok(Core) for both
/// (it reads `core = true` and trusts it). The test MUST return Err for
/// the attacker case, which kills that implementation.
///
/// Red: `verify_plugin` does not exist — compile fail.
/// Green: returns Err for attacker key even though `core = true`.
#[test]
fn core_true_attacker_signed_is_rejected_provenance_not_field() {
    // R-0005-h: core=true signed by attacker → Err, NOT Ok(Core) or Ok(NonCore).
    let root_key = generate_keypair();
    let attacker_key = generate_keypair();

    // CONTROL: root-signed core=true → Ok(Core).
    {
        let manifest = manifest_bytes_unsigned("tasks", "0.2.0", true);
        let sig = sign_manifest(&root_key, &manifest);
        let full_manifest =
            manifest_with_signature("tasks", "0.2.0", true, &root_key.verifying_key(), &sig);

        let result: Result<CoreStatus, SigningError> =
            verify_plugin(&full_manifest, &root_key.verifying_key().to_bytes());
        assert!(
            matches!(result, Ok(CoreStatus::Core)),
            "root-signed core=true must verify as Ok(Core) — control case (R-0005-h); \
             got {result:?}"
        );
    }

    // ATTACK: attacker-signed core=true → Err (NOT Ok(Core)).
    // Same manifest content, different signing key.
    {
        let manifest = manifest_bytes_unsigned("tasks", "0.2.0", true);
        let sig = sign_manifest(&attacker_key, &manifest); // NOT root key
        let full_manifest =
            manifest_with_signature("tasks", "0.2.0", true, &attacker_key.verifying_key(), &sig);

        let result: Result<CoreStatus, SigningError> =
            verify_plugin(&full_manifest, &root_key.verifying_key().to_bytes());
        assert!(
            result.is_err(),
            "core=true signed by attacker key must be REJECTED (R-0005-h); \
             field says core=true but provenance is wrong; \
             got {result:?}"
        );
        // Specifically must not return Ok(Core) — that would be field-trust.
        assert!(
            !matches!(result, Ok(CoreStatus::Core)),
            "core=true attacker-signed must NOT return Ok(Core) — \
             that would be manifest-field trust, not provenance (R-0005-h)"
        );
        let err = result.unwrap_err();
        assert_eq!(
            err.plugin_name(),
            "tasks",
            "error must name the plugin (R-0005-h)"
        );
        assert_eq!(
            err.plugin_version(),
            "0.2.0",
            "error must name the version (R-0005-h)"
        );
    }
}

// ---------------------------------------------------------------------------
// R-0005-g — core=true is required; non-core plugin → reject
// ---------------------------------------------------------------------------

/// Assert any plugin whose manifest does not carry `core: true` signed by
/// the mnemra root is rejected at load.
///
/// # R-0005-g
///
/// "The runtime SHALL reject at V0 any plugin whose manifest does not carry
/// `core: true` signed by the mnemra root; non-core plugin installation is
/// blocked at the load path."
///
/// Note: R-0005-g and R-0003-e overlap but are distinct:
///   R-0003-e — reject core=false manifests (schema validation gate)
///   R-0005-g — reject any manifest not carrying core=true signed by root
///              (signing provenance gate)
///
/// This test exercises the signing provenance gate: a root-signed manifest
/// where `core = false` is rejected. The field value is the rejection reason
/// here (not the signature); R-0003-e is the applicable R-ID.
/// See `core_false_manifest_rejected_at_load` below for the dedicated R-0003-e test.
///
/// Red: `verify_plugin` does not exist — compile fail.
#[test]
fn no_core_true_in_manifest_is_rejected() {
    // R-0005-g: manifest without core=true signed by root → Err.
    let root_key = generate_keypair();

    // A root-signed manifest with core=false. The runtime should reject
    // this at the schema/field level (R-0003-e) OR the signing-provenance
    // level. Either way, the outcome must be Err.
    let manifest = manifest_bytes_unsigned("tasks", "0.2.0", false); // core=false
    let sig = sign_manifest(&root_key, &manifest);
    let full_manifest =
        manifest_with_signature("tasks", "0.2.0", false, &root_key.verifying_key(), &sig);

    let result: Result<CoreStatus, SigningError> =
        verify_plugin(&full_manifest, &root_key.verifying_key().to_bytes());

    assert!(
        result.is_err(),
        "manifest without core=true (even if root-signed) must be rejected (R-0005-g, R-0003-e); \
         got Ok"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.plugin_name(),
        "tasks",
        "error must name the plugin (R-0005-g)"
    );
}

// ---------------------------------------------------------------------------
// R-0003-e — core=false manifest → reject at load
// ---------------------------------------------------------------------------

/// Assert any plugin manifest where `core = false` is rejected at load time.
///
/// # R-0003-e
///
/// "The runtime SHALL reject at load time any plugin manifest where
/// `core = false`; non-core plugin installation is V0.1+ scope."
///
/// This is a schema-level rejection that occurs regardless of signature
/// validity. A correctly root-signed `core=false` manifest is still rejected.
///
/// Red: `verify_plugin` does not exist — compile fail.
/// Green: returns `Err` for `core=false` even when the signature is valid.
#[test]
fn core_false_manifest_rejected_at_load() {
    // R-0003-e: core=false → Err regardless of signature validity.
    let root_key = generate_keypair();

    let manifest = manifest_bytes_unsigned("third-party-plugin", "1.0.0", false);
    let sig = sign_manifest(&root_key, &manifest);
    let full_manifest = manifest_with_signature(
        "third-party-plugin",
        "1.0.0",
        false,
        &root_key.verifying_key(),
        &sig,
    );

    // Even though this is signed by the root key, core=false must be rejected.
    let result: Result<CoreStatus, SigningError> =
        verify_plugin(&full_manifest, &root_key.verifying_key().to_bytes());

    assert!(
        result.is_err(),
        "core=false manifest must be rejected at load (R-0003-e), \
         even when root-signed; non-core install is V0.1+ scope; got Ok"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.plugin_name(),
        "third-party-plugin",
        "error must name the plugin (R-0003-e)"
    );
    assert_eq!(
        err.plugin_version(),
        "1.0.0",
        "error must name the version (R-0003-e)"
    );
}

// ---------------------------------------------------------------------------
// R-0005-d — embedded root material: no runtime key-fetch path
// ---------------------------------------------------------------------------

/// Assert the embedded root material is accessible as a compile-time constant.
///
/// # R-0005-d
///
/// "The verification material (root public key / cert) SHALL be embedded in
/// the mnemra-core binary at build time; no runtime key-fetch path is
/// permitted at V0."
///
/// This pins the seam shape: `signing::root_material::ROOT` is a `&[u8]`
/// (or `[u8; 32]`) constant, not a function that reads from a file or
/// network. The absence of `async fn root_material()` or `fn load_root()`
/// is the structural guarantee.
///
/// Red: `mnemra_host::signing::root_material` does not exist — compile fail.
/// Green: `ROOT` is a `const &[u8]` or `const [u8; 32]` — a compile-time
///        constant; no function call required to access it.
#[test]
fn embedded_root_material_is_a_compile_time_constant() {
    // R-0005-d: ROOT must be a compile-time constant (not loaded at runtime).
    // `ROOT` references itself — if it were a function returning a Runtime
    // type, this test would look different. The type of ROOT must be a const
    // byte slice or fixed-size array, not a struct with a constructor.
    let root_bytes: &[u8] = ROOT;

    // The exact length pins the Ed25519 public key size (32 bytes).
    assert_eq!(
        root_bytes.len(),
        32,
        "ROOT must be a 32-byte Ed25519 public key (R-0005-d); got {} bytes",
        root_bytes.len()
    );

    // ROOT must be non-zero — an all-zero key is an obvious placeholder that
    // would accept any signature (ed25519_dalek accepts all-zero as a
    // verifying key but it has no corresponding private key in practice).
    assert!(
        root_bytes.iter().any(|b| *b != 0),
        "ROOT must not be all-zero bytes — a real build-embedded key (R-0005-d)"
    );
}

// ---------------------------------------------------------------------------
// R-0005-f — startup file-mode check: mode 600 required for both files
// ---------------------------------------------------------------------------

/// Assert the startup check passes when BOTH the admin-token file and the
/// signing-material file are mode 600.
///
/// # R-0005-f
///
/// "On host startup, the system SHALL check that the admin-token file and
/// signing-verification-material file are both mode 600 and not world-readable;
/// if either check fails, the host SHALL refuse to start."
///
/// Red: `mnemra_host::startup::file_mode_check` does not exist — compile fail.
/// Green: `check(token_path, signing_material_path)` returns `Ok(())`.
#[test]
fn startup_file_mode_check_passes_for_mode_600() {
    // R-0005-f: both files at mode 600 → check() returns Ok.
    let dir = TempDir::new().unwrap();
    let token_path = create_file_with_mode(&dir, "admin_token", 0o600);
    let signing_material_path = create_file_with_mode(&dir, "root_verification.pub", 0o600);

    let result = check_file_mode(&token_path, &signing_material_path);

    assert!(
        result.is_ok(),
        "startup file-mode check must pass when both files are mode 600 (R-0005-f); \
         got Err: {result:?}"
    );
}

/// Assert the startup check FAILS (host refuses to start) when the
/// admin-token file is world-readable (mode 644).
///
/// # R-0005-f
///
/// Red: `mnemra_host::startup::file_mode_check` does not exist — compile fail.
/// Green: `check(...)` returns `Err` when token file is mode 644.
#[test]
fn startup_check_fails_if_admin_token_is_world_readable() {
    // R-0005-f: admin-token at mode 644 → check() returns Err (host refuses start).
    let dir = TempDir::new().unwrap();
    let token_path = create_file_with_mode(&dir, "admin_token", 0o644); // world-readable
    let signing_material_path = create_file_with_mode(&dir, "root_verification.pub", 0o600);

    let result = check_file_mode(&token_path, &signing_material_path);

    assert!(
        result.is_err(),
        "startup file-mode check must FAIL (refuse start) when admin-token is \
         world-readable (mode 644) (R-0005-f); got Ok"
    );
}

/// Assert the startup check FAILS when the signing-material file is
/// world-readable (mode 644), even if the admin-token file is mode 600.
///
/// # R-0005-f
///
/// Both files must be checked; a world-readable signing-material file is a
/// separate failure from a world-readable token file. The host refuses to
/// start if EITHER check fails.
///
/// Red: `mnemra_host::startup::file_mode_check` does not exist — compile fail.
/// Green: `check(...)` returns `Err` when signing material is mode 644.
#[test]
fn startup_check_fails_if_signing_material_is_world_readable() {
    // R-0005-f: signing-material at mode 644 → check() returns Err (host refuses start).
    let dir = TempDir::new().unwrap();
    let token_path = create_file_with_mode(&dir, "admin_token", 0o600);
    let signing_material_path = create_file_with_mode(&dir, "root_verification.pub", 0o644); // world-readable

    let result = check_file_mode(&token_path, &signing_material_path);

    assert!(
        result.is_err(),
        "startup file-mode check must FAIL (refuse start) when signing-material is \
         world-readable (mode 644) (R-0005-f); got Ok"
    );
}

/// Assert the startup check FAILS when BOTH files are world-readable.
///
/// # R-0005-f
///
/// The worst case: neither file has been secured. The host must refuse to start.
#[test]
fn startup_check_fails_if_both_files_are_world_readable() {
    // R-0005-f: both files world-readable → check() returns Err.
    let dir = TempDir::new().unwrap();
    let token_path = create_file_with_mode(&dir, "admin_token", 0o644);
    let signing_material_path = create_file_with_mode(&dir, "root_verification.pub", 0o644);

    let result = check_file_mode(&token_path, &signing_material_path);

    assert!(
        result.is_err(),
        "startup file-mode check must FAIL when both files are world-readable (R-0005-f)"
    );
}

// ===========================================================================
// Adversarial byte-slice canonicalization tests (Glitch seal — dispatch 1018)
// ===========================================================================
//
// Context: The green impl (dispatch 1016) recovers the signed payload by slicing
// the raw manifest at the FIRST occurrence of `\n[signature]` (see
// `signing/verify.rs` `extract_signed_payload`). This is NOT a re-serialize
// pass — it is a byte-exact slice.
//
// This section probes the attack surface of that byte-slice boundary:
//
//   1. `\n[signature]` substring INSIDE a field value → wrong cut point
//   2. CRLF vs LF line endings → trailing `\r` shifts the signed bytes
//   3. `[signature]` appears at byte 0 (no leading `\n`) → marker not found
//   4. Trailing content appended AFTER `[signature]` section → only pre-sig
//      bytes are signed; post-sig content is sliced away (correct behavior)
//   5. Duplicate `[signature]` sections → TOML parse rejects at step 1
//
// For each layout attack that exercises the slice path (not a prior gate), the
// test includes a POSITIVE CONTROL that passes all pre-slice gates and verifies
// the correct impl's behavior before applying the attack mutation.
//
// Rule: if a test added here FAILS, that means the correct impl rejects an
// attack case that should be accepted, or accepts one that should be rejected.
// Either is a real finding — do NOT weaken the test to force it green.

// ---------------------------------------------------------------------------
// Adversarial #1: `\n[signature]` substring INSIDE a field value
// ---------------------------------------------------------------------------

/// Attack: inject `\n[signature]` as a literal substring inside a `[plugin]`
/// string field. The byte-slice search finds this injected occurrence BEFORE
/// the real `[signature]` section — cutting the "signed payload" too early
/// (a truncated byte slice). The signature was made over the FULL unsigned
/// bytes, so the truncated payload does not match → crypto fails → `Err`.
///
/// Verifies: the impl does NOT silently verify a truncated payload against
/// the original signature, and does NOT accept the manifest.
///
/// This test also includes a positive CONTROL: the same manifest without the
/// injected marker verifies correctly, proving the rejection is caused by the
/// injection rather than an earlier gate.
#[test]
fn adversarial_infield_signature_marker_causes_rejection() {
    // Build a signing key and sign an honest manifest first (the control).
    let root_key = generate_keypair();
    let root_vk = root_key.verifying_key();

    // CONTROL: a normal manifest without embedded marker → Ok(Core).
    {
        let unsigned = manifest_bytes_unsigned("tasks", "0.2.0", true);
        let sig = sign_manifest(&root_key, &unsigned);
        let full = manifest_with_signature("tasks", "0.2.0", true, &root_vk, &sig);
        let result: Result<CoreStatus, SigningError> = verify_plugin(&full, &root_vk.to_bytes());
        assert!(
            matches!(result, Ok(CoreStatus::Core)),
            "control: honest manifest must verify — adversarial_infield #1 control; got {result:?}"
        );
    }

    // ATTACK: construct unsigned bytes where a field value contains the literal
    // substring `\n[signature]`. Use raw bytes to embed it, bypassing TOML
    // string-escape normalization. We build a manifest where the schema_doc
    // field in [content_types] contains the injected text.
    //
    // The trick: sign the bytes that include the injection, then present the
    // same bytes with `[signature]` appended. The slice should cut at the
    // injected marker (too early), not at the real `[signature]` boundary.
    // Result: payload seen by verifier ≠ bytes signed by root → Err.
    let attack_unsigned: Vec<u8> = format!(
        // The `schema_doc` value embeds the marker. TOML parses this as a
        // string containing literal `\n[signature]` text — but the byte-slice
        // search operates on raw bytes, not on parsed TOML values.
        // We use a double-quoted TOML string with a literal backslash-n to
        // insert the newline byte directly in the raw bytes.
        "[plugin]\nname = \"tasks\"\nversion = \"0.2.0\"\nschema_version = 1\ncore = true\n\n[verbs]\nexposed = [\"task.create\"]\n\n[content_types]\ntask = {{ table = \"tasks\", schema_doc = \"docs/schemas/task.md\n[signature]\nalgorithm = \\\"injected\\\"\" }}\n\n[state_scopes]\n\n[host_fns]\nrequired = []\noptional = []\n"
    ).into_bytes();

    // Sign the attack bytes under root.
    let sig = sign_manifest(&root_key, &attack_unsigned);

    // Build [signature] section using root's pubkey (passes Interp-B check).
    let pubkey_hex = hex_encode(root_vk.as_bytes());
    let sig_hex = hex_encode(&sig);
    // Append a real `[signature]` section after the attack payload.
    let mut attack_full = attack_unsigned.clone();
    let sig_section = format!(
        "\n[signature]\nalgorithm = \"ed25519\"\npublic_key = \"{pubkey_hex}\"\nsig_bytes = \"{sig_hex}\"\nsigned_at = \"2026-06-13T00:00:00Z\"\n"
    );
    attack_full.extend_from_slice(sig_section.as_bytes());

    // The signed payload was the full `attack_unsigned` bytes, but the verifier
    // slices at the FIRST `\n[signature]` — which is the injected one inside the
    // field value (in the raw bytes). The slice is therefore shorter than what
    // was signed, so crypto fails.
    let result: Result<CoreStatus, SigningError> = verify_plugin(&attack_full, &root_vk.to_bytes());

    // Must reject — do not silently accept a truncated/mismatched payload.
    // NOTE: The impl parses TOML at step 1 before slicing. If the injected
    // bytes produce a TOML parse error (e.g. unterminated string), the reject
    // still happens at step 1 (also Err). Either way `is_err()` is correct;
    // we do NOT assert the kind because SigningErrorKind is private.
    assert!(
        result.is_err(),
        "manifest with \\n[signature] embedded in raw field bytes must be rejected \
         (truncated payload attack); got Ok — canonicalization is unsound"
    );
}

// ---------------------------------------------------------------------------
// Adversarial #2: CRLF vs LF line endings
// ---------------------------------------------------------------------------

/// Verify: the canonicalization is LF-only. Any manifest with CRLF line endings
/// is ALWAYS rejected — even if the signature was produced over CRLF bytes.
///
/// Root cause: the marker search uses `b"\n[signature]"`. When line endings are
/// CRLF, the sequence in raw bytes is `...\r\n[signature]...`. The slice cuts at
/// the `\n` in that pair, so the sliced payload INCLUDES the trailing `\r` but
/// EXCLUDES the final `\n`. This means:
///
/// - LF-signed, CRLF-presented → slice includes trailing `\r` not in signed bytes → Err
/// - CRLF-signed, CRLF-presented → slice ends with trailing `\r` but signed bytes
///   also end with `\n` → slice ≠ signed bytes → Err
///
/// Both cases fail. The canonicalization enforces LF as the only valid format.
/// This is SAFE: the build pipeline produces LF manifests; CRLF is never valid
/// input at this boundary. CRLF rejection is a defense-in-depth property.
///
/// This test documents the LF-only constraint and verifies it is enforced.
#[test]
fn adversarial_crlf_manifests_always_rejected() {
    let root_key = generate_keypair();
    let root_vk = root_key.verifying_key();
    let pubkey_hex = hex_encode(root_vk.as_bytes());

    // LF-signed bytes (what `manifest_bytes_unsigned` produces — Unix newlines).
    let lf_unsigned = manifest_bytes_unsigned("tasks", "0.2.0", true);

    // CRLF version: replace every `\n` byte with `\r\n`.
    let crlf_unsigned: Vec<u8> = lf_unsigned
        .iter()
        .flat_map(|&b| {
            if b == b'\n' {
                vec![b'\r', b'\n']
            } else {
                vec![b]
            }
        })
        .collect();

    // Case A: Sign LF bytes, present CRLF manifest → Err (slice includes `\r`,
    // signed bytes do not).
    {
        let sig_lf = sign_manifest(&root_key, &lf_unsigned);
        let sig_hex = hex_encode(&sig_lf);
        let sig_section = format!(
            "\r\n[signature]\r\nalgorithm = \"ed25519\"\r\npublic_key = \"{pubkey_hex}\"\r\nsig_bytes = \"{sig_hex}\"\r\nsigned_at = \"2026-06-13T00:00:00Z\"\r\n"
        );
        let mut crlf_full = crlf_unsigned.clone();
        crlf_full.extend_from_slice(sig_section.as_bytes());

        let result: Result<CoreStatus, SigningError> =
            verify_plugin(&crlf_full, &root_vk.to_bytes());
        assert!(
            result.is_err(),
            "LF-signed manifest presented as CRLF must be rejected \
             (trailing \\r in slice ≠ LF-signed bytes); got Ok — CRLF not enforced"
        );
    }

    // Case B: Sign CRLF bytes, present CRLF manifest → also Err. The marker
    // `b"\n[signature]"` slices at the `\n` in `\r\n[signature]`, giving a
    // payload that ends with `\r` (missing the final `\n`). But the signed
    // bytes (crlf_unsigned) end with `\n`. Slice ≠ signed bytes → Err.
    {
        let sig_crlf = sign_manifest(&root_key, &crlf_unsigned);
        let sig_hex = hex_encode(&sig_crlf);
        let sig_section = format!(
            "\r\n[signature]\r\nalgorithm = \"ed25519\"\r\npublic_key = \"{pubkey_hex}\"\r\nsig_bytes = \"{sig_hex}\"\r\nsigned_at = \"2026-06-13T00:00:00Z\"\r\n"
        );
        let mut crlf_full = crlf_unsigned.clone();
        crlf_full.extend_from_slice(sig_section.as_bytes());

        let result: Result<CoreStatus, SigningError> =
            verify_plugin(&crlf_full, &root_vk.to_bytes());
        // CRLF round-trip also fails: slice ends with `\r` not `\n`; signed
        // bytes ended with `\n`. LF is the only valid line ending at this boundary.
        assert!(
            result.is_err(),
            "CRLF-signed CRLF-presented manifest must also be rejected — \
             the byte-slice marker enforces LF-only canonicalization \
             (slice of CRLF full at \\n[signature] strips the trailing \\n, \
             producing bytes that differ from what root signed); got Ok"
        );
    }
}

// ---------------------------------------------------------------------------
// Adversarial #3: `[signature]` at byte 0 — no leading `\n`
// ---------------------------------------------------------------------------

/// Attack: a malformed manifest where `[signature]` appears as the very first
/// bytes (no preceding `\n`). The marker `b"\n[signature]"` does not match at
/// byte 0 (the `\n` is missing), so `find_subsequence` returns `None` and
/// `extract_signed_payload` returns the ENTIRE input as the "payload".
///
/// The signature was made over a valid unsigned manifest, not over the full
/// malformed bytes, so crypto fails → Err.
///
/// Also verifies: no panic (no slice-out-of-bounds, no index error).
#[test]
fn adversarial_signature_at_byte_zero_is_rejected_without_panic() {
    let root_key = generate_keypair();
    let root_vk = root_key.verifying_key();
    let pubkey_hex = hex_encode(root_vk.as_bytes());

    // Sign a normal manifest (the reference bytes).
    let unsigned = manifest_bytes_unsigned("tasks", "0.2.0", true);
    let sig = sign_manifest(&root_key, &unsigned);
    let sig_hex = hex_encode(&sig);

    // Build a malformed manifest that starts with `[signature]` (no `\n` prefix).
    // The real unsigned content follows AFTER the sig section — a confusion attempt.
    let malformed = format!(
        "[signature]\nalgorithm = \"ed25519\"\npublic_key = \"{pubkey_hex}\"\nsig_bytes = \"{sig_hex}\"\nsigned_at = \"2026-06-13T00:00:00Z\"\n\n[plugin]\nname = \"tasks\"\nversion = \"0.2.0\"\nschema_version = 1\ncore = true\n"
    ).into_bytes();

    // Must not panic — `find_subsequence` returns None; the whole input is
    // treated as payload; crypto fails. The primary assertion is is_err(), but
    // the test itself panicking would indicate a slice-out-of-bounds bug.
    let result: Result<CoreStatus, SigningError> = verify_plugin(&malformed, &root_vk.to_bytes());

    assert!(
        result.is_err(),
        "manifest with [signature] at byte 0 (no leading \\n) must be rejected gracefully \
         (no panic, no slice-OOB); got Ok"
    );
}

// ---------------------------------------------------------------------------
// Adversarial #4: trailing content appended after `[signature]`
// ---------------------------------------------------------------------------

/// Verify: content appended AFTER the `[signature]` section has NO effect on
/// verification — the byte-slice discards everything from `\n[signature]`
/// onward, so post-section tampering does NOT create a forged payload.
///
/// This is a DIFFERENTIAL test:
///   A. Malicious bytes appended AFTER `[signature]` → Ok(Core) (they're sliced away)
///   B. Same malicious bytes inserted BEFORE `[signature]` → Err (breaks signed payload)
///
/// The differential proves: only pre-`\n[signature]` bytes are covered by the
/// signature; tampering post-section is inert within `verify_plugin`'s contract.
///
/// (Post-section content has no path to privilege escalation inside
/// `verify_plugin` — the result is a binary `Ok(Core)/Err` with no payload
/// passthrough. Whether the caller re-parses the full manifest for plugin-loading
/// semantics is a TOCTOU concern outside this function's boundary; recorded in
/// the dispatch report followups.)
#[test]
fn adversarial_trailing_content_after_signature_is_sliced_away() {
    let root_key = generate_keypair();
    let root_vk = root_key.verifying_key();

    let unsigned = manifest_bytes_unsigned("tasks", "0.2.0", true);
    let sig = sign_manifest(&root_key, &unsigned);
    let honest_full = manifest_with_signature("tasks", "0.2.0", true, &root_vk, &sig);

    // Case A: append malicious content AFTER the `[signature]` section.
    // The verifier slices at `\n[signature]` and sees exactly the original
    // `unsigned` bytes — tampering is silently discarded. Result: Ok(Core).
    let mut with_trailing = honest_full.clone();
    with_trailing.extend_from_slice(b"\n\n[fake_admin]\ntoken = \"hunter2\"\nescalate = true\n");

    let result_trailing: Result<CoreStatus, SigningError> =
        verify_plugin(&with_trailing, &root_vk.to_bytes());
    assert!(
        matches!(result_trailing, Ok(CoreStatus::Core)),
        "trailing content after [signature] must NOT invalidate verification — \
         the slice discards it; got {result_trailing:?}"
    );

    // Case B: insert the SAME malicious bytes BEFORE the `[signature]` section
    // (spliced into the honest unsigned bytes). This DOES change the signed
    // payload → crypto fails → Err.
    let evil_insertion = b"\n\n[fake_admin]\ntoken = \"hunter2\"\nescalate = true\n";
    let mut tampered_unsigned = unsigned.clone();
    tampered_unsigned.extend_from_slice(evil_insertion);

    // Re-sign would make this valid, but we DON'T re-sign — we use the original
    // signature over the original unsigned bytes, then append the [signature] block.
    let tampered_full = {
        let pubkey_hex = hex_encode(root_vk.as_bytes());
        let sig_hex = hex_encode(&sig);
        let sig_section = format!(
            "\n[signature]\nalgorithm = \"ed25519\"\npublic_key = \"{pubkey_hex}\"\nsig_bytes = \"{sig_hex}\"\nsigned_at = \"2026-06-13T00:00:00Z\"\n"
        );
        let mut v = tampered_unsigned;
        v.extend_from_slice(sig_section.as_bytes());
        v
    };

    let result_tampered: Result<CoreStatus, SigningError> =
        verify_plugin(&tampered_full, &root_vk.to_bytes());
    assert!(
        result_tampered.is_err(),
        "content inserted BEFORE [signature] must break verification \
         (signed payload differs from what root signed); got Ok — \
         canonicalization does not bind the full pre-sig payload"
    );
}

// ---------------------------------------------------------------------------
// Adversarial #5: duplicate `[signature]` sections
// ---------------------------------------------------------------------------

/// Attack: a manifest containing TWO `[signature]` tables. TOML does not allow
/// duplicate table headers; `toml::Value` parse (step 1 of the verifier) must
/// return an error, resulting in `Err` from `verify_plugin`.
///
/// This verifies: the verifier does not silently pick one of the two sections
/// (e.g. the attacker-controlled one) and accept an ambiguous manifest.
///
/// Note: because the parse fails at step 1, `plugin_name()` will be the sentinel
/// `"(unparseable)"` — we do NOT assert the name here.
#[test]
fn adversarial_duplicate_signature_sections_rejected() {
    let root_key = generate_keypair();
    let root_vk = root_key.verifying_key();

    let unsigned = manifest_bytes_unsigned("tasks", "0.2.0", true);
    let sig = sign_manifest(&root_key, &unsigned);
    let honest_full = manifest_with_signature("tasks", "0.2.0", true, &root_vk, &sig);
    let honest_str = String::from_utf8(honest_full).unwrap();

    // Append a second `[signature]` section. This makes the TOML document
    // have duplicate top-level keys, which is rejected by the TOML spec.
    let attacker_sig_hex = "0".repeat(128); // 64 attacker-controlled bytes (all-zero hex)
    let attacker_pubkey_hex = hex_encode(root_vk.as_bytes()); // doesn't matter — parse fails first
    let dup = format!(
        "[signature]\nalgorithm = \"ed25519\"\npublic_key = \"{attacker_pubkey_hex}\"\nsig_bytes = \"{attacker_sig_hex}\"\nsigned_at = \"2026-06-14T00:00:00Z\"\n"
    );
    let double_sig = format!("{honest_str}\n{dup}");

    let result: Result<CoreStatus, SigningError> =
        verify_plugin(double_sig.as_bytes(), &root_vk.to_bytes());

    // TOML parse must reject duplicate tables.
    // We do NOT assert plugin_name() == "tasks" because parse fails before
    // name extraction and the sentinel is "(unparseable)".
    assert!(
        result.is_err(),
        "manifest with duplicate [signature] sections must be rejected \
         (TOML parse rejects duplicate tables); got Ok — ambiguous manifest accepted"
    );
}

// ===========================================================================
// Cross-dispatch handoff contract
// ===========================================================================
//
// The following documents the EXACT seam Task 17 must expose. This is not
// executable Rust — it is the binding contract that Task 17 implements against.
//
// ## Module paths
//
//   mnemra_host::signing::verify
//   mnemra_host::signing::root_material
//   mnemra_host::startup::file_mode_check
//
//   These map to files:
//     libs/mnemra-host/signing/verify.rs
//     libs/mnemra-host/signing/root_material.rs
//     libs/mnemra-host/startup/file_mode_check.rs
//
//   Task 17 must also add `pub mod signing;` and `pub mod startup;` to
//   `libs/mnemra-host/mnemra_host.rs`, and the submodule declarations within
//   each module file.
//
// ## signing::verify
//
//   /// Verification outcome: core status is determined by signature provenance.
//   ///
//   /// `Core` means the manifest signature chained to the mnemra root material.
//   /// There is no `NonCore` variant at V0 — all non-core plugins are rejected.
//   #[derive(Debug, PartialEq)]
//   pub enum CoreStatus {
//       Core,
//   }
//
//   /// Structured verification failure. Names the plugin that was rejected.
//   ///
//   /// # Required accessor methods
//   ///
//   /// These tests call `err.plugin_name()` and `err.plugin_version()` —
//   /// Task 17 MUST expose these as `pub fn` on `SigningError`.
//   #[derive(Debug)]
//   pub struct SigningError {
//       // Task 17 owns the fields; the tests only access via accessors.
//       // Minimum required:
//       //   plugin_name: String,
//       //   plugin_version: String,
//       //   kind: SigningErrorKind,  // e.g. MalformedSignature, UnknownKey, CoreFalse, ...
//   }
//
//   impl SigningError {
//       pub fn plugin_name(&self) -> &str { ... }
//       pub fn plugin_version(&self) -> &str { ... }
//   }
//
//   /// Verify a plugin manifest's signature synchronously.
//   ///
//   /// # Parameters
//   ///
//   ///   manifest_toml: &[u8]
//   ///     Full manifest TOML bytes including the `[signature]` table.
//   ///
//   ///   root_material: &[u8]
//   ///     The 32-byte Ed25519 root verifying key bytes. At runtime this is
//   ///     `signing::root_material::ROOT`; in tests, a per-run generated key.
//   ///     This is a parameter (not a global) so tests can inject test keys.
//   ///
//   /// # Verification algorithm
//   ///
//   ///   1. Parse `manifest_toml` as TOML.
//   ///   2. Extract plugin `name` and `version` for error messages.
//   ///   3. Check `core = true`; if not, return Err(SigningError { core_false }).
//   ///   4. Extract `[signature]` table: `sig_bytes` (hex) and optionally
//   ///      `public_key` (hex).
//   ///   5. Remove the `[signature]` table from the parsed document and
//   ///      re-serialize to canonical TOML bytes (the "signed payload").
//   ///      Canonical form: the TOML sections in the ORDER they appear in this
//   ///      file's `manifest_bytes_unsigned()` helper — [plugin], [verbs],
//   ///      [content_types], [state_scopes], [host_fns]. No trailing newline
//   ///      differences; match the exact byte output of that helper.
//   ///   6. Verify `sig_bytes` over `signed_payload` using `root_material` as
//   ///      the Ed25519 verifying key. Use `ed25519_dalek::VerifyingKey::verify`.
//   ///   7. If verification succeeds, return Ok(CoreStatus::Core).
//   ///   8. If verification fails (wrong key, bad sig, wrong length), return
//   ///      Err(SigningError { plugin_name, plugin_version, kind: VerificationFailed }).
//   ///
//   /// # MUST NOT be async
//   ///
//   ///   R-0005-a: this function MUST be `fn`, not `async fn`.
//   ///
//   /// # Public_key field handling (chain_break_rejects_with_named_error)
//   ///
//   ///   The manifest's `public_key` field is informational only at V0.
//   ///   Verification is against `root_material` (the embedded root bytes),
//   ///   NOT against the manifest-declared `public_key`. This means:
//   ///     - `chain_break_rejects_with_named_error` expects Err because the
//   ///       sig was made by the root key but the manifest declares the attacker's
//   ///       public_key. Task 17 verifies against `root_material`; the sig was
//   ///       made by root; verification SUCCEEDS cryptographically, but the
//   ///       manifest declares a different public_key.
//   ///
//   ///   TASK 17 NOTE: `chain_break_rejects_with_named_error` creates an
//   ///   asymmetric case. Two valid interpretations:
//   ///
//   ///   Interpretation A (simpler): Verify sig_bytes against root_material
//   ///   only. Ignore the `public_key` manifest field entirely. Then for the
//   ///   chain-break test (root-signed, attacker pubkey in manifest), verify
//   ///   against root_material → SUCCEEDS → returns Ok(Core). The test assertion
//   ///   `result.is_err()` fails. Under Interp. A, the chain-break test needs
//   ///   adjustment (or it's not a chain-break — it's a valid root-signed manifest
//   ///   with an informational-only public_key field).
//   ///
//   ///   Interpretation B (stricter): Also verify that the manifest's
//   ///   `public_key` field matches `root_material`. Then the chain-break test
//   ///   (attacker pubkey in field, root-signed) fails verification correctly.
//   ///
//   ///   TASK 17 MUST DECIDE AND DOCUMENT which interpretation it implements.
//   ///   If Interpretation A: update the chain-break test to match.
//   ///   If Interpretation B: implement the pubkey field consistency check.
//   ///   Either is acceptable; the decision must be recorded in signing/verify.rs.
//   ///
//   pub fn verify_plugin(manifest_toml: &[u8], root_material: &[u8]) -> Result<CoreStatus, SigningError>;
//
// ## signing::root_material
//
//   /// The mnemra root verification key, embedded at build time.
//   ///
//   /// At V0 this is a 32-byte Ed25519 public key generated on the build host
//   /// and embedded via `include_bytes!` or a `const fn` from build.rs output.
//   ///
//   /// MUST be a `const` or `static` — no function call required to access it.
//   /// No runtime file read, no network fetch (R-0005-d).
//   ///
//   /// For the test binary this is a placeholder — the tests inject per-run keys
//   /// via the `root_material` parameter to `verify_plugin()`. The `ROOT` constant
//   /// is tested only for shape (32 bytes, non-zero) in
//   /// `embedded_root_material_is_a_compile_time_constant`.
//   ///
//   /// Build pipeline integration: the `verify-build` justfile recipe
//   /// (R-0018-f) MUST produce a binary with a real Ed25519 public key here.
//   pub static ROOT: &[u8];  // or pub const ROOT: [u8; 32];
//
// ## startup::file_mode_check
//
//   use std::path::Path;
//
//   #[derive(Debug)]
//   pub struct FileModeError {
//       pub path: std::path::PathBuf,
//       pub actual_mode: u32,
//       pub required_mode: u32,
//   }
//
//   impl std::fmt::Display for FileModeError { ... }
//   impl std::error::Error for FileModeError {}
//
//   /// Check that BOTH `token_path` and `signing_material_path` are mode 600.
//   ///
//   /// Returns Err if either file is world-readable (has any bits set in 0o007).
//   /// Returns Err if either file's mode is not exactly 0o600.
//   ///
//   /// Called on host startup before any plugin is loaded (R-0005-f).
//   /// If this returns Err, the host MUST refuse to start.
//   ///
//   /// Note: this function does NOT check owner UID. UID check is advisory at V0.
//   pub fn check(token_path: &Path, signing_material_path: &Path) -> Result<(), FileModeError>;
//
// ## mnemra_host.rs additions Task 17 must make
//
//   // Add to libs/mnemra-host/mnemra_host.rs:
//   pub mod signing;
//   pub mod startup;
//
//   // Add libs/mnemra-host/signing.rs (or signing/mod.rs):
//   pub mod verify;
//   pub mod root_material;
//
//   // Add libs/mnemra-host/startup.rs (or startup/mod.rs):
//   pub mod file_mode_check;
//
// ## Green-flip gate for Task 17
//
//   When Task 17 implements the above, `cargo test --manifest-path
//   libs/mnemra-host/Cargo.toml --test signing_chain` must pass.
//   The test outcome at green:
//
//     verify_api_is_synchronous                          PASS
//     malformed_signature_rejects_with_named_error       PASS
//     empty_signature_rejects_with_named_error           PASS
//     unknown_key_rejects_with_named_error               PASS
//     chain_break_rejects_with_named_error               PASS (or adjusted — see note)
//     core_true_attacker_signed_is_rejected_provenance_not_field  PASS
//     no_core_true_in_manifest_is_rejected               PASS
//     core_false_manifest_rejected_at_load               PASS
//     embedded_root_material_is_a_compile_time_constant  PASS
//     startup_file_mode_check_passes_for_mode_600        PASS
//     startup_check_fails_if_admin_token_is_world_readable  PASS
//     startup_check_fails_if_signing_material_is_world_readable PASS
//     startup_check_fails_if_both_files_are_world_readable  PASS
