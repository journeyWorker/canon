//! Integration test for `canon learn promote <strategy_id>` (S6
//! `role-strategy-memory` task group 4 / task 5.2, gate wiring s37
//! `execution-graph-topology`), run as a real subprocess against the
//! real `canon` binary: seed a distilled `StrategyItem` into the
//! operator-local parquet warm tier, then prove direct promotion is
//! refused without paired evaluation and verified approval.
//!
//! # Why every promoting fixture now also seeds trajectories
//! S6 shipped `canon learn promote` before S7's statistical gates
//! existed, so the original version of this file seeded ONLY a
//! `StrategyItem` and asserted exit `0`. That encoded the gap S6's own
//! design named as temporary ("[Mitigation] S7's statistical-promotion
//! gate is the primary enforcement point"): promotion into the
//! git-tracked, PR-reviewed tier was reachable with zero corroborating
//! evidence. The gate is wired now, so a promoting fixture must supply
//! the evidence a promotion claims to rest on — and direct activation
//! additionally requires paired evaluation and verified approval.
//!
//! # Why every promoting fixture now also seeds trajectories
//! S6 shipped `canon learn promote` before S7's statistical gates
//! existed, so the original version of this file seeded ONLY a
//! `StrategyItem` and asserted exit `0`. That encoded the gap S6's own
//! design named as temporary ("[Mitigation] S7's statistical-promotion
//! gate is the primary enforcement point"): promotion into the
//! git-tracked, PR-reviewed tier was reachable with zero corroborating
//! evidence. The gate is wired now, so a promoting fixture must supply
//! the evidence a promotion claims to rest on — and the blocked cases
//! below are the other half of that contract.

use std::path::Path;
use std::process::Command;

use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
use canon_learn::{
    ParquetStrategyStore, ParquetTrajectoryStore, StrategyId, StrategyItem, StrategyStore, Trajectory, TrajectoryId,
    TrajectoryStore, TrajectoryVerdict, VerdictOutcome,
};
use canon_model::ids::{regime_key, RegimeKey, RoleId};
use chrono::{Duration, Utc};

/// The single regime every fixture in this file shares — the strategy
/// and its corroborating trajectories MUST agree on it, because
/// `PromotionGate::evaluate` ignores samples from any other regime.
fn fixture_regime() -> RegimeKey {
    RegimeKey::parse(regime_key("dev", "canon", "join-spine", "9c93d024b1a2")).unwrap()
}

fn dev_role() -> RoleId {
    RoleId::parse("dev").unwrap()
}

fn seed_strategy(repo: &Path, content: &str) -> StrategyId {
    let store = ParquetStrategyStore::open(repo.join(".canon").join("learn").join("strategies"));
    let id = StrategyId::new();
    let item =
        StrategyItem::new(id, fixture_regime(), dev_role(), "review guidance", "one-liner", content, vec![TrajectoryId::new()], Utc::now());
    store.append(&item).expect("seed strategy");
    id
}

/// Appends `count` trajectories to [`fixture_regime`], each already
/// RESOLVED to `outcome` and spaced one day apart inside the gate's
/// trailing 30-day window. `outcome` is applied via
/// `Trajectory::with_verdict_record`, the documented seam for a fixture
/// that wants a pre-resolved trajectory without a separate
/// `mark_trajectory_verdict` round trip.
fn seed_trajectories(repo: &Path, count: usize, outcome: VerdictOutcome) {
    let store = ParquetTrajectoryStore::open(repo.join(".canon").join("learn").join("trajectories"));
    let now = Utc::now();
    for i in 0..count {
        // Chronological, most recent last — the occurrence gate folds
        // samples in time order, so ordering is load-bearing.
        let at = now - Duration::days((count - i) as i64);
        append_trajectory(&store, at, outcome);
    }
}

