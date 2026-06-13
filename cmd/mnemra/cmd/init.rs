//! `mnemra init` subcommand.
//!
//! First-run bootstrap: starts the embedded Postgres engine, enables pgvector,
//! creates substrate tables + indexes, creates the `default` workspace, creates
//! the four least-privilege DB roles, and asserts health returns `overall: ok`.
//!
//! # Arg handling
//!
//! Hand-rolled single-subcommand match (no clap dependency). The `init` verb has
//! no flags at V0 — `clap` would add ~500ms compile cost for zero user-visible
//! benefit. If flags are needed at V0.1+ this module is the natural extension
//! point; the consequence is a one-file change, not a structural refactor.

use mnemra_host::schema::init::{HealthStatus, health_snapshot, init};
use mnemra_host::storage::postgres::engine::EmbeddedEngine;
use std::error::Error;

/// Run `mnemra init`.
///
/// Starts a fresh embedded engine, delegates to `mnemra_host::schema::init::init`,
/// asserts the health snapshot, and prints a success summary to stdout.
pub async fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    eprintln!("mnemra init: starting embedded Postgres engine...");

    let engine = EmbeddedEngine::start().await?;

    eprintln!("mnemra init: bootstrapping schema...");

    init(&engine, "vector").await?;

    // Assert health returns ok — if init succeeded, this must be true.
    let snapshot = health_snapshot(engine.pool.as_ref())
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

    if snapshot.overall != HealthStatus::Ok {
        return Err(format!(
            "mnemra init: unexpected health status after init: {}",
            snapshot.overall
        )
        .into());
    }

    println!(
        "mnemra init: complete. postgres={} pgvector={} workspace_default={} overall={}",
        snapshot.postgres, snapshot.pgvector, snapshot.workspace_default, snapshot.overall
    );

    Ok(())
}
