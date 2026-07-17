//! The privileged-audit record — the shape staged into the `coordination_audit`
//! outbox table (R-0075-b/-c).
//!
//! Decision hidden: **audit is an in-transaction outbox row, not a host-fn
//! call.** `AuditRecord` mirrors the D-SURFACE emit shape (`event_type` +
//! `event_version` + json payload) so the future external reader consumes the
//! outbox without re-mapping. Staging happens on the coordination txn
//! ([`crate::coordination::write_path::CoordinationTxn::stage_audit`]); the
//! flush inside the commit txn is implemented by the machinery
//! ([`crate::coordination::write_path::CoordinationTxn::flush_staged_audit`]).

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

/// One privileged-audit event, staged into the `coordination_audit` outbox
/// atomically with the state transition it records.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditRecord {
    /// The audited event class (closed-at-read, additive via `#[non_exhaustive]`).
    pub event_type: AuditEventType,
    /// Payload schema version; starts at 1, evolves additively (R-0070-c shape).
    pub event_version: u16,
    /// Owning workspace (tenant scope, R-0076-b).
    pub workspace_id: Uuid,
    /// The principal the event concerns, where applicable.
    pub actor_id: Option<Uuid>,
    /// Structured evidence (prior/successor session, expiry, role-instance …).
    pub payload: serde_json::Value,
}

/// The audited privileged-event classes (R-0075-b subset).
///
/// `#[non_exhaustive]` (F4): Tasks 5/7 add `LeaseTakeover` / `Disposition` call
/// sites and any later class without breaking downstream matches.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEventType {
    /// An actor row was minted (bind resolve-or-create; send-side is Task 7).
    Registration,
    /// A fresh attachment (no prior holder).
    Attachment,
    /// An audited takeover of a stale attachment (carries prior + successor
    /// session — the idle-resume vs genuine-succession discriminator, R-0064-d).
    AttachmentSuccession,
    /// A lease takeover (Task 5).
    LeaseTakeover,
    /// A message disposition (Task 7).
    Disposition,
}

impl AuditEventType {
    /// Stable TEXT-column encoding for `coordination_audit.event_type`
    /// (migration 25, `schema/init.rs`). The column deliberately carries no
    /// `CHECK` constraint — `AuditEventType` is `#[non_exhaustive]` and
    /// evolves additively (Tasks 5/7 add classes), so the closed set is
    /// enforced here, in host code, not at the schema level (mirrors the
    /// `messages.payload` host-side validation posture named in the migration
    /// comment).
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditEventType::Registration => "registration",
            AuditEventType::Attachment => "attachment",
            AuditEventType::AttachmentSuccession => "attachment_succession",
            AuditEventType::LeaseTakeover => "lease_takeover",
            AuditEventType::Disposition => "disposition",
        }
    }
}

/// Evidence carried by an audited succession: the expired lease's declared
/// expiry and the transaction-time "now" that observed it past expiry
/// (`now >= expires_at`, store clock — R-0064-d / R-0066-a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpiryEvidence {
    /// The prior holder's declared expiry.
    pub expires_at: DateTime<Utc>,
    /// The store-clock instant that observed the lease past expiry.
    pub observed_now: DateTime<Utc>,
}

impl AuditRecord {
    /// Registration: an actor row was minted for `role_instance`.
    pub fn registration(workspace_id: Uuid, actor_id: Uuid, role_instance: &str) -> Self {
        AuditRecord {
            event_type: AuditEventType::Registration,
            event_version: 1,
            workspace_id,
            actor_id: Some(actor_id),
            payload: json!({ "role_instance": role_instance }),
        }
    }

    /// Fresh attachment: `session` bound to `actor_id` with no prior holder.
    pub fn attachment(workspace_id: Uuid, actor_id: Uuid, session: Uuid) -> Self {
        AuditRecord {
            event_type: AuditEventType::Attachment,
            event_version: 1,
            workspace_id,
            actor_id: Some(actor_id),
            payload: json!({ "session": session.to_string() }),
        }
    }

