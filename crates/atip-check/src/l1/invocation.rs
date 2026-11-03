//! A documented invocation read against a command document (spec 9.12, 10.13 rule 12, 11.5-11.7).
//!
//! OWNER: the invocation agent. The signatures below are the contract the checks are written against.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

use jsonschema::Validator;
use regex::Regex;
use serde_json::Value;

use super::model::{camel, Arg, CommandDoc};
use crate::schema::schemas;

/// The explicitly given arguments of one invocation (spec 11.7 step 4).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Args {
    /// The operand values in order. A value is the argv word as a string, except that an operand of valueType `json` is parsed.
    pub operands: Vec<Value>,
    /// The given options by name: `true` for a switch, the number of occurrences for a count, an array of values for a repeatable option, the one value otherwise.
    /// A value is the argv word as a string (an integer is `"5"`, not `5`), except that a `json` value is parsed.
    pub options: BTreeMap<String, Value>,
}

/// One word of an invocation after the command path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A free operand, or any word after `--`.
    Operand(String),
    /// One occurrence of an option: `--name`, `--name=value` or `--name value` (and a declared short form), by the option's long `name`. `value` is `None` for a switch or a count.
    Option { name: String, value: Option<String> },
}

/// An invocation split into the words before the path, the path, and the items after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// Words before the command path (tool-wide options such as `--repo .`), verbatim.
    pub before: Vec<String>,
    /// The command path words.
    pub path: Vec<String>,
    /// Options and operands in argv order.
    pub items: Vec<Item>,
}

/// The most an integer may be, in magnitude: 2^53 - 1 (spec 10.7.5).
const MAX_SAFE: f64 = 9_007_199_254_740_991.0;

/// A compiled regular expression, built on first use.
fn cached(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| {
        Regex::new(pattern)
            .unwrap_or_else(|reason| panic!("the pattern {pattern} is fixed: {reason}"))
    })
}

/// A placeholder or an ellipsis in a documented line (spec 10.5 `examples`, 6.8): `<UPPER_CASE>`, `...` or `…`.
fn placeholder() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    cached(&CELL, r"<[A-Z][A-Z0-9_]*>|\.\.\.|…")
}

/// The JSON integer literal of spec 11.5.
fn integer_form() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    cached(&CELL, r"^-?(0|[1-9][0-9]*)$")
}

/// The JSON number grammar of RFC 8259 section 6.
fn number_form() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    cached(&CELL, r"^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?$")
}

/// Whether `word` is a plain word of spec 9.12: non-empty and not starting with `-`, or exactly `-`.
fn plain(word: &str) -> bool {
    word == "-" || (!word.is_empty() && !word.starts_with('-'))
}

/// Splits a documented command line into argv words (approaches/checker.md 7.1): whitespace separates; `'…'` and `"…"` quote
/// (a backslash escapes `"` and `\` inside double quotes); a backslash outside quotes escapes the next character and a trailing
/// one joins the next line. An unquoted `|`, `>`, `<` that is not part of a `<PLACEHOLDER>`, `;`, `&`, `$(` or a backtick is an error.
pub fn split_line(line: &str) -> Result<Vec<String>, String> {
    let chars: Vec<char> = line.chars().collect();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else if c == '\\' && q == '"' {
                match chars.get(i) {
                    Some('\n') => i += 1,
                    Some(&next) if matches!(next, '"' | '\\' | '$' | '`') => {
                        word.push(next);
                        i += 1;
                    }
                    _ => word.push('\\'),
                }
            } else {
                word.push(c);
            }
            continue;
        }
        match c {
            '\'' | '"' => {
                quote = Some(c);
                started = true;
            }
            '\\' => match chars.get(i) {
                Some('\n') => i += 1,
                Some('\r') if chars.get(i + 1) == Some(&'\n') => i += 2,
                Some(&next) => {
                    word.push(next);
                    i += 1;
                    started = true;
                }
                None => {}
            },
            c if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                }
                started = false;
            }
            '|' | ';' | '&' | '>' | '`' => {
                return Err(format!("unsupported shell syntax: an unquoted `{c}`"));
            }
            '$' if chars.get(i) == Some(&'(') => {
                return Err("unsupported shell syntax: an unquoted `$(`".to_string());
            }
            '<' => {
                let mut end = i;
                if chars.get(end).is_some_and(char::is_ascii_uppercase) {
                    while chars
                        .get(end)
                        .is_some_and(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
                    {
                        end += 1;
                    }
                }
                if end > i && chars.get(end) == Some(&'>') {
                    word.extend(&chars[i - 1..=end]);
                    i = end + 1;
                    started = true;
                } else {
                    return Err("unsupported shell syntax: an unquoted `<`".to_string());
                }
            }
            _ => {
                word.push(c);
                started = true;
            }
        }
    }
    if let Some(q) = quote {
        return Err(format!("unclosed {q} quote"));
    }
    if started {
        words.push(word);
    }
    Ok(words)
}

/// Reads `words` (an argv without the tool name: tool-wide options, the command path, then the arguments) against `doc` by the
/// grammar of spec 9.12. `Err` names the first word the grammar or the document does not allow.
pub fn parse(doc: &CommandDoc, words: &[String]) -> Result<Invocation, String> {
    let path_len = doc.path.len();
    if path_len == 0 {
        return Err("the command document has no path".to_string());
    }
    let path_at = |at: usize| words.get(at..at + path_len) == Some(doc.path.as_slice());
    let mut before = Vec::new();
    let mut at = 0;
    while !path_at(at) {
        let Some(word) = words.get(at) else {
            return Err(format!(
                "the command path {} is not in the words",
                doc.command
            ));
        };
        if !word.starts_with('-') {
            return Err(format!(
                "{word} is not a tool-wide option and the command path {} does not start here",
                doc.command
            ));
        }
        before.push(word.clone());
        at += 1;
        if let Some(value) = words.get(at) {
            if !value.starts_with('-') && *value != doc.path[0] {
                before.push(value.clone());
                at += 1;
            }
        }
    }
    let rest = &words[at + path_len..];
    let mut items = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    let mut j = 0;
    while j < rest.len() {
        let word = &rest[j];
        j += 1;
        if word == "--" {
            items.extend(rest[j..].iter().cloned().map(Item::Operand));
            break;
        }
        if plain(word) {
            items.push(Item::Operand(word.clone()));
            continue;
        }
        let mut joined: Option<&str> = None;
        let option = if let Some(long) = word.strip_prefix("--") {
            let (name, value) = match long.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (long, None),
            };
            joined = value;
            doc.option(name)
        } else {
            let mut letters = word.chars();
            match (letters.next(), letters.next(), letters.next()) {
                (Some('-'), Some(letter), None) if letter.is_ascii_alphabetic() => doc
                    .options
                    .iter()
                    .find(|option| option.short == Some(letter)),
                _ => None,
            }
        };
        let Some(option) = option else {
            return Err(if word.is_empty() {
                "an empty word (an empty value is written --name=)".to_string()
            } else {
                format!("unknown option {word}")
            });
        };
        if option.is_flag() {
            if joined.is_some() {
                return Err(format!("--{} takes no value", option.name));
            }
            if option.value_type == "switch" && seen.contains(&option.name.as_str()) {
                return Err(format!("--{} given twice", option.name));
            }
            seen.push(&option.name);
            items.push(Item::Option {
                name: option.name.clone(),
                value: None,
            });
            continue;
        }
        let value = match joined {
            Some(value) => value.to_string(),
            None => match rest.get(j).filter(|next| plain(next)) {
                Some(next) => {
                    j += 1;
                    next.clone()
                }
                None => {
                    return Err(format!(
                        "--{} needs a value (a value starting with \"-\" is written --{}=…)",
                        option.name, option.name
                    ));
                }
            },
        };
        if !option.repeatable && seen.contains(&option.name.as_str()) {
            return Err(format!("--{} given twice", option.name));
        }
        seen.push(&option.name);
        items.push(Item::Option {
            name: option.name.clone(),
            value: Some(value),
        });
    }
    Ok(Invocation {
        before,
        path: doc.path.clone(),
        items,
    })
}

/// The argv of `invocation`: `before`, the path, every option as `--name=value` (`--name` for a flag, once per occurrence), then the
/// operands (after one `--` when any of them starts with `-`).
///
/// Items keep their order; the `--` is written before the first operand that is not a plain word (empty, or starting with `-`
/// other than exactly `-`), and the options that come after that operand in `items` are written before the `--`, because
/// everything after it is an operand. (What `parse` produced has only operands after such a word.)
pub fn render(invocation: &Invocation) -> Vec<String> {
    let mut argv = invocation.before.clone();
    argv.extend(invocation.path.iter().cloned());
    let ends_at = invocation
        .items
        .iter()
        .position(|item| matches!(item, Item::Operand(value) if !plain(value)));
    let word = |item: &Item| match item {
        Item::Operand(value) => value.clone(),
        Item::Option { name, value: Some(value) } => format!("--{name}={value}"),
        Item::Option { name, value: None } => format!("--{name}"),
    };
    match ends_at {
        None => argv.extend(invocation.items.iter().map(word)),
        Some(at) => {
            argv.extend(invocation.items[..at].iter().map(word));
            argv.extend(invocation.items[at..].iter().filter(|item| matches!(item, Item::Option { .. })).map(word));
            argv.push("--".to_string());
            argv.extend(invocation.items[at..].iter().filter(|item| matches!(item, Item::Operand(_))).map(word));
        }
    }
    argv
}

