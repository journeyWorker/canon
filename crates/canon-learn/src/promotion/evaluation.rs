use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::PromotionEvidenceError;
use crate::ids::{StrategyId, TrajectoryId};
use crate::strategy::{StrategyItem, StrategyLifecycle};

pub const PROMOTION_EVALUATION_VERSION: u32 = 1;
pub const PROMOTION_APPROVAL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromotionEvaluation {
    pub schema_version: u32,
    pub candidate_strategy_id: StrategyId,
    pub candidate_version: String,
    pub candidate_lifecycle: StrategyLifecycle,
    pub candidate_digest: String,
    pub baseline_result_digests: Vec<String>,
    pub candidate_result_digests: Vec<String>,
    pub corpus_version: String,
    pub eval_version: String,
    pub context_digest: String,
    pub policy_digest: String,
    pub model_digest: String,
    pub tool_digest: String,
    pub paired_metrics: BTreeMap<String, f64>,
    pub passed: bool,
    pub decision: String,
    pub source_trajectory_ids: Vec<TrajectoryId>,
    pub integrity_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromotionApproval {
    pub schema_version: u32,
    pub candidate_strategy_id: StrategyId,
    pub evaluation_digest: String,
    pub approver_identity: String,
    pub approver_role: String,
    pub verified: bool,
    pub approved_at: DateTime<Utc>,
    /// Detached signature over the domain-separated candidate binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_key: Option<String>,
    pub integrity_digest: String,
}
#[derive(Serialize)]
struct EvaluationIntegrity<'a> {
    schema_version: u32,
    candidate_strategy_id: &'a StrategyId,
    candidate_version: &'a str,
    candidate_lifecycle: StrategyLifecycle,
    candidate_digest: &'a str,
    baseline_result_digests: &'a [String],
    candidate_result_digests: &'a [String],
    corpus_version: &'a str,
    eval_version: &'a str,
    context_digest: &'a str,
    policy_digest: &'a str,
    model_digest: &'a str,
    tool_digest: &'a str,
    paired_metrics: &'a BTreeMap<String, f64>,
    passed: bool,
    decision: &'a str,
    source_trajectory_ids: &'a [TrajectoryId],
}

#[derive(Serialize)]
struct ApprovalIntegrity<'a> {
    schema_version: u32,
    candidate_strategy_id: &'a StrategyId,
    evaluation_digest: &'a str,
    approver_identity: &'a str,
    approver_role: &'a str,
    verified: bool,
    approved_at: DateTime<Utc>,
    signature: &'a Option<String>,
    signer_key: &'a Option<String>,
}

fn sha256<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("promotion integrity payload is serializable");
    format!("{:x}", Sha256::digest(bytes))
}

/// Field list for [`PromotionEvaluation::new`].
#[derive(Debug, Clone)]
pub struct PromotionEvaluationFields {
    pub candidate_version: String,
    pub baseline_result_digests: Vec<String>,
    pub candidate_result_digests: Vec<String>,
    pub corpus_version: String,
    pub eval_version: String,
    pub context_digest: String,
    pub policy_digest: String,
    pub model_digest: String,
    pub tool_digest: String,
    pub paired_metrics: BTreeMap<String, f64>,
    pub passed: bool,
    pub decision: String,
}

impl PromotionEvaluation {
    pub fn new(candidate: &StrategyItem, fields: PromotionEvaluationFields) -> Self {
        let PromotionEvaluationFields {
            candidate_version,
            baseline_result_digests,
            candidate_result_digests,
            corpus_version,
            eval_version,
            context_digest,
            policy_digest,
            model_digest,
            tool_digest,
            paired_metrics,
            passed,
            decision,
        } = fields;
        let mut value = Self {
            schema_version: PROMOTION_EVALUATION_VERSION,
            candidate_strategy_id: candidate.id,
            candidate_version,
            candidate_lifecycle: candidate.lifecycle.unwrap_or(StrategyLifecycle::Active),
            candidate_digest: strategy_digest(candidate),
            baseline_result_digests,
            candidate_result_digests,
            corpus_version,
            eval_version,
            context_digest,
            policy_digest,
            model_digest,
            tool_digest,
            paired_metrics,
            passed,
            decision,
            source_trajectory_ids: candidate.source_trajectory_ids.clone(),
            integrity_digest: String::new(),
        };
        value.integrity_digest = value.recomputed_integrity_digest();
        value
    }

