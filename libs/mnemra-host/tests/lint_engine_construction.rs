//! Engine-construction guard lint: R-0032, Tier-2 Task 6.
//!
//! # Purpose
//!
//! This file is the production guard lint (`cargo test --test
//! lint_engine_construction`). It FAILs if any of the three engine-
//! construction symbols — `EmbeddedEngine::start`, `start_embedded`,
//! `postgresql_embedded` — appear in **code** anywhere under
//! `libs/mnemra-host/tests/` outside the shared-engine fixture module
//! (`tests/common/shared_engine.rs`), except for the R-0030 no-migrate set
//! named in `tests/common/no_migrate_set.rs` (single-sourced, `#[path]`-
//! included below, shared with T7's R-0033 enumerator). Exceptions are
//! either whole-binary (every construction site in that test binary is
//! excepted) or function-granular (only the named function is excepted;
//! every other construction site in the same binary is still linted) — see
//! `no_migrate_set.rs`'s module doc for the `NoMigrateEntry` shape and the
//! context-key rule.
//!
//! # Design: default-deny token walk (Peter, 2026-10-14)
//!
//! Earlier revisions walked the `syn` AST and matched at enumerated
//! expression/item positions. Every position the walk did not list failed
//! open, and three review rounds found one more unlisted position each time
//! (macro invocations at item/impl/trait position, `use` and type positions,
//! `macro_rules!` bodies). This revision inverts the polarity:
//!
//! 1. **Detection is a whole-file token walk.** `scan_tokens` walks the
//!    file's `proc_macro2` token stream — every group, recursively — for the
//!    three symbols. Nothing is position-gated, so an item-position macro, a
//!    `macro_rules!` body, a `use` path, a type annotation, a
//!    qualified-self call (`<EmbeddedEngine>::start()`) and an opaque
//!    `Verbatim` region are all scanned by construction, not by listing.
//! 2. **Exactly two things are skipped:** attribute groups (`#[...]` /
//!    `#![...]` — which is also what `///` and `//!` doc comments lex to) and
//!    literals (string, char, byte, number tokens are never `Ident`s). `//`
//!    comments never reach the token stream at all.
//! 3. **The AST only attributes a hit to a context** (`attribute_item` and
//!    friends): each region of the file is rendered back to tokens and
//!    scanned under the name of its enclosing item. The key rule is in
//!    `no_migrate_set.rs` ("Function-name matching key").
//! 4. **Reconciliation closes the attribution step.** The attributed hit
//!    count per symbol must equal the whole-file count from step 1. Any hit
//!    the AST walk failed to place (an item/impl/trait variant added by a
//!    future `syn`, a region the partition misses) is emitted with context
//!    `UNATTRIBUTED_CONTEXT`, which no exception can match — it is a
//!    violation. A scan that attributes MORE than it detected panics: the
//!    partition and the detector disagree and neither can be trusted.
//!
//! Symbol matching (`scan_tokens`):
//!
//! - `start_embedded`, `postgresql_embedded`: any `Ident` token with that
//!   text, in any position (a call, a method name, a path segment, a `use`
//!   target, a definition).
//! - `EmbeddedEngine::start`: the ident `EmbeddedEngine` followed, at the same
//!   token-stream level, by the ident `start` with only `::`/angle-bracket
//!   generic-argument tokens in between (so `<EmbeddedEngine>::start` and
//!   `EmbeddedEngine::<T>::start` match; `use x::EmbeddedEngine; fn start`
//!   does not, because the `;` ends the lookahead).
//!
//! Any false positive this produces fails CLOSED (a loud violation to be
//! resolved or ruled on), the safe direction for a guard whose stated failure
//! mode is silent under-reporting.
//!
//! **What is scanned:** every token of every `.rs` file under `tests/` except
//! `common/shared_engine.rs`, other than attributes and literals —
//! including macro invocations and `macro_rules!` bodies defined in those
//! files.
//!
//! **What remains out of scope (every class below was measured or read to
//! pass; none is closeable by a syntactic walk without widening `::start`
//! matching, which would hit unrelated `EpochTickThread::start` /
//! `RefreshWorker::start`):**
//! - Macros defined OUTSIDE `tests/` (in `libs/mnemra-host/src/` or a
//!   dependency) whose expansion constructs an engine: the call site shows
//!   only the macro's arguments. Closing that needs real macro expansion.
//! - Aliasing: `use ...::EmbeddedEngine as Eng; Eng::start()`, `type Eng2 =
//!   EmbeddedEngine; Eng2::start()`, and re-export under another name from
//!   library code. The alias declaration of a matched symbol is itself a hit
//!   only where it names `postgresql_embedded` or `start_embedded`.
//! - `Self::start()` inside `impl ... for EmbeddedEngine`.
//! - Argument assembly across tokens: `call_assoc!(EmbeddedEngine, start)`
//!   (a narrowing against an earlier matcher that ignored punctuation
//!   between the two idents), and token pasting (`paste!`, `concat_idents!`).
//! - Tests that call `mnemra_host::run_with` or other library entry points
//!   that build an engine in production code.
//! - Attribute arguments (skipped by design: `#[cfg_attr(.., foo(..))]`,
//!   proc-macro attributes), and `#[path = "..."]` modules pointing outside
//!   `tests/` (their files are not walked unless also under `tests/`).
//! - Code outside `libs/mnemra-host/tests/` (e.g. `cmd/`, `src/`).
//!
//! Raw identifiers are normalised (`r#start_embedded`, `EmbeddedEngine::r#start`
//! still match).
//!
//! This lint catches accidental drift by a cooperating author, not
//! adversarial evasion; the list above is the evasion boundary.
//!
//! # Obligations on what must never match
//!
//! - **(i) ordinary comments** — `//` and `/* */` are discarded by the lexer.
//! - **(ii) doc comments** — `///`/`//!` lex to `#[doc = "..."]` attribute
//!   groups, skipped by `scan_tokens`.
//! - **(iii) literal contents** — a `Literal` token is never an `Ident`; this
//!   is also what keeps the lint from self-flagging on its own fixture
//!   consts, see `lint_does_not_self_flag_on_its_own_fixture_literals`.
//!
//! # Spec requirements traced
//!
//! R-0032 (this lint, AC1-AC3); R-0033 (T7 references `no_migrate_set.rs`,
//! not re-declared here).

#[path = "common/no_migrate_set.rs"]
mod no_migrate_set;

use no_migrate_set::{NO_MIGRATE_SET, NoMigrateEntry};
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use syn::ext::IdentExt;
use syn::{ImplItem, Item, Stmt, TraitItem};

// ---------------------------------------------------------------------------
// Lint engine: whole-file token walk + AST attribution
// ---------------------------------------------------------------------------

/// Context given to a hit the AST attribution step could not place. No
/// exception entry may match it (see `is_excepted`): an unattributable hit is
/// a violation by construction.
const UNATTRIBUTED_CONTEXT: &str = "<unattributed>";

/// A lint finding: a forbidden engine-construction symbol, with the context
/// key of its enclosing item (rule: `no_migrate_set.rs`, "Function-name
/// matching key").
#[derive(Debug, Clone, PartialEq, Eq)]
struct LintViolation {
    symbol: String,
    context: String,
}

impl LintViolation {
    fn new(symbol: &str, context: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            context: context.to_string(),
        }
    }
}

