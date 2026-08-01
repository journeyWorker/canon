//! The nine S9/S20/S24/S36/s43-owned marts (`crates/canon-store/sql/views.sql`'s
//! addition" section, design D5) — one [`MartSpec`] per panel, each a
//! bare `SELECT * FROM mart_x ORDER BY …`. No aggregation happens
//! here: every number this module returns is exactly what the DuckDB
//! view already computed (design D1) — this is a thin, ordered-column
//! typed wrapper over [`crate::query::Row`], nothing more.
//!
//! [`fetch_all`] is the read a report uses: all nine statements in one
//! pinned batch, so no two panels can be computed from different
//! corpora. The per-mart `fetch_*` functions run one statement each
//! and exist for callers reading a SINGLE number, where there is
//! nothing to be inconsistent with.

use crate::error::ReportError;
use crate::query::{self, Row};
use crate::roots::Roots;

/// One mart's result: its declared column order (for stable markdown
/// rendering — `serde_json::Map`'s own key order is NOT relied upon;
/// this workspace enables no `preserve_order` feature anywhere,
/// verified across every `Cargo.toml`, 2026-07-11) plus every row
/// `-json` mode returned.
pub struct MartResult {
    pub columns: &'static [&'static str],
    pub rows: Vec<Row>,
}

/// One mart's declared read: the view, the total `ORDER BY` that makes
/// its row order stable, and the column order the markdown table
/// renders. [`MartSpec::sql`] is the ONLY place a mart's statement is
/// built, so the batched all-marts read ([`fetch_all`]) and a targeted
/// single-mart read cannot issue different SQL for the same panel.
#[derive(Clone, Copy)]
pub struct MartSpec {
    pub view: &'static str,
    pub order_by: &'static str,
    pub columns: &'static [&'static str],
}

impl MartSpec {
    pub fn sql(&self) -> String {
        format!("SELECT * FROM {} ORDER BY {};", self.view, self.order_by)
    }
}

fn fetch(roots: &Roots, spec: &MartSpec) -> Result<MartResult, ReportError> {
    let rows = query::run_query(roots, &spec.sql())?;
    Ok(MartResult { columns: spec.columns, rows })
}

pub const TRUST_MATRIX_COLUMNS: &[&str] = &["change_id", "task_id", "title", "task_status", "covered", "green", "who", "evidence_count"];

pub const TRUST_MATRIX: MartSpec = MartSpec { view: "mart_trust_matrix", order_by: "change_id, task_id", columns: TRUST_MATRIX_COLUMNS };

pub fn fetch_trust_matrix(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, &TRUST_MATRIX)
}

pub const SESSION_COSTS_COLUMNS: &[&str] =
    &["session_id", "client", "role", "workspace_label", "run_count", "total_cost", "total_tokens", "first_event_at", "last_event_at"];

pub const SESSION_COSTS: MartSpec = MartSpec { view: "mart_session_costs", order_by: "session_id", columns: SESSION_COSTS_COLUMNS };

pub fn fetch_session_costs(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, &SESSION_COSTS)
}

pub const ROLE_MEMORY_COLUMNS: &[&str] =
    &["role", "regime_key", "strategy_count", "active_count", "demoted_count", "hit_rate", "avg_source_trajectories", "latest_recorded_at"];

pub const ROLE_MEMORY: MartSpec = MartSpec { view: "mart_role_memory", order_by: "role, regime_key", columns: ROLE_MEMORY_COLUMNS };

pub fn fetch_role_memory(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, &ROLE_MEMORY)
}

pub const FLYWHEEL_FUNNEL_COLUMNS: &[&str] =
    &["role", "verdicts", "distilled", "retrieved", "applied", "applied_attributed", "applied_proxy"];

pub const FLYWHEEL_FUNNEL: MartSpec = MartSpec { view: "mart_flywheel_funnel", order_by: "role", columns: FLYWHEEL_FUNNEL_COLUMNS };

