//! Integration tests for `canon brief check` and bare `canon brief` (0.19
//! D2), the PRD brief `canon change new` writes (D1) and `language:` (D6),
//! against the actually-built `canon` binary.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).output().expect("spawn canon binary")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn ok(repo: &Path, args: &[&str]) {
    let out = run(repo, args);
    assert!(out.status.success(), "{args:?}: {}", stderr(&out));
}

/// A `canon init` repo (language `lang` when given) with subject `demo`
/// and change `add-login` written by `canon change new`.
fn repo_with_change(lang: Option<&str>) -> TempDir {
    let dir = TempDir::new().unwrap();
    let mut init = vec!["init", "--providers", "none"];
    if let Some(lang) = lang {
        init.extend(["--language", lang]);
    }
    ok(dir.path(), &init);
    ok(dir.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(dir.path(), &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    dir
}

fn brief_path(repo: &Path) -> std::path::PathBuf {
    repo.join(".canon/changes/add-login/brief.md")
}

fn check(repo: &Path, extra: &[&str]) -> (i32, Value) {
    let mut args = vec!["brief", "check", "--json"];
    args.extend(extra);
    let out = run(repo, &args);
    let json: Value = serde_json::from_str(&stdout(&out)).unwrap_or_else(|e| panic!("not json ({e}): {}\n{}", stdout(&out), stderr(&out)));
    (out.status.code().unwrap(), json)
}

fn rules(report: &Value) -> Vec<String> {
    report["findings"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap().to_string()).collect()
}

/// A complete English PRD brief with flow steps `F1`, `F2`; `extra` is
/// appended (waivers, an approval line).
fn en_brief(extra: &str) -> String {
    format!(
        "# Add login\n\n\
<!-- canon:section intent role=context -->\n## Intent\n\nAdd login\n\nVisitors lose their cart when they switch devices.\n\n\
<!-- canon:section outcome role=normative -->\n## Desired outcome\n\nA visitor who signs in sees the saved cart. ==**The cart follows the account.**==\n\n\
<!-- canon:section flow role=normative -->\n## User flow\n\n1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n2. F2 The visitor submits a wrong password → sees an error → can retry.\n\n\
<!-- canon:section detail role=normative -->\n## Step detail\n\n### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n### F2 Failed submit\n\nFive failures lock the form for thirty seconds.\n\n\
<!-- canon:section acceptance role=normative -->\n## Acceptance\n\n- F1: valid credentials open the catalog and set a session cookie.\n- F2: a wrong password shows the error and sets no cookie. ==**F2 covers the failure path.**==\n\n\
<!-- canon:section questions role=question -->\n## Questions and assumptions\n\nNone.\n\n\
<!-- canon:section out-of-scope role=context -->\n## Out of scope\n\nSocial sign-in.\n\n{extra}"
    )
}

#[test]
fn a_fresh_ko_repo_writes_a_ko_brief_whose_only_findings_are_placeholders_until_it_is_filled_in() {
    let dir = repo_with_change(Some("ko"));
    assert!(std::fs::read_to_string(dir.path().join("canon.yaml")).unwrap().contains("\nlanguage: ko\n"));
    let brief = std::fs::read_to_string(brief_path(dir.path())).unwrap();
    assert!(brief.contains("## 의도") && brief.contains("<!-- canon:section flow role=normative -->"), "{brief}");

    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 2);
    let found = rules(&report);
    assert_eq!(found.len(), 7, "{report}");
    assert!(found.iter().all(|r| r == "brief.placeholder"), "{found:?}");
    assert_eq!(report["counts"], serde_json::json!({"blockers": 7, "advisories": 0}));

    let filled = "# Add login\n\n\
<!-- canon:section intent role=context -->\n## 의도\n\nAdd login\n\n방문자는 기기를 바꾸면 장바구니를 잃는다.\n\n\
<!-- canon:section outcome role=normative -->\n## 원하는 결과\n\n로그인한 방문자는 저장한 장바구니를 본다. ==**장바구니는 계정을 따라간다.**==\n\n\
<!-- canon:section flow role=normative -->\n## 사용자 흐름\n\n1. F1 방문자는 입력란을 보고 → 자격 증명을 제출하면 → 상품 목록으로 이동한다. ==**성공한 로그인만 세션을 만든다.**==\n\n\
<!-- canon:section detail role=normative -->\n## 단계 상세\n\n### F1 제출\n\n세션은 서버가 발급한다. ==**비밀번호는 로그에 남기지 않는다.**==\n\n\
<!-- canon:section acceptance role=normative -->\n## 인수 조건\n\n- F1: 올바른 자격 증명이면 목록이 열리고, 틀리면 오류가 보인다. ==**실패 경로도 F1이 다룬다.**==\n\n\
<!-- canon:section questions role=question -->\n## 질문과 가정\n\n없음.\n\n\
<!-- canon:section out-of-scope role=context -->\n## 범위 밖\n\n소셜 로그인.\n";
    std::fs::write(brief_path(dir.path()), filled).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!((code, rules(&report)), (0, Vec::<String>::new()), "{report}");
}

#[test]
fn a_pre_0_19_brief_checks_as_legacy_advisory_and_status_shows_no_finding_counts() {
    let dir = repo_with_change(None);
    std::fs::write(brief_path(dir.path()), "# Add login\n\n## Why\n\nAdd login\n\n## What Changes\n\n- the form\n").unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 0, "{report}");
    assert_eq!(rules(&report), vec!["brief.legacy"]);
    assert_eq!(report["findings"][0]["severity"], "advisory");
    assert_eq!(report["legacy"], true);

    let status: Value = serde_json::from_str(&stdout(&run(dir.path(), &["status", "--json"]))).unwrap();
    let brief = &status["subjects"][0]["brief"];
    assert!(brief.get("findings").is_none(), "a legacy brief reads as before: {brief}");
}