const SYM_ENGINE_START: &str = "EmbeddedEngine::start";
const SYM_START_EMBEDDED: &str = "start_embedded";
const SYM_PG_EMBEDDED: &str = "postgresql_embedded";

/// The attribution step: partition the parsed file's items into hits keyed
/// by context. Injectable so a test can run a no-op attributor and prove
/// `reconcile` is wired in (the only thing standing between a broken
/// partition and a silent pass).
type Attributor = fn(&syn::File, &mut Vec<LintViolation>);

fn attribute_file(ast: &syn::File, out: &mut Vec<LintViolation>) {
    for item in &ast.items {
        attribute_item(item, out);
    }
}

/// Run the engine-construction lint over a string of Rust source code.
///
/// Returns every occurrence of the three forbidden symbols anywhere in the
/// source's tokens except inside attributes and literals, each with its
/// context key. `Err` means the source did not parse or lex — the caller
/// names the file and fails closed. This function has no notion of "the
/// fixture module" or "the exception list" — those are scan-level (per-file)
/// concerns applied by the production scan test below.
fn try_lint(source: &str) -> Result<Vec<LintViolation>, String> {
    run_lint_with(source, attribute_file)
}

fn run_lint_with(source: &str, attributor: Attributor) -> Result<Vec<LintViolation>, String> {
    let ast = syn::parse_file(source).map_err(|e| format!("does not parse: {e}"))?;
    let whole: TokenStream = source.parse().map_err(|e| format!("does not lex: {e}"))?;

    // Authority: what is in the file at all.
    let mut detected = Vec::new();
    scan_tokens(whole, &mut |sym| detected.push(sym));

    // Attribution: the same tokens, partitioned by enclosing item.
    let mut violations = Vec::new();
    attributor(&ast, &mut violations);

    reconcile(&detected, &mut violations);
    Ok(violations)
}

/// Fixture-test convenience: panics on an unparseable fixture.
fn run_lint(source: &str) -> Vec<LintViolation> {
    try_lint(source).unwrap_or_else(|e| panic!("lint fixture {e}"))
}

/// Reconciliation: per symbol, the attributed hit count must equal the
/// whole-file detected count. A shortfall is emitted as `UNATTRIBUTED_CONTEXT`
/// violations (fail closed); a surplus means the partition and the detector
/// disagree, and panics.
fn reconcile(detected: &[&'static str], violations: &mut Vec<LintViolation>) {
    for sym in [SYM_ENGINE_START, SYM_START_EMBEDDED, SYM_PG_EMBEDDED] {
        let want = detected.iter().filter(|s| **s == sym).count();
        let got = violations.iter().filter(|v| v.symbol == sym).count();
        assert!(
            got <= want,
            "guard lint attribution found {got} `{sym}` hit(s) but the whole-file \
            token walk found only {want} — the partition and the detector disagree"
        );
        for _ in got..want {
            violations.push(LintViolation::new(sym, UNATTRIBUTED_CONTEXT));
        }
    }
}

/// Walk a token stream for the forbidden symbols, calling `hit` with the
/// symbol name for each. Skips attribute groups and never inspects literals.
fn scan_tokens(ts: TokenStream, hit: &mut dyn FnMut(&'static str)) {
    let tokens: Vec<TokenTree> = ts.into_iter().collect();
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            // Attribute: `#` `[...]` or `#` `!` `[...]`. Everything else
            // starting with `#` (e.g. quote-style interpolation) falls through
            // as an ordinary punct.
            TokenTree::Punct(p) if p.as_char() == '#' => {
                let mut j = i + 1;
                if matches!(tokens.get(j), Some(TokenTree::Punct(b)) if b.as_char() == '!') {
                    j += 1;
                }
                if matches!(
                    tokens.get(j),
                    Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Bracket
                ) {
                    i = j + 1;
                    continue;
                }
            }
            TokenTree::Group(g) => scan_tokens(g.stream(), hit),
            TokenTree::Ident(id) => {
                let name = id.unraw().to_string();
                if name == SYM_START_EMBEDDED {
                    hit(SYM_START_EMBEDDED);
                } else if name == SYM_PG_EMBEDDED {
                    hit(SYM_PG_EMBEDDED);
                } else if name == "EmbeddedEngine" && engine_start_follows(&tokens[i + 1..]) {
                    hit(SYM_ENGINE_START);
                }
            }
            // Obligation (iii): literal contents are never inspected.
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
        i += 1;
    }
}

/// After an `EmbeddedEngine` ident: true if the ident `start` follows at the
/// same level, reached only through `:` puncts and `<...>` generic arguments.
fn engine_start_follows(rest: &[TokenTree]) -> bool {
    let mut depth = 0usize;
    let mut colon = false;
    for tt in rest {
        match tt {
            TokenTree::Punct(p) => match p.as_char() {
                '<' => depth += 1,
                '>' => depth = depth.saturating_sub(1),
                ':' => colon = colon || depth == 0,
                _ if depth > 0 => {}
                _ => return false,
            },
            TokenTree::Ident(_) if depth > 0 => {}
            TokenTree::Ident(id) => return colon && id.unraw() == "start",
            TokenTree::Group(_) | TokenTree::Literal(_) if depth > 0 => {}
            TokenTree::Group(_) | TokenTree::Literal(_) => return false,
        }
    }
    false
}

fn scan_into(ts: TokenStream, ctx: &str, out: &mut Vec<LintViolation>) {
    scan_tokens(ts, &mut |sym| out.push(LintViolation::new(sym, ctx)));
}

/// Context key for an item-position / impl-position / trait-position macro
/// invocation or `macro_rules!` definition: the macro's own ident if it has
/// one (`macro_rules! name`), else the last segment of its path
/// (`thread_local`).
fn macro_key(ident: Option<&syn::Ident>, mac: &syn::Macro) -> String {
    match ident {
        Some(i) => i.to_string(),
        None => mac
            .path
            .segments
            .last()
            .map_or_else(|| UNATTRIBUTED_CONTEXT.to_string(), |s| s.ident.to_string()),
    }
}

/// Context key for an item with no fn/macro rule: its own ident where it has
/// one, else a fixed `<kind>` tag no exception entry can name.
fn item_key(item: &Item) -> String {
    match item {
        Item::Const(i) => i.ident.to_string(),
        Item::Static(i) => i.ident.to_string(),
        Item::Struct(i) => i.ident.to_string(),
        Item::Enum(i) => i.ident.to_string(),
        Item::Union(i) => i.ident.to_string(),
        Item::Type(i) => i.ident.to_string(),
        Item::TraitAlias(i) => i.ident.to_string(),
        Item::ExternCrate(i) => i.ident.to_string(),
        Item::Macro(i) => macro_key(i.ident.as_ref(), &i.mac),
        Item::Use(_) => "<use>".to_string(),
        Item::ForeignMod(_) => "<extern>".to_string(),
        Item::Verbatim(_) => "<verbatim>".to_string(),
        // An item kind added by a future `syn`: no key rule. The hit still
        // exists in the whole-file walk, so reconciliation reports it.
        _ => UNATTRIBUTED_CONTEXT.to_string(),
    }
}

/// Attribute every token of `item` to a context. Fn-like items scan their own
/// header under their ident and recurse into nested items; container items
/// (`mod`, `impl`, `trait`) scan their header and recurse. Everything else is
/// scanned whole under `item_key`.
fn attribute_item(item: &Item, out: &mut Vec<LintViolation>) {
    match item {
        Item::Fn(f) => {
            let ctx = f.sig.ident.to_string();
            let mut header = f.clone();
            header.block.stmts.clear();
            scan_into(header.to_token_stream(), &ctx, out);
            attribute_block(&f.block, &ctx, out);
        }
        Item::Mod(m) => {
            let ctx = m.ident.to_string();
            let mut header = m.clone();
            header.content = None;
            scan_into(header.to_token_stream(), &ctx, out);
            if let Some((_, items)) = &m.content {
                items.iter().for_each(|i| attribute_item(i, out));
            }
        }
        Item::Impl(imp) => {
            let mut header = imp.clone();
            header.items.clear();
            scan_into(header.to_token_stream(), "<impl>", out);
            imp.items.iter().for_each(|i| attribute_impl_item(i, out));
        }
        Item::Trait(t) => {
            let ctx = t.ident.to_string();
            let mut header = t.clone();
            header.items.clear();
            scan_into(header.to_token_stream(), &ctx, out);
            t.items.iter().for_each(|i| attribute_trait_item(i, out));
        }
        other => scan_into(other.to_token_stream(), &item_key(other), out),
    }
}

fn attribute_impl_item(item: &ImplItem, out: &mut Vec<LintViolation>) {
    match item {
        ImplItem::Fn(m) => {
            let ctx = m.sig.ident.to_string();
            let mut header = m.clone();
            header.block.stmts.clear();
            scan_into(header.to_token_stream(), &ctx, out);
            attribute_block(&m.block, &ctx, out);
        }
        ImplItem::Const(c) => scan_into(c.to_token_stream(), &c.ident.to_string(), out),
        ImplItem::Type(t) => scan_into(t.to_token_stream(), &t.ident.to_string(), out),
        ImplItem::Macro(m) => scan_into(m.to_token_stream(), &macro_key(None, &m.mac), out),
        ImplItem::Verbatim(ts) => scan_into(ts.clone(), "<verbatim>", out),
        // A variant added by a future `syn`: left unscanned here, reported by
        // run_lint's reconciliation as UNATTRIBUTED_CONTEXT.
        _ => {}
    }
}

fn attribute_trait_item(item: &TraitItem, out: &mut Vec<LintViolation>) {
    match item {
        TraitItem::Fn(m) => {
            let ctx = m.sig.ident.to_string();
            let mut header = m.clone();
            if let Some(block) = &mut header.default {
                block.stmts.clear();
            }
            scan_into(header.to_token_stream(), &ctx, out);
            if let Some(block) = &m.default {
                attribute_block(block, &ctx, out);
            }
        }
        TraitItem::Const(c) => scan_into(c.to_token_stream(), &c.ident.to_string(), out),
        TraitItem::Type(t) => scan_into(t.to_token_stream(), &t.ident.to_string(), out),
        TraitItem::Macro(m) => scan_into(m.to_token_stream(), &macro_key(None, &m.mac), out),
        TraitItem::Verbatim(ts) => scan_into(ts.clone(), "<verbatim>", out),
        // Same fail-closed route as attribute_impl_item.
        _ => {}
    }
}

/// A block's statements: a nested item gets its own context (a fn nested in a
/// fn body is keyed by its own ident); every other statement belongs to the
/// enclosing fn's context — including items nested inside an expression
/// (closure or block-expression bodies), which are not separately keyed.
fn attribute_block(block: &syn::Block, ctx: &str, out: &mut Vec<LintViolation>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Item(item) => attribute_item(item, out),
            other => scan_into(other.to_token_stream(), ctx, out),
        }
    }
}

