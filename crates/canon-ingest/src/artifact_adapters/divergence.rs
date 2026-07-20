//! The divergence adapter (S4 wave-2, design D2) — reads a
//! `canon.yaml`-configured, Hive-partitioned tree of divergence-log
//! `.jsonl` files (`lane=<lane>/area=<area>/surface=<surface>/*.jsonl`,
//! `ArtifactSourceConfig::divergences_root`) and normalizes each line
//! into an [`ArtifactEvent`] keyed by `scenario_id`.
//!
//! Grounded in `design.md` D2 + `specs/artifact-ingest-adapters/spec.md`
//! ("Divergence manifest and review lines are distinguished by type")
//! and the parity-harness donor audit. The donor consumer repo's
//! `spec/divergences/` tree is the reference donor SHAPE only — this
//! adapter never reads it; its
//! FROZEN fixture corpus (`tests/fixtures/divergences/`) is a
//! hand-authored, checked-in sample in the identical Hive layout (S4
//! foundation rescope, design D6).
//!
//! Each `.jsonl` file mixes three line `"type"`s (design D2):
//! - `"manifest"` — round bookkeeping (`reviewed_ids`, `reviewer`,
//!   `round`, …) for every scenario the round touched. Normalizes to
//!   exactly ONE non-verdict [`ArtifactEventKind::NonVerdict`] `Event`
//!   per line (spec.md: "the manifest line normalizes to a
//!   round-bookkeeping Event", singular) — keyed by the FIRST
//!   parseable id in `reviewed_ids` (a representative anchor; the full
//!   list survives verbatim in `detail`). A manifest with no parseable
//!   `reviewed_ids` entry has no S1 join-spine identity to anchor on
//!   and is skipped (design §7 "malformed evidence is no evidence").
//! - `"review"` with `"status":"open"` — an open/still-divergent
//!   finding: the review-verdict-mapping table's `code-review finding`
//!   row when the file's `lane=` Hive PATH partition (never the
//!   line's own optional inline `"lane"` field, which may be absent
//!   or drift from the path — the path partition is this adapter's
//!   sole source of truth for classification) is `code`, or
//!   `design-review finding` when `design` (the table's row split
//!   mirrors the ledger adapter's `kind=code-review`/`kind=design-review`
//!   split). A `review` line at any other status (`still-divergent`,
//!   `deferred`, `resolved`, …) or an unrecognized path `lane=`
//!   partition still normalizes to an `Event` (join-spine identity
//!   preserved) but carries [`ArtifactEventKind::NonVerdict`] — D2
//!   names literally `"status":"open"` as the finding trigger, not
//!   the vendor's wider `_DIV_ACTIVE` status set (a broader mapping
//!   is a deliberate follow-up, not fabricated here).
//! - `"remediation"` with `"status":"resolved"` — the "remediation +
//!   later resolved" row ([`ArtifactEventKind::RemediationResolved`]).
//!   Any other remediation status (e.g. the vendor's provenance-only
//!   `"remediated"` pin) normalizes to a `NonVerdict` `Event` — a fix
//!   attempt alone never greens a cell.
//!
//! A line that fails to parse as JSON, carries an unrecognized `"type"`,
//! or is missing a field its type requires (`at`, `scenario_id`/
//! `reviewed_ids`, `status`) is skipped AND counted (never a crash,
//! never a silent drop) — one skip per malformed line, the whole file
//! is still read to its end.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use canon_model::ids::ScenarioId;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::artifact_adapter::{
    ArtifactAdapter, ArtifactEvent, ArtifactEventKind, ArtifactJoinKey, ArtifactParseOutcome, ArtifactSourceConfig, ArtifactSourceHandle,
    resolve_path_source,
};
use crate::scanner;

const ADAPTER_ID: &str = "divergence";

pub struct DivergenceAdapter;

impl ArtifactAdapter for DivergenceAdapter {
    fn adapter_id(&self) -> &'static str {
        ADAPTER_ID
    }

    fn resolve_source(&self, config: &ArtifactSourceConfig) -> Option<ArtifactSourceHandle> {
        resolve_path_source(&config.divergences_root)
    }

    fn parse(&self, source: &ArtifactSourceHandle) -> ArtifactParseOutcome {
        let root = match source {
            ArtifactSourceHandle::Path(p) => p,
            // This adapter is exclusively path-based (see module doc);
            // an already-fetched-records handle is simply not this
            // adapter's shape — never this adapter's malformed count.
            ArtifactSourceHandle::Records(_) => return ArtifactParseOutcome::empty(),
        };

        let files = scanner::scan_dir(root, |p| p.extension().and_then(|e| e.to_str()) == Some("jsonl"));

        let mut events = Vec::new();
        let mut skipped = 0usize;
        for file in &files {
            let outcome = parse_divergence_file(file);
            events.extend(outcome.events);
            skipped += outcome.skipped;
        }
        ArtifactParseOutcome { events, skipped }
    }
}

