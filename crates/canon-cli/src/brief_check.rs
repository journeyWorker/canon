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
//! - **Advisories (prose, `prose.*`)**: closed per-language phrase lists
//!   (evaluative, connective, contrast, colloquial, degree, and the stock
//!   AI vocabulary `prose.ai-vocabulary`), sentence and paragraph length,
//!   a normative section without exactly one `==**key sentence**==`, and
//!   the 0.26 rules each reading one mechanism: `prose.code-citation`
//!   (file paths, `path:line`, code identifiers in a code span),
//!   `prose.parenthetical` (asides per section above a density),
//!   `prose.decision-history` (dates and decision notes), `prose.ko-ending`
//!   (a Korean sentence that is not a plain `~다`) and `prose.length` (a
//!   character budget per section and for the brief). The finding carries
//!   the matched text or the measured value and the threshold, never an
//!   authorship verdict.
//!
//!   Where they apply (0.26 D2): the phrase lists and the new rules read
//!   `role=normative` sections, `intent` and question sections
//!   (decision-history: normative and `intent` only); sentence length,
//!   paragraph length and the key sentence read normative sections only;
//!   `prose.length` reads every section that has a budget. A rule applies
//!   in the language its section is written in
//!   ([`crate::language::for_brief`], [`crate::language::section_language`]):
//!   `ja`/`zh` text gets the structure rules and no prose rules. A
//!   `Source: <path>` line is exempt from every prose rule, and so is the
//!   `Approved:` line.
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
//! switch. A finding about the whole brief (no section: the total of
//! `prose.length`) is waived with the section key `brief`.
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
use crate::language::{Language, Resolved, Shipped, Tally};

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
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Blocker => "blocker",
            Severity::Advisory => "advisory",
        }
    }
}

