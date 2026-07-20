//! The ledger adapter (S4 wave-2, design D1) — reads a
//! `canon.yaml`-configured, Hive-partitioned tree of ledger JSON
//! records (`ArtifactSourceConfig::ledger_root`) and normalizes each
//! record into an [`ArtifactEvent`] keyed by `scenario_id`.
//!
//! Grounded in `design.md` D1 + `specs/review-verdict-mapping/spec.md`
//! and the parity-harness donor audit. The donor consumer repo's
//! `spec/ledger/` tree is the reference donor SHAPE
//! only — this adapter never reads it; its FROZEN fixture corpus
//! (`tests/fixtures/ledger/`) is a hand-authored, checked-in sample in
//! the identical Hive layout (S4 foundation rescope, design D6).
//!
//! ## Layout (design D1, ledger-reader.md §3.3)
//! One JSON object per file, walked from the configured root:
//! - `review`/`design-review`/`code-review`/`clear` records live at
//!   `kind=<kind>/area=<area>/<scenario_id>.json` — THREE path
//!   segments, where `area` is not trusted from the directory but
//!   **recomputed from the record's own `scenario_id`** (its first
//!   dot-segment, `_area_of` in the donor) — a file whose directory
//!   `area=` disagrees with its own `scenario_id`, or whose basename
//!   isn't exactly `<scenario_id>.json`, is layout-malformed
//!   (ledger-reader.md §3.3's "misfiled = malformed = violation").
//! - `run`/`drill` records live flat at `kind=<kind>/<file>.json` —
//!   TWO path segments, no `area=` (they span areas via a
//!   `scenario_ids: []` array instead of one `scenario_id`).
//!
//! Every record's own `kind` field (defaulting to `"run"` when
//! absent, matching the donor's legacy-record default) must equal the
//! directory's `kind=<kind>` segment — a mismatch is a layout
//! violation, not a silent directory override (D1: "walks
//! `kind=<kind>/[area=<area>/]*.json` exactly as parity.py's
//! `_load_ledger`/`_ledger_layout_problem` do today").
//!
//! ## Kind → event-kind dispatch (design §5 S4 table, ArtifactEventKind doc)
//! - `kind=review` → [`ArtifactEventKind::ReviewPromotion`] always (a
//!   `review` record's mere existence IS the promotion-to-`@reviewed`
//!   event).
//! - `kind=clear` → [`ArtifactEventKind::ClearAfterFlagged`] always.
//! - `kind=design-review` / `kind=code-review` → the matching
//!   `*Finding` variant when `verdict` is absent or not exactly
//!   `"faithful"` (frozen `ArtifactEventKind` doc: "verdict absent or
//!   not `faithful` — an open/still-divergent finding"; this adapter
//!   reads that condition literally, so a hypothetical `"n-a"` value
//!   is NOT carved out as a special case — no such value has been
//!   documented as a finding exception anywhere in the frozen
//!   contract), else [`ArtifactEventKind::NonVerdict`] (a `faithful`
//!   record closes the finding, it does not open one).
//! - `kind=run` / `kind=drill` → always
//!   [`ArtifactEventKind::NonVerdict`] (`ArtifactEventKind::NonVerdict`
//!   doc: "a ledger `run`/`drill` record (D1)") — one event PER
//!   `scenario_id` entry in the record's `scenario_ids` array, each
//!   individually parsed (an unparseable entry is dropped from the
//!   list, not a whole-file skip — the record itself is well-formed).
//!
//! `authoring_role` (needed only for `ReviewPromotion`, task 5.1) is
//! read from a `review` record by field name — `authoring_role` /
//! `author_role` / `role`, first match wins — via `Option<T>`, per D1
//! ("the adapter reads by field name with `Option<T>`, not a
//! fixed-arity struct match"): today's donor `review` record shape
//! (`schema`, `kind`, `scenario_id`, `upstream_ref`, `pin`, `reviewer`,
//! `at`) carries none of these, so `authoring_role` degrades to `None`
//! (and `derive_verdict` correctly yields no verdict rather than
//! fabricating a role) until a future schema upgrade backfills one —
//! exactly the graceful-degrade D1 requires, exercised directly against
//! a synthetic record in this module's tests since the frozen fixture
//! corpus reflects today's donor shape.
//!
//! A record that fails to parse as JSON, carries a `kind` this adapter
//! does not recognize, fails its layout check, or is missing a field
//! its kind requires (`scenario_id`/`scenario_ids`, a derivable `at`)
//! is skipped AND counted (never a crash) — one skip per malformed
//! file (a ledger record is always exactly one JSON object per file,
//! so file-level and record-level skip-and-continue coincide here).

