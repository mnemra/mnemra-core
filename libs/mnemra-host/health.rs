//! `/health` loopback HTTP listener (R-0004-g / R-0022-b).
//!
//! # Module decision
//!
//! This module owns exactly one decision: the HTTP wrapper around
//! `schema::init::health_snapshot` is a hand-rolled, single-route HTTP/1.1
//! listener over `std::net` — not a new HTTP-server dependency. R-0004-g
//! calls for "a separate, **minimal** HTTP server"; the workspace has no
//! axum/hyper/tiny_http dependency anywhere, and this crate's own `tokio`
//! dependency declares only `rt` + `sync` (no `net`) — see `skills/rust.md`
//! `<dependencies>`: manual stdlib is fine for a fixed, single-route,
//! exhaustively-enumerable surface like this one. This module owns the HTTP
//! wrapper only: binding, request routing, readiness gating, response
//! serialization. It does NOT own the health-body computation — that is
//! `schema::init::health_snapshot` (already built; see that function's
//! "Task 25 hook-in seam" doc comment in `schema/init.rs`).
//!
//! # GREEN phase (T5, #1935 — Forge)
//!
//! Implements the bodies RED (T4, #1991) left as `todo!()` stubs, exercised
//! by `tests/health_listener.rs` — all 10 tests pass with no test-file
//! edits. Representation choices (struct fields, the accept-loop strategy,
//! the single-threaded tokio runtime driving `health_snapshot`'s `.await`)
//! are this module's own; the **public** signatures RED fixed are
//! unchanged.
//!
//! # Readiness / pool sequencing (composing spec's 5a → 5c boundaries)
//!
//! [`ReadinessHandle`] / [`ReadinessSignal`] and [`PoolCell`] are all
//! constructible, cloneable (where relevant), and usable with **no**
//! `PgPool` anywhere in scope — `run()` builds these and binds the listener
//! at the 5a boundary, before config load and before any storage exists.
//! `PoolCell::set` and `ReadinessHandle::mark_ready` are called later, at the
//! 5c boundary, once `start_embedded()` + schema init complete. Before that,
//! `GET /health` answers **not-ready** rather than crashing or hanging on a
//! missing pool.
//!
//! ## Not-ready response shape — flagged interpretation
//!
//! R-0004-g's structured body (`{postgres, pgvector, workspace_default,
//! overall}`) presupposes a `health_snapshot(pool)` call, which is undefined
//! with no pool at all. This module's tests pin a **503** status with a
//! minimal `{"ready": false}` body for the not-ready case — distinct from
//! the R-0004-g detail body, which is reserved for once a pool exists. This
//! was a T4 (Glitch) interpretation, not literal spec text; T5 (GREEN)
//! implements it as pinned by the test suite, per Puck's dispatch sign-off.
//!
//! ## Loopback-bind check — flagged interpretation, resolved
//!
//! T4's skeleton flagged a genuine spec ambiguity: whether `bind()`'s
//! loopback allow-list should be the exact address `127.0.0.1` only, or
//! the broader `IpAddr::is_loopback()` (all of `127.0.0.0/8` plus IPv6
//! `::1`). The test suite deliberately does not pin this boundary. Per
//! Puck's dispatch instruction for this task, `bind()` uses
//! `addr.ip().is_loopback()` — production callers (`run()`) pass exactly
//! `Ipv4Addr::LOCALHOST` regardless, so the two choices are observably
//! identical in the production path; the broader check only changes
//! behavior for a caller that deliberately passes another loopback address.

use crate::schema::init::health_snapshot;
use serde_json::json;
use sqlx::PgPool;
use std::fmt;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Default `/health` listener port (R-0004-g).
pub const DEFAULT_HEALTH_PORT: u16 = 8877;

/// Env var overriding the default port (R-0004-g).
pub const HEALTH_PORT_ENV_VAR: &str = "MNEMRA_HEALTH_PORT";

/// Maximum total bytes read from a single connection's request line + header
/// block combined (security review, dispatch 1249/1250 — Medium finding).
/// The hand-rolled reader previously had no cap: a client that streamed
/// bytes with no `\n` grew `request_line` without limit, memory-exhausting
/// the whole host process and — because `serve()` handles one connection
/// fully before the next `accept()` — head-of-line-blocking every
/// subsequent `/health` check behind it. 8 KiB is generous headroom over any
/// legitimate probe (a bare `GET /health HTTP/1.1` request plus a handful of
/// headers is well under 1 KiB) while bounding per-connection memory even
/// under a sustained flood.
const MAX_REQUEST_BYTES: u64 = 8 * 1024;

