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
        ok(r, &["change", "new", change, "--subject", "demo", "--title", &change.to_uppercase()]);
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

/// Puts every file of `snapshot` (a [`tree`]) back and removes every other file, `.git` and the backups untouched.
fn restore(repo: &Path, snapshot: &BTreeMap<String, Vec<u8>>) {
    for path in tree(repo).keys().filter(|path| !snapshot.contains_key(*path)) {
        fs::remove_file(repo.join(path)).unwrap();
    }
    for (path, bytes) in snapshot {
        let path = repo.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
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
    ok(r, &["change", "new", "a", "--subject", "demo", "--title", "A"]);
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

    // The plan import reads the same Intent section, and the dashboard page (tests/dashboard_briefs.rs) the same keys and roles.
    ok(r, &["ingest", "plans"]);
    let changes = json(&ok(r, &["query", "--kind", "change", "--json"]));
    let summary = changes["data"]["records"].as_array().unwrap().iter().find(|c| c["change_id"] == "a").map(|c| c["summary"].clone());
    assert_eq!(summary, Some(Value::from("A")), "{changes}");
    assert_eq!(canon_cli::brief_check::parse_sections(&ko).iter().map(|s| s.key.clone()).collect::<Vec<_>>(), ["intent", "outcome", "flow", "detail", "acceptance", "questions", "out-of-scope"]);

    // Every reader finds the same seven sections with the key and role the heading names, in English and Korean prose, and
    // the flow steps F1 and F2 are the ones the same brief in 0.30's format has.
    let keys_roles = |text: &str| canon_cli::brief_check::parse_sections(text).iter().map(|s| (s.key.clone(), s.role)).collect::<Vec<_>>();
    let expected: Vec<(String, &str)> = EN.iter().map(|(key, role, _, _)| (key.to_string(), *role)).collect();
    assert_eq!((keys_roles(&en), keys_roles(&ko)), (expected.clone(), expected.clone()));
    let steps = |text: &str| canon_cli::brief_check::parse_sections(text).iter().flat_map(|s| s.steps.iter().map(|st| st.id.clone())).collect::<Vec<_>>();
    assert_eq!(steps(&ko), ["F1", "F2"].map(String::from));
    assert_eq!((keys_roles(&old_brief("A", false, "")), steps(&old_brief("A", false, ""))), (expected, steps(&ko)), "the 0.30 brief reads the same");
    let old = canon_ingest::brief::parse(&old_brief("A", false, ""));
    let new = canon_ingest::brief::parse(&ko);
    assert_eq!(new.sections.iter().map(|s| (s.key, s.role)).collect::<Vec<_>>(), old.sections.iter().map(|s| (s.key, s.role)).collect::<Vec<_>>(), "the shared reader finds the same sections in both formats");
    // The flow steps are the ones `scenario new --flow` checks against: F1 and F2 resolve, F3 does not.
    for (flow, id, code) in [("F1", "cart.add.01", Some(0)), ("F2", "cart.add.02", Some(0)), ("F3", "cart.add.03", Some(2))] {
        let out = canon_as(r, Some("implementer"), &["scenario", "new", id, "--title", "t", "--subject", "demo", "--change", "a", "--flow", flow, "--case", "happy", "--json"]);
        assert_eq!(out.status.code(), code, "{flow}: {}", out_text(&out));
    }

    // A chore's review and ship read the new brief the same way: the 0.31 chore has the sections and roles of the 0.30
    // chore, `review change` pins it, and the ship check counts it as an approved chore.
    let sections = |text: &str| canon_ingest::brief::parse(text).sections.iter().map(|s| (s.key, s.role)).collect::<Vec<_>>();
    assert_eq!(sections(NEW_CHORE), sections(&old_chore("K")), "the chore sections read the same in both formats");
    ok(r, &["change", "new", "k", "--subject", "demo", "--title", "K", "--change-kind", "chore"]);
    write_brief(r, "k", NEW_CHORE);
    commit_all(r);
    ok(r, &["policy", "approve", "--approver", "Ana"]);
    let reviewed = canon_as(r, Some("reviewer"), &["review", "change", "k", "--original-spec-ref", ".canon/changes/k/brief.md", "--actor", "rev", "--role", "reviewer"]);
    assert!(reviewed.status.success(), "{}", out_text(&reviewed));
    assert!(out_text(&reviewed).contains(&canon_cli::brief::brief_spec_digest(NEW_CHORE).as_str()[..12]), "the review pins the new chore: {}", out_text(&reviewed));
    let (error, names_chore) = ship_check_after(NEW_CHORE);
    assert!(names_chore, "the ship check reads the new chore: {error}");
}

/// authoring.brief-check.18
#[test]
fn authoring_brief_check_18_a_misspelt_heading_names_the_nearest_one_and_a_deleted_or_moved_section_is_reported() {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["change", "new", "a", "--subject", "demo", "--title", "A"]);
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
        ok(dir.path(), &["change", "new", "a", "--subject", "demo", "--title", "A"]);
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
    ok(r, &["change", "new", "n", "--subject", "demo", "--title", "N"]);
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
    ok(r, &["change", "new", "a", "--subject", "demo", "--title", "A"]);
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
    ok(r, &["change", "new", "a", "--subject", "demo", "--title", "A"]);
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
    // A frontmatter that never closes and one behind a byte-order mark are blockers, never silently prose.
    for frontmatter in ["---\nkind: chore\n\n", "\u{feff}---\nkind: chore\n---\n\n"] {
        write_brief(r, "a", &new_brief("A", frontmatter, ""));
        let (code, report) = check(r, "a");
        assert_eq!(code, 1, "{frontmatter:?}: {report}");
        assert!(rules(&report).contains(&"brief.frontmatter-invalid".to_string()), "{frontmatter:?}: {report}");
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
    ok(r, &["change", "new", "k", "--subject", "demo", "--title", "K", "--change-kind", "chore"]);
    write_brief(r, "k", NEW_CHORE);
    let (code, report) = check(r, "k");
    assert_eq!((code, rules(&report)), (0, Vec::<String>::new()), "{report}");
    let stripped = NEW_CHORE.replacen("---\nkind: chore\n---\n\n", "", 1);
    write_brief(r, "k", &stripped);
    let (code, report) = check(r, "k");
    assert_eq!(code, 1, "without the frontmatter it is a feature brief: {report}");
    assert!(rules(&report).contains(&"brief.section-missing".to_string()), "{report}");
    let missing_sections = |report: &Value| -> Vec<String> { report["findings"].as_array().unwrap().iter().filter(|f| f["rule"] == "brief.section-missing").map(|f| f["section"].as_str().unwrap().to_string()).collect() };
    let feature_missing = missing_sections(&report);
    assert!(feature_missing.contains(&"outcome".to_string()) && !feature_missing.contains(&"category".to_string()), "judged as a feature brief: {report}");
    // The chore rule set is intent, category, invariants, checks and out-of-scope: a missing category is named, and the
    // feature sections (outcome, flow, ...) are not asked for.
    write_brief(r, "k", &NEW_CHORE.replace("## Category\n\nci\n\n", ""));
    let (code, report) = check(r, "k");
    assert_eq!((code, missing_sections(&report)), (1, vec!["category".to_string()]), "{report}");
    assert!(canon_cli::brief_check::is_chore(NEW_CHORE) && !canon_cli::brief_check::is_chore(&stripped));

    // The chore readers read the frontmatter: the change review pins the chore brief.
    write_brief(r, "k", NEW_CHORE);
    commit_all(r);
    ok(r, &["policy", "approve", "--approver", "Ana"]);
    let out = canon_as(r, Some("reviewer"), &["review", "change", "k", "--original-spec-ref", ".canon/changes/k/brief.md", "--actor", "rev", "--role", "reviewer"]);
    assert!(out.status.success(), "{}", out_text(&out));
    write_brief(r, "k", &stripped);
    let out = canon_as(r, Some("reviewer"), &["review", "change", "k", "--original-spec-ref", ".canon/changes/k/brief.md", "--actor", "rev", "--role", "reviewer"]);
    assert!(out_text(&out).contains("is not a chore"), "a brief without the frontmatter is no chore: {}", out_text(&out));

    // The ship check reads it too: an approved chore with `kind: chore` asks for evidence recorded after its approval and
    // refuses the ship naming it; the same brief without the frontmatter is no chore, and the ship check never names it.
    let (error, names_chore) = ship_check_after(NEW_CHORE);
    assert!(names_chore, "the ship check reads the chore from the frontmatter: {error}");
    assert!(error.to_string().contains("cart.add.01"), "{error}");
    let (error, names_chore) = ship_check_after(&stripped);
    assert_eq!(error["code"], "SUBJECT_GATE_VIOLATION", "the ship check ran: {error}");
    assert!(!names_chore, "a brief without the frontmatter is no chore to the ship check: {error}");
}

// ─────────────────────────────── F3 ───────────────────────────────

/// authoring.change.08
#[test]
fn authoring_change_08_change_new_writes_the_title_and_empty_headed_sections_without_a_comment() {
    for (language, kind) in [(None, "feature"), (Some("ko"), "feature"), (None, "chore")] {
        let dir = repo(language);
        let r = dir.path();
        ok(r, &["change", "new", "x", "--subject", "demo", "--title", "Add login", "--change-kind", kind]);
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
        ok(r, &["change", "new", "x", "--subject", "demo", "--title", "Add login"]);
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
    ok(r, &["change", "new", "x", "--subject", "demo", "--title", "A"]);
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
    ok(r, &["change", "new", "x", "--subject", "demo", "--title", "A"]);
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
        let warnings: Vec<Value> = doc["meta"]["warnings"].as_array().cloned().unwrap_or_default();
        assert!(warnings.iter().any(|w| w["message"].as_str().unwrap().contains("0.30") && w["message"].as_str().unwrap().contains("canon migrate") && w["code"].is_string()), "{args:?}: {warnings:?}");
        assert_eq!(doc["meta"]["next"][0]["command"], "canon migrate --json", "{args:?}: {}", doc["meta"]);
        assert_eq!(doc["meta"]["next"][0]["argv"], serde_json::json!(["migrate", "--json"]), "{args:?}: {}", doc["meta"]);
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
    ok(r, &["change", "new", "a", "--subject", "demo", "--title", "A"]);
    write_text(r, ".canon/specs/kind=feature/area=cart/add.feature", "Feature: cart add\n  # canon: {\"schema\":1,\"at\":\"2026-10-01T00:00:00Z\",\"actor\":{\"agent_id\":\"x\"}}\n\n  # canon: {\"schema\":1,\"at\":\"2026-10-01T00:00:00Z\",\"actor\":{\"agent_id\":\"x\"}}\n  @subject:demo\n  @cart.add.01\n  Scenario: Add one\n    Given an empty cart\n    When one item is added\n    Then the cart holds 1\n");
    ok(r, &["inventory", "sync"]);
    for to in ["specced", "building"] {
        ok(r, &["subject", "move", "demo", "--to", to]);
    }
    commit_all(r);
    dir
}

/// What the ship check says about the change `k` whose brief is `brief`, approved now: subject `demo` is verifying with
/// evidence for `cart.add.01` recorded before the approval and code changed since, so an approved chore leaves that
/// evidence older than the chore. Returns the refusal's `error` and whether it names the chore `k`.
fn ship_check_after(brief: &str) -> (Value, bool) {
    let dir = building_repo();
    let r = dir.path();
    write_brief(r, "a", &new_brief("A", "", ""));
    ok(r, &["brief", "approve", "a", "--approver", "Ana", "--note", "ok"]);
    ok(r, &["evidence", "add", "--project", "root", "--scenario", "cart.add.01", "--class", "test-run", "--ref", "run", "--role", "implementer", "--actor", "impl", "--promote", "--override-reason", "no review in this test"]);
    ok(r, &["subject", "move", "demo", "--to", "verifying", "--override-reason", "no review in this test"]);
    ok(r, &["change", "new", "k", "--subject", "demo", "--title", "K", "--change-kind", "chore"]);
    // The approval's time has whole seconds: one second later it is after the evidence.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    write_brief(r, "k", &format!("{brief}Approved: Ana Kim {now} — \"ship it\" (brief {})\n", canon_cli::brief::brief_digest(brief)));
    write_text(r, "src/lib.rs", "pub fn changed() {}\n");
    let out = canon(r, &["subject", "move", "demo", "--to", "shipped", "--json"]);
    assert_ne!(out.status.code(), Some(0), "{}", out_text(&out));
    let error = json(&out)["error"].clone();
    let names_chore = error.to_string().contains("chore change `k` approved");
    (error, names_chore)
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
                "--scenario",
                "cart.add.01",
                "--project",
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

/// evidence.add.131 (every governing change): a scenario tagged for one change is refused while another governing change
/// of the subject, one it does not answer for, is in the old format.
#[test]
fn evidence_add_131_every_governing_change_of_the_subject_is_checked() {
    let dir = building_repo();
    let r = dir.path();
    write_brief(r, "a", &new_brief("A", "", ""));
    ok(r, &["brief", "approve", "a", "--approver", "Ana", "--note", "ok"]);
    ok(r, &["change", "new", "b", "--subject", "demo", "--title", "B"]);
    write_brief(r, "b", &approved(&old_brief("B", false, "")));
    let feature = ".canon/specs/kind=feature/area=cart/add.feature";
    let text = fs::read_to_string(r.join(feature)).unwrap();
    write_text(r, feature, &text.replace("  @cart.add.01\n", "  @flow:a.F1\n  @cart.add.01\n"));
    ok(r, &["inventory", "sync"]);
    commit_all(r);
    let before = tree(r);
    let out = canon_as(r, Some("implementer"), &["evidence", "add", "--scenario", "cart.add.01", "--project", "root", "--class", "test-run", "--ref", "cargo test", "--verdict", "faithful", "--role", "implementer", "--summary", "ran", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    let doc = json(&out);
    assert_eq!(doc["error"]["code"], "OLD_BRIEF_FORMAT", "{doc}");
    assert!(doc["error"]["message"].as_str().unwrap().contains("`b`"), "the old brief the evidence does not answer to is named: {doc}");
    assert_eq!(tree(r), before);
}

/// scenario new --flow reads a brief's flow steps: an old-format brief is refused with OLD_BRIEF_FORMAT.
#[test]
fn authoring_brief_check_25_scenario_new_flow_refuses_an_old_format_brief_with_its_code() {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["change", "new", "x", "--subject", "demo", "--title", "A"]);
    write_brief(r, "x", &old_brief("A", false, ""));
    let out = canon_as(r, Some("implementer"), &["scenario", "new", "cart.add.07", "--title", "t", "--subject", "demo", "--change", "x", "--flow", "F1", "--case", "happy", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    assert_eq!(json(&out)["error"]["code"], "OLD_BRIEF_FORMAT", "{}", out_text(&out));
}

/// subject.lifecycle.47
#[test]
fn subject_lifecycle_47_ship_and_reopen_refuse_an_old_brief_the_subject_answers_for_and_ignore_one_it_does_not() {
    let dir = building_repo();
    let r = dir.path();
    let status_of = |subject: &str| {
        let subjects = json(&ok(r, &["query", "--kind", "subject", "--json"]));
        subjects["data"]["records"].as_array().unwrap().iter().find(|s| s["subject_id"] == subject).unwrap()["status"].as_str().unwrap().to_string()
    };
    write_brief(r, "a", &new_brief("A", "", ""));
    ok(r, &["brief", "approve", "a", "--approver", "Ana", "--note", "ok"]);
    ok(r, &["change", "new", "draft", "--subject", "demo", "--title", "D"]);
    write_brief(r, "draft", &old_brief("D", false, ""));
    ok(r, &["subject", "move", "demo", "--to", "verifying", "--override-reason", "no review in this test"]);
    // `demo` answers for the approved change `a`; `draft` is never approved, untagged and no chore: not judged, so the ship
    // is refused by the gate for its own reasons (uncovered flows), never for the old format.
    let out = canon(r, &["subject", "move", "demo", "--to", "shipped", "--json"]);
    assert_ne!(json(&out)["error"]["code"], "OLD_BRIEF_FORMAT", "a change the subject does not answer for never blocks: {}", out_text(&out));
    assert!(out_text(&out).contains("SUBJECT_GATE_VIOLATION"), "{}", out_text(&out));
    // Once the old brief carries an Approved line the subject answers for it: the ship is refused naming it, nothing moves.
    write_brief(r, "draft", &approved(&old_brief("D", false, "")));
    let before = tree(r);
    let out = canon(r, &["subject", "move", "demo", "--to", "shipped", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    let doc = json(&out);
    assert_eq!(doc["error"]["code"], "OLD_BRIEF_FORMAT", "{doc}");
    assert!(doc["error"]["message"].as_str().unwrap().contains("`draft`"), "{doc}");
    let after = tree(r);
    let changed: Vec<&String> = after.keys().chain(before.keys()).filter(|k| after.get(*k) != before.get(*k)).collect();
    assert_eq!((status_of("demo"), changed), ("verifying".to_string(), Vec::new()));

    // A subject whose only old brief is one it does not answer for moves to shipped: it has evidenced scenarios, an approved
    // change, and the draft is never judged.
    ok(r, &["subject", "new", "plain", "--domain", "dev", "--title", "Plain"]);
    ok(r, &["change", "new", "pa", "--subject", "plain", "--title", "PA"]);
    write_brief(r, "pa", &new_brief("PA", "", ""));
    ok(r, &["brief", "approve", "pa", "--approver", "Ana", "--note", "ok"]);
    ok(r, &["change", "new", "pl", "--subject", "plain", "--title", "P"]);
    write_brief(r, "pl", &old_brief("P", false, ""));
    for (id, flow, case) in [("plain.run.01", "F1", "happy"), ("plain.run.02", "F1", "failure"), ("plain.run.03", "F2", "happy"), ("plain.run.04", "F2", "failure")] {
        ok(r, &["scenario", "new", id, "--title", id, "--subject", "plain", "--change", "pa", "--flow", flow, "--case", case]);
    }
    ok(r, &["inventory", "sync"]);
    for to in ["specced", "building"] {
        ok(r, &["subject", "move", "plain", "--to", to, "--override-reason", "no review in this test"]);
    }
    for id in ["plain.run.01", "plain.run.02", "plain.run.03", "plain.run.04"] {
        ok(r, &["evidence", "add", "--project", "root", "--scenario", id, "--class", "test-run", "--ref", "run", "--role", "implementer", "--actor", "impl", "--promote", "--override-reason", "no review in this test"]);
    }
    for to in ["verifying", "shipped"] {
        ok(r, &["subject", "move", "plain", "--to", to, "--override-reason", "no review in this test"]);
    }
    assert_eq!(status_of("plain"), "shipped");
    // A shipped subject that answers for an old-format brief is not reopened: the same guard, nothing moves.
    write_brief(r, "pl", &approved(&old_brief("P", false, "")));
    let before = tree(r);
    let out = canon(r, &["subject", "move", "plain", "--to", "verifying", "--reason", "the spec changed", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    let doc = json(&out);
    assert_eq!(doc["error"]["code"], "OLD_BRIEF_FORMAT", "{doc}");
    assert!(doc["error"]["message"].as_str().unwrap().contains("`pl`"), "{doc}");
    let unchanged = tree(r) == before;
    assert_eq!((status_of("plain"), unchanged), ("shipped".to_string(), true), "a refused reopen leaves the subject shipped and writes nothing");

    // The same guard stops a reopen (shipped to verifying, one `if ships || reopens`); a second subject is refused at the ship too.
    ok(r, &["subject", "new", "other", "--domain", "dev", "--title", "Other"]);
    ok(r, &["change", "new", "old2", "--subject", "other", "--title", "O"]);
    write_brief(r, "old2", &approved(&old_brief("O", false, "")));
    for to in ["specced", "building", "verifying"] {
        ok(r, &["subject", "move", "other", "--to", to, "--override-reason", "no review in this test"]);
    }
    let out = canon(r, &["subject", "move", "other", "--to", "shipped", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    assert_eq!(json(&out)["error"]["code"], "OLD_BRIEF_FORMAT");
    assert_eq!(status_of("other"), "verifying");
}

/// review.add.103
#[test]
fn review_add_103_a_change_review_pins_a_new_chore_brief_and_refuses_an_old_format_one() {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["policy", "approve", "--approver", "Ana"]);
    ok(r, &["change", "new", "k", "--subject", "demo", "--title", "K", "--change-kind", "chore"]);
    commit_all(r);
    write_brief(r, "k", &old_chore("K"));
    let out = canon_as(r, Some("reviewer"), &["review", "change", "k", "--original-spec-ref", ".canon/changes/k/brief.md", "--actor", "rev", "--role", "reviewer", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    assert_eq!(json(&out)["error"]["code"], "OLD_BRIEF_FORMAT");
    write_brief(r, "k", NEW_CHORE);
    let out = canon_as(r, Some("reviewer"), &["review", "change", "k", "--original-spec-ref", ".canon/changes/k/brief.md", "--actor", "rev", "--role", "reviewer"]);
    assert!(out.status.success(), "{}", out_text(&out));
    assert!(out_text(&out).contains(&canon_cli::brief::brief_spec_digest(NEW_CHORE).as_str()[..12]), "{}", out_text(&out));
}

/// ingest.plans.13
#[test]
fn ingest_plans_13_the_intent_summary_is_the_first_paragraph_of_the_intent_section() {
    let dir = repo(None);
    let r = dir.path();
    ok(r, &["change", "new", "a", "--subject", "demo", "--title", "A"]);
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
    ok(r, &["change", "new", "m", "--subject", "demo", "--title", "M"]);
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

/// The review records of `change`'s change-level reviews in the ledger and the archive: path and body.
fn change_reviews(r: &Path, change: &str) -> Vec<(String, Value)> {
    let mut found = Vec::new();
    for (path, bytes) in tree(r) {
        if path.contains("kind=review") && path.ends_with(".json") {
            let body: Value = serde_json::from_slice(&bytes).unwrap();
            if body["change_id"] == change {
                found.push((path, body));
            }
        }
    }
    found
}

/// Gives `r` a chore change `d` with an approved 0.30 brief and a change review pinned to it, as 0.30 left them,
/// committed. Returns the review's body.
fn old_chore_with_review(r: &Path) -> Value {
    ok(r, &["change", "new", "d", "--subject", "demo", "--title", "D"]);
    let old_d = approved(&old_chore("D").replace("canon:kind chore", "canon:kind chore (ci)"));
    // Made on the brief in the new format (the old one is refused), then pinned to the old text.
    write_brief(r, "d", &NEW_CHORE.replace("# K", "# D"));
    ok(r, &["policy", "approve", "--approver", "Ana"]);
    commit_all(r);
    let reviewed = canon_as(r, Some("reviewer"), &["review", "change", "d", "--original-spec-ref", ".canon/changes/d/brief.md", "--actor", "rev", "--role", "reviewer"]);
    assert!(reviewed.status.success(), "{}", out_text(&reviewed));
    let found = change_reviews(r, "d");
    assert_eq!(found.len(), 1, "{found:?}");
    let (path, mut body) = found[0].clone();
    body["pins"]["scenario_digest"] = Value::from(canon_cli::brief::brief_spec_digest(&old_d).as_str());
    fs::remove_file(r.join(&path)).unwrap();
    let ledger = Path::new(&path).ancestors().find(|a| a.ends_with("ledger")).unwrap().to_path_buf();
    let pinned = ledger.join(canon_store::partition::expected_relative_path(canon_model::RecordKind::Review, &body).unwrap());
    fs::write(r.join(&pinned), serde_json::to_vec_pretty(&body).unwrap()).unwrap();
    write_brief(r, "d", &old_d);
    commit_all(r);
    body
}

/// authoring.migrate.22
#[test]
fn authoring_migrate_22_commit_rewrites_every_brief_keeps_approvals_and_a_second_run_has_nothing_to_do() {
    let dir = old_repo();
    let r = dir.path();
    // A comment line between two lines of a paragraph, and a chore marker with its category, as 0.30 read them.
    ok(r, &["change", "new", "p", "--subject", "demo", "--title", "P"]);
    let split = "Visitors lose their cart.\n<!-- canon:hint what -->\nThey switch devices.";
    write_brief(r, "p", &approved(&old_brief("P", false, "").replace(BODIES[0], split)));
    let old_review = old_chore_with_review(r);
    let before: BTreeMap<&str, String> = ["a", "b", "c", "d", "p"].iter().map(|c| (*c, read_brief(r, c))).collect();
    let (_, digest) = plan(r);
    let out = commit(r, None, &digest);
    assert!(out.status.success(), "{}", out_text(&out));
    for change in ["a", "b", "c", "d", "p"] {
        let text = read_brief(r, change);
        assert!(!text.contains("<!--"), "{change}: {text}");
        assert!(canon_ingest::brief::parse(&text).old_format.is_none());
    }
    // The findings after are those of the same brief written in the 0.31 format by hand (the paragraph stays two paragraphs,
    // the chore marker with its category is a chore).
    let waiver = "---\nwaive:\n  - rule: prose.length\n    section: brief\n    reason: \"a long brief by design\"\n---\n\n";
    let expected_p = new_brief("P", "", "").replace(BODIES[0], "Visitors lose their cart.\n\nThey switch devices.");
    let same = |a: &str, b: &Value| -> bool { rules(&check(r, a).1) == rules(b) };
    let probe = repo(None);
    let pr = probe.path();
    ok(pr, &["change", "new", "e", "--subject", "demo", "--title", "E"]);
    for (change, text) in [("a", new_brief("A", waiver, "")), ("b", new_brief("B", "", "")), ("p", expected_p), ("d", NEW_CHORE.replace("# K", "# D"))] {
        write_brief(pr, "e", &text);
        let (_, want) = check(pr, "e");
        assert!(same(change, &want), "{change}: migrated {:?}, by hand {:?}", rules(&check(r, change).1), rules(&want));
    }
    assert!(read_brief(r, "d").starts_with("---\nkind: chore\n---\n\n# D\n"), "the marker with its category is a chore");
    // The chore review is a new version pinned to the new digest with every other field as before; the old version is archived.
    let versions = change_reviews(r, "d");
    assert_eq!(versions.len(), 2, "the old version moved to the archive and a new one is in the ledger: {versions:?}");
    let new_pin = canon_cli::brief::brief_spec_digest(&read_brief(r, "d"));
    let (new_versions, old_versions): (Vec<_>, Vec<_>) = versions.iter().partition(|(_, v)| v["pins"]["scenario_digest"] == new_pin.as_str());
    assert_eq!((new_versions.len(), old_versions.len()), (1, 1), "{versions:?}");
    assert_eq!(old_versions[0].1, old_review, "the old version is untouched");
    let mut expected = old_review.clone();
    expected["pins"]["scenario_digest"] = Value::from(new_pin.as_str());
    assert_eq!(new_versions[0].1, expected, "every field but the pin is as before");
    assert!(!new_versions[0].0.contains("archive") && old_versions[0].0.contains("archive"), "{versions:?}");
    // Approved briefs read approved, with no BRIEF_CHANGED_SINCE_APPROVAL; the waived brief has no blocker.
    for change in ["a", "c", "d", "p"] {
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
    for change in ["a", "b", "c", "d"] {
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
    assert!(out_text(&out).contains("PLAN_STALE"), "{}", out_text(&out));
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
    ok(p, &["change", "new", "b", "--subject", "demo", "--title", "B"]);
    write_brief(p, "b", &old_brief("B", true, ""));
    commit_all(p);
    let (report, digest) = plan(p);
    assert!(report["data"]["steps"][0].as_str().unwrap().contains("re-binds no `Approved:` line"), "{report}");
    let out = commit(p, Some("orchestrator"), &digest);
    assert!(out.status.success(), "{}", out_text(&out));
    assert!(!read_brief(p, "b").contains("<!--"));
}

/// subject.lifecycle.57: a migrate commit that re-binds `Approved:` lines is
/// recorded by an agent role only with the relay option, the human's name and
/// answer and an actor, and the backup journal then holds the relay.
#[test]
fn subject_lifecycle_57_a_relayed_migrate_commit_records_the_relay_in_the_backup_journal() {
    let dir = old_repo();
    let r = dir.path();
    let (_, digest) = plan(r);
    let before = tree(r);
    let relayed = |extra: &[&str]| {
        let mut args = vec!["migrate", "--commit", "--plan-digest", digest.as_str(), "--json", "--relayed-via", "AskUserQuestion"];
        args.extend(extra);
        canon_as(r, Some("planner"), &args)
    };
    let plain = commit(r, Some("planner"), &digest);
    assert_eq!((plain.status.code(), json(&plain)["error"]["code"].as_str()), (Some(2), Some("HUMAN_ONLY")), "{}", out_text(&plain));
    assert!(json(&plain)["error"]["hint"].as_str().unwrap().contains("--relayed-via"), "{}", out_text(&plain));
    // (This file's runner sets CANON_ACTOR, so the actor is never the missing part here.)
    for extra in [&["--actor", "plan-1", "--note", "ok"][..], &["--actor", "plan-1", "--approver", "Ana"]] {
        let out = relayed(extra);
        assert_eq!((out.status.code(), json(&out)["error"]["code"].as_str()), (Some(2), Some("USAGE")), "{extra:?}: {}", out_text(&out));
    }
    assert_eq!(tree(r), before, "the refused calls rewrote nothing");

    let out = relayed(&["--actor", "plan-1", "--approver", "Ana", "--note", "Yes, migrate them."]);
    assert!(out.status.success(), "{}", out_text(&out));
    assert!(!read_brief(r, "a").contains("<!--"), "the briefs were rewritten");
    let backups: Vec<_> = std::fs::read_dir(r.join(".canon/migrate-backup")).unwrap().filter_map(Result::ok).collect();
    let journal: Value = backups.iter().find_map(|d| std::fs::read(d.path().join("briefs-journal.json")).ok()).map(|b| serde_json::from_slice(&b).unwrap()).expect("a briefs journal");
    assert_eq!(journal["relay"], serde_json::json!({ "role": "planner", "actor": "plan-1", "tool": "AskUserQuestion", "approver": "Ana", "answer": "Yes, migrate them." }), "{journal}");
    assert_eq!(journal["complete"], true);

    // The human's own commit leaves no relay in its journal.
    let own = old_repo();
    let (_, own_digest) = plan(own.path());
    assert!(commit(own.path(), None, &own_digest).status.success());
    let own_journal: Value = std::fs::read_dir(own.path().join(".canon/migrate-backup")).unwrap().filter_map(Result::ok).find_map(|d| std::fs::read(d.path().join("briefs-journal.json")).ok()).map(|b| serde_json::from_slice(&b).unwrap()).unwrap();
    assert!(own_journal.get("relay").is_none(), "{own_journal}");
}

/// authoring.migrate.25
#[test]
fn authoring_migrate_25_revert_restores_byte_for_byte_refuses_a_changed_file_and_a_stopped_run_is_finished() {
    let dir = old_repo();
    let r = dir.path();
    ok(r, &["change", "new", "z", "--subject", "demo", "--title", "Z"]);
    write_brief(r, "z", &new_brief("Z", "", ""));
    commit_all(r);
    // A review pinned to the 0.30 chore `d`: the migrate writes a new version pinned to the new digest.
    let old_review = old_chore_with_review(r);
    let before = tree(r);
    let (_, digest) = plan(r);
    assert!(commit(r, None, &digest).status.success());
    let migrated = tree(r);
    assert_ne!(migrated, before);
    assert_eq!(change_reviews(r, "d").len(), 2, "the migrate wrote a new version of the chore review");

    // A later version of the review than the one the migrate wrote: the revert refuses naming it, and writes nothing.
    let rereviewed = canon_as(r, Some("reviewer"), &["review", "change", "d", "--original-spec-ref", ".canon/changes/d/brief.md", "--actor", "rev", "--role", "reviewer", "--review-outcome", "indeterminate"]);
    assert!(rereviewed.status.success(), "{}", out_text(&rereviewed));
    assert_eq!(change_reviews(r, "d").len(), 3, "a third version of the review");
    let before_refusal = tree(r);
    let out = canon(r, &["migrate", "--revert", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
    assert!(message.contains("kind=review") && message.contains("nothing was written"), "{message}");
    assert_eq!(tree(r), before_refusal, "a refused preview writes nothing");
    restore(r, &migrated);

    // The version the migrate wrote, deleted without a later one to archive it: the revert refuses naming it too, rather
    // than putting the old version back over the deletion.
    let (written, _) = change_reviews(r, "d").into_iter().find(|(path, _)| !path.contains("archive")).expect("the version the migrate wrote");
    fs::remove_file(r.join(&written)).unwrap();
    let before_refusal = tree(r);
    let out = canon(r, &["migrate", "--revert", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
    assert!(message.contains(&written) && message.contains("nothing was written"), "{message}");
    assert_eq!(tree(r), before_refusal, "a refused preview writes nothing");
    restore(r, &migrated);

    // A later edit of a rewritten brief makes the revert refuse, naming it, and nothing is written.
    let edited = format!("{}\nlater\n", read_brief(r, "b"));
    write_brief(r, "b", &edited);
    let out = canon(r, &["migrate", "--revert", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    assert!(json(&out)["error"]["message"].as_str().unwrap().contains(".canon/changes/b/brief.md"), "{}", out_text(&out));
    assert_eq!(read_brief(r, "b"), edited);
    write_brief(r, "b", &String::from_utf8(migrated[".canon/changes/b/brief.md"].clone()).unwrap());

    // The same for a brief the migrate did not rewrite (`z` was already in the 0.31 format): a later edit, not only an approval.
    let z_text = read_brief(r, "z");
    let z_edited = format!("{z_text}\nlater\n");
    write_brief(r, "z", &z_edited);
    let before_refusal = tree(r);
    let out = canon(r, &["migrate", "--revert", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    assert!(json(&out)["error"]["message"].as_str().unwrap().contains(".canon/changes/z/brief.md"), "{}", out_text(&out));
    assert_eq!(tree(r), before_refusal, "a refused preview writes nothing");
    write_brief(r, "z", &z_text);

    // A file the migrate left that was deleted since is a change too: the revert would bring it back.
    fs::remove_file(brief_file(r, "z")).unwrap();
    let before_refusal = tree(r);
    let out = canon(r, &["migrate", "--revert", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    assert!(json(&out)["error"]["message"].as_str().unwrap().contains(".canon/changes/z/brief.md"), "{}", out_text(&out));
    assert_eq!(tree(r), before_refusal, "a refused preview writes nothing");
    write_brief(r, "z", &z_text);

    // And for a record version the migrate's journal does not hold: a finding written after it.
    let finding = canon_as(r, Some("reviewer"), &["finding", "add", "--change", "z", "--round", "1", "--seq", "1", "--severity", "note", "--actor", "rev", "--summary", "later", "--promote", "--json"]);
    assert!(finding.status.success(), "{}", out_text(&finding));
    let before_refusal = tree(r);
    let out = canon(r, &["migrate", "--revert", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
    assert!(message.contains("kind=finding") && message.contains("nothing was written"), "{message}");
    assert_eq!(tree(r), before_refusal);
    let finding_path = before_refusal.keys().find(|k| k.contains("kind=finding")).cloned().expect("the finding is a file");
    fs::remove_file(r.join(finding_path)).unwrap();

    // A human's approval made after the migrate is never discarded: the approval of a brief the migrate did not rewrite
    // and a policy approval.
    ok(r, &["brief", "approve", "z", "--approver", "Ana", "--note", "ok"]);
    let refused = canon(r, &["migrate", "--revert", "--json"]);
    assert_eq!(refused.status.code(), Some(2), "{}", out_text(&refused));
    assert!(out_text(&refused).contains(".canon/changes/z/brief.md"), "{}", out_text(&refused));
    assert!(read_brief(r, "z").contains("Approved:"), "the approval is still there");
    git(r, &["checkout", "--", ".canon/changes/z"]);
    // `brief approve` recorded the policy approval too; the one the migrate left is put back after the check.
    let policy = r.join(".canon/policy.approval.json");
    let approval = migrated[".canon/policy.approval.json"].clone();
    fs::remove_file(&policy).unwrap();
    ok(r, &["policy", "approve", "--approver", "Bo"]);
    let refused = canon(r, &["migrate", "--revert", "--json"]);
    assert_eq!(refused.status.code(), Some(2), "{}", out_text(&refused));
    assert!(out_text(&refused).contains(".canon/policy.approval.json") && policy.is_file(), "{}", out_text(&refused));
    fs::write(&policy, approval).unwrap();

    // The revert puts back an Approved: digest, so committing it is the human's.
    let out = ok(r, &["migrate", "--revert", "--json"]);
    let revert_digest = json(&out)["data"]["planDigest"].as_str().unwrap().to_string();
    let refused = canon_as(r, Some("orchestrator"), &["migrate", "--revert", "--commit", "--plan-digest", &revert_digest, "--json"]);
    assert_eq!(refused.status.code(), Some(2), "{}", out_text(&refused));
    assert_eq!(json(&refused)["error"]["code"], "HUMAN_ONLY");
    let done = canon(r, &["migrate", "--revert", "--commit", "--plan-digest", &revert_digest, "--json"]);
    assert!(done.status.success(), "{}", out_text(&done));
    assert_eq!(tree(r), before, "the old briefs, digests and reviews are back byte for byte");
    let reviews = change_reviews(r, "d");
    assert_eq!(reviews.len(), 1, "the review version the migrate wrote is gone: {reviews:?}");
    assert_eq!(reviews[0].1, old_review, "the old review is back");

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

    // A run stopped after its last brief write, before its journal was complete and the backup sealed: the next run has
    // nothing left to write but finishes the journal and seals the backup, so a later edit is refused again.
    commit_all(r);
    let (_, digest) = plan(r);
    assert!(commit(r, None, &digest).status.success());
    let backup = fs::read_dir(r.join(".canon/migrate-backup")).unwrap().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.join("briefs-journal.json").is_file()).max().unwrap();
    let journal_path = backup.join("briefs-journal.json");
    let mut journal: Value = serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
    journal["complete"] = Value::Bool(false);
    fs::write(&journal_path, serde_json::to_vec(&journal).unwrap()).unwrap();
    fs::remove_file(backup.join("tree-after.json")).unwrap();
    let (_, digest) = plan(r);
    let out = commit(r, None, &digest);
    assert!(out.status.success(), "{}", out_text(&out));
    let journal: Value = serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
    assert_eq!(journal["complete"], true, "the same journal was finished");
    assert!(backup.join("tree-after.json").is_file(), "the resumed run sealed the backup");
    let z_text = read_brief(r, "z");
    write_brief(r, "z", &format!("{z_text}\nlater\n"));
    let before_refusal = tree(r);
    let out = canon(r, &["migrate", "--revert", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    assert!(json(&out)["error"]["message"].as_str().unwrap().contains(".canon/changes/z/brief.md"), "{}", out_text(&out));
    assert_eq!(tree(r), before_refusal, "a refused preview writes nothing");
}
