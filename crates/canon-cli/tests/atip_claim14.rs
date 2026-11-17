//! `canon-0-36-claim-1-4` (scenarios `atip.claim14.*`): canon writes `canon schema` in the 1.4 form, which holds at 1.5 and 1.6 too (contract 15.1). The documents are read in process from the declared model (`canon_cli::spec`);
//! the built binary answers the calls that need a process: a bare destructive command, the refusals of `--schemas`, the compact envelopes.
//!
//! The 1.4 form (contract 5.1, 9, 15.7): the tree defines the shared options once
//! and carries no `path`, no `kind: "command"` and no member at its default; a command document is the call surface only (no `schemas`, `path`,
//! `input.schemaRef`, `input.option`, `rejects`, no implied option or code); `schema <COMMAND> --schemas` answers the schemas, the derived `Input` and the
//! `rejects`; every JSON document is compact.
//!
//! The checks that seeded faults must trip are small pure functions over text or `serde_json::Value` in this file (`form_problems`, `whitespace_problems`):
//! each real document passes them, each seeded fault is found, and no sweep is vacuous.
//! Tests are named by scenario id (`atip_claim14_05_…` proves `atip.claim14.05`).

#[path = "support/hermetic.rs"]
mod hermetic;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::LazyLock;

use serde_json::{json, Map, Value};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Running the binary.

