//! 0.31 `canon-0-31-brief-format`: a brief without HTML comments (English `##` headings, frontmatter waivers and kind,
//! empty template sections), the old-format detection of the readers and writers, and `canon migrate` rewriting the
//! briefs of a repo (preview, commit, revert, interruption), through the real binary. One test per scenario, named by its id.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn canon_as(repo: &Path, role: Option<&str>, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_canon"));
    cmd.args(args).env("CANON_ACTOR", "bf-tests").current_dir(repo);
    match role {
        Some(role) => cmd.env("CANON_ROLE", role),
        None => cmd.env_remove("CANON_ROLE"),
    };
    cmd.output().expect("spawn canon")
}

fn canon(repo: &Path, args: &[&str]) -> Output {
    canon_as(repo, None, args)
}

fn out_text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn stdout_of(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = canon(repo, args);
    assert!(out.status.success(), "{args:?}: {}", out_text(&out));
    out
}

fn json(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| panic!("not json ({e}): {}", out_text(o)))
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]).args(args).current_dir(repo).output().expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn commit_all(repo: &Path) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "state", "--allow-empty"]);
}

fn brief_file(repo: &Path, change: &str) -> PathBuf {
    repo.join(format!(".canon/changes/{change}/brief.md"))
}

fn write_brief(repo: &Path, change: &str, text: &str) {
    let path = brief_file(repo, change);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn read_brief(repo: &Path, change: &str) -> String {
    fs::read_to_string(brief_file(repo, change)).unwrap()
}

fn write_text(repo: &Path, relative: &str, text: &str) {
    let path = repo.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

// ─────────────────────────────── briefs ───────────────────────────────

const EN: [(&str, &str, &str, &str); 7] = [
    ("intent", "context", "Intent", "의도"),
    ("outcome", "normative", "Desired outcome", "원하는 결과"),
    ("flow", "normative", "User flow", "사용자 흐름"),
    ("detail", "normative", "Step detail", "단계 상세"),
    ("acceptance", "normative", "Acceptance", "인수 조건"),
    ("questions", "question", "Questions and assumptions", "질문과 가정"),
    ("out-of-scope", "context", "Out of scope", "범위 밖"),
];

const BODIES: [&str; 7] = [
    "Visitors lose their cart when they switch devices.",
    "A visitor who signs in sees the saved cart. ==**The cart follows the account.**==",
    "1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n2. F2 The visitor submits a wrong password → sees an error → can retry.",
    "### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n### F2 Failed submit\n\nFive failures lock the form for thirty seconds.",
    "- F1: valid credentials open the catalog and set a session cookie.\n- F2: a wrong password shows the error and sets no cookie. ==**F2 covers the failure path.**==",
    "None.",
    "Social sign-in.",
];

/// A complete 0.31 feature brief; `frontmatter` goes first, `extra` last.
fn new_brief(title: &str, frontmatter: &str, extra: &str) -> String {
    let mut out = format!("{frontmatter}# {title}\n\n");
    for (i, (_, _, heading, _)) in EN.iter().enumerate() {
        out.push_str(&format!("## {heading}\n\n{}{}\n\n", if i == 0 { format!("{title}\n\n") } else { String::new() }, BODIES[i]));
    }
    out.push_str(extra);
    out
}

/// The same brief as canon 0.30 wrote it: a marker before every heading, waiver comments and the approval note.
fn old_brief(title: &str, korean: bool, waivers: &str) -> String {
    let mut out = format!("# {title}\n\n");
    for (i, (key, role, heading_en, heading_ko)) in EN.iter().enumerate() {
        out.push_str(&format!(
            "<!-- canon:section {key} role={role} -->\n## {}\n\n{}{}\n\n",
            if korean { heading_ko } else { heading_en },
            if i == 0 { format!("{title}\n\n") } else { String::new() },
            BODIES[i]
        ));
    }
    out.push_str(waivers);
    out.push_str("\n<!-- The human records their answer, and only that answer, with: canon brief approve --approver <who> --note \"<their words>\" -->\n");
    out
}

fn old_chore(title: &str) -> String {
    format!("# {title}\n\n<!-- canon:kind chore -->\n\n<!-- canon:section intent role=context -->\n## Intent\n\n{title}\n\nThe client has a security fix.\n\n<!-- canon:section category role=context -->\n## Category\n\nci\n\n<!-- canon:section invariants role=normative -->\n## Invariants\n\n- Requests and responses stay byte-identical. ==**No behavior changes.**==\n\n<!-- canon:section checks role=normative -->\n## Checks\n\n- The suite runs again at the chore commit. ==**The suite is the check.**==\n\n<!-- canon:section out-of-scope role=context -->\n## Out of scope\n\nNone.\n")
}

const NEW_CHORE: &str = "---\nkind: chore\n---\n\n# K\n\n## Intent\n\nK\n\nWhy.\n\n## Category\n\nci\n\n## Invariants\n\n- Nothing changes. ==**No behavior changes.**==\n\n## Checks\n\n- The suite runs again. ==**The suite is the check.**==\n\n## Out of scope\n\nNone.\n";

/// `text` with the `Approved:` line the human's `brief approve` writes, bound to the text.
fn approved(text: &str) -> String {
    format!("{text}Approved: Ana Kim 2026-10-12T09:00:00Z — \"ship it\" (brief {})\n", canon_cli::brief::brief_digest(text))
}

fn waive_comment(rule: &str, section: &str, reason: &str) -> String {
    format!("<!-- canon:waive {rule} {section} \"{reason}\" -->\n")
}

// ─────────────────────────────── repos ───────────────────────────────

/// A git repo made by `canon init`, with subject `demo`; nothing committed yet.
fn repo(language: Option<&str>) -> TempDir {
    let dir = TempDir::new().unwrap();
    let mut init = vec!["init", "--providers", "none"];
    if let Some(language) = language {
        init.extend(["--language", language]);
    }
    ok(dir.path(), &init);
    git(dir.path(), &["init", "-q", "-b", "main"]);
    ok(dir.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    dir
}

/// A repo whose changes `a` (approved, English headings, a waiver), `b` (unapproved, Korean headings) and `c` (an approved
/// chore) hold briefs in the 0.30 format, committed.
fn old_repo() -> TempDir {
    let dir = repo(None);
    let r = dir.path();
    for change in ["a", "b", "c"] {
        ok(r, &["change", "new", change, "--subject-id", "demo", "--title", &change.to_uppercase()]);
    }
    write_brief(r, "a", &approved(&old_brief("A", false, &waive_comment("prose.length", "brief", "a long brief by design"))));
    write_brief(r, "b", &old_brief("B", true, ""));
    write_brief(r, "c", &approved(&old_chore("C")));
    commit_all(r);
    dir
}

fn plan(repo: &Path) -> (Value, String) {
    let out = ok(repo, &["migrate", "--json"]);
    let report = json(&out);
    let digest = report["data"]["planDigest"].as_str().unwrap().to_string();
    (report, digest)
}

fn commit(repo: &Path, role: Option<&str>, digest: &str) -> Output {
    canon_as(repo, role, &["migrate", "--commit", "--plan-digest", digest, "--json"])
}

/// Every file but `.git` and the backups: path to bytes.
fn tree(repo: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(repo: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(repo).unwrap().to_string_lossy().replace('\\', "/");
            if relative == ".git" || relative == ".canon/migrate-backup" {
                continue;
            }
            if path.is_dir() {
                walk(repo, &path, out);
            } else {
                out.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(repo, repo, &mut out);
    out
}

fn check(repo: &Path, change: &str) -> (i32, Value) {
    let out = canon(repo, &["brief", "check", change, "--json"]);
    let parsed = json(&out);
    (out.status.code().unwrap(), if parsed["data"].is_object() { parsed["data"].clone() } else { parsed })
}

fn rules(report: &Value) -> Vec<String> {
    report["findings"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap().to_string()).collect()
}

fn todo_in_scope(text: &str) -> String {
    text.replace("Social sign-in.", "TODO decide the provider list.")
}

// ─────────────────────────────── F1 ───────────────────────────────

/// authoring.brief-check.17
#[test]
fn authoring_brief_check_17_english_headings_without_comments_pass_and_every_reader_finds_the_same_sections() {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["change", "new", "a", "--subject-id", "demo", "--title", "A"]);
    let en = new_brief("A", "", "");
    write_brief(r, "a", &en);
    let (code, report) = check(r, "a");
    assert_eq!((code, rules(&report)), (0, Vec::<String>::new()), "{report}");
    assert!(!en.contains("<!--"));

    // Korean prose under the same English headings reads the same.
    let ko = en.replace("Visitors lose their cart when they switch devices.", "방문자는 기기를 바꾸면 장바구니를 잃는다.");
    write_brief(r, "a", &ko);
    assert_eq!(check(r, "a").0, 0);

    // brief show and status read the same sections: the state, no empty section, the finding counts.
    let show = json(&ok(r, &["brief", "show", "a", "--json"]));
    assert_eq!(show["data"]["state"], "ready", "{show}");
    assert_eq!(show["data"]["emptySections"], serde_json::json!([]), "{show}");
    let status = json(&ok(r, &["status", "--json"]));
    assert_eq!(status["data"]["subjects"][0]["briefs"][0]["findings"]["blockers"], 0, "{status}");
}

/// authoring.brief-check.18
#[test]
fn authoring_brief_check_18_a_misspelt_heading_names_the_nearest_one_and_a_deleted_or_moved_section_is_reported() {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["change", "new", "a", "--subject-id", "demo", "--title", "A"]);
    let text = new_brief("A", "", "");
    for (wrong, expected) in [("## Intnet", "`## Intnet`"), ("## intent", "`## intent`")] {
        write_brief(r, "a", &text.replace("## Intent", wrong));
        let (code, report) = check(r, "a");
        assert_eq!(code, 1, "{report}");
        let missing = report["findings"].as_array().unwrap().iter().find(|f| f["rule"] == "brief.section-missing").unwrap_or_else(|| panic!("{report}"));
        assert_eq!(missing["section"], "intent");
        let hint = missing["hint"].as_str().unwrap();
        assert!(hint.contains(expected) && hint.contains("`## Intent`"), "{hint}");
        assert!(hint.contains("line 3"), "the heading's real line: {hint}");
    }
    write_brief(r, "a", &text.replace("## Step detail\n\n### F1 Submit", "### F1 Submit"));
    assert!(rules(&check(r, "a").1).contains(&"brief.section-missing".to_string()));
    let flow_at = text.find("## User flow").unwrap();
    let acceptance_at = text.find("## Acceptance").unwrap();
    let questions_at = text.find("## Questions and assumptions").unwrap();
    let swapped = format!("{}{}{}{}", &text[..flow_at], &text[acceptance_at..questions_at], &text[flow_at..acceptance_at], &text[questions_at..]);
    write_brief(r, "a", &swapped);
    assert!(rules(&check(r, "a").1).contains(&"brief.section-order".to_string()));

    // A shipped Korean heading with no marker is the old format and gets no near-heading hint.
    write_brief(r, "a", &text.replace("## Intent", "## 의도"));
    let (_, report) = check(r, "a");
    assert_eq!(rules(&report), vec!["brief.old-format"], "{report}");
}

/// authoring.brief-check.19
#[test]
fn authoring_brief_check_19_the_language_never_changes_the_headings_and_a_note_with_one_known_heading_is_legacy() {
    let ko_repo = repo(Some("ko"));
    let en_repo = repo(Some("en"));
    let text = new_brief("A", "", "").replace("Visitors lose their cart when they switch devices.", "방문자는 기기를 바꾸면 장바구니를 잃는다.");
    let mut seen = Vec::new();
    for dir in [&ko_repo, &en_repo] {
        ok(dir.path(), &["change", "new", "a", "--subject-id", "demo", "--title", "A"]);
        write_brief(dir.path(), "a", &text);
        let (_, report) = check(dir.path(), "a");
        seen.push(rules(&report).into_iter().filter(|r| r.starts_with("brief.")).collect::<Vec<_>>());
        write_brief(dir.path(), "a", &text.replace("## Intent", "## Intnet"));
        assert!(rules(&check(dir.path(), "a").1).contains(&"brief.section-missing".to_string()), "the heading judgement does not follow language");
    }
    assert_eq!(seen[0], seen[1], "the structure verdict is the same in ko and en");

    // A heading inside a fence is none; one known heading is a pre-PRD note.
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["change", "new", "n", "--subject-id", "demo", "--title", "N"]);
    write_brief(r, "n", "# Notes\n\n## Intent\n\nwhy\n\n```\n## User flow\n```\n\n## Open items\n\n- a\n");
    let (code, report) = check(r, "n");
    assert_eq!((code, rules(&report)), (0, vec!["brief.legacy".to_string()]), "{report}");
    assert_eq!(report["legacy"], true);
}

// ─────────────────────────────── F2 ───────────────────────────────

/// authoring.brief-check.20
#[test]
fn authoring_brief_check_20_a_frontmatter_waiver_clears_one_rule_is_listed_and_binds_to_the_approval() {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["change", "new", "a", "--subject-id", "demo", "--title", "A"]);
    let waiver = "---\nwaive:\n  - rule: brief.placeholder\n    section: out-of-scope\n    reason: \"the provider list is a policy question\"\n---\n\n";
    write_brief(r, "a", &todo_in_scope(&new_brief("A", "", "")));
    assert_eq!(check(r, "a").0, 1);
    write_brief(r, "a", &todo_in_scope(&new_brief("A", waiver, "")));
    let (code, report) = check(r, "a");
    assert_eq!((code, rules(&report)), (0, Vec::<String>::new()), "{report}");
    let listed = &report["waivers"][0];
    assert_eq!(
        (listed["rule"].as_str(), listed["section"].as_str(), listed["applied"].as_bool(), listed["severity"].as_str(), listed["line"].as_u64()),
        (Some("brief.placeholder"), Some("out-of-scope"), Some(true), Some("blocker"), Some(3)),
        "{report}"
    );
    let text = stdout_of(&canon(r, &["brief", "check", "a"]));
    assert!(text.contains("waived: blocker brief.placeholder [out-of-scope]"), "{text}");

    // A `match` waiver clears only the findings that contain it.
    let matching = "---\nwaive:\n  - rule: brief.placeholder\n    section: out-of-scope\n    reason: \"x\"\n    match: \"nothing like it\"\n---\n\n";
    write_brief(r, "a", &todo_in_scope(&new_brief("A", matching, "")));
    let (code, report) = check(r, "a");
    assert_eq!((code, report["waivers"][0]["applied"].as_bool()), (1, Some(false)), "{report}");

    // The approval covers the frontmatter: a waiver added after it is a change after it.
    write_brief(r, "a", &new_brief("A", "", ""));
    ok(r, &["brief", "approve", "a", "--approver", "Ana", "--note", "ok"]);
    let approved_text = read_brief(r, "a");
    assert!(canon_cli::brief::approval_of(&approved_text).approved());
    write_brief(r, "a", &format!("{waiver}{approved_text}"));
    let shown = json(&ok(r, &["brief", "show", "a", "--json"]));
    assert_eq!(shown["data"]["state"], "changed-since-approval", "{shown}");
}

/// authoring.brief-check.21
#[test]
fn authoring_brief_check_21_invalid_waivers_and_unreadable_frontmatter_are_blockers_that_waive_nothing() {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["change", "new", "a", "--subject-id", "demo", "--title", "A"]);
    let entry = |lines: &str| format!("---\nwaive:\n  - {lines}---\n\n");
    for lines in [
        "rule: brief.placeholder\n    section: out-of-scope\n    reason: \"\"\n",
        "rule: brief.nope\n    section: out-of-scope\n    reason: \"x\"\n",
        "rule: brief.placeholder\n    reason: \"x\"\n",
        "rule: brief.placeholder\n    section: out-of-scope\n    reason: \"x\"\n    until: never\n",
    ] {
        write_brief(r, "a", &todo_in_scope(&new_brief("A", &entry(lines), "")));
        let (code, report) = check(r, "a");
        assert_eq!(code, 1, "{lines}: {report}");
        let found = rules(&report);
        assert!(found.contains(&"brief.waiver-invalid".to_string()) && found.contains(&"brief.placeholder".to_string()), "{lines}: {found:?}");
    }
    let valid = "waive:\n  - rule: brief.placeholder\n    section: out-of-scope\n    reason: \"x\"\n";
    for frontmatter in [format!("---\n{valid}  - [\n---\n\n"), format!("---\n{valid}kinds: chore\n---\n\n"), format!("---\n{valid}kind: epic\n---\n\n")] {
        write_brief(r, "a", &todo_in_scope(&new_brief("A", &frontmatter, "")));
        let (code, report) = check(r, "a");
        assert_eq!(code, 1, "{frontmatter}: {report}");
        let found = rules(&report);
        assert!(found.contains(&"brief.frontmatter-invalid".to_string()) && found.contains(&"brief.placeholder".to_string()), "no waiver of a broken frontmatter is read: {found:?}");
    }
    // An unknown section key is no blocker: the waiver is listed as matching nothing.
    let unknown = "---\nwaive:\n  - rule: prose.length\n    section: mystery\n    reason: \"x\"\n---\n\n";
    write_brief(r, "a", &new_brief("A", unknown, ""));
    let (code, report) = check(r, "a");
    assert_eq!((code, report["waivers"][0]["applied"].as_bool()), (0, Some(false)), "{report}");
    assert!(stdout_of(&canon(r, &["brief", "check", "a"])).contains("waiver matches nothing"));
}

