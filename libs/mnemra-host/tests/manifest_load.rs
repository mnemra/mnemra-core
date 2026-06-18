//! Plugin manifest load pipeline acceptance tests — RED phase (Task 20 of TDD pair 20/21).
//!
//! # Purpose
//!
//! These tests pin the plugin-manifest load pipeline that Task 21 must implement.
//! Every test calls `mnemra_host::plugin::*` which does NOT exist yet — the
//! compile-fail against the missing `plugin` module IS the correct red signal.
//!
//! Each Rust integration-test file is its own compilation unit. `manifest_load.rs`
//! failing to compile does NOT break the other passing test binaries
//! (`signing_chain`, `admin_token`, etc.).
//!
//! # RED-phase deviation: compile-fail as red signal
//!
//! The runtime module (`mnemra_host::plugin`) does not exist until Task 21.
//! Every test calls `PluginRuntime::load(...)` which resolves only after GREEN.
//! The compile error names the missing surface:
//!   `error[E0433]: failed to resolve: could not find 'plugin' in 'mnemra_host'`
//! Task 21 resolving that error IS the green-flip condition.
//!
//! # verify: [] rationale
//!
//! `verify: []` is correct by design. The test binary cannot be linked
//! (missing module) until Task 21 lands. Recipes do not exist for this test yet.
//! Task 21 populates `verify`.
//!
//! # Fail-shut ordering invariant
//!
//! The load pipeline is fail-shut: `verify_plugin` (signature verification) runs
//! BEFORE allowlist-compile / schema_version-branch / capability-check /
//! output-validation. Therefore EVERY fixture manifest used to probe those
//! downstream behaviors MUST be a validly signed manifest — sign in-memory with a
//! freshly generated keypair and set `public_key = hex(that key)` (Interpretation B,
//! locked 2026-06-13 in `signing/verify.rs`). Otherwise the test dies at
//! `FingerprintMismatch`/`VerificationFailed` and never reaches the logic it claims
//! to assert (a green-looking false test).
//!
//! # Proposed runtime API seam
//!
//! These tests call the not-yet-existing `mnemra_host::plugin` module and propose
//! its public API by calling it. Task 21 (Forge GREEN) implements to match.
//! Puck re-reads the committed seam and may amend before GREEN.
//!
//! Hard constraints on the proposed surface:
//!   - `PluginRuntime::load(manifest_toml: &[u8], root_material: &[u8])` — injectable
//!     `root_material` so production passes `signing::root_material::ROOT` and tests
//!     pass per-run generated key bytes, both without friction.
//!   - `load` is synchronous (R-0005-a: no async at the verify/load seam).
//!   - `LoadError` is a structured enum — schema_version errors carry observed and
//!     supported values; verification errors wrap `SigningError`.
//!
//! Proposed public API (Given/When/Then comments in each test elaborate):
//!
//! ```rust,ignore
//! // mnemra_host::plugin::runtime
//!
//! pub struct PluginRuntime { /* opaque */ }
//!
//! pub enum LoadError {
//!     VerificationFailed(mnemra_host::signing::verify::SigningError),
//!     UnknownSchemaVersion { found: u64, supported: &'static [u64] },
//!     MalformedManifest(String),
//! }
//!
//! impl PluginRuntime {
//!     /// Load a plugin from its signed manifest bytes.
//!     ///
//!     /// Verification runs BEFORE any other pipeline step. If `verify_plugin`
//!     /// returns Err, load returns `LoadError::VerificationFailed` and halts.
//!     pub fn load(manifest_toml: &[u8], root_material: &[u8]) -> Result<Self, LoadError>;
//!
//!     /// Query the per-instance allowlist compiled from `[host_fns]` at load time.
//!     ///
//!     /// Returns true iff `fn_name` appears in `required` or `optional`.
//!     /// Used to enforce the WIT boundary: undeclared calls are rejected before
//!     /// reaching the host-fn body (R-0003-b).
//!     pub fn is_host_fn_allowed(&self, fn_name: &str) -> bool;
//!
//!     /// Query the capability list compiled from `[verbs].exposed` at load time.
//!     ///
//!     /// Returns true iff `verb` appears in `exposed`. The runtime checks this
//!     /// before dispatching to the plugin (R-0010-d).
//!     pub fn is_verb_allowed(&self, verb: &str) -> bool;
//!
//!     /// The `schema_version` field extracted from the loaded manifest.
//!     pub fn schema_version(&self) -> u64;
//!
//!     /// Validate a plugin's output bytes against the WIT-declared schema for `verb`.
//!     ///
//!     /// R-0003-f: fails shut on schema mismatch (returns Err, never truncates).
//!     /// Per-field size caps are enforced here.
//!     /// Called at dispatch time with the plugin's raw output before returning to
//!     /// the caller.
//!     pub fn validate_output(&self, verb: &str, output_bytes: &[u8]) -> Result<(), OutputError>;
//! }
//!
//! pub enum OutputError {
//!     SchemaMismatch { verb: String, detail: String },
//!     FieldSizeCap { field: String, max_bytes: usize, actual_bytes: usize },
//! }
//! ```

