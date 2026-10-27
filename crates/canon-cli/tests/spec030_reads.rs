use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
use canon_learn::{
    promote_strategy_approved, rebuild_namespace, ParquetStrategyStore, ParquetTrajectoryStore, PromotionApproval, PromotionEvaluation, PromotionEvaluationFields, StrategyItem, Trajectory,
    TrajectoryId, TrajectoryStore,
};
use canon_model::approval::{approval_payload_bytes, ApprovalPayload};
use canon_model::ids::{regime_key, RegimeKey, RoleId};
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).env_remove("CANON_ROLE").env("CANON_ACTOR", "spec030-reads").output().expect("spawn canon binary")
}

fn json(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

fn error_code(out: &Output) -> String {
    json(out)["error"]["code"].as_str().unwrap_or_default().to_string()
}

/// Every file under `dir` (relative path -> bytes), `.git` excluded.
fn tree(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().to_path_buf(), fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// An initialised canon repo with subject `demo`.
fn repo_with_subject() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(dir.path()).output().unwrap();
        assert!(out.status.success());
    }
    assert!(canon(dir.path(), &["init", "--providers", "none"]).status.success());
    assert!(canon(dir.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]).status.success());
    dir
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
    RegimeKey::parse(regime_key("dev", "repo", "spec030-reads", "deadbeef")).unwrap()
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
    let evaluation = PromotionEvaluation::new(&candidate, PromotionEvaluationFields { candidate_version: "candidate-v2".into(), baseline_result_digests: vec![digest("baseline", 0, 0.50), digest("baseline", 1, 0.54)], candidate_result_digests: vec![digest("candidate", 0, 0.91), digest("candidate", 1, 0.88)], corpus_version: "corpus-v1".into(), eval_version: "paired-eval-v1".into(), context_digest: "context-digest-v1".into(), policy_digest: "policy-digest-v1".into(), model_digest: "model-digest-v1".into(), tool_digest: "tool-digest-v1".into(), paired_metrics: BTreeMap::from([("pairs".to_string(), 2.0), ("uplift".to_string(), 0.75), ("regressions".to_string(), 0.0)]).into_iter().collect(), passed: true, decision: "promote".into() });
    (candidate, evaluation)
}

const UNKNOWN_STRATEGY: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

