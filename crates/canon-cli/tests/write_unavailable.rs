//! A command that writes into a target it cannot write refuses before it
//! changes anything (`canon-0-33-atip-l0` F7, scenario `atip.l0.24`): exit 2,
//! `WRITE_UNAVAILABLE`, a message that names the path, and a repo that holds
//! exactly what it held. A write that fails after the first record is exit 1
//! `WRITE_FAILED`.
//!
//! Every command whose `canon schema` effect is not `read` is covered here or
//! says why not:
//!
//! | command | target made read-only |
//! |---|---|
//! | `subject new`, `subject move`, `subject adopt`, `subject unadopt` | `.canon/ledger/kind=subject` |
//! | `change new` | `.canon/ledger/kind=change` |
//! | `inventory sync` | `.canon/ledger/kind=scenario` |
//! | `ingest plans` | `.canon/ledger/kind=change` |
//! | `ingest sessions` | `.canon` (the hot db's directory) |
//! | `ingest records` | `.canon/learn` (the trajectory store) |
//! | `report build`, `export` | `.canon` (the report file, the audit file) |
//! | `report snapshot` | the directory holding `<DIR>` |
//! | `dispatch begin`, `dispatch end` | `.canon` / `.canon/dispatch` |
//! | `context-pack add`, `prompt add` | `.canon` |
//! | `plugin sync` | `.canon/ledger` |
//! | `gate promote`, `divergence resolve`, `divergence defer` | `.canon/ledger` |
//! | `ledger compact` | `.canon/ledger/kind=subject` |
//! | `tier age`, `purge` | `.canon/ledger/kind=trajectory` |
//! | `strategy promote`, `strategy rollback` | `.canon/learn/strategies` |
//! | `artifacts store` | `.canon/artifacts/sha256` |
//! | `skills install` | the repo directory |
//!
//! Not covered through a read-only directory: `artifacts move` (it needs an
//! object-store tier; its commit refuses a read-only lock, blob directory,
//! `canon.yaml` or `.gitignore` up front), `dashboard` (a server: an
//! unwritable snapshot directory is reported as `data.snapshot.available =
//! false`, and the process stays up), `demo init`/`demo review`, `init`,
//! `migrate`, `brief approve`, `policy approve`, `feature new`, `scenario new`,
//! `finding add`/`close`, `evidence add`/`audit`, `review change`/`scenario`,
//! `divergence add`/`accept`, `task done` and `gate install-hooks` (each
//! refused up front since 0.33 and covered by its own suite or by
//! `exit_registry.rs`).
//!
//! Tests are named by scenario id (`atip_l0_24_…` proves `atip.l0.24`).

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn canon(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "write-unavailable").env("CANON_R2_LOCAL_ROOT", dir.join("r2-local")).output().expect("spawn canon binary")
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn ok(dir: &Path, args: &[&str]) -> Output {
    let out = canon(dir, args);
    assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    out
}

fn git(dir: &Path, args: &[&str]) {
    assert!(Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com"]).args(args).current_dir(dir).status().unwrap().success(), "git {args:?}");
}

/// A repo with two subjects, a change, a feature and two scenarios, committed.
fn scratch_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["subject", "new", "demo-two", "--domain", "dev", "--title", "Demo two"]);
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    ok(repo, &["feature", "new", "auth.login", "--title", "Login"]);
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    ok(repo, &["inventory", "sync"]);
    git(repo, &["init", "-q"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "fixture"]);
    dir
}

/// Gives a path its permissions back when dropped, so the temp dir can be removed.
struct ReadOnly {
    path: PathBuf,
    original: fs::Permissions,
}

impl ReadOnly {
    fn new(path: &Path) -> Self {
        let original = fs::metadata(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())).permissions();
        let mut read_only = original.clone();
        read_only.set_mode(if path.is_dir() { 0o555 } else { 0o444 });
        fs::set_permissions(path, read_only).unwrap();
        Self { path: path.to_path_buf(), original }
    }
}

impl Drop for ReadOnly {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, self.original.clone());
    }
}

