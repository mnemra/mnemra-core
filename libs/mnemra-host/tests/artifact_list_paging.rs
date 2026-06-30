//! R-0020 artifact-list keyset-pagination — black-box acceptance suite (Task 15 RED).
//!
//! # Phase
//!
//! RED. Glitch authors the failing acceptance tests now; Forge implements to GREEN
//! in T16–T19. These tests exercise the page contract through the **MCP `echo.list`
//! surface** (the same in-process duplex transport + `MnemraMcpServer` path the
//! existing `mcp_server.rs` list tests use) — no implementation code is read.
//!
//! # Right-reason RED
//!
//! Every non-ignored test below **compiles** and **fails on an assertion** (not a
//! compile error): T14 left the host body as placeholder paging (`has-more=false`,
//! `next-cursor=none`) and the result mapping surfaces ids-only text content with
//! `structured_content = None`. So:
//!   - walk / boundary tests fail because the `artifact-page` record is not surfaced
//!     as `structured_content` (see `paging_harness::extract_page`);
//!   - cursor-validation tests fail because no boundary validation / clamp exists yet;
//!   - the telemetry test fails because the per-verb metric event is not emitted yet.
//!
//! Scenario → GREEN-task mapping (from the plan, R-0020 §Tasks):
//!   369 → T16 · 375-A → T16 · 383 → T16(+T22) · 395 → T16 · 401 → **T18** ·
//!   407 → T16 · 413 → T16 · 419 → T16 · 425 → T16(+T22) · 431 → T16 ·
//!   443 → **T19** · 449 → T16/**T19** · 455 → T16 · 375-B/437 → **T17 (G1-BLOCKED, #[ignore])**.
//!
//! # verify: []
//!
//! `verify = []` by design — these fail against the pre-GREEN tree (right-reason RED).
//! There is no `just` recipe to run a red binary to green; GREEN adds it.

#[path = "common/paging_harness.rs"]
mod paging_harness;

use paging_harness::*;
use rmcp::model::ErrorCode;
use std::collections::HashSet;
use tracing_test::traced_test;
use uuid::Uuid;

/// Default host-side page size when `limit` is unspecified / zero (R-0020-c).
const DEFAULT_PAGE: usize = 100;
/// Host-side hard cap on page size (R-0020-c).
const CAP: usize = 500;
const TYPE: &str = "echo_fixture";

// ---------------------------------------------------------------------------
// Shared no-leak assertion (R-0020-e): an error body carries no DB/schema internals.
// ---------------------------------------------------------------------------

fn assert_no_db_internals(error_data: &rmcp::model::ErrorData, scenario: &str) {
    let body = format!("{} {:?}", error_data.message, error_data.data).to_lowercase();
    for marker in [
        "postgres",
        "sqlx",
        "echo_fixture",
        "relation",
        "syntax error",
        "select ",
        "pg_",
        "statement_timeout",
        "current_setting",
    ] {
        assert!(
            !body.contains(marker),
            "{scenario}: R-0020-e no-leak — error body must carry no DB/schema internals, \
             found marker {marker:?} in: {body}"
        );
    }
}

fn expect_mcp_error(
    result: Result<rmcp::model::CallToolResult, rmcp::ServiceError>,
    scenario: &str,
) -> rmcp::model::ErrorData {
    match result.expect_err(&format!("{scenario}: expected a JSON-RPC error, got Ok")) {
        rmcp::ServiceError::McpError(error_data) => error_data,
        other => panic!("{scenario}: expected ServiceError::McpError, got {other:?}"),
    }
}

// ===========================================================================
// 369 [R-0020-a/-b → T16] — keyset walk covers a type end-to-end, exactly once.
// ===========================================================================

