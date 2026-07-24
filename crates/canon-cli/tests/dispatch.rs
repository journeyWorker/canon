//! Integration test for `canon dispatch begin` (S8 `retrieve-before-task`
//! tasks 4.1/4.2/4.3): the live run-manifest write seam, exercised
//! end-to-end against the real `canon` binary.
//!
//! - 4.1 retrieve -> dispatch -> manifest write: a seeded strategy is
//!   retrieved at dispatch time and recorded into the written
//!   `.canon/dispatch/<run_id>.json` manifest's `injected_guidance`.
//! - 4.2 replay is byte-identical after the store mutates: the recorded
//!   snapshot survives a later demotion verbatim
//!   (`manifest_guidance_for_replay`).
//! - 4.3 demoted-excluded-from-new-retrieval but present-in-old-manifest:
//!   after demotion a fresh `retrieve_guidance` drops the strategy, yet
//!   the already-written manifest still carries it.

use std::path::Path;
use std::process::Command;

use canon_learn::{
    demote_strategy, manifest_guidance_for_replay, retrieve_guidance, DemotionPolicy, ParquetStrategyStore, StrategyId, StrategyItem,
    StrategyStore, TrajectoryId,
};
use canon_model::ids::{RegimeKey, RoleId};
use canon_model::records::Run;
use chrono::Utc;

const REGIME: &str = "dev/canon/join-spine/9c93d024b1a2";

fn strategy_store(repo: &Path) -> ParquetStrategyStore {
    ParquetStrategyStore::open(repo.join(".canon").join("learn").join("strategies"))
}

fn seed(repo: &Path, content: &str) -> StrategyId {
    let store = strategy_store(repo);
    let id = StrategyId::new();
    let rk = RegimeKey::parse(REGIME).unwrap();
    let item = StrategyItem::new(id, rk, RoleId::parse("dev").unwrap(), "guidance", "one-liner", content, vec![TrajectoryId::new()], Utc::now());
    store.append(&item).unwrap();
    id
}

fn dispatch_begin(repo: &Path) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["dispatch", "begin", "--role", "dev", "--regime", REGIME, "--json", "--repo"])
        .arg(repo)
        .output()
        .expect("spawn canon dispatch begin");
    assert!(output.status.success(), "dispatch begin must exit 0; stderr: {}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).expect("--json emits a JSON summary object")
}

#[test]
fn dispatch_begin_records_retrieved_guidance_into_the_manifest() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), "always check Option before unwrap");

    let summary = dispatch_begin(dir.path());
    let manifest_path = summary["manifest"].as_str().expect("summary carries the manifest path");
    assert!(Path::new(manifest_path).exists(), "the dispatch manifest must be written to {manifest_path}");

    let guidance = summary["injected_guidance"].as_array().expect("injected_guidance is an array");
    assert_eq!(guidance.len(), 1, "the one seeded strategy is recorded: {summary}");
    assert_eq!(guidance[0]["content"], "always check Option before unwrap");

    // The written file itself deserializes as a Run carrying the same snapshot.
    let run: Run = serde_json::from_str(&std::fs::read_to_string(manifest_path).unwrap()).expect("the manifest is a valid Run");
    assert_eq!(run.injected_guidance.len(), 1);
}

#[test]
fn a_recorded_manifest_replays_verbatim_even_after_the_source_is_demoted() {
    let dir = tempfile::tempdir().unwrap();
    let id = seed(dir.path(), "prefer the boring, correct option");

    let summary = dispatch_begin(dir.path());
    let manifest_path = summary["manifest"].as_str().unwrap().to_string();
    let run: Run = serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    assert_eq!(run.injected_guidance.len(), 1, "guidance recorded at dispatch time");

    // Mutate the store AFTER the dispatch: demote the source strategy.
    let store = strategy_store(dir.path());
    let git_tier_root = dir.path().join(".canon").join("strategies");
    demote_strategy(&store, id, TrajectoryId::new(), &git_tier_root, DemotionPolicy::SOFT_FLAG).unwrap();

    // 4.3 new-retrieval half: a FRESH retrieval now excludes the demoted strategy.
    let fresh = retrieve_guidance(&store, &RoleId::parse("dev").unwrap(), &RegimeKey::parse(REGIME).unwrap(), None);
    assert!(fresh.is_empty(), "a fresh retrieval must exclude the demoted strategy, got {fresh:?}");

    // 4.2 + 4.3 old-manifest half: the recorded manifest still replays it verbatim.
    let replayed = manifest_guidance_for_replay(&run);
    assert_eq!(replayed, run.injected_guidance, "replay returns the recorded snapshot unchanged");
    assert_eq!(replayed.len(), 1, "the demotion never perturbs an already-written manifest");
    assert_eq!(replayed[0].content, "prefer the boring, correct option");
}

#[test]
fn a_role_regime_mismatch_is_a_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["dispatch", "begin", "--role", "reviewer", "--regime", REGIME, "--repo"])
        .arg(dir.path())
        .output()
        .expect("spawn canon dispatch begin");
    assert!(!output.status.success(), "a --role/--regime mismatch must be a nonzero usage error");
}
