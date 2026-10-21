//! Scenarios `authoring.brief-check.01` … `.14` (0.26 D1–D3, #108): the
//! language a brief is judged in, the sections the prose rules read, and
//! the six rules that read one mechanism each, against the built `canon`
//! binary and the public fixture briefs in `tests/fixtures/briefs/`
//! (invented products; the Korean ones are written in the style the
//! mechanisms were measured on).

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const NEW_RULES: [&str; 6] = ["prose.code-citation", "prose.parenthetical", "prose.decision-history", "prose.ai-vocabulary", "prose.ko-ending", "prose.length"];

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).output().expect("spawn canon binary")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = run(repo, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    out
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/briefs").join(name)).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

/// A `canon init` repo (`language:` set when `lang` is given) with
/// subject `demo` and change `add-login`.
fn repo_with_change(lang: Option<&str>) -> TempDir {
    let dir = TempDir::new().unwrap();
    let mut init = vec!["init", "--providers", "none"];
    if let Some(lang) = lang {
        init.extend(["--language", lang]);
    }
    ok(dir.path(), &init);
    ok(dir.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(dir.path(), &["change", "new", "add-login", "--subject-id", "demo", "--title", "Add login"]);
    dir
}

fn write_brief(repo: &Path, text: &str) {
    std::fs::write(repo.join(".canon/changes/add-login/brief.md"), text).unwrap();
}

fn check(repo: &Path) -> (i32, Value) {
    let out = run(repo, &["brief", "check", "--json"]);
    let json: Value = serde_json::from_str(&stdout(&out)).unwrap_or_else(|e| panic!("not json ({e}): {}", stdout(&out)));
    (out.status.code().unwrap(), json)
}

/// The report of `text` judged in a repo with `lang` configured (or not).
fn judged(lang: Option<&str>, text: &str) -> Value {
    let dir = repo_with_change(lang);
    write_brief(dir.path(), text);
    check(dir.path()).1
}

fn findings(report: &Value) -> Vec<&Value> {
    report["findings"].as_array().unwrap().iter().collect()
}

fn rules(report: &Value) -> Vec<String> {
    findings(report).iter().map(|f| f["rule"].as_str().unwrap().to_string()).collect()
}

fn rules_in(report: &Value, section: &str) -> Vec<String> {
    findings(report).iter().filter(|f| f["section"] == section).map(|f| f["rule"].as_str().unwrap().to_string()).collect()
}

fn hits<'a>(report: &'a Value, rule: &str) -> Vec<&'a Value> {
    findings(report).into_iter().filter(|f| f["rule"] == rule).collect()
}

fn hits_owned(report: &Value, rule: &str) -> Vec<String> {
    hits(report, rule).iter().map(|f| f["section"].as_str().unwrap_or("brief").to_string()).collect()
}

fn new_rules(report: &Value) -> Vec<String> {
    rules(report).into_iter().filter(|r| NEW_RULES.contains(&r.as_str())).collect()
}

/// `text` with `from` replaced by `to`, which must be there.
fn swap(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "`{from}` is not in the brief");
    text.replacen(from, to, 1)
}

/// authoring.brief-check.01: with no `language:`, a brief is judged in
/// the language its text is written in, and the output says which.
#[test]
fn authoring_brief_check_01_a_brief_is_judged_in_the_language_it_is_written_in() {
    let dir = repo_with_change(None);
    assert!(!std::fs::read_to_string(dir.path().join("canon.yaml")).unwrap().contains("\nlanguage:"), "no language: is set");

    write_brief(dir.path(), &fixture("good-ko-lantern.md"));
    let (code, report) = check(dir.path());
    assert_eq!((code, report["language"].as_str(), report["languageSource"].as_str()), (0, Some("ko"), Some("detected")), "{report}");
    assert!(report.get("languageNote").is_none(), "{report}");
    assert_eq!(rules(&report), Vec::<String>::new(), "{report}");
    let human = stdout(&run(dir.path(), &["brief", "check"]));
    assert_eq!(human.lines().filter(|l| l.trim_start().starts_with("language:")).count(), 1, "{human}");
    assert!(human.contains("language: ko (detected)"), "{human}");

    write_brief(dir.path(), &fixture("good-en-cobalt.md"));
    let (_, report) = check(dir.path());
    assert_eq!((report["language"].as_str(), report["languageSource"].as_str()), (Some("en"), Some("detected")), "{report}");
    assert!(stdout(&run(dir.path(), &["brief", "check"])).contains("language: en (detected)"));

    // The same Korean that the `en` rules would have waved through is judged by the `ko` rules.
    write_brief(dir.path(), &fixture("bad-ko-tidepool.md"));
    let (code, report) = check(dir.path());
    assert_eq!((code, report["language"].as_str()), (0, Some("ko")), "advisories never fail the check");
    assert!(!hits(&report, "prose.ko-ending").is_empty() && !hits(&report, "prose.colloquial").is_empty(), "{report}");

    // Bare `canon brief` reads the same resolution.
    assert!(stdout(&run(dir.path(), &["brief"])).contains("polish"));
}

