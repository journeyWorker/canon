//! `canon status [--repo <dir>] [--json]`: where this repo stands and
//! what to do next.
//!
//! `canon context` answers what CAN be authored; this answers what HAS
//! been: the policy's level (0.16 D3, [`crate::policy_level`]), each
//! subject's scenario, evidence and review counts, its open
//! blockers and missing required cases, the scenarios no subject owns,
//! the audit obligations `spec_coverage.require_audit` sampled and every refuted
//! audit (0.15 D3), and a short `next:` list of concrete commands.
//!
//! # One set of joins
//! Every count is read through the joins the gate itself uses, over a
//! [`GateContext`] loaded exactly as `canon gate check` loads it:
//! [`subject_scenarios`] for ownership, [`latest_verdicts`] for evidence
//! (the `verifying → shipped` gate's verdict fold), [`case_gaps`] for
//! required cases, [`review_gap`] for review (the `require_review` rule)
//! and [`open_blockers`] for findings. `spec_coverage.exclude_lanes`
//! removes scenarios from the counts as it does from the gate. Nothing is
//! re-derived here, so status and the gate cannot disagree about a gap.
//!
//! # A read
//! Status always exits `0` and writes nothing. A problem reading the
//! repo is reported as a warning with the command that fixes it.
//!
//! # `next:`
//! Deterministic rules over the counts, at most [`NEXT_LIMIT`] entries
//! (the rest are counted, never listed). Subjects are visited closest to
//! done first: verifying, building, specced, proposed, then shipped.
//!
//! # Roles and the write commands (0.21 D2, D3)
//! Every step carries the `role` that takes it (`null` for the `human`
//! step). With a declared role ([`crate::role::Role::current`]) `next`
//! keeps that role's steps (the orchestrator's also include the human
//! step) and `waitingOn` counts the rest per role, `human` included;
//! unset, nothing is filtered. A successful write command does not call
//! this module's renderer: [`crate::next`] reads [`StatusReport::all_next`]
//! (the steps before the limit and the role filter) and keeps the ones
//! about the subject it touched, so a write and `canon status` share one
//! set of rules.
//!
//! # The brief (0.16 D1, M7; 0.17 D3)
//! One human answer approves a subject's scope: its brief and the
//! policy. [`crate::brief::brief_of`] decides whether a subject's brief
//! is approved, and the `verifying → shipped` guard of `canon subject
//! status` and `canon evidence add` refuse on the same call, so status
//! and the guards cannot disagree. A subject whose newest change has a
//! brief to answer (its `.canon/changes/<change>/brief.md`, or an
//! external openspec `proposal.md` while that canon file does not exist)
//! and no `Approved:` line in the canon `brief.md` (the brief is
//! enforced) gets the one `human` step status ever emits once `canon brief
//! check` has no blocker left for it (0.22 D4); while it has some, the
//! subject's step is the planner's `canon brief check <slug>`,
//! `N blocker(s): write the brief`. With no blocker but advisories left
//! (0.26 D4), a planner step to polish the brief
//! (`canon skills show brief-polish`) and re-run `canon brief check` comes
//! first, naming the count and the top rule ids; the human step stays after
//! it (advisories never refuse an approval). Those steps are the
//! subject's only ones, whatever its status: the agent stops and reports
//! the brief, and nothing is built or attested for the subject (no
//! `feature new`, `scenario new`, `evidence add`, review or status
//! advance) before the human answers. The step names the canon file to
//! write, with its `Source:` pointer for an external plan. With no brief
//! to answer, the `brief:` line still says not approved, as an advisory,
//! and nothing is withheld. A `proposed` subject shows its brief once its
//! newest change has one. Evidence attested under a brief
//! override is listed as a warning ([`crate::brief::override_lines`]).
//! Every other step is the agent's.
//!
//! # A shipped subject whose gate went red (0.18 D3)
//! Evidence, specs and reviews keep moving after a subject ships. A
//! `shipped` subject that the `verifying → shipped` guard would now
//! refuse ([`crate::subject::shipped_gate_gaps`], the guard's own checks:
//! ownership, verdicts, required cases, audits, and the review guard,
//! open blockers only under `block_on_findings`, where `require_review`
//! covers `shipped`) is flagged red ([`SubjectSummary::red`]), and its
//! only step is the reopen, `canon subject move <id> --to verifying
//! --reason …`: the follow-up goes into its approved change as tasks,
//! then scenario, evidence and review, and it ships again through every
//! guard. A subject's changes are joined from both sides
//! ([`subject_change_ids`]), so a change shared by two subjects counts
//! its blockers against both.
//!
//! # No bookkeeping steps (0.16 M6)
//! A stale scenario index is read as a refresh would leave it, in memory
//! ([`crate::inventory::overlay_stale_index`]; `status` is a read command and
//! writes nothing), so no step says "then `canon inventory sync`": the sync
//! step appears only when the refresh would be refused (a format violation or a duplicate id), and
//! `canon inventory sync` names why (see Failing closed). The evidence step names
//! `canon gate promote` only when `canon.yaml` keeps writes staged
//! (`write.promote: manual`, the default without a `write:` key).
//!
//! # Failing closed
//! Status never suggests progress the gate would refuse. A spec root
//! whose changed `.feature` files the index refresh could not take
//! ([`crate::inventory::unrefreshable_roots`], the decision `canon gate
//! check` and the ship guard also fail on) makes `canon inventory sync`
//! the only step, since every count would describe the old specs. A corpus kind
//! routed away from the rung status reads (`scenario`, `subject`,
//! `review`, `finding`) would read as empty, so status then lists only
//! the routing repair, like the ship gate and `spec_coverage` refuse
//! rather than judge an empty corpus. An unusable `spec_coverage`
//! section is a blocking gap: the policy repair comes first and no
//! subject is suggested to move on until `canon gate check` can run.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use canon_gate::policy::subject_status_name;
use canon_gate::review_gate::{open_blockers, review_gap, ReviewGapKind};
use canon_gate::spec_coverage::{case_gaps, latest_scenarios, latest_subjects, subject_change_ids, subject_scenarios};
use canon_gate::review_gate::active_require_review;
use canon_gate::audit;
use canon_gate::{latest_verdicts, CellKey, GateContext, GateCtx, LedgerEntry, PolicyDiagnostic, PolicyResolution, RequireReview, SpecCoverage};
use canon_model::{AuditResult, ChangeId, EvidenceVerdict, Finding, ProjectId, RecordKind, Scenario, Subject, SubjectStatus};
use canon_policy::SchemaRegistry;
use chrono::Utc;
use schemars::JsonSchema;
use serde::Serialize;

use crate::cli_error::CliErrorExt as _;
use crate::json_out::Report;
use crate::context::{resolve_repo_root, spec_coverage_surface, SpecCoverageSurface};
use crate::brief::BriefStatus;
use crate::role::Role;
use crate::inventory::{load_spec_roots, scan_feature_corpus};

/// `canon status --json`'s shape version. Bumped when a field changes
/// meaning or is removed; an added field does not bump it. `2` (0.16):
/// a `next` entry's `command` is a canon command line only when its
/// `actor` is `agent`. `3` (0.17): `brief.proposal` is removed; the canon
/// `brief.md` is `brief.brief`, with `briefExists` and `source`. `4` (0.31):
/// a subject's `brief` is replaced by `briefs`, one entry per change the
/// subject answers for (tagged, approved or chore), each with its `status`
/// and `reason`; `pendingDrafts` holds only drafts, and `flow` the steps of
/// every tagged change.
const STATUS_VERSION: u32 = 4;

/// The most `next:` entries status prints.
pub const NEXT_LIMIT: usize = 5;

/// Lifecycle order: how subjects are grouped in the report.
const DISPLAY_ORDER: [SubjectStatus; 6] =
    [SubjectStatus::Proposed, SubjectStatus::Specced, SubjectStatus::Building, SubjectStatus::Verifying, SubjectStatus::Shipped, SubjectStatus::Retired];

/// Closest to done first: the order the `next:` rules visit subjects.
const NEXT_ORDER: [SubjectStatus; 5] = [SubjectStatus::Verifying, SubjectStatus::Building, SubjectStatus::Specced, SubjectStatus::Proposed, SubjectStatus::Shipped];

/// The whole report, in the order it prints.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StatusReport {
    pub status_version: u32,
    pub canon_version: &'static str,
    pub policy: PolicyStatus,
    /// One line per condition that makes the counts below incomplete or
    /// the gate weaker than it looks.
    pub warnings: Vec<String>,
    /// Lifecycle order (proposed → retired), then subject id.
    pub subjects: Vec<SubjectSummary>,
    /// Scenario ids (latest generation) whose `@subject:` tag is absent.
    pub unowned: Vec<String>,
    /// Audit obligations and refutations (0.15 D3). Omitted when
    /// `require_audit` is off and no latest audit is refuted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audits: Option<AuditSummary>,
    /// References of findings, evidence and subjects to a source nothing committed holds
    /// (0.30 `record-links`), in all and per kind of reference. Absent when they were not
    /// checked: a run that stopped early, or sources that could not be read (a warning says).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dangling_references: Option<crate::references::DanglingSummary>,
    /// The next steps, role-filtered: carried in `meta.next` (with `nextOmitted` and `waitingOn`), not in `data`.
    #[serde(skip)]
    #[schemars(skip)]
    pub next: Vec<NextStep>,
    /// How many further `next` entries the rules produced past
    /// [`NEXT_LIMIT`]: `meta.nextOmitted`.
    #[serde(skip)]
    #[schemars(skip)]
    pub next_omitted: usize,
    /// With a declared role: how many steps each other owner holds
    /// (`human` for the steps only the human takes): `meta.waitingOn`.
    #[serde(skip)]
    #[schemars(skip)]
    pub waiting_on: Option<BTreeMap<String, usize>>,
    /// Every step the rules produced, before the limit and the role
    /// filter: what [`crate::next`] scopes a write's `meta.next` from.
    #[serde(skip)]
    pub all_next: Vec<NextStep>,
    /// The subject each `all_next` entry belongs to (`None`: repo-wide).
    #[serde(skip)]
    pub step_subjects: Vec<Option<String>>,
    /// Scenario id → owning subject id.
    #[serde(skip)]
    pub scenario_owner: BTreeMap<String, String>,
    /// Change id → the subjects joined to it.
    #[serde(skip)]
    pub change_subjects: BTreeMap<String, Vec<String>>,
    /// Whether the ledger was read at all. False when canon.yaml or the
    /// ledger could not be loaded: nothing was examined (D3), so the run is
    /// `unverifiable` rather than a report of empty counts.
    #[serde(skip)]
    pub examined: bool,
}

