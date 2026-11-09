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
//! A chore whose approval recorded the code tree (0.37,
//! [`crate::brief::approval_tree`]) asks for no evidence at all while that
//! tree is the current code tree: the chore changed no code since the human
//! approved it, whatever tree each scenario's latest evidence is on. The
//! change-level review is still asked, and it must be of that code
//! ([`canon_gate::review_gate::change_review_gap`]'s `on_tree`): a review that
//! saw other code (the same brief approved again over code that came after
//! the review) does not count. An approval with no recorded tree
//! (written before 0.37, or with no git commit) keeps the rule above.
//!
//! A scenario with a code scope (0.38, [`crate::code_scopes`], the policy's
//! `code_scopes`) asks nothing of the chore when no path that differs between
//! the code tree of its latest evidence and the code tree now lies in the scope
//! ([`crate::git_rev::TreeDiffer`]); a path that does asks for it, and the
//! refusal names the path. No scope, no recorded tree and an old tree that
//! cannot be had keep the rule above: the exemption stands only when the
//! difference was computed.
//!
//! New evidence stales a pinned review, so the re-review follows with no
//! rule of its own. Every refusal names the scenarios and leads with ONE batch
//! command, [`canon_gate::review_gate::batch_commands`]: `canon evidence add
//! --all-scenarios --report junit:a.xml --report junit:b.xml --role
//! implementer`, built from the report paths bound to the subject's latest
//! evidence.

use std::path::Path;

use canon_gate::freshness::{latest_evidence, scenario_evidence, scenarios_without_evidence_after_on_code};
use canon_gate::review_gate::{
    active_require_review, batch_commands, change_review_gap, fixed_blockers, is_reverify, is_scoped_reverify, reverify_remedy, reverify_scenarios, scoped_reverify_remedy,
};
use canon_gate::spec_coverage::subject_change_ids;
use canon_gate::{FailureClass, GateContext, Violation};
use canon_model::{ChangeId, Scenario, SpecDigest, Subject, SubjectStatus};
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
    /// need evidence, one `canon evidence add --scenario` each; the
    /// rest of the suite is not asked for.
    pub scoped: bool,
    /// The batch command(s) that clear a whole-suite gap: one `canon evidence
    /// add --all-scenarios --report …` (one per spec root when the repo has
    /// several), built from the reports the scenarios' latest evidence bound.
    /// Empty when that evidence bound no report, and for a scoped gap.
    pub commands: Vec<String>,
    /// The chore change whose approval triggered the gap, when it is a chore.
    pub chore: Option<ChangeId>,
    /// The scoped scenarios among `scenarios` and the changed paths that put each in
    /// the gap (0.38 code scopes).
    pub hits: Vec<ScopeHit>,
    /// The `code_scopes` key of each scenario the chore does not ask for by scope.
    pub exempt: Vec<String>,
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
        Gap { trigger, since, scenarios, scoped, commands, chore, hits: Vec::new(), exempt: Vec::new(), remedy }
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
            return vec![format!("canon evidence add --project {project} --scenario {id} --report <junit|cucumber|swift-testing>:<path> --role implementer")];
        }
        if self.commands.is_empty() {
            return vec![format!("canon evidence add --project {} --all-scenarios --report <junit|cucumber|swift-testing>:<path> --role implementer", self.project())];
        }
        self.commands.clone()
    }

    /// `N evidenced scenario(s) (a, b, c, … k more)`.
    pub fn scenario_list(&self) -> String {
        reverify_scenarios(&self.pairs())
    }

    /// The project to pass to `--project`.
    pub fn project(&self) -> &str {
        self.scenarios.first().map_or("<root>", |(project, _)| project.as_str())
    }

    /// The step's text for `subject` in `canon status`.
    pub fn why(&self, subject: &str) -> String {
        format!(
            "{subject}: {} (since {}) and {} evidenced scenario(s) ({}) have no evidence recorded after it{}; the ship guard refuses until each does. {}; a review stays current while the scenario's verdict and checks are unchanged",
            self.trigger,
            self.since.to_rfc3339_opts(SecondsFormat::Secs, true),
            self.scenarios.len(),
            self.scenario_list(),
            self.scope_note(),
            if self.scoped { "The finding names these scenarios, so evidence for each of them alone is enough" } else { "Run the whole suite again; a scenario last evidenced on the code as it stands needs nothing" }
        )
    }

    /// What the code scopes say about this gap (0.38): the changed paths in scope that
    /// ask for each scoped scenario, and how many scenarios the scopes let go. Each
    /// part starts with `; `; empty when no scope played a part.
    pub fn scope_note(&self) -> String {
        let mut note = String::new();
        // Scenarios with the same scope and the same changed paths are one group.
        let mut groups: Vec<(&str, &[String], Vec<&str>)> = Vec::new();
        for hit in &self.hits {
            match groups.iter_mut().find(|(key, paths, _)| *key == hit.key && *paths == hit.paths.as_slice()) {
                Some((_, _, scenarios)) => scenarios.push(&hit.scenario),
                None => groups.push((&hit.key, &hit.paths, vec![&hit.scenario])),
            }
        }
        for (key, paths, scenarios) in groups {
            let shown = scenarios.iter().take(3).map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", ");
            let scenarios = match scenarios.len().saturating_sub(3) {
                0 => shown,
                more => format!("{shown}, … {more} more"),
            };
            let first = paths.iter().take(PATHS_SHOWN).map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ");
            let more = match paths.len().saturating_sub(PATHS_SHOWN) {
                0 => String::new(),
                more => format!(", … {more} more"),
            };
            note.push_str(&format!("; {scenarios} changed in scope `{key}`: {} path(s) ({first}{more})", paths.len()));
        }
        if !self.exempt.is_empty() {
            let mut keys = self.exempt.clone();
            keys.sort();
            keys.dedup();
            note.push_str(&format!("; {} scenario(s) need nothing from this chore: no path in their code scope ({}) changed since their evidence", self.exempt.len(), keys.iter().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ")));
        }
        note
    }
}

