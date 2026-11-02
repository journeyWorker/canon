//! Field selection (spec 6.10): the path grammar, the projection of a record, the declaration a command document
//! publishes (`outputs[].fields`, spec 10.9) and the rules an output with `fields` must keep (spec 10.13 rules 11, 15).
//!
//! One parser reads a path ([`FieldPath::parse`]) for both the `--fields` value and the declaration, and one writer
//! ([`FieldPath`]'s `Display`) spells it, so a path in `meta.fields` compares as text: a member whose name is an
//! identifier is written with a dot, never in brackets; any other name is a bracketed JSON-style key
//! (`attributes["http.status-code"]`). [`split_list`] splits a `--fields` value at the commas outside a bracketed key.
//!
//! The projection ([`project`]) keeps the kept paths and what covers or leads to them. [`check_declaration`] holds a
//! declaration and the schemas it points at to the contract: every cut node is projectable, every path names a member
//! of the record schema and goes through arrays only with `[]`. `projected_problems` (feature `validate`) validates a projected value
//! against the output's schema with `required` reduced to the members that are present, which is the claim spec 6.10
//! makes: a tool's tests hold its real answers to it.
//!
//! A schema is looked up by `$ref` in a document (`doc`): the command document's `{"schemas": {…}}` with refs
//! `#/schemas/<Name>`, or a report schema's own root with its `$defs`. `root` is the schema node of the whole output.

use std::collections::BTreeSet;
use std::fmt;

use serde_json::{json, Map, Value};

use crate::minor::{Minor, UnknownMinor};

/// Whether `name` is an identifier (`[A-Za-z_][A-Za-z0-9_]*`), which a path writes with a dot and never in brackets.
pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_alphabetic() || first == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// One step of a path: a member name, and whether the path goes into each element of the array there (`name[]`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Step {
    pub name: String,
    pub each: bool,
}

/// A field path: the members to go through, from the record down.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldPath(Vec<Step>);

/// Why a string is not a field path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError {
    pub path: String,
    pub why: String,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` is not a field path: {}", self.path, self.why)
    }
}

impl std::error::Error for PathError {}

impl FieldPath {
    /// The steps, from the record down.
    pub fn steps(&self) -> &[Step] {
        &self.0
    }

    /// The member names only: `owners[].email` is `owners`, `email`.
    pub fn names(&self) -> Vec<&str> {
        self.0.iter().map(|step| step.name.as_str()).collect()
    }

    /// The path of one member of the record: `name`, or the bracketed key when the name is not an identifier.
    pub fn member(name: &str) -> FieldPath {
        FieldPath(vec![Step { name: name.to_string(), each: false }])
    }

    /// Whether `other` goes through the same members as `self` and then possibly further (`self` equals or is an ancestor of `other`).
    pub fn covers(&self, other: &FieldPath) -> bool {
        self.0.len() <= other.0.len() && self.0.iter().zip(&other.0).all(|(a, b)| a.name == b.name)
    }

    /// Parses a path in its one spelling (spec 6.10, `envelope.schema.json#/$defs/fieldPath`).
    ///
    /// # Errors
    /// When the text is not a path, or spells a member some other way than the one spelling: an identifier in brackets,
    /// a leading dot, `[]` first, a doubled `[]`, a bracketed key with an unknown escape or a NUL.
    pub fn parse(text: &str) -> Result<FieldPath, PathError> {
        let fail = |why: &str| PathError { path: text.to_string(), why: why.to_string() };
        let chars: Vec<char> = text.chars().collect();
        let mut steps = Vec::new();
        let mut i = 0;
        let mut after_dot = false;
        loop {
            let Some(&c) = chars.get(i) else { return Err(fail(if steps.is_empty() { "it is empty" } else { "a member name must follow the dot" })) };
            let name = if c == '[' {
                if after_dot {
                    return Err(fail("a dot is followed by an identifier, never by a bracketed key"));
                }
                if chars.get(i + 1) == Some(&']') {
                    return Err(fail("`[]` follows the name of an array and cannot start a path or follow another `[]`"));
                }
                i += 1;
                if chars.get(i) != Some(&'"') {
                    return Err(fail("a bracketed key is a quoted string: [\"name\"]"));
                }
                i += 1;
                let mut key = String::new();
                loop {
                    match chars.get(i) {
                        None => return Err(fail("the quoted key is not closed")),
                        Some('"') => {
                            i += 1;
                            break;
                        }
                        Some('\\') => match chars.get(i + 1) {
                            Some(&escaped @ ('"' | '\\')) => {
                                key.push(escaped);
                                i += 2;
                            }
                            _ => return Err(fail("inside a quoted key only \\\" and \\\\ are escapes")),
                        },
                        Some('\0') => return Err(fail("a key with U+0000 cannot be selected: argv cannot carry it")),
                        Some(&other) => {
                            key.push(other);
                            i += 1;
                        }
                    }
                }
                if chars.get(i) != Some(&']') {
                    return Err(fail("the quoted key must be closed by `]`"));
                }
                i += 1;
                if is_identifier(&key) {
                    return Err(fail(&format!("`{key}` is an identifier: write it without brackets")));
                }
                key
            } else if c.is_ascii_alphabetic() || c == '_' {
                let start = i;
                while chars.get(i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_') {
                    i += 1;
                }
                chars[start..i].iter().collect()
            } else {
                return Err(fail(&format!("`{c}` cannot start a member name (an identifier, or a bracketed key for any other name)")));
            };
            after_dot = false;
            let each = chars.get(i) == Some(&'[') && chars.get(i + 1) == Some(&']');
            if each {
                i += 2;
            }
            steps.push(Step { name, each });
            match chars.get(i) {
                None => return Ok(FieldPath(steps)),
                Some('.') => {
                    after_dot = true;
                    i += 1;
                }
                Some('[') => {}
                Some(other) => return Err(fail(&format!("`{other}` cannot follow a member: expected `.`, `[` or the end"))),
            }
        }
    }
}

impl fmt::Display for FieldPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (at, step) in self.0.iter().enumerate() {
            if is_identifier(&step.name) {
                if at > 0 {
                    f.write_str(".")?;
                }
                f.write_str(&step.name)?;
            } else {
                f.write_str("[\"")?;
                for c in step.name.chars() {
                    if c == '"' || c == '\\' {
                        f.write_str("\\")?;
                    }
                    write!(f, "{c}")?;
                }
                f.write_str("\"]")?;
            }
            if step.each {
                f.write_str("[]")?;
            }
        }
        Ok(())
    }
}

