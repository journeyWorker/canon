//! `canon-0-37-claim-1-5` (scenarios `atip.claim15.*`): canon declares atip/1 1.5 (and 1.6 since) and writes the white space of its domain patterns as the Unicode
//! property `White_Space`. The documents are read in process from the declared model (`canon_cli::spec`); the built binary answers the calls that need a process.
//!
//! The declaration is one value (`json_out::DECLARED_MINOR`). The 59 declarations (17 distinct patterns) of canon 0.36.0 that listed white space by hand are `\p{White_Space}` and `\P{White_Space}` now
//! (Annex V2), and the values they take and refuse are the values of 0.36.0: `atip/white_space_0_36.json` holds those patterns as `canon schema` printed them, and
//! `atip.claim15.04` holds each new pattern to its old one over every scalar value, so no value changed sides.
//!
//! The checks that seeded faults must trip are small pure functions over text or `serde_json::Value` in this file (`hand_listed`, `alias_problems`, `differences`,
//! `declaration_problems`, `form_problems`, `whitespace_problems`): each real input passes them, each seeded fault is found, and no sweep is vacuous.
//! Tests are named by scenario id (`atip_claim15_05_…` proves `atip.claim15.05`).

#[path = "support/hermetic.rs"]
mod hermetic;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::LazyLock;

use atip::constraint::document_problems;
use atip::doc::{OptionDoc, ValueType};
use canon_cli::json_out::DECLARED_MINOR;
use serde_json::{json, Value};
use tempfile::TempDir;

/// The 17 patterns canon 0.36.0 printed with white space listed by hand, and where (`<command> --<option>`, `<command> <operand>`, `tree --<option>`).
const GOLDEN: &str = "crates/canon-cli/tests/atip/white_space_0_36.json";

// ---------------------------------------------------------------------------
// Running the binary.

fn canon(dir: &Path, args: &[&str]) -> Output {
    hermetic::canon().args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-claim15").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).output().expect("canon runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("`{path}` is readable: {e}"))
}

/// A git repo with canon's config, to run the commands that need state in.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    let run = |program: &str, args: &[&str]| {
        let out = Command::new(program).current_dir(path).args(args).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-claim15").output().expect("spawn");
        assert!(out.status.success(), "{program} {args:?}: {}{}", stdout(&out), stderr(&out));
    };
    run("git", &["init", "-q"]);
    run(env!("CARGO_BIN_EXE_canon"), &["init", "--no-harness"]);
    run("git", &["add", "-A"]);
    run("git", &["-c", "user.name=canon", "-c", "user.email=canon@example.invalid", "commit", "-q", "-m", "fixture"]);
    run(env!("CARGO_BIN_EXE_canon"), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    run(env!("CARGO_BIN_EXE_canon"), &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    dir
}

/// One answer of `canon schema …`, read in process: the document the declared model holds, in an envelope with the members the checks read, and the compact line the binary prints as its `data`.
struct Answer {
    value: Value,
    text: String,
}

fn answer_of(document: &atip::Document<'_>) -> Answer {
    let value = json!({ "ok": true, "data": document.to_value(), "meta": { "tool": "canon", "toolVersion": env!("CARGO_PKG_VERSION"), "contract": atip::CONTRACT, "contractVersion": DECLARED_MINOR, "schemaVersion": DECLARED_MINOR, "command": "schema" } });
    Answer { value, text: document.to_line() }
}

/// The tree and every command document, as the declared model builds them: the ones `canon schema` answers, without a process per command.
struct Surface {
    tree: Answer,
    docs: BTreeMap<String, Answer>,
}

static SURFACE: LazyLock<Surface> = LazyLock::new(|| {
    let described = canon_cli::spec::described(&canon_cli::spec::built_root(), None).expect("the documents are built from tables the tests keep whole");
    let tree = answer_of(&described.tree());
    let entries = tree.value["data"]["commands"].as_array().expect("the tree lists commands");
    let names: Vec<String> = entries.iter().filter(|entry| entry["kind"] != "group").map(|entry| entry["command"].as_str().expect("an entry names its command").to_string()).collect();
    let docs = names.into_iter().map(|name| (name.clone(), answer_of(&described.command(&name).expect("a command of the tree has its document")))).collect();
    Surface { tree, docs }
});

/// Every domain pattern the tree and the command documents declare, by where: `tree --<option>`, `<command> --<option>`, `<command> <operand>`. An option a command
/// names (a bare string in its `options`) is the tree's and is not counted again.
fn declared_patterns(surface: &Surface) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let mut take = |owner: &str, dash: &str, argument: &Value| {
        if let Some(pattern) = argument["domain"]["pattern"].as_str() {
            let name = argument["name"].as_str().or_else(|| argument["valueName"].as_str()).expect("an argument has a name");
            let previous = found.insert(format!("{owner} {dash}{name}"), pattern.to_string());
            assert!(previous.is_none(), "`{owner} {dash}{name}` is declared once");
        }
    };
    for option in surface.tree.value["data"]["options"].as_array().expect("the tree has options") {
        take("tree", "--", option);
    }
    for (command, doc) in &surface.docs {
        for option in doc.value["data"]["options"].as_array().into_iter().flatten() {
            take(command, "--", option);
        }
        for operand in doc.value["data"]["operands"].as_array().into_iter().flatten() {
            take(command, "", operand);
        }
    }
    found
}

