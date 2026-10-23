//! `canon migrate`'s review pinning (0.23 D6) through the real binary in a
//! real git repo. A review written before 0.15 carries no `pins`, only
//! `pin`, the commit it reviewed; one written in 0.15–0.22 pins evidence
//! record files. The gate has one freshness rule (what the evidence says),
//! so neither kind counts until `canon migrate` rewrites it: a review that
//! was current when it was written becomes a current 0.23 review, a stale
//! one stays stale, and a commit this clone cannot read pins the review as
//! the scenario stands.

use std::path::Path;
use std::process::{Command, Output};

use canon_model::RecordKind;

const FEATURE: &str = ".canon/specs/kind=feature/area=cart/add.feature";
const MOVED: &str = ".canon/specs/kind=feature/area=cart/basket.feature";

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
    git(repo, &["add", "-A", ".canon/specs"]);
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
    ok(repo, &["evidence", "add", "--scenario-id", id, "--project-id", "root", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent"]);
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


const MIGRATED: &str = "canon migrate: pinned";

/// The reviews of `scenario` in the ledger.
fn reviews_of(repo: &Path, scenario: &str) -> Vec<serde_json::Value> {
    walk(&repo.join(".canon/ledger/kind=review"))
        .into_iter()
        .map(|p| serde_json::from_slice::<serde_json::Value>(&std::fs::read(p).unwrap()).unwrap())
        .filter(|r| r["scenario_id"] == scenario)
        .collect()
}

/// Every file under `dir`, recursively.
fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files
}

fn commit_ledger(repo: &Path) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "--allow-empty", "-m", "ledger"]);
}

fn migrate(repo: &Path) -> String {
    commit_ledger(repo);
    let out = ok(repo, &["migrate"]);
    commit_ledger(repo);
    out
}

/// migrate.pins.01: a pre-0.15 review does not count before `canon migrate`
/// (the gate says why and names the command); after it, both reviews of
/// unchanged scenarios are current 0.23 reviews, pinned to what the
/// evidence says, and the gate is clean.
#[test]
fn migrate_pins_01_a_current_legacy_review_counts_only_after_migrate() {
    let (dir, _) = legacy_repo();
    let repo = dir.path();
    let before = gate(repo, 1);
    assert!(before.contains("predates 0.23 review pinning") && before.contains("`canon migrate`"), "{before}");

    let out = migrate(repo);
    assert!(out.contains(&format!("{MIGRATED} 2 review(s) to what their evidence says")), "{out}");
    for id in ["cart.add.01", "cart.add.02"] {
        let review = &reviews_of(repo, id)[0];
        assert!(review["pins"]["checks"].is_array(), "{review}");
        assert_eq!(review["pins"]["checks"][0]["verdict"], "faithful", "{review}");
        assert_eq!(review["reviewer"], "reviewer-2", "the converted review keeps its reviewer: {review}");
        assert!(review["pins"].get("evidence").is_none(), "no record ids any more: {review}");
    }
    let after = gate(repo, 0);
    assert!(!after.contains("predates 0.23"), "{after}");
    assert!(out.contains("canon migrate") && !migrate(repo).contains(MIGRATED), "a second run finds nothing to convert");
}

/// migrate.pins.02: an edit that leaves the scenario's meaning alone keeps a
/// legacy review current through the migration; a scenario whose steps
/// changed after the pin stays stale, and `canon review add` makes it
/// current again.
#[test]
fn migrate_pins_02_a_legacy_review_of_a_changed_scenario_stays_stale() {
    let (dir, pin) = legacy_repo();
    let repo = dir.path();
    commit_file(repo, FEATURE, &BASE.replace("Then the cart holds two of that item", "Then the cart holds one line with quantity two"), "edit cart.add.02");
    ok(repo, &["inventory", "sync"]);

    let out = migrate(repo);
    assert!(out.contains(&format!("{MIGRATED} 1 review(s)")) && out.contains("left 1 stale review(s)"), "{out}");
    assert!(reviews_of(repo, "cart.add.01")[0]["pins"]["checks"].is_array(), "the sibling is converted");
    let stale = &reviews_of(repo, "cart.add.02")[0];
    assert!(stale.get("pins").is_none(), "a stale review is left as it was: {stale}");
    assert_eq!(stale["pin"], pin.as_str());

    let out = gate(repo, 1);
    assert!(out.contains("unreviewed-promotion cart.add.02") && !out.contains("unreviewed-promotion cart.add.01"), "{out}");

    ok(
        repo,
        &[
            "review", "scenario", "cart.add.02", "--project-id", "root", "--pin", "r2", "--upstream-ref", "docs/plan.md#re-review",
            "--actor-id", "reviewer-2", "--role", "reviewer", "--session-id", "sess-review",
        ],
    );
    gate(repo, 0);
}

