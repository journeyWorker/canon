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
/// Named diagnostic (s37 `execution-graph-topology`, subject
/// `flywheel-execution-graph`) for one `- Consumes:` task reference
/// that named no `### Task N:` section this document actually has —
/// dropped from the imported `Task.depends_on`, counted once per
/// unresolvable token, never sinking the section's other well-formed
/// dependencies or its own `Task` import. Keyed
/// `"<DIAG_UNRESOLVABLE_TASK_DEP>:<task_id>"` for the same reason the
/// openspec dialect's identically-named diagnostic is (a flat count
/// cannot tell an operator WHICH section to fix); a fresh per-module
/// constant rather than a cross-module import, exactly like the
/// openspec dialect's own identically-named constant.
pub(crate) const DIAG_UNRESOLVABLE_TASK_DEP: &str = "unresolvable-task-dep";

pub struct SuperpowersPlanAdapter;

impl PlanAdapter for SuperpowersPlanAdapter {
    fn dialect_id(&self) -> &'static str {
        "superpowers"
    }

    /// `2` — this dialect's parse output changed for identical input
    /// when `s37-execution-graph-topology` taught [`parse_plan_doc`] to
    /// extract `depends_on` from the `- Consumes: … from Task <n>`
    /// interface line. An unchanged plan doc therefore yields DIFFERENT
    /// `Task` records than it did before that change, which is exactly
    /// what this generation declares to the plan-import cursor
    /// (`canon-cli::plans::plan_source_cursor_id`) so the source is
    /// re-parsed rather than reported `skipped unchanged`.
    fn parse_version(&self) -> u32 {
        2
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

/// The `**Interfaces:**` block's dependency bullet label (design D1's
/// grammar authority — the `writing-plans` skill's own task template:
/// "Consumes: [what this task uses from earlier tasks — exact
/// signatures]"). Matched case-insensitively after a `- `/`* ` list
/// marker.
const CONSUMES_LABEL: &str = "consumes:";

/// The reference keyword a `- Consumes:` bullet names its source
/// sections with. Every one of the corpus's Consumes lines is either
/// `nothing (first task)` or cites sections exactly this way — `from
/// Task 1`, `(Task 2)`, `(Tasks 2-3)`, `everything produced by Tasks
/// 1-3`, `the fully assembled app from Tasks 1-5` — so this dialect's
/// dependency expression is READ, not invented (s37
/// `execution-graph-topology`).
const TASK_KEYWORDS: [&str; 2] = ["task", "tasks"];

/// One `- Consumes: …` bullet's remainder (design D1's
/// `**Interfaces:**` block), or `None` when `line` is not that bullet.
/// Distinct from a checkbox STEP line ([`checkbox_state`] requires
/// `- [`), so the two recognizers can never claim the same line.
fn consumes_line(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* "))?;
    let len = CONSUMES_LABEL.len();
    // `is_char_boundary` is false past the end AND mid-character, so
    // this one guard covers both (the bullet text is arbitrary prose).
    if !rest.is_char_boundary(len) || !rest[..len].eq_ignore_ascii_case(CONSUMES_LABEL) {
        return None;
    }
    Some(rest[len..].trim())
}

/// Extract every DECLARED dependency reference token from one section's
/// `- Consumes:` bullet text (s37 `execution-graph-topology`). Returns
/// raw `<n>` tokens in first-seen order; resolution against the
/// document's own sections, and the drop of anything that does not
/// resolve, is [`task_rows::resolve_declared_deps`]'s job.
///
/// Grammar: a `Task`/`Tasks` keyword on both-side word boundaries, then
/// a `<n>` token, then optionally a `,`/`/`/`+`/`&`/`and`-separated
/// list — plus INCLUSIVE RANGES (`Tasks 1-3` -> `1`, `2`, `3`), which
/// this dialect's Consumes lines use as their dominant plural form and
/// which are unambiguous here because `### Task N:` numbering is flat.
/// A range is expanded only between two FLAT endpoints in ascending
/// order and only up to [`MAX_RANGE_SPAN`] wide; outside that, the
/// element degrades to its LEFT endpoint alone, so a mis-read
/// `Tasks 1-9999` yields one reference rather than thousands of
/// phantom ones.
///
/// Scoped to the Consumes bullet ALONE — never the section's prose,
/// step lines, or code blocks, where "Task 2" appears as ordinary
/// narration. A section with no Consumes bullet, or one reading
/// `nothing (first task)`, yields nothing.
fn consumes_refs(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut refs: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let Some(keyword_len) = task_rows::word_start_boundary(bytes, i).then(|| task_rows::word_at(bytes, i, &TASK_KEYWORDS)).flatten() else {
            i += 1;
            continue;
        };
        // A `Task`/`Tasks` keyword with no number behind it is ordinary
        // prose ("nothing (first task)"): abandon the match and keep
        // scanning from just past the keyword.
        i = scan_task_ref_list(bytes, i + keyword_len, &mut refs).unwrap_or(i + keyword_len);
    }
    refs
}