/// Every file and directory under `root` (but `.git`) with the bytes of each file.
fn listing(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, found: &mut BTreeMap<String, Option<Vec<u8>>>) {
        for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().display().to_string();
            if relative == ".git" {
                continue;
            }
            if path.is_dir() {
                found.insert(relative, None);
                walk(root, &path, found);
            } else {
                found.insert(relative, Some(fs::read(&path).unwrap()));
            }
        }
    }
    let mut found = BTreeMap::new();
    walk(root, root, &mut found);
    found
}

/// Whether this process is stopped by a read-only directory (root is not).
fn enforces_permissions() -> bool {
    let dir = TempDir::new().unwrap();
    let _guard = ReadOnly::new(dir.path());
    let enforced = fs::write(dir.path().join("probe"), b"").is_err();
    if !enforced {
        eprintln!("skipped: this process can write into a directory made read-only (running as root?), so the refusal cannot be shown");
    }
    enforced
}

/// Makes `read_only` read-only, runs `canon <args> --json` in `repo`, and
/// checks it refused before changing anything: exit 2, `code`, a message that
/// names `names`, and `repo` holding what it held.
fn assert_refused(repo: &Path, read_only: &[&Path], args: &[&str], code: &str, names: &str) {
    if !enforces_permissions() {
        return;
    }
    let before = listing(repo);
    let guards: Vec<ReadOnly> = read_only.iter().map(|path| ReadOnly::new(path)).collect();
    let mut argv = args.to_vec();
    argv.push("--json");
    let out = canon(repo, &argv);
    let document = json(&out);
    assert_eq!(out.status.code(), Some(2), "canon {args:?}: {document}");
    assert_eq!(document["ok"], false, "canon {args:?}: {document}");
    assert_eq!(document["error"]["code"], code, "canon {args:?}: {document}");
    let message = document["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains(names), "canon {args:?}: the message names `{names}`: {message}");
    assert_eq!(listing(repo), before, "canon {args:?}: a refused write leaves the repo as it was");
    drop(guards);
}

const WRITE_UNAVAILABLE: &str = "WRITE_UNAVAILABLE";

#[test]
fn atip_l0_24_subject_new_into_a_read_only_ledger_is_refused() {
    let dir = scratch_repo();
    let kind = dir.path().join(".canon/ledger/kind=subject");
    assert_refused(dir.path(), &[&kind], &["subject", "new", "fresh", "--domain", "dev", "--title", "Fresh"], WRITE_UNAVAILABLE, "kind=subject");
}

#[test]
fn atip_l0_24_subject_move_into_a_read_only_ledger_is_refused() {
    let dir = scratch_repo();
    let kind = dir.path().join(".canon/ledger/kind=subject");
    assert_refused(dir.path(), &[&kind], &["subject", "move", "demo", "--to", "specced"], WRITE_UNAVAILABLE, "kind=subject");
}

#[test]
fn atip_l0_24_subject_adopt_and_unadopt_into_a_read_only_ledger_are_refused() {
    let dir = scratch_repo();
    let kind = dir.path().join(".canon/ledger/kind=subject");
    assert_refused(dir.path(), &[&kind], &["subject", "adopt", "demo-two", "--change", "add-login"], WRITE_UNAVAILABLE, "kind=subject");
    assert_refused(dir.path(), &[&kind], &["subject", "unadopt", "demo", "--change", "add-login"], WRITE_UNAVAILABLE, "kind=subject");
}

#[test]
fn atip_l0_24_change_new_refuses_a_read_only_change_store_before_creating_the_change() {
    let dir = scratch_repo();
    let kind = dir.path().join(".canon/ledger/kind=change");
    assert_refused(dir.path(), &[&kind], &["change", "new", "second", "--subject", "demo", "--title", "Second"], WRITE_UNAVAILABLE, "kind=change");
    assert!(!dir.path().join(".canon/changes/second").exists(), "no change directory was started");
}

#[test]
fn atip_l0_24_inventory_sync_into_a_read_only_ledger_is_refused() {
    let dir = scratch_repo();
    let kind = dir.path().join(".canon/ledger/kind=scenario");
    assert_refused(dir.path(), &[&kind], &["inventory", "sync"], WRITE_UNAVAILABLE, "kind=scenario");
}

#[test]
fn atip_l0_24_ingest_plans_into_a_read_only_change_store_is_refused() {
    let dir = scratch_repo();
    let kind = dir.path().join(".canon/ledger/kind=change");
    assert_refused(dir.path(), &[&kind], &["ingest", "plans"], WRITE_UNAVAILABLE, "kind=change");
}

#[test]
fn atip_l0_24_ingest_sessions_into_a_read_only_hot_store_is_refused() {
    let dir = scratch_repo();
    let home = TempDir::new().unwrap();
    let sessions = home.path().join(".omp/agent/sessions/-tmp-proj");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join("s1.jsonl"),
        "{\"type\":\"session\",\"id\":\"wu_ses_1\",\"cwd\":\"/tmp/proj\"}\n\
         {\"type\":\"message\",\"timestamp\":\"2026-07-01T00:00:00Z\",\"message\":{\"role\":\"user\",\"content\":\"hello\"}}\n\
         {\"type\":\"message\",\"timestamp\":\"2026-07-01T00:00:01Z\",\"message\":{\"role\":\"assistant\",\"model\":\"gpt-4o-mini\",\"provider\":\"openai\",\"usage\":{\"input\":10,\"output\":5}}}\n",
    )
    .unwrap();
    let canon_dir = dir.path().join(".canon");
    let home_arg = home.path().display().to_string();
    assert_refused(dir.path(), &[&canon_dir], &["ingest", "sessions", "--home", &home_arg, "--all-workspaces"], WRITE_UNAVAILABLE, "hot.db");
}

