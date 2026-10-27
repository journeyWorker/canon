//! `--dry-run` on every writing leaf that offers a preview (0.28 D6): the
//! full validation runs and refuses with the code the real run would, nothing
//! is written (the working tree and `.canon/` stay byte-identical), and the
//! report says it is a preview.
//!
//! Each leaf has two tests: a valid invocation (exit 0, tree hash unchanged,
//! `data.dryRun` true, and the real run afterwards DOES change the tree, so the
//! fixture really reaches a write) and a bad invocation (same exit code and
//! `error.code` as the real run, tree unchanged).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_model::HandoffFields;
use canon_model::{
    Actor, Change, ChangeId, ChangeStatus, DomainId, Envelope, Handoff, HandoffBody, HandoffId, HandoffState, RecordKind, RoleId, Subject, SubjectId, SubjectStatus,
};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use serde_json::Value;
use sha2::{Digest, Sha256};

mod support;

fn run(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(args)
        .current_dir(cwd)
        .env_remove("CODEX_HOME")
        .env_remove("CANON_INGEST_CLAUDE_SESSIONS_DIR")
        .env_remove("CANON_INGEST_OMP_SESSIONS_DIR")
        .env_remove("HERMES_HOME")
        .output()
        .expect("spawn canon binary")
}

/// A digest of every file and directory under each root (relative path and
/// bytes), `.git` excluded.
fn tree_hash(roots: &[&Path]) -> String {
    let mut hasher = Sha256::new();
    for root in roots {
        let mut entries: Vec<PathBuf> = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.file_name().is_some_and(|name| name == ".git") {
                    continue;
                }
                entries.push(path.clone());
                if path.is_dir() {
                    stack.push(path);
                }
            }
        }
        entries.sort();
        for path in entries {
            hasher.update(path.strip_prefix(root).unwrap().to_string_lossy().as_bytes());
            if path.is_dir() {
                hasher.update(b"/");
            } else {
                hasher.update(std::fs::read(&path).unwrap());
            }
            hasher.update([0]);
        }
    }
    format!("{:x}", hasher.finalize())
}

fn json_of(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document: {e}\nstdout: {}", String::from_utf8_lossy(&output.stdout)))
}

fn error_code(output: &Output) -> String {
    let body = json_of(output);
    assert_eq!(body["ok"], false, "{body}");
    body["error"]["code"].as_str().unwrap_or_else(|| panic!("no error.code: {body}")).to_string()
}

/// A valid preview: succeeds, writes nothing, says `dryRun: true`; the same
/// command without `--dry-run` then writes, so the fixture reaches a write.
/// Returns the preview's `data`.
fn valid_preview(args: &[&str], cwd: &Path, roots: &[&Path]) -> Value {
    let before = tree_hash(roots);
    let mut preview_args = args.to_vec();
    preview_args.extend(["--dry-run", "--json"]);
    let preview = run(&preview_args, cwd);
    assert!(preview.status.success(), "{args:?} --dry-run failed: {}{}", String::from_utf8_lossy(&preview.stdout), String::from_utf8_lossy(&preview.stderr));
    assert_eq!(tree_hash(roots), before, "{args:?} --dry-run changed the tree");
    let body = json_of(&preview);
    assert_eq!(body["ok"], true, "{body}");
    assert_eq!(body["data"]["dryRun"], true, "{body}");

    let mut real_args = args.to_vec();
    real_args.push("--json");
    let real = run(&real_args, cwd);
    assert!(real.status.success(), "{args:?} failed: {}{}", String::from_utf8_lossy(&real.stdout), String::from_utf8_lossy(&real.stderr));
    assert_ne!(tree_hash(roots), before, "{args:?} wrote nothing, so this fixture does not reach a write");
    body["data"].clone()
}

/// A bad input: the preview refuses with the exit code and `error.code` the
/// real run gives, and writes nothing.
fn bad_preview(args: &[&str], cwd: &Path, roots: &[&Path]) -> String {
    let before = tree_hash(roots);
    let mut preview_args = args.to_vec();
    preview_args.extend(["--dry-run", "--json"]);
    let preview = run(&preview_args, cwd);
    assert!(!preview.status.success(), "{args:?} --dry-run accepted a bad input");
    assert_eq!(tree_hash(roots), before, "{args:?} --dry-run changed the tree");
    let code = error_code(&preview);

    let mut real_args = args.to_vec();
    real_args.push("--json");
    let real = run(&real_args, cwd);
    assert_eq!(preview.status.code(), real.status.code(), "{args:?}: the preview and the real run exit differently");
    assert_eq!(code, error_code(&real), "{args:?}: the preview and the real run refuse with different codes");
    code
}

