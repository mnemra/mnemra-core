//! mnemra-echo — V0 reference/fixture `core: true` plugin.
//!
//! # Role: typed `content` export (R-0019-a, P-0013)
//!
//! The plugin exports the fixed typed `content` interface (`create`/`get`/
//! `list`/`update`/`delete`) the host invokes per authenticated MCP verb. The
//! retired `run(input: string) -> string` string-dispatch export is gone
//! (R-0019-e). Each `content` method routes to a host `artifact` import — the
//! guest-driven invocation model: the host invokes `content.create`, the guest
//! body calls back into `artifact.artifact-create` to persist and obtain the id.
//!
//! # Slice 1 scope
//!
//! `create`/`get`/`list`/`update` are the fully-wired methods (the walking
//! skeleton: MCP verb -> host -> guest export -> host `artifact` import -> fenced
//! map -> typed return). `delete` remains a minimal typed-but-empty stub wired in
//! a later T12 slice.
//!
//! # WorkspaceCtx on the export boundary
//!
//! The typed `content` export takes NO `ctx` parameter — the host threads the
//! single authoritative `WorkspaceCtx` (constructed at the dispatch site,
//! R-0006-b) through the host's store data and reads it inside the `artifact`
//! host-fn import body. The guest cannot supply or forge a workspace id: the
//! `workspace-ctx` value it passes across the import boundary is a structural
//! placeholder the host IGNORES (R-0006-e: no internal bypass; the real
//! scoping key is host-derived, never plugin-supplied). The placeholder exists
//! only because the import WIT signature (locked by P-0003, not re-opened here)
//! still declares `ctx: workspace-ctx`.

// Generates the guest-side bindings for `world plugin` defined in
// `wit/echo.wit`. `export!` below registers our struct as the world's typed
// `content` exporter.
wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use exports::mnemra::host::content::Guest as ContentGuest;
use mnemra::host::artifact;
use mnemra::host::types::{ArtifactPage, WorkspaceCtx};

struct EchoPlugin;

/// The host-ignored placeholder `WorkspaceCtx` the guest passes across the
/// `artifact` import boundary. The import WIT signature requires a `ctx`
/// argument; the host's import body derives the real `workspace_id` from its
/// own store data and IGNORES this value (R-0006-b/e). The guest has no access
/// to the real workspace id and must not fabricate a meaningful one.
fn host_supplied_ctx() -> WorkspaceCtx {
    WorkspaceCtx {
        workspace_id: String::new(),
    }
}

impl ContentGuest for EchoPlugin {
    /// `content.create` — persist a new artifact via the host `artifact-create`
    /// import and return the host-generated id (R-0019-a, guest-driven model).
    fn create(type_name: String, frontmatter: String, body: Option<String>) -> String {
        artifact::artifact_create(
            &host_supplied_ctx(),
            &type_name,
            &frontmatter,
            body.as_deref(),
        )
    }

    /// `content.get` — read a single artifact by id via the host `artifact-get`
    /// import; `None` when not found / not visible in the caller's workspace.
    fn get(id: String) -> Option<String> {
        artifact::artifact_get(&host_supplied_ctx(), &id)
    }

    /// `content.list` — list ids of `type_name` artifacts via the host
    /// `artifact-list` import (R-0019-a, guest-driven model, R-0020). `filters`
    /// is passed through to the host; predicate application is deferred (brain
    /// #1846). `limit` and `cursor` are passed through for paging; T14 host body
    /// returns placeholder paging only (has-more=false, next-cursor=none). The
    /// host scopes the result to the caller's workspace (R-0006-d); the guest
    /// cannot supply or widen the scope.
    fn list(
        type_name: String,
        filters: String,
        limit: u32,
        cursor: Option<String>,
    ) -> ArtifactPage {
        artifact::artifact_list(
            &host_supplied_ctx(),
            &type_name,
            &filters,
            limit,
            cursor.as_deref(),
        )
    }

    /// `content.update` — merge the frontmatter patch + (optionally) replace the
    /// body via the host `artifact-update` import (R-0019-a, guest-driven model).
    /// `body=None` leaves the existing body unchanged. The host scopes the target
    /// to the caller's workspace (R-0006-d); a missing/cross-workspace target is a
    /// silent no-op. The guest cannot supply or widen the workspace scope.
    fn update(id: String, frontmatter_patch: String, body: Option<String>) {
        artifact::artifact_update(
            &host_supplied_ctx(),
            &id,
            &frontmatter_patch,
            body.as_deref(),
        )
    }

    /// `content.delete` — remove an artifact via the host `artifact-delete` import
    /// (R-0019-a, guest-driven model). The host scopes the target to the caller's
    /// workspace (R-0006-d); a missing/cross-workspace target is a silent no-op.
    /// The guest cannot supply or widen the workspace scope.
    fn delete(id: String) {
        artifact::artifact_delete(&host_supplied_ctx(), &id)
    }
}

export!(EchoPlugin);
