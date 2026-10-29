//! The REAL `verify-smoke` gate (R-0022-c) — T6, task #1993.
//!
//! # Purpose
//!
//! Retires the `if true` placeholder `verify-smoke` recipe (`justfile`).
//! Exercises the **production** integrity-gated path end-to-end: spawns the
//! actual `mnemra` binary (never the plugin — this file does not build or
//! touch `mnemra-echo`) from the repo root, and asserts:
//!
//! 1. A real MCP 2025-06-18 `initialize` handshake + `list_tools` catalog
//!    call succeed over the child's own stdin/stdout (R-0022-c serve-loop;
//!    R-0010-a stdio transport) — production startup (embedded Postgres,
//!    builtins, the verified plugin pool) runs to completion first; the
//!    handshake attempt simply waits on the pipe, so no separate timed
//!    "startup complete" precondition races the child's own lifecycle.
//! 2. Once the handshake succeeds, `/health` answers ready with an `ok`
//!    overall status (R-0022-b) and the child owns exactly one listening
//!    TCP port — that `/health` listener — and no other (stdio-only;
//!    R-0010-e's runtime half; the compile-time half — no streamable-http
//!    rmcp feature compiled in — is `tests/mcp_feature_guard.rs`, a
//!    standing guardrail).
//! 3. `BLAKE3(committed artifact bytes) == [component].hash` in the
//!    committed, real-signed manifest — the bytes the production path would
//!    serve are the bytes that were signed (§ Constraints, spec
//!    `2026-06-30-signing-to-runnable.md`).
//!
//! # R-ID mapping
//!
//! | Test function                                              | R-ID(s)               |
//! |-------------------------------------------------------------|------------------------|
//! | committed_artifact_hash_matches_signed_manifest_hash         | R-0022-c smoke gate    |
//! | production_binary_serves_mcp_over_stdio_with_health_only_listener | R-0022-c serve-loop + smoke gate, R-0010-a/-e |
//!
//! # Test design notes (task #1993)
//!
//! `committed_artifact_hash_matches_signed_manifest_hash` is a pure
//! file-property check with no dependency on `RunHandle::serve_stdio` — it
//! is expected **GREEN today** (M1 already landed the real signed artifact
//! + `[component].hash` pairing; T6 does not change either file).
//!
//! It is included here — rather than only inside the live-serve test — so
//! the hash-equality property (locked AC #3) has an isolated, fast failure
//! signal independent of the (slow) subprocess scenario, and because the
//! spec names it as one of the smoke gate's own binary-observables.
//!
//! `production_binary_serves_mcp_over_stdio_with_health_only_listener`
//! exercises `RunHandle::serve_stdio` (`mnemra_host.rs`), which `run()`
//! calls after `run_with` succeeds (T5's implementation). The
//! `postgres engine started` / `plugin load` `tracing` lines (R-0022-a's
//! shared 5b-i/5b-ii observable, proof every prior boundary held) land on
//! **stderr**, not stdout: `cmd/mnemra/logging.rs` writes tracing to
//! stderr (`.with_writer(std::io::stderr)`, `logging.rs:39`) precisely so
//! stdout stays reserved exclusively for the MCP JSON-RPC wire protocol
//! this test's MCP client reads — this is why the test drains the
//! child's stderr separately (see `stderr_task` below): stderr is where
//! the tracing diagnostic evidence actually lands, captured continuously
//! so it's available for failure messages, while stdout is left
//! untouched as the live wire protocol. The handshake itself is the live
//! guard on that stdout exclusivity — a tracing line landing on stdout
//! would corrupt the wire protocol and the handshake would fail outright,
//! not just a diagnostics inconvenience. A panic in `main` would also
//! kill the WHOLE child process, including its detached `/health` OS
//! thread (unlike an `Err` return that `run_with`'s own test-suite observes
//! from *within* the same process), so a separate poll-for-health-ready
//! precondition is deliberately NOT used here — the handshake attempt
//! itself simply waits on the pipe for as long as production startup
//! takes, and any startup or serve-loop failure surfaces as a transport /
//! connection-closed error rather than racing a separate readiness check.
//!
//! # No `TokioChildProcess` (rmcp `transport-child-process` feature is not
//! enabled in this workspace — `Cargo.toml` is outside T6's touch scope).
//! The child's `(ChildStdout, ChildStdin)` pair is fed to `serve_client`
//! directly via rmcp's generic `(R, W)` `IntoTransport` blanket impl
//! (`transport-async-rw`, already active via the `server` feature) — the
//! same pattern `tests/mcp_server.rs` uses for its in-process
//! `tokio::io::duplex` transport, just over the child's real stdio pipes.
//!
//! # No hardcoded secrets
//!
//! The admin-token file's *content* is an opaque per-run UUID
//! (`startup_probe::write_admin_token_file`) — the 5-pre check reads only
//! the file's Unix mode, never its bytes.