impl Report for StatusReport {}

/// Where audits stand (0.15 D3).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditSummary {
    /// In-scope scenarios `spec_coverage.require_audit` sampled (0 when
    /// it is off).
    pub sampled: usize,
    /// Sampled scenarios whose latest audit is `confirmed`.
    pub confirmed: usize,
    /// Sampled scenarios still owing an audit: none recorded, or the
    /// latest is `indeterminate`.
    pub owed: Vec<AuditEntry>,
    /// Every scenario whose latest audit is `refuted` (`false-success`),
    /// sampled or not.
    pub refuted: Vec<AuditEntry>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub scenario: String,
    pub project_id: String,
    /// `missing`, `indeterminate`, or `refuted`.
    pub state: &'static str,
    /// The latest audit's evaluator, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluator: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PolicyStatus {
    /// Whether `.canon/policy.yaml` exists.
    pub present: bool,
    /// The effective `spec_coverage` section, keyed like `policy.yaml`
    /// (the same shape `canon context --json` prints); `null` when absent.
    pub spec_coverage: Option<SpecCoverageSurface>,
    /// The policy's level (0.16 D3, [`crate::policy_level::detect`]):
    /// `light`, `standard`, `strict`, or `custom`; `null` when the
    /// policy is absent.
    pub level: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubjectSummary {
    pub id: String,
    pub title: String,
    pub status: &'static str,
    /// Owned scenarios the gate counts (after `exclude_lanes`).
    pub scenarios: usize,
    /// Owned scenarios `exclude_lanes` removes from every count.
    pub excluded: usize,
    /// Counted scenarios with a ledger verdict.
    pub evidenced: usize,
    /// Counted scenarios whose latest verdict (for some role) is divergent.
    pub divergent: usize,
    /// Counted divergent scenarios a reviewer accepted as known gaps
    /// (`canon divergence accept`) whose acceptance still holds, each as
    /// `accepted-divergence <id> — <tracking> (expires <date>)`. They are
    /// not in `divergent`; omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub accepted_divergences: Vec<String>,
    /// Counted scenarios with a qualifying review under the
    /// `require_review` rule (any current signed review when it is off).
    pub reviewed: usize,
    /// Counted scenarios whose only signed reviews are stale: pinned to a
    /// spec or evidence that has since changed (0.15 D1).
    pub stale_reviews: usize,
    /// Counted scenarios whose latest reviews are all `indeterminate` or
    /// `unavailable` (0.15 D4).
    pub unsigned_reviews: usize,
    /// Whether `require_review` covers this subject's status or the next
    /// one on the chain, i.e. whether its reviews are due now.
    pub review_due: bool,
    /// Open blocker findings on the subject's adopted changes.
    pub open_blockers: usize,
    pub missing_cases: Vec<MissingCase>,
    /// The briefs the subject answers for (0.31): one entry per adopted
    /// change that is tagged by one of its scenarios, has an `Approved:`
    /// line, or is a chore, in adoption order, each with whether it reads as
    /// approved, why it governs and what `canon brief check` found. A subject
    /// with no such change has the one entry of its newest adopted change,
    /// which its reopen, ship and evidence wait on for a first approval, with
    /// no `reason` (or, adopting no change, an entry with no `change`).
    /// Present for `building`, `verifying` and `shipped` subjects, and for a
    /// `proposed` subject whose change has a brief to answer; omitted when
    /// empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub briefs: Vec<BriefStatus>,
    /// The briefs of [`Self::pending_drafts`]: the next step's input, not part of the report.
    #[serde(skip)]
    pub draft_briefs: Vec<BriefStatus>,
    /// The adopted changes that are pending drafts ([`crate::brief::governing`]):
    /// never approved, not a chore, and no scenario of the subject tagged
    /// `@flow:<change>.*` for them, so reopen, evidence and ship do not read
    /// them. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pending_drafts: Vec<String>,
    /// For a `shipped` subject, what makes its gate red now (0.18 D3):
    /// what the `verifying → shipped` guard would refuse it on
    /// ([`crate::subject::shipped_gate_gaps`], the same checks), one
    /// phrase per failure class: `<n> <class> (first: <subject>)`.
    /// Status then names the reopen command. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub red: Vec<String>,
    /// The flow steps of the briefs of every tagged change the subject answers
    /// for, with the scenarios tagged `@flow:<change>.<F-id>` (0.19 D4, 0.31).
    /// Omitted for a brief with no flow steps (a legacy brief) or no brief.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flow: Vec<FlowStep>,
    /// What the subject must be re-verified for before it ships (0.22 D2,
    /// [`crate::reverify`]): the next step's input, not part of the report.
    #[serde(skip)]
    pub reverify: Vec<crate::reverify::Gap>,
    /// Approved chore changes with no qualifying change-level review (0.23 D5).
    pub chore_reviews: Vec<(canon_model::ChangeId, String)>,
}

/// One flow step of the brief and what covers it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FlowStep {
    /// `F1`.
    pub id: String,
    /// The change whose brief defines it.
    pub change: String,
    pub scenarios: Vec<FlowScenarioState>,
    /// `no scenario` and/or `no failure case`; empty when covered.
    pub gaps: Vec<&'static str>,
}

/// One scenario covering a flow step, with the evidence and review state
/// the gate reads for it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FlowScenarioState {
    pub id: String,
    pub title: String,
    /// Tagged `@case:failure`.
    pub failure: bool,
    /// Has a ledger verdict ([`Joins::verdict_state`]).
    pub evidenced: bool,
    /// `reviewed`, or the review gap's kind: `missing`, `stale`,
    /// `unsigned`, `not-distinct`; `not-indexed` when the scenario index
    /// has no record for the id yet (`canon inventory sync`).
    pub review: &'static str,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MissingCase {
    /// The `specs.roots[]` id the surface lives under.
    pub project_id: String,
    /// `<area>.<surface>`.
    pub surface: String,
    pub case: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NextStep {
    /// A canon command line when `actor` is `agent`; what to get from
    /// the human when it is `human`.
    pub command: String,
    pub why: String,
    pub actor: StepActor,
    /// Who in the role set carries the step out (0.21 D3); `null` for a
    /// `human` step.
    pub role: Option<Role>,
}

/// Who carries out a `next` step (0.16 D1). The only `human` step is
/// getting a brief approved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StepActor {
    Agent,
    Human,
}

/// The policy warning `canon status` and `canon context`'s header print:
/// `Some` when no `spec_coverage` section is in force — the policy file
/// is absent or unreadable or has no such section (the gate requires
/// nothing), or the section is invalid (the gate refuses) — saying which.
/// `None` when a well-formed section is in force.
pub fn policy_warning(policy: &PolicyResolution) -> Option<String> {
    const NOTHING: &str = "`canon gate check` requires no evidence, failure cases or review here";
    for diagnostic in &policy.diagnostics {
        match diagnostic {
            PolicyDiagnostic::Missing { .. } => return Some(format!("no policy: .canon/policy.yaml not found, so {NOTHING}")),
            PolicyDiagnostic::Malformed { detail, .. } => {
                return Some(format!("policy unreadable: .canon/policy.yaml ({detail}) falls back to defaults, so {NOTHING}"))
            }
            _ => {}
        }
    }
    match &policy.spec_coverage {
        None => Some(format!("policy has no `spec_coverage` section, so {NOTHING}")),
        Some(SpecCoverage::Invalid { detail }) => Some(format!("policy's `spec_coverage` section is unusable ({detail}); `canon gate check` refuses until it is fixed")),
        Some(SpecCoverage::Active { .. }) => None,
    }
}

/// `canon status`: resolve, report, exit `0`.
pub fn run(repo: &Path, json: bool, fields: Option<&[String]>) -> u8 {
    if let Some(requested) = fields {
        if let Err(e) = crate::paging::check_known("canon status", requested, &REPORT_FIELDS) {
            return e.emit("canon status", json);
        }
    }
    let repo = resolve_repo_root(repo);
    crate::legacy::announce(&repo);
    let mut report = resolve_for_status(&repo);
    report.apply_role(Role::current(None));
    // A ledger that could not be read gave the report nothing to count (D3): `unverifiable`, never a silent pass.
    let coverage = status_coverage(&repo, &report);
    let unverifiable = coverage.examined_nothing();
    let error = unverifiable.then(|| crate::coverage::unverifiable_error("canon status", "nothing: canon.yaml or the ledger could not be read", "fix what `warnings` names (`canon init --check` reports what is wrong in canon.yaml), then run `canon status` again"));
    if json {
        let after = crate::next::After::of_status(&report);
        let mut meta = crate::json_out::warnings_meta(&report.warnings);
        if !meta.is_object() {
            meta = serde_json::json!({});
        }
        meta["coverage"] = coverage.meta()["coverage"].clone();
        match (&error, fields) {
            (Some(error), _) => return error.emit_with_data_and_meta("canon status", true, &report, meta),
            (None, Some(requested)) => {
                let projected = serde_json::to_value(&report).map(|v| crate::paging::project(&v, requested));
                match projected {
                    Ok(value) => crate::json_out::data_with_next_and_meta("status", &crate::json_out::Projected::<StatusReport>::new(value), &after, meta),
                    Err(e) => return crate::cli_error::CliError::failed("SERIALIZE_FAILED", format!("failed to serialize status: {e}")).hint("this is a canon bug: report it together with the command line that produced it").emit("canon status", true),
                }
            }
            (None, None) => crate::json_out::data_with_next_and_meta("status", &report, &after, meta),
        }
    } else {
        print!("{}", render(&report));
        if let Some(error) = &error {
            eprintln!("{}", error.human("canon status"));
        }
    }
    if let Some(error) = error {
        return error.exit;
    }
    0
}