/// Splits a `--fields` value at the commas outside a bracketed key. Nothing is trimmed and an empty item stays
/// (`a,` has two), so [`FieldPath::parse`] refuses what is not a path.
pub fn split_list(value: &str) -> Vec<String> {
    let chars: Vec<char> = value.chars().collect();
    let mut items = Vec::new();
    let mut current = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            ',' => {
                items.push(std::mem::take(&mut current));
                i += 1;
            }
            '[' if chars.get(i + 1) == Some(&'"') => {
                // A quoted key: up to its closing quote, where `\"` and `\\` do not close it. An unclosed one runs to the end.
                current.push_str("[\"");
                i += 2;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        current.push(chars[i]);
                        i += 1;
                    }
                    current.push(chars[i]);
                    i += 1;
                }
            }
            other => {
                current.push(other);
                i += 1;
            }
        }
    }
    items.push(current);
    items
}

// -------------------------------------------------------------------------------------------------- projection

/// Projects one record to the kept paths (spec 6.10 rules 2-4): a member a kept path covers stays whole, a member that
/// leads to a kept path keeps only what leads on, an array on the way keeps every element, and a `null` stays `null`.
/// A kept member the record does not have is not made.
pub fn project(record: &Value, kept: &[FieldPath]) -> Value {
    let kept: Vec<Vec<&str>> = kept.iter().map(FieldPath::names).collect();
    project_at(record, &mut Vec::new(), &kept)
}

fn project_at(value: &Value, prefix: &mut Vec<String>, kept: &[Vec<&str>]) -> Value {
    let on = |path: &[&str], prefix: &[String]| path.iter().zip(prefix).all(|(a, b)| *a == b.as_str());
    match value {
        Value::Array(items) => Value::Array(items.iter().map(|item| project_at(item, prefix, kept)).collect()),
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, member) in map {
                prefix.push(key.clone());
                if kept.iter().any(|path| path.len() <= prefix.len() && on(path, prefix)) {
                    out.insert(key.clone(), member.clone());
                } else if kept.iter().any(|path| path.len() > prefix.len() && on(path, prefix)) {
                    out.insert(key.clone(), project_at(member, prefix, kept));
                }
                prefix.pop();
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// A projection ready to apply to an output's `data`: where the records are, and the paths kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projection {
    /// RFC 6901 pointer of the record array in `data` (`""` when `data` is the record).
    pub records: String,
    pub kept: Vec<FieldPath>,
}

impl Projection {
    /// `meta.fields`: the kept paths, each in its one spelling.
    pub fn meta_fields(&self) -> Vec<String> {
        self.kept.iter().map(ToString::to_string).collect()
    }

    /// Projects every record of `data`. A pointer that leads nowhere changes nothing.
    pub fn apply(&self, data: &mut Value) {
        let Some(target) = pointer_mut(data, &self.records) else { return };
        match target {
            Value::Array(records) => {
                for record in records {
                    *record = project(record, &self.kept);
                }
            }
            record => *record = project(record, &self.kept),
        }
    }
}

/// The tokens of an RFC 6901 pointer, decoded (`~1` is `/`, `~0` is `~`); `None` when it is not a pointer.
pub fn pointer_tokens(pointer: &str) -> Option<Vec<String>> {
    if pointer.is_empty() {
        return Some(Vec::new());
    }
    pointer.strip_prefix('/').map(|rest| rest.split('/').map(|token| token.replace("~1", "/").replace("~0", "~")).collect())
}

/// The value at `pointer`, going through object members only (spec 6.10).
fn pointer_mut<'a>(data: &'a mut Value, pointer: &str) -> Option<&'a mut Value> {
    let mut at = data;
    for token in pointer_tokens(pointer)? {
        at = at.as_object_mut()?.get_mut(&token)?;
    }
    Some(at)
}

// -------------------------------------------------------------------------------------------------- declaration

/// What a command document declares for an output (`outputs[].fields`, spec 10.9): the paths a caller may select.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    pub records: String,
    pub paths: Vec<FieldPath>,
    pub keep: Vec<FieldPath>,
    pub default: Vec<FieldPath>,
    /// The declared paths whose schema is a map: their keys are selectable too (since 1.1).
    pub maps: Vec<FieldPath>,
}

/// Why a `--fields` value is refused: `USAGE`, `field` `--fields`, `expected` the selectable paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectError {
    pub message: String,
    /// The selectable paths, each in its one spelling.
    pub expected: Vec<String>,
    /// The part of the value that was refused.
    pub received: String,
}

impl fmt::Display for SelectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SelectError {}

/// What `--fields` (or the declared default) selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// The paths of the answer: the always-kept ones, then the selected, each once.
    pub kept: Vec<FieldPath>,
}

impl Declared {
    /// The selectable paths, each in its one spelling: what a refusal's `expected` lists.
    pub fn expected(&self) -> Vec<String> {
        self.paths.iter().map(ToString::to_string).collect()
    }

    fn is_selectable(&self, path: &FieldPath) -> bool {
        self.paths.contains(path) || self.maps.iter().any(|map| path.0.len() == map.0.len() + 1 && path.0[..map.0.len()] == map.0[..] && !path.0[map.0.len()].each)
    }

    /// The paths a `--fields` value names (spec 6.10 rule 6).
    ///
    /// `None`, or a declaration without `default`: no selection is made and the answer is the whole record, without
    /// `meta.fields`. `None` with a `default`: the answer is projected to it.
    ///
    /// # Errors
    /// An empty value, an item that is not a path, a path that is not declared (nor a key of a declared map), a path that is one of the
    /// always-kept paths.
    pub fn select(&self, value: Option<&str>) -> Result<Option<Selection>, SelectError> {
        let refuse = |message: String, received: &str| SelectError { message, expected: self.expected(), received: received.to_string() };
        let Some(value) = value else {
            return Ok((!self.default.is_empty()).then(|| self.selection_of(&self.default)));
        };
        let mut selected: Vec<FieldPath> = Vec::new();
        for item in split_list(value) {
            let path = FieldPath::parse(&item).map_err(|error| refuse(error.to_string(), &item))?;
            if self.keep.contains(&path) {
                return Err(refuse(format!("`{item}` is always kept: it cannot be selected"), &item));
            }
            if !self.is_selectable(&path) {
                let why = if self.paths.iter().any(|declared| declared.0.len() > path.0.len() && path.covers(declared)) {
                    format!("`{item}` is not a path of its own: select one of the paths below it")
                } else {
                    format!("there is no field `{item}` to select")
                };
                return Err(refuse(why, &item));
            }
            if !selected.contains(&path) {
                selected.push(path);
            }
        }
        Ok(Some(self.selection_of(&selected)))
    }

