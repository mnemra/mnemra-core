//! Test-only SQL-observation seam + scan-cost `statement_timeout` knobs for the
//! R-0020 artifact-list keyset-pagination read path.
//!
//! # Why this module exists (Verify-Contract white-box residual, spec line 664)
//!
//! R-0020-a / -c / -f carry **white-box** acceptance criteria that assert the
//! *emitted SQL structure* — `ORDER BY id`, `LIMIT $effective_limit + 1`, no
//! `LIMIT`-less read path, and `limit = 0` → `LIMIT 101` (never `LIMIT NULL` /
//! `LIMIT NULLIF`-zeroed). These are **not reachable by a black-box test** of the
//! page contract (a caller sees only `{ ids, has-more, next-cursor }`, never the
//! SQL). The spec's Verify Contract residual note assigns the RED phase the job of
//! provisioning a SQL-observation seam so those structural facts become observable;
//! the assertions themselves are owned at code-level review (the security reviewer,
//! Task 22), with executable backstop tests through this seam.
//!
//! # Gating discipline (R-0018 / no_test_seams)
//!
//! The whole module is `#[cfg(feature = "test-hooks")]` (declared in `plugin/mod.rs`),
//! so it is **absent from production builds** and the `no_test_seams` gate stays
//! green. Forge's GREEN call sites that feed this seam are likewise `#[cfg(feature =
//! "test-hooks")]` (see the named handoff below) — the production read path carries
//! no observation hook.
//!
//! # Author → implementer handoff (Glitch T15 RED → Forge T16/T17 GREEN)
//!
//! Glitch (this task) provisions the capture + accessor + knob **interfaces**; they
//! compile but capture nothing until Forge wires the call sites, which is the
//! right-reason RED for the white-box tests (the capture is empty → the
//! SQL-structure / GUC-placement assertions fail because the behaviour is absent).
//!
//! - **T16 (keyset read path):** call [`record_list_sql`] from the `artifact_list`
//!   body with the exact SQL string it is about to execute, the host-side
//!   `effective_limit` (`limit == 0 ? 100 : min(limit, 500)`), the actual emitted
//!   `LIMIT` value (`effective_limit + 1`, fetch-one-extra — passed as a number so the
//!   `+1` assertion survives a `LIMIT $n` bind), and whether the `id > $cursor` keyset
//!   predicate is present (`cursor = some`). Gate the call `#[cfg(feature = "test-hooks")]`.
//! - **T17 (scan-cost backstop):** inside the explicit transaction, after
//!   `SET LOCAL statement_timeout`, read back `current_setting('statement_timeout')`
//!   and pass the milliseconds to [`record_statement_timeout_in_txn`]; and compute the
//!   GUC value via [`effective_statement_timeout_ms`] so the (G1-blocked)
//!   cancellation-fires test can force a low timeout via [`set_test_statement_timeout_ms`].
//!   Gate the read-back call `#[cfg(feature = "test-hooks")]`; the
//!   `effective_statement_timeout_ms` consult is the only production-reachable seam
//!   point and is itself feature-gated at the call site (default build uses a plain
//!   `3000`).

use std::sync::{LazyLock, Mutex};

/// One captured emission of the keyset list query (R-0020-a/-c/-f observation).
///
/// Forge's T16 read-path body records this immediately before executing the SELECT
/// so a test can assert the SQL structure that a black-box caller cannot see.
#[derive(Clone, Debug)]
pub struct CapturedListSql {
    /// The exact SQL string the read path is about to execute. The white-box
    /// assertions inspect this string (`ORDER BY id`, the `LIMIT` value, the
    /// absence of any `LIMIT`-less form, the `id > ` keyset predicate).
    pub sql: String,
    /// The host-side clamp result applied for this call:
    /// `effective_limit = (limit == 0 ? 100 : min(limit, 500))` (R-0020-c/-f).
    pub effective_limit: u32,
    /// The actual `LIMIT` value the read path emits — `effective_limit + 1`
    /// (fetch-one-extra, R-0020-a). Captured as a NUMBER (not parsed from `sql`) so
    /// the fetch-one-extra `+1` assertion is robust whether GREEN inlines the LIMIT
    /// as a literal or binds it as `LIMIT $n` (the repo's parameter idiom).
    pub limit_value: u32,
    /// Whether the `id > $cursor` keyset predicate is present on this query
    /// (`true` when `cursor = some`, `false` when `cursor = none` — the first page).
    pub has_cursor_predicate: bool,
}

/// Last keyset SQL the read path emitted (None until Forge wires `record_list_sql`).
static LIST_SQL: LazyLock<Mutex<Option<CapturedListSql>>> = LazyLock::new(|| Mutex::new(None));

/// Last `current_setting('statement_timeout')` read back inside the explicit txn,
/// in milliseconds (None until Forge T17 wires `record_statement_timeout_in_txn`).
static STMT_TIMEOUT_IN_TXN: LazyLock<Mutex<Option<i64>>> = LazyLock::new(|| Mutex::new(None));

