//! Component content-hash binding acceptance tests — RED phase (dispatch #1196, Task 1934 / T1).
//!
//! # Purpose
//!
//! Pins the R-0021 contract for `populate_verified_pool`: after the manifest
//! **signature** verifies (R-0005-a), the verified-load gate MUST enforce that the
//! component (`.wasm`) bytes it loads match the `[component].hash` declared in the
//! **signed** manifest body — the supply-chain integrity layer
//! (`P-SecurityLayered`: signature = provenance of the declaration; content-hash =
//! integrity of the artifact; each layer independently load-bearing).
//!
//! # Approach (locked by Puck — manifest-declared-hash, NO injection seam)
//!
//! The plan's Task-1 named a `#[cfg(test)]` component-artifact injection seam so a
//! test could feed *swapped* component bytes to the gate. **That seam is dropped.**
//! The adversarial lever is the **signed manifest**, which these tests already
//! control via the per-run test-key signing fixture. `populate_verified_pool` loads
//! the one real echo component at `echo_component_path()` (unchanged today); the
//! test controls the *declared* `[component].hash`:
//!
//! - **Positive:** declare `hash = BLAKE3(on-disk echo .wasm)` → expect `Ok`.
//! - **Swap/tamper:** declare a *different* valid hash → the gate recomputes BLAKE3
//!   over the loaded bytes and finds them ≠ the declared value → expect `Err` on the
//!   **hash-mismatch** path. "Loaded-bytes ≠ declared-hash" is reached by making the
//!   *declared* value wrong — same terminal branch, same observable, as a physical
//!   byte-swap. `populate_verified_pool`'s signature is therefore **unchanged**:
//!   `pub fn populate_verified_pool(manifest_toml: &[u8], root_material: &[u8])
//!   -> Result<Arc<PluginPool>, StartupError>`.
//!
//! # R-ID mapping
//!
//! | Test function                                       | R-ID(s)          | Scenario |
//! |-----------------------------------------------------|------------------|----------|
//! | correct_declared_hash_loads_ok                      | R-0021-e (pos)   | keystone |
//! | swapped_declared_hash_rejected_on_hash_path         | R-0021-e (swap)  | keystone |
//! | hash_mismatch_uses_distinct_error_variant           | R-0021-f         | keystone |
//! | absent_component_hash_rejected_fail_closed          | R-0021-c         | field-absent |
//! | unsigned_position_component_hash_rejected           | R-0021-c         | field-absent |
//! | weak_hash_algorithm_rejected                        | R-0021-a         | keystone |
//!
//! # RED-phase design (fails for the RIGHT reason against today's code)
//!
//! Today `populate_verified_pool` performs NO content-hash check: it verifies the
//! signature then loads+registers the component regardless of any `[component]`
//! field. So today:
//!
//! - the **positive** test loads `Ok` (the correct-hash control — it also PROVES the
//!   signed-slice byte-layout: the signature only verifies if `[component]` fell
//!   inside the signature-covered slice);
//! - every **adversarial** test also loads `Ok` (no binding logic), so each
//!   `assert!(result.is_err())` fails → RED. The RED signal is the *absent binding
//!   logic*, not a missing symbol (these tests name no not-yet-existing type) and
//!   not a signature failure (every adversarial manifest is **validly signed**) and
//!   not a component-load failure (`just plugin` builds the component first).
//!
//! # Discrimination (not a vacuous suite)
//!
//! - Every adversarial manifest is signed by the **test private key** and the
//!   matching **test public key** is passed as `root_material` — so the signature
//!   gate always passes, and any rejection is provably the **hash gate**, not the
//!   signature gate (each error test also asserts the failure is NOT the
//!   signature path).
//! - The **swap** and **algorithm** tests differ from the positive by exactly one
//!   thing (the declared hash value / the declared `hash_alg`), so an `Ok`→`Err`
//!   delta is attributable solely to that field.
//! - The **algorithm** tests declare the *correct* BLAKE3 hash with a weak
//!   `hash_alg` — a GREEN gate that ignored `hash_alg` and only compared digests
//!   would return `Ok` (correct hash) and be caught here; only a gate that
//!   validates the algorithm set rejects.
//! - The **unsigned-position** test declares the *correct* hash but only outside the
//!   signed slice — a GREEN gate that wrongly read the unsigned copy would return
//!   `Ok` and be caught here.
//!
//! # verify: []
//!
//! Intentional for a RED dispatch. The adversarial tests cannot pass until the
//! GREEN phase (T2 / #1820) adds the binding gate. The GREEN phase adds the recipe.
//!
//! # No hardcoded key material
//!
//! All Ed25519 keypairs are generated per-run via `generate_keypair()`. No key
//! bytes, seed bytes, or DER blobs appear literally in this file.
//!
//! # Signing fixture (inlined — private fns cannot cross integration-test crates)
//!
//! The per-run keypair + canonical-body signing helpers mirror the ones in
//! `tests/signing_chain.rs` / `tests/startup_population.rs` (private fns are not
//! importable across integration-test crates, so they are inlined verbatim). They
//! are EXTENDED here to compute-and-embed a `[component]` section in the signed body
//! (before the `\n[signature]` marker) and to place it in an unsigned position for
//! the negative case.
//!
//! # Handoff for the T2 GREEN implementer (#1820)
//!
//! - **Fixture location:** this file. It signs adversarial manifests with a per-run
//!   test key and passes the matching public key as `root_material` (the existing
//!   `populate_verified_pool(manifest, root_material)` seam — no ceremony key needed).
//! - **`[component]` schema assumed** (matches the spec Data Model + P-0003 amendment):
//!   a `[component]` table in the **signed** body carrying `hash_alg` (V0 = `"blake3"`)
//!   and `hash` (lowercase-hex of the digest). Absence of `hash` is fail-closed.
//! - **Correct-hash encoding to match:** `blake3::hash(&bytes).to_hex().to_string()`
//!   (lowercase hex of the 32-byte one-shot BLAKE3 digest) over the bytes the gate
//!   loads. The gate MUST load from `echo_component_path()`
//!   (`target/wasm32-wasip2/release/mnemra_echo.wasm`) so the recompute matches the
//!   declared value; if T5 later moves the production load to a committed signed
//!   artifact, keep this test's load path aligned with the gate's.
//! - **Parse only from the signed slice:** read `[component].hash_alg`/`hash` from the
//!   `extract_signed_payload` slice (`signing::verify::extract_signed_payload`,
//!   `verify.rs:337-344`) — a `[component]` table in the unsigned region must NOT
//!   satisfy presence (the `unsigned_position` test enforces this).
//! - **R-0021-f `Display` contract:** a hash mismatch (and a fail-closed absence)
//!   must return a **distinct** error whose `Display` carries a hash-mismatch/tamper
//!   signal (these tests accept any of: `hash`, `mismatch`, `tamper`, `integrity`,
//!   `digest`), NAMES the plugin (contains `echo`), and does NOT reuse the benign
//!   `ComponentLoad` message ("failed to load echo component") nor leak `wasmtime` /
//!   `/target/` internals. `StartupError` is `#[non_exhaustive]`, so a new
//!   `ComponentHashMismatch`-class variant is additive.