    pub fn recomputed_integrity_digest(&self) -> String {
        sha256(&EvaluationIntegrity {
            schema_version: self.schema_version,
            candidate_strategy_id: &self.candidate_strategy_id,
            candidate_version: &self.candidate_version,
            candidate_lifecycle: self.candidate_lifecycle,
            candidate_digest: &self.candidate_digest,
            baseline_result_digests: &self.baseline_result_digests,
            candidate_result_digests: &self.candidate_result_digests,
            corpus_version: &self.corpus_version,
            eval_version: &self.eval_version,
            context_digest: &self.context_digest,
            policy_digest: &self.policy_digest,
            model_digest: &self.model_digest,
            tool_digest: &self.tool_digest,
            paired_metrics: &self.paired_metrics,
            passed: self.passed,
            decision: &self.decision,
            source_trajectory_ids: &self.source_trajectory_ids,
        })
    }

    pub fn digest(&self) -> String {
        self.integrity_digest.clone()
    }

    pub fn validate_for(&self, candidate: &StrategyItem) -> Result<(), PromotionEvidenceError> {
        if self.schema_version != PROMOTION_EVALUATION_VERSION {
            return Err(PromotionEvidenceError::EvaluationVersion);
        }
        if self.candidate_strategy_id != candidate.id {
            return Err(PromotionEvidenceError::EvaluationCandidateId);
        }
        if self.candidate_lifecycle != StrategyLifecycle::Quarantined {
            return Err(PromotionEvidenceError::EvaluationNotQuarantined);
        }
        if self.candidate_digest != strategy_digest(candidate) {
            return Err(PromotionEvidenceError::EvaluationCandidateDigest);
        }
        if self.source_trajectory_ids.is_empty() || self.source_trajectory_ids != candidate.source_trajectory_ids {
            return Err(PromotionEvidenceError::EvaluationSourceTrajectories);
        }
        if self.baseline_result_digests.is_empty()
            || self.candidate_result_digests.is_empty()
            || self.baseline_result_digests.len() != self.candidate_result_digests.len()
        {
            return Err(PromotionEvidenceError::PairedResultsUnpaired);
        }
        let result_digests = self.baseline_result_digests.iter().chain(self.candidate_result_digests.iter());
        if result_digests.clone().any(|digest| digest.trim().is_empty()) {
            return Err(PromotionEvidenceError::PairedEmptyDigest);
        }
        if self.baseline_result_digests.iter().any(|digest| self.candidate_result_digests.contains(digest))
            || self.baseline_result_digests.iter().chain(self.candidate_result_digests.iter()).any(|digest| {
                self.baseline_result_digests.iter().filter(|other| *other == digest).count() > 1
                    || self.candidate_result_digests.iter().filter(|other| *other == digest).count() > 1
            })
        {
            return Err(PromotionEvidenceError::PairedDuplicateDigests);
        }
        for provenance in [&self.corpus_version, &self.eval_version, &self.context_digest, &self.policy_digest, &self.model_digest, &self.tool_digest] {
            if provenance.trim().is_empty() {
                return Err(PromotionEvidenceError::EvaluationProvenanceMissing);
            }
        }
        if self.paired_metrics.is_empty() || self.paired_metrics.values().any(|value| !value.is_finite()) {
            return Err(PromotionEvidenceError::PairedMetricsMissing);
        }
        let pairs = self.paired_metrics.get("pairs").copied().ok_or(PromotionEvidenceError::PairCountMissing)?;
        let uplift = self.paired_metrics.get("uplift").copied().ok_or(PromotionEvidenceError::UpliftMissing)?;
        let regressions = self.paired_metrics.get("regressions").copied().ok_or(PromotionEvidenceError::RegressionCountMissing)?;
        if pairs <= 0.0 || pairs != self.baseline_result_digests.len() as f64 || uplift <= 0.0 || regressions != 0.0 {
            return Err(PromotionEvidenceError::NoMeasuredUplift);
        }
        let derived_pass = pairs > 0.0 && uplift > 0.0 && regressions == 0.0;
        if self.passed != derived_pass || !matches!(self.decision.as_str(), "pass" | "promote" | "approved") {
            return Err(PromotionEvidenceError::DecisionMismatch);
        }
        if self.integrity_digest != self.recomputed_integrity_digest() {
            return Err(PromotionEvidenceError::EvaluationIntegrity);
        }
        Ok(())
    }
}

