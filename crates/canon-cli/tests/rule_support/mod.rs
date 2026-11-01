//! What `tests/arg_rules.rs` drives the declared constraints, domains and validators with (0.34 `canon-0-34-atip-constraints`).
//!
//! - [`parse_call`] reads a command line against a command document (spec 9.12), the way a checker that knows only the
//!   document does: which operands and options it gives, with which values;
//! - [`Solver`] finds, for one rule, a set of arguments that breaks that rule and no other (or satisfies it while
//!   exercising it), by looking at every set of the arguments the rules name;
//! - [`Provision`] is a repo the command's example runs in (the story of `tests/harness`), copied for every call;
//! - [`run_call`] runs a call through the built binary and reads the envelope: the exit, the code, the field and whatever
//!   the call wrote.
//!
//! Every function takes the documents the generator builds as `serde_json::Value`, so a seeded wrong document is as
//! easy to test as the real one.

#![expect(dead_code, reason = "each test binary that includes this module uses a subset")]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Output;

use atip::constraint::Given;
use atip::doc::{ConditionDoc, ConstraintDoc, RuleKind};
use serde_json::{json, Value};
use tempfile::TempDir;

use super::harness::{self, canon_in, copy_dir, tree};

pub mod sites;
mod samples_authoring;
mod samples_evidence;
mod samples_reads;
mod sites_authoring;
mod sites_evidence;
mod sites_reads;

// ------------------------------------------------------------------------------------------------ the documents

/// Every command document of the spec, in tree order.
pub fn docs() -> Vec<&'static Value> {
    harness::leaves()
}

fn arguments(doc: &Value) -> Vec<&Value> {
    doc["options"].as_array().into_iter().flatten().chain(doc["operands"].as_array().into_iter().flatten()).collect()
}

pub fn argument<'a>(doc: &'a Value, name: &str) -> Option<&'a Value> {
    arguments(doc).into_iter().find(|argument| argument["name"] == name)
}

pub fn is_operand(doc: &Value, name: &str) -> bool {
    doc["operands"].as_array().into_iter().flatten().any(|operand| operand["name"] == name)
}

pub fn constraints(doc: &Value) -> Vec<ConstraintDoc> {
    doc["constraints"].as_array().into_iter().flatten().map(constraint_of).collect()
}

/// A `constraints` member read back as the typed rule.
pub fn constraint_of(value: &Value) -> ConstraintDoc {
    let strings = |member: &Value| member.as_array().map(|items| items.iter().filter_map(|item| item.as_str().map(str::to_string)).collect::<Vec<_>>());
    let rule = match value["rule"].as_str().unwrap_or_default() {
        "atLeastOne" => RuleKind::AtLeastOne,
        "exactlyOne" => RuleKind::ExactlyOne,
        "atMostOne" => RuleKind::AtMostOne,
        "requires" => RuleKind::Requires,
        "forbids" => RuleKind::Forbids,
        other => panic!("unknown rule {other}"),
    };
    ConstraintDoc {
        rule,
        names: strings(&value["names"]),
        when: value.get("if").map(|when| ConditionDoc { name: when["name"].as_str().unwrap_or_default().to_string(), values: when.get("in").and_then(Value::as_array).cloned() }),
        then: strings(&value["then"]),
        code: value["code"].as_str().map(str::to_string),
        help: value["help"].as_str().map(str::to_string),
        extensions: BTreeMap::new(),
    }
}

/// The code a violation of `rule` produces.
pub fn code_of(rule: &ConstraintDoc) -> String {
    rule.code.clone().unwrap_or_else(|| "USAGE".to_string())
}

/// A readable name of a rule, for messages.
pub fn label(rule: &ConstraintDoc) -> String {
    serde_json::to_string(rule).unwrap_or_default()
}

// ------------------------------------------------------------------------------------------------ reading a call

/// What a command line gives, read against the document.
#[derive(Debug, Clone, Default)]
pub struct Parsed {
    /// Each given argument with its values, normalised (spec 11.5): an integer is a number, a switch is `true`.
    pub given: Given,
    /// The values as typed, by argument name.
    pub text: BTreeMap<String, Vec<String>>,
}

fn normal(argument: &Value, text: &str) -> Value {
    match argument["valueType"].as_str() {
        Some("integer" | "count") => text.parse::<i64>().map_or_else(|_| json!(text), |number| json!(number)),
        _ => json!(text),
    }
}