#[path = "common/startup_probe.rs"]
mod startup_probe;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// Bounded wait for the MCP handshake to complete — covers BOTH full
/// production startup (embedded-Postgres, builtins, the verified plugin
/// pool — real wall-clock work, T5 precedent: sub-second when warm,
/// potentially much longer on a cold CI runner downloading Postgres
/// binaries) AND the handshake itself. Generous so a GREEN implementation
/// never trips it; a RED run resolves far sooner, on the child's exit
/// (EOF), not on this timeout.
const HANDSHAKE_CEILING: Duration = Duration::from_secs(150);

/// Bounded wait for `list_tools` once the handshake has already completed
/// (a live serve loop's expected latency here is sub-second).
const LIST_TOOLS_CEILING: Duration = Duration::from_secs(30);

/// Bounded wait for the host to exit after its stdin closes. MEASURED
/// 2026-10-29: about 155 ms in both tests that use it (3 runs each, spread
/// under 2 ms), so 10 s is roughly 60x headroom for a loaded machine. The
/// bound exists because a SIGKILL instead of a clean exit orphans the
/// daemonized embedded postmaster (#4370).
const EOF_EXIT_CEILING: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------
// Checkpoint 4 — hash-equality (pure; expected GREEN today, see module doc)
// ---------------------------------------------------------------------------

/// R-0022-c smoke gate: `BLAKE3(committed artifact)` equals the signed
/// `[component].hash` in the committed, real-signed manifest — the bytes
/// the production integrity-gated path would serve are the bytes that were
/// signed. Pure file-property check; no subprocess, no network.
#[test]
fn committed_artifact_hash_matches_signed_manifest_hash() {
    let repo_root = startup_probe::repo_root();

    let wasm_path = repo_root.join("artifacts/mnemra-echo/mnemra_echo.wasm");
    let wasm_bytes = std::fs::read(&wasm_path).unwrap_or_else(|e| {
        panic!("committed signed artifact must be readable at {wasm_path:?}: {e}")
    });
    let recomputed_hash = blake3::hash(&wasm_bytes).to_hex().to_string();

    let manifest_path = repo_root.join("plugins/mnemra-echo/manifest.toml");
    let manifest_text = std::fs::read_to_string(&manifest_path).unwrap_or_else(|e| {
        panic!("committed manifest must be readable at {manifest_path:?}: {e}")
    });
    let signed_hash = parse_component_hash(&manifest_text).unwrap_or_else(|| {
        panic!(
            "committed manifest at {manifest_path:?} must carry a [component].hash field \
             (M1, R-0021, must already be landed)"
        )
    });

    assert_eq!(
        recomputed_hash, signed_hash,
        "R-0022-c smoke gate: BLAKE3(committed artifact {wasm_path:?}) must equal the signed \
         [component].hash in {manifest_path:?} — the bytes the production integrity-gated path \
         would serve must be the bytes that were signed"
    );
}

/// Read `[component].hash` from manifest TOML text via a real TOML parse
/// (not a substring scan — `skills/bdd.md` <rules>: forbidden/required-token
/// assertions over structured text must parse structurally).
fn parse_component_hash(manifest_toml: &str) -> Option<String> {
    let value: toml::Value = manifest_toml.parse().ok()?;
    value
        .get("component")?
        .get("hash")?
        .as_str()
        .map(str::to_owned)
}

