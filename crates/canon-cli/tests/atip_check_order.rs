//! The order of the stateless checks (canon 0.35 `canon-0-35-canon-check-order`, atip/1 1.2 spec 11.7 step 6, 10.8.1, 10.8.3,
//! `L1.check-order` of approaches/checker.md): an invocation that breaks two stages at once is answered by the earlier one, with its
//! code and `field`: a failure to parse (the earliest on the line), then `required`, then the domains, the constraints and the
//! validators. Tests are named by scenario id (`atip.checkorder.NN`).
//!
//! The sweeps build their calls the way the checker does, from the command documents alone: the document's example is the passing
//! invocation, and each pair changes it to break two stages. The answer of the pair is held to the answer of the earlier stage broken
//! alone (or, for two constraints, to the first rule the document lists that the call breaks). Every call runs in an empty directory with
//! an empty environment: the stateless stage answers before any state is read.

mod harness;
mod rule_support;

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};

use atip::constraint::violations;
use rule_support::{constraints, declared_role, given_from, parse_call, run_empty_with, solve, Assignment, Found, Goal, Outcome};
use serde_json::Value;

/// The call that breaks rule `at` of a command: the command's name and the rule's index, and what the search found.
type Searched = BTreeMap<(String, usize), Option<Found>>;

/// The call that breaks rule `at` of `doc` and, when it can, nothing else: searched once per rule (the search looks at every set of the
/// arguments the rules name), and shared by the sweeps of this binary.
fn violate(doc: &Value, rules: &[atip::doc::ConstraintDoc], at: usize) -> Option<Found> {
    static FOUND: LazyLock<Mutex<Searched>> = LazyLock::new(|| Mutex::new(BTreeMap::new()));
    let key = (doc["command"].as_str().unwrap_or_default().to_string(), at);
    if let Some(found) = FOUND.lock().expect("the cache").get(&key) {
        return found.clone();
    }
    let found = solve(doc, &example_of(doc), rules, Goal::Violate(at)).map(|found| completed(doc, found));
    FOUND.lock().expect("the cache").insert(key, found.clone());
    found
}

/// The call of a search with every required argument the search did not name, as the passing example gives it: the rules say which
/// arguments break them, and a call must still be whole for the stateless stage to reach them.
fn completed(doc: &Value, mut found: Found) -> Found {
    for (name, values) in passing(doc) {
        if required(doc).contains(&name) {
            found.assignment.entry(name).or_insert(values);
        }
    }
    found
}

// -------------------------------------------------------------------------------------------------------------- the calls

/// The example of a command: the invocation that passes (without `canon` and `--json`).
fn example_of(doc: &Value) -> Vec<String> {
    let line = doc["examples"][0].as_str().unwrap_or_else(|| panic!("`{}` has an example", doc["command"]));
    let mut words = atip::split_argv(line);
    assert_eq!(words.remove(0), "canon", "{line}");
    words.retain(|word| word != "--json");
    words
}

/// The arguments the example gives, by name.
fn passing(doc: &Value) -> Assignment {
    parse_call(doc, &example_of(doc)).unwrap_or_else(|problem| panic!("`{}`: {problem}", doc["command"])).text
}

/// The options of a document in the order it declares them, and its operands before them.
fn arguments(doc: &Value) -> Vec<&Value> {
    doc["operands"].as_array().into_iter().flatten().chain(doc["options"].as_array().into_iter().flatten()).collect()
}

fn is_operand(doc: &Value, name: &str) -> bool {
    rule_support::is_operand(doc, name)
}

/// The `field` an answer names for the argument `name` (spec 7.1.1): `--name` for an option, the value name for an operand.
fn field_for(doc: &Value, name: &str) -> String {
    if is_operand(doc, name) {
        rule_support::argument(doc, name).and_then(|operand| operand["valueName"].as_str()).unwrap_or(name).to_string()
    } else {
        format!("--{name}")
    }
}

/// The words of a call: the command path, then the arguments as the assignment gives them. Options are written as `--name=value`,
/// in the document's order (or its reverse), the operands before them (or after, when `operands_last`); an operand that starts with a
/// dash comes after `--`. Nothing the assignment does not name is added.
fn render(doc: &Value, assignment: &Assignment, reversed: bool) -> Vec<String> {
    let mut words: Vec<String> = doc["path"].as_array().into_iter().flatten().filter_map(|word| word.as_str().map(str::to_string)).collect();
    let mut operand_words: Vec<String> = Vec::new();
    for operand in doc["operands"].as_array().into_iter().flatten() {
        if let Some(values) = assignment.get(operand["name"].as_str().unwrap_or_default()) {
            operand_words.extend(values.iter().cloned());
        }
    }
    let mut option_words: Vec<Vec<String>> = Vec::new();
    for option in doc["options"].as_array().into_iter().flatten() {
        let name = option["name"].as_str().unwrap_or_default();
        let Some(values) = assignment.get(name) else { continue };
        for value in values {
            option_words.push(vec![if matches!(option["valueType"].as_str(), Some("switch" | "count")) { format!("--{name}") } else { format!("--{name}={value}") }]);
        }
    }
    if reversed {
        option_words.reverse();
    }
    let dashed = operand_words.iter().any(|word| word.starts_with('-'));
    if reversed && !dashed {
        words.extend(option_words.into_iter().flatten());
        words.extend(operand_words);
    } else if dashed {
        words.extend(option_words.into_iter().flatten());
        words.push("--".to_string());
        words.extend(operand_words);
    } else {
        words.extend(operand_words);
        words.extend(option_words.into_iter().flatten());
    }
    words
}

/// Runs a call in an empty directory with an empty environment and the role the call declares.
fn run(doc: &Value, assignment: &Assignment, words: &[String]) -> Outcome {
    let path = doc["command"].as_str().unwrap_or_default();
    match declared_role(path, assignment) {
        Some(role) => run_empty_with(words, &[("CANON_ROLE", role)]),
        None => run_empty_with(words, &[]),
    }
}