use std::path::Path;

use canon_model::ids::{RoleId, ScenarioId};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use serde::Deserialize;

use crate::artifact_adapter::{
    ArtifactAdapter, ArtifactEvent, ArtifactEventKind, ArtifactJoinKey, ArtifactParseOutcome, ArtifactSourceConfig, ArtifactSourceHandle,
    resolve_path_source,
};
use crate::scanner;

const ADAPTER_ID: &str = "ledger";

pub struct LedgerAdapter;

impl ArtifactAdapter for LedgerAdapter {
    fn adapter_id(&self) -> &'static str {
        ADAPTER_ID
    }

    fn resolve_source(&self, config: &ArtifactSourceConfig) -> Option<ArtifactSourceHandle> {
        resolve_path_source(&config.ledger_root)
    }

    fn parse(&self, source: &ArtifactSourceHandle) -> ArtifactParseOutcome {
        let root = match source {
            ArtifactSourceHandle::Path(p) => p,
            // This adapter is exclusively path-based — an
            // already-fetched-records handle is simply not this
            // adapter's shape (that is the handoff adapter's
            // territory), never this adapter's malformed count.
            ArtifactSourceHandle::Records(_) => return ArtifactParseOutcome::empty(),
        };

        let files = scanner::scan_dir(root, |p| p.extension().and_then(|e| e.to_str()) == Some("json"));

        let mut events = Vec::new();
        let mut skipped = 0usize;
        for file in &files {
            match parse_ledger_file(root, file) {
                Some(mut file_events) => events.append(&mut file_events),
                None => skipped += 1,
            }
        }
        ArtifactParseOutcome { events, skipped }
    }
}

/// Loose per-record probe: every field name any ledger kind this
/// adapter recognizes might carry, all `Option<T>` (D1: "reads by
/// field name with `Option<T>`, not a fixed-arity struct match") — a
/// field absent from a given kind's real shape simply stays `None`,
/// never a deserialize failure. `detail` on the emitted event carries
/// the FULL raw record (this probe's own fields included) so
/// `port_ref`/`upstream_ref`/`pin`/`reviewer`/`evidence`/… survive
/// verbatim without this adapter needing to model them exhaustively.
#[derive(Debug, Deserialize)]
struct LedgerRecord {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    scenario_id: Option<String>,
    #[serde(default)]
    scenario_ids: Option<Vec<String>>,
    #[serde(default)]
    verdict: Option<String>,
    #[serde(default)]
    at: Option<String>,
    #[serde(default)]
    trust_level: Option<String>,
    #[serde(default)]
    authoring_role: Option<String>,
    #[serde(default)]
    author_role: Option<String>,
    #[serde(default)]
    role: Option<String>,
}

impl LedgerRecord {
    /// First match wins across the candidate field names a future
    /// schema upgrade might use — see module doc's `authoring_role`
    /// discussion.
    fn resolve_authoring_role(&self) -> Option<RoleId> {
        self.authoring_role
            .as_deref()
            .or(self.author_role.as_deref())
            .or(self.role.as_deref())
            .and_then(|slug| RoleId::parse(slug).ok())
    }
}

