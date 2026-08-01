//! Task 2.6 acceptance (extended by s24 `scope_status`, s36
//! `subjects`, s43 `review_rounds`/`review_totals`): the fixture
//! corpus (`crates/canon-report/fixtures/corpus.rs`) renders every one
//! of the nine marts to its documented KNOWN expected values — never a
//! "some rows came back" smoke check.
//!
//! `mart_review_totals` is the one exception to "documented KNOWN
//! expected values", deliberately. Its whole claim is that it equals
//! the sum of the `mart_review_rounds` rows a reader sees, and a
//! literal in `corpus.rs` would be a THIRD place that number lives:
//! the view could drift, the literal could be updated to match, and
//! the test would stay green while the total stopped being the total.
//! So the tests below assert the RELATION against whatever the rounds
//! mart returns, plus a non-vacuity guard so an all-zero corpus can
//! never satisfy them.

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

    assert_eq!(result.rows.len(), 4, "dev + content + reviewer + fixer role rows, got {:?}", result.rows);
    let row = |role: &str| result.rows.iter().find(|r| r.get("role").and_then(|v| v.as_str()) == Some(role)).unwrap_or_else(|| panic!("missing row for role {role}"));

    let dev = row("dev");
    assert_eq!(dev["strategy_count"], corpus::role_memory::DEV_STRATEGY_COUNT);
    assert_eq!(dev["active_count"], corpus::role_memory::DEV_ACTIVE_COUNT);
    assert_eq!(dev["demoted_count"], corpus::role_memory::DEV_DEMOTED_COUNT);
    assert!((dev["hit_rate"].as_f64().unwrap() - corpus::role_memory::DEV_HIT_RATE).abs() < 1e-9);

    let content = row("content");
    assert_eq!(content["strategy_count"], corpus::role_memory::CONTENT_STRATEGY_COUNT);
    assert!((content["hit_rate"].as_f64().unwrap() - corpus::role_memory::CONTENT_HIT_RATE).abs() < 1e-9);

    let reviewer = row("reviewer");
    assert_eq!(reviewer["strategy_count"], corpus::role_memory::REVIEWER_STRATEGY_COUNT);
    assert!((reviewer["hit_rate"].as_f64().unwrap() - corpus::role_memory::REVIEWER_HIT_RATE).abs() < 1e-9);

    let fixer = row("fixer");
    assert_eq!(fixer["strategy_count"], corpus::role_memory::FIXER_STRATEGY_COUNT);
    assert!((fixer["hit_rate"].as_f64().unwrap() - corpus::role_memory::FIXER_HIT_RATE).abs() < 1e-9);
}

