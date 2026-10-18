//! `canon brief check` and bare `canon brief` (0.19 D2): a read-only
//! checker for a change's PRD brief ([`crate::brief_template`]).
//!
//! # Findings
//! Each finding carries a namespaced rule id ([`RULES`]), a severity, the
//! section key, a line, an excerpt and a hint. Exit `0` clean (advisories
//! allowed), `2` blocker findings, `1` an operational error with a stable
//! string `error.code`.
//!
//! - **Blockers (structure, `brief.*`)**: a missing, duplicated or
//!   out-of-order section; a flow with no steps or a duplicate step id;
//!   step-detail subheadings that do not match the flow ids in order;
//!   leftover placeholders (a `canon:hint` comment still in the brief, a
//!   section with nothing in it, `TODO`/`TBD`); once the subject is
//!   `building` or later, a flow step no scenario points at
//!   (`brief.flow-uncovered`) or one with no `@case:failure` scenario
//!   (`brief.flow-no-failure`); a waiver that is malformed.
//! - **Advisories (prose, `prose.*`)**: closed per-language phrase lists,
//!   sentence and paragraph length, and a normative section without
//!   exactly one `==**key sentence**==`. Applied to `role=normative`
//!   sections only, and only for a `language:` canon ships a rule list
//!   for ([`crate::language`]): the finding carries the matched phrase or
//!   the threshold and the measured value, never an authorship verdict.
//! - **`brief.legacy` (advisory)**: a brief with no section markers (every
//!   brief before 0.19) is read as before; no other rule applies to it.
//! - **Chore briefs (0.19 D5)**: a brief carrying `<!-- canon:kind chore -->`
//!   is judged by the chore rule set: sections intent, category,
//!   invariants, checks, out-of-scope, in that order; a flow, detail or
//!   acceptance section is `brief.chore-section`; a category that is not
//!   one of [`CHORE_CATEGORIES`] is `brief.chore-category`; invariants or
//!   checks that say only "none" are `brief.chore-empty`. Placeholders
//!   block and the prose rules apply to the normative sections (invariants,
//!   checks) as for any brief. The flow-coverage rules never apply.
//!
//! Sections are found by their marker comment, never by heading text.
//! A marker key canon does not know is skipped, not reported.
//!
//! # Waivers
//! `<!-- canon:waive <rule-id> <section-key> "<reason>" -->` waives one
//! rule in one section; the reason is required. The output lists every
//! waiver, with the severity of the rule it waives. There is no global
//! switch.
//!
//! # Flow coverage
//! The flow rules read scenarios through [`flow_scenarios`], the one
//! place that maps a flow step to its scenarios by their
//! `@flow:<change>.<F-id>` tag (0.19 D4), written by `canon scenario new
//! --flow <F-id>`. It reads the spec roots' `.feature` files through the
//! scan `canon inventory sync` uses (`canon_fmt::gherkin`), so a tag
//! counts as soon as it is written. `canon status` joins the same index
//! to each scenario's evidence and review state.
//!
//! # `canon brief`
//! Prints the brief's state and the next one to three steps (`shape`,
//! `clarify`, `harden`, `align`, `polish`, `approve`), derived from the
//! findings. It never edits.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use canon_ingest::markdown::{fence_of, field};
use canon_model::{paths, ChangeId, SubjectStatus};
use serde::Serialize;

use crate::brief_template::{canonical_role, heading_of, marker, ChangeKind, Role, CHORE_CATEGORIES};
use crate::cli_error::CliError;
use crate::context::resolve_repo_root;
use crate::language::{Language, Shipped};

/// Exit code for blocker findings (mirrors `canon change new`'s refusal).
const EXIT_BLOCKERS: i32 = 2;

/// A finding's severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Blocker,
    Advisory,
}

impl Severity {
    fn as_str(self) -> &'static str {
        match self {
            Severity::Blocker => "blocker",
            Severity::Advisory => "advisory",
        }
    }
}

/// Every rule id and its severity. A waiver names one of these.
pub const RULES: [(&str, Severity); 22] = [
    ("brief.section-missing", Severity::Blocker),
    ("brief.section-duplicate", Severity::Blocker),
    ("brief.section-order", Severity::Blocker),
    ("brief.placeholder", Severity::Blocker),
    ("brief.flow-no-steps", Severity::Blocker),
    ("brief.flow-duplicate-id", Severity::Blocker),
    ("brief.detail-mismatch", Severity::Blocker),
    ("brief.flow-uncovered", Severity::Blocker),
    ("brief.flow-no-failure", Severity::Blocker),
    ("brief.chore-section", Severity::Blocker),
    ("brief.chore-category", Severity::Blocker),
    ("brief.chore-empty", Severity::Blocker),
    ("brief.waiver-invalid", Severity::Blocker),
    ("brief.legacy", Severity::Advisory),
    ("prose.evaluative", Severity::Advisory),
    ("prose.connective", Severity::Advisory),
    ("prose.contrast", Severity::Advisory),
    ("prose.colloquial", Severity::Advisory),
    ("prose.degree", Severity::Advisory),
    ("prose.sentence-length", Severity::Advisory),
    ("prose.paragraph-length", Severity::Advisory),
    ("prose.key-sentence", Severity::Advisory),
];

/// The blocker rules that grade the scenarios against the brief rather than
/// the brief itself. They gate shipping; `canon brief approve` ignores them,
/// since a scope is approved before its scenarios exist (0.19 D3).
pub const COVERAGE_RULES: [&str; 2] = ["brief.flow-uncovered", "brief.flow-no-failure"];

fn severity_of(rule: &str) -> Option<Severity> {
    RULES.iter().find(|(id, _)| *id == rule).map(|(_, severity)| *severity)
}

/// One finding.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub rule: String,
    pub severity: Severity,
    /// The section key the finding is in; `None` for a whole-brief finding.
    pub section: Option<String>,
    pub line: usize,
    pub excerpt: String,
    pub hint: String,
}

/// One waiver comment, with the severity of the rule it waives and
/// whether it matched a finding.
#[derive(Debug, Clone, Serialize)]
pub struct Waiver {
    pub rule: String,
    pub section: String,
    pub reason: String,
    pub line: usize,
    pub severity: Option<Severity>,
    pub applied: bool,
}

