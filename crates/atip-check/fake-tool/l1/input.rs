//! `--input` of an L2 tool, as spec 11.2-11.11 order it: read and frame, validate each document against the fragment, find the
//! conflicts with argv, merge, apply `env`, validate the arguments, then execute (a batch in two phases). It is derived from the
//! model of `spec.rs`, so the documents `schema` prints and the behaviour agree by construction.
//!
//! A single document is converted to the argv that says the same (spec 11.9) and the ordinary handler of the command runs it; a
//! batch is executed item by item by [`super::run::execute_batch`]. The seeded violations are members of the configuration; each
//! names the rule it breaks where it acts.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{IsTerminal, Read};
use std::process::ExitCode;

use serde_json::{json, Map, Value};

use super::doc::Model;
use super::inputdoc::{camel, MAX_BYTES, MAX_ITEMS};
use super::parse::{check_domains, check_rules, check_validators, field_of, read_words, typed, Failure, Parsed};
use super::run::{path_words, refuse_failure};
use super::spec::{Arg, Command, Kind};
use crate::Tool;

/// The largest integer a JSON parser, JavaScript and a Rust `i64` agree on (spec 10.7.5).
const SAFE: i64 = 9_007_199_254_740_991;

/// What the preparation of a call that has `--input` came to.
pub enum Prepared {
    /// The call has no `--input`, or is no call of a command that takes one.
    Unchanged,
    /// The call is answered: the refusal has been printed.
    Done(ExitCode),
    /// One document, merged: the argv that says the same, for the ordinary handler.
    Argv(Vec<String>),
    /// A batch whose preflight passed: the command and the merged arguments of each item.
    Batch(&'static str, Vec<Parsed>),
}

/// One problem of a document.
struct Problem {
    reason: &'static str,
    pointer: String,
    message: String,
}

/// Where the value of `--input` came from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Inline,
    Stdin,
    File,
}

fn refuse(tool: &Tool, command: &str, code: &str, reason: &'static str, message: &str, field: &str, details: &[Problem]) -> ExitCode {
    let seeded_field = tool.text("inputField", "");
    let in_batch_item = field.chars().nth(1).is_some_and(|character| character.is_ascii_digit());
    let field = if !seeded_field.is_empty() && code != "INPUT_CONFLICT" && reason != "STDIN_TTY" && !in_batch_item { seeded_field.as_str() } else { field };
    // Seeded: the size of the document is refused on a pointer into it, not on `--input`.
    let too_large_field = tool.text("tooLargeField", "");
    let field = if code == "INPUT_TOO_LARGE" && !too_large_field.is_empty() { too_large_field.as_str() } else { field };
    let mut extras: Vec<(&str, Value)> = vec![("field", json!(field))];
    let reason_value = (!tool.flag("inputNoReason") && tool.is_1_1() && code.starts_with("INPUT_")).then_some(reason);
    if let Some(reason) = reason_value.filter(|reason| !reason.is_empty()) {
        extras.push(("reason", json!(reason)));
    }
    if details.len() > 1 && !tool.flag("inputNoDetails") {
        let items: Vec<Value> = details
            .iter()
            .map(|problem| json!({"code": code, "reason": problem.reason, "message": problem.message, "field": problem.pointer}))
            .collect();
        extras.push(("details", Value::Array(items)));
    }
    tool.refuse(code, message, command, &extras)
}

fn usage(tool: &Tool, command: &str, message: &str, code_knob: &str) -> ExitCode {
    // Seeded: a registered code of exit 2 that is not USAGE answers (a finer replacement of USAGE, spec 7.2.2).
    let replacement = tool.text("inputUsageCode", "USAGE");
    let code = if tool.flag(code_knob) { "INPUT_INVALID" } else { replacement.as_str() };
    let field = tool.text("inputUsageField", "--input");
    tool.refuse(code, message, command, &[("field", json!(field))])
}

/// Whether `value` is a JSON integer in the safe range, as a number or as a numeric string (spec 11.5).
/// A float that is a whole number below 2^53, as the integer it is.
#[expect(clippy::cast_possible_truncation, reason = "the caller checked that the float is whole and below 9e15")]
fn whole_of(float: f64) -> i64 {
    float as i64
}

