//! `canon-0-36-claim-1-4` (scenarios `atip.claim14.01-16`): canon declares atip/1 1.4 and writes `canon schema` in the 1.4 form, against the built binary.
//!
//! The 1.4 form (contract 5.1, 9, 15.7): every envelope says `contractVersion` `1.4` and the tree `level` `L2`; the tree defines the shared options once
//! and carries no `path`, no `kind: "command"` and no member at its default; a command document is the call surface only (no `schemas`, `path`,
//! `input.schemaRef`, `input.option`, `rejects`, no implied option or code); `schema <COMMAND> --schemas` answers the schemas, the derived `Input` and the
//! `rejects`; every JSON document is compact. The consumers (guides, website, evals, conformance, generated spec) and the CI configuration say 1.4, and the
//! CHANGELOG entry states the measured sizes.
//!
//! The checks that seeded faults must trip are small pure functions over text or `serde_json::Value` in this file (`form_problems`, `whitespace_problems`,
//! `consumer_problems`, `minor_problems`, `changelog_problems`): each real document passes them, each seeded fault is found, and no sweep is vacuous.
//! Tests are named by scenario id (`atip_claim14_05_…` proves `atip.claim14.05`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

use serde_json::{json, Map, Value};
use tempfile::TempDir;

const MINOR: &str = "1.4";

// ---------------------------------------------------------------------------
// Running the binary.

fn canon(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-claim14").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).output().expect("canon runs")
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
    run(env!("CARGO_BIN_EXE_canon"), &["init", "--providers", "none"]);
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

fn words(path: &str) -> Vec<String> {
    path.split(' ').map(str::to_string).collect()
}

fn schema_argv(path: &str, extra: &[&str]) -> Vec<String> {
    let mut argv = vec!["schema".to_string()];
    argv.extend(words(path).into_iter().filter(|word| !word.is_empty()));
    argv.extend(extra.iter().map(ToString::to_string));
    argv.push("--json".to_string());
    argv
}

/// A directory that is no repository: the cwd of every call that must not need one.
static NEUTRAL: LazyLock<TempDir> = LazyLock::new(|| TempDir::new().unwrap());

/// The tree, every group and every command document, answered by the binary.
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
    let tree = answer(NEUTRAL.path(), &schema_argv("", &[]));
    let mut groups = Vec::new();
    let mut commands = Vec::new();
    for entry in entries(&tree.value) {
        let name = entry["command"].as_str().expect("an entry names its command").to_string();
        if entry["kind"] == "group" {
            groups.push(name);
        } else {
            commands.push(name);
        }
    }
    let argvs: Vec<Vec<String>> = groups.iter().chain(commands.iter()).map(|name| schema_argv(name, &[])).collect();
    let mut got = answers(NEUTRAL.path(), &argvs).into_iter();
    let groups = groups.into_iter().map(|name| (name, got.next().unwrap())).collect();
    let docs = commands.into_iter().map(|name| (name, got.next().unwrap())).collect();
    Surface { tree, groups, docs }
});

