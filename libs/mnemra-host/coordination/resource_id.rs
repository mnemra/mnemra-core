//! Resource identifiers: the closed, host-registered `<family>:<qualifier>`
//! structured-string scheme (R-0067-a/-b).
//!
//! Pure, IO-free — per the mnemra plugin principle that core logic stays
//! IO-free, this module is validators only. The reserved `actor` family
//! (R-0067-c) is NOT rejected by [`parse`] — a well-formed `actor:<x>`
//! parses; whether it is barred is a CALLER decision ([`Family::is_reserved`]),
//! because the enforcement point differs per `claim` action:
//! `acquire`/`takeover` check the request-side resource string, `renew`/
//! `release` check the resolved lease row's stored `resource` column.

use std::fmt;

/// The closed, host-registered V0 family set (R-0067-a/-b). Closed enum ⇒
/// adding a family is a spec amendment (R-0067-b), never a config knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    /// A git repository's serialized lane — qualifier `<repo>/<lane>`.
    RepoLane,
    /// A workspace file or directory path held across time.
    File,
    /// A named configuration or tool surface.
    Surface,
    /// Reserved for the R-0064-f attachment-as-lease realization; barred
    /// from the entire `claim` surface (R-0067-c).
    Actor,
}

impl Family {
    fn as_str(self) -> &'static str {
        match self {
            Family::RepoLane => "repo-lane",
            Family::File => "file",
            Family::Surface => "surface",
            Family::Actor => "actor",
        }
    }

    /// Exact match on the family token; `None` ⇒ out-of-family.
    fn parse(s: &str) -> Option<Family> {
        match s {
            "repo-lane" => Some(Family::RepoLane),
            "file" => Some(Family::File),
            "surface" => Some(Family::Surface),
            "actor" => Some(Family::Actor),
            _ => None,
        }
    }

    /// R-0067-c: the `actor` family is reserved, barred from every `claim`
    /// action. [`parse`] does not reject it (a well-formed `actor:<x>` still
    /// parses) — the caller applies this check where the spec requires it
    /// (request-side for `acquire`/`takeover`, resolved-lease-row-side for
    /// `renew`/`release`).
    pub(crate) fn is_reserved(self) -> bool {
        matches!(self, Family::Actor)
    }
}

impl fmt::Display for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why [`parse`] rejected a resource string — carried so an `invalid_resource`
/// refusal detail can name the rule (R-0067-a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResourceIdError {
    /// No `:` separator, or the family token is outside the closed set.
    UnknownFamily,
    /// The family parsed but the qualifier (the text after the first `:`) is
    /// empty.
    EmptyQualifier,
    /// The qualifier is non-empty but fails the family's own grammar.
    QualifierRejected { family: Family },
}

impl ResourceIdError {
    /// Render as the `invalid_resource` refusal detail's `rule` text
    /// (R-0067-a) — names the specific reason a resource identifier was
    /// rejected, rather than a generic "malformed" string. Used by
    /// [`crate::coordination::leases::acquire`], which re-derives this from
    /// a SECOND (pure, side-effect-free) [`parse`] call outside the
    /// deciding transaction — `acquire_body` already made the refusal
    /// decision inside it; re-parsing the SAME resource string is
    /// deterministic (this module is IO-free), so the re-derived detail can
    /// never diverge from the transaction's refusal.
    pub(crate) fn detail_rule(&self) -> String {
        match self {
            ResourceIdError::UnknownFamily => {
                "unknown_family: no ':' separator, or the family token is outside \
                 the closed set {repo-lane, file, surface, actor}"
                    .to_string()
            }
            ResourceIdError::EmptyQualifier => {
                "empty_qualifier: the qualifier (text after the family's ':') is empty".to_string()
            }
            ResourceIdError::QualifierRejected { family } => {
                format!("qualifier_rejected: the `{family}` qualifier failed its family's grammar")
            }
        }
    }
}

/// A validated resource identifier `<family>:<qualifier>`. Fields are
/// private — [`parse`] is the sole construction path (already this type's
/// validating constructor: family + qualifier grammar are checked before the
/// struct is built), so no intra-crate struct literal outside this module
/// can mint a `ResourceId` that bypassed [`validate_qualifier`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceId {
    family: Family,
    qualifier: String,
}

