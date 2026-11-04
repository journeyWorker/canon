//! `canon schema` as the atip/1 L1 documents (0.34 `canon-0-34-atip-schema-doc`, spec 10): the tree, group
//! and command documents, the value type and typed default of every option, the schemas a command carries, the
//! `--input` fragment schema and example, the help line that points at `schema`, and the checked-in spec that is
//! the same documents. Tests are named by scenario id (`atip.schemadoc.NN`); every sweep is over the documents the
//! generator builds, and the answers of the built binary are held equal to them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::LazyLock;

use jsonschema::{Registry, Resource};
use serde_json::{json, Value};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(args: &[&str], dir: &Path, role: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command.args(args).current_dir(dir).env_remove("CANON_ROLE");
    if let Some(role) = role {
        command.env("CANON_ROLE", role);
    }
    command.output().expect("spawn canon binary")
}

fn answer(args: &[&str]) -> (Option<i32>, Value, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = run(args, dir.path(), None);
    let text = String::from_utf8(out.stdout).expect("stdout is text");
    let value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{args:?}: {e}: {text}"));
    (out.status.code(), value, text)
}

/// The tree document, as the generator builds it.
static TREE: LazyLock<Value> = LazyLock::new(|| {
    let root = canon_cli::spec::built_root();
    serde_json::to_value(canon_cli::spec::tree(&root, None).expect("the tree is built")).expect("the tree serializes")
});

/// Every command document, as the generator builds them.
static COMMANDS: LazyLock<Vec<Value>> = LazyLock::new(|| {
    let root = canon_cli::spec::built_root();
    canon_cli::spec::commands(&root).expect("the commands are built").into_iter().map(|doc| serde_json::to_value(doc).expect("a command serializes")).collect()
});

fn names(value: &Value, key: &str) -> Vec<String> {
    value[key].as_array().map(|items| items.iter().filter_map(|item| item.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn option<'a>(doc: &'a Value, name: &str) -> &'a Value {
    doc["options"].as_array().and_then(|options| options.iter().find(|option| option["name"] == name)).unwrap_or_else(|| panic!("`{}` has no --{name}", doc["command"]))
}

fn command_doc(command: &str) -> &'static Value {
    COMMANDS.iter().find(|doc| doc["command"] == command).unwrap_or_else(|| panic!("`{command}` is not a command"))
}

fn camel(name: &str) -> String {
    atip::project::camel_case(name)
}

// ----------------------------------------------------------------------------------------------------------
// The rules of spec 10.13 a document is held to, and the schema validators they need.

/// A validator of the schema `name` of the command document `doc`, whose `schemas` the `$ref`s of the members resolve in.
fn member_validator(doc: &Value, name: &str) -> Result<jsonschema::Validator, String> {
    let registry = Registry::new().add("https://atip.dev/1/guide.schema.json", Resource::from_contents(serde_json::from_str(atip::GUIDE_SCHEMA).expect("the guide schema is JSON"))).expect("the contract schema registers").add("urn:canon:doc", Resource::from_contents(doc.clone())).map_err(|e| e.to_string())?.prepare().map_err(|e| e.to_string())?;
    jsonschema::options().with_registry(&registry).build(&json!({ "$ref": format!("urn:canon:doc#/schemas/{name}") })).map_err(|e| format!("`{name}` does not compile: {e}"))
}

fn ref_name(reference: &str) -> Option<&str> {
    reference.strip_prefix("#/schemas/")
}

/// Rules 1-10 of spec 10.13 (the ones a document alone decides) for one command document: one line per violation.
fn rule_problems(doc: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    let command = doc["command"].as_str().unwrap_or_default().to_string();
    let mut say = |text: String| problems.push(format!("`{command}`: {text}"));
    // 1
    let path = names(doc, "path");
    if path.join(" ") != command {
        say(format!("command is not its path joined by one space: {path:?}"));
    }
    // 2
    let schemas = doc["schemas"].as_object().cloned().unwrap_or_default();
    let mut refs: Vec<String> = Vec::new();
    for output in doc["outputs"].as_array().into_iter().flatten() {
        for key in ["schemaRef", "eventRef"] {
            if let Some(reference) = output[key].as_str() {
                refs.push(reference.to_string());
            }
        }
    }
    for key in ["schemaRef", "argumentsRef"] {
        if let Some(reference) = doc["input"][key].as_str() {
            refs.push(reference.to_string());
        }
    }
    for reference in &refs {
        match ref_name(reference) {
            Some(name) if schemas.contains_key(name) => {}
            _ => say(format!("the ref `{reference}` is not a member of schemas")),
        }
    }
    let output_refs: Vec<&str> = doc["outputs"].as_array().into_iter().flatten().filter_map(|output| output["schemaRef"].as_str()).collect();
    if output_refs.iter().collect::<BTreeSet<_>>().len() != output_refs.len() {
        say(format!("two outputs share a schemaRef: {output_refs:?}"));
    }
    for name in schemas.keys() {
        if let Err(reason) = member_validator(doc, name) {
            say(reason);
        }
    }
    // 3: an enum default is a member; a count's default is one integer
    for argument in doc["options"].as_array().into_iter().flatten().chain(doc["operands"].as_array().into_iter().flatten()) {
        let members = names(argument, "possibleValues");
        let default = &argument["default"];
        if argument["valueType"] == "enum" {
            let defaults: Vec<&Value> = match default {
                Value::Array(items) => items.iter().collect(),
                Value::Null => Vec::new(),
                single => vec![single],
            };
            for value in defaults {
                if !value.as_str().is_some_and(|text| members.iter().any(|member| member == text)) {
                    say(format!("the default {value} of `{}` is not in possibleValues", argument["name"]));
                }
            }
        }
        if argument["valueType"] == "count" && !default.is_u64() {
            say(format!("the default of the count `{}` is not one integer", argument["name"]));
        }
    }
    // 4: the reserved code of each feature
    let errors = names(doc, "errors");
    let has_option = |name: &str| doc["options"].as_array().into_iter().flatten().any(|option| option["name"] == name);
    let wants: Vec<(&str, bool)> = vec![
        ("INPUT_INVALID", doc["input"].is_object()),
        ("INPUT_CONFLICT", doc["input"].is_object()),
        ("INPUT_TOO_LARGE", doc["input"].is_object()),
        ("BATCH_INCOMPLETE", doc["input"]["batch"] == true),
        ("PLAN_STALE", has_option("commit")),
        ("CURSOR_INVALID", has_option("cursor")),
    ];
    for (code, applies) in wants {
        if applies && !errors.iter().any(|known| known == code) {
            say(format!("the errors lack {code}"));
        }
    }
    if errors.iter().collect::<BTreeSet<_>>().len() != errors.len() {
        say("an error code is listed twice".to_string());
    }
    // 5, 6
    let outputs = doc["outputs"].as_array().cloned().unwrap_or_default();
    let batch_outputs = outputs.iter().filter(|output| output["batch"] == true).count();
    if (doc["input"]["batch"] == true) != (batch_outputs > 0) {
        say(format!("input.batch is {} and the batch outputs are {batch_outputs}", doc["input"]["batch"]));
    }
    if outputs.len() > 1 {
        let plain = outputs.iter().filter(|output| output.get("when").is_none() && output["batch"] != true).count();
        if plain > 1 {
            say("two outputs have neither `when` nor `batch`".to_string());
        }
    }
    // 8: reserved options have their reserved shape
    for (name, value_type) in [("json", "switch"), ("commit", "switch"), ("all", "switch"), ("input", "string"), ("plan-digest", "string"), ("cursor", "string"), ("fields", "string")] {
        if has_option(name) && option(doc, name)["valueType"] != value_type {
            say(format!("--{name} is not a {value_type}"));
        }
    }
    if has_option("limit") && option(doc, "limit")["valueType"] != "integer" {
        say("--limit is not an integer".to_string());
    }
    for name in ["json", "input", "commit", "plan-digest"] {
        if has_option(name) && option(doc, name)["flagOnly"] != true {
            say(format!("--{name} is not flagOnly"));
        }
    }
    // 9
    let option_names: Vec<String> = doc["options"].as_array().into_iter().flatten().filter_map(|option| option["name"].as_str().map(str::to_string)).collect();
    let operand_names: Vec<String> = doc["operands"].as_array().into_iter().flatten().filter_map(|operand| operand["name"].as_str().map(str::to_string)).collect();
    if option_names.iter().collect::<BTreeSet<_>>().len() != option_names.len() || operand_names.iter().collect::<BTreeSet<_>>().len() != operand_names.len() {
        say("an option or operand name is repeated".to_string());
    }
    for operand in &operand_names {
        if option_names.contains(operand) {
            say(format!("the operand `{operand}` has the name of an option"));
        }
    }
    let shorts: Vec<&str> = doc["options"].as_array().into_iter().flatten().filter_map(|option| option["short"].as_str()).collect();
    if shorts.iter().collect::<BTreeSet<_>>().len() != shorts.len() {
        say("a short option is repeated".to_string());
    }
    // 10: the fragment
    if let Some(name) = doc["input"]["schemaRef"].as_str().and_then(ref_name) {
        let fragment = &schemas[name];
        if fragment.get("required").is_some() {
            say("the fragment schema has a root `required`".to_string());
        }
        if fragment["additionalProperties"] != json!(false) {
            say("the fragment schema is open".to_string());
        }
        let mut wanted: BTreeSet<String> = doc["options"].as_array().into_iter().flatten().filter(|option| option["flagOnly"] != true).filter_map(|option| option["name"].as_str().map(camel)).collect();
        if !operand_names.is_empty() {
            wanted.insert("operands".to_string());
        }
        let got: BTreeSet<String> = fragment["properties"].as_object().map(|properties| properties.keys().cloned().collect()).unwrap_or_default();
        if got != wanted {
            say(format!("the fragment's properties are {got:?}, not {wanted:?}"));
        }
    }
    problems
}

