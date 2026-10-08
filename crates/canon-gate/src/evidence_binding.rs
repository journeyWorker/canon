//! Experimental evidence binding (`policy.yaml`'s
//! `experimental.evidence_binding`, OFF by default).
//!
//! # Why
//! An `EvidenceRecord` is an attestation: `--ref` names a test and canon
//! never runs it. That keeps canon language-agnostic — it cannot run
//! Rust, COBOL, Flutter, embedded, and agent-driven QA alike — but it
//! also means a failure scenario can be "covered" by a happy-path test
//! nobody re-checks. Binding closes part of that gap without canon
//! executing anything: `canon evidence add --artifact/--report` reads
//! files the team's own runner (or agent) already produced and records
//! their sha256, and for a parsed JUnit/Cucumber report the matched case
//! and whether it passed. This check reads those attachments.
//!
//! # Strength, weakest first
//! - `attested` — no attachment (today's default form);
//! - `artifact` — at least one file bound by digest (a report, a trace,
//!   screenshots, an agent QA log);
//! - `report` — a parsed report whose matched case passed.
//!
//! # Modes
//! `off` (also the absent-section behavior) checks nothing. `warn` emits
//! no violation; [`binding_summary`] carries the gaps for `canon gate
//! check` to print as advisories. `require` reports each gap as
//! `uncovered-cell`. A present-but-broken section is a violation in
//! every mode — a typo must never read as "binding was never enabled".
//!
//! # What it reads
//! Each scenario's LATEST evidence record (by `at`, the same last-wins
//! rule the ledger verdict fold uses), among scenarios that HAVE evidence
//! — an unevidenced scenario is `spec_coverage`'s finding, not this
//! one's. `case`/`lane`/`scope` filters narrow which scenarios are held
//! to `strength`, so a team can require binding only where it matters
//! (e.g. failure cases of subjects about to ship).

use std::collections::BTreeMap;

use canon_model::{EvidenceRecord, ProjectId, ReportOutcome, Scenario, ScenarioId, SubjectStatus};

use crate::context::{GateCheck, GateContext};
use crate::failure_class::{FailureClass, Violation};
use crate::policy::{BindingMode, BindingStrength, EvidenceBinding};
use crate::spec_coverage::{latest_by_key, scope_decision, ScopeDecision};

/// The strength an evidence record reaches (module doc).
pub fn strength_of(record: &EvidenceRecord) -> BindingStrength {
    if record
        .attachments
        .iter()
        .any(|a| a.format.is_some() && a.outcome == Some(ReportOutcome::Passed))
    {
        BindingStrength::Report
    } else if !record.attachments.is_empty() {
        BindingStrength::Artifact
    } else {
        BindingStrength::Attested
    }
}

/// One filtered, evidenced scenario whose latest evidence is below the
/// policy's minimum strength.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingGap {
    pub scenario_id: ScenarioId,
    pub actual: BindingStrength,
    pub required: BindingStrength,
}

impl BindingGap {
    pub fn detail(&self) -> String {
        format!(
            "latest evidence is {} but experimental evidence binding requires {} — bind it with `canon evidence add --artifact <path>` or `--report <junit|cucumber>:<path>`",
            self.actual.as_str(),
            self.required.as_str()
        )
    }
}

/// What the binding policy saw over its filtered scenario set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingSummary {
    pub mode: BindingMode,
    pub required: BindingStrength,
    /// Filtered, evidenced scenarios per strength actually reached.
    pub counts: BTreeMap<BindingStrength, usize>,
    pub gaps: Vec<BindingGap>,
}

