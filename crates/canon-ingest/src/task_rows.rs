//! Canon's own `tasks.md`/plan checkbox ROW GRAMMAR — the single,
//! dialect-neutral reader + writer for a checkbox row's on-disk shape
//! (s35 `gate-plan-dialect-seam`, design D2: grammar unification).
//!
//! # One grammar, formerly two
//! Before s35 this exact row shape had TWO independent implementations:
//! `canon-gate::checkbox` (the gate crate's own reader/writer, framed
//! as "the openspec checkbox grammar") and `canon-ingest::openspec_rows`
//! (a READ-ONLY mirror the S4 verdict adapter + the s17 plan adapters
//! read). s35 collapses them into THIS one module: canon-gate sheds all
//! markdown/dialect knowledge (it keeps only the pure evidence
//! decision, `canon_gate::gate_task`), and the WRITER — the stricter,
//! byte-identical round-tripping [`format_line`]/[`TaskRow::covers_raw`]
//! semantics the gate crate used to own — lands here as the canonical
//! version. There is exactly one `TaskRow`, one [`parse_line`], one
//! [`format_line`], and one join-key derivation ([`task_id_for`]) for
//! every reader/writer in the workspace.
//!
//! # Dialect-neutral: this is canon's row format, not openspec's
//! The row shape (`- [ ] `/`- [x] ` + an optional `**DEFERRED to §<to>**`
//! /`**DROPPED**` annotation immediately after the id token + a checked
//! row's ` — ✅ <evidence>` suffix + an optional trailing `[covers: …]`
//! segment, design s20 Decision 2) is canon's OWN authoritative format,
//! never a best-effort reader of every legacy shape. A dialect
//! (`crate::plan_adapters::openspec`) owns only WHERE a task's rows live
//! on disk (directory layout, `crate::plan_writeback::PlanWriteBack::
//! locate_task`); the row grammar itself is shared. A row outside this
//! canonical shape is simply not recognized ([`parse_line`] returns
//! `None`) rather than guessed at — "malformed input becomes absence,
//! never a crash or a silent guess".
//!
//! # Round-tripping is byte-identical for any recognized row
//! [`parse_line`]/[`format_line`] round-trip byte-identically for any
//! row [`format_line`] itself could have produced (spec.md "Round-
//! tripping every recognized row shape"). [`TaskRow::covers_raw`]
//! captures a recognized `[covers: …]` segment VERBATIM so a rewrite
//! (the evidence-gated flip, `PlanWriteBack::flip_task`) touches ONLY
//! the checked-state and the evidence suffix — never title/covers
//! content, even when the covers segment mixes a malformed token
//! between well-formed ones (s20 Decision 2 review finding).
//!
//! # Scope: one grammar, several consumers
//! The S4 verdict adapter (`artifact_adapters::openspec_task`) emits an
//! `ArtifactEvent` only for a FLIPPED/annotated row; s17's plan adapters
//! (`plan_adapters::{openspec,superpowers}`) emit a `Task` candidate for
//! EVERY row regardless of state; the s35 write-back
//! (`plan_writeback::PlanWriteBack::flip_task`) rewrites exactly one
//! row. All read/write the SAME [`parse_line`]/[`format_line`]/
//! [`task_id_for`] below — one grammar, several consumers.

use canon_model::ids::{ChangeId, ScenarioId, TaskId};

const DROPPED_MARKER: &str = "**DROPPED**";
const DEFERRED_PREFIX: &str = "**DEFERRED to §";
const DEFERRED_SUFFIX: &str = "**";
const EVIDENCE_MARKER: &str = " — ✅ ";
/// The `[covers: <scenario_id>[, <scenario_id>]*]` trailing segment's
/// opening marker (design s20 Decision 2) — deliberately no trailing
/// space in the constant itself so [`extract_covers`] can `rfind` it
/// against text that may have arbitrary internal spacing before the
/// first scenario id token.
const COVERS_PREFIX: &str = "[covers:";