const LOCAL_TIER: &str = "tiers:\n  local: { backend: git, root: .canon/ledger }\n";

fn repo_with(canon_yaml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), canon_yaml).unwrap();
    dir
}

// ── init ──

#[test]
fn init_dry_run_previews_what_it_would_create() {
    let dir = tempfile::tempdir().unwrap();
    let data = valid_preview(&["init", "--providers", "none"], dir.path(), &[dir.path()]);
    assert!(data["steps"].as_array().is_some_and(|steps| steps.iter().any(|s| s["step"] == "config")), "{data}");
}

#[test]
fn init_dry_run_refuses_like_the_real_run() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("AGENTS.md"), "<!-- canon:begin -->\nunterminated\n").unwrap();
    assert_eq!(bad_preview(&["init", "--providers", "none"], dir.path(), &[dir.path()]), "INIT_AGENTS_MD_CONFLICT");
}

#[test]
fn init_dry_run_conflicts_with_check_config() {
    let dir = repo_with(LOCAL_TIER);
    let before = tree_hash(&[dir.path()]);
    let output = run(&["init", "--check-config", "--dry-run"], dir.path());
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(tree_hash(&[dir.path()]), before);
}

// ── skills install ──

fn canonical_source(root: &Path) -> PathBuf {
    let source = root.join("source");
    std::fs::create_dir_all(source.join("reference")).unwrap();
    std::fs::write(source.join("SKILL.src.md"), "---\nname: canon\ndescription: umbrella\n---\n\n# Canon\n").unwrap();
    std::fs::write(source.join("reference/topic.md"), "# topic\n").unwrap();
    source
}

#[test]
fn skills_install_dry_run_previews_without_writing() {
    let tmp = tempfile::tempdir().unwrap();
    let source = canonical_source(tmp.path());
    let target = tmp.path().join("target");
    std::fs::create_dir_all(&target).unwrap();
    let data = valid_preview(
        &["skills", "install", "--skills-source", source.to_str().unwrap(), "--repo", target.to_str().unwrap(), "--providers", "claude"],
        tmp.path(),
        &[tmp.path()],
    );
    assert_eq!(data["changed"], true, "{data}");
}

#[test]
fn skills_install_dry_run_refuses_like_the_real_run() {
    let tmp = tempfile::tempdir().unwrap();
    let source = canonical_source(tmp.path());
    let target = tmp.path().join("target");
    std::fs::create_dir_all(&target).unwrap();
    assert_eq!(
        bad_preview(&["skills", "install", "--skills-source", source.to_str().unwrap(), "--repo", target.to_str().unwrap(), "--providers", "bogus"], tmp.path(), &[tmp.path()]),
        "SKILLS_INVALID_ARGUMENT"
    );
}

// ── gate install-hooks ──

#[test]
fn gate_install_hooks_dry_run_previews_without_writing() {
    let dir = repo_with(LOCAL_TIER);
    let data = valid_preview(&["gate", "install-hooks"], dir.path(), &[dir.path()]);
    assert_eq!(data["installed"], true, "{data}");
}

#[test]
fn gate_install_hooks_dry_run_refuses_like_the_real_run() {
    let dir = repo_with(LOCAL_TIER);
    // A file where the settings directory must go: the write cannot happen.
    std::fs::write(dir.path().join(".claude"), "not a directory").unwrap();
    assert_eq!(bad_preview(&["gate", "install-hooks"], dir.path(), &[dir.path()]), "HOOKS_INSTALL_FAILED");
}

// ── inventory sync ──

#[test]
fn inventory_sync_dry_run_counts_the_records_it_would_write() {
    let dir = repo_with(&format!("{LOCAL_TIER}routing:\n  scenario: local\n"));
    support::declare_scenario(dir.path(), "demo", "area.surface.01", None);
    let data = valid_preview(&["inventory", "sync"], dir.path(), &[dir.path()]);
    assert_eq!(data["totalWritten"], 1, "{data}");
}