impl ResourceId {
    /// The parsed family — read accessor for callers outside this module
    /// (fields are private; see the type's doc).
    pub(crate) fn family(&self) -> Family {
        self.family
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.family, self.qualifier)
    }
}

/// Parse + validate a resource identifier. Splits on the FIRST `:` — a
/// `repo-lane` qualifier carries `/` (never `:`) and the family token carries
/// only `-`, so the first-colon split is unambiguous.
pub(crate) fn parse(resource: &str) -> Result<ResourceId, ResourceIdError> {
    let (family_token, qualifier) = resource
        .split_once(':')
        .ok_or(ResourceIdError::UnknownFamily)?;
    let family = Family::parse(family_token).ok_or(ResourceIdError::UnknownFamily)?;
    if qualifier.is_empty() {
        return Err(ResourceIdError::EmptyQualifier);
    }
    if !validate_qualifier(family, qualifier) {
        return Err(ResourceIdError::QualifierRejected { family });
    }
    Ok(ResourceId {
        family,
        qualifier: qualifier.to_owned(),
    })
}

/// Per-family qualifier validators (impl-tier grammars; the spec fixes only
/// "non-empty, family-validated" — R-0067-a).
///
/// off-default note: none of these deviate from the spec; the exact grammar
/// is the spec-silent detail R-0067-a leaves to implementation (dogfooder-
/// default: simple + sufficient for our V0 usage — revisit-trigger: a real
/// external consumer, or a legitimate identifier these validators reject,
/// surfaces a gap).
fn validate_qualifier(family: Family, q: &str) -> bool {
    match family {
        Family::RepoLane => validate_repo_lane(q),
        Family::File => validate_file(q),
        Family::Surface => validate_surface(q),
        Family::Actor => validate_actor(q),
    }
}

/// `<repo>/<lane>` — non-empty repo, non-empty lane split on the first `/`,
/// free of control characters, length-bounded (parity with `validate_file`/
/// `validate_surface`, which already reject control chars and cap length —
/// this validator previously only checked non-emptiness, so e.g.
/// `repo-lane:\n\x00/lane` was accepted).
fn validate_repo_lane(q: &str) -> bool {
    const MAX_LEN: usize = 512;
    if q.is_empty() || q.chars().count() > MAX_LEN || q.chars().any(|c| c.is_control()) {
        return false;
    }
    match q.split_once('/') {
        Some((repo, lane)) => !repo.is_empty() && !lane.is_empty(),
        None => false,
    }
}

/// A non-empty path, free of control characters, length-bounded.
fn validate_file(q: &str) -> bool {
    const MAX_LEN: usize = 1024;
    !q.is_empty() && q.chars().count() <= MAX_LEN && !q.chars().any(|c| c.is_control())
}