    fn selection_of(&self, selected: &[FieldPath]) -> Selection {
        let mut kept = self.keep.clone();
        for path in selected {
            if !kept.contains(path) {
                kept.push(path.clone());
            }
        }
        Selection { kept }
    }

    /// The projection of `selection` for an output's `data`.
    pub fn projection(&self, selection: &Selection) -> Projection {
        Projection { records: self.records.clone(), kept: selection.kept.clone() }
    }
}

// -------------------------------------------------------------------------------------------------- schemas

const ANNOTATIONS: [&str; 8] = ["title", "description", "default", "examples", "deprecated", "readOnly", "writeOnly", "$comment"];

/// The keywords that can reject a projection of a valid value (spec 6.10): a cut node has none of them.
const FORBIDDEN: [&str; 17] = [
    "const", "enum", "minProperties", "maxProperties", "dependentRequired", "dependentSchemas", "patternProperties", "propertyNames", "unevaluatedProperties", "if", "then", "else", "oneOf", "allOf", "not", "contains", "uniqueItems",
];

fn resolve<'a>(doc: &'a Value, reference: &str) -> Option<&'a Value> {
    doc.pointer(reference.strip_prefix('#')?)
}

fn is_null_schema(node: &Value) -> bool {
    node.as_object().is_some_and(|object| object.get("type").and_then(Value::as_str) == Some("null") && object.keys().all(|key| key == "type" || ANNOTATIONS.contains(&key.as_str())))
}