#[test]
fn atip_l0_24_ingest_records_into_a_read_only_trajectory_store_is_refused() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    fs::write(repo.join("canon.yaml"), "tiers:\n  local: { backend: git, root: .canon/ledger }\n\nartifacts:\n  ledger_root: fixtures/ledger\n").unwrap();
    let review = repo.join("fixtures/ledger/kind=code-review/area=world");
    fs::create_dir_all(&review).unwrap();
    fs::write(review.join("world.firstbuy-hotdeal.26.json"), r#"{"kind":"code-review","scenario_id":"world.firstbuy-hotdeal.26","at":"2026-07-01T00:00:00Z"}"#).unwrap();
    let learn = repo.join(".canon/learn");
    fs::create_dir_all(&learn).unwrap();
    assert_refused(repo, &[&learn], &["ingest", "records", "--repo", "."], WRITE_UNAVAILABLE, "trajectories");
}

#[test]
fn atip_l0_24_report_build_into_a_read_only_canon_directory_is_refused() {
    let dir = scratch_repo();
    let canon_dir = dir.path().join(".canon");
    assert_refused(dir.path(), &[&canon_dir], &["report", "build"], WRITE_UNAVAILABLE, "REPORT.md");
}

#[test]
fn atip_l0_24_report_snapshot_into_a_read_only_directory_is_refused() {
    let dir = scratch_repo();
    fs::create_dir(dir.path().join("out")).unwrap();
    let out = dir.path().join("out");
    assert_refused(dir.path(), &[&out], &["report", "snapshot", "out/snap"], WRITE_UNAVAILABLE, "snap");
}

const REGIME: &str = "dev/canon/join-spine/9c93d024b1a2";

#[test]
fn atip_l0_24_dispatch_begin_into_a_read_only_canon_directory_is_refused() {
    let dir = scratch_repo();
    let canon_dir = dir.path().join(".canon");
    assert_refused(dir.path(), &[&canon_dir], &["dispatch", "begin", "--role", "dev", "--regime", REGIME], WRITE_UNAVAILABLE, ".canon/dispatch");
}

#[test]
fn atip_l0_24_dispatch_end_into_a_read_only_manifest_directory_is_refused() {
    let dir = scratch_repo();
    let begun = json(&ok(dir.path(), &["dispatch", "begin", "--role", "dev", "--regime", REGIME, "--json"]));
    let run_id = begun["data"]["runId"].as_str().expect("the run id").to_string();
    let manifests = dir.path().join(".canon/dispatch");
    assert_refused(dir.path(), &[&manifests], &["dispatch", "end", &run_id, "--run-outcome", "succeeded"], WRITE_UNAVAILABLE, ".canon/dispatch");
}

