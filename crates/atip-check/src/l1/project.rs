//! Field paths and projection (spec 6.10, 10.13 rules 11 and 15).
//!
//! A field path names a member of a record: `profile.name`, `owners[].email`, and since 1.1 a bracketed key for a
//! member whose name is not an identifier, `attributes["http.status-code"]`. This module reads paths, finds the
//! schema a path names, and judges a projected answer: no member that no kept path covers (`leaks`), every kept
//! member the unprojected answer has (`dropped`), and the projected schema it validates against.

use std::collections::BTreeMap;

use serde_json::{Map, Number, Value};

use super::model::{escape_pointer, CommandDoc, Output};

/// One step of a field path: a member, whether `[]` follows it, whether it was written as a bracketed key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    pub array: bool,
    pub bracket: bool,
}

/// Whether `name` is an identifier: `[A-Za-z_][A-Za-z0-9_]*` (the members a path writes without brackets).
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The number of characters of the identifier that `text` starts with, `0` when it starts with none.
fn identifier_prefix(text: &[char]) -> usize {
    let Some(first) = text.first() else {
        return 0;
    };
    if !(first.is_ascii_alphabetic() || *first == '_') {
        return 0;
    }
    text.iter()
        .take_while(|c| c.is_ascii_alphanumeric() || **c == '_')
        .count()
}

/// The steps of a field path (spec 6.10), `None` when it is malformed: a bracketed key is a JSON string whose only
/// escapes are `\"` and `\\` and that holds no NUL; every other step is an identifier after a `.`; each step may end in `[]`.
pub fn parse_field_path(path: &str) -> Option<Vec<Step>> {
    let chars: Vec<char> = path.chars().collect();
    let mut steps: Vec<Step> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let mut name = String::new();
        let mut bracket = false;
        if chars[i] == '[' && chars.get(i + 1) == Some(&'"') {
            let mut j = i + 2;
            while j < chars.len() && chars[j] != '"' {
                if chars[j] == '\0' {
                    return None;
                }
                if chars[j] == '\\' {
                    match chars.get(j + 1) {
                        Some('"' | '\\') => j += 1,
                        _ => return None,
                    }
                }
                name.push(chars[j]);
                j += 1;
            }
            if !(chars.get(j) == Some(&'"') && chars.get(j + 1) == Some(&']')) {
                return None;
            }
            bracket = true;
            i = j + 2;
        } else {
            if !steps.is_empty() {
                if chars[i] != '.' {
                    return None;
                }
                i += 1;
            }
            let length = identifier_prefix(&chars[i..]);
            if length == 0 {
                return None;
            }
            name.extend(&chars[i..i + length]);
            i += length;
        }
        let array = chars.get(i) == Some(&'[') && chars.get(i + 1) == Some(&']');
        if array {
            i += 2;
        }
        steps.push(Step {
            name,
            array,
            bracket,
        });
    }
    (!steps.is_empty()).then_some(steps)
}

/// The member names of a path, empty when it is malformed.
pub fn step_names(path: &str) -> Vec<String> {
    parse_field_path(path)
        .unwrap_or_default()
        .into_iter()
        .map(|step| step.name)
        .collect()
}

/// Spec 10.13 rule 15: why `path` is malformed, or writes an identifier in brackets; `None` when it is well formed.
pub fn field_path_problem(path: &str) -> Option<String> {
    let Some(steps) = parse_field_path(path) else {
        return Some(format!("field path {path} is malformed (spec 6.10)"));
    };
    steps
        .iter()
        .find(|step| step.bracket && is_identifier(&step.name))
        .map(|step| {
            format!(
                "field path {path} writes the identifier {} in brackets (spec 10.13 rule 15)",
                step.name
            )
        })
}

/// The decoded segments of an RFC 6901 pointer (`~1` is `/`, `~0` is `~`). Like the reference implementation, a
/// pointer that does not start with `/` has none: `""` and a malformed pointer both lead to the root.
pub fn decode_pointer(pointer: &str) -> Vec<String> {
    pointer
        .split('/')
        .skip(1)
        .map(|segment| segment.replace("~1", "/").replace("~0", "~"))
        .collect()
}

