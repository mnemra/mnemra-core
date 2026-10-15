//! WHERE-clause lint: R-0006-d, R-0018-d
//!
//! # Purpose
//!
//! The production WHERE-clause lint (`cargo test --test lint_workspace_clause`,
//! run by `verify-lint`). At V0 there is no database row-level security
//! (accepted risk R-0001), so this lint is the whole tenant boundary's
//! tripwire: a read that never threads the workspace identity must fail loudly.
//!
//! # Design: default-deny over every file (Peter, 2026-10-15)
//!
//! An earlier revision scanned five hard-coded files, skipped a missing file
//! with a message, and inspected only top-level `fn` items whose return type
//! mentioned `Option` or `Vec`. Sixteen SQL-bearing production files, every
//! impl method and every other return shape passed unexamined. This revision
//! inverts the polarity, as the R-0032 guard lint
//! (`tests/lint_engine_construction.rs`) did:
//!
//! 1. **The population is computed, not listed.** Every `.rs` file under
//!    `libs/mnemra-host/`, `cmd/` and `plugins/` (excluding each crate's own
//!    `tests/` and `target/`) is
//!    parsed. A file holding at least one SQL `SELECT` string literal (found by
//!    a token walk of the whole file, attributes skipped, no position gated) is
//!    in the population and is scanned. `EXPECTED_SQL_FILE_COUNT` pins the
//!    population exactly, so a new SQL-bearing file is noticed.
//! 2. **Every `fn` is a unit**: free functions, impl methods, trait default
//!    methods, functions inside `mod`s and nested in other functions' statement
//!    lists, whatever the return type. A unit whose body holds a SQL `SELECT`
//!    literal must read `.workspace_id` from a binding the unit DECLARES as a
//!    `WorkspaceCtx` or `CoordinationTxn` (parameter or closure parameter).
//!    Trust follows the declared type, never the receiver's name, and a type
//!    is matched by its last segment AND its path: a bare name, or a path
//!    whose leading segments are the host's own module path
//!    (`TRUSTED_WORKSPACE_TYPES`). The guest-supplied `WitWorkspaceCtx`, a
//!    `tx` of another type, a row's `workspace_id` field, and the bindgen
//!    record spelled `mnemra::host::types::WorkspaceCtx` or
//!    `types::WorkspaceCtx` all fail. Presence only: whether the value
//!    reaches the WHERE clause correctly is the semantic half, #1656.
//! 3. **Reconciliation.** The per-function SQL count must sum to the whole-file
//!    count. SQL that no function body owns (a module `const`, a `macro_rules!`
//!    body, an item-position macro) is reported as `<unattributed>` and only a
//!    whole-file exclusion can cover it.
//! 4. **Exclusions are the opt-out**, an explicit list with a reason each
//!    (`EXCLUSIONS`) and the exact number of SELECT literals each excuses. An
//!    entry whose file or function no longer exists, that suppresses nothing,
//!    or whose function gained a SELECT, FAILS.
//! 5. **Caller tripwires** (`CALL_PINS`). Exclusions that rest on "no
//!    production caller", or on callers passing a trusted workspace, pin the
//!    production call-site count, so the first new caller fails loudly.
//! 6. **Fail closed.** A file that does not parse, cannot be read, or a root
//!    that is missing is an error, never a skip.
//!
//! ## What counts as SQL
//!
//! A string literal holding the word `select`, except `GRANT`/`REVOKE` directly
//! before it and the whole-literal allow-list `PROSE_LITERALS` (log and panic
//! messages, each matched in full). Every other `select` is SQL, so an
//! unforeseen form fails closed.
//!
//! ## Not covered (named, not claimed)
//!
//! - Semantics: that the bound workspace value is the one in the WHERE clause
//!   (#1656). A function can read `ctx.workspace_id` and still query unscoped.
//! - Writes that do not contain a `select` word (`UPDATE`/`DELETE` by id).
//! - SQL assembled across several literals so that no literal holds `select`,
//!   SQL read from files or `include_str!`, and SQL built from non-literal
//!   tokens.
//! - Nested `fn` items inside inner blocks (an `if` arm, a closure): they stay
//!   part of the enclosing function's unit.
//! - `WorkspaceCtx::new` is `pub` (#1774): a forged `WorkspaceCtx` passes.
//! - Type trust is a token match, not name resolution. A bare `WorkspaceCtx`
//!   passes whatever it resolves to, so a host file that imported the bindgen
//!   record under that bare name would pass. That route is closed by pins, not
//!   by the type check: `host::types` occurs in production exactly where the
//!   aliasing re-export and the plugin sit, and `WitWorkspaceCtx` only in its
//!   alias and its guest-ignored parameters.
//! - A trusted name re-bound in the same unit (`let ctx = untrusted();`)
//!   stays trusted, because the declaration is collected and the re-binding
//!   is not.
//! - Two-segment `Call` pins (`users::get`, `users::list`, `workspaces::list`,
//!   `token::rotate`) match the written path, not the resolved item: a caller
//!   that `use`s the function and calls it bare, or passes it as a value
//!   (`.map(users::get)`), is not counted. The distinctive names are pinned as
//!   `Ident`, which counts every mention including `use` lines.
//! - `tests/` and `target/` directly under a crate root are skipped; the same
//!   names elsewhere are walked.
//! - `#[cfg(test)]` code is scanned like any other; the one unit test that
//!   needs it is listed in `EXCLUSIONS`.
//!
//! # Spec requirements traced
//!
//! - R-0006-d: all read-path host-fns SHALL include `workspace_id = ctx.workspace_id`
//!   as a WHERE-clause condition derived from WorkspaceCtx; a CI lint
//!   check SHALL assert this
//! - R-0018-d: CI lint = `cargo test --test lint_workspace_clause` (syn AST);
//!   a planted violation MUST return non-zero AND name the offending function
//! - R-0006-e: `builtins::authentication::bootstrap` is the single pre-token
//!   carve-out, pinned by `bootstrap_is_the_single_pre_token_carve_out`

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use syn::{ImplItem, Item, Stmt, TraitItem};

// ---------------------------------------------------------------------------
// Lint engine: default-deny, whole-file
// ---------------------------------------------------------------------------

/// Context key for SQL that no `fn` body owns (a module-level `const`, a
/// `macro_rules!` body, an item-position macro, an impl/trait const). No
/// per-function exclusion can name it: only a whole-file exclusion covers it.
const UNATTRIBUTED_CONTEXT: &str = "<unattributed>";

/// The only types whose `.workspace_id` is a trusted tenant scope:
/// `WorkspaceCtx` (built once at the authenticated dispatch site) and
/// `CoordinationTxn` (its `workspace_id` field is private and is set in one
/// place, `PgCoordinationStore::run_write`, from `ctx.workspace_id()`).
///
/// A receiver is trusted by what it is declared to be, never by what it is
/// called. A declared type is trusted when its last segment is the name AND
/// either it is bare (the host's `use crate::auth::workspace_ctx::WorkspaceCtx`
/// produces that everywhere) or its leading segments are exactly one of the
/// listed prefixes. The path matters because bindgen generates the GUEST
/// record as the literal `mnemra::host::types::WorkspaceCtx` (no `with:`
/// remap; only the `WitWorkspaceCtx` alias in `plugin/component.rs` keeps it
/// out), so the last segment alone is not an identity. A `mnemra::host::types::…`
/// or `types::…` path matches no prefix and is denied, as is any `tx` or `ctx`
/// of another type.
const TRUSTED_WORKSPACE_TYPES: &[(&str, &[&[&str]])] = &[
    (
        "WorkspaceCtx",
        &[
            &["crate", "auth", "workspace_ctx"],
            &["auth", "workspace_ctx"],
            &["workspace_ctx"],
        ],
    ),
    (
        "CoordinationTxn",
        &[
            &["crate", "coordination", "write_path"],
            &["coordination", "write_path"],
            &["write_path"],
        ],
    ),
];

/// True if a declared type path (`segments`, the last being the type) is
/// trusted under `TRUSTED_WORKSPACE_TYPES`.
fn is_trusted_type_path(segments: &[String]) -> bool {
    let Some((last, leading)) = segments.split_last() else {
        return false;
    };
    TRUSTED_WORKSPACE_TYPES
        .iter()
        .filter(|(name, _)| last == name)
        .any(|(_, prefixes)| {
            leading.is_empty()
                || prefixes
                    .iter()
                    .any(|p| p.iter().copied().eq(leading.iter().map(String::as_str)))
        })
}