/// GIVEN a workspace holding N=250 `echo_fixture` artifacts (N > one page)
/// WHEN a client walks from cursor=none, following next-cursor until has-more=false
/// THEN the concatenated page ids cover all N exactly once, id-ascending across page
///   boundaries; every page has-more ⇔ next-cursor=some; non-final next-cursor =
///   ids.last(); the final page has-more=false ∧ next-cursor=none; the walk terminates.
#[tokio::test]
async fn keyset_walk_covers_type_exactly_once_369() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    let seeded = synthetic_ids(250);
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &seeded).await;

    let (walked, pages) = walk_all(&setup, TYPE, 64).await;

    // Coverage: each of N exactly once, no gaps, no duplicates.
    assert_eq!(
        walked.len(),
        seeded.len(),
        "369/R-0020-a: walk must return each of N={} artifacts exactly once (got {})",
        seeded.len(),
        walked.len()
    );
    assert_eq!(
        walked.iter().collect::<HashSet<_>>().len(),
        seeded.len(),
        "369/R-0020-a: walk must contain no duplicates"
    );
    assert_eq!(
        walked, seeded,
        "369/R-0020-a: walk must return the artifacts in global id-ascending (creation) order"
    );

    // Multiple pages, and the R-0020-b per-page invariant.
    assert!(
        pages.len() > 1,
        "369: N=250 over a 100-default page must span multiple pages (got {})",
        pages.len()
    );
    for (i, page) in pages.iter().enumerate() {
        assert_eq!(
            page.has_more,
            page.next_cursor.is_some(),
            "369/R-0020-b: page {i} must satisfy has-more ⇔ next-cursor=some"
        );
        if page.has_more {
            assert_eq!(
                page.next_cursor.as_deref(),
                page.ids.last().map(String::as_str),
                "369/R-0020-b: non-final page {i} next-cursor must equal ids.last()"
            );
        }
    }
    let last = pages.last().expect("at least one page");
    assert!(
        !last.has_more && last.next_cursor.is_none(),
        "369/R-0020-b: final page must report has-more=false ∧ next-cursor=none"
    );

    setup.shutdown().await;
}

// ===========================================================================
// 375-A [R-0020-c/-f → T16] — DoS size cap: limit=100_000 clamps to <= 500.
// ===========================================================================

/// GIVEN a type holding > 500 artifacts
/// WHEN a caller requests limit=100_000
/// THEN a single bounded page is returned with ids.len() <= 500 (the clamp), even
///   though the requested limit exceeded the cap.
/// (The "rows returned <= cap+1" / "no LIMIT-less query" SQL-structure halves are
///  white-box — see `artifact_list_paging_whitebox.rs`.)
#[tokio::test]
async fn dos_size_cap_clamps_to_500_375a() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    let seeded = synthetic_ids(CAP + 2);
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &seeded).await;

    let result = list_call(&setup, TYPE, Some(100_000), None)
        .await
        .expect("375-A: echo.list must return Ok");
    let page = extract_page(&result).unwrap_or_else(|e| panic!("{e}"));

    assert!(
        page.ids.len() <= CAP,
        "375-A/R-0020-c: limit=100_000 over >500 rows must clamp to <= {CAP} ids (got {})",
        page.ids.len()
    );

    setup.shutdown().await;
}

// ===========================================================================
// 383 [R-0006-d → T16, verified T22] — tenant isolation across the keyset rewrite.
// ===========================================================================

