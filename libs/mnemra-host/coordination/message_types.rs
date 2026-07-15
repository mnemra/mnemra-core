//! Closed, code-registered message-type schema validation for the
//! coordination cluster (Task 6; spec
//! `docs/specs/2026-07-06-coordination-wedge.md`, R-0070 + R-0071).
//!
//! The decision this module hides: **message types are a closed set,
//! registered in code, never a runtime-mutable table** (R-0070-a). Every
//! validation call routes through [`SchemaRegistry::validate`] (or the
//! [`validate_message`] convenience wrapper over [`SchemaRegistry::production`]),
//! which resolves `(type_name, schema_version)` to a registered validator
//! FIRST — a lookup miss is [`MessageValidationError::UnknownType`], even
//! when the payload would otherwise be well-formed for a *different*
//! registered version of the same type (R-0070-b: version-before-schema
//! ordering). Only a registry HIT then deserializes the payload into its
//! typed, `#[serde(deny_unknown_fields)]` struct; a deserialization failure
//! — an undeclared field, a wrong-typed value, or a missing required field,
//! at any nesting depth — is [`MessageValidationError::SchemaViolation`].
//! Schema evolution is additive-only (R-0070-c): registering a new `(type,
//! version)` entry never disturbs an existing one; both stay simultaneously
//! reachable on one registry instance.
//!
//! Callers (Task 7's `message send`, out of scope here) go through
//! [`validate_message`], the production entry point.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// `merge-request` message-type identifier (R-0070-a: named constant, not a
/// magic string scattered across Task 6/7 call sites).
pub const MERGE_REQUEST_TYPE_NAME: &str = "merge-request";
/// `merge-request` schema version 1.
pub const MERGE_REQUEST_V1: u16 = 1;
/// `handoff` message-type identifier.
pub const HANDOFF_TYPE_NAME: &str = "handoff";
/// `handoff` schema version 1.
pub const HANDOFF_V1: u16 = 1;

// ---------------------------------------------------------------------
// merge-request v1 (R-0071-a) — every nested `{..}` shape the spec names
// is its own deny_unknown_fields struct; closed-schema applies inside
// array items too, not just at the top level.
// ---------------------------------------------------------------------

/// One governing artifact reference (spec / ADR / plan) a merge request
/// cites as its shift-left review substrate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoverningArtifactRef {
    pub path: String,
    pub kind: String,
}

/// One review-marker reference tying a governing artifact to the marker
/// that reviewed it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewMarkerRef {
    pub artifact_path: String,
    pub marker: String,
}

/// One ride-along change riding with the merge request but not itself
/// governed by a separate artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RideAlong {
    pub description: String,
    pub path_or_ref: String,
}

/// `merge-request` v1 payload — the nine locked particulars (R-0071-a) as
/// named, typed fields; zero freeform/catch-all fallbacks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergeRequestV1 {
    pub repo: String,
    pub branch: String,
    pub worktree_ref: String,
    pub governing_artifacts: Vec<GoverningArtifactRef>,
    pub review_markers: Vec<ReviewMarkerRef>,
    /// Opaque structured object (LOCKED — do not add typed inner fields;
    /// the spec names this a "structured object" without enumerating inner
    /// keys). `deny_unknown_fields` on this struct does not recurse into
    /// the `Value`s held by the map, so arbitrary inner keys are accepted
    /// at V0; only the top-level JSON-object shape is enforced.
    pub gate_facts: serde_json::Map<String, serde_json::Value>,
    pub ride_alongs: Vec<RideAlong>,
    pub base_pin: String,
    pub expected_conflicts: Vec<String>,
    /// Opaque structured object — same LOCKED shape as `gate_facts`.
    pub ci_expectations: serde_json::Map<String, serde_json::Value>,
}

// ---------------------------------------------------------------------
// handoff v1 (R-0071-b) — subject REQUIRED; body/artifact_refs optional
// (serde default), never a fourth field.
// ---------------------------------------------------------------------

/// `handoff` v1 payload — exactly three fields; only `subject` is required.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffV1 {
    pub subject: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
}

// ---------------------------------------------------------------------
// Validated result + closed error taxonomy (R-0070-b)
// ---------------------------------------------------------------------

/// A payload successfully validated against a registered schema.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidatedMessage {
    MergeRequestV1(MergeRequestV1),
    HandoffV1(HandoffV1),
}

/// The closed refusal taxonomy for message validation (R-0070-b).
#[derive(Debug, Clone, PartialEq)]
pub enum MessageValidationError {
    /// `(type_name, schema_version)` is not registered — checked BEFORE any
    /// attempt to deserialize the payload (version-before-schema ordering).
    UnknownType {
        type_name: String,
        schema_version: u16,
    },
    /// A registered `(type_name, schema_version)` was found, but the
    /// payload failed to deserialize into its typed schema — an undeclared
    /// field, a wrong-typed value, or a missing required field, at any
    /// nesting depth.
    SchemaViolation {
        type_name: String,
        schema_version: u16,
        /// The underlying `serde_json` deserialization error message —
        /// human-readable detail for logs/diagnostics. Callers branch on
        /// `.code()`, not on this string's contents.
        detail: String,
    },
}

