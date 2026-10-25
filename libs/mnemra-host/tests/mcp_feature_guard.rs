//! R-0010-e feature guard — no streamable-HTTP transport on rmcp.
//!
//! # Purpose
//!
//! R-0010-e: Streamable-HTTP MCP transport SHALL NOT be activated at V0.
//! This file asserts that the rmcp dependency's resolved feature set is
//! EXACTLY the pinned allowlist, so no http/streamable-http transport (nor any
//! other unreviewed capability such as `auth` or `reqwest`) can be enabled.
//!
//! The allowlist supersedes the earlier denylist of five streamable-HTTP
//! feature names, which failed open on every feature it did not name
//! (control-integrity CI2). The test also fails closed when it cannot
//! evaluate: a missing `features` field, no rmcp package, or a failing
//! `cargo` invocation is a failure, never a pass (CI1), and every package
//! named rmcp in the graph is checked, not just the first (CI3).
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
//! correct feature set** (`server`, `transport-io`, `client` for tests —
//! no `http`/`streamable-http` features; see the allowlist consts below).
//! This is a standing guardrail, not one of the four behavioral reds.
//! Documented per the dispatch note.
//!
//! # Gate membership
//!
//! This binary is listed in `NONPG_TEST_FLAGS` in the justfile, so `just ci`
//! runs it. It needs no embedded Postgres; it shells out to `cargo tree` and
//! `cargo metadata` (with `--locked --all-features`, network allowed).

use std::collections::BTreeSet;
use std::process::Command;

/// rmcp features resolved for a normal (non-dev) build: the workspace entry
/// `features = ["server", "transport-io"]` plus what those two imply in rmcp 3.7
/// (`server` -> transport-async-rw, schemars, uuid; `transport-io` ->
/// transport-async-rw). `dep:` edges do not appear as feature names.
const NORMAL_FEATURES: &[&str] = &[
    "schemars",
    "server",
    "transport-async-rw",
    "transport-io",
    "uuid",
];

/// The same set plus `client`, which `libs/mnemra-host`'s dev-dependency adds.
/// `cargo metadata` cannot separate dev from normal, so it is compared to this.
const WITH_DEV_FEATURES: &[&str] = &[
    "client",
    "schemars",
    "server",
    "transport-async-rw",
    "transport-io",
    "uuid",
];

/// Run `cargo <args>` against this workspace. `--locked` never re-resolves
/// (the lockfile pins checksums). `--offline` is deliberately absent: `--target
/// all` and unfiltered metadata need every platform's crates, so a cache filled
/// by a host-only fetch would give a false red. `--all-features` makes a
/// non-default mnemra-host feature that enables an rmcp transport visible.
/// Any failure to run is a test failure (fail closed).
fn cargo(args: &[&str]) -> String {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let output = Command::new("cargo")
        .args(args)
        .args(["--locked", "--all-features", "--manifest-path"])
        .arg(manifest)
        .output()
        .unwrap_or_else(|e| panic!("cargo {args:?} must run: {e}"));
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("download") || stderr.contains("--offline") {
            panic!(
                "APPARATUS FAILURE (network or registry), not an R-0010-e violation: \
                 cargo {args:?} could not fetch crates. Run `cargo fetch --locked` and retry. \
                 cargo said: {stderr}"
            );
        }
        panic!("cargo {args:?} failed: {stderr}");
    }
    String::from_utf8(output.stdout).expect("cargo output must be UTF-8")
}

/// Assert `actual` equals `expected`, naming every extra and missing feature.
fn assert_exact(context: &str, actual: &BTreeSet<String>, expected: &[&str]) {
    let expected: BTreeSet<String> = expected.iter().map(|s| (*s).to_string()).collect();
    let extra: Vec<&String> = actual.difference(&expected).collect();
    let missing: Vec<&String> = expected.difference(actual).collect();
    assert!(
        extra.is_empty() && missing.is_empty(),
        "R-0010-e: {context}: rmcp feature set differs from the allowlist. \
         Unexpected (enabled but not allowed): {extra:?}. \
         Missing (allowed but not enabled): {missing:?}. \
         Actual: {actual:?}"
    );
}

