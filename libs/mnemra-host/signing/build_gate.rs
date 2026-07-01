//! Verify-build root-pin allowlist gate (positive allowlist, SF2).
//!
//! # Decision hidden
//!
//! WHETHER the root verification key baked into this binary
//! (`root_material::ROOT`) is the exact key the build was authorised against
//! (`root_material::ROOT_PIN`). The gate is a POSITIVE allowlist: it passes only
//! when the embedded root byte-equals the independently-declared pin, and fails
//! for every other root — the round-1a placeholder, a dev key, a tampered key.
//! There is no "block-list" of bad roots; anything that is not the pin is denied
//! by default (complete mediation).
//!
//! # Why the pin is an INDEPENDENT declaration
//!
//! `ROOT_PIN` is a separate constant, never `= ROOT` or derived from it. If the
//! pin were a function of the embedded root, the gate would compare a value to
//! itself and pass tautologically — zero protection. That independence is the
//! load-bearing property; `tests/build_gate.rs` proves it with a mutate-one-byte
//! adversarial pair (mutate the embedded side → deny; mutate the pin side →
//! deny). See `root_material::ROOT_PIN` for the declaration-side invariant.
//!
//! # Round-1a state (placeholder — gate FAILS by design)
//!
//! `ROOT` and `ROOT_PIN` are DISTINCT placeholders in round-1a, so the gate
//! currently reports FAIL — correct, because no real key has been pinned yet.
//! The gate therefore lives in a standalone `verify-signing-root` justfile
//! recipe kept OUT of the `ci` chain, so the branch stays green. The signing
//! ceremony round sets BOTH constants to the real 32-byte root public key
//! (making them byte-equal), un-ignores the live test, and wires the gate into
//! `just ci`.
//!
//! # Mechanism: runtime check now, compile-time assertion later
//!
//! The strongest form is a compile-time assertion:
//!
//! ```ignore
//! const _: () = assert!(roots_match(ROOT, ROOT_PIN));
//! ```
//!
//! but with the distinct round-1a placeholders it would fail to COMPILE, red-ing
//! every build on the branch. It is therefore DEFERRED to the ceremony round
//! (see the commented block at the bottom of this file). The round-1a gate is a
//! runtime check: an `#[ignore]`d test asserts `roots_match(ROOT, ROOT_PIN)` and
//! the `verify-signing-root` recipe runs it explicitly, mapping the exit status
//! to `GATE signing-root PASS|FAIL`.

use super::root_material::{ROOT, ROOT_PIN};

/// Byte-exact equality of two verification-key slices.
///
/// Returns `true` iff `embedded` and `pinned` have the same length and every
/// byte is equal. This is the unit-testable core of the gate.
///
/// Constant-time comparison is deliberately NOT used — these are public keys,
/// not secrets, so an early-return length/byte check leaks nothing of value.
///
/// `const fn` so the ceremony round can promote the gate to a compile-time
/// `const _: () = assert!(roots_match(ROOT, ROOT_PIN));`. Slice `==`
/// (`<[u8]>::eq`) is not `const`-stable, so the comparison is a manual length
/// check plus a `while`-loop byte compare rather than `embedded == pinned`.
pub const fn roots_match(embedded: &[u8], pinned: &[u8]) -> bool {
    if embedded.len() != pinned.len() {
        return false;
    }
    let mut i = 0;
    while i < embedded.len() {
        if embedded[i] != pinned[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The verify-build gate result: `true` iff the build-embedded root
/// (`root_material::ROOT`) byte-matches the pin (`root_material::ROOT_PIN`).
///
/// This is the semantic gate entry point the `verify-signing-root` recipe
/// exercises (via the `#[ignore]`d live test in `tests/build_gate.rs`). It reads
/// the two build-embedded constants and defers the comparison to [`roots_match`]
/// — so the ceremony round only has to make the two constants equal.
pub fn embedded_root_is_pinned() -> bool {
    roots_match(ROOT, ROOT_PIN)
}

// Ceremony-round promotion target — UNCOMMENT once `ROOT` and `ROOT_PIN` are
// BOTH set to the real root public key (making them byte-equal). With the
// distinct round-1a placeholders this assertion fails to COMPILE, so it stays
// commented until the ceremony round; the runtime `verify-signing-root` recipe
// enforces the gate meanwhile.
//
// const _: () = assert!(roots_match(ROOT, ROOT_PIN));
