//! The invocations that probe a declared constraint, domain or number range (approaches/checker.md 6:
//! `L1.constraints`, `L1.domains`, `L1.numbers`).
//!
//! A probe is built from a passing invocation of the fixture, changing the fewest arguments. What it must do is
//! decided from the self-description alone:
//!
//! - **Constraints** (spec 10.8). The rules are compared over the finite table of which named arguments are given
//!   and which `in` class each value falls in. Rules with the same set of violating configurations are one *group*
//!   (one test obligation: one invocation that violates the group and nothing else). A group no configuration
//!   violates alone, because every violating one also breaks `required` or another rule, is *entailed*.
//! - **Domains** (spec 10.7.4). Candidate values at each bound of the profile and one step beyond it are classified
//!   by the document's own domain: a value the domain admits must be accepted, one it does not must be refused.
//! - **Numbers** (spec 10.7.5). `9007199254740992` and its negation, one beyond the safe range.

use std::collections::BTreeSet;

use serde_json::Value;

use super::invocation::{
    args, arg_given, normalise, parse, remove_operand, remove_option, render, requirement_problems, set_operand, set_option,
    value_problem, violated_constraints, Args, Invocation, Item,
};
use super::model::{Arg, CommandDoc, Constraint};

/// The most configurations of the constraint table the checker enumerates.
const MAX_CONFIGS: usize = 200_000;
/// The longest string a probe sends (one argv word is bounded by the operating system).
const MAX_PROBE_CHARS: u64 = 100_000;

/// The argv word of a typed value: a string as it is, anything else as its JSON text.
pub fn word_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The typed value of an argv word for `arg`: parsed for `json`, the word as a string otherwise (spec: `Args` holds argv words).
pub fn typed(arg: &Arg, word: &str) -> Value {
    if arg.value_type == "json" {
        serde_json::from_str(word).unwrap_or_else(|_| Value::String(word.to_string()))
    } else {
        Value::String(word.to_string())
    }
}

/// A value for `arg` that is valid (type and domain) and not one of `avoid`, found among a few candidates; `None` when none fits.
pub fn generate(arg: &Arg, avoid: &[Value]) -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();
    match arg.value_type.as_str() {
        "enum" => candidates.extend(arg.possible_values.iter().cloned()),
        "integer" | "count" => candidates.extend(["1", "0", "2", "5", "10", "100", "3", "7", "-1"].map(String::from)),
        "number" => candidates.extend(["1.5", "1", "0.5", "2", "10", "100", "0"].map(String::from)),
        "json" => candidates.extend(["{}", "[]", "\"x\"", "1", "true", "{\"a\":1}"].map(String::from)),
        _ => candidates.extend(["x", "a", "0", "a-b", "a_b", "abc", "A", "item", "example", "x1", "a.b"].map(String::from)),
    }
    if let Some(min) = arg.domain.as_ref().and_then(|domain| domain.get("minLength")).and_then(Value::as_u64) {
        if min > 0 && min <= MAX_PROBE_CHARS {
            for letter in ["x", "a", "0"] {
                candidates.push(letter.repeat(usize::try_from(min).unwrap_or(1)));
            }
        }
    }
    if let Some(domain) = &arg.domain {
        for key in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] {
            if let Some(bound) = domain.get(key).and_then(Value::as_i64) {
                candidates.extend([bound - 1, bound, bound + 1].map(|number| number.to_string()));
            }
        }
    }
    let avoid: Vec<Value> = avoid.iter().map(|value| normalise(&arg.value_type, value)).collect();
    candidates.into_iter().find(|word| {
        let value = typed(arg, word);
        value_problem(arg, &value).is_none() && !avoid.contains(&normalise(&arg.value_type, &value))
    })
}

// ---- constraints ----

/// What one rule group needs: an invocation that violates only it, or nothing to run.
#[derive(Debug, Clone, PartialEq)]
pub enum GroupOutcome {
    /// Run `words` (an argv without the tool name): it must be refused with one of the group's codes.
    Witness { words: Vec<String> },
    /// Every invocation that breaks the group also breaks `required` or another group: `broken_with` has, for each
    /// violating configuration, the other groups it breaks and whether it breaks `required` too.
    Entailed { broken_with: Vec<Broken> },
    /// The group can be violated alone but no invocation could be built from the fixture.
    Unprovided(String),
}

/// What one configuration that violates an entailed group breaks besides it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Broken {
    /// The indices of the other groups the configuration violates.
    pub groups: Vec<usize>,
    /// Whether it also leaves a required argument out.
    pub required: bool,
}