#[test]
fn inventory_sync_dry_run_refuses_like_the_real_run() {
    let dir = repo_with(&format!("{LOCAL_TIER}specs: 5\n"));
    assert_eq!(bad_preview(&["inventory", "sync"], dir.path(), &[dir.path()]), "INVENTORY_FAILED");
}

// ── plugin sync ──

const PORTING_YAML: &str = "id: porting\nnamespace: porting\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n    fields:\n      - name: covered\n        type: bool\n      - name: surface_ref\n        type: { list: string }\n";

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn porting_repo() -> tempfile::TempDir {
    let dir = repo_with(&format!("{LOCAL_TIER}routing:\n  scenario: local\n"));
    write(dir.path(), ".canon/plugins/porting/plugin.yaml", PORTING_YAML);
    let prov = "  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"canon-fmt\"}}";
    write(
        dir.path(),
        ".canon/specs/kind=feature/area=idolive/hub.feature",
        &format!("Feature: idolive hub\n{prov}\n\n  @idolive.hub.01\n  Scenario: Opening the hub\n{prov}\n    Given a step\n"),
    );
    write(
        dir.path(),
        ".canon/specs/inventory/kind=inventory/area=idolive/surface=hub/hub.yaml",
        "schema: 1\nkind: inventory\nat: \"2026-07-10T00:00:00Z\"\nactor:\n  agent_id: test\nidolive.hub.hub-header:\n  upstream:\n    pin: a\n    file: f.tsx\n    symbol: S\n    lines: \"1-2\"\n  covered_by: [idolive.hub.01]\n",
    );
    assert!(run(&["inventory", "sync"], dir.path()).status.success());
    dir
}

#[test]
fn plugin_sync_dry_run_counts_the_overlays_it_would_write() {
    let dir = porting_repo();
    let data = valid_preview(&["plugin", "sync", "porting"], dir.path(), &[dir.path()]);
    assert_eq!(data["written"], 1, "{data}");
}

#[test]
fn plugin_sync_dry_run_refuses_like_the_real_run() {
    let dir = porting_repo();
    assert_eq!(bad_preview(&["plugin", "sync", "nosuch"], dir.path(), &[dir.path()]), "PLUGIN_SYNC_FAILED");
}

// ── ingest records ──

fn records_repo() -> tempfile::TempDir {
    let dir = repo_with(&format!("{LOCAL_TIER}routing:\n  handoff: local\n\nartifacts:\n  ledger_root: fixtures/ledger\n"));
    let finding_dir = dir.path().join("fixtures/ledger/kind=code-review/area=world");
    std::fs::create_dir_all(&finding_dir).unwrap();
    let finding = serde_json::json!({ "kind": "code-review", "scenario_id": "world.firstbuy-hotdeal.26", "at": "2026-07-01T00:00:00Z" });
    std::fs::write(finding_dir.join("world.firstbuy-hotdeal.26.json"), serde_json::to_vec_pretty(&finding).unwrap()).unwrap();

    let envelope = Envelope::new(1, RecordKind::Handoff, "2026-07-01T09:00:00Z".parse().unwrap(), Actor::new("fixture", RoleId::parse("implementer").unwrap()));
    let body = HandoffBody { domain: DomainId::parse("development").unwrap(), template_version: 1, fields: serde_json::json!({}) };
    let mut handoff = Handoff::new(HandoffFields { envelope, id: HandoffId::parse("20260701-0910-fixture-a1b2").unwrap(), chain_id: uuid::Uuid::new_v4(), parent_handoff_id: None, seq: 1, title: "Fixture handoff".into(), openspec_change_slug: None, body });
    handoff.transition_to(HandoffState::InProgress, "2026-07-01T09:11:00Z".parse().unwrap(), Some("agent")).unwrap();
    handoff.transition_to(HandoffState::Done, "2026-07-01T09:20:00Z".parse().unwrap(), None).unwrap();
    GitTier::new(dir.path().join(".canon/ledger")).write(&handoff).unwrap();
    dir
}

