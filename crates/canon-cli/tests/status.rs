//! Integration tests for `canon status [--repo] [--json]` (0.14 D7),
//! invoking the built `canon` binary against offline git-tier fixtures
//! in a tmpdir. Records are planted through the same `GitTier` root the
//! CLI reads (`tests/subject.rs`'s shape), so each test controls exactly
//! which scenarios, evidence, reviews and findings exist.

use std::path::Path;
use std::process::{Command, Output};

use canon_model::{
    Actor, ChangeId, Envelope, EvidenceRecord, EvidenceVerdict, Finding, FindingSeverity, ProjectId, ProvenanceRef, RecordKind, Review, RoleId, Scenario,
    ScenarioId, SpecDigest, Subject, SubjectId, SubjectStatus,
};
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
";

/// Evidence, failure cases and an independent review, all required.
const FULL_POLICY: &str = "spec_coverage:\n  require_evidence: true\n  require_cases: [failure]\n  require_review: {}\n";

fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), CANON_YAML).unwrap();
    dir
}

fn write_policy(repo: &Path, yaml: &str) {
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), yaml).unwrap();
}

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).output().expect("spawn canon binary")
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

fn seed_subject(repo: &Path, id: &str, status: SubjectStatus, changes: &[&str]) {
    let envelope = Envelope::new(1, RecordKind::Subject, Utc::now(), actor("canon", "implementer"));
    let subject = Subject::new(envelope, SubjectId::parse(id).unwrap(), "Seeded", "s", "dev", status, RoleId::parse("implementer").unwrap())
        .with_change_ids(changes.iter().map(|c| ChangeId::parse(*c).unwrap()).collect());
    ledger(repo).write(&subject).unwrap();
}

/// One scenario as `canon inventory sync` writes it from its tags.
fn seed_scenario(repo: &Path, id: &str, subject: Option<&str>, case: Option<&str>) {
    let envelope = Envelope::new(1, RecordKind::Scenario, Utc::now(), actor("canon", "implementer"));
    let mut scenario = Scenario::new(envelope, ProjectId::parse("demo").unwrap(), ScenarioId::parse(id).unwrap(), "seeded", "", SpecDigest::of(id.as_bytes()));
    scenario.subject_id = subject.map(|s| SubjectId::parse(s).unwrap());
    scenario.case = case.map(str::to_string);
    ledger(repo).write(&scenario).unwrap();
}

fn seed_evidence(repo: &Path, scenario: &str, verdict: EvidenceVerdict) {
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), actor("impl-agent", "implementer"));
    let record = EvidenceRecord::new(envelope, None, Some(ScenarioId::parse(scenario).unwrap()), None, verdict).with_project_id(ProjectId::parse("demo").unwrap());
    ledger(repo).write(&record).unwrap();
}

fn seed_review(repo: &Path, scenario: &str, reviewer: &str) {
    let envelope = Envelope::new(1, RecordKind::Review, Utc::now(), actor(reviewer, "reviewer"));
    let review = Review::new(
        envelope,
        ProjectId::parse("demo").unwrap(),
        ScenarioId::parse(scenario).unwrap(),
        reviewer,
        format!("pin-{reviewer}-{scenario}"),
        ProvenanceRef::OriginalSpecRef("specs/demo.feature".into()),
    );
    ledger(repo).write(&review).unwrap();
}

fn seed_blocker(repo: &Path, change: &str) {
    let envelope = Envelope::new(1, RecordKind::Finding, Utc::now(), actor("reviewer-2", "reviewer"));
    let finding = Finding::new(envelope, ChangeId::parse(change).unwrap(), 1, 1, FindingSeverity::Blocker, "reviewer-2", "unbounded subprocess wait");
    ledger(repo).write(&finding).unwrap();
}

fn status_json(repo: &Path) -> Value {
    let out = run(repo, &["status", "--json"]);
    assert_eq!(out.status.code(), Some(0), "status always exits 0; stderr: {}", stderr(&out));
    serde_json::from_str(&stdout(&out)).expect("status --json prints one JSON object")
}

fn subject<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["subjects"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap_or_else(|| panic!("no subject `{id}` in {report}"))
}

fn commands(report: &Value) -> Vec<String> {
    report["next"].as_array().unwrap().iter().map(|n| n["command"].as_str().unwrap().to_string()).collect()
}