/// Parse one already-located ledger file into zero or more events, or
/// `None` when the WHOLE file is malformed (design §7: skip AND
/// count, never a crash) — the caller counts a `None` as exactly one
/// skip. `root` is needed to compute the file's Hive path segments
/// relative to the configured source root.
fn parse_ledger_file(root: &Path, path: &Path) -> Option<Vec<ArtifactEvent>> {
    let content = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    let record: LedgerRecord = serde_json::from_value(value.clone()).ok()?;

    let components = relative_components(root, path);
    // Donor default: a record with no `kind` field is legacy-`run`
    // (ledger-reader.md §3.1: `kind = rec.get("kind", "run")`).
    let record_kind = record.kind.clone().unwrap_or_else(|| "run".to_string());

    match record_kind.as_str() {
        "run" | "drill" => parse_flat_record(&record, &record_kind, &components, path, &value),
        "review" | "design-review" | "code-review" | "clear" => {
            parse_partitioned_record(&record, &record_kind, &components, &value).map(|event| vec![event])
        }
        // Unrecognized kind — this adapter has no `ArtifactEventKind`
        // to classify it under; skip rather than guess.
        _ => None,
    }
}

/// `run`/`drill`: flat `kind=<kind>/<file>.json`, no `area=` segment
/// — spans areas via a `scenario_ids: []` array instead of one
/// `scenario_id`. Always [`ArtifactEventKind::NonVerdict`] (design
/// D1) — one event per parseable entry in `scenario_ids`, each
/// carrying its OWN area (that scenario id's first dot-segment).
fn parse_flat_record(record: &LedgerRecord, kind: &str, components: &[String], path: &Path, value: &serde_json::Value) -> Option<Vec<ArtifactEvent>> {
    if components.len() != 2 || components[0] != format!("kind={kind}") {
        return None;
    }
    let raw_ids = record.scenario_ids.as_ref()?;
    let at = resolve_at(record.at.as_deref(), path)?;

    let scenarios: Vec<ScenarioId> = raw_ids.iter().filter_map(|s| ScenarioId::parse(s.clone()).ok()).collect();
    if scenarios.is_empty() {
        // Either an empty array or every entry unparseable — no
        // join-spine identity to anchor any event on.
        return None;
    }

    Some(
        scenarios
            .into_iter()
            .map(|scenario| {
                let area = area_of(&scenario);
                ArtifactEvent {
                    adapter_id: ADAPTER_ID,
                    join_key: ArtifactJoinKey::Scenario(scenario),
                    kind: ArtifactEventKind::NonVerdict,
                    authoring_role: None,
                    area: Some(area),
                    trust_level: None,
                    at,
                    detail: value.clone(),
                }
            })
            .collect(),
    )
}

/// `review`/`design-review`/`code-review`/`clear`: Hive-partitioned
/// `kind=<kind>/area=<area>/<scenario_id>.json` — THREE path
/// segments, `area` recomputed from `scenario_id` (never trusted from
/// the directory, ledger-reader.md §3.3) and the basename validated
/// to equal `<scenario_id>.json` exactly.
fn parse_partitioned_record(record: &LedgerRecord, kind: &str, components: &[String], value: &serde_json::Value) -> Option<ArtifactEvent> {
    if components.len() != 3 || components[0] != format!("kind={kind}") {
        return None;
    }
    let scenario = ScenarioId::parse(record.scenario_id.clone()?).ok()?;
    let area = area_of(&scenario);
    if components[1] != format!("area={area}") {
        return None;
    }
    if components[2] != format!("{}.json", scenario.as_str()) {
        return None;
    }
    let at = parse_at(record.at.as_deref()?).ok()?;

    let event_kind = match kind {
        "review" => ArtifactEventKind::ReviewPromotion,
        "clear" => ArtifactEventKind::ClearAfterFlagged,
        "design-review" => {
            if is_faithful(&record.verdict) {
                ArtifactEventKind::NonVerdict
            } else {
                ArtifactEventKind::DesignReviewFinding
            }
        }
        "code-review" => {
            if is_faithful(&record.verdict) {
                ArtifactEventKind::NonVerdict
            } else {
                ArtifactEventKind::CodeReviewFinding
            }
        }
        _ => unreachable!("dispatched only for review/design-review/code-review/clear"),
    };

    Some(ArtifactEvent {
        adapter_id: ADAPTER_ID,
        join_key: ArtifactJoinKey::Scenario(scenario),
        kind: event_kind,
        authoring_role: record.resolve_authoring_role(),
        area: Some(area),
        trust_level: record.trust_level.clone(),
        at,
        detail: value.clone(),
    })
}