/// authoring.brief-check.22
#[test]
fn authoring_brief_check_22_kind_chore_in_the_frontmatter_makes_a_chore_and_its_absence_a_feature() {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["change", "new", "k", "--subject-id", "demo", "--title", "K", "--change-kind", "chore"]);
    write_brief(r, "k", NEW_CHORE);
    let (code, report) = check(r, "k");
    assert_eq!((code, rules(&report)), (0, Vec::<String>::new()), "{report}");
    let stripped = NEW_CHORE.replacen("---\nkind: chore\n---\n\n", "", 1);
    write_brief(r, "k", &stripped);
    let (code, report) = check(r, "k");
    assert_eq!(code, 1, "without the frontmatter it is a feature brief: {report}");
    assert!(rules(&report).contains(&"brief.section-missing".to_string()), "{report}");
    assert!(canon_cli::brief_check::is_chore(NEW_CHORE) && !canon_cli::brief_check::is_chore(&stripped));

    // The chore readers read the frontmatter: the change review pins the chore brief.
    write_brief(r, "k", NEW_CHORE);
    commit_all(r);
    ok(r, &["policy", "approve", "--approver", "Ana"]);
    let out = canon_as(r, Some("reviewer"), &["review", "change", "k", "--original-spec-ref", ".canon/changes/k/brief.md", "--actor-id", "rev", "--role", "reviewer"]);
    assert!(out.status.success(), "{}", out_text(&out));
    write_brief(r, "k", &stripped);
    let out = canon_as(r, Some("reviewer"), &["review", "change", "k", "--original-spec-ref", ".canon/changes/k/brief.md", "--actor-id", "rev", "--role", "reviewer"]);
    assert!(out_text(&out).contains("is not a chore"), "a brief without the frontmatter is no chore: {}", out_text(&out));
}

