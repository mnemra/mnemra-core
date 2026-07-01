//! Round-trip acceptance tests for the signing-ceremony producer.
//!
//! These drive the producer's public API (`sign_ceremony::build_signed_manifest`)
//! and check its output against the REAL mnemra-host consumers — not a
//! reimplemented verifier:
//!
//! - the **signature** layer: `signing::verify::verify_plugin` → `Ok(Core)`, and
//!   a one-byte body mutation → `Err`;
//! - the **integrity** layer: `startup::populate_verified_pool` (the content-hash
//!   gate that `verify_plugin` never inspects) over the REAL echo manifest + REAL
//!   echo `.wasm` → `Ok`. This is the only test that exercises the producer's
//!   `[component]` embedding against its actual consumer, closing both halves of
//!   the P-SecurityLayered chain (provenance + integrity).
//!
//! No hardcoded key material — every keypair is generated per-run.

use std::path::PathBuf;

use mnemra_host::signing::verify::{CoreStatus, verify_plugin};
use mnemra_host::startup::populate_verified_pool;
use sign_ceremony::{build_signed_manifest, generate_keypair, hex_encode};

const SIGNED_AT: &str = "2026-07-01T00:00:00Z";

/// A minimal `core = true` manifest carrying a PLACEHOLDER `[signature]` (so the
/// strip-then-resign path is exercised) and NO `[component]` yet — the shape the
/// real echo manifest has before the ceremony.
fn sample_manifest_with_placeholder_sig() -> Vec<u8> {
    br#"[plugin]
name = "sample-plugin"
version = "0.1.0"
schema_version = 1
core = true

[verbs]
exposed = ["sample.echo"]

[host_fns]
required = ["log.emit"]
optional = []

[signature]
algorithm = "ed25519"
public_key = "PLACEHOLDER"
sig_bytes = "PLACEHOLDER_SIG"
signed_at = "1970-01-01T00:00:00Z"
"#
    .to_vec()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Workspace root, resolved from this crate's manifest dir (`cmd/sign-ceremony`).
fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root from cmd/sign-ceremony")
        .to_path_buf()
}

/// The built echo component — the same path `populate_verified_pool` loads from,
/// so a hash declared over these bytes matches the gate's recompute. Asserts
/// existence with a helpful message (run `just plugin`).
fn echo_component_path() -> PathBuf {
    let path = workspace_root().join("target/wasm32-wasip2/release/mnemra_echo.wasm");
    assert!(
        path.exists(),
        "echo component not found at {} — run `just plugin` before these tests",
        path.display()
    );
    path
}

/// The committed real echo manifest (pre-ceremony: placeholder signature, no
/// `[component]`).
fn real_manifest_path() -> PathBuf {
    workspace_root().join("plugins/mnemra-echo/manifest.toml")
}

#[test]
fn produced_manifest_verifies_against_real_verify_plugin() {
    let key = generate_keypair().unwrap();
    let wasm = b"pretend-this-is-the-committed-echo-wasm";
    let produced = build_signed_manifest(
        &key,
        wasm,
        &sample_manifest_with_placeholder_sig(),
        SIGNED_AT,
    )
    .unwrap();

    let root = key.verifying_key().to_bytes();
    let result = verify_plugin(&produced, &root);
    assert!(
        matches!(result, Ok(CoreStatus::Core)),
        "producer output must verify against the REAL verify_plugin as Ok(Core); got {result:?}"
    );
}

#[test]
fn one_byte_body_mutation_fails_verification() {
    let key = generate_keypair().unwrap();
    let wasm = b"pretend-this-is-the-committed-echo-wasm";
    let produced = build_signed_manifest(
        &key,
        wasm,
        &sample_manifest_with_placeholder_sig(),
        SIGNED_AT,
    )
    .unwrap();
    let root = key.verifying_key().to_bytes();

    // Sanity: it verifies before mutation.
    assert!(matches!(
        verify_plugin(&produced, &root),
        Ok(CoreStatus::Core)
    ));

    // Flip one byte inside the SIGNED body (the plugin name value) — TOML stays
    // well-formed, but the signed bytes change, so the crypto check must fail.
    let mut mutated = produced.clone();
    let idx = find_bytes(&mutated, b"sample-plugin").expect("name present in signed body");
    mutated[idx] ^= 0x01;

    let result = verify_plugin(&mutated, &root);
    assert!(
        result.is_err(),
        "a one-byte body mutation must fail verification; got {result:?}"
    );
}

