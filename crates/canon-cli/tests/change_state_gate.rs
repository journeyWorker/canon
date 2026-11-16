//! What a closed change takes out of the requirements (0.42 `change-drop`, flows F3 and F7), against the built
//! binary: `gate check`, `status` and `subject move` read the same set, and a clone with the full history reads it
//! the same. Records carry explicit times, and commits explicit dates, so the window of a closed change is the
//! test's, not the clock's: the brief is committed 2026-02-01, the change is closed 2026-02-10.
//!
//! The scenarios are named by id in the test names (`change.drop.09` is `change_drop_09_…`).

#[path = "support/hermetic.rs"]
mod hermetic;
use std::path::Path;
use std::process::{Command, Output};

use canon_model::{
    Actor, Change, ChangeDecision, ChangeId, ChangeState, ChangeStatus, Envelope, EvidenceRecord, EvidenceVerdict, Finding, FindingSeverity, ProjectId, RecordKind, RoleId, ScenarioId, Sha, Subject, SubjectId,
    SubjectStatus,
};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use tempfile::TempDir;

mod support;

const CANON_YAML: &str = "\
tiers:
  local: { backend: git, root: .canon/ledger }
routing:
  subject: local
  change: local
  scenario: local
  evidence: local
  finding: local
  review: local
  change_state: local
";

const POLICY: &str = "spec_coverage:\n  require_evidence: true\n  require_review: {}\nstaleness:\n  max_commits_behind: 2\n";

const BRIEF_AT: &str = "2026-02-01T00:00:00Z";
const IN_WINDOW: &str = "2026-02-02T00:00:00Z";
const CLOSED_AT: &str = "2026-02-10T00:00:00Z";
const BEFORE_BRIEF: &str = "2026-01-15T00:00:00Z";
const AFTER_CLOSE: &str = "2026-03-01T00:00:00Z";

fn at(text: &str) -> DateTime<Utc> {
    text.parse().unwrap()
}

fn git(repo: &Path, args: &[&str], date: Option<&str>) -> String {
    let mut command = Command::new("git");
    command.args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]).args(args).current_dir(repo);
    if let Some(date) = date {
        command.env("GIT_AUTHOR_DATE", date).env("GIT_COMMITTER_DATE", date);
    }
    let out = command.output().expect("git must be on PATH");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit(repo: &Path, message: &str, date: &str) {
    git(repo, &["add", "-A"], None);
    git(repo, &["commit", "-q", "--allow-empty", "-m", message], Some(date));
}

fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"], None);
    std::fs::write(dir.path().join("canon.yaml"), CANON_YAML).unwrap();
    std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
    std::fs::write(dir.path().join(".canon/policy.yaml"), POLICY).unwrap();
    dir
}

fn run(repo: &Path, args: &[&str]) -> Output {
    hermetic::canon().args(args).current_dir(repo).env_remove("CANON_ROLE").env_remove("CANON_ACTOR").output().expect("spawn canon binary")
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

fn actor(id: &str, role: &str) -> Actor {
    Actor::new(id, RoleId::parse(role).unwrap())
}

fn seed_subject(repo: &Path, status: SubjectStatus, changes: &[&str]) {
    let envelope = Envelope::new(1, RecordKind::Subject, at("2026-01-01T00:00:00Z"), actor("canon", "implementer"));
    let subject = Subject::new(envelope, SubjectId::parse("demo-subject").unwrap(), "Seeded", "s", "dev", status, RoleId::parse("implementer").unwrap())
        .with_change_ids(changes.iter().map(|c| ChangeId::parse(*c).unwrap()).collect());
    ledger(repo).write(&subject).unwrap();
    for change in changes {
        let envelope = Envelope::new(1, RecordKind::Change, at("2026-01-01T00:00:00Z"), actor("importer", "planner"));
        ledger(repo).write(&Change::new(envelope, ChangeId::parse(*change).unwrap(), "Seeded", "why", ChangeStatus::InProgress)).unwrap();
    }
}

fn write_brief(repo: &Path, change: &str, text: &str) {
    let dir = repo.join(".canon/changes").join(change);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("brief.md"), text).unwrap();
}

