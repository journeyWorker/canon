//! The review→verdict mapping table (S4 design §5, reproduced verbatim
//! in `openspec/changes/s4-artifact-ingest/specs/review-verdict-mapping/
//! spec.md`) — a pure, table-driven function from a normalized
//! [`crate::artifact_adapter::ArtifactEventKind`] to an optional verdict
//! `{role, polarity, becomes}` (task 5.1). Table-driven, never
//! per-adapter logic (design D5) — every wave-2 adapter calls the SAME
//! [`derive_verdict`], never re-implements a row of this table itself.
//!
//! `regime_key` attachment (task 5.2) is a separate step ([`attach_regime_key`])
//! layered on top of [`derive_verdict`]'s bare [`VerdictRow`] — the
//! `canon_model::ids::regime_key` free function is the single canonical
//! serialization S4/S6/S7/S8 all reuse; this module only calls it, it
//! does not redefine it.
//!
//! **S15 P4 addendum — native verdict derivation (design D7).** The
//! table above is the S4 raw-artifact path's ONLY dispatch; it is
//! FROZEN and untouched by this addendum. [`derive_native_review_verdict`]
//! and [`derive_native_divergence_verdict`] are a SEPARATE, explicit
//! path for canon's own native `Review`/`Divergence` records (never
//! routed through [`derive_verdict`]'s `ReviewPromotion`/
//! `RemediationResolved` rows, whose role handling is shaped around a
//! ledger-sourced artifact, not a native record's own
//! `envelope.actor.role`) — see
//! `crate::artifact_adapters::{review, native_divergence}`, the two
//! handle-based records-source adapters that emit the events these
//! functions derive verdicts from.

use canon_model::ids::{JoinKeyError, RegimeKey, RoleId, regime_key};
use canon_model::records::DivergenceStatus;

use crate::artifact_adapter::{ArtifactEventKind, ArtifactJoinKey};

/// Which direction a verdict points (design §5 S4's "Polarity" column).
/// `Corrective` is distinct from `Success`/`Failure` — a clear-record
/// after `@flagged` (table row 4) is neither "the work was good" nor
/// "the work failed"; it is evidence the REVIEW PROCESS caught
/// something, credited to the `review` role, not the authoring role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Polarity {
    Failure,
    Success,
    Corrective,
}

impl Polarity {
    pub fn as_str(self) -> &'static str {
        match self {
            Polarity::Failure => "failure",
            Polarity::Success => "success",
            Polarity::Corrective => "corrective",
        }
    }
}

/// What a verdict "becomes" downstream (design §5 S4's "Becomes"
/// column) — S6's distillation input, never computed here (S4 stops at
/// emitting the verdict record, design Non-Goals).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Becomes {
    GuardrailCandidate,
    StrategyCandidate,
    /// Table row 4's distinct text: "guardrail (what the sample
    /// caught)" — kept as its own variant rather than collapsed into
    /// `GuardrailCandidate` because its provenance (a review PROCESS
    /// catching a flagged sample) differs from an open review finding.
    GuardrailWhatTheSampleCaught,
}

impl Becomes {
    /// The design table's own literal cell text — the wire/display
    /// form, exactly as `proposal.md`/`design.md`/the spec quote it.
    pub fn as_str(self) -> &'static str {
        match self {
            Becomes::GuardrailCandidate => "guardrail candidate",
            Becomes::StrategyCandidate => "strategy candidate",
            Becomes::GuardrailWhatTheSampleCaught => "guardrail (what the sample caught)",
        }
    }
}

/// One row of the review→verdict mapping table — the bare
/// `{role, polarity, becomes}` [`derive_verdict`] returns for a mapped
/// [`ArtifactEventKind`], before `regime_key`/join-key attachment
/// (task 5.2, [`attach_regime_key`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerdictRow {
    pub role: RoleId,
    pub polarity: Polarity,
    pub becomes: Becomes,
}

/// A verdict with its `regime_key` attached (design §5 S4: "Severity +
/// area tags on the source artifact become regime-key components…so a
/// verdict is retrievable by the same key S6/S7 read at strategy-lookup
/// time") — the shape a wave-2 adapter emits after calling
/// [`attach_regime_key`] on its [`derive_verdict`] output plus the
/// source event's join key and a trust-level passthrough (task 5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub join_key: ArtifactJoinKey,
    pub row: VerdictRow,
    pub regime_key: RegimeKey,
    /// Passthrough trust-level tag (`@reviewed`/`@ratified`, task 5.3)
    /// — never collapsed into a single "success" bucket (design Risk:
    /// "S4 emits the verdict with whatever trust-level tag the source
    /// record carries as a passthrough field… S6/S7's statistical
    /// promotion is where trust-weighting actually happens").
    pub trust_level: Option<String>,
}