/// Rule 7 for one command document: the `input.example` passes the fragment and the arguments schema.
fn example_problems(doc: &Value) -> Vec<String> {
    let Some(input) = doc["input"].as_object() else { return Vec::new() };
    let command = doc["command"].as_str().unwrap_or_default();
    let mut problems = Vec::new();
    for key in ["schemaRef", "argumentsRef"] {
        let Some(name) = input.get(key).and_then(Value::as_str).and_then(ref_name) else { continue };
        match member_validator(doc, name) {
            Ok(validator) => {
                for error in validator.iter_errors(&input["example"]) {
                    problems.push(format!("`{command}`: the example fails `{name}`: {error} at {}", error.instance_path()));
                }
            }
            Err(reason) => problems.push(format!("`{command}`: {reason}")),
        }
    }
    problems
}

/// The words of an example, and what the real parser says of them (`None`: it parses).
fn parse_problem(example: &str) -> Option<String> {
    let argv = atip::split_argv(example);
    if argv.first().map(String::as_str) != Some("canon") {
        return Some(format!("does not start with the tool name: {example}"));
    }
    if atip::has_placeholder(example) || example.contains("...") || example.contains('…') {
        return Some(format!("has a placeholder or an ellipsis: {example}"));
    }
    let mut root = canon_cli::cli::command();
    root.build();
    root.try_get_matches_from(argv.iter().map(String::as_str)).err().map(|error| format!("does not parse: {example}: {}", error.render()))
}

// ----------------------------------------------------------------------------------------------------------
// The rules of the 1.4 form (contract Annex S2) for the tree and the command documents the file holds.

/// The reserved option names a 1.4 command document lists by name beside the tree's shared ones.
const RESERVED_NAMED: [&str; 2] = ["commit", "plan-digest"];

/// A value of a 1.4 document that is the default of its member, and so is absent.
fn default_members(owner: &str, object: &Value, problems: &mut Vec<String>) {
    let name = object["name"].as_str().unwrap_or_default();
    let value_type = object["valueType"].as_str().unwrap_or("string");
    for flag in ["required", "repeatable", "variadic", "flagOnly", "async"] {
        if object.get(flag) == Some(&json!(false)) {
            problems.push(format!("{owner}: `{name}` lists {flag}: false"));
        }
    }
    match (value_type, object.get("default")) {
        (_, Some(Value::Null)) => problems.push(format!("{owner}: `{name}` lists a null default")),
        ("switch", Some(Value::Bool(false))) | ("count", Some(Value::Number(_))) if object["default"] == json!(0) || object["default"] == json!(false) => problems.push(format!("{owner}: `{name}` lists the default of its type")),
        _ => {}
    }
    if object.get("valueType") == Some(&json!("string")) {
        problems.push(format!("{owner}: `{name}` lists valueType string"));
    }
    if object.get("valueName").and_then(Value::as_str) == Some(&name.to_uppercase().replace('-', "_")) {
        problems.push(format!("{owner}: `{name}` lists its upper-cased name as valueName"));
    }
    for (key, value) in object.as_object().into_iter().flatten() {
        if value.as_array().is_some_and(Vec::is_empty) {
            problems.push(format!("{owner}: `{name}` lists an empty {key}"));
        }
    }
}

