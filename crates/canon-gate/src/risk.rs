//! Effect-aware risk-tier approval verification.
//!
//! This check is deliberately separate from `risk_routing`: routing derives
//! ordinary role cells, while these declarative rules select a highest-ranked
//! tier from the CURRENT faithful evidence generation's surface references
//! and require verified, accountable human approvals. `evidence_sha` is the
//! existing binding axis; if it is absent, or a required tier cannot be
//! evaluated against a current surface/effect binding, the check fails closed.
//! It uses the existing `uncovered-cell` failure class so the gate's closed
//! failure vocabulary remains unchanged.

use std::collections::{BTreeMap, BTreeSet};

use canon_model::{EvidenceRecord, EvidenceVerdict};
use canon_store::partition::content_digest12;

use crate::context::{GateCheck, GateContext};
use crate::coverage::CellSubject;
use crate::failure_class::{FailureClass, Violation};
use crate::policy::{PolicyDiagnostic, RiskTierRule};
use crate::staleness::StalenessCheck;

/// Verifies minimum distinct human approvals for the highest matching risk
/// tier in each existing evidence subject group.
pub struct RiskApprovalCheck;

impl GateCheck for RiskApprovalCheck {
    fn name(&self) -> &'static str {
        "risk-approval"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        if let Some(detail) = ctx.policy.diagnostics.iter().find_map(|diagnostic| match diagnostic {
            PolicyDiagnostic::InvalidSection { section: "risk_tiers", detail } => Some(detail.as_str()),
            _ => None,
        }) {
            return vec![Violation::new(
                FailureClass::UncoveredCell,
                "risk_tiers",
                format!("risk_tiers policy-invalid: {detail}"),
            )];
        }
        if ctx.policy.risk_tiers.is_empty() {
            return Vec::new();
        }

        let stale_subjects: BTreeSet<String> = StalenessCheck
            .run(ctx)
            .into_iter()
            .filter(|violation| violation.class == FailureClass::StaleEvidence)
            .map(|violation| violation.subject)
            .collect();
        let mut groups: BTreeMap<CellSubject, Vec<&EvidenceRecord>> = BTreeMap::new();
        for record in &ctx.evidence {
            if let Some(subject) = CellSubject::of(record) {
                groups.entry(subject).or_default().push(record);
            }
        }

        groups
            .into_iter()
            .filter_map(|(subject, records)| {
                let required_tier = || ctx.policy.risk_tiers.iter()
                    .filter(|(_, rule)| rule.min_human_approvals > 0)
                    .max_by(|(name_a, rule_a), (name_b, rule_b)| rule_a.rank.cmp(&rule_b.rank).then_with(|| name_b.cmp(name_a)));
                let current = latest_record(&records)?;
                let current_records: Vec<&EvidenceRecord> = match current.evidence_sha.as_ref() {
                    Some(current_sha) => records
                        .iter()
                        .copied()
                        .filter(|record| !stale_subjects.contains(subject.as_str()) && record.evidence_sha.as_ref() == Some(current_sha) && record.verdict == EvidenceVerdict::Faithful)
                        .collect(),
                    None => Vec::new(),
                };
                let surfaces: BTreeSet<&str> = if current_records.is_empty() {
                    current.surface_ref.iter().map(String::as_str).collect()
                } else {
                    current_records.iter().flat_map(|record| record.surface_ref.iter().map(String::as_str)).collect()
                };
                let (tier_name, rule) = ctx.policy.risk_tiers.iter()
                    .filter(|(_, rule)| surfaces.iter().any(|surface| surface_matches(surface, rule)))
                    .max_by(|(name_a, rule_a), (name_b, rule_b)| rule_a.rank.cmp(&rule_b.rank).then_with(|| name_b.cmp(name_a)))
                    .or_else(required_tier)?;
                if rule.min_human_approvals == 0 {
                    return None;
                }
                if surfaces.is_empty() || !surfaces.iter().any(|surface| surface_matches(surface, rule)) {
                    return Some(Violation::new(
                        FailureClass::UncoveredCell,
                        subject.as_str(),
                        format!("risk tier `{tier_name}` requires a current surface_ref/effect binding; self-declared safe or missing refs cannot establish that the configured rule does not apply"),
                    ));
                }

                let approvers: BTreeSet<&str> = current_records
                    .iter()
                    .filter_map(|record| record.approval.as_ref())
                    .filter(|approval| approval.verified && approval.role.as_str() == "human" && !approval.approver.trim().is_empty())
                    .map(|approval| approval.approver.as_str())
                    .collect();
                let observed = approvers.len();
                (observed < rule.min_human_approvals as usize).then(|| {
                    Violation::new(
                        FailureClass::UncoveredCell,
                        subject.as_str(),
                        format!(
                            "risk tier `{tier_name}` requires at least {} distinct verified human approval(s), observed {observed}",
                            rule.min_human_approvals
                        ),
                    )
                })
            })
            .collect()
    }
}

