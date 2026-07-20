//! Integration tests for `canon subject {new,adopt,status}` (s36
//! `subject-domain-loop`), invoking the actually-built `canon` binary
//! against an offline git-tier fixture in a tmpdir — zero network, no
//! credentials (mirrors `tests/gate.rs`/`tests/query.rs`'s shape). The
//! subject write path routes through `TierRegistry` (subject → `local`
//! rung → git tier at `canon/ledger`); records seeded directly here go
//! through the SAME `GitTier` root, so the CLI reads back exactly what
//! the fixtures plant.

use std::path::Path;
use std::process::{Command, Output};

use canon_model::{
    Actor, Change, ChangeId, ChangeStatus, Envelope, EvidenceRecord, EvidenceVerdict, RecordKind, RoleId, ScenarioId, Subject,
    SubjectId, SubjectStatus,
};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::Utc;
use serde_json::Value;
use tempfile::TempDir;

/// A minimal, WORKING `canon.yaml` routing every kind this suite
/// touches to the git-backed `local` rung at `canon/ledger` — the same
/// root `GateCtx::from_repo` (the `verifying → shipped` evidence gate)
/// resolves from `tiers.local.root`.
const CANON_YAML: &str = "\
tiers:
  local: { backend: git, root: canon/ledger }
routing:
  subject: local
  change: local
  scenario: local
  evidence_record: local
";

fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), CANON_YAML).unwrap();
    dir
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
    GitTier::new(repo.join("canon/ledger"))
}

fn seed_change(repo: &Path, change_id: &str) {
    let envelope = Envelope::new(1, RecordKind::Change, Utc::now(), Actor::new("importer", RoleId::parse("planner").unwrap()));
    let change = Change::new(envelope, ChangeId::parse(change_id).unwrap(), "Imported", "why", ChangeStatus::InProgress);
    ledger(repo).write(&change).unwrap();
}

/// Plant a `Subject` at an arbitrary lifecycle state with linked
/// scenarios — the only way to reach the `verifying → shipped` gate,
/// since scenario links land via inventory sync's `@subject` tag, not a
/// CLI verb.
fn seed_subject(repo: &Path, id: &str, status: SubjectStatus, scenarios: &[&str]) {
    let envelope = Envelope::new(1, RecordKind::Subject, Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap()));
    let subject = Subject::new(envelope, SubjectId::parse(id).unwrap(), "Seeded", "s", "dev", status, RoleId::parse("implementer").unwrap())
        .with_links(Vec::new(), scenarios.iter().map(|s| ScenarioId::parse(*s).unwrap()).collect());
    ledger(repo).write(&subject).unwrap();
}

fn seed_scenario_verdict(repo: &Path, scenario: &str, verdict: EvidenceVerdict) {
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("reviewer-1", RoleId::parse("reviewer").unwrap()));
    let record = EvidenceRecord::new(envelope, None, Some(ScenarioId::parse(scenario).unwrap()), None, verdict);
    ledger(repo).write(&record).unwrap();
}

fn query_subjects(repo: &Path, extra: &[&str]) -> Value {
    let mut args = vec!["query", "--kind", "subject", "--json"];
    args.extend_from_slice(extra);
    let out = run(repo, &args);
    assert!(out.status.success(), "query failed: {}", stderr(&out));
    serde_json::from_str(&stdout(&out)).expect("valid JSON on stdout")
}

#[test]
fn new_then_query_round_trips_the_authored_subject() {
    let dir = repo();
    let out = run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["count"], 1);
    let records = payload["records"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["subject_id"], "demo-subject");
    assert_eq!(records[0]["domain"], "dev");
    assert_eq!(records[0]["status"], "proposed");
}

#[test]
fn duplicate_new_is_refused_and_leaves_the_store_unchanged() {
    let dir = repo();
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "First"]).status.success());

    let out = run(dir.path(), &["subject", "new", "demo-subject", "--domain", "planning", "--title", "Second"]);
    assert_eq!(out.status.code(), Some(2), "a duplicate id must be refused");
    assert!(stderr(&out).contains("already exists"), "stderr: {}", stderr(&out));

    // Still exactly one row, still the original domain (the second
    // write never happened).
    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["count"], 1);
    assert_eq!(payload["records"][0]["domain"], "dev");
}

