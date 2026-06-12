//! ABI contract tests for the mnemra host-fn WIT interface.
//!
//! RED PHASE — all tests in this file MUST fail until Task 4 authors
//! `wit/host.wit` containing the required host-fn interfaces.
//!
//! verify: [] by design (red phase); the failing state here is NOT a
//! regression — it is the red-phase invariant that Task 4 is expected to make
//! green.
//!
//! Spec requirements traced in each test:
//!   R-0003-d  — no `workspace-id` param on write-path host-fns
//!   R-0003-g  — `artifact-delete` must be explicitly declared (opt-in)
//!   R-0006-a  — every host-fn takes `WorkspaceCtx` as its first parameter
//!   R-0012-a  — required host-fn set
//!   R-0012-b  — `sampling-request` is optional; `context-ids` not bodies
//!   R-0012-c  — `secrets-get` is optional; no write path to secrets store
//!   R-0012-d  — structural: no write-path fn exposes `workspace-id`
//!   R-0012-e  — stability annotations present; behavioural stubs ignored
//!   R-0012-f  — no raw `list<u8>` byte-buffer params/returns
//!
//! # Naming conventions
//!
//! The API Contract table uses dotted verb notation (`artifact.create`).
//! WIT requires kebab-case identifiers. The mapping is:
//!
//!   artifact.create   → `artifact-create`   (in interface `artifact`)
//!   artifact.update   → `artifact-update`
//!   artifact.get      → `artifact-get`
//!   artifact.list     → `artifact-list`
//!   artifact.delete   → `artifact-delete`
//!   metrics.record    → `metrics-record`     (in interface `metrics`)
//!   log.emit          → `log-emit`           (in interface `log`)
//!   event.emit        → `event-emit`         (in interface `event`)
//!   projection.emit   → `projection-emit`    (in interface `projection`)
//!   sampling.request  → `sampling-request`   (in interface `sampling`)
//!   secrets.get       → `secrets-get`        (in interface `secrets`)
//!
//! The host-fn ABI groups functions by interface name (the part before the
//! dot).  Task 4 must declare these as separate named WIT interfaces inside
//! `wit/host.wit` under the `mnemra:host` package.
//!
//! # WorkspaceCtx first-parameter convention
//!
//! R-0006-a requires every host-fn to take a `WorkspaceCtx` as its FIRST
//! parameter.  In WIT this means the first entry of `Function.params` has
//! `name == "ctx"` and its type resolves to the `workspace-ctx` record
//! (or resource) defined within the package.  The exact WIT type name Task 4
//! must use is `workspace-ctx` (kebab-case).  These tests assert:
//!   - first param name is `"ctx"`
//!   - first param type is a named type (Type::Id) resolvable as
//!     `workspace-ctx` in the same package
//!
//! # Stability annotation convention
//!
//! WIT does not have a literal `@stable` token.  The spec's `@stable` maps
//! to `@since(version = 0.1.0)` in WIT syntax, which the parser represents as
//! `Stability::Stable { since: Version { .. } }`.  The spec's `@unstable`
//! maps to `@unstable(feature = <feature-name>)`, represented as
//! `Stability::Unstable { feature: _ }`.
//!
//! These tests assert that EVERY function in EVERY host interface carries a
//! stability annotation that is NOT `Stability::Unknown`.  Task 4 must
//! annotate all functions with either `@since(version = 0.1.0)` or
//! `@unstable(feature = <name>)`.
//!
//! Note: `@unstable` functions are filtered out by the parser unless
//! `Resolve::all_features = true`.  All tests in this file set
//! `all_features = true` so that the full contract surface is visible.
//!
//! # Behavioural stubs (R-0012-e runtime half)
//!
//! The runtime dispatch wrapper (unstable→log warn; deprecated→structured
//! error) does not exist until Task 4 lands the binding skeleton.  The
//! behavioural tests are marked `#[ignore]` with a comment naming their
//! filler task.  The data-level annotation assertions above are the testable
//! part of R-0012-e at this phase.

use mnemra_host::abi::{
    DispatchError, DispatchWarning, DispatchWrapper, Stability as AbiStability,
};
use std::cell::Cell;
use std::path::Path;
use wit_parser::{Resolve, Stability, Type};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Absolute path to the wit/ directory.  Tests parse this directory; the
/// expected `host.wit` file does not exist until Task 4.
fn wit_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../wit"))
}

/// Load the wit/ directory into a `Resolve` with all features enabled so that
/// `@unstable`-annotated items are visible.
///
/// Returns `Err` if `wit/host.wit` is absent or unparseable — which is the
/// expected red-phase failure mode.
fn load_resolve() -> anyhow::Result<(Resolve, wit_parser::PackageId)> {
    let mut resolve = Resolve::new();
    resolve.all_features = true;
    let (pkg_id, _) = resolve.push_path(wit_dir())?;
    Ok((resolve, pkg_id))
}