#[test]
fn embedded_hash_is_blake3_of_wasm_bytes() {
    let key = generate_keypair().unwrap();
    let wasm = b"exact-wasm-bytes-under-test";
    let produced = build_signed_manifest(
        &key,
        wasm,
        &sample_manifest_with_placeholder_sig(),
        SIGNED_AT,
    )
    .unwrap();

    let expected = blake3::hash(wasm).to_hex().to_string();
    let produced_str = String::from_utf8(produced).unwrap();
    assert!(
        produced_str.contains(&format!("hash = \"{expected}\"")),
        "the embedded [component].hash must be blake3 of the exact wasm bytes"
    );
    assert!(
        produced_str.contains("hash_alg = \"blake3\""),
        "the embedded [component] must declare hash_alg = blake3"
    );
}

#[test]
fn public_key_field_matches_verifying_key_interp_b() {
    let key = generate_keypair().unwrap();
    let produced = build_signed_manifest(
        &key,
        b"w",
        &sample_manifest_with_placeholder_sig(),
        SIGNED_AT,
    )
    .unwrap();
    let produced_str = String::from_utf8(produced).unwrap();
    let pubkey_hex = hex_encode(key.verifying_key().as_bytes());
    assert!(
        produced_str.contains(&format!("public_key = \"{pubkey_hex}\"")),
        "public_key field must be hex(verifying_key) so verify.rs Interpretation-B holds"
    );
}

#[test]
fn re_signing_is_idempotent_single_component() {
    let key = generate_keypair().unwrap();
    let wasm = b"same-wasm";

    let once = build_signed_manifest(
        &key,
        wasm,
        &sample_manifest_with_placeholder_sig(),
        SIGNED_AT,
    )
    .unwrap();
    // Re-sign the ALREADY-signed manifest (has both [component] and [signature])
    // — must still verify and carry exactly one of each.
    let twice = build_signed_manifest(&key, wasm, &once, SIGNED_AT).unwrap();

    let root = key.verifying_key().to_bytes();
    assert!(
        matches!(verify_plugin(&twice, &root), Ok(CoreStatus::Core)),
        "re-signing must still produce a verifying manifest"
    );
    let twice_str = String::from_utf8(twice).unwrap();
    assert_eq!(
        twice_str.matches("[component]").count(),
        1,
        "re-signing must not duplicate the [component] table"
    );
    assert_eq!(
        twice_str.matches("[signature]").count(),
        1,
        "re-signing must not duplicate the [signature] table"
    );
}

#[test]
fn non_core_manifest_fails_self_verify_shape() {
    // build_signed_manifest signs it (it does not judge core), but verify_plugin
    // rejects core=false — which is what cmd_sign's self-verify gate relies on.
    let key = generate_keypair().unwrap();
    let manifest = br#"[plugin]
name = "not-core"
version = "0.1.0"
schema_version = 1
core = false

[host_fns]
required = ["log.emit"]
optional = []
"#
    .to_vec();
    let produced = build_signed_manifest(&key, b"w", &manifest, SIGNED_AT).unwrap();
    let root = key.verifying_key().to_bytes();
    assert!(
        verify_plugin(&produced, &root).is_err(),
        "a core=false manifest must fail self-verification (cmd_sign refuses to write it)"
    );
}

/// Integrity layer: the producer's `[component]` embedding, over the REAL echo
/// manifest and REAL echo `.wasm`, is accepted by the real content-hash gate
/// (`populate_verified_pool`) — not just by the signature layer. This is the
/// half of the trust chain `verify_plugin` does not inspect.
#[test]
fn produced_manifest_passes_content_hash_integrity_gate() {
    let wasm_bytes = std::fs::read(echo_component_path()).expect("read echo wasm");
    let manifest_bytes = std::fs::read(real_manifest_path()).expect("read echo manifest");

    let key = generate_keypair().unwrap();
    let produced = build_signed_manifest(&key, &wasm_bytes, &manifest_bytes, SIGNED_AT).unwrap();

    let result = populate_verified_pool(&produced, &key.verifying_key().to_bytes());
    assert!(
        result.is_ok(),
        "producer output over the REAL manifest + REAL wasm must pass the content-hash \
         integrity gate (populate_verified_pool); got Err: {:?}",
        result.err().map(|e| e.to_string())
    );
}