/// Maximum header lines drained per request. Defense-in-depth alongside
/// `MAX_REQUEST_BYTES` — bounds the header-drain loop's iteration count
/// independently of the byte cap (e.g. a flood of short/empty lines that
/// stays within the byte budget).
const MAX_HEADER_LINES: usize = 100;

/// Total wall-clock budget for one connection's request-line + header read
/// phase (security re-review addendum, dispatch 1249 — "slow-drip residual",
/// Low finding conf 85). `MAX_REQUEST_BYTES` bounds *total bytes*, and the
/// per-read socket timeout bounds *idle time between bytes*; neither bounds
/// a peer that keeps sending a trickle of bytes just under the per-read
/// timeout forever — that peer can hold the single-threaded accept loop for
/// up to `MAX_REQUEST_BYTES × per-read-timeout` (with the old fixed 5s
/// per-read timeout: ~11 hours). `CONNECTION_DEADLINE` is a hard wall-clock
/// ceiling on the whole read phase, independent of how fast or slow the
/// peer drips: see [`CappedLineReader`] for why it must be re-armed before
/// every underlying socket read, not just once per request/header line. 10s
/// is generous headroom over any legitimate probe (sub-millisecond on
/// loopback) while keeping a worst-case stuck connection short.
const CONNECTION_DEADLINE: Duration = Duration::from_secs(10);

/// Per-syscall read chunk size for [`CappedLineReader`]. Well under
/// `MAX_REQUEST_BYTES` so a well-formed request (well under 1 KiB) reads in
/// a single syscall; large enough that hitting the full `MAX_REQUEST_BYTES`
/// cap takes on the order of tens of syscalls, not thousands.
const READ_CHUNK_BYTES: usize = 512;

/// Errors from [`HealthListener::bind`].
#[derive(Debug)]
pub enum HealthBindError {
    /// R-0004-g tripwire: the caller attempted to bind a non-loopback
    /// interface (anything other than the IPv4 loopback address
    /// `127.0.0.1`). The listener SHALL NOT silently bind a routable
    /// address — this variant is the fail-closed error path instead.
    NonLoopbackBind { addr: SocketAddr },
    /// The underlying `std::net::TcpListener::bind` call failed for a
    /// reason other than the loopback tripwire (e.g. port already in use,
    /// permission denied).
    Io(std::io::Error),
}

impl fmt::Display for HealthBindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthBindError::NonLoopbackBind { addr } => write!(
                f,
                "refused to bind /health listener to non-loopback address {addr} (R-0004-g tripwire)"
            ),
            HealthBindError::Io(e) => write!(f, "/health listener bind failed: {e}"),
        }
    }
}

impl std::error::Error for HealthBindError {}

/// Resolve the `/health` listener port from an optional env-var value.
///
/// Pure function — takes the env value as a plain argument rather than
/// reading `std::env` itself, specifically so it is unit-testable without
/// `std::env::set_var` (workspace canon, `skills/rust.md` `<P-TDDPairs>`
/// TF2: `env::set_var` flakes under parallel test execution). The
/// production call site (`run()`, T5) is expected to read
/// `std::env::var(HEALTH_PORT_ENV_VAR).ok()` and pass the `Option<&str>`
/// here.
///
/// Falls back to [`DEFAULT_HEALTH_PORT`] when `env_value` is `None` or does
/// not parse as a `u16` (an unparseable override must not panic the host).
pub fn resolve_port(env_value: Option<&str>) -> u16 {
    env_value
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(DEFAULT_HEALTH_PORT)
}

/// Shared readiness state, writer half.
///
/// Constructed together with its paired [`ReadinessSignal`] reader half via
/// [`ReadinessHandle::new`]. Neither half requires a `PgPool` to construct
/// or use (5a boundary — the listener answers `GET /health` as not-ready
/// before any pool exists).
///
/// Representation: `Arc<AtomicBool>`, shared with the paired
/// [`ReadinessSignal`]. `SeqCst` ordering is used for both the store here
/// and the load in [`ReadinessSignal::is_ready`] — this is a single boolean
/// flag read at most once per request on a low-throughput loopback
/// endpoint, not a hot path, so the strongest ordering is free to reason
/// about and costs nothing measurable.
pub struct ReadinessHandle {
    ready: Arc<AtomicBool>,
}

