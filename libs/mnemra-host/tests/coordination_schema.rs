//! Whitebox / operator-SQL-surface schema-contract tests for the coordination
//! wedge's two operational tables — `leases` and `messages` (Task 2 of the
//! coordination-wedge build; spec §Data Model,
//! `docs/specs/2026-07-06-coordination-wedge.md`; R-0076-a..e / R-0077-a).
//!
//! # RED phase (Glitch-first TDD triplet: red → green → re-review)
//!
//! These tests are authored BEFORE the `leases`/`messages` migrations exist.
//! They MUST fail by design right now — the suite is red until Forge appends
//! the migrations to `V0_MIGRATIONS` in the green phase. Nothing here writes
//! implementation; the schema is Forge's to build.
//!
//! ## Red-cleanliness (why these fail for the RIGHT reason)
//!
//! Every catalog assertion is written so it FAILS BY ASSERTION, not by a
//! compile error and not vacuously:
//!
//! - Structural queries hit `information_schema` / `pg_catalog` and JOIN
//!   `pg_class` on `relname = $1` (NOT `$1::regclass`, which THROWS when the
//!   relation is absent). With the table absent the query returns zero rows,
//!   so the assertion fails cleanly — and it compiles because it names no Rust
//!   symbol for a not-yet-existing table.
//! - Absence assertions ("no FK reaches content", "no cross-table FK", "no
//!   content-shaped column") each carry a POSITIVE CONTROL that fails when the
//!   table is absent (the FK set / column set must be non-empty first). Without
//!   the control a `COUNT(...) = 0` passes vacuously against a table that does
//!   not exist yet — the exact false-green scar recorded in
//!   `admin_token.rs:556` — and would stay vacuous in green if a table name
//!   were ever typo'd.
//! - Behavioral inserts (uniqueness enforcement, CHECK enforcement) fail in
//!   red because the INSERT errors on the absent table; in green they assert
//!   the SPECIFIC SQLSTATE (`23505` unique_violation, `23514` check_violation),
//!   so green verifies the real constraint, not merely "some error occurred".
//!
//! ## Mirrors
//!
//! Catalog-introspection style + shared-engine usage mirror
//! `tests/schema_init.rs` (positive-control + `information_schema` pattern),
//! `tests/actors_entity.rs` (Task 1's entity test — raw-SQL writes that bypass
//! the typed API so a schema-level regression fails even though the typed API
//! could not construct the invalid value), and `tests/content_schema.rs` /
//! `tests/tenancy_isolation.rs`.
//!
//! ## Engine / isolation
//!
//! Most tests acquire the binary-wide shared engine
//! (`shared_engine::shared_engine()`) and provision their own fresh, isolated
//! database via `EmbeddedEngine::provision_test_database()` (which runs the
//! same `V0_MIGRATIONS` sequence `schema::init::init()` runs — so the
//! leases/messages migrations run automatically once Forge appends them). Each
//! test uses fresh `Uuid` workspace ids and unique resources, so the
//! double-insert test cannot contaminate siblings even though the engine is
//! shared. The idempotency test (`coordination_migrations_idempotent_*`) is the
//! one exception: it boots a FRESH `EmbeddedEngine::start()` to exercise the
//! public `init()` re-run surface twice, mirroring
//! `schema_init.rs::init_idempotent`. That makes this the first binary to mix
//! the shared-engine fixture with a raw `EmbeddedEngine::start()`; the two
//! operate on independent Postgres instances.
//!
//! ## Realization-coupling note (`session_id`)
//!
//! The `leases.session_id` column assertion is coupled to the attachment-as-
//! lease realization, which Task 2's plan carries as the advisory baseline (a
//! dedicated-linkage realization is a material deviation that HALTs to Puck).
//! If a GREEN failure lands on `session_id`, suspect the realization choice —
//! not a phantom column bug.

#[path = "common/shared_engine.rs"]
mod shared_engine;

use mnemra_host::schema::init::init;
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use sqlx::PgPool;
use uuid::Uuid;

// ===========================================================================
// Catalog-introspection helpers
// ===========================================================================