#[test]
fn structure_blockers_name_duplicate_ids_a_detail_mismatch_and_section_order() {
    let dir = repo_with_change(None);
    std::fs::write(brief_path(dir.path()), en_brief("")).unwrap();
    assert_eq!(check(dir.path(), &[]).0, 0);

    // F2 repeated in the flow; detail lacks F2's subheading.
    let broken = en_brief("").replace("2. F2 The visitor", "2. F1 The visitor").replace("### F2 Failed submit", "### F3 Failed submit");
    std::fs::write(brief_path(dir.path()), broken).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 2);
    let found = rules(&report);
    assert!(found.contains(&"brief.flow-duplicate-id".to_string()), "{found:?}");
    assert!(found.contains(&"brief.detail-mismatch".to_string()), "{found:?}");

    // Acceptance moved above the flow.
    let text = en_brief("");
    let (flow_at, detail_at, acceptance_at, questions_at) = (
        text.find("<!-- canon:section flow").unwrap(),
        text.find("<!-- canon:section detail").unwrap(),
        text.find("<!-- canon:section acceptance").unwrap(),
        text.find("<!-- canon:section questions").unwrap(),
    );
    let swapped = format!("{}{}{}{}", &text[..flow_at], &text[acceptance_at..questions_at], &text[flow_at..acceptance_at], &text[questions_at..]);
    assert!(detail_at < acceptance_at);
    std::fs::write(brief_path(dir.path()), swapped).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 2);
    assert!(rules(&report).contains(&"brief.section-order".to_string()), "{report}");

    // A section whose marker is gone.
    std::fs::write(brief_path(dir.path()), en_brief("").replace("<!-- canon:section out-of-scope role=context -->\n", "")).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 2);
    assert!(rules(&report).contains(&"brief.section-missing".to_string()), "{report}");
}

#[test]
fn a_waiver_clears_one_rule_in_one_section_is_listed_and_needs_a_reason() {
    let dir = repo_with_change(None);
    let with_todo = en_brief("").replace("Social sign-in.", "TODO decide the provider list.");
    std::fs::write(brief_path(dir.path()), &with_todo).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!((code, rules(&report)), (2, vec!["brief.placeholder".to_string()]));
    assert_eq!(report["findings"][0]["section"], "out-of-scope");

    let waived = format!("{with_todo}<!-- canon:waive brief.placeholder out-of-scope \"the provider list is a policy question\" -->\n");
    std::fs::write(brief_path(dir.path()), waived).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!((code, rules(&report)), (0, Vec::<String>::new()), "{report}");
    assert_eq!(report["waivers"][0]["severity"], "blocker");
    assert_eq!(report["waivers"][0]["applied"], true);
    assert_eq!(report["waivers"][0]["reason"], "the provider list is a policy question");

    // Another section's waiver does not clear it; a missing reason is a blocker itself.
    let wrong_section = format!("{with_todo}<!-- canon:waive brief.placeholder outcome \"x\" -->\n");
    std::fs::write(brief_path(dir.path()), wrong_section).unwrap();
    assert_eq!(check(dir.path(), &[]).0, 2);
    let no_reason = format!("{with_todo}<!-- canon:waive brief.placeholder out-of-scope -->\n");
    std::fs::write(brief_path(dir.path()), no_reason).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 2);
    assert!(rules(&report).contains(&"brief.waiver-invalid".to_string()), "{report}");
}