/// What the report counted: the subjects and scenarios it joined, under the
/// spec roots `canon.yaml` configures.
pub fn status_coverage(repo: &Path, report: &StatusReport) -> crate::coverage::Coverage {
    let mut coverage = crate::coverage::Coverage::new();
    match crate::inventory::load_spec_roots(&repo.join("canon.yaml")) {
        Ok(roots) => {
            for root in roots {
                coverage.root(root.id.as_str(), root.root.strip_prefix(repo).unwrap_or(&root.root).display().to_string(), 0);
            }
        }
        Err(err) => {
            coverage.skip("specs.roots", err.to_string());
        }
    }
    coverage.count("subjects", report.subjects.len()).count("scenarios", report.scenario_owner.len() + report.unowned.len());
    coverage.count("ledgers", usize::from(report.examined));
    if !report.examined {
        coverage.skip("ledger", "canon.yaml or the ledger could not be read");
    }
    coverage
}

impl StatusReport {
    /// With a declared role, `next` keeps that role's steps and
    /// `waiting_on` counts the rest per owner (0.21 D3). No role: nothing
    /// changes.
    pub fn apply_role(&mut self, role: Option<Role>) {
        let Some(role) = role else { return };
        let (mine, waiting) = split_by_role(self.all_next.clone(), role);
        self.next_omitted = mine.len().saturating_sub(NEXT_LIMIT);
        self.next = mine.into_iter().take(NEXT_LIMIT).collect();
        self.waiting_on = Some(waiting);
    }
}

/// Whether `role` carries out `step`: its own steps, and the human steps
/// (role-less) for the orchestrator, who reports them to the human.
fn owned_by(step: &NextStep, role: Role) -> bool {
    step.role.map_or(role == Role::Orchestrator, |owner| owner == role)
}

/// `steps` split into `role`'s steps (order kept) and a count per other
/// owner (`human` for a step only the human takes).
pub fn split_by_role(steps: Vec<NextStep>, role: Role) -> (Vec<NextStep>, BTreeMap<String, usize>) {
    let mut waiting = BTreeMap::new();
    let mut mine = Vec::new();
    for step in steps {
        if owned_by(&step, role) {
            mine.push(step);
        } else {
            *waiting.entry(step.role.map_or("human", Role::as_str).to_string()).or_insert(0) += 1;
        }
    }
    (mine, waiting)
}

/// The top-level fields of the `--json` report, which `--fields` may name.
/// `audits` is omitted from a report that has none.
const REPORT_FIELDS: [&str; 8] = ["statusVersion", "canonVersion", "policy", "warnings", "subjects", "unowned", "audits", "danglingReferences"];

/// Build the report for an already-resolved repo root. The caller has
/// refreshed a stale scenario index (a writer that reports does, first);
/// `canon status` itself reads it with [`resolve_with_pending_index`].
pub fn resolve(repo: &Path) -> StatusReport {
    resolve_with(repo, false, false)
}

/// [`resolve`] for a command that wrote `.feature` files and must not
/// write the index: the scenario index a refresh would leave is read in
/// memory ([`crate::inventory::overlay_stale_index`]), so the report is
/// the one `canon status` gives after its own refresh.
pub fn resolve_with_pending_index(repo: &Path) -> StatusReport {
    resolve_with(repo, true, false)
}

/// [`resolve_with_pending_index`] for `canon status` itself: it also checks the references of
/// findings, evidence and subjects against the committed sources ([`crate::references`]), which
/// the commands that only need the next steps do not pay for.
fn resolve_for_status(repo: &Path) -> StatusReport {
    resolve_with(repo, true, true)
}