// ---------------------------------------------------------------------------
// Imports — the missing Task-21 module is the red signal.
// ---------------------------------------------------------------------------

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use mnemra_host::plugin::runtime::{LoadError, OutputError, PluginRuntime};
use rand::TryRng;

// ---------------------------------------------------------------------------
// Test keypair generation — local copy (avoids `pub` pollution in common/mod.rs)
// ---------------------------------------------------------------------------

/// Generate a fresh Ed25519 signing keypair using the OS CSPRNG.
///
/// Per-run generated — no hardcoded key material appears in this file.
fn generate_keypair() -> SigningKey {
    let mut seed = [0u8; 32];
    rand::rngs::SysRng
        .try_fill_bytes(&mut seed)
        .expect("SysRng fill failed");
    SigningKey::from_bytes(&seed)
}

// ---------------------------------------------------------------------------
// Manifest fixture helpers — local copies, parameterized for the new sections
// ---------------------------------------------------------------------------

/// Hex-encode `bytes` as a lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Sign manifest bytes and return the detached 64-byte signature.
fn sign_manifest(signing_key: &SigningKey, manifest_bytes: &[u8]) -> Vec<u8> {
    signing_key.sign(manifest_bytes).to_bytes().to_vec()
}

/// Build the unsigned manifest body for schema_version `sv`, with the given
/// verbs and host_fns.
///
/// This is the "signed payload" — the exact bytes the build pipeline signs.
/// `manifest_with_signature` appends `\n[signature]\n...` to these bytes;
/// `signing/verify.rs extract_signed_payload` recovers them by slicing at
/// `\n[signature]`. Sign THESE bytes; include the result in `sig_bytes`.
fn manifest_bytes_unsigned_ext(
    name: &str,
    version: &str,
    schema_version: u64,
    verbs: &[&str],
    required_fns: &[&str],
    optional_fns: &[&str],
) -> Vec<u8> {
    let verbs_toml = if verbs.is_empty() {
        r#"exposed = []"#.to_owned()
    } else {
        let items: Vec<String> = verbs.iter().map(|v| format!("  \"{v}\"")).collect();
        format!("exposed = [\n{}\n]", items.join(",\n"))
    };

    let req_toml = if required_fns.is_empty() {
        r#"required = []"#.to_owned()
    } else {
        let items: Vec<String> = required_fns.iter().map(|f| format!("  \"{f}\"")).collect();
        format!("required = [\n{}\n]", items.join(",\n"))
    };

    let opt_toml = if optional_fns.is_empty() {
        r#"optional = []"#.to_owned()
    } else {
        let items: Vec<String> = optional_fns.iter().map(|f| format!("  \"{f}\"")).collect();
        format!("optional = [\n{}\n]", items.join(",\n"))
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
{req_toml}
{opt_toml}
"#
    )
    .into_bytes()
}