fn canon(dir: &Path, args: &[&str]) -> Output {
    hermetic::canon().args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-claim14").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).output().expect("canon runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The one JSON document on stdout, with nothing on stderr.
fn json_of(output: &Output) -> Value {
    assert!(output.stderr.is_empty(), "stderr is empty in JSON mode: {}", stderr(output));
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}", stdout(output)))
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A git repo with canon's config, a subject and a change, to run the commands that need state in.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    let run = |program: &str, args: &[&str]| {
        let out = Command::new(program).current_dir(path).args(args).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-claim14").output().expect("spawn");
        assert!(out.status.success(), "{program} {args:?}: {}{}", stdout(&out), stderr(&out));
    };
    run("git", &["init", "-q"]);
    run(env!("CARGO_BIN_EXE_canon"), &["init", "--no-harness"]);
    run("git", &["add", "-A"]);
    run("git", &["-c", "user.name=canon", "-c", "user.email=canon@example.invalid", "commit", "-q", "-m", "fixture"]);
    run(env!("CARGO_BIN_EXE_canon"), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    run(env!("CARGO_BIN_EXE_canon"), &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    run(env!("CARGO_BIN_EXE_canon"), &["finding", "add", "--change", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--actor", "bob", "--summary", "Naming nit"]);
    dir
}

/// Every file of `root` by path and bytes, `.git` left out.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            if rel == ".git" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(rel, std::fs::read(&path).unwrap_or_default());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// One answer of `canon schema …`, read in process: the document the declared model holds, in an envelope with the members `atip::schema_doc_problems` reads, and the compact line the binary prints as its `data`.
struct Answer {
    value: Value,
    text: String,
}

/// The documents of the whole CLI, as the declared model builds them: the ones `canon schema` answers, without a process per command.
fn declared() -> atip::Described {
    canon_cli::spec::described(&canon_cli::spec::built_root(), None).expect("the documents are built from tables the tests keep whole")
}

fn answer_of(document: &atip::Document<'_>) -> Answer {
    let value = json!({ "ok": true, "data": document.to_value(), "meta": { "tool": "canon", "toolVersion": env!("CARGO_PKG_VERSION"), "contract": atip::CONTRACT, "contractVersion": canon_cli::json_out::DECLARED_MINOR, "schemaVersion": canon_cli::json_out::DECLARED_MINOR, "command": "schema" } });
    Answer { value, text: document.to_line() }
}

/// The tree, every group and every command document.
struct Surface {
    tree: Answer,
    groups: BTreeMap<String, Answer>,
    docs: BTreeMap<String, Answer>,
}

impl Surface {
    fn data(&self, command: &str) -> &Value {
        &self.docs[command].value["data"]
    }

    fn tree_options(&self) -> BTreeMap<String, &Value> {
        self.tree.value["data"]["options"].as_array().expect("the tree has options").iter().map(|option| (option["name"].as_str().expect("an option has a name").to_string(), option)).collect()
    }
}

fn entries(tree: &Value) -> &Vec<Value> {
    tree["data"]["commands"].as_array().expect("the tree lists commands")
}

static SURFACE: LazyLock<Surface> = LazyLock::new(|| {
    let described = declared();
    let tree = answer_of(&described.tree());
    let mut groups = BTreeMap::new();
    let mut docs = BTreeMap::new();
    for entry in entries(&tree.value) {
        let name = entry["command"].as_str().expect("an entry names its command").to_string();
        if entry["kind"] == "group" {
            let group = answer_of(&described.group(&name).expect("a group of the tree has its document"));
            groups.insert(name, group);
        } else {
            let doc = answer_of(&described.command(&name).expect("a command of the tree has its document"));
            docs.insert(name, doc);
        }
    }
    Surface { tree, groups, docs }
});

/// The `--schemas` answer of every command.
static SCHEMAS: LazyLock<BTreeMap<String, Answer>> = LazyLock::new(|| {
    let described = declared();
    SURFACE.docs.keys().map(|name| (name.clone(), answer_of(&described.schemas(name).expect("a command has its schemas answer")))).collect()
});

// ---------------------------------------------------------------------------
// The form check: a tree, a group or a command document in the 1.4 form (contract 9.3-9.5, Annex S2 rule 17).

/// The members an option, an operand or an output states only when they differ from their default (spec 9.5).
fn member_defaults(what: &str, member: &Map<String, Value>) -> Vec<String> {
    let mut found = Vec::new();
    for key in ["required", "repeatable", "variadic", "flagOnly", "async"] {
        if member.get(key) == Some(&Value::Bool(false)) {
            found.push(format!("{what}: `{key}` is false, its default"));
        }
    }
    let value_type = member.get("valueType").and_then(Value::as_str);
    match member.get("default") {
        Some(Value::Null) => found.push(format!("{what}: `default` is null, its default")),
        Some(Value::Bool(false)) if value_type == Some("switch") => found.push(format!("{what}: `default` is false on a switch, its default")),
        Some(value) if value_type == Some("count") && value == &json!(0) => found.push(format!("{what}: `default` is 0 on a count, its default")),
        _ => {}
    }
    if value_type == Some("string") {
        found.push(format!("{what}: `valueType` is string, its default"));
    }
    if value_type == Some("count") && member.contains_key("repeatable") {
        found.push(format!("{what}: a count is always repeatable and does not say so"));
    }
    if let (Some(name), Some(value_name)) = (member.get("name").and_then(Value::as_str), member.get("valueName").and_then(Value::as_str)) {
        if value_name == name.to_uppercase().replace('-', "_") {
            found.push(format!("{what}: `valueName` {value_name} is the name in upper case, its default"));
        }
    }
    for (key, value) in member {
        if value.as_array().is_some_and(Vec::is_empty) {
            found.push(format!("{what}: `{key}` is an empty list, its default"));
        }
    }
    found
}

fn entry_problems(entry: &Value) -> Vec<String> {
    let name = entry["command"].as_str().unwrap_or("?");
    let mut found = Vec::new();
    if entry.get("path").is_some() {
        found.push(format!("entry `{name}` has `path`"));
    }
    if entry["kind"] == "command" {
        found.push(format!("entry `{name}` has `kind: command`, its default"));
    }
    if entry.get("summary").is_none() {
        found.push(format!("entry `{name}` has no `summary`"));
    }
    if entry["kind"] != "group" && entry.get("effect").is_none() {
        found.push(format!("entry `{name}` is no group and has no `effect`"));
    }
    found
}

/// The name an entry of `options` holds: a bare name or an option object.
fn option_name(option: &Value) -> &str {
    option.as_str().or_else(|| option["name"].as_str()).unwrap_or("?")
}

fn error_code(error: &Value) -> &str {
    error.as_str().or_else(|| error["code"].as_str()).unwrap_or("?")
}

/// Every way the `data` of a `schema` answer (a tree, a group or a command document) is not in the 1.4 form; empty when it is.
fn form_problems(data: &Value) -> Vec<String> {
    let mut found = Vec::new();
    match data["kind"].as_str() {
        Some("tree" | "group") => {
            if data.get("path").is_some() {
                found.push("the document has `path`".to_string());
            }
            for entry in data["commands"].as_array().map_or_default(Vec::as_slice) {
                found.extend(entry_problems(entry));
            }
            for option in data["options"].as_array().map_or_default(Vec::as_slice) {
                match option.as_object() {
                    Some(object) => found.extend(member_defaults(&format!("tree option `{}`", option_name(option)), object)),
                    None => found.push(format!("a tree option is a bare name: {option}")),
                }
            }
        }
        Some("command") => found.extend(command_problems(data)),
        other => found.push(format!("`kind` {other:?} is no self-description")),
    }
    found
}

fn command_problems(doc: &Value) -> Vec<String> {
    let name = doc["command"].as_str().unwrap_or("?");
    let mut found = Vec::new();
    for key in ["schemas", "path"] {
        if doc.get(key).is_some() {
            found.push(format!("`{name}`: the command document has `{key}`"));
        }
    }
    for (index, validator) in doc["validators"].as_array().map_or_default(Vec::as_slice).iter().enumerate() {
        if validator.get("rejects").is_some() {
            found.push(format!("`{name}`: validator {index} has `rejects`"));
        }
    }
    let has_input = doc.get("input").is_some();
    for key in ["schemaRef", "option"] {
        if doc["input"].get(key).is_some() {
            found.push(format!("`{name}`: `input.{key}` is in the document"));
        }
    }
    if doc["input"]["batch"] == false {
        found.push(format!("`{name}`: `input.batch` is false, its default"));
    }
    let outputs = doc["outputs"].as_array().map_or_default(Vec::as_slice);
    let has_fields = outputs.iter().any(|output| output.get("fields").is_some());
    let mut names = BTreeSet::new();
    for option in doc["options"].as_array().map_or_default(Vec::as_slice) {
        let option_name = option_name(option);
        names.insert(option_name.to_string());
        if option_name == "json" || (option_name == "input" && has_input) || (option_name == "fields" && has_fields) {
            found.push(format!("`{name}`: the implied option `--{option_name}` is listed"));
        }
        if let Some(object) = option.as_object() {
            found.extend(member_defaults(&format!("`{name}` option `{option_name}`"), object));
        }
    }
    for operand in doc["operands"].as_array().map_or_default(Vec::as_slice) {
        if let Some(object) = operand.as_object() {
            found.extend(member_defaults(&format!("`{name}` operand `{}`", option_name_of_operand(operand)), object));
        }
    }
    for output in outputs {
        if output["kind"] == "report" {
            found.push(format!("`{name}`: an output says `kind: report`, its default"));
        }
        if output["schemaVersion"] == "1.0" {
            found.push(format!("`{name}`: an output says `schemaVersion: 1.0`, its default"));
        }
        if let Some(object) = output.as_object() {
            for (key, value) in object {
                if value.as_array().is_some_and(Vec::is_empty) {
                    found.push(format!("`{name}`: an output has `{key}`, an empty list"));
                }
            }
        }
    }
    let mut implied = vec!["USAGE", "INTERNAL"];
    if has_input {
        implied.extend(["INPUT_INVALID", "INPUT_CONFLICT", "INPUT_TOO_LARGE"]);
    }
    if doc["input"]["batch"] == true {
        implied.push("BATCH_INCOMPLETE");
    }
    for (option, code) in [("commit", "PLAN_STALE"), ("cursor", "CURSOR_INVALID"), ("operation", "OPERATION_REUSED")] {
        if names.contains(option) {
            implied.push(code);
        }
    }
    for error in doc["errors"].as_array().map_or_default(Vec::as_slice) {
        if implied.contains(&error_code(error)) {
            found.push(format!("`{name}`: the implied code {} is listed", error_code(error)));
        }
    }
    if let Some(object) = doc.as_object() {
        for (key, value) in object {
            if (value.as_array().is_some_and(Vec::is_empty)) || (key == "async" && value == &Value::Bool(false)) {
                found.push(format!("`{name}`: `{key}` is its default (an empty list or a false `async`)"));
            }
        }
    }
    if doc["effect"] == "destructive" && !(names.contains("commit") && names.contains("plan-digest")) {
        found.push(format!("`{name}`: a destructive command whose document lists no `commit` and `plan-digest`"));
    }
    found
}

fn option_name_of_operand(operand: &Value) -> &str {
    operand["name"].as_str().unwrap_or("?")
}

/// A clone of `base` with `key` set at the JSON pointer `at`.
fn seeded(base: &Value, at: &str, key: &str, value: Value) -> Value {
    let mut doc = base.clone();
    let target = if at.is_empty() { Some(&mut doc) } else { doc.pointer_mut(at) };
    target.unwrap_or_else(|| panic!("`{at}` is in the document")).as_object_mut().unwrap_or_else(|| panic!("`{at}` is an object")).insert(key.to_string(), value);
    doc
}

/// A clone of `base` with `value` appended to the list at `at`.
fn pushed(base: &Value, at: &str, value: Value) -> Value {
    let mut doc = base.clone();
    doc.pointer_mut(at).unwrap_or_else(|| panic!("`{at}` is in the document")).as_array_mut().unwrap_or_else(|| panic!("`{at}` is a list")).push(value);
    doc
}

fn assert_found(what: &str, problems: &[String], needle: &str) {
    assert!(problems.iter().any(|problem| problem.contains(needle)), "the form check finds {what} (a problem naming `{needle}`): {problems:?}");
}

// ---------------------------------------------------------------------------
// The whitespace check (spec 5.1).

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
    if in_string {
        found.push("a string is never closed".to_string());
    }
    found
}

/// The stdout of a call as one document's text: it must end with exactly one newline.
fn line_of(output: &Output) -> String {
    let text = stdout(output);
    text.strip_suffix('\n').unwrap_or_else(|| panic!("stdout ends with a newline: {text}")).to_string()
}

// ---------------------------------------------------------------------------
// The derived input schema (spec 9.9.2) and the refs of a command document.

const INTEGER_PATTERN: &str = "^-?(0|[1-9][0-9]*)$";
const NUMBER_PATTERN: &str = "^-?(0|[1-9][0-9]*)(\\.[0-9]+)?([eE][+-]?[0-9]+)?$";

/// What one `--input` value of an argument may be (spec 9.7.2).
fn encoded(value_type: &str, members: Option<&Value>) -> Value {
    match value_type {
        "switch" => json!({ "type": "boolean" }),
        "count" => json!({ "type": "integer", "minimum": 0 }),
        "string" | "path" => json!({ "type": "string" }),
        "integer" => json!({ "anyOf": [{ "type": "integer" }, { "type": "string", "pattern": INTEGER_PATTERN }] }),
        "number" => json!({ "anyOf": [{ "type": "number" }, { "type": "string", "pattern": NUMBER_PATTERN }] }),
        "enum" => json!({ "enum": members.cloned().unwrap_or_else(|| json!([])) }),
        "json" => json!({ "not": { "type": "null" } }),
        other => panic!("a valueType the contract does not have: {other}"),
    }
}

/// The camelCase name of an option (`plan-digest` is `planDigest`).
fn camel(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in name.chars() {
        if c == '-' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// The option an entry of a document's `options` stands for: itself, the tree's definition of its name, or a reserved option's fixed shape.
fn option_definition(entry: &Value, shared: &BTreeMap<String, &Value>) -> Value {
    if entry.is_object() {
        return entry.clone();
    }
    let name = entry.as_str().expect("an option is a name or an object");
    if let Some(shared) = shared.get(name) {
        return (*shared).clone();
    }
    match name {
        "dry-run" => json!({ "name": name, "valueType": "switch" }),
        "operation" => json!({ "name": name }),
        "json" | "input" | "commit" | "plan-digest" | "limit" | "cursor" | "all" | "fields" => json!({ "name": name, "flagOnly": true }),
        other => panic!("`--{other}` is neither a tree option nor a reserved option"),
    }
}

/// The schema of one `--input` document the contract derives from the operands and options (spec 9.9.2): an object, `additionalProperties` false, no
/// `required`, `operands` (an array, at least one, at most one per operand unless the last is variadic) and one property per option that is not flag-only.
fn derived_input(doc: &Value, shared: &BTreeMap<String, &Value>) -> Value {
    let mut properties = Map::new();
    let operands = doc["operands"].as_array().map_or_default(Vec::as_slice);
    if let Some(last) = operands.last() {
        let variadic = last["variadic"] == true;
        let of = |operand: &Value| encoded(operand["valueType"].as_str().unwrap_or("string"), operand.get("possibleValues"));
        let fixed = if variadic { &operands[..operands.len() - 1] } else { operands };
        let mut array = Map::new();
        array.insert("type".into(), json!("array"));
        array.insert("minItems".into(), json!(1));
        if !variadic {
            array.insert("maxItems".into(), json!(operands.len()));
        }
        if !fixed.is_empty() {
            array.insert("prefixItems".into(), Value::Array(fixed.iter().map(of).collect()));
        }
        array.insert("items".into(), if variadic { of(last) } else { Value::Bool(false) });
        properties.insert("operands".into(), Value::Object(array));
    }
    for entry in doc["options"].as_array().map_or_default(Vec::as_slice) {
        let option = option_definition(entry, shared);
        if option["flagOnly"] == true {
            continue;
        }
        let value_type = option["valueType"].as_str().unwrap_or("string");
        let value = encoded(value_type, option.get("possibleValues"));
        let property = if option["repeatable"] == true && value_type != "count" { json!({ "type": "array", "items": value }) } else { value };
        properties.insert(camel(option["name"].as_str().expect("an option has a name")), property);
    }
    json!({ "type": "object", "additionalProperties": false, "properties": properties })
}

/// The schema names a command document refers to: `schemaRef`, `eventRef` and `argumentsRef` of its outputs and input.
fn doc_refs(doc: &Value) -> BTreeSet<String> {
    fn walk(value: &Value, out: &mut BTreeSet<String>) {
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    if matches!(key.as_str(), "schemaRef" | "eventRef" | "argumentsRef") {
                        if let Some(name) = value.as_str().and_then(|reference| reference.strip_prefix("#/schemas/")) {
                            out.insert(name.to_string());
                        }
                    }
                    walk(value, out);
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, out);
                }
            }
            _ => {}
        }
    }
    let mut out = BTreeSet::new();
    walk(&doc["outputs"], &mut out);
    walk(&doc["input"], &mut out);
    out
}

