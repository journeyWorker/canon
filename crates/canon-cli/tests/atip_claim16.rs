//! `canon-0-41-atip-1-6` (scenarios `atip.claim16.01-14`): canon declares atip/1 1.6 and uses neither of its two optional members: no repeatable option declares `commaList`
//! and the tree declares no `pathShape`. The crate is `atip-crate-v0.3.0`, the checker `atip-check-v0.3.0`.
//!
//! Each check that a seeded fault must trip is a small pure function over text or `serde_json::Value` in this file (`declaration_problems`, `forbidden_member_problems`,
//! `depth_problems`, `one_meaning_problems`, `stale_tag_lines`, `changelog_problems`): the real input passes it and each seeded fault is found. The checker itself is not
//! run (the CI job `atip` and `just atip-deep` run it). The one-off comparison with the 0.40.0 binary is evidence, not a test: a test cannot run 0.40.0.
//! Tests are named by scenario id (`atip_claim16_05_…` proves `atip.claim16.05`).

#[path = "support/hermetic.rs"]
mod hermetic;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

use serde_json::{json, Value};
use tempfile::TempDir;

const MINOR: &str = "1.6";
const FIXTURE: &str = "crates/canon-cli/tests/atip/canon.fixture.json";

// ---------------------------------------------------------------------------
// Running the binary.

fn canon(dir: &Path, args: &[&str]) -> Output {
    hermetic::canon().args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-claim16").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).output().expect("canon runs")
}

fn json_of(output: &Output) -> Value {
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {text}"))
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("`{path}` is readable: {e}"))
}

fn assert_found(what: &str, problems: &[String], needle: &str) {
    assert!(problems.iter().any(|problem| problem.contains(needle)), "{what}: no problem names `{needle}`: {problems:#?}");
}

/// A directory that is no repository: the cwd of every call that must not need one.
static NEUTRAL: LazyLock<TempDir> = LazyLock::new(|| TempDir::new().unwrap());

/// The tree and every command document, answered by the binary.
struct Surface {
    tree: Value,
    docs: BTreeMap<String, Value>,
}

static SURFACE: LazyLock<Surface> = LazyLock::new(|| {
    let ask = |args: Vec<String>| -> Value {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = canon(NEUTRAL.path(), &args);
        assert!(output.status.success() && output.stderr.is_empty(), "canon {args:?}: exit {:?}", output.status.code());
        json_of(&output)
    };
    let tree = ask(vec!["schema".into(), "--json".into()]);
    let names: Vec<String> = tree["data"]["commands"].as_array().expect("the tree lists commands").iter().filter(|entry| entry["kind"] != "group").map(|entry| entry["command"].as_str().expect("a command").to_string()).collect();
    let next = AtomicUsize::new(0);
    let done: Mutex<BTreeMap<String, Value>> = Mutex::new(BTreeMap::new());
    std::thread::scope(|scope| {
        for _ in 0..6 {
            scope.spawn(|| {
                while let Some(name) = names.get(next.fetch_add(1, Ordering::SeqCst)) {
                    let mut args = vec!["schema".to_string()];
                    args.extend(name.split(' ').map(str::to_string));
                    args.push("--json".to_string());
                    let got = ask(args);
                    done.lock().unwrap().insert(name.clone(), got);
                }
            });
        }
    });
    Surface { tree, docs: done.into_inner().unwrap() }
});

// ---------------------------------------------------------------------------
// atip.claim16.01 - 02 (F1): the declaration.

/// Every way an envelope does not declare 1.6: `contractVersion`, and `schemaVersion` of a `schema` answer.
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