/// authoring.brief-check.02: a configured `language:` decides; when the
/// text reads as another language, one line says so.
#[test]
fn authoring_brief_check_02_a_configured_language_overrides_and_a_disagreeing_text_is_noted() {
    let dir = repo_with_change(Some("en"));
    write_brief(dir.path(), &fixture("bad-ko-tidepool.md"));
    let (code, report) = check(dir.path());
    assert_eq!(code, 0, "{report}");
    assert_eq!((report["language"].as_str(), report["languageSource"].as_str(), report["languageNote"].as_str()), (Some("en"), Some("configured"), Some("the text reads as ko")), "{report}");
    let human = stdout(&run(dir.path(), &["brief", "check"]));
    assert!(human.contains("language: en (configured; the text reads as ko)"), "{human}");
    assert_eq!(human.lines().filter(|l| l.trim_start().starts_with("language:")).count(), 1, "{human}");
    // Judged in `en`: no Korean-only rule fires, the language-free ones still do.
    let found = rules(&report);
    assert!(!found.contains(&"prose.ko-ending".to_string()) && !found.contains(&"prose.colloquial".to_string()), "{found:?}");
    assert!(found.contains(&"prose.code-citation".to_string()), "{found:?}");

    // `ko` configured over an English brief reads as en the other way round; a matching text has no note.
    let ko = repo_with_change(Some("ko"));
    write_brief(ko.path(), &fixture("good-en-cobalt.md"));
    let (_, report) = check(ko.path());
    assert_eq!(report["languageNote"], "the text reads as en", "{report}");
    write_brief(ko.path(), &fixture("good-ko-lantern.md"));
    let (_, report) = check(ko.path());
    assert!(report.get("languageNote").is_none() && report["languageSource"] == "configured", "{report}");
    assert!(stdout(&run(ko.path(), &["brief", "check"])).contains("language: ko (configured)"));
}

/// authoring.brief-check.03: a section whose own script differs from the
/// brief's is judged in its own language.
#[test]
fn authoring_brief_check_03_a_section_in_another_script_is_judged_in_its_own_language() {
    // An English brief whose questions are written in Korean.
    let en = fixture("good-en-cobalt.md");
    let mixed = swap(&en, "- Assumed: invitees outside the organization share no calendar by default.", "- 조직 밖 초대 대상자의 캘린더 공유 기본값을 정해야 합니다.");
    let report = judged(None, &mixed);
    assert_eq!(report["language"], "en", "{report}");
    assert_eq!(rules_in(&report, "questions"), ["prose.ko-ending"], "the Korean section gets the Korean rules: {report}");
    assert!(rules_in(&report, "detail").is_empty() && rules_in(&report, "intent").is_empty(), "{report}");

    // A Korean brief whose acceptance is English: judged by the English lists.
    let ko = fixture("good-ko-lantern.md");
    let mixed = swap(
        &ko,
        "- F1: 대출 중인 책을 넣으면 영수증이 열리고 대출 목록에서 그 책이 사라진다.\n- F2: 태그가 없는 책을 넣으면 오류 안내가 뜨고 대출 목록은 그대로다. ==**실패한 반납은 기록을 남기지 않는다.**==",
        "- F1: The receipt opens. Therefore it is very clear that the book leaves the loan list.\n- F2: A book without a tag shows the error and the loan list stays as it was. ==**A failed return leaves no record.**==",
    );
    let report = judged(None, &mixed);
    assert_eq!(report["language"], "ko", "{report}");
    let in_acceptance = rules_in(&report, "acceptance");
    assert!(in_acceptance.contains(&"prose.connective".to_string()) && in_acceptance.contains(&"prose.degree".to_string()), "{in_acceptance:?}");
    assert!(rules_in(&report, "outcome").is_empty(), "{report}");

    // A configured language is not overridden per section: the same Korean section that drew prose.ko-ending
    // above draws nothing when `en` is configured, and the Korean-only rule runs on it when `ko` is.
    let korean_questions = swap(&en, "- Assumed: invitees outside the organization share no calendar by default.", "- 조직 밖 초대 대상자의 캘린더 공유 기본값을 정해야 합니다.");
    let configured_en = judged(Some("en"), &korean_questions);
    assert_eq!(configured_en["languageSource"], "configured");
    assert_eq!(rules_in(&configured_en, "questions"), Vec::<String>::new(), "{configured_en}");
    let configured_ko = judged(Some("ko"), &korean_questions);
    assert_eq!(rules_in(&configured_ko, "questions"), ["prose.ko-ending"], "{configured_ko}");
}

/// A complete brief (Japanese or Chinese headings and body) with a code
/// citation, a date and asides in it: the mechanisms the rules would read
/// in a language canon ships a list for.
fn cjk_brief(intent: &str, outcome: &str, step: &str, detail: &str, acceptance: &str, none: &str, headings: [&str; 7]) -> String {
    let marked = |key: &str, role: &str, heading: &str, body: &str| format!("<!-- canon:section {key} role={role} -->\n## {heading}\n\n{body}\n\n");
    format!(
        "# add-login\n\n{}{}{}{}{}{}{}",
        marked("intent", "context", headings[0], intent),
        marked("outcome", "normative", headings[1], outcome),
        marked("flow", "normative", headings[2], step),
        marked("detail", "normative", headings[3], &format!("### F1 {}\n\n{detail}", headings[3])),
        marked("acceptance", "normative", headings[4], acceptance),
        marked("questions", "question", headings[5], none),
        marked("out-of-scope", "context", headings[6], none),
    )
}