const MANIFEST: &str = r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["doc.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#;

fn repo_with_manifest() -> TempDir {
    let dir = scratch_repo();
    fs::write(dir.path().join("doc.md"), "a document\n").unwrap();
    fs::write(dir.path().join("manifest.json"), MANIFEST).unwrap();
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "manifest"]);
    dir
}

#[test]
fn atip_l0_24_context_pack_add_into_a_read_only_canon_directory_is_refused() {
    let dir = repo_with_manifest();
    let canon_dir = dir.path().join(".canon");
    assert_refused(dir.path(), &[&canon_dir], &["context-pack", "add", "--manifest", "manifest.json"], WRITE_UNAVAILABLE, ".canon/context-packs");
}

#[test]
fn atip_l0_24_prompt_add_into_a_read_only_canon_directory_is_refused() {
    let dir = repo_with_manifest();
    let canon_dir = dir.path().join(".canon");
    assert_refused(dir.path(), &[&canon_dir], &["prompt", "add", "reviewer@v1", "--manifest", "manifest.json"], WRITE_UNAVAILABLE, ".canon/prompts");
}

#[test]
fn atip_l0_24_plugin_sync_into_a_read_only_ledger_is_refused() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    let write = |relative: &str, content: &str| {
        let path = repo.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    };
    write("canon.yaml", "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  scenario: local\n");
    write(".canon/plugins/porting/plugin.yaml", "id: porting\nnamespace: porting\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n    fields:\n      - name: covered\n        type: bool\n      - name: surface_ref\n        type: { list: string }\n");
    let provenance = "  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"canon-fmt\"}}";
    write(".canon/specs/kind=feature/area=idolive/hub.feature", &format!("Feature: idolive hub\n{provenance}\n\n  @idolive.hub.01\n  Scenario: Opening the hub\n{provenance}\n    Given a step\n"));
    write(
        ".canon/specs/inventory/kind=inventory/area=idolive/surface=hub/hub.yaml",
        "schema: 1\nkind: inventory\nat: \"2026-07-10T00:00:00Z\"\nactor:\n  agent_id: test\nidolive.hub.hub-header:\n  upstream:\n    pin: a\n    file: f.tsx\n    symbol: S\n    lines: \"1-2\"\n  covered_by: [idolive.hub.01]\n",
    );
    ok(repo, &["inventory", "sync", "--repo", "."]);
    let ledger = repo.join(".canon/ledger");
    assert_refused(repo, &[&ledger], &["plugin", "sync", "porting", "--repo", "."], WRITE_UNAVAILABLE, ".canon/ledger");
}