/// atip.claim16.01: every envelope and every `schema` answer says 1.6 and the tree says `L2`.
#[test]
fn atip_claim16_01_every_envelope_and_schema_answer_says_1_6_and_the_tree_says_l2() {
    assert_eq!(canon_cli::json_out::DECLARED_MINOR, MINOR, "the constant");
    let dir = NEUTRAL.path();
    for (what, args, schema) in [
        ("the errors registry", vec!["schema", "--errors", "--json"], true),
        ("the warnings registry", vec!["schema", "--warnings", "--json"], true),
        ("the schemas of a command", vec!["schema", "finding", "close", "--schemas", "--json"], true),
        ("a refusal", vec!["status", "--json"], false),
        ("an unknown command", vec!["nosuchcommand", "--json"], false),
    ] {
        let document = json_of(&canon(dir, &args));
        assert_eq!(declaration_problems(&document, schema), Vec::<String>::new(), "{what} (`canon {args:?}`)");
    }
    assert_eq!(declaration_problems(&SURFACE.tree, true), Vec::<String>::new(), "the tree");
    assert_eq!(SURFACE.tree["data"]["level"], "L2", "the tree claims L2");
    assert!(SURFACE.docs.len() > 70, "the sweep covered every command ({})", SURFACE.docs.len());
    let wrong: Vec<String> = SURFACE.docs.iter().flat_map(|(name, doc)| declaration_problems(doc, true).into_iter().map(move |problem| format!("{name}: {problem}"))).collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
    let spec: Value = serde_json::from_str(&read("docs/cli/canon.cli.json")).expect("the generated spec is JSON");
    assert_eq!(spec["contractVersion"], MINOR, "the generated spec");
}

/// atip.claim16.02: a declaration of 1.5 in an envelope or a schema answer is found and named; a constant of 1.5 fails the first test.
#[test]
fn atip_claim16_02_a_declaration_of_1_5_fails_the_declaration_check() {
    let answer = json!({ "ok": true, "data": {}, "meta": { "contractVersion": "1.5", "schemaVersion": "1.5" } });
    let problems = declaration_problems(&answer, true);
    assert_found("a 1.5 envelope", &problems, "meta.contractVersion");
    assert_found("a 1.5 schema answer", &problems, "meta.schemaVersion");
    assert_found("a refusal that says 1.5", &declaration_problems(&json!({ "meta": { "contractVersion": "1.5" } }), false), "meta.contractVersion");
    assert_found("a missing declaration", &declaration_problems(&json!({ "meta": {} }), false), "meta.contractVersion");
    assert_eq!(declaration_problems(&json!({ "meta": { "contractVersion": "1.6" } }), false), Vec::<String>::new());
    let mut broken = SURFACE.docs.clone();
    broken.get_mut("init").expect("a command").as_object_mut().unwrap()["meta"]["contractVersion"] = json!("1.5");
    let wrong: Vec<String> = broken.iter().flat_map(|(name, doc)| declaration_problems(doc, true).into_iter().map(move |problem| format!("{name}: {problem}"))).collect();
    assert_eq!(wrong.len(), 1, "{wrong:?}");
    assert!(wrong[0].starts_with("init:"), "the command is named: {wrong:?}");
}

// ---------------------------------------------------------------------------
// atip.claim16.03 - 04 (F2): the two optional members are not used.

/// The places in `value` that carry one of the 1.6 members (`commaList` on an option, `pathShape` on a tree).
fn forbidden_member_problems(what: &str, value: &Value) -> Vec<String> {
    fn walk(path: &str, value: &Value, found: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, inner) in map {
                    let here = format!("{path}/{key}");
                    if key == "commaList" || key == "pathShape" {
                        found.push(here.clone());
                    }
                    walk(&here, inner, found);
                }
            }
            Value::Array(items) => {
                for (index, inner) in items.iter().enumerate() {
                    walk(&format!("{path}/{index}"), inner, found);
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    walk("", value, &mut found);
    found.into_iter().map(|place| format!("{what}: `{place}` is a member of 1.6 canon does not use")).collect()
}

/// `text` with the value of every `"contractVersion"` and `"schemaVersion"` replaced: what two expectations of two declarations may differ in.
fn without_declarations(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = ["\"contractVersion\":", "\"schemaVersion\":"].iter().filter_map(|key| rest.find(key).map(|at| (at, key.len()))).min() {
        let (start, len) = at;
        out.push_str(&rest[..start + len]);
        rest = rest[start + len..].trim_start_matches([' ']);
        let Some(open) = rest.strip_prefix('"') else { continue };
        let end = open.find('"').map_or(open.len(), |end| end + 1);
        out.push_str("\"_\"");
        rest = &open[end..];
    }
    out.push_str(rest);
    out
}

fn conformance_outputs() -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut pending = vec![root().join("conformance/cases")];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).expect("the corpus is readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "out") {
                found.push((path.strip_prefix(root()).unwrap().display().to_string(), std::fs::read_to_string(&path).expect("an expectation")));
            }
        }
    }
    found
}