// ---------------------------------------------------------------------------
// Checkpoints 1-3 — the live serve-loop scenario (T6, RunHandle::serve_stdio)
// ---------------------------------------------------------------------------

/// Live serve end-to-end (spec Scenario "Live serve end-to-end", R-0022-c).
///
/// Spawns the real `mnemra` binary (no subcommand → `mnemra_host::run()`)
/// from the repo root, drives a real MCP `initialize` handshake +
/// `list_tools` over the child's stdio, then asserts `/health` is ready and
/// the child owns no listening TCP port besides it.
#[tokio::test]
async fn production_binary_serves_mcp_over_stdio_with_health_only_listener() {
    let repo_root = startup_probe::repo_root();

    // -- fixtures: an admin-token file (mode 600; content is opaque — 5-pre
    // reads only metadata) and a reserved loopback health port. This test
    // spawns a REAL subprocess (not a RunConfig injection), so there is no
    // in-process seam to hand these in with — the existing production env
    // surface (MNEMRA_TOKEN_FILE, MNEMRA_HEALTH_PORT) is what a real
    // deployment would also use. --
    let tmp = tempfile::tempdir().expect("tempdir");
    let token_path = startup_probe::write_admin_token_file(tmp.path(), 0o600);
    let health_addr = startup_probe::reserve_loopback_addr();

    // -- the HOST binary, never the plugin. `verify-smoke`'s `build`
    // prerequisite (bare `cargo build`, default-members = ["cmd/mnemra"] —
    // never `plugin`) is expected to have produced this by the time this
    // test runs. --
    let binary_path = repo_root.join("target/debug/mnemra");
    assert!(
        binary_path.is_file(),
        "the host binary must already be built at {binary_path:?} — verify-smoke's `build` \
         prerequisite (never `plugin`) is expected to produce it before this test runs"
    );

    // -- spawn the REAL production binary, from the repo root (run()
    // resolves root_dir via CARGO_MANIFEST_DIR at COMPILE time, not cwd —
    // this matters only for realism: don't relocate the binary out of its
    // checkout). --
    let mut child = Command::new(&binary_path)
        .current_dir(&repo_root)
        .env("MNEMRA_TOKEN_FILE", &token_path)
        .env("MNEMRA_HEALTH_PORT", health_addr.port().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawning the production mnemra binary must succeed");
    let pid = child
        .id()
        .expect("a freshly spawned child process has a pid");

    // Drain stderr continuously (not just on failure) — the child's
    // `tracing` logs are the reliable, ordered evidence of what happened
    // before any failure (see module doc: a panic in the child's main
    // thread kills its detached /health thread too, so a separate
    // poll-for-health-ready precondition would race the panic instead of
    // reliably diagnosing it). Draining also prevents the child blocking on
    // a full stderr pipe buffer.
    let mut stderr_pipe = child.stderr.take().expect("stderr was piped");
    let stderr_task: tokio::task::JoinHandle<Vec<u8>> = tokio::spawn(async move {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf).await;
        buf
    });

    // -- the real MCP initialize handshake + list_tools, over the child's
    // own stdin/stdout — never a TCP socket (R-0010-a/-e). No separate
    // "wait for startup" precondition: the handshake attempt itself simply
    // waits on the pipe for as long as production startup takes. --
    let stdout = child.stdout.take().expect("stdout was piped");
    let stdin = child.stdin.take().expect("stdin was piped");

    let handshake = tokio::time::timeout(
        HANDSHAKE_CEILING,
        rmcp::service::serve_client((), (stdout, stdin)),
    )
    .await;
    let client = match handshake {
        Err(_elapsed) => {
            let _ = child.start_kill();
            let stderr_tail = collect_stderr_tail(stderr_task).await;
            panic!(
                "MCP initialize handshake did not complete within {HANDSHAKE_CEILING:?} over \
                 stdio — the handshake is expected to succeed once production startup \
                 completes; a hang here is a genuine startup or serve-loop regression, not \
                 a known failure mode. Child stderr tail:\n{stderr_tail}"
            );
        }
        Ok(Err(e)) => {
            let _ = child.start_kill();
            let stderr_tail = collect_stderr_tail(stderr_task).await;
            panic!(
                "MCP initialize handshake over stdio failed: {e:?} — RunHandle::serve_stdio \
                 (mnemra_host.rs) is implemented and this handshake is expected to succeed; \
                 a transport/connection-closed error here is a genuine regression in startup \
                 or the serve loop, not a build error or hash mismatch (which \
                 committed_artifact_hash_matches_signed_manifest_hash already checks \
                 independently of this subprocess). Child stderr tail:\n{stderr_tail}"
            );
        }
        Ok(Ok(client)) => client,
    };

    let tools = tokio::time::timeout(LIST_TOOLS_CEILING, client.list_all_tools())
        .await
        .expect("list_tools must not hang once the handshake completed")
        .expect("list_tools must succeed once the handshake completed");
    let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(
        tool_names.contains(&"echo.create"),
        "list_tools must advertise the manifest's exposed verbs (plugins/mnemra-echo/manifest.toml); \
         got {tool_names:?}"
    );

    // -- the handshake succeeded, so the server is definitively alive and
    // serving: /health must answer ready+ok now (no race — nothing before
    // this point required health to be reachable). --
    let (status, body) = try_http_get(health_addr, "/health").unwrap_or_else(|| {
        panic!("/health at {health_addr} did not answer after a successful MCP handshake")
    });
    assert_eq!(
        status, 200,
        "/health must answer 200 once serving; body: {body}"
    );
    assert!(
        body.contains("\"overall\":\"ok\""),
        "/health answered but reports a non-ok overall status: {body}"
    );

    // -- stdio-only: the child owns exactly one listening TCP port (the
    // loopback /health listener) and no other. --
    let listening_ports = listening_tcp_ports(pid);
    assert_eq!(
        listening_ports,
        vec![health_addr.port()],
        "the child process must own exactly one listening TCP port — the loopback /health \
         listener — and no other (no HTTP MCP transport, R-0010-a/-e); got {listening_ports:?}"
    );

    // -- graceful shutdown: close our end of the transport (the child's
    // stdin) — a GREEN serve loop is expected to end on stdin EOF (R-0010-a
    // stdio-only). Force-kill as a backstop regardless of GREEN's exact
    // shutdown semantics, so this test never leaks a process. --
    let _ = client.cancel().await;
    let exit = tokio::time::timeout(EOF_EXIT_CEILING, child.wait()).await;
    let _ = child.start_kill();
    assert!(
        exit.is_ok(),
        "the host must exit within {EOF_EXIT_CEILING:?} of its stdin closing; the kill above \
         is the backstop and orphans the embedded Postgres postmaster (#4370)"
    );
}

