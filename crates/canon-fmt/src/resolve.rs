//! Content-trusted partition-value resolution (S11 design D1/`_area_of`
//! generalized): every function here derives a [`ResolvedPartition`]
//! from a record's OWN content, never from the directory it was found
//! in — used by `check.rs` (`canon fmt --check`).

use canon_model::family::refs::Ref;
use canon_model::family::{FamilyKind, LedgerKind, ResolvedPartition};
use canon_model::ids::{JoinKeyError, ScenarioId};

/// Why a record's partition values couldn't be resolved from its own
/// content — always a `canon fmt --check` diagnostic, never a panic.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FmtResolveError {
    /// A required string field is absent (or not a string).
    #[error("missing `{field}` field")]
    MissingField { field: &'static str },
    #[error("malformed scenario_id: {0}")]
    MalformedScenarioId(#[source] JoinKeyError),
    #[error("manifest event has no `reviewed_ids` to derive `area` from")]
    NoReviewedIds,
    #[error("malformed reviewed_ids entry: {0}")]
    MalformedReviewedId(#[source] JoinKeyError),
    #[error("reviewed_ids disagree on area (`{first}` vs `{other}`) — ambiguous-partition")]
    ReviewedIdsDisagree { first: String, other: String },
    #[error("unrecognized divergence event `type`: `{event_type}`")]
    UnrecognizedEventType { event_type: String },
    #[error("no `@<area>.<surface>.<nn>`-shaped scenario tag found to derive `area` from")]
    NoScenarioTag,
    #[error("malformed scenario tag: {0}")]
    MalformedScenarioTag(#[source] JoinKeyError),
    #[error("scenarios disagree on area (`{first}` vs `{other}`) — ambiguous-partition")]
    ScenariosDisagree { first: String, other: String },
    #[error("entry key `{key}` does not have an `<area>.<surface>.<leaf>` shape")]
    MalformedEntryKey { key: String },
    #[error("file has no entries to derive area/surface from")]
    NoEntries,
    #[error("entries disagree on area/surface prefix (not all `{area}.{surface}.*`) — ambiguous-partition")]
    EntriesDisagree { area: String, surface: String },
}

pub fn resolve_ledger(kind: LedgerKind, json: &serde_json::Value) -> Result<ResolvedPartition, FmtResolveError> {
    if kind.is_run_shaped() {
        return Ok(ResolvedPartition::default());
    }
    let scenario_id = json
        .get("scenario_id")
        .and_then(|v| v.as_str())
        .ok_or(FmtResolveError::MissingField { field: "scenario_id" })?;
    let parsed = ScenarioId::parse(scenario_id).map_err(FmtResolveError::MalformedScenarioId)?;
    Ok(ResolvedPartition {
        values: vec![("area", parsed.area().to_string())],
        leaf_name: Some(format!("{}.json", parsed.as_str())),
        optional_segment: None,
    })
}

/// Divergence partition values, per JSONL-line `type` (module doc:
/// `manifest` carries `surface` literally but derives `area` from
/// `reviewed_ids`; `review`/`remediation` derive both from
/// `scenario_id` via [`ScenarioId::area`]/[`ScenarioId::surface_key`]).
pub fn resolve_divergence(json: &serde_json::Value) -> Result<ResolvedPartition, FmtResolveError> {
    let lane = json.get("lane").and_then(|v| v.as_str()).ok_or(FmtResolveError::MissingField { field: "lane" })?;
    let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match event_type {
        "manifest" => {
            let surface = json.get("surface").and_then(|v| v.as_str()).ok_or(FmtResolveError::MissingField { field: "surface" })?;
            let reviewed_ids: Vec<&str> = json.get("reviewed_ids").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
            let Some(first) = reviewed_ids.first() else {
                return Err(FmtResolveError::NoReviewedIds);
            };
            let first_area = ScenarioId::parse(*first).map_err(FmtResolveError::MalformedReviewedId)?.area().to_string();
            for id in &reviewed_ids[1..] {
                let area = ScenarioId::parse(*id).map_err(FmtResolveError::MalformedReviewedId)?.area().to_string();
                if area != first_area {
                    return Err(FmtResolveError::ReviewedIdsDisagree { first: first_area, other: area });
                }
            }
            Ok(ResolvedPartition {
                values: vec![("lane", lane.to_string()), ("area", first_area), ("surface", surface.to_string())],
                leaf_name: None,
                optional_segment: None,
            })
        }
        "review" | "remediation" => {
            let scenario_id = json.get("scenario_id").and_then(|v| v.as_str()).ok_or(FmtResolveError::MissingField { field: "scenario_id" })?;
            let parsed = ScenarioId::parse(scenario_id).map_err(FmtResolveError::MalformedScenarioId)?;
            Ok(ResolvedPartition {
                values: vec![("lane", lane.to_string()), ("area", parsed.area().to_string()), ("surface", parsed.surface_key())],
                leaf_name: None,
                optional_segment: None,
            })
        }
        other => Err(FmtResolveError::UnrecognizedEventType { event_type: other.to_string() }),
    }
}

/// A feature file's `area` — derived from the FIRST `@<area>.<surface>.<nn>`-shaped
/// scenario tag found in the file (module `gherkin`'s scan), never from
/// the pre-migration directory it sits in.
pub fn resolve_feature(scenario_ids: &[String]) -> Result<ResolvedPartition, FmtResolveError> {
    let Some(first) = scenario_ids.first() else {
        return Err(FmtResolveError::NoScenarioTag);
    };
    let parsed = ScenarioId::parse(first).map_err(FmtResolveError::MalformedScenarioTag)?;
    for id in &scenario_ids[1..] {
        let area = ScenarioId::parse(id).map_err(FmtResolveError::MalformedScenarioTag)?.area().to_string();
        if area != parsed.area() {
            return Err(FmtResolveError::ScenariosDisagree { first: parsed.area().to_string(), other: area });
        }
    }
    Ok(ResolvedPartition { values: vec![("area", parsed.area().to_string())], leaf_name: None, optional_segment: None })
}

/// An inventory file's `area`/`surface` — derived from the dot-prefix
/// shared by EVERY entry key (`idolive.hub.hub-header` → area=`idolive`,
/// surface=`hub`); `Err` when entries disagree (D8 `ambiguous-partition`)
/// or the file has no entries at all to derive from.
pub fn resolve_inventory(entry_keys: &[String]) -> Result<ResolvedPartition, FmtResolveError> {
    let mut prefixes: Vec<(String, String)> = Vec::new();
    for key in entry_keys {
        let parts: Vec<&str> = key.splitn(3, '.').collect();
        let [area, surface, ..] = parts.as_slice() else {
            return Err(FmtResolveError::MalformedEntryKey { key: key.clone() });
        };
        prefixes.push((area.to_string(), surface.to_string()));
    }
    let Some((area, surface)) = prefixes.first().cloned() else {
        return Err(FmtResolveError::NoEntries);
    };
    if prefixes.iter().any(|(a, s)| *a != area || *s != surface) {
        return Err(FmtResolveError::EntriesDisagree { area, surface });
    }
    Ok(ResolvedPartition { values: vec![("area", area)], leaf_name: None, optional_segment: Some(surface) })
}

pub fn family_kind_for_ledger(kind: LedgerKind) -> FamilyKind {
    FamilyKind::Ledger(kind)
}

/// A ref field's raw string, wherever this kind stores one — the one
/// place `check.rs` looks to decide which field is THE ref field for
/// a given kind.
pub fn ref_fields(kind: LedgerKind) -> &'static [&'static str] {
    match kind {
        LedgerKind::Review | LedgerKind::Clear => &["upstream_ref", "original_spec_ref"],
        LedgerKind::CodeReview | LedgerKind::DesignReview => &["port_ref"],
        LedgerKind::Run | LedgerKind::Drill => &[],
    }
}

/// Structured `refs` built from `raw` (module `refparse`), never
/// invented — re-exported here purely so `check.rs` has one import
/// path.
pub fn parse_ref_field(raw: &str) -> (Vec<Ref>, Vec<String>) {
    let outcome = crate::refparse::parse_refs(raw);
    (outcome.refs, outcome.unparsed)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// The string the old `ResolveError(String)` carried for a malformed id.
    fn bad_id_detail(prefix: &str) -> String {
        format!("{prefix}: {}", ScenarioId::parse("bad").unwrap_err())
    }

    #[test]
    fn resolve_ledger_error_strings_match_the_old_literals() {
        let missing = resolve_ledger(LedgerKind::Review, &json!({})).unwrap_err();
        assert_eq!(missing.to_string(), "missing `scenario_id` field");
        let malformed = resolve_ledger(LedgerKind::Review, &json!({"scenario_id": "bad"})).unwrap_err();
        assert_eq!(malformed.to_string(), bad_id_detail("malformed scenario_id"));
        assert!(std::error::Error::source(&malformed).is_some());
    }

    #[test]
    fn resolve_divergence_error_strings_match_the_old_literals() {
        let no_lane = resolve_divergence(&json!({})).unwrap_err();
        assert_eq!(no_lane.to_string(), "missing `lane` field");
        let no_surface = resolve_divergence(&json!({"lane": "l", "type": "manifest"})).unwrap_err();
        assert_eq!(no_surface.to_string(), "missing `surface` field");
        let no_ids = resolve_divergence(&json!({"lane": "l", "type": "manifest", "surface": "s"})).unwrap_err();
        assert_eq!(no_ids.to_string(), "manifest event has no `reviewed_ids` to derive `area` from");
        let bad_entry = resolve_divergence(&json!({"lane": "l", "type": "manifest", "surface": "s", "reviewed_ids": ["bad"]})).unwrap_err();
        assert_eq!(bad_entry.to_string(), bad_id_detail("malformed reviewed_ids entry"));
        let disagree = resolve_divergence(&json!({"lane": "l", "type": "manifest", "surface": "s", "reviewed_ids": ["a.b.01", "c.d.02"]})).unwrap_err();
        assert_eq!(disagree.to_string(), "reviewed_ids disagree on area (`a` vs `c`) — ambiguous-partition");
        let no_sid = resolve_divergence(&json!({"lane": "l", "type": "review"})).unwrap_err();
        assert_eq!(no_sid.to_string(), "missing `scenario_id` field");
        let bad_sid = resolve_divergence(&json!({"lane": "l", "type": "review", "scenario_id": "bad"})).unwrap_err();
        assert_eq!(bad_sid.to_string(), bad_id_detail("malformed scenario_id"));
        let other = resolve_divergence(&json!({"lane": "l", "type": "weird"})).unwrap_err();
        assert_eq!(other.to_string(), "unrecognized divergence event `type`: `weird`");
    }

    #[test]
    fn resolve_feature_error_strings_match_the_old_literals() {
        let none = resolve_feature(&[]).unwrap_err();
        assert_eq!(none.to_string(), "no `@<area>.<surface>.<nn>`-shaped scenario tag found to derive `area` from");
        let bad = resolve_feature(&["bad".to_string()]).unwrap_err();
        assert_eq!(bad.to_string(), bad_id_detail("malformed scenario tag"));
        let disagree = resolve_feature(&["a.b.01".to_string(), "c.d.02".to_string()]).unwrap_err();
        assert_eq!(disagree.to_string(), "scenarios disagree on area (`a` vs `c`) — ambiguous-partition");
    }

    #[test]
    fn resolve_inventory_error_strings_match_the_old_literals() {
        let shape = resolve_inventory(&["nodots".to_string()]).unwrap_err();
        assert_eq!(shape.to_string(), "entry key `nodots` does not have an `<area>.<surface>.<leaf>` shape");
        let empty = resolve_inventory(&[]).unwrap_err();
        assert_eq!(empty.to_string(), "file has no entries to derive area/surface from");
        let disagree = resolve_inventory(&["a.b.x".to_string(), "a.c.y".to_string()]).unwrap_err();
        assert_eq!(disagree.to_string(), "entries disagree on area/surface prefix (not all `a.b.*`) — ambiguous-partition");
    }
}
