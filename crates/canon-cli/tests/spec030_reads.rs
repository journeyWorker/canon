#[path = "support/hermetic.rs"]
mod hermetic;
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
    hermetic::canon().args(args).current_dir(repo).env_remove("CANON_ROLE").env("CANON_ACTOR", "spec030-reads").output().expect("spawn canon binary")
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
    assert!(canon(dir.path(), &["init", "--no-harness"]).status.success());
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
    assert_eq!(error_code(&unknown), "STRATEGY_NOT_FOUND");
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
    assert_eq!(error_code(&unreadable), "STRATEGY_SIGNATURE_NOT_FOUND");
    let bad_signature = rollback(&id, "bad.sig", "2030-01-01T00:00:00Z");
    assert_eq!(bad_signature.status.code(), Some(2));
    assert_eq!(error_code(&bad_signature), "STRATEGY_SIGNATURE_INVALID");
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
    assert_eq!(error_code(&none), "BRIEF_REQUIRED");

    assert!(canon(repo.path(), &["change", "new", "a", "--subject", "demo", "--title", "A"]).status.success());
    assert!(canon(repo.path(), &["change", "new", "b", "--subject", "demo", "--title", "B"]).status.success());
    let before = tree(repo.path());
    let unknown = canon(repo.path(), &["brief", "show", "nope", "--json"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(error_code(&unknown), "BRIEF_NOT_FOUND");
    assert!(json(&unknown)["error"]["message"].as_str().unwrap().contains(".canon/changes/nope/brief.md"));
    let ambiguous = canon(repo.path(), &["brief", "show", "--json"]);
    assert_eq!(ambiguous.status.code(), Some(2));
    assert_eq!(error_code(&ambiguous), "CHANGE_REQUIRED");
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
    assert!(json(&sub_path)["error"]["message"].as_str().unwrap().contains("takes no sub-path"));
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
    assert_eq!(error_code(&cursor), "CURSOR_INVALID");
    let both = canon(repo.path(), &["divergence", "list", "--limit", "1", "--all", "--json"]);
    assert_eq!(both.status.code(), Some(2));
    assert_eq!(error_code(&both), "USAGE");
    assert_eq!(json(&both)["error"]["field"], "--limit", "the combined flags name the option");
    assert_eq!(tree(repo.path()), before, "a refused list writes nothing");
}

#[test]
fn report_generation_14_snapshot_without_an_output_directory_is_refused_with_exit_2_and_writes_nothing() {
    let repo = repo_with_subject();
    let before = tree(repo.path());
    let out = canon(repo.path(), &["report", "snapshot", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(error_code(&out), "USAGE");
    assert_eq!(json(&out)["error"]["field"], "DIR");
    assert_eq!(tree(repo.path()), before, "a refused snapshot writes nothing");
}

/// atip.codestored.02 (the detached signature of a strategy rollback): a signature file the system refuses is `STRATEGY_SIGNATURE_DENIED`, a path
/// that is no file (a directory) or bytes that are not UTF-8 text are `STRATEGY_SIGNATURE_INVALID`, a path that is not there is
/// `STRATEGY_SIGNATURE_NOT_FOUND`; each exits 2, is not retryable and writes nothing (a test run as root, which ignores file modes, skips the
/// refused read and says so).
#[test]
fn atip_codestored_02_a_detached_signature_that_cannot_be_read_is_denied_invalid_or_not_found_by_its_cause() {
    use std::os::unix::fs::PermissionsExt;
    let signing = Signing::new();
    let repo = signing.repo.path();
    let (candidate, _) = candidate_and_evaluation(repo);
    let id = candidate.id.to_string();
    let files = tempfile::TempDir::new().unwrap();
    let (directory, not_utf8, closed) = (files.path().join("a-directory"), files.path().join("not-utf8.sig"), files.path().join("closed.sig"));
    fs::create_dir(&directory).unwrap();
    fs::write(&not_utf8, [0xff, 0xfe, 0x00, 0x41]).unwrap();
    fs::write(&closed, "x").unwrap();
    let before = tree(repo);
    let rollback = |signature: &Path| {
        let out = canon(repo, &["strategy", "rollback", &id, "--reason", "regressed", "--actor", "alice", "--signature-file", signature.to_str().unwrap(), "--approval-at", "2030-01-01T00:00:00Z", "--json"]);
        assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stdout));
        assert_eq!(json(&out)["error"]["retryable"], false);
        assert!(json(&out).get("data").is_none(), "a refusal carries no data");
        error_code(&out)
    };
    assert_eq!(rollback(&files.path().join("missing.sig")), "STRATEGY_SIGNATURE_NOT_FOUND");
    assert_eq!(rollback(&directory), "STRATEGY_SIGNATURE_INVALID");
    assert_eq!(rollback(&not_utf8), "STRATEGY_SIGNATURE_INVALID");
    fs::set_permissions(&closed, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&closed).is_err() {
        assert_eq!(rollback(&closed), "STRATEGY_SIGNATURE_DENIED");
    } else {
        eprintln!("skipped: this process can read a file whose mode is 000 (running as root?), so a refused read cannot be shown");
    }
    fs::set_permissions(&closed, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(tree(repo), before, "a refused rollback writes nothing");
}

/// atip.codesplit.06: a `strategy rollback` the signature check turns away says which condition failed, each exit 2 with nothing recorded: a blank
/// `--reason` is `STRATEGY_ARGUMENT_INVALID` (field `--reason`), a policy with no `approval.allowed_signers` is `STRATEGY_VERIFIER_REQUIRED`, a policy that does
/// not parse or an allowed-signers file that is not there is `CONFIG_INVALID` with the reason `POLICY`, one this user may not read is `CONFIG_DENIED`, an
/// actor the file does not list is `STRATEGY_SIGNER_DENIED`, an empty signature file is `STRATEGY_SIGNATURE_REQUIRED`, and only a signature that does not verify
/// is `STRATEGY_SIGNATURE_INVALID` (the mapping of the operating-system errors that a repeat can pass is the unit test of `learn.rs`).
#[test]
fn atip_codesplit_06_a_rollback_the_signature_check_turns_away_says_which_condition_failed() {
    use std::os::unix::fs::PermissionsExt;
    let signing = Signing::new();
    let repo = signing.repo.path();
    let (candidate, _) = candidate_and_evaluation(repo);
    let id = candidate.id.to_string();
    fs::write(repo.join("bad.sig"), "not a signature\n").unwrap();
    fs::write(repo.join("empty.sig"), "").unwrap();
    let before = tree(repo);
    let rollback = |reason: &str, actor: &str, signature: &str| {
        canon(repo, &["strategy", "rollback", &id, "--reason", reason, "--actor", actor, "--signature-file", signature, "--approval-at", "2030-01-01T00:00:00Z", "--json"])
    };
    let refused = |out: Output, code: &str, reason: Option<&str>| -> Value {
        let doc = json(&out);
        assert_eq!(out.status.code(), Some(2), "{doc}");
        assert_eq!((doc["error"]["code"].as_str(), doc["error"]["reason"].as_str(), doc["error"]["retryable"].as_bool()), (Some(code), reason, Some(false)), "{doc}");
        assert!(doc.get("data").is_none(), "a refusal carries no data: {doc}");
        doc
    };

    // A blank reason is the caller's value, named by its field.
    let blank = refused(rollback("  ", "alice", "bad.sig"), "STRATEGY_ARGUMENT_INVALID", None);
    assert_eq!(blank["error"]["field"], "--reason", "{blank}");
    // An actor the allowed-signers file does not list, and a signature file with nothing in it.
    refused(rollback("regressed", "bob", "bad.sig"), "STRATEGY_SIGNER_DENIED", None);
    refused(rollback("regressed", "alice", "empty.sig"), "STRATEGY_SIGNATURE_REQUIRED", None);
    // A signature that is there and does not verify is the one INVALID.
    refused(rollback("regressed", "alice", "bad.sig"), "STRATEGY_SIGNATURE_INVALID", None);

    // The allowed-signers file this user may not read, and one that is not there.
    let allowed = repo.join(".canon/allowed_signers");
    let signers = fs::read(&allowed).unwrap();
    fs::set_permissions(&allowed, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&allowed).is_err() {
        refused(rollback("regressed", "alice", "bad.sig"), "CONFIG_DENIED", None);
    } else {
        eprintln!("skipped: this process can read a file whose mode is 000 (running as root?), so a refused read cannot be shown");
    }
    fs::set_permissions(&allowed, fs::Permissions::from_mode(0o644)).unwrap();
    fs::remove_file(&allowed).unwrap();
    refused(rollback("regressed", "alice", "bad.sig"), "CONFIG_INVALID", Some("POLICY"));
    fs::write(&allowed, &signers).unwrap();

    // A policy that does not parse, and one that names no verifier.
    let policy = repo.join(".canon/policy.yaml");
    let policy_text = fs::read_to_string(&policy).unwrap();
    fs::write(&policy, "approval: [x\n").unwrap();
    refused(rollback("regressed", "alice", "bad.sig"), "CONFIG_INVALID", Some("POLICY"));
    fs::write(&policy, "{}\n").unwrap();
    let required = refused(rollback("regressed", "alice", "bad.sig"), "STRATEGY_VERIFIER_REQUIRED", None);
    assert!(required["error"]["hint"].as_str().unwrap().contains("allowed_signers"), "{required}");
    fs::write(&policy, policy_text).unwrap();

    assert_eq!(tree(repo), before, "a refused rollback writes nothing");
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
        assert_eq!(error_code(&out), "STRATEGY_NOT_FOUND", "{sig}");
        assert!(json(&out)["error"]["message"].as_str().unwrap().contains(UNKNOWN_STRATEGY));
    }
    let existing = canon(repo, &["strategy", "rollback", &candidate.id.to_string(), "--reason", "regressed", "--actor", "alice", "--signature-file", "bad.sig", "--approval-at", "2030-01-01T00:00:00Z", "--json"]);
    assert_eq!(existing.status.code(), Some(2));
    assert_eq!(error_code(&existing), "STRATEGY_SIGNATURE_INVALID");
    assert_eq!(tree(repo), before, "a refused rollback writes nothing");
}

// ---------------------------------------------------------------------------
// `canon-0-37-option-forms`: `--approval-at` is an RFC 3339 date-time, the instant the payload holds and the signature was made over.

/// One instant (2030-01-01T10:00:00.123456789Z), as `--approval-at` may spell it.
fn instant_spellings() -> Vec<String> {
    let at = instant();
    let offset = chrono::FixedOffset::east_opt(9 * 3600).expect("an offset");
    vec![
        at.to_rfc3339_opts(SecondsFormat::Nanos, true),
        at.to_rfc3339_opts(SecondsFormat::Nanos, true).to_lowercase(),
        at.with_timezone(&offset).to_rfc3339_opts(SecondsFormat::Nanos, false),
        at.with_timezone(&chrono::FixedOffset::west_opt(5 * 3600).expect("an offset")).to_rfc3339_opts(SecondsFormat::Nanos, false),
    ]
}

fn instant() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2030-01-01T10:00:00.123456789Z").expect("an instant").with_timezone(&Utc)
}

