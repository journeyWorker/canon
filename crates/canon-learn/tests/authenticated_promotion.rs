use std::fs;
use std::path::PathBuf;
use std::process::Command;

use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
use canon_learn::{
    promote_strategy_approved, rebuild_namespace, retrieve, rollback_strategy_authenticated, ParquetStrategyStore, ParquetTrajectoryStore,
    PromotionApproval, PromotionEvaluation, StrategyLifecycle, StrategyStore, Trajectory, TrajectoryId, TrajectoryStore,
};
use canon_model::approval::approval_payload_bytes;
use canon_model::ids::{regime_key, RegimeKey, RoleId};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

struct SigningFixture {
    repo: tempfile::TempDir,
    private_key: PathBuf,
    principal: &'static str,
}

impl SigningFixture {
    fn new() -> TestResult<Self> {
        let repo = tempfile::tempdir()?;
        let canon_dir = repo.path().join(".canon");
        fs::create_dir_all(&canon_dir)?;
        let private_key = repo.path().join("approval_ed25519");
        let generated = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-C", "canon-learning-test", "-f"])
            .arg(&private_key)
            .output()?;
        if !generated.status.success() {
            return Err(format!("ssh-keygen key generation failed: {}", String::from_utf8_lossy(&generated.stderr)).into());
        }
        let public_key = fs::read_to_string(private_key.with_extension("pub"))?;
        fs::write(canon_dir.join("allowed_signers"), format!("alice {public_key}"))?;
        fs::write(
            canon_dir.join("policy.yaml"),
            "approval:\n  allowed_signers: .canon/allowed_signers\n",
        )?;
        Ok(Self { repo, private_key, principal: "alice" })
    }

