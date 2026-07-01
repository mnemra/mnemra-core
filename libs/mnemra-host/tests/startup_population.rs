//! Startup pool population acceptance tests — RED phase (dispatch #1097, Task 1815 / T11).
//!
//! # Purpose
//!
//! Pins the contract for `populate_verified_pool`: the production startup function
//! that verifies a plugin manifest's signature BEFORE populating the pool
//! (fail-closed security gate, R-0005-a).
//!
//! # R-ID mapping
//!
//! | Test function                                                    | R-ID(s)             |
//! |------------------------------------------------------------------|---------------------|
//! | verified_manifest_populates_pool_and_call_tool_dispatches        | R-0005-a, R-0016-a  |
//! | wrong_root_material_refuses_population                           | R-0005-a, R-0005-b  |
//!
//! # Stub-resistance (why these two tests must be paired)
//!
//! Test 1 (`verified_manifest_populates_pool_and_call_tool_dispatches`) forces a
//! REAL populated pool: the echo create→get round-trip fails if the pool holds no
//! live instance (stub returning `Ok(empty_pool)` would fail at dispatch).
//!
//! Test 2 (`wrong_root_material_refuses_population`) forces the Err gate: calling
//! with the wrong root key must return `Err`. A stub returning `Ok(real_pool)`
//! unconditionally would pass Test 1 but fail Test 2. Neither stub can satisfy
//! both — the pair forces a real implementation.
//!
//! # RED-phase design
//!
//! `populate_verified_pool` and `StartupError` do not exist yet — they are T11 Forge
//! deliverables. This file compiles EXCEPT for those two unresolved symbols; that
//! compile-fail IS the red signal. Every other symbol is valid (signing helpers,
//! server setup, harness utilities).
//!
//! # verify: []
//!
//! `verify: []` is intentional for a red-phase dispatch. The tests cannot pass
//! (the function doesn't exist). Green phase adds the recipe.
//!
//! # No hardcoded key material
//!
//! All Ed25519 keypairs are generated per-run via `generate_keypair()`.
//! No key bytes, seed bytes, or DER blobs appear literally in this file.
//!
//! # Success-path signing mechanism (inline, not imported)
//!
//! The signing helpers (`generate_keypair`, `manifest_bytes_unsigned`,
//! `sign_manifest`, `manifest_with_signature`, `hex_encode`) are private fns in
//! `tests/signing_chain.rs` and cannot be imported across integration-test crates.
//! They are inlined here verbatim (exact bodies, same doc contract).
//!
//! # Test 1 server setup
//!
//! The server setup is inlined (not `#[path] mod slice1_harness`) to avoid
//! importing `slice1_echo_harness` + `echo_component_path` as dead code, which
//! would produce unused-symbol warnings or errors that would pollute RED evidence.
//! The sequence mirrors `slice1_harness.rs` lines 75-136 exactly, except the
//! `PluginPool` comes from `populate_verified_pool` rather than the bypass loader.
//!
//! # Interpretation-B note (for Forge)
//!
//! `verify_plugin` (the gate inside `populate_verified_pool`) applies an
//! Interpretation-B fingerprint cross-check BEFORE the ed25519 math: the
//! manifest's `public_key` hex field must match the hex of `root_material`. Test 2
//! therefore trips `FingerprintMismatch` (not `VerificationFailed`) for the
//! wrong-key and garbage-key cases. Both are `Err` — the test asserts only
//! `.is_err()`, which is correct and sufficient. Forge must not weaken the
//! Interpretation-B check to make the wrong-key path reach ed25519 math.

// ---------------------------------------------------------------------------
// Imports
// ---------------------------------------------------------------------------

use std::sync::{Arc, Mutex};

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use mnemra_host::auth::token::{generate, hash};
use mnemra_host::mcp::server::MnemraMcpServer;
use mnemra_host::schema::init::init;
use mnemra_host::startup::populate_verified_pool;
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
// StartupError is referenced via explicit type annotation to ensure it appears
// as an unresolved symbol in the RED compile output alongside populate_verified_pool.
use mnemra_host::startup::StartupError;
use rand::TryRng;
use rmcp::model::{CallToolRequestParams, RawContent};
use rmcp::service::{RoleClient, RunningService, serve_client, serve_server};
use serde_json::json;
use tokio::io::duplex;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Startup serialisation lock (A-11 design decision)
// ---------------------------------------------------------------------------

/// Serialises embedded-Postgres engine startup across concurrent test threads
/// within this binary. Mirrors the same lock in `mcp_slice1_e2e.rs` (A-11).
static STARTUP_LOCK: Mutex<()> = Mutex::new(());