static DECLARED: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| declared_patterns(&SURFACE));

/// The golden patterns: `(the pattern of 0.36.0, where it was declared)`.
fn golden() -> Vec<(String, Vec<String>)> {
    let file: Value = serde_json::from_str(&read(GOLDEN)).expect("the golden patterns are JSON");
    file["patterns"].as_array().expect("patterns").iter().map(|entry| (entry["pattern"].as_str().expect("a pattern").to_string(), entry["at"].as_array().expect("places").iter().map(|place| place.as_str().expect("a place").to_string()).collect())).collect()
}

/// The name and sentence options that `canon-0-37-option-values` gave one domain. A place of the golden file that holds one has a pattern that deliberately takes other
/// values than in 0.36.0 (the white space around it is the parser's to trim, a control character is refused in every command), so it is not held to the old pattern;
/// `white_space_0_36.json` names the same list under `changedBy037`.
const OPTION_VALUES_OPTIONS: [&str; 6] = ["actor", "approver", "relayed-via", "reason", "summary", "override-reason"];

/// The time options that `canon-0-37-option-forms` narrowed to RFC 3339. A place of the golden file that holds one has a
/// pattern that deliberately refuses what 0.36.0 took (white space, a one-digit month, a sign on the year, a date alone for `--approval-at`),
/// and says it without the white-space property, so it is not held to the old pattern (`atip.optionforms.*` hold it; `white_space_0_36.json` names the list under
/// `changedBy037Forms`).
const OPTION_FORMS_OPTIONS: [&str; 6] = ["approval-at", "expiry", "as-of", "before", "after", "since"];

/// The option `canon-0-38-provider-option` removed: `--providers` is declared nowhere in 0.38, so its places of the golden file (`changedBy038`) have no pattern to hold.
const REMOVED_OPTIONS: [&str; 1] = ["providers"];

/// How many of the 59 places of the golden file hold an option of each change, and how many distinct patterns of the unchanged places are compared with their new spelling.
const VALUES_PLACES: usize = 17;
const FORMS_PLACES: usize = 11;
const REMOVED_PLACES: usize = 2;
const CHANGED_PLACES: usize = VALUES_PLACES + FORMS_PLACES + REMOVED_PLACES;
const DISTINCT_COMPARED: usize = 10;

fn changed_by_option_values(place: &str) -> bool {
    place.rsplit_once(" --").is_some_and(|(_, option)| OPTION_VALUES_OPTIONS.contains(&option))
}

fn changed_by_option_forms(place: &str) -> bool {
    place.rsplit_once(" --").is_some_and(|(_, option)| OPTION_FORMS_OPTIONS.contains(&option))
}

fn removed_by_038(place: &str) -> bool {
    place.rsplit_once(" --").is_some_and(|(_, option)| REMOVED_OPTIONS.contains(&option))
}

/// A place of the golden file that a 0.37 change re-declared on purpose, or the 0.38 change removed.
fn changed_by_037(place: &str) -> bool {
    changed_by_option_values(place) || changed_by_option_forms(place) || removed_by_038(place)
}

/// The places that had no pattern with white space in 0.36.0 and have the text pattern or the regime key now.
fn new_in_option_values(place: &str) -> bool {
    changed_by_option_values(place) || place == "dispatch begin --regime" || new_in_change_drop(place)
}

/// The places of the three closing leaves of 0.42 (`canon-0-42-change-drop`) that take `--ref`, the one evidence reference domain (not blank), and the tree's union of the same option.
fn new_in_change_drop(place: &str) -> bool {
    matches!(place, "change defer --ref" | "change drop --ref" | "change reopen --ref" | "tree --ref")
}

fn assert_found(what: &str, problems: &[String], needle: &str) {
    assert!(problems.iter().any(|problem| problem.contains(needle)), "{what}: no problem names `{needle}`: {problems:#?}");
}

// ---------------------------------------------------------------------------
// How a pattern writes white space.

/// The atoms of a pattern body in order: a character, an escape (`\t`, `\n`) or a property (`\p{Zl}`), each with whether it stands inside a character class.
fn atoms(pattern: &str) -> Vec<(String, bool)> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut found = Vec::new();
    let mut in_class = false;
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        match c {
            '\\' if index + 1 < chars.len() => {
                let next = chars[index + 1];
                let end = if matches!(next, 'p' | 'P') && chars.get(index + 2) == Some(&'{') { chars[index..].iter().position(|c| *c == '}').map_or(index + 2, |close| index + close) } else { index + 1 };
                found.push((chars[index..=end].iter().collect(), in_class));
                index = end + 1;
            }
            '[' if !in_class => {
                in_class = true;
                index += 1;
                if chars.get(index) == Some(&'^') {
                    index += 1;
                }
            }
            ']' if in_class => {
                in_class = false;
                index += 1;
            }
            _ => {
                found.push((c.to_string(), in_class));
                index += 1;
            }
        }
    }
    found
}