/// The `#/schemas/<Name>` refs inside a schema.
fn schema_refs(schema: &Value, out: &mut BTreeSet<String>) {
    match schema {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "$ref" {
                    if let Some(name) = value.as_str().and_then(|reference| reference.strip_prefix("#/schemas/")) {
                        out.insert(name.to_string());
                    }
                }
                schema_refs(value, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                schema_refs(item, out);
            }
        }
        _ => {}
    }
}

/// Every problem of one `--schemas` answer against the document of its command: the exact set of schemas, `Input`, `rejects`, the compile of each
/// schema (draft 2020-12) and the resolution of each ref. `shared` are the tree's options.
fn schemas_problems(doc: &Value, answered: &Value, shared: &BTreeMap<String, &Value>) -> Vec<String> {
    let name = doc["command"].as_str().unwrap_or("?");
    let mut found = Vec::new();
    let data = &answered["data"];
    if data["kind"] != "schemas" || data["command"] != doc["command"] {
        found.push(format!("`{name}`: the answer is no `schemas` answer of its command: kind {} command {}", data["kind"], data["command"]));
    }
    let schemas = data["schemas"].as_object().cloned().unwrap_or_default();
    for key in data.as_object().map_or_default(|object| object.keys().cloned().collect::<Vec<_>>()) {
        if !["kind", "command", "schemas", "rejects"].contains(&key.as_str()) {
            found.push(format!("`{name}`: the answer has the member `{key}`"));
        }
    }
    // Exactly the schemas the refs name, and those they reach.
    let mut reached = doc_refs(doc);
    let mut queue: Vec<String> = reached.iter().cloned().collect();
    while let Some(next) = queue.pop() {
        let Some(schema) = schemas.get(&next) else {
            found.push(format!("`{name}`: the ref `#/schemas/{next}` does not resolve in the answer"));
            continue;
        };
        let mut inner = BTreeSet::new();
        schema_refs(schema, &mut inner);
        for reference in inner {
            if reached.insert(reference.clone()) {
                queue.push(reference);
            }
        }
    }
    let mut expected = reached;
    if doc.get("input").is_some() {
        expected.insert("Input".to_string());
    }
    let held: BTreeSet<String> = schemas.keys().cloned().collect();
    if held != expected {
        found.push(format!("`{name}`: the schemas are {held:?}, the refs reach {expected:?}"));
    }
    if doc.get("input").is_some() && schemas.get("Input") != Some(&derived_input(doc, shared)) {
        found.push(format!("`{name}`: `Input` is not the schema derived from the options: {} against {}", schemas.get("Input").unwrap_or(&Value::Null), derived_input(doc, shared)));
    }
    // `rejects`: one list per validator, in the document's order.
    let validators = doc["validators"].as_array().map_or(0, Vec::len);
    match data.get("rejects") {
        Some(Value::Array(lists)) if lists.len() == validators && lists.iter().all(Value::is_array) => {}
        None if validators == 0 => {}
        other => found.push(format!("`{name}`: `rejects` is {other:?} for {validators} validators")),
    }
    // Every ref of every schema resolves, and every schema compiles.
    let mut all = BTreeSet::new();
    for schema in schemas.values() {
        schema_refs(schema, &mut all);
    }
    for reference in all.iter().filter(|reference| !schemas.contains_key(*reference)) {
        found.push(format!("`{name}`: `#/schemas/{reference}` is referred to and not in the answer"));
    }
    found.extend(compile_problems(name, &schemas));
    found
}

