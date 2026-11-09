//! `spec_coverage.require_ci_run` (0.40): the latest declared evidence of a scenario must be a
//! CI record of the code as it stands now, from a run that ran enough cases.
//!
//! # What is judged
//! For every in-scope scenario ([`crate::policy::RequireCiRun::scope`], then `exclude_lanes`)
//! that has declared evidence, its newest declared record (the one with the latest `at`; an
//! audit is a separate fold and never the newest) must pass five questions, asked in this
//! order. The first that fails is the scenario's one violation, and its name is in the detail:
//!
//! | reason | the newest record ... | class |
//! |---|---|---|
//! | `no_ci_run` | has no `ci` block: a person or an agent recorded it | `uncovered-cell` |
//! | `code_tree_unknown` | cannot be compared: git did not give the code tree now | `uncovered-cell` |
//! | `other_code_tree` | is of a code tree other than the one now (none recorded counts as other) | `stale-evidence` |
//! | `dirty_worktree` | says its work tree differed from `HEAD` | `uncovered-cell` |
//! | `short_run` | ran fewer than `min_cases` cases (`passed + failed` over `ci.reports`) | `uncovered-cell` |
//!
//! A scenario with no declared evidence at all is not judged here: `require_evidence` says it.
//! A newer record by a person or an agent than the CI's makes the scenario `no_ci_run` until the
//! next CI record. The scenario's records come from the evidence index, so the cost is the
//! number of scenarios, not a scan of the ledger per scenario.
//!
//! # One function, three readers
//! [`scenario_violation`] is the only place a scenario is judged. `canon gate check`
//! ([`evaluate`], the tree [`GateContext::code_tree`] already holds), the `verifying → shipped`
//! guard and `canon status`' red flag ([`subject_violations`], the tree the command memoised)
//! all call it, so they cannot disagree. `--override-reason` does not waive it.

use canon_model::{EvidenceRecord, Scenario, SubjectId, SubjectStatus};

use crate::context::GateContext;
use crate::failure_class::{FailureClass, Violation};
use crate::freshness::{latest_evidence, scenario_evidence};
use crate::policy::{RequireCiRun, SpecCoverage};
use crate::spec_coverage::{latest_scenarios, latest_subjects, scope_decision, subject_scenarios, ScopeDecision};

/// The reason a scenario's newest evidence is not a CI record the policy accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gap {
    /// The newest record has no `ci` block.
    NoCiRun,
    /// The code tree now could not be read from git.
    CodeTreeUnknown,
    /// The newest record is of another code tree (`recorded` is `None` when it names none).
    OtherCodeTree { recorded: Option<String>, now: String },
    /// The newest record says its work tree differed from `HEAD`.
    DirtyWorktree,
    /// The run ran `ran` cases and the policy wants `min`.
    ShortRun { ran: u64, min: u64 },
}

impl Gap {
    /// The reason's name, as the violation's detail spells it.
    pub fn reason(&self) -> &'static str {
        match self {
            Gap::NoCiRun => "no_ci_run",
            Gap::CodeTreeUnknown => "code_tree_unknown",
            Gap::OtherCodeTree { .. } => "other_code_tree",
            Gap::DirtyWorktree => "dirty_worktree",
            Gap::ShortRun { .. } => "short_run",
        }
    }

    fn class(&self) -> FailureClass {
        match self {
            Gap::OtherCodeTree { .. } => FailureClass::StaleEvidence,
            _ => FailureClass::UncoveredCell,
        }
    }
}

/// The active `require_ci_run` settings and the `exclude_lanes` they apply with, or `None` when
/// the policy asks nothing of where evidence came from.
pub fn active_require_ci_run(ctx: &GateContext) -> Option<(&RequireCiRun, &[String])> {
    match &ctx.policy.spec_coverage {
        Some(SpecCoverage::Active { require_ci_run: Some(require_ci_run), exclude_lanes, .. }) => Some((require_ci_run, exclude_lanes.as_slice())),
        _ => None,
    }
}

/// The reason name (`no_ci_run`, `other_code_tree`, …) a violation `detail` of [`scenario_violation`] starts with,
/// `None` for any other violation.
pub fn reason_of(detail: &str) -> Option<&str> {
    detail.strip_prefix("spec_coverage.require_ci_run: ")?.split(' ').next()
}

