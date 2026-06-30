//! R-0020 artifact-list keyset pagination — WHITE-BOX SQL-structure + GUC-placement
//! suite (Task 15 RED). Gated behind `test-hooks`; runs under
//! `cargo test --features test-hooks`.
//!
//! # Why this file is separate from the black-box suite
//!
//! R-0020-a / -c / -f assert the *emitted SQL structure* (`ORDER BY id`,
//! `LIMIT $effective_limit + 1`, no `LIMIT`-less query, `limit=0` → `LIMIT 101`), and
//! R-0020-d part (i) asserts the `statement_timeout` GUC is placed inside the explicit
//! transaction. None of these is reachable by a black-box page-contract test — a caller
//! sees only `{ ids, has-more, next-cursor }`, never the SQL or the GUC. The spec's
//! Verify-Contract white-box residual (line 664) assigns the RED phase a
//! SQL-observation seam; this file is its executable backstop (the assertions are also
//! owned at code-level review, Task 22).
//!
//! # Right-reason RED
//!
//! Each test drives `echo.list` through the MCP surface, then reads the
//! `mnemra_host::plugin::sql_observe` seam. Today the read path routes nothing through
//! the seam (T14 placeholder body), so the capture is empty → every test fails with
//! "no SQL captured" / "no in-txn statement_timeout captured". Forge wires the GREEN
//! call sites:
//!   - **T16** calls `sql_observe::record_list_sql(sql, effective_limit, has_cursor_predicate)`
//!     from the keyset read path → the SQL-structure tests green.
//!   - **T17** calls `sql_observe::record_statement_timeout_in_txn(ms)` after reading
//!     back `current_setting('statement_timeout')` inside the explicit txn → the
//!     GUC-placement test greens.
//!
//! # verify: []
//!
//! `verify = []` by design (fails against the pre-GREEN tree).

#![cfg(feature = "test-hooks")]

#[path = "common/paging_harness.rs"]
mod paging_harness;

use mnemra_host::plugin::sql_observe;
use paging_harness::*;
use uuid::Uuid;

const TYPE: &str = "echo_fixture";

/// Drive one `echo.list` call and return the keyset SQL the read path captured.
/// Panics with the right-reason-RED message when the read path is not yet routed
/// through the seam (the capture is empty before Forge's T16 wiring).
async fn capture_list_sql(
    setup: &Setup,
    limit: Option<u32>,
    cursor: Option<&str>,
) -> sql_observe::CapturedListSql {
    sql_observe::reset_captured_list_sql();
    let _ = list_call(setup, TYPE, limit, cursor)
        .await
        .expect("echo.list must return Ok");
    sql_observe::take_captured_list_sql().expect(
        "R-0020-a/-c/-f: no keyset SQL captured — the read path does not yet route its \
         emitted SQL through plugin::sql_observe::record_list_sql. GREEN T16 wires the \
         call site from the artifact_list keyset body.",
    )
}

// ===========================================================================
// 369 (white-box) [R-0020-a → T16] — ORDER BY id; id>cursor predicate present/absent.
// ===========================================================================

/// The keyset SELECT emits `ORDER BY id`; the `id > $cursor` predicate is ABSENT on
/// the first page (cursor=none) and PRESENT on a resumed page (cursor=some).
#[tokio::test]
async fn keyset_sql_order_by_id_and_cursor_predicate_369() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    let seeded = synthetic_ids(10);
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &seeded).await;

    // First page: cursor=none → ORDER BY id, NO id>cursor predicate.
    let first = capture_list_sql(&setup, None, None).await;
    let sql_lc = first.sql.to_lowercase();
    assert!(
        sql_lc.contains("order by id"),
        "369/R-0020-a: keyset SQL must contain `ORDER BY id`; got: {}",
        first.sql
    );
    assert!(
        !first.has_cursor_predicate,
        "369/R-0020-a: first page (cursor=none) must omit the `id > $cursor` predicate"
    );

    // Resumed page: cursor=some → the id>cursor keyset predicate is present.
    let resumed = capture_list_sql(&setup, None, Some(&seeded[3])).await;
    assert!(
        resumed.sql.to_lowercase().contains("order by id"),
        "369/R-0020-a: resumed keyset SQL must contain `ORDER BY id`; got: {}",
        resumed.sql
    );
    assert!(
        resumed.has_cursor_predicate,
        "369/R-0020-a: resumed page (cursor=some) must AND in the `id > $cursor` predicate"
    );

    setup.shutdown().await;
}

// ===========================================================================
// 375-A (white-box) [R-0020-c → T16] — no read path emits a LIMIT-less query.
// ===========================================================================

/// Every read path emits a bounded `LIMIT` clause — never a `LIMIT`-less query, even
/// for a huge requested `limit` (it is clamped, fetch-one-extra). Closes the
/// result-set-size half of the prior unbounded-SELECT finding.
#[tokio::test]
async fn keyset_sql_is_never_limit_less_375a() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &synthetic_ids(5)).await;

    let captured = capture_list_sql(&setup, Some(100_000), None).await;
    let sql_lc = captured.sql.to_lowercase();
    assert!(
        sql_lc.contains("limit "),
        "375-A/R-0020-c: the keyset SQL must carry a bounded LIMIT clause (no LIMIT-less \
         query); got: {}",
        captured.sql
    );
    assert!(
        !sql_lc.contains("limit null") && !sql_lc.contains("nullif"),
        "375-A/R-0020-f: the LIMIT must be a concrete bound, never LIMIT NULL / NULLIF-zeroed; \
         got: {}",
        captured.sql
    );
    assert!(
        captured.effective_limit <= 500,
        "375-A/R-0020-c: limit=100_000 must clamp the effective_limit to <= 500, got {}",
        captured.effective_limit
    );

    setup.shutdown().await;
}

