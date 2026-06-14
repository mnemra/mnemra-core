//! mnemra-host — runtime host library.
//!
//! Exposes the startup entry point that the `mnemra` binary calls.
//! This is a skeleton; subsequent tasks extend it:
//!   - Tasks 5–7:  storage initialisation
//!   - Task 23:    MCP server startup
//!   - Tasks 19/20: plugin-runtime integration

pub mod abi;
pub mod auth;
pub mod builtins;
// Task 18: per-deployment config (LLM-key, R-0014-a/b/c).
pub mod config;
// Task 18: outbound hostname allowlist for embedding-call pathway (R-0014-b).
pub mod net;
pub mod projection;
pub mod schema;
// Task 17: plugin signing-chain verification + embedded root (R-0005).
pub mod signing;
pub mod storage;
// Task 17: host startup invariant checks — file-mode gate before plugin load (R-0005-f).
// Task 18: extended to cover LLM-key file as well (R-0014-d).
pub mod startup;

/// Start the mnemra host runtime.
///
/// Returns when the runtime has initialised successfully. Later tasks will
/// extend this to accept configuration, wire storage, and launch the MCP
/// server; for now it is a compile-clean anchor.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_returns_ok() {
        assert!(run().is_ok());
    }
}
