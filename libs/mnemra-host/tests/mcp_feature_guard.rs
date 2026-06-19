//! R-0010-e feature guard — no streamable-HTTP transport on rmcp.
//!
//! # Purpose
//!
//! R-0010-e: Streamable-HTTP MCP transport SHALL NOT be activated at V0.
//! This file asserts that the rmcp dependency's enabled feature set includes
//! none of the forbidden http/streamable-http transport features.
//!
//! # Why a separate file
//!
//! `mcp_server.rs` references `mnemra_host::mcp::*`, which does not exist yet
//! and will fail to compile. If this test shared that binary, it could never be
//! independently green. Keeping the feature guard in its own integration-test
//! binary lets it compile and pass while `mcp_server.rs` is red.
//!
//! # Red/green status
//!
//! R-0010-e guard: **expected to be GREEN from the moment rmcp is added with the
//! correct feature set** (`server`, `macros`, `transport-io`, `client` for tests —
//! no `http`/`streamable-http` features). This is a standing guardrail, not one
//! of the four behavioral reds. Documented per the dispatch note.
//!
//! # verify: []
//!
//! `verify: []` — no just recipe yet; added in green phase.

/// R-0010-e — assert rmcp's activated features contain no http/streamable-http transport.
///
/// The rmcp dependency is configured as:
///   workspace: `rmcp = { version = "1.7", features = ["server", "macros", "transport-io"] }`
///   dev-dep extra: `rmcp = { workspace = true, features = ["client"] }`
///
/// Feature breakdown (from rmcp 1.7 Cargo.toml):
///   - `server` enables: transport-async-rw, schemars, pastey
///   - `macros` enables: rmcp-macros, pastey
///   - `transport-io` enables: transport-async-rw, tokio/io-std
///   - `client` enables: tokio-stream
///
/// Features that MUST NOT appear (R-0010-e):
///   - transport-streamable-http-server (enables server-side HTTP MCP)
///   - transport-streamable-http-client (enables client-side streamable HTTP)
///   - transport-streamable-http-server-session
///   - server-side-http
///   - transport-streamable-http-client-reqwest
///
/// Strategy: parse `cargo metadata` output for the rmcp node, extract its
/// activated features, and assert none of the forbidden names are present.
#[test]
fn rmcp_no_streamable_http_features() {
    // Locate the workspace Cargo.toml from CARGO_MANIFEST_DIR (this crate's dir).
    // Walk up to find the workspace root.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(manifest_dir.join("Cargo.toml"))
        .output()
        .expect("cargo metadata must run");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("cargo metadata failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let meta: serde_json::Value =
        serde_json::from_str(&stdout).expect("cargo metadata output must be valid JSON");

    // Find the rmcp package entry in metadata.
    let packages = meta["packages"]
        .as_array()
        .expect("packages must be an array");

    // Find rmcp's package id from the packages list first.
    let rmcp_id = packages
        .iter()
        .find(|pkg| pkg["name"].as_str() == Some("rmcp"))
        .and_then(|pkg| pkg["id"].as_str())
        .expect("rmcp must appear in cargo metadata packages (added in this dispatch)");

    // The activated features are in `resolve.nodes`, not `packages[].features`.
    // `packages[].features` lists ALL declared features; `resolve.nodes[].features`
    // lists only the features actually enabled for this compilation.
    let resolve_nodes = meta["resolve"]["nodes"]
        .as_array()
        .expect("resolve.nodes must be an array");

    let rmcp_node = resolve_nodes
        .iter()
        .find(|node| node["id"].as_str() == Some(rmcp_id))
        .expect("rmcp must appear in resolve.nodes");

    // Extract the activated features for this rmcp instance.
    let features: Vec<&str> = rmcp_node["features"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    // R-0010-e: none of these transport features may be activated.
    let forbidden = [
        "transport-streamable-http-server",
        "transport-streamable-http-client",
        "transport-streamable-http-server-session",
        "server-side-http",
        "transport-streamable-http-client-reqwest",
    ];

    for f in &forbidden {
        assert!(
            !features.contains(f),
            "R-0010-e: rmcp activated feature set MUST NOT include '{}'; \
             streamable-HTTP transport is forbidden at V0. \
             All rmcp features in metadata: {:?}",
            f,
            features
        );
    }
}
