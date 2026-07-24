//! Shared fixture/process helpers for the `tier_age`/`query` integration
//! tests (S2 tasks 3.3/4.1) — a git-tier + local-filesystem-backed r2-tier
//! fixture repo (`R2Tier::local`, zero network, no credentials), records
//! planted directly via `canon-store`'s library (mirroring
//! `canon-store`'s own `registry::tests` fixtures), then exercised
//! through the actually-built `canon` binary (`env!("CARGO_BIN_EXE_canon")`),
//! never by calling `canon_cli`'s library functions in-process.
//!
//! Shared across multiple `tests/*.rs` binaries — each binary only calls
//! a subset of these helpers, so per-binary dead-code warnings are
//! expected and suppressed here rather than by padding every test file
//! with calls it doesn't otherwise need.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::handoff::{DomainId, Handoff, HandoffBody};
use canon_model::ids::{ChangeId, HandoffId, ProjectId, RoleId, RunId, ScenarioId, SpecDigest, TaskId};
use canon_model::records::{Change, ChangeStatus, Scenario, Task, TaskStatus, Trajectory};
use canon_store::git_tier::GitTier;
use canon_store::tier::{Tier, WriteReceipt};
use chrono::{DateTime, Utc};

/// A tempdir-rooted fixture: `<root>/canon.yaml`, `<root>/.canon/ledger`
/// (git tier), `<root>/r2-local` (the offline r2 tier's local root,
/// bound in via `canon-cli`'s `CANON_R2_LOCAL_ROOT` test seam).
pub struct Fixture {
    pub root: tempfile::TempDir,
}

impl Fixture {
    /// `routing`/`aging` are pasted verbatim under their respective
    /// `canon.yaml` keys — the caller supplies exactly the YAML body
    /// each test needs, mirroring `canon-store`'s own `registry::tests`
    /// `POLICY_YAML` fixtures.
    pub fn new(routing_yaml: &str, aging_yaml: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let canon_yaml = format!(
            "tiers:\n  local: {{ backend: git, root: .canon/ledger }}\n  cold: {{ backend: s3, bucket_env: CANON_R2_BUCKET_TEST_UNUSED, prefix: \"canon/\" }}\nrouting:\n{routing_yaml}\naging:\n{aging_yaml}\n"
        );
        std::fs::write(root.path().join("canon.yaml"), canon_yaml).unwrap();
        Self { root }
    }

    pub fn canon_yaml_path(&self) -> PathBuf {
        self.root.path().join("canon.yaml")
    }

    pub fn git_root(&self) -> PathBuf {
        self.root.path().join(".canon/ledger")
    }

    pub fn r2_root(&self) -> PathBuf {
        self.root.path().join("r2-local")
    }

    /// Write a `Trajectory` record straight into the git tier via
    /// `canon-store`'s own library `Tier::write` — never through the
    /// CLI — so a test's fixture setup is independent of the CLI code
    /// under test.
    pub fn plant_trajectory_in_git(&self, at: DateTime<Utc>, reward: f64) -> RunId {
        let git = GitTier::new(self.git_root());
        let run_id = RunId::new();
        let record = Trajectory::new(Envelope::new(1, RecordKind::Trajectory, at, actor()), run_id, None, None, None, None, Some(reward));
        git.write(&record).unwrap();
        run_id
    }

    /// Write a `Scenario` record straight into the git tier (mirrors
    /// `plant_trajectory_in_git`'s own "planted independently of the
    /// CLI under test" discipline) -- returns the [`WriteReceipt`] so a
    /// caller can locate the exact on-disk file
    /// (`self.git_root().join(receipt.location)`) for a byte-identical
    /// before/after comparison (s16 P3, tasks.md 3.4).
    pub fn plant_scenario_in_git(&self, project_id: &str, scenario_id: &str, title: &str, at: DateTime<Utc>) -> WriteReceipt {
        let git = GitTier::new(self.git_root());
        let record = Scenario::new(
            Envelope::new(1, RecordKind::Scenario, at, actor()),
            ProjectId::parse(project_id).unwrap(),
            ScenarioId::parse(scenario_id).unwrap(),
            title,
            "",
            SpecDigest::parse("a".repeat(64)).unwrap(),
        );
        git.write(&record).unwrap()
    }

    /// Write a `Change` record straight into the git tier (s19
    /// `query-scope-filters` fixtures) -- mirrors `plant_scenario_in_git`'s
    /// own "planted independently of the CLI under test" discipline.
    pub fn plant_change_in_git(&self, change_id: &str, title: &str, status: ChangeStatus, at: DateTime<Utc>) -> WriteReceipt {
        let git = GitTier::new(self.git_root());
        let record = Change::new(Envelope::new(1, RecordKind::Change, at, actor()), ChangeId::parse(change_id).unwrap(), title, "", status);
        git.write(&record).unwrap()
    }

