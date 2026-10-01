//! Effect-aware risk-tier approval verification.
//!
//! This check is deliberately separate from `risk_routing`: routing derives
//! ordinary role cells, while these declarative rules select a highest-ranked
//! tier from evidence surface references and require accountable human
//! approval attestations. It uses the existing `uncovered-cell` failure class
//! so the gate's closed failure vocabulary remains unchanged.

use std::collections::{BTreeMap, BTreeSet};

use canon_model::EvidenceRecord;

use crate::context::{GateCheck, GateContext};
use crate::coverage::CellSubject;
use crate::failure_class::{FailureClass, Violation};
use crate::policy::RiskTierRule;

/// Verifies minimum distinct human approvals for the highest matching risk
/// tier in each existing evidence subject group.
pub struct RiskApprovalCheck;

impl GateCheck for RiskApprovalCheck {
    fn name(&self) -> &'static str {
        "risk-approval"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        if ctx.policy.risk_tiers.is_empty() {
            return Vec::new();
        }

        let mut groups: BTreeMap<CellSubject, Vec<&EvidenceRecord>> = BTreeMap::new();
        for record in &ctx.evidence {
            if let Some(subject) = CellSubject::of(record) {
                groups.entry(subject).or_default().push(record);
            }
        }

        groups
            .into_iter()
            .filter_map(|(subject, records)| {
                // Lexically smallest name breaks equal-rank ties, independent
                // of evidence order. Scan the policy map without a per-record
                // allocation or copying any surface references.
                let (tier_name, rule) = ctx.policy.risk_tiers.iter()
                    .filter(|(_, rule)| records.iter().any(|record| record.surface_ref.iter().any(|surface| surface_matches(surface, rule))))
                    .max_by(|(name_a, rule_a), (name_b, rule_b)| rule_a.rank.cmp(&rule_b.rank).then_with(|| name_b.cmp(name_a)))?;
                if rule.min_human_approvals == 0 {
                    return None;
                }

                let approvers: BTreeSet<&str> = records
                    .iter()
                    .filter_map(|record| record.approval.as_ref())
                    .filter(|approval| approval.role.as_str() == "human" && !approval.approver.trim().is_empty())
                    .map(|approval| approval.approver.as_str())
                    .collect();
                let observed = approvers.len();
                (observed < rule.min_human_approvals as usize).then(|| {
                    Violation::new(
                        FailureClass::UncoveredCell,
                        subject.as_str(),
                        format!(
                            "risk tier `{tier_name}` requires at least {} distinct human approval(s), observed {observed}",
                            rule.min_human_approvals
                        ),
                    )
                })
            })
            .collect()
    }
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

    use canon_model::{Actor, Envelope, EvidenceApproval, EvidenceVerdict, RecordKind, RoleId, ScenarioId, TaskId};
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
        ).with_surface_ref(vec![surface.to_string()]);
        if let Some((approver, role)) = approver {
            record = record.with_approval(EvidenceApproval { approver: approver.to_string(), role: RoleId::parse(role).unwrap(), at: now() });
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
    fn absent_tiers_empty_groups_and_nonmatching_refs_remain_clean() {
        let mut ctx = context(vec![record("src/auth/login.rs", None)]);
        ctx.policy.risk_tiers.clear();
        assert!(RiskApprovalCheck.run(&ctx).is_empty());
        assert!(RiskApprovalCheck.run(&context(Vec::new())).is_empty());
        assert!(RiskApprovalCheck.run(&context(vec![record("src/api/login.rs", None)])).is_empty());
    }

    #[test]
    fn high_risk_without_approval_has_stable_subject_and_detail() {
        let violations = RiskApprovalCheck.run(&context(vec![record("src/auth/deep/login.rs", None)]));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].class, FailureClass::UncoveredCell);
        assert_eq!(violations[0].subject, "change#1.1");
        assert_eq!(violations[0].detail, "risk tier `high` requires at least 1 distinct human approval(s), observed 0");
    }

    #[test]
    fn human_approval_counts_but_agent_and_empty_ids_do_not() {
        assert!(RiskApprovalCheck.run(&context(vec![record("src/auth/login.rs", Some(("alice", "human")))])).is_empty());
        for approval in [("bot", "agent"), ("alice", "human-reviewer"), ("", "human"), ("  ", "human")] {
            let violations = RiskApprovalCheck.run(&context(vec![record("src/auth/login.rs", Some(approval))]));
            assert_eq!(violations[0].detail, "risk tier `high` requires at least 1 distinct human approval(s), observed 0");
        }
    }

    #[test]
    fn highest_tier_requires_distinct_humans_across_the_subject_group() {
        let mut ctx = context(vec![
            record("src/auth/login.rs", Some(("alice", "human"))),
            record("deploy/prod/app.yaml", Some(("alice", "human"))),
        ]);
        let violations = RiskApprovalCheck.run(&ctx);
        assert_eq!(violations[0].detail, "risk tier `very-high` requires at least 2 distinct human approval(s), observed 1");
        ctx.evidence.reverse();
        assert_eq!(RiskApprovalCheck.run(&ctx), violations);
        ctx.evidence.push(record("deploy/prod/runbook.txt", Some(("bob", "human"))));
        assert!(RiskApprovalCheck.run(&ctx).is_empty());
    }

    #[test]
    fn explicit_effects_match_exactly_and_are_not_path_matches() {
        assert_eq!(RiskApprovalCheck.run(&context(vec![record("effect:secret-access", None)])).len(), 1);
        assert!(RiskApprovalCheck.run(&context(vec![record("effect:secret-access-extra", None)])).is_empty());
        let mut ctx = context(vec![record("effect:unknown", None)]);
        ctx.policy.risk_tiers.get_mut("high").unwrap().paths = vec!["*".to_string()];
        assert!(RiskApprovalCheck.run(&ctx).is_empty());
    }

    #[test]
    fn scenario_subject_and_approvals_stay_isolated_from_other_subjects() {
        let mut risky = record("effect:production-deploy", None);
        risky.task_id = None;
        risky.scenario_id = Some(ScenarioId::parse("auth.login.01").unwrap());
        let violations = RiskApprovalCheck.run(&context(vec![risky, record("docs/plain.txt", Some(("alice", "human")))]));
        assert_eq!(violations[0].subject, "auth.login.01");
        assert_eq!(violations[0].detail, "risk tier `very-high` requires at least 2 distinct human approval(s), observed 0");
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
