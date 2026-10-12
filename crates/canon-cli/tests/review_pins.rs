//! review.add.28 (0.15 D1) through the real binary: `canon inventory sync`
//! records each scenario's own digest, and `canon review add` pins it, so
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
                "review", "add", "--project-id", "root", "--scenario-id", id, "--reviewer", "reviewer-2", "--pin", "r1", "--original-spec-ref", FEATURE, "--actor-id",
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
