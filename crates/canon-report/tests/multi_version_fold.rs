//! Acceptance: "a record stored as N versions at one natural key
//! contributes to a mart exactly ONCE" — the regression guard for
//! `crates/canon-store/sql/views.sql`'s "Multi-version records" header
//! section, and specifically for the cost double-count the s42
//! re-review found.
//!
//! Every canon tier is APPEND-ONLY at a natural key
//! (`canon_store::partition::resolve_partition`), so a state
//! transition is a SECOND physical row at the same key, not an
//! in-place update. `crate::query`'s Rust fold collapses those
//! versions; `views.sql` does not inherit that fold — `stg_records` is
//! a plain `UNION ALL` — so every mart that joins or aggregates has to
//! fold its own key. Before that fix, `canon dispatch begin`/`end`
//! storing two `Run` versions per dispatch multiplied
//! `mart_session_costs`' `total_cost` AND `total_tokens` by the
//! version count, silently: `run_count` is `count(DISTINCT run_id)`
//! and stayed correct, so a doubled bill read as an expensive run.
//!
//! Deliberately builds standalone per-scenario corpora rather than
//! reusing `fixtures/corpus.rs` — that shared fixture writes exactly
//! ONE version per natural key (which is why it never caught this),
//! and this crate's precedent (`session_costs_multi_workspace.rs`) is
//! one purpose-built corpus per documented scenario.
//!
//! The last test is a NEGATIVE guard: `mart_review_burndown` MUST keep
//! reading the raw version stream, because for a divergence the
//! transition IS the curve. It fails if someone "generalizes" the fold
//! into `stg_records` for every reader.

mod support;

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{ProjectId, RoleId, RunId, ScenarioId, Sha, SessionId, SubjectId, TaskId, TotalOrder};
use canon_model::records::{
    Divergence, DivergenceStatus, Event, EvidenceRecord, EvidenceVerdict, Run, RunStatus, Session, Subject, SubjectStatus, Task, TaskStatus,
};
use canon_report::marts;
use canon_report::query;
use canon_report::roots::Roots;
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;

fn at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, h, 0, 0).single().unwrap()
}

fn actor(role: &str) -> Actor {
    Actor::new("fold-fixture", RoleId::parse(role).unwrap())
}

/// One session, and ONE run persisted as the two versions `canon
/// dispatch begin`/`end` actually write: `Running` at begin, then
/// `Succeeded` re-stamped at a strictly greater `envelope.at`
/// (`canon-cli`'s `close_version_at`). Both land in the git tier as
/// distinct digest-suffixed objects, which is precisely the shape
/// `stg_records` returns twice.
fn dispatched_run_corpus(git_root: &std::path::Path) {
    let tier = GitTier::new(git_root);
    let session_id = SessionId::parse("fold-session").unwrap();
    tier.write(&Session::new(
        Envelope::new(1, RecordKind::Session, at(2026, 5, 1, 9), actor("dev")),
        session_id.clone(),
        "claude-code",
        at(2026, 5, 1, 9),
        Some(at(2026, 5, 1, 12)),
    ))
    .unwrap();

    let run_id = RunId::new();
    tier.write(&Run::new(
        Envelope::new(1, RecordKind::Run, at(2026, 5, 1, 10), Actor::new_unattributed("claude-code")),
        run_id,
        Some(session_id.clone()),
        None,
        RunStatus::Running,
        at(2026, 5, 1, 10),
        None,
    ))
    .unwrap();
    tier.write(&Run::new(
        Envelope::new(1, RecordKind::Run, at(2026, 5, 1, 11), Actor::new_unattributed("claude-code")),
        run_id,
        Some(session_id.clone()),
        None,
        RunStatus::Succeeded,
        at(2026, 5, 1, 10),
        Some(at(2026, 5, 1, 11)),
    ))
    .unwrap();

    // Exactly ONE `token_usage` event: $0.01 and 15 tokens, spent once.
    tier.write(&Event::new(
        Envelope::new(1, RecordKind::Event, at(2026, 5, 1, 10), Actor::new_unattributed("claude-code")),
        run_id,
        1,
        "token_usage",
        json!({
            "provider_id": "anthropic",
            "workspace_key": "canon",
            "workspace_label": "canon",
            "tokens": {"input": 10, "output": 5, "cache_read": 0, "cache_write": 0, "reasoning": 0, "total": 15},
            "cost": 0.01,
            "cost_source": "api",
        }),
    ))
    .unwrap();
}