/// `(column_name, data_type, is_nullable)` for every column of `table` in the
/// `public` schema. Empty when the table does not exist (red).
async fn columns(pool: &PgPool, table: &str) -> Vec<(String, String, String)> {
    sqlx::query_as(
        "SELECT column_name, data_type, is_nullable \
         FROM information_schema.columns \
         WHERE table_schema = 'public' AND table_name = $1",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .expect("information_schema.columns query must execute")
}

/// `pg_get_constraintdef` text for every CHECK constraint on `table`. Empty
/// when the table does not exist (red). JOIN on `relname` (never `$1::regclass`
/// which throws on an absent relation).
async fn check_defs(pool: &PgPool, table: &str) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT pg_get_constraintdef(con.oid) \
         FROM pg_constraint con \
         JOIN pg_class rel ON rel.oid = con.conrelid \
         WHERE con.contype = 'c' \
           AND rel.relnamespace = 'public'::regnamespace \
           AND rel.relname = $1",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .expect("pg_constraint CHECK-def query must execute");
    rows.into_iter().map(|(d,)| d).collect()
}

/// One row per foreign-key COLUMN on `table`:
/// `(referencing_column, referenced_table, num_key_columns)`. A composite FK
/// yields one row per key column, each carrying the constraint's total key
/// count (so a `num_key_columns > 1` flags a composite FK). Empty in red.
async fn fk_rows(pool: &PgPool, table: &str) -> Vec<(String, String, i64)> {
    sqlx::query_as(
        "SELECT att.attname AS referencing_column, \
                ref.relname AS referenced_table, \
                cardinality(con.conkey)::bigint AS num_key_columns \
         FROM pg_constraint con \
         JOIN pg_class rel ON rel.oid = con.conrelid \
         JOIN pg_class ref ON ref.oid = con.confrelid \
         JOIN pg_attribute att \
              ON att.attrelid = con.conrelid AND att.attnum = ANY (con.conkey) \
         WHERE con.contype = 'f' \
           AND rel.relnamespace = 'public'::regnamespace \
           AND rel.relname = $1",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .expect("pg_constraint FK query must execute")
}

/// Lower-cased `CREATE INDEX` definition text for every index on `table`.
/// Empty in red.
async fn index_defs(pool: &PgPool, table: &str) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT indexdef FROM pg_indexes WHERE schemaname = 'public' AND tablename = $1",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .expect("pg_indexes query must execute");
    rows.into_iter().map(|(d,)| d.to_lowercase()).collect()
}

/// Assert `table.name` exists with a data_type in `types` and the given
/// nullability. Panics with the present-column list when the column is absent
/// (the red failure mode).
fn assert_column(
    cols: &[(String, String, String)],
    table: &str,
    name: &str,
    types: &[&str],
    nullable: bool,
) {
    let col = cols
        .iter()
        .find(|c| c.0.as_str() == name)
        .unwrap_or_else(|| {
            let present: Vec<&str> = cols.iter().map(|c| c.0.as_str()).collect();
            panic!(
                "{table}.{name} column must exist (spec §Data Model); columns present: {present:?}"
            );
        });
    let dtype = col.1.as_str();
    let isnull = col.2.as_str();
    assert!(
        types.contains(&dtype),
        "{table}.{name} data_type must be one of {types:?}; got '{dtype}'"
    );
    let want = if nullable { "YES" } else { "NO" };
    assert_eq!(
        isnull, want,
        "{table}.{name} is_nullable must be '{want}'; got '{isnull}'"
    );
}

/// True iff some CHECK def names EVERY member of `members` as a quoted literal
/// — i.e. that check closes the set. Independent of `IN (...)` vs
/// `= ANY (ARRAY[...])` rendering (Postgres normalizes both to `'lit'::text`).
fn any_check_covers(defs: &[String], members: &[&str]) -> bool {
    defs.iter()
        .any(|d| members.iter().all(|m| d.contains(&format!("'{m}'"))))
}

/// True iff the FK set contains a single-column FK on `col` referencing
/// `reftbl`.
fn has_single_col_fk(fks: &[(String, String, i64)], col: &str, reftbl: &str) -> bool {
    fks.iter()
        .any(|f| f.0.as_str() == col && f.1.as_str() == reftbl && f.2 == 1)
}