/// The `--schemas` answer of every command.
static SCHEMAS: LazyLock<BTreeMap<String, Answer>> = LazyLock::new(|| {
    let commands: Vec<String> = SURFACE.docs.keys().cloned().collect();
    let argvs: Vec<Vec<String>> = commands.iter().map(|name| schema_argv(name, &["--schemas"])).collect();
    commands.into_iter().zip(answers(NEUTRAL.path(), &argvs)).collect()
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
            for entry in data["commands"].as_array().map(Vec::as_slice).unwrap_or_default() {
                found.extend(entry_problems(entry));
            }
            for option in data["options"].as_array().map(Vec::as_slice).unwrap_or_default() {
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
    for (index, validator) in doc["validators"].as_array().map(Vec::as_slice).unwrap_or_default().iter().enumerate() {
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
    let outputs = doc["outputs"].as_array().map(Vec::as_slice).unwrap_or_default();
    let has_fields = outputs.iter().any(|output| output.get("fields").is_some());
    let mut names = BTreeSet::new();
    for option in doc["options"].as_array().map(Vec::as_slice).unwrap_or_default() {
        let option_name = option_name(option);
        names.insert(option_name.to_string());
        if option_name == "json" || (option_name == "input" && has_input) || (option_name == "fields" && has_fields) {
            found.push(format!("`{name}`: the implied option `--{option_name}` is listed"));
        }
        if let Some(object) = option.as_object() {
            found.extend(member_defaults(&format!("`{name}` option `{option_name}`"), object));
        }
    }
    for operand in doc["operands"].as_array().map(Vec::as_slice).unwrap_or_default() {
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
    for error in doc["errors"].as_array().map(Vec::as_slice).unwrap_or_default() {
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
    let operands = doc["operands"].as_array().map(Vec::as_slice).unwrap_or_default();
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
    for entry in doc["options"].as_array().map(Vec::as_slice).unwrap_or_default() {
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
    for key in data.as_object().map(|object| object.keys().cloned().collect::<Vec<_>>()).unwrap_or_default() {
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

// ---------------------------------------------------------------------------
// The consumers (spec 15.7): what the guides, pages, scripts and expectations say about the declaration and the form.

/// The version spelled right after `needle`'s end in `rest`, past quotes, colons, `=` and the contract's name, when it is `1.0`-`1.3`.
fn old_minor_after(rest: &str) -> Option<String> {
    let skip = |text: &str| text.trim_start_matches([' ', '`', '"', '\'', ':', '=', '\\']).to_string();
    let mut rest = skip(rest);
    if let Some(after) = rest.strip_prefix("atip/1") {
        rest = skip(after);
    }
    let bytes = rest.as_bytes();
    let is_old = bytes.len() >= 3 && &bytes[..2] == b"1." && (b'0'..=b'3').contains(&bytes[2]) && bytes.get(3).is_none_or(|next| !next.is_ascii_digit());
    is_old.then(|| rest[..3].to_string())
}

/// The versions below 1.4 that `line` spells as the declaration: after `contractVersion`, `--contract`, `claims` and `declares`.
fn old_declarations(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    for needle in ["contractVersion", "--contract", "claims", "declares", "declaring"] {
        for (at, _) in line.match_indices(needle) {
            if let Some(version) = old_minor_after(&line[at + needle.len()..]) {
                found.push(format!("{needle} {version}"));
            }
        }
    }
    found
}

/// A sentence that states a member of the 1.3 form as current: the member and a context that names a command document or a tree entry, without a
/// negation or a mark that it is history (those lines tell what is gone).
fn old_form_member(line: &str) -> Option<&'static str> {
    let lower = line.to_lowercase();
    let history = [
        " no ", "no `", "not ", "never", "without", "instead", "longer", "gone", "removed", "left out", "absent", "neither", "until", "before ", "was ", "were ", "used to", "formerly", "1.3", "1.2", "1.1", "0.35", "answer", "없", "않", "아니", "이전", "대신",
    ];
    if history.iter().any(|mark| lower.contains(mark)) {
        return None;
    }
    let document = lower.contains("command document") || lower.contains("command's document") || lower.contains("명령 문서");
    let entry = lower.contains("tree entr") || lower.contains("entry") || lower.contains("entries");
    if lower.contains("input.schemaref") {
        return Some("input.schemaRef");
    }
    if lower.contains("input.option") && !lower.contains("input.options") {
        return Some("input.option");
    }
    if (lower.contains("kind: \"command\"") || lower.contains("\"kind\": \"command\"") || lower.contains("kind: command")) && (entry || lower.contains("tree")) {
        return Some("kind command of a tree entry");
    }
    if lower.contains("`path`") && (document || entry) {
        return Some("path of a command document");
    }
    if lower.contains("`schemas`") && document {
        return Some("schemas inside a command document");
    }
    if lower.contains("`rejects`") && document {
        return Some("rejects in a command document");
    }
    None
}

/// Every stale sentence of one text file: a declaration below 1.4, and (in prose, not in data files) a member of the old form stated as current.
fn consumer_problems(path: &str, text: &str) -> Vec<String> {
    let data_file = path.ends_with(".json") || path.ends_with(".out");
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate() {
        for declared in old_declarations(line) {
            found.push(format!("{path}:{}: declares {declared}: {}", number + 1, line.trim().chars().take(160).collect::<String>()));
        }
        if !data_file {
            if let Some(member) = old_form_member(line) {
                found.push(format!("{path}:{}: states {member} as current: {}", number + 1, line.trim().chars().take(160).collect::<String>()));
            }
        }
    }
    found
}

/// The self-description documents a data file holds (a conformance expectation prints envelopes, the generated spec holds a tree and the command
/// documents): each must be in the 1.4 form.
fn consumer_document_problems(path: &str, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut check = |what: &str, data: &Value| {
        if matches!(data["kind"].as_str(), Some("tree" | "group" | "command")) {
            found.extend(form_problems(data).into_iter().map(|problem| format!("{path}: {what}: {problem}")));
        }
    };
    if path.ends_with(".out") {
        for (number, line) in text.lines().enumerate() {
            if let Ok(document) = serde_json::from_str::<Value>(line) {
                check(&format!("line {}", number + 1), &document["data"]);
            }
        }
    } else if path.ends_with(".json") {
        if let Ok(spec) = serde_json::from_str::<Value>(text) {
            check("the tree", &spec["tree"]);
            for command in spec["commands"].as_array().map(Vec::as_slice).unwrap_or_default() {
                check(&format!("`{}`", command["command"].as_str().unwrap_or("?")), command);
            }
        }
    }
    found
}

/// The tracked files whose text is a consumer of the declaration: the guides, the website pages (en and ko), the eval scripts, the conformance
/// expectations, the generated spec, README and ARCHITECTURE. The CHANGELOG, `.canon`, the tests (`crates/canon-cli/tests/atip_*` name older minors on
/// purpose) and the research notes are not.
fn is_consumer(path: &str) -> bool {
    path == "README.md" || path == "ARCHITECTURE.md" || path == "docs/cli/canon.cli.json" || ["canon/", "packages/website/src/content/docs/", "evals/", "conformance/"].iter().any(|prefix| path.starts_with(prefix))
}

fn tracked_files() -> Vec<String> {
    let listed = Command::new("git").current_dir(root()).args(["ls-files", "-z"]).output().expect("git ls-files");
    assert!(listed.status.success(), "{}", stderr(&listed));
    String::from_utf8(listed.stdout).unwrap().split('\0').filter(|path| !path.is_empty()).map(str::to_string).collect()
}

// ---------------------------------------------------------------------------
// The CI configuration (F6).

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

/// The header and the body of the recipe `name` of a justfile.
fn recipe(justfile: &str, name: &str) -> (String, Vec<String>) {
    let mut lines = justfile.lines();
    let header = lines.by_ref().find(|line| line.split(':').next() == Some(name) && line.contains(':') && !line.starts_with(' ')).unwrap_or_else(|| panic!("the justfile has the recipe `{name}`")).to_string();
    let body = lines.take_while(|line| line.starts_with("    ") || line.starts_with('\t')).map(str::to_string).collect();
    (header, body)
}

/// Every problem of the declared minors: each of the five must be `1.4` (they agree because they are the one value).
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

fn tokens_of(lines: &[String], needle: &str) -> Vec<String> {
    let line = lines.iter().find(|line| line.contains(needle)).unwrap_or_else(|| panic!("a line holds `{needle}`: {lines:#?}"));
    line.split_whitespace().map(str::to_string).collect()
}

/// The value after `flag` in `tokens`.
fn value_after<'a>(tokens: &'a [String], flag: &str) -> Option<&'a str> {
    tokens.iter().position(|token| token == flag).and_then(|index| tokens.get(index + 1)).map(String::as_str)
}

// ---------------------------------------------------------------------------
// The CHANGELOG entry (F7).

/// The five commands the entry measures, with the argv that prints each document, and the tree.
const MEASURED: [(&str, &[&str]); 5] = [
    ("finding close", &["schema", "finding", "close", "--json"]),
    ("finding list", &["schema", "finding", "list", "--json"]),
    ("purge", &["schema", "purge", "--json"]),
    ("skills show", &["schema", "skills", "show", "--json"]),
    ("evidence add", &["schema", "evidence", "add", "--json"]),
];
const TREE_ROW: &str = "schema (tree)";
const SIZE_ROWS: [&str; 11] = [
    "schema finding close",
    "schema finding list",
    "schema purge",
    "schema skills show",
    "schema evidence add",
    TREE_ROW,
    "schema (tree) + finding close",
    "schema (tree) + finding list",
    "schema (tree) + purge",
    "schema (tree) + skills show",
    "schema (tree) + evidence add",
];
const FORM_WORDS: [&str; 6] = ["declaration", "tree", "group", "command", "--schemas", "--help"];
const ENTRY_HEADING: &str = "canon declares atip/1 1.4";

fn cells(row: &str) -> Vec<String> {
    let inner = row.trim().trim_start_matches('|').trim_end_matches('|');
    inner.split('|').map(|cell| cell.trim().to_string()).collect()
}

/// A size cell: digits, with commas as thousands separators, and an optional trailing `bytes`.
fn bytes_of(cell: &str) -> Option<usize> {
    let cell = cell.trim().trim_matches('`');
    let cell = cell.strip_suffix("bytes").map_or(cell, str::trim);
    (!cell.is_empty() && cell.chars().all(|c| c.is_ascii_digit() || c == ',') && cell.starts_with(|c: char| c.is_ascii_digit())).then(|| cell.replace(',', "").parse::<usize>().ok()).flatten()
}

fn heading_level(line: &str) -> usize {
    line.chars().take_while(|c| *c == '#').count()
}

/// Every problem of the CHANGELOG's `Unreleased` section: one entry whose heading names the declaration, its before/after table, the size rows (each 1.4
/// size equal to a live measure and smaller than its 1.3 size, except the tree's) and the paragraph for the callers. `live` maps a row name to the
/// sizes that row may state (the sum row may be the tree plus any one of the commands).
fn changelog_problems(text: &str, live: &BTreeMap<String, Vec<usize>>) -> Vec<String> {
    let mut found = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines.iter().position(|line| line.starts_with("## 0.36.0")) else { return vec!["there is no `## 0.36.0` section".to_string()] };
    let end = lines.iter().enumerate().skip(start + 1).find(|(_, line)| line.starts_with("## ")).map_or(lines.len(), |(index, _)| index);
    let section = &lines[start + 1..end];
    let headings: Vec<usize> = section.iter().enumerate().filter(|(_, line)| (line.starts_with('#') || line.starts_with("**")) && line.contains(ENTRY_HEADING)).map(|(index, _)| index).collect();
    if headings.len() != 1 {
        return vec![format!("`Unreleased` holds {} headings that name `{ENTRY_HEADING}`, not one", headings.len())];
    }
    let heading = headings[0];
    let level = if section[heading].starts_with('#') { heading_level(section[heading]) } else { 3 };
    let entry_end = section.iter().enumerate().skip(heading + 1).find(|(_, line)| line.starts_with('#') && heading_level(line) <= level).map_or(section.len(), |(index, _)| index);
    let entry = &section[heading + 1..entry_end];

    let rows: Vec<&str> = entry.iter().copied().filter(|line| line.trim_start().starts_with('|')).collect();
    if !rows.iter().any(|row| cells(row).iter().all(|cell| !cell.is_empty() && cell.chars().all(|c| matches!(c, '-' | ':' | ' ')))) {
        found.push("the entry has no table".to_string());
    }
    let is_size_row = |row: &str| cells(row).first().is_some_and(|first| SIZE_ROWS.contains(&first.trim_matches('`').to_lowercase().as_str()));
    let form_text = rows.iter().filter(|row| !is_size_row(row)).copied().collect::<Vec<_>>().join("\n").to_lowercase();
    for word in FORM_WORDS {
        if !form_text.contains(word) {
            found.push(format!("the before/after table has no row for `{word}`"));
        }
    }
    let mut stated: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for name in SIZE_ROWS {
        let Some(row) = rows.iter().find(|row| cells(row).first().is_some_and(|first| first.trim_matches('`').to_lowercase() == name)) else {
            found.push(format!("the table has no size row `{name}`"));
            continue;
        };
        let columns = cells(row);
        let (Some(before), Some(after)) = (columns.get(1).and_then(|cell| bytes_of(cell)), columns.get(2).and_then(|cell| bytes_of(cell))) else {
            found.push(format!("the size row `{name}` has no two plain integers: {row}"));
            continue;
        };
        match live.get(name) {
            Some(sizes) if sizes.contains(&after) => {}
            Some(sizes) => found.push(format!("the size row `{name}` states {after} bytes for 1.4, the live measure is {sizes:?}")),
            None => found.push(format!("no live measure for `{name}`")),
        }
        // A command document is smaller; the tree may grow (it holds the shared options); a sum may grow, with its reason.
        if !name.contains("(tree)") && after >= before {
            found.push(format!("the size row `{name}` is not smaller: {after} bytes against {before}"));
        }
        stated.insert(name, (before, after));
    }
    let mut grown = Vec::new();
    if let Some(&(tree_before, _)) = stated.get(TREE_ROW) {
        for (command, _) in MEASURED {
            let (Some(&(command_before, _)), Some(&(sum_before, sum_after))) = (stated.get(format!("schema {command}").as_str()), stated.get(format!("{TREE_ROW} + {command}").as_str())) else { continue };
            if sum_before != tree_before + command_before {
                found.push(format!("the sum row `{TREE_ROW} + {command}` states {sum_before} bytes for 0.35, not the tree plus the command ({})", tree_before + command_before));
            }
            if sum_after >= sum_before {
                grown.push(command);
            }
        }
    }
    if !grown.is_empty() && !entry.iter().any(|line| line.to_lowercase().contains("sharing")) {
        found.push(format!("the sum for {grown:?} is not smaller and the entry gives no reason (the crate's sharing rule)"));
    }
    let paragraph = entry.iter().position(|line| line.trim_start_matches(['*', '_', '>', '-', ' ']).starts_with("Callers"));
    match paragraph {
        None => found.push("the entry has no paragraph that starts with `Callers`".to_string()),
        Some(index) => {
            let body: Vec<&str> = entry[index..].iter().copied().take_while(|line| !line.trim().is_empty()).collect();
            if !body.join(" ").contains("1.3") {
                found.push("the `Callers` paragraph does not mention 1.3".to_string());
            }
        }
    }
    found
}

/// The `data`-carrying stdout of a call, without its newline: the size the changelog states.
fn live_size(dir: &Path, args: &[&str]) -> usize {
    let output = canon(dir, args);
    assert!(output.status.success() && output.stderr.is_empty(), "canon {args:?}: {}{}", stdout(&output), stderr(&output));
    let text = line_of(&output);
    assert!(serde_json::from_str::<Value>(&text).is_ok_and(|document| document.get("data").is_some()), "canon {args:?} carries data");
    text.len()
}

fn live_sizes(dir: &Path) -> BTreeMap<String, Vec<usize>> {
    let tree = live_size(dir, &["schema", "--json"]);
    let mut sizes = BTreeMap::from([(TREE_ROW.to_string(), vec![tree])]);
    for (command, argv) in MEASURED {
        let size = live_size(dir, argv);
        sizes.insert(format!("schema {command}"), vec![size]);
        sizes.insert(format!("{TREE_ROW} + {command}"), vec![tree + size]);
    }
    sizes
}

/// A complete entry in the format the check reads, stating `live` sizes, a 1.3 size 1000 bytes larger for each command and one byte smaller
/// for the tree (which grew), so that every sum is smaller.
fn complete_entry(live: &BTreeMap<String, Vec<usize>>) -> String {
    let mut text = String::from("# Changelog\n\n## 0.36.0\n\n### Added\n\n#### canon declares atip/1 1.4\n\nThe schema answer is the call surface.\n\n| | 0.35 | 0.36 |\n|---|---|---|\n| declaration | atip/1 1.2, L2 | atip/1 1.4, L2 |\n| `schema` tree | commands only | commands and shared options |\n| `schema <group>` | with `path` | without `path` |\n| `schema <command>` | schemas inline | call surface only |\n| `--schemas` | none | schemas, `Input`, `rejects` |\n| `--help --json` | text | refused |\n\n| answer | 0.35 bytes | 0.36 bytes |\n|---|---|---|\n");
    let thousands = |number: usize| number.to_string().as_bytes().rchunks(3).rev().map(|digits| std::str::from_utf8(digits).unwrap()).collect::<Vec<_>>().join(",");
    let tree_after = live[TREE_ROW][0];
    let tree_before = tree_after - 1;
    for name in SIZE_ROWS {
        let after = live[name][0];
        let before = if name == TREE_ROW {
            tree_before
        } else if let Some(command) = name.strip_prefix("schema (tree) + ") {
            tree_before + live[&format!("schema {command}")][0] + 1000
        } else {
            after + 1000
        };
        text.push_str(&format!("| {name} | {} | {} |\n", thousands(before), thousands(after)));
    }
    text.push_str("\nCallers that validated the self-description against the 1.3 schemas reject a 1.4 document; callers that read it without validating read a missing member as the default of the minor before.\n\n## 0.35.0 — 2026-11-04\n");
    text
}

// ===========================================================================
// atip.claim14.01 - 02 (F1)

#[test]
fn atip_claim14_01_every_envelope_says_1_4_and_the_tree_says_l2_with_shared_options() {
    let dir = repo();
    let at = dir.path();
    let calls: [&[&str]; 7] = [
        &["status", "--json"],
        &["nosuchcommand", "--json"],
        &["schema", "--json"],
        &["schema", "finding", "close", "--json"],
        &["schema", "--errors", "--json"],
        &["schema", "--warnings", "--json"],
        &["schema", "finding", "close", "--schemas", "--json"],
    ];
    for args in calls {
        let output = canon(at, args);
        let document = json_of(&output);
        let problems = atip::envelope_problems(&document);
        assert!(problems.is_empty(), "canon {args:?} passes the contract's envelope schema: {problems:?}");
        assert_eq!(document["meta"]["contractVersion"], MINOR, "canon {args:?} declares {MINOR}: {}", document["meta"]);
        if args[0] == "schema" {
            assert_eq!(document["meta"]["schemaVersion"], MINOR, "a schema answer says schemaVersion {MINOR}: canon {args:?}");
        }
    }
    let refused = canon(at, &["nosuchcommand", "--json"]);
    assert_eq!(refused.status.code(), Some(2), "an unknown command is a refusal");

    let tree = answer(at, &schema_argv("", &[]));
    let data = &tree.value["data"];
    assert_eq!(data["kind"], "tree");
    assert_eq!(data["level"], "L2");
    let options = data["options"].as_array().expect("the tree has `options`");
    let commands = entries(&tree.value);
    assert!(options.len() > 30, "the tree defines the shared options once ({})", options.len());
    assert!(commands.len() > 100, "the tree lists the commands and the groups ({})", commands.len());
    assert!(commands.iter().all(|entry| entry.get("path").is_none() && entry["kind"] != "command"), "no entry has `path` or `kind: command`");
    assert!(commands.iter().any(|entry| entry["kind"] == "group") && commands.iter().any(|entry| entry["effect"].is_string()), "the sweep saw groups and commands");
    assert!(options.iter().all(Value::is_object), "a tree option is a full definition");
    let problems = form_problems(data);
    assert!(problems.is_empty(), "no member of the tree is at its default: {problems:?}");
    let contract = atip::schema_doc_problems(&tree.value);
    assert!(contract.is_empty(), "the tree passes the contract's schema-doc schema: {contract:?}");
}

#[test]
fn atip_claim14_02_schema_answers_the_same_bytes_without_a_repository_and_the_form_check_finds_a_1_3_tree() {
    let one = TempDir::new().unwrap();
    let two = TempDir::new().unwrap();
    let bare = |dir: &Path, args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_clear().stdin(Stdio::null()).output().expect("canon runs");
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

const DESTRUCTIVE: [&str; 5] = ["artifacts move", "ledger compact", "migrate", "purge", "tier age"];

fn option_names(doc: &Value) -> Vec<String> {
    doc["options"].as_array().map(Vec::as_slice).unwrap_or_default().iter().map(|option| option_name(option).to_string()).collect()
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
    assert!(names.len() > 5, "finding close lists its options: {names:?}");
    for implied in ["json", "input", "fields"] {
        assert!(!names.contains(&implied.to_string()), "the implied `--{implied}` is not listed: {names:?}");
    }
    let codes: Vec<&str> = close["errors"].as_array().expect("finding close lists errors").iter().map(error_code).collect();
    assert!(!codes.is_empty() && codes.iter().all(|code| !matches!(*code, "USAGE" | "INTERNAL") && !code.starts_with("INPUT_")), "no implied code is listed: {codes:?}");
    assert_eq!(form_problems(close), Vec::<String>::new(), "finding close is in the 1.4 form");

    // The five destructive commands, and only they, list `commit` and `plan-digest` by name.
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
    assert!(surface.docs.len() > 60, "the sweep covered the commands ({})", surface.docs.len());
    let with_validators = surface.docs.values().filter(|doc| doc.value["data"]["validators"].as_array().is_some_and(|list| !list.is_empty())).count();
    assert!(with_validators > 5, "the sweep saw validators ({with_validators})");
    let mut uses: BTreeMap<String, usize> = BTreeMap::new();
    for (name, doc) in &surface.docs {
        let data = &doc.value["data"];
        assert!(data.get("schemas").is_none() && data.get("path").is_none() && data.get("rejects").is_none(), "`{name}` has no `schemas`, `path` or `rejects`");
        for option in data["options"].as_array().map(Vec::as_slice).unwrap_or_default() {
            if let Some(shared) = option.as_str() {
                *uses.entry(shared.to_string()).or_default() += 1;
            }
        }
    }
    let shared = surface.tree_options();
    assert!(shared.len() > 30, "the tree shares options ({})", shared.len());
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
    assert!(surface.docs.len() > 60 && surface.groups.len() > 15, "the sweep covered the commands and the groups ({} and {})", surface.docs.len(), surface.groups.len());
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
    assert!(faults.len() >= 25, "the seeds cover every kind ({})", faults.len());
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
        let document = answer(at, &schema_argv(&name, &[])).value;
        let options = document["data"]["options"].as_array().unwrap();
        assert!(options.iter().any(|option| option.as_str() == Some("commit")), "the document of `{name}` lists `commit`: {options:?}");
        let before = tree(at);
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();
        let output = canon(at, &args);
        let result = json_of(&output);
        assert!(output.status.success() && result["ok"] == true, "the bare `canon {name}` runs: {result}");
        assert_eq!(result["data"]["committed"], false, "the bare `canon {name}` says it did not commit: {}", result["data"]);
        assert!(result["data"]["planDigest"].as_str().is_some_and(|digest| digest.starts_with("sha256:")), "the bare `canon {name}` names the plan to commit: {}", result["data"]);
        let after = tree(at);
        assert!(before.len() > 5, "the repository has files ({})", before.len());
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
    let purge = &answer(at, &schema_argv("purge", &[])).value["data"].clone();
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
    assert!(schemas.len() >= 5 && schemas.contains_key("Input") && schemas.contains_key("FindingCloseReport"), "finding close answers its schemas and `Input`: {:?}", schemas.keys());
    assert_eq!(answered["data"]["kind"], "schemas");
    assert_eq!(answered["data"]["command"], "finding close");
    assert_eq!(answered["meta"]["schemaVersion"], MINOR);

    // The same holds for every command of the tree.
    let mut problems = Vec::new();
    let (mut with_input, mut with_rejects) = (0, 0);
    for (name, got) in SCHEMAS.iter() {
        let doc = surface.data(name);
        problems.extend(schemas_problems(doc, &got.value, &shared));
        with_input += usize::from(doc.get("input").is_some());
        with_rejects += usize::from(got.value["data"]["rejects"].as_array().is_some_and(|lists| !lists.is_empty()));
    }
    assert!(SCHEMAS.len() > 60, "the sweep covered the commands ({})", SCHEMAS.len());
    assert!(with_input > 10 && with_rejects > 5, "the sweep saw commands with `input` ({with_input}) and with validators ({with_rejects})");
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
    assert!(texts.len() > 150, "the sweep covered the tree, the groups and both answers of every command ({})", texts.len());
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

// ===========================================================================
// atip.claim14.11 - 12 (F5)

#[test]
fn atip_claim14_11_the_consumers_declare_1_4_and_state_no_old_form_member_as_current() {
    let root = root();
    let mut scanned = 0;
    let mut declared_current = 0;
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    for path in tracked_files().into_iter().filter(|path| is_consumer(path)) {
        let Ok(text) = std::fs::read_to_string(root.join(&path)) else { continue };
        scanned += 1;
        seen.insert(path.clone());
        declared_current += text.lines().filter(|line| ["contractVersion", "--contract"].iter().any(|needle| line.match_indices(needle).any(|(at, _)| old_minor_after(&line[at + needle.len()..]).is_none() && line[at + needle.len()..].trim_start_matches([' ', '`', '"', '\'', ':', '=', '\\']).starts_with(MINOR)))).count();
        found.extend(consumer_problems(&path, &text));
        found.extend(consumer_document_problems(&path, &text));
    }
    assert!(scanned > 50, "the scan read the consumers ({scanned})");
    for needed in ["README.md", "ARCHITECTURE.md", "docs/cli/canon.cli.json", "packages/website/src/content/docs/cli.mdx", "packages/website/src/content/docs/ko/cli.mdx"] {
        assert!(seen.contains(needed), "the scan read `{needed}`");
    }
    assert!(seen.iter().any(|path| path.starts_with("canon/skills/")) && seen.iter().any(|path| path.starts_with("evals/")) && seen.iter().any(|path| path.starts_with("conformance/cases/")), "the scan read the guides, the evals and the conformance expectations");
    assert!(declared_current > 20, "the scan sees the spellings of the declaration as 1.4 ({declared_current})");
    assert!(found.is_empty(), "stale consumer text:\n{}", found.join("\n"));
}

#[test]
fn atip_claim14_12_the_consumer_scan_finds_a_declaration_below_1_4_and_a_stated_old_form_member() {
    let stale = [
        ("1.2", "canon declares `\"contractVersion\": \"1.2\"` on every envelope"),
        ("1.3", "and `just atip-deep` runs `atip check --level L2 --contract 1.3 --deep`"),
        ("1.1", "{\"meta\":{\"contractVersion\":\"1.1\"}}"),
        ("1.2", "canon claims atip/1 1.2 at L2"),
        ("1.3", "The tree declares contractVersion=1.3."),
    ];
    for (version, line) in stale {
        let found = consumer_problems("guide.md", line);
        assert_found(&format!("the declaration {version} in `{line}`"), &found, "declares");
        assert!(found.iter().any(|problem| problem.contains(version)), "the finding names {version}: {found:?}");
    }
    for line in ["canon declares `\"contractVersion\": \"1.4\"` on every envelope", "runs `atip check --level L2 --contract 1.4 --deep`", "canon claims atip/1 1.4 at L2", "the schema `1.2` of a report is not the declaration", "{\"schemaVersion\":\"1.2\"}"] {
        assert_eq!(consumer_problems("guide.md", line), Vec::<String>::new(), "`{line}` is no stale declaration");
    }
    let members = [
        "A command document carries `input.schemaRef`, the name of the schema of one document.",
        "The option of the input is named by `input.option`.",
        "Each entry of the tree has `kind: \"command\"` and a `path`.",
        "A command document has a `path` that lists its words.",
        "A command document has `schemas`, the JSON Schema of each report.",
        "A command document lists `rejects`, the calls each validator refuses.",
    ];
    for line in members {
        assert!(!consumer_problems("guide.md", line).is_empty(), "the scan finds `{line}`");
    }
    for line in ["There is no `path`, and a command document has no `schemas`.", "A command document no longer has `input.schemaRef`.", "`--schemas` answers `schemas` and `rejects`.", "In 1.3 a command document had `rejects`."] {
        assert_eq!(consumer_problems("guide.md", line), Vec::<String>::new(), "`{line}` tells what is gone");
    }
    // The documents a data file holds.
    let old_tree = json!({ "ok": true, "data": { "kind": "tree", "level": "L2", "commands": [{ "command": "init", "path": ["init"], "kind": "command", "summary": "s", "effect": "write" }] } });
    let found = consumer_document_problems("conformance/cases/x/expected/1.out", &old_tree.to_string());
    assert_found("an old tree in an expectation", &found, "`path`");
    assert_found("an old tree in an expectation", &found, "kind: command");
    let spec = json!({ "tree": old_tree["data"], "commands": [] });
    assert!(!consumer_document_problems("docs/cli/x.json", &spec.to_string()).is_empty(), "an old tree in the generated spec is found");
    // Data files are scanned for the declaration, not for prose.
    assert!(!consumer_problems("conformance/cases/x/expected/1.out", "{\"meta\":{\"contractVersion\":\"1.2\"}}").is_empty());
    assert_eq!(consumer_problems("conformance/cases/x/expected/1.out", "{\"data\":{\"kind\":\"command\",\"note\":\"a command document has `schemas`\"}}"), Vec::<String>::new());
}

// ===========================================================================
// atip.claim14.13 - 14 (F6)

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("`{path}` is readable: {e}"))
}

const FIXTURE: &str = "crates/canon-cli/tests/atip/canon.fixture.json";

#[test]
fn atip_claim14_13_the_declared_minor_the_fixture_the_job_and_the_recipes_agree_on_1_4() {
    let workflow = read(".github/workflows/test.yml");
    let justfile = read("justfile");
    let fixture: Value = serde_json::from_str(&read(FIXTURE)).expect("the fixture is JSON");
    let fixture_minor = fixture["contractVersion"].as_str().expect("the fixture has a contractVersion").to_string();
    let ci = contracts_in(&job_block(&workflow, "atip"));
    let atip = contracts_in(&recipe(&justfile, "atip").1);
    let deep = contracts_in(&recipe(&justfile, "atip-deep").1);
    let declared = canon_cli::json_out::DECLARED_MINOR;

    assert_eq!(minor_problems(declared, &fixture_minor, &ci, &atip, &deep), Vec::<String>::new(), "the declaration, the fixture, the job and the recipes");
    assert_eq!((ci.len(), atip.len(), deep.len()), (1, 1, 1), "each of the job and the two recipes gives its `--contract` once: {ci:?} {atip:?} {deep:?}");
    let built = &SURFACE.tree.value["meta"]["contractVersion"];
    assert_eq!(built, declared, "the binary declares the constant");

    // Lowering one of them fails the agreement.
    let one = |value: &str| vec![value.to_string()];
    let lowered = [
        minor_problems("1.3", &fixture_minor, &ci, &atip, &deep),
        minor_problems(declared, "1.3", &ci, &atip, &deep),
        minor_problems(declared, &fixture_minor, &one("1.3"), &atip, &deep),
        minor_problems(declared, &fixture_minor, &ci, &one("1.3"), &deep),
        minor_problems(declared, &fixture_minor, &ci, &atip, &one("1.3")),
    ];
    let named = ["DECLARED_MINOR", "fixture", "CI job", "`atip`", "`atip-deep`"];
    for (problems, name) in lowered.iter().zip(named) {
        assert_found(&format!("{name} lowered to 1.3"), problems, name);
        assert_eq!(problems.len(), 1, "only the lowered one is found: {problems:?}");
    }
    assert_found("a missing declaration", &minor_problems(declared, &fixture_minor, &[], &atip, &deep), "gives no minor");
    // The readers of the files find a lowered value too.
    assert_eq!(contracts_in(&job_block(&workflow.replace("--contract 1.4", "--contract 1.3"), "atip")), vec!["1.3".to_string()]);
    assert_eq!(contracts_in(&recipe(&justfile.replace("--contract 1.4", "--contract 1.3"), "atip-deep").1), vec!["1.3".to_string()]);
}

#[test]
fn atip_claim14_14_the_job_and_the_recipes_run_the_checker_at_l2_1_4_with_the_fixture() {
    let workflow = read(".github/workflows/test.yml");
    let justfile = read("justfile");
    let job = job_block(&workflow, "atip");
    assert!(job.len() > 20, "the job `atip` was read ({} lines)", job.len());
    let (_, deep_body) = recipe(&justfile, "atip-deep");
    let (_, plain_body) = recipe(&justfile, "atip");
    let expected_fixture = FIXTURE;
    for (what, lines, deep) in [("the job `atip`", &job, true), ("the recipe `atip-deep`", &deep_body, true), ("the recipe `atip`", &plain_body, false)] {
        let tokens = tokens_of(lines, "atip check --bin");
        assert_eq!(value_after(&tokens, "--level"), Some("L2"), "{what}: {tokens:?}");
        assert_eq!(value_after(&tokens, "--contract"), Some(MINOR), "{what}: {tokens:?}");
        assert_eq!(tokens.iter().any(|token| token == "--deep"), deep, "{what} {} `--deep`: {tokens:?}", if deep { "runs" } else { "runs without" });
        assert_eq!(value_after(&tokens, "--fixture"), Some(expected_fixture), "{what} runs the canon fixture: {tokens:?}");
        assert!(tokens.join(" ").contains("--bin {{ target }}/debug/canon") || value_after(&tokens, "--bin").is_some_and(|bin| bin.ends_with("debug/canon")), "{what} runs the built canon: {tokens:?}");
    }
    // The checker's tag is the same in the job (install and cache key) and in the recipe that installs it.
    assert!(job.iter().any(|line| line.contains("cargo install") && line.contains("--tag atip-check-v0.2.0")), "the job installs the checker at atip-check-v0.2.0");
    assert!(job.iter().any(|line| line.contains("key:") && line.contains("atip-check-v0.2.0")), "the cache key of the checker names its tag");
    let (_, install) = recipe(&justfile, "atip-install");
    assert!(install.iter().any(|line| line.contains("--tag atip-check-v0.2.0")), "`just atip-install` installs the checker at atip-check-v0.2.0");
    // `just ci` includes the deep run.
    let (ci_header, _) = recipe(&justfile, "ci");
    assert!(ci_header.split_whitespace().skip(1).any(|dependency| dependency == "atip-deep"), "`just ci` depends on atip-deep: {ci_header}");
    // The fixture claims 1.4 and is the file the commands name.
    let fixture: Value = serde_json::from_str(&read(FIXTURE)).expect("the fixture is JSON");
    assert_eq!(fixture["contractVersion"], MINOR, "the fixture claims {MINOR}");
    // The fixture holds what the checks read.
    assert!(fixture["commands"].as_object().is_some_and(|commands| DESTRUCTIVE.iter().all(|name| commands[*name]["preview"].is_array())), "the fixture previews the five destructive commands");
}

// ===========================================================================
// atip.claim14.15 - 16 (F7)

#[test]
fn atip_claim14_15_the_changelog_entry_states_the_measured_sizes_and_the_note_for_callers() {
    let live = live_sizes(NEUTRAL.path());
    for (name, sizes) in &live {
        assert!(sizes.iter().all(|size| *size > 100), "`{name}` was measured: {sizes:?}");
    }
    let problems = changelog_problems(&read("CHANGELOG.md"), &live);
    assert!(problems.is_empty(), "the CHANGELOG entry `{ENTRY_HEADING}` under `Unreleased` (live sizes {live:?}): {problems:#?}");
}

#[test]
fn atip_claim14_16_the_entry_check_fails_when_the_table_a_size_row_or_the_callers_paragraph_is_missing() {
    let mut live = BTreeMap::from([(TREE_ROW.to_string(), vec![30000])]);
    for (index, (command, _)) in MEASURED.iter().enumerate() {
        let size = 2000 + 500 * index;
        live.insert(format!("schema {command}"), vec![size]);
        live.insert(format!("{TREE_ROW} + {command}"), vec![30000 + size]);
    }
    let complete = complete_entry(&live);
    assert_eq!(changelog_problems(&complete, &live), Vec::<String>::new(), "the complete entry passes:\n{complete}");

    let without_lines = |keep: &dyn Fn(&str) -> bool| complete.lines().filter(|line| keep(line)).collect::<Vec<_>>().join("\n");
    let no_table = without_lines(&|line| !line.starts_with('|'));
    assert_found("an entry without the table", &changelog_problems(&no_table, &live), "no table");
    let no_form = without_lines(&|line| !line.starts_with('|') || line.contains("bytes") || SIZE_ROWS.iter().any(|name| line.starts_with(&format!("| {name} |"))));
    assert_found("an entry without the before/after rows", &changelog_problems(&no_form, &live), "before/after");
    for name in SIZE_ROWS {
        let no_row = without_lines(&|line| !line.starts_with(&format!("| {name} |")));
        assert_found(&format!("an entry without the size row of `{name}`"), &changelog_problems(&no_row, &live), &format!("no size row `{name}`"));
    }
    let no_callers = without_lines(&|line| !line.starts_with("Callers"));
    assert_found("an entry without the callers paragraph", &changelog_problems(&no_callers, &live), "Callers");
    let no_mention = complete.replace("1.3 schemas", "older schemas");
    assert_found("a callers paragraph that does not mention 1.3", &changelog_problems(&no_mention, &live), "1.3");
    let with_row = |name: &str, before: &str, after: usize| complete.lines().map(|line| if line.starts_with(&format!("| {name} |")) { format!("| {name} | {before} | {after} |") } else { line.to_string() }).collect::<Vec<_>>().join("\n");
    let wrong = with_row("schema purge", "4,000", live["schema purge"][0] + 1);
    assert_found("a stated size that is not the live one", &changelog_problems(&wrong, &live), "live measure");
    let larger = with_row("schema purge", "2,999", live["schema purge"][0]);
    assert_found("a size that is not smaller than the 1.3 size", &changelog_problems(&larger, &live), "not smaller");
    let grown = with_row("schema (tree) + purge", "1", live["schema (tree) + purge"][0]);
    assert_found("a sum that grew without a reason", &changelog_problems(&grown, &live), "gives no reason");
    let wrong_sum = with_row("schema (tree) + purge", "99,999", live["schema (tree) + purge"][0]);
    assert_found("a 0.35 sum that is not the tree plus the command", &changelog_problems(&wrong_sum, &live), "not the tree plus the command");
    let reasoned = grown.replace("\nCallers that", "\nA sum grew because the atip crate's sharing rule moves every shared option into the tree.\n\nCallers that");
    assert!(!changelog_problems(&reasoned, &live).iter().any(|problem| problem.contains("gives no reason")), "a grown sum with its reason is accepted");
    let words = complete.replace(ENTRY_HEADING, "canon declares something");
    assert_found("an entry without the heading", &changelog_problems(&words, &live), "not one");
    let outside = complete.replace("## 0.36.0", "## 0.99.0");
    assert_found("an entry outside `Unreleased`", &changelog_problems(&outside, &live), "0.36.0");
    let bold = complete.replace("#### canon declares atip/1 1.4", "**canon declares atip/1 1.4**").replace("Callers that", "**Callers** that");
    assert_eq!(changelog_problems(&bold, &live), Vec::<String>::new(), "an entry that starts with a bold line passes:\n{bold}");

}