#[test]
fn prose_rules_flag_only_normative_sections_in_en_and_ko_and_never_block() {
    let dir = repo_with_change(None);
    let wordy = en_brief("")
        .replace("The server issues the session.", "Therefore it is important that the server very reliably issues the session, not by chance but by design.")
        .replace("Visitors lose their cart when they switch devices.", "Therefore it is important to be very clear here.");
    std::fs::write(brief_path(dir.path()), wordy).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 0, "advisories never fail the check: {report}");
    let findings = report["findings"].as_array().unwrap();
    let in_detail: Vec<&str> = findings.iter().filter(|f| f["section"] == "detail").map(|f| f["rule"].as_str().unwrap()).collect();
    for rule in ["prose.connective", "prose.evaluative", "prose.degree", "prose.contrast"] {
        assert!(in_detail.contains(&rule), "{rule} missing from {in_detail:?}");
    }
    assert!(findings.iter().all(|f| f["section"] != "intent"), "context sections are skipped: {report}");

    // A normative section without its key sentence.
    std::fs::write(brief_path(dir.path()), en_brief("").replace(" ==**The cart follows the account.**==", "")).unwrap();
    let (_, report) = check(dir.path(), &[]);
    assert_eq!(report["findings"][0]["rule"], "prose.key-sentence");
    assert_eq!(report["findings"][0]["section"], "outcome");

    // Korean list on a ko repo; a tag canon ships no list for gets none.
    let ko = repo_with_change(Some("ko"));
    let brief = std::fs::read_to_string(brief_path(ko.path())).unwrap();
    let body = "따라서 이 기능은 매우 중요하다. 로그인이 되는지 확인해요. 이것은 폼이 아니라 세션이다.";
    let flagged = brief.replace("<!-- canon:hint 출시되면 사용자 입장에서 눈에 보이는 상태. -->", body);
    std::fs::write(brief_path(ko.path()), &flagged).unwrap();
    let (_, report) = check(ko.path(), &[]);
    let in_outcome: Vec<String> = report["findings"].as_array().unwrap().iter().filter(|f| f["section"] == "outcome" && f["rule"].as_str().unwrap().starts_with("prose.")).map(|f| f["rule"].as_str().unwrap().to_string()).collect();
    for rule in ["prose.connective", "prose.evaluative", "prose.degree", "prose.colloquial", "prose.contrast"] {
        assert!(in_outcome.contains(&rule.to_string()), "{rule} missing from {in_outcome:?}");
    }

    let fr = repo_with_change(Some("fr"));
    assert!(std::fs::read_to_string(brief_path(fr.path())).unwrap().contains("## Intent"), "an unshipped tag gets en headings");
    std::fs::write(brief_path(fr.path()), en_brief("").replace("The server issues the session.", "Therefore it is very important.")).unwrap();
    let (code, report) = check(fr.path(), &[]);
    assert_eq!((code, rules(&report)), (0, Vec::<String>::new()), "no prose rules for fr: {report}");
}

#[test]
fn flow_coverage_blocks_once_the_subject_is_building_and_reads_the_flow_tag() {
    let dir = repo_with_change(None);
    std::fs::write(brief_path(dir.path()), en_brief("")).unwrap();
    assert_eq!(check(dir.path(), &[]).0, 0, "before building, no coverage rule applies");

    ok(dir.path(), &["subject", "status", "demo", "specced"]);
    ok(dir.path(), &["subject", "status", "demo", "building"]);
    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 2);
    assert_eq!(report["phase"], "building");
    assert_eq!(rules(&report), vec!["brief.flow-uncovered", "brief.flow-uncovered"], "{report}");

    let feature_dir = dir.path().join(".canon/specs/kind=feature/area=world");
    std::fs::create_dir_all(&feature_dir).unwrap();
    std::fs::write(
        feature_dir.join("login.feature"),
        "Feature: Login\n\n  @subject:demo @flow:add-login.F1\n  @world.login.01\n  Scenario: Valid sign-in\n    Given a form\n\n  @subject:demo @flow:add-login.F2 @case:happy\n  @world.login.02\n  Scenario: Wrong password\n    Given a form\n",
    )
    .unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 2);
    assert_eq!(rules(&report), vec!["brief.flow-no-failure"; 2], "{report}");

    std::fs::write(
        feature_dir.join("login.feature"),
        "Feature: Login\n\n  @subject:demo @flow:add-login.F1 @case:failure\n  @world.login.01\n  Scenario: Valid sign-in\n    Given a form\n\n  @subject:demo @flow:add-login.F2 @case:failure\n  @world.login.02\n  Scenario: Wrong password\n    Given a form\n",
    )
    .unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!((code, rules(&report)), (0, Vec::<String>::new()), "{report}");
}