/// Look up an interface by name in a given package.  Returns `None` if the
/// interface is absent — which is the expected red-phase failure for any
/// interface that does not exist yet.
fn find_interface<'r>(
    resolve: &'r Resolve,
    pkg_id: wit_parser::PackageId,
    iface_name: &str,
) -> Option<&'r wit_parser::Interface> {
    let pkg = &resolve.packages[pkg_id];
    let iface_id = pkg.interfaces.get(iface_name)?;
    Some(&resolve.interfaces[*iface_id])
}

/// Returns true if the given `Type` resolves to a named typedef whose name
/// matches `expected_name` within the `Resolve`.
fn type_is_named(resolve: &Resolve, ty: &Type, expected_name: &str) -> bool {
    match ty {
        Type::Id(id) => {
            let td = &resolve.types[*id];
            td.name.as_deref() == Some(expected_name)
        }
        _ => false,
    }
}

/// Returns true if the given `Type` is or transitively contains `list<u8>`
/// (a raw byte buffer).  This walks one level of typedef aliases.
fn is_raw_byte_buffer(resolve: &Resolve, ty: &Type) -> bool {
    match ty {
        Type::U8 => false,
        Type::Id(id) => {
            let td = &resolve.types[*id];
            match &td.kind {
                wit_parser::TypeDefKind::List(inner) => {
                    matches!(inner, Type::U8)
                }
                wit_parser::TypeDefKind::Type(alias) => is_raw_byte_buffer(resolve, alias),
                _ => false,
            }
        }
        _ => false,
    }
}

/// Collect all write-path interface names.  These are the interfaces where
/// workspace-id MUST NOT appear as a parameter (R-0012-d, R-0003-d).
/// Read-only interfaces (`artifact-get`, `artifact-list`) are on the list too
/// because they are still host-fn interfaces that must not leak workspace_id.
fn write_path_interface_names() -> &'static [&'static str] {
    &[
        "artifact",
        "metrics",
        "log",
        "event",
        "projection",
        "sampling",
        "secrets",
    ]
}

/// Assert that a function's first parameter is named `"ctx"` and resolves to
/// the `workspace-ctx` type (R-0006-a).
fn assert_ctx_first(resolve: &Resolve, fn_name: &str, func: &wit_parser::Function) {
    let first = func.params.first().unwrap_or_else(|| {
        panic!(
            "fn `{}` has no parameters — expected WorkspaceCtx first (R-0006-a)",
            fn_name
        )
    });
    assert_eq!(
        first.name, "ctx",
        "fn `{}`: first param must be named `ctx`, got `{}` (R-0006-a)",
        fn_name, first.name
    );
    assert!(
        type_is_named(resolve, &first.ty, "workspace-ctx"),
        "fn `{}`: first param `ctx` must have type `workspace-ctx`, got {:?} (R-0006-a)",
        fn_name,
        first.ty
    );
}

/// Assert that a function's parameter list contains no parameter named
/// `workspace-id` (R-0012-d, R-0003-d).
fn assert_no_workspace_id_param(fn_name: &str, func: &wit_parser::Function) {
    for param in &func.params {
        assert_ne!(
            param.name, "workspace-id",
            "fn `{}` exposes `workspace-id` as a parameter — \
             violates R-0012-d / R-0003-d (Cross-workspace SQL leak)",
            fn_name
        );
    }
}

/// Assert that no parameter or return type in the function is `list<u8>`
/// (R-0012-f).
fn assert_no_raw_byte_buffer(resolve: &Resolve, fn_name: &str, func: &wit_parser::Function) {
    for param in &func.params {
        assert!(
            !is_raw_byte_buffer(resolve, &param.ty),
            "fn `{}` param `{}` uses raw list<u8> byte buffer — violates R-0012-f",
            fn_name,
            param.name
        );
    }
    if let Some(ret) = &func.result {
        assert!(
            !is_raw_byte_buffer(resolve, ret),
            "fn `{}` return type uses raw list<u8> byte buffer — violates R-0012-f",
            fn_name
        );
    }
}