/// A row's scheduling annotation, immediately after the row's id token
/// — `**DROPPED**` or `**DEFERRED to §<to>**`. `None` for a plain row
/// with no annotation. An annotated row is exempt from the "must be
/// `[x]`" gate regardless of its own checkbox state (design decision 6,
/// s5) — but THIS module only recognizes the grammar; the enumerator
/// that consumes the exemption lives in a caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Annotation {
    /// `**DEFERRED to §<to>**` — `to` is the target section/task
    /// reference verbatim (e.g. `"4.2"`), never parsed further here.
    Deferred { to: String },
    /// `**DROPPED**` — no rationale field; the row's own `title` text
    /// carries the rationale prose, unmodeled by this grammar.
    Dropped,
}

impl Annotation {
    /// Reconstruct this annotation's canonical on-disk marker text
    /// (`**DEFERRED to §<to>**` / `**DROPPED**`) — the s17 plan adapters
    /// use this to carry the scheduling annotation verbatim into
    /// `Task.evidence_note` when no ` — ✅ ` evidence suffix is present
    /// (design "Dialect -> RecordKind mapping" table). Routed through
    /// the same constants [`parse_annotation`] itself matches against,
    /// so the marker text can never drift from what this module
    /// recognizes. [`format_line`] emits the annotation the same way.
    pub fn marker_text(&self) -> String {
        match self {
            Annotation::Deferred { to } => format!("{DEFERRED_PREFIX}{to}{DEFERRED_SUFFIX}"),
            Annotation::Dropped => DROPPED_MARKER.to_string(),
        }
    }
}

/// One parsed `tasks.md` checkbox row, OWNED (design s35 D2: the single
/// grammar carries the writer, so the row it parses must be mutable +
/// re-emittable, not a borrow of the source line). Distinct from
/// [`canon_model::Task`], the S1 envelope-wrapped record an ingest
/// adapter derives FROM a row like this one — this type is the raw
/// markdown-grammar layer underneath that adapter, not a replacement
/// for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRow {
    /// Leading whitespace before `- [`, preserved verbatim for
    /// byte-identical round-tripping of nested/indented rows.
    pub indent: String,
    /// The row's own id token (e.g. `"3.2"`) — the `<n>` half of a
    /// join-spine [`TaskId`] (`<change_id>#<n>`); the change half is
    /// never present in the row text itself, only in which file it
    /// lives under.
    pub id: String,
    /// `true` for `- [x] `, `false` for `- [ ] `.
    pub checked: bool,
    /// The row's scheduling annotation, when present.
    pub annotation: Option<Annotation>,
    /// The row's title text (annotation + covers + evidence stripped).
    pub title: String,
    /// Declared scenario-coverage references parsed from a trailing
    /// `[covers: …]` segment (design s20 Decision 1/2) — empty when no
    /// such segment is present. A malformed individual token is never
    /// represented here; see [`TaskRow::malformed_scenario_refs`].
    pub scenario_refs: Vec<ScenarioId>,
    /// Raw `[covers: …]` tokens that failed [`ScenarioId::parse`],
    /// dropped from [`TaskRow::scenario_refs`] but preserved here so a
    /// caller can surface a NAMED `malformed-scenario-ref` diagnostic
    /// scoped to this row (design s20 Decision 2) — the row's other
    /// well-formed refs, and the row itself, still parse successfully.
    /// [`format_line`] never reconstructs the bracket from THIS list
    /// alone (see [`TaskRow::covers_raw`]) — it exists for a caller's
    /// diagnostic bookkeeping, not for round-tripping.
    pub malformed_scenario_refs: Vec<String>,
    /// The row's ORIGINAL `[covers: …]` bracket text, byte-for-byte
    /// (from `[covers:` through the closing `]`), captured whenever
    /// [`parse_line`] recognized a covers segment — regardless of
    /// whether every token inside parsed cleanly. [`format_line`]
    /// re-emits this verbatim instead of reconstructing the bracket
    /// from [`TaskRow::scenario_refs`] alone, so a malformed token
    /// (design s20 Decision 2) is never silently dropped on rewrite —
    /// the flip's own contract is to touch only checked-state and the
    /// evidence suffix, never title/covers content. `None` when no
    /// covers segment was recognized at all. A `TaskRow` built by hand
    /// rather than via [`parse_line`] can leave this `None` even with
    /// `scenario_refs` populated — [`format_line`] then falls back to
    /// reconstructing the well-formed-only bracket via [`push_covers`].
    pub covers_raw: Option<String>,
    /// The ` — ✅ <evidence>` suffix's content, when present. Only ever
    /// populated when `checked` — [`format_line`] does not gate on this
    /// itself (a caller COULD construct an inconsistent row), but every
    /// row a flip produces keeps the two consistent.
    pub evidence: Option<String>,
}

