//! Openspec task-state adapter (S4 wave-2, tasks 4.1/4.2; design D4)
//! — reads a `canon.yaml`-configured `openspec_root`
//! ([`crate::artifact_adapter::ArtifactSourceConfig::openspec_root`])
//! for `openspec/changes/*/tasks.md` files and normalizes each
//! checkbox row into an [`ArtifactEvent`] keyed by `task_id`
//! (`<change_id>#<n>`, S1 join spine).
//!
//! # Checkbox grammar
//! Row parsing (`- [ ]`/`- [x]` + annotation + evidence suffix) and
//! `task_id` derivation live in [`crate::task_rows`] — the single
//! dialect-neutral grammar both this adapter and s17's plan adapters
//! read (design D5; s35 D2 unified it with the gate crate's former own
//! copy — `canon-ingest` still has no `canon-gate` dependency). This
//! adapter owns only how a parsed row
//! becomes (or doesn't become) an [`ArtifactEvent`] — see below.
//!
//! # Two readers, one join (design R5)
//! [`crate::plan_adapters::openspec`] (s17 P2) reads the SAME
//! `openspec/changes/**` tree for a DIFFERENT job: plan-STATE
//! `Change`/`Task` records, not verdict events. Both derive `task_id`
//! through the one shared [`crate::task_rows::task_id_for`]
//! function (design D5), so this adapter's `ArtifactEvent`s and that
//! one's `Task` records join on that key without ever double-counting
//! each other — the plan side emits a `Task` candidate for EVERY row
//! this adapter reads (plus untouched-open rows this adapter skips,
//! since a plain `- [ ]` with no annotation produces no verdict event
//! from a single point-in-time snapshot); see that module's doc
//! comment for the reciprocal cross-reference.
//!
//! # Verdict-evidence classification (D4)
//! A checked row's evidence string feeds a verdict ONLY when it names
//! a parseable merge/CI outcome:
//! - a PR reference (`/pull/<n>`, `/merge_requests/<n>`, `/pr/<n>`, or
//!   a bare `PR #<n>`/`PR#<n>` marker) with no accompanying "revert"
//!   language -> [`ArtifactEventKind::PrMergeNoRevert`] (table row 7).
//! - a PR reference alongside "revert" language, OR a CI-run reference
//!   (`/actions/runs/<n>`, `/-/jobs/<n>`, `/-/pipelines/<n>`,
//!   `/checks/<n>`) alongside failure/revert language ->
//!   [`ArtifactEventKind::CiFailOrPrRevert`] (table row 6).
//! - anything else (prose-only evidence, no evidence at all, a bare
//!   passing-CI reference with no failure/revert language — the
//!   7-row mapping table has no "CI pass" row to map onto) ->
//!   [`ArtifactEventKind::NonVerdict`]: the row still normalizes to an
//!   `Event` for the join spine, but this adapter never invents a
//!   success signal from an unverifiable checkbox.
//!
//! A `**DEFERRED**`/`**DROPPED**` row always normalizes to an `Event`
//! with `NonVerdict` (deferral/drop is a scheduling fact, never a
//! failure signal for the implementing role) regardless of its own
//! evidence text. A plain, untouched `- [ ]` row (no annotation) is
//! not yet actionable — it produces no event at all, since there is no
//! flip/rewrite to observe from a single point-in-time snapshot.

use std::fs;
use std::path::{Path, PathBuf};

use canon_model::ids::ChangeId;
use chrono::{DateTime, TimeZone, Utc};

use crate::artifact_adapter::{
    ArtifactAdapter, ArtifactEvent, ArtifactEventKind, ArtifactJoinKey, ArtifactParseOutcome, ArtifactSourceConfig, ArtifactSourceHandle,
    resolve_path_source,
};
use crate::scanner::scan_dir;
use crate::task_rows::{self, Annotation, TaskRow, parse_line};

/// The `ArtifactAdapter` for `openspec/changes/*/tasks.md` checkbox
/// state (S4 design D4).
pub struct OpenspecTaskAdapter;