#[test]
fn flywheel_funnel_matches_the_fixture_corpus_exactly() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_flywheel_funnel(&inputs(dir.path()).roots).unwrap();

    assert_eq!(result.rows.len(), 4, "dev + content + reviewer + fixer role rows, got {:?}", result.rows);
    let row = |role: &str| result.rows.iter().find(|r| r.get("role").and_then(|v| v.as_str()) == Some(role)).unwrap_or_else(|| panic!("missing row for role {role}"));

    let dev = row("dev");
    assert_eq!(dev["verdicts"], corpus::flywheel_funnel::DEV_VERDICTS);
    assert_eq!(dev["distilled"], corpus::flywheel_funnel::DEV_DISTILLED);
    assert_eq!(dev["retrieved"], corpus::flywheel_funnel::DEV_RETRIEVED);
    assert_eq!(dev["applied"], corpus::flywheel_funnel::DEV_APPLIED);
    assert_eq!(dev["applied_attributed"], corpus::flywheel_funnel::DEV_APPLIED_ATTRIBUTED);
    assert_eq!(dev["applied_proxy"], corpus::flywheel_funnel::DEV_APPLIED_PROXY);

    let content = row("content");
    assert_eq!(content["verdicts"], corpus::flywheel_funnel::CONTENT_VERDICTS);
    assert_eq!(content["distilled"], corpus::flywheel_funnel::CONTENT_DISTILLED);
    assert_eq!(content["retrieved"], corpus::flywheel_funnel::CONTENT_RETRIEVED);
    assert_eq!(content["applied"], corpus::flywheel_funnel::CONTENT_APPLIED);
    assert_eq!(content["applied_attributed"], corpus::flywheel_funnel::CONTENT_APPLIED_ATTRIBUTED);
    assert_eq!(content["applied_proxy"], corpus::flywheel_funnel::CONTENT_APPLIED_PROXY);

    let reviewer = row("reviewer");
    assert_eq!(reviewer["verdicts"], corpus::flywheel_funnel::REVIEWER_VERDICTS);
    assert_eq!(reviewer["distilled"], corpus::flywheel_funnel::REVIEWER_DISTILLED);
    assert_eq!(reviewer["retrieved"], corpus::flywheel_funnel::REVIEWER_RETRIEVED);
    assert_eq!(reviewer["applied"], corpus::flywheel_funnel::REVIEWER_APPLIED);
    assert_eq!(reviewer["applied_attributed"], corpus::flywheel_funnel::REVIEWER_APPLIED_ATTRIBUTED);
    assert_eq!(reviewer["applied_proxy"], corpus::flywheel_funnel::REVIEWER_APPLIED_PROXY);

    let fixer = row("fixer");
    assert_eq!(fixer["verdicts"], corpus::flywheel_funnel::FIXER_VERDICTS);
    assert_eq!(fixer["distilled"], corpus::flywheel_funnel::FIXER_DISTILLED);
    assert_eq!(fixer["retrieved"], corpus::flywheel_funnel::FIXER_RETRIEVED);
    assert_eq!(fixer["applied"], corpus::flywheel_funnel::FIXER_APPLIED);
    assert_eq!(fixer["applied_attributed"], corpus::flywheel_funnel::FIXER_APPLIED_ATTRIBUTED);
    assert_eq!(fixer["applied_proxy"], corpus::flywheel_funnel::FIXER_APPLIED_PROXY);
}

/// s42 (`close-the-open-loops`) task 3.3: the ATTRIBUTION rule is what
/// closes s40 task 3.1's original wording, and it must be provably the
/// thing producing `reviewer`'s count — not the proxy wearing a new
/// column name.
///
/// `reviewer` and `content` are the SAME shape in every respect the proxy
/// can see: one distilled strategy, cited by one run, that run still
/// `Running`, and a RESOLVED trajectory in the store. They differ in
/// exactly one bit — `reviewer`'s trajectory carries that run's
/// `run_id`. So `reviewer.applied_attributed == 1` while
/// `content.applied == 0` is a difference no `Run.status` rule could
/// produce, and `reviewer.applied_proxy == 0` states positively that the
/// weaker rule contributed nothing.
#[test]
fn applied_attribution_is_what_counts_a_resolved_trajectory_joined_to_its_own_run() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_flywheel_funnel(&inputs(dir.path()).roots).unwrap();
    let row = |role: &str| result.rows.iter().find(|r| r.get("role").and_then(|v| v.as_str()) == Some(role)).unwrap_or_else(|| panic!("missing row for role {role}"));

    let reviewer = row("reviewer");
    assert_eq!(reviewer["applied"], 1, "the attributed run must count as applied: {reviewer:?}");
    assert_eq!(reviewer["applied_attributed"], 1, "and by ATTRIBUTION, not the proxy: {reviewer:?}");
    assert_eq!(reviewer["applied_proxy"], 0, "its run never reached a terminal status, so the proxy admits nothing: {reviewer:?}");

    // The control: identical in every respect the proxy can observe,
    // differing only in the absent stamp.
    let content = row("content");
    assert_eq!(content["retrieved"], reviewer["retrieved"], "the control must match on the stage above");
    assert_eq!(content["applied"], 0, "an UNSTAMPED resolved trajectory must not be attributed: {content:?}");

    // And the other direction: the proxy still works where it always
    // did, and never masquerades as attribution.
    let dev = row("dev");
    assert_eq!(dev["applied"], 1);
    assert_eq!(dev["applied_proxy"], 1, "dev's terminal run is the s40 proxy: {dev:?}");
    assert_eq!(dev["applied_attributed"], 0, "nothing stamped dev's run, so attribution admits nothing: {dev:?}");
}