/// Build a full manifest including the `[signature]` section.
///
/// The `public_key` field is set to `hex(verifying_key)` — Interpretation B
/// cross-check requires this to equal `hex(root_material)` at the call site.
/// In every fixture that passes verification, `verifying_key` IS the root key.
fn manifest_with_signature_ext(
    name: &str,
    version: &str,
    schema_version: u64,
    verbs: &[&str],
    required_fns: &[&str],
    optional_fns: &[&str],
    verifying_key: &VerifyingKey,
    sig_bytes: &[u8],
) -> Vec<u8> {
    let unsigned = manifest_bytes_unsigned_ext(
        name,
        version,
        schema_version,
        verbs,
        required_fns,
        optional_fns,
    );
    let unsigned_str = String::from_utf8(unsigned).unwrap();

    let pubkey_hex = hex_encode(verifying_key.as_bytes());
    let sig_hex = hex_encode(sig_bytes);

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

/// Build a validly signed manifest with standard echo plugin shape.
///
/// Use this to get a manifest that PASSES verification so tests can probe
/// downstream behavior (allowlist, schema_version, capability check, output).
fn signed_manifest_standard(
    signing_key: &SigningKey,
    schema_version: u64,
    verbs: &[&str],
    required_fns: &[&str],
    optional_fns: &[&str],
) -> Vec<u8> {
    let vk = signing_key.verifying_key();
    let unsigned = manifest_bytes_unsigned_ext(
        "echo",
        "0.0.1",
        schema_version,
        verbs,
        required_fns,
        optional_fns,
    );
    let sig = sign_manifest(signing_key, &unsigned);
    manifest_with_signature_ext(
        "echo",
        "0.0.1",
        schema_version,
        verbs,
        required_fns,
        optional_fns,
        &vk,
        &sig,
    )
}

// ===========================================================================
// R-0003-b — Host-fn allowlist compiled from manifest before instance creation
// ===========================================================================

/// Given a signed manifest declaring `required = ["artifact.create", "log.emit"]`
/// and `optional = ["metrics.record"]`,
/// When the manifest is loaded into a PluginRuntime,
/// Then `is_host_fn_allowed` returns true for each declared fn and false for any
/// undeclared fn — the allowlist was compiled at load time, before any instance is
/// created (R-0003-b).
///
/// Red: `mnemra_host::plugin` does not exist — compile fail.
/// Green: `PluginRuntime::load` returns Ok; `is_host_fn_allowed` reflects the manifest.
#[test]
fn allowlist_compiled_from_manifest_before_instance_creation() {
    // Given
    let signing_key = generate_keypair();
    let required = &["artifact.create", "log.emit"];
    let optional = &["metrics.record"];
    let manifest = signed_manifest_standard(&signing_key, 1, &["echo.create"], required, optional);

    // When
    let runtime = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes()).expect(
        "load must succeed for a validly-signed core=true schema_version=1 manifest (R-0003-b)",
    );

    // Then — declared required fns are allowed
    assert!(
        runtime.is_host_fn_allowed("artifact.create"),
        "artifact.create is in required[] — must be allowed (R-0003-b)"
    );
    assert!(
        runtime.is_host_fn_allowed("log.emit"),
        "log.emit is in required[] — must be allowed (R-0003-b)"
    );
    // Then — declared optional fn is allowed
    assert!(
        runtime.is_host_fn_allowed("metrics.record"),
        "metrics.record is in optional[] — must be allowed (R-0003-b)"
    );
    // Then — undeclared fn is rejected (WIT boundary enforcement requires this)
    assert!(
        !runtime.is_host_fn_allowed("artifact.delete"),
        "artifact.delete NOT in required[] or optional[] — must be denied (R-0003-b)"
    );
    assert!(
        !runtime.is_host_fn_allowed("event.emit"),
        "event.emit NOT declared — must be denied (R-0003-b)"
    );
}

/// Given a manifest with an empty `required = []` and `optional = []`,
/// When the manifest is loaded,
/// Then NO host-fn is allowed — the allowlist is empty.
///
/// This pins the boundary case: an empty allowlist is a valid state and is
/// compiled faithfully from the manifest at load time.
///
/// Red: compile fail (missing plugin module).
/// Green: `is_host_fn_allowed` returns false for any fn name.
#[test]
fn empty_host_fn_declaration_produces_empty_allowlist() {
    // Given
    let signing_key = generate_keypair();
    let manifest = signed_manifest_standard(&signing_key, 1, &["echo.get"], &[], &[]);

    // When
    let runtime = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes())
        .expect("empty allowlist manifest must load (R-0003-b)");

    // Then
    assert!(
        !runtime.is_host_fn_allowed("artifact.create"),
        "no host-fns declared — artifact.create must be denied (R-0003-b)"
    );
    assert!(
        !runtime.is_host_fn_allowed("log.emit"),
        "no host-fns declared — log.emit must be denied (R-0003-b)"
    );
}

