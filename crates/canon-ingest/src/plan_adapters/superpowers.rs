//! The `PlanAdapter` for the `superpowers` `writing-plans`-skill plan
//! dialect (s30 `plan-dialect-superpowers`, s17 D9's named follow-up —
//! deferred there for lack of a grammar authority, now shipped against
//! one).
//!
//! # The grammar authority (design D1)
//! The shape this adapter pins is exactly what the superpowers
//! `writing-plans` skill instructs authors to produce: a
//! `# <Feature Name> Implementation Plan` H1, a one-sentence
//! `**Goal:** <sentence>` header line, `### Task N: <Component Name>`
//! sections, and `- [ ]`/`- [x]` checkbox STEP lines inside each
//! section (the skill's `**Step N:**` bolding is NOT load-bearing —
//! [`checkbox_state`] recognizes any checkbox line, bold or not).
//!
//! # Identity + the shared join key (design D2/D3)
//! `change_id` is the filename stem, slugified ([`slugify`]) then
//! validated through [`ChangeId::parse`] — forgiving of punctuation a
//! raw basename-as-identity dialect (openspec's) would reject outright,
//! since the superpowers convention is prose-derived
//! (`YYYY-MM-DD-<feature-name>.md`), not an author-picked slug. Every
//! `Task`'s `task_id` still derives through the SAME shared
//! [`crate::task_rows::task_id_for`] the openspec dialect and the
//! S4 verdict adapter use — one join-key derivation for every reader
//! (design D3, s17 D5/R5's "two readers, one join" extended to a third
//! dialect).
//!
//! # Status derivation is shared with the openspec dialect (design D4)
//! A superpowers plan has no archive convention, so `Change` status is
//! [`super::openspec::derive_status`] called with `archived: false` —
//! the SAME tally semantics (`(done, open)` -> proposed/in_progress/
//! completed), reused rather than re-derived so the two dialects can
//! never silently drift on what "in progress" means. `done`/`open`
//! here tally derived TASK statuses (one increment per well-formed,
//! non-duplicate `### Task N:` section), never raw checkbox lines —
//! design D3's "the section's checkboxes... are ignored" for an
//! invalid/duplicate heading means they contribute to NEITHER a `Task`
//! NOR the `Change`-level tally.
//!
//! # Unmapped + malformed vocabulary (design D6)
//! Steps, `**Architecture:**`/`**Tech Stack:**` prose, Global
//! Constraints, and non-task headings are simply never read — no
//! per-line diagnostic (design D6: "a construct-per-drop diagnostic
//! for every step line would be noise, not signal"). Only two named
//! `unmapped` diagnostics exist ([`DIAG_GOAL_MISSING`],
//! [`DIAG_NOT_A_PLAN_DOC`]) plus four `malformed` reasons
//! (`"unreadable-file"`, `"invalid-change-id-slug"`,
//! `"invalid-task-number"`, `"duplicate-task-number"`).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{ChangeId, TaskId};
use canon_model::records::{Change, Task, TaskStatus};
use chrono::{DateTime, TimeZone, Utc};

use crate::task_rows;
use crate::plan_writeback::{FlipDocOutcome, PlanTaskLocation, PlanWriteBack, WriteBackError};
use crate::plan_adapter::{PlanAdapter, PlanParseOutcome, PlanSourceConfig, PlanSourceHandle, resolve_path_source};
use crate::plan_adapters::openspec::derive_status;

/// canon-model's envelope schema version every record this adapter
/// constructs carries (mirrors `openspec.rs`'s `SCHEMA_VERSION`'s
/// own doc comment — a fresh per-dialect constant since that one is
/// private to its own module, never a cross-module import for a bare
/// `u32`).
const SCHEMA_VERSION: u32 = 1;

/// The fixed, per-dialect unattributed actor every `Change`/`Task`
/// this adapter emits carries (design D7, s17 D7's identical
/// "provenance visible in every record, byte-stable across runs" —
/// never a wall-clock- or run-derived value).
const ACTOR_AGENT_ID: &str = "canon-plan-import-superpowers";

