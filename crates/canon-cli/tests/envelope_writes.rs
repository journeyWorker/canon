//! The write commands that printed their own JSON shape answer `{ data, meta }`
//! like every other (0.21): `export`, `learn approve` and `learn rollback`
//! (`purge`, `dispatch`, `ingest` are covered where their other behavior is).
//! A write names what follows in `meta.next`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
use canon_learn::{
    promote_strategy_approved, rebuild_namespace, ParquetStrategyStore, ParquetTrajectoryStore, PromotionApproval, PromotionEvaluation, StrategyItem, Trajectory,
    TrajectoryId, TrajectoryStore,
};
use canon_model::approval::approval_payload_bytes;
use canon_model::ids::{regime_key, RegimeKey, RoleId};
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).env("CANON_ACTOR", "envelope-writes").output().expect("spawn canon binary")
}

fn body(out: &Output) -> Value {
    assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

#[test]
fn export_json_is_the_manifest_as_data_with_its_page_in_meta_and_a_next_step() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    for args in [&["init", "--providers", "none"][..], &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"][..]] {
        assert!(canon(repo, args).status.success());
    }
    let out = canon(repo, &["export", "--kind", "evidence", "--json"]);
    let shown = body(&out);
    assert_eq!(shown["meta"]["command"], "export", "{shown}");
    assert!(shown["meta"]["next"].is_array(), "a write carries meta.next: {shown}");
    assert_eq!(shown["data"]["schema"], "canon.retention.export", "{shown}");
    assert!(shown["data"]["records"].is_array(), "{shown}");
    assert!(shown["data"].get("meta").is_none(), "the page state moved to meta: {shown}");
    assert!(shown["meta"]["total"].is_number() && shown["meta"]["returned"].is_number(), "{shown}");
    assert!(shown.get("records").is_none(), "the old top-level keys are gone: {shown}");

    // With --out the manifest is the file; stdout names it.
    let file = repo.join("manifest.json");
    let written = body(&canon(repo, &["export", "--kind", "evidence", "--out", file.to_str().unwrap(), "--json"]));
    assert_eq!(written["data"]["out"], file.display().to_string(), "{written}");
    let manifest: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    assert_eq!(manifest["schema"], "canon.retention.export", "the file is the manifest itself");
    assert_eq!(written["data"]["recordCount"], manifest["record_count"], "{written}");
}

struct Signing {
    repo: tempfile::TempDir,
    private_key: PathBuf,
}

impl Signing {
    fn new() -> Self {
        let repo = tempfile::tempdir().unwrap();
        let canon_dir = repo.path().join(".canon");
        fs::create_dir_all(&canon_dir).unwrap();
        let private_key = repo.path().join("approval_ed25519");
        let generated = Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-C", "canon-learning-test", "-f"]).arg(&private_key).output().expect("ssh-keygen");
        assert!(generated.status.success(), "{}", String::from_utf8_lossy(&generated.stderr));
        let public_key = fs::read_to_string(private_key.with_extension("pub")).unwrap();
        fs::write(canon_dir.join("allowed_signers"), format!("alice {public_key}")).unwrap();
        fs::write(canon_dir.join("policy.yaml"), "approval:\n  allowed_signers: .canon/allowed_signers\n").unwrap();
        Self { repo, private_key }
    }

    fn sign(&self, payload: &[u8]) -> String {
        let payload_file = tempfile::NamedTempFile::new().unwrap();
        fs::write(payload_file.path(), payload).unwrap();
        let output = Command::new("ssh-keygen").args(["-Y", "sign", "-f"]).arg(&self.private_key).args(["-n", "canon-learning-approval-v1"]).arg(payload_file.path()).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(fs::read(format!("{}.sig", payload_file.path().display())).unwrap()).unwrap()
    }
}

fn regime() -> RegimeKey {
    RegimeKey::parse(regime_key("dev", "repo", "envelope-writes", "deadbeef")).unwrap()
}