/// s40 (`plan-vs-actual-diff`, task 3.2) / s42 task 3.4: the panel's
/// whole claim is that it NARROWS. `applied <= retrieved <= distilled` is
/// structural in the view — all three stages count strategies, and each
/// is a restriction of the one above it — so it must hold for every row
/// of any corpus, not just for the fixture's hand-checked numbers above.
/// The `content` row is the one that earns the first inequality: its
/// trajectory is resolved (exactly what the pre-s40 `applied` counted)
/// while the run its guidance went into never finished, the shape that
/// used to render `applied` above `retrieved`.
///
/// s42 adds the second half of "by construction, not by luck": splitting
/// `applied` by rule can only widen the stage if the two rules are
/// tallied as separate overlapping sets, so the split must PARTITION the
/// total — `applied == applied_attributed + applied_proxy` on every row.
/// The fixture's `fixer` role is the row that earns the partition
/// assertion: its ONE strategy is injected into an attributed run AND a
/// merely-terminal one, which is exactly where a naive pair of
/// independently-filtered `count(DISTINCT strategy_id)`s would report
/// `1 + 1` against an `applied` of `1`. The view resolves each strategy
/// to ONE rule instead, attribution winning.
#[test]
fn flywheel_funnel_never_widens() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_flywheel_funnel(&inputs(dir.path()).roots).unwrap();

    assert!(!result.rows.is_empty(), "an empty funnel would satisfy the invariant vacuously");
    for row in &result.rows {
        let count = |column: &str| row.get(column).and_then(|v| v.as_i64()).unwrap_or_else(|| panic!("missing `{column}` in {row:?}"));
        assert!(count("applied") <= count("retrieved"), "applied must never exceed retrieved: {row:?}");
        assert!(count("retrieved") <= count("distilled"), "retrieved must never exceed distilled: {row:?}");
        assert_eq!(
            count("applied"),
            count("applied_attributed") + count("applied_proxy"),
            "the two rules must PARTITION applied, never overlap: {row:?}"
        );
    }
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
    assert_eq!(
        fully_green["spec_project_id"],
        corpus::scope_status::FULLY_GREEN_SPEC_PROJECT_ID,
        "the row must name WHOSE coverage it reports — `porting.coverage` is keyed (project_id, scenario_id)"
    );
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
    assert!(
        unauthored.get("spec_project_id").is_none_or(|v| v.is_null()),
        "spec_project_id must be NULL too — there is no overlay row to attribute, got {:?}",
        unauthored.get("spec_project_id")
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

/// Every column of every `mart_review_rounds` row, against the
/// documented corpus (s43 `findings-are-records`). Asserting the WHOLE
/// row per round, not just `fix_of_fix`, is deliberate: a severity or
/// disposition `FILTER` that started matching the wrong literal would
/// otherwise pass while the panel's totals stopped adding up.
#[test]
fn review_rounds_matches_the_fixture_corpus_exactly() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_review_rounds(&inputs(dir.path()).roots).unwrap();

    assert_eq!(result.rows.len(), 4, "three `s9-fixture` rounds plus one on the second change, got {:?}", result.rows);

    let row = |change_id: &str, round: i64| {
        result
            .rows
            .iter()
            .find(|r| r.get("change_id").and_then(|v| v.as_str()) == Some(change_id) && r.get("round").and_then(|v| v.as_i64()) == Some(round))
            .unwrap_or_else(|| panic!("missing row for {change_id} round {round} in {:?}", result.rows))
    };
    let assert_row = |label: &str, r: &canon_report::query::Row, expected: &corpus::review_rounds::Row| {
        assert_eq!(r["findings"], expected.findings, "{label}: findings");
        assert_eq!(r["severity_blocker"], expected.blocker, "{label}: severity_blocker");
        assert_eq!(r["severity_should_fix"], expected.should_fix, "{label}: severity_should_fix");
        assert_eq!(r["severity_note"], expected.note, "{label}: severity_note");
        assert_eq!(r["disposition_open"], expected.open, "{label}: disposition_open");
        assert_eq!(r["disposition_fixed"], expected.fixed, "{label}: disposition_fixed");
        assert_eq!(r["disposition_rejected"], expected.rejected, "{label}: disposition_rejected");
        assert_eq!(r["disposition_deferred"], expected.deferred, "{label}: disposition_deferred");
        assert_eq!(r["fix_of_fix"], expected.fix_of_fix, "{label}: fix_of_fix");
        assert_eq!(r["introduced_by_sourced"], expected.sourced, "{label}: introduced_by_sourced");
        assert_eq!(r["introduced_by_unsourced"], expected.unsourced, "{label}: introduced_by_unsourced");
        // The two arithmetic identities the panel states as holding by
        // construction. If either stops holding, the panel's claim that
        // the unknown is readable against the count is false.
        assert_eq!(expected.sourced + expected.unsourced, expected.findings, "{label}: sourced + unsourced must equal findings");
        assert!(expected.fix_of_fix <= expected.sourced, "{label}: fix_of_fix can never exceed the sourced count");
    };

    let change = corpus::review_rounds::CHANGE_ID;
    assert_row("round 1", row(change, 1), &corpus::review_rounds::ROUND_1);
    assert_row("round 2", row(change, 2), &corpus::review_rounds::ROUND_2);
    assert_row("round 3", row(change, 3), &corpus::review_rounds::ROUND_3);
    assert_row("other change round 1", row(corpus::review_rounds::OTHER_CHANGE_ID, 1), &corpus::review_rounds::OTHER_ROUND_1);

    // Round 1 committed what it reviewed; rounds 2 and 3 reviewed an
    // uncommitted worktree, which is the common case and the reason
    // `Finding::reviewed_sha` is `Option`. A round whose findings named
    // no commit must still be a ROW here, with an honest NULL — never a
    // dropped round, and never a borrowed adjacent sha.
    assert_eq!(row(change, 1)["reviewed_sha"], corpus::review_rounds::ROUND_1_REVIEWED_SHA);
    for round in [2, 3] {
        assert!(
            row(change, round)["reviewed_sha"].is_null(),
            "round {round} reviewed a worktree, so its reviewed_sha must be NULL, got {:?}",
            row(change, round)["reviewed_sha"]
        );
    }
}