/// GIVEN workspace A and workspace B each holding artifacts of the same type
/// WHEN A paginates the type across all pages (including under a cursor B's rows
///   would sort into)
/// THEN only A's artifacts are ever returned, on any page, for any cursor; a
///   cross-workspace probe returns zero B-owned ids.
#[tokio::test]
async fn tenant_isolation_across_keyset_rewrite_383() {
    let workspace_a = Uuid::new_v4();
    let workspace_b = Uuid::new_v4();
    let setup = setup_in_workspace(workspace_a).await;

    // Interleave A and B ids in id-order so a B id sorts among A's rows.
    let all = synthetic_ids(200);
    let a_ids: Vec<String> = all.iter().step_by(2).cloned().collect();
    let b_ids: Vec<String> = all.iter().skip(1).step_by(2).cloned().collect();
    seed_artifacts(setup.pool(), workspace_a, TYPE, &a_ids).await;
    seed_artifacts(setup.pool(), workspace_b, TYPE, &b_ids).await;

    let b_set: HashSet<&String> = b_ids.iter().collect();

    // Full walk under A: no B id on any page.
    let (walked, _pages) = walk_all(&setup, TYPE, 64).await;
    for id in &walked {
        assert!(
            !b_set.contains(id),
            "383/R-0006-d: tenant isolation violated — A's walk leaked B-owned id {id}"
        );
    }
    assert_eq!(
        walked.iter().collect::<HashSet<_>>(),
        a_ids.iter().collect::<HashSet<_>>(),
        "383: A's walk must return exactly A's own ids"
    );

    // Cross-workspace probe: pass a B id as cursor; zero foreign rows.
    let probe_cursor = &b_ids[b_ids.len() / 2];
    let probe = list_call(&setup, TYPE, None, Some(probe_cursor))
        .await
        .expect("383: probe list must return Ok");
    let probe_page = extract_page(&probe).unwrap_or_else(|e| panic!("{e}"));
    for id in &probe_page.ids {
        assert!(
            !b_set.contains(id),
            "383/R-0006-d: cross-workspace probe under a B cursor leaked B-owned id {id}"
        );
    }

    setup.shutdown().await;
}

// ===========================================================================
// 395-a [R-0020-e → T16] — malformed cursor → R-0010-f param-invalid, no leak.
// ===========================================================================

/// GIVEN the host-fn boundary cursor validation (ahead of query construction)
/// WHEN a caller supplies a malformed cursor (not a 26-char ULID)
/// THEN the host-fn returns the R-0010-f structured "parameter invalid" error
///   (-32602) — not an empty page, not a raw Postgres error — and the error body
///   carries no DB/schema/Postgres internals.
#[tokio::test]
async fn malformed_cursor_returns_param_invalid_395a() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &synthetic_ids(5)).await;

    let result = list_call(&setup, TYPE, None, Some(MALFORMED_CURSOR)).await;
    let err = expect_mcp_error(result, "395-a");
    assert_eq!(
        err.code,
        ErrorCode::INVALID_PARAMS,
        "395-a/R-0020-e: malformed cursor must return -32602 INVALID_PARAMS, got {:?}",
        err.code
    );
    assert_no_db_internals(&err, "395-a");

    setup.shutdown().await;
}

// ===========================================================================
// 395-b [R-0020-e → T16] — out-of-range valid ULID cursor → empty page.
// ===========================================================================

/// GIVEN a well-formed ULID cursor past the end of the type's id range
/// WHEN a caller lists with that cursor
/// THEN an empty page is returned (ids=[], has-more=false, next-cursor=none),
///   derivable from keyset `id > $cursor`.
#[tokio::test]
async fn out_of_range_cursor_returns_empty_page_395b() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &synthetic_ids(20)).await;

    let result = list_call(&setup, TYPE, None, Some(MAX_ULID))
        .await
        .expect("395-b: echo.list must return Ok (not an error) for a valid out-of-range cursor");
    let page = extract_page(&result).unwrap_or_else(|e| panic!("{e}"));

    assert!(
        page.ids.is_empty() && !page.has_more && page.next_cursor.is_none(),
        "395-b/R-0020-e: out-of-range valid cursor must yield an empty page \
         (ids=[], has-more=false, next-cursor=none); got {page:?}"
    );

    setup.shutdown().await;
}

// ===========================================================================
// 395-c [R-0020-c/-f → T16] — limit=0 → bounded default-clamped page (<= 100).
// ===========================================================================

/// GIVEN the host-side clamp `effective_limit = (limit==0 ? 100 : min(limit,500))`
/// WHEN a caller supplies limit=0 against a type with > 100 rows
/// THEN a bounded default-clamped page is returned (ids.len() <= 100) — never an
///   unbounded query. (The "emitted SQL is LIMIT 101" half is white-box.)
#[tokio::test]
async fn limit_zero_returns_bounded_default_page_395c() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &synthetic_ids(150)).await;

    let result = list_call(&setup, TYPE, Some(0), None)
        .await
        .expect("395-c: echo.list must return Ok");
    let page = extract_page(&result).unwrap_or_else(|e| panic!("{e}"));

    assert!(
        page.ids.len() <= DEFAULT_PAGE,
        "395-c/R-0020-f: limit=0 must clamp to the default page (<= {DEFAULT_PAGE} ids), got {}",
        page.ids.len()
    );

    setup.shutdown().await;
}