    /// Audited succession: `successor_session` took over `actor_id` from the
    /// expired `prior_session`, with the expiry evidence that authorized it.
    pub fn attachment_succession(
        workspace_id: Uuid,
        actor_id: Uuid,
        prior_session: Uuid,
        successor_session: Uuid,
        expiry: ExpiryEvidence,
    ) -> Self {
        AuditRecord {
            event_type: AuditEventType::AttachmentSuccession,
            event_version: 1,
            workspace_id,
            actor_id: Some(actor_id),
            payload: json!({
                "prior_session": prior_session.to_string(),
                "successor_session": successor_session.to_string(),
                "expires_at": expiry.expires_at.to_rfc3339(),
                "observed_now": expiry.observed_now.to_rfc3339(),
            }),
        }
    }

    /// Audited lease takeover (R-0066-a/-b, Task 5 d): `new_holder` recovered
    /// `resource` from `prior_holder` after the prior lease's declared
    /// expiry. Carries the four R-0066-b evidence fields — `prior_holder`,
    /// `new_holder`, `expires_at` (the PRIOR lease's declared expiry), and
    /// `takeover_ts` (the store-clock instant that observed it past expiry,
    /// `expiry.observed_now`). The JSON key is `takeover_ts`, NOT
    /// `observed_now` — build plan §3.4 step 5 pins this key name, and
    /// `tests/coordination_leases.rs`'s `lease_takeover_audit_rows` queries
    /// it verbatim.
    ///
    /// `actor_id` is `Some(new_holder)` — the acting actor performing the
    /// takeover, the SAME actor `run_write`'s own op-log attribution already
    /// carries via `CoordinationTxn::record_acting_actor` (R-0075-a). A
    /// takeover concerns two distinct actors and the build plan leaves this
    /// field unspecified, so this is a dogfooder-default pick made for
    /// consistency with the op-log's own attribution convention — the
    /// `payload`'s `prior_holder`/`new_holder` keys carry both identities
    /// regardless of this field's value.
    pub fn lease_takeover(
        workspace_id: Uuid,
        prior_holder: Uuid,
        new_holder: Uuid,
        expiry: ExpiryEvidence,
    ) -> Self {
        AuditRecord {
            event_type: AuditEventType::LeaseTakeover,
            event_version: 1,
            workspace_id,
            actor_id: Some(new_holder),
            payload: json!({
                "prior_holder": prior_holder.to_string(),
                "new_holder": new_holder.to_string(),
                "expires_at": expiry.expires_at.to_rfc3339(),
                "takeover_ts": expiry.observed_now.to_rfc3339(),
            }),
        }
    }

    /// A message disposition (Task 7 slice b; R-0075-b disposition half /
    /// AC10): `actor_id` — the addressee, the only principal permitted to
    /// disposition (R-0069-b) — dispositioned `message_id`. The payload's
    /// identifying key is `message_id`, the natural analogue of
    /// `registration`'s `role_instance` key and `lease_takeover`'s
    /// `prior_holder`/`new_holder` keys — no plan/spec text fixes an exact
    /// key name, so this is the contract `tests/coordination_messages.rs`
    /// test 20 pins. Kept minimal (no `disposition` member or `note`
    /// duplicated into the payload): the row itself is the source of truth
    /// for those fields, and this audit exists to prove EMISSION happened,
    /// not to re-carry state the `messages` row already owns.
    pub fn disposition(workspace_id: Uuid, actor_id: Uuid, message_id: Uuid) -> Self {
        AuditRecord {
            event_type: AuditEventType::Disposition,
            event_version: 1,
            workspace_id,
            actor_id: Some(actor_id),
            payload: json!({ "message_id": message_id.to_string() }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `coordination_audit.event_type` TEXT column carries no `CHECK`
    /// constraint (migration 25 comment) — `as_str()` IS the closed-set
    /// enforcement. Pin the encoding for every variant that exists today; a
    /// new non-exhaustive variant added later needs its own case here (the
    /// match is exhaustive within-crate despite `#[non_exhaustive]`, which
    /// only affects downstream crates).
    #[test]
    fn as_str_covers_every_current_variant_with_a_stable_encoding() {
        assert_eq!(AuditEventType::Registration.as_str(), "registration");
        assert_eq!(AuditEventType::Attachment.as_str(), "attachment");
        assert_eq!(
            AuditEventType::AttachmentSuccession.as_str(),
            "attachment_succession"
        );
        assert_eq!(AuditEventType::LeaseTakeover.as_str(), "lease_takeover");
        assert_eq!(AuditEventType::Disposition.as_str(), "disposition");
    }
}