/// A lint finding: SQL that reads a table outside a function that threads a
/// trusted workspace identity.
#[derive(Debug)]
struct LintViolation {
    /// Context key: `fn_name`, `Type::method`, or `UNATTRIBUTED_CONTEXT`.
    fn_name: String,
    /// SQL SELECT literals this unit holds. An exclusion must match it exactly,
    /// so a second SELECT added to an excluded function fails.
    sql_literals: usize,
    reason: String,
}

/// What one source file contributed.
#[derive(Debug)]
struct FileReport {
    /// SQL `SELECT` string literals anywhere in the file (attributes skipped).
    /// Non-zero puts the file in the scanned population.
    sql_literals: usize,
    violations: Vec<LintViolation>,
}

/// A function, with nested `fn`/`impl`/`mod`/`trait` statement-items carved out
/// into their own units so an inner function cannot lend its scoping to an
/// outer one's SELECT or the reverse.
struct FnUnit {
    key: String,
    /// The parameter list, where a typed receiver is declared.
    sig: TokenStream,
    body: TokenStream,
}

/// The text of a string-ish literal token (`"..."`, `r#"..."#`, `b"..."`), or
/// `None` for any other literal.
fn string_literal_content(lit: &proc_macro2::Literal) -> Option<String> {
    let repr = lit.to_string();
    let q = repr.find('"')?;
    // Everything before the first quote must be a string prefix.
    if !repr[..q].chars().all(|c| matches!(c, 'r' | 'b' | '#')) {
        return None;
    }
    // Escapes that separate words would otherwise glue `\nSELECT` into one word.
    Some(
        repr[q..]
            .replace("\\n", " ")
            .replace("\\t", " ")
            .replace("\\r", " ")
            .replace("\\\n", " "),
    )
}

/// Words that, directly before `select`, make it prose rather than a query
/// (`GRANT SELECT ON ...`).
const PROSE_BEFORE_SELECT: &[&str] = &["grant", "revoke"];
/// Whole string literals (inner text, quotes excluded) that are log or panic
/// messages mentioning `SELECT`, matched in FULL and nothing wider. A phrase
/// match after `select` (` failed`, ` canceled`) fails open on
/// `SELECT failed_at FROM t` or `select failed, id FROM t`; an exact literal
/// cannot. All six sit in `plugin/component.rs`. A new message is a loud false
/// positive to add here.
const PROSE_LITERALS: &[&str] = &[
    "artifact_get SELECT failed — fail closed (T13-g)",
    "artifact_get SELECT failed: {e}",
    "artifact_list keyset SELECT canceled by statement_timeout (57014) — scan-cost backstop fired (R-0020-d)",
    "artifact_list keyset SELECT canceled by statement_timeout (R-0020-d)",
    "artifact_list SELECT failed — fail closed (T13-g)",
    "artifact_list SELECT failed: {e}",
];

/// True if the literal holds a SQL `SELECT`. Inverted, fail-closed polarity:
/// EVERY word-boundary `select` in a string literal is SQL, except the
/// enumerated prose forms above. A new prose form is a loud false positive to
/// add to the lists; a SQL form the heuristic did not foresee (a leading
/// comment, `EXPLAIN ANALYZE SELECT`, `DECLARE c CURSOR FOR SELECT`,
/// `INSERT ... SELECT`) is caught by construction.
fn is_sql_select(content: &str) -> bool {
    if PROSE_LITERALS.iter().any(|p| content == format!("\"{p}\"")) {
        return false;
    }
    let lower = content.to_lowercase();
    let bytes = lower.as_bytes();
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut from = 0;
    while let Some(off) = lower[from..].find("select") {
        let at = from + off;
        let end = at + "select".len();
        from = end;
        let before_ok = at == 0 || !is_word(bytes[at - 1]);
        let after_ok = end >= bytes.len() || !is_word(bytes[end]);
        if !(before_ok && after_ok) {
            continue;
        }
        let prev_word: String = lower[..at]
            .trim_end()
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if !PROSE_BEFORE_SELECT.contains(&prev_word.as_str()) {
            return true;
        }
    }
    false
}

/// End index of an attribute group at `tokens[i]` (`#[..]` / `#![..]`, which
/// is also what doc comments lex to), or `None` if there is none there.
fn attribute_end(tokens: &[TokenTree], i: usize) -> Option<usize> {
    if !matches!(&tokens[i], TokenTree::Punct(p) if p.as_char() == '#') {
        return None;
    }
    let mut j = i + 1;
    if matches!(tokens.get(j), Some(TokenTree::Punct(b)) if b.as_char() == '!') {
        j += 1;
    }
    matches!(
        tokens.get(j),
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Bracket
    )
    .then_some(j + 1)
}

/// Count SQL `SELECT` literals in a token stream: every group recursively,
/// attributes skipped, nothing position-gated.
fn count_sql_literals(ts: TokenStream) -> usize {
    let tokens: Vec<TokenTree> = ts.into_iter().collect();
    let mut n = 0;
    let mut i = 0;
    while i < tokens.len() {
        if let Some(next) = attribute_end(&tokens, i) {
            i = next;
            continue;
        }
        match &tokens[i] {
            TokenTree::Group(g) => n += count_sql_literals(g.stream()),
            TokenTree::Literal(l) => {
                if string_literal_content(l).is_some_and(|c| is_sql_select(&c)) {
                    n += 1;
                }
            }
            TokenTree::Ident(_) | TokenTree::Punct(_) => {}
        }
        i += 1;
    }
    n
}