// ===========================================================================
// 401 [R-0020-g → T18] — paging params excluded from telemetry (metric + log).
// ===========================================================================

/// GIVEN a paginated dispatch carrying a cursor in and a next-cursor out, with the
///   per-verb metric (R-0004-a) + structured-log emission
/// WHEN the dispatch completes and emits telemetry
/// THEN the emitted per-verb metric record contains the R-0004-a floor and does NOT
///   contain the cursor (in) or next-cursor (out) value; the structured log likewise
///   omits them.
///
/// Observation surface: the per-verb metric is emitted as a structured `tracing`
/// event named `verb_metric` (R-0004-f storage-independent emission, observable on
/// stdout/OTel). Captured via `#[traced_test]`. **Pinned token for T18: `verb_metric`.**
/// RED reason: no `verb_metric` event is emitted yet (R-0004-a metric path unwired).
#[traced_test]
#[tokio::test]
async fn paging_params_excluded_from_telemetry_401() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    let seeded = synthetic_ids(50);
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &seeded).await;

    // Deterministic page: cursor = seeded[5]; limit = 20 → page is seeded[6..26],
    // so the would-be next-cursor (out) is seeded[25]. Both must be excluded.
    let in_cursor = seeded[5].clone();
    let out_next_cursor = seeded[25].clone();

    let _ = list_call(&setup, TYPE, Some(20), Some(&in_cursor))
        .await
        .expect("401: echo.list must return Ok");

    // The R-0020-g guarantee is scoped to the per-verb metric record + the dispatch
    // structured-log line — NOT arbitrary spans. Scope the assertion to the
    // `verb_metric` line(s): (1) at least one must exist (RED-discriminating — unwired
    // today); (2) none may carry the cursor (in) or next-cursor (out) value. Scoping to
    // the metric line avoids a green-reason failure if some unrelated debug span happens
    // to carry the cursor (which R-0020-g does not forbid).
    logs_assert(|lines: &[&str]| {
        let metric_lines: Vec<&&str> = lines.iter().filter(|l| l.contains("verb_metric")).collect();
        if metric_lines.is_empty() {
            return Err(
                "401/R-0004-a: a per-verb `verb_metric` telemetry event must be emitted for \
                 the list dispatch (RED until T18 wires R-0004-a metric emission)"
                    .to_owned(),
            );
        }
        for line in &metric_lines {
            if line.contains(&in_cursor) {
                return Err(format!(
                    "401/R-0020-g: the cursor (in) value {in_cursor} MUST NOT appear in the \
                     verb_metric line: {line}"
                ));
            }
            if line.contains(&out_next_cursor) {
                return Err(format!(
                    "401/R-0020-g: the next-cursor (out) value {out_next_cursor} MUST NOT appear \
                     in the verb_metric line: {line}"
                ));
            }
        }
        Ok(())
    });

    setup.shutdown().await;
}

// ===========================================================================
// 407 [R-0020-b → T16] — empty type (N=0) → single empty page.
// ===========================================================================

/// GIVEN a workspace holding zero artifacts of the type
/// WHEN a client lists with cursor=none
/// THEN a single page is returned with ids=[], has-more=false, next-cursor=none;
///   no error, no cap+1 probe-row implied.
#[tokio::test]
async fn empty_type_returns_empty_page_407() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    // No artifacts seeded — the type is empty on this fresh engine.

    let result = list_call(&setup, TYPE, None, None)
        .await
        .expect("407: echo.list on an empty type must return Ok, not an error");
    let page = extract_page(&result).unwrap_or_else(|e| panic!("{e}"));

    assert!(
        page.ids.is_empty() && !page.has_more && page.next_cursor.is_none(),
        "407/R-0020-b: empty type must yield a single empty page \
         (ids=[], has-more=false, next-cursor=none); got {page:?}"
    );

    setup.shutdown().await;
}

