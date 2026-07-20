//! Golden fixture verdict streams for the occurrence half of S7 task
//! group 6 (fixtures + selftest), exercising the crate's PUBLIC API
//! only (`canon_learn::{...}`) — mirrors `fixture_round_trip.rs`'s own
//! "every `VerdictRow` here is a SYNTHETIC fixture" convention, never
//! routed through a real ingest pipeline.
//!
//! - 6.1: a stream that PROMOTES (`n_min` corroborating successes, zero
//!   contradictions) — [`a_stream_of_n_min_successes_with_zero_contradictions_promotes`].
//! - 6.2: streams that REJECT (below `n_min`; `n_min` successes but a
//!   contradicting failure inside the window resets the count) —
//!   [`a_stream_below_n_min_rejects`],
//!   [`a_stream_with_a_contradicting_failure_inside_the_window_rejects`].
//! - 6.3: a contradicting trajectory arrives AFTER promotion —
//!   `demote_strategy` fires and the git-tier file is soft-flagged —
//!   [`a_contradicting_trajectory_after_promotion_demotes_the_strategy_and_soft_flags_its_git_tier_file`].

use std::fs;

use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
use canon_learn::{
    DemotionPolicy, LearnConfig, OccurrencePromotionGate, ParquetStrategyStore, ParquetTrajectoryStore, PromotionGate, StrategyId,
    StrategyItem, StrategyStore, Trajectory, TrajectoryId, TrajectoryStore, TrajectoryVerdict, VerdictOutcome, demote_strategy,
};
use canon_model::ids::{RegimeKey, RoleId, regime_key};
use chrono::{DateTime, Duration, Utc};

fn dev_regime() -> RegimeKey {
    RegimeKey::parse(regime_key("dev", "repo", "occurrence-fixture", "deadbeef")).unwrap()
}

/// A synthetic, already-resolved `dev` trajectory — one `Success`/
/// `Failure` `VerdictRow` (S4's shape) folded into an S7-level
/// `verdict_record` at `recorded_at` (`Trajectory::new` always seeds
/// `Pending`; `with_verdict_record` overrides it, the same fixture
/// convention `reward_core.rs`/`mark_verdict.rs`'s own tests use).
fn resolved_trajectory(outcome: VerdictOutcome, recorded_at: DateTime<Utc>) -> Trajectory {
    let polarity = if outcome == VerdictOutcome::Success { Polarity::Success } else { Polarity::Failure };
    let verdict = VerdictRow { role: RoleId::parse("dev").unwrap(), polarity, becomes: Becomes::StrategyCandidate };
    Trajectory::new(TrajectoryId::new(), dev_regime(), "task", "reasoning trace", vec![verdict], recorded_at, vec!["fixture".to_string()])
        .unwrap()
        .with_verdict_record(TrajectoryVerdict::new(outcome, outcome.default_reward()))
}

/// The occurrence gate's conservative default config
/// (`LearnConfig::promotion_config_for` when a role has no explicit
/// `promotion.<role>` entry) — `n_min: 5`, a 30-day window — proven
/// end-to-end via a real `LearnConfig::from_manifest("")` parse, not a
/// hand-built `OccurrencePromotionGate::new` bypassing the config path.
fn default_gate_for_dev() -> OccurrencePromotionGate {
    let config = LearnConfig::from_manifest("").unwrap();
    let role = RoleId::parse("dev").unwrap();
    OccurrencePromotionGate::from_config(config.promotion_config_for(&role))
}

/// **6.1**: `n_min` (5, the conservative default) corroborating
/// `Success`-verdict trajectories for the SAME `regime_key`, zero
/// `Failure`-verdict trajectories in the window — promotes.
#[test]
fn a_stream_of_n_min_successes_with_zero_contradictions_promotes() {
    let gate = default_gate_for_dev();
    let now = Utc::now();
    let stream: Vec<Trajectory> = (0..5).map(|i| resolved_trajectory(VerdictOutcome::Success, now - Duration::days(5 - i))).collect();

    let decision = gate.evaluate(&dev_regime(), &stream, now);
    assert!(decision.is_promote(), "expected promotion, got {decision:?}");
}

/// **6.2a**: below `n_min` — 4 successes, one short of the default
/// `n_min: 5` — rejects.
#[test]
fn a_stream_below_n_min_rejects() {
    let gate = default_gate_for_dev();
    let now = Utc::now();
    let stream: Vec<Trajectory> = (0..4).map(|i| resolved_trajectory(VerdictOutcome::Success, now - Duration::days(4 - i))).collect();

    let decision = gate.evaluate(&dev_regime(), &stream, now);
    assert!(!decision.is_promote(), "expected rejection, got {decision:?}");
}

