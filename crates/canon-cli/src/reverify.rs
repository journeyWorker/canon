//! Re-verification before shipping (0.22 D2, #72, #77): one freshness
//! rule, [`canon_gate::review_gate::scenarios_without_evidence_after`],
//! with two triggers. A subject cannot ship while one of its evidenced
//! scenarios has no evidence recorded after a trigger:
//!
//! - a blocker finding closed `fixed` (trigger: when it was closed `fixed`); that
//!   half lives in the gate's `require_review` rule
//!   ([`canon_gate::review_gate::fixed_blockers`]) and keeps blocking
//!   under `block_on_findings`;
//! - an approved chore change on the subject (trigger: its approval time,
//!   [`crate::brief::approved_at`]); this half needs the change's
//!   `brief.md`, so it is read here, from the repo.
//!
//! A chore approval asks only where the code changed (0.28 D11, #116): a
//! scenario whose latest evidence was recorded on the current code tree
//! (`git_rev::code_tree`) needs no new evidence, so a docs-only chore needs
//! its change-level review alone.
//!
//! New evidence stales a pinned review, so the re-review follows with no
//! rule of its own. Every refusal names the scenarios and leads with ONE batch
//! command, [`canon_gate::review_gate::batch_commands`]: `canon evidence add
//! --all-scenarios --report junit:a.xml --report junit:b.xml --role
//! implementer`, built from the report paths bound to the subject's latest
//! evidence.

use std::path::Path;

use canon_gate::freshness::scenarios_without_evidence_after_on_code;
use canon_gate::review_gate::{
    active_require_review, batch_commands, change_review_gap, fixed_blockers, is_reverify, is_scoped_reverify, reverify_remedy, reverify_scenarios, scoped_reverify_remedy,
};
use canon_gate::spec_coverage::subject_change_ids;
use canon_gate::{FailureClass, GateContext, Violation};
use canon_model::{ChangeId, SpecDigest, Subject, SubjectStatus};
use chrono::{DateTime, SecondsFormat, Utc};

/// What a re-verification gap waits on.
#[derive(Debug, Clone)]
pub struct Gap {
    /// `blocker finding <change>#<round>.<seq> closed fixed` or
    /// `chore change <slug> approved`.
    pub trigger: String,
    pub since: DateTime<Utc>,
    /// The `(project_id, scenario_id)` pairs with no evidence after `since`.
    pub scenarios: Vec<(String, String)>,
    /// The finding names its scenarios (0.24.1, #87), so only `scenarios`
    /// need evidence, one `canon evidence add --scenario-id` each; the
    /// rest of the suite is not asked for.
    pub scoped: bool,
    /// The batch command(s) that clear a whole-suite gap: one `canon evidence
    /// add --all-scenarios --report …` (one per spec root when the repo has
    /// several), built from the reports the scenarios' latest evidence bound.
    /// Empty when that evidence bound no report, and for a scoped gap.
    pub commands: Vec<String>,
    /// The chore change whose approval triggered the gap, when it is a chore.
    pub chore: Option<ChangeId>,
    remedy: String,
}

impl Gap {
    fn pairs(&self) -> Vec<(&str, &str)> {
        self.scenarios.iter().map(|(project, id)| (project.as_str(), id.as_str())).collect()
    }

    /// The gap of `scenarios` after `trigger`, with its commands built from `ctx`.
    fn new(repo: &Path, ctx: &GateContext, trigger: String, since: DateTime<Utc>, scenarios: Vec<(String, String)>, scoped: bool, chore: Option<ChangeId>) -> Self {
        let pairs: Vec<(&str, &str)> = scenarios.iter().map(|(project, id)| (project.as_str(), id.as_str())).collect();
        let multi_root = multi_root(repo);
        let (commands, remedy) = if scoped {
            (Vec::new(), scoped_reverify_remedy(&pairs))
        } else {
            (batch_commands(ctx, &pairs, multi_root), reverify_remedy(ctx, &pairs, multi_root))
        };
        Gap { trigger, since, scenarios, scoped, commands, chore, remedy }
    }

