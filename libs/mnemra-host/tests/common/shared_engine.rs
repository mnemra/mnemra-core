//! Shared-engine fixture (R-0026/R-0027/R-0028/R-0029, Task 2 GREEN).
//!
//! Single get-or-init entry point every fixture-consuming member binary
//! acquires its embedded Postgres engine through. The first caller boots;
//! every later caller — including two callers racing concurrently
//! in-process — observes the same `&'static EmbeddedEngine` with exactly one
//! boot (R-0026 AC3). Once-semantics live HERE, not in `--test-threads 1`
//! (R-0026 Decision).
//!
//! Included by consumers via `#[path = "common/shared_engine.rs"] mod
//! shared_engine;` (mirrors `tests/common/paging_harness.rs`'s inclusion
//! pattern) — never edits `common/mod.rs` (byte-locked, R-0037/SO-3).
//!
//! # Teardown (R-0028)
//!
//! The engine lives in a `'static` `OnceCell` — Rust statics never drop, so
//! `Drop`-based teardown is structurally impossible here. Deterministic
//! exit-time teardown is instead registered via `libc::atexit` (already a
//! `mnemra-host` dependency — no new crate) exactly once, at the point the
//! engine is first booted. `libc::atexit` handlers run on every normal
//! process exit — the all-tests-passed path AND the some-tests-failed path
//! both route through C's `exit()` (libtest's synthesized `main()` returns
//! normally, or calls `std::process::exit(101)` after catching per-test
//! panics) — so teardown fires on both (Scenario S6).

use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::OnceCell;

static ENGINE: OnceCell<EmbeddedEngine> = OnceCell::const_new();
static BOOT_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Get-or-init this binary's single shared embedded engine.
///
/// The first caller boots the engine; every later caller — including two
/// callers racing concurrently in-process — reuses the same instance
/// (`tokio::sync::OnceCell::get_or_try_init`: if two callers race, only one's
/// future actually runs the initializer, the other awaits the same in-flight
/// result). No manual locking, no new dependency.
///
/// # Panics
///
/// Panics if the engine fails to start — there is no meaningful fallback for
/// a test binary whose engine never came up.
pub async fn shared_engine() -> &'static EmbeddedEngine {
    ENGINE
        .get_or_try_init(|| async {
            let engine = EmbeddedEngine::start().await?;
            let n = BOOT_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
            // R-0026 AC1 evidence path: one-per-boot marker line. `eprintln!`,
            // not `tracing::info!` — a tracing event with no installed
            // subscriber is a silent no-op; this line must actually land in
            // the flake-runner log
            // (scratch/flake-runs/<bin>_run<i>_t<threads>.log) unconditionally.
            eprintln!("FIXTURE_BOOT engine_boot_number={n}");
            // Register exit-time teardown exactly once, at the point the
            // engine first exists — never depends on a second call site
            // remembering to. Safe to register unconditionally here: this
            // closure body runs at most once per process, guaranteed by
            // `OnceCell::get_or_try_init`.
            //
            // SAFETY: `libc::atexit` requires a valid `extern "C" fn()`
            // pointer with `'static` lifetime and no captured state —
            // `teardown_shared_engine` is exactly that (a bare fn item, no
            // closure environment).
            unsafe { libc::atexit(teardown_shared_engine) };
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(engine)
        })
        .await
        .expect("shared embedded engine failed to start")
}

/// Programmatic boot-count observable for R-0026 AC3's concurrent-acquisition
/// test — NOT a proxy. Pointer-equality of two `&'static` returns only proves
/// "same instance" (an `OnceCell` implementation detail), not "booted exactly
/// once". `boot_count` is an independent `AtomicUsize` bumped once inside the
/// init closure above, so it is not a proxy.
pub fn boot_count() -> usize {
    BOOT_COUNT.load(Ordering::SeqCst)
}

