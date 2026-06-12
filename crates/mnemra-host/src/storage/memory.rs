//! In-memory `Storage` adapter.
//!
//! Intended for testing and the host-layer seam only. Not a production engine.
//!
//! # Atomicity
//!
//! Writes staged through a `MemTransaction` are held in a local buffer and are
//! **not** applied to the shared store until `commit` is called. Dropping the
//! transaction without committing discards all staged writes — rollback is
//! structural, not a method call.
//!
//! # Workspace isolation
//!
//! Each workspace gets its own `HashMap<String, Record>` inside the shared
//! store. A transaction opened for workspace A holds a reference only to A's
//! sub-map and cannot address workspace B's entries.

use super::{Record, Storage, Transaction, WorkspaceId};
use async_trait::async_trait;
use std::{
    collections::HashMap,
    error::Error,
    sync::{Arc, Mutex},
};

// ---------------------------------------------------------------------------
// Shared store
// ---------------------------------------------------------------------------

/// Committed state shared across all transactions.
///
/// `Arc<Mutex<_>>` gives `Clone + Send + Sync` so the `MemStorage` value can
/// be shared between tests (or cloned for test setup) without ceremony.
type Store = Arc<Mutex<HashMap<WorkspaceId, HashMap<String, Record>>>>;

// ---------------------------------------------------------------------------
// MemStorage
// ---------------------------------------------------------------------------

/// In-memory storage backend.
#[derive(Clone, Default)]
pub struct MemStorage {
    store: Store,
}

impl MemStorage {
    /// Create a new, empty in-memory store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Storage for MemStorage {
    async fn begin(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Box<dyn Transaction>, Box<dyn Error + Send + Sync>> {
        Ok(Box::new(MemTransaction {
            workspace,
            store: Arc::clone(&self.store),
            staged: HashMap::new(),
        }))
    }
}

// ---------------------------------------------------------------------------
// MemTransaction
// ---------------------------------------------------------------------------

/// An in-flight transaction against the in-memory store.
///
/// `staged` buffers writes for this transaction. On `commit`, the buffer is
/// merged into `store` under `workspace`. Dropping without committing silently
/// discards `staged` — that is the rollback mechanism.
pub struct MemTransaction {
    workspace: WorkspaceId,
    store: Store,
    /// Writes buffered in this transaction, not yet applied to `store`.
    staged: HashMap<String, Record>,
}

#[async_trait]
impl Transaction for MemTransaction {
    async fn put(&mut self, record: Record) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.staged.insert(record.key.clone(), record);
        Ok(())
    }

    async fn get(&mut self, key: &str) -> Result<Option<Record>, Box<dyn Error + Send + Sync>> {
        // Read-your-own-writes: staged takes priority over committed.
        if let Some(r) = self.staged.get(key) {
            return Ok(Some(r.clone()));
        }
        let store = self.store.lock().expect("store lock poisoned");
        Ok(store
            .get(&self.workspace)
            .and_then(|ws| ws.get(key))
            .cloned())
    }

    async fn list(&mut self) -> Result<Vec<Record>, Box<dyn Error + Send + Sync>> {
        let store = self.store.lock().expect("store lock poisoned");
        let committed: HashMap<&str, &Record> = store
            .get(&self.workspace)
            .map(|ws| ws.iter().map(|(k, v)| (k.as_str(), v)).collect())
            .unwrap_or_default();

        // Merge: staged overwrites committed for the same key.
        let mut merged: HashMap<&str, &Record> = committed;
        for (k, v) in &self.staged {
            merged.insert(k.as_str(), v);
        }

        Ok(merged.into_values().cloned().collect())
    }

    async fn commit(self: Box<Self>) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut store = self.store.lock().expect("store lock poisoned");
        let ws = store.entry(self.workspace).or_default();
        for (key, record) in self.staged {
            ws.insert(key, record);
        }
        Ok(())
    }
}
