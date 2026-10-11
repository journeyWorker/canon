//! gate.check.90–94 and review.add.60–61 (0.16 D6) through the real
//! binary in a real git repo: a review written before 0.15 carries no
//! `pins`, only `pin`, the commit it reviewed. The gate reads the scenario
//! at that commit, with the same per-scenario digest `canon inventory
//! sync` stores, and keeps the review only while that digest is the
//! current one and no declared evidence is newer than the review. A pin
//! the clone cannot read keeps the 0.15 behavior: the review counts, and
//! the advisory says how many were left unchecked.

use std::path::Path;
use std::process::{Command, Output};

use canon_model::RecordKind;

const FEATURE: &str = "specs/features/kind=feature/area=cart/add.feature";
const MOVED: &str = "specs/features/kind=feature/area=cart/basket.feature";

const BASE: &str = r#"Feature: cart add
  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}

  Background:
    Given an empty cart

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:cart
  @cart.add.01
  Scenario: Adding an item puts it in the cart
    When the shopper adds an item
    Then the cart holds that item

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:cart
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

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git must be on PATH");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Write `path` and commit the spec tree; the new commit's sha.
fn commit_file(repo: &Path, path: &str, body: &str, message: &str) -> String {
    let file = repo.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, body).unwrap();
    git(repo, &["add", "-A", "specs"]);
    git(repo, &["commit", "-q", "-m", message]);
    git(repo, &["rev-parse", "HEAD"])
}

/// The gate's stdout and stderr, asserting its exit code.
fn gate(repo: &Path, exit: i32) -> String {
    let out = canon(repo, &["gate", "check"]);
    assert_eq!(out.status.code(), Some(exit), "{}", text(&out));
    text(&out)
}

