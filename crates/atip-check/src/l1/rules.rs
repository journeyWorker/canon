//! The rules of spec 10.13 that JSON Schema cannot express, over the documents of a `schema` answer.
//!
//! Each function returns [`Findings`]: one `(condition, problem)` pair per condition it judged, `None` when the
//! condition holds. A condition is reported only when the document has something it is about: the conditions of
//! rule 3 about domains need an argument with a domain, `rule-7` and `rule-10` need an `input`, `rule-11` an output
//! with `fields`, `rule-13` a validator, and the three conditions of rule 4 a constraint. The structural ones
//! (`rule-1`, `rule-2-*`, `rule-3-defaults`, `rule-5`, `rule-6`, `rule-9`) are reported for every command.
//!
//! **One owner per violation.** A condition does not read what another condition judges: a reference that does not
//! resolve is `rule-2-refs`'s, so `rule-7` and `rule-10` skip the schema it names; a domain outside its profile is
//! `rule-3-domain-profile`'s, so the default it constrains is not judged against it. The rules that need the grammar
//! of an invocation (7 and 13) read it through [`invocation`].
//!
//! A problem names the command id and the rule number, then what is wrong:
//! `acme member tag: rule 4: constraint 2 repeats constraint 0 (spec 10.8.2)`.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};

use super::invocation::{self, Args};
use super::model::{camel, ref_name, Arg, CommandDoc, GroupDoc, Output, Tree, TreeEntry};
use super::project::{
    decode_pointer, field_path_problem, is_map, non_null, path_node, record_schema, step_names, types_of, Members, Trie,
};
use super::regex;
use crate::claim::Claim;
use crate::schema::schemas;

/// What a rule found in one document: `(condition, problem)`, one pair per condition judged, `None` when it holds.
pub type Findings = Vec<(&'static str, Option<String>)>;

/// The problems as one message, or `None` when there are none.
fn joined(prefix: &str, problems: &[String]) -> Option<String> {
    (!problems.is_empty()).then(|| format!("{prefix}{}", problems.join("; ")))
}

/// `L1.tree`: the conditions are `level`, `groups-first`, `unique-ids`, `entry-path` (rule 1 for each entry).
/// `answer` is the whole `schema --json` envelope; `claimed_level` is `"L1"`.
///
/// `groups-first` is not judged for a search answer (`data.query`), which lists matches and not the tree.
/// Levels are cumulative: a tree that declares a level at or above the claimed one meets it (L0 < L1 < L2).
fn level_meets(declared: &str, claimed: &str) -> bool {
    const ORDER: [&str; 3] = ["L0", "L1", "L2"];
    match (ORDER.iter().position(|level| *level == declared), ORDER.iter().position(|level| *level == claimed)) {
        (Some(declared), Some(claimed)) => declared >= claimed,
        _ => declared == claimed,
    }
}

pub fn tree_findings(answer: &Value, tree: &Tree, claimed_level: &str) -> Findings {
    let mut findings = Findings::new();
    findings.push((
        "level",
        (!level_meets(&tree.level, claimed_level)).then(|| {
            format!(
                "tree: level {:?} is below the claimed level {claimed_level}",
                tree.level
            )
        }),
    ));

    let search = match answer.pointer("/data/query") {
        None | Some(Value::Null) => false,
        Some(Value::String(query)) => !query.is_empty(),
        Some(_) => true,
    };
    if !search {
        let mut groups: BTreeSet<String> = BTreeSet::new();
        let mut problems = Vec::new();
        for entry in &tree.commands {
            if let Some((_, parents)) = entry.path.split_last() {
                if !parents.is_empty() && !groups.contains(&parents.join(" ")) {
                    problems.push(format!(
                        "{} is listed before its group {}",
                        entry.path.join(" "),
                        parents.join(" ")
                    ));
                }
            }
            if entry.is_group() {
                groups.insert(entry.path.join(" "));
            }
        }
        findings.push(("groups-first", joined("tree: ", &problems)));
    }

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut repeated: BTreeSet<&str> = BTreeSet::new();
    for entry in &tree.commands {
        if !seen.insert(entry.command.as_str()) {
            repeated.insert(entry.command.as_str());
        }
    }
    let problems: Vec<String> = repeated
        .iter()
        .map(|id| format!("command id {id} is listed more than once"))
        .collect();
    findings.push(("unique-ids", joined("tree: ", &problems)));

    findings.push(("entry-path", entry_path_problem("tree", &tree.commands)));
    findings
}

/// Rule 1 for each entry of a tree or group answer: `command` is `path` joined by one space.
fn entry_path_problem(owner: &str, entries: &[TreeEntry]) -> Option<String> {
    let problems: Vec<String> = entries
        .iter()
        .filter(|entry| entry.command != entry.path.join(" "))
        .map(|entry| {
            format!(
                "entry {:?} is not its path {:?} (rule 1)",
                entry.command,
                entry.path.join(" ")
            )
        })
        .collect();
    joined(&format!("{owner}: "), &problems)
}

/// `L1.walk`, for a group document: `group-path` (rule 1: `command` is `path` joined by one space), `group-children` (the children are the tree's, exactly).
pub fn group_findings(group: &GroupDoc, tree: &Tree) -> Findings {
    let id = group.path.join(" ");
    let mut problems = Vec::new();
    if group.command != id {
        problems.push(format!(
            "group {:?} is not its path {id:?} (rule 1)",
            group.command
        ));
    }
    let entries = entry_path_problem(&format!("group {id}"), &group.commands);

    let children = tree.children(&group.path);
    let mut mismatches = Vec::new();
    let mut listed: BTreeSet<&str> = BTreeSet::new();
    for entry in &group.commands {
        if !listed.insert(entry.command.as_str()) {
            mismatches.push(format!("{} is listed twice", entry.command));
            continue;
        }
        match children.iter().find(|child| child.command == entry.command) {
            None => mismatches.push(format!("{} is not a child of {id} in the tree", entry.command)),
            Some(child) if *child != entry => {
                mismatches.push(format!("{} differs from its entry in the tree", entry.command));
            }
            Some(_) => {}
        }
    }
    for child in &children {
        if !listed.contains(child.command.as_str()) {
            mismatches.push(format!("the tree's child {} is not listed", child.command));
        }
    }

    vec![
        (
            "group-path",
            joined(&format!("group {id}: "), &problems).or(entries),
        ),
        ("group-children", joined(&format!("group {id}: "), &mismatches)),
    ]
}

// ---- command documents ----

/// The id of a command for a problem message.
fn label(doc: &CommandDoc) -> String {
    if doc.command.is_empty() {
        doc.path.join(" ")
    } else {
        doc.command.clone()
    }
}

/// One problem line of rule `rule` of `doc` from the details found, or `None` when there are none.
fn finding(doc: &CommandDoc, rule: &str, details: &[String]) -> Option<String> {
    joined(&format!("{}: rule {rule}: ", label(doc)), details)
}

/// The first line of an error's text, for a problem message.
fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or(text)
}

