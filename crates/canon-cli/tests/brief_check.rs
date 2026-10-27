//! Integration tests for `canon brief check` and `canon brief show` (0.19
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
    // The report rides in `data` (blockers fail the envelope but keep it); a bare
    // failure has no `data`, so its `error` is what the caller reads.
    let report = if json["data"].is_object() { json["data"].clone() } else { json };
    (out.status.code().unwrap(), report)
}

fn rules(report: &Value) -> Vec<String> {
    report["findings"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap().to_string()).collect()
}

/// A complete English PRD brief with flow steps `F1`, `F2`; `extra` is
/// appended (waivers, an approval line).
fn en_brief(extra: &str) -> String {
    format!(
        "# Add login\n\n\
## Intent\n\nAdd login\n\nVisitors lose their cart when they switch devices.\n\n\
## Desired outcome\n\nA visitor who signs in sees the saved cart. ==**The cart follows the account.**==\n\n\
## User flow\n\n1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n2. F2 The visitor submits a wrong password → sees an error → can retry.\n\n\
## Step detail\n\n### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n### F2 Failed submit\n\nFive failures lock the form for thirty seconds.\n\n\
## Acceptance\n\n- F1: valid credentials open the catalog and set a session cookie.\n- F2: a wrong password shows the error and sets no cookie. ==**F2 covers the failure path.**==\n\n\
## Questions and assumptions\n\nNone.\n\n\
## Out of scope\n\nSocial sign-in.\n\n{extra}"
    )
}

#[test]
fn a_fresh_ko_repo_writes_a_ko_brief_whose_only_findings_are_placeholders_until_it_is_filled_in() {
    let dir = repo_with_change(Some("ko"));
    assert!(std::fs::read_to_string(dir.path().join("canon.yaml")).unwrap().contains("\nlanguage: ko\n"));
    let brief = std::fs::read_to_string(brief_path(dir.path())).unwrap();
    assert!(brief.contains("## Intent") && brief.contains("## User flow") && !brief.contains("<!--"), "{brief}");

    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 1);
    let found = rules(&report);
    assert_eq!(found.len(), 6, "every section but Intent (the title) is empty: {report}");
    assert!(found.iter().all(|r| r == "brief.placeholder"), "{found:?}");
    assert_eq!(report["counts"], serde_json::json!({"blockers": 6, "advisories": 0}));

    let filled = "# Add login\n\n\
## Intent\n\nAdd login\n\n방문자는 기기를 바꾸면 장바구니를 잃는다.\n\n\
## Desired outcome\n\n로그인한 방문자는 저장한 장바구니를 본다. ==**장바구니는 계정을 따라간다.**==\n\n\
## User flow\n\n1. F1 방문자는 입력란을 보고 → 자격 증명을 제출하면 → 상품 목록으로 이동한다. ==**성공한 로그인만 세션을 만든다.**==\n\n\
## Step detail\n\n### F1 제출\n\n세션은 서버가 발급한다. ==**비밀번호는 로그에 남기지 않는다.**==\n\n\
## Acceptance\n\n- F1: 올바른 자격 증명이면 목록이 열리고, 틀리면 오류가 보인다. ==**실패 경로도 F1이 다룬다.**==\n\n\
## Questions and assumptions\n\n없음.\n\n\
## Out of scope\n\n소셜 로그인.\n";
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
    let brief = &status["data"]["subjects"][0]["briefs"][0];
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
    assert_eq!(code, 1);
    let found = rules(&report);
    assert!(found.contains(&"brief.flow-duplicate-id".to_string()), "{found:?}");
    assert!(found.contains(&"brief.detail-mismatch".to_string()), "{found:?}");

    // Acceptance moved above the flow.
    let text = en_brief("");
    let (flow_at, detail_at, acceptance_at, questions_at) =
        (text.find("## User flow").unwrap(), text.find("## Step detail").unwrap(), text.find("## Acceptance").unwrap(), text.find("## Questions and assumptions").unwrap());
    let swapped = format!("{}{}{}{}", &text[..flow_at], &text[acceptance_at..questions_at], &text[flow_at..acceptance_at], &text[questions_at..]);
    assert!(detail_at < acceptance_at);
    std::fs::write(brief_path(dir.path()), swapped).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 1);
    assert!(rules(&report).contains(&"brief.section-order".to_string()), "{report}");

    // A section whose marker is gone.
    std::fs::write(brief_path(dir.path()), en_brief("").replace("## Out of scope", "Out of scope")).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 1);
    assert!(rules(&report).contains(&"brief.section-missing".to_string()), "{report}");
}

