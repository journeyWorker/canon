//! The `PlanAdapter` for openspec change directories
//! (`openspec/changes/**`, s17's reference dialect, tasks 2.1-2.7).
//!
//! # Discovery (design "Dialect -> RecordKind mapping" table, task 2.1)
//! Mirrors `crate::artifact_adapters::openspec_task::discover_task_files`'s
//! root-shape tolerance: a repo root containing `openspec/changes/`, a
//! changes dir passed directly, or a fixture tree that only holds the
//! changes substructure. Unlike that adapter (which walks for
//! `tasks.md` files at any depth, since a proposal-only change dir has
//! none to find), this adapter discovers CHANGE DIRECTORIES themselves
//! — the immediate children of the resolved changes root, plus the
//! immediate children of its `archive/` subdirectory (flagged
//! archived) — so a proposal-only dir is discovered even with no
//! `tasks.md` inside it.
//!
//! # Two readers, one join (design R5)
//! `crate::artifact_adapters::openspec_task` (S4) reads the SAME
//! `openspec/changes/**` tree for a DIFFERENT job: verdict EVENTS keyed
//! by `task_id`, not plan state. Both derive `task_id` through the one
//! shared `crate::task_rows::task_id_for` function (design D5), so
//! the two outputs join on that key without ever double-counting each
//! other — see that module's doc comment for the reciprocal
//! cross-reference.
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{ChangeId, TaskId};
use canon_model::records::{Change, ChangeStatus, Task, TaskStatus};
use chrono::{DateTime, TimeZone, Utc};

use crate::task_rows::{self, format_line, parse_line};
use crate::plan_writeback::{FlipDocOutcome, PlanTaskLocation, PlanWriteBack, WriteBackError};
use crate::plan_adapter::{PlanAdapter, PlanParseOutcome, PlanSourceConfig, PlanSourceHandle, resolve_path_source};
use crate::scanner::scan_dir;

/// canon-model's envelope schema version every record this adapter
/// constructs carries (mirrors `crate::normalize::SCHEMA_VERSION`'s own
/// doc comment: "per-kind schema version, bumped on any breaking field
/// change to that kind").
const SCHEMA_VERSION: u32 = 1;

/// The fixed, per-dialect unattributed actor every `Change`/`Task` this
/// adapter emits carries (design D7: "provenance visible in every
/// record, byte-stable across runs" — never a wall-clock- or
/// run-derived value).
const ACTOR_AGENT_ID: &str = "canon-plan-import-openspec";

/// Named diagnostic (design D3-style `unmapped` bookkeeping) for a
/// change dir whose proposal.md has no `## Why` heading — the summary
/// is empty rather than invented, but the drop is still visible.
/// `pub(crate)` (not merely private) so `crate::plan_selftest`'s
/// fixture-corpus oracle (P4 task 4.1) can assert against the SAME
/// stable diagnostic-name constants this adapter emits, rather than
/// re-typing the string literals a second place they could drift from.
pub(crate) const DIAG_PROPOSAL_MISSING_WHY: &str = "proposal-missing-why";
/// Named diagnostic for a `specs/**/spec.md` `#### Scenario:` block —
/// no core `Scenario` record exists to map onto (design D3).
pub(crate) const DIAG_SPEC_DELTA_SCENARIO: &str = "spec-delta-scenario";
/// Named diagnostic for a `design.md` — design prose is never mapped.
pub(crate) const DIAG_DESIGN_DOC: &str = "design-doc";
/// Named diagnostic (design s20 Decision 2/tasks.md task 2.2) for one
/// `[covers: …]` token that failed `ScenarioId::parse` — dropped from
/// the imported `Task.scenario_refs`, counted once per malformed
/// token, never sinking the row's other well-formed refs or the row's
/// own `Task` import. Recorded under the composite key
/// `"<DIAG_MALFORMED_SCENARIO_REF>:<task_id>"` (never the bare
/// constant alone) — s20's `task-scenario-join` spec requires the
/// diagnostic be "scoped to that row's task_id", and `PlanParseOutcome
/// .unmapped` is keyed by construct NAME (design D3), so the task_id
/// rides in the name itself rather than a second bookkeeping field.
pub(crate) const DIAG_MALFORMED_SCENARIO_REF: &str = "malformed-scenario-ref";

pub struct OpenspecPlanAdapter;

impl PlanAdapter for OpenspecPlanAdapter {
    fn dialect_id(&self) -> &'static str {
        "openspec"
    }

    fn resolve_source(&self, config: &PlanSourceConfig) -> Option<PlanSourceHandle> {
        resolve_path_source(&config.root)
    }

    fn parse(&self, source: &PlanSourceHandle) -> PlanParseOutcome {
        let PlanSourceHandle::Path(root) = source;
        let mut outcome = PlanParseOutcome::empty();
        for dir in discover_change_dirs(root) {
            parse_change_dir(&dir, root, &mut outcome);
        }
        outcome
    }
}