#[test]
fn session_costs_charges_a_run_stored_as_two_versions_exactly_once() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    dispatched_run_corpus(&git_root);

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));
    let result = marts::fetch_session_costs(&roots).unwrap();

    assert_eq!(result.rows.len(), 1, "one session in one workspace must yield exactly one row, got {:?}", result.rows);
    let row = &result.rows[0];

    // `run_count` was ALWAYS right — `count(DISTINCT run_id)` absorbs
    // the duplicate. Asserted anyway so a future fold regression can
    // never be mistaken for a run_count regression.
    assert_eq!(row["run_count"], 1, "one run, however many versions it is stored as");

    // These two are the actual blocker. Unfolded, the `runs` CTE
    // yielded one row per stored VERSION, the `token_usage` join fanned
    // out, and both sums doubled to 0.02 / 30.
    let cost = row["total_cost"].as_f64().unwrap();
    assert!((cost - 0.01).abs() < 1e-9, "a run stored as two versions must be billed ONCE: expected 0.01, got {cost}");
    assert_eq!(row["total_tokens"], 15, "a run stored as two versions must have its tokens counted ONCE");
}

#[test]
fn session_run_handoff_reports_only_the_runs_latest_status() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    dispatched_run_corpus(&git_root);

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));
    // No `fetch_*` wrapper exists for this mart (it is not a report
    // panel) — read it the way `canon-report` reads every view.
    let rows = query::run_query(&roots, "SELECT session_id, run_id, run_status FROM mart_session_run_handoff ORDER BY run_id;").unwrap();

    // Unfolded this emitted TWO rows for one run, one claiming
    // `running` and one `succeeded` — a view whose whole purpose is
    // answering "what is this triple's state" reporting both answers
    // at once, with no column saying which is current.
    assert_eq!(rows.len(), 1, "one session/run pair must yield exactly one row, got {rows:?}");
    assert_eq!(rows[0]["run_status"], "succeeded", "the surviving row must be the LATEST version, never the superseded `running` one");
}

#[test]
fn subjects_counts_scenarios_once_across_a_status_lifecycle() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let tier = GitTier::new(&git_root);

    let subject_id = SubjectId::parse("fold-subject").unwrap();
    let covered = ScenarioId::parse("fold.subject.01").unwrap();
    let uncovered = ScenarioId::parse("fold.subject.02").unwrap();

    // The full `canon subject status` walk: FIVE versions at one
    // `subject_id`, which is the ordinary end state of any shipped
    // subject, not an exotic corpus.
    for (hour, status) in [
        (9, SubjectStatus::Proposed),
        (10, SubjectStatus::Specced),
        (11, SubjectStatus::Building),
        (12, SubjectStatus::Verifying),
        (13, SubjectStatus::Shipped),
    ] {
        tier.write(
            &Subject::new(
                Envelope::new(1, RecordKind::Subject, at(2026, 5, 2, hour), actor("planner")),
                subject_id.clone(),
                "folded subject",
                "one product unit, five stored versions",
                "dev",
                status,
                RoleId::parse("dev").unwrap(),
            )
            .with_links(vec![], vec![covered.clone(), uncovered.clone()]),
        )
        .unwrap();
    }

    tier.write(&EvidenceRecord::new(
        Envelope::new(1, RecordKind::EvidenceRecord, at(2026, 5, 2, 14), actor("dev")),
        None,
        Some(covered.clone()),
        None,
        EvidenceVerdict::Faithful,
    ))
    .unwrap();

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));
    let result = marts::fetch_subjects(&roots).unwrap();

    assert_eq!(result.rows.len(), 1, "one subject must yield exactly one row however many statuses it walked, got {:?}", result.rows);
    let row = &result.rows[0];
    assert_eq!(row["status"], "shipped", "the row must carry the LATEST status, not an arbitrary superseded one");
    // Unfolded, `subject_scenarios` fanned out five-fold: 2 linked
    // scenarios read as 10, and the 1 covered one read as 5.
    assert_eq!(row["scenario_count"], 2, "the subject links two scenarios, not two-per-stored-version");
    assert_eq!(row["covered_scenarios"], 1, "exactly one of them carries a non-divergent verdict");
}