/// The finding counts `canon status` shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub blockers: usize,
    pub advisories: usize,
    /// The blockers `canon brief approve` refuses on ([`blocks_approval`]):
    /// while any remain, status gives an unapproved brief's step to the
    /// planner who writes it, not the human (0.22 D3, #70). Not shown.
    #[serde(skip)]
    pub unapprovable: usize,
}

/// Whether `finding` keeps a brief from being approved: a blocker that is
/// not a coverage rule, since a scope is approved before its scenarios
/// exist (0.19 D3).
pub fn blocks_approval(finding: &Finding) -> bool {
    finding.severity == Severity::Blocker && !COVERAGE_RULES.contains(&finding.rule.as_ref())
}

/// What reading a brief found.
#[derive(Debug, Clone)]
pub struct Evaluation {
    pub findings: Vec<Finding>,
    pub waivers: Vec<Waiver>,
    /// No section markers: a pre-0.19 brief, read as before.
    pub legacy: bool,
    /// Lines of the questions section that are not a "none" answer.
    pub open_questions: usize,
    /// What the brief's `Approved:` line says about its text (0.19 D3).
    pub approval: crate::brief::Approval,
    /// Whether the brief reads as approved (a digest that matches, or a
    /// pre-0.19 line with none).
    pub approved: bool,
}

impl Evaluation {
    pub fn counts(&self) -> Counts {
        let blockers = self.findings.iter().filter(|f| f.severity == Severity::Blocker).count();
        let unapprovable = self.findings.iter().filter(|f| blocks_approval(f)).count();
        Counts { blockers, advisories: self.findings.len() - blockers, unapprovable }
    }

    fn has(&self, rules: &[&str]) -> bool {
        self.findings.iter().any(|f| rules.contains(&f.rule.as_str()))
    }
}

// ───────────────────────────── flow coverage ─────────────────────────────

/// One scenario a flow step points at.
#[derive(Debug, Clone)]
pub struct FlowScenario {
    /// The scenario's `<area>.<surface>.<nn>` id.
    pub id: String,
    pub title: String,
    /// Whether it is tagged `@case:failure`.
    pub failure: bool,
}

/// Flow step id (`F1`) → the scenarios tagged `@flow:<change>.<F-id>`.
pub type FlowIndex = BTreeMap<String, Vec<FlowScenario>>;

/// The scenarios pointing at `change`'s flow steps (module doc, "Flow
/// coverage"): the one function the `@flow:` tag lookup lives in. It
/// reads the `.feature` files under the spec roots through the same
/// Gherkin scan `canon inventory sync` indexes them with, so the gate,
/// status and the brief rules cannot disagree, and a tag written a
/// moment ago counts without a sync.
pub fn flow_scenarios(repo: &Path, change: &ChangeId) -> Result<FlowIndex, String> {
    let roots = crate::inventory::load_spec_roots(&repo.join("canon.yaml")).map_err(|e| e.to_string())?;
    let prefix = format!("{}.", change.as_str());
    let mut index = FlowIndex::new();
    for root in roots {
        for path in canon_fmt::util::feature_files(&root.root) {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            for scenario in canon_fmt::gherkin::scan(&text).scenarios {
                let failure = scenario.case_tags.iter().any(|c| c == "failure");
                for step in scenario.flow_tags.iter().filter_map(|t| t.strip_prefix(&prefix)) {
                    let entry = FlowScenario { id: scenario.scenario_id.clone(), title: scenario.title.clone(), failure };
                    let steps = index.entry(step.to_string()).or_default();
                    if !steps.iter().any(|s| s.id == entry.id) {
                        steps.push(entry);
                    }
                }
            }
        }
    }
    Ok(index)
}

/// The flow step ids `change`'s brief defines, in order; empty for a
/// legacy brief or no brief.
pub fn flow_ids(repo: &Path, change: &ChangeId) -> Vec<String> {
    let Some(path) = crate::brief::canon_brief(repo, change) else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(&path) else { return Vec::new() };
    let language = crate::language::configured(repo).unwrap_or_else(|_| Language::unset());
    let (lines, comments) = scan(&text);
    let secs = sections(&lines, &comments);
    let mut sink = Out { findings: Vec::new() };
    flow_and_detail(&mut sink, &secs, &language).into_iter().map(|(id, _)| id).collect()
}

/// The `<change>.<F-id>` value of the `@flow:` tag `canon scenario new
/// --flow <flow>` writes. The change is `change` when given, else the
/// newest change `subject` adopted, else the repo's only live change.
/// Refuses a flow id the change's brief does not define.
pub fn flow_tag_value(repo: &Path, subject: Option<&canon_model::SubjectId>, change: Option<&ChangeId>, flow: &str) -> Result<String, String> {
    let change = match (change, subject) {
        (Some(change), _) => change.clone(),
        (None, Some(subject)) => newest_change_of(repo, subject)?,
        (None, None) => resolve_change(repo, None).map(|(change, _)| change).map_err(|e| format!("{} — {}; pass --change or --subject", e.code, e.message))?,
    };
    if crate::brief::canon_brief(repo, &change).is_none() {
        return Err(format!("change `{}` has no brief at {}/{}/brief.md; write it with `canon change new`", change.as_str(), paths::CHANGES_DIR, change.as_str()));
    }
    let ids = flow_ids(repo, &change);
    if ids.iter().any(|id| id == flow) {
        return Ok(format!("{}.{flow}", change.as_str()));
    }
    Err(if ids.is_empty() {
        format!("the brief of change `{}` defines no flow steps (`F1`, `F2`, …); write the flow first", change.as_str())
    } else {
        format!("the brief of change `{}` defines no flow step `{flow}` (it has {})", change.as_str(), ids.join(", "))
    })
}

/// The newest change `subject` adopted ([`canon_gate::spec_coverage::subject_change_ids`]'s last).
pub fn newest_change_of(repo: &Path, subject: &canon_model::SubjectId) -> Result<ChangeId, String> {
    use canon_gate::spec_coverage::{latest_subjects, subject_change_ids};
    use canon_gate::{GateContext, GateCtx};
    let registry = canon_policy::SchemaRegistry::load();
    let ctx = GateCtx::from_repo(repo).map_err(|e| e.to_string()).and_then(|ctx| GateContext::load(ctx, &registry, chrono::Utc::now()).map_err(|e| e.to_string()))?;
    let found = latest_subjects(&ctx).into_iter().find(|s| &s.subject_id == subject).ok_or_else(|| format!("no subject `{}`", subject.as_str()))?;
    subject_change_ids(&ctx, found).pop().ok_or_else(|| format!("subject `{}` has not adopted a change; run `canon change new <slug> --subject {}`", subject.as_str(), subject.as_str()))
}

