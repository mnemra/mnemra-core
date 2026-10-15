//! Labelled per-await bound for shared-pool awaits (#3987).
//!
//! Included via `#[path = "common/await_bound.rs"] mod await_bound;` only by
//! binaries that call it. It is a separate module from `shared_engine.rs`
//! because that file is included by ~20 binaries, and a `pub` helper only one
//! of them uses would be dead code in the rest.
//!
//! Why the bound exists: see "Hang backstops (#3987)" in `shared_engine.rs`.
//! A trip panics, an ordinary test failure, so libtest exits normally and the
//! `atexit` teardown reaps the postmaster.

use std::future::Future;
use std::time::Duration;

/// Bound for one labelled await. Above the pools' own `acquire_timeout`
/// (30 s superuser / 60 s app), so a plain acquire failure surfaces as sqlx's
/// own `PoolTimedOut` first and this bound fires only on an await sqlx does
/// not bound itself.
pub const AWAIT_BOUND: Duration = Duration::from_secs(120);

/// Await `fut` for at most [`AWAIT_BOUND`]; on expiry panic with a message
/// naming `label`, so a recurrence says which await hung.
pub async fn bounded<T>(label: &str, fut: impl Future<Output = T>) -> T {
    bounded_for(label, AWAIT_BOUND, fut).await
}

/// [`bounded`] with an explicit limit.
pub async fn bounded_for<T>(label: &str, limit: Duration, fut: impl Future<Output = T>) -> T {
    match tokio::time::timeout(limit, fut).await {
        Ok(v) => v,
        Err(_) => panic!(
            "FIXTURE_BOUND await '{label}' did not complete within {limit:?} (#3987: a pooled \
             connection registered on an earlier test's dropped runtime parks silently)"
        ),
    }
}