/// The rules of the 1.4 form for `tree` (a 1.4 tree document's `data`), `commands` (1.4 command documents) and `schemas` (command -> the `--schemas` answer's data): one line per violation.
fn form_problems(tree: &Value, commands: &[Value], schemas: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    let shared: Vec<&str> = tree["options"].as_array().into_iter().flatten().map(|option| option["name"].as_str().expect("a shared option has a name")).collect();
    for option in tree["options"].as_array().into_iter().flatten() {
        default_members("tree", option, &mut problems);
    }
    let mut uses: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in tree["commands"].as_array().into_iter().flatten() {
        if entry.get("path").is_some() {
            problems.push(format!("tree entry `{}` has `path`", entry["command"]));
        }
        if entry["kind"] == "command" {
            problems.push(format!("tree entry `{}` has kind command", entry["command"]));
        }
    }
    let model = canon_cli::spec::model();
    let model_commands = model["commands"].as_array().expect("the model's commands");
    for doc in commands {
        let command = doc["command"].as_str().unwrap_or_default();
        let owner = format!("`{command}`");
        for member in ["path", "schemas"] {
            if doc.get(member).is_some() {
                problems.push(format!("{owner}: has `{member}`"));
            }
        }
        for validator in doc["validators"].as_array().into_iter().flatten() {
            if validator.get("rejects").is_some() {
                problems.push(format!("{owner}: a validator has `rejects`"));
            }
        }
        for key in ["schemaRef", "argumentsRef", "option"] {
            if key != "argumentsRef" && doc["input"].get(key).is_some() {
                problems.push(format!("{owner}: input has `{key}`"));
            }
        }
        if doc["input"].get("batch") == Some(&json!(false)) {
            problems.push(format!("{owner}: input lists batch: false"));
        }
        for output in doc["outputs"].as_array().into_iter().flatten() {
            if output["kind"] == "report" || output["schemaVersion"] == "1.0" {
                problems.push(format!("{owner}: an output lists the default kind report or schemaVersion 1.0"));
            }
        }
        for (key, value) in doc.as_object().into_iter().flatten() {
            if value.as_array().is_some_and(Vec::is_empty) || value.as_object().is_some_and(serde_json::Map::is_empty) {
                problems.push(format!("{owner}: lists an empty {key}"));
            }
        }
        let mut named: Vec<&str> = Vec::new();
        for option in doc["options"].as_array().into_iter().flatten() {
            match option {
                Value::String(name) => {
                    named.push(name);
                    *uses.entry(name).or_default() += 1;
                    if !shared.contains(&name.as_str()) && !RESERVED_NAMED.contains(&name.as_str()) {
                        problems.push(format!("{owner}: names `{name}`, which is neither a tree option nor a reserved option"));
                    }
                }
                object => {
                    let name = object["name"].as_str().unwrap_or_default();
                    named.push(name);
                    default_members(&owner, object, &mut problems);
                    if ["json", "input", "fields"].contains(&name) && (name == "json" || (name == "input" && doc["input"].is_object()) || (name == "fields" && doc["outputs"].as_array().into_iter().flatten().any(|output| output.get("fields").is_some()))) {
                        problems.push(format!("{owner}: lists the implied --{name}"));
                    }
                }
            }
        }
        for operand in doc["operands"].as_array().into_iter().flatten() {
            default_members(&owner, operand, &mut problems);
        }
        let errors = names(doc, "errors");
        let mut implied = vec!["USAGE", "INTERNAL"];
        if doc["input"].is_object() {
            implied.extend(["INPUT_INVALID", "INPUT_CONFLICT", "INPUT_TOO_LARGE"]);
        }
        if doc["input"]["batch"] == true {
            implied.push("BATCH_INCOMPLETE");
        }
        if named.contains(&"commit") {
            implied.push("PLAN_STALE");
        }
        if named.contains(&"cursor") {
            implied.push("CURSOR_INVALID");
        }
        for code in implied {
            if errors.iter().any(|known| known == code) {
                problems.push(format!("{owner}: lists the implied code {code}"));
            }
        }
        for code in &errors {
            if canon_cli::cli_error::code_info(code).is_none() {
                problems.push(format!("{owner}: the error `{code}` is not in the registry"));
            }
        }
        if named.contains(&"commit") != named.contains(&"plan-digest") {
            problems.push(format!("{owner}: lists --commit and --plan-digest apart"));
        }
        // The `--schemas` answer of the command: every ref resolves, `Input` is the derivation, `rejects` follow the validators.
        let answer = &schemas[command];
        let held = answer["schemas"].as_object().cloned().unwrap_or_default();
        let mut refs: Vec<String> = Vec::new();
        for output in doc["outputs"].as_array().into_iter().flatten() {
            for key in ["schemaRef", "eventRef"] {
                refs.extend(output[key].as_str().map(str::to_string));
            }
        }
        for key in ["schemaRef", "argumentsRef"] {
            refs.extend(doc["input"][key].as_str().map(str::to_string));
        }
        for reference in &refs {
            if !ref_name(reference).is_some_and(|name| held.contains_key(name)) {
                problems.push(format!("{owner}: the ref {reference} does not resolve in the --schemas answer"));
            }
        }
        for (name, schema) in &held {
            let wrapper = json!({ "schemas": answer["schemas"].clone() });
            if let Err(reason) = member_validator(&wrapper, name) {
                problems.push(format!("{owner}: {reason}"));
            }
            let _ = &schema;
        }
        let model_doc = model_commands.iter().find(|model_doc| model_doc["command"] == command).expect("the model has the command");
        if doc["input"].is_object() {
            let typed = canon_cli::spec::command(&canon_cli::spec::built_root(), &command.split(' ').map(str::to_string).collect::<Vec<_>>()).expect("the model has the command");
            if held.get("Input") != Some(&atip::fragment::derived_fragment(&typed.operands, &typed.options)) {
                problems.push(format!("{owner}: `Input` is not the derivation from the options"));
            }
        } else if held.contains_key("Input") {
            problems.push(format!("{owner}: has `Input` and no input"));
        }
        let validators = model_doc["validators"].as_array().cloned().unwrap_or_default();
        let rejects = answer["rejects"].as_array().cloned();
        let wanted: Option<Vec<Value>> = (!validators.is_empty()).then(|| validators.iter().map(|validator| validator["rejects"].clone()).collect());
        if rejects != wanted {
            problems.push(format!("{owner}: rejects are not the validators' rejects in order"));
        }
    }
    // A shared option is named by at least two commands, and every one defined in the tree is named by some.
    for name in shared {
        match uses.get(name) {
            None | Some(0) => problems.push(format!("tree option `{name}` is named by no command")),
            Some(1) => problems.push(format!("tree option `{name}` is named by one command only")),
            _ => {}
        }
    }
    problems
}

// ----------------------------------------------------------------------------------------------------------
// atip.schemadoc.01 — F1: the tree

/// atip.schemadoc.01: `canon schema --json` is a `tree` that claims L2, lists 27 groups and 78 commands once, a
/// group before its commands, byte-identically twice; `--as` filters and `CANON_ROLE` does not.
#[test]
fn atip_schemadoc_01_the_tree_lists_every_group_and_command_once_and_claims_l2() {
    let (code, first, text) = answer(&["schema", "--json"]);
    assert_eq!(code, Some(0));
    assert_eq!(first["data"]["kind"], "tree");
    assert_eq!(first["data"]["level"], "L2");
    assert_eq!(first["meta"]["schemaVersion"], "1.4");
    assert_eq!(first["meta"]["contractVersion"], "1.4");
    assert_eq!(first["meta"]["command"], "schema");
    assert_eq!(atip::schema_doc_problems(&first), Vec::<String>::new());
    assert!(first["data"].get("roles").is_none(), "the old top-level roles member is gone");
    let entries = first["data"]["commands"].as_array().expect("commands");
    let kinds: Vec<&str> = entries.iter().map(|entry| if entry["kind"] == "group" { "group" } else { "command" }).collect();
    assert!(entries.iter().all(|entry| entry["kind"] != "command" && entry.get("path").is_none()), "a 1.4 tree entry has no `kind: command` and no `path`");
    assert_eq!((kinds.iter().filter(|kind| **kind == "group").count(), kinds.iter().filter(|kind| **kind == "command").count()), (27, 78));
    let ids: Vec<&str> = entries.iter().map(|entry| entry["command"].as_str().expect("command")).collect();
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len(), "every id once");
    for (at, entry) in entries.iter().enumerate() {
        assert!(entry["summary"].is_string(), "{entry}");
        if entry["kind"] == "group" {
            assert!(entry.get("effect").is_none(), "a group has no effect");
        } else {
            assert!(entry["effect"].is_string(), "a command has one: {entry}");
            assert!(entry["x-canon"]["roles"].is_array(), "the roles of a command are x-canon.roles: {entry}");
            let parent = &ids[..at].iter().rev().find(|id| entry["command"].as_str().expect("command").starts_with(&format!("{id} "))).copied();
            if entry["command"].as_str().expect("command").contains(' ') {
                assert!(parent.is_some(), "a group comes before its command: {entry}");
            }
        }
    }
    assert!(ids.contains(&"schema"), "schema is listed like any other command");
    let (_, _, again) = answer(&["schema", "--json"]);
    assert_eq!(text, again, "two runs of one build print the same bytes");
    // `--as` is an explicit filter; CANON_ROLE is not one.
    let dir = tempfile::tempdir().expect("tempdir");
    let reviewer = serde_json::from_slice::<Value>(&run(&["schema", "--as", "reviewer", "--json"], dir.path(), None).stdout).expect("JSON");
    let listed = reviewer["data"]["commands"].as_array().expect("commands");
    assert!(!listed.is_empty() && listed.len() < entries.len());
    assert_eq!(reviewer["data"]["x-canon"], json!({ "as": "reviewer" }), "a filtered tree names the role in x-canon.as");
    assert_eq!(atip::schema_doc_problems(&reviewer), Vec::<String>::new());
    assert!(first["data"].get("x-canon").is_none(), "an unfiltered tree has no x-canon");
    for entry in listed {
        if entry["kind"] != "group" {
            assert!(names(&entry["x-canon"], "roles").contains(&"reviewer".to_string()), "{entry}");
        } else {
            let prefix = format!("{} ", entry["command"].as_str().expect("command"));
            assert!(listed.iter().any(|other| other["kind"] != "group" && other["command"].as_str().is_some_and(|c| c.starts_with(&prefix))), "a group is listed only for a command it holds: {entry}");
        }
    }
    let by_env = run(&["schema", "--json"], dir.path(), Some("reviewer"));
    let without = run(&["schema", "--json"], dir.path(), None);
    assert_eq!(by_env.stdout, without.stdout, "CANON_ROLE does not change the answer");
    let bad = run(&["schema", "--as", "janitor", "--json"], dir.path(), None);
    assert_eq!(bad.status.code(), Some(2), "an unknown role is refused, not listed as a filter");
}