/// **6.2b**: 5 successes accumulate, but a `Failure`-verdict trajectory
/// lands INSIDE the window before the count would otherwise clear —
/// the contradiction RESETS the counter (never averaged away), so the
/// stream stays rejected even though 5 total successes were recorded.
#[test]
fn a_stream_with_a_contradicting_failure_inside_the_window_rejects() {
    let gate = default_gate_for_dev();
    let now = Utc::now();
    let mut stream: Vec<Trajectory> = (0..5).map(|i| resolved_trajectory(VerdictOutcome::Success, now - Duration::days(10 - i))).collect();
    // The contradiction arrives after the fifth success but is still
    // the most RECENT event in the window — it resets the streak to 0.
    stream.push(resolved_trajectory(VerdictOutcome::Failure, now - Duration::days(1)));

    let decision = gate.evaluate(&dev_regime(), &stream, now);
    assert!(!decision.is_promote(), "a contradicting failure must reset promotion eligibility, got {decision:?}");
}

/// **6.3**: a strategy already promoted for a regime (n_min
/// corroborating successes, zero contradictions — the SAME shape 6.1
/// proves eligible) later collects a contradicting `Failure`-verdict
/// trajectory. Asserts BOTH halves of design D4's contract:
/// - re-evaluating the gate over the UPDATED sample set now rejects
///   (the contradiction resets the streak, proving demotion is
///   warranted, not just assumed);
/// - `demote_strategy` fires: durable evidence lands on the
///   `StrategyStore` row AND the git-tier `<role>/<id>.md` file is
///   soft-flagged (`status: demoted` front-matter + reason), its OTHER
///   front matter and body left byte-unchanged (append-only, §7).
#[test]
fn a_contradicting_trajectory_after_promotion_demotes_the_strategy_and_soft_flags_its_git_tier_file() {
    let repo_root = tempfile::tempdir().unwrap();
    let strategy_store = ParquetStrategyStore::open(repo_root.path().join("canon/learn/strategies"));
    let trajectory_store = ParquetTrajectoryStore::open(repo_root.path().join("canon/learn/trajectories"));
    let git_tier_root = repo_root.path().join("canon/strategies");

    let gate = OccurrencePromotionGate::new(3, Duration::days(30));
    let now = Utc::now();

    // --- build eligibility: n_min successes, zero contradictions ---
    let mut stream: Vec<Trajectory> = (0..3).map(|i| resolved_trajectory(VerdictOutcome::Success, now - Duration::days(5 - i))).collect();
    for t in &stream {
        trajectory_store.append(t).unwrap();
    }
    let eligible = gate.evaluate(&dev_regime(), &stream, now);
    assert!(eligible.is_promote(), "fixture setup must reach promotion eligibility first, got {eligible:?}");

    // --- simulate `canon learn promote`: a StrategyItem + its git-tier
    // file exist for this regime (that CLI command is unbuilt — S6
    // task 4.1 / lib.rs module doc — so the fixture stands in for its
    // output directly: a StrategyItem row + a front-matter `.md` file,
    // the exact shape `canon learn promote` would have produced) ---
    let strategy = StrategyItem::new(
        StrategyId::new(),
        dev_regime(),
        RoleId::parse("dev").unwrap(),
        "batch the parquet writes",
        "avoids one fsync per row",
        "buffer writes and flush once per namespace",
        stream.iter().map(|t| t.id).collect(),
        now,
    );
    strategy_store.append(&strategy).unwrap();

    let role_dir = git_tier_root.join("dev");
    fs::create_dir_all(&role_dir).unwrap();
    let git_tier_file = role_dir.join(format!("{}.md", strategy.id));
    fs::write(
        &git_tier_file,
        "---\ntitle: batch the parquet writes\ndescription: avoids one fsync per row\n---\nbuffer writes and flush once per namespace\n",
    )
    .unwrap();

    // --- a contradicting failure arrives AFTER promotion ---
    let contradicting = resolved_trajectory(VerdictOutcome::Failure, now - Duration::days(1));
    trajectory_store.append(&contradicting).unwrap();
    stream.push(contradicting.clone());

    let after_contradiction = gate.evaluate(&dev_regime(), &stream, now);
    assert!(!after_contradiction.is_promote(), "the contradicting failure must flip the gate to reject, got {after_contradiction:?}");

    // --- demote_strategy fires ---
    let record = demote_strategy(&strategy_store, strategy.id, contradicting.id, &git_tier_root, DemotionPolicy::default()).unwrap();
    assert_eq!(record.strategy_id, strategy.id);
    assert_eq!(record.contradicting_trajectory_id, contradicting.id);

    // Durable evidence: the StrategyStore row itself now carries it.
    let reloaded = strategy_store.find_by_id(&strategy.id).unwrap().unwrap();
    let evidence = reloaded.demotion.expect("demote_strategy must persist demotion evidence onto the StrategyItem row");
    assert_eq!(evidence.contradicting_trajectory_id, contradicting.id);

    // Git-tier file: soft-flagged, other front matter + body untouched.
    let updated = fs::read_to_string(&git_tier_file).unwrap();
    assert!(updated.contains("status: demoted"), "git-tier file must carry `status: demoted`:\n{updated}");
    assert!(updated.contains("reason:"), "git-tier file must carry a `reason`:\n{updated}");
    assert!(updated.contains("title: batch the parquet writes"), "unrelated front matter must survive byte-unchanged:\n{updated}");
    assert!(updated.contains("buffer writes and flush once per namespace"), "the body must survive byte-unchanged:\n{updated}");
}
