//! Native trust-ladder types (S1 design D9/task 1.5, s15
//! `gate-native-record-fields` spec): [`TrustLifecycle`] and
//! [`FlaggedOverlay`] — the two DATA types [`crate::records::EvidenceRecord`]
//! carries natively as `lifecycle: Option<TrustLifecycle>` and
//! `flagged: Option<FlaggedOverlay>`.
//!
//! These types originated as `canon-gate`-owned companions
//! (`canon-gate/src/trust_ladder.rs`, S5) because canon-gate's territory
//! excluded editing `canon-model` directly at the time — that module's
//! own doc comment named the exact migration this is: "a mechanical
//! move (this type's fields become `EvidenceRecord`'s own), not a
//! redesign." `canon-gate` re-exports both from here so its own
//! `TrustLevel`/`TrustRung`/`TrustLadderState`/classification logic
//! (`canon-gate::trust_ladder`) keeps compiling unchanged; the READ-side
//! migration (canon-gate reading these fields off `ctx.evidence` instead
//! of a raw-JSON re-scan, and deleting its now-redundant companion
//! types) is a LATER wave (design sequencing P3b), not this one.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::envelope::Actor;

/// The three-rung evidence lifecycle — exactly one per artifact, never
/// a fourth "flagged" rung ([`FlaggedOverlay`] is an orthogonal
/// overlay, not a lifecycle state). Unlike the donor's freeform
/// `@draft`/`@reviewed`/`@ratified` tags (which can co-occur), a typed
/// Rust enum makes "two lifecycle tags at once" structurally
/// impossible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TrustLifecycle {
    Draft,
    Reviewed,
    Ratified,
}

impl TrustLifecycle {
    pub const ALL: [TrustLifecycle; 3] = [TrustLifecycle::Draft, TrustLifecycle::Reviewed, TrustLifecycle::Ratified];

    pub fn as_str(self) -> &'static str {
        match self {
            TrustLifecycle::Draft => "draft",
            TrustLifecycle::Reviewed => "reviewed",
            TrustLifecycle::Ratified => "ratified",
        }
    }

    pub fn from_str_exact(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }
}

/// The orthogonal `flagged` overlay, paired with whatever
/// `EvidenceRecord` it marks. `flagged_by`/`flagged_at` carry who/when;
/// the flag-CLEAR ratchet's enforcement (only a human-attributed actor
/// may set or clear this, one-way) lives in `canon-gate::trust`
/// (`attempt_clear`), not here — this type carries the shape only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FlaggedOverlay {
    pub flagged: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flagged_by: Option<Actor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flagged_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl FlaggedOverlay {
    pub fn clear() -> Self {
        Self { flagged: false, flagged_by: None, flagged_at: None }
    }

    pub fn set(by: Actor, at: chrono::DateTime<chrono::Utc>) -> Self {
        Self { flagged: true, flagged_by: Some(by), flagged_at: Some(at) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::RoleId;

    #[test]
    fn trust_lifecycle_round_trips_kebab_case() {
        for lifecycle in TrustLifecycle::ALL {
            let json = serde_json::to_value(lifecycle).unwrap();
            assert_eq!(json, serde_json::json!(lifecycle.as_str()));
            assert_eq!(TrustLifecycle::from_str_exact(lifecycle.as_str()), Some(lifecycle));
        }
    }

    #[test]
    fn flagged_overlay_set_then_clear() {
        let actor = Actor::new("reviewer-1", RoleId::parse("reviewer").unwrap());
        let set = FlaggedOverlay::set(actor.clone(), chrono::Utc::now());
        assert!(set.flagged);
        assert_eq!(set.flagged_by, Some(actor));

        let cleared = FlaggedOverlay::clear();
        assert!(!cleared.flagged);
        assert!(cleared.flagged_by.is_none());
    }
}
