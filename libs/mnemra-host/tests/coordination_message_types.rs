//! RED-phase acceptance tests for the coordination-wedge message-type
//! registry (Task 6; spec `docs/specs/2026-07-06-coordination-wedge.md`,
//! R-0070 + R-0071). Glitch, dispatch 1579.
//!
//! # Status: RED by design (verify = [])
//!
//! `libs/mnemra-host/coordination/message_types.rs` does not exist yet, and
//! `coordination/mod.rs` does not yet declare `pub mod message_types;`
//! (that one-line registration is also the GREEN phase's — `coordination/**`
//! is this dispatch's `forbid_scope`). Every `use` below therefore fails to
//! resolve and the crate does not COMPILE — that compile failure IS the
//! correct TDD red (mirrors `tests/resource_limits.rs`'s and
//! `tests/workspace_ctx.rs`'s stance on a not-yet-existing module). Nothing
//! here writes implementation; Forge builds `message_types.rs` (plus the
//! `mod.rs` line) to the contract pinned below.
//!
//! # What this file pins (the contract)
//!
//! - **R-0070-a** — a message type is a named, versioned schema shipped in
//!   host code; no message-type table exists. This file references no table
//!   and no runtime registration call — the registry is the `SchemaRegistry`
//!   type + `validate_message` free function below, both host code.
//! - **R-0070-b** — `validate_message` is closed-schema: an undeclared
//!   payload field is `schema_violation`, never silently dropped or
//!   accepted; an unknown `(type_name, schema_version)` pair is
//!   `unknown_type`.
//! - **R-0070-c** — schema evolution is additive-only; a v1 payload
//!   validates against a registry that also carries a v2 of the same type.
//! - **R-0071-a** — `merge-request` v1 carries all nine locked particulars
//!   as named, typed fields (no catch-all).
//! - **R-0071-b** — `handoff` v1 carries exactly `subject` (REQUIRED),
//!   `body`, `artifact_refs` (no catch-all).
//!
//! # PROPOSED PUBLIC API SEAM (the contract GREEN implements)
//!
//! ```text
//! // libs/mnemra-host/coordination/message_types.rs
//!
//! use serde::{Deserialize, Serialize};
//! use std::collections::HashMap;
//!
//! // Founding type identifiers (F9: named constants, not magic strings
//! // scattered across Task 6/7 call sites).
//! pub const MERGE_REQUEST_TYPE_NAME: &str = "merge-request";
//! pub const MERGE_REQUEST_V1: u16 = 1;
//! pub const HANDOFF_TYPE_NAME: &str = "handoff";
//! pub const HANDOFF_V1: u16 = 1;
//!
//! // --- merge-request v1 (R-0071-a) — every nested {..} shape the spec
//! // names is its own deny_unknown_fields struct; closed-schema applies
//! // inside array items too, not just at the top level.
//!
//! #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
//! #[serde(deny_unknown_fields)]
//! pub struct GoverningArtifactRef { pub path: String, pub kind: String }
//!
//! #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
//! #[serde(deny_unknown_fields)]
//! pub struct ReviewMarkerRef { pub artifact_path: String, pub marker: String }
//!
//! #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
//! #[serde(deny_unknown_fields)]
//! pub struct RideAlong { pub description: String, pub path_or_ref: String }
//!
//! #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
//! #[serde(deny_unknown_fields)]
//! pub struct MergeRequestV1 {
//!     pub repo: String,
//!     pub branch: String,
//!     pub worktree_ref: String,
//!     pub governing_artifacts: Vec<GoverningArtifactRef>,
//!     pub review_markers: Vec<ReviewMarkerRef>,
//!     pub gate_facts: serde_json::Map<String, serde_json::Value>,
//!     pub ride_alongs: Vec<RideAlong>,
//!     pub base_pin: String,
//!     pub expected_conflicts: Vec<String>,
//!     pub ci_expectations: serde_json::Map<String, serde_json::Value>,
//! }
//!
//! // --- handoff v1 (R-0071-b) — subject REQUIRED; body/artifact_refs
//! // optional (serde default), never a fourth field.
//!
//! #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
//! #[serde(deny_unknown_fields)]
//! pub struct HandoffV1 {
//!     pub subject: String,
//!     #[serde(default)]
//!     pub body: String,
//!     #[serde(default)]
//!     pub artifact_refs: Vec<String>,
//! }
//!
//! // --- validated result + closed error taxonomy (R-0070-b) ---
//!
//! #[derive(Debug, Clone, PartialEq)]
//! pub enum ValidatedMessage {
//!     MergeRequestV1(MergeRequestV1),
//!     HandoffV1(HandoffV1),
//! }
//!
//! #[derive(Debug, Clone, PartialEq)]
//! pub enum MessageValidationError {
//!     UnknownType { type_name: String, schema_version: u16 },
//!     SchemaViolation { type_name: String, schema_version: u16, detail: String },
//! }
//!
//! impl MessageValidationError {
//!     /// The observable machine-readable code (R-0070-b) — the string Task
//!     /// 7's `send` maps onto its structured refusal grammar.
//!     pub fn code(&self) -> &'static str {
//!         match self {
//!             MessageValidationError::UnknownType { .. } => "unknown_type",
//!             MessageValidationError::SchemaViolation { .. } => "schema_violation",
//!         }
//!     }
//! }
//!
//! impl std::fmt::Display for MessageValidationError { /* .. */ }
//! impl std::error::Error for MessageValidationError {}
//!
//! // --- the registry (R-0070-a: code-registered closed set) ---
//!
//! pub type Validator = fn(&serde_json::Value) -> Result<ValidatedMessage, String>;
//!
//! pub struct SchemaRegistry { entries: HashMap<(String, u16), Validator> }
//!
//! impl SchemaRegistry {
//!     /// The production registry — the two founding registrations. A
//!     /// CLOSED set: extending it rides the designed-tier pipeline
//!     /// (spec/ADR + review + merge gate, R-0070-a), never a runtime call.
//!     pub fn production() -> Self { /* .. */ }
//!
//!     pub fn validate(
//!         &self,
//!         type_name: &str,
//!         schema_version: u16,
//!         payload: &serde_json::Value,
//!     ) -> Result<ValidatedMessage, MessageValidationError> { /* .. */ }
//!
//!     /// TEST-ONLY seam exercising the additive-evolution (R-0070-c)
//!     /// `(type_name, version) -> schema` lookup directly. `test-hooks`-
//!     /// gated (NOT `#[cfg(test)]` — integration tests link the crate as
//!     /// an external consumer, so a `cfg(test)`-gated item is invisible to
//!     /// `tests/*.rs`; mirrors the existing `CoordinationFault` seam's
//!     /// gating in `coordination/write_path.rs`). An always-on
//!     /// registration method would reopen exactly the "data-driven runtime
//!     /// message-type registry" the R-0070-a / Out-of-Scope section
//!     /// forecloses, so this must never compile into the default build.
//!     #[cfg(feature = "test-hooks")]
//!     pub fn insert_test_entry(&mut self, type_name: &str, schema_version: u16, validator: Validator) { /* .. */ }
//! }
//!
//! /// The production entry point — what Task 7's `send` calls.
//! pub fn validate_message(
//!     type_name: &str,
//!     schema_version: u16,
//!     payload: &serde_json::Value,
//! ) -> Result<ValidatedMessage, MessageValidationError> {
//!     SchemaRegistry::production().validate(type_name, schema_version, payload)
//! }
//! ```
//!
//! `coordination/mod.rs` additionally needs one line: `pub mod message_types;`
//! (outside this dispatch's `touch_scope`; GREEN adds it).
//!
//! # Design choices flagged for GREEN / future revisit (not spec gaps — the
//! # locked particulars are the nine/three top-level field NAMES; their
//! # inner shape beyond what the spec spells out is this file's call)
//!
//! - `gate_facts` / `ci_expectations`: the spec names these "structured
//!   object" without enumerating inner keys ("named gate → status/evidence
//!   entries"; "naming the checks expected to run/pass"). Typed here as
//!   `serde_json::Map<String, serde_json::Value>` — guarantees the JSON-
//!   object shape (a bare string or array is refused, see
//!   `merge_request_v1_gate_facts_not_an_object_is_refused_schema_violation`)
//!   without inventing inner field names the spec never names. Revisit if
//!   Task 7 or a consumer needs typed inner keys.
//! - `expected_conflicts`: "array, possibly empty" with no item shape named.
//!   Modeled as `Vec<String>` (each entry a path/description). Revisit if a
//!   consumer needs a richer per-conflict shape.
//! - `SchemaRegistry::insert_test_entry` is the only piece of this contract
//!   not exercised under the default feature set — see the `test-hooks`
//!   rationale inline above. `just verify-test` (default features) still
//!   runs every OTHER test in this file; `just verify-test-hooks` additionally
//!   runs the additive-evolution test that needs the seam.
//! - `SchemaRegistry::production()` rebuilds its two-entry `HashMap` on every
//!   call (via `validate_message`) rather than caching a `LazyLock` static.
//!   Fine at V0 scale (two entries); flagged as a cheap future optimization,
//!   not a correctness concern.
//!
//! # AC ↔ test map
//!
//! | AC / R-ID | Test(s) |
//! |---|---|
//! | R-0070-a (code-registered; no table; closed set) | Whole-file compile-red posture (no table symbol anywhere); `SchemaRegistry::production()` is the only construction path exercised |
//! | R-0071-a (merge-request v1: nine named fields, no catch-all — happy path) | `merge_request_v1_conformant_payload_validates_into_every_locked_field` |
//! | R-0071-a (`expected_conflicts: []` boundary) | same test (fixture uses `[]`) |
//! | R-0071-b (handoff v1: exactly three fields — happy path) | `handoff_v1_conformant_payload_validates_into_exactly_three_fields` |
//! | R-0071-b (only `subject` REQUIRED) | `handoff_v1_omitted_optional_fields_default_to_empty` |
//! | R-0071-b (`subject` missing → refused) | `handoff_v1_missing_required_subject_is_schema_violation` |
//! | R-0070-b / R-0071-a (merge-request undeclared field → `schema_violation`) | `merge_request_v1_undeclared_extra_field_is_refused_schema_violation` |
//! | R-0071-b (handoff undeclared 4th field → `schema_violation`) | `handoff_v1_undeclared_fourth_field_is_refused_schema_violation` |
//! | R-0070-b (unknown type → `unknown_type`) | `unregistered_type_name_is_refused_unknown_type` |
//! | R-0070-b (unknown version of a known type → `unknown_type`, NOT `schema_violation`) | `unregistered_schema_version_of_a_known_type_is_refused_unknown_type_not_schema_violation` |
//! | R-0070-b (wrong-typed field value → `schema_violation`) | `merge_request_v1_wrong_field_type_is_refused_schema_violation` |
//! | R-0070-b (closed-schema applies inside nested array items — missing required nested field) | `merge_request_v1_nested_governing_artifact_missing_kind_is_refused_schema_violation` |
//! | R-0070-b (`deny_unknown_fields` regression guard on a nested struct — undeclared EXTRA nested field) | `merge_request_v1_nested_governing_artifact_extra_field_is_refused_schema_violation` |
//! | R-0071-a (`gate_facts` must be object-shaped) | `merge_request_v1_gate_facts_not_an_object_is_refused_schema_violation` |
//! | R-0070-c (v1 validates alongside a registered v2 of the same type) | `v1_payload_still_validates_against_a_registry_that_also_carries_a_synthetic_v2` (`test-hooks`) |