/// The operand of `doc` that the `index`-th operand word stands for: the last one takes the rest when it is variadic.
fn operand_arg(doc: &CommandDoc, index: usize) -> Option<&Arg> {
    doc.operands
        .get(index)
        .or_else(|| doc.operands.last().filter(|operand| operand.variadic))
}

/// The word `text` as a typed value of `arg`: a `json` value parsed, anything else the string itself.
fn typed(arg: Option<&Arg>, text: &str) -> Result<Value, String> {
    if arg.is_some_and(|arg| arg.value_type == "json") {
        serde_json::from_str(text).map_err(|_parse| "is not JSON".to_string())
    } else {
        Ok(Value::String(text.to_string()))
    }
}

/// The typed arguments of `invocation`.
pub fn args(doc: &CommandDoc, invocation: &Invocation) -> Args {
    let mut out = Args::default();
    for item in &invocation.items {
        match item {
            Item::Operand(text) => {
                let value = typed(operand_arg(doc, out.operands.len()), text)
                    .unwrap_or_else(|_| Value::String(text.clone()));
                out.operands.push(value);
            }
            Item::Option { name, value } => {
                let option = doc.option(name);
                let kind = option.map_or("string", |option| option.value_type.as_str());
                if kind == "switch" {
                    out.options.insert(name.clone(), Value::Bool(true));
                } else if kind == "count" {
                    let seen = out.options.get(name).and_then(Value::as_u64).unwrap_or(0);
                    out.options.insert(name.clone(), Value::from(seen + 1));
                } else if let Some(text) = value {
                    let value = typed(option, text).unwrap_or_else(|_| Value::String(text.clone()));
                    if option.is_some_and(|option| option.repeatable) {
                        let entry = out
                            .options
                            .entry(name.clone())
                            .or_insert_with(|| Value::Array(Vec::new()));
                        if let Value::Array(values) = entry {
                            values.push(value);
                        }
                    } else {
                        out.options.insert(name.clone(), value);
                    }
                }
            }
        }
    }
    out
}

/// A string value of an integer, count or number argument as the number it denotes (spec 11.5); anything else is returned as it is.
/// An integer outside the safe range stays a string, so that it is refused and never rounded (spec 10.7.5).
pub fn normalise(value_type: &str, value: &Value) -> Value {
    let Value::String(text) = value else {
        return value.clone();
    };
    if (value_type == "integer" || value_type == "count") && integer_form().is_match(text) {
        if let Ok(number) = text.parse::<i64>() {
            if number.unsigned_abs() <= 9_007_199_254_740_991 {
                return Value::from(number);
            }
        }
    } else if value_type == "number" && number_form().is_match(text) {
        if integer_form().is_match(text) {
            if let Ok(number) = text.parse::<i64>() {
                if number.unsigned_abs() <= 9_007_199_254_740_991 {
                    return Value::from(number);
                }
            }
        }
        if let Some(number) = text
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .and_then(serde_json::Number::from_f64)
        {
            return Value::Number(number);
        }
    }
    value.clone()
}

/// The value of a JSON number that is an integer in the safe range (`5` and `5.0` both), else `None`.
fn safe_integer(value: &Value) -> Option<f64> {
    let number = match value.as_i64() {
        Some(integer) => integer as f64,
        None => value.as_f64().filter(|number| number.fract() == 0.0)?,
    };
    (number.abs() <= MAX_SAFE).then_some(number)
}

/// Whether `value` (already normalised) is valid for the valueType of `arg` (spec 10.7.2).
fn valid_value(arg: &Arg, value: &Value) -> bool {
    match arg.value_type.as_str() {
        "integer" => safe_integer(value).is_some(),
        "number" => value.as_f64().is_some_and(f64::is_finite),
        "enum" => value
            .as_str()
            .is_some_and(|text| arg.possible_values.iter().any(|allowed| allowed == text)),
        "string" | "path" => value.is_string(),
        "json" => !value.is_null(),
        "switch" => value.is_boolean(),
        "count" => safe_integer(value).is_some_and(|number| number >= 0.0),
        _ => false,
    }
}

/// A domain's compiled validator (or why it does not compile), by the domain's JSON text.
type DomainCache = Mutex<HashMap<String, Result<Arc<Validator>, String>>>;

/// The validators of the arguments' domains, compiled once per domain.
/// `value` without its `pattern` keywords, at any depth; with `only_bad`, only those that are not of the form of spec 10.7.4 or that Rust refuses.
fn strip_patterns(value: &mut Value, only_bad: bool) {
    match value {
        Value::Object(members) => {
            let bad = match members.get("pattern") {
                Some(Value::String(pattern)) => {
                    !only_bad || super::regex::form_problem(pattern).is_some() || super::regex::rust_problem(pattern).is_some()
                }
                Some(_) => true,
                None => false,
            };
            if bad {
                members.remove("pattern");
            }
            for member in members.values_mut() {
                strip_patterns(member, only_bad);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_patterns(item, only_bad);
            }
        }
        _ => {}
    }
}

/// Why `domain` does not compile as a JSON Schema (its `pattern` keywords aside: `L1.patterns`' to report); `None` when it does.
pub fn domain_compile_error(domain: &Value) -> Option<String> {
    domain_validator(domain).err()
}

fn domain_validator(domain: &Value) -> Result<Arc<Validator>, String> {
    static CACHE: OnceLock<DomainCache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = domain.to_string();
    let mut guard = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard
        .entry(key)
        .or_insert_with(|| {
            // A domain whose `pattern` does not compile is `L1.patterns`' to report (and its other rules `L1.walk`'s):
            // the value is judged against the rest of the domain.
            // A `pattern` that is not of the form, or that Rust refuses, is `L1.patterns`' to report: the value is judged against the rest.
            let mut judged = domain.clone();
            strip_patterns(&mut judged, true);
            schemas().validator(&judged).or_else(|first| {
                let mut stripped = domain.clone();
                strip_patterns(&mut stripped, false);
                if stripped == judged {
                    Err(first)
                } else {
                    schemas().validator(&stripped)
                }
            }).map(Arc::new)
        })
        .clone()
}

/// The first two problems of `value` against `validator`, as `where message`.
fn schema_problems(validator: &Validator, value: &Value) -> Option<String> {
    let problems: Vec<String> = validator
        .iter_errors(value)
        .take(2)
        .map(|error| {
            let at = error.instance_path().to_string();
            format!("{} {error}", if at.is_empty() { "/" } else { &at })
        })
        .collect();
    (!problems.is_empty()).then(|| problems.join("; "))
}

/// What is wrong with `value` as a value of `arg`: not valid for its valueType after normalisation (spec 10.7.2), or outside its domain.
pub fn value_problem(arg: &Arg, value: &Value) -> Option<String> {
    let normalised = normalise(&arg.value_type, value);
    if !valid_value(arg, &normalised) {
        return Some(format!("{value} is not a valid {} value", arg.value_type));
    }
    let domain = arg.domain.as_ref()?;
    // A domain outside the profile of its valueType is `L1.walk`'s (rule 3): a value is not judged against it.
    if super::rules::domain_unusable(arg) {
        return None;
    }
    match domain_validator(domain) {
        Ok(validator) => schema_problems(&validator, &normalised)
            .map(|why| format!("{value} is outside the domain of {}: {why}", arg.name)),
        // A domain that does not compile is `L1.walk`'s (rule 3): a value is not judged against it.
        Err(_) => None,
    }
}

/// Structural JSON equality: numbers by value, arrays by order, objects by members whatever their order, no scalar equals an array.
fn json_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y || x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| json_equal(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(key, x)| y.get(key).is_some_and(|y| json_equal(x, y)))
        }
        _ => a == b,
    }
}

/// Whether the argument `name` (an operand or an option) is explicitly given.
fn given(doc: &CommandDoc, args: &Args, name: &str) -> bool {
    args.options.contains_key(name)
        || doc
            .operands
            .iter()
            .position(|operand| operand.name == name)
            .is_some_and(|index| args.operands.len() > index)
}

/// Whether the operand or option `name` is among the explicitly given arguments (an operand when `args.operands` has its index).
pub fn arg_given(doc: &CommandDoc, args: &Args, name: &str) -> bool {
    given(doc, args, name)
}