/// authoring.brief-check.04: Japanese and Chinese text is named, gets
/// the structure rules and no prose rules.
#[test]
fn authoring_brief_check_04_other_cjk_scripts_are_named_and_get_no_prose_rules() {
    let ja = cjk_brief(
        "会議の時間を決めるとき、参加者の予定が見えないため、空いている時間を探すのに時間がかかる。2026-10-01 の会議で決めた方針に沿って、主催者が時間を選ぶあいだ、参加者の予定を表示する。",
        "主催者は予定表を開くと、全員の予定が見えるので、一度の操作で空いている時間を選べる。 ==**予定の詳細は表示されない。**==",
        "1. F1 主催者は予定表を見て → 参加者を追加すると → その人の予定が表示される。 ==**表示は二秒以内に行われる。**==",
        "予定の描画は `src/calendar/blocks.ts:42` が担当する（主催者の時間帯、重なりは結合、詳細は非表示）。 ==**予定の題名は描画しない。**==",
        "- F1: 参加者を追加すると予定が描画される。 ==**題名は描画されない。**==",
        "なし。",
        ["意図", "期待する結果", "ユーザーの流れ", "ステップの詳細", "受け入れ条件", "質問と前提", "対象外"],
    );
    let report = judged(None, &ja);
    assert_eq!((report["language"].as_str(), report["languageSource"].as_str()), (Some("ja"), Some("detected")), "{report}");
    assert_eq!(rules(&report), Vec::<String>::new(), "no prose rule reads ja text: {report}");

    // The structure rules still apply to it.
    let dir = repo_with_change(None);
    write_brief(dir.path(), &swap(&ja, "<!-- canon:section out-of-scope role=context -->\n", ""));
    let (code, report) = check(dir.path());
    assert_eq!(code, 2, "{report}");
    assert_eq!(report["language"], "ja");
    assert!(rules(&report).contains(&"brief.section-missing".to_string()), "{report}");
    assert!(stdout(&run(dir.path(), &["brief", "check"])).contains("language: ja (detected)"));

    let zh = cjk_brief(
        "团队成员在选择会议时间时看不到谁有空，因此很难找到合适的时间。按照 2026-10-01 会议上定下的方案，组织者选择时段时显示每位受邀者的忙碌时段。",
        "组织者打开时段选择器后可以看到所有受邀者的忙碌时段，并一次选定空闲时段。 ==**不显示事件标题。**==",
        "1. F1 组织者看到时段选择器 → 添加受邀者 → 看到对方的忙碌时段。 ==**忙碌时段在两秒内出现。**==",
        "忙碌时段的绘制由 `src/calendar/blocks.ts:42` 负责（按组织者的时区，重叠时合并，不显示标题）。 ==**不绘制事件标题。**==",
        "- F1: 添加受邀者后绘制忙碌时段。 ==**不绘制标题。**==",
        "无。",
        ["意图", "期望结果", "用户流程", "步骤详情", "验收条件", "问题与假设", "不在范围内"],
    );
    let report = judged(None, &zh);
    assert_eq!((report["language"].as_str(), report["languageSource"].as_str()), (Some("zh"), Some("detected")), "{report}");
    assert_eq!(rules(&report), Vec::<String>::new(), "{report}");
}

/// authoring.brief-check.05: Intent and Questions get the phrase rules
/// and the new rules; length, paragraph and key-sentence rules stay on
/// the normative sections; out of scope is not judged.
#[test]
fn authoring_brief_check_05_intent_and_questions_are_judged_with_the_style_rules() {
    let en = fixture("good-en-cobalt.md");
    let long_sentence = "word ".repeat(40);
    let text = swap(&en, "Team members pick meeting times without seeing who is busy.", &format!("Therefore it is important that members see who is busy, {long_sentence}see `PickerState` and src/picker/state.rs. We decided this on 2026-10-01."));
    let text = swap(&text, "- Assumed: invitees outside the organization share no calendar by default.", "- Is this very hard to build, see `slot_picker` in docs/notes.md?");
    let text = swap(&text, "Recurring meetings.", &format!("Therefore it is important to skip recurring meetings, see `rrule_parser` and src/rrule.rs, decided on 2026-10-02. {}", "Recurring meetings are not part of this change. ".repeat(20)));
    let report = judged(None, &text);
    let intent = rules_in(&report, "intent");
    for rule in ["prose.connective", "prose.evaluative", "prose.code-citation", "prose.decision-history"] {
        assert!(intent.contains(&rule.to_string()), "{rule} missing from intent: {intent:?}");
    }
    for never in ["prose.sentence-length", "prose.paragraph-length", "prose.key-sentence"] {
        assert!(!intent.contains(&never.to_string()), "{never} must stay on normative sections: {intent:?}");
    }
    let questions = rules_in(&report, "questions");
    assert!(questions.contains(&"prose.degree".to_string()) && questions.contains(&"prose.code-citation".to_string()), "{questions:?}");
    assert!(!questions.contains(&"prose.decision-history".to_string()), "a question may name when something was decided: {questions:?}");
    assert!(rules_in(&report, "out-of-scope").is_empty(), "out of scope is not judged, not even for length: {report}");
}