/// Given a manifest declaring host-fns,
/// When an undeclared host-fn is attempted,
/// Then load still succeeds (the allowlist is compiled at load time — the WIT
/// boundary enforcement itself is call-time) but `is_host_fn_allowed` returns
/// false for the undeclared fn.
///
/// # Integration-test scope note (R-0003-b partial)
///
/// R-0003-b states "calls outside the allowlist fail AT THE WIT BOUNDARY, not at
/// the host-fn body". Testing the WIT boundary rejection requires a live plugin
/// instance making an actual call — that is integration-test scope, not exercisable
/// at the manifest-load seam. This test pins the allowlist query surface (the
/// pre-condition for WIT enforcement); the dispatch report flags the WIT-boundary
/// assertion as pending integration coverage.
#[test]
fn undeclared_host_fn_is_denied_by_allowlist_query() {
    // Given
    let signing_key = generate_keypair();
    let required = &["artifact.create", "artifact.get", "artifact.list"];
    let manifest = signed_manifest_standard(&signing_key, 1, &["echo.create"], required, &[]);

    // When
    let runtime = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes())
        .expect("manifest with partial host-fn list must load (R-0003-b)");

    // Then
    assert!(
        runtime.is_host_fn_allowed("artifact.create"),
        "artifact.create declared — must be allowed"
    );
    assert!(
        !runtime.is_host_fn_allowed("artifact.delete"),
        "artifact.delete NOT declared — must be denied (R-0003-b allowlist check)"
    );
    assert!(
        !runtime.is_host_fn_allowed("projection.emit"),
        "projection.emit NOT declared — must be denied"
    );
}

// ===========================================================================
// R-0003-c / R-0017-b — Schema version branching
// ===========================================================================

/// Given a manifest with `schema_version = 1` (the supported V0 version),
/// When the manifest is loaded,
/// Then the load succeeds and `runtime.schema_version()` returns 1.
///
/// Red: compile fail (missing plugin module).
/// Green: `PluginRuntime::load` returns `Ok`; `schema_version()` returns 1.
#[test]
fn schema_version_1_loads_successfully() {
    // Given
    let signing_key = generate_keypair();
    let manifest = signed_manifest_standard(
        &signing_key,
        1,
        &["echo.create", "echo.get"],
        &["artifact.create"],
        &[],
    );

    // When
    let result = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes());

    // Then
    assert!(
        result.is_ok(),
        "schema_version = 1 must load successfully against the V0 runtime (R-0003-c); \
         got Err: {result:?}"
    );
    let runtime = result.unwrap();
    assert_eq!(
        runtime.schema_version(),
        1,
        "loaded runtime must report schema_version = 1 (R-0003-c)"
    );
}

/// Given a manifest with an unknown `schema_version` (e.g. 99),
/// When the manifest is loaded,
/// Then the load fails with a structured `LoadError::UnknownSchemaVersion` that
/// carries both the observed version and the set of supported versions (R-0003-c,
/// R-0017-b).
///
/// The structured error enables the host to emit a diagnostic that tells the
/// operator exactly which version was found and what the runtime supports — this
/// is operationally necessary for forward-compat migrations.
///
/// Red: compile fail (missing plugin module).
/// Green: `PluginRuntime::load` returns `Err(LoadError::UnknownSchemaVersion { found: 99, .. })`.
#[test]
fn unknown_schema_version_produces_structured_load_error() {
    // Given — schema_version 99 is not supported at V0
    let signing_key = generate_keypair();
    let manifest = signed_manifest_standard(
        &signing_key,
        99,
        &["echo.create"],
        &["artifact.create"],
        &[],
    );

    // When
    let result = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes());

    // Then
    assert!(
        result.is_err(),
        "schema_version = 99 must fail to load with a structured error (R-0003-c, R-0017-b); \
         got Ok"
    );

    match result.unwrap_err() {
        LoadError::UnknownSchemaVersion { found, supported } => {
            assert_eq!(
                found, 99,
                "LoadError::UnknownSchemaVersion must carry the observed version (R-0017-b)"
            );
            assert!(
                !supported.is_empty(),
                "LoadError::UnknownSchemaVersion must name at least one supported version (R-0017-b)"
            );
            assert!(
                supported.contains(&1),
                "supported list must include version 1 — the V0 schema version (R-0017-b); \
                 got {supported:?}"
            );
        }
        other => panic!(
            "expected LoadError::UnknownSchemaVersion for schema_version=99, got {:?} (R-0003-c)",
            other
        ),
    }
}