/// Named diagnostic (design D4) for a plan doc with no `**Goal:**`
/// line — the `Change` still imports, with an empty summary rather
/// than invented prose. `pub(crate)` so `crate::plan_selftest`'s
/// fixture-corpus oracle can assert against the SAME stable name this
/// adapter emits, rather than a second string literal that could drift.
pub(crate) const DIAG_GOAL_MISSING: &str = "goal-missing";
/// Named diagnostic (design D5) for a markdown file under the plans
/// root that carries neither a `**Goal:**` line nor any
/// `### Task N:` heading — a docs-dir false positive (e.g. a stray
/// `README.md`) is skipped loud, never imported as a garbage `Change`.
pub(crate) const DIAG_NOT_A_PLAN_DOC: &str = "not-a-plan-doc";

pub struct SuperpowersPlanAdapter;

impl PlanAdapter for SuperpowersPlanAdapter {
    fn dialect_id(&self) -> &'static str {
        "superpowers"
    }

    fn resolve_source(&self, config: &PlanSourceConfig) -> Option<PlanSourceHandle> {
        resolve_path_source(&config.root)
    }

    fn parse(&self, source: &PlanSourceHandle) -> PlanParseOutcome {
        let PlanSourceHandle::Path(root) = source;
        let mut outcome = PlanParseOutcome::empty();
        for file in discover_plan_files(root) {
            parse_plan_doc(&file, root, &mut outcome);
        }
        outcome
    }
}

/// s35 `gate-plan-dialect-seam` (design D1): the superpowers dialect can
/// LOCATE a task's plan doc (which `docs/superpowers/plans/*.md` file's
/// slugified stem matches the change) but does NOT support the
/// evidence-gated flip. Its `### Task N:` sections carry `**Step N:**`
/// checkbox lines with no canonical per-row evidence-suffix convention
/// to round-trip (`crate::task_rows`'s ` — ✅ ` grammar is not part of
/// the `writing-plans` skill's shape), so [`flip_task`] returns a loud,
/// typed [`WriteBackError::Unsupported`] naming the dialect rather than
/// silently no-op'ing a flip an operator believes landed.
/// [`typed_atoms_path`] is `None`: this dialect has no S10
/// typed-vocabulary convention.
impl PlanWriteBack for SuperpowersPlanAdapter {
    fn locate_task(&self, root: &Path, task_id: &TaskId) -> Option<PlanTaskLocation> {
        // The plan doc whose slugified filename stem IS this task's
        // change (design D2's stem->slug->ChangeId identity), regardless
        // of whether the specific `### Task <n>` section exists inside —
        // FILE existence only, mirroring the openspec dialect (module
        // doc).
        for file in discover_plan_files(root) {
            let Some(stem) = file.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(cid) = ChangeId::parse(slugify(stem)) else {
                continue;
            };
            if cid == task_id.change_id() {
                return Some(PlanTaskLocation { document_path: file });
            }
        }
        None
    }

    fn flip_task(&self, _document: &str, _task_id: &TaskId, _evidence_note: &str) -> Result<FlipDocOutcome, WriteBackError> {
        Err(WriteBackError::Unsupported { dialect: "superpowers" })
    }

    fn typed_atoms_path(&self, _root: &Path, _change_id: &ChangeId) -> Option<PathBuf> {
        None
    }
}

fn actor() -> Actor {
    Actor::new_unattributed(ACTOR_AGENT_ID)
}

/// Find every plan document this adapter should read from `root`
/// (design D5): the immediate `*.md` children of
/// `<root>/docs/superpowers/plans/` when that substructure exists (the
/// ordinary consumer-repo shape), otherwise the immediate `*.md`
/// children of `root` itself (the plans dir passed directly, or a
/// fixture dir holding bare plan docs — mirrors
/// `discover_change_dirs`'s identical fallback).
/// Subdirectories are never descended into — the skill's flat layout
/// is not recursive. Deterministic (byte-lexical path order) so two
/// passes over the same tree enumerate identically.
fn discover_plan_files(root: &Path) -> Vec<PathBuf> {
    let plans_dir = root.join("docs").join("superpowers").join("plans");
    let scan_root: PathBuf = if plans_dir.is_dir() { plans_dir } else { root.to_path_buf() };
    list_md_files(&scan_root)
}

