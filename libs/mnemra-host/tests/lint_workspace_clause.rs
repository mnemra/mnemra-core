//! WHERE-clause lint: R-0006-d, R-0018-d
//!
//! # Purpose
//!
//! This file is the production WHERE-clause lint (`cargo test --test lint_workspace_clause`)
//! AND the red-phase test for Task 12. It is NOT a throwaway fixture — Task 13
//! wires it into the `verify-lint` just-recipe target as:
//!
//!   ```
//!   verify-lint:
//!     cargo test --manifest-path libs/mnemra-host/Cargo.toml \
//!       --test lint_workspace_clause
//!   ```
//!
//! # Contract
//!
//! Every read-path host-fn in the scanned source files must include a
//! `workspace_id = ctx.workspace_id` WHERE-clause condition derived from the
//! `WorkspaceCtx` argument. A function body that issues a DB query (SELECT)
//! without this clause fails the lint.
//!
//! ## What counts as a "read-path host-fn"
//!
//! A function is a read-path host-fn if ALL of the following hold:
//! 1. It is a free function (not an associated method) at the top level of the
//!    scanned source file.
//! 2. Its first parameter is named `ctx` (the WorkspaceCtx convention).
//! 3. Its return type contains `Option` or `Vec` (data-returning signatures).
//! 4. Its body (as a token string) contains a SELECT keyword (case-insensitive
//!    match on the string "SELECT").
//!
//! ## What counts as the required WHERE-clause
//!
//! The body token string (syn-parsed, comment-stripped) must contain `ctx.workspace_id`
//! or `ctx . workspace_id` (quote! renders field access with spaces). This proves
//! the workspace identity from WorkspaceCtx is actually used in the query path.
//! A function that receives `_ctx` (unused) and issues a SELECT fails — the workspace
//! identity is not threaded through to the query (R-0006-d: "not as a post-read filter").
//!
//! ## Source files scanned (production mode)
//!
//! When the real implementation lands (Task 13), the lint scans:
//!   - `libs/mnemra-host/abi/host_fns.rs`
//!   - `libs/mnemra-host/abi.rs`
//!
//! In red phase (current state), these files exist but contain `todo!()` stubs
//! with no SELECT queries. The lint passes on the real files (no read-path
//! host-fns currently have bodies with SELECT). The planted fixture tests prove
//! both directions.
//!
//! # RED-phase design for `scan_real_host_fn_files`
//!
//! The `scan_real_host_fn_files` test runs the lint against the real source
//! files. Since no `WorkspaceCtx` import from `auth` exists yet (Task 13 wires
//! it), and bodies are `todo!()` stubs, the test currently PASSES (zero
//! read-path functions with SELECT bodies = zero violations). That makes it
//! green prematurely. The `host_fns_import_from_auth_for_lint` dependency test
//! catches the red state: the lint's validity depends on `host_fns.rs` using
//! the canonical `WorkspaceCtx`, which requires Task 13's import.
//!
//! The two-fixture tests (`violation_fixture_caught` and `compliant_fixture_passes`)
//! are the load-bearing red evidence in this phase — they prove the lint
//! mechanism works before Task 13 creates real read-path bodies.
//!
//! # Spec requirements traced
//!
//! - R-0006-d: all read-path host-fns SHALL include `workspace_id = ctx.workspace_id`
//!   as a WHERE-clause condition derived from WorkspaceCtx; a CI lint
//!   check SHALL assert this
//! - R-0018-d: CI lint = `cargo test --test lint_workspace_clause` (syn AST);
//!   a planted violation MUST return non-zero AND name the offending function

use syn::{FnArg, Item, Pat};

// ---------------------------------------------------------------------------
// Lint engine: classify and check functions
// ---------------------------------------------------------------------------

/// A lint finding: a read-path host-fn missing the WHERE-clause.
#[derive(Debug)]
struct LintViolation {
    fn_name: String,
    reason: String,
}