/// True iff `t` is a P-0017 content-taxonomy table (content, state_config, or a
/// per-type artifact table).
fn is_content_table(t: &str) -> bool {
    t == "content" || t == "state_config" || t.starts_with("artifact")
}

/// Extract the SQLSTATE from a sqlx error, if it carries a database error.
fn db_error_code(err: &sqlx::Error) -> Option<String> {
    err.as_database_error()
        .and_then(|e| e.code())
        .map(|c| c.into_owned())
}

// ===========================================================================
// Row-insertion helpers (bypass the typed API — raw SQL, so a SCHEMA-level
// regression fails these tests even though no typed API exists yet)
// ===========================================================================

/// Insert an `agent` actor under `ws` and return its id (the FK target for
/// leases/messages). `actors` exists as of Task 1.
async fn insert_actor(pool: &PgPool, ws: Uuid, name: &str) -> Uuid {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO actors (workspace_id, actor_type, name) VALUES ($1, 'agent', $2) RETURNING id",
    )
    .bind(ws)
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("actor insert (FK target) must succeed");
    row.0
}

/// The SQL fragment for a 900-second `duration` value, chosen by catalog
/// introspection so the insert is tolerant of the spec's realization latitude
/// (`duration` is `interval` OR `bigint`/`integer` seconds — implementation-
/// tier). This returns one of two FIXED constant fragments — no external value
/// is interpolated, so the parameterization rule is not implicated. Falls back
/// to the numeric form when the column is absent (red), where the caller's
/// INSERT then fails on the absent table (the intended red).
async fn duration_expr(pool: &PgPool) -> &'static str {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT data_type FROM information_schema.columns \
         WHERE table_schema = 'public' AND table_name = 'leases' AND column_name = 'duration'",
    )
    .fetch_optional(pool)
    .await
    .expect("duration data_type introspection must execute");
    match row {
        Some((ref t,)) if t == "interval" => "INTERVAL '900 seconds'",
        _ => "900",
    }
}

/// Insert a lease with all NOT-NULL columns satisfied. `terminal_state` is
/// `None` for a live (non-terminal) lease or `Some(state)` for a terminal one.
/// Returns the raw sqlx result so callers can assert on success OR the specific
/// error code.
async fn insert_lease(
    pool: &PgPool,
    ws: Uuid,
    resource: &str,
    holder: Uuid,
    terminal_state: Option<&str>,
) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error> {
    let dur = duration_expr(pool).await;
    // `dur` is one of two FIXED constant fragments selected by catalog
    // introspection (no external value is interpolated), so asserting the
    // built SQL is safe is legitimate here. `sqlx::query` in this sqlx version
    // requires a `'static`/`SqlSafeStr`; the codebase's pattern for a
    // runtime-built statement is `AssertSqlSafe(owned_string)` (see
    // storage/postgres/engine.rs).
    let sql = format!(
        "INSERT INTO leases \
         (id, workspace_id, resource, holder_actor_id, acquired_at, duration, expires_at, terminal_state) \
         VALUES ($1, $2, $3, $4, now(), {dur}, now() + INTERVAL '1 hour', $5)"
    );
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(Uuid::new_v4())
        .bind(ws)
        .bind(resource)
        .bind(holder)
        .bind(terminal_state)
        .execute(pool)
        .await
}

/// Insert a message with all NOT-NULL columns satisfied. `state` and
/// `disposition` are passed through so a caller can drive the CHECK boundary.
async fn insert_message(
    pool: &PgPool,
    ws: Uuid,
    sender: Uuid,
    addressee: Uuid,
    state: &str,
    disposition: Option<&str>,
) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error> {
    sqlx::query(
        "INSERT INTO messages \
         (id, workspace_id, sender_actor_id, addressee_actor_id, message_type, schema_version, \
          payload, state, sent_at, disposition) \
         VALUES ($1, $2, $3, $4, 'merge-request', 1, '{}'::jsonb, $5, now(), $6)",
    )
    .bind(Uuid::new_v4())
    .bind(ws)
    .bind(sender)
    .bind(addressee)
    .bind(state)
    .bind(disposition)
    .execute(pool)
    .await
}

// ===========================================================================
// AC1 — two separate tables with the §Data Model column shape
// ===========================================================================

