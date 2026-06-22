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
//! 2. Its return type contains `Option` or `Vec` (data-returning signatures).
//! 3. Its body (as a token string) contains a SELECT keyword (case-insensitive
//!    match on the string "SELECT").
//!
//! The classifier is **position-independent**: it does NOT require the first
//! parameter to be named `ctx`. The builtins read-paths (added to the scanned
//! set in T13.3) take `pool: &PgPool` first; a first-param-name gate would
//! silently skip them and let an unscoped builtin SELECT pass the lint — a false
//! green. See the rationale on `run_lint`.
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
//! The lint scans:
//!   - `libs/mnemra-host/abi/host_fns.rs`
//!   - `libs/mnemra-host/abi.rs`
//!   - `libs/mnemra-host/builtins/agents.rs`     (T13.3 / R-0006-e)
//!   - `libs/mnemra-host/builtins/sessions.rs`   (T13.3 / R-0006-e)
//!   - `libs/mnemra-host/builtins/projects.rs`   (R-0006-e expansion)
//!
//! The `abi/*` files contain `todo!()` stubs with no SELECT queries, so they
//! produce no read-path classifications. The classified read-paths
//! (`agents::list_by_workspace`, `sessions::list_active_by_workspace`,
//! `projects::list_by_workspace`) return `Vec` and issue `SELECT`, so they are
//! classified — and at the current raw-`workspace_id` state they bind
//! `workspace_id` directly without referencing `ctx.workspace_id`, so the lint
//! FLAGS them (RED). The green phase threads `&WorkspaceCtx` and binds
//! `ctx.workspace_id()`, which clears the flag.
//!
//! Note: `projects::exists` returns `Result<bool, _>` — `bool` is neither
//! `Option` nor `Vec`, so the data-returning classifier does NOT pick it up. The
//! lint flags `projects::list_by_workspace` (a `Vec`-returning SELECT), which is
//! sufficient to make the projects.rs scan RED. The cross-tenant behavior of
//! `exists` is pinned by the behavior test `projects_exists_is_false_cross_tenant`
//! in `tests/tenancy_isolation.rs` rather than by this structural lint.
//!
//! `builtins/projects.rs` was SILENTLY omitted from the scanned set before the
//! R-0006-e expansion (security audit, dispatch 1073) — a false green: the lint
//! that enforces R-0006-e scanned only the builtins already threaded, so the live
//! `projects` bypass passed by construction. The fix is to enumerate every
//! tenant-table builtin source in the scanned set, not just the threaded ones.
//!
//! `builtins/authentication.rs` is intentionally NOT in the scanned set:
//! `bootstrap` is the documented pre-token carve-out (R-0006-e) — it runs before
//! any `WorkspaceCtx` can exist, so it cannot reference `ctx.workspace_id`. See
//! the `bootstrap_is_the_single_pre_token_carve_out` test.
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