use serde_json::json;

use mnemra_host::coordination::message_types::{
    HANDOFF_TYPE_NAME, HANDOFF_V1, MERGE_REQUEST_TYPE_NAME, MERGE_REQUEST_V1,
    MessageValidationError, ValidatedMessage, validate_message,
};
#[cfg(feature = "test-hooks")]
use mnemra_host::coordination::message_types::{HandoffV1, SchemaRegistry};

// ===========================================================================
// Fixture builders
// ===========================================================================

/// A fully-conformant `merge-request` v1 payload populating all nine locked
/// particulars, including the explicit `expected_conflicts: []`
/// possibly-empty boundary the AC calls out by name.
fn valid_merge_request_v1_json() -> serde_json::Value {
    json!({
        "repo": "mnemra-core",
        "branch": "impl/coord-wedge-t6-message-types",
        "worktree_ref": ".worktrees/impl/coord-wedge-t6-message-types",
        "governing_artifacts": [
            { "path": "docs/specs/2026-07-06-coordination-wedge.md", "kind": "spec" }
        ],
        "review_markers": [
            {
                "artifact_path": "docs/specs/2026-07-06-coordination-wedge.md",
                "marker": "reviewed:8f27cc58a381"
            }
        ],
        "gate_facts": {
            "ci": "green",
            "warden_review": "approved"
        },
        "ride_alongs": [
            { "description": "justfile NONPG_TEST_FLAGS wiring", "path_or_ref": "justfile" }
        ],
        "base_pin": "92fa7a7725f16b8066a671fae02d54f8fe213d06",
        "expected_conflicts": [],
        "ci_expectations": {
            "verify-test": "pass",
            "verify-lint": "pass"
        }
    })
}