// ===========================================================================
// 413 [R-0020-a/-b → T16] — exact-last-page: final page exactly full, end-of-set.
// ===========================================================================

/// GIVEN a workspace holding exactly 2 × default-page (200) artifacts
/// WHEN the client walks to the last page
/// THEN page 1 has 100 ids, has-more=true, next-cursor=some(ids.last()); page 2
///   (final) has 100 ids, has-more=false, next-cursor=none — the final page is
///   exactly full yet reports end-of-set. Catches an `ids.len()==limit` heuristic
///   in place of fetch-one-extra.
#[tokio::test]
async fn exact_last_page_reports_end_of_set_413() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    let seeded = synthetic_ids(2 * DEFAULT_PAGE); // 200
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &seeded).await;

    let (_walked, pages) = walk_all(&setup, TYPE, 8).await;

    assert_eq!(
        pages.len(),
        2,
        "413: exactly 200 rows over a 100-page must span exactly 2 pages, got {}",
        pages.len()
    );
    assert_eq!(
        pages[0].ids.len(),
        DEFAULT_PAGE,
        "413: page 1 must be exactly full (100)"
    );
    assert!(pages[0].has_more, "413: page 1 must report has-more=true");
    assert_eq!(
        pages[0].next_cursor.as_deref(),
        pages[0].ids.last().map(String::as_str),
        "413/R-0020-b: page 1 next-cursor = ids.last()"
    );
    assert_eq!(
        pages[1].ids.len(),
        DEFAULT_PAGE,
        "413: the FINAL page is exactly full (100) — the high-value edge"
    );
    assert!(
        !pages[1].has_more && pages[1].next_cursor.is_none(),
        "413/R-0020-a: an exactly-full FINAL page must report has-more=false ∧ \
         next-cursor=none (fetch-one-extra, NOT len==limit); got {:?}",
        pages[1]
    );

    setup.shutdown().await;
}

// ===========================================================================
// 419 [R-0020-c → T16] — page-size cap boundary: limit 499 / 500 / 501.
// ===========================================================================

/// GIVEN a workspace holding > 501 artifacts and the clamp min(limit, 500)
/// WHEN a client requests limit 499, then 500, then 501
/// THEN the effective page sizes are 499, 500, 500 — every page ids.len() <= 500;
///   501 is clamped to 500. (The SQL LIMIT 500/501/501 half is white-box.)
#[tokio::test]
async fn cap_boundary_499_500_501_419() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    seed_artifacts(
        setup.pool(),
        setup.workspace_id,
        TYPE,
        &synthetic_ids(CAP + 2),
    )
    .await; // 502

    for (requested, expected) in [(499u32, 499usize), (500, 500), (501, 500)] {
        let result = list_call(&setup, TYPE, Some(requested), None)
            .await
            .expect("419: echo.list must return Ok");
        let page = extract_page(&result).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            page.ids.len() <= CAP,
            "419/R-0020-c: every page must be <= {CAP} ids; limit={requested} gave {}",
            page.ids.len()
        );
        assert_eq!(
            page.ids.len(),
            expected,
            "419/R-0020-c: limit={requested} must yield effective page size {expected} (got {})",
            page.ids.len()
        );
    }

    setup.shutdown().await;
}

// ===========================================================================
// 425 [R-0020-e → T16, verified T22] — foreign-but-valid cursor → zero foreign rows.
// ===========================================================================

