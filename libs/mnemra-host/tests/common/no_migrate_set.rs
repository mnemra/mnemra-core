//! Single-sourced by-name exception list for the R-0032 engine-construction
//! guard lint (`tests/lint_engine_construction.rs`) and T7's source-derived
//! membership enumerator (R-0033).
//!
//! # AMENDMENT 2026-07-18 — function-granular exceptions
//!
//! The plan's Task 6 originally assumed every exception is **whole-binary**
//! (a test-binary name, no finer grain). That assumption broke on
//! `coordination_schema.rs`, which is already correctly migrated onto the
//! shared-engine fixture (15 call sites on `shared_engine::shared_engine()`)
//! but deliberately boots one raw `EmbeddedEngine::start()` inside
//! `coordination_migrations_idempotent_and_tables_present_after_reinit`
//! (documented in that file's own module doc, mirroring
//! `schema_init.rs::init_idempotent`). A whole-binary exception for
//! `coordination_schema` would blind the guard to any *future* real
//! violation elsewhere in that same file — exactly the vacuity failure this
//! lint exists to prevent. Peter ruled (2026-07-18): the list is keyed on
//! **`binary::function` pairs, not binary names**, with whole-binary entries
//! staying expressible as a distinct shape (not a `::function` suffix
//! convention on a plain string — see `NoMigrateEntry` below). This
//! amendment is the locked register for T6's revision round (dispatch 1708);
//! it supersedes the whole-binary-only shape dispatch 1704 shipped.
//!
//! # What this list is
//!
//! The **R-0030 "no-migrate set"** (plan term — see the plan's Task 3
//! amendment, 2026-07-03, and the Task 6 amendment above): entries excepted
//! from the R-0032 guard lint because they are legitimate test binaries that
//! do not acquire their engine (wholly, or for one specific test) through the
//! shared-engine fixture (`#[path]` inclusion of
//! `tests/common/shared_engine.rs`), and are therefore invisible to T7's
//! fixture-reference walk. Most sit in `PG_TEST_FLAGS`/`NONPG_TEST_FLAGS`;
//! `postgres_engine_cold_start` sits in neither — it runs as its own gate
//! (`verify-cold-start`), which `_ci-verify-chain` wires separately.
//!
//! # Entry granularity
//!
//! Two entry shapes, both `NoMigrateEntry` variants — a whole-binary entry
//! cannot be mistaken for a function-granular one or vice versa, because
//! they are distinct enum variants, not a string convention a caller has to
//! parse correctly:
//!
//! - **`NoMigrateEntry::WholeBinary { binary }`** — every construction site
//!   in this test binary (file stem under `libs/mnemra-host/tests/`) is
//!   excepted, because the binary's own tests never acquire the engine
//!   through the shared-engine fixture at all.
//! - **`NoMigrateEntry::Function { binary, function }`** — only the named
//!   function within that binary is excepted; every OTHER construction site
//!   in the same binary is still linted normally. Use this shape whenever a
//!   binary is otherwise correctly migrated onto the shared-engine fixture
//!   but one specific test has a genuine, documented need for an independent
//!   engine.
//!
//! `NoMigrateEntry::binary()` returns the binary-level name either way — T7's
//! R-0033 enumerator, which only computes binary-level `PG_TEST_FLAGS`
//! membership (not per-function), should call `.binary()` and never needs to
//! branch on which variant it got.
//!
//! # Function-name matching key (read before adding a `Function` entry)
//!
//! The lint walks each file's whole token stream for the forbidden symbols
//! (attributes and literals skipped), then uses the AST only to name the
//! enclosing item of each hit. That name is the **context key**
//! (`LintViolation::context` in `tests/lint_engine_construction.rs`), and the
//! `function` field of a `Function` entry must equal it exactly. Bare
//! identifiers only — no module path, no type qualifier, no `impl fn` /
//! `static` / `const` prefix, ever:
//!
//! - **A fn** (free, nested in another fn's body, `impl` method, trait
//!   default-body fn): the fn's own identifier, covering its signature and
//!   body — e.g. `coordination_migrations_idempotent_and_tables_present_after_reinit`.
//!   A fn nested as a statement inside a fn body is keyed by its own ident; an
//!   item buried inside an expression (a closure or block-expression body) is
//!   keyed by the enclosing fn.
//! - **A `const` / `static`** (item, `impl` or trait associated const): the
//!   item's own identifier, covering its type and initializer.
//! - **A macro invocation at item, `impl` or trait position, or a
//!   `macro_rules!` definition:** the macro's own identifier if it has one
//!   (`macro_rules! boot` -> `boot`), else the last segment of its path
//!   (`thread_local! { .. }` -> `thread_local`).
//! - **A `mod`, `trait`, `struct`, `enum`, `union`, `type` alias,
//!   `extern crate`, trait alias:** the item's own identifier (a `mod` or
//!   `trait` key covers only its header; its members are keyed on their own).
//! - **A `use` item:** the literal `<use>`. **An `impl` header** (generics,
//!   trait path, self type, where clause): `<impl>`. **An `extern` block:**
//!   `<extern>`. **Opaque `Verbatim` tokens:** `<verbatim>`. Those angle-bracket
//!   keys can never equal an identifier, so only a `WholeBinary` entry can
//!   except them.
//! - **A hit the attribution step could not place:** `<unattributed>`. Fails
//!   closed — no entry, `WholeBinary` included, ever excepts it.
//!
//! **Known limitation:** two same-named items in the same binary (e.g. two
//! `impl` blocks in one file both defining a method with the same name) are
//! indistinguishable to this matching key. Not the case for any entry below
//! today; if it ever becomes one, split the file or rename one of the
//! methods before adding the entry — do not widen the matcher to disambiguate,
//! that reintroduces the string-convention fragility this shape was chosen
//! to avoid.
//!
//! # Register invariants (enforced in `tests/lint_engine_construction.rs`)
//!
//! - No binary appears in both a `WholeBinary` and a `Function` entry.
//! - Every `Function` entry suppresses exactly one site on the real tree.
//! - Every entry's binary has a file under `tests/`, and suppresses within
//!   stated bounds (at least 1; `startup_run_full` EXACTLY 0, see its entry) so a renamed file cannot
//!   leave a stem-keyed exception waiting for a future file of that name.
//!
//! # Entries
//!
//! - **`WholeBinary { binary: "startup_run_full" }`** — exercises the *real*
//!   production bootstrap path (`mnemra_host::run_with`); engine
//!   construction happens inside production code (`libs/mnemra-host/src/`),
//!   out of this lint's `tests/` scan scope, so this binary's own test code
//!   has no textual match today. Named here by contract (and because T7's
//!   union needs it), not because an active match is currently suppressed.
//! - **`WholeBinary { binary: "schema_init" }`** — `schema::init::init()` is
//!   engine-hardwired (`schema/init.rs:667`), so `schema_init.rs`'s tests
//!   need a fresh, independent engine per run; deliberately NOT migrated onto
//!   the shared fixture (plan Task 3 amendment). The `init_on()` production
//!   refactor that would close this gap is deferred to task **#2069** — that
//!   deferred task is this entry's removal tripwire: once `init_on()` lands
//!   and `schema_init.rs` migrates onto the fixture, this entry comes out.
//! - **`Function { binary: "coordination_schema", function:
//!   "coordination_migrations_idempotent_and_tables_present_after_reinit" }`**
//!   — the one function in `coordination_schema.rs` that boots a fresh
//!   `EmbeddedEngine::start()` to exercise the public `init()` re-run surface
//!   twice (mirrors `schema_init.rs::init_idempotent`); the other 15 tests in
//!   that file correctly use `shared_engine::shared_engine()` and stay fully
//!   covered by the guard (see the Task 6 amendment above and
//!   `lint_engine_construction.rs`'s
//!   `function_granular_exception_leaves_other_violations_in_same_binary_flagged`
//!   regression test for the proof this stays narrow). No removal tripwire —
//!   this is a permanent, documented design choice (the idempotency test's
//!   whole point is a genuinely independent engine), not deferred debt.
//! - **`WholeBinary { binary: "postgres_engine_cold_start" }`** — Peter ruled
//!   2026-09-27 (brain `#2408` activity 5623) that the whole binary is
//!   excepted: every test in `postgres_engine_cold_start.rs` deliberately
//!   constructs a cold engine against an isolated `HOME` (`#3597`), which the
//!   shared-engine fixture cannot provide (the fixture's engine is warm by
//!   construction). Per-function entries were considered and rejected — the
//!   file's 6 real construction sites sit in 6 named items (the
//!   `HomeGuard::install` and `template_install_dir` helpers, plus 4 tests), so every new cold-start scenario added to the file would need
//!   its own new entry, whereas the whole file's contract is already "boots
//!   cold, on purpose," making the binary-level shape the accurate one. A
//!   design-lane spec erratum recording the exception was considered and
//!   deferred (not this ruling's scope). **Accepted cost:** a future
//!   non-cold-start test added to `postgres_engine_cold_start.rs` would go
//!   unlinted under this exception. **Not yet decided:** whether T7's R-0033
//!   enumerator should count this binary as its own standalone full-binary
//!   cell (the way it does `smoke_e2e`) — flagged in the same ruling, left
//!   for T7 (`#2409`).
//!
//! # Single definition site (R-0032 AC3)
//!
//! This file is the **one** definition site. Consumers `#[path]`-include it
//! (never re-declare it) via:
//!
//! ```rust,ignore
//! #[path = "common/no_migrate_set.rs"]
//! mod no_migrate_set;
//! ```
//!
//! Known consumers: `tests/lint_engine_construction.rs` (R-0032, this list's
//! origin task); T7's source-derived membership enumerator (R-0033), which
//! unions `NO_MIGRATE_SET.iter().map(NoMigrateEntry::binary)` with
//! fixture-referencing binaries to compute expected `PG_TEST_FLAGS`
//! membership.