/// A fully-conformant `handoff` v1 payload populating all three fields.
fn valid_handoff_v1_json() -> serde_json::Value {
    json!({
        "subject": "Task 6 red-phase handoff",
        "body": "message-type registry tests committed on impl/coord-wedge-t6-message-types",
        "artifact_refs": ["docs/specs/2026-07-06-coordination-wedge.md"]
    })
}

// ===========================================================================
// R-0071-a — merge-request v1: every locked particular is a named field
// (happy path + schema audit, `expected_conflicts: []` boundary)
// ===========================================================================

/// GIVEN a payload populating all nine locked `merge-request` v1 particulars
/// (including the possibly-empty `expected_conflicts: []` boundary),
/// WHEN it is validated against `(merge-request, 1)`,
/// THEN it validates and every particular is readable as a named, typed
/// field on the resulting `MergeRequestV1` — not a string-matched key in a
/// freeform blob. Each field read below only compiles/passes if the
/// particular is a first-class named field: that IS the R-0071-a schema
/// audit, expressed as a test rather than a manual review pass.
#[test]
fn merge_request_v1_conformant_payload_validates_into_every_locked_field() {
    let payload = valid_merge_request_v1_json();
    let result = validate_message(MERGE_REQUEST_TYPE_NAME, MERGE_REQUEST_V1, &payload);

    let ValidatedMessage::MergeRequestV1(mr) = result.expect(
        "R-0071-a: a fixture populating all nine locked merge-request v1 particulars must \
         validate",
    ) else {
        panic!(
            "validate_message(\"merge-request\", 1, ..) must return ValidatedMessage::MergeRequestV1"
        );
    };

    assert_eq!(mr.repo, "mnemra-core");
    assert_eq!(mr.branch, "impl/coord-wedge-t6-message-types");
    assert_eq!(
        mr.worktree_ref,
        ".worktrees/impl/coord-wedge-t6-message-types"
    );
    assert_eq!(mr.governing_artifacts.len(), 1);
    assert_eq!(
        mr.governing_artifacts[0].path,
        "docs/specs/2026-07-06-coordination-wedge.md"
    );
    assert_eq!(mr.governing_artifacts[0].kind, "spec");
    assert_eq!(mr.review_markers.len(), 1);
    assert_eq!(
        mr.review_markers[0].artifact_path,
        "docs/specs/2026-07-06-coordination-wedge.md"
    );
    assert_eq!(mr.review_markers[0].marker, "reviewed:8f27cc58a381");
    assert_eq!(
        mr.gate_facts.get("ci").and_then(|v| v.as_str()),
        Some("green")
    );
    assert_eq!(
        mr.gate_facts.get("warden_review").and_then(|v| v.as_str()),
        Some("approved")
    );
    assert_eq!(mr.ride_alongs.len(), 1);
    assert_eq!(
        mr.ride_alongs[0].description,
        "justfile NONPG_TEST_FLAGS wiring"
    );
    assert_eq!(mr.ride_alongs[0].path_or_ref, "justfile");
    assert_eq!(mr.base_pin, "92fa7a7725f16b8066a671fae02d54f8fe213d06");
    assert_eq!(
        mr.expected_conflicts,
        Vec::<String>::new(),
        "R-0071-a: expected_conflicts: [] is the declared possibly-empty boundary"
    );
    assert_eq!(
        mr.ci_expectations
            .get("verify-test")
            .and_then(|v| v.as_str()),
        Some("pass")
    );
}