fn resolve_with(repo: &Path, pending_index: bool, references: bool) -> StatusReport {
    let registry = SchemaRegistry::load();
    let mut warnings = Vec::new();
    let mut next = Vec::new();

    if !repo.join("canon.yaml").is_file() {
        warnings.push(format!("no canon.yaml at {} — canon is not set up in this repo", repo.display()));
        next.push(NextStep { command: "canon init".into(), why: "set canon up here".into(), actor: StepActor::Agent, role: Some(Role::Orchestrator) });
    }

    let loaded = GateCtx::from_repo(repo).map_err(|e| e.to_string()).and_then(|ctx| GateContext::load(ctx, &registry, Utc::now()).map_err(|e| e.to_string()));
    let mut ctx = match loaded {
        Ok(ctx) => ctx,
        Err(e) => {
            // The ledger cannot be read, so nothing below can be counted.
            let policy = PolicyResolution::resolve(repo, &registry);
            warnings.extend(policy_warning(&policy));
            warnings.extend(crate::policy_approval::advisory(repo));
            warnings.push(format!("cannot read this repo's ledger: {e}"));
            next.push(NextStep { command: "canon init --check".into(), why: "canon.yaml must load before anything can be counted".into(), actor: StepActor::Agent, role: Some(Role::Orchestrator) });
            let mut report = finish(repo, &policy, warnings, Vec::new(), Vec::new(), None, next);
            report.examined = false;
            return report;
        }
    };

    if pending_index {
        crate::inventory::overlay_stale_index(&mut ctx);
    }
    warnings.extend(policy_warning(&ctx.policy));
    warnings.extend(crate::policy_approval::advisory(repo));
    let routed_away: Vec<&str> = ctx
        .unreadable_kinds
        .iter()
        .filter(|kind| matches!(kind, RecordKind::Scenario | RecordKind::Subject | RecordKind::Review | RecordKind::Finding))
        .map(|kind| kind.as_str())
        .collect();
    for kind in &routed_away {
        warnings.push(format!("`{kind}` routes away from the `local` rung status reads, so its records are not counted"));
    }
    let old_layout = ctx.violations.iter().chain(ctx.corpus_violations.iter().map(|(_, v)| v)).filter(|v| v.class == canon_model::FailureClass::OldLayout).count();
    if old_layout > 0 {
        warnings.push(format!("{old_layout} ledger record file(s) are named by the 0.29 rule and are not counted; run `canon migrate --json` to preview the rename, then `canon migrate --commit --plan-digest <digest>`"));
    }
    // The always-on ledger check names what is in `violations`; a corpus-kind file is named by the gate only
    // as a layout violation under a `spec_coverage` section that judges the corpus. `canon query` reports all.
    let named = ctx.violations.len() - ctx.violations.iter().filter(|v| v.class == canon_model::FailureClass::OldLayout).count();
    if named > 0 {
        warnings.push(format!("{named} ledger record(s) could not be read and are not counted; `canon gate check` names them"));
    }
    let corpus: Vec<_> = ctx.corpus_violations.iter().filter(|(_, v)| v.class != canon_model::FailureClass::OldLayout).collect();
    if !corpus.is_empty() {
        let mut kinds: Vec<&str> = corpus.iter().map(|(kind, _)| kind.as_str()).collect();
        kinds.sort_unstable();
        kinds.dedup();
        let queries = kinds.iter().map(|k| format!("`canon query --kind {k}`")).collect::<Vec<_>>().join(", ");
        let who = if canon_gate::spec_coverage::judges_corpus(&ctx) {
            format!("`canon gate check` names the misnamed ones, and {queries} reports each")
        } else {
            format!("`canon gate check` does not judge them without a `spec_coverage` section; {queries} reports each")
        };
        warnings.push(format!("{} ledger record(s) of kind {} could not be read and are not counted; {who}", corpus.len(), kinds.join(", ")));
    }
    if let Some(note) = canon_gate::review_gate::unmigrated_review_note(&ctx) {
        warnings.push(note);
    }
    warnings.extend(crate::brief::override_lines(&ctx));
    warnings.extend(crate::subject::override_lines(&ctx));

    let joins = Joins { verdicts: latest_verdicts(&ctx), blockers: open_blockers(&ctx) };
    let subjects = latest_subjects(&ctx);
    let spec_roots = load_spec_roots(&repo.join("canon.yaml")).unwrap_or_default();
    let roots: Vec<ProjectId> = spec_roots.iter().map(|r| r.id.clone()).collect();
    let plan_roots: Vec<PathBuf> = match crate::brief::plan_roots(repo) {
        Ok(roots) => roots,
        Err(e) => {
            warnings.push(format!("canon.yaml's `plans:` section cannot be read ({e}), so no brief reads as approved; `canon init --check` names the problem"));
            Vec::new()
        }
    };
    let derived = Derived { code_tree: crate::reverify::CodeTreeMemo::lazy(), flow_changes: crate::brief_check::FlowChangesIndex::lazy() };
    let mut summaries: Vec<(&Subject, SubjectSummary)> = subjects.iter().map(|s| (*s, summarize(&ctx, &joins, repo, &plan_roots, &derived, s))).collect();
    summaries.sort_by_key(|(s, _)| (rank(&DISPLAY_ORDER, s.status), s.subject_id.as_str().to_string()));

    let scenarios = latest_scenarios(&ctx);
    let unowned: Vec<&Scenario> = scenarios.iter().copied().filter(|s| s.subject_id.is_none()).collect();
    let known: BTreeSet<&str> = subjects.iter().map(|s| s.subject_id.as_str()).collect();
    let mut dangling: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for scenario in &scenarios {
        if let Some(id) = scenario.subject_id.as_ref().map(canon_model::SubjectId::as_str).filter(|id| !known.contains(*id)) {
            dangling.entry(id).or_default().push(scenario.scenario_id.as_str());
        }
    }
    let audits = audit_summary(&ctx);

    // ── fail closed: a corpus read as empty for a routing reason ──
    if !routed_away.is_empty() {
        let settings = routed_away.iter().map(|kind| format!("routing.{kind}: local")).collect::<Vec<_>>().join(", ");
        next.push(NextStep {
            actor: StepActor::Agent,
            role: Some(Role::Orchestrator),
            command: "canon init --check".into(),
            why: format!(
                "set `{settings}` in canon.yaml, then check it: status and the gate read only the local rung, so every count and step would rest on an empty corpus"
            ),
        });
        let summaries = summaries.into_iter().map(|(_, summary)| summary).collect();
        let unowned = unowned.iter().map(|s| s.scenario_id.as_str().to_string()).collect();
        return finish(repo, &ctx.policy, warnings, summaries, unowned, audits, next);
    }

    // ── fail closed: .feature edits the index refresh could not take ──
    // The same decision `canon gate check` and the ship guard fail on:
    // every count below would describe specs the files no longer say, so
    // the step that unblocks it is the only step. A root still on the
    // pre-0.17 layout is never read: `canon migrate` moves it (0.17 D1).
    let unrefreshable = crate::inventory::unrefreshable_roots(repo);
    if !unrefreshable.is_empty() {
        let quote = |roots: &[&crate::inventory::UnrefreshableRoot]| roots.iter().map(|root| format!("`{}`", root.id.as_str())).collect::<Vec<_>>().join(", ");
        let (old, stale): (Vec<_>, Vec<_>) = unrefreshable.iter().partition(|root| root.pre_0_17.is_some());
        if scenarios.is_empty() {
            warnings.push("the ledger holds no scenario records, so every subject reads as owning none".into());
        }
        if !old.is_empty() {
            let ids = quote(&old);
            warnings.push(format!("spec root(s) {ids} are on the pre-0.17 layout (`features/kind=feature/`), which canon no longer reads, so `canon gate check` fails and no subject is suggested to move on"));
            next.push(NextStep {
                actor: StepActor::Agent,
                role: Some(Role::Orchestrator),
                command: "canon migrate".into(),
                why: format!("spec root(s) {ids} hold their .feature files under `features/kind=feature/`; `canon migrate` moves them to `kind=feature/` directly under the root"),
            });
        }
        if !stale.is_empty() {
            let ids = quote(&stale);
            let (violations, errors) = stale.iter().fold((0, 0), |(v, e), root| (v + root.violations, e + root.sync_errors));
            warnings.push(format!("the scenario index for spec root(s) {ids} is stale and could not be refreshed, so `canon gate check` fails and no subject is suggested to move on"));
            next.push(NextStep {
                actor: StepActor::Agent,
                role: Some(Role::Orchestrator),
                command: "canon inventory sync".into(),
                why: format!(
                    "the .feature files under spec root(s) {ids} changed, but the automatic refresh could not index them ({violations} format violation(s), {errors} sync error(s)); this names each format violation or duplicate id to fix"
                ),
            });
        }
        let summaries = summaries.into_iter().map(|(_, summary)| summary).collect();
        let unowned = unowned.iter().map(|s| s.scenario_id.as_str().to_string()).collect();
        return finish(repo, &ctx.policy, warnings, summaries, unowned, audits, next);
    }

    // ── next: global rules ──
    // An unusable `spec_coverage` makes `canon gate check` refuse, so it
    // blocks every subject: repair it first, and suggest no status move.
    let policy_blocks = match &ctx.policy.spec_coverage {
        Some(SpecCoverage::Invalid { detail }) => {
            next.push(NextStep {
                actor: StepActor::Agent,
                role: Some(Role::Orchestrator),
                command: "canon gate check".into(),
                why: format!("fix `spec_coverage` in .canon/policy.yaml ({detail}); the gate refuses until it parses, so no subject is suggested to move on"),
            });
            true
        }
        _ => false,
    };
    for (id, tagged) in &dangling {
        warnings.push(format!("{} scenario(s) are tagged `@subject:{id}` but no subject `{id}` exists (first: {})", tagged.len(), tagged[0]));
        next.push(NextStep {
            actor: StepActor::Agent,
            role: Some(Role::Planner),
            command: format!("canon subject new {id} --domain <domain> --title \"<title>\" --actor <unit> --session <session>"),
            why: format!("{} scenario(s) are tagged @subject:{id}, which no subject record carries", tagged.len()),
        });
    }
    if subjects.is_empty() && dangling.is_empty() {
        next.push(NextStep {
            actor: StepActor::Agent,
            role: Some(Role::Planner),
            command: "canon subject new <id> --domain <domain> --title \"<title>\" --actor <unit> --session <session>".into(),
            why: "no subject exists yet; a subject is the unit status, review and shipping track".into(),
        });
    }

    // ── next: per-subject rules, closest to done first ──
    // `canon status` refreshes a stale scenario index before this runs
    // (module doc), so an empty index under a .feature corpus that holds
    // scenarios means the refresh refused: a format violation or a
    // duplicate id, which `canon inventory sync` names. Say that once
    // instead of one "owns no scenario" step per subject. But when the
    // .feature corpus itself holds no scenario, there is nothing to
    // index: the next step is writing features and scenarios, so each
    // subject gets its `scenario new … --subject` step below.
    let corpus_has_scenarios = || spec_roots.iter().any(|root| !scan_feature_corpus(&root.root).is_empty());
    let promote_manual = !matches!(crate::write_mode::configured_promote_mode(&repo.join("canon.yaml")), Ok(crate::write_mode::PromoteMode::Auto));
    let mut owners: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    if scenarios.is_empty() && !subjects.is_empty() && corpus_has_scenarios() {
        warnings.push("the ledger holds no scenario records, so every subject reads as owning none".into());
        next.push(NextStep {
            command: "canon inventory sync".into(),
            why: "the .feature files hold scenarios but the index holds none, so the automatic refresh could not index them; this names each format violation or duplicate id to fix".into(),
            actor: StepActor::Agent,
            role: Some(Role::Orchestrator),
        });
    } else {
        // A subject waiting on its brief starts no work, so it asks for
        // no feature either.
        if scenarios.is_empty() && summaries.iter().any(|(_, summary)| !awaits_brief(summary)) {
            next.push(NextStep {
                actor: StepActor::Agent,
                role: Some(Role::Implementer),
                command: format!("canon feature new <area>.<surface> --title \"<feature>\" --actor <unit> --session <session>{}", project_flag(&roots, None)),
                why: "no .feature file holds a scenario yet; write the features, then each subject's scenarios with `canon scenario new … --subject <id>`".into(),
            });
        }
        let mut by_priority: Vec<&(&Subject, SubjectSummary)> = summaries.iter().filter(|(s, _)| NEXT_ORDER.contains(&s.status)).collect();
        by_priority.sort_by_key(|(s, _)| (rank(&NEXT_ORDER, s.status), s.subject_id.as_str().to_string()));
        let inputs = StepInputs { ctx: &ctx, joins: &joins, all: &scenarios, roots: &roots };
        for (subject, summary) in by_priority {
            let start = next.len();
            subject_steps(&inputs, !policy_blocks, promote_manual, subject, summary, &mut next);
            owners.push((start..next.len(), subject.subject_id.as_str().to_string()));
        }
    }

    if let Some(first) = unowned.first() {
        next.push(NextStep {
            actor: StepActor::Agent,
            role: Some(Role::Orchestrator),
            command: "canon status".into(),
            why: format!(
                "{} scenario(s) carry no @subject tag (first: {}); add `@subject:<id>` above each in its .feature file, then check again: status re-indexes the edited files itself",
                unowned.len(),
                first.scenario_id.as_str()
            ),
        });
    }

    let scenario_owner = scenarios.iter().filter_map(|s| Some((s.scenario_id.as_str().to_string(), s.subject_id.as_ref()?.as_str().to_string()))).collect();
    let change_subjects = {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for subject in &subjects {
            for change in subject_change_ids(&ctx, subject) {
                map.entry(change.as_str().to_string()).or_default().push(subject.subject_id.as_str().to_string());
            }
        }
        map
    };
    let summaries = summaries.into_iter().map(|(_, summary)| summary).collect();
    let unowned = unowned.iter().map(|s| s.scenario_id.as_str().to_string()).collect();
    let dangling_references = if references { dangling_references(repo, &ctx, &mut warnings) } else { None };
    let mut report = finish(repo, &ctx.policy, warnings, summaries, unowned, audits, next);
    report.dangling_references = dangling_references;
    for (range, id) in owners {
        for slot in &mut report.step_subjects[range] {
            *slot = Some(id.clone());
        }
    }
    report.scenario_owner = scenario_owner;
    report.change_subjects = change_subjects;
    report
}

fn finish(
    repo: &Path,
    policy: &PolicyResolution,
    warnings: Vec<String>,
    subjects: Vec<SubjectSummary>,
    unowned: Vec<String>,
    audits: Option<AuditSummary>,
    mut next: Vec<NextStep>,
) -> StatusReport {
    let all_next = next.clone();
    let next_omitted = next.len().saturating_sub(NEXT_LIMIT);
    next.truncate(NEXT_LIMIT);
    StatusReport {
        status_version: STATUS_VERSION,
        canon_version: env!("CARGO_PKG_VERSION"),
        policy: PolicyStatus {
            present: !policy.diagnostics.iter().any(|d| matches!(d, PolicyDiagnostic::Missing { .. })),
            spec_coverage: spec_coverage_surface(policy),
            level: crate::policy_level::detect(repo, policy),
        },
        warnings,
        subjects,
        unowned,
        audits,
        dangling_references: None,
        next,
        next_omitted,
        waiting_on: None,
        step_subjects: vec![None; all_next.len()],
        all_next,
        scenario_owner: BTreeMap::new(),
        change_subjects: BTreeMap::new(),
        examined: true,
    }
}

/// The references of the ledger's findings, evidence and subjects that no committed source holds
/// ([`crate::references`]). `None`, with a warning, when the sources cannot be read.
fn dangling_references(repo: &Path, ctx: &GateContext, warnings: &mut Vec<String>) -> Option<crate::references::DanglingSummary> {
    let sources = match crate::references::Sources::load(repo) {
        Ok(sources) => sources,
        Err(error) => {
            warnings.push(format!("references to missing sources were not checked: {error}"));
            return None;
        }
    };
    let mut summary = crate::references::DanglingSummary::default();
    crate::references::summarize(&sources, RecordKind::Finding, &ctx.findings, &mut summary);
    crate::references::summarize(&sources, RecordKind::Evidence, ctx.evidence.iter().chain(&ctx.audits), &mut summary);
    crate::references::summarize(&sources, RecordKind::Subject, latest_subjects(ctx), &mut summary);
    Some(summary)
}

