// Fixture (#3842, C1): `VerifiedGrants::new` is `pub(crate)`, so nothing outside
// the crate can mint grants from a list it compiled itself. MUST NOT compile.
// This is a type-level guarantee, not a test seam: it holds in every feature
// configuration.

use mnemra_host::plugin::allowlist::{HostFnAllowlist, VerifiedGrants};

fn main() {
    let _ = VerifiedGrants::new(HostFnAllowlist::default(), &[]);
}