fn split_first_token(s: &str) -> Option<(&str, &str)> {
    if s.is_empty() {
        return None;
    }
    match s.find(' ') {
        Some(i) => Some((&s[..i], &s[i + 1..])),
        None => Some((s, "")),
    }
}

fn parse_annotation(rest: &str) -> (Option<Annotation>, &str) {
    if let Some(after) = rest.strip_prefix(DROPPED_MARKER) {
        return (Some(Annotation::Dropped), after.strip_prefix(' ').unwrap_or(after));
    }
    if let Some(after_prefix) = rest.strip_prefix(DEFERRED_PREFIX) {
        if let Some(close_idx) = after_prefix.find(DEFERRED_SUFFIX) {
            let to = &after_prefix[..close_idx];
            let after = &after_prefix[close_idx + DEFERRED_SUFFIX.len()..];
            if !to.is_empty() {
                return (Some(Annotation::Deferred { to: to.to_string() }), after.strip_prefix(' ').unwrap_or(after));
            }
        }
    }
    (None, rest)
}

fn push_annotation(s: &mut String, ann: &Annotation) {
    match ann {
        Annotation::Dropped => s.push_str(DROPPED_MARKER),
        Annotation::Deferred { to } => {
            s.push_str(DEFERRED_PREFIX);
            s.push_str(to);
            s.push_str(DEFERRED_SUFFIX);
        }
    }
}

/// Extract a trailing `[covers: <scenario_id>[, <scenario_id>]*]`
/// segment from `text` (title text with any evidence suffix already
/// stripped) — design s20 Decision 2. Returns the remaining title text
/// (unchanged when no segment is recognized), the well-formed
/// `ScenarioId`s found, the raw tokens that failed
/// [`ScenarioId::parse`] (dropped from the first list, never sinking
/// the row), and the ORIGINAL bracket text byte-for-byte (`Some` iff a
/// segment was recognized at all — see [`TaskRow::covers_raw`]). A
/// bracket that is unbalanced, empty, or contains a nested `[`/`]` is
/// NOT recognized at all — left as ordinary title prose, never
/// partially guessed at.
fn extract_covers(text: &str) -> (String, Vec<ScenarioId>, Vec<String>, Option<String>) {
    let trimmed = text.trim_end();
    let unrecognized = || (text.to_string(), Vec::new(), Vec::new(), None);
    if !trimmed.ends_with(']') {
        return unrecognized();
    }
    let Some(start) = trimmed.rfind(COVERS_PREFIX) else {
        return unrecognized();
    };
    let inner = &trimmed[start + COVERS_PREFIX.len()..trimmed.len() - 1];
    if inner.contains('[') || inner.contains(']') {
        return unrecognized();
    }
    let content = inner.trim();
    if content.is_empty() {
        return unrecognized();
    }
    let mut refs = Vec::new();
    let mut malformed = Vec::new();
    for token in content.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        match ScenarioId::parse(token) {
            Ok(id) => refs.push(id),
            Err(_) => malformed.push(token.to_string()),
        }
    }
    let raw = trimmed[start..].to_string();
    (trimmed[..start].trim_end().to_string(), refs, malformed, Some(raw))
}

