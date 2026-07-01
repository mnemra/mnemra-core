//! Tests for the verify-build root-pin allowlist gate (`signing::build_gate`).
//!
//! The synthetic-input tests below use only local byte arrays — no real key —
//! and run in ci via `NONPG_TEST_FLAGS` (`--test build_gate`). They exercise the
//! pure `roots_match` core in both directions, including the independence
//! (tautological-pin) property that makes the gate meaningful.
//!
//! The LIVE gate over the build-embedded `ROOT` / `ROOT_PIN` is
//! `root_pin_gate_matches_embedded`, below. The signing ceremony is complete
//! (`ROOT == ROOT_PIN`), so it is no longer `#[ignore]`d — it runs as part of
//! the normal suite via `NONPG_TEST_FLAGS` (`--test build_gate`), which
//! `verify-test` runs. The `verify-signing-root` recipe also runs it
//! explicitly and maps the exit status to `GATE signing-root PASS|FAIL`.

use mnemra_host::signing::build_gate::{embedded_root_is_pinned, roots_match};

/// A fixed 32-byte "pin" reused across the synthetic tests.
const PIN: [u8; 32] = [
    0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99,
    0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xa0, 0xb0, 0xc0, 0xd0, 0xe0, 0xf0, 0x01,
];

/// A distinct 32-byte "placeholder root" — differs from `PIN` in every byte
/// (stands in for the round-1a `ROOT` placeholder).
const PLACEHOLDER_ROOT: [u8; 32] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
    0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20,
];

/// A distinct 32-byte "dev key" — a third value, unequal to both `PIN` and
/// `PLACEHOLDER_ROOT` (stands in for an unauthorised developer key).
const DEV_KEY: [u8; 32] = [
    0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef,
    0xca, 0xfe, 0xba, 0xbe, 0xca, 0xfe, 0xba, 0xbe, 0xca, 0xfe, 0xba, 0xbe, 0xca, 0xfe, 0xba, 0xbe,
];

#[test]
fn roots_match_true_when_embedded_equals_pin() {
    // Positive branch — proven with a synthetic key, no real ceremony key needed.
    assert!(roots_match(&PIN, &PIN));
}

#[test]
fn roots_match_false_for_placeholder_root() {
    // The round-1a state: a placeholder embedded root against the pin → deny.
    assert!(!roots_match(&PLACEHOLDER_ROOT, &PIN));
}

#[test]
fn roots_match_false_for_dev_key() {
    // A developer key that is not the pinned root → deny (positive-allowlist:
    // anything that is not the pin is rejected).
    assert!(!roots_match(&DEV_KEY, &PIN));
}

#[test]
fn roots_match_false_when_embedded_mutated_one_byte() {
    // Independence property (tautological-pin guard): hold the pin fixed, mutate
    // a single byte of the embedded side → must be rejected. If the gate
    // compared a value to itself, this flip would wrongly pass.
    let mut embedded = PIN;
    embedded[0] ^= 0x01;
    assert!(!roots_match(&embedded, &PIN));
}

#[test]
fn roots_match_false_when_pin_mutated_one_byte() {
    // Symmetric independence property: hold the embedded root fixed, mutate a
    // single byte of the pin → must be rejected.
    let mut pin = PIN;
    pin[31] ^= 0x01;
    assert!(!roots_match(&PIN, &pin));
}

#[test]
fn roots_match_false_on_length_mismatch() {
    // Length guard in the const fn: a short slice can never match a 32-byte pin.
    assert!(!roots_match(&PIN[..31], &PIN));
}

/// LIVE gate: the build-embedded `ROOT` MUST byte-equal `ROOT_PIN`.
///
/// The signing ceremony is complete: `ROOT` and `ROOT_PIN` are now set to the
/// real ceremony root public key (byte-equal), so this is a live enforced
/// gate, no longer skipped-by-design. It runs in the normal suite — part of
/// `NONPG_TEST_FLAGS --test build_gate`, which `verify-test` runs — and is
/// also run explicitly by the `verify-signing-root` recipe, which maps the
/// exit status to `GATE signing-root PASS|FAIL`.
#[test]
fn root_pin_gate_matches_embedded() {
    assert!(
        embedded_root_is_pinned(),
        "embedded root (ROOT) does not match the pin (ROOT_PIN) — signing-root \
         gate FAIL (expected in round-1a with distinct placeholder keys)"
    );
}