/// Given a manifest with `schema_version = 0` (below V0 minimum),
/// When the manifest is loaded,
/// Then the load fails with a structured `LoadError::UnknownSchemaVersion`.
///
/// schema_version = 0 was never a valid version; this pins the lower bound.
#[test]
fn schema_version_zero_produces_structured_load_error() {
    // Given
    let signing_key = generate_keypair();
    let manifest = signed_manifest_standard(&signing_key, 0, &["echo.create"], &[], &[]);

    // When
    let result = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes());

    // Then
    assert!(
        result.is_err(),
        "schema_version = 0 must fail to load (R-0003-c); got Ok"
    );
    assert!(
        matches!(
            result.unwrap_err(),
            LoadError::UnknownSchemaVersion { found: 0, .. }
        ),
        "load error must be UnknownSchemaVersion with found=0 (R-0003-c)"
    );
}

// ===========================================================================
// R-0003-f — Plugin output validation: fail-shut on schema mismatch
// ===========================================================================

/// Given a loaded plugin runtime with a known verb,
/// When `validate_output` is called with a small within-cap byte slice,
/// Then the result must NOT be `OutputError::FieldSizeCap` — the size-cap error
/// is reserved for oversized output only (R-0003-f).
///
/// # What this test pins
///
/// This pins the SIZE-CAP LOWER BOUNDARY: bytes well within any reasonable cap
/// must not trigger `FieldSizeCap`. It does NOT assert `Ok` — honest WIT
/// validation in GREEN will reject arbitrary bytes once it binds the WIT type
/// (echo's WIT output types are `string`/`u32`; 4 zero bytes are neither).
/// Asserting `Ok` here would deadlock GREEN: the test file is forbid-scoped for
/// GREEN, so GREEN cannot fix an un-implementable positive fixture.
///
/// The genuine "valid WIT-serialized output → Ok" happy path is
/// GREEN-integration scope (real serialized bytes, added by the implementer in a
/// new test file and verified at re-review).
///
/// Red: compile fail (missing plugin module).
/// Green: `validate_output` returns something other than `Err(FieldSizeCap)` for
///        within-cap bytes; `FieldSizeCap` is reserved for the oversized test.
#[test]
fn within_cap_output_does_not_trip_size_cap() {
    // Given — a loaded runtime with echo.create verb
    let signing_key = generate_keypair();
    let manifest = signed_manifest_standard(
        &signing_key,
        1,
        &["echo.create", "echo.get"],
        &["artifact.create"],
        &[],
    );
    let runtime = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes())
        .expect("manifest must load for size-cap lower-boundary test (R-0003-f)");

    // When — a tiny within-cap slice (far below any reasonable per-field size cap)
    let within_cap_output = b"\x00\x00\x00\x00";
    let result = runtime.validate_output("echo.create", within_cap_output);

    // Then — within-cap bytes must NOT trip the per-field size cap.
    // (They may still be schema-rejected once GREEN binds the WIT type; that's
    //  fine — this test pins ONLY that FieldSizeCap is reserved for oversized output.)
    assert!(
        !matches!(result, Err(OutputError::FieldSizeCap { .. })),
        "within-cap output must not trip the per-field size cap (R-0003-f); \
         FieldSizeCap is reserved for oversized output. got: {result:?}"
    );
}

/// Given a loaded plugin runtime,
/// When `validate_output` is called with bytes that exceed the per-field size cap,
/// Then validation returns `Err(OutputError::FieldSizeCap)` — fails shut, does NOT
/// truncate (R-0003-f).
///
/// Red: compile fail (missing plugin module).
/// Green: `validate_output` returns `Err(OutputError::FieldSizeCap { .. })` for
///        oversized output.
#[test]
fn validate_output_fails_shut_on_oversized_field() {
    // Given — a loaded runtime
    let signing_key = generate_keypair();
    let manifest =
        signed_manifest_standard(&signing_key, 1, &["echo.create"], &["artifact.create"], &[]);
    let runtime = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes())
        .expect("manifest must load for size-cap test (R-0003-f)");

    // When — output bytes far exceed any reasonable per-field size cap (1 MiB)
    let oversized_output = vec![0xffu8; 1024 * 1024 + 1]; // 1 MiB + 1 byte
    let result = runtime.validate_output("echo.create", &oversized_output);

    // Then — must fail shut, not truncate
    assert!(
        result.is_err(),
        "validate_output must fail shut (Err) for oversized output, not truncate (R-0003-f); \
         got Ok"
    );
    assert!(
        matches!(result.unwrap_err(), OutputError::FieldSizeCap { .. }),
        "oversized output must produce OutputError::FieldSizeCap, not SchemaMismatch (R-0003-f)"
    );
}