/// The `require_audit` sample's state and every refuted latest audit,
/// through the gate's own folds ([`canon_gate::audit`]). `None` when the
/// rule is off and nothing is refuted.
fn audit_summary(ctx: &GateContext) -> Option<AuditSummary> {
    let evaluator = |record: &canon_model::EvidenceRecord| record.audit.as_ref().map(|a| format!("{} {}", a.evaluator_id, a.evaluator_version));
    let refuted: Vec<AuditEntry> = audit::latest_audits(ctx)
        .into_iter()
        .filter(|(_, record)| record.audit.as_ref().is_some_and(|a| a.result == AuditResult::Refuted))
        .map(|((project, scenario), record)| AuditEntry {
            scenario: scenario.as_str().to_string(),
            project_id: project.as_str().to_string(),
            state: "refuted",
            evaluator: evaluator(record),
        })
        .collect();
    let active = audit::active_require_audit(ctx);
    if active.is_none() && refuted.is_empty() {
        return None;
    }
    let sampled = active.map(|(ra, lanes)| audit::sampled(ctx, ra, lanes)).unwrap_or_default();
    let owed = sampled
        .iter()
        .filter(|o| o.state.is_owed())
        .map(|o| AuditEntry {
            scenario: o.scenario.scenario_id.as_str().to_string(),
            project_id: o.scenario.project_id.as_str().to_string(),
            state: o.state.as_str(),
            evaluator: match o.state {
                audit::AuditState::Indeterminate(record) => evaluator(record),
                _ => None,
            },
        })
        .collect();
    Some(AuditSummary {
        sampled: sampled.len(),
        confirmed: sampled.iter().filter(|o| matches!(o.state, audit::AuditState::Confirmed(_))).count(),
        owed,
        refuted,
    })
}

fn rank(order: &[SubjectStatus], status: SubjectStatus) -> usize {
    order.iter().position(|s| *s == status).unwrap_or(order.len())
}

/// The status after `status` on the forward chain.
fn next_status(status: SubjectStatus) -> Option<SubjectStatus> {
    match status {
        SubjectStatus::Proposed => Some(SubjectStatus::Specced),
        SubjectStatus::Specced => Some(SubjectStatus::Building),
        SubjectStatus::Building => Some(SubjectStatus::Verifying),
        SubjectStatus::Verifying => Some(SubjectStatus::Shipped),
        SubjectStatus::Shipped | SubjectStatus::Retired => None,
    }
}

/// `spec_coverage`'s `exclude_lanes` and `require_cases`, empty when the
/// section is absent or unusable.
fn coverage_settings(ctx: &GateContext) -> (&[String], &[String]) {
    match &ctx.policy.spec_coverage {
        Some(SpecCoverage::Active { exclude_lanes, require_cases, .. }) => (exclude_lanes, require_cases),
        _ => (&[], &[]),
    }
}

/// The gate's corpus-wide folds, computed once per report.
struct Joins<'a> {
    verdicts: &'a BTreeMap<CellKey, LedgerEntry>,
    blockers: Vec<&'a Finding>,
}

impl Joins<'_> {
    /// Whether `scenario` has a ledger verdict, and whether any role's
    /// latest one is divergent — the `verifying → shipped` gate's reading.
    fn verdict_state(&self, scenario: &Scenario) -> (bool, bool) {
        let id = scenario.scenario_id.as_str();
        let mut entries = canon_gate::verdicts_of(self.verdicts, id).map(|(_, entry)| entry).peekable();
        let evidenced = entries.peek().is_some();
        (evidenced, entries.any(|e| e.verdict == EvidenceVerdict::Divergent))
    }

    /// Open blocker findings on `changes`, a subject's adopted changes
    /// ([`subject_change_ids`]).
    fn blockers_of(&self, changes: &[ChangeId]) -> Vec<&Finding> {
        self.blockers.iter().copied().filter(|f| changes.contains(&f.change_id)).collect()
    }
}

/// The active `require_review` rule, if any.
fn review_rule(ctx: &GateContext) -> Option<&RequireReview> {
    active_require_review(ctx).map(|(rr, _)| rr)
}

/// The owned scenarios the gate counts (after `exclude_lanes`).
fn counted<'a>(ctx: &'a GateContext, subject: &Subject) -> (Vec<&'a Scenario>, usize) {
    let (exclude_lanes, _) = coverage_settings(ctx);
    let owned = subject_scenarios(ctx, &subject.subject_id);
    let total = owned.len();
    let counted: Vec<&Scenario> = owned.into_iter().filter(|s| !s.lane.as_ref().is_some_and(|lane| exclude_lanes.contains(lane))).collect();
    let excluded = total - counted.len();
    (counted, excluded)
}

/// Whole-corpus values derived at most once per `canon status` run, each
/// computed on first need so a run that never asks pays nothing.
struct Derived {
    code_tree: crate::reverify::CodeTreeMemo,
    flow_changes: crate::brief_check::FlowChangesIndex,
}

fn summarize(ctx: &GateContext, joins: &Joins, repo: &Path, plan_roots: &[PathBuf], derived: &Derived, subject: &Subject) -> SubjectSummary {
    let (counted, excluded) = counted(ctx, subject);
    let (_, require_cases) = coverage_settings(ctx);
    let rr = review_rule(ctx);
    let distinct_actor = rr.is_some_and(|rr| rr.distinct_actor);

    let mut evidenced = 0;
    let mut divergent = 0;
    let mut accepted_divergences = Vec::new();
    for scenario in &counted {
        let (has, diverged) = joins.verdict_state(scenario);
        evidenced += usize::from(has);
        match canon_gate::acceptance::held(ctx, scenario) {
            Some(canon_gate::acceptance::Held::Holds(acceptance)) if diverged => accepted_divergences.push(acceptance.line()),
            _ => divergent += usize::from(diverged),
        }
    }
    let gaps: Vec<ReviewGapKind> = counted.iter().filter_map(|s| review_gap(ctx, s, distinct_actor)).map(|gap| gap.kind).collect();
    let reviewed = counted.len() - gaps.len();
    let gap_count = |kind| gaps.iter().filter(|k| **k == kind).count();
    let changes = subject_change_ids(ctx, subject);
    let open_blockers = joins.blockers_of(&changes).len();
    let missing_cases: Vec<MissingCase> = case_gaps(counted.iter().copied(), require_cases)
        .into_iter()
        .map(|gap| MissingCase { project_id: gap.project_id.as_str().to_string(), surface: gap.surface, case: gap.case })
        .collect();
    let review_due = rr.is_some_and(|rr| rr.covers(subject.status) || next_status(subject.status).is_some_and(|n| rr.covers(n)));
    let stale_reviews = gap_count(ReviewGapKind::Stale);
    let red = if subject.status == SubjectStatus::Shipped { shipped_red(repo, ctx, &derived.code_tree, subject) } else { Vec::new() };

    // The subject answers for its tagged, approved and chore changes (0.31);
    // a pending draft is shown, never judged.
    let governing = crate::brief::governing(repo, ctx, &derived.flow_changes, subject, &changes);
    let one = |change: &ChangeId| brief_with_findings(repo, plan_roots, subject, std::slice::from_ref(change));
    let mut briefs: Vec<BriefStatus> = governing
        .governed
        .iter()
        .filter_map(|(change, reason)| {
            let mut brief = one(change)?;
            brief.reason = Some(*reason);
            Some(brief)
        })
        .collect();
    // No governing change: the newest adopted change awaits its first approval.
    briefs.extend(if changes.is_empty() { crate::brief::brief_of(repo, plan_roots, subject, &[]) } else { governing.awaiting.as_ref().and_then(one) });
    let draft_briefs: Vec<BriefStatus> = governing.drafts.iter().filter_map(one).collect();
    let flow = governing.governed.iter().filter(|(_, reason)| *reason == crate::brief::Reason::Tagged).flat_map(|(change, _)| flow_steps(ctx, joins, repo, change, distinct_actor)).collect();
    SubjectSummary {
        id: subject.subject_id.as_str().to_string(),
        title: subject.title.clone(),
        status: subject_status_name(subject.status),
        scenarios: counted.len(),
        excluded,
        evidenced,
        divergent,
        accepted_divergences,
        reviewed,
        stale_reviews,
        unsigned_reviews: gap_count(ReviewGapKind::Unsigned),
        review_due,
        open_blockers,
        missing_cases,
        briefs,
        draft_briefs,
        pending_drafts: governing.drafts.iter().map(|c| c.as_str().to_string()).collect(),
        red,
        flow,
        reverify: if matches!(subject.status, SubjectStatus::Building | SubjectStatus::Verifying) { crate::reverify::gaps(repo, ctx, &derived.code_tree, subject) } else { Vec::new() },
        chore_reviews: if matches!(subject.status, SubjectStatus::Building | SubjectStatus::Verifying) { crate::reverify::chore_review_gaps(repo, ctx, subject) } else { Vec::new() },
    }
}

/// [`SubjectSummary::flow`]: each step of `change`'s brief, joined to its
/// scenarios through [`crate::brief_check::flow_scenarios`] (the one
/// `@flow:` lookup the brief rules also read) and to their evidence and
/// review state through the gate's own joins.
fn flow_steps(ctx: &GateContext, joins: &Joins, repo: &Path, change: &ChangeId, distinct_actor: bool) -> Vec<FlowStep> {
    let ids = crate::brief_check::flow_ids(repo, change);
    if ids.is_empty() {
        return Vec::new();
    }
    let index = crate::brief_check::flow_scenarios(repo, change).unwrap_or_default();
    // The first record per id wins, as the linear search it replaces did.
    let mut known: BTreeMap<&str, &Scenario> = BTreeMap::new();
    for scenario in latest_scenarios(ctx) {
        known.entry(scenario.scenario_id.as_str()).or_insert(scenario);
    }
    ids.into_iter()
        .map(|id| {
            let scenarios: Vec<FlowScenarioState> = index
                .get(&id)
                .into_iter()
                .flatten()
                .map(|found| {
                    let record = known.get(found.id.as_str()).copied();
                    let (evidenced, review) = match record {
                        None => (false, "not-indexed"),
                        Some(record) => (
                            joins.verdict_state(record).0,
                            match review_gap(ctx, record, distinct_actor).map(|gap| gap.kind) {
                                None => "reviewed",
                                Some(ReviewGapKind::Missing) => "missing",
                                Some(ReviewGapKind::Unsigned) => "unsigned",
                                Some(ReviewGapKind::Stale) => "stale",
                                Some(ReviewGapKind::NotDistinct) => "not-distinct",
                            },
                        ),
                    };
                    FlowScenarioState { id: found.id.clone(), title: found.title.clone(), failure: found.failure, evidenced, review }
                })
                .collect();
            let mut gaps = Vec::new();
            if scenarios.is_empty() {
                gaps.push("no scenario");
            } else if !scenarios.iter().any(|s| s.failure) {
                gaps.push("no failure case");
            }
            FlowStep { id, change: change.as_str().to_string(), scenarios, gaps }
        })
        .collect()
}