fn compile_problems(name: &str, schemas: &Map<String, Value>) -> Vec<String> {
    use jsonschema::{Draft, Registry, Resource};
    let registry = Registry::new()
        .add("https://atip.dev/1/guide.schema.json", Resource::from_contents(serde_json::from_str(atip::GUIDE_SCHEMA).expect("the guide schema is JSON")))
        .expect("the contract schema registers")
        .add("urn:canon:doc", Resource::from_contents(json!({ "schemas": schemas })))
        .expect("the document registers")
        .prepare()
        .expect("the registry prepares");
    let mut found = Vec::new();
    for schema in schemas.keys() {
        if let Err(e) = jsonschema::options().with_draft(Draft::Draft202012).with_registry(&registry).build(&json!({ "$ref": format!("urn:canon:doc#/schemas/{schema}") })) {
            found.push(format!("`{name}`: the schema `{schema}` does not compile: {e}"));
        }
    }
    found
}

// ===========================================================================
// atip.claim14.02 (F1)

#[test]
fn atip_claim14_02_schema_answers_the_same_bytes_without_a_repository_and_the_form_check_finds_a_1_3_tree() {
    let one = TempDir::new().unwrap();
    let two = TempDir::new().unwrap();
    let bare = |dir: &Path, args: &[&str]| {
        let output = hermetic::canon().args(args).current_dir(dir).env_clear().stdin(Stdio::null()).output().expect("canon runs");
        assert!(output.status.success() && output.stderr.is_empty(), "canon {args:?} answers with an empty environment: {}{}", stdout(&output), stderr(&output));
        output.stdout
    };
    for args in [&["schema", "--json"][..], &["schema", "finding", "close", "--json"][..]] {
        let first = bare(one.path(), args);
        let second = bare(two.path(), args);
        assert!(!first.is_empty() && first == second, "canon {args:?} answers the same bytes from two directories without a repository and without an environment");
        let with_environment = canon(one.path(), args);
        assert_eq!(first, with_environment.stdout, "canon {args:?} answers the same bytes with the environment of a caller");
    }

    // The form check finds the 1.3 form: `kind: command`, `path`, and members at their default.
    let tree = &SURFACE.tree.value["data"];
    assert!(form_problems(tree).is_empty(), "the real tree is in the 1.4 form: {:?}", form_problems(tree));
    let command_entry = entries(&SURFACE.tree.value).iter().position(|entry| entry["kind"] != "group").expect("the tree has a command");
    let at = format!("/commands/{command_entry}");
    assert_found("`kind: command` on an entry", &form_problems(&seeded(tree, &at, "kind", json!("command"))), "kind: command");
    assert_found("`path` on an entry", &form_problems(&seeded(tree, &at, "path", json!(["x"]))), "`path`");
    let seeds = [("required", json!(false)), ("repeatable", json!(false)), ("variadic", json!(false)), ("flagOnly", json!(false)), ("default", Value::Null), ("valueType", json!("string")), ("valueName", json!("REPO"))];
    for (key, value) in seeds {
        // `repo` is the first shared option: a string-typed path, flag-only; each seed is a default for it (valueName REPO is its name in upper case).
        let options = tree["options"].as_array().unwrap();
        let index = options.iter().position(|option| option["name"] == "repo").expect("the tree shares `repo`");
        let problems = form_problems(&seeded(tree, &format!("/options/{index}"), key, value));
        assert_found(&format!("`{key}` at its default in a tree option"), &problems, key);
    }
    assert_found("a bare-name tree option", &form_problems(&pushed(tree, "/options", json!("repo"))), "bare name");
}