/// Shared readiness state, reader half. See [`ReadinessHandle`].
#[derive(Debug)]
pub struct ReadinessSignal {
    ready: Arc<AtomicBool>,
}

impl ReadinessHandle {
    /// Construct a fresh not-ready readiness pair.
    pub fn new() -> (ReadinessHandle, ReadinessSignal) {
        let ready = Arc::new(AtomicBool::new(false));
        (
            ReadinessHandle {
                ready: Arc::clone(&ready),
            },
            ReadinessSignal { ready },
        )
    }

    /// Mark the readiness state as ready. Idempotent; safe to call from any
    /// thread — the listener's accept-loop thread reads via
    /// [`ReadinessSignal`] on a different thread than `run()`'s startup
    /// sequence.
    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
    }
}

impl ReadinessSignal {
    /// `true` once the paired [`ReadinessHandle::mark_ready`] has been
    /// called at least once; `false` before that — including immediately
    /// after construction, with no `PgPool` ever having existed.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }
}

/// Shared cell holding the `PgPool` the listener queries once available.
///
/// Constructible via [`PoolCell::empty`] and clonable with **no** pool
/// present (5a boundary) — `run()` clones one [`PoolCell`] into
/// [`HealthListener::bind`] and keeps another to call [`PoolCell::set`]
/// later, once storage init completes (5c boundary). Both clones observe
/// the same underlying pool once set (GREEN's expected representation:
/// `Arc<RwLock<Option<PgPool>>>` or equivalent interior-mutable shared
/// cell).
#[derive(Clone)]
pub struct PoolCell {
    inner: Arc<RwLock<Option<PgPool>>>,
}