// ===========================================================================
// R-0071-b — handoff v1: exactly three fields (happy path; only `subject`
// is REQUIRED)
// ===========================================================================

/// GIVEN a payload populating all three `handoff` v1 fields,
/// WHEN it is validated against `(handoff, 1)`,
/// THEN it validates and each field is readable as a named, typed field on
/// the resulting `HandoffV1`.
#[test]
fn handoff_v1_conformant_payload_validates_into_exactly_three_fields() {
    let payload = valid_handoff_v1_json();
    let result = validate_message(HANDOFF_TYPE_NAME, HANDOFF_V1, &payload);

    let ValidatedMessage::HandoffV1(h) =
        result.expect("R-0071-b: a fixture populating all three handoff v1 fields must validate")
    else {
        panic!("validate_message(\"handoff\", 1, ..) must return ValidatedMessage::HandoffV1");
    };

    assert_eq!(h.subject, "Task 6 red-phase handoff");
    assert_eq!(
        h.body,
        "message-type registry tests committed on impl/coord-wedge-t6-message-types"
    );
    assert_eq!(
        h.artifact_refs,
        vec!["docs/specs/2026-07-06-coordination-wedge.md".to_string()]
    );
}

/// GIVEN a `handoff` payload carrying ONLY the required `subject` field,
/// WHEN it is validated,
/// THEN it validates — `body` and `artifact_refs` are NOT required (R-0071-b
/// names only `subject` REQUIRED) and default to empty.
#[test]
fn handoff_v1_omitted_optional_fields_default_to_empty() {
    let payload = json!({ "subject": "Only subject provided" });
    let result = validate_message(HANDOFF_TYPE_NAME, HANDOFF_V1, &payload);

    let ValidatedMessage::HandoffV1(h) = result.expect(
        "R-0071-b: body and artifact_refs are optional — omitting both must still validate",
    ) else {
        panic!("validate_message(\"handoff\", 1, ..) must return ValidatedMessage::HandoffV1");
    };

    assert_eq!(h.subject, "Only subject provided");
    assert_eq!(h.body, "");
    assert!(h.artifact_refs.is_empty());
}