/// Missing required arguments and too many operands: the part of spec 11.7 step 6 that is not a constraint.
pub fn requirement_problems(doc: &CommandDoc, args: &Args) -> Vec<String> {
    let mut problems = Vec::new();
    for option in &doc.options {
        if option.required && !args.options.contains_key(&option.name) {
            problems.push(format!("required --{} missing", option.name));
        }
    }
    let required = doc
        .operands
        .iter()
        .filter(|operand| operand.required)
        .count();
    if args.operands.len() < required {
        problems.push(format!("{required} operand(s) required"));
    }
    if !doc.operands.iter().any(|operand| operand.variadic)
        && args.operands.len() > doc.operands.len()
    {
        problems.push("too many operands".to_string());
    }
    problems
}

/// The normalised values `args` holds for the argument `name`, one element per occurrence.
fn values_of(doc: &CommandDoc, args: &Args, name: &str) -> Vec<Value> {
    if let Some(option) = doc.option(name) {
        let value = args.options.get(name).cloned().unwrap_or(Value::Null);
        let values = match value {
            Value::Array(values) if option.repeatable => values,
            other => vec![other],
        };
        return values
            .iter()
            .map(|value| normalise(&option.value_type, value))
            .collect();
    }
    let Some(index) = doc.operands.iter().position(|operand| operand.name == name) else {
        return Vec::new();
    };
    let operand = &doc.operands[index];
    let values = if operand.variadic {
        args.operands.get(index..).unwrap_or_default().to_vec()
    } else {
        vec![args.operands.get(index).cloned().unwrap_or(Value::Null)]
    };
    values
        .iter()
        .map(|value| normalise(&operand.value_type, value))
        .collect()
}

/// The indices into `doc.constraints` of the rules that `args` violate (spec 10.8.1: "given" means supplied explicitly; values compare after normalisation).
pub fn violated_constraints(doc: &CommandDoc, args: &Args) -> Vec<usize> {
    let mut violated = Vec::new();
    for (index, constraint) in doc.constraints.iter().enumerate() {
        // A rule that names an argument the command does not have is `L1.walk`'s (rule 4), not a rule an invocation can break.
        let names_unknown = constraint
            .names
            .iter()
            .chain(constraint.if_name.iter())
            .chain(constraint.then.iter())
            .any(|name| doc.arg(name).is_none());
        if names_unknown {
            continue;
        }
        let count = constraint
            .names
            .iter()
            .filter(|name| given(doc, args, name))
            .count();
        let broken = match constraint.rule.as_str() {
            "atLeastOne" => count < 1,
            "exactlyOne" => count != 1,
            "atMostOne" => count > 1,
            rule @ ("requires" | "forbids") => {
                let Some(name) = constraint
                    .if_name
                    .as_deref()
                    .filter(|name| given(doc, args, name))
                else {
                    continue;
                };
                if let Some(literals) = &constraint.if_in {
                    let kind = doc
                        .arg(name)
                        .map_or("string", |arg| arg.value_type.as_str());
                    let literals: Vec<Value> = literals
                        .iter()
                        .map(|literal| normalise(kind, literal))
                        .collect();
                    let actual = values_of(doc, args, name);
                    if !actual
                        .iter()
                        .any(|value| literals.iter().any(|literal| json_equal(literal, value)))
                    {
                        continue;
                    }
                }
                constraint.then.iter().any(|target| {
                    let target_given = given(doc, args, target);
                    if rule == "requires" {
                        !target_given
                    } else {
                        target_given
                    }
                })
            }
            _ => false,
        };
        if broken {
            violated.push(index);
        }
    }
    violated
}

/// An argument's value normalised by its valueType and cardinality (spec 11.7 step 3).
fn normalise_argument(arg: &Arg, value: &Value) -> Value {
    if arg.repeatable && arg.value_type != "count" {
        return match value {
            Value::Array(values) => Value::Array(
                values
                    .iter()
                    .map(|value| normalise(&arg.value_type, value))
                    .collect(),
            ),
            other => Value::Array(vec![normalise(&arg.value_type, other)]),
        };
    }
    normalise(&arg.value_type, value)
}

/// The argument object of 11.7 step 6 that `input.argumentsRef` describes.
///
/// `operands` normalised (absent operands with a non-null default appended in order), then every non-flag-only option under
/// its camelCase name with its normalised explicit value, or its non-null default. Environment values are not modelled.
pub fn arguments_object(doc: &CommandDoc, args: &Args) -> Value {
    let mut object = serde_json::Map::new();
    let mut operands: Vec<Value> = args
        .operands
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let kind =
                operand_arg(doc, index).map_or("string", |operand| operand.value_type.as_str());
            normalise(kind, value)
        })
        .collect();
    for operand in doc.operands.iter().skip(operands.len()) {
        if operand.default.is_null() {
            break;
        }
        if operand.variadic {
            match &operand.default {
                Value::Array(defaults) => operands.extend(
                    defaults
                        .iter()
                        .map(|value| normalise(&operand.value_type, value)),
                ),
                other => operands.push(normalise(&operand.value_type, other)),
            }
        } else {
            operands.push(normalise(&operand.value_type, &operand.default));
        }
    }
    if !operands.is_empty() {
        object.insert("operands".to_string(), Value::Array(operands));
    }
    for option in doc.options.iter().filter(|option| !option.flag_only) {
        let value = match args.options.get(&option.name) {
            Some(explicit) => normalise_argument(option, explicit),
            None => option.default.clone(),
        };
        if !value.is_null() {
            object.insert(camel(&option.name), value);
        }
    }
    Value::Object(object)
}

/// Replaces every occurrence of the option `name` by one occurrence with `value` (at the place of the first); appends it when absent.
pub fn set_option(invocation: &mut Invocation, name: &str, value: Option<String>) {
    let item = Item::Option {
        name: name.to_string(),
        value,
    };
    let mut placed = false;
    let mut items = Vec::with_capacity(invocation.items.len() + 1);
    for existing in std::mem::take(&mut invocation.items) {
        match existing {
            Item::Option { name: existing, .. } if existing == name => {
                if !placed {
                    placed = true;
                    items.push(item.clone());
                }
            }
            other => items.push(other),
        }
    }
    if !placed {
        items.push(item);
    }
    invocation.items = items;
}

/// Removes every occurrence of the option `name`.
pub fn remove_option(invocation: &mut Invocation, name: &str) {
    invocation
        .items
        .retain(|item| !matches!(item, Item::Option { name: existing, .. } if existing == name));
}

/// Sets the `index`-th operand; appends it when `index` is the number of operands given (or more).
pub fn set_operand(invocation: &mut Invocation, index: usize, value: String) {
    let slot = invocation
        .items
        .iter_mut()
        .filter(|item| matches!(item, Item::Operand(_)))
        .nth(index);
    match slot {
        Some(slot) => *slot = Item::Operand(value),
        None => invocation.items.push(Item::Operand(value)),
    }
}

/// Removes the `index`-th operand and every operand after it (the options stay).
pub fn remove_operand(invocation: &mut Invocation, index: usize) {
    let mut seen = 0;
    invocation.items.retain(|item| {
        if matches!(item, Item::Operand(_)) {
            seen += 1;
            seen <= index
        } else {
            true
        }
    });
}

/// The validator of the contract's `--input` document shape (`input-document.schema.json`), compiled once.
fn input_document_validator() -> Result<&'static Validator, String> {
    static CELL: OnceLock<Result<Validator, String>> = OnceLock::new();
    CELL.get_or_init(|| {
        let schema: Value = serde_json::from_str(atip::schema::INPUT_DOCUMENT_SCHEMA)
            .map_err(|error| error.to_string())?;
        schemas().validator(&schema)
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// The explicit arguments of one `--input` document: camelCase keys name options (spec 11.5). Problems of the values go to `problems`.
fn document_args(doc: &CommandDoc, item: &Value, at: &str, problems: &mut Vec<String>) -> Args {
    let mut out = Args::default();
    let Some(object) = item.as_object() else {
        return out;
    };
    if let Some(operands) = object.get("operands").and_then(Value::as_array) {
        out.operands.clone_from(operands);
    }
    for (index, value) in out.operands.iter().enumerate() {
        if let Some(problem) =
            operand_arg(doc, index).and_then(|operand| value_problem(operand, value))
        {
            problems.push(format!("{at}/operands/{index}: {problem}"));
        }
    }
    for (key, value) in object {
        if key == "operands" {
            continue;
        }
        let Some(option) = doc
            .options
            .iter()
            .find(|option| camel(&option.name) == *key)
        else {
            problems.push(format!("{at}/{key}: no such option"));
            continue;
        };
        if option.flag_only {
            problems.push(format!("{at}/{key}: flag-only option in a document"));
        }
        let values: Vec<&Value> = match value {
            Value::Array(values) if option.repeatable && option.value_type != "count" => {
                values.iter().collect()
            }
            other => vec![other],
        };
        for value in values {
            if let Some(problem) = value_problem(option, value) {
                problems.push(format!("{at}/{key}: {problem}"));
            }
        }
        let absent = (option.value_type == "switch" && value == &Value::Bool(false))
            || (option.value_type == "count" && value.as_u64() == Some(0));
        if !absent {
            out.options.insert(option.name.clone(), value.clone());
        }
    }
    out
}

/// The explicit arguments of one `--input` document (the keys are camelCase option names and `operands`) and the problems of its values:
/// a key no option has, a flag-only key, a value outside its type or domain.
pub fn document_arguments(doc: &CommandDoc, item: &Value) -> (Args, Vec<String>) {
    let mut problems = Vec::new();
    let args = document_args(doc, item, "", &mut problems);
    (args, problems)
}

/// Compares a document's arguments with argv's and merges them (spec 11.7 steps 3-4).
fn merge_args(
    doc: &CommandDoc,
    argv: &Args,
    document: &Args,
    at: &str,
    problems: &mut Vec<String>,
) -> Args {
    if !argv.operands.is_empty() && !document.operands.is_empty() {
        let normalised = |operands: &[Value]| -> Vec<Value> {
            operands
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    normalise(
                        operand_arg(doc, index)
                            .map_or("string", |operand| operand.value_type.as_str()),
                        value,
                    )
                })
                .collect()
        };
        if !json_equal(
            &Value::Array(normalised(&argv.operands)),
            &Value::Array(normalised(&document.operands)),
        ) {
            problems.push(format!("{at}: INPUT_CONFLICT on /operands"));
        }
    }
    let mut options = argv.options.clone();
    for (name, value) in &document.options {
        if let (Some(option), Some(previous)) = (doc.option(name), options.get(name)) {
            if !json_equal(
                &normalise_argument(option, previous),
                &normalise_argument(option, value),
            ) {
                problems.push(format!("{at}: INPUT_CONFLICT on /{}", camel(name)));
            }
        }
        options.insert(name.clone(), value.clone());
    }
    let operands = if argv.operands.is_empty() {
        document.operands.clone()
    } else {
        argv.operands.clone()
    };
    Args { operands, options }
}

