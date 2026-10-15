//! Per-plugin host-function allowlist enforcement through the PRODUCTION load
//! path (P-0003, R-0003-b; brain #3842).
//!
//! # What this pins
//!
//! `populate_verified_pool_from_dir` is what `run()` calls at startup. These
//! tests drive THAT function, with the committed signed echo component and a
//! manifest signed by a per-run test key, so the only variable is what the
//! signed manifest declares in `[host_fns]`:
//!
//! - every host function the component imports is declared  -> loads (control);
//! - any one of them omitted from the signed manifest       -> load refused,
//!   naming the undeclared function, no pool returned;
//! - a `[host_fns]` table placed AFTER `[signature]` (unsigned bytes)
//!   cannot grant anything;
//! - an unknown key under `[host_fns]` fails the load instead of silently
//!   forking the grant;
//! - the shipped manifest still verifies against the embedded root, which is
//!   the check `MnemraMcpServer::new` applies to the embedded verb-gate copy.
//!
//! # Why a load-time refusal satisfies "cannot call it"
//!
//! The component imports each host function by name. The linker refuses to
//! build when an import is undeclared, so no instance exists in which the call
//! could be made. The seeded-failure proof for these tests is removing that
//! check: the omitted-function cases then load `Ok` and every `is_err` assert
//! below fails.

use std::path::{Path, PathBuf};

use ed25519_dalek::{Signer, SigningKey};
use mnemra_host::mcp::server::{ECHO_PLUGIN_NAME, MnemraMcpServer};
use mnemra_host::plugin::pool::PluginPool;
use mnemra_host::plugin::runtime::PluginRuntime;
use mnemra_host::signing::root_material::ROOT;
use mnemra_host::signing::verify::verify_plugin;
use mnemra_host::startup::{StartupError, populate_verified_pool_from_dir};
use rand::TryRng;

/// The five host functions the committed echo component imports
/// (`wasm-tools component wit artifacts/mnemra-echo/mnemra_echo.wasm`).
const ECHO_IMPORTED_HOST_FNS: [&str; 5] = [
    "artifact.create",
    "artifact.get",
    "artifact.list",
    "artifact.update",
    "artifact.delete",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root from libs/mnemra-host")
        .to_path_buf()
}

