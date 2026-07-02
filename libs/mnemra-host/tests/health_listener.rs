//! Acceptance tests for the `/health` loopback HTTP listener (T4 RED,
//! R-0022-b / R-0004-g). Glitch, task #1991.
//!
//! # Scope
//!
//! These tests exercise `mnemra_host::health` as a black box: real TCP
//! connections against a listener this crate binds via its public API, real
//! HTTP/1.1 request lines written by hand (no framework — the listener under
//! test doesn't have one; see `health.rs`'s module doc), real response
//! parsing. No reading of `health.rs`'s implementation.
//!
//! # RED phase (T4) — right-reason failures
//!
//! `HealthListener::bind`, `::serve`, `::local_addr`, `ReadinessHandle::new`/
//! `::mark_ready`, `ReadinessSignal::is_ready`, `PoolCell::empty`/`::set`,
//! and `resolve_port` are all `todo!()` stubs (see `health.rs`). Every test
//! below panics at the first stub call it reaches on the path to its
//! assertions — that IS the red (`skills/tdd.md` `<cycle>`: "the panic
//! propagates as a test failure — that IS the red"). `verify = []` for this
//! dispatch: RED fails by design until the GREEN (T5) implementer lands the
//! bodies.
//!
//! # No live Postgres required (NONPG_TEST_FLAGS)
//!
//! The body-content test (AC1) uses `sqlx::postgres::PgPoolOptions::connect_lazy`
//! against an address nothing listens on, so `health_snapshot`'s first query
//! fails connection-refused-style and deterministically maps to
//! `overall: "down"` (`schema/init.rs` A-15's `EngineUnavailable` mapping) —
//! no embedded-Postgres engine needed anywhere in this file. Wired into
//! `NONPG_TEST_FLAGS` in the `justfile`, not `PG_TEST_FLAGS`.
//!
//! # AC ↔ test map
//!
//! - AC1 (structured body from `health_snapshot`): `get_health_returns_structured_detail_body_sourced_from_health_snapshot`
//! - AC2 (loopback-only bind, tripwire): `non_loopback_bind_attempt_fires_tripwire`, `binding_the_same_port_twice_surfaces_io_error`
//! - AC3 (only `GET /health` served): `only_get_health_is_served_other_requests_rejected`
//! - AC4 (readiness-state source): `readiness_signal_reports_not_ready_before_mark_ready`, `readiness_signal_reports_ready_after_mark_ready`, `get_health_before_ready_with_no_pool_answers_not_ready_without_crashing`
//! - Port strategy (dispatch instruction): `resolve_port_defaults_when_env_var_absent`, `resolve_port_parses_valid_env_value`, `resolve_port_falls_back_on_unparseable_env_value`
//! - Flood-hardening regression (M2 security-review fix, `d59e1f4`, task #1998): `oversized_request_head_is_rejected_431_and_listener_stays_alive`, `header_line_flood_is_rejected_431_and_listener_stays_alive`