/// The members of one node of a [`Trie`].
pub type Members = BTreeMap<String, Trie>;

/// A trie of field paths: a member is `Whole` (covered, kept entirely) or `Branch`, the trie of its kept descendants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trie {
    Whole,
    Branch(Members),
}

/// The trie of `paths` (spec 6.10 rule 2): a path covers its member and replaces any narrower selection below it, and a
/// path below a covered member adds nothing. A malformed path adds nothing.
pub fn field_trie(paths: &[String]) -> Members {
    let mut root = Members::new();
    for path in paths {
        let names = step_names(path);
        let mut node = &mut root;
        for (index, name) in names.iter().enumerate() {
            if matches!(node.get(name), Some(Trie::Whole)) {
                break;
            }
            if index + 1 == names.len() {
                node.insert(name.clone(), Trie::Whole);
                break;
            }
            let entry = node
                .entry(name.clone())
                .or_insert_with(|| Trie::Branch(Members::new()));
            let Trie::Branch(children) = entry else {
                break;
            };
            node = children;
        }
    }
    root
}

/// The records of `data` for `output`: the elements of the array at `fields.records` (an RFC 6901 pointer), or the one value there.
/// Empty when the output declares no `fields` or the pointer leads nowhere.
pub fn records<'a>(data: &'a Value, output: &Output) -> Vec<&'a Value> {
    let Some(fields) = &output.fields else {
        return Vec::new();
    };
    let mut at = data;
    for segment in decode_pointer(&fields.records) {
        match at.as_object().and_then(|object| object.get(&segment)) {
            Some(next) => at = next,
            None => return Vec::new(),
        }
    }
    match at {
        Value::Array(items) => items.iter().collect(),
        one => vec![one],
    }
}

/// Collects the members of `value` that `trie` does not keep.
fn leaks_in(value: &Value, trie: &Members, at: &str, found: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                leaks_in(item, trie, &format!("{at}/{index}"), found);
            }
        }
        Value::Object(object) => {
            for (key, member) in object {
                let here = format!("{at}/{}", escape_pointer(key));
                match trie.get(key) {
                    None => found.push(here),
                    Some(Trie::Branch(children)) => leaks_in(member, children, &here, found),
                    Some(Trie::Whole) => {}
                }
            }
        }
        _ => {}
    }
}

/// Members the projected `data` holds that no kept path covers and that are not an ancestor of one (spec 6.10 rules 2-3), as `/pointer` paths from the record.
pub fn leaks(data: &Value, output: &Output, kept: &[String]) -> Vec<String> {
    let trie = field_trie(kept);
    let mut found = Vec::new();
    for record in records(data, output) {
        leaks_in(record, &trie, "", &mut found);
    }
    found
}

/// Collects the members `steps` names that `unprojected` has and `projected` lacks, walking the two records in step.
fn dropped_in(unprojected: &Value, projected: &Value, steps: &[Step], at: &str, found: &mut Vec<String>) {
    let Some((step, rest)) = steps.split_first() else {
        return;
    };
    // An array on the way keeps every element, each projected the same way.
    if let Value::Array(items) = unprojected {
        let Value::Array(kept) = projected else {
            found.push(at.to_string());
            return;
        };
        if items.len() != kept.len() {
            found.push(at.to_string());
            return;
        }
        for (index, (before, after)) in items.iter().zip(kept).enumerate() {
            dropped_in(before, after, steps, &format!("{at}/{index}"), found);
        }
        return;
    }
    let Some(before) = unprojected.as_object().and_then(|o| o.get(&step.name)) else {
        return;
    };
    let here = format!("{at}/{}", escape_pointer(&step.name));
    let Some(after) = projected.as_object().and_then(|o| o.get(&step.name)) else {
        found.push(here);
        return;
    };
    dropped_in(before, after, rest, &here, found);
}