/// True if any entry in `entries` excepts a construction site with the given
/// `binary` (file stem under `tests/`) and `function` (the context key — see
/// `no_migrate_set.rs`'s "Function-name matching key"). A whole-binary entry
/// matches any context in that binary; a function-granular entry matches only
/// the named one — see `NoMigrateEntry::excepts`. An unattributable hit is
/// never excepted, by either variant (fail closed). Parameterized over
/// `entries` so the production scan and the regression tests share one
/// implementation.
fn is_excepted(entries: &[NoMigrateEntry], binary: &str, function: &str) -> bool {
    function != UNATTRIBUTED_CONTEXT && entries.iter().any(|e| e.excepts(binary, function))
}

// ---------------------------------------------------------------------------
// Fixture source strings
// ---------------------------------------------------------------------------

/// Top-level fn constructing the engine directly via the path-call form.
/// Baseline violation shape — must be caught.
const VIOLATION_TOP_LEVEL_PATH_CALL_FIXTURE: &str = r#"
struct EmbeddedEngine;
impl EmbeddedEngine {
    async fn start() -> Result<Self, ()> { Ok(EmbeddedEngine) }
}

async fn boot_directly() {
    let _engine = EmbeddedEngine::start().await;
}
"#;

/// `start_embedded` via the path-call (associated-fn) form.
const VIOLATION_START_EMBEDDED_PATH_CALL_FIXTURE: &str = r#"
struct PostgresStorage;
impl PostgresStorage {
    async fn start_embedded() -> Result<Self, ()> { Ok(PostgresStorage) }
}

async fn boot_storage() {
    let _s = PostgresStorage::start_embedded().await.unwrap();
}
"#;

/// `start_embedded` via the method-call form (`x.start_embedded()`), where
/// the symbol is a bare ident after a `.` rather than a path segment.
const VIOLATION_START_EMBEDDED_METHOD_CALL_FIXTURE: &str = r#"
struct Factory;
impl Factory {
    async fn start_embedded(&self) -> Result<(), ()> { Ok(()) }
}

async fn boot_via_method(f: Factory) {
    f.start_embedded().await.unwrap();
}
"#;

/// `postgresql_embedded` referenced as a crate/module path segment.
const VIOLATION_POSTGRESQL_EMBEDDED_FIXTURE: &str = r#"
mod postgresql_embedded {
    pub struct PostgreSql;
    impl PostgreSql {
        pub fn new() -> Self { PostgreSql }
    }
}

fn boot_raw() {
    let _pg = postgresql_embedded::PostgreSql::new();
}
"#;

/// Construction inside an `impl` method (not a top-level free fn) — the
/// exact gap the precedent's top-level-only `Item::Fn` walk would miss.
const VIOLATION_IMPL_METHOD_FIXTURE: &str = r#"
struct EmbeddedEngine;
impl EmbeddedEngine {
    async fn start() -> Result<Self, ()> { Ok(EmbeddedEngine) }
}

struct Harness;
impl Harness {
    async fn boot(&self) {
        let _engine = EmbeddedEngine::start().await;
    }
}
"#;

/// Construction inside a nested item — a `mod` block nested under a
/// top-level item, exercising `Item::Mod` recursion.
const VIOLATION_NESTED_MOD_FIXTURE: &str = r#"
mod fixtures {
    struct PostgresStorage;
    impl PostgresStorage {
        async fn start_embedded() -> Result<Self, ()> { Ok(PostgresStorage) }
    }

    pub async fn boot() {
        let _s = PostgresStorage::start_embedded().await.unwrap();
    }
}
"#;

/// Construction inside a fn locally-defined inside another fn's body —
/// exercises the nested-item arm of `attribute_block` (keyed by `inner`).
const VIOLATION_LOCALLY_NESTED_FN_FIXTURE: &str = r#"
struct EmbeddedEngine;
impl EmbeddedEngine {
    async fn start() -> Result<Self, ()> { Ok(EmbeddedEngine) }
}

