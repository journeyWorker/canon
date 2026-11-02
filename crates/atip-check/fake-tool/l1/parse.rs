//! Parsing and enforcement derived from the model of `spec.rs`: argv syntax (spec 9.12), typed values (10.7.2),
//! domains (10.7.4), integers within ±(2^53-1) (10.7.5), constraints (10.8.1) and validators (10.8.3). The seeded
//! violations of this module are runtime toggles of the configuration; each names the model entry it skips.

use std::collections::BTreeMap;

use jsonschema::Draft;
use serde_json::{json, Value};

use super::spec::{Arg, Check, Command, Rule};
use crate::Tool;

/// The largest integer a JSON parser, JavaScript and a Rust `i64` agree on (spec 10.7.5).
const SAFE: i128 = 9_007_199_254_740_991;

/// What argv gave a command.
#[derive(Debug, Default)]
pub struct Parsed {
    /// Each given argument by name, its values in order; a switch holds `true`, a count its number of occurrences.
    pub given: BTreeMap<&'static str, Vec<Value>>,
    /// The first option the command does not have, when the tool defers its refusal behind the requirements (`unknownOptionDeferred`).
    pub deferred_unknown: Option<String>,
}

impl Parsed {
    pub fn has(&self, name: &str) -> bool {
        self.given.contains_key(name)
    }

    pub fn values(&self, name: &str) -> &[Value] {
        self.given.get(name).map_or(&[], Vec::as_slice)
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.values(name).first().and_then(Value::as_str)
    }

    pub fn int(&self, name: &str) -> Option<i64> {
        self.values(name).first().and_then(Value::as_i64)
    }
}

/// Why a command line was not accepted.
#[derive(Debug)]
pub enum Failure {
    /// An option the command does not have.
    Unknown(String),
    /// A refusal with a code; `source` says which rule of the model refused (`parse`, `required`, `domain`,
    /// `constraint`, `validator`).
    Refuse {
        code: String,
        message: String,
        field: Option<String>,
        expected: Option<Vec<String>>,
        source: &'static str,
    },
}

fn usage(source: &'static str, message: String, field: String) -> Failure {
    usage_coded("USAGE", source, message, field)
}

/// A refusal of the model's own rule with `code`.
fn usage_coded(code: &str, source: &'static str, message: String, field: String) -> Failure {
    Failure::Refuse {
        code: code.to_string(),
        message,
        field: Some(field),
        expected: None,
        source,
    }
}

/// Whether the knob `name`, an object from command id to a list, lists `item` for `command`.
fn listed(tool: &Tool, name: &str, command: &str, item: &Value) -> bool {
    tool.config
        .get(name)
        .and_then(|knob| knob.get(command))
        .and_then(Value::as_array)
        .is_some_and(|items| items.contains(item))
}

fn parse_integer(tool: &Tool, arg: &Arg, text: &str) -> Result<Value, Failure> {
    let field = arg.field();
    let digits = text.strip_prefix('-').unwrap_or(text);
    let well_formed = !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'));
    if !well_formed {
        return Err(usage("parse", format!("{text} is not an integer"), field));
    }
    let value = text.parse::<i128>().ok().filter(|value| value.abs() <= SAFE);
    if let Some(value) = value {
        return Ok(json!(i64::try_from(value).unwrap_or_default()));
    }
    if tool.flag("roundBigIntegers") {
        // The seeded violation: a value beyond ±(2^53-1) is rounded to the nearest binary64 value and accepted.
        if let Some(rounded) = text
            .parse::<f64>()
            .ok()
            .and_then(|rounded| format!("{rounded:.0}").parse::<i64>().ok())
        {
            return Ok(json!(rounded));
        }
    }
    Err(usage(
        "parse",
        format!("{text} is outside ±(2^53-1)"),
        field,
    ))
}