/// GIVEN a `handoff` payload missing the REQUIRED `subject` field,
/// WHEN it is validated,
/// THEN it is refused `schema_violation` (R-0071-b: `subject` is REQUIRED).
#[test]
fn handoff_v1_missing_required_subject_is_schema_violation() {
    let payload = json!({ "body": "no subject here" });
    let err = validate_message(HANDOFF_TYPE_NAME, HANDOFF_V1, &payload)
        .expect_err("R-0071-b: subject is REQUIRED — omitting it must be refused");

    assert_eq!(err.code(), "schema_violation");
    assert!(matches!(
        err,
        MessageValidationError::SchemaViolation { .. }
    ));
}

// ===========================================================================
// R-0070-b — closed-schema validation: undeclared extra fields refused
// ===========================================================================

/// GIVEN an otherwise-conformant `merge-request` v1 payload carrying one
/// undeclared extra field,
/// WHEN it is validated,
/// THEN it is refused `schema_violation` — never silently dropped, never
/// silently accepted (R-0070-b, R-0071-a "zero freeform/catch-all
/// fallbacks").
#[test]
fn merge_request_v1_undeclared_extra_field_is_refused_schema_violation() {
    let mut payload = valid_merge_request_v1_json();
    payload.as_object_mut().unwrap().insert(
        "notes".to_string(),
        serde_json::Value::String("freeform prose fallback".to_string()),
    );

    let err = validate_message(MERGE_REQUEST_TYPE_NAME, MERGE_REQUEST_V1, &payload).expect_err(
        "R-0070-b/R-0071-a: an undeclared extra field must be refused, never silently dropped \
         or accepted",
    );

    assert_eq!(err.code(), "schema_violation");
    assert!(matches!(
        err,
        MessageValidationError::SchemaViolation { ref type_name, schema_version: 1, .. }
            if type_name == "merge-request"
    ));
}

/// GIVEN an otherwise-conformant `handoff` v1 payload carrying a fourth
/// field,
/// WHEN it is validated,
/// THEN it is refused `schema_violation` — `handoff` v1 carries EXACTLY
/// three fields, no catch-all (R-0071-b).
#[test]
fn handoff_v1_undeclared_fourth_field_is_refused_schema_violation() {
    let mut payload = valid_handoff_v1_json();
    payload.as_object_mut().unwrap().insert(
        "priority".to_string(),
        serde_json::Value::String("high".to_string()),
    );

    let err = validate_message(HANDOFF_TYPE_NAME, HANDOFF_V1, &payload).expect_err(
        "R-0071-b: handoff v1 carries EXACTLY three fields — a fourth field must be refused \
         schema_violation, not silently accepted",
    );

    assert_eq!(err.code(), "schema_violation");
}

// ===========================================================================
// R-0070-b — unknown type / unknown version: refused `unknown_type`,
// distinct from `schema_violation`
// ===========================================================================

