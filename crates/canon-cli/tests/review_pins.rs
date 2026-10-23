//! review.add.28 (0.15 D1) through the real binary: `canon inventory sync`
//! records each scenario's own digest, and `canon review scenario` pins it, so
//! a review goes stale when its scenario, or the context the scenario
//! inherits, changes — never because a sibling scenario in the same
//! `.feature` file did.

use std::path::Path;
use std::process::{Command, Output};

const FEATURE: &str = ".canon/specs/kind=feature/area=cart/add.feature";

const BASE: &str = r#"Feature: cart add
  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}

  Background:
    Given an empty cart

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @cart.add.01
  Scenario: Adding an item puts it in the cart
    When the shopper adds an item
    Then the cart holds that item

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @cart.add.02
  Scenario: Adding the same item twice counts it twice
    When the shopper adds an item twice
    Then the cart holds two of that item
"#;

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn ok(repo: &Path, args: &[&str]) -> String {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}", text(&out));
    text(&out)
}

fn write_feature(repo: &Path, text: &str) {
    let path = repo.join(FEATURE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    ok(repo, &["inventory", "sync"]);
}

/// The gate's output, asserting its exit code.
fn gate(repo: &Path, exit: i32) -> String {
    let out = canon(repo, &["gate", "check"]);
    assert_eq!(out.status.code(), Some(exit), "{}", text(&out));
    text(&out)
}

/// A repo with `require_review` over every scenario, both scenarios
/// attested by `impl-agent` and reviewed (pinned) by `reviewer-2`.
fn reviewed_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(repo).status().unwrap().success());
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_review:\n    scope: []\n").unwrap();
    write_feature(repo, BASE);
    for id in ["cart.add.01", "cart.add.02"] {
        ok(repo, &["evidence", "add", "--scenario-id", id, "--project-id", "root", "--kind", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent"]);
    }
    ok(repo, &["gate", "promote"]);
    assert!(gate(repo, 1).contains("unreviewed-promotion cart.add.01 — no review record"), "require_review covers these scenarios");
    for id in ["cart.add.01", "cart.add.02"] {
        let out = ok(
            repo,
            &[
                "review", "scenario", id, "--project-id", "root", "--pin", "r1", "--original-spec-ref", FEATURE, "--actor-id",
                "reviewer-2", "--role", "reviewer", "--session-id", "sess-review",
            ],
        );
        assert!(out.contains(&format!("pinned {id} at scenario digest ")), "{out}");
    }
    gate(repo, 0);
    dir
}

fn stale_line(id: &str) -> String {
    format!("unreviewed-promotion {id} — stale review by `reviewer-2`: its spec changed (the scenario's digest is not the pinned one)")
}

#[test]
fn appending_a_scenario_leaves_its_siblings_reviews_current() {
    let dir = reviewed_repo();
    let repo = dir.path();
    write_feature(
        repo,
        &format!("{BASE}\n  # canon: {{\"schema\":1,\"at\":\"2026-10-11T00:00:00Z\",\"actor\":{{\"agent_id\":\"canon\"}}}}\n  @cart.add.03\n  Scenario: Removing an item empties the cart\n    When the shopper removes the item\n    Then the cart is empty\n"),
    );
    let out = gate(repo, 1);
    assert!(out.contains("unreviewed-promotion cart.add.03 — no review record"), "{out}");
    assert!(!out.contains("cart.add.01") && !out.contains("cart.add.02"), "the siblings' reviews stay current: {out}");
}

#[test]
fn editing_a_scenarios_steps_stales_only_that_scenario() {
    let dir = reviewed_repo();
    let repo = dir.path();
    write_feature(repo, &BASE.replace("Then the cart holds two of that item", "Then the cart holds one of that item with quantity two"));
    let out = gate(repo, 1);
    assert!(out.contains(&stale_line("cart.add.02")), "{out}");
    assert!(!out.contains("cart.add.01"), "{out}");
}

#[test]
fn editing_the_background_stales_every_scenario_that_inherits_it() {
    let dir = reviewed_repo();
    let repo = dir.path();
    write_feature(repo, &BASE.replace("Given an empty cart", "Given a cart holding one item"));
    let out = gate(repo, 1);
    assert!(out.contains(&stale_line("cart.add.01")) && out.contains(&stale_line("cart.add.02")), "{out}");
}

// ── 0.23 D5: a review pins what the evidence says, not which record it is ──

const REPORT: &str = r#"<testsuite><testcase name="cart.add.01 adds"/><testcase name="cart.add.02 counts"/></testsuite>"#;

fn record_suite(repo: &Path, report: &str) {
    std::fs::create_dir_all(repo.join("reports")).unwrap();
    std::fs::write(repo.join("reports/junit.xml"), report).unwrap();
    ok(
        repo,
        &[
            "evidence", "add", "--project-id", "root", "--all-scenarios", "--report", "junit:reports/junit.xml", "--kind", "test-run", "--ref", "npm test", "--role", "implementer",
            "--actor-id", "impl-agent",
        ],
    );
    ok(repo, &["gate", "promote"]);
}

/// Every evidence record ever written: the ledger holds the latest per key
/// and the archive the ones it superseded (0.23 D2).
fn evidence_records(repo: &Path) -> usize {
    [".canon/ledger/kind=evidence_record", ".canon/archive/kind=evidence_record"]
        .iter()
        .map(|dir| std::fs::read_dir(repo.join(dir)).map(|d| d.count()).unwrap_or(0))
        .sum()
}

/// A repo whose evidence is the suite's report and whose scenarios are both
/// reviewed against it.
fn reviewed_against_a_report() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(repo).status().unwrap().success());
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_review:\n    scope: []\n").unwrap();
    write_feature(repo, BASE);
    record_suite(repo, REPORT);
    for id in ["cart.add.01", "cart.add.02"] {
        ok(
            repo,
            &[
                "review", "scenario", id, "--project-id", "root", "--pin", "r1", "--original-spec-ref", FEATURE, "--actor-id", "reviewer-2",
                "--role", "reviewer", "--session-id", "sess-review",
            ],
        );
    }
    gate(repo, 0);
    dir
}