impl ArtifactAdapter for OpenspecTaskAdapter {
    fn adapter_id(&self) -> &'static str {
        "openspec-task"
    }

    fn resolve_source(&self, config: &ArtifactSourceConfig) -> Option<ArtifactSourceHandle> {
        resolve_path_source(&config.openspec_root)
    }

    fn parse(&self, source: &ArtifactSourceHandle) -> ArtifactParseOutcome {
        let root = match source {
            ArtifactSourceHandle::Path(p) => p,
            // This adapter is path-based only (unlike the handoff
            // adapter) — an already-fetched `Records` handle is never
            // constructed for it; treat defensively as "nothing to
            // parse" rather than panicking on a mismatched handle.
            ArtifactSourceHandle::Records(_) => return ArtifactParseOutcome::default(),
        };

        let mut outcome = ArtifactParseOutcome::default();
        for file in discover_task_files(root) {
            parse_tasks_file(&file, &mut outcome);
        }
        outcome
    }
}

/// Find every `tasks.md` this adapter should read from `root`: a
/// single file when `root` already names one directly (the
/// `ArtifactSourceHandle::Path` doc comment's "a single `tasks.md`
/// file" case — convenient for a targeted read/test), otherwise a walk
/// of `<root>/openspec/changes/**/tasks.md` when that substructure
/// exists (the "ordinarily the consumer repo's own root" case),
/// falling back to a walk of `root` itself for any other directory
/// shape (e.g. a fixture root that only contains the changes tree
/// directly). A missing root yields an empty list (`scan_dir`'s own
/// "absent root -> zero records, never an error" contract), never a
/// hardcoded fallback path.
fn discover_task_files(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.to_path_buf()];
    }
    let changes_dir = root.join("openspec").join("changes");
    let scan_root: &Path = if changes_dir.is_dir() { &changes_dir } else { root };
    scan_dir(scan_root, |p| p.file_name().and_then(|n| n.to_str()) == Some("tasks.md"))
}

/// Parse one `tasks.md` file's checkbox rows into `outcome`. The whole
/// file is skipped-and-counted (never a crash) when it cannot be read
/// as UTF-8 text, or when its own change slug (the parent directory's
/// basename) is not a valid `ChangeId` — either failure means no
/// `TaskId` is derivable for anything in the file.
fn parse_tasks_file(path: &Path, outcome: &mut ArtifactParseOutcome) {
    let Ok(text) = fs::read_to_string(path) else {
        outcome.skipped += 1;
        return;
    };
    let Some(change_id) = change_id_for(path) else {
        outcome.skipped += 1;
        return;
    };
    let at = file_modified_at(path);

    for line in text.lines() {
        match parse_line(line) {
            None => {} // not a checkbox row at all — ordinary markdown prose/headers, never counted
            Some(row) => match to_event(&change_id, at, &row, line) {
                RowOutcome::NotApplicable => {}
                RowOutcome::Event(event) => outcome.events.push(event),
                RowOutcome::Malformed => outcome.skipped += 1,
            },
        }
    }
}

/// `openspec/changes/<change-slug>/tasks.md` -> `<change-slug>` as a
/// validated [`ChangeId`] — the change half of every row's `task_id`
/// lives in the containing directory name, never in the row text
/// itself (mirrors `crate::task_rows`'s own `TaskRow` doc comment on `id`).
fn change_id_for(path: &Path) -> Option<ChangeId> {
    let parent = path.parent()?;
    let name = parent.file_name()?.to_str()?;
    ChangeId::parse(name).ok()
}

/// `tasks.md` rows carry no per-row timestamp of their own — the
/// file's own mtime is the best available "when observed" signal
/// (mirrors `adapters::{omp,claude,codex}::file_modified_timestamp_ms`'s
/// identical fallback-to-now convention, this crate's established
/// idiom for a source with no native timestamp field).
fn file_modified_at(path: &Path) -> DateTime<Utc> {
    let ms = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    Utc.timestamp_millis_opt(ms).single().unwrap_or_else(Utc::now)
}