// ───────────────────────────── document scan ─────────────────────────────

/// A brief line outside HTML comments and fenced code.
struct Line {
    no: usize,
    text: String,
}

/// An HTML comment: its first line and its body.
struct Comment {
    line: usize,
    body: String,
}

fn scan(text: &str) -> (Vec<Line>, Vec<Comment>) {
    let mut lines = Vec::new();
    let mut comments = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut open: Option<Comment> = None;
    for (i, raw) in text.lines().enumerate() {
        let no = i + 1;
        if open.is_none() {
            match (fence, fence_of(raw)) {
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
                        comment.body = comment.body.trim().to_string();
                        comments.push(comment);
                        rest = &rest[at + 3..];
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
                        open = Some(Comment { line: no, body: String::new() });
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

/// One marked section: its marker and the lines up to the next marker.
struct Section<'a> {
    key: String,
    role: Role,
    marker_line: usize,
    lines: Vec<&'a Line>,
}

impl Section<'_> {
    /// The lines after the section's own heading, blanks dropped.
    fn body(&self) -> Vec<&Line> {
        let mut heading_seen = false;
        self.lines
            .iter()
            .copied()
            .filter(|l| !l.text.trim().is_empty())
            .filter(|l| {
                if !heading_seen && l.text.trim_start().starts_with('#') {
                    heading_seen = true;
                    return false;
                }
                true
            })
            .collect()
    }
}

fn sections<'a>(lines: &'a [Line], comments: &[Comment]) -> Vec<Section<'a>> {
    let mut markers: Vec<(usize, String, Option<Role>)> = Vec::new();
    for comment in comments {
        let mut tokens = comment.body.split_whitespace();
        if tokens.next() != Some("canon:section") {
            continue;
        }
        let Some(key) = tokens.next() else { continue };
        let role = tokens.find_map(|t| t.strip_prefix("role=")).and_then(Role::parse);
        markers.push((comment.line, key.to_string(), role));
    }
    markers.sort_by_key(|m| m.0);
    let mut out = Vec::new();
    for (i, (line, key, role)) in markers.iter().enumerate() {
        let end = markers.get(i + 1).map_or(usize::MAX, |m| m.0);
        let canonical = canonical_role(key);
        out.push(Section {
            key: key.clone(),
            role: role.or(canonical).unwrap_or(Role::Context),
            marker_line: *line,
            lines: lines.iter().filter(|l| l.no > *line && l.no < end).collect(),
        });
    }
    out
}

fn excerpt(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 80 {
        format!("{}…", flat.chars().take(80).collect::<String>())
    } else {
        flat
    }
}

/// A step id (`F1`) at the start of `s`, after the markers a list item or
/// heading may carry (`1.`, `-`, `###`, `**`, `[`).
fn leading_flow_id(s: &str) -> Option<String> {
    let mut s = s.trim_start();
    s = s.trim_start_matches('#').trim_start();
    s = s.trim_start_matches(['-', '*']).trim_start();
    let digits = s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits > 0 && s[digits..].starts_with(['.', ')']) {
        s = s[digits + 1..].trim_start();
    }
    s = s.trim_start_matches(['*', '[']);
    let rest = s.strip_prefix('F')?;
    let n = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if n == 0 || rest[n..].chars().next().is_some_and(char::is_alphanumeric) {
        return None;
    }
    Some(format!("F{}", &rest[..n]))
}

// ───────────────────────────── the rules ─────────────────────────────

struct Out {
    findings: Vec<Finding>,
}

impl Out {
    fn push(&mut self, rule: &str, section: Option<&str>, line: usize, excerpt: String, hint: String) {
        let severity = severity_of(rule).expect("every emitted rule is in RULES");
        self.findings.push(Finding { rule: rule.to_string(), severity, section: section.map(str::to_string), line, excerpt, hint });
    }
}

/// Reads `text` against the rules. `flows` is `Some` once the subject is
/// `building` or later: only then do the flow-coverage rules apply.
pub fn evaluate(text: &str, language: &Language, flows: Option<&FlowIndex>) -> Evaluation {
    let (lines, comments) = scan(text);
    let approval = crate::brief::approval_of(text);
    let approved = approval.approved();
    let secs = sections(&lines, &comments);
    if secs.is_empty() {
        let mut out = Out { findings: Vec::new() };
        out.push(
            "brief.legacy",
            None,
            1,
            "no `canon:section` markers".to_string(),
            "a pre-0.19 brief is read as before; the structure and prose rules do not apply to it. `canon change new` writes the PRD format".to_string(),
        );
        return Evaluation { findings: out.findings, waivers: Vec::new(), legacy: true, open_questions: 0, approval, approved };
    }

    let kind = kind_of_comments(&comments);
    let mut out = Out { findings: Vec::new() };
    structure(&mut out, &secs, language, kind);
    placeholders(&mut out, &secs, &comments);
    match kind {
        ChangeKind::Feature => {
            let flow_ids = flow_and_detail(&mut out, &secs, language);
            if let Some(index) = flows {
                coverage(&mut out, &secs, &flow_ids, index);
            }
        }
        ChangeKind::Chore => chore_rules(&mut out, &secs),
    }
    if let Some(shipped) = language.prose_rules() {
        let judged = |s: &&Section| kind == ChangeKind::Feature || kind.sections().iter().any(|k| k.key == s.key);
        for section in secs.iter().filter(|s| s.role == Role::Normative).filter(judged) {
            prose(&mut out, section, shipped);
        }
    }
    let open_questions = secs
        .iter()
        .find(|s| s.key == "questions")
        .map_or(0, |s| s.body().iter().filter(|l| !is_none_answer(&l.text)).count());

    let (findings, waivers) = apply_waivers(out.findings, &comments);
    Evaluation { findings, waivers, legacy: false, open_questions, approval, approved }
}