/// Every file under `dir`, recursively, with its bytes.
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push((path.display().to_string(), std::fs::read(&path).unwrap()));
            }
        }
    }
    files.sort();
    files
}

/// A `building` subject with two surfaces: `world.demo` has two scenarios,
/// one evidenced and reviewed by someone else, one neither, and no failure
/// case; `world.edge` has one evidenced failure-case scenario reviewed only
/// by its own evidence actor. An adopted change carries an open blocker.
#[test]
fn status_counts_each_subject_through_the_gates_joins() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &["c-demo"]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), Some("happy"));
    seed_scenario(dir.path(), "world.demo.02", Some("demo-subject"), Some("happy"));
    seed_scenario(dir.path(), "world.edge.01", Some("demo-subject"), Some("failure"));
    seed_evidence(dir.path(), "world.demo.01", EvidenceVerdict::Faithful);
    seed_evidence(dir.path(), "world.edge.01", EvidenceVerdict::Divergent);
    seed_review(dir.path(), "world.demo.01", "reviewer-2");
    seed_review(dir.path(), "world.edge.01", "impl-agent");
    seed_blocker(dir.path(), "c-demo");

    let report = status_json(dir.path());
    assert_eq!(report["statusVersion"], 1);
    assert_eq!(report["canonVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(report["policy"]["present"], true);
    assert_eq!(report["policy"]["spec_coverage"]["require_cases"], serde_json::json!(["failure"]));
    let s = subject(&report, "demo-subject");
    assert_eq!(s["status"], "building");
    assert_eq!(s["scenarios"], 3);
    assert_eq!(s["evidenced"], 2);
    assert_eq!(s["divergent"], 1);
    // distinct_actor: the self-review of world.edge.01 does not count.
    assert_eq!(s["reviewed"], 1, "{s}");
    // building → verifying enters require_review's default scope.
    assert_eq!(s["reviewDue"], true);
    assert_eq!(s["openBlockers"], 1);
    assert_eq!(s["missingCases"], serde_json::json!([{"surface": "world.demo", "case": "failure"}]));

    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains("  building (1):\n    demo-subject: 3 scenarios, 2 evidenced, 1 divergent, 1 reviewed (2 due), 1 open blockers\n"), "{text}");
    assert!(text.contains("      missing @case:failure on world.demo\n"), "{text}");
}

#[test]
fn status_next_names_the_command_for_each_gap() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["c-demo"]);
    seed_subject(dir.path(), "empty-subject", SubjectStatus::Building, &[]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), Some("happy"));
    seed_scenario(dir.path(), "world.demo.02", Some("demo-subject"), Some("happy"));
    seed_evidence(dir.path(), "world.demo.01", EvidenceVerdict::Faithful);
    seed_blocker(dir.path(), "c-demo");

    let report = status_json(dir.path());
    assert_eq!(
        commands(&report),
        [
            "canon scenario new world.demo.03 --title \"<what happens on this path>\" --subject demo-subject --case failure",
            "canon evidence add --scenario-id world.demo.02 --project-id demo --kind test-run --role implementer --verdict faithful --ref \"<test command>\"",
            "canon finding close --change-id c-demo --round 1 --seq 1 --disposition fixed --resolution-sha <sha>",
            "canon review add --project-id demo --scenario-id world.demo.01 --reviewer <reviewer> --actor-id <reviewer> --role reviewer --pin <sha> --original-spec-ref <feature file>",
            "canon scenario new <area>.<surface>.01 --title \"<behavior>\" --subject empty-subject --case happy",
        ]
    );
    assert_eq!(report["nextOmitted"], 0);
    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains("next:\n  # demo-subject: 1 surface(s) lack a @case:failure scenario (first: world.demo)"), "{text}");
}

#[test]
fn status_on_a_repo_with_no_subject_points_at_subject_new() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    let report = status_json(dir.path());
    assert_eq!(report["subjects"], serde_json::json!([]));
    assert_eq!(commands(&report), ["canon subject new <id> --domain <domain> --title \"<title>\""]);
}