/// Pins the production wiring of the incoming-line cap (#4295): an over-cap line
/// written to the REAL binary's stdin ends the session visibly (stdout closes and
/// the process exits) rather than being dropped with the pipe left open.
///
/// A source-level check could not tell whether `serve_stdio` is wired to the capped
/// transport; only the spawned binary can. Nothing is written to stdout: it
/// carries MCP only.
#[tokio::test]
async fn production_binary_ends_the_session_on_an_over_cap_incoming_line() {
    let repo_root = startup_probe::repo_root();
    let tmp = tempfile::tempdir().expect("tempdir");
    let token_path = startup_probe::write_admin_token_file(tmp.path(), 0o600);
    let health_addr = startup_probe::reserve_loopback_addr();
    let binary_path = repo_root.join("target/debug/mnemra");
    assert!(
        binary_path.is_file(),
        "the host binary must already be built at {binary_path:?} — verify-smoke's `build` \
         prerequisite is expected to produce it before this test runs"
    );

    let mut child = Command::new(&binary_path)
        .current_dir(&repo_root)
        .env("MNEMRA_TOKEN_FILE", &token_path)
        .env("MNEMRA_HEALTH_PORT", health_addr.port().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawning the production mnemra binary must succeed");
    let mut stderr_pipe = child.stderr.take().expect("stderr was piped");
    let stderr_task: tokio::task::JoinHandle<Vec<u8>> = tokio::spawn(async move {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf).await;
        buf
    });
    let mut stdout = child.stdout.take().expect("stdout was piped");
    let mut stdin = child.stdin.take().expect("stdin was piped");

    // One byte past the cap, no newline. Written from a task because the pipe
    // fills until the child finishes starting up and begins reading; a write
    // error after the child has closed its end is the expected outcome.
    let over_cap = vec![b'x'; mnemra_host::mcp::transport::MAX_INCOMING_LINE_BYTES + 1];
    let writer = tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        let _ = stdin.write_all(&over_cap).await;
        stdin
    });

    let mut stdout_bytes = Vec::new();
    let closed =
        tokio::time::timeout(HANDSHAKE_CEILING, stdout.read_to_end(&mut stdout_bytes)).await;
    if closed.is_err() {
        let _ = child.start_kill();
        let stderr_tail = collect_stderr_tail(stderr_task).await;
        panic!(
            "stdout did not close within {HANDSHAKE_CEILING:?} of an over-cap incoming line — \
             the line was dropped with the session left open, or the binary is not wired to the \
             capped transport. Child stderr tail:\n{stderr_tail}"
        );
    }
    assert!(
        stdout_bytes.is_empty(),
        "stdout carries MCP only; an over-cap line before initialize must produce no output, \
         got {} bytes",
        stdout_bytes.len()
    );
    let status = tokio::time::timeout(Duration::from_secs(30), child.wait())
        .await
        .expect("the binary exits once its MCP session ends")
        .expect("wait on the child");
    assert!(
        !status.success(),
        "a session ended by an over-cap line is a failed serve, not a clean shutdown; got {status:?}"
    );
    writer.abort();

    // The session must end because of the cap, not by some other failure: the
    // child's log carries the reader's error text.
    let stderr_bytes = tokio::time::timeout(Duration::from_secs(5), stderr_task)
        .await
        .expect("stderr closes once the child has exited")
        .expect("stderr drain task did not panic");
    let stderr_text = String::from_utf8_lossy(&stderr_bytes);
    assert!(
        stderr_text.contains("incoming line exceeds the"),
        "the child's stderr must carry the LineCappedReader cap error; got:\n{stderr_text}"
    );
}