// ─────────────────────────────── F3 ───────────────────────────────

/// authoring.change.08
#[test]
fn authoring_change_08_change_new_writes_the_title_and_empty_headed_sections_without_a_comment() {
    for (language, kind) in [(None, "feature"), (Some("ko"), "feature"), (None, "chore")] {
        let dir = repo(language);
        let r = dir.path();
        ok(r, &["change", "new", "x", "--subject-id", "demo", "--title", "Add login", "--change-kind", kind]);
        let text = read_brief(r, "x");
        assert!(!text.contains("<!--"), "{text}");
        if kind == "chore" {
            assert_eq!(text, "---\nkind: chore\n---\n\n# Add login\n\n## Intent\n\nAdd login\n\n## Category\n\n## Invariants\n\n## Checks\n\n## Out of scope\n", "{text}");
        } else {
            assert_eq!(
                text, "# Add login\n\n## Intent\n\nAdd login\n\n## Desired outcome\n\n## User flow\n\n## Step detail\n\n## Acceptance\n\n## Questions and assumptions\n\n## Out of scope\n",
                "headings stay English with language {language:?}"
            );
        }
    }
}

/// authoring.brief-check.23
#[test]
fn authoring_brief_check_23_show_prints_each_empty_sections_guidance_in_the_briefs_language() {
    for (language, snippet) in [(Some("ko"), "출시되면 사용자 입장에서 눈에 보이는 상태"), (None, "What is observably true once this ships")] {
        let dir = repo(language);
        let r = dir.path();
        ok(r, &["change", "new", "x", "--subject-id", "demo", "--title", "Add login"]);
        let (code, report) = check(r, "x");
        assert_eq!(code, 1);
        let found = report["findings"].as_array().unwrap();
        assert_eq!(found.len(), 6, "every section but Intent is empty: {report}");
        let outcome = found.iter().find(|f| f["section"] == "outcome").unwrap();
        assert!(outcome["hint"].as_str().unwrap().contains(snippet), "{outcome}");
        let shown = ok(r, &["brief", "show", "x"]);
        assert!(stdout_of(&shown).contains(snippet) && stdout_of(&shown).contains("empty: ## Desired outcome"), "{}", stdout_of(&shown));
        let shown = json(&ok(r, &["brief", "show", "x", "--json"]));
        let empty = shown["data"]["emptySections"].as_array().unwrap();
        assert_eq!(empty.len(), 6);
        assert!(empty.iter().any(|s| s["key"] == "flow" && s["heading"] == "User flow"), "{shown}");
    }
}