// ===========================================================================
// atip.claim14.03 - 06 (F2)

const DESTRUCTIVE: [&str; 6] = ["artifacts move", "artifacts prune", "ledger compact", "migrate", "purge", "tier age"];

fn option_names(doc: &Value) -> Vec<String> {
    doc["options"].as_array().map_or_default(Vec::as_slice).iter().map(|option| option_name(option).to_string()).collect()
}

#[test]
fn atip_claim14_03_a_command_document_carries_the_call_surface_only() {
    let surface = &*SURFACE;
    let close = surface.data("finding close");
    assert!(close.get("input").is_some(), "finding close has `input`, so the implied `--input` and the `INPUT_*` codes apply");
    for key in ["schemas", "path", "rejects"] {
        assert!(close.get(key).is_none(), "the document of finding close has no `{key}`");
    }
    assert!(close["input"].get("schemaRef").is_none() && close["input"].get("option").is_none(), "no `input.schemaRef` and no `input.option`: {}", close["input"]);
    let names = option_names(close);
    assert!(!names.is_empty(), "finding close lists its options");
    for implied in ["json", "input", "fields"] {
        assert!(!names.contains(&implied.to_string()), "the implied `--{implied}` is not listed: {names:?}");
    }
    let codes: Vec<&str> = close["errors"].as_array().expect("finding close lists errors").iter().map(error_code).collect();
    assert!(!codes.is_empty() && codes.iter().all(|code| !matches!(*code, "USAGE" | "INTERNAL") && !code.starts_with("INPUT_")), "no implied code is listed: {codes:?}");
    assert_eq!(form_problems(close), Vec::<String>::new(), "finding close is in the 1.4 form");

    // The six destructive commands, and only they, list `commit` and `plan-digest` by name.
    let destructive: Vec<&str> = surface.docs.iter().filter(|(_, doc)| doc.value["data"]["effect"] == "destructive").map(|(name, _)| name.as_str()).collect();
    assert_eq!(destructive, DESTRUCTIVE, "the destructive commands");
    for name in DESTRUCTIVE {
        let options = surface.data(name)["options"].as_array().unwrap();
        for reserved in ["commit", "plan-digest"] {
            assert!(options.iter().any(|option| option.as_str() == Some(reserved)), "`{name}` lists `{reserved}` by name: {options:?}");
        }
    }
    for (name, doc) in &SURFACE.docs {
        if !DESTRUCTIVE.contains(&name.as_str()) {
            assert!(!option_names(&doc.value["data"]).contains(&"commit".to_string()), "`{name}` offers `--commit` without a destructive effect");
        }
    }

    // Every document: no `schemas`, `path`, `rejects`, and the options the commands share are defined once and used by two or more.
    assert!(!surface.docs.is_empty(), "the sweep covered the commands");
    let with_validators = surface.docs.values().filter(|doc| doc.value["data"]["validators"].as_array().is_some_and(|list| !list.is_empty())).count();
    assert!(with_validators > 0, "the sweep saw validators");
    let mut uses: BTreeMap<String, usize> = BTreeMap::new();
    for (name, doc) in &surface.docs {
        let data = &doc.value["data"];
        assert!(data.get("schemas").is_none() && data.get("path").is_none() && data.get("rejects").is_none(), "`{name}` has no `schemas`, `path` or `rejects`");
        for option in data["options"].as_array().map_or_default(Vec::as_slice) {
            if let Some(shared) = option.as_str() {
                *uses.entry(shared.to_string()).or_default() += 1;
            }
        }
    }
    let shared = surface.tree_options();
    assert!(!shared.is_empty(), "the tree shares options");
    for name in shared.keys() {
        assert!(uses.get(name).copied().unwrap_or(0) >= 2, "the tree option `{name}` is named by two or more commands, not defined for one: {:?}", uses.get(name));
    }
}