/// AC1: `leases` carries exactly the §Data Model columns, with their spec
/// types and nullability. `duration`'s type is left tolerant (interval OR a
/// bigint/integer/numeric seconds count) per the spec's implementation-tier
/// latitude; `session_id` is asserted per the attachment-as-lease baseline (see
/// module doc's realization-coupling note).
#[tokio::test]
async fn leases_table_has_expected_column_shape() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let cols = columns(pool, "leases").await;

    assert_column(&cols, "leases", "id", &["uuid"], false);
    assert_column(&cols, "leases", "workspace_id", &["uuid"], false);
    assert_column(&cols, "leases", "resource", &["text"], false);
    assert_column(&cols, "leases", "holder_actor_id", &["uuid"], false);
    assert_column(&cols, "leases", "project_id", &["uuid"], true);
    assert_column(
        &cols,
        "leases",
        "acquired_at",
        &["timestamp with time zone"],
        false,
    );
    assert_column(
        &cols,
        "leases",
        "duration",
        &["interval", "bigint", "integer", "numeric"],
        false,
    );
    assert_column(
        &cols,
        "leases",
        "expires_at",
        &["timestamp with time zone"],
        false,
    );
    assert_column(&cols, "leases", "terminal_state", &["text"], true);
    assert_column(
        &cols,
        "leases",
        "terminated_at",
        &["timestamp with time zone"],
        true,
    );
    assert_column(&cols, "leases", "superseded_by", &["uuid"], true);
    // Realization-coupled to attachment-as-lease (Task 2 advisory baseline).
    assert_column(&cols, "leases", "session_id", &["uuid"], true);
}

/// AC1: `messages` carries exactly the §Data Model columns, with their spec
/// types and nullability.
#[tokio::test]
async fn messages_table_has_expected_column_shape() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let cols = columns(pool, "messages").await;

    assert_column(&cols, "messages", "id", &["uuid"], false);
    assert_column(&cols, "messages", "workspace_id", &["uuid"], false);
    assert_column(&cols, "messages", "sender_actor_id", &["uuid"], false);
    assert_column(&cols, "messages", "addressee_actor_id", &["uuid"], false);
    assert_column(&cols, "messages", "message_type", &["text"], false);
    assert_column(
        &cols,
        "messages",
        "schema_version",
        &["integer", "bigint", "smallint"],
        false,
    );
    assert_column(&cols, "messages", "payload", &["jsonb"], false);
    assert_column(&cols, "messages", "state", &["text"], false);
    assert_column(
        &cols,
        "messages",
        "sent_at",
        &["timestamp with time zone"],
        false,
    );
    assert_column(
        &cols,
        "messages",
        "delivered_at",
        &["timestamp with time zone"],
        true,
    );
    assert_column(
        &cols,
        "messages",
        "acknowledged_at",
        &["timestamp with time zone"],
        true,
    );
    assert_column(
        &cols,
        "messages",
        "dispositioned_at",
        &["timestamp with time zone"],
        true,
    );
    assert_column(&cols, "messages", "disposition", &["text"], true);
    assert_column(&cols, "messages", "disposition_note", &["text"], true);
}

// ===========================================================================
// AC1 — closed enums realized as TEXT + CHECK (structural, coupling-immune)
// ===========================================================================

/// AC1: `leases.terminal_state` is closed to `{released, taken_over}` by a
/// CHECK constraint (mirrors `actors.actor_type_chk`). Structural assertion on
/// `pg_get_constraintdef` — proves the CHECK EXISTS and names the set,
/// independent of any state-coupling CHECK a behavioral probe could confuse it
/// with.
#[tokio::test]
async fn leases_terminal_state_is_closed_by_check_constraint() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let defs = check_defs(pool, "leases").await;

    assert!(
        any_check_covers(&defs, &["released", "taken_over"]),
        "leases.terminal_state must be TEXT + a CHECK closing the set \
         {{released, taken_over}} (AC1; mirrors actors.actor_type_chk); \
         CHECK defs present: {defs:?}"
    );
}