fn integer_of(value: &Value, strings: bool) -> Option<i64> {
    match value {
        Value::Number(number) => {
            let whole = number.as_i64().or_else(|| number.as_f64().filter(|float| float.fract() == 0.0 && float.abs() <= 9.0e15).map(whole_of))?;
            (whole.abs() <= SAFE).then_some(whole)
        }
        Value::String(text) if strings => {
            let digits = text.strip_prefix('-').unwrap_or(text);
            let plain = !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) && (digits == "0" || !digits.starts_with('0'));
            plain.then(|| text.parse::<i64>().ok()).flatten().filter(|whole| whole.abs() <= SAFE)
        }
        _ => None,
    }
}

fn number_of(value: &Value, strings: bool) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64().filter(|float| float.is_finite()),
        Value::String(text) if strings => {
            let grammar = regex::Regex::new(r"^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?$").ok()?;
            grammar.is_match(text).then(|| text.parse::<f64>().ok()).flatten().filter(|float| float.is_finite())
        }
        _ => None,
    }
}

/// Whether `value` is a value of the type of `arg` (not null, not repeated: one element).
fn element_ok(tool: &Tool, arg: &Arg, value: &Value) -> bool {
    let strings = !tool.flag("inputNoNumericStrings");
    match arg.value_type {
        "integer" => integer_of(value, strings).is_some() || (tool.flag("inputRoundsBigIntegers") && number_of(value, strings).is_some()),
        "number" => number_of(value, strings).is_some(),
        "enum" => value.as_str().is_some_and(|text| arg.possible.contains(&text)),
        "switch" => value.is_boolean(),
        "count" => value.as_u64().is_some(),
        "json" => !value.is_null(),
        _ => value.is_string(),
    }
}

/// The value the document gives, as `typed` would give it from argv: numbers normalised, an absent switch or count left out.
fn normalised(arg: &Arg, value: &Value) -> Vec<Value> {
    let one = |element: &Value| -> Option<Value> {
        Some(match arg.value_type {
            "integer" => json!(integer_of(element, true)?),
            "number" => json!(number_of(element, true)?),
            "switch" => return element.as_bool().filter(|flag| *flag).map(|_| json!(true)),
            "count" => return element.as_u64().filter(|count| *count > 0).map(|count| json!(count)),
            _ => element.clone(),
        })
    };
    match value {
        Value::Array(items) if arg.repeatable && arg.value_type != "count" => items.iter().filter_map(one).collect(),
        other => one(other).into_iter().collect(),
    }
}

/// The problems of one document against the fragment (spec 11.7 step 2): known keys, flag-only keys absent, `null`, types.
fn fragment_problems(tool: &Tool, command: &Command, document: &Map<String, Value>, prefix: &str) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut found = |reason: &'static str, pointer: String, message: String| problems.push(Problem { reason, pointer: format!("{prefix}{pointer}"), message });
    let operand_slots: Vec<&Arg> = command.operands().collect();
    for (key, value) in document {
        if key == "operands" {
            if tool.lists("inputAccepts", "operands") {
                continue;
            }
            let Some(items) = value.as_array() else {
                found("INVALID_VALUE", "/operands".to_string(), "operands must be an array".to_string());
                continue;
            };
            if operand_slots.is_empty() {
                if !tool.lists("inputAccepts", "unknown-key") {
                    found("UNKNOWN_KEY", "/operands".to_string(), format!("{} takes no operands", command.id));
                }
                continue;
            }
            if items.is_empty() && !tool.lists("inputAccepts", "empty-operands") {
                found("INVALID_VALUE", "/operands".to_string(), "operands is empty: omit the key".to_string());
            }
            let variadic = operand_slots.last().is_some_and(|slot| slot.repeatable);
            if items.len() > operand_slots.len() && !variadic {
                found("INVALID_VALUE", "/operands".to_string(), format!("{} takes {} operands", command.id, operand_slots.len()));
            }
            for (index, item) in items.iter().enumerate() {
                let slot = operand_slots.get(index).copied().or_else(|| operand_slots.last().copied().filter(|slot| slot.repeatable));
                if item.is_null() && !tool.lists("inputAccepts", "null") || slot.is_some_and(|slot| !item.is_null() && !element_ok(tool, slot, item)) {
                    found("INVALID_VALUE", format!("/operands/{index}"), format!("operands[{index}] is not a value of {}", slot.map_or("an operand", |slot| slot.name)));
                }
            }
            continue;
        }
        let Some(arg) = command.flags().find(|arg| camel(arg.name) == *key) else {
            let situation = if key == "commit" { "commit" } else { "unknown-key" };
            if !tool.lists("inputAccepts", situation) && !tool.lists("inputAccepts", "unknown-key") {
                found("UNKNOWN_KEY", format!("/{key}"), format!("{key} is not an option of {}", command.id));
            }
            continue;
        };
        if arg.flag_only {
            if !tool.lists("inputAccepts", key) {
                found("FLAG_ONLY_KEY", format!("/{key}"), format!("{key} is flag-only and cannot be set by a document"));
            }
            continue;
        }
        if value.is_null() {
            if !tool.lists("inputAccepts", "null") {
                found("INVALID_VALUE", format!("/{key}"), format!("{key} is null"));
            }
            continue;
        }
        let ok = match value {
            Value::Array(items) if arg.repeatable && arg.value_type != "count" => {
                !items.is_empty() && items.iter().all(|item| !item.is_null() && element_ok(tool, arg, item))
            }
            other => !(arg.repeatable && arg.value_type != "count") && element_ok(tool, arg, other),
        };
        if !ok {
            found("INVALID_VALUE", format!("/{key}"), format!("{key} is not a value of {}", arg.name));
        }
    }
    problems
}