/// Test override for the read path's `statement_timeout` GUC, in milliseconds.
/// `Some(n)` forces the read path to apply `n` ms; `None` → the production value.
static TEST_STMT_TIMEOUT_MS: LazyLock<Mutex<Option<u32>>> = LazyLock::new(|| Mutex::new(None));

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ---------------------------------------------------------------------------
// SQL-observation seam — Forge (T16) writes via `record_list_sql`; tests read.
// ---------------------------------------------------------------------------

/// **Forge T16 call site.** Record the keyset SQL the read path is about to execute,
/// plus the derived clamp facts, so the white-box R-0020-a/-c/-f assertions are
/// observable. Pass `limit_value` = the actual emitted `LIMIT` (`effective_limit + 1`,
/// fetch-one-extra) so the `+1` is asserted numerically regardless of literal-vs-bind.
/// Overwrites any prior capture (last-write-wins; one list call per observed assertion
/// — tests `reset` then drive a single call).
pub fn record_list_sql(
    sql: impl Into<String>,
    effective_limit: u32,
    limit_value: u32,
    has_cursor_predicate: bool,
) {
    *lock(&LIST_SQL) = Some(CapturedListSql {
        sql: sql.into(),
        effective_limit,
        limit_value,
        has_cursor_predicate,
    });
}

/// Test accessor: take + clear the last captured keyset SQL. `None` means the read
/// path did not route any SQL through the seam — the right-reason RED before T16.
pub fn take_captured_list_sql() -> Option<CapturedListSql> {
    lock(&LIST_SQL).take()
}

/// Clear any captured SQL before driving a list call (test setup hygiene).
pub fn reset_captured_list_sql() {
    *lock(&LIST_SQL) = None;
}

// ---------------------------------------------------------------------------
// In-txn statement_timeout read-back — Forge (T17) writes; tests read.
// ---------------------------------------------------------------------------

/// **Forge T17 call site.** Inside the explicit transaction, after
/// `SET LOCAL statement_timeout`, read back `current_setting('statement_timeout')`
/// and record the milliseconds so the R-0020-d part-i GUC-placement assertion
/// (non-zero, == 3000 ms, < 5 s, observed *inside the same transaction*) is testable.
pub fn record_statement_timeout_in_txn(ms: i64) {
    *lock(&STMT_TIMEOUT_IN_TXN) = Some(ms);
}

/// Test accessor: take + clear the in-txn `statement_timeout` read-back (ms). `None`
/// means no explicit-txn GUC read-back happened — the right-reason RED before T17.
pub fn take_statement_timeout_in_txn() -> Option<i64> {
    lock(&STMT_TIMEOUT_IN_TXN).take()
}

/// Clear the in-txn `statement_timeout` read-back before driving a call.
pub fn reset_statement_timeout_in_txn() {
    *lock(&STMT_TIMEOUT_IN_TXN) = None;
}

// ---------------------------------------------------------------------------
// Reduced-statement_timeout knob — tests set; Forge (T17) reads via effective_*.
// ---------------------------------------------------------------------------

/// Test setter for the read path's `statement_timeout` (ms). `Some(n)` forces the
/// read path to apply `n` ms (so the G1-blocked cancellation-fires test can drive a
/// deliberately-low timeout below the production 3000 ms); `None` restores production.
pub fn set_test_statement_timeout_ms(ms: Option<u32>) {
    *lock(&TEST_STMT_TIMEOUT_MS) = ms;
}

/// **Forge T17 call site (feature-gated at the call).** The effective
/// `statement_timeout` the read path applies: the test override when set, else
/// `default_ms` (production 3000). The production read path uses a plain `3000` in
/// the default build; this consult exists only under `test-hooks`.
pub fn effective_statement_timeout_ms(default_ms: u32) -> u32 {
    lock(&TEST_STMT_TIMEOUT_MS).unwrap_or(default_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_sql_capture_round_trips() {
        reset_captured_list_sql();
        assert!(take_captured_list_sql().is_none());
        record_list_sql("SELECT id ... ORDER BY id LIMIT $3", 100, 101, false);
        let captured = take_captured_list_sql().expect("captured");
        assert_eq!(captured.effective_limit, 100);
        assert_eq!(captured.limit_value, 101);
        assert!(!captured.has_cursor_predicate);
        assert!(captured.sql.contains("ORDER BY id"));
        // take() clears.
        assert!(take_captured_list_sql().is_none());
    }

    #[test]
    fn statement_timeout_knob_overrides_default() {
        set_test_statement_timeout_ms(None);
        assert_eq!(effective_statement_timeout_ms(3000), 3000);
        set_test_statement_timeout_ms(Some(50));
        assert_eq!(effective_statement_timeout_ms(3000), 50);
        set_test_statement_timeout_ms(None);
    }
}
