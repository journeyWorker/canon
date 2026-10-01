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

use crate::ids::{StrategyId, StrategyIdentity, TrajectoryId};

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
    pub contradicting_trajectory_id: TrajectoryId,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome_provenance: Option<String>,
}

impl DemotionEvidence {
    pub fn new(contradicting_trajectory_id: TrajectoryId, reason: impl Into<String>, at: DateTime<Utc>) -> Self {
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, at, Actor::new_unattributed("canon-learn::demote_strategy"));
        Self { envelope, contradicting_trajectory_id, reason: reason.into(), evaluation_digest: None, approval_digest: None, outcome_provenance: None }
    }

    pub fn demoted_at(&self) -> DateTime<Utc> {
        self.envelope.at
    }
}

/// Lifecycle of a distilled strategy candidate. The field on
/// [`StrategyItem`] is optional for wire compatibility: `None` means the
/// pre-quarantine rows are legacy active rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyLifecycle {
    Quarantined,
    Active,
    Rejected,
    RolledBack,
}

impl StrategyLifecycle {
    pub fn retrievable(self) -> bool {
        matches!(self, Self::Active)
    }
}

/// A distilled, non-destructively-derived strategy insight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyItem {
    pub id: StrategyId,
    pub regime_key: RegimeKey,
    pub role: RoleId,
    pub title: String,
    pub description: String,
    pub content: String,
    pub source_trajectory_ids: Vec<TrajectoryId>,
    pub recorded_at: DateTime<Utc>,
    /// `None` is a legacy active row. New distillation starts in
    /// `quarantined` and only the approved writer can set `active`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<StrategyLifecycle>,
    /// Durable contradiction evidence. Presence excludes retrieval.
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
            lifecycle: None,
            demotion: None,
        }
    }

    pub fn with_lifecycle(mut self, lifecycle: StrategyLifecycle) -> Self {
        self.lifecycle = Some(lifecycle);
        self
    }

    pub fn is_retrievable(&self) -> bool {
        self.demotion.is_none() && self.lifecycle.map_or(true, StrategyLifecycle::retrievable)
    }

    /// Constructs a distilled item whose `id` is DERIVED from the rest
    /// of its own content ([`StrategyId::derive`]) rather than minted.
    /// The constructor [`crate::distill::distill_trajectory`] uses, and
    /// the reason a [`crate::rebuild::rebuild_namespace`] that
    /// re-derives the same strategy reproduces the same id instead of
    /// orphaning every `Run.injected_guidance` snapshot and promoted
    /// `.canon/strategies/<role>/<id>.md` file that cited the old one
    /// (`crate::ids` module doc).
    ///
    /// [`StrategyItem::new`] is retained for callers that already HOLD
    /// an id — a fixture, or a decode path reconstructing a stored row
    /// — and is not a second identity scheme: an item built here
    /// satisfies `item.id == StrategyId::derive(&item.identity())`, an
    /// equation any holder of the row can check.
    pub fn from_identity(identity: StrategyIdentity<'_>) -> Self {
        Self {
            id: StrategyId::derive(&identity),
            regime_key: identity.regime_key.clone(),
            role: identity.role.clone(),
            title: identity.title.to_string(),
            description: identity.description.to_string(),
            content: identity.content.to_string(),
            source_trajectory_ids: identity.source_trajectory_ids.to_vec(),
            recorded_at: identity.recorded_at,
            lifecycle: None,
            demotion: None,
        }
    }

    /// This row's own content-identity — the exact preimage
    /// [`StrategyId::derive`] consumed. Makes a derived [`StrategyId`]
    /// AUDITABLE: every field here is carried by both the parquet row's
    /// JSON body and a promoted markdown file's front matter + body, so
    /// a reader can recompute the id and confirm the row was not
    /// hand-edited away from its own name.
    pub fn identity(&self) -> StrategyIdentity<'_> {
        StrategyIdentity {
            regime_key: &self.regime_key,
            role: &self.role,
            title: &self.title,
            description: &self.description,
            content: &self.content,
            source_trajectory_ids: &self.source_trajectory_ids,
            recorded_at: self.recorded_at,
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
    fn lifecycle_round_trips_and_legacy_rows_are_active() {
        let item = StrategyItem::new(
            StrategyId::new(),
            regime(),
            RoleId::parse("dev").unwrap(),
            "title",
            "description",
            "content",
            vec![TrajectoryId::new()],
            Utc::now(),
        )
        .with_lifecycle(StrategyLifecycle::Quarantined);
        let back: StrategyItem = serde_json::from_str(&serde_json::to_string(&item).unwrap()).unwrap();
        assert_eq!(back.lifecycle, Some(StrategyLifecycle::Quarantined));
        assert!(!back.is_retrievable());
        let legacy = StrategyItem::new(
            StrategyId::new(),
            regime(),
            RoleId::parse("dev").unwrap(),
            "title",
            "description",
            "content",
            vec![TrajectoryId::new()],
            Utc::now(),
        );
        assert!(legacy.is_retrievable());
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

    /// The equation `from_identity`'s doc promises: an item names
    /// itself. A reader holding only the persisted row (or a promoted
    /// markdown file's front matter + body) can recompute the id.
    #[test]
    fn a_derived_item_names_itself() {
        let ids = vec![TrajectoryId::new()];
        let role = RoleId::parse("dev").unwrap();
        let rk = regime();
        let item = StrategyItem::from_identity(StrategyIdentity {
            regime_key: &rk,
            role: &role,
            title: "batch the writes",
            description: "Validated strategy.",
            content: "ctx text",
            source_trajectory_ids: &ids,
            recorded_at: Utc::now(),
        });
        assert_eq!(item.id, StrategyId::derive(&item.identity()));
    }

    /// Demotion is a soft-flag written IN PLACE by
    /// `StrategyStore::mark_demoted`, so it must not sit in the
    /// identity — a demoted row that re-keyed itself would dangle every
    /// reference to it, the failure `crate::ids`' module doc exists to
    /// close.
    #[test]
    fn demoting_an_item_does_not_move_its_derived_id() {
        let ids = vec![TrajectoryId::new()];
        let role = RoleId::parse("dev").unwrap();
        let rk = regime();
        let item = StrategyItem::from_identity(StrategyIdentity {
            regime_key: &rk,
            role: &role,
            title: "t",
            description: "d",
            content: "c",
            source_trajectory_ids: &ids,
            recorded_at: Utc::now(),
        });
        let before = item.id;
        let demoted = item.with_demotion(DemotionEvidence::new(TrajectoryId::new(), "contradicting failure", Utc::now()));
        assert_eq!(demoted.id, before);
        assert_eq!(StrategyId::derive(&demoted.identity()), before);
    }
}