/// Exit-time teardown callback, registered once via `libc::atexit` inside
/// [`shared_engine`]'s init closure.
///
/// Calls `EmbeddedEngine::shutdown` to completion (R-0028: explicit,
/// deterministic teardown — never `Drop` of a `static`). Serial, no fan-out
/// (SO-2): nothing here touches `PluginPool`/wasmtime lifecycle, and exactly
/// one shutdown is in flight at a time.
///
/// # Why this spawns a fresh OS thread instead of just `block_on`-ing here
///
/// Empirically verified (not assumed) to be load-bearing: a `libc::atexit`
/// handler runs on the *original calling thread*, very late in process
/// shutdown — after Rust's own main-thread cleanup has already torn down
/// that thread's `std::thread::current()` thread-local. Tokio's
/// `Runtime::block_on` (even a throwaway current-thread runtime built fresh
/// right here) parks the calling thread while awaiting, and its park loop
/// calls `std::thread::current()` on THAT thread — which panics with "use of
/// std::thread::current() is not possible after the thread's local data has
/// been destroyed" (a non-unwinding panic, observed as SIGABRT). The same
/// applies to `JoinHandle::join()` on the calling thread (also park-based).
///
/// A brand-new OS thread has fresh, not-yet-destroyed thread-locals, so
/// Tokio's runtime machinery works normally *there*. The calling (atexit)
/// thread never touches `thread::current()`/park/join itself — it only
/// busy-polls a plain `AtomicBool` with a short sleep, neither of which
/// touches thread-locals.
///
/// # Hardening (review-gate findings, #2049)
///
/// Two Low-severity findings on this mechanism (both reviewers) are folded
/// in here, conservatively — same spawned-thread + `AtomicBool`-poll shape,
/// made fail-safe:
///
/// - **Deadline-bounded wait (guaranteed backstop).** The poll loop below
///   exits when `TEARDOWN_DONE` flips OR `TEARDOWN_WAIT_DEADLINE` elapses,
///   whichever comes first. Without this, a shutdown chain that panicked
///   inside the spawned thread would never flip the done-flag and the poll
///   loop would hang the process forever at exit — invisible to a SIGABRT
///   scan, surfacing later as an inscrutable CI timeout. On deadline-exceeded
///   the process exits anyway; the postmaster/temp-dir may leak, which is
///   the accepted A-12-class residual and vastly preferable to an infinite
///   hang.
/// - **Panic-catching (belt-and-suspenders).** The spawned thread wraps its
///   shutdown chain in `catch_unwind` so a panic is reported via `eprintln!`
///   instead of silently vanishing, and `TEARDOWN_DONE` is set on every
///   path — success, error, or panic — so the poll loop is never left
///   waiting on a flag that will never flip.
extern "C" fn teardown_shared_engine() {
    // Never booted (e.g. every test in this binary was filtered out before
    // the first `shared_engine()` call) — nothing to tear down.
    let Some(engine) = ENGINE.get() else {
        return;
    };

    static TEARDOWN_DONE: AtomicBool = AtomicBool::new(false);

    let spawned = std::thread::Builder::new()
        .name("fixture-teardown".into())
        .spawn(move || {
            // Catch a panic anywhere in the shutdown chain so it is
            // surfaced (via `eprintln!`, below) instead of silently leaving
            // `TEARDOWN_DONE` unset, which would hang the poll loop forever.
            //
            // `AssertUnwindSafe`: the wrapped closure builds a fresh runtime
            // and awaits `engine.shutdown()` entirely inside this catch —
            // `engine` is a `&'static EmbeddedEngine` shared reference, and
            // nothing outside this closure observes any broken invariant if
            // the shutdown chain panics mid-await, because the process is
            // already exiting via `atexit` and no other code touches
            // `engine` afterward. Sound here specifically because this is
            // the terminal use of `engine` in the process's lifetime.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())
                    .and_then(|rt| rt.block_on(engine.shutdown()).map_err(|e| e.to_string()))
            }));

            match outcome {
                Ok(Ok(())) => {}
                Ok(Err(e)) => eprintln!("FIXTURE_TEARDOWN engine shutdown failed: {e}"),
                Err(panic_payload) => {
                    eprintln!(
                        "FIXTURE_TEARDOWN engine shutdown panicked: {}",
                        panic_message(&*panic_payload)
                    );
                }
            }
            // Set on ALL paths (success, error, panic) — the spawned thread
            // must never leave this unset.
            TEARDOWN_DONE.store(true, Ordering::SeqCst);
        });

    match spawned {
        // Deliberately NOT `.join()`-ed — see the doc comment above.
        Ok(_handle) => {
            let deadline = std::time::Instant::now() + TEARDOWN_WAIT_DEADLINE;
            while !TEARDOWN_DONE.load(Ordering::SeqCst) {
                if std::time::Instant::now() >= deadline {
                    eprintln!(
                        "FIXTURE_TEARDOWN deadline ({TEARDOWN_WAIT_DEADLINE:?}) exceeded waiting \
                         for teardown thread — abandoning wait and exiting; postmaster/temp-dir \
                         may leak (accepted A-12-class residual)"
                    );
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        Err(e) => {
            eprintln!("FIXTURE_TEARDOWN failed to spawn teardown thread: {e}");
        }
    }
}

/// Wall-clock bound on how long the atexit-calling thread waits for the
/// spawned teardown thread to finish (L2 hardening, #2049). Chosen
/// generously above the observed shutdown time (`postgres` stop plus
/// temp-dir/password-file/socket-dir cleanup — sub-second in practice), so a
/// healthy teardown never trips it, while still guaranteeing process exit if
/// the spawned thread never sets `TEARDOWN_DONE` for any reason.
const TEARDOWN_WAIT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// Extracts a human-readable message from a `catch_unwind` panic payload.
/// Panic payloads are conventionally `&'static str` (`panic!("literal")`) or
/// `String` (`panic!("{msg}")`); anything else falls back to a fixed string
/// rather than failing to report the panic at all.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}