/// Every rule id and its severity. A waiver names one of these.
pub const RULES: [(&str, Severity); 28] = [
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
    ("prose.code-citation", Severity::Advisory),
    ("prose.parenthetical", Severity::Advisory),
    ("prose.decision-history", Severity::Advisory),
    ("prose.ai-vocabulary", Severity::Advisory),
    ("prose.ko-ending", Severity::Advisory),
    ("prose.length", Severity::Advisory),
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
        Counts { blockers, advisories: self.findings.len() - blockers }
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
/// Gherkin scan `canon inventory sync` indexes them with (one scan per
/// file per process, [`crate::inventory::feature_file_scan`]), so the
/// gate, status and the brief rules cannot disagree, and a tag written a
/// moment ago counts without a sync.
pub fn flow_scenarios(repo: &Path, change: &ChangeId) -> Result<FlowIndex, String> {
    let roots = crate::inventory::load_spec_roots(&repo.join("canon.yaml")).map_err(|e| e.to_string())?;
    let prefix = format!("{}.", change.as_str());
    let mut index = FlowIndex::new();
    for root in roots {
        for path in canon_fmt::util::feature_files(&root.root) {
            let Some(file) = crate::inventory::feature_file_scan(&path) else { continue };
            for scenario in &file.scan.scenarios {
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

/// The changes the `@flow:<change>.<F-id>` tags of scenario `scenario_id`
/// in spec root `project` name (0.25 D1), in tag order, read through the
/// same Gherkin scan as [`flow_scenarios`]. Empty for an untagged
/// scenario or an unreadable spec configuration.
pub fn flow_changes_of(
    repo: &Path,
    project: &canon_model::ProjectId,
    scenario_id: &canon_model::ScenarioId,
) -> Vec<ChangeId> {
    let Ok(roots) = crate::inventory::load_spec_roots(&repo.join("canon.yaml")) else {
        return Vec::new();
    };
    let mut changes: Vec<ChangeId> = Vec::new();
    for root in roots.iter().filter(|root| &root.id == project) {
        for path in canon_fmt::util::feature_files(&root.root) {
            let Some(file) = crate::inventory::feature_file_scan(&path) else {
                continue;
            };
            for scenario in file
                .scan
                .scenarios
                .iter()
                .filter(|s| s.scenario_id == scenario_id.as_str())
            {
                for tag in &scenario.flow_tags {
                    if let Some(change) = tag
                        .rsplit_once('.')
                        .and_then(|(change, _)| ChangeId::parse(change).ok())
                    {
                        if !changes.contains(&change) {
                            changes.push(change);
                        }
                    }
                }
            }
        }
    }
    changes
}

/// The flow step ids `change`'s brief defines, in order; empty for a
/// legacy brief or no brief.
pub fn flow_ids(repo: &Path, change: &ChangeId) -> Vec<String> {
    let Some(path) = crate::brief::canon_brief(repo, change) else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(&path) else { return Vec::new() };
    let language = Language::unset();
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
        (None, None) => resolve_change(repo, None).map(|(change, _)| change).map_err(|e| format!("{} — {}; pass --change-id or --subject-id", e.code, e.message))?,
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
    subject_change_ids(&ctx, found).pop().ok_or_else(|| format!("subject `{}` has not adopted a change; run `canon change new <slug> --subject-id {}`", subject.as_str(), subject.as_str()))
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
pub fn evaluate(text: &str, resolved: &Resolved, flows: Option<&FlowIndex>) -> Evaluation {
    let language = &resolved.language;
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
    let judged: Vec<&Section> = secs.iter().filter(|s| kind == ChangeKind::Feature || kind.sections().iter().any(|k| k.key == s.key)).collect();
    for section in &judged {
        let Some(scope) = scope_of(section) else { continue };
        let Some(shipped) = crate::language::section_language(resolved, &section_tally(section)).prose_rules() else { continue };
        prose(&mut out, section, shipped, scope);
    }
    lengths(&mut out, &judged, resolved);
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
                kept.retain(|f| !(f.rule == rule && f.section.as_deref().unwrap_or("brief") == section));
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

// ── 0.26 D3 lists and thresholds ──
//
// Calibration (0.26). Each threshold below was set against four corpora:
// canon's own briefs (the 11 `conformance/cases/**/brief.md`, which are
// legacy or template briefs, and the 6 eval-13 fixture briefs, all
// English), the public fixtures under `tests/fixtures/briefs/` (3 bad
// Korean, 2 good Korean, 2 good English, invented products), and, as an
// aggregate only, 14 Korean briefs of a private consumer repo (P1..P14,
// 2.1k-13.3k characters of text, all detected `ko`). The rule: a bad
// fixture and every private brief that shows a mechanism get a finding
// for it; the good fixtures and canon's own briefs get none.
//
// Briefs with at least one finding / findings, per corpus:
//
// | rule                   | private (14) | bad fixtures (3) | good fixtures (4) | canon's own (17) |
// |------------------------|--------------|------------------|-------------------|------------------|
// | prose.code-citation    | 14 / 473     | 3 / 50           | 0                 | 0                |
// | prose.parenthetical    | 14 / 38      | 3 / 11           | 0                 | 0                |
// | prose.decision-history | 14 / 28      | 3 / 24           | 0                 | 0                |
// | prose.ai-vocabulary    | 0 / 0        | 3 / 7            | 0                 | 0                |
// | prose.ko-ending        | 8 / 17       | 3 / 11           | 0                 | 0                |
// | prose.length           | 14 / 71      | 3 / 4            | 0                 | 0                |
//
// Notes. `prose.ai-vocabulary`: the private briefs hold none of the
// listed words, so the Korean list rests on the stock words themselves
// and on the bad fixtures, not on a measured hit. `prose.ko-ending`: the
// private briefs write plain `~다` almost everywhere (1420 sentences);
// the 17 findings are polite `~니다` sentences in 8 briefs, and every
// other non-`~다` sentence ending in them is a list-item fragment, which
// is why a fragment is exempt. `prose.parenthetical`: private sections
// carry 4-10 asides per 1000 characters, a good fixture none, so the
// line sits at more than 4 per 1000 with at least 3 asides. `prose.length`:
// private Intent sections measure 600-1100 characters (budget 400), whole
// briefs 2100-13300 (budget 4000); every private brief is over at least
// one section budget, 11 of 14 over the total. Detection: the private
// briefs are 43-66% Hangul of their raw letters, code and all, and more
// once code is removed; the 30% line is covered by the language unit
// tests and the mixed-script scenarios.

/// Stock AI words and metaphors, lowercase. `(term, prefix)`: a `prefix`
/// term also matches the words it starts (`leverag` → leverages); any
/// term needs a word boundary before it.
const EN_AI_VOCABULARY: [(&str, bool); 19] = [
    ("seamless", true),
    ("robust", true),
    ("leverag", true),
    ("delve", true),
    ("delving", false),
    ("tapestry", false),
    ("north star", false),
    ("holistic", true),
    ("pivotal", false),
    ("cornerstone", false),
    ("bedrock", false),
    ("streamlin", true),
    ("empower", true),
    ("game-changer", true),
    ("cutting-edge", false),
    ("ever-evolving", false),
    ("in the realm of", false),
    ("plays a crucial role", false),
    ("testament to", false),
];
const KO_AI_VOCABULARY: [&str; 14] = ["원활", "매끄럽", "견고", "탄탄", "유기적", "시너지", "혁신적", "패러다임", "북극성", "나침반", "초석", "토대", "청사진", "한 단계 도약"];

/// Phrases that make a sentence a decision note (`prose.decision-history`):
/// who decided what, and when it changed.
const EN_DECISION_NOTES: [&str; 9] = ["we decided", "was decided", "were decided", "we agreed", "was agreed", "agreed on", "changed on", "after discussion", "in the meeting"];
const KO_DECISION_NOTES: [&str; 11] = ["결정했", "결정하였", "결정됐", "결정되었", "합의", "논의 결과", "논의를 거", "하기로 했", "하기로 하였", "변경했", "바꾸기로"];
const EN_MONTHS: [&str; 23] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December", "Jan", "Feb", "Mar", "Apr", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// File extensions that make a bare token a file citation.
const SOURCE_EXTENSIONS: [&str; 34] = [
    "rs", "ts", "tsx", "js", "jsx", "mjs", "py", "go", "java", "kt", "swift", "rb", "php", "c", "h", "cc", "cpp", "cs", "md", "json", "yaml", "yml", "toml", "sql", "html", "css", "scss", "sh", "feature", "lock", "txt", "xml", "proto", "vue",
];
/// First path segments that make `dir/name` a file citation.
const SOURCE_DIRS: [&str; 13] = ["src", "crates", "lib", "app", "tests", "test", "docs", "packages", "internal", "cmd", "pkg", "scripts", "config"];

/// Imperative and hortative Korean endings `prose.ko-ending` reports
/// (the polite `…요` endings in [`KO_COLLOQUIAL`] are `prose.colloquial`'s).
const KO_IMPERATIVE: [&str; 12] = ["하라", "마라", "말라", "보라", "가라", "어라", "아라", "으라", "오라", "두라", "쓰라", "내라"];
const KO_HORTATIVE: [&str; 4] = ["하자", "보자", "가자", "두자"];

/// `prose.parenthetical` fires on a section with at least
/// [`PAREN_MIN`] asides and more than [`PAREN_PER_1000`] of them per
/// 1000 characters of its text.
const PAREN_MIN: usize = 3;
const PAREN_PER_1000: usize = 4;
/// An aside of this many characters or fewer with no space (`(F1)`,
/// `(30초)`, `(ms)`) is a label, not an aside.
const PAREN_LABEL_CHARS: usize = 6;

/// `prose.length` budgets, in characters of a section's text (headings,
/// comments, code fences and `Source:`/`Approved:` lines excluded):
/// `(section key, ko, en)`. A Korean character carries about twice what
/// an English one does, so the Korean budgets are about half. They sit a
/// little above the longest good fixture section and well below what the
/// private briefs measure (intent 600-1100, detail 2300-7600, total
/// 2100-13300 characters).
const LENGTH_BUDGETS: [(&str, usize, usize); 9] = [
    ("intent", 400, 700),
    ("outcome", 500, 900),
    ("flow", 800, 1500),
    ("detail", 2500, 3500),
    ("acceptance", 1000, 1800),
    ("questions", 800, 1200),
    ("out-of-scope", 300, 600),
    ("invariants", 800, 1500),
    ("checks", 800, 1500),
];
/// `prose.length`'s budget for the sum of every section, `(ko, en)`.
const TOTAL_BUDGET: (usize, usize) = (4000, 6000);

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
    /// The block is one list item (a fragment is allowed there).
    item: bool,
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
        if crate::language::is_meta_line(trimmed) {
            out.extend(current.take());
            continue;
        }
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
        let block = current.get_or_insert_with(|| Block { text: String::new(), starts: Vec::new(), item });
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
    if let Some(term) = match shipped {
        Shipped::En => en_ai_vocabulary(&s.to_lowercase()),
        Shipped::Ko => KO_AI_VOCABULARY.iter().find(|w| s.contains(**w)).map(|w| (*w).to_string()),
    } {
        hits.push(("prose.ai-vocabulary", term));
    }
    hits
}

/// How much of the prose rules a section gets (0.26 D2): the phrase
/// lists and the new rules read normative sections, `intent` and
/// question sections; length, paragraph and key-sentence rules read
/// normative sections only; a decision note is judged in normative and
/// `intent` text, not in a question.
#[derive(Clone, Copy)]
struct Scope {
    structure: bool,
    history: bool,
}

fn scope_of(section: &Section) -> Option<Scope> {
    match (section.role, section.key.as_str()) {
        (Role::Normative, _) => Some(Scope { structure: true, history: true }),
        (_, "intent") => Some(Scope { structure: false, history: true }),
        (Role::Question, _) => Some(Scope { structure: false, history: false }),
        _ => None,
    }
}

/// The matched values of a finding, at most three, then a count.
fn listed(values: &[String]) -> String {
    let shown: Vec<String> = values.iter().take(3).map(|v| format!("`{v}`")).collect();
    match values.len() - shown.len() {
        0 => shown.join(", "),
        more => format!("{} and {more} more", shown.join(", ")),
    }
}

/// Whether `candidate` (a token or the inside of a code span) cites a
/// file: `path:line`, a name with a source extension, a path of two or
/// more segments, or `dir/name` under a conventional source directory.
/// A leading `/` with no extension is a route the user sees, a `://`
/// token a URL; neither is a file.
fn is_path(candidate: &str) -> bool {
    let mut core = candidate.trim();
    if core.is_empty() || core.contains("://") || !core.is_ascii() || core.contains(char::is_whitespace) {
        return false;
    }
    let mut has_line = false;
    loop {
        let stripped = core
            .rsplit_once(':')
            .or_else(|| core.rsplit_once("#L"))
            .filter(|(head, tail)| !head.is_empty() && tail.starts_with(|c: char| c.is_ascii_digit()) && tail.chars().all(|c| c.is_ascii_digit() || c == '-'));
        match stripped {
            Some((head, _)) => {
                core = head;
                has_line = true;
            }
            None => break,
        }
    }
    let core = core.trim_end_matches('/');
    if !core.bytes().all(|b| b.is_ascii_alphanumeric() || b"_./-~@+".contains(&b)) || !core.bytes().any(|b| b.is_ascii_alphabetic()) {
        return false;
    }
    let last = core.rsplit('/').next().unwrap_or(core);
    let has_ext = last.rsplit_once('.').is_some_and(|(stem, ext)| !stem.is_empty() && SOURCE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()));
    let slashes = core.matches('/').count();
    if core.starts_with('/') && !has_ext && !has_line {
        return false;
    }
    has_ext
        || (has_line && slashes >= 1)
        || (slashes >= 2 && !core.starts_with('/'))
        || ["./", "../", "~/"].iter().any(|p| core.starts_with(p))
        || (slashes == 1 && core.split('/').next().is_some_and(|dir| SOURCE_DIRS.contains(&dir)))
}

/// Whether the inside of a code span names a code identifier: `a::b`,
/// `f()`, `snake_case`, `obj.member`, `camelCase`. A span with a space,
/// non-ASCII text or a leading `-` (a flag, a UI label, a sentence) is not.
fn is_identifier(span: &str) -> bool {
    let s = span.trim();
    if s.is_empty() || !s.is_ascii() || s.contains(char::is_whitespace) || s.starts_with('-') {
        return false;
    }
    let b = s.as_bytes();
    s.contains("::")
        || s.contains("->")
        || (s.contains('(') && s.ends_with(')'))
        || (s.contains('_') && b.iter().any(u8::is_ascii_alphabetic))
        || b.windows(3).any(|w| w[1] == b'.' && w[0].is_ascii_alphabetic() && w[2].is_ascii_alphabetic())
        || b.windows(2).any(|w| w[0].is_ascii_lowercase() && w[1].is_ascii_uppercase())
}

/// The code citations of one sentence, as written: file paths and
/// `path:line` in the text or in a code span, and code identifiers in a
/// code span.
fn citations(sentence: &str) -> Vec<String> {
    let mut spans: Vec<String> = Vec::new();
    let mut outside = String::new();
    let mut open: Option<String> = None;
    for c in sentence.chars() {
        match (c, open.as_mut()) {
            ('`', None) => open = Some(String::new()),
            ('`', Some(_)) => {
                spans.extend(open.take());
                outside.push(' ');
            }
            (_, Some(span)) => span.push(c),
            (_, None) => outside.push(c),
        }
    }
    let mut found: Vec<String> = spans.into_iter().filter(|s| is_path(s) || is_identifier(s)).collect();
    for token in outside.split_whitespace() {
        let token = token.trim_start_matches(['(', '[', '{', '<', '"', '\'', '“', '‘']);
        let ascii = token.split(|c: char| !c.is_ascii() || c.is_whitespace()).next().unwrap_or("");
        let ascii = ascii.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '>', '"', '\'']);
        if is_path(ascii) {
            found.push(ascii.to_string());
        }
    }
    found
}

/// Top-level `(…)` and `（…）` asides of `text` with their byte offsets,
/// outside code spans, labels ([`PAREN_LABEL_CHARS`]) excluded.
fn asides(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut in_code = false;
    let mut depth = 0usize;
    let mut start = 0;
    for (at, c) in text.char_indices() {
        match c {
            '`' => in_code = !in_code,
            _ if in_code => {}
            '(' | '（' => {
                if depth == 0 {
                    start = at;
                }
                depth += 1;
            }
            ')' | '）' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    let inner = text[start + c_len_at(text, start)..at].trim();
                    if !(inner.chars().count() <= PAREN_LABEL_CHARS && !inner.contains(char::is_whitespace)) {
                        out.push((start, inner.to_string()));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Byte length of the character at `at`.
fn c_len_at(text: &str, at: usize) -> usize {
    text[at..].chars().next().map_or(0, char::len_utf8)
}

/// The first date `s` writes: `2026-10-21` (also `/` and `.`),
/// `10월 21일`, `2026년 10월`, and for `english` also `Oct 21`,
/// `October 21`, `21 October`.
fn find_date(s: &str, english: bool) -> Option<String> {
    let ch: Vec<char> = s.chars().collect();
    let run = |from: usize| ch[from.min(ch.len())..].iter().take_while(|c| c.is_ascii_digit()).count();
    let text = |a: usize, b: usize| ch[a..b].iter().collect::<String>();
    for i in 0..ch.len() {
        if !ch[i].is_ascii_digit() || (i > 0 && (ch[i - 1].is_alphanumeric() || matches!(ch[i - 1], '.' | '-' | '/'))) {
            continue;
        }
        let n = run(i);
        let after = i + n;
        if n == 4 {
            if let Some(&sep) = ch.get(after).filter(|c| matches!(c, '-' | '/' | '.')) {
                let m = run(after + 1);
                if (1..=2).contains(&m) && ch.get(after + 1 + m) == Some(&sep) {
                    let d = run(after + 2 + m);
                    if (1..=2).contains(&d) {
                        return Some(text(i, after + 2 + m + d));
                    }
                }
            }
            if ch.get(after) == Some(&'년') {
                let mut k = after + 1;
                while ch.get(k) == Some(&' ') {
                    k += 1;
                }
                let m = run(k);
                if (1..=2).contains(&m) && ch.get(k + m) == Some(&'월') {
                    return Some(text(i, k + m + 1));
                }
            }
        }
        if n <= 2 && ch.get(after) == Some(&'월') {
            let mut k = after + 1;
            while ch.get(k) == Some(&' ') {
                k += 1;
            }
            let d = run(k);
            if (1..=2).contains(&d) && ch.get(k + d) == Some(&'일') {
                return Some(text(i, k + d + 1));
            }
        }
    }
    if english {
        let tokens: Vec<&str> = s.split_whitespace().collect();
        let day = |t: &str| {
            let t = t.trim_matches(|c: char| !c.is_alphanumeric());
            let digits = t.chars().take_while(char::is_ascii_digit).count();
            (1..=2).contains(&digits) && matches!(&t[digits..], "" | "st" | "nd" | "rd" | "th")
        };
        let month = |t: &str| EN_MONTHS.contains(&t.trim_matches(|c: char| !c.is_alphanumeric()));
        for (i, t) in tokens.iter().enumerate() {
            let name = t.trim_matches(|c: char| !c.is_alphanumeric());
            if !month(name) || (name == "May" && i == 0) {
                continue;
            }
            if tokens.get(i + 1).is_some_and(|n| day(n)) {
                return Some(format!("{name} {}", tokens[i + 1].trim_matches(|c: char| !c.is_alphanumeric())));
            }
            if i > 0 && day(tokens[i - 1]) {
                return Some(format!("{} {name}", tokens[i - 1].trim_matches(|c: char| !c.is_alphanumeric())));
            }
        }
    }
    None
}

/// The decision-history evidence of one sentence: a date, else a
/// decision-note phrase.
fn decision_history(shipped: Shipped, sentence: &str) -> Option<String> {
    let s = plain(sentence);
    if let Some(date) = find_date(&s, shipped == Shipped::En) {
        return Some(date);
    }
    match shipped {
        Shipped::Ko => KO_DECISION_NOTES.iter().find(|p| s.contains(**p)).map(|p| (*p).to_string()),
        Shipped::En => {
            let lower = s.to_lowercase();
            EN_DECISION_NOTES.iter().find(|p| lower.contains(**p)).map(|p| (*p).to_string())
        }
    }
}

/// The first of [`EN_AI_VOCABULARY`] in lowercase `s`, with a word
/// boundary before it (and after it for a whole-word term).
fn en_ai_vocabulary(s: &str) -> Option<String> {
    EN_AI_VOCABULARY.iter().find_map(|(term, prefix)| {
        s.match_indices(term).find_map(|(at, _)| {
            let before = s[..at].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
            let after = *prefix || s[at + term.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric());
            (before && after).then(|| (*term).to_string())
        })
    })
}

/// How a Korean sentence ends when it is not the plain declarative `~다`
/// and not an ending `prose.colloquial` already owns, as the matched
/// ending. `None` for a plain `~다`, a question, a sentence that ends in
/// code, a quote or a non-Hangul character, a flow step (`→` clauses), a
/// "none" answer, and, for a list item, a fragment: only a polite,
/// imperative or hortative ending is reported there.
fn ko_ending(sentence: &str, item: bool) -> Option<String> {
    if sentence.contains('→') || is_none_answer(sentence) {
        return None;
    }
    let raw = sentence.trim().trim_end_matches(['.', '!', '。', '*', '=', ' ']);
    if raw.ends_with(['`', '"', '\'', '”', '’', '」', '』', '>', '?', '？']) || sentence.trim_end().trim_end_matches(['*', '=']).ends_with(['?', '？']) {
        return None;
    }
    let terminated = sentence.trim_end().trim_end_matches(['*', '=']).ends_with(['.', '!', '。']);
    let s = plain(&strip_asides(raw));
    let end = s.trim_end_matches(|c: char| !c.is_alphanumeric());
    let last = end.chars().next_back()?;
    if !('\u{AC00}'..='\u{D7A3}').contains(&last) {
        return None;
    }
    let tilde = |suffix: &str| Some(format!("~{suffix}"));
    if end.ends_with("니다") || end.ends_with("니까") {
        return tilde(&end[end.len() - 6..]);
    }
    if end.ends_with("시다") {
        return tilde("시다");
    }
    if KO_COLLOQUIAL.iter().any(|e| end.ends_with(e)) {
        return None;
    }
    if end.ends_with('요') {
        return tilde("요");
    }
    if let Some(e) = KO_IMPERATIVE.iter().find(|e| end.ends_with(**e)) {
        return tilde(&e[e.len() - 3..]);
    }
    if let Some(e) = KO_HORTATIVE.iter().find(|e| end.ends_with(**e)) {
        return tilde(&e[e.len() - 3..]);
    }
    if end.ends_with('다') || item || !terminated {
        return None;
    }
    end.split_whitespace().next_back().map(|w| format!("…{w}"))
}

/// `s` without its parenthesized asides.
fn strip_asides(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '(' | '（' => depth += 1,
            ')' | '）' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// The 0.26 D3 hits of one sentence: `(rule, hint)`.
fn sentence_hits(shipped: Shipped, sentence: &str, item: bool, scope: Scope) -> Vec<(&'static str, String)> {
    let mut hits = Vec::new();
    let cited = citations(sentence);
    if !cited.is_empty() {
        hits.push(("prose.code-citation", format!("matched {}; describe the behavior, and put provenance on one `Source:` line", listed(&cited))));
    }
    if scope.history {
        if let Some(note) = decision_history(shipped, sentence) {
            hits.push(("prose.decision-history", format!("matched `{note}`; state the behavior as it is; when it was decided and what changed belong in the commit or the plan")));
        }
    }
    if shipped == Shipped::Ko {
        if let Some(ending) = ko_ending(sentence, item) {
            hits.push(("prose.ko-ending", format!("ends in `{ending}`; write the plain declarative ending ~다")));
        }
    }
    hits
}

/// `prose.parenthetical` over one section's blocks.
fn parentheticals(out: &mut Out, section: &Section, blocks: &[Block]) {
    let mut found: Vec<(usize, String)> = Vec::new();
    let mut chars = 0;
    for block in blocks {
        chars += block.text.chars().count();
        found.extend(asides(&block.text).into_iter().map(|(at, inner)| (block.line_at(at), inner)));
    }
    let n = found.len();
    if n >= PAREN_MIN && n * 1000 > PAREN_PER_1000 * chars {
        out.push(
            "prose.parenthetical",
            Some(&section.key),
            found[0].0,
            excerpt(&format!("({})", found[0].1)),
            format!("{n} asides in {chars} characters; threshold more than {PAREN_PER_1000} per 1000 characters and at least {PAREN_MIN}; make an aside its own sentence or cut it"),
        );
    }
}

/// The characters of a section's text `prose.length` measures.
fn text_chars(section: &Section) -> usize {
    section.body().iter().filter(|l| !crate::language::is_meta_line(&l.text)).map(|l| l.text.trim().chars().count()).sum()
}

/// `prose.length`: each judged section against its budget, then the sum.
fn lengths(out: &mut Out, secs: &[&Section], resolved: &Resolved) {
    let pick = |shipped: Shipped, (ko, en): (usize, usize)| if shipped == Shipped::Ko { ko } else { en };
    let mut total = 0;
    for section in secs {
        let chars = text_chars(section);
        total += chars;
        let Some(shipped) = crate::language::section_language(resolved, &section_tally(section)).prose_rules() else { continue };
        let Some(&(_, ko, en)) = LENGTH_BUDGETS.iter().find(|(key, _, _)| *key == section.key) else { continue };
        let budget = pick(shipped, (ko, en));
        if chars > budget {
            let first = section.body().first().map_or(section.marker_line, |l| l.no);
            out.push("prose.length", Some(&section.key), first, format!("{chars} characters"), format!("{chars} characters; budget {budget} for `{}`; keep what states a condition, an action, a result or a number", section.key));
        }
    }
    if let Some(shipped) = resolved.language.prose_rules() {
        let budget = pick(shipped, TOTAL_BUDGET);
        if total > budget {
            out.push("prose.length", None, 1, format!("{total} characters in all sections"), format!("{total} characters; budget {budget} for the whole brief; waive it with section key `brief`"));
        }
    }
}

/// Prose rules over one judged section, in `shipped`, the language of
/// its own text.
fn prose(out: &mut Out, section: &Section, shipped: Shipped, scope: Scope) {
    let key = Some(section.key.as_str());
    let body = section.body();
    if body.is_empty() {
        return;
    }
    let blocks = blocks(section);
    for block in &blocks {
        let all = sentences(&block.text);
        for (offset, sentence) in &all {
            let line = block.line_at(*offset);
            for (rule, phrase) in phrase_hits(shipped, sentence) {
                out.push(rule, key, line, excerpt(sentence), format!("matched `{phrase}`; say what the system does or the user sees instead"));
            }
            for (rule, hint) in sentence_hits(shipped, sentence, block.item, scope) {
                out.push(rule, key, line, excerpt(sentence), hint);
            }
            if !scope.structure {
                continue;
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
        if scope.structure && all.len() > PARAGRAPH_SENTENCES {
            out.push("prose.paragraph-length", key, block.line_at(0), excerpt(&block.text), format!("{} sentences; threshold {PARAGRAPH_SENTENCES}", all.len()));
        }
    }
    parentheticals(out, section, &blocks);
    if !scope.structure {
        return;
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

/// Counts the prose letters of `section`'s lines.
fn section_tally(section: &Section) -> Tally {
    let mut tally = Tally::default();
    for line in &section.lines {
        tally.add_line(&line.text);
    }
    tally
}

/// The prose letters of a whole brief, for [`crate::language::resolve`].
pub fn prose_tally(text: &str) -> Tally {
    let mut tally = Tally::default();
    for line in scan(text).0 {
        tally.add_line(&line.text);
    }
    tally
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
    /// The language the brief was judged in ([`crate::language::for_brief`]),
    /// where it came from, and the note when a configured language
    /// disagrees with the text.
    pub language: Language,
    pub language_source: crate::language::Source,
    pub language_note: Option<String>,
    pub phase: Phase,
    pub evaluation: Evaluation,
}

impl Checked {
    /// The resolution, as [`crate::language::for_brief`] returned it.
    pub fn resolved(&self) -> Resolved {
        Resolved { language: self.language.clone(), source: self.language_source, note: self.language_note.clone() }
    }
}

fn rel(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo).unwrap_or(path).display().to_string()
}

/// The change to check: `requested`, or the repo's only live change.
fn resolve_change(repo: &Path, requested: Option<&ChangeId>) -> Result<(ChangeId, PathBuf), CliError> {
    if let Some(change) = requested {
        return crate::brief::canon_brief(repo, change)
            .map(|path| (change.clone(), path))
            .ok_or_else(|| CliError::failed("BRIEF_NOT_FOUND", format!("no brief at {}/{}/brief.md", paths::CHANGES_DIR, change.as_str())).hint("check the slug against the changes directory, or create the change with `canon change new <slug> --subject-id <id> --title <title>`"));
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
        0 => Err(CliError::failed("NO_BRIEF", format!("no brief under {}/; write one with `canon change new`", paths::CHANGES_DIR)).hint("create one with `canon change new <slug> --subject-id <id> --title <title>`; it scaffolds brief.md")),
        1 => Ok(live.remove(0)),
        _ => Err(CliError::failed(
            "CHANGE_AMBIGUOUS",
            format!("{} live changes have a brief ({}); pass --change-id <slug>", live.len(), live.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>().join(", ")),
        ).hint("pass --change-id <slug> naming one of the listed changes")),
    }
}

/// Reads and judges the brief of `requested` (or the only live change).
pub fn check(repo: &Path, requested: Option<&ChangeId>) -> Result<Checked, CliError> {
    let (change, path) = resolve_change(repo, requested)?;
    let text = std::fs::read_to_string(&path).map_err(|e| CliError::failed("BRIEF_UNREADABLE", format!("cannot read `{}`: {e}", rel(repo, &path))).hint("check that brief.md is a readable UTF-8 file, then run the command again"))?;
    check_text(repo, change, &path, &text)
}

/// Judges `text`, the brief of `change` read from `path`: [`check`] for a
/// caller that has already read the file and must judge those same bytes.
pub fn check_text(repo: &Path, change: ChangeId, path: &Path, text: &str) -> Result<Checked, CliError> {
    let resolved = crate::language::for_brief(repo, text).map_err(|e| CliError::failed("CONFIG_UNREADABLE", e).hint("fix canon.yaml so it parses (`canon init --check-config` reports what is wrong), then run the command again"))?;
    let phase = phase_of(repo, &change, text);
    let flows = match phase {
        Phase::Building if !is_chore(text) => Some(flow_scenarios(repo, &change).map_err(|e| CliError::failed("SPEC_ROOTS_UNREADABLE", format!("cannot read the scenarios the flow steps point at: {e}")).hint("fix `specs.roots` in canon.yaml (`canon init --check-config` reports what is wrong), then run the command again"))?),
        _ => None,
    };
    let evaluation = evaluate(text, &resolved, flows.as_ref());
    Ok(Checked { change, brief: rel(repo, path), language: resolved.language, language_source: resolved.source, language_note: resolved.note, phase, evaluation })
}

/// One flow step of a brief: its id (`F1`), its text with the list marker
/// dropped, and its line.
#[derive(Debug, Clone, Serialize)]
pub struct FlowStep {
    pub id: String,
    pub text: String,
    pub line: usize,
}

/// One marked section of a brief as a reader shows it: found by marker
/// (never by heading text, so the `ko` headings read the same), with its
/// heading and the body lines after it. Fenced code and HTML comments are
/// not part of a body, as in every other reading of the brief.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BriefSection {
    pub key: String,
    pub role: &'static str,
    /// The heading's text, `#` marks dropped.
    pub heading: Option<String>,
    /// The marker's line.
    pub line: usize,
    /// The body lines, a blank line between paragraphs.
    pub body: Vec<String>,
    /// The steps of the `flow` section; empty for any other.
    pub steps: Vec<FlowStep>,
}

/// The sections of `text`, in brief order: what [`evaluate`] reads, so the
/// page and the checker cannot disagree about where a section starts.
pub fn parse_sections(text: &str) -> Vec<BriefSection> {
    let (lines, comments) = scan(text);
    sections(&lines, &comments)
        .iter()
        .map(|section| {
            let mut heading = None;
            let mut body: Vec<String> = Vec::new();
            for line in &section.lines {
                let trimmed = line.text.trim();
                if heading.is_none() && trimmed.starts_with('#') {
                    heading = Some(trimmed.trim_start_matches('#').trim().to_string());
                    continue;
                }
                if trimmed.is_empty() && body.last().is_none_or(|last| last.is_empty()) {
                    continue;
                }
                body.push(line.text.trim_end().to_string());
            }
            while body.last().is_some_and(|last| last.is_empty()) {
                body.pop();
            }
            let mut steps: Vec<FlowStep> = Vec::new();
            if section.key == "flow" {
                for line in section.body() {
                    if line.text.trim_start().starts_with('<') {
                        continue;
                    }
                    let text = strip_list_marker(&line.text).trim().to_string();
                    match leading_flow_id(&line.text) {
                        Some(id) => steps.push(FlowStep { id, text, line: line.no }),
                        None => {
                            if let Some(last) = steps.last_mut() {
                                last.text.push(' ');
                                last.text.push_str(&text);
                            }
                        }
                    }
                }
            }
            BriefSection { key: section.key.clone(), role: section.role.as_str(), heading, line: section.marker_line, body, steps }
        })
        .collect()
}

/// The blocker findings the ship guard refuses on, for `change`'s brief:
/// empty when it has no canon brief, or a legacy one, or when the
/// scenarios the flow rules read cannot be (the guard's own
/// spec-root check names that).
pub fn guard_blockers(repo: &Path, change: &ChangeId) -> Vec<Finding> {
    let Some(path) = crate::brief::canon_brief(repo, change) else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(&path) else { return Vec::new() };
    let language = crate::language::for_brief(repo, &text).unwrap_or_else(|_| crate::language::resolve(None, &crate::language::Tally::default()));
    let flows = flow_scenarios(repo, change).ok();
    let mut findings = evaluate(&text, &language, flows.as_ref()).findings;
    findings.retain(|f| f.severity == Severity::Blocker);
    findings
}

/// The finding counts `canon status` shows for `change`'s brief, with the
/// advisory rule ids found (the one with the most findings first, ties by
/// rule id: what the planner's polish step names). `None` for a legacy
/// brief (read as before, nothing shown) or no brief.
pub fn status_report(repo: &Path, change: &ChangeId, building: bool) -> Option<(Counts, Vec<String>)> {
    let path = crate::brief::canon_brief(repo, change)?;
    let text = std::fs::read_to_string(&path).ok()?;
    let language = crate::language::for_brief(repo, &text).ok()?;
    let flows = if building && field(&text, "Approved:").is_some() { flow_scenarios(repo, change).ok() } else { None };
    let evaluation = evaluate(&text, &language, flows.as_ref());
    if evaluation.legacy {
        return None;
    }
    let mut per_rule: BTreeMap<&str, usize> = BTreeMap::new();
    for f in evaluation.findings.iter().filter(|f| f.severity == Severity::Advisory) {
        *per_rule.entry(f.rule.as_str()).or_default() += 1;
    }
    let mut rules: Vec<(&str, usize)> = per_rule.into_iter().collect();
    rules.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    Some((evaluation.counts(), rules.into_iter().map(|(rule, _)| rule.to_string()).collect()))
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
    language_source: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_note: Option<&'a str>,
    phase: Phase,
    legacy: bool,
    approved: bool,
    counts: Counts,
    findings: &'a [Finding],
    waivers: &'a [Waiver],
}

/// `canon brief check [--change-id <slug>] [--json]`.
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
            language_source: checked.language_source.as_str(),
            language_note: checked.language_note.as_deref(),
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
            Err(e) => return CliError::failed("SERIALIZE_FAILED", e.to_string()).hint("this is a canon bug: report it together with the command line that produced it").emit("canon brief check", true),
        }
    } else {
        println!("canon brief check: {} — {} blocker(s), {} advisory(ies)", checked.brief, counts.blockers, counts.advisories);
        println!("  language: {}", checked.resolved().describe());
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

/// Bare `canon brief [--change-id <slug>] [--json]`: state and next steps.
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
        let document = serde_json::to_value(&body).unwrap_or_default();
        println!("{}", serde_json::to_string_pretty(&document).unwrap_or_default());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cited(s: &str) -> Vec<String> {
        citations(s)
    }

    #[test]
    fn a_code_citation_is_a_path_a_path_with_a_line_or_a_code_identifier_in_a_span() {
        assert_eq!(cited("The check lives in src/booking/seat_hold.rs:88 and reads it."), ["src/booking/seat_hold.rs:88"]);
        assert_eq!(cited("It calls `SeatHold::release()` once."), ["SeatHold::release()"]);
        assert_eq!(cited("The `hold_ttl_seconds` setting and the `holdTimer` field."), ["hold_ttl_seconds", "holdTimer"]);
        assert_eq!(cited("Read docs/plan.md, crates/tide, ./run.sh and `Reminder.due_now`."), ["Reminder.due_now", "docs/plan.md", "crates/tide", "./run.sh"]);
        assert_eq!(cited("canon.yaml에서 `seat_hold.rs`를 읽는다."), ["seat_hold.rs", "canon.yaml"]);
        assert_eq!(cited("src/lib.rs에서 읽는다 (web/src/a.ts:3-9)."), ["src/lib.rs", "web/src/a.ts:3-9"]);
        // UI text, flags, commands, prose with a slash, dates, routes and URLs are not citations.
        for miss in [
            "The `Save` button and the `Save draft` label.",
            "The `저장` button and the `No calendar shared` label.",
            "Pass `--project-id` to `canon brief check`.",
            "Read and/or write, yes/no, read/write access.",
            "On 2026/10/21 the 1/2 share opens.",
            "The /settings/profile route and https://example.com/a/b.html open.",
            "Version v1.2 of the `OK` and `README` flows, e.g. this.",
            "입력/수정 화면에서 \"저장\"을 누른다.",
        ] {
            assert!(cited(miss).is_empty(), "{miss}: {:?}", cited(miss));
        }
    }

    #[test]
    fn asides_outside_code_count_unless_they_are_labels() {
        let found: Vec<String> = asides("a (one two) b (F1) c (30초) d `f(x)` (see later) e （전각 설명）.").into_iter().map(|(_, s)| s).collect();
        assert_eq!(found, ["one two", "see later", "전각 설명"]);
        assert!(asides("an (unclosed aside and a closed one) (x").len() == 1);
    }

    #[test]
    fn dates_and_decision_notes_are_history_in_each_language() {
        let date = |s: &str, en: bool| find_date(s, en);
        assert_eq!(date("On 2026-10-21 it changed.", true).as_deref(), Some("2026-10-21"));
        assert_eq!(date("회의(2026.10.2)에서", false).as_deref(), Some("2026.10.2"));
        assert_eq!(date("10월 21일 회의에서", false).as_deref(), Some("10월 21일"));
        assert_eq!(date("2026년 10월 개편", false).as_deref(), Some("2026년 10월"));
        assert_eq!(date("Changed Oct 21 by hand.", true).as_deref(), Some("Oct 21"));
        assert_eq!(date("Shipped 21 October.", true).as_deref(), Some("21 October"));
        assert_eq!(date("We meet on May 5th.", true).as_deref(), Some("May 5th"));
        for miss in ["Version 1.2.3 ships.", "May 5 attempts are allowed.", "Wait 10 seconds.", "3월에 한 번", "5개월 21일 동안", "2026 is a year."] {
            assert_eq!(date(miss, true), None, "{miss}");
        }
        assert_eq!(decision_history(Shipped::En, "We decided to drop it.").as_deref(), Some("we decided"));
        assert_eq!(decision_history(Shipped::En, "The user agreed to the terms.").as_deref(), None);
        assert_eq!(decision_history(Shipped::Ko, "이 동작은 합의했다.").as_deref(), Some("합의"));
        assert_eq!(decision_history(Shipped::Ko, "정책은 결정 대상이다.").as_deref(), None);
    }

    #[test]
    fn the_closed_ai_vocabulary_matches_words_not_substrings() {
        for hit in ["a seamless flow", "it leverages the cache", "a robust design", "let us delve into it", "the north star metric", "streamlining checkout"] {
            assert!(en_ai_vocabulary(hit).is_some(), "{hit}");
        }
        for miss in ["a smooth flow", "the lever moves", "unrobust is not a word", "a pivotally placed cornerstones"] {
            assert!(en_ai_vocabulary(miss).is_none(), "{miss}");
        }
        assert!(phrase_hits(Shipped::Ko, "원활한 경험이다.").iter().any(|(rule, _)| *rule == "prose.ai-vocabulary"));
        assert!(!phrase_hits(Shipped::Ko, "서버가 응답한다.").iter().any(|(rule, _)| *rule == "prose.ai-vocabulary"));
    }

    #[test]
    fn a_korean_sentence_ending_is_plain_da_or_named() {
        let end = |s: &str, item: bool| ko_ending(s, item);
        assert_eq!(end("서버가 응답한다.", false), None);
        assert_eq!(end("서버가 응답합니다.", false).as_deref(), Some("~니다"));
        assert_eq!(end("다시 확인합시다.", false).as_deref(), Some("~시다"));
        assert_eq!(end("책을 다시 넣으라.", false).as_deref(), Some("~라"));
        assert_eq!(end("함께 살펴보자.", false).as_deref(), Some("~자"));
        assert_eq!(end("이 규칙은 로그인 화면.", false).as_deref(), Some("…화면"));
        assert_eq!(end("그렇게 해요.", false).as_deref(), None, "~요 in the colloquial list is prose.colloquial's");
        assert_eq!(end("그렇게 하면 돼요.", false).as_deref(), Some("~요"));
        assert_eq!(end("그렇게 하면 된다는 말이에요.", false).as_deref(), None);
        assert_eq!(end("저장하세요.", false), None);
        assert_eq!(end("이 값은 좋지요.", false).as_deref(), Some("~요"));
        // Exempt: question, none answer, code or quote at the end, Latin end, flow step, aside-only tail.
        for exempt in ["무엇이 필요한가?", "없음.", "값은 `ok`.", "버튼은 \"저장\".", "이 값은 API.", "보고 → 누르면 → 열립니다.", "응답한다 (확인합니다)."] {
            assert_eq!(end(exempt, false), None, "{exempt}");
        }
        // A list fragment is exempt, a polite list item is not; an unterminated tail is only judged when polite.
        assert_eq!(end("영수증 화면.", true), None);
        assert_eq!(end("영수증을 보냅니다.", true).as_deref(), Some("~니다"));
        assert_eq!(end("영수증 화면", false), None);
        assert_eq!(end("영수증이 열립니다", false).as_deref(), Some("~니다"));
    }

    #[test]
    fn the_scope_of_a_section_follows_its_role_and_key() {
        let line = Line { no: 1, text: String::new() };
        let section = |key: &str, role| Section { key: key.to_string(), role, marker_line: 1, lines: vec![&line] };
        let scope = |s: &Section| scope_of(s).map(|s| (s.structure, s.history));
        assert_eq!(scope(&section("detail", Role::Normative)), Some((true, true)));
        assert_eq!(scope(&section("intent", Role::Context)), Some((false, true)));
        assert_eq!(scope(&section("questions", Role::Question)), Some((false, false)));
        assert_eq!(scope(&section("out-of-scope", Role::Context)), None);
        assert_eq!(scope(&section("category", Role::Context)), None);
    }
}
