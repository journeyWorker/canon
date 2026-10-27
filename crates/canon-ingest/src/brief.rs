//! The one reader of a change brief's structure (0.31 `brief-format`).
//!
//! A brief is a markdown file: an optional YAML frontmatter, a `# title`,
//! `##` sections and, at the end, the `Approved:` line. Every reader that
//! needs the sections or the kind of a brief goes through [`parse`]:
//! `canon brief check` and `show`, `canon status`, the dashboard page, the
//! chore review and re-verify reads, the ship guard, `canon scenario new
//! --flow`, language detection, and this crate's plan import (the Intent
//! summary of [`crate::plan_adapters::openspec`]). A reader of its own would
//! read a moved chore as a feature without a word.
//!
//! # Sections
//! A section starts at a `##` line whose text, trimmed and compared exactly
//! (case included), is the English heading of a known section
//! ([`FEATURE_SECTIONS`], [`CHORE_SECTIONS`]); the key and the role come from
//! that table. A `##` line that is not a known heading is body text of the
//! section before it, as is every line inside a code fence. A brief with fewer
//! than two distinct known headings has no sections ([`Brief::sections`] is
//! empty): it is a note from before the PRD format, and one matching heading
//! must not turn it into a pile of missing-section blockers.
//!
//! # Frontmatter
//! The lines from a `---` on the first line to the next `---` line are YAML,
//! not prose: [`Brief::lines`] holds them blank, so no sentence rule, length
//! count or language detection reads them. The keys are `kind` (`chore`) and
//! `waive` (a list of `rule`, `section`, `reason` and optionally `match`).
//! [`Brief::frontmatter`] reports what could not be read.
//!
//! # The old format
//! Before 0.31 a brief carried `<!-- canon:section|hint|waive|kind … -->`
//! comments and a shipped Korean heading per section. [`Brief::old_format`]
//! names what was found; `canon migrate` rewrites such a brief. While it is
//! there, a `canon:kind chore` comment still marks the brief a chore, so a
//! reader of an unmigrated repo does not start reading a chore as a feature.

use serde_yaml::Value;

/// How a section's text is judged: `normative` sections carry the
/// requirements and get the prose rules; `context` and `question` sections
/// do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Normative,
    Context,
    Question,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Normative => "normative",
            Role::Context => "context",
            Role::Question => "question",
        }
    }

    pub fn parse(raw: &str) -> Option<Role> {
        match raw {
            "normative" => Some(Role::Normative),
            "context" => Some(Role::Context),
            "question" => Some(Role::Question),
            _ => None,
        }
    }
}

/// What a change is: a `chore` changes how the subject is built or kept,
/// never what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Feature,
    Chore,
}

impl Kind {
    pub fn parse(raw: &str) -> Option<Kind> {
        match raw {
            "feature" => Some(Kind::Feature),
            "chore" => Some(Kind::Chore),
            _ => None,
        }
    }

    /// The sections a brief of this kind carries, in order.
    pub fn sections(self) -> &'static [SectionDef] {
        match self {
            Kind::Feature => &FEATURE_SECTIONS,
            Kind::Chore => &CHORE_SECTIONS,
        }
    }
}

/// One section of a brief: its key, role, English heading (what readers
/// find it by) and the Korean heading canon 0.30 and earlier shipped (what
/// `canon migrate` translates).
#[derive(Debug, PartialEq, Eq)]
pub struct SectionDef {
    pub key: &'static str,
    pub role: Role,
    pub heading: &'static str,
    pub shipped_ko_heading: &'static str,
}

/// The feature brief's sections, in the order a brief carries them.
pub const FEATURE_SECTIONS: [SectionDef; 7] = [
    SectionDef { key: "intent", role: Role::Context, heading: "Intent", shipped_ko_heading: "의도" },
    SectionDef { key: "outcome", role: Role::Normative, heading: "Desired outcome", shipped_ko_heading: "원하는 결과" },
    SectionDef { key: "flow", role: Role::Normative, heading: "User flow", shipped_ko_heading: "사용자 흐름" },
    SectionDef { key: "detail", role: Role::Normative, heading: "Step detail", shipped_ko_heading: "단계 상세" },
    SectionDef { key: "acceptance", role: Role::Normative, heading: "Acceptance", shipped_ko_heading: "인수 조건" },
    SectionDef { key: "questions", role: Role::Question, heading: "Questions and assumptions", shipped_ko_heading: "질문과 가정" },
    SectionDef { key: "out-of-scope", role: Role::Context, heading: "Out of scope", shipped_ko_heading: "범위 밖" },
];