/// atip.schemadoc.02: `schema` answers from an empty directory with no environment and writes nothing.
#[test]
fn atip_schemadoc_02_schema_answers_outside_a_repo_with_no_environment_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let home = tempfile::tempdir().expect("home");
    for args in [&["schema", "--json"][..], &["schema", "finding", "--json"], &["schema", "finding", "close", "--json"], &["schema", "--errors", "--json"]] {
        let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir.path()).env_clear().env("HOME", home.path()).output().expect("spawn canon");
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        assert!(out.stderr.is_empty(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        let document: Value = serde_json::from_slice(&out.stdout).expect("JSON");
        assert_eq!(document["ok"], true, "{args:?}");
    }
    assert_eq!(std::fs::read_dir(dir.path()).expect("dir").count(), 0, "the working directory is still empty");
    assert_eq!(std::fs::read_dir(home.path()).expect("home").count(), 0, "HOME is still empty");
}

// ----------------------------------------------------------------------------------------------------------
// atip.schemadoc.03 / 04 — F2: group, path, refusal

/// atip.schemadoc.03: a group document lists its direct children; a path in words or in one string gives one answer.
#[test]
fn atip_schemadoc_03_a_group_lists_its_children_and_a_path_in_words_or_one_string_is_one_answer() {
    let (code, group, _) = answer(&["schema", "finding", "--json"]);
    assert_eq!(code, Some(0));
    assert_eq!(group["data"]["kind"], "group");
    assert_eq!(group["data"]["command"], "finding");
    assert!(group["data"].get("path").is_none(), "a 1.4 group has no `path`: {group}");
    assert!(group["data"]["commands"].as_array().expect("commands").iter().all(|entry| entry.get("path").is_none() && entry["kind"] != "command"));
    assert_eq!(atip::schema_doc_problems(&group), Vec::<String>::new());
    let children: Vec<&str> = group["data"]["commands"].as_array().expect("commands").iter().map(|entry| entry["command"].as_str().expect("command")).collect();
    assert_eq!(children, ["finding add", "finding close", "finding list"]);
    let (_, words, _) = answer(&["schema", "finding", "close", "--json"]);
    let (_, quoted, _) = answer(&["schema", "finding close", "--json"]);
    assert_eq!(words, quoted);
    assert_eq!(words["data"]["kind"], "command");
    assert_eq!(atip::schema_doc_problems(&words), Vec::<String>::new());
    // Every group of the tree answers with the entries the tree lists for its direct children.
    let tree = &*TREE;
    let entries = tree["commands"].as_array().expect("commands");
    for entry in entries.iter().filter(|entry| entry["kind"] == "group") {
        let command = entry["command"].as_str().expect("command");
        let root = canon_cli::spec::built_root();
        let doc = serde_json::to_value(canon_cli::spec::group(&root, &names(entry, "path"), None).expect("a group")).expect("serializes");
        let direct: Vec<&Value> = entries.iter().filter(|other| other["path"].as_array().is_some_and(|path| path.len() == names(entry, "path").len() + 1) && other["command"].as_str().is_some_and(|c| c.starts_with(&format!("{command} ")))).collect();
        assert_eq!(doc["commands"].as_array().expect("commands").iter().collect::<Vec<_>>(), direct, "{command}");
        assert_eq!(doc["summary"], entry["summary"]);
    }
}

/// atip.schemadoc.04: an unknown path is `UNKNOWN_COMMAND` with the valid words; a path with a registry option is
/// `USAGE`; a group run bare is `COMMAND_REQUIRED`.
#[test]
fn atip_schemadoc_04_an_unknown_path_names_what_is_valid_and_overlapping_selectors_are_usage() {
    let (code, unknown, _) = answer(&["schema", "finding", "nosuch", "--json"]);
    assert_eq!(code, Some(2));
    assert_eq!(unknown["error"]["code"], "UNKNOWN_COMMAND");
    assert_eq!(names(&unknown["error"], "expected"), ["add", "close", "list"]);
    let (code, top, _) = answer(&["schema", "nosuch", "--json"]);
    assert_eq!((code, top["error"]["code"].clone()), (Some(2), json!("UNKNOWN_COMMAND")));
    assert!(names(&top["error"], "expected").contains(&"finding".to_string()));
    let (code, typo, _) = answer(&["schema", "finding", "clos", "--json"]);
    assert_eq!((code, names(&typo["error"], "suggestions")), (Some(2), vec!["close".to_string()]));
    for args in [&["schema", "finding", "--errors", "--json"][..], &["schema", "finding", "--error", "USAGE", "--json"], &["schema", "finding", "--warnings", "--json"], &["schema", "finding", "--warning", "OLD_LAYOUT_FOUND", "--json"], &["schema", "--errors", "--warnings", "--json"]] {
        let (code, failure, _) = answer(args);
        assert_eq!((code, failure["error"]["code"].clone()), (Some(2), json!("USAGE")), "{args:?}");
    }
    let (code, required, _) = answer(&["finding", "--json"]);
    assert_eq!((code, required["error"]["code"].clone()), (Some(2), json!("COMMAND_REQUIRED")));
    assert_eq!(names(&required["error"], "expected"), ["add", "close", "list"]);
}

