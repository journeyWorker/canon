//! The eight S9/S20/S24/S36/s43-owned marts (`crates/canon-store/sql/views.sql`'s
//! addition" section, design D5) — one `fetch_*` per panel, each a
//! bare `SELECT * FROM mart_x ORDER BY …` against
//! [`crate::query::run_query`]. No aggregation happens here: every
//! number this module returns is exactly what the DuckDB view already
//! computed (design D1) — this is a thin, ordered-column typed wrapper
//! over [`crate::query::Row`], nothing more.

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

fn fetch(roots: &Roots, view: &str, order_by: &str, columns: &'static [&'static str]) -> Result<MartResult, ReportError> {
    let sql = format!("SELECT * FROM {view} ORDER BY {order_by};");
    let rows = query::run_query(roots, &sql)?;
    Ok(MartResult { columns, rows })
}

pub const TRUST_MATRIX_COLUMNS: &[&str] = &["change_id", "task_id", "title", "task_status", "covered", "green", "who", "evidence_count"];

pub fn fetch_trust_matrix(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, "mart_trust_matrix", "change_id, task_id", TRUST_MATRIX_COLUMNS)
}

pub const SESSION_COSTS_COLUMNS: &[&str] =
    &["session_id", "client", "role", "workspace_label", "run_count", "total_cost", "total_tokens", "first_event_at", "last_event_at"];

pub fn fetch_session_costs(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, "mart_session_costs", "session_id", SESSION_COSTS_COLUMNS)
}

pub const ROLE_MEMORY_COLUMNS: &[&str] =
    &["role", "regime_key", "strategy_count", "active_count", "demoted_count", "hit_rate", "avg_source_trajectories", "latest_recorded_at"];

pub fn fetch_role_memory(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, "mart_role_memory", "role, regime_key", ROLE_MEMORY_COLUMNS)
}

pub const FLYWHEEL_FUNNEL_COLUMNS: &[&str] =
    &["role", "verdicts", "distilled", "retrieved", "applied", "applied_attributed", "applied_proxy"];

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
    fetch(roots, "mart_flywheel_funnel", "role", FLYWHEEL_FUNNEL_COLUMNS)
}

pub const REVIEW_BURNDOWN_COLUMNS: &[&str] =
    &["day", "evidence_faithful", "evidence_divergent", "evidence_not_applicable", "divergence_opened", "divergence_resolved", "divergence_open_running_total"];

pub fn fetch_review_burndown(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, "mart_review_burndown", "day", REVIEW_BURNDOWN_COLUMNS)
}

pub const SCOPE_STATUS_COLUMNS: &[&str] =
    &["task_id", "scenario_id", "task_status", "evidence_covered", "green", "spec_project_id", "spec_covered"];

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
    fetch(roots, "mart_scope_status", "task_id, scenario_id, spec_project_id", SCOPE_STATUS_COLUMNS)
}

pub const SUBJECTS_COLUMNS: &[&str] = &["domain", "subject_id", "title", "status", "scenario_count", "covered_scenarios"];

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
    fetch(roots, "mart_subjects", "domain, subject_id", SUBJECTS_COLUMNS)
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
///   means no finding in the round recorded one, which is how canon
///   records a round that reviewed an uncommitted working tree — the
///   common case, and the reason the field is `Option` at all.
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
    fetch(roots, "mart_review_rounds", "change_id, \"round\"", REVIEW_ROUNDS_COLUMNS)
}