/// Evaluate an active binding policy. `None` when the section is absent,
/// `off`, or invalid (the check itself reports an invalid section).
pub fn binding_summary(ctx: &GateContext) -> Option<BindingSummary> {
    let Some(EvidenceBinding::Active {
        mode,
        strength,
        cases,
        lanes,
        scope,
    }) = &ctx.policy.evidence_binding
    else {
        return None;
    };
    if *mode == BindingMode::Off {
        return None;
    }

    let subject_status: BTreeMap<&str, SubjectStatus> =
        latest_by_key(&ctx.subjects, |s| s.subject_id.as_str().to_string())
            .into_iter()
            .map(|s| (s.subject_id.as_str(), s.status))
            .collect();

    let mut latest_evidence: BTreeMap<(ProjectId, ScenarioId), &EvidenceRecord> = BTreeMap::new();
    for record in &ctx.evidence {
        let (Some(project_id), Some(scenario_id)) =
            (record.project_id.clone(), record.scenario_id.clone())
        else {
            continue;
        };
        let slot = latest_evidence
            .entry((project_id, scenario_id))
            .or_insert(record);
        if record.envelope.at > slot.envelope.at {
            *slot = record;
        }
    }

    let held = |scenario: &Scenario| {
        (cases.is_empty() || scenario.case.as_ref().is_some_and(|c| cases.contains(c)))
            && (lanes.is_empty() || scenario.lane.as_ref().is_some_and(|l| lanes.contains(l)))
            && matches!(
                scope_decision(scope, scenario.subject_id.as_ref(), &subject_status),
                ScopeDecision::InScope
            )
    };

    let mut counts = BTreeMap::new();
    let mut gaps = Vec::new();
    for scenario in latest_by_key(&ctx.scenarios, |s| {
        (s.project_id.clone(), s.scenario_id.clone())
    }) {
        if !held(scenario) {
            continue;
        }
        let Some(record) =
            latest_evidence.get(&(scenario.project_id.clone(), scenario.scenario_id.clone()))
        else {
            continue;
        };
        let actual = strength_of(record);
        *counts.entry(actual).or_insert(0) += 1;
        if actual < *strength {
            gaps.push(BindingGap {
                scenario_id: scenario.scenario_id.clone(),
                actual,
                required: *strength,
            });
        }
    }
    gaps.sort_by(|a, b| a.scenario_id.cmp(&b.scenario_id));
    Some(BindingSummary {
        mode: *mode,
        required: *strength,
        counts,
        gaps,
    })
}

/// The `canon gate check` arm (module doc).
pub struct EvidenceBindingCheck;

