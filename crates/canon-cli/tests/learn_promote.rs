//! Integration test for `canon learn promote <strategy_id>` (S6
//! `role-strategy-memory` task group 4 / task 5.2), run as a real
//! subprocess against the real `canon` binary: seed a distilled
//! `StrategyItem` into the operator-local parquet warm tier, then prove
//! `canon learn promote` materializes it as a git-tier
//! `.canon/strategies/<role>/<id>.md` file (and that `--dry-run` writes
//! nothing).

use std::path::Path;
use std::process::Command;

use canon_learn::{ParquetStrategyStore, StrategyId, StrategyItem, StrategyStore, TrajectoryId};
use canon_model::ids::{regime_key, RegimeKey, RoleId};
use chrono::Utc;

fn seed_strategy(repo: &Path, content: &str) -> StrategyId {
    let store = ParquetStrategyStore::open(repo.join(".canon").join("learn").join("strategies"));
    let id = StrategyId::new();
    let rk = RegimeKey::parse(regime_key("dev", "canon", "join-spine", "9c93d024b1a2")).unwrap();
    let item = StrategyItem::new(id, rk, RoleId::parse("dev").unwrap(), "review guidance", "one-liner", content, vec![TrajectoryId::new()], Utc::now());
    store.append(&item).expect("seed strategy");
    id
}

fn run_promote(repo: &Path, id: &StrategyId, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_canon"))
        .arg("learn")
        .arg("promote")
        .arg(id.to_string())
        .arg("--repo")
        .arg(repo)
        .args(extra)
        .output()
        .expect("spawn canon learn promote")
}

#[test]
fn promote_materializes_a_seeded_strategy_as_a_git_tier_file() {
    let dir = tempfile::tempdir().unwrap();
    let id = seed_strategy(dir.path(), "prefer the boring, correct option");

    let output = run_promote(dir.path(), &id, &[]);
    assert!(output.status.success(), "promote must exit 0; stderr: {}", String::from_utf8_lossy(&output.stderr));

    let git_tier_file = dir.path().join(".canon").join("strategies").join("dev").join(format!("{id}.md"));
    assert!(git_tier_file.exists(), "promote must write the git-tier file at {}", git_tier_file.display());
    let written = std::fs::read_to_string(&git_tier_file).unwrap();
    assert!(written.starts_with("---\n"), "front-matter opener");
    assert!(written.contains("status: active"), "opens active");
    assert!(written.contains("prefer the boring, correct option"), "body carries the strategy content");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("promoted") && stdout.contains(&id.to_string()), "reports the promotion: {stdout}");
}

#[test]
fn dry_run_previews_without_writing_the_git_tier_file() {
    let dir = tempfile::tempdir().unwrap();
    let id = seed_strategy(dir.path(), "no side effects on a dry run");

    let output = run_promote(dir.path(), &id, &["--dry-run"]);
    assert!(output.status.success(), "dry-run must exit 0; stderr: {}", String::from_utf8_lossy(&output.stderr));

    let git_tier_file = dir.path().join(".canon").join("strategies").join("dev").join(format!("{id}.md"));
    assert!(!git_tier_file.exists(), "--dry-run must NOT write the git-tier file");
    assert!(String::from_utf8_lossy(&output.stdout).contains("[dry-run]"), "dry-run output is labeled");
}

#[test]
fn an_unknown_strategy_id_fails_loud() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_promote(dir.path(), &StrategyId::new(), &[]);
    assert!(!output.status.success(), "an unknown strategy id must be a nonzero exit, not a silent no-op");
}