/// The three shapes `introduced_by` can take, read off the same corpus
/// row-by-row rather than only through the aggregate above — because
/// `fix_of_fix` and `introduced_by_sourced` would be indistinguishable
/// on a corpus where every sourced finding happened to match, and
/// `introduced_by_unsourced` would be indistinguishable from zero on a
/// corpus where every finding was sourced.
#[test]
fn review_rounds_separates_matched_sourced_and_unsourced_introducing_commits() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_review_rounds(&inputs(dir.path()).roots).unwrap();
    let round_2 = result
        .rows
        .iter()
        .find(|r| r.get("change_id").and_then(|v| v.as_str()) == Some(corpus::review_rounds::CHANGE_ID) && r.get("round").and_then(|v| v.as_i64()) == Some(2))
        .expect("round 2 row");

    // Three findings, three distinct shapes: one matched, one sourced
    // but unmatched, one unsourced. The gap between `fix_of_fix` and
    // `introduced_by_sourced` is what proves the join filters rather
    // than just counting non-NULLs.
    assert_eq!(round_2["findings"], 3);
    assert_eq!(round_2["fix_of_fix"], 1, "only the finding naming round 1's resolution_sha joins");
    assert_eq!(round_2["introduced_by_sourced"], 2, "two findings carry a sourced introducing commit; only one of them matches");
    assert_eq!(round_2["introduced_by_unsourced"], 1, "the unsourced finding is counted as UNKNOWN, never as not-a-fix-of-fix");
}