fn append_trajectory(store: &ParquetTrajectoryStore, at: chrono::DateTime<Utc>, outcome: VerdictOutcome) {
    let row = VerdictRow { role: dev_role(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
    let trajectory = Trajectory::new(TrajectoryId::new(), fixture_regime(), "landed the change", "fixture", vec![row], at, vec![])
        .expect("fixture trajectory is well formed");
    let trajectory = if outcome == VerdictOutcome::Pending {
        // `Pending` IS the constructor default; `with_verdict_record`
        // would be a no-op, and `mark_trajectory_verdict` rejects it.
        trajectory
    } else {
        trajectory.with_verdict_record(TrajectoryVerdict::new(outcome, 0.9))
    };
    store.append(&trajectory).expect("seed trajectory");
}

/// Enough corroborating successes to clear the conservative default gate
/// (`n_min: 5`, 30-day window).
fn seed_proven_regime(repo: &Path) {
    seed_trajectories(repo, 5, VerdictOutcome::Success);
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

fn git_tier_file(repo: &Path, id: &StrategyId) -> std::path::PathBuf {
    repo.join(".canon").join("strategies").join("dev").join(format!("{id}.md"))
}

#[test]
fn promote_materializes_a_seeded_strategy_as_a_git_tier_file() {
    let dir = tempfile::tempdir().unwrap();
    let id = seed_strategy(dir.path(), "prefer the boring, correct option");
    seed_proven_regime(dir.path());

    let output = run_promote(dir.path(), &id, &[]);
    assert!(!output.status.success(), "direct promotion without approval must be refused");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("strategy promotion requires a valid paired evaluation and verified human approval"),
        "stable refusal reason must be reported: {stderr}"
    );

    let written_at = git_tier_file(dir.path(), &id);
    assert!(!written_at.exists(), "refused direct promotion must not write the git-tier file");
}

#[test]
fn dry_run_previews_without_writing_the_git_tier_file() {
    let dir = tempfile::tempdir().unwrap();
    let id = seed_strategy(dir.path(), "no side effects on a dry run");
    seed_proven_regime(dir.path());

    let output = run_promote(dir.path(), &id, &["--dry-run"]);
    assert!(output.status.success(), "dry-run must exit 0; stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(!git_tier_file(dir.path(), &id).exists(), "--dry-run must NOT write the git-tier file");
    assert!(String::from_utf8_lossy(&output.stdout).contains("[dry-run]"), "dry-run output is labeled");
}

#[test]
fn an_unknown_strategy_id_fails_loud() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_promote(dir.path(), &StrategyId::new(), &[]);
    assert!(!output.status.success(), "an unknown strategy id must be a nonzero exit, not a silent no-op");
}

#[test]
fn an_unproven_strategy_is_blocked_by_the_promotion_gate() {
    let dir = tempfile::tempdir().unwrap();
    let id = seed_strategy(dir.path(), "unproven insight");
    // No trajectories at all: zero corroborating successes.

    let output = run_promote(dir.path(), &id, &[]);
    assert!(!output.status.success(), "an unproven strategy must not reach the git-tracked tier");
    assert!(!git_tier_file(dir.path(), &id).exists(), "a blocked promotion must write nothing");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("promotion gate"), "the refusal names the gate: {stderr}");
}

/// The exact state this repo was in before the flywheel was wired:
/// `canon ingest artifacts` created trajectories and never resolved
/// them, so every sample sat at `VerdictOutcome::Pending`. A `Pending`
/// sample neither corroborates nor contradicts, so a wall of them must
/// still be a refusal — not an accidental promotion.
#[test]
fn pending_trajectories_never_corroborate_a_promotion() {
    let dir = tempfile::tempdir().unwrap();
    let id = seed_strategy(dir.path(), "distilled from unresolved trajectories");
    seed_trajectories(dir.path(), 8, VerdictOutcome::Pending);

    let output = run_promote(dir.path(), &id, &[]);
    assert!(!output.status.success(), "pending samples must never satisfy the gate, however many there are");
    assert!(!git_tier_file(dir.path(), &id).exists(), "a blocked promotion must write nothing");
}

/// Proves the gate genuinely EVALUATES rather than counting rows: a
/// later contradiction resets the corroboration streak, so five
/// successes followed by one failure is not a promotion.
#[test]
fn a_later_contradiction_resets_the_streak_and_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let id = seed_strategy(dir.path(), "contradicted insight");
    seed_proven_regime(dir.path());
    let store = ParquetTrajectoryStore::open(dir.path().join(".canon").join("learn").join("trajectories"));
    append_trajectory(&store, Utc::now(), VerdictOutcome::Failure);

    let output = run_promote(dir.path(), &id, &[]);
    assert!(!output.status.success(), "a contradiction after the streak must block");
    assert!(!git_tier_file(dir.path(), &id).exists(), "a blocked promotion must write nothing");
}

/// A blocked `--dry-run` still shows the operator what WOULD be written
/// (that is what a preview is for) but keeps its exit code honest about
/// the refusal, and still writes nothing.
#[test]
fn a_blocked_dry_run_previews_then_reports_the_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let id = seed_strategy(dir.path(), "previewable but unproven");

    let output = run_promote(dir.path(), &id, &["--dry-run"]);
    assert!(!output.status.success(), "a blocked dry-run must report the refusal in its exit code");
    assert!(String::from_utf8_lossy(&output.stdout).contains("[dry-run]"), "the preview is still shown");
    assert!(String::from_utf8_lossy(&output.stderr).contains("promotion gate"), "and the refusal reason with it");
    assert!(!git_tier_file(dir.path(), &id).exists(), "--dry-run still writes nothing");
}

/// `canon.yaml`'s `learn.promotion.<role>` block is still parsed and
/// applied to the statistical gate, but it cannot bypass the separate
/// paired-evaluation and verified-approval activation boundary.
#[test]
fn the_per_role_promotion_config_in_canon_yaml_drives_the_gate() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), "learn:\n  promotion:\n    dev:\n      mode: occurrence\n      n_min: 1\n").unwrap();
    let id = seed_strategy(dir.path(), "proven by one corroborating success");
    seed_trajectories(dir.path(), 1, VerdictOutcome::Success);

    let output = run_promote(dir.path(), &id, &[]);
    assert!(!output.status.success(), "direct promotion must still require approval");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("strategy promotion requires a valid paired evaluation and verified human approval"),
        "stable refusal reason must be reported: {stderr}"
    );
    assert!(
        !git_tier_file(dir.path(), &id).exists(),
        "configured statistical gate must not bypass activation approval"
    );
}