/// Frames and validates an inline `--input` value of an example as spec 11.2-11.7 say (10.13 rule 12). `None` when the value
/// cannot be framed (a file or stdin, not JSON); the reason is in `problems`.
fn inline_documents(doc: &CommandDoc, text: &str, problems: &mut Vec<String>) -> Option<Vec<Args>> {
    if !text.trim_start().starts_with(['{', '[']) {
        problems.push(format!(
            "an example must not read stdin or a file (--input {text})"
        ));
        return None;
    }
    let Some(input) = &doc.input else {
        problems.push("the command declares no input".to_string());
        return None;
    };
    let Ok(parsed) = serde_json::from_str::<Value>(text) else {
        problems.push("the --input value is not JSON".to_string());
        return None;
    };
    match input_document_validator() {
        Ok(validator) => {
            if let Some(why) = schema_problems(validator, &parsed) {
                problems.push(format!("not an input document: {why}"));
                return None;
            }
        }
        Err(reason) => {
            problems.push(format!(
                "the input-document schema does not compile: {reason}"
            ));
            return None;
        }
    }
    let batch = parsed.is_array();
    if batch {
        if !input.batch {
            problems.push(format!(
                "a batch, but {} takes no batch (spec 11.4)",
                doc.command
            ));
        }
        let count = parsed.as_array().map_or(0, Vec::len);
        if count as u64 > input.max_items.unwrap_or(0) {
            problems.push(format!("a batch of {count} over maxItems"));
        }
    }
    // A schema that does not compile is `L1.walk`'s (rule 2): the documents are not judged against it.
    let fragment = doc.compile_ref(&input.schema_ref).ok();
    let documents: Vec<&Value> = match &parsed {
        Value::Array(items) => items.iter().collect(),
        other => vec![other],
    };
    Some(
        documents
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let at = if batch {
                    format!("/{index}")
                } else {
                    String::new()
                };
                if let Some(why) = fragment
                    .as_ref()
                    .and_then(|fragment| schema_problems(fragment, item))
                {
                    problems.push(format!("{at}: fragment: {why}"));
                }
                document_args(doc, item, &at, problems)
            })
            .collect(),
    )
}

/// Every typed value of the options and operands of `invocation`, judged by `value_problem`; a count by its occurrences.
fn value_problems(doc: &CommandDoc, invocation: &Invocation) -> Vec<String> {
    let mut problems = Vec::new();
    let mut operand_index = 0;
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    for item in &invocation.items {
        let (spelling, arg, text) = match item {
            Item::Operand(text) => {
                let index = operand_index;
                operand_index += 1;
                (format!("operand {index}"), operand_arg(doc, index), text)
            }
            Item::Option { name, value } => {
                let Some(option) = doc.option(name).filter(|option| option.name != "input") else {
                    continue;
                };
                if option.value_type == "count" {
                    *counts.entry(&option.name).or_insert(0) += 1;
                    continue;
                }
                let Some(text) = value else { continue };
                (format!("--{name}"), Some(option), text)
            }
        };
        let Some(arg) = arg else { continue };
        match typed(Some(arg), text) {
            Ok(value) => {
                problems.extend(value_problem(arg, &value).map(|why| format!("{spelling}: {why}")));
            }
            Err(why) => problems.push(format!("{spelling}: {why}")),
        }
    }
    for (name, count) in counts {
        if let Some(option) = doc.option(name) {
            problems.extend(
                value_problem(option, &Value::from(count)).map(|why| format!("--{name}: {why}")),
            );
        }
    }
    problems
}

/// The names of the arguments of `invocation` whose value is outside its type or its domain (a count by its occurrences), each once, in the order of the arguments.
pub fn broken_arguments(doc: &CommandDoc, invocation: &Invocation) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut operand_index = 0;
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    for item in &invocation.items {
        let (arg, text) = match item {
            Item::Operand(text) => {
                let index = operand_index;
                operand_index += 1;
                (operand_arg(doc, index), text)
            }
            Item::Option { name, value } => {
                let Some(option) = doc.option(name).filter(|option| option.name != "input") else {
                    continue;
                };
                if option.value_type == "count" {
                    *counts.entry(&option.name).or_insert(0) += 1;
                    continue;
                }
                let Some(text) = value else { continue };
                (Some(option), text)
            }
        };
        let Some(arg) = arg else { continue };
        let wrong = match typed(Some(arg), text) {
            Ok(value) => value_problem(arg, &value).is_some(),
            Err(_) => true,
        };
        if wrong && !names.contains(&arg.name) {
            names.push(arg.name.clone());
        }
    }
    for (name, count) in counts {
        if let Some(option) = doc.option(name) {
            if value_problem(option, &Value::from(count)).is_some() && !names.contains(&option.name) {
                names.push(option.name.clone());
            }
        }
    }
    names
}

/// `Some(problems joined)` when there are any.
fn joined(problems: &[String]) -> Option<String> {
    (!problems.is_empty()).then(|| problems.join("; "))
}