/// authoring.brief-check.24
#[test]
fn authoring_brief_check_24_a_filled_section_that_keeps_a_todo_is_still_a_placeholder() {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["change", "new", "x", "--subject-id", "demo", "--title", "A"]);
    write_brief(r, "x", &todo_in_scope(&new_brief("A", "", "")));
    let (code, report) = check(r, "x");
    assert_eq!(code, 1, "{report}");
    let placeholder = report["findings"].as_array().unwrap().iter().find(|f| f["rule"] == "brief.placeholder").unwrap();
    assert_eq!(placeholder["section"], "out-of-scope");
    assert!(placeholder["hint"].as_str().unwrap().contains("TODO"), "{placeholder}");
    // The guidance is never judged: a section holding only guidance text still reads as written.
    let guidance = new_brief("A", "", "").replace("Social sign-in.", "What this change deliberately does not do. Write `None.` if nothing.");
    write_brief(r, "x", &guidance);
    assert_eq!(check(r, "x").0, 0, "guidance text is no placeholder");
}

// ─────────────────────────────── F4 ───────────────────────────────

/// authoring.brief-check.25
#[test]
fn authoring_brief_check_25_an_old_format_brief_gets_one_blocker_and_its_waiver_comments_are_not_read() {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["change", "new", "x", "--subject-id", "demo", "--title", "A"]);
    let waivers = waive_comment("prose.length", "brief", "a long brief by design");
    write_brief(r, "x", &old_brief("A", false, &waivers).replace("Social sign-in.", "TODO decide."));
    let (code, report) = check(r, "x");
    assert_eq!(code, 1, "{report}");
    assert_eq!(rules(&report), vec!["brief.old-format"], "no section rule is applied: {report}");
    assert_eq!(report["oldFormat"], true);
    assert_eq!(report["waivers"], serde_json::json!([]), "the waiver comment is not read");
    assert!(report["findings"][0]["hint"].as_str().unwrap().contains("canon migrate"));
    let shown = json(&ok(r, &["brief", "show", "x", "--json"]));
    assert_eq!((shown["data"]["state"].as_str(), shown["data"]["next"][0]["step"].as_str()), (Some("old-format"), Some("migrate")), "{shown}");
}