/// The widest `Tasks <a>-<b>` span [`consumes_refs`] will expand. A
/// plan doc's section count is small by construction (the skill's own
/// task right-sizing), so a wider span is a mis-read number, not a
/// dependency on 64 sections.
const MAX_RANGE_SPAN: u32 = 64;

/// Consume the reference LIST (single, range, or separated list) that
/// must follow a `Task`/`Tasks` keyword, pushing each token onto `refs`.
/// `None` when no reference follows at all; otherwise the byte offset
/// just past the last token consumed.
fn scan_task_ref_list(bytes: &[u8], start: usize, refs: &mut Vec<String>) -> Option<usize> {
    let mut end = scan_one_task_ref(bytes, task_rows::skip_ws(bytes, start), refs)?;
    while let Some(after_sep) = task_rows::list_separator(bytes, end) {
        let Some(next) = scan_one_task_ref(bytes, task_rows::skip_ws(bytes, after_sep), refs) else {
            break;
        };
        end = next;
    }
    Some(end)
}

/// One list element: a `<n>` token, or an inclusive `<a>-<b>` range
/// expanded into every number it covers. Returns the byte offset just
/// past the element, or `None` when no token starts at `i`. A range
/// whose endpoints are not both flat, are descending, or span more than
/// [`MAX_RANGE_SPAN`] contributes its LEFT endpoint alone — the
/// conservative reading, never a guess at what the author meant.
fn scan_one_task_ref(bytes: &[u8], i: usize, refs: &mut Vec<String>) -> Option<usize> {
    let (first, first_end) = task_rows::number_token_at(bytes, i)?;
    let Some(after_dash) = range_dash(bytes, first_end) else {
        refs.push(first);
        return Some(first_end);
    };
    let Some((last, last_end)) = task_rows::number_token_at(bytes, after_dash) else {
        refs.push(first);
        return Some(first_end);
    };
    match (first.parse::<u32>(), last.parse::<u32>()) {
        (Ok(lo), Ok(hi)) if hi >= lo && hi - lo <= MAX_RANGE_SPAN => {
            for n in lo..=hi {
                refs.push(n.to_string());
            }
            Some(last_end)
        }
        _ => {
            refs.push(first);
            Some(first_end)
        }
    }
}

/// The range dash between two `Tasks <a>-<b>` endpoints — ASCII `-` or
/// the en dash `–` the corpus's prose also uses — returning the byte
/// offset just past it. Deliberately NOT whitespace-tolerant: a
/// spaced `1 - 3` in prose is far more likely a dash in a sentence than
/// a range, and a spaced form appears nowhere in the corpus.
fn range_dash(bytes: &[u8], i: usize) -> Option<usize> {
    if bytes.get(i) == Some(&b'-') {
        return Some(i + 1);
    }
    bytes[i..].starts_with(EN_DASH.as_bytes()).then_some(i + EN_DASH.len())
}

/// The en dash (`–`, 3 bytes in UTF-8) — a constant rather than a byte
/// comparison for the same multi-byte reason [`task_rows`]'s lexer is
/// byte-wise.
const EN_DASH: &str = "–";