/// The chore brief's sections, in order.
pub const CHORE_SECTIONS: [SectionDef; 5] = [
    SectionDef { key: "intent", role: Role::Context, heading: "Intent", shipped_ko_heading: "의도" },
    SectionDef { key: "category", role: Role::Context, heading: "Category", shipped_ko_heading: "분류" },
    SectionDef { key: "invariants", role: Role::Normative, heading: "Invariants", shipped_ko_heading: "불변 조건" },
    SectionDef { key: "checks", role: Role::Normative, heading: "Checks", shipped_ko_heading: "검증" },
    SectionDef { key: "out-of-scope", role: Role::Context, heading: "Out of scope", shipped_ko_heading: "범위 밖" },
];

/// Every known section, feature then chore (a heading both share is listed once).
fn known() -> impl Iterator<Item = &'static SectionDef> {
    FEATURE_SECTIONS.iter().chain(CHORE_SECTIONS.iter().filter(|chore| !FEATURE_SECTIONS.iter().any(|f| f.key == chore.key)))
}

/// The section whose English heading is `heading` (trimmed, case-sensitive).
pub fn def_of_heading(heading: &str) -> Option<&'static SectionDef> {
    let heading = heading.trim();
    known().find(|def| def.heading == heading)
}

/// The section whose shipped Korean heading is `heading`.
pub fn def_of_shipped_ko_heading(heading: &str) -> Option<&'static SectionDef> {
    let heading = heading.trim();
    known().find(|def| def.shipped_ko_heading == heading)
}

/// The role `key` has in a brief of either kind, if canon knows the key.
pub fn canonical_role(key: &str) -> Option<Role> {
    known().find(|def| def.key == key).map(|def| def.role)
}

/// A brief line outside HTML comments and fenced code (frontmatter lines and
/// fence lines are empty), with its 1-based line in the file.
#[derive(Debug, Clone)]
pub struct Line {
    pub no: usize,
    pub text: String,
}

/// An HTML comment outside fenced code.
#[derive(Debug, Clone)]
pub struct Comment {
    /// The line it opens on.
    pub line: usize,
    /// The line it closes on.
    pub end_line: usize,
    /// Only the comment is on its lines: a whole-line comment can be removed with them.
    pub whole_line: bool,
    /// The text between `<!--` and `-->`, trimmed.
    pub body: String,
}

/// One section of a brief.
#[derive(Debug, Clone)]
pub struct Section {
    pub key: &'static str,
    pub role: Role,
    /// The heading line's text, `##` and surrounding space dropped.
    pub heading: String,
    /// The file line of the heading.
    pub line: usize,
    /// Index into [`Brief::lines`] of the heading line.
    pub start: usize,
    /// Index into [`Brief::lines`] one past the section's last line.
    pub end: usize,
}

/// One `waive` entry of the frontmatter, as written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WaiverEntry {
    /// The file line the entry starts on.
    pub line: usize,
    pub rule: Option<String>,
    pub section: Option<String>,
    pub reason: Option<String>,
    pub match_text: Option<String>,
    /// Keys of the entry that are not `rule`, `section`, `reason`, `match`.
    pub unknown_keys: Vec<String>,
    /// A field that is present but not a string.
    pub not_text: Vec<String>,
}

/// What the frontmatter said.
#[derive(Debug, Clone, Default)]
pub struct Frontmatter {
    /// The file line of the closing `---`.
    pub end_line: usize,
    /// The `kind` it names (`chore`).
    pub kind: Option<Kind>,
    /// The `waive` entries; empty when [`Self::problems`] is not.
    pub waivers: Vec<WaiverEntry>,
    /// `(line, message)`: YAML that does not parse, an unknown key, an
    /// unknown kind, a `waive` that is not a list. Then no waiver is read.
    pub problems: Vec<(usize, String)>,
}

/// A comment or heading of the format canon 0.30 and earlier wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldMarker {
    pub line: usize,
    /// `canon:section`, `canon:hint`, `canon:waive`, `canon:kind` or `heading` (a shipped Korean one).
    pub what: &'static str,
}

