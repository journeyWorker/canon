//! The trust ladder (design D21, design decision 2): a closed
//! three-rung lifecycle (`draft | reviewed | ratified`) plus an
//! orthogonal, human-only `flagged` overlay, and the REQUIRED-trust
//! vocabulary `policy.yaml`'s `trust_required`/`trust_sample` fields
//! reference. Generalizes `tools/parity.py`'s `_trust_level` classifier
//! (the donor parity-harness audit's trust-ladder notes §3.1) —
//! "the smallest possible implementation of a multi-rung completion
//! ladder that is still impossible to game: no rung above `Draft` is
//! reachable by [`canon_model::TrustLifecycle`] alone;
//! [`TrustRung::Agent`] additionally requires a review-record."
//!
//! # s15 P1: TrustLifecycle/FlaggedOverlay moved to canon-model
//! [`canon_model::TrustLifecycle`]/[`canon_model::FlaggedOverlay`] used
//! to live here as canon-gate-owned, wire-compatible companion types
//! (this crate's territory once excluded editing `canon-model`
//! directly). s15 P1 (the `s15-spec-ledger-unification` change)
//! moved both DATA type definitions natively onto
//! `canon_model::EvidenceRecord` (`lifecycle: Option<TrustLifecycle>`,
//! `flagged: Option<FlaggedOverlay>`) — this module now only
//! RE-EXPORTS them (via the `use` below) so [`TrustLadderState`]/
//! [`TrustRung`]/[`TrustLevel`] keep compiling unchanged. Reading those
//! native fields off `ctx.evidence` (instead of the raw-JSON
//! `trust_ladder` companion `crate::trust::trust_ladder_tag_of` still
//! scans) and deleting that now-redundant re-scan is a LATER wave
//! (design sequencing P3b) — NOT done here.

use canon_model::{FlaggedOverlay, TrustLifecycle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `policy.yaml`'s `trust_required`/`trust_sample` REQUIRED-level
/// vocabulary (D21's `TRUST_RANK`, minus `none` — `none` is never a
/// valid *requirement*, only a possible *achieved* rung with no green
/// mapping at all, see [`TrustRung::green`]). `Ord` follows
/// declaration order (`Agent < Human`), mirroring `TRUST_RANK`'s
/// `{"agent": 1, "human": 2}` so a release check can compare an
/// achieved level against a required one with a plain `<`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TrustLevel {
    Agent,
    Human,
}

impl TrustLevel {
    pub const ALL: [TrustLevel; 2] = [TrustLevel::Agent, TrustLevel::Human];

    pub fn as_str(self) -> &'static str {
        match self {
            TrustLevel::Agent => "agent",
            TrustLevel::Human => "human",
        }
    }

    pub fn from_str_exact(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }
}

/// The achieved trust rung for one artifact — the classifier
/// trust-ladder.md §3.1 names as the direct S5 lift. `green(&self)` is
/// kept as a SEPARATE question from classification itself ("classify,
/// then separately decide if this counts as green") so a stricter
/// caller (a future release-profile check) can reuse the identical
/// classifier without touching it (trust-ladder.md pattern 3.1's own
/// two-step design).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TrustRung {
    /// `TrustLifecycle::Draft` — spec.md "draft is never green": no
    /// matching evidence record, however faithful, moves this rung.
    Draft,
    /// `TrustLifecycle::Reviewed` with no matching ledger
    /// review-record — spec.md "reviewed without a review-record is a
    /// violation" (`FailureClass::UnreviewedPromotion`).
    UnreviewedPromotion,
    /// The human-only overlay is set — spec.md "flagged overrides
    /// passing evidence"; checked FIRST in [`TrustRung::classify`], so
    /// a flagged + ratified artifact still reports here, never
    /// [`TrustRung::Human`].
    Flagged,
    /// `TrustLifecycle::Reviewed` WITH a matching review-record —
    /// green at [`TrustLevel::Agent`] (design's own framing: "draft →
    /// reviewed (agent + review-record) → ratified (human)").
    Agent,
    /// `TrustLifecycle::Ratified` — green at [`TrustLevel::Human`].
    Human,
}

