//! Re-verification before shipping (0.22 D2, #72, #77; 0.23 D5), against the
//! built binary: a blocker closed `fixed` and an approved chore each need
//! every evidenced scenario evidenced again before the subject ships. The
//! reviews stay current while verdicts and checks are unchanged, so a
//! chore asks instead for one change-level review of the chore itself. Records carry explicit times so the order is the test's, not the
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
  finding: local
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
/// `canon review add --change-id`: the chore's change-level review.
fn change_review(repo: &Path, change: &str) -> Output {
    run(
        repo,
        &[
            "review", "add", "--project-id", "demo", "--change-id", change, "--reviewer", "reviewer-3", "--pin", "pin-sha", "--original-spec-ref", ".canon/changes/brief.md", "--actor-id",
            "reviewer-3", "--role", "reviewer",
        ],
    )
}

fn seed_blocker(repo: &Path, raised: DateTime<Utc>, fixed: Option<DateTime<Utc>>) {
    seed_scoped_blocker(repo, raised, fixed, &[]);
}

/// [`seed_blocker`] naming the scenarios the finding is about.
fn seed_scoped_blocker(repo: &Path, raised: DateTime<Utc>, fixed: Option<DateTime<Utc>>, scenarios: &[&str]) {
    let envelope = Envelope::new(1, RecordKind::Finding, raised, actor("reviewer-2", "reviewer"));
    let finding = Finding::new(envelope, ChangeId::parse("c-demo").unwrap(), 1, 1, FindingSeverity::Blocker, "reviewer-2", "unbounded subprocess wait")
        .with_scenario_ids(scenarios.iter().map(|id| ScenarioId::parse(*id).unwrap()));
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
/// is recorded again after the close for every evidenced scenario; the
/// reviews stay current when the verdicts are the same.
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
    assert!(err.contains("hint: ") && err.contains("canon evidence add --project-id") && err.contains("--all-scenarios") && err.contains("stays current"), "{err}");
    assert_eq!(subject_json(r)["status"], "verifying");
    let gate = run(r, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "gate check reports the same: {}", stdout(&gate));
    assert!(stdout(&gate).contains("closed `fixed` but not re-verified"), "{}", stdout(&gate));

    // Evidence recorded after the raise but before the close predates the
    // fix and cannot verify it.
    seed_evidence(r, "world.demo.01", ago(150));
    seed_evidence(r, "world.demo.02", ago(150));
    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("`world.demo.01`") && stderr(&out).contains("`world.demo.02`") && stderr(&out).contains("closed `fixed` but not re-verified"), "{}", stderr(&out));

    // One scenario evidenced after the close is not enough.
    seed_evidence(r, "world.demo.01", ago(50));
    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("(`world.demo.02`)"), "only the scenario still without new evidence is named: {}", stderr(&out));

    // The rest: the new evidence says the same as the evidence the reviews
    // saw, so the reviews stay current and the subject ships.
    seed_evidence(r, "world.demo.02", ago(30));
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

/// #77, 0.23 D5: an approved chore on a shipped subject asks for the suite
/// again, then for one change-level review of the chore. Recording the
/// suite again leaves every scenario review current.
#[test]
fn an_approved_chore_on_a_shipped_subject_needs_the_suite_again_then_a_change_review() {
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
    assert!(err.contains("hint: ") && err.contains("--all-scenarios") && err.contains("canon review add --change-id"), "{err}");
    assert_eq!(subject_json(r)["status"], "verifying");

    // The suite again, same verdict: the scenario review stays current, and
    // what the chore still needs is its own review (that review predates
    // this evidence, so it did not look at the chore's result).
    seed_evidence(r, "world.demo.01", ago(0));
    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "the chore has no review yet: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("no review of change `bump-client`") && !err.contains("unreviewed-promotion world.demo.01"), "{err}");
    let next = status_steps(r);
    assert!(
        next[0]["command"].as_str().unwrap().starts_with("canon review add") && next[0]["command"].as_str().unwrap().contains("--change-id bump-client"),
        "the change review follows: {next:?}"
    );

    let out = change_review(r, "bump-client");
    assert!(out.status.success(), "change review: {}", stderr(&out));
    let out = ship(r);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(subject_json(r)["status"], "shipped");
    let gate = run(r, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(0), "{}", stdout(&gate));
}