/// Which white-space member an atom is, as one name: a raw character of the set, `\t`, `\n`, `\r`, `\p{Zl}`, `\p{Zp}` or `\p{Zs}`.
fn member(atom: &str) -> Option<String> {
    match atom {
        "\\t" | "\\n" | "\\r" | "\\p{Zl}" | "\\p{Zp}" | "\\p{Zs}" => Some(atom.to_string()),
        _ => {
            let mut chars = atom.chars();
            let c = chars.next()?;
            let in_set = matches!(c, '\u{9}'..='\u{d}' | ' ' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}');
            (chars.next().is_none() && in_set).then(|| format!("U+{:04X}", u32::from(c)))
        }
    }
}

/// The classes of a pattern that list white space by hand: three or more members of the set in one class (a class of `[^\n\r]` or `[Tt ]` is a line or a letter, and the
/// class of Annex V2 `[\P{White_Space}\p{Zs}\t]` names two), outside a class too.
fn hand_listed(pattern: &str) -> Vec<String> {
    let mut classes: Vec<BTreeSet<String>> = Vec::new();
    let mut inside: Vec<String> = Vec::new();
    let mut outside: BTreeSet<String> = BTreeSet::new();
    let mut was_in_class = false;
    for (atom, in_class) in atoms(pattern) {
        if was_in_class && !in_class {
            classes.push(inside.drain(..).collect());
        }
        was_in_class = in_class;
        if let Some(found) = member(&atom) {
            if in_class {
                inside.push(found);
            } else {
                outside.insert(found);
            }
        }
    }
    if was_in_class {
        classes.push(inside.drain(..).collect());
    }
    classes.push(outside);
    classes.into_iter().filter(|class| class.len() >= 3).map(|class| format!("a class lists {} white-space members by hand: {}", class.len(), class.into_iter().collect::<Vec<_>>().join(" "))).collect()
}

/// `\s`, `\S` and any other shorthand, and a property of a name that is not one the profile names (`WSpace`, `space`, `Z`): none is in a 1.5 pattern.
fn alias_problems(pattern: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (atom, _) in atoms(pattern) {
        if matches!(atom.as_str(), "\\s" | "\\S" | "\\w" | "\\W" | "\\d" | "\\D") {
            found.push(format!("the shorthand {atom}"));
        }
        if let Some(name) = atom.strip_prefix("\\p{").or_else(|| atom.strip_prefix("\\P{")) {
            let name = name.trim_end_matches('}');
            if !["Cc", "Zl", "Zp", "Zs", "White_Space"].contains(&name) {
                found.push(format!("the property {atom}"));
            }
        }
    }
    found
}

// ---------------------------------------------------------------------------
// Values: the corpus a pattern is held to, and how two patterns differ over it.

/// The scalar values worth writing a pattern against: every one below U+3100 (the white space, the controls, the separators and their neighbours all lie below it) and
/// a sample of the rest, with the ones that are famous for being almost white space.
fn scalars() -> Vec<char> {
    let mut found: Vec<char> = (0..0x3100_u32).filter_map(char::from_u32).collect();
    found.extend((0x3100..=0x10_FFFF_u32).step_by(991).filter_map(char::from_u32));
    found.extend(['\u{feff}', '\u{180e}', '\u{200b}', '\u{2060}', '\u{e000}', '\u{10000}', '\u{10ffff}']);
    found
}

/// Strings built round one character: alone, and between and beside letters.
fn shapes(c: char) -> [String; 4] {
    [c.to_string(), format!("a{c}"), format!("{c}a"), format!("a{c}a")]
}

/// The bases the patterns read: a word, a time, a list of providers, a path of a regime, an id, a language, a token and a quote.
const BASES: [&str; 17] = ["", "a", "ab", "none", "claude, codex", "claude,,pi", "ko", "en-US", "2030-01-01", "2030-01-01T10:00:00Z", "a/b/c/abcdef", "01ARZ3NDEKTSV4RRFFQ69G5FAV", "abcdef0", "x_1_2", "a'b", "(a)", "a=b"];

/// The characters the near-miss strings carry: the controls and the white space and their neighbours.
fn special() -> Vec<char> {
    let mut found: Vec<char> = (0..0x100_u32).filter_map(char::from_u32).collect();
    for point in [0x167f_u32, 0x1680, 0x1681, 0x180e, 0x1fff, 0x2000, 0x2005, 0x200a, 0x200b, 0x2027, 0x2028, 0x2029, 0x202a, 0x202e, 0x202f, 0x2030, 0x205e, 0x205f, 0x2060, 0x2fff, 0x3000, 0x3001, 0xfeff] {
        found.extend(char::from_u32(point));
    }
    found
}