/// `mart_flywheel_funnel`: one row per role, `verdicts → distilled →
/// retrieved → applied`, with `applied` broken out by the RULE that
/// admitted each count. What each column counts, restated here because
/// the bare names read like a funnel long before the SQL actually
/// computed one (s40 `plan-vs-actual-diff`, tasks 3.1/3.2; s42
/// `close-the-open-loops`, task 3.3):
///
/// - `verdicts` — `VerdictRow`s across this role's raw trajectories,
///   the evidence the distiller consumes. The one stage not counted in
///   strategies, and the natural upper bound on `distilled` — strictly
///   above it when a trajectory repeats a byte-identical verdict,
///   which distils to one strategy rather than two indistinguishable
///   copies of it.
/// - `distilled` — this role's `canon-learn` `StrategyItem` rows.
/// - `retrieved` — DISTINCT strategies appearing in at least one
///   `Run::injected_guidance` AND still present as a distilled row,
///   never injection EVENTS: counting events let `retrieved` exceed
///   `distilled` as soon as one strategy was injected into two runs.
///   The "still present" half is not a leak: `canon-learn`'s
///   `StrategyId` is derived from a strategy's own content, so
///   `rebuild_namespace` re-derives the same id and a recorded
///   `StrategyRef` keeps resolving across an ordinary re-ingest. Only
///   a strategy re-derived from CHANGED evidence gets a new id and
///   leaves this stage — correctly, since the cited strategy no longer
///   exists.
/// - `applied` — that same distinct set, restricted to strategies whose
///   recipient run has SOMETHING recorded about how it ended, under
///   exactly one of the two rules below. Deliberately NOT "resolved
///   trajectories", which is what it counted before s40, with no
///   reference to retrieval at all.
/// - `applied_attributed` / `applied_proxy` — WHICH rule admitted each
///   part of `applied`, because a column that silently mixes two rules
///   asserts more than its data carries. Both are CO-OCCURRENCE within
///   one run, never causation.
///   `attributed` is s40 task 3.1's original wording, finally
///   implementable in s42: the strategy appears in some run's
///   `injected_guidance`, and that SAME run has at least one
///   trajectory of the SAME role stamped with that run's own id
///   (`canon_learn::Trajectory::run_id`, set only by an explicit
///   `canon ingest artifacts --run`) whose `outcome` is one of the
///   resolved variants (`success`/`failure`/`rolled-back`).
///   `proxy` is s40's retained fallback when no such trajectory
///   exists: all that is recorded is the recipient run's own terminal
///   `Run.status`.
///
///   Attribution is the STRONGER of the two — it needs a judged
///   outcome out of that run, not merely that the run reached a
///   terminal state — and is still WEAKER than attributing the
///   outcome to the guidance. The join is `(run_id, role)` and nothing
///   else, so every still-distilled strategy of that role injected
///   into that run is admitted alike, whether it was followed or
///   ignored; no record kind carries an edge from a `StrategyId` to
///   the verdict decided alongside it. Making the causal claim would
///   require adding that edge — a `StrategyId` stamped on
///   `Trajectory`/`VerdictRow` at judgment time, joined here in place
///   of `run_id` alone — not re-wording this column (s42
///   `close-the-open-loops` re-review; the same defect class as s39's
///   model-level ceiling, s40's funnel columns and s41's burn-down).
///
///   The two PARTITION `applied` — `applied == applied_attributed +
///   applied_proxy` — because the view assigns each counted strategy
///   exactly one rule, attribution winning.
///
/// So `applied <= retrieved <= distilled` holds by construction (the
/// view's own comment carries the proof). Exactly the view's own
/// `SELECT` list, no renaming/reordering (design D1).
pub fn fetch_flywheel_funnel(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, &FLYWHEEL_FUNNEL)
}