#[test]
fn atip_optionforms_01_the_strategy_approval_payload_is_the_same_bytes_for_one_instant_in_any_spelling_and_reads_no_clock() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    let (candidate, evaluation) = candidate_and_evaluation(repo);
    fs::write(repo.join("evaluation.json"), serde_json::to_vec(&evaluation).unwrap()).unwrap();
    let id = candidate.id.to_string();
    let payload = |at: &str| canon(repo, &["strategy", "approval-payload", id.as_str(), "--evaluation", "evaluation.json", "--approver", "alice", "--approval-at", at]);
    let before = tree(repo);
    let spellings = instant_spellings();
    let first = payload(&spellings[0]);
    assert_eq!(first.status.code(), Some(0), "{}", String::from_utf8_lossy(&first.stderr));
    assert!(String::from_utf8_lossy(&first.stdout).contains("\"timestamp\":\"2030-01-01T10:00:00.123456789Z\""), "{}", String::from_utf8_lossy(&first.stdout));
    for at in &spellings {
        let got = payload(at);
        assert_eq!((got.status.code(), &got.stdout), (Some(0), &first.stdout), "`{at}` is the bytes of the instant");
    }
    assert_eq!(payload(&spellings[0]).stdout, first.stdout, "the same arguments print the same bytes");
    assert_ne!(payload("2030-01-01T10:00:00.123456788Z").stdout, first.stdout, "another instant is other bytes");
    // The report holds the instant as UTC, whatever the spelling.
    let doc = json(&canon(repo, &["strategy", "approval-payload", id.as_str(), "--evaluation", "evaluation.json", "--approver", "alice", "--approval-at", "2030-01-01T19:00:00+09:00", "--json"]));
    assert_eq!(doc["data"]["approvalAt"], "2030-01-01T10:00:00Z");
    assert_eq!(tree(repo), before, "the payload is a read");
    // A date alone, and the loose spellings, print no bytes.
    for at in ["2030-01-01", " 2030-01-01T10:00:00Z", "2030-01-01 10:00:00Z", "2030-1-1T10:00:00Z", "+2030-01-01T10:00:00Z", "2030-01-01T10:00:00\u{2212}02:00", "2030-01-01T10:00", "2030-01-01T10:00:00", "2030-01-01T25:00:00Z"] {
        let value = format!("--approval-at={at}");
        let got = canon(repo, &["strategy", "approval-payload", id.as_str(), "--evaluation", "evaluation.json", "--approver", "alice", &value]);
        assert_eq!(got.status.code(), Some(2), "`{at}`");
        assert!(got.stdout.is_empty(), "`{at}` printed bytes: {}", String::from_utf8_lossy(&got.stdout));
    }
    assert_eq!(tree(repo), before, "a refused payload writes nothing");
}