/// Start a fresh embedded engine with startup serialised (A-11).
async fn start_engine() -> EmbeddedEngine {
    {
        let _guard = STARTUP_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    }
    EmbeddedEngine::start()
        .await
        .expect("failed to start embedded Postgres")
}

// ---------------------------------------------------------------------------
// Signing fixture helpers (inlined from tests/signing_chain.rs — private fns
// cannot be imported across integration-test crates)
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

/// Build a minimal valid `core = true` manifest TOML, ready to be signed.
///
/// The `[signature]` section is NOT included — this is the canonical form
/// (manifest-minus-signature) that the signing contract operates over.
fn manifest_bytes_unsigned(name: &str, version: &str, core: bool) -> Vec<u8> {
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
fn sign_manifest(signing_key: &SigningKey, manifest_bytes: &[u8]) -> Vec<u8> {
    signing_key.sign(manifest_bytes).to_bytes().to_vec()
}

/// Build the full manifest TOML including the `[signature]` section.
fn manifest_with_signature(
    name: &str,
    version: &str,
    core: bool,
    verifying_key: &VerifyingKey,
    sig_bytes: &[u8],
) -> Vec<u8> {
    let unsigned = manifest_bytes_unsigned(name, version, core);
    let unsigned_str = String::from_utf8(unsigned).unwrap();
    let pubkey_hex = hex_encode(verifying_key.as_bytes());
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
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Locate the built `mnemra-echo` component (`wasm32-wasip2`, release).
///
/// Same path as `pool_population.rs::echo_component_path()` and
/// `content_hash_binding.rs::echo_component_path()` — private fns cannot cross
/// integration-test crates, so each test file inlines its own copy.
fn echo_component_path() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root from libs/mnemra-host");
    root.join("target/wasm32-wasip2/release/mnemra_echo.wasm")
}

/// Lowercase-hex BLAKE3 digest of the on-disk echo component.
///
/// Used to embed a correct `[component].hash` in the signed manifest for the
/// positive test (R-0021-e fixture update — T2 GREEN).
fn echo_blake3_hex() -> String {
    let bytes = std::fs::read(echo_component_path()).expect("read echo component bytes");
    blake3::hash(&bytes).to_hex().to_string()
}

// ---------------------------------------------------------------------------
// MCP test utilities (inlined from mcp_slice1_e2e.rs helpers)
// ---------------------------------------------------------------------------

/// Build a `Meta` carrying the auth token in the `token` key.
fn token_meta(token_str: &str) -> rmcp::model::Meta {
    let mut meta = rmcp::model::Meta::new();
    meta.insert("token".to_owned(), json!(token_str));
    meta
}

/// Extract all text strings from a `CallToolResult`'s content vector.
fn extract_text_content(result: &rmcp::model::CallToolResult) -> Vec<&str> {
    result
        .content
        .iter()
        .filter_map(|c| match &c.raw {
            RawContent::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect()
}

/// Validate that `s` is a well-formed ULID: 26 chars, Crockford base32 alphabet.
fn is_valid_ulid(s: &str) -> bool {
    if s.len() != 26 {
        return false;
    }
    s.chars()
        .all(|c| "0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(c))
}

// ---------------------------------------------------------------------------
// Test 1 — success path: verified manifest populates pool + call_tool dispatches
// ---------------------------------------------------------------------------

/// R-0005-a, R-0016-a — verified manifest populates pool; call_tool round-trips.
///
/// # Given / When / Then
///
/// GIVEN a synthetic manifest signed by a fresh keypair (the same key is passed
///       as root_material so `verify_plugin` accepts it)
/// WHEN `populate_verified_pool(&signed_manifest, &verifying_key.to_bytes())` is called
/// THEN it returns `Ok(pool)` with a live populated instance pool
///   AND building an `MnemraMcpServer` over that pool and invoking echo.create
///       returns a well-formed ULID
///   AND echo.get round-trips the payload
///
/// # Stub-resistance
///
/// A stub returning `Ok(empty_pool)` would fail at the echo.create dispatch
/// (no pooled instance to borrow). A stub returning `Err` always would fail
/// the `is_ok()` assertion. Only a real `populate_verified_pool` that loads
/// the echo component can satisfy both halves.
///
/// # Engine lifetime
///
/// The `engine` binding MUST remain in scope for the entire test — dropping it
/// tears down the embedded Postgres. The pool is cloned (PgPool is Arc-backed).
#[tokio::test]
async fn verified_manifest_populates_pool_and_call_tool_dispatches() {
    // Build a synthetic signed manifest — on-disk manifest.toml has placeholder
    // sig bytes and is NOT verifiable (real signing is Task 26).
    //
    // T2 GREEN fixture update (R-0021-e): include [component] section with a
    // correct BLAKE3 hash inside the signed body so the content-hash gate passes.
    // [component] must fall inside the signature-covered slice; manifest_with_signature
    // re-generates the unsigned base internally (losing [component]), so the full
    // manifest is built inline here, mirroring wrap_signed in content_hash_binding.rs.
    let signing_key = generate_keypair();
    let verifying_key = signing_key.verifying_key();
    let base = String::from_utf8(manifest_bytes_unsigned("mnemra-echo", "0.1.0", true)).unwrap();
    let component_sect = format!(
        "\n[component]\nhash_alg = \"blake3\"\nhash = \"{}\"\n",
        echo_blake3_hex()
    );
    let signed_body = base + &component_sect;
    let sig = sign_manifest(&signing_key, signed_body.as_bytes());
    let pubkey_hex = hex_encode(verifying_key.as_bytes());
    let sig_hex = hex_encode(&sig);
    let signed_manifest = format!(
        "{signed_body}\n[signature]\nalgorithm = \"ed25519\"\npublic_key = \
         \"{pubkey_hex}\"\nsig_bytes = \"{sig_hex}\"\nsigned_at = \"2026-06-13T00:00:00Z\"\n"
    )
    .into_bytes();

    // WHEN: populate_verified_pool verifies then builds the pool
    // Type annotation pins StartupError as an unresolved symbol in RED output.
    let result: Result<Arc<mnemra_host::plugin::pool::PluginPool>, StartupError> =
        populate_verified_pool(&signed_manifest, &verifying_key.to_bytes());

    // THEN: returns Ok with a real pool
    assert!(
        result.is_ok(),
        "R-0005-a: populate_verified_pool with a validly-signed manifest must return Ok(pool); \
         got Err: {:?}",
        result.err()
    );
    let plugin_pool = Arc::new(result.unwrap());

    // Build the embedded Postgres + schema to back the MCP server auth path
    let engine = start_engine().await;
    init(&engine, "vector")
        .await
        .expect("schema init should succeed");
    let pg_pool = engine.pool.as_ref().clone();

    // Seed an admin-scoped token for the default workspace (per-run, no literals)
    let admin_token = generate();
    let token_hash = hash(&admin_token);
    sqlx::query("INSERT INTO admin_tokens (token_hash, workspace_id, scopes) VALUES ($1, $2, $3)")
        .bind(token_hash.as_bytes())
        .bind(mnemra_host::schema::init::DEFAULT_WORKSPACE_ID)
        .bind(&vec!["admin".to_owned()])
        .execute(&pg_pool)
        .await
        .expect("seed admin token");

    // Build the real MnemraMcpServer over the pool returned by populate_verified_pool
    let server = MnemraMcpServer::new(pg_pool, Arc::clone(&plugin_pool));

    // Serve over an in-process duplex transport
    let (server_transport, client_transport) = duplex(8192);
    let _server_handle = tokio::spawn(async move {
        match serve_server(server, server_transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => eprintln!("startup_population harness server init failed: {e:?}"),
        }
    });
    let client: RunningService<RoleClient, ()> = serve_client((), client_transport)
        .await
        .expect("startup_population harness client init");

    // WHEN: call echo.create with a recognisable payload
    let recognisable_payload = format!("startup_population_payload_{}", Uuid::new_v4());
    let mut create_params = CallToolRequestParams::new("echo.create");
    create_params.meta = Some(token_meta(admin_token.as_str()));
    create_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("content_type".to_owned(), json!("echo_fixture"));
        m.insert("payload".to_owned(), json!(recognisable_payload));
        m
    });

    let create_result = client
        .call_tool(create_params)
        .await
        .expect("R-0019, R-0005-a: echo.create through verify-gated pool must return Ok");

    // THEN: result must contain a well-formed ULID
    let texts = extract_text_content(&create_result);
    assert!(
        !texts.is_empty(),
        "R-0019: echo.create result must have at least one text content item; \
         got content: {:?}",
        create_result.content
    );

    let ulid_str = texts
        .iter()
        .find(|t| is_valid_ulid(t.trim()))
        .map(|t| t.trim())
        .unwrap_or_else(|| {
            panic!(
                "R-0019: echo.create result must contain a well-formed ULID (26 chars, \
                 Crockford base32 [0-9A-HJKMNP-TV-Z]{{26}}); got text content: {:?}",
                texts
            )
        });

    // AND WHEN: call echo.get with the returned ULID to assert round-trip
    let mut get_params = CallToolRequestParams::new("echo.get");
    get_params.meta = Some(token_meta(admin_token.as_str()));
    get_params.arguments = Some({
        let mut m = serde_json::Map::new();
        m.insert("id".to_owned(), json!(ulid_str));
        m
    });

    let get_result = client
        .call_tool(get_params)
        .await
        .expect("R-0005-a, R-0019: echo.get through verify-gated pool must return Ok");

    // THEN: result content must contain the recognisable payload (round-trip)
    let get_texts = extract_text_content(&get_result);
    let payload_found = get_texts.iter().any(|t| t.contains(&recognisable_payload));
    assert!(
        payload_found,
        "R-0019: echo.get must round-trip the written payload through the verify-gated pool; \
         expected text content to contain '{}', got: {:?}",
        recognisable_payload, get_texts
    );
}

// ---------------------------------------------------------------------------
// Test 2 — failure path: wrong root material refuses population (security gate)
// ---------------------------------------------------------------------------

/// R-0005-a, R-0005-b — wrong root material causes verify-gate to reject; no pool produced.
///
/// # Given / When / Then
///
/// GIVEN a manifest validly signed by keypair A
/// WHEN `populate_verified_pool(&signed_manifest, &keypair_B.verifying_key().to_bytes())`
///      is called with the WRONG root key (keypair B)
/// THEN it returns `Err(...)` — the verify gate rejects before any pool is built
///
/// GIVEN the same signed manifest
/// WHEN `populate_verified_pool(&signed_manifest, &[])` is called with garbage root material
/// THEN it also returns `Err(...)` — no pool is produced
///
/// # Security centerpiece
///
/// This test proves the verify gate actually checks the key, not merely that
/// absent input fails. The Interpretation-B fingerprint cross-check in
/// `verify_plugin` means: the manifest's `public_key` hex must match
/// `hex(root_material)`. A wrong key produces `FingerprintMismatch` → `Err`.
/// Garbage key bytes trip `VerificationFailed` (invalid key bytes) → `Err`.
///
/// # Stub-resistance
///
/// A stub returning `Ok(real_pool)` unconditionally would fail this test's
/// `is_err()` assertion. Only an implementation that actually calls `verify_plugin`
/// and propagates its `Err` can satisfy both Test 1 and Test 2.
///
/// # No Postgres needed
///
/// This test does not start EmbeddedEngine — the Err gate fires inside
/// `populate_verified_pool` before any pool construction, so no PgPool or
/// MCP server is needed.
#[test]
fn wrong_root_material_refuses_population() {
    // Keypair A — the key that signed the manifest
    let signing_key_a = generate_keypair();
    let verifying_key_a = signing_key_a.verifying_key();

    // Keypair B — an ATTACKER'S key, not the signer
    let signing_key_b = generate_keypair();
    let verifying_key_b = signing_key_b.verifying_key();

    // Build a manifest signed by keypair A (valid sig, valid public_key = hex(A))
    let unsigned = manifest_bytes_unsigned("mnemra-echo", "0.1.0", true);
    let sig = sign_manifest(&signing_key_a, &unsigned);
    let signed_manifest =
        manifest_with_signature("mnemra-echo", "0.1.0", true, &verifying_key_a, &sig);

    // CASE 1: wrong root material (keypair B's key) — must Err
    // Interpretation B: manifest public_key = hex(A) != hex(B) → FingerprintMismatch → Err
    let result_wrong_key: Result<Arc<mnemra_host::plugin::pool::PluginPool>, StartupError> =
        populate_verified_pool(&signed_manifest, &verifying_key_b.to_bytes());

    assert!(
        result_wrong_key.is_err(),
        "R-0005-a, R-0005-b: populate_verified_pool with wrong root key must return Err; \
         the verify gate MUST check root material identity, not just that material is present; \
         got Ok — the gate is absent or bypassed"
    );

    // CASE 2: garbage root material (empty slice) — must also Err
    // verify_plugin rejects if root_material is not 32 bytes (VerificationFailed)
    let result_garbage: Result<Arc<mnemra_host::plugin::pool::PluginPool>, StartupError> =
        populate_verified_pool(&signed_manifest, &[]);

    assert!(
        result_garbage.is_err(),
        "R-0005-a: populate_verified_pool with empty root material must return Err; \
         got Ok — the verify gate does not check root_material length"
    );

    // CASE 3: short garbage root material — must also Err
    let result_short: Result<Arc<mnemra_host::plugin::pool::PluginPool>, StartupError> =
        populate_verified_pool(&signed_manifest, &[0u8; 8]);

    assert!(
        result_short.is_err(),
        "R-0005-a: populate_verified_pool with malformed root material (8 bytes, not 32) must \
         return Err; got Ok — the verify gate does not validate root_material"
    );
}