async fn outer() {
    async fn inner() {
        let _engine = EmbeddedEngine::start().await;
    }
    inner().await;
}
"#;

/// Construction hidden inside macro invocation token streams (F3) — both the
/// bare-statement form (`Stmt::Macro`) and the expression form nested inside
/// a `let` binding (`Expr::Macro`). The realistic future path this guards
/// against: `futures::join!(EmbeddedEngine::start(), …)`, `tokio::select!`,
/// or a similar third-party macro wrapping a real construction call.
const VIOLATION_MACRO_INVOCATION_FIXTURE: &str = r#"
struct EmbeddedEngine;
impl EmbeddedEngine {
    async fn start() -> Result<Self, ()> { Ok(EmbeddedEngine) }
}

async fn other() {}

async fn boot_via_macro_statement() {
    futures::join!(EmbeddedEngine::start(), other());
}

async fn boot_via_macro_expr() {
    let _joined = futures::join!(EmbeddedEngine::start(), other());
}
"#;

/// Negative control for F3 (obligation iii through the new macro path): a
/// macro invocation whose argument is a string *literal* containing the
/// forbidden symbol text — `assert_eq!(s, "start_embedded")` — must NOT
/// match. `scan_tokens` only matches `TokenTree::Ident`, never
/// `TokenTree::Literal`, so obligation (iii) holds inside macro arguments
/// too. Paired with a real violation for the same non-vacuity reason as the
/// top-level obligation tests below.
const MACRO_STRING_LITERAL_MENTION_WITH_REAL_VIOLATION_FIXTURE: &str = r#"
struct EmbeddedEngine;
impl EmbeddedEngine {
    async fn start() -> Result<Self, ()> { Ok(EmbeddedEngine) }
}

fn check_symbol_name(s: &str) {
    assert_eq!(s, "start_embedded");
}

async fn real_violation() {
    let _engine = EmbeddedEngine::start().await;
}
"#;

/// Construction referenced from an associated-const initializer inside an
/// `impl` block, and from a trait default-const's value expression (F6) —
/// symmetric with the existing top-level `Item::Const`/`Item::Static`
/// coverage, which associated consts previously lacked.
const VIOLATION_ASSOC_CONST_FIXTURE: &str = r#"
struct Harness;
impl Harness {
    const MARKER: fn() = postgresql_embedded::marker;
}

trait Bootable {
    const DEFAULT_MARKER: fn() = postgresql_embedded::marker;
}
"#;

/// A compliant fixture: acquires the engine through the fixture accessor
/// pattern (mirrors real `shared_engine::shared_engine()` usage) — no
/// forbidden symbol appears. Proves the lint doesn't false-positive on the
/// sanctioned acquisition path.
const COMPLIANT_FIXTURE_ACCESSOR_FIXTURE: &str = r#"
mod shared_engine {
    pub struct EmbeddedEngine;
    pub async fn shared_engine() -> &'static EmbeddedEngine {
        unimplemented!()
    }
}

async fn get_engine() {
    let _engine = shared_engine::shared_engine().await;
}
"#;

/// Negative control (obligation i): an ordinary `//` comment mentions all
/// three forbidden symbols. Paired with a REAL, differently-shaped
/// violation in the same fixture so the assertion proves narrowness (the
/// comment is excluded) rather than vacuity (an unconditional empty Vec
/// would also make a comment-only fixture "pass").
const COMMENT_ONLY_MENTION_WITH_REAL_VIOLATION_FIXTURE: &str = r#"
// This comment mentions EmbeddedEngine::start, start_embedded, and
// postgresql_embedded — the lint must NOT match text inside a `//` comment.

struct EmbeddedEngine;
impl EmbeddedEngine {
    async fn start() -> Result<Self, ()> { Ok(EmbeddedEngine) }
}

async fn real_violation() {
    let _engine = EmbeddedEngine::start().await;
}
"#;

/// Negative control (obligation ii): both a module-level `//!` doc comment
/// and an item-level `///` doc comment mention the forbidden symbols — both
/// desugar to `#[doc = "..."]` attributes, which the scanner never reads.
/// Paired with a real violation for the same non-vacuity reason as above.
const DOC_COMMENT_MENTION_WITH_REAL_VIOLATION_FIXTURE: &str = r#"
//! Module doc: mentions start_embedded and postgresql_embedded and
//! EmbeddedEngine::start in prose. Desugars to #[doc = "..."] attributes —
//! must NOT match.

/// Item doc: also mentions EmbeddedEngine::start here. Must NOT match either.
fn documented_noop() {
    let _ = 1 + 1;
}

fn real_violation() {
    let _pg = postgresql_embedded::PostgreSql::new();
}
"#;

/// Negative control (obligation iii): string literals contain the forbidden
/// symbol text verbatim. Paired with a real violation. This is also the
/// direct counter-proof against a `contains`-based text match (which the
/// precedent's technique uses and this lint deliberately does not) — a
/// `contains` scan would flag every string here; the token walk must not.
const STRING_LITERAL_MENTION_WITH_REAL_VIOLATION_FIXTURE: &str = r#"
fn holds_symbol_names_as_strings() {
    let _a = "EmbeddedEngine::start";
    let _b = "start_embedded";
    let _c = "postgresql_embedded";
}

async fn real_violation() {
    start_embedded().await.unwrap();
}
"#;

// ---------------------------------------------------------------------------
// Fixture tests
// ---------------------------------------------------------------------------

#[test]
fn top_level_path_call_construction_is_caught() {
    let violations = run_lint(VIOLATION_TOP_LEVEL_PATH_CALL_FIXTURE);
    assert!(
        violations
            .iter()
            .any(|v| v.symbol == "EmbeddedEngine::start"),
        "must catch EmbeddedEngine::start() at top level; got {violations:?}"
    );
}

#[test]
fn start_embedded_path_call_construction_is_caught() {
    let violations = run_lint(VIOLATION_START_EMBEDDED_PATH_CALL_FIXTURE);
    assert!(
        violations.iter().any(|v| v.symbol == "start_embedded"),
        "must catch PostgresStorage::start_embedded() (path-call form); got {violations:?}"
    );
}

#[test]
fn start_embedded_method_call_construction_is_caught() {
    let violations = run_lint(VIOLATION_START_EMBEDDED_METHOD_CALL_FIXTURE);
    assert!(
        violations.iter().any(|v| v.symbol == "start_embedded"),
        "must catch x.start_embedded() (method-call form, not just path-call); got {violations:?}"
    );
}

#[test]
fn postgresql_embedded_reference_is_caught() {
    let violations = run_lint(VIOLATION_POSTGRESQL_EMBEDDED_FIXTURE);
    assert!(
        violations.iter().any(|v| v.symbol == "postgresql_embedded"),
        "must catch postgresql_embedded::PostgreSql::new(); got {violations:?}"
    );
}

#[test]
fn construction_inside_impl_method_is_caught() {
    let violations = run_lint(VIOLATION_IMPL_METHOD_FIXTURE);
    assert!(
        violations
            .iter()
            .any(|v| v.symbol == "EmbeddedEngine::start" && v.context.contains("boot")),
        "must catch construction inside an impl method, not just top-level fns \
        (the precedent's top-level-only Item::Fn walk misses this); got {violations:?}"
    );
}