/// s35 `gate-plan-dialect-seam` (design D1): the openspec dialect owns
/// its on-disk layout for the evidence-gated flip too. `<root>/openspec/
/// changes/<change_id>/tasks.md` carries a task's rows; its
/// `tasks.vocab.yaml` sibling carries the S10 typed atoms — the SAME
/// paths `canon gate task` hardcoded before s35, now owned here. The row
/// GRAMMAR itself is dialect-neutral (`crate::task_rows`); this impl
/// owns only WHERE the rows live and delegates the mutation to
/// [`format_line`].
impl PlanWriteBack for OpenspecPlanAdapter {
    fn locate_task(&self, root: &Path, task_id: &TaskId) -> Option<PlanTaskLocation> {
        let path = tasks_md_path(root, &task_id.change_id());
        // FILE existence only (module doc): whether the specific `<n>`
        // row lives IN this tasks.md is `flip_task`'s concern, so a
        // located-but-rowless flip stays a gate-red "no matching row",
        // never a "no source located it" usage error.
        path.is_file().then_some(PlanTaskLocation { document_path: path })
    }

    fn flip_task(&self, document: &str, task_id: &TaskId, evidence_note: &str) -> Result<FlipDocOutcome, WriteBackError> {
        let want_id = row_number(task_id);
        let mut lines: Vec<String> = document.split('\n').map(str::to_string).collect();
        let row_idx = lines.iter().position(|line| parse_line(line).is_some_and(|row| row.id == want_id));
        let Some(row_idx) = row_idx else {
            return Err(WriteBackError::RowNotFound(task_id.clone()));
        };
        let row = parse_line(&lines[row_idx]).expect("row_idx was located via a successful parse_line above");
        if row.checked {
            // Already `[x]` — idempotent no-op, document byte-identical.
            return Ok(FlipDocOutcome { document: document.to_string(), flipped: false });
        }
        let mut flipped = row;
        flipped.checked = true;
        flipped.evidence = Some(evidence_note.to_string());
        lines[row_idx] = format_line(&flipped);
        Ok(FlipDocOutcome { document: lines.join("\n"), flipped: true })
    }

    fn typed_atoms_path(&self, root: &Path, change_id: &ChangeId) -> Option<PathBuf> {
        Some(change_dir(root, change_id).join("tasks.vocab.yaml"))
    }
}

/// `<root>/openspec/changes/<change_id>` — the openspec dialect's
/// change-dir layout (s35 D1), the SAME path `canon ingest plans` reads
/// and `canon gate task` hardcoded before s35.
fn change_dir(root: &Path, change_id: &ChangeId) -> PathBuf {
    root.join("openspec").join("changes").join(change_id.as_str())
}

fn tasks_md_path(root: &Path, change_id: &ChangeId) -> PathBuf {
    change_dir(root, change_id).join("tasks.md")
}

/// The `<n>` half of a `<change_id>#<n>` join-spine [`TaskId`] — the row
/// id token a `tasks.md` row carries (mirrors the pre-s35 `gate_task`'s
/// own `rsplit_once('#')` extraction).
fn row_number(task_id: &TaskId) -> &str {
    task_id.as_str().rsplit_once('#').map(|(_, n)| n).unwrap_or_else(|| task_id.as_str())
}

fn actor() -> Actor {
    Actor::new_unattributed(ACTOR_AGENT_ID)
}

/// Find every change directory this adapter should read from `root`:
/// the immediate children of `<root>/openspec/changes/` when that
/// substructure exists (the ordinary consumer-repo shape), otherwise
/// the immediate children of `root` itself (a changes dir passed
/// directly, or a fixture tree that only holds the changes
/// substructure — mirrors `discover_task_files`'s identical fallback).
/// Immediate children of an `archive/` subdirectory are included too
/// (each one a legitimate archived change dir, task 2.1) — `archive`
/// itself is never treated as a change dir. Deterministic (byte-lexical
/// path order, `list_subdirs`'s own sort) so two passes over the same
/// tree enumerate identically.
fn discover_change_dirs(root: &Path) -> Vec<PathBuf> {
    let changes_dir = root.join("openspec").join("changes");
    let scan_root: PathBuf = if changes_dir.is_dir() { changes_dir } else { root.to_path_buf() };

    let mut dirs: Vec<PathBuf> =
        list_subdirs(&scan_root).into_iter().filter(|p| p.file_name() != Some(OsStr::new("archive"))).collect();
    dirs.extend(list_subdirs(&scan_root.join("archive")));
    dirs.sort_unstable();
    dirs
}

/// Immediate child directories of `dir`, byte-lexically sorted. A
/// missing/unreadable `dir` yields an empty `Vec` (mirrors
/// `crate::scanner::scan_dir`'s "absent root -> zero records, never an
/// error" contract), never a hardcoded fallback path.
fn list_subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries.filter_map(|entry| entry.ok()).map(|entry| entry.path()).filter(|p| p.is_dir()).collect();
    dirs.sort_unstable();
    dirs
}