/// [`crate::brief::brief_of`] of the one change in `changes` plus what
/// `canon brief check` found in the brief's text (0.19 D2): the counts
/// `canon status` shows. Omitted for a pre-0.19 brief and for a brief that
/// does not exist yet.
fn brief_with_findings(repo: &Path, plan_roots: &[PathBuf], subject: &Subject, changes: &[ChangeId]) -> Option<BriefStatus> {
    let mut brief = crate::brief::brief_of(repo, plan_roots, subject, changes)?;
    if let (true, Some(change)) = (brief.brief_exists, changes.last()) {
        let building = matches!(subject.status, SubjectStatus::Building | SubjectStatus::Verifying | SubjectStatus::Shipped);
        if let Some((counts, rules)) = crate::brief_check::status_report(repo, change, building) {
            brief.findings = Some(counts);
            brief.advisory_rules = rules;
        }
    }
    Some(brief)
}

/// What makes a `shipped` subject's gate red ([`SubjectSummary::red`]):
/// the ship guard's own gaps ([`crate::subject::shipped_gate_gaps`]),
/// grouped by failure class in the order the guard reports them.
fn shipped_red(repo: &Path, ctx: &GateContext, code_tree: &crate::reverify::CodeTreeMemo, subject: &Subject) -> Vec<String> {
    let mut classes: Vec<(&'static str, usize, String)> = Vec::new();
    for violation in crate::subject::shipped_gate_gaps(repo, ctx, code_tree, subject) {
        match classes.iter_mut().find(|(class, _, _)| *class == violation.class.as_str()) {
            Some((_, count, _)) => *count += 1,
            None => classes.push((violation.class.as_str(), 1, violation.subject)),
        }
    }
    classes.into_iter().map(|(class, count, first)| format!("{count} {class} (first: {first})")).collect()
}

/// `canon subject move <id> --to verifying --reason …`: the reopen command
/// status names for a red shipped subject (0.18 D3).
pub(crate) fn reopen_command(id: &str) -> String {
    format!("canon subject move {id} --to verifying --reason \"<why it reopens>\" --actor <unit> --session <session>")
}

/// The next scenario number free on `surface` within spec root `project`.
fn next_number(scenarios: &[&Scenario], project: &str, surface: &str) -> String {
    let max = scenarios
        .iter()
        .filter(|s| s.project_id.as_str() == project)
        .filter_map(|s| s.scenario_id.as_str().rsplit_once('.'))
        .filter(|(prefix, _)| *prefix == surface)
        .filter_map(|(_, n)| n.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("{surface}.{:02}", max + 1)
}

/// `canon scenario new`'s `--project` flag: needed (and refused when
/// absent) only when the repo configures more than one spec root. A
/// known owning root is named exactly; otherwise a placeholder.
fn project_flag(roots: &[ProjectId], project: Option<&str>) -> String {
    if roots.len() <= 1 {
        return String::new();
    }
    format!(" --project {}", project.unwrap_or("<root-id>"))
}

/// The step for the first brief flow step with no scenario or no failure
/// case: `canon scenario new … --flow <F-id>`, which tags the scenario
/// `@flow:<change>.<F-id>` so it covers that step. `hint` is the
/// `(project, <area>.<surface>)` to number the suggested id on, when the
/// subject has one; a flow step that already has a scenario numbers on its
/// surface.
fn flow_step(id: &str, all: &[&Scenario], roots: &[ProjectId], summary: &SubjectSummary, hint: Option<(String, String)>) -> Option<NextStep> {
    let step = summary.flow.iter().find(|step| !step.gaps.is_empty())?;
    let (case, role, gap, title) = if step.scenarios.is_empty() {
        ("happy", Role::Implementer, "no scenario", "<behavior>")
    } else {
        ("failure", Role::Implementer, "no failure case", "<what happens on this path>")
    };
    let located = step
        .scenarios
        .first()
        .and_then(|covering| all.iter().find(|s| s.scenario_id.as_str() == covering.id))
        .and_then(|s| s.scenario_id.as_str().rsplit_once('.').map(|(surface, _)| (s.project_id.as_str().to_string(), surface.to_string())))
        .or(hint);
    let (suggested, project) = match located {
        Some((project, surface)) => (next_number(all, &project, &surface), Some(project)),
        None => ("<area>.<surface>.01".to_string(), None),
    };
    Some(NextStep {
        actor: StepActor::Agent,
        role: Some(role),
        command: format!("canon scenario new {suggested} --subject {id} --case {case} --flow {} --title \"{title}\" --actor <unit> --session <session>{}", step.id, project_flag(roots, project.as_deref())),
        why: format!("{id}: brief flow step {} has {gap}, and the ship guard refuses until it has one; tag it with --flow so it covers the step", step.id),
    })
}

/// The briefs the subject answers for that wait for the human (module doc):
/// enforced and not approved, among its governing changes and, with none, its
/// newest adopted change ([`SubjectSummary::briefs`]). A pending draft is not
/// one of them.
fn blocking(summary: &SubjectSummary) -> Vec<&BriefStatus> {
    summary.briefs.iter().filter(|brief| brief.enforced && !brief.approved).collect()
}

/// The planner's polish step for a brief with no blockers and advisories
/// left (0.26 D4): polish it, then run `canon brief check` again. It never
/// blocks: the human step stays in the list after it.
fn polish_step(id: &str, brief: &BriefStatus, draft: bool) -> Option<NextStep> {
    let (change, counts) = (brief.change.as_deref()?, brief.findings.filter(|counts| counts.blockers == 0 && counts.advisories > 0)?);
    let top = brief.advisory_rules.iter().take(3).map(String::as_str).collect::<Vec<_>>().join(", ");
    let draft = if draft { format!("; it is a pending draft, so {id} is not blocked meanwhile") } else { String::new() };
    Some(NextStep {
        actor: StepActor::Agent,
        role: Some(Role::Planner),
        command: format!("polish the brief in {} (`canon skills show brief-polish`), then run `canon brief check {change}`", brief.report_in()),
        why: format!("{id}: {} advisory finding(s) remain in `{change}` ({top}); the check is clean of blockers, so the human can answer now, but a polished brief is easier to read; advisories never refuse an approval{draft}", counts.advisories),
    })
}

fn awaits_brief(summary: &SubjectSummary) -> bool {
    !blocking(summary).is_empty()
}

/// What every subject's `next:` rules ([`subject_steps`]) read alike.
#[derive(Clone, Copy)]
struct StepInputs<'a> {
    ctx: &'a GateContext,
    joins: &'a Joins<'a>,
    all: &'a [&'a Scenario],
    roots: &'a [ProjectId],
}