/// authoring.brief-check.06: `prose.code-citation` names the path,
/// `path:line` or identifier it matched; a `Source:` line and UI text
/// are not citations.
#[test]
fn authoring_brief_check_06_a_code_citation_is_flagged_and_provenance_goes_on_one_source_line() {
    let en = fixture("good-en-cobalt.md");
    let cited = swap(&en, "- Blocks are drawn in the organizer's time zone.", "- Blocks are drawn in the organizer's time zone (see src/calendar/blocks.ts:42 and `BlockLayout::draw()`).");
    let report = judged(None, &cited);
    let found = hits(&report, "prose.code-citation");
    assert_eq!(found.len(), 1, "{report}");
    assert_eq!(found[0]["section"], "detail");
    let hint = found[0]["hint"].as_str().unwrap();
    assert!(hint.contains("`src/calendar/blocks.ts:42`") && hint.contains("`BlockLayout::draw()`") && hint.contains("`Source:`"), "{hint}");

    // The same file on a `Source:` line, and UI text in the flow, are not citations.
    let sourced = swap(&en, "### F2 No shared calendar", "Source: src/calendar/blocks.ts:42\n\n### F2 No shared calendar");
    let sourced = swap(&sourced, "sees a \"No calendar shared\" label", "sees the `No calendar shared` label and the `Add invitee` button");
    assert!(hits(&judged(None, &sourced), "prose.code-citation").is_empty(), "a Source line inside a judged section is exempt");
    // Without the exemption the same line in the same section is a citation.
    let unsourced = swap(&en, "### F2 No shared calendar", "Provenance: src/calendar/blocks.ts:42\n\n### F2 No shared calendar");
    assert_eq!(hits(&judged(None, &unsourced), "prose.code-citation").len(), 1);
    // A command, a slash between words and product names are not citations.
    let commands = swap(&en, "- The invitee row shows the label and no blocks.", "- Run `canon brief check`, read/write access, src/dst pairs, an `iPhone` user and Next.js pages show the label and no blocks.");
    assert!(hits(&judged(None, &commands), "prose.code-citation").is_empty());
    let app = swap(&en, "- The invitee row shows the label and no blocks.", "- The pick is saved by App.js as usual.");
    assert_eq!(hits(&judged(None, &app), "prose.code-citation").len(), 1, "App.js is a file; Next.js above is a product");
    let ko_slash = swap(&fixture("good-ko-lantern.md"), "- 반납함은 책 태그를 읽고 대출 목록에서 해당 책을 지운다.", "- 문자/메일 알림과 Next.js로 만든 화면에서 반납함은 책 태그를 읽고 대출 목록에서 해당 책을 지운다.");
    assert!(hits(&judged(None, &ko_slash), "prose.code-citation").is_empty());

    // Korean sentences cite the same way.
    let ko = swap(&fixture("good-ko-lantern.md"), "- 반납함은 책 태그를 읽고 대출 목록에서 해당 책을 지운다.", "- 반납함은 책 태그를 읽고 `loan_list`에서 해당 책을 지운다 (`src/kiosk/return.rs:31`).");
    let found = hits(&judged(None, &ko), "prose.code-citation").len();
    assert_eq!(found, 1);
}

/// authoring.brief-check.07: `prose.parenthetical` reports the number of
/// asides, the characters measured and its threshold.
#[test]
fn authoring_brief_check_07_asides_above_the_density_are_flagged_with_count_and_threshold() {
    let ko = fixture("good-ko-lantern.md");
    let dense = swap(&ko, "- 반납함은 책 태그를 읽고 대출 목록에서 해당 책을 지운다.", "- 반납함은 책 태그를 읽고 (근거리 무선) 대출 목록에서 (서버 기준) 해당 책을 지운다 (지연 없이).");
    let report = judged(None, &dense);
    let found = hits(&report, "prose.parenthetical");
    assert_eq!(found.len(), 1, "{report}");
    let hint = found[0]["hint"].as_str().unwrap();
    assert!(hint.starts_with("3 asides in ") && hint.contains("characters; threshold more than 4 per 1000 characters and at least 3"), "{hint}");
    assert_eq!(found[0]["section"], "detail");
    assert_eq!(found[0]["excerpt"], "(근거리 무선)");

    // Two asides, and labels such as (F1) or (30초), do not count.
    let few = swap(&ko, "- 반납함은 책 태그를 읽고 대출 목록에서 해당 책을 지운다.", "- 반납함은 책 태그를 읽고 (근거리 무선) 대출 목록에서 (서버 기준) 해당 책을 지운다 (F1) (30초) (ms).");
    assert!(hits(&judged(None, &few), "prose.parenthetical").is_empty());
    let en = swap(&fixture("good-en-cobalt.md"), "- Blocks are drawn in the organizer's time zone.", "- Blocks are drawn (in the organizer's time zone) for every invitee (as busy), merged (when they overlap).");
    assert_eq!(hits(&judged(None, &en), "prose.parenthetical").len(), 1);
}