/// Every string the corpus holds, built from the characters of `special` set into every place of the bases, and from every scalar in its four shapes.
fn corpus() -> Vec<String> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    for c in scalars() {
        found.extend(shapes(c));
    }
    for c in special() {
        for base in BASES {
            let chars: Vec<char> = base.chars().collect();
            for at in 0..=chars.len() {
                let mut text: String = chars[..at].iter().collect();
                text.push(c);
                text.extend(&chars[at..]);
                found.insert(text);
            }
            found.insert(format!("{c}{base}{c}"));
            found.insert(format!(" {c}{base} "));
        }
    }
    found.into_iter().collect()
}

static CORPUS: LazyLock<Vec<String>> = LazyLock::new(corpus);

/// The values over `strings` that `before` and `after` decide differently, as `(value, before, after)`.
fn differences(before: &str, after: &str, strings: &[String]) -> Vec<(String, bool, bool)> {
    let compile = |pattern: &str| atip::domain::compile(&json!({ "pattern": pattern })).unwrap_or_else(|e| panic!("`{pattern}` compiles: {e}"));
    let (old, new) = (compile(before), compile(after));
    strings
        .iter()
        .filter_map(|text| {
            let value = json!(text);
            let (was, is) = (old.admits(&value), new.admits(&value));
            (was != is).then(|| (text.clone(), was, is))
        })
        .collect()
}

fn declared() -> atip::Minor {
    atip::Minor::parse(canon_cli::json_out::DECLARED_MINOR).expect("the declared minor is one the crate knows")
}

fn minor(text: &str) -> atip::Minor {
    atip::Minor::parse(text).expect("a minor the crate knows")
}

// ===========================================================================
// atip.claim15.02 (F1)

/// Every problem of what the binary says it declares: the minor of an envelope, the minor of a `schema` answer and the level of the tree.
fn declaration_problems(envelope: &Value, is_schema_answer: bool) -> Vec<String> {
    let mut found = Vec::new();
    if envelope["meta"]["contractVersion"] != DECLARED_MINOR {
        found.push(format!("meta.contractVersion is {}, not {DECLARED_MINOR}", envelope["meta"]["contractVersion"]));
    }
    if is_schema_answer && envelope["meta"]["schemaVersion"] != DECLARED_MINOR {
        found.push(format!("meta.schemaVersion is {}, not {DECLARED_MINOR}", envelope["meta"]["schemaVersion"]));
    }
    found
}

#[test]
fn atip_claim15_02_a_declaration_of_1_4_fails_the_check_and_the_crate_judges_a_document_by_the_declared_minor() {
    // The check fails for a constant, a fixture or an answer that says 1.4.
    let answer = json!({ "ok": true, "data": {}, "meta": { "contractVersion": "1.4", "schemaVersion": "1.4" } });
    let problems = declaration_problems(&answer, true);
    assert_found("a 1.4 envelope", &problems, "meta.contractVersion");
    assert_found("a 1.4 schema answer", &problems, "meta.schemaVersion");
    assert_found("a 1.5 envelope", &declaration_problems(&json!({ "meta": { "contractVersion": "1.5" } }), false), "meta.contractVersion");
    assert_eq!(declaration_problems(&json!({ "meta": { "contractVersion": DECLARED_MINOR } }), false), Vec::<String>::new());
    assert_found("a missing declaration", &declaration_problems(&json!({ "meta": {} }), false), "meta.contractVersion");

    // The crate's checks take the minor canon declares: a property pattern is a problem of a 1.4 document and none of a 1.5 one.
    let pattern = "^\\P{White_Space}([\\P{White_Space}\\p{Zs}\\t]*\\P{White_Space})?$";
    assert_eq!(atip::domain::pattern_problem(pattern, declared()), None, "a 1.5 declaration may use the property");
    let at_1_4 = atip::domain::pattern_problem(pattern, minor("1.4")).expect("a 1.4 declaration may not");
    assert!(at_1_4.contains("1.5") && at_1_4.contains("White_Space"), "{at_1_4}");
    let option = |pattern: &str| OptionDoc {
        name: "n".to_string(),
        short: None,
        value_name: Some("N".to_string()),
        value_type: ValueType::String,
        possible_values: None,
        required: false,
        repeatable: false,
        default: Value::Null,
        default_note: None,
        env: None,
        flag_only: false,
        comma_list: false,
        domain: Some(json!({ "pattern": pattern })),
        help: "h".to_string(),
    };
    let check = |minor: atip::Minor, pattern: &str| document_problems(&[option(pattern)], &[], &[], &[], &["USAGE".to_string()], &[], minor);
    assert!(check(declared(), pattern).is_empty(), "a document that declares 1.5 may use the property");
    assert_found("a document that declares 1.4", &check(minor("1.4"), pattern), "declares contract 1.4");
}