/// review.add.30: recording the suite again with the same verdicts from the
/// same cases is new evidence that says the same thing, so every review
/// stays current and no review is owed.
#[test]
fn review_add_30_re_recording_the_suite_with_the_same_verdicts_keeps_every_review_current() {
    let dir = reviewed_against_a_report();
    let repo = dir.path();
    let before = evidence_records(repo);
    record_suite(repo, REPORT);
    assert!(evidence_records(repo) > before, "the suite was recorded again");
    let out = gate(repo, 0);
    assert!(!out.contains("stale review") && !out.contains("unreviewed-promotion"), "{out}");
}

/// review.add.31: a case that appears (or disappears) changes what the
/// evidence names, so the review of that scenario is stale and its sibling's
/// is not.
#[test]
fn review_add_31_a_changed_case_set_stales_only_that_scenarios_review() {
    let dir = reviewed_against_a_report();
    let repo = dir.path();
    record_suite(repo, r#"<testsuite><testcase name="cart.add.01 adds"/><testcase name="cart.add.01 adds twice"/><testcase name="cart.add.02 counts"/></testsuite>"#);
    let out = gate(repo, 1);
    assert!(out.contains("unreviewed-promotion cart.add.01 — stale review by `reviewer-2`: its evidence changed"), "{out}");
    assert!(!out.contains("cart.add.02"), "{out}");
}

/// review.add.32: a different verdict stales the review of that scenario.
#[test]
fn review_add_32_a_changed_verdict_stales_the_review() {
    let dir = reviewed_against_a_report();
    let repo = dir.path();
    ok(
        repo,
        &[
            "evidence", "add", "--project-id", "root", "--scenario-id", "cart.add.02", "--kind", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent",
            "--verdict", "divergent", "--summary", "the count is off by one",
        ],
    );
    ok(repo, &["gate", "promote"]);
    let out = gate(repo, 1);
    assert!(out.contains("unreviewed-promotion cart.add.02 — stale review by `reviewer-2`: its evidence changed"), "{out}");
    assert!(!out.contains("unreviewed-promotion cart.add.01"), "{out}");
}