impl PromotionApproval {
    pub fn new(candidate_strategy_id: StrategyId, evaluation_digest: impl Into<String>, approver_identity: impl Into<String>, approver_role: impl Into<String>, verified: bool, approved_at: DateTime<Utc>) -> Self {
        let mut value = Self {
            schema_version: PROMOTION_APPROVAL_VERSION,
            candidate_strategy_id,
            evaluation_digest: evaluation_digest.into(),
            approver_identity: approver_identity.into(),
            approver_role: approver_role.into(),
            verified,
            approved_at,
            signature: None,
            signer_key: None,
            integrity_digest: String::new(),
        };
        value.integrity_digest = value.recomputed_integrity_digest();
        value
    }

    pub fn recomputed_integrity_digest(&self) -> String {
        sha256(&ApprovalIntegrity {
            schema_version: self.schema_version,
            candidate_strategy_id: &self.candidate_strategy_id,
            evaluation_digest: &self.evaluation_digest,
            approver_identity: &self.approver_identity,
            approver_role: &self.approver_role,
            verified: self.verified,
            approved_at: self.approved_at,
            signature: &self.signature,
            signer_key: &self.signer_key,
        })
    }


    pub fn validate_for(&self, candidate: &StrategyItem, evaluation: &PromotionEvaluation) -> Result<(), PromotionEvidenceError> {
        if self.schema_version != PROMOTION_APPROVAL_VERSION {
            return Err(PromotionEvidenceError::ApprovalVersion);
        }
        if self.signature.as_ref().is_none_or(std::string::String::is_empty) || self.signer_key.as_ref().is_none_or(std::string::String::is_empty) {
            return Err(PromotionEvidenceError::ApprovalSignatureRequired);
        }
        if self.candidate_strategy_id != candidate.id || self.evaluation_digest != evaluation.digest() {
            return Err(PromotionEvidenceError::ApprovalNotBound);
        }
        if self.approver_identity.trim().is_empty() || self.approver_role != "human" {
            return Err(PromotionEvidenceError::ApproverNotHuman);
        }
        if self.signer_key.as_deref() != Some(self.approver_identity.as_str()) {
            return Err(PromotionEvidenceError::ApprovalActorMismatch);
        }
        if self.integrity_digest != self.recomputed_integrity_digest() {
            return Err(PromotionEvidenceError::ApprovalIntegrity);
        }
        Ok(())
    }

    pub fn verify_signature(&self, candidate: &StrategyItem, allowed_signers: &Path) -> Result<(), PromotionEvidenceError> {
        if self.signature.is_none() || self.signer_key.is_none() {
            return Err(PromotionEvidenceError::ApprovalSignatureMissing);
        }
        let payload = canon_model::approval::approval_payload_bytes(&canon_model::ApprovalPayload {
            namespace: "canon-learning-approval-v1",
            subject: &format!("strategy:{}", candidate.id),
            project: None,
            artifact_sha: &self.evaluation_digest,
            run_id: None,
            surface: &[],
            effects: &[],
            actor: &self.approver_identity,
            timestamp: &self.approved_at,
        });
        Ok(canon_gate::verify_ssh_signature(
            "canon-learning-approval-v1",
            &payload,
            self.signature.as_ref().expect("checked above").as_bytes(),
            self.signer_key.as_ref().expect("checked above"),
            allowed_signers,
        )?)
    }
}

pub fn strategy_digest(candidate: &StrategyItem) -> String {
    sha256(&serde_json::json!({
        "id": candidate.id,
        "regime_key": candidate.regime_key,
        "role": candidate.role,
        "title": candidate.title,
        "description": candidate.description,
        "content": candidate.content,
        "source_trajectory_ids": candidate.source_trajectory_ids,
        "recorded_at": candidate.recorded_at,
    }))
}


#[cfg(test)]
mod tests {
    use canon_model::ids::{regime_key, RegimeKey, RoleId};

    use super::*;
    use crate::strategy::StrategyItemFields;

    fn candidate() -> StrategyItem {
        let rk = RegimeKey::parse(regime_key("dev", "canon", "join-spine", "9c93d024b1a2")).unwrap();
        StrategyItem::new(StrategyItemFields { id: StrategyId::new(), regime_key: rk, role: RoleId::parse("dev").unwrap(), title: "title".into(), description: "description".into(), content: "content".into(), source_trajectory_ids: vec![TrajectoryId::new()], recorded_at: Utc::now() })
            .with_lifecycle(StrategyLifecycle::Quarantined)
    }

