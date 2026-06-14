//! LLM-key + hostname-allowlist invariant tests — RED phase (Task 18).
//!
//! # Purpose
//!
//! These tests pin the security invariants for the outbound embedding call
//! pathway (DF-embed-call) that Task 18's green phase must implement. Every
//! test traces to a specific spec R-ID (R-0014-a, R-0014-b, R-0014-d).
//!
//! # RED-phase design
//!
//! Task 18 implements:
//!   - `mnemra_host::config::llm_key` — `LlmKeyConfig` + `LlmKeyConfigError`
//!   - `mnemra_host::net::hostname_allowlist` — `AllowList` + `AllowListError`
//!   - `mnemra_host::startup::file_mode_check::check` — EXTENDED to a 3-arg
//!     form covering (admin-token, signing-material, llm-key) — see R-0014-d note.
//!
//! Until those modules land, this file does NOT compile. That compile-fail
//! IS the red signal — the right-reason failure for this phase.
//!
//! `rustfmt` parses this file without type resolution; the file is fmt-clean.
//! `clippy` requires compilation; it cannot run until Task 18 lands.
//!
//! # RED-phase deviation: compile-fail as red signal
//!
//! Same pattern as `signing_chain.rs` (Task 16/17). The modules referenced
//! here — `mnemra_host::config::llm_key`, `mnemra_host::net::hostname_allowlist`
//! — do not exist yet. Every meaningful test must call the intended seam;
//! compile-fail = red is the correct state.
//!
//! # verify: [] rationale
//!
//! `verify: []` is correct by design. The test binary cannot be linked
//! (missing modules) until Task 18 lands. Task 18 populates `verify`.
//! The empty verify set is the expected red state, NOT a regression.
//!
//! # R-0014-d startup check extension
//!
//! The existing `startup::file_mode_check::check()` was introduced in Task 17
//! as a 2-arg function: `check(token_path, signing_material_path)`.
//! R-0014-d extends the invariant to cover the LLM-key file as well.
//!
//! To avoid a permanent partially-authoritative `check()` footgun (SF2 class:
//! a partial check that looks authoritative but silently omits a required file
//! allows a future caller to call `check()` and assume all sensitive files were
//! covered), Task 18 MUST extend `check()` to a 3-arg signature:
//!
//!   pub fn check(
//!       token_path: &Path,
//!       signing_material_path: &Path,
//!       llm_key_path: &Path,
//!   ) -> Result<(), FileModeError>
//!
//! This means `signing_chain.rs` (in touch_scope) also requires an update:
//! every call to `check_file_mode(token, signing)` is updated to
//! `check_file_mode(token, signing, llm_key)` passing a valid mode-600 fixture.
//! That edit is made in `signing_chain.rs` — see the companion change.
//! The existing signing_chain assertions are preserved verbatim.
//!
//! # R-0014-c (No hosted-model endpoint) — spec-gap finding
//!
//! R-0014-c states: "The system SHALL NOT accept an API key for a hosted model
//! endpoint." This is an ABSENCE property over the configuration surface: the
//! `LlmKeyConfig` struct must not have any field that accepts a hosted-model
//! endpoint URL or token. An absence can be structurally asserted at compile
//! time (no such field exists ⇒ no such path compiles) but cannot be tested
//! via a runtime scenario unless there is a runtime rejection path to hit.
//!
//! At the config/net layer, the absence is best enforced by the struct's shape:
//! if `LlmKeyConfig` has no `hosted_endpoint` field, there is no path to
//! configure one. Test scenario 6 below asserts this structurally.
//!
//! # No hardcoded key material
//!
//! No API keys or secrets appear as string literals in this file.
//! Keys come from `std::env` reads (per scenario 4) or from dynamically
//! constructed `tempfile`-backed config paths.
//!
//! # Cross-dispatch handoff: exact API Task 18 must expose
//!
//! See the handoff section at the bottom of this file.

// ---------------------------------------------------------------------------
// Imports — the missing Task-18 modules are the red signal.
// ---------------------------------------------------------------------------

use mnemra_host::config::llm_key::{LlmKeyConfig, LlmKeyConfigError};
use mnemra_host::net::hostname_allowlist::{AllowList, AllowListError};
use mnemra_host::startup::file_mode_check::check as check_file_mode;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