/// One group of equivalent rules and what the checker does with it.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupPlan {
    /// Indices into `CommandDoc::constraints`.
    pub members: Vec<usize>,
    /// The codes the members declare (`USAGE` for a rule with no `code`): more than one is an ambiguous refusal.
    pub codes: BTreeSet<String>,
    pub outcome: GroupOutcome,
}

/// How an argument of the constraint table is given.
#[derive(Debug, Clone, PartialEq)]
enum State {
    Absent,
    /// Given with the value of pool entry `i`.
    Given(usize),
}

/// The values an argument can have in the table: each literal of an `in` of a rule on it, and one value in none of them.
#[derive(Debug, Clone)]
struct Pool {
    name: String,
    /// The normalised `in` literals; the last entry of the table's choices is "no literal" and has no entry here.
    literals: Vec<Value>,
    flag: bool,
}

impl Pool {
    /// The number of states besides absent: one per literal, one for "other" (a flag has just the one).
    fn size(&self) -> usize {
        if self.flag {
            1
        } else {
            self.literals.len() + 1
        }
    }
}

fn pool_of(doc: &CommandDoc, name: &str) -> Option<Pool> {
    let arg = doc.arg(name)?;
    let mut literals: Vec<Value> = Vec::new();
    for constraint in &doc.constraints {
        if constraint.if_name.as_deref() == Some(name) {
            for value in constraint.if_in.iter().flatten() {
                let value = normalise(&arg.value_type, value);
                if !literals.contains(&value) {
                    literals.push(value);
                }
            }
        }
    }
    Some(Pool {
        name: name.to_string(),
        literals,
        flag: arg.is_flag(),
    })
}

/// The names the constraints mention, once each, in order of first mention, and only those the command declares.
fn named(doc: &CommandDoc) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut push = |name: &str| {
        if doc.arg(name).is_some() && !names.iter().any(|known| known == name) {
            names.push(name.to_string());
        }
    };
    for Constraint { names: listed, if_name, then, .. } in &doc.constraints {
        for name in listed.iter().chain(if_name).chain(then.iter()) {
            push(name);
        }
    }
    names
}

/// `base` with the arguments of `names` set as `states` say; `None` when the configuration cannot be an invocation
/// (an operand given after one left out).
fn apply(doc: &CommandDoc, base: &Args, pools: &[Pool], states: &[State], fills: &dyn Fn(&Arg, usize) -> Option<Value>) -> Option<Args> {
    let mut result = base.clone();
    let mut operand_gap = false;
    for (pool, state) in pools.iter().zip(states) {
        let arg = doc.arg(&pool.name)?;
        if arg.is_option {
            result.options.remove(&pool.name);
            if let State::Given(choice) = state {
                let value = if pool.flag {
                    if arg.value_type == "count" { Value::from(1) } else { Value::Bool(true) }
                } else {
                    fills(arg, *choice)?
                };
                result.options.insert(pool.name.clone(), if arg.repeatable && !pool.flag { Value::Array(vec![value]) } else { value });
            }
        }
    }
    let mut operands = pools.iter().zip(states).filter_map(|(pool, state)| {
        let index = doc.operands.iter().position(|operand| operand.name == pool.name)?;
        Some((index, pool, state))
    }).collect::<Vec<_>>();
    operands.sort_by_key(|(index, _, _)| *index);
    for (index, pool, state) in operands {
        let arg = doc.operands.get(index)?;
        match state {
            State::Absent => {
                result.operands.truncate(index);
                operand_gap = true;
            }
            State::Given(choice) => {
                if operand_gap {
                    return None;
                }
                while result.operands.len() < index {
                    let earlier = doc.operands.get(result.operands.len())?;
                    result.operands.push(typed(earlier, &generate(earlier, &[])?));
                }
                let value = fills(arg, *choice)?;
                if result.operands.len() == index {
                    result.operands.push(value);
                } else {
                    result.operands[index] = value;
                }
            }
        }
        let _ = pool;
    }
    Some(result)
}