/// context.status.87
#[test]
fn context_status_87_the_readers_warn_and_lead_with_canon_migrate() {
    let dir = old_repo();
    let r = dir.path();
    for args in [&["status", "--json"][..], &["gate", "check", "--json"], &["context", "--json"], &["brief", "show", "a", "--json"]] {
        let out = canon(r, args);
        let doc = json(&out);
        let warnings = doc["meta"]["warnings"].as_array().map(|w| w.iter().map(|x| x.as_str().unwrap().to_string()).collect::<Vec<_>>()).unwrap_or_default();
        assert!(warnings.iter().any(|w| w.contains("0.30") && w.contains("canon migrate")), "{args:?}: {warnings:?}");
        assert_eq!(doc["meta"]["next"][0]["command"], "canon migrate --json", "{args:?}: {}", doc["meta"]);
        assert!(doc["error"].is_null() || doc["error"]["code"] != "CONFIG_UNREADABLE", "{args:?}: {doc}");
    }
    // An approved old brief whose digest matches still reads approved.
    let status = json(&ok(r, &["status", "--json"]));
    let subjects = status["data"]["subjects"].as_array().unwrap();
    let briefs: Vec<&Value> = subjects.iter().flat_map(|s| s["briefs"].as_array().into_iter().flatten()).collect();
    assert!(briefs.iter().any(|b| b["change"] == "a" && b["status"] == "approved"), "{status}");
}

/// A repo whose subject `demo` is building with scenario `cart.add.01` and an adopted change `a`.
fn building_repo() -> TempDir {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["policy", "approve", "--approver", "Ana"]);
    ok(r, &["change", "new", "a", "--subject-id", "demo", "--title", "A"]);
    write_text(r, ".canon/specs/kind=feature/area=cart/add.feature", "Feature: cart add\n  # canon: {\"schema\":1,\"at\":\"2026-10-01T00:00:00Z\",\"actor\":{\"agent_id\":\"x\"}}\n\n  # canon: {\"schema\":1,\"at\":\"2026-10-01T00:00:00Z\",\"actor\":{\"agent_id\":\"x\"}}\n  @subject:demo\n  @cart.add.01\n  Scenario: Add one\n    Given an empty cart\n    When one item is added\n    Then the cart holds 1\n");
    ok(r, &["inventory", "sync"]);
    for to in ["specced", "building"] {
        ok(r, &["subject", "move", "demo", "--to", to]);
    }
    commit_all(r);
    dir
}

/// evidence.add.131
#[test]
fn evidence_add_131_is_refused_when_a_brief_it_judges_is_old_format_and_an_override_does_not_waive_it() {
    let dir = building_repo();
    let r = dir.path();
    for text in [approved(&old_brief("A", false, "")), old_brief("A", false, "")] {
        write_brief(r, "a", &text);
        let before = tree(r);
        for extra in [&[][..], &["--override-reason", "go on"]] {
            let mut args = vec![
                "evidence",
                "add",
                "--scenario-id",
                "cart.add.01",
                "--project-id",
                "root",
                "--class",
                "test-run",
                "--ref",
                "cargo test",
                "--verdict",
                "faithful",
                "--role",
                "implementer",
                "--summary",
                "ran",
                "--json",
            ];
            args.extend(extra);
            let out = canon_as(r, Some("implementer"), &args);
            assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
            let doc = json(&out);
            assert_eq!(doc["error"]["code"], "OLD_BRIEF_FORMAT", "{doc}");
            assert!(doc["error"]["message"].as_str().unwrap().contains("canon migrate"), "{doc}");
        }
        assert_eq!(tree(r), before, "nothing was written");
    }
}