fn feature_brief(repo: &Path, change: &str) {
    write_brief(repo, change, "## Why\n\nSeeded.\n\nApproved: Journey 2026-01-20T00:00:00Z — \"go\"\n");
}

fn chore_brief(approved: &str) -> String {
    format!("---\nkind: chore\n---\n\n# Bump the client\n\n## Intent\n\nBump the client\n\nApproved: Journey {approved} — \"go\"\n")
}

/// One scenario `id` (`<area>.<surface>.<nn>`) in project `demo`, owned by `demo-subject`, tagged with `flows`
/// (`<change>.F1` values) and created at `created`.
fn scenario(repo: &Path, id: &str, flows: &[&str], created: &str) {
    let provenance = format!("# canon: {{\"schema\":1,\"at\":\"{created}\",\"actor\":{{\"agent_id\":\"canon\"}}}}");
    write_scenario(repo, id, flows, &provenance);
}

/// The same scenario with no provenance line, so no creation time.
fn scenario_undated(repo: &Path, id: &str, flows: &[&str]) {
    write_scenario(repo, id, flows, "");
}

fn write_scenario(repo: &Path, id: &str, flows: &[&str], provenance: &str) {
    support::declare_spec_root(repo, "demo");
    let mut parts = id.split('.');
    let (area, surface) = (parts.next().unwrap(), parts.next().unwrap());
    let dir = repo.join(format!("specs-demo/kind=feature/area={area}"));
    std::fs::create_dir_all(&dir).unwrap();
    let tags: String = flows.iter().map(|f| format!("  @flow:{f}\n")).collect();
    let text = format!("Feature: {area} {surface}\n  {provenance}\n\n  {provenance}\n  @subject:demo-subject\n{tags}  @{id}\n  Scenario: {id} seeded\n    Given a step\n");
    std::fs::write(dir.join(format!("{surface}.feature")), text).unwrap();
}

fn evidence(repo: &Path, scenario: &str, verdict: EvidenceVerdict, sha: Option<&str>) {
    let envelope = Envelope::new(1, RecordKind::Evidence, at("2026-02-03T00:00:00Z"), actor("impl-agent", "implementer"));
    let mut record = EvidenceRecord::new(envelope, None, Some(ScenarioId::parse(scenario).unwrap()), None, verdict).with_project_id(ProjectId::parse("demo").unwrap());
    if let Some(sha) = sha {
        record = record.with_evidence_sha(Sha::parse(sha).unwrap());
    }
    ledger(repo).write(&record).unwrap();
}

fn review(repo: &Path, scenario: &str) {
    let out = run(repo, &["review", "scenario", scenario, "--project", "demo", "--original-spec-ref", "specs/demo.feature", "--actor", "reviewer-2", "--role", "reviewer"]);
    assert!(out.status.success(), "review add: {}", stderr(&out));
}

fn blocker(repo: &Path, change: &str) {
    let envelope = Envelope::new(1, RecordKind::Finding, at("2026-02-04T00:00:00Z"), actor("reviewer-2", "reviewer"));
    ledger(repo).write(&Finding::new(envelope, ChangeId::parse(change).unwrap(), 1, 1, FindingSeverity::Blocker, "reviewer-2", "unbounded subprocess wait")).unwrap();
}

/// A `change_state` record as `canon change defer|drop|reopen` writes it, at an explicit time.
fn seed_state(repo: &Path, change: &str, state: ChangeDecision, when: &str) {
    let envelope = Envelope::current(RecordKind::ChangeState, at(when), actor("someone", "planner"));
    ledger(repo).write(&ChangeState::new(envelope, ChangeId::parse(change).unwrap(), state, "measured: the job is slower", vec!["https://example.com/pr/147".to_string()], "journey")).unwrap();
}

fn json_of(o: &Output) -> Value {
    serde_json::from_str(&stdout(o)).unwrap_or_else(|e| panic!("not JSON ({e}): {}{}", stdout(o), stderr(o)))
}

/// `(exit, violations as "class subject", advisories)` of `canon gate check --json`.
fn gate(repo: &Path) -> (Option<i32>, Vec<String>, Vec<Value>) {
    let out = run(repo, &["gate", "check", "--json"]);
    let body = json_of(&out);
    let violations = body["data"]["violations"].as_array().into_iter().flatten().map(|v| format!("{} {}", v["class"].as_str().unwrap_or_default(), v["subject"].as_str().unwrap_or_default())).collect();
    let advisories = body["data"]["advisories"].as_array().cloned().unwrap_or_default();
    (out.status.code(), violations, advisories)
}

