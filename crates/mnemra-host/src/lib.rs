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
pub mod projection;
pub mod schema;
pub mod storage;

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