/// What a refusal answers: its code and its `field`.
fn answer(outcome: &Outcome) -> (Option<String>, Option<String>) {
    (outcome.code.clone(), outcome.field.clone())
}

fn refused(outcome: &Outcome) -> bool {
    !outcome.ok && outcome.exit == 2
}

/// A value outside the domain of the argument, when the document or the tool's table gives one: a row's `outside` value, else one just
/// beyond an integer bound.
fn broken_value(doc: &Value, argument: &Value) -> Option<String> {
    let path = doc["command"].as_str().unwrap_or_default();
    let name = argument["name"].as_str().unwrap_or_default();
    let operand = is_operand(doc, name);
    if let Some(row) = canon_cli::arg_rules::domain_for(path, name, argument["valueName"].as_str()) {
        if let Some(value) = row.outside.iter().find(|value| !(operand && value.starts_with('-'))) {
            return Some((*value).to_string());
        }
    }
    if argument["valueType"] == "integer" {
        if let Some(minimum) = argument["domain"]["minimum"].as_i64() {
            return Some((minimum - 1).to_string());
        }
        if let Some(maximum) = argument["domain"]["maximum"].as_i64() {
            return Some((maximum + 1).to_string());
        }
    }
    None
}

/// The arguments that have a value outside their domain, operands first and then options, as the document declares them, each with the
/// value.
fn breakable(doc: &Value) -> Vec<(String, String)> {
    arguments(doc)
        .into_iter()
        .filter(|argument| !matches!(argument["valueType"].as_str(), Some("switch" | "count" | "enum")) && argument["name"] != "as" && argument["name"] != "json")
        .filter_map(|argument| broken_value(doc, argument).map(|value| (argument["name"].as_str().unwrap_or_default().to_string(), value)))
        .collect()
}

/// The `required` arguments of a document, operands in their order and then options in their declaration order.
fn required(doc: &Value) -> Vec<String> {
    arguments(doc).into_iter().filter(|argument| argument["required"] == true && argument["name"] != "json").filter_map(|argument| argument["name"].as_str().map(str::to_string)).collect()
}

fn in_parallel<T: Sync>(jobs: &[T], work: impl Fn(&T) -> Vec<String> + Sync) -> Vec<String> {
    let next = Mutex::new(0usize);
    let problems = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..6 {
            scope.spawn(|| loop {
                let at = {
                    let mut next = next.lock().expect("the counter");
                    let at = *next;
                    *next += 1;
                    at
                };
                let Some(job) = jobs.get(at) else { return };
                let found = work(job);
                problems.lock().expect("the problems").extend(found);
            });
        }
    });
    let mut problems = problems.into_inner().expect("the problems");
    problems.sort();
    problems
}

/// Every command document that a call can be built for: the command has an example the sweeps can read.
fn commands() -> Vec<&'static Value> {
    rule_support::docs().into_iter().filter(|doc| doc["examples"][0].is_string() && parse_call(doc, &example_of(doc)).is_ok()).collect()
}

fn empty(line: &str, envs: &[(&str, &str)]) -> Outcome {
    run_empty_with(&harness::words(line.trim_start_matches("canon ")), envs)
}

fn assert_answer(outcome: &Outcome, code: &str, field: &str) {
    assert!(refused(outcome), "{outcome:?}");
    assert_eq!((outcome.code.as_deref(), outcome.field.as_deref()), (Some(code), Some(field)), "{outcome:?}");
    assert!(outcome.changed.is_empty(), "a refusal wrote {:?}", outcome.changed);
}

// ----------------------------------------------------------------------------------------------------------------- F1