/// atip.claim16.03: no document, the generated spec or an expectation carries `commaList` or `pathShape`.
#[test]
fn atip_claim16_03_no_document_has_comma_list_or_path_shape() {
    let mut found = forbidden_member_problems("the tree", &SURFACE.tree);
    for (name, doc) in &SURFACE.docs {
        found.extend(forbidden_member_problems(&format!("`{name}`"), doc));
    }
    let spec: Value = serde_json::from_str(&read("docs/cli/canon.cli.json")).expect("the generated spec is JSON");
    found.extend(forbidden_member_problems("the generated spec", &spec));
    let outputs = conformance_outputs();
    assert!(outputs.len() > 100, "the scan read the expectations ({})", outputs.len());
    for (path, text) in &outputs {
        if text.contains("\"commaList\"") || text.contains("\"pathShape\"") {
            found.push(format!("`{path}` prints a member of 1.6 canon does not use"));
        }
    }
    assert!(found.is_empty(), "{found:#?}");
    for args in [vec!["schema", "--errors", "--json"], vec!["schema", "--warnings", "--json"], vec!["schema", "finding", "close", "--schemas", "--json"]] {
        let document = json_of(&canon(NEUTRAL.path(), &args));
        assert_eq!(forbidden_member_problems("an answer", &document), Vec::<String>::new(), "{args:?}");
    }
}