/// The typed value of `text` for `arg` (10.7.2).
pub fn typed(tool: &Tool, arg: &Arg, text: &str) -> Result<Value, Failure> {
    let field = arg.field();
    match arg.value_type {
        "integer" => parse_integer(tool, arg, text),
        "number" => {
            let number_like = !text.is_empty()
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.' | b'e' | b'E'));
            match text.parse::<f64>() {
                Ok(number) if number_like && number.is_finite() => Ok(json!(number)),
                _ => Err(usage("parse", format!("{text} is not a number"), field)),
            }
        }
        "enum" => {
            if arg.possible.contains(&text) {
                Ok(json!(text))
            } else {
                Err(Failure::Refuse {
                    code: "USAGE".to_string(),
                    message: format!("{text} is not one of the values of {field}"),
                    field: Some(field),
                    expected: Some(arg.possible.iter().map(ToString::to_string).collect()),
                    source: "parse",
                })
            }
        }
        "json" => match serde_json::from_str::<Value>(text) {
            Ok(value) if !value.is_null() => Ok(value),
            _ => Err(usage("parse", format!("{field} is not JSON"), field)),
        },
        _ => Ok(json!(text)),
    }
}

/// Reads `args` (the whole argv: path words, options and operands) by the grammar of 9.12 against `command`,
/// then enforces required arguments, domains, constraints and validators, in that order (spec 11.7 step 6).
pub fn parse(tool: &Tool, command: &Command, args: &[String], skip_rules: bool) -> Result<Parsed, Failure> {
    let mut parsed = read_words(tool, command, args)?;
    if let Some(text) = parsed.text("input").map(ToString::to_string) {
        merge_input(tool, command, &text, &mut parsed)?;
    }
    enforce(tool, command, &parsed, skip_rules)?;
    Ok(parsed)
}

/// Reads `args` (the whole argv: path words, options and operands) by the grammar of 9.12 against `command`: no
/// requirement, domain, constraint or validator is enforced (spec 11.7 step 3).
pub fn read_words(tool: &Tool, command: &Command, args: &[String]) -> Result<Parsed, Failure> {
    let path_len = command.path().len();
    let mut parsed = Parsed::default();
    let mut path_seen = 0;
    let mut operands: Vec<String> = Vec::new();
    let mut done = false;
    let mut index = 0;
    let mut deferred_unknown: Option<String> = None;
    while index < args.len() {
        let word = &args[index];
        index += 1;
        if done {
            operands.push(word.clone());
            continue;
        }
        if word == "--" {
            done = true;
            continue;
        }
        let plain = |text: Option<&String>| text.is_some_and(|text| text == "-" || !(text.is_empty() || text.starts_with('-')));
        let option = if let Some(rest) = word.strip_prefix("--") {
            let (name, joined) = rest
                .split_once('=')
                .map_or((rest, None), |(name, value)| (name, Some(value.to_string())));
            Some((command.flags().find(|arg| arg.name == name), format!("--{name}"), joined))
        } else if word.len() == 2 && word.starts_with('-') && word.as_bytes()[1].is_ascii_alphabetic() {
            let letter = word.chars().nth(1);
            Some((
                command.flags().find(|arg| arg.short.is_some() && arg.short == letter),
                word.clone(),
                None,
            ))
        } else {
            None
        };
        // Seeded: of two unknown options the tool answers the later one (the checker's own are named `--no-such-*`).
        let later_unknown = tool.flag("unknownOptionLast") && args[index..].iter().any(|later| later.starts_with("--no-such"));
        // Seeded: an unknown option is answered after the values and the required arguments (the checker's own are named `--no-such-*`).
        let deferred = tool.flag("unknownOptionDeferred") && word.starts_with("--no-such");
        let Some((arg, written, joined)) = option else {
            if word != "-" && word.starts_with('-') {
                if later_unknown {
                    continue;
                }
                if deferred {
                    deferred_unknown.get_or_insert_with(|| word.clone());
                    continue;
                }
                return Err(Failure::Unknown(word.clone()));
            }
            if path_seen < path_len && command.path().get(path_seen) == Some(&word.as_str()) {
                path_seen += 1;
            } else {
                operands.push(word.clone());
            }
            continue;
        };
        let Some(arg) = arg else {
            if later_unknown {
                continue;
            }
            if deferred {
                deferred_unknown.get_or_insert(written);
                continue;
            }
            return Err(Failure::Unknown(written));
        };
        if !arg.takes_value() {
            if joined.is_some() {
                return Err(usage("parse", format!("{written} takes no value"), arg.field()));
            }
            let entry = parsed.given.entry(arg.name).or_default();
            if arg.value_type == "count" {
                let count = entry.first().and_then(Value::as_u64).unwrap_or(0) + 1;
                *entry = vec![json!(count)];
            } else {
                *entry = vec![json!(true)];
            }
            continue;
        }
        let text = if let Some(text) = joined {
            text
        } else if plain(args.get(index)) {
            index += 1;
            args[index - 1].clone()
        } else {
            return Err(usage(
                "parse",
                format!("{written} needs a value (a value starting with - needs {written}=<value>)"),
                arg.field(),
            ));
        };
        let value = typed(tool, arg, &text)?;
        let entry = parsed.given.entry(arg.name).or_default();
        if arg.repeatable {
            entry.push(value);
        } else {
            // A repeated single-valued option: the last one wins.
            *entry = vec![value];
        }
    }
    bind_operands(tool, command, &operands, &mut parsed)?;
    parsed.deferred_unknown = deferred_unknown;
    Ok(parsed)
}