// ===========================================================================
// atip.claim15.03 - 05 (F2)

/// Every pattern the binary declares carries the white space as the property, and none lists it.
#[test]
fn atip_claim15_03_no_declared_pattern_lists_white_space_and_each_says_it_as_the_property() {
    let declared = &*DECLARED;
    assert!(!declared.is_empty(), "the sweep covered the tree and every command document");
    let mut problems = Vec::new();
    for (place, pattern) in declared {
        for problem in hand_listed(pattern).into_iter().chain(alias_problems(pattern)) {
            problems.push(format!("{place}: {problem}: {pattern}"));
        }
        if let Some(problem) = atip::domain::pattern_problem(pattern, declared_minor()) {
            problems.push(format!("{place}: {problem}: {pattern}"));
        }
        if pattern.contains("\\p{Zl}") && pattern.contains("\\p{Zp}") && !pattern.contains("White_Space") {
            problems.push(format!("{place}: lists the line and paragraph separators and not the property: {pattern}"));
        }
    }
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));

    // The places 0.36.0 listed white space are the places that say the property now, and no place gained or lost one; the places of the six name and sentence options
    // (`canon-0-37-option-values`) say the one text pattern, and they and the regime key of `dispatch begin` are the only places that gained one.
    let golden = golden();
    let before: BTreeSet<&String> = golden.iter().flat_map(|(_, places)| places).filter(|place| !changed_by_037(place)).collect();
    let after: BTreeSet<&String> = declared.iter().filter(|(place, pattern)| pattern.contains("White_Space") && !new_in_option_values(place)).map(|(place, _)| place).collect();
    assert_eq!(before, after, "the declarations that name white space are the ones that did");
    assert_eq!((golden.iter().map(|(_, places)| places.len()).sum::<usize>(), golden.len()), (59, 17), "59 declarations, 17 distinct patterns of 0.36.0");
    assert_eq!(golden.iter().flat_map(|(_, places)| places).filter(|place| changed_by_option_values(place)).count(), VALUES_PLACES);
    assert_eq!(golden.iter().flat_map(|(_, places)| places).filter(|place| changed_by_option_forms(place)).count(), FORMS_PLACES);
    assert_eq!(golden.iter().flat_map(|(_, places)| places).filter(|place| removed_by_038(place)).count(), REMOVED_PLACES);
    assert!(!declared.keys().any(|place| removed_by_038(place)), "`--providers` is declared nowhere in 0.38");
    for (place, pattern) in declared.iter().filter(|(place, _)| changed_by_option_values(place)) {
        assert!(pattern.contains("[^\\p{Cc}\\p{Zl}\\p{Zp}]"), "`{place}` declares the one text pattern of 0.37: {pattern}");
    }
    for (place, pattern) in declared.iter().filter(|(place, _)| changed_by_option_forms(place)) {
        let want = if place.ends_with("--approval-at") {
            canon_cli::arg_rules::DATE_TIME
        } else {
            canon_cli::arg_rules::TIMESTAMP
        };
        assert_eq!(pattern, want, "`{place}` declares the RFC 3339 pattern of 0.37");
    }
    let distinct: BTreeSet<&String> = declared.iter().filter(|(_, pattern)| pattern.contains("White_Space")).map(|(_, pattern)| pattern).collect();
    assert!(!distinct.is_empty(), "the distinct patterns now: {distinct:#?}");
    // The one-line pattern of Annex V2 is the pattern of the places that want a trimmed line.
    let session = &declared["finding add --session"];
    assert_eq!(session, "^[^\\p{Cc}\\p{White_Space}]([^\\p{Cc}]*[^\\p{Cc}\\p{White_Space}])?$");
    assert!(declared.values().any(|pattern| pattern == "^\\P{White_Space}([\\P{White_Space}\\p{Zs}\\t]*\\P{White_Space})?$"), "the trimmed one line is the form of Annex V2");
}

fn declared_minor() -> atip::Minor {
    declared()
}

/// Every new pattern takes and refuses the values the old one did, over every scalar value and the near misses of the bases.
#[test]
fn atip_claim15_04_each_pattern_takes_and_refuses_the_values_it_did_in_0_36() {
    let declared = &*DECLARED;
    let golden = golden();
    let strings = &*CORPUS;
    assert!(!strings.is_empty(), "the corpus is not vacuous");
    let mut problems = Vec::new();
    let mut compared = BTreeSet::new();
    let mut places = 0;
    for (old, at) in &golden {
        for place in at.iter().filter(|place| !changed_by_037(place)) {
            let new = declared.get(place).unwrap_or_else(|| panic!("`{place}` is declared"));
            places += 1;
            if !compared.insert((old.clone(), new.clone())) {
                continue;
            }
            let different = differences(old, new, strings);
            if !different.is_empty() {
                problems.push(format!("{place}: {} values change sides, the first five: {:?}\n  before {old}\n  after  {new}", different.len(), different.iter().take(5).collect::<Vec<_>>()));
            }
        }
    }
    assert_eq!(places, 59 - CHANGED_PLACES, "the places that kept their values were compared");
    assert_eq!(compared.len(), DISTINCT_COMPARED, "each pattern of a place that kept its values was compared with its new spelling");
    assert!(problems.is_empty(), "{}", problems.join("\n"));

    // The corpus reaches both sides of every pattern: each takes some and refuses some.
    for (old, _) in &golden {
        let compiled = atip::domain::compile(&json!({ "pattern": old })).expect("compiles");
        let taken = strings.iter().filter(|text| compiled.admits(&json!(text))).count();
        assert!(taken > 0 && taken < strings.len(), "`{old}` takes {taken} of {} values", strings.len());
    }
}