/// migrate.pins.03: a pre-0.15 review older than its scenario's declared
/// evidence is stale and stays stale through the migration.
#[test]
fn migrate_pins_03_a_legacy_review_older_than_the_evidence_stays_stale() {
    let (dir, _) = legacy_repo();
    let repo = dir.path();
    attest(repo, "cart.add.01");
    let out = migrate(repo);
    assert!(out.contains("left 1 stale review(s)"), "{out}");
    assert!(reviews_of(repo, "cart.add.01")[0].get("pins").is_none());
    let out = gate(repo, 1);
    assert!(out.contains("unreviewed-promotion cart.add.01"), "{out}");
}

/// migrate.pins.04: a pin this clone does not have, and a commit that
/// predates the scenario, leave the review undecidable: it is pinned as the
/// scenario stands and counts, and the migration says how many it could not
/// check.
#[test]
fn migrate_pins_04_an_unreadable_pin_is_pinned_as_the_scenario_stands() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_review:\n    scope: []\n").unwrap();
    let before = commit_file(repo, ".canon/specs/README.md", "specs land here\n", "before the scenarios");
    commit_file(repo, FEATURE, BASE, "cart add");
    ok(repo, &["inventory", "sync"]);
    attest(repo, "cart.add.01");
    attest(repo, "cart.add.02");
    plant_legacy_review(repo, "cart.add.01", "0123456789abcdef0123456789abcdef01234567");
    plant_legacy_review(repo, "cart.add.02", &before);

    let out = migrate(repo);
    assert!(out.contains("2 of those could not be checked"), "{out}");
    let out = gate(repo, 0);
    assert!(out.contains("canon gate check: clean (0 violations)"), "{out}");
}

/// migrate.pins.05: a scenario that lived in another `.feature` file at the
/// pinned commit is found there, so moving it between files does not strand
/// its legacy review.
#[test]
fn migrate_pins_05_a_legacy_review_finds_its_scenario_in_the_file_it_lived_in() {
    let (dir, _) = legacy_repo();
    let repo = dir.path();
    std::fs::remove_file(repo.join(FEATURE)).unwrap();
    commit_file(repo, MOVED, &BASE.replace("Feature: cart add", "Feature: cart basket"), "move the cart scenarios");
    ok(repo, &["inventory", "sync"]);
    let out = migrate(repo);
    assert!(out.contains(&format!("{MIGRATED} 2 review(s)")) && !out.contains("could not be checked"), "{out}");
    gate(repo, 0);
}