/// R-0010-e — assert rmcp's resolved features are exactly the allowlist.
///
/// The rmcp dependency is configured as:
///   workspace: `rmcp = { version = "3.7", features = ["server", "transport-io"], default-features = false }`
///   dev-dep extra: `rmcp = { workspace = true, features = ["client"] }`
///
/// Two views, because `cargo metadata` unifies dev-dependency features:
///   - `cargo tree -e no-dev` gives the normal-build set, which must equal
///     [`NORMAL_FEATURES`] (this is what ships);
///   - `cargo metadata` `resolve.nodes[].features` gives the set with
///     dev-dependencies, which must equal [`WITH_DEV_FEATURES`].
///
/// Several rmcp versions in the graph (e.g. mid-upgrade) are each checked
/// against the same allowlist: a version whose feature names differ fails
/// loudly and forces a reviewed update of the consts, which is the point.
#[test]
fn rmcp_feature_set_is_exactly_the_allowlist() {
    // Normal-build view. `{p}` is "name vX.Y.Z[ (path)]", `{f}` the comma-joined
    // resolved features; repeats carry a trailing " (*)".
    let tree = cargo(&[
        "tree",
        "--workspace",
        "-e",
        "no-dev",
        "--target",
        "all",
        "--prefix",
        "none",
        "-f",
        "{p}|{f}",
    ]);
    let mut normal_nodes = 0;
    // `--workspace` prints one tree per member, separated by blank lines; any
    // other line without the `|` separator is an unrecognised shape and fails.
    for line in tree.lines().filter(|l| !l.is_empty()) {
        let (pkg, features) = line
            .split_once('|')
            .unwrap_or_else(|| panic!("unparseable cargo tree line: {line:?}"));
        if pkg.split_whitespace().next() != Some("rmcp") {
            continue;
        }
        normal_nodes += 1;
        let features: BTreeSet<String> = features
            .trim_end_matches(" (*)")
            .split(',')
            .filter(|f| !f.is_empty())
            .map(str::to_string)
            .collect();
        assert_exact(&format!("normal build, {pkg}"), &features, NORMAL_FEATURES);
    }
    assert!(
        normal_nodes > 0,
        "rmcp must appear in the normal-build dependency tree"
    );

    // With-dev-dependencies view.
    let meta: serde_json::Value =
        serde_json::from_str(&cargo(&["metadata", "--format-version", "1"]))
            .expect("cargo metadata output must be valid JSON");
    let rmcp_ids: BTreeSet<&str> = meta["packages"]
        .as_array()
        .expect("packages must be an array")
        .iter()
        .filter(|pkg| pkg["name"].as_str() == Some("rmcp"))
        .map(|pkg| pkg["id"].as_str().expect("package id must be a string"))
        .collect();
    assert!(
        !rmcp_ids.is_empty(),
        "rmcp must appear in cargo metadata packages"
    );

    let nodes = meta["resolve"]["nodes"]
        .as_array()
        .expect("resolve.nodes must be an array");
    for id in &rmcp_ids {
        let node = nodes
            .iter()
            .find(|n| n["id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("{id} must appear in resolve.nodes"));
        // A missing or non-array `features` is a failure, never an empty list.
        let features: BTreeSet<String> = node["features"]
            .as_array()
            .unwrap_or_else(|| panic!("{id}: resolve node has no `features` array"))
            .iter()
            .map(|v| {
                v.as_str()
                    .unwrap_or_else(|| panic!("{id}: non-string feature {v:?}"))
                    .to_string()
            })
            .collect();
        assert_exact(
            &format!("with dev-dependencies, {id}"),
            &features,
            WITH_DEV_FEATURES,
        );
    }
}