/// An approved chore change of a subject.
struct ChoreApproval {
    change: ChangeId,
    /// When it was approved.
    at: DateTime<Utc>,
    /// The digest of its brief as it reads now.
    digest: SpecDigest,
    /// The code tree the approval recorded (0.37); `None` for an approval
    /// from before 0.37 or one written with no git commit.
    tree: Option<String>,
}

impl ChoreApproval {
    /// Whether the approval saw the code as it is now (0.37): then the chore
    /// changed no code since, so it asks for no evidence. An approval that
    /// recorded no tree, or a work tree whose tree cannot be computed, is
    /// never that: the time rule stands.
    fn saw(&self, current: Option<&str>) -> bool {
        matches!((self.tree.as_deref(), current), (Some(then), Some(now)) if then == now)
    }
}

/// The approved chore changes among `subject`'s adopted changes, each with
/// its approval time, the code tree the approval recorded and the digest of
/// its brief as it reads now. A chore whose brief is not approved (or whose
/// approval line carries no readable time) triggers nothing.
fn chore_approvals(repo: &Path, ctx: &GateContext, subject: &Subject) -> Vec<ChoreApproval> {
    subject_change_ids(ctx, subject)
        .into_iter()
        .filter_map(|change| {
            let text = std::fs::read_to_string(crate::brief::canon_brief(repo, &change)?).ok()?;
            if !crate::brief_check::is_chore(&text) || !crate::brief::approval_of(&text).approved() {
                return None;
            }
            let line = canon_ingest::markdown::field(&text, "Approved:")?;
            let at = crate::brief::approved_at(line)?;
            Some(ChoreApproval { change, at, digest: crate::brief::brief_spec_digest(&text), tree: crate::brief::approval_tree(line).map(str::to_string) })
        })
        .collect()
}

