//! Canonical, domain-separated approval payloads.
//!
//! The payload is intentionally boring JSON with sorted/deduplicated surfaces
//! and effects. It is the bytes signed by an external human key; no boolean,
//! environment variable, or caller-provided verification bit is authoritative.

use chrono::{DateTime, Utc};
use serde::Serialize;

/// Domain used by risk approvals. Learning approvals use their own domain.
pub const APPROVAL_NAMESPACE: &str = "canon-approval-v1";

#[derive(Debug, Clone, Serialize)]
struct CanonicalApprovalPayload<'a> {
    namespace: &'a str,
    subject: &'a str,
    project: Option<&'a str>,
    artifact_sha: &'a str,
    run_id: Option<&'a str>,
    surface: Vec<&'a str>,
    effects: Vec<&'a str>,
    actor: &'a str,
    timestamp: &'a str,
}

/// The fields of one approval signature payload.
#[derive(Debug, Clone, Copy)]
pub struct ApprovalPayload<'a> {
    /// Domain-separation namespace.
    pub namespace: &'a str,
    /// Approval subject.
    pub subject: &'a str,
    /// Optional project id.
    pub project: Option<&'a str>,
    /// Approved artifact digest.
    pub artifact_sha: &'a str,
    /// Optional run id.
    pub run_id: Option<&'a str>,
    /// Approved surface set.
    pub surface: &'a [String],
    /// Approved effect set.
    pub effects: &'a [String],
    /// Approving actor.
    pub actor: &'a str,
    /// Approval time.
    pub timestamp: &'a DateTime<Utc>,
}

/// Return deterministic bytes for an approval signature.
///
/// Surface and effect collections are normalized as sorted sets. The timestamp
/// is RFC3339 with nanosecond precision, as emitted by `DateTime<Utc>`.
pub fn approval_payload_bytes(fields: &ApprovalPayload<'_>) -> Vec<u8> {
    let ApprovalPayload { namespace, subject, project, artifact_sha, run_id, surface, effects, actor, timestamp } = *fields;
    let mut normalized_surface: Vec<&str> = surface.iter().map(String::as_str).collect();
    normalized_surface.sort_unstable();
    normalized_surface.dedup();
    let mut normalized_effects: Vec<&str> = effects.iter().map(String::as_str).collect();
    normalized_effects.sort_unstable();
    normalized_effects.dedup();
    let timestamp = timestamp.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    serde_json::to_vec(&CanonicalApprovalPayload {
        namespace,
        subject,
        project,
        artifact_sha,
        run_id,
        surface: normalized_surface,
        effects: normalized_effects,
        actor,
        timestamp: &timestamp,
    })
    .expect("canonical approval payload is always serializable")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_normalizes_sets_and_binds_domain() {
        let at = "2026-10-01T00:00:00Z".parse().unwrap();
        let p = |namespace, surface: &[String], effects: &[String]| {
            approval_payload_bytes(&ApprovalPayload { namespace, subject: "task:t", project: Some("p"), artifact_sha: "a", run_id: None, surface, effects, actor: "alice", timestamp: &at })
        };
        let a = p(APPROVAL_NAMESPACE, &["b".into(), "a".into(), "a".into()], &["z".into(), "x".into()]);
        let b = p(APPROVAL_NAMESPACE, &["a".into(), "b".into()], &["x".into(), "z".into()]);
        assert_eq!(a, b);
        assert_ne!(a, p("other", &["a".into(), "b".into()], &["x".into(), "z".into()]));
    }
}
