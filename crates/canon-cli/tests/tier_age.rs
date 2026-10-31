//! Integration test for `canon tier age [--commit --plan-digest]` (S2 task 3.3),
//! invoking the actually-built `canon` binary (never `canon_cli`'s
//! library functions in-process) against an offline git+r2(local)
//! fixture (`support::Fixture`) — zero network, no credentials.
//!
//! Covers the tier-policy spec's own scenarios: a record past its
//! aging threshold moves tiers (and is gone from its prior tier), a
//! record within threshold is left untouched, and re-running `canon
//! tier age` on an already-aged record performs zero duplicate moves
//! (digest idempotence) — plus the bare run's "preview only, never
//! writes" contract.

mod support;

use chrono::{Duration, Utc};

const ROUTING: &str = "  trajectory: local\n";
const AGING: &str = "  trajectory: { after: 1d, to: cold }\n";

/// The `planDigest` of a bare `canon tier age --json` run.
fn preview(fixture: &support::Fixture) -> serde_json::Value {
    let output = fixture.run_canon(&["tier", "age", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    serde_json::from_str(&support::stdout(&output)).unwrap()
}

fn commit(fixture: &support::Fixture, digest: &str) -> std::process::Output {
    fixture.run_canon(&["tier", "age", "--commit", "--plan-digest", digest, "--json"])
}

#[test]
fn the_bare_run_previews_the_candidates_and_performs_no_writes() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    fixture.plant_trajectory_in_git(Utc::now() - Duration::days(30), 0.3);
    fixture.plant_trajectory_in_git(Utc::now(), 0.9); // within threshold

    let body = preview(&fixture);
    let data = &body["data"];
    assert_eq!(data["committed"], false, "{body}");
    assert_eq!(data["effects"].as_array().unwrap().len(), 1, "one effect per record past the threshold: {body}");
    assert_eq!(data["rules"][0]["candidates"], 1, "{body}");
    assert_eq!(data["rules"][0]["moved"], 0, "{body}");
    let digest = data["planDigest"].as_str().expect("a preview carries a planDigest");
    assert_eq!(body["meta"]["next"][0]["argv"], serde_json::json!(["tier", "age", "--repo", fixture.root.path().display().to_string(), "--commit", "--plan-digest", digest, "--json"]), "{body}");

    // A preview must never write or delete anything.
    assert_eq!(fixture.git_file_count(), 2, "the preview moved a file out of the git tier");
    assert_eq!(fixture.r2_file_count(), 0, "the preview wrote to the r2 tier");
}

#[test]
fn a_committed_plan_moves_the_aged_record_leaves_the_rest_and_settles() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    fixture.plant_trajectory_in_git(Utc::now() - Duration::days(30), 0.3);
    fixture.plant_trajectory_in_git(Utc::now(), 0.9); // within threshold, must stay
    let digest = preview(&fixture)["data"]["planDigest"].as_str().unwrap().to_string();

    let first = commit(&fixture, &digest);
    assert!(first.status.success(), "stderr: {}", support::stderr(&first));
    let body: serde_json::Value = serde_json::from_str(&support::stdout(&first)).unwrap();
    assert_eq!(body["data"]["committed"], true, "{body}");
    assert_eq!(body["data"]["rules"][0]["moved"], 1, "{body}");
    assert_eq!(body["data"]["rules"][0]["alreadyAged"], 0, "{body}");
    assert_eq!(fixture.git_file_count(), 1, "the within-threshold record must remain in git");
    assert_eq!(fixture.r2_file_count(), 1, "the aged record must have landed in r2");

    // A second, immediate run: nothing left in git past the threshold to
    // re-select — idempotent (tier-policy spec, task 3.4's own pattern).
    let again = preview(&fixture);
    assert_eq!(again["data"]["effects"].as_array().unwrap().len(), 0, "{again}");
    let second = commit(&fixture, again["data"]["planDigest"].as_str().unwrap());
    assert!(second.status.success(), "stderr: {}", support::stderr(&second));
    assert_eq!(fixture.git_file_count(), 1, "re-run must not touch the within-threshold record");
    assert_eq!(fixture.r2_file_count(), 1, "re-run must not duplicate the already-aged record");
}

#[test]
fn a_stale_digest_moves_nothing_and_a_fresh_preview_commits() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    fixture.plant_trajectory_in_git(Utc::now() - Duration::days(30), 0.3);
    fixture.plant_trajectory_in_git(Utc::now(), 0.9);
    let digest = preview(&fixture)["data"]["planDigest"].as_str().unwrap().to_string();

    // Another aged record lands between the preview and the commit.
    fixture.plant_trajectory_in_git(Utc::now() - Duration::days(20), 0.5);
    let stale = commit(&fixture, &digest);
    assert_eq!(stale.status.code(), Some(2));
    let stale_body: serde_json::Value = serde_json::from_str(&support::stdout(&stale)).unwrap();
    assert_eq!(stale_body["error"]["code"], "PLAN_STALE", "{stale_body}");
    assert_eq!(fixture.git_file_count(), 3, "a stale digest moves nothing");
    assert_eq!(fixture.r2_file_count(), 0);

    let digest = preview(&fixture)["data"]["planDigest"].as_str().unwrap().to_string();
    let fresh = commit(&fixture, &digest);
    assert!(fresh.status.success(), "stderr: {}", support::stderr(&fresh));
    assert_eq!(fixture.git_file_count(), 1);
    assert_eq!(fixture.r2_file_count(), 2);
}

/// Every file under `root` with its bytes, sorted: a whole-repo before/after snapshot.
fn snapshot(root: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, Vec<u8>)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                out.push((path, bytes));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out.sort();
    out
}

#[test]
fn the_old_tier_age_flags_are_usage_errors() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    // The Given: a record past its threshold, so the bare run WOULD have something to move.
    fixture.plant_trajectory_in_git(Utc::now() - Duration::days(30), 0.3);
    let candidates = preview(&fixture)["data"]["rules"][0]["candidates"].clone();
    assert_eq!(candidates, 1, "the fixture holds one record past its threshold");

    let before = snapshot(fixture.root.path());
    let (git_before, r2_before) = (fixture.git_file_count(), fixture.r2_file_count());
    assert_eq!(git_before, 1, "the aged record sits in the git tier");
    for args in [&["tier", "age", "--dry-run"][..], &["tier", "age", "--canon-yaml", "x"], &["tier", "age", "--commit"]] {
        let output = fixture.run_canon(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty() || !support::stdout(&output).contains("\"committed\":true"), "{args:?} must not commit");
    }
    assert_eq!(fixture.git_file_count(), git_before, "nothing was removed from the git tier");
    assert_eq!(fixture.r2_file_count(), r2_before, "nothing was written to the r2 tier");
    assert!(snapshot(fixture.root.path()) == before, "no byte of the repo changed across the three refused invocations");

    // And the bare run is the preview: it too writes nothing.
    let _ = preview(&fixture);
    assert!(snapshot(fixture.root.path()) == before, "the bare run previews and writes nothing");
}

#[test]
fn no_aging_rules_configured_is_a_clean_no_op() {
    let fixture = support::Fixture::new(ROUTING, "");
    let output = fixture.run_canon(&["tier", "age"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    assert!(support::stdout(&output).contains("no `aging` rules configured"));
}