/// The kind a brief's `canon:kind` comment names; no marker, or a value
/// canon does not know, is a feature brief.
fn kind_of_comments(comments: &[Comment]) -> ChangeKind {
    comments
        .iter()
        .find_map(|c| {
            let mut tokens = c.body.split_whitespace();
            (tokens.next() == Some("canon:kind")).then(|| tokens.next().and_then(ChangeKind::parse))
        })
        .flatten()
        .unwrap_or(ChangeKind::Feature)
}

/// Whether `text` is a chore brief (carries [`CHORE_KIND_MARKER`]).
pub fn is_chore(text: &str) -> bool {
    kind_of_comments(&scan(text).1) == ChangeKind::Chore
}

fn is_none_answer(line: &str) -> bool {
    let word = line.trim().trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    ["none", "n/a", "없음", "해당 없음"].contains(&word.as_str())
}

/// Missing, duplicated and out-of-order sections.
fn structure(out: &mut Out, secs: &[Section], language: &Language, kind: ChangeKind) {
    let expected_sections = kind.sections();
    for expected in expected_sections {
        let found: Vec<&Section> = secs.iter().filter(|s| s.key == expected.key).collect();
        let heading = heading_of(language, expected.key);
        if found.is_empty() {
            out.push(
                "brief.section-missing",
                Some(expected.key),
                1,
                format!("no `{}` marker", expected.key),
                format!("add `{}` above the `{heading}` heading", marker(expected.key, expected.role)),
            );
        }
        for extra in found.iter().skip(1) {
            out.push("brief.section-duplicate", Some(expected.key), extra.marker_line, format!("second `{}` marker", expected.key), "keep one section per key".to_string());
        }
    }
    let order = |key: &str| expected_sections.iter().position(|s| s.key == key);
    let known: Vec<(&Section, usize)> = secs.iter().filter_map(|s| order(&s.key).map(|o| (s, o))).collect();
    if let Some(pair) = known.windows(2).find(|w| w[1].1 < w[0].1) {
        let section = pair[1].0;
        out.push(
            "brief.section-order",
            Some(&section.key),
            section.marker_line,
            format!("`{}` follows `{}`", section.key, pair[0].0.key),
            format!("sections go in this order: {}", expected_sections.iter().map(|s| s.key).collect::<Vec<_>>().join(" → ")),
        );
    }
}

/// The chore rule set's own rules: no flow, detail or acceptance; one
/// valid category; invariants and checks that say something.
fn chore_rules(out: &mut Out, secs: &[Section]) {
    for section in secs.iter().filter(|s| ["flow", "detail", "acceptance"].contains(&s.key.as_str())) {
        out.push(
            "brief.chore-section",
            Some(&section.key),
            section.marker_line,
            format!("a chore brief has no `{}` section", section.key),
            "a chore adds no flow, detail or acceptance; work that changes behavior is a feature change (`canon change new` without `--kind chore`)".to_string(),
        );
    }
    if let Some(category) = secs.iter().find(|s| s.key == "category") {
        let body = category.body();
        if !body.is_empty() {
            let joined = body.iter().map(|l| l.text.trim()).collect::<Vec<_>>().join(" ");
            let value = joined.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
            if !CHORE_CATEGORIES.contains(&value.as_str()) {
                out.push(
                    "brief.chore-category",
                    Some("category"),
                    body[0].no,
                    excerpt(&joined),
                    format!("the category is exactly one of: {}", CHORE_CATEGORIES.join(", ")),
                );
            }
        }
    }
    for key in ["invariants", "checks"] {
        let Some(section) = secs.iter().find(|s| s.key == key) else { continue };
        let body = section.body();
        if !body.is_empty() && body.iter().all(|l| is_none_answer(&l.text)) {
            out.push(
                "brief.chore-empty",
                Some(key),
                body[0].no,
                excerpt(&body[0].text),
                format!("a chore states what must not change and how that is checked; `{key}` cannot be \"none\""),
            );
        }
    }
}

/// Hints still in the brief, empty sections, and `TODO`/`TBD`.
fn placeholders(out: &mut Out, secs: &[Section], comments: &[Comment]) {
    for (i, section) in secs.iter().enumerate() {
        let end = secs.get(i + 1).map_or(usize::MAX, |s| s.marker_line);
        let hints: Vec<&Comment> = comments.iter().filter(|c| c.line > section.marker_line && c.line < end && c.body.starts_with("canon:hint")).collect();
        for hint in &hints {
            out.push(
                "brief.placeholder",
                Some(&section.key),
                hint.line,
                excerpt(hint.body.trim_start_matches("canon:hint").trim()),
                "replace the template hint with the section's content".to_string(),
            );
        }
        let body = section.body();
        if hints.is_empty() && body.is_empty() {
            out.push("brief.placeholder", Some(&section.key), section.marker_line, "the section is empty".to_string(), "write the section; if there is nothing to say, say so in one line".to_string());
        }
        for line in body {
            if line.text.split(|c: char| !c.is_alphanumeric()).any(|w| w == "TODO" || w == "TBD") {
                out.push("brief.placeholder", Some(&section.key), line.no, excerpt(&line.text), "resolve the TODO/TBD, or move it to the questions section".to_string());
            }
        }
    }
}