use mnemra_host::health::{
    DEFAULT_HEALTH_PORT, HealthBindError, HealthListener, PoolCell, ReadinessHandle,
    ReadinessSignal, resolve_port,
};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Test helpers (single call site family — inline per rule-of-three; extract
// to tests/common/ if a second health-surface test file needs them).
// ---------------------------------------------------------------------------

/// A `PgPool` that never eagerly connects — `connect_lazy` only parses the
/// URL; the first *query* against it fails connection-refused-style (no
/// embedded Postgres needed). Port 1 is a privileged port nothing in a test
/// environment binds.
///
/// Two sqlx-core 0.9.0 defaults, both verified against the vendored source,
/// have to be overridden here for that premise to hold in practice:
///
/// 1. `max_lifetime(None).idle_timeout(None)` — `PoolInner::new_arc` calls
///    `spawn_maintenance_tasks` unconditionally at *construction* time
///    (before any query runs). That function only skips
///    `crate::rt::spawn` (which requires a Tokio context) when
///    `max_lifetime` and `idle_timeout` are both `None` AND
///    `min_connections == 0` (`src/pool/inner.rs::spawn_maintenance_tasks`,
///    `(None, None)` arm). The crate defaults are `idle_timeout: Some(10
///    min)` / `max_lifetime: Some(30 min)`, so an unmodified
///    `PgPoolOptions` always spawns that maintenance task at
///    `connect_lazy` time — on a plain test thread with no Tokio runtime,
///    that spawn panics with "this functionality requires a Tokio context"
///    before this helper even returns, let alone before the first query.
/// 2. `acquire_timeout(Duration::from_millis(500))` — `PoolInner::connect`
///    (`src/pool/inner.rs::connect`) treats a connection-refused I/O error
///    as "the system starting up" and retries with exponential backoff
///    until the pool's `acquire_timeout` deadline, which defaults to 30
///    seconds — it does not fail fast on the first refusal the way a raw
///    `TcpStream::connect` does. Left at the default, the first real query
///    in `health_snapshot` would retry against port 1 for up to 30s,
///    blowing well past this suite's `http_roundtrip` client-side 2s read
///    timeout (a spurious `WouldBlock`, not a listener defect — the
///    listener is still working, just slower than the test client waits).
///    500ms is ample margin over the near-instant local refusal while
///    staying far under the 2s client timeout; `health_snapshot` maps
///    *any* first-query failure (refused or timed-out) to `pg_ok: false`
///    (`schema/init.rs`), so which failure mode fires doesn't change the
///    asserted `overall: "down"` outcome.
fn unreachable_lazy_pool() -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_lifetime(None)
        .idle_timeout(None)
        .acquire_timeout(Duration::from_millis(500))
        .connect_lazy("postgres://health_red_test:health_red_test@127.0.0.1:1/health_red_test")
        .expect("connect_lazy must not eagerly connect (defers to first query)")
}

/// Read a full HTTP response from `stream` until the peer closes the
/// connection, and parse `(status_code, body)`. Extracted on second use
/// (`http_roundtrip` below, well-formed requests; `send_raw_and_read_response`,
/// malformed/oversized requests the flood-hardening tests need) — the
/// framing-and-parse half of the round-trip is identical either way, only
/// what gets written to the socket differs.
fn read_http_response(stream: &mut TcpStream) -> (u16, String) {
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .expect("read_to_end failed (server did not close the connection as expected)");
    let text = String::from_utf8_lossy(&raw).into_owned();

    let mut parts = text.splitn(2, "\r\n\r\n");
    let head = parts.next().unwrap_or_default();
    let body = parts.next().unwrap_or_default().to_string();

    let status_code: u16 = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| {
            panic!("could not parse an HTTP status line from response head: {head:?}")
        });

    (status_code, body)
}

/// Send a raw HTTP/1.1 request line and read the full response until the
/// peer closes the connection (the listener is expected to answer with
/// `Connection: close` for this single-shot, framework-less surface).
/// Returns `(status_code, body_string)`.
fn http_roundtrip(addr: SocketAddr, method: &str, path: &str) -> (u16, String) {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .unwrap_or_else(|e| panic!("failed to connect to {addr}: {e}"));
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set_read_timeout failed");
    let request =
        format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .expect("write_all failed");
    stream.flush().expect("flush failed");

    read_http_response(&mut stream)
}

/// Send raw bytes verbatim — no request-line/header framing added — and
/// parse the response the same way `http_roundtrip` does. Used to construct
/// the malformed / oversized requests the flood-hardening tests need; the
/// happy-path helper above always sends a well-formed request.
fn send_raw_and_read_response(addr: SocketAddr, raw_request: &[u8]) -> (u16, String) {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .unwrap_or_else(|e| panic!("failed to connect to {addr}: {e}"));
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set_read_timeout failed");
    stream.write_all(raw_request).expect("write_all failed");
    stream.flush().expect("flush failed");

    read_http_response(&mut stream)
}

/// Bind a listener on an OS-assigned loopback port and spawn its accept loop
/// on a dedicated thread (mirrors the shape `run()`, T5, is expected to use
/// — `serve()` blocks the calling thread by design). Returns the actually-
/// bound address.
fn spawn_listener(readiness: ReadinessSignal, pool: PoolCell) -> SocketAddr {
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
    let listener = HealthListener::bind(bind_addr, readiness, pool)
        .expect("loopback bind on port 0 must succeed");
    let local_addr = listener.local_addr();
    std::thread::spawn(move || {
        let _ = listener.serve();
    });
    // Covers only the OS thread-scheduling gap between spawn() and the
    // accept loop's first accept — not a retry/backoff policy. GREEN's
    // serve() is expected to be listening synchronously by the time
    // bind() returns (the TCP socket is already bound then); this sleep
    // is generous slack for the spawned thread to actually start running.
    std::thread::sleep(Duration::from_millis(50));
    local_addr
}

