//! Divergence events (S11 task 1.1): `spec/divergences/lane=<lane>/
//! area=<area>/surface=<surface>/<round>-<round>-<sha>-<rand>.jsonl`,
//! one JSON object per line, `type`-tagged `manifest`/`review`/
//! `remediation` (grounded directly in the real corpus — field unions
//! read from the donor's real divergence corpus, S11
//! design §Context). Layout is UNCHANGED by S11 (design Non-Goal: "both
//! already Hive and marked ✓"); this module upgrades FIELDS only:
//! optional `actor` (cross-family gap: "no session/actor identity
//! anywhere"), structured `refs` alongside the raw `port_ref` (design
//! D4), and `divergence_refs`' reciprocal on the ledger side (design D6,
//! implemented in [`crate::family::ledger::LedgerReviewRecord`]).
//!
//! Every variant keeps an `extra` flatten-catchall for the donor's
//! richer, review/remediation-specific fields observed in the real
//! corpus but not central to S11's audited gaps (`aspects`,
//! `architecture_equivalence`, `defer_*`, `note`, `prior_divergence`,
//! `disposition`) — round-tripped losslessly, not reinterpreted; a
//! later change can promote any of these to a first-class field without
//! this one needing to guess their full semantics today.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::envelope::Actor;
use crate::family::refs::Ref;
use crate::ids::ScenarioId;

/// `type: "manifest"` — one review round's scope: which scenarios
/// (`reviewed_ids`) a reviewer took up, at which pin/app_sha.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DivergenceManifest {
    pub schema: u32,
    pub lane: String,
    pub surface: String,
    pub round: u32,
    pub reviewer: String,
    pub reviewed_ids: Vec<ScenarioId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    pub app_sha: String,
    pub at: DateTime<Utc>,
    pub run_seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Actor>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// `type: "review"` — one scenario's outcome within a manifest's round:
/// `status` ∈ `open | resolved | still-divergent | deferred` (real
/// corpus distribution). `ledger_ref` is the donor's EXISTING one-way
/// pointer at the corresponding ledger record's corpus-relative path —
/// design D6's reciprocal (`divergence_refs`) is added on that ledger
/// record by `canon-fmt`'s cross-indexing pass, not here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DivergenceReview {
    pub schema: u32,
    pub lane: String,
    pub scenario_id: ScenarioId,
    pub round: u32,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    pub app_sha: String,
    pub reviewer: String,
    pub at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledger_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledger_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledger_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledger_app_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledger_reviewer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_ref: Option<String>,
    /// Structured replacement for `port_ref` (design D4) — see
    /// [`crate::family::ledger::LedgerReviewRecord::refs`]'s doc for the
    /// same parse-or-report discipline.
    #[serde(default)]
    pub refs: Vec<Ref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Actor>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// `type: "remediation"` — a fix applied in response to a divergence
/// (`disposition`, `files` changed, `status: "remediated"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DivergenceRemediation {
    pub schema: u32,
    pub lane: String,
    pub scenario_id: ScenarioId,
    pub round: u32,
    pub app_sha: String,
    pub at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Actor>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// One JSONL line — `type`-tagged, internally-tagged serde enum so the
/// wire form stays `{"type": "manifest", ...}` with no extra nesting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum DivergenceEvent {
    Manifest(DivergenceManifest),
    Review(DivergenceReview),
    Remediation(DivergenceRemediation),
}

impl DivergenceEvent {
    pub fn lane(&self) -> &str {
        match self {
            DivergenceEvent::Manifest(m) => &m.lane,
            DivergenceEvent::Review(r) => &r.lane,
            DivergenceEvent::Remediation(r) => &r.lane,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 8, 16, 9, 34).unwrap()
    }

    #[test]
    fn manifest_round_trips_and_tags_type() {
        let event = DivergenceEvent::Manifest(DivergenceManifest {
            schema: 1,
            lane: "design".to_string(),
            surface: "idolive-photocard-viewer".to_string(),
            round: 2,
            reviewer: "design-reviewer-idolive-photocard-viewer".to_string(),
            reviewed_ids: vec![ScenarioId::parse("idolive.photocard-viewer.09").unwrap()],
            pin: Some("9c93d024b".to_string()),
            app_sha: "eb54b66c0bf2b835d85176560ad2b2087cb6717a".to_string(),
            at: at(),
            run_seq: 2,
            actor: None,
            extra: BTreeMap::new(),
        });
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json.get("type").and_then(|v| v.as_str()), Some("manifest"));
        let back: DivergenceEvent = serde_json::from_value(json).unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn review_extra_fields_round_trip_losslessly() {
        let raw = serde_json::json!({
            "schema": 1, "type": "review", "lane": "design",
            "scenario_id": "idolive.photocard-viewer.14", "round": 2,
            "status": "still-divergent", "pin": "9c93d024b",
            "app_sha": "eb54b66c0bf2b835d85176560ad2b2087cb6717a",
            "reviewer": "design-reviewer-idolive-photocard-viewer",
            "at": at().to_rfc3339(),
            "port_ref": "lib/a.dart#Foo",
            "aspects": ["layout", "copy"],
            "note": "matches upstream"
        });
        let event: DivergenceEvent = serde_json::from_value(raw.clone()).unwrap();
        let back = serde_json::to_value(&event).unwrap();
        assert_eq!(back.get("aspects"), raw.get("aspects"));
        assert_eq!(back.get("note"), raw.get("note"));
    }
}