fn closed_lines(advisories: &[Value]) -> Vec<String> {
    advisories.iter().filter(|a| a["kind"] == "closed_change").flat_map(|a| a["items"].as_array().cloned().unwrap_or_default()).map(|v| v.as_str().unwrap().to_string()).collect()
}

fn status_subject(repo: &Path) -> Value {
    let body = json_of(&run(repo, &["status", "--json"]));
    body["data"]["subjects"].as_array().unwrap().iter().find(|s| s["id"] == "demo-subject").unwrap_or_else(|| panic!("no demo-subject: {body}")).clone()
}

fn set_aside_ids(subject: &Value) -> Vec<String> {
    subject["setAsideScenarios"].as_array().into_iter().flatten().map(|s| s["scenario"].as_str().unwrap().to_string()).collect()
}

/// A verifying subject that adopted `c-live` (one evidenced, reviewed scenario) and `c-gone`, whose brief was committed at [`BRIEF_AT`].
fn two_changes() -> TempDir {
    let dir = repo();
    let r = dir.path();
    seed_subject(r, SubjectStatus::Verifying, &["c-live", "c-gone"]);
    feature_brief(r, "c-live");
    feature_brief(r, "c-gone");
    commit(r, "briefs", BRIEF_AT);
    scenario(r, "world.live.01", &["c-live.F1"], IN_WINDOW);
    evidence(r, "world.live.01", EvidenceVerdict::Faithful, None);
    review(r, "world.live.01");
    dir
}

/// change.drop.09: a shipped subject's approved chore without a change review turns `gate check` red; a deferral
/// makes it green, and a reopen makes it red again. The subject's next step no longer names the chore while it is deferred.
#[test]
fn change_drop_09_a_closed_change_stops_turning_gate_check_red_and_stops_being_asked_for() {
    let dir = repo();
    let r = dir.path();
    seed_subject(r, SubjectStatus::Shipped, &["c-live", "bump-client"]);
    feature_brief(r, "c-live");
    write_brief(r, "bump-client", &chore_brief("2026-02-05T00:00:00Z"));
    commit(r, "briefs", BRIEF_AT);
    scenario(r, "world.live.01", &["c-live.F1"], IN_WINDOW);
    evidence(r, "world.live.01", EvidenceVerdict::Faithful, None);
    review(r, "world.live.01");

    let (code, violations, _) = gate(r);
    assert_eq!(code, Some(1), "the approved chore asks for evidence again and a change review: {violations:?}");

    seed_state(r, "bump-client", ChangeDecision::Deferred, CLOSED_AT);
    let (code, violations, advisories) = gate(r);
    assert_eq!(code, Some(0), "a deferred chore asks for nothing: {violations:?}");
    assert!(closed_lines(&advisories).iter().any(|l| l.contains("`bump-client` is deferred")), "{advisories:?}");
    let subject = status_subject(r);
    assert_eq!(subject["setAsideChanges"][0]["change"], "bump-client", "{subject}");
    assert_eq!(subject["setAsideChanges"][0]["state"], "deferred");
    assert!(subject["red"].as_array().is_none_or(Vec::is_empty), "the shipped subject is not red: {subject}");
    let text = stdout(&run(r, &["status"]));
    assert!(text.contains("`bump-client` is deferred by journey") && text.contains("canon change reopen bump-client"), "{text}");
    assert!(!text.contains("canon review change bump-client"), "no step asks for the deferred chore: {text}");

    seed_state(r, "bump-client", ChangeDecision::Reopened, "2026-02-12T00:00:00Z");
    let (_, _, advisories) = gate(r);
    assert!(closed_lines(&advisories).is_empty(), "{advisories:?}");
    // The earlier approval predates the reopen, so it is not an approval: the human approves again, then the chore is asked for.
    write_brief(r, "bump-client", &chore_brief("2026-02-13T00:00:00Z"));
    let (code, violations, _) = gate(r);
    assert_eq!(code, Some(1), "a reopened and approved-again change is asked for again: {violations:?}");

    // The ship guard asked for the chore while it was approved and unreviewed; the deferral takes it out, and the shipped subject reopens.
    let dir = repo();
    let r = dir.path();
    seed_subject(r, SubjectStatus::Verifying, &["c-live", "bump-client"]);
    feature_brief(r, "c-live");
    write_brief(r, "bump-client", &chore_brief("2026-02-05T00:00:00Z"));
    commit(r, "briefs", BRIEF_AT);
    scenario(r, "world.live.01", &["c-live.F1"], IN_WINDOW);
    evidence(r, "world.live.01", EvidenceVerdict::Faithful, None);
    review(r, "world.live.01");
    let refused = run(r, &["subject", "move", "demo-subject", "--to", "shipped"]);
    assert!(!refused.status.success() && stderr(&refused).contains("bump-client"), "the ship guard asks for the chore: {}", stderr(&refused));
    seed_state(r, "bump-client", ChangeDecision::Deferred, CLOSED_AT);
    let shipped = run(r, &["subject", "move", "demo-subject", "--to", "shipped"]);
    assert!(shipped.status.success() && !stderr(&shipped).contains("bump-client"), "the ship guard no longer asks: {}", stderr(&shipped));
    let reopened = run(r, &["subject", "move", "demo-subject", "--to", "verifying", "--reason", "follow-up"]);
    assert!(reopened.status.success() && !stderr(&reopened).contains("bump-client"), "the reopen guard does not ask for it: {}", stderr(&reopened));
}