/// The state of each pooled argument in `base`.
fn states_of(doc: &CommandDoc, base: &Args, pools: &[Pool]) -> Vec<State> {
    pools
        .iter()
        .map(|pool| {
            let Some(arg) = doc.arg(&pool.name) else {
                return State::Absent;
            };
            if !arg_given(doc, base, &pool.name) {
                return State::Absent;
            }
            if pool.flag {
                return State::Given(0);
            }
            let value = if arg.is_option {
                base.options.get(&pool.name).cloned()
            } else {
                doc.operands
                    .iter()
                    .position(|operand| operand.name == pool.name)
                    .and_then(|index| base.operands.get(index).cloned())
            };
            let first = match value {
                Some(Value::Array(values)) => values.into_iter().next(),
                other => other,
            };
            let normalised = first.map(|value| normalise(&arg.value_type, &value));
            let choice = normalised
                .and_then(|value| pool.literals.iter().position(|literal| *literal == value))
                .unwrap_or(pool.literals.len());
            State::Given(choice)
        })
        .collect()
}

/// The value an argument takes for pool choice `choice`: the literal, or a value in no literal of the argument.
fn fill_for(pools: &[Pool]) -> impl Fn(&Arg, usize) -> Option<Value> + '_ {
    move |arg: &Arg, choice: usize| {
        let pool = pools.iter().find(|pool| pool.name == arg.name)?;
        match pool.literals.get(choice) {
            Some(literal) => {
                let word = word_of(literal);
                value_problem(arg, &typed(arg, &word)).is_none().then(|| typed(arg, &word))
            }
            None => generate(arg, &pool.literals).map(|word| typed(arg, &word)),
        }
    }
}

/// The groups of equivalent rules of `doc` and what to run for each, built from the passing invocation `base` (an argv without the tool name).
/// The rules are judged by the checker's own reading of the document (`violated_constraints`), so the plan and the
/// `L1.examples` check agree on what a rule means.
pub fn plan_constraints(doc: &CommandDoc, base: &Invocation) -> Vec<GroupPlan> {
    let all_unprovided = |why: String| -> Vec<GroupPlan> {
        let mut seen: Vec<GroupPlan> = Vec::new();
        for (index, constraint) in doc.constraints.iter().enumerate() {
            seen.push(GroupPlan {
                members: vec![index],
                codes: BTreeSet::from([constraint.code.clone().unwrap_or_else(|| "USAGE".to_string())]),
                outcome: GroupOutcome::Unprovided(why.clone()),
            });
        }
        seen
    };
    let base_args = args(doc, base);
    let problems = requirement_problems(doc, &base_args);
    let violated = violated_constraints(doc, &base_args);
    if !problems.is_empty() || !violated.is_empty() {
        let what = problems
            .first()
            .cloned()
            .or_else(|| violated.first().and_then(|index| doc.constraints.get(*index)).map(Constraint::label))
            .unwrap_or_default();
        return all_unprovided(format!("the fixture invocation `{}` does not satisfy the command's requirements: {what}", render(base).join(" ")));
    }
    let names = named(doc);
    let pools: Vec<Pool> = names.iter().filter_map(|name| pool_of(doc, name)).collect();
    let sizes: Vec<usize> = pools.iter().map(|pool| pool.size() + 1).collect();
    let total = sizes.iter().try_fold(1_usize, |product, size| product.checked_mul(*size));
    if total.is_none_or(|total| total > MAX_CONFIGS) {
        return all_unprovided("the rules name too many arguments to compare".to_string());
    }
    let fills = fill_for(&pools);
    let base_states = states_of(doc, &base_args, &pools);
    // Every configuration: its states, what it violates, and whether it leaves a required argument out.
    struct Config {
        states: Vec<State>,
        violated: Vec<usize>,
        required_broken: bool,
    }
    let mut configs: Vec<Config> = Vec::new();
    let mut counter = vec![0_usize; pools.len()];
    loop {
        let states: Vec<State> = counter.iter().map(|step| if *step == 0 { State::Absent } else { State::Given(*step - 1) }).collect();
        if let Some(applied) = apply(doc, &base_args, &pools, &states, &fills) {
            configs.push(Config {
                states,
                violated: violated_constraints(doc, &applied),
                required_broken: !requirement_problems(doc, &applied).is_empty(),
            });
        }
        let mut at = 0;
        loop {
            if at == counter.len() {
                break;
            }
            counter[at] += 1;
            if counter[at] < sizes[at] {
                break;
            }
            counter[at] = 0;
            at += 1;
        }
        if at == counter.len() {
            break;
        }
    }
    // The rules with the same set of violating configurations are one group.
    let mut groups: Vec<(Vec<usize>, Vec<usize>)> = Vec::new();
    for (index, _) in doc.constraints.iter().enumerate() {
        let violating: Vec<usize> = configs
            .iter()
            .enumerate()
            .filter(|(_, config)| config.violated.contains(&index))
            .map(|(at, _)| at)
            .collect();
        match groups.iter_mut().find(|(_, set)| *set == violating) {
            Some((members, _)) => members.push(index),
            None => groups.push((vec![index], violating)),
        }
    }
    let group_of = |constraint: usize| groups.iter().position(|(members, _)| members.contains(&constraint));
    groups
        .iter()
        .map(|(members, violating)| {
            let codes = members
                .iter()
                .filter_map(|index| doc.constraints.get(*index))
                .map(|constraint| constraint.code.clone().unwrap_or_else(|| "USAGE".to_string()))
                .collect();
            let mut candidates: Vec<&Config> = configs
                .iter()
                .filter(|config| !config.required_broken && config.violated == *members)
                .collect();
            let distance = |config: &Config| -> usize {
                config.states.iter().zip(&base_states).filter(|(state, base)| state != base).count()
            };
            candidates.sort_by_key(|config| (distance(config), config.states.iter().filter(|state| **state != State::Absent).count()));
            let mut why = String::new();
            for config in &candidates {
                match build(doc, base, &base_args, &pools, &base_states, &config.states, &fills) {
                    Ok(words) => {
                        let reparsed = parse(doc, &words).map(|invocation| violated_constraints(doc, &args(doc, &invocation)));
                        if reparsed.as_deref() == Ok(members.as_slice()) {
                            return GroupPlan { members: members.clone(), codes, outcome: GroupOutcome::Witness { words } };
                        }
                        why = format!("the invocation built for it, `{}`, is read differently by the grammar", words.join(" "));
                    }
                    Err(reason) => why = reason,
                }
            }
            if !candidates.is_empty() {
                return GroupPlan { members: members.clone(), codes, outcome: GroupOutcome::Unprovided(why) };
            }
            let broken_with = violating
                .iter()
                .filter_map(|at| configs.get(*at))
                .map(|config| {
                    let mut others: Vec<usize> = config
                        .violated
                        .iter()
                        .filter(|index| !members.contains(index))
                        .filter_map(|index| group_of(*index))
                        .collect();
                    others.sort_unstable();
                    others.dedup();
                    Broken { groups: others, required: config.required_broken }
                })
                .collect();
            let _ = &args_of_unused;
            GroupPlan { members: members.clone(), codes, outcome: GroupOutcome::Entailed { broken_with } }
        })
        .collect()
}