/// A chore reviewed scenario by scenario after its evidence, as 0.22
/// required, needs no change-level review: a repo upgraded with such a chore
/// shipped stays green.
#[test]
fn a_chore_reviewed_scenario_by_scenario_after_its_evidence_needs_no_change_review() {
    let dir = repo();
    let r = dir.path();
    seed_change(r, "c-demo");
    seed_change(r, "bump-client");
    seed_subject(r, SubjectStatus::Verifying, &["c-demo", "bump-client"], ago(250));
    seed_scenario(r, "world.demo.01");
    feature_brief(r);
    write_brief(r, "bump-client", &chore_brief(ago(100)));
    seed_evidence(r, "world.demo.01", ago(50));
    review_add(r, "world.demo.01");

    let out = ship(r);
    assert!(out.status.success(), "reviewed after the chore's evidence: {}", stderr(&out));
    let gate = run(r, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(0), "{}", stdout(&gate));
}

/// 0.23 D5: the chore's review pins its brief. Editing the brief stales it,
/// an unsigned outcome never counts, and `--change-id` refuses a feature change.
#[test]
fn a_chore_review_goes_stale_when_the_brief_changes_and_a_feature_change_is_refused() {
    let dir = repo();
    let r = dir.path();
    seed_change(r, "c-demo");
    seed_change(r, "bump-client");
    seed_subject(r, SubjectStatus::Verifying, &["c-demo", "bump-client"], ago(250));
    seed_scenario(r, "world.demo.01");
    feature_brief(r);
    write_brief(r, "bump-client", &chore_brief(ago(100)));
    seed_evidence(r, "world.demo.01", ago(150));
    review_add(r, "world.demo.01");
    // The chore's run, after the scenario review: no reviewer has looked at
    // it per scenario, so the chore's own review is what covers it.
    seed_evidence(r, "world.demo.01", ago(0));

    let refused = change_review(r, "c-demo");
    assert_eq!(refused.status.code(), Some(2), "{}", stderr(&refused));
    assert!(stderr(&refused).contains("is not a chore"), "{}", stderr(&refused));

    assert!(change_review(r, "bump-client").status.success());
    write_brief(r, "bump-client", &format!("{}\nAlso bump the lockfile.\n", chore_brief(ago(100))));
    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("stale review of change `bump-client`") && stderr(&out).contains("brief was edited"), "{}", stderr(&out));

    assert!(change_review(r, "bump-client").status.success(), "reviewing the edited brief again");
    let out = ship(r);
    assert!(out.status.success(), "{}", stderr(&out));
}