/// The per-subject `next:` rules (module doc), each producing at most one
/// step. An enforced unapproved brief is the subject's only step; else
/// the gap rules; the status advance only when none fired and
/// `may_advance` (no blocking policy gap). `promote_manual` is whether
/// `canon.yaml` keeps writes staged for `canon gate promote`.
fn subject_steps(
    inputs: &StepInputs<'_>,
    may_advance: bool,
    promote_manual: bool,
    subject: &Subject,
    summary: &SubjectSummary,
    next: &mut Vec<NextStep>,
) {
    let StepInputs { ctx, joins, all, roots } = *inputs;
    let id = subject.subject_id.as_str();
    let status = subject.status;

    // A pending draft blocks nothing: its subject keeps building, attesting
    // and shipping for the scope it answers for. While `canon brief check` has
    // blockers the draft is the planner's to write; once clean, the human is
    // told it awaits an answer.
    for brief in summary.draft_briefs.iter().filter(|brief| brief.enforced) {
        let draft = brief.change.as_deref().unwrap_or_default();
        match (brief.change.as_deref(), brief.findings.as_ref().filter(|counts| counts.blockers > 0)) {
            (Some(change), Some(counts)) => next.push(NextStep {
                actor: StepActor::Agent,
                role: Some(Role::Planner),
                command: format!("canon brief check {change}"),
                why: format!("{id}: `{draft}` is a pending draft with {} blocker(s): write the brief in {} until `canon brief check` is clean; only then does the human answer it; {id} is not blocked meanwhile", counts.blockers, brief.report_in()),
            }),
            _ => {
                // Advisories never block the human's answer: the planner's
                // polish step comes first, the human step stays after it.
                next.extend(polish_step(id, brief, true));
                next.push(NextStep {
                    actor: StepActor::Human,
                    role: None,
                    command: format!("show the pending draft's brief in {} to the human and ask; {id} is not blocked meanwhile", brief.report_in()),
                    why: format!(
                        "{id}: `{draft}` is a pending draft ({}) and not yet part of the subject's scope: reopen, evidence and ship do not read it until the human approves it or a scenario is tagged `@flow:{draft}.*`, and then the subject cannot ship until the human answers; {}",
                        brief.unanswered(),
                        brief.record()
                    ),
                });
            }
        }
    }
    // The pending-draft step is not a gap: it must not hold back the advance.
    let before = next.len();
    let waiting = blocking(summary);
    if !waiting.is_empty() {
        // A brief `canon brief check` still has blockers for is the planner's
        // to write; the human is asked only once every one is clean (0.22 D4).
        let unclean: Vec<&&BriefStatus> = waiting.iter().filter(|brief| brief.findings.as_ref().is_some_and(|counts| counts.blockers > 0)).collect();
        if !unclean.is_empty() {
            for brief in unclean {
                let (Some(change), Some(counts)) = (brief.change.as_deref(), brief.findings.as_ref()) else { continue };
                next.push(NextStep {
                    actor: StepActor::Agent,
                    role: Some(Role::Planner),
                    command: format!("canon brief check {change}"),
                    why: format!("{id}: {} blocker(s): write the brief in {} until `canon brief check` is clean; only then does the human answer it", counts.blockers, brief.report_in()),
                });
            }
            return;
        }
        for brief in &waiting {
            next.extend(polish_step(id, brief, false));
        }
        let places = waiting.iter().map(|brief| brief.report_in().to_string()).collect::<Vec<_>>().join(", ");
        let named = waiting.iter().map(|brief| brief.unanswered()).collect::<Vec<_>>().join("; ");
        let tagged = waiting.iter().filter_map(|brief| brief.change.as_deref()).map(|change| format!("`@flow:{change}.*`")).collect::<Vec<_>>().join(", ");
        let changes = waiting.iter().filter_map(|brief| brief.change.as_deref()).map(|change| format!("`{change}`")).collect::<Vec<_>>().join(", ");
        next.push(NextStep {
            actor: StepActor::Human,
            role: None,
            command: format!(
                "show the brief in {places} to the human (`canon brief show <change> --body`) and ask; build and attest nothing that answers to it until the human answers (one answer approves brief and policy)"
            ),
            why: format!(
                "{id}: {named} (an older change's approval does not cover a new scope), and the approval is never assumed: `canon evidence add` refuses the scenarios tagged {tagged} and {changes}'s tasks (and {id}'s untagged scenarios not evidenced before {id} adopted its first change, while no adopted change is approved) until it is recorded; {} (one call records the brief and the policy approval)",
                crate::brief::record_all(&waiting)
            ),
        });
        return;
    }

    // A shipped subject whose gate went red (0.18 D3) is reopened first:
    // every gap below is follow-up work inside its approved change.
    if status == SubjectStatus::Shipped && !summary.red.is_empty() {
        let tasks = match summary.briefs.last().and_then(|brief| brief.change.as_deref()) {
            Some(change) => format!("keep the follow-up in its approved change `{change}` (never `canon change new`)"),
            None => "keep the follow-up in its change".to_string(),
        };
        next.push(NextStep {
            actor: StepActor::Agent,
            role: Some(Role::Orchestrator),
            command: reopen_command(id),
            why: format!(
                "{id} is shipped but its gate is red ({}); reopen it, {tasks}, then scenario, evidence and review, and ship it again",
                summary.red.join(", ")
            ),
        });
        return;
    }

    if summary.scenarios + summary.excluded == 0 {
        if let Some(step) = flow_step(id, all, roots, summary, None) {
            next.push(step);
            return;
        }
        let roots_note = if roots.len() > 1 { format!(" (spec roots: {})", roots.iter().map(ProjectId::as_str).collect::<Vec<_>>().join(", ")) } else { String::new() };
        next.push(NextStep {
            actor: StepActor::Agent,
            role: Some(Role::Implementer),
            command: format!("canon scenario new <area>.<surface>.01 --title \"<behavior>\" --subject {id} --case happy --actor <unit> --session <session>{}", project_flag(roots, None)),
            why: format!("{id} owns no scenario; specify its behavior, one scenario per behavior{roots_note}"),
        });
        return;
    }

    let attested = matches!(status, SubjectStatus::Building | SubjectStatus::Verifying | SubjectStatus::Shipped);
    let (counted, _) = counted(ctx, subject);
    // A brief flow step with no scenario or no failure case is the more specific
    // scenario to add (the ship guard refuses on it), so it replaces the generic one.
    let hint = summary
        .missing_cases
        .first()
        .map(|gap| (gap.project_id.clone(), gap.surface.clone()))
        .or_else(|| counted.first().and_then(|s| s.scenario_id.as_str().rsplit_once('.').map(|(surface, _)| (s.project_id.as_str().to_string(), surface.to_string()))));
    if let Some(step) = flow_step(id, all, roots, summary, hint) {
        next.push(step);
    } else if let Some(gap) = summary.missing_cases.first() {
        next.push(NextStep {
            actor: StepActor::Agent,
            role: Some(Role::Implementer),
            command: format!(
                "canon scenario new {} --title \"<what happens on this path>\" --subject {id} --case {} --actor <unit> --session <session>{}",
                next_number(all, &gap.project_id, &gap.surface),
                gap.case,
                project_flag(roots, Some(&gap.project_id))
            ),
            why: format!("{id}: {} surface(s) lack a @case:{} scenario (first: {}); add one, never retag an existing scenario", summary.missing_cases.len(), gap.case, gap.surface),
        });
    }

    if attested {
        if let Some(first) = counted.iter().find(|s| !joins.verdict_state(s).0) {
            next.push(NextStep {
                actor: StepActor::Agent,
                role: Some(Role::Implementer),
                command: format!(
                    "canon evidence add --scenario {} --project {} --class test-run --role implementer --actor <unit> --session <session> --verdict faithful --ref \"<test command>\"",
                    first.scenario_id.as_str(),
                    first.project_id.as_str()
                ),
                why: format!(
                    "{id}: {} of {} scenario(s) have no evidence; attest each{}",
                    summary.scenarios - summary.evidenced,
                    summary.scenarios,
                    if promote_manual { ", then `canon gate promote` (`write.promote: manual`)" } else { "" }
                ),
            });
        }
        // A fixed blocker or an approved chore asks for the whole suite
        // again (0.22 D2); the re-review follows the new evidence.
        if let Some(gap) = summary.reverify.first() {
            for command in gap.commands() {
                next.push(NextStep { actor: StepActor::Agent, role: Some(Role::Implementer), command, why: gap.why(id) });
            }
        }
                if let Some(first) = counted.iter().find(|s| joins.verdict_state(s).1 && !canon_gate::acceptance::is_accepted(ctx, s)) {
            next.push(NextStep {
                actor: StepActor::Agent,
                role: Some(Role::Implementer),
                command: format!(
                    "canon evidence add --scenario {} --project {} --class test-run --role implementer --actor <unit> --session <session> --verdict faithful --ref \"<test command>\"",
                    first.scenario_id.as_str(),
                    first.project_id.as_str()
                ),
                why: format!("{id}: {} scenario(s) have a divergent latest verdict; fix the behavior, then re-attest, or have a reviewer accept the tracked gap (`canon divergence accept`)", summary.divergent),
            });
        }
    }

    if let Some(blocker) = joins.blockers_of(&subject_change_ids(ctx, subject)).first() {
        next.push(NextStep {
            actor: StepActor::Agent,
            role: Some(Role::Reviewer),
            command: format!(
                "canon finding close {} --round {} --seq {} --disposition fixed --actor <unit> --session <session>",
                blocker.change_id.as_str(),
                blocker.round,
                blocker.seq
            ),
            why: format!("{id}: {} open blocker finding(s) (first: {}); fix it, commit, then close it", summary.open_blockers, blocker.summary),
        });
    }

    if let Some((change, why)) = summary.chore_reviews.first() {
        next.push(NextStep {
            actor: StepActor::Agent,
            role: Some(Role::Reviewer),
            command: format!(
                "canon review change {} --project <root> --actor <reviewer> --session <review-session> --role reviewer --original-spec-ref \"<brief>\"",
                change.as_str()
            ),
            why: format!("{id}: {why}"),
        });
    }

    if summary.review_due {
        let distinct_actor = review_rule(ctx).is_some_and(|rr| rr.distinct_actor);
        if let Some(first) = counted.iter().find(|s| joins.verdict_state(s).0 && review_gap(ctx, s, distinct_actor).is_some()) {
            next.push(NextStep {
                actor: StepActor::Agent,
                role: Some(Role::Reviewer),
                command: format!(
                    "canon review scenario {1} --project {0} --actor <reviewer> --session <review-session> --role reviewer --original-spec-ref \"<feature file>\"",
                    first.project_id.as_str(),
                    first.scenario_id.as_str()
                ),
                why: format!(
                    "{id}: {} of {} scenario(s) lack a qualifying review{}; review from another session (--session must differ from every evidence session), as an actor other than the evidence actor",
                    summary.scenarios - summary.reviewed,
                    summary.scenarios,
                    review_breakdown(summary)
                ),
            });
        }
    }

    // Audits (0.15 D3): a refuted latest audit always blocks; an owed
    // sampled audit blocks once `require_audit` covers this status or
    // the next one — the same rule the ship guard applies.
    let audit_target = audit::active_require_audit(ctx)
        .and_then(|(ra, _)| [Some(status), next_status(status)].into_iter().flatten().find(|s| ra.covers(*s)))
        .unwrap_or(status);
    let audit_gaps = audit::subject_audit_gaps(ctx, &subject.subject_id, audit_target);
    let owned = subject_scenarios(ctx, &subject.subject_id);
    let project_of = |scenario: &str| owned.iter().find(|s| s.scenario_id.as_str() == scenario).map_or("<root-id>", |s| s.project_id.as_str());
    let audit_command = |scenario: &str| {
        format!(
            "canon evidence audit {scenario} --project {} --evaluator <evaluator> --evaluator-version <version> --evaluator-digest <sha256> --audit-result <confirmed|refuted|indeterminate> --ref \"<evaluator run>\" --role auditor --actor <evaluator>",
            project_of(scenario)
        )
    };
    for (class, why) in [
        (canon_gate::FailureClass::FalseSuccess, "scenario(s) have a refuted latest audit (false-success); fix the behavior, re-attest, and have the external evaluator re-audit"),
        (canon_gate::FailureClass::UncoveredCell, "sampled scenario(s) owe an audit; an external evaluator re-audits each, canon only records the result"),
    ] {
        let gaps: Vec<&canon_gate::Violation> = audit_gaps.iter().filter(|v| v.class == class).collect();
        if let Some(first) = gaps.first() {
            let command = audit_command(&first.subject);
            next.push(NextStep { command, why: format!("{id}: {} {why} (first: {})", gaps.len(), first.subject), actor: StepActor::Agent, role: Some(Role::Orchestrator) });
        }
    }

    // Every gap rule above was silent: the subject may move on (shipped
    // and retired have nowhere to go), unless a policy gap blocks it.
    if next.len() > before || !may_advance {
        return;
    }
    if let Some(to) = next_status(status) {
        let to = subject_status_name(to);
        let reviewed = if summary.review_due { " and reviewed" } else { "" };
        next.push(NextStep {
            actor: StepActor::Agent,
            role: Some(Role::Orchestrator),
            command: format!("canon subject move {id} --to {to} --actor <unit> --session <session>"),
            why: match status {
                SubjectStatus::Proposed | SubjectStatus::Specced => format!("{id} has its scenarios and every required case; move it on"),
                SubjectStatus::Verifying => {
                    let approved = !summary.briefs.is_empty() && summary.briefs.iter().all(|brief| brief.approved);
                    let brief = match (!summary.pending_drafts.is_empty(), approved) {
                        (true, true) => "the briefs of its scope are approved (the draft is pending)",
                        (true, false) => "no brief of its scope is approved",
                        (false, true) => "the brief is approved",
                        (false, false) => "no brief is written to approve",
                    };
                    format!("{id}: every scenario is evidenced{reviewed}, with no blocker or case gap, and {brief}; ship it yourself unless the brief says the human ships")
                }
                _ => format!("{id}: every scenario is evidenced{reviewed}, with no blocker or case gap"),
            },
        });
    }
}

