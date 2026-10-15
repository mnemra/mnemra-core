// Fixture (#3842, C1): `PluginPool::register_module` takes `&VerifiedGrants`, so a
// bare `HostFnAllowlist` (the type an unsigned parse yields) is a type error.
// MUST NOT compile.

use mnemra_host::plugin::allowlist::HostFnAllowlist;
use mnemra_host::plugin::pool::PluginPool;
use wasmtime::component::Component;

fn register(pool: &PluginPool, component: &Component) {
    let _ = pool.register_module("x", "0", component, &HostFnAllowlist::default());
}

fn main() {}
