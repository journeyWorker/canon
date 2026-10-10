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
use std::path::Path;
use std::process::Command;

use canon_model::{approval_payload_bytes, EvidenceApproval, EvidenceRecord, EvidenceVerdict};
use canon_store::partition::content_digest12;

use crate::approval::verify_risk_approval;
use crate::context::{GateCheck, GateContext};
use crate::coverage::CellSubject;
use crate::failure_class::{FailureClass, Violation};
use crate::policy::{allowed_signers_path, PolicyDiagnostic, RiskTierRule};
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
            PolicyDiagnostic::Malformed { detail, .. } => Some(detail.as_str()),
            _ => None,
        }) {
            return vec![Violation::new(FailureClass::UncoveredCell, "risk_tiers", format!("risk policy-invalid: {detail}"))];
        }
        if ctx.policy.risk_tiers.is_empty() {
            return Vec::new();
        }
        let allowed_signers = match allowed_signers_path(&ctx.ctx.repo) {
            Ok(path) => path,
            Err(detail) => {
                return vec![Violation::new(FailureClass::UncoveredCell, "risk_tiers", format!("approval verifier policy-invalid: {detail}"))];
            }
        };

        let stale_subjects: BTreeSet<String> = StalenessCheck
            .run(ctx)
            .into_iter()
            .filter(|violation| violation.class == FailureClass::StaleEvidence)
            .map(|violation| violation.subject)
            .collect();
        let mut groups: BTreeMap<(CellSubject, Option<String>), Vec<&EvidenceRecord>> = BTreeMap::new();
        for record in &ctx.evidence {
            for subject in CellSubject::all_of(record) {
                groups.entry((subject, record.project_id.as_ref().map(ToString::to_string))).or_default().push(record);
            }
        }

        groups
            .into_iter()
            .filter_map(|((subject, _project), records)| {
                let current = latest_record(&records)?;
                let current_records: Vec<&EvidenceRecord> = match current.evidence_sha.as_ref() {
                    Some(current_sha) => records
                        .iter()
                        .copied()
                        .filter(|record| !stale_subjects.contains(subject.as_str())
                            && record.evidence_sha.as_ref() == Some(current_sha)
                            && record.project_id == current.project_id
                            && record.run_id == current.run_id
                            && record.verdict == EvidenceVerdict::Faithful)
                        .collect(),
                    None => Vec::new(),
                };
                let analysis = current.evidence_sha.as_ref()
                    .ok_or_else(|| "missing artifact Git SHA".to_string())
                    .and_then(|sha| artifact_changed_paths(&ctx.ctx.repo, sha));
                let analysis_known = analysis.is_ok();
                let mut derived_surfaces = analysis.unwrap_or_default();
                // Effects are supplemental; self-declared paths cannot hide
                // security-sensitive changes present in the verified commit.
                derived_surfaces.extend(current.surface_ref.iter().filter(|surface| surface.starts_with("effect:")).cloned());
                derived_surfaces.sort_unstable();
                derived_surfaces.dedup();
                let surfaces: BTreeSet<&str> = derived_surfaces.iter().map(String::as_str).collect();
                // Known, unaffected surfaces are clean. Only an unknown binding
                // (empty surfaces) falls back to the highest required tier.
                let selected = ctx.policy.risk_tiers.iter()
                    .filter(|(_, rule)| surfaces.iter().any(|surface| surface_matches(surface, rule)))
                    .max_by(|(name_a, rule_a), (name_b, rule_b)| rule_a.rank.cmp(&rule_b.rank).then_with(|| name_b.cmp(name_a)));
                let (tier_name, rule) = match selected {
                    Some(selected) => selected,
                    None if !analysis_known => ctx.policy.risk_tiers.iter()
                        .filter(|(_, rule)| rule.min_human_approvals > 0)
                        .max_by(|(name_a, rule_a), (name_b, rule_b)| rule_a.rank.cmp(&rule_b.rank).then_with(|| name_b.cmp(name_a)))?,
                    None => return None,
                };
                if rule.min_human_approvals == 0 {
                    return None;
                }
                if !analysis_known {
                    return Some(Violation::new(FailureClass::UncoveredCell, subject.as_str(), format!("risk tier `{tier_name}` requires a resolvable artifact Git SHA; unknown analysis cannot establish safety")));
                }
                if current.verdict != EvidenceVerdict::Faithful {
                    return Some(Violation::new(FailureClass::UncoveredCell, subject.as_str(), format!("risk tier `{tier_name}` has no current faithful verdict; old approvals cannot override divergence")));
                }
                let Some(current_sha) = current.evidence_sha.as_ref() else {
                    return Some(Violation::new(FailureClass::UncoveredCell, subject.as_str(), format!("risk tier `{tier_name}` requires a resolvable artifact Git SHA")));
                };
                let Some(allowed_signers) = allowed_signers.as_deref() else {
                    return Some(Violation::new(FailureClass::UncoveredCell, subject.as_str(), format!("risk tier `{tier_name}` requires authenticated human approvals, but no allowed_signers verifier is configured")));
                };
                let authenticated: BTreeSet<&str> = current_records
                    .iter()
                    .filter_map(|record| record.approval.as_ref().and_then(|approval| {
                        approval_is_authenticated(approval, &subject, current, current_sha, &derived_surfaces, allowed_signers)
                            .then_some(approval.approver.as_str())
                    }))
                    .collect();

                let observed = authenticated.len();
                (observed < rule.min_human_approvals as usize).then(|| Violation::new(
                    FailureClass::UncoveredCell,
                    subject.as_str(),
                    format!("risk tier `{tier_name}` requires at least {} distinct authenticated human approval(s), observed {observed}", rule.min_human_approvals),
                ))
            })
            .collect()
    }
}
pub fn artifact_changed_paths(repo: &Path, sha: &canon_model::Sha) -> Result<Vec<String>, String> {
    let sha = sha.to_string();
    let resolved = Command::new("git")
        .arg("-C").arg(repo)
        .args(["rev-parse", "--verify", &format!("{sha}^{{commit}}")])
        .output()
        .map_err(|error| format!("resolve artifact SHA: {error}"))?;
    if !resolved.status.success() {
        return Err(format!("artifact SHA is not a commit: {sha}"));
    }
    let output = Command::new("git")
        .arg("-C").arg(repo)
        .args(["diff-tree", "--root", "--no-commit-id", "-r", "-m", "--no-renames", "--name-only", &sha])
        .output()
        .map_err(|error| format!("derive artifact paths: {error}"))?;
    if !output.status.success() {
        return Err(format!("cannot derive changed paths for artifact SHA: {sha}"));
    }
    let text = String::from_utf8(output.stdout).map_err(|error| format!("artifact path output is not UTF-8: {error}"))?;
    let mut paths: Vec<String> = text.lines().map(str::trim).filter(|path| !path.is_empty()).map(str::to_string).collect();
    paths.sort_unstable();
    paths.dedup();
    Ok(paths)
}