/// Immediate `*.md` file children of `dir`, byte-lexically sorted. A
/// missing/unreadable `dir` yields an empty `Vec` (mirrors
/// `list_subdirs`'s "absent root -> zero records,
/// never an error" contract), never a hardcoded fallback path.
fn list_md_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();
    files.sort_unstable();
    files
}

/// Render `path` relative to `root` (s18 `loud-plan-import-
/// diagnostics` spec's "the construct's relative path" contract, same
/// discipline as `openspec.rs`'s `relative_to_root`) — every path
/// this adapter derives is actually rooted under `root`, so
/// `strip_prefix` always succeeds in practice; falling back to `path`
/// verbatim on the (should-be-unreachable) failure case is a
/// defensive never-panic, never the ordinary path.
fn relative_to_root(path: &Path, root: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).display().to_string()
}

/// Lowercase `stem`, collapse each `[^a-z0-9]+` run to one `-`, trim
/// edge `-` (design D2). Unlike openspec's raw-basename identity, this
/// is a forgiving transform — a stem that slugs to an EMPTY string
/// (e.g. an all-punctuation filename) is the only way this adapter's
/// `invalid-change-id-slug` malformed reason fires, since any
/// non-empty result composed of `[a-z0-9-]` with no leading/trailing/
/// doubled `-` always passes [`ChangeId::parse`].
fn slugify(stem: &str) -> String {
    let mut result = String::with_capacity(stem.len());
    let mut last_was_sep = true; // suppresses a leading '-'
    for ch in stem.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_lowercase() || lower.is_ascii_digit() {
            result.push(lower);
            last_was_sep = false;
        } else if !last_was_sep {
            result.push('-');
            last_was_sep = true;
        }
    }
    if result.ends_with('-') {
        result.pop();
    }
    result
}

/// The plan doc's H1 title (`# <Feature Name> Implementation Plan`,
/// design D1) — display prose only, never identity (design D2). The
/// first line trimming to `# <rest>` (a bare single `#`, never `##`+)
/// wins; absent entirely, the caller falls back to the (unslugged)
/// filename stem.
fn h1_title(text: &str) -> Option<String> {
    text.lines().map(str::trim).find_map(|line| line.strip_prefix("# ").map(|title| title.trim().to_string()))
}