pub const REVIEW_BURNDOWN_COLUMNS: &[&str] =
    &["day", "evidence_faithful", "evidence_divergent", "evidence_not_applicable", "divergence_opened", "divergence_resolved", "divergence_open_running_total"];

pub const REVIEW_BURNDOWN: MartSpec = MartSpec { view: "mart_review_burndown", order_by: "day", columns: REVIEW_BURNDOWN_COLUMNS };

pub fn fetch_review_burndown(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, &REVIEW_BURNDOWN)
}

pub const SCOPE_STATUS_COLUMNS: &[&str] =
    &["task_id", "scenario_id", "task_status", "evidence_covered", "green", "spec_project_id", "spec_covered"];

pub const SCOPE_STATUS: MartSpec =
    MartSpec { view: "mart_scope_status", order_by: "task_id, scenario_id, spec_project_id", columns: SCOPE_STATUS_COLUMNS };

/// `mart_scope_status` (s20 `task-scenario-join`, surfaced by s24): one
/// row per declared `(task_id, scenario_id)` pair PER COVERING PROJECT,
/// unifying `task_status` (done — the checkbox) x
/// `evidence_covered`/`green` (verified — evidence-side) x
/// `spec_project_id`/`spec_covered` (scenario-authored — spec-side).
///
/// `spec_project_id` names WHOSE coverage each row reports.
/// `porting.coverage` is keyed `(project_id, scenario_id)` while
/// `Task::scenario_refs` carries no project, so two spec roots
/// authoring one scenario id give a declared pair two coverage answers
/// and the view emits both rather than picking one arbitrarily (the
/// view's own GRAIN note carries the full argument). NULL means no
/// overlay row exists for that scenario at all — distinct from an
/// overlay row present and saying `spec_covered = false`.
///
/// The `ORDER BY` includes `spec_project_id` for the same reason: with
/// more than one row per pair, `(task_id, scenario_id)` alone is no
/// longer a total order and the rendered table's row order would not be
/// stable. Exactly the view's own `SELECT` list, no
/// renaming/reordering (design D1).
pub fn fetch_scope_status(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, &SCOPE_STATUS)
}

pub const SUBJECTS_COLUMNS: &[&str] = &["domain", "subject_id", "title", "status", "scenario_count", "covered_scenarios"];

pub const SUBJECTS: MartSpec = MartSpec { view: "mart_subjects", order_by: "domain, subject_id", columns: SUBJECTS_COLUMNS };

/// `mart_subjects` (s36 `subject-domain-loop`): the per-domain subject
/// rollup — one row per `subject` record (the reviewed 13th kind),
/// `domain`/`subject_id`/`title`/`status` plus `scenario_count` (how
/// many `scenario_ids` the subject links) x `covered_scenarios` (how
/// many carry a latest non-Divergent evidence verdict, the same
/// last-wins-by-`at` fold `mart_trust_matrix`'s `green` uses).
/// Read-only reporting, never a `canon-gate` input. A missing/empty
/// subject corpus yields zero rows, never an error. Exactly the view's
/// own `SELECT` list, no renaming/reordering (design D1).
pub fn fetch_subjects(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, &SUBJECTS)
}

pub const REVIEW_ROUNDS_COLUMNS: &[&str] = &[
    "change_id",
    "round",
    "reviewed_sha",
    "findings",
    "severity_blocker",
    "severity_should_fix",
    "severity_note",
    "disposition_open",
    "disposition_fixed",
    "disposition_rejected",
    "disposition_deferred",
    "fix_of_fix",
    "introduced_by_sourced",
    "introduced_by_unsourced",
];

pub const REVIEW_ROUNDS: MartSpec = MartSpec { view: "mart_review_rounds", order_by: "change_id, \"round\"", columns: REVIEW_ROUNDS_COLUMNS };

