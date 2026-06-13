//! Schema management: migrations, initialization, and health snapshot.
//!
//! # Public surface
//!
//! - [`init::init`] — first-run bootstrap (`mnemra init`).
//! - [`init::health_snapshot`] — R-0004-g health probe (Task 25 wraps in HTTP).
//! - [`init::StorageError`] — A-15 degradation seam.
//! - [`init::HealthSnapshot`] + [`init::HealthStatus`] — health body shape.

pub mod init;
pub mod migrations;