// ---------------------------------------------------------------------------
// Imports
// ---------------------------------------------------------------------------

use std::path::PathBuf;

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use mnemra_host::startup::populate_verified_pool;
use rand::TryRng;

// The plugin name the manifest fixtures declare; the R-0005-b / R-0021-e error
// "names the plugin" contract is asserted against this (case-insensitive `echo`).
const PLUGIN_NAME: &str = "mnemra-echo";

// ---------------------------------------------------------------------------
// Signing fixture helpers (inlined from tests/signing_chain.rs +
// tests/startup_population.rs — private fns cannot be imported across
// integration-test crates; EXTENDED to embed a signed-body [component] section)
// ---------------------------------------------------------------------------

/// Generate a fresh Ed25519 signing keypair using the OS CSPRNG.
///
/// No hardcoded key material — called fresh in every test that needs a keypair.
/// rand 0.10: `OsRng` was renamed to `SysRng` (re-exported from getrandom).
fn generate_keypair() -> SigningKey {
    let mut seed = [0u8; 32];
    rand::rngs::SysRng
        .try_fill_bytes(&mut seed)
        .expect("SysRng fill failed");
    SigningKey::from_bytes(&seed)
}

/// Build the canonical manifest body (`core = true`), WITHOUT the `[component]`
/// section and WITHOUT the `[signature]` section.
///
/// This is the base of the signature-covered slice; callers append a
/// `[component]` section (via [`component_section`]) before signing to place the
/// content-hash field inside the signed body. Ends with a trailing newline so the
/// `\n[signature]` boundary matches the `startup_population.rs` fixture layout.
fn manifest_body_base(name: &str, version: &str, core: bool) -> String {
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
}