impl TrustRung {
    /// Classify one artifact's rung (design D21 / spec.md's lifecycle
    /// scenarios). Pure: no I/O, no corpus scan — `has_review_record`
    /// is the ONE piece of external evidence this function consults,
    /// supplied by the caller (S5 wave-2's verdict-ledger check
    /// resolves it from the ledger; this function never reads a
    /// ledger itself, mirroring `_trust_level(s, review_index)`'s own
    /// "index passed in, not looked up" shape).
    pub fn classify(lifecycle: TrustLifecycle, has_review_record: bool, flagged: bool) -> Self {
        if flagged {
            return TrustRung::Flagged;
        }
        match lifecycle {
            TrustLifecycle::Draft => TrustRung::Draft,
            TrustLifecycle::Reviewed if !has_review_record => TrustRung::UnreviewedPromotion,
            TrustLifecycle::Reviewed => TrustRung::Agent,
            TrustLifecycle::Ratified => TrustRung::Human,
        }
    }

    /// Whether this rung counts as green at all, and at which
    /// [`TrustLevel`] — `None` for `Draft`/`UnreviewedPromotion`/
    /// `Flagged` (spec.md: never green).
    pub fn green(self) -> Option<TrustLevel> {
        match self {
            TrustRung::Agent => Some(TrustLevel::Agent),
            TrustRung::Human => Some(TrustLevel::Human),
            TrustRung::Draft | TrustRung::UnreviewedPromotion | TrustRung::Flagged => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TrustRung::Draft => "draft",
            TrustRung::UnreviewedPromotion => "unreviewed-promotion",
            TrustRung::Flagged => "flagged",
            TrustRung::Agent => "agent",
            TrustRung::Human => "human",
        }
    }
}

/// Trust-ladder metadata for one artifact, paired by whichever
/// join-spine key (`task_id`/`scenario_id`) the artifact already
/// carries — canon-gate's own companion record since
/// `canon_model::EvidenceRecord`/`Task` carry no `lifecycle`/`flagged`
/// field yet (module doc, INTERFACE REQUEST to S1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TrustLadderState {
    pub lifecycle: TrustLifecycle,
    pub flagged: FlaggedOverlay,
}

impl TrustLadderState {
    pub fn new(lifecycle: TrustLifecycle) -> Self {
        Self { lifecycle, flagged: FlaggedOverlay::clear() }
    }

    /// [`TrustRung::classify`] applied to this state — `has_review_record`
    /// is still caller-supplied (see that function's doc).
    pub fn rung(&self, has_review_record: bool) -> TrustRung {
        TrustRung::classify(self.lifecycle, has_review_record, self.flagged.flagged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_is_never_green() {
        assert_eq!(TrustRung::classify(TrustLifecycle::Draft, true, false).green(), None);
        assert_eq!(TrustRung::classify(TrustLifecycle::Draft, false, false).green(), None);
    }

    #[test]
    fn reviewed_without_review_record_is_unreviewed_promotion() {
        let rung = TrustRung::classify(TrustLifecycle::Reviewed, false, false);
        assert_eq!(rung, TrustRung::UnreviewedPromotion);
        assert_eq!(rung.green(), None);
    }

    #[test]
    fn reviewed_with_review_record_is_green_at_agent() {
        let rung = TrustRung::classify(TrustLifecycle::Reviewed, true, false);
        assert_eq!(rung, TrustRung::Agent);
        assert_eq!(rung.green(), Some(TrustLevel::Agent));
    }

    #[test]
    fn ratified_is_green_at_human() {
        let rung = TrustRung::classify(TrustLifecycle::Ratified, true, false);
        assert_eq!(rung, TrustRung::Human);
        assert_eq!(rung.green(), Some(TrustLevel::Human));
    }

    #[test]
    fn flagged_overrides_ratified_and_is_never_green() {
        let rung = TrustRung::classify(TrustLifecycle::Ratified, true, true);
        assert_eq!(rung, TrustRung::Flagged);
        assert_eq!(rung.green(), None);
    }

    #[test]
    fn flagged_overrides_agent_too() {
        let rung = TrustRung::classify(TrustLifecycle::Reviewed, true, true);
        assert_eq!(rung, TrustRung::Flagged);
        assert_eq!(rung.green(), None);
    }

    #[test]
    fn trust_level_ranks_agent_below_human() {
        assert!(TrustLevel::Agent < TrustLevel::Human);
    }

    #[test]
    fn trust_ladder_state_rung_matches_classify() {
        let state = TrustLadderState::new(TrustLifecycle::Reviewed);
        assert_eq!(state.rung(true), TrustRung::Agent);
        assert_eq!(state.rung(false), TrustRung::UnreviewedPromotion);
    }

    #[test]
    fn lifecycle_and_level_round_trip_through_as_str() {
        for l in TrustLifecycle::ALL {
            assert_eq!(TrustLifecycle::from_str_exact(l.as_str()), Some(l));
        }
        for t in TrustLevel::ALL {
            assert_eq!(TrustLevel::from_str_exact(t.as_str()), Some(t));
        }
    }
}