use syn::Item;

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
/// 2. Its return type contains `Option` or `Vec` (data-returning).
/// 3. Its body token string contains "SELECT" (case-insensitive).
///
/// ## Why the first-parameter NAME is NOT a classifier gate (T13.3 / R-0006-e)
///
/// An earlier version of this lint required the first parameter to be named
/// `ctx`/`_ctx`. That gate was sound for the `abi/host_fns.rs` surface (where
/// `WorkspaceCtx` is genuinely the first param), but it would SILENTLY SKIP the
/// identity-query builtins (`builtins/agents.rs`, `builtins/sessions.rs`), whose
/// first parameter is `pool: &PgPool`. With the builtins now in the scanned set
/// (T13.3), a name gate would mean a raw-`workspace_id` builtin issuing an
/// unscoped SELECT is never even classified — the lint would pass on a real
/// tenancy bypass (a false green / inverted gate). The classifier is therefore
/// position-independent: any data-returning function whose body contains SELECT
/// is a read-path, regardless of where `WorkspaceCtx` sits in its parameter
/// list. The contract is enforced by the required-clause check below.
///
/// # Required clause
///
/// The body token string must contain `ctx . workspace_id` or `ctx.workspace_id`
/// (syn strips comments; `quote!` renders `.` field access with spaces).
/// This proves the workspace identity derived from `WorkspaceCtx` is actually
/// used in the query path, not as a post-read filter. A function that issues a
/// SELECT but never references `ctx.workspace_id` fails the lint — this is
/// exactly the state of the current raw-`workspace_id` builtins (RED).
fn run_lint(source: &str) -> Vec<LintViolation> {
    let ast = syn::parse_file(source).expect("lint source must parse");
    let mut violations = Vec::new();

    for item in &ast.items {
        if let Item::Fn(func) = item {
            let fn_name = func.sig.ident.to_string();

            // 1. Return type contains Option or Vec
            let sig_output = &func.sig.output;
            let return_type_str = format!("{}", quote::quote!(#sig_output));
            let is_data_returning =
                return_type_str.contains("Option") || return_type_str.contains("Vec");

            if !is_data_returning {
                continue;
            }

            // 2. Body contains SELECT keyword
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
// T13.3 / R-0006-e — builtins read-path fixtures (pool-first signature)
//
// The identity-query builtins take `pool: &PgPool` first; `WorkspaceCtx`/raw
// `workspace_id` is NOT the first parameter. These fixtures pin that the lint's
// position-independent classifier catches an unscoped builtin SELECT and clears
// a ctx-scoped one — the property that a first-param-name gate would have missed.
// ---------------------------------------------------------------------------

/// A planted BUILTIN read-path that mirrors the CURRENT raw-`workspace_id`
/// builtins: `pool` first, a raw `workspace_id: Uuid` param, and a SELECT that
/// binds the raw id without going through `ctx.workspace_id`. The lint MUST flag
/// this — it is the live tenancy-bypass shape T13.3 forbids.
const BUILTIN_RAW_WORKSPACE_ID_VIOLATION_FIXTURE: &str = r#"
use uuid::Uuid;

// Stub types for fixture compilation
pub struct PgPool;
pub struct AgentError;
pub struct Agent;

/// builtin read-path with a RAW workspace_id param (pre-T13.3 bypass shape).
/// Returns Vec, contains SELECT, but never references ctx.workspace_id → violation.
pub fn list_by_workspace(
    _pool: &PgPool,
    workspace_id: Uuid,
) -> Result<Vec<Agent>, AgentError> {
    // Raw workspace_id bound directly — no WorkspaceCtx threaded (R-0006-e bypass).
    let _query = "SELECT id, workspace_id FROM agents WHERE workspace_id = $1";
    let _bind = workspace_id;
    todo!()
}
"#;

/// A compliant BUILTIN read-path in the LOCKED target shape: `pool` first,
/// `ctx: &WorkspaceCtx` threaded, SELECT scoped by `ctx.workspace_id()`. The
/// lint must NOT flag this — proves the position-independent classifier clears
/// the green shape even though `ctx` is not the first parameter.
const BUILTIN_CTX_THREADED_COMPLIANT_FIXTURE: &str = r#"
use uuid::Uuid;

// Stub types for fixture compilation
pub struct PgPool;
pub struct AgentError;
pub struct Agent;
pub struct WorkspaceCtx { workspace_id: Uuid }
impl WorkspaceCtx {
    pub fn workspace_id(&self) -> Uuid { self.workspace_id }
}

/// builtin read-path in the T13.3 target shape — ctx threaded (second param),
/// SELECT scoped by ctx.workspace_id(). Compliant.
pub fn list_by_workspace(
    _pool: &PgPool,
    ctx: &WorkspaceCtx,
) -> Result<Vec<Agent>, AgentError> {
    // ctx.workspace_id() drives the WHERE clause (R-0006-d / R-0006-e).
    let _query = "SELECT id, workspace_id FROM agents WHERE workspace_id = $1";
    let _bind = ctx.workspace_id();
    todo!()
}
"#;

/// T13.3 / R-0006-e (direction 1 — builtin bypass caught):
/// A builtin read-path that binds a raw `workspace_id` (pool-first signature,
/// ctx NOT first) must be flagged. This is the property the old first-param-name
/// gate would have silently skipped — proving the classifier loosening is
/// load-bearing, not cosmetic.
#[test]
fn builtin_raw_workspace_id_read_path_is_flagged() {
    // R-0006-e direction 1
    let violations = run_lint(BUILTIN_RAW_WORKSPACE_ID_VIOLATION_FIXTURE);

    assert!(
        !violations.is_empty(),
        "Lint must flag a builtin read-path that binds a RAW workspace_id and never \
        references ctx.workspace_id (R-0006-e). The function takes `pool` first and \
        `workspace_id` second; a first-param-name classifier would skip it entirely. \
        Zero violations here means the classifier still gates on the param name — \
        an inverted/false-green lint."
    );

    let names: Vec<&str> = violations.iter().map(|v| v.fn_name.as_str()).collect();
    assert!(
        names.contains(&"list_by_workspace"),
        "Lint must name the offending builtin read-path `list_by_workspace` (R-0006-e); \
        found violations naming: {:?}",
        names
    );
}

/// T13.3 / R-0006-e (direction 2 — ctx-threaded builtin passes):
/// A builtin read-path in the locked target shape (`ctx: &WorkspaceCtx` second,
/// SELECT scoped by `ctx.workspace_id()`) must NOT be flagged — even though ctx
/// is not the first parameter.
#[test]
fn builtin_ctx_threaded_read_path_passes() {
    // R-0006-e direction 2
    let violations = run_lint(BUILTIN_CTX_THREADED_COMPLIANT_FIXTURE);

    assert!(
        violations.is_empty(),
        "Lint must NOT flag a ctx-threaded builtin read-path (R-0006-e direction 2): \
        `ctx.workspace_id()` is referenced even though `ctx` is the second param. \
        Found violations:\n{}",
        format_violations(&violations)
    );
}

// ---------------------------------------------------------------------------
// T13.3 / R-0006-e — bootstrap pre-token carve-out pin
// ---------------------------------------------------------------------------

/// R-0006-e carve-out pin: `authentication::bootstrap` is the SINGLE pre-token
/// exception. It runs before any token (and therefore any `WorkspaceCtx`) can
/// exist — it creates the first `admin_tokens` row — so it cannot, by
/// construction, reference `ctx.workspace_id`. It legitimately takes a raw
/// `workspace_id: Uuid`.
///
/// This test pins two things so a future change can't silently fold bootstrap
/// into the type-threaded set or drop the carve-out:
///
/// 1. `builtins/authentication.rs` is NOT in the lint's scanned set (so bootstrap
///    is never classified / flagged). A future edit that adds it to the scanned
///    list of `scan_real_host_fn_files_no_where_clause_violations` would break
///    this expectation — and bootstrap would be flagged, surfacing the change.
/// 2. `bootstrap` still exists with a raw `workspace_id: Uuid` parameter and is
///    NOT threaded by `&WorkspaceCtx` — documenting the carve-out at the source.
///
/// If a future task threads `WorkspaceCtx` into bootstrap (impossible without a
/// pre-existing token), or adds `authentication.rs` to the lint scope, this test
/// fails and forces an explicit decision rather than a silent drift.
#[test]
fn bootstrap_is_the_single_pre_token_carve_out() {
    // R-0006-e carve-out
    let auth_path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR")))
        .join("builtins/authentication.rs");

    let src = std::fs::read_to_string(&auth_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", auth_path.display(), e));

    let ast = syn::parse_file(&src).expect("builtins/authentication.rs must parse");

    // Locate `pub async fn bootstrap` and inspect its first non-pool parameter.
    let mut found_bootstrap = false;
    let mut threads_workspace_ctx = false;
    let mut takes_raw_workspace_id = false;

    for item in &ast.items {
        if let Item::Fn(func) = item
            && func.sig.ident == "bootstrap"
        {
            found_bootstrap = true;
            for input in &func.sig.inputs {
                let param_str = format!("{}", quote::quote!(#input));
                if param_str.contains("WorkspaceCtx") {
                    threads_workspace_ctx = true;
                }
                // The raw carve-out param: `workspace_id : Uuid` (quote! spacing).
                if param_str.contains("workspace_id")
                    && param_str.contains("Uuid")
                    && !param_str.contains("WorkspaceCtx")
                {
                    takes_raw_workspace_id = true;
                }
            }
        }
    }

    assert!(
        found_bootstrap,
        "authentication::bootstrap must exist (R-0006-e carve-out pin)"
    );
    assert!(
        !threads_workspace_ctx,
        "authentication::bootstrap MUST NOT thread `WorkspaceCtx` (R-0006-e carve-out): \
        it runs before any token/ctx can exist (it creates the first admin_token). \
        If this fires, the pre-token carve-out is being eroded — make that decision \
        explicitly, do not fold bootstrap into the type-threaded set."
    );
    assert!(
        takes_raw_workspace_id,
        "authentication::bootstrap must keep its raw `workspace_id: Uuid` parameter \
        (R-0006-e carve-out — the documented pre-token exception)."
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

/// R-0006-d, R-0018-d, R-0006-e: run the WHERE-clause lint against the real
/// source files. All read-path host-fns must include the WHERE-clause.
///
/// # T13.3 / R-0006-e RED state (this is the failing production-scan assertion)
///
/// The `abi/*` files still have `todo!()` bodies with no SELECT, so they
/// contribute zero violations. But the builtins read-paths now in the scanned
/// set (`agents::list_by_workspace`, `sessions::list_active_by_workspace`,
/// `projects::list_by_workspace`) DO issue a `SELECT ... WHERE workspace_id = $N`
/// that binds a RAW `workspace_id: Uuid` and never references `ctx.workspace_id`.
/// Under the position-independent classifier they are read-paths, and the missing
/// clause makes this test FAIL — naming those functions. That failure IS the RED
/// for T13.3 / R-0006-e: it proves the lint catches the real tenancy bypass in
/// the live builtins, exactly the contract R-0006-e asks the lint to enforce. The
/// `projects.rs` entry in particular closes the silent-omission false-green the
/// security audit flagged.
///
/// The green phase (Forge) threads `&WorkspaceCtx` and binds
/// `ctx.workspace_id()`, after which this test passes for the right reason.
#[test]
fn scan_real_host_fn_files_no_where_clause_violations() {
    // R-0006-d, R-0018-d, R-0006-e
    // RED today: the builtins read-paths bind a raw workspace_id and do not
    // reference ctx.workspace_id, so this scan FAILS naming them. Green clears it.

    let files = vec![
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"))).join("abi/host_fns.rs"),
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"))).join("abi.rs"),
        // T13.3 / R-0006-e: builtins read-paths are now in the lint's scanned set.
        // RED today (raw workspace_id, no ctx.workspace_id); GREEN once Forge
        // threads &WorkspaceCtx and binds ctx.workspace_id().
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"))).join("builtins/agents.rs"),
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"))).join("builtins/sessions.rs"),
        // R-0006-e expansion: the security audit (dispatch 1073) flagged that
        // `builtins/projects.rs` was SILENTLY omitted from this set while it
        // contains the exact raw-`workspace_id` SELECT shape the lint exists to
        // catch (`projects::list_by_workspace`, `projects::exists`) — a false
        // green over a live R-0006-e bypass. Adding it makes this scan RED until
        // the green phase threads `&WorkspaceCtx` and binds `ctx.workspace_id()`
        // (an enumerate-the-permitted control: the scanned set must list every
        // tenant-table builtin source, not just the ones already threaded).
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"))).join("builtins/projects.rs"),
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