/// A `verdict` field closes a finding only when it is exactly
/// `"faithful"` — see module doc's dispatch section for why no other
/// value (including a hypothetical `"n-a"`) is carved out here.
fn is_faithful(verdict: &Option<String>) -> bool {
    verdict.as_deref() == Some("faithful")
}

/// `_area_of` (ledger-reader.md §3.3): a `scenario_id`'s first
/// dot-segment, e.g. `world.firstbuy-hotdeal.26` → `world`. `scenario`
/// is already a validated [`ScenarioId`] (exactly three dot-segments),
/// so the first `.`-split segment always exists.
fn area_of(scenario: &ScenarioId) -> String {
    scenario.as_str().split('.').next().expect("ScenarioId grammar guarantees at least one dot-segment").to_string()
}

fn parse_at(raw: &str) -> Result<DateTime<Utc>, ()> {
    DateTime::parse_from_rfc3339(raw).map(|dt| dt.with_timezone(&Utc)).map_err(|_| ())
}

/// Resolve a `run`/`drill` record's timestamp: prefer the record's own
/// `at` field when present and parseable, else fall back to the
/// filename's leading `<stamp>-…` segment (`%Y%m%dT%H%M%S`, the donor's
/// `_RUN_FN_RE` shape, ledger-reader.md §3.3) — `run`/`drill` records
/// are not guaranteed an `at` field the way review-kind records are
/// (`_RUN_REQUIRED` never lists it), so the filename is the fallback
/// timestamp source, not a second required field.
fn resolve_at(raw: Option<&str>, path: &Path) -> Option<DateTime<Utc>> {
    if let Some(raw) = raw {
        if let Ok(at) = parse_at(raw) {
            return Some(at);
        }
    }
    filename_stamp(path)
}

fn filename_stamp(path: &Path) -> Option<DateTime<Utc>> {
    let stem = path.file_stem()?.to_str()?;
    let stamp = stem.split('-').next()?;
    let naive = NaiveDateTime::parse_from_str(stamp, "%Y%m%dT%H%M%S").ok()?;
    Some(Utc.from_utc_datetime(&naive))
}