/// Pins the production wiring of the initialize gate (#4295): against the REAL
/// binary, a first message that is not `initialize` but carries complete inline
/// `_meta` naming a released version is refused with -32600 "initialize
/// required", where rmcp alone would serve it.
#[tokio::test]
async fn production_binary_refuses_a_request_before_initialize() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let repo_root = startup_probe::repo_root();
    let tmp = tempfile::tempdir().expect("tempdir");
    let token_path = startup_probe::write_admin_token_file(tmp.path(), 0o600);
    let health_addr = startup_probe::reserve_loopback_addr();
    let binary_path = repo_root.join("target/debug/mnemra");
    assert!(
        binary_path.is_file(),
        "the host binary must already be built at {binary_path:?} — verify-smoke's `build` \
         prerequisite is expected to produce it before this test runs"
    );

    let mut child = Command::new(&binary_path)
        .current_dir(&repo_root)
        .env("MNEMRA_TOKEN_FILE", &token_path)
        .env("MNEMRA_HEALTH_PORT", health_addr.port().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawning the production mnemra binary must succeed");
    let mut stderr_pipe = child.stderr.take().expect("stderr was piped");
    let stderr_task: tokio::task::JoinHandle<Vec<u8>> = tokio::spawn(async move {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf).await;
        buf
    });
    let stdout = child.stdout.take().expect("stdout was piped");
    let mut stdin = child.stdin.take().expect("stdin was piped");

    let request = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/list",
        "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "2025-11-25",
            "io.modelcontextprotocol/clientCapabilities": {}
        }}
    });
    let mut line = serde_json::to_vec(&request).expect("request serializes");
    line.push(b'\n');
    stdin.write_all(&line).await.expect("write request");

    let mut lines = BufReader::new(stdout).lines();
    let reply = tokio::time::timeout(HANDSHAKE_CEILING, lines.next_line()).await;
    // End the session by closing stdin and let the host shut its embedded
    // Postgres down. A bare SIGKILL here orphans the daemonized postmaster
    // (pg_ctl detaches it to PID 1), one per green run (#4370). The kill
    // below is only the backstop for a host that does not exit.
    drop(stdin);
    let exit = tokio::time::timeout(EOF_EXIT_CEILING, child.wait()).await;
    let _ = child.start_kill();
    let reply = match reply {
        Ok(Ok(Some(reply))) => reply,
        other => {
            let stderr_tail = collect_stderr_tail(stderr_task).await;
            panic!("no reply line from the binary: {other:?}. Child stderr tail:\n{stderr_tail}");
        }
    };
    let reply: serde_json::Value = serde_json::from_str(&reply).expect("reply is JSON");
    assert_eq!(
        reply["error"]["code"],
        serde_json::json!(-32600),
        "a pre-initialize request must be refused by the gate; got {reply}"
    );
    assert_eq!(
        reply["error"]["message"],
        serde_json::json!("initialize required"),
        "got {reply}"
    );
    // No exit status is asserted: before initialize, EOF ends the session
    // through serve_stdio's `?`, and 0 and 1 are both clean exits.
    assert!(
        exit.is_ok(),
        "the host must exit within {EOF_EXIT_CEILING:?} of its stdin closing; the kill above \
         is the backstop and orphans the embedded Postgres postmaster (#4370)"
    );
}