/// GIVEN workspaces A and B holding the same type, and a well-formed ULID cursor
///   that is the id of one of B's artifacts
/// WHEN A lists passing B's id as cursor
/// THEN the page contains only A-owned ids whose id sorts after the cursor — never a
///   B-owned row; the foreign-but-valid cursor is safe (confined to A's workspace).
#[tokio::test]
async fn foreign_but_valid_cursor_returns_zero_foreign_425() {
    let workspace_a = Uuid::new_v4();
    let workspace_b = Uuid::new_v4();
    let setup = setup_in_workspace(workspace_a).await;

    let all = synthetic_ids(100);
    let a_ids: Vec<String> = all.iter().step_by(2).cloned().collect();
    let b_ids: Vec<String> = all.iter().skip(1).step_by(2).cloned().collect();
    seed_artifacts(setup.pool(), workspace_a, TYPE, &a_ids).await;
    seed_artifacts(setup.pool(), workspace_b, TYPE, &b_ids).await;

    // Use a B id near the middle as the cursor.
    let foreign_cursor = b_ids[b_ids.len() / 2].clone();
    let b_set: HashSet<&String> = b_ids.iter().collect();

    let result = list_call(&setup, TYPE, None, Some(&foreign_cursor))
        .await
        .expect("425: a foreign-but-valid ULID cursor must return Ok, not an error");
    let page = extract_page(&result).unwrap_or_else(|e| panic!("{e}"));

    for id in &page.ids {
        assert!(
            !b_set.contains(id),
            "425/R-0020-e: foreign cursor leaked B-owned id {id}"
        );
        assert!(
            *id > foreign_cursor,
            "425: returned A id {id} must sort after the cursor {foreign_cursor}"
        );
    }

    setup.shutdown().await;
}

// ===========================================================================
// 431 [R-0020-a → T16] — concurrent insert during walk: forward-non-losing keyset.
// ===========================================================================

/// GIVEN a client mid-walk under cursor=C, having emitted all ids <= C
/// WHEN new artifacts (each id > C) are inserted concurrently
/// THEN the new rows appear on a LATER page of the same walk (forward-non-losing:
///   no already-emitted row is lost or duplicated); the walk is not a point-in-time
///   snapshot — including post-walk-begin rows is the defined keyset behavior.
#[tokio::test]
async fn concurrent_insert_during_walk_forward_non_losing_431() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    let full = synthetic_ids(150);
    // Seed DEFAULT_PAGE + 1 rows so that page 1 (fetch-one-extra: LIMIT 101) fetches
    // 101 rows → has_more = true, next_cursor = Some(full[99]) = cursor C.
    // The spec scenario starts mid-walk under an existing cursor C; we need page 1
    // to legitimately carry a cursor before the concurrent insert happens (R-0020-a).
    let initial: Vec<String> = full[..DEFAULT_PAGE + 1].to_vec(); // 101 rows: full[0..=100]
    // Concurrently inserted rows all sort at id > C (C = full[99]), so they appear
    // on later pages — this is the forward-non-losing property under test.
    let inserted_later: Vec<String> = full[DEFAULT_PAGE + 1..].to_vec(); // full[101..149]
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &initial).await;

    // Page 1 (cursor=none): 101 rows in DB → LIMIT 101 returns 101 → has_more = true;
    // emits the first 100 ids; next-cursor C = full[99] (last of the returned page).
    let p1 = list_call(&setup, TYPE, None, None)
        .await
        .expect("431: page 1 must return Ok");
    let page1 = extract_page(&p1).unwrap_or_else(|e| panic!("{e}"));
    let cursor = page1
        .next_cursor
        .clone()
        .expect("431: page 1 of a 101-row seed must carry a next-cursor (fetch-one-extra)");

    // Concurrent insert of rows that all sort at id > C.
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &inserted_later).await;

    // Resume the walk from C: the newly-inserted id>C rows must appear here.
    let (rest, _pages) = {
        let mut all = Vec::new();
        let mut cur = Some(cursor.clone());
        let mut pages = Vec::new();
        for _ in 0..16 {
            let r = list_call(&setup, TYPE, None, cur.as_deref())
                .await
                .expect("431: resumed page must return Ok");
            let page = extract_page(&r).unwrap_or_else(|e| panic!("{e}"));
            all.extend(page.ids.iter().cloned());
            let more = page.has_more;
            cur = page.next_cursor.clone();
            pages.push(page);
            if !more {
                break;
            }
        }
        (all, pages)
    };

    let page1_set: HashSet<&String> = page1.ids.iter().collect();
    let rest_set: HashSet<&String> = rest.iter().collect();
    // Forward-non-losing: no overlap between page 1 and the resumed remainder.
    assert!(
        page1_set.is_disjoint(&rest_set),
        "431/R-0020-a: forward-non-losing — resumed pages must not re-emit page-1 ids"
    );
    // The concurrently-inserted id>C rows appear on a later page.
    for id in &inserted_later {
        assert!(
            rest_set.contains(id),
            "431/R-0020-a: concurrently-inserted id>C row {id} must appear on a later page"
        );
    }

    setup.shutdown().await;
}