    fn sign(&self, namespace: &str, payload: &[u8]) -> TestResult<String> {
        let payload_file = tempfile::NamedTempFile::new()?;
        fs::write(payload_file.path(), payload)?;
        let output = Command::new("ssh-keygen")
            .args(["-Y", "sign", "-f"])
            .arg(&self.private_key)
            .args(["-n", namespace])
            .arg(payload_file.path())
            .output()?;
        if !output.status.success() {
            return Err(format!("ssh-keygen signing failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }
        let signature_path = PathBuf::from(format!("{}.sig", payload_file.path().display()));
        Ok(String::from_utf8(fs::read(signature_path)?)?)
    }
}

fn regime() -> RegimeKey {
    RegimeKey::parse(regime_key("dev", "repo", "authenticated-promotion", "deadbeef")).unwrap()
}

fn trajectory(at: DateTime<Utc>) -> Trajectory {
    Trajectory::new(
        TrajectoryId::new(),
        regime(),
        "batch authenticated writes",
        "buffer the paired writes before flushing",
        vec![VerdictRow {
            role: RoleId::parse("dev").unwrap(),
            polarity: Polarity::Success,
            becomes: Becomes::StrategyCandidate,
        }],
        at,
        vec!["paired-fixture".to_string()],
    )
    .unwrap()
}


fn result_digest(variant: &str, panel: usize, score: f64) -> String {
    format!("{:x}", Sha256::digest(format!("{variant}|panel={panel}|score={score:.3}").as_bytes()))
}

fn paired_result_digests() -> (Vec<String>, Vec<String>) {
    let baseline = [(0, 0.50), (1, 0.54)];
    let candidate = [(0, 0.91), (1, 0.88)];
    (
        baseline.into_iter().map(|(panel, score)| result_digest("baseline", panel, score)).collect(),
        candidate.into_iter().map(|(panel, score)| result_digest("candidate", panel, score)).collect(),
    )
}
fn approved_bundle(
    candidate: &canon_learn::StrategyItem,
    signing: &SigningFixture,
) -> TestResult<(PromotionEvaluation, PromotionApproval)> {
    let (baseline_results, candidate_results) = paired_result_digests();
    let evaluation = PromotionEvaluation::new(
        candidate,
        "candidate-v2",
        baseline_results,
        candidate_results,
        "corpus-v1",
        "paired-eval-v1",
        "context-digest-v1",
        "policy-digest-v1",
        "model-digest-v1",
        "tool-digest-v1",
        [
            ("pairs".to_string(), 2.0),
            ("uplift".to_string(), 0.75),
            ("regressions".to_string(), 0.0),
        ]
        .into_iter()
        .collect(),
        true,
        "promote",
    );
    let approved_at = Utc::now();
    let mut approval = PromotionApproval::new(candidate.id, evaluation.digest(), signing.principal, "human", true, approved_at);
    let payload = approval_payload_bytes(
        "canon-learning-approval-v1",
        &format!("strategy:{}", candidate.id),
        None,
        &evaluation.digest(),
        None,
        &[],
        &[],
        signing.principal,
        &approved_at,
    );
    approval.signature = Some(signing.sign("canon-learning-approval-v1", &payload)?);
    approval.signer_key = Some(signing.principal.to_string());
    approval.integrity_digest = approval.recomputed_integrity_digest();
    Ok((evaluation, approval))
}

fn rollback_signature(signing: &SigningFixture, strategy_id: canon_learn::StrategyId, reason: &str, at: &DateTime<Utc>) -> TestResult<String> {
    let reason_digest = format!("{:x}", Sha256::digest(reason.as_bytes()));
    let payload = approval_payload_bytes(
        "canon-learning-approval-v1",
        &format!("strategy:{strategy_id}:rollback:{reason_digest}"),
        None,
        &reason_digest,
        None,
        &[],
        &[],
        signing.principal,
        at,
    );
    signing.sign("canon-learning-approval-v1", &payload)
}

fn active_ids(items: &[canon_learn::StrategyItem]) -> Vec<canon_learn::StrategyId> {
    items.iter().filter(|item| item.lifecycle == Some(StrategyLifecycle::Active)).map(|item| item.id).collect()
}

#[test]
fn authenticated_activation_retrieval_rebuild_and_rollback_are_end_to_end() -> TestResult<()> {
    let signing = SigningFixture::new()?;
    let trajectories = ParquetTrajectoryStore::open(signing.repo.path().join(".canon/learn/trajectories"));
    let strategies = ParquetStrategyStore::open(signing.repo.path().join(".canon/learn/strategies"));
    let git_tier = signing.repo.path().join(".canon/strategies");
    let now = Utc::now();
    let source = trajectory(now);
    trajectories.append(&source)?;

    let initial = rebuild_namespace(&trajectories, &strategies, &regime())?;
    assert!(retrieve(&strategies, &regime(), None)?.is_empty(), "quarantined candidates must be excluded");
    let candidate = initial.into_iter().next().expect("fixture distills one candidate");
    assert_eq!(candidate.lifecycle, Some(StrategyLifecycle::Quarantined));
    let (evaluation, approval) = approved_bundle(&candidate, &signing)?;
    let promotion = promote_strategy_approved(&strategies, &candidate.id, &git_tier, &evaluation, &approval)?;
    let promoted_bytes = fs::read(&promotion.path)?;
    assert_eq!(active_ids(&strategies.query_by_regime_key(&regime())?), vec![candidate.id]);
    assert_eq!(retrieve(&strategies, &regime(), None)?.iter().map(|item| item.id).collect::<Vec<_>>(), vec![candidate.id]);
    assert!(fs::read_to_string(&promotion.path)?.contains(&format!("evaluation_digest: {}", evaluation.digest())));
    assert!(fs::read_to_string(&promotion.path)?.contains(&format!("approval_digest: {}", approval.integrity_digest)));

    rebuild_namespace(&trajectories, &strategies, &regime())?;
    let rebuilt = strategies.find_by_id(&candidate.id)?.expect("active candidate survives rebuild");
    assert_eq!(rebuilt.lifecycle, Some(StrategyLifecycle::Active));
    assert_eq!(rebuilt.source_trajectory_ids, candidate.source_trajectory_ids);
    assert_eq!(fs::read(&promotion.path)?, promoted_bytes, "rebuild preserves active promotion provenance");
    assert_eq!(retrieve(&strategies, &regime(), None)?.len(), 1);

    let rollback_reason = "paired fixture contradiction";
    let rollback_at = Utc::now();
    let rollback_sig = rollback_signature(&signing, candidate.id, rollback_reason, &rollback_at)?;
    rollback_strategy_authenticated(
        &strategies,
        candidate.id,
        rollback_reason,
        signing.principal,
        Some(source.id),
        &git_tier,
        &rollback_sig,
        rollback_at,
    )?;
    assert!(retrieve(&strategies, &regime(), None)?.is_empty(), "rolled-back candidates must be excluded");
    let rolled_back_bytes = fs::read(&promotion.path)?;
    assert!(String::from_utf8_lossy(&rolled_back_bytes).contains("status: rolled_back"));

    rollback_strategy_authenticated(
        &strategies,
        candidate.id,
        rollback_reason,
        signing.principal,
        Some(source.id),
        &git_tier,
        &rollback_sig,
        rollback_at,
    )?;
    assert_eq!(fs::read(&promotion.path)?, rolled_back_bytes, "repeated rollback is idempotent");
    assert_eq!(strategies.find_by_id(&candidate.id)?.unwrap().lifecycle, Some(StrategyLifecycle::RolledBack));
    Ok(())
}

#[test]
fn tampered_candidate_evaluation_and_signature_leave_zero_active_writes() -> TestResult<()> {
    let signing = SigningFixture::new()?;
    let trajectories = ParquetTrajectoryStore::open(signing.repo.path().join(".canon/learn/trajectories"));
    let strategies = ParquetStrategyStore::open(signing.repo.path().join(".canon/learn/strategies"));
    let git_tier = signing.repo.path().join(".canon/strategies");
    let source = trajectory(Utc::now());
    trajectories.append(&source)?;
    rebuild_namespace(&trajectories, &strategies, &regime())?;
    let candidate = strategies.query_by_regime_key(&regime())?.into_iter().next().unwrap();
    let (evaluation, approval) = approved_bundle(&candidate, &signing)?;
    let target = git_tier.join("dev").join(format!("{}.md", candidate.id));

    let mut tampered_candidate = candidate.clone();
    tampered_candidate.content.push_str(" tampered");
    strategies.append(&tampered_candidate)?;
    assert!(promote_strategy_approved(&strategies, &candidate.id, &git_tier, &evaluation, &approval).is_err());
    assert_eq!(strategies.find_by_id(&candidate.id)?.unwrap().lifecycle, Some(StrategyLifecycle::Quarantined));
    assert!(!target.exists());

    strategies.append(&candidate)?;
    let mut tampered_evaluation = evaluation.clone();
    tampered_evaluation.candidate_digest = "tampered-candidate-digest".to_string();
    assert!(promote_strategy_approved(&strategies, &candidate.id, &git_tier, &tampered_evaluation, &approval).is_err());
    assert_eq!(strategies.find_by_id(&candidate.id)?.unwrap().lifecycle, Some(StrategyLifecycle::Quarantined));
    assert!(!target.exists());

    let mut tampered_approval = approval.clone();
    tampered_approval.signature = Some("-----BEGIN SSH SIGNATURE-----\ntampered\n-----END SSH SIGNATURE-----\n".to_string());
    assert!(promote_strategy_approved(&strategies, &candidate.id, &git_tier, &evaluation, &tampered_approval).is_err());
    assert_eq!(strategies.find_by_id(&candidate.id)?.unwrap().lifecycle, Some(StrategyLifecycle::Quarantined));
    assert!(!target.exists());
    Ok(())
}