/// change.drop.10: the scenarios only a closed change made are set aside, `status` and `gate check` say so, and the
/// closed change's open blocker finding stays on record but no longer blocks.
#[test]
fn change_drop_10_the_scenarios_only_a_closed_change_made_are_set_aside_and_its_blocker_does_not_block() {
    let dir = two_changes();
    let r = dir.path();
    scenario(r, "world.gone.01", &["c-gone.F1"], IN_WINDOW);
    blocker(r, "c-gone");

    let (code, violations, _) = gate(r);
    assert_eq!(code, Some(1));
    assert!(violations.iter().any(|v| v.starts_with("uncovered-cell") && v.contains("world.gone.01")), "{violations:?}");
    assert!(violations.iter().any(|v| v.starts_with("open-blocker")), "{violations:?}");

    seed_state(r, "c-gone", ChangeDecision::Dropped, CLOSED_AT);
    let (code, violations, advisories) = gate(r);
    assert_eq!(code, Some(0), "{violations:?}");
    let lines = closed_lines(&advisories);
    assert!(lines.iter().any(|l| l.contains("scenario `world.gone.01`") && l.contains("set aside")), "{lines:?}");
    let subject = status_subject(r);
    assert_eq!(set_aside_ids(&subject), ["world.gone.01"], "{subject}");
    assert_eq!(subject["setAsideChanges"][0]["change"], "c-gone");
    assert_eq!(subject["setAsideChanges"][0]["state"], "dropped");
    assert_eq!(subject["setAsideChanges"][0]["approver"], "journey");
    assert_eq!(subject["setAsideChanges"][0]["refs"][0], "https://example.com/pr/147");
    assert_eq!(subject["scenarios"], 1, "only the live scenario is counted: {subject}");
    assert_eq!(subject["openBlockers"], 0, "{subject}");
    let findings = json_of(&run(r, &["finding", "list", "--change", "c-gone", "--json"]));
    assert_eq!(findings["data"]["findings"].as_array().map(Vec::len).or_else(|| findings["data"]["records"].as_array().map(Vec::len)), Some(1), "the finding stays on record: {findings}");

    let body = json_of(&run(r, &["status", "--json"]));
    assert_eq!(body["data"]["statusVersion"].as_u64().or(body["data"]["status_version"].as_u64()), Some(4), "statusVersion stays 4");
}

