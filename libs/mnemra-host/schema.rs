//! Schema management: migrations, initialization, health snapshot, and
//! per-artifact-type table generator.
//!
//! # Public surface
//!
//! - [`init::init`] — first-run bootstrap (`mnemra init`), wires artifact-type
//!   table generator for fixture types.
//! - [`init::health_snapshot`] — R-0004-g health probe (Task 25 wraps in HTTP).
//! - [`init::StorageError`] — A-15 degradation seam.
//! - [`init::HealthSnapshot`] + [`init::HealthStatus`] — health body shape.
//! - [`artifact_table::create_artifact_table`] — per-artifact-type DDL generator
//!   (Tasks 8/9; Task 19 calls directly for plugin-registered types).
//! - [`artifact_table::validate_type_name`] — injection-prevention boundary.
//! - [`history_trigger::create_history_machinery`] — history shadow table +
//!   BEFORE UPDATE / BEFORE DELETE triggers (R-0001-e).

pub mod artifact_table;
pub mod history_trigger;
pub mod init;
pub mod migrations;
