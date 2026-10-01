use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::LearnError;
use crate::ids::{StrategyId, TrajectoryId};
use crate::store::StrategyStore;
use crate::strategy::{DemotionEvidence, StrategyLifecycle};

use super::git_tier_path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollbackRecord {
    pub strategy_id: StrategyId,
    pub actor: String,
    pub reason: String,
    pub rolled_back_at: DateTime<Utc>,
    pub evaluation_digest: Option<String>,
    pub approval_digest: Option<String>,
    pub contradicting_trajectory_id: Option<TrajectoryId>,
    pub outcome_provenance: String,
}

/// Legacy callers cannot bypass authenticated rollback. Use
/// [`rollback_strategy_authenticated`] with a detached SSH signature.
pub fn rollback_strategy(
    _strategy_store: &dyn StrategyStore,
    _strategy_id: StrategyId,
    _reason: &str,
    _actor: &str,
    _contradicting_trajectory_id: Option<TrajectoryId>,
    _git_tier_root: &Path,
) -> Result<RollbackRecord, LearnError> {
    Err(LearnError::InvalidPromotionEvidence("rollback requires a detached SSH signature and policy-pinned approval verifier".into()))
}

pub fn rollback_strategy_authenticated(
    strategy_store: &dyn StrategyStore,
    strategy_id: StrategyId,
    reason: &str,
    actor: &str,
    contradicting_trajectory_id: Option<TrajectoryId>,
    git_tier_root: &Path,
    signature: &str,
    approved_at: DateTime<Utc>,
) -> Result<RollbackRecord, LearnError> {
    if reason.trim().is_empty() || actor.trim().is_empty() {
        return Err(LearnError::InvalidPromotionEvidence("rollback reason and actor are required".into()));
    }
    let repo_root = git_tier_root.parent().and_then(Path::parent).unwrap_or(git_tier_root);
    let allowed_signers = canon_gate::policy::allowed_signers_path(repo_root)
        .map_err(LearnError::InvalidPromotionEvidence)?
        .ok_or_else(|| LearnError::InvalidPromotionEvidence("no policy-pinned approval.allowed_signers verifier is configured".into()))?;
    let authorized = fs::read_to_string(&allowed_signers)
        .map_err(|error| LearnError::InvalidPromotionEvidence(format!("cannot read approval.allowed_signers: {error}")))?
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .filter_map(|line| line.split_whitespace().next())
        .any(|principal| principal == actor);
    if !authorized {
        return Err(LearnError::InvalidPromotionEvidence("rollback actor is not listed in policy-pinned approval.allowed_signers".into()));
    }
    if signature.trim().is_empty() {
        return Err(LearnError::InvalidPromotionEvidence("rollback detached signature is required".into()));
    }
    let reason_digest = format!("{:x}", Sha256::digest(reason.as_bytes()));
    let payload = canon_model::approval_payload_bytes(
        "canon-learning-approval-v1",
        &format!("strategy:{strategy_id}:rollback:{reason_digest}"),
        None,
        &reason_digest,
        None,
        &[],
        &[],
        actor,
        &approved_at,
    );
    canon_gate::verify_ssh_signature("canon-learning-approval-v1", &payload, signature.as_bytes(), actor, &allowed_signers)
        .map_err(LearnError::InvalidPromotionEvidence)?;
    let Some(item) = strategy_store.find_by_id(&strategy_id)? else {
        return Err(LearnError::UnknownStrategyId(strategy_id.to_string()));
    };
    if item.lifecycle == Some(StrategyLifecycle::RolledBack) {
        return Ok(RollbackRecord {
            strategy_id,
            actor: actor.to_string(),
            reason: reason.to_string(),
            rolled_back_at: item.demotion.as_ref().map_or_else(Utc::now, DemotionEvidence::demoted_at),
            evaluation_digest: None,
            approval_digest: None,
            contradicting_trajectory_id,
            outcome_provenance: "already rolled back; no additional write".to_string(),
        });
    }
    let now = Utc::now();
    let approval_digest = Some(format!("{:x}", Sha256::digest(signature.as_bytes())));
    let record = RollbackRecord {
        strategy_id,
        actor: actor.to_string(),
        reason: reason.to_string(),
        rolled_back_at: now,
        evaluation_digest: None,
        approval_digest,
        contradicting_trajectory_id,
        outcome_provenance: format!("authenticated rollback actor {actor} approved at {approved_at} and applied at {now} for regime {}", item.regime_key.as_str()),
    };
    if let Some(contradicting_trajectory_id) = contradicting_trajectory_id {
        let evidence = DemotionEvidence {
            envelope: canon_model::envelope::Envelope::new(1, canon_model::envelope::RecordKind::EvidenceRecord, now, canon_model::envelope::Actor::new_unattributed(actor)),
            contradicting_trajectory_id,
            reason: reason.to_string(),
            evaluation_digest: record.evaluation_digest.clone(),
            approval_digest: record.approval_digest.clone(),
            outcome_provenance: Some(record.outcome_provenance.clone()),
        };
        strategy_store.mark_demoted(&strategy_id, evidence)?;
    }
    strategy_store.set_lifecycle(&strategy_id, StrategyLifecycle::RolledBack)?;
    let path = git_tier_path(git_tier_root, item.role.as_str(), strategy_id);
    if path.exists() {
        let mut content = fs::read_to_string(&path)?;
        if content.starts_with("---\n") {
            content = content.replacen("status: active", "status: rolled_back", 1);
            content = content.replacen("---\n", &format!("---\nrollback_reason: {:?}\nrollback_actor: {:?}\n", reason, actor), 1);
            fs::write(path, content)?;
        }
    }
    Ok(record)
}
