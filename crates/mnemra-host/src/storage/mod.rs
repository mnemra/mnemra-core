//! Engine-agnostic storage abstraction.
//!
//! The `Storage` trait exposes a **unit-of-work / transaction boundary** — not
//! per-write autocommit — so that any conforming adapter can express atomic
//! multi-write operations (P-0010 D5).
//!
//! Two co-equal invariants every adapter must satisfy:
//!
//! 1. **Atomic multi-write:** a `Transaction` commits all writes as a unit or
//!    rolls them back entirely. No partial visibility.
//!
//! 2. **Workspace-scoped isolation:** operations bound to one `WorkspaceId`
//!    cannot read or mutate rows belonging to another workspace.
//!
//! # Adapter extension point (Task 6)
//!
//! Task 6 adds the Postgres adapter by implementing `Storage` for its engine
//! type and registering the constructor with the contract suite (see
//! `tests/storage_contract.rs`). The adapter satisfies the same trait; no
//! other plumbing changes in this module.

pub mod memory;
pub mod postgres;

use async_trait::async_trait;
use std::error::Error;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// WorkspaceId newtype
// ---------------------------------------------------------------------------

/// Opaque identifier scoping all storage operations to a single tenant.
///
/// UUID-backed newtype (A-16, Task 7). The invariant — that all ops within a
/// transaction are implicitly scoped to this id — is locked by this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceId(pub Uuid);

// ---------------------------------------------------------------------------
// Opaque keyed record
// ---------------------------------------------------------------------------

/// Minimal record used to exercise the two storage invariants.
///
/// Real artifact columns (`embedding`, `search_tsv`, etc.) land in Tasks 7–9.
/// This shape is enough to exercise atomic multi-write and workspace isolation
/// without pre-designing artifact-type tables or V0.1+ keyed-supersession
/// columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Application-assigned key, unique within a workspace.
    pub key: String,
    /// Opaque payload bytes.
    pub value: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Transaction trait
// ---------------------------------------------------------------------------

/// A scoped, in-flight unit of work.
///
/// All read and write operations happen through a `Transaction` bound to a
/// specific `WorkspaceId`. The transaction either commits (making all writes
/// visible atomically) or is abandoned (no writes are applied).
///
/// # Object-safety
///
/// `async-trait` rewrites `async fn` to return `Box<dyn Future>`, preserving
/// object-safety so the host can hold `Box<dyn Transaction>` without knowing
/// the concrete adapter type at compile time.
#[async_trait]
pub trait Transaction: Send {
    /// Insert or replace a record by key within this transaction's workspace.
    ///
    /// The write is **not** visible to readers until `commit` succeeds.
    async fn put(&mut self, record: Record) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Retrieve a record by key within this transaction's workspace.
    ///
    /// Returns `None` if the key does not exist *and is committed*. Writes
    /// staged in this transaction but not yet committed are visible to reads
    /// within the same transaction (read-your-own-writes).
    async fn get(&mut self, key: &str) -> Result<Option<Record>, Box<dyn Error + Send + Sync>>;

    /// List all committed records in this transaction's workspace.
    ///
    /// Uncommitted (staged) writes from *this* transaction are also included,
    /// allowing callers to observe their own in-flight state.
    async fn list(&mut self) -> Result<Vec<Record>, Box<dyn Error + Send + Sync>>;

    /// Commit all staged writes atomically.
    ///
    /// After a successful commit, every write made through this transaction
    /// becomes visible to subsequent transactions.
    async fn commit(self: Box<Self>) -> Result<(), Box<dyn Error + Send + Sync>>;
}

// ---------------------------------------------------------------------------
// Storage trait
// ---------------------------------------------------------------------------

/// Engine-agnostic storage backend.
///
/// The sole entry point is `begin`, which opens a transaction scoped to a
/// workspace. Per-write autocommit is not expressible through this API —
/// callers must explicitly commit (P-0010 D5).
#[async_trait]
pub trait Storage: Send + Sync {
    /// Open a new transaction scoped to `workspace`.
    ///
    /// All reads and writes through the returned transaction are confined to
    /// `workspace`; cross-workspace visibility is structurally prevented.
    async fn begin(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Box<dyn Transaction>, Box<dyn Error + Send + Sync>>;
}