#[test]
fn atip_l0_24_gate_promote_into_a_read_only_ledger_is_refused() {
    let dir = scratch_repo();
    ok(dir.path(), &["finding", "add", "--change", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--actor", "rev", "--summary", "a note", "--no-promote"]);
    let ledger = dir.path().join(".canon/ledger");
    assert_refused(dir.path(), &[&ledger], &["gate", "promote"], WRITE_UNAVAILABLE, ".canon/ledger");
}

#[test]
fn atip_l0_24_divergence_resolve_and_defer_into_a_read_only_ledger_are_refused() {
    let dir = scratch_repo();
    let ledger = dir.path().join(".canon/ledger");
    assert_refused(dir.path(), &[&ledger], &["divergence", "resolve", "auth.login.01", "--actor", "rev", "--role", "reviewer"], WRITE_UNAVAILABLE, ".canon/ledger");
    assert_refused(
        dir.path(),
        &[&ledger],
        &["divergence", "defer", "auth.login.01", "--actor", "rev", "--role", "reviewer", "--reason", "later", "--expiry", "2099-01-01T00:00:00Z"],
        WRITE_UNAVAILABLE,
        ".canon/ledger",
    );
}

#[test]
fn atip_l0_24_ledger_compact_refuses_a_read_only_ledger_before_moving_a_version() {
    let dir = scratch_repo();
    // An older version of the subject the ledger still holds: a version the compaction would move to the archive.
    let subjects = dir.path().join(".canon/ledger/kind=subject");
    let current = fs::read_dir(&subjects).unwrap().map(|entry| entry.unwrap().path()).find(|path| path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("demo_"))).expect("the demo subject's record");
    let mut older: Value = serde_json::from_slice(&fs::read(&current).unwrap()).unwrap();
    older["at"] = Value::from("2020-01-01T00:00:00Z");
    older["title"] = Value::from("Demo, older");
    // A ledger file is named by its key and the digest of its content; one named otherwise is misfiled and left out of the fold.
    let name = format!("demo_{}.json", canon_store::partition::content_digest(&older));
    fs::write(subjects.join(name), serde_json::to_vec_pretty(&older).unwrap()).unwrap();
    let preview = json(&ok(dir.path(), &["ledger", "compact", "--json"]));
    let digest = preview["data"]["planDigest"].as_str().expect("a preview carries a planDigest").to_string();
    assert!(preview["data"]["effects"].as_array().is_some_and(|effects| !effects.is_empty()), "the older version is a move the plan lists: {preview}");
    assert_refused(dir.path(), &[&subjects], &["ledger", "compact", "--commit", "--plan-digest", &digest], WRITE_UNAVAILABLE, "kind=subject");
}

#[test]
fn atip_l0_24_export_into_a_read_only_canon_directory_is_refused() {
    let dir = scratch_repo();
    let canon_dir = dir.path().join(".canon");
    assert_refused(dir.path(), &[&canon_dir], &["export", "--kind", "subject", "--output", "out.json"], WRITE_UNAVAILABLE, "access.jsonl");
}

#[test]
fn atip_l0_24_skills_install_into_a_read_only_repo_is_refused() {
    let dir = scratch_repo();
    let repo = dir.path().to_path_buf();
    assert_refused(&repo, &[&repo], &["skills", "install", "--providers", "claude"], WRITE_UNAVAILABLE, "read-only");
}

#[test]
fn atip_l0_24_artifacts_store_into_a_read_only_blob_directory_is_refused() {
    let dir = scratch_repo();
    let repo = dir.path();
    fs::write(repo.join("shot.png"), b"png bytes").unwrap();
    ok(repo, &["review", "scenario", "auth.login.01", "--upstream-ref", "ref", "--actor", "rev", "--role", "reviewer", "--artifact", "shot.png"]);
    // The blob the review stored is gone from the store, so storing it again is a write.
    let store = repo.join(".canon/artifacts/sha256");
    for blob in fs::read_dir(&store).unwrap() {
        fs::remove_file(blob.unwrap().path()).unwrap();
    }
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "review"]);
    assert_refused(repo, &[&store], &["artifacts", "store"], WRITE_UNAVAILABLE, "sha256");
}

#[test]
fn atip_l0_24_tier_age_and_purge_refuse_a_read_only_store_before_moving_or_removing_a_record() {
    use chrono::{Duration, Utc};

    let fixture = support::Fixture::new("  trajectory: local\n", "  trajectory: { after: 1d, to: cold }\n");
    fixture.plant_trajectory_in_git(Utc::now() - Duration::days(30), 0.3);
    fs::create_dir_all(fixture.root.path().join(".canon/learn")).unwrap();
    let repo = fixture.root.path();
    let repo_arg = repo.display().to_string();

    let age = json(&ok(repo, &["tier", "age", "--json", "--repo", &repo_arg]));
    let age_digest = age["data"]["planDigest"].as_str().expect("a planDigest").to_string();
    let kind = repo.join(".canon/ledger/kind=trajectory");
    assert_refused(repo, &[&kind], &["tier", "age", "--commit", "--plan-digest", &age_digest, "--repo", &repo_arg], WRITE_UNAVAILABLE, "kind=trajectory");

    let purge = json(&ok(repo, &["purge", "--kind", "trajectory", "--before", "2099-01-01T00:00:00Z", "--json", "--repo", &repo_arg]));
    let purge_digest = purge["data"]["planDigest"].as_str().expect("a planDigest").to_string();
    assert_refused(repo, &[&kind], &["purge", "--kind", "trajectory", "--before", "2099-01-01T00:00:00Z", "--commit", "--plan-digest", &purge_digest, "--repo", &repo_arg], WRITE_UNAVAILABLE, "kind=trajectory");
}