/// Write a `[covers: …]` segment for `refs` — the FALLBACK
/// reconstruction [`format_line`] uses only when a [`TaskRow`] carries
/// no [`TaskRow::covers_raw`] (i.e. was built by hand, never via
/// [`parse_line`]); malformed tokens are never reconstructed here,
/// since a hand-built row has no raw malformed text to reconstruct
/// FROM in the first place.
fn push_covers(s: &mut String, refs: &[ScenarioId]) {
    s.push_str(COVERS_PREFIX);
    s.push(' ');
    for (i, id) in refs.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(id.as_str());
    }
    s.push(']');
}

/// Parse one `tasks.md` LINE (no trailing `\n`) as a checkbox row —
/// `None` when the line does not match this module's canonical shape
/// (module doc: not every historical row shape is recognized, only
/// canon's own; an unrecognized line is text to a caller, never a
/// crash). Preserves the leading indent verbatim so [`format_line`]
/// round-trips a nested/indented row byte-identically.
pub fn parse_line(line: &str) -> Option<TaskRow> {
    let indent_len = line.len() - line.trim_start_matches(' ').len();
    let (indent, after_indent) = line.split_at(indent_len);
    let after_bracket = after_indent.strip_prefix("- [")?;
    let mark = after_bracket.chars().next()?;
    let checked = match mark {
        ' ' => false,
        'x' | 'X' => true,
        _ => return None,
    };
    let rest = &after_bracket[mark.len_utf8()..];
    let rest = rest.strip_prefix("] ")?;
    let (id, rest) = split_first_token(rest)?;
    if id.is_empty() {
        return None;
    }
    let (annotation, rest) = parse_annotation(rest);
    let (title_and_covers, evidence) = if checked {
        match rest.find(EVIDENCE_MARKER) {
            Some(i) => (&rest[..i], Some(rest[i + EVIDENCE_MARKER.len()..].to_string())),
            None => (rest, None),
        }
    } else {
        (rest, None)
    };
    let (title, scenario_refs, malformed_scenario_refs, covers_raw) = extract_covers(title_and_covers);
    Some(TaskRow { indent: indent.to_string(), id: id.to_string(), checked, annotation, title, scenario_refs, malformed_scenario_refs, covers_raw, evidence })
}

/// Write one [`TaskRow`] back to its canonical line — the exact inverse
/// of [`parse_line`] for any row this module recognizes (spec.md
/// "Round-tripping every recognized row shape").
pub fn format_line(row: &TaskRow) -> String {
    let mut s = String::new();
    s.push_str(&row.indent);
    s.push_str("- [");
    s.push(if row.checked { 'x' } else { ' ' });
    s.push_str("] ");
    s.push_str(&row.id);
    let has_covers = row.covers_raw.is_some() || !row.scenario_refs.is_empty();
    if row.annotation.is_some() || !row.title.is_empty() || has_covers || row.evidence.is_some() {
        s.push(' ');
        if let Some(ann) = &row.annotation {
            push_annotation(&mut s, ann);
            if !row.title.is_empty() || has_covers || row.evidence.is_some() {
                s.push(' ');
            }
        }
        s.push_str(&row.title);
        if has_covers {
            if !row.title.is_empty() {
                s.push(' ');
            }
            // `covers_raw` (verbatim, malformed tokens included) wins
            // whenever present — only a hand-built `TaskRow` with no
            // captured raw text falls back to reconstructing a
            // well-formed-only bracket (s20 review: the flip's own
            // contract is to touch only checked-state and the evidence
            // suffix, never title/covers content, so a row read via
            // parse_line must round-trip its covers segment
            // byte-identically even when it mixes malformed tokens with
            // well-formed ones).
            match &row.covers_raw {
                Some(raw) => s.push_str(raw),
                None => push_covers(&mut s, &row.scenario_refs),
            }
        }
        if let Some(evidence) = &row.evidence {
            s.push_str(EVIDENCE_MARKER);
            s.push_str(evidence);
        }
    }
    s
}