/// What `canon status` and `canon gate check` warn about the policy's code scopes (0.38 F5): a
/// pattern that matches no file of the code tree, a key that matches no scenario, a key that
/// is not a map of lists. Nothing for a policy that declares no scope. Never a refusal.
pub fn scope_warnings(repo: &Path, ctx: &GateContext, code_tree: &CodeTreeMemo) -> Vec<String> {
    let scopes = crate::code_scopes::CodeScopes::read(repo);
    if !scopes.declares() {
        return Vec::new();
    }
    let files = code_tree.files(repo);
    scopes.warnings(files, canon_gate::spec_coverage::latest_scenarios(ctx).into_iter().map(|s| &s.scenario_id))
}

/// The paths that changed between an evidence record's code tree and now, when that is known.
type Changed = Option<std::rc::Rc<Vec<String>>>;

/// The changed paths a refusal names for one scope before it counts the rest.
const PATHS_SHOWN: usize = 3;

/// The code tree as it is now, computed at most once per command and only
/// when a subject with an approved chore needs it (`git add -A` over the
/// work tree is the cost). Errors (no git work tree) read as `None`, the
/// same as [`crate::git_rev::code_tree`] failing for the time-rule-only case.
/// It also holds, once and only when a code scope asks, the working tree's code
/// in a scratch object directory, and each comparison of an evidence record's
/// code with it (0.38).
#[derive(Debug)]
pub struct CodeTreeMemo {
    tree: std::cell::OnceCell<Option<String>>,
    differ: std::cell::OnceCell<Option<crate::git_rev::TreeDiffer>>,
    changed: std::cell::RefCell<std::collections::HashMap<(String, String), Changed>>,
    files: std::cell::OnceCell<Option<Vec<String>>>,
    live: std::cell::RefCell<std::collections::HashMap<String, bool>>,
}

impl CodeTreeMemo {
    /// A memo that computes the tree on first use.
    pub fn lazy() -> Self {
        Self { tree: std::cell::OnceCell::new(), differ: std::cell::OnceCell::new(), changed: std::cell::RefCell::default(), files: std::cell::OnceCell::new(), live: std::cell::RefCell::default() }
    }

    /// A memo for a tree the command already computed.
    pub fn known(tree: Option<String>) -> Self {
        Self { tree: std::cell::OnceCell::from(tree), ..Self::lazy() }
    }

    /// The code tree now, computed on first use.
    pub fn get(&self, repo: &Path) -> Option<&str> {
        self.tree.get_or_init(|| crate::git_rev::code_tree(repo, None).ok().map(|c| c.tree)).as_deref()
    }

    fn differ(&self, repo: &Path) -> Option<&crate::git_rev::TreeDiffer> {
        self.differ.get_or_init(|| crate::git_rev::TreeDiffer::new(repo).ok()).as_ref()
    }

    /// The paths that changed between the code tree `recorded` of commit `sha` and the
    /// working tree's; `None` when that cannot be known ([`crate::git_rev::TreeDiffer::changed_since`]).
    fn changed(&self, repo: &Path, sha: &str, recorded: &str) -> Option<std::rc::Rc<Vec<String>>> {
        let key = (sha.to_string(), recorded.to_string());
        if let Some(known) = self.changed.borrow().get(&key) {
            return known.clone();
        }
        let found = self.differ(repo).and_then(|differ| differ.changed_since(sha, recorded)).map(std::rc::Rc::new);
        self.changed.borrow_mut().insert(key, found.clone());
        found
    }

    /// The files of the working tree's code, for the checks of the scope declarations.
    pub fn files(&self, repo: &Path) -> Option<&[String]> {
        self.files.get_or_init(|| self.differ(repo).and_then(crate::git_rev::TreeDiffer::files)).as_deref()
    }

    /// Whether the scope `key` stands: one of its patterns matches a file of the code tree
    /// ([`crate::code_scopes::live`]).
    fn key_live(&self, repo: &Path, key: &str, patterns: &[String]) -> bool {
        if let Some(known) = self.live.borrow().get(key) {
            return *known;
        }
        let live = crate::code_scopes::live(patterns, self.files(repo));
        self.live.borrow_mut().insert(key.to_string(), live);
        live
    }
}

