//! The ledger's six kinds (S11 task 1.1): `run`/`drill` (flat, zero
//! partition keys) and `review`/`clear`/`code-review`/`design-review`
//! (area-scoped). Field shapes are grounded directly in the donor's
//! real ledger sample records (S11 design
//! §Context) — `app_sha`/`harness_sha`/`pin` stay plain `String` (never
//! [`crate::ids::Sha`]'s strict 40-hex newtype): a record with an
//! abbreviated sha must still deserialize and validate against this
//! schema's required-field set — `canon-fmt`'s OWN `abbreviated-sha`
//! check, not JSON-schema rejection, is what flags it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::family::FamilyEnvelope;
use crate::family::refs::Ref;
use crate::ids::{ChangeId, ScenarioId, TaskId};

/// The ledger's six on-disk `kind` values (S11 task 1.1). `run`/`drill`
/// share [`LedgerRunRecord`]'s shape; `review`/`clear`/`code-review`/
/// `design-review` share [`LedgerReviewRecord`]'s shape — `envelope.kind`
/// alone discriminates within each shared-shape pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LedgerKind {
    Run,
    Drill,
    Review,
    Clear,
    CodeReview,
    DesignReview,
}

impl LedgerKind {
    pub const ALL: [LedgerKind; 6] =
        [LedgerKind::Run, LedgerKind::Drill, LedgerKind::Review, LedgerKind::Clear, LedgerKind::CodeReview, LedgerKind::DesignReview];

    pub fn as_str(self) -> &'static str {
        match self {
            LedgerKind::Run => "run",
            LedgerKind::Drill => "drill",
            LedgerKind::Review => "review",
            LedgerKind::Clear => "clear",
            LedgerKind::CodeReview => "code-review",
            LedgerKind::DesignReview => "design-review",
        }
    }

    pub fn parse(s: &str) -> Option<LedgerKind> {
        LedgerKind::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// `run`/`drill` share [`LedgerRunRecord`]'s flat, zero-partition-key
    /// shape; the other four share [`LedgerReviewRecord`]'s area-scoped
    /// shape (S11 design D1).
    pub fn is_run_shaped(self) -> bool {
        matches!(self, LedgerKind::Run | LedgerKind::Drill)
    }
}

/// `kind=run/` and `kind=drill/` (S11 design D1: zero partition keys —
/// a run covering multiple scenarios cannot nest under one `area=`).
/// Audit gaps this closes: "no actor/session/cost/duration, `evidence:
/// []` unspecified, no change/task join" (design §5 S11 table).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LedgerRunRecord {
    #[serde(flatten)]
    pub envelope: FamilyEnvelope<LedgerKind>,
    pub scenario_ids: Vec<ScenarioId>,
    pub lane: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    pub app_sha: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness_sha: Option<String>,
    /// The bare `by: "flutter-test-machine"` string this schema
    /// version replaces with `envelope.actor` — kept here, optional,
    /// ONLY so an as-yet-unmigrated record still deserializes (design
    /// "additive-where-possible": a prior schema's required set stays
    /// satisfiable). Absent once a record carries a populated `actor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    pub result: String,
    #[serde(default)]
    pub evidence: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_id: Option<ChangeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
}

/// `kind=review/`, `kind=clear/`, `kind=code-review/`,
/// `kind=design-review/` (S11 design D1: area-scoped, one
/// `<scenario_id>.json` leaf). Audit gaps this closes: free-text
/// `upstream_ref`, `;`/`,`-joined `port_ref` strings (design §5 S11 table)
/// — both raw source strings are KEPT (never destroyed) alongside the
/// new structured `refs` array, since a free-text ref that fails to
/// parse must still be readable on the record `canon fmt --check` flags it on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LedgerReviewRecord {
    #[serde(flatten)]
    pub envelope: FamilyEnvelope<LedgerKind>,
    pub scenario_id: ScenarioId,
    pub reviewer: String,
    pub pin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_spec_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_ref: Option<String>,
    /// The structured replacement for `upstream_ref`/`port_ref` (design
    /// D4) — every successfully-parsed `<file>#<symbol>[:<a>-<b>]`
    /// segment, in source order. Empty when nothing parsed (fully
    /// free-text ref, flagged as `free-text-ref` by `canon fmt --check`)
    /// or when the record predates this migration.
    #[serde(default)]
    pub refs: Vec<Ref>,
    /// `code-review`/`design-review` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_sha: Option<String>,
    /// `code-review`/`design-review` only — `"faithful"` or `"n-a"`
    /// (parity.py's axis-2 rule: a divergence is represented by record
    /// ABSENCE, never a third verdict value here).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    /// D4.5: optional aspects-checked content on a passing review —
    /// left absent unless the source data actually names what was
    /// checked (never fabricated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_id: Option<ChangeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    /// D6's reciprocal half: one entry per divergence event (identified
    /// by its own corpus-relative file path) whose `ledger_ref` points
    /// at this record.
    #[serde(default)]
    pub divergence_refs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::Actor;
    use chrono::Utc;

    fn envelope(kind: LedgerKind) -> FamilyEnvelope<LedgerKind> {
        FamilyEnvelope::new(1, kind, Utc::now(), Actor::new_unattributed("flutter-test-machine"))
    }

    #[test]
    fn ledger_kind_as_str_matches_serde_kebab_case() {
        for kind in LedgerKind::ALL {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, format!("\"{}\"", kind.as_str()));
            assert_eq!(LedgerKind::parse(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn run_record_round_trips_with_bare_by_absent_when_actor_present() {
        let record = LedgerRunRecord {
            envelope: envelope(LedgerKind::Run),
            scenario_ids: vec![ScenarioId::parse("settings.index.03").unwrap()],
            lane: "unit".to_string(),
            platform: Some("headless".to_string()),
            app_sha: "2745ca4c889d49f11aa96c51b2f2cf01a4be0009".to_string(),
            harness_sha: Some("dfd2985fa7b3".to_string()),
            by: None,
            result: "pass".to_string(),
            evidence: vec![],
            cost_usd: None,
            duration_ms: None,
            change_id: None,
            task_id: None,
        };
        let json = serde_json::to_value(&record).unwrap();
        assert!(json.get("by").is_none());
        assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("run"));
        let back: LedgerRunRecord = serde_json::from_value(json).unwrap();
        assert_eq!(back, record);
    }

    #[test]
    fn review_record_keeps_raw_ref_alongside_structured_refs() {
        let record = LedgerReviewRecord {
            envelope: envelope(LedgerKind::Review),
            scenario_id: ScenarioId::parse("idolive.hub.25").unwrap(),
            reviewer: "draft-reconcile-idolive-hub".to_string(),
            pin: "9c93d024b".to_string(),
            upstream_ref: Some("reconciled vs upstream @9c93d024b (see spec/inventory/idolive-hub.yaml)".to_string()),
            original_spec_ref: None,
            port_ref: None,
            refs: vec![],
            app_sha: None,
            verdict: None,
            checked: None,
            change_id: None,
            task_id: None,
            divergence_refs: vec![],
        };
        let json = serde_json::to_value(&record).unwrap();
        assert!(json.get("upstream_ref").is_some(), "raw free-text ref must survive migration for provenance");
        assert_eq!(json.get("refs").and_then(|v| v.as_array()).map(Vec::len), Some(0));
    }
}