/// The property says what the standard library says white space is, over every scalar value.
#[test]
fn atip_claim15_04_the_property_is_the_white_space_of_the_old_lists_over_every_scalar_value() {
    // The new spelling of the old one-word token, which `canon-0-37-option-values` moved from the domain of `--relayed-via` to the validator beside it.
    let token = "^[^\\p{White_Space}()\"']+$";
    let compiled = atip::domain::compile(&json!({ "pattern": token })).expect("compiles");
    let old = golden().into_iter().find(|(_, at)| at.iter().any(|place| place == "brief approve --relayed-via")).expect("the golden has it").0;
    let hand = atip::domain::compile(&json!({ "pattern": old })).expect("compiles");
    let mut count = 0_u32;
    for c in (0..=0x10_FFFF_u32).filter_map(char::from_u32) {
        let text = c.to_string();
        let expected = !c.is_whitespace() && !"()\"'".contains(c);
        let value = json!(text);
        assert_eq!(compiled.admits(&value), expected, "U+{:04X}: the property pattern", u32::from(c));
        assert_eq!(hand.admits(&value), expected, "U+{:04X}: the pattern of 0.36.0", u32::from(c));
        count += 1;
    }
    assert!(count > 0, "every scalar value was read ({count})");
    // The set is the 25 code points of Unicode's White_Space: nothing else is white space.
    assert_eq!((0..=0x10_FFFF_u32).filter_map(char::from_u32).filter(|c| c.is_whitespace()).count(), 25);
}

#[test]
fn atip_claim15_05_the_checks_find_a_listed_class_a_shorthand_an_alias_and_a_value_that_changed_sides() {
    // A list of white space by hand is found, in every pattern of 0.36.0.
    for (old, _) in golden() {
        assert_found(&format!("the 0.36.0 pattern {old}"), &hand_listed(&old), "by hand");
    }
    assert_found("a class of the separators", &hand_listed("^[^\\p{Cc}\\n\\r\\p{Zl}\\p{Zp}]$"), "by hand");
    assert_found("a list outside a class", &hand_listed("^\\t\\n\\r\\p{Zl}$"), "by hand");
    // The spellings of the profile are not.
    for fine in ["^\\P{White_Space}([\\P{White_Space}\\p{Zs}\\t]*\\P{White_Space})?$", "^[^\\p{Cc}\\p{Zl}\\p{Zp}]$", "^[^\\n\\r]*$", "^[0-9]{4}-[0-9]{2}[Tt ][0-9]{2}$", "^[^/\\p{White_Space}]+$"] {
        assert_eq!(hand_listed(fine), Vec::<String>::new(), "{fine}");
        assert_eq!(alias_problems(fine), Vec::<String>::new(), "{fine}");
    }
    // `\s`, `\S` and the aliases are found by this file and refused by the crate, in a 1.5 document too.
    for bad in ["^\\s+$", "^\\S+$", "^[\\s]$", "^\\p{WSpace}$", "^\\p{space}$", "^[^\\p{white_space}]$"] {
        assert!(!alias_problems(bad).is_empty(), "{bad} is found");
        assert!(atip::domain::pattern_problem(bad, declared()).is_some(), "the crate refuses {bad} at {DECLARED_MINOR}");
    }
    // `\p{Z}` is I-Regexp (a general category) and no white-space spelling canon uses: this file finds it, the crate does not.
    assert!(!alias_problems("^\\p{Z}$").is_empty());
    // A value that changes sides is found: a class that lost one member, and a pattern that gained one.
    let old = golden().into_iter().find(|(_, at)| at.iter().any(|place| place == "brief approve --relayed-via")).expect("golden").0;
    let new = "^[^\\p{White_Space}()\"']+$";
    assert!(differences(&old, new, &CORPUS).is_empty());
    let lost = old.replace('\u{a0}', "");
    let found = differences(&lost, new, &CORPUS);
    assert!(found.iter().any(|(text, was, is)| text == "\u{a0}" && *was && !*is), "the no-break space changed sides: {:?}", found.iter().take(3).collect::<Vec<_>>());
    let gained = "^[^\\p{White_Space}\\p{Cc}()\"']+$";
    let found = differences(gained, new, &CORPUS);
    assert!(found.iter().any(|(text, was, is)| text == "\u{7}" && !*was && *is), "the bell changed sides: {:?}", found.iter().take(3).collect::<Vec<_>>());
    let narrowed = new.replace("\\p{White_Space}", "\\p{Zs}");
    assert!(!differences(&old, &narrowed, &CORPUS).is_empty(), "the separators alone are not the white space");
}

