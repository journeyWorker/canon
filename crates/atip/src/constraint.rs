//! Constraints and validators (spec 10.8, approaches/rust.md 4.2): the relations between a command's arguments and the
//! stateless checks neither a domain nor a relation can say.
//!
//! - what clap says through its public getters (`conflicts_with`, `ArgGroup`) is read by [`clap_rules`];
//! - what clap keeps private (`requires`, `required_unless_present`) and what a handler enforces is written by the tool in a
//!   table of [`Rule`]s and [`Validator`]s, and a test of the tool drives every one through the real parser (the
//!   behavioural test replaces reading clap's `Debug` text);
//! - [`Given`] and [`violations`] say what a set of arguments breaks, so a test can build a call that breaks one rule
//!   and nothing else, and hold every example and every `rejects` to the declared rules;
//! - [`document_problems`] is 10.13 rules 3, 4 and 13 as far as a document alone decides them.

use std::collections::{BTreeMap, BTreeSet};

use clap::{Arg, Command};
use serde_json::{json, Value};

use crate::doc::{ConditionDoc, ConstraintDoc, OperandDoc, OptionDoc, RuleKind, ValidatorDoc, ValueType};
use crate::project;

/// When a `requires` or `forbids` applies: `name` is given, and, when `values` is not empty, one of its values is in `values`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct When {
    pub name: &'static str,
    pub values: &'static [&'static str],
}

/// One rule of a tool's table, written once per relation (spec 10.8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub kind: RuleKind,
    pub names: &'static [&'static str],
    pub when: Option<When>,
    pub then: &'static [&'static str],
    pub code: Option<&'static str>,
    pub help: Option<&'static str>,
    /// What the tool's own refusal says, in place of the sentence generated from the rule (not a document member).
    pub says: Option<&'static str>,
    /// The `hint` of the refusal (`{value}` is the value of the argument the `if` names); not a document member.
    pub hint: Option<&'static str>,
    /// The `reason` of the registered code the refusal carries; not a document member.
    pub reason: Option<&'static str>,
}