impl MessageValidationError {
    /// The observable machine-readable code (R-0070-b) — the string Task
    /// 7's `send` maps onto its structured refusal grammar.
    pub fn code(&self) -> &'static str {
        match self {
            MessageValidationError::UnknownType { .. } => "unknown_type",
            MessageValidationError::SchemaViolation { .. } => "schema_violation",
        }
    }
}

/// **Log-injection / payload-echo boundary (Warden M-80):** this `Display`
/// impl interpolates attacker-controlled `type_name` and payload-derived
/// `detail` (the raw `serde_json` deserialization error string) verbatim
/// and unescaped. It MUST NOT be written to any audit/op-log path. Inert as
/// of Task 6 — nothing in this module logs it — but a future consumer (Task
/// 7's `send`/audit) MUST serialize the structured fields discretely
/// (`code()`, `type_name`, `schema_version`, `detail`) per R-0075-e
/// (log-field hygiene), never reach for this `Display` string / `{err}` in
/// a log line.
impl std::fmt::Display for MessageValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageValidationError::UnknownType {
                type_name,
                schema_version,
            } => write!(
                f,
                "unknown message type: ({type_name}, v{schema_version}) is not registered"
            ),
            MessageValidationError::SchemaViolation {
                type_name,
                schema_version,
                detail,
            } => write!(
                f,
                "schema violation for ({type_name}, v{schema_version}): {detail}"
            ),
        }
    }
}

impl std::error::Error for MessageValidationError {}

// ---------------------------------------------------------------------
// The registry (R-0070-a: code-registered closed set)
// ---------------------------------------------------------------------

/// A schema validator: deserializes a payload into its typed
/// [`ValidatedMessage`] variant, or returns a detail string on failure.
pub type Validator = fn(&serde_json::Value) -> Result<ValidatedMessage, String>;

/// The closed, code-registered message-type schema set (R-0070-a).
pub struct SchemaRegistry {
    entries: HashMap<(String, u16), Validator>,
}

impl SchemaRegistry {
    /// The production registry — the two founding registrations. A CLOSED
    /// set: extending it rides the designed-tier pipeline (spec/ADR +
    /// review + merge gate, R-0070-a), never a runtime call.
    pub fn production() -> Self {
        let mut entries: HashMap<(String, u16), Validator> = HashMap::new();
        entries.insert(
            (MERGE_REQUEST_TYPE_NAME.to_string(), MERGE_REQUEST_V1),
            validate_merge_request_v1,
        );
        entries.insert(
            (HANDOFF_TYPE_NAME.to_string(), HANDOFF_V1),
            validate_handoff_v1,
        );
        SchemaRegistry { entries }
    }

    /// Validate `payload` against the registered `(type_name,
    /// schema_version)` schema. Version-before-schema ordering (R-0070-b):
    /// the registry lookup happens FIRST — a miss is `UnknownType` even
    /// when the payload is well-formed for a different registered version
    /// of the same type. Only a registry HIT then deserializes the
    /// payload; a deserialization failure is `SchemaViolation`.
    pub fn validate(
        &self,
        type_name: &str,
        schema_version: u16,
        payload: &serde_json::Value,
    ) -> Result<ValidatedMessage, MessageValidationError> {
        let key = (type_name.to_string(), schema_version);
        let Some(validator) = self.entries.get(&key) else {
            return Err(MessageValidationError::UnknownType {
                type_name: type_name.to_string(),
                schema_version,
            });
        };

        validator(payload).map_err(|detail| MessageValidationError::SchemaViolation {
            type_name: type_name.to_string(),
            schema_version,
            detail,
        })
    }

    /// TEST-ONLY seam exercising the additive-evolution (R-0070-c)
    /// `(type_name, version) -> schema` lookup directly. `test-hooks`-gated
    /// (not `#[cfg(test)]` — integration tests link the crate as an
    /// external consumer, so a `cfg(test)`-gated item is invisible to
    /// `tests/*.rs`; mirrors [`super::write_path::PgCoordinationStore`]'s
    /// `CoordinationFault` seam gating). An always-on registration method
    /// would reopen exactly the "data-driven runtime message-type
    /// registry" R-0070-a forecloses, so this must never compile into the
    /// default build.
    #[cfg(feature = "test-hooks")]
    pub fn insert_test_entry(
        &mut self,
        type_name: &str,
        schema_version: u16,
        validator: Validator,
    ) {
        self.entries
            .insert((type_name.to_string(), schema_version), validator);
    }
}

fn validate_merge_request_v1(payload: &serde_json::Value) -> Result<ValidatedMessage, String> {
    serde_json::from_value::<MergeRequestV1>(payload.clone())
        .map(ValidatedMessage::MergeRequestV1)
        .map_err(|e| e.to_string())
}

fn validate_handoff_v1(payload: &serde_json::Value) -> Result<ValidatedMessage, String> {
    serde_json::from_value::<HandoffV1>(payload.clone())
        .map(ValidatedMessage::HandoffV1)
        .map_err(|e| e.to_string())
}

/// The production entry point — what Task 7's `send` calls.
pub fn validate_message(
    type_name: &str,
    schema_version: u16,
    payload: &serde_json::Value,
) -> Result<ValidatedMessage, MessageValidationError> {
    SchemaRegistry::production().validate(type_name, schema_version, payload)
}