#[test]
fn construction_inside_nested_mod_item_is_caught() {
    let violations = run_lint(VIOLATION_NESTED_MOD_FIXTURE);
    assert!(
        violations.iter().any(|v| v.symbol == "start_embedded"),
        "must catch construction inside a nested `mod` item; got {violations:?}"
    );
}

#[test]
fn construction_inside_locally_nested_fn_is_caught() {
    let violations = run_lint(VIOLATION_LOCALLY_NESTED_FN_FIXTURE);
    assert!(
        violations
            .iter()
            .any(|v| v.symbol == "EmbeddedEngine::start"),
        "must catch construction inside a fn nested inside another fn's body; got {violations:?}"
    );
}

#[test]
fn construction_inside_macro_invocation_is_caught() {
    // A real construction inside a `futures::join!`/`tokio::select!`-shaped
    // macro, in statement and expression position, is found by the token walk.
    let violations = run_lint(VIOLATION_MACRO_INVOCATION_FIXTURE);
    let hits: Vec<&LintViolation> = violations
        .iter()
        .filter(|v| v.symbol == "EmbeddedEngine::start")
        .collect();
    assert_eq!(
        hits.len(),
        2,
        "must catch EmbeddedEngine::start() inside both a bare macro-invocation \
        statement (Stmt::Macro) and a macro invocation used as an expression \
        (Expr::Macro), e.g. futures::join!(EmbeddedEngine::start(), ...); \
        got {violations:?}"
    );
    assert!(
        hits.iter()
            .any(|v| v.context.contains("boot_via_macro_statement")),
        "must catch the Stmt::Macro form; got {hits:?}"
    );
    assert!(
        hits.iter()
            .any(|v| v.context.contains("boot_via_macro_expr")),
        "must catch the Expr::Macro form; got {hits:?}"
    );
}

#[test]
fn macro_argument_string_literal_mention_does_not_match_but_real_violation_still_caught() {
    // F3 obligation-(iii) regression proof: the macro token-walk must not
    // match Literal tokens, only Ident tokens.
    let violations = run_lint(MACRO_STRING_LITERAL_MENTION_WITH_REAL_VIOLATION_FIXTURE);
    assert_eq!(
        violations.len(),
        1,
        "assert_eq!(s, \"start_embedded\") must not match through the macro \
        token-walk (obligation iii — Literal tokens are never inspected); \
        exactly the real `real_violation` construction must be caught; \
        got {violations:?}"
    );
    assert_eq!(violations[0].symbol, "EmbeddedEngine::start");
    assert!(violations[0].context.contains("real_violation"));
}

#[test]
fn construction_inside_assoc_const_initializer_is_caught() {
    // F6: ImplItem::Const / TraitItem::Const bodies, symmetric with the
    // existing top-level Item::Const / Item::Static coverage.
    let violations = run_lint(VIOLATION_ASSOC_CONST_FIXTURE);
    let hits: Vec<&LintViolation> = violations
        .iter()
        .filter(|v| v.symbol == "postgresql_embedded")
        .collect();
    assert_eq!(
        hits.len(),
        2,
        "must catch postgresql_embedded referenced from both an impl \
        associated-const initializer (MARKER) and a trait default-const \
        value expression (DEFAULT_MARKER); got {violations:?}"
    );
    assert!(
        hits.iter().any(|v| v.context == "MARKER"),
        "must catch the ImplItem::Const initializer; got {hits:?}"
    );
    assert!(
        hits.iter().any(|v| v.context == "DEFAULT_MARKER"),
        "must catch the TraitItem::Const default-value expression; got {hits:?}"
    );
}

#[test]
fn compliant_fixture_accessor_usage_passes() {
    let violations = run_lint(COMPLIANT_FIXTURE_ACCESSOR_FIXTURE);
    assert!(
        violations.is_empty(),
        "must NOT flag acquisition through the fixture accessor pattern; got {violations:?}"
    );
}

#[test]
fn ordinary_comment_mention_does_not_match_but_real_violation_still_caught() {
    // Obligation (i). Non-vacuity: this fixture pairs the comment with a real
    // violation. A lint that matched nothing at all would still "pass" a
    // bare comment-only fixture — asserting exactly one match (the real one)
    // rules that out.
    let violations = run_lint(COMMENT_ONLY_MENTION_WITH_REAL_VIOLATION_FIXTURE);
    assert_eq!(
        violations.len(),
        1,
        "the // comment must not match (obligation i); exactly the real \
        `real_violation` construction must be caught — zero matches would be \
        vacuous, more than one means the comment leaked through; got {violations:?}"
    );
    assert_eq!(violations[0].symbol, "EmbeddedEngine::start");
    assert!(violations[0].context.contains("real_violation"));
}

#[test]
fn doc_comment_mention_does_not_match_but_real_violation_still_caught() {
    // Obligation (ii). Same non-vacuity shape as the comment test above.
    let violations = run_lint(DOC_COMMENT_MENTION_WITH_REAL_VIOLATION_FIXTURE);
    assert_eq!(
        violations.len(),
        1,
        "module-level //! and item-level /// doc comments must not match \
        (obligation ii — they desugar to #[doc = \"...\"] attributes); \
        exactly the real `real_violation` construction must be caught; got {violations:?}"
    );
    assert_eq!(violations[0].symbol, "postgresql_embedded");
    assert!(violations[0].context.contains("real_violation"));
}

#[test]
fn string_literal_mention_does_not_match_but_real_violation_still_caught() {
    // Obligation (iii). Same non-vacuity shape. Also the direct counter-proof
    // against the precedent's contains()-based technique: a text scan would
    // flag all three string literals here; the token walk must not.
    let violations = run_lint(STRING_LITERAL_MENTION_WITH_REAL_VIOLATION_FIXTURE);
    assert_eq!(
        violations.len(),
        1,
        "string-literal contents must not match (obligation iii); exactly the \
        real `real_violation` construction must be caught, not the three \
        string literals holding the symbol names as text; got {violations:?}"
    );
    assert_eq!(violations[0].symbol, "start_embedded");
    assert!(violations[0].context.contains("real_violation"));
}

/// Item-position macro invocation (`thread_local!`) — the realistic vector the
/// old AST-position walk left open.
const ITEM_MACRO_FIXTURE: &str = r#"
thread_local! { static X: u32 = EmbeddedEngine::start(); }
"#;

/// Macro invocations inside an `impl` block and a `trait` block.
const IMPL_AND_TRAIT_MACRO_FIXTURE: &str = r#"
struct Harness;
impl Harness {
    boot_macro!(EmbeddedEngine::start());
}
trait Bootable {
    trait_boot_macro!(start_embedded());
}
"#;

/// A `use` of the crate, plus a later construction through the imported name.
/// The `use` is the hit; the bare `PostgreSQL::default()` is not a matched
/// symbol (alias boundary, file doc).
const USE_PATH_FIXTURE: &str = r#"
use postgresql_embedded::PostgreSQL;

fn boot() {
    let _pg = PostgreSQL::default();
}
"#;

/// Type positions: a `let` annotation and a return type.
const TYPE_POSITION_FIXTURE: &str = r#"
fn annotated() {
    let _e: postgresql_embedded::PostgreSQL = make();
}

fn returns_it() -> postgresql_embedded::PostgreSQL {
    make()
}
"#;