impl PoolCell {
    /// A pool cell with no pool yet.
    pub fn empty() -> PoolCell {
        PoolCell {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// Supply the pool. In production this is called once, at the 5c
    /// boundary; the method does not forbid a second call (overwrite
    /// semantics), since nothing in this module's contract depends on
    /// exactly-once.
    pub fn set(&self, pool: PgPool) {
        let mut guard = self
            .inner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(pool);
    }

    /// The current pool, if one has been [`PoolCell::set`]. `PgPool`'s
    /// `Clone` is cheap (an `Arc`-backed handle to the pool, not a new
    /// connection pool), so cloning out of the lock and releasing it
    /// immediately is preferable to holding the lock across the
    /// `health_snapshot` query.
    fn get(&self) -> Option<PgPool> {
        self.inner
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

/// `PgPool` itself has no meaningful text form; a manual, opaque `Debug`
/// avoids requiring `PgPool: Debug` just to satisfy `HealthListener`'s
/// derive.
impl fmt::Debug for PoolCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoolCell").finish_non_exhaustive()
    }
}

/// A bound, loopback-only `/health` HTTP listener (R-0004-g / R-0022-b).
///
/// Owns the HTTP wrapper only — the health-body computation is
/// `schema::init::health_snapshot`.
#[derive(Debug)]
pub struct HealthListener {
    listener: TcpListener,
    readiness: ReadinessSignal,
    pool: PoolCell,
}

impl HealthListener {
    /// Bind the `/health` listener.
    ///
    /// Fails closed with [`HealthBindError::NonLoopbackBind`] (R-0004-g
    /// tripwire) on any non-loopback address — it does not silently bind a
    /// routable interface (`skills/rust.md` `<control-code>` SF1/SF2: fail
    /// closed, enumerate the permitted set rather than the forbidden set).
    /// Any other bind failure (e.g. port already in use) surfaces as
    /// [`HealthBindError::Io`].
    ///
    /// Loopback test: `SocketAddr::ip().is_loopback()` — covers the full
    /// `127.0.0.0/8` range and IPv6 `::1`, not only the literal
    /// `127.0.0.1`. Dispatch-sanctioned resolution of the ambiguity the RED
    /// (T4) module doc flagged: the exact `127.0.0.2` / `::1` boundary is
    /// deliberately un-pinned by the test suite, and production callers
    /// (`run()`, T5+) pass exactly `Ipv4Addr::LOCALHOST` regardless of
    /// which loopback test is used here.
    pub fn bind(
        addr: SocketAddr,
        readiness: ReadinessSignal,
        pool: PoolCell,
    ) -> Result<HealthListener, HealthBindError> {
        if !addr.ip().is_loopback() {
            return Err(HealthBindError::NonLoopbackBind { addr });
        }
        let listener = TcpListener::bind(addr).map_err(HealthBindError::Io)?;
        Ok(HealthListener {
            listener,
            readiness,
            pool,
        })
    }

    /// The address actually bound — useful when [`HealthListener::bind`]
    /// was called with an OS-assigned port (i.e. port `0`), which is how
    /// this module's own tests avoid port collisions.
    pub fn local_addr(&self) -> SocketAddr {
        self.listener
            .local_addr()
            .expect("a successfully bound TcpListener always has a local_addr")
    }

    /// Run the accept loop. Blocks the calling thread — callers spawn this
    /// on a dedicated OS thread (`run()`, T5, does not drive this on the
    /// MCP-serving async task; R-0022-a's ordering guarantees are about
    /// readiness signals, not which OS thread runs which listener).
    ///
    /// Serves only `GET /health` (R-0004-g):
    /// - Method != `GET`, path == `/health` → `405 Method Not Allowed`.
    /// - Path != `/health` (any method) → `404 Not Found`.
    /// - `GET /health` before ready (or with no pool set yet) → `503` +
    ///   `{"ready": false}` (flagged interpretation — see module doc
    ///   "Not-ready response shape").
    /// - `GET /health` once ready with a pool set → `200` + the R-0004-g
    ///   structured detail body, computed by calling
    ///   `schema::init::health_snapshot` against the pool in the
    ///   [`PoolCell`] this listener was bound with.
    ///
    /// A connection error handling one request (a malformed request line,
    /// a peer that disconnects mid-read) is logged and the accept loop
    /// continues — one bad connection does not take the listener down.
    pub fn serve(self) -> Result<(), std::io::Error> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build the /health listener's single-threaded tokio runtime");

        for incoming in self.listener.incoming() {
            let stream = match incoming {
                Ok(stream) => stream,
                Err(err) => {
                    // A single accept() error (fd exhaustion, ECONNABORTED,
                    // EINTR, ...) must not permanently kill this listener
                    // thread (Low finding, dispatch 1249/1250) — the MCP
                    // server keeps serving, and a poller reading
                    // connection-refused afterward would misjudge the whole
                    // host as down. std's `incoming()` surfaces every
                    // accept() failure the same `io::Error` way; there is no
                    // distinct "fatal, stop the loop" variant to match here,
                    // so every error is logged and the loop continues. The
                    // brief sleep bounds a hot spin against a persistent
                    // failure (e.g. the fd limit staying exhausted).
                    tracing::warn!(error = %err, "/health listener: accept() failed, continuing");
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
            };
            if let Err(err) = handle_connection(&stream, &self.readiness, &self.pool, &rt) {
                tracing::warn!(error = %err, "/health listener: error handling a connection");
            }
        }
        Ok(())
    }
}

/// Handle exactly one request on `stream`: parse the request line, drain
/// the (bodiless, on this fixed single-route surface) remaining headers,
/// route, respond, close.
fn handle_connection(
    stream: &TcpStream,
    readiness: &ReadinessSignal,
    pool: &PoolCell,
    rt: &tokio::runtime::Runtime,
) -> std::io::Result<()> {
    // Per-connection wall-clock deadline (security re-review addendum,
    // dispatch 1249/1252 — "slow-drip residual", Low conf 85). Replaces the
    // old flat `set_read_timeout(5s)`: that bounded only idle time between
    // bytes, not a peer that keeps sending *some* byte within every 5s
    // window forever. `deadline` is fixed once, at accept; `CappedLineReader`
    // re-checks it before every underlying socket read (see its doc comment
    // for why per-line is not sufficient) so total time spent reading the
    // request line + headers cannot exceed CONNECTION_DEADLINE regardless of
    // how the peer paces its bytes.
    let deadline = Instant::now() + CONNECTION_DEADLINE;

    // Shares one byte budget (MAX_REQUEST_BYTES) across the request line and
    // every header line — a client streaming bytes with no `\n` cannot grow
    // the accumulated line without limit.
    let mut reader = CappedLineReader::new(stream, MAX_REQUEST_BYTES);

    let request_line = match reader.read_line(deadline)? {
        LineRead::Eof => {
            // Peer connected and disconnected without sending anything.
            return Ok(());
        }
        LineRead::TooLarge => {
            // Either MAX_REQUEST_BYTES was exhausted mid-line, or the peer
            // closed the connection mid-line — both are oversized/malformed
            // from this listener's perspective. Respond minimally and close
            // rather than routing on a truncated request line.
            return write_response(
                stream,
                431,
                &json!({"error": "request too large"}).to_string(),
            );
        }
        LineRead::DeadlineExceeded => {
            return write_response(
                stream,
                408,
                &json!({"error": "request timed out"}).to_string(),
            );
        }
        LineRead::Line(line) => line,
    };
    let mut tokens = request_line.split_whitespace();
    let method = tokens.next().unwrap_or_default().to_string();
    let path = tokens.next().unwrap_or_default().to_string();

    // Drain the remaining header lines. None of this surface's requests
    // carry a body (GET/HEAD/POST probes with no Content-Length), so
    // reading to the blank line that terminates the header block reads
    // exactly what the peer sent — needed so the peer's write completes
    // before this side closes the connection (closing early, with unread
    // bytes still in the socket's receive buffer, can surface as a
    // connection reset on the peer rather than the clean EOF the test
    // client's `read_to_end` expects). Bounded three ways: MAX_REQUEST_BYTES
    // (shared budget with the request line, above), MAX_HEADER_LINES
    // (defense-in-depth against a flood of short lines within budget), and
    // CONNECTION_DEADLINE (defense against a byte-at-a-time drip that stays
    // within both caps).
    let mut header_lines_drained = 0usize;
    loop {
        if header_lines_drained >= MAX_HEADER_LINES {
            return write_response(
                stream,
                431,
                &json!({"error": "request too large"}).to_string(),
            );
        }
        match reader.read_line(deadline)? {
            LineRead::Eof => break,
            LineRead::TooLarge => {
                // MAX_REQUEST_BYTES exhausted mid-header-line (or the peer
                // closed mid-line — same conflation as the request-line
                // case above).
                return write_response(
                    stream,
                    431,
                    &json!({"error": "request too large"}).to_string(),
                );
            }
            LineRead::DeadlineExceeded => {
                return write_response(
                    stream,
                    408,
                    &json!({"error": "request timed out"}).to_string(),
                );
            }
            LineRead::Line(line) => {
                if line == "\r\n" || line == "\n" {
                    break;
                }
                header_lines_drained += 1;
            }
        }
    }

    let (status, body) = route(&method, &path, readiness, pool, rt);
    write_response(stream, status, &body)
}

/// Outcome of one [`CappedLineReader::read_line`] call.
#[derive(Debug)]
enum LineRead {
    /// A complete `\n`-terminated line (CRLF or bare LF), read within both
    /// the byte budget and the wall-clock deadline.
    Line(String),
    /// The peer closed the connection with zero bytes read for this line —
    /// a clean, un-owed close, not a breach of either bound.
    Eof,
    /// The shared byte budget (`MAX_REQUEST_BYTES`) was exhausted with a
    /// partial, unterminated line pending, or the peer closed mid-line.
    /// Both map to the same 431 response — see the module's flood-hardening
    /// history (dispatch 1249/1250) for why they are not distinguished.
    TooLarge,
    /// `CONNECTION_DEADLINE` elapsed before a complete line was assembled —
    /// either the pre-read check found no time left, or the underlying
    /// socket read itself timed out against the deadline-derived
    /// `set_read_timeout`.
    DeadlineExceeded,
}

/// A byte-budget- and wall-clock-deadline-bounded line reader over a raw
/// `TcpStream`. Purpose-built replacement for `BufRead::read_line()` on this
/// surface — not a general-purpose reader.
///
/// # Why not `BufRead::read_line()` + a flat `set_read_timeout`
///
/// `set_read_timeout` bounds a single underlying `read()` syscall. Calling
/// it once and then calling `BufRead::read_line()` leaves that timeout fixed
/// for every syscall `read_line()` performs internally while hunting for
/// `\n` — and a peer that sends one byte every `(timeout - ε)` never lets
/// any single syscall time out, so `read_line()` keeps looping, bounded only
/// by the byte cap (`MAX_REQUEST_BYTES × timeout` ≈ 11 hours at the old 5s
/// timeout — the exact "slow-drip" finding this reader closes). A deadline
/// re-armed only *between* `read_line()` calls (i.e. once per logical line)
/// does not fix this either: a single never-terminated line — the actual
/// attack shape — is still one `read_line()` call, so the deadline is never
/// re-checked inside it.
///
/// This reader instead recomputes the remaining budget and re-arms
/// `set_read_timeout` before **every** underlying `read()` — the granularity
/// the deadline actually needs to bound total connection occupancy to
/// ~`CONNECTION_DEADLINE`, independent of drip rate or line count.
struct CappedLineReader<'a> {
    stream: &'a TcpStream,
    /// Bytes read from the socket but not yet returned as part of a
    /// completed line (a single `read()` can return more than one line's
    /// worth of bytes, or a partial next line).
    buf: Vec<u8>,
    /// Remaining shared byte budget across the whole connection — decremented
    /// by every byte actually read from the socket, mirroring the old
    /// `Read::take(MAX_REQUEST_BYTES)` behavior.
    bytes_budget: u64,
}

impl<'a> CappedLineReader<'a> {
    fn new(stream: &'a TcpStream, bytes_budget: u64) -> Self {
        CappedLineReader {
            stream,
            buf: Vec::new(),
            bytes_budget,
        }
    }