/// Assert that a function's stability is NOT `Stability::Unknown` (R-0012-e).
fn assert_stability_annotated(fn_name: &str, func: &wit_parser::Function) {
    assert!(
        !matches!(func.stability, Stability::Unknown),
        "fn `{}` has no stability annotation (`Stability::Unknown`) — \
         every host-fn must carry @since(version=…) or @unstable(feature=…) (R-0012-e)",
        fn_name
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The `mnemra:host` package must contain an `artifact` interface.
///
/// Red: fails because `wit/host.wit` does not exist.
///
/// R-0012-a
#[test]
fn host_interface_artifact_exists() {
    let (resolve, pkg_id) =
        load_resolve().expect("wit/ directory must parse (echo.wit already parses)");
    assert!(
        find_interface(&resolve, pkg_id, "artifact").is_some(),
        "package `mnemra:host` must export an `artifact` interface (R-0012-a)"
    );
}

/// The `mnemra:host` package must contain a `metrics` interface.
///
/// Red: fails because `wit/host.wit` does not exist.
///
/// R-0012-a
#[test]
fn host_interface_metrics_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    assert!(
        find_interface(&resolve, pkg_id, "metrics").is_some(),
        "package `mnemra:host` must export a `metrics` interface (R-0012-a)"
    );
}

/// The `mnemra:host` package must contain a `log` interface.
///
/// R-0012-a
#[test]
fn host_interface_log_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    assert!(
        find_interface(&resolve, pkg_id, "log").is_some(),
        "package `mnemra:host` must export a `log` interface (R-0012-a)"
    );
}

/// The `mnemra:host` package must contain an `event` interface.
///
/// R-0012-a
#[test]
fn host_interface_event_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    assert!(
        find_interface(&resolve, pkg_id, "event").is_some(),
        "package `mnemra:host` must export an `event` interface (R-0012-a)"
    );
}

/// The `mnemra:host` package must contain a `projection` interface.
///
/// R-0012-a
#[test]
fn host_interface_projection_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    assert!(
        find_interface(&resolve, pkg_id, "projection").is_some(),
        "package `mnemra:host` must export a `projection` interface (R-0012-a)"
    );
}

/// The `mnemra:host` package must contain a `sampling` interface (optional
/// opt-in surface).
///
/// R-0012-b
#[test]
fn host_interface_sampling_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    assert!(
        find_interface(&resolve, pkg_id, "sampling").is_some(),
        "package `mnemra:host` must export a `sampling` interface (R-0012-b)"
    );
}

/// The `mnemra:host` package must contain a `secrets` interface (optional
/// opt-in surface).
///
/// R-0012-c
#[test]
fn host_interface_secrets_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    assert!(
        find_interface(&resolve, pkg_id, "secrets").is_some(),
        "package `mnemra:host` must export a `secrets` interface (R-0012-c)"
    );
}

// ---------------------------------------------------------------------------
// Required functions — R-0012-a
// ---------------------------------------------------------------------------

/// `artifact` interface must contain `artifact-create`.
///
/// R-0012-a
#[test]
fn artifact_create_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "artifact")
        .expect("artifact interface must exist (R-0012-a)");
    assert!(
        iface.functions.contains_key("artifact-create"),
        "`artifact` interface must declare `artifact-create` (R-0012-a)"
    );
}

/// `artifact` interface must contain `artifact-update`.
///
/// R-0012-a
#[test]
fn artifact_update_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "artifact")
        .expect("artifact interface must exist (R-0012-a)");
    assert!(
        iface.functions.contains_key("artifact-update"),
        "`artifact` interface must declare `artifact-update` (R-0012-a)"
    );
}

/// `artifact` interface must contain `artifact-get`.
///
/// R-0012-a
#[test]
fn artifact_get_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "artifact")
        .expect("artifact interface must exist (R-0012-a)");
    assert!(
        iface.functions.contains_key("artifact-get"),
        "`artifact` interface must declare `artifact-get` (R-0012-a)"
    );
}

/// `artifact` interface must contain `artifact-list`.
///
/// R-0012-a
#[test]
fn artifact_list_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "artifact")
        .expect("artifact interface must exist (R-0012-a)");
    assert!(
        iface.functions.contains_key("artifact-list"),
        "`artifact` interface must declare `artifact-list` (R-0012-a)"
    );
}

/// `artifact` interface must contain `artifact-delete` (opt-in).
///
/// R-0012-a, R-0003-g
#[test]
fn artifact_delete_exists_as_opt_in() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "artifact")
        .expect("artifact interface must exist (R-0012-a)");
    assert!(
        iface.functions.contains_key("artifact-delete"),
        "`artifact` interface must declare `artifact-delete` as an opt-in \
         function (R-0012-a, R-0003-g)"
    );
}

/// `metrics` interface must contain `metrics-record`.
///
/// R-0012-a
#[test]
fn metrics_record_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "metrics")
        .expect("metrics interface must exist (R-0012-a)");
    assert!(
        iface.functions.contains_key("metrics-record"),
        "`metrics` interface must declare `metrics-record` (R-0012-a)"
    );
}

/// `log` interface must contain `log-emit`.
///
/// R-0012-a
#[test]
fn log_emit_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface =
        find_interface(&resolve, pkg_id, "log").expect("log interface must exist (R-0012-a)");
    assert!(
        iface.functions.contains_key("log-emit"),
        "`log` interface must declare `log-emit` (R-0012-a)"
    );
}

