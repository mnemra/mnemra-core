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
//! # Ceremony root (set 2026-07-01)
//!
//! `ROOT` is the real root public key produced by the mnemra V0 signing
//! ceremony, set 2026-07-01. It is a valid, non-zero Ed25519 public key that
//! also satisfies the shape test in
//! `embedded_root_material_is_a_compile_time_constant`.
//!
//! The test suite injects per-run keypairs via the `root_material` parameter
//! of `verify_plugin()` and only checks `ROOT` for shape (32 bytes, non-zero).

/// The mnemra root verification key, embedded at build time.
///
/// This is the real 32-byte Ed25519 public key produced by the mnemra V0
/// signing ceremony — the public half of the root signing key, embedded as
/// of 2026-07-01.
///
/// MUST NOT be all-zero — an all-zero key has no corresponding private key and
/// would silently accept signatures in a broken way. The test pins this.
pub static ROOT: &[u8] = &[
    // Real ceremony root public key, embedded 2026-07-01 (R-0005-d, R-0018-f).
    0x73, 0xde, 0xb7, 0xec, 0x5f, 0xd7, 0xef, 0xff, 0xbe, 0xbe, 0x86, 0xd6, 0xdc, 0xdc, 0xe9, 0xd8,
    0x1b, 0x4b, 0x0c, 0x43, 0x15, 0x52, 0x67, 0x88, 0x7a, 0xb3, 0xff, 0x04, 0x2b, 0xb7, 0x73, 0xe5,
];

/// The root the build was AUTHORISED against — an INDEPENDENT declaration,
/// never `= ROOT` or derived from it.
///
/// The verify-build gate (`signing::build_gate::roots_match`) passes only when
/// `ROOT` byte-equals `ROOT_PIN`. Declaring the pin independently is what gives
/// the gate meaning: if the pin were a function of `ROOT`, the comparison would
/// be a tautology — a value against itself — and would detect nothing. Keep the
/// two declarations separate; do NOT rewrite this as `= ROOT`.
///
/// # Ceremony pin (set 2026-07-01) — now byte-equal to ROOT
///
/// As of the 2026-07-01 signing ceremony, this is independently declared as
/// the same 32-byte value as `ROOT`. It is byte-equal to `ROOT` only because
/// the ceremony output was transcribed into both declarations separately —
/// NOT because one is derived from the other. Keep the two declarations
/// separate; do NOT rewrite this as `= ROOT`.
///
/// MUST be 32 bytes to match `ROOT`'s length.
pub static ROOT_PIN: &[u8] = &[
    // Real ceremony root public key, pinned 2026-07-01 — independently
    // declared, byte-equal to ROOT as of the ceremony. Do NOT rewrite as
    // `= ROOT`; the independence is what gives the gate meaning.
    0x73, 0xde, 0xb7, 0xec, 0x5f, 0xd7, 0xef, 0xff, 0xbe, 0xbe, 0x86, 0xd6, 0xdc, 0xdc, 0xe9, 0xd8,
    0x1b, 0x4b, 0x0c, 0x43, 0x15, 0x52, 0x67, 0x88, 0x7a, 0xb3, 0xff, 0x04, 0x2b, 0xb7, 0x73, 0xe5,
];