/// Whether `change` is an approved chore whose approval recorded the code tree
/// that is the current code tree (0.37): `canon status` says so on the chore's
/// line. The tree is computed only for an approval that recorded one.
pub fn tree_unchanged(repo: &Path, code_tree: &CodeTreeMemo, change: &ChangeId) -> bool {
    let Some(text) = crate::brief::canon_brief(repo, change).and_then(|path| std::fs::read_to_string(path).ok()) else { return false };
    if !crate::brief_check::is_chore(&text) || !crate::brief::approval_of(&text).approved() {
        return false;
    }
    let Some(recorded) = canon_ingest::markdown::field(&text, "Approved:").and_then(crate::brief::approval_tree) else { return false };
    code_tree.get(repo) == Some(recorded)
}

/// Every unmet chore trigger of `subject`. A chore whose approval recorded
/// the code tree that is the current one (0.37) triggers nothing: the code is
/// as the approval saw it, whatever tree the scenarios' evidence is on.
pub fn chore_gaps(repo: &Path, ctx: &GateContext, code_tree: &CodeTreeMemo, subject: &Subject) -> Vec<Gap> {
    judged(repo, ctx, code_tree, subject).into_iter().filter_map(|j| j.gap).collect()
}

/// A scenario a chore asks evidence for although it is scoped (0.38): paths that
/// changed since its latest evidence lie in its scope.
#[derive(Debug, Clone)]
pub struct ScopeHit {
    pub scenario: String,
    /// The `code_scopes` key the scenario's scope came from.
    pub key: String,
    /// The changed paths in the scope, sorted.
    pub paths: Vec<String>,
}

/// What a chore asks of one subject, and what it does not ask for by code scope.
struct Judged {
    change: ChangeId,
    gap: Option<Gap>,
    /// The `code_scopes` key of each scenario the chore does not ask for by scope.
    exempt: Vec<String>,
}

/// How one scenario fares against its code scope.
enum Scoped {
    /// No scope, or no diff could be had: the 0.37 rule stands.
    Unscoped,
    /// No path in the scope changed since the latest evidence.
    Exempt(String),
    /// A path in the scope changed.
    Changed(ScopeHit),
}

fn scoped(repo: &Path, ctx: &GateContext, code_tree: &CodeTreeMemo, scopes: &crate::code_scopes::CodeScopes, scenario: &Scenario) -> Scoped {
    let Some((key, patterns)) = scopes.of(&scenario.scenario_id) else { return Scoped::Unscoped };
    // A key none of whose patterns matches a file of the code tree declares no scope (fail closed: a misspelt pattern never exempts).
    if !code_tree.key_live(repo, key, patterns) {
        return Scoped::Unscoped;
    }
    let evidence = scenario_evidence(ctx, scenario);
    let Some(latest) = latest_evidence(&evidence) else { return Scoped::Unscoped };
    let (Some(sha), Some(tree)) = (latest.evidence_sha.as_ref(), latest.code_tree.as_deref()) else { return Scoped::Unscoped };
    let Some(changed) = code_tree.changed(repo, sha.as_str(), tree) else { return Scoped::Unscoped };
    let globs = crate::code_scopes::compile(patterns);
    let inside: Vec<String> = changed.iter().filter(|path| globs.iter().any(|glob| glob.matches(path))).cloned().collect();
    if inside.is_empty() {
        Scoped::Exempt(key.to_string())
    } else {
        Scoped::Changed(ScopeHit { scenario: scenario.scenario_id.as_str().to_string(), key: key.to_string(), paths: inside })
    }
}