/// One `**Goal:**` header line's remainder, whitespace-normalized
/// (design D4) — `None` when `line` (already trimmed) is not a Goal
/// line at all, distinct from an empty-but-present Goal line's `Some(
/// String::new())`.
fn goal_line(line: &str) -> Option<String> {
    let rest = line.strip_prefix("**Goal:**")?;
    Some(rest.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// One `### Task N: <name>` heading's `(n_token, whitespace-normalized
/// name)` — `None` when `line` (already trimmed) does not even match
/// the `### Task <token>:` SHAPE (no colon at all is a non-task
/// heading, design D6, never counted as an attempt). `n_token`'s OWN
/// grammar validity ([`task_rows::is_task_number`]) is checked
/// later, by the caller — this function only recognizes the shape.
fn task_heading(line: &str) -> Option<(&str, String)> {
    let rest = line.strip_prefix("### Task ")?;
    let colon = rest.find(':')?;
    let n_token = rest[..colon].trim();
    let name = rest[colon + 1..].split_whitespace().collect::<Vec<_>>().join(" ");
    Some((n_token, name))
}

/// `true` when `line` (untrimmed — leading indent is the checkbox's
/// own, per Markdown list nesting) is a Markdown checkbox list item —
/// `- [ ]`/`- [x]`/`- [X]`, with the checked/unchecked state. Deliberately
/// looser than [`task_rows::parse_line`]'s `- [ ]/[x] <id> …` shape
/// (design D1: "the skill's `**Step N:**` bolding is NOT load-bearing
/// — any checkbox line inside the section counts"), so a step line
/// with no id token at all (`- [x] **Step 1:** wire it up`) still
/// counts.
fn checkbox_state(line: &str) -> Option<bool> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("- [")?;
    let mark = rest.chars().next()?;
    let after_mark = &rest[mark.len_utf8()..];
    if !after_mark.starts_with(']') {
        return None;
    }
    match mark {
        ' ' => Some(false),
        'x' | 'X' => Some(true),
        _ => None,
    }
}

/// `true` when `line` (untrimmed) is a Markdown heading of any level —
/// the boundary a `### Task N:` section's checkbox-STEP scan stops at.
fn is_heading_line(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// One attempted `### Task N: <name>` section (design D3): the heading
/// SHAPE matched ([`task_heading`]); `n_token`'s own grammar validity
/// is checked by the caller. `done`/`open` are this section's own
/// checkbox-STEP tallies ([`checkbox_state`]) — never leaked into a
/// sibling section, since a heading line (any level) always closes the
/// current one first.
struct AttemptedSection {
    n_token: String,
    name: String,
    done: usize,
    open: usize,
}

/// One forward pass over `text`'s lines producing: the first
/// `**Goal:**` line's remainder (`None` when absent), whether ANY
/// `### Task N:` heading SHAPE was seen at all (design D5's
/// plan-shape OR-condition — independent of any individual heading's
/// `n_token` validity), and every attempted task section in document
/// order (design D3).
fn scan_plan_doc(text: &str) -> (Option<String>, bool, Vec<AttemptedSection>) {
    let mut goal: Option<String> = None;
    let mut has_task_heading_shape = false;
    let mut sections: Vec<AttemptedSection> = Vec::new();
    let mut current: Option<AttemptedSection> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if goal.is_none() {
            if let Some(g) = goal_line(trimmed) {
                goal = Some(g);
            }
        }

        if is_heading_line(line) {
            if let Some(section) = current.take() {
                sections.push(section);
            }
            if let Some((n_token, name)) = task_heading(trimmed) {
                has_task_heading_shape = true;
                current = Some(AttemptedSection { n_token: n_token.to_string(), name, done: 0, open: 0 });
            }
            continue;
        }

        if let Some(checked) = checkbox_state(line) {
            if let Some(section) = current.as_mut() {
                if checked {
                    section.done += 1;
                } else {
                    section.open += 1;
                }
            }
        }
    }
    if let Some(section) = current.take() {
        sections.push(section);
    }

    (goal, has_task_heading_shape, sections)
}

/// Parse one plan document into `outcome`: at most one `Change`
/// candidate plus zero or more `Task` candidates, or a named
/// unmapped/malformed entry when the file is unreadable, not
/// plan-shaped at all, or its filename slugs to an invalid
/// [`ChangeId`]. Never a crash — every failure mode here is
/// skip-and-count (design D5/D6).
fn parse_plan_doc(path: &Path, root: &Path, outcome: &mut PlanParseOutcome) {
    let Ok(text) = fs::read_to_string(path) else {
        outcome.record_malformed(relative_to_root(path, root), "unreadable-file");
        return;
    };

    let (goal, has_task_heading_shape, sections) = scan_plan_doc(&text);

    if goal.is_none() && !has_task_heading_shape {
        // Neither a Goal line nor any `### Task N:` heading shape at
        // all — a docs-dir false positive (design D5), never imported.
        outcome.record_unmapped(DIAG_NOT_A_PLAN_DOC);
        return;
    }

    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        outcome.record_malformed(relative_to_root(path, root), "unreadable-file");
        return;
    };
    let Ok(change_id) = ChangeId::parse(slugify(stem)) else {
        outcome.record_malformed(relative_to_root(path, root), "invalid-change-id-slug");
        return;
    };

    if goal.is_none() {
        outcome.record_unmapped(DIAG_GOAL_MISSING);
    }
    let summary = goal.unwrap_or_default();
    let title = h1_title(&text).unwrap_or_else(|| stem.to_string());
    let at = file_modified_at(path);

    let mut seen_numbers: BTreeSet<String> = BTreeSet::new();
    let mut tasks = Vec::new();
    let mut done_count = 0usize;
    let mut open_count = 0usize;

    for section in sections {
        let Some(task_id) = task_rows::task_id_for(&change_id, &section.n_token) else {
            // Invalid `<n>` (design D3): named malformed, and the
            // section's checkboxes belong to NO task -- excluded from
            // both Task emission and the Change-level tally below.
            outcome.record_malformed(format!("{}#{}", relative_to_root(path, root), section.n_token), "invalid-task-number");
            continue;
        };
        if !seen_numbers.insert(section.n_token.clone()) {
            // Duplicate `Task N` heading (design D3): first wins, this
            // later one is named malformed and its checkboxes are
            // likewise excluded from the tally.
            outcome.record_malformed(format!("{}#{}", relative_to_root(path, root), section.n_token), "duplicate-task-number");
            continue;
        }

        let status = if section.done > 0 && section.open == 0 { TaskStatus::Done } else { TaskStatus::Open };
        match status {
            TaskStatus::Done => done_count += 1,
            TaskStatus::Open => open_count += 1,
        }
        let envelope = Envelope::new(SCHEMA_VERSION, RecordKind::Task, at, actor());
        tasks.push(Task::new(envelope, task_id, section.name, status, None));
    }

    // No archive convention (design D4): `derive_status` shared
    // verbatim with the openspec dialect, `archived: false` always.
    let status = derive_status(false, done_count, open_count);
    let envelope = Envelope::new(SCHEMA_VERSION, RecordKind::Change, at, actor());
    outcome.changes.push(Change::new(envelope, change_id, title, summary, status));
    outcome.tasks.extend(tasks);
}

