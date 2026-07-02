//! Host startup checks and pool population.
//!
//! Pre-flight invariants that must pass before any plugin is loaded or any
//! MCP connection is accepted. A failure here causes the host to refuse to
//! start entirely (fail-shut, R-0005-f).

pub mod file_mode_check;
// Task 11 / T11: verify-gated pool population — populate_verified_pool (R-0005-a, R-0016-a).
// T5 (R-0022-e): populate_verified_pool_from_dir — production disk-driven load.
pub mod pool_population;
pub use pool_population::{StartupError, populate_verified_pool, populate_verified_pool_from_dir};