/// A `[component]` section as TOML text (leading blank line + trailing newline),
/// to be concatenated onto the canonical body so it falls inside the signed slice.
fn component_section(hash_alg: &str, hash_hex: &str) -> String {
    format!("\n[component]\nhash_alg = \"{hash_alg}\"\nhash = \"{hash_hex}\"\n")
}

/// Sign the given canonical bytes with `signing_key`; return the detached
/// Ed25519 signature (64 bytes).
fn sign_body(signing_key: &SigningKey, body: &[u8]) -> Vec<u8> {
    signing_key.sign(body).to_bytes().to_vec()
}

/// Wrap a signed body with the `[signature]` table, producing the full manifest.
///
/// Byte-layout note (load-bearing): the raw string places exactly one `\n` between
/// `{signed_body}` and `[signature]`. `signed_body` itself already ends with a
/// `\n`, so the boundary is `...\n\n[signature]`. The verifier's
/// `extract_signed_payload` returns the bytes before `\n[signature]` — i.e. exactly
/// `signed_body`, the bytes that were signed. Mirrors `manifest_with_signature`
/// in `startup_population.rs` (known-good).
fn wrap_signed(signed_body: &str, verifying_key: &VerifyingKey, sig_bytes: &[u8]) -> Vec<u8> {
    let pubkey_hex = hex_encode(verifying_key.as_bytes());
    let sig_hex = hex_encode(sig_bytes);
    let full = format!(
        r#"{signed_body}
[signature]
algorithm = "ed25519"
public_key = "{pubkey_hex}"
sig_bytes = "{sig_hex}"
signed_at = "2026-06-13T00:00:00Z"
"#
    );
    full.into_bytes()
}

/// Minimal lowercase-hex encoding for key/signature material in manifest fixtures.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Locate the built `mnemra-echo` component (`wasm32-wasip2`, release), produced
/// by `just plugin`. Resolved relative to the workspace target dir — the same path
/// `populate_verified_pool` loads from (`pool_population.rs::echo_component_path`),
/// so a hash declared over these bytes matches the gate's recompute.
///
/// Asserts existence with a helpful message so a missing component fails clearly
/// (run `just plugin`) rather than as a confusing hash mismatch.
fn echo_component_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root from libs/mnemra-host");
    let path = root.join("target/wasm32-wasip2/release/mnemra_echo.wasm");
    assert!(
        path.exists(),
        "echo component not found at {} — run `just plugin` (cargo build --release \
         -p mnemra-echo --target wasm32-wasip2) before these tests",
        path.display()
    );
    path
}

/// Lowercase-hex BLAKE3 digest of the on-disk echo component — the value a correct
/// manifest declares. The GREEN gate must recompute this exact encoding over the
/// bytes it loads (`blake3::hash(&bytes).to_hex()`).
fn echo_blake3_hex() -> String {
    let bytes = std::fs::read(echo_component_path()).expect("read echo component bytes");
    blake3::hash(&bytes).to_hex().to_string()
}

// ---------------------------------------------------------------------------
// Shared assertion helper for the adversarial (reject) cases
// ---------------------------------------------------------------------------

/// Assert a `populate_verified_pool` result was rejected by the **content-hash
/// gate** (not the signature gate, not a benign component-load failure). Returns
/// the error's `Display` string for further scenario-specific assertions.
///
/// Because every adversarial manifest here is validly signed with the test key,
/// an `Err` on the signature path would indicate a wrong-reason failure — this
/// helper rules that out, keeping the RED signal honest.
fn assert_rejected_by_hash_gate(
    result: Result<std::sync::Arc<mnemra_host::plugin::pool::PluginPool>, impl std::fmt::Display>,
    red_msg: &str,
) -> String {
    let err = match result {
        Ok(_) => panic!("{red_msg}"),
        Err(e) => e.to_string(),
    };
    let lower = err.to_lowercase();
    assert!(
        !lower.contains("signature verification failed"),
        "rejection must be on the content-hash path, not the signature path (the manifest is \
         validly signed by the test key); got signature-path error: {err}"
    );
    err
}

// ---------------------------------------------------------------------------
// Scenario 1 (positive) — correct declared hash loads Ok. R-0021-e.
// Also the byte-layout proof: Ok today ⇒ [component] fell inside the signed slice.
// ---------------------------------------------------------------------------