/// `event` interface must contain `event-emit`.
///
/// R-0012-a
#[test]
fn event_emit_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface =
        find_interface(&resolve, pkg_id, "event").expect("event interface must exist (R-0012-a)");
    assert!(
        iface.functions.contains_key("event-emit"),
        "`event` interface must declare `event-emit` (R-0012-a)"
    );
}

/// `projection` interface must contain `projection-emit`.
///
/// R-0012-a
#[test]
fn projection_emit_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "projection")
        .expect("projection interface must exist (R-0012-a)");
    assert!(
        iface.functions.contains_key("projection-emit"),
        "`projection` interface must declare `projection-emit` (R-0012-a)"
    );
}

/// `sampling` interface must contain `sampling-request`.
///
/// R-0012-b
#[test]
fn sampling_request_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "sampling")
        .expect("sampling interface must exist (R-0012-b)");
    assert!(
        iface.functions.contains_key("sampling-request"),
        "`sampling` interface must declare `sampling-request` (R-0012-b)"
    );
}

/// `secrets` interface must contain `secrets-get`.
///
/// R-0012-c
#[test]
fn secrets_get_exists() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "secrets")
        .expect("secrets interface must exist (R-0012-c)");
    assert!(
        iface.functions.contains_key("secrets-get"),
        "`secrets` interface must declare `secrets-get` (R-0012-c)"
    );
}

// ---------------------------------------------------------------------------
// WorkspaceCtx-first invariant (universal) — R-0006-a
// ---------------------------------------------------------------------------

/// Every function in every host interface must have `WorkspaceCtx` (named
/// `workspace-ctx`) as its first parameter.  This test enumerates ALL
/// functions across ALL host interfaces and asserts the invariant holds
/// universally — spot-checks would allow violations to slip through on
/// newly added functions.
///
/// Red: fails because the host interfaces do not exist yet.
///
/// R-0006-a
#[test]
fn all_host_fns_take_workspace_ctx_first() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");

    let host_iface_names = write_path_interface_names();
    let mut checked = 0usize;

    for &iface_name in host_iface_names {
        let Some(iface) = find_interface(&resolve, pkg_id, iface_name) else {
            panic!(
                "host interface `{}` not found — must be declared in wit/host.wit (R-0006-a)",
                iface_name
            );
        };
        for (fn_name, func) in &iface.functions {
            assert_ctx_first(&resolve, fn_name, func);
            checked += 1;
        }
    }

    assert!(
        checked > 0,
        "no host-fn functions found across all host interfaces — \
         wit/host.wit must declare them (R-0006-a)"
    );
}

// ---------------------------------------------------------------------------
// No workspace-id on any write-path host-fn — R-0012-d, R-0003-d
// (Cross-workspace SQL leak — compile-time ABI prevention)
// ---------------------------------------------------------------------------

/// UNIVERSAL: iterate every function across every host interface and assert
/// none exposes `workspace-id` as a parameter.  A spot-check would pass for
/// functions in the contract table while allowing future additions to violate
/// the invariant silently.
///
/// Red: fails because the host interfaces do not exist yet (zero functions
/// found → assertion fires).
///
/// R-0012-d, R-0003-d
#[test]
fn write_path_host_fns_expose_no_workspace_id() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");

    let host_iface_names = write_path_interface_names();
    let mut checked = 0usize;

    for &iface_name in host_iface_names {
        let Some(iface) = find_interface(&resolve, pkg_id, iface_name) else {
            panic!(
                "host interface `{}` not found — must be declared in wit/host.wit \
                 (R-0012-d / R-0003-d)",
                iface_name
            );
        };
        for (fn_name, func) in &iface.functions {
            assert_no_workspace_id_param(fn_name, func);
            checked += 1;
        }
    }

    assert!(
        checked > 0,
        "no host-fn functions found — cannot verify workspace-id absence (R-0012-d)"
    );
}

// ---------------------------------------------------------------------------
// Stability annotations — R-0012-e (data-level)
// ---------------------------------------------------------------------------

/// Every function in every host interface must carry a non-Unknown stability
/// annotation (`@since(version=…)` maps to `Stability::Stable`; `@unstable`
/// maps to `Stability::Unstable`).
///
/// Note: `@unstable` functions are included because `all_features = true` is
/// set in `load_resolve()`.
///
/// Red: fails because the host interfaces do not exist yet.
///
/// R-0012-e
#[test]
fn all_host_fns_have_stability_annotation() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");

    let host_iface_names = write_path_interface_names();
    let mut checked = 0usize;

    for &iface_name in host_iface_names {
        let Some(iface) = find_interface(&resolve, pkg_id, iface_name) else {
            panic!(
                "host interface `{}` not found — must be declared in wit/host.wit (R-0012-e)",
                iface_name
            );
        };
        for (fn_name, func) in &iface.functions {
            assert_stability_annotated(fn_name, func);
            checked += 1;
        }
    }

    assert!(
        checked > 0,
        "no host-fn functions found — cannot verify stability annotations (R-0012-e)"
    );
}