/// Kept members the unprojected `data` has and the projected one lacks, as `/pointer` paths (spec 6.10 rule 4), record by record;
/// empty when the two do not have the same number of records.
pub fn dropped(unprojected: &Value, projected: &Value, output: &Output, kept: &[String]) -> Vec<String> {
    let before = records(unprojected, output);
    let after = records(projected, output);
    if before.len() != after.len() {
        return Vec::new();
    }
    let paths: Vec<Vec<Step>> = kept
        .iter()
        .filter_map(|path| parse_field_path(path))
        .collect();
    let mut found = Vec::new();
    let several = before.len() > 1;
    for (index, (record, projection)) in before.into_iter().zip(after).enumerate() {
        let at = if several {
            format!("/{index}")
        } else {
            String::new()
        };
        for steps in &paths {
            dropped_in(record, projection, steps, &at, &mut found);
        }
    }
    found
}

// ---- the schemas a path leads through ----

/// The schema `$ref` names, when it is the `#/schemas/<Name>` form and a member of `schemas`; follows a chain of them.
pub fn deref<'a>(schemas: &'a Map<String, Value>, node: Option<&'a Value>) -> Option<&'a Value> {
    let mut current = node;
    for _ in 0..16 {
        let Some(name) = current
            .and_then(Value::as_object)
            .and_then(|object| object.get("$ref"))
            .and_then(Value::as_str)
            .and_then(|reference| reference.strip_prefix("#/schemas/"))
        else {
            break;
        };
        current = schemas.get(name);
    }
    current
}

/// The non-null branch of a null-admitting `anyOf`, or the node itself.
pub fn non_null<'a>(schemas: &'a Map<String, Value>, node: Option<&'a Value>) -> Option<&'a Value> {
    let resolved = deref(schemas, node);
    let branches = resolved
        .and_then(Value::as_object)
        .and_then(|object| object.get("anyOf"))
        .and_then(Value::as_array);
    match branches {
        Some(branches) => deref(
            schemas,
            branches.iter().find(|branch| !is_null_type(branch)),
        ),
        None => resolved,
    }
}

/// A schema whose `type` is `"null"` (and maybe more keywords).
fn is_null_type(node: &Value) -> bool {
    node.get("type").and_then(Value::as_str) == Some("null")
}

/// The `type` keyword of a schema as a list of names.
pub fn types_of(node: &Map<String, Value>) -> Vec<&str> {
    match node.get("type") {
        Some(Value::String(name)) => vec![name.as_str()],
        Some(Value::Array(names)) => names.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

/// The schema at the `records` pointer of an output schema (the record array, or the record).
pub fn records_schema<'a>(schemas: &'a Map<String, Value>, output: &Output) -> Option<&'a Value> {
    let mut node = non_null(schemas, schemas.get(output.name()));
    let pointer = output
        .fields
        .as_ref()
        .map(|fields| fields.records.as_str())
        .unwrap_or_default();
    for segment in decode_pointer(pointer) {
        let member = node
            .and_then(Value::as_object)
            .and_then(|object| object.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get(&segment));
        node = non_null(schemas, member);
    }
    node
}

/// The record schema: the items of the records array, or the records schema itself.
pub fn record_schema<'a>(schemas: &'a Map<String, Value>, output: &Output) -> Option<&'a Value> {
    let node = records_schema(schemas, output);
    match node
        .and_then(Value::as_object)
        .and_then(|object| object.get("items"))
    {
        Some(items) => non_null(schemas, Some(items)),
        None => node,
    }
}

static ANY: Value = Value::Bool(true);