/// What `L1.examples` judges of one documented line, as `(condition, problem)` pairs: one pair per condition judged, `None` when it holds.
/// The conditions are `tool-name`, `path`, `grammar`, `no-placeholder`, `values`, `requirements`, `constraints`, `arguments-ref`, `input-inline`.
/// `with_arguments_ref` is `false` for a validator's `rejects` line (spec 10.13 rule 13).
///
/// A line that does not split is judged by `grammar` alone; one whose path is not the command's stops after `path`; one that does
/// not parse stops at `grammar`, and one with a placeholder at `no-placeholder`. A `--input` value that cannot be framed
/// (a file, stdin, not JSON) leaves `requirements`, `constraints` and `arguments-ref` unjudged, as there are no merged arguments.
pub fn check_line(
    doc: &CommandDoc,
    tool: &str,
    line: &str,
    with_arguments_ref: bool,
) -> Vec<(&'static str, Option<String>)> {
    let mut out: Vec<(&'static str, Option<String>)> = Vec::new();
    let words = match split_line(line) {
        Ok(words) => words,
        Err(why) => return vec![("grammar", Some(why))],
    };
    out.push((
        "tool-name",
        (words.first().map(String::as_str) != Some(tool))
            .then(|| format!("does not start with {tool}")),
    ));
    let rest = words.get(1..).unwrap_or_default();
    let on_path = rest.get(..doc.path.len()) == Some(doc.path.as_slice());
    out.push((
        "path",
        (!on_path).then(|| format!("the path is not {}", doc.command)),
    ));
    if !on_path {
        return out;
    }
    let invocation = match parse(doc, rest) {
        Ok(invocation) => invocation,
        Err(why) => {
            out.push(("grammar", Some(why)));
            return out;
        }
    };
    out.push(("grammar", None));
    let has_placeholder = words.iter().find(|word| placeholder().is_match(word));
    out.push((
        "no-placeholder",
        has_placeholder.map(|word| format!("placeholder or ellipsis in {word}")),
    ));
    if has_placeholder.is_some() {
        return out;
    }
    out.push(("values", joined(&value_problems(doc, &invocation))));

    let input_text = invocation.items.iter().find_map(|item| match item {
        Item::Option {
            name,
            value: Some(value),
        } if name == "input" => Some(value.as_str()),
        _ => None,
    });
    let mut argv = args(doc, &invocation);
    argv.options.remove("input");
    let mut inline_problems = Vec::new();
    let documents = input_text.map(|text| inline_documents(doc, text, &mut inline_problems));
    let merged: Option<Vec<(String, Args)>> = match documents {
        None => Some(vec![(String::new(), argv)]),
        Some(None) => None,
        Some(Some(documents)) => {
            let many = documents.len() > 1;
            Some(
                documents
                    .iter()
                    .enumerate()
                    .map(|(index, document)| {
                        let at = if many {
                            format!("item {index}")
                        } else {
                            String::new()
                        };
                        let merged = merge_args(doc, &argv, document, &at, &mut inline_problems);
                        (at, merged)
                    })
                    .collect(),
            )
        }
    };
    if let Some(merged) = merged {
        let prefixed = |at: &str, why: String| {
            if at.is_empty() {
                why
            } else {
                format!("{at}: {why}")
            }
        };
        let mut requirements = Vec::new();
        let mut constraints = Vec::new();
        let mut arguments = Vec::new();
        let arguments_ref = doc
            .input
            .as_ref()
            .and_then(|input| input.arguments_ref.as_deref())
            .filter(|reference| {
                with_arguments_ref
                    && super::model::ref_name(reference)
                        .is_some_and(|name| doc.schemas.contains_key(name))
            });
        let validator = arguments_ref.map(|reference| doc.compile_ref(reference));
        for (at, merged) in &merged {
            requirements.extend(
                requirement_problems(doc, merged)
                    .into_iter()
                    .map(|why| prefixed(at, why)),
            );
            constraints.extend(violated_constraints(doc, merged).into_iter().map(|index| {
                prefixed(
                    at,
                    format!("violates constraint {}", doc.constraints[index].label()),
                )
            }));
            if let Some(Ok(validator)) = &validator {
                if let Some(why) = schema_problems(validator, &arguments_object(doc, merged)) {
                    arguments.push(prefixed(
                        at,
                        format!("the argument object fails argumentsRef: {why}"),
                    ));
                }
            }
        }
        out.push(("requirements", joined(&requirements)));
        out.push(("constraints", joined(&constraints)));
        if validator.is_some() {
            out.push(("arguments-ref", joined(&arguments)));
        }
    }
    if input_text.is_some() {
        out.push(("input-inline", joined(&inline_problems)));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema-docs")
    }

    /// The command document of a fixture file, and the tool it belongs to.
    fn load(path: &std::path::Path) -> (CommandDoc, String) {
        let text = std::fs::read_to_string(path).expect("fixture is readable");
        let document: Value = serde_json::from_str(&text).expect("fixture is JSON");
        let doc = CommandDoc::parse(&document["data"]).expect("fixture holds a command document");
        let tool = doc
            .examples
            .first()
            .and_then(|line| line.split_whitespace().next())
            .unwrap_or("")
            .to_string();
        (doc, tool)
    }

    fn files(dir: &std::path::Path, prefix: &str) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("fixture directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(prefix) && name.ends_with(".json"))
            })
            .collect();
        found.sort();
        found
    }

    fn problems(results: &[(&'static str, Option<String>)]) -> Vec<(&'static str, String)> {
        results
            .iter()
            .filter_map(|(condition, problem)| problem.clone().map(|problem| (*condition, problem)))
            .collect()
    }

    #[test]
    fn atip_checkl1_invocation_valid_examples_pass_every_condition() {
        let documents = files(&fixtures(), "schema-command-");
        assert!(documents.len() >= 14, "the valid documents are copied");
        let mut lines = 0;
        for path in documents {
            let (doc, tool) = load(&path);
            assert!(!doc.examples.is_empty(), "{path:?} has examples");
            for line in &doc.examples {
                let results = check_line(&doc, &tool, line, true);
                assert!(
                    problems(&results).is_empty(),
                    "{line}: {:?}",
                    problems(&results)
                );
                let judged: Vec<&str> = results.iter().map(|(condition, _)| *condition).collect();
                for needed in [
                    "tool-name",
                    "path",
                    "grammar",
                    "no-placeholder",
                    "values",
                    "requirements",
                    "constraints",
                ] {
                    assert!(judged.contains(&needed), "{line}: {needed} is judged");
                }
                lines += 1;
            }
        }
        assert!(lines >= 14);
    }

    #[test]
    fn atip_checkl1_invocation_valid_rejects_lines_pass_without_arguments_ref() {
        let mut rejects = 0;
        for path in files(&fixtures(), "schema-command-") {
            let (doc, tool) = load(&path);
            for validator in &doc.validators {
                for line in &validator.rejects {
                    let results = check_line(&doc, &tool, line, false);
                    assert!(
                        problems(&results).is_empty(),
                        "{line}: {:?}",
                        problems(&results)
                    );
                    assert!(results
                        .iter()
                        .all(|(condition, _)| *condition != "arguments-ref"));
                    rejects += 1;
                }
            }
        }
        assert!(
            rejects > 0,
            "some valid document has a validator with rejects lines"
        );
    }

    #[test]
    fn atip_checkl1_invocation_invalid_documents_break_the_matching_condition() {
        let expected: &[(&str, &str)] = &[
            ("schema-example-count-outside-domain", "values"),
            ("schema-example-input-incomplete", "requirements"),
            ("schema-example-input-json-shape-conflict", "input-inline"),
            ("schema-example-input-scalar-repeatable", "input-inline"),
            ("schema-example-integer-max-plus-one", "values"),
            ("schema-example-integer-min-minus-one", "values"),
            ("schema-example-integer-unsafe", "values"),
            ("schema-example-member-tag-zero", "values"),
            ("schema-example-number-form", "values"),
            ("schema-example-operand-type", "values"),
            ("schema-example-outside-domain-json", "values"),
            ("schema-example-outside-domain", "values"),
            ("schema-example-placeholder", "no-placeholder"),
            ("schema-example-reads-stdin", "input-inline"),
            ("schema-example-short-undeclared", "grammar"),
            ("schema-example-unclosed-quote", "grammar"),
            ("schema-example-variadic-without-default", "arguments-ref"),
            ("schema-example-violates-constraint", "constraints"),
        ];
        let dir = fixtures().join("invalid");
        for (name, condition) in expected {
            let (doc, tool) = load(&dir.join(format!("{name}.json")));
            let broken: Vec<(&str, String)> = doc
                .examples
                .iter()
                .flat_map(|line| problems(&check_line(&doc, &tool, line, true)))
                .collect();
            assert!(
                broken.iter().any(|(found, _)| found == condition),
                "{name}: expected a problem in {condition}, got {broken:?}"
            );
        }
    }

    #[test]
    fn atip_checkl1_invocation_rejects_line_that_fails_its_domain_is_found() {
        let (doc, tool) =
            load(&fixtures().join("invalid/schema-validator-rejects-fails-domain.json"));
        let broken: Vec<_> = doc
            .validators
            .iter()
            .flat_map(|validator| &validator.rejects)
            .flat_map(|line| problems(&check_line(&doc, &tool, line, false)))
            .collect();
        assert!(
            broken.iter().any(|(condition, _)| *condition == "values"),
            "{broken:?}"
        );
    }

    /// A hand-written command exercising every part of the grammar: `tool run <src> [rest...]`.
    fn synthetic() -> CommandDoc {
        let data = json!({
            "kind": "command", "command": "run", "path": ["run"], "effect": "read",
            "operands": [
                {"name": "src", "valueType": "string", "required": true, "variadic": false, "default": null},
                {"name": "rest", "valueType": "integer", "required": false, "variadic": true, "default": [1, 2]}
            ],
            "options": [
                {"name": "out", "valueType": "string", "short": "o", "required": false, "repeatable": false, "default": null},
                {"name": "level", "valueType": "integer", "required": false, "repeatable": false, "default": 3,
                 "domain": {"minimum": 1, "maximum": 9}},
                {"name": "verbose", "valueType": "count", "short": "v", "required": false, "repeatable": true, "default": 0,
                 "domain": {"maximum": 2}},
                {"name": "dry-run", "valueType": "switch", "short": "d", "required": false, "repeatable": false, "default": false},
                {"name": "tag", "valueType": "string", "required": false, "repeatable": true, "default": null},
                {"name": "mode", "valueType": "enum", "possibleValues": ["a", "b"], "required": false, "repeatable": false, "default": null},
                {"name": "scope", "valueType": "json", "required": false, "repeatable": false, "default": null},
                {"name": "weight", "valueType": "number", "required": false, "repeatable": false, "default": null},
                {"name": "ratio", "valueType": "number", "required": false, "repeatable": false, "default": null},
                {"name": "input", "valueType": "string", "required": false, "repeatable": false, "default": null, "flagOnly": true}
            ],
            "constraints": [
                {"rule": "requires", "if": {"name": "mode", "in": ["a"]}, "then": ["level"]},
                {"rule": "forbids", "if": {"name": "dry-run"}, "then": ["out"]},
                {"rule": "atMostOne", "names": ["level", "weight"]},
                {"rule": "requires", "if": {"name": "tag", "in": ["x"]}, "then": ["out"]},
                {"rule": "requires", "if": {"name": "rest", "in": [5]}, "then": ["ratio"]}
            ],
            "input": {
                "schemaRef": "#/schemas/RunInput", "argumentsRef": "#/schemas/RunArgs", "batch": true, "maxItems": 2,
                "example": {"operands": ["a"]}
            },
            "schemas": {
                "RunInput": {
                    "type": "object", "additionalProperties": false,
                    "properties": {
                        "operands": {"type": "array", "items": {"not": {"type": "null"}}, "minItems": 1},
                        "out": {"type": "string"}, "level": {"type": ["integer", "string"]},
                        "verbose": {"type": "integer", "minimum": 0}, "dryRun": {"type": "boolean"},
                        "tag": {"type": "array", "items": {"type": "string"}},
                        "mode": {"enum": ["a", "b"]}, "scope": {"not": {"type": "null"}},
                        "weight": {"type": ["number", "string"]}, "ratio": {"type": ["number", "string"]}
                    }
                },
                "RunArgs": {"type": "object", "properties": {"level": {"type": "integer", "maximum": 5}}}
            },
            "examples": ["tool run a"]
        });
        CommandDoc::parse(&data).expect("a command document")
    }

    fn words(line: &str) -> Vec<String> {
        split_line(line).expect("the line splits")
    }

    fn read(doc: &CommandDoc, line: &str) -> Invocation {
        parse(doc, &words(line)).unwrap_or_else(|why| panic!("{line}: {why}"))
    }

    fn option(name: &str, value: Option<&str>) -> Item {
        Item::Option {
            name: name.to_string(),
            value: value.map(str::to_string),
        }
    }

    fn operand(value: &str) -> Item {
        Item::Operand(value.to_string())
    }

    fn judged(doc: &CommandDoc, line: &str) -> Vec<(&'static str, Option<String>)> {
        check_line(doc, "tool", line, true)
    }

    fn broken(doc: &CommandDoc, line: &str) -> Vec<&'static str> {
        problems(&judged(doc, line))
            .into_iter()
            .map(|(condition, _)| condition)
            .collect()
    }

    #[test]
    fn atip_checkl1_invocation_split_line_quotes_and_escapes() {
        assert_eq!(
            words(r#"a  'b c'  "d \" e"  f\ g"#),
            ["a", "b c", "d \" e", "f g"]
        );
        assert_eq!(
            words(r#"--json '{"a": "b c"}' ''"#),
            ["--json", r#"{"a": "b c"}"#, ""]
        );
        assert_eq!(
            words("a\\\nb"),
            ["ab"],
            "a trailing backslash joins the next line"
        );
        assert_eq!(words(r#""a\\b" "a\nb""#), [r"a\b", r"a\nb"]);
        assert_eq!(words("<CHANGE_ID> x<A_1>y"), ["<CHANGE_ID>", "x<A_1>y"]);
        assert!(words("   ").is_empty());
        assert_eq!(split_line("a 'b").unwrap_err(), "unclosed ' quote");
        assert_eq!(split_line("a \"b").unwrap_err(), "unclosed \" quote");
    }

    #[test]
    fn atip_checkl1_invocation_split_line_refuses_shell_syntax_outside_quotes() {
        for line in [
            "a | b",
            "a > f",
            "a < f",
            "a; b",
            "a && b",
            "a & b",
            "echo $(x)",
            "a `b`",
            "a 2>&1",
            "a <lower>",
            "a <>",
            "a <A",
        ] {
            assert!(split_line(line).is_err(), "{line} is unsupported");
        }
        assert_eq!(
            words(r#"a '|' ">" "$(x)" \| \; $HOME"#),
            ["a", "|", ">", "$(x)", "|", ";", "$HOME"]
        );
    }

    #[test]
    fn atip_checkl1_invocation_parse_reads_every_option_form() {
        let doc = synthetic();
        let inv = read(
            &doc,
            "run a --out=x --level=-5 --mode b -v -v --tag t1 --tag=t2 -d --scope='[1]' --weight=",
        );
        assert_eq!(
            inv.items,
            [
                operand("a"),
                option("out", Some("x")),
                option("level", Some("-5")),
                option("mode", Some("b")),
                option("verbose", None),
                option("verbose", None),
                option("tag", Some("t1")),
                option("tag", Some("t2")),
                option("dry-run", None),
                option("scope", Some("[1]")),
                option("weight", Some("")),
            ]
        );
        assert_eq!(
            read(&doc, "run -o y a").items,
            [option("out", Some("y")), operand("a")],
            "a short form is the long name"
        );
        assert_eq!(
            read(&doc, "run --input - a").items,
            [option("input", Some("-")), operand("a")]
        );
        assert_eq!(read(&doc, "run a -").items, [operand("a"), operand("-")]);
    }

    #[test]
    fn atip_checkl1_invocation_parse_refuses_what_the_grammar_does_not_allow() {
        let doc = synthetic();
        for (line, reason) in [
            ("run a --level -5", "needs a value"),
            ("run a --level", "needs a value"),
            ("run a --level --out=x", "needs a value"),
            ("run a --out x --out y", "given twice"),
            ("run a --level=1 --level=2", "given twice"),
            ("run a -d --dry-run", "given twice"),
            ("run a --dry-run=true", "takes no value"),
            ("run a --verbose=2", "takes no value"),
            ("run a --nosuch", "unknown option --nosuch"),
            ("run a -x", "unknown option -x"),
            ("run a -vv", "unknown option -vv"),
            ("run a -ox", "unknown option -ox"),
            ("run a -o=x", "unknown option -o=x"),
            ("run a -5", "unknown option -5"),
        ] {
            let why = parse(&doc, &words(line)).unwrap_err();
            assert!(why.contains(reason), "{line}: {why}");
        }
        assert!(
            parse(&doc, &words("other a")).is_err(),
            "the path must be there"
        );
        parse(&doc, &[]).unwrap_err();
        let empty = vec!["run".to_string(), String::new()];
        assert!(
            parse(&doc, &empty).is_err(),
            "an empty word is neither an operand nor an option"
        );
    }

    #[test]
    fn atip_checkl1_invocation_parse_double_dash_makes_everything_an_operand() {
        let doc = synthetic();
        let inv = read(&doc, "run a --out=x -- --level -d -- ''");
        assert_eq!(
            inv.items,
            [
                operand("a"),
                option("out", Some("x")),
                operand("--level"),
                operand("-d"),
                operand("--"),
                operand("")
            ]
        );
        assert_eq!(read(&doc, "run --").items, []);
    }

    #[test]
    fn atip_checkl1_invocation_parse_keeps_tool_wide_options_before_the_path() {
        let doc = synthetic();
        let inv = read(&doc, "--repo . --json run a");
        assert_eq!(inv.before, ["--repo", ".", "--json"]);
        assert_eq!(inv.path, ["run"]);
        assert_eq!(inv.items, [operand("a")]);
        assert_eq!(read(&doc, "--repo=. run a").before, ["--repo=."]);
        let two = {
            let mut two = synthetic();
            two.path = vec!["g".to_string(), "run".to_string()];
            two
        };
        assert_eq!(
            read(&two, "--repo g run").before,
            ["--repo"],
            "the first path word is not a value"
        );
        parse(&two, &words("g other")).unwrap_err();
    }

    #[test]
    fn atip_checkl1_invocation_render_round_trips_what_parse_produced() {
        let doc = synthetic();
        for line in [
            "run a",
            "--repo . run a --out=x -v -v --tag t1 --tag=t2 -d --scope='[1]'",
            "run a --weight= --level=-5 --input -",
            "run a -- --level -d -- '' -",
            "run a b -- -1 2 --out",
            "run -- -a",
            "run",
        ] {
            let inv = read(&doc, line);
            let argv = render(&inv);
            assert_eq!(
                read(
                    &doc,
                    &argv
                        .iter()
                        .map(|w| format!("'{w}'"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
                inv,
                "{line}: {argv:?}"
            );
            assert_eq!(parse(&doc, &argv), Ok(inv));
        }
        assert_eq!(
            render(&read(&doc, "run a --out x -d -v --level=-1 -- -b")),
            [
                "run",
                "a",
                "--out=x",
                "--dry-run",
                "--verbose",
                "--level=-1",
                "--",
                "-b"
            ]
        );
        assert_eq!(render(&read(&doc, "run a b")), ["run", "a", "b"]);
    }

    #[test]
    fn atip_checkl1_invocation_args_types_operands_and_options() {
        let mut doc = synthetic();
        doc.operands[0].value_type = "json".to_string();
        let inv = read(
            &doc,
            "run '{\"a\":1}' 5 6 -v -v -v -d --tag a --tag b --level 4 --scope='[1]' --mode b",
        );
        let given = args(&doc, &inv);
        assert_eq!(given.operands, [json!({"a": 1}), json!("5"), json!("6")]);
        assert_eq!(given.options["verbose"], json!(3));
        assert_eq!(given.options["dry-run"], json!(true));
        assert_eq!(given.options["tag"], json!(["a", "b"]));
        assert_eq!(
            given.options["level"],
            json!("4"),
            "a value is the argv word"
        );
        assert_eq!(given.options["scope"], json!([1]));
        assert_eq!(given.options["mode"], json!("b"));
        let bad = args(&doc, &read(&doc, "run not-json --scope=nope"));
        assert_eq!(
            bad.operands,
            [json!("not-json")],
            "a json value that does not parse stays a string"
        );
        assert_eq!(bad.options["scope"], json!("nope"));
    }

    #[test]
    fn atip_checkl1_invocation_normalise_keeps_unsafe_integers_as_strings() {
        assert_eq!(normalise("integer", &json!("5")), json!(5));
        assert_eq!(normalise("integer", &json!("-0")), json!(0));
        assert_eq!(
            normalise("integer", &json!("9007199254740991")),
            json!(9_007_199_254_740_991_i64)
        );
        assert_eq!(
            normalise("integer", &json!("-9007199254740991")),
            json!(-9_007_199_254_740_991_i64)
        );
        assert_eq!(
            normalise("integer", &json!("9007199254740992")),
            json!("9007199254740992")
        );
        assert_eq!(
            normalise("integer", &json!("-9007199254740992")),
            json!("-9007199254740992")
        );
        assert_eq!(
            normalise("integer", &json!("99999999999999999999")),
            json!("99999999999999999999")
        );
        for text in ["+1", "01", "1e3", " 1", "1.0", "0x7", ""] {
            assert_eq!(normalise("integer", &json!(text)), json!(text), "{text}");
        }
        assert_eq!(normalise("count", &json!("3")), json!(3));
        assert_eq!(normalise("number", &json!("1.5")), json!(1.5));
        assert_eq!(normalise("number", &json!("1e3")), json!(1000.0));
        assert_eq!(normalise("number", &json!("-2")), json!(-2));
        assert_eq!(normalise("number", &json!("1e999")), json!("1e999"));
        assert_eq!(normalise("number", &json!(".5")), json!(".5"));
        assert_eq!(normalise("string", &json!("5")), json!("5"));
        assert_eq!(normalise("integer", &json!(true)), json!(true));
        assert_eq!(normalise("integer", &json!(7)), json!(7));
    }

    #[test]
    fn atip_checkl1_invocation_value_problem_judges_type_and_domain() {
        let doc = synthetic();
        let level = doc.option("level").expect("level");
        assert_eq!(value_problem(level, &json!("5")), None);
        assert_eq!(value_problem(level, &json!(5)), None);
        assert_eq!(value_problem(level, &json!(5.0)), None);
        assert!(value_problem(level, &json!("x"))
            .expect("not an integer")
            .contains("not a valid integer"));
        assert!(value_problem(level, &json!("9007199254740992"))
            .expect("unsafe")
            .contains("not a valid integer"));
        assert!(value_problem(level, &json!(1.5)).is_some());
        assert!(value_problem(level, &json!("0"))
            .expect("below")
            .contains("outside the domain of level"));
        assert!(value_problem(level, &json!("10"))
            .expect("above")
            .contains("maximum"));
        let verbose = doc.option("verbose").expect("verbose");
        assert_eq!(value_problem(verbose, &json!(2)), None);
        assert!(value_problem(verbose, &json!(3)).is_some());
        assert!(value_problem(verbose, &json!(-1)).is_some());
        let mode = doc.option("mode").expect("mode");
        assert_eq!(value_problem(mode, &json!("a")), None);
        assert!(value_problem(mode, &json!("c")).is_some());
        assert!(value_problem(doc.option("weight").expect("weight"), &json!("1e999")).is_some());
        assert_eq!(
            value_problem(doc.option("weight").expect("weight"), &json!("-1.5e2")),
            None
        );
        let scope = doc.option("scope").expect("scope");
        assert_eq!(value_problem(scope, &json!(false)), None);
        assert!(value_problem(scope, &Value::Null).is_some());
        assert_eq!(
            value_problem(doc.option("dry-run").expect("dry-run"), &json!(true)),
            None
        );
        assert!(value_problem(doc.option("dry-run").expect("dry-run"), &json!("true")).is_some());
        assert!(
            value_problem(doc.option("out").expect("out"), &json!(5)).is_some(),
            "no coercion to a string"
        );
    }

    #[test]
    fn atip_checkl1_invocation_requirements_count_operands_and_options() {
        let mut doc = synthetic();
        doc.options[0].required = true;
        let none = args(&doc, &read(&doc, "run"));
        assert_eq!(
            requirement_problems(&doc, &none),
            ["required --out missing", "1 operand(s) required"]
        );
        let enough = args(&doc, &read(&doc, "run a 1 2 3 -o x"));
        assert!(
            requirement_problems(&doc, &enough).is_empty(),
            "the last operand is variadic"
        );
        doc.operands[1].variadic = false;
        let many = args(&doc, &read(&doc, "run a 1 2 -o x"));
        assert_eq!(requirement_problems(&doc, &many), ["too many operands"]);
    }

    #[test]
    fn atip_checkl1_invocation_constraints_use_given_arguments_and_normalised_in() {
        let doc = synthetic();
        let violated = |line: &str| violated_constraints(&doc, &args(&doc, &read(&doc, line)));
        assert_eq!(violated("run a"), Vec::<usize>::new());
        assert_eq!(
            violated("run a --mode a"),
            [0],
            "requires level when mode is a"
        );
        assert_eq!(
            violated("run a --mode b"),
            Vec::<usize>::new(),
            "mode b is not in the condition"
        );
        assert_eq!(violated("run a --mode a --level 2"), Vec::<usize>::new());
        assert_eq!(violated("run a -d -o x"), [1], "forbids");
        assert_eq!(violated("run a -d"), Vec::<usize>::new());
        assert_eq!(violated("run a --level 2 --weight 1"), [2], "atMostOne");
        assert_eq!(
            violated("run a --tag y --tag x"),
            [3],
            "a repeatable option holds when any value is in"
        );
        assert_eq!(violated("run a --tag y"), Vec::<usize>::new());
        assert_eq!(
            violated("run a 1 5 2"),
            [4],
            "a variadic operand holds when any value is in; 5 is normalised"
        );
        assert_eq!(violated("run a 1 6"), Vec::<usize>::new());
        assert_eq!(violated("run a 1 5 --ratio 1"), Vec::<usize>::new());
        // A value that is only a default is not given.
        assert_eq!(violated("run a"), Vec::<usize>::new());
    }

    #[test]
    fn atip_checkl1_invocation_arguments_object_normalises_and_adds_defaults() {
        let doc = synthetic();
        let object = arguments_object(
            &doc,
            &args(
                &doc,
                &read(
                    &doc,
                    "run a 7 --tag t -v -v --level 4 -d --weight 2 --input -",
                ),
            ),
        );
        assert_eq!(
            object,
            json!({"operands": ["a", 7], "level": 4, "verbose": 2, "dryRun": true, "tag": ["t"], "weight": 2})
        );
        let defaults = arguments_object(&doc, &args(&doc, &read(&doc, "run a")));
        assert_eq!(
            defaults,
            json!({"operands": ["a", 1, 2], "level": 3, "verbose": 0, "dryRun": false}),
            "the variadic default appends its elements; flag-only options and null defaults are absent"
        );
    }

    #[test]
    fn atip_checkl1_invocation_helpers_plan_probe_invocations() {
        let doc = synthetic();
        let mut inv = read(&doc, "run a --tag x -o y --tag z 1");
        let given = args(&doc, &inv);
        assert!(
            arg_given(&doc, &given, "src")
                && arg_given(&doc, &given, "rest")
                && arg_given(&doc, &given, "tag")
        );
        assert!(!arg_given(&doc, &given, "level") && !arg_given(&doc, &given, "nosuch"));
        assert!(!arg_given(&doc, &args(&doc, &read(&doc, "run")), "src"));
        set_option(&mut inv, "tag", Some("only".to_string()));
        assert_eq!(
            inv.items,
            [
                operand("a"),
                option("tag", Some("only")),
                option("out", Some("y")),
                operand("1")
            ]
        );
        set_option(&mut inv, "dry-run", None);
        assert_eq!(inv.items.last(), Some(&option("dry-run", None)));
        remove_option(&mut inv, "out");
        remove_option(&mut inv, "nosuch");
        assert_eq!(
            inv.items,
            [
                operand("a"),
                option("tag", Some("only")),
                operand("1"),
                option("dry-run", None)
            ]
        );
        set_operand(&mut inv, 1, "9".to_string());
        set_operand(&mut inv, 2, "10".to_string());
        assert_eq!(
            inv.items,
            [
                operand("a"),
                option("tag", Some("only")),
                operand("9"),
                option("dry-run", None),
                operand("10")
            ]
        );
        remove_operand(&mut inv, 1);
        assert_eq!(
            inv.items,
            [
                operand("a"),
                option("tag", Some("only")),
                option("dry-run", None)
            ]
        );
        remove_operand(&mut inv, 0);
        assert_eq!(
            inv.items,
            [option("tag", Some("only")), option("dry-run", None)]
        );
    }

    #[test]
    fn atip_checkl1_invocation_check_line_reports_one_pair_per_condition_judged() {
        let doc = synthetic();
        let names = |line: &str| {
            judged(&doc, line)
                .into_iter()
                .map(|(condition, _)| condition)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names("tool run a --level 2"),
            [
                "tool-name",
                "path",
                "grammar",
                "no-placeholder",
                "values",
                "requirements",
                "constraints",
                "arguments-ref"
            ]
        );
        assert_eq!(
            names("tool run a --input '{\"level\":2}'"),
            [
                "tool-name",
                "path",
                "grammar",
                "no-placeholder",
                "values",
                "requirements",
                "constraints",
                "arguments-ref",
                "input-inline"
            ]
        );
        assert!(problems(&judged(&doc, "tool run a --level 2")).is_empty());
        assert_eq!(
            check_line(&doc, "tool", "tool run a --level 2", false).len(),
            7
        );
        assert_eq!(names("tool run a --level '2"), ["grammar"]);
        assert_eq!(names("tool run a | cat"), ["grammar"]);
        assert_eq!(
            names("other run a"),
            [
                "tool-name",
                "path",
                "grammar",
                "no-placeholder",
                "values",
                "requirements",
                "constraints",
                "arguments-ref"
            ]
        );
        assert_eq!(broken(&doc, "other run a"), ["tool-name"]);
        assert_eq!(names("tool walk a"), ["tool-name", "path"]);
        assert_eq!(broken(&doc, "tool walk a"), ["path"]);
        assert_eq!(names(""), ["tool-name", "path"]);
        assert_eq!(
            names("tool run a --nosuch"),
            ["tool-name", "path", "grammar"]
        );
        assert_eq!(broken(&doc, "tool run a --nosuch"), ["grammar"]);
        assert_eq!(
            names("tool run <SRC>"),
            ["tool-name", "path", "grammar", "no-placeholder"]
        );
        assert_eq!(broken(&doc, "tool run a --tag ..."), ["no-placeholder"]);
        assert_eq!(broken(&doc, "tool run a --tag …"), ["no-placeholder"]);
    }

    #[test]
    fn atip_checkl1_invocation_check_line_judges_values_requirements_and_constraints() {
        let doc = synthetic();
        assert_eq!(broken(&doc, "tool run a --level=-5"), ["values"]);
        assert_eq!(
            broken(&doc, "tool run a --level=9"),
            ["arguments-ref"],
            "argumentsRef alone caps level at 5"
        );
        assert_eq!(
            broken(&doc, "tool run a -v -v -v"),
            ["values"],
            "a count's occurrences are its value"
        );
        assert_eq!(broken(&doc, "tool run a 7 x"), ["values"]);
        assert_eq!(broken(&doc, "tool run a --scope=nope"), ["values"]);
        assert_eq!(broken(&doc, "tool run a --mode c"), ["values"]);
        assert_eq!(broken(&doc, "tool run --level 2"), ["requirements"]);
        assert_eq!(broken(&doc, "tool run a 1 2 --mode a"), ["constraints"]);
        let results = judged(&doc, "tool run a -d -o x");
        assert_eq!(
            results
                .iter()
                .find(|(condition, _)| *condition == "constraints")
                .and_then(|(_, p)| p.as_deref()),
            Some("violates constraint forbids dry-run -> [out]")
        );
        assert!(broken(&doc, "tool run a --level=-5 --weight 1").contains(&"constraints"));
    }

    #[test]
    fn atip_checkl1_invocation_inline_input_is_framed_and_merged() {
        let doc = synthetic();
        assert!(problems(&judged(
            &doc,
            r#"tool run --input '{"operands":["a"],"level":2}'"#
        ))
        .is_empty());
        assert!(
            problems(&judged(&doc, r#"tool run a --input '{"level":2}'"#)).is_empty(),
            "argv and document merge"
        );
        assert!(
            problems(&judged(
                &doc,
                r#"tool run a --level 2 --input '{"level":"2"}'"#
            ))
            .is_empty(),
            "equal after normalisation"
        );
        assert!(problems(&judged(
            &doc,
            r#"tool run --input ' [{"operands":["a"]},{"operands":["b"],"tag":["t"]}]'"#
        ))
        .is_empty());
        assert_eq!(
            broken(&doc, r#"tool run a --level 2 --input '{"level":3}'"#),
            ["input-inline"]
        );
        assert_eq!(
            broken(&doc, r#"tool run a --input '{"operands":["b"]}'"#),
            ["input-inline"]
        );
        assert_eq!(
            broken(&doc, r#"tool run a --tag w --input '{"tag":["w","y"]}'"#),
            ["input-inline"]
        );
        assert_eq!(
            broken(&doc, r#"tool run --input '{"level":2}'"#),
            ["requirements"],
            "the operand is missing from both"
        );
        assert_eq!(
            broken(&doc, r#"tool run a --input '{"nosuch":1}'"#),
            ["input-inline"]
        );
        assert_eq!(
            broken(&doc, r#"tool run a --input '{"input":"-"}'"#),
            ["input-inline"],
            "input is never a key"
        );
        assert_eq!(
            broken(&doc, r#"tool run a --input '{"dryRun":true,"out":"x"}'"#),
            ["constraints"]
        );
        // 9 is inside the domain of level, so only the merged object (argumentsRef caps level at 5) fails.
        assert_eq!(
            broken(&doc, r#"tool run a --input '{"level":9}'"#),
            ["arguments-ref"]
        );
        assert_eq!(
            broken(&doc, r#"tool run a --input '{"level":10}'"#),
            ["arguments-ref", "input-inline"]
        );
    }

    #[test]
    fn atip_checkl1_invocation_inline_batch_is_bounded_by_max_items() {
        let mut doc = synthetic();
        let two = r#"tool run --input '[{"operands":["a"]},{"operands":["b"]}]'"#;
        assert!(problems(&judged(&doc, two)).is_empty());
        let three =
            r#"tool run --input '[{"operands":["a"]},{"operands":["b"]},{"operands":["c"]}]'"#;
        let results = problems(&judged(&doc, three));
        assert_eq!(results.len(), 1, "{results:?}");
        assert_eq!(results[0].0, "input-inline");
        assert!(results[0].1.contains("over maxItems"));
        let error = r#"tool run --input '[{"operands":["a"]},{"operands":["b"],"level":9}]'"#;
        let results = problems(&judged(&doc, error));
        let at = results
            .iter()
            .find(|(condition, _)| *condition == "arguments-ref")
            .expect("the second item fails argumentsRef");
        assert!(at.1.starts_with("item 1: "), "{results:?}");
        doc.input.as_mut().expect("input").batch = false;
        let results = problems(&judged(&doc, two));
        assert!(
            results
                .iter()
                .any(|(_, why)| why.contains("takes no batch")),
            "{results:?}"
        );
        doc.input.as_mut().expect("input").batch = true;
        let results = problems(&judged(&doc, r#"tool run --input '[]'"#));
        assert!(
            results
                .iter()
                .any(|(condition, _)| *condition == "input-inline"),
            "{results:?}"
        );
    }

    #[test]
    fn atip_checkl1_invocation_inline_input_never_reads_a_file_or_stdin() {
        let doc = synthetic();
        for line in [
            "tool run --input -",
            "tool run --input=- a",
            "tool run a --input file.json",
            "tool run a --input ./{x}.json",
            "tool run a --input=",
            "tool run a --input '\"str\"'",
            "tool run a --input 7",
        ] {
            let results = judged(&doc, line);
            let hits: Vec<_> = results
                .iter()
                .filter(|(condition, problem)| *condition == "input-inline" && problem.is_some())
                .collect();
            assert_eq!(hits.len(), 1, "{line}: {results:?}");
            assert!(
                results.iter().all(|(condition, _)| ![
                    "requirements",
                    "constraints",
                    "arguments-ref"
                ]
                .contains(condition)),
                "{line}: nothing is merged"
            );
        }
        let results = problems(&judged(&doc, "tool run a --input -"));
        assert!(
            results[0].1.contains("must not read stdin or a file"),
            "{results:?}"
        );
        let results = problems(&judged(&doc, "tool run a --input '{nope'"));
        assert!(results[0].1.contains("not JSON"), "{results:?}");
    }

    #[test]
    fn atip_checkl1_invocation_inline_input_needs_a_declared_input() {
        let mut doc = synthetic();
        doc.input = None;
        let results = problems(&judged(&doc, r#"tool run a --input '{"level":1}'"#));
        assert_eq!(results.len(), 1);
        assert!(results[0].1.contains("declares no input"));
    }
}