// ---------------------------------------------------------------------------
// AC1 — GET /health returns the R-0004-g structured detail body, sourced
// from schema::init::health_snapshot
// ---------------------------------------------------------------------------

#[test]
fn get_health_returns_structured_detail_body_sourced_from_health_snapshot() {
    // Given a listener bound loopback, marked ready, backed by a pool that
    // is reachable-but-down (deterministic overall:"down" — A-15 — no live
    // Postgres needed).
    let (handle, signal) = ReadinessHandle::new();
    handle.mark_ready();

    let pool_cell = PoolCell::empty();
    pool_cell.set(unreachable_lazy_pool());

    let addr = spawn_listener(signal, pool_cell);

    // When a caller issues GET /health.
    let (status, body) = http_roundtrip(addr, "GET", "/health");

    // Then it returns 200 with the R-0004-g structured detail body,
    // computed by the existing health_snapshot (not a listener-invented
    // shape).
    assert_eq!(status, 200, "GET /health must return 200; body: {body}");
    let json: Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("GET /health body must be valid JSON: {e}; body: {body}"));
    assert_eq!(
        json.get("postgres"),
        Some(&Value::Bool(false)),
        "unreachable pool must report postgres:false; body: {json}"
    );
    assert_eq!(
        json.get("pgvector"),
        Some(&Value::Bool(false)),
        "unreachable pool must report pgvector:false; body: {json}"
    );
    assert_eq!(
        json.get("workspace_default"),
        Some(&Value::Bool(false)),
        "unreachable pool must report workspace_default:false; body: {json}"
    );
    assert_eq!(
        json.get("overall"),
        Some(&Value::String("down".to_string())),
        "unreachable pool must report overall:\"down\" (schema/init.rs A-15 EngineUnavailable mapping); body: {json}"
    );
}

// ---------------------------------------------------------------------------
// AC2 — loopback-only bind; non-loopback bind attempts fire the R-0004-g
// tripwire
// ---------------------------------------------------------------------------

#[test]
fn non_loopback_bind_attempt_fires_tripwire() {
    // Each address below is unambiguously non-loopback under any reading of
    // R-0004-g's "bind to 127.0.0.1 only" — none is `::1` or another
    // 127.0.0.0/8 address (that boundary is a genuine spec ambiguity this
    // test does NOT pin; see the T4 completion report).
    let non_loopback_addrs = [
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0), // wildcard v4
        SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),     // wildcard v6 (::)
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7)), 0), // TEST-NET-3, routable-looking
    ];

    for addr in non_loopback_addrs {
        let (_handle, signal) = ReadinessHandle::new();
        let pool = PoolCell::empty();

        let result = HealthListener::bind(addr, signal, pool);

        match result {
            Err(HealthBindError::NonLoopbackBind { addr: rejected }) => {
                assert_eq!(
                    rejected, addr,
                    "tripwire error must name the exact address it rejected"
                );
            }
            other => panic!(
                "expected HealthBindError::NonLoopbackBind for non-loopback {addr}; got {other:?}"
            ),
        }
    }
}

#[test]
fn binding_the_same_port_twice_surfaces_io_error() {
    // Given a listener already bound and serving on an OS-assigned loopback
    // port.
    let (handle, signal) = ReadinessHandle::new();
    let pool = PoolCell::empty();
    let addr = spawn_listener(signal, pool);
    // Keep the readiness handle alive for the test's duration — dropping it
    // is not part of what this test exercises.
    let _handle = handle;

    // When a second listener attempts to bind the exact same address.
    let (_second_handle, second_signal) = ReadinessHandle::new();
    let second_pool = PoolCell::empty();
    let result = HealthListener::bind(addr, second_signal, second_pool);

    // Then the bind fails with a real I/O error (address in use) — not the
    // loopback tripwire (the address IS loopback) and not a silent success.
    match result {
        Err(HealthBindError::Io(_)) => {}
        other => panic!(
            "expected HealthBindError::Io for a same-address double-bind on {addr}; got {other:?}"
        ),
    }
}

// ---------------------------------------------------------------------------
// AC3 — only GET /health is served
// ---------------------------------------------------------------------------