/// Loose per-line probe: only the fields this adapter's type-dispatch
/// and join-key/timestamp extraction need. `detail` on the emitted
/// event carries the FULL raw line (this probe's own fields included)
/// so `port_ref`/`upstream_ref`/`aspects: [{what,upstream,port,ref}]` and
/// every other field this schema carries survive verbatim (tasks.md
/// 2.2), without this adapter needing to model them exhaustively.
#[derive(Debug, Deserialize)]
struct DivergenceLineProbe {
    #[serde(rename = "type")]
    line_type: String,
    status: Option<String>,
    scenario_id: Option<String>,
    reviewed_ids: Option<Vec<String>>,
    at: Option<String>,
}

/// Parse one `.jsonl` file, line by line (design D2: "Each `.jsonl`
/// file is read line-by-line"). A file that cannot even be opened
/// yields an empty outcome (mirrors `adapters::omp::parse_pi_file`'s
/// non-fatal-missing-file behavior) rather than a crash.
fn parse_divergence_file(path: &Path) -> ArtifactParseOutcome {
    let Ok(file) = File::open(path) else {
        return ArtifactParseOutcome::empty();
    };

    // The `area=<area>`/`lane=<lane>` Hive partition segments live in
    // the file's own path, never trusted from the line itself — a
    // line's optional inline `"lane"` field (when present) survives
    // verbatim in `detail` only; classification always reads the path
    // partition (P2 fix, `ReviewS4Full`: a well-formed `lane=code`
    // review that omits or drifts its inline `lane` must still
    // classify as the code finding) — `ArtifactEvent.area` doc:
    // "divergence area=/surface= partition keys" folded in by the
    // emitting adapter; `lane` is this adapter's own review/design
    // classification input, threaded into `parse_divergence_line`.
    let area = hive_partition_value(path, "area");
    let lane = hive_partition_value(path, "lane");

    let mut events = Vec::new();
    let mut skipped = 0usize;

    for line in BufReader::new(file).lines() {
        let Ok(line) = line else {
            skipped += 1;
            continue;
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match parse_divergence_line(trimmed, area.as_deref(), lane.as_deref()) {
            Ok(event) => events.push(event),
            Err(()) => skipped += 1,
        }
    }

    ArtifactParseOutcome { events, skipped }
}

/// Normalize one already-trimmed, non-empty JSONL line into an
/// `ArtifactEvent`, or `Err(())` when the line is malformed relative
/// to this schema (unparseable JSON, unrecognized `"type"`, or a
/// required field missing for its type) — the caller counts every
/// `Err` as one skip (design §7).
fn parse_divergence_line(line: &str, area: Option<&str>, lane: Option<&str>) -> Result<ArtifactEvent, ()> {
    let value: serde_json::Value = serde_json::from_str(line).map_err(|_| ())?;
    let probe: DivergenceLineProbe = serde_json::from_value(value.clone()).map_err(|_| ())?;
    let at = parse_at(probe.at.as_deref().ok_or(())?)?;

    let (join_key, kind) = match probe.line_type.as_str() {
        "manifest" => {
            let reviewed_ids = probe.reviewed_ids.ok_or(())?;
            let anchor = reviewed_ids.iter().find_map(|id| ScenarioId::parse(id.clone()).ok()).ok_or(())?;
            (ArtifactJoinKey::Scenario(anchor), ArtifactEventKind::NonVerdict)
        }
        "review" => {
            let scenario = ScenarioId::parse(probe.scenario_id.ok_or(())?).map_err(|_| ())?;
            let status = probe.status.as_deref().ok_or(())?;
            let kind = if status == "open" {
                // Classifies from the file's `lane=` Hive PATH
                // partition (threaded in from `parse_divergence_file`),
                // never the line's own optional inline `"lane"` field
                // — P2 fix, `ReviewS4Full`: a well-formed `lane=code`
                // review that omits/drifts its inline `lane` must
                // still derive the code finding.
                match lane {
                    Some("code") => ArtifactEventKind::CodeReviewFinding,
                    Some("design") => ArtifactEventKind::DesignReviewFinding,
                    // An unrecognized/absent path lane still carries a
                    // real scenario-scoped event; it just doesn't
                    // resolve to a mapped verdict row.
                    _ => ArtifactEventKind::NonVerdict,
                }
            } else {
                ArtifactEventKind::NonVerdict
            };
            (ArtifactJoinKey::Scenario(scenario), kind)
        }
        "remediation" => {
            let scenario = ScenarioId::parse(probe.scenario_id.ok_or(())?).map_err(|_| ())?;
            let status = probe.status.as_deref().ok_or(())?;
            let kind = if status == "resolved" { ArtifactEventKind::RemediationResolved } else { ArtifactEventKind::NonVerdict };
            (ArtifactJoinKey::Scenario(scenario), kind)
        }
        _ => return Err(()),
    };

    Ok(ArtifactEvent {
        adapter_id: ADAPTER_ID,
        join_key,
        kind,
        // The divergence log never carries a promoting-scenario's
        // authoring role (that is a ledger `kind=review` concern,
        // `ArtifactEventKind::ReviewPromotion`, not this adapter's
        // vocabulary) — always `None` here, matching `ArtifactEvent`'s
        // own doc ("`None` for every other kind").
        authoring_role: None,
        area: area.map(str::to_string),
        // Divergence lines carry no `@reviewed`/`@ratified` trust tag
        // (that lives on ledger records) — always `None` here.
        trust_level: None,
        at,
        detail: value,
    })
}

fn parse_at(raw: &str) -> Result<DateTime<Utc>, ()> {
    DateTime::parse_from_rfc3339(raw).map(|dt| dt.with_timezone(&Utc)).map_err(|_| ())
}

/// Find a `<key>=<value>` Hive path segment among `path`'s ancestor
/// directory names (e.g. `area` in `.../lane=code/area=world/
/// surface=world-firstbuy-hotdeal/3-3-....jsonl`). `None` when no
/// ancestor carries this key — a fixture/root that doesn't follow the
/// Hive layout simply has no `area` tag, never a parse error.
fn hive_partition_value(path: &Path, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    path.ancestors().find_map(|a| a.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_prefix(prefix.as_str())).map(str::to_string))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use canon_model::ids::RoleId;

    use super::*;
    use crate::artifact_adapter::ArtifactSourceHandle;
    use crate::verdict::{Becomes, Polarity, attach_regime_key, derive_verdict};

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/divergences")
    }

    fn parse_fixture() -> ArtifactParseOutcome {
        let adapter = DivergenceAdapter;
        let config = ArtifactSourceConfig { divergences_root: Some(fixture_root()), ..Default::default() };
        let source = adapter.resolve_source(&config).expect("divergences_root configured");
        adapter.parse(&source)
    }

    #[test]
    fn adapter_id_is_divergence() {
        assert_eq!(DivergenceAdapter.adapter_id(), "divergence");
    }

    #[test]
    fn resolve_source_is_none_when_unconfigured() {
        let adapter = DivergenceAdapter;
        assert!(adapter.resolve_source(&ArtifactSourceConfig::default()).is_none());
    }

    #[test]
    fn records_handle_is_not_this_adapters_shape() {
        let adapter = DivergenceAdapter;
        let outcome = adapter.parse(&ArtifactSourceHandle::Records(Vec::new()));
        assert_eq!(outcome, ArtifactParseOutcome::empty());
    }

    #[test]
    fn fixture_corpus_yields_expected_event_kinds_and_skips_the_corrupt_line() {
        let outcome = parse_fixture();

        // manifest + open-review + remediation-resolved = 3 events;
        // exactly one corrupt line skipped.
        assert_eq!(outcome.events.len(), 3, "events: {:#?}", outcome.events);
        assert_eq!(outcome.skipped, 1);

        let kinds: Vec<ArtifactEventKind> = outcome.events.iter().map(|e| e.kind).collect();
        assert!(kinds.contains(&ArtifactEventKind::NonVerdict), "manifest line must yield a NonVerdict event");
        assert!(kinds.contains(&ArtifactEventKind::CodeReviewFinding), "lane=code open review must yield CodeReviewFinding");
        assert!(kinds.contains(&ArtifactEventKind::RemediationResolved), "resolved remediation must yield RemediationResolved");

        for event in &outcome.events {
            assert_eq!(event.adapter_id, "divergence");
            assert_eq!(event.area.as_deref(), Some("world"), "area must be folded in from the lane=code/area=world/... path");
        }
    }

    #[test]
    fn manifest_line_yields_no_verdict() {
        let outcome = parse_fixture();
        let manifest_event = outcome.events.iter().find(|e| e.kind == ArtifactEventKind::NonVerdict).expect("manifest event present");
        assert!(derive_verdict(manifest_event.kind, None).is_none());
        // The full reviewed_ids list survives verbatim in `detail`.
        assert_eq!(manifest_event.detail["type"], "manifest");
        assert!(manifest_event.detail["reviewed_ids"].as_array().unwrap().len() >= 2);
    }

    #[test]
    fn open_review_becomes_dev_failure_guardrail_candidate() {
        let outcome = parse_fixture();
        let finding = outcome.events.iter().find(|e| e.kind == ArtifactEventKind::CodeReviewFinding).expect("open review event present");
        let row = derive_verdict(finding.kind, None).expect("open finding must derive a verdict");
        assert_eq!(row.role.as_str(), "dev");
        assert_eq!(row.polarity, Polarity::Failure);
        assert_eq!(row.becomes, Becomes::GuardrailCandidate);
        // Structured aspects survive verbatim.
        let aspects = finding.detail["aspects"].as_array().expect("aspects array present");
        assert!(!aspects.is_empty());
        assert!(aspects[0].get("what").is_some());
        assert!(aspects[0].get("upstream").is_some());
        assert!(aspects[0].get("port").is_some());
        assert!(aspects[0].get("ref").is_some());
    }

    #[test]
    fn remediation_resolved_becomes_dev_success_strategy_candidate() {
        let outcome = parse_fixture();
        let resolved = outcome.events.iter().find(|e| e.kind == ArtifactEventKind::RemediationResolved).expect("remediation-resolved event present");
        let row = derive_verdict(resolved.kind, None).expect("resolved remediation must derive a verdict");
        assert_eq!(row.role.as_str(), "dev");
        assert_eq!(row.polarity, Polarity::Success);
        assert_eq!(row.becomes, Becomes::StrategyCandidate);
    }

    #[test]
    fn golden_verdict_stream_from_the_fixture_corpus() {
        let outcome = parse_fixture();
        let mut verdicts: Vec<_> = outcome
            .events
            .iter()
            .filter_map(|event| {
                let row = derive_verdict(event.kind, event.authoring_role.as_ref())?;
                let area = event.area.clone().unwrap_or_default();
                Some(attach_regime_key(row, event.join_key.clone(), "canon", &area, "abc123", event.trust_level.clone()).expect("valid regime key"))
            })
            .collect();
        verdicts.sort_by(|a, b| a.regime_key.as_str().cmp(b.regime_key.as_str()));

        // Golden shape: exactly the two verdict-bearing lines
        // (manifest never derives one), both role=dev, sharing the
        // `dev/canon/world/` regime-key prefix (same role/repo/area).
        assert_eq!(verdicts.len(), 2);
        for verdict in &verdicts {
            assert_eq!(verdict.row.role.as_str(), "dev");
            assert!(verdict.regime_key.as_str().starts_with("dev/canon/world/"));
        }
        let polarities: Vec<_> = verdicts.iter().map(|v| v.row.polarity).collect();
        assert!(polarities.contains(&Polarity::Failure));
        assert!(polarities.contains(&Polarity::Success));
    }

    #[test]
    fn corrupt_line_is_skipped_not_a_crash() {
        let outcome = parse_fixture();
        assert_eq!(outcome.skipped, 1);
    }

    #[test]
    fn re_parsing_the_unchanged_fixture_is_idempotent() {
        let first = parse_fixture();
        let second = parse_fixture();
        assert_eq!(first, second, "re-parsing an unchanged corpus must yield byte-identical events/skipped counts");

        let first_json = serde_json::to_value(first.events.iter().map(|e| &e.detail).collect::<Vec<_>>()).unwrap();
        let second_json = serde_json::to_value(second.events.iter().map(|e| &e.detail).collect::<Vec<_>>()).unwrap();
        assert_eq!(serde_json::to_vec(&first_json).unwrap(), serde_json::to_vec(&second_json).unwrap());
    }

    #[test]
    fn unconfigured_source_never_scans_a_hardcoded_path() {
        // No `divergences_root` configured -> `resolve_source` is
        // `None` -> the caller never invokes `parse` at all (mirrors
        // `artifact_registry::resolve_and_parse`'s contract) — this
        // adapter has no compiled-in donor-repo fallback to prove
        // absent by omission.
        let adapter = DivergenceAdapter;
        assert!(adapter.resolve_source(&ArtifactSourceConfig::default()).is_none());
    }

    #[test]
    fn deferred_review_status_yields_no_verdict() {
        // A `review` line whose status is not `open` (e.g. a prior
        // round's `deferred`) still normalizes to a scenario-keyed
        // event, but never a Finding kind.
        let line = r#"{"schema":1,"type":"review","lane":"code","surface":"world-firstbuy-hotdeal","scenario_id":"world.firstbuy-hotdeal.14","status":"deferred","at":"2026-07-07T12:00:00Z"}"#;
        let event = parse_divergence_line(line, Some("world"), Some("code")).expect("well-formed line parses");
        assert_eq!(event.kind, ArtifactEventKind::NonVerdict);
        assert!(derive_verdict(event.kind, None).is_none());
    }

    #[test]
    fn remediation_provenance_only_status_yields_no_verdict() {
        // A `remediation` line pinned to the vendor's provenance-only
        // status never greens a cell by itself.
        let line = r#"{"schema":1,"type":"remediation","lane":"code","surface":"world-firstbuy-hotdeal","scenario_id":"world.firstbuy-hotdeal.14","status":"remediated","at":"2026-07-08T09:00:00Z"}"#;
        let event = parse_divergence_line(line, Some("world"), Some("code")).expect("well-formed line parses");
        assert_eq!(event.kind, ArtifactEventKind::NonVerdict);
    }

    #[test]
    fn design_lane_open_review_becomes_design_review_finding() {
        let line = r#"{"schema":1,"type":"review","lane":"design","surface":"world-firstbuy-hotdeal","scenario_id":"world.firstbuy-hotdeal.14","status":"open","at":"2026-07-07T12:00:00Z"}"#;
        let event = parse_divergence_line(line, Some("world"), Some("design")).expect("well-formed line parses");
        assert_eq!(event.kind, ArtifactEventKind::DesignReviewFinding);
        let row = derive_verdict(event.kind, None).unwrap();
        assert_eq!(row.role.as_str(), "design");
    }

    #[test]
    fn lane_from_path_partition_wins_when_inline_lane_is_absent() {
        // P2 fix, `ReviewS4Full`: a well-formed `lane=code` review
        // that carries no inline `"lane"` field at all must still
        // classify from the file's Hive PATH partition, deriving the
        // dev/code failure guardrail-candidate verdict.
        let line = r#"{"schema":1,"type":"review","surface":"world-firstbuy-hotdeal","scenario_id":"world.firstbuy-hotdeal.14","status":"open","at":"2026-07-07T12:00:00Z"}"#;
        let event = parse_divergence_line(line, Some("world"), Some("code")).expect("well-formed line parses");
        assert_eq!(event.kind, ArtifactEventKind::CodeReviewFinding);
        let row = derive_verdict(event.kind, None).expect("open code finding must derive a verdict");
        assert_eq!(row.role.as_str(), "dev");
        assert_eq!(row.polarity, Polarity::Failure);
        assert_eq!(row.becomes, Becomes::GuardrailCandidate);
    }

    #[test]
    fn malformed_json_line_is_rejected() {
        assert_eq!(parse_divergence_line("{not json", None, None), Err(()));
    }

    #[test]
    fn unrecognized_type_is_rejected() {
        let line = r#"{"schema":1,"type":"unknown","at":"2026-07-07T12:00:00Z"}"#;
        assert_eq!(parse_divergence_line(line, None, None), Err(()));
    }

    #[test]
    fn manifest_with_no_parseable_reviewed_id_is_rejected() {
        let line = r#"{"schema":1,"type":"manifest","reviewed_ids":[],"at":"2026-07-07T12:00:00Z"}"#;
        assert_eq!(parse_divergence_line(line, None, None), Err(()));
    }

    #[test]
    fn review_line_missing_scenario_id_is_rejected() {
        let line = r#"{"schema":1,"type":"review","status":"open","lane":"code","at":"2026-07-07T12:00:00Z"}"#;
        assert_eq!(parse_divergence_line(line, None, None), Err(()));
    }

    #[test]
    fn hive_partition_value_extracts_area_from_ancestor_dirs() {
        let path = Path::new("/root/lane=code/area=world/surface=world-firstbuy-hotdeal/3-3-x.jsonl");
        assert_eq!(hive_partition_value(path, "area"), Some("world".to_string()));
        assert_eq!(hive_partition_value(path, "lane"), Some("code".to_string()));
        assert_eq!(hive_partition_value(path, "surface"), Some("world-firstbuy-hotdeal".to_string()));
        assert_eq!(hive_partition_value(path, "missing"), None);
    }

    #[test]
    fn authoring_role_stays_none_for_every_kind_the_adapter_emits() {
        // ReviewPromotion is a ledger concept, never emitted by this
        // adapter — RoleId import used only to keep this contract
        // documented for a future reader diffing against `verdict.rs`.
        let _unused_marker: Option<RoleId> = None;
        let outcome = parse_fixture();
        assert!(outcome.events.iter().all(|e| e.authoring_role.is_none()));
    }
}
