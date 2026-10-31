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
    demote_strategy, manifest_guidance_for_replay, DemotionPolicy, ParquetStrategyStore, StrategyId, StrategyItem, StrategyItemFields,
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
    let item = StrategyItem::new(StrategyItemFields { id, regime_key: rk, role: RoleId::parse("dev").unwrap(), title: "guidance".into(), description: "one-liner".into(), content: content.into(), source_trajectory_ids: vec![TrajectoryId::new()], recorded_at: Utc::now() });
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
    let body: serde_json::Value = serde_json::from_slice(&output.stdout).expect("--json emits one JSON document");
    // 0.21: the summary is `data` of the `{ data, meta }` envelope, and a write names its next step.
    assert_eq!(body["meta"]["command"], "dispatch begin", "{body}");
    assert!(body["meta"]["next"].is_array(), "a write carries meta.next: {body}");
    assert!(body.get("run_id").is_none(), "the old top-level keys are gone: {body}");
    body["data"].clone()
}

#[test]
fn a_consumer_reads_the_run_id_from_data_and_closes_that_run() {
    let dir = tempfile::tempdir().unwrap();
    let begun = dispatch_begin(dir.path());
    let run_id = begun["runId"].as_str().expect("`.data.run_id` is the run id a hook closes later").to_string();

    let output = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["dispatch", "end", &run_id, "--run-outcome", "succeeded", "--json", "--repo"])
        .arg(dir.path())
        .output()
        .expect("spawn canon dispatch end");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let ended: serde_json::Value = serde_json::from_slice(&output.stdout).expect("one JSON document");
    assert_eq!(ended["meta"]["command"], "dispatch end", "{ended}");
    assert!(ended["meta"]["next"].is_array(), "{ended}");
    assert_eq!(ended["data"]["runId"], run_id.as_str(), "{ended}");
    assert_eq!(ended["data"]["status"], "succeeded", "{ended}");
}

#[test]
fn dispatch_begin_records_retrieved_guidance_into_the_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let id = seed(dir.path(), "always check Option before unwrap");

    let summary = dispatch_begin(dir.path());
    let manifest_path = summary["manifest"].as_str().expect("summary carries the manifest path");
    assert!(Path::new(manifest_path).exists(), "the dispatch manifest must be written to {manifest_path}");

    let guidance = summary["injectedGuidance"].as_array().expect("injectedGuidance is an array");
    assert_eq!(guidance.len(), 1, "the one seeded strategy is recorded: {summary}");
    assert_eq!(guidance[0]["strategy_id"], id.to_string(), "the injected guidance is the seeded strategy");
    assert_eq!(guidance[0]["title"], "guidance");
    assert_eq!(guidance[0]["content"], "always check Option before unwrap");

    // The written file itself deserializes as a Run carrying the SAME
    // snapshot the summary reported — identity and content, not a count.
    let run: Run = serde_json::from_str(&std::fs::read_to_string(manifest_path).unwrap()).expect("the manifest is a valid Run");
    assert_eq!(serde_json::to_value(&run.injected_guidance).unwrap(), summary["injectedGuidance"], "the on-disk guidance must equal the injected summary snapshot");
    assert_eq!(run.run_id.to_string(), summary["runId"].as_str().expect("summary carries the run id"), "the manifest is the reported run");
}

#[test]
fn a_recorded_manifest_replays_verbatim_even_after_the_source_is_demoted() {
    let dir = tempfile::tempdir().unwrap();
    let id = seed(dir.path(), "prefer the boring, correct option");

    let summary = dispatch_begin(dir.path());
    let manifest_path = summary["manifest"].as_str().unwrap().to_string();
    let before_bytes = std::fs::read(&manifest_path).unwrap();
    let run: Run = serde_json::from_slice(&before_bytes).unwrap();
    assert_eq!(run.injected_guidance.len(), 1, "guidance recorded at dispatch time");

    // Before the demotion the CLI retrieval returns the strategy.
    let retrieve = |repo: &Path| -> Vec<serde_json::Value> {
        let output = Command::new(env!("CARGO_BIN_EXE_canon"))
            .args(["retrieve", "--for-role", "dev", "--regime", REGIME, "--json", "--repo"])
            .arg(repo)
            .output()
            .expect("spawn canon retrieve");
        assert!(output.status.success(), "canon retrieve must exit 0; stderr: {}", String::from_utf8_lossy(&output.stderr));
        let body: serde_json::Value = serde_json::from_slice(&output.stdout).expect("one JSON document");
        body["data"].as_array().expect("data is the guidance array").clone()
    };
    assert_eq!(retrieve(dir.path()).len(), 1, "before the demotion the live store returns the strategy");

    // Mutate the store AFTER the dispatch: demote the source strategy.
    let store = strategy_store(dir.path());
    let git_tier_root = dir.path().join(".canon").join("strategies");
    demote_strategy(&store, id, TrajectoryId::new(), &git_tier_root, DemotionPolicy::SOFT_FLAG).unwrap();

    // 4.3 new-retrieval half: a FRESH retrieval through the CLI now excludes the demoted strategy.
    let fresh = retrieve(dir.path());
    assert!(fresh.is_empty(), "a fresh `canon retrieve` must exclude the demoted strategy, got {fresh:?}");

    // 4.2 + 4.3 old-manifest half: re-read the manifest from disk AFTER the demotion.
    let after_bytes = std::fs::read(&manifest_path).unwrap();
    assert_eq!(after_bytes, before_bytes, "the demotion never perturbs a manifest already written (byte-identical on disk)");
    let reread: Run = serde_json::from_slice(&after_bytes).unwrap();
    let replayed = manifest_guidance_for_replay(&reread);
    assert_eq!(replayed, run.injected_guidance, "replay of the re-read manifest returns the recorded snapshot unchanged");
    assert_eq!(serde_json::to_value(&replayed).unwrap(), summary["injectedGuidance"], "and it equals what dispatch reported");
    assert_eq!(replayed.len(), 1, "still one item");
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