/// change.drop.11: a set-aside scenario's divergent verdict and its stale evidence no longer turn `gate check` red.
#[test]
fn change_drop_11_a_set_aside_scenarios_stale_evidence_and_divergent_verdict_do_not_turn_gate_check_red() {
    let dir = two_changes();
    let r = dir.path();
    scenario(r, "world.gone.01", &["c-gone.F1"], IN_WINDOW);
    scenario(r, "world.gone.02", &["c-gone.F1"], IN_WINDOW);
    std::fs::write(r.join("code.rs"), "1").unwrap();
    commit(r, "code 1", "2026-02-02T12:00:00Z");
    let base = git(r, &["rev-parse", "HEAD"], None);
    evidence(r, "world.gone.01", EvidenceVerdict::Divergent, None);
    evidence(r, "world.gone.02", EvidenceVerdict::Faithful, Some(&base));
    for n in 2..=4 {
        std::fs::write(r.join("code.rs"), n.to_string()).unwrap();
        commit(r, &format!("code {n}"), "2026-02-03T12:00:00Z");
    }
    let (_, violations, _) = gate(r);
    assert!(violations.iter().any(|v| v.contains("world.gone.01")), "the divergent verdict turns it red while the change is open: {violations:?}");
    assert!(violations.iter().any(|v| v.starts_with("stale-evidence") && v.contains("world.gone.02")), "the stale evidence turns it red: {violations:?}");

    seed_state(r, "c-gone", ChangeDecision::Deferred, CLOSED_AT);
    let (code, violations, _) = gate(r);
    assert_eq!(code, Some(0), "{violations:?}");
    assert!(violations.iter().all(|v| !v.contains("world.gone")), "{violations:?}");
}

/// change.drop.12 (failure path): what another change requires stays required, a scenario tagged for a closed and
/// an open change stays required, so does an untagged one, and `status`, `gate check` and `subject move` set aside
/// the same scenarios. A subject whose every scenario is set aside is refused at ship like one with none.
#[test]
fn change_drop_12_what_another_change_requires_stays_required_and_the_readers_set_aside_the_same_scenarios() {
    let dir = two_changes();
    let r = dir.path();
    scenario(r, "world.gone.01", &["c-gone.F1"], IN_WINDOW);
    scenario(r, "world.mixed.01", &["c-gone.F1", "c-live.F1"], IN_WINDOW);
    scenario(r, "world.untagged.01", &[], IN_WINDOW);
    scenario(r, "world.other.01", &["c-live.F1"], IN_WINDOW);
    seed_state(r, "c-gone", ChangeDecision::Deferred, CLOSED_AT);

    let (code, violations, advisories) = gate(r);
    assert_eq!(code, Some(1));
    for required in ["world.mixed.01", "world.untagged.01", "world.other.01"] {
        assert!(violations.iter().any(|v| v.contains(required)), "{required} stays required: {violations:?}");
    }
    assert!(violations.iter().all(|v| !v.contains("world.gone.01")), "{violations:?}");

    let subject = status_subject(r);
    assert_eq!(set_aside_ids(&subject), ["world.gone.01"], "{subject}");
    let from_gate: Vec<String> = closed_lines(&advisories).into_iter().filter(|l| l.starts_with("scenario ")).collect();
    assert!(from_gate.len() == 1 && from_gate[0].contains("world.gone.01"), "{from_gate:?}");
    let moved = run(r, &["subject", "move", "demo-subject", "--to", "shipped"]);
    assert_eq!(moved.status.code(), Some(2), "{}", stderr(&moved));
    let err = stderr(&moved);
    assert!(err.contains("world.untagged.01") && err.contains("world.mixed.01") && !err.contains("world.gone.01"), "the ship guard reads the same set: {err}");

    // Every scenario the subject owns was made by the closed change: ship is refused like an empty subject.
    let all = repo();
    let a = all.path();
    seed_subject(a, SubjectStatus::Verifying, &["c-gone"]);
    feature_brief(a, "c-gone");
    commit(a, "brief", BRIEF_AT);
    scenario(a, "world.gone.01", &["c-gone.F1"], IN_WINDOW);
    seed_state(a, "c-gone", ChangeDecision::Dropped, CLOSED_AT);
    let moved = run(a, &["subject", "move", "demo-subject", "--to", "shipped"]);
    assert_eq!(moved.status.code(), Some(2), "{}", stderr(&moved));
    assert!(stderr(&moved).contains("set aside by a closed change"), "{}", stderr(&moved));
}