/// What is wrong with `record`, a scenario's newest declared evidence, under `require`, as of
/// `code_tree` (the code tree now; `None` when git did not give it).
pub fn gap_of(record: &EvidenceRecord, require: &RequireCiRun, code_tree: Option<&str>) -> Option<Gap> {
    let Some(ci) = record.ci.as_ref() else { return Some(Gap::NoCiRun) };
    let Some(now) = code_tree else { return Some(Gap::CodeTreeUnknown) };
    if record.code_tree.as_deref() != Some(now) {
        return Some(Gap::OtherCodeTree { recorded: record.code_tree.clone(), now: now.to_string() });
    }
    if record.worktree_dirty == Some(true) {
        return Some(Gap::DirtyWorktree);
    }
    let ran = ci.cases_run();
    match require.min_cases {
        Some(min) if ran < min => Some(Gap::ShortRun { ran, min }),
        _ => None,
    }
}

/// The violation of `scenario`, when its newest declared evidence is not what `require` asks.
/// `None` for a scenario with no declared evidence.
pub fn scenario_violation(ctx: &GateContext, scenario: &Scenario, require: &RequireCiRun, code_tree: Option<&str>) -> Option<Violation> {
    let evidence = scenario_evidence(ctx, scenario);
    let latest = latest_evidence(&evidence)?;
    let gap = gap_of(latest, require, code_tree)?;
    let record = format!("recorded by `{}` at {}", latest.envelope.actor.agent_id, latest.envelope.at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    let record_command = format!(
        "run the whole suite in CI and record it there: `canon evidence add --project {} --all-scenarios --from-ci --ref \"<the suite command>\" --report <junit|cucumber|swift-testing>:<path> --role implementer`",
        scenario.project_id.as_str()
    );
    let detail = match &gap {
        Gap::NoCiRun => format!("spec_coverage.require_ci_run: no_ci_run — the newest evidence ({record}) was not recorded from CI; {record_command}"),
        Gap::CodeTreeUnknown => format!(
            "spec_coverage.require_ci_run: code_tree_unknown — git did not give the code tree now, so the newest evidence ({record}) cannot be told current; run canon inside the git work tree and check `git status` works there"
        ),
        Gap::OtherCodeTree { recorded, now } => {
            let url = latest.ci.as_ref().map_or("", |ci| ci.url.as_str());
            format!(
                "spec_coverage.require_ci_run: other_code_tree — the newest CI record ({record}, run {url}) is of code tree {}, and the code now is code tree {now}; {record_command}",
                recorded.as_deref().unwrap_or("(none recorded)")
            )
        }
        Gap::DirtyWorktree => {
            let url = latest.ci.as_ref().map_or("", |ci| ci.url.as_str());
            format!(
                "spec_coverage.require_ci_run: dirty_worktree — the newest CI record ({record}, run {url}) says its work tree differed from HEAD, so its code tree is not a commit's; {record_command} from a clean checkout"
            )
        }
        Gap::ShortRun { ran, min } => {
            let url = latest.ci.as_ref().map_or("", |ci| ci.url.as_str());
            format!(
                "spec_coverage.require_ci_run: short_run — the newest CI record ({record}, run {url}) ran {ran} case(s) and min_cases is {min}, so it is a partial run; {record_command} over every report of the suite"
            )
        }
    };
    Some(Violation::new(gap.class(), scenario.scenario_id.as_str(), detail))
}

/// The corpus-wide rule `canon gate check` applies: every in-scope scenario, as of the code tree
/// the context holds ([`GateContext::code_tree`]).
pub(crate) fn evaluate(ctx: &GateContext, require: &RequireCiRun, exclude_lanes: &[String]) -> Vec<Violation> {
    let status: std::collections::BTreeMap<&str, SubjectStatus> = latest_subjects(ctx).into_iter().map(|s| (s.subject_id.as_str(), s.status)).collect();
    latest_scenarios(ctx)
        .into_iter()
        .filter(|s| matches!(scope_decision(&require.scope, s.subject_id.as_ref(), &status), ScopeDecision::InScope))
        .filter(|s| !is_excluded(s, exclude_lanes))
        .filter_map(|scenario| scenario_violation(ctx, scenario, require, ctx.code_tree.as_deref()))
        .collect()
}

fn is_excluded(scenario: &Scenario, exclude_lanes: &[String]) -> bool {
    scenario.lane.as_ref().is_some_and(|lane| exclude_lanes.contains(lane))
}

/// The rule `canon subject move <id> --to shipped` applies and `canon status` reads as a shipped
/// subject's red: when `require_ci_run` covers `target`, every owned scenario's violation, as of
/// `code_tree`. The same [`scenario_violation`] the gate uses.
pub fn subject_violations(ctx: &GateContext, subject_id: &SubjectId, target: SubjectStatus, code_tree: impl FnOnce() -> Option<String>) -> Vec<Violation> {
    let Some((require, exclude_lanes)) = active_require_ci_run(ctx).filter(|(require, _)| require.covers(target)) else { return Vec::new() };
    let owned: Vec<&Scenario> = subject_scenarios(ctx, subject_id).into_iter().filter(|s| !is_excluded(s, exclude_lanes)).collect();
    if owned.is_empty() {
        return Vec::new();
    }
    let code_tree = code_tree();
    owned.into_iter().filter_map(|scenario| scenario_violation(ctx, scenario, require, code_tree.as_deref())).collect()
}

#[cfg(test)]
mod tests {
    use canon_model::{Actor, CiReport, CiRun, Envelope, EvidenceVerdict, RecordKind, ReportFormat, RoleId};
    use chrono::Utc;

    use super::*;

    fn record(ci: Option<CiRun>, code_tree: Option<&str>, dirty: Option<bool>) -> EvidenceRecord {
        let envelope = Envelope::new(1, RecordKind::Evidence, Utc::now(), Actor::new("agent", RoleId::parse("implementer").unwrap()));
        let mut record = EvidenceRecord::new(envelope, None, None, None, EvidenceVerdict::Faithful);
        record.ci = ci;
        record.code_tree = code_tree.map(str::to_string);
        record.worktree_dirty = dirty;
        record
    }

    fn run(passed: u64, failed: u64, skipped: u64) -> CiRun {
        CiRun {
            provider: "github-actions".into(),
            run: "1".into(),
            attempt: 1,
            url: "https://github.com/o/r/actions/runs/1/attempts/1".into(),
            repository: "o/r".into(),
            sha: "a".repeat(40),
            workflow: None,
            job: None,
            reports: vec![CiReport { path: "r.xml".into(), sha256: "b".repeat(64), format: ReportFormat::Junit, passed, failed, skipped }],
        }
    }

    fn require(min_cases: Option<u64>) -> RequireCiRun {
        RequireCiRun { scope: Vec::new(), min_cases }
    }

    #[test]
    fn a_record_without_a_ci_block_is_no_ci_run_whatever_else_holds() {
        assert_eq!(gap_of(&record(None, Some("t"), Some(false)), &require(None), Some("t")), Some(Gap::NoCiRun));
        assert_eq!(gap_of(&record(None, Some("t"), Some(false)), &require(None), None), Some(Gap::NoCiRun));
    }

    #[test]
    fn a_ci_record_of_the_code_now_and_a_complete_run_passes() {
        assert_eq!(gap_of(&record(Some(run(100, 0, 3)), Some("t"), Some(false)), &require(Some(100)), Some("t")), None);
        assert_eq!(gap_of(&record(Some(run(1, 0, 0)), Some("t"), Some(false)), &require(None), Some("t")), None);
    }

    #[test]
    fn the_cases_that_ran_are_passed_plus_failed_and_a_skipped_case_did_not_run() {
        let policy = require(Some(100));
        assert_eq!(gap_of(&record(Some(run(90, 9, 50)), Some("t"), Some(false)), &policy, Some("t")), Some(Gap::ShortRun { ran: 99, min: 100 }));
        assert_eq!(gap_of(&record(Some(run(90, 10, 0)), Some("t"), Some(false)), &policy, Some("t")), None);
    }

    #[test]
    fn another_code_tree_none_recorded_and_an_unknown_tree_now_each_have_their_own_reason() {
        let other = gap_of(&record(Some(run(1, 0, 0)), Some("old"), Some(false)), &require(None), Some("new"));
        assert_eq!(other, Some(Gap::OtherCodeTree { recorded: Some("old".into()), now: "new".into() }));
        let none = gap_of(&record(Some(run(1, 0, 0)), None, None), &require(None), Some("new"));
        assert_eq!(none, Some(Gap::OtherCodeTree { recorded: None, now: "new".into() }));
        assert_eq!(gap_of(&record(Some(run(1, 0, 0)), Some("t"), Some(false)), &require(None), None), Some(Gap::CodeTreeUnknown));
        assert_eq!(Gap::OtherCodeTree { recorded: None, now: String::new() }.class(), FailureClass::StaleEvidence);
        assert_eq!(Gap::CodeTreeUnknown.class(), FailureClass::UncoveredCell);
    }

    #[test]
    fn a_dirty_record_is_dirty_worktree_and_only_after_its_tree_matches() {
        assert_eq!(gap_of(&record(Some(run(1, 0, 0)), Some("t"), Some(true)), &require(None), Some("t")), Some(Gap::DirtyWorktree));
        assert_eq!(gap_of(&record(Some(run(1, 0, 0)), Some("old"), Some(true)), &require(None), Some("t")), Some(Gap::OtherCodeTree { recorded: Some("old".into()), now: "t".into() }));
    }

    #[test]
    fn the_reason_of_a_detail_is_the_name_scenario_violation_leads_with() {
        assert_eq!(reason_of("spec_coverage.require_ci_run: short_run — the newest CI record"), Some("short_run"));
        assert_eq!(reason_of("verifying → shipped: something else"), None);
    }
}
