//! The six S9/S20/S24-owned marts (`crates/canon-store/sql/views.sql`'s
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

pub const FLYWHEEL_FUNNEL_COLUMNS: &[&str] = &["role", "verdicts", "distilled", "retrieved", "applied"];

pub fn fetch_flywheel_funnel(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, "mart_flywheel_funnel", "role", FLYWHEEL_FUNNEL_COLUMNS)
}

pub const REVIEW_BURNDOWN_COLUMNS: &[&str] =
    &["day", "evidence_faithful", "evidence_divergent", "evidence_not_applicable", "divergence_opened", "divergence_resolved", "divergence_open_running_total"];

pub fn fetch_review_burndown(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, "mart_review_burndown", "day", REVIEW_BURNDOWN_COLUMNS)
}

pub const SCOPE_STATUS_COLUMNS: &[&str] = &["task_id", "scenario_id", "task_status", "evidence_covered", "green", "spec_covered"];

/// `mart_scope_status` (s20 `task-scenario-join`, surfaced by s24): one
/// row per declared `(task_id, scenario_id)` pair, unifying `task_status`
/// (done — the checkbox) x `evidence_covered`/`green` (verified — evidence-
/// side) x `spec_covered` (scenario-authored — spec-side). Exactly the
/// view's own `SELECT` list (`crates/canon-store/sql/views.sql:271-287`),
/// no renaming/reordering (design D1).
pub fn fetch_scope_status(roots: &Roots) -> Result<MartResult, ReportError> {
    fetch(roots, "mart_scope_status", "task_id, scenario_id", SCOPE_STATUS_COLUMNS)
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