/// What is left of `parse` after the words are read and the document merged: the stages of spec 11.7 step 6.
fn enforce(tool: &Tool, command: &Command, parsed: &Parsed, skip_rules: bool) -> Result<(), Failure> {
    // The seeded violations of `L1.check-order` (spec 11.7 step 6): a stage answers before an earlier one.
    if tool.flag("orderDomainFirst") {
        check_domains(tool, command, parsed, false)?;
    }
    for arg in command.args.iter().filter(|arg| arg.required) {
        if !parsed.has(arg.name) {
            return Err(usage(
                "required",
                format!("{} is required", arg.field()),
                arg.field(),
            ));
        }
    }
    if let Some(word) = &parsed.deferred_unknown {
        return Err(Failure::Unknown(word.clone()));
    }
    if tool.flag("orderConstraintFirst") && !skip_rules {
        check_rules(tool, command, parsed)?;
    }
    check_domains(tool, command, parsed, tool.flag("orderLaterDomain"))?;
    if tool.flag("orderValidatorFirst") {
        check_validators(tool, command, parsed)?;
    }
    if !skip_rules {
        check_rules(tool, command, parsed)?;
    }
    check_validators(tool, command, parsed)?;
    Ok(())
}

/// Reads the document of `--input` (`-`: the document on stdin) into the arguments it gives; an argument that argv gave stays as it is.
fn merge_input(tool: &Tool, command: &Command, text: &str, parsed: &mut Parsed) -> Result<(), Failure> {
    let body = if text == "-" {
        let mut buffer = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer).unwrap_or_default();
        buffer
    } else {
        text.to_string()
    };
    let invalid = |why: String| Failure::Refuse {
        code: "INPUT_INVALID".to_string(),
        message: why,
        field: Some("--input".to_string()),
        expected: None,
        source: "parse",
    };
    let Ok(Value::Object(document)) = serde_json::from_str::<Value>(&body) else {
        return Err(invalid("--input is not a JSON object".to_string()));
    };
    for (key, value) in document {
        let Some(arg) = command.flags().find(|arg| arg.name == key && !arg.flag_only) else {
            return Err(invalid(format!("{key} is not an argument a document may give")));
        };
        let word = match &value {
            Value::String(word) => word.clone(),
            other => other.to_string(),
        };
        let value = typed(tool, arg, &word)?;
        parsed.given.entry(arg.name).or_insert_with(|| vec![value]);
    }
    Ok(())
}

/// Binds the operand words to the command's operands in order; the variadic one takes the rest.
fn bind_operands(tool: &Tool, command: &Command, words: &[String], parsed: &mut Parsed) -> Result<(), Failure> {
    let slots: Vec<&Arg> = command.operands().collect();
    for (position, word) in words.iter().enumerate() {
        let slot = slots
            .get(position)
            .copied()
            .or_else(|| slots.last().copied().filter(|slot| slot.repeatable));
        let Some(slot) = slot else {
            return Err(usage(
                "parse",
                format!("{} takes no operand {word}", command.id),
                "ARGUMENT".to_string(),
            ));
        };
        let value = typed(tool, slot, word)?;
        parsed.given.entry(slot.name).or_default().push(value);
    }
    Ok(())
}

pub fn compiled(domain: &Value) -> Option<jsonschema::Validator> {
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .should_validate_formats(false)
        .build(domain)
        .ok()
}