fn attest(repo: &Path, id: &str) {
    ok(repo, &["evidence", "add", "--scenario-id", id, "--project-id", "root", "--kind", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent"]);
    ok(repo, &["gate", "promote"]);
}

/// A review written before 0.15, as one sits in the ledger: no `pins`, no
/// `outcome`, only the commit it reviewed. Written now, so after every
/// evidence record already in the ledger.
fn plant_legacy_review(repo: &Path, id: &str, pin: &str) {
    let legacy = serde_json::json!({
        "schema": 1,
        "kind": "review",
        "at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
        "actor": {"agent_id": "reviewer-2", "role": "reviewer"},
        "project_id": "root",
        "scenario_id": id,
        "reviewer": "reviewer-2",
        "pin": pin,
        "provenance_ref": {"upstream_ref": "docs/plan.md#review"},
    });
    let relative = canon_store::partition::expected_relative_path(RecordKind::Review, &legacy).unwrap();
    let path = repo.join(".canon/ledger").join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();
}

/// A git repo with `require_review` over every scenario: `BASE` committed
/// (the returned sha), both scenarios attested by `impl-agent`, and both
/// reviewed by `reviewer-2` before review pinning, pinned to that commit.
fn legacy_repo() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_review:\n    scope: []\n").unwrap();
    let pin = commit_file(repo, FEATURE, BASE, "cart add");
    ok(repo, &["inventory", "sync"]);
    for id in ["cart.add.01", "cart.add.02"] {
        attest(repo, id);
        plant_legacy_review(repo, id, &pin);
    }
    (dir, pin)
}

fn advisory(current: usize, stale: usize, undecidable: usize) -> String {
    format!(
        "advisory: {} review(s) predate review pinning (0.15) and were checked against the commit each one pinned: {current} current, {stale} stale, {undecidable} undecidable",
        current + stale + undecidable
    )
}

/// gate.check.90: an edit that leaves the scenarios' meaning alone (a
/// comment, re-indented steps) keeps both legacy reviews current, though
/// the file's bytes changed; the advisory counts them as checked-current.
#[test]
fn a_legacy_review_of_an_unchanged_scenario_counts_and_is_checked_current() {
    let (dir, _) = legacy_repo();
    let repo = dir.path();
    commit_file(repo, FEATURE, &format!("{}\n  # a comment the digest ignores\n", BASE.replace("    When the shopper adds an item\n", "      When the shopper adds an item\n")), "reflow");
    ok(repo, &["inventory", "sync"]);
    let out = gate(repo, 0);
    assert!(out.contains(&format!("{}\n", advisory(2, 0, 0))), "{out}");
    assert!(!out.contains("fetch-depth"), "nothing was left unchecked: {out}");
}

/// gate.check.91: a scenario whose steps changed after the pinned commit
/// has a stale legacy review, named with the pin; its sibling stays
/// current. `canon status` prints the same split and counts the gap.
#[test]
fn a_legacy_review_whose_scenario_changed_after_its_pin_is_stale() {
    let (dir, pin) = legacy_repo();
    let repo = dir.path();
    commit_file(repo, FEATURE, &BASE.replace("Then the cart holds two of that item", "Then the cart holds one line with quantity two"), "edit cart.add.02");
    ok(repo, &["inventory", "sync"]);
    let out = gate(repo, 1);
    let line = format!(
        "unreviewed-promotion cart.add.02 — stale review by `reviewer-2` (written before review pinning; checked against commit {pin}): its spec changed since {pin} (the scenario's digest at that commit is not its current one); a review counts only while what it saw is current — review it again (`canon review add`)"
    );
    assert!(out.contains(&line), "{out}");
    assert!(!out.contains("unreviewed-promotion cart.add.01"), "the sibling's review stays current: {out}");
    assert!(out.contains(&format!("{}; a stale one no longer counts — review its scenario again (`canon review add`): cart.add.02\n", advisory(1, 1, 0))), "{out}");

    let status = ok(repo, &["status"]);
    assert!(status.contains(&format!("warning: {}", advisory(1, 1, 0).trim_start_matches("advisory: "))), "{status}");
}

/// gate.check.92: declared evidence recorded after a legacy review stales
/// it even though the scenario is unchanged since the pin.
#[test]
fn a_legacy_review_older_than_its_scenarios_evidence_is_stale() {
    let (dir, pin) = legacy_repo();
    let repo = dir.path();
    attest(repo, "cart.add.01");
    let out = gate(repo, 1);
    assert!(
        out.contains(&format!("unreviewed-promotion cart.add.01 — stale review by `reviewer-2` (written before review pinning; checked against commit {pin}): it has declared evidence newer than the review;")),
        "{out}"
    );
    assert!(out.contains(&advisory(1, 1, 0)), "{out}");
}

/// gate.check.93: a pin this clone does not have (a fake sha, as after a
/// shallow clone or a rewrite) and a commit that predates the scenario
/// both leave the review undecidable: it counts, the gate passes, and the
/// advisory suggests full history.
#[test]
fn a_legacy_review_whose_pin_cannot_be_read_counts_with_an_advisory() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_review:\n    scope: []\n").unwrap();
    let before = commit_file(repo, "specs/README.md", "specs land here\n", "before the scenarios");
    commit_file(repo, FEATURE, BASE, "cart add");
    ok(repo, &["inventory", "sync"]);
    attest(repo, "cart.add.01");
    attest(repo, "cart.add.02");
    plant_legacy_review(repo, "cart.add.01", "0123456789abcdef0123456789abcdef01234567");
    plant_legacy_review(repo, "cart.add.02", &before);
    let out = gate(repo, 0);
    assert!(out.contains("canon gate check: clean (0 violations)"), "{out}");
    assert!(
        out.contains(&format!(
            "{}; an undecidable one still counts unchecked, because its pinned commit or the scenario at it is not in this clone — check out full history (`fetch-depth: 0` in CI) to check it\n",
            advisory(0, 0, 2)
        )),
        "{out}"
    );
}