/// subject.lifecycle.47
#[test]
fn subject_lifecycle_47_ship_and_reopen_refuse_an_old_brief_the_subject_answers_for_and_ignore_one_it_does_not() {
    let dir = building_repo();
    let r = dir.path();
    write_brief(r, "a", &new_brief("A", "", ""));
    ok(r, &["brief", "approve", "a", "--approver", "Ana", "--note", "ok"]);
    ok(r, &["change", "new", "draft", "--subject-id", "demo", "--title", "D"]);
    write_brief(r, "draft", &old_brief("D", false, ""));
    ok(r, &["subject", "move", "demo", "--to", "verifying", "--override-reason", "no review in this test"]);
    // `demo` answers for the approved change `a`; `draft` is never approved, untagged and no chore: not judged.
    let out = canon(r, &["subject", "move", "demo", "--to", "shipped", "--json"]);
    assert_ne!(json(&out)["error"]["code"], "OLD_BRIEF_FORMAT", "a change the subject does not answer for never blocks: {}", out_text(&out));
    // Once the old brief carries an Approved line the subject answers for it, and shipping is refused naming it.
    write_brief(r, "draft", &approved(&old_brief("D", false, "")));
    let out = canon(r, &["subject", "move", "demo", "--to", "shipped", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    let doc = json(&out);
    assert_eq!(doc["error"]["code"], "OLD_BRIEF_FORMAT", "{doc}");
    assert!(doc["error"]["message"].as_str().unwrap().contains("`draft`"), "{doc}");
    let subjects = json(&ok(r, &["query", "--kind", "subject", "--json"]));
    let demo = subjects["data"]["records"].as_array().unwrap().iter().find(|s| s["subject_id"] == "demo").unwrap();
    assert_eq!(demo["status"], "verifying", "{demo}");
}

/// review.add.103
#[test]
fn review_add_103_a_change_review_pins_a_new_chore_brief_and_refuses_an_old_format_one() {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["policy", "approve", "--approver", "Ana"]);
    ok(r, &["change", "new", "k", "--subject-id", "demo", "--title", "K", "--change-kind", "chore"]);
    commit_all(r);
    write_brief(r, "k", &old_chore("K"));
    let out = canon_as(r, Some("reviewer"), &["review", "change", "k", "--original-spec-ref", ".canon/changes/k/brief.md", "--actor-id", "rev", "--role", "reviewer", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    assert_eq!(json(&out)["error"]["code"], "OLD_BRIEF_FORMAT");
    write_brief(r, "k", NEW_CHORE);
    let out = canon_as(r, Some("reviewer"), &["review", "change", "k", "--original-spec-ref", ".canon/changes/k/brief.md", "--actor-id", "rev", "--role", "reviewer"]);
    assert!(out.status.success(), "{}", out_text(&out));
    assert!(out_text(&out).contains(&canon_cli::brief::brief_spec_digest(NEW_CHORE).as_str()[..12]), "{}", out_text(&out));
}

/// ingest.plans.13
#[test]
fn ingest_plans_13_the_intent_summary_is_the_first_paragraph_of_the_intent_section() {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["change", "new", "a", "--subject-id", "demo", "--title", "A"]);
    write_brief(r, "a", &new_brief("Add\nlogin", "", "").replace("Visitors lose", "More.\n\nVisitors lose"));
    write_brief(r, "e", "# E\n\n## Intent\n\n## Desired outcome\n\nBody.\n");
    ok(r, &["ingest", "plans"]);
    let changes = json(&ok(r, &["query", "--kind", "change", "--json"]));
    let summary = |id: &str| changes["data"]["records"].as_array().unwrap().iter().find(|c| c["change_id"] == id).map(|c| c["summary"].as_str().unwrap().to_string());
    assert_eq!(summary("a").as_deref(), Some("Add login"), "{changes}");
    assert_eq!(summary("e").as_deref(), Some(""), "{changes}");
}

/// report.dashboard.16: the page itself is exercised in tests/dashboard_briefs.rs; the reader both share is checked here.
#[test]
fn report_dashboard_16_the_shared_reader_gives_each_section_its_key_and_role_and_flags_the_old_format() {
    let sections = canon_cli::brief_check::parse_sections(&new_brief("A", "", ""));
    assert_eq!((sections[0].key.as_str(), sections[0].role, sections[0].line), ("intent", "context", 3));
    assert_eq!(sections[2].steps.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["F1", "F2"]);
    assert_eq!(sections.iter().map(|s| s.role).collect::<Vec<_>>(), ["context", "normative", "normative", "normative", "normative", "question", "context"]);
    let ko = canon_cli::brief_check::parse_sections(&new_brief("A", "", "").replace("The visitor sees the form", "방문자는 입력란을 본다"));
    assert_eq!(ko.iter().map(|s| s.key.as_str()).collect::<Vec<_>>(), sections.iter().map(|s| s.key.as_str()).collect::<Vec<_>>());
    assert!(canon_ingest::brief::parse(&new_brief("A", "", "").replace("## Intent", "## 의도")).old_format.is_some());
}

/// skills.install.74
#[test]
fn skills_install_74_the_brief_skill_documents_carry_the_table_the_guidance_and_the_frontmatter_waiver() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../canon/skills/reference");
    let shape = fs::read_to_string(root.join("brief-shape.md")).unwrap();
    let polish = fs::read_to_string(root.join("brief-polish.md")).unwrap();
    let english = canon_cli::language::Language { tag: "en".to_string() };
    for def in canon_ingest::brief::FEATURE_SECTIONS.iter().chain(canon_ingest::brief::CHORE_SECTIONS.iter()) {
        assert!(shape.contains(&format!("`## {}`", def.heading)), "{}", def.heading);
        assert!(shape.contains(canon_cli::brief_template::guidance(&english, def.key)), "the skill carries the guidance of {} as `brief show` prints it", def.key);
    }
    assert!(shape.contains("waive:") && shape.contains("canon migrate"), "the frontmatter waiver and the old-format path");
    for doc in [&shape, &polish] {
        assert!(!doc.contains("<!-- canon:section") && !doc.contains("<!-- canon:hint") && !doc.contains("<!-- canon:waive"), "no skill names a comment as the way to write a brief");
    }
}

// ─────────────────────────────── F5, F6 ───────────────────────────────

/// authoring.migrate.20
#[test]
fn authoring_migrate_20_the_preview_writes_nothing_and_says_first_whether_it_re_binds() {
    let dir = old_repo();
    let r = dir.path();
    let before = tree(r);
    let (report, digest) = plan(r);
    assert_eq!(tree(r), before, "the working tree is byte-identical after a preview");
    assert_eq!(report["data"]["committed"], false);
    let steps: Vec<&str> = report["data"]["steps"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect();
    assert!(steps[0].contains("briefs") && steps[0].contains("re-binds 2 `Approved:` line(s) and 0 review pin(s)"), "{steps:?}");
    assert!(steps[1].contains("old digest matches the old text"), "the condition is a sentence: {steps:?}");
    let all = steps.join("\n");
    assert!(all.contains(".canon/changes/a/brief.md") && all.contains("1 waiver(s) to the frontmatter"), "{all}");
    assert!(all.contains("heading(s) to English"), "the Korean-titled brief: {all}");
    assert!(all.contains("`kind: chore`"), "{all}");
    let old_digest = canon_cli::brief::brief_digest(&read_brief(r, "a"));
    assert!(all.contains(&format!("Approved: {old_digest} -> sha256:")), "old and new digest: {all}");
    assert_eq!(digest.len(), 64);
}

/// authoring.migrate.21
#[test]
fn authoring_migrate_21_a_dirty_brief_is_refused_and_edited_or_role_mismatched_briefs_are_named() {
    let dir = old_repo();
    let r = dir.path();
    let dirty = format!("{}\nnote\n", read_brief(r, "b"));
    fs::write(brief_file(r, "b"), &dirty).unwrap();
    let out = canon(r, &["migrate", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    assert!(out_text(&out).contains("MIGRATE_UNCOMMITTED") && out_text(&out).contains("changes/b/brief.md"), "{}", out_text(&out));
    assert_eq!(fs::read_to_string(brief_file(r, "b")).unwrap(), dirty);
    git(r, &["checkout", "--", "."]);

    // Edited after its approval, and a marker role that differs from the table.
    let edited = read_brief(r, "a").replace("Social sign-in.", "Social sign-in and email links.");
    write_brief(r, "a", &edited);
    let mismatch = old_brief("M", false, "").replace("<!-- canon:section outcome role=normative -->", "<!-- canon:section outcome role=context -->");
    ok(r, &["change", "new", "m", "--subject-id", "demo", "--title", "M"]);
    write_brief(r, "m", &approved(&mismatch));
    commit_all(r);
    let (report, _) = plan(r);
    let effects = report["data"]["effects"].as_array().unwrap();
    let kept: Vec<String> = effects.iter().filter(|e| e["action"] == "keep").map(|e| format!("{} {}", e["path"].as_str().unwrap_or(""), e["detail"].as_str().unwrap_or(""))).collect();
    assert!(kept.iter().any(|k| k.contains("changes/a/brief.md") && k.contains("edited after its approval")), "{kept:?}");
    assert!(kept.iter().any(|k| k.contains("changes/m/brief.md") && k.contains("role=context")), "{kept:?}");
    let rebinds: Vec<&str> = effects.iter().filter(|e| e["action"] == "rebind").map(|e| e["path"].as_str().unwrap()).collect();
    assert!(!rebinds.contains(&".canon/changes/a/brief.md") && !rebinds.contains(&".canon/changes/m/brief.md"), "{rebinds:?}");
}

/// authoring.migrate.22
#[test]
fn authoring_migrate_22_commit_rewrites_every_brief_keeps_approvals_and_a_second_run_has_nothing_to_do() {
    let dir = old_repo();
    let r = dir.path();
    let before: BTreeMap<&str, String> = ["a", "b", "c"].iter().map(|c| (*c, read_brief(r, c))).collect();
    let (_, digest) = plan(r);
    let out = commit(r, None, &digest);
    assert!(out.status.success(), "{}", out_text(&out));
    for change in ["a", "b", "c"] {
        let text = read_brief(r, change);
        assert!(!text.contains("<!--"), "{change}: {text}");
        assert!(canon_ingest::brief::parse(&text).old_format.is_none());
    }
    // Approved briefs read approved, with no BRIEF_CHANGED_SINCE_APPROVAL; the waived brief has no blocker.
    for change in ["a", "c"] {
        let shown = json(&ok(r, &["brief", "show", change, "--json"]));
        assert_ne!(shown["data"]["state"], "changed-since-approval", "{change}: {shown}");
        assert!(canon_cli::brief::approval_of(&read_brief(r, change)).approved(), "{change}");
    }
    let (code, report) = check(r, "a");
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["waivers"][0]["rule"], "prose.length", "the waiver moved with its reason: {report}");
    let a = read_brief(r, "a");
    assert!(a.starts_with("---\nwaive:\n  - rule: \"prose.length\"\n    section: \"brief\"\n    reason: \"a long brief by design\"\n---\n\n# A\n"), "{a}");
    assert!(read_brief(r, "c").starts_with("---\nkind: chore\n---\n\n# C\n"));
    let b = read_brief(r, "b");
    assert!(b.contains("## Intent") && b.contains("## Desired outcome") && !b.contains("## 의도"), "{b}");
    // The approval line keeps its approver, time and words; only the digest moved.
    let line = |t: &str| t.lines().find(|l| l.starts_with("Approved:")).unwrap().to_string();
    let (old_line, new_line) = (line(&before["a"]), line(&a));
    assert_eq!(old_line.split(" (brief ").next(), new_line.split(" (brief ").next());
    assert_ne!(old_line, new_line);
    // The prose (everything but comments, headings, frontmatter and the digest) is byte for byte the same.
    let prose = |t: &str| t.lines().filter(|l| !l.trim().is_empty() && !l.starts_with("## ") && !l.starts_with("<!--") && !l.starts_with("Approved:")).map(str::to_string).collect::<Vec<_>>();
    for change in ["a", "b", "c"] {
        let new_text = read_brief(r, change);
        let body = if new_text.starts_with("---\n") { new_text.split("\n---\n\n").nth(1).unwrap_or_default().to_string() } else { new_text.clone() };
        assert_eq!(prose(&body), prose(&before[change]), "{change}");
    }
    // A second run has nothing to do.
    let again = ok(r, &["migrate", "--json"]);
    let steps = json(&again)["data"]["steps"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect::<Vec<_>>().join("\n");
    assert!(steps.contains("nothing to migrate"), "{steps}");
}

/// authoring.migrate.23
#[test]
fn authoring_migrate_23_a_changed_plan_is_refused_and_a_brief_edited_after_approval_stays_changed() {
    let dir = old_repo();
    let r = dir.path();
    let before = tree(r);
    let out = commit(r, None, &"0".repeat(64));
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    assert!(out_text(&out).contains("PLAN_CHANGED"), "{}", out_text(&out));
    assert_eq!(tree(r), before, "nothing changed");

    let edited = read_brief(r, "a").replace("Social sign-in.", "Social sign-in and email links.");
    write_brief(r, "a", &edited);
    commit_all(r);
    let (_, digest) = plan(r);
    let out = commit(r, None, &digest);
    assert!(out.status.success(), "{}", out_text(&out));
    assert!(!read_brief(r, "a").contains("<!--"));
    let shown = json(&ok(r, &["brief", "show", "a", "--json"]));
    assert_eq!(shown["data"]["state"], "changed-since-approval", "{shown}");
}

/// authoring.migrate.24
#[test]
fn authoring_migrate_24_a_plan_that_re_binds_is_human_only_and_one_that_does_not_is_not() {
    let dir = old_repo();
    let r = dir.path();
    let (_, digest) = plan(r);
    let before = tree(r);
    for role in ["planner", "orchestrator"] {
        let out = commit(r, Some(role), &digest);
        assert_eq!(out.status.code(), Some(2), "{role}: {}", out_text(&out));
        let doc = json(&out);
        assert_eq!(doc["error"]["code"], "HUMAN_ONLY", "{doc}");
        assert!(doc["error"]["hint"].as_str().unwrap().contains(&digest), "the hint carries the exact call: {doc}");
        assert_eq!(tree(r), before, "{role}: the tree is unchanged");
    }
    // A preview is open to every role.
    assert!(canon_as(r, Some("planner"), &["migrate", "--json"]).status.success());
    // A plan that re-binds nothing (no approval, no review) runs under an orchestrator.
    let plain = repo(None);
    let p = plain.path();
    ok(p, &["change", "new", "b", "--subject-id", "demo", "--title", "B"]);
    write_brief(p, "b", &old_brief("B", true, ""));
    commit_all(p);
    let (report, digest) = plan(p);
    assert!(report["data"]["steps"][0].as_str().unwrap().contains("re-binds no `Approved:` line"), "{report}");
    let out = commit(p, Some("orchestrator"), &digest);
    assert!(out.status.success(), "{}", out_text(&out));
    assert!(!read_brief(p, "b").contains("<!--"));
}

/// authoring.migrate.25
#[test]
fn authoring_migrate_25_revert_restores_byte_for_byte_refuses_a_changed_file_and_a_stopped_run_is_finished() {
    let dir = old_repo();
    let r = dir.path();
    let before = tree(r);
    let (_, digest) = plan(r);
    assert!(commit(r, None, &digest).status.success());
    let migrated = tree(r);
    assert_ne!(migrated, before);

    // A later edit of a rewritten brief makes the revert refuse, naming it, and nothing is written.
    let edited = format!("{}\nlater\n", read_brief(r, "b"));
    write_brief(r, "b", &edited);
    let out = canon(r, &["migrate", "--revert", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    assert!(json(&out)["error"]["message"].as_str().unwrap().contains(".canon/changes/b/brief.md"), "{}", out_text(&out));
    assert_eq!(read_brief(r, "b"), edited);
    write_brief(r, "b", &String::from_utf8(migrated[".canon/changes/b/brief.md"].clone()).unwrap());

    // The revert puts back an Approved: digest, so committing it is the human's.
    let out = ok(r, &["migrate", "--revert", "--json"]);
    let revert_digest = json(&out)["data"]["planDigest"].as_str().unwrap().to_string();
    let refused = canon_as(r, Some("orchestrator"), &["migrate", "--revert", "--commit", "--plan-digest", &revert_digest, "--json"]);
    assert_eq!(refused.status.code(), Some(2), "{}", out_text(&refused));
    assert_eq!(json(&refused)["error"]["code"], "HUMAN_ONLY");
    let done = canon(r, &["migrate", "--revert", "--commit", "--plan-digest", &revert_digest, "--json"]);
    assert!(done.status.success(), "{}", out_text(&done));
    assert_eq!(tree(r), before, "the old briefs and digests are back byte for byte");

    // A stopped run: a brief back in the old format and its journal incomplete; the next run finishes it.
    commit_all(r);
    let (_, digest) = plan(r);
    assert!(commit(r, None, &digest).status.success());
    let backup = fs::read_dir(r.join(".canon/migrate-backup")).unwrap().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.join("briefs-journal.json").is_file()).max().unwrap();
    let original = fs::read(backup.join(".snapshot/.canon/changes/b/brief.md")).unwrap();
    fs::write(brief_file(r, "b"), &original).unwrap();
    let journal_path = backup.join("briefs-journal.json");
    let mut journal: Value = serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
    journal["complete"] = Value::Bool(false);
    fs::write(&journal_path, serde_json::to_vec(&journal).unwrap()).unwrap();
    commit_all(r);
    let (_, digest) = plan(r);
    let out = commit(r, None, &digest);
    assert!(out.status.success(), "{}", out_text(&out));
    assert!(!read_brief(r, "b").contains("<!--"), "the next run finished the stopped one");
    let journal: Value = serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
    assert_eq!(journal["complete"], true, "the same journal was continued");
    let revert_digest = json(&ok(r, &["migrate", "--revert", "--json"]))["data"]["planDigest"].as_str().unwrap().to_string();
    let done = canon(r, &["migrate", "--revert", "--commit", "--plan-digest", &revert_digest, "--json"]);
    assert!(done.status.success(), "{}", out_text(&done));
    assert_eq!(tree(r)[".canon/changes/b/brief.md"], before[".canon/changes/b/brief.md"], "the revert after a finished stopped run restores the original");
}