#[test]
fn a_waiver_clears_one_rule_in_one_section_is_listed_and_needs_a_reason() {
    let dir = repo_with_change(None);
    let with_todo = en_brief("").replace("Social sign-in.", "TODO decide the provider list.");
    std::fs::write(brief_path(dir.path()), &with_todo).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!((code, rules(&report)), (1, vec!["brief.placeholder".to_string()]));
    assert_eq!(report["findings"][0]["section"], "out-of-scope");

    let waived = waive(&with_todo, "brief.placeholder", "out-of-scope", "the provider list is a policy question", None);
    std::fs::write(brief_path(dir.path()), waived).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!((code, rules(&report)), (0, Vec::<String>::new()), "{report}");
    assert_eq!(report["waivers"][0]["severity"], "blocker");
    assert_eq!(report["waivers"][0]["applied"], true);
    assert_eq!(report["waivers"][0]["reason"], "the provider list is a policy question");

    // Another section's waiver does not clear it; a missing reason is a blocker itself.
    let wrong_section = waive(&with_todo, "brief.placeholder", "outcome", "x", None);
    std::fs::write(brief_path(dir.path()), wrong_section).unwrap();
    assert_eq!(check(dir.path(), &[]).0, 1);
    let no_reason = waive(&with_todo, "brief.placeholder", "out-of-scope", "", None);
    std::fs::write(brief_path(dir.path()), no_reason).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 1);
    assert!(rules(&report).contains(&"brief.waiver-invalid".to_string()), "{report}");
}