/// The old format's traces in a brief.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OldFormat {
    pub markers: Vec<OldMarker>,
}

impl OldFormat {
    /// What was found, in words: `3 canon:section comment(s), 2 shipped Korean heading(s)`.
    pub fn describe(&self) -> String {
        let count = |what: &str| self.markers.iter().filter(|m| m.what == what).count();
        let mut parts: Vec<String> = ["canon:section", "canon:hint", "canon:waive", "canon:kind"].iter().filter(|w| count(w) > 0).map(|w| format!("{} `{w}` comment(s)", count(w))).collect();
        if count("heading") > 0 {
            parts.push(format!("{} shipped Korean heading(s)", count("heading")));
        }
        let first = self.markers.iter().map(|m| m.line).min().unwrap_or(1);
        format!("{} (first at line {first})", parts.join(", "))
    }
}

/// A brief read once.
#[derive(Debug, Clone)]
pub struct Brief {
    /// Every line of the file, comments, fences and frontmatter blanked.
    pub lines: Vec<Line>,
    /// The HTML comments outside fences.
    pub comments: Vec<Comment>,
    /// The frontmatter, when the file starts with one.
    pub frontmatter: Option<Frontmatter>,
    /// The sections, in file order; empty for a note with fewer than two distinct known headings.
    pub sections: Vec<Section>,
    /// `##` lines that are not a known heading, with their line: body text of the section before them,
    /// and what a missing section's hint looks for a near miss in.
    pub other_h2: Vec<(usize, String)>,
    /// The kind: `kind: chore` in the frontmatter (a `canon:kind chore` comment while the brief is old-format).
    pub kind: Kind,
    /// The old format's traces; `None` for a brief in the 0.31 format.
    pub old_format: Option<OldFormat>,
}

impl Brief {
    /// The lines of `section`, its heading line first.
    pub fn section_lines(&self, section: &Section) -> &[Line] {
        &self.lines[section.start..section.end]
    }
}

fn is_frontmatter_fence(line: &str) -> bool {
    line.trim_end() == "---"
}

/// The 1-based line of the closing `---`, when the first line is `---` and a later one is too.
fn frontmatter_end(text: &str) -> Option<usize> {
    let mut lines = text.lines();
    if !lines.next().is_some_and(is_frontmatter_fence) {
        return None;
    }
    lines.position(is_frontmatter_fence).map(|i| i + 2)
}

/// The lines outside fences and comments, and the comments.
fn scan(text: &str, first_body_line: usize) -> (Vec<Line>, Vec<Comment>) {
    struct Open {
        line: usize,
        starts_clean: bool,
        body: String,
    }
    let mut lines = Vec::new();
    let mut comments = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut open: Option<Open> = None;
    for (i, raw) in text.lines().enumerate() {
        let no = i + 1;
        if no < first_body_line {
            lines.push(Line { no, text: String::new() });
            continue;
        }
        if open.is_none() {
            match (fence, crate::markdown::fence_of(raw)) {
                (None, Some((ch, run, _))) => {
                    fence = Some((ch, run));
                    lines.push(Line { no, text: String::new() });
                    continue;
                }
                (Some((open_ch, open_run)), Some((ch, run, true))) if ch == open_ch && run >= open_run => {
                    fence = None;
                    lines.push(Line { no, text: String::new() });
                    continue;
                }
                (Some(_), _) => {
                    lines.push(Line { no, text: String::new() });
                    continue;
                }
                (None, None) => {}
            }
        }
        let mut out = String::new();
        let mut rest = raw;
        loop {
            match open.take() {
                Some(mut comment) => match rest.find("-->") {
                    Some(at) => {
                        comment.body.push_str(&rest[..at]);
                        rest = &rest[at + 3..];
                        comments.push(Comment { line: comment.line, end_line: no, whole_line: comment.starts_clean && rest.trim().is_empty(), body: comment.body.trim().to_string() });
                    }
                    None => {
                        comment.body.push_str(rest);
                        comment.body.push(' ');
                        open = Some(comment);
                        break;
                    }
                },
                None => match rest.find("<!--") {
                    Some(at) => {
                        out.push_str(&rest[..at]);
                        open = Some(Open { line: no, starts_clean: out.trim().is_empty(), body: String::new() });
                        rest = &rest[at + 4..];
                    }
                    None => {
                        out.push_str(rest);
                        break;
                    }
                },
            }
        }
        lines.push(Line { no, text: out.trim_end().to_string() });
    }
    (lines, comments)
}