/// The one rule behind [`chore_gaps`] and [`chore_exemptions`] (0.38): the 0.37
/// set of scenarios a chore asks evidence for, less each scenario with a code
/// scope whose paths changed since its latest evidence lie all outside it.
fn judged(repo: &Path, ctx: &GateContext, code_tree: &CodeTreeMemo, subject: &Subject) -> Vec<Judged> {
    let approvals = chore_approvals(repo, ctx, subject);
    if approvals.is_empty() {
        return Vec::new();
    }
    // The code as it is now (D11): a scenario last evidenced on it needs nothing.
    // No git work tree means nothing to compare, and the time rule stands.
    let current = code_tree.get(repo);
    let scopes = crate::code_scopes::CodeScopes::read(repo);
    approvals
        .into_iter()
        .filter(|approval| !approval.saw(current))
        .map(|ChoreApproval { change, at: since, .. }| {
            let mut needed: Vec<(String, String)> = Vec::new();
            let mut hits = Vec::new();
            let mut exempt = Vec::new();
            for scenario in scenarios_without_evidence_after_on_code(ctx, &subject.subject_id, since, current) {
                match scoped(repo, ctx, code_tree, &scopes, scenario) {
                    Scoped::Exempt(key) => exempt.push(key),
                    Scoped::Changed(hit) => {
                        hits.push(hit);
                        needed.push((scenario.project_id.as_str().to_string(), scenario.scenario_id.as_str().to_string()));
                    }
                    Scoped::Unscoped => needed.push((scenario.project_id.as_str().to_string(), scenario.scenario_id.as_str().to_string())),
                }
            }
            let gap = (!needed.is_empty()).then(|| {
                let mut gap = Gap::new(repo, ctx, format!("chore change `{}` approved", change.as_str()), since, needed, false, Some(change.clone()));
                gap.hits = hits;
                gap.exempt = exempt.clone();
                gap
            });
            Judged { change, gap, exempt }
        })
        .collect()
}

/// The scenarios an approved chore of `subject` does not ask evidence for by code scope (0.38),
/// per chore: how many, and by which `code_scopes` keys, sorted. `canon status` says it on the
/// chore's line and `canon gate check` as an advisory.
pub fn chore_exemptions(repo: &Path, ctx: &GateContext, code_tree: &CodeTreeMemo, subject: &Subject) -> Vec<(ChangeId, usize, Vec<String>)> {
    judged(repo, ctx, code_tree, subject)
        .into_iter()
        .filter(|j| !j.exempt.is_empty())
        .map(|j| {
            let mut keys = j.exempt.clone();
            keys.sort();
            keys.dedup();
            (j.change, j.exempt.len(), keys)
        })
        .collect()
}

/// Whether `canon.yaml` configures several spec roots, which makes
/// `--project` required on the commands the hints name.
fn multi_root(repo: &Path) -> bool {
    crate::inventory::load_spec_roots(&repo.join("canon.yaml")).is_ok_and(|roots| roots.len() > 1)
}

