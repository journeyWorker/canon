//! S7 task-group-1 acceptance coverage exercising the crate's PUBLIC API
//! only (`canon_learn::{...}`) — dev reward formula fidelity, clamp
//! invariants, `mark_trajectory_verdict`'s Pending -> covering-verdict
//! write-back + persistence, and the default reward convention. Every
//! `VerdictRow` here is a SYNTHETIC fixture (plain struct literal),
//! never routed through a real ingest pipeline — mirrors
//! `fixture_round_trip.rs`'s own documented convention.

use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
use canon_learn::{
    ParquetTrajectoryStore, RewardRegistry, Trajectory, TrajectoryId, TrajectoryStore, TrajectoryVerdict, VerdictOutcome,
    mark_trajectory_verdict,
};
use canon_model::ids::{RegimeKey, RoleId, regime_key};
use chrono::Utc;

fn dev_regime() -> RegimeKey {
    RegimeKey::parse(regime_key("dev", "repo", "auth-flow", "deadbeef")).unwrap()
}

fn dev_trajectory(task: &str, polarity: Polarity, becomes: Becomes) -> Trajectory {
    let verdict = VerdictRow { role: RoleId::parse("dev").unwrap(), polarity, becomes };
    Trajectory::new(TrajectoryId::new(), dev_regime(), task, format!("reasoning trace for: {task}"), vec![verdict], Utc::now(), vec![
        "fixture".to_string(),
    ])
    .unwrap()
}

#[test]
fn dev_reward_formula_matches_the_compute_dev_reward_composite_shape() {
    let registry = RewardRegistry::builtin();

    // S4's stabilized VerdictRow only carries ONE positive dev shape
    // (Success, StrategyCandidate) — canon reads it as the fully
    // resolved positive triad (pr-merged + ci-pass + no-rollback, see
    // `reward.rs` module doc), matching the spec.md scenario "dev role
    // reward reflects PR/CI/rollback signals".
    let resolved = dev_trajectory("ship the fix", Polarity::Success, Becomes::StrategyCandidate);
    assert_eq!(registry.compute_for_trajectory(&resolved).unwrap(), (VerdictOutcome::Success, 1.0));

    // A dev failure (CI-fail/PR-revert shape) floors to 0.1, never the
    // default-convention 0.3 — the dev role's OWN weighted formula
    // overrides the generic default.
    let failed = dev_trajectory("break the build", Polarity::Failure, Becomes::GuardrailCandidate);
    assert_eq!(registry.compute_for_trajectory(&failed).unwrap(), (VerdictOutcome::Failure, 0.1));
}

#[test]
fn a_non_dev_role_uses_its_own_registered_function_never_devs_weights() {
    let registry = RewardRegistry::builtin();
    let content_role = RoleId::parse("content").unwrap();
    let row = VerdictRow { role: content_role.clone(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
    // content has no PR/CI-weighted formula — it gets the provisional
    // default-convention reward (0.9), not dev's 1.0-via-triad value
    // (spec.md "A non-dev role never uses the dev weight formula").
    assert_eq!(registry.compute(&content_role, &[row]), (VerdictOutcome::Success, 0.9));
}

#[test]
fn reward_clamp_holds_even_for_a_hand_constructed_out_of_range_value() {
    let too_high = TrajectoryVerdict::new(VerdictOutcome::Success, 42.0);
    assert_eq!(too_high.reward, 1.0);
    let too_low = TrajectoryVerdict::new(VerdictOutcome::Failure, -42.0);
    assert_eq!(too_low.reward, 0.0);
    let not_a_number = TrajectoryVerdict::new(VerdictOutcome::Pending, f64::NAN);
    assert!((0.0..=1.0).contains(&not_a_number.reward), "NaN must never escape the [0,1] invariant, got {}", not_a_number.reward);
}

#[test]
fn mark_trajectory_verdict_flips_pending_to_the_covering_outcome_and_persists_the_reward() {
    let dir = tempfile::tempdir().unwrap();
    let store = ParquetTrajectoryStore::open(dir.path());
    let registry = RewardRegistry::builtin();

    let t = dev_trajectory("land the PR", Polarity::Success, Becomes::StrategyCandidate);
    store.append(&t).unwrap();

    // Freshly stored: default convention, Pending at 0.5.
    let before = store.find_by_id(&t.id).unwrap().unwrap();
    assert_eq!(before.verdict_record, TrajectoryVerdict::new(VerdictOutcome::Pending, 0.5));

    // A covering verdict arrives — compute via the registry, then write back.
    let (outcome, reward) = registry.compute_for_trajectory(&t).unwrap();
    mark_trajectory_verdict(&store, &t.id, outcome, reward).unwrap();

    let after = store.find_by_id(&t.id).unwrap().unwrap();
    assert_ne!(after.verdict_record.outcome, VerdictOutcome::Pending, "must never stay Pending once a covering verdict arrives");
    assert_eq!(after.verdict_record, TrajectoryVerdict::new(VerdictOutcome::Success, 1.0));
}

#[test]
fn default_reward_convention_applies_for_a_role_with_no_dev_style_weighted_formula() {
    let dir = tempfile::tempdir().unwrap();
    let store = ParquetTrajectoryStore::open(dir.path());
    let registry = RewardRegistry::builtin();

    let regime = RegimeKey::parse(regime_key("planning", "repo", "roadmap", "cafebabe")).unwrap();
    let verdict = VerdictRow { role: RoleId::parse("planning").unwrap(), polarity: Polarity::Failure, becomes: Becomes::GuardrailCandidate };
    let t = Trajectory::new(TrajectoryId::new(), regime, "plan the sprint", "reasoning", vec![verdict], Utc::now(), vec![]).unwrap();
    store.append(&t).unwrap();

    let (outcome, reward) = registry.compute_for_trajectory(&t).unwrap();
    // The default convention (design D2): 0.3 for failure.
    assert_eq!((outcome, reward), (VerdictOutcome::Failure, 0.3));

    mark_trajectory_verdict(&store, &t.id, outcome, reward).unwrap();
    let after = store.find_by_id(&t.id).unwrap().unwrap();
    assert_eq!(after.verdict_record, TrajectoryVerdict::new(VerdictOutcome::Failure, 0.3));
}
