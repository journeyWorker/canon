//! The search for a call (`canon-0-34-atip-constraints`, `canon-0-35-canon-check-order`), in one module.
//!
//! [`solve`] finds, for one rule of a command, the set of arguments that breaks that rule and no other (or satisfies it while
//! exercising it), by looking at every set of the arguments the rules name. What it finds is a pure function of the rules of the document, the
//! arguments they name, the words of the example and the code of this module. `calls.rs` keeps what it finds as data (0.42
//! `canon-0-42-test-cost`) and the sweeps read it, so a sweep never searches.
//!
//! The text of this file and of `samples_*.rs` is the searcher's source: the stored calls record a digest of it, and a change here makes
//! them stale until `just rule-calls` makes them again. So this file holds everything the search reads and nothing the sweeps use to judge:
//! the reading of a command line against a document ([`parse_call`]), the values a search gives an argument ([`sample`], [`candidates`]),
//! the search itself, the choice of the pairs of rules a sweep asks a call for ([`walk_pairs`]) and the filling of a found call with the
//! required arguments it did not name ([`complete`]).

use std::collections::{BTreeMap, BTreeSet};

use atip::constraint::Given;
use atip::doc::{ConditionDoc, ConstraintDoc, RuleKind};
use serde_json::{json, Value};

use super::{samples_authoring, samples_evidence, samples_reads};

// ------------------------------------------------------------------------------------------------ the documents

/// The options of a document, then its operands.
pub fn arguments(doc: &Value) -> Vec<&Value> {
    doc["options"].as_array().into_iter().flatten().chain(doc["operands"].as_array().into_iter().flatten()).collect()
}

pub fn argument<'a>(doc: &'a Value, name: &str) -> Option<&'a Value> {
    arguments(doc).into_iter().find(|argument| argument["name"] == name)
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

/// The example of a command a search starts from: the invocation that passes (without `canon` and `--json`).
pub fn doc_example(doc: &Value) -> Vec<String> {
    let line = doc["examples"][0].as_str().unwrap_or_else(|| panic!("`{}` has an example", doc["command"]));
    let mut words = atip::split_argv(line);
    assert_eq!(words.remove(0), "canon", "{line}");
    words.retain(|word| word != "--json");
    words
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

pub fn normal(argument: &Value, text: &str) -> Value {
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
        "TIMESTAMP" => "2030-01-01T00:00:00Z",
        "ACTOR_ID" => "alice",
        "N" => "2",
        "REV" => "HEAD",
        "FORMAT:PATH" => "junit:report.xml",
        "NAME@VERSION" => "reviewer@v1",
        "PATH_OR_EFFECT" => "effect:cache",
        "DIR" | "FILE" | "PATH" => "sample.txt",
        "DIGEST" | "PLAN_DIGEST" => "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Goal {
    /// The call breaks rule `at` and, when possible, nothing else.
    Violate(usize),
    /// The call breaks rules `a` and `b` and, when possible, nothing else.
    Both(usize, usize),
    /// The call breaks nothing and exercises rule `at`: gives what makes it apply and meets it.
    Satisfy(usize),
}

/// The call the solver found: the arguments, and the other rules it breaks (empty for a satisfying call).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub assignment: Assignment,
    pub also_breaks: Vec<usize>,
}

pub fn names_of(rules: &[ConstraintDoc]) -> BTreeSet<String> {
    rules.iter().flat_map(atip::constraint::members).collect()
}

/// The values a name may take in a search: each value an `in` list names, and one value no list names.
pub fn candidates(doc: &Value, example: &[String], rules: &[ConstraintDoc], name: &str) -> Vec<String> {
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
pub fn exercised(rule: &ConstraintDoc, given: &Given) -> bool {
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
            Goal::Both(a, b) => broken.contains(&a) && broken.contains(&b),
            Goal::Satisfy(target) => broken.is_empty() && exercised(&rules[target], &given),
        };
        if fits {
            let others: Vec<usize> = match goal {
                Goal::Violate(target) => broken.iter().copied().filter(|at| *at != target).collect(),
                Goal::Both(a, b) => broken.iter().copied().filter(|at| *at != a && *at != b).collect(),
                Goal::Satisfy(_) => Vec::new(),
            };
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

// ------------------------------------------------------------------------------------- what a sweep asks for

/// The most pairs of rules of one command the sweep of `atip.checkorder.07` asks a call for.
pub const MOST_PAIRS: usize = 8;

/// The pairs of rules the sweep of `atip.checkorder.07` visits in one command, in the document's order: each rule against the later ones of
/// another code, until [`MOST_PAIRS`] of them have a call. `ask` answers the call of a pair (`None`: no call breaks both); the result lists
/// every pair visited with its answer.
pub fn walk_pairs<T>(rules: &[ConstraintDoc], mut ask: impl FnMut(usize, usize) -> Option<T>) -> Vec<((usize, usize), Option<T>)> {
    let mut visited = Vec::new();
    let mut chosen = 0;
    for first in 0..rules.len() {
        for second in first + 1..rules.len() {
            if chosen == MOST_PAIRS {
                return visited;
            }
            if code_of(&rules[first]) == code_of(&rules[second]) {
                continue;
            }
            let answer = ask(first, second);
            chosen += usize::from(answer.is_some());
            visited.push(((first, second), answer));
        }
    }
    visited
}

/// The call of a search with every required argument the search did not name, as the passing `example` gives it: the rules say which
/// arguments break them, and a call must still be whole for the stateless stage to reach them.
pub fn complete(doc: &Value, example: &[String], mut found: Found) -> Found {
    let passing = parse_call(doc, example).unwrap_or_else(|problem| panic!("`{}`: {problem}", doc["command"])).text;
    for (name, values) in passing {
        if arguments(doc).into_iter().any(|argument| argument["required"] == true && argument["name"] != "json" && argument["name"] == name.as_str()) {
            found.assignment.entry(name).or_insert(values);
        }
    }
    found
}