/// Qualified-self call.
const QUALIFIED_SELF_FIXTURE: &str = r#"
async fn boot() {
    let _e = <EmbeddedEngine>::start().await;
}
"#;

/// Construction inside a locally-defined `macro_rules!` body.
const MACRO_RULES_BODY_FIXTURE: &str = r#"
macro_rules! boot_engine {
    () => {
        EmbeddedEngine::start()
    };
}
"#;

/// Negative: the symbols in an `///` doc comment, a `//!` doc comment, a
/// `#[doc]` attribute, another attribute carrying the idents, an inner
/// attribute, and string literals — plus one real violation so a scanner that
/// matches nothing cannot pass.
const NEGATIVE_POSITIONS_WITH_REAL_VIOLATION_FIXTURE: &str = r##"
//! Inner doc mentions EmbeddedEngine::start and postgresql_embedded.
#![allow(unused, reason = "start_embedded")]

/// Outer doc mentions start_embedded and EmbeddedEngine::start.
#[doc = "postgresql_embedded EmbeddedEngine::start"]
#[cfg_attr(test, my_attr(start_embedded, postgresql_embedded))]
fn documented() {
    let _a = "EmbeddedEngine::start";
    let _b = r#"start_embedded postgresql_embedded"#;
}

fn real_violation() {
    let _e = EmbeddedEngine::start();
}
"##;

fn hits(source: &str) -> Vec<(String, String)> {
    run_lint(source)
        .into_iter()
        .map(|v| (v.symbol, v.context))
        .collect()
}

fn hit(symbol: &str, context: &str) -> (String, String) {
    (symbol.to_string(), context.to_string())
}

const RAW_IDENT_FIXTURE: &str = r#"
fn raw() {
    let _a = r#start_embedded();
    let _b = EmbeddedEngine::r#start();
    let _c = r#postgresql_embedded::X;
}
"#;

#[test]
fn raw_identifiers_are_normalised_and_caught() {
    assert_eq!(
        hits(RAW_IDENT_FIXTURE),
        vec![
            hit(SYM_START_EMBEDDED, "raw"),
            hit(SYM_ENGINE_START, "raw"),
            hit(SYM_PG_EMBEDDED, "raw"),
        ]
    );
}

#[test]
fn reconcile_is_wired_into_run_lint() {
    // A no-op attributor places nothing; every detected hit must surface as
    // unattributed. Deleting the `reconcile` call from `run_lint_with` turns
    // this red.
    fn noop(_: &syn::File, _: &mut Vec<LintViolation>) {}
    let got = run_lint_with(MACRO_RULES_BODY_FIXTURE, noop).expect("fixture parses");
    assert_eq!(
        got,
        vec![LintViolation::new(SYM_ENGINE_START, UNATTRIBUTED_CONTEXT)]
    );
    let got = run_lint_with(RAW_IDENT_FIXTURE, noop).expect("fixture parses");
    assert_eq!(
        got,
        vec![
            LintViolation::new(SYM_ENGINE_START, UNATTRIBUTED_CONTEXT),
            LintViolation::new(SYM_START_EMBEDDED, UNATTRIBUTED_CONTEXT),
            LintViolation::new(SYM_PG_EMBEDDED, UNATTRIBUTED_CONTEXT),
        ]
    );
    let whole = [NoMigrateEntry::WholeBinary { binary: "b" }];
    assert!(
        got.iter().all(|v| !is_excepted(&whole, "b", &v.context)),
        "a WholeBinary entry must not except unattributed hits"
    );
}

#[test]
fn unparseable_source_is_an_error_not_a_pass() {
    assert!(try_lint("fn broken( {").is_err());
}

#[test]
fn item_position_macro_is_caught_keyed_by_macro_name() {
    assert_eq!(
        hits(ITEM_MACRO_FIXTURE),
        vec![hit(SYM_ENGINE_START, "thread_local")]
    );
}

#[test]
fn impl_and_trait_position_macros_are_caught_keyed_by_macro_name() {
    assert_eq!(
        hits(IMPL_AND_TRAIT_MACRO_FIXTURE),
        vec![
            hit(SYM_ENGINE_START, "boot_macro"),
            hit(SYM_START_EMBEDDED, "trait_boot_macro"),
        ]
    );
}

#[test]
fn use_of_postgresql_embedded_is_caught_keyed_use() {
    assert_eq!(hits(USE_PATH_FIXTURE), vec![hit(SYM_PG_EMBEDDED, "<use>")]);
}

#[test]
fn type_position_references_are_caught_keyed_by_enclosing_fn() {
    assert_eq!(
        hits(TYPE_POSITION_FIXTURE),
        vec![
            hit(SYM_PG_EMBEDDED, "annotated"),
            hit(SYM_PG_EMBEDDED, "returns_it"),
        ]
    );
}

#[test]
fn qualified_self_construction_is_caught() {
    assert_eq!(
        hits(QUALIFIED_SELF_FIXTURE),
        vec![hit(SYM_ENGINE_START, "boot")]
    );
}

#[test]
fn macro_rules_body_construction_is_caught_keyed_by_macro_name() {
    assert_eq!(
        hits(MACRO_RULES_BODY_FIXTURE),
        vec![hit(SYM_ENGINE_START, "boot_engine")]
    );
}

#[test]
fn doc_comments_attributes_and_literals_never_match_but_real_violation_does() {
    assert_eq!(
        hits(NEGATIVE_POSITIONS_WITH_REAL_VIOLATION_FIXTURE),
        vec![hit(SYM_ENGINE_START, "real_violation")]
    );
}

#[test]
fn unattributed_hit_is_a_violation_no_exception_can_match() {
    // A hit the whole-file walk saw but the AST partition did not place.
    let mut violations = Vec::new();
    reconcile(&[SYM_ENGINE_START], &mut violations);
    assert_eq!(
        violations,
        vec![LintViolation::new(SYM_ENGINE_START, UNATTRIBUTED_CONTEXT)]
    );
    let whole = [NoMigrateEntry::WholeBinary { binary: "b" }];
    assert!(
        !is_excepted(&whole, "b", UNATTRIBUTED_CONTEXT),
        "a WholeBinary entry must not except an unattributed hit"
    );
    assert!(
        is_excepted(&whole, "b", "any_fn"),
        "control: it does except a keyed one"
    );
}

#[test]
#[should_panic(expected = "the partition and the detector disagree")]
fn attribution_surplus_over_detection_panics() {
    let mut violations = vec![LintViolation::new(SYM_ENGINE_START, "f")];
    reconcile(&[], &mut violations);
}

// ---------------------------------------------------------------------------
// Self-flag check — the lint must not flag its own fixture literals
// ---------------------------------------------------------------------------