/// Join the stderr-draining task with a short bound and render its buffer
/// as a display-friendly tail (last 4 KiB) for a panic message. Never
/// panics itself — diagnostic-only, used from inside another panic path.
async fn collect_stderr_tail(task: tokio::task::JoinHandle<Vec<u8>>) -> String {
    let buf = tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .ok()
        .and_then(|joined| joined.ok())
        .unwrap_or_default();
    let text = String::from_utf8_lossy(&buf);
    let tail_start = text.len().saturating_sub(4096);
    text[tail_start..].to_string()
}

/// One best-effort `GET {path}` — `None` on any connect/IO failure,
/// `Some((status, body))` on a completed HTTP round-trip. Non-panicking
/// counterpart to `startup_probe::http_roundtrip` (which panics on connect
/// failure — correct for its callers, which probe only once a listener is
/// already known to be up).
fn try_http_get(addr: SocketAddr, path: &str) -> Option<(u16, String)> {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(500)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;
    stream.flush().ok()?;

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw).into_owned();

    let mut parts = text.splitn(2, "\r\n\r\n");
    let head = parts.next().unwrap_or_default();
    let body = parts.next().unwrap_or_default().to_string();
    let status: u16 = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())?;
    Some((status, body))
}

/// The set of TCP ports process `pid` is LISTENING on, via `lsof` (present
/// on macOS and the `ubuntu-latest` GitHub Actions runner image this
/// workspace's CI uses — a system utility, not a new crate dependency).
/// Black-box by necessity: proving the ABSENCE of a listener is an
/// OS-process-table property no in-process Rust seam can observe from
/// outside the process (`skills/rust.md` <testing>: a test-only in-process
/// seam would need to be feature-gated at minimum, and this property does
/// not need one — `lsof` reads the OS's own view of the process's sockets,
/// no production code changes at all).
fn listening_tcp_ports(pid: u32) -> Vec<u16> {
    let output = std::process::Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-iTCP", "-sTCP:LISTEN", "-Fn"])
        .output()
        .expect(
            "lsof must be runnable in this environment (macOS / ubuntu-latest CI both ship it)",
        );
    // `-Fn` output: one 'n'-prefixed line per matching socket, e.g.
    // "n*:8877" or "n127.0.0.1:8877". A process with zero listening sockets
    // produces zero 'n' lines (and a non-zero lsof exit, which is NOT
    // treated as a hard error here — only presence/absence of 'n' lines is
    // read).
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut ports: Vec<u16> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .filter_map(|addr| addr.rsplit(':').next())
        .filter_map(|port_str| port_str.parse::<u16>().ok())
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports
}