/// gate.check.94: a scenario that lived in another `.feature` file at the
/// pinned commit is found there, so moving it between files never stales
/// or strands its legacy review.
#[test]
fn a_legacy_review_finds_its_scenario_in_the_file_it_lived_in_at_the_pin() {
    let (dir, _) = legacy_repo();
    let repo = dir.path();
    std::fs::remove_file(repo.join(FEATURE)).unwrap();
    commit_file(repo, MOVED, &BASE.replace("Feature: cart add", "Feature: cart basket"), "move the cart scenarios");
    ok(repo, &["inventory", "sync"]);
    let out = gate(repo, 0);
    assert!(out.contains(&advisory(2, 0, 0)), "both reviews are checked at their old file: {out}");
}

/// review.add.60: `canon review add` on a scenario whose legacy review is
/// stale writes a pinned review that supersedes it: the gate is clean and
/// the advisory no longer counts that reviewer's legacy review.
#[test]
fn a_new_review_supersedes_a_stale_legacy_review() {
    let (dir, _) = legacy_repo();
    let repo = dir.path();
    commit_file(repo, FEATURE, &BASE.replace("Then the cart holds two of that item", "Then the cart holds one line with quantity two"), "edit cart.add.02");
    ok(repo, &["inventory", "sync"]);
    gate(repo, 1);
    ok(
        repo,
        &[
            "review", "add", "--project-id", "root", "--scenario-id", "cart.add.02", "--reviewer", "reviewer-2", "--pin", "r2", "--upstream-ref", "docs/plan.md#re-review",
            "--actor-id", "reviewer-2", "--role", "reviewer", "--session-id", "sess-review",
        ],
    );
    let out = gate(repo, 0);
    assert!(out.contains(&format!("{}\n", advisory(1, 0, 0))), "{out}");
}

/// review.add.61: `canon subject status` decides a legacy review the way
/// the gate does: a stale one refuses the move into review scope with the
/// gate's detail, and the guard prints the same split for the subject's
/// own scenarios; once the scenario is reviewed again the move goes
/// through. `canon status` counts the stale review as a stale gap.
#[test]
fn the_subject_status_guard_and_status_apply_the_same_legacy_decision() {
    let (dir, pin) = legacy_repo();
    let repo = dir.path();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_review: {}\n").unwrap();
    ok(repo, &["subject", "new", "cart", "--domain", "shop", "--title", "Cart"]);
    ok(repo, &["subject", "status", "cart", "specced"]);
    ok(repo, &["subject", "status", "cart", "building"]);
    commit_file(repo, FEATURE, &BASE.replace("Then the cart holds two of that item", "Then the cart holds one line with quantity two"), "edit cart.add.02");
    ok(repo, &["inventory", "sync"]);

    let status = ok(repo, &["status"]);
    assert!(status.contains(", 1 stale review(s)") && status.contains(&advisory(1, 1, 0).trim_start_matches("advisory: ").to_string()), "{status}");

    let refused = canon(repo, &["subject", "status", "cart", "verifying"]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    let err = String::from_utf8_lossy(&refused.stderr).to_string();
    assert!(err.contains(&format!("unreviewed-promotion cart.add.02 — building → verifying: stale review by `reviewer-2` (written before review pinning; checked against commit {pin}): its spec changed since {pin}")), "{err}");
    assert!(err.contains(&format!("canon subject status: {}", advisory(1, 1, 0))), "{err}");

    ok(
        repo,
        &[
            "review", "add", "--project-id", "root", "--scenario-id", "cart.add.02", "--reviewer", "reviewer-2", "--pin", "r2", "--upstream-ref", "docs/plan.md#re-review",
            "--actor-id", "reviewer-2", "--role", "reviewer", "--session-id", "sess-review",
        ],
    );
    let moved = canon(repo, &["subject", "status", "cart", "verifying"]);
    assert_eq!(moved.status.code(), Some(0), "{}", text(&moved));
}