/// A non-empty named surface — identifier-ish charset (alnum, `-`, `_`).
fn validate_surface(q: &str) -> bool {
    const MAX_LEN: usize = 256;
    !q.is_empty()
        && q.chars().count() <= MAX_LEN
        && q.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Used only by the session plane's `actor:<uuid>` realization; `claim` never
/// accepts a resource in this family unrejected (`Family::is_reserved` bars
/// it at the caller). Non-empty is the only rule needed here.
fn validate_actor(q: &str) -> bool {
    !q.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------
    // Family — round-trip + exact-match parse.
    // -----------------------------------------------------------------

    #[test]
    fn family_display_and_parse_round_trip_every_member() {
        for (family, s) in [
            (Family::RepoLane, "repo-lane"),
            (Family::File, "file"),
            (Family::Surface, "surface"),
            (Family::Actor, "actor"),
        ] {
            assert_eq!(family.to_string(), s);
            assert_eq!(Family::parse(s), Some(family));
        }
    }

    #[test]
    fn family_parse_rejects_unknown_token() {
        assert_eq!(Family::parse("bogus"), None);
        assert_eq!(Family::parse(""), None);
        assert_eq!(
            Family::parse("Repo-Lane"),
            None,
            "family match is case-sensitive"
        );
    }

    #[test]
    fn only_actor_family_is_reserved() {
        assert!(Family::Actor.is_reserved());
        for family in [Family::RepoLane, Family::File, Family::Surface] {
            assert!(!family.is_reserved(), "{family} must not be reserved");
        }
    }

    // -----------------------------------------------------------------
    // parse — first-colon split, malformed → error, well-formed → Ok.
    // -----------------------------------------------------------------

    #[test]
    fn parse_accepts_well_formed_identifiers_per_family() {
        let id = parse("repo-lane:mnemra/main-merge").expect("well-formed repo-lane");
        assert_eq!(id.family, Family::RepoLane);
        assert_eq!(id.qualifier, "mnemra/main-merge");

        let id = parse("file:src/lib.rs").expect("well-formed file");
        assert_eq!(id.family, Family::File);
        assert_eq!(id.qualifier, "src/lib.rs");

        let id = parse("surface:ci-build").expect("well-formed surface");
        assert_eq!(id.family, Family::Surface);
        assert_eq!(id.qualifier, "ci-build");
    }

    #[test]
    fn parse_rejects_out_of_family_token() {
        assert_eq!(parse("bogus:x"), Err(ResourceIdError::UnknownFamily));
    }

    #[test]
    fn parse_rejects_missing_qualifier_no_colon_at_all() {
        assert_eq!(parse("repo-lane"), Err(ResourceIdError::UnknownFamily));
    }

    #[test]
    fn parse_rejects_empty_qualifier() {
        assert_eq!(parse("repo-lane:"), Err(ResourceIdError::EmptyQualifier));
    }

    #[test]
    fn parse_rejects_qualifier_failing_its_family_grammar() {
        // repo-lane requires a `/`-separated repo + lane.
        assert_eq!(
            parse("repo-lane:no-slash-here"),
            Err(ResourceIdError::QualifierRejected {
                family: Family::RepoLane
            })
        );
        // surface requires an identifier-ish charset — a space fails it.
        assert_eq!(
            parse("surface:has space"),
            Err(ResourceIdError::QualifierRejected {
                family: Family::Surface
            })
        );
    }

    #[test]
    fn parse_splits_on_the_first_colon_only() {
        // A qualifier that itself carries a `:` is preserved intact — the
        // split point is the FIRST colon, not the last.
        let id = parse("file:src/mod:extra.rs").expect("qualifier may itself carry structure");
        assert_eq!(id.qualifier, "src/mod:extra.rs");
    }

    #[test]
    fn parse_accepts_well_formed_but_reserved_actor_family() {
        // R-0067-c: `parse` does not reject the reserved family — the caller
        // applies `Family::is_reserved` where the spec requires the bar.
        let id = parse("actor:some-uuid").expect("actor family parses");
        assert_eq!(id.family, Family::Actor);
        assert!(id.family.is_reserved());
    }

    // -----------------------------------------------------------------
    // validate_repo_lane — parity hardening: control chars + length bound,
    // matching `validate_file`/`validate_surface`'s shape (self-tested here
    // since this is a pure validator).
    // -----------------------------------------------------------------

    #[test]
    fn validate_repo_lane_rejects_control_chars_and_over_length() {
        // A control character anywhere in the qualifier is rejected — before
        // this fix, only non-emptiness of the split repo/lane halves was
        // checked, so `repo-lane:\n\x00/lane` parsed successfully.
        assert!(
            !validate_repo_lane("repo\n/lane"),
            "a control char in the repo half must be rejected"
        );
        assert!(
            !validate_repo_lane("repo/lane\x00"),
            "a control char in the lane half must be rejected"
        );
        assert_eq!(
            parse("repo-lane:\n\x00/lane"),
            Err(ResourceIdError::QualifierRejected {
                family: Family::RepoLane
            }),
            "a control-char repo-lane qualifier must be refused via `parse` too"
        );

        // Over the length bound is rejected even though repo/lane are each
        // individually non-empty.
        let too_long_repo = "a".repeat(600);
        assert!(
            !validate_repo_lane(&format!("{too_long_repo}/lane")),
            "a repo-lane qualifier over the length bound must be rejected"
        );
    }

    // -----------------------------------------------------------------
    // Display — round-trips `<family>:<qualifier>`.
    // -----------------------------------------------------------------

    #[test]
    fn resource_id_display_round_trips() {
        for resource in [
            "repo-lane:mnemra/main-merge",
            "file:src/lib.rs",
            "surface:ci-build",
        ] {
            let id = parse(resource).expect("well-formed");
            assert_eq!(id.to_string(), resource);
        }
    }
}
