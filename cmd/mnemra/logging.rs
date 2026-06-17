//! Structured logging initialisation for the `mnemra` binary.
//!
//! The binary owns the global `tracing` subscriber — libraries use only
//! `tracing` macros and have no knowledge of the subscriber implementation.
//! This follows the facade-from-binary pattern: `libs/mnemra-host` carries
//! zero subscriber setup; `cmd/mnemra` installs it once, as the first action
//! in `main`.

use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};

/// Initialise the global structured-log subscriber.
///
/// Installs a `tracing-subscriber` registry with:
/// - A JSON formatting layer writing to **stdout**.
/// - An `EnvFilter` that reads `MNEMRA_LOG` first, then `RUST_LOG`, then
///   falls back to `info`.
///
/// Uses `try_init()` so repeated calls (e.g. in tests that share a process)
/// are safe — a second call returns `Err` and is intentionally ignored.
///
/// # V0 logging foundation
///
/// // V0 logging foundation. OTel export + redaction + emission semantics land in Task 25 (R-0004).
pub fn init_logging() {
    let filter = EnvFilter::try_from_env("MNEMRA_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = Registry::default()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().json())
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `init_logging` is callable without panic and is safe to call
    /// repeatedly (idempotent for the no-panic contract).
    ///
    /// The global subscriber may already be set by a prior test in the same
    /// process — `try_init()` handles that by returning Err, which we ignore.
    /// This test passes as long as neither call panics.
    #[test]
    fn init_logging_is_callable_without_panic() {
        init_logging();
        init_logging(); // second call must not panic
    }
}