/// atip.claim16.04: a document with one of the members, and an expectation that differs beyond the declaration strings, are found.
#[test]
fn atip_claim16_04_a_document_with_a_member_of_1_6_or_an_expectation_that_differs_beyond_the_declaration_is_found() {
    let mut option = SURFACE.docs["init"].clone();
    option["data"]["options"].as_array_mut().unwrap().iter_mut().find(|o| o["name"] == "harness").expect("--harness").as_object_mut().unwrap().insert("commaList".into(), json!(true));
    assert_found("a commaList on --harness", &forbidden_member_problems("`init`", &option), "/data/options/");
    let mut tree = SURFACE.tree.clone();
    tree["data"].as_object_mut().unwrap().insert("pathShape".into(), json!(["noun", "verb"]));
    assert_found("a pathShape on the tree", &forbidden_member_problems("the tree", &tree), "/data/pathShape");
    let (old, new) = (r#"{"meta":{"contractVersion":"1.5","schemaVersion": "1.5","toolVersion":"0.40.0"},"x":1}"#, r#"{"meta":{"contractVersion":"1.6","schemaVersion":"1.6","toolVersion":"0.40.0"},"x":1}"#);
    assert_eq!(without_declarations(old), without_declarations(new), "two expectations that differ in the declaration strings are equal beyond them");
    let changed = new.replace("\"x\":1", "\"x\":2");
    assert_ne!(without_declarations(old), without_declarations(&changed), "a difference beyond the declaration strings is found");
    let other_tool = new.replace("0.40.0", "0.41.0");
    assert_ne!(without_declarations(old), without_declarations(&other_tool), "the tool version is no declaration string here");
}

// ---------------------------------------------------------------------------
// atip.claim16.05 - 06 (F3): comma lists.

fn refusal(dir: &Path, args: &[&str]) -> Value {
    let output = canon(dir, args);
    assert_eq!(output.status.code(), Some(2), "canon {args:?}: {}", String::from_utf8_lossy(&output.stdout));
    let document = json_of(&output);
    assert_eq!(document["ok"], false, "{document}");
    document
}

/// atip.claim16.05: a comma list for `--harness` or `--kind` is refused as a whole, `--input` refuses the same value at its pointer, one option per value passes.
#[test]
fn atip_claim16_05_a_comma_list_is_refused_whole_and_one_option_per_value_passes() {
    let dir = TempDir::new().unwrap();
    let harness = refusal(dir.path(), &["init", "--harness", "claude,omp", "--json"]);
    let error = &harness["error"];
    assert_eq!((error["code"].as_str(), error["field"].as_str(), error["received"].as_str()), (Some("USAGE"), Some("--harness"), Some("claude,omp")), "{harness}");
    assert_eq!(error["expected"], json!(["claude", "codex", "omp", "pi"]), "{harness}");
    let kind = refusal(dir.path(), &["search", "x", "--kind", "change,task", "--json"]);
    let error = &kind["error"];
    assert_eq!((error["code"].as_str(), error["field"].as_str(), error["received"].as_str()), (Some("USAGE"), Some("--kind"), Some("change,task")), "{kind}");
    let expected: Vec<&str> = error["expected"].as_array().expect("the allowed values").iter().map(|value| value.as_str().expect("a value")).collect();
    assert!(expected.contains(&"change") && expected.contains(&"task") && expected.iter().all(|value| !value.contains(',')), "{kind}");
    let input = dir.path().join("input.json");
    std::fs::write(&input, r#"{"harness":["claude,omp"]}"#).unwrap();
    let from_input = refusal(dir.path(), &["init", "--input", input.to_str().unwrap(), "--json"]);
    let error = &from_input["error"];
    assert_eq!((error["code"].as_str(), error["field"].as_str(), error["received"].as_str()), (Some("INPUT_INVALID"), Some("/harness/0"), Some("claude,omp")), "{from_input}");
    // The call that gives one option per value passes, in a directory with nothing set up.
    let fresh = TempDir::new().unwrap();
    let ok = canon(fresh.path(), &["init", "--harness", "claude", "--harness", "omp", "--no-agents-md", "--json"]);
    assert_eq!(ok.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&ok.stdout), String::from_utf8_lossy(&ok.stderr));
    assert_eq!(json_of(&ok)["ok"], true);
}

/// atip.claim16.06: an option that declares `commaList` while the parser refuses the comma string is found, and the real parser refuses it.
#[test]
fn atip_claim16_06_a_declared_comma_list_the_parser_does_not_split_is_found() {
    let repeatable: Vec<String> = SURFACE
        .docs
        .iter()
        .flat_map(|(name, doc)| doc["data"]["options"].as_array().into_iter().flatten().filter(|option| option["repeatable"] == true).map(move |option| format!("{name} --{}", option["name"].as_str().unwrap_or("?"))))
        .collect();
    assert!(repeatable.iter().any(|place| place == "init --harness"), "the sweep sees the repeatable options: {repeatable:?}");
    assert!(repeatable.len() >= 10, "{repeatable:?}");
    let mut seeded = SURFACE.docs["init"].clone();
    seeded["data"]["options"].as_array_mut().unwrap().iter_mut().find(|o| o["name"] == "harness").unwrap().as_object_mut().unwrap().insert("commaList".into(), json!(true));
    assert_found("a declared comma list", &forbidden_member_problems("`init`", &seeded), "commaList");
    let dir = TempDir::new().unwrap();
    let refused = refusal(dir.path(), &["init", "--harness", "claude,omp", "--json"]);
    assert_eq!(refused["error"]["code"], "USAGE", "the parser does not split what the seeded document declares: {refused}");
}

// ---------------------------------------------------------------------------
// atip.claim16.07 - 08 (F4): path depth.

/// Every way the tree's paths break the decision: a `pathShape`, or a command path of more than two words.
fn depth_problems(tree: &Value) -> Vec<String> {
    let mut found = Vec::new();
    if tree["data"].get("pathShape").is_some() {
        found.push("the tree declares a pathShape".to_string());
    }
    for entry in tree["data"]["commands"].as_array().into_iter().flatten() {
        let path = entry["command"].as_str().unwrap_or_default();
        let words = path.split(' ').filter(|word| !word.is_empty()).count();
        if words > 2 {
            found.push(format!("`{path}` has {words} words: a path of more than two words needs a pathShape or the L1.depth advice"));
        }
    }
    found
}

/// atip.claim16.07: the longest path is two words and the tree has no `pathShape`.
#[test]
fn atip_claim16_07_the_longest_path_is_two_words_and_the_tree_has_no_path_shape() {
    assert_eq!(depth_problems(&SURFACE.tree), Vec::<String>::new());
    let lengths: BTreeSet<usize> = SURFACE.tree["data"]["commands"].as_array().expect("commands").iter().map(|entry| entry["command"].as_str().unwrap().split(' ').count()).collect();
    assert_eq!(lengths, BTreeSet::from([1, 2]), "paths of one and two words, none longer");
    assert!(SURFACE.tree["data"].get("pathShape").is_none());
}

/// atip.claim16.08: a tree with a three-word entry or a `pathShape` is found.
#[test]
fn atip_claim16_08_a_tree_with_a_three_word_entry_or_a_path_shape_fails_the_depth_check() {
    let mut deep = SURFACE.tree.clone();
    deep["data"]["commands"].as_array_mut().unwrap().push(json!({ "command": "tier age now", "kind": "command", "summary": "s" }));
    assert_found("a three-word entry", &depth_problems(&deep), "`tier age now` has 3 words");
    let mut shaped = SURFACE.tree.clone();
    shaped["data"].as_object_mut().unwrap().insert("pathShape".into(), json!(["noun", "verb"]));
    assert_found("a pathShape", &depth_problems(&shaped), "pathShape");
}

// ---------------------------------------------------------------------------
// atip.claim16.09 - 10 (F5): one name, one meaning.

/// The value name of an option document: the member, or the upper-cased option name where the document leaves it out (an option that takes a value and names it
/// as the contract's default), or none for a switch and a count.
fn value_name_of(option: &Value) -> String {
    match (option["valueName"].as_str(), option["valueType"].as_str()) {
        (Some(name), _) => name.to_string(),
        (None, Some("switch" | "count")) => "(none)".to_string(),
        (None, _) => option["name"].as_str().unwrap_or_default().to_uppercase(),
    }
}

/// The options by name over the tree's shared options and every document: the value names and value kinds each name has, and where.
fn one_meaning_problems(tree: &Value, docs: &[(&str, &Value)]) -> Vec<String> {
    let mut names: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();
    let mut take = |option: &Value| {
        let Some(name) = option["name"].as_str() else { return };
        let entry = names.entry(name.to_string()).or_default();
        entry.0.insert(value_name_of(option));
        entry.1.insert(option["valueType"].as_str().unwrap_or("(none)").to_string());
    };
    for option in tree["data"]["options"].as_array().into_iter().flatten() {
        take(option);
    }
    for (_, doc) in docs {
        for option in doc["data"]["options"].as_array().into_iter().flatten() {
            take(option);
        }
    }
    let mut found = Vec::new();
    for (name, (value_names, value_types)) in names {
        if value_names.len() > 1 {
            found.push(format!("`--{name}` has the value names {value_names:?}"));
        }
        if value_types.len() > 1 {
            found.push(format!("`--{name}` has the value kinds {value_types:?}"));
        }
    }
    found
}

/// atip.claim16.09: every option name has one value name and one value kind over every command document.
#[test]
fn atip_claim16_09_an_option_name_has_one_value_name_and_one_value_kind_in_every_document() {
    let docs: Vec<(&str, &Value)> = SURFACE.docs.iter().map(|(name, doc)| (name.as_str(), doc)).collect();
    assert_eq!(one_meaning_problems(&SURFACE.tree, &docs), Vec::<String>::new());
    let kind_names: BTreeSet<String> = docs.iter().flat_map(|(_, doc)| doc["data"]["options"].as_array().into_iter().flatten().filter(|o| o["name"] == "kind").map(value_name_of)).collect();
    assert_eq!(kind_names, BTreeSet::from(["KIND".to_string()]), "`--kind` of `gate promote` and `purge` keep the one value name that the L1.one-meaning-values advice names");
    let total: usize = docs.iter().map(|(_, doc)| doc["data"]["options"].as_array().map_or(0, Vec::len)).sum();
    assert!(total > 300, "the sweep saw the options of every document ({total})");
}

/// atip.claim16.10: documents that give one name two value names or two value kinds are found.
#[test]
fn atip_claim16_10_a_document_set_with_one_name_and_two_meanings_fails_the_check() {
    let mut other = SURFACE.docs["purge"].clone();
    let kind = other["data"]["options"].as_array_mut().unwrap().iter_mut().find(|o| o["name"] == "kind").expect("--kind");
    kind["valueName"] = json!("RECORD");
    let mut docs: Vec<(&str, &Value)> = SURFACE.docs.iter().filter(|(name, _)| name.as_str() != "purge").map(|(name, doc)| (name.as_str(), doc)).collect();
    docs.push(("purge", &other));
    assert_found("a second value name", &one_meaning_problems(&SURFACE.tree, &docs), "`--kind` has the value names");
    let mut retyped = SURFACE.docs["purge"].clone();
    retyped["data"]["options"].as_array_mut().unwrap().iter_mut().find(|o| o["name"] == "kind").unwrap()["valueType"] = json!("string");
    let mut docs: Vec<(&str, &Value)> = SURFACE.docs.iter().filter(|(name, _)| name.as_str() != "purge").map(|(name, doc)| (name.as_str(), doc)).collect();
    docs.push(("purge", &retyped));
    assert_found("a second value kind", &one_meaning_problems(&SURFACE.tree, &docs), "`--kind` has the value kinds");
}

// ---------------------------------------------------------------------------
// atip.claim16.11 - 12 (F6): the tags.

/// The lines of `text` that name the checker or the crate at 0.2.0.
fn stale_tag_lines(text: &str) -> Vec<String> {
    text.lines().enumerate().filter(|(_, line)| line.contains("atip-check-v0.2.0") || line.contains("atip-crate-v0.2.0")).map(|(number, line)| format!("line {}: {}", number + 1, line.trim())).collect()
}

/// atip.claim16.11: no consumer names the tags of 0.2.0, and the install lines of the job, the recipe and the test name the checker at 0.3.0.
#[test]
fn atip_claim16_11_the_install_lines_and_the_dependency_name_the_tags_of_0_3_0() {
    for path in ["justfile", ".github/workflows/test.yml", "Cargo.toml", "README.md", "ARCHITECTURE.md", "crates/canon-cli/tests/atip_check.rs"] {
        assert_eq!(stale_tag_lines(&read(path)), Vec::<String>::new(), "`{path}` names a tag of 0.2.0");
    }
    let install = "--tag atip-check-v0.3.0 atip-check --locked";
    assert!(read("justfile").contains(install), "`just atip-install`");
    assert!(read(".github/workflows/test.yml").contains(install), "the job installs the checker at 0.3.0");
    assert!(read(".github/workflows/test.yml").contains("key: canon-test-atip-check-v0.3.0-"), "the cache key names the tag");
    assert!(read("crates/canon-cli/tests/atip_check.rs").contains(install), "the test's install constant");
    assert!(read("Cargo.toml").contains("tag = \"atip-crate-v0.3.0\""), "the dependency");
    let lock = read("Cargo.lock");
    assert!(lock.contains("source = \"git+https://github.com/journeyWorker/atip?tag=atip-crate-v0.3.0#9cbc74a4"), "the lock holds the tag at its commit");
    let fixture: Value = serde_json::from_str(&read(FIXTURE)).expect("the fixture is JSON");
    assert_eq!(fixture["contractVersion"], MINOR, "the fixture");
}

/// atip.claim16.12: an install line or a dependency at 0.2.0 is found.
#[test]
fn atip_claim16_12_a_line_at_a_tag_of_0_2_0_is_found() {
    let stale = "run: cargo install --git https://github.com/journeyWorker/atip --tag atip-check-v0.2.0 atip-check --locked\nkey: x\natip = { tag = \"atip-crate-v0.2.0\" }";
    let found = stale_tag_lines(stale);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_found("the install line", &found, "line 1");
    assert_found("the dependency", &found, "line 3");
    assert_eq!(stale_tag_lines("--tag atip-check-v0.3.0 atip-check --locked and atip-crate-v0.3.0"), Vec::<String>::new(), "a line at 0.3.0 is no stale line");
}

// ---------------------------------------------------------------------------
// atip.claim16.13 - 14 (F7): the CHANGELOG.

const ENTRY_HEADING: &str = "canon declares atip/1 1.6";

/// Every problem of the newest section of the CHANGELOG (`Unreleased` or the release it became): one entry that names the declaration with a before and after table
/// (the declaration, the tags, the options that declare `commaList`, `pathShape`, the longest path, the names with two meanings, the advice), a `**Callers:**` paragraph,
/// a `**Not used:**` paragraph that says why neither member is used and how to use it, and the contributors line.
fn changelog_problems(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines.iter().position(|line| line.starts_with("## ")) else { return vec!["the CHANGELOG has no section".to_string()] };
    let end = lines.iter().enumerate().skip(start + 1).find(|(_, line)| line.starts_with("## ")).map_or(lines.len(), |(index, _)| index);
    let section = &lines[start + 1..end];
    let mut found = Vec::new();
    let headings: Vec<usize> = section.iter().enumerate().filter(|(_, line)| line.contains(ENTRY_HEADING)).map(|(index, _)| index).collect();
    if headings.len() != 1 {
        return vec![format!("the newest section holds {} lines that name `{ENTRY_HEADING}`, not one", headings.len())];
    }
    let entry = &section[headings[0]..];
    let table = entry.iter().filter(|line| line.trim_start().starts_with('|')).copied().collect::<Vec<_>>().join("\n").to_lowercase();
    if table.is_empty() {
        found.push("the entry has no table".to_string());
    }
    for word in ["declaration", "`1.5`", "`1.6`", "atip-crate-v0.2.0", "atip-crate-v0.3.0", "atip-check-v0.2.0", "atip-check-v0.3.0", "`commalist`", "`pathshape`", "longest path", "two meanings", "should advice"] {
        if !table.contains(word) {
            found.push(format!("the before/after table has no `{word}`"));
        }
    }
    match entry.iter().find(|line| line.starts_with("**Callers:**")) {
        None => found.push("the entry has no `**Callers:**` paragraph".to_string()),
        Some(note) => {
            for needle in ["1.5", "15.10", "nothing to change", "string", "\"1.6\""] {
                if !note.contains(needle) {
                    found.push(format!("the callers paragraph does not say `{needle}`"));
                }
            }
        }
    }
    match entry.iter().find(|line| line.starts_with("**Not used:**")) {
        None => found.push("the entry has no `**Not used:**` paragraph".to_string()),
        Some(note) => {
            for needle in ["commaList", "pathShape", "value_delimiter", "L1.depth", "L1.comma-list"] {
                if !note.contains(needle) {
                    found.push(format!("the paragraph on the members not used does not say `{needle}`"));
                }
            }
        }
    }
    if !section.iter().any(|line| line.contains("9cbc74a") && line.contains("atip-crate-v0.3.0") && line.contains("atip-check-v0.3.0")) {
        found.push("the contributors line names neither the tags nor the commit 9cbc74a".to_string());
    }
    found
}

fn complete_entry() -> String {
    let rows = [
        "| | 0.40 | 0.41 |",
        "|---|---|---|",
        "| the declaration | `1.5` | `1.6` |",
        "| crate | atip-crate-v0.2.0 | atip-crate-v0.3.0 |",
        "| checker | atip-check-v0.2.0 | atip-check-v0.3.0 |",
        "| options that declare `commaList` | 0 | 0 |",
        "| `pathShape` | none | none |",
        "| longest path | two words | two words |",
        "| names with two meanings | 0 | 0 |",
        "| SHOULD advice | `--kind` | `--kind` |",
    ];
    format!(
        "## Unreleased\n\n### Changed\n\n**{ENTRY_HEADING}** (`canon-0-41-atip-1-6`).\n\n{}\n\n**Callers:** a 1.5 caller has nothing to change (spec 15.10); a caller that compares the string reads \"1.6\" for \"1.5\".\n\n**Not used:** `commaList` (an option splits at `value_delimiter`; `L1.comma-list` judges it) and `pathShape` (`L1.depth`).\n\n### Contributors\n\ncommit 9cbc74a: atip-crate-v0.3.0 and atip-check-v0.3.0.\n\n## 0.40.0\n",
        rows.join("\n")
    )
}

/// atip.claim16.13: the entry of the CHANGELOG has the table, the callers note, the reasons and the contributors line.
#[test]
fn atip_claim16_13_the_changelog_entry_has_the_table_the_callers_note_and_the_reasons() {
    assert_eq!(changelog_problems(&complete_entry()), Vec::<String>::new(), "the checker passes a complete entry");
    let problems = changelog_problems(&read("CHANGELOG.md"));
    assert!(problems.is_empty(), "the newest CHANGELOG section: {problems:#?}");
}

/// atip.claim16.14: an entry without the callers note, the reasons, a table row or the contributors line is found.
#[test]
fn atip_claim16_14_an_entry_without_the_callers_note_the_table_or_the_reasons_is_found() {
    let complete = complete_entry();
    let without_callers: String = complete.lines().filter(|line| !line.starts_with("**Callers:**")).collect::<Vec<_>>().join("\n");
    assert_found("no callers paragraph", &changelog_problems(&without_callers), "`**Callers:**`");
    let without_reasons: String = complete.lines().filter(|line| !line.starts_with("**Not used:**")).collect::<Vec<_>>().join("\n");
    assert_found("no reasons", &changelog_problems(&without_reasons), "`**Not used:**`");
    let without_row: String = complete.lines().filter(|line| !line.contains("longest path")).collect::<Vec<_>>().join("\n");
    assert_found("no longest path row", &changelog_problems(&without_row), "longest path");
    let without_contributors = complete.replace("9cbc74a", "0000000");
    assert_found("no commit", &changelog_problems(&without_contributors), "contributors line");
    assert_found("no entry", &changelog_problems("## Unreleased\n\n### Changed\n\nother\n"), "not one");
}
