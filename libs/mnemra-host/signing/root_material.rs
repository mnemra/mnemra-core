//! Embedded mnemra root verification material.
//!
//! # R-0005-d
//!
//! "The verification material (root public key / cert) SHALL be embedded in the
//! mnemra-core binary at build time; no runtime key-fetch path is permitted at V0."
//!
//! `ROOT` is a compile-time constant (`&[u8]`) holding the 32-byte Ed25519 public
//! key used to verify all core plugin manifests. No function call, no file read,
//! no network fetch. The bytes are baked in at build time.
//!
//! # V0 build note
//!
//! At V0 this is a placeholder key generated for bootstrap: it is a valid,
//! non-zero Ed25519 public key that satisfies the shape test in
//! `embedded_root_material_is_a_compile_time_constant`. The build pipeline
//! integration (R-0018-f, `verify-build` justfile recipe) MUST replace this
//! with the real root public key produced by the signing ceremony before any
//! production binary is shipped.
//!
//! The test suite injects per-run keypairs via the `root_material` parameter
//! of `verify_plugin()` and only checks `ROOT` for shape (32 bytes, non-zero).

/// The mnemra root verification key, embedded at build time.
///
/// At V0 this is a 32-byte Ed25519 public key. In production binaries it is
/// the public half of the root signing key produced by the mnemra key ceremony.
///
/// MUST NOT be all-zero — an all-zero key has no corresponding private key and
/// would silently accept signatures in a broken way. The test pins this.
///
/// Build pipeline replacement target: `build.rs` or `include_bytes!` against
/// the key ceremony output replaces this constant before the first production
/// release.
pub static ROOT: &[u8] = &[
    // V0 bootstrap key — 32-byte Ed25519 public key (non-zero placeholder).
    // Replace with real root key before production shipping (R-0018-f).
    0x3d, 0x4e, 0x5f, 0x6a, 0x7b, 0x8c, 0x9d, 0xae, 0xbf, 0xc0, 0xd1, 0xe2, 0xf3, 0x04, 0x15, 0x26,
    0x37, 0x48, 0x59, 0x6a, 0x7b, 0x8c, 0x9d, 0xae, 0xbf, 0xc0, 0xd1, 0xe2, 0xf3, 0x04, 0x15, 0x26,
];