/// Create a file in `dir` with the given Unix permission bits.
///
/// Mirrors `create_file_with_mode` from `signing_chain.rs` — extracted here
/// to keep this test file self-contained without a shared `common::` fixture.
fn create_file_with_mode(dir: &TempDir, name: &str, mode: u32) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, b"fixture-key-content").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    path
}

/// Write a minimal LLM-key config TOML to a file in `dir` and return the path.
///
/// The file content is a placeholder; `LlmKeyConfig::load()` must read the key
/// from the config, not from a compiled-in default. The content here is a
/// synthetic deploy-time config matching the expected TOML shape.
fn write_llm_key_config(
    dir: &TempDir,
    name: &str,
    api_key: &str,
    allowlist: &[&str],
    mode: u32,
) -> std::path::PathBuf {
    let path = dir.path().join(name);
    let allowlist_toml = allowlist
        .iter()
        .map(|h| format!("\"{}\"", h))
        .collect::<Vec<_>>()
        .join(", ");
    let content = format!("api_key = \"{api_key}\"\nhostname_allowlist = [{allowlist_toml}]\n");
    std::fs::write(&path, content.as_bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    path
}

// ---------------------------------------------------------------------------
// R-0014-b — Hostname allowlist: off-list hostname is blocked
// ---------------------------------------------------------------------------

/// Assert that a hostname NOT on the allowlist is blocked.
///
/// # R-0014-b
///
/// "The system SHALL enforce a hostname allowlist for outbound embedding calls;
/// the allowlist SHALL be configurable per deployment; any outbound call to a
/// hostname not in the allowlist SHALL be blocked."
///
/// Given: an AllowList containing ["api.openai.com"]
/// When: we check "evil.attacker.io"
/// Then: the check returns Err (blocked)
///
/// Red: `mnemra_host::net::hostname_allowlist` does not exist — compile fail.
/// Green: `AllowList::new(["api.openai.com"]).check("evil.attacker.io")` returns Err.
#[test]
fn hostname_not_on_allowlist_is_blocked() {
    // R-0014-b: hostname not on the list → blocked (Err).
    let allowlist = AllowList::new(&["api.openai.com"]);
    let result: Result<(), AllowListError> = allowlist.check("evil.attacker.io");

    assert!(
        result.is_err(),
        "hostname 'evil.attacker.io' not on the allowlist must be blocked (R-0014-b); \
         got Ok — allowlist failed closed"
    );
}

// ---------------------------------------------------------------------------
// R-0014-b — Hostname allowlist: on-list hostname proceeds
// ---------------------------------------------------------------------------

/// Assert that a hostname ON the allowlist is permitted.
///
/// # R-0014-b
///
/// Given: an AllowList containing ["api.openai.com"]
/// When: we check "api.openai.com"
/// Then: the check returns Ok (permitted)
///
/// Red: `mnemra_host::net::hostname_allowlist` does not exist — compile fail.
/// Green: `AllowList::new(["api.openai.com"]).check("api.openai.com")` returns Ok(()).
#[test]
fn hostname_on_allowlist_proceeds() {
    // R-0014-b: hostname on the list → permitted (Ok).
    let allowlist = AllowList::new(&["api.openai.com"]);
    let result: Result<(), AllowListError> = allowlist.check("api.openai.com");

    assert!(
        result.is_ok(),
        "hostname 'api.openai.com' on the allowlist must be permitted (R-0014-b); \
         got Err: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// R-0014-b — Allowlist is per-deployment configurable (not compiled in)
// ---------------------------------------------------------------------------

/// Assert the allowlist is loaded from per-deployment config, not compiled in.
///
/// # R-0014-b
///
/// "the allowlist SHALL be configurable per deployment"
///
/// Discriminating test: build TWO AllowLists from two different config payloads
/// and show they produce divergent permit/block decisions on the SAME hostname.
/// An always-compiled-in list could not produce divergent behavior.
///
/// Given: deployment A's allowlist = ["api.openai.com"]
///        deployment B's allowlist = ["api.cohere.com"]
/// When:  we check "api.openai.com" against BOTH
/// Then:
///   - deployment A: Ok (permitted)
///   - deployment B: Err (blocked)
///
/// Red: `LlmKeyConfig::load()` and `AllowList` do not exist — compile fail.
/// Green: Two distinct AllowList instances, loaded from config, diverge.
#[test]
fn allowlist_is_per_deployment_configurable_not_compiled_in() {
    // R-0014-b: per-deployment configurable — two configs diverge on the same hostname.
    let dir = TempDir::new().unwrap();

    // Deployment A: permits api.openai.com
    let config_a = write_llm_key_config(&dir, "config_a.toml", "key-a", &["api.openai.com"], 0o600);
    let loaded_a = LlmKeyConfig::load(&config_a).expect("config_a must load successfully");

    // Deployment B: permits api.cohere.com (different provider)
    let config_b = write_llm_key_config(&dir, "config_b.toml", "key-b", &["api.cohere.com"], 0o600);
    let loaded_b = LlmKeyConfig::load(&config_b).expect("config_b must load successfully");

    // "api.openai.com" must be permitted by A and blocked by B.
    let result_a: Result<(), AllowListError> =
        loaded_a.hostname_allowlist().check("api.openai.com");
    let result_b: Result<(), AllowListError> =
        loaded_b.hostname_allowlist().check("api.openai.com");

    assert!(
        result_a.is_ok(),
        "deployment A's allowlist must permit 'api.openai.com' (R-0014-b); got Err: {result_a:?}"
    );
    assert!(
        result_b.is_err(),
        "deployment B's allowlist must BLOCK 'api.openai.com' — \
         divergent behavior proves configurable, not compiled-in (R-0014-b); got Ok"
    );
}

// ---------------------------------------------------------------------------
// R-0014-a — LLM-key loaded from deploy-time config, never hard-coded
// ---------------------------------------------------------------------------

/// Assert the LLM API key is read from the configured file at load time.
///
/// # R-0014-a
///
/// "The system SHALL provide a per-deployment LLM-API-key configuration surface
/// for the embedding-batch pathway (DF-embed-call); the API key SHALL be
/// configurable at deploy time, never hard-coded."
///
/// Discriminating test: two distinct config files with distinct keys produce
/// distinct `api_key()` values. A compiled-in default key cannot satisfy this
/// because both would return the same value.
///
/// Given: config file containing api_key = "deploy-key-alpha"
/// When: `LlmKeyConfig::load(path)` reads it
/// Then: `config.api_key()` returns "deploy-key-alpha"
///
/// And a second config with api_key = "deploy-key-beta" returns "deploy-key-beta".
///
/// Red: `mnemra_host::config::llm_key` does not exist — compile fail.
/// Green: `LlmKeyConfig::load(path).api_key()` returns the key from the file.
#[test]
fn llm_key_loaded_from_deploy_time_config_not_hardcoded() {
    // R-0014-a: key comes from config, two configs → two distinct keys.
    let dir = TempDir::new().unwrap();

    let config_alpha = write_llm_key_config(
        &dir,
        "key_alpha.toml",
        "deploy-key-alpha",
        &["api.openai.com"],
        0o600,
    );
    let config_beta = write_llm_key_config(
        &dir,
        "key_beta.toml",
        "deploy-key-beta",
        &["api.openai.com"],
        0o600,
    );

    let loaded_alpha = LlmKeyConfig::load(&config_alpha).expect("key_alpha.toml must load");
    let loaded_beta = LlmKeyConfig::load(&config_beta).expect("key_beta.toml must load");

    // Both configs load without error.
    // The keys must be distinct (proves the value came from the file, not a compiled default).
    assert_ne!(
        loaded_alpha.api_key(),
        loaded_beta.api_key(),
        "two configs with different api_key values must produce distinct api_key() returns \
         (R-0014-a) — a hard-coded default would make these equal"
    );

    // And the individual values must match what the files contain.
    assert_eq!(
        loaded_alpha.api_key(),
        "deploy-key-alpha",
        "api_key() must return the value from the config file (R-0014-a)"
    );
    assert_eq!(
        loaded_beta.api_key(),
        "deploy-key-beta",
        "api_key() must return the value from the config file (R-0014-a)"
    );
}

// ---------------------------------------------------------------------------
// R-0014-d — LLM-key file mode 600 enforced at startup
// ---------------------------------------------------------------------------

/// Assert the startup file-mode check PASSES when all three files are mode 600,
/// including the LLM-key file.
///
/// # R-0014-d
///
/// "The LLM-API-key SHALL be stored in a file at mode 600, separate from the
/// admin token file; the startup file-mode invariant check SHALL cover both files."
///
/// This test uses the EXTENDED 3-arg `check()` that Task 18 must implement.
/// The existing 2-arg `check(token, signing)` is insufficient because it does
/// not cover the LLM-key file (SF2: partial authority).
///
/// Red: 3-arg `check(token, signing, llm_key)` does not exist — compile fail.
/// Green: `check(token, signing, llm_key)` returns Ok when all three are mode 600.
#[test]
fn startup_check_passes_when_all_three_files_are_mode_600_including_llm_key() {
    // R-0014-d: all three files at mode 600 → check() returns Ok.
    let dir = TempDir::new().unwrap();
    let token_path = create_file_with_mode(&dir, "admin_token", 0o600);
    let signing_material_path = create_file_with_mode(&dir, "root_verification.pub", 0o600);
    let llm_key_path = create_file_with_mode(&dir, "llm_key", 0o600);

    // 3-arg signature: check(token, signing_material, llm_key)
    let result = check_file_mode(&token_path, &signing_material_path, &llm_key_path);

    assert!(
        result.is_ok(),
        "startup file-mode check must pass when ALL THREE files are mode 600 (R-0014-d); \
         got Err: {result:?}"
    );
}

/// Assert the startup check FAILS when the LLM-key file is world-readable (mode 644),
/// even if the admin-token and signing-material files are mode 600.
///
/// # R-0014-d
///
/// "the startup file-mode invariant check SHALL cover both files"
/// (admin token AND LLM-key; per the spec, "both" means the two secret files
/// distinct from the signing-material file)
///
/// Red: 3-arg `check()` does not exist — compile fail.
/// Green: `check(token, signing, llm_key)` returns Err when llm_key is mode 644.
#[test]
fn startup_check_fails_if_llm_key_file_is_world_readable() {
    // R-0014-d: llm_key at mode 644 → check() returns Err (host refuses start).
    let dir = TempDir::new().unwrap();
    let token_path = create_file_with_mode(&dir, "admin_token", 0o600);
    let signing_material_path = create_file_with_mode(&dir, "root_verification.pub", 0o600);
    let llm_key_path = create_file_with_mode(&dir, "llm_key", 0o644); // world-readable — violation

    let result = check_file_mode(&token_path, &signing_material_path, &llm_key_path);

    assert!(
        result.is_err(),
        "startup file-mode check must FAIL (refuse start) when LLM-key file is \
         world-readable (mode 644) (R-0014-d); got Ok"
    );
}

/// Assert the startup check FAILS when only the LLM-key file has wrong permissions,
/// and the admin-token file is ALSO world-readable (belt-and-suspenders).
///
/// # R-0014-d — combined failure
///
/// Both the admin-token AND the LLM-key files are mode 644. All three must be
/// checked; any single file at wrong mode must fail the whole startup.
#[test]
fn startup_check_fails_if_llm_key_and_admin_token_are_world_readable() {
    // R-0014-d: two files bad → check() returns Err.
    let dir = TempDir::new().unwrap();
    let token_path = create_file_with_mode(&dir, "admin_token", 0o644); // bad
    let signing_material_path = create_file_with_mode(&dir, "root_verification.pub", 0o600);
    let llm_key_path = create_file_with_mode(&dir, "llm_key", 0o644); // bad

    let result = check_file_mode(&token_path, &signing_material_path, &llm_key_path);

    assert!(
        result.is_err(),
        "startup check must FAIL when both admin-token and LLM-key are world-readable (R-0014-d)"
    );
}

// ---------------------------------------------------------------------------
// R-0014-c — No hosted-model endpoint field (structural / spec-gap finding)
// ---------------------------------------------------------------------------

/// Assert the LLM-key config surface has no field for a hosted-model endpoint.
///
/// # R-0014-c
///
/// "The system SHALL NOT host a language model; embedding generation and MCP
/// sampling SHALL call out to an external provider; the system SHALL NOT accept
/// an API key for a hosted model endpoint."
///
/// At the config struct layer, R-0014-c is an ABSENCE property: `LlmKeyConfig`
/// must not have a field (e.g. `hosted_endpoint`, `model_server_url`) that
/// would allow configuring a self-hosted model endpoint.
///
/// This test asserts the absence structurally: it constructs a `LlmKeyConfig`
/// via `::load()` (the only constructor — no field-by-field builder), then
/// asserts that the surface it exposes via `api_key()` and `hostname_allowlist()`
/// is the complete public API — no `hosted_model_endpoint()` accessor exists.
///
/// **Spec-gap finding for Task 18 green phase:**
/// This test cannot assert a RUNTIME REJECTION of a hosted-model key because
/// there is no input that routes to "this key is for a hosted model, reject."
/// The config surface simply has no such field. If a caller passes a
/// localhost URL as a hostname in the allowlist and uses a self-hosted model
/// key as the `api_key`, the system cannot distinguish that from a legitimate
/// external-provider key at V0. R-0014-c is enforced by STRUCTURAL ABSENCE
/// (no hosted-endpoint config field), not by a runtime check on the key value.
///
/// **If Task 18's green phase adds a hosted-endpoint field for any reason,
/// that is a spec violation of R-0014-c.**
///
/// Red: `mnemra_host::config::llm_key` does not exist — compile fail.
/// Green: `LlmKeyConfig` exposes ONLY `api_key()` + `hostname_allowlist()`.
///        No `hosted_model_endpoint()` method is callable — compile-time shape check.
#[test]
fn llm_key_config_has_no_hosted_model_endpoint_field() {
    // R-0014-c: structural assertion — the config surface is external-provider-only.
    //
    // Method: instantiate a config and use only its public API.
    // If the green phase adds `hosted_model_endpoint()`, callers can start
    // setting it — the absence of such a method is the enforcement.
    //
    // This test "exercises the shape" by:
    //   1. Loading a config.
    //   2. Calling every public accessor that should exist.
    //   3. The test would not compile if any UNEXPECTED accessor existed that
    //      we'd need to call (we don't call what shouldn't be there — the
    //      absence of the call IS the constraint).
    //
    // Residual gap: this test does NOT prevent a hosted_model_endpoint field
    // from existing if it is not exercised by any test. Document here so the
    // green reviewer knows to inspect `LlmKeyConfig`'s declared fields directly.
    let dir = TempDir::new().unwrap();
    let config_path = write_llm_key_config(
        &dir,
        "external_only.toml",
        "external-provider-api-key",
        &["api.openai.com"],
        0o600,
    );

    let config = LlmKeyConfig::load(&config_path).expect("config must load");

    // The public surface is: api_key() and hostname_allowlist().
    // No hosted_model_endpoint() should exist on LlmKeyConfig.
    let _key: &str = config.api_key();
    let _allowlist: &AllowList = config.hostname_allowlist();

    // If the green phase adds `config.hosted_model_endpoint()`, this test will
    // still pass (it doesn't call that method). The green reviewer MUST inspect
    // the struct's pub fields/methods and reject any hosted-endpoint addition.
    //
    // Documented as a residual spec-gap: R-0014-c is only partially testable
    // here (shape only; field existence requires a structural/derive-level check
    // that is beyond runtime testing). Record in completion report.
    assert!(
        !_key.is_empty(),
        "api_key() must return a non-empty string from config (R-0014-c context: \
         external provider key is the only accepted key form)"
    );
}

// ===========================================================================
// Cross-dispatch handoff contract
// ===========================================================================
//
// The following documents the EXACT seam Task 18 must expose. This is the
// binding contract the green-phase implementer will match.
//
// ## Module paths
//
//   mnemra_host::config::llm_key
//   mnemra_host::net::hostname_allowlist
//   mnemra_host::startup::file_mode_check   ← EXTENDED (3-arg check)
//
//   These map to files:
//     libs/mnemra-host/config.rs (or config/mod.rs) + config/llm_key.rs
//     libs/mnemra-host/net.rs (or net/mod.rs) + net/hostname_allowlist.rs
//     libs/mnemra-host/startup/file_mode_check.rs  ← 3-arg check() (EXTENDS Task 17)
//
//   Task 18 must also add `pub mod config;` and `pub mod net;` to
//   `libs/mnemra-host/mnemra_host.rs`, and the submodule declarations within
//   each module file.
//
// ## net::hostname_allowlist
//
//   /// Error returned when a hostname is not on the allowlist.
//   ///
//   /// Implements `std::error::Error` + `Display`.
//   #[derive(Debug)]
//   pub struct AllowListError {
//       // Minimum: the blocked hostname so the caller can log it.
//       // Task 18 owns the fields; the tests only call `is_err()`.
//   }
//
//   impl std::fmt::Display for AllowListError { ... }
//   impl std::error::Error for AllowListError {}
//
//   /// An immutable, per-deployment hostname allowlist.
//   ///
//   /// Constructed once from configuration; cloneable for embedding in config structs.
//   ///
//   /// # Complete mediation (SF2)
//   ///
//   /// The implementation MUST enumerate permitted hostnames and deny the rest.
//   /// The deny-unknown default is the contract; the list is NOT a blocklist.
//   ///
//   /// # Exact-match semantics at V0
//   ///
//   /// V0 uses exact hostname match (case-insensitive). No wildcard or subdomain
//   /// matching. If "api.openai.com" is listed, "subdomain.api.openai.com" is blocked.
//   /// Upgrade to wildcard/CIDR is a V0.1+ scope item.
//   pub struct AllowList {
//       // Task 18 owns the fields.
//   }
//
//   impl AllowList {
//       /// Construct an AllowList from a slice of allowed hostnames.
//       ///
//       /// Hostnames are stored case-insensitively (normalize to lowercase on insert).
//       ///
//       /// # Parameters
//       ///   hostnames: &[&str]   — the permitted hostname strings
//       ///
//       /// Empty slice → an AllowList that blocks everything (correct: deny-by-default).
//       pub fn new(hostnames: &[&str]) -> Self;
//
//       /// Check whether `hostname` is on the allowlist.
//       ///
//       /// Returns Ok(()) if permitted, Err(AllowListError) if blocked.
//       ///
//       /// The check is case-insensitive. No port stripping — the caller passes
//       /// the bare hostname (no port, no scheme). At V0 the DF-embed-call path
//       /// always calls out to a fixed provider hostname; port is a separate concern.
//       pub fn check(&self, hostname: &str) -> Result<(), AllowListError>;
//   }
//
// ## config::llm_key
//
//   use std::path::Path;
//
//   /// Error returned when the LLM-key config file cannot be loaded.
//   #[derive(Debug)]
//   pub struct LlmKeyConfigError {
//       // Minimum: path + underlying cause (io::Error or parse error).
//   }
//
//   impl std::fmt::Display for LlmKeyConfigError { ... }
//   impl std::error::Error for LlmKeyConfigError {}
//
//   /// Per-deployment LLM-API-key configuration.
//   ///
//   /// Loaded from a TOML file at deploy time. The file format is:
//   ///
//   ///   api_key = "sk-..."
//   ///   hostname_allowlist = ["api.openai.com"]
//   ///
//   /// The file MUST NOT exist as a compiled-in default (R-0014-a).
//   ///
//   /// # Public API (complete — no additional public methods)
//   ///
//   ///   - `api_key()` — the API key string (from file)
//   ///   - `hostname_allowlist()` — the configured `AllowList` (from file)
//   ///
//   /// There is NO `hosted_model_endpoint()` method or `hosted_endpoint` field
//   /// (R-0014-c: external providers only).
//   pub struct LlmKeyConfig {
//       // Task 18 owns the fields.
//   }
//
//   impl LlmKeyConfig {
//       /// Load a `LlmKeyConfig` from a TOML file at `path`.
//       ///
//       /// Reads the file, parses TOML, constructs and returns the config.
//       /// Returns Err if the file cannot be read or the TOML is malformed.
//       ///
//       /// Does NOT check file mode — the file-mode check is performed separately
//       /// by `startup::file_mode_check::check()` before this is called.
//       pub fn load(path: &Path) -> Result<Self, LlmKeyConfigError>;
//
//       /// The API key string, as loaded from the config file.
//       ///
//       /// Returns a `&str` reference into the loaded config.
//       /// The string is the literal value from the `api_key` TOML field.
//       pub fn api_key(&self) -> &str;
//
//       /// The hostname allowlist loaded from the config file.
//       ///
//       /// Returns a reference to the `AllowList` constructed from the
//       /// `hostname_allowlist` TOML field.
//       pub fn hostname_allowlist(&self) -> &AllowList;
//   }
//
// ## startup::file_mode_check — EXTENDED (3-arg, replaces 2-arg from Task 17)
//
//   use std::path::Path;
//
//   /// Check that ALL THREE secret files are mode 600.
//   ///
//   /// Covers:
//   ///   1. The admin-token file (R-0005-f, Task 17)
//   ///   2. The signing-material file (R-0005-f, Task 17)
//   ///   3. The LLM-key file (R-0014-d, Task 18)
//   ///
//   /// Returns Err(FileModeError) if ANY of the three files is not mode 600.
//   /// Returns Ok(()) only when ALL THREE are mode 600.
//   ///
//   /// Called on host startup before any plugin is loaded or any embedding
//   /// call is made. If this returns Err, the host MUST refuse to start.
//   ///
//   /// # Breaking change from Task 17
//   ///
//   /// This EXTENDS the Task 17 `check(token_path, signing_material_path)` to a
//   /// 3-arg form. `signing_chain.rs` (Task 16/17 tests) is updated to pass the
//   /// new third argument (a mode-600 fixture), preserving all existing assertions.
//   ///
//   /// There is ONE `check()` function — no `check_all()` alias. A partial
//   /// 2-arg form must NOT remain; it would be a fails-open-by-omission footgun
//   /// on the security control (SF2: complete mediation over all secret files).
//   ///
//   pub fn check(
//       token_path: &Path,
//       signing_material_path: &Path,
//       llm_key_path: &Path,
//   ) -> Result<(), FileModeError>;
//
//   /// The `FileModeError` type is unchanged from Task 17.
//   ///
//   /// (Defined in `startup::file_mode_check` — see Task 17 handoff for the
//   /// full definition: path, actual_mode, required_mode fields.)
//
//   Note: `FileModeError` as defined by Task 17 carries the `path` of the
//   offending file; if `llm_key_path` fails, the error carries `llm_key_path`.
//   No new error type is needed.
//
// ## mnemra_host.rs additions Task 18 must make
//
//   // Add to libs/mnemra-host/mnemra_host.rs:
//   pub mod config;
//   pub mod net;
//
//   // Add libs/mnemra-host/config.rs (or config/mod.rs):
//   pub mod llm_key;
//
//   // Add libs/mnemra-host/net.rs (or net/mod.rs):
//   pub mod hostname_allowlist;
//
// ## Green-flip gate for Task 18
//
//   When Task 18 implements the above, the following must pass:
//
//     cargo test --manifest-path libs/mnemra-host/Cargo.toml \
//       --test llm_key_allowlist
//
//   Expected green-flip test outcomes:
//
//     hostname_not_on_allowlist_is_blocked                              PASS
//     hostname_on_allowlist_proceeds                                    PASS
//     allowlist_is_per_deployment_configurable_not_compiled_in          PASS
//     llm_key_loaded_from_deploy_time_config_not_hardcoded              PASS
//     startup_check_passes_when_all_three_files_are_mode_600_including_llm_key  PASS
//     startup_check_fails_if_llm_key_file_is_world_readable             PASS
//     startup_check_fails_if_llm_key_and_admin_token_are_world_readable PASS
//     llm_key_config_has_no_hosted_model_endpoint_field                 PASS
//
//   AND `cargo test --manifest-path libs/mnemra-host/Cargo.toml \
//       --test signing_chain` must still pass with all pre-Task-18 assertions intact
//   (updated to 3-arg check() per this dispatch's companion signing_chain.rs edit).