/// authoring.brief-check.08: `prose.decision-history` flags dates and
/// decision notes in normative and Intent text, not in a question.
#[test]
fn authoring_brief_check_08_dates_and_decision_notes_are_history_not_behavior() {
    let ko = fixture("good-ko-lantern.md");
    let text = swap(&ko, "무인 반납함은 책을 넣어도", "2026-09-14 회의에서 결정했으며 무인 반납함은 책을 넣어도");
    let text = swap(&text, "- 태그를 읽지 못하면 책을 되돌려 주고 대출 목록은 바꾸지 않는다.", "- 태그를 읽지 못하면 책을 되돌려 주고 대출 목록은 바꾸지 않는다. 이 동작은 10월 21일에 합의했다.");
    let text = swap(&text, "- 영수증 용지가 떨어지면 화면 영수증만 보여 주는 것으로 가정한다.", "- 영수증 용지 문제는 2026-10-03 회의에서 결정했다.");
    let report = judged(None, &text);
    let found = hits(&report, "prose.decision-history");
    let matched: Vec<(&str, String)> = found.iter().map(|f| (f["section"].as_str().unwrap(), f["hint"].as_str().unwrap().to_string())).collect();
    assert_eq!(matched.len(), 2, "intent and detail only, not the question: {report}");
    assert_eq!(matched[0].0, "intent");
    assert!(matched[0].1.contains("`2026-09-14`"), "{matched:?}");
    assert_eq!(matched[1].0, "detail");
    assert!(matched[1].1.contains("`합의했다`"), "{matched:?}");

    // English: a date and a decision note are each history; "agreed" in behavior is not.
    let en = fixture("good-en-cobalt.md");
    let dated = swap(&en, "Show each invitee's busy blocks while the organizer chooses a slot.", "Show each invitee's busy blocks while the organizer chooses a slot, as we decided on Oct 21.");
    let found = hits(&judged(None, &dated), "prose.decision-history").len();
    assert_eq!(found, 1);
    let behavior = swap(&en, "- The organizer's pick is saved as usual.", "- An invitee who agreed on the terms appears with busy blocks. Every invitee in the meeting sees the pick. A slot changed on the server is redrawn. The pick is saved on May 3 releases.");
    assert!(hits(&judged(None, &behavior), "prose.decision-history").is_empty());
    // A date alone is not history: behavior that starts on a date, and the Korean date gate (a date needs a past decision verb).
    let dated_behavior = swap(&en, "- The organizer's pick is saved as usual.", "- Invitees added after Oct 21 see the new picker. Events updated on Nov 3 show a badge.");
    assert!(hits(&judged(None, &dated_behavior), "prose.decision-history").is_empty());
    let ko_dated = swap(&ko, "- 태그를 읽지 못하면 책을 되돌려 주고 대출 목록은 바꾸지 않는다.", "- 2026-12-01부터 연체 시 추가 요금이 붙는다. 2026년 11월 이후 수정된 대출 기록은 다시 보낸다. 10월 21일 이후에 열린다.");
    assert!(hits(&judged(None, &ko_dated), "prose.decision-history").is_empty());
    let ko_decided = swap(&ko, "- 태그를 읽지 못하면 책을 되돌려 주고 대출 목록은 바꾸지 않는다.", "- 2026-12-01 회의에서 연체료를 받지 않으며 확정됐다.");
    assert_eq!(hits(&judged(None, &ko_decided), "prose.decision-history").len(), 1);
    // Any stem with 기로 했다 is a decision.
    let ko_gironeun = swap(&ko, "- 태그를 읽지 못하면 책을 되돌려 주고 대출 목록은 바꾸지 않는다.", "- 대출 목록은 그대로 두기로 했다.");
    assert_eq!(hits(&judged(None, &ko_gironeun), "prose.decision-history").len(), 1);
    // A note with no date is still a note: removing either phrase list fails here.
    let undated = swap(&en, "- The organizer's pick is saved as usual.", "- The pick is saved as usual, because we decided to keep it.");
    assert_eq!(hits(&judged(None, &undated), "prose.decision-history").len(), 1);
    let ko_behavior = swap(&ko, "- 태그를 읽지 못하면 책을 되돌려 주고 대출 목록은 바꾸지 않는다.", "- 이용자가 비밀번호를 변경했으면 다시 로그인한다. 이용자와 사서가 연장에 합의하면 기간이 늘어난다.");
    assert!(hits(&judged(None, &ko_behavior), "prose.decision-history").is_empty());
    let ko_undated = swap(&ko, "- 태그를 읽지 못하면 책을 되돌려 주고 대출 목록은 바꾸지 않는다.", "- 회의에서 대출 목록은 바꾸지 않기로 결정했다.");
    assert_eq!(hits(&judged(None, &ko_undated), "prose.decision-history").len(), 1);
}

