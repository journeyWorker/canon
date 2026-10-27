//! The latest-only ledger (0.23 D2) through the real binary: an append moves
//! the versions it supersedes to `.canon/archive`, default reads see the
//! ledger only, `canon query --history` and the rules that need history read
//! the archive, and `canon ledger compact` brings a ledger written before 0.23
//! to the same state without changing what the gate or `status` say.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_model::{Actor, Envelope, EvidenceRecord, EvidenceVerdict, Finding, FindingSeverity, ProjectId, RecordKind, RoleId, ScenarioId, SessionId, Subject, SubjectId, SubjectStatus};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::{Duration, Utc};
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
  divergence: local
  finding: local
";

const SCENARIO: &str = "world.demo.01";

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).output().expect("spawn canon binary")
}

fn out(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn json(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", out(o)))
}

fn role(name: &str) -> RoleId {
    RoleId::parse(name).unwrap()
}

fn ledger(repo: &Path) -> GitTier {
    GitTier::new(repo.join(".canon/ledger"))
}

fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(json_files(&path));
        } else if path.extension().is_some_and(|e| e == "json") {
            out.push(path);
        }
    }
    out.sort();
    out
}

fn ledger_count(repo: &Path, kind: &str) -> usize {
    json_files(&repo.join(format!(".canon/ledger/kind={kind}"))).len()
}

fn archive_count(repo: &Path, kind: &str) -> usize {
    json_files(&repo.join(format!(".canon/archive/kind={kind}"))).len()
}

/// A repo with one `verifying` subject owning `world.demo.01`; no evidence.
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
    support::declare_scenario(repo, "demo", SCENARIO, Some("demo-subject"));
    assert!(canon(repo, &["inventory", "sync"]).status.success(), "indexing the seeded scenario");
    dir
}

/// One attestation of `SCENARIO`, `offset` seconds from now.
fn attest(repo: &Path, agent: &str, agent_role: &str, offset: i64, verdict: EvidenceVerdict) {
    let actor = Actor::new(agent, role(agent_role)).with_session(SessionId::parse(format!("{agent}-session")).unwrap());
    let record = EvidenceRecord::new(Envelope::new(1, RecordKind::Evidence, Utc::now() + Duration::seconds(offset), actor), None, Some(ScenarioId::parse(SCENARIO).unwrap()), None, verdict)
        .with_project_id(ProjectId::parse("demo").unwrap());
    ledger(repo).write(&record).unwrap();
}

/// Put the ledger back in its pre-0.23 shape: every archived file returns to
/// the ledger, so each key holds all of its versions.
fn inflate(repo: &Path) {
    for path in json_files(&repo.join(".canon/archive")) {
        let relative = path.strip_prefix(repo.join(".canon/archive")).unwrap().to_path_buf();
        let target = repo.join(".canon/ledger").join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::rename(&path, &target).unwrap();
    }
}

fn query_count(repo: &Path, kind: &str, history: bool) -> usize {
    let mut args = vec!["query", "--kind", kind, "--json", "--all"];
    if history {
        args.push("--history");
    }
    let o = canon(repo, &args);
    assert!(o.status.success(), "{}", out(&o));
    json(&o)["data"]["count"].as_u64().map_or_else(|| panic!("no count: {}", out(&o)), |count| usize::try_from(count).expect("count fits usize"))
}

/// `status --json` without the fields that change from one run to the next.
fn status_json(repo: &Path) -> Value {
    let o = canon(repo, &["status", "--json"]);
    let mut value = json(&o);
    fn scrub(value: &mut Value) {
        match value {
            Value::Object(map) => {
                map.retain(|key, _| !matches!(key.as_str(), "generatedAt" | "now" | "at"));
                for child in map.values_mut() {
                    scrub(child);
                }
            }
            Value::Array(items) => {
                for item in items {
                    scrub(item);
                }
            }
            _ => {}
        }
    }
    scrub(&mut value);
    value
}

fn gate(repo: &Path) -> (Option<i32>, String) {
    let o = canon(repo, &["gate", "check"]);
    (o.status.code(), out(&o))
}

#[test]
fn an_append_moves_the_superseded_versions_and_default_reads_see_the_ledger_only() {
    let dir = repo();
    let repo = dir.path();
    for (offset, verdict) in [(0, EvidenceVerdict::Divergent), (10, EvidenceVerdict::Divergent), (20, EvidenceVerdict::Faithful)] {
        attest(repo, "impl-agent", "implementer", offset, verdict);
    }
    assert_eq!(ledger_count(repo, "evidence"), 1, "the ledger holds one file per key");
    assert_eq!(archive_count(repo, "evidence"), 2, "the two older versions moved to .canon/archive");
    assert_eq!(query_count(repo, "evidence", false), 1);
    assert_eq!(query_count(repo, "evidence", true), 3, "--history returns every version");
    assert!(canon(repo, &["query", "--kind", "evidence", "--history", "--plugin", "p"]).status.code() == Some(2), "--history does not combine with --plugin");
}