/// R-0021-e (positive keystone) — a validly-signed manifest declaring the CORRECT
/// `[component].hash` loads.
///
/// GIVEN a manifest whose signed body carries `[component]` with
///       `hash_alg = "blake3"` and `hash = BLAKE3(on-disk echo .wasm)`, signed by
///       a per-run test key
/// WHEN  `populate_verified_pool(&manifest, &test_public_key)` is called
/// THEN  it returns `Ok` — the signature verifies AND (post-GREEN) the recomputed
///       hash equals the declared hash.
///
/// This test PASSES today (no hash check ⇒ Ok) and after GREEN (correct hash ⇒ Ok);
/// it is the control that forbids a GREEN "reject everything" stub. Its `Ok` today
/// also PROVES the signed-slice byte-layout: if `[component]` fell outside the
/// signature-covered slice the signature would not verify and this would `Err`.
#[test]
fn correct_declared_hash_loads_ok() {
    let signing_key = generate_keypair();
    let verifying_key = signing_key.verifying_key();

    let signed_body = manifest_body_base(PLUGIN_NAME, "0.1.0", true)
        + &component_section("blake3", &echo_blake3_hex());
    let sig = sign_body(&signing_key, signed_body.as_bytes());
    let manifest = wrap_signed(&signed_body, &verifying_key, &sig);

    let result = populate_verified_pool(&manifest, &verifying_key.to_bytes());

    assert!(
        result.is_ok(),
        "R-0021-e: a validly-signed manifest declaring the correct [component].hash must load Ok; \
         got Err: {:?}. (If this fails TODAY it is a byte-layout error — [component] fell outside \
         the signed slice — or the parser rejects [component]; HALT and report, do not commit.)",
        result.err().map(|e| e.to_string())
    );
}

// ---------------------------------------------------------------------------
// Scenario 1 (swap) — wrong declared hash rejected on the hash path. R-0021-e.
// ---------------------------------------------------------------------------