// ── checkbox-row grammar: see `crate::task_rows` (design D5/s35 D2) ──
enum RowOutcome {
    NotApplicable,
    Event(ArtifactEvent),
    Malformed,
}

fn to_event(change_id: &ChangeId, at: DateTime<Utc>, row: &TaskRow, raw_line: &str) -> RowOutcome {
    let state_label = match &row.annotation {
        Some(Annotation::Deferred { .. }) => "deferred",
        Some(Annotation::Dropped) => "dropped",
        None if row.checked => "done",
        None => return RowOutcome::NotApplicable, // untouched `- [ ]`, no flip observed yet
    };

    let Some(task_id) = task_rows::task_id_for(change_id, &row.id) else {
        return RowOutcome::Malformed;
    };

    // Deferred/dropped rows never carry flip evidence, even if the
    // parser found something after the annotation (design D4: a
    // scheduling fact, not a verdict input).
    let evidence = if state_label == "done" { row.evidence.as_deref() } else { None };
    let kind = match evidence {
        Some(ev) => classify_evidence(ev),
        None => ArtifactEventKind::NonVerdict,
    };

    let mut detail = serde_json::json!({
        "task_id": task_id.as_str(),
        "change_id": change_id.as_str(),
        "state": state_label,
        "title": row.title.trim(),
        "row_text": raw_line.trim(),
    });
    if let Some(ev) = evidence {
        detail["evidence"] = serde_json::Value::String(ev.to_string());
    }
    if let Some(Annotation::Deferred { to }) = &row.annotation {
        detail["deferred_to"] = serde_json::Value::String(to.clone());
    }

    RowOutcome::Event(ArtifactEvent {
        adapter_id: "openspec-task",
        join_key: ArtifactJoinKey::Task(task_id),
        kind,
        authoring_role: None,
        area: Some(change_id.as_str().to_string()),
        trust_level: None,
        at,
        detail,
    })
}

// ── merge/CI evidence classification (design D4) ──

fn classify_evidence(evidence: &str) -> ArtifactEventKind {
    let lower = evidence.to_ascii_lowercase();
    let has_pr = contains_pr_reference(&lower);
    let has_ci = contains_ci_reference(&lower);
    let reverted = lower.contains("revert");
    let failed = ["fail", "broken", "errored"].into_iter().any(|k| lower.contains(k));

    // CI-failure/revert guardrail check FIRST, before the generic
    // "PR present -> success" branch below (P2 fix, `ReviewS4Full`):
    // evidence carrying BOTH a PR reference AND a failed-CI/revert
    // signal must win the guardrail row (table row 6) over the
    // PR-merge-success row (table row 7) — a PR-merge note is not
    // proof of success once its own evidence text also says CI
    // failed or the change was reverted. PR evidence is only ever
    // treated as success after failure/revert is ruled out.
    if (has_pr || has_ci) && (reverted || failed) {
        return ArtifactEventKind::CiFailOrPrRevert;
    }
    if has_pr {
        return ArtifactEventKind::PrMergeNoRevert;
    }
    // Prose-only evidence, or a CI reference with no failure/revert
    // language — the 7-row mapping table has no "CI pass" row to map
    // a bare passing-CI link onto, so this never invents one.
    ArtifactEventKind::NonVerdict
}

fn contains_pr_reference(lower: &str) -> bool {
    lower.contains("/pull/") || lower.contains("/merge_requests/") || lower.contains("/pr/") || contains_pr_hash_marker(lower)
}