/// Reads `words` (a command line without `canon`, ending with `--json` or not) against `doc`: the command path first,
/// then operands and options by the grammar of spec 9.12.
pub fn parse_call(doc: &Value, words: &[String]) -> Result<Parsed, String> {
    let path = doc["path"].as_array().map_or(0, Vec::len);
    let mut parsed = Parsed::default();
    let operands: Vec<&Value> = doc["operands"].as_array().into_iter().flatten().collect();
    let mut operand_at = 0;
    let mut at = path;
    let mut only_operands = false;
    while at < words.len() {
        let word = &words[at];
        at += 1;
        if !only_operands && word == "--" {
            only_operands = true;
            continue;
        }
        if !only_operands && word.starts_with('-') && word.len() > 1 {
            let (name, inline) = match word.strip_prefix("--") {
                Some(rest) => match rest.split_once('=') {
                    Some((name, value)) => (name.to_string(), Some(value.to_string())),
                    None => (rest.to_string(), None),
                },
                None => {
                    let short = &word[1..];
                    let found = doc["options"].as_array().into_iter().flatten().find(|option| option["short"] == short).ok_or_else(|| format!("the option `{word}` is not an option of `{}`", doc["command"]))?;
                    (found["name"].as_str().unwrap_or_default().to_string(), None)
                }
            };
            let option = doc["options"].as_array().into_iter().flatten().find(|option| option["name"] == name.as_str()).ok_or_else(|| format!("`--{name}` is not an option of `{}`", doc["command"]))?;
            let valueless = matches!(option["valueType"].as_str(), Some("switch" | "count"));
            if valueless {
                parsed.given.entry(name.clone()).or_default().push(json!(true));
                parsed.text.entry(name).or_default().push("true".to_string());
            } else {
                let value = match inline {
                    Some(value) => value,
                    None => {
                        let value = words.get(at).ok_or_else(|| format!("`--{name}` has no value"))?.clone();
                        at += 1;
                        value
                    }
                };
                parsed.given.entry(name.clone()).or_default().push(normal(option, &value));
                parsed.text.entry(name).or_default().push(value);
            }
        } else {
            let operand = operands.get(operand_at.min(operands.len().saturating_sub(1))).ok_or_else(|| format!("`{word}` is an operand and `{}` takes none", doc["command"]))?;
            let name = operand["name"].as_str().unwrap_or_default().to_string();
            if operand["variadic"] != true {
                operand_at += 1;
            }
            parsed.given.entry(name.clone()).or_default().push(normal(operand, word));
            parsed.text.entry(name).or_default().push(word.clone());
        }
    }
    Ok(parsed)
}