#[test]
fn ingest_records_dry_run_counts_the_trajectories_it_would_persist() {
    let dir = records_repo();
    let data = valid_preview(&["ingest", "records"], dir.path(), &[dir.path()]);
    assert_eq!(data["trajectoriesPersisted"].as_array().map(Vec::len), Some(1), "{data}");
}

#[test]
fn ingest_records_dry_run_refuses_like_the_real_run() {
    let dir = records_repo();
    // A well-formed run id that no dispatch ever minted.
    assert_eq!(bad_preview(&["ingest", "records", "--run", "01ARZ3NDEKTSV4RRFFQ69G5FAV"], dir.path(), &[dir.path()]), "INGEST_FAILED");
}

// ── ingest plans ──

fn plans_repo() -> tempfile::TempDir {
    let dir = repo_with(&format!("{LOCAL_TIER}routing:\n  change: local\n  task: local\n"));
    write(
        dir.path(),
        "plans/2026-07-14-demo-feature.md",
        "# Demo Feature Implementation Plan\n\n**Goal:** Demo goal.\n\n### Task 1: Adapter\n\n- [x] step one\n- [ ] step two\n",
    );
    dir
}

#[test]
fn ingest_plans_dry_run_counts_the_records_it_would_persist() {
    let dir = plans_repo();
    let data = valid_preview(&["ingest", "plans", "--dialect", "superpowers", "--plan-source", "plans"], dir.path(), &[dir.path()]);
    assert_eq!(data["changesPersisted"], 1, "{data}");
}

#[test]
fn ingest_plans_dry_run_refuses_like_the_real_run() {
    let dir = plans_repo();
    assert_eq!(bad_preview(&["ingest", "plans", "--dialect", "nosuch", "--plan-source", "plans"], dir.path(), &[dir.path()]), "INGEST_FAILED");
}

// ── ingest sessions ──

fn sessions_fixture() -> (support::Fixture, tempfile::TempDir) {
    let fixture = support::Fixture::new("  session: local\n  run: local\n  event: local\n", "");
    let home = tempfile::tempdir().unwrap();
    let session_dir = home.path().join(".omp/agent/sessions/-tmp-proj");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(
        session_dir.join("s1.jsonl"),
        "{\"type\":\"session\",\"id\":\"dry_run_ses_1\",\"cwd\":\"/tmp/proj\"}\n\
         {\"type\":\"message\",\"timestamp\":\"2026-07-01T00:00:00Z\",\"message\":{\"role\":\"user\",\"content\":\"a directive\"}}\n\
         {\"type\":\"message\",\"timestamp\":\"2026-07-01T00:00:01Z\",\"message\":{\"role\":\"assistant\",\"model\":\"gpt-4o-mini\",\"provider\":\"openai\",\"usage\":{\"input\":10,\"output\":5}}}\n",
    )
    .unwrap();
    (fixture, home)
}

#[test]
fn ingest_sessions_dry_run_counts_what_it_would_write() {
    let (fixture, home) = sessions_fixture();
    let root = fixture.root.path();
    let data = valid_preview(&["ingest", "sessions", "--home", home.path().to_str().unwrap(), "--all-workspaces"], root, &[root, home.path()]);
    assert_eq!(data["runsWritten"], 1, "{data}");
}

#[test]
fn ingest_sessions_dry_run_refuses_like_the_real_run() {
    let (fixture, home) = sessions_fixture();
    let root = fixture.root.path();
    std::fs::write(root.join("canon.yaml"), "tiers: [unclosed\n").unwrap();
    assert_eq!(
        bad_preview(&["ingest", "sessions", "--home", home.path().to_str().unwrap(), "--all-workspaces"], root, &[root, home.path()]),
        "CONFIG_UNREADABLE"
    );
}

// ── export ──

fn export_repo() -> tempfile::TempDir {
    let dir = repo_with(&format!("{LOCAL_TIER}routing:\n  change: local\n"));
    let envelope = Envelope::new(1, RecordKind::Change, chrono::Utc::now(), Actor::new("importer", RoleId::parse("planner").unwrap()));
    let change = Change::new(envelope, ChangeId::parse("2026-10-01-demo").unwrap(), "Imported", "why", ChangeStatus::InProgress);
    GitTier::new(dir.path().join(".canon/ledger")).write(&change).unwrap();
    dir
}