#[test]
fn compaction_is_idempotent_and_the_gate_and_status_say_the_same_after_it() {
    let dir = repo();
    let repo = dir.path();
    for (offset, agent, agent_role, verdict) in [
        (0, "impl-agent", "implementer", EvidenceVerdict::Divergent),
        (10, "impl-agent", "implementer", EvidenceVerdict::Divergent),
        (15, "review-agent", "reviewer", EvidenceVerdict::Faithful),
        (20, "impl-agent", "implementer", EvidenceVerdict::Faithful),
    ] {
        attest(repo, agent, agent_role, offset, verdict);
    }
    inflate(repo);
    assert_eq!((ledger_count(repo, "evidence"), archive_count(repo, "evidence")), (4, 0), "a pre-0.23 ledger: every version in the ledger");

    let (gate_before, gate_out_before) = gate(repo);
    let status_before = status_json(repo);

    let preview = canon(repo, &["ledger", "compact", "--json"]);
    assert!(preview.status.success(), "{}", out(&preview));
    assert_eq!(json(&preview)["data"]["committed"], false);
    assert_eq!(json(&preview)["data"]["superseded"], 2, "the implementer's two older records");
    assert_eq!(json(&preview)["data"]["effects"].as_array().unwrap().len(), 2, "one move per superseded file");
    assert_eq!((json(&preview)["data"]["moved"].as_u64(), ledger_count(repo, "evidence")), (Some(0), 4), "a preview moves nothing");
    let digest = json(&preview)["data"]["planDigest"].as_str().unwrap().to_string();

    let first = canon(repo, &["ledger", "compact", "--commit", "--plan-digest", &digest, "--json"]);
    assert!(first.status.success(), "{}", out(&first));
    let first = json(&first);
    assert_eq!(first["data"]["moved"], 2);
    assert_eq!(first["meta"]["command"], "ledger compact");
    assert!(first["meta"]["next"].is_array(), "a write carries meta.next");
    let evidence_row = first["data"]["kinds"].as_array().unwrap().iter().find(|row| row["kind"] == "evidence").unwrap().clone();
    assert_eq!((evidence_row["files"].as_u64(), evidence_row["live"].as_u64(), evidence_row["moved"].as_u64()), (Some(4), Some(2), Some(2)));
    assert_eq!((ledger_count(repo, "evidence"), archive_count(repo, "evidence")), (2, 2), "one file per (scenario, role) cell");

    let second = canon(repo, &["ledger", "compact", "--json"]);
    assert_eq!(json(&second)["data"]["moved"], 0, "idempotent");
    assert_eq!(json(&second)["data"]["superseded"], 0);
    let nothing = json(&second)["data"]["planDigest"].as_str().unwrap().to_string();
    let again = canon(repo, &["ledger", "compact", "--commit", "--plan-digest", &nothing, "--json"]);
    assert!(again.status.success(), "{}", out(&again));
    assert_eq!(json(&again)["data"]["moved"], 0, "a second commit finds nothing to move");

    let (gate_after, gate_out_after) = gate(repo);
    assert_eq!((gate_before, &gate_out_before), (gate_after, &gate_out_after), "gate check is unchanged by compaction");
    assert_eq!(status_before, status_json(repo), "status is unchanged by compaction");
    assert_eq!(query_count(repo, "evidence", true), 4, "history still has every version");
}

#[test]
fn compaction_commits_only_the_previewed_plan_and_the_old_flag_is_gone() {
    let dir = repo();
    let repo = dir.path();
    for offset in [0, 10, 20] {
        attest(repo, "impl-agent", "implementer", offset, EvidenceVerdict::Divergent);
    }
    inflate(repo);
    let before = (ledger_count(repo, "evidence"), archive_count(repo, "evidence"));
    let preview = canon(repo, &["ledger", "compact", "--json"]);
    let digest = json(&preview)["data"]["planDigest"].as_str().unwrap().to_string();
    assert_eq!((ledger_count(repo, "evidence"), archive_count(repo, "evidence")), before, "a preview moves nothing");

    // A newer version arrives after the preview: the plan is another plan.
    attest(repo, "impl-agent", "implementer", 30, EvidenceVerdict::Divergent);
    inflate(repo);
    let moved = (ledger_count(repo, "evidence"), archive_count(repo, "evidence"));
    let stale = canon(repo, &["ledger", "compact", "--commit", "--plan-digest", &digest, "--json"]);
    assert_eq!(stale.status.code(), Some(2), "{}", out(&stale));
    assert_eq!(json(&stale)["error"]["code"], "PLAN_CHANGED");
    assert_eq!((ledger_count(repo, "evidence"), archive_count(repo, "evidence")), moved, "a stale digest moves nothing");

    assert_eq!(canon(repo, &["ledger", "compact", "--dry-run"]).status.code(), Some(2), "--dry-run is gone");
    assert_eq!(canon(repo, &["ledger", "compact", "--commit"]).status.code(), Some(2), "--commit needs --plan-digest");
}