/// A `##` heading line's text: up to three spaces of indent, `##`, a space.
fn h2_text(line: &str) -> Option<&str> {
    let trimmed = line.trim_end();
    let indent = trimmed.len() - trimmed.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = trimmed[indent..].strip_prefix("##")?;
    rest.strip_prefix(' ').or_else(|| rest.strip_prefix('\t')).map(str::trim)
}

fn parse_frontmatter(text: &str, end_line: usize) -> Frontmatter {
    let inner: String = text.lines().skip(1).take(end_line.saturating_sub(2)).map(|l| format!("{l}\n")).collect();
    let mut out = Frontmatter { end_line, ..Frontmatter::default() };
    let value: Value = match serde_yaml::from_str(&inner) {
        Ok(value) => value,
        Err(e) => {
            let line = e.location().map_or(1, |at| at.line() + 1);
            out.problems.push((line, format!("the frontmatter is not YAML: {e}")));
            return out;
        }
    };
    let map = match value {
        Value::Null => return out,
        Value::Mapping(map) => map,
        _ => {
            out.problems.push((1, "the frontmatter is not a mapping of keys".to_string()));
            return out;
        }
    };
    let waive_line = text.lines().position(|l| l.starts_with("waive:")).map_or(1, |i| i + 1);
    let mut item_lines =
        text.lines().enumerate().skip(waive_line).take(end_line.saturating_sub(waive_line + 1)).filter(|(_, l)| l.trim_start().starts_with("- ") || l.trim() == "-").map(|(i, _)| i + 1);
    for (key, value) in &map {
        match key.as_str() {
            Some("kind") => match value.as_str().and_then(Kind::parse) {
                Some(kind) => out.kind = Some(kind),
                None => out.problems.push((1, "`kind` must be `chore` (a feature brief has no `kind`)".to_string())),
            },
            Some("waive") => match value.as_sequence() {
                Some(items) => {
                    for item in items {
                        let line = item_lines.next().unwrap_or(waive_line);
                        out.waivers.push(waiver_entry(item, line));
                    }
                }
                None => out.problems.push((waive_line, "`waive` must be a list of entries with `rule`, `section`, `reason`".to_string())),
            },
            Some(other) => out.problems.push((1, format!("unknown frontmatter key `{other}`; the keys are `kind` and `waive`"))),
            None => out.problems.push((1, "a frontmatter key that is not text".to_string())),
        }
    }
    if !out.problems.is_empty() {
        out.waivers.clear();
    }
    out
}

fn waiver_entry(item: &Value, line: usize) -> WaiverEntry {
    let mut entry = WaiverEntry { line, ..WaiverEntry::default() };
    let Some(map) = item.as_mapping() else {
        entry.unknown_keys.push("(the entry is not a mapping)".to_string());
        return entry;
    };
    for (key, value) in map {
        let name = key.as_str().unwrap_or("(not text)").to_string();
        let slot = match name.as_str() {
            "rule" => &mut entry.rule,
            "section" => &mut entry.section,
            "reason" => &mut entry.reason,
            "match" => &mut entry.match_text,
            _ => {
                entry.unknown_keys.push(name);
                continue;
            }
        };
        match value.as_str() {
            Some(text) => *slot = Some(text.to_string()),
            None => entry.not_text.push(name),
        }
    }
    entry
}

