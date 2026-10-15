//! Compiled host-fn and verb allowlists extracted from a loaded manifest.
//!
//! Both lists are compiled once at `PluginRuntime::load` time, before any
//! plugin instance is created (R-0003-b). Subsequent per-call checks are
//! O(n) over the compiled list — small N in practice (< 20 fns per plugin).
//!
//! # R-0003-b enforcement
//!
//! This module is the allowlist QUERY surface. Enforcement happens at link time
//! in `component::build_linker`, which refuses a component importing any host
//! function this list does not name. The production writer of the list is
//! `PluginRuntime::load` (signed manifest slice only), which wraps it in
//! [`VerifiedGrants`], the only type `PluginPool::register_module` and
//! `component::build_linker` accept. Its constructor is `pub(crate)`, so a list
//! built from unsigned bytes cannot reach the linker (trybuild-pinned).

use crate::plugin::manifest::HostFnsSection;

// ---------------------------------------------------------------------------
// HostFnAllowlist
// ---------------------------------------------------------------------------

/// Compiled allowlist of host-fn names drawn from a manifest's `[host_fns]`
/// table. Both `required` and `optional` entries are included; the distinction
/// is surfacing-only (required entries produce a load-time error if absent at
/// Task-22 validation time; optional entries are advisory). Both are ALLOWED.
///
/// `Default` is the EMPTY list, which denies every host function (fail closed).
#[derive(Debug, Clone, Default)]
pub struct HostFnAllowlist {
    /// Union of `required` and `optional` fn names, deduplicated.
    allowed: Vec<String>,
}

impl HostFnAllowlist {
    /// Compile an allowlist from a manifest's `[host_fns]` section.
    ///
    /// `pub(crate)`: the only path from manifest bytes to something the linker
    /// accepts is `PluginRuntime::load` -> [`VerifiedGrants`].
    pub(crate) fn from_manifest(section: &HostFnsSection) -> Self {
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

// ---------------------------------------------------------------------------
// VerifiedGrants
// ---------------------------------------------------------------------------

/// What a plugin's SIGNED manifest grants: the host functions it may import and
/// the verbs it exposes.
///
/// The one-writer-of-record guarantee, by type: [`VerifiedGrants::new`] is
/// `pub(crate)` and is called only from `PluginRuntime::load`, after signature
/// verification and over the signed slice. `PluginPool::register_module` and
/// `component::build_linker` take this type and nothing weaker, so no caller
/// outside the crate can hand them a list compiled from unsigned bytes.
/// [`VerifiedGrants::deny_all`] is the only other source, and it grants nothing.
///
/// "Verified" means signed against the trust anchor the caller of
/// `PluginRuntime::load` passed, not against the embedded root as such: `load`
/// takes the anchor as a parameter. Only the production startup path
/// (`mnemra_host.rs`, `populate_verified_pool_from_dir(.., ROOT)`) passes the
/// embedded `ROOT`, and nothing pins that.
#[derive(Debug, Clone)]
pub struct VerifiedGrants {
    host_fns: HostFnAllowlist,
    verbs: VerbAllowlist,
    /// `[verbs].exposed` in manifest order, for `list_tools`.
    verb_order: Vec<String>,
}

impl VerifiedGrants {
    pub(crate) fn new(host_fns: HostFnAllowlist, exposed: &[String]) -> Self {
        Self {
            host_fns,
            verbs: VerbAllowlist::from_exposed(exposed),
            verb_order: exposed.to_vec(),
        }
    }

    /// Grants nothing: no host function, no verb. For fixtures that carry no
    /// manifest (component-WAT traps), where denying everything is the point.
    pub fn deny_all() -> Self {
        Self::new(HostFnAllowlist::default(), &[])
    }

    /// `true` iff the signed manifest declares `fn_name` (R-0003-b).
    pub fn is_host_fn_allowed(&self, fn_name: &str) -> bool {
        self.host_fns.is_allowed(fn_name)
    }

    /// `true` iff the signed manifest's `[verbs].exposed` names `verb` (R-0010-d).
    /// The MCP verb gate calls this through `PluginPool::is_verb_exposed`.
    pub fn is_verb_allowed(&self, verb: &str) -> bool {
        self.verbs.is_allowed(verb)
    }

    /// The exposed verbs, in manifest order.
    pub fn verbs(&self) -> &[String] {
        &self.verb_order
    }
}