#[test]
fn trust_matrix_and_scope_status_yield_one_row_per_task_after_a_checkbox_flip() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let tier = GitTier::new(&git_root);

    let task_id = TaskId::parse("fold-change#1").unwrap();
    let scenario_id = ScenarioId::parse("fold.task.01").unwrap();

    // `canon gate task` flipping a checkbox appends a second `task`
    // version at the same `task_id`.
    for (hour, status) in [(9, TaskStatus::Open), (10, TaskStatus::Done)] {
        tier.write(
            &Task::new(
                Envelope::new(1, RecordKind::Task, at(2026, 5, 3, hour), actor("planner")),
                task_id.clone(),
                "folded task",
                status,
                None,
            )
            .with_scenario_refs(vec![scenario_id.clone()]),
        )
        .unwrap();
    }

    tier.write(&EvidenceRecord::new(
        Envelope::new(1, RecordKind::EvidenceRecord, at(2026, 5, 3, 11), actor("dev")),
        Some(task_id.clone()),
        None,
        None,
        EvidenceVerdict::Faithful,
    ))
    .unwrap();

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));

    let trust = marts::fetch_trust_matrix(&roots).unwrap();
    assert_eq!(trust.rows.len(), 1, "one task must yield exactly one trust-matrix row, got {:?}", trust.rows);
    assert_eq!(trust.rows[0]["task_status"], "done", "the row must carry the flipped status, not the superseded `open` one");

    // `mart_scope_status` joined TWO unfolded task relations
    // (`int_task_scenario_refs` and `mart_trust_matrix`), so it was
    // QUADRATIC in versions: one declared pair rendered as four rows,
    // two of them claiming a `task_status` the plan no longer holds.
    let scope = marts::fetch_scope_status(&roots).unwrap();
    assert_eq!(scope.rows.len(), 1, "one declared (task_id, scenario_id) pair must yield exactly one row, got {:?}", scope.rows);
    assert_eq!(scope.rows[0]["task_status"], "done");
}

#[test]
fn review_burndown_still_reads_every_divergence_version_as_its_own_event() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let tier = GitTier::new(&git_root);

    // ONE divergence, opened day 1 and resolved day 3 — two versions at
    // the same natural key (`project__scenario__run_seq__round`).
    for (day, status) in [(1, DivergenceStatus::Open), (3, DivergenceStatus::Resolved)] {
        tier.write(&Divergence::new(
            Envelope::new(1, RecordKind::Divergence, at(2026, 5, day, 12), actor("reviewer")),
            ProjectId::parse("root").unwrap(),
            ScenarioId::parse("fold.diverge.01").unwrap(),
            Sha::parse("c".repeat(40)).unwrap(),
            status,
            TotalOrder::new(1),
            1,
            "reviewer1",
            "one divergence, opened then resolved",
        ))
        .unwrap();
    }

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));
    let result = marts::fetch_review_burndown(&roots).unwrap();

    // This is the case a single global `stg_records` fold would break:
    // folding to the latest version drops the `+1` and leaves only the
    // `-1`, driving the running total NEGATIVE. The burn-down is on
    // `views.sql`'s RAW-STREAM list for exactly this reason.
    assert_eq!(result.rows.len(), 2, "the open day and the resolve day are two distinct points on the curve, got {:?}", result.rows);
    assert_eq!(result.rows[0]["divergence_opened"], 1);
    assert_eq!(result.rows[0]["divergence_resolved"], 0);
    assert_eq!(result.rows[0]["divergence_open_running_total"], 1, "opened, not yet resolved");
    assert_eq!(result.rows[1]["divergence_opened"], 0);
    assert_eq!(result.rows[1]["divergence_resolved"], 1);
    assert_eq!(result.rows[1]["divergence_open_running_total"], 0, "burnt down to zero — never negative");
}