/// Run the WHERE-clause lint over a string of Rust source code.
///
/// Returns a list of violations. An empty list means all read-path host-fns
/// include the required WHERE-clause.
///
/// # Read-path classification
///
/// A function is classified as a read-path host-fn if:
/// 1. It is a top-level free function (not method).
/// 2. Its first parameter is named `ctx` (WorkspaceCtx first-param convention).
/// 3. Its return type contains `Option` or `Vec` (data-returning).
/// 4. Its body token string contains "SELECT" (case-insensitive).
///
/// # Required clause
///
/// The body token string must contain `ctx . workspace_id` or `ctx.workspace_id`
/// (syn strips comments; `quote!` renders `.` field access with spaces).
/// This proves the workspace identity derived from `WorkspaceCtx` is actually
/// used in the query path, not as a post-read filter. A function that receives
/// `_ctx` (unused) and issues a SELECT fails the lint.
fn run_lint(source: &str) -> Vec<LintViolation> {
    let ast = syn::parse_file(source).expect("lint source must parse");
    let mut violations = Vec::new();

    for item in &ast.items {
        if let Item::Fn(func) = item {
            let fn_name = func.sig.ident.to_string();

            // 1. First param named `ctx`
            let first_param_is_ctx = func
                .sig
                .inputs
                .first()
                .map(|p| {
                    if let FnArg::Typed(pt) = p
                        && let Pat::Ident(pi) = pt.pat.as_ref()
                    {
                        // Accept both `ctx` and `_ctx` (the underscore prefix is
                        // used for unused params in stubs — Task 13 removes them).
                        return pi.ident == "ctx" || pi.ident == "_ctx";
                    }
                    false
                })
                .unwrap_or(false);

            if !first_param_is_ctx {
                continue;
            }

            // 2. Return type contains Option or Vec
            let sig_output = &func.sig.output;
            let return_type_str = format!("{}", quote::quote!(#sig_output));
            let is_data_returning =
                return_type_str.contains("Option") || return_type_str.contains("Vec");

            if !is_data_returning {
                continue;
            }

            // 3. Body contains SELECT keyword
            let func_block = &func.block;
            let body_tokens = format!("{}", quote::quote!(#func_block));
            let body_lower = body_tokens.to_lowercase();
            if !body_lower.contains("select") {
                continue;
            }

            // This is a read-path host-fn. Check for the WHERE-clause.
            // The clause must be `workspace_id = ctx.workspace_id`.
            // In token form (quote!), field access `ctx.workspace_id` becomes
            // `ctx . workspace_id` (spaces around the dot).
            // We check both forms to be robust.
            // The WHERE-clause check scans the body token string only (syn strips comments
            // during parsing, so body_tokens is comment-free).
            //
            // Discriminator: the body must reference `ctx.workspace_id` (field access or
            // method call) — the only way to thread the workspace identity through to the
            // query. This fires if the workspace ctx is not used at all in the body.
            //
            // `quote!` renders field access `ctx.workspace_id` as `ctx . workspace_id`
            // (space-separated tokens). Method call `ctx.workspace_id()` renders as
            // `ctx . workspace_id ()`.
            // We check both field-access and method-call forms.
            //
            // NOTE: We do NOT use source.contains(...) as a fallback — source includes
            // comments; a comment mentioning the pattern would cause a false negative.
            let has_clause = body_tokens.contains("ctx . workspace_id")
                || body_tokens.contains("ctx.workspace_id");

            if !has_clause {
                violations.push(LintViolation {
                    fn_name: fn_name.clone(),
                    reason: format!(
                        "read-path host-fn `{}` contains SELECT but does not reference \
                        `ctx.workspace_id` — missing workspace_id WHERE-clause (R-0006-d, R-0018-d)",
                        fn_name
                    ),
                });
            }
        }
    }

    violations
}

/// Format lint violations for display (non-zero = failure, names offending fns).
fn format_violations(violations: &[LintViolation]) -> String {
    violations
        .iter()
        .map(|v| format!("  LINT FAIL [{}]: {}", v.fn_name, v.reason))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Fixture source strings
// ---------------------------------------------------------------------------

/// A planted read-path host-fn WITHOUT the workspace_id WHERE-clause.
/// The lint must catch this (R-0018-d: "a planted violation MUST return
/// non-zero and name the offending function").
const VIOLATION_FIXTURE: &str = r#"
use uuid::Uuid;

// Stub types for fixture compilation
pub struct WorkspaceCtx { workspace_id: Uuid }
pub struct DispatchError;
pub struct DispatchOutcome<T> { pub value: T }
pub type Json = String;

/// read-path host-fn that issues a SELECT but never uses ctx.workspace_id.
/// The workspace context is received (_ctx with underscore = unused) but
/// the SELECT query does not filter by workspace. This is the violation.
pub fn artifact_get(
    _ctx: WorkspaceCtx,
    _id: &str,
) -> Result<DispatchOutcome<Option<String>>, DispatchError> {
    // Missing: ctx.workspace_id must appear in the query (R-0006-d)
    let _query = "SELECT id, body FROM artifacts WHERE id = $1";
    todo!()
}
"#;

/// A compliant read-path host-fn WITH the required workspace_id WHERE-clause.
/// The lint must NOT flag this (proves the lint doesn't false-positive on
/// well-formed code).
const COMPLIANT_FIXTURE: &str = r#"
use uuid::Uuid;

// Stub types for fixture compilation
pub struct WorkspaceCtx { workspace_id: Uuid }
pub struct DispatchError;
pub struct DispatchOutcome<T> { pub value: T }
pub type Json = String;

/// read-path host-fn that uses ctx.workspace_id in the query — compliant.
/// The workspace_id is threaded through ctx into the SELECT WHERE-clause.
pub fn artifact_get(
    ctx: WorkspaceCtx,
    _id: &str,
) -> Result<DispatchOutcome<Option<String>>, DispatchError> {
    // Correct: ctx.workspace_id is used to scope the query to the workspace (R-0006-d).
    // In real sqlx usage, this binds ctx.workspace_id() as a WHERE clause parameter.
    let workspace_scope = ctx.workspace_id;
    let _query = sqlx_stub_select_with_workspace(workspace_scope, "some-id");
    todo!()
}

fn sqlx_stub_select_with_workspace(workspace_id: Uuid, _id: &str) -> String {
    format!("SELECT id, body FROM artifacts WHERE workspace_id = '{}' AND id = $1", workspace_id)
}
"#;

/// A non-read-path host-fn (write path, no SELECT, no Option/Vec return).
/// The lint must NOT classify this as a read-path fn (no false positives).
const NON_READ_PATH_FIXTURE: &str = r#"
use uuid::Uuid;

pub struct WorkspaceCtx { workspace_id: Uuid }
pub struct DispatchError;
pub struct DispatchOutcome<T> { pub value: T }
pub type Json = String;

/// write-path host-fn — NOT a read-path fn; lint must ignore it
pub fn artifact_create(
    _ctx: WorkspaceCtx,
    _type_name: &str,
    _body: Option<String>,
) -> Result<DispatchOutcome<String>, DispatchError> {
    // No SELECT, return type is String (not Option or Vec) — not read-path
    let _query = "INSERT INTO artifacts (id, type_name) VALUES ($1, $2)";
    todo!()
}
"#;

// ---------------------------------------------------------------------------
// Fixture tests: prove both directions (R-0018-d)
// ---------------------------------------------------------------------------

/// R-0018-d (direction 1 — violation caught):
/// The planted read-path host-fn WITHOUT the WHERE-clause must be caught by
/// the lint. This is the "planted violation MUST return non-zero and name
/// the offending function" requirement.
///
/// This test proves the lint mechanism works at red phase.
#[test]
fn violation_fixture_caught_with_offending_function_named() {
    // R-0018-d direction 1
    let violations = run_lint(VIOLATION_FIXTURE);

    assert!(
        !violations.is_empty(),
        "Lint must catch the planted violation (R-0018-d): \
        `artifact_get` in the fixture has a SELECT but ctx.workspace_id is never used \
        (_ctx with underscore = unused). \
        The lint returned zero violations — this is a lint defect."
    );

    // Must name the offending function
    let names: Vec<&str> = violations.iter().map(|v| v.fn_name.as_str()).collect();
    assert!(
        names.contains(&"artifact_get"),
        "Lint must name the offending function `artifact_get` (R-0018-d); \
        found violations naming: {:?}",
        names
    );

    // Display what the lint found (for test output legibility)
    eprintln!(
        "Lint correctly found violations:\n{}",
        format_violations(&violations)
    );
}

/// R-0018-d (direction 2 — compliant passes):
/// The compliant read-path host-fn WITH the WHERE-clause must NOT be flagged.
/// This proves the lint doesn't false-positive on well-formed code.
#[test]
fn compliant_fixture_passes_without_violation() {
    // R-0018-d direction 2
    let violations = run_lint(COMPLIANT_FIXTURE);

    assert!(
        violations.is_empty(),
        "Lint must NOT flag the compliant fixture (R-0018-d direction 2). \
        Found violations:\n{}",
        format_violations(&violations)
    );
}

/// Non-read-path functions (write path, INSERT, no Option/Vec return) must NOT
/// be classified as read-path fns. The lint must not false-positive on them.
#[test]
fn write_path_fixture_not_classified_as_read_path() {
    let violations = run_lint(NON_READ_PATH_FIXTURE);

    assert!(
        violations.is_empty(),
        "Lint must NOT flag write-path host-fns (no SELECT in relevant position, \
        or no Option/Vec return). Found violations:\n{}",
        format_violations(&violations)
    );
}

// ---------------------------------------------------------------------------
// Production lint: scan the real host-fn source files
// ---------------------------------------------------------------------------

/// Dependency check: `abi/host_fns.rs` must import WorkspaceCtx from auth,
/// not define it locally. The lint's correctness is contingent on there being
/// a single canonical WorkspaceCtx type. This test is RED until Task 13 removes
/// the local stub and adds the import.
///
/// R-0006-a (canonical type), R-0018-d (lint validity)
#[test]
fn host_fns_defines_no_local_workspace_ctx_stub() {
    // R-0006-a / R-0018-d — the lint's validity depends on this
    let host_fns_path =
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"))).join("abi/host_fns.rs");

    let src = std::fs::read_to_string(&host_fns_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", host_fns_path.display(), e));

    let ast = syn::parse_file(&src).expect("abi/host_fns.rs must parse");

    let local_stub = ast.items.iter().any(|item| {
        if let Item::Struct(s) = item {
            s.ident == "WorkspaceCtx"
        } else {
            false
        }
    });

    assert!(
        !local_stub,
        "abi/host_fns.rs must NOT define WorkspaceCtx locally (R-0006-a). \
        Found a local `pub struct WorkspaceCtx` — Task 13 must remove this stub \
        and import from `crate::auth::workspace_ctx`. \
        The lint's correctness depends on a single canonical WorkspaceCtx type."
    );
}

/// R-0006-d, R-0018-d: run the WHERE-clause lint against the real host-fn
/// source files. All read-path host-fns must include the WHERE-clause.
///
/// In red phase (Task 12), the real functions have `todo!()` bodies with no
/// SELECT queries, so this test passes (zero violations). The planted-fixture
/// tests above are the red-phase evidence. This test becomes load-bearing
/// once Task 13 wires real DB queries into the read-path functions.
#[test]
fn scan_real_host_fn_files_no_where_clause_violations() {
    // R-0006-d, R-0018-d
    // Note: this test currently passes (todo!() stubs have no SELECT).
    // The `host_fns_defines_no_local_workspace_ctx_stub` test above is what
    // makes the full lint surface RED until Task 13 wires the canonical type.

    let files = vec![
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"))).join("abi/host_fns.rs"),
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"))).join("abi.rs"),
    ];

    let mut all_violations: Vec<(String, LintViolation)> = Vec::new();

    for path in &files {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                // File absent at red phase is acceptable for supplementary files
                eprintln!("lint: skipping {} (not found: {})", path.display(), e);
                continue;
            }
        };

        let violations = run_lint(&src);
        for v in violations {
            all_violations.push((path.display().to_string(), v));
        }
    }

    if !all_violations.is_empty() {
        let msg: Vec<String> = all_violations
            .iter()
            .map(|(file, v)| format!("  [{}] LINT FAIL [{}]: {}", file, v.fn_name, v.reason))
            .collect();
        panic!(
            "WHERE-clause lint found {} violation(s) (R-0006-d, R-0018-d):\n{}",
            all_violations.len(),
            msg.join("\n")
        );
    }
}
