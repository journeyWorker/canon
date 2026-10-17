//! Re-verification before shipping (0.22 D2, #72, #77), against the built
//! binary: a blocker closed `fixed` and an approved chore each need every
//! evidenced scenario evidenced again, then re-reviewed, before the subject
//! ships. Records carry explicit times so the order is the test's, not the
//! clock's.

use std::path::Path;
use std::process::{Command, Output};

use canon_model::{
    Actor, Change, ChangeId, ChangeStatus, Envelope, EvidenceRecord, EvidenceVerdict, Finding, FindingSeverity, ProjectId, RecordKind, RoleId, Scenario, ScenarioId, Sha, SpecDigest, Subject, SubjectId, SubjectStatus,
};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use tempfile::TempDir;

const CANON_YAML: &str = "\
tiers:
  local: { backend: git, root: .canon/ledger }
routing:
  subject: local
  change: local
  scenario: local
  evidence_record: local
";

const REQUIRE_REVIEW: &str = "spec_coverage:\n  require_evidence: true\n  require_review: {}\n";

fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), CANON_YAML).unwrap();
    std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
    std::fs::write(dir.path().join(".canon/policy.yaml"), REQUIRE_REVIEW).unwrap();
    dir
}

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).env_remove("CANON_ROLE").output().expect("spawn canon binary")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn ledger(repo: &Path) -> GitTier {
    GitTier::new(repo.join(".canon/ledger"))
}

fn ago(seconds: i64) -> DateTime<Utc> {
    Utc::now() - Duration::seconds(seconds)
}

fn actor(id: &str, role: &str) -> Actor {
    Actor::new(id, RoleId::parse(role).unwrap())
}

fn seed_subject(repo: &Path, status: SubjectStatus, changes: &[&str], at: DateTime<Utc>) {
    let envelope = Envelope::new(1, RecordKind::Subject, at, actor("canon", "implementer"));
    let subject = Subject::new(envelope, SubjectId::parse("demo-subject").unwrap(), "Seeded", "s", "dev", status, RoleId::parse("implementer").unwrap())
        .with_change_ids(changes.iter().map(|c| ChangeId::parse(*c).unwrap()).collect());
    ledger(repo).write(&subject).unwrap();
}

fn seed_scenario(repo: &Path, id: &str) {
    let envelope = Envelope::new(1, RecordKind::Scenario, ago(600), actor("canon", "implementer"));
    let mut scenario = Scenario::new(envelope, ProjectId::parse("demo").unwrap(), ScenarioId::parse(id).unwrap(), "seeded", "", SpecDigest::of(id.as_bytes()));
    scenario.subject_id = Some(SubjectId::parse("demo-subject").unwrap());
    ledger(repo).write(&scenario).unwrap();
}

fn seed_change(repo: &Path, id: &str) {
    let envelope = Envelope::new(1, RecordKind::Change, ago(600), actor("importer", "planner"));
    ledger(repo).write(&Change::new(envelope, ChangeId::parse(id).unwrap(), "Seeded", "why", ChangeStatus::InProgress)).unwrap();
}

fn seed_evidence(repo: &Path, scenario: &str, at: DateTime<Utc>) {
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, at, actor("impl-agent", "implementer"));
    let record = EvidenceRecord::new(envelope, None, Some(ScenarioId::parse(scenario).unwrap()), None, EvidenceVerdict::Faithful).with_project_id(ProjectId::parse("demo").unwrap());
    ledger(repo).write(&record).unwrap();
}

/// `canon review add` for `scenario`: pinned to the evidence and spec as
/// they are now, so newer evidence makes it stale (0.15 D1).
fn review_add(repo: &Path, scenario: &str) {
    let out = run(
        repo,
        &[
            "review", "add", "--project-id", "demo", "--scenario-id", scenario, "--reviewer", "reviewer-2", "--pin", "pin-sha", "--original-spec-ref", "specs/demo.feature", "--actor-id",
            "reviewer-2", "--role", "reviewer",
        ],
    );
    assert!(out.status.success(), "review add: {}", stderr(&out));
}

/// A blocker on `c-demo`: raised at `raised`, and when `fixed` is given,
/// closed `fixed` at that time (a second record of the same natural key).
fn seed_blocker(repo: &Path, raised: DateTime<Utc>, fixed: Option<DateTime<Utc>>) {
    let envelope = Envelope::new(1, RecordKind::Finding, raised, actor("reviewer-2", "reviewer"));
    let finding = Finding::new(envelope, ChangeId::parse("c-demo").unwrap(), 1, 1, FindingSeverity::Blocker, "reviewer-2", "unbounded subprocess wait");
    ledger(repo).write(&finding).unwrap();
    if let Some(at) = fixed {
        let mut closed = finding.fixed_by(Sha::parse("c".repeat(40)).unwrap());
        closed.envelope.at = at;
        ledger(repo).write(&closed).unwrap();
    }
}