// ===========================================================================
// atip.claim15.06 - 07 (F3)

/// Every whitespace (space, tab, CR, LF) outside a string of `text`, which is one JSON document without its line end.
fn whitespace_problems(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for (at, c) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            ' ' | '\t' | '\r' | '\n' => found.push(format!("{c:?} outside a string at byte {at}")),
            _ => {}
        }
    }
    found
}

/// The members a command document states only when they differ from the default (contract 9.5), and the members it no longer has since 1.4.
fn form_problems(doc: &Value) -> Vec<String> {
    let mut found = Vec::new();
    for gone in ["path", "schemas"] {
        if doc.get(gone).is_some() {
            found.push(format!("has `{gone}`"));
        }
    }
    if doc["input"].get("schemaRef").is_some() || doc["input"].get("option").is_some() {
        found.push("has input.schemaRef or input.option".to_string());
    }
    for option in doc["options"].as_array().into_iter().flatten().filter(|option| option.is_object()) {
        for (member, default) in [("required", json!(false)), ("repeatable", json!(false)), ("flagOnly", json!(false)), ("valueType", json!("string"))] {
            if option.get(member) == Some(&default) {
                found.push(format!("option `{}` states `{member}` at its default", option["name"]));
            }
        }
    }
    found
}

#[test]
fn atip_claim15_06_the_rules_of_1_4_hold_at_1_5_compact_documents_usage_for_help_json_and_no_default_members() {
    let surface = &*SURFACE;
    let mut texts: Vec<(String, &str)> = vec![("schema --json".to_string(), surface.tree.text.as_str())];
    texts.extend(surface.docs.iter().map(|(name, got)| (format!("schema {name} --json"), got.text.as_str())));
    let mut found = Vec::new();
    for (what, text) in &texts {
        found.extend(whitespace_problems(text).into_iter().map(|problem| format!("{what}: {problem}")));
    }
    assert!(found.is_empty(), "whitespace outside strings: {found:#?}");
    let forms: Vec<String> = surface.docs.iter().flat_map(|(name, got)| form_problems(&got.value["data"]).into_iter().map(move |problem| format!("{name}: {problem}"))).collect();
    assert!(forms.is_empty(), "{forms:#?}");
    // `--help --json` is refused with USAGE.
    let dir = repo();
    for args in [vec!["--help", "--json"], vec!["finding", "close", "--help", "--json"], vec!["--version", "--json"]] {
        let output = canon(dir.path(), &args);
        assert_eq!(output.status.code(), Some(2), "canon {args:?}: {}", stdout(&output));
        let document: Value = serde_json::from_str(&stdout(&output)).expect("one document");
        assert_eq!(document["error"]["code"], "USAGE", "canon {args:?}");
        assert_eq!(document["meta"]["contractVersion"], DECLARED_MINOR);
    }
    // The six destructive commands list `commit` and `plan-digest`, and a bare call is a preview.
    for name in ["artifacts move", "artifacts prune", "ledger compact", "migrate", "purge", "tier age"] {
        let options: Vec<&str> = surface.docs[name].value["data"]["options"].as_array().expect("options").iter().map(|option| option.as_str().or_else(|| option["name"].as_str()).expect("a name")).collect();
        assert!(options.contains(&"commit") && options.contains(&"plan-digest"), "{name}: {options:?}");
    }
}