/// A promoted strategy and the signature `rollback.sig` of its rollback by `alice` at `at`; the strategy id.
fn promoted_and_signed(signing: &Signing, reason: &str, at: DateTime<Utc>) -> String {
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
    let reason_digest = format!("{:x}", Sha256::digest(reason.as_bytes()));
    let rollback_payload = approval_payload_bytes(&ApprovalPayload { namespace: "canon-learning-approval-v1", subject: &format!("strategy:{}:rollback:{reason_digest}", candidate.id), project: None, artifact_sha: &reason_digest, run_id: None, surface: &[], effects: &[], actor: "alice", timestamp: &at });
    fs::write(repo.join("rollback.sig"), signing.sign(&rollback_payload)).unwrap();
    candidate.id.to_string()
}

#[test]
fn atip_optionforms_05_a_rollback_signed_over_an_instant_is_recorded_when_the_time_is_written_in_any_spelling_of_it() {
    let reason = "paired fixture contradiction";
    for (index, at) in instant_spellings().iter().enumerate() {
        let signing = Signing::new();
        let repo = signing.repo.path();
        let id = promoted_and_signed(&signing, reason, instant());
        let promoted = repo.join(".canon/strategies/dev").join(format!("{id}.md"));
        assert!(fs::read_to_string(&promoted).unwrap().contains("status: active"), "the strategy is promoted");
        let out = canon(repo, &["strategy", "rollback", &id, "--reason", reason, "--actor", "alice", "--signature-file", "rollback.sig", "--approval-at", at, "--json"]);
        let doc = json(&out);
        assert_eq!((out.status.code(), &doc["ok"]), (Some(0), &Value::Bool(true)), "spelling {index} `{at}`: {doc}");
        assert_eq!(doc["data"]["strategyId"], id.as_str());
        let after = fs::read_to_string(&promoted).unwrap();
        assert!(after.contains("status: rolled_back") && after.contains("rollback_actor: \"alice\""), "spelling {index}: {after}");
    }
}