    /// The `canon evidence add` step this gap names: the batch for the
    /// whole suite, the per-scenario command for a scoped finding.
    pub fn remedy(&self) -> &str {
        &self.remedy
    }

    /// The `canon status` steps for this gap: the batch command(s) for the
    /// whole suite; for a scoped finding, the first scenario's own command
    /// (`evidence add` takes one scenario; the next follows once it is
    /// evidenced).
    pub fn commands(&self) -> Vec<String> {
        if let Some((project, id)) = self.scoped.then(|| self.scenarios.first()).flatten() {
            return vec![format!("canon evidence add --project-id {project} --scenario-id {id} --report <junit|cucumber|swift-testing>:<path> --role implementer")];
        }
        if self.commands.is_empty() {
            return vec![format!("canon evidence add --project-id {} --all-scenarios --report <junit|cucumber|swift-testing>:<path> --role implementer", self.project())];
        }
        self.commands.clone()
    }

    /// `N evidenced scenario(s) (a, b, c, … k more)`.
    pub fn scenario_list(&self) -> String {
        reverify_scenarios(&self.pairs())
    }

    /// The project to pass to `--project-id`.
    pub fn project(&self) -> &str {
        self.scenarios.first().map_or("<root>", |(project, _)| project.as_str())
    }

    /// The step's text for `subject` in `canon status`.
    pub fn why(&self, subject: &str) -> String {
        format!(
            "{subject}: {} (since {}) and {} evidenced scenario(s) ({}) have no evidence recorded after it; the ship guard refuses until each does. {}; a review stays current while the scenario's verdict and checks are unchanged",
            self.trigger,
            self.since.to_rfc3339_opts(SecondsFormat::Secs, true),
            self.scenarios.len(),
            self.scenario_list(),
            if self.scoped { "The finding names these scenarios, so evidence for each of them alone is enough" } else { "Run the whole suite again; a scenario last evidenced on the code as it stands needs nothing" }
        )
    }
}

/// The approved chore changes among `subject`'s adopted changes, each with
/// its approval time and the digest of its brief as it reads now. A chore
/// whose brief is not approved (or whose approval line carries no readable
/// time) triggers nothing.
fn chore_approvals(repo: &Path, ctx: &GateContext, subject: &Subject) -> Vec<(ChangeId, DateTime<Utc>, SpecDigest)> {
    subject_change_ids(ctx, subject)
        .into_iter()
        .filter_map(|change| {
            let text = std::fs::read_to_string(crate::brief::canon_brief(repo, &change)?).ok()?;
            if !crate::brief_check::is_chore(&text) || !crate::brief::approval_of(&text).approved() {
                return None;
            }
            let at = crate::brief::approved_at(canon_ingest::markdown::field(&text, "Approved:")?)?;
            Some((change, at, crate::brief::brief_spec_digest(&text)))
        })
        .collect()
}

/// Every unmet chore trigger of `subject`.
pub fn chore_gaps(repo: &Path, ctx: &GateContext, subject: &Subject) -> Vec<Gap> {
    let approvals = chore_approvals(repo, ctx, subject);
    if approvals.is_empty() {
        return Vec::new();
    }
    // The code as it is now (D11): a scenario last evidenced on it needs nothing.
    // No git work tree means nothing to compare, and the time rule stands.
    let code_tree = crate::git_rev::code_tree(repo, None).ok().map(|c| c.tree);
    approvals
        .into_iter()
        .filter_map(|(change, since, _)| {
            let scenarios: Vec<(String, String)> = scenarios_without_evidence_after_on_code(ctx, &subject.subject_id, since, code_tree.as_deref())
                .into_iter()
                .map(|s| (s.project_id.as_str().to_string(), s.scenario_id.as_str().to_string()))
                .collect();
            (!scenarios.is_empty()).then(|| Gap::new(repo, ctx, format!("chore change `{}` approved", change.as_str()), since, scenarios, false, Some(change)))
        })
        .collect()
}