/// A change dir directly under an `archive/` parent is archived
/// (design D4: identity is the basename VERBATIM, no date-prefix
/// stripping — this only asks "which parent held it", never touches
/// the name itself).
fn is_archived(dir: &Path) -> bool {
    dir.parent().and_then(|p| p.file_name()) == Some(OsStr::new("archive"))
}

/// Render `path` relative to `root` (s18 `loud-plan-import-
/// diagnostics` spec's "the construct's relative path" contract —
/// `MalformedEntry.path`'s own doc comment: "never an absolute path
/// leaking the host filesystem layout"). Every path this adapter
/// derives is actually rooted under `root` (it was built by joining
/// onto `root` or a descendant of it), so `strip_prefix` always
/// succeeds in practice; falling back to `path` verbatim on the
/// (should-be-unreachable) failure case is a defensive never-panic,
/// never the ordinary path.
fn relative_to_root(path: &Path, root: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).display().to_string()
}

/// Parse one change directory into `outcome`: at most one `Change`
/// candidate plus zero or more `Task` candidates, or a `malformed`
/// increment when the directory itself is structurally broken (task
/// 2.2). Never a crash — every failure mode here is skip-and-count.
fn parse_change_dir(dir: &Path, root: &Path, outcome: &mut PlanParseOutcome) {
    let Some(basename) = dir.file_name().and_then(|n| n.to_str()) else {
        outcome.record_malformed(relative_to_root(dir, root), "unreadable-directory");
        return;
    };
    let Ok(change_id) = ChangeId::parse(basename) else {
        // A basename failing the grammar skips the whole dir, counted
        // — siblings are unaffected (design D4).
        outcome.record_malformed(relative_to_root(dir, root), "invalid-change-id-grammar");
        return;
    };
    let proposal_path = dir.join("proposal.md");
    let Ok(proposal_text) = fs::read_to_string(&proposal_path) else {
        // Missing/unreadable proposal.md: not a valid openspec change.
        // A `changes`-named directory that fails here is the EXACT
        // root-one-level-too-high near-miss signature (s18
        // `loud-plan-import-diagnostics` spec's "A malformed
        // changes-directory near-miss carries an actionable root
        // hint") — the configured `root:` most likely names THIS
        // directory's own parent, rather than (or above)
        // `openspec/changes` itself.
        let dir_rel = relative_to_root(dir, root);
        if basename == "changes" {
            let parent_rel = dir.parent().map(|p| relative_to_root(p, root)).unwrap_or_else(|| dir_rel.clone());
            let hint = format!(
                "the configured `root:` may be pointing at `{}`, this directory's parent, rather than at (or above) `openspec/changes` itself — point it at `{}` (or a directory whose own `openspec/changes` resolves there) instead",
                parent_rel,
                dir_rel,
            );
            outcome.record_malformed_with_hint(dir_rel, "missing-proposal-md", hint);
        } else {
            outcome.record_malformed(dir_rel, "missing-proposal-md");
        }
        return;
    };
    let proposal_mtime = file_modified_at(&proposal_path);

    let (summary, why_found) = why_summary(&proposal_text);
    if !why_found {
        outcome.record_unmapped(DIAG_PROPOSAL_MISSING_WHY);
    }

    let archived = is_archived(dir);
    let (tasks, done_count, open_count, change_at) = parse_tasks_file(dir, root, &change_id, proposal_mtime, outcome);

    let status = derive_status(archived, done_count, open_count);
    let envelope = Envelope::new(SCHEMA_VERSION, RecordKind::Change, change_at, actor());
    outcome.changes.push(Change::new(envelope, change_id, basename, summary, status));
    outcome.tasks.extend(tasks);

    count_drop_diagnostics(dir, outcome);
}