// ===========================================================================
// 395-c (white-box) [R-0020-f → T16] — limit=0 → emitted SQL is LIMIT 101.
// ===========================================================================

/// `limit = 0` clamps to the default 100, and the emitted SQL is `LIMIT 101`
/// (effective_limit + 1, fetch-one-extra) — never a `LIMIT`-less or `LIMIT NULLIF`-
/// zeroed query.
#[tokio::test]
async fn keyset_sql_limit_zero_emits_limit_101_395c() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &synthetic_ids(5)).await;

    let captured = capture_list_sql(&setup, Some(0), None).await;
    assert_eq!(
        captured.effective_limit, 100,
        "395-c/R-0020-f: limit=0 must clamp effective_limit to the default 100, got {}",
        captured.effective_limit
    );
    // The fetch-one-extra `+1` asserted numerically (robust to `LIMIT 101` literal OR
    // `LIMIT $n` bind — the repo's parameter idiom).
    assert_eq!(
        captured.limit_value, 101,
        "395-c/R-0020-f: limit=0 must emit LIMIT = effective_limit+1 = 101, got {}",
        captured.limit_value
    );
    // Structural prohibition: never a LIMIT-less / LIMIT NULLIF-zeroed query.
    let sql_lc = captured.sql.to_lowercase();
    assert!(
        sql_lc.contains("limit ") && !sql_lc.contains("limit null") && !sql_lc.contains("nullif"),
        "395-c/R-0020-f: limit=0 must emit a concrete LIMIT, never LIMIT-less / NULLIF-zeroed; \
         got: {}",
        captured.sql
    );

    setup.shutdown().await;
}

// ===========================================================================
// 419 (white-box) [R-0020-c → T16] — cap boundary SQL: LIMIT 500 / 501 / 501.
// ===========================================================================

/// `limit` 499 / 500 / 501 → effective 499 / 500 / 500 and emitted SQL
/// `LIMIT 500 / 501 / 501` (effective_limit + 1, with 501 clamped to 500).
#[tokio::test]
async fn keyset_sql_cap_boundary_limit_clauses_419() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &synthetic_ids(5)).await;

    for (requested, eff, limit_value) in
        [(499u32, 499u32, 500u32), (500, 500, 501), (501, 500, 501)]
    {
        let captured = capture_list_sql(&setup, Some(requested), None).await;
        assert_eq!(
            captured.effective_limit, eff,
            "419/R-0020-c: limit={requested} must clamp to effective_limit={eff}, got {}",
            captured.effective_limit
        );
        // Emitted LIMIT = effective_limit+1, asserted numerically (literal-or-bind robust).
        assert_eq!(
            captured.limit_value, limit_value,
            "419/R-0020-c: limit={requested} must emit LIMIT={limit_value} (effective_limit+1), got {}",
            captured.limit_value
        );
    }

    setup.shutdown().await;
}

// ===========================================================================
// 437 part-i (white-box) [R-0020-d part i → T17] — GUC placement inside the txn.
// ===========================================================================

/// R-0020-d part (i) — GUC placement (fully buildable now, NOT G1-blocked):
/// the keyset query runs inside an explicit transaction, and
/// `current_setting('statement_timeout')` read back **inside that same transaction**
/// is non-zero, equals 3000 ms, and is strictly below the R-0007-b 5 s guest epoch
/// deadline. RED now (no explicit-txn GUC + no read-back); T17 greens it.
#[tokio::test]
async fn guc_placement_statement_timeout_in_txn_437i() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &synthetic_ids(5)).await;

    // Production value (no test override): the knob must be cleared.
    sql_observe::set_test_statement_timeout_ms(None);
    sql_observe::reset_statement_timeout_in_txn();

    let _ = list_call(&setup, TYPE, None, None)
        .await
        .expect("437-i: echo.list must return Ok");

    let ms = sql_observe::take_statement_timeout_in_txn().expect(
        "R-0020-d part i: no in-txn statement_timeout captured — the read path does not yet \
         open an explicit transaction and read back current_setting('statement_timeout') into \
         plugin::sql_observe::record_statement_timeout_in_txn. GREEN T17 wires it (parse the \
         GUC to milliseconds before recording).",
    );

    assert!(
        ms != 0,
        "437-i/R-0020-d: statement_timeout read back in-txn must be non-zero, got {ms}"
    );
    assert_eq!(
        ms, 3000,
        "437-i/R-0020-d: the locked statement_timeout is 3000 ms (maintainer-ratified), got {ms}"
    );
    assert!(
        ms < 5000,
        "437-i/R-0020-d: statement_timeout must be strictly below the R-0007-b 5 s guest epoch \
         deadline, got {ms}"
    );

    setup.shutdown().await;
}