// ---------------------------------------------------------------------------
// No raw byte buffers — R-0012-f
// ---------------------------------------------------------------------------

/// No host-fn parameter or return type may be a raw `list<u8>` byte buffer.
///
/// Red: fails because the host interfaces do not exist yet.
///
/// R-0012-f
#[test]
fn no_host_fn_uses_raw_byte_buffer() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");

    let host_iface_names = write_path_interface_names();
    let mut checked = 0usize;

    for &iface_name in host_iface_names {
        let Some(iface) = find_interface(&resolve, pkg_id, iface_name) else {
            panic!(
                "host interface `{}` not found — must be declared in wit/host.wit (R-0012-f)",
                iface_name
            );
        };
        for (fn_name, func) in &iface.functions {
            assert_no_raw_byte_buffer(&resolve, fn_name, func);
            checked += 1;
        }
    }

    assert!(
        checked > 0,
        "no host-fn functions found — cannot verify absence of raw byte buffers (R-0012-f)"
    );
}

// ---------------------------------------------------------------------------
// sampling-request: context_ids param shape — R-0012-b
// ---------------------------------------------------------------------------

/// `sampling-request` must accept a `context-ids` parameter of type
/// `list<string>`, NOT artifact bodies.  The parameter must NOT be named
/// `bodies` or `body`.
///
/// Red: fails because `sampling` interface does not exist yet.
///
/// R-0012-b
#[test]
fn sampling_request_accepts_context_ids_not_bodies() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "sampling")
        .expect("sampling interface must exist (R-0012-b)");
    let func = iface
        .functions
        .get("sampling-request")
        .expect("`sampling-request` function must exist (R-0012-b)");

    // Must have a param named `context-ids`
    let has_context_ids = func.params.iter().any(|p| p.name == "context-ids");
    assert!(
        has_context_ids,
        "`sampling-request` must have a `context-ids` parameter (R-0012-b); \
         got params: {:?}",
        func.params.iter().map(|p| &p.name).collect::<Vec<_>>()
    );

    // Must NOT have a param named `bodies` or `body`
    for param in &func.params {
        assert_ne!(
            param.name, "bodies",
            "`sampling-request` must not accept artifact `bodies` (R-0012-b)"
        );
        assert_ne!(
            param.name, "body",
            "`sampling-request` must not accept artifact `body` (R-0012-b)"
        );
    }

    // context-ids must be list<string>
    let ctx_ids_param = func
        .params
        .iter()
        .find(|p| p.name == "context-ids")
        .unwrap();
    let is_list_string = match &ctx_ids_param.ty {
        Type::Id(id) => {
            let td = &resolve.types[*id];
            matches!(&td.kind, wit_parser::TypeDefKind::List(Type::String))
        }
        _ => false,
    };
    assert!(
        is_list_string,
        "`sampling-request` param `context-ids` must be `list<string>`, \
         got {:?} (R-0012-b)",
        ctx_ids_param.ty
    );
}

// ---------------------------------------------------------------------------
// secrets interface: no write-path functions — R-0012-c
// ---------------------------------------------------------------------------

/// The `secrets` interface must contain ONLY read operations.  At V0 only
/// `secrets-get` is declared; there must be no `secrets-set`, `secrets-delete`,
/// or any other write-path function.
///
/// Red: fails because `secrets` interface does not exist yet.
///
/// R-0012-c
#[test]
fn secrets_interface_has_no_write_path() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "secrets")
        .expect("secrets interface must exist (R-0012-c)");

    let write_fn_prefixes = [
        "secrets-set",
        "secrets-delete",
        "secrets-put",
        "secrets-create",
    ];
    for fn_name in iface.functions.keys() {
        for prefix in &write_fn_prefixes {
            assert_ne!(
                fn_name.as_str(),
                *prefix,
                "`secrets` interface must not expose write-path function `{}` — \
                 no write path to secrets store at V0 (R-0012-c)",
                fn_name
            );
        }
    }
    let _ = resolve; // suppress unused warning
}

// ---------------------------------------------------------------------------
// artifact-create signature — R-0012-a (contract table)
// ---------------------------------------------------------------------------

