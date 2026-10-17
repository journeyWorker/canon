//! `canon divergence accept` (0.22 D1, issue #78) through the real binary:
//! a reviewer's tracked acceptance of a divergent scenario lets its subject
//! ship, stays listed by `status` and `gate check`, and lapses on new
//! evidence or at its expiry. Records are seeded straight into the ledger,
//! as `tests/subject.rs` does, so the evidence's actor and session are the
//! test's to choose.

use std::path::Path;
use std::process::{Command, Output};

use canon_model::{Actor, Envelope, EvidenceRecord, EvidenceVerdict, ProjectId, RecordKind, RoleId, Scenario, ScenarioId, SessionId, SpecDigest, Subject, SubjectId, SubjectStatus};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::Utc;
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
  divergence: local
";

const SCENARIO: &str = "world.demo.01";
const TRACKING: &str = "https://example.com/engine/issues/118";

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).output().expect("spawn canon binary")
}

fn out(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn ledger(repo: &Path) -> GitTier {
    GitTier::new(repo.join(".canon/ledger"))
}

fn role(name: &str) -> RoleId {
    RoleId::parse(name).unwrap()
}

/// A `verifying` subject owning `world.demo.01`, whose latest evidence is a
/// divergent verdict attested by `impl-agent` in session `impl-session`.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), CANON_YAML).unwrap();
    let repo = dir.path();
    let subject = Subject::new(
        Envelope::new(1, RecordKind::Subject, Utc::now(), Actor::new("canon", role("implementer"))),
        SubjectId::parse("demo-subject").unwrap(),
        "Seeded",
        "s",
        "dev",
        SubjectStatus::Verifying,
        role("implementer"),
    );
    ledger(repo).write(&subject).unwrap();
    let mut scenario = Scenario::new(
        Envelope::new(1, RecordKind::Scenario, Utc::now(), Actor::new("canon", role("implementer"))),
        ProjectId::parse("demo").unwrap(),
        ScenarioId::parse(SCENARIO).unwrap(),
        "seeded",
        "",
        SpecDigest::of(SCENARIO.as_bytes()),
    );
    scenario.subject_id = Some(SubjectId::parse("demo-subject").unwrap());
    ledger(repo).write(&scenario).unwrap();
    attest(repo, EvidenceVerdict::Divergent);
    dir
}

fn attest(repo: &Path, verdict: EvidenceVerdict) {
    let actor = Actor::new("impl-agent", role("implementer")).with_session(SessionId::parse("impl-session").unwrap());
    let record = EvidenceRecord::new(Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), actor), None, Some(ScenarioId::parse(SCENARIO).unwrap()), None, verdict)
        .with_project_id(ProjectId::parse("demo").unwrap());
    ledger(repo).write(&record).unwrap();
}

/// `canon divergence accept` with the defaults below; each `(flag, value)` in
/// `overrides` replaces that flag's default (`--scenario-id` adds another).
fn accept(repo: &Path, overrides: &[(&str, &str)]) -> Output {
    let mut flags: Vec<(&str, &str)> = vec![
        ("--project-id", "demo"),
        ("--scenario-id", SCENARIO),
        ("--reason", "the engine drops the event"),
        ("--tracking", TRACKING),
        ("--reviewer", "reviewer-2"),
        ("--role", "reviewer"),
        ("--actor-id", "reviewer-2"),
        ("--session-id", "review-session"),
    ];
    for (flag, value) in overrides {
        match flags.iter_mut().find(|(f, _)| f == flag).filter(|_| *flag != "--scenario-id") {
            Some(slot) => slot.1 = value,
            None => flags.push((flag, value)),
        }
    }
    let mut args = vec!["divergence", "accept"];
    for (flag, value) in &flags {
        args.extend([*flag, *value]);
    }
    canon(repo, &args)
}

fn ship(repo: &Path) -> Output {
    canon(repo, &["subject", "status", "demo-subject", "--to", "shipped"])
}

