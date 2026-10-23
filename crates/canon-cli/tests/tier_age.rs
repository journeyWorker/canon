//! Integration test for `canon tier age [--dry-run]` (S2 task 3.3),
//! invoking the actually-built `canon` binary (never `canon_cli`'s
//! library functions in-process) against an offline git+r2(local)
//! fixture (`support::Fixture`) — zero network, no credentials.
//!
//! Covers the tier-policy spec's own scenarios: a record past its
//! aging threshold moves tiers (and is gone from its prior tier), a
//! record within threshold is left untouched, and re-running `canon
//! tier age` on an already-aged record performs zero duplicate moves
//! (digest idempotence) — plus `--dry-run`'s "preview only, never
//! writes" contract.

mod support;

use chrono::{Duration, Utc};

const ROUTING: &str = "  trajectory: local\n";
const AGING: &str = "  trajectory: { after: 1d, to: cold }\n";

#[test]
fn dry_run_reports_candidates_and_performs_no_writes() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    fixture.plant_trajectory_in_git(Utc::now() - Duration::days(30), 0.3);
    fixture.plant_trajectory_in_git(Utc::now(), 0.9); // within threshold

    let output = fixture.run_canon(&["tier", "age", "--dry-run"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let stdout = support::stdout(&output);
    assert!(stdout.contains("--dry-run"), "stdout: {stdout}");
    assert!(stdout.contains("would move: 1 candidate(s)"), "stdout: {stdout}");
    assert!(stdout.contains("trajectory"), "stdout: {stdout}");
    assert!(stdout.contains("local -> cold"), "stdout: {stdout}");
    assert!(stdout.contains("(after 1d)"), "stdout: {stdout}");

    // Dry-run must never write or delete anything.
    assert_eq!(fixture.git_file_count(), 2, "dry-run moved a file out of the git tier");
    assert_eq!(fixture.r2_file_count(), 0, "dry-run wrote to the r2 tier");
}

#[test]
fn real_run_moves_the_aged_record_and_leaves_the_fresh_one_and_is_idempotent_on_rerun() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    fixture.plant_trajectory_in_git(Utc::now() - Duration::days(30), 0.3);
    fixture.plant_trajectory_in_git(Utc::now(), 0.9); // within threshold, must stay

    // First real run: the aged record moves git -> r2; the fresh one stays.
    let first = fixture.run_canon(&["tier", "age"]);
    assert!(first.status.success(), "stderr: {}", support::stderr(&first));
    let stdout = support::stdout(&first);
    assert!(!stdout.contains("--dry-run"), "stdout: {stdout}");
    assert!(stdout.contains("moved: 1, already_aged: 0"), "stdout: {stdout}");

    assert_eq!(fixture.git_file_count(), 1, "the within-threshold record must remain in git");
    assert_eq!(fixture.r2_file_count(), 1, "the aged record must have landed in r2");

    // Second, immediate re-run: nothing left in git past the threshold to
    // re-select — idempotent (tier-policy spec, task 3.4's own pattern).
    let second = fixture.run_canon(&["tier", "age"]);
    assert!(second.status.success(), "stderr: {}", support::stderr(&second));
    let stdout = support::stdout(&second);
    assert!(stdout.contains("moved: 0, already_aged: 0"), "stdout: {stdout}");

    assert_eq!(fixture.git_file_count(), 1, "re-run must not touch the within-threshold record");
    assert_eq!(fixture.r2_file_count(), 1, "re-run must not duplicate the already-aged record");
}

#[test]
fn no_aging_rules_configured_is_a_clean_no_op() {
    let fixture = support::Fixture::new(ROUTING, "");
    let output = fixture.run_canon(&["tier", "age"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    assert!(support::stdout(&output).contains("no `aging` rules configured"));
}