/// `artifact-create` must have the contract-table parameter shape:
/// `(ctx: workspace-ctx, type: string, frontmatter: <JSON-type>, body: option<string>) -> <id-type>`
///
/// The spec leaves the exact WIT spelling of `JSON` and the return `id` type
/// to the implementer; these tests only assert param names and positions that
/// the contract table fixes.  Task 4 chooses WIT spellings for JSON, id, etc.
///
/// Red: fails because `artifact` interface does not exist yet.
///
/// R-0012-a
#[test]
fn artifact_create_signature_params() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface =
        find_interface(&resolve, pkg_id, "artifact").expect("artifact interface must exist");
    let func = iface
        .functions
        .get("artifact-create")
        .expect("`artifact-create` must exist (R-0012-a)");

    // param[0]: ctx: workspace-ctx  (R-0006-a)
    assert_ctx_first(&resolve, "artifact-create", func);

    // param[1]: type: string
    let p1 = func
        .params
        .get(1)
        .expect("`artifact-create` must have param[1] `type`");
    assert_eq!(
        p1.name, "type",
        "`artifact-create` param[1] must be named `type`"
    );
    assert!(
        matches!(p1.ty, Type::String),
        "`artifact-create` param `type` must be WIT `string`, got {:?}",
        p1.ty
    );

    // param[2]: frontmatter: <JSON type> — name only (type left to implementer)
    let p2 = func
        .params
        .get(2)
        .expect("`artifact-create` must have param[2] `frontmatter`");
    assert_eq!(
        p2.name, "frontmatter",
        "`artifact-create` param[2] must be named `frontmatter`"
    );

    // param[3]: body: option<string>
    let p3 = func
        .params
        .get(3)
        .expect("`artifact-create` must have param[3] `body`");
    assert_eq!(
        p3.name, "body",
        "`artifact-create` param[3] must be named `body`"
    );
    let is_option_string = match &p3.ty {
        Type::Id(id) => {
            let td = &resolve.types[*id];
            matches!(&td.kind, wit_parser::TypeDefKind::Option(Type::String))
        }
        _ => false,
    };
    assert!(
        is_option_string,
        "`artifact-create` param `body` must be `option<string>`, got {:?}",
        p3.ty
    );

    // no workspace-id anywhere
    assert_no_workspace_id_param("artifact-create", func);
}

// ---------------------------------------------------------------------------
// artifact-update signature — R-0012-a (contract table)
// ---------------------------------------------------------------------------

/// `artifact-update` must have: `(ctx, id: string, frontmatter-patch: <JSON>, body: option<string>)`
///
/// R-0012-a
#[test]
fn artifact_update_signature_params() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface =
        find_interface(&resolve, pkg_id, "artifact").expect("artifact interface must exist");
    let func = iface
        .functions
        .get("artifact-update")
        .expect("`artifact-update` must exist (R-0012-a)");

    assert_ctx_first(&resolve, "artifact-update", func);

    let p1 = func
        .params
        .get(1)
        .expect("`artifact-update` must have param[1] `id`");
    assert_eq!(p1.name, "id");
    assert!(
        matches!(p1.ty, Type::String),
        "`artifact-update` param `id` must be string"
    );

    let p2 = func
        .params
        .get(2)
        .expect("`artifact-update` must have param[2] `frontmatter-patch`");
    assert_eq!(p2.name, "frontmatter-patch");

    let p3 = func
        .params
        .get(3)
        .expect("`artifact-update` must have param[3] `body`");
    assert_eq!(p3.name, "body");

    assert_no_workspace_id_param("artifact-update", func);
}

// ---------------------------------------------------------------------------
// artifact-get signature — R-0012-a (contract table)
// ---------------------------------------------------------------------------

/// `artifact-get` must have: `(ctx, id: string)`
///
/// R-0012-a
#[test]
fn artifact_get_signature_params() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface =
        find_interface(&resolve, pkg_id, "artifact").expect("artifact interface must exist");
    let func = iface
        .functions
        .get("artifact-get")
        .expect("`artifact-get` must exist (R-0012-a)");

    assert_ctx_first(&resolve, "artifact-get", func);

    let p1 = func
        .params
        .get(1)
        .expect("`artifact-get` must have param[1] `id`");
    assert_eq!(p1.name, "id");
    assert!(
        matches!(p1.ty, Type::String),
        "`artifact-get` param `id` must be string"
    );

    assert_no_workspace_id_param("artifact-get", func);
}

// ---------------------------------------------------------------------------
// artifact-list signature — R-0012-a (contract table)
// ---------------------------------------------------------------------------

/// `artifact-list` must have: `(ctx, type: string, filters: <JSON>)`
///
/// R-0012-a
#[test]
fn artifact_list_signature_params() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface =
        find_interface(&resolve, pkg_id, "artifact").expect("artifact interface must exist");
    let func = iface
        .functions
        .get("artifact-list")
        .expect("`artifact-list` must exist (R-0012-a)");

    assert_ctx_first(&resolve, "artifact-list", func);

    let p1 = func
        .params
        .get(1)
        .expect("`artifact-list` must have param[1] `type`");
    assert_eq!(p1.name, "type");
    assert!(
        matches!(p1.ty, Type::String),
        "`artifact-list` param `type` must be string"
    );

    let p2 = func
        .params
        .get(2)
        .expect("`artifact-list` must have param[2] `filters`");
    assert_eq!(p2.name, "filters");

    assert_no_workspace_id_param("artifact-list", func);
}