/// The file's path segments relative to the configured source root,
/// as owned strings (e.g. `["kind=review", "area=world",
/// "world.firstbuy-hotdeal.26.json"]`). Empty when `path` is not
/// actually under `root` (never happens for paths `scanner::scan_dir`
/// itself returned, but kept total rather than panicking).
fn relative_components(root: &Path, path: &Path) -> Vec<String> {
    path.strip_prefix(root)
        .map(|rel| rel.components().filter_map(|c| c.as_os_str().to_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::artifact_adapter::ArtifactSourceHandle;
    use crate::verdict::{Becomes, Polarity, attach_regime_key, derive_verdict};

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ledger")
    }

    fn parse_fixture() -> ArtifactParseOutcome {
        let adapter = LedgerAdapter;
        let config = ArtifactSourceConfig { ledger_root: Some(fixture_root()), ..Default::default() };
        let source = adapter.resolve_source(&config).expect("ledger_root configured");
        adapter.parse(&source)
    }

    #[test]
    fn adapter_id_is_ledger() {
        assert_eq!(LedgerAdapter.adapter_id(), "ledger");
    }

    #[test]
    fn resolve_source_is_none_when_unconfigured() {
        let adapter = LedgerAdapter;
        assert!(adapter.resolve_source(&ArtifactSourceConfig::default()).is_none());
    }

    #[test]
    fn unconfigured_source_never_scans_a_hardcoded_path() {
        // No `ledger_root` configured -> `resolve_source` is `None` ->
        // the caller never invokes `parse` at all — this adapter has
        // no compiled-in donor-repo fallback to prove absent by
        // omission.
        let adapter = LedgerAdapter;
        assert!(adapter.resolve_source(&ArtifactSourceConfig::default()).is_none());
    }

    #[test]
    fn records_handle_is_not_this_adapters_shape() {
        let adapter = LedgerAdapter;
        let outcome = adapter.parse(&ArtifactSourceHandle::Records(Vec::new()));
        assert_eq!(outcome, ArtifactParseOutcome::empty());
    }

    #[test]
    fn fixture_corpus_yields_expected_events_and_skips_the_malformed_record() {
        let outcome = parse_fixture();

        // review (1) + code-review finding (1) + run (2 scenario_ids)
        // = 4 events; exactly one malformed record skipped.
        assert_eq!(outcome.events.len(), 4, "events: {:#?}", outcome.events);
        assert_eq!(outcome.skipped, 1);

        let kinds: Vec<ArtifactEventKind> = outcome.events.iter().map(|e| e.kind).collect();
        assert!(kinds.contains(&ArtifactEventKind::ReviewPromotion), "kind=review must yield ReviewPromotion");
        assert!(kinds.contains(&ArtifactEventKind::CodeReviewFinding), "non-faithful code-review must yield CodeReviewFinding");
        assert_eq!(kinds.iter().filter(|k| **k == ArtifactEventKind::NonVerdict).count(), 2, "both run scenario_ids yield NonVerdict");

        for event in &outcome.events {
            assert_eq!(event.adapter_id, "ledger");
        }
    }

    #[test]
    fn review_promotion_without_authoring_role_yields_no_verdict() {
        let outcome = parse_fixture();
        let promotion = outcome.events.iter().find(|e| e.kind == ArtifactEventKind::ReviewPromotion).expect("review event present");
        // Today's donor `review` shape carries no authoring-role
        // field — degrade to no verdict, never a fabricated role.
        assert!(promotion.authoring_role.is_none());
        assert!(derive_verdict(promotion.kind, promotion.authoring_role.as_ref()).is_none());
        assert_eq!(promotion.area.as_deref(), Some("world"));
    }

    #[test]
    fn code_review_finding_becomes_dev_failure_guardrail_candidate() {
        let outcome = parse_fixture();
        let finding = outcome.events.iter().find(|e| e.kind == ArtifactEventKind::CodeReviewFinding).expect("code-review finding present");
        let row = derive_verdict(finding.kind, None).expect("open finding must derive a verdict");
        assert_eq!(row.role.as_str(), "dev");
        assert_eq!(row.polarity, Polarity::Failure);
        assert_eq!(row.becomes, Becomes::GuardrailCandidate);
        // Provenance fields survive verbatim in `detail`.
        assert!(finding.detail.get("port_ref").is_some());
        assert!(finding.detail.get("upstream_ref").is_some());
    }

    #[test]
    fn run_records_are_non_verdict_and_carry_their_own_area() {
        let outcome = parse_fixture();
        let runs: Vec<_> = outcome.events.iter().filter(|e| e.detail.get("kind").and_then(|v| v.as_str()) == Some("run")).collect();
        assert_eq!(runs.len(), 2);
        for run in &runs {
            assert!(derive_verdict(run.kind, None).is_none());
            assert!(run.area.is_some());
        }
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

        // Golden shape: exactly the ONE verdict-bearing record (the
        // non-faithful code-review finding) — review-promotion has no
        // authoring role, run records are always NonVerdict.
        assert_eq!(verdicts.len(), 1);
        let verdict = &verdicts[0];
        assert_eq!(verdict.row.role.as_str(), "dev");
        assert_eq!(verdict.row.polarity, Polarity::Failure);
        assert_eq!(verdict.row.becomes, Becomes::GuardrailCandidate);
        assert!(verdict.regime_key.as_str().starts_with("dev/canon/world/"));
    }

    #[test]
    fn malformed_record_is_skipped_not_a_crash() {
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

    // ── Inline record-level tests (synthetic records, no fixture files) ──

    fn value_of(json: &str) -> serde_json::Value {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn review_promotion_with_authoring_role_present_derives_a_verdict() {
        // Proves D1's forward-compat claim ("S11's later schema
        // upgrade backfills the richer fields without requiring an S4
        // adapter rewrite") against a synthetic record carrying a
        // field today's donor shape doesn't have.
        let value = value_of(
            r#"{"schema":1,"kind":"review","scenario_id":"world.firstbuy-hotdeal.26","authoring_role":"content","upstream_ref":"x","pin":"p","reviewer":"r","at":"2026-07-09T10:00:00Z"}"#,
        );
        let record: LedgerRecord = serde_json::from_value(value.clone()).unwrap();
        let components = vec!["kind=review".to_string(), "area=world".to_string(), "world.firstbuy-hotdeal.26.json".to_string()];
        let event = parse_partitioned_record(&record, "review", &components, &value).expect("well-formed record parses");
        assert_eq!(event.authoring_role.as_ref().map(RoleId::as_str), Some("content"));
        let row = derive_verdict(event.kind, event.authoring_role.as_ref()).expect("authoring role present must derive a verdict");
        assert_eq!(row.role.as_str(), "content");
        assert_eq!(row.polarity, Polarity::Success);
        assert_eq!(row.becomes, Becomes::StrategyCandidate);
    }

    #[test]
    fn code_review_faithful_verdict_yields_non_verdict() {
        let value = value_of(
            r#"{"schema":1,"kind":"code-review","scenario_id":"world.firstbuy-hotdeal.14","verdict":"faithful","port_ref":"x","upstream_ref":"y","pin":"p","reviewer":"r","at":"2026-07-08T20:05:49Z"}"#,
        );
        let record: LedgerRecord = serde_json::from_value(value.clone()).unwrap();
        let components = vec!["kind=code-review".to_string(), "area=world".to_string(), "world.firstbuy-hotdeal.14.json".to_string()];
        let event = parse_partitioned_record(&record, "code-review", &components, &value).expect("well-formed record parses");
        assert_eq!(event.kind, ArtifactEventKind::NonVerdict);
        assert!(derive_verdict(event.kind, None).is_none());
    }

    #[test]
    fn design_review_absent_verdict_becomes_a_design_finding() {
        let value = value_of(
            r#"{"schema":1,"kind":"design-review","scenario_id":"world.firstbuy-hotdeal.14","upstream_ref":"y","pin":"p","reviewer":"r","at":"2026-07-08T20:05:49Z"}"#,
        );
        let record: LedgerRecord = serde_json::from_value(value.clone()).unwrap();
        let components = vec!["kind=design-review".to_string(), "area=world".to_string(), "world.firstbuy-hotdeal.14.json".to_string()];
        let event = parse_partitioned_record(&record, "design-review", &components, &value).expect("well-formed record parses");
        assert_eq!(event.kind, ArtifactEventKind::DesignReviewFinding);
        let row = derive_verdict(event.kind, None).unwrap();
        assert_eq!(row.role.as_str(), "design");
    }

    #[test]
    fn clear_record_becomes_clear_after_flagged() {
        let value = value_of(r#"{"schema":1,"kind":"clear","scenario_id":"world.firstbuy-hotdeal.14","reviewer":"r","pin":"p","at":"2026-07-09T09:00:00Z"}"#);
        let record: LedgerRecord = serde_json::from_value(value.clone()).unwrap();
        let components = vec!["kind=clear".to_string(), "area=world".to_string(), "world.firstbuy-hotdeal.14.json".to_string()];
        let event = parse_partitioned_record(&record, "clear", &components, &value).expect("well-formed record parses");
        assert_eq!(event.kind, ArtifactEventKind::ClearAfterFlagged);
        let row = derive_verdict(event.kind, None).unwrap();
        assert_eq!(row.role.as_str(), "review");
        assert_eq!(row.polarity, Polarity::Corrective);
    }

    #[test]
    fn area_directory_mismatched_with_scenario_id_is_layout_malformed() {
        // ledger-reader.md §3.3's central gotcha: `area` is derived
        // from `scenario_id`, never trusted from the directory it
        // happens to sit in.
        let value = value_of(r#"{"schema":1,"kind":"review","scenario_id":"world.firstbuy-hotdeal.26","reviewer":"r","pin":"p","at":"2026-07-09T10:00:00Z"}"#);
        let record: LedgerRecord = serde_json::from_value(value.clone()).unwrap();
        let components = vec!["kind=review".to_string(), "area=promise-date".to_string(), "world.firstbuy-hotdeal.26.json".to_string()];
        assert!(parse_partitioned_record(&record, "review", &components, &value).is_none());
    }

    #[test]
    fn filename_not_matching_scenario_id_is_layout_malformed() {
        let value = value_of(r#"{"schema":1,"kind":"review","scenario_id":"world.firstbuy-hotdeal.26","reviewer":"r","pin":"p","at":"2026-07-09T10:00:00Z"}"#);
        let record: LedgerRecord = serde_json::from_value(value.clone()).unwrap();
        let components = vec!["kind=review".to_string(), "area=world".to_string(), "wrong-name.json".to_string()];
        assert!(parse_partitioned_record(&record, "review", &components, &value).is_none());
    }

    #[test]
    fn record_kind_mismatched_with_directory_kind_is_rejected() {
        // Record declares `"kind":"clear"` but sits under a
        // `kind=review/` directory — `parse_ledger_file` dispatches on
        // the RECORD's own kind, so it expects a `kind=clear/...`
        // layout and rejects the mismatch.
        let value = value_of(r#"{"schema":1,"kind":"clear","scenario_id":"world.firstbuy-hotdeal.26","reviewer":"r","pin":"p","at":"2026-07-09T10:00:00Z"}"#);
        let dir = tempfile::tempdir().unwrap();
        let kind_dir = dir.path().join("kind=review").join("area=world");
        std::fs::create_dir_all(&kind_dir).unwrap();
        let file = kind_dir.join("world.firstbuy-hotdeal.26.json");
        std::fs::write(&file, value.to_string()).unwrap();
        assert!(parse_ledger_file(dir.path(), &file).is_none());
    }

    #[test]
    fn missing_scenario_ids_on_a_run_record_is_malformed() {
        let value = value_of(r#"{"schema":1,"kind":"run","result":"pass","by":"flutter-test-machine","at":"2026-07-07T15:52:27Z"}"#);
        let record: LedgerRecord = serde_json::from_value(value.clone()).unwrap();
        let components = vec!["kind=run".to_string(), "20260707T155227-unit-x.json".to_string()];
        let path = PathBuf::from("/root/kind=run/20260707T155227-unit-x.json");
        assert!(parse_flat_record(&record, "run", &components, &path, &value).is_none());
    }

    #[test]
    fn run_record_falls_back_to_filename_stamp_when_at_is_absent() {
        let value = value_of(r#"{"schema":1,"kind":"run","scenario_ids":["world.firstbuy-hotdeal.26"],"result":"pass"}"#);
        let record: LedgerRecord = serde_json::from_value(value.clone()).unwrap();
        let components = vec!["kind=run".to_string(), "20260707T155227-unit-2745ca4c8-efd5b2.json".to_string()];
        let path = PathBuf::from("/root/kind=run/20260707T155227-unit-2745ca4c8-efd5b2.json");
        let events = parse_flat_record(&record, "run", &components, &path, &value).expect("well-formed record parses");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].at, Utc.with_ymd_and_hms(2026, 7, 7, 15, 52, 27).unwrap());
    }

    #[test]
    fn malformed_json_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let kind_dir = dir.path().join("kind=review").join("area=world");
        std::fs::create_dir_all(&kind_dir).unwrap();
        let file = kind_dir.join("world.broken.99.json");
        std::fs::write(&file, "{not valid json").unwrap();
        assert!(parse_ledger_file(dir.path(), &file).is_none());
    }

    #[test]
    fn unrecognized_kind_is_rejected() {
        let value = value_of(r#"{"schema":1,"kind":"portreview","scenario_id":"world.firstbuy-hotdeal.26","at":"2026-07-09T10:00:00Z"}"#);
        let dir = tempfile::tempdir().unwrap();
        let kind_dir = dir.path().join("kind=portreview");
        std::fs::create_dir_all(&kind_dir).unwrap();
        let file = kind_dir.join("world.firstbuy-hotdeal.26.json");
        std::fs::write(&file, value.to_string()).unwrap();
        assert!(parse_ledger_file(dir.path(), &file).is_none());
    }

    #[test]
    fn area_of_derives_first_dot_segment() {
        let scenario = ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap();
        assert_eq!(area_of(&scenario), "world");
    }
}
