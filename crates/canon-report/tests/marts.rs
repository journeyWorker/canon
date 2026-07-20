//! Task 2.6 acceptance (extended by s24 `scope_status`, s36
//! `subjects`): the fixture corpus (`crates/canon-report/fixtures/
//! corpus.rs`) renders every one of the seven marts to its documented
//! KNOWN expected values — never a "some rows came back" smoke check.

mod support;

use canon_report::{marts, ReportInputs};
use support::corpus;

fn inputs(dir: &std::path::Path) -> ReportInputs {
    let roots = corpus::build(dir);
    ReportInputs::new(dir, roots)
}

#[test]
fn trust_matrix_matches_the_fixture_corpus_exactly() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_trust_matrix(&inputs(dir.path()).roots).unwrap();

    assert_eq!(result.rows.len(), 3, "exactly three fixture task subjects, got {:?}", result.rows);

    let row = |task_id: &str| result.rows.iter().find(|r| r.get("task_id").and_then(|v| v.as_str()) == Some(task_id)).unwrap_or_else(|| panic!("missing row for {task_id}"));

    let (id1, covered1, green1, who1) = corpus::trust_matrix::TASK_1_COVERED_GREEN;
    let r1 = row(id1);
    assert_eq!(r1["change_id"], corpus::trust_matrix::CHANGE_ID);
    assert_eq!(r1["covered"], covered1);
    assert_eq!(r1["green"], green1);
    assert_eq!(r1["who"], who1);

    let (id2, covered2, green2, who2) = corpus::trust_matrix::TASK_2_COVERED_NOT_GREEN;
    let r2 = row(id2);
    assert_eq!(r2["covered"], covered2);
    assert_eq!(r2["green"], green2);
    assert_eq!(r2["who"], who2);

    let r3 = row(corpus::trust_matrix::TASK_3_NOT_COVERED);
    assert_eq!(r3["covered"], false);
    assert_eq!(r3["green"], false);
    assert!(r3["who"].is_null());
}

#[test]
fn session_costs_matches_the_fixture_corpus_exactly() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_session_costs(&inputs(dir.path()).roots).unwrap();

    assert_eq!(result.rows.len(), 1, "exactly one fixture session, got {:?}", result.rows);
    let row = &result.rows[0];
    assert_eq!(row["session_id"], corpus::session_costs::SESSION_ID);
    assert_eq!(row["client"], corpus::session_costs::CLIENT);
    assert_eq!(row["role"], corpus::session_costs::ROLE);
    assert_eq!(row["workspace_label"], corpus::session_costs::WORKSPACE_LABEL);
    assert_eq!(row["run_count"], corpus::session_costs::RUN_COUNT);
    assert!((row["total_cost"].as_f64().unwrap() - corpus::session_costs::TOTAL_COST).abs() < 1e-9, "total_cost was {:?}", row["total_cost"]);
    assert_eq!(row["total_tokens"], corpus::session_costs::TOTAL_TOKENS);
}

#[test]
fn role_memory_matches_the_fixture_corpus_exactly() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_role_memory(&inputs(dir.path()).roots).unwrap();

    assert_eq!(result.rows.len(), 2, "dev + content role rows, got {:?}", result.rows);
    let row = |role: &str| result.rows.iter().find(|r| r.get("role").and_then(|v| v.as_str()) == Some(role)).unwrap_or_else(|| panic!("missing row for role {role}"));

    let dev = row("dev");
    assert_eq!(dev["strategy_count"], corpus::role_memory::DEV_STRATEGY_COUNT);
    assert_eq!(dev["active_count"], corpus::role_memory::DEV_ACTIVE_COUNT);
    assert_eq!(dev["demoted_count"], corpus::role_memory::DEV_DEMOTED_COUNT);
    assert!((dev["hit_rate"].as_f64().unwrap() - corpus::role_memory::DEV_HIT_RATE).abs() < 1e-9);

    let content = row("content");
    assert_eq!(content["strategy_count"], corpus::role_memory::CONTENT_STRATEGY_COUNT);
    assert!((content["hit_rate"].as_f64().unwrap() - corpus::role_memory::CONTENT_HIT_RATE).abs() < 1e-9);
}

