//! 0.28 D11 through the real binary (T18, #116, #117): a chore approval asks
//! for new evidence only where the code changed; when it did, the ship guard,
//! `status` and `meta.next` lead with ONE batch command built from the real
//! report paths, which runs as written; and a bound report decides the
//! verdict of a single-scenario `evidence add` as it does the batch's.

use std::path::Path;
use std::process::{Command, Output};

use canon_model::{Actor, Change, ChangeId, ChangeStatus, Envelope, RecordKind, RoleId, Subject, SubjectId, SubjectStatus};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::{Duration, SecondsFormat, Utc};
use serde_json::Value;
use tempfile::TempDir;

const FEATURE: &str = ".canon/specs/kind=feature/area=shop/cart.feature";
const REQUIRE_REVIEW: &str = "spec_coverage:\n  require_evidence: true\n  require_review: {}\n";
const REPORT_A: &str = r#"<testsuite><testcase classname="cart" name="shop.cart.01 adds"/></testsuite>"#;
const REPORT_B: &str = r#"<testsuite><testcase classname="cart" name="shop.cart.02 counts"/></testsuite>"#;
const BATCH: &str = "canon evidence add --all-scenarios --report junit:a.xml --report junit:b.xml --role implementer";

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").env_remove("CANON_ROLE").current_dir(repo).output().expect("spawn canon")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

fn ok(repo: &Path, args: &[&str]) -> String {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}{}", text(&out.stdout), text(&out.stderr));
    text(&out.stdout)
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").current_dir(repo).args(args).output().expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", text(&out.stderr));
}

fn write(repo: &Path, rel: &str, body: &str) {
    let path = repo.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn commit_all(repo: &Path, message: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", message]);
}

fn feature() -> String {
    let stamp = "# canon: {\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{\"agent_id\":\"canon\"}}";
    let mut text = format!("Feature: shop cart\n  {stamp}\n");
    for n in 1..=2 {
        text.push_str(&format!("\n  {stamp}\n  @subject:demo\n  @shop.cart.0{n}\n  Scenario: Cart behavior {n}\n    When the shopper acts\n    Then it works\n"));
    }
    text
}

fn actor(id: &str, role: &str) -> Actor {
    Actor::new(id, RoleId::parse(role).unwrap())
}

fn chore_brief(approved: chrono::DateTime<Utc>) -> String {
    format!("# Bump the client\n\n<!-- canon:kind chore -->\n\n## Intent\n\nBump the client\n\nApproved: Journey {} — \"go\"\n", approved.to_rfc3339_opts(SecondsFormat::Secs, true))
}

/// A git repo with code, two scenarios of subject `demo` evidenced by the
/// reports `a.xml` and `b.xml` and reviewed, the subject `verifying`, and the
/// whole thing committed.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let r = dir.path();
    git(r, &["init", "-q", "-b", "main"]);
    git(r, &["config", "user.name", "t"]);
    git(r, &["config", "user.email", "t@example.com"]);
    ok(r, &["init", "--repo", ".", "--no-policy"]);
    write(r, ".canon/policy.yaml", REQUIRE_REVIEW);
    write(r, FEATURE, &feature());
    write(r, "src/lib.rs", "pub fn cart() {}\n");
    write(r, "a.xml", REPORT_A);
    write(r, "b.xml", REPORT_B);
    ok(r, &["inventory", "sync"]);

    let ledger = GitTier::new(r.join(".canon/ledger"));
    let at = Utc::now() - Duration::seconds(600);
    for change in ["add-cart", "bump-client"] {
        let envelope = Envelope::new(1, RecordKind::Change, at, actor("importer", "planner"));
        ledger.write(&Change::new(envelope, ChangeId::parse(change).unwrap(), "Seeded", "why", ChangeStatus::InProgress)).unwrap();
    }
    let envelope = Envelope::new(1, RecordKind::Subject, at, actor("canon", "implementer"));
    let subject = Subject::new(envelope, SubjectId::parse("demo").unwrap(), "Demo", "s", "dev", SubjectStatus::Verifying, RoleId::parse("implementer").unwrap())
        .with_change_ids(["add-cart", "bump-client"].iter().map(|c| ChangeId::parse(*c).unwrap()).collect());
    ledger.write(&subject).unwrap();
    write(r, ".canon/changes/add-cart/brief.md", "## Why\n\nSeeded.\n\nApproved: Journey 2026-10-12 — \"go\"\n");
    commit_all(r, "setup");

    ok(r, &["evidence", "add", "--all-scenarios", "--report", "junit:a.xml", "--report", "junit:b.xml", "--role", "implementer"]);
    for scenario in ["shop.cart.01", "shop.cart.02"] {
        ok(r, &["review", "scenario", scenario, "--actor-id", "reviewer-2", "--role", "reviewer", "--original-spec-ref", "docs/prd.md"]);
    }
    commit_all(r, "baseline");
    dir
}