/// A marker so the unused-parameter lint of the inner closure stays quiet.
const fn args_of_unused() {}

/// The argv of the configuration `states`, as the fewest changes to `base`.
fn build(
    doc: &CommandDoc,
    base: &Invocation,
    base_args: &Args,
    pools: &[Pool],
    base_states: &[State],
    states: &[State],
    fills: &dyn Fn(&Arg, usize) -> Option<Value>,
) -> Result<Vec<String>, String> {
    let mut invocation = base.clone();
    // Operands first: leaving one out drops those after it, giving one gives those before it.
    let mut operand_changes: Vec<(usize, &Pool, &State)> = Vec::new();
    for ((pool, state), before) in pools.iter().zip(states).zip(base_states) {
        if state == before {
            continue;
        }
        let Some(arg) = doc.arg(&pool.name) else { continue };
        if arg.is_option {
            match state {
                State::Absent => remove_option(&mut invocation, &pool.name),
                State::Given(choice) => {
                    if pool.flag {
                        remove_option(&mut invocation, &pool.name);
                        set_option(&mut invocation, &pool.name, None);
                    } else {
                        let value = fills(arg, *choice).ok_or_else(|| format!("no valid value of --{} outside the values the rules name", pool.name))?;
                        set_option(&mut invocation, &pool.name, Some(word_of(&value)));
                    }
                }
            }
        } else if let Some(index) = doc.operands.iter().position(|operand| operand.name == pool.name) {
            operand_changes.push((index, pool, state));
        }
    }
    operand_changes.sort_by_key(|(index, _, _)| *index);
    for (index, pool, state) in operand_changes {
        match state {
            State::Absent => remove_operand(&mut invocation, index),
            State::Given(choice) => {
                let count = invocation.items.iter().filter(|item| matches!(item, Item::Operand(_))).count();
                for earlier in count..index {
                    let operand = doc.operands.get(earlier).ok_or("the operands before it are not declared")?;
                    let word = generate(operand, &[]).ok_or_else(|| format!("no valid value of the operand {}", operand.name))?;
                    set_operand(&mut invocation, earlier, word);
                }
                let arg = doc.arg(&pool.name).ok_or("the operand is not declared")?;
                let value = fills(arg, *choice).ok_or_else(|| format!("no valid value of the operand {} outside the values the rules name", pool.name))?;
                set_operand(&mut invocation, index, word_of(&value));
            }
        }
    }
    let _ = base_args;
    Ok(render(&invocation))
}