/// GIVEN a type name that is not registered at all,
/// WHEN it is validated,
/// THEN it is refused `unknown_type` (R-0070-b).
#[test]
fn unregistered_type_name_is_refused_unknown_type() {
    let payload = json!({ "anything": "goes" });
    let err = validate_message("not-a-real-type", 1, &payload)
        .expect_err("R-0070-b: an unregistered type name must be refused unknown_type");

    assert_eq!(err.code(), "unknown_type");
    assert!(matches!(
        err,
        MessageValidationError::UnknownType { ref type_name, schema_version: 1 }
            if type_name == "not-a-real-type"
    ));
}

/// GIVEN a KNOWN type name at an unregistered schema version (only v1 of
/// `merge-request` exists at V0),
/// WHEN a well-formed-for-v1 payload is validated against version 99,
/// THEN it is refused `unknown_type` — NOT `schema_violation`. Version
/// selection happens before schema validation: a well-formed payload for a
/// different version is not "close but invalid," it targets a schema that
/// does not exist.
#[test]
fn unregistered_schema_version_of_a_known_type_is_refused_unknown_type_not_schema_violation() {
    let payload = valid_merge_request_v1_json();
    let err = validate_message(MERGE_REQUEST_TYPE_NAME, 99, &payload).expect_err(
        "R-0070-b: an unregistered (type, version) pair must be refused unknown_type even when \
         the payload itself is well-formed for v1 of the same type",
    );

    assert_eq!(err.code(), "unknown_type");
    assert!(matches!(
        err,
        MessageValidationError::UnknownType { ref type_name, schema_version: 99 }
            if type_name == "merge-request"
    ));
}

// ===========================================================================
// R-0070-b — invalid values: wrong types, nested-array closed schema,
// object-shaped structured fields
// ===========================================================================

/// GIVEN a `merge-request` v1 payload where `repo` (a String field) carries
/// a number instead,
/// WHEN it is validated,
/// THEN it is refused `schema_violation`.
#[test]
fn merge_request_v1_wrong_field_type_is_refused_schema_violation() {
    let mut payload = valid_merge_request_v1_json();
    payload["repo"] = json!(12345);

    let err = validate_message(MERGE_REQUEST_TYPE_NAME, MERGE_REQUEST_V1, &payload)
        .expect_err("R-0070-b: a wrong-typed field value must be refused schema_violation");

    assert_eq!(err.code(), "schema_violation");
}

/// GIVEN a `merge-request` v1 payload whose single `governing_artifacts`
/// entry is missing the nested `kind` field (`governing_artifacts` is
/// `{path, kind}` per R-0071-a),
/// WHEN it is validated,
/// THEN it is refused `schema_violation` — closed-schema applies inside
/// array items, not only at the payload's top level.
#[test]
fn merge_request_v1_nested_governing_artifact_missing_kind_is_refused_schema_violation() {
    let mut payload = valid_merge_request_v1_json();
    payload["governing_artifacts"] = json!([
        { "path": "docs/specs/2026-07-06-coordination-wedge.md" }
    ]);

    let err = validate_message(MERGE_REQUEST_TYPE_NAME, MERGE_REQUEST_V1, &payload).expect_err(
        "R-0071-a: governing_artifacts entries are {path, kind} — a missing nested field must \
         be refused schema_violation",
    );

    assert_eq!(err.code(), "schema_violation");
}

/// GIVEN a `merge-request` v1 payload whose single `governing_artifacts`
/// entry carries an undeclared EXTRA field alongside its two declared ones
/// (`governing_artifacts` is `{path, kind}` per R-0071-a),
/// WHEN it is validated,
/// THEN it is refused `schema_violation` — this is the actual
/// `deny_unknown_fields` regression guard for `GoverningArtifactRef`.
/// (Warden Medium-85: the sibling test above — missing `kind` — is refused
/// because `kind` is a REQUIRED field, a rejection serde enforces
/// independent of `#[serde(deny_unknown_fields)]`; it does not prove the
/// attribute is actually present on the nested struct. An extra, undeclared
/// field is refused only if `deny_unknown_fields` is genuinely on
/// `GoverningArtifactRef` — that's the property this test pins, closing the
/// gap the sibling test left open.)
#[test]
fn merge_request_v1_nested_governing_artifact_extra_field_is_refused_schema_violation() {
    let mut payload = valid_merge_request_v1_json();
    payload["governing_artifacts"] = json!([
        {
            "path": "docs/specs/2026-07-06-coordination-wedge.md",
            "kind": "spec",
            "bogus": "x"
        }
    ]);

    let err = validate_message(MERGE_REQUEST_TYPE_NAME, MERGE_REQUEST_V1, &payload).expect_err(
        "R-0070-b: GoverningArtifactRef carries #[serde(deny_unknown_fields)] — an undeclared \
         extra field inside a governing_artifacts array item must be refused schema_violation, \
         not silently dropped or accepted",
    );

    assert_eq!(err.code(), "schema_violation");
    assert!(matches!(
        err,
        MessageValidationError::SchemaViolation { ref type_name, schema_version: 1, .. }
            if type_name == "merge-request"
    ));
}