/// migrate.pins.06: a 0.15-0.22 review pins the evidence record files it
/// saw. When the latest evidence per role is still that file the review is
/// converted faithfully (same reviewer, session and outcome, the verdict
/// and checks the evidence says); when newer evidence exists it stays stale.
#[test]
fn migrate_pins_06_a_record_pinned_review_converts_when_its_evidence_is_still_the_latest() {
    let (dir, _) = legacy_repo();
    let repo = dir.path();
    // Replace the planted pre-0.15 reviews by 0.15-0.22 ones: a `pins` block naming the evidence files.
    std::fs::remove_dir_all(repo.join(".canon/ledger/kind=review")).unwrap();
    let scenario_digest = |id: &str| {
        let file = walk(&repo.join(".canon/ledger/kind=scenario")).into_iter().map(|p| serde_json::from_slice::<serde_json::Value>(&std::fs::read(p).unwrap()).unwrap()).find(|r| r["scenario_id"] == id).unwrap();
        file["scenario_digest"].clone()
    };
    let evidence_pin = |id: &str| {
        let path = walk(&repo.join(".canon/ledger/kind=evidence_record")).into_iter().find(|p| std::fs::read_to_string(p).unwrap().contains(id)).unwrap();
        let name = path.file_name().unwrap().to_str().unwrap().trim_end_matches(".json").to_string();
        let (key, digest) = name.rsplit_once("__").unwrap();
        serde_json::json!({"key": key, "digest": digest})
    };
    for id in ["cart.add.01", "cart.add.02"] {
        let review = serde_json::json!({
            "schema": 1,
            "kind": "review",
            "at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
            "actor": {"agent_id": "reviewer-2", "role": "reviewer", "session_id": "sess-review"},
            "project_id": "root",
            "scenario_id": id,
            "reviewer": "reviewer-2",
            "pin": "p1",
            "provenance_ref": {"upstream_ref": "docs/plan.md#review"},
            "pins": {"scenario_digest": scenario_digest(id), "scenario_digest_scope": "scenario", "evidence": [evidence_pin(id)]},
        });
        let path = repo.join(".canon/ledger").join(canon_store::partition::expected_relative_path(RecordKind::Review, &review).unwrap());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_vec_pretty(&review).unwrap()).unwrap();
    }
    gate(repo, 1);

    // A newer evidence record for cart.add.02 makes its review stale under the old rule.
    attest(repo, "cart.add.02");
    let out = migrate(repo);
    assert!(out.contains(&format!("{MIGRATED} 1 review(s)")) && out.contains("left 1 stale review(s)"), "{out}");
    let converted = &reviews_of(repo, "cart.add.01")[0];
    assert_eq!(converted["actor"]["session_id"], "sess-review", "{converted}");
    assert_eq!(converted["pins"]["checks"][0]["verdict"], "faithful", "{converted}");
    assert!(converted["pins"].get("evidence").is_none(), "{converted}");
    assert!(reviews_of(repo, "cart.add.02")[0]["pins"].get("checks").is_none(), "the stale review keeps its old pin");
}

/// migrate.pins.07: a spec root `canon migrate` moved (`specs` →
/// `.canon/specs`, `former_roots: [specs]`) is searched at its old path at a
/// pin from before the move, and only for its own project: another root
/// declaring the same scenario id with the same text never answers for it.
/// Project `a`'s review is pinned as current from its former root; project
/// `b`'s, pinned to a commit before `b` existed, is undecidable although
/// `a`'s identical scenario sat in the repo then.
#[test]
fn migrate_pins_07_a_moved_root_is_searched_at_its_former_path_and_only_for_its_own_project() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_review:\n    scope: []\n").unwrap();
    let yaml = std::fs::read_to_string(repo.join("canon.yaml")).unwrap();
    let roots = "    - id: root\n      root: .canon/specs\n";
    assert!(yaml.contains(roots), "{yaml}");
    std::fs::write(repo.join("canon.yaml"), yaml.replace(roots, "    - id: a\n      root: specs\n")).unwrap();
    let old = "specs/kind=feature/area=cart/add.feature";
    std::fs::create_dir_all(repo.join(old).parent().unwrap()).unwrap();
    std::fs::write(repo.join(old), BASE).unwrap();
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "project a at specs"]);
    let pin = git(repo, &["rev-parse", "HEAD"]);

    std::fs::create_dir_all(repo.join(".canon/specs")).unwrap();
    git(repo, &["mv", "specs/kind=feature", ".canon/specs/kind=feature"]);
    std::fs::write(
        repo.join("canon.yaml"),
        yaml.replace(roots, "    - id: a\n      root: .canon/specs\n      former_roots: [specs]\n    - id: b\n      root: b/specs\n"),
    )
    .unwrap();
    commit_file(repo, &format!("b/{old}"), BASE, "project b");
    for project in ["a", "b"] {
        ok(repo, &["evidence", "add", "--scenario-id", "cart.add.01", "--project-id", project, "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent"]);
        let legacy = serde_json::json!({
            "schema": 1,
            "kind": "review",
            "at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
            "actor": {"agent_id": "reviewer-2", "role": "reviewer"},
            "project_id": project,
            "scenario_id": "cart.add.01",
            "reviewer": "reviewer-2",
            "pin": pin,
            "provenance_ref": {"upstream_ref": "docs/plan.md#review"},
        });
        let path = repo.join(".canon/ledger").join(canon_store::partition::expected_relative_path(RecordKind::Review, &legacy).unwrap());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();
    }
    let out = migrate(repo);
    assert!(out.contains(&format!("{MIGRATED} 2 review(s)")) && out.contains("1 of those could not be checked"), "a is current at its former root, b is undecidable: {out}");
}