/// The approved chore changes of `subject` that lack a qualifying
/// change-level review (0.23 D5), each with why. Only when
/// `require_review` is active and covers `shipped`: a repo with no review
/// policy asks no one to review. A chore that asks for no evidence because its
/// approval saw the current code tree (0.37) needs a review of that code
/// ([`change_review_gap`]'s `on_tree`).
pub fn chore_review_gaps(repo: &Path, ctx: &GateContext, code_tree: &CodeTreeMemo, subject: &Subject) -> Vec<(ChangeId, String)> {
    let Some((rr, _)) = active_require_review(ctx).filter(|(rr, _)| rr.covers(SubjectStatus::Shipped)) else { return Vec::new() };
    let approvals = chore_approvals(repo, ctx, subject);
    // Computed only for an approval that recorded a tree.
    let current = approvals.iter().any(|approval| approval.tree.is_some()).then(|| code_tree.get(repo)).flatten();
    approvals
        .into_iter()
        .filter(|approval| !reviewed_scenario_by_scenario_since(ctx, subject, approval.at))
        .filter_map(|approval| {
            let on_tree = if approval.saw(current) { approval.tree.as_deref() } else { None };
            change_review_gap(ctx, subject, &approval.change, &approval.digest, rr.distinct_actor, on_tree).map(|why| (approval.change.clone(), why))
        })
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
pub fn gaps(repo: &Path, ctx: &GateContext, code_tree: &CodeTreeMemo, subject: &Subject) -> Vec<Gap> {
    let mut gaps = chore_gaps(repo, ctx, code_tree, subject);
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
pub fn chore_violations(repo: &Path, ctx: &GateContext, code_tree: &CodeTreeMemo, subject: &Subject) -> Vec<Violation> {
    let reviews = chore_review_gaps(repo, ctx, code_tree, subject).into_iter().map(|(change, why)| {
        Violation::new(
            FailureClass::UnreviewedPromotion,
            subject.subject_id.as_str(),
            format!("chore change `{}` approved: {why}; {}", change.as_str(), chore_review_remedy(&change))
        )
    });
    chore_gaps(repo, ctx, code_tree, subject)
        .into_iter()
        .map(|gap| {
            Violation::new(
                FailureClass::UncoveredCell,
                subject.subject_id.as_str(),
                format!(
                    "{} {}: {} evidenced scenario(s) ({}) have no evidence recorded after the approval{}; {}",
                    gap.trigger,
                    gap.since.to_rfc3339_opts(SecondsFormat::Secs, true),
                    gap.scenarios.len(),
                    gap.scenario_list(),
                    gap.scope_note(),
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
        "have a reviewer record one review of the chore: `canon review change {} --project <root> --actor <reviewer> --session <review-session> --role reviewer --original-spec-ref \"<brief>\"`",
        change.as_str()
    )
}

/// The hint a refusal carries when any of `violations` is a
/// re-verification one: the batch command (built from `subject`'s real
/// report paths, [`Gap::commands`]) and what follows it, or, when every such
/// refusal is for a finding that names its scenarios, the per-scenario
/// command.
pub fn hint(repo: &Path, ctx: Option<&GateContext>, code_tree: &CodeTreeMemo, subject: &Subject, violations: &[&Violation]) -> Option<String> {
    let reverify: Vec<&&Violation> = violations.iter().filter(|v| is_reverify(v)).collect();
    if reverify.is_empty() {
        return None;
    }
    if reverify.iter().all(|v| is_scoped_reverify(v)) {
        return Some("record evidence again for the scenarios named, one command each (`canon evidence add --project <root> --scenario <id> --report <junit|cucumber|swift-testing>:<path> --role implementer`); the rest of the suite needs nothing; reviews stay current while a scenario's verdict and checks are unchanged, then ship".to_string());
    }
    let commands = ctx.map(|ctx| batch_commands_of(repo, ctx, code_tree, subject)).unwrap_or_default();
    let how = match commands.as_slice() {
        [] => "record the suite again for the scenarios named with `canon evidence add --all-scenarios` and a --report for each test report of the suite".to_string(),
        [one] => format!("record the suite again for the scenarios named: `{one}`"),
        many => format!("record the suite again for the scenarios named, once per spec root: {}", many.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join("; ")),
    };
    let review = chore_review_commands(repo, ctx, subject).first().map_or_else(
        String::new,
        |change| format!("; a chore also needs its change-level review (`canon review change {change}`)"),
    );
    Some(format!("{how}; reviews stay current while a scenario's verdict and checks are unchanged{review}, then ship"))
}

/// The batch commands of every whole-suite gap of `subject`, each once.
pub fn batch_commands_of(repo: &Path, ctx: &GateContext, code_tree: &CodeTreeMemo, subject: &Subject) -> Vec<String> {
    let mut commands: Vec<String> = Vec::new();
    for gap in gaps(repo, ctx, code_tree, subject).into_iter().filter(|g| !g.scoped) {
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
    ctx.map(|ctx| chore_approvals(repo, ctx, subject).into_iter().map(|approval| approval.change.as_str().to_string()).collect()).unwrap_or_default()
}