pub fn check_domains(tool: &Tool, command: &Command, parsed: &Parsed, reverse: bool) -> Result<(), Failure> {
    let order: Vec<&Arg> = if reverse { command.args.iter().rev().collect() } else { command.args.iter().collect() };
    for arg in order {
        let Some(domain) = &arg.domain else { continue };
        if listed(tool, "acceptDomain", command.id, &json!(arg.name)) {
            continue;
        }
        let Some(validator) = compiled(domain) else { continue };
        let strict = listed(tool, "strictBounds", command.id, &json!(arg.name));
        for value in parsed.values(arg.name) {
            let on_bound = strict
                && ["minimum", "maximum"]
                    .iter()
                    .any(|keyword| domain.get(keyword) == Some(value));
            if !validator.is_valid(value) || on_bound {
                let code = tool.text("domainCode", "USAGE");
                return Err(usage_coded(
                    &code,
                    "domain",
                    format!("{value} is outside the domain of {}", arg.field()),
                    arg.field(),
                ));
            }
        }
    }
    Ok(())
}

pub fn given(command: &Command, parsed: &Parsed, name: &str) -> bool {
    command.arg(name).is_some() && parsed.has(name)
}

pub fn field_of(command: &Command, name: &str) -> String {
    command
        .arg(name)
        .map_or_else(|| name.to_string(), Arg::field)
}

/// The argument at fault when `rule` is violated by `parsed`, or `None` when it holds.
fn violation(command: &Command, parsed: &Parsed, rule: &Rule) -> Option<&'static str> {
    let first_given = |names: &[&'static str]| names.iter().copied().find(|name| given(command, parsed, name));
    let count = rule.names.iter().filter(|name| given(command, parsed, name)).count();
    match rule.rule {
        "atLeastOne" => (count < 1).then(|| rule.names[0]),
        "exactlyOne" => match count {
            1 => None,
            0 => Some(rule.names[0]),
            _ => first_given(&rule.names),
        },
        "atMostOne" => (count > 1).then(|| first_given(&rule.names)).flatten(),
        _ => {
            let (name, values) = rule.when.as_ref()?;
            if !given(command, parsed, name) {
                return None;
            }
            if !values.is_empty() && !parsed.values(name).iter().any(|value| values.contains(value)) {
                return None;
            }
            if rule.rule == "requires" {
                rule.then.iter().copied().find(|then| !given(command, parsed, then))
            } else {
                first_given(&rule.then)
            }
        }
    }
}

pub fn check_rules(tool: &Tool, command: &Command, parsed: &Parsed) -> Result<(), Failure> {
    // Seeded: the constraints are judged from the last declared to the first.
    let mut rules: Vec<(usize, &super::spec::Rule)> = command.rules.iter().enumerate().collect();
    if tool.flag("rulesReversed") {
        rules.reverse();
    }
    for (index, rule) in rules {
        if listed(tool, "ignoreConstraint", command.id, &json!(index)) {
            continue;
        }
        let Some(name) = violation(command, parsed, rule) else { continue };
        let code = rule
            .code
            .filter(|_| !tool.flag("constraintCodeUsage"))
            .unwrap_or("USAGE");
        return Err(Failure::Refuse {
            code: code.to_string(),
            message: format!("{} breaks the rule {}", field_of(command, name), rule.rule),
            field: Some(field_of(command, name)),
            expected: None,
            source: "constraint",
        });
    }
    Ok(())
}

/// Whether `text` is `YYYY-MM-DD` and names a day of the calendar.
fn is_calendar_date(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    let [year, month, day] = parts.as_slice() else {
        return false;
    };
    let (Ok(year), Ok(month), Ok(day)) = (year.parse::<u32>(), month.parse::<u32>(), day.parse::<u32>()) else {
        return false;
    };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

pub fn check_validators(tool: &Tool, command: &Command, parsed: &Parsed) -> Result<(), Failure> {
    if tool.flag("validatorRejectsNothing") {
        return Ok(());
    }
    for validator in &command.validators {
        let Check::CalendarDate(name) = validator.check;
        let Some(text) = parsed.text(name) else { continue };
        if is_calendar_date(text) {
            continue;
        }
        let code = if tool.flag("validatorCodeUsage") {
            "USAGE"
        } else {
            validator.code
        };
        let field = tool
            .config
            .get("validatorField")
            .and_then(Value::as_str)
            .map_or_else(|| field_of(command, name), ToString::to_string);
        return Err(Failure::Refuse {
            code: code.to_string(),
            message: format!("{text} is not a day of the calendar"),
            field: Some(field),
            expected: None,
            source: "validator",
        });
    }
    Ok(())
}