/// Flow-step rules and the detail-to-flow match. Returns the flow ids
/// (first occurrences, in order) with the line each is on.
fn flow_and_detail(out: &mut Out, secs: &[Section], language: &Language) -> Vec<(String, usize)> {
    let Some(flow) = secs.iter().find(|s| s.key == "flow") else { return Vec::new() };
    let flow_body = flow.body();
    let mut ids: Vec<(String, usize)> = Vec::new();
    for line in &flow_body {
        if line.text.trim_start().starts_with('<') {
            continue;
        }
        if let Some(id) = leading_flow_id(&line.text) {
            if ids.iter().any(|(seen, _)| *seen == id) {
                out.push("brief.flow-duplicate-id", Some("flow"), line.no, excerpt(&line.text), format!("`{id}` is already a step; ids are stable and unique"));
            } else {
                ids.push((id, line.no));
            }
        }
    }
    if ids.is_empty() && !flow_body.is_empty() {
        out.push(
            "brief.flow-no-steps",
            Some("flow"),
            flow_body[0].no,
            excerpt(&flow_body[0].text),
            format!("write the flow as a numbered list whose items start with `F1`, `F2`, …: `1. F1 …` (heading `{}`)", heading_of(language, "flow")),
        );
    }
    let Some(detail) = secs.iter().find(|s| s.key == "detail") else { return ids };
    let detail_body = detail.body();
    let headings: Vec<(String, usize)> = detail_body
        .iter()
        .filter(|l| l.text.trim_start().starts_with("###"))
        .map(|l| (leading_flow_id(&l.text).unwrap_or_else(|| "?".to_string()), l.no))
        .collect();
    if !ids.is_empty() && !detail_body.is_empty() {
        let expected: Vec<&str> = ids.iter().map(|(id, _)| id.as_str()).collect();
        let actual: Vec<&str> = headings.iter().map(|(id, _)| id.as_str()).collect();
        if expected != actual {
            let at = (0..headings.len()).find(|&i| expected.get(i) != Some(&actual[i])).and_then(|i| headings.get(i)).map_or(detail.marker_line, |h| h.1);
            out.push(
                "brief.detail-mismatch",
                Some("detail"),
                at,
                format!("detail headings: {}", if actual.is_empty() { "none".to_string() } else { actual.join(", ") }),
                format!("one `### <F-id> …` subheading per flow step, in order: {}", expected.join(", ")),
            );
        }
    }
    ids
}

/// A step no scenario points at, and a step with no failure scenario.
fn coverage(out: &mut Out, secs: &[Section], ids: &[(String, usize)], index: &FlowIndex) {
    if !secs.iter().any(|s| s.key == "flow") {
        return;
    }
    for (id, line) in ids {
        match index.get(id) {
            None => out.push(
                "brief.flow-uncovered",
                Some("flow"),
                *line,
                id.clone(),
                format!("no scenario is tagged `@flow:<change>.{id}`; add one with `canon scenario new --flow {id}`"),
            ),
            Some(scenarios) if !scenarios.iter().any(|s| s.failure) => out.push(
                "brief.flow-no-failure",
                Some("flow"),
                *line,
                id.clone(),
                format!("{} scenario(s) cover {id}, none is `@case:failure`; add the failure path", scenarios.len()),
            ),
            Some(_) => {}
        }
    }
}

fn apply_waivers(findings: Vec<Finding>, comments: &[Comment]) -> (Vec<Finding>, Vec<Waiver>) {
    let mut kept = findings;
    let mut waivers = Vec::new();
    let mut invalid = Vec::new();
    for comment in comments.iter().filter(|c| c.body.starts_with("canon:waive")) {
        let rest = comment.body.trim_start_matches("canon:waive").trim();
        let parsed = (|| {
            let (rule, rest) = rest.split_once(char::is_whitespace)?;
            let (section, rest) = rest.trim_start().split_once(char::is_whitespace)?;
            let reason = rest.trim().strip_prefix('"')?.strip_suffix('"')?.trim();
            (!reason.is_empty()).then(|| (rule.to_string(), section.to_string(), reason.to_string()))
        })();
        match parsed {
            Some((rule, section, reason)) if severity_of(&rule).is_some() => {
                let before = kept.len();
                kept.retain(|f| !(f.rule == rule && f.section.as_deref() == Some(section.as_str())));
                waivers.push(Waiver { severity: severity_of(&rule), applied: kept.len() < before, rule, section, reason, line: comment.line });
            }
            _ => invalid.push(Finding {
                rule: "brief.waiver-invalid".to_string(),
                severity: Severity::Blocker,
                section: None,
                line: comment.line,
                excerpt: excerpt(&comment.body),
                hint: "write `<!-- canon:waive <rule-id> <section-key> \"<reason>\" -->` with a known rule id and a non-empty reason".to_string(),
            }),
        }
    }
    kept.extend(invalid);
    kept.sort_by_key(|f| f.line);
    (kept, waivers)
}

// ───────────────────────────── prose rules ─────────────────────────────

const KO_EVALUATIVE: [&str; 5] = ["중요하", "핵심이", "필요가 있", "바람직하", "의미가 있"];
const KO_CONNECTIVES: [&str; 5] = ["따라서", "결론적으로", "이처럼", "사실", "참고로"];
const KO_COLLOQUIAL: [&str; 11] = ["세요", "어요", "아요", "해요", "에요", "예요", "네요", "군요", "나요", "까요", "죠"];
const KO_DEGREE: [&str; 4] = ["매우", "겨우", "무려", "고작"];
const EN_EVALUATIVE: [&str; 3] = ["it is important", "it is crucial", "it is essential"];
const EN_CONNECTIVES: [&str; 2] = ["therefore", "in conclusion"];
const EN_DEGREE: [&str; 3] = ["very", "really", "extremely"];

/// Sentence length above which `prose.sentence-length` fires: words for
/// `en`, characters for `ko`.
const EN_SENTENCE_WORDS: usize = 30;
const KO_SENTENCE_CHARS: usize = 80;
/// Sentences per paragraph above which `prose.paragraph-length` fires.
const PARAGRAPH_SENTENCES: usize = 5;

/// A run of text a prose rule reads: one paragraph or one list item, with
/// the line each character offset falls on.
struct Block {
    text: String,
    starts: Vec<(usize, usize)>,
}

impl Block {
    fn line_at(&self, offset: usize) -> usize {
        self.starts.iter().rev().find(|(start, _)| *start <= offset).map_or(1, |(_, line)| *line)
    }
}

fn is_list_item(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with("- ") || t.starts_with("* ") || t.chars().take_while(char::is_ascii_digit).count() > 0 && {
        let digits = t.chars().take_while(char::is_ascii_digit).count();
        t[digits..].starts_with(". ") || t[digits..].starts_with(") ")
    }
}

fn strip_list_marker(text: &str) -> &str {
    let t = text.trim_start();
    if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
        return rest;
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        if let Some(rest) = t[digits..].strip_prefix(". ").or_else(|| t[digits..].strip_prefix(") ")) {
            return rest;
        }
    }
    t
}