impl GateCheck for EvidenceBindingCheck {
    fn name(&self) -> &'static str {
        "evidence-binding"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        if let Some(EvidenceBinding::Invalid { detail }) = &ctx.policy.evidence_binding {
            return vec![Violation::new(
                FailureClass::UncoveredCell,
                "experimental.evidence_binding",
                format!("`experimental.evidence_binding` is present but invalid ({detail}); fix it or remove it — a broken section never reads as off"),
            )];
        }
        let Some(summary) = binding_summary(ctx) else {
            return Vec::new();
        };
        if summary.mode != BindingMode::Require {
            return Vec::new();
        }
        summary
            .gaps
            .iter()
            .map(|gap| {
                Violation::new(
                    FailureClass::UncoveredCell,
                    gap.scenario_id.as_str(),
                    gap.detail(),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{PolicyField, PolicyResolution, StalenessPolicy};
    use canon_model::{
        Actor, Envelope, EvidenceAttachment, EvidenceVerdict, RecordKind, ReportFormat, RoleId,
        SpecDigest, Subject, SubjectId,
    };
    use chrono::{Duration, Utc};

    fn project() -> ProjectId {
        ProjectId::parse("p").unwrap()
    }

    fn scenario(id: &str, case: Option<&str>) -> Scenario {
        let envelope = Envelope::new(
            1,
            RecordKind::Scenario,
            Utc::now(),
            Actor::new("canon", RoleId::parse("implementer").unwrap()),
        );
        let mut s = Scenario::new(
            envelope,
            project(),
            ScenarioId::parse(id).unwrap(),
            "t",
            "",
            SpecDigest::of(id.as_bytes()),
        );
        s.case = case.map(str::to_string);
        s
    }

    fn evidence(id: &str, attachment: Option<EvidenceAttachment>, offset: i64) -> EvidenceRecord {
        let envelope = Envelope::new(
            1,
            RecordKind::EvidenceRecord,
            Utc::now() + Duration::seconds(offset),
            Actor::new("agent", RoleId::parse("implementer").unwrap()),
        );
        EvidenceRecord::new(
            envelope,
            None,
            Some(ScenarioId::parse(id).unwrap()),
            None,
            EvidenceVerdict::Faithful,
        )
        .with_project_id(project())
        .with_attachments(attachment.into_iter().collect())
    }

    fn artifact() -> EvidenceAttachment {
        EvidenceAttachment {
            path: "trace.zip".into(),
            sha256: "a".repeat(64),
            format: None,
            case: None,
            outcome: None,
        }
    }

    fn report(outcome: ReportOutcome) -> EvidenceAttachment {
        EvidenceAttachment {
            path: "junit.xml".into(),
            sha256: "b".repeat(64),
            format: Some(ReportFormat::Junit),
            case: Some("c".into()),
            outcome: Some(outcome),
        }
    }

    fn binding(
        mode: BindingMode,
        strength: BindingStrength,
        cases: &[&str],
    ) -> Option<EvidenceBinding> {
        Some(EvidenceBinding::Active {
            mode,
            strength,
            cases: cases.iter().map(|s| s.to_string()).collect(),
            lanes: Vec::new(),
            scope: Vec::new(),
        })
    }

    fn ctx(
        binding: Option<EvidenceBinding>,
        scenarios: Vec<Scenario>,
        evidence: Vec<EvidenceRecord>,
        subjects: Vec<Subject>,
    ) -> GateContext {
        GateContext {
            ctx: crate::context::GateCtx {
                repo: "/tmp/repo".into(),
                ledger_root: "/tmp/repo/.canon/ledger".into(),
            },
            policy: PolicyResolution {
                trust_required: BTreeMap::new(),
                trust_sample: BTreeMap::new(),
                staleness: StalenessPolicy {
                    max_commits_behind: PolicyField::Flat(50),
                    surface_scoped: PolicyField::Flat(true),
                },
                risk_routing: BTreeMap::new(),
                risk_tiers: BTreeMap::new(),
                spec_coverage: None,
                evidence_binding: binding,
                diagnostics: Vec::new(),
            },
            evidence,
            scenarios,
            divergences: Vec::new(),
            subjects,
            violations: Vec::new(),
            corpus_violations: Vec::new(),
            unreadable_kinds: Vec::new(),
            now: Utc::now(),
        }
    }

    #[test]
    fn strength_orders_attested_below_artifact_below_a_passing_report() {
        assert_eq!(
            strength_of(&evidence("a.b.01", None, 0)),
            BindingStrength::Attested
        );
        assert_eq!(
            strength_of(&evidence("a.b.01", Some(artifact()), 0)),
            BindingStrength::Artifact
        );
        assert_eq!(
            strength_of(&evidence("a.b.01", Some(report(ReportOutcome::Passed)), 0)),
            BindingStrength::Report
        );
        // A report whose case failed or was skipped binds the file, but
        // proves nothing passed: it is an artifact, never `report`.
        assert_eq!(
            strength_of(&evidence("a.b.01", Some(report(ReportOutcome::Failed)), 0)),
            BindingStrength::Artifact
        );
        assert_eq!(
            strength_of(&evidence("a.b.01", Some(report(ReportOutcome::Skipped)), 0)),
            BindingStrength::Artifact
        );
    }

    /// Off (and absent) is the default and checks nothing; warn reports
    /// the gap without a violation; require turns it into one.
    #[test]
    fn off_and_warn_never_fail_the_gate_and_require_does() {
        let scenarios = vec![scenario("a.b.01", Some("failure"))];
        let evidence = vec![evidence("a.b.01", None, 0)];

        for absent_or_off in [
            None,
            binding(BindingMode::Off, BindingStrength::Artifact, &[]),
        ] {
            let c = ctx(
                absent_or_off,
                scenarios.clone(),
                evidence.clone(),
                Vec::new(),
            );
            assert!(binding_summary(&c).is_none());
            assert!(EvidenceBindingCheck.run(&c).is_empty());
        }

        let warn = ctx(
            binding(BindingMode::Warn, BindingStrength::Artifact, &[]),
            scenarios.clone(),
            evidence.clone(),
            Vec::new(),
        );
        assert!(
            EvidenceBindingCheck.run(&warn).is_empty(),
            "warn must never fail the gate"
        );
        assert_eq!(binding_summary(&warn).unwrap().gaps.len(), 1);

        let require = ctx(
            binding(BindingMode::Require, BindingStrength::Artifact, &[]),
            scenarios,
            evidence,
            Vec::new(),
        );
        let violations = EvidenceBindingCheck.run(&require);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].subject, "a.b.01");
        assert!(
            violations[0].detail.contains("attested") && violations[0].detail.contains("artifact"),
            "{}",
            violations[0].detail
        );
    }

    /// The latest record decides: binding an older attestation does not
    /// carry over to a newer unbound one, and vice versa.
    #[test]
    fn the_latest_evidence_record_decides_the_strength() {
        let scenarios = vec![scenario("a.b.01", None)];
        let newer_unbound = vec![
            evidence("a.b.01", Some(artifact()), 0),
            evidence("a.b.01", None, 5),
        ];
        let c = ctx(
            binding(BindingMode::Require, BindingStrength::Artifact, &[]),
            scenarios.clone(),
            newer_unbound,
            Vec::new(),
        );
        assert_eq!(EvidenceBindingCheck.run(&c).len(), 1);

        let newer_bound = vec![
            evidence("a.b.01", None, 0),
            evidence("a.b.01", Some(artifact()), 5),
        ];
        let c = ctx(
            binding(BindingMode::Require, BindingStrength::Artifact, &[]),
            scenarios,
            newer_bound,
            Vec::new(),
        );
        assert!(EvidenceBindingCheck.run(&c).is_empty());
    }

    /// Filters narrow who is held: only `@case:failure` here, so an
    /// unbound happy scenario passes; `strength: report` refuses an
    /// artifact-only failure scenario; unevidenced scenarios are left to
    /// `spec_coverage`.
    #[test]
    fn filters_and_strength_select_which_scenarios_must_bind() {
        let scenarios = vec![
            scenario("a.b.01", Some("happy")),
            scenario("a.b.02", Some("failure")),
            scenario("a.b.03", Some("failure")),
        ];
        let evidence = vec![
            evidence("a.b.01", None, 0),
            evidence("a.b.02", Some(artifact()), 0),
        ];
        let c = ctx(
            binding(BindingMode::Require, BindingStrength::Report, &["failure"]),
            scenarios,
            evidence,
            Vec::new(),
        );
        let summary = binding_summary(&c).unwrap();
        assert_eq!(summary.counts.get(&BindingStrength::Artifact), Some(&1));
        assert_eq!(
            summary
                .gaps
                .iter()
                .map(|g| g.scenario_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a.b.02"]
        );
    }

    /// `scope` holds only scenarios whose subject is in a listed status.
    #[test]
    fn scope_limits_binding_to_subjects_in_the_listed_status() {
        let mut shipping = scenario("a.b.01", None);
        shipping.subject_id = Some(SubjectId::parse("ship-me").unwrap());
        let mut building = scenario("a.b.02", None);
        building.subject_id = Some(SubjectId::parse("wip").unwrap());
        let subject = |id: &str, status| {
            Subject::new(
                Envelope::new(
                    1,
                    RecordKind::Subject,
                    Utc::now(),
                    Actor::new("canon", RoleId::parse("implementer").unwrap()),
                ),
                SubjectId::parse(id).unwrap(),
                "t",
                "s",
                "dev",
                status,
                RoleId::parse("implementer").unwrap(),
            )
        };
        let policy = Some(EvidenceBinding::Active {
            mode: BindingMode::Require,
            strength: BindingStrength::Artifact,
            cases: Vec::new(),
            lanes: Vec::new(),
            scope: vec![SubjectStatus::Verifying],
        });
        let c = ctx(
            policy,
            vec![shipping, building],
            vec![evidence("a.b.01", None, 0), evidence("a.b.02", None, 0)],
            vec![
                subject("ship-me", SubjectStatus::Verifying),
                subject("wip", SubjectStatus::Building),
            ],
        );
        let violations = EvidenceBindingCheck.run(&c);
        assert_eq!(
            violations
                .iter()
                .map(|v| v.subject.as_str())
                .collect::<Vec<_>>(),
            vec!["a.b.01"]
        );
    }

    #[test]
    fn an_invalid_section_is_a_violation_even_though_the_feature_is_experimental() {
        let c = ctx(
            Some(EvidenceBinding::Invalid {
                detail: "bad mode".into(),
            }),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let violations = EvidenceBindingCheck.run(&c);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].detail.contains("bad mode"));
    }
}