/// AC1: `messages.state` is closed to `{sent, delivered, acknowledged,
/// dispositioned}` and `messages.disposition` to `{completed, declined,
/// obsolete}`, each by a CHECK constraint. Structural.
#[tokio::test]
async fn messages_state_and_disposition_are_closed_by_check_constraints() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let defs = check_defs(pool, "messages").await;

    assert!(
        any_check_covers(
            &defs,
            &["sent", "delivered", "acknowledged", "dispositioned"]
        ),
        "messages.state must be TEXT + a CHECK closing \
         {{sent, delivered, acknowledged, dispositioned}} (AC1/R-0069-a); \
         CHECK defs present: {defs:?}"
    );
    assert!(
        any_check_covers(&defs, &["completed", "declined", "obsolete"]),
        "messages.disposition must be TEXT + a CHECK closing \
         {{completed, declined, obsolete}} (AC1/R-0069-c); \
         CHECK defs present: {defs:?}"
    );
}

// ===========================================================================
// AC1 — CHECK enforcement (behavioral: an out-of-set value is REJECTED)
// ===========================================================================

/// AC1 enforcement: a raw INSERT with an out-of-set `terminal_state` is
/// rejected AT WRITE (SQLSTATE 23514 check_violation). A positive control (a
/// valid live lease inserts) proves the rejection is the CHECK, not a
/// structural insert failure.
#[tokio::test]
async fn leases_out_of_set_terminal_state_is_rejected_by_check() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let ws = Uuid::new_v4();
    let holder = insert_actor(pool, ws, "terminal-check-holder").await;

    // Positive control: a valid (non-terminal) lease inserts.
    insert_lease(pool, ws, "file:positive-control.txt", holder, None)
        .await
        .expect(
            "positive control: a valid live lease must insert — proves the \
             rejection below is the terminal_state CHECK, not a structural failure",
        );

    // Negative: distinct resource so the partial unique index cannot mask the
    // CHECK; out-of-set terminal_state must be rejected.
    let res = insert_lease(
        pool,
        ws,
        "file:negative-probe.txt",
        holder,
        Some("not_a_terminal_state"),
    )
    .await;
    let err = res.expect_err("an out-of-set terminal_state must be rejected at write");
    assert_eq!(
        db_error_code(&err).as_deref(),
        Some("23514"),
        "an out-of-set terminal_state must be rejected by a CHECK constraint \
         (SQLSTATE 23514 check_violation); got error: {err:?}"
    );
}

/// AC1 enforcement: a raw INSERT with an out-of-set `state` is rejected at
/// write (SQLSTATE 23514). Positive control: a valid `state = 'sent'` message
/// inserts.
#[tokio::test]
async fn messages_out_of_set_state_is_rejected_by_check() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let ws = Uuid::new_v4();
    let sender = insert_actor(pool, ws, "state-check-sender").await;
    let addressee = insert_actor(pool, ws, "state-check-addressee").await;

    // Positive control.
    insert_message(pool, ws, sender, addressee, "sent", None)
        .await
        .expect("positive control: a valid message (state='sent') must insert");

    // Negative.
    let res = insert_message(pool, ws, sender, addressee, "not_a_state", None).await;
    let err = res.expect_err("an out-of-set state must be rejected at write");
    assert_eq!(
        db_error_code(&err).as_deref(),
        Some("23514"),
        "an out-of-set message state must be rejected by a CHECK constraint \
         (SQLSTATE 23514 check_violation); got error: {err:?}"
    );
}

/// AC1 enforcement: a raw INSERT with an out-of-set `disposition` is rejected
/// at write (SQLSTATE 23514). The disposition ENUM's existence is pinned
/// structurally above; this is the enforcement proof. Positive control: a valid
/// message (disposition NULL) inserts.
#[tokio::test]
async fn messages_out_of_set_disposition_is_rejected_by_check() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let ws = Uuid::new_v4();
    let sender = insert_actor(pool, ws, "disposition-check-sender").await;
    let addressee = insert_actor(pool, ws, "disposition-check-addressee").await;

    // Positive control: disposition NULL is valid.
    insert_message(pool, ws, sender, addressee, "sent", None)
        .await
        .expect("positive control: a valid message (disposition NULL) must insert");

    // Negative.
    let res = insert_message(
        pool,
        ws,
        sender,
        addressee,
        "sent",
        Some("not_a_disposition"),
    )
    .await;
    let err = res.expect_err("an out-of-set disposition must be rejected at write");
    assert_eq!(
        db_error_code(&err).as_deref(),
        Some("23514"),
        "an out-of-set disposition must be rejected by a CHECK constraint \
         (SQLSTATE 23514 check_violation); got error: {err:?}"
    );
}

