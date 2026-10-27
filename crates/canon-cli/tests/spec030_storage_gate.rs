//! 0.30 spec backfill, F2 (storage maintenance) and F3 (gate and policy): one
//! test per scenario, named by the scenario id. Each runs the built `canon`
//! binary against a tempdir repo and pins today's behaviour of `artifacts
//! move`, `artifacts verify`, `evidence approval-payload`, `ledger compact`,
//! `export`, `purge`, `gate install-hooks`, `selftest` and `policy show`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_model::{Actor, Envelope, EvidenceRecord, EvidenceVerdict, ProjectId, RecordKind, RoleId, ScenarioId, SessionId, Subject, SubjectId, SubjectStatus};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

mod support;

// ───────────────────────── shared helpers ─────────────────────────

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn stdout_json(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", text(o)))
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Every file under `dir` (but `.git`) with its bytes: equal snapshots mean an unchanged tree.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().to_path_buf(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").args(args).current_dir(repo).output().expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn commit_all(repo: &Path) {
    for args in [&["config", "user.name", "t"][..], &["config", "user.email", "t@example.com"], &["add", "-A"], &["commit", "-q", "-m", "x"]] {
        git(repo, args);
    }
}

// ───────────────────────── evidence repo (artifacts, approval payload, export) ─────────────────────────

struct Repo {
    dir: TempDir,
    /// The object store's root: what a bucket holds (the offline substitute).
    store: TempDir,
}

const COLD_STANZA: &str = "  cold:\n    backend: s3\n    bucket_env: CANON_R2_BUCKET\n    prefix: \"canon/\"\n";
const V1: &[u8] = br#"{"at":"2026-10-10T14:00:00Z","checks":16}"#;

impl Repo {
    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn canon(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_canon"))
            .args(args)
            .env("CANON_ACTOR", "canon")
            .env_remove("CANON_ROLE")
            .env("CANON_R2_LOCAL_ROOT", self.store.path())
            .current_dir(self.path())
            .output()
            .expect("spawn canon")
    }

    fn ok(&self, args: &[&str]) -> Output {
        let out = self.canon(args);
        assert!(out.status.success(), "canon {args:?}: {}", text(&out));
        out
    }

    fn canon_yaml(&self) -> String {
        std::fs::read_to_string(self.path().join("canon.yaml")).unwrap()
    }

    fn local_blob(&self, bytes: &[u8]) -> PathBuf {
        self.path().join(".canon/artifacts/sha256").join(sha(bytes))
    }

    fn object(&self, bytes: &[u8]) -> PathBuf {
        self.store.path().join("artifacts/sha256").join(sha(bytes))
    }

    /// Attest `game.run.01` with `reports/smoke.json` bound as its artifact.
    fn attest_report(&self) {
        std::fs::create_dir_all(self.path().join("reports")).unwrap();
        std::fs::write(self.path().join("reports/smoke.json"), V1).unwrap();
        self.ok(&["evidence", "add", "--scenario", "game.run.01", "--project", "root", "--class", "test-run", "--ref", "npm run smoke", "--role", "implementer", "--artifact", "reports/smoke.json"]);
        self.ok(&["gate", "promote"]);
    }
}

/// `git init` + `canon init --no-policy` + one scenario, with an s3 tier `cold` declared, committed.
fn new_repo() -> Repo {
    let repo = Repo { dir: tempfile::tempdir().unwrap(), store: tempfile::tempdir().unwrap() };
    git(repo.path(), &["init", "-q"]);
    repo.ok(&["init", "--repo", ".", "--no-policy"]);
    commit_all(repo.path());
    let yaml = repo.canon_yaml();
    let marker = "  # cold:\n  #   backend: s3\n  #   bucket_env: CANON_R2_BUCKET\n  #   prefix: \"canon/\"\n";
    assert!(yaml.contains(marker), "the skeleton's commented cold tier moved:\n{yaml}");
    std::fs::write(repo.path().join("canon.yaml"), yaml.replace(marker, COLD_STANZA)).unwrap();
    repo.ok(&["scenario", "new", "game.run.01", "--title", "A run ends at zero health", "--case", "failure"]);
    repo.ok(&["inventory", "sync"]);
    repo
}

// ── evidence.add.116 / 117: artifacts verify ──

#[test]
fn evidence_add_116_artifacts_verify_reports_every_bound_file_as_stored() {
    let repo = new_repo();
    repo.attest_report();
    let before = snapshot(repo.path());

    let out = repo.canon(&["artifacts", "verify", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let body = stdout_json(&out);
    assert_eq!(body["ok"], true, "{body}");
    assert_eq!((body["data"]["bound"].as_u64(), body["data"]["stored"].as_u64()), (Some(1), Some(1)), "{body}");
    assert_eq!(body["data"]["problems"], serde_json::json!([]), "{body}");
    assert_eq!(snapshot(repo.path()), before, "verify reads and writes nothing");

    // The human form says the same in one line.
    let human = repo.ok(&["artifacts", "verify"]);
    assert!(text(&human).contains("1 bound file(s) checked") && text(&human).contains("problems 0"), "{}", text(&human));
}

#[test]
fn evidence_add_117_artifacts_verify_names_a_bound_file_the_store_lost_and_exits_1() {
    let repo = new_repo();
    repo.attest_report();
    std::fs::remove_file(repo.local_blob(V1)).unwrap();
    std::fs::remove_file(repo.path().join("reports/smoke.json")).unwrap();
    let before = snapshot(repo.path());

    let out = repo.canon(&["artifacts", "verify", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    let body = stdout_json(&out);
    assert_eq!(body["ok"], false, "{body}");
    assert_eq!(body["error"]["code"], "ARTIFACTS_VERIFY_FAILED", "{body}");
    let problems = body["data"]["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 1, "{body}");
    assert_eq!((problems[0]["path"].as_str(), problems[0]["blob"].as_str(), problems[0]["sha256"].as_str()), (Some("reports/smoke.json"), Some("missing"), Some(sha(V1).as_str())), "{body}");
    assert_eq!(snapshot(repo.path()), before, "a failed verify changes nothing");
}

// ── evidence.add.118 / 119 / 120: artifacts move ──

#[test]
fn evidence_add_118_artifacts_move_previews_its_plan_then_moves_the_blobs_when_the_plan_digest_is_given() {
    let repo = new_repo();
    repo.attest_report();
    let before = snapshot(repo.path());

    let preview = repo.canon(&["artifacts", "move", "--tier", "cold", "--json"]);
    assert_eq!(preview.status.code(), Some(0), "{}", text(&preview));
    let body = stdout_json(&preview);
    assert_eq!(body["data"]["committed"], false, "{body}");
    let digest = body["data"]["planDigest"].as_str().expect("a preview carries a planDigest").to_string();
    assert_eq!(snapshot(repo.path()), before, "the preview writes nothing");
    assert!(!repo.object(V1).exists(), "the preview uploads nothing");

    let commit = repo.canon(&["artifacts", "move", "--tier", "cold", "--commit", "--plan-digest", &digest, "--json"]);
    assert_eq!(commit.status.code(), Some(0), "{}", text(&commit));
    let body = stdout_json(&commit);
    assert_eq!((body["data"]["committed"].as_bool(), body["data"]["uploaded"].as_u64(), body["data"]["removedLocal"].as_u64()), (Some(true), Some(1), Some(1)), "{body}");
    assert_eq!(std::fs::read(repo.object(V1)).unwrap(), V1, "the store holds the blob under its digest");
    assert!(!repo.local_blob(V1).exists(), "the local copy is removed");
    assert!(repo.canon_yaml().contains("artifact_store:") && repo.canon_yaml().contains("backend: cold"), "canon.yaml names the tier:\n{}", repo.canon_yaml());

    let verify = repo.canon(&["artifacts", "verify", "--full"]);
    assert_eq!(verify.status.code(), Some(0), "the moved blobs still verify: {}", text(&verify));
}

#[test]
fn evidence_add_119_artifacts_move_refuses_a_tier_that_is_no_object_store_or_a_missing_tier_and_changes_nothing() {
    let repo = new_repo();
    repo.attest_report();
    let before = snapshot(repo.path());

    let hot = repo.canon(&["artifacts", "move", "--tier", "hot", "--json"]);
    assert_eq!(hot.status.code(), Some(2), "{}", text(&hot));
    let body = stdout_json(&hot);
    assert_eq!(body["error"]["code"], "ARTIFACTS_MOVE_REFUSED", "{body}");
    assert!(body["error"]["message"].as_str().unwrap().contains("not an object store"), "{body}");
    assert_eq!(snapshot(repo.path()), before, "a refused tier changes nothing");

    let none = repo.canon(&["artifacts", "move"]);
    assert_eq!(none.status.code(), Some(2), "{}", text(&none));
    assert!(text(&none).contains("--tier"), "{}", text(&none));
    assert_eq!(snapshot(repo.path()), before, "a missing --tier changes nothing");

    let no_digest = repo.canon(&["artifacts", "move", "--tier", "cold", "--commit"]);
    assert_eq!(no_digest.status.code(), Some(2), "{}", text(&no_digest));
    assert!(text(&no_digest).contains("--plan-digest"), "{}", text(&no_digest));
    assert_eq!(snapshot(repo.path()), before, "--commit without a digest changes nothing");
    assert!(repo.local_blob(V1).exists() && !repo.object(V1).exists());
}

#[test]
fn evidence_add_120_artifacts_move_refuses_a_plan_that_changed_since_its_preview_and_moves_nothing() {
    let repo = new_repo();
    repo.attest_report();
    let preview = repo.canon(&["artifacts", "move", "--tier", "cold", "--json"]);
    let digest = stdout_json(&preview)["data"]["planDigest"].as_str().unwrap().to_string();

    // Another blob lands after the preview, so the plan recomputed now is another plan.
    std::fs::write(repo.path().join("reports/second.json"), br#"{"checks":15}"#).unwrap();
    repo.ok(&["evidence", "add", "--scenario", "game.run.01", "--project", "root", "--class", "test-run", "--ref", "npm run smoke", "--role", "reviewer", "--artifact", "reports/second.json"]);
    let before = snapshot(repo.path());

    let stale = repo.canon(&["artifacts", "move", "--tier", "cold", "--commit", "--plan-digest", &digest, "--json"]);
    assert_eq!(stale.status.code(), Some(2), "{}", text(&stale));
    assert_eq!(stdout_json(&stale)["error"]["code"], "PLAN_CHANGED");
    assert_eq!(snapshot(repo.path()), before, "a stale digest changes nothing");
    assert!(!repo.object(V1).exists(), "nothing was uploaded");

    // Previewing again prints a new digest that commits.
    let again = repo.canon(&["artifacts", "move", "--tier", "cold", "--json"]);
    let fresh = stdout_json(&again)["data"]["planDigest"].as_str().unwrap().to_string();
    assert_ne!(fresh, digest);
    let commit = repo.canon(&["artifacts", "move", "--tier", "cold", "--commit", "--plan-digest", &fresh, "--json"]);
    assert_eq!(commit.status.code(), Some(0), "{}", text(&commit));
}

// ── evidence.add.121 / 122: approval-payload ──

const AT: &str = "2026-10-26T00:00:00Z";

#[test]
fn evidence_add_121_an_approval_payload_prints_the_exact_bytes_to_sign_and_writes_nothing() {
    let repo = new_repo();
    let before = snapshot(repo.path());
    let args = ["evidence", "approval-payload", "--scenario", "game.run.01", "--project", "root", "--approver", "alice", "--approval-at", AT];

    let human = repo.canon(&args);
    assert_eq!(human.status.code(), Some(0), "{}", text(&human));
    let payload = String::from_utf8(human.stdout.clone()).unwrap();
    let doc: Value = serde_json::from_str(&payload).unwrap_or_else(|e| panic!("the payload is one JSON document ({e}): {payload}"));
    assert_eq!((doc["namespace"].as_str(), doc["subject"].as_str(), doc["project"].as_str(), doc["actor"].as_str()), (Some("canon-approval-v1"), Some("game.run.01"), Some("root"), Some("alice")), "{payload}");
    assert!(doc["timestamp"].as_str().unwrap().starts_with("2026-10-26T00:00:00"), "{payload}");
    assert!(!payload.ends_with('\n'), "the bytes to sign carry no trailing newline");

    let mut json_args = args.to_vec();
    json_args.push("--json");
    let json = repo.canon(&json_args);
    assert_eq!(json.status.code(), Some(0), "{}", text(&json));
    let body = stdout_json(&json);
    assert_eq!(body["data"]["payload"].as_str(), Some(payload.as_str()), "--json carries the same bytes");
    let hex: String = payload.bytes().map(|b| format!("{b:02x}")).collect();
    assert_eq!(body["data"]["payloadHex"].as_str(), Some(hex.as_str()));
    assert_eq!(snapshot(repo.path()), before, "emitting a payload writes nothing");
}

#[test]
fn evidence_add_130_an_approval_payload_for_a_scenario_the_index_does_not_hold_is_refused_and_prints_no_bytes() {
    let repo = new_repo();
    let before = snapshot(repo.path());
    let args = ["evidence", "approval-payload", "--scenario", "game.run.99", "--project", "root", "--approver", "alice", "--approval-at", AT];

    let human = repo.canon(&args);
    assert_eq!(human.status.code(), Some(2), "{}", text(&human));
    assert!(human.stdout.is_empty(), "no bytes to sign: {}", text(&human));
    assert!(text(&human).contains("EVIDENCE_SCENARIO_NOT_INDEXED") && text(&human).contains("canon inventory sync"), "{}", text(&human));
    let mut json_args = args.to_vec();
    json_args.push("--json");
    let json = repo.canon(&json_args);
    assert_eq!(json.status.code(), Some(2));
    let body = stdout_json(&json);
    assert_eq!(body["error"]["code"], "EVIDENCE_SCENARIO_NOT_INDEXED", "{body}");
    let message = body["error"]["message"].as_str().unwrap();
    assert!(message.contains("game.run.99") && message.contains("spec root `root`"), "{body}");
    assert!(body["error"]["hint"].as_str().unwrap().contains("canon inventory sync"), "{body}");
    assert_eq!(snapshot(repo.path()), before);

    let indexed = repo.canon(&["evidence", "approval-payload", "--scenario", "game.run.01", "--project", "root", "--approver", "alice", "--approval-at", AT]);
    assert_eq!(indexed.status.code(), Some(0), "{}", text(&indexed));
    let doc: Value = serde_json::from_slice(&indexed.stdout).unwrap_or_else(|e| panic!("the payload is one JSON document ({e}): {}", text(&indexed)));
    assert_eq!((doc["namespace"].as_str(), doc["subject"].as_str(), doc["project"].as_str(), doc["actor"].as_str()), (Some("canon-approval-v1"), Some("game.run.01"), Some("root"), Some("alice")), "{doc}");
    assert!(doc["timestamp"].as_str().unwrap().starts_with("2026-10-26T00:00:00"), "{doc}");
    assert_eq!(snapshot(repo.path()), before, "neither the refused runs nor the payload run write anything");
}

#[test]
fn evidence_add_122_an_approval_payload_is_refused_without_a_task_or_scenario_or_with_an_unreadable_time_and_prints_no_bytes() {
    let repo = new_repo();
    let before = snapshot(repo.path());

    let no_target = repo.canon(&["evidence", "approval-payload", "--approver", "alice", "--approval-at", AT, "--json"]);
    assert_eq!(no_target.status.code(), Some(2), "{}", text(&no_target));
    assert_eq!(stdout_json(&no_target)["error"]["code"], "EVIDENCE_REFUSED");

    let bad_time = repo.canon(&["evidence", "approval-payload", "--scenario", "game.run.01", "--project", "root", "--approver", "alice", "--approval-at", "yesterday"]);
    assert_eq!(bad_time.status.code(), Some(2), "{}", text(&bad_time));
    assert!(bad_time.stdout.is_empty(), "no payload bytes on stdout: {}", text(&bad_time));
    assert!(text(&bad_time).contains("invalid timestamp"), "{}", text(&bad_time));

    let no_approver = repo.canon(&["evidence", "approval-payload", "--scenario", "game.run.01", "--project", "root", "--approval-at", AT]);
    assert_eq!(no_approver.status.code(), Some(2), "{}", text(&no_approver));
    assert!(no_approver.stdout.is_empty() && text(&no_approver).contains("--approver"), "{}", text(&no_approver));
    assert_eq!(snapshot(repo.path()), before);
}

// ───────────────────────── ledger compact ─────────────────────────

const LEDGER_YAML: &str = "\
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

fn role(name: &str) -> RoleId {
    RoleId::parse(name).unwrap()
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
    out
}

fn ledger_files(repo: &Path) -> usize {
    json_files(&repo.join(".canon/ledger/kind=evidence")).len()
}

fn archive_files(repo: &Path) -> usize {
    json_files(&repo.join(".canon/archive/kind=evidence")).len()
}

fn canon_in(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env_remove("CANON_ROLE").current_dir(repo).output().expect("spawn canon binary")
}

/// A repo with one `verifying` subject owning `world.demo.01`; no evidence.
fn compact_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    std::fs::write(repo.join("canon.yaml"), LEDGER_YAML).unwrap();
    let subject = Subject::new(
        Envelope::new(1, RecordKind::Subject, Utc::now(), Actor::new("canon", role("implementer"))),
        SubjectId::parse("demo-subject").unwrap(),
        "Seeded",
        "s",
        "dev",
        SubjectStatus::Verifying,
        role("implementer"),
    );
    GitTier::new(repo.join(".canon/ledger")).write(&subject).unwrap();
    support::declare_scenario(repo, "demo", SCENARIO, Some("demo-subject"));
    assert!(canon_in(repo, &["inventory", "sync"]).status.success(), "indexing the seeded scenario");
    dir
}

fn attest(repo: &Path, offset: i64) {
    let actor = Actor::new("impl-agent", role("implementer")).with_session(SessionId::parse("impl-agent-session").unwrap());
    let record = EvidenceRecord::new(
        Envelope::new(1, RecordKind::Evidence, Utc::now() + Duration::seconds(offset), actor),
        None,
        Some(ScenarioId::parse(SCENARIO).unwrap()),
        None,
        EvidenceVerdict::Divergent,
    )
    .with_project_id(ProjectId::parse("demo").unwrap());
    GitTier::new(repo.join(".canon/ledger")).write(&record).unwrap();
}

/// Put the ledger back in its pre-0.23 shape: every archived file returns to the ledger.
fn inflate(repo: &Path) {
    for path in json_files(&repo.join(".canon/archive")) {
        let relative = path.strip_prefix(repo.join(".canon/archive")).unwrap().to_path_buf();
        let target = repo.join(".canon/ledger").join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::rename(&path, &target).unwrap();
    }
}

#[test]
fn tier_storage_12_ledger_compact_previews_the_superseded_versions_then_moves_them_on_the_previewed_digest() {
    let dir = compact_repo();
    let repo = dir.path();
    for offset in [0, 10, 20] {
        attest(repo, offset);
    }
    inflate(repo);
    assert_eq!((ledger_files(repo), archive_files(repo)), (3, 0), "every version sits in the ledger");
    let before = snapshot(repo);

    let preview = canon_in(repo, &["ledger", "compact", "--json"]);
    assert_eq!(preview.status.code(), Some(0), "{}", text(&preview));
    let body = stdout_json(&preview);
    assert_eq!((body["data"]["committed"].as_bool(), body["data"]["superseded"].as_u64(), body["data"]["moved"].as_u64()), (Some(false), Some(2), Some(0)), "{body}");
    let digest = body["data"]["planDigest"].as_str().expect("a preview carries a planDigest").to_string();
    assert_eq!(snapshot(repo), before, "the preview moves nothing");

    let commit = canon_in(repo, &["ledger", "compact", "--commit", "--plan-digest", &digest, "--json"]);
    assert_eq!(commit.status.code(), Some(0), "{}", text(&commit));
    let body = stdout_json(&commit);
    assert_eq!((body["data"]["committed"].as_bool(), body["data"]["moved"].as_u64()), (Some(true), Some(2)), "{body}");
    assert_eq!((ledger_files(repo), archive_files(repo)), (1, 2), "the newest version stays, the two older ones moved to the archive");

    let again = canon_in(repo, &["ledger", "compact", "--json"]);
    let again = stdout_json(&again);
    assert_eq!((again["data"]["superseded"].as_u64(), again["data"]["moved"].as_u64()), (Some(0), Some(0)), "nothing is left to compact: {again}");
}

#[test]
fn tier_storage_13_ledger_compact_refuses_a_plan_that_changed_a_commit_without_a_digest_and_a_ledger_with_no_archive() {
    let dir = compact_repo();
    let repo = dir.path();
    for offset in [0, 10, 20] {
        attest(repo, offset);
    }
    inflate(repo);
    let preview = canon_in(repo, &["ledger", "compact", "--json"]);
    let digest = stdout_json(&preview)["data"]["planDigest"].as_str().unwrap().to_string();

    // A newer version arrives after the preview: the plan is another plan.
    attest(repo, 30);
    inflate(repo);
    let before = snapshot(repo);

    let stale = canon_in(repo, &["ledger", "compact", "--commit", "--plan-digest", &digest, "--json"]);
    assert_eq!(stale.status.code(), Some(2), "{}", text(&stale));
    assert_eq!(stdout_json(&stale)["error"]["code"], "PLAN_CHANGED");
    assert_eq!(snapshot(repo), before, "a stale digest moves nothing");

    let no_digest = canon_in(repo, &["ledger", "compact", "--commit"]);
    assert_eq!(no_digest.status.code(), Some(2), "{}", text(&no_digest));
    assert!(text(&no_digest).contains("--plan-digest"), "{}", text(&no_digest));
    assert_eq!(snapshot(repo), before, "--commit without a digest moves nothing");

    let custom = TempDir::new().unwrap();
    std::fs::write(custom.path().join("canon.yaml"), LEDGER_YAML.replace(".canon/ledger", "records")).unwrap();
    let refused = canon_in(custom.path(), &["ledger", "compact", "--json"]);
    assert_eq!(refused.status.code(), Some(2), "{}", text(&refused));
    assert_eq!(stdout_json(&refused)["error"]["code"], "LEDGER_ARCHIVE_UNAVAILABLE");
}

// ───────────────────────── export ─────────────────────────

#[test]
fn tier_storage_14_export_prints_a_redacted_retention_manifest_and_writes_it_to_a_private_file_on_request() {
    let repo = new_repo();
    let out = repo.canon(&["export", "--kind", "scenario", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let body = stdout_json(&out);
    let data = &body["data"];
    assert_eq!((data["schema"].as_str(), data["kind"].as_str(), data["recordCount"].as_u64(), data["sensitive"].as_bool()), (Some("canon.retention.export"), Some("scenario"), Some(1), Some(false)), "{body}");
    let record = &data["records"][0];
    let mut keys: Vec<&str> = record.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["at", "digest", "id", "kind"], "a record is identity and digest, never content: {record}");
    assert!(!text(&out).contains("A run ends at zero health"), "the scenario title is not exported");

    let file = repo.path().join("manifest.json");
    let wrote = repo.canon(&["export", "--kind", "scenario", "--output", "manifest.json"]);
    assert_eq!(wrote.status.code(), Some(0), "{}", text(&wrote));
    assert!(text(&wrote).contains("wrote 1 record(s)"), "{}", text(&wrote));
    let manifest: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!((manifest["schema"].as_str(), manifest["recordCount"].as_u64()), (Some("canon.retention.export"), Some(1)), "{manifest}");
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600, "the manifest file is private");

    let before = snapshot(repo.path());
    let dry = repo.canon(&["export", "--kind", "scenario", "--dry-run", "--json"]);
    assert_eq!(dry.status.code(), Some(0), "{}", text(&dry));
    assert_eq!(stdout_json(&dry)["data"]["dryRun"], true);
    assert_eq!(snapshot(repo.path()), before, "--dry-run writes nothing");
}

#[test]
fn tier_storage_15_export_refuses_sensitive_records_without_policy_permission_an_unknown_kind_and_an_unreadable_bound() {
    let repo = new_repo();
    let before = snapshot(repo.path());

    let sensitive = repo.canon(&["export", "--kind", "scenario", "--include-sensitive", "--output", "manifest.json", "--json"]);
    assert_eq!(sensitive.status.code(), Some(2), "{}", text(&sensitive));
    let body = stdout_json(&sensitive);
    assert_eq!(body["error"]["code"], "RETENTION_EXPORT_FAILED", "{body}");
    assert!(body["error"]["message"].as_str().unwrap().contains("query.allow_sensitive"), "{body}");
    assert!(!repo.path().join("manifest.json").exists(), "no file is written");

    let kind = repo.canon(&["export", "--kind", "bogus"]);
    assert_eq!(kind.status.code(), Some(2), "{}", text(&kind));
    assert!(text(&kind).contains("invalid value 'bogus'") && text(&kind).contains("scenario"), "{}", text(&kind));

    let bound = repo.canon(&["export", "--kind", "scenario", "--before", "nope"]);
    assert_eq!(bound.status.code(), Some(2), "{}", text(&bound));
    assert!(bound.stdout.is_empty() && text(&bound).contains("invalid timestamp"), "{}", text(&bound));
    assert_eq!(snapshot(repo.path()), before, "a refused export writes nothing");
}

// ───────────────────────── purge ─────────────────────────

fn session(at: DateTime<Utc>, id: &str) -> canon_model::records::Session {
    canon_model::records::Session::new(Envelope::new(1, RecordKind::Session, at, Actor::new("test-agent", role("implementer"))), SessionId::parse(id).unwrap(), "test-client", at, None)
}

fn purge_fixture() -> support::Fixture {
    let fixture = support::Fixture::new("  session: local\n", "");
    let git = GitTier::new(fixture.git_root());
    git.write(&session("2026-01-01T00:00:00Z".parse().unwrap(), "old-session")).unwrap();
    git.write(&session("2027-06-01T00:00:00Z".parse().unwrap(), "new-session")).unwrap();
    fixture
}

fn out_text(o: &Output) -> String {
    text(o)
}

#[test]
fn tier_storage_16_purge_previews_the_removal_set_then_deletes_exactly_that_set_on_the_previewed_digest() {
    let fixture = purge_fixture();
    assert_eq!(fixture.git_file_count(), 2);
    let before = snapshot(fixture.root.path());

    let preview = fixture.run_canon(&["purge", "--kind", "session", "--before", "2026-06-01"]);
    assert_eq!(preview.status.code(), Some(0), "{}", out_text(&preview));
    let digest = support::preview_digest(&support::stdout(&preview)).unwrap_or_else(|| panic!("a preview prints its planDigest: {}", out_text(&preview)));
    assert!(support::stdout(&preview).contains("nothing written"), "{}", out_text(&preview));
    assert_eq!(snapshot(fixture.root.path()), before, "the preview deletes nothing");

    let commit = fixture.run_canon(&["purge", "--kind", "session", "--before", "2026-06-01", "--commit", "--plan-digest", &digest]);
    assert_eq!(commit.status.code(), Some(0), "{}", out_text(&commit));
    assert!(support::stdout(&commit).contains("1 matched, 1 deleted"), "{}", out_text(&commit));
    assert_eq!(fixture.git_file_count(), 1, "only the record before the cutoff is gone");
}

#[test]
fn tier_storage_17_purge_refuses_a_stale_digest_a_kind_outside_its_four_and_a_missing_cutoff_and_deletes_nothing() {
    let fixture = purge_fixture();
    let preview = fixture.run_canon(&["purge", "--kind", "session", "--before", "2026-06-01"]);
    let digest = support::preview_digest(&support::stdout(&preview)).unwrap();
    // The cutoff moves past the second record, so the removal set is another set.
    let before = snapshot(fixture.root.path());

    let stale = fixture.run_canon(&["purge", "--kind", "session", "--before", "2028-01-01", "--commit", "--plan-digest", &digest, "--json"]);
    assert_eq!(stale.status.code(), Some(2), "{}", out_text(&stale));
    assert_eq!(stdout_json(&stale)["error"]["code"], "PLAN_CHANGED");
    assert_eq!(snapshot(fixture.root.path()), before, "a stale digest deletes nothing");

    let kind = fixture.run_canon(&["purge", "--kind", "change", "--before", "2026-06-01"]);
    assert_eq!(kind.status.code(), Some(2), "{}", out_text(&kind));
    assert!(out_text(&kind).contains("[possible values: session, run, event, trajectory]"), "{}", out_text(&kind));

    let no_cutoff = fixture.run_canon(&["purge", "--kind", "session"]);
    assert_eq!(no_cutoff.status.code(), Some(2), "{}", out_text(&no_cutoff));
    assert!(out_text(&no_cutoff).contains("--before"), "{}", out_text(&no_cutoff));

    let no_digest = fixture.run_canon(&["purge", "--kind", "session", "--before", "2026-06-01", "--commit"]);
    assert_eq!(no_digest.status.code(), Some(2), "{}", out_text(&no_digest));
    assert!(out_text(&no_digest).contains("--plan-digest"), "{}", out_text(&no_digest));
    assert_eq!(snapshot(fixture.root.path()), before);
    assert_eq!(fixture.git_file_count(), 2);
}

// ───────────────────────── gate install-hooks, selftest, policy show ─────────────────────────

/// A git work tree with a bare canon.yaml.
fn hook_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("canon.yaml"), "tiers: {}\n").unwrap();
    dir
}

#[test]
fn gate_check_105_gate_install_hooks_writes_the_hook_commands_and_the_pre_commit_script_and_a_second_run_writes_nothing() {
    let dir = hook_repo();
    let repo = dir.path();

    let first = canon_in(repo, &["gate", "install-hooks", "--json"]);
    assert_eq!(first.status.code(), Some(0), "{}", text(&first));
    let body = stdout_json(&first);
    assert_eq!((body["data"]["dryRun"].as_bool(), body["data"]["installed"].as_bool(), body["data"]["preCommitScriptWritten"].as_bool()), (Some(false), Some(true), Some(true)), "{body}");
    for settings in [".claude/settings.json", ".codex/hooks.json"] {
        let doc: Value = serde_json::from_slice(&std::fs::read(repo.join(settings)).unwrap_or_else(|e| panic!("{settings}: {e}"))).unwrap();
        let hook = &doc["hooks"]["PreToolUse"][0]["hooks"][0];
        assert_eq!((hook["type"].as_str(), hook["command"].as_str(), hook["timeout"].as_u64()), (Some("command"), Some("canon task done"), Some(30)), "{settings}: {doc}");
    }
    assert!(repo.join(".canon/scripts/canon-gate-pre-commit.sh").is_file(), "the pre-commit script is written");

    let after_first = snapshot(repo);
    let again = hook_repo();
    let human = canon_in(again.path(), &["gate", "install-hooks"]);
    assert_eq!(human.status.code(), Some(0), "{}", text(&human));
    let said = String::from_utf8_lossy(&human.stdout).to_string();
    for named in [".claude/settings.json", ".codex/hooks.json", "canon-gate-pre-commit.sh", "no git hook was installed", ".git/hooks/pre-commit", "hook runner"] {
        assert!(said.contains(named), "the output names {named}: {said}");
    }
    let second = canon_in(repo, &["gate", "install-hooks", "--json"]);
    assert_eq!(second.status.code(), Some(0), "{}", text(&second));
    let body = stdout_json(&second);
    assert_eq!((body["data"]["installed"].as_bool(), body["data"]["preCommitScriptWritten"].as_bool(), body["meta"]["unchanged"].as_bool()), (Some(false), Some(false), Some(true)), "{body}");
    assert_eq!(snapshot(repo), after_first, "a second run changes no file");

    let custom_dir = hook_repo();
    let custom = canon_in(custom_dir.path(), &["gate", "install-hooks", "--event", "Stop", "--matcher", "Bash", "--command", "canon gate check", "--timeout", "10"]);
    assert_eq!(custom.status.code(), Some(0), "{}", text(&custom));
    let doc: Value = serde_json::from_slice(&std::fs::read(custom_dir.path().join(".claude/settings.json")).unwrap()).unwrap();
    let entry = &doc["hooks"]["Stop"][0];
    assert_eq!((entry["matcher"].as_str(), entry["hooks"][0]["command"].as_str(), entry["hooks"][0]["timeout"].as_u64()), (Some("Bash"), Some("canon gate check"), Some(10)), "{doc}");
}

#[test]
fn gate_check_106_gate_install_hooks_with_dry_run_reports_what_it_would_install_and_writes_nothing() {
    let dir = hook_repo();
    let repo = dir.path();
    let before = snapshot(repo);

    let out = canon_in(repo, &["gate", "install-hooks", "--dry-run", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let body = stdout_json(&out);
    let mut fields: Vec<&str> = body["data"].as_object().unwrap().keys().map(String::as_str).collect();
    fields.sort_unstable();
    assert_eq!(fields, ["dryRun", "installed", "preCommitScriptWritten"], "the preview reports three fields and names no file: {body}");
    assert_eq!((body["data"]["dryRun"].as_bool(), body["data"]["installed"].as_bool(), body["data"]["preCommitScriptWritten"].as_bool()), (Some(true), Some(true), Some(true)), "{body}");
    assert_eq!(snapshot(repo), before, "the preview writes nothing");
    let human = canon_in(repo, &["gate", "install-hooks", "--dry-run"]);
    assert_eq!(human.status.code(), Some(0), "{}", text(&human));
    let said = String::from_utf8_lossy(&human.stdout).to_string();
    for named in [".claude/settings.json", ".codex/hooks.json", "canon-gate-pre-commit.sh", "no git hook was installed"] {
        assert!(said.contains(named), "the preview names {named}: {said}");
    }
    assert!(!repo.join(".claude").exists() && !repo.join(".codex").exists() && !repo.join(".canon").exists());

    let real = canon_in(repo, &["gate", "install-hooks", "--json"]);
    assert_eq!(real.status.code(), Some(0), "{}", text(&real));
    assert_eq!(stdout_json(&real)["data"]["dryRun"], false);
    let written: Vec<PathBuf> = snapshot(repo).into_keys().filter(|p| !before.contains_key(p)).collect();
    let expected: Vec<PathBuf> = [".canon/scripts/canon-gate-pre-commit.sh", ".claude/settings.json", ".codex/hooks.json"].iter().map(PathBuf::from).collect();
    assert_eq!(written, expected, "the real run writes exactly these files");
}

#[test]
fn gate_check_107_gate_install_hooks_refuses_a_repo_that_is_no_directory_and_an_unreadable_timeout_and_writes_nothing() {
    let dir = hook_repo();
    let repo = dir.path();
    std::fs::write(repo.join("afile"), "x").unwrap();
    let before = snapshot(repo);

    let not_dir = canon_in(repo, &["gate", "install-hooks", "--repo", "afile", "--json"]);
    assert_eq!(not_dir.status.code(), Some(2), "{}", text(&not_dir));
    let body = stdout_json(&not_dir);
    assert_eq!(body["error"]["code"], "HOOKS_INSTALL_FAILED", "{body}");
    assert!(body["error"]["message"].as_str().unwrap().contains("is not a directory"), "{body}");

    for dry in [&[][..], &["--dry-run"][..]] {
        let mut args = vec!["gate", "install-hooks", "--repo", "nowhere", "--json"];
        args.extend_from_slice(dry);
        let missing = canon_in(repo, &args);
        assert_eq!(missing.status.code(), Some(2), "{}", text(&missing));
        assert_eq!(stdout_json(&missing)["error"]["code"], "HOOKS_INSTALL_FAILED");
        assert!(!repo.join("nowhere").exists(), "the missing directory is not created");
    }

    let timeout = canon_in(repo, &["gate", "install-hooks", "--timeout", "abc", "--json"]);
    assert_eq!(timeout.status.code(), Some(2), "{}", text(&timeout));
    let body = stdout_json(&timeout);
    assert_eq!((body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some("USAGE"), Some("--timeout")), "{body}");

    let unknown = canon_in(repo, &["gate", "install-hooks", "--no-such-option"]);
    assert_eq!(unknown.status.code(), Some(2), "{}", text(&unknown));
    assert_eq!(snapshot(repo), before, "every refusal leaves the tree as it was");
}

#[test]
fn gate_check_108_selftest_refuses_an_unknown_suite_and_runs_nothing() {
    let dir = TempDir::new().unwrap();
    let out = canon_in(dir.path(), &["selftest", "--suite", "no-such-suite"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(out.stdout.is_empty(), "no suite ran, so no result line: {}", text(&out));
    let message = text(&out);
    assert!(message.contains("invalid value 'no-such-suite'") && message.contains("policy-expressions") && message.contains("trust-spine-gate"), "the refusal lists the suites: {message}");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0, "nothing was written");

    // A named suite, by contrast, runs and exits 0.
    let ok = canon_in(dir.path(), &["selftest", "--suite", "policy-expressions"]);
    assert_eq!(ok.status.code(), Some(0), "{}", text(&ok));
}

// ── policy show ──

fn policy_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q"]);
    let out = canon_in(dir.path(), &["init", "--repo", "."]);
    assert!(out.status.success(), "{}", text(&out));
    dir
}

fn policy_show(repo: &Path) -> Value {
    let out = canon_in(repo, &["policy", "show", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    stdout_json(&out)["data"].clone()
}

#[test]
fn policy_expressions_33_policy_show_reports_the_policy_digest_and_whether_its_approval_holds_no_longer_holds_or_is_absent() {
    let dir = policy_repo();
    let repo = dir.path();
    let before = snapshot(repo);

    let unapproved = policy_show(repo);
    assert_eq!((unapproved["policy"].as_str(), unapproved["state"].as_str(), unapproved["approvedBy"].is_null()), (Some(".canon/policy.yaml"), Some("unapproved"), true), "{unapproved}");
    let digest = unapproved["policyDigest"].as_str().unwrap().to_string();
    assert!(digest.starts_with("sha256:"), "{unapproved}");
    let human = canon_in(repo, &["policy", "show"]);
    assert_eq!(human.status.code(), Some(0));
    assert!(text(&human).contains(&format!("digest: {digest}")) && text(&human).contains("approval: unapproved"), "{}", text(&human));
    assert_eq!(snapshot(repo), before, "policy show writes nothing");

    let approve = canon_in(repo, &["policy", "approve", "--approver", "alice", "--note", "reviewed"]);
    assert_eq!(approve.status.code(), Some(0), "{}", text(&approve));
    let approved = policy_show(repo);
    assert_eq!((approved["state"].as_str(), approved["approvedBy"].as_str(), approved["approvedDigest"].as_str(), approved["note"].as_str()), (Some("approved"), Some("alice"), Some(digest.as_str()), Some("reviewed")), "{approved}");
    assert!(approved["approvedAt"].as_str().is_some(), "{approved}");

    let policy = repo.join(".canon/policy.yaml");
    let mut edited = std::fs::read_to_string(&policy).unwrap();
    edited.push_str("# edited after approval\n");
    std::fs::write(&policy, edited).unwrap();
    let changed = policy_show(repo);
    assert_eq!((changed["state"].as_str(), changed["approvedDigest"].as_str()), (Some("changed"), Some(digest.as_str())), "{changed}");
    assert_ne!(changed["policyDigest"].as_str(), Some(digest.as_str()), "the digest follows the bytes in force");
    let human = canon_in(repo, &["policy", "show"]);
    assert!(text(&human).contains("approval: changed") && text(&human).contains("fails closed"), "{}", text(&human));

    // No policy file at all: no digest, state none.
    let bare = TempDir::new().unwrap();
    std::fs::write(bare.path().join("canon.yaml"), "tiers: {}\n").unwrap();
    let none = policy_show(bare.path());
    assert_eq!((none["state"].as_str(), none["policyDigest"].is_null()), (Some("none"), true), "{none}");
}

#[test]
fn policy_expressions_34_policy_show_outside_a_configured_repo_is_refused_with_config_missing() {
    let dir = TempDir::new().unwrap();
    let out = canon_in(dir.path(), &["policy", "show", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let body = stdout_json(&out);
    assert_eq!((body["ok"].as_bool(), body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some(false), Some("CONFIG_MISSING"), Some("--repo")), "{body}");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0, "nothing was written");

    let human = canon_in(dir.path(), &["policy", "show"]);
    assert_eq!(human.status.code(), Some(2));
    assert!(text(&human).contains("CONFIG_MISSING"), "{}", text(&human));
}

// ── plugin.overlay.10 / gate.check.109 ──

#[test]
fn plugin_overlay_10_plugin_sync_refuses_a_plugin_id_with_no_installed_manifest_and_writes_nothing() {
    let dir = hook_repo();
    let repo = dir.path();
    let before = snapshot(repo);

    let out = canon_in(repo, &["plugin", "sync", "nope", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let body = stdout_json(&out);
    assert_eq!(body["error"]["code"], "PLUGIN_SYNC_FAILED", "{body}");
    assert!(body["error"]["message"].as_str().unwrap().contains("has no installed manifest"), "{body}");
    let human = canon_in(repo, &["plugin", "sync", "nope"]);
    assert_eq!(human.status.code(), Some(2), "{}", text(&human));
    assert!(text(&human).contains("PLUGIN_SYNC_FAILED"), "{}", text(&human));

    let none = canon_in(repo, &["plugin", "sync"]);
    assert_eq!(none.status.code(), Some(2), "{}", text(&none));
    assert_eq!(snapshot(repo), before, "every refusal leaves the tree as it was");
}

#[test]
fn gate_check_109_selftest_runs_one_named_suite_clean_and_exits_0() {
    let dir = TempDir::new().unwrap();
    let out = canon_in(dir.path(), &["selftest", "--suite", "trust-spine-gate", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let body = stdout_json(&out);
    assert_eq!(body["data"]["clean"], true, "{body}");
    let suites = body["data"]["suites"].as_array().unwrap();
    assert_eq!(suites.len(), 1, "{body}");
    assert_eq!((suites[0]["name"].as_str(), suites[0]["clean"].as_bool(), suites[0]["failures"].as_array().map(Vec::len)), (Some("trust-spine-gate"), Some(true), Some(0)), "{body}");
    assert!(suites[0]["passed"].as_u64().unwrap() > 0, "{body}");
}