fn write_brief(repo: &Path, change: &str, text: &str) {
    let dir = repo.join(".canon/changes").join(change);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("brief.md"), text).unwrap();
}

fn feature_brief(repo: &Path) {
    write_brief(repo, "c-demo", "## Why\n\nSeeded.\n\nApproved: Journey 2026-10-12 — \"go\"\n");
}

fn chore_brief(approved: DateTime<Utc>) -> String {
    format!(
        "# Bump the client\n\n<!-- canon:kind chore -->\n\n## Intent\n\nBump the client\n\nApproved: Journey {} — \"go\"\n",
        approved.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    )
}

fn subject_json(repo: &Path) -> Value {
    let out = run(repo, &["query", "--kind", "subject", "--json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    serde_json::from_str::<Value>(&stdout(&out)).unwrap()["records"][0].clone()
}

fn status_steps(repo: &Path) -> Vec<Value> {
    let out = run(repo, &["status", "--json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let report: Value = serde_json::from_str(&stdout(&out)).unwrap();
    report["next"].as_array().unwrap().iter().filter(|s| s["why"].as_str().unwrap_or("").starts_with("demo-subject")).cloned().collect()
}

fn ship(repo: &Path) -> Output {
    run(repo, &["subject", "status", "demo-subject", "--to", "shipped"])
}

/// #72: a blocker closed `fixed` keeps the ship refused until the suite
/// is recorded again for every evidenced scenario; the re-review follows.
#[test]
fn a_fixed_blocker_keeps_blocking_until_the_suite_is_evidenced_again_and_reviewed() {
    let dir = repo();
    let r = dir.path();
    seed_change(r, "c-demo");
    seed_subject(r, SubjectStatus::Verifying, &["c-demo"], ago(500));
    seed_scenario(r, "world.demo.01");
    seed_scenario(r, "world.demo.02");
    feature_brief(r);
    for scenario in ["world.demo.01", "world.demo.02"] {
        seed_evidence(r, scenario, ago(300));
        review_add(r, scenario);
    }
    seed_blocker(r, ago(200), Some(ago(100)));

    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("open-blocker c-demo#1.1"), "{err}");
    assert!(err.contains("closed `fixed` but not re-verified") && err.contains("`world.demo.01`") && err.contains("`world.demo.02`"), "{err}");
    assert!(err.contains("hint: ") && err.contains("canon evidence add --project-id") && err.contains("--all-scenarios") && err.contains("re-review follows"), "{err}");
    assert_eq!(subject_json(r)["status"], "verifying");
    let gate = run(r, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "gate check reports the same: {}", stdout(&gate));
    assert!(stdout(&gate).contains("closed `fixed` but not re-verified"), "{}", stdout(&gate));

    // One scenario evidenced again is not enough.
    seed_evidence(r, "world.demo.01", ago(50));
    review_add(r, "world.demo.01");
    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("(`world.demo.02`)"), "only the scenario still without new evidence is named: {}", stderr(&out));

    // The rest, then new evidence staled the old reviews: the re-review is
    // the step that follows.
    seed_evidence(r, "world.demo.02", ago(30));
    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("unreviewed-promotion world.demo.02") && !err.contains("not re-verified"), "{err}");
    review_add(r, "world.demo.02");
    let out = ship(r);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(subject_json(r)["status"], "shipped");
}

/// #72: after a blocker is closed `fixed`, status asks for the batch
/// evidence (never the ship), the same call the refusal names.
#[test]
fn status_names_the_batch_evidence_after_a_blocker_is_closed_fixed() {
    let dir = repo();
    let r = dir.path();
    seed_change(r, "c-demo");
    seed_subject(r, SubjectStatus::Verifying, &["c-demo"], ago(500));
    seed_scenario(r, "world.demo.01");
    feature_brief(r);
    seed_evidence(r, "world.demo.01", ago(300));
    review_add(r, "world.demo.01");
    seed_blocker(r, ago(200), Some(ago(100)));

    let steps = status_steps(r);
    let first = &steps[0];
    let command = first["command"].as_str().unwrap();
    assert!(command.starts_with("canon evidence add --project-id demo --all-scenarios"), "{first}");
    assert!(first["why"].as_str().unwrap().contains("blocker finding c-demo#1.1 was closed fixed"), "{first}");
    assert!(steps.iter().all(|s| !s["command"].as_str().unwrap().contains("--to shipped")), "never ship: {steps:?}");
}

/// #77: an approved chore on a shipped subject asks for the suite again:
/// the gate and status say so, the reopen is followed by the batch
/// evidence, and shipping needs the new evidence and its re-review.
#[test]
fn an_approved_chore_on_a_shipped_subject_needs_the_suite_again_then_a_re_review() {
    let dir = repo();
    let r = dir.path();
    seed_change(r, "c-demo");
    seed_change(r, "bump-client");
    seed_subject(r, SubjectStatus::Shipped, &["c-demo", "bump-client"], ago(250));
    seed_scenario(r, "world.demo.01");
    feature_brief(r);
    write_brief(r, "bump-client", &chore_brief(ago(100)));
    seed_evidence(r, "world.demo.01", ago(300));
    review_add(r, "world.demo.01");

    let gate = run(r, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}", stdout(&gate));
    assert!(stdout(&gate).contains("chore change `bump-client` approved") && stdout(&gate).contains("--all-scenarios"), "{}", stdout(&gate));
    let steps = status_steps(r);
    assert!(steps[0]["command"].as_str().unwrap().starts_with("canon subject status demo-subject --to verifying --reason"), "red shipped subject: reopen first: {steps:?}");

    let out = run(r, &["subject", "status", "demo-subject", "--to", "verifying", "--reason", "chore bump-client", "--json"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let reopened: Value = serde_json::from_str(&stdout(&out)).unwrap();
    let next = reopened["meta"]["next"].as_array().unwrap();
    assert!(next[0]["command"].as_str().unwrap().starts_with("canon evidence add --project-id demo --all-scenarios"), "after the reopen: {next:?}");
    assert!(next.iter().all(|s| !s["command"].as_str().unwrap().contains("--to shipped")), "not ship: {next:?}");

    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("chore change `bump-client` approved") && err.contains("`world.demo.01`"), "{err}");
    assert!(err.contains("hint: ") && err.contains("--all-scenarios") && err.contains("re-review follows"), "{err}");
    assert_eq!(subject_json(r)["status"], "verifying");

    seed_evidence(r, "world.demo.01", ago(10));
    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "the new evidence staled the review: {}", stderr(&out));
    assert!(stderr(&out).contains("unreviewed-promotion world.demo.01") && !stderr(&out).contains("chore change"), "{}", stderr(&out));
    let next = status_steps(r);
    assert!(next[0]["command"].as_str().unwrap().starts_with("canon review add"), "the re-review follows: {next:?}");

    review_add(r, "world.demo.01");
    let out = ship(r);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(subject_json(r)["status"], "shipped");
    let gate = run(r, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(0), "{}", stdout(&gate));
}

/// #77: a chore whose approval is older than the evidence asks for
/// nothing, and a feature change's approval never does.
#[test]
fn evidence_recorded_after_the_chore_approval_satisfies_it_and_a_feature_approval_never_asks() {
    let dir = repo();
    let r = dir.path();
    seed_change(r, "c-demo");
    seed_change(r, "bump-client");
    seed_subject(r, SubjectStatus::Verifying, &["c-demo", "bump-client"], ago(250));
    seed_scenario(r, "world.demo.01");
    write_brief(r, "c-demo", "## Why\n\nSeeded.\n\nApproved: Journey 2026-10-12 — \"go\"\n");
    write_brief(r, "bump-client", &chore_brief(ago(100)));
    seed_evidence(r, "world.demo.01", ago(50));
    review_add(r, "world.demo.01");
    assert!(ship(r).status.success(), "stderr: {}", "evidence newer than the chore approval ships");

    let dir = repo();
    let r = dir.path();
    seed_change(r, "c-demo");
    seed_subject(r, SubjectStatus::Verifying, &["c-demo"], ago(250));
    seed_scenario(r, "world.demo.01");
    feature_brief(r);
    seed_evidence(r, "world.demo.01", ago(300));
    review_add(r, "world.demo.01");
    assert!(ship(r).status.success(), "a feature approval after the evidence asks for no re-run");
}

/// #77: `change new` on a shipped subject names the reopen, in its output
/// and in `meta.next`.
#[test]
fn change_new_on_a_shipped_subject_names_the_reopen() {
    let dir = repo();
    let r = dir.path();
    seed_subject(r, SubjectStatus::Shipped, &[], ago(250));
    let reopen = "canon subject status demo-subject --to verifying --reason";

    let out = run(r, &["change", "new", "bump-client", "--subject", "demo-subject", "--kind", "chore", "--title", "Bump the client"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("subject `demo-subject` is shipped and stays so until reopened") && stdout(&out).contains(reopen), "{}", stdout(&out));

    let out = run(r, &["change", "new", "bump-again", "--subject", "demo-subject", "--kind", "chore", "--title", "Bump again", "--json"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let value: Value = serde_json::from_str(&stdout(&out)).unwrap();
    let next = value["meta"]["next"].as_array().unwrap();
    assert!(next.iter().any(|s| s["command"].as_str().unwrap().starts_with(reopen)), "{next:?}");

    // A subject that is not shipped gets no reopen step.
    let dir = repo();
    seed_subject(dir.path(), SubjectStatus::Building, &[], ago(250));
    let out = run(dir.path(), &["change", "new", "bump-client", "--subject", "demo-subject", "--kind", "chore", "--title", "Bump the client", "--json"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(!stdout(&out).contains("--to verifying"), "{}", stdout(&out));
}
