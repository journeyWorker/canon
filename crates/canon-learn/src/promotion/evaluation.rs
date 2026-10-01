use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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

impl PromotionEvaluation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        candidate: &StrategyItem,
        candidate_version: impl Into<String>,
        baseline_result_digests: Vec<String>,
        candidate_result_digests: Vec<String>,
        corpus_version: impl Into<String>,
        eval_version: impl Into<String>,
        context_digest: impl Into<String>,
        policy_digest: impl Into<String>,
        model_digest: impl Into<String>,
        tool_digest: impl Into<String>,
        paired_metrics: BTreeMap<String, f64>,
        passed: bool,
        decision: impl Into<String>,
    ) -> Self {
        let mut value = Self {
            schema_version: PROMOTION_EVALUATION_VERSION,
            candidate_strategy_id: candidate.id,
            candidate_version: candidate_version.into(),
            candidate_lifecycle: candidate.lifecycle.unwrap_or(StrategyLifecycle::Active),
            candidate_digest: strategy_digest(candidate),
            baseline_result_digests,
            candidate_result_digests,
            corpus_version: corpus_version.into(),
            eval_version: eval_version.into(),
            context_digest: context_digest.into(),
            policy_digest: policy_digest.into(),
            model_digest: model_digest.into(),
            tool_digest: tool_digest.into(),
            paired_metrics,
            passed,
            decision: decision.into(),
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

    pub fn validate_for(&self, candidate: &StrategyItem) -> Result<(), String> {
        if self.schema_version != PROMOTION_EVALUATION_VERSION {
            return Err("unsupported promotion evaluation version".into());
        }
        if self.candidate_strategy_id != candidate.id {
            return Err("evaluation candidate strategy id does not match".into());
        }
        if self.candidate_lifecycle != StrategyLifecycle::Quarantined {
            return Err("evaluation candidate lifecycle is not quarantined".into());
        }
        if self.candidate_digest != strategy_digest(candidate) {
            return Err("evaluation candidate digest does not match strategy".into());
        }
        if self.source_trajectory_ids.is_empty() || self.source_trajectory_ids != candidate.source_trajectory_ids {
            return Err("evaluation source trajectory provenance is missing or mismatched".into());
        }
        if self.baseline_result_digests.is_empty()
            || self.candidate_result_digests.is_empty()
            || self.baseline_result_digests.len() != self.candidate_result_digests.len()
        {
            return Err("paired evaluation result provenance is missing or unpaired".into());
        }
        let result_digests = self.baseline_result_digests.iter().chain(self.candidate_result_digests.iter());
        if result_digests.clone().any(|digest| digest.trim().is_empty()) {
            return Err("paired evaluation contains an empty result digest".into());
        }
        if self.baseline_result_digests.iter().any(|digest| self.candidate_result_digests.contains(digest))
            || self.baseline_result_digests.iter().chain(self.candidate_result_digests.iter()).any(|digest| {
                self.baseline_result_digests.iter().filter(|other| *other == digest).count() > 1
                    || self.candidate_result_digests.iter().filter(|other| *other == digest).count() > 1
            })
        {
            return Err("paired evaluation result digests are duplicated or shared across variants".into());
        }
        for provenance in [&self.corpus_version, &self.eval_version, &self.context_digest, &self.policy_digest, &self.model_digest, &self.tool_digest] {
            if provenance.trim().is_empty() {
                return Err("evaluation provenance is missing".into());
            }
        }
        if self.paired_metrics.is_empty() || self.paired_metrics.values().any(|value| !value.is_finite()) {
            return Err("paired evaluation metrics are missing or unknown".into());
        }
        let pairs = self.paired_metrics.get("pairs").copied().ok_or("paired evaluation pair count is missing")?;
        let uplift = self.paired_metrics.get("uplift").copied().ok_or("paired evaluation uplift is missing")?;
        let regressions = self.paired_metrics.get("regressions").copied().ok_or("paired evaluation regression count is missing")?;
        if pairs <= 0.0 || pairs != self.baseline_result_digests.len() as f64 || uplift <= 0.0 || regressions != 0.0 {
            return Err("paired evaluation is not a positive, non-regressing measured uplift".into());
        }
        let derived_pass = pairs > 0.0 && uplift > 0.0 && regressions == 0.0;
        if self.passed != derived_pass || !matches!(self.decision.as_str(), "pass" | "promote" | "approved") {
            return Err("evaluation decision does not match measured paired evidence".into());
        }
        if self.integrity_digest != self.recomputed_integrity_digest() {
            return Err("promotion evaluation integrity digest is invalid".into());
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


    pub fn validate_for(&self, candidate: &StrategyItem, evaluation: &PromotionEvaluation) -> Result<(), String> {
        if self.schema_version != PROMOTION_APPROVAL_VERSION {
            return Err("unsupported promotion approval version".into());
        }
        if self.signature.as_ref().map_or(true, |value| value.is_empty()) || self.signer_key.as_ref().map_or(true, |value| value.is_empty()) {
            return Err("promotion approval signature and signer key are required".into());
        }
        if self.candidate_strategy_id != candidate.id || self.evaluation_digest != evaluation.digest() {
            return Err("promotion approval is not bound to this candidate evaluation".into());
        }
        if self.approver_identity.trim().is_empty() || self.approver_role != "human" {
            return Err("promotion approval requires a nonempty human approver".into());
        }
        if self.signer_key.as_deref() != Some(self.approver_identity.as_str()) {
            return Err("signed approval actor must equal the approval principal".into());
        }
        if self.integrity_digest != self.recomputed_integrity_digest() {
            return Err("promotion approval integrity digest is invalid".into());
        }
        Ok(())
    }

    pub fn verify_signature(&self, candidate: &StrategyItem, allowed_signers: &Path) -> Result<(), String> {
        if self.signature.is_none() || self.signer_key.is_none() {
            return Err("promotion approval signature is missing".into());
        }
        let payload = canon_model::approval::approval_payload_bytes(
            "canon-learning-approval-v1",
            &format!("strategy:{}", candidate.id),
            None,
            &self.evaluation_digest,
            None,
            &[],
            &[],
            &self.approver_identity,
            &self.approved_at,
        );
        canon_gate::verify_ssh_signature(
            "canon-learning-approval-v1",
            &payload,
            self.signature.as_ref().expect("checked above").as_bytes(),
            self.signer_key.as_ref().expect("checked above"),
            allowed_signers,
        )
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