// ===========================================================================
// AC2 — exactly-one-live-lease uniqueness (partial UNIQUE on
// (workspace_id, resource) WHERE terminal_state IS NULL) — the QA-1 mechanism
// ===========================================================================

/// AC2 (structural) + AC3(a): a PARTIAL UNIQUE index on
/// `(workspace_id, resource) WHERE terminal_state IS NULL` exists. This single
/// index is the live-lease-per-resource hot-predicate index (AC3 does not
/// require a separate redundant one).
#[tokio::test]
async fn leases_has_partial_unique_index_on_live_resource() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let defs = index_defs(pool, "leases").await;
    let has = defs.iter().any(|d| {
        d.contains("unique")
            && d.contains("workspace_id")
            && d.contains("resource")
            && d.contains("terminal_state is null")
    });
    assert!(
        has,
        "leases must have a PARTIAL UNIQUE index on (workspace_id, resource) \
         WHERE terminal_state IS NULL (AC2/AC3a; R-0065-b; the QA-1 mechanism). \
         The predicate must be non-terminated (terminal_state IS NULL), NOT \
         expiry-based (a partial index cannot reference now()). Indexes present: \
         {defs:?}"
    );
}

/// AC2 (enforcement, QA-1): inserting a SECOND live lease on the same
/// `(workspace_id, resource)` is REJECTED with the specific unique-constraint
/// violation (SQLSTATE 23505) — not merely "some error". This is the
/// structural half of QA-1, before any tool exists.
#[tokio::test]
async fn leases_second_live_lease_on_same_resource_is_rejected() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let ws = Uuid::new_v4();
    let holder = insert_actor(pool, ws, "uniqueness-holder").await;
    let resource = "repo-lane:mnemra/main-merge";

    insert_lease(pool, ws, resource, holder, None)
        .await
        .expect("the FIRST live lease on a resource must insert");

    let res = insert_lease(pool, ws, resource, holder, None).await;
    let err = res.expect_err(
        "the SECOND live lease on the same (workspace_id, resource) must be \
         REJECTED (QA-1: exactly one live lease per resource)",
    );
    assert_eq!(
        db_error_code(&err).as_deref(),
        Some("23505"),
        "the second live lease must be rejected by the partial UNIQUE index \
         (SQLSTATE 23505 unique_violation) specifically, not just 'some error'; \
         got error: {err:?}"
    );
}

/// AC2 (partialness): a TERMINAL lease does NOT block a new LIVE lease on the
/// same resource. Together with the double-live rejection above, this pins the
/// index as PARTIAL on `WHERE terminal_state IS NULL`: a full UNIQUE index on
/// `(workspace_id, resource)` would wrongly reject re-acquisition after
/// release (this test would fail), and a plain composite UNIQUE that included
/// `terminal_state` would wrongly permit two live rows (the double-live test
/// would fail). Only the correct partial index passes both.
#[tokio::test]
async fn leases_terminal_lease_does_not_block_new_live_lease() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let ws = Uuid::new_v4();
    let holder = insert_actor(pool, ws, "partialness-holder").await;
    let resource = "repo-lane:mnemra/partial-lane";

    insert_lease(pool, ws, resource, holder, Some("released"))
        .await
        .expect("a terminal (released) lease must insert");

    insert_lease(pool, ws, resource, holder, None).await.expect(
        "a NEW live lease on a resource whose only other row is TERMINAL must \
             insert — the uniqueness index must be PARTIAL (WHERE terminal_state \
             IS NULL). A full UNIQUE index on (workspace_id, resource) would \
             wrongly reject this (re-acquisition after release), which is exactly \
             the failure this test guards.",
    );
}

// ===========================================================================
// AC3 — hot-predicate index on messages (undispositioned-per-addressee)
// ===========================================================================