/// A quarantined candidate strategy distilled from one trajectory, and the evaluation that passes it.
fn candidate_and_evaluation(repo: &Path) -> (StrategyItem, PromotionEvaluation) {
    let trajectories = ParquetTrajectoryStore::open(repo.join(".canon/learn/trajectories"));
    let strategies = ParquetStrategyStore::open(repo.join(".canon/learn/strategies"));
    let trajectory = Trajectory::new(
        TrajectoryId::new(),
        regime(),
        "batch writes",
        "buffer the paired writes before flushing",
        vec![VerdictRow { role: RoleId::parse("dev").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate }],
        Utc::now(),
        vec!["fixture".to_string()],
    )
    .unwrap();
    trajectories.append(&trajectory).unwrap();
    let candidate = rebuild_namespace(&trajectories, &strategies, &regime()).unwrap().into_iter().next().expect("one candidate");
    let digest = |variant: &str, panel: usize, score: f64| format!("{:x}", Sha256::digest(format!("{variant}|panel={panel}|score={score:.3}").as_bytes()));
    let evaluation = PromotionEvaluation::new(
        &candidate,
        "candidate-v2",
        vec![digest("baseline", 0, 0.50), digest("baseline", 1, 0.54)],
        vec![digest("candidate", 0, 0.91), digest("candidate", 1, 0.88)],
        "corpus-v1",
        "paired-eval-v1",
        "context-digest-v1",
        "policy-digest-v1",
        "model-digest-v1",
        "tool-digest-v1",
        BTreeMap::from([("pairs".to_string(), 2.0), ("uplift".to_string(), 0.75), ("regressions".to_string(), 0.0)]).into_iter().collect(),
        true,
        "promote",
    );
    (candidate, evaluation)
}

#[test]
fn learn_approve_json_is_the_payload_as_data_and_the_signing_is_a_human_step() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    let (candidate, evaluation) = candidate_and_evaluation(repo);
    fs::write(repo.join("evaluation.json"), serde_json::to_vec(&evaluation).unwrap()).unwrap();

    let out = canon(repo, &["learn", "approve", &candidate.id.to_string(), "--evaluation", "evaluation.json", "--principal", "alice", "--repo", ".", "--json"]);
    let shown = body(&out);
    assert_eq!(shown["meta"]["command"], "learn approve", "{shown}");
    assert_eq!(shown["data"]["candidateStrategyId"], candidate.id.to_string(), "{shown}");
    assert!(shown["data"]["payloadHex"].as_str().is_some_and(|hex| !hex.is_empty()), "{shown}");
    let step = &shown["meta"]["next"][0];
    assert_eq!(step["actor"], "human", "signing is the human's: {shown}");
    assert!(step["command"].as_str().unwrap().contains("canon learn promote"), "{shown}");
    assert!(shown.get("payloadHex").is_none(), "the old top-level keys are gone: {shown}");

    // The human form stays the bare payload a signer reads.
    let human = canon(repo, &["learn", "approve", &candidate.id.to_string(), "--evaluation", "evaluation.json", "--principal", "alice", "--repo", "."]);
    let payload: Value = serde_json::from_slice(&human.stdout).expect("the payload");
    assert!(payload["payloadHex"].is_string(), "{payload}");
}

#[test]
fn learn_rollback_json_is_the_record_as_data_with_a_next_step() {
    let signing = Signing::new();
    let repo = signing.repo.path();
    let (candidate, evaluation) = candidate_and_evaluation(repo);
    let strategies = ParquetStrategyStore::open(repo.join(".canon/learn/strategies"));
    let approved_at = Utc::now();
    let mut approval = PromotionApproval::new(candidate.id, evaluation.digest(), "alice", "human", true, approved_at);
    let payload = approval_payload_bytes("canon-learning-approval-v1", &format!("strategy:{}", candidate.id), None, &evaluation.digest(), None, &[], &[], "alice", &approved_at);
    approval.signature = Some(signing.sign(&payload));
    approval.signer_key = Some("alice".to_string());
    approval.integrity_digest = approval.recomputed_integrity_digest();
    promote_strategy_approved(&strategies, &candidate.id, &repo.join(".canon/strategies"), &evaluation, &approval).unwrap();

    let reason = "paired fixture contradiction";
    let at: DateTime<Utc> = Utc::now();
    let reason_digest = format!("{:x}", Sha256::digest(reason.as_bytes()));
    let rollback_payload = approval_payload_bytes("canon-learning-approval-v1", &format!("strategy:{}:rollback:{reason_digest}", candidate.id), None, &reason_digest, None, &[], &[], "alice", &at);
    fs::write(repo.join("rollback.sig"), signing.sign(&rollback_payload)).unwrap();

    let out = canon(
        repo,
        &["learn", "rollback", &candidate.id.to_string(), "--reason", reason, "--actor-id", "alice", "--signature-file", "rollback.sig", "--approved-at", &at.to_rfc3339_opts(SecondsFormat::Nanos, true), "--repo", ".", "--json"],
    );
    let shown = body(&out);
    assert_eq!(shown["meta"]["command"], "learn rollback", "{shown}");
    assert!(shown["meta"]["next"].is_array(), "a write carries meta.next: {shown}");
    assert!(shown["data"].is_object() && shown["data"].get("meta").is_none(), "the rollback record is data: {shown}");
    assert!(shown.get("strategy_id").is_none() && shown.get("reason").is_none(), "the old top-level keys are gone: {shown}");
}