/// authoring.brief-check.09: `prose.ai-vocabulary` names the stock word
/// or metaphor from the closed list of the section's language.
#[test]
fn authoring_brief_check_09_stock_ai_vocabulary_is_named_in_each_language() {
    let en = fixture("good-en-cobalt.md");
    let text = swap(&en, "Show each invitee's busy blocks while the organizer chooses a slot.", "Leverage a seamless, robust picker as the north star.");
    let report = judged(None, &text);
    let found = hits(&report, "prose.ai-vocabulary");
    assert_eq!(found.len(), 1, "one finding per sentence: {report}");
    assert_eq!(found[0]["section"], "intent");
    assert!(["leverag", "seamless", "robust", "north star"].iter().any(|w| found[0]["hint"].as_str().unwrap().contains(w)), "{found:?}");
    // The plain words next to them are not on the list.
    let plain = swap(&en, "Show each invitee's busy blocks while the organizer chooses a slot.", "Use a smooth picker that shows every invitee's busy blocks.");
    assert!(hits(&judged(None, &plain), "prose.ai-vocabulary").is_empty());

    let ko = swap(&fixture("good-ko-lantern.md"), "반납 직후 화면과 문자로 영수증을 보여 준다.", "원활하고 견고한 반납 경험이 이번 변경의 북극성이다.");
    let found = hits(&judged(None, &ko), "prose.ai-vocabulary").len();
    assert_eq!(found, 1);
    // The ㅂ-irregular adnominal form is caught too.
    let smooth = swap(&fixture("good-ko-lantern.md"), "반납 직후 화면과 문자로 영수증을 보여 준다.", "매끄러운 화면으로 영수증을 보여 준다.");
    assert_eq!(hits(&judged(None, &smooth), "prose.ai-vocabulary").len(), 1);
}

/// authoring.brief-check.10: `prose.ko-ending` flags a Korean sentence
/// that is not a plain `~다`; `prose.colloquial` keeps `~요`, list fragments
/// and flow steps are exempt.
#[test]
fn authoring_brief_check_10_a_korean_sentence_not_ending_in_da_is_flagged() {
    let ko = fixture("good-ko-lantern.md");
    let text = swap(&ko, "- 문자 수신에 동의한 이용자에게는 같은 내용을 문자로도 보낸다.", "- 문자 수신에 동의한 이용자에게는 같은 내용을 문자로도 보냅니다.\n- 영수증을 확인하세요.\n- 다음 화면으로 넘어갑시다.\n- 책을 다시 넣으라.\n- 영수증 화면.\n- 영수증을 보여 줌");
    let report = judged(None, &text);
    let endings: Vec<String> = hits(&report, "prose.ko-ending").iter().map(|f| f["hint"].as_str().unwrap().to_string()).collect();
    assert_eq!(endings.len(), 3, "{report}");
    assert!(endings[0].contains("`~니다`") && endings[1].contains("`~시다`") && endings[2].contains("`~라`"), "{endings:?}");
    // The polite `~세요` is prose.colloquial's, not reported twice.
    let colloquial = hits(&report, "prose.colloquial");
    assert_eq!(colloquial.len(), 1, "{report}");
    assert!(colloquial[0]["excerpt"].as_str().unwrap().contains("확인하세요"));
    // A noun-phrase list item and a flow step are exempt; a question is not an ending to fix.
    assert!(!endings.iter().any(|e| e.contains("화면")));
    let flow = swap(&ko, "→ 영수증 화면이 열린다.", "→ 영수증 화면이 열립니다.");
    assert!(hits(&judged(None, &flow), "prose.ko-ending").is_empty(), "flow-step clauses are exempt");
    let asked = swap(&ko, "- 영수증 용지가 떨어지면 화면 영수증만 보여 주는 것으로 가정한다.", "- 용지가 떨어지면 화면 영수증만 보여 줄까?\n- 없음.");
    assert!(hits(&judged(None, &asked), "prose.ko-ending").is_empty());
    // Plain forms that end in 니다 (아니다, 지니다, 다닌다) are not polite; 합니다, 입니다, 없습니다 are.
    let plain = swap(&ko, "- 문자 수신에 동의한 이용자에게는 같은 내용을 문자로도 보낸다.", "- 가정: 아니다.\n- 반납함은 태그를 지니다.\n- 이용자는 매일 반납함에 다닌다.");
    assert!(hits(&judged(None, &plain), "prose.ko-ending").is_empty());
    let polite = swap(&ko, "- 문자 수신에 동의한 이용자에게는 같은 내용을 문자로도 보낸다.", "- 영수증은 한 장입니다.\n- 용지가 없습니다.\n- 반납함이 닫힙니다.");
    assert_eq!(hits(&judged(None, &polite), "prose.ko-ending").len(), 3);
    // Noun fragments that end in 자 or 라 are not hortative or imperative, in a list item or a table-free fragment.
    let nouns = swap(&ko, "- 문자 수신에 동의한 이용자에게는 같은 내용을 문자로도 보낸다.", "- 알림 받는 사람: 주최자, 참가자\n- 검토 대상: 후보자\n- 반납 거절 사유: 분실, 파손, 하자\n- 배경색: 보라");
    assert!(hits(&judged(None, &nouns), "prose.ko-ending").is_empty());
    // A terminated imperative or hortative list item is a sentence, whatever the verb.
    let imperatives = swap(&ko, "- 문자 수신에 동의한 이용자에게는 같은 내용을 문자로도 보낸다.", "- 결제 전에 재고를 확인하라.\n- 반납이 끝나면 이용자에게 문자를 보내자.\n- 오류가 나면 사서를 부르십시오.");
    assert_eq!(hits(&judged(None, &imperatives), "prose.ko-ending").len(), 3);
    // English text never gets it.
    let en = swap(&fixture("good-en-cobalt.md"), "Recurring meetings.", "Recurring meetings are planned.");
    assert!(hits(&judged(None, &en), "prose.ko-ending").is_empty());
}