#[test]
fn prose_rules_flag_normative_intent_and_question_sections_in_en_and_ko_and_never_block() {
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
    let in_intent: Vec<&str> = findings.iter().filter(|f| f["section"] == "intent").map(|f| f["rule"].as_str().unwrap()).collect();
    assert_eq!(in_intent, ["prose.evaluative", "prose.connective", "prose.degree"], "intent gets the phrase rules and none of the length or key-sentence rules: {report}");
    assert!(findings.iter().all(|f| f["section"] != "out-of-scope"), "out of scope is not judged: {report}");

    // A normative section without its key sentence.
    std::fs::write(brief_path(dir.path()), en_brief("").replace(" ==**The cart follows the account.**==", "")).unwrap();
    let (_, report) = check(dir.path(), &[]);
    assert_eq!(report["findings"][0]["rule"], "prose.key-sentence");
    assert_eq!(report["findings"][0]["section"], "outcome");

    // Korean list on a ko repo; a tag canon ships no list for gets none.
    let ko = repo_with_change(Some("ko"));
    let brief = std::fs::read_to_string(brief_path(ko.path())).unwrap();
    let body = "따라서 이 기능은 매우 중요하다. 로그인이 되는지 확인해요. 이것은 폼이 아니라 세션이다.";
    let flagged = brief.replace("## Desired outcome\n", &format!("## Desired outcome\n\n{body}\n"));
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

/// Dogfood eval 7: a flow step is three clauses ("sees → does → becomes"),
/// so length is judged per clause; one overlong clause still fires.
#[test]
fn a_flow_step_is_measured_clause_by_clause() {
    let dir = repo_with_change(None);
    let clause = "word ".repeat(20);
    let long = "word ".repeat(40);
    let brief = en_brief("")
        .replace("1. F1 The visitor sees the form → submits credentials → lands on the catalog.", &format!("1. F1 {clause}→ {clause}→ {clause}."))
        .replace("2. F2 The visitor submits a wrong password → sees an error → can retry.", &format!("2. F2 {clause}→ {long}→ {clause}."));
    std::fs::write(brief_path(dir.path()), brief).unwrap();
    let (_, report) = check(dir.path(), &[]);
    let long_lines: Vec<&str> = report["findings"].as_array().unwrap().iter().filter(|f| f["rule"] == "prose.sentence-length").filter_map(|f| f["excerpt"].as_str()).collect();
    assert_eq!(long_lines.len(), 1, "only the step with a 40-word clause: {report}");
    assert!(long_lines[0].starts_with("F2"), "{long_lines:?}");
}

/// A digest-less approval line: enough for the coverage rules to apply.
const APPROVED: &str = "Approved: Ana 2026-10-14 — \"Go.\"\n";

#[test]
fn flow_coverage_blocks_once_the_brief_is_approved_and_the_subject_is_building() {
    let dir = repo_with_change(None);
    std::fs::write(brief_path(dir.path()), en_brief("")).unwrap();
    assert_eq!(check(dir.path(), &[]).0, 0, "before building, no coverage rule applies");

    ok(dir.path(), &["subject", "move", "demo", "--to", "specced"]);
    ok(dir.path(), &["subject", "move", "demo", "--to", "building"]);
    // A new scope on a subject already past `building` (dogfood eval 7):
    // its scenarios come after the approval, so none is due yet.
    let (code, report) = check(dir.path(), &[]);
    assert_eq!((code, report["phase"].as_str(), rules(&report)), (0, Some("shaping"), Vec::<String>::new()), "{report}");

    std::fs::write(brief_path(dir.path()), en_brief(APPROVED)).unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!(code, 1);
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
    assert_eq!(code, 1);
    assert_eq!(rules(&report), vec!["brief.flow-no-failure"; 2], "{report}");

    std::fs::write(
        feature_dir.join("login.feature"),
        "Feature: Login\n\n  @subject:demo @flow:add-login.F1 @case:failure\n  @world.login.01\n  Scenario: Valid sign-in\n    Given a form\n\n  @subject:demo @flow:add-login.F2 @case:failure\n  @world.login.02\n  Scenario: Wrong password\n    Given a form\n",
    )
    .unwrap();
    let (code, report) = check(dir.path(), &[]);
    assert_eq!((code, rules(&report)), (0, Vec::<String>::new()), "{report}");
}

/// 0.19 D4: `scenario new --flow` writes `@flow:<change>.<F-id>` (the
/// change is the subject's newest adopted one), refuses a step the brief
/// does not define, and `canon status` lists each step with its scenarios.
#[test]
fn scenario_new_flow_tags_the_step_and_status_lists_per_step_coverage() {
    let dir = repo_with_change(None);
    let repo = dir.path();
    std::fs::write(brief_path(repo), en_brief(APPROVED)).unwrap();
    assert!(!repo.join(".canon/changes/add-login/tasks.md").exists(), "change new writes no tasks.md");
    ok(repo, &["subject", "move", "demo", "--to", "specced"]);
    ok(repo, &["subject", "move", "demo", "--to", "building"]);

    let refused = run(repo, &["scenario", "new", "world.login.01", "--title", "x", "--subject", "demo", "--flow", "F9"]);
    assert_eq!(refused.status.code(), Some(2), "{}", stderr(&refused));
    assert!(stderr(&refused).contains("defines no flow step `F9` (it has F1, F2)"), "{}", stderr(&refused));
    assert!(!repo.join(".canon/specs/kind=feature/area=world/login.feature").exists(), "a refused flow writes nothing");

    ok(repo, &["scenario", "new", "world.login.01", "--title", "Valid sign-in", "--subject", "demo", "--flow", "F1"]);
    ok(repo, &["scenario", "new", "world.login.02", "--title", "Wrong password", "--subject", "demo", "--flow", "F1", "--case", "failure"]);
    let feature = std::fs::read_to_string(repo.join(".canon/specs/kind=feature/area=world/login.feature")).unwrap();
    assert!(feature.contains("  @flow:add-login.F1\n  @world.login.01\n"), "{feature}");

    // F1 is covered with a failure case; F2 has no scenario: the brief blocks.
    let (code, report) = check(repo, &[]);
    assert_eq!((code, rules(&report)), (1, vec!["brief.flow-uncovered".to_string()]), "{report}");
    let out = run(repo, &["status"]);
    let text = stdout(&out);
    assert!(text.contains("flow F1\n") && text.contains("world.login.02 \"Wrong password\" [failure] — evidence: none, review: missing"), "{text}");
    assert!(text.contains("flow F2 — no scenario"), "{text}");
    let json: Value = serde_json::from_str(&stdout(&run(repo, &["status", "--json"]))).unwrap();
    let flow = &json["data"]["subjects"][0]["flow"];
    assert_eq!((flow[0]["id"].as_str(), flow[1]["gaps"][0].as_str()), (Some("F1"), Some("no scenario")), "{flow}");

    ok(repo, &["scenario", "new", "world.login.03", "--title", "Retry", "--subject", "demo", "--flow", "F2"]);
    let text = stdout(&run(repo, &["status"]));
    assert!(text.contains("flow F2 — no failure case"), "{text}");
}

#[test]
fn operational_errors_carry_a_string_code_and_the_registry_exit() {
    let dir = TempDir::new().unwrap();
    ok(dir.path(), &["init", "--providers", "none"]);
    let (code, report) = check(dir.path(), &[]);
    assert_eq!((code, report["error"]["code"].as_str()), (2, Some("NO_BRIEF")));

    ok(dir.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(dir.path(), &["change", "new", "one", "--subject", "demo", "--title", "One"]);
    ok(dir.path(), &["change", "new", "two", "--subject", "demo", "--title", "Two"]);
    let (code, report) = check(dir.path(), &[]);
    assert_eq!((code, report["error"]["code"].as_str()), (2, Some("CHANGE_AMBIGUOUS")));
    let (code, report) = check(dir.path(), &["one"]);
    assert_eq!((code, report["findings"].is_array()), (1, true), "naming the change resolves the ambiguity");
    let (code, report) = check(dir.path(), &["missing"]);
    assert_eq!((code, report["error"]["code"].as_str()), (2, Some("BRIEF_NOT_FOUND")));

    std::fs::write(dir.path().join("canon.yaml"), std::fs::read_to_string(dir.path().join("canon.yaml")).unwrap() + "language: [ko]\n").unwrap();
    let (code, report) = check(dir.path(), &["one"]);
    assert_eq!((code, report["error"]["code"].as_str()), (2, Some("CONFIG_UNREADABLE")));
}

#[test]
fn brief_show_names_the_next_steps_from_the_findings_and_never_edits() {
    let dir = repo_with_change(None);
    let before = std::fs::read(brief_path(dir.path())).unwrap();
    let out = run(dir.path(), &["brief", "show"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("next 1: shape"), "{}", stdout(&out));
    assert_eq!(std::fs::read(brief_path(dir.path())).unwrap(), before, "`canon brief show` never edits");

    let questions = en_brief("").replace("None.\n", "Should guests keep their cart?\n");
    std::fs::write(brief_path(dir.path()), questions).unwrap();
    let json: Value = serde_json::from_str(&stdout(&run(dir.path(), &["brief", "show", "--json"]))).unwrap();
    let steps: Vec<&str> = json["data"]["next"].as_array().unwrap().iter().map(|s| s["step"].as_str().unwrap()).collect();
    assert_eq!(steps, vec!["clarify", "approve"], "{json}");

    std::fs::write(brief_path(dir.path()), en_brief("Approved: Ana 2026-10-14 — \"go\"\n")).unwrap();
    let json: Value = serde_json::from_str(&stdout(&run(dir.path(), &["brief", "show", "--json"]))).unwrap();
    assert_eq!((json["data"]["state"].as_str(), json["data"]["next"].as_array().unwrap().len()), (Some("approved"), 0), "{json}");
}

#[test]
fn init_language_is_validated_recorded_and_checked() {
    let dir = TempDir::new().unwrap();
    let out = run(dir.path(), &["init", "--providers", "none", "--language", "korean language"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(!dir.path().join("canon.yaml").exists(), "nothing is written on a refused tag");

    ok(dir.path(), &["init", "--providers", "none", "--language", "ko"]);
    let out = run(dir.path(), &["init", "--check"]);
    assert!(stdout(&out).contains("[PASS] language (ko)"), "{}", stdout(&out));

    std::fs::write(dir.path().join("canon.yaml"), std::fs::read_to_string(dir.path().join("canon.yaml")).unwrap().replace("language: ko", "language: 한국어")).unwrap();
    let out = run(dir.path(), &["init", "--check"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("[FAIL] language"), "{}", stdout(&out));
}

/// A filled-in English chore brief; `category` is the category section's text.
fn chore_brief(category: &str) -> String {
    format!(
        "---\nkind: chore\n---\n\n# Bump the client\n\n\
## Intent\n\nBump the client\n\nThe client has a security fix.\n\n\
## Category\n\n{category}\n\n\
## Invariants\n\n- Requests and responses stay byte-identical. ==**No behavior changes.**==\n\n\
## Checks\n\n- The suite runs again at the chore commit. ==**The suite is the check.**==\n\n\
## Out of scope\n\nNone.\n"
    )
}

/// 0.19 D5: `change new --kind chore` writes the kind marker and the five
/// chore sections in the repo's language; the chore rule set blocks the
/// template, a category outside the five, a flow/detail/acceptance
/// section and "none" invariants; and the flow-coverage rules never apply,
/// even once the subject is building.
#[test]
fn a_chore_brief_is_written_and_judged_by_the_chore_rule_set() {
    let dir = repo_with_change(Some("ko"));
    ok(dir.path(), &["change", "new", "bump-client", "--subject", "demo", "--change-kind", "chore", "--title", "클라이언트 올리기"]);
    let path = dir.path().join(".canon/changes/bump-client/brief.md");
    let brief = std::fs::read_to_string(&path).unwrap();
    assert!(brief.starts_with("---\nkind: chore\n---\n\n") && brief.contains("## Invariants") && brief.contains("## Category"), "{brief}");
    assert!(!brief.contains("## User flow") && !brief.contains("<!--"), "{brief}");
    let (code, report) = check(dir.path(), &["bump-client"]);
    assert_eq!(code, 1);
    assert_eq!(rules(&report), vec!["brief.placeholder"; 4], "{report}");

    let en = |name: &str, brief: String| {
        std::fs::write(&path, brief).unwrap();
        check(dir.path(), &[name])
    };
    let (code, report) = en("bump-client", chore_brief("`Dependency`."));
    assert_eq!((code, rules(&report)), (0, Vec::<String>::new()), "a clean chore, category case and punctuation aside: {report}");

    let (code, report) = en("bump-client", chore_brief("upgrade"));
    assert_eq!((code, rules(&report)), (1, vec!["brief.chore-category".to_string()]), "{report}");
    let (_, report) = en("bump-client", chore_brief("refactor and build"));
    assert_eq!(rules(&report), vec!["brief.chore-category"], "two categories are not one: {report}");

    let with_flow = chore_brief("ci").replace("## Invariants", "## User flow\n\n1. F1 x\n\n## Invariants");
    let (code, report) = en("bump-client", with_flow);
    assert_eq!((code, rules(&report)), (1, vec!["brief.chore-section".to_string()]), "{report}");

    let none_invariants = chore_brief("ci").replace("- Requests and responses stay byte-identical. ==**No behavior changes.**==", "None.");
    let (code, report) = en("bump-client", none_invariants);
    assert!(code == 1 && rules(&report).contains(&"brief.chore-empty".to_string()), "{report}");

    std::fs::write(&path, format!("{}\n{APPROVED}", chore_brief("infra"))).unwrap();
    ok(dir.path(), &["subject", "move", "demo", "--to", "specced"]);
    ok(dir.path(), &["subject", "move", "demo", "--to", "building"]);
    let (code, report) = check(dir.path(), &["bump-client"]);
    assert_eq!((code, report["phase"].as_str(), rules(&report)), (0, Some("building"), Vec::<String>::new()), "a chore has no flow to cover: {report}");

    let out = run(dir.path(), &["change", "new", "x", "--subject", "demo", "--change-kind", "epic", "--title", "x"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
}

/// `text` with one more entry in its frontmatter `waive` list (a new frontmatter when it has none).
fn waive(text: &str, rule: &str, section: &str, reason: &str, matching: Option<&str>) -> String {
    let q = |v: &str| serde_json::to_string(v).unwrap();
    let mut entry = format!("  - rule: {}\n    section: {}\n    reason: {}\n", q(rule), q(section), q(reason));
    if let Some(m) = matching {
        entry.push_str(&format!("    match: {}\n", q(m)));
    }
    match text.strip_prefix("---\nwaive:\n") {
        Some(rest) => format!("---\nwaive:\n{entry}{rest}"),
        None => format!("---\nwaive:\n{entry}---\n\n{text}"),
    }
}