/// Whether `canon.yaml` configures several spec roots, which makes
/// `--project-id` required on the commands the hints name.
fn multi_root(repo: &Path) -> bool {
    crate::inventory::load_spec_roots(&repo.join("canon.yaml")).is_ok_and(|roots| roots.len() > 1)
}

/// The approved chore changes of `subject` that lack a qualifying
/// change-level review (0.23 D5), each with why. Only when
/// `require_review` is active and covers `shipped`: a repo with no review
/// policy asks no one to review.
pub fn chore_review_gaps(repo: &Path, ctx: &GateContext, subject: &Subject) -> Vec<(ChangeId, String)> {
    let Some((rr, _)) = active_require_review(ctx).filter(|(rr, _)| rr.covers(SubjectStatus::Shipped)) else { return Vec::new() };
    chore_approvals(repo, ctx, subject)
        .into_iter()
        .filter(|(_, since, _)| !reviewed_scenario_by_scenario_since(ctx, subject, *since))
        .filter_map(|(change, _, digest)| change_review_gap(ctx, subject, &change, &digest, rr.distinct_actor).map(|why| (change, why)))
        .collect()
}

/// Whether every evidenced scenario of `subject` carries a current, signed
/// review recorded after `since` and after the scenario's latest evidence:
/// a reviewer looked at each scenario as the chore left it, which covers at
/// least what one change-level review would. That is how a chore was
/// reviewed before 0.23, so a repo upgraded with such a chore shipped
/// stays green; the per-scenario reviews' own independence and currency
/// are the scenario rule's.
fn reviewed_scenario_by_scenario_since(ctx: &GateContext, subject: &Subject, since: DateTime<Utc>) -> bool {
    let reviews = canon_gate::review_gate::current_reviews(ctx);
    let evidenced: Vec<_> = canon_gate::spec_coverage::subject_scenarios(ctx, &subject.subject_id)
        .into_iter()
        .filter_map(|scenario| {
            let latest = canon_gate::freshness::scenario_evidence(ctx, scenario).into_iter().map(|e| e.envelope.at).max()?;
            Some((scenario, latest))
        })
        .collect();
    !evidenced.is_empty()
        && evidenced.iter().all(|(scenario, latest_evidence)| {
            reviews.iter().any(|review| {
                review.outcome.is_signed()
                    && review.project_id == scenario.project_id
                    && review.scenario_id.as_ref() == Some(&scenario.scenario_id)
                    && review.envelope.at >= since
                    && review.envelope.at >= *latest_evidence
            })
        })
}

/// Every unmet trigger of `subject`, for `canon status`: the chore
/// triggers and, when `require_review.block_on_findings` is on, the
/// fixed blockers on its changes.
pub fn gaps(repo: &Path, ctx: &GateContext, subject: &Subject) -> Vec<Gap> {
    let mut gaps = chore_gaps(repo, ctx, subject);
    if active_require_review(ctx).is_some_and(|(rr, _)| rr.block_on_findings) {
        let changes = subject_change_ids(ctx, subject);
        for fixed in fixed_blockers(ctx).into_iter().filter(|f| changes.contains(&f.finding.change_id)) {
            let scenarios: Vec<(String, String)> =
                fixed.unmet(ctx, &subject.subject_id).into_iter().map(|s| (s.project_id.as_str().to_string(), s.scenario_id.as_str().to_string())).collect();
            if !scenarios.is_empty() {
                let f = fixed.finding;
                gaps.push(Gap::new(repo, ctx, format!("blocker finding {}#{}.{} was closed fixed", f.change_id.as_str(), f.round, f.seq), fixed.closed, scenarios, fixed.is_scoped(), None));
            }
        }
    }
    gaps
}

