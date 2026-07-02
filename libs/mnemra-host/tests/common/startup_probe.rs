//! Shared probe helpers for the T5 startup-ordering acceptance suites
//! (`startup_run_ordering.rs`, `startup_run_full.rs`).
//!
//! Included per-file via `#[path = "common/startup_probe.rs"] mod ...;` — the
//! same selective-include pattern as `common/slice1_harness.rs`, so a file
//! pulls only the helpers it needs and no unrelated harness code.
//!
//! `http_roundtrip` is adapted from `tests/health_listener.rs` (T4), whose
//! inline copy carries a "extract to tests/common/ if a second health-surface
//! test file needs them" note — the T5 files are that second (and third)
//! user. The T4 file's own copy is left untouched to avoid churning a merged
//! green suite; unifying the three is a follow-up.
#![allow(dead_code)] // each including test binary uses a subset of these helpers

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The repo root of this workspace checkout — two directories above
/// `libs/mnemra-host` (same resolution as production's component-path
/// helpers). The real `plugins/` and `artifacts/` trees live here; tests
/// read them, never write them.
pub fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root resolution from libs/mnemra-host")
        .to_path_buf()
}

/// Reserve a loopback address with an OS-assigned port, then release it.
///
/// Used by tests that must probe a port *after* `run_with` returns `Err`
/// (no `RunHandle` exists on the failure path, so the port cannot be
/// discovered — it must be chosen up front). There is a small window between
/// the release here and the bind inside `run_with` in which another process
/// could grab the port; nothing else in the test environment binds
/// OS-assigned loopback ports it did not itself reserve, so the race is
/// accepted (mirrors the double-bind probe pattern in health_listener.rs).
pub fn reserve_loopback_addr() -> SocketAddr {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .expect("reserving a loopback port must succeed");
    let addr = listener
        .local_addr()
        .expect("a bound TcpListener always has a local_addr");
    drop(listener);
    addr
}

/// Write an admin-token file with the given Unix mode and return its path.
///
/// Content is a per-run random value (no secret literals in tests — workspace
/// canon); the 5-pre check reads only the file's *metadata*, never its bytes.
pub fn write_admin_token_file(dir: &Path, mode: u32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join("admin-token");
    std::fs::write(&path, uuid::Uuid::new_v4().to_string())
        .expect("writing the test admin-token file must succeed");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
        .expect("setting the test admin-token file mode must succeed");
    path
}

/// Send one raw HTTP/1.1 request and read the response until the peer closes
/// the connection. Returns `(status_code, body_string)`.
///
/// Adapted from `tests/health_listener.rs::http_roundtrip` (T4) — the
/// listener under test serves one request per connection with
/// `Connection: close`.
pub fn http_roundtrip(addr: SocketAddr, method: &str, path: &str) -> (u16, String) {
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