/// What a call breaks of everything a document says without reading state: a required argument that is missing, a
/// value outside its domain, a rule. Empty when the call is one the document accepts.
pub fn document_problems(doc: &Value, words: &[String]) -> Vec<String> {
    let parsed = match parse_call(doc, words) {
        Ok(parsed) => parsed,
        Err(problem) => return vec![problem],
    };
    let mut problems = Vec::new();
    for argument in arguments(doc) {
        let name = argument["name"].as_str().unwrap_or_default();
        if argument["required"] == true && !parsed.given.contains_key(name) {
            problems.push(format!("`{name}` is required and missing"));
        }
        if let (Some(domain), Some(values)) = (argument.get("domain"), parsed.given.get(name)) {
            for value in values {
                if !atip::domain::admits(domain, value).unwrap_or(false) {
                    problems.push(format!("{value} is outside the domain of `{name}`: {domain}"));
                }
            }
        }
        if let Some(values) = parsed.given.get(name) {
            if argument["valueType"] == "enum" {
                let members: Vec<&str> = argument["possibleValues"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                for value in values {
                    if !value.as_str().is_some_and(|text| members.contains(&text)) {
                        problems.push(format!("{value} is not a possible value of `{name}`"));
                    }
                }
            }
        }
    }
    for (at, field) in atip::constraint::violations(&constraints(doc), &parsed.given) {
        problems.push(format!("breaks {} (field {field})", label(&constraints(doc)[at])));
    }
    problems
}

// -------------------------------------------------------------------------------------------------- samples

/// A valid value of the argument, as a command line writes it: the one the example uses when it gives the argument,
/// else one by value name, else one by value type.
pub fn sample(doc: &Value, example: &[String], argument: &Value) -> String {
    let name = argument["name"].as_str().unwrap_or_default();
    let path = doc["command"].as_str().unwrap_or_default();
    if let Some(value) = SAMPLES.iter().chain(samples_evidence::SAMPLES).chain(samples_authoring::SAMPLES).chain(samples_reads::SAMPLES).find(|(command, argument, _)| *command == path && *argument == name).map(|(_, _, value)| (*value).to_string()) {
        return value;
    }
    if let Ok(parsed) = parse_call(doc, example) {
        if let Some(value) = parsed.text.get(name).and_then(|values| values.first()) {
            return value.clone();
        }
    }
    if let Some(members) = argument["possibleValues"].as_array() {
        if let Some(first) = members.first().and_then(Value::as_str) {
            return first.to_string();
        }
    }
    let by_name = match argument["valueName"].as_str().unwrap_or_default() {
        "CHANGE_ID" => "add-login",
        "SUBJECT_ID" => "demo",
        "ROLE" => "reviewer",
        "PROJECT_ID" => "root",
        "SCENARIO_ID" => "auth.login.01",
        "TASK_ID" => "add-login#1",
        "RUN_ID" | "REVIEW_ID" | "STRATEGY_ID" => "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "SESSION_ID" => "s1",
        "TIMESTAMP" => "2030-01-01",
        "ACTOR_ID" => "alice",
        "N" => "2",
        "REV" => "HEAD",
        "FORMAT:PATH" => "junit:report.xml",
        "NAME@VERSION" => "reviewer@v1",
        "PATH_OR_EFFECT" => "effect:cache",
        "DIR" | "FILE" | "PATH" => "sample.txt",
        "DIGEST" => "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        _ => "",
    };
    if !by_name.is_empty() {
        return by_name.to_string();
    }
    match argument["valueType"].as_str() {
        Some("integer") => "2".to_string(),
        Some("number") => "1.5".to_string(),
        Some("path") => "sample.txt".to_string(),
        _ => "text".to_string(),
    }
}

/// Values a command needs that neither its example nor its value name give: `(command, argument, value)`.
const SAMPLES: &[(&str, &str, &str)] = &[
    ("divergence accept", "role", "reviewer"),
    ("review scenario", "upstream-ref", "HEAD"),
    ("review scenario", "original-spec-ref", "HEAD"),
    ("review change", "upstream-ref", "HEAD"),
    ("review change", "original-spec-ref", "HEAD"),
    ("schema", "error", "USAGE"),
    ("brief approve", "digest", "add-login=sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
    ("schema", "warning", "OLD_LAYOUT_FOUND"),
];

/// A value of `argument` that is not in `excluded` (an enum's other member, a string that is none of them).
pub fn other_value(doc: &Value, example: &[String], argument: &Value, excluded: &BTreeSet<String>) -> Option<String> {
    if let Some(members) = argument["possibleValues"].as_array() {
        return members.iter().filter_map(Value::as_str).find(|member| !excluded.contains(*member)).map(str::to_string);
    }
    let first = sample(doc, example, argument);
    if !excluded.contains(&first) {
        return Some(first);
    }
    (0..).map(|n| format!("{first}{n}")).find(|candidate| !excluded.contains(candidate))
}

// ---------------------------------------------------------------------------------------------- the solver

/// How a name is given: absent, or with one value.
#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Absent,
    Given(String),
}

/// The arguments a call gives: name to values, as typed.
pub type Assignment = BTreeMap<String, Vec<String>>;

/// What a search for a call is after.
#[derive(Debug, Clone, Copy)]
pub enum Goal {
    /// The call breaks rule `at` and, when possible, nothing else.
    Violate(usize),
    /// The call breaks nothing and exercises rule `at`: gives what makes it apply and meets it.
    Satisfy(usize),
}

/// The call the solver found: the arguments, and the other rules it breaks (empty for a satisfying call).
#[derive(Debug, Clone)]
pub struct Found {
    pub assignment: Assignment,
    pub also_breaks: Vec<usize>,
}

fn names_of(rules: &[ConstraintDoc]) -> BTreeSet<String> {
    rules.iter().flat_map(atip::constraint::members).collect()
}

/// The values a name may take in a search: each value an `in` list names, and one value no list names.
fn candidates(doc: &Value, example: &[String], rules: &[ConstraintDoc], name: &str) -> Vec<String> {
    let Some(argument) = argument(doc, name) else { return vec![] };
    if matches!(argument["valueType"].as_str(), Some("switch")) {
        return vec!["true".to_string()];
    }
    let listed: BTreeSet<String> = rules.iter().filter_map(|rule| rule.when.as_ref()).filter(|when| when.name == name).flat_map(|when| when.values.iter().flatten()).map(|value| value.as_str().map_or_else(|| value.to_string(), str::to_string)).collect();
    if listed.is_empty() {
        return vec![sample(doc, example, argument)];
    }
    let mut values: Vec<String> = listed.iter().cloned().collect();
    if let Some(other) = other_value(doc, example, argument, &listed) {
        values.push(other);
    }
    values
}

fn given_of(doc: &Value, chosen: &BTreeMap<String, State>) -> Given {
    chosen
        .iter()
        .filter_map(|(name, state)| match state {
            State::Absent => None,
            State::Given(text) => {
                let value = argument(doc, name).map_or_else(|| json!(text), |argument| if argument["valueType"] == "switch" { json!(true) } else { normal(argument, text) });
                Some((name.clone(), vec![value]))
            }
        })
        .collect()
}

/// Whether `given` exercises the rule: it holds its `if`, or names enough of its `names`.
fn exercised(rule: &ConstraintDoc, given: &Given) -> bool {
    let named = rule.names.iter().flatten().filter(|name| given.contains_key(*name)).count();
    match rule.rule {
        RuleKind::AtLeastOne | RuleKind::ExactlyOne | RuleKind::AtMostOne => named >= 1,
        RuleKind::Requires | RuleKind::Forbids => {
            let Some(when) = &rule.when else { return false };
            match (given.get(&when.name), &when.values) {
                (None, _) => false,
                (Some(_), None) => true,
                (Some(values), Some(wanted)) => values.iter().any(|value| wanted.iter().any(|candidate| candidate == value || candidate.as_str().zip(value.as_str()).is_some_and(|(a, b)| a == b))),
            }
        }
    }
}

/// Finds a call for `goal` among every set of the arguments the rules name (each with every value an `in` list needs),
/// together with the arguments the command cannot do without (`required`). The smallest wins.
pub fn solve(doc: &Value, example: &[String], rules: &[ConstraintDoc], goal: Goal) -> Option<Found> {
    let names: Vec<String> = names_of(rules).into_iter().collect();
    let forced: Vec<String> = arguments(doc).into_iter().filter(|argument| argument["required"] == true).filter_map(|argument| argument["name"].as_str().map(str::to_string)).collect();
    let options: Vec<Vec<State>> = names
        .iter()
        .map(|name| {
            let mut states = vec![State::Absent];
            states.extend(candidates(doc, example, rules, name).into_iter().map(State::Given));
            if forced.contains(name) {
                states.remove(0);
            }
            states
        })
        .collect();
    let mut best: Option<(usize, Found)> = None;
    let mut chosen: BTreeMap<String, State> = BTreeMap::new();
    let mut counter = vec![0usize; names.len()];
    loop {
        chosen.clear();
        for (at, name) in names.iter().enumerate() {
            chosen.insert(name.clone(), options[at][counter[at]].clone());
        }
        let given = given_of(doc, &chosen);
        let broken: Vec<usize> = atip::constraint::violations(rules, &given).into_iter().map(|(at, _)| at).collect();
        let size = given.len();
        let fits = match goal {
            Goal::Violate(target) => broken.contains(&target),
            Goal::Satisfy(target) => broken.is_empty() && exercised(&rules[target], &given),
        };
        if fits {
            let others: Vec<usize> = match goal {
                Goal::Violate(target) => broken.iter().copied().filter(|at| *at != target).collect(),
                Goal::Satisfy(_) => Vec::new(),
            };
            // A call that breaks only the target beats one that breaks more; among those, the smaller call.
            let rank = others.len() * 1000 + size;
            if best.as_ref().is_none_or(|(known, _)| rank < *known) {
                let assignment: Assignment = chosen
                    .iter()
                    .filter_map(|(name, state)| match state {
                        State::Absent => None,
                        State::Given(text) => Some((name.clone(), vec![text.clone()])),
                    })
                    .collect();
                best = Some((rank, Found { assignment, also_breaks: others }));
            }
        }
        // The next set.
        let mut at = 0;
        loop {
            if at == counter.len() {
                return best.map(|(_, found)| found);
            }
            counter[at] += 1;
            if counter[at] < options[at].len() {
                break;
            }
            counter[at] = 0;
            at += 1;
        }
    }
}

/// The words of a call (without `canon`, without `--json`): the command path, the operands in order, then the options.
/// An argument the assignment does not name keeps the value the example gives when it is required, else is left out.
pub fn words_of(doc: &Value, example: &[String], assignment: &Assignment) -> Vec<String> {
    let mut words: Vec<String> = doc["path"].as_array().into_iter().flatten().filter_map(|word| word.as_str().map(str::to_string)).collect();
    let mut given = assignment.clone();
    for argument in arguments(doc) {
        let name = argument["name"].as_str().unwrap_or_default().to_string();
        if argument["required"] == true && !given.contains_key(&name) {
            given.insert(name, vec![sample(doc, example, argument)]);
        }
    }
    let mut operand_words: Vec<String> = Vec::new();
    for operand in doc["operands"].as_array().into_iter().flatten() {
        if let Some(values) = given.get(operand["name"].as_str().unwrap_or_default()) {
            operand_words.extend(values.iter().cloned());
        }
    }
    let mut option_words: Vec<String> = Vec::new();
    for option in doc["options"].as_array().into_iter().flatten() {
        let name = option["name"].as_str().unwrap_or_default();
        let Some(values) = given.get(name) else { continue };
        for value in values {
            if matches!(option["valueType"].as_str(), Some("switch" | "count")) {
                option_words.push(format!("--{name}"));
            } else {
                option_words.push(format!("--{name}={value}"));
            }
        }
    }
    if operand_words.iter().any(|word| word.starts_with('-')) {
        // An operand that starts with a dash is one only after `--`.
        words.extend(option_words);
        words.push("--".to_string());
        words.extend(operand_words);
    } else {
        words.extend(operand_words);
        words.extend(option_words);
    }
    words
}

// ------------------------------------------------------------------------------------------------ running

/// A repo a command's example runs in (the story of `tests/harness`, with whatever state the command needs), kept
/// as a template that every call copies.
pub struct Provision {
    pub template: PathBuf,
    pub envs: Vec<(String, String)>,
    /// The example's command line (without `canon`, with `--json`), as the story writes it.
    pub example: Vec<String>,
    _case: harness::Case,
}

/// The provisioned repo of the command `path`, built from the fixture at `base`.
pub fn provision(path: &str, base: &Path, home: &Path) -> Provision {
    let case = harness::case(path, base, home);
    // A writer refreshes a stale scenario index before it reads its own arguments, and that is the writer's doing, not the
    // refusal's: the template starts with the index current, so a refused call is seen to write nothing.
    let envs: Vec<(&str, &str)> = case.envs.iter().map(|(key, value)| (key.as_str(), value.as_str())).chain(std::iter::once(("CANON_ROLE", "implementer"))).collect();
    let _ = canon_in(&case.dir, home, &["inventory".to_string(), "sync".to_string(), "--json".to_string()], &envs);
    Provision { template: case.dir.clone(), envs: case.envs.clone(), example: case.argv.clone(), _case: case }
}

/// What a call did.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub exit: i32,
    pub ok: bool,
    pub code: Option<String>,
    pub field: Option<String>,
    pub message: Option<String>,
    /// The paths the call changed in the repo.
    pub changed: Vec<PathBuf>,
    pub stdout: String,
}