/// A plan doc carries no per-record timestamp of its own — the file's
/// own mtime is the best available "when observed" signal (design D4,
/// `file_modified_at` convention; mirrors
/// `openspec.rs`'s `file_modified_at`'s identical fallback-to-now
/// behavior, this crate's established per-adapter idiom for a source
/// with no native timestamp field).
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
    use canon_model::records::ChangeStatus;

    use super::*;

    fn write_plan(dir: &Path, filename: &str, contents: &str) -> PathBuf {
        let path = dir.join(filename);
        fs::write(&path, contents).expect("write fixture plan doc");
        path
    }

    fn parse_root(root: &Path) -> PlanParseOutcome {
        let adapter = SuperpowersPlanAdapter;
        let config = PlanSourceConfig { root: Some(root.to_path_buf()) };
        let handle = adapter.resolve_source(&config).expect("configured root resolves");
        adapter.parse(&handle)
    }

    fn find_change<'a>(outcome: &'a PlanParseOutcome, change_id: &str) -> &'a Change {
        outcome.changes.iter().find(|c| c.change_id.as_str() == change_id).unwrap_or_else(|| panic!("{change_id} not found"))
    }

    fn find_task<'a>(outcome: &'a PlanParseOutcome, task_id: &str) -> &'a Task {
        outcome.tasks.iter().find(|t| t.task_id.as_str() == task_id).unwrap_or_else(|| panic!("{task_id} not found"))
    }

    #[test]
    fn dialect_id_is_superpowers() {
        assert_eq!(SuperpowersPlanAdapter.dialect_id(), "superpowers");
    }

    #[test]
    fn resolve_source_is_none_when_unconfigured() {
        assert!(SuperpowersPlanAdapter.resolve_source(&PlanSourceConfig::default()).is_none());
    }

    /// Spec `A writing-plans-shaped doc becomes a Change keyed by its
    /// filename stem`: the exact worked example from spec.md — filename
    /// stem identity, verbatim `**Goal:**` summary, and the fixed
    /// per-dialect actor.
    #[test]
    fn a_writing_plans_shaped_doc_becomes_a_change_keyed_by_its_filename_stem() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(
            tmp.path(),
            "2026-07-14-website-design.md",
            "# Website Implementation Plan\n\n**Goal:** Build the project website.\n\n### Task 1: Adapter\n- [x] wire it up\n",
        );

        let outcome = parse_root(tmp.path());
        let change = find_change(&outcome, "2026-07-14-website-design");
        assert_eq!(change.summary, "Build the project website.");
        assert_eq!(change.title, "Website Implementation Plan");
        assert_eq!(change.envelope.actor, Actor::new_unattributed("canon-plan-import-superpowers"));
    }

    /// Spec `A Goal-less plan imports with an empty summary and a named
    /// diagnostic`: task headings present, no `**Goal:**` line at all.
    #[test]
    fn a_goal_less_plan_imports_with_an_empty_summary_and_a_named_diagnostic() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(tmp.path(), "2026-07-14-no-goal.md", "# No Goal Plan\n\n### Task 1: Adapter\n- [ ] step one\n");

        let outcome = parse_root(tmp.path());
        let change = find_change(&outcome, "2026-07-14-no-goal");
        assert_eq!(change.summary, "", "never invented prose when the Goal line is absent");
        assert_eq!(outcome.unmapped.get(DIAG_GOAL_MISSING), Some(&1));
    }

    /// Spec `Checked steps complete a task, unchecked steps keep it
    /// open`: an all-checked section is Done, a mixed section is Open,
    /// and (this test's addition, same requirement's body text) a
    /// zero-checkbox section is Open too, never Done.
    #[test]
    fn checkbox_status_matrix_done_mixed_and_zero_checkbox_sections() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(
            tmp.path(),
            "2026-07-14-status-matrix.md",
            "# Status Matrix Plan\n\n**Goal:** Prove the checkbox status matrix.\n\n\
             ### Task 1: Adapter\n- [x] step one\n- [x] step two\n\n\
             ### Task 2: Docs\n- [x] step one\n- [ ] step two\n\n\
             ### Task 3: Empty\nNo checkbox lines in this section at all.\n",
        );

        let outcome = parse_root(tmp.path());
        assert_eq!(find_task(&outcome, "2026-07-14-status-matrix#1").status, TaskStatus::Done, "all-checked -> Done");
        assert_eq!(find_task(&outcome, "2026-07-14-status-matrix#2").status, TaskStatus::Open, "mixed -> Open");
        assert_eq!(find_task(&outcome, "2026-07-14-status-matrix#3").status, TaskStatus::Open, "zero checkboxes -> Open, never Done");
        assert_eq!(
            find_change(&outcome, "2026-07-14-status-matrix").status,
            ChangeStatus::InProgress,
            "1 done + 2 open tasks -> in_progress, same derive_status tally openspec uses"
        );
    }

    /// Spec requirement body text: "a duplicate task number SHALL keep
    /// the first section and name the later one malformed".
    #[test]
    fn a_duplicate_task_number_keeps_the_first_section_and_names_the_later_one_malformed() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(
            tmp.path(),
            "2026-07-14-dup-task.md",
            "# Dup Task Plan\n\n**Goal:** Prove duplicate task numbers.\n\n\
             ### Task 1: First\n- [x] done here\n\n\
             ### Task 1: Second\n- [ ] never counted\n",
        );

        let outcome = parse_root(tmp.path());
        let task = find_task(&outcome, "2026-07-14-dup-task#1");
        assert_eq!(task.title, "First", "the FIRST section wins, verbatim");
        assert_eq!(task.status, TaskStatus::Done, "the first section's own tally, untouched by the duplicate");
        assert_eq!(outcome.tasks.iter().filter(|t| t.task_id.as_str() == "2026-07-14-dup-task#1").count(), 1, "never a second Task for the same task_id");
        let entry = outcome.malformed.iter().find(|e| e.reason == "duplicate-task-number").expect("the later heading must be named malformed");
        assert!(entry.path.ends_with("#1"), "path: {}", entry.path);
    }

    /// Spec requirement body text: "An invalid task number SHALL be
    /// skipped and named malformed".
    #[test]
    fn an_invalid_task_number_is_skipped_and_named_malformed() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(
            tmp.path(),
            "2026-07-14-bad-number.md",
            "# Bad Number Plan\n\n**Goal:** Prove invalid task numbers are skipped.\n\n### Task one: Adapter\n- [x] step\n",
        );

        let outcome = parse_root(tmp.path());
        assert!(outcome.tasks.is_empty(), "a non-numeric heading never emits a Task");
        let entry = outcome.malformed.iter().find(|e| e.reason == "invalid-task-number").expect("must be named malformed");
        assert!(entry.path.ends_with("#one"), "path: {}", entry.path);
        // The Change itself still imports -- one malformed heading
        // does not sink the whole document (design D3/D6).
        let change = find_change(&outcome, "2026-07-14-bad-number");
        assert_eq!(change.status, ChangeStatus::Proposed, "the invalid section contributes to NEITHER the done nor open tally");
    }

    /// Spec `A stray README in the plans dir is named, not imported`.
    #[test]
    fn a_stray_readme_in_the_plans_dir_is_named_not_imported() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(
            tmp.path(),
            "2026-07-14-real-plan.md",
            "# Real Plan\n\n**Goal:** A real plan doc.\n\n### Task 1: Adapter\n- [x] step\n",
        );
        write_plan(tmp.path(), "README.md", "# Plans Directory\n\nJust an ordinary docs README with no Goal and no Task headings.\n");

        let outcome = parse_root(tmp.path());
        assert_eq!(outcome.changes.len(), 1, "exactly one Change imports");
        assert_eq!(find_change(&outcome, "2026-07-14-real-plan").summary, "A real plan doc.");
        assert_eq!(outcome.unmapped.get(DIAG_NOT_A_PLAN_DOC), Some(&1));
    }

    /// Spec requirement body text: "an absent or unreadable root SHALL
    /// yield zero records without error".
    #[test]
    fn an_absent_root_yields_zero_records_without_error() {
        let source = PlanSourceHandle::Path(PathBuf::from("/tmp/definitely-does-not-exist-s30-superpowers"));
        assert_eq!(SuperpowersPlanAdapter.parse(&source), PlanParseOutcome::empty());
    }

    /// Spec `The task join key is byte-identical to the S4 verdict
    /// layer's`: this adapter's `task_id` for change `x` task `3` is
    /// exactly what [`task_rows::task_id_for`] (the ONE shared
    /// derivation, design D3) produces directly -- no second grammar.
    #[test]
    fn the_task_join_key_is_byte_identical_to_task_rows_task_id_for() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(tmp.path(), "join-key-parity.md", "# Join Key Parity\n\n**Goal:** Prove join-key parity.\n\n### Task 3: Parity\n- [x] step\n");

        let outcome = parse_root(tmp.path());
        let task = find_task(&outcome, "join-key-parity#3");
        let change_id = ChangeId::parse("join-key-parity").unwrap();
        let expected = task_rows::task_id_for(&change_id, "3").expect("3 is a valid task number");
        assert_eq!(task.task_id, expected, "one derivation, no second grammar (design D3)");
    }

    #[test]
    fn an_unreadable_file_is_named_malformed_never_a_crash() {
        let tmp = tempfile::tempdir().unwrap();
        // Invalid UTF-8 bytes make `fs::read_to_string` fail even
        // though the path is an ordinary readable file -- exercises
        // the `unreadable-file` malformed path without touching real
        // filesystem permissions.
        fs::write(tmp.path().join("not-utf8.md"), [0x2d, 0x20, 0xff, 0xfe, 0x00]).unwrap();

        let outcome = parse_root(tmp.path());
        assert!(outcome.changes.is_empty());
        assert_eq!(outcome.malformed.len(), 1);
        assert_eq!(outcome.malformed[0].reason, "unreadable-file");
    }

    #[test]
    fn an_all_punctuation_stem_slugs_to_empty_and_is_named_malformed() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(tmp.path(), "!!!.md", "# Punctuation Only\n\n**Goal:** Prove empty-slug rejection.\n\n### Task 1: Adapter\n- [x] step\n");

        let outcome = parse_root(tmp.path());
        assert!(outcome.changes.is_empty());
        let entry = outcome.malformed.iter().find(|e| e.reason == "invalid-change-id-slug").expect("must be named malformed");
        assert!(entry.path.ends_with("!!!.md"), "path: {}", entry.path);
    }

    #[test]
    fn discovery_prefers_docs_superpowers_plans_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(
            &{
                let dir = tmp.path().join("docs/superpowers/plans");
                fs::create_dir_all(&dir).unwrap();
                dir
            },
            "2026-07-14-nested.md",
            "# Nested Plan\n\n**Goal:** Discovered through the nested shape.\n",
        );
        // A decoy at root level must NOT be discovered once the nested
        // substructure exists (mirrors the openspec dialect's own
        // shape-tolerance discipline).
        write_plan(tmp.path(), "2026-07-14-decoy.md", "# Decoy\n\n**Goal:** Never discovered.\n");

        let outcome = parse_root(tmp.path());
        assert!(outcome.changes.iter().any(|c| c.change_id.as_str() == "2026-07-14-nested"));
        assert!(outcome.changes.iter().all(|c| c.change_id.as_str() != "2026-07-14-decoy"));
    }
}