// ---------------------------------------------------------------------------
// artifact-delete signature — R-0012-a, R-0003-g
// ---------------------------------------------------------------------------

/// `artifact-delete` must have: `(ctx, id: string)`
///
/// R-0012-a, R-0003-g
#[test]
fn artifact_delete_signature_params() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface =
        find_interface(&resolve, pkg_id, "artifact").expect("artifact interface must exist");
    let func = iface
        .functions
        .get("artifact-delete")
        .expect("`artifact-delete` must exist (R-0012-a, R-0003-g)");

    assert_ctx_first(&resolve, "artifact-delete", func);

    let p1 = func
        .params
        .get(1)
        .expect("`artifact-delete` must have param[1] `id`");
    assert_eq!(p1.name, "id");
    assert!(
        matches!(p1.ty, Type::String),
        "`artifact-delete` param `id` must be string"
    );

    assert_no_workspace_id_param("artifact-delete", func);
}

// ---------------------------------------------------------------------------
// metrics-record signature — R-0012-a (contract table)
// ---------------------------------------------------------------------------

/// `metrics-record` must have:
/// `(ctx, verb: string, duration-ms: u64, outcome: string)`
///
/// R-0012-a
#[test]
fn metrics_record_signature_params() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "metrics").expect("metrics interface must exist");
    let func = iface
        .functions
        .get("metrics-record")
        .expect("`metrics-record` must exist (R-0012-a)");

    assert_ctx_first(&resolve, "metrics-record", func);

    let p1 = func
        .params
        .get(1)
        .expect("`metrics-record` must have param[1] `verb`");
    assert_eq!(p1.name, "verb");
    assert!(matches!(p1.ty, Type::String));

    let p2 = func
        .params
        .get(2)
        .expect("`metrics-record` must have param[2] `duration-ms`");
    assert_eq!(p2.name, "duration-ms");
    assert!(
        matches!(p2.ty, Type::U64),
        "`metrics-record` param `duration-ms` must be u64"
    );

    let p3 = func
        .params
        .get(3)
        .expect("`metrics-record` must have param[3] `outcome`");
    assert_eq!(p3.name, "outcome");
    assert!(matches!(p3.ty, Type::String));

    assert_no_workspace_id_param("metrics-record", func);
}

// ---------------------------------------------------------------------------
// log-emit signature — R-0012-a (contract table)
// ---------------------------------------------------------------------------

/// `log-emit` must have: `(ctx, level: string, message: string, context: option<JSON>)`
///
/// R-0012-a
#[test]
fn log_emit_signature_params() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "log").expect("log interface must exist");
    let func = iface
        .functions
        .get("log-emit")
        .expect("`log-emit` must exist (R-0012-a)");

    assert_ctx_first(&resolve, "log-emit", func);

    let p1 = func
        .params
        .get(1)
        .expect("`log-emit` must have param[1] `level`");
    assert_eq!(p1.name, "level");
    assert!(matches!(p1.ty, Type::String));

    let p2 = func
        .params
        .get(2)
        .expect("`log-emit` must have param[2] `message`");
    assert_eq!(p2.name, "message");
    assert!(matches!(p2.ty, Type::String));

    let p3 = func
        .params
        .get(3)
        .expect("`log-emit` must have param[3] `context`");
    assert_eq!(p3.name, "context");

    assert_no_workspace_id_param("log-emit", func);
}

// ---------------------------------------------------------------------------
// event-emit signature — R-0012-a (contract table)
// ---------------------------------------------------------------------------

/// `event-emit` must have:
/// `(ctx, event-type: string, event-version: u16, payload: JSON)`
///
/// R-0012-a
#[test]
fn event_emit_signature_params() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface = find_interface(&resolve, pkg_id, "event").expect("event interface must exist");
    let func = iface
        .functions
        .get("event-emit")
        .expect("`event-emit` must exist (R-0012-a)");

    assert_ctx_first(&resolve, "event-emit", func);

    let p1 = func
        .params
        .get(1)
        .expect("`event-emit` must have param[1] `event-type`");
    assert_eq!(p1.name, "event-type");
    assert!(matches!(p1.ty, Type::String));

    let p2 = func
        .params
        .get(2)
        .expect("`event-emit` must have param[2] `event-version`");
    assert_eq!(p2.name, "event-version");
    assert!(
        matches!(p2.ty, Type::U16),
        "`event-emit` param `event-version` must be u16"
    );

    let p3 = func
        .params
        .get(3)
        .expect("`event-emit` must have param[3] `payload`");
    assert_eq!(p3.name, "payload");

    assert_no_workspace_id_param("event-emit", func);
}