// ===========================================================================
// 443 [R-0020-a → T19] — degraded-DB mid-walk: structured error, resumable.
// ===========================================================================

/// GIVEN a client mid-walk having received next-cursor=C on the prior page
/// WHEN it retries the same call with cursor=C (keyset cursors are stateless,
///   idempotent — there is no server-side walk state)
/// THEN the walk resumes from exactly where it left off; re-issuing the identical
///   cursor call yields the identical continuation page.
///
/// Scope note: this authors the RESUMABILITY / statelessness half of 443 as a real
/// red test. The connection-degradation→structured-error half is **verified-emergent**
/// from the existing T13-g fail-closed machinery (`component.rs` panic→catch_unwind→
/// structured error) + keyset statelessness — verified at T19 against existing code,
/// not simulated here (a black-box connection-drop mid-walk would require tearing down
/// the embedded engine, a fragile non-deterministic mechanism). Flagged in the report.
#[tokio::test]
async fn degraded_db_mid_walk_resumes_from_cursor_443() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &synthetic_ids(150)).await;

    // Prior page → cursor C.
    let p1 = list_call(&setup, TYPE, Some(50), None)
        .await
        .expect("443: prior page must return Ok");
    let page1 = extract_page(&p1).unwrap_or_else(|e| panic!("{e}"));
    let c = page1
        .next_cursor
        .clone()
        .expect("443: prior page must carry a next-cursor");

    // Resume twice with the identical cursor — stateless/idempotent.
    let r1 = extract_page(
        &list_call(&setup, TYPE, Some(50), Some(&c))
            .await
            .expect("443: resume 1 Ok"),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let r2 = extract_page(
        &list_call(&setup, TYPE, Some(50), Some(&c))
            .await
            .expect("443: resume 2 Ok"),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(
        r1.ids, r2.ids,
        "443/R-0020-a: keyset cursors are stateless — re-issuing cursor=C must yield the \
         identical continuation page (resumable, no server-side walk state)"
    );
    if let Some(first) = r1.ids.first() {
        assert!(
            *first > c,
            "443: the resumed page must continue strictly after the cursor C"
        );
    }

    setup.shutdown().await;
}

// ===========================================================================
// 449 [R-0016-a / R-0020-a → T16/T19] — concurrent cursor churn under pool pressure.
// ===========================================================================

/// GIVEN multiple clients concurrently walking the same type under the fixed plugin
///   pool (R-0016-a) under contention
/// WHEN the concurrent walks proceed through the guest export → host-fn → Postgres
/// THEN every walk returns a correct, complete, duplicate-free page sequence
///   (concurrency does not corrupt any individual walk — cursors are per-call,
///   stateless per R-0020-a/-b).
#[tokio::test]
async fn concurrent_cursor_churn_yields_correct_walks_449() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    let seeded = synthetic_ids(150);
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &seeded).await;

    // Three concurrent walks over the same type (pool pressure / cursor churn).
    let (w1, w2, w3) = tokio::join!(
        walk_all(&setup, TYPE, 32),
        walk_all(&setup, TYPE, 32),
        walk_all(&setup, TYPE, 32),
    );

    for (label, (ids, _pages)) in [("w1", w1), ("w2", w2), ("w3", w3)] {
        assert_eq!(
            ids, seeded,
            "449/R-0020-a: concurrent walk {label} must return the complete, in-order, \
             dup-free id set under pool pressure"
        );
    }

    setup.shutdown().await;
}