/// The schema a field path names: a member the record schema declares, reached through an array only with `[]`; a
/// last step without `[]` may name a whole array (spec 6.10). `None` when the path is not declared.
pub fn path_node<'a>(
    schemas: &'a Map<String, Value>,
    record: Option<&'a Value>,
    path: &str,
) -> Option<&'a Value> {
    let steps = parse_field_path(path)?;
    let mut node = record;
    for (index, step) in steps.iter().enumerate() {
        let properties = node
            .and_then(Value::as_object)
            .and_then(|object| object.get("properties"))
            .and_then(Value::as_object)?;
        let member = properties.get(&step.name)?;
        node = non_null(schemas, Some(member));
        let is_array = node
            .and_then(Value::as_object)
            .is_some_and(|object| types_of(object).contains(&"array") || object.contains_key("items"));
        if step.array != is_array && (step.array || index + 1 < steps.len()) {
            return None;
        }
        // Only an explicit `[]` descends into the items: a last step without it names the whole array (spec 6.10).
        if step.array {
            if let Some(object) = node.and_then(Value::as_object) {
                node = non_null(schemas, object.get("items"));
            }
        }
    }
    Some(node.unwrap_or(&ANY))
}

/// A map (spec 6.10): an object schema whose `additionalProperties` is a schema (`true` included) and whose `properties` is absent or empty.
pub fn is_map(node: Option<&Value>) -> bool {
    let Some(object) = node.and_then(Value::as_object) else {
        return false;
    };
    let no_properties = match object.get("properties") {
        None => true,
        Some(Value::Object(properties)) => properties.is_empty(),
        Some(_) => false,
    };
    no_properties
        && matches!(
            object.get("additionalProperties"),
            Some(Value::Object(_) | Value::Bool(true))
        )
}

/// The declared path of a map that `path` extends by one key, if `path` is a map-key selection (spec 6.10).
pub fn map_key_of(doc: &CommandDoc, output: &Output, path: &str) -> Option<String> {
    let steps = parse_field_path(path)?;
    let last = steps.last()?;
    if steps.len() < 2 || last.array {
        return None;
    }
    let shape = |steps: &[Step]| -> Vec<(String, bool)> {
        steps
            .iter()
            .map(|step| (step.name.clone(), step.array))
            .collect()
    };
    let prefix = shape(&steps[..steps.len() - 1]);
    let record = record_schema(&doc.schemas, output);
    let fields = output.fields.as_ref()?;
    fields
        .paths
        .iter()
        .chain(&fields.keep)
        .find(|declared| {
            parse_field_path(declared).is_some_and(|steps| shape(&steps) == prefix)
                && is_map(path_node(&doc.schemas, record, declared))
        })
        .cloned()
}

// ---- the projected schema ----

/// A copy of `node` (resolved through `$ref`) with `required` reduced along `keep` below it.
fn project(schemas: &Map<String, Value>, node: Option<&Value>, keep: &Members) -> Option<Value> {
    let mut target = deref(schemas, node)?.clone();
    let Value::Object(object) = &mut target else {
        return Some(target);
    };
    if let Some(Value::Array(branches)) = object.get_mut("anyOf") {
        for branch in branches {
            if !is_null_type(branch) {
                let original = branch.clone();
                if let Some(projected) = project(schemas, Some(&original), keep) {
                    *branch = projected;
                }
            }
        }
        return Some(target);
    }
    if let Some(items) = object.get_mut("items") {
        let original = items.clone();
        if let Some(projected) = project(schemas, Some(&original), keep) {
            *items = projected;
        }
        return Some(target);
    }
    if let Some(Value::Array(required)) = object.get_mut("required") {
        required.retain(|member| {
            let name = match member {
                Value::String(name) => name.clone(),
                other => other.to_string(),
            };
            keep.contains_key(&name)
        });
    }
    if let Some(Value::Object(properties)) = object.get_mut("properties") {
        for (member, child) in keep {
            let Trie::Branch(below) = child else {
                continue;
            };
            if let Some(slot) = properties.get_mut(member) {
                let original = slot.clone();
                if let Some(projected) = project(schemas, Some(&original), below) {
                    *slot = projected;
                }
            }
        }
    }
    Some(target)
}

