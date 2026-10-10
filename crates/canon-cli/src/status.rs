//! `canon status [--repo <dir>] [--json]`: where this repo stands and
//! what to do next.
//!
//! `canon context` answers what CAN be authored; this answers what HAS
//! been: each subject's scenario, evidence and review counts, its open
//! blockers and missing required cases, the scenarios no subject owns,
//! and a short `next:` list of concrete commands.
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
//! # Failing closed
//! Status never suggests progress the gate would refuse. A corpus kind
//! routed away from the rung status reads (`scenario`, `subject`,
//! `review`, `finding`) would read as empty, so status then lists only
//! the routing repair, like the ship gate and `spec_coverage` refuse
//! rather than judge an empty corpus. An unusable `spec_coverage`
//! section is a blocking gap: the policy repair comes first and no
//! subject is suggested to move on until `canon gate check` can run.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use canon_gate::policy::subject_status_name;
use canon_gate::review_gate::{open_blockers, review_gap};
use canon_gate::spec_coverage::{case_gaps, latest_scenarios, latest_subjects, subject_scenarios};
use canon_gate::review_gate::active_require_review;
use canon_gate::{latest_verdicts, CellKey, GateContext, GateCtx, LedgerEntry, PolicyDiagnostic, PolicyResolution, RequireReview, SpecCoverage};
use canon_model::{EvidenceVerdict, Finding, ProjectId, RecordKind, Scenario, Subject, SubjectStatus};
use canon_policy::SchemaRegistry;
use chrono::Utc;
use serde::Serialize;

use crate::context::{resolve_repo_root, spec_coverage_surface, SpecCoverageSurface};
use crate::inventory::load_spec_roots;

/// `canon status --json`'s shape version. Bumped when a field changes
/// meaning or is removed; an added field does not bump it.
const STATUS_VERSION: u32 = 1;

/// The most `next:` entries status prints.
pub const NEXT_LIMIT: usize = 5;

/// Lifecycle order: how subjects are grouped in the report.
const DISPLAY_ORDER: [SubjectStatus; 6] =
    [SubjectStatus::Proposed, SubjectStatus::Specced, SubjectStatus::Building, SubjectStatus::Verifying, SubjectStatus::Shipped, SubjectStatus::Retired];

/// Closest to done first: the order the `next:` rules visit subjects.
const NEXT_ORDER: [SubjectStatus; 5] = [SubjectStatus::Verifying, SubjectStatus::Building, SubjectStatus::Specced, SubjectStatus::Proposed, SubjectStatus::Shipped];

/// The whole report, in the order it prints.
#[derive(Debug, Clone, Serialize)]
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
    pub next: Vec<NextStep>,
    /// How many further `next` entries the rules produced past
    /// [`NEXT_LIMIT`].
    pub next_omitted: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicyStatus {
    /// Whether `.canon/policy.yaml` exists.
    pub present: bool,
    /// The effective `spec_coverage` section, keyed like `policy.yaml`
    /// (the same shape `canon context --json` prints); `null` when absent.
    pub spec_coverage: Option<SpecCoverageSurface>,
}

#[derive(Debug, Clone, Serialize)]
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
    /// Counted scenarios with a qualifying review under the
    /// `require_review` rule (any review record when it is off).
    pub reviewed: usize,
    /// Whether `require_review` covers this subject's status or the next
    /// one on the chain, i.e. whether its reviews are due now.
    pub review_due: bool,
    /// Open blocker findings on the subject's adopted changes.
    pub open_blockers: usize,
    pub missing_cases: Vec<MissingCase>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingCase {
    /// The `specs.roots[]` id the surface lives under.
    pub project_id: String,
    /// `<area>.<surface>`.
    pub surface: String,
    pub case: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NextStep {
    pub command: String,
    pub why: String,
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
pub fn run(repo: &Path, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let report = resolve(&repo);
    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap_or_else(|e| format!("{{\"error\": \"failed to serialize status: {e}\"}}")));
    } else {
        print!("{}", render(&report));
    }
    0
}