/// Parse `<dir>/tasks.md` (when present) into `Task` candidates plus
/// the done/open checkbox tallies D6's status derivation needs.
/// `tasks.md` absent is a legitimate proposal-stage change (task
/// 2.2/2.3): zero tasks, zero tallies, `Change.at` stays the
/// proposal.md mtime alone. A present-but-unreadable `tasks.md` is
/// counted malformed (whole file skipped, never a crash) but the
/// `Change` itself still imports with zero tasks — the directory as a
/// whole is not thereby invalid, only its task list is unavailable.
fn parse_tasks_file(
    dir: &Path,
    root: &Path,
    change_id: &ChangeId,
    proposal_mtime: DateTime<Utc>,
    outcome: &mut PlanParseOutcome,
) -> (Vec<Task>, usize, usize, DateTime<Utc>) {
    let tasks_path = dir.join("tasks.md");
    if !tasks_path.is_file() {
        return (Vec::new(), 0, 0, proposal_mtime);
    }
    let Ok(text) = fs::read_to_string(&tasks_path) else {
        outcome.record_malformed(relative_to_root(&tasks_path, root), "unreadable-tasks-md");
        return (Vec::new(), 0, 0, proposal_mtime);
    };
    let tasks_mtime = file_modified_at(&tasks_path);
    let change_at = proposal_mtime.max(tasks_mtime);

    let mut tasks = Vec::new();
    let mut done_count = 0usize;
    let mut open_count = 0usize;

    for line in text.lines() {
        let Some(row) = parse_line(line) else {
            continue; // not a checkbox row at all — ordinary prose/headers, never counted
        };
        // Status tallying is a pure function of every row `parse_line`
        // recognizes as a checkbox row (design D6's "parseable
        // checkbox rows") — independent of whether the row's `<n>`
        // token itself is a valid task number, since `parse_line`
        // deliberately does not check that grammar (its own doc
        // comment: "checked by a consumer deriving task_id"). A bad
        // `<n>` skips the row's TASK emission below, never its
        // contribution to the tally.
        if row.checked {
            done_count += 1;
        } else {
            open_count += 1;
        }

        let Some(task_id) = task_rows::task_id_for(change_id, &row.id) else {
            // A row whose `<n>` fails the task-number grammar is
            // skipped and counted (task 2.4) — never emitted as a Task.
            outcome.record_malformed(format!("{}#{}", relative_to_root(&tasks_path, root), row.id), "invalid-task-number-grammar");
            continue;
        };
        let status = if row.checked { TaskStatus::Done } else { TaskStatus::Open };
        let evidence_note = row.evidence.map(|ev| ev.trim().to_string()).or_else(|| row.annotation.as_ref().map(|a| a.marker_text()));
        let title = row.title.trim().to_string();
        for _ in &row.malformed_scenario_refs {
            // One malformed `[covers: …]` token, scoped to THIS row's
            // own `task_id` (s20 Decision 2 + Wave-1 review finding: a
            // flat count cannot tell an operator WHICH row to fix) —
            // the row's other well-formed refs, and the row's own
            // Task import, both still succeed.
            outcome.record_unmapped(&format!("{DIAG_MALFORMED_SCENARIO_REF}:{}", task_id.as_str()));
        }
        let envelope = Envelope::new(SCHEMA_VERSION, RecordKind::Task, tasks_mtime, actor());
        tasks.push(Task::new(envelope, task_id, title, status, evidence_note).with_scenario_refs(row.scenario_refs));
    }

    (tasks, done_count, open_count, change_at)
}

/// `ChangeStatus` as a pure function of the snapshot (design D6): an
/// archive location wins unconditionally; otherwise the checkbox
/// tallies alone decide. `pub(crate)` (s30 design D4) so
/// `crate::plan_adapters::superpowers` reuses this SAME tally function
/// (`archived: false` always, since that dialect has no archive
/// convention) rather than re-deriving byte-identical semantics a
/// second place they could drift from.
pub(crate) fn derive_status(archived: bool, done_count: usize, open_count: usize) -> ChangeStatus {
    if archived {
        return ChangeStatus::Archived;
    }
    match (done_count, open_count) {
        (0, 0) => ChangeStatus::Proposed,
        (d, 0) if d > 0 => ChangeStatus::Completed,
        (0, _) => ChangeStatus::Proposed,
        (_, _) => ChangeStatus::InProgress,
    }
}

/// The first paragraph under proposal.md's `## Why` heading,
/// whitespace-normalized, plus whether the heading was found at all
/// (task 2.2: an absent heading yields an empty summary AND a
/// diagnostic — never invented prose). The paragraph ends at the first
/// blank line or the next heading, whichever comes first; leading
/// blank lines between the heading and its first content line are
/// skipped.
fn why_summary(proposal_text: &str) -> (String, bool) {
    let mut lines = proposal_text.lines();
    let found = lines.by_ref().any(|line| line.trim() == "## Why");
    if !found {
        return (String::new(), false);
    }

    let mut paragraph_started = false;
    let mut raw = String::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if paragraph_started {
                break;
            }
            continue; // still skipping leading blank lines
        }
        if trimmed.starts_with('#') {
            break; // hit the next heading before any paragraph content
        }
        if paragraph_started {
            raw.push(' ');
        }
        raw.push_str(trimmed);
        paragraph_started = true;
    }
    (raw.split_whitespace().collect::<Vec<_>>().join(" "), true)
}

/// Increment `outcome`'s named drop counts for `dir`'s
/// `specs/**/spec.md` `#### Scenario:` blocks and `design.md` (design
/// D3, task 2.5) — never a `Scenario` record, never a mapping guess.
fn count_drop_diagnostics(dir: &Path, outcome: &mut PlanParseOutcome) {
    if dir.join("design.md").is_file() {
        outcome.record_unmapped(DIAG_DESIGN_DOC);
    }

    let specs_dir = dir.join("specs");
    if !specs_dir.is_dir() {
        return;
    }
    for spec_file in scan_dir(&specs_dir, |p| p.file_name().and_then(|n| n.to_str()) == Some("spec.md")) {
        let Ok(text) = fs::read_to_string(&spec_file) else {
            continue; // an unreadable spec-delta file drops nothing to diagnose, never a crash
        };
        for _ in text.lines().filter(|line| line.trim_start().starts_with("#### Scenario:")) {
            outcome.record_unmapped(DIAG_SPEC_DELTA_SCENARIO);
        }
    }
}