/// Reads `text` as a brief (module doc).
pub fn parse(text: &str) -> Brief {
    let fm_end = frontmatter_end(text);
    let (lines, comments) = scan(text, fm_end.map_or(1, |end| end + 1));
    let frontmatter = fm_end.map(|end| parse_frontmatter(text, end));

    let mut old = OldFormat::default();
    for comment in &comments {
        let first = comment.body.split_whitespace().next().unwrap_or_default();
        if let Some(what) = ["canon:section", "canon:hint", "canon:waive", "canon:kind"].into_iter().find(|w| *w == first) {
            old.markers.push(OldMarker { line: comment.line, what });
        }
    }

    let mut heads: Vec<(usize, &'static SectionDef, String)> = Vec::new();
    let mut other_h2: Vec<(usize, String)> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(heading) = h2_text(&line.text) else {
            continue;
        };
        if let Some(def) = def_of_heading(heading) {
            heads.push((index, def, heading.to_string()));
        } else if let Some(def) = def_of_shipped_ko_heading(heading) {
            old.markers.push(OldMarker { line: line.no, what: "heading" });
            heads.push((index, def, heading.to_string()));
        } else {
            other_h2.push((line.no, heading.to_string()));
        }
    }
    let distinct = heads.iter().enumerate().filter(|(i, (_, def, _))| !heads[..*i].iter().any(|(_, other, _)| other.key == def.key)).count();
    let sections: Vec<Section> = if distinct < 2 {
        Vec::new()
    } else {
        heads
            .iter()
            .enumerate()
            .map(|(i, (start, def, heading))| Section {
                key: def.key,
                role: def.role,
                heading: heading.clone(),
                line: lines[*start].no,
                start: *start,
                end: heads.get(i + 1).map_or(lines.len(), |next| next.0),
            })
            .collect()
    };
    old.markers.sort_by_key(|m| m.line);
    let old_format = (!old.markers.is_empty()).then_some(old);
    let legacy_chore = old_format.as_ref().is_some_and(|_| comments.iter().any(|c| c.body.split_whitespace().collect::<Vec<_>>() == ["canon:kind", "chore"]));
    let kind = if frontmatter.as_ref().and_then(|f| f.kind) == Some(Kind::Chore) || legacy_chore { Kind::Chore } else { Kind::Feature };
    Brief { lines, comments, frontmatter, sections, other_h2, kind, old_format }
}

/// Whether `text` is a chore brief.
pub fn is_chore(text: &str) -> bool {
    parse(text).kind == Kind::Chore
}

/// The edit distance between two headings, compared by characters.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(ca != cb)).min(row[j] + 1).min(above + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

/// The `##` heading among `candidates` (line, text) closest to `expected`,
/// when it differs by case or by at most two characters.
pub fn near_miss<'a>(expected: &str, candidates: &'a [(usize, String)]) -> Option<&'a (usize, String)> {
    candidates
        .iter()
        .filter_map(|c| {
            let distance = if c.1.to_lowercase() == expected.to_lowercase() { 0 } else { edit_distance(&c.1, expected) };
            (distance <= 2).then_some((distance, c))
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, c)| c)
}