/// The check does not self-flag on its own literal symbol references
/// (Test Expectations). This file's fixture consts above are string
/// literals containing the forbidden symbol text, and its matcher code
/// contains string comparisons like `name == SYM_START_EMBEDDED` against
/// consts — fixtures are `Literal` tokens, never inspected as identifiers
/// (obligation iii), so scanning this file's own source must return zero
/// violations. Independent
/// of `scan_real_test_tree_no_engine_construction_outside_fixture` below
/// (which legitimately finds violations elsewhere in the tree) — this test
/// scans only this one file and must always stay green.
#[test]
fn lint_does_not_self_flag_on_its_own_fixture_literals() {
    let path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR")))
        .join("tests/lint_engine_construction.rs");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
    let violations = run_lint(&src);
    assert!(
        violations.is_empty(),
        "the lint must not self-flag on its own fixture string-literal symbol \
        references or its own string-comparison matcher code (obligation iii); \
        found: {violations:?}"
    );

    // F9 non-vacuity: prove the empty result above isn't from a lint that
    // catches nothing. Append a real, differently-shaped violation to this
    // file's own source (in memory only — never written to disk) and confirm
    // it IS caught, so this test fails in both directions like the three
    // obligation tests above.
    let mut src_with_planted_violation = src.clone();
    src_with_planted_violation.push_str(
        "\nasync fn self_flag_non_vacuity_probe() { \
        let _e = EmbeddedEngine::start().await; }\n",
    );
    let probe_violations = run_lint(&src_with_planted_violation);
    assert!(
        probe_violations
            .iter()
            .any(|v| v.symbol == "EmbeddedEngine::start"
                && v.context == "self_flag_non_vacuity_probe"),
        "non-vacuity check: a real violation deliberately appended to this \
        file's own source must be caught — if it isn't, the assertion above \
        is vacuously passing rather than proving obligation (iii); got \
        {probe_violations:?}"
    );
}

// ---------------------------------------------------------------------------
// Production scan: real files under libs/mnemra-host/tests/
// ---------------------------------------------------------------------------

/// Path (relative to `tests/`) of the one legitimate construction site — the
/// shared-engine fixture module itself. Excluded from the scan by path, not
/// by name-list, since it is not a "caller" being excepted; it IS the
/// fixture.
const FIXTURE_MODULE_RELATIVE_PATH: &str = "common/shared_engine.rs";