/// Select the current artifact generation with the same deterministic order
/// used by other evidence readers. Approval attestations are restricted to
/// records carrying this generation's `evidence_sha`; when the current record
/// cannot bind an artifact, the required-risk path fails closed.
fn latest_record<'a>(records: &[&'a EvidenceRecord]) -> Option<&'a EvidenceRecord> {
    records.iter().copied().max_by(|left, right| {
        let left_digest = content_digest12(&serde_json::to_value(left).unwrap_or_default());
        let right_digest = content_digest12(&serde_json::to_value(right).unwrap_or_default());
        (left.envelope.at, left.envelope.schema, left_digest).cmp(&(right.envelope.at, right.envelope.schema, right_digest))
    })
}


fn surface_matches(surface: &str, rule: &RiskTierRule) -> bool {
    if let Some(effect) = surface.strip_prefix("effect:") {
        return rule.effects.iter().any(|candidate| candidate == effect);
    }
    rule.paths.iter().any(|pattern| wildcard_matches(pattern, surface))
}

/// Deterministic glob matching where `*` matches any sequence, including `/`.
fn wildcard_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let mut p = 0;
    let mut v = 0;
    let mut star = None;
    let mut star_value = 0;

    while v < value.len() {
        if p < pattern.len() && pattern[p] == value[v] {
            p += 1;
            v += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            star_value = v;
        } else if let Some(star_position) = star {
            p = star_position + 1;
            star_value += 1;
            v = star_value;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use canon_model::{Actor, Envelope, EvidenceApproval, EvidenceRecord, EvidenceVerdict, RecordKind, RoleId, ScenarioId, Sha, TaskId};
    use chrono::{DateTime, Utc};
    use crate::context::GateCtx;
    use crate::policy::{PolicyField, PolicyResolution, StalenessPolicy};
    use super::*;

    fn now() -> DateTime<Utc> {
        "2026-10-01T00:00:00Z".parse().unwrap()
    }

    fn record(surface: &str, approver: Option<(&str, &str)>) -> EvidenceRecord {
        let mut record = EvidenceRecord::new(
            Envelope::new(1, RecordKind::EvidenceRecord, now(), Actor::new("agent", RoleId::parse("implementer").unwrap())),
            Some(TaskId::parse("change#1.1").unwrap()), None, None, EvidenceVerdict::Faithful,
        ).with_surface_ref(vec![surface.to_string()])
         .with_evidence_sha(Sha::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap());
        if let Some((approver, role)) = approver {
            record = record.with_approval(EvidenceApproval {
                approver: approver.to_string(),
                role: RoleId::parse(role).unwrap(),
                at: now(),
                verified: role == "human",
            });
        }
        record
    }

    fn context(evidence: Vec<EvidenceRecord>) -> GateContext {
        let risk_tiers = [
            ("high".to_string(), RiskTierRule { rank: 3, paths: vec!["src/auth/**".to_string()], effects: vec!["secret-access".to_string()], min_human_approvals: 1 }),
            ("very-high".to_string(), RiskTierRule { rank: 4, paths: vec!["deploy/**".to_string()], effects: vec!["production-deploy".to_string()], min_human_approvals: 2 }),
        ].into_iter().collect();
        GateContext {
            ctx: GateCtx { repo: "/tmp/repo".into(), ledger_root: "/tmp/repo/.canon/ledger".into() },
            policy: PolicyResolution {
                trust_required: BTreeMap::new(), trust_sample: BTreeMap::new(),
                staleness: StalenessPolicy { max_commits_behind: PolicyField::Flat(50), surface_scoped: PolicyField::Flat(true) },
                risk_routing: BTreeMap::new(), risk_tiers, spec_coverage: None, diagnostics: Vec::new(),
            },
            evidence, scenarios: Vec::new(), divergences: Vec::new(), subjects: Vec::new(),
            violations: Vec::new(), corpus_violations: Vec::new(), unreadable_kinds: Vec::new(), now: now(),
        }
    }

    #[test]
    fn absent_tiers_are_clean_but_unbound_refs_fail_closed() {
        let mut ctx = context(vec![record("src/auth/login.rs", None)]);
        ctx.policy.risk_tiers.clear();
        assert!(RiskApprovalCheck.run(&ctx).is_empty());
        assert!(RiskApprovalCheck.run(&context(Vec::new())).is_empty());
        let violations = RiskApprovalCheck.run(&context(vec![record("src/api/login.rs", None)]));
        assert_eq!(violations.len(), 1);
        assert!(violations[0].detail.contains("surface_ref/effect binding"));
    }
    #[test]
    fn malformed_risk_policy_is_a_gate_violation_not_an_empty_map() {
        let mut ctx = context(Vec::new());
        ctx.policy.diagnostics.push(PolicyDiagnostic::InvalidSection {
            section: "risk_tiers",
            detail: "tier `high` is invalid".to_string(),
        });
        let violations = RiskApprovalCheck.run(&ctx);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].class, FailureClass::UncoveredCell);
        assert_eq!(violations[0].subject, "risk_tiers");
        assert_eq!(violations[0].detail, "risk_tiers policy-invalid: tier `high` is invalid");
    }

    #[test]
    fn approval_from_an_old_artifact_generation_cannot_satisfy_current_one() {
        let old = record("src/auth/login.rs", Some(("alice", "human")));
        let mut current = record("src/auth/login.rs", None)
            .with_evidence_sha(Sha::parse("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap());
        current.envelope.at = now() + chrono::Duration::hours(1);
        let violations = RiskApprovalCheck.run(&context(vec![old, current]));
        assert_eq!(violations.len(), 1);
        assert!(violations[0].detail.contains("observed 0"));
    }


    #[test]
    fn high_risk_without_approval_has_stable_subject_and_detail() {
        let violations = RiskApprovalCheck.run(&context(vec![record("src/auth/deep/login.rs", None)]));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].class, FailureClass::UncoveredCell);
        assert_eq!(violations[0].subject, "change#1.1");
        assert_eq!(violations[0].detail, "risk tier `high` requires at least 1 distinct verified human approval(s), observed 0");
    }

    #[test]
    fn human_approval_counts_but_agent_and_empty_ids_do_not() {
        assert!(RiskApprovalCheck.run(&context(vec![record("src/auth/login.rs", Some(("alice", "human")))])).is_empty());
        for approval in [("bot", "agent"), ("alice", "human-reviewer"), ("", "human"), ("  ", "human")] {
            let violations = RiskApprovalCheck.run(&context(vec![record("src/auth/login.rs", Some(approval))]));
            assert_eq!(violations[0].detail, "risk tier `high` requires at least 1 distinct verified human approval(s), observed 0");
        }
    }

    #[test]
    fn highest_tier_requires_distinct_humans_across_the_subject_group() {
        let mut ctx = context(vec![
            record("src/auth/login.rs", Some(("alice", "human"))),
            record("deploy/prod/app.yaml", Some(("alice", "human"))),
        ]);
        let violations = RiskApprovalCheck.run(&ctx);
        assert_eq!(violations[0].detail, "risk tier `very-high` requires at least 2 distinct verified human approval(s), observed 1");
        ctx.evidence.reverse();
        assert_eq!(RiskApprovalCheck.run(&ctx), violations);
        ctx.evidence.push(record("deploy/prod/runbook.txt", Some(("bob", "human"))));
        assert!(RiskApprovalCheck.run(&ctx).is_empty());
    }

    #[test]
    fn explicit_effects_match_exactly_and_are_not_path_matches() {
        assert_eq!(RiskApprovalCheck.run(&context(vec![record("effect:secret-access", None)])).len(), 1);
        assert_eq!(RiskApprovalCheck.run(&context(vec![record("effect:secret-access-extra", None)])).len(), 1);
        let mut ctx = context(vec![record("effect:unknown", None)]);
        ctx.policy.risk_tiers.get_mut("high").unwrap().paths = vec!["*".to_string()];
        assert_eq!(RiskApprovalCheck.run(&ctx).len(), 1);
    }

    #[test]
    fn scenario_subject_and_approvals_stay_isolated_from_other_subjects() {
        let mut risky = record("effect:production-deploy", None);
        risky.task_id = None;
        risky.scenario_id = Some(ScenarioId::parse("auth.login.01").unwrap());
        let violations = RiskApprovalCheck.run(&context(vec![risky, record("docs/plain.txt", Some(("alice", "human")))]));
        assert_eq!(violations[0].subject, "change#1.1");
        assert_eq!(violations[0].detail, "risk tier `very-high` requires a current surface_ref/effect binding; self-declared safe or missing refs cannot establish that the configured rule does not apply");
    }

    #[test]
    fn wildcard_spans_separators_but_other_metacharacters_are_literal() {
        for (pattern, value, expected) in [
            ("src/auth/**", "src/auth/deep/login.rs", true),
            ("deploy/*", "deploy/prod/app.yaml", true),
            ("src/auth/**", "src/api/login.rs", false),
            ("*", "", true), ("a*b*c", "axybzc", true), ("a*b", "ac", false),
            ("a?b", "axb", false), ("a?b", "a?b", true), ("x*", "x", true),
        ] {
            assert_eq!(wildcard_matches(pattern, value), expected, "{pattern} {value}");
        }
    }
}