    /// Read one line (including its trailing `\n`), bounded by the shared
    /// byte budget and by `deadline`. See the struct doc comment for why the
    /// deadline is re-armed per socket read, not per call to this method.
    fn read_line(&mut self, deadline: Instant) -> std::io::Result<LineRead> {
        loop {
            if let Some(newline_idx) = self.buf.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = self.buf.drain(..=newline_idx).collect();
                return match String::from_utf8(line_bytes) {
                    Ok(line) => Ok(LineRead::Line(line)),
                    Err(err) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, err)),
                };
            }

            if self.bytes_budget == 0 {
                let had_data = !self.buf.is_empty();
                self.buf.clear();
                return Ok(if had_data {
                    LineRead::TooLarge
                } else {
                    LineRead::Eof
                });
            }

            let remaining = match deadline.checked_duration_since(Instant::now()) {
                Some(d) if !d.is_zero() => d,
                _ => return Ok(LineRead::DeadlineExceeded),
            };
            // set_read_timeout errors on a zero Duration; `remaining` is
            // guarded non-zero above.
            self.stream.set_read_timeout(Some(remaining))?;

            let take = (self.bytes_budget as usize).min(READ_CHUNK_BYTES);
            let mut chunk = [0u8; READ_CHUNK_BYTES];
            let n = match self.stream.read(&mut chunk[..take]) {
                Ok(n) => n,
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // The socket read itself hit the deadline-derived
                    // timeout — same disposition as the pre-read check
                    // above finding no time left.
                    return Ok(LineRead::DeadlineExceeded);
                }
                Err(err) => return Err(err),
            };
            if n == 0 {
                let had_data = !self.buf.is_empty();
                self.buf.clear();
                return Ok(if had_data {
                    LineRead::TooLarge
                } else {
                    LineRead::Eof
                });
            }
            self.bytes_budget -= n as u64;
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }
}