/// The values a document gives, by argument name.
fn document_given(command: &Command, document: &Map<String, Value>) -> BTreeMap<&'static str, Vec<Value>> {
    let mut given: BTreeMap<&'static str, Vec<Value>> = BTreeMap::new();
    let slots: Vec<&Arg> = command.operands().collect();
    if let Some(items) = document.get("operands").and_then(Value::as_array) {
        for (index, item) in items.iter().enumerate() {
            let Some(slot) = slots.get(index).copied().or_else(|| slots.last().copied().filter(|slot| slot.repeatable)) else { continue };
            given.entry(slot.name).or_default().extend(normalised(slot, item));
        }
    }
    for (key, value) in document {
        if let Some(arg) = command.flags().find(|arg| camel(arg.name) == *key && !arg.flag_only) {
            let values = normalised(arg, value);
            if !values.is_empty() {
                given.insert(arg.name, values);
            }
        }
    }
    given
}

/// Whether the two values an argument got, from argv and from a document, are one value (spec 11.7 step 3).
fn same_value(tool: &Tool, arg: &Arg, argv: &[Value], document: &[Value], raw: &Value) -> bool {
    if tool.flag("conflictJsonObjects") && arg.value_type == "json" && argv.iter().chain(document).any(|value| value.as_object().is_some_and(|members| members.len() > 1)) {
        return false;
    }
    if tool.flag("conflictFloatSpelling") && matches!(arg.value_type, "number" | "integer") {
        let spelt_other_way = match raw {
            Value::Number(number) => number.is_f64(),
            Value::String(text) => text.starts_with('-') && text.trim_start_matches('-') == "0",
            _ => false,
        };
        if spelt_other_way {
            return false;
        }
    }
    argv == document
}