/// Runs `words` (a command line without `canon`) with `--json` in a copy of the provisioned repo.
pub fn run_call(provision: &Provision, home: &Path, words: &[String], role: Option<&str>) -> Outcome {
    let scratch = TempDir::new().expect("a scratch directory");
    let repo = scratch.path().join("repo");
    copy_dir(&provision.template, &repo);
    let before = tree(&repo);
    let mut argv = words.to_vec();
    argv.push("--json".to_string());
    let mut envs: Vec<(&str, &str)> = provision.envs.iter().map(|(key, value)| (key.as_str(), value.as_str())).collect();
    if let Some(role) = role {
        envs.push(("CANON_ROLE", role));
    }
    let output: Output = canon_in(&repo, home, &argv, &envs);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let document: Value = serde_json::from_str(&stdout).unwrap_or(Value::Null);
    Outcome {
        exit: output.status.code().unwrap_or(-1),
        ok: document["ok"] == true,
        code: document["error"]["code"].as_str().map(str::to_string),
        field: document["error"]["field"].as_str().map(str::to_string),
        message: document["error"]["message"].as_str().map(str::to_string),
        changed: harness::changed(&before, &tree(&repo)),
        stdout,
    }
}

/// The role a call declares (`CANON_ROLE`): an agent role, except for the human's own acts, which refuse one
/// (`HUMAN_ONLY`) unless the call relays the human's answer.
pub fn declared_role(path: &str, assignment: &Assignment) -> Option<&'static str> {
    match path {
        "brief approve" | "policy approve" | "divergence accept" | "migrate" if !assignment.contains_key("relayed-via") => None,
        _ => Some("implementer"),
    }
}