#[test]
fn flywheel_funnel_matches_the_fixture_corpus_exactly() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_flywheel_funnel(&inputs(dir.path()).roots).unwrap();

    assert_eq!(result.rows.len(), 2, "dev + content role rows, got {:?}", result.rows);
    let row = |role: &str| result.rows.iter().find(|r| r.get("role").and_then(|v| v.as_str()) == Some(role)).unwrap_or_else(|| panic!("missing row for role {role}"));

    let dev = row("dev");
    assert_eq!(dev["verdicts"], corpus::flywheel_funnel::DEV_VERDICTS);
    assert_eq!(dev["distilled"], corpus::flywheel_funnel::DEV_DISTILLED);
    assert_eq!(dev["retrieved"], corpus::flywheel_funnel::DEV_RETRIEVED);
    assert_eq!(dev["applied"], corpus::flywheel_funnel::DEV_APPLIED);

    let content = row("content");
    assert_eq!(content["verdicts"], corpus::flywheel_funnel::CONTENT_VERDICTS);
    assert_eq!(content["distilled"], corpus::flywheel_funnel::CONTENT_DISTILLED);
    assert_eq!(content["retrieved"], corpus::flywheel_funnel::CONTENT_RETRIEVED);
    assert_eq!(content["applied"], corpus::flywheel_funnel::CONTENT_APPLIED);
}

#[test]
fn review_burndown_matches_the_fixture_corpus_exactly() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_review_burndown(&inputs(dir.path()).roots).unwrap();

    assert!(!result.rows.is_empty(), "expected at least the two divergence-bearing days");
    let day1 = &result.rows[0];
    assert_eq!(day1["divergence_opened"], corpus::review_burndown::DAY_1_OPENED);
    assert_eq!(day1["divergence_open_running_total"], corpus::review_burndown::DAY_1_OPENED);

    let last = result.rows.last().unwrap();
    assert_eq!(last["divergence_resolved"], corpus::review_burndown::DAY_3_RESOLVED);
    assert_eq!(last["divergence_open_running_total"], 0, "opened(1) - resolved(1) running total returns to zero");
}

#[test]
fn scope_status_matches_the_fixture_corpus_exactly() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_scope_status(&inputs(dir.path()).roots).unwrap();

    assert_eq!(result.rows.len(), 2, "exactly two declared (task_id, scenario_id) rows, got {:?}", result.rows);

    let row = |task_id: &str, scenario_id: &str| {
        result
            .rows
            .iter()
            .find(|r| r.get("task_id").and_then(|v| v.as_str()) == Some(task_id) && r.get("scenario_id").and_then(|v| v.as_str()) == Some(scenario_id))
            .unwrap_or_else(|| panic!("missing row for ({task_id}, {scenario_id})"))
    };

    let fully_green = row(corpus::scope_status::FULLY_GREEN_TASK_ID, corpus::scope_status::FULLY_GREEN_SCENARIO_ID);
    assert_eq!(fully_green["task_status"], corpus::scope_status::FULLY_GREEN_TASK_STATUS);
    assert_eq!(fully_green["evidence_covered"], corpus::scope_status::FULLY_GREEN_EVIDENCE_COVERED);
    assert_eq!(fully_green["green"], corpus::scope_status::FULLY_GREEN_GREEN);
    assert_eq!(fully_green["spec_covered"], corpus::scope_status::FULLY_GREEN_SPEC_COVERED);

    let unauthored = row(corpus::scope_status::UNAUTHORED_TASK_ID, corpus::scope_status::UNAUTHORED_SCENARIO_ID);
    assert_eq!(unauthored["task_status"], corpus::scope_status::UNAUTHORED_TASK_STATUS);
    assert_eq!(unauthored["evidence_covered"], corpus::scope_status::UNAUTHORED_EVIDENCE_COVERED);
    assert_eq!(unauthored["green"], corpus::scope_status::UNAUTHORED_GREEN);
    assert!(
        unauthored.get("spec_covered").is_none_or(|v| v.is_null()),
        "spec_covered must be an honest NULL when no porting.coverage overlay exists, got {:?}",
        unauthored.get("spec_covered")
    );
}

#[test]
fn a_task_with_no_declared_scenario_refs_contributes_no_scope_status_row() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_scope_status(&inputs(dir.path()).roots).unwrap();

    assert!(
        result.rows.iter().all(|r| r.get("task_id").and_then(|v| v.as_str()) != Some(corpus::scope_status::NO_SCENARIO_REFS_TASK_ID)),
        "a task with empty scenario_refs (fixture task 3) must never surface in mart_scope_status, got {:?}",
        result.rows
    );
}

#[test]
fn subjects_matches_the_fixture_corpus_exactly() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_subjects(&inputs(dir.path()).roots).unwrap();

    assert_eq!(result.rows.len(), 1, "exactly one fixture subject, got {:?}", result.rows);
    let row = &result.rows[0];
    assert_eq!(row["domain"], corpus::subjects::DOMAIN);
    assert_eq!(row["subject_id"], corpus::subjects::SUBJECT_ID);
    assert_eq!(row["title"], corpus::subjects::TITLE);
    assert_eq!(row["status"], corpus::subjects::STATUS);
    assert_eq!(row["scenario_count"], corpus::subjects::SCENARIO_COUNT);
    assert_eq!(
        row["covered_scenarios"], corpus::subjects::COVERED_SCENARIOS,
        "one of the two linked scenarios carries a latest Faithful verdict; the other has no evidence"
    );
}