/// Recursively collect every `.rs` file under `dir`.
fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read dir {}: {}", dir.display(), e));
    for entry in entries {
        let entry = entry.expect("dir entry read failed");
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// R-0032 AC1/AC2: the guard lint over the real `libs/mnemra-host/tests/`
/// tree. Scans every `.rs` file (recursively — a standing guard must cover
/// files added after this lint lands, not an enumerated list someone has to
/// remember to update), excluding the fixture module by path and applying
/// the R-0030 no-migrate set (`no_migrate_set.rs`, single-sourced) per
/// construction site — not per file — so a function-granular entry excepts
/// only its named function, never the whole file.
///
/// Invariants closed against silent-failure classes rather than accepted as
/// documented limitations: (F7) `files` is count-pinned EXACTLY (the repo's
/// count-pin convention) so a near-empty or silently-shrunk collection can't
/// pass green having scanned less; (F5) file stems (the exception-matching
/// key) are asserted unique, so a new `tests/common/<stem>.rs` can't
/// silently inherit an unrelated `WholeBinary` exemption; (F2) every
/// `Function` entry's suppression count is asserted `== 1` — 0 means the
/// entry rotted, >1 means the matching key collided and the exception
/// silently widened; (L5) every entry has a minimum suppression count
/// (`suppression_bounds`) so a renamed file fails loudly instead of leaving a
/// stem-keyed exception waiting for a future file of that name.
/// `coordination_schema_exception_stays_function_granular` and
/// `no_binary_is_both_whole_binary_and_function_excepted` below pin the
/// register's shape.
#[test]
fn scan_real_test_tree_no_engine_construction_outside_fixture() {
    let tests_dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"))).join("tests");
    let mut files = Vec::new();
    collect_rs_files(&tests_dir, &mut files);
    files.sort();

    // F7: exact count pin, mirroring the project's own count-pin convention
    // (PG_TEST_FLAGS/NONPG_TEST_FLAGS). Adding or removing a `.rs` file under
    // tests/ means bumping this pin in the same commit; a drift in either
    // direction is a scan whose population changed without anyone saying so.
    const EXPECTED_TEST_FILE_COUNT: usize = 57;
    assert_eq!(
        files.len(),
        EXPECTED_TEST_FILE_COUNT,
        "R-0032 guard lint's tests/ file collection is {} files, pinned at {} — \
        bump EXPECTED_TEST_FILE_COUNT in the same commit that adds or removes a \
        .rs file under tests/",
        files.len(),
        EXPECTED_TEST_FILE_COUNT
    );

    // F5: exception keys are file stems (NoMigrateEntry's `binary` field),
    // not full paths. A new tests/common/<stem>.rs sharing a stem with a
    // WholeBinary-excepted top-level binary would silently inherit that
    // binary's whole-file exemption. Fail loud on any stem collision rather
    // than let it ride quietly.
    {
        let mut stems: Vec<&str> = files
            .iter()
            .filter_map(|p| p.file_stem().and_then(|s| s.to_str()))
            .collect();
        stems.sort_unstable();
        if let Some(w) = stems.windows(2).find(|w| w[0] == w[1]) {
            panic!(
                "two files under tests/ share the file stem \"{}\" — the R-0030 \
                no-migrate set keys exceptions by file stem, so this pairing \
                would let one file silently inherit the other's exception. \
                Rename one file (see F5, dispatch 1713 review) before adding \
                this pairing.",
                w[0]
            );
        }
    }

    let mut all_violations: Vec<(String, LintViolation)> = Vec::new();
    // F2/L5: per-entry suppression counts, checked below against
    // `suppression_bounds`.
    let mut suppressed: Vec<usize> = vec![0; NO_MIGRATE_SET.len()];

    for path in &files {
        let rel = path.strip_prefix(&tests_dir).unwrap_or(path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        if rel_str == FIXTURE_MODULE_RELATIVE_PATH {
            continue; // the fixture module is the one legitimate construction site
        }

        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        let src = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
        let found =
            try_lint(&src).unwrap_or_else(|e| panic!("R-0032 guard lint: {} {e}", path.display()));
        for v in found {
            if let Some(i) = NO_MIGRATE_SET
                .iter()
                .position(|e| is_excepted(std::slice::from_ref(e), stem, &v.context))
            {
                suppressed[i] += 1;
                continue; // R-0030 no-migrate set: whole-binary or function-granular
            }
            all_violations.push((rel_str.clone(), v));
        }
    }

    if !all_violations.is_empty() {
        let msg: Vec<String> = all_violations
            .iter()
            .map(|(file, v)| format!("  [{file}] {} (in {})", v.symbol, v.context))
            .collect();
        panic!(
            "R-0032 guard lint found {} engine-construction violation(s) outside the \
            shared-engine fixture and not on the R-0030 no-migrate set \
            (tests/common/no_migrate_set.rs):\n{}\n\n\
            If this is a genuine new finding (not a known exception), route it back \
            to the owning feature spec, or get a Peter-approved exception added to \
            no_migrate_set.rs — do not except it away unilaterally here.",
            all_violations.len(),
            msg.join("\n")
        );
    }

    for (i, e) in NO_MIGRATE_SET.iter().enumerate() {
        assert!(
            files
                .iter()
                .any(|p| p.file_stem().and_then(|s| s.to_str()) == Some(e.binary())),
            "no-migrate entry {e:?} names a binary with no file under tests/ — the \
            file was renamed or removed; update or remove the entry"
        );
        let (min, max) = suppression_bounds(e);
        assert!(
            (min..=max).contains(&suppressed[i]),
            "no-migrate entry {e:?} suppressed {} site(s), outside its bounds \
            {min}..={max} — the file was renamed, the construction migrated, or \
            new sites appeared; update or remove the entry (a stem-keyed \
            exception with no site is waiting for a future file of that name)",
            suppressed[i]
        );
        if let NoMigrateEntry::Function { binary, function } = e {
            assert_eq!(
                suppressed[i], 1,
                "Function exception {binary}::{function} must except exactly ONE \
                construction site: 0 means the entry has rotted (typo, or the site \
                was migrated — remove it); >1 means the bare-identifier matching \
                key has collided and the exception has silently widened (split the \
                file or rename, per no_migrate_set.rs's matching-key doc)"
            );
        }
    }
}

/// Inclusive (min, max) number of sites each no-migrate entry must suppress
/// on the real tree (L5, so a renamed file fails loudly). The exhaustive
/// match means a new entry shape must state its bounds here.
/// - `startup_run_full`: EXACTLY 0. It exercises production bootstrap
///   (`mnemra_host::run_with`), whose engine construction lives in
///   `libs/mnemra-host/src/`, outside this scan; a site appearing there means
///   the entry's rationale no longer holds. Its stem must still exist (checked
///   by the caller), so a rename cannot leave a stem-keyed exception waiting.
/// - Other `WholeBinary`: at least 1.
/// - `Function`: exactly 1 (a second site means the bare-ident key collided).
fn suppression_bounds(entry: &NoMigrateEntry) -> (usize, usize) {
    match entry {
        NoMigrateEntry::WholeBinary {
            binary: "startup_run_full",
        } => (0, 0),
        NoMigrateEntry::WholeBinary { .. } => (1, usize::MAX),
        NoMigrateEntry::Function { .. } => (1, 1),
    }
}

/// Binaries that appear in both a `WholeBinary` and a `Function` entry. A
/// `Function` entry beside a `WholeBinary` for the same binary is dead — the
/// whole-binary entry already excepts everything — and is the way a
/// function-granular ruling gets silently widened by appending one line.
fn mixed_granularity_binaries(entries: &[NoMigrateEntry]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = entries
        .iter()
        .filter_map(|e| match e {
            NoMigrateEntry::Function { binary, .. } => Some(*binary),
            NoMigrateEntry::WholeBinary { .. } => None,
        })
        .filter(|b| {
            entries
                .iter()
                .any(|e| matches!(e, NoMigrateEntry::WholeBinary { binary } if binary == b))
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// M2: no binary may be both whole-binary and function-excepted in the real
/// register. Fixture-local lists prove the detector fires.
#[test]
fn no_binary_is_both_whole_binary_and_function_excepted() {
    assert_eq!(
        mixed_granularity_binaries(NO_MIGRATE_SET),
        Vec::<&str>::new(),
        "a binary has both a WholeBinary and a Function entry in NO_MIGRATE_SET"
    );
    let seeded = [
        NoMigrateEntry::Function {
            binary: "coordination_schema",
            function: "f",
        },
        NoMigrateEntry::WholeBinary {
            binary: "coordination_schema",
        },
        NoMigrateEntry::WholeBinary { binary: "other" },
    ];
    assert_eq!(
        mixed_granularity_binaries(&seeded),
        vec!["coordination_schema"],
        "control: the detector must flag a seeded mixed pair"
    );
}

/// Permanent pin for Peter's 2026-07-18 ruling (see `no_migrate_set.rs`'s
/// "AMENDMENT 2026-07-18" doc): `coordination_schema`'s exception MUST stay
/// function-granular (F1, dispatch 1713 review). Before this test, nothing
/// executable asserted the real `NO_MIGRATE_SET` shape — flipping the entry
/// to `WholeBinary` left every other test green (the mechanism test below
/// uses a fixture-local list, deliberately independent of the production
/// set), silently reverting the ruling and reopening the blind spot to the
/// file's other 15 sites.
#[test]
fn coordination_schema_exception_stays_function_granular() {
    assert!(
        NO_MIGRATE_SET.contains(&NoMigrateEntry::Function {
            binary: "coordination_schema",
            function: "coordination_migrations_idempotent_and_tables_present_after_reinit",
        }),
        "the coordination_schema exception must stay function-granular (T6 amendment \
        2026-07-18, Peter-approved) — a WholeBinary entry here would blind the guard \
        to the file's other 15 fixture-based sites; got {NO_MIGRATE_SET:?}"
    );
}

// ---------------------------------------------------------------------------
// Anti-vacuity regression: function-granular exception stays narrow
// ---------------------------------------------------------------------------

/// Permanent regression proof for the T6 amendment (dispatch 1708,
/// 2026-07-18): a `NoMigrateEntry::Function` exception must except ONLY the
/// named function, never blind the guard to other construction sites in the
/// same binary. This is the exact failure a whole-binary exception for
/// `coordination_schema` would have caused (see the file-level doc comment)
/// and the thing a future function-granular entry could silently regress
/// into if `is_excepted`/`NoMigrateEntry::excepts` were ever loosened to
/// match on binary alone.
///
/// Deliberately uses a fixture-local `NoMigrateEntry` list, not the real
/// `NO_MIGRATE_SET` — this proves the exception-check MECHANISM stays
/// narrow, independent of what's currently in the production list (which
/// today has no second construction site in `coordination_schema.rs` to
/// exercise this against).
#[test]
fn function_granular_exception_leaves_other_violations_in_same_binary_flagged() {
    const FIXTURE: &str = r#"
struct EmbeddedEngine;
impl EmbeddedEngine {
    async fn start() -> Result<Self, ()> { Ok(EmbeddedEngine) }
}

async fn excepted_reinit_fn() {
    let _engine = EmbeddedEngine::start().await;
}

async fn non_excepted_violation_fn() {
    let _engine = EmbeddedEngine::start().await;
}
"#;

    let entries = [NoMigrateEntry::Function {
        binary: "pretend_binary",
        function: "excepted_reinit_fn",
    }];

    let violations = run_lint(FIXTURE);
    assert_eq!(
        violations.len(),
        2,
        "sanity check: run_lint itself is exception-agnostic and must see \
        BOTH raw violations before any filtering is applied; got {violations:?}"
    );

    let remaining: Vec<&LintViolation> = violations
        .iter()
        .filter(|v| !is_excepted(&entries, "pretend_binary", &v.context))
        .collect();

    assert_eq!(
        remaining.len(),
        1,
        "a function-granular exception must except ONLY the named function \
        (excepted_reinit_fn) — the other violation in the SAME binary \
        (non_excepted_violation_fn) must still be flagged, not blinded away; \
        got {remaining:?}"
    );
    assert_eq!(
        remaining[0].context, "non_excepted_violation_fn",
        "the surviving violation must be the non-excepted one, not the excepted one"
    );

    // Cross-check: a WholeBinary entry for the same binary WOULD swallow
    // both — proving the two variants really do behave differently, not
    // just carry different field names.
    let whole_binary_entries = [NoMigrateEntry::WholeBinary {
        binary: "pretend_binary",
    }];
    let remaining_whole_binary: Vec<&LintViolation> = violations
        .iter()
        .filter(|v| !is_excepted(&whole_binary_entries, "pretend_binary", &v.context))
        .collect();
    assert!(
        remaining_whole_binary.is_empty(),
        "sanity check: a WholeBinary entry for the same binary must except \
        BOTH functions (this is the exact blind-spot the amendment moves \
        away from for coordination_schema); got {remaining_whole_binary:?}"
    );
}