fn role(slug: &'static str) -> RoleId {
    RoleId::parse(slug).unwrap_or_else(|e| panic!("built-in role slug {slug:?} must be a valid RoleId: {e}"))
}

/// The pure, table-driven review→verdict mapping (task 5.1, design §5
/// S4, reproduced verbatim in `specs/review-verdict-mapping/spec.md`).
///
/// `authoring_role` is consulted ONLY for [`ArtifactEventKind::ReviewPromotion`]
/// (table row 3: "role = the authoring role of the scenario" — the one
/// row whose role is not a fixed constant). When it is `None` for that
/// kind, this function returns `None` rather than guessing a role —
/// synthesizing a verdict with a fabricated role would violate the same
/// "malformed evidence is no evidence" principle design D4 states for
/// the openspec task-state adapter, applied here to a missing role
/// instead of missing PR/CI evidence.
///
/// Returns `None` for [`ArtifactEventKind::NonVerdict`] and for
/// `ReviewPromotion` with no derivable `authoring_role` — every other
/// variant always maps to exactly one row.
pub fn derive_verdict(kind: ArtifactEventKind, authoring_role: Option<&RoleId>) -> Option<VerdictRow> {
    match kind {
        ArtifactEventKind::CodeReviewFinding => {
            Some(VerdictRow { role: role("dev"), polarity: Polarity::Failure, becomes: Becomes::GuardrailCandidate })
        }
        ArtifactEventKind::DesignReviewFinding => {
            Some(VerdictRow { role: role("design"), polarity: Polarity::Failure, becomes: Becomes::GuardrailCandidate })
        }
        ArtifactEventKind::ReviewPromotion => {
            authoring_role.map(|r| VerdictRow { role: r.clone(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate })
        }
        ArtifactEventKind::ClearAfterFlagged => {
            Some(VerdictRow { role: role("review"), polarity: Polarity::Corrective, becomes: Becomes::GuardrailWhatTheSampleCaught })
        }
        ArtifactEventKind::RemediationResolved => {
            Some(VerdictRow { role: role("dev"), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate })
        }
        ArtifactEventKind::CiFailOrPrRevert => {
            Some(VerdictRow { role: role("dev"), polarity: Polarity::Failure, becomes: Becomes::GuardrailCandidate })
        }
        ArtifactEventKind::PrMergeNoRevert => {
            Some(VerdictRow { role: role("dev"), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate })
        }
        ArtifactEventKind::NonVerdict => None,
    }
}

/// Native verdict derivation for a `Review` record (S15 P4, design
/// D7) — deliberately NOT routed through [`derive_verdict`]: a
/// `Review` record's mere existence IS the positive verdict (there is
/// no status field to branch on — `canon_model::records::Review` is a
/// promotion, full stop), and its role is ALWAYS
/// `envelope.actor.role` (spec `native-record-flywheel` Requirement
/// 2), never `derive_verdict`'s `ReviewPromotion` row (whose
/// "authoring role of the scenario" semantics belong to the ledger
/// path, not this native one). A caller with no derivable
/// `authoring_role` (`envelope.actor.role == None`) does not call this
/// function at all — it skips and counts the event instead, mirroring
/// `derive_verdict`'s own missing-role discipline (this function's
/// signature takes `&RoleId`, not `Option<&RoleId>`, precisely so a
/// missing role can never be silently coerced into one here).
pub fn derive_native_review_verdict(actor_role: &RoleId) -> VerdictRow {
    VerdictRow { role: actor_role.clone(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate }
}

/// Native verdict derivation for a `Divergence` record (S15 P4,
/// design D7) — the status → verdict table:
///
/// | `DivergenceStatus` | `Polarity` | `Becomes` |
/// |---|---|---|
/// | `Resolved` | `Success` | `StrategyCandidate` |
/// | `StillDivergent` | `Failure` | `GuardrailCandidate` |
/// | `Open` | — | `None` (freshly staged, in-flight — no terminal verdict yet) |
/// | `Deferred { .. }` | — | `None` (intentionally deferred — no terminal verdict) |
///
/// `role` is ALWAYS `actor_role` regardless of status (spec
/// Requirement 2) — never `derive_verdict`'s `RemediationResolved`
/// row's hard-coded `dev` constant: a native `Divergence`'s author may
/// be any registered role, and this path exists precisely so that
/// role survives rather than being papered over by a fixed constant.
pub fn derive_native_divergence_verdict(status: &DivergenceStatus, actor_role: &RoleId) -> Option<VerdictRow> {
    match status {
        DivergenceStatus::Resolved => Some(VerdictRow { role: actor_role.clone(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate }),
        DivergenceStatus::StillDivergent => {
            Some(VerdictRow { role: actor_role.clone(), polarity: Polarity::Failure, becomes: Becomes::GuardrailCandidate })
        }
        DivergenceStatus::Open | DivergenceStatus::Deferred { .. } => None,
    }
}

/// Attach a `regime_key` to a [`VerdictRow`] (task 5.2) — calls
/// `canon_model::ids::regime_key` (the single canonical serialization,
/// canon-model foundation) with this row's role, the caller-supplied
/// `repo`/`area`/`hash`, and validates the result through
/// [`RegimeKey::parse`] so a caller can never emit an unvalidated key.
/// `join_key`/`trust_level` are copied through verbatim.
pub fn attach_regime_key(
    row: VerdictRow,
    join_key: ArtifactJoinKey,
    repo: &str,
    area: &str,
    hash: &str,
    trust_level: Option<String>,
) -> Result<Verdict, JoinKeyError> {
    let key = regime_key(row.role.as_str(), repo, area, hash);
    let regime_key = RegimeKey::parse(key)?;
    Ok(Verdict { join_key, row, regime_key, trust_level })
}

#[cfg(test)]
mod tests {
    use canon_model::ids::ScenarioId;

    use super::*;

    #[test]
    fn code_review_finding_becomes_dev_guardrail_candidate() {
        let row = derive_verdict(ArtifactEventKind::CodeReviewFinding, None).unwrap();
        assert_eq!(row.role.as_str(), "dev");
        assert_eq!(row.polarity, Polarity::Failure);
        assert_eq!(row.becomes, Becomes::GuardrailCandidate);
    }

    #[test]
    fn design_review_finding_becomes_design_guardrail_candidate() {
        let row = derive_verdict(ArtifactEventKind::DesignReviewFinding, None).unwrap();
        assert_eq!(row.role.as_str(), "design");
        assert_eq!(row.polarity, Polarity::Failure);
        assert_eq!(row.becomes, Becomes::GuardrailCandidate);
    }

    #[test]
    fn review_promotion_becomes_authoring_role_success() {
        let authoring = RoleId::parse("content").unwrap();
        let row = derive_verdict(ArtifactEventKind::ReviewPromotion, Some(&authoring)).unwrap();
        assert_eq!(row.role.as_str(), "content");
        assert_eq!(row.polarity, Polarity::Success);
        assert_eq!(row.becomes, Becomes::StrategyCandidate);
    }

    #[test]
    fn review_promotion_with_no_authoring_role_yields_no_verdict() {
        // Fabricating a role would be exactly the "malformed evidence is
        // no evidence" violation design D4 names for a different field.
        assert!(derive_verdict(ArtifactEventKind::ReviewPromotion, None).is_none());
    }

    #[test]
    fn clear_after_flagged_becomes_review_corrective_guardrail() {
        let row = derive_verdict(ArtifactEventKind::ClearAfterFlagged, None).unwrap();
        assert_eq!(row.role.as_str(), "review");
        assert_eq!(row.polarity, Polarity::Corrective);
        assert_eq!(row.becomes, Becomes::GuardrailWhatTheSampleCaught);
        assert_eq!(row.becomes.as_str(), "guardrail (what the sample caught)");
    }

    #[test]
    fn remediation_resolved_becomes_dev_strategy_candidate() {
        let row = derive_verdict(ArtifactEventKind::RemediationResolved, None).unwrap();
        assert_eq!(row.role.as_str(), "dev");
        assert_eq!(row.polarity, Polarity::Success);
        assert_eq!(row.becomes, Becomes::StrategyCandidate);
    }

    #[test]
    fn ci_fail_or_pr_revert_becomes_dev_failure_guardrail() {
        let row = derive_verdict(ArtifactEventKind::CiFailOrPrRevert, None).unwrap();
        assert_eq!(row.role.as_str(), "dev");
        assert_eq!(row.polarity, Polarity::Failure);
        assert_eq!(row.becomes, Becomes::GuardrailCandidate);
    }

    #[test]
    fn pr_merge_no_revert_becomes_dev_success_strategy_candidate() {
        let row = derive_verdict(ArtifactEventKind::PrMergeNoRevert, None).unwrap();
        assert_eq!(row.role.as_str(), "dev");
        assert_eq!(row.polarity, Polarity::Success);
        assert_eq!(row.becomes, Becomes::StrategyCandidate);
    }

    /// D2 (manifest bookkeeping), D3 (handoff transition alone), D4
    /// (prose-only/deferred/dropped task flip) — every explicit
    /// non-verdict case collapses to `NonVerdict`, which never
    /// synthesizes a row.
    #[test]
    fn non_verdict_kind_yields_no_verdict() {
        assert!(derive_verdict(ArtifactEventKind::NonVerdict, None).is_none());
        // Even with an authoring role available, a non-verdict kind
        // still never produces one — `authoring_role` is only consulted
        // for `ReviewPromotion`.
        let authoring = RoleId::parse("dev").unwrap();
        assert!(derive_verdict(ArtifactEventKind::NonVerdict, Some(&authoring)).is_none());
    }

    #[test]
    fn attach_regime_key_shares_prefix_for_same_role_repo_area() {
        let scenario = ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap();
        let join_key = ArtifactJoinKey::Scenario(scenario);
        let row = derive_verdict(ArtifactEventKind::CodeReviewFinding, None).unwrap();
        let verdict_a = attach_regime_key(row.clone(), join_key.clone(), "canon", "world", "abcdef", Some("@reviewed".to_string())).unwrap();
        let verdict_b = attach_regime_key(row, join_key, "canon", "world", "123456", None).unwrap();
        assert_eq!(verdict_a.regime_key.role(), "dev");
        assert!(verdict_a.regime_key.as_str().starts_with("dev/canon/world/"));
        assert!(verdict_b.regime_key.as_str().starts_with("dev/canon/world/"));
        assert_eq!(verdict_a.trust_level.as_deref(), Some("@reviewed"));
        assert_eq!(verdict_b.trust_level, None);
    }

    #[test]
    fn native_review_verdict_is_always_success_strategy_candidate_with_actor_role() {
        let actor = RoleId::parse("content").unwrap();
        let row = derive_native_review_verdict(&actor);
        assert_eq!(row.role.as_str(), "content");
        assert_eq!(row.polarity, Polarity::Success);
        assert_eq!(row.becomes, Becomes::StrategyCandidate);
    }

    #[test]
    fn native_review_verdict_passes_through_a_non_dev_role_unchanged() {
        // The whole point of the native path (design D7 spec
        // Requirement 2): unlike `derive_verdict`'s hard-coded
        // constants, the role is exactly the actor's own role, never a
        // fixed "dev"/"design"/"review" slug.
        let actor = RoleId::parse("qa").unwrap();
        let row = derive_native_review_verdict(&actor);
        assert_eq!(row.role.as_str(), "qa");
    }

    #[test]
    fn native_divergence_resolved_becomes_success_strategy_candidate() {
        let actor = RoleId::parse("dev").unwrap();
        let row = derive_native_divergence_verdict(&DivergenceStatus::Resolved, &actor).unwrap();
        assert_eq!(row.role.as_str(), "dev");
        assert_eq!(row.polarity, Polarity::Success);
        assert_eq!(row.becomes, Becomes::StrategyCandidate);
    }

    #[test]
    fn native_divergence_still_divergent_becomes_failure_guardrail_candidate() {
        let actor = RoleId::parse("dev").unwrap();
        let row = derive_native_divergence_verdict(&DivergenceStatus::StillDivergent, &actor).unwrap();
        assert_eq!(row.polarity, Polarity::Failure);
        assert_eq!(row.becomes, Becomes::GuardrailCandidate);
    }

    #[test]
    fn native_divergence_open_yields_no_verdict() {
        // Freshly staged, in-flight — no terminal verdict yet.
        let actor = RoleId::parse("dev").unwrap();
        assert!(derive_native_divergence_verdict(&DivergenceStatus::Open, &actor).is_none());
    }

    #[test]
    fn native_divergence_deferred_yields_no_verdict() {
        // Intentionally deferred — no terminal verdict either.
        let actor = RoleId::parse("dev").unwrap();
        let expiry: chrono::DateTime<chrono::Utc> = "2026-08-01T00:00:00Z".parse().unwrap();
        let status = DivergenceStatus::Deferred { reason: "waiting on design".to_string(), expiry };
        assert!(derive_native_divergence_verdict(&status, &actor).is_none());
    }

    #[test]
    fn native_divergence_role_is_always_the_actor_role_never_a_fixed_dev_constant() {
        // Unlike `derive_verdict`'s `RemediationResolved` row (fixed
        // `role("dev")`), the native path's role is the actor's own —
        // here a non-dev role proves it is not silently coerced.
        let actor = RoleId::parse("content").unwrap();
        let row = derive_native_divergence_verdict(&DivergenceStatus::Resolved, &actor).unwrap();
        assert_eq!(row.role.as_str(), "content");
    }
}