/// The chore is approved a little after the evidence.
fn approve_chore(repo: &Path) {
    write(repo, ".canon/changes/bump-client/brief.md", &chore_brief(Utc::now() + Duration::seconds(3)));
}

fn ship(repo: &Path) -> Output {
    canon(repo, &["subject", "move", "demo", "--to", "shipped"])
}

fn status_steps(repo: &Path) -> Vec<Value> {
    let report: Value = serde_json::from_str(&ok(repo, &["status", "--json"])).unwrap();
    report["meta"]["next"].as_array().unwrap().iter().filter(|s| s["why"].as_str().unwrap_or("").starts_with("demo")).cloned().collect()
}

fn subject_status(repo: &Path) -> String {
    let report: Value = serde_json::from_str(&ok(repo, &["query", "--kind", "subject", "--json"])).unwrap();
    report["data"]["records"][0]["status"].as_str().unwrap().to_string()
}

#[test]
fn a_docs_only_chore_needs_no_new_evidence_only_its_change_review() {
    let dir = repo();
    let r = dir.path();
    approve_chore(r);
    write(r, "docs/guide.md", "# Guide\n\nA docs-only chore.\n");
    write(r, "README.md", "readme\n");
    commit_all(r, "docs-only chore");

    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "the chore still needs its review: {}", text(&out.stderr));
    let err = text(&out.stderr);
    assert!(err.contains("no review of change `bump-client`"), "{err}");
    assert!(!err.contains("no evidence recorded after"), "evidence on the unchanged code needs nothing new: {err}");
    let steps = status_steps(r);
    assert!(steps[0]["command"].as_str().unwrap().starts_with("canon review change bump-client"), "{steps:?}");
    assert!(steps.iter().all(|s| !s["command"].as_str().unwrap().contains("evidence add")), "no evidence is asked for: {steps:?}");

    ok(r, &["review", "change", "bump-client", "--actor-id", "reviewer-3", "--role", "reviewer", "--original-spec-ref", ".canon/changes/bump-client/brief.md"]);
    let out = ship(r);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(subject_status(r), "shipped");
}

#[test]
fn a_chore_after_a_code_change_asks_for_every_scenario_with_one_runnable_batch_command() {
    let dir = repo();
    let r = dir.path();
    approve_chore(r);
    write(r, "src/lib.rs", "pub fn cart() { /* changed by the chore */ }\n");
    commit_all(r, "chore that touches code");

    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    let err = text(&out.stderr);
    assert!(err.contains("no evidence recorded after") && err.contains("`shop.cart.01`") && err.contains("`shop.cart.02`"), "{err}");
    let hint = err.lines().find(|l| l.starts_with("hint: ")).unwrap_or_else(|| panic!("a refusal carries a hint: {err}"));
    assert!(hint.contains(&format!("`{BATCH}`")), "the hint leads with the generated batch command: {hint}");
    assert!(!err.contains("--scenario-id"), "never a per-scenario loop: {err}");

    let steps = status_steps(r);
    assert_eq!(steps[0]["command"], BATCH, "{steps:?}");
    assert_eq!(steps.iter().filter(|s| s["command"].as_str().unwrap().starts_with("canon evidence add")).count(), 1, "one batch step: {steps:?}");

    // The command runs as written.
    let words: Vec<&str> = BATCH.split(' ').skip(1).collect();
    ok(r, &words);
    let out = ship(r);
    let err = text(&out.stderr);
    assert!(!err.contains("no evidence recorded after"), "the batch cleared the evidence gap: {err}");
    assert!(err.contains("no review of change `bump-client`"), "what remains is the chore's review: {err}");
}