/// GIVEN a `merge-request` v1 payload where `gate_facts` (a "structured
/// object" per R-0071-a) carries a bare string instead of a JSON object,
/// WHEN it is validated,
/// THEN it is refused `schema_violation` — not silently coerced.
#[test]
fn merge_request_v1_gate_facts_not_an_object_is_refused_schema_violation() {
    let mut payload = valid_merge_request_v1_json();
    payload["gate_facts"] = json!("not an object");

    let err = validate_message(MERGE_REQUEST_TYPE_NAME, MERGE_REQUEST_V1, &payload).expect_err(
        "R-0071-a: gate_facts is a structured object — a bare string must be refused \
         schema_violation, not silently coerced",
    );

    assert_eq!(err.code(), "schema_violation");
}

// ===========================================================================
// R-0070-c — additive-only evolution: a v1 payload validates against a host
// carrying v1 AND v2 of the same type
// ===========================================================================

/// GIVEN a registry carrying the production `(handoff, 1)` registration PLUS
/// a test-only synthetic `(handoff, 2)` entry (only v1 of each founding type
/// exists at V0 — this test exercises the `(type_name, version) -> schema`
/// lookup mechanism directly via the `test-hooks`-gated
/// `SchemaRegistry::insert_test_entry` seam, per the module doc's "Design
/// choices" section on why that seam must not be always-compiled),
/// WHEN a v1 `handoff` payload is validated at version 1,
/// THEN it validates exactly as it would on a v1-only registry — v2's
/// coexistence does not shadow or otherwise disturb the v1 lookup — AND the
/// same payload also validates when looked up AT version 2, proving both
/// versions are genuinely, simultaneously live on the one registry instance
/// (R-0070-c).
#[cfg(feature = "test-hooks")]
#[test]
fn v1_payload_still_validates_against_a_registry_that_also_carries_a_synthetic_v2() {
    fn synthetic_handoff_v2(payload: &serde_json::Value) -> Result<ValidatedMessage, String> {
        // A trivial v2 for THIS test only: same shape as v1. The point is
        // proving the (type, version) lookup coexists with v1, not
        // authoring a real v2 schema — no real v2 registration exists at V0
        // (R-0070-c is additive-only and no breaking change has been
        // authored against either founding type).
        serde_json::from_value::<HandoffV1>(payload.clone())
            .map(ValidatedMessage::HandoffV1)
            .map_err(|e| e.to_string())
    }

    let mut registry = SchemaRegistry::production();
    registry.insert_test_entry(HANDOFF_TYPE_NAME, 2, synthetic_handoff_v2);

    let payload = valid_handoff_v1_json();

    let v1_result = registry.validate(HANDOFF_TYPE_NAME, HANDOFF_V1, &payload);
    assert!(
        matches!(v1_result, Ok(ValidatedMessage::HandoffV1(_))),
        "R-0070-c: a v1 payload must still validate against v1's own schema when a v2 entry \
         for the SAME type is also registered on the host — the v1 lookup must be unaffected \
         by v2's coexistence. Got: {v1_result:?}"
    );

    // Evidence the v2 entry is genuinely present alongside v1 (not merely
    // declared and never reached): the same payload also validates when
    // looked up AT version 2, proving both versions are simultaneously live.
    let v2_result = registry.validate(HANDOFF_TYPE_NAME, 2, &payload);
    assert!(
        matches!(v2_result, Ok(ValidatedMessage::HandoffV1(_))),
        "R-0070-c: the synthetic v2 entry must be independently reachable at (handoff, 2) \
         while v1 remains reachable at (handoff, 1) — both versions live on the same registry \
         instance. Got: {v2_result:?}"
    );
}