/// atip.checkorder.01: two failures to parse are answered by the one earliest on the command line, in every command: two unknown
/// options, and an unknown option before or after a value that is not one of the `possibleValues` of `--as`.
#[test]
fn atip_checkorder_01_the_earliest_failure_to_parse_answers() {
    let docs = commands();
    let problems = in_parallel(&docs, |doc| {
        let mut problems = Vec::new();
        let path = doc["command"].as_str().unwrap_or_default();
        let base = passing(doc);
        let words = render(doc, &base, false);
        for (extra, field) in [
            (vec!["--zz-first", "--zz-second"], "--zz-first"),
            (vec!["--zz-second", "--zz-first"], "--zz-second"),
            (vec!["--zz-first", "--as=zzz"], "--zz-first"),
            (vec!["--as=zzz", "--zz-first"], "--as"),
        ] {
            let mut call = words.clone();
            call.extend(extra.iter().map(ToString::to_string));
            let outcome = run(doc, &base, &call);
            if !refused(&outcome) || answer(&outcome) != (Some("USAGE".to_string()), Some(field.to_string())) {
                problems.push(format!("`{path}`: `{}` answered {:?}, not USAGE {field}", call.join(" "), answer(&outcome)));
            }
        }
        problems
    });
    assert!(docs.len() > 60, "{} commands", docs.len());
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

/// atip.checkorder.02 (the failure path): a value of the wrong type that comes before an unknown option answers, not the unknown
/// option; the call of the brief, `canon finding close --bogus --seq x ...`, is answered by the unknown option that comes first.
#[test]
fn atip_checkorder_02_a_value_of_the_wrong_type_before_an_unknown_option_answers() {
    let typed = empty("canon finding close add-login --seq x --bogus --round 1 --disposition fixed", &[("CANON_ROLE", "implementer")]);
    assert_answer(&typed, "USAGE", "--seq");
    let unknown = empty("canon finding close add-login --bogus --seq x --round 1 --disposition fixed", &[("CANON_ROLE", "implementer")]);
    assert_answer(&unknown, "USAGE", "--bogus");
    let two = empty("canon finding close add-login --bogus --other --seq 1", &[("CANON_ROLE", "implementer")]);
    assert_answer(&two, "USAGE", "--bogus");
    // A command word the tree lacks is a failure to parse too, ahead of a broken domain further on.
    let word = empty("canon finding nosuch --round 0", &[]);
    assert_eq!((word.code.as_deref(), word.field.as_deref()), (Some("UNKNOWN_COMMAND"), Some("command")), "{word:?}");
}

// ----------------------------------------------------------------------------------------------------------------- F2

/// atip.checkorder.03: a failure to parse answers before a missing required argument, and a missing required argument answers
/// before a broken domain or constraint; among several missing, the operands come first and then the options in declaration order.
#[test]
fn atip_checkorder_03_a_missing_required_argument_is_answered_after_a_failure_to_parse_and_before_the_rest() {
    let docs: Vec<&Value> = commands().into_iter().filter(|doc| !required(doc).is_empty()).collect();
    let pairs = Mutex::new(0usize);
    let problems = in_parallel(&docs, |doc| {
        let mut problems = Vec::new();
        let path = doc["command"].as_str().unwrap_or_default();
        let base = passing(doc);
        let needed = required(doc);
        let rules = constraints(doc);
        // The first and the last required argument, each alone and both: the field is the first missing in the document's order.
        let mut cases: Vec<Vec<String>> = vec![vec![needed[0].clone()], vec![needed[needed.len() - 1].clone()]];
        if needed.len() > 1 {
            cases.push(vec![needed[needed.len() - 1].clone(), needed[0].clone()]);
        }
        for missing in cases {
            let mut assignment = base.clone();
            for name in &missing {
                assignment.remove(name);
            }
            let expected = missing.iter().filter_map(|name| needed.iter().position(|candidate| candidate == name)).min().map(|at| field_for(doc, &needed[at])).unwrap_or_default();
            let words = render(doc, &assignment, false);
            let alone = run(doc, &assignment, &words);
            *pairs.lock().expect("count") += 1;
            if !(refused(&alone) && answer(&alone) == (Some("USAGE".to_string()), Some(expected.clone()))) {
                problems.push(format!("`{path}`: `{}` answered {:?}, not USAGE {expected}", words.join(" "), answer(&alone)));
                continue;
            }
            // A failure to parse beside it answers first.
            let mut with_unknown = words.clone();
            with_unknown.push("--zz-first".to_string());
            let outcome = run(doc, &assignment, &with_unknown);
            if answer(&outcome) != (Some("USAGE".to_string()), Some("--zz-first".to_string())) {
                problems.push(format!("`{path}`: `{}` answered {:?}, not the unknown option", with_unknown.join(" "), answer(&outcome)));
            }
            // A broken domain of another argument, written first on the line and last, answers after it.
            if let Some((name, value)) = breakable(doc).into_iter().find(|(name, _)| !missing.contains(name)) {
                let mut broken = assignment.clone();
                broken.insert(name.clone(), vec![value]);
                for reversed in [false, true] {
                    let call = render(doc, &broken, reversed);
                    let outcome = run(doc, &broken, &call);
                    *pairs.lock().expect("count") += 1;
                    if answer(&outcome) != answer(&alone) {
                        problems.push(format!("`{path}`: `{}` answered {:?}, not the missing required {:?}", call.join(" "), answer(&outcome), answer(&alone)));
                    }
                }
            }
            // A broken constraint answers after it too.
            for at in 0..rules.len() {
                let Some(found) = violate(doc, &rules, at) else { continue };
                let mut broken = assignment.clone();
                broken.extend(found.assignment.iter().filter(|(name, _)| !missing.contains(name)).map(|(name, values)| (name.clone(), values.clone())));
                let given = given_from(doc, &broken);
                if violations(&rules, &given).is_empty() {
                    continue;
                }
                let call = render(doc, &broken, false);
                let outcome = run(doc, &broken, &call);
                *pairs.lock().expect("count") += 1;
                if answer(&outcome) != answer(&alone) {
                    problems.push(format!("`{path}`: `{}` breaks {} and misses {missing:?}; answered {:?}, not the missing required {:?}", call.join(" "), rule_support::label(&rules[at]), answer(&outcome), answer(&alone)));
                }
            }
        }
        problems
    });
    assert!(docs.len() > 20, "{} commands with a required argument", docs.len());
    assert!(*pairs.lock().expect("count") > 100, "the sweep is not empty");
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

/// atip.checkorder.04 (the failure path): only a required argument missing is `USAGE` naming it; only a domain broken is the domain's code.
#[test]
fn atip_checkorder_04_a_missing_required_argument_alone_and_a_broken_domain_alone_keep_their_own_codes() {
    let missing = empty("canon finding add --round 1 --seq 1 --severity note --actor a --summary s", &[("CANON_ROLE", "implementer")]);
    assert_answer(&missing, "USAGE", "--change");
    let domain = empty("canon finding add --change add-login --round 0 --seq 1 --severity note --actor a --summary s", &[("CANON_ROLE", "implementer")]);
    assert_answer(&domain, "FINDING_ARGUMENT_INVALID", "--round");
    let both = empty("canon finding add --round 0 --seq 1 --severity note --actor a --summary s", &[("CANON_ROLE", "implementer")]);
    assert_answer(&both, "USAGE", "--change");
    // An operand missing is named by its value name, and comes before an option missing.
    let operand = empty("canon finding close --round 1", &[("CANON_ROLE", "implementer")]);
    assert_eq!((operand.code.as_deref(), operand.field.as_deref()), (Some("USAGE"), Some("CHANGE_ID")), "{operand:?}");
}

// ----------------------------------------------------------------------------------------------------------------- F3

/// atip.checkorder.05: a broken domain is answered before a broken constraint, and of two broken domains the first in the document's
/// order answers, whatever order the command line gives them in.
#[test]
fn atip_checkorder_05_a_broken_domain_is_answered_before_a_constraint_and_before_a_later_domain() {
    let docs = commands();
    let pairs = Mutex::new((0usize, 0usize));
    let problems = in_parallel(&docs, |doc| {
        let mut problems = Vec::new();
        let path = doc["command"].as_str().unwrap_or_default();
        let base = passing(doc);
        let rules = constraints(doc);
        let broken = breakable(doc);
        // Two domains: each alone answers for itself; the pair answers as the first in the document's order, written either way round.
        for (first, second) in broken.iter().zip(broken.iter().skip(1)) {
            let (mut one, mut two, mut both) = (base.clone(), base.clone(), base.clone());
            one.insert(first.0.clone(), vec![first.1.clone()]);
            two.insert(second.0.clone(), vec![second.1.clone()]);
            both.insert(first.0.clone(), vec![first.1.clone()]);
            both.insert(second.0.clone(), vec![second.1.clone()]);
            let (alone_first, alone_second) = (run(doc, &one, &render(doc, &one, false)), run(doc, &two, &render(doc, &two, false)));
            if !refused(&alone_first) || !refused(&alone_second) || answer(&alone_first) == answer(&alone_second) {
                continue;
            }
            for reversed in [false, true] {
                let call = render(doc, &both, reversed);
                let outcome = run(doc, &both, &call);
                pairs.lock().expect("count").0 += 1;
                if answer(&outcome) != answer(&alone_first) {
                    problems.push(format!("`{path}`: `{}` answered {:?}, not the first domain's {:?}", call.join(" "), answer(&outcome), answer(&alone_first)));
                }
            }
        }
        // A domain and a constraint.
        for at in 0..rules.len() {
            let Some((name, value)) = broken.iter().find(|(name, _)| !atip::constraint::members(&rules[at]).contains(name)) else { continue };
            let Some(found) = violate(doc, &rules, at) else { continue };
            let mut alone = base.clone();
            alone.insert(name.clone(), vec![value.clone()]);
            let domain_alone = run(doc, &alone, &render(doc, &alone, false));
            let mut combined = found.assignment.clone();
            combined.insert(name.clone(), vec![value.clone()]);
            let given = given_from(doc, &combined);
            if !refused(&domain_alone) || violations(&rules, &given).is_empty() {
                continue;
            }
            let constraint_alone = run(doc, &found.assignment, &render(doc, &found.assignment, false));
            if answer(&constraint_alone) == answer(&domain_alone) {
                continue;
            }
            for reversed in [false, true] {
                let call = render(doc, &combined, reversed);
                let outcome = run(doc, &combined, &call);
                pairs.lock().expect("count").1 += 1;
                if answer(&outcome) != answer(&domain_alone) {
                    problems.push(format!("`{path}`: `{}` breaks {} and the domain of --{name}; answered {:?}, not the domain's {:?}", call.join(" "), rule_support::label(&rules[at]), answer(&outcome), answer(&domain_alone)));
                }
            }
        }
        problems
    });
    let (domains, with_constraints) = *pairs.lock().expect("count");
    assert!(domains > 10 && with_constraints > 20, "the sweep is not empty: {domains} pairs of domains, {with_constraints} of a domain and a constraint");
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

/// atip.checkorder.06 (the call of the brief and its failure paths): `--round 0 --promote --no-promote` is the domain's, a constraint
/// alone is the constraint's.
#[test]
fn atip_checkorder_06_the_round_domain_answers_before_the_promote_conflict() {
    let rest = "--change add-login --seq 1 --severity note --actor a --summary s";
    let both = empty(&format!("canon finding add {rest} --round 0 --promote --no-promote"), &[("CANON_ROLE", "implementer")]);
    assert_answer(&both, "FINDING_ARGUMENT_INVALID", "--round");
    let flipped = empty(&format!("canon finding add {rest} --no-promote --promote --round 0"), &[("CANON_ROLE", "implementer")]);
    assert_answer(&flipped, "FINDING_ARGUMENT_INVALID", "--round");
    let constraint = empty(&format!("canon finding add {rest} --round 1 --promote --no-promote"), &[("CANON_ROLE", "implementer")]);
    assert_answer(&constraint, "USAGE", "--promote");
    // Two domains broken: the first in the document's order (`--round` before `--seq`), though `--seq` is written first.
    let two = empty("canon finding add --change add-login --seq 0 --round 0 --severity note --actor a --summary s", &[("CANON_ROLE", "implementer")]);
    assert_answer(&two, "FINDING_ARGUMENT_INVALID", "--round");
}

// ----------------------------------------------------------------------------------------------------------------- F4

/// atip.checkorder.07: of two broken constraints with different codes the first the document lists answers, with its field; and a
/// constraint is answered before a validator.
#[test]
fn atip_checkorder_07_the_first_constraint_the_document_lists_answers() {
    let docs = commands();
    let pairs = Mutex::new(0usize);
    let problems = in_parallel(&docs, |doc| {
        let mut problems = Vec::new();
        let path = doc["command"].as_str().unwrap_or_default();
        let rules = constraints(doc);
        // Every rule against the later ones of another code, at most eight pairs for a command (a search for a call that breaks two rules
        // looks at every set of the arguments the rules name).
        let mut chosen = 0;
        for first in 0..rules.len() {
            for second in first + 1..rules.len() {
                if chosen == 8 {
                    break;
                }
                if rule_support::code_of(&rules[first]) == rule_support::code_of(&rules[second]) {
                    continue;
                }
                let Some(found) = solve(doc, &example_of(doc), &rules, Goal::Both(first, second)).map(|found| completed(doc, found)) else { continue };
                chosen += 1;
                let given = given_from(doc, &found.assignment);
                let Some((at, fault)) = violations(&rules, &given).into_iter().next() else { continue };
                let expected = (Some(rule_support::code_of(&rules[at])), Some(field_for(doc, &fault)));
                for reversed in [false, true] {
                    let call = render(doc, &found.assignment, reversed);
                    let outcome = run(doc, &found.assignment, &call);
                    *pairs.lock().expect("count") += 1;
                    if answer(&outcome) != expected {
                        problems.push(format!("`{path}`: `{}` breaks {} and {}; answered {:?}, not {expected:?}", call.join(" "), rule_support::label(&rules[first]), rule_support::label(&rules[second]), answer(&outcome)));
                    }
                }
            }
        }
        problems
    });
    assert!(*pairs.lock().expect("count") > 40, "the sweep is not empty");
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

/// atip.checkorder.08: a validator's `rejects` call, changed to break a constraint too, is answered by the constraint; unchanged it is
/// the validator's.
#[test]
fn atip_checkorder_08_a_constraint_is_answered_before_a_validator() {
    let docs = commands();
    let pairs = Mutex::new(0usize);
    let problems = in_parallel(&docs, |doc| {
        let mut problems = Vec::new();
        let path = doc["command"].as_str().unwrap_or_default();
        let rules = constraints(doc);
        for validator in doc["validators"].as_array().into_iter().flatten() {
            let Some(call) = validator["rejects"][0].as_str() else { continue };
            let mut words = atip::split_argv(call);
            words.remove(0);
            words.retain(|word| word != "--json");
            let Ok(parsed) = parse_call(doc, &words) else { continue };
            let code = validator["code"].as_str().unwrap_or_default().to_string();
            // The call a validator rejects is the one the tool lists: no role in the environment (a relayed approval is refused for the want of one).
            let alone = run_empty_with(&words, &[]);
            if !refused(&alone) || alone.code.as_deref() != Some(code.as_str()) {
                problems.push(format!("`{path}`: `{call}` answered {:?}, not the validator's {code}", answer(&alone)));
                continue;
            }
            let own: Vec<&str> = validator["arguments"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            for at in 0..rules.len() {
                if rule_support::code_of(&rules[at]) == code {
                    continue;
                }
                let Some(found) = violate(doc, &rules, at) else { continue };
                let mut combined = parsed.text.clone();
                for (name, values) in &found.assignment {
                    if !own.contains(&name.as_str()) {
                        combined.insert(name.clone(), values.clone());
                    }
                }
                let given = given_from(doc, &combined);
                let Some((first, fault)) = violations(&rules, &given).into_iter().next() else { continue };
                if rule_support::code_of(&rules[first]) == code {
                    continue;
                }
                let expected = (Some(rule_support::code_of(&rules[first])), Some(field_for(doc, &fault)));
                let call = render(doc, &combined, false);
                let outcome = run_empty_with(&call, &[]);
                *pairs.lock().expect("count") += 1;
                if answer(&outcome) != expected {
                    problems.push(format!("`{path}`: `{}` breaks {} and the validator {code}; answered {:?}, not {expected:?}", call.join(" "), rule_support::label(&rules[at]), answer(&outcome)));
                }
            }
        }
        problems
    });
    assert!(*pairs.lock().expect("count") > 5, "the sweep is not empty");
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

/// atip.checkorder.18 (the failure path of F4): a constraint broken beside a validator's fault is answered by the constraint, and each of
/// the two alone keeps its own code: the constraint's call is not swallowed by the validator, nor the validator's by the constraint.
#[test]
fn atip_checkorder_18_a_constraint_beside_a_validators_fault_answers_and_each_alone_keeps_its_code() {
    // brief approve: `--actor` needs `--relayed-via` (a constraint, USAGE); a blank `--note` is a validator (APPROVAL_INVALID).
    let validator = empty("canon brief approve add-login --approver alice --note \" \"", &[]);
    assert_answer(&validator, "APPROVAL_INVALID", "--note");
    let constraint = empty("canon brief approve add-login --approver alice --note ok --actor impl", &[]);
    assert_answer(&constraint, "USAGE", "--relayed-via");
    let both = empty("canon brief approve add-login --approver alice --note \" \" --actor impl", &[]);
    assert_answer(&both, "USAGE", "--relayed-via");
    let flipped = empty("canon brief approve add-login --actor impl --approver alice --note \" \"", &[]);
    assert_answer(&flipped, "USAGE", "--relayed-via");
    // The name of the human is a domain (0.37), which answers before a constraint: a blank `--approver` is `APPROVAL_INVALID` beside the same constraint.
    let domain = empty("canon brief approve add-login --approver \" \" --note ok", &[]);
    assert_answer(&domain, "APPROVAL_INVALID", "--approver");
    let domain_first = empty("canon brief approve add-login --approver \" \" --note ok --actor impl", &[]);
    assert_answer(&domain_first, "APPROVAL_INVALID", "--approver");
    // policy approve: the same, its `--approver` holds the one domain.
    let domain = empty("canon policy approve --approver \" \"", &[]);
    assert_answer(&domain, "APPROVAL_INVALID", "--approver");
    let domain_first = empty("canon policy approve --approver \" \" --actor impl", &[]);
    assert_answer(&domain_first, "APPROVAL_INVALID", "--approver");
}

// ----------------------------------------------------------------------------------------------------------------- F5

/// atip.checkorder.09: an unknown `CANON_ROLE` is a check that depends on an environment value, so it is a validator and answers
/// last; alone it is `ROLE_INVALID`; an explicit `--as` that is no role is a failure to parse.
#[test]
fn atip_checkorder_09_an_unknown_role_in_the_environment_answers_after_every_other_stateless_check() {
    let bogus = [("CANON_ROLE", "bogus")];
    let rest = "--change add-login --seq 1 --severity note --actor a --summary s";
    // Alone: the role.
    let alone = empty(&format!("canon finding add {rest} --round 1"), &bogus);
    assert_answer(&alone, "ROLE_INVALID", "CANON_ROLE");
    assert_answer(&empty("canon status", &bogus), "ROLE_INVALID", "CANON_ROLE");
    // Beside a broken domain, a missing required argument, a broken constraint, a validator and a failure to parse: those answer.
    assert_answer(&empty(&format!("canon finding add {rest} --round 0"), &bogus), "FINDING_ARGUMENT_INVALID", "--round");
    assert_answer(&empty("canon finding add --seq 1 --severity note --actor a --summary s --round 1", &bogus), "USAGE", "--change");
    assert_answer(&empty(&format!("canon finding add {rest} --round 1 --promote --no-promote"), &bogus), "USAGE", "--promote");
    assert_answer(&empty("canon export --kind change --before 2026-02-30", &bogus), "USAGE", "--before");
    assert_answer(&empty(&format!("canon finding add {rest} --round 1 --zz-first"), &bogus), "USAGE", "--zz-first");
    // The relayed approval's own role validator names the variable too, after the constraints.
    let digest = "add-login=sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let relay = empty(&format!("canon brief approve add-login --relayed-via ask --actor a --approver alice --note ok --digest {digest}"), &bogus);
    assert_answer(&relay, "ROLE_INVALID", "CANON_ROLE");
    let relay_constraint = empty("canon brief approve add-login --actor a --approver alice --note ok", &bogus);
    assert_answer(&relay_constraint, "USAGE", "--relayed-via");
}

/// atip.checkorder.10 (the failure path): a role the explicit `--as` names that is none is a failure to parse, as before (`USAGE`, `--as`),
/// answered by the stage that reads it and ahead of the later ones; and `--as` overrides an unknown `CANON_ROLE`.
#[test]
fn atip_checkorder_10_an_explicit_role_that_is_none_keeps_its_code_and_overrides_the_variable() {
    let rest = "--change add-login --seq 1 --severity note --actor a --summary s";
    let implementer = [("CANON_ROLE", "implementer")];
    assert_answer(&empty(&format!("canon finding add {rest} --round 1 --as zzz"), &implementer), "USAGE", "--as");
    assert_answer(&empty(&format!("canon finding add {rest} --round 0 --as zzz"), &implementer), "USAGE", "--as");
    assert_answer(&empty(&format!("canon finding add {rest} --round 0 --as zzz"), &[("CANON_ROLE", "bogus")]), "USAGE", "--as");
    let overridden = empty(&format!("canon finding add {rest} --round 1 --as implementer"), &[("CANON_ROLE", "bogus")]);
    assert_ne!(overridden.code.as_deref(), Some("ROLE_INVALID"), "{overridden:?}");
    assert_eq!(empty(&format!("canon finding add {rest} --round 0 --as implementer"), &[("CANON_ROLE", "bogus")]).code.as_deref(), Some("FINDING_ARGUMENT_INVALID"));
}

// ----------------------------------------------------------------------------------------------------------------- F6

/// atip.checkorder.11: the `field` of a violated rule is the one the contract's table names, for each of the five rule kinds.
#[test]
fn atip_checkorder_11_the_field_of_each_kind_of_rule_follows_the_contracts_table() {
    let implementer = [("CANON_ROLE", "implementer")];
    // requires: the first name of `then` the call does not give.
    assert_answer(&empty("canon ingest plans --dialect openspec", &[]), "USAGE", "--plan-source");
    assert_answer(&empty("canon migrate --note x", &implementer), "USAGE", "--relayed-via");
    // forbids: the first name of `then` the call gives, in the rule's order, not the call's.
    assert_answer(&empty("canon evidence add --all-scenarios --report junit:r.xml --task add-login#1 --scenario auth.login.01 --class test-run --ref r", &implementer), "EVIDENCE_ARGUMENT_CONFLICT", "--scenario");
    assert_answer(&empty("canon evidence add --all-scenarios --report junit:r.xml --scenario auth.login.01 --task add-login#1 --class test-run --ref r", &implementer), "EVIDENCE_ARGUMENT_CONFLICT", "--scenario");
    // atMostOne: the first of the names the call gives, in the rule's order.
    assert_answer(&empty("canon schema --error X --errors", &[]), "USAGE", "--errors");
    assert_answer(&empty("canon schema --errors --error X", &[]), "USAGE", "--errors");
    assert_answer(&empty("canon finding add --change add-login --round 1 --seq 1 --severity note --actor a --summary s --no-promote --promote", &implementer), "USAGE", "--promote");
    // exactlyOne, several given: the first given in the rule's order; none given: the first name of the rule.
    assert_answer(&empty("canon retrieve --for-role reviewer --domain dev --regime reviewer/canon/join-spine/9c93d024b1a2", &[]), "USAGE", "--regime");
    assert_answer(&empty("canon retrieve --for-role reviewer", &[]), "USAGE", "--regime");
    assert_answer(&empty("canon review scenario auth.login.01 --actor a --original-spec-ref HEAD --upstream-ref HEAD", &implementer), "USAGE", "--upstream-ref");
    assert_answer(&empty("canon review scenario auth.login.01 --actor a", &implementer), "USAGE", "--upstream-ref");
    assert_answer(&empty("canon review change add-login --actor a", &implementer), "USAGE", "--upstream-ref");
    // atLeastOne: the first name of the rule when the call gives none of them.
    assert_answer(&empty("canon evidence add --scenario auth.login.01", &implementer), "USAGE", "--class");
    assert_answer(&empty("canon evidence add --scenario auth.login.01 --class test-run", &implementer), "USAGE", "--ref");
    assert_answer(&empty("canon evidence add --class test-run --ref r", &implementer), "EVIDENCE_REQUIRED", "--task");
    assert_answer(&empty("canon evidence approval-payload --approver a --approval-at 2030-01-01", &implementer), "EVIDENCE_REQUIRED", "--task");
}

/// atip.checkorder.12 (every rule): the answer for each rule of each command is the code of the rule and the field the table names for
/// the way the rule is broken: held to the document's rules by the same function the sweeps use.
#[test]
fn atip_checkorder_12_every_rule_is_answered_with_the_field_of_the_first_argument_at_fault() {
    let docs = commands();
    let count = Mutex::new(0usize);
    let problems = in_parallel(&docs, |doc| {
        let mut problems = Vec::new();
        let path = doc["command"].as_str().unwrap_or_default();
        let rules = constraints(doc);
        for at in 0..rules.len() {
            let Some(found) = violate(doc, &rules, at) else { continue };
            let given = given_from(doc, &found.assignment);
            let Some((first, fault)) = violations(&rules, &given).into_iter().next() else { continue };
            let expected = (Some(rule_support::code_of(&rules[first])), Some(field_for(doc, &fault)));
            let call = render(doc, &found.assignment, false);
            let outcome = run(doc, &found.assignment, &call);
            *count.lock().expect("count") += 1;
            if answer(&outcome) != expected {
                problems.push(format!("`{path}`: `{}` breaks {}; answered {:?}, not {expected:?}", call.join(" "), rule_support::label(&rules[at]), answer(&outcome)));
            }
        }
        problems
    });
    assert!(*count.lock().expect("count") > 50, "the sweep is not empty");
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

/// atip.checkorder.19 (the failure path of F6): a call that satisfies a rule is not answered with that rule's `field`, for each of the
/// five kinds; its twin that breaks the rule is. The near miss is the case a table that names a field for every call would get wrong.
#[test]
fn atip_checkorder_19_a_call_that_satisfies_a_rule_is_not_answered_with_the_rules_field() {
    let implementer = [("CANON_ROLE", "implementer")];
    // (rule kind, the call that satisfies the rule, the call that breaks it, the field the broken call names)
    let pairs: [(&str, &str, &str, &str); 5] = [
        ("requires", "canon ingest plans --dialect openspec --plan-source plans", "canon ingest plans --dialect openspec", "--plan-source"),
        ("forbids", "canon evidence add --all-scenarios --report junit:r.xml --class test-run --ref r", "canon evidence add --all-scenarios --report junit:r.xml --scenario auth.login.01 --class test-run --ref r", "--scenario"),
        ("atMostOne", "canon schema --error CHANGE_EXISTS", "canon schema --error CHANGE_EXISTS --errors", "--errors"),
        ("exactlyOne", "canon retrieve --for-role reviewer --regime reviewer/canon/join-spine/9c93d024b1a2", "canon retrieve --for-role reviewer", "--regime"),
        ("atLeastOne", "canon evidence add --scenario auth.login.01 --class test-run --ref r", "canon evidence add --scenario auth.login.01", "--class"),
    ];
    for (kind, satisfied, broken, field) in pairs {
        let outcome = empty(broken, &implementer);
        assert!(refused(&outcome) && outcome.field.as_deref() == Some(field), "{kind}: `{broken}` names {field}: {outcome:?}");
        let outcome = empty(satisfied, &implementer);
        assert_ne!(outcome.field.as_deref(), Some(field), "{kind}: `{satisfied}` satisfies the rule and is not answered with {field}: {outcome:?}");
        assert!(outcome.changed.is_empty(), "{kind}: a call the stateless stage lets through wrote {:?}", outcome.changed);
    }
}

// ------------------------------------------------------------------------------------------------------------- --input

/// atip.checkorder.13: the order is the same with `--input` and without it: a document that breaks a domain and a constraint is
/// answered by the domain, a missing required key by the missing key, and the code is the one argv gives.
#[test]
fn atip_checkorder_13_the_order_is_the_same_through_an_input_document() {
    let implementer = [("CANON_ROLE", "implementer")];
    let flag = empty("canon finding add --change add-login --seq 1 --severity note --actor a --summary s --round 0 --promote --no-promote", &implementer);
    let document = empty(r#"canon finding add --input '{"change":"add-login","seq":1,"severity":"note","actor":"a","summary":"s","round":0,"promote":true,"noPromote":true}'"#, &implementer);
    assert_eq!(flag.code, document.code, "{flag:?} {document:?}");
    assert_eq!(flag.code.as_deref(), Some("FINDING_ARGUMENT_INVALID"));
    assert!(document.field.as_deref().is_some_and(|field| field.ends_with("round")), "{document:?}");
    let missing_flag = empty("canon finding add --seq 1 --severity note --actor a --summary s --round 0", &implementer);
    let missing_document = empty(r#"canon finding add --input '{"seq":1,"severity":"note","actor":"a","summary":"s","round":0}'"#, &implementer);
    assert_eq!(missing_flag.code, missing_document.code, "{missing_flag:?} {missing_document:?}");
    assert_eq!(missing_flag.code.as_deref(), Some("USAGE"));
    assert!(missing_document.field.as_deref().is_some_and(|field| field.ends_with("change")), "{missing_document:?}");
    let constraint_flag = empty("canon finding add --change add-login --seq 1 --severity note --actor a --summary s --round 1 --promote --no-promote", &implementer);
    let constraint_document = empty(r#"canon finding add --input '{"change":"add-login","seq":1,"severity":"note","actor":"a","summary":"s","round":1,"promote":true,"noPromote":true}'"#, &implementer);
    assert_eq!(constraint_flag.code, constraint_document.code, "{constraint_flag:?} {constraint_document:?}");
    assert_eq!(constraint_flag.code.as_deref(), Some("USAGE"));
}

// ------------------------------------------------------------------------------------------------------------ the tables

/// atip.checkorder.14: the rules clap used to enforce while it parsed (`conflicts_with`, `requires`, `required_unless_present`,
/// `ArgGroup`) are the tool's tables, so no relation is declared to clap: the command tree has none, and the parser alone accepts a
/// call that breaks one, which the stateless stage then refuses.
#[test]
fn atip_checkorder_14_the_command_tree_declares_no_relation_to_clap() {
    let mut root = canon_cli::cli::command();
    root.build();
    let mut leaves = 0;
    fn walk(command: &clap::Command, leaves: &mut usize, problems: &mut Vec<String>) {
        let children: Vec<&clap::Command> = command.get_subcommands().filter(|sub| sub.get_name() != "help").collect();
        if children.is_empty() {
            *leaves += 1;
            if command.get_groups().any(|group| group.is_required_set() || !group.clone().is_multiple()) {
                problems.push(format!("`{}` declares an ArgGroup", command.get_name()));
            }
            for arg in command.get_arguments() {
                if !command.get_arg_conflicts_with(arg).is_empty() {
                    problems.push(format!("`{}` --{} declares a conflict", command.get_name(), arg.get_id()));
                }
            }
        }
        for child in children {
            walk(child, leaves, problems);
        }
    }
    let mut problems = Vec::new();
    walk(&root, &mut leaves, &mut problems);
    assert!(leaves > 60, "{leaves} leaves");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    // `--promote --no-promote` parses (the parser has no conflict) and is refused by the tables.
    let relaxed = canon_cli::arg_rules::relaxed::tree(canon_cli::cli::command());
    relaxed.try_get_matches_from(["canon", "finding", "list", "--change", "add-login", "--all", "--limit", "5"]).expect("the parser has no conflict to refuse");
}

/// atip.checkorder.15 (the failure path): the order table is not decorative: a rule table row that is removed makes the sweeps fail,
/// which is checked here by the rules a document lists being the ones `rules_of` gives (the document is built from it).
#[test]
fn atip_checkorder_15_the_document_lists_the_rules_the_stateless_stage_enforces() {
    let mut root = canon_cli::cli::command();
    root.build();
    let mut checked = 0;
    for doc in rule_support::docs() {
        let path = doc["command"].as_str().unwrap_or_default();
        let node = path.split(' ').fold(&root, |command, word| command.find_subcommand(word).expect("a command of the tree"));
        let enforced: Vec<serde_json::Value> = canon_cli::arg_rules::rules_of(node, path).iter().map(|rule| serde_json::to_value(rule.doc()).expect("a rule")).collect();
        let listed: Vec<Value> = doc["constraints"].as_array().cloned().unwrap_or_default();
        assert_eq!(listed, enforced, "`{path}`: the document lists the rules the stage enforces, in its order");
        checked += listed.len();
    }
    assert!(checked > 50, "{checked} rules");
}

// ------------------------------------------------------------------------------------------ a tool-wide option before the command

/// atip.checkorder.17: a `--as` that is no role, written before the command words or after a group word, is a failure to parse at the place it
/// was written: it answers ahead of an unknown command word, a missing command or a group without a command that follows it; an unknown option
/// or an unknown command word written before it still answers first. And the refusal of two options that exclude each other names the closed
/// set of the one at fault.
#[test]
fn atip_checkorder_17_a_leading_unknown_role_answers_before_an_unknown_or_missing_command() {
    for line in [
        "canon --as zzz nosuch",
        "canon --as zzz",
        "canon --as=zzz finding nosuch",
        "canon --as zzz finding",
        "canon --json --as zzz nosuch",
        "canon --repo . --as zzz status",
        "canon finding --as zzz nosuch",
        "canon finding --as=zzz nosuch",
        "canon finding --repo . --as zzz nosuch",
        "canon finding --as zzz",
        "canon finding --json --as zzz nosuch",
    ] {
        let outcome = empty(line, &[]);
        assert_eq!((outcome.code.as_deref(), outcome.field.as_deref()), (Some("USAGE"), Some("--as")), "`{line}`: {outcome:?}");
        assert_eq!(outcome.exit, 2, "`{line}`");
        assert!(outcome.changed.is_empty());
    }
    // Written the same way after a leaf it was always answered where it stood; an unknown option before it answers first.
    assert_answer(&empty("canon status --as zzz", &[]), "USAGE", "--as");
    assert_answer(&empty("canon --bogus --as zzz nosuch", &[]), "USAGE", "--bogus");
    // An unknown command word written before the `--as` answers first, and a `--as` that names a role is not a failure.
    for line in ["canon finding nosuch --as zzz", "canon finding --as reviewer nosuch", "canon finding --as=reviewer nosuch"] {
        let word = empty(line, &[]);
        assert_eq!((word.code.as_deref(), word.field.as_deref()), (Some("UNKNOWN_COMMAND"), Some("command")), "`{line}`: {word:?}");
    }
    let word = empty("canon nosuch", &[]);
    assert_eq!((word.code.as_deref(), word.field.as_deref()), (Some("UNKNOWN_COMMAND"), Some("command")), "{word:?}");
    let bare = empty("canon finding", &[]);
    assert_eq!(bare.code.as_deref(), Some("COMMAND_REQUIRED"), "{bare:?}");
    // The pair that excludes itself names the closed set of the argument at fault.
    let pair = empty("canon init --policy light --no-policy", &[]);
    assert_eq!((pair.code.as_deref(), pair.field.as_deref()), (Some("USAGE"), Some("--policy")), "{pair:?}");
    let document: Value = serde_json::from_str(&pair.stdout).expect("a failure envelope");
    assert_eq!(document["error"]["expected"], serde_json::json!(["light", "standard", "strict"]), "{document}");
}