/// `<n>` grammar (`TaskId`'s own: one or more dot-separated integers)
/// — validated locally rather than round-tripped through
/// `TaskId::parse` twice; [`task_id_for`] still routes the final
/// `<change_id>#<n>` string through `TaskId::parse` so the S1 grammar
/// stays the single source of truth for the FULL key.
pub fn is_task_number(token: &str) -> bool {
    !token.is_empty() && token.split('.').all(|seg| !seg.is_empty() && seg.bytes().all(|b| b.is_ascii_digit()))
}

/// Derive a row's full `task_id` (`<change_id>#<n>`, S1 join spine) —
/// `None` when `row_id` fails the `<n>` grammar ([`is_task_number`]) or
/// the combined `<change_id>#<row_id>` string fails `TaskId::parse`
/// (structurally unreachable once `change_id` is itself a valid
/// [`ChangeId`] and `row_id` passes [`is_task_number`], but the parse
/// still runs so `TaskId`'s grammar stays the single source of truth
/// for the full key rather than being re-derived here). Every
/// consumer's own "row id token isn't even a valid number, or the
/// composed key doesn't parse" malformed-row handling routes through
/// this one function so the readers never drift.
pub fn task_id_for(change_id: &ChangeId, row_id: &str) -> Option<TaskId> {
    if !is_task_number(row_id) {
        return None;
    }
    TaskId::parse(format!("{}#{}", change_id.as_str(), row_id)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_line: base shapes ──

    #[test]
    fn parse_line_reads_a_checked_row_with_evidence() {
        let row = parse_line("- [x] 1.1 Implement the checkbox parser — ✅ https://example.com/org/repo/pull/482 merged").unwrap();
        assert_eq!(row.id, "1.1");
        assert!(row.checked);
        assert_eq!(row.annotation, None);
        assert_eq!(row.title, "Implement the checkbox parser");
        assert_eq!(row.evidence.as_deref(), Some("https://example.com/org/repo/pull/482 merged"));
    }

    #[test]
    fn parse_line_reads_a_checked_row_without_the_evidence_marker() {
        let row = parse_line("- [x] 2.1 Ship the thing").unwrap();
        assert_eq!(row.id, "2.1");
        assert!(row.checked);
        assert_eq!(row.title, "Ship the thing");
        assert_eq!(row.evidence, None, "no ` — ✅ ` marker present, so no evidence — never a crash");
    }

    #[test]
    fn parse_line_reads_an_untouched_open_row() {
        let row = parse_line("- [ ] 1.5 Not started yet").unwrap();
        assert_eq!(row.id, "1.5");
        assert!(!row.checked);
        assert_eq!(row.annotation, None);
        assert_eq!(row.title, "Not started yet");
        assert_eq!(row.evidence, None);
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
        assert!(parse_line("## 1. A heading").is_none());
        assert!(parse_line("").is_none());
        assert!(parse_line("some prose about the fixture").is_none());
    }

    #[test]
    fn parse_line_returns_none_for_a_malformed_bracket() {
        assert!(parse_line("- [z] 1.1 not a real checkbox state").is_none());
    }

    #[test]
    fn parse_line_returns_none_when_the_id_token_is_missing() {
        assert!(parse_line("- [ ] ").is_none());
        assert!(parse_line("- [ ]").is_none());
    }

    #[test]
    fn parse_line_tolerates_a_non_numeric_id_token() {
        // The base row shape doesn't itself demand a numeric id — that
        // grammar is `is_task_number`'s job, checked by a consumer
        // deriving `task_id`, not by `parse_line` (malformed evidence is
        // no evidence, never a crash at the row-parsing layer).
        let row = parse_line("- [ ] not-a-number Some title").unwrap();
        assert_eq!(row.id, "not-a-number");
    }

    // ── format_line round-trip (spec.md "Round-tripping every recognized row shape") ──

    #[test]
    fn round_trips_an_open_checkbox() {
        let line = "- [ ] 3.1 Implement the checkbox grammar parser + writer";
        let row = parse_line(line).expect("recognized open row");
        assert!(!row.checked);
        assert_eq!(row.id, "3.1");
        assert_eq!(row.annotation, None);
        assert_eq!(format_line(&row), line);
    }

    #[test]
    fn round_trips_a_flipped_checkbox_with_evidence_suffix() {
        let line = "- [x] 3.2 Implement canon gate task — ✅ cargo test -p canon-gate: 40 passed";
        let row = parse_line(line).expect("recognized done row");
        assert!(row.checked);
        assert_eq!(row.title, "Implement canon gate task");
        assert_eq!(row.evidence.as_deref(), Some("cargo test -p canon-gate: 40 passed"));
        assert_eq!(format_line(&row), line);
    }

    #[test]
    fn round_trips_a_deferred_row() {
        let line = "- [ ] 3.5 **DEFERRED to §4.2** Ship the CLI wiring";
        let row = parse_line(line).expect("recognized deferred row");
        assert_eq!(row.annotation, Some(Annotation::Deferred { to: "4.2".to_string() }));
        assert_eq!(row.title, "Ship the CLI wiring");
        assert_eq!(format_line(&row), line);
    }

    #[test]
    fn round_trips_a_dropped_row_with_no_trailing_title() {
        let line = "- [ ] 3.7 **DROPPED**";
        let row = parse_line(line).expect("recognized dropped row with empty title");
        assert_eq!(row.title, "");
        assert_eq!(format_line(&row), line);
    }

    #[test]
    fn round_trips_an_indented_row() {
        let line = "  - [ ] 3.1.1 A nested sub-task";
        let row = parse_line(line).expect("recognized indented row");
        assert_eq!(row.indent, "  ");
        assert_eq!(format_line(&row), line);
    }

    // ── covers segment (design s20 Decision 2) ──

    #[test]
    fn round_trips_a_covers_segment_with_evidence() {
        let line = "- [x] 3.2 Implement the widget renderer [covers: wall.render.01, wall.render.02] — ✅ crates/app/src/widget.rs";
        let row = parse_line(line).expect("recognized covers row");
        assert_eq!(row.title, "Implement the widget renderer");
        assert_eq!(row.scenario_refs, vec![ScenarioId::parse("wall.render.01").unwrap(), ScenarioId::parse("wall.render.02").unwrap()]);
        assert!(row.malformed_scenario_refs.is_empty());
        assert_eq!(row.evidence.as_deref(), Some("crates/app/src/widget.rs"));
        assert_eq!(format_line(&row), line);
    }

    #[test]
    fn round_trips_a_covers_and_deferred_row() {
        let line = "- [ ] 4.1 **DEFERRED to §5** Wire the audio bus [covers: wall.audio.03]";
        let row = parse_line(line).expect("recognized deferred+covers row");
        assert_eq!(row.annotation, Some(Annotation::Deferred { to: "5".to_string() }));
        assert_eq!(row.title, "Wire the audio bus");
        assert_eq!(row.scenario_refs, vec![ScenarioId::parse("wall.audio.03").unwrap()]);
        assert_eq!(format_line(&row), line);
    }

    #[test]
    fn a_row_without_a_covers_segment_has_empty_scenario_refs() {
        let row = parse_line("- [ ] 1.5 Not started yet").unwrap();
        assert!(row.scenario_refs.is_empty());
        assert!(row.malformed_scenario_refs.is_empty());
    }

    #[test]
    fn one_malformed_covers_token_is_dropped_but_the_others_and_the_row_survive() {
        let line = "- [ ] 5.1 Cover several scenarios [covers: wall.render.01, not-a-scenario-id, wall.render.02]";
        let row = parse_line(line).expect("recognized covers row despite one malformed token");
        assert_eq!(row.title, "Cover several scenarios");
        assert_eq!(row.scenario_refs, vec![ScenarioId::parse("wall.render.01").unwrap(), ScenarioId::parse("wall.render.02").unwrap()]);
        assert_eq!(row.malformed_scenario_refs, vec!["not-a-scenario-id".to_string()]);
        // The malformed token, and the original interleaved order, must
        // survive byte-identically on rewrite (s20 Decision 2 review).
        assert_eq!(format_line(&row), line, "format_line must never drop an operator-authored malformed covers token");
    }

    #[test]
    fn format_line_never_drops_a_malformed_covers_token_even_when_only_malformed_tokens_are_present() {
        let line = "- [ ] 5.4 Cover nothing real [covers: not-a-scenario-id, also-not-one]";
        let row = parse_line(line).expect("recognized covers row despite every token being malformed");
        assert!(row.scenario_refs.is_empty());
        assert_eq!(row.malformed_scenario_refs, vec!["not-a-scenario-id".to_string(), "also-not-one".to_string()]);
        assert_eq!(format_line(&row), line, "an all-malformed covers bracket must still round-trip verbatim");
    }

    #[test]
    fn an_empty_covers_bracket_is_left_as_title_prose() {
        let line = "- [ ] 5.2 Some task [covers: ]";
        let row = parse_line(line).expect("recognized as a plain row, bracket unrecognized");
        assert_eq!(row.title, "Some task [covers: ]");
        assert!(row.scenario_refs.is_empty());
        assert!(row.malformed_scenario_refs.is_empty());
        assert_eq!(format_line(&row), line);
    }

    #[test]
    fn an_unbalanced_covers_bracket_is_left_as_title_prose() {
        let line = "- [ ] 5.3 Some task [covers: wall.render.01";
        let row = parse_line(line).expect("recognized as a plain row, bracket unrecognized");
        assert_eq!(row.title, "Some task [covers: wall.render.01");
        assert!(row.scenario_refs.is_empty());
        assert!(row.malformed_scenario_refs.is_empty());
        assert_eq!(format_line(&row), line);
    }

    // ── is_task_number ──

    #[test]
    fn is_task_number_accepts_plain_and_dotted_integers() {
        assert!(is_task_number("1"));
        assert!(is_task_number("1.1"));
        assert!(is_task_number("6.2.3"));
    }

    #[test]
    fn is_task_number_rejects_non_numeric_or_empty_segments() {
        assert!(!is_task_number(""));
        assert!(!is_task_number("1."));
        assert!(!is_task_number(".1"));
        assert!(!is_task_number("1.a"));
        assert!(!is_task_number("not-a-number"));
    }

    // ── task_id_for ──

    #[test]
    fn task_id_for_derives_the_change_id_hash_n_key() {
        let change_id = ChangeId::parse("add-widget").unwrap();
        let task_id = task_id_for(&change_id, "1.2").expect("valid change_id + numeric row id derives a task_id");
        assert_eq!(task_id.as_str(), "add-widget#1.2");
    }

    #[test]
    fn task_id_for_is_none_when_the_row_id_is_not_a_task_number() {
        let change_id = ChangeId::parse("add-widget").unwrap();
        assert!(task_id_for(&change_id, "not-a-number").is_none());
        assert!(task_id_for(&change_id, "").is_none());
    }

    // ── marker_text ──

    #[test]
    fn marker_text_round_trips_through_parse_annotation() {
        assert_eq!(Annotation::Dropped.marker_text(), "**DROPPED**");
        assert_eq!(Annotation::Deferred { to: "4.2".to_string() }.marker_text(), "**DEFERRED to §4.2**");
    }
}