#[test]
fn only_get_health_is_served_other_requests_rejected() {
    let (handle, signal) = ReadinessHandle::new();
    handle.mark_ready();
    let pool_cell = PoolCell::empty();
    pool_cell.set(unreachable_lazy_pool());

    let addr = spawn_listener(signal, pool_cell);

    // Wrong method, correct path -> 405 (R-0004-g: GET only).
    let (status, body) = http_roundtrip(addr, "POST", "/health");
    assert_eq!(status, 405, "POST /health must be rejected; body: {body}");

    let (status, body) = http_roundtrip(addr, "HEAD", "/health");
    assert_eq!(status, 405, "HEAD /health must be rejected; body: {body}");

    // Correct method, wrong path -> 404 (R-0004-g: /health only, no other
    // routes).
    let (status, body) = http_roundtrip(addr, "GET", "/healthz");
    assert_eq!(status, 404, "GET /healthz must be rejected; body: {body}");

    let (status, body) = http_roundtrip(addr, "GET", "/");
    assert_eq!(status, 404, "GET / must be rejected; body: {body}");
}

// ---------------------------------------------------------------------------
// AC4 — readiness-state source: not-ready before signal, ready after
// ---------------------------------------------------------------------------

#[test]
fn readiness_signal_reports_not_ready_before_mark_ready() {
    // Given a freshly constructed readiness pair — valid with NO PgPool
    // anywhere in scope (T5 5a boundary: readiness exists before storage).
    let (_handle, signal) = ReadinessHandle::new();

    // Then it reports not-ready.
    assert!(
        !signal.is_ready(),
        "a freshly constructed ReadinessSignal must report not-ready before mark_ready() is called"
    );
}

#[test]
fn readiness_signal_reports_ready_after_mark_ready() {
    let (handle, signal) = ReadinessHandle::new();

    handle.mark_ready();

    assert!(
        signal.is_ready(),
        "ReadinessSignal must report ready once ReadinessHandle::mark_ready() has been called"
    );
}

/// Design constraint (plan Reading note 6): the listener must be able to
/// answer not-ready with no pool. This is the flagged interpretation from
/// `health.rs`'s module doc — 503 + `{"ready": false}` — distinct from the
/// R-0004-g detail body (reserved for once a pool exists). Puck/reviewer
/// sign-off needed on this status/shape before GREEN commits to it.
#[test]
fn get_health_before_ready_with_no_pool_answers_not_ready_without_crashing() {
    let (_handle, signal) = ReadinessHandle::new(); // never marked ready
    let pool = PoolCell::empty(); // never set

    let addr = spawn_listener(signal, pool);

    let (status, body) = http_roundtrip(addr, "GET", "/health");

    assert_eq!(
        status, 503,
        "GET /health before mark_ready() with no pool must answer 503, not crash, hang, or serve the R-0004-g detail body; body: {body}"
    );
    let json: Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("not-ready body must be valid JSON: {e}; body: {body}"));
    assert_eq!(
        json.get("ready"),
        Some(&Value::Bool(false)),
        "not-ready body must report ready:false; body: {json}"
    );
}

// ---------------------------------------------------------------------------
// Port resolution — pure function, no env::set_var (workspace canon:
// skills/rust.md <P-TDDPairs> TF2 — env::set_var flakes under parallel test
// execution). resolve_port takes the env-var value as a plain argument
// specifically so it is testable without touching process env at all.
// ---------------------------------------------------------------------------

#[test]
fn resolve_port_defaults_when_env_var_absent() {
    assert_eq!(
        resolve_port(None),
        DEFAULT_HEALTH_PORT,
        "resolve_port(None) must default to DEFAULT_HEALTH_PORT (MNEMRA_HEALTH_PORT default, 8877)"
    );
}

#[test]
fn resolve_port_parses_valid_env_value() {
    assert_eq!(
        resolve_port(Some("9999")),
        9999,
        "resolve_port must parse a valid numeric env value"
    );
}

#[test]
fn resolve_port_falls_back_on_unparseable_env_value() {
    assert_eq!(
        resolve_port(Some("not-a-port")),
        DEFAULT_HEALTH_PORT,
        "resolve_port must fall back to DEFAULT_HEALTH_PORT on an unparseable MNEMRA_HEALTH_PORT value, not panic"
    );
}