/// authoring.brief-check.11: `prose.length` states the measured
/// characters and the budget, per section and for the whole brief.
#[test]
fn authoring_brief_check_11_a_section_or_brief_over_its_character_budget_is_flagged() {
    let ko = fixture("good-ko-lantern.md");
    let long = "이용자는 반납함 앞에서 책을 넣고 화면에 뜨는 안내를 읽은 뒤 영수증을 받아 간다. ".repeat(8);
    let text = swap(&ko, "무인 반납함은 책을 넣어도", &format!("{long}무인 반납함은 책을 넣어도"));
    let report = judged(None, &text);
    let found = hits(&report, "prose.length");
    assert_eq!(found.len(), 1, "{report}");
    assert_eq!(found[0]["section"], "intent");
    let (hint, excerpt) = (found[0]["hint"].as_str().unwrap(), found[0]["excerpt"].as_str().unwrap());
    assert!(excerpt.ends_with("characters") && hint.contains("budget 400 for `intent`"), "{excerpt} / {hint}");

    // The whole brief against its own budget: the finding has no section.
    let filler: String = (0..80).map(|i| format!("- 항목 {i}: 반납함은 책 태그를 읽고 대출 목록에서 해당 책을 지운 뒤 영수증을 보여 준다.\n")).collect();
    let text = swap(&ko, "### F2 인식 실패\n", &format!("{filler}\n### F2 인식 실패\n"));
    let report = judged(None, &text);
    let total: Vec<&Value> = findings(&report).into_iter().filter(|f| f["rule"] == "prose.length" && f["section"].is_null()).collect();
    assert_eq!(total.len(), 1, "{report}");
    let (hint, excerpt) = (total[0]["hint"].as_str().unwrap(), total[0]["excerpt"].as_str().unwrap());
    assert!(excerpt.ends_with("characters in all sections") && hint.contains("budget 4000 for the whole brief"), "{excerpt} / {hint}");
    // English budgets are larger: the same brief length is fine in en.
    // An English brief whose detail is over the Korean budget (2500) and under the English one (3500).
    let en = fixture("good-en-cobalt.md");
    let filler: String = (0..40).map(|i| format!("- Rule {i}: the invitee row draws one merged block for overlapping busy time.\n")).collect();
    let en_long = swap(&en, "### F2 No shared calendar", &format!("{filler}\n### F2 No shared calendar"));
    assert!(hits(&judged(None, &en_long), "prose.length").is_empty(), "within the English budget");
    let as_ko = hits_owned(&judged(Some("ko"), &en_long), "prose.length");
    assert!(as_ko.contains(&"detail".to_string()), "the same brief is over the Korean budget when ko is configured: {as_ko:?}");
    // Out of scope has no budget of its own: a long one is not flagged for itself.
    let long_scope = swap(&ko, "연체료 정산은 다루지 않는다.", &"연체료 정산은 다루지 않는다. ".repeat(30));
    assert!(rules_in(&judged(None, &long_scope), "out-of-scope").is_empty());
}

/// authoring.brief-check.12: each new rule is waived with the existing
/// waiver comment; the whole-brief length finding with section key `brief`.
#[test]
fn authoring_brief_check_12_a_waiver_clears_each_new_rule_in_its_section() {
    let report = judged(None, &fixture("bad-ko-tidepool.md"));
    let mut text = fixture("bad-ko-tidepool.md");
    let mut waived = Vec::new();
    for f in findings(&report) {
        let rule = f["rule"].as_str().unwrap();
        if !NEW_RULES.contains(&rule) {
            continue;
        }
        let section = f["section"].as_str().unwrap_or("brief");
        let comment = format!("<!-- canon:waive {rule} {section} \"fixture waiver\" -->\n");
        if !text.contains(&comment) {
            text.push_str(&comment);
            waived.push((rule.to_string(), section.to_string()));
        }
    }
    for rule in NEW_RULES {
        assert!(waived.iter().any(|(r, _)| r == rule), "the bad fixture shows {rule}: {waived:?}");
    }
    let report = judged(None, &text);
    assert_eq!(new_rules(&report), Vec::<String>::new(), "{report}");
    assert!(report["waivers"].as_array().unwrap().iter().all(|w| w["applied"] == true && w["severity"] == "advisory"), "{report}");

    // A waiver for another section clears nothing.
    let elsewhere = format!("{}<!-- canon:waive prose.ko-ending intent \"x\" -->\n", fixture("bad-ko-tidepool.md"));
    assert!(!hits(&judged(None, &elsewhere), "prose.ko-ending").is_empty());
}

