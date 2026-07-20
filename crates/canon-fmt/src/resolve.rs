//! Content-trusted partition-value resolution (S11 design D1/`_area_of`
//! generalized): every function here derives a [`ResolvedPartition`]
//! from a record's OWN content, never from the directory it was found
//! in — used by `check.rs` (`canon fmt --check`).

use canon_model::family::refs::Ref;
use canon_model::family::{FamilyKind, LedgerKind, ResolvedPartition};
use canon_model::ids::ScenarioId;

/// Why a record's partition values couldn't be resolved from its own
/// content — always a `canon fmt --check` diagnostic, never a panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveError(pub String);

pub fn resolve_ledger(kind: LedgerKind, json: &serde_json::Value) -> Result<ResolvedPartition, ResolveError> {
    if kind.is_run_shaped() {
        return Ok(ResolvedPartition::default());
    }
    let scenario_id = json
        .get("scenario_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ResolveError("missing `scenario_id` field".to_string()))?;
    let parsed = ScenarioId::parse(scenario_id).map_err(|e| ResolveError(format!("malformed scenario_id: {e}")))?;
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
pub fn resolve_divergence(json: &serde_json::Value) -> Result<ResolvedPartition, ResolveError> {
    let lane = json.get("lane").and_then(|v| v.as_str()).ok_or_else(|| ResolveError("missing `lane` field".to_string()))?;
    let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match event_type {
        "manifest" => {
            let surface = json.get("surface").and_then(|v| v.as_str()).ok_or_else(|| ResolveError("missing `surface` field".to_string()))?;
            let reviewed_ids: Vec<&str> = json.get("reviewed_ids").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
            let Some(first) = reviewed_ids.first() else {
                return Err(ResolveError("manifest event has no `reviewed_ids` to derive `area` from".to_string()));
            };
            let first_area = ScenarioId::parse(*first).map_err(|e| ResolveError(format!("malformed reviewed_ids entry: {e}")))?.area().to_string();
            for id in &reviewed_ids[1..] {
                let area = ScenarioId::parse(*id).map_err(|e| ResolveError(format!("malformed reviewed_ids entry: {e}")))?.area().to_string();
                if area != first_area {
                    return Err(ResolveError(format!(
                        "reviewed_ids disagree on area (`{first_area}` vs `{area}`) — ambiguous-partition"
                    )));
                }
            }
            Ok(ResolvedPartition {
                values: vec![("lane", lane.to_string()), ("area", first_area), ("surface", surface.to_string())],
                leaf_name: None,
                optional_segment: None,
            })
        }
        "review" | "remediation" => {
            let scenario_id = json.get("scenario_id").and_then(|v| v.as_str()).ok_or_else(|| ResolveError("missing `scenario_id` field".to_string()))?;
            let parsed = ScenarioId::parse(scenario_id).map_err(|e| ResolveError(format!("malformed scenario_id: {e}")))?;
            Ok(ResolvedPartition {
                values: vec![("lane", lane.to_string()), ("area", parsed.area().to_string()), ("surface", parsed.surface_key())],
                leaf_name: None,
                optional_segment: None,
            })
        }
        other => Err(ResolveError(format!("unrecognized divergence event `type`: `{other}`"))),
    }
}

/// A feature file's `area` — derived from the FIRST `@<area>.<surface>.<nn>`-shaped
/// scenario tag found in the file (module `gherkin`'s scan), never from
/// the pre-migration directory it sits in.
pub fn resolve_feature(scenario_ids: &[String]) -> Result<ResolvedPartition, ResolveError> {
    let Some(first) = scenario_ids.first() else {
        return Err(ResolveError("no `@<area>.<surface>.<nn>`-shaped scenario tag found to derive `area` from".to_string()));
    };
    let parsed = ScenarioId::parse(first).map_err(|e| ResolveError(format!("malformed scenario tag: {e}")))?;
    for id in &scenario_ids[1..] {
        let area = ScenarioId::parse(id).map_err(|e| ResolveError(format!("malformed scenario tag: {e}")))?.area().to_string();
        if area != parsed.area() {
            return Err(ResolveError(format!("scenarios disagree on area (`{}` vs `{area}`) — ambiguous-partition", parsed.area())));
        }
    }
    Ok(ResolvedPartition { values: vec![("area", parsed.area().to_string())], leaf_name: None, optional_segment: None })
}

/// An inventory file's `area`/`surface` — derived from the dot-prefix
/// shared by EVERY entry key (`idolive.hub.hub-header` → area=`idolive`,
/// surface=`hub`); `Err` when entries disagree (D8 `ambiguous-partition`)
/// or the file has no entries at all to derive from.
pub fn resolve_inventory(entry_keys: &[String]) -> Result<ResolvedPartition, ResolveError> {
    let mut prefixes: Vec<(String, String)> = Vec::new();
    for key in entry_keys {
        let parts: Vec<&str> = key.splitn(3, '.').collect();
        let [area, surface, ..] = parts.as_slice() else {
            return Err(ResolveError(format!("entry key `{key}` does not have an `<area>.<surface>.<leaf>` shape")));
        };
        prefixes.push((area.to_string(), surface.to_string()));
    }
    let Some((area, surface)) = prefixes.first().cloned() else {
        return Err(ResolveError("file has no entries to derive area/surface from".to_string()));
    };
    if prefixes.iter().any(|(a, s)| *a != area || *s != surface) {
        return Err(ResolveError(format!(
            "entries disagree on area/surface prefix (not all `{area}.{surface}.*`) — ambiguous-partition"
        )));
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