impl Rule {
    const fn names(kind: RuleKind, names: &'static [&'static str]) -> Rule {
        Rule { kind, names, when: None, then: &[], code: None, help: None, says: None, hint: None, reason: None }
    }

    /// At least one of `names` is given.
    pub const fn at_least_one(names: &'static [&'static str]) -> Rule {
        Rule::names(RuleKind::AtLeastOne, names)
    }

    /// Exactly one of `names` is given.
    pub const fn exactly_one(names: &'static [&'static str]) -> Rule {
        Rule::names(RuleKind::ExactlyOne, names)
    }

    /// At most one of `names` is given.
    pub const fn at_most_one(names: &'static [&'static str]) -> Rule {
        Rule::names(RuleKind::AtMostOne, names)
    }

    /// When `name` is given, every name in `then` is given.
    pub const fn requires(name: &'static str, then: &'static [&'static str]) -> Rule {
        Rule { kind: RuleKind::Requires, names: &[], when: Some(When { name, values: &[] }), then, code: None, help: None, says: None, hint: None, reason: None }
    }

    /// When `name` is given with one of `values`, every name in `then` is given.
    pub const fn requires_in(name: &'static str, values: &'static [&'static str], then: &'static [&'static str]) -> Rule {
        Rule { kind: RuleKind::Requires, names: &[], when: Some(When { name, values }), then, code: None, help: None, says: None, hint: None, reason: None }
    }

    /// When `name` is given, no name in `then` is given.
    pub const fn forbids(name: &'static str, then: &'static [&'static str]) -> Rule {
        Rule { kind: RuleKind::Forbids, names: &[], when: Some(When { name, values: &[] }), then, code: None, help: None, says: None, hint: None, reason: None }
    }

    /// When `name` is given with one of `values`, no name in `then` is given.
    pub const fn forbids_in(name: &'static str, values: &'static [&'static str], then: &'static [&'static str]) -> Rule {
        Rule { kind: RuleKind::Forbids, names: &[], when: Some(When { name, values }), then, code: None, help: None, says: None, hint: None, reason: None }
    }

    /// The registered code a violation produces (default `USAGE`).
    pub const fn code(mut self, code: &'static str) -> Rule {
        self.code = Some(code);
        self
    }

    /// One sentence saying what the rule means.
    pub const fn help(mut self, help: &'static str) -> Rule {
        self.help = Some(help);
        self
    }

    /// What the refusal says (the tool's own words for the violation).
    pub const fn says(mut self, says: &'static str) -> Rule {
        self.says = Some(says);
        self
    }

    /// The refusal's `hint`: the action that fixes it; `{value}` stands for the value of the argument the `if` names.
    pub const fn hint(mut self, hint: &'static str) -> Rule {
        self.hint = Some(hint);
        self
    }

    /// The `reason` the registered code of the refusal carries.
    pub const fn reason(mut self, reason: &'static str) -> Rule {
        self.reason = Some(reason);
        self
    }

    /// The rule as the document member.
    pub fn doc(&self) -> ConstraintDoc {
        let strings = |names: &[&str]| names.iter().map(|name| (*name).to_string()).collect::<Vec<_>>();
        let named = !self.names.is_empty();
        ConstraintDoc {
            rule: self.kind,
            names: named.then(|| strings(self.names)),
            when: self.when.map(|when| ConditionDoc { name: when.name.to_string(), values: (!when.values.is_empty()).then(|| when.values.iter().map(|value| json!(value)).collect()) }),
            then: (!self.then.is_empty()).then(|| strings(self.then)),
            code: self.code.map(str::to_string),
            help: self.help.map(str::to_string),
            extensions: BTreeMap::new(),
        }
    }
}

/// One stateless check of a tool's table that neither a domain nor the five rules can say (spec 10.8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Validator {
    pub arguments: &'static [&'static str],
    pub code: &'static str,
    pub description: &'static str,
    pub rejects: &'static [&'static str],
}

impl Validator {
    pub fn doc(&self) -> ValidatorDoc {
        ValidatorDoc {
            arguments: self.arguments.iter().map(|name| (*name).to_string()).collect(),
            code: self.code.to_string(),
            description: self.description.to_string(),
            rejects: self.rejects.iter().map(|call| (*call).to_string()).collect(),
            extensions: BTreeMap::new(),
        }
    }
}

// ------------------------------------------------------------------ from clap

/// The name a document gives `arg`: its long name, or an operand's name.
pub fn name_of(arg: &Arg) -> String {
    arg.get_long().map_or_else(|| project::operand_name(arg), str::to_string)
}

/// The relations clap states through public getters, as rules: a pair of arguments that conflict is an `atMostOne` of two,
/// a required `ArgGroup` that takes one member is an `exactlyOne` and one that takes several an `atLeastOne`, an optional
/// one that takes one member an `atMostOne` (approaches/rust.md 4.2). Members come in the command's declaration order.
/// `requires` and `required_unless_present` have no getter: the tool's table says them.
pub fn clap_rules(cmd: &Command) -> Vec<ConstraintDoc> {
    let args: Vec<&Arg> = cmd.get_arguments().filter(|arg| !project::is_builtin_arg(arg)).collect();
    let position = |id: &str| args.iter().position(|arg| arg.get_id() == id);
    let mut rules: Vec<ConstraintDoc> = Vec::new();
    let mut grouped: BTreeSet<(usize, usize)> = BTreeSet::new();
    for group in cmd.get_groups() {
        let mut members: Vec<usize> = group.get_args().filter_map(|id| position(id.as_str())).collect();
        members.sort_unstable();
        let names: Vec<String> = members.iter().map(|at| name_of(args[*at])).collect();
        let multiple = (*group).clone().is_multiple();
        let required = group.is_required_set();
        let kind = match (required, multiple) {
            (true, false) => RuleKind::ExactlyOne,
            (true, true) => RuleKind::AtLeastOne,
            (false, false) => RuleKind::AtMostOne,
            (false, true) => continue,
        };
        if !multiple {
            for (a, first) in members.iter().enumerate() {
                for second in &members[a + 1..] {
                    grouped.insert((*first, *second));
                }
            }
        }
        if names.len() >= 2 {
            rules.push(ConstraintDoc { rule: kind, names: Some(names), when: None, then: None, code: None, help: None, extensions: BTreeMap::new() });
        }
    }
    let mut pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
    for (at, arg) in args.iter().enumerate() {
        for other in cmd.get_arg_conflicts_with(arg) {
            if let Some(other_at) = position(other.get_id().as_str()) {
                let pair = (at.min(other_at), at.max(other_at));
                if pair.0 != pair.1 && !grouped.contains(&pair) {
                    pairs.insert(pair);
                }
            }
        }
    }
    // Pairs that conflict with each other all the way round are one `atMostOne` (`schema --errors --error --warnings
    // --warning` and the path); a pair that does not extend is an `atMostOne` of two (10.8.1).
    for clique in maximal_cliques(args.len(), &pairs) {
        let names: Vec<String> = clique.iter().map(|at| name_of(args[*at])).collect();
        rules.push(ConstraintDoc { rule: RuleKind::AtMostOne, names: Some(names), when: None, then: None, code: None, help: None, extensions: BTreeMap::new() });
    }
    rules
}

/// The maximal sets of vertices (`0..count`) every two of which are joined by an edge in `edges`, each in ascending order, the
/// sets in ascending order of their first members (Bron-Kerbosch over a graph of a few dozen vertices at most).
fn maximal_cliques(count: usize, edges: &BTreeSet<(usize, usize)>) -> Vec<Vec<usize>> {
    let joined = |a: usize, b: usize| edges.contains(&(a.min(b), a.max(b)));
    fn grow(joined: &dyn Fn(usize, usize) -> bool, current: &mut Vec<usize>, mut candidates: Vec<usize>, mut excluded: Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if candidates.is_empty() && excluded.is_empty() {
            if current.len() >= 2 {
                out.push(current.clone());
            }
            return;
        }
        while let Some(vertex) = candidates.first().copied() {
            let next_candidates: Vec<usize> = candidates.iter().copied().filter(|other| joined(vertex, *other)).collect();
            let next_excluded: Vec<usize> = excluded.iter().copied().filter(|other| joined(vertex, *other)).collect();
            current.push(vertex);
            grow(joined, current, next_candidates, next_excluded, out);
            current.pop();
            candidates.remove(0);
            excluded.push(vertex);
        }
    }
    let mut out = Vec::new();
    grow(&joined, &mut Vec::new(), (0..count).collect(), Vec::new(), &mut out);
    for clique in &mut out {
        clique.sort_unstable();
    }
    out.sort();
    out
}

// --------------------------------------------------------------- evaluating

/// The arguments one invocation gives explicitly (spec 10.8.1): name to its values, normalised (an integer as a JSON
/// number, a switch as `true`). A default or an environment value is not in it.
pub type Given = BTreeMap<String, Vec<Value>>;

fn is_given(given: &Given, name: &str) -> bool {
    given.contains_key(name)
}

/// Whether the `if` of a rule holds for `given`.
fn holds(given: &Given, when: &ConditionDoc) -> bool {
    match (given.get(&when.name), &when.values) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(values), Some(wanted)) => values.iter().any(|value| wanted.iter().any(|candidate| same_value(candidate, value))),
    }
}