// ---------------------------------------------------------------------------
// projection-emit signature — R-0012-a, R-0003-d (contract table)
// ---------------------------------------------------------------------------

/// `projection-emit` must have: `(ctx, projection-name: string, data: JSON)`
///
/// The workspace_id is derived from `ctx`; it does NOT appear as an explicit
/// parameter.  This directly tests the normative calling convention stated in
/// R-0003-d.
///
/// R-0012-a, R-0003-d
#[test]
fn projection_emit_signature_params() {
    let (resolve, pkg_id) = load_resolve().expect("wit/ directory must parse");
    let iface =
        find_interface(&resolve, pkg_id, "projection").expect("projection interface must exist");
    let func = iface
        .functions
        .get("projection-emit")
        .expect("`projection-emit` must exist (R-0012-a)");

    assert_ctx_first(&resolve, "projection-emit", func);

    let p1 = func
        .params
        .get(1)
        .expect("`projection-emit` must have param[1] `projection-name`");
    assert_eq!(p1.name, "projection-name");
    assert!(matches!(p1.ty, Type::String));

    let p2 = func
        .params
        .get(2)
        .expect("`projection-emit` must have param[2] `data`");
    assert_eq!(p2.name, "data");

    assert_no_workspace_id_param("projection-emit", func);
}

// ---------------------------------------------------------------------------
// R-0012-e — behavioural stubs (ignored; Task 4 runtime half)
// ---------------------------------------------------------------------------

/// Behavioural half of R-0012-e: invoking an `@unstable` function via the
/// dispatch wrapper must return a [`DispatchWarning`] value on
/// `DispatchOutcome.warning`.
///
/// Note: R-0012-e says "@unstable emits a deprecation warning to the log."
/// At the skeleton stage the warning is returned as a value rather than
/// written to `log.emit`.  The caller owns the warning and can forward it to
/// the log surface when logging lands (named follow-up).  This satisfies the
/// R-ID at skeleton stage: the warning is observable and the contract intention
/// (caller is informed of unstable usage) is met.
///
/// R-0012-e
#[test]
fn unstable_fn_invocation_emits_deprecation_warning() {
    // R-0012-e: @unstable dispatch must return the result AND a DispatchWarning.
    let outcome = DispatchWrapper::invoke(
        &AbiStability::Unstable {
            feature: "sampling-v0",
        },
        "sampling-request",
        || "stub-result",
    )
    .expect("unstable dispatch must succeed (R-0012-e)");

    // Value is returned — the closure was called.
    assert_eq!(
        outcome.value, "stub-result",
        "unstable dispatch must return closure value"
    );

    // Warning is present with correct fields.
    let warn: DispatchWarning = outcome
        .warning
        .expect("unstable dispatch must emit DispatchWarning (R-0012-e)");
    assert_eq!(
        warn.feature, "sampling-v0",
        "DispatchWarning.feature must match the @unstable feature name (R-0012-e)"
    );
    assert_eq!(
        warn.fn_name, "sampling-request",
        "DispatchWarning.fn_name must match the dispatched function name (R-0012-e)"
    );
}

/// Behavioural half of R-0012-e: invoking a deprecated function via the
/// dispatch wrapper must return [`DispatchError::Deprecated`] WITHOUT
/// executing the closure body.
///
/// R-0012-e
#[test]
fn deprecated_fn_invocation_returns_structured_error() {
    // R-0012-e: @deprecated dispatch must short-circuit before calling `f`.
    let body_entered = Cell::new(false);

    let err: DispatchError = DispatchWrapper::invoke(
        &AbiStability::Deprecated {
            since: "0.0.1",
            reason: "replaced by new-fn",
        },
        "old-fn",
        || {
            body_entered.set(true);
        },
    )
    .expect_err("deprecated dispatch must return DispatchError (R-0012-e)");

    // Closure body must NOT have been entered.
    assert!(
        !body_entered.get(),
        "deprecated dispatch must short-circuit before calling the body closure (R-0012-e)"
    );

    // Error must be the structured Deprecated variant with correct fields.
    match err {
        DispatchError::Deprecated {
            since,
            reason,
            fn_name,
        } => {
            assert_eq!(since, "0.0.1", "DispatchError.since must match (R-0012-e)");
            assert_eq!(
                reason, "replaced by new-fn",
                "DispatchError.reason must match (R-0012-e)"
            );
            assert_eq!(
                fn_name, "old-fn",
                "DispatchError.fn_name must match (R-0012-e)"
            );
        }
    }
}