// ---------------------------------------------------------------------------
// Flood-hardening regression (M2 security-review fix, dispatch 1250,
// commit d59e1f4): the /health reader shares one MAX_REQUEST_BYTES=8192
// budget across the request line + every header line (`Read::take` wrapped
// before the `BufReader`), and independently caps header-line COUNT at
// MAX_HEADER_LINES=100. Either breach answers 431 + a JSON `error` body and
// closes the connection. The liveness assertion in both tests below — a
// well-formed request on a NEW connection still gets a normal answer — IS
// the security property under test: a fix that stopped the flood but killed
// or hung the listener would still be a DoS.
// ---------------------------------------------------------------------------

#[test]
fn oversized_request_head_is_rejected_431_and_listener_stays_alive() {
    // Given a listener bound loopback, marked ready (so the follow-up
    // well-formed request below takes the normal 200 path, not a 503
    // not-ready confound).
    let (handle, signal) = ReadinessHandle::new();
    handle.mark_ready();
    let pool_cell = PoolCell::empty();
    pool_cell.set(unreachable_lazy_pool());
    let addr = spawn_listener(signal, pool_cell);

    // When a connection sends a request head (request line + headers) that
    // exceeds the shared 8 KiB budget with no completing CRLF inside that
    // budget — one oversized header value is the simplest way to blow the
    // budget mid-line, and does not depend on how many separate lines
    // preceded it.
    let oversized_value = "A".repeat(9000);
    let raw_request =
        format!("GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Pad: {oversized_value}");
    let (status, body) = send_raw_and_read_response(addr, raw_request.as_bytes());

    // Then the listener answers 431 with the documented error body and
    // closes the connection (read_to_end returning at all is evidence of
    // close — see read_http_response).
    assert_eq!(
        status, 431,
        "a request head exceeding the 8 KiB shared budget must be rejected 431; body: {body}"
    );
    let json: Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("431 response body must be valid JSON: {e}; body: {body}"));
    assert_eq!(
        json.get("error"),
        Some(&Value::String("request too large".to_string())),
        "431 response body must report the documented error message; body: {json}"
    );

    // And the listener stays alive: a well-formed request on a NEW
    // connection still gets a normal answer.
    let (status, body) = http_roundtrip(addr, "GET", "/health");
    assert_eq!(
        status, 200,
        "the listener must still serve a well-formed request on a new connection after \
         rejecting an oversized one; body: {body}"
    );
}

#[test]
fn header_line_flood_is_rejected_431_and_listener_stays_alive() {
    // Given a listener bound loopback, marked ready.
    let (handle, signal) = ReadinessHandle::new();
    handle.mark_ready();
    let pool_cell = PoolCell::empty();
    pool_cell.set(unreachable_lazy_pool());
    let addr = spawn_listener(signal, pool_cell);

    // When a connection sends more than MAX_HEADER_LINES (100) short,
    // well-formed header lines — each terminated with its own CRLF, so the
    // byte budget (8 KiB) is deliberately NOT what trips first here: 150
    // lines of "X-<n>: 1\r\n" is on the order of 1.3 KB total, comfortably
    // inside the 8 KiB byte budget. Only the independent header-line COUNT
    // cap can explain a rejection of this request.
    let mut raw_request = String::from("GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\n");
    for i in 0..150 {
        raw_request.push_str(&format!("X-{i}: 1\r\n"));
    }
    let (status, body) = send_raw_and_read_response(addr, raw_request.as_bytes());

    // Then the listener answers 431 (the same breach response the byte cap
    // uses) and closes the connection — proving the header-COUNT cap is a
    // real, independent second bound, not merely documented intent.
    assert_eq!(
        status, 431,
        "more than MAX_HEADER_LINES (100) header lines must be rejected 431, even while \
         staying inside the 8 KiB byte budget; body: {body}"
    );
    let json: Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("431 response body must be valid JSON: {e}; body: {body}"));
    assert_eq!(
        json.get("error"),
        Some(&Value::String("request too large".to_string())),
        "431 response body must report the documented error message; body: {json}"
    );

    // And the listener stays alive for the next connection.
    let (status, body) = http_roundtrip(addr, "GET", "/health");
    assert_eq!(
        status, 200,
        "the listener must still serve a well-formed request on a new connection after \
         rejecting a header-count flood; body: {body}"
    );
}