#[test]
fn atip_claim14_04_the_form_check_finds_an_implied_option_an_implied_code_or_a_default_member() {
    let surface = &*SURFACE;
    // No real document is in the 1.4 form wrongly.
    let mut real = Vec::new();
    for (name, doc) in &surface.docs {
        real.extend(form_problems(&doc.value["data"]));
        let contract = atip::schema_doc_problems(&doc.value);
        assert!(contract.is_empty(), "`{name}` passes the contract's schema-doc schema: {contract:?}");
    }
    for (name, group) in &surface.groups {
        real.extend(form_problems(&group.value["data"]).into_iter().map(|problem| format!("group `{name}`: {problem}")));
    }
    assert!(!surface.docs.is_empty() && !surface.groups.is_empty(), "the sweep covered the commands and the groups");
    assert!(real.is_empty(), "the real documents are in the 1.4 form: {real:#?}");

    let close = surface.data("finding close");
    let purge = surface.data("purge");
    let with_fields = surface.docs.values().map(|doc| &doc.value["data"]).find(|doc| doc["outputs"].as_array().is_some_and(|outputs| outputs.iter().any(|output| output.get("fields").is_some()))).expect("a command declares selectable fields");
    let first_option = close["options"].as_array().unwrap().iter().position(Value::is_object).expect("finding close has an option object");
    let option = format!("/options/{first_option}");

    let faults: Vec<(&str, Value, &str)> = vec![
        ("the implied option --json", pushed(close, "/options", json!("json")), "implied option `--json`"),
        ("the implied option --input", pushed(close, "/options", json!("input")), "implied option `--input`"),
        ("the implied option --input as an object", pushed(close, "/options", json!({ "name": "input", "flagOnly": true })), "implied option `--input`"),
        ("the implied option --fields", pushed(with_fields, "/options", json!("fields")), "implied option `--fields`"),
        ("the implied code USAGE", pushed(close, "/errors", json!("USAGE")), "implied code USAGE"),
        ("the implied code INTERNAL", pushed(close, "/errors", json!("INTERNAL")), "implied code INTERNAL"),
        ("the implied code INPUT_INVALID", pushed(close, "/errors", json!("INPUT_INVALID")), "implied code INPUT_INVALID"),
        ("the implied code INPUT_TOO_LARGE", pushed(close, "/errors", json!("INPUT_TOO_LARGE")), "implied code INPUT_TOO_LARGE"),
        ("the implied code BATCH_INCOMPLETE", pushed(close, "/errors", json!("BATCH_INCOMPLETE")), "implied code BATCH_INCOMPLETE"),
        ("the implied code PLAN_STALE with --commit", pushed(purge, "/errors", json!("PLAN_STALE")), "implied code PLAN_STALE"),
        ("`required: false` on an option", seeded(close, &option, "required", json!(false)), "`required` is false"),
        ("`repeatable: false` on an option", seeded(close, &option, "repeatable", json!(false)), "`repeatable` is false"),
        ("`flagOnly: false` on an option", seeded(close, &option, "flagOnly", json!(false)), "`flagOnly` is false"),
        ("`default: null` on an option", seeded(close, &option, "default", Value::Null), "`default` is null"),
        ("`valueType: string` on an option", seeded(close, &option, "valueType", json!("string")), "`valueType` is string"),
        ("`valueName` as the upper-cased name", seeded(close, &option, "valueName", json!(close["options"][first_option]["name"].as_str().unwrap().to_uppercase())), "`valueName`"),
        ("`variadic: false` on an operand", seeded(close, "/operands/0", "variadic", json!(false)), "`variadic` is false"),
        ("`kind: report` on an output", seeded(close, "/outputs/0", "kind", json!("report")), "`kind: report`"),
        ("`schemaVersion: 1.0` on an output", seeded(close, "/outputs/0", "schemaVersion", json!("1.0")), "`schemaVersion: 1.0`"),
        ("`input.batch: false`", seeded(close, "/input", "batch", json!(false)), "`input.batch` is false"),
        ("an empty list", seeded(close, "", "constraints", json!([])), "`constraints`"),
        ("`async: false`", seeded(close, "", "async", json!(false)), "`async`"),
        ("`schemas` in the document", seeded(close, "", "schemas", json!({})), "has `schemas`"),
        ("`path` in the document", seeded(close, "", "path", json!(["finding", "close"])), "has `path`"),
        ("`input.schemaRef`", seeded(close, "/input", "schemaRef", json!("#/schemas/FindingCloseArguments")), "`input.schemaRef`"),
        ("`input.option`", seeded(close, "/input", "option", json!("input")), "`input.option`"),
        ("`rejects` in a validator", seeded(purge, "/validators/0", "rejects", json!([])), "has `rejects`"),
    ];
    for (what, document, needle) in &faults {
        assert_found(what, &form_problems(document), needle);
    }
}

/// The previews the fixture of the checker runs: the first bare call of each destructive command.
fn preview_argvs() -> BTreeMap<String, Vec<String>> {
    let text = std::fs::read_to_string(root().join("crates/canon-cli/tests/atip/canon.fixture.json")).expect("the fixture is there");
    let fixture: Value = serde_json::from_str(&text).expect("the fixture is JSON");
    DESTRUCTIVE
        .iter()
        .map(|name| {
            let argv: Vec<String> = fixture["commands"][name]["preview"][0].as_array().unwrap_or_else(|| panic!("the fixture has a preview of `{name}`")).iter().map(|word| word.as_str().unwrap().to_string()).collect();
            (name.to_string(), argv)
        })
        .collect()
}

/// `a` against `b`: the paths that differ.
fn differing(a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    a.keys().chain(b.keys()).collect::<BTreeSet<_>>().into_iter().filter(|path| a.get(*path) != b.get(*path)).cloned().collect()
}

