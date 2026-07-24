//! End-to-end fixture corpus exercising the crate's PUBLIC API only
//! (`canon_learn::{...}`) — the store→distill→rebuild→search round
//! trip spec.md's "Store, distill, rebuild, search round-trip"
//! requirement names, and the write-time role-registry rejection
//! spec.md's "Role-namespaced trajectory store" requirement names.
//!
//! Every `VerdictRow` here is built as a plain struct literal — a
//! SYNTHETIC fixture, never routed through `canon_ingest::
//! artifact_adapter`/`derive_verdict` or any real ingest pipeline
//! (that production `canon ingest` artifact-driver is a deferred
//! residual outside this change's scope — see `src/lib.rs`'s module
//! doc "Out of this crate's scope").

use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
use canon_learn::{
    LearnConfig, LearnError, ParquetStrategyStore, ParquetTrajectoryStore, RoleRegistry, Trajectory, TrajectoryId,
    TrajectoryStore, rebuild_namespace, retrieve, store_trajectory,
};
use canon_model::ids::{RegimeKey, RoleId, regime_key};
use chrono::Utc;

fn dev_regime_key() -> RegimeKey {
    RegimeKey::parse(regime_key("dev", "repo", "auth-flow", "deadbeef")).unwrap()
}

fn synthetic_trajectory(task: &str, polarity: Polarity, becomes: Becomes) -> Trajectory {
    let verdict = VerdictRow { role: RoleId::parse("dev").unwrap(), polarity, becomes };
    Trajectory::new(TrajectoryId::new(), dev_regime_key(), task, format!("reasoning trace for: {task}"), vec![verdict], Utc::now(), vec![
        "fixture".to_string(),
    ])
    .unwrap()
}

/// A `canon.yaml`-shaped fixture manifest — proves `LearnConfig`
/// resolves an operator-local learn root + widens the role registry
/// from a real (fixture) config document, not just hard-coded defaults.
const FIXTURE_CANON_YAML: &str = "learn:\n  root: .canon/learn\n  roles:\n    - triage\n";

#[test]
fn store_distill_rebuild_search_round_trip_over_a_fixture_corpus() {
    let repo_root = tempfile::tempdir().unwrap();
    let config = LearnConfig::from_manifest(FIXTURE_CANON_YAML).unwrap();
    let registry = RoleRegistry::from_config(&config);

    let learn_root = repo_root.path().join(&config.root);
    let trajectory_store = ParquetTrajectoryStore::open(learn_root.join("trajectories"));
    let strategy_store = ParquetStrategyStore::open(learn_root.join("strategies"));

    // --- store: three synthetic trajectories under one regime_key ---
    let fixtures = vec![
        synthetic_trajectory("batch the parquet writes", Polarity::Success, Becomes::StrategyCandidate),
        synthetic_trajectory("skip the null check on join key", Polarity::Failure, Becomes::GuardrailCandidate),
        synthetic_trajectory("remediate the null-check gap", Polarity::Success, Becomes::StrategyCandidate),
    ];
    for trajectory in &fixtures {
        store_trajectory(&registry, &trajectory_store, trajectory).unwrap();
    }
    assert_eq!(trajectory_store.query_by_regime_key(&dev_regime_key()).unwrap().len(), 3);

    // --- distill + rebuild: strategy layer derived from raw trajectories ---
    let first_build = rebuild_namespace(&trajectory_store, &strategy_store, &dev_regime_key()).unwrap();
    assert_eq!(first_build.len(), 3, "one strategy item per trajectory's single verdict");

    // --- retrieve: the read side of the apply loop ---
    let retrieved = retrieve(&strategy_store, &dev_regime_key(), None).unwrap();
    let mut titles: Vec<&str> = retrieved.iter().map(|i| i.title.as_str()).collect();
    titles.sort_unstable();
    assert_eq!(titles, vec!["avoid: skip the null check on join key", "batch the parquet writes", "remediate the null-check gap"]);
    for item in &retrieved {
        assert_eq!(item.regime_key, dev_regime_key());
        assert_eq!(item.role, RoleId::parse("dev").unwrap());
    }

    // Raw trajectories are UNTOUCHED by distillation/rebuild — same
    // count, same content, still there.
    let raw_after = trajectory_store.query_by_regime_key(&dev_regime_key()).unwrap();
    assert_eq!(raw_after.len(), 3);
    for original in &fixtures {
        assert!(raw_after.contains(original), "raw trajectory {:?} must survive the distill/rebuild cycle untouched", original.id);
    }

    // --- rebuild again: non-destructive delete-rebuild, content-equivalent ---
    let second_build = rebuild_namespace(&trajectory_store, &strategy_store, &dev_regime_key()).unwrap();
    assert_eq!(second_build.len(), 3);
    let retrieved_after_rebuild = retrieve(&strategy_store, &dev_regime_key(), None).unwrap();
    let mut titles_after_rebuild: Vec<String> = retrieved_after_rebuild.iter().map(|i| i.title.clone()).collect();
    titles_after_rebuild.sort_unstable();
    assert_eq!(
        titles_after_rebuild,
        titles,
        "the strategy set found by search after rebuild matches the set found before, for the same source trajectories"
    );

    // A different regime_key never sees this namespace's trajectories
    // or strategies (similarity-search-scoping requirement).
    let content_regime = RegimeKey::parse(regime_key("content", "repo", "auth-flow", "deadbeef")).unwrap();
    assert!(trajectory_store.query_by_regime_key(&content_regime).unwrap().is_empty());
    assert!(retrieve(&strategy_store, &content_regime, None).unwrap().is_empty());
}

#[test]
fn an_unregistered_role_write_is_rejected_and_a_registered_extra_role_succeeds() {
    let config = LearnConfig::from_manifest(FIXTURE_CANON_YAML).unwrap();
    let registry = RoleRegistry::from_config(&config);
    let dir = tempfile::tempdir().unwrap();
    let trajectory_store = ParquetTrajectoryStore::open(dir.path());

    // "triage" was registered via the fixture manifest's `learn.roles:`.
    let triage_key = RegimeKey::parse(regime_key("triage", "repo", "ops", "cafebabe")).unwrap();
    let verdict = VerdictRow { role: RoleId::parse("triage").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
    let triage_trajectory = Trajectory::new(TrajectoryId::new(), triage_key, "task", "ctx", vec![verdict], Utc::now(), vec![]).unwrap();
    store_trajectory(&registry, &trajectory_store, &triage_trajectory).unwrap();

    // A role NOT built in and NOT in the fixture manifest is rejected.
    let unknown_key = RegimeKey::parse(regime_key("unknown-role", "repo", "ops", "cafebabe")).unwrap();
    let unknown_verdict =
        VerdictRow { role: RoleId::parse("unknown-role").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
    let unknown_trajectory =
        Trajectory::new(TrajectoryId::new(), unknown_key.clone(), "task", "ctx", vec![unknown_verdict], Utc::now(), vec![]).unwrap();
    let err = store_trajectory(&registry, &trajectory_store, &unknown_trajectory).unwrap_err();
    assert!(matches!(err, LearnError::UnregisteredRole(role) if role == "unknown-role"));
    assert!(trajectory_store.query_by_regime_key(&unknown_key).unwrap().is_empty(), "rejected write must not persist");
}
