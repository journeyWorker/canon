//! `canon-0-37-claim-1-5` (scenarios `atip.claim15.01-15`): canon declares atip/1 1.5 and writes the white space of its domain patterns as the Unicode
//! property `White_Space`, against the built binary.
//!
//! The declaration is one value (`json_out::DECLARED_MINOR`), which every envelope, every `schema` answer, the fixture, the job and the recipes of the checker and the
//! consumers say. The 59 declarations (17 distinct patterns) of canon 0.36.0 that listed white space by hand are `\p{White_Space}` and `\P{White_Space}` now
//! (Annex V2), and the values they take and refuse are the values of 0.36.0: `atip/white_space_0_36.json` holds those patterns as `canon schema` printed them, and
//! `atip.claim15.04` holds each new pattern to its old one over every scalar value, so no value changed sides.
//!
//! The checks that seeded faults must trip are small pure functions over text or `serde_json::Value` in this file (`hand_listed`, `alias_problems`, `differences`,
//! `minor_problems`, `consumer_problems`, `changelog_problems`): each real input passes them, each seeded fault is found, and no sweep is vacuous.
//! Tests are named by scenario id (`atip_claim15_05_…` proves `atip.claim15.05`).

#[path = "support/hermetic.rs"]
mod hermetic;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

use atip::constraint::document_problems;
use atip::doc::{OptionDoc, ValueType};
use serde_json::{json, Value};
use tempfile::TempDir;

const MINOR: &str = "1.5";

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
    run(env!("CARGO_BIN_EXE_canon"), &["init", "--providers", "none"]);
    run("git", &["add", "-A"]);
    run("git", &["-c", "user.name=canon", "-c", "user.email=canon@example.invalid", "commit", "-q", "-m", "fixture"]);
    run(env!("CARGO_BIN_EXE_canon"), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    run(env!("CARGO_BIN_EXE_canon"), &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    dir
}

/// One answer of `canon schema …`: the document and the text as stdout held it, without the newline that ends the line.
struct Answer {
    value: Value,
    text: String,
}

/// Runs `canon <args>` in `dir`, expecting a success with a clean stderr, one line of stdout.
fn answer(dir: &Path, args: &[String]) -> Answer {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = canon(dir, &args);
    assert!(output.status.success() && output.stderr.is_empty(), "canon {args:?}: exit {:?}: {}{}", output.status.code(), stdout(&output), stderr(&output));
    let text = stdout(&output);
    let text = text.strip_suffix('\n').unwrap_or_else(|| panic!("canon {args:?}: stdout ends with a newline")).to_string();
    let value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("canon {args:?}: stdout is one JSON document ({e}): {text}"));
    Answer { value, text }
}

/// The answers of `argvs`, in order, run on a few threads (each call builds the whole model: a quarter of a second in a debug build).
fn answers(dir: &Path, argvs: &[Vec<String>]) -> Vec<Answer> {
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<Option<Answer>>> = Mutex::new(argvs.iter().map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..6 {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::SeqCst);
                let Some(argv) = argvs.get(index) else { break };
                let got = answer(dir, argv);
                done.lock().unwrap()[index] = Some(got);
            });
        }
    });
    done.into_inner().unwrap().into_iter().map(|got| got.expect("every call answered")).collect()
}

fn schema_argv(path: &str, extra: &[&str]) -> Vec<String> {
    let mut argv = vec!["schema".to_string()];
    argv.extend(path.split(' ').filter(|word| !word.is_empty()).map(str::to_string));
    argv.extend(extra.iter().map(ToString::to_string));
    argv.push("--json".to_string());
    argv
}

/// A directory that is no repository: the cwd of every call that must not need one.
static NEUTRAL: LazyLock<TempDir> = LazyLock::new(|| TempDir::new().unwrap());

/// The tree and every command document, answered by the binary.
struct Surface {
    tree: Answer,
    docs: BTreeMap<String, Answer>,
}