    fn evaluation(candidate: &StrategyItem) -> PromotionEvaluation {
        let metrics = BTreeMap::from([("pairs".to_string(), 1.0), ("uplift".to_string(), 0.1), ("regressions".to_string(), 0.0)]);
        PromotionEvaluation::new(candidate, PromotionEvaluationFields { candidate_version: "v2".into(), baseline_result_digests: vec!["b0".into()], candidate_result_digests: vec!["c0".into()], corpus_version: "corpus".into(), eval_version: "eval".into(), context_digest: "ctx".into(), policy_digest: "policy".into(), model_digest: "model".into(), tool_digest: "tool".into(), paired_metrics: metrics, passed: true, decision: "pass".into() })
    }

    #[test]
    fn evaluation_validate_for_keeps_its_messages() {
        let candidate = candidate();
        evaluation(&candidate).validate_for(&candidate).unwrap();

        let mut wrong_version = evaluation(&candidate);
        wrong_version.schema_version = 0;
        assert_eq!(wrong_version.validate_for(&candidate).unwrap_err().to_string(), "unsupported promotion evaluation version");

        let other = self::candidate();
        assert_eq!(evaluation(&candidate).validate_for(&other).unwrap_err().to_string(), "evaluation candidate strategy id does not match");

        let mut active = evaluation(&candidate);
        active.candidate_lifecycle = StrategyLifecycle::Active;
        assert_eq!(active.validate_for(&candidate).unwrap_err().to_string(), "evaluation candidate lifecycle is not quarantined");

        let mut no_pairs = evaluation(&candidate);
        no_pairs.baseline_result_digests.clear();
        assert_eq!(no_pairs.validate_for(&candidate).unwrap_err().to_string(), "paired evaluation result provenance is missing or unpaired");

        let mut tampered = evaluation(&candidate);
        tampered.decision = "rejected".into();
        assert_eq!(tampered.validate_for(&candidate).unwrap_err().to_string(), "evaluation decision does not match measured paired evidence");

        let mut stale = evaluation(&candidate);
        stale.integrity_digest = "stale".into();
        assert_eq!(stale.validate_for(&candidate).unwrap_err().to_string(), "promotion evaluation integrity digest is invalid");
    }

    #[test]
    fn approval_validate_for_and_verify_signature_keep_their_messages() {
        let candidate = candidate();
        let evaluation = evaluation(&candidate);
        let approval = || PromotionApproval::new(candidate.id, evaluation.digest(), "alice", "human", true, Utc::now());

        assert_eq!(approval().validate_for(&candidate, &evaluation).unwrap_err().to_string(), "promotion approval signature and signer key are required");
        assert_eq!(approval().verify_signature(&candidate, Path::new("signers")).unwrap_err().to_string(), "promotion approval signature is missing");

        let mut signed = approval();
        signed.signature = Some("sig".into());
        signed.signer_key = Some("bob".into());
        assert_eq!(signed.validate_for(&candidate, &evaluation).unwrap_err().to_string(), "signed approval actor must equal the approval principal");

        signed.signer_key = Some("alice".into());
        signed.evaluation_digest = "elsewhere".into();
        assert_eq!(signed.validate_for(&candidate, &evaluation).unwrap_err().to_string(), "promotion approval is not bound to this candidate evaluation");

        let mut machine = approval();
        machine.signature = Some("sig".into());
        machine.signer_key = Some("alice".into());
        machine.approver_role = "agent".into();
        assert_eq!(machine.validate_for(&candidate, &evaluation).unwrap_err().to_string(), "promotion approval requires a nonempty human approver");

        let mut stale = approval();
        stale.signature = Some("sig".into());
        stale.signer_key = Some("alice".into());
        assert_eq!(stale.validate_for(&candidate, &evaluation).unwrap_err().to_string(), "promotion approval integrity digest is invalid");

        let mut bad_version = approval();
        bad_version.schema_version = 0;
        assert_eq!(bad_version.validate_for(&candidate, &evaluation).unwrap_err().to_string(), "unsupported promotion approval version");

        let mut unsigned_path = approval();
        unsigned_path.signature = Some("sig".into());
        unsigned_path.signer_key = Some("alice".into());
        let err = unsigned_path.verify_signature(&candidate, Path::new("/nonexistent/allowed_signers")).unwrap_err();
        assert_eq!(err.to_string(), "allowed signers file is missing: /nonexistent/allowed_signers");
        assert!(std::error::Error::source(&err).is_none(), "transparent wrapper forwards the inner source, which has none");
    }
}