/// AC3(b): `messages` carries an index serving the undispositioned-per-
/// addressee poll predicate (an index over `addressee_actor_id` partial on
/// `WHERE dispositioned_at IS NULL`).
#[tokio::test]
async fn messages_has_undispositioned_per_addressee_index() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let defs = index_defs(pool, "messages").await;
    let has = defs
        .iter()
        .any(|d| d.contains("addressee_actor_id") && d.contains("dispositioned_at is null"));
    assert!(
        has,
        "messages must have an index serving the undispositioned-per-addressee \
         hot predicate — over addressee_actor_id, partial on \
         WHERE dispositioned_at IS NULL (AC3b; R-0076-a poll path). \
         Indexes present: {defs:?}"
    );
}

// ===========================================================================
// AC4 — workspace_id NOT NULL; single-column hard FKs to core entities only
// ===========================================================================

/// AC4: both tables have `workspace_id NOT NULL`; every hard FK is
/// SINGLE-COLUMN and references ONLY `actors` / `projects`; and the expected
/// FKs are present. The expected-FK positive controls make the "references only
/// core" assertion non-vacuous (an empty FK set — red — fails here first).
///
/// The spec (R-0076-b) makes cross-workspace tenant consistency an
/// application-layer concern (WorkspaceCtx) at V0, NOT a composite
/// `(workspace_id, actor_id)` schema FK — so a single-column `actor_id` FK is
/// correct and a composite FK would be a deviation. This asserts single-column.
#[tokio::test]
async fn coordination_tables_workspace_not_null_and_single_col_core_fks() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    // workspace_id NOT NULL on both (positive control: the column must exist).
    let leases_cols = columns(pool, "leases").await;
    let messages_cols = columns(pool, "messages").await;
    assert_column(&leases_cols, "leases", "workspace_id", &["uuid"], false);
    assert_column(&messages_cols, "messages", "workspace_id", &["uuid"], false);

    let leases_fks = fk_rows(pool, "leases").await;
    let messages_fks = fk_rows(pool, "messages").await;

    // Expected FKs present (positive controls — these fail first in red).
    assert!(
        has_single_col_fk(&leases_fks, "holder_actor_id", "actors"),
        "leases.holder_actor_id must be a single-column FK -> actors \
         (AC4/R-0076-c); FKs present: {leases_fks:?}"
    );
    assert!(
        has_single_col_fk(&leases_fks, "project_id", "projects"),
        "leases.project_id must be a single-column FK -> projects (AC4); \
         FKs present: {leases_fks:?}"
    );
    assert!(
        has_single_col_fk(&messages_fks, "sender_actor_id", "actors"),
        "messages.sender_actor_id must be a single-column FK -> actors (AC4); \
         FKs present: {messages_fks:?}"
    );
    assert!(
        has_single_col_fk(&messages_fks, "addressee_actor_id", "actors"),
        "messages.addressee_actor_id must be a single-column FK -> actors (AC4); \
         FKs present: {messages_fks:?}"
    );

    // Every FK on both tables references ONLY actors/projects and is
    // single-column.
    for (tbl, fks) in [("leases", &leases_fks), ("messages", &messages_fks)] {
        for (col, reftbl, ncols) in fks {
            assert!(
                matches!(reftbl.as_str(), "actors" | "projects"),
                "{tbl}.{col} FK references '{reftbl}', but coordination hard FKs \
                 may reference ONLY core entities (actors/projects) at V0 \
                 (AC4/R-0076-b); FKs present: {fks:?}"
            );
            assert_eq!(
                *ncols, 1,
                "{tbl}.{col} FK must be SINGLE-COLUMN (AC4: NO composite \
                 (workspace_id, actor_id) FK — tenant consistency is app-layer); \
                 num_key_columns = {ncols}"
            );
        }
    }
}

// ===========================================================================
// AC5 — no content-table reach (no FK into, and no content-shaped column on,
// either coordination table)
// ===========================================================================

