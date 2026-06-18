//! Happy-path output validation tests — GREEN phase counterweight (R-0003-f).
//!
//! # Purpose
//!
//! The RED tests in `manifest_load.rs` pin the fail-shut paths (oversized output,
//! schema mismatch). This file is the anti-"always-Err" counterweight: it verifies
//! that genuinely valid WIT-serialized output returns `Ok` from `validate_output`.
//!
//! # Verb → WIT output-type binding (V0)
//!
//! `echo.*` verbs map to the WIT `echo` interface's `string` return type.
//! The component model lowers `string` as raw UTF-8 bytes (the host adapter
//! resolves the ptr+len indirect before calling `validate_output`). This test
//! passes real UTF-8 string bytes — the exact type that `echo.create`, `echo.get`,
//! etc. return.
//!
//! # R-0003-f happy path
//!
//! `validate_output(verb, <real WIT-serialized valid output>) → Ok(())`.
//! This is the seal required by the Task 21 dispatch as a counterweight to the
//! fail-shut error paths. It runs in this file (not `manifest_load.rs`, which
//! is forbid-scoped).

use ed25519_dalek::{Signer, SigningKey};
use mnemra_host::plugin::runtime::{OutputError, PluginRuntime};
use rand::TryRng;

// ---------------------------------------------------------------------------
// Test keypair helper (matches manifest_load.rs convention)
// ---------------------------------------------------------------------------

fn generate_keypair() -> SigningKey {
    let mut seed = [0u8; 32];
    rand::rngs::SysRng
        .try_fill_bytes(&mut seed)
        .expect("SysRng fill failed");
    SigningKey::from_bytes(&seed)
}

// ---------------------------------------------------------------------------
// Manifest fixture helpers (duplicated here — manifest_load.rs is forbid-scoped)
// ---------------------------------------------------------------------------

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sign_manifest(signing_key: &SigningKey, manifest_bytes: &[u8]) -> Vec<u8> {
    signing_key.sign(manifest_bytes).to_bytes().to_vec()
}

fn manifest_bytes_unsigned(
    name: &str,
    version: &str,
    schema_version: u64,
    verbs: &[&str],
) -> Vec<u8> {
    let verbs_toml = if verbs.is_empty() {
        r#"exposed = []"#.to_owned()
    } else {
        let items: Vec<String> = verbs.iter().map(|v| format!("  \"{v}\"")).collect();
        format!("exposed = [\n{}\n]", items.join(",\n"))
    };

    format!(
        r#"[plugin]
name = "{name}"
version = "{version}"
schema_version = {schema_version}
core = true

[verbs]
{verbs_toml}

[host_fns]
required = []
optional = []
"#
    )
    .into_bytes()
}

fn signed_manifest_with_verbs(signing_key: &SigningKey, verbs: &[&str]) -> Vec<u8> {
    let vk = signing_key.verifying_key();
    let unsigned = manifest_bytes_unsigned("echo", "0.0.1", 1, verbs);
    let sig = sign_manifest(signing_key, &unsigned);
    let unsigned_str = String::from_utf8(unsigned).unwrap();
    let pubkey_hex = hex_encode(vk.as_bytes());
    let sig_hex = hex_encode(&sig);

    format!(
        r#"{unsigned_str}
[signature]
algorithm = "ed25519"
public_key = "{pubkey_hex}"
sig_bytes = "{sig_hex}"
signed_at = "2026-06-18T00:00:00Z"
"#
    )
    .into_bytes()
}

// ---------------------------------------------------------------------------
// Happy-path tests
// ---------------------------------------------------------------------------