/// `mart_review_rounds` (s43 `findings-are-records`): one row per
/// `(change_id, round)` over the fourteenth kind,
/// [`canon_model::records::Finding`] — the reviewed sha, the finding
/// count, the per-severity and per-disposition splits, and the DERIVED
/// fix-of-fix count. Exactly the view's own `SELECT` list, no
/// renaming/reordering (design D1); every number is the view's.
///
/// The three columns whose bare names would otherwise assert more than
/// the query computes:
///
/// - `reviewed_sha` — `max()` over the round's findings, because
///   `reviewed_sha` is per-finding provenance and the natural key
///   (`{change_id}__{round:04}__{seq:04}`) does not include it. NULL
///   means no finding in the round recorded one, and nothing more. A
///   round that reviewed an uncommitted working tree is the usual
///   reason and the reason the field is `Option` at all, but the view
///   cannot tell that from a round whose findings simply left the
///   field unset — so a blank is not evidence of a worktree review.
///   (Every other surface already said so; this doc was the one that
///   still read the NULL as the worktree case — s43 round 6, the same
///   shape as finding 2's numbering gap.)
/// - `fix_of_fix` — findings in this round whose SOURCED
///   `introduced_by` equals the `resolution_sha` of a finding EARLIER
///   in the SAME change, ordered strictly by the natural key's own
///   `(round, seq)` pair. That is a commit-id equality join and
///   nothing more: it reports that the commit which closed an earlier
///   finding is the commit a later finding RECORDS as its introducing
///   commit. Whether that record is right is a property of how the
///   author sourced `introduced_by`, which canon requires to be
///   sourced and never infers — so this column inherits the corpus's
///   sourcing discipline and establishes no causal claim of its own.
///   Strictness is what stops a finding matching its own resolution;
///   the same-change scope is what keeps `(round, seq)` meaningful,
///   since `round` restarts at 1 per change.
/// - `introduced_by_unsourced` — the UNKNOWN bucket. `introduced_by`
///   is `None` when the introducing commit could not be sourced, so
///   such a finding is never counted as NOT-a-fix-of-fix; it is
///   counted as unknown, here. `introduced_by_sourced +
///   introduced_by_unsourced = findings` and `fix_of_fix <=
///   introduced_by_sourced`, both by construction, so the size of the
///   unknown is readable against the count.
///
/// What that count MEANS is one sentence, stated here exactly as every
/// other surface states it ([`crate::render::FIX_OF_FIX_MEANING`],
/// whose own doc records what each surface used to say instead — this
/// one asserted a one-directional bound, s43 round 2 finding 5):
///
/// `fix_of_fix` bounds NOTHING — not from below, not from above: it
/// UNDER-counts, because an unsourced finding is never counted and a
/// fix in one change that breaks something first found while reviewing
/// a DIFFERENT change is not counted at all; it OVER-counts, because a
/// `resolution_sha` commit may carry work BEYOND the fix and every
/// finding recording that commit is counted regardless; and for any
/// individual match the data cannot say whether the fix or the other
/// work in that commit introduced the defect.
pub fn fetch_review_rounds(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, &REVIEW_ROUNDS)
}

pub const REVIEW_TOTALS_COLUMNS: &[&str] = &[
    "change_id",
    "rounds_recorded",
    "highest_round",
    "findings",
    "severity_blocker",
    "severity_should_fix",
    "severity_note",
    "disposition_open",
    "disposition_fixed",
    "disposition_rejected",
    "disposition_deferred",
    "fix_of_fix",
    "introduced_by_sourced",
    "introduced_by_unsourced",
];

pub const REVIEW_TOTALS: MartSpec = MartSpec { view: "mart_review_totals", order_by: "change_id", columns: REVIEW_TOTALS_COLUMNS };