// ------------------------------------------------------------------------------------------- the parser, in process

/// Whether clap alone (no handler, no repo) refuses `words` (a command line without `canon`), and what it names:
/// `Err(text)` holds the argument clap blames and its message.
pub fn clap_refusal(words: &[String]) -> Option<(String, String)> {
    use clap::error::ContextKind;
    static ROOT: std::sync::LazyLock<clap::Command> = std::sync::LazyLock::new(|| {
        let mut root = canon_cli::cli::command();
        root.build();
        root
    });
    let argv = std::iter::once("canon".to_string()).chain(words.iter().cloned());
    match ROOT.clone().try_get_matches_from(argv) {
        Ok(_) => None,
        Err(error) => {
            let blamed = error.get(ContextKind::InvalidArg).map(ToString::to_string).unwrap_or_default();
            Some((blamed, error.to_string()))
        }
    }
}

/// The arguments a document's call gives, as an assignment: what the call typed.
pub fn assignment_of(parsed: &Parsed) -> Assignment {
    parsed.text.clone()
}

/// The explicitly given arguments of an assignment, normalised (spec 11.5).
pub fn given_from(doc: &Value, assignment: &Assignment) -> Given {
    assignment
        .iter()
        .map(|(name, texts)| {
            let values = texts.iter().map(|text| argument(doc, name).map_or_else(|| json!(text), |argument| if argument["valueType"] == "switch" { json!(true) } else { normal(argument, text) })).collect();
            (name.clone(), values)
        })
        .collect()
}