/// Values compare after normalisation: a number is a number, any other value its text.
fn same_value(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a.as_str().zip(b.as_str()).map_or(a == b, |(x, y)| x == y),
    }
}

/// Whether `given` breaks `rule`, and, when it does, the argument a violation names (spec 10.8.1, 7.1.1): the first argument of the
/// rule at fault in the order the rule declares them: the first of `names` for a rule that demands one and has none, the first
/// given of `names` for a rule that allows one (or at most one) and has several, the first missing of `then` for a `requires`
/// and the first given of `then` for a `forbids`. The order is the document's, never the order on the command line.
pub fn violation(rule: &ConstraintDoc, given: &Given) -> Option<String> {
    let names = rule.names.as_deref().unwrap_or_default();
    let then = rule.then.as_deref().unwrap_or_default();
    let given_names: Vec<&String> = names.iter().filter(|name| is_given(given, name)).collect();
    match rule.rule {
        RuleKind::AtLeastOne => given_names.is_empty().then(|| names.first().cloned().unwrap_or_default()),
        RuleKind::ExactlyOne => match given_names.len() {
            1 => None,
            0 => Some(names.first().cloned().unwrap_or_default()),
            _ => Some(given_names[0].clone()),
        },
        RuleKind::AtMostOne => (given_names.len() > 1).then(|| given_names[0].clone()),
        RuleKind::Requires => {
            let when = rule.when.as_ref()?;
            if !holds(given, when) {
                return None;
            }
            then.iter().find(|name| !is_given(given, name)).cloned()
        }
        RuleKind::Forbids => {
            let when = rule.when.as_ref()?;
            if !holds(given, when) {
                return None;
            }
            then.iter().find(|name| is_given(given, name)).cloned()
        }
    }
}

