//! mnemra-echo — V0 reference/fixture `core: true` plugin.
//!
//! This plugin serves two roles:
//!
//! 1. **Round-trip and state-persistence fixture** — the `run()` export calls
//!    `echo.echo` and `echo.increment-counter` to demonstrate the host-fn
//!    round-trip and per-instance state persistence that the V0.01 spike
//!    exercises.
//!
//! 2. **Artifact ABI compile-bind surface** — the plugin imports the `artifact`
//!    interface declared in `wit/host.wit` and provides compile-bind call-sites
//!    in `artifact_ops()` that type-check against the full WIT contract (R-0003-a,
//!    R-0003-g). These are NOT on the executed `run()` path; live execution is
//!    Task 21 (host runtime + storage). See `manifest.toml`.
//!
//! # Why artifact_ops() is not called from run()
//!
//! The `artifact_*` host-fns are `todo!()` stubs in the host (Task 5/21 seam).
//! Calling them at runtime would panic. The V0 compile-bind invariant is: the
//! guest WIT world imports `artifact`, the bindings compile, and the call-sites
//! type-check. Live execution requires the host runtime (Task 21). This is the
//! explicit Task 19 scope: "bind artifact.* means author the WIT import +
//! call-sites that compile-bind; it does NOT mean route an executed test path
//! through a stubbed artifact.* host-fn."

// Generates the guest-side bindings for `world plugin` defined in
// `wit/echo.wit`. `export!` below registers our struct as the world's
// `run` exporter.
wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use mnemra::host::artifact;
use mnemra::host::types::WorkspaceCtx;

struct EchoPlugin;

impl Guest for EchoPlugin {
    fn run(input: String) -> String {
        // Round-trip: ask the host to echo the input.
        let echoed = mnemra::host::echo::echo(&input);
        // State: ask the host for its current counter (it auto-increments).
        let counter = mnemra::host::echo::increment_counter();
        format!("{echoed} | counter: {counter}")
        // Note: artifact_ops() is NOT called here — artifact.* stubs are
        // todo!() in the host at V0. Call-sites live in artifact_ops() below,
        // exercised once the host runtime + storage land in Task 21.
    }
}

/// Compile-bind call-sites for the `artifact` host-fn interface.
///
/// These calls type-check against the WIT ABI declared in `wit/host.wit` and
/// imported via `wit/echo.wit`'s `import artifact`. They are NOT called from
/// `run()` — live execution is Task 21 (host runtime + plugin pool).
///
/// R-0003-a, R-0003-g: the corresponding manifest.toml `[host_fns].required`
/// declares all five artifact.* functions, including the opt-in `artifact.delete`.
#[allow(dead_code)]
fn artifact_ops(ctx: &WorkspaceCtx, id: &str, frontmatter: &str, body: Option<&str>) {
    // artifact-create: creates a new artifact of type "echo_fixture", returns id.
    // R-0003-d: workspace context is passed via ctx (host-derived); workspace_id
    // is not a standalone parameter — WorkspaceCtx carries it.
    // wit_bindgen generates ctx params as &WorkspaceCtx (borrowed).
    let _created_id = artifact::artifact_create(ctx, "echo_fixture", frontmatter, body);

    // artifact-get: retrieve a single artifact by id.
    let _artifact = artifact::artifact_get(ctx, id);

    // artifact-list: list artifacts by type with optional filters.
    let _list = artifact::artifact_list(ctx, "echo_fixture", "{}");

    // artifact-update: patch frontmatter and/or body.
    artifact::artifact_update(ctx, id, frontmatter, body);

    // artifact-delete: destructive op; opted-in via manifest.toml host_fns.required
    // declaration (R-0003-g). A plugin that does not declare this cannot call it.
    artifact::artifact_delete(ctx, id);
}

export!(EchoPlugin);