/// AC5: neither `leases` nor `messages` reaches into the P-0017 content
/// taxonomy — no FK references a content table, and neither carries a C1
/// frontmatter/body/content column. Positive controls (FK set non-empty;
/// column set non-empty) make both absence assertions non-vacuous, so they fail
/// in red for the right reason rather than passing against absent tables.
#[tokio::test]
async fn coordination_tables_do_not_reach_content_tables() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    // Part A — no FK reaches a content table.
    let leases_fks = fk_rows(pool, "leases").await;
    let messages_fks = fk_rows(pool, "messages").await;
    assert!(
        !leases_fks.is_empty() && !messages_fks.is_empty(),
        "positive control: both coordination tables must have FKs (else the \
         content-reach absence check is vacuous — in red the tables do not \
         exist); leases FKs: {leases_fks:?}, messages FKs: {messages_fks:?}"
    );
    for (tbl, fks) in [("leases", &leases_fks), ("messages", &messages_fks)] {
        for (col, reftbl, _) in fks {
            assert!(
                !is_content_table(reftbl),
                "{tbl}.{col} FK references content-layer table '{reftbl}' — \
                 coordination tables must not reach into the P-0017 content \
                 taxonomy (AC5: standalone over zero content)"
            );
        }
    }

    // Part B — no content-shaped column (no C1 frontmatter/body layout, no
    // content join column).
    for tbl in ["leases", "messages"] {
        let cols = columns(pool, tbl).await;
        assert!(
            !cols.is_empty(),
            "positive control: {tbl} must have columns (else the content-column \
             absence check is vacuous)"
        );
        for forbidden in [
            "content_id",
            "artifact_id",
            "state_config_id",
            "content_hash",
            "frontmatter",
            "body",
        ] {
            assert!(
                !cols.iter().any(|c| c.0.as_str() == forbidden),
                "{tbl} must not carry the content-reach column '{forbidden}' \
                 (AC5: no C1 frontmatter/body layout, no content read-path)"
            );
        }
    }
}

// ===========================================================================
// AC6 — independently flippable: no cross-table FK beyond the shared actors
// spine
// ===========================================================================

/// AC6: `leases` and `messages` have no FK to each other (their only shared
/// dependency is the `actors` spine). Positive control: both FK sets are
/// non-empty, so the "no FK between them" assertion is not vacuous in red.
#[tokio::test]
async fn leases_and_messages_have_no_cross_table_fk() {
    let engine: &'static EmbeddedEngine = shared_engine::shared_engine().await;
    let db = engine
        .provision_test_database()
        .await
        .expect("provision_test_database should succeed");
    let pool: &PgPool = &db.pool;

    let leases_fks = fk_rows(pool, "leases").await;
    let messages_fks = fk_rows(pool, "messages").await;
    assert!(
        !leases_fks.is_empty() && !messages_fks.is_empty(),
        "positive control: both coordination tables must have FKs (else the \
         cross-table absence check is vacuous); leases FKs: {leases_fks:?}, \
         messages FKs: {messages_fks:?}"
    );

    assert!(
        !leases_fks.iter().any(|f| f.1.as_str() == "messages"),
        "leases must have NO FK -> messages (AC6: independently flippable; no \
         cross-table reference beyond the shared actors spine)"
    );
    assert!(
        !messages_fks.iter().any(|f| f.1.as_str() == "leases"),
        "messages must have NO FK -> leases (AC6: independently flippable)"
    );
}

// ===========================================================================
// AC7 — additive / forward-only / idempotent migrations
// ===========================================================================

/// AC7: re-running `init()` a second time is a no-op that does not error, and
/// both new tables exist afterward (proving the migrations are appended into
/// the idempotent `V0_MIGRATIONS` set, forward-only). Mirrors
/// `schema_init.rs::init_idempotent`, on a FRESH engine (see module doc).
#[tokio::test]
async fn coordination_migrations_idempotent_and_tables_present_after_reinit() {
    let engine = EmbeddedEngine::start()
        .await
        .expect("a fresh embedded Postgres engine must start");

    init(&engine, "vector")
        .await
        .expect("first init must succeed");
    init(&engine, "vector").await.expect(
        "second init must be an idempotent no-op — re-running the migrations \
         must not error (AC7: additive / forward-only / idempotent)",
    );

    for table in ["leases", "messages"] {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = 'public' AND table_name = $1",
        )
        .bind(table)
        .fetch_optional(engine.pool.as_ref())
        .await
        .expect("table introspection query must execute");
        assert!(
            row.is_some(),
            "table '{table}' must exist after init (AC7/AC1: the coordination \
             migration is appended to V0_MIGRATIONS, and an idempotent second \
             init keeps it)"
        );
    }
}
