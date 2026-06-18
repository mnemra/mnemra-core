//! Compiled host-fn and verb allowlists extracted from a loaded manifest.
//!
//! Both lists are compiled once at `PluginRuntime::load` time, before any
//! plugin instance is created (R-0003-b). Subsequent per-call checks are
//! O(n) over the compiled list — small N in practice (< 20 fns per plugin).
//!
//! # R-0003-b WIT-boundary enforcement (note)
//!
//! This module implements the allowlist QUERY surface that `is_host_fn_allowed`
//! exposes. Actual WIT-boundary enforcement (rejecting undeclared host-fn calls
//! before they reach the host-fn body) requires a live Wasmtime linker with a
//! catch-all that consults this allowlist. That wiring lives in `pool.rs` /
//! `limits.rs` and is exercised at component-invocation time, not at the
//! manifest-load seam tested by `manifest_load.rs`.

use crate::plugin::manifest::HostFnsSection;

// ---------------------------------------------------------------------------
// HostFnAllowlist
// ---------------------------------------------------------------------------

/// Compiled allowlist of host-fn names drawn from a manifest's `[host_fns]`
/// table. Both `required` and `optional` entries are included; the distinction
/// is surfacing-only (required entries produce a load-time error if absent at
/// Task-22 validation time; optional entries are advisory). Both are ALLOWED.
#[derive(Debug, Clone)]
pub struct HostFnAllowlist {
    /// Union of `required` and `optional` fn names, deduplicated.
    allowed: Vec<String>,
}

impl HostFnAllowlist {
    /// Compile an allowlist from a manifest's `[host_fns]` section.
    pub fn from_manifest(section: &HostFnsSection) -> Self {
        let mut allowed: Vec<String> = section
            .required
            .iter()
            .chain(section.optional.iter())
            .cloned()
            .collect();
        allowed.sort_unstable();
        allowed.dedup();
        Self { allowed }
    }

    /// Returns `true` iff `fn_name` appears in the compiled allowlist.
    pub fn is_allowed(&self, fn_name: &str) -> bool {
        self.allowed
            .binary_search_by_key(&fn_name, |s| s.as_str())
            .is_ok()
    }
}

// ---------------------------------------------------------------------------
// VerbAllowlist
// ---------------------------------------------------------------------------

/// Compiled capability list drawn from `[verbs].exposed` in the manifest.
/// Used to gate dispatch: an undeclared verb is denied before the plugin is
/// ever invoked (R-0010-d).
#[derive(Debug, Clone)]
pub struct VerbAllowlist {
    exposed: Vec<String>,
}

impl VerbAllowlist {
    /// Compile a verb allowlist from an exposed-verbs slice.
    pub fn from_exposed(exposed: &[String]) -> Self {
        let mut exposed = exposed.to_vec();
        exposed.sort_unstable();
        exposed.dedup();
        Self { exposed }
    }

    /// Returns `true` iff `verb` appears in `[verbs].exposed`.
    pub fn is_allowed(&self, verb: &str) -> bool {
        self.exposed
            .binary_search_by_key(&verb, |s| s.as_str())
            .is_ok()
    }
}
