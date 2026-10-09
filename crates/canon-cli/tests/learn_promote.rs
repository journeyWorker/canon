//! Integration test for `canon learn promote <strategy_id>` (S6
//! `role-strategy-memory` task group 4 / task 5.2, gate wiring s37
//! `execution-graph-topology`), run as a real subprocess against the
//! real `canon` binary: seed a distilled `StrategyItem` into the
//! operator-local parquet warm tier, then prove direct promotion is
//! refused without paired evaluation and verified approval, and that
//! promotion with a valid paired evaluation and an SSH-signed human
//! approval writes the git-tier file.
//!
//! # Why every promoting fixture now also seeds trajectories
//! S6 shipped `canon learn promote` before S7's statistical gates
//! existed, so the original version of this file seeded ONLY a
//! `StrategyItem` and asserted exit `0`. That encoded the gap S6's own
//! design named as temporary ("[Mitigation] S7's statistical-promotion
//! gate is the primary enforcement point"): promotion into the
//! git-tracked, PR-reviewed tier was reachable with zero corroborating
//! evidence. The gate is wired now, so a promoting fixture must supply
//! the evidence a promotion claims to rest on — direct activation
//! additionally requires paired evaluation and verified approval, and
//! the blocked cases below are the other half of that contract.

use std::path::Path;
use std::process::Command;

use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
use canon_learn::{
    ParquetStrategyStore, ParquetTrajectoryStore, PromotionApproval, PromotionEvaluation, StrategyId, StrategyItem, StrategyLifecycle,
    StrategyStore, Trajectory, TrajectoryId, TrajectoryStore, TrajectoryVerdict, VerdictOutcome,
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

/// Configures `.canon/policy.yaml` to pin an `allowed_signers` file
/// holding a freshly generated ed25519 key for principal `alice`, and
/// returns the private key path an external signer would use.
fn pin_approval_signer(repo: &Path) -> std::path::PathBuf {
    let canon_dir = repo.join(".canon");
    std::fs::create_dir_all(&canon_dir).unwrap();
    let private_key = repo.join("approval_ed25519");
    let generated = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "canon-learn-promote-test", "-f"])
        .arg(&private_key)
        .output()
        .expect("spawn ssh-keygen");
    assert!(generated.status.success(), "ssh-keygen key generation failed: {}", String::from_utf8_lossy(&generated.stderr));
    let public_key = std::fs::read_to_string(private_key.with_extension("pub")).unwrap();
    std::fs::write(canon_dir.join("allowed_signers"), format!("alice {public_key}")).unwrap();
    std::fs::write(canon_dir.join("policy.yaml"), "approval:\n  allowed_signers: .canon/allowed_signers\n").unwrap();
    private_key
}