/// change.drop.13 (failure path): a closed change's tag on a scenario older than the change, or made after it was
/// closed, is ignored: the scenario stays required and `closed_change` lists the tag with its reason.
#[test]
fn change_drop_13_a_tag_on_a_scenario_outside_the_window_is_ignored_and_reported() {
    let dir = two_changes();
    let r = dir.path();
    scenario(r, "world.old.01", &["c-gone.F1"], BEFORE_BRIEF);
    scenario(r, "world.late.01", &["c-gone.F1"], AFTER_CLOSE);
    scenario(r, "world.gone.01", &["c-gone.F1"], IN_WINDOW);
    scenario_undated(r, "world.nodate.01", &["c-gone.F1"]);
    seed_state(r, "c-gone", ChangeDecision::Deferred, CLOSED_AT);

    let (code, violations, advisories) = gate(r);
    assert_eq!(code, Some(1));
    assert!(violations.iter().any(|v| v.contains("world.old.01")) && violations.iter().any(|v| v.contains("world.late.01")) && violations.iter().any(|v| v.contains("world.nodate.01")), "all three stay required: {violations:?}");
    let lines = closed_lines(&advisories);
    for (id, reason) in [("world.old.01", "outside_window"), ("world.late.01", "outside_window"), ("world.nodate.01", "no_creation_time")] {
        assert!(lines.iter().any(|l| l.contains(id) && l.contains(reason) && l.contains("c-gone")), "{id} is listed with {reason}: {lines:?}");
    }
    assert!(lines.iter().all(|l| !(l.contains("world.gone.01") && l.contains("ignored"))), "{lines:?}");
    assert_eq!(set_aside_ids(&status_subject(r)), ["world.gone.01"]);
}

/// change.drop.14: a fresh full clone reads the same set aside, runs `inventory sync` and has a green `gate check`.
#[test]
fn change_drop_14_a_fresh_full_clone_sets_aside_the_same_scenarios() {
    let dir = two_changes();
    let r = dir.path();
    scenario(r, "world.gone.01", &["c-gone.F1"], IN_WINDOW);
    seed_state(r, "c-gone", ChangeDecision::Deferred, CLOSED_AT);
    commit(r, "everything", "2026-02-11T00:00:00Z");
    let original = set_aside_ids(&status_subject(r));
    assert_eq!(original, ["world.gone.01"]);

    let clone = TempDir::new().unwrap();
    git(clone.path(), &["clone", "-q", &format!("file://{}", r.display()), "."], None);
    let out = run(clone.path(), &["inventory", "sync", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));
    let (code, violations, _) = gate(clone.path());
    assert_eq!(code, Some(0), "{violations:?}");
    assert_eq!(set_aside_ids(&status_subject(clone.path())), original, "the same set as the original work tree");
}

/// change.drop.15 (failure path): a shallow clone cannot read when the brief was added, so it sets nothing aside,
/// says `history_unavailable` in `closed_change` and in `status`, and the scenario stays required; the change-level
/// requirement is still gone, because it comes from the `change_state` record.
#[test]
fn change_drop_15_a_shallow_clone_sets_nothing_aside_and_says_the_history_is_unavailable() {
    let dir = two_changes();
    let r = dir.path();
    scenario(r, "world.gone.01", &["c-gone.F1"], IN_WINDOW);
    seed_state(r, "c-gone", ChangeDecision::Deferred, CLOSED_AT);
    commit(r, "everything", "2026-02-11T00:00:00Z");
    commit(r, "one more", "2026-02-12T00:00:00Z");

    let clone = TempDir::new().unwrap();
    git(clone.path(), &["clone", "-q", "--depth", "1", &format!("file://{}", r.display()), "."], None);
    let out = run(clone.path(), &["inventory", "sync", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));
    let (code, violations, advisories) = gate(clone.path());
    assert_eq!(code, Some(1), "the scenario stays required: {violations:?}");
    assert!(violations.iter().any(|v| v.contains("world.gone.01")), "{violations:?}");
    assert!(closed_lines(&advisories).iter().any(|l| l.contains("world.gone.01") && l.contains("history_unavailable")), "{advisories:?}");
    let body = json_of(&run(clone.path(), &["status", "--json"]));
    let warned = body["meta"]["warnings"].as_array().into_iter().flatten().any(|w| w["code"] == "CHANGE_HISTORY_MISSING" && w["message"].as_str().unwrap().contains("history_unavailable"));
    assert!(warned, "status says the history is unavailable: {body}");
    let subject = status_subject(clone.path());
    assert!(set_aside_ids(&subject).is_empty(), "{subject}");
    assert_eq!(subject["setAsideChanges"][0]["change"], "c-gone", "the change-level fact needs no history: {subject}");
}

