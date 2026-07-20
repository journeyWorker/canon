//! The distilled tier: [`StrategyItem`] — title/description/content,
//! generalizing the donor harness's `StrategyMemoryItem`
//! (id/namespace/sourceTrajectoryIds/
//! title/description/content/recordedAt/tags). `sourceEngineHash`/
//! `sourceCatalogHash` (donor-tuning-specific staleness filters) are
//! NOT carried here — `regime_key`'s own `hash` segment already IS
//! that staleness filter, generalized past `sim` to every role
//! (design decision 2).

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{RegimeKey, RoleId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{StrategyId, TrajectoryId};

/// Durable evidence a [`StrategyItem`] was demoted (S7 design D4, task
/// group 4) — S1-envelope-shaped: composes
/// [`canon_model::envelope::Envelope`] via `#[serde(flatten)]`, the
/// SAME `{schema, kind, at, actor}` wrapper every `canon-model` record
/// kind carries (`s1-state-model-join-spine` design D2), even though
/// `canon-learn` does not implement `canon_model::envelope::
/// CanonRecord` for it (that trait also requires `JsonSchema`, which
/// this crate does not otherwise depend on — `canon-learn` reuses only
/// canon-model's join-key/envelope SHAPES, the same precedent
/// `Trajectory`/`StrategyItem` themselves already set, per `lib.rs`'s
/// "Deviates from the literal plan" note on S6 task 1.1). `kind` is
/// [`RecordKind::EvidenceRecord`] — canon-model's twelve closed record
/// kinds have no dedicated `Demotion` variant, and adding one is
/// canon-model's own call, out of this crate's insulated surface; the
/// closest existing kind is reused rather than smuggling an untyped
/// `kind: String`. `strategy_id`/`regime_key` are NOT duplicated here —
/// they are the enclosing [`StrategyItem`]'s own fields, and this
/// value only ever lives nested inside [`StrategyItem::demotion`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DemotionEvidence {
    #[serde(flatten)]
    pub envelope: Envelope,
    /// The `Failure`/`RolledBack`-verdict [`crate::ids::TrajectoryId`]
    /// that contradicted this (previously-eligible-for-promotion)
    /// strategy's regime.
    pub contradicting_trajectory_id: TrajectoryId,
    /// Human-readable reason — mirrors the git-tier file's own
    /// `status: demoted` front-matter `reason` field (S7 design D4);
    /// the SAME text lands in both places.
    pub reason: String,
}

impl DemotionEvidence {
    pub fn new(contradicting_trajectory_id: TrajectoryId, reason: impl Into<String>, at: DateTime<Utc>) -> Self {
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, at, Actor::new_unattributed("canon-learn::demote_strategy"));
        Self { envelope, contradicting_trajectory_id, reason: reason.into() }
    }

    pub fn demoted_at(&self) -> DateTime<Utc> {
        self.envelope.at
    }
}

/// A distilled, non-destructively-derived strategy insight (design
/// decision 3). Every field here is plain-`Serialize`/`Deserialize`
/// (unlike [`crate::trajectory::Trajectory`], which carries the
/// non-serde [`canon_ingest::verdict::VerdictRow`]) — the parquet
/// store encodes this type's JSON form directly, no wire mirror
/// needed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyItem {
    pub id: StrategyId,
    pub regime_key: RegimeKey,
    pub role: RoleId,
    /// Concise strategy identifier (`StrategyMemoryItem.title`'s
    /// analog; paper §3.2 `title`, cited by the reasoning-bank-
    /// substrate audit).
    pub title: String,
    /// One-sentence summary (`StrategyMemoryItem.description`'s
    /// analog).
    pub description: String,
    /// Distilled reasoning / rationale / operational insight — the
    /// low-level execution detail abstracted away
    /// (`StrategyMemoryItem.content`'s analog).
    pub content: String,
    /// Provenance: the trajectory id(s) this item was distilled from
    /// (`StrategyMemoryItem.sourceTrajectoryIds`'s analog) — never
    /// empty; a strategy item that cites no source trajectory has no
    /// audit trail (design decision 3's "audit trail of what the
    /// distiller believed at time T").
    pub source_trajectory_ids: Vec<TrajectoryId>,
    pub recorded_at: DateTime<Utc>,
    /// `None` while active; `Some(_)` once [`crate::promotion::
    /// demote_strategy`] soft-flags this item after a contradicting
    /// trajectory arrives (S7 design D4) — presence alone IS "demoted",
    /// no separate status enum duplicating the same state.
    /// `#[serde(default)]` is load-bearing: a pre-S7 row with no
    /// `demotion` key at all deserializes as `None`, the same
    /// backward-compat contract [`crate::trajectory::Trajectory::
    /// verdict_record`] uses for pre-S7 rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub demotion: Option<DemotionEvidence>,
}

impl StrategyItem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: StrategyId,
        regime_key: RegimeKey,
        role: RoleId,
        title: impl Into<String>,
        description: impl Into<String>,
        content: impl Into<String>,
        source_trajectory_ids: Vec<TrajectoryId>,
        recorded_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            regime_key,
            role,
            title: title.into(),
            description: description.into(),
            content: content.into(),
            source_trajectory_ids,
            recorded_at,
            demotion: None,
        }
    }

    /// Builder-style override for [`StrategyItem::demotion`] — the
    /// constructor always seeds `None`; this is the escape hatch a
    /// test fixture (or [`crate::store::StrategyStore::mark_demoted`]'s
    /// own impl) uses to set a resolved value directly, mirroring
    /// [`crate::trajectory::Trajectory::with_verdict_record`]'s exact
    /// convention.
    pub fn with_demotion(mut self, demotion: DemotionEvidence) -> Self {
        self.demotion = Some(demotion);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regime() -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key("dev", "repo", "auth", "abc123")).unwrap()
    }

    #[test]
    fn serde_round_trips() {
        let item = StrategyItem::new(
            StrategyId::new(),
            regime(),
            RoleId::parse("dev").unwrap(),
            "title",
            "description",
            "content",
            vec![TrajectoryId::new()],
            Utc::now(),
        );
        let json = serde_json::to_string(&item).unwrap();
        let back: StrategyItem = serde_json::from_str(&json).unwrap();
        assert_eq!(back, item);
    }

    #[test]
    fn demotion_evidence_round_trips_and_defaults_to_none() {
        let mut item = StrategyItem::new(
            StrategyId::new(),
            regime(),
            RoleId::parse("dev").unwrap(),
            "title",
            "description",
            "content",
            vec![TrajectoryId::new()],
            Utc::now(),
        );
        assert!(item.demotion.is_none());

        item = item.with_demotion(DemotionEvidence::new(TrajectoryId::new(), "contradicting failure", Utc::now()));
        let json = serde_json::to_string(&item).unwrap();
        let back: StrategyItem = serde_json::from_str(&json).unwrap();
        assert_eq!(back, item);
        assert_eq!(back.demotion.unwrap().reason, "contradicting failure");
    }

    #[test]
    fn a_pre_s7_row_with_no_demotion_key_deserializes_as_none() {
        // Simulates a strategy row written before this field existed —
        // the exact JSON shape `StrategyItem::new` produced pre-S7.
        let json = format!(
            r#"{{"id":"{}","regime_key":"{}","role":"dev","title":"t","description":"d","content":"c","source_trajectory_ids":["{}"],"recorded_at":"{}"}}"#,
            StrategyId::new(),
            regime().as_str(),
            TrajectoryId::new(),
            Utc::now().to_rfc3339(),
        );
        let item: StrategyItem = serde_json::from_str(&json).unwrap();
        assert!(item.demotion.is_none());
    }
}