/// R-0021-e (swap keystone) — a validly-signed manifest declaring a DIFFERENT
/// hash is rejected on the hash-mismatch path; no pool is returned.
///
/// GIVEN a manifest whose signed body carries `[component]` with a valid but
///       WRONG `hash` (the digest a *different* component would have), signed by a
///       per-run test key (so the signature verifies)
/// WHEN  `populate_verified_pool(&manifest, &test_public_key)` is called
/// THEN  it returns `Err` on the hash-mismatch path (NOT the signature path);
///   AND no `PluginPool` is returned to the caller (the `Err` IS that observable);
///   AND the error names the plugin and leaks no filesystem/wasmtime internals.
///
/// TODAY: no hash check ⇒ the wrong-hash manifest loads `Ok` ⇒ `is_err()` fails ⇒
/// RED (right reason: binding logic absent).
#[test]
fn swapped_declared_hash_rejected_on_hash_path() {
    let signing_key = generate_keypair();
    let verifying_key = signing_key.verifying_key();

    // A valid-but-wrong hash: the BLAKE3 of some OTHER bytes (a "swapped" component).
    let wrong_hash = blake3::hash(b"swapped-component-bytes-not-the-real-echo-wasm")
        .to_hex()
        .to_string();
    let signed_body =
        manifest_body_base(PLUGIN_NAME, "0.1.0", true) + &component_section("blake3", &wrong_hash);
    let sig = sign_body(&signing_key, signed_body.as_bytes());
    let manifest = wrap_signed(&signed_body, &verifying_key, &sig);

    let result = populate_verified_pool(&manifest, &verifying_key.to_bytes());

    let err = assert_rejected_by_hash_gate(
        result,
        "R-0021-e: a validly-signed manifest declaring a WRONG [component].hash must be rejected \
         on the hash-mismatch path with no pool returned; got Ok (no content-hash binding today = RED)",
    );
    let lower = err.to_lowercase();

    // Names the plugin (R-0005-b / R-0021-e).
    assert!(
        lower.contains("echo"),
        "R-0021-e: the hash-mismatch error must name the plugin (contains 'echo'); got: {err}"
    );
    // Leaks no filesystem / wasmtime internals (R-0021-e).
    assert!(
        !lower.contains("wasmtime") && !err.contains("/target/"),
        "R-0021-e: the hash-mismatch error must not leak wasmtime / filesystem internals; got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 2 — distinct, tamper-legible error variant. R-0021-f.
// ---------------------------------------------------------------------------

/// R-0021-f — a content-hash mismatch surfaces a DISTINCT, tamper-legible error,
/// not the benign `ComponentLoad`.
///
/// GIVEN the same validly-signed, wrong-hash manifest as the swap keystone
/// WHEN  `populate_verified_pool` is called
/// THEN  the error's `Display` carries a hash-mismatch/tamper signal AND is
///       distinct from the benign `ComponentLoad` message ("failed to load echo
///       component").
///
/// Asserts on the OBSERVABLE `Display` (not a not-yet-existing Rust variant name),
/// so the suite compiles today. The GREEN implementer adds a
/// `ComponentHashMismatch`-class variant to the `#[non_exhaustive]` enum.
#[test]
fn hash_mismatch_uses_distinct_error_variant() {
    let signing_key = generate_keypair();
    let verifying_key = signing_key.verifying_key();

    let wrong_hash = blake3::hash(b"a-different-component-entirely")
        .to_hex()
        .to_string();
    let signed_body =
        manifest_body_base(PLUGIN_NAME, "0.1.0", true) + &component_section("blake3", &wrong_hash);
    let sig = sign_body(&signing_key, signed_body.as_bytes());
    let manifest = wrap_signed(&signed_body, &verifying_key, &sig);

    let result = populate_verified_pool(&manifest, &verifying_key.to_bytes());

    let err = assert_rejected_by_hash_gate(
        result,
        "R-0021-f: a content-hash mismatch must be rejected with a distinct error; got Ok \
         (no content-hash binding today = RED)",
    );
    let lower = err.to_lowercase();

    // Carries a hash-mismatch / tamper signal (lenient word-set — the exact wording
    // is GREEN's to choose; the tamper signal must be legible).
    let signal_words = ["hash", "mismatch", "tamper", "integrity", "digest"];
    assert!(
        signal_words.iter().any(|w| lower.contains(w)),
        "R-0021-f: the mismatch error's Display must carry a hash-mismatch/tamper signal \
         (one of {signal_words:?}); got: {err}"
    );
    // Distinct from the benign ComponentLoad variant (must NOT reuse it — R-0021-f).
    assert!(
        !lower.contains("failed to load echo component"),
        "R-0021-f: the mismatch error must be DISTINCT from the benign ComponentLoad error \
         ('failed to load echo component'); got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 3 — field absence is fail-closed (not skip-on-absent). R-0021-c.
// ---------------------------------------------------------------------------

/// R-0021-c — a signed body that omits `[component].hash` is REJECTED fail-closed
/// (a missing hash is a load rejection, not a "no hash declared → skip the check").
///
/// Covers BOTH forms of "the `.hash` field is absent from the signed body", since
/// R-0021-c is about the *field* being present, not merely the table:
///   (a) no `[component]` table at all (the pre-R-0021 manifest format);
///   (b) a `[component]` table present but with NO `hash` key — pins a GREEN gate
///       that checks table-presence but forgets to require the field.
///
/// GIVEN a validly-signed manifest whose signed body omits `[component].hash` (in
///       either form above)
/// WHEN  `populate_verified_pool` is called
/// THEN  it returns `Err` (fail-closed) with a content-hash error — NOT `Ok`.
///
/// TODAY: no presence check ⇒ loads `Ok` ⇒ RED. (Because `schema_version` stays 1,
/// the version number cannot signal presence; GREEN enforces presence directly.)
#[test]
fn absent_component_hash_rejected_fail_closed() {
    let base = manifest_body_base(PLUGIN_NAME, "0.1.0", true);
    let cases = [
        ("no [component] table", base.clone()),
        (
            "[component] present, hash key missing",
            base.clone() + "\n[component]\nhash_alg = \"blake3\"\n",
        ),
    ];

    for (label, signed_body) in cases {
        let signing_key = generate_keypair();
        let verifying_key = signing_key.verifying_key();
        let sig = sign_body(&signing_key, signed_body.as_bytes());
        let manifest = wrap_signed(&signed_body, &verifying_key, &sig);

        let result = populate_verified_pool(&manifest, &verifying_key.to_bytes());

        let err = assert_rejected_by_hash_gate(
            result,
            &format!(
                "R-0021-c ({label}): a signed body omitting [component].hash must be rejected \
                 fail-closed; got Ok (absence treated as skip-the-check = RED)"
            ),
        );
        let lower = err.to_lowercase();
        let signal_words = [
            "hash",
            "mismatch",
            "tamper",
            "integrity",
            "digest",
            "component",
        ];
        assert!(
            signal_words.iter().any(|w| lower.contains(w)),
            "R-0021-c ({label}): the fail-closed absence error must reference the missing \
             content-hash (one of {signal_words:?}); got: {err}"
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario 4 — hash present only in an unsigned position is rejected. R-0021-c.
// ---------------------------------------------------------------------------

/// R-0021-c (complete-mediation) — `[component].hash` present ONLY in an unsigned
/// position (after the `\n[signature]` marker) does NOT satisfy the field-presence
/// requirement; the manifest is rejected.
///
/// GIVEN a validly-signed manifest whose signed body carries NO `[component]`, but
///       a `[component]` table with the CORRECT hash is appended AFTER the
///       `[signature]` table (outside the signature-covered slice)
/// WHEN  `populate_verified_pool` is called
/// THEN  it returns `Err` — the enforced value is read only from the signed slice,
///       so the unsigned-region copy is not honored (even though the hash is
///       *correct*, its position is wrong).
///
/// The correct hash here is the discriminator: a GREEN gate that wrongly read the
/// unsigned copy would return `Ok` (the hash IS correct) and be caught. TODAY: no
/// check ⇒ loads `Ok` ⇒ RED.
#[test]
fn unsigned_position_component_hash_rejected() {
    let signing_key = generate_keypair();
    let verifying_key = signing_key.verifying_key();

    // Sign a body WITHOUT [component]; then append [component] with the CORRECT
    // hash in the UNSIGNED region (after [signature]). Single [component] table —
    // no duplicate (a duplicate table would be a TOML parse error, a wrong reason).
    let signed_body = manifest_body_base(PLUGIN_NAME, "0.1.0", true);
    let sig = sign_body(&signing_key, signed_body.as_bytes());
    let mut manifest = wrap_signed(&signed_body, &verifying_key, &sig);
    manifest.extend_from_slice(component_section("blake3", &echo_blake3_hex()).as_bytes());

    let result = populate_verified_pool(&manifest, &verifying_key.to_bytes());

    assert_rejected_by_hash_gate(
        result,
        "R-0021-c: a [component].hash present only in an unsigned position (after [signature]) \
         must NOT satisfy presence — reject; got Ok (unsigned copy honored, or no check = RED)",
    );
}

// ---------------------------------------------------------------------------
// Scenario 5 — weak hash algorithm rejected at load. R-0021-a.
// ---------------------------------------------------------------------------

/// R-0021-a — a `hash_alg` outside the strong set `{blake3, sha256, sha384,
/// sha512}` (here `md5` and `sha1`) is REJECTED at load; `blake3` is accepted
/// (the accept side is `correct_declared_hash_loads_ok`).
///
/// GIVEN validly-signed manifests declaring `hash_alg = "md5"` / `"sha1"` with the
///       CORRECT BLAKE3 hash value in the signed body
/// WHEN  `populate_verified_pool` is called
/// THEN  each returns `Err` — rejected structurally on the algorithm, not on the
///       digest.
///
/// The correct hash value is the discriminator (advisor): a GREEN gate that ignored
/// `hash_alg` and only compared digests would return `Ok` (the digest IS correct)
/// and be caught here; only a gate that validates the algorithm set rejects. TODAY:
/// no check ⇒ both load `Ok` ⇒ RED.
#[test]
fn weak_hash_algorithm_rejected() {
    let correct_hash = echo_blake3_hex();

    for weak_alg in ["md5", "sha1"] {
        let signing_key = generate_keypair();
        let verifying_key = signing_key.verifying_key();

        let signed_body = manifest_body_base(PLUGIN_NAME, "0.1.0", true)
            + &component_section(weak_alg, &correct_hash);
        let sig = sign_body(&signing_key, signed_body.as_bytes());
        let manifest = wrap_signed(&signed_body, &verifying_key, &sig);

        let result = populate_verified_pool(&manifest, &verifying_key.to_bytes());

        let err = assert_rejected_by_hash_gate(
            result,
            &format!(
                "R-0021-a: hash_alg = \"{weak_alg}\" (outside the strong set) must be rejected at \
                 load even with a correct hash value; got Ok (no algorithm check = RED)"
            ),
        );
        // The rejection must be a deliberate policy rejection, not the benign
        // ComponentLoad path (the component IS present).
        assert!(
            !err.to_lowercase().contains("failed to load echo component"),
            "R-0021-a: the weak-algorithm rejection must be distinct from the benign ComponentLoad \
             error; got: {err}"
        );
    }
}