/// The two over-counting traps, asserted as ZEROES on rows that do
/// carry a sourced `introduced_by` — so a `0` here means "the join
/// refused it", not "there was nothing to refuse".
#[test]
fn review_rounds_never_counts_a_finding_against_itself_or_across_changes() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = marts::fetch_review_rounds(&inputs(dir.path()).roots).unwrap();
    let row = |change_id: &str, round: i64| {
        result
            .rows
            .iter()
            .find(|r| r.get("change_id").and_then(|v| v.as_str()) == Some(change_id) && r.get("round").and_then(|v| v.as_i64()) == Some(round))
            .unwrap_or_else(|| panic!("missing row for {change_id} round {round}"))
    };

    // Round 3's finding names its OWN `resolution_sha` as its
    // `introduced_by`. Strict `(round, seq)` is the only thing keeping
    // it out; a `<=` would score this `1`.
    let self_caused = row(corpus::review_rounds::CHANGE_ID, 3);
    assert_eq!(self_caused["introduced_by_sourced"], 1, "the trap only means something if the finding IS sourced");
    assert_eq!(self_caused["fix_of_fix"], 0, "a finding may never be matched against its own resolution_sha");

    // The other change's finding names `s9-fixture` round 1's
    // resolution. Both are SHAs, so the equality holds — only the
    // same-change scope excludes it. This is the documented
    // cross-change miss; widening the scope without defining a
    // cross-change order must fail here.
    let other = row(corpus::review_rounds::OTHER_CHANGE_ID, 1);
    assert_eq!(other["introduced_by_sourced"], 1);
    assert_eq!(other["fix_of_fix"], 0, "the fix-of-fix join is scoped to one change_id; `round` orders nothing between changes");
}