fn word_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Reads the value of `--input`: the document text, and where it came from, or the refusal.
fn read_source(tool: &Tool, command: &Command, value: &str) -> Result<(String, Source), ExitCode> {
    let refuse_input = |code: &str, reason: &'static str, message: &str| refuse(tool, command.id, code, reason, message, "--input", &[]);
    if value.is_empty() {
        return Err(refuse_input("INPUT_INVALID", "EMPTY", "--input is empty"));
    }
    let too_large = |size: usize| size > MAX_BYTES && !tool.lists("inputAccepts", "too-large");
    let trimmed = value.trim_start();
    let (text, source) = if trimmed.starts_with('{') || trimmed.starts_with('[') {
        (value.to_string(), Source::Inline)
    } else if value == "-" {
        if std::io::stdin().is_terminal() {
            let code = tool.text("inputTtyCode", "INPUT_INVALID");
            let field = tool.text("inputTtyField", "--input");
            return Err(refuse(tool, command.id, &code, "STDIN_TTY", "--input - with a terminal on stdin", &field, &[]));
        }
        if tool.flag("stdinPreviewRefused") && command.id == "purge" {
            return Err(refuse_input("INPUT_INVALID", "NOT_JSON", "--input is not JSON"));
        }
        // What a handler that writes the argv of a replay needs to know: the arguments came from stdin.
        std::env::set_var("FAKE_INPUT_FROM_STDIN", "1");
        let mut buffer = Vec::new();
        let _ = std::io::stdin().take(MAX_BYTES as u64 * 4).read_to_end(&mut buffer);
        (String::from_utf8_lossy(&buffer).into_owned(), Source::Stdin)
    } else {
        let path = value.strip_prefix("./").map_or(value, |rest| rest);
        match std::fs::read_to_string(path) {
            Ok(text) => (text, Source::File),
            Err(_) if tool.lists("inputAccepts", "missing-file") => ("{}".to_string(), Source::File),
            Err(_) => return Err(refuse_input("INPUT_INVALID", "UNREADABLE", &format!("{value} cannot be read"))),
        }
    };
    if too_large(text.len()) {
        return Err(refuse_input("INPUT_TOO_LARGE", "TOO_MANY_BYTES", "--input is larger than the command accepts"));
    }
    if text.trim().is_empty() {
        return Err(refuse_input("INPUT_INVALID", "EMPTY", "--input is empty"));
    }
    Ok((text, source))
}

/// Frames the text: one document, or a batch (an array; NDJSON lines of a file or of stdin).
fn frame(tool: &Tool, command: &Command, text: &str, source: Source) -> Result<(Vec<Value>, bool), ExitCode> {
    let refuse_input = |reason: &'static str, message: &str| refuse(tool, command.id, "INPUT_INVALID", reason, message, "--input", &[]);
    let batch_allowed = command.input.as_ref().and_then(|input| input.get("batch")).and_then(Value::as_bool).unwrap_or(false);
    let (documents, batch) = match serde_json::from_str::<Value>(text) {
        Ok(Value::Array(items)) => (items, true),
        Ok(other) => (vec![other], false),
        Err(_) if source != Source::Inline => {
            let lines: Result<Vec<Value>, _> = text.lines().filter(|line| !line.trim().is_empty()).map(serde_json::from_str::<Value>).collect();
            match lines {
                Ok(items) if !items.is_empty() => (items, true),
                _ => return Err(refuse_input("NOT_JSON", "--input is not JSON")),
            }
        }
        Err(_) => return Err(refuse_input("NOT_JSON", "--input is not JSON")),
    };
    if batch && !batch_allowed && !tool.lists("inputAccepts", "batch-unsupported") {
        return Err(refuse_input("BATCH_UNSUPPORTED", &format!("{} takes no batch", command.id)));
    }
    if batch && documents.is_empty() {
        return Err(refuse_input("INVALID_VALUE", "a batch holds at least one document"));
    }
    if batch && documents.len() > MAX_ITEMS {
        return Err(refuse(tool, command.id, "INPUT_TOO_LARGE", "TOO_MANY_ITEMS", "the batch holds more documents than the command accepts", "--input", &[]));
    }
    if let Some(index) = documents.iter().position(|document| !document.is_object()) {
        let field = if batch { format!("/{index}") } else { "--input".to_string() };
        return Err(refuse(tool, command.id, "INPUT_INVALID", "NOT_OBJECT", "a document is not a JSON object", &field, &[]));
    }
    Ok((documents, batch))
}

/// Translates the field of a refusal of step 6 into the pointer of the document (spec 11.8) when the value came from it.
fn translate(tool: &Tool, command: &Command, failure: Failure, from_document: &BTreeSet<&'static str>, prefix: &str) -> Failure {
    if tool.flag("inputFieldsAsArgv") && prefix.is_empty() && !matches!(&failure, Failure::Refuse { source: "required", .. }) {
        return failure;
    }
    let Failure::Refuse { code, message, field: Some(field), expected, source } = failure else { return failure };
    let arg = command.args.iter().find(|arg| arg.field() == field);
    let pointer = arg.and_then(|arg| match arg.kind {
        Kind::Operand => (source == "required" || from_document.contains(arg.name)).then(|| format!("{prefix}/operands")),
        Kind::Option => (source == "required" || from_document.contains(arg.name)).then(|| format!("{prefix}/{}", camel(arg.name))),
    });
    Failure::Refuse { code, message, field: Some(pointer.unwrap_or(field)), expected, source }
}