// ---- domains ----

/// What an invocation with one argument changed must do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// The domain admits the value: it is not refused for it.
    Accepted,
    /// The domain does not admit the value: it is refused (exit 2, `field` naming the argument).
    Refused,
}

/// One value to try for an argument.
#[derive(Debug, Clone, PartialEq)]
pub struct DomainProbe {
    /// The argv word: a string as is, a JSON value as its text.
    pub word: String,
    pub expect: Expect,
    /// What it probes, for a report: `minimum - 1`, ...
    pub why: String,
}

/// The probes of one argument's domain, and whether part of it (a `pattern`, `enum`, `const`, `properties`, `required`,
/// `additionalProperties`, `items`) can only be probed with the fixture's `domainRejects`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DomainPlan {
    pub probes: Vec<DomainProbe>,
    pub needs_rejects: bool,
    /// The keywords the probes could not cover (a bound too long to send).
    pub skipped: Vec<String>,
}

fn number(domain: &Value, key: &str) -> Option<f64> {
    domain.get(key).and_then(Value::as_f64)
}

fn format_number(value: f64, integral: bool) -> String {
    if integral || value.fract() == 0.0 && value.abs() < 9.0e15 {
        // The bound is a whole number of at most 15 digits.
        #[expect(clippy::cast_possible_truncation, reason = "the value is whole and below 9e15, so the cast is exact")]
        let whole = value as i64;
        format!("{whole}")
    } else {
        format!("{value}")
    }
}