fn blocks(section: &Section) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut current: Option<Block> = None;
    let mut heading_seen = false;
    for line in &section.lines {
        let trimmed = line.text.trim();
        if trimmed.is_empty() {
            out.extend(current.take());
            continue;
        }
        if trimmed.starts_with('#') {
            if heading_seen {
                out.extend(current.take());
            }
            heading_seen = true;
            continue;
        }
        let item = is_list_item(&line.text);
        if item {
            out.extend(current.take());
        }
        let content = strip_list_marker(&line.text).trim();
        let block = current.get_or_insert_with(|| Block { text: String::new(), starts: Vec::new() });
        if !block.text.is_empty() {
            block.text.push(' ');
        }
        block.starts.push((block.text.len(), line.no));
        block.text.push_str(content);
    }
    out.extend(current.take());
    out
}

/// The sentences of `text` with their byte offsets.
fn sentences(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = 0;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (i, &(at, c)) in chars.iter().enumerate() {
        if !matches!(c, '.' | '!' | '?' | '。') {
            continue;
        }
        let after = chars.get(i + 1).map(|&(_, n)| n);
        if after.is_none_or(|n| n.is_whitespace() || matches!(n, '"' | '\'' | ')' | '*' | '=')) {
            let end = at + c.len_utf8();
            let sentence = text[start..end].trim();
            if !sentence.is_empty() {
                out.push((start + (text[start..end].len() - text[start..end].trim_start().len()), sentence));
            }
            start = end;
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        out.push((start + (text[start..].len() - text[start..].trim_start().len()), tail));
    }
    out
}

/// Sentence text as the phrase lists read it: emphasis marks and inline
/// code dropped.
fn plain(sentence: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    for c in sentence.chars() {
        match c {
            '`' => in_code = !in_code,
            _ if in_code => {}
            '*' | '=' => {}
            _ => out.push(c),
        }
    }
    out.trim().to_string()
}

fn words(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|w| !w.is_empty())
}

/// `(rule, matched phrase)` hits of the closed lists for one sentence.
fn phrase_hits(shipped: Shipped, sentence: &str) -> Vec<(&'static str, String)> {
    let s = plain(sentence);
    let mut hits = Vec::new();
    match shipped {
        Shipped::Ko => {
            if let Some(stem) = KO_EVALUATIVE.iter().find(|stem| s.contains(**stem)) {
                hits.push(("prose.evaluative", (*stem).to_string()));
            }
            let lead = s.trim_start_matches(['"', '\'', '(', '“', '‘']);
            if let Some(word) = KO_CONNECTIVES.iter().find(|w| {
                lead.strip_prefix(**w).is_some_and(|rest| rest.chars().next().is_none_or(|c| c.is_whitespace() || c == ',' || (c == '은' && rest.chars().nth(1).is_none_or(|n| n.is_whitespace()))))
            }) {
                hits.push(("prose.connective", (*word).to_string()));
            }
            if let Some(at) = s.find("아니라") {
                if s[..at].trim_end().ends_with(['이', '가', '은', '는']) {
                    hits.push(("prose.contrast", "A가 아니라 B".to_string()));
                }
            }
            let end = s.trim_end_matches(['.', '!', '?', '~', '…', ')', '"', '\'', '”']);
            if let Some(ending) = KO_COLLOQUIAL.iter().find(|e| end.ends_with(**e)) {
                hits.push(("prose.colloquial", format!("~{ending}")));
            }
            if let Some(word) = s.split_whitespace().find_map(|w| KO_DEGREE.iter().find(|d| w.starts_with(**d))) {
                hits.push(("prose.degree", (*word).to_string()));
            }
        }
        Shipped::En => {
            let lower = s.to_lowercase();
            if let Some(phrase) = EN_EVALUATIVE.iter().find(|p| lower.contains(**p)) {
                hits.push(("prose.evaluative", (*phrase).to_string()));
            }
            if let Some(word) = EN_CONNECTIVES.iter().find(|w| lower.strip_prefix(**w).is_some_and(|rest| rest.chars().next().is_none_or(|c| !c.is_alphabetic()))) {
                hits.push(("prose.connective", (*word).to_string()));
            }
            let mut ws = words(&lower);
            if ws.any(|w| w == "not") && ws.any(|w| w == "but") {
                hits.push(("prose.contrast", "not X but Y".to_string()));
            }
            let degree = words(&lower).find(|w| EN_DEGREE.contains(w)).map(str::to_string);
            if let Some(word) = degree {
                hits.push(("prose.degree", word));
            }
        }
    }
    hits
}

/// Prose rules over one normative section.
fn prose(out: &mut Out, section: &Section, shipped: Shipped) {
    let key = Some(section.key.as_str());
    let body = section.body();
    if body.is_empty() {
        return;
    }
    for block in blocks(section) {
        let all = sentences(&block.text);
        for (offset, sentence) in &all {
            let line = block.line_at(*offset);
            for (rule, phrase) in phrase_hits(shipped, sentence) {
                out.push(rule, key, line, excerpt(sentence), format!("matched `{phrase}`; say what the system does or the user sees instead"));
            }
            // A flow step is three clauses by design ("sees → does →
            // becomes"), so each clause is measured on its own.
            let longest = sentence
                .split('→')
                .map(|clause| match shipped {
                    Shipped::En => words(clause).count(),
                    Shipped::Ko => plain(clause).chars().count(),
                })
                .max()
                .unwrap_or(0);
            let (threshold, unit) = match shipped {
                Shipped::En => (EN_SENTENCE_WORDS, "words"),
                Shipped::Ko => (KO_SENTENCE_CHARS, "characters"),
            };
            if longest > threshold {
                out.push("prose.sentence-length", key, line, excerpt(sentence), format!("{longest} {unit}; threshold {threshold}"));
            }
        }
        if all.len() > PARAGRAPH_SENTENCES {
            out.push("prose.paragraph-length", key, block.line_at(0), excerpt(&block.text), format!("{} sentences; threshold {PARAGRAPH_SENTENCES}", all.len()));
        }
    }
    let marked: usize = body.iter().map(|l| l.text.matches("==**").count()).sum();
    if marked != 1 {
        let line = body[0].no;
        let (excerpt_text, hint) = if marked == 0 {
            ("no key sentence".to_string(), "mark the one sentence a reader must not miss as `==**…**==`".to_string())
        } else {
            (format!("{marked} key sentences"), "keep one `==**…**==` per normative section".to_string())
        };
        out.push("prose.key-sentence", key, line, excerpt_text, hint);
    }
}