/// The advance rule fires only for a subject the transition gates accept:
/// following the suggested `shipped` command succeeds.
#[test]
fn status_suggests_shipping_only_what_the_ship_gate_accepts() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &[]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), Some("happy"));
    seed_scenario(dir.path(), "world.demo.02", Some("demo-subject"), Some("failure"));
    for id in ["world.demo.01", "world.demo.02"] {
        seed_evidence(dir.path(), id, EvidenceVerdict::Faithful);
        seed_review(dir.path(), id, "reviewer-2");
    }

    let report = status_json(dir.path());
    assert_eq!(commands(&report), ["canon subject status demo-subject shipped"]);
    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped"]);
    assert!(out.status.success(), "the suggested transition must pass its gates: {}", stderr(&out));
    let report = status_json(dir.path());
    assert_eq!(subject(&report, "demo-subject")["status"], "shipped");
    assert_eq!(report["next"], serde_json::json!([]));
    assert!(stdout(&run(dir.path(), &["status"])).contains("next: nothing"));
}

#[test]
fn status_lists_scenarios_no_subject_owns() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Proposed, &[]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), Some("failure"));
    seed_scenario(dir.path(), "world.loose.01", None, None);
    seed_scenario(dir.path(), "world.loose.02", None, None);

    let report = status_json(dir.path());
    assert_eq!(report["unowned"], serde_json::json!(["world.loose.01", "world.loose.02"]));
    let next = report["next"].as_array().unwrap();
    let last = next.last().unwrap();
    assert_eq!(last["command"], "canon inventory sync");
    assert!(last["why"].as_str().unwrap().contains("2 scenario(s) carry no @subject tag (first: world.loose.01)"), "{last}");
    assert!(stdout(&run(dir.path(), &["status"])).contains("unowned (2, no @subject): world.loose.01, world.loose.02\n"));
}

#[test]
fn status_warns_when_no_policy_enforces_coverage() {
    let dir = repo();
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &[]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), None);

    let report = status_json(dir.path());
    assert_eq!(report["policy"]["present"], false);
    assert!(report["policy"]["spec_coverage"].is_null());
    let warnings = report["warnings"].as_array().unwrap();
    assert!(warnings.iter().any(|w| w.as_str().unwrap().starts_with("no policy: .canon/policy.yaml not found")), "{report}");

    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains("policy: absent\nwarning: no policy: .canon/policy.yaml not found, so `canon gate check` requires no evidence, failure cases or review here\n"), "{text}");

    write_policy(dir.path(), "trust_required:\n  p1: human\n");
    let report = status_json(dir.path());
    assert_eq!(report["policy"]["present"], true);
    assert!(report["warnings"][0].as_str().unwrap().starts_with("policy has no `spec_coverage` section"), "{report}");
}

#[test]
fn status_reports_an_unreadable_canon_yaml_and_still_exits_zero() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), "tiers: [not, a, map]\n").unwrap();

    let report = status_json(dir.path());
    assert!(report["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().starts_with("cannot read this repo's ledger: parsing `")), "{report}");
    assert_eq!(commands(&report), ["canon init --check-config"]);
}

#[test]
fn status_is_a_read_that_writes_nothing() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["c-demo"]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), None);
    seed_blocker(dir.path(), "c-demo");

    let before = snapshot(dir.path());
    for args in [&["status"][..], &["status", "--json"][..]] {
        let out = run(dir.path(), args);
        assert_eq!(out.status.code(), Some(0), "gaps never fail a read: {}", stderr(&out));
    }
    assert_eq!(snapshot(dir.path()), before, "status must not write any file, including .canon/audit");
}

#[test]
fn status_caps_next_at_five() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    for n in 1..=7 {
        seed_subject(dir.path(), &format!("subject-{n}"), SubjectStatus::Building, &[]);
    }
    seed_scenario(dir.path(), "world.demo.01", None, None);

    let report = status_json(dir.path());
    assert_eq!(report["next"].as_array().unwrap().len(), 5);
    assert_eq!(report["nextOmitted"], 3, "7 subject steps + 1 unowned step, 5 shown");
    assert!(stdout(&run(dir.path(), &["status"])).contains("  … 3 more once these are done\n"));
}

#[test]
fn status_on_an_unsynced_corpus_points_at_inventory_sync() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &[]);
    seed_subject(dir.path(), "other-subject", SubjectStatus::Building, &[]);

    let report = status_json(dir.path());
    assert_eq!(commands(&report), ["canon inventory sync"]);
    assert!(report["warnings"].as_array().unwrap().iter().any(|w| w == "the ledger holds no scenario records, so every subject reads as owning none"), "{report}");
}