static SURFACE: LazyLock<Surface> = LazyLock::new(|| {
    let tree = answer(NEUTRAL.path(), &schema_argv("", &[]));
    let entries = tree.value["data"]["commands"].as_array().expect("the tree lists commands");
    let names: Vec<String> = entries.iter().filter(|entry| entry["kind"] != "group").map(|entry| entry["command"].as_str().expect("an entry names its command").to_string()).collect();
    let argvs: Vec<Vec<String>> = names.iter().map(|name| schema_argv(name, &[])).collect();
    let docs = names.into_iter().zip(answers(NEUTRAL.path(), &argvs)).collect();
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

/// The time options and the provider list that `canon-0-37-option-forms` narrowed to RFC 3339 and to the exact names. A place of the golden file that holds one has a
/// pattern that deliberately refuses what 0.36.0 took (white space, a one-digit month, a sign on the year, a date alone for `--approval-at`, a capital in a provider),
/// and says it without the white-space property, so it is not held to the old pattern (`atip.optionforms.*` hold it; `white_space_0_36.json` names the list under
/// `changedBy037Forms`).
const OPTION_FORMS_OPTIONS: [&str; 7] = ["approval-at", "expiry", "as-of", "before", "after", "since", "providers"];

/// How many of the 59 places of the golden file hold an option of each change, how many distinct patterns of the unchanged places are compared with their new spelling,
/// and how many distinct patterns that name white space the build declares now.
const VALUES_PLACES: usize = 17;
const FORMS_PLACES: usize = 13;
const CHANGED_PLACES: usize = VALUES_PLACES + FORMS_PLACES;
const DISTINCT_COMPARED: usize = 10;
const DISTINCT_NOW: usize = 11;

fn changed_by_option_values(place: &str) -> bool {
    place.rsplit_once(" --").is_some_and(|(_, option)| OPTION_VALUES_OPTIONS.contains(&option))
}

fn changed_by_option_forms(place: &str) -> bool {
    place.rsplit_once(" --").is_some_and(|(_, option)| OPTION_FORMS_OPTIONS.contains(&option))
}

/// A place of the golden file that a 0.37 change re-declared on purpose.
fn changed_by_037(place: &str) -> bool {
    changed_by_option_values(place) || changed_by_option_forms(place)
}

/// The places that had no pattern with white space in 0.36.0 and have the text pattern or the regime key now.
fn new_in_option_values(place: &str) -> bool {
    changed_by_option_values(place) || place == "dispatch begin --regime"
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
// atip.claim15.01 - 02 (F1)

/// Every problem of what the binary says it declares: the minor of an envelope, the minor of a `schema` answer and the level of the tree.
fn declaration_problems(envelope: &Value, is_schema_answer: bool) -> Vec<String> {
    let mut found = Vec::new();
    if envelope["meta"]["contractVersion"] != MINOR {
        found.push(format!("meta.contractVersion is {}, not {MINOR}", envelope["meta"]["contractVersion"]));
    }
    if is_schema_answer && envelope["meta"]["schemaVersion"] != MINOR {
        found.push(format!("meta.schemaVersion is {}, not {MINOR}", envelope["meta"]["schemaVersion"]));
    }
    found
}

#[test]
fn atip_claim15_01_every_envelope_and_schema_answer_says_1_5_and_the_tree_says_l2() {
    let dir = repo();
    let at = dir.path();
    assert_eq!(canon_cli::json_out::DECLARED_MINOR, MINOR, "the constant");
    let calls: [(&str, Vec<&str>, bool); 8] = [
        ("the tree", vec!["schema", "--json"], true),
        ("a command document", vec!["schema", "finding", "close", "--json"], true),
        ("the schemas of a command", vec!["schema", "finding", "close", "--schemas", "--json"], true),
        ("the error registry", vec!["schema", "--errors", "--json"], true),
        ("the warning registry", vec!["schema", "--warnings", "--json"], true),
        ("a status", vec!["status", "--json"], false),
        ("a refusal", vec!["finding", "list", "--change", "NOT A SLUG", "--json"], false),
        ("an unknown command", vec!["nosuchcommand", "--json"], false),
    ];
    for (what, args, schema) in calls {
        let output = canon(at, &args);
        assert!(output.stderr.is_empty(), "{what}: stderr is empty: {}", stderr(&output));
        let document: Value = serde_json::from_str(&stdout(&output)).unwrap_or_else(|e| panic!("{what} is one document ({e})"));
        assert_eq!(declaration_problems(&document, schema), Vec::<String>::new(), "{what} (`canon {args:?}`)");
    }
    let tree = &SURFACE.tree.value;
    assert_eq!(tree["data"]["level"], "L2", "the tree claims L2");
    assert_eq!(tree["data"]["kind"], "tree");
    // Every command document and the tree: the same two members.
    assert!(SURFACE.docs.len() > 70, "the sweep covered every command ({})", SURFACE.docs.len());
    let wrong: Vec<String> = SURFACE.docs.iter().flat_map(|(name, got)| declaration_problems(&got.value, true).into_iter().map(move |problem| format!("{name}: {problem}"))).collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert_eq!(declaration_problems(&SURFACE.tree.value, true), Vec::<String>::new());
    // The generated spec says it too.
    let spec: Value = serde_json::from_str(&read("docs/cli/canon.cli.json")).expect("the generated spec is JSON");
    assert_eq!(spec["contractVersion"], MINOR, "the generated spec");
    // The same build answers the same bytes outside a repository, from two directories.
    let other = TempDir::new().unwrap();
    assert_eq!(stdout(&canon(other.path(), &["schema", "--json"])), format!("{}\n", SURFACE.tree.text), "`schema --json` is the same bytes from another directory");
}

#[test]
fn atip_claim15_02_a_declaration_of_1_4_fails_the_check_and_the_crate_judges_a_document_by_the_declared_minor() {
    // The check fails for a constant, a fixture or an answer that says 1.4.
    let answer = json!({ "ok": true, "data": {}, "meta": { "contractVersion": "1.4", "schemaVersion": "1.4" } });
    let problems = declaration_problems(&answer, true);
    assert_found("a 1.4 envelope", &problems, "meta.contractVersion");
    assert_found("a 1.4 schema answer", &problems, "meta.schemaVersion");
    assert_eq!(declaration_problems(&json!({ "meta": { "contractVersion": "1.5" } }), false), Vec::<String>::new());
    assert_found("a missing declaration", &declaration_problems(&json!({ "meta": {} }), false), "meta.contractVersion");

    // The crate's checks take the minor canon declares: a property pattern is a problem of a 1.4 document and none of a 1.5 one.
    assert_eq!(declared(), minor(MINOR), "the crate reads the declared minor");
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
    assert!(declared.len() > 120, "the sweep covered the tree and every command document ({} patterns)", declared.len());
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
    for (place, pattern) in declared.iter().filter(|(place, _)| changed_by_option_values(place)) {
        assert!(pattern.contains("[^\\p{Cc}\\p{Zl}\\p{Zp}]"), "`{place}` declares the one text pattern of 0.37: {pattern}");
    }
    for (place, pattern) in declared.iter().filter(|(place, _)| changed_by_option_forms(place)) {
        let want = if place.ends_with("--providers") {
            canon_cli::arg_rules::PROVIDERS
        } else if place.ends_with("--approval-at") {
            canon_cli::arg_rules::DATE_TIME
        } else {
            canon_cli::arg_rules::TIMESTAMP
        };
        assert_eq!(pattern, want, "`{place}` declares the RFC 3339 or the provider pattern of 0.37");
    }
    let distinct: BTreeSet<&String> = declared.iter().filter(|(_, pattern)| pattern.contains("White_Space")).map(|(_, pattern)| pattern).collect();
    assert_eq!(distinct.len(), DISTINCT_NOW, "the distinct patterns now: {distinct:#?}");
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
    assert!(strings.len() > 60_000, "the corpus is not vacuous ({} values)", strings.len());
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
    assert!(count > 1_100_000, "every scalar value was read ({count})");
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
        assert!(atip::domain::pattern_problem(bad, declared()).is_some(), "the crate refuses {bad} at {MINOR}");
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
        assert_eq!(document["meta"]["contractVersion"], MINOR);
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
    assert_eq!(distinct.len(), DISTINCT_NOW);
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
    assert!(plain.len() > 50);
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
// atip.claim15.10 - 11 (F5)

/// The `--contract` values of `lines`.
fn contracts_in(lines: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for line in lines {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        for (index, token) in tokens.iter().enumerate() {
            if *token == "--contract" {
                if let Some(next) = tokens.get(index + 1) {
                    found.push(next.trim_matches(['"', '\'']).to_string());
                }
            } else if let Some(value) = token.strip_prefix("--contract=") {
                found.push(value.trim_matches(['"', '\'']).to_string());
            }
        }
    }
    found
}

/// The lines of the job `name` of a workflow: from its two-space-indented key to the next job.
fn job_block(workflow: &str, name: &str) -> Vec<String> {
    let header = format!("  {name}:");
    let lines = workflow.lines().skip_while(|line| *line != header).skip(1);
    let mut block = Vec::new();
    for line in lines {
        let is_job_key = line.starts_with("  ") && !line.starts_with("   ") && line.trim_end().ends_with(':') && !line.trim_start().starts_with('#');
        if is_job_key {
            break;
        }
        block.push(line.to_string());
    }
    block
}

/// The body of the recipe `name` of a justfile.
fn recipe(justfile: &str, name: &str) -> Vec<String> {
    let mut lines = justfile.lines();
    lines.by_ref().find(|line| line.split(':').next() == Some(name) && line.contains(':') && !line.starts_with(' ')).unwrap_or_else(|| panic!("the justfile has the recipe `{name}`"));
    lines.take_while(|line| line.starts_with("    ") || line.starts_with('\t')).map(str::to_string).collect()
}

fn tokens_of(lines: &[String], needle: &str) -> Vec<String> {
    let line = lines.iter().find(|line| line.contains(needle)).unwrap_or_else(|| panic!("a line holds `{needle}`: {lines:#?}"));
    line.split_whitespace().map(str::to_string).collect()
}

/// The value after `flag` in `tokens`.
fn value_after<'a>(tokens: &'a [String], flag: &str) -> Option<&'a str> {
    tokens.iter().position(|token| token == flag).and_then(|index| tokens.get(index + 1)).map(String::as_str)
}

/// Every problem of the declared minors: each of the five must be `1.5` (they agree because they are the one value).
fn minor_problems(declared: &str, fixture: &str, ci: &[String], atip: &[String], deep: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    let mut check = |what: &str, values: &[String]| {
        if values.is_empty() {
            found.push(format!("{what} gives no minor"));
        }
        for value in values {
            if value != MINOR {
                found.push(format!("{what} says {value}, not {MINOR}"));
            }
        }
    };
    check("DECLARED_MINOR", &[declared.to_string()]);
    check("the fixture's contractVersion", &[fixture.to_string()]);
    check("the CI job `atip`", ci);
    check("the recipe `atip`", atip);
    check("the recipe `atip-deep`", deep);
    found
}

const FIXTURE: &str = "crates/canon-cli/tests/atip/canon.fixture.json";

#[test]
fn atip_claim15_10_the_job_and_the_recipes_run_the_checker_at_l2_1_5_with_the_fixture_and_the_deep_report_binds() {
    let workflow = read(".github/workflows/test.yml");
    let justfile = read("justfile");
    let fixture: Value = serde_json::from_str(&read(FIXTURE)).expect("the fixture is JSON");
    let fixture_minor = fixture["contractVersion"].as_str().expect("the fixture has a contractVersion").to_string();
    let job = job_block(&workflow, "atip");
    assert!(job.len() > 20, "the job `atip` was read ({} lines)", job.len());
    let (atip, deep) = (recipe(&justfile, "atip"), recipe(&justfile, "atip-deep"));
    let (ci, plain, long) = (contracts_in(&job), contracts_in(&atip), contracts_in(&deep));
    assert_eq!(minor_problems(canon_cli::json_out::DECLARED_MINOR, &fixture_minor, &ci, &plain, &long), Vec::<String>::new(), "the declaration, the fixture, the job and the recipes");
    assert_eq!((ci.len(), plain.len(), long.len()), (1, 1, 1), "each gives its `--contract` once: {ci:?} {plain:?} {long:?}");
    for (what, lines, with_deep) in [("the job `atip`", &job, true), ("the recipe `atip-deep`", &deep, true), ("the recipe `atip`", &atip, false)] {
        let tokens = tokens_of(lines, "atip check --bin");
        assert_eq!(value_after(&tokens, "--level"), Some("L2"), "{what}: {tokens:?}");
        assert_eq!(value_after(&tokens, "--contract"), Some(MINOR), "{what}: {tokens:?}");
        assert_eq!(tokens.iter().any(|token| token == "--deep"), with_deep, "{what}: {tokens:?}");
        assert_eq!(value_after(&tokens, "--fixture"), Some(FIXTURE), "{what}: {tokens:?}");
    }
    // The tree the fixture runs against says it, and the checker is the one of contract 1.5.
    assert_eq!(SURFACE.tree.value["meta"]["contractVersion"], MINOR);
    assert!(job.iter().any(|line| line.contains("cargo install") && line.contains("--tag atip-check-v0.2.0")), "the job installs the checker at atip-check-v0.2.0");
    let install = recipe(&justfile, "atip-install");
    assert!(install.iter().any(|line| line.contains("--tag atip-check-v0.2.0")), "`just atip-install` installs the checker at atip-check-v0.2.0");
    // The fixture the checker reads keeps what the L1.patterns row compares: values a domain takes (`ok`) and refuses (`domainRejects`).
    assert!(fixture["commands"].as_object().is_some_and(|commands| commands.values().any(|command| command.get("domainRejects").is_some())), "the fixture lists values the domains refuse");
}

#[test]
fn atip_claim15_11_lowering_the_declaration_the_fixture_the_job_or_a_recipe_fails_the_agreement() {
    let workflow = read(".github/workflows/test.yml");
    let justfile = read("justfile");
    let fixture: Value = serde_json::from_str(&read(FIXTURE)).expect("the fixture is JSON");
    let fixture_minor = fixture["contractVersion"].as_str().unwrap().to_string();
    let ci = contracts_in(&job_block(&workflow, "atip"));
    let atip = contracts_in(&recipe(&justfile, "atip"));
    let deep = contracts_in(&recipe(&justfile, "atip-deep"));
    let declared = canon_cli::json_out::DECLARED_MINOR;
    let one = |value: &str| vec![value.to_string()];
    let lowered = [
        minor_problems("1.4", &fixture_minor, &ci, &atip, &deep),
        minor_problems(declared, "1.4", &ci, &atip, &deep),
        minor_problems(declared, &fixture_minor, &one("1.4"), &atip, &deep),
        minor_problems(declared, &fixture_minor, &ci, &one("1.4"), &deep),
        minor_problems(declared, &fixture_minor, &ci, &atip, &one("1.4")),
    ];
    let named = ["DECLARED_MINOR", "fixture", "CI job", "`atip`", "`atip-deep`"];
    for (problems, name) in lowered.iter().zip(named) {
        assert_found(&format!("{name} lowered to 1.4"), problems, name);
        assert_eq!(problems.len(), 1, "only the lowered one is found: {problems:?}");
    }
    assert_found("a missing declaration", &minor_problems(declared, &fixture_minor, &[], &atip, &deep), "gives no minor");
    // The readers of the files find a lowered value too.
    assert_eq!(contracts_in(&job_block(&workflow.replace("--contract 1.5", "--contract 1.4"), "atip")), vec!["1.4".to_string()]);
    assert_eq!(contracts_in(&recipe(&justfile.replace("--contract 1.5", "--contract 1.4"), "atip-deep")), vec!["1.4".to_string()]);
    // A fixture that claims 1.4 is the fixture the test of the generated fixture refuses (its source is the one that says 1.5).
    let source = read("crates/canon-cli/tests/atip/canon.fixture.src.json");
    assert!(source.contains("\"contractVersion\": \"1.5\""), "the source of the fixture says 1.5");
}

// ===========================================================================
// atip.claim15.12 - 13 (F6)

/// The versions below 1.5 that `line` spells as the declaration: after `contractVersion`, `--contract`, `claims` and `declares`.
fn old_declarations(line: &str) -> Vec<String> {
    let skip = |text: &str| text.trim_start_matches([' ', '`', '"', '\'', ':', '=', '\\']).to_string();
    let mut found = Vec::new();
    for needle in ["contractVersion", "--contract", "claims", "declares", "declaring"] {
        for (at, _) in line.match_indices(needle) {
            let mut rest = skip(&line[at + needle.len()..]);
            if let Some(after) = rest.strip_prefix("atip/1") {
                rest = skip(after);
            }
            let bytes = rest.as_bytes();
            let is_old = bytes.len() >= 3 && &bytes[..2] == b"1." && (b'0'..=b'4').contains(&bytes[2]) && bytes.get(3).is_none_or(|next| !next.is_ascii_digit());
            if is_old {
                found.push(format!("{needle} {}", &rest[..3]));
            }
        }
    }
    found
}

/// Every stale sentence of one text file: a declaration below 1.5, and a list of white space shown by hand (the line and paragraph separators together).
fn consumer_problems(path: &str, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let shown: String = line.trim().chars().take(160).collect();
        for declared in old_declarations(line) {
            found.push(format!("{path}:{}: declares {declared}: {shown}", number + 1));
        }
        // The class `[^\p{Cc}\p{Zl}\p{Zp}]` of a name or a sentence (`canon-0-37-option-values`) names the two separators beside the controls: it is no list of white space, as written in a document and as escaped in JSON.
        if line.contains("\\p{Zl}") && line.contains("\\p{Zp}") && !line.contains("\\p{Cc}\\p{Zl}\\p{Zp}") && !line.contains("\\\\p{Cc}\\\\p{Zl}\\\\p{Zp}") {
            found.push(format!("{path}:{}: shows a list of white space: {shown}", number + 1));
        }
    }
    found
}

/// The minors every `"contractVersion":"…"` of a data file says (a conformance expectation prints envelopes).
fn printed_minors(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for line in text.lines() {
        for (at, key) in line.match_indices("\"contractVersion\":\"") {
            if let Some(value) = line[at + key.len()..].split('"').next() {
                found.insert(value.to_string());
            }
        }
    }
    found
}

/// The tracked files whose text is a consumer of the declaration: the guides, the website pages (en and ko), the eval scripts, the conformance
/// expectations, the generated spec, README and ARCHITECTURE, the workflow and the justfile. The CHANGELOG, `.canon`, the tests (`crates/canon-cli/tests/atip_*`
/// name older minors on purpose) and the research notes are not.
fn is_consumer(path: &str) -> bool {
    path == "README.md" || path == "ARCHITECTURE.md" || path == "justfile" || path == "docs/cli/canon.cli.json" || path == ".github/workflows/test.yml" || ["canon/", "packages/website/src/content/docs/", "evals/", "conformance/"].iter().any(|prefix| path.starts_with(prefix))
}

fn tracked_files() -> Vec<String> {
    let listed = Command::new("git").current_dir(root()).args(["ls-files", "-z"]).output().expect("git ls-files");
    assert!(listed.status.success(), "{}", stderr(&listed));
    String::from_utf8(listed.stdout).unwrap().split('\0').filter(|path| !path.is_empty()).map(str::to_string).collect()
}

#[test]
fn atip_claim15_12_the_consumers_declare_1_5_and_show_no_list_of_white_space() {
    let root = root();
    let mut scanned = 0;
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    let mut minors = BTreeSet::new();
    for path in tracked_files().into_iter().filter(|path| is_consumer(path)) {
        let Ok(text) = std::fs::read_to_string(root.join(&path)) else { continue };
        scanned += 1;
        seen.insert(path.clone());
        found.extend(consumer_problems(&path, &text));
        if path.starts_with("conformance/") && path.ends_with(".out") {
            minors.extend(printed_minors(&text));
        }
    }
    assert!(scanned > 50, "the scan read the consumers ({scanned})");
    for needed in ["README.md", "ARCHITECTURE.md", "docs/cli/canon.cli.json", "justfile", ".github/workflows/test.yml", "packages/website/src/content/docs/cli.mdx", "packages/website/src/content/docs/ko/cli.mdx", "canon/skills/SKILL.src.md"] {
        assert!(seen.contains(needed), "the scan read `{needed}`");
    }
    assert!(seen.iter().any(|path| path.starts_with("evals/")) && seen.iter().any(|path| path.starts_with("conformance/cases/")), "the scan read the evals and the conformance expectations");
    assert_eq!(minors, BTreeSet::from([MINOR.to_string()]), "every envelope an expectation prints declares {MINOR}");
    assert!(found.is_empty(), "stale consumer text:\n{}", found.join("\n"));
    // The pages say the declaration where they name it.
    for page in ["packages/website/src/content/docs/cli.mdx", "packages/website/src/content/docs/ko/cli.mdx"] {
        let text = read(page);
        assert!(text.contains("\"contractVersion\": \"1.5\"") && text.contains("`contractVersion`") && text.contains("`1.5`"), "`{page}` says 1.5");
    }
    assert!(read("canon/skills/SKILL.src.md").contains("`contractVersion` (`1.5`)"), "the guide says 1.5");
}

#[test]
fn atip_claim15_13_the_consumer_scan_finds_a_declaration_below_1_5_and_a_shown_list_of_white_space() {
    for (version, line) in [
        ("1.4", "canon declares `\"contractVersion\": \"1.4\"` on every envelope"),
        ("1.4", "and `just atip-deep` runs `atip check --level L2 --contract 1.4 --deep`"),
        ("1.3", "{\"meta\":{\"contractVersion\":\"1.3\"}}"),
        ("1.4", "canon claims atip/1 1.4 at L2"),
        ("1.2", "The tree declares contractVersion=1.2."),
    ] {
        let found = consumer_problems("guide.md", line);
        assert_found(&format!("the declaration {version} in `{line}`"), &found, "declares");
        assert!(found.iter().any(|problem| problem.contains(version)), "the finding names {version}: {found:?}");
    }
    for line in ["canon declares `\"contractVersion\": \"1.5\"` on every envelope", "runs `atip check --level L2 --contract 1.5 --deep`", "canon claims atip/1 1.5 at L2", "the schema `1.4` of a report is not the declaration", "contractVersion 1.50"] {
        assert_eq!(consumer_problems("guide.md", line), Vec::<String>::new(), "`{line}` is no stale declaration");
    }
    // A list of white space shown by hand, as 0.36.0 printed it, is found; the class of 1.5 is not.
    let old = golden().remove(0).0;
    assert_found("a pattern of 0.36.0 shown in a guide", &consumer_problems("guide.md", &format!("`{old}`")), "list of white space");
    assert_eq!(consumer_problems("guide.md", "a name is `[^\\p{Cc}\\p{Zl}\\p{Zp}]` and a line is `^\\P{White_Space}$`"), Vec::<String>::new());
    // The printed minors of an expectation are found when one says 1.4.
    assert_eq!(printed_minors("{\"meta\":{\"contractVersion\":\"1.4\"}}\n{\"meta\":{\"contractVersion\":\"1.5\"}}"), BTreeSet::from(["1.4".to_string(), "1.5".to_string()]));
}

// ===========================================================================
// atip.claim15.14 - 15 (F7)

const ENTRY_HEADING: &str = "canon declares atip/1 1.5";

/// The pattern of `--session` is the one the entry measures: its length in characters before (0.36.0, from the golden file) and after.
const MEASURED_PLACE: &str = "finding add --session";

fn heading_level(line: &str) -> usize {
    line.chars().take_while(|c| *c == '#').count()
}

fn cells(row: &str) -> Vec<String> {
    row.trim().trim_matches('|').split('|').map(|cell| cell.trim().to_string()).collect()
}

/// Every problem of the CHANGELOG's `Unreleased` section: one entry whose heading names the declaration, its before/after table (the declaration, the white space in a
/// pattern, the length of one pattern: `before` and `after`, characters) and the paragraph for the callers (1.4 callers that check patterns, callers that only compile).
fn changelog_problems(text: &str, before: usize, after: usize) -> Vec<String> {
    let mut found = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines.iter().position(|line| line.starts_with("## 0.37.0")) else { return vec!["there is no `## 0.37.0` section".to_string()] };
    let end = lines.iter().enumerate().skip(start + 1).find(|(_, line)| line.starts_with("## ")).map_or(lines.len(), |(index, _)| index);
    let section = &lines[start + 1..end];
    let headings: Vec<usize> = section.iter().enumerate().filter(|(_, line)| (line.starts_with('#') || line.starts_with("**")) && line.contains(ENTRY_HEADING)).map(|(index, _)| index).collect();
    if headings.len() != 1 {
        return vec![format!("`0.37.0` holds {} headings that name `{ENTRY_HEADING}`, not one", headings.len())];
    }
    let heading = headings[0];
    let level = if section[heading].starts_with('#') { heading_level(section[heading]) } else { 3 };
    let entry_end = section.iter().enumerate().skip(heading + 1).find(|(_, line)| line.starts_with('#') && heading_level(line) <= level).map_or(section.len(), |(index, _)| index);
    let entry = &section[heading + 1..entry_end];

    let rows: Vec<&str> = entry.iter().copied().filter(|line| line.trim_start().starts_with('|')).collect();
    if !rows.iter().any(|row| cells(row).iter().all(|cell| !cell.is_empty() && cell.chars().all(|c| matches!(c, '-' | ':' | ' ')))) {
        found.push("the entry has no table".to_string());
    }
    let table = rows.join("\n").to_lowercase();
    for word in ["declaration", "white space", "pattern length", "`1.4`", "`1.5`", "`\\p{white_space}`"] {
        if !table.contains(word) {
            found.push(format!("the before/after table has no `{word}`"));
        }
    }
    let length_row = rows.iter().find(|row| cells(row).first().is_some_and(|first| first.to_lowercase().contains("pattern length")));
    match length_row {
        None => found.push("the table has no row for the length of a pattern".to_string()),
        Some(row) => {
            let numbers: Vec<usize> = cells(row).iter().skip(1).filter_map(|cell| cell.split_whitespace().next().and_then(|word| word.replace(',', "").parse().ok())).collect();
            if numbers != [before, after] {
                found.push(format!("the length row says {numbers:?}, the pattern is {before} then {after} characters"));
            }
            if after >= before {
                found.push(format!("the pattern is not shorter ({before} then {after})"));
            }
        }
    }
    let note = entry.iter().find(|line| line.starts_with("**Callers:**"));
    match note {
        None => found.push("the entry has no `**Callers:**` paragraph".to_string()),
        Some(note) => {
            for needle in ["1.4", "pattern", "compile", "refuse", "profile"] {
                if !note.contains(needle) {
                    found.push(format!("the callers paragraph does not say `{needle}`"));
                }
            }
        }
    }
    found
}

fn chars_of(pattern: &str) -> usize {
    pattern.chars().count()
}

/// The lengths the entry states: the pattern of `--session` in 0.36.0 and now.
fn measured() -> (usize, usize) {
    let golden = golden();
    let old = &golden.iter().find(|(_, at)| at.iter().any(|place| place == MEASURED_PLACE)).expect("the golden has the session pattern").0;
    (chars_of(old), chars_of(&DECLARED[MEASURED_PLACE]))
}

fn complete_entry(before: usize, after: usize) -> String {
    format!(
        "## 0.37.0\n\n### Changed\n\n**{ENTRY_HEADING} and writes white space in its patterns as `\\p{{White_Space}}`** (`canon-0-37-claim-1-5`).\n\n| | 0.36 (declares 1.4) | 0.37 (declares 1.5) |\n|---|---|---|\n| the declaration | `1.4` | `1.5` |\n| white space in a pattern | a list of 25 code points | `\\p{{White_Space}}` |\n| pattern length of `--session`, characters | {before} | {after} |\n\n**Callers:** a 1.4 caller that checks patterns by its profile refuses the escape; one that only compiles the pattern is unchanged.\n\n### Contributors\n\nx\n\n## 0.36.0\n"
    )
}

#[test]
fn atip_claim15_14_the_changelog_entry_has_the_table_the_lengths_and_the_note_for_callers() {
    let (before, after) = measured();
    assert!(before > after && after > 20, "the pattern got shorter ({before} then {after})");
    let problems = changelog_problems(&read("CHANGELOG.md"), before, after);
    assert!(problems.is_empty(), "the CHANGELOG entry `{ENTRY_HEADING}` under `Unreleased`: {problems:#?}");
}

#[test]
fn atip_claim15_15_the_entry_check_fails_when_the_table_a_length_or_the_callers_paragraph_is_missing() {
    let (before, after) = (93, 63);
    let complete = complete_entry(before, after);
    assert_eq!(changelog_problems(&complete, before, after), Vec::<String>::new(), "the complete entry passes:\n{complete}");
    let without = |needle: &str| complete.lines().filter(|line| !line.contains(needle)).collect::<Vec<_>>().join("\n");
    assert_found("no callers paragraph", &changelog_problems(&without("**Callers:**"), before, after), "Callers");
    assert_found("no length row", &changelog_problems(&without("pattern length"), before, after), "length");
    assert_found("no declaration row", &changelog_problems(&without("the declaration |"), before, after), "declaration");
    assert_found("no table", &changelog_problems(&complete.lines().filter(|line| !line.starts_with('|')).collect::<Vec<_>>().join("\n"), before, after), "no table");
    assert_found("a wrong length", &changelog_problems(&complete.replace(&format!("| {after} |"), "| 70 |"), before, after), "length row says");
    assert_found("a callers paragraph without the profile", &changelog_problems(&complete.replace("by its profile", "somehow"), before, after), "profile");
    assert_found("no entry", &changelog_problems(&complete.replace(ENTRY_HEADING, "canon declares something"), before, after), "not one");
    assert_found("no section", &changelog_problems("# Changelog\n", before, after), "no `## 0.37.0`");
    assert_found("a longer pattern", &changelog_problems(&complete_entry(60, 93), 60, 93), "not shorter");
}