/// The `type`s a schema declares.
fn types_of(node: &Value) -> Vec<&str> {
    match node.get("type") {
        Some(Value::String(one)) => vec![one.as_str()],
        Some(Value::Array(many)) => many.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

fn is_array_node(node: &Value) -> bool {
    types_of(node).contains(&"array") || (node.get("items").is_some() && !types_of(node).contains(&"object"))
}

/// Strips `$ref` and a null-admitting union from `node` and checks each wrapper as a cut node's wrapper must be
/// (`$ref` alone, or `anyOf` of exactly two with one `{"type": "null"}`, each with annotations at most). Returns the
/// schema underneath; `None` when the wrappers lead nowhere.
fn unwrap<'a>(doc: &'a Value, node: &'a Value, label: &str, problems: &mut Vec<String>) -> Option<&'a Value> {
    let mut node = node;
    for _ in 0..32 {
        let Some(object) = node.as_object() else {
            problems.push(format!("{label} is a boolean schema: a cut node is an object schema with a type"));
            return None;
        };
        if let Some(reference) = object.get("$ref") {
            if let Some(extra) = object.keys().find(|key| *key != "$ref" && !ANNOTATIONS.contains(&key.as_str())) {
                problems.push(format!("{label} has `{extra}` beside its `$ref`: a cut node that is a reference has annotations only"));
            }
            let Some(target) = reference.as_str().and_then(|reference| resolve(doc, reference)) else {
                problems.push(format!("{label} refers to {reference}, which is not in the document"));
                return None;
            };
            node = target;
        } else if let Some(branches) = object.get("anyOf") {
            if let Some(extra) = object.keys().find(|key| *key != "anyOf" && !ANNOTATIONS.contains(&key.as_str())) {
                problems.push(format!("{label} has `{extra}` beside its `anyOf`: a null-admitting union has annotations only"));
            }
            let Some([first, second]) = branches.as_array().map(Vec::as_slice) else {
                problems.push(format!("{label} has an `anyOf` that is not exactly two branches, one of them {{\"type\": \"null\"}}"));
                return None;
            };
            node = match (is_null_schema(first), is_null_schema(second)) {
                (true, false) => second,
                (false, true) => first,
                _ => {
                    problems.push(format!("{label} has an `anyOf` that is not exactly two branches, one of them {{\"type\": \"null\"}}"));
                    return None;
                }
            };
        } else {
            return Some(node);
        }
    }
    problems.push(format!("{label} refers to itself"));
    None
}

/// Checks the object or array schema a cut node is, once its wrappers are off.
fn check_cut(node: &Value, label: &str, problems: &mut Vec<String>) {
    let Some(object) = node.as_object() else { return };
    let types = types_of(node);
    let nullable = |main: &str| types.iter().all(|t| *t == main || *t == "null") && types.contains(&main);
    let allowed: &[&str] = if nullable("object") {
        &["type", "properties", "required", "additionalProperties"]
    } else if nullable("array") {
        &["type", "items", "minItems", "maxItems"]
    } else {
        problems.push(format!("{label} declares no `type` of \"object\" or \"array\" (or that with \"null\"): a cut node is typed"));
        return;
    };
    for key in object.keys() {
        if ANNOTATIONS.contains(&key.as_str()) || allowed.contains(&key.as_str()) {
            continue;
        }
        if FORBIDDEN.contains(&key.as_str()) {
            problems.push(format!("{label} uses `{key}`, which can reject a projection of a valid value: a cut node is never projected through it"));
        } else {
            problems.push(format!("{label} uses `{key}`: besides annotations a cut node uses only {}", allowed[1..].iter().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ")));
        }
    }
}

/// An object schema whose `additionalProperties` is a schema and whose `properties` is absent or empty (spec 6.10).
fn is_map(node: &Value) -> bool {
    types_of(node).contains(&"object") && node.get("additionalProperties").is_some_and(|extra| !extra.is_boolean() || extra.as_bool() == Some(true) || extra.is_object()) && node.get("properties").and_then(Value::as_object).is_none_or(Map::is_empty)
}

/// How a declaration is held to the contract (spec 10.13 rules 11, 15, 16): by the minor the command document declares.
/// Built by [`Rules::at`] or [`Rules::for_minor`], never by its field:
///
/// ```compile_fail,E0451
/// let _ = atip::fields::Rules { v11: true };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    /// The document declares 1.1 or later (10.13 rule 16, "since 1.1" rules hold for every later minor): a bracketed
    /// path and map keys are allowed, and a declared map is a cut node.
    v11: bool,
}

impl Rules {
    /// The rules of a document that declares `minor`: 1.1 and 1.2 keep the same rules, 1.0 has no bracketed path, no map key and no map cut node.
    pub const fn at(minor: Minor) -> Self {
        Self { v11: minor.at_least(Minor::V1_1) }
    }

    /// The rules of a document that declares the minor written `minor` (`"1.2"`), or [`UnknownMinor`] for any other text.
    pub fn for_minor(minor: &str) -> Result<Self, UnknownMinor> {
        Minor::parse(minor).map(Self::at)
    }
}

/// The problems of one `fields` declaration and the schemas it points at; empty when it keeps the contract.
///
/// `root` is the schema node of all of `data` (the one `schemaRef` names) and `doc` the document its `$ref`s are read in.
/// A path is declared in `paths`, `keep` or `default`, each as a string, as the document writes it.
pub fn check_declaration(doc: &Value, root: &Value, records: &str, paths: &[String], keep: &[String], default: &[String], rules: Rules) -> Vec<String> {
    let mut problems = Vec::new();
    let parse = |what: &str, list: &[String], problems: &mut Vec<String>| -> Vec<FieldPath> {
        let mut seen = BTreeSet::new();
        list.iter()
            .filter_map(|text| match FieldPath::parse(text) {
                Ok(path) if !rules.v11 && text.contains('[') && !text.contains("[]") => {
                    problems.push(format!("`{what}` holds the bracketed key `{text}`, which a document that declares 1.0 cannot have"));
                    Some(path)
                }
                Ok(path) => {
                    if !seen.insert(text.clone()) {
                        problems.push(format!("`{what}` lists `{text}` twice"));
                    }
                    Some(path)
                }
                Err(error) => {
                    problems.push(format!("`{what}`: {error}"));
                    None
                }
            })
            .collect()
    };
    let declared_paths = parse("paths", paths, &mut problems);
    let declared_keep = parse("keep", keep, &mut problems);
    let declared_default = parse("default", default, &mut problems);
    if paths.is_empty() {
        problems.push("`paths` is empty: an output with `fields` has at least one selectable path".to_string());
    }
    for path in &declared_default {
        if !declared_paths.contains(path) {
            problems.push(format!("`default` has `{path}`, which is not in `paths`"));
        }
    }
    for path in &declared_keep {
        if declared_paths.contains(path) {
            problems.push(format!("`{path}` is in both `keep` and `paths`: a kept path is not selectable"));
        }
    }
    let Some(tokens) = pointer_tokens(records) else {
        problems.push(format!("`records` is `{records}`, which is not an RFC 6901 pointer"));
        return problems;
    };

    // The cut nodes above and at the record.
    let cut = |node: &Value, label: &str, problems: &mut Vec<String>| -> Option<Value> {
        let target = unwrap(doc, node, label, problems)?;
        check_cut(target, label, problems);
        Some(target.clone())
    };
    let Some(mut at) = cut(root, "the root schema", &mut problems) else { return dedup(problems) };
    for (index, token) in tokens.iter().enumerate() {
        let label = format!("the schema of `{token}` on the way to the records");
        let Some(member) = at.get("properties").and_then(|properties| properties.get(token)) else {
            problems.push(format!("`records` goes through `{token}`, which the schema does not declare as a member"));
            return dedup(problems);
        };
        let Some(next) = cut(member, &label, &mut problems) else { return dedup(problems) };
        at = next;
        if index + 1 == tokens.len() && is_array_node(&at) {
            let Some(items) = at.get("items").cloned() else {
                problems.push("the schema at `records` is an array without `items`".to_string());
                return dedup(problems);
            };
            let Some(record) = cut(&items, "the record schema", &mut problems) else { return dedup(problems) };
            at = record;
        } else if index + 1 < tokens.len() && is_array_node(&at) {
            problems.push(format!("`records` goes through `{token}`, an array: the pointer goes through object members only"));
            return dedup(problems);
        }
    }
    if tokens.is_empty() && is_array_node(&at) {
        problems.push("`records` is \"\" but the root schema is an array: the root is the record".to_string());
        return dedup(problems);
    }
    if is_array_node(&at) {
        problems.push("the record schema is an array".to_string());
        return dedup(problems);
    }
    let record = at;

    // Every declared path: a member of the record schema, through arrays only with `[]`; the ancestors are cut nodes.
    let all: Vec<&FieldPath> = declared_paths.iter().chain(&declared_keep).collect();
    for path in &all {
        let steps = path.steps();
        let mut cur = record.clone();
        for (index, step) in steps.iter().enumerate() {
            let last = index + 1 == steps.len();
            let so_far = FieldPath(steps[..=index].to_vec());
            let label = format!("the schema of `{so_far}`");
            let member = cur.get("properties").and_then(|properties| properties.get(&step.name)).cloned();
            let Some(member) = member else {
                // A key of a declared map.
                let prefix = FieldPath(steps[..index].to_vec());
                let is_key = rules.v11 && last && !step.each && declared_paths.contains(&prefix) && {
                    let mut ignore = Vec::new();
                    schema_at(doc, &record, &prefix, &mut ignore).is_some_and(|schema| is_map(&schema))
                };
                if !is_key {
                    problems.push(format!("`{path}` names `{}`, which the record schema does not declare", step.name));
                }
                break;
            };
            let mut ignore = Vec::new();
            let Some(target) = unwrap(doc, &member, &label, &mut ignore) else {
                problems.push(format!("`{path}`: {}", ignore.join("; ")));
                break;
            };
            if step.each {
                if !is_array_node(target) {
                    problems.push(format!("`{path}` goes into `{}[]`, which is not an array", step.name));
                    break;
                }
            } else if !last && is_array_node(target) {
                problems.push(format!("`{path}` goes through the array `{}` without `[]`", step.name));
                break;
            }
            if last && !step.each && !is_array_node(target) && is_map(target) && rules.v11 {
                cut(&member, &label, &mut problems);
            }
            if last {
                break;
            }
            // `path` goes on below this member: it is a cut node, and so are the items of an array.
            let Some(inner) = cut(&member, &label, &mut problems) else {
                break;
            };
            cur = inner;
            if step.each {
                let Some(items) = cur.get("items").cloned() else {
                    problems.push(format!("`{so_far}` is an array without `items`"));
                    break;
                };
                let Some(item) = cut(&items, &format!("the items of `{so_far}`"), &mut problems) else {
                    break;
                };
                cur = item;
            }
        }
    }
    dedup(problems)
}

fn dedup(problems: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    problems.into_iter().filter(|problem| seen.insert(problem.clone())).collect()
}

/// The schema `path` names below `record`, wrappers stripped (leniently).
fn schema_at(doc: &Value, record: &Value, path: &FieldPath, problems: &mut Vec<String>) -> Option<Value> {
    let mut cur = record.clone();
    let mut found = None;
    for step in path.steps() {
        let member = cur.get("properties")?.get(&step.name)?;
        let mut target = unwrap(doc, member, "a member", problems)?.clone();
        if step.each {
            let items = target.get("items")?.clone();
            target = unwrap(doc, &items, "an item", problems)?.clone();
        }
        found = Some(target.clone());
        cur = target;
    }
    found
}

/// The schema of one record of the output (the schema at `records`, or its items; the root when `records` is `""`), wrappers stripped.
pub fn record_schema(doc: &Value, root: &Value, records: &str) -> Option<Value> {
    let mut ignore = Vec::new();
    let tokens = pointer_tokens(records)?;
    let mut at = unwrap(doc, root, "the root", &mut ignore)?.clone();
    for (index, token) in tokens.iter().enumerate() {
        let member = at.get("properties")?.get(token)?.clone();
        at = unwrap(doc, &member, "a member", &mut ignore)?.clone();
        if index + 1 == tokens.len() && is_array_node(&at) {
            let items = at.get("items")?.clone();
            at = unwrap(doc, &items, "the record", &mut ignore)?.clone();
        }
    }
    Some(at)
}

/// The member names the record schema declares, in the order it lists them.
pub fn member_names(doc: &Value, root: &Value, records: &str) -> Vec<String> {
    record_schema(doc, root, records).and_then(|record| record.get("properties").and_then(Value::as_object).map(|properties| properties.keys().cloned().collect())).unwrap_or_default()
}

/// The declared paths of `paths` (as strings) whose schema, below `record`, is a map (spec 6.10): the keys of each are selectable.
pub fn declared_maps(doc: &Value, root: &Value, records: &str, paths: &[String]) -> Vec<FieldPath> {
    let mut ignore = Vec::new();
    let Some(record) = record_schema(doc, root, records) else { return Vec::new() };
    paths.iter().filter_map(|text| FieldPath::parse(text).ok()).filter(|path| schema_at(doc, &record, path, &mut ignore).is_some_and(|schema| !is_array_node(&schema) && is_map(&schema) || path.steps().last().is_some_and(|step| step.each) && is_map(&schema))).collect()
}

// -------------------------------------------------------------------------------------------------- reduced schema

/// `root` with `required` reduced to the members present, in every object cut node at or below the record (spec 6.10,
/// "Projected schema"). The result is self-contained apart from the references it keeps to leaves and members kept
/// whole; `projected_problems` (feature `validate`) puts the document back around it.
pub fn reduced(doc: &Value, root: &Value, records: &str, kept: &[FieldPath]) -> Value {
    let kept: Vec<Vec<String>> = kept.iter().map(|path| path.names().into_iter().map(str::to_string).collect()).collect();
    let tokens = pointer_tokens(records).unwrap_or_default();
    reduce_above(doc, root, &tokens, &kept)
}

fn deref<'a>(doc: &'a Value, node: &'a Value) -> &'a Value {
    let mut node = node;
    for _ in 0..32 {
        match node.get("$ref").and_then(Value::as_str).and_then(|reference| resolve(doc, reference)) {
            Some(target) => node = target,
            None => break,
        }
    }
    node
}

/// The `anyOf` of a null-admitting union: the index of the branch that is not null.
fn nullable_branch(node: &Value) -> Option<usize> {
    let branches = node.get("anyOf")?.as_array()?;
    match branches.as_slice() {
        [first, second] if is_null_schema(second) && !is_null_schema(first) => Some(0),
        [first, second] if is_null_schema(first) && !is_null_schema(second) => Some(1),
        _ => None,
    }
}

fn map_through(doc: &Value, node: &Value, f: &mut dyn FnMut(&Value) -> Value) -> Value {
    let node = deref(doc, node);
    if let Some(index) = nullable_branch(node) {
        let mut out = node.clone();
        if let Some(branches) = out.get_mut("anyOf").and_then(Value::as_array_mut) {
            let inner = deref(doc, &branches[index].clone()).clone();
            branches[index] = map_through(doc, &inner, f);
        }
        return out;
    }
    f(node)
}

fn reduce_above(doc: &Value, node: &Value, tokens: &[String], kept: &[Vec<String>]) -> Value {
    map_through(doc, node, &mut |node| {
        let mut out = node.clone();
        match tokens.split_first() {
            None => {
                return if is_array_node(node) {
                    if let Some(items) = node.get("items") {
                        out["items"] = reduce_at(doc, items, &mut Vec::new(), kept);
                    }
                    out
                } else {
                    reduce_at(doc, node, &mut Vec::new(), kept)
                };
            }
            Some((token, rest)) => {
                if let Some(member) = node.get("properties").and_then(|p| p.get(token)) {
                    out["properties"][token] = reduce_above(doc, member, rest, kept);
                }
            }
        }
        out
    })
}

fn present(path: &[String], kept: &[Vec<String>]) -> bool {
    kept.iter().any(|k| (k.len() <= path.len() && k.iter().zip(path).all(|(a, b)| a == b)) || (k.len() > path.len() && path.iter().zip(k).all(|(a, b)| a == b)))
}

fn covered(path: &[String], kept: &[Vec<String>]) -> bool {
    kept.iter().any(|k| k.len() <= path.len() && k.iter().zip(path).all(|(a, b)| a == b))
}

fn reduce_at(doc: &Value, node: &Value, rel: &mut Vec<String>, kept: &[Vec<String>]) -> Value {
    map_through(doc, node, &mut |node| {
        let mut out = node.clone();
        if is_array_node(node) {
            if let Some(items) = node.get("items") {
                out["items"] = reduce_at(doc, items, rel, kept);
            }
            return out;
        }
        if let Some(required) = node.get("required").and_then(Value::as_array) {
            let keep: Vec<Value> = required
                .iter()
                .filter(|name| {
                    name.as_str().is_some_and(|name| {
                        rel.push(name.to_string());
                        let keeps = present(rel, kept);
                        rel.pop();
                        keeps
                    })
                })
                .cloned()
                .collect();
            if keep.is_empty() {
                if let Some(map) = out.as_object_mut() {
                    map.remove("required");
                }
            } else {
                out["required"] = Value::Array(keep);
            }
        }
        if let Some(properties) = node.get("properties").and_then(Value::as_object) {
            for (name, member) in properties {
                rel.push(name.clone());
                if !covered(rel, kept) && present(rel, kept) {
                    out["properties"][name] = reduce_at(doc, member, rel, kept);
                }
                rel.pop();
            }
        }
        out
    })
}

/// The problems of `data` against the output's schema with `required` reduced for `kept` (spec 6.10): empty when the
/// projected value is one the contract allows. The schema is `root`, its `$ref`s read in `doc`.
#[cfg(feature = "validate")]
pub fn projected_problems(doc: &Value, root: &Value, records: &str, kept: &[FieldPath], data: &Value) -> Vec<String> {
    let mut schema = reduced(doc, root, records, kept);
    if !schema.is_object() {
        return vec!["the output schema is not an object schema".to_string()];
    }
    for key in ["schemas", "$defs"] {
        if let Some(defs) = doc.get(key) {
            schema[key] = defs.clone();
        }
    }
    match jsonschema::validator_for(&schema) {
        Ok(validator) => validator.iter_errors(data).map(|error| format!("{error} at {}", error.instance_path())).collect(),
        Err(error) => vec![format!("the reduced schema does not compile: {error}")],
    }
}

/// `{"schemas": …}`, the document a command document's `$ref`s are read in.
pub fn schemas_document(schemas: &Map<String, Value>) -> Value {
    json!({ "schemas": schemas })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(text: &str) -> FieldPath {
        FieldPath::parse(text).unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn a_path_is_written_in_the_one_spelling_it_was_read_in() {
        for text in ["id", "profile.name", "tags[]", "owners[].email", "a.b[].c.d", "attributes[\"http.status-code\"]", "attributes[\"\"]", "[\"x.y\"].value", "[\"display-name\"]", "labels[].region", "a[\"q\\\"uote\"]", "a[\"back\\\\slash\"][]", "[\"a b\"][\"c d\"]", "a[\"\n\"]"] {
            assert_eq!(path(text).to_string(), text, "{text}");
        }
    }

    #[test]
    fn what_is_not_the_one_spelling_is_refused() {
        for text in ["", ".a", "a.", "a..b", "[]", "a[][]", "a[]b", "a[\"b\"", "[\"b\"", "a.[\"b\"]", "a[\"b\"c]", "[\"id\"]", "a[\"name\"]", "a[\"x\\ny\"]", "a[\"\\u0041.\"]", "a b", "1a", "a-b", "a[\"\0\"]", "a[0]", "a[ ]"] {
            assert!(FieldPath::parse(text).is_err(), "`{text}` should be refused");
        }
    }

    #[test]
    fn the_value_splits_at_the_commas_outside_a_bracketed_key() {
        assert_eq!(split_list("a,b"), ["a", "b"]);
        assert_eq!(split_list("a[\"x,y\"],b"), ["a[\"x,y\"]", "b"]);
        assert_eq!(split_list("[\"a\\\",b\"],c"), ["[\"a\\\",b\"]", "c"]);
        assert_eq!(split_list("a[],b"), ["a[]", "b"]);
        assert_eq!(split_list(""), [""]);
        assert_eq!(split_list("a,"), ["a", ""]);
        assert_eq!(split_list("[\"a,b"), ["[\"a,b"]);
    }

    fn kept(list: &[&str]) -> Vec<FieldPath> {
        list.iter().map(|text| path(text)).collect()
    }

    fn record() -> Value {
        json!({
            "id": 1,
            "name": "Ada",
            "display-name": "Ada L.",
            "profile": { "name": "Ada", "email": "a@x", "timezone": "UTC" },
            "owners": [ { "name": "Bo", "email": "b@x" }, { "name": "Cy", "email": "c@x" } ],
            "tags": ["x", "y"],
            "attributes": { "region": "eu", "http.status-code": 200, "": "blank" },
            "labels": [ { "region": "eu", "tier": "a" }, {} ],
            "settings": { "theme": "dark" }
        })
    }

    #[test]
    fn a_covered_member_stays_whole_and_an_ancestor_keeps_only_what_leads_on() {
        let out = project(&record(), &kept(&["id", "profile.name", "owners[].email", "tags"]));
        assert_eq!(out, json!({ "id": 1, "profile": { "name": "Ada" }, "owners": [ { "email": "b@x" }, { "email": "c@x" } ], "tags": ["x", "y"] }));
        let whole = project(&record(), &kept(&["profile", "profile.name"]));
        assert_eq!(whole, json!({ "profile": { "name": "Ada", "email": "a@x", "timezone": "UTC" } }), "selecting profile and profile.name keeps all of profile");
    }

    #[test]
    fn a_bracketed_key_and_a_map_key_select_one_member() {
        let out = project(&record(), &kept(&["id", "[\"display-name\"]", "attributes.region", "attributes[\"http.status-code\"]", "attributes[\"\"]", "labels[].region", "settings.theme"]));
        assert_eq!(
            out,
            json!({
                "id": 1,
                "display-name": "Ada L.",
                "attributes": { "region": "eu", "http.status-code": 200, "": "blank" },
                "labels": [ { "region": "eu" }, {} ],
                "settings": { "theme": "dark" }
            })
        );
        let unselected = project(&record(), &kept(&["attributes.region"]));
        assert_eq!(unselected, json!({ "attributes": { "region": "eu" } }), "a map key that was not selected is left out, the empty key included");
    }

    #[test]
    fn a_member_the_record_lacks_is_not_made_and_a_null_ancestor_stays_null() {
        let out = project(&json!({ "id": 1, "profile": null }), &kept(&["id", "profile.name", "badge"]));
        assert_eq!(out, json!({ "id": 1, "profile": null }));
        assert_eq!(project(&json!(5), &kept(&["id"])), json!(5));
    }

    #[test]
    fn a_projection_applies_to_each_record_at_the_pointer_and_to_the_whole_data_at_the_root() {
        let projection = Projection { records: "/rows".into(), kept: kept(&["id"]) };
        let mut data = json!({ "team": "core", "rows": [ { "id": 1, "name": "a" }, { "id": 2, "name": "b" } ] });
        projection.apply(&mut data);
        assert_eq!(data, json!({ "team": "core", "rows": [ { "id": 1 }, { "id": 2 } ] }));
        let root = Projection { records: String::new(), kept: kept(&["a"]) };
        let mut whole = json!({ "a": 1, "b": 2 });
        root.apply(&mut whole);
        assert_eq!(whole, json!({ "a": 1 }));
        assert_eq!(projection.meta_fields(), ["id"]);
    }

    fn declared() -> Declared {
        Declared { records: "/rows".into(), paths: kept(&["name", "profile", "profile.name", "owners[].email", "attributes"]), keep: kept(&["id"]), default: kept(&["name"]), maps: kept(&["attributes"]) }
    }

    #[test]
    fn a_selection_is_the_kept_paths_then_the_selected_each_once() {
        let selection = declared().select(Some("name,profile.name,name")).unwrap().unwrap();
        let written: Vec<String> = selection.kept.iter().map(ToString::to_string).collect();
        assert_eq!(written, ["id", "name", "profile.name"]);
        assert_eq!(declared().select(None).unwrap().unwrap().kept, kept(&["id", "name"]), "the default applies without --fields");
        let mut no_default = declared();
        no_default.default.clear();
        assert_eq!(no_default.select(None).unwrap(), None, "without a default the answer is the whole record");
    }

    #[test]
    fn a_map_key_is_selectable_through_a_declared_map_only() {
        let select = |text: &str| declared().select(Some(text));
        select("attributes.region").unwrap();
        select("attributes[\"http.status-code\"]").unwrap();
        select("attributes[\"\"]").unwrap();
        select("profile.email").unwrap_err();
        select("attributes.region.deeper").unwrap_err();
        select("name.region").unwrap_err();
    }

    #[test]
    fn a_refusal_names_the_selectable_paths() {
        for (value, why) in [("nosuch", "there is no field"), ("id", "always kept"), ("", "empty"), ("name[]", "no field"), ("owners.email", "no field"), ("owners", "not a path of its own"), ("name,", "empty"), ("[\"name\"]", "identifier")] {
            let error = declared().select(Some(value)).expect_err(value);
            assert!(error.message.contains(why), "{value}: {error}");
            assert_eq!(error.expected, ["name", "profile", "profile.name", "owners[].email", "attributes"]);
        }
    }

    fn schemas() -> Value {
        json!({ "schemas": {
            "Report": { "type": "object", "required": ["team", "rows"], "properties": { "team": { "type": "string" }, "rows": { "type": "array", "items": { "$ref": "#/schemas/Row" } } }, "additionalProperties": false },
            "Row": { "type": "object", "required": ["id", "name", "profile", "owners"], "properties": {
                "id": { "type": "integer" }, "name": { "type": "string" },
                "profile": { "anyOf": [ { "$ref": "#/schemas/Profile" }, { "type": "null" } ] },
                "owners": { "type": "array", "items": { "$ref": "#/schemas/Owner" } },
                "attributes": { "type": "object", "additionalProperties": { "type": "string" } },
                "display-name": { "type": "string" }
            }, "additionalProperties": false },
            "Profile": { "type": "object", "required": ["name", "email"], "properties": { "name": { "type": "string" }, "email": { "type": "string" }, "timezone": { "enum": ["UTC"] } } },
            "Owner": { "type": "object", "required": ["name", "email"], "properties": { "name": { "type": "string" }, "email": { "type": "string" } } }
        } })
    }

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    fn check(doc: &Value, paths: &[&str], keep: &[&str], default: &[&str]) -> Vec<String> {
        check_declaration(doc, &json!({ "$ref": "#/schemas/Report" }), "/rows", &strings(paths), &strings(keep), &strings(default), Rules::at(Minor::V1_2))
    }

    #[test]
    fn atip_crate12_10_rules_follow_the_declared_minor_and_1_1_and_1_2_agree() {
        let doc = json!({ "schemas": { "Report": { "type": "object", "properties": { "rows": { "type": "array", "items": { "$ref": "#/schemas/Row" } } } }, "Row": { "type": "object", "properties": { "attributes": { "type": "object", "additionalProperties": { "type": "string" }, "minProperties": 1 } } } } });
        let declaration = |rules: Rules| check_declaration(&doc, &json!({ "$ref": "#/schemas/Report" }), "/rows", &strings(&["attributes"]), &[], &[], rules);
        let bracketed = |rules: Rules| check_declaration(&doc, &json!({ "$ref": "#/schemas/Report" }), "/rows", &strings(&["attributes", "attributes[\"x.y\"]"]), &[], &[], rules);
        // 1.0 keeps a map whole, so its `minProperties` is no problem, and has no bracketed key.
        let one_zero = Rules::for_minor("1.0").expect("1.0 is a minor");
        assert_eq!(declaration(one_zero), Vec::<String>::new());
        assert!(bracketed(one_zero).iter().any(|problem| problem.contains("bracketed key")), "{:?}", bracketed(one_zero));
        // 1.1 and 1.2 are the same rules: the map is a cut node, which `minProperties` breaks, and a bracketed key is allowed.
        for text in ["1.1", "1.2"] {
            let rules = Rules::for_minor(text).expect("a known minor");
            assert!(declaration(rules).iter().any(|problem| problem.contains("minProperties")), "{text}");
            assert!(!bracketed(rules).iter().any(|problem| problem.contains("bracketed key")), "{text}");
        }
        assert_eq!(Rules::for_minor("1.1"), Rules::for_minor("1.2"));
        assert_eq!(Rules::at(Minor::V1_2), Rules::for_minor("1.2").expect("known"));
        assert_ne!(Rules::at(Minor::V1_0), Rules::at(Minor::V1_1));
    }

    #[test]
    fn atip_crate12_11_a_minor_the_crate_does_not_know_is_refused_never_read_as_the_nearest_one() {
        for text in ["1.9", "1.3", "2.0", "", "1"] {
            assert_eq!(Rules::for_minor(text), Err(UnknownMinor(text.to_string())), "{text:?}");
        }
    }

    #[test]
    fn a_declaration_that_keeps_the_contract_has_no_problem() {
        let problems = check(&schemas(), &["name", "profile", "profile.name", "owners", "owners[].email", "attributes", "[\"display-name\"]"], &["id"], &["name"]);
        assert_eq!(problems, Vec::<String>::new());
    }

    #[test]
    fn a_path_outside_the_record_schema_or_through_an_array_without_brackets_is_a_problem() {
        let problems = check(&schemas(), &["nosuch", "owners.email", "name[]", "profile[].name", "[\"id\"]"], &[], &[]);
        let text = problems.join("\n");
        assert!(text.contains("`nosuch` names `nosuch`"), "{text}");
        assert!(text.contains("goes through the array `owners` without `[]`"), "{text}");
        assert!(text.contains("`name[]` goes into `name[]`, which is not an array"), "{text}");
        assert!(text.contains("identifier"), "{text}");
    }

    #[test]
    fn default_outside_paths_and_keep_overlapping_paths_are_problems() {
        let problems = check(&schemas(), &["name", "id"], &["id"], &["profile"]);
        let text = problems.join("\n");
        assert!(text.contains("`default` has `profile`"), "{text}");
        assert!(text.contains("`id` is in both `keep` and `paths`"), "{text}");
    }

    #[test]
    fn a_cut_node_that_can_reject_a_projection_is_a_problem() {
        for (what, patch) in [
            ("root const", ("Report", json!({ "const": { "team": "x" } }))),
            ("root allOf", ("Report", json!({ "allOf": [ { "required": ["team"] } ] }))),
            ("record enum", ("Row", json!({ "enum": [ { "id": 1 } ] }))),
            ("record oneOf", ("Row", json!({ "oneOf": [ { "required": ["id"] } ] }))),
            ("record minProperties", ("Row", json!({ "minProperties": 1 }))),
            ("ancestor const", ("Profile", json!({ "const": { "name": "a" } }))),
        ] {
            let mut doc = schemas();
            for (key, value) in patch.1.as_object().unwrap() {
                doc["schemas"][patch.0][key] = value.clone();
            }
            let problems = check(&doc, &["name", "profile.name"], &["id"], &[]);
            assert!(!problems.is_empty(), "{what} must be refused");
        }
        let mut doc = schemas();
        doc["schemas"]["Row"]["properties"]["owners"]["uniqueItems"] = json!(true);
        assert!(check(&doc, &["owners[].email"], &[], &[]).iter().any(|p| p.contains("uniqueItems")), "uniqueItems on an array cut node");
        let mut boolean = schemas();
        boolean["schemas"]["Owner"] = json!(true);
        assert!(check(&boolean, &["owners[].email"], &[], &[]).iter().any(|p| p.contains("boolean schema")));
        let mut untyped = schemas();
        untyped["schemas"]["Owner"] = json!({ "properties": { "email": { "type": "string" } } });
        assert!(check(&untyped, &["owners[].email"], &[], &[]).iter().any(|p| p.contains("no `type`")));
    }

    #[test]
    fn a_member_kept_whole_may_use_what_a_cut_node_may_not() {
        let mut doc = schemas();
        // `timezone` is an enum on a leaf under a cut node, `owners` is selected whole: both are unrestricted.
        doc["schemas"]["Row"]["properties"]["owners"]["uniqueItems"] = json!(true);
        assert_eq!(check(&doc, &["owners", "profile.name"], &[], &[]), Vec::<String>::new());
    }

    #[test]
    fn a_declared_map_is_a_cut_node_in_1_1_only() {
        let mut doc = schemas();
        doc["schemas"]["Row"]["properties"]["attributes"]["minProperties"] = json!(1);
        let bad = check(&doc, &["attributes"], &[], &[]);
        assert!(bad.iter().any(|p| p.contains("minProperties")), "{bad:?}");
        let old = check_declaration(&doc, &json!({ "$ref": "#/schemas/Report" }), "/rows", &strings(&["attributes"]), &[], &[], Rules::at(Minor::V1_0));
        assert_eq!(old, Vec::<String>::new(), "a 1.0 map is kept whole");
        assert_eq!(declared_maps(&doc, &json!({ "$ref": "#/schemas/Report" }), "/rows", &strings(&["attributes", "name"])), kept(&["attributes"]));
    }

    #[test]
    fn a_projected_value_passes_the_schema_with_required_reduced_and_a_leak_or_a_wrong_member_does_not() {
        let doc = schemas();
        let root = json!({ "$ref": "#/schemas/Report" });
        let data = json!({ "team": "core", "rows": [ { "id": 1, "profile": { "name": "Ada" }, "owners": [ { "email": "b@x" } ] } ] });
        let fine = projected_problems(&doc, &root, "/rows", &kept(&["id", "profile.name", "owners[].email"]), &data);
        assert_eq!(fine, Vec::<String>::new());
        // Above the record `required` stays: dropping `team` is not a projection.
        let no_team = json!({ "rows": [ { "id": 1 } ] });
        assert!(!projected_problems(&doc, &root, "/rows", &kept(&["id"]), &no_team).is_empty());
        // A selected member that is missing from the record's required, below the record, is a problem; one never selected is not.
        let missing = json!({ "team": "core", "rows": [ { "profile": { "name": "Ada" } } ] });
        assert!(!projected_problems(&doc, &root, "/rows", &kept(&["id", "profile.name"]), &missing).is_empty(), "id is kept and required: it must be there");
        let wrong = json!({ "team": "core", "rows": [ { "id": "one" } ] });
        assert!(!projected_problems(&doc, &root, "/rows", &kept(&["id"]), &wrong).is_empty());
    }

    #[test]
    fn a_record_that_is_the_whole_data_reduces_required_at_the_root() {
        let doc = json!({ "schemas": { "Status": { "type": "object", "required": ["a", "b"], "properties": { "a": { "type": "integer" }, "b": { "type": "string" } } } } });
        let root = json!({ "$ref": "#/schemas/Status" });
        assert_eq!(projected_problems(&doc, &root, "", &kept(&["a"]), &json!({ "a": 1 })), Vec::<String>::new());
        assert!(!projected_problems(&doc, &root, "", &kept(&["a"]), &json!({ "b": "x" })).is_empty());
    }
}