/// The indices of the rules `given` breaks, with the argument each names.
pub fn violations(rules: &[ConstraintDoc], given: &Given) -> Vec<(usize, String)> {
    rules.iter().enumerate().filter_map(|(at, rule)| violation(rule, given).map(|field| (at, field))).collect()
}

/// The names a rule is about, in the order the document lists them.
pub fn members(rule: &ConstraintDoc) -> Vec<String> {
    let mut all: Vec<String> = rule.names.clone().unwrap_or_default();
    if let Some(when) = &rule.when {
        all.push(when.name.clone());
    }
    all.extend(rule.then.clone().unwrap_or_default());
    all
}

/// The canonical form of a rule, so that two rules that state one relation are equal (spec 10.8.2): the rule, its names
/// sorted, its `if` with its values sorted, its `then` sorted; the code and the help are not part of the relation.
fn relation_key(rule: &ConstraintDoc) -> String {
    let sorted = |names: &Option<Vec<String>>| {
        let mut names = names.clone().unwrap_or_default();
        names.sort();
        names
    };
    let when = rule.when.as_ref().map(|when| {
        let mut values: Vec<String> = when.values.iter().flatten().map(Value::to_string).collect();
        values.sort();
        (when.name.clone(), values)
    });
    json!([rule.rule.as_str(), sorted(&rule.names), when, sorted(&rule.then)]).to_string()
}

// ----------------------------------------------------------------- document