/// `mart_review_totals` (s43 `findings-are-records`, round 5): one row
/// per `change_id` — the per-change total of everything
/// [`fetch_review_rounds`] reports per round. Exactly the view's own
/// `SELECT` list, no renaming/reordering (design D1); every number is
/// the view's.
///
/// s43 exists so a release narrative's issue count is DERIVED rather
/// than typed, and `mart_review_rounds` alone did not finish the job:
/// the per-change sentence a release note actually contains ("N
/// findings over R rounds") still meant adding the per-round table up
/// by hand, which is the operation that put four wrong numbers into
/// this release line's published notes. This view is the number, so a release note is a
/// COPY rather than a computation.
///
/// The view's only `FROM` is `mart_review_rounds`: every column is a
/// `sum()`/`count(*)`/`max()` over the rows a reader is looking at,
/// never a second aggregate over `finding_latest`. One number, one
/// implementation — two places to compute it is how this class
/// recurs. Nothing is summed on the Rust side either; this function
/// selects and renders.
///
/// The three columns whose bare names would otherwise assert more than
/// the query computes:
///
/// - `rounds_recorded` — `count(*)` over `mart_review_rounds` rows,
///   which exist only for rounds that RECORDED a finding. A round that
///   found nothing wrote no `Finding` and is in neither table, so this
///   is the count of rounds that found something, never the rounds
///   RUN — and canon cannot supply the latter from any view, since
///   `Review` is a per-scenario attestation and no record kind marks a
///   round as run.
/// - `highest_round` — `max("round")` over the same rows: the greatest
///   round NUMBER that recorded a finding, and it witnesses nothing
///   about rounds. Exceeding `rounds_recorded` says only that some
///   round number below it has no row, and this view cannot say why:
///   `round` is author-supplied, nothing requires a change's rounds
///   to be numbered from 1 or without gaps, so a change whose only
///   finding is labelled round 7 shows the same gap six silent rounds
///   would. Neither the gap nor its absence evidences a round that
///   RAN. It is a round NUMBER, not a count — neither column is the
///   rounds-run count.
/// - `fix_of_fix` — `sum()` of the per-round derived counts. The
///   round view's `EXISTS` semi-join is already scoped to one
///   `change_id`, so summing changes the grain and nothing else.
///
/// What that count MEANS is one sentence, stated here exactly as every
/// other surface states it ([`crate::render::FIX_OF_FIX_MEANING`]):
///
/// `fix_of_fix` bounds NOTHING — not from below, not from above: it
/// UNDER-counts, because an unsourced finding is never counted and a
/// fix in one change that breaks something first found while reviewing
/// a DIFFERENT change is not counted at all; it OVER-counts, because a
/// `resolution_sha` commit may carry work BEYOND the fix and every
/// finding recording that commit is counted regardless; and for any
/// individual match the data cannot say whether the fix or the other
/// work in that commit introduced the defect.
///
/// No column here is a claim about the change: `findings` counts what
/// reviewers RECORDED, `severity_blocker` is the severity a reviewer
/// TYPED, and `disposition_rejected` records that a finding was
/// rejected rather than that it was wrong. There is no defect rate and
/// no quality score in this view.
pub fn fetch_review_totals(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, &REVIEW_TOTALS)
}

/// Every panel the report renders, in design D5's own declared order
/// — the SINGLE declaration of that order, which
/// [`crate::snapshot::SNAPSHOT_TABLES`] and [`crate::render::render`]
/// both follow (pinned by `report_marts_are_the_snapshot_tables` below
/// and by `packages/dashboard/test/panel-copy.test.ts`'s
/// rendered-order check).
pub const REPORT_MARTS: [MartSpec; 9] =
    [TRUST_MATRIX, SESSION_COSTS, ROLE_MEMORY, FLYWHEEL_FUNNEL, REVIEW_BURNDOWN, SCOPE_STATUS, SUBJECTS, REVIEW_ROUNDS, REVIEW_TOTALS];