fn generate_keypair() -> SigningKey {
    let mut seed = [0u8; 32];
    rand::rngs::SysRng
        .try_fill_bytes(&mut seed)
        .expect("SysRng fill failed");
    SigningKey::from_bytes(&seed)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn committed_component() -> Vec<u8> {
    std::fs::read(repo_root().join("artifacts/mnemra-echo/mnemra_echo.wasm"))
        .expect("committed echo component must exist")
}

/// Signed body: `[plugin]`, `[verbs]`, optional `[host_fns]` (verbatim text),
/// `[component]` bound to the committed artifact.
fn signed_body(host_fns_table: &str, component: &[u8]) -> String {
    signed_body_with_verbs(&["echo.create"], host_fns_table, component)
}

fn signed_body_with_verbs(verbs: &[&str], host_fns_table: &str, component: &[u8]) -> String {
    let hash = blake3::hash(component).to_hex();
    let verbs = verbs
        .iter()
        .map(|v| format!("\"{v}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "[plugin]\nname = \"mnemra-echo\"\nversion = \"0.0.1\"\nschema_version = 1\ncore = true\n\n\
         [verbs]\nexposed = [{verbs}]\n\n{host_fns_table}\n\
         [component]\nhash_alg = \"blake3\"\nhash = \"{hash}\"\n"
    )
}

fn host_fns_table(declared: &[&str]) -> String {
    let list = declared
        .iter()
        .map(|f| format!("\"{f}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[host_fns]\nrequired = [{list}]\noptional = []\n")
}

/// Sign `body`, append `[signature]` and then `unsigned_tail` (bytes the
/// signature does not cover).
fn manifest(key: &SigningKey, body: &str, unsigned_tail: &str) -> Vec<u8> {
    let sig = key.sign(body.as_bytes()).to_bytes();
    format!(
        "{body}\n[signature]\nalgorithm = \"ed25519\"\npublic_key = \"{}\"\nsig_bytes = \"{}\"\n\
         signed_at = \"2026-10-15T00:00:00Z\"\n{unsigned_tail}",
        hex_encode(key.verifying_key().as_bytes()),
        hex_encode(&sig),
    )
    .into_bytes()
}

/// Lay out `<root>/plugins/mnemra-echo/manifest.toml` and
/// `<root>/artifacts/mnemra-echo/mnemra_echo.wasm` and run the production
/// disk-driven load against it.
fn load_through_production_path(
    manifest_bytes: &[u8],
    component: &[u8],
    root_key: &SigningKey,
) -> Result<(), StartupError> {
    load_pool_through_production_path(manifest_bytes, component, root_key).map(|_| ())
}

fn load_pool_through_production_path(
    manifest_bytes: &[u8],
    component: &[u8],
    root_key: &SigningKey,
) -> Result<std::sync::Arc<PluginPool>, StartupError> {
    let dir = tempfile::tempdir().expect("tempdir");
    let plugins = dir.path().join("plugins/mnemra-echo");
    let artifacts = dir.path().join("artifacts/mnemra-echo");
    std::fs::create_dir_all(&plugins).expect("mkdir plugins");
    std::fs::create_dir_all(&artifacts).expect("mkdir artifacts");
    std::fs::write(plugins.join("manifest.toml"), manifest_bytes).expect("write manifest");
    std::fs::write(artifacts.join("mnemra_echo.wasm"), component).expect("write component");
    populate_verified_pool_from_dir(dir.path(), &root_key.verifying_key().to_bytes())
}

/// Positive control: declaring everything the component imports loads. Without
/// it, the refusals below could be a blanket failure of the fixture.
#[test]
fn fully_declared_host_fns_load() {
    let key = generate_keypair();
    let component = committed_component();
    let body = signed_body(&host_fns_table(&ECHO_IMPORTED_HOST_FNS), &component);
    let m = manifest(&key, &body, "");

    let result = load_through_production_path(&m, &component, &key);

    assert!(
        result.is_ok(),
        "control: a manifest declaring every imported host fn must load; got {:?}",
        result.err().map(|e| e.to_string())
    );
}

/// Property over the import set: omitting ANY ONE imported host function from
/// the signed manifest refuses the load, and the error names that function.
#[test]
fn omitting_any_imported_host_fn_refuses_the_load() {
    let key = generate_keypair();
    let component = committed_component();

    for omitted in ECHO_IMPORTED_HOST_FNS {
        let declared: Vec<&str> = ECHO_IMPORTED_HOST_FNS
            .iter()
            .copied()
            .filter(|f| *f != omitted)
            .collect();
        let body = signed_body(&host_fns_table(&declared), &component);
        let m = manifest(&key, &body, "");

        let err = load_through_production_path(&m, &component, &key)
            .expect_err(&format!("manifest omitting '{omitted}' must be refused"));

        assert!(
            matches!(err, StartupError::UndeclaredHostFn(_)),
            "undeclared import is refused at link time with its own variant; got {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains(&format!("'{omitted}'")) && msg.contains("does not declare"),
            "error must name the undeclared host fn '{omitted}'; got: {msg}"
        );
    }
}

/// No `[host_fns]` table at all is the empty allowlist: every import is
/// undeclared, so the load fails closed.
#[test]
fn absent_host_fns_table_denies_everything() {
    let key = generate_keypair();
    let component = committed_component();
    let body = signed_body("", &component);
    let m = manifest(&key, &body, "");

    let result = load_through_production_path(&m, &component, &key);

    assert!(
        matches!(result, Err(StartupError::UndeclaredHostFn(_))),
        "absent [host_fns] must deny every host fn; got {:?}",
        result.err().map(|e| e.to_string())
    );
}

/// Bytes after `[signature]` are not covered by it. A `[host_fns]` table
/// appended there, with the signed body declaring none, must grant nothing.
#[test]
fn host_fns_in_the_unsigned_tail_grant_nothing() {
    let key = generate_keypair();
    let component = committed_component();
    let body = signed_body("", &component);
    let tail = host_fns_table(&ECHO_IMPORTED_HOST_FNS);
    let m = manifest(&key, &body, &tail);

    let result = load_through_production_path(&m, &component, &key);

    assert!(
        matches!(result, Err(StartupError::UndeclaredHostFn(_))),
        "an unsigned [host_fns] tail must not widen the allowlist; got {:?}",
        result.err().map(|e| e.to_string())
    );
}

/// A misspelt key in the grant table fails the load rather than silently
/// producing a different grant.
#[test]
fn unknown_key_in_host_fns_fails_the_load() {
    let key = generate_keypair();
    let component = committed_component();
    let table = "[host_fns]\nrequried = [\"artifact.create\"]\n";
    let body = signed_body(table, &component);
    let m = manifest(&key, &body, "");

    let result = load_through_production_path(&m, &component, &key);

    assert!(
        matches!(result, Err(StartupError::ManifestRejected(_))),
        "an unknown [host_fns] key must be rejected; got {:?}",
        result.err().map(|e| e.to_string())
    );
}

/// The shipped echo manifest, signed by the ceremony root, verifies against the
/// embedded root, declares every host fn the committed component imports, and
/// loads through the production path. `MnemraMcpServer::new` applies the same
/// `verify_plugin(.., ROOT)` to its embedded copy before reading the verb list.
#[test]
fn shipped_manifest_verifies_declares_every_import_and_loads() {
    let bytes = std::fs::read(repo_root().join("plugins/mnemra-echo/manifest.toml"))
        .expect("shipped manifest");

    verify_plugin(&bytes, ROOT).expect("shipped manifest must verify against the embedded root");

    let runtime = PluginRuntime::load(&bytes, ROOT).expect("shipped manifest must load");
    for f in ECHO_IMPORTED_HOST_FNS {
        assert!(
            runtime.is_host_fn_allowed(f),
            "shipped manifest must declare '{f}'"
        );
    }

    let loaded = populate_verified_pool_from_dir(&repo_root(), ROOT);
    assert!(
        loaded.is_ok(),
        "the real signed echo plugin must load through the production path; got {:?}",
        loaded.err().map(|e| e.to_string())
    );
}

/// Tripwire for the link-time check's scope. `build_linker` mediates only
/// imports under `mnemra:host/`; imports elsewhere are inert trap-stubs. That is
/// sound exactly while the `plugin` world imports nothing outside that package.
/// If a future world import lands in another namespace (real WASI, say), this
/// fails and the check's namespace scope must be widened in the same change.
#[test]
fn plugin_world_imports_only_mnemra_host_interfaces() {
    let mut resolve = wit_parser::Resolve::new();
    resolve.all_features = true;
    let (pkg, _) = resolve
        .push_path(repo_root().join("wit"))
        .expect("wit/ must parse");
    let world_id = resolve
        .select_world(&[pkg], Some("plugin"))
        .expect("`plugin` world must exist");
    let world = &resolve.worlds[world_id];

    assert!(!world.imports.is_empty(), "control: the world has imports");
    for (key, item) in &world.imports {
        let wit_parser::WorldItem::Interface { id, .. } = item else {
            panic!("non-interface world import {key:?}: widen build_linker's scope");
        };
        let owner = resolve.interfaces[*id]
            .package
            .map(|p| &resolve.packages[p].name);
        assert!(
            owner.is_some_and(|n| n.namespace == "mnemra" && n.name == "host"),
            "world import {key:?} is outside `mnemra:host`: widen build_linker's scope"
        );
    }
}

/// The MCP verb gate's source is the pool's VERIFIED grants (C2): the verbs a
/// signed manifest exposes, and only those, are exposed by the pool.
#[test]
fn pool_verb_gate_reads_the_signed_manifest() {
    let key = generate_keypair();
    let component = committed_component();
    let body = signed_body_with_verbs(
        &["echo.get", "echo.list"],
        &host_fns_table(&ECHO_IMPORTED_HOST_FNS),
        &component,
    );
    let m = manifest(&key, &body, "");

    let pool = load_pool_through_production_path(&m, &component, &key)
        .expect("a fully declared manifest must load");

    assert_eq!(
        pool.verified_verbs(ECHO_PLUGIN_NAME),
        vec!["echo.get".to_owned(), "echo.list".to_owned()]
    );
    assert!(pool.is_verb_exposed(ECHO_PLUGIN_NAME, "echo.get"));
    assert!(!pool.is_verb_exposed(ECHO_PLUGIN_NAME, "echo.create"));
    assert!(
        !pool.is_verb_exposed("not-registered", "echo.get"),
        "an unregistered plugin exposes nothing"
    );
}

/// The server's advertised verbs come from the verified runtime, not from any
/// embedded copy of the manifest. The shipped manifest exposes `echo.create`;
/// this signed one does not, so a server that read the embedded file would
/// advertise it. `tools/list` never touches the database, hence the lazy pool.
#[tokio::test]
async fn server_advertises_only_the_verified_manifests_verbs() {
    let key = generate_keypair();
    let component = committed_component();
    let body = signed_body_with_verbs(
        &["echo.get", "echo.list"],
        &host_fns_table(&ECHO_IMPORTED_HOST_FNS),
        &component,
    );
    let m = manifest(&key, &body, "");
    let pool = load_pool_through_production_path(&m, &component, &key)
        .expect("a fully declared manifest must load");

    let pg = sqlx::PgPool::connect_lazy("postgres://unused@127.0.0.1:1/unused")
        .expect("lazy pool construction does not connect");
    let server = MnemraMcpServer::new(pg, pool);
    let (server_transport, client_transport) = tokio::io::duplex(4096);
    tokio::spawn(async move {
        if let Ok(running) = rmcp::service::serve_server(server, server_transport).await {
            let _ = running.waiting().await;
        }
    });
    let client = rmcp::service::serve_client((), client_transport)
        .await
        .expect("client init");

    let tools = client.list_all_tools().await.expect("tools/list");
    let mut echo_tools: Vec<String> = tools
        .iter()
        .map(|t| t.name.to_string())
        .filter(|n| n.starts_with("echo."))
        .collect();
    echo_tools.sort();

    assert_eq!(
        echo_tools,
        vec!["echo.get".to_owned(), "echo.list".to_owned()]
    );
}