#[test]
fn atip_claim14_05_a_bare_destructive_command_is_a_preview_that_writes_nothing() {
    let dir = repo();
    let at = dir.path();
    for (name, argv) in preview_argvs() {
        let options = SURFACE.data(&name)["options"].as_array().unwrap();
        assert!(options.iter().any(|option| option.as_str() == Some("commit")), "the document of `{name}` lists `commit`: {options:?}");
        let before = tree(at);
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();
        let output = canon(at, &args);
        let result = json_of(&output);
        assert!(output.status.success() && result["ok"] == true, "the bare `canon {name}` runs: {result}");
        assert_eq!(result["data"]["committed"], false, "the bare `canon {name}` says it did not commit: {}", result["data"]);
        assert!(result["data"]["planDigest"].as_str().is_some_and(|digest| digest.starts_with("sha256:")), "the bare `canon {name}` names the plan to commit: {}", result["data"]);
        let after = tree(at);
        assert!(!before.is_empty(), "the repository has files");
        assert_eq!(differing(&before, &after), Vec::<String>::new(), "the bare `canon {name}` writes nothing");
    }
}

#[test]
fn atip_claim14_06_commit_without_plan_digest_is_refused_and_a_destructive_document_needs_commit() {
    let dir = repo();
    let at = dir.path();
    let digest = format!("sha256:{}", "0".repeat(64));
    for (_command, argv) in preview_argvs() {
        let base: Vec<String> = argv.into_iter().filter(|word| word != "--json").collect();
        let cases = [(vec!["--commit".to_string()], "--plan-digest"), (vec!["--plan-digest".to_string(), digest.clone()], "--commit")];
        for (extra, field) in cases {
            let mut args: Vec<&str> = base.iter().map(String::as_str).collect();
            args.extend(extra.iter().map(String::as_str));
            args.push("--json");
            let before = tree(at);
            let output = canon(at, &args);
            let result = json_of(&output);
            assert_eq!(output.status.code(), Some(2), "`canon {args:?}` is refused: {result}");
            assert_eq!(result["ok"], false);
            assert_eq!(result["error"]["field"], field, "`canon {args:?}` names the missing `{field}`: {result}");
            assert_eq!(differing(&before, &tree(at)), Vec::<String>::new(), "`canon {args:?}` writes nothing");
        }
    }
    // The form check: a destructive effect without `commit`.
    let purge = SURFACE.data("purge");
    assert!(form_problems(purge).is_empty(), "the real document of purge is in the form");
    let without = |name: &str| {
        let mut doc = purge.clone();
        doc["options"].as_array_mut().unwrap().retain(|option| option_name(option) != name);
        doc
    };
    assert_found("a destructive document without `commit`", &form_problems(&without("commit")), "no `commit`");
    assert_found("a destructive document without `plan-digest`", &form_problems(&without("plan-digest")), "no `commit`");
    let mut read = without("commit");
    read["effect"] = json!("read");
    assert!(!form_problems(&read).iter().any(|problem| problem.contains("destructive")), "a read command needs no `commit`");
}


// ===========================================================================
// atip.claim14.07 - 08 (F3)

#[test]
fn atip_claim14_07_schemas_answers_exactly_the_referenced_schemas_the_derived_input_and_rejects() {
    let surface = &*SURFACE;
    let shared = surface.tree_options();
    let close = surface.data("finding close");
    let answered = &SCHEMAS["finding close"].value;
    assert_eq!(schemas_problems(close, answered, &shared), Vec::<String>::new(), "the answer of finding close");
    let schemas = answered["data"]["schemas"].as_object().unwrap();
    assert!(schemas.contains_key("Input") && schemas.contains_key("FindingCloseReport"), "finding close answers its schemas and `Input`: {:?}", schemas.keys());
    assert_eq!(answered["data"]["kind"], "schemas");
    assert_eq!(answered["data"]["command"], "finding close");

    // The same holds for every command of the tree.
    let mut problems = Vec::new();
    let (mut with_input, mut with_rejects) = (0, 0);
    for (name, got) in SCHEMAS.iter() {
        let doc = surface.data(name);
        problems.extend(schemas_problems(doc, &got.value, &shared));
        with_input += usize::from(doc.get("input").is_some());
        with_rejects += usize::from(got.value["data"]["rejects"].as_array().is_some_and(|lists| !lists.is_empty()));
    }
    assert!(!SCHEMAS.is_empty(), "the sweep covered the commands");
    assert!(with_input > 0 && with_rejects > 0, "the sweep saw commands with `input` ({with_input}) and with validators ({with_rejects})");
    assert!(problems.is_empty(), "the `--schemas` answers: {problems:#?}");

    // The check can fail: each seeded fault is found.
    let purge = surface.data("purge");
    let purge_answer = &SCHEMAS["purge"].value;
    assert_eq!(schemas_problems(purge, purge_answer, &shared), Vec::<String>::new());
    assert_found("an extra schema", &schemas_problems(close, &seeded(answered, "/data/schemas", "Extra", json!({ "type": "string" })), &shared), "the schemas are");
    assert_found("a changed `Input`", &schemas_problems(close, &seeded(answered, "/data/schemas/Input", "additionalProperties", json!(true)), &shared), "`Input`");
    assert_found("`rejects` with a list too few", &schemas_problems(purge, &seeded(purge_answer, "/data", "rejects", json!([])), &shared), "`rejects`");
    assert_found("a ref that does not resolve", &schemas_problems(close, &seeded(answered, "/data/schemas/FindingCloseReport", "$ref", json!("#/schemas/Nope")), &shared), "Nope");
    assert_found("a schema that does not compile", &schemas_problems(close, &seeded(answered, "/data/schemas", "FindingCloseReport", json!({ "type": "nosuchtype" })), &shared), "does not compile");
}

