//! Plugin output validation — fail-shut schema and size-cap checks (R-0003-f).
//!
//! # Check ordering (pinned by RED tests)
//!
//! 1. **Size-cap check** — runs FIRST. Oversized output → `OutputError::FieldSizeCap`.
//!    The RED test `validate_output_fails_shut_on_oversized_field` asserts that 1 MiB+1
//!    bytes return `FieldSizeCap`, NOT `SchemaMismatch`.
//! 2. **Schema / WIT decode check** — runs AFTER the size-cap gate. Invalid WIT bytes
//!    → `OutputError::SchemaMismatch`.
//!
//! The RED test `within_cap_output_does_not_trip_size_cap` asserts that 4 zero bytes
//! do NOT return `FieldSizeCap` (size-cap lower boundary). Those bytes may still
//! produce `SchemaMismatch` — that is correct behaviour.
//!
//! # Verb → output-type binding (R-0003-f)
//!
//! Plugin manifest verbs use a dot-namespaced naming convention (`echo.create`,
//! `echo.get`, ...) that is a PLUGIN-LEVEL dispatch namespace, not a WIT function
//! name. The WIT for `echo` exposes `echo: func(s: string) -> string` and
//! `increment-counter: func() -> u32`. The output type for ANY `echo.*` verb is
//! `string` (the WIT component's `run: func(input: string) -> string` return).
//!
//! Binding table (V0):
//!   - any verb in the `echo.*` namespace   → WIT string (WASM-component-model
//!     `string` lowering: 8-byte ptr+len pair, UTF-8 body in linear memory).
//!     At V0 we validate via a lightweight structural check: the bytes must be
//!     valid UTF-8 and within the per-field cap.
//!   - verbs in other namespaces             → unknown at V0; returns `SchemaMismatch`
//!     with `detail: "unbound verb"`. This is deliberately fail-shut: an unbound
//!     verb whose output is never validated would be a hole in R-0003-f. If a
//!     future plugin needs a different output type, add a binding entry here and
//!     extend the verb→type table.
//!
//! # Per-field size cap
//!
//! V0 cap: 1 MiB (1_048_576 bytes) per output field. Chosen to match the RED
//! test fixture (1 MiB + 1 byte is oversized; 4 bytes is within cap).

use crate::plugin::runtime::OutputError;

/// Per-output-field size cap in bytes (V0: 1 MiB).
pub const FIELD_SIZE_CAP: usize = 1_048_576;

/// Output field name used in `FieldSizeCap` errors.
const OUTPUT_FIELD: &str = "output";

/// Validate plugin output bytes against the WIT-declared schema for `verb`.
///
/// # Ordering (test-pinned)
///
/// 1. Size-cap gate (first).
/// 2. Schema / WIT decode gate (second).
///
/// Returns `Ok(())` on a valid, within-cap, correctly-typed output.
pub fn validate_output(verb: &str, output_bytes: &[u8]) -> Result<(), OutputError> {
    // Step 1: Per-field size cap — MUST run before schema check.
    if output_bytes.len() > FIELD_SIZE_CAP {
        return Err(OutputError::FieldSizeCap {
            field: OUTPUT_FIELD.to_owned(),
            max_bytes: FIELD_SIZE_CAP,
            actual_bytes: output_bytes.len(),
        });
    }

    // Step 2: Schema / WIT type check based on verb namespace.
    validate_schema(verb, output_bytes)
}

/// Route `verb` to its WIT output type and validate the bytes.
///
/// Returns `Err(SchemaMismatch)` for any bytes that don't conform to the
/// declared type, including verbs with no known binding.
fn validate_schema(verb: &str, output_bytes: &[u8]) -> Result<(), OutputError> {
    if is_echo_verb(verb) {
        // echo.* → WIT string output. The component model lowers `string` as
        // a UTF-8 byte sequence. We validate UTF-8 here; the ptr+len indirect
        // is resolved by the runtime before calling validate_output, so `output_bytes`
        // holds the raw string bytes.
        std::str::from_utf8(output_bytes).map_err(|e| OutputError::SchemaMismatch {
            verb: verb.to_owned(),
            detail: format!("echo verb output must be valid UTF-8 (WIT string): {e}"),
        })?;
        Ok(())
    } else {
        // No binding for this verb at V0 — fail shut per R-0003-f.
        Err(OutputError::SchemaMismatch {
            verb: verb.to_owned(),
            detail: format!(
                "no WIT output-type binding for verb '{verb}' at V0; \
                 output validation is fail-shut by default (R-0003-f)"
            ),
        })
    }
}

/// Returns `true` iff `verb` is in the `echo.*` namespace.
///
/// The echo plugin's manifest exposes `echo.create`, `echo.get`, `echo.list`, etc.
/// All of these route to the WIT `echo` interface's `string` return type.
fn is_echo_verb(verb: &str) -> bool {
    verb.starts_with("echo.")
}