/// The deepest two errors of `validator` against `value`, one line.
fn validation_errors(validator: &jsonschema::Validator, value: &Value) -> String {
    let mut errors: Vec<(usize, String)> = validator
        .iter_errors(value)
        .map(|error| {
            let at = error.instance_path().to_string();
            (
                at.matches('/').count(),
                format!("{} {error}", if at.is_empty() { "/" } else { &at }),
            )
        })
        .collect();
    errors.sort_by_key(|(depth, _)| std::cmp::Reverse(*depth));
    errors
        .into_iter()
        .take(2)
        .map(|(_, text)| text)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Whether `text` holds a placeholder or an ellipsis: `<UPPER_CASE>`, `...` or `…` (approaches/checker.md 7.1).
fn has_placeholder(text: &str) -> bool {
    if text.contains("...") || text.contains('…') {
        return true;
    }
    let chars: Vec<char> = text.chars().collect();
    chars.iter().enumerate().any(|(start, c)| {
        *c == '<'
            && chars.get(start + 1).is_some_and(char::is_ascii_uppercase)
            && chars[start + 1..]
                .iter()
                .position(|c| !(c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_'))
                .is_some_and(|end| chars[start + 1 + end] == '>')
    })
}

/// What is wrong with `value` as a value of `arg`: a placeholder, a value of another type, a value outside its domain.
fn value_issues(arg: &Arg, value: &Value) -> Vec<String> {
    let mut issues = Vec::new();
    if let Value::String(text) = value {
        if has_placeholder(text) {
            issues.push(format!("placeholder {text}"));
        }
    }
    issues.extend(invocation::value_problem(arg, value));
    issues
}

/// `L1.walk`, for a command document: spec 10.13 rules 1-11 and 13 (rule 12 is `L1.examples`).
/// The conditions are `rule-1`, `rule-2-refs`, `rule-2-compile`, `rule-2-outputs`, `rule-3-defaults`, `rule-3-domain-profile`, `rule-3-domain-pattern`,
/// `rule-3-domain-compile`, `rule-4-names`, `rule-4-codes`, `rule-4-relation-once`, `rule-5`, `rule-6`, `rule-7`, `rule-9`, `rule-10`, `rule-11`, `rule-13`
/// (rule 8 is `L1.grammar`'s).
/// `exit_of` gives the registered exit of a code; `claim` decides whether a declared map is a cut node (rule 16).
pub fn command_findings(doc: &CommandDoc, tool: &str, claim: Claim, exit_of: &dyn Fn(&str) -> Option<i64>) -> Findings {
    let mut findings: Findings = vec![("rule-1", rule_1(doc))];

    findings.push(("rule-2-refs", rule_2_refs(doc)));
    findings.push(("rule-2-compile", rule_2_compile(doc)));
    findings.push(("rule-2-outputs", rule_2_outputs(doc)));

    let domains = DomainStates::new(doc);
    findings.push(("rule-3-defaults", rule_3_defaults(doc, &domains)));
    if domains.any {
        findings.push((
            "rule-3-domain-profile",
            finding(doc, "3", &domains.profile()),
        ));
    }
    if domains.any_pattern {
        findings.push((
            "rule-3-domain-pattern",
            finding(doc, "3", &domains.patterns()),
        ));
    }
    if domains.any {
        findings.push((
            "rule-3-domain-compile",
            finding(doc, "3", &domains.compile()),
        ));
    }

    if !doc.constraints.is_empty() {
        let (names, codes) = rule_4_names_and_codes(doc);
        findings.push(("rule-4-names", names));
        findings.push(("rule-4-codes", codes));
        findings.push(("rule-4-relation-once", rule_4_relation_once(doc)));
    }

    findings.push(("rule-5", rule_5(doc)));
    findings.push(("rule-6", rule_6(doc)));
    if doc.input.is_some() {
        findings.push(("rule-7", rule_7(doc, &domains)));
    }
    findings.push(("rule-9", rule_9(doc)));
    if doc.input.is_some() {
        findings.push(("rule-10", rule_10(doc)));
    }
    if doc.outputs.iter().any(|output| output.fields.is_some()) {
        findings.push(("rule-11", rule_11(doc, claim.at_least(Claim::V1_1))));
    }
    if !doc.validators.is_empty() {
        findings.push(("rule-13", rule_13(doc, tool, exit_of, &domains)));
    }
    findings
}

/// Rule 1: `command` is `path` joined by one space.
fn rule_1(doc: &CommandDoc) -> Option<String> {
    let path = doc.path.join(" ");
    if doc.command == path {
        return None;
    }
    finding(
        doc,
        "1",
        &[format!("command {:?} is not its path {path:?}", doc.command)],
    )
}

/// Whether the schema `reference` names is a member of the command's `schemas`.
fn resolves(doc: &CommandDoc, reference: &str) -> bool {
    ref_name(reference).is_some_and(|name| doc.schemas.contains_key(name))
}

/// Rule 2: every `schemaRef`, `eventRef`, `input.schemaRef` and `input.argumentsRef` names a member of `schemas`.
fn rule_2_refs(doc: &CommandDoc) -> Option<String> {
    let mut references: Vec<(String, &str)> = Vec::new();
    for (index, output) in doc.outputs.iter().enumerate() {
        references.push((format!("outputs[{index}].schemaRef"), &output.schema_ref));
        if let Some(event) = &output.event_ref {
            references.push((format!("outputs[{index}].eventRef"), event));
        }
    }
    if let Some(input) = &doc.input {
        references.push(("input.schemaRef".to_string(), &input.schema_ref));
        if let Some(arguments) = &input.arguments_ref {
            references.push(("input.argumentsRef".to_string(), arguments));
        }
    }
    let problems: Vec<String> = references
        .into_iter()
        .filter(|(_, reference)| !reference.is_empty() && !resolves(doc, reference))
        .map(|(what, reference)| format!("{what} {reference} does not name a member of schemas"))
        .collect();
    finding(doc, "2", &problems)
}

/// Rule 2: every member of `schemas` compiles.
fn rule_2_compile(doc: &CommandDoc) -> Option<String> {
    let problems: Vec<String> = doc
        .schemas
        .keys()
        .filter_map(|name| {
            doc.compile_ref(&format!("#/schemas/{name}"))
                .err()
                .map(|error| format!("schemas.{name} does not compile: {}", first_line(&error)))
        })
        .collect();
    finding(doc, "2", &problems)
}

/// Rule 2: no two outputs share a `schemaRef`.
fn rule_2_outputs(doc: &CommandDoc) -> Option<String> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut problems = Vec::new();
    for output in &doc.outputs {
        if !seen.insert(&output.schema_ref) {
            problems.push(format!("two outputs share the schemaRef {}", output.schema_ref));
        }
    }
    finding(doc, "2", &problems)
}

// ---- rule 3: defaults and domains ----

const NUMERIC_KEYWORDS: [&str; 5] = ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum", "multipleOf"];
const STRING_KEYWORDS: [&str; 3] = ["minLength", "maxLength", "pattern"];
const STRUCTURAL_KEYWORDS: [&str; 7] = ["type", "enum", "const", "properties", "required", "additionalProperties", "items"];
const DOMAIN_ANNOTATIONS: [&str; 4] = ["title", "description", "$comment", "format"];

/// The keywords a domain of `value_type` may use (spec 10.7.4), `None` for a type that has no domain.
fn domain_keywords(value_type: &str) -> Option<Vec<&'static str>> {
    match value_type {
        "integer" | "number" | "count" => Some(NUMERIC_KEYWORDS.to_vec()),
        "string" | "path" => Some(STRING_KEYWORDS.to_vec()),
        "json" => Some(
            NUMERIC_KEYWORDS
                .iter()
                .chain(&STRING_KEYWORDS)
                .chain(&STRUCTURAL_KEYWORDS)
                .copied()
                .collect(),
        ),
        _ => None,
    }
}

/// What is wrong with the domain of `arg` against the profile of its valueType (spec 10.7.4), as rule 3 reports it; a value is not judged against such a domain.
pub fn domain_profile_problems(arg: &Arg) -> Vec<String> {
    let Some(domain) = arg.domain.as_ref().filter(|domain| !domain.is_null()) else {
        return Vec::new();
    };
    let mut state = DomainState::default();
    state.walk(arg, domain, &format!("the domain of {}", arg.name));
    state.profile
}

/// Whether the domain of `arg` is one a value cannot be judged against: outside the profile of its valueType or not a JSON Schema that compiles. `L1.walk` reports it (rule 3); the other checks leave the argument's domain alone.
pub fn domain_unusable(arg: &Arg) -> bool {
    !domain_profile_problems(arg).is_empty() || arg.domain.as_ref().is_some_and(|domain| invocation::domain_compile_error(domain).is_some())
}

/// What rule 3 finds in the domain of one argument.
#[derive(Debug, Default)]
struct DomainState {
    /// Keywords outside the profile of the argument's valueType, numbers that are not finite or not safe, non-scalar `enum`.
    profile: Vec<String>,
    /// A `pattern` that is not of the form of 10.7.4, or that Rust refuses.
    patterns: Vec<String>,
    has_pattern: bool,
    /// Why the domain does not compile as a JSON Schema; judged only when the two lists above are empty.
    compile: Option<String>,
}

impl DomainState {
    /// Whether the domain is unusable as a constraint on a value, for any of the three conditions.
    fn broken(&self) -> bool {
        !self.profile.is_empty() || !self.patterns.is_empty() || self.compile.is_some()
    }

    /// Reads `node` as a domain of `arg` (the check of spec 10.7.4: structural, then the patterns, then the compile).
    fn walk(&mut self, arg: &Arg, node: &Value, at: &str) {
        let Some(keywords) = domain_keywords(&arg.value_type) else {
            self.profile.push(format!(
                "{at}: a {} argument has no domain (spec 10.7.4)",
                arg.value_type
            ));
            return;
        };
        let Some(object) = node.as_object() else {
            self.profile.push(format!("{at}: a domain schema is an object"));
            return;
        };
        let integral = arg.value_type == "integer" || arg.value_type == "count";
        for (keyword, value) in object {
            let keyword = keyword.as_str();
            if DOMAIN_ANNOTATIONS.contains(&keyword) {
                continue;
            }
            if !keywords.contains(&keyword) {
                self.profile.push(format!(
                    "{at}: keyword {keyword} is outside the domain profile of {} (spec 10.7.4)",
                    arg.value_type
                ));
                continue;
            }
            if NUMERIC_KEYWORDS.contains(&keyword) {
                match value {
                    Value::Number(number) if number.as_f64().is_some_and(f64::is_finite) => {
                        if integral && !super::project::is_safe_integer(number) {
                            self.profile.push(format!(
                                "{at}: {keyword} {number} is not a safe integer (spec 10.7.5)"
                            ));
                        } else if keyword == "multipleOf" && number.as_f64().is_some_and(|n| n <= 0.0) {
                            self.profile.push(format!("{at}: multipleOf is not positive"));
                        }
                    }
                    _ => self
                        .profile
                        .push(format!("{at}: {keyword} is not a finite number")),
                }
            }
            if keyword == "pattern" {
                self.has_pattern = true;
                let problem = match value {
                    Value::String(pattern) => {
                        regex::form_problem(pattern).or_else(|| regex::rust_problem(pattern))
                    }
                    _ => Some("is not a string".to_string()),
                };
                if let Some(problem) = problem {
                    self.patterns
                        .push(format!("{at}: pattern {value}: {problem} (spec 10.7.4)"));
                }
            }
            if keyword == "enum" || keyword == "const" {
                let items: &[Value] = match value {
                    Value::Array(items) => items,
                    one => std::slice::from_ref(one),
                };
                if items
                    .iter()
                    .any(|item| matches!(item, Value::Object(_) | Value::Array(_)))
                {
                    self.profile.push(format!(
                        "{at}: {keyword} admits only scalar values in a domain (spec 10.7.4)"
                    ));
                }
            }
            if keyword == "properties" {
                if let Value::Object(properties) = value {
                    for (name, child) in properties {
                        self.walk(arg, child, &format!("{at}/properties/{name}"));
                    }
                }
            }
            if keyword == "items" || (keyword == "additionalProperties" && !value.is_boolean()) {
                self.walk(arg, value, &format!("{at}/{keyword}"));
            }
        }
    }
}

/// The domains of a command, by the position of the argument in `doc.args()`.
struct DomainStates {
    states: BTreeMap<usize, DomainState>,
    any: bool,
    any_pattern: bool,
    /// The names of the arguments whose domain is broken: a value is not judged against it by any other condition.
    broken: Vec<String>,
}

impl DomainStates {
    fn new(doc: &CommandDoc) -> Self {
        let mut states = BTreeMap::new();
        let mut broken = Vec::new();
        for (index, arg) in doc.args().enumerate() {
            let Some(domain) = arg.domain.as_ref().filter(|domain| !domain.is_null()) else {
                continue;
            };
            let mut state = DomainState::default();
            let at = format!("the domain of {}", arg.name);
            state.walk(arg, domain, &at);
            if state.profile.is_empty() && state.patterns.is_empty() {
                if let Err(error) = schemas().validator(domain) {
                    state.compile = Some(format!(
                        "{at} does not compile: {}",
                        first_line(&error)
                    ));
                }
            }
            if state.broken() {
                broken.push(arg.name.clone());
            }
            states.insert(index, state);
        }
        Self {
            any: !states.is_empty(),
            any_pattern: states.values().any(|state| state.has_pattern),
            states,
            broken,
        }
    }

    /// Whether `problem` is a value refused by the broken domain of an argument (the domain conditions report that domain).
    fn hides(&self, problem: &str) -> bool {
        self.broken
            .iter()
            .any(|name| problem.contains(&format!("domain of {name}")))
    }

    fn profile(&self) -> Vec<String> {
        self.states
            .values()
            .flat_map(|state| state.profile.iter().cloned())
            .collect()
    }

    fn patterns(&self) -> Vec<String> {
        self.states
            .values()
            .flat_map(|state| state.patterns.iter().cloned())
            .collect()
    }

    fn compile(&self) -> Vec<String> {
        self.states
            .values()
            .filter_map(|state| state.compile.clone())
            .collect()
    }
}

/// The text `String(value)` gives in JavaScript for a scalar: how a default is compared with `possibleValues`.
fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Rule 3: an `enum` default (each element of a repeatable one) is in `possibleValues`; every non-null default is a valid
/// value of its valueType and inside its domain; a count's default is one integer (10.7.2).
fn rule_3_defaults(doc: &CommandDoc, domains: &DomainStates) -> Option<String> {
    let mut problems = Vec::new();
    for (index, arg) in doc.args().enumerate() {
        if arg.default.is_null() {
            continue;
        }
        let elements: Vec<&Value> = match &arg.default {
            Value::Array(items) => items.iter().collect(),
            one => vec![one],
        };
        let mut outside = false;
        if arg.value_type == "enum" {
            for element in &elements {
                if !arg.possible_values.contains(&scalar_text(element)) {
                    outside = true;
                    problems.push(format!(
                        "default {element} of {} is not in possibleValues",
                        arg.name
                    ));
                }
            }
        }
        // A broken domain is the domain conditions' to report, and a value cannot be judged against it.
        if outside || domains.states.get(&index).is_some_and(DomainState::broken) {
            continue;
        }
        let defaults: Vec<&Value> =
            if arg.value_type != "count" && (arg.repeatable || arg.variadic) && arg.default.is_array() {
                elements
            } else {
                vec![&arg.default]
            };
        for default in defaults {
            for issue in value_issues(arg, default) {
                problems.push(format!("default of {}: {issue}", arg.name));
            }
        }
    }
    finding(doc, "3", &problems)
}

// ---- rule 4: constraints ----

/// Rule 4: names of constraints, their `in` values (rule 3 values of the argument they name), and their codes.
fn rule_4_names_and_codes(doc: &CommandDoc) -> (Option<String>, Option<String>) {
    let names: BTreeSet<&str> = doc.args().map(|arg| arg.name.as_str()).collect();
    let mut name_problems = Vec::new();
    let mut code_problems = Vec::new();
    for (index, constraint) in doc.constraints.iter().enumerate() {
        for name in constraint
            .names
            .iter()
            .chain(constraint.if_name.iter())
            .chain(&constraint.then)
        {
            if !names.contains(name.as_str()) {
                name_problems.push(format!("constraint {index} names unknown argument {name}"));
            }
        }
        let target = constraint.if_name.as_deref().and_then(|name| doc.arg(name));
        if let (Some(target), Some(values)) = (target, &constraint.if_in) {
            if target.is_flag() {
                name_problems.push(format!(
                    "constraint {index} uses in on the {} {}",
                    target.value_type, target.name
                ));
            } else {
                for value in values {
                    for issue in value_issues(target, value) {
                        name_problems.push(format!(
                            "constraint {index} value for {}: {issue}",
                            target.name
                        ));
                    }
                }
            }
        }
        if let Some(code) = &constraint.code {
            if !doc.errors.contains(code) {
                code_problems.push(format!(
                    "constraint {index} code {code} is not in errors"
                ));
            }
        }
    }
    (
        finding(doc, "4", &name_problems),
        finding(doc, "4", &code_problems),
    )
}

/// Rule 4: no relation is stated twice (spec 10.8.2): the same rule over the same names, `if`, `in` and `then`, in any
/// order and whatever the `code`; `in` values compare after the normalisation of 11.5.
fn rule_4_relation_once(doc: &CommandDoc) -> Option<String> {
    let sorted = |values: &[Value]| -> Vec<String> {
        let mut texts: Vec<String> = values.iter().map(Value::to_string).collect();
        texts.sort();
        texts
    };
    let words = |words: &[String]| -> Vec<String> {
        let mut texts = words.to_vec();
        texts.sort();
        texts
    };
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut problems = Vec::new();
    for (index, constraint) in doc.constraints.iter().enumerate() {
        let value_type = constraint
            .if_name
            .as_deref()
            .and_then(|name| doc.arg(name))
            .map_or("string", |arg| arg.value_type.as_str());
        let condition = constraint.if_in.as_ref().map(|values| {
            let normalised: Vec<Value> = values
                .iter()
                .map(|value| invocation::normalise(value_type, value))
                .collect();
            sorted(&normalised)
        });
        let key = json!([
            constraint.rule,
            words(&constraint.names),
            constraint.if_name,
            condition,
            words(&constraint.then)
        ])
        .to_string();
        match seen.get(&key) {
            Some(earlier) => problems.push(format!(
                "constraint {index} repeats constraint {earlier} (spec 10.8.2)"
            )),
            None => {
                seen.insert(key, index);
            }
        }
    }
    finding(doc, "4", &problems)
}

// ---- rules 5, 6, 9 ----

/// Rule 5: `input.batch` is true exactly when an output has `batch: true`.
fn rule_5(doc: &CommandDoc) -> Option<String> {
    let input_batch = doc.input.as_ref().is_some_and(|input| input.batch);
    let output_batch = doc.outputs.iter().any(|output| output.batch);
    let problem = match (input_batch, output_batch) {
        (true, false) => "input.batch is true but no output has batch: true",
        (false, true) => "an output has batch: true but input.batch is not true",
        _ => return None,
    };
    finding(doc, "5", &[problem.to_string()])
}

/// Rule 6: outputs are distinguished by `when` or `batch`; at most one has neither.
fn rule_6(doc: &CommandDoc) -> Option<String> {
    let mut seen: BTreeSet<(bool, &str)> = BTreeSet::new();
    let mut problems = Vec::new();
    for output in &doc.outputs {
        let when = output.when.as_deref().unwrap_or_default();
        if !seen.insert((output.batch, when)) {
            problems.push(format!(
                "outputs are not distinguished by when or batch ({} {})",
                if output.batch { "batch" } else { "not batch" },
                if when.is_empty() {
                    "without when".to_string()
                } else {
                    format!("when {when:?}")
                }
            ));
        }
    }
    finding(doc, "6", &problems)
}

/// Rule 9: option names are unique, operand names are unique and distinct from option names, short letters are unique.
fn rule_9(doc: &CommandDoc) -> Option<String> {
    let mut names: BTreeSet<&str> = BTreeSet::new();
    let mut repeated: BTreeSet<&str> = BTreeSet::new();
    for arg in doc.args() {
        if !names.insert(&arg.name) {
            repeated.insert(&arg.name);
        }
    }
    let mut problems: Vec<String> = repeated
        .iter()
        .map(|name| format!("duplicate argument name {name}"))
        .collect();
    let mut letters: BTreeSet<char> = BTreeSet::new();
    let mut repeated_letters: BTreeSet<char> = BTreeSet::new();
    for short in doc.options.iter().filter_map(|option| option.short) {
        if !letters.insert(short) {
            repeated_letters.insert(short);
        }
    }
    problems.extend(
        repeated_letters
            .iter()
            .map(|letter| format!("duplicate short option letter -{letter}")),
    );
    finding(doc, "9", &problems)
}

// ---- rule 10: the fragment schema ----

/// Rule 10: the fragment schema at `input.schemaRef` has no `required` at its root, `additionalProperties: false`, and as
/// properties `operands` and the camelCase names of the options that are not flag-only.
fn rule_10(doc: &CommandDoc) -> Option<String> {
    let input = doc.input.as_ref()?;
    // A reference that does not resolve is rule 2's.
    let name = ref_name(&input.schema_ref).filter(|name| doc.schemas.contains_key(*name))?;
    let fragment = super::project::deref(&doc.schemas, doc.schemas.get(name));
    let Some(object) = fragment.and_then(Value::as_object) else {
        return finding(
            doc,
            "10",
            &["the fragment schema is not an object schema".to_string()],
        );
    };
    let mut problems = Vec::new();
    if object.contains_key("required") {
        problems.push("the fragment schema has a root required list (spec 10.10)".to_string());
    }
    if object.get("additionalProperties") != Some(&Value::Bool(false)) {
        problems.push("the fragment schema does not set additionalProperties: false".to_string());
    }
    let allowed: BTreeSet<String> = std::iter::once("operands".to_string())
        .chain(
            doc.options
                .iter()
                .filter(|option| !option.flag_only)
                .map(|option| camel(&option.name)),
        )
        .collect();
    if let Some(Value::Object(properties)) = object.get("properties") {
        for key in properties.keys() {
            if !allowed.contains(key) {
                problems.push(format!(
                    "the fragment schema has property {key}, which is not a document key"
                ));
            }
        }
    }
    finding(doc, "10", &problems)
}

// ---- rule 7: input.example ----

/// The explicit arguments of one `--input` document, `camelCase` keys naming options (spec 11.5), with the problems of
/// their values.
fn document_args(doc: &CommandDoc, document: &Map<String, Value>, at: &str, problems: &mut Vec<String>) -> Args {
    let mut args = Args::default();
    if let Some(Value::Array(operands)) = document.get("operands") {
        args.operands.clone_from(operands);
        for (index, value) in operands.iter().enumerate() {
            let arg = doc.operands.get(index.min(doc.operands.len().saturating_sub(1)));
            if let Some(arg) = arg.filter(|arg| index < doc.operands.len() || arg.variadic) {
                for issue in value_issues(arg, value) {
                    problems.push(format!("{at} /operands/{index}: {issue}"));
                }
            }
        }
    }
    for (key, value) in document {
        if key == "operands" {
            continue;
        }
        let Some(option) = doc.options.iter().find(|option| camel(&option.name) == *key) else {
            problems.push(format!("{at} /{key}: no such option"));
            continue;
        };
        if option.flag_only {
            problems.push(format!("{at} /{key}: a flag-only option in a document"));
        }
        let values: Vec<&Value> = match value {
            Value::Array(items) if option.repeatable && option.value_type != "count" => items.iter().collect(),
            one => vec![one],
        };
        for item in values {
            for issue in value_issues(option, item) {
                problems.push(format!("{at} /{key}: {issue}"));
            }
        }
        let absent = (option.value_type == "switch" && *value == Value::Bool(false))
            || (option.value_type == "count" && value.as_f64() == Some(0.0));
        if !absent {
            args.options.insert(option.name.clone(), value.clone());
        }
    }
    args
}

/// Rule 7: `input.example` validates against the fragment schema, satisfies `required`, every `domain` and `constraints`,
/// and, when `argumentsRef` is present, its argument object (11.7 step 6) validates against that schema.
fn rule_7(doc: &CommandDoc, domains: &DomainStates) -> Option<String> {
    let input = doc.input.as_ref()?;
    let mut problems = Vec::new();
    // A schema that does not resolve or compile is rule 2's.
    if resolves(doc, &input.schema_ref) {
        if let Ok(fragment) = doc.compile_ref(&input.schema_ref) {
            if !fragment.is_valid(&input.example) {
                problems.push(format!(
                    "input.example fails the fragment schema: {}",
                    validation_errors(&fragment, &input.example)
                ));
            }
        }
    }
    if let Some(document) = input.example.as_object() {
        let args = document_args(doc, document, "input.example", &mut problems);
        for problem in invocation::requirement_problems(doc, &args) {
            problems.push(format!("input.example: {problem}"));
        }
        for index in invocation::violated_constraints(doc, &args) {
            let label = doc
                .constraints
                .get(index)
                .map(super::model::Constraint::label)
                .unwrap_or_default();
            problems.push(format!(
                "input.example violates constraint {index} ({label})"
            ));
        }
        if let Some(reference) = input.arguments_ref.as_deref().filter(|r| resolves(doc, r)) {
            if let Ok(validator) = doc.compile_ref(reference) {
                let object = invocation::arguments_object(doc, &args);
                if !validator.is_valid(&object) {
                    problems.push(format!(
                        "input.example: the argument object fails argumentsRef: {}",
                        validation_errors(&validator, &object)
                    ));
                }
            }
        }
    }
    problems.retain(|problem| !domains.hides(problem));
    finding(doc, "7", &problems)
}

// ---- rule 11: fields ----

/// Keywords a cut node may use beside its shape (spec 6.10).
const ANNOTATIONS: [&str; 8] = [
    "title",
    "description",
    "default",
    "examples",
    "deprecated",
    "readOnly",
    "writeOnly",
    "$comment",
];

/// The depth at which the walk of a cut node gives up: a reference cycle through an array's items never ends otherwise.
const CUT_DEPTH: usize = 64;

/// Whether `branch` is exactly `{"type": "null"}`.
fn is_bare_null(branch: &Value) -> bool {
    branch
        .as_object()
        .is_some_and(|object| object.len() == 1 && object.get("type").and_then(Value::as_str) == Some("null"))
}

/// Checks that every cut node along `trie` is projectable (spec 6.10, rule 11). The caller passes the output's root
/// schema and a trie that starts with the decoded `fields.records` segments, so the spine above the records is checked
/// as well as the records, the record and the ancestors of every declared path.
fn check_cut(
    schemas: &Map<String, Value>,
    node: Option<&Value>,
    trie: &Members,
    at: &str,
    depth: usize,
    problems: &mut Vec<String>,
) {
    // A member the schema does not declare: the path check reports it.
    let Some(node) = node else {
        return;
    };
    if depth > CUT_DEPTH {
        problems.push(format!("{at}: the cut nodes are nested without end (a reference cycle)"));
        return;
    }
    let Some(object) = node.as_object() else {
        problems.push(format!(
            "{at}: the {} schema {node} is not a projectable cut node (spec 6.10)",
            if node.is_boolean() { "boolean" } else { "non-object" }
        ));
        return;
    };
    let beside = |allowed: &str, problems: &mut Vec<String>| {
        for keyword in object.keys() {
            if keyword != allowed && !ANNOTATIONS.contains(&keyword.as_str()) {
                problems.push(format!(
                    "{at}: keyword {keyword} beside {allowed} in a cut node (spec 6.10)"
                ));
            }
        }
    };
    if object.get("$ref").is_some_and(Value::is_string) {
        beside("$ref", problems);
        let target = super::project::deref(schemas, Some(node));
        if target.is_some_and(|target| std::ptr::eq(target, node)) {
            return;
        }
        check_cut(schemas, target, trie, at, depth + 1, problems);
        return;
    }
    if let Some(any_of) = object.get("anyOf") {
        beside("anyOf", problems);
        let branches: &[Value] = any_of.as_array().map_or(&[], Vec::as_slice);
        if branches.len() != 2 || branches.iter().filter(|b| is_bare_null(b)).count() != 1 {
            problems.push(format!(
                "{at}: anyOf in a cut node must be exactly one schema and {{\"type\":\"null\"}}"
            ));
            return;
        }
        check_cut(
            schemas,
            branches.iter().find(|b| !is_bare_null(b)),
            trie,
            at,
            depth + 1,
            problems,
        );
        return;
    }
    let types = types_of(object);
    if types.is_empty() {
        problems.push(format!("{at}: a cut node declares no type (spec 6.10)"));
        return;
    }
    if types.contains(&"array") || object.contains_key("items") {
        for keyword in object.keys() {
            let allowed = ANNOTATIONS.contains(&keyword.as_str())
                || ["type", "items", "minItems", "maxItems"].contains(&keyword.as_str());
            if !allowed {
                problems.push(format!(
                    "{at}: keyword {keyword} in an array cut node (spec 6.10)"
                ));
            }
        }
        if types.iter().any(|t| *t != "array" && *t != "null") {
            problems.push(format!("{at}: an array cut node admits {}", types.join(", ")));
        }
        check_cut(
            schemas,
            object.get("items"),
            trie,
            &format!("{at}[]"),
            depth + 1,
            problems,
        );
        return;
    }
    for keyword in object.keys() {
        let allowed = ANNOTATIONS.contains(&keyword.as_str())
            || ["type", "properties", "required", "additionalProperties"].contains(&keyword.as_str());
        if !allowed {
            problems.push(format!(
                "{at}: keyword {keyword} in an object cut node (spec 6.10)"
            ));
        }
    }
    if types.iter().any(|t| *t != "object" && *t != "null") {
        problems.push(format!("{at}: an object cut node admits {}", types.join(", ")));
    }
    let properties = object.get("properties").and_then(Value::as_object);
    for (member, child) in trie {
        if let Trie::Branch(below) = child {
            check_cut(
                schemas,
                properties.and_then(|properties| properties.get(member)),
                below,
                &format!("{at}.{member}"),
                depth + 1,
                problems,
            );
        }
    }
}

/// The cut-node trie of one output: the decoded `fields.records` segments, then every proper ancestor of a declared path
/// and, for a document that declares 1.1, every declared map itself (its keys are selectable). A 1.0 document keeps a
/// map whole and it is no cut node.
fn cut_trie(doc: &CommandDoc, output: &Output, v11: bool) -> Members {
    let record = record_schema(&doc.schemas, output);
    let mut ancestors = Members::new();
    let declared: Vec<&String> = output
        .fields
        .iter()
        .flat_map(|fields| fields.paths.iter().chain(&fields.keep))
        .collect();
    for path in declared {
        let names = step_names(path);
        let map = v11 && is_map(path_node(&doc.schemas, record, path));
        let take = if map { names.len() } else { names.len().saturating_sub(1) };
        let mut node = &mut ancestors;
        for segment in &names[..take] {
            let entry = node
                .entry(segment.clone())
                .or_insert_with(|| Trie::Branch(Members::new()));
            let Trie::Branch(below) = entry else {
                break;
            };
            node = below;
        }
    }
    let records = output
        .fields
        .as_ref()
        .map(|fields| decode_pointer(&fields.records))
        .unwrap_or_default();
    records.into_iter().rev().fold(ancestors, |below, segment| {
        let mut spine = Members::new();
        spine.insert(segment, Trie::Branch(below));
        spine
    })
}

/// Rule 11: every output with `fields` has projectable cut nodes from the root schema down; `fields.records` is `""` or a
/// pointer that leads through declared object members; `fields.default` is within `fields.paths`; `fields.keep` and
/// `fields.paths` are disjoint; every path is well formed (rule 15) and names a member the record schema declares, through
/// arrays only with `[]`.
fn rule_11(doc: &CommandDoc, v11: bool) -> Option<String> {
    let mut problems = Vec::new();
    for output in &doc.outputs {
        let Some(fields) = &output.fields else {
            continue;
        };
        if !fields.records.is_empty() && !fields.records.starts_with('/') {
            problems.push(format!(
                "fields.records {:?} is neither \"\" nor an RFC 6901 pointer",
                fields.records
            ));
            continue;
        }
        let record = record_schema(&doc.schemas, output);
        let root = doc.schemas.get(output.name());
        let mut at = non_null(&doc.schemas, root);
        for segment in decode_pointer(&fields.records) {
            let member = at
                .and_then(Value::as_object)
                .and_then(|object| object.get("properties"))
                .and_then(Value::as_object)
                .and_then(|properties| properties.get(&segment));
            match member {
                Some(member) => at = non_null(&doc.schemas, Some(member)),
                None => {
                    problems.push(format!(
                        "fields.records {:?} does not lead through declared members",
                        fields.records
                    ));
                    at = None;
                    break;
                }
            }
        }
        // A schema that does not compile is rule 2's; its keywords are not read as cut nodes.
        if at.is_some() && doc.compile_ref(&output.schema_ref).is_ok() {
            let spine = cut_trie(doc, output, v11);
            check_cut(&doc.schemas, root, &spine, output.name(), 0, &mut problems);
        }
        for default in &fields.default {
            if !fields.paths.contains(default) {
                problems.push(format!("fields.default {default} is not in fields.paths"));
            }
        }
        for keep in &fields.keep {
            if fields.paths.contains(keep) {
                problems.push(format!("fields.keep {keep} is also in fields.paths"));
            }
        }
        for path in fields.paths.iter().chain(&fields.keep) {
            if let Some(problem) = field_path_problem(path) {
                problems.push(problem);
            } else if path_node(&doc.schemas, record, path).is_none() {
                problems.push(format!("field path {path} is not declared by the record schema"));
            }
        }
    }
    finding(doc, "11", &problems)
}

// ---- rule 13: validators ----

/// The conditions of a documented line that a validator's `rejects` line must still satisfy (spec 10.13 rule 13): it
/// parses, and has the right values, requirements and constraints, so that only the validator can refuse it.
const BEFORE_THE_VALIDATOR: [&str; 4] = ["grammar", "values", "requirements", "constraints"];

/// Rule 13: every validator names arguments of the command; its `code` is in `errors` and registered with exit 2; each
/// `rejects` line parses and satisfies `required`, `constraints` and every `domain`, and none is an example.
fn rule_13(
    doc: &CommandDoc,
    tool: &str,
    exit_of: &dyn Fn(&str) -> Option<i64>,
    domains: &DomainStates,
) -> Option<String> {
    let names: BTreeSet<&str> = doc.args().map(|arg| arg.name.as_str()).collect();
    let mut problems = Vec::new();
    for (index, validator) in doc.validators.iter().enumerate() {
        for name in &validator.arguments {
            if !names.contains(name.as_str()) {
                problems.push(format!("validator {index} names unknown argument {name}"));
            }
        }
        if !doc.errors.contains(&validator.code) {
            problems.push(format!(
                "validator {index} code {} is not in errors",
                validator.code
            ));
        }
        if let Some(exit) = exit_of(&validator.code).filter(|exit| *exit != 2) {
            problems.push(format!(
                "validator {index} code {} exits {exit}, not 2 (spec 10.8.3)",
                validator.code
            ));
        }
        for line in &validator.rejects {
            if doc.examples.contains(line) {
                problems.push(format!(
                    "validator {index} rejects {line:?}, which is also a valid example"
                ));
            }
            let before = invocation::check_line(doc, tool, line, false)
                .into_iter()
                .filter(|(condition, _)| BEFORE_THE_VALIDATOR.contains(condition))
                .find_map(|(_, problem)| problem);
            if let Some(problem) = before {
                problems.push(format!(
                    "validator {index} rejects {line:?}, which fails before the validator: {problem}"
                ));
            }
        }
    }
    problems.retain(|problem| !domains.hides(problem));
    finding(doc, "13", &problems)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use serde_json::{json, Value};

    use super::{command_findings, group_findings, tree_findings, Findings};
    use crate::claim::Claim;
    use crate::l1::model::{CommandDoc, GroupDoc, Tree};

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    fn load(relative: &str) -> Value {
        let path = fixtures().join(relative);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    /// The exit of every code of the example registries of canon and acme, for rule 13.
    fn exit_of(code: &str) -> Option<i64> {
        let mut exits: BTreeMap<String, i64> = BTreeMap::new();
        for file in ["atip-examples/schema-errors.json", "atip-examples/schema-errors-acme.json"] {
            for entry in load(file)["data"]["errors"].as_array().expect("an errors list") {
                if let (Some(code), Some(exit)) = (entry["code"].as_str(), entry["exit"].as_i64()) {
                    exits.entry(code.to_string()).or_insert(exit);
                }
            }
        }
        exits.get(code).copied()
    }

    /// The tool and the claim of an answer: `meta.tool` and `meta.contractVersion` (absent means 1.0).
    fn identity(answer: &Value) -> (String, Claim) {
        let tool = answer["meta"]["tool"].as_str().unwrap_or("tool").to_string();
        let claim = match answer["meta"]["contractVersion"].as_str() {
            Some("1.2") => Claim::V1_2,
            Some("1.1") => Claim::V1_1,
            _ => Claim::V1_0,
        };
        (tool, claim)
    }

    fn judge_value(answer: &Value, claim: Option<Claim>) -> (CommandDoc, Findings) {
        let (tool, declared) = identity(answer);
        let doc = CommandDoc::parse(&answer["data"]).expect("a command document");
        let findings = command_findings(&doc, &tool, claim.unwrap_or(declared), &exit_of);
        (doc, findings)
    }

    fn judge(relative: &str) -> (CommandDoc, Findings) {
        judge_value(&load(relative), None)
    }

    /// The conditions with a problem.
    fn failing(findings: &Findings) -> Vec<&'static str> {
        findings
            .iter()
            .filter(|(_, problem)| problem.is_some())
            .map(|(condition, _)| *condition)
            .collect()
    }

    fn problem_of<'a>(findings: &'a Findings, condition: &str) -> &'a str {
        findings
            .iter()
            .find(|(name, _)| *name == condition)
            .and_then(|(_, problem)| problem.as_deref())
            .unwrap_or_else(|| panic!("{condition} has no problem in {findings:#?}"))
    }

    fn valid_commands() -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(fixtures().join("schema-docs"))
            .expect("the fixture directory")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with("schema-command-") && name.ends_with(".json"))
            .collect();
        names.sort();
        names
    }

    /// The condition of the rule the document breaks, and the files that break only that rule without the help of
    /// `invocation.rs` (values, requirements, the grammar of an invocation).
    const OWNED: &[(&str, &str)] = &[
        ("schema-path-mismatch", "rule-1"),
        ("schema-dangling-schemaref", "rule-2-refs"),
        ("schema-missing-schemas", "rule-2-refs"),
        ("schema-outputs-share-ref", "rule-2-outputs"),
        ("schema-schemas-not-compiling", "rule-2-compile"),
        ("schema-enum-default-outside", "rule-3-defaults"),
        ("schema-domain-bound-unsafe", "rule-3-domain-profile"),
        ("schema-domain-numeric-keyword-on-string", "rule-3-domain-profile"),
        ("schema-domain-outside-profile", "rule-3-domain-profile"),
        ("schema-domain-ref", "rule-3-domain-profile"),
        ("schema-domain-structural-enum", "rule-3-domain-profile"),
        ("schema-domain-pattern-alternation", "rule-3-domain-pattern"),
        ("schema-domain-pattern-class-intersection", "rule-3-domain-pattern"),
        ("schema-domain-pattern-class-symmetric-difference", "rule-3-domain-pattern"),
        ("schema-domain-pattern-dot", "rule-3-domain-pattern"),
        ("schema-domain-pattern-lookahead", "rule-3-domain-pattern"),
        ("schema-domain-pattern-rust-size-limit", "rule-3-domain-pattern"),
        ("schema-domain-pattern-shorthand", "rule-3-domain-pattern"),
        ("schema-domain-pattern-unanchored", "rule-3-domain-pattern"),
        ("schema-constraint-code-not-in-errors", "rule-4-codes"),
        ("schema-constraint-duplicate", "rule-4-relation-once"),
        ("schema-constraint-in-on-switch", "rule-4-names"),
        ("schema-constraint-unknown-name", "rule-4-names"),
        ("schema-batch-without-batch-output", "rule-5"),
        ("schema-outputs-indistinct", "rule-6"),
        ("schema-short-duplicate", "rule-9"),
        ("schema-fragment-flag-only-property", "rule-10"),
        ("schema-fragment-open", "rule-10"),
        ("schema-fragment-required", "rule-10"),
        ("schema-fields-bracket-identifier", "rule-11"),
        ("schema-fields-cut-node-boolean", "rule-11"),
        ("schema-fields-cut-node-const", "rule-11"),
        ("schema-fields-cut-node-enum", "rule-11"),
        ("schema-fields-cut-node-root-allof", "rule-11"),
        ("schema-fields-cut-node-root-const", "rule-11"),
        ("schema-fields-cut-node-unique-items", "rule-11"),
        ("schema-fields-cut-node-untyped", "rule-11"),
    ];

    /// More files of rules 11 and 13 that do not need `invocation.rs`.
    const OWNED_MORE: &[(&str, &str)] = &[
        ("schema-fields-default-outside", "rule-11"),
        ("schema-fields-keep-overlap", "rule-11"),
        ("schema-fields-map-cut-node-min-properties", "rule-11"),
        ("schema-fields-not-projectable", "rule-11"),
        ("schema-fields-path-array-without-brackets", "rule-11"),
        ("schema-fields-path-malformed", "rule-11"),
        ("schema-fields-path-undeclared", "rule-11"),
        ("schema-fields-records-not-pointer", "rule-11"),
        ("schema-validator-code-exit-1", "rule-13"),
        ("schema-validator-code-not-listed", "rule-13"),
        ("schema-validator-rejects-is-example", "rule-13"),
        ("schema-validator-unknown-argument", "rule-13"),
    ];

    /// Files whose violation is found only through `invocation.rs`: a value against its type or domain, a normalised
    /// value, a constraint on an example, the argument object.
    const OWNED_BY_INVOCATION: &[(&str, &str)] = &[
        ("schema-domain-default-outside", "rule-3-defaults"),
        ("schema-domain-count-default-outside", "rule-3-defaults"),
        ("schema-operand-untyped-default", "rule-3-defaults"),
        ("schema-untyped-default", "rule-3-defaults"),
        ("schema-constraint-in-wrong-type", "rule-4-names"),
        ("schema-input-example-violates-constraint", "rule-7"),
        ("schema-example-fails-arguments-ref", "rule-7"),
        ("schema-example-variadic-without-default", "rule-7"),
        ("schema-validator-rejects-fails-domain", "rule-13"),
    ];

    /// Judges `file` of `schema-docs/invalid` and asserts that exactly `owner` has a problem, naming the command and the
    /// rule. `ignoring` lists the conditions that read `invocation.rs`, which is a stub on this branch.
    fn assert_owned(file: &str, owner: &str, ignoring: &[&str]) {
        let (doc, findings) = judge(&format!("schema-docs/invalid/{file}.json"));
        let failing: Vec<&str> = failing(&findings)
            .into_iter()
            .filter(|condition| *condition == owner || !ignoring.contains(condition))
            .collect();
        assert_eq!(failing, vec![owner], "{file}: {findings:#?}");
        let problem = problem_of(&findings, owner);
        let rule = owner.split('-').nth(1).expect("a rule number");
        assert!(
            problem.starts_with(&format!("{}: rule {rule}: ", doc.command)),
            "{file}: {problem}"
        );
    }

    #[test]
    fn atip_checkl1_rules_valid_commands_hold_every_condition_that_does_not_read_invocation() {
        let names = valid_commands();
        assert_eq!(names.len(), 17, "{names:?}");
        for name in names {
            let (_, findings) = judge(&format!("schema-docs/{name}"));
            let failing: Vec<&str> = failing(&findings)
                .into_iter()
                .filter(|condition| !["rule-7", "rule-13"].contains(condition))
                .collect();
            assert!(failing.is_empty(), "{name}: {findings:#?}");
        }
    }

    #[test]
    #[ignore = "reads invocation.rs (value_problem, arguments_object, check_line), a stub on this branch: run with --ignored once it is merged"]
    fn atip_checkl1_rules_valid_commands_hold_rules_7_and_13() {
        for name in valid_commands() {
            let (_, findings) = judge(&format!("schema-docs/{name}"));
            assert!(failing(&findings).is_empty(), "{name}: {findings:#?}");
        }
    }

    #[test]
    fn atip_checkl1_rules_a_command_reports_the_conditions_its_document_has_something_to_judge_for() {
        let (_, tag) = judge("schema-docs/schema-command-acme-member-tag.json");
        let conditions: Vec<&str> = tag.iter().map(|(condition, _)| *condition).collect();
        assert_eq!(
            conditions,
            [
                "rule-1",
                "rule-2-refs",
                "rule-2-compile",
                "rule-2-outputs",
                "rule-3-defaults",
                "rule-3-domain-profile",
                "rule-3-domain-pattern",
                "rule-3-domain-compile",
                "rule-5",
                "rule-6",
                "rule-7",
                "rule-9",
                "rule-10",
                "rule-13"
            ]
        );
        let (_, close) = judge("schema-docs/schema-command-canon-finding-close.json");
        let conditions: Vec<&str> = close.iter().map(|(condition, _)| *condition).collect();
        for condition in ["rule-4-names", "rule-4-codes", "rule-4-relation-once"] {
            assert!(conditions.contains(&condition), "{conditions:?}");
        }
        let (_, list) = judge("schema-docs/schema-command-acme-member-list.json");
        let conditions: Vec<&str> = list.iter().map(|(condition, _)| *condition).collect();
        assert!(conditions.contains(&"rule-11"), "{conditions:?}");
        assert!(!conditions.contains(&"rule-13") && !conditions.contains(&"rule-7"));
    }

    #[test]
    fn atip_checkl1_rules_each_invalid_example_breaks_exactly_its_rule() {
        for (file, owner) in OWNED.iter().chain(OWNED_MORE) {
            // Rules 7 and 13 read invocation.rs: with the stub they say nothing true about a document, so they are not judged here.
            assert_owned(file, owner, &["rule-7", "rule-13"]);
        }
    }

    #[test]
    #[ignore = "reads invocation.rs, a stub on this branch: run with --ignored once it is merged"]
    fn atip_checkl1_rules_each_invalid_example_breaks_exactly_its_rule_with_invocation() {
        for (file, owner) in OWNED.iter().chain(OWNED_MORE).chain(OWNED_BY_INVOCATION) {
            assert_owned(file, owner, &[]);
        }
    }

    #[test]
    fn atip_checkl1_rules_a_declared_map_is_a_cut_node_only_for_a_claim_of_1_1() {
        let answer = load("schema-docs/invalid/schema-fields-map-cut-node-min-properties.json");
        let (_, findings) = judge_value(&answer, Some(Claim::V1_1));
        assert!(problem_of(&findings, "rule-11").contains("minProperties"));
        let (_, findings) = judge_value(&answer, Some(Claim::V1_0));
        assert!(
            !failing(&findings).contains(&"rule-11"),
            "a 1.0 claim keeps a map whole: {findings:#?}"
        );
    }

    fn mutated(relative: &str, change: impl FnOnce(&mut Value)) -> Findings {
        let mut answer = load(relative);
        change(&mut answer["data"]);
        judge_value(&answer, None).1
    }

    #[test]
    fn atip_checkl1_rules_an_event_ref_that_names_no_schema_is_rule_2() {
        let findings = mutated("schema-docs/schema-command-acme-build.json", |data| {
            data["outputs"][0]["eventRef"] = json!("#/schemas/Nope");
        });
        assert_eq!(failing(&findings), vec!["rule-2-refs"]);
        assert!(problem_of(&findings, "rule-2-refs").contains("outputs[0].eventRef"));
    }

    #[test]
    fn atip_checkl1_rules_a_schema_that_is_false_is_a_member_of_schemas() {
        // `acme build` refers to a `false` schema: membership, not truthiness (spec 10.13 rule 2).
        let (doc, findings) = judge("schema-docs/schema-command-acme-build.json");
        assert!(doc.schemas.values().any(|schema| *schema == json!(false)));
        assert!(failing(&findings).is_empty(), "{findings:#?}");
    }

    #[test]
    fn atip_checkl1_rules_a_batch_output_without_input_batch_is_rule_5() {
        let findings = mutated("schema-docs/schema-command-acme-member-tag.json", |data| {
            data["input"]["batch"] = json!(false);
        });
        assert!(failing(&findings).contains(&"rule-5"), "{findings:#?}");
        assert!(problem_of(&findings, "rule-5").contains("input.batch is not true"));
    }

    #[test]
    fn atip_checkl1_rules_two_outputs_with_neither_when_nor_batch_are_rule_6() {
        let findings = mutated("schema-docs/schema-command-acme-member-tag.json", |data| {
            let second = data["outputs"][1].as_object_mut().expect("an output");
            second.remove("when");
            second.remove("batch");
        });
        assert!(failing(&findings).contains(&"rule-6"), "{findings:#?}");
    }

    #[test]
    fn atip_checkl1_rules_an_operand_named_like_an_option_is_rule_9() {
        let findings = mutated("schema-docs/schema-command-acme-member-show.json", |data| {
            data["operands"][0]["name"] = json!("json");
        });
        assert!(problem_of(&findings, "rule-9").contains("duplicate argument name json"));
    }

    #[test]
    fn atip_checkl1_rules_a_relation_stated_in_another_order_with_another_code_is_stated_twice() {
        let findings = mutated("schema-docs/schema-command-canon-finding-close.json", |data| {
            let constraints = data["constraints"].as_array_mut().expect("constraints");
            constraints.push(json!({"rule": "atMostOne", "names": ["no-promote", "promote"], "code": "USAGE"}));
        });
        assert_eq!(failing(&findings), vec!["rule-4-relation-once"], "{findings:#?}");
        assert!(problem_of(&findings, "rule-4-relation-once")
            .ends_with("constraint 3 repeats constraint 2 (spec 10.8.2)"));
    }

    #[test]
    fn atip_checkl1_rules_a_validator_code_registered_with_exit_1_is_rule_13_and_an_unregistered_one_is_not() {
        let mut answer = load("schema-docs/schema-command-canon-purge.json");
        let (tool, claim) = identity(&answer);
        answer["data"]["validators"][0]["code"] = json!("WRITE_FAILED");
        answer["data"]["errors"]
            .as_array_mut()
            .expect("errors")
            .push(json!("WRITE_FAILED"));
        let doc = CommandDoc::parse(&answer["data"]).expect("a command");
        let findings = command_findings(&doc, &tool, claim, &exit_of);
        assert!(problem_of(&findings, "rule-13").contains("exits 1, not 2"));
        let findings = command_findings(&doc, &tool, claim, &|_| None);
        assert!(!failing(&findings).contains(&"rule-13"), "an unregistered code is not judged: {findings:#?}");
    }

    #[test]
    fn atip_checkl1_rules_a_domain_that_does_not_compile_is_the_only_rule_3_problem_it_causes() {
        let findings = mutated("schema-docs/schema-command-acme-filter-test.json", |data| {
            data["operands"][0]["domain"] = json!({"type": "bogus"});
        });
        let failing: Vec<&str> = failing(&findings)
            .into_iter()
            .filter(|condition| !["rule-7", "rule-13"].contains(condition))
            .collect();
        assert_eq!(failing, vec!["rule-3-domain-compile"], "{findings:#?}");
    }

    // ---- trees and groups ----

    fn tree_answer(relative: &str) -> (Value, Tree) {
        let answer = load(relative);
        let tree = Tree::parse(&answer["data"]).expect("a tree");
        (answer, tree)
    }

    fn holding(findings: &super::Findings) -> bool {
        findings.iter().all(|(_, problem)| problem.is_none())
    }

    #[test]
    fn atip_checkl1_rules_the_example_trees_hold_every_condition() {
        for file in ["schema-tree.json", "schema-tree-acme.json"] {
            let (answer, tree) = tree_answer(&format!("schema-docs/{file}"));
            let findings = tree_findings(&answer, &tree, &tree.level);
            let conditions: Vec<&str> = findings.iter().map(|(c, _)| *c).collect();
            assert_eq!(conditions, ["level", "groups-first", "unique-ids", "entry-path"], "{file}");
            assert!(holding(&findings), "{file}: {findings:#?}");
        }
        // A search answer lists matches, not the tree: its order is not judged.
        let (answer, tree) = tree_answer("schema-docs/schema-tree-search.json");
        let findings = tree_findings(&answer, &tree, &tree.level);
        let conditions: Vec<&str> = findings.iter().map(|(c, _)| *c).collect();
        assert_eq!(conditions, ["level", "unique-ids", "entry-path"]);
        assert!(holding(&findings));
    }

    #[test]
    fn atip_checkl1_rules_a_tree_at_or_above_the_claimed_level_meets_it_and_another_fails_only_level() {
        let (answer, tree) = tree_answer("schema-docs/schema-tree.json");
        // The example tree declares L2: it meets an L1 claim (levels are cumulative) and fails an unknown one.
        assert_eq!(tree.level, "L2");
        assert!(tree_findings(&answer, &tree, "L1")[0].1.is_none());
        let findings = tree_findings(&answer, &tree, "L3");
        assert!(findings[0].1.is_some(), "{findings:#?}");
        assert!(findings[1..].iter().all(|(_, problem)| problem.is_none()));
    }

    #[test]
    fn atip_checkl1_rules_a_command_listed_before_its_group_fails_only_groups_first() {
        let (mut answer, _) = tree_answer("schema-docs/schema-tree.json");
        let commands = answer["data"]["commands"].as_array_mut().expect("commands");
        let group = commands.remove(0);
        assert_eq!(group["kind"], "group");
        commands.push(group);
        let tree = Tree::parse(&answer["data"]).expect("a tree");
        let findings = tree_findings(&answer, &tree, &tree.level);
        let failing: Vec<&str> = findings
            .iter()
            .filter(|(_, problem)| problem.is_some())
            .map(|(condition, _)| *condition)
            .collect();
        assert_eq!(failing, ["groups-first"], "{findings:#?}");
        assert!(problem_of(&findings, "groups-first").contains("finding add is listed before its group finding"));
    }

    #[test]
    fn atip_checkl1_rules_a_repeated_id_fails_only_unique_ids() {
        let (mut answer, _) = tree_answer("schema-docs/schema-tree.json");
        let commands = answer["data"]["commands"].as_array_mut().expect("commands");
        let last = commands.last().cloned().expect("an entry");
        commands.push(last);
        let tree = Tree::parse(&answer["data"]).expect("a tree");
        let findings = tree_findings(&answer, &tree, &tree.level);
        let failing: Vec<&str> = findings
            .iter()
            .filter(|(_, problem)| problem.is_some())
            .map(|(condition, _)| *condition)
            .collect();
        assert_eq!(failing, ["unique-ids"], "{findings:#?}");
    }

    #[test]
    fn atip_checkl1_rules_an_entry_whose_command_is_not_its_path_fails_only_entry_path() {
        let (mut answer, _) = tree_answer("schema-docs/schema-tree.json");
        answer["data"]["commands"][1]["command"] = json!("finding adds");
        let tree = Tree::parse(&answer["data"]).expect("a tree");
        let findings = tree_findings(&answer, &tree, &tree.level);
        let failing: Vec<&str> = findings
            .iter()
            .filter(|(_, problem)| problem.is_some())
            .map(|(condition, _)| *condition)
            .collect();
        assert!(failing.contains(&"entry-path"), "{findings:#?}");
    }

    fn group_answer(relative: &str) -> (Value, GroupDoc) {
        let answer = load(relative);
        let group = GroupDoc::parse(&answer["data"]).expect("a group");
        (answer, group)
    }

    #[test]
    fn atip_checkl1_rules_the_example_group_has_the_trees_children() {
        let (_, tree) = tree_answer("schema-docs/schema-tree.json");
        let (_, group) = group_answer("schema-docs/schema-group.json");
        let findings = group_findings(&group, &tree);
        let conditions: Vec<&str> = findings.iter().map(|(c, _)| *c).collect();
        assert_eq!(conditions, ["group-path", "group-children"]);
        assert!(holding(&findings), "{findings:#?}");
    }

    #[test]
    fn atip_checkl1_rules_a_group_whose_command_is_not_its_path_fails_only_group_path() {
        let (_, tree) = tree_answer("schema-docs/schema-tree.json");
        let (_, group) = group_answer("schema-docs/invalid/schema-group-id-mismatch.json");
        let findings = group_findings(&group, &tree);
        assert!(problem_of(&findings, "group-path").contains("group \"findings\" is not its path \"finding\""));
        assert!(findings[1].1.is_none(), "{findings:#?}");
    }

    #[test]
    fn atip_checkl1_rules_a_group_that_lists_other_children_than_the_tree_fails_only_group_children() {
        let (_, tree) = tree_answer("schema-docs/schema-tree.json");
        let (mut answer, _) = group_answer("schema-docs/schema-group.json");
        let removed = answer["data"]["commands"].as_array_mut().expect("commands").remove(0);
        let group = GroupDoc::parse(&answer["data"]).expect("a group");
        let findings = group_findings(&group, &tree);
        assert!(findings[0].1.is_none());
        assert!(problem_of(&findings, "group-children")
            .contains(&format!("the tree's child {} is not listed", removed["command"].as_str().expect("a command"))));

        let (mut answer, _) = group_answer("schema-docs/schema-group.json");
        answer["data"]["commands"].as_array_mut().expect("commands").push(json!(
            {"command": "finding purge", "path": ["finding", "purge"], "kind": "command", "effect": "write"}
        ));
        let group = GroupDoc::parse(&answer["data"]).expect("a group");
        let findings = group_findings(&group, &tree);
        assert!(problem_of(&findings, "group-children").contains("finding purge is not a child of finding in the tree"));
    }
}