/// atip.schemadoc.03: a group document under `--as` lists the children the role's tree lists, and a group the role's tree
/// omits is refused, so the tree and the groups never disagree about what a role has.
#[test]
fn atip_schemadoc_03_a_group_under_as_lists_what_the_roles_tree_lists() {
    let dir = tempfile::tempdir().expect("tempdir");
    let json = |args: &[&str]| -> (Option<i32>, Value) {
        let out = run(args, dir.path(), None);
        (out.status.code(), serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{args:?}: {e}")))
    };
    let mut refused = 0;
    for role in ["planner", "implementer", "reviewer", "orchestrator"] {
        let (_, tree) = json(&["schema", "--as", role, "--json"]);
        let entries = tree["data"]["commands"].as_array().expect("commands").clone();
        for group in TREE["commands"].as_array().expect("commands").iter().filter(|entry| entry["kind"] == "group") {
            let path = names(group, "path");
            let mut args: Vec<&str> = vec!["schema"];
            args.extend(path.iter().map(String::as_str));
            args.extend(["--as", role, "--json"]);
            let (code, answer) = json(&args);
            let wanted: Vec<&Value> = entries.iter().filter(|entry| entry["command"].as_str().is_some_and(|c| c.starts_with(&format!("{} ", group["command"].as_str().expect("command"))) && c.split(' ').count() == path.len() + 1)).collect();
            if wanted.is_empty() {
                refused += 1;
                assert_eq!((code, answer["error"]["code"].clone(), answer["error"]["field"].clone()), (Some(2), json!("UNKNOWN_COMMAND"), json!("command")), "{role} {path:?}");
                assert!(!entries.iter().any(|entry| entry["command"] == group["command"]), "the tree lists {path:?} for {role}");
            } else {
                assert_eq!(code, Some(0), "{role} {path:?}");
                assert_eq!(answer["data"]["commands"].as_array().expect("commands").iter().collect::<Vec<_>>(), wanted, "{role} {path:?}");
                assert_eq!(answer["data"]["x-canon"], json!({ "as": role }));
                assert!(entries.iter().any(|entry| entry["command"] == group["command"]), "{role} {path:?}");
            }
        }
    }
    assert!(refused > 0, "some group holds no command of some role");
    let (_, brief) = json(&["schema", "brief", "--as", "reviewer", "--json"]);
    let commands = names_of_entries(&brief["data"]["commands"]);
    assert!(!commands.contains(&"brief approve".to_string()) && commands.contains(&"brief check".to_string()), "{commands:?}");
    let (_, plain) = json(&["schema", "brief", "--json"]);
    assert!(plain["data"].get("x-canon").is_none() && names_of_entries(&plain["data"]["commands"]).contains(&"brief approve".to_string()));
}

fn names_of_entries(entries: &Value) -> Vec<String> {
    entries.as_array().expect("entries").iter().map(|entry| entry["command"].as_str().expect("command").to_string()).collect()
}

/// atip.schemadoc.04: an unknown word is `UNKNOWN_COMMAND` with field `command`; `expected` is the valid words where it
/// went wrong, and is absent after a command, where no word is valid (spec 7.1.1).
#[test]
fn atip_schemadoc_04_an_unknown_path_word_names_the_field_command_and_expects_only_where_a_word_is_valid() {
    let (code, group, _) = answer(&["schema", "finding", "nosuch", "--json"]);
    assert_eq!((code, group["error"]["code"].clone(), group["error"]["field"].clone()), (Some(2), json!("UNKNOWN_COMMAND"), json!("command")));
    assert_eq!(names(&group["error"], "expected"), ["add", "close", "list"]);
    let (code, past, _) = answer(&["schema", "finding", "close", "nosuch", "--json"]);
    assert_eq!((code, past["error"]["code"].clone(), past["error"]["field"].clone()), (Some(2), json!("UNKNOWN_COMMAND"), json!("command")));
    assert!(past["error"].get("expected").is_none(), "no word is valid after a command: {past}");
    let (_, top, _) = answer(&["schema", "nosuch", "--json"]);
    assert_eq!(top["error"]["field"], "command");
    assert!(names(&top["error"], "expected").contains(&"finding".to_string()));
}

/// atip.schemadoc.05: a variable fills only an option that declares it (spec 16.3): `--as` and attribution `--role` declare
/// `CANON_ROLE` on every command but `schema`, whose `--as` reads no variable; `--skills-source` declares
/// `CANON_SKILLS_SOURCE` on the three commands that read it; and every `CANON_*` a help text names is the option's `env`.
#[test]
fn atip_schemadoc_05_an_option_that_an_environment_variable_fills_declares_it() {
    let mut problems = Vec::new();
    for doc in COMMANDS.iter() {
        let command = doc["command"].as_str().expect("command");
        for option in doc["options"].as_array().expect("options") {
            let name = option["name"].as_str().expect("name");
            let env = option.get("env").and_then(Value::as_str);
            let expected = match (command, name) {
                ("schema", _) => None,
                (_, "as" | "role") => Some("CANON_ROLE"),
                ("skills install" | "skills check" | "skills doctor", "skills-source") => Some("CANON_SKILLS_SOURCE"),
                ("feature new" | "scenario new" | "brief approve" | "migrate" | "policy approve", "actor") => Some("CANON_ACTOR"),
                _ => None,
            };
            if env != expected {
                problems.push(format!("`{command}` --{name}: env is {env:?}, expected {expected:?}"));
            }
            // A variable the help names as filling the option is declared on it (the shared `--as` text names CANON_ROLE
            // as what it overrides, which `schema` does not read).
            let help = option["help"].as_str().expect("help");
            for variable in ["CANON_ACTOR", "CANON_SKILLS_SOURCE"] {
                if help.contains(variable) && env != Some(variable) {
                    problems.push(format!("`{command}` --{name}: the help names {variable} and the option does not declare it"));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert!(option(command_doc("schema"), "as").get("env").is_none(), "schema's --as reads no variable");
    assert_eq!(option(command_doc("status"), "as")["env"], "CANON_ROLE");
    assert_eq!(option(command_doc("skills install"), "providers")["defaultNote"].as_str().map(|note| note.contains("skills.providers")), Some(true));
    for command in ["skills check", "skills doctor"] {
        assert!(option(command_doc(command), "providers").get("defaultNote").is_some(), "{command}");
    }
}

// ----------------------------------------------------------------------------------------------------------
// atip.schemadoc.05 / 06 — F3: options and operands

/// atip.schemadoc.05: every option and operand has a `valueType`, a typed default and the members the contract names.
#[test]
fn atip_schemadoc_05_every_option_has_a_value_type_a_typed_default_and_no_id_suffix() {
    let kinds = ["switch", "count", "string", "integer", "number", "path", "enum", "json"];
    for doc in COMMANDS.iter() {
        let command = doc["command"].as_str().expect("command");
        for option in doc["options"].as_array().expect("options") {
            let name = option["name"].as_str().expect("name");
            assert!(!name.ends_with("-id"), "`{command}`: --{name} ends in -id");
            let value_type = option["valueType"].as_str().expect("valueType");
            assert!(kinds.contains(&value_type), "`{command}` --{name}: {value_type}");
            let default = &option["default"];
            match value_type {
                "switch" => assert_eq!(default, &json!(false), "`{command}` --{name}"),
                "count" => assert_eq!(default, &json!(0), "`{command}` --{name}"),
                "integer" => assert!(default.is_null() || default.is_i64() || default.as_array().is_some_and(|items| items.iter().all(Value::is_i64)), "`{command}` --{name}: {default}"),
                "string" | "path" | "enum" => assert!(default.is_null() || default.is_string() || default.is_array(), "`{command}` --{name}: {default}"),
                other => panic!("`{command}` --{name}: {other}"),
            }
            if option["required"] == true {
                assert!(default.is_null(), "a required option has no default: `{command}` --{name}");
            }
            assert_eq!(option.get("valueName").is_some(), !matches!(value_type, "switch" | "count") && name != "plan-digest", "`{command}` --{name}");
            assert_eq!(option.get("possibleValues").is_some(), value_type == "enum", "`{command}` --{name}");
            assert!(option["help"].as_str().is_some_and(|help| !help.is_empty()), "`{command}` --{name} has no help");
        }
        for operand in doc["operands"].as_array().expect("operands") {
            let name = operand["name"].as_str().expect("name");
            assert!(["string", "integer", "number", "path", "enum", "json"].contains(&operand["valueType"].as_str().expect("valueType")), "`{command}` <{name}>");
            assert!(operand["help"].as_str().is_some_and(|help| !help.is_empty()), "`{command}` <{name}> has no help");
            assert!(operand.get("variadic").is_some() && operand.get("default").is_some());
        }
    }
    let close = command_doc("finding close");
    assert_eq!(option(close, "round")["valueType"], "integer");
    assert_eq!(option(close, "seq")["valueType"], "integer");
    assert_eq!(option(command_doc("evidence add"), "max-artifact-mib")["default"], json!(25));
    for command in ["query", "finding list", "divergence list", "export"] {
        let limit = option(command_doc(command), "limit");
        assert_eq!((limit["default"].clone(), limit["valueType"].clone(), limit["valueName"].clone()), (json!(100), json!("integer"), json!("N")), "{command}");
    }
    assert_eq!(option(command_doc("retrieve"), "limit")["default"], json!(5));
    assert_eq!(option(command_doc("search"), "limit")["default"], json!(20));
    assert_eq!(option(command_doc("scenario new"), "actor")["env"], "CANON_ACTOR");
    assert_eq!(option(command_doc("feature new"), "actor")["env"], "CANON_ACTOR");
    for doc in COMMANDS.iter() {
        for name in ["json", "input", "commit", "plan-digest", "repo", "as"] {
            if let Some(option) = doc["options"].as_array().expect("options").iter().find(|option| option["name"] == name) {
                assert_eq!(option["flagOnly"], true, "`{}` --{name}", doc["command"]);
            }
        }
        assert_eq!(option(doc, "json")["valueType"], "switch");
        assert_eq!(option(doc, "as")["valueType"], "enum");
        assert!(doc["options"].as_array().expect("options").iter().all(|option| option["flagOnly"].is_boolean() || option.get("flagOnly").is_none()));
    }
    for old in ["kind", "aliases", "conflictsWith", "requires", "arity"] {
        assert!(COMMANDS.iter().all(|doc| doc["options"].as_array().expect("options").iter().all(|option| option.get(old).is_none())), "the old option member `{old}` is gone");
    }
    for doc in COMMANDS.iter() {
        for old in ["nodeKind", "about", "errorCodes", "exits", "supportsJson", "roles", "aliases", "name", "requiredOneOf"] {
            assert!(doc.get(old).is_none(), "`{}` still has the old member `{old}`", doc["command"]);
        }
    }
}

/// The value of a `[default: X]` that help prints for each option of `command`, by option name.
fn help_defaults(command: &str) -> BTreeMap<String, String> {
    let mut root = canon_cli::cli::command();
    root.build();
    let mut node = &root;
    for word in command.split(' ') {
        node = node.find_subcommand(word).unwrap_or_else(|| panic!("`{command}` is not a command"));
    }
    let help = node.clone().render_long_help().to_string();
    let mut found = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in help.lines() {
        let trimmed = line.trim_start();
        let is_option_line = line.starts_with("  ") && (trimmed.starts_with("--") || trimmed.chars().nth(1).is_some_and(|c| c.is_ascii_alphanumeric()) && trimmed.starts_with('-'));
        if is_option_line {
            current = trimmed.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).find(|word| word.starts_with("--")).map(|word| word.trim_start_matches("--").to_string());
        }
        if let (Some(name), Some(start)) = (&current, line.find("[default: ")) {
            let value = &line[start + "[default: ".len()..];
            found.insert(name.clone(), value[..value.find(']').unwrap_or(value.len())].trim_matches('"').to_string());
        }
    }
    found
}

/// atip.schemadoc.05 (F6, F3): every `[default: X]` that `--help` prints is the document's default, and the other
/// way round; a default the help text promises in words is a typed default or a `defaultNote`; one name is one
/// `valueName` and `valueType` on every command.
#[test]
fn atip_schemadoc_05_help_defaults_equal_the_documented_defaults_and_a_name_means_one_thing() {
    let mut problems = Vec::new();
    let mut shapes: BTreeMap<String, (String, String, String)> = BTreeMap::new();
    for doc in COMMANDS.iter() {
        let command = doc["command"].as_str().expect("command");
        let printed = help_defaults(command);
        for option in doc["options"].as_array().expect("options") {
            let name = option["name"].as_str().expect("name");
            let default = &option["default"];
            let shown = match default {
                Value::Null | Value::Bool(false) => None,
                Value::String(text) => Some(text.clone()),
                Value::Array(items) => Some(items.iter().map(|item| item.as_str().map_or_else(|| item.to_string(), str::to_string)).collect::<Vec<_>>().join(",")),
                other => Some(other.to_string()),
            };
            if shown != printed.get(name).cloned() {
                problems.push(format!("`{command}` --{name}: the document says {default}, --help prints {:?}", printed.get(name)));
            }
            let help = option["help"].as_str().expect("help").to_lowercase();
            let promises = help.contains("(default") || help.contains("defaults to") || help.contains("default is") || help.contains("default:") || help.contains("[default") || help.contains("omitted:") || help.contains("as for install") || help.contains("default to") || help.contains("falls back");
            if promises && default.is_null() && option.get("defaultNote").is_none() {
                problems.push(format!("`{command}` --{name} promises a default in its help and has neither a typed default nor a defaultNote: {help}"));
            }
            let shape = (option["valueName"].as_str().unwrap_or("").to_string(), option["valueType"].as_str().expect("valueType").to_string(), command.to_string());
            match shapes.get(name) {
                Some(first) if (first.0.as_str(), first.1.as_str()) != (shape.0.as_str(), shape.1.as_str()) => problems.push(format!("--{name} is `{}` {} on `{}` and `{}` {} on `{command}`", first.0, first.1, first.2, shape.0, shape.1)),
                Some(_) => {}
                None => {
                    shapes.insert(name.to_string(), shape);
                }
            }
        }
        for operand in doc["operands"].as_array().expect("operands") {
            let help = operand["help"].as_str().expect("help").to_lowercase();
            if (help.contains("(default") || help.contains("defaults to")) && operand["default"].is_null() && operand.get("defaultNote").is_none() {
                problems.push(format!("`{command}` <{}> promises a default in its help and has no defaultNote", operand["name"]));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// atip.schemadoc.06: a parser of canon's own with no entry in the table fails the generation, and one with an entry does not.
#[test]
fn atip_schemadoc_06_a_custom_parser_missing_from_the_table_fails_the_generation() {
    fn parse_thing(text: &str) -> Result<String, String> {
        Ok(text.to_string())
    }
    let mut cmd = clap::Command::new("canon").subcommand(clap::Command::new("probe").about("Probe").arg(clap::Arg::new("thing").long("thing").value_name("THING_ID").value_parser(parse_thing).help("A thing")));
    cmd.build();
    let arg = cmd.find_subcommand("probe").and_then(|probe| probe.get_arguments().find(|arg| arg.get_id() == "thing")).expect("the probe argument");
    let failure = atip::project::option_doc("probe", arg, canon_cli::spec::CUSTOM_PARSERS, canon_cli::input::flags_only()).expect_err("a parser with no table entry is refused");
    assert!(failure.to_string().contains("THING_ID"), "{failure}");
    let table: Vec<(&str, atip::doc::ValueType)> = canon_cli::spec::CUSTOM_PARSERS.iter().copied().chain([("THING_ID", atip::doc::ValueType::String)]).collect();
    assert_eq!(atip::project::option_doc("probe", arg, &table, canon_cli::input::flags_only()).expect("listed").value_type, atip::doc::ValueType::String);
    // Every value name in the table is used by a parser of canon's own: no entry stands for nothing.
    let used: BTreeSet<String> = {
        let mut root = canon_cli::cli::command();
        root.build();
        let mut used = BTreeSet::new();
        for node in atip::project::nodes(&root) {
            for arg in node.cmd.get_arguments().filter(|arg| !atip::project::is_builtin_arg(arg)) {
                if format!("{:?}", arg.get_value_parser()).starts_with("ValueParser::other") && arg.get_possible_values().is_empty() && arg.get_action().takes_values() {
                    used.insert(atip::project::value_name(arg));
                }
            }
        }
        used
    };
    for (name, _) in canon_cli::spec::CUSTOM_PARSERS {
        assert!(used.contains(*name), "the table lists `{name}` and no argument with a parser of canon's own has it");
    }
}

// ----------------------------------------------------------------------------------------------------------
// atip.schemadoc.07 / 08 — F4: errors, outputs, schemas

/// atip.schemadoc.07: every command document is a conforming `command` answer whose errors carry the reserved codes of
/// its features and whose outputs and schemas resolve; `brief approve` has three distinct outputs.
#[test]
fn atip_schemadoc_07_errors_outputs_and_schemas_hold_together_in_every_command_document() {
    let mut problems = Vec::new();
    for doc in COMMANDS.iter() {
        let envelope = json!({ "ok": true, "data": doc, "meta": { "tool": "canon", "toolVersion": "0.0.0", "contract": "atip/1", "contractVersion": "1.4", "schemaVersion": "1.4", "command": "schema" } });
        problems.extend(atip::schema_doc_problems(&envelope).into_iter().map(|problem| format!("`{}`: {problem}", doc["command"])));
        problems.extend(rule_problems(doc));
        for code in names(doc, "errors") {
            if canon_cli::cli_error::code_info(&code).is_none() {
                problems.push(format!("`{}`: the error `{code}` is not in the registry", doc["command"]));
            }
        }
        if doc["effect"] == "read" && doc.get("idempotency").is_some() {
            problems.push(format!("`{}`: a read has an idempotency", doc["command"]));
        }
        if doc["effect"] != "read" && doc.get("idempotency").is_none() {
            problems.push(format!("`{}`: a write has no idempotency", doc["command"]));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    let approve = command_doc("brief approve");
    let outputs = approve["outputs"].as_array().expect("outputs");
    let refs: BTreeSet<&str> = outputs.iter().map(|output| output["schemaRef"].as_str().expect("schemaRef")).collect();
    assert_eq!(outputs.len(), 3);
    assert_eq!(refs.len(), 3, "no two outputs share a schemaRef: {outputs:?}");
    assert_eq!(outputs.iter().filter(|output| output["batch"] == true).count(), 1);
    for reference in refs {
        assert!(approve["schemas"].get(ref_name(reference).expect("a #/schemas ref")).is_some(), "{reference}");
    }
    let init = command_doc("init");
    assert_eq!(init["outputs"].as_array().expect("outputs").iter().filter(|output| output["batch"] != true && output.get("when").is_some()).count(), 2);
    assert_eq!(command_doc("dashboard")["outputs"].as_array().expect("outputs").len(), 1, "dashboard declares one report until the stream protocol");
    assert_eq!(command_doc("dashboard")["outputs"][0]["kind"], "report");
    assert_eq!(command_doc("completion")["outputs"][0]["kind"], "artifact");
    assert!(command_doc("completion")["outputs"][0]["mediaType"].is_string());
    let list = command_doc("finding list");
    assert!(names(list, "errors").contains(&"CURSOR_INVALID".to_string()));
    for command in ["purge", "migrate", "ledger compact", "tier age", "artifacts move"] {
        assert!(names(command_doc(command), "errors").contains(&"PLAN_STALE".to_string()), "{command}");
    }
}

/// atip.schemadoc.08: a ref that does not resolve, a member that does not compile and a duplicated output ref are each found.
#[test]
fn atip_schemadoc_08_a_dangling_ref_and_a_schema_that_does_not_compile_are_found() {
    let good = command_doc("finding close").clone();
    assert_eq!(rule_problems(&good), Vec::<String>::new());
    let mut dangling = good.clone();
    dangling["outputs"][0]["schemaRef"] = json!("#/schemas/NoSuchReport");
    assert!(rule_problems(&dangling).iter().any(|problem| problem.contains("NoSuchReport")), "{:?}", rule_problems(&dangling));
    let mut broken = good.clone();
    broken["schemas"]["FindingCloseReport"] = json!({ "type": "nonsense" });
    assert!(rule_problems(&broken).iter().any(|problem| problem.contains("does not compile")), "{:?}", rule_problems(&broken));
    let mut shared = command_doc("brief approve").clone();
    shared["outputs"][1]["schemaRef"] = shared["outputs"][0]["schemaRef"].clone();
    assert!(rule_problems(&shared).iter().any(|problem| problem.contains("share a schemaRef")), "{:?}", rule_problems(&shared));
    let mut bare = good;
    bare["errors"] = json!(["USAGE", "INTERNAL"]);
    assert!(rule_problems(&bare).iter().any(|problem| problem.contains("INPUT_INVALID")), "{:?}", rule_problems(&bare));
}

// ----------------------------------------------------------------------------------------------------------
// atip.schemadoc.09 / 10 — F5: input and examples

/// atip.schemadoc.09: 47 commands carry `input`; each fragment has no `required` and no flag-only key, the example
/// passes it, and every example of every command parses with the real parser.
#[test]
fn atip_schemadoc_09_the_input_fragment_and_every_example_hold() {
    let with_input: Vec<&Value> = COMMANDS.iter().filter(|doc| doc["input"].is_object()).collect();
    assert_eq!(with_input.len(), 47);
    let mut problems = Vec::new();
    for doc in &with_input {
        problems.extend(example_problems(doc));
        let input = &doc["input"];
        assert_eq!(input["option"], "--input");
        assert_eq!(input["maxBytes"], json!(8_388_608));
        assert_eq!(input["batch"] == true, input.get("maxItems").is_some(), "{}", doc["command"]);
        let name = ref_name(input["schemaRef"].as_str().expect("schemaRef")).expect("a ref");
        assert!(doc["schemas"][name].get("required").is_none());
        assert!(input["argumentsRef"].is_string(), "the arguments schema is published: {}", doc["command"]);
    }
    // The 30 commands whose old document schema had `required` now publish it as the arguments schema.
    let with_required = with_input.iter().filter(|doc| ref_name(doc["input"]["argumentsRef"].as_str().expect("ref")).is_some_and(|name| doc["schemas"][name].get("required").is_some())).count();
    assert_eq!(with_required, 30);
    for doc in COMMANDS.iter() {
        let examples = names(doc, "examples");
        assert!(!examples.is_empty(), "`{}` has no example", doc["command"]);
        for example in examples {
            if let Some(problem) = parse_problem(&example) {
                problems.push(format!("`{}`: {problem}", doc["command"]));
            }
            let argv = atip::split_argv(&example);
            let command_words = names(doc, "path");
            if !argv[1..].starts_with(&command_words.iter().map(String::clone).collect::<Vec<_>>()) {
                problems.push(format!("`{}`: the example does not run the command: {example}", doc["command"]));
            }
            if argv.last().map(String::as_str) != Some("--json") {
                problems.push(format!("`{}`: the example does not end with --json after the path: {example}", doc["command"]));
            }
            if argv.iter().any(|word| word == "--input" || word.starts_with("--input=")) {
                problems.push(format!("`{}`: an example uses --input: {example}", doc["command"]));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// atip.schemadoc.10: an example that does not parse, one with a placeholder, a fragment with `required` and an
/// example the fragment refuses are each found.
#[test]
fn atip_schemadoc_10_an_unparsable_example_a_placeholder_and_a_bad_fragment_are_found() {
    assert!(parse_problem("canon finding close add-login --nosuch 1 --json").expect("an unknown option does not parse").contains("does not parse"));
    assert!(parse_problem("canon finding close <CHANGE> --round 1 --seq 1 --disposition fixed --json").expect("a placeholder").contains("placeholder"));
    assert!(parse_problem("canon finding close add-login ... --json").expect("an ellipsis").contains("placeholder"));
    assert!(parse_problem("tool finding close add-login --json").expect("another tool name").contains("tool name"));
    assert!(parse_problem("canon finding close add-login --round 1 --seq 1 --disposition fixed --json").is_none());
    let good = command_doc("finding close").clone();
    let mut required = good.clone();
    required["schemas"]["FindingCloseInput"]["required"] = json!(["round"]);
    assert!(rule_problems(&required).iter().any(|problem| problem.contains("root `required`")), "{:?}", rule_problems(&required));
    let mut open = good.clone();
    open["schemas"]["FindingCloseInput"]["additionalProperties"] = json!(true);
    assert!(rule_problems(&open).iter().any(|problem| problem.contains("open")));
    let mut flag = good.clone();
    flag["schemas"]["FindingCloseInput"]["properties"]["json"] = json!({ "type": "boolean" });
    assert!(rule_problems(&flag).iter().any(|problem| problem.contains("fragment's properties")));
    let mut wrong = good;
    wrong["input"]["example"] = json!({ "round": "one", "nosuch": true });
    assert!(!example_problems(&wrong).is_empty());
    let mut numeric = command_doc("finding close").clone();
    numeric["input"]["example"] = json!({ "operands": ["add-login"], "round": "1", "seq": 1, "disposition": "fixed" });
    assert_eq!(example_problems(&numeric), Vec::<String>::new(), "an integer is accepted as a number or a numeric string");
}

// ----------------------------------------------------------------------------------------------------------
// atip.schemadoc.11 / 12 — F6: the help line

fn first_line(args: &[&str]) -> String {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = run(args, dir.path(), None);
    String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or_default().to_string()
}

/// atip.schemadoc.11: the first line of the help of the root, a group and a command points at `schema`.
#[test]
fn atip_schemadoc_11_every_help_starts_with_the_line_that_points_at_schema() {
    let line = canon_cli::spec::HELP_POINTER;
    assert_eq!(line, "Agents: \"canon schema --json\" describes every command; \"canon skills show\" prints the guide.");
    for args in [&["--help"][..], &["finding", "--help"], &["finding", "close", "--help"], &["help", "finding"], &["finding", "close", "-h"], &["schema", "--help"]] {
        assert_eq!(first_line(args), line, "{args:?}");
    }
    let mut root = canon_cli::cli::command();
    root.build();
    for node in atip::project::nodes(&root) {
        assert!(node.cmd.clone().render_long_help().to_string().starts_with(line), "`{}`", node.command);
    }
    assert!(root.render_help().to_string().contains("Specs, evidence gates, and agent memory for your repo"), "the introduction stays below the line");
}

/// atip.schemadoc.12: a tree whose commands were not given the line is found by the same check.
#[test]
fn atip_schemadoc_12_a_command_without_the_line_is_found() {
    let bare = clap::Command::new("tool").about("A tool").subcommand(clap::Command::new("group").about("G").subcommand(clap::Command::new("leaf").about("L")));
    let missing: Vec<String> = atip::project::nodes(&bare).into_iter().filter(|node| !node.cmd.clone().render_long_help().to_string().starts_with("Agents:")).map(|node| node.command).collect();
    assert_eq!(missing, ["group", "group leaf"]);
    let pointed = atip::project::point_help_at_schema(bare, "Agents: \"tool schema --json\" describes every command as JSON.");
    assert!(atip::project::nodes(&pointed).iter().all(|node| node.cmd.clone().render_long_help().to_string().starts_with("Agents:")));
}

// ----------------------------------------------------------------------------------------------------------
// atip.schemadoc.13 / 14 — F7: the spec file and the consumers

fn spec_file() -> Value {
    serde_json::from_str(&std::fs::read_to_string(repo_root().join(canon_cli::spec::SPEC_PATH)).expect("the spec file")).expect("the spec file is JSON")
}

/// atip.schemadoc.13: the checked-in spec holds the tree and, for every command, the document `canon schema <command>
/// --json` answers, byte for byte once printed the same way.
#[test]
fn atip_schemadoc_13_the_spec_file_is_the_tree_and_the_answers_of_every_command() {
    let file = spec_file();
    let root = canon_cli::spec::built_root();
    let described = canon_cli::spec::described(&root, None).expect("described");
    assert_eq!(file["tree"], described.tree().to_value());
    assert_eq!(file["tree"]["level"], "L2");
    assert_eq!(file["contractVersion"], "1.4");
    // The file is the 1.4 form: the tree and the command documents hold no `path`, and the schemas live in their own map.
    let commands = file["commands"].as_array().expect("commands");
    assert_eq!(commands.len(), 78);
    let dir = tempfile::tempdir().expect("tempdir");
    let mut problems = form_problems(&file["tree"], commands, &file["schemas"]);
    for doc in commands {
        let command = doc["command"].as_str().expect("command").to_string();
        let path: Vec<&str> = command.split(' ').collect();
        let mut args: Vec<&str> = vec!["schema"];
        args.extend(path.iter().copied());
        args.push("--json");
        let out = run(&args, dir.path(), None);
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        let text = String::from_utf8(out.stdout).expect("text");
        let answer: Value = serde_json::from_str(&text).expect("JSON");
        assert_eq!(answer["data"], *doc, "`{command}`: the file and the answer differ");
        // The bytes of `data` in the answer are the bytes the serializer writes.
        let generated = described.command(&command).expect("a command document").to_line();
        let printed = &text[text.find("\"data\":").expect("data") + "\"data\":".len()..text.rfind(",\"meta\":").expect("meta")];
        assert_eq!(printed, generated, "`{command}`");
        // The `schemas` map of the file holds what `--schemas` answers.
        args.insert(args.len() - 1, "--schemas");
        let out = run(&args, dir.path(), None);
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        let answer: Value = serde_json::from_slice(&out.stdout).expect("JSON");
        assert_eq!(answer["data"], file["schemas"][&command], "`{command}`: the file's schemas and the --schemas answer differ");
        assert_eq!(answer["data"]["kind"], "schemas");
    }
    assert_eq!(file["schemas"].as_object().expect("schemas").len(), commands.len(), "the file's schemas map has one answer per command");
    assert!(problems.is_empty(), "{}", { problems.sort(); problems.join("\n") });
    assert_eq!(file["specVersion"], canon_cli::spec::SPEC_VERSION);
    assert_eq!(file["specVersion"], "3");
    assert!(file.get("globalOptions").is_none(), "the file has no top-level `globalOptions`");
}

/// The files whose text must not use a name the L1 documents dropped: guides, the website, the evaluation scripts and
/// the design notes that describe the spec. The CHANGELOG's replacement table is the one place the old names live.
fn consumer_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files = Vec::new();
    for (dir, extensions) in [("canon/skills", &["md"][..]), ("packages/website/src/content", &["md", "mdx"]), ("evals", &["py", "json", "md", "sh"]), ("docs/design", &["md"])] {
        let mut stack = vec![root.join(dir)];
        while let Some(current) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&current) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if path.file_name().is_some_and(|name| name != "__pycache__" && name != "node_modules") {
                        stack.push(path);
                    }
                } else if path.extension().is_some_and(|ext| extensions.contains(&ext.to_string_lossy().as_ref())) {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    files
}

/// The names of the old spec in `text`: members that cannot mean anything else, and the ambiguous ones when they are
/// written as a member (`"exits"`, a backticked `exits`).
fn old_names_in(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for name in ["nodeKind", "errorCodes", "supportsJson", "requiredOneOf", "conflictsWith"] {
        if text.contains(name) {
            found.push(name.to_string());
        }
    }
    for name in ["exits", "about", "inputs", "arity", "aliases", "roles"] {
        for written in [format!("`{name}`"), format!("[\"{name}\"]"), format!("['{name}']"), format!(".get(\"{name}\""), format!("\"{name}\":")] {
            if text.contains(&written) {
                found.push(written);
            }
        }
    }
    found
}

/// atip.schemadoc.13 (F7): no guide, website page, evaluation script or design note uses a name the old spec had.
#[test]
fn atip_schemadoc_13_no_consumer_uses_an_old_name() {
    let mut problems = Vec::new();
    for path in consumer_files() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for name in old_names_in(&text) {
            problems.push(format!("{}: {name}", path.strip_prefix(repo_root()).unwrap_or(&path).display()));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// atip.schemadoc.13 (F7): the sources of the workspace do not spell a member name the old spec had. (`canon context`'s
/// own report keeps `about` and `roles`: its `data` shape is not this change's; it reads the new documents.)
#[test]
fn atip_schemadoc_13_no_source_uses_an_old_name() {
    let mut problems = Vec::new();
    let mut stack: Vec<PathBuf> = std::fs::read_dir(repo_root().join("crates")).expect("crates").flatten().map(|entry| entry.path().join("src")).collect();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                for name in ["nodeKind", "errorCodes", "supportsJson", "requiredOneOf", "conflictsWith"] {
                    if text.contains(name) {
                        problems.push(format!("{}: {name}", path.strip_prefix(repo_root()).unwrap_or(&path).display()));
                    }
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// atip.schemadoc.14: an old name brought back in a consumer is found, and the new names are not mistaken for it.
#[test]
fn atip_schemadoc_14_a_revived_old_name_is_found() {
    for revived in ["every leaf has `nodeKind`", "read `errorCodes` from the schema", "`supportsJson` is true", "{\"exits\": []}", "see `exits` of the command", "a `requiredOneOf` group", "node[\"about\"]", "spec.get(\"inputs\")"] {
        assert!(!old_names_in(revived).is_empty(), "{revived}");
    }
    for current in ["`errors` lists every code", "the `summary` of a group", "`x-canon.roles`", "exit 1 means the command ran", "the `input` member and its `example`"] {
        assert_eq!(old_names_in(current), Vec::<String>::new(), "{current}");
    }
    assert!(!consumer_files().is_empty());
    assert!(consumer_files().iter().any(|path| path.ends_with("canon/skills/SKILL.src.md")));
    assert!(consumer_files().iter().any(|path| path.to_string_lossy().contains("website")));
    assert!(consumer_files().iter().any(|path| path.to_string_lossy().contains("evals")));
}