fn ssh_sign(private_key: &Path, namespace: &str, payload: &[u8]) -> String {
    let payload_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(payload_file.path(), payload).unwrap();
    let output = Command::new("ssh-keygen").args(["-Y", "sign", "-f"]).arg(private_key).args(["-n", namespace]).arg(payload_file.path()).output().unwrap();
    assert!(output.status.success(), "ssh-keygen signing failed: {}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(format!("{}.sig", payload_file.path().display())).unwrap()
}

/// Writes a passing paired evaluation for `candidate` and a human
/// approval of it signed by `alice`, returning their file paths.
fn write_approved_bundle(repo: &Path, candidate: &StrategyItem, private_key: &Path) -> (std::path::PathBuf, std::path::PathBuf, PromotionEvaluation, PromotionApproval) {
    let evaluation = PromotionEvaluation::new(
        candidate,
        "candidate-v2",
        vec!["baseline-panel-0".to_string(), "baseline-panel-1".to_string()],
        vec!["candidate-panel-0".to_string(), "candidate-panel-1".to_string()],
        "corpus-v1",
        "paired-eval-v1",
        "context-digest-v1",
        "policy-digest-v1",
        "model-digest-v1",
        "tool-digest-v1",
        [("pairs".to_string(), 2.0), ("uplift".to_string(), 0.75), ("regressions".to_string(), 0.0)].into_iter().collect(),
        true,
        "promote",
    );
    let approved_at = Utc::now();
    let mut approval = PromotionApproval::new(candidate.id, evaluation.digest(), "alice", "human", true, approved_at);
    let payload = canon_model::approval_payload_bytes(
        "canon-learning-approval-v1",
        &format!("strategy:{}", candidate.id),
        None,
        &evaluation.digest(),
        None,
        &[],
        &[],
        "alice",
        &approved_at,
    );
    approval.signature = Some(ssh_sign(private_key, "canon-learning-approval-v1", &payload));
    approval.signer_key = Some("alice".to_string());
    approval.integrity_digest = approval.recomputed_integrity_digest();
    let evaluation_path = repo.join("evaluation.json");
    let approval_path = repo.join("approval.json");
    std::fs::write(&evaluation_path, serde_json::to_vec_pretty(&evaluation).unwrap()).unwrap();
    std::fs::write(&approval_path, serde_json::to_vec_pretty(&approval).unwrap()).unwrap();
    (evaluation_path, approval_path, evaluation, approval)
}

/// The scenario's whole contract through the real binary: a quarantined
/// candidate whose regime clears the gate is still refused (exit 1,
/// nothing written) without approval; with a valid paired evaluation and
/// a verified human approval the command succeeds, the git-tier file
/// appears carrying the strategy and both digests, and the warm-tier
/// lifecycle flips to active.
#[test]
fn promote_materializes_a_seeded_strategy_as_a_git_tier_file() {
    let dir = tempfile::tempdir().unwrap();
    let private_key = pin_approval_signer(dir.path());
    let store = ParquetStrategyStore::open(dir.path().join(".canon").join("learn").join("strategies"));
    let candidate = StrategyItem::new(
        StrategyId::new(),
        fixture_regime(),
        dev_role(),
        "review guidance",
        "one-liner",
        "prefer the boring, correct option",
        vec![TrajectoryId::new()],
        Utc::now(),
    )
    .with_lifecycle(StrategyLifecycle::Quarantined);
    store.append(&candidate).expect("seed strategy");
    let id = candidate.id;
    seed_proven_regime(dir.path());
    let written_at = git_tier_file(dir.path(), &id);

    let refused = run_promote(dir.path(), &id, &[]);
    assert_eq!(refused.status.code(), Some(1), "direct promotion without approval must be refused");
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains("strategy promotion requires a valid paired evaluation and verified human approval"),
        "stable refusal reason must be reported: {stderr}"
    );
    assert!(!written_at.exists(), "refused direct promotion must not write the git-tier file");

    let (evaluation_path, approval_path, evaluation, approval) = write_approved_bundle(dir.path(), &candidate, &private_key);
    let output = run_promote(dir.path(), &id, &["--evaluation", evaluation_path.to_str().unwrap(), "--approval", approval_path.to_str().unwrap()]);
    assert!(output.status.success(), "approved promotion must succeed; stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), format!("promoted {id} -> {}", written_at.display()));

    let written = std::fs::read_to_string(&written_at).expect("the git-tier file must exist after an approved promotion");
    assert!(written.starts_with("---\n"), "the file opens with front matter: {written}");
    assert!(written.contains("status: active"), "{written}");
    assert!(written.contains("prefer the boring, correct option"), "the body carries the strategy's content: {written}");
    assert!(written.contains(&format!("evaluation_digest: {}", evaluation.digest())), "{written}");
    assert!(written.contains(&format!("approval_digest: {}", approval.integrity_digest)), "{written}");
    assert_eq!(store.find_by_id(&id).unwrap().unwrap().lifecycle, Some(StrategyLifecycle::Active), "the warm-tier candidate is now active");
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