#[test]
fn adopt_links_the_change_and_the_subject_on_both_sides() {
    let dir = repo();
    seed_change(dir.path(), "s36-demo");
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]).status.success());

    let out = run(dir.path(), &["subject", "adopt", "s36-demo", "--subject", "demo-subject"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    // Subject side: `change_ids` gained the change (folded to one row).
    let subjects = query_subjects(dir.path(), &[]);
    assert_eq!(subjects["count"], 1);
    let change_ids = subjects["records"][0]["change_ids"].as_array().unwrap();
    assert!(change_ids.iter().any(|c| c == "s36-demo"), "subject.change_ids must include the adopted change: {change_ids:?}");

    // Change side: some version carries the stamped `subject_id`.
    let cq = run(dir.path(), &["query", "--kind", "change", "--json"]);
    assert!(cq.status.success(), "stderr: {}", stderr(&cq));
    let change_payload: Value = serde_json::from_str(&stdout(&cq)).unwrap();
    let changes = change_payload["records"].as_array().unwrap();
    assert!(
        changes.iter().any(|c| c["subject_id"] == "demo-subject"),
        "an adopted change must carry subject_id=demo-subject: {changes:?}"
    );
}

#[test]
fn adopt_refuses_an_unknown_change() {
    let dir = repo();
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]).status.success());
    let out = run(dir.path(), &["subject", "adopt", "no-such-change", "--subject", "demo-subject"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("does not exist"), "stderr: {}", stderr(&out));
}

#[test]
fn the_forward_status_chain_advances_and_folds_to_one_row() {
    let dir = repo();
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]).status.success());

    for state in ["specced", "building", "verifying"] {
        let out = run(dir.path(), &["subject", "status", "demo-subject", state]);
        assert!(out.status.success(), "transition to {state} failed: {}", stderr(&out));
    }

    // Four writes (new + three transitions) fold to ONE current row.
    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["count"], 1, "adopt/status re-writes must read back as one latest row");
    assert_eq!(payload["records"][0]["status"], "verifying");
}

#[test]
fn an_off_chain_transition_is_refused_and_the_record_is_unchanged() {
    let dir = repo();
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]).status.success());

    // proposed → shipped skips the chain.
    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("invalid transition"), "stderr: {}", stderr(&out));

    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["records"][0]["status"], "proposed");
}

#[test]
fn shipped_is_blocked_when_a_linked_scenario_has_no_verdict() {
    let dir = repo();
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["world.demo.01"]);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped"]);
    assert_eq!(out.status.code(), Some(1), "verifying → shipped must fail closed without evidence");
    let err = stderr(&out);
    assert!(err.contains("uncovered-cell"), "must print by failure class: {err}");
    assert!(err.contains("world.demo.01"), "must name the uncovered scenario: {err}");

    // Record unchanged — still verifying.
    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["records"][0]["status"], "verifying");
}

#[test]
fn shipped_is_blocked_when_a_linked_scenario_is_divergent() {
    let dir = repo();
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["world.demo.01"]);
    seed_scenario_verdict(dir.path(), "world.demo.01", EvidenceVerdict::Divergent);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped"]);
    assert_eq!(out.status.code(), Some(1), "a divergent latest verdict must fail closed");
    assert!(stderr(&out).contains("divergent"), "stderr: {}", stderr(&out));

    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["records"][0]["status"], "verifying");
}

#[test]
fn shipped_is_allowed_with_a_faithful_verdict_for_every_linked_scenario() {
    let dir = repo();
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["world.demo.01"]);
    seed_scenario_verdict(dir.path(), "world.demo.01", EvidenceVerdict::Faithful);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["records"][0]["status"], "shipped");
}

#[test]
fn domain_and_status_filters_scope_the_subject_view() {
    let dir = repo();
    assert!(run(dir.path(), &["subject", "new", "alpha", "--domain", "dev", "--title", "A"]).status.success());
    assert!(run(dir.path(), &["subject", "new", "beta", "--domain", "planning", "--title", "B"]).status.success());

    // --domain filters to one row.
    let dev = query_subjects(dir.path(), &["--domain", "dev"]);
    assert_eq!(dev["count"], 1);
    assert_eq!(dev["records"][0]["subject_id"], "alpha");

    // --status filters by the subject's own status domain.
    let proposed = query_subjects(dir.path(), &["--status", "proposed"]);
    assert_eq!(proposed["count"], 2);
    let shipped = query_subjects(dir.path(), &["--status", "shipped"]);
    assert_eq!(shipped["count"], 0);
}

#[test]
fn domain_filter_is_rejected_on_a_non_subject_kind() {
    let dir = repo();
    let out = run(dir.path(), &["query", "--kind", "change", "--domain", "dev"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("--domain"), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("--kind subject"), "must name the one supported kind: {}", stderr(&out));
}