/// Probes for the domain of `arg` by its keywords (spec 10.7.4): values at each bound and one step beyond it, a non-multiple,
/// strings one character short of and one beyond the length limits, a JSON value of a type the domain excludes. Each value is
/// classified by the domain itself.
pub fn domain_plan(arg: &Arg) -> DomainPlan {
    let mut plan = DomainPlan::default();
    let Some(domain) = arg.domain.as_ref().filter(|domain| domain.is_object()) else {
        return plan;
    };
    plan.needs_rejects = ["pattern", "enum", "const", "properties", "required", "additionalProperties", "items"]
        .iter()
        .any(|key| domain.get(*key).is_some());
    let integral = matches!(arg.value_type.as_str(), "integer" | "count");
    let is_json = arg.value_type == "json";
    let mut words: Vec<(String, String)> = Vec::new();
    // Numbers.
    let mut numbers: Vec<(f64, String)> = Vec::new();
    let steps: &[f64] = if integral { &[-1.0, 0.0, 1.0] } else { &[-1.0, -0.5, 0.0, 0.5, 1.0] };
    for key in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] {
        if let Some(bound) = number(domain, key) {
            for step in steps {
                numbers.push((bound + step, format!("{key} {}{}", if *step < 0.0 { "- " } else if *step > 0.0 { "+ " } else { "" }, if *step == 0.0 { String::new() } else { step.abs().to_string() }).trim().to_string()));
            }
        }
    }
    if let Some(step) = number(domain, "multipleOf").filter(|step| *step > 0.0) {
        let low = number(domain, "minimum").or_else(|| number(domain, "exclusiveMinimum")).unwrap_or(0.0);
        let first = (low / step).ceil() * step;
        for count in 0..3 {
            let multiple = first + step * f64::from(count);
            numbers.push((multiple, "a multiple of multipleOf".to_string()));
            numbers.push((multiple + if integral { 1.0 } else { step / 2.0 }, "not a multiple of multipleOf".to_string()));
        }
    }
    let numeric = domain.as_object().is_some_and(|keys| keys.keys().any(|key| matches!(key.as_str(), "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" | "multipleOf")));
    if numeric {
        for (value, why) in numbers {
            if arg.value_type == "count" && value < 0.0 {
                continue;
            }
            words.push((format_number(value, integral), why));
        }
    }
    // Strings.
    let min_length = domain.get("minLength").and_then(Value::as_u64);
    let max_length = domain.get("maxLength").and_then(Value::as_u64);
    if min_length.is_some() || max_length.is_some() {
        let lengths: Vec<(u64, &str)> = [
            min_length.and_then(|min| min.checked_sub(1)).map(|n| (n, "minLength - 1")),
            min_length.map(|n| (n, "minLength")),
            max_length.map(|n| (n, "maxLength")),
            max_length.map(|n| (n + 1, "maxLength + 1")),
        ]
        .into_iter()
        .flatten()
        .collect();
        for (length, why) in lengths {
            if length > MAX_PROBE_CHARS {
                plan.skipped.push(format!("{why} is {length} characters"));
                continue;
            }
            let length = usize::try_from(length).unwrap_or(0);
            // A character the rest of the domain (a pattern) admits, found with the shortest length it can have.
            let letter = ["a", "x", "0", "A", "-", "_"].into_iter().find(|letter| {
                let sample = letter.repeat(length.clamp(min_length.map_or(1, |n| usize::try_from(n).unwrap_or(1)).max(1), max_length.map_or(usize::MAX, |n| usize::try_from(n).unwrap_or(usize::MAX)).max(1)));
                value_problem(arg, &typed(arg, &if is_json { Value::String(sample).to_string() } else { sample })).is_none()
            }).unwrap_or("a");
            let text = letter.repeat(length);
            let word = if is_json { Value::String(text).to_string() } else { text };
            words.push((word, why.to_string()));
        }
    }
    // A JSON type the domain excludes.
    if is_json && domain.get("type").is_some() {
        for (word, why) in [("\"x\"", "a string"), ("1.5", "a number"), ("true", "a boolean"), ("{}", "an object"), ("[]", "an array")] {
            words.push((word.to_string(), format!("{why} where `type` excludes it")));
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (word, why) in words {
        if !seen.insert(word.clone()) {
            continue;
        }
        let value = typed(arg, &word);
        let expect = if value_problem(arg, &value).is_none() { Expect::Accepted } else { Expect::Refused };
        // A json value that does not even parse says nothing about the domain.
        if is_json && matches!(value, Value::String(_)) && !word.starts_with('"') {
            continue;
        }
        plan.probes.push(DomainProbe { word, expect, why });
    }
    plan
}

/// `invocation` with `arg` given as `word` and nothing else changed: an option once with that value (a count that many times),
/// an operand in its place.
pub fn with_argument(doc: &CommandDoc, invocation: &Invocation, arg: &Arg, word: &str) -> Option<Invocation> {
    let mut changed = invocation.clone();
    if arg.is_option {
        if arg.value_type == "count" {
            let times: usize = word.parse().ok()?;
            remove_option(&mut changed, &arg.name);
            for _ in 0..times {
                set_option_again(&mut changed, &arg.name);
            }
        } else {
            set_option(&mut changed, &arg.name, Some(word.to_string()));
        }
    } else {
        let index = doc.operands.iter().position(|operand| operand.name == arg.name)?;
        let count = changed.items.iter().filter(|item| matches!(item, Item::Operand(_))).count();
        for earlier in count..index {
            let operand = doc.operands.get(earlier)?;
            set_operand(&mut changed, earlier, generate(operand, &[])?);
        }
        set_operand(&mut changed, index, word.to_string());
    }
    Some(changed)
}

/// One more occurrence of the flag option `name`.
fn set_option_again(invocation: &mut Invocation, name: &str) {
    invocation.items.push(Item::Option { name: name.to_string(), value: None });
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{domain_plan, Expect};
    use crate::l1::model::CommandDoc;

    fn doc_with(option: serde_json::Value) -> CommandDoc {
        CommandDoc::parse(&json!({
            "kind": "command", "command": "a", "path": ["a"], "effect": "read",
            "operands": [], "options": [option], "errors": [], "outputs": [], "examples": [], "schemas": {}
        }))
        .expect("a command")
    }

    #[test]
    fn atip_checkl1_witness_an_integer_domain_gets_a_value_at_each_bound_and_one_beyond() {
        let doc = doc_with(json!({"name": "limit", "valueName": "N", "valueType": "integer", "required": false, "repeatable": false, "default": null, "help": "h", "domain": {"minimum": 1, "maximum": 100}}));
        let plan = domain_plan(&doc.options[0]);
        let by_word = |word: &str| plan.probes.iter().find(|probe| probe.word == word).map(|probe| probe.expect);
        assert_eq!(by_word("0"), Some(Expect::Refused), "minimum - 1");
        assert_eq!(by_word("1"), Some(Expect::Accepted), "minimum");
        assert_eq!(by_word("100"), Some(Expect::Accepted), "maximum");
        assert_eq!(by_word("101"), Some(Expect::Refused), "maximum + 1");
        assert!(!plan.needs_rejects);
    }
}