#[test]
fn atip_claim14_08_schemas_is_refused_without_a_command_path_and_with_the_registries() {
    let dir = TempDir::new().unwrap();
    let at = dir.path();
    let usage: [&[&str]; 8] = [
        &["schema", "--schemas", "--json"],
        &["schema", "finding", "--schemas", "--json"],
        &["schema", "finding", "close", "--schemas", "--errors", "--json"],
        &["schema", "finding", "close", "--schemas", "--error", "USAGE", "--json"],
        &["schema", "finding", "close", "--schemas", "--warnings", "--json"],
        &["schema", "finding", "close", "--schemas", "--warning", "STOPPED_MIDWAY", "--json"],
        &["schema", "--schemas", "--errors", "--json"],
        &["schema", "--schemas", "--warning", "STOPPED_MIDWAY", "--json"],
    ];
    for args in usage {
        let output = canon(at, args);
        let result = json_of(&output);
        assert_eq!(output.status.code(), Some(2), "canon {args:?} is a refusal: {result}");
        assert_eq!(result["error"]["code"], "USAGE", "canon {args:?}: {result}");
        assert_eq!(result["error"]["field"], "--schemas", "canon {args:?} names `--schemas`: {result}");
        assert!(result.get("data").is_none(), "a refusal carries no schemas: canon {args:?}");
    }
    for args in [&["schema", "nosuch", "--schemas", "--json"][..], &["schema", "finding", "nosuch", "--schemas", "--json"][..]] {
        let output = canon(at, args);
        let result = json_of(&output);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(result["error"]["code"], "UNKNOWN_COMMAND", "canon {args:?}: {result}");
    }
}

// ===========================================================================
// atip.claim14.09 - 10 (F4)

#[test]
fn atip_claim14_09_no_schema_answer_and_no_envelope_has_whitespace_outside_a_string() {
    let surface = &*SURFACE;
    let mut texts: Vec<(String, &str)> = vec![("schema --json".to_string(), surface.tree.text.as_str())];
    texts.extend(surface.groups.iter().map(|(name, got)| (format!("schema {name} --json"), got.text.as_str())));
    texts.extend(surface.docs.iter().map(|(name, got)| (format!("schema {name} --json"), got.text.as_str())));
    texts.extend(SCHEMAS.iter().map(|(name, got)| (format!("schema {name} --schemas --json"), got.text.as_str())));
    assert!(!surface.docs.is_empty() && !surface.groups.is_empty() && !SCHEMAS.is_empty(), "the sweep covered the tree, the groups and both answers of the commands");
    let mut found = Vec::new();
    for (what, text) in &texts {
        assert!(!text.is_empty() && text.starts_with('{') && text.ends_with('}'), "`{what}` is one document");
        found.extend(whitespace_problems(text).into_iter().map(|problem| format!("{what}: {problem}")));
    }
    assert!(found.is_empty(), "whitespace outside strings: {found:#?}");

    // The envelopes of a success, a refusal and a batch.
    let dir = repo();
    let at = dir.path();
    let batch = r#"[{"operands":["add-login"],"round":1,"seq":1,"disposition":"rejected","reason":"works as designed"},{"operands":["add-login"],"round":1,"seq":9,"disposition":"rejected","reason":"no such finding"}]"#;
    let calls: [(&str, Vec<&str>); 5] = [
        ("a success", vec!["finding", "list", "--change", "add-login", "--json"]),
        ("a refusal", vec!["finding", "list", "--change", "NOT A SLUG", "--json"]),
        ("an unknown command", vec!["nosuchcommand", "--json"]),
        ("a batch", vec!["finding", "close", "--input", batch, "--json"]),
        ("a status", vec!["status", "--json"]),
    ];
    for (what, args) in calls {
        let output = canon(at, &args);
        assert!(output.stderr.is_empty(), "{what}: stderr is empty");
        let text = line_of(&output);
        let document: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{what} is one document ({e}): {text}"));
        assert_eq!(whitespace_problems(&text), Vec::<String>::new(), "{what} (`canon {args:?}`) has no whitespace outside a string");
        match what {
            "a success" => assert_eq!(document["ok"], true, "{text}"),
            "a refusal" | "an unknown command" => assert_eq!(document["ok"], false, "{text}"),
            "a batch" => assert!(document["data"]["items"].as_array().is_some_and(|items| items.len() == 2), "the batch envelope reports its items: {text}"),
            _ => {}
        }
    }
}

#[test]
fn atip_claim14_10_the_whitespace_check_finds_formatted_documents_and_accepts_strings() {
    let value = json!({ "ok": true, "data": { "items": [1, 2], "text": "a b" } });
    let pretty = serde_json::to_string_pretty(&value).unwrap();
    assert_found("a pretty-printed document", &whitespace_problems(&pretty), "outside a string");
    assert_found("a space after a colon", &whitespace_problems(r#"{"a": 1}"#), "' '");
    assert_found("a space after a comma", &whitespace_problems(r#"{"a":1, "b":2}"#), "' '");
    assert_found("a trailing space", &whitespace_problems("{\"a\":1} "), "' '");
    assert_found("a newline inside the document", &whitespace_problems("{\"a\":1,\n\"b\":2}"), "'\\n'");
    assert_found("a tab", &whitespace_problems("{\"a\":\t1}"), "'\\t'");
    assert_found("a carriage return", &whitespace_problems("{\"a\":1}\r"), "'\\r'");
    assert_found("a string that never closes", &whitespace_problems("{\"a\":\"b}"), "never closed");
    // Whitespace inside a string, an escaped quote and an escaped backslash are no findings.
    assert_eq!(whitespace_problems(r#"{"a":"x y\tz\n","b":"q\" w","c":"\\ ","d":"\u0020 "}"#), Vec::<String>::new());
    assert_eq!(whitespace_problems(&serde_json::to_string(&value).unwrap()), Vec::<String>::new());
    assert_eq!(whitespace_problems(""), Vec::<String>::new());
}