/// #77: a chore whose approval is older than the evidence asks for no new
/// evidence (only its change-level review), and a feature change's approval
/// never asks for anything.
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
    seed_evidence(r, "world.demo.01", ago(150));
    review_add(r, "world.demo.01");
    // The chore's run, after the scenario review: no reviewer has looked at
    // it per scenario, so the chore's own review is what covers it.
    seed_evidence(r, "world.demo.01", ago(0));
    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "the chore still needs its review: {}", stderr(&out));
    assert!(!stderr(&out).contains("no evidence recorded after"), "evidence newer than the chore approval is enough: {}", stderr(&out));
    assert!(change_review(r, "bump-client").status.success());
    assert!(ship(r).status.success(), "stderr: {}", stderr(&ship(r)));

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

    let out = run(r, &["change", "new", "bump-client", "--subject-id", "demo-subject", "--kind", "chore", "--title", "Bump the client"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("subject `demo-subject` is shipped and stays so until reopened") && stdout(&out).contains(reopen), "{}", stdout(&out));

    let out = run(r, &["change", "new", "bump-again", "--subject-id", "demo-subject", "--kind", "chore", "--title", "Bump again", "--json"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let value: Value = serde_json::from_str(&stdout(&out)).unwrap();
    let next = value["meta"]["next"].as_array().unwrap();
    assert!(next.iter().any(|s| s["command"].as_str().unwrap().starts_with(reopen)), "{next:?}");

    // A subject that is not shipped gets no reopen step.
    let dir = repo();
    seed_subject(dir.path(), SubjectStatus::Building, &[], ago(250));
    let out = run(dir.path(), &["change", "new", "bump-client", "--subject-id", "demo-subject", "--kind", "chore", "--title", "Bump the client", "--json"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(!stdout(&out).contains("--to verifying"), "{}", stdout(&out));
}

/// A subject with two evidenced, reviewed scenarios (A = `world.demo.01`,
/// B = `world.demo.02`), both evidenced 300s ago.
fn two_scenarios(r: &Path) {
    seed_change(r, "c-demo");
    seed_subject(r, SubjectStatus::Verifying, &["c-demo"], ago(500));
    seed_scenario(r, "world.demo.01");
    seed_scenario(r, "world.demo.02");
    feature_brief(r);
    for scenario in ["world.demo.01", "world.demo.02"] {
        seed_evidence(r, scenario, ago(300));
        review_add(r, scenario);
    }
}

/// #87: a blocker naming scenario A is re-verified by fresh evidence for A
/// alone: B's evidence stays older than the close and shipping and `gate
/// check` both clear. Until then the refusal names A only, and its hint is
/// the per-scenario command, not the whole-suite batch.
#[test]
fn gate_check_97_a_fixed_blocker_naming_a_scenario_is_reverified_by_that_scenarios_evidence_alone() {
    let dir = repo();
    let r = dir.path();
    two_scenarios(r);
    seed_scoped_blocker(r, ago(200), Some(ago(100)), &["world.demo.01"]);

    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("closed `fixed` but not re-verified") && err.contains("1 evidenced scenario(s) (`world.demo.01`)"), "{err}");
    assert!(!err.contains("world.demo.02"), "B is not named: {err}");
    assert!(err.contains("--scenario-id world.demo.01") && !err.contains("--all-scenarios"), "the remedy is the per-scenario command: {err}");
    let gate = run(r, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}", stdout(&gate));
    assert!(stdout(&gate).contains("(`world.demo.01`)") && !stdout(&gate).contains("world.demo.02"), "gate check names A only: {}", stdout(&gate));

    let steps = status_steps(r);
    let command = steps[0]["command"].as_str().unwrap();
    assert!(command.starts_with("canon evidence add --project-id demo --scenario-id world.demo.01 "), "{}", steps[0]);
    assert!(!command.contains("--all-scenarios"), "{command}");

    // Evidence for A recorded before the close does not count.
    seed_evidence(r, "world.demo.01", ago(150));
    assert_eq!(ship(r).status.code(), Some(1));

    // Evidence for A after the close is enough; B's is still 300s old.
    seed_evidence(r, "world.demo.01", ago(50));
    let out = ship(r);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(subject_json(r)["status"], "shipped");
    let gate = run(r, &["gate", "check"]);
    assert!(!stdout(&gate).contains("not re-verified"), "{}", stdout(&gate));
}

/// #87: fresh evidence for B does not re-verify a finding about A.
#[test]
fn gate_check_97_evidence_for_another_scenario_does_not_reverify_a_scoped_finding() {
    let dir = repo();
    let r = dir.path();
    two_scenarios(r);
    seed_scoped_blocker(r, ago(200), Some(ago(100)), &["world.demo.01"]);
    seed_evidence(r, "world.demo.02", ago(50));

    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("(`world.demo.01`)") && !stderr(&out).contains("world.demo.02"), "{}", stderr(&out));
}

/// #87: a finding that names no scenario keeps the 0.22 rule: every
/// evidenced scenario needs evidence after the close.
#[test]
fn gate_check_98_a_fixed_blocker_naming_no_scenario_still_asks_every_evidenced_scenario() {
    let dir = repo();
    let r = dir.path();
    two_scenarios(r);
    seed_scoped_blocker(r, ago(200), Some(ago(100)), &[]);
    seed_evidence(r, "world.demo.01", ago(50));

    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("(`world.demo.02`)") && stderr(&out).contains("--all-scenarios"), "{}", stderr(&out));
    seed_evidence(r, "world.demo.02", ago(40));
    assert!(ship(r).status.success());
}

fn git(r: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(r)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_AUTHOR_NAME", "a")
        .env("GIT_AUTHOR_EMAIL", "a@a")
        .env("GIT_COMMITTER_NAME", "a")
        .env("GIT_COMMITTER_EMAIL", "a@a")
        .output()
        .expect("git on PATH");
    assert!(out.status.success(), "git {args:?}: {}", stderr(&out));
    stdout(&out).trim().to_string()
}

/// #87 through the CLI: `finding add --scenario-id` names the scenario, the
/// close inherits it (it takes no list of its own), and the ship refusal
/// then names that scenario only.
#[test]
fn finding_close_81_inherits_the_scenarios_the_finding_names() {
    let dir = repo();
    let r = dir.path();
    two_scenarios(r);
    git(r, &["init", "-q"]);
    git(r, &["add", "canon.yaml"]);
    git(r, &["commit", "-q", "-m", "initial"]);
    let head = git(r, &["rev-parse", "HEAD"]);

    let add = run(
        r,
        &[
            "finding", "add", "--change-id", "c-demo", "--round", "1", "--seq", "1", "--severity", "blocker", "--reviewer", "reviewer-2", "--summary", "double click", "--scenario-id", "world.demo.01",
            "--scenario-id", "world.demo.01", "--promote",
        ],
    );
    assert!(add.status.success(), "stderr: {}", stderr(&add));
    let close = run(r, &["finding", "close", "--change-id", "c-demo", "--round", "1", "--seq", "1", "--disposition", "fixed", "--resolution-sha", &head, "--promote"]);
    assert!(close.status.success(), "stderr: {}", stderr(&close));

    let listed = run(r, &["query", "--kind", "finding", "--json"]);
    let records: Value = serde_json::from_str(&stdout(&listed)).unwrap();
    let fixed: Vec<&Value> = records["records"].as_array().unwrap().iter().filter(|f| f["disposition"] == "fixed").collect();
    assert_eq!(fixed.len(), 1, "{records}");
    assert_eq!(fixed[0]["scenario_ids"], serde_json::json!(["world.demo.01"]), "named once, inherited by the close: {}", fixed[0]);

    // The close is newer than every evidence record seeded above.
    let out = ship(r);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("(`world.demo.01`)") && !stderr(&out).contains("world.demo.02"), "{}", stderr(&out));
    seed_evidence(r, "world.demo.01", Utc::now() + Duration::seconds(5));
    let out = ship(r);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

/// #87: a `--scenario-id` no scenario record carries is refused with its
/// code, the flag and a hint, and nothing is staged.
#[test]
fn finding_add_33_refuses_a_scenario_id_the_index_does_not_know() {
    let dir = repo();
    let r = dir.path();
    two_scenarios(r);
    let out = run(
        r,
        &["finding", "add", "--change-id", "c-demo", "--round", "1", "--seq", "1", "--severity", "blocker", "--reviewer", "r", "--summary", "s", "--scenario-id", "world.demo.99", "--json"],
    );
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    let body: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(body["error"]["code"], "FINDING_UNKNOWN_SCENARIO", "{body}");
    assert_eq!(body["error"]["field"], "--scenario-id", "{body}");
    assert!(body["error"]["message"].as_str().unwrap().contains("`world.demo.99`"), "{body}");
    assert!(body["error"]["hint"].is_string(), "{body}");
    assert!(!r.join(".canon/ledger/_staging/kind=finding").exists() || std::fs::read_dir(r.join(".canon/ledger/_staging/kind=finding")).unwrap().next().is_none(), "nothing staged");
}

/// #87: `--scenario-id` is repeatable; the record carries each id once, in
/// the order given, and a finding without the flag carries no field at all.
#[test]
fn finding_add_32_records_the_scenarios_it_names_once_each() {
    let dir = repo();
    let r = dir.path();
    two_scenarios(r);
    let base = ["finding", "add", "--change-id", "c-demo", "--round", "1", "--reviewer", "r", "--summary", "s", "--severity", "should-fix", "--promote"];
    let named = [&base[..], &["--seq", "1", "--scenario-id", "world.demo.02", "--scenario-id", "world.demo.01", "--scenario-id", "world.demo.02"]].concat();
    let plain = [&base[..], &["--seq", "2"]].concat();
    for args in [named, plain] {
        let out = run(r, &args);
        assert!(out.status.success(), "stderr: {}", stderr(&out));
    }
    let listed = run(r, &["query", "--kind", "finding", "--json"]);
    let records: Value = serde_json::from_str(&stdout(&listed)).unwrap();
    let by_seq = |seq: u64| records["records"].as_array().unwrap().iter().find(|f| f["seq"] == seq).unwrap().clone();
    assert_eq!(by_seq(1)["scenario_ids"], serde_json::json!(["world.demo.02", "world.demo.01"]), "{records}");
    assert!(by_seq(2).get("scenario_ids").is_none(), "a finding naming none carries no field: {records}");
}