/// Mends an assignment until the rules hold, never touching `keep` (the argument under test): a missing name that a
/// `requires` or an `atLeastOne` wants is added, one a `forbids` or an `atMostOne` refuses is dropped.
pub fn repair(doc: &Value, example: &[String], rules: &[ConstraintDoc], assignment: &mut Assignment, keep: &str) -> Result<(), String> {
    for _ in 0..12 {
        let given = given_from(doc, assignment);
        let broken = atip::constraint::violations(rules, &given);
        let Some((at, _)) = broken.first() else { return Ok(()) };
        let rule = &rules[*at];
        let names = rule.names.clone().unwrap_or_default();
        let then = rule.then.clone().unwrap_or_default();
        match rule.rule {
            RuleKind::Requires | RuleKind::AtLeastOne | RuleKind::ExactlyOne if given.is_empty() || rule.rule != RuleKind::ExactlyOne || names.iter().all(|name| !given.contains_key(name)) => {
                let wanted: Vec<String> = if rule.rule == RuleKind::Requires { then } else { names.iter().take(1).cloned().collect() };
                for name in wanted {
                    let argument = argument(doc, &name).ok_or_else(|| format!("`{name}` is not an argument"))?;
                    assignment.entry(name).or_insert_with(|| vec![sample(doc, example, argument)]);
                }
            }
            RuleKind::Forbids => {
                let victims: Vec<String> = then.iter().filter(|name| *name != keep).cloned().collect();
                if victims.is_empty() {
                    if let Some(when) = &rule.when {
                        assignment.remove(&when.name);
                    }
                }
                for victim in victims {
                    assignment.remove(&victim);
                }
            }
            _ => {
                // atMostOne, or an exactlyOne with several given: keep the one under test, else the first.
                let mut kept = false;
                for name in &names {
                    if !given.contains_key(name) {
                        continue;
                    }
                    if (name == keep && !kept) || (!kept && !names.contains(&keep.to_string())) {
                        kept = true;
                    } else {
                        assignment.remove(name);
                    }
                }
            }
        }
    }
    Err(format!("the rules of `{}` do not settle for {assignment:?}", doc["command"]))
}