/// The `verifying → shipped` violations of the chore trigger: one
/// `uncovered-cell` per unapproved-by-evidence chore, naming `subject`.
pub fn chore_violations(repo: &Path, ctx: &GateContext, subject: &Subject) -> Vec<Violation> {
    let reviews = chore_review_gaps(repo, ctx, subject).into_iter().map(|(change, why)| {
        Violation::new(
            FailureClass::UnreviewedPromotion,
            subject.subject_id.as_str(),
            format!("chore change `{}` approved: {why}; {}", change.as_str(), chore_review_remedy(&change))
        )
    });
    chore_gaps(repo, ctx, subject)
        .into_iter()
        .map(|gap| {
            Violation::new(
                FailureClass::UncoveredCell,
                subject.subject_id.as_str(),
                format!(
                    "{} {}: {} evidenced scenario(s) ({}) have no evidence recorded after the approval; {}",
                    gap.trigger,
                    gap.since.to_rfc3339_opts(SecondsFormat::Secs, true),
                    gap.scenarios.len(),
                    gap.scenario_list(),
                    gap.remedy()
                ),
            )
        })
        .chain(reviews)
        .collect()
}

/// The `canon review change` step a chore's review gap names.
pub fn chore_review_remedy(change: &ChangeId) -> String {
    format!(
        "have a reviewer record one review of the chore: `canon review change {} --project-id <root> --actor-id <reviewer> --session-id <review-session> --role reviewer --original-spec-ref \"<brief>\"`",
        change.as_str()
    )
}

/// The hint a refusal carries when any of `violations` is a
/// re-verification one: the batch command (built from `subject`'s real
/// report paths, [`Gap::commands`]) and what follows it, or, when every such
/// refusal is for a finding that names its scenarios, the per-scenario
/// command.
pub fn hint(repo: &Path, ctx: Option<&GateContext>, subject: &Subject, violations: &[&Violation]) -> Option<String> {
    let reverify: Vec<&&Violation> = violations.iter().filter(|v| is_reverify(v)).collect();
    if reverify.is_empty() {
        return None;
    }
    if reverify.iter().all(|v| is_scoped_reverify(v)) {
        return Some("record evidence again for the scenarios named, one command each (`canon evidence add --project-id <root> --scenario-id <id> --report <junit|cucumber|swift-testing>:<path> --role implementer`); the rest of the suite needs nothing; reviews stay current while a scenario's verdict and checks are unchanged, then ship".to_string());
    }
    let commands = ctx.map(|ctx| batch_commands_of(repo, ctx, subject)).unwrap_or_default();
    let how = match commands.as_slice() {
        [] => "record the suite again for the scenarios named with `canon evidence add --all-scenarios` and a --report for each test report of the suite".to_string(),
        [one] => format!("record the suite again for the scenarios named: `{one}`"),
        many => format!("record the suite again for the scenarios named, once per spec root: {}", many.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join("; ")),
    };
    let review = chore_review_commands(repo, ctx, subject).first().map_or_else(
        || String::new(),
        |change| format!("; a chore also needs its change-level review (`canon review change {change}`)"),
    );
    Some(format!("{how}; reviews stay current while a scenario's verdict and checks are unchanged{review}, then ship"))
}

/// The batch commands of every whole-suite gap of `subject`, each once.
pub fn batch_commands_of(repo: &Path, ctx: &GateContext, subject: &Subject) -> Vec<String> {
    let mut commands: Vec<String> = Vec::new();
    for gap in gaps(repo, ctx, subject).into_iter().filter(|g| !g.scoped) {
        for command in gap.commands {
            if !commands.contains(&command) {
                commands.push(command);
            }
        }
    }
    commands
}

/// The chore changes of `subject` whose approval is on record, by slug.
fn chore_review_commands(repo: &Path, ctx: Option<&GateContext>, subject: &Subject) -> Vec<String> {
    ctx.map(|ctx| chore_approvals(repo, ctx, subject).into_iter().map(|(change, _, _)| change.as_str().to_string()).collect()).unwrap_or_default()
}