mod strategy {
    use super::*;
    use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
    use canon_learn::{ParquetStrategyStore, ParquetTrajectoryStore, StrategyId, StrategyItem, StrategyItemFields, StrategyStore, Trajectory, TrajectoryId, TrajectoryStore, TrajectoryVerdict, VerdictOutcome};
    use canon_model::ids::{regime_key, RegimeKey, RoleId};
    use chrono::{Duration, Utc};

    fn regime() -> RegimeKey {
        RegimeKey::parse(regime_key("dev", "canon", "join-spine", "9c93d024b1a2")).unwrap()
    }

    /// A strategy in the warm tier, and five resolved successes to corroborate it.
    fn seeded(repo: &Path) -> StrategyId {
        let role = RoleId::parse("dev").unwrap();
        let store = ParquetStrategyStore::open(repo.join(".canon/learn/strategies"));
        let id = StrategyId::new();
        store
            .append(&StrategyItem::new(StrategyItemFields { id, regime_key: regime(), role: role.clone(), title: "review guidance".into(), description: "one-liner".into(), content: "guidance".into(), source_trajectory_ids: vec![TrajectoryId::new()], recorded_at: Utc::now() }))
            .expect("seed strategy");
        let trajectories = ParquetTrajectoryStore::open(repo.join(".canon/learn/trajectories"));
        for i in 0..5 {
            let row = VerdictRow { role: role.clone(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
            let at = Utc::now() - Duration::days(5 - i);
            let trajectory = Trajectory::new(TrajectoryId::new(), regime(), "landed the change", "fixture", vec![row], at, vec![]).expect("a well-formed trajectory").with_verdict_record(TrajectoryVerdict::new(VerdictOutcome::Success, 0.9));
            trajectories.append(&trajectory).expect("seed trajectory");
        }
        id
    }

    #[test]
    fn atip_l0_24_strategy_promote_into_a_read_only_strategy_store_is_refused() {
        let dir = scratch_repo();
        let id = seeded(dir.path()).to_string();
        let store = dir.path().join(".canon/learn/strategies");
        assert_refused(dir.path(), &[&store], &["strategy", "promote", &id], WRITE_UNAVAILABLE, "strategies");
    }

    #[test]
    fn atip_l0_24_strategy_rollback_into_a_read_only_strategy_store_is_refused() {
        let dir = scratch_repo();
        let id = seeded(dir.path()).to_string();
        fs::write(dir.path().join("approval.sig"), "not a signature\n").unwrap();
        let store = dir.path().join(".canon/learn/strategies");
        assert_refused(
            dir.path(),
            &[&store],
            &["strategy", "rollback", &id, "--reason", "it regressed", "--actor", "human", "--signature-file", "approval.sig", "--approval-at", "2026-01-01T00:00:00Z"],
            WRITE_UNAVAILABLE,
            "strategies",
        );
    }
}

/// The second half of `atip.l0.24`: a write that fails after the first record
/// was written is exit 1 `WRITE_FAILED`, not a refusal. `demo init` scaffolds
/// `canon.yaml` and `.gitignore`, then writes `.canon/policy.yaml`; a directory
/// standing at that path makes the third write fail after the first two landed.
#[test]
fn atip_l0_24_a_write_that_fails_after_the_first_record_is_exit_1_write_failed() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    fs::create_dir_all(repo.join(".canon/policy.yaml")).unwrap();
    let before = listing(repo);

    let out = canon(repo, &["demo", "init", "--json"]);
    let document = json(&out);
    assert_eq!(out.status.code(), Some(1), "{document}");
    assert_eq!(document["ok"], false, "{document}");
    assert_eq!(document["error"]["code"], "WRITE_FAILED", "{document}");
    assert!(document["error"]["message"].as_str().is_some_and(|message| message.contains(".canon/policy.yaml")), "the message names the path: {document}");

    let after = listing(repo);
    let added: Vec<&String> = after.keys().filter(|path| !before.contains_key(*path)).collect();
    assert!(added.iter().any(|path| path.as_str() == "canon.yaml"), "the write failed after canon.yaml was written, so work had begun: {added:?}");
}