/// What a command document's declarations break of the rules a document alone decides (spec 10.13 rules 3, 4 and 13):
/// every name names an operand or option; no relation is stated twice; `in` is absent on a switch and a count and holds valid
/// values of the argument, inside its domain; every `code` is in `errors`; a validator reads named arguments, has a code
/// in `errors`, a description and at least one `rejects`, none of which is one of the `examples`.
pub fn document_problems(options: &[OptionDoc], operands: &[OperandDoc], constraints: &[ConstraintDoc], validators: &[ValidatorDoc], errors: &[String], examples: &[String]) -> Vec<String> {
    struct Known<'a> {
        value_type: ValueType,
        members: Option<&'a [String]>,
        domain: Option<&'a Value>,
    }
    let mut known: BTreeMap<&str, Known<'_>> = BTreeMap::new();
    for option in options {
        known.insert(&option.name, Known { value_type: option.value_type, members: option.possible_values.as_deref(), domain: option.domain.as_ref() });
    }
    for operand in operands {
        known.insert(&operand.name, Known { value_type: operand.value_type, members: operand.possible_values.as_deref(), domain: operand.domain.as_ref() });
    }
    let mut problems = Vec::new();
    let domained = options.iter().map(|option| (&option.name, option.value_type, &option.domain, &option.default)).chain(operands.iter().map(|operand| (&operand.name, operand.value_type, &operand.domain, &operand.default)));
    for (name, value_type, domain, default) in domained {
        let Some(domain) = domain else { continue };
        for problem in crate::domain::profile_problems(value_type, domain) {
            problems.push(format!("`{name}`: {problem}"));
        }
        let defaults: Vec<&Value> = match default {
            Value::Null => Vec::new(),
            Value::Array(items) => items.iter().collect(),
            single => vec![single],
        };
        for value in defaults {
            if !crate::domain::admits(domain, value).unwrap_or(false) {
                problems.push(format!("`{name}`: the default {value} is outside its domain {domain}"));
            }
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for rule in constraints {
        let label = format!("the {} rule on {:?}", rule.rule.as_str(), members(rule));
        for name in members(rule) {
            if !known.contains_key(name.as_str()) {
                problems.push(format!("{label}: `{name}` is neither an operand nor an option"));
            }
        }
        match rule.rule {
            RuleKind::AtLeastOne | RuleKind::ExactlyOne | RuleKind::AtMostOne => {
                if rule.names.as_ref().is_none_or(|names| names.len() < 2) || rule.when.is_some() || rule.then.is_some() {
                    problems.push(format!("{label}: a {} has `names` (two or more) and no `if` or `then`", rule.rule.as_str()));
                }
            }
            RuleKind::Requires | RuleKind::Forbids => {
                if rule.when.is_none() || rule.then.as_ref().is_none_or(Vec::is_empty) || rule.names.is_some() {
                    problems.push(format!("{label}: a {} has `if` and `then` and no `names`", rule.rule.as_str()));
                }
            }
        }
        if let Some(when) = &rule.when {
            if let Some(argument) = known.get(when.name.as_str()) {
                if let Some(values) = &when.values {
                    if matches!(argument.value_type, ValueType::Switch | ValueType::Count) {
                        problems.push(format!("{label}: `in` is not allowed on the {} `{}`", argument.value_type.as_str(), when.name));
                    }
                    for value in values {
                        let valid = match argument.value_type {
                            ValueType::Enum => value.as_str().is_some_and(|text| argument.members.is_some_and(|members| members.iter().any(|member| member == text))),
                            ValueType::Integer | ValueType::Count => value.is_i64() || value.is_u64(),
                            ValueType::Number => value.is_number(),
                            ValueType::Switch => false,
                            ValueType::String | ValueType::Path | ValueType::Json => value.is_string(),
                        };
                        if !valid {
                            problems.push(format!("{label}: {value} is not a valid value of `{}`", when.name));
                        } else if let Some(domain) = argument.domain {
                            if !crate::domain::admits(domain, value).unwrap_or(false) {
                                problems.push(format!("{label}: {value} is outside the domain of `{}`", when.name));
                            }
                        }
                    }
                }
            }
        }
        if let Some(code) = &rule.code {
            if !errors.iter().any(|known| known == code) {
                problems.push(format!("{label}: the code {code} is not in errors"));
            }
        }
        if !seen.insert(relation_key(rule)) {
            problems.push(format!("{label}: the same relation is stated twice (spec 10.8.2)"));
        }
    }
    for validator in validators {
        let label = format!("the validator of {:?}", validator.arguments);
        if validator.arguments.is_empty() {
            problems.push(format!("{label}: names no argument"));
        }
        for name in &validator.arguments {
            if !known.contains_key(name.as_str()) {
                problems.push(format!("{label}: `{name}` is neither an operand nor an option"));
            }
        }
        if !errors.contains(&validator.code) {
            problems.push(format!("{label}: the code {} is not in errors", validator.code));
        }
        if validator.description.trim().is_empty() {
            problems.push(format!("{label}: has no description"));
        }
        if validator.rejects.is_empty() {
            problems.push(format!("{label}: has no rejects"));
        }
        for call in &validator.rejects {
            if examples.iter().any(|example| example == call) {
                problems.push(format!("{label}: `{call}` is also an example"));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use clap::{ArgAction, ArgGroup};

    use super::*;

    fn command() -> Command {
        let mut cmd = Command::new("tool").subcommand(
            Command::new("run")
                .about("Runs")
                .arg(Arg::new("check").long("check").action(ArgAction::SetTrue).help("c").conflicts_with_all(["policy", "quiet"]))
                .arg(Arg::new("policy").long("policy").help("p").conflicts_with("no_policy"))
                .arg(Arg::new("no_policy").long("no-policy").action(ArgAction::SetTrue).help("n"))
                .arg(Arg::new("quiet").long("quiet").action(ArgAction::SetTrue).help("q"))
                .arg(Arg::new("upstream").long("upstream").help("u"))
                .arg(Arg::new("original").long("original").help("o"))
                .arg(Arg::new("a").long("a").help("a"))
                .arg(Arg::new("b").long("b").help("b"))
                .group(ArgGroup::new("provenance").args(["upstream", "original"]).required(true).multiple(false))
                .group(ArgGroup::new("some").args(["a", "b"]).required(true).multiple(true)),
        );
        cmd.build();
        cmd
    }

    #[test]
    fn clap_conflicts_and_groups_are_rules_stated_once() {
        let cmd = command();
        let rules = clap_rules(cmd.find_subcommand("run").expect("run"));
        let shown: Vec<(RuleKind, Vec<String>)> = rules.iter().map(|rule| (rule.rule, rule.names.clone().unwrap_or_default())).collect();
        let names = |words: &[&str]| words.iter().map(|word| (*word).to_string()).collect::<Vec<_>>();
        assert!(shown.contains(&(RuleKind::ExactlyOne, names(&["upstream", "original"]))), "{shown:?}");
        assert!(shown.contains(&(RuleKind::AtLeastOne, names(&["a", "b"]))), "{shown:?}");
        assert!(shown.contains(&(RuleKind::AtMostOne, names(&["check", "policy"]))), "{shown:?}");
        assert!(shown.contains(&(RuleKind::AtMostOne, names(&["check", "quiet"]))), "{shown:?}");
        assert!(shown.contains(&(RuleKind::AtMostOne, names(&["policy", "no-policy"]))), "{shown:?}");
        assert!(!shown.contains(&(RuleKind::AtMostOne, names(&["upstream", "original"]))), "the group's pair is stated by its exactlyOne: {shown:?}");
        assert_eq!(shown.len(), 5, "{shown:?}");
    }

    #[test]
    fn a_rule_is_violated_by_the_arguments_it_names_and_says_which() {
        let given = |names: &[(&str, Value)]| -> Given { names.iter().map(|(name, value)| ((*name).to_string(), vec![value.clone()])).collect() };
        let requires = Rule::requires("commit", &["plan-digest"]).doc();
        assert_eq!(violation(&requires, &given(&[("commit", json!(true))])), Some("plan-digest".to_string()));
        assert_eq!(violation(&requires, &given(&[("commit", json!(true)), ("plan-digest", json!("x"))])), None);
        assert_eq!(violation(&requires, &given(&[])), None);
        let conditional = Rule::requires_in("disposition", &["rejected", "deferred"], &["reason"]).doc();
        assert_eq!(violation(&conditional, &given(&[("disposition", json!("rejected"))])), Some("reason".to_string()));
        assert_eq!(violation(&conditional, &given(&[("disposition", json!("fixed"))])), None);
        let one = Rule::exactly_one(&["regime", "domain"]).doc();
        assert_eq!(violation(&one, &given(&[])), Some("regime".to_string()));
        assert_eq!(violation(&one, &given(&[("regime", json!("k")), ("domain", json!("d"))])), Some("regime".to_string()));
        assert_eq!(violation(&one, &given(&[("domain", json!("d"))])), None);
        let forbids = Rule::forbids("all-scenarios", &["task", "scenario"]).doc();
        assert_eq!(violation(&forbids, &given(&[("all-scenarios", json!(true)), ("scenario", json!("x"))])), Some("scenario".to_string()));
    }

    #[test]
    fn a_relation_stated_twice_and_a_name_that_is_not_an_argument_are_found() {
        let option = |name: &str| OptionDoc {
            name: name.to_string(),
            short: None,
            value_name: None,
            value_type: ValueType::Switch,
            possible_values: None,
            required: false,
            repeatable: false,
            default: json!(false),
            default_note: None,
            env: None,
            flag_only: false,
            domain: None,
            help: "h".to_string(),
        };
        let options = [option("a"), option("b")];
        let twice = [Rule::at_most_one(&["a", "b"]).doc(), Rule::at_most_one(&["b", "a"]).code("OTHER").doc()];
        let errors = vec!["USAGE".to_string(), "OTHER".to_string()];
        let problems = document_problems(&options, &[], &twice, &[], &errors, &[]);
        assert!(problems.iter().any(|p| p.contains("stated twice")), "{problems:?}");
        let unknown = [Rule::requires("a", &["zzz"]).doc()];
        assert!(document_problems(&options, &[], &unknown, &[], &errors, &[]).iter().any(|p| p.contains("zzz")));
        let uncoded = [Rule::requires("a", &["b"]).code("MISSING").doc()];
        assert!(document_problems(&options, &[], &uncoded, &[], &errors, &[]).iter().any(|p| p.contains("MISSING")));
        assert!(document_problems(&options, &[], &[Rule::requires("a", &["b"]).doc()], &[], &errors, &[]).is_empty());
    }
}