    /// Write a `Task` record straight into the git tier (s19
    /// `query-scope-filters` fixtures).
    pub fn plant_task_in_git(&self, task_id: &str, title: &str, status: TaskStatus, at: DateTime<Utc>) -> WriteReceipt {
        let git = GitTier::new(self.git_root());
        let record = Task::new(Envelope::new(1, RecordKind::Task, at, actor()), TaskId::parse(task_id).unwrap(), title, status, None);
        git.write(&record).unwrap()
    }

    /// Write a `Handoff` record straight into the git tier (s21 P4
    /// `cross-tier-supersession` reader-migration fixtures) —
    /// `content_tag` varies the body so two writes at the SAME `id`
    /// produce distinct digests (a genuine second version, never a
    /// dedup no-op).
    pub fn plant_handoff_in_git(&self, id: &str, content_tag: &str, at: DateTime<Utc>) -> WriteReceipt {
        let git = GitTier::new(self.git_root());
        let body = HandoffBody { domain: DomainId::parse("기획").unwrap(), template_version: 1, fields: serde_json::json!({"tag": content_tag}) };
        let record =
            Handoff::new(Envelope::new(1, RecordKind::Handoff, at, actor()), HandoffId::parse(id).unwrap(), uuid::Uuid::new_v4(), None, 1, content_tag, None, body);
        git.write(&record).unwrap()
    }

    /// Write `.canon/plugins/<id>/plugin.yaml` under this fixture's
    /// project root (`resolve_plugin_snapshot::PLUGINS_DIR_RELATIVE_PATH`,
    /// s16 P1) -- `yaml` is pasted verbatim, mirroring `Fixture::new`'s
    /// own "caller supplies exactly the YAML body" convention.
    pub fn write_plugin_manifest(&self, id: &str, yaml: &str) {
        let dir = self.root.path().join(".canon/plugins").join(id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plugin.yaml"), yaml).unwrap();
    }

    /// Same, but written straight into the local r2 tier (simulating a
    /// record that has already aged there, without running `canon tier
    /// age` first).
    pub fn plant_trajectory_in_r2(&self, at: DateTime<Utc>, reward: f64) -> RunId {
        let r2 = canon_store::r2_tier::R2Tier::local(self.r2_root(), "canon/").unwrap();
        let run_id = RunId::new();
        let record = Trajectory::new(Envelope::new(1, RecordKind::Trajectory, at, actor()), run_id, None, None, None, None, Some(reward));
        r2.write(&record).unwrap();
        run_id
    }

    pub fn git_file_count(&self) -> usize {
        count_files(&self.git_root())
    }

    pub fn r2_file_count(&self) -> usize {
        count_files(&self.r2_root())
    }

    /// Run the built `canon` binary against this fixture's `canon.yaml`,
    /// with `CANON_R2_LOCAL_ROOT` bound to this fixture's local r2 root
    /// (the offline-r2 test seam `canon-cli`'s `tiers` module
    /// documents). `subcommand_args` is the full subcommand path plus
    /// any of its own flags (e.g. `["tier", "age", "--dry-run"]` or
    /// `["query", "--kind", "trajectory"]`) — `--canon-yaml` is a
    /// per-subcommand flag in `canon-cli`'s `clap` tree, so it is
    /// appended after the subcommand path, never before it.
    pub fn run_canon(&self, subcommand_args: &[&str]) -> Output {
        let bin = Path::new(env!("CARGO_BIN_EXE_canon"));
        let mut args: Vec<String> = subcommand_args.iter().map(|s| s.to_string()).collect();
        args.push("--canon-yaml".to_string());
        args.push(self.canon_yaml_path().display().to_string());
        Command::new(bin).args(&args).env("CANON_R2_LOCAL_ROOT", self.r2_root()).output().expect("spawning the built `canon` binary")
    }

    /// Same as [`Fixture::run_canon`], but resolves the `canon.yaml`
    /// through `--repo <fixture-root>` instead of an explicit
    /// `--canon-yaml` override (s26 `repo-flag-uniformity` D2/F4) --
    /// exercises the `resolve_repo_root(repo).join("canon.yaml")` path,
    /// never the `--canon-yaml`-bypass arm.
    pub fn run_canon_with_repo(&self, subcommand_args: &[&str]) -> Output {
        let bin = Path::new(env!("CARGO_BIN_EXE_canon"));
        let mut args: Vec<String> = subcommand_args.iter().map(|s| s.to_string()).collect();
        args.push("--repo".to_string());
        args.push(self.root.path().display().to_string());
        Command::new(bin).args(&args).env("CANON_R2_LOCAL_ROOT", self.r2_root()).output().expect("spawning the built `canon` binary")
    }
}

fn actor() -> Actor {
    Actor::new("test-agent", RoleId::parse("implementer").unwrap())
}

fn count_files(dir: &Path) -> usize {
    if !dir.exists() {
        return 0;
    }
    walkdir_count(dir)
}

fn walkdir_count(dir: &Path) -> usize {
    let mut count = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            count += walkdir_count(&entry.path());
        } else {
            count += 1;
        }
    }
    count
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