/// The first paragraph of the Intent section, whitespace-normalized: what a
/// change's summary is. `None` when the brief has no Intent section; an empty
/// string when the section holds nothing yet.
pub fn intent_summary(brief: &Brief) -> Option<String> {
    let section = brief.sections.iter().find(|s| s.key == "intent")?;
    let mut paragraph = String::new();
    for line in brief.section_lines(section).iter().skip(1) {
        let trimmed = line.text.trim();
        if trimmed.is_empty() {
            if paragraph.is_empty() {
                continue;
            }
            break;
        }
        if trimmed.starts_with('#') {
            break;
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(trimmed);
    }
    Some(paragraph.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEATURE: &str = "# Add login\n\n## Intent\n\nAdd login\n\n## Desired outcome\n\nA visitor signs in.\n\n## User flow\n\n1. F1 The visitor signs in.\n";

    #[test]
    fn sections_are_found_by_their_english_heading_with_key_and_role() {
        let brief = parse(FEATURE);
        let found: Vec<(&str, &str, usize)> = brief.sections.iter().map(|s| (s.key, s.role.as_str(), s.line)).collect();
        assert_eq!(found, vec![("intent", "context", 3), ("outcome", "normative", 7), ("flow", "normative", 11)]);
        assert!(brief.old_format.is_none() && brief.frontmatter.is_none());
    }

    #[test]
    fn headings_compare_exactly_and_a_fenced_heading_is_not_one() {
        let text = "# t\n\n## Intent\n\nx\n\n## Desired Outcome\n\ny\n\n```\n## User flow\n```\n\n## Step detail\n\nz\n";
        let brief = parse(text);
        let keys: Vec<&str> = brief.sections.iter().map(|s| s.key).collect();
        assert_eq!(keys, vec!["intent", "detail"], "the wrong case and the fenced line are body text");
        assert_eq!(brief.other_h2, vec![(7, "Desired Outcome".to_string())]);
    }

    #[test]
    fn a_note_with_one_known_heading_has_no_sections() {
        let brief = parse("# Notes\n\n## Intent\n\nwhy\n\n## Open items\n\n- a\n");
        assert!(brief.sections.is_empty());
    }

    #[test]
    fn the_frontmatter_shifts_no_line_and_is_not_prose() {
        let text = "---\nkind: chore\nwaive:\n  - rule: prose.length\n    section: brief\n    reason: \"long\"\n---\n\n# t\n\n## Intent\n\nx\n\n## Category\n\nci\n";
        let brief = parse(text);
        assert_eq!(brief.kind, Kind::Chore);
        assert!(brief.lines[..7].iter().all(|l| l.text.is_empty()));
        assert_eq!(brief.sections[0].line, 11);
        let fm = brief.frontmatter.unwrap();
        assert_eq!(fm.waivers.len(), 1);
        assert_eq!((fm.waivers[0].line, fm.waivers[0].rule.as_deref(), fm.waivers[0].reason.as_deref()), (4, Some("prose.length"), Some("long")));
        assert!(fm.problems.is_empty());
    }

    #[test]
    fn a_broken_frontmatter_reads_no_waiver_and_says_why() {
        for (text, want) in [
            ("---\nwaive: [\n---\n# t\n", "not YAML"),
            ("---\nkinds: chore\n---\n# t\n", "unknown frontmatter key `kinds`"),
            ("---\nkind: epic\n---\n# t\n", "`kind` must be"),
            ("---\nwaive: x\n---\n# t\n", "must be a list"),
        ] {
            let fm = parse(text).frontmatter.unwrap();
            assert!(fm.waivers.is_empty() && fm.problems.iter().any(|(_, m)| m.contains(want)), "{text:?}: {:?}", fm.problems);
        }
    }

    #[test]
    fn the_old_format_is_found_by_its_comments_and_its_korean_headings() {
        let marked = parse("# t\n\n<!-- canon:section intent role=context -->\n## Intent\n\nx\n\n<!-- canon:hint what -->\n");
        assert_eq!(marked.old_format.as_ref().unwrap().markers.len(), 2);
        let korean = parse("# t\n\n## 의도\n\nx\n\n## 원하는 결과\n\ny\n");
        assert_eq!(korean.old_format.unwrap().markers.iter().map(|m| m.what).collect::<Vec<_>>(), vec!["heading", "heading"]);
        let approval_note = parse("# t\n\n## Intent\n\nx\n\n## Out of scope\n\ny\n\n<!-- The human records their answer -->\n");
        assert!(approval_note.old_format.is_none(), "an approval note is not a marker");
    }

    #[test]
    fn an_old_chore_marker_still_reads_as_a_chore() {
        assert!(is_chore("# t\n\n<!-- canon:kind chore -->\n"));
        assert!(!is_chore("# t\n\n<!-- canon:kind epic -->\n"));
        assert!(is_chore("---\nkind: chore\n---\n# t\n"));
        assert!(!is_chore("# t\n"));
    }

    #[test]
    fn a_near_miss_is_a_case_difference_or_two_edits() {
        let others = vec![(3, "Intnet".to_string()), (9, "Details".to_string())];
        assert_eq!(near_miss("Intent", &others).map(|c| c.1.as_str()), Some("Intnet"));
        assert_eq!(near_miss("Acceptance", &[(1, "acceptance".to_string())]).map(|c| c.0), Some(1));
        assert!(near_miss("Step detail", &others).is_none());
    }

    #[test]
    fn the_intent_summary_is_its_first_paragraph() {
        let brief = parse("# t\n\n## Intent\n\nAdd\nlogin.\n\nMore.\n\n## Desired outcome\n\nx\n");
        assert_eq!(intent_summary(&brief), Some("Add login.".to_string()));
        let empty = parse("# t\n\n## Intent\n\n## Desired outcome\n\nx\n");
        assert_eq!(intent_summary(&empty), Some(String::new()));
        assert_eq!(intent_summary(&parse("# t\n\n## Why\n\nx\n")), None);
    }

    #[test]
    fn whole_line_comments_are_marked() {
        let brief = parse("# t\n\n<!-- a -->\ntext <!-- b --> more\n<!-- c\nd -->\n");
        let whole: Vec<(usize, usize, bool)> = brief.comments.iter().map(|c| (c.line, c.end_line, c.whole_line)).collect();
        assert_eq!(whole, vec![(3, 3, true), (4, 4, false), (5, 6, true)]);
    }
}