#[test]
fn subject_lifecycle_40_show_prints_one_subjects_record_status_and_owner_and_writes_nothing() {
    let repo = repo_with_subject();
    let before = tree(repo.path());
    let out = canon(repo.path(), &["subject", "show", "demo", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let doc = json(&out);
    assert_eq!(doc["ok"], true, "{doc}");
    assert_eq!(doc["meta"]["command"], "subject show");
    assert_eq!(doc["data"]["subject_id"], "demo");
    assert_eq!(doc["data"]["title"], "Demo");
    assert_eq!(doc["data"]["domain"], "dev");
    assert_eq!(doc["data"]["status"], "proposed");
    assert_eq!(doc["data"]["owner_role"], "implementer");

    let human = canon(repo.path(), &["subject", "show", "demo"]);
    assert_eq!(human.status.code(), Some(0));
    let text = String::from_utf8_lossy(&human.stdout).to_string();
    assert!(text.contains("demo (dev, proposed)") && text.contains("Demo") && text.contains("owner: implementer"), "{text}");
    assert_eq!(tree(repo.path()), before, "show writes nothing");
}

#[test]
fn subject_lifecycle_41_show_of_an_unknown_or_missing_subject_id_is_refused_with_exit_2_and_writes_nothing() {
    let repo = repo_with_subject();
    let before = tree(repo.path());
    let unknown = canon(repo.path(), &["subject", "show", "nope", "--json"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(error_code(&unknown), "SUBJECT_NOT_FOUND");
    assert!(json(&unknown)["error"]["message"].as_str().unwrap().contains("nope"));
    let missing = canon(repo.path(), &["subject", "show", "--json"]);
    assert_eq!(missing.status.code(), Some(2));
    assert_eq!(error_code(&missing), "USAGE");
    assert_eq!(tree(repo.path()), before, "a refused show writes nothing");
}

#[test]
fn learn_promotion_10_the_approval_payload_of_a_seeded_strategy_is_the_same_bytes_on_every_call_carrying_the_approver_time_and_subject() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    let (candidate, evaluation) = candidate_and_evaluation(repo);
    fs::write(repo.join("evaluation.json"), serde_json::to_vec(&evaluation).unwrap()).unwrap();
    let id = candidate.id.to_string();
    let args = ["strategy", "approval-payload", id.as_str(), "--evaluation", "evaluation.json", "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z"];
    let before = tree(repo);
    let first = canon(repo, &args);
    let second = canon(repo, &args);
    assert_eq!(first.status.code(), Some(0));
    assert!(!first.stdout.is_empty());
    assert_eq!(first.stdout, second.stdout, "identical arguments print identical bytes");
    let mut with_json = args.to_vec();
    with_json.push("--json");
    let doc = json(&canon(repo, &with_json));
    let hex: String = first.stdout.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(doc["data"]["payloadHex"], hex.as_str());
    assert_eq!(doc["data"]["approver"], "alice");
    assert_eq!(doc["data"]["approvalAt"], "2030-01-01T00:00:00Z");
    assert_eq!(doc["data"]["subject"], format!("strategy:{id}"));
    assert_eq!(tree(repo), before, "the payload is a read");
}

#[test]
fn learn_promotion_11_the_approval_payload_of_an_unknown_strategy_or_a_malformed_id_is_refused_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    fs::write(repo.join("evaluation.json"), "{}").unwrap();
    let before = tree(repo);
    let unknown = canon(repo, &["strategy", "approval-payload", UNKNOWN_STRATEGY, "--evaluation", "evaluation.json", "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z", "--json"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(error_code(&unknown), "LEARN_STRATEGY_NOT_FOUND");
    let malformed = canon(repo, &["strategy", "approval-payload", "nope", "--evaluation", "evaluation.json", "--approver", "alice", "--approval-at", "2030-01-01T00:00:00Z", "--json"]);
    assert_eq!(malformed.status.code(), Some(2));
    assert_eq!(error_code(&malformed), "USAGE");
    let no_evaluation = canon(repo, &["strategy", "approval-payload", UNKNOWN_STRATEGY, "--json"]);
    assert_eq!(no_evaluation.status.code(), Some(2));
    assert_eq!(error_code(&no_evaluation), "USAGE");
    assert_eq!(tree(repo), before, "a refused payload writes nothing");
}

#[test]
fn learn_promotion_12_a_rollback_of_a_promoted_strategy_signed_by_the_pinned_approver_disables_it_and_prints_the_rollback_record() {
    let signing = Signing::new();
    let repo = signing.repo.path();
    let (candidate, evaluation) = candidate_and_evaluation(repo);
    let strategies = ParquetStrategyStore::open(repo.join(".canon/learn/strategies"));
    let approved_at = Utc::now();
    let mut approval = PromotionApproval::new(candidate.id, evaluation.digest(), "alice", "human", true, approved_at);
    let payload = approval_payload_bytes(&ApprovalPayload { namespace: "canon-learning-approval-v1", subject: &format!("strategy:{}", candidate.id), project: None, artifact_sha: &evaluation.digest(), run_id: None, surface: &[], effects: &[], actor: "alice", timestamp: &approved_at });
    approval.signature = Some(signing.sign(&payload));
    approval.signer_key = Some("alice".to_string());
    approval.integrity_digest = approval.recomputed_integrity_digest();
    promote_strategy_approved(&strategies, &candidate.id, &repo.join(".canon/strategies"), &evaluation, &approval).unwrap();

    let reason = "paired fixture contradiction";
    let at: DateTime<Utc> = Utc::now();
    let at = at.to_rfc3339_opts(SecondsFormat::Nanos, true);
    let reason_digest = format!("{:x}", Sha256::digest(reason.as_bytes()));
    let rollback_payload = approval_payload_bytes(&ApprovalPayload { namespace: "canon-learning-approval-v1", subject: &format!("strategy:{}:rollback:{reason_digest}", candidate.id), project: None, artifact_sha: &reason_digest, run_id: None, surface: &[], effects: &[], actor: "alice", timestamp: &DateTime::parse_from_rfc3339(&at).unwrap().with_timezone(&Utc) });
    fs::write(repo.join("rollback.sig"), signing.sign(&rollback_payload)).unwrap();
    let id = candidate.id.to_string();
    let promoted = repo.join(".canon/strategies/dev").join(format!("{id}.md"));
    let before = fs::read_to_string(&promoted).unwrap();
    let out = canon(repo, &["strategy", "rollback", &id, "--reason", reason, "--actor", "alice", "--signature-file", "rollback.sig", "--approval-at", &at, "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let doc = json(&out);
    assert_eq!(doc["ok"], true, "{doc}");
    assert_eq!(doc["meta"]["command"], "strategy rollback");
    assert_eq!(doc["data"]["strategyId"], id.as_str());
    assert_eq!(doc["data"]["actor"], "alice");
    assert_eq!(doc["data"]["reason"], reason);
    assert!(doc["data"]["approvalDigest"].as_str().is_some_and(|d| !d.is_empty()), "{doc}");
    assert!(doc["data"]["rolledBackAt"].is_string(), "{doc}");
    let after = fs::read_to_string(&promoted).unwrap();
    assert!(before.contains("status: active") && !before.contains("rolled_back"));
    assert!(after.contains("status: rolled_back") && after.contains("rollback_actor: \"alice\"") && after.contains("rollback_reason: \"paired fixture contradiction\""), "{after}");
    assert!(after.ends_with("buffer the paired writes before flushing\n"), "the body survives: {after}");
}

#[test]
fn learn_promotion_13_a_rollback_with_an_unreadable_or_invalid_signature_or_a_malformed_timestamp_is_refused_and_writes_nothing() {
    let signing = Signing::new();
    let repo = signing.repo.path();
    let (candidate, _) = candidate_and_evaluation(repo);
    let id = candidate.id.to_string();
    fs::write(repo.join("bad.sig"), "not a signature\n").unwrap();
    let before = tree(repo);
    let rollback = |id: &str, sig: &str, at: &str| {
        canon(repo, &["strategy", "rollback", id, "--reason", "regressed", "--actor", "alice", "--signature-file", sig, "--approval-at", at, "--json"])
    };
    let unreadable = rollback(&id, "missing.sig", "2030-01-01T00:00:00Z");
    assert_eq!(unreadable.status.code(), Some(2));
    assert_eq!(error_code(&unreadable), "LEARN_SIGNATURE_UNREADABLE");
    let bad_signature = rollback(&id, "bad.sig", "2030-01-01T00:00:00Z");
    assert_eq!(bad_signature.status.code(), Some(1));
    assert_eq!(error_code(&bad_signature), "LEARN_ROLLBACK_FAILED");
    let message = json(&bad_signature)["error"]["message"].as_str().unwrap().to_string();
    assert!(message.contains("ssh signature verification failed"), "{message}");
    let bad_time = rollback(&id, "bad.sig", "yesterday");
    assert_eq!(bad_time.status.code(), Some(2));
    assert_eq!(error_code(&bad_time), "USAGE");
    let no_reason = canon(repo, &["strategy", "rollback", &id, "--json"]);
    assert_eq!(no_reason.status.code(), Some(2));
    assert_eq!(error_code(&no_reason), "USAGE");
    assert_eq!(tree(repo), before, "a refused rollback writes nothing");
}

#[test]
fn retrieve_guidance_07_regime_show_refuses_a_missing_segment_or_a_hash_that_is_not_hex_with_exit_2() {
    let dir = tempfile::tempdir().unwrap();
    let regime_show = |extra: &[&str]| {
        let mut args = vec!["regime", "show", "--role-segment", "reviewer", "--repo-name", "my-repo", "--area", "api"];
        args.extend_from_slice(extra);
        args.push("--json");
        canon(dir.path(), &args)
    };
    let ok = regime_show(&["--hash", "abcdef012345"]);
    assert_eq!(ok.status.code(), Some(0));
    assert_eq!(json(&ok)["data"]["regimeKey"], "reviewer/my-repo/api/abcdef012345");
    let not_hex = regime_show(&["--hash", "zz"]);
    assert_eq!(not_hex.status.code(), Some(2));
    assert_eq!(error_code(&not_hex), "REGIME_KEY_INVALID");
    let missing = regime_show(&[]);
    assert_eq!(missing.status.code(), Some(2));
    assert_eq!(error_code(&missing), "USAGE");
    assert_eq!(json(&missing)["error"]["field"], "--hash");
}

#[test]
fn context_capability_45_brief_show_of_a_change_with_no_brief_or_in_a_repo_with_no_briefs_is_refused_with_exit_2() {
    let repo = repo_with_subject();
    let none = canon(repo.path(), &["brief", "show", "--json"]);
    assert_eq!(none.status.code(), Some(2));
    assert_eq!(error_code(&none), "NO_BRIEF");

    assert!(canon(repo.path(), &["change", "new", "a", "--subject", "demo", "--title", "A"]).status.success());
    assert!(canon(repo.path(), &["change", "new", "b", "--subject", "demo", "--title", "B"]).status.success());
    let before = tree(repo.path());
    let unknown = canon(repo.path(), &["brief", "show", "nope", "--json"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(error_code(&unknown), "BRIEF_NOT_FOUND");
    assert!(json(&unknown)["error"]["message"].as_str().unwrap().contains(".canon/changes/nope/brief.md"));
    let ambiguous = canon(repo.path(), &["brief", "show", "--json"]);
    assert_eq!(ambiguous.status.code(), Some(2));
    assert_eq!(error_code(&ambiguous), "CHANGE_AMBIGUOUS");
    assert_eq!(tree(repo.path()), before, "a refused show writes nothing");
}

#[test]
fn context_capability_46_schema_of_an_unknown_command_or_a_leaf_with_a_sub_path_is_refused_with_unknown_command_and_exit_2() {
    let repo = repo_with_subject();
    let before = tree(repo.path());
    let unknown = canon(repo.path(), &["schema", "nope", "--json"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(error_code(&unknown), "UNKNOWN_COMMAND");
    assert!(json(&unknown)["error"]["message"].as_str().unwrap().contains("nope"));
    let sub_path = canon(repo.path(), &["schema", "subject", "show", "nope", "extra", "--json"]);
    assert_eq!(sub_path.status.code(), Some(2));
    assert_eq!(error_code(&sub_path), "UNKNOWN_COMMAND");
    assert!(json(&sub_path)["error"]["message"].as_str().unwrap().contains("is a leaf command"));
    assert_eq!(tree(repo.path()), before, "a refused schema writes nothing");
}

#[test]
fn divergence_lifecycle_13_list_refuses_an_invalid_timestamp_a_zero_limit_or_a_cursor_canon_did_not_issue_with_exit_2() {
    let repo = repo_with_subject();
    let before = tree(repo.path());
    let ok = canon(repo.path(), &["divergence", "list", "--json"]);
    assert_eq!(ok.status.code(), Some(0));
    assert_eq!(json(&ok)["data"]["summary"]["total"], 0);

    let bad_time = canon(repo.path(), &["divergence", "list", "--as-of", "bogus", "--json"]);
    assert_eq!(bad_time.status.code(), Some(2));
    assert_eq!(error_code(&bad_time), "USAGE");
    assert_eq!(json(&bad_time)["error"]["field"], "--as-of");
    let zero = canon(repo.path(), &["divergence", "list", "--limit", "0", "--json"]);
    assert_eq!(zero.status.code(), Some(2));
    assert_eq!(error_code(&zero), "USAGE");
    assert_eq!(json(&zero)["error"]["field"], "--limit");
    let cursor = canon(repo.path(), &["divergence", "list", "--cursor", "zzz", "--json"]);
    assert_eq!(cursor.status.code(), Some(2));
    assert_eq!(error_code(&cursor), "INVALID_CURSOR");
    let both = canon(repo.path(), &["divergence", "list", "--limit", "1", "--all", "--json"]);
    assert_eq!(both.status.code(), Some(2));
    assert_eq!(error_code(&both), "USAGE");
    assert_eq!(tree(repo.path()), before, "a refused list writes nothing");
}

#[test]
fn report_generation_14_snapshot_without_an_output_directory_is_refused_with_exit_2_and_writes_nothing() {
    let repo = repo_with_subject();
    let before = tree(repo.path());
    let out = canon(repo.path(), &["report", "snapshot", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(error_code(&out), "USAGE");
    assert_eq!(json(&out)["error"]["field"], "<DIR>");
    assert_eq!(tree(repo.path()), before, "a refused snapshot writes nothing");
}

#[test]
fn learn_promotion_14_a_rollback_of_an_unknown_strategy_is_refused_before_any_signature_is_read_and_writes_nothing() {
    let signing = Signing::new();
    let repo = signing.repo.path();
    let (candidate, _) = candidate_and_evaluation(repo);
    fs::write(repo.join("bad.sig"), "not a signature\n").unwrap();
    let before = tree(repo);
    for sig in ["missing.sig", "bad.sig"] {
        let out = canon(repo, &["strategy", "rollback", UNKNOWN_STRATEGY, "--reason", "regressed", "--actor", "alice", "--signature-file", sig, "--approval-at", "2030-01-01T00:00:00Z", "--json"]);
        assert_eq!(out.status.code(), Some(2), "{sig}");
        assert_eq!(error_code(&out), "LEARN_STRATEGY_NOT_FOUND", "{sig}");
        assert!(json(&out)["error"]["message"].as_str().unwrap().contains(UNKNOWN_STRATEGY));
    }
    let existing = canon(repo, &["strategy", "rollback", &candidate.id.to_string(), "--reason", "regressed", "--actor", "alice", "--signature-file", "bad.sig", "--approval-at", "2030-01-01T00:00:00Z", "--json"]);
    assert_eq!(existing.status.code(), Some(1));
    assert_eq!(error_code(&existing), "LEARN_ROLLBACK_FAILED");
    assert_eq!(tree(repo), before, "a refused rollback writes nothing");
}