#[test]
fn migrate_compacts_the_ledger() {
    let dir = repo();
    let repo = dir.path();
    for offset in [0, 10, 20] {
        attest(repo, "impl-agent", "implementer", offset, EvidenceVerdict::Divergent);
    }
    inflate(repo);
    let preview = canon(repo, &["migrate"]);
    let digest = support::preview_digest(&String::from_utf8_lossy(&preview.stdout)).expect("a preview prints its planDigest");
    let migrate = canon(repo, &["migrate", "--commit", "--plan-digest", &digest]);
    assert!(migrate.status.success(), "{}", out(&migrate));
    assert!(out(&migrate).contains("moved 2 superseded version(s)"), "{}", out(&migrate));
    assert_eq!((ledger_count(repo, "evidence"), archive_count(repo, "evidence")), (1, 2));
}

fn accept(repo: &Path, actor: &str, session: &str) -> Output {
    canon(
        repo,
        &[
            "divergence", "accept", SCENARIO, "--project-id", "demo", "--reason", "the engine drops the event", "--tracking", "https://example.com/issues/1",
            "--role", "reviewer", "--actor-id", actor, "--session-id", session,
        ],
    )
}

#[test]
fn accepting_a_divergence_still_checks_independence_against_superseded_attestations() {
    let dir = repo();
    let repo = dir.path();
    attest(repo, "impl-agent", "implementer", 0, EvidenceVerdict::Divergent);
    attest(repo, "impl-two", "implementer", 10, EvidenceVerdict::Divergent);
    assert_eq!(archive_count(repo, "evidence"), 1, "impl-agent's attestation is archived: impl-two's replaced it");

    let refused = accept(repo, "impl-agent", "review-session");
    assert_eq!(refused.status.code(), Some(2), "{}", out(&refused));
    assert!(out(&refused).contains("`impl-agent` attested"), "an archived attestation still binds the reviewer: {}", out(&refused));
    let same_session = accept(repo, "reviewer-2", "impl-agent-session");
    assert_eq!(same_session.status.code(), Some(2), "{}", out(&same_session));
    assert!(out(&same_session).contains("session `impl-agent-session` attested"), "{}", out(&same_session));

    let accepted = accept(repo, "reviewer-2", "review-session");
    assert_eq!(accepted.status.code(), Some(0), "{}", out(&accepted));
}

#[test]
fn a_finding_closed_after_a_re_verification_keeps_its_close_time_in_the_ledger() {
    let dir = repo();
    let repo = dir.path();
    let raised = Finding::new(
        Envelope::current(RecordKind::Finding, Utc::now() - Duration::seconds(60), Actor::new("reviewer-1", role("reviewer"))),
        canon_model::ChangeId::parse("add-login").unwrap(),
        1,
        1,
        FindingSeverity::Blocker,
        "reviewer-1",
        "token is logged",
    );
    ledger(repo).write(&raised).unwrap();
    let closed = Finding::new(
        Envelope::current(RecordKind::Finding, Utc::now(), Actor::new("reviewer-1", role("reviewer"))),
        canon_model::ChangeId::parse("add-login").unwrap(),
        1,
        1,
        FindingSeverity::Blocker,
        "reviewer-1",
        "token is logged",
    )
    .fixed_by(canon_model::Sha::parse("a".repeat(40)).unwrap());
    let closed_at = closed.envelope.at;
    ledger(repo).write(&closed).unwrap();

    assert_eq!((ledger_count(repo, "finding"), archive_count(repo, "finding")), (1, 1), "the open version is history; the close is the record");
    let ctx = canon_gate::GateCtx::from_repo(repo).unwrap();
    let context = canon_gate::GateContext::load(ctx, &canon_policy::SchemaRegistry::load(), Utc::now()).unwrap();
    let fixed = canon_gate::review_gate::fixed_blockers(&context);
    assert_eq!(fixed.len(), 1);
    assert_eq!(fixed[0].closed, closed_at, "the close time is the latest record's `at`: no archive read needed");
    assert!(canon_gate::review_gate::open_blockers(&context).is_empty());
}

#[test]
fn a_custom_ledger_root_has_no_archive_and_compact_says_so() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), CANON_YAML.replace(".canon/ledger", "records")).unwrap();
    let refused = canon(dir.path(), &["ledger", "compact"]);
    assert_eq!(refused.status.code(), Some(2), "{}", out(&refused));
    assert!(out(&refused).contains("LEDGER_ARCHIVE_UNAVAILABLE"), "{}", out(&refused));
}