/// One attempted `### Task N: <name>` section (design D3): the heading
/// SHAPE matched ([`task_heading`]); `n_token`'s own grammar validity
/// is checked by the caller. `done`/`open` are this section's own
/// checkbox-STEP tallies ([`checkbox_state`]) — never leaked into a
/// sibling section, since a heading line (any level) always closes the
/// current one first. `consumes` is this section's own `- Consumes:`
/// bullet text (s37 `execution-graph-topology`), the FIRST one if a
/// document repeats the label, and `None` when the section has no
/// `**Interfaces:**` block at all — scoped per section for the same
/// reason the tallies are.
struct AttemptedSection {
    n_token: String,
    name: String,
    done: usize,
    open: usize,
    consumes: Option<String>,
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
                current = Some(AttemptedSection { n_token: n_token.to_string(), name, done: 0, open: 0, consumes: None });
            }
            continue;
        }

        if let Some(consumes) = consumes_line(line) {
            if let Some(section) = current.as_mut() {
                if section.consumes.is_none() {
                    section.consumes = Some(consumes.to_string());
                }
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
    // Each emitted task's index paired with its section's `- Consumes:`
    // text, resolved only AFTER the loop (s37 `execution-graph-
    // topology`): a Consumes bullet may cite any section, and
    // `seen_numbers` — the set membership in which IS the validation —
    // is not complete until every section has been accepted or skipped.
    let mut pending_deps: Vec<(usize, String)> = Vec::new();
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
        if let Some(consumes) = section.consumes {
            pending_deps.push((tasks.len(), consumes));
        }
        let envelope = Envelope::current(RecordKind::Task, at, actor());
        tasks.push(Task::new(envelope, task_id, section.name, status, None));
    }

    for (idx, consumes) in pending_deps {
        let task_id = tasks[idx].task_id.clone();
        let (depends_on, unresolvable) = task_rows::resolve_declared_deps(&change_id, &task_id, &consumes_refs(&consumes), &seen_numbers);
        for _ in &unresolvable {
            // One Consumes reference naming no section this document has
            // — dropped, counted against THIS section's task_id, never
            // an import failure (s37: the expression is prose, so a
            // mis-read candidate must fail soft and stay visible).
            outcome.record_unmapped(&format!("{DIAG_UNRESOLVABLE_TASK_DEP}:{}", task_id.as_str()));
        }
        tasks[idx].depends_on = depends_on;
    }

    // No archive convention (design D4): `derive_status` shared
    // verbatim with the openspec dialect, `archived: false` always.
    let status = derive_status(false, done_count, open_count);
    let envelope = Envelope::current(RecordKind::Change, at, actor());
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

    /// `s38-evidence-bearing-memory`: this dialect gained `- Consumes:
    /// … from Task <n>` dependency extraction, so its parse output for
    /// an IDENTICAL plan doc changed and its generation must be past
    /// `1` — otherwise `canon-cli::plans`'s cursor id is unchanged and
    /// `canon ingest plans` reports `skipped unchanged` instead of
    /// re-parsing.
    #[test]
    fn parse_version_is_two_because_dependency_extraction_changed_this_dialects_output() {
        assert_eq!(SuperpowersPlanAdapter.parse_version(), 2);
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

    // ── `- Consumes:` -> Task.depends_on (s37 execution-graph-topology) ──

    fn dep_ids(task: &Task) -> Vec<&str> {
        task.depends_on.iter().map(|t| t.as_str()).collect()
    }

    /// The corpus's own Consumes shapes, verbatim: `nothing (first
    /// task).`, `… from Task 1.`, `… (Task 2).`, `… (Tasks 2-3).` —
    /// including the inclusive-range expansion that is this dialect's
    /// dominant plural form.
    #[test]
    fn a_consumes_bullet_populates_depends_on_including_an_inclusive_range() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(
            tmp.path(),
            "2026-07-14-consumes.md",
            "# Consumes Plan\n\n**Goal:** Prove Consumes-line dependency extraction.\n\n\
             ### Task 1: Assets\n\n**Interfaces:**\n- Consumes: nothing (first task).\n- Produces: `loadTextures()`.\n\n- [x] **Step 1:** wire it\n\n\
             ### Task 2: Simulation\n\n**Interfaces:**\n- Consumes: `loadTextures()`, `ASSET_DEFS` (Task 1).\n\n- [x] **Step 1:** wire it\n\n\
             ### Task 3: Enemies\n\n**Interfaces:**\n- Consumes: `GameSimulation`, `RenderSnapshot` (Task 2).\n\n- [ ] **Step 1:** not yet\n\n\
             ### Task 4: Platforms\n\n**Interfaces:**\n- Consumes: `resolveAxis`'s `extraSolids` parameter (Tasks 2-3).\n\n- [ ] **Step 1:** not yet\n\n\
             ### Task 5: Smoke test\n\n**Interfaces:**\n- Consumes: the fully assembled app from Tasks 1-4. Produces nothing new.\n\n- [ ] **Step 1:** not yet\n",
        );

        let outcome = parse_root(tmp.path());
        assert!(dep_ids(find_task(&outcome, "2026-07-14-consumes#1")).is_empty(), "`nothing (first task)` declares nothing");
        assert_eq!(dep_ids(find_task(&outcome, "2026-07-14-consumes#2")), vec!["2026-07-14-consumes#1"]);
        assert_eq!(dep_ids(find_task(&outcome, "2026-07-14-consumes#3")), vec!["2026-07-14-consumes#2"]);
        assert_eq!(
            dep_ids(find_task(&outcome, "2026-07-14-consumes#4")),
            vec!["2026-07-14-consumes#2", "2026-07-14-consumes#3"],
            "`Tasks 2-3` expands inclusively"
        );
        assert_eq!(
            dep_ids(find_task(&outcome, "2026-07-14-consumes#5")),
            vec!["2026-07-14-consumes#1", "2026-07-14-consumes#2", "2026-07-14-consumes#3", "2026-07-14-consumes#4"],
            "`Tasks 1-4` expands inclusively, in ascending order, deduplicated"
        );
        assert!(outcome.unmapped.keys().all(|k| !k.starts_with(DIAG_UNRESOLVABLE_TASK_DEP)), "every reference resolved: {:?}", outcome.unmapped);
    }

    #[test]
    fn a_section_with_no_consumes_bullet_has_an_empty_depends_on_and_no_diagnostic() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(
            tmp.path(),
            "2026-07-14-no-interfaces.md",
            "# No Interfaces Plan\n\n**Goal:** A plan doc with no Interfaces block at all.\n\n\
             ### Task 1: Adapter\n- [x] **Step 1:** wire it up\n\n\
             ### Task 2: Docs\nProse mentioning Task 1 outside any Consumes bullet.\n- [ ] **Step 1:** draft\n",
        );

        let outcome = parse_root(tmp.path());
        assert!(dep_ids(find_task(&outcome, "2026-07-14-no-interfaces#1")).is_empty());
        assert!(
            dep_ids(find_task(&outcome, "2026-07-14-no-interfaces#2")).is_empty(),
            "`Task 1` in ordinary section prose is narration, not a declaration — extraction is scoped to the Consumes bullet alone"
        );
        assert!(outcome.unmapped.keys().all(|k| !k.starts_with(DIAG_UNRESOLVABLE_TASK_DEP)), "no reference read means no diagnostic: {:?}", outcome.unmapped);
    }

    #[test]
    fn an_unresolvable_consumes_reference_is_dropped_and_counted_without_sinking_the_section() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(
            tmp.path(),
            "2026-07-14-bad-consumes.md",
            "# Bad Consumes Plan\n\n**Goal:** Prove an unresolvable Consumes reference fails soft.\n\n\
             ### Task 1: Adapter\n\n**Interfaces:**\n- Consumes: `helper()` from Task 1 and Task 9.\n\n- [x] **Step 1:** wire it up\n",
        );

        let outcome = parse_root(tmp.path());
        let task = find_task(&outcome, "2026-07-14-bad-consumes#1");
        assert!(dep_ids(task).is_empty(), "`Task 1` is this section's own number (dropped silently) and `Task 9` names no section");
        assert_eq!(task.status, TaskStatus::Done, "the section itself still imports — a dropped reference is never an import failure");
        assert_eq!(
            outcome.unmapped.get(&format!("{DIAG_UNRESOLVABLE_TASK_DEP}:2026-07-14-bad-consumes#1")),
            Some(&1),
            "exactly ONE diagnostic: the self-reference is not a mistake, only `Task 9` is"
        );
        assert!(outcome.malformed.is_empty(), "an unresolvable reference is an `unmapped` diagnostic, never a `malformed` construct: {:?}", outcome.malformed);
    }

    // ── consumes_line / consumes_refs: the bullet grammar itself ──

    #[test]
    fn consumes_line_recognizes_only_the_consumes_bullet() {
        assert_eq!(consumes_line("- Consumes: `loadTextures()` (Task 1)."), Some("`loadTextures()` (Task 1)."));
        assert_eq!(consumes_line("  - consumes: lowercase label"), Some("lowercase label"));
        assert_eq!(consumes_line("* Consumes: a star bullet"), Some("a star bullet"));
        assert_eq!(consumes_line("- Produces: `TILE`"), None);
        assert_eq!(consumes_line("- [x] **Step 1:** a checkbox step, never a Consumes bullet"), None);
        assert_eq!(consumes_line("Consumes: not a list item at all"), None);
        assert_eq!(consumes_line("- ✅"), None, "a short multi-byte bullet must not panic on the label-length slice");
    }

    #[test]
    fn consumes_refs_reads_the_corpus_shapes_and_declines_the_rest() {
        assert!(consumes_refs("nothing (first task).").is_empty());
        assert_eq!(consumes_refs("`CANVAS_W`, `keys`, `update(dt)` from Task 1."), vec!["1"]);
        assert_eq!(consumes_refs("`loadTextures()`, `ASSET_DEFS` (Task 1)."), vec!["1"]);
        assert_eq!(consumes_refs("everything produced by Tasks 1-3 (`update`, `render`)."), vec!["1", "2", "3"]);
        assert_eq!(consumes_refs("`HudSnapshot`, `LEVELS` (Tasks 2, 3 and 4)."), vec!["2", "3", "4"]);
        assert_eq!(consumes_refs("`GameSimulation` (Tasks 2–4)."), vec!["2", "3", "4"], "the en dash the corpus's prose also uses");
        assert_eq!(consumes_refs("the fully assembled app from Tasks 1-5. Produces nothing new."), vec!["1", "2", "3", "4", "5"]);
        assert_eq!(consumes_refs("Tasks 1-9999 is a mis-read number"), vec!["1"], "a span wider than MAX_RANGE_SPAN degrades to its left endpoint, never thousands of phantom refs");
        assert_eq!(consumes_refs("Tasks 4-2 is descending"), vec!["4"], "a descending range degrades to its left endpoint");
    }

    /// s37 review finding, pinned against the corpus document VERBATIM.
    ///
    /// A live-store read reported `2026-07-14-red-panda-ridge-v2` tasks
    /// `#2`/`#3`/`#4` as unpopulated while `#5`/`#6` — structurally the
    /// SAME `(Tasks A-B)` shape — were populated, with zero
    /// `unresolvable-task-dep` diagnostics. The parser was never at
    /// fault: re-importing the real document into a fresh tier
    /// populates all six correctly. The split came from the READ path —
    /// a superpowers `Task`'s `at` is the plan doc's mtime, which a
    /// canon CODE change does not advance, so a pre-s37 record and its
    /// post-s37 replacement carried an IDENTICAL `at` and
    /// `canon_store::fold::fold_latest_by_key` had only a LEXICOGRAPHIC
    /// DIGEST to decide between them, which is arbitrary per row.
    ///
    /// `s38-evidence-bearing-memory` closed both halves:
    /// [`SuperpowersPlanAdapter::parse_version`] forces the re-parse the
    /// content-digest cursor could not detect, and the fold now orders
    /// an equal-`at` tie by `Envelope.schema` (`Task` is generation `2`)
    /// before it ever reaches the digest.
    ///
    /// Every `- Consumes:` line below is copied byte-for-byte from
    /// `docs/superpowers/plans/2026-07-14-red-panda-ridge-v2.md`
    /// (lines 37, 281, 870, 1014, 1233, 1495), as are the six
    /// `### Task N:` headings and the `**Interfaces:**`/`**Files:**`
    /// block structure around them. The intervening step bodies (which
    /// in the real document run to hundreds of lines each) are elided —
    /// they carry no Consumes bullet, and section scope is asserted
    /// independently by `a_section_with_no_consumes_bullet_…` above.
    #[test]
    fn the_verbatim_ridge_v2_consumes_lines_populate_the_whole_dependency_chain() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(
            tmp.path(),
            "2026-07-14-red-panda-ridge-v2.md",
            "# Red Panda Ridge v2 Implementation Plan\n\
             \n\
             **Goal:** Rebuild the platformer on Vite/React/Pixi with three levels.\n\
             \n\
             ### Task 1: Vite/React/Pixi scaffold and asset pipeline\n\
             \n\
             **Files:**\n\
             - Create: `examples/platformer/src/render/assets.ts`\n\
             \n\
             **Interfaces:**\n\
             - Consumes: nothing (first task). Assumes all 8 sprite PNGs already exist at `examples/platformer/assets/{red-panda,squirrel,acorn,tile,background,pinecone,heart,platform}.png` (5 pre-existing, 3 new — produced by the art workstream in parallel; this task only wires the loader against those exact filenames).\n\
             - Produces:\n\
             \x20 - `ASSET_DEFS` in `src/render/assets.ts`.\n\
             \n\
             - [x] **Step 1:** wire the loader\n\
             \n\
             ### Task 2: Engine port — physics, level, camera parity with v1\n\
             \n\
             **Files:**\n\
             - Modify: `examples/platformer/src/render/PixiStage.tsx`\n\
             \n\
             **Interfaces:**\n\
             - Consumes: `loadTextures()`, `ASSET_DEFS` (Task 1).\n\
             - Produces:\n\
             \x20 - `GameSimulation` in `simulation.ts`.\n\
             \n\
             - [x] **Step 1:** port the physics\n\
             \n\
             ### Task 3: Pinecone enemies, hearts, and game over\n\
             \n\
             **Files:**\n\
             - Modify: `examples/platformer/src/engine/simulation.ts`\n\
             \n\
             **Interfaces:**\n\
             - Consumes: `PlayerState`, `LevelData`, `aabbOverlap`, `GameSimulation`, `RenderSnapshot`, `HudSnapshot` (Task 2).\n\
             - Produces:\n\
             \x20 - `EnemyState` in `types.ts`.\n\
             \n\
             - [ ] **Step 1:** add the enemies\n\
             \n\
             ### Task 4: Moving platforms, 3-level data, and progression\n\
             \n\
             **Files:**\n\
             - Modify: `examples/platformer/src/engine/level.ts` (add `LEVELS: LevelData[]`)\n\
             \n\
             **Interfaces:**\n\
             - Consumes: `resolveAxis`'s `extraSolids` parameter, `updatePlayer`'s `extraSolids`/return value, `GameSimulation`, `HudSnapshot`, `RenderSnapshot` (Tasks 2-3).\n\
             - Produces:\n\
             \x20 - `MovingPlatformState` in `types.ts`.\n\
             \n\
             - [ ] **Step 1:** add the platforms\n\
             \n\
             ### Task 5: React HUD, menus, and localStorage records\n\
             \n\
             **Files:**\n\
             - Modify: `examples/platformer/src/App.tsx`\n\
             \n\
             **Interfaces:**\n\
             - Consumes: `GameSimulation` (constructor, `setPaused`, `restartLevel`, `goToLevel`), `useHudSnapshot`, `HudSnapshot` (all fields), `LEVELS` (Tasks 2-4).\n\
             - Produces:\n\
             \x20 - `LevelRecord` in `records.ts`.\n\
             \n\
             - [ ] **Step 1:** build the HUD\n\
             \n\
             ### Task 6: End-to-end smoke test\n\
             \n\
             **Files:**\n\
             - Test: manual smoke pass\n\
             \n\
             **Interfaces:**\n\
             - Consumes: the fully assembled app from Tasks 1-5. Produces nothing new — this task only verifies.\n\
             \n\
             - [ ] **Step 1:** play it through\n",
        );

        let outcome = parse_root(tmp.path());
        let id = |n: u32| format!("2026-07-14-red-panda-ridge-v2#{n}");

        assert!(
            dep_ids(find_task(&outcome, &id(1))).is_empty(),
            "`nothing (first task)` declares nothing — and neither `this task only wires the loader` nor the 8/5/3 counts fabricate a reference"
        );
        assert_eq!(dep_ids(find_task(&outcome, &id(2))), vec![id(1)], "`(Task 1).` — reported unpopulated by the live-store read");
        assert_eq!(dep_ids(find_task(&outcome, &id(3))), vec![id(2)], "`(Task 2).` — reported unpopulated by the live-store read");
        assert_eq!(dep_ids(find_task(&outcome, &id(4))), vec![id(2), id(3)], "`(Tasks 2-3).` — reported unpopulated by the live-store read");
        assert_eq!(dep_ids(find_task(&outcome, &id(5))), vec![id(2), id(3), id(4)], "`(Tasks 2-4).`");
        assert_eq!(dep_ids(find_task(&outcome, &id(6))), vec![id(1), id(2), id(3), id(4), id(5)], "`from Tasks 1-5.`");
        assert!(
            outcome.unmapped.keys().all(|k| !k.starts_with(DIAG_UNRESOLVABLE_TASK_DEP)),
            "every reference in the real document resolves: {:?}",
            outcome.unmapped
        );
        assert!(outcome.malformed.is_empty(), "the real document imports clean: {:?}", outcome.malformed);
    }
}