/// Given a loaded plugin runtime,
/// When `validate_output` is called with output that is structurally mismatched
/// to the declared WIT schema (e.g., random bytes for a structured WIT type),
/// Then validation returns `Err(OutputError::SchemaMismatch)` — fails shut (R-0003-f).
///
/// Red: compile fail (missing plugin module).
/// Green: `validate_output` returns `Err(OutputError::SchemaMismatch { .. })`.
#[test]
fn validate_output_fails_shut_on_schema_mismatch() {
    // Given
    let signing_key = generate_keypair();
    let manifest = signed_manifest_standard(&signing_key, 1, &["echo.get"], &["artifact.get"], &[]);
    let runtime = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes())
        .expect("manifest must load for schema-mismatch test (R-0003-f)");

    // When — clearly invalid WIT bytes: all 0xFF is not a valid WIT encoding
    let invalid_wit_bytes = vec![0xffu8; 16];
    let result = runtime.validate_output("echo.get", &invalid_wit_bytes);

    // Then — must fail shut
    assert!(
        result.is_err(),
        "validate_output must return Err for schema-mismatched bytes (R-0003-f); got Ok"
    );
    // The error kind is SchemaMismatch or FieldSizeCap depending on what the runtime
    // detects first — either is acceptable as long as it is Err (fail-shut).
    assert!(
        matches!(
            result.unwrap_err(),
            OutputError::SchemaMismatch { .. } | OutputError::FieldSizeCap { .. }
        ),
        "schema-mismatched output must produce SchemaMismatch or FieldSizeCap, not Ok (R-0003-f)"
    );
}

// ===========================================================================
// R-0010-d — Per-verb capability check before dispatch
// ===========================================================================

/// Given a signed manifest declaring `verbs.exposed = ["echo.create", "echo.get"]`,
/// When the manifest is loaded,
/// Then `is_verb_allowed("echo.create")` and `is_verb_allowed("echo.get")` return
/// true, and any undeclared verb returns false — enforced before dispatching to the
/// plugin runtime (R-0010-d).
///
/// Red: compile fail (missing plugin module).
/// Green: `is_verb_allowed` reflects the manifest's `[verbs].exposed` list.
#[test]
fn capability_check_permits_declared_verbs() {
    // Given
    let signing_key = generate_keypair();
    let verbs = &["echo.create", "echo.get", "echo.list"];
    let manifest = signed_manifest_standard(&signing_key, 1, verbs, &["artifact.create"], &[]);

    // When
    let runtime = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes())
        .expect("manifest with declared verbs must load (R-0010-d)");

    // Then — declared verbs are allowed
    assert!(
        runtime.is_verb_allowed("echo.create"),
        "echo.create is declared in exposed[] — must be allowed (R-0010-d)"
    );
    assert!(
        runtime.is_verb_allowed("echo.get"),
        "echo.get is declared in exposed[] — must be allowed (R-0010-d)"
    );
    assert!(
        runtime.is_verb_allowed("echo.list"),
        "echo.list is declared in exposed[] — must be allowed (R-0010-d)"
    );

    // Then — undeclared verbs are denied (before dispatch)
    assert!(
        !runtime.is_verb_allowed("echo.delete"),
        "echo.delete NOT in exposed[] — must be denied before dispatch (R-0010-d)"
    );
    assert!(
        !runtime.is_verb_allowed("echo.update"),
        "echo.update NOT in exposed[] — must be denied before dispatch (R-0010-d)"
    );
    assert!(
        !runtime.is_verb_allowed("task.create"),
        "task.create is a different plugin's verb — must be denied (R-0010-d)"
    );
}