fn accepted_line(expires: &str) -> String {
    format!("accepted-divergence {SCENARIO} — {TRACKING} ({expires})")
}

/// A divergent verdict blocks shipping, and the refusal names both ways out.
#[test]
fn a_divergent_verdict_blocks_shipping_and_the_hint_names_both_ways_out() {
    let dir = repo();
    let json = canon(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped", "--json"]);
    assert_eq!(json.status.code(), Some(1), "{}", out(&json));
    let body: Value = serde_json::from_slice(&json.stdout).expect("JSON error on stdout");
    assert_eq!(body["error"]["code"], "SUBJECT_GATE_VIOLATION");
    assert!(body["error"]["message"].as_str().unwrap().contains("uncovered-cell world.demo.01 — verifying → shipped: latest verdict is divergent (by impl-agent)"), "{body}");
    let hint = body["error"]["hint"].as_str().expect("the refusal carries a hint");
    assert!(hint.contains("re-attest") && hint.contains("canon divergence accept --project-id demo --scenario-id world.demo.01"), "{hint}");
}

/// A reviewer's acceptance lets the subject ship; `status` and `gate check`
/// still list the gap with its tracking reference and expiry.
#[test]
fn an_accepted_divergence_ships_and_stays_listed_in_status_and_gate_check() {
    let dir = repo();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_evidence: true\n").unwrap();

    let before = canon(repo, &["gate", "check"]);
    assert_eq!(before.status.code(), Some(1), "{}", out(&before));
    assert!(out(&before).contains("uncovered-cell world.demo.01 — latest verdict is divergent (by impl-agent)"), "{}", out(&before));
    assert!(out(&before).contains("canon divergence accept"), "the gate line names the way out: {}", out(&before));

    let accepted = accept(repo, &[("--expiry", "2999-01-01")]);
    assert!(accepted.status.success(), "{}", out(&accepted));

    let gate = canon(repo, &["gate", "check"]);
    assert!(!out(&gate).contains("uncovered-cell world.demo.01"), "an accepted gap is covered: {}", out(&gate));
    assert!(out(&gate).contains(&accepted_line("expires 2999-01-01")), "gate check lists it: {}", out(&gate));

    let status = canon(repo, &["status"]);
    assert!(out(&status).contains(&accepted_line("expires 2999-01-01")), "status lists it: {}", out(&status));

    let shipped = ship(repo);
    assert!(shipped.status.success(), "{}", out(&shipped));
}

/// An acceptance without `--expiry` holds until new evidence and says so.
#[test]
fn an_acceptance_without_an_expiry_says_so() {
    let dir = repo();
    assert!(accept(dir.path(), &[]).status.success());
    assert!(out(&canon(dir.path(), &["gate", "check"])).contains(&accepted_line("no expiry")));
    assert!(ship(dir.path()).status.success());
}

/// The accepting reviewer must be independent of the evidence: its actor,
/// the `--reviewer` it names, and its session each refuse when they attested it.
#[test]
fn acceptance_from_the_evidence_actor_or_session_is_refused() {
    let dir = repo();
    let repo = dir.path();
    let same_session = accept(repo, &[("--session-id", "impl-session")]);
    assert_eq!(same_session.status.code(), Some(2), "{}", out(&same_session));
    assert!(out(&same_session).contains("DIVERGENCE_ACCEPT_REFUSED") && out(&same_session).contains("session `impl-session`"), "{}", out(&same_session));

    let same_actor = accept(repo, &[("--actor-id", "impl-agent")]);
    assert_eq!(same_actor.status.code(), Some(2), "{}", out(&same_actor));
    assert!(out(&same_actor).contains("`impl-agent` attested"), "{}", out(&same_actor));

    let same_reviewer = accept(repo, &[("--reviewer", "impl-agent")]);
    assert_eq!(same_reviewer.status.code(), Some(2), "{}", out(&same_reviewer));

    assert_eq!(ship(repo).status.code(), Some(1), "nothing was accepted, so the gap still blocks");
}

/// New evidence, divergent or not, ends an acceptance: the cell blocks again
/// and the refusal says the acceptance is stale.
#[test]
fn new_evidence_makes_the_acceptance_stale_and_blocks_again() {
    let dir = repo();
    let repo = dir.path();
    assert!(accept(repo, &[]).status.success());
    attest(repo, EvidenceVerdict::Divergent);

    let shipped = ship(repo);
    assert_eq!(shipped.status.code(), Some(1), "{}", out(&shipped));
    assert!(out(&shipped).contains("the acceptance by reviewer-2") && out(&shipped).contains("is stale"), "{}", out(&shipped));
    assert!(out(&shipped).contains("canon divergence accept"), "{}", out(&shipped));
    assert!(!out(&canon(repo, &["gate", "check"])).contains("accepted-divergence"), "a stale acceptance is not listed as covering anything");

    assert!(accept(repo, &[]).status.success(), "a reviewer can accept the new evidence");
    assert!(ship(repo).status.success());
}

/// An acceptance whose expiry has passed blocks again, with a hint.
#[test]
fn an_expired_acceptance_blocks_again() {
    let dir = repo();
    let repo = dir.path();
    assert!(accept(repo, &[("--expiry", "2020-01-01")]).status.success());

    let shipped = ship(repo);
    assert_eq!(shipped.status.code(), Some(1), "{}", out(&shipped));
    assert!(out(&shipped).contains("expired 2020-01-01"), "{}", out(&shipped));
    assert!(out(&shipped).contains("canon divergence accept"), "{}", out(&shipped));
}

/// Only a divergent verdict can be accepted, by a reviewer, with a tracking
/// reference; a batch writes nothing when one of its scenarios is refused.
#[test]
fn refusals_name_the_missing_condition_and_a_batch_writes_nothing() {
    let dir = repo();
    let repo = dir.path();

    let empty = accept(repo, &[("--tracking", " ")]);
    assert_eq!(empty.status.code(), Some(2), "{}", out(&empty));
    assert!(out(&empty).contains("--tracking is empty"), "{}", out(&empty));

    let implementer = canon(repo, &["divergence", "accept", "--project-id", "demo", "--scenario-id", SCENARIO, "--reason", "r", "--tracking", TRACKING, "--reviewer", "r2", "--role", "implementer"]);
    assert_eq!(implementer.status.code(), Some(2), "{}", out(&implementer));
    assert!(out(&implementer).contains("reviewer step"), "{}", out(&implementer));

    let unknown = accept(repo, &[("--scenario-id", "world.demo.99")]);
    assert_eq!(unknown.status.code(), Some(2), "{}", out(&unknown));
    assert!(out(&unknown).contains("world.demo.99"), "{}", out(&unknown));
    assert!(out(&canon(repo, &["divergence", "status"])).contains("accepted=0"), "the unknown scenario refused the whole batch");

    attest(repo, EvidenceVerdict::Faithful);
    let faithful = accept(repo, &[]);
    assert_eq!(faithful.status.code(), Some(2), "{}", out(&faithful));
    assert!(out(&faithful).contains("not divergent"), "{}", out(&faithful));
}

/// `divergence status` reports an acceptance beside the other states.
#[test]
fn divergence_status_counts_the_acceptance() {
    let dir = repo();
    assert!(accept(dir.path(), &[("--expiry", "2999-01-01")]).status.success());
    let status = canon(dir.path(), &["divergence", "status", "--json"]);
    let body: Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(body["data"]["summary"]["accepted"], 1, "{body}");
    assert_eq!(body["data"]["scenarios"][0]["state"], "accepted", "{body}");
    assert_eq!(body["data"]["scenarios"][0]["tracking"], TRACKING, "{body}");
}