fn contains_pr_hash_marker(lower: &str) -> bool {
    for marker in ["pr #", "pr#"] {
        if let Some(idx) = lower.find(marker) {
            if lower[idx + marker.len()..].bytes().next().is_some_and(|b| b.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

fn contains_ci_reference(lower: &str) -> bool {
    lower.contains("/actions/runs/") || lower.contains("/-/jobs/") || lower.contains("/-/pipelines/") || lower.contains("/checks/")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::verdict::{attach_regime_key, derive_verdict};
    use canon_model::ids::TaskId;

    fn fixture_repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/openspec_task")
    }

    fn fixture_tasks_file() -> PathBuf {
        fixture_repo_root().join("openspec/changes/frozen-fixture-change/tasks.md")
    }

    // ── checkbox-row grammar ──

    #[test]
    fn parse_line_reads_a_checked_row_with_pr_evidence() {
        let row = parse_line("- [x] 1.1 Implement the checkbox parser — ✅ https://example.com/org/repo/pull/482 merged").unwrap();
        assert_eq!(row.id, "1.1");
        assert!(row.checked);
        assert_eq!(row.annotation, None);
        assert_eq!(row.title, "Implement the checkbox parser");
        assert_eq!(row.evidence.as_deref(), Some("https://example.com/org/repo/pull/482 merged"));
    }

    #[test]
    fn parse_line_reads_a_deferred_row() {
        let row = parse_line("- [ ] 1.3 **DEFERRED to §2.1** Backfill the legacy schema shim (blocked)").unwrap();
        assert_eq!(row.id, "1.3");
        assert!(!row.checked);
        assert_eq!(row.annotation, Some(Annotation::Deferred { to: "2.1".to_string() }));
        assert_eq!(row.title, "Backfill the legacy schema shim (blocked)");
        assert_eq!(row.evidence, None);
    }

    #[test]
    fn parse_line_reads_a_dropped_row() {
        let row = parse_line("- [ ] 1.4 **DROPPED** Patch the old formatter in place").unwrap();
        assert_eq!(row.id, "1.4");
        assert!(!row.checked);
        assert_eq!(row.annotation, Some(Annotation::Dropped));
        assert_eq!(row.title, "Patch the old formatter in place");
    }

    #[test]
    fn parse_line_returns_none_for_non_checkbox_lines() {
        assert!(parse_line("## 1. Frozen fixture section").is_none());
        assert!(parse_line("").is_none());
        assert!(parse_line("some prose about the fixture").is_none());
    }

    #[test]
    fn parse_line_returns_none_for_a_malformed_bracket() {
        assert!(parse_line("- [z] 1.1 not a real checkbox state").is_none());
    }

    // ── merge/CI evidence classification (design D4) ──

    #[test]
    fn merge_evidenced_flip_classifies_as_pr_merge_no_revert() {
        assert_eq!(classify_evidence("https://github.com/example-org/canon/pull/482 merged"), ArtifactEventKind::PrMergeNoRevert);
        assert_eq!(classify_evidence("PR #482 merged cleanly"), ArtifactEventKind::PrMergeNoRevert);
    }

    #[test]
    fn pr_revert_evidence_classifies_as_ci_fail_or_pr_revert() {
        assert_eq!(classify_evidence("https://github.com/example-org/canon/pull/482 reverted after regression"), ArtifactEventKind::CiFailOrPrRevert);
    }

    #[test]
    fn pr_evidence_with_ci_failure_note_classifies_as_ci_fail_or_pr_revert_not_pr_merge() {
        // P2 fix, `ReviewS4Full`: evidence naming BOTH a PR ref AND a
        // CI-failure note (no "revert" word at all) must win the
        // guardrail row over the generic "PR present -> success"
        // branch — CI-failure/revert is checked BEFORE `has_pr` alone.
        assert_eq!(classify_evidence("\u{2705} PR #482; CI failed: https://example.com/ci/build/771"), ArtifactEventKind::CiFailOrPrRevert);
    }

    #[test]
    fn ci_failure_evidence_classifies_as_ci_fail_or_pr_revert() {
        assert_eq!(
            classify_evidence("CI run failed: https://github.com/example-org/canon/actions/runs/999 (2 tests red)"),
            ArtifactEventKind::CiFailOrPrRevert
        );
    }

    #[test]
    fn bare_passing_ci_link_has_no_mapped_row_and_yields_non_verdict() {
        // The 7-row table has no "CI pass" row — a CI link alone,
        // without failure/revert language, is not itself table-mapped
        // evidence.
        assert_eq!(classify_evidence("https://github.com/example-org/canon/actions/runs/1 (all green)"), ArtifactEventKind::NonVerdict);
    }

    #[test]
    fn prose_only_evidence_yields_non_verdict() {
        assert_eq!(classify_evidence("verified manually against the fixture corpus, all green"), ArtifactEventKind::NonVerdict);
    }

    // ── full adapter round trip over the frozen fixture ──

    fn parse_fixture() -> ArtifactParseOutcome {
        let adapter = OpenspecTaskAdapter;
        let config = ArtifactSourceConfig { openspec_root: Some(fixture_repo_root()), ..Default::default() };
        let source = adapter.resolve_source(&config).expect("openspec_root configured");
        adapter.parse(&source)
    }

    #[test]
    fn adapter_id_is_openspec_task() {
        assert_eq!(OpenspecTaskAdapter.adapter_id(), "openspec-task");
    }

    #[test]
    fn resolve_source_is_none_when_unconfigured() {
        let adapter = OpenspecTaskAdapter;
        assert!(adapter.resolve_source(&ArtifactSourceConfig::default()).is_none());
    }

    #[test]
    fn resolve_source_ignores_a_records_handle() {
        let adapter = OpenspecTaskAdapter;
        let outcome = adapter.parse(&ArtifactSourceHandle::Records(Vec::new()));
        assert_eq!(outcome, ArtifactParseOutcome::default());
    }

    #[test]
    fn fixture_produces_exactly_four_events_and_skips_nothing() {
        let outcome = parse_fixture();
        // 1.1 (done+pr), 1.2 (done+prose), 1.3 (deferred), 1.4 (dropped).
        // 1.5 (untouched open row) produces no event.
        assert_eq!(outcome.events.len(), 4, "events: {:#?}", outcome.events);
        assert_eq!(outcome.skipped, 0);
    }

    #[test]
    fn merge_evidenced_flip_normalizes_to_an_event_that_feeds_a_verdict() {
        let outcome = parse_fixture();
        let change_id = ChangeId::parse("frozen-fixture-change").unwrap();
        let task_id = TaskId::parse("frozen-fixture-change#1.1").unwrap();
        let event = outcome.events.iter().find(|e| e.join_key == ArtifactJoinKey::Task(task_id.clone())).expect("task 1.1 event present");

        assert_eq!(event.adapter_id, "openspec-task");
        assert_eq!(event.kind, ArtifactEventKind::PrMergeNoRevert);
        assert_eq!(event.area.as_deref(), Some(change_id.as_str()));

        let row = derive_verdict(event.kind, event.authoring_role.as_ref()).expect("PR-merge evidence feeds a verdict");
        assert_eq!(row.role.as_str(), "dev");
        assert_eq!(row.polarity, crate::verdict::Polarity::Success);
        assert_eq!(row.becomes, crate::verdict::Becomes::StrategyCandidate);

        let verdict = attach_regime_key(row, event.join_key.clone(), "canon", event.area.as_deref().unwrap_or("unknown"), "abcdef", event.trust_level.clone()).unwrap();
        assert!(verdict.regime_key.as_str().starts_with("dev/canon/frozen-fixture-change/"));
    }

    #[test]
    fn prose_only_flip_normalizes_to_an_event_with_no_verdict() {
        let outcome = parse_fixture();
        let task_id = TaskId::parse("frozen-fixture-change#1.2").unwrap();
        let event = outcome.events.iter().find(|e| e.join_key == ArtifactJoinKey::Task(task_id.clone())).expect("task 1.2 event present");

        assert_eq!(event.kind, ArtifactEventKind::NonVerdict);
        assert!(derive_verdict(event.kind, event.authoring_role.as_ref()).is_none());
        assert_eq!(event.detail["evidence"], serde_json::json!("verified manually against the fixture corpus, all green"));
    }

    #[test]
    fn deferred_and_dropped_rows_normalize_to_events_with_no_verdict() {
        let outcome = parse_fixture();

        let deferred_id = TaskId::parse("frozen-fixture-change#1.3").unwrap();
        let deferred = outcome.events.iter().find(|e| e.join_key == ArtifactJoinKey::Task(deferred_id.clone())).expect("task 1.3 (deferred) event present");
        assert_eq!(deferred.kind, ArtifactEventKind::NonVerdict);
        assert!(derive_verdict(deferred.kind, deferred.authoring_role.as_ref()).is_none());
        assert_eq!(deferred.detail["state"], serde_json::json!("deferred"));
        assert_eq!(deferred.detail["deferred_to"], serde_json::json!("2.1"));

        let dropped_id = TaskId::parse("frozen-fixture-change#1.4").unwrap();
        let dropped = outcome.events.iter().find(|e| e.join_key == ArtifactJoinKey::Task(dropped_id.clone())).expect("task 1.4 (dropped) event present");
        assert_eq!(dropped.kind, ArtifactEventKind::NonVerdict);
        assert!(derive_verdict(dropped.kind, dropped.authoring_role.as_ref()).is_none());
        assert_eq!(dropped.detail["state"], serde_json::json!("dropped"));
    }

    #[test]
    fn untouched_open_row_produces_no_event() {
        let outcome = parse_fixture();
        let untouched_id = TaskId::parse("frozen-fixture-change#1.5").unwrap();
        assert!(!outcome.events.iter().any(|e| e.join_key == ArtifactJoinKey::Task(untouched_id.clone())));
    }

    #[test]
    fn re_ingesting_the_unchanged_fixture_is_idempotent() {
        let first = parse_fixture();
        let second = parse_fixture();
        assert_eq!(first, second, "re-parsing an unchanged tasks.md fixture must yield identical events");
    }

    #[test]
    fn pointing_directly_at_a_single_tasks_md_file_also_works() {
        let adapter = OpenspecTaskAdapter;
        let config = ArtifactSourceConfig { openspec_root: Some(fixture_tasks_file()), ..Default::default() };
        let source = adapter.resolve_source(&config).unwrap();
        let outcome = adapter.parse(&source);
        assert_eq!(outcome.events.len(), 4);
        assert_eq!(outcome.skipped, 0);
    }

    #[test]
    fn a_checkbox_row_with_an_unparseable_task_number_is_skipped_and_counted() {
        let dir = tempfile::tempdir().unwrap();
        let change_dir = dir.path().join("openspec/changes/some-change");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(change_dir.join("tasks.md"), "- [x] not-a-number Broken id token — ✅ https://example.com/o/r/pull/1 merged\n").unwrap();

        let adapter = OpenspecTaskAdapter;
        let config = ArtifactSourceConfig { openspec_root: Some(dir.path().to_path_buf()), ..Default::default() };
        let source = adapter.resolve_source(&config).unwrap();
        let outcome = adapter.parse(&source);
        assert_eq!(outcome.events.len(), 0);
        assert_eq!(outcome.skipped, 1);
    }

    #[test]
    fn a_tasks_md_under_an_invalid_change_slug_directory_is_skipped_whole_file() {
        let dir = tempfile::tempdir().unwrap();
        // Underscore is not a valid kebab-case `ChangeId` segment.
        let change_dir = dir.path().join("openspec/changes/not_a_valid_slug");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(change_dir.join("tasks.md"), "- [x] 1.1 Anything — ✅ https://example.com/o/r/pull/1 merged\n").unwrap();

        let adapter = OpenspecTaskAdapter;
        let config = ArtifactSourceConfig { openspec_root: Some(dir.path().to_path_buf()), ..Default::default() };
        let source = adapter.resolve_source(&config).unwrap();
        let outcome = adapter.parse(&source);
        assert_eq!(outcome.events.len(), 0);
        assert_eq!(outcome.skipped, 1);
    }

    #[test]
    fn unconfigured_source_via_registry_helper_is_a_no_op() {
        // Mirrors `artifact_registry::resolve_and_parse`'s "unconfigured
        // -> ArtifactParseOutcome::empty()" contract without depending
        // on the shared registry file's current entry set.
        let adapter: &dyn ArtifactAdapter = &OpenspecTaskAdapter;
        assert!(adapter.resolve_source(&ArtifactSourceConfig::default()).is_none());
    }
}