/// Build the report for an already-resolved repo root.
pub fn resolve(repo: &Path) -> StatusReport {
    let registry = SchemaRegistry::load();
    let mut warnings = Vec::new();
    let mut next = Vec::new();

    if !repo.join("canon.yaml").is_file() {
        warnings.push(format!("no canon.yaml at {} — canon is not set up in this repo", repo.display()));
        next.push(NextStep { command: "canon init".into(), why: "set canon up here".into() });
    }

    let loaded = GateCtx::from_repo(repo).map_err(|e| e.to_string()).and_then(|ctx| GateContext::load(ctx, &registry, Utc::now()).map_err(|e| e.to_string()));
    let ctx = match loaded {
        Ok(ctx) => ctx,
        Err(e) => {
            // The ledger cannot be read, so nothing below can be counted.
            let policy = PolicyResolution::resolve(repo, &registry);
            warnings.extend(policy_warning(&policy));
            warnings.push(format!("cannot read this repo's ledger: {e}"));
            next.push(NextStep { command: "canon init --check-config".into(), why: "canon.yaml must load before anything can be counted".into() });
            return finish(&policy, warnings, Vec::new(), Vec::new(), next);
        }
    };

    warnings.extend(policy_warning(&ctx.policy));
    let routed_away: Vec<&str> = ctx
        .unreadable_kinds
        .iter()
        .filter(|kind| matches!(kind, RecordKind::Scenario | RecordKind::Subject | RecordKind::Review | RecordKind::Finding))
        .map(|kind| kind.as_str())
        .collect();
    for kind in &routed_away {
        warnings.push(format!("`{kind}` routes away from the `local` rung status reads, so its records are not counted"));
    }
    let unreadable = ctx.violations.len() + ctx.corpus_violations.len();
    if unreadable > 0 {
        warnings.push(format!("{unreadable} ledger record(s) could not be read and are not counted; `canon gate check` names them"));
    }

    let joins = Joins { verdicts: latest_verdicts(&ctx), blockers: open_blockers(&ctx) };
    let subjects = latest_subjects(&ctx);
    let roots: Vec<ProjectId> = load_spec_roots(&repo.join("canon.yaml")).map(|roots| roots.into_iter().map(|r| r.id).collect()).unwrap_or_default();
    let mut summaries: Vec<(&Subject, SubjectSummary)> = subjects.iter().map(|s| (*s, summarize(&ctx, &joins, s))).collect();
    summaries.sort_by_key(|(s, _)| (rank(&DISPLAY_ORDER, s.status), s.subject_id.as_str().to_string()));

    let scenarios = latest_scenarios(&ctx);
    let unowned: Vec<&Scenario> = scenarios.iter().copied().filter(|s| s.subject_id.is_none()).collect();
    let known: BTreeSet<&str> = subjects.iter().map(|s| s.subject_id.as_str()).collect();
    let mut dangling: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for scenario in &scenarios {
        if let Some(id) = scenario.subject_id.as_ref().map(|id| id.as_str()).filter(|id| !known.contains(*id)) {
            dangling.entry(id).or_default().push(scenario.scenario_id.as_str());
        }
    }

    // ── fail closed: a corpus read as empty for a routing reason ──
    if !routed_away.is_empty() {
        let settings = routed_away.iter().map(|kind| format!("routing.{kind}: local")).collect::<Vec<_>>().join(", ");
        next.push(NextStep {
            command: "canon init --check-config".into(),
            why: format!(
                "set `{settings}` in canon.yaml, then check it: status and the gate read only the local rung, so every count and step would rest on an empty corpus"
            ),
        });
        let summaries = summaries.into_iter().map(|(_, summary)| summary).collect();
        let unowned = unowned.iter().map(|s| s.scenario_id.as_str().to_string()).collect();
        return finish(&ctx.policy, warnings, summaries, unowned, next);
    }

    // ── next: global rules ──
    // An unusable `spec_coverage` makes `canon gate check` refuse, so it
    // blocks every subject: repair it first, and suggest no status move.
    let policy_blocks = match &ctx.policy.spec_coverage {
        Some(SpecCoverage::Invalid { detail }) => {
            next.push(NextStep {
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
            command: format!("canon subject new {id} --domain <domain> --title \"<title>\""),
            why: format!("{} scenario(s) are tagged @subject:{id}, which no subject record carries", tagged.len()),
        });
    }
    if subjects.is_empty() && dangling.is_empty() {
        next.push(NextStep {
            command: "canon subject new <id> --domain <domain> --title \"<title>\"".into(),
            why: "no subject exists yet; a subject is the unit status, review and shipping track".into(),
        });
    }

    // ── next: per-subject rules, closest to done first ──
    // An empty scenario corpus under existing subjects is almost always an
    // index never built (scenario records are derived and often
    // gitignored), not a dozen subjects with no spec: say that once
    // instead of one "owns no scenario" step per subject.
    if scenarios.is_empty() && !subjects.is_empty() {
        warnings.push("the ledger holds no scenario records, so every subject reads as owning none".into());
        next.push(NextStep { command: "canon inventory sync".into(), why: "index the .feature corpus into scenario records; status counts nothing until then".into() });
    } else {
        let mut by_priority: Vec<&(&Subject, SubjectSummary)> = summaries.iter().filter(|(s, _)| NEXT_ORDER.contains(&s.status)).collect();
        by_priority.sort_by_key(|(s, _)| (rank(&NEXT_ORDER, s.status), s.subject_id.as_str().to_string()));
        for (subject, summary) in by_priority {
            subject_steps(&ctx, &joins, &scenarios, &roots, !policy_blocks, subject, summary, &mut next);
        }
    }

    if let Some(first) = unowned.first() {
        next.push(NextStep {
            command: "canon inventory sync".into(),
            why: format!(
                "{} scenario(s) carry no @subject tag (first: {}); add `@subject:<id>` above each in its .feature file, then sync",
                unowned.len(),
                first.scenario_id.as_str()
            ),
        });
    }

    let summaries = summaries.into_iter().map(|(_, summary)| summary).collect();
    let unowned = unowned.iter().map(|s| s.scenario_id.as_str().to_string()).collect();
    finish(&ctx.policy, warnings, summaries, unowned, next)
}

fn finish(policy: &PolicyResolution, warnings: Vec<String>, subjects: Vec<SubjectSummary>, unowned: Vec<String>, mut next: Vec<NextStep>) -> StatusReport {
    let next_omitted = next.len().saturating_sub(NEXT_LIMIT);
    next.truncate(NEXT_LIMIT);
    StatusReport {
        status_version: STATUS_VERSION,
        canon_version: env!("CARGO_PKG_VERSION"),
        policy: PolicyStatus {
            present: !policy.diagnostics.iter().any(|d| matches!(d, PolicyDiagnostic::Missing { .. })),
            spec_coverage: spec_coverage_surface(policy),
        },
        warnings,
        subjects,
        unowned,
        next,
        next_omitted,
    }
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
    verdicts: BTreeMap<CellKey, LedgerEntry>,
    blockers: Vec<&'a Finding>,
}

impl Joins<'_> {
    /// Whether `scenario` has a ledger verdict, and whether any role's
    /// latest one is divergent — the `verifying → shipped` gate's reading.
    fn verdict_state(&self, scenario: &Scenario) -> (bool, bool) {
        let id = scenario.scenario_id.as_str();
        let mut entries = self.verdicts.iter().filter(|((subject, _), _)| subject == id).map(|(_, entry)| entry).peekable();
        let evidenced = entries.peek().is_some();
        (evidenced, entries.any(|e| e.verdict == EvidenceVerdict::Divergent))
    }

    /// Open blocker findings on `subject`'s adopted changes.
    fn blockers_of(&self, subject: &Subject) -> Vec<&Finding> {
        self.blockers.iter().copied().filter(|f| subject.change_ids.contains(&f.change_id)).collect()
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

fn summarize(ctx: &GateContext, joins: &Joins, subject: &Subject) -> SubjectSummary {
    let (counted, excluded) = counted(ctx, subject);
    let (_, require_cases) = coverage_settings(ctx);
    let rr = review_rule(ctx);
    let distinct_actor = rr.is_some_and(|rr| rr.distinct_actor);

    let mut evidenced = 0;
    let mut divergent = 0;
    for scenario in &counted {
        let (has, diverged) = joins.verdict_state(scenario);
        evidenced += usize::from(has);
        divergent += usize::from(diverged);
    }
    let reviewed = counted.iter().filter(|s| review_gap(ctx, s, distinct_actor).is_none()).count();
    let open_blockers = joins.blockers_of(subject).len();
    let missing_cases = case_gaps(counted.iter().copied(), require_cases)
        .into_iter()
        .map(|gap| MissingCase { project_id: gap.project_id.as_str().to_string(), surface: gap.surface, case: gap.case })
        .collect();

    SubjectSummary {
        id: subject.subject_id.as_str().to_string(),
        title: subject.title.clone(),
        status: subject_status_name(subject.status),
        scenarios: counted.len(),
        excluded,
        evidenced,
        divergent,
        reviewed,
        review_due: rr.is_some_and(|rr| rr.covers(subject.status) || next_status(subject.status).is_some_and(|n| rr.covers(n))),
        open_blockers,
        missing_cases,
    }
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

/// The per-subject `next:` rules (module doc), each producing at most one
/// step. Gap rules come first; the status advance only when none fired
/// and `may_advance` (no blocking policy gap).
#[allow(clippy::too_many_arguments)]
fn subject_steps(
    ctx: &GateContext,
    joins: &Joins,
    all: &[&Scenario],
    roots: &[ProjectId],
    may_advance: bool,
    subject: &Subject,
    summary: &SubjectSummary,
    next: &mut Vec<NextStep>,
) {
    let id = subject.subject_id.as_str();
    let status = subject.status;
    let before = next.len();

    if summary.scenarios + summary.excluded == 0 {
        let roots_note = if roots.len() > 1 { format!(" (spec roots: {})", roots.iter().map(ProjectId::as_str).collect::<Vec<_>>().join(", ")) } else { String::new() };
        next.push(NextStep {
            command: format!("canon scenario new <area>.<surface>.01 --title \"<behavior>\" --subject {id} --case happy{}", project_flag(roots, None)),
            why: format!("{id} owns no scenario; specify its behavior, then `canon inventory sync`{roots_note}"),
        });
        return;
    }

    if let Some(gap) = summary.missing_cases.first() {
        next.push(NextStep {
            command: format!(
                "canon scenario new {} --title \"<what happens on this path>\" --subject {id} --case {}{}",
                next_number(all, &gap.project_id, &gap.surface),
                gap.case,
                project_flag(roots, Some(&gap.project_id))
            ),
            why: format!("{id}: {} surface(s) lack a @case:{} scenario (first: {}); add one, never retag an existing scenario", summary.missing_cases.len(), gap.case, gap.surface),
        });
    }

    let attested = matches!(status, SubjectStatus::Building | SubjectStatus::Verifying | SubjectStatus::Shipped);
    let (counted, _) = counted(ctx, subject);
    if attested {
        if let Some(first) = counted.iter().find(|s| !joins.verdict_state(s).0) {
            next.push(NextStep {
                command: format!(
                    "canon evidence add --scenario-id {} --project-id {} --kind test-run --role implementer --session-id <session> --verdict faithful --ref \"<test command>\"",
                    first.scenario_id.as_str(),
                    first.project_id.as_str()
                ),
                why: format!("{id}: {} of {} scenario(s) have no evidence; attest each, then `canon gate promote`", summary.scenarios - summary.evidenced, summary.scenarios),
            });
        }
        if let Some(first) = counted.iter().find(|s| joins.verdict_state(s).1) {
            next.push(NextStep {
                command: format!(
                    "canon evidence add --scenario-id {} --project-id {} --kind test-run --role implementer --session-id <session> --verdict faithful --ref \"<test command>\"",
                    first.scenario_id.as_str(),
                    first.project_id.as_str()
                ),
                why: format!("{id}: {} scenario(s) have a divergent latest verdict; fix the behavior, then re-attest", summary.divergent),
            });
        }
    }

    if let Some(blocker) = joins.blockers_of(subject).first() {
        next.push(NextStep {
            command: format!(
                "canon finding close --change-id {} --round {} --seq {} --disposition fixed --resolution-sha <sha>",
                blocker.change_id.as_str(),
                blocker.round,
                blocker.seq
            ),
            why: format!("{id}: {} open blocker finding(s) (first: {}); fix it, commit, then close it", summary.open_blockers, blocker.summary),
        });
    }

    if summary.review_due {
        let distinct_actor = review_rule(ctx).is_some_and(|rr| rr.distinct_actor);
        if let Some(first) = counted.iter().find(|s| joins.verdict_state(s).0 && review_gap(ctx, s, distinct_actor).is_some()) {
            next.push(NextStep {
                command: format!(
                    "canon review add --project-id {} --scenario-id {} --reviewer <reviewer> --actor-id <reviewer> --session-id <review-session> --role reviewer --pin <sha> --original-spec-ref <feature file>",
                    first.project_id.as_str(),
                    first.scenario_id.as_str()
                ),
                why: format!(
                    "{id}: {} of {} scenario(s) lack a qualifying review; review from another session (--session-id must differ from every evidence session), as an actor other than the evidence actor",
                    summary.scenarios - summary.reviewed,
                    summary.scenarios
                ),
            });
        }
    }

    // Every gap rule above was silent: the subject may move on (shipped
    // and retired have nowhere to go), unless a policy gap blocks it.
    if next.len() > before || !may_advance {
        return;
    }
    if let Some(to) = next_status(status) {
        let to = subject_status_name(to);
        next.push(NextStep {
            command: format!("canon subject status {id} {to}"),
            why: match status {
                SubjectStatus::Proposed | SubjectStatus::Specced => format!("{id} has its scenarios and every required case; move it on"),
                _ => format!("{id}: every scenario is evidenced{}, with no blocker or case gap", if summary.review_due { " and reviewed" } else { "" }),
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
        (true, Some(SpecCoverageSurface::Active { require_evidence, scope, exclude_lanes, require_cases, require_review })) => {
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
        }
    }
    for warning in &report.warnings {
        let _ = writeln!(out, "warning: {warning}");
    }

    let _ = writeln!(out);
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
            let _ = write!(line, ", {} open blockers", s.open_blockers);
            let _ = writeln!(out, "    {}: {line}", s.id);
            for gap in &s.missing_cases {
                let _ = writeln!(out, "      missing @case:{} on {}", gap.case, gap.surface);
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

    let _ = writeln!(out);
    if report.next.is_empty() {
        let _ = writeln!(out, "next: nothing — every subject is shipped or retired and every scenario has a subject");
    } else {
        let _ = writeln!(out, "next:");
        for step in &report.next {
            let _ = writeln!(out, "  # {}", step.why);
            let _ = writeln!(out, "  {}", step.command);
        }
        if report.next_omitted > 0 {
            let _ = writeln!(out, "  … {} more once these are done", report.next_omitted);
        }
    }
    out
}

fn list_or(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        empty.to_string()
    } else {
        values.join(", ")
    }
}