/// Given a signed manifest with an empty `verbs.exposed = []`,
/// When the manifest is loaded,
/// Then NO verb is allowed — the capability list is empty.
///
/// Red: compile fail.
/// Green: `is_verb_allowed` returns false for any verb name.
#[test]
fn empty_verbs_declaration_produces_empty_capability_list() {
    // Given
    let signing_key = generate_keypair();
    let manifest = signed_manifest_standard(&signing_key, 1, &[], &[], &[]);

    // When
    let runtime = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes())
        .expect("empty verbs manifest must load (R-0010-d)");

    // Then
    assert!(
        !runtime.is_verb_allowed("echo.create"),
        "no verbs declared — echo.create must be denied (R-0010-d)"
    );
}

/// Given a manifest where one verb is declared and another is not,
/// When a dispatch for the undeclared verb is attempted,
/// Then `is_verb_allowed` returns false — blocking the dispatch before it reaches
/// the plugin runtime (R-0010-d).
///
/// This pins the pre-dispatch check: the allowlist is consulted BEFORE sending
/// the invocation to the WASM instance.
#[test]
fn undeclared_verb_is_denied_before_dispatch() {
    // Given
    let signing_key = generate_keypair();
    let manifest =
        signed_manifest_standard(&signing_key, 1, &["echo.create"], &["artifact.create"], &[]);

    // When
    let runtime = PluginRuntime::load(&manifest, &signing_key.verifying_key().to_bytes())
        .expect("manifest must load (R-0010-d)");

    // Then
    assert!(
        runtime.is_verb_allowed("echo.create"),
        "echo.create is declared — allowed (R-0010-d)"
    );
    assert!(
        !runtime.is_verb_allowed("echo.delete"),
        "echo.delete NOT declared — denied before dispatch (R-0010-d)"
    );
}

// ===========================================================================
// Fail-shut ordering: verify_plugin runs BEFORE all downstream checks
// ===========================================================================

/// Given an unsigned (or invalidly signed) manifest with a known schema_version
/// and valid verb/host_fn declarations,
/// When the manifest is loaded,
/// Then the load fails with `LoadError::VerificationFailed` — not with
/// `UnknownSchemaVersion` or any other error from downstream pipeline steps.
///
/// This confirms the fail-shut ordering: verification runs first; downstream
/// pipeline gates are unreachable if verification fails.
///
/// Red: compile fail.
/// Green: `Err(LoadError::VerificationFailed(..))` on bad signature.
#[test]
fn verification_runs_before_schema_version_check() {
    // Given — valid schema_version=1 content but WRONG signing key (attacker key)
    let root_key = generate_keypair();
    let attacker_key = generate_keypair();

    let manifest = signed_manifest_standard(&attacker_key, 1, &["echo.create"], &[], &[]);
    // Load with root_key's bytes as root_material — attacker's key won't match
    // (Interpretation B: manifest public_key = hex(attacker) != hex(root))

    // When
    let result = PluginRuntime::load(&manifest, &root_key.verifying_key().to_bytes());

    // Then — must fail at verification, not at schema_version or allowlist stage
    assert!(
        result.is_err(),
        "attacker-signed manifest must fail to load (fail-shut ordering)"
    );
    assert!(
        matches!(result.unwrap_err(), LoadError::VerificationFailed(_)),
        "failure must be VerificationFailed, not UnknownSchemaVersion — \
         verification runs before schema_version check (fail-shut ordering)"
    );
}

/// Given a manifest with an unknown schema_version AND an invalid signature,
/// When the manifest is loaded,
/// Then the load fails with `VerificationFailed` — NOT `UnknownSchemaVersion`.
///
/// Confirms ordering: even a bad schema_version doesn't surface if signature fails.
#[test]
fn verification_failure_precedes_unknown_schema_version() {
    // Given — unknown schema_version AND wrong key
    let root_key = generate_keypair();
    let attacker_key = generate_keypair();

    let manifest = signed_manifest_standard(&attacker_key, 99, &[], &[], &[]);

    // When
    let result = PluginRuntime::load(&manifest, &root_key.verifying_key().to_bytes());

    // Then
    assert!(result.is_err(), "must fail (bad sig + bad schema_version)");
    assert!(
        matches!(result.unwrap_err(), LoadError::VerificationFailed(_)),
        "VerificationFailed must surface before UnknownSchemaVersion — \
         verification is the outermost gate (fail-shut ordering)"
    );
}
