//! `canon-0-41-atip-1-6` (scenarios `atip.claim16.*`; `11` and `12` were removed by `canon-0-43-atip-0-3-2`): canon declares atip/1 1.6 and uses neither of its two optional members: no repeatable option declares `commaList`
//! and the tree declares no `pathShape`. The documents are read in process from the declared model (`canon_cli::spec`); the built binary answers the calls that need a process.
//!
//! Each check that a seeded fault must trip is a small pure function over text or `serde_json::Value` in this file (`declaration_problems`, `forbidden_member_problems`,
//! `depth_problems`, `one_meaning_problems`): the real input passes it and each seeded fault is found. The checker itself is not
//! run (the CI job `atip` and `just atip-deep` run it).
//! Tests are named by scenario id (`atip_claim16_05_…` proves `atip.claim16.05`).

#[path = "support/hermetic.rs"]
mod hermetic;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::LazyLock;

use canon_cli::json_out::DECLARED_MINOR;
use serde_json::{json, Value};
use tempfile::TempDir;

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

/// The tree and every command document, as the declared model builds them (what `canon schema` answers, without a process per command).
struct Surface {
    tree: Value,
    docs: BTreeMap<String, Value>,
}

fn answer_of(document: &atip::Document<'_>) -> Value {
    json!({ "ok": true, "data": document.to_value() })
}

static SURFACE: LazyLock<Surface> = LazyLock::new(|| {
    let described = canon_cli::spec::described(&canon_cli::spec::built_root(), None).expect("the documents are built from tables the tests keep whole");
    let tree = answer_of(&described.tree());
    let docs = tree["data"]["commands"].as_array().expect("the tree lists commands").iter().filter(|entry| entry["kind"] != "group").map(|entry| {
        let name = entry["command"].as_str().expect("a command");
        (name.to_string(), answer_of(&described.command(name).expect("a command of the tree has its document")))
    }).collect();
    Surface { tree, docs }
});

// ---------------------------------------------------------------------------
// atip.claim16.02 (F1): the declaration.

/// Every way an envelope does not declare the minor canon declares: `contractVersion`, and `schemaVersion` of a `schema` answer.
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

/// atip.claim16.02: a declaration of 1.5 in an envelope or a schema answer is found and named.
#[test]
fn atip_claim16_02_a_declaration_of_1_5_fails_the_declaration_check() {
    let answer = json!({ "ok": true, "data": {}, "meta": { "contractVersion": "1.5", "schemaVersion": "1.5" } });
    let problems = declaration_problems(&answer, true);
    assert_found("a 1.5 envelope", &problems, "meta.contractVersion");
    assert_found("a 1.5 schema answer", &problems, "meta.schemaVersion");
    assert_found("a refusal that says 1.5", &declaration_problems(&json!({ "meta": { "contractVersion": "1.5" } }), false), "meta.contractVersion");
    assert_found("a missing declaration", &declaration_problems(&json!({ "meta": {} }), false), "meta.contractVersion");
    assert_eq!(declaration_problems(&json!({ "meta": { "contractVersion": DECLARED_MINOR } }), false), Vec::<String>::new());
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
    assert!(!outputs.is_empty(), "the scan read the expectations");
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
    let old = r#"{"meta":{"contractVersion":"1.5","schemaVersion": "1.5","toolVersion":"0.40.0"},"x":1}"#;
    let new = format!(r#"{{"meta":{{"contractVersion":"{DECLARED_MINOR}","schemaVersion":"{DECLARED_MINOR}","toolVersion":"0.40.0"}},"x":1}}"#);
    assert_eq!(without_declarations(old), without_declarations(&new), "two expectations that differ in the declaration strings are equal beyond them");
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
    assert!(total > 0, "the sweep saw the options of every document");
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