/// Route a parsed `(method, path)` to a `(status, json-body)` pair.
fn route(
    method: &str,
    path: &str,
    readiness: &ReadinessSignal,
    pool: &PoolCell,
    rt: &tokio::runtime::Runtime,
) -> (u16, String) {
    if path != "/health" {
        return (404, json!({"error": "not found"}).to_string());
    }
    if method != "GET" {
        return (405, json!({"error": "method not allowed"}).to_string());
    }

    let Some(pg_pool) = ready_pool(readiness, pool) else {
        return (503, json!({"ready": false}).to_string());
    };

    match rt.block_on(health_snapshot(&pg_pool)) {
        Ok(snapshot) => match serde_json::to_string(&snapshot) {
            Ok(body) => (200, body),
            Err(err) => (
                500,
                json!({"ready": false, "error": err.to_string()}).to_string(),
            ),
        },
        Err(err) => (
            500,
            json!({"ready": false, "error": err.to_string()}).to_string(),
        ),
    }
}

/// `Some(pool)` only once both the readiness gate has fired and a pool has
/// been supplied (5c boundary) — otherwise `None` (still at the 5a
/// boundary: not-ready, no pool, or both).
fn ready_pool(readiness: &ReadinessSignal, pool: &PoolCell) -> Option<PgPool> {
    if !readiness.is_ready() {
        return None;
    }
    pool.get()
}

/// Write a JSON HTTP/1.1 response and close the connection. `Connection:
/// close` is not just declared but honored — this handler serves one
/// request per accepted connection, matching the test client's
/// read-until-EOF expectation.
fn write_response(stream: &TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
        status = status,
        reason = reason_phrase(status),
        len = body.len(),
    );
    let mut writer = stream;
    writer.write_all(response.as_bytes())?;
    writer.flush()?;
    let _ = stream.shutdown(std::net::Shutdown::Both);
    Ok(())
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    }
}