/// Clones the schemas along `segments` (the `records` pointer) without reducing anything above the records, then projects from there.
fn descend(
    schemas: &Map<String, Value>,
    node: Option<&Value>,
    segments: &[String],
    keep: &Members,
) -> Option<Value> {
    let Some((head, tail)) = segments.split_first() else {
        return project(schemas, node, keep);
    };
    let mut target = deref(schemas, node)?.clone();
    let Value::Object(object) = &mut target else {
        return Some(target);
    };
    if let Some(Value::Array(branches)) = object.get_mut("anyOf") {
        for branch in branches {
            if !is_null_type(branch) {
                let original = branch.clone();
                if let Some(projected) = descend(schemas, Some(&original), segments, keep) {
                    *branch = projected;
                }
            }
        }
        return Some(target);
    }
    if let Some(Value::Object(properties)) = object.get_mut("properties") {
        if let Some(slot) = properties.get_mut(head) {
            let original = slot.clone();
            if let Some(projected) = descend(schemas, Some(&original), tail, keep) {
                *slot = projected;
            }
        }
    }
    Some(target)
}

/// The output's schema with `required` reduced in every object cut node at or below the record along `kept` (spec 6.10, "Projected schema").
pub fn projected_validator(doc: &CommandDoc, output: &Output, kept: &[String]) -> Result<jsonschema::Validator, String> {
    let trie = field_trie(kept);
    let mut schemas = doc.schemas.clone();
    let name = output.name().to_string();
    let pointer = output
        .fields
        .as_ref()
        .map(|fields| fields.records.as_str())
        .unwrap_or_default();
    let segments = decode_pointer(pointer);
    if let Some(projected) = descend(&doc.schemas, doc.schemas.get(&name), &segments, &trie) {
        schemas.insert(name, projected);
    }
    doc.compile_ref_in(&output.schema_ref, &schemas)
}

/// Whether `path` is selectable: declared in `fields.paths` or `fields.keep`, or, for a document that declares 1.1, one key of a declared map (spec 6.10).
pub fn selectable(doc: &CommandDoc, output: &Output, path: &str, v11: bool) -> bool {
    let declared = output
        .fields
        .as_ref()
        .is_some_and(|fields| fields.paths.iter().chain(&fields.keep).any(|d| d == path));
    declared || (v11 && map_key_of(doc, output, path).is_some())
}