// ───────────────────────────── loading ─────────────────────────────

/// The phase the flow-coverage rules need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// The brief is not approved yet, or no adopting subject is
    /// `building` yet: its scenarios are not due.
    Shaping,
    /// The brief carries an approval and an adopting subject is
    /// `building`, `verifying` or `shipped`.
    Building,
    /// The ledger could not be read, so the phase is not known and the
    /// coverage rules are skipped.
    Unknown,
}

/// The phase of `change`, whose brief reads `text`. Scenarios are written
/// after the human approves the scope (0.19 D3), so an unapproved brief is
/// `Shaping` even when its subject is already past `building`: a new
/// change on a shipped subject is not judged on scenarios that cannot
/// exist yet (dogfood eval 7).
pub fn phase_of(repo: &Path, change: &ChangeId, text: &str) -> Phase {
    use canon_gate::spec_coverage::change_subjects;
    use canon_gate::{GateContext, GateCtx};
    if field(text, "Approved:").is_none() {
        return Phase::Shaping;
    }
    let registry = canon_policy::SchemaRegistry::load();
    let Ok(ctx) = GateCtx::from_repo(repo) else { return Phase::Unknown };
    let Ok(ctx) = GateContext::load(ctx, &registry, chrono::Utc::now()) else { return Phase::Unknown };
    let building = change_subjects(&ctx, change).iter().any(|s| matches!(s.status, SubjectStatus::Building | SubjectStatus::Verifying | SubjectStatus::Shipped));
    if building {
        Phase::Building
    } else {
        Phase::Shaping
    }
}

/// A brief read from disk and judged.
pub struct Checked {
    pub change: ChangeId,
    /// The brief, relative to the repo root.
    pub brief: String,
    pub language: Language,
    pub phase: Phase,
    pub evaluation: Evaluation,
}

fn rel(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo).unwrap_or(path).display().to_string()
}

/// The change to check: `requested`, or the repo's only live change.
fn resolve_change(repo: &Path, requested: Option<&ChangeId>) -> Result<(ChangeId, PathBuf), CliError> {
    if let Some(change) = requested {
        return crate::brief::canon_brief(repo, change)
            .map(|path| (change.clone(), path))
            .ok_or_else(|| CliError::failed("BRIEF_NOT_FOUND", format!("no brief at {}/{}/brief.md", paths::CHANGES_DIR, change.as_str())));
    }
    let dir = repo.join(paths::CHANGES_DIR);
    let mut live: Vec<(ChangeId, PathBuf)> = std::fs::read_dir(&dir)
        .map(|entries| entries.filter_map(Result::ok).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.file_name() != "archive" && e.path().join("brief.md").is_file())
        .filter_map(|e| Some((ChangeId::parse(e.file_name().to_str()?).ok()?, e.path().join("brief.md"))))
        .collect();
    live.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    match live.len() {
        0 => Err(CliError::failed("NO_BRIEF", format!("no brief under {}/; write one with `canon change new`", paths::CHANGES_DIR))),
        1 => Ok(live.remove(0)),
        _ => Err(CliError::failed(
            "CHANGE_AMBIGUOUS",
            format!("{} live changes have a brief ({}); pass --change <slug>", live.len(), live.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>().join(", ")),
        )),
    }
}

/// Reads and judges the brief of `requested` (or the only live change).
pub fn check(repo: &Path, requested: Option<&ChangeId>) -> Result<Checked, CliError> {
    let (change, path) = resolve_change(repo, requested)?;
    let text = std::fs::read_to_string(&path).map_err(|e| CliError::failed("BRIEF_UNREADABLE", format!("cannot read `{}`: {e}", rel(repo, &path))))?;
    let language = crate::language::configured(repo).map_err(|e| CliError::failed("CONFIG_UNREADABLE", e))?;
    let phase = phase_of(repo, &change, &text);
    let flows = match phase {
        Phase::Building if !is_chore(&text) => Some(flow_scenarios(repo, &change).map_err(|e| CliError::failed("SPEC_ROOTS_UNREADABLE", format!("cannot read the scenarios the flow steps point at: {e}")))?),
        _ => None,
    };
    let evaluation = evaluate(&text, &language, flows.as_ref());
    Ok(Checked { change, brief: rel(repo, &path), language, phase, evaluation })
}

/// The blocker findings the ship guard refuses on, for `change`'s brief:
/// empty when it has no canon brief, or a legacy one, or when the
/// scenarios the flow rules read cannot be (the guard's own
/// spec-root check names that).
pub fn guard_blockers(repo: &Path, change: &ChangeId) -> Vec<Finding> {
    let Some(path) = crate::brief::canon_brief(repo, change) else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(&path) else { return Vec::new() };
    let language = crate::language::configured(repo).unwrap_or_else(|_| Language::unset());
    let flows = flow_scenarios(repo, change).ok();
    let mut findings = evaluate(&text, &language, flows.as_ref()).findings;
    findings.retain(|f| f.severity == Severity::Blocker);
    findings
}

/// The finding counts `canon status` shows for `change`'s brief: `None`
/// for a legacy brief (read as before, nothing shown) or no brief.
pub fn status_counts(repo: &Path, change: &ChangeId, building: bool) -> Option<Counts> {
    let path = crate::brief::canon_brief(repo, change)?;
    let text = std::fs::read_to_string(&path).ok()?;
    let language = crate::language::configured(repo).unwrap_or_else(|_| Language::unset());
    let flows = if building && field(&text, "Approved:").is_some() { flow_scenarios(repo, change).ok() } else { None };
    let evaluation = evaluate(&text, &language, flows.as_ref());
    (!evaluation.legacy).then(|| evaluation.counts())
}

// ───────────────────────────── commands ─────────────────────────────

fn fail(json: bool, error: &CliError) -> i32 {
    error.emit("canon brief", json)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckJson<'a> {
    change: &'a str,
    brief: &'a str,
    language: &'a str,
    phase: Phase,
    legacy: bool,
    approved: bool,
    counts: Counts,
    findings: &'a [Finding],
    waivers: &'a [Waiver],
}