/// s43 round 5, the blocker. `mart_review_totals` must be exactly the
/// per-change sum of the `mart_review_rounds` rows the report prints
/// above it — every column, every change in the corpus. Asserted as an
/// INVARIANT over the two marts rather than against literals: the
/// point of the view is that the two can never disagree, and a literal
/// would let them disagree with each other while both matched it.
#[test]
fn review_totals_are_exactly_the_per_change_sum_of_the_review_rounds_rows() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let roots = inputs(dir.path()).roots;
    let rounds = marts::fetch_review_rounds(&roots).unwrap();
    let totals = marts::fetch_review_totals(&roots).unwrap();

    let n = |row: &canon_report::query::Row, column: &str| row.get(column).and_then(|v| v.as_i64()).unwrap_or_else(|| panic!("{column} is not an integer in {row:?}"));

    // Same change set, no more and no fewer: a change with rounds but
    // no totals row would leave a release note with nothing to copy,
    // and a totals row for a change with no rounds would be a number
    // out of thin air.
    let changes = |m: &canon_report::marts::MartResult| {
        m.rows.iter().map(|r| r["change_id"].as_str().expect("change_id").to_string()).collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(changes(&totals), changes(&rounds), "every change with rounds needs a totals row and vice versa");
    assert_eq!(totals.rows.len(), changes(&totals).len(), "mart_review_totals must be one row per change_id");

    // The summed columns are every numeric column the rounds mart has;
    // listing them here rather than the three the release note happens
    // to use is the same reasoning as `review_rounds_matches_the_
    // fixture_corpus_exactly` asserting whole rows.
    const SUMMED: &[&str] = &[
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

    for total in &totals.rows {
        let change = total["change_id"].as_str().expect("change_id");
        let mine: Vec<_> = rounds.rows.iter().filter(|r| r["change_id"].as_str() == Some(change)).collect();
        assert!(!mine.is_empty(), "{change}: totals row with no rounds behind it");

        assert_eq!(n(total, "rounds_recorded"), mine.len() as i64, "{change}: rounds_recorded must be the number of rounds that recorded a finding");
        assert_eq!(
            n(total, "highest_round"),
            mine.iter().map(|r| n(r, "round")).max().unwrap(),
            "{change}: highest_round must be the greatest round number that recorded a finding"
        );
        for column in SUMMED {
            let expected: i64 = mine.iter().map(|r| n(r, column)).sum();
            assert_eq!(n(total, column), expected, "{change}: {column} must be the sum of the per-round values");
        }

        // The identities the panel prints beside the numbers, checked
        // on the TOTAL row: a reader who copies one cell and sanity-
        // checks it against its neighbours must not find them
        // inconsistent.
        let findings = n(total, "findings");
        assert_eq!(n(total, "severity_blocker") + n(total, "severity_should_fix") + n(total, "severity_note"), findings, "{change}: the severity split must partition findings");
        assert_eq!(
            n(total, "disposition_open") + n(total, "disposition_fixed") + n(total, "disposition_rejected") + n(total, "disposition_deferred"),
            findings,
            "{change}: the disposition split must partition findings"
        );
        assert_eq!(n(total, "introduced_by_sourced") + n(total, "introduced_by_unsourced"), findings, "{change}: sourced + unsourced must equal findings");
    }

    // Non-vacuity. Every assertion above holds trivially over an empty
    // or single-round corpus, so pin that the fixture actually
    // exercises the summing: one change spans several rounds and its
    // total exceeds every individual round's.
    let multi = totals
        .rows
        .iter()
        .find(|t| n(t, "rounds_recorded") > 1)
        .expect("the corpus must contain a change reviewed over more than one round, or this test proves nothing");
    let change = multi["change_id"].as_str().unwrap();
    let biggest_round = rounds.rows.iter().filter(|r| r["change_id"].as_str() == Some(change)).map(|r| n(r, "findings")).max().unwrap();
    assert!(n(multi, "findings") > biggest_round, "{change}: the fixture must have a total that no single round already equals");
}

/// The second number the wrong release note typed ("four of them
/// defects in the previous round's fix"). It is derived per round, and
/// the per-change total has to be the sum of those and nothing else —
/// no re-derivation, no widened scope, no re-run of the semi-join at a
/// coarser grain that would quietly start matching across rounds it
/// previously could not.
#[test]
fn the_fix_of_fix_total_is_the_sum_of_the_per_round_fix_of_fix_values() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let roots = inputs(dir.path()).roots;
    let rounds = marts::fetch_review_rounds(&roots).unwrap();
    let totals = marts::fetch_review_totals(&roots).unwrap();
    let n = |row: &canon_report::query::Row, column: &str| row.get(column).and_then(|v| v.as_i64()).unwrap();

    for total in &totals.rows {
        let change = total["change_id"].as_str().unwrap();
        let expected: i64 = rounds.rows.iter().filter(|r| r["change_id"].as_str() == Some(change)).map(|r| n(r, "fix_of_fix")).sum();
        assert_eq!(n(total, "fix_of_fix"), expected, "{change}: the fix-of-fix total must be the sum of the per-round counts");
        // The bound the panel states beside the number, at this grain.
        assert!(n(total, "fix_of_fix") <= n(total, "introduced_by_sourced"), "{change}: fix_of_fix can never exceed the sourced findings it narrows");
    }

    // Non-vacuity: `0 == 0` would satisfy the loop over a corpus where
    // the join never fires. The fixture's `s9-fixture` round 2 carries
    // the one edge the derivation admits, so the change's total is 1 —
    // and the OTHER change, which names a `resolution_sha` from a
    // different change, must still total 0.
    let total_for = |change: &str| n(totals.rows.iter().find(|t| t["change_id"].as_str() == Some(change)).unwrap(), "fix_of_fix");
    assert_eq!(total_for(corpus::review_rounds::CHANGE_ID), 1, "the fixture's one admitted fix-of-fix edge must reach the total");
    assert_eq!(
        total_for(corpus::review_rounds::OTHER_CHANGE_ID),
        0,
        "the same-change scope must survive the roll-up: a cross-change match counts zero at every grain"
    );
}