/// The human form (module doc).
pub fn render(report: &StatusReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "canon {}", report.canon_version);
    match (&report.policy.present, &report.policy.spec_coverage) {
        (false, _) => {
            let _ = writeln!(out, "policy: absent");
        }
        (true, None) => {
            let _ = writeln!(out, "policy: .canon/policy.yaml (no spec_coverage section)");
        }
        (true, Some(SpecCoverageSurface::Invalid { invalid })) => {
            let _ = writeln!(out, "policy: .canon/policy.yaml (spec_coverage INVALID — {invalid})");
        }
        (true, Some(SpecCoverageSurface::Active { require_evidence, scope, exclude_lanes, require_cases, require_review, require_audit })) => {
            let _ = writeln!(out, "policy: .canon/policy.yaml");
            let mut coverage = format!("require_evidence={require_evidence} scope={}", list_or(scope, "<every scenario>"));
            if !exclude_lanes.is_empty() {
                let _ = write!(coverage, " exclude_lanes={}", exclude_lanes.join(", "));
            }
            let _ = writeln!(out, "  spec_coverage: {coverage}");
            let _ = writeln!(out, "  require_cases: {}", list_or(require_cases, "<none>"));
            let _ = match require_review {
                Some(rr) => writeln!(
                    out,
                    "  require_review: scope={} distinct_actor={} block_on_findings={}",
                    list_or(&rr.scope, "<every scenario>"),
                    rr.distinct_actor,
                    rr.block_on_findings
                ),
                None => writeln!(out, "  require_review: <absent>"),
            };
            if let Some(ra) = require_audit {
                let _ = writeln!(out, "  require_audit: {}", crate::context::require_audit_line(ra));
            }
        }
    }
    if let Some(level) = report.policy.level {
        match crate::policy_level::raise_hint(level) {
            Some(hint) => {
                let _ = writeln!(out, "  level: {level} — {hint}");
            }
            None => {
                let _ = writeln!(out, "  level: {level}");
            }
        }
    }
    for warning in &report.warnings {
        let _ = writeln!(out, "warning: {warning}");
    }

    let _ = writeln!(out);
    if let Some(kinds) = report.dangling_references.as_ref().and_then(crate::references::DanglingSummary::describe) {
        let count = report.dangling_references.as_ref().map_or(0, |summary| summary.count);
        let _ = writeln!(out, "dangling references: {count} point at a source nothing committed holds ({kinds})");
    }
    let _ = writeln!(out, "subjects ({}):", report.subjects.len());
    for status in DISPLAY_ORDER {
        let name = subject_status_name(status);
        let group: Vec<&SubjectSummary> = report.subjects.iter().filter(|s| s.status == name).collect();
        if group.is_empty() {
            continue;
        }
        let _ = writeln!(out, "  {name} ({}):", group.len());
        for s in group {
            let mut line = format!("{} scenarios", s.scenarios);
            if s.excluded > 0 {
                let _ = write!(line, " (+{} excluded by lane)", s.excluded);
            }
            let _ = write!(line, ", {} evidenced", s.evidenced);
            if s.divergent > 0 {
                let _ = write!(line, ", {} divergent", s.divergent);
            }
            let _ = write!(line, ", {} reviewed", s.reviewed);
            if s.review_due && s.reviewed < s.scenarios {
                let _ = write!(line, " ({} due)", s.scenarios - s.reviewed);
            }
            if s.stale_reviews > 0 {
                let _ = write!(line, ", {} stale review(s)", s.stale_reviews);
            }
            if s.unsigned_reviews > 0 {
                let _ = write!(line, ", {} unsigned review(s)", s.unsigned_reviews);
            }
            let _ = write!(line, ", {} open blockers", s.open_blockers);
            let _ = writeln!(out, "    {}: {line}", s.id);
            for gap in &s.missing_cases {
                let _ = writeln!(out, "      missing @case:{} on {}", gap.case, gap.surface);
            }
            for line in &s.accepted_divergences {
                let _ = writeln!(out, "      advisory: {line}");
            }
            let many = s.briefs.len() > 1;
            for brief in &s.briefs {
                let of = match (many, brief.change.as_deref()) {
                    (true, Some(change)) => format!(" [{change}{}]", brief.reason.map_or(String::new(), |reason| format!(", {}", reason_word(reason)))),
                    _ => String::new(),
                };
                let _ = match (&brief.approval, &brief.brief, &brief.change) {
                    (Some(_), Some(_), _) if brief.changed => writeln!(out, "      brief{of}: not approved ({}); {}", brief.unanswered(), brief.record()),
                    (Some(approval), Some(path), _) if brief.legacy_approval => writeln!(
                        out,
                        "      brief{of}: approved in {path} — {approval}\n      advisory: that approval has no digest, so it does not follow the brief's text; to bind it, {}",
                        brief.record_digest_hint()
                    ),
                    (Some(approval), Some(path), _) => writeln!(out, "      brief{of}: approved in {path} — {approval}"),
                    (None, Some(_), _) => writeln!(out, "      brief{of}: not approved ({})", brief.unanswered()),
                    (_, None, Some(change)) => writeln!(out, "      brief{of}: not approved (change `{change}` has no brief; advisory, nothing to approve yet)"),
                    _ => writeln!(out, "      brief{of}: not approved (no adopted change; advisory, nothing to approve yet)"),
                };
            }
            if !s.pending_drafts.is_empty() {
                let names = s.pending_drafts.iter().map(|d| format!("`{d}`")).collect::<Vec<_>>().join(", ");
                let verb = if s.pending_drafts.len() == 1 { "is a pending draft" } else { "are pending drafts" };
                let _ = writeln!(out, "      {names} {verb}: not yet part of the subject's scope (never approved, not a chore, no scenario tagged `@flow:<change>.*`); reopen, evidence and ship do not read {}", if s.pending_drafts.len() == 1 { "it" } else { "them" });
            }
            for brief in &s.briefs {
                if let Some(counts) = brief.findings {
                    let of = match (many, brief.change.as_deref()) {
                        (true, Some(change)) => format!(" [{change}]"),
                        _ => String::new(),
                    };
                    let _ = writeln!(out, "      brief check{of}: {} blocker(s), {} advisory(ies); `canon brief show` names the next step", counts.blockers, counts.advisories);
                }
            }
            for step in &s.flow {
                let _ = writeln!(out, "      flow {}{}", step.id, if step.gaps.is_empty() { String::new() } else { format!(" — {}", step.gaps.join(", ")) });
                for scenario in &step.scenarios {
                    let _ = writeln!(
                        out,
                        "        {} \"{}\"{} — evidence: {}, review: {}",
                        scenario.id,
                        scenario.title,
                        if scenario.failure { " [failure]" } else { "" },
                        if scenario.evidenced { "yes" } else { "none" },
                        scenario.review
                    );
                }
            }
            if !s.red.is_empty() {
                if awaits_brief(s) {
                    let _ = writeln!(out, "      gate: red — {}; reopening is refused until the brief above is approved (answer it, or remove its `@flow:` tags, after which `canon subject unadopt` can take the change back)", s.red.join(", "));
                } else {
                    let _ = writeln!(out, "      gate: red — {}; reopen with `{}`", s.red.join(", "), reopen_command(&s.id));
                }
            }
        }
    }

    let _ = writeln!(out);
    if report.unowned.is_empty() {
        let _ = writeln!(out, "unowned: none");
    } else {
        let shown = report.unowned.iter().take(NEXT_LIMIT).cloned().collect::<Vec<_>>().join(", ");
        let more = report.unowned.len().saturating_sub(NEXT_LIMIT);
        let _ = writeln!(out, "unowned ({}, no @subject): {shown}{}", report.unowned.len(), if more > 0 { format!(", … {more} more") } else { String::new() });
    }

    if let Some(audits) = &report.audits {
        let _ = writeln!(out);
        let _ = writeln!(out, "audits: {} sampled, {} confirmed, {} owed, {} refuted", audits.sampled, audits.confirmed, audits.owed.len(), audits.refuted.len());
        for entry in audits.owed.iter().chain(&audits.refuted) {
            let evaluator = entry.evaluator.as_ref().map(|e| format!(" by {e}")).unwrap_or_default();
            let label = if entry.state == "refuted" { "false-success" } else { "owes an audit" };
            let _ = writeln!(out, "  {label}: {} ({}{evaluator})", entry.scenario, entry.state);
        }
    }
    let _ = writeln!(out);
    if report.next.is_empty() {
        if report.waiting_on.as_ref().is_some_and(|waiting| !waiting.is_empty()) {
            let _ = writeln!(out, "next: nothing for your role right now");
        } else {
            let _ = writeln!(out, "next: nothing — every subject is shipped or retired and every scenario has a subject");
        }
    } else {
        let _ = writeln!(out, "next:");
        for step in &report.next {
            let _ = writeln!(out, "  # {}", step.why);
            let _ = match step.actor {
                StepActor::Agent => writeln!(out, "  {}", step.command),
                StepActor::Human => writeln!(out, "  human: {}", step.command),
            };
        }
        if report.next_omitted > 0 {
            let _ = writeln!(out, "  … {} more once these are done", report.next_omitted);
        }
    }
    for (role, count) in report.waiting_on.iter().flatten() {
        let _ = writeln!(out, "waiting on {role}: {count}");
    }
    out
}

/// How a [`crate::brief::Reason`] reads in the human form.
fn reason_word(reason: crate::brief::Reason) -> &'static str {
    match reason {
        crate::brief::Reason::Tagged => "tagged",
        crate::brief::Reason::Approved => "approved",
        crate::brief::Reason::Chore => "chore",
    }
}

fn list_or(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        empty.to_string()
    } else {
        values.join(", ")
    }
}

/// The review next step's breakdown of stale and unsigned gaps, empty
/// when there are none.
fn review_breakdown(summary: &SubjectSummary) -> String {
    let mut parts = Vec::new();
    if summary.stale_reviews > 0 {
        parts.push(format!("{} with a stale review (spec or evidence changed since it was reviewed)", summary.stale_reviews));
    }
    if summary.unsigned_reviews > 0 {
        parts.push(format!("{} with only indeterminate or unavailable reviews", summary.unsigned_reviews));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join("; "))
    }
}