/// Given a loaded PluginRuntime with echo.create verb declared,
/// When `validate_output` is called with valid UTF-8 bytes (a real WIT string),
/// Then the result is `Ok(())` — valid output passes the validation gate.
///
/// This is the R-0003-f happy path: `validate_output(verb, <real WIT-serialized
/// valid output>) → Ok`. It is the counterweight to the fail-shut error tests
/// in `manifest_load.rs` — proof that valid output is never rejected.
#[test]
fn valid_utf8_output_passes_validation_for_echo_verb() {
    // Given — a loaded runtime with echo.create verb
    let signing_key = generate_keypair();
    let manifest = signed_manifest_with_verbs(&signing_key, &["echo.create"]);
    let runtime = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes())
        .expect("manifest must load for happy-path test (R-0003-f)");

    // When — a real WIT string: `echo.create` returns a string; the component
    // model lowers `string` as UTF-8 bytes. "echo: hello" is valid output.
    let valid_output = b"echo: hello";
    let result = runtime.validate_output("echo.create", valid_output);

    // Then — valid UTF-8 string output must pass (R-0003-f happy path)
    assert!(
        result.is_ok(),
        "valid UTF-8 string output must pass validate_output for echo.create \
         (R-0003-f happy path); got: {result:?}"
    );
}

/// Given a loaded runtime, verify that valid output for multiple echo verbs passes.
///
/// Tests that the verb-namespace binding applies to all `echo.*` verbs, not
/// just `echo.create`.
#[test]
fn valid_utf8_output_passes_for_multiple_echo_verbs() {
    // Given
    let signing_key = generate_keypair();
    let manifest = signed_manifest_with_verbs(
        &signing_key,
        &[
            "echo.create",
            "echo.get",
            "echo.list",
            "echo.update",
            "echo.delete",
        ],
    );
    let runtime = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes())
        .expect("manifest must load");

    // When/Then — all echo.* verbs accept valid UTF-8 string output
    for verb in &[
        "echo.create",
        "echo.get",
        "echo.list",
        "echo.update",
        "echo.delete",
    ] {
        let result = runtime.validate_output(verb, b"artifact-id-abc123");
        assert!(
            result.is_ok(),
            "valid UTF-8 output must pass for verb '{verb}' (R-0003-f); got: {result:?}"
        );
    }
}

/// Given a loaded runtime, verify that empty output (empty string) is valid for
/// an echo verb.
///
/// An empty UTF-8 string is a legitimate WIT string return value.
#[test]
fn empty_string_output_passes_for_echo_verb() {
    // Given
    let signing_key = generate_keypair();
    let manifest = signed_manifest_with_verbs(&signing_key, &["echo.get"]);
    let runtime = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes())
        .expect("manifest must load");

    // When — empty slice is valid UTF-8 (empty string)
    let result = runtime.validate_output("echo.get", b"");

    // Then — empty string is a valid WIT string; must not be rejected (R-0003-f)
    assert!(
        result.is_ok(),
        "empty string output must pass validate_output for echo.get (R-0003-f); got: {result:?}"
    );
}

/// Confirm that a verb outside the echo.* namespace is rejected (fail-shut for
/// unbound verbs per R-0003-f).
///
/// Unbound verbs return `SchemaMismatch` — the fail-shut default prevents silent
/// validation bypass for verbs not yet mapped to a WIT output type.
#[test]
fn unknown_namespace_verb_fails_shut() {
    // Given
    let signing_key = generate_keypair();
    let manifest = signed_manifest_with_verbs(&signing_key, &["task.create"]);
    let runtime = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes())
        .expect("manifest must load");

    // When — a verb in an unbound namespace with valid UTF-8 bytes
    let result = runtime.validate_output("task.create", b"task-id-xyz");

    // Then — unbound verb must fail shut (R-0003-f: no known WIT type = reject)
    assert!(
        result.is_err(),
        "unbound verb 'task.create' must fail shut even with valid UTF-8 output (R-0003-f)"
    );
    assert!(
        matches!(result.unwrap_err(), OutputError::SchemaMismatch { .. }),
        "unbound verb must produce SchemaMismatch (R-0003-f fail-shut default)"
    );
}