// ===========================================================================
// 455 [R-0020-e → T16] — malformed cursor: invalid base32 alphabet (26-char).
// ===========================================================================

/// GIVEN boundary validation of `cursor` as a well-formed 26-char ULID (Crockford
///   base32 alphabet + range) before query construction
/// WHEN a client supplies a 26-char cursor of the correct LENGTH but containing
///   characters outside the Crockford alphabet (e.g. `I`)
/// THEN the host-fn rejects it with the R-0010-f "parameter invalid" error before
///   any query is constructed (alphabet-and-range, NOT a naive length check); the
///   error body carries no DB/schema/Postgres internals.
#[tokio::test]
async fn malformed_cursor_wrong_base32_alphabet_455() {
    let setup = setup_in_workspace(Uuid::new_v4()).await;
    seed_artifacts(setup.pool(), setup.workspace_id, TYPE, &synthetic_ids(5)).await;

    assert_eq!(
        WRONG_ALPHABET_ULID.len(),
        26,
        "455 precondition: the cursor must be the correct LENGTH (26), invalid only in alphabet"
    );

    let result = list_call(&setup, TYPE, None, Some(WRONG_ALPHABET_ULID)).await;
    let err = expect_mcp_error(result, "455");
    assert_eq!(
        err.code,
        ErrorCode::INVALID_PARAMS,
        "455/R-0020-e: a 26-char wrong-alphabet cursor must be rejected as -32602 \
         (alphabet+range validation, not a length check), got {:?}",
        err.code
    );
    assert_no_db_internals(&err, "455");

    setup.shutdown().await;
}

// ===========================================================================
// 375-B / 437 [R-0020-d part ii → T17] — scan-cost cancellation FIRES.
//   BLOCKED on Spec gap G1 — carried as an explicit #[ignore]'d test.
// ===========================================================================

/// R-0020-d part (ii): a host read-path query that exceeds the `statement_timeout`
/// is canceled and surfaces BOTH the R-0004-a metric `outcome="timeout"` AND a
/// caller-facing `query_scan_timeout` JSON-RPC error (distinct from
/// `plugin_execution_timeout` and `-32602`), with no DB/schema internals in the body.
///
/// # Why this test is `#[ignore]`'d (Spec gap G1 — maintainer decision pending)
///
/// The spec's named slow-query mechanism is "an adversarial sparse **non-indexed
/// `filters` predicate**" — but filter-predicate application is DEFERRED (#1846), and
/// the `workspace_id` btree index defeats every filter-free slow-query, so **no
/// deterministic slow-query construction exists at V0**. Authoring this test to FIRE
/// would require building a slow-query mechanism (un-deferring a filter slice, or
/// sanctioning an alternative), which would silently resolve G1 — a maintainer
/// decision (un-defer / sanction-alternative / accept-deferred-with-tripwire), not
/// the acceptance-test author's. So this test is carried as an explicit, named,
/// ignored test (NOT dropped, NOT stubbed-green, NO slow-query mechanism built); it
/// is enabled once G1 resolves and T17's reduced-`statement_timeout` harness drives it.
///
/// The buildable-now GUC-placement half (R-0020-d part i) is a REAL red test in
/// `artifact_list_paging_whitebox.rs` (`guc_placement_statement_timeout_in_txn_*`).
#[tokio::test]
#[ignore = "R-0020-d part-ii cancellation-fires: blocked on spec gap G1 — slow-query mechanism deferred (#1846); pending maintainer decision (un-defer filter slice / sanction alt mechanism / accept deferred-with-tripwire). No slow-query mechanism is built here; enabled at T17 once G1 resolves."]
async fn scan_cost_cancellation_fires_437_g1_blocked() {
    // Intentionally not implemented: firing this test requires a deterministic
    // slow-query mechanism that does not exist at V0 (G1). Building one would
    // resolve G1 unilaterally. When G1 resolves, T17 enables this against the
    // reduced-`statement_timeout` knob (paging_harness / plugin::sql_observe) and a
    // seeded-rows slow path, asserting both `outcome="timeout"` and the
    // `query_scan_timeout` caller error with a no-leak body.
}