/// Collect every binding declared `name: [&] [mut] path::Type` whose type's
/// path is a `TRUSTED_WORKSPACE_TYPES` entry (`is_trusted_type_path`): function
/// parameters and closure parameters alike (both lex as `name : type`).
fn collect_trusted_receivers(ts: TokenStream, out: &mut HashSet<String>) {
    let tokens: Vec<TokenTree> = ts.into_iter().collect();
    for (i, tt) in tokens.iter().enumerate() {
        match tt {
            TokenTree::Group(g) => collect_trusted_receivers(g.stream(), out),
            TokenTree::Ident(name) => {
                // A single `:` (not the first half of `::`).
                if !matches!(
                    tokens.get(i + 1),
                    Some(TokenTree::Punct(p)) if p.as_char() == ':' && p.spacing() == Spacing::Alone
                ) {
                    continue;
                }
                let mut j = i + 2;
                // Skip `&`, a lifetime (`'` + ident) and `mut` before the path.
                loop {
                    match tokens.get(j) {
                        Some(TokenTree::Punct(p)) if p.as_char() == '&' => j += 1,
                        Some(TokenTree::Punct(p)) if p.as_char() == '\'' => j += 2,
                        Some(TokenTree::Ident(m)) if m == "mut" => j += 1,
                        _ => break,
                    }
                }
                // Walk `a :: b :: C`, keeping every segment.
                let mut segments: Vec<String> = Vec::new();
                while let Some(TokenTree::Ident(seg)) = tokens.get(j) {
                    segments.push(seg.to_string());
                    let is_path_sep = matches!(tokens.get(j + 1), Some(TokenTree::Punct(p)) if p.as_char() == ':')
                        && matches!(tokens.get(j + 2), Some(TokenTree::Punct(p)) if p.as_char() == ':');
                    if is_path_sep {
                        j += 3;
                    } else {
                        break;
                    }
                }
                if is_trusted_type_path(&segments) {
                    out.insert(name.to_string());
                }
            }
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
}

/// True if the stream contains `<trusted receiver> . workspace_id` (field
/// access, or the start of a `workspace_id()` call). Presence only: that the
/// workspace identity of a trusted type is threaded into the function. Whether
/// it reaches the WHERE clause correctly is the semantic half, #1656.
fn references_trusted_workspace_id(ts: TokenStream, trusted: &HashSet<String>) -> bool {
    let tokens: Vec<TokenTree> = ts.into_iter().collect();
    for (i, tt) in tokens.iter().enumerate() {
        match tt {
            TokenTree::Group(g) => {
                if references_trusted_workspace_id(g.stream(), trusted) {
                    return true;
                }
            }
            TokenTree::Ident(id) => {
                if trusted.contains(&id.to_string())
                    && matches!(tokens.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                    && matches!(tokens.get(i + 2), Some(TokenTree::Ident(f)) if f == "workspace_id")
                {
                    return true;
                }
            }
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
    false
}

fn type_key(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default(),
        other => other.to_token_stream().to_string().replace(' ', ""),
    }
}

fn collect_block(prefix: &str, sig: &syn::Signature, block: &syn::Block, out: &mut Vec<FnUnit>) {
    let name = sig.ident.to_string();
    let key = if prefix.is_empty() {
        name
    } else {
        format!("{prefix}::{name}")
    };
    let mut body = TokenStream::new();
    let mut nested: Vec<&Item> = Vec::new();
    for stmt in &block.stmts {
        match stmt {
            Stmt::Item(item @ (Item::Fn(_) | Item::Impl(_) | Item::Mod(_) | Item::Trait(_))) => {
                nested.push(item)
            }
            other => other.to_tokens(&mut body),
        }
    }
    out.push(FnUnit {
        key,
        sig: sig.inputs.to_token_stream(),
        body,
    });
    for item in nested {
        collect_item(item, "", out);
    }
}

fn collect_item(item: &Item, prefix: &str, out: &mut Vec<FnUnit>) {
    match item {
        Item::Fn(f) => collect_block(prefix, &f.sig, &f.block, out),
        Item::Mod(m) => {
            if let Some((_, items)) = &m.content {
                for i in items {
                    collect_item(i, "", out);
                }
            }
        }
        Item::Impl(imp) => {
            let ty = type_key(&imp.self_ty);
            for ii in &imp.items {
                if let ImplItem::Fn(f) = ii {
                    collect_block(&ty, &f.sig, &f.block, out);
                }
            }
        }
        Item::Trait(t) => {
            let ty = t.ident.to_string();
            for ti in &t.items {
                if let TraitItem::Fn(f) = ti
                    && let Some(block) = &f.default
                {
                    collect_block(&ty, &f.sig, block, out);
                }
            }
        }
        _ => {}
    }
}

/// Run the lint over one file's source.
///
/// Every `fn` (free, impl method, trait default method, nested in a `mod` or
/// in another `fn`'s statement list) whose body holds a SQL `SELECT` literal
/// is a read path, whatever its return type, and must reference
/// `<receiver>.workspace_id` where the unit declares `<receiver>` as a
/// `WorkspaceCtx` or `CoordinationTxn`. SQL that no `fn` body owns is
/// reconciled against the whole-file count and reported as
/// `UNATTRIBUTED_CONTEXT`. `Err` means the source did not parse: the caller
/// fails closed.
fn try_lint(source: &str) -> Result<FileReport, String> {
    let ast = syn::parse_file(source).map_err(|e| format!("does not parse: {e}"))?;
    let sql_literals = count_sql_literals(ast.to_token_stream());

    let mut units = Vec::new();
    for item in &ast.items {
        collect_item(item, "", &mut units);
    }

    let mut violations = Vec::new();
    let mut attributed = 0usize;
    for unit in units {
        let n = count_sql_literals(unit.body.clone());
        attributed += n;
        if n == 0 {
            continue;
        }
        let mut trusted = HashSet::new();
        collect_trusted_receivers(unit.sig.clone(), &mut trusted);
        collect_trusted_receivers(unit.body.clone(), &mut trusted);
        if !references_trusted_workspace_id(unit.body, &trusted) {
            violations.push(LintViolation {
                reason: format!(
                    "read-path `{}` holds {n} SQL SELECT literal(s) but reads `.workspace_id` \
                    from no binding declared `WorkspaceCtx` or `CoordinationTxn` — missing \
                    workspace_id WHERE-clause (R-0006-d, R-0018-d)",
                    unit.key
                ),
                fn_name: unit.key,
                sql_literals: n,
            });
        }
    }

    // The partition cannot place more SQL than the whole-file walk saw; if it
    // does, the two disagree and neither can be trusted.
    assert!(
        attributed <= sql_literals,
        "lint partition attributed {attributed} SQL literals but the whole-file walk saw \
        {sql_literals}: the attribution step is broken"
    );
    if attributed < sql_literals {
        let n = sql_literals - attributed;
        violations.push(LintViolation {
            fn_name: UNATTRIBUTED_CONTEXT.to_string(),
            sql_literals: n,
            reason: format!(
                "{n} SQL SELECT literal(s) sit outside any fn body (module const, macro body, \
                item-position macro, impl/trait const) — no workspace scoping can be checked"
            ),
        });
    }

    Ok(FileReport {
        sql_literals,
        violations,
    })
}

/// Fixture helper: violations for a source string. An unparseable source is a
/// test failure, never an empty list.
fn run_lint(source: &str) -> Vec<LintViolation> {
    try_lint(source)
        .unwrap_or_else(|e| panic!("lint source {e}"))
        .violations
}

// ---------------------------------------------------------------------------
// Population: every .rs file under the production roots that holds SQL
// ---------------------------------------------------------------------------

/// Production roots, relative to the workspace root. A `tests/` or `target/`
/// directory directly under a crate root (a directory holding `Cargo.toml`)
/// is cargo's own and is not walked; one anywhere else is walked.
/// `plugins/` holds wasm components with no database access: it is listed so
/// that claim is a check, not prose.
const PRODUCTION_ROOTS: &[&str] = &["libs/mnemra-host", "cmd", "plugins"];

/// A deliberately unscoped read. `function: None` excludes the whole file
/// (including `UNATTRIBUTED_CONTEXT` SQL); `Some(key)` excludes one function,
/// keyed `name` or `Type::name`. `sql_literals` is the exact number of SELECT
/// literals excused: a function that gains a second SELECT fails. Every entry
/// must still suppress something: one for a renamed or removed file or
/// function FAILS, never skips.
struct Exclusion {
    file: &'static str,
    function: Option<&'static str>,
    sql_literals: usize,
    reason: &'static str,
}

/// Outcome of a scan over a root set.
#[derive(Debug, Default)]
struct ScanOutcome {
    /// Relative paths of every file holding SQL, sorted.
    population: Vec<String>,
    /// `(file, context, reason)` not covered by any exclusion.
    violations: Vec<(String, String, String)>,
    /// Exclusions that suppressed nothing, or the wrong number of sites or
    /// literals.
    stale: Vec<String>,
    /// `(file, context, reason)` for every site an exclusion covered.
    excluded: Vec<(String, String, &'static str)>,
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read dir {}: {e}", dir.display()))?;
    let is_crate_root = dir.join("Cargo.toml").is_file();
    for entry in entries {
        let path = entry
            .map_err(|e| format!("dir entry in {}: {e}", dir.display()))?
            .path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if is_crate_root && (name == "tests" || name == "target") {
                continue;
            }
            collect_rs_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// Scan every `.rs` file under `roots` (relative to `base`). Fails closed: a
/// missing root, an unreadable file or an unparseable file is an `Err`, never
/// a skip.
fn scan(base: &Path, roots: &[&str], exclusions: &[Exclusion]) -> Result<ScanOutcome, String> {
    let mut files = Vec::new();
    for root in roots {
        collect_rs_files(&base.join(root), &mut files)?;
    }
    files.sort();

    let mut out = ScanOutcome::default();
    let mut suppressed_sites = vec![0usize; exclusions.len()];
    let mut suppressed_literals = vec![0usize; exclusions.len()];

    for path in &files {
        let rel = path
            .strip_prefix(base)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let src = std::fs::read_to_string(path).map_err(|e| format!("cannot read {rel}: {e}"))?;
        let report = try_lint(&src).map_err(|e| format!("{rel} {e}"))?;
        if report.sql_literals == 0 {
            continue;
        }
        out.population.push(rel.clone());
        for v in report.violations {
            let hit = exclusions.iter().position(|e| {
                e.file == rel
                    && match e.function {
                        None => true,
                        Some(f) => f == v.fn_name && v.fn_name != UNATTRIBUTED_CONTEXT,
                    }
            });
            match hit {
                Some(i) => {
                    suppressed_sites[i] += 1;
                    suppressed_literals[i] += v.sql_literals;
                    out.excluded
                        .push((rel.clone(), v.fn_name, exclusions[i].reason));
                }
                None => out.violations.push((rel.clone(), v.fn_name, v.reason)),
            }
        }
    }

    for (i, e) in exclusions.iter().enumerate() {
        let label = format!("{}::{}", e.file, e.function.unwrap_or("*"));
        if !out.population.iter().any(|p| p == e.file) {
            out.stale.push(format!(
                "{label}: file is absent or holds no SQL SELECT — renamed, removed or migrated"
            ));
        } else if e.function.is_some() && suppressed_sites[i] != 1 {
            out.stale.push(format!(
                "{label}: suppressed {} site(s), must be exactly 1 (0 = the function was \
                renamed, or its SELECTs are gone or now read a trusted workspace — confirm the \
                read is really scoped before deleting the entry; >1 = the matching key \
                collided and the exclusion widened)",
                suppressed_sites[i]
            ));
        } else if suppressed_literals[i] == 0 {
            out.stale.push(format!(
                "{label}: whole-file exclusion suppressed nothing — the file is now compliant"
            ));
        } else if suppressed_literals[i] != e.sql_literals {
            out.stale.push(format!(
                "{label}: excuses {} SELECT literal(s) but {} are unscoped now — a SELECT was \
                added or removed; review the new one for workspace scoping, then update \
                `sql_literals`",
                e.sql_literals, suppressed_literals[i]
            ));
        }
    }
    Ok(out)
}

/// `Some(message)` when the population size differs from the pin.
fn population_pin_error(actual: usize, pinned: usize) -> Option<String> {
    (actual != pinned).then(|| {
        format!(
            "SQL-bearing population is {actual} files, pinned at {pinned} — a file gained or lost \
            its SQL. Review it for workspace scoping, then bump EXPECTED_SQL_FILE_COUNT in the \
            same commit"
        )
    })
}

// ---------------------------------------------------------------------------
// Caller tripwires: production call sites of trusted-identity and no-caller
// functions
// ---------------------------------------------------------------------------

/// What a `CallPin` counts.
#[derive(Clone, Copy)]
enum PinKind {
    /// `<path>(...)`: the last segment followed by a call group, preceded by
    /// the earlier segments joined with `::` (a method call needs only the
    /// last). A definition (`fn name(`) is not a call.
    Call,
    /// Every occurrence of the single ident.
    Ident,
}

/// A pinned production call-site count. An exclusion whose reason is "no
/// production caller", or that rests on its callers passing a trusted
/// workspace, is only as good as that fact: this makes the first new caller
/// fail loudly instead of inheriting the exclusion.
struct CallPin {
    path: &'static [&'static str],
    kind: PinKind,
    expected: usize,
    why: &'static str,
}

/// Occurrences of the pinned symbol in a token stream.
fn count_symbol(ts: TokenStream, pin: &CallPin) -> usize {
    let tokens: Vec<TokenTree> = ts.into_iter().collect();
    let last = pin.path[pin.path.len() - 1];
    let mut n = 0;
    let mut i = 0;
    while i < tokens.len() {
        if let Some(next) = attribute_end(&tokens, i) {
            i = next;
            continue;
        }
        match &tokens[i] {
            TokenTree::Group(g) => n += count_symbol(g.stream(), pin),
            TokenTree::Ident(id) if id == last => match pin.kind {
                PinKind::Ident => n += 1,
                PinKind::Call => {
                    let is_call = matches!(
                        tokens.get(i + 1),
                        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis
                    );
                    let is_def =
                        i > 0 && matches!(&tokens[i - 1], TokenTree::Ident(f) if f == "fn");
                    let mut pos = i;
                    let mut path_ok = true;
                    for seg in pin.path[..pin.path.len() - 1].iter().rev() {
                        let sep = pos >= 3
                            && matches!(&tokens[pos - 1], TokenTree::Punct(p) if p.as_char() == ':')
                            && matches!(&tokens[pos - 2], TokenTree::Punct(p) if p.as_char() == ':')
                            && matches!(&tokens[pos - 3], TokenTree::Ident(s) if s == seg);
                        if !sep {
                            path_ok = false;
                            break;
                        }
                        pos -= 3;
                    }
                    if is_call && !is_def && path_ok {
                        n += 1;
                    }
                }
            },
            TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
        i += 1;
    }
    n
}

/// Occurrences of each pin over every `.rs` under `roots`, in pin order.
fn count_pins(base: &Path, roots: &[&str], pins: &[CallPin]) -> Result<Vec<usize>, String> {
    let mut files = Vec::new();
    for root in roots {
        collect_rs_files(&base.join(root), &mut files)?;
    }
    let mut counts = vec![0usize; pins.len()];
    for path in &files {
        let src = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let ts = syn::parse_file(&src)
            .map_err(|e| format!("{} does not parse: {e}", path.display()))?
            .to_token_stream();
        for (i, pin) in pins.iter().enumerate() {
            counts[i] += count_symbol(ts.clone(), pin);
        }
    }
    Ok(counts)
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
    let _query = sqlx::query("SELECT id, body FROM artifacts WHERE workspace_id = $1 AND id = $2")
        .bind(ctx.workspace_id)
        .bind(_id);
    todo!()
}
"#;

/// A non-read-path host-fn (write path, no SELECT).
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
        "Lint must NOT flag write-path host-fns (no SELECT literal). Found violations:\n{}",
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
/// into the type-threaded set or widen the carve-out:
///
/// 1. `bootstrap` is the ONLY entry in `EXCLUSIONS` for
///    `builtins/authentication.rs`. A second unscoped read in that file must
///    fail the production scan, not ride on this carve-out.
/// 2. `bootstrap` still exists with a raw `workspace_id: Uuid` parameter and is
///    NOT threaded by `&WorkspaceCtx` — documenting the carve-out at the source.
///
/// If a future task threads `WorkspaceCtx` into bootstrap (impossible without a
/// pre-existing token), this test fails and forces an explicit decision rather
/// than a silent drift.
#[test]
fn bootstrap_is_the_single_pre_token_carve_out() {
    // R-0006-e carve-out
    let auth_path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR")))
        .join("builtins/authentication.rs");

    let src = std::fs::read_to_string(&auth_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", auth_path.display(), e));

    let ast = syn::parse_file(&src).expect("builtins/authentication.rs must parse");

    let auth_exclusions: Vec<_> = EXCLUSIONS
        .iter()
        .filter(|e| e.file == "libs/mnemra-host/builtins/authentication.rs")
        .collect();
    assert_eq!(
        auth_exclusions.len(),
        1,
        "builtins/authentication.rs must carry exactly the bootstrap exclusion (R-0006-e)"
    );
    assert_eq!(auth_exclusions[0].function, Some("bootstrap"));

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

/// Workspace root: the directory holding the root `Cargo.toml`.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Count of production files holding SQL, pinned exactly (the repo's count-pin
/// convention). A new SQL-bearing file, or one that loses its SQL, moves it.
const EXPECTED_SQL_FILE_COUNT: usize = 20;

/// Deliberately unscoped reads, or reads scoped by an identity other than a
/// `WorkspaceCtx` argument. One line of reason each; see `Exclusion`.
///
/// Adding an entry is a tenancy decision with the same review as a new read
/// path, and a read of tenant data with no workspace filter is a bug to fix,
/// never an entry here. The entries are NOT uniformly safe, and the register
/// says so rather than claiming it:
/// - Groups 1-2 read with no workspace predicate by design (token identity,
///   global tables, probes). `rotate` reads a tenant table (`admin_tokens`)
///   by id alone: that is a known gap (#4014), held only by its caller pin.
/// - Group 3 carries a `workspace_id` predicate in the SQL, keyed on a value
///   the function does not receive as a trusted type. Each rests on a fact
///   about its callers, which `CALL_PINS` makes loud when it changes.
///
/// `sql_literals` is the exact number of SELECT literals each entry excuses.
const EXCLUSIONS: &[Exclusion] = &[
    // --- Group 1: token identity. The workspace is the OUTPUT of the lookup.
    Exclusion {
        file: "libs/mnemra-host/auth/token.rs",
        function: Some("lookup_by_hash"),
        sql_literals: 1,
        reason: "pre-auth token lookup by hash; yields the workspace, cannot consume one",
    },
    Exclusion {
        file: "libs/mnemra-host/auth/token.rs",
        function: Some("rotate"),
        sql_literals: 1,
        reason: "UNSCOPED read of admin_tokens by id (gap, #4014); no production caller, held by CALL_PINS",
    },
    Exclusion {
        file: "libs/mnemra-host/builtins/authentication.rs",
        function: Some("bootstrap"),
        sql_literals: 1,
        reason: "R-0006-e pre-token carve-out: runs before any ctx exists; SQL filters workspace_id = $1",
    },
    // --- Group 2: tables that are not per-workspace, and probes that return nothing.
    Exclusion {
        file: "libs/mnemra-host/builtins/users.rs",
        function: Some("get"),
        sql_literals: 1,
        reason: "users is a global identity table with no workspace_id column; no production caller, held by CALL_PINS",
    },
    Exclusion {
        file: "libs/mnemra-host/builtins/users.rs",
        function: Some("list"),
        sql_literals: 1,
        reason: "users is a global identity table with no workspace_id column; no production caller, held by CALL_PINS",
    },
    Exclusion {
        file: "libs/mnemra-host/builtins/workspaces.rs",
        function: Some("list"),
        sql_literals: 1,
        reason: "workspaces is the tenant registry itself; no production caller, held by CALL_PINS",
    },
    Exclusion {
        file: "libs/mnemra-host/schema/migrations.rs",
        function: Some("apply"),
        sql_literals: 1,
        reason: "reads schema_migrations, a global bookkeeping table",
    },
    Exclusion {
        file: "libs/mnemra-host/builtins.rs",
        function: Some("init_all"),
        sql_literals: 6,
        reason: "startup readiness probe: COUNT(*) over tenant tables, unscoped, result discarded; only table existence is checked",
    },
    Exclusion {
        file: "libs/mnemra-host/schema/init.rs",
        function: Some("health_snapshot"),
        sql_literals: 3,
        reason: "/health probe: SELECT 1, pg_extension and the default workspace row; no tenant data",
    },
    Exclusion {
        file: "libs/mnemra-host/schema/init.rs",
        function: Some("create_least_privilege_roles"),
        sql_literals: 1,
        reason: "DDL: the SELECT is a pg_roles existence guard inside a DO block",
    },
    Exclusion {
        file: "libs/mnemra-host/projection/fixture_projection.rs",
        function: Some("create_echo_fixture_projection"),
        sql_literals: 1,
        reason: "DDL for a fixture matview that aggregates ALL workspaces with no workspace dimension; tests only, held by CALL_PINS",
    },
    // --- Group 3: SQL filters workspace_id; the value arrives by a route the type check cannot see.
    Exclusion {
        file: "libs/mnemra-host/builtins/actors.rs",
        function: Some("resolve_or_create_in_txn"),
        sql_literals: 1,
        reason: "takes a raw workspace_id from its callers (ctx or CoordinationTxn derived); SQL filters workspace_id = $1; callers held by CALL_PINS",
    },
    Exclusion {
        file: "libs/mnemra-host/plugin/component.rs",
        function: Some("HostState::artifact_get"),
        sql_literals: 1,
        reason: "workspace is self.workspace_id, bound from the authenticated ctx by set_workspace_id (held by CALL_PINS); the guest's WitWorkspaceCtx is ignored by design; SQL filters workspace_id = $2",
    },
    Exclusion {
        file: "libs/mnemra-host/plugin/component.rs",
        function: Some("HostState::artifact_list"),
        sql_literals: 4,
        reason: "workspace is self.workspace_id, bound from the authenticated ctx by set_workspace_id (held by CALL_PINS); the guest's WitWorkspaceCtx is ignored by design; SQL filters workspace_id = $1",
    },
    Exclusion {
        file: "libs/mnemra-host/storage/postgres.rs",
        function: Some("PostgresTransaction::get"),
        sql_literals: 1,
        reason: "scoped by the WorkspaceId bound at begin(), which is a forgeable pub Uuid; no production caller, held by CALL_PINS; SQL filters workspace_id = $1",
    },
    Exclusion {
        file: "libs/mnemra-host/storage/postgres.rs",
        function: Some("PostgresTransaction::list"),
        sql_literals: 1,
        reason: "scoped by the WorkspaceId bound at begin(), which is a forgeable pub Uuid; no production caller, held by CALL_PINS; SQL filters workspace_id = $1",
    },
    // --- #[cfg(test)] unit test inside a production file: not compiled into the host.
    Exclusion {
        file: "libs/mnemra-host/plugin/sql_observe.rs",
        function: Some("list_sql_capture_round_trips"),
        sql_literals: 1,
        reason: "cfg(test) unit test; the SELECT is a string it hands to a capture seam, never executed",
    },
];

/// Production call-site counts that the exclusions above rest on. A first new
/// caller fails here, so it gets reviewed before it inherits an exclusion.
const CALL_PINS: &[CallPin] = &[
    CallPin {
        path: &["token", "rotate"],
        kind: PinKind::Call,
        expected: 0,
        why: "rotate reads admin_tokens by id with no workspace predicate (#4014)",
    },
    CallPin {
        path: &["users", "get"],
        kind: PinKind::Call,
        expected: 0,
        why: "users::get reads a global table by id",
    },
    CallPin {
        path: &["users", "list"],
        kind: PinKind::Call,
        expected: 0,
        why: "users::list returns every user",
    },
    CallPin {
        path: &["workspaces", "list"],
        kind: PinKind::Call,
        expected: 0,
        why: "workspaces::list returns every tenant",
    },
    CallPin {
        path: &["create_echo_fixture_projection"],
        kind: PinKind::Ident,
        expected: 1,
        why: "the fixture matview aggregates across all workspaces; the one mention is its own definition, so any caller, `use` or value reference adds one",
    },
    CallPin {
        path: &["PostgresStorage"],
        kind: PinKind::Ident,
        expected: 6,
        why: "PostgresTransaction is scoped by a forgeable WorkspaceId; every mention of its store is counted (2 in mnemra_host.rs, which holds it unused; 4 in its own module)",
    },
    CallPin {
        path: &["_storage"],
        kind: PinKind::Ident,
        expected: 3,
        why: "mnemra_host.rs holds the PostgresStorage in `_storage` (field, drop, constructor), unused. Wiring it to a caller (`_storage.begin(WorkspaceId(raw))`) must rename or touch this field, which trips this pin without adding a `PostgresStorage` token; review the caller's workspace derivation",
    },
    CallPin {
        path: &["WorkspaceId"],
        kind: PinKind::Call,
        expected: 1,
        why: "WorkspaceId is a forgeable `pub Uuid` newtype that scopes PostgresTransaction; the one match is the tuple-struct definition in storage.rs, so every construction or destructuring site adds one",
    },
    CallPin {
        path: &["resolve_or_create_in_txn"],
        kind: PinKind::Ident,
        expected: 6,
        why: "takes a raw workspace_id; each caller must derive it from a trusted ctx or txn (3 uses, 2 `use` lines, 1 definition)",
    },
    CallPin {
        path: &["set_workspace_id"],
        kind: PinKind::Ident,
        expected: 2,
        why: "binds HostState.workspace_id, the scope for artifact_get/artifact_list; the one caller derives it from the authenticated ctx (1 caller, 1 definition)",
    },
    CallPin {
        path: &["types"],
        kind: PinKind::Ident,
        expected: 5,
        why: "`mnemra::host::types` is the bindgen module whose WorkspaceCtx is the GUEST-supplied record. Host side it appears only as the aliasing re-exports and the Host impl in plugin/component.rs (3) and in the plugin's own import (1), plus the unrelated wasmtime::component::types (1). A glob or new import of it would let a bare `WorkspaceCtx` name the guest record and pass the type check",
    },
    CallPin {
        path: &["WitWorkspaceCtx"],
        kind: PinKind::Ident,
        expected: 6,
        why: "the alias that keeps the guest record out of the trusted names: 1 re-export and 5 guest-ignored parameters in plugin/component.rs. A re-alias of it `as WorkspaceCtx` adds a mention",
    },
];

/// One error line per pin whose measured count differs from its pin.
fn pin_errors(counts: &[usize], pins: &[CallPin]) -> Vec<String> {
    pins.iter()
        .zip(counts)
        .filter(|(p, n)| p.expected != **n)
        .map(|(p, n)| {
            format!(
                "CALL PIN `{}` is {n} in production, pinned at {} — a new (or removed) caller. \
                Review it against: {}. Then update the pin in the same commit",
                p.path.join("::"),
                p.expected,
                p.why
            )
        })
        .collect()
}

/// R-0006-d, R-0018-d, R-0006-e: the WHERE-clause lint over the real
/// production tree, default-deny. Every `.rs` file under the production roots
/// is parsed; every one holding SQL is scanned; anything not covered by
/// `EXCLUSIONS` is a violation.
#[test]
fn scan_real_production_tree_no_unscoped_read() {
    let outcome = scan(&workspace_root(), PRODUCTION_ROOTS, EXCLUSIONS)
        .unwrap_or_else(|e| panic!("WHERE-clause lint cannot evaluate (fails closed): {e}"));
    let counts = count_pins(&workspace_root(), PRODUCTION_ROOTS, CALL_PINS)
        .unwrap_or_else(|e| panic!("call-site pins cannot evaluate (fails closed): {e}"));

    eprintln!("population ({}):", outcome.population.len());
    for p in &outcome.population {
        eprintln!("  {p}");
    }
    eprintln!("excluded ({}):", outcome.excluded.len());
    for (file, ctx, reason) in &outcome.excluded {
        eprintln!("  [{file}] {ctx}: {reason}");
    }
    eprintln!("call pins: {counts:?}");
    let mut problems = Vec::new();
    for (file, ctx, reason) in &outcome.violations {
        problems.push(format!("  [{file}] LINT FAIL [{ctx}]: {reason}"));
    }
    for s in &outcome.stale {
        problems.push(format!("  STALE EXCLUSION {s}"));
    }
    if let Some(m) = population_pin_error(outcome.population.len(), EXPECTED_SQL_FILE_COUNT) {
        problems.push(format!("  {m}"));
    }
    for e in pin_errors(&counts, CALL_PINS) {
        problems.push(format!("  {e}"));
    }
    assert!(
        problems.is_empty(),
        "WHERE-clause lint found {} problem(s) (R-0006-d, R-0018-d):\n{}",
        problems.len(),
        problems.join("\n")
    );
}

// ---------------------------------------------------------------------------
// Seeded can-fail proofs: each control below is shown to go red on the
// violation it exists to catch, against a throwaway tree.
// ---------------------------------------------------------------------------

/// Build a throwaway workspace root holding the given `(relative path, source)`
/// files, with every production root present.
fn seeded_tree(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    for root in PRODUCTION_ROOTS {
        std::fs::create_dir_all(dir.path().join(root)).expect("mkdir root");
    }
    for (rel, src) in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, src).expect("write seeded file");
    }
    dir
}

const SEEDED_UNFILTERED_FREE_FN: &str = r#"
pub async fn leak_everything(pool: &PgPool) -> Result<(), E> {
    sqlx::query("SELECT id, body FROM artifacts").fetch_all(pool).await?;
    Ok(())
}
"#;

const SEEDED_COMPLIANT: &str = r#"
pub async fn scoped(pool: &PgPool, ctx: &WorkspaceCtx) -> Result<(), E> {
    sqlx::query("SELECT id FROM artifacts WHERE workspace_id = $1")
        .bind(ctx.workspace_id())
        .fetch_all(pool)
        .await?;
    Ok(())
}
"#;

/// A control that has never been seen to fire proves nothing: an unfiltered
/// read in a file the old hand-written list never named must be red, and a
/// scoped read beside it must stay green.
#[test]
fn seeded_unfiltered_read_in_a_new_file_is_red() {
    let tree = seeded_tree(&[
        ("libs/mnemra-host/brand_new.rs", SEEDED_UNFILTERED_FREE_FN),
        ("libs/mnemra-host/scoped.rs", SEEDED_COMPLIANT),
        ("cmd/mnemra/cmd/also_new.rs", SEEDED_UNFILTERED_FREE_FN),
    ]);
    let out = scan(tree.path(), PRODUCTION_ROOTS, &[]).expect("seeded tree scans");
    assert_eq!(
        out.population,
        [
            "cmd/mnemra/cmd/also_new.rs",
            "libs/mnemra-host/brand_new.rs",
            "libs/mnemra-host/scoped.rs"
        ]
    );
    let flagged: Vec<(&str, &str)> = out
        .violations
        .iter()
        .map(|(f, c, _)| (f.as_str(), c.as_str()))
        .collect();
    assert_eq!(
        flagged,
        [
            ("cmd/mnemra/cmd/also_new.rs", "leak_everything"),
            ("libs/mnemra-host/brand_new.rs", "leak_everything")
        ],
        "both new unfiltered reads must be named; the scoped one must not be"
    );
}

/// Positions the old lint never inspected: impl methods, trait default
/// methods, fns inside a `mod`, and a return type that is neither `Option`
/// nor `Vec`.
#[test]
fn seeded_unfiltered_read_in_impl_trait_and_mod_positions_is_red() {
    let src = r#"
pub struct Repo;
impl Repo {
    pub async fn all(&self, pool: &PgPool) -> Result<usize, E> {
        let rows = sqlx::query("SELECT id FROM artifacts").fetch_all(pool).await?;
        Ok(rows.len())
    }
}
pub trait Reader {
    async fn peek(&self, pool: &PgPool) -> bool {
        sqlx::query("SELECT 1 FROM agents").fetch_optional(pool).await.is_ok()
    }
}
pub mod inner {
    pub async fn deep(pool: &PgPool) -> () {
        let _ = sqlx::query("SELECT id FROM sessions").fetch_all(pool).await;
    }
}
"#;
    let tree = seeded_tree(&[("libs/mnemra-host/positions.rs", src)]);
    let out = scan(tree.path(), PRODUCTION_ROOTS, &[]).expect("seeded tree scans");
    let mut keys: Vec<&str> = out.violations.iter().map(|(_, c, _)| c.as_str()).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["Reader::peek", "Repo::all", "deep"]);
}

/// The scope is trusted by declared TYPE, never by receiver name. Each of
/// these reads `.workspace_id` from a binding that is not a `WorkspaceCtx` or
/// `CoordinationTxn` and must be red. The first is the guest-supplied WIT ctx,
/// the exact cross-tenant breach `plugin/trap_recovery.rs` forbids.
#[test]
fn seeded_untrusted_receivers_are_red_whatever_they_are_called() {
    let cases = [
        (
            "guest-supplied WIT ctx, ctx-suffixed name",
            "fn artifact_get(&mut self, _ctx: WitWorkspaceCtx, id: String) {\n\
             let w = _ctx.workspace_id; sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }",
        ),
        (
            "a `tx` of another type",
            "fn f(tx: &mut AuditRecord) { let w = tx.workspace_id; \
             sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }",
        ),
        (
            "a `ctx` of another type",
            "fn f(ctx: &Foo) { let w = ctx.workspace_id; \
             sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }",
        ),
        (
            "a type that merely ends in the trusted name",
            "fn f(ctx: &MyWorkspaceCtx) { let w = ctx.workspace_id; \
             sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }",
        ),
        (
            "an untyped row field",
            "fn f(pool: &PgPool) { let row = load(); let w = row.workspace_id; \
             sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }",
        ),
        (
            "the bindgen guest record, spelled in full",
            "fn f(ctx: mnemra::host::types::WorkspaceCtx) { let w = ctx.workspace_id; \
             sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }",
        ),
        (
            "the bindgen guest record, through its module",
            "fn f(ctx: types::WorkspaceCtx) { let w = ctx.workspace_id; \
             sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }",
        ),
        (
            "a trusted name under a foreign module path",
            "fn f(tx: &mut other::CoordinationTxn) { let w = tx.workspace_id(); \
             sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }",
        ),
        (
            "an untyped local named ctx",
            "fn f(pool: &PgPool) { let ctx = make(); let w = ctx.workspace_id(); \
             sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }",
        ),
    ];
    for (name, src) in cases {
        let v = run_lint(src);
        assert_eq!(v.len(), 1, "must be red: {name}");
    }
}

/// The two trusted types pass, however the receiver is named and wherever the
/// declaration sits (fn parameter, path-qualified type, closure parameter).
#[test]
fn seeded_trusted_receivers_are_green() {
    let cases = [
        (
            "WorkspaceCtx parameter",
            "fn f(pool: &PgPool, ctx: &WorkspaceCtx) { let w = ctx.workspace_id(); \
             sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }",
        ),
        (
            "CoordinationTxn parameter",
            "async fn f(tx: &mut CoordinationTxn) { let w = tx.workspace_id(); \
             sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }",
        ),
        (
            "path-qualified type, unrelated name",
            "fn f(scope: crate::auth::workspace_ctx::WorkspaceCtx) { let w = scope.workspace_id; \
             sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }",
        ),
        (
            "closure parameter",
            "fn f(store: &Store) { store.run(move |tx: &mut CoordinationTxn| { \
             let w = tx.workspace_id(); sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }); }",
        ),
    ];
    for (name, src) in cases {
        let v = run_lint(src);
        assert!(v.is_empty(), "must be green: {name}: {v:?}");
    }
}

/// The named residual, pinned so the header's claim is exactly what is
/// measured: a trusted name re-bound by `let` in the same unit stays trusted,
/// because only declarations are collected.
#[test]
fn shadowed_trusted_name_is_a_named_residual() {
    let src = "fn f(ctx: &WorkspaceCtx) { let ctx = untrusted(); let w = ctx.workspace_id; \
               sqlx::query(\"SELECT id FROM t WHERE workspace_id = $1\"); }";
    assert!(
        run_lint(src).is_empty(),
        "residual changed: shadowing is now detected; update the header's Not-covered list"
    );
}

/// An inner `fn` must not lend its trusted receiver to an outer unscoped
/// SELECT.
#[test]
fn seeded_nested_fn_does_not_lend_its_scoping_to_the_outer_read() {
    let src = r#"
pub async fn outer(pool: &PgPool, ctx: &Other) {
    fn helper(ctx: &WorkspaceCtx) -> uuid::Uuid { ctx.workspace_id() }
    let _ = sqlx::query("SELECT id FROM artifacts").fetch_all(pool).await;
}
"#;
    let report = try_lint(src).expect("parses");
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].fn_name, "outer");
}

/// SQL that no fn body owns cannot be scoping-checked; it is a violation, and
/// only a whole-file exclusion (never a per-function one) may cover it.
#[test]
fn seeded_sql_outside_any_fn_body_is_red_and_not_function_excludable() {
    let src = r#"
const LEAK: &str = "SELECT id FROM artifacts";
pub fn uses_it() -> &'static str { LEAK }
"#;
    let tree = seeded_tree(&[("libs/mnemra-host/consts.rs", src)]);
    let out = scan(tree.path(), PRODUCTION_ROOTS, &[]).expect("seeded tree scans");
    assert_eq!(out.violations.len(), 1);
    assert_eq!(out.violations[0].1, UNATTRIBUTED_CONTEXT);

    // A per-function exclusion that names the sentinel must not cover it.
    let by_fn = [Exclusion {
        file: "libs/mnemra-host/consts.rs",
        function: Some(UNATTRIBUTED_CONTEXT),
        sql_literals: 1,
        reason: "seeded",
    }];
    let out = scan(tree.path(), PRODUCTION_ROOTS, &by_fn).expect("seeded tree scans");
    assert_eq!(
        out.violations.len(),
        1,
        "a function exclusion covered unattributed SQL"
    );
    assert_eq!(out.stale.len(), 1);

    // A whole-file exclusion does.
    let whole = [Exclusion {
        file: "libs/mnemra-host/consts.rs",
        function: None,
        sql_literals: 1,
        reason: "seeded",
    }];
    let out = scan(tree.path(), PRODUCTION_ROOTS, &whole).expect("seeded tree scans");
    assert!(out.violations.is_empty());
    assert!(out.stale.is_empty());
}

/// An exclusion whose file or function is gone, or that no longer suppresses
/// anything, is stale and must be reported, never silently ignored.
#[test]
fn seeded_stale_exclusions_are_red() {
    let tree = seeded_tree(&[
        ("libs/mnemra-host/brand_new.rs", SEEDED_UNFILTERED_FREE_FN),
        ("libs/mnemra-host/scoped.rs", SEEDED_COMPLIANT),
    ]);
    let stale = [
        // File no longer exists.
        Exclusion {
            file: "libs/mnemra-host/renamed_away.rs",
            function: Some("leak_everything"),
            sql_literals: 1,
            reason: "seeded",
        },
        // File exists, function does not.
        Exclusion {
            file: "libs/mnemra-host/brand_new.rs",
            function: Some("renamed_fn"),
            sql_literals: 1,
            reason: "seeded",
        },
        // File exists and is compliant: a whole-file exclusion suppresses nothing.
        Exclusion {
            file: "libs/mnemra-host/scoped.rs",
            function: None,
            sql_literals: 1,
            reason: "seeded",
        },
    ];
    let out = scan(tree.path(), PRODUCTION_ROOTS, &stale).expect("seeded tree scans");
    assert_eq!(out.stale.len(), 3, "stale report: {:?}", out.stale);
    // The real finding is still reported: a stale entry covers nothing.
    assert_eq!(out.violations.len(), 1);
}

/// A live exclusion is quiet: positive control for the stale check above.
#[test]
fn seeded_live_exclusion_is_green() {
    let tree = seeded_tree(&[("libs/mnemra-host/brand_new.rs", SEEDED_UNFILTERED_FREE_FN)]);
    let live = [Exclusion {
        file: "libs/mnemra-host/brand_new.rs",
        function: Some("leak_everything"),
        sql_literals: 1,
        reason: "seeded",
    }];
    let out = scan(tree.path(), PRODUCTION_ROOTS, &live).expect("seeded tree scans");
    assert!(out.violations.is_empty());
    assert!(out.stale.is_empty());
    assert_eq!(out.excluded.len(), 1);
}

/// An exclusion excuses exactly the SELECTs it was written for. A second
/// unscoped SELECT added to an excluded function must go red, not ride along.
#[test]
fn seeded_second_select_in_an_excluded_function_is_red() {
    let src = r#"
pub async fn probe(pool: &PgPool) {
    let _ = sqlx::query("SELECT COUNT(*) FROM agents").fetch_one(pool).await;
    let _ = sqlx::query("SELECT id, body FROM artifacts").fetch_all(pool).await;
}
"#;
    let tree = seeded_tree(&[("libs/mnemra-host/probe.rs", src)]);
    let one = [Exclusion {
        file: "libs/mnemra-host/probe.rs",
        function: Some("probe"),
        sql_literals: 1,
        reason: "seeded: written when probe held one SELECT",
    }];
    let out = scan(tree.path(), PRODUCTION_ROOTS, &one).expect("seeded tree scans");
    assert_eq!(
        out.stale.len(),
        1,
        "the second SELECT must surface: {:?}",
        out.stale
    );

    let two = [Exclusion {
        sql_literals: 2,
        ..one_exclusion_for("probe")
    }];
    let out = scan(tree.path(), PRODUCTION_ROOTS, &two).expect("seeded tree scans");
    assert!(out.stale.is_empty() && out.violations.is_empty());
}

fn one_exclusion_for(function: &'static str) -> Exclusion {
    Exclusion {
        file: "libs/mnemra-host/probe.rs",
        function: Some(function),
        sql_literals: 1,
        reason: "seeded",
    }
}

/// The population pin goes red on a drift in either direction.
#[test]
fn seeded_population_count_pin_is_red_on_drift() {
    assert!(population_pin_error(20, 20).is_none());
    assert!(
        population_pin_error(21, 20).is_some(),
        "a new SQL file must trip the pin"
    );
    assert!(
        population_pin_error(19, 20).is_some(),
        "a file losing its SQL must trip the pin"
    );
}

/// Fail closed: a file that does not parse, or a root that does not exist, is
/// an error, never a skip.
#[test]
fn seeded_unparseable_file_and_missing_root_fail_closed() {
    let tree = seeded_tree(&[("libs/mnemra-host/broken.rs", "pub fn oops( {")]);
    let err = scan(tree.path(), PRODUCTION_ROOTS, &[]).expect_err("must not pass");
    assert!(err.contains("broken.rs"), "error must name the file: {err}");

    let tree = seeded_tree(&[]);
    let err =
        scan(tree.path(), &["libs/mnemra-host", "cmd", "gone"], &[]).expect_err("must not pass");
    assert!(err.contains("gone"), "error must name the root: {err}");
}

/// Only cargo's own `tests/` and `target/` (directly under a crate root) are
/// skipped; the same names anywhere else are walked, so a source directory
/// cannot hide SQL by being called `tests`.
#[test]
fn seeded_only_crate_level_tests_and_target_dirs_are_skipped() {
    let tree = seeded_tree(&[
        ("libs/mnemra-host/Cargo.toml", "[package]\n"),
        (
            "libs/mnemra-host/tests/skipped.rs",
            SEEDED_UNFILTERED_FREE_FN,
        ),
        (
            "libs/mnemra-host/target/skipped2.rs",
            SEEDED_UNFILTERED_FREE_FN,
        ),
        (
            "libs/mnemra-host/net/tests/walked.rs",
            SEEDED_UNFILTERED_FREE_FN,
        ),
    ]);
    let out = scan(tree.path(), PRODUCTION_ROOTS, &[]).expect("seeded tree scans");
    assert_eq!(out.population, ["libs/mnemra-host/net/tests/walked.rs"]);
}

/// The classifier treats every `select` word as SQL except enumerated prose,
/// and sees through raw strings, escapes and forms the earlier heuristic
/// missed.
#[test]
fn sql_literal_classifier_cases() {
    // Each case is lexed to a real literal token first, so the escape and
    // raw-string handling in `string_literal_content` is part of the proof.
    let classify = |src: &str| -> bool {
        let lit = src
            .parse::<TokenStream>()
            .expect("lexes")
            .into_iter()
            .find_map(|t| match t {
                TokenTree::Literal(l) => Some(l),
                _ => None,
            })
            .expect("one literal");
        string_literal_content(&lit).is_some_and(|c| is_sql_select(&c))
    };
    for sql in [
        "\"SELECT id FROM t\"",
        "\"select id from t\"",
        "\"\\nSELECT 1\"",
        "r#\"\n  SELECT id FROM t\"#",
        "\"SELECT EXISTS (SELECT 1 FROM t)\"",
        "\"WITH x AS (SELECT id FROM t) SELECT id FROM x\"",
        "\"CREATE MATERIALIZED VIEW v AS SELECT 1\"",
        "\"IF NOT EXISTS (SELECT FROM pg_roles) THEN\"",
        // Forms the earlier heuristic missed.
        "\"-- hint\\nSELECT id FROM t\"",
        "\"/* hint */ SELECT id FROM t\"",
        "\"EXPLAIN ANALYZE SELECT id FROM t\"",
        "\"DECLARE c CURSOR FOR SELECT id FROM t\"",
        "\"INSERT INTO t (a) SELECT a FROM u\"",
        // A GRANT that also holds a query is still SQL.
        "\"GRANT SELECT ON t TO r; SELECT 1\"",
        // A phrase after `select` is not prose: only the whole literals are.
        "\"SELECT failed_at FROM t\"",
        "\"SELECT canceled FROM t\"",
        "\"select failed, id FROM t\"",
        "\"artifact_get SELECT failed — fail closed (T13-g) FROM t\"",
        "\"artifact_get SELECT failed\"",
    ] {
        assert!(classify(sql), "must classify as SQL: {sql}");
    }
    for prose in PROSE_LITERALS {
        assert!(
            !classify(&format!("\"{prose}\"")),
            "must NOT classify as SQL: {prose}"
        );
    }
    for prose in [
        "\"GRANT SELECT ON workspaces TO r\"",
        "\"GRANT SELECT, INSERT, UPDATE ON ALL TABLES TO r\"",
        "\"REVOKE SELECT ON t FROM r\"",
        "\"selection\"",
        "\"preselect_id\"",
    ] {
        assert!(!classify(prose), "must NOT classify as SQL: {prose}");
    }
}

/// The trusted-receiver collector reads declarations from the token shapes
/// the real tree uses.
#[test]
fn trusted_receiver_collector_shapes() {
    let mut out = HashSet::new();
    let ts: TokenStream = "a: &WorkspaceCtx, b: &mut CoordinationTxn, \
        c: crate::auth::workspace_ctx::WorkspaceCtx, \
        d: &'a mut coordination::write_path::CoordinationTxn, e: Other, \
        f: WitWorkspaceCtx, g::h, h: x::y::WorkspaceCtx, \
        i: mnemra::host::types::WorkspaceCtx, j: types::WorkspaceCtx"
        .parse()
        .expect("lexes");
    collect_trusted_receivers(ts, &mut out);
    let mut got: Vec<&str> = out.iter().map(String::as_str).collect();
    got.sort_unstable();
    assert_eq!(got, ["a", "b", "c", "d"]);
}

/// A file whose only SQL is a `select <phrase>` query stays in the population:
/// the old after-phrase exception removed it, so the file count never moved.
#[test]
fn seeded_phrase_after_select_does_not_hide_a_file_from_the_population() {
    for sql in [
        "SELECT failed_at FROM t",
        "SELECT canceled FROM t",
        "select failed, id FROM t",
    ] {
        let src = format!("pub fn leak(pool: &PgPool) {{ sqlx::query(\"{sql}\"); }}");
        let tree = seeded_tree(&[("libs/mnemra-host/a.rs", &src)]);
        let out = scan(tree.path(), PRODUCTION_ROOTS, &[]).expect("scans");
        assert_eq!(out.population, ["libs/mnemra-host/a.rs"], "{sql}");
        assert_eq!(out.violations.len(), 1, "{sql}");
    }
}

/// The real pin whose path is exactly `path`.
fn real_pin(path: &[&str]) -> &'static CallPin {
    CALL_PINS
        .iter()
        .find(|p| p.path == path)
        .unwrap_or_else(|| panic!("no pin {path:?}"))
}

/// Every shape a new caller or import would take is counted by the pin that
/// exists to catch it, so the first one fails the real-tree test.
#[test]
fn seeded_wiring_and_import_shapes_trip_their_pins() {
    let count =
        |path: &[&str], src: &str| count_symbol(src.parse().expect("lexes"), real_pin(path));
    // Wiring the held PostgresStorage to a caller, with no new
    // `PostgresStorage` token.
    let wiring = "async fn h(&self, raw: Uuid) { \
        let tx = self._storage.as_ref().unwrap().begin(WorkspaceId(raw)).await; }";
    assert_eq!(count(&["_storage"], wiring), 1);
    assert_eq!(count(&["WorkspaceId"], wiring), 1);
    assert_eq!(
        count(&["PostgresStorage"], wiring),
        0,
        "the old pin is blind"
    );
    // Importing the guest record under the trusted bare name.
    assert_eq!(count(&["types"], "use mnemra::host::types::*;"), 1);
    assert_eq!(
        count(&["types"], "use mnemra::host::{types::WorkspaceCtx};"),
        1
    );
    assert_eq!(
        count(
            &["WitWorkspaceCtx"],
            "use c::WitWorkspaceCtx as WorkspaceCtx;"
        ),
        1
    );
    // A `use` then a bare call, or a value reference: Ident pins see both.
    assert_eq!(
        count(
            &["create_echo_fixture_projection"],
            "use p::create_echo_fixture_projection; fn f() { create_echo_fixture_projection(pool); }"
        ),
        2
    );
    assert_eq!(
        count(
            &["set_workspace_id"],
            "fn f() { xs.map(HostState::set_workspace_id); }"
        ),
        1
    );
    assert_eq!(
        count(
            &["resolve_or_create_in_txn"],
            "use a::resolve_or_create_in_txn; fn f() { g(resolve_or_create_in_txn); }"
        ),
        2
    );
}

/// The call-site counter counts calls (path-qualified where the pin is), not
/// definitions, and counts a bare ident when asked.
#[test]
fn seeded_call_pin_counts_calls_not_definitions() {
    let src: TokenStream = "fn rotate() {} fn set_workspace_id(&mut self) {} \
        fn x() { token::rotate(a); other::rotate(b); y.rotate(c); \
        s.set_workspace_id(1); let p = PostgresStorage::new(); }"
        .parse()
        .expect("lexes");
    let call = |path: &'static [&'static str]| CallPin {
        path,
        kind: PinKind::Call,
        expected: 0,
        why: "seeded",
    };
    assert_eq!(count_symbol(src.clone(), &call(&["token", "rotate"])), 1);
    assert_eq!(count_symbol(src.clone(), &call(&["set_workspace_id"])), 1);
    let ident = CallPin {
        path: &["PostgresStorage"],
        kind: PinKind::Ident,
        expected: 0,
        why: "seeded",
    };
    assert_eq!(count_symbol(src, &ident), 1);
}

/// A new caller of a pinned function goes red, and a pin that matches is quiet.
#[test]
fn seeded_new_caller_trips_the_call_pin() {
    let pins = [CallPin {
        path: &["workspaces", "list"],
        kind: PinKind::Call,
        expected: 0,
        why: "seeded",
    }];
    let quiet = seeded_tree(&[("libs/mnemra-host/a.rs", "fn a() { let _ = 1; }")]);
    let counts = count_pins(quiet.path(), PRODUCTION_ROOTS, &pins).expect("counts");
    assert!(pin_errors(&counts, &pins).is_empty());

    let caller = seeded_tree(&[(
        "libs/mnemra-host/a.rs",
        "async fn a(pool: &PgPool) { let _ = builtins::workspaces::list(pool).await; }",
    )]);
    let counts = count_pins(caller.path(), PRODUCTION_ROOTS, &pins).expect("counts");
    assert_eq!(pin_errors(&counts, &pins).len(), 1);
}

/// Positive control for every zero this lint reports: with no exclusions the
/// real tree is red on EXACTLY the excluded functions, so a green production
/// scan is the exclusions' doing and not a scan that saw nothing.
#[test]
fn real_tree_is_red_without_its_exclusions() {
    let out = scan(&workspace_root(), PRODUCTION_ROOTS, &[]).expect("real tree scans");
    let flagged: std::collections::BTreeSet<(String, String)> = out
        .violations
        .iter()
        .map(|(f, c, _)| (f.clone(), c.clone()))
        .collect();
    let excluded: std::collections::BTreeSet<(String, String)> = EXCLUSIONS
        .iter()
        .map(|e| {
            (
                e.file.to_string(),
                e.function.expect("function entry").to_string(),
            )
        })
        .collect();
    assert_eq!(
        flagged, excluded,
        "violations with no exclusions must equal the exclusion register exactly"
    );
}

/// An exclusion is a reviewed tenancy decision: each carries a reason and no
/// entry appears twice.
#[test]
fn exclusions_are_unique_and_carry_a_reason() {
    for (i, e) in EXCLUSIONS.iter().enumerate() {
        assert!(
            !e.reason.trim().is_empty(),
            "{}::{:?} has no reason",
            e.file,
            e.function
        );
        assert!(
            EXCLUSIONS[i + 1..]
                .iter()
                .all(|o| !(o.file == e.file && o.function == e.function)),
            "duplicate exclusion {}::{:?}",
            e.file,
            e.function
        );
    }
}