/// change.drop.22: the first use's shape in a temporary repository, through the real commands: the shipped subject's
/// approved chore turns the gate red, `canon change defer` turns it green (also in a new clone, which reads the
/// history through `canon query --kind change_state --history`), and `canon change reopen` turns it red again.
#[test]
fn change_drop_22_a_deferred_chore_is_green_in_a_new_clone_and_red_again_after_a_reopen() {
    let dir = repo();
    let r = dir.path();
    seed_subject(r, SubjectStatus::Shipped, &["c-live", "bump-client"]);
    feature_brief(r, "c-live");
    write_brief(r, "bump-client", &chore_brief("2026-02-05T00:00:00Z"));
    scenario(r, "world.live.01", &["c-live.F1"], IN_WINDOW);
    commit(r, "briefs", BRIEF_AT);
    evidence(r, "world.live.01", EvidenceVerdict::Faithful, None);
    review(r, "world.live.01");
    assert_eq!(gate(r).0, Some(1), "the approved chore is asked for");

    let out = run(r, &["change", "defer", "bump-client", "--reason", "the offline job is too slow", "--ref", "https://example.com/pr/147", "--approver", "journey", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));
    assert_eq!(json_of(&out)["data"]["state"], "deferred");
    let (code, violations, _) = gate(r);
    assert_eq!(code, Some(0), "{violations:?}");

    commit(r, "everything", "2026-02-11T00:00:00Z");
    let clone = TempDir::new().unwrap();
    git(clone.path(), &["clone", "-q", &format!("file://{}", r.display()), "."], None);
    assert_eq!(run(clone.path(), &["inventory", "sync", "--json"]).status.code(), Some(0));
    assert_eq!(gate(clone.path()).0, Some(0), "a new clone reads the deferral");
    let history = json_of(&run(clone.path(), &["query", "--kind", "change_state", "--history", "--json"]));
    assert!(history.to_string().contains("the offline job is too slow"), "{history}");

    let out = run(r, &["change", "reopen", "bump-client", "--reason", "owner picked the scope option", "--approver", "journey", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));
    write_brief(r, "bump-client", &chore_brief(&(Utc::now() + Duration::hours(1)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)));
    assert_eq!(gate(r).0, Some(1), "reopened and approved again: the chore is asked for again");
}

/// change.drop.23 (failure path): a tree without the change refuses `CHANGE_NOT_FOUND` and writes nothing; an agent
/// without the human's answer is refused `ACTOR_DENIED`.
#[test]
fn change_drop_23_a_tree_without_the_change_and_an_agent_without_the_humans_answer_are_refused() {
    let dir = repo();
    let r = dir.path();
    seed_subject(r, SubjectStatus::Shipped, &["bump-client"]);
    write_brief(r, "bump-client", &chore_brief("2026-02-05T00:00:00Z"));
    let args = ["change", "defer", "canon-0-41-ci-nextest", "--reason", "slow", "--ref", "x", "--approver", "journey", "--json"];
    let out = run(r, &args);
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert_eq!(json_of(&out)["error"]["code"], "CHANGE_NOT_FOUND");
    assert!(!r.join(".canon/ledger/kind=change_state").exists(), "nothing was written");

    let out = hermetic::canon().args(["change", "defer", "bump-client", "--reason", "slow", "--ref", "x", "--approver", "journey", "--json"]).current_dir(r).env("CANON_ROLE", "implementer").env("CANON_ACTOR", "impl-1").output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert_eq!(json_of(&out)["error"]["code"], "ACTOR_DENIED");
    assert!(!r.join(".canon/ledger/kind=change_state").exists(), "nothing was written");
    let _ = Duration::seconds(0);
}