/// What `prepare` needs of the call: the command, the argv without its `--input` words, and the value.
struct Call<'a> {
    command: &'a Command,
    rest: Vec<String>,
    value: String,
}

/// Splits `--input <value>` (or `--input=<value>`) out of `args`; a second one, or one with no value, is refused as a malformed command line.
fn take_input(tool: &Tool, command: &Command, args: &[String]) -> Result<Option<(Vec<String>, String)>, ExitCode> {
    let mut rest: Vec<String> = Vec::new();
    let mut values: Vec<String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let word = &args[index];
        index += 1;
        if word == "--" {
            rest.extend(args[index - 1..].iter().cloned());
            break;
        }
        if let Some(value) = word.strip_prefix("--input=") {
            values.push(value.to_string());
        } else if word == "--input" {
            match args.get(index) {
                Some(value) if value == "-" || value.is_empty() || !value.starts_with('-') => {
                    values.push(value.clone());
                    index += 1;
                }
                _ if tool.flag("inputUsageAccepted") => {}
                _ => return Err(usage(tool, command.id, "--input needs a value", "inputLastWordInvalid")),
            }
        } else {
            rest.push(word.clone());
        }
    }
    if values.len() > 1 && !tool.flag("inputUsageAccepted") {
        return Err(usage(tool, command.id, "--input was given twice", "inputTwiceInvalid"));
    }
    Ok(values.pop().map(|value| (rest, value)))
}

/// Prepares a call whose command takes `--input`: everything of spec 11.7 before execution.
pub fn prepare(tool: &Tool, args: &[String]) -> Prepared {
    let words = path_words(args);
    if args.iter().take_while(|arg| arg.as_str() != "--").any(|arg| arg == "--help" || arg == "-h") {
        return Prepared::Unchanged;
    }
    let model = Model::of(tool);
    let Some(command) = (1..=words.len()).rev().find_map(|length| model.command(&words[..length].join(" "))).filter(|command| command.input.is_some()) else {
        return Prepared::Unchanged;
    };
    let (rest, value) = match take_input(tool, command, args) {
        Ok(Some(taken)) => taken,
        Ok(None) => return Prepared::Unchanged,
        Err(code) => return Prepared::Done(code),
    };
    let call = Call { command, rest, value };
    match prepare_call(tool, &call) {
        Ok(prepared) => prepared,
        Err(code) => Prepared::Done(code),
    }
}

