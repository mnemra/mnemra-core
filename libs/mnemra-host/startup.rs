//! Host startup checks.
//!
//! Pre-flight invariants that must pass before any plugin is loaded or any
//! MCP connection is accepted. A failure here causes the host to refuse to
//! start entirely (fail-shut, R-0005-f).

pub mod file_mode_check;