/// The nine panels one report renders, fetched together.
///
/// Lives here rather than beside [`crate::render::render`] because the
/// nine are one READ before they are nine panels: [`fetch_all`] is the
/// only thing that builds this struct, and it builds it from one
/// corpus.
pub struct ReportMarts {
    pub trust_matrix: MartResult,
    pub session_costs: MartResult,
    pub role_memory: MartResult,
    pub flywheel_funnel: MartResult,
    pub review_burndown: MartResult,
    pub scope_status: MartResult,
    pub subjects: MartResult,
    pub review_rounds: MartResult,
    pub review_totals: MartResult,
}

/// Fetches all nine marts from ONE materialized read of the corpus, in
/// one `duckdb` process ([`query::run_pinned_queries`]).
///
/// This is what makes "the total and the rows it totals cannot
/// disagree" true of the RENDERED report and not merely of the SQL.
/// `mart_review_totals`' only `FROM` is `mart_review_rounds`, so the
/// two agree by construction over any one input — but nine
/// `run_query` calls are nine processes over a LIVE ledger, and a
/// finding written between two of them lands in the later panel and
/// not the earlier one. Pinning removes the between: a record written
/// during a report run reaches every panel or none.
///
/// The guarantee is BOTH-OR-NEITHER across panels, which is the
/// property a reader comparing two cells needs — and nothing beyond
/// it. s43 round 7 finding 5: this doc used to go further and claim
/// that in practice a mid-run write landed in NOTHING, since the pin
/// is taken before the first mart is computed. That is false — the
/// pin is taken HERE, and [`crate::report`] computes
/// [`crate::digest::DigestHeader`] BEFORE calling this, so a write
/// landing after the digest and before this call reaches every panel
/// while the header still describes the corpus before it.
/// Digest/panel skew is possible; panel/panel skew is not. Nor is
/// this a claim that the four physical sources were captured at one
/// instant — the pin materializes them with four statements.
pub fn fetch_all(roots: &Roots) -> Result<ReportMarts, ReportError> {
    let statements: Vec<String> = REPORT_MARTS.iter().map(MartSpec::sql).collect();
    let sets = query::run_pinned_queries(roots, &statements)?;

    // `run_pinned_queries` already rejected any other length, so the
    // zip is total and the binding order below is exactly
    // `REPORT_MARTS`' declaration order.
    let mut results = REPORT_MARTS.iter().zip(sets).map(|(spec, rows)| MartResult { columns: spec.columns, rows });
    let mut next = || results.next().expect("one MartResult per REPORT_MARTS entry");
    Ok(ReportMarts {
        trust_matrix: next(),
        session_costs: next(),
        role_memory: next(),
        flywheel_funnel: next(),
        review_burndown: next(),
        scope_status: next(),
        subjects: next(),
        review_rounds: next(),
        review_totals: next(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One declared panel order, three consumers. `fetch_all`'s batch,
    /// `--snapshot`'s exports and the rendered markdown all walk this
    /// list, and the dashboard's `panel-copy.test.ts` pins its own
    /// surfaces to `SNAPSHOT_TABLES` — so this assertion is the link
    /// that makes that pin cover the fetch as well.
    #[test]
    fn report_marts_are_the_snapshot_tables() {
        let views: Vec<&str> = REPORT_MARTS.iter().map(|spec| spec.view).collect();
        assert_eq!(views, crate::snapshot::SNAPSHOT_TABLES, "the fetch order and the snapshot order are one order");
    }

    /// The batch is parsed positionally, so a spec whose `columns` do
    /// not belong to its `view` would mislabel a whole panel without
    /// any query failing.
    #[test]
    fn every_spec_names_its_own_columns() {
        for spec in &REPORT_MARTS {
            let expected_prefix = spec.view.strip_prefix("mart_").expect("every mart view is `mart_`-prefixed");
            assert!(spec.sql().contains(spec.view), "{}: sql must select from its own view", spec.view);
            assert!(!spec.columns.is_empty(), "{expected_prefix}: a panel with no declared columns renders an empty table");
            assert!(!spec.order_by.is_empty(), "{expected_prefix}: a panel with no ORDER BY has no stable row order");
        }
    }
}