fn approval_is_authenticated(
    approval: &EvidenceApproval,
    subject: &CellSubject,
    current: &EvidenceRecord,
    current_sha: &canon_model::Sha,
    derived_surfaces: &[String],
    allowed_signers: &Path,
) -> bool {
    if approval.role.as_str() != "human"
        || approval.approver.trim().is_empty()
        || approval.signature.is_none()
        || approval.subject.as_deref() != Some(subject.as_str())
        || approval.artifact_sha.as_ref() != Some(current_sha)
        || approval.project_id != current.project_id
        || approval.run_id != current.run_id
        || normalized(&approval.surface) != normalized(derived_surfaces)
        || normalized(&approval.effects) != normalized(&effects_for(derived_surfaces))
    {
        return false;
    }
    let artifact_sha = current_sha.to_string();
    let project = current.project_id.as_ref().map(ToString::to_string);
    let run_id = current.run_id.as_ref().map(ToString::to_string);
    let payload = approval_payload_bytes(
        canon_model::APPROVAL_NAMESPACE,
        subject.as_str(),
        project.as_deref(),
        &artifact_sha,
        run_id.as_deref(),
        &approval.surface,
        &approval.effects,
        &approval.approver,
        &approval.at,
    );
    verify_risk_approval(&payload, approval.signature.as_deref().unwrap_or_default().as_bytes(), &approval.approver, allowed_signers).is_ok()
}

fn normalized(values: &[String]) -> Vec<&str> {
    let mut values: Vec<&str> = values.iter().map(String::as_str).collect();
    values.sort_unstable();
    values.dedup();
    values
}

fn effects_for(surface: &[String]) -> Vec<String> {
    let mut effects: Vec<String> = surface.iter().filter_map(|surface| surface.strip_prefix("effect:").map(str::to_string)).collect();
    effects.sort_unstable();
    effects.dedup();
    effects
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
                signature: None,
                subject: None,
                project_id: None,
                artifact_sha: None,
                run_id: None,
                surface: Vec::new(),
                effects: Vec::new(),
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
                risk_routing: BTreeMap::new(), risk_tiers, spec_coverage: None, evidence_binding: None, diagnostics: Vec::new(),
            },
            evidence, scenarios: Vec::new(), divergences: Vec::new(), subjects: Vec::new(), reviews: Vec::new(), findings: Vec::new(),
            violations: Vec::new(), corpus_violations: Vec::new(), unreadable_kinds: Vec::new(), now: now(),
        }
    }

    #[test]
    fn absent_tiers_are_clean() {
        let mut ctx = context(vec![record("src/auth/login.rs", None)]);
        ctx.policy.risk_tiers.clear();
        assert!(RiskApprovalCheck.run(&ctx).is_empty());
        assert!(RiskApprovalCheck.run(&context(Vec::new())).is_empty());
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
        assert_eq!(violations[0].detail, "risk policy-invalid: tier `high` is invalid");
    }

    #[test]
    fn unsigned_and_legacy_verified_approvals_never_count() {
        let violations = RiskApprovalCheck.run(&context(vec![record("src/auth/login.rs", Some(("alice", "human")))]));
        assert_eq!(violations.len(), 1);
        assert!(violations[0].detail.contains("resolvable artifact Git SHA") || violations[0].detail.contains("observed 0"));
    }

    #[test]
    fn task_and_scenario_subjects_are_separate_groups() {
        let mut scenario = record("effect:production-deploy", None);
        scenario.task_id = None;
        scenario.scenario_id = Some(ScenarioId::parse("auth.login.01").unwrap());
        let violations = RiskApprovalCheck.run(&context(vec![scenario, record("docs/plain.txt", Some(("alice", "human")))]));
        assert!(violations.iter().any(|violation| violation.subject == "auth.login.01") || violations.iter().any(|violation| violation.subject == "change#1.1"));
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