/// `canon brief check [--change <slug>] [--json]`.
pub fn run_check(repo: &Path, change: Option<&ChangeId>, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let checked = match check(&repo, change) {
        Ok(checked) => checked,
        Err(e) => return fail(json, &e),
    };
    let ev = &checked.evaluation;
    let counts = ev.counts();
    if json {
        let body = CheckJson {
            change: checked.change.as_str(),
            brief: &checked.brief,
            language: &checked.language.tag,
            phase: checked.phase,
            legacy: ev.legacy,
            approved: ev.approved,
            counts,
            findings: &ev.findings,
            waivers: &ev.waivers,
        };
        match serde_json::to_value(&body) {
            Ok(mut value) => {
                if counts.blockers > 0 {
                    // One document: the findings keep their fields and gain the shared `error`.
                    let error = CliError::refused("BRIEF_HAS_BLOCKERS", format!("{} blocker(s) in {}", counts.blockers, checked.brief)).hint("fix each blocker in `findings`, then run `canon brief check` again");
                    eprintln!("{}", error.human("canon brief check"));
                    value["error"] = error.to_json()["error"].clone();
                }
                println!("{}", serde_json::to_string_pretty(&value).expect("a JSON value always serializes"));
            }
            Err(e) => return CliError::failed("SERIALIZE_FAILED", e.to_string()).emit("canon brief check", true),
        }
    } else {
        println!("canon brief check: {} ({}) — {} blocker(s), {} advisory(ies)", checked.brief, checked.language.tag, counts.blockers, counts.advisories);
        if checked.phase == Phase::Unknown {
            println!("  note: the ledger cannot be read, so the flow-coverage rules were skipped");
        }
        for f in &ev.findings {
            let section = f.section.as_deref().map(|s| format!(" [{s}]")).unwrap_or_default();
            println!("  {} {}{section} line {}: {}", f.severity.as_str(), f.rule, f.line, f.excerpt);
            println!("    hint: {}", f.hint);
        }
        for w in &ev.waivers {
            let severity = w.severity.map_or("unknown", Severity::as_str);
            let state = if w.applied { "waived" } else { "waiver matches nothing" };
            println!("  {state}: {severity} {} [{}] line {} — \"{}\"", w.rule, w.section, w.line, w.reason);
        }
    }
    if counts.blockers > 0 {
        EXIT_BLOCKERS
    } else {
        0
    }
}

/// One next step of bare `canon brief`.
#[derive(Debug, Serialize)]
struct Step {
    step: &'static str,
    why: String,
}

/// The `approve` step: `fresh` when the brief was never approved, the
/// re-approval when its text changed after the approval (0.19 D3).
fn approve_step(ev: &Evaluation, fresh: &str) -> Step {
    let why = if matches!(ev.approval, crate::brief::Approval::Changed { .. }) {
        format!("the brief changed after its approval ({}); the human answers the changed brief and `canon brief approve` records it", crate::brief::CHANGED_CODE)
    } else {
        format!("{fresh}; `canon brief approve --by <the human> --note \"<their words>\"` records it")
    };
    Step { step: "approve", why }
}

/// The next one to three steps for `ev` (module doc).
fn next_steps(ev: &Evaluation) -> Vec<Step> {
    let mut steps = Vec::new();
    if ev.legacy {
        if !ev.approved {
            steps.push(approve_step(ev, "the brief has no `Approved:` line; the human answers it"));
        }
        return steps;
    }
    if ev.has(&["brief.section-missing", "brief.section-duplicate", "brief.section-order", "brief.placeholder", "brief.flow-no-steps", "brief.waiver-invalid", "brief.chore-section", "brief.chore-category", "brief.chore-empty"]) {
        steps.push(Step { step: "shape", why: "sections are missing, out of order or still hold the template's placeholders".to_string() });
    }
    if ev.open_questions > 0 {
        steps.push(Step { step: "clarify", why: format!("{} line(s) under the questions section are still open", ev.open_questions) });
    }
    if ev.has(&["brief.flow-duplicate-id", "brief.detail-mismatch"]) {
        steps.push(Step { step: "harden", why: "the step detail does not match the flow's step ids".to_string() });
    }
    if ev.has(&["brief.flow-uncovered", "brief.flow-no-failure"]) {
        steps.push(Step { step: "align", why: "a flow step has no scenario, or no failure scenario".to_string() });
    }
    if ev.findings.iter().any(|f| f.rule.starts_with("prose.")) {
        steps.push(Step { step: "polish", why: "prose advisories remain in the normative sections".to_string() });
    }
    if ev.counts().blockers == 0 && !ev.approved {
        steps.push(approve_step(ev, "no blockers; the human answers the brief"));
    }
    steps.truncate(3);
    steps
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NextJson<'a> {
    change: &'a str,
    brief: &'a str,
    state: &'static str,
    counts: Counts,
    next: Vec<Step>,
}

/// Bare `canon brief [--change <slug>] [--json]`: state and next steps.
pub fn run_next(repo: &Path, change: Option<&ChangeId>, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let checked = match check(&repo, change) {
        Ok(checked) => checked,
        Err(e) => return fail(json, &e),
    };
    let ev = &checked.evaluation;
    let counts = ev.counts();
    let changed = matches!(ev.approval, crate::brief::Approval::Changed { .. });
    let state = match (ev.legacy, ev.approved, counts.blockers) {
        _ if changed => "changed-since-approval",
        (true, true, _) => "legacy-approved",
        (true, false, _) => "legacy",
        (false, true, _) => "approved",
        (false, false, 0) => "ready",
        (false, false, _) => "draft",
    };
    let next = next_steps(ev);
    if json {
        let body = NextJson { change: checked.change.as_str(), brief: &checked.brief, state, counts, next };
        println!("{}", serde_json::to_string_pretty(&body).unwrap_or_default());
    } else {
        println!("canon brief: {} — {state} ({} blocker(s), {} advisory(ies))", checked.brief, counts.blockers, counts.advisories);
        if next.is_empty() {
            println!("  next: nothing — the brief is approved and clean");
        }
        for (i, step) in next.iter().enumerate() {
            println!("  next {}: {} — {}", i + 1, step.step, step.why);
        }
        println!("  `canon brief check` lists the findings");
    }
    0
}