#[test]
fn operational_errors_carry_a_string_code_and_exit_1() {
    let dir = TempDir::new().unwrap();
    ok(dir.path(), &["init", "--providers", "none"]);
    let (code, report) = check(dir.path(), &[]);
    assert_eq!((code, report["error"]["code"].as_str()), (1, Some("NO_BRIEF")));

    ok(dir.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(dir.path(), &["change", "new", "one", "--subject", "demo", "--title", "One"]);
    ok(dir.path(), &["change", "new", "two", "--subject", "demo", "--title", "Two"]);
    let (code, report) = check(dir.path(), &[]);
    assert_eq!((code, report["error"]["code"].as_str()), (1, Some("CHANGE_AMBIGUOUS")));
    let (code, report) = check(dir.path(), &["--change", "one"]);
    assert_eq!((code, report["findings"].is_array()), (2, true), "naming the change resolves the ambiguity");
    let (code, report) = check(dir.path(), &["--change", "missing"]);
    assert_eq!((code, report["error"]["code"].as_str()), (1, Some("BRIEF_NOT_FOUND")));

    std::fs::write(dir.path().join("canon.yaml"), std::fs::read_to_string(dir.path().join("canon.yaml")).unwrap() + "language: [ko]\n").unwrap();
    let (code, report) = check(dir.path(), &["--change", "one"]);
    assert_eq!((code, report["error"]["code"].as_str()), (1, Some("CONFIG_UNREADABLE")));
}

#[test]
fn bare_brief_names_the_next_steps_from_the_findings_and_never_edits() {
    let dir = repo_with_change(None);
    let before = std::fs::read(brief_path(dir.path())).unwrap();
    let out = run(dir.path(), &["brief"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("next 1: shape"), "{}", stdout(&out));
    assert_eq!(std::fs::read(brief_path(dir.path())).unwrap(), before, "bare `canon brief` never edits");

    let questions = en_brief("").replace("None.\n", "Should guests keep their cart?\n");
    std::fs::write(brief_path(dir.path()), questions).unwrap();
    let json: Value = serde_json::from_str(&stdout(&run(dir.path(), &["brief", "--json"]))).unwrap();
    let steps: Vec<&str> = json["next"].as_array().unwrap().iter().map(|s| s["step"].as_str().unwrap()).collect();
    assert_eq!(steps, vec!["clarify", "approve"], "{json}");

    std::fs::write(brief_path(dir.path()), en_brief("Approved: Ana 2026-10-14 — \"go\"\n")).unwrap();
    let json: Value = serde_json::from_str(&stdout(&run(dir.path(), &["brief", "--json"]))).unwrap();
    assert_eq!((json["state"].as_str(), json["next"].as_array().unwrap().len()), (Some("approved"), 0), "{json}");
}

#[test]
fn init_language_is_validated_recorded_and_checked() {
    let dir = TempDir::new().unwrap();
    let out = run(dir.path(), &["init", "--providers", "none", "--language", "korean language"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(!dir.path().join("canon.yaml").exists(), "nothing is written on a refused tag");

    ok(dir.path(), &["init", "--providers", "none", "--language", "ko"]);
    let out = run(dir.path(), &["init", "--check-config"]);
    assert!(stdout(&out).contains("[PASS] language (ko)"), "{}", stdout(&out));

    std::fs::write(dir.path().join("canon.yaml"), std::fs::read_to_string(dir.path().join("canon.yaml")).unwrap().replace("language: ko", "language: 한국어")).unwrap();
    let out = run(dir.path(), &["init", "--check-config"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("[FAIL] language"), "{}", stdout(&out));
}