#[test]
fn atip_claim15_07_a_formatted_document_and_a_member_at_its_default_fail_the_checks() {
    let value = json!({ "ok": true, "data": { "items": [1, 2] } });
    assert_found("a pretty-printed document", &whitespace_problems(&serde_json::to_string_pretty(&value).unwrap()), "outside a string");
    assert_found("a space after a colon", &whitespace_problems(r#"{"a": 1}"#), "' '");
    assert_eq!(whitespace_problems(&serde_json::to_string(&value).unwrap()), Vec::<String>::new());
    assert_eq!(whitespace_problems(r#"{"a":"x y\tz","b":"q\" w"}"#), Vec::<String>::new());
    let doc = json!({ "command": "x", "options": [{ "name": "n", "required": false }, { "name": "m", "valueType": "string" }, { "name": "k", "flagOnly": false }, { "name": "j", "repeatable": false }], "path": ["x"], "schemas": {}, "input": { "schemaRef": "I" } });
    let problems = form_problems(&doc);
    for needle in ["`path`", "`schemas`", "input.schemaRef", "`required` at its default", "`valueType` at its default", "`flagOnly` at its default", "`repeatable` at its default"] {
        assert_found("a document of the 1.3 form", &problems, needle);
    }
    assert_eq!(form_problems(&json!({ "command": "x", "options": [{ "name": "n", "required": true }, "repo"] })), Vec::<String>::new());
}

// ===========================================================================
// atip.claim15.08 - 09 (F4)

/// A caller that checks patterns by the profile of `minor` reads each distinct property pattern of canon as a problem when the minor is 1.4 and as none when it is 1.5.
#[test]
fn atip_claim15_08_a_1_4_profile_reads_the_property_as_a_problem_and_a_compile_only_caller_takes_the_same_values() {
    let declared = &*DECLARED;
    let distinct: BTreeSet<&String> = declared.values().filter(|pattern| pattern.contains("White_Space")).collect();
    assert!(!distinct.is_empty());
    let golden = golden();
    let sample: Vec<String> = CORPUS.iter().step_by(4).cloned().collect();
    let mut by_new: BTreeMap<&String, &String> = BTreeMap::new();
    for (old, at) in &golden {
        for place in at.iter().filter(|place| !changed_by_037(place)) {
            by_new.insert(&declared[place], old);
        }
    }
    for pattern in &distinct {
        // The profile of 1.4 refuses the escape; the profile of 1.5 reads the pattern.
        let problem = atip::domain::pattern_problem(pattern, minor("1.4")).unwrap_or_else(|| panic!("a 1.4 profile reads {pattern} as a problem"));
        assert!(problem.contains("White_Space") && problem.contains("1.5"), "{problem}");
        assert_eq!(atip::domain::pattern_problem(pattern, minor("1.5")), None, "{pattern}");
        // A caller that only compiles it (as ECMAScript with `u` or Rust `regex` do) gets a pattern that takes the values 0.36.0 took.
        assert!(atip::domain::compile(&json!({ "pattern": pattern })).is_ok(), "{pattern} compiles");
        if let Some(old) = by_new.get(pattern) {
            assert!(differences(old, pattern, &sample).is_empty(), "{pattern}");
        }
    }
    // The profile of 1.4 still accepts what it did: every pattern of 0.36.0.
    for (old, _) in &golden {
        assert_eq!(atip::domain::pattern_problem(old, minor("1.4")), None, "{old}");
    }
    // The patterns without white space are read the same at both minors.
    let plain: Vec<&String> = declared.values().filter(|pattern| !pattern.contains("White_Space")).collect();
    assert!(!plain.is_empty());
    for pattern in plain {
        assert_eq!(atip::domain::pattern_problem(pattern, minor("1.4")), atip::domain::pattern_problem(pattern, minor("1.5")), "{pattern}");
    }
}

#[test]
fn atip_claim15_09_a_document_that_declares_1_4_has_its_property_pattern_read_as_a_problem() {
    let (place, pattern) = DECLARED.iter().find(|(_, pattern)| pattern.contains("White_Space")).expect("a property pattern");
    let option = OptionDoc {
        name: "n".to_string(),
        short: None,
        value_name: Some("N".to_string()),
        value_type: ValueType::String,
        possible_values: None,
        required: false,
        repeatable: false,
        default: Value::Null,
        default_note: None,
        env: None,
        flag_only: false,
        comma_list: false,
        domain: Some(json!({ "pattern": pattern })),
        help: "h".to_string(),
    };
    let found = |minor: atip::Minor| document_problems(std::slice::from_ref(&option), &[], &[], &[], &["USAGE".to_string()], &[], minor);
    assert_found(&format!("`{place}` in a document of 1.4"), &found(minor("1.4")), "White_Space");
    assert!(found(minor("1.5")).is_empty(), "`{place}` in a document of 1.5: {:?}", found(minor("1.5")));
    // `\s` is a problem at every minor, and the 1.4 minor does not make the property a part of its profile.
    let shorthand = OptionDoc { domain: Some(json!({ "pattern": "^\\s+$" })), ..option };
    assert_found("`\\s` in 1.5", &found_with(&shorthand, "1.5"), "not I-Regexp");
    assert_found("`\\s` in 1.4", &found_with(&shorthand, "1.4"), "not I-Regexp");
}

fn found_with(option: &OptionDoc, at: &str) -> Vec<String> {
    document_problems(std::slice::from_ref(option), &[], &[], &[], &["USAGE".to_string()], &[], minor(at))
}

// ===========================================================================
// atip.claim15.10 (F5)

#[test]
fn atip_claim15_10_the_fixture_and_the_tree_say_the_minor_canon_declares() {
    let fixture: Value = serde_json::from_str(&read("crates/canon-cli/tests/atip/canon.fixture.json")).expect("the fixture is JSON");
    assert_eq!(fixture["contractVersion"], canon_cli::json_out::DECLARED_MINOR, "the fixture claims the declared minor");
    assert_eq!(SURFACE.tree.value["meta"]["contractVersion"], canon_cli::json_out::DECLARED_MINOR, "the tree declares it");
}