/// authoring.brief-check.13: the good fixtures stay free of every new
/// rule (and every other), the bad fixtures show each mechanism.
#[test]
fn authoring_brief_check_13_good_briefs_are_clean_and_bad_briefs_show_every_mechanism() {
    for name in ["good-ko-lantern.md", "good-ko-puddle.md", "good-en-cobalt.md", "good-en-meadow.md"] {
        let report = judged(None, &fixture(name));
        assert_eq!(rules(&report), Vec::<String>::new(), "{name}: {report}");
    }
    for name in ["bad-ko-tidepool.md", "bad-ko-ledgerleaf.md", "bad-ko-marigold.md"] {
        let report = judged(None, &fixture(name));
        assert_eq!(report["language"], "ko", "{name}");
        let found = rules(&report);
        for rule in ["prose.code-citation", "prose.parenthetical", "prose.decision-history", "prose.ai-vocabulary"] {
            assert!(found.contains(&rule.to_string()), "{name} lacks {rule}: {found:?}");
        }
        assert!(found.contains(&"prose.ko-ending".to_string()), "{name}");
        assert!(rules_in(&report, "intent").contains(&"prose.parenthetical".to_string()) || rules_in(&report, "intent").contains(&"prose.length".to_string()), "{name}: {found:?}");
    }
    // `prose.length` is shown by the longest bad brief and every one of them is over some budget.
    let long = judged(None, &fixture("bad-ko-tidepool.md"));
    assert!(!hits(&long, "prose.length").is_empty(), "{long}");
    for name in ["bad-ko-ledgerleaf.md", "bad-ko-marigold.md"] {
        assert!(!hits(&judged(None, &fixture(name)), "prose.length").is_empty(), "{name}");
    }
}

/// authoring.brief-check.14: `canon status` and the ship guard judge the
/// brief in the same detected language.
#[test]
fn authoring_brief_check_14_status_counts_the_findings_of_the_detected_language() {
    let dir = repo_with_change(None);
    write_brief(dir.path(), &fixture("bad-ko-tidepool.md"));
    let status = stdout(&ok(dir.path(), &["status"]));
    let (_, report) = check(dir.path());
    let advisories = report["counts"]["advisories"].as_u64().unwrap();
    assert!(advisories > 10, "{report}");
    assert!(status.contains(&format!("brief check: 0 blocker(s), {advisories} advisory(ies)")), "status and brief check agree: {status}");

    write_brief(dir.path(), &fixture("good-ko-lantern.md"));
    let status = stdout(&ok(dir.path(), &["status"]));
    assert!(status.contains("brief check: 0 blocker(s), 0 advisory(ies)") || !status.contains("brief check:"), "{status}");
}

/// authoring.brief-check.15: a markdown table is not read as sentences.
#[test]
fn authoring_brief_check_15_a_markdown_table_is_not_measured_as_a_sentence() {
    let ko = fixture("good-ko-lantern.md");
    let rows: String = (1..=6).map(|i| format!("| 상태 {i} | 반납함은 책 태그를 읽고 대출 목록에서 해당 책을 지운 뒤 영수증을 화면과 문자로 보여 준다 | 이용자에게 보인다 |\n")).collect();
    let table = format!("| 상태 | 동작 | 결과 |\n|---|---|---|\n{rows}\n");
    let text = swap(&ko, "### F2 인식 실패\n", &format!("{table}### F2 인식 실패\n"));
    let report = judged(None, &text);
    assert_eq!(rules(&report), Vec::<String>::new(), "no sentence, paragraph, aside or ending rule reads a table: {report}");

    // The token rules read the cells: a path with a line, an identifier, a dated decision note, stock vocabulary.
    let cells = "| 2026-09-14 회의에서 결정했다 | `src/kiosk/return.rs:31`, `loan_list` | 원활한 경험 |\n";
    let text = swap(&ko, "### F2 인식 실패\n", &format!("| 항목 | 값 | 비고 |\n|---|---|---|\n{cells}\n### F2 인식 실패\n"));
    let report = judged(None, &text);
    let found = rules_in(&report, "detail");
    for rule in ["prose.decision-history", "prose.code-citation", "prose.ai-vocabulary"] {
        assert!(found.contains(&rule.to_string()), "{rule} missing from {found:?}");
    }
    assert!(!found.contains(&"prose.sentence-length".to_string()) && !found.contains(&"prose.parenthetical".to_string()), "{found:?}");

    // `prose.length` still counts the table's characters.
    let big: String = (0..40).map(|i| format!("| 상태 {i} | 반납함은 책 태그를 읽고 대출 목록에서 해당 책을 지운 뒤 영수증을 화면과 문자로 보여 준다 | 이용자에게 보인다 |\n")).collect();
    let text = swap(&ko, "### F2 인식 실패\n", &format!("| 상태 | 동작 | 결과 |\n|---|---|---|\n{big}\n### F2 인식 실패\n"));
    let report = judged(None, &text);
    assert_eq!(hits_owned(&report, "prose.length").first().map(String::as_str), Some("detail"), "{report}");
}