/// Whether `number` is a safe integer in the sense of JavaScript's `Number.isSafeInteger` (spec 10.7.5): an integral number within ±(2^53-1).
pub fn is_safe_integer(number: &Number) -> bool {
    const MAX: u64 = 9_007_199_254_740_991;
    if let Some(value) = number.as_i64() {
        return value.unsigned_abs() <= MAX;
    }
    if let Some(value) = number.as_u64() {
        return value <= MAX;
    }
    number
        .as_f64()
        .is_some_and(|value| value.is_finite() && value.fract() == 0.0 && value.abs() <= MAX as f64)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::{json, Value};

    use super::{
        dropped, field_path_problem, field_trie, leaks, map_key_of, parse_field_path, projected_validator, records,
        selectable, Step, Trie,
    };
    use crate::l1::model::{CommandDoc, Output};

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    fn load(relative: &str) -> Value {
        let path = fixtures().join(relative);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    /// The valid command document a report of `tool`'s `command` is about.
    fn command_of(tool: &str, command: &str) -> CommandDoc {
        let mut files: Vec<_> = std::fs::read_dir(fixtures().join("schema-docs"))
            .expect("the fixture directory")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with("schema-command-") && name.ends_with(".json"))
            .collect();
        files.sort();
        for file in files {
            let answer = load(&format!("schema-docs/{file}"));
            if answer["meta"]["tool"] == tool && answer["data"]["command"] == command {
                return CommandDoc::parse(&answer["data"]).expect("a command document");
            }
        }
        panic!("no document of {tool} {command}");
    }

    /// A report example, its command document, and the output that declares `fields`.
    fn report(relative: &str) -> (Value, CommandDoc, Output) {
        let report = load(relative);
        let doc = command_of(
            report["meta"]["tool"].as_str().expect("a tool"),
            report["meta"]["command"].as_str().expect("a command"),
        );
        let output = doc
            .outputs
            .iter()
            .find(|output| output.fields.is_some())
            .expect("an output with fields")
            .clone();
        (report, doc, output)
    }

    fn kept(report: &Value) -> Vec<String> {
        report["meta"]["fields"]
            .as_array()
            .expect("meta.fields")
            .iter()
            .map(|field| field.as_str().expect("a path").to_string())
            .collect()
    }

    const VALID: [&str; 11] = [
        "atip-examples/report-acme-member-list-fields-nested.json",
        "atip-examples/report-acme-member-list-fields-null-profile.json",
        "atip-examples/report-acme-member-list-fields-parent-child.json",
        "atip-examples/report-acme-member-list-fields-map-keys.json",
        "atip-examples/report-acme-member-list-fields-structural-enum.json",
        "atip-examples/report-acme-member-list-fields-note-not-profile.json",
        "atip-examples/report-acme-member-roster-fields.json",
        "atip-examples/report-acme-member-export-fields.json",
        "atip-examples/report-acme-member-export-null.json",
        "atip-examples/report-finding-list-fields.json",
        "atip-examples/report-ledger-entry-list-fields.json",
    ];

    fn step(name: &str, array: bool, bracket: bool) -> Step {
        Step {
            name: name.to_string(),
            array,
            bracket,
        }
    }

    #[test]
    fn atip_checkl1_project_a_field_path_is_dotted_identifiers_and_bracketed_keys() {
        assert_eq!(parse_field_path("id"), Some(vec![step("id", false, false)]));
        assert_eq!(
            parse_field_path("owners[].email"),
            Some(vec![step("owners", true, false), step("email", false, false)])
        );
        assert_eq!(
            parse_field_path(r#"attributes["http.status-code"]"#),
            Some(vec![step("attributes", false, false), step("http.status-code", false, true)])
        );
        assert_eq!(parse_field_path(r#"["display-name"]"#), Some(vec![step("display-name", false, true)]));
        assert_eq!(parse_field_path(r#"attributes[""]"#).map(|steps| steps[1].name.clone()), Some(String::new()));
        assert_eq!(
            parse_field_path(r#"labels[]["a.b"][]"#),
            Some(vec![step("labels", true, false), step("a.b", true, true)])
        );
        // The only escapes of a bracketed key are \" and \\.
        assert_eq!(parse_field_path(r#"a["b\"c"]"#).map(|steps| steps[1].name.clone()), Some("b\"c".to_string()));
        assert_eq!(parse_field_path(r#"a["b\\c"]"#).map(|steps| steps[1].name.clone()), Some("b\\c".to_string()));
    }

    #[test]
    fn atip_checkl1_project_a_malformed_field_path_has_no_steps() {
        for path in [
            "", ".a", "a.", "a..b", "a[", "a[region]", r#"["a""#, r#"["a\n"]"#, r#"a.["b"]"#, "1a", "a[]b", "a b", "a-b",
            "a\0b", "[\"a\0b\"]",
        ] {
            assert_eq!(parse_field_path(path), None, "{path:?}");
            assert!(
                field_path_problem(path).is_some_and(|problem| problem.contains("is malformed")),
                "{path:?}"
            );
        }
    }

    #[test]
    fn atip_checkl1_project_brackets_are_for_names_that_are_not_identifiers_rule_15() {
        assert_eq!(field_path_problem("profile.name"), None);
        assert_eq!(field_path_problem(r#"["display-name"]"#), None);
        assert_eq!(field_path_problem(r#"attributes["http.status-code"]"#), None);
        let problem = field_path_problem(r#"["name"]"#).expect("a problem");
        assert!(problem.contains("writes the identifier name in brackets"), "{problem}");
        assert!(field_path_problem(r#"profile["email"]"#).is_some());
    }

    #[test]
    fn atip_checkl1_project_a_covered_member_is_kept_whole_and_an_ancestor_keeps_only_what_is_present() {
        let paths = |paths: &[&str]| paths.iter().map(ToString::to_string).collect::<Vec<_>>();
        for order in [["profile", "profile.name"], ["profile.name", "profile"]] {
            let trie = field_trie(&paths(&order));
            assert_eq!(trie.get("profile"), Some(&Trie::Whole), "{order:?}");
        }
        let trie = field_trie(&paths(&["id", "owners[].email", "profile.name", "profile.email"]));
        assert_eq!(trie.get("id"), Some(&Trie::Whole));
        let Some(Trie::Branch(profile)) = trie.get("profile") else {
            panic!("profile is an ancestor: {trie:?}");
        };
        assert_eq!(profile.len(), 2);
        assert!(matches!(trie.get("owners"), Some(Trie::Branch(owners)) if owners.get("email") == Some(&Trie::Whole)));
        assert!(field_trie(&paths(&["a..b"])).is_empty());
    }

    #[test]
    fn atip_checkl1_project_the_records_are_found_through_the_pointer() {
        let (nested, _, output) = report("atip-examples/report-acme-member-list-fields-nested.json");
        assert_eq!(records(&nested["data"], &output).len(), 2);
        let (roster, _, output) = report("atip-examples/report-acme-member-roster-fields.json");
        assert_eq!(output.fields.as_ref().expect("fields").records, "/member~1list");
        assert_eq!(records(&roster["data"], &output).len(), 2);
        let (export, _, output) = report("atip-examples/report-acme-member-export-fields.json");
        assert_eq!(output.fields.as_ref().expect("fields").records, "/items/items");
        assert_eq!(records(&export["data"], &output).len(), 2);
        let (null, _, output) = report("atip-examples/report-acme-member-export-null.json");
        assert!(records(&null["data"], &output).is_empty(), "a null report leads nowhere");
        assert!(records(&json!({"other": []}), &output).is_empty());
    }

    #[test]
    fn atip_checkl1_project_the_valid_reports_have_no_leak_no_drop_and_validate_against_the_projected_schema() {
        for file in VALID {
            let (report, doc, output) = report(file);
            let kept = kept(&report);
            assert_eq!(leaks(&report["data"], &output, &kept), Vec::<String>::new(), "{file}");
            assert_eq!(dropped(&report["data"], &report["data"], &output, &kept), Vec::<String>::new(), "{file}");
            let validator = projected_validator(&doc, &output, &kept).unwrap_or_else(|error| panic!("{file}: {error}"));
            assert!(validator.is_valid(&report["data"]), "{file}");
        }
    }

    #[test]
    fn atip_checkl1_project_without_reducing_required_a_projection_does_not_validate() {
        let (report, doc, output) = report("atip-examples/report-acme-member-list-fields-nested.json");
        let whole = doc.compile_ref(&output.schema_ref).expect("compiles");
        assert!(!whole.is_valid(&report["data"]), "the projection lacks required members of the unprojected schema");
        let projected = projected_validator(&doc, &output, &kept(&report)).expect("compiles");
        assert!(projected.is_valid(&report["data"]));
    }

    #[test]
    fn atip_checkl1_project_a_member_no_kept_path_covers_leaks() {
        let cases = [
            ("report-acme-member-list-fields-leak", vec!["/profile/email"]),
            ("report-acme-member-list-fields-parent-child-leak", vec!["/name"]),
            ("report-acme-member-list-fields-map-key-leak", vec!["/attributes/zone"]),
            ("report-acme-member-list-fields-empty-key-leak", vec!["/attributes/"]),
        ];
        for (file, expected) in cases {
            let (report, _, output) = report(&format!("atip-examples/invalid/{file}.json"));
            assert_eq!(leaks(&report["data"], &output, &kept(&report)), expected, "{file}");
        }
        // A mutated valid report: one more member on a record, in a nested array element.
        let (mut report, _, output) = report("atip-examples/report-acme-member-list-fields-nested.json");
        report["data"]["members"][0]["owners"][0]["name"] = json!("Lead");
        report["data"]["members"][1]["email"] = json!("grace@example.org");
        assert_eq!(
            leaks(&report["data"], &output, &kept(&report)),
            ["/owners/0/name", "/email"].map(str::to_string)
        );
    }

    #[test]
    fn atip_checkl1_project_a_kept_member_the_unprojected_record_has_must_be_in_the_projection() {
        let (report, _, output) = report("atip-examples/report-acme-member-list-fields-nested.json");
        let selected = kept(&report);
        let unprojected = report["data"].clone();
        let mut projected = report["data"].clone();
        projected["members"][0]["profile"]
            .as_object_mut()
            .expect("a profile")
            .remove("name");
        projected["members"][0]["owners"][0]
            .as_object_mut()
            .expect("an owner")
            .remove("email");
        projected["members"][1]
            .as_object_mut()
            .expect("a record")
            .remove("id");
        assert_eq!(
            dropped(&unprojected, &projected, &output, &selected),
            ["/0/profile/name", "/0/owners/0/email", "/1/id"].map(str::to_string)
        );
        // A member the unprojected record lacks is not required of the projection.
        let mut lacking = report["data"].clone();
        lacking["members"][1]
            .as_object_mut()
            .expect("a record")
            .remove("profile");
        assert!(dropped(&lacking, &lacking, &output, &selected).is_empty());
        // Different numbers of records are another rule's.
        let mut fewer = report["data"].clone();
        fewer["members"].as_array_mut().expect("members").pop();
        assert!(dropped(&unprojected, &fewer, &output, &selected).is_empty());
    }

    #[test]
    fn atip_checkl1_project_a_kept_required_member_removed_fails_the_projected_schema() {
        for file in ["missing-child", "missing-nested"] {
            let (report, doc, output) = report(&format!("atip-examples/invalid/report-acme-member-list-fields-{file}.json"));
            let validator = projected_validator(&doc, &output, &kept(&report)).expect("compiles");
            assert!(!validator.is_valid(&report["data"]), "{file}");
        }
    }

    #[test]
    fn atip_checkl1_project_a_map_key_is_selectable_only_for_a_document_that_declares_1_1() {
        let (map_keys, doc, output) = report("atip-examples/report-acme-member-list-fields-map-keys.json");
        for field in kept(&map_keys) {
            assert!(selectable(&doc, &output, &field, true), "{field}");
        }
        assert!(!selectable(&doc, &output, "attributes.region", false));
        assert_eq!(map_key_of(&doc, &output, "attributes.region").as_deref(), Some("attributes"));
        assert_eq!(map_key_of(&doc, &output, r#"attributes["http.status-code"]"#).as_deref(), Some("attributes"));
        assert_eq!(map_key_of(&doc, &output, "labels[].region").as_deref(), Some("labels[]"));
        assert_eq!(map_key_of(&doc, &output, "id"), None);

        // A path that does not descend through the array, or names no declared map: not selectable.
        for (file, field) in [
            ("report-acme-member-list-fields-map-key-without-descent", "labels.region"),
            ("report-acme-member-list-fields-not-a-map-key", "profile.nickname"),
        ] {
            let (report, doc, output) = report(&format!("atip-examples/invalid/{file}.json"));
            assert!(kept(&report).contains(&field.to_string()), "{file}");
            assert!(!selectable(&doc, &output, field, true), "{file}");
        }
        // A 1.0 document keeps a map whole: its map key is no selection.
        let (ledger, doc, output) = report("atip-examples/invalid/report-ledger-entry-list-map-key.json");
        assert!(!selectable(&doc, &output, "attributes.kind", false));
        assert!(selectable(&doc, &output, "id", false));
        assert_eq!(kept(&ledger), ["id", "attributes.kind"]);
    }

    #[test]
    fn atip_checkl1_project_a_whole_map_of_a_1_0_document_is_kept_and_selectable() {
        let (report, doc, output) = report("atip-examples/report-ledger-entry-list-fields.json");
        for field in kept(&report) {
            assert!(selectable(&doc, &output, &field, false), "{field}");
        }
    }
}