fn prepare_call(tool: &Tool, call: &Call<'_>) -> Result<Prepared, ExitCode> {
    let command = call.command;
    let (text, source) = read_source(tool, command, &call.value)?;
    let (documents, batch) = frame(tool, command, &text, source)?;
    // Step 2: every document against the fragment; every problem is reported, the first as the error.
    let mut problems: Vec<Problem> = Vec::new();
    for (index, document) in documents.iter().enumerate() {
        let prefix = if batch { format!("/{index}") } else { String::new() };
        if let Some(object) = document.as_object() {
            problems.extend(fragment_problems(tool, command, object, &prefix));
        }
    }
    if let Some(first) = problems.first() {
        let field = if first.pointer.is_empty() { "--input" } else { first.pointer.as_str() };
        let message = if problems.len() > 1 { format!("the --input document has {} problems", problems.len()) } else { first.message.clone() };
        return Err(refuse(tool, command.id, "INPUT_INVALID", first.reason, &message, field, &problems));
    }
    // Step 3: argv read without enforcing anything, and the conflicts with each document.
    let argv = match read_words(tool, command, &call.rest) {
        Ok(parsed) => parsed,
        Err(failure) => return Err(refuse_failure(tool, command.id, failure)),
    };
    let mut merged: Vec<(Parsed, BTreeSet<&'static str>)> = Vec::new();
    for (index, document) in documents.iter().enumerate() {
        let prefix = if batch { format!("/{index}") } else { String::new() };
        let Some(object) = document.as_object() else { continue };
        let given = document_given(command, object);
        let mut from_document: BTreeSet<&'static str> = BTreeSet::new();
        let mut explicit = Parsed { deferred_unknown: argv.deferred_unknown.clone(), ..Parsed::default() };
        for (name, values) in &argv.given {
            explicit.given.insert(name, values.clone());
        }
        for (name, values) in given {
            let Some(arg) = command.arg(name) else { continue };
            match explicit.given.get(name) {
                Some(on_argv) if !on_argv.is_empty() => {
                    let raw = match arg.kind {
                        Kind::Operand => object.get("operands").cloned().unwrap_or(Value::Null),
                        Kind::Option => object.get(&camel(name)).cloned().unwrap_or(Value::Null),
                    };
                    let raw_one = match &raw {
                        Value::Array(items) if items.len() == 1 => items[0].clone(),
                        other => other.clone(),
                    };
                    if !tool.flag("inputNoConflict") && !same_value(tool, arg, on_argv, &values, &raw_one) {
                        let pointer = match arg.kind {
                            Kind::Operand => format!("{prefix}/operands"),
                            Kind::Option => format!("{prefix}/{}", camel(name)),
                        };
                        let field = tool.text("inputConflictField", &pointer);
                        return Err(refuse(tool, command.id, "INPUT_CONFLICT", "", &format!("{} differs between argv and the document", arg.field()), &field, &[]));
                    }
                }
                _ => {
                    explicit.given.insert(name, values);
                    from_document.insert(name);
                }
            }
        }
        merged.push((explicit, from_document));
    }
    if tool.flag("envBeforeMerge") {
        for arg in command.flags().filter(|arg| arg.env.is_some()) {
            if let Some(text) = arg.env.and_then(|variable| std::env::var(variable).ok()) {
                let failure = typed(tool, arg, &text).err().or_else(|| {
                    let mut probe = Parsed::default();
                    probe.given.insert(arg.name, vec![typed(tool, arg, &text).unwrap_or(Value::Null)]);
                    check_domains(tool, command, &probe, false).err()
                });
                if let Some(failure) = failure {
                    return Err(refuse_failure(tool, command.id, failure));
                }
            }
        }
    }
    // Steps 5 and 6, for each document: `env` for what is absent, then requirements, domains, constraints, validators.
    let mut effective_items: Vec<Parsed> = Vec::new();
    for (index, (explicit, from_document)) in merged.iter().enumerate() {
        let prefix = if batch && !tool.flag("batchNoPrefix") { format!("/{index}") } else { String::new() };
        let lazily = batch && tool.flag("batchLatePreflight") && index + 1 == merged.len();
        match enforce_item(tool, command, explicit, from_document, &prefix, batch) {
            Ok(effective) => effective_items.push(effective),
            Err(failure) if lazily => {
                let _ = failure;
                effective_items.push(explicit_clone(explicit));
                return Ok(Prepared::Batch(command.id, mark_late(effective_items, index)));
            }
            Err(failure) => return Err(refuse_failure(tool, command.id, translate(tool, command, failure, from_document, &prefix))),
        }
    }
    if batch {
        return Ok(Prepared::Batch(command.id, effective_items));
    }
    let (Some((explicit, from_document)), Some(effective)) = (merged.first(), effective_items.first()) else {
        return Ok(Prepared::Unchanged);
    };
    Ok(Prepared::Argv(synthesize(tool, command, &call.rest, explicit, effective, from_document)))
}

fn explicit_clone(parsed: &Parsed) -> Parsed {
    let mut clone = Parsed { deferred_unknown: parsed.deferred_unknown.clone(), ..Parsed::default() };
    for (name, values) in &parsed.given {
        clone.given.insert(name, values.clone());
    }
    clone
}

/// A batch whose last item failed preflight late is marked by a `__late__` entry in its last item: the executor stops there.
fn mark_late(mut items: Vec<Parsed>, index: usize) -> Vec<Parsed> {
    if let Some(item) = items.get_mut(index) {
        item.given.insert("__late__", vec![json!(true)]);
    }
    items
}

/// Steps 5 and 6 for one document: what `env` adds, then the stages in the order of the contract. The values the item executes with.
fn enforce_item(tool: &Tool, command: &Command, explicit: &Parsed, from_document: &BTreeSet<&'static str>, prefix: &str, batch: bool) -> Result<Parsed, Failure> {
    let _ = prefix;
    let mut effective = explicit_clone(explicit);
    for arg in command.flags().filter(|arg| arg.env.is_some() && !arg.flag_only) {
        if explicit.has(arg.name) {
            continue;
        }
        if let Some(text) = arg.env.and_then(|variable| std::env::var(variable).ok()) {
            effective.given.insert(arg.name, vec![typed(tool, arg, &text)?]);
        }
    }
    // The seeded violation: a document that carries every argument, with none on argv, is refused as missing them.
    let complete = !batch && command.args.iter().filter(|arg| arg.required).all(|arg| effective.has(arg.name));
    let carried = command.args.iter().filter(|arg| !arg.flag_only && from_document.contains(arg.name)).count();
    let argv_gives_nothing = complete && carried >= 2 && std::env::var_os("FAKE_INPUT_FROM_STDIN").is_none() && !command.args.iter().any(|arg| !arg.flag_only && explicit.has(arg.name) && !from_document.contains(arg.name));
    let requirements = if tool.flag("inputRequiresArgv") && argv_gives_nothing { &Parsed::default() } else { &effective };
    for arg in command.args.iter().filter(|arg| arg.required) {
        if !requirements.has(arg.name) {
            let message = format!("{} is required", arg.field());
            let code = if tool.flag("inputRequiredInvalid") { "INPUT_INVALID" } else { "USAGE" };
            let source = if tool.flag("inputRequiredFieldAsArgv") { "required-as-argv" } else { "required" };
            return Err(Failure::Refuse { code: code.to_string(), message, field: Some(field_of(command, arg.name)), expected: None, source });
        }
    }
    check_domains(tool, command, &effective, false)?;
    if let Err(Failure::Refuse { code, message, field, expected, source }) = check_rules(tool, command, explicit) {
        let code = tool.text("inputConstraintCode", &code);
        return Err(Failure::Refuse { code, message, field, expected, source });
    }
    check_validators(tool, command, &effective)?;
    Ok(effective)
}

/// The argv that says what the merged document and argv say (spec 11.9): options as `--name=value`, operands after one `--`.
fn synthesize(tool: &Tool, command: &Command, rest: &[String], explicit: &Parsed, effective: &Parsed, from_document: &BTreeSet<&'static str>) -> Vec<String> {
    let mut before: Vec<String> = Vec::new();
    let mut after: Vec<String> = Vec::new();
    match rest.iter().position(|word| word == "--") {
        Some(at) => {
            before.extend(rest[..at].iter().cloned());
            after.extend(rest[at + 1..].iter().cloned());
        }
        None => before.extend(rest.iter().cloned()),
    }
    let mut operands: Vec<String> = Vec::new();
    for arg in &command.args {
        let values: &[Value] = if arg.kind == Kind::Operand {
            if !from_document.contains(arg.name) {
                continue;
            }
            explicit.values(arg.name)
        } else if from_document.contains(arg.name) {
            explicit.values(arg.name)
        } else if !explicit.has(arg.name) && effective.has(arg.name) {
            effective.values(arg.name)
        } else {
            continue;
        };
        match arg.kind {
            Kind::Operand => operands.extend(values.iter().map(word_of)),
            Kind::Option => match arg.value_type {
                "switch" => before.push(format!("--{}", arg.name)),
                "count" => {
                    for _ in 0..values.first().and_then(Value::as_u64).unwrap_or(0) {
                        before.push(format!("--{}", arg.name));
                    }
                }
                "json" => {
                    for value in values {
                        before.push(format!("--{}={}", arg.name, value));
                    }
                }
                _ => {
                    for value in values {
                        if tool.flag("dashValuesAsOptions") {
                            before.push(format!("--{}", arg.name));
                            before.push(word_of(value));
                        } else {
                            before.push(format!("--{}={}", arg.name, word_of(value)));
                        }
                    }
                }
            },
        }
    }
    let mut words = before;
    if !after.is_empty() || !operands.is_empty() {
        words.push("--".to_string());
        words.extend(after);
        words.extend(operands);
    }
    words
}