#[test]
fn export_dry_run_writes_neither_the_output_file_nor_the_audit_line() {
    let dir = export_repo();
    let data = valid_preview(&["export", "--kind", "change", "--output", "out.json"], dir.path(), &[dir.path()]);
    assert_eq!(data["recordCount"], 1, "{data}");
    assert!(dir.path().join("out.json").is_file(), "the real run wrote the file");
}

#[test]
fn export_dry_run_leaves_no_output_file() {
    let dir = export_repo();
    let preview = run(&["export", "--kind", "change", "--output", "out.json", "--dry-run"], dir.path());
    assert!(preview.status.success());
    assert!(!dir.path().join("out.json").exists());
}

#[test]
fn export_dry_run_refuses_like_the_real_run() {
    let dir = export_repo();
    assert_eq!(bad_preview(&["export", "--kind", "change", "--include-sensitive"], dir.path(), &[dir.path()]), "RETENTION_EXPORT_FAILED");
    assert_eq!(bad_preview(&["export", "--kind", "change", "--output", "missing-dir/out.json"], dir.path(), &[dir.path()]), "RETENTION_EXPORT_FAILED");
}

// ── subject adopt / unadopt ──

fn subject_repo(listing: &[&str]) -> tempfile::TempDir {
    let dir = repo_with(&format!("{LOCAL_TIER}routing:\n  subject: local\n  change: local\n  scenario: local\n  evidence: local\n"));
    let ledger = GitTier::new(dir.path().join(".canon/ledger"));
    let change = Change::new(
        Envelope::new(1, RecordKind::Change, chrono::Utc::now(), Actor::new("importer", RoleId::parse("planner").unwrap())),
        ChangeId::parse("2026-10-01-demo").unwrap(),
        "Imported",
        "why",
        ChangeStatus::InProgress,
    );
    ledger.write(&change).unwrap();
    let subject = Subject::new(
        Envelope::new(1, RecordKind::Subject, chrono::Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap())),
        SubjectId::parse("checkout").unwrap(),
        "Seeded",
        "s",
        "dev",
        SubjectStatus::Proposed,
        RoleId::parse("implementer").unwrap(),
    )
    .with_change_ids(listing.iter().map(|c| ChangeId::parse(*c).unwrap()).collect());
    ledger.write(&subject).unwrap();
    dir
}

#[test]
fn subject_adopt_dry_run_records_no_link() {
    let dir = subject_repo(&[]);
    let data = valid_preview(&["subject", "adopt", "checkout", "--change", "2026-10-01-demo"], dir.path(), &[dir.path()]);
    assert_eq!(data["change_ids"], serde_json::json!(["2026-10-01-demo"]), "{data}");
}

#[test]
fn subject_adopt_dry_run_refuses_like_the_real_run() {
    let dir = subject_repo(&[]);
    assert_eq!(bad_preview(&["subject", "adopt", "nosuch", "--change", "2026-10-01-demo"], dir.path(), &[dir.path()]), "SUBJECT_ADOPT_REFUSED");
}

#[test]
fn subject_unadopt_dry_run_removes_no_link() {
    let dir = subject_repo(&["2026-10-01-demo"]);
    let data = valid_preview(&["subject", "unadopt", "checkout", "--change", "2026-10-01-demo"], dir.path(), &[dir.path()]);
    assert!(data["change_ids"].as_array().is_none_or(Vec::is_empty), "{data}");
}

#[test]
fn subject_unadopt_dry_run_refuses_like_the_real_run() {
    let dir = subject_repo(&["2026-10-01-demo"]);
    let code = bad_preview(&["subject", "unadopt", "nosuch", "--change", "2026-10-01-demo"], dir.path(), &[dir.path()]);
    assert!(code.starts_with("SUBJECT_"), "{code}");
}

// ── the shared flag ──

#[test]
fn gate_and_learn_promote_take_the_shared_flag() {
    let dir = repo_with(LOCAL_TIER);
    for args in [&["gate", "promote", "--dry-run"][..], &["strategy", "promote", "--help"]] {
        let output = run(args, dir.path());
        assert_ne!(output.status.code(), Some(2), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
    }
}