/// One entry in the R-0030 no-migrate set: either a whole-binary exception
/// (no test in this binary uses the shared-engine fixture) or a
/// function-granular exception (only this specific function is excepted;
/// every other construction site in the binary is still linted). See the
/// module doc above for the matching-key rule and the per-entry rationale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoMigrateEntry {
    /// Exception applies to every construction site in this test binary.
    WholeBinary { binary: &'static str },
    /// Exception applies to exactly this function within this binary; every
    /// other construction site in the binary is still linted normally.
    Function {
        binary: &'static str,
        function: &'static str,
    },
}

impl NoMigrateEntry {
    /// The binary (file stem under `tests/`) this entry applies to, either
    /// variant. T7's R-0033 enumerator only needs binary-level membership —
    /// call this, never match on the variant directly.
    ///
    /// `#[allow(dead_code)]`: this lint (R-0032, the only consumer that
    /// exists yet) only ever needs `excepts()` below, which matches on
    /// `binary` + `function` together; `.binary()` is the accessor T7's
    /// R-0033 enumerator (not yet built) is documented to call. Not
    /// speculative — the module doc above names T7 as the intended
    /// consumer; remove this allow once T7 lands and calls it.
    #[allow(dead_code)]
    pub const fn binary(&self) -> &'static str {
        match self {
            NoMigrateEntry::WholeBinary { binary } => binary,
            NoMigrateEntry::Function { binary, .. } => binary,
        }
    }

    /// True if this entry excepts a construction site with the given
    /// `binary` (file stem) and `function` (bare enclosing-item identifier —
    /// see the module doc's "Function-name matching key"). A whole-binary
    /// entry matches any function in that binary; a function-granular entry
    /// matches only the named function in that binary.
    pub fn excepts(&self, binary: &str, function: &str) -> bool {
        match self {
            NoMigrateEntry::WholeBinary { binary: b } => *b == binary,
            NoMigrateEntry::Function {
                binary: b,
                function: f,
            } => *b == binary && *f == function,
        }
    }
}

/// The R-0030 no-migrate set. See the module doc above for the per-entry
/// rationale and the function-name matching-key rule.
pub const NO_MIGRATE_SET: &[NoMigrateEntry] = &[
    NoMigrateEntry::WholeBinary {
        binary: "startup_run_full",
    },
    NoMigrateEntry::WholeBinary {
        binary: "schema_init",
    },
    NoMigrateEntry::Function {
        binary: "coordination_schema",
        function: "coordination_migrations_idempotent_and_tables_present_after_reinit",
    },
    // Peter ruled 2026-09-27 (brain #2408 activity 5623) — see the module doc
    // "# Entries" section above for the full rationale and accepted cost.
    NoMigrateEntry::WholeBinary {
        binary: "postgres_engine_cold_start",
    },
];