/// `tasks.md`/proposal.md carry no per-record timestamp of their own —
/// the file's own mtime is the best available "when observed" signal
/// (design D7; mirrors
/// `artifact_adapters::openspec_task::file_modified_at`'s identical
/// fallback-to-now convention, this crate's established per-adapter
/// idiom for a source with no native timestamp field — the fallback
/// only fires when the mtime itself is unreadable, never as the
/// ordinary-path source of `at`).
fn file_modified_at(path: &Path) -> DateTime<Utc> {
    let ms = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    Utc.timestamp_millis_opt(ms).single().unwrap_or_else(Utc::now)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use canon_model::ids::{ScenarioId, TaskId};

    use super::*;
    use crate::artifact_adapter::{ArtifactAdapter, ArtifactJoinKey, ArtifactSourceConfig};
    use crate::artifact_adapters::openspec_task::OpenspecTaskAdapter;

    // ── P1 seam tests, still true under the real P2 implementation ──

    #[test]
    fn dialect_id_is_openspec() {
        assert_eq!(OpenspecPlanAdapter.dialect_id(), "openspec");
    }

    #[test]
    fn resolve_source_is_none_when_unconfigured() {
        assert!(OpenspecPlanAdapter.resolve_source(&PlanSourceConfig::default()).is_none());
    }

    #[test]
    fn resolve_source_wraps_a_configured_root() {
        let config = PlanSourceConfig { root: Some(PathBuf::from("/tmp/some-openspec-root")) };
        let source = OpenspecPlanAdapter.resolve_source(&config);
        assert_eq!(source, Some(PlanSourceHandle::Path(PathBuf::from("/tmp/some-openspec-root"))));
    }

    #[test]
    fn parse_over_a_nonexistent_root_is_an_honest_empty_outcome() {
        let source = PlanSourceHandle::Path(PathBuf::from("/tmp/definitely-does-not-exist-s17-p2"));
        assert_eq!(OpenspecPlanAdapter.parse(&source), PlanParseOutcome::empty());
    }

    // ── fixture tree (task 2.7) ──

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plan_openspec")
    }

    fn parse_fixture() -> PlanParseOutcome {
        let adapter = OpenspecPlanAdapter;
        let config = PlanSourceConfig { root: Some(fixture_root()) };
        let source = adapter.resolve_source(&config).expect("fixture root configured");
        adapter.parse(&source)
    }

    fn find_change<'a>(outcome: &'a PlanParseOutcome, change_id: &str) -> &'a Change {
        outcome.changes.iter().find(|c| c.change_id.as_str() == change_id).unwrap_or_else(|| panic!("{change_id} not found"))
    }

    fn find_task<'a>(outcome: &'a PlanParseOutcome, task_id: &str) -> &'a Task {
        outcome.tasks.iter().find(|t| t.task_id.as_str() == task_id).unwrap_or_else(|| panic!("{task_id} not found"))
    }

    #[test]
    fn a_live_change_dir_imports_change_and_tasks_with_the_mapping_table_shapes() {
        let outcome = parse_fixture();

        let change = find_change(&outcome, "add-widget");
        assert_eq!(change.title, "add-widget");
        assert_eq!(change.summary, "Adds a widget capability to the demo surface, closing a gap flagged during the s17 fixture design review.");
        assert_eq!(change.status, ChangeStatus::InProgress, "2 done + 3 open (bad-id row excluded from Task emission but still open) -> mixed");
        assert_eq!(change.envelope.actor, Actor::new_unattributed("canon-plan-import-openspec"));

        let done_with_evidence = find_task(&outcome, "add-widget#1.1");
        assert_eq!(done_with_evidence.status, TaskStatus::Done);
        assert_eq!(done_with_evidence.title, "wire the driver");
        assert_eq!(done_with_evidence.evidence_note.as_deref(), Some("crates/canon-cli tests green"));

        let done_without_evidence = find_task(&outcome, "add-widget#1.2");
        assert_eq!(done_without_evidence.status, TaskStatus::Done);
        assert_eq!(done_without_evidence.evidence_note, None);

        let deferred = find_task(&outcome, "add-widget#1.3");
        assert_eq!(deferred.status, TaskStatus::Open, "checkbox state wins, never invented from the annotation");
        assert_eq!(deferred.evidence_note.as_deref(), Some("**DEFERRED to §2.1**"));

        let dropped = find_task(&outcome, "add-widget#1.4");
        assert_eq!(dropped.status, TaskStatus::Open);
        assert_eq!(dropped.evidence_note.as_deref(), Some("**DROPPED**"));

        let untouched = find_task(&outcome, "add-widget#1.5");
        assert_eq!(untouched.status, TaskStatus::Open);
        assert_eq!(untouched.evidence_note, None);

        assert!(outcome.tasks.iter().all(|t| t.task_id.as_str() != "add-widget#1.a"), "the bad-id row never emits a Task");
    }

    #[test]
    fn an_archived_change_dir_imports_as_archived_regardless_of_tallies() {
        let outcome = parse_fixture();
        let change = find_change(&outcome, "archived-change");
        assert_eq!(change.status, ChangeStatus::Archived, "archive location wins unconditionally, even with every row open");
        let task = find_task(&outcome, "archived-change#1.1");
        assert_eq!(task.status, TaskStatus::Open, "Task status is still the checkbox state verbatim, unaffected by archive location");
    }

    #[test]
    fn an_all_done_change_dir_imports_as_completed() {
        let outcome = parse_fixture();
        assert_eq!(find_change(&outcome, "all-done-change").status, ChangeStatus::Completed);
    }

    #[test]
    fn a_proposal_only_change_dir_imports_as_proposed_with_zero_tasks_and_zero_diagnostics() {
        let outcome = parse_fixture();
        let change = find_change(&outcome, "proposal-only-change");
        assert_eq!(change.status, ChangeStatus::Proposed);
        assert!(outcome.tasks.iter().all(|t| !t.task_id.as_str().starts_with("proposal-only-change#")));
    }

    #[test]
    fn a_missing_why_heading_yields_empty_summary_and_a_named_diagnostic() {
        let outcome = parse_fixture();
        let change = find_change(&outcome, "missing-why-change");
        assert_eq!(change.summary, "", "never invented prose when the heading is absent");
        assert_eq!(change.status, ChangeStatus::Proposed, "none of its rows are done");
        assert_eq!(outcome.unmapped.get(DIAG_PROPOSAL_MISSING_WHY), Some(&1));
    }

    #[test]
    fn a_bad_basename_skips_the_whole_dir_counted_malformed_siblings_unaffected() {
        let outcome = parse_fixture();
        assert!(outcome.changes.iter().all(|c| c.change_id.as_str() != "Bad_Slug!"));
        // Every well-formed sibling still imports.
        assert!(outcome.changes.iter().any(|c| c.change_id.as_str() == "add-widget"));
        assert!(outcome.changes.iter().any(|c| c.change_id.as_str() == "all-done-change"));
    }

    #[test]
    fn a_missing_proposal_md_skips_the_whole_dir_even_with_a_readable_tasks_md() {
        let outcome = parse_fixture();
        assert!(outcome.changes.iter().all(|c| c.change_id.as_str() != "missing-proposal-change"));
        assert!(outcome.tasks.iter().all(|t| !t.task_id.as_str().starts_with("missing-proposal-change#")), "no proposal.md means no Change, so its tasks.md rows never surface either");
    }

    #[test]
    fn malformed_dirs_and_bad_task_number_rows_are_all_counted() {
        let outcome = parse_fixture();
        // Bad_Slug! (bad basename) + missing-proposal-change (no
        // proposal.md) + add-widget's one bad-`<n>` row.
        assert_eq!(outcome.malformed.len(), 3);
    }

    #[test]
    fn a_missing_proposal_md_is_named_by_path_and_reason() {
        let outcome = parse_fixture();
        let entry = outcome.malformed.iter().find(|e| e.path.ends_with("missing-proposal-change")).expect("missing-proposal-change must be a named malformed entry");
        assert_eq!(entry.reason, "missing-proposal-md");
        assert_eq!(entry.hint, None, "an ordinary (non-`changes`-basename) near-miss carries no hint");
        assert!(
            !entry.path.starts_with(fixture_root().to_str().unwrap()),
            "the path must be RELATIVE to the source root, never the absolute fixture path: {}",
            entry.path
        );
    }

    #[test]
    fn an_invalid_change_id_basename_is_named_by_path_and_reason() {
        let outcome = parse_fixture();
        let entry = outcome.malformed.iter().find(|e| e.path.ends_with("Bad_Slug!")).expect("Bad_Slug! must be a named malformed entry");
        assert_eq!(entry.reason, "invalid-change-id-grammar");
    }

    #[test]
    fn a_malformed_change_dir_still_does_not_sink_sibling_imports() {
        let outcome = parse_fixture();
        // The malformed entries above sit ALONGSIDE, never instead of,
        // every well-formed sibling's successful import (s18 spec: "A
        // malformed change dir still does not sink sibling imports").
        assert!(outcome.changes.iter().any(|c| c.change_id.as_str() == "add-widget"));
        assert!(outcome.changes.iter().any(|c| c.change_id.as_str() == "all-done-change"));
        assert!(!outcome.malformed.is_empty());
    }

    #[test]
    fn root_pointed_one_level_above_openspec_changes_surfaces_the_hint() {
        // s18 `loud-plan-import-diagnostics` spec: "root: pointed one
        // level above openspec/changes surfaces the hint" -- `root`
        // resolves to `tmp` (no `tmp/openspec/changes`), and `tmp`
        // directly contains a `changes` subdirectory with no
        // `proposal.md` of its own -- the EXACT near-miss signature the
        // SYNTHESIS reproduces.
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("changes")).unwrap();

        let outcome = OpenspecPlanAdapter.parse(&PlanSourceHandle::Path(tmp.path().to_path_buf()));
        assert_eq!(outcome.malformed.len(), 1);
        let entry = &outcome.malformed[0];
        assert_eq!(entry.reason, "missing-proposal-md");
        assert!(entry.path.ends_with("changes"), "path: {}", entry.path);
        assert_eq!(entry.path, "changes", "must be relative to `root`, never the tmpdir's absolute path");
        let hint = entry.hint.as_deref().expect("the `changes`-basename near-miss must carry the root hint");
        assert!(hint.contains("root:"), "hint must reference the `root:` config key: {hint}");
        assert!(
            !hint.contains(tmp.path().to_str().unwrap()),
            "the hint text must never leak the tmpdir's absolute host path: {hint}"
        );
    }

    #[test]
    fn a_changes_named_directory_that_legitimately_is_a_change_is_never_hinted() {
        // s18 spec: "A changes-named directory that legitimately IS a
        // change is never hinted" -- a readable proposal.md makes this
        // an ordinary well-formed import, no malformed entry, no hint,
        // no special-casing of the basename.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("changes");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("proposal.md"), "## Why\nA legitimately named change dir.\n").unwrap();

        let outcome = OpenspecPlanAdapter.parse(&PlanSourceHandle::Path(tmp.path().to_path_buf()));
        assert!(outcome.malformed.is_empty(), "a well-formed `changes` dir must never be flagged malformed: {:?}", outcome.malformed);
        assert!(outcome.changes.iter().any(|c| c.change_id.as_str() == "changes"), "it must import as an ordinary Change record");
    }

    #[test]
    fn spec_delta_scenarios_and_design_docs_produce_no_scenario_records_and_exact_named_counts() {
        let outcome = parse_fixture();
        assert_eq!(outcome.unmapped.get(DIAG_SPEC_DELTA_SCENARIO), Some(&3), "add-widget/specs/widget/spec.md carries exactly 3 `#### Scenario:` blocks");
        assert_eq!(outcome.unmapped.get(DIAG_DESIGN_DOC), Some(&1), "exactly one design.md across the fixture tree");
        // No Scenario records exist to emit in the first place — this
        // crate's `canon_model::records::Scenario` type is never
        // referenced anywhere in this adapter (structural proof: the
        // adapter module doesn't even import it).
    }

    // ── covers segment -> Task.scenario_refs (s20 P2 tasks 2.1/2.2) ──

    #[test]
    fn a_covers_bearing_row_maps_onto_task_scenario_refs_with_task_id_parity_preserved() {
        let outcome = parse_fixture();
        let task = find_task(&outcome, "covers-change#1.1");
        assert_eq!(task.task_id.as_str(), "covers-change#1.1", "the covers segment never perturbs task_id derivation");
        assert_eq!(task.scenario_refs, vec![ScenarioId::parse("world.hotdeal.01").unwrap(), ScenarioId::parse("world.hotdeal.02").unwrap()]);
        assert_eq!(task.title, "wire the hotdeal surface", "title excludes the bracket segment");
    }

    #[test]
    fn one_malformed_covers_token_is_dropped_and_counted_under_a_named_diagnostic_without_sinking_the_row() {
        let outcome = parse_fixture();
        let task = find_task(&outcome, "covers-change#1.2");
        assert_eq!(task.scenario_refs, vec![ScenarioId::parse("world.hotdeal.03").unwrap(), ScenarioId::parse("world.hotdeal.04").unwrap()]);
        assert_eq!(
            outcome.unmapped.get(&format!("{DIAG_MALFORMED_SCENARIO_REF}:covers-change#1.2")),
            Some(&1),
            "the malformed diagnostic surfaces in the pass summary named per row (task_id embedded in the key)"
        );
    }

    #[test]
    fn a_covers_free_row_has_empty_scenario_refs() {
        let outcome = parse_fixture();
        let task = find_task(&outcome, "covers-change#1.3");
        assert!(task.scenario_refs.is_empty());
    }

    #[test]
    fn two_parses_of_the_same_snapshot_are_byte_identical() {
        let first = parse_fixture();
        let second = parse_fixture();
        assert_eq!(first, second, "fixed actor + mtime-derived at + deterministic discovery order -> no wall-clock drift between runs");
    }

    // ── task_id derivation parity against the S4 verdict adapter (task 2.7) ──

    #[test]
    fn task_id_derivation_is_byte_identical_to_the_verdict_adapter_and_a_proper_subset() {
        let plan_outcome = parse_fixture();

        let verdict_adapter = OpenspecTaskAdapter;
        let verdict_config = ArtifactSourceConfig { openspec_root: Some(fixture_root()), ..Default::default() };
        let verdict_source = verdict_adapter.resolve_source(&verdict_config).expect("fixture root configured");
        let verdict_outcome = verdict_adapter.parse(&verdict_source);

        let plan_task_ids: BTreeSet<TaskId> = plan_outcome.tasks.iter().map(|t| t.task_id.clone()).collect();
        let plan_change_ids: BTreeSet<ChangeId> = plan_outcome.changes.iter().map(|c| c.change_id.clone()).collect();
        let all_verdict_task_ids: BTreeSet<TaskId> = verdict_outcome
            .events
            .iter()
            .filter_map(|event| match &event.join_key {
                ArtifactJoinKey::Task(task_id) => Some(task_id.clone()),
                _ => None,
            })
            .collect();

        // The parity claim is about rows BOTH adapters read under the
        // SAME successfully-imported change dir. `missing-proposal-change`
        // is a dir the verdict adapter still reads (it has no
        // proposal.md requirement at all) but the plan adapter skips
        // whole (task 2.2) — so it is deliberately excluded here, and
        // separately asserted below to prove that exclusion is real,
        // not silently vacuous.
        let verdict_task_ids: BTreeSet<TaskId> =
            all_verdict_task_ids.iter().filter(|task_id| plan_change_ids.contains(&task_id.change_id())).cloned().collect();

        assert!(!verdict_task_ids.is_empty(), "the fixture must actually exercise the verdict adapter's event path");
        for task_id in &verdict_task_ids {
            assert!(
                plan_task_ids.contains(task_id),
                "task_id {task_id:?} the verdict adapter emitted an event for must be byte-identical to one the plan adapter emits"
            );
        }
        assert!(
            verdict_task_ids.is_subset(&plan_task_ids) && verdict_task_ids != plan_task_ids,
            "the verdict adapter's emitted task_id set must be a PROPER subset — the plan side also emits untouched-open rows (e.g. add-widget#1.5) the verdict adapter skips via NotApplicable"
        );
        assert!(plan_task_ids.contains(&TaskId::parse("add-widget#1.5").unwrap()), "the untouched-open row is plan-only, proving the subset is proper");
        assert!(!verdict_task_ids.contains(&TaskId::parse("add-widget#1.5").unwrap()), "the verdict adapter never emits an event for an untouched-open row");

        let missing_proposal_task_id = TaskId::parse("missing-proposal-change#1.1").unwrap();
        assert!(
            all_verdict_task_ids.contains(&missing_proposal_task_id),
            "sanity: the verdict adapter has no proposal.md requirement, so it DOES read missing-proposal-change/tasks.md"
        );
        assert!(
            !plan_task_ids.contains(&missing_proposal_task_id),
            "the plan adapter never emits this task_id — its whole change dir was skipped malformed for lacking proposal.md"
        );
    }

    #[test]
    fn status_tally_counts_a_row_with_an_unparseable_task_number_as_open() {
        // Pins the D6 interpretation documented on `parse_tasks_file`:
        // status derivation counts every row `parse_line` recognizes as
        // a checkbox row, independent of the row's own `<n>` grammar
        // validity (a separate, later concern that only gates Task
        // emission). `missing-why-change` carries two untouched-open
        // rows with valid numeric ids and no done rows, so it already
        // proves `proposed`; this test isolates the bad-id
        // contribution specifically via `add-widget`, which mixes a
        // done row with ONLY a bad-id open row plus other genuinely
        // open rows — asserting the dir is `in_progress` (not
        // `completed`) proves the bad-id row is NOT silently excluded
        // from the open tally.
        let outcome = parse_fixture();
        let change = find_change(&outcome, "add-widget");
        assert_eq!(change.status, ChangeStatus::InProgress);
    }


    #[test]
    fn malformed_entry_paths_never_leak_the_absolute_source_root() {
        // s18 `loud-plan-import-diagnostics` spec + `MalformedEntry
        // ::path`'s own doc comment: relative to the source root,
        // "never an absolute path leaking the host filesystem layout"
        // — the existing `ends_with`/`contains`-style assertions above
        // are blind to a path that happens to ALSO carry the absolute
        // prefix; this test checks the negative directly.
        let outcome = parse_fixture();
        let root = fixture_root();
        let root_str = root.to_str().unwrap();
        assert!(!outcome.malformed.is_empty(), "the fixture must exercise at least one malformed entry");
        for entry in &outcome.malformed {
            assert!(!entry.path.starts_with(root_str), "malformed entry path leaked the absolute fixture root: {}", entry.path);
            assert!(!entry.path.starts_with('/'), "malformed entry path must never be absolute: {}", entry.path);
            if let Some(hint) = &entry.hint {
                assert!(!hint.contains(root_str), "malformed entry hint leaked the absolute fixture root: {hint}");
            }
        }
    }
}