fn latest_verdicts(repo: &Path, scenario: &str) -> Vec<String> {
    let report: Value = serde_json::from_str(&ok(repo, &["query", "--kind", "evidence", "--json"])).unwrap();
    let mut rows: Vec<(String, String)> = report["data"]["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["scenario_id"] == scenario)
        .map(|r| (r["at"].as_str().unwrap().to_string(), r["verdict"].as_str().unwrap().to_string()))
        .collect();
    rows.sort();
    rows.into_iter().map(|(_, v)| v).collect()
}

#[test]
fn a_bound_report_decides_the_verdict_of_a_single_scenario_add() {
    let dir = repo();
    let r = dir.path();
    write(r, "skipped.xml", r#"<testsuite><testcase name="shop.cart.01 adds"/><testcase name="shop.cart.01 flaky"><skipped/></testcase></testsuite>"#);
    write(r, "failed.xml", r#"<testsuite><testcase name="shop.cart.01 adds"/><testcase name="shop.cart.01 breaks"><failure/></testcase></testsuite>"#);
    let add = |report: &str, extra: &[&str]| {
        let mut args = vec!["evidence", "add", "--scenario-id", "shop.cart.01", "--report", report, "--role", "implementer"];
        args.extend_from_slice(extra);
        canon(r, &args)
    };
    let baseline = latest_verdicts(r, "shop.cart.01").len();

    let refused = add("junit:skipped.xml", &["--verdict", "faithful"]);
    assert_eq!(refused.status.code(), Some(1), "an explicit faithful over a skipped case stays refused: {}", text(&refused.stderr));
    assert_eq!(latest_verdicts(r, "shop.cart.01").len(), baseline, "a refusal writes nothing");

    let out = add("junit:skipped.xml", &[]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(latest_verdicts(r, "shop.cart.01").last().map(String::as_str), Some("not_applicable"));

    let refused = add("junit:failed.xml", &["--verdict", "faithful"]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused.stderr));
    let out = add("junit:failed.xml", &[]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(latest_verdicts(r, "shop.cart.01").last().map(String::as_str), Some("divergent"));

    let out = add("junit:a.xml", &[]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(latest_verdicts(r, "shop.cart.01").last().map(String::as_str), Some("faithful"));
}

#[test]
fn an_override_is_recorded_echoed_and_listed_by_status() {
    let dir = repo();
    let r = dir.path();
    // The subject's change has a brief nobody answered: the attestation is refused.
    for change in ["add-cart", "bump-client"] {
        write(r, &format!(".canon/changes/{change}/brief.md"), "## Why\n\nSeeded, never answered.\n");
    }
    let refused = canon(r, &["evidence", "add", "--scenario-id", "shop.cart.01", "--report", "junit:a.xml", "--role", "implementer"]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused.stderr));
    let waived = canon(r, &["evidence", "add", "--scenario-id", "shop.cart.01", "--report", "junit:a.xml", "--role", "implementer", "--override-reason", "ship it", "--json"]);
    assert!(waived.status.success(), "{}{}", text(&waived.stdout), text(&waived.stderr));
    let report: Value = serde_json::from_str(&text(&waived.stdout)).unwrap();
    let overrides = report["data"]["overrides"].as_array().expect("data.overrides[] is always present");
    assert_eq!(overrides.len(), 1, "{report}");
    assert_eq!(overrides[0]["reason"], "ship it");
    assert!(overrides[0]["code"].as_str().unwrap().starts_with("BRIEF_"), "{report}");
    let status = ok(r, &["status", "--json"]);
    assert!(status.contains("ship it"), "status lists the override for the human: {status}");
}