#[test]
fn atip_optionforms_06_a_rollback_with_another_instant_or_a_refused_spelling_is_refused_and_records_nothing() {
    let reason = "paired fixture contradiction";
    let signing = Signing::new();
    let repo = signing.repo.path();
    let id = promoted_and_signed(&signing, reason, instant());
    let before = tree(repo);
    let rollback = |at: &str| {
        let value = format!("--approval-at={at}");
        canon(repo, &["strategy", "rollback", &id, "--reason", reason, "--actor", "alice", "--signature-file", "rollback.sig", &value, "--json"])
    };
    // Another instant: the signature does not verify over it.
    for at in ["2030-01-01T10:00:00.123456788Z", "2030-01-01T10:00:00Z", "2030-01-01T10:00:00.123456789+01:00", "2030-01-02T10:00:00.123456789Z"] {
        let out = rollback(at);
        assert_eq!((out.status.code(), error_code(&out).as_str()), (Some(2), "STRATEGY_SIGNATURE_INVALID"), "`{at}`: {}", String::from_utf8_lossy(&out.stdout));
        assert_eq!(tree(repo), before, "`{at}` recorded something");
    }
    // A spelling that is no RFC 3339 date-time: the argument stage refuses it, the date the signature may have been made over included.
    for at in ["2030-01-01", " 2030-01-01T10:00:00.123456789Z", "2030-01-01 10:00:00.123456789Z", "2030-01-01T10:00", "2030-1-1T10:00:00Z", "2030-01-01T10:00:00.123456789\u{2212}02:00", ""] {
        let out = rollback(at);
        let doc = json(&out);
        assert_eq!((out.status.code(), doc["error"]["code"].as_str(), doc["error"]["field"].as_str()), (Some(2), Some("USAGE"), Some("--approval-at")), "`{at}`: {doc}");
        assert_eq!(tree(repo), before, "`{at}` recorded something");
    }
    assert!(fs::read_to_string(repo.join(".canon/strategies/dev").join(format!("{id}.md"))).unwrap().contains("status: active"), "the strategy is still active");
}
