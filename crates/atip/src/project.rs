//! Projecting a clap command tree onto the atip/1 self-description (spec 10;
//! approaches/rust.md section 4).
//!
//! clap parses argv, so clap is the one description of syntax. This module
//! reads it and writes the parts of the documents of [`crate::doc`] that clap
//! knows: the tree and its paths, each option's and operand's `valueType`,
//! typed default, `possibleValues`, `env` and `help`, the `--input` fragment
//! schema, and the contract's argv semantics for a path that does not resolve.
//! What clap cannot say (effect, error codes, outputs, examples) is the tool's
//! table, and the tool fills it in.
//!
//! **Nothing is guessed.** An argument whose value parser is a function of the
//! tool's own (an id kind, a timestamp) has no type clap can read: the tool
//! lists it in a table keyed by the argument's value name, and an argument
//! with such a parser and no entry fails the projection ([`Problem::UnknownParser`]).
//! A default that does not parse as the argument's type fails it too
//! ([`Problem::BadDefault`]). A guessed `string` is how canon once ended up with
//! no option types at all.

use std::any::TypeId;
use std::collections::BTreeMap;
use std::fmt;

use clap::{Arg, ArgAction, Command};
use serde_json::{json, Map, Value};

use crate::doc::{OperandDoc, OptionDoc, ValueType};
use crate::minor::Minor;

/// Why a command tree cannot be projected: a defect of the tool's tables or declarations, found when the document is built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// An argument has a parser of the tool's own and the table has no entry for its value name.
    UnknownParser { command: String, argument: String, value_name: String },
    /// A default value does not parse as the argument's type, or is not one of its possible values.
    BadDefault { command: String, argument: String, default: String, reason: &'static str },
    /// An option or operand has no help text, or a command no summary.
    MissingText { command: String, what: String },
    /// An option has no long name.
    NoLongName { command: String, argument: String },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::UnknownParser { command, argument, value_name } => write!(f, "`{command}`: `{argument}` has a value parser of the tool's own and the table of custom parsers has no entry for its value name `{value_name}`: add one with the value type"),
            Problem::BadDefault { command, argument, default, reason } => write!(f, "`{command}`: the default `{default}` of `{argument}` {reason}"),
            Problem::MissingText { command, what } => write!(f, "`{command}`: {what} has no text"),
            Problem::NoLongName { command, argument } => write!(f, "`{command}`: option `{argument}` has no long name"),
        }
    }
}

impl std::error::Error for Problem {}

/// The table of custom value parsers: `(value name, value type)`.
pub type CustomParsers<'a> = &'a [(&'a str, ValueType)];

// ------------------------------------------------------------------ the tree

/// One node of the command tree below the root.
#[derive(Debug, Clone)]
pub struct Node<'a> {
    /// The words from the root: `["finding", "close"]`.
    pub path: Vec<String>,
    /// The path joined by one space.
    pub command: String,
    pub cmd: &'a Command,
    /// Whether the node has subcommands.
    pub is_group: bool,
    /// The `command` of the direct children, sorted.
    pub children: Vec<String>,
}

/// The subcommands of `cmd` a caller can run: everything but clap's own `help`, sorted by name.
pub fn subcommands(cmd: &Command) -> Vec<&Command> {
    let mut subs: Vec<&Command> = cmd.get_subcommands().filter(|sub| sub.get_name() != "help").collect();
    subs.sort_by(|a, b| a.get_name().cmp(b.get_name()));
    subs
}

/// Every group and command below `root`, a group before its commands, in an order that does not depend on the run.
/// `root` must be built (`Command::build`) so a parent's global options have reached its children.
pub fn nodes(root: &Command) -> Vec<Node<'_>> {
    fn walk<'a>(cmd: &'a Command, prefix: &mut Vec<String>, out: &mut Vec<Node<'a>>) {
        for sub in subcommands(cmd) {
            prefix.push(sub.get_name().to_string());
            let command = prefix.join(" ");
            let children: Vec<String> = subcommands(sub).iter().map(|child| format!("{command} {}", child.get_name())).collect();
            out.push(Node { path: prefix.clone(), command, cmd: sub, is_group: !children.is_empty(), children });
            walk(sub, prefix, out);
            prefix.pop();
        }
    }
    let mut out = Vec::new();
    walk(root, &mut Vec::new(), &mut out);
    out
}

/// The summary of a node: its `about`, which a document must carry.
pub fn summary(node: &Node<'_>) -> Result<String, Problem> {
    match node.cmd.get_about().map(ToString::to_string) {
        Some(text) if !text.trim().is_empty() => Ok(text),
        _ => Err(Problem::MissingText { command: node.command.clone(), what: "the command's summary (its about)".to_string() }),
    }
}

/// A path that does not resolve: where it stops and what is valid there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unknown {
    /// The words that did resolve.
    pub resolved: Vec<String>,
    /// The word that did not.
    pub word: String,
    /// The valid words at that position (empty after a command: it takes no sub-path).
    pub expected: Vec<String>,
    /// The valid words nearest to `word`.
    pub suggestions: Vec<String>,
}

impl Unknown {
    /// One sentence naming the first unknown word.
    pub fn message(&self) -> String {
        let at = |word: &str| if self.resolved.is_empty() { word.to_string() } else { format!("{} {word}", self.resolved.join(" ")) };
        if self.expected.is_empty() {
            format!("`{}` is a command and takes no sub-path `{}`", self.resolved.join(" "), self.word)
        } else {
            format!("unknown command `{}`; valid here: {}", at(&self.word), self.expected.join(", "))
        }
    }
}

/// A path that resolved: the root (empty path), a group or a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    Tree,
    Group(Vec<String>),
    Command(Vec<String>),
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push((prev[j] + usize::from(ca != *cb)).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Resolves `words` (each already one path word) against the tree. There are no aliases: a word is a name or it is unknown (spec 9.10).
pub fn resolve(root: &Command, words: &[String]) -> Result<Resolved, Unknown> {
    let mut cmd = root;
    let mut resolved: Vec<String> = Vec::new();
    for word in words {
        let subs = subcommands(cmd);
        match subs.iter().find(|sub| sub.get_name() == word) {
            Some(next) => {
                resolved.push(next.get_name().to_string());
                cmd = next;
            }
            None => {
                let expected: Vec<String> = subs.iter().map(|sub| sub.get_name().to_string()).collect();
                let limit = (word.chars().count() / 3).max(2);
                let mut near: Vec<(usize, &String)> = expected.iter().map(|name| (distance(word, name), name)).filter(|(d, _)| *d <= limit).collect();
                near.sort();
                let suggestions = near.into_iter().take(3).map(|(_, name)| name.clone()).collect();
                return Err(Unknown { resolved, word: word.clone(), expected, suggestions });
            }
        }
    }
    Ok(match (resolved.is_empty(), subcommands(cmd).is_empty()) {
        (true, _) => Resolved::Tree,
        (false, false) => Resolved::Group(resolved),
        (false, true) => Resolved::Command(resolved),
    })
}

/// The first line of every `--help` of the tool: it points at the self-description (spec 4.7). A tool's
/// root `about` and every group and command keep their own text below it.
pub fn point_help_at_schema(cmd: Command, line: &'static str) -> Command {
    cmd.before_help(line).mut_subcommands(|sub| point_help_at_schema(sub, line))
}

// -------------------------------------------------------------- value types

fn possible_values(arg: &Arg) -> Vec<String> {
    arg.get_possible_values().iter().filter(|value| !value.is_hide_set()).map(|value| value.get_name().to_string()).collect()
}

fn is_builtin(arg: &Arg) -> bool {
    matches!(arg.get_action(), ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong | ArgAction::Version)
}

/// Clap's own `--help` and `--version`, which every command accepts and no document lists (spec 10.5).
pub fn is_builtin_arg(arg: &Arg) -> bool {
    is_builtin(arg)
}

fn display_name(arg: &Arg) -> String {
    arg.get_long().map_or_else(|| arg.get_id().to_string(), |long| format!("--{long}"))
}

/// The placeholder help shows for `arg`.
pub fn value_name(arg: &Arg) -> String {
    arg.get_value_names().and_then(|names| names.first()).map_or_else(|| arg.get_id().as_str().to_uppercase(), ToString::to_string)
}

/// Whether `arg` takes several values: appended once per occurrence, or several in one.
pub fn is_repeatable(arg: &Arg) -> bool {
    matches!(arg.get_action(), ArgAction::Append | ArgAction::Count) || arg.get_num_args().is_some_and(|range| range.max_values() > 1)
}

fn is_integer_type(id: &clap::builder::ValueParser) -> bool {
    let id = id.type_id();
    [TypeId::of::<i8>(), TypeId::of::<i16>(), TypeId::of::<i32>(), TypeId::of::<i64>(), TypeId::of::<isize>(), TypeId::of::<u8>(), TypeId::of::<u16>(), TypeId::of::<u32>(), TypeId::of::<u64>(), TypeId::of::<usize>()].iter().any(|integer| id == *integer)
}

/// The width and signedness of the integer type `arg` parses into, when it is one.
fn integer_width(arg: &Arg) -> Option<(u32, bool)> {
    let id = arg.get_value_parser().type_id();
    let widths: [(TypeId, u32, bool); 10] = [
        (TypeId::of::<i8>(), 8, true),
        (TypeId::of::<i16>(), 16, true),
        (TypeId::of::<i32>(), 32, true),
        (TypeId::of::<i64>(), 64, true),
        (TypeId::of::<isize>(), 64, true),
        (TypeId::of::<u8>(), 8, false),
        (TypeId::of::<u16>(), 16, false),
        (TypeId::of::<u32>(), 32, false),
        (TypeId::of::<u64>(), 64, false),
        (TypeId::of::<usize>(), 64, false),
    ];
    widths.iter().find(|(known, _, _)| id == *known).map(|(_, bits, signed)| (*bits, *signed))
}

/// Whether `arg` parses into one of Rust's integer types.
pub fn is_integer_arg(arg: &Arg) -> bool {
    integer_width(arg).is_some()
}

/// The bounds the Rust integer type of `arg` carries (`u32` is `(Some(0), Some(4294967295))`), within ±(2^53-1); `None` for an argument that is
/// not an integer.
pub fn implicit_bounds(arg: &Arg) -> Option<(Option<i64>, Option<i64>)> {
    let (bits, signed) = integer_width(arg)?;
    Some(match crate::domain::implicit(bits, signed) {
        Some(crate::domain::Domain::Integer { minimum, maximum }) => (minimum, maximum),
        _ => (None, None),
    })
}

/// The restriction the Rust type of an integer argument carries (spec 10.7.4): an unsigned parser refuses a negative
/// value, a narrow one its overflow. clap erases the bounds of `value_parser!(u64).range(1..)`, so a tool's own table
/// says those and the two are merged ([`crate::domain::merge`]); a test holds the table and the parser to one answer.
fn implicit_domain(arg: &Arg, value_type: ValueType) -> Option<Value> {
    if value_type != ValueType::Integer {
        return None;
    }
    let (bits, signed) = integer_width(arg)?;
    crate::domain::implicit(bits, signed).map(|domain| domain.to_value())
}

/// The `valueType` of `arg`, read from its action and its value parser (approaches/rust.md 4.1): a switch and a
/// count by action, an enum by its possible values, an integer or a number by the parser's output type, a path
/// and a string by the built-in parsers. A parser of the tool's own is looked up in `custom` by the argument's value name.
pub fn value_type(command: &str, arg: &Arg, custom: CustomParsers<'_>) -> Result<ValueType, Problem> {
    match arg.get_action() {
        ArgAction::SetTrue | ArgAction::SetFalse => return Ok(ValueType::Switch),
        ArgAction::Count => return Ok(ValueType::Count),
        _ => {}
    }
    if !possible_values(arg).is_empty() {
        return Ok(ValueType::Enum);
    }
    let parser = arg.get_value_parser();
    let id = parser.type_id();
    if is_integer_type(parser) {
        return Ok(ValueType::Integer);
    }
    if id == TypeId::of::<f64>() || id == TypeId::of::<f32>() {
        return Ok(ValueType::Number);
    }
    match format!("{parser:?}").as_str() {
        "ValueParser::string" | "ValueParser::os_string" => return Ok(ValueType::String),
        "ValueParser::path_buf" => return Ok(ValueType::Path),
        _ => {}
    }
    let name = value_name(arg);
    custom.iter().find(|(known, _)| *known == name).map(|(_, value_type)| *value_type).ok_or_else(|| Problem::UnknownParser { command: command.to_string(), argument: display_name(arg), value_name: name })
}

/// Largest integer a JSON parser, JavaScript and a Rust `i64` all read the same (spec 10.7.5).
const SAFE_INTEGER: i64 = 9_007_199_254_740_991;

fn parse_default(command: &str, arg: &Arg, value_type: ValueType, text: &str, members: &[String]) -> Result<Value, Problem> {
    let bad = |reason: &'static str| Problem::BadDefault { command: command.to_string(), argument: display_name(arg), default: text.to_string(), reason };
    match value_type {
        ValueType::Integer => {
            let number: i64 = text.parse().map_err(|_unparsed| bad("is not an integer"))?;
            if number.abs() > SAFE_INTEGER {
                return Err(bad("is outside ±(2^53-1)"));
            }
            Ok(json!(number))
        }
        ValueType::Number => {
            let number: f64 = text.parse().map_err(|_unparsed| bad("is not a number"))?;
            serde_json::Number::from_f64(number).map(Value::Number).ok_or_else(|| bad("is not finite"))
        }
        ValueType::Enum => {
            if members.iter().any(|member| member == text) {
                Ok(json!(text))
            } else {
                Err(bad("is not one of the possible values"))
            }
        }
        ValueType::Json => serde_json::from_str(text).map_err(|_unparsed| bad("is not JSON")),
        ValueType::String | ValueType::Path => Ok(json!(text)),
        ValueType::Switch | ValueType::Count => Ok(Value::Null),
    }
}

/// The typed `default` of `arg` (spec 10.7.3): `false` for a switch, `0` for a count, `null` for a required argument or
/// one with no default, else the clap default parsed with the argument's type (`"100"` is `100`); a repeatable
/// argument's default is an array of its element type.
pub fn typed_default(command: &str, arg: &Arg, value_type: ValueType, required: bool, repeatable: bool) -> Result<Value, Problem> {
    match value_type {
        ValueType::Switch => return Ok(json!(false)),
        ValueType::Count => return Ok(json!(0)),
        _ => {}
    }
    let raw: Vec<String> = arg.get_default_values().iter().map(|value| value.to_string_lossy().into_owned()).collect();
    if required || raw.is_empty() {
        return Ok(Value::Null);
    }
    let members = possible_values(arg);
    let parsed = raw.iter().map(|text| parse_default(command, arg, value_type, text, &members)).collect::<Result<Vec<Value>, Problem>>()?;
    Ok(if repeatable { Value::Array(parsed) } else { parsed.into_iter().next().unwrap_or(Value::Null) })
}

fn help_of(command: &str, arg: &Arg) -> Result<String, Problem> {
    match arg.get_help().map(ToString::to_string) {
        Some(text) if !text.trim().is_empty() => Ok(text),
        _ => Err(Problem::MissingText { command: command.to_string(), what: format!("the help of `{}`", display_name(arg)) }),
    }
}

/// The document of the option `arg` of `command`. `flag_only` lists the long names a document cannot carry.
pub fn option_doc(command: &str, arg: &Arg, custom: CustomParsers<'_>, flag_only: &[&str]) -> Result<OptionDoc, Problem> {
    let Some(long) = arg.get_long() else {
        return Err(Problem::NoLongName { command: command.to_string(), argument: arg.get_id().to_string() });
    };
    let value_type = value_type(command, arg, custom)?;
    let required = arg.is_required_set();
    let repeatable = is_repeatable(arg);
    let valued = !matches!(value_type, ValueType::Switch | ValueType::Count);
    let members = possible_values(arg);
    Ok(OptionDoc {
        name: long.to_string(),
        short: arg.get_short().map(|short| short.to_string()),
        value_name: valued.then(|| value_name(arg)),
        value_type,
        possible_values: (value_type == ValueType::Enum).then_some(members),
        required,
        repeatable,
        default: typed_default(command, arg, value_type, required, repeatable)?,
        default_note: None,
        env: arg.get_env().map(|name| name.to_string_lossy().into_owned()),
        flag_only: flag_only.contains(&long),
        domain: implicit_domain(arg, value_type),
        help: help_of(command, arg)?,
    })
}

/// The options no document can carry in a tool that declares `minor` (spec 11.6): `json`, `input`, `commit` and
/// `plan-digest`, and since 1.2 `limit`, `cursor`, `all` and `fields` too. A tool adds its own flag-only options
/// (canon's `repo` and `as`) to this set before it passes it to [`option_doc`]; the crate does not know their names.
pub const fn flag_only_options(minor: Minor) -> &'static [&'static str] {
    if minor.at_least(Minor::V1_2) {
        &["json", "input", "commit", "plan-digest", "limit", "cursor", "all", "fields"]
    } else {
        &["json", "input", "commit", "plan-digest"]
    }
}

/// What is wrong with the `flagOnly` marks of one command's `options` in a tool that declares `minor`: one line per
/// option of [`flag_only_options`] that the command has and does not mark `flagOnly: true` (spec 11.6, 10.7.1).
pub fn flag_only_problems(options: &[OptionDoc], minor: Minor) -> Vec<String> {
    options
        .iter()
        .filter(|option| flag_only_options(minor).contains(&option.name.as_str()) && !option.flag_only)
        .map(|option| format!("`--{}` is flag-only in a tool that declares {minor} (spec 11.6) but its document does not say `flagOnly: true`", option.name))
        .collect()
}

/// A kebab-case word naming an operand: its clap id without a trailing `_id` or `_ids` (`change_ids` is `change`).
pub fn operand_name(arg: &Arg) -> String {
    let id = arg.get_id().as_str().to_lowercase().replace('_', "-");
    for suffix in ["-ids", "-id"] {
        if let Some(stem) = id.strip_suffix(suffix).filter(|stem| !stem.is_empty()) {
            return stem.to_string();
        }
    }
    id
}

/// The document of the operand `arg` of `command`.
pub fn operand_doc(command: &str, arg: &Arg, custom: CustomParsers<'_>) -> Result<OperandDoc, Problem> {
    let value_type = value_type(command, arg, custom)?;
    let required = arg.is_required_set();
    let variadic = is_repeatable(arg);
    let members = possible_values(arg);
    Ok(OperandDoc {
        name: operand_name(arg),
        value_name: value_name(arg),
        value_type,
        possible_values: (value_type == ValueType::Enum).then_some(members),
        required,
        variadic,
        default: typed_default(command, arg, value_type, required, variadic)?,
        default_note: None,
        domain: implicit_domain(arg, value_type),
        help: help_of(command, arg)?,
    })
}

// ----------------------------------------------------------- input documents

/// The camelCase `--input` key of the option `name` (`resolution-rev` is `resolutionRev`).
pub fn camel_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut upper = false;
    for c in name.chars() {
        match c {
            '-' => upper = true,
            c if upper => {
                out.extend(c.to_uppercase());
                upper = false;
            }
            c => out.push(c),
        }
    }
    out
}

/// A JSON number as a string, per RFC 8259 section 6.
const NUMBER_PATTERN: &str = "^-?(0|[1-9][0-9]*)(\\.[0-9]+)?([eE][+-]?[0-9]+)?$";
/// A JSON integer as a string (spec 11.5).
const INTEGER_PATTERN: &str = "^-?(0|[1-9][0-9]*)$";

/// The encoded form one value of `value_type` takes in an `--input` document (spec 10.7.2, 11.5).
fn encoded(value_type: ValueType, members: Option<&[String]>) -> Value {
    match value_type {
        ValueType::Switch => json!({ "type": "boolean" }),
        ValueType::Count => json!({ "type": "integer", "minimum": 0 }),
        ValueType::String | ValueType::Path => json!({ "type": "string" }),
        ValueType::Integer => json!({ "type": ["integer", "string"], "pattern": INTEGER_PATTERN }),
        ValueType::Number => json!({ "type": ["number", "string"], "pattern": NUMBER_PATTERN }),
        ValueType::Enum => json!({ "enum": members.unwrap_or_default() }),
        ValueType::Json => json!({ "type": ["string", "number", "boolean", "object", "array"] }),
    }
}

fn property(option: &OptionDoc) -> Value {
    let item = encoded(option.value_type, option.possible_values.as_deref());
    let mut schema = if option.repeatable && option.value_type != ValueType::Count { json!({ "type": "array", "items": item }) } else { item };
    schema["description"] = json!(option.help);
    schema
}

fn operands_property(operands: &[OperandDoc]) -> Value {
    let mut schema = json!({ "type": "array", "minItems": 1 });
    let items: Vec<Value> = operands.iter().map(|operand| encoded(operand.value_type, operand.possible_values.as_deref())).collect();
    if let [item] = items.as_slice() {
        schema["items"] = item.clone();
    } else {
        schema["prefixItems"] = Value::Array(items);
    }
    if operands.iter().all(|operand| !operand.variadic) {
        schema["maxItems"] = json!(operands.len());
    }
    schema["description"] = json!(operands.iter().map(|operand| operand.help.as_str()).collect::<Vec<_>>().join("; "));
    schema
}

fn document_properties(operands: &[OperandDoc], options: &[OptionDoc]) -> Map<String, Value> {
    let mut properties = Map::new();
    if !operands.is_empty() {
        properties.insert("operands".to_string(), operands_property(operands));
    }
    for option in options.iter().filter(|option| !option.flag_only) {
        properties.insert(camel_case(&option.name), property(option));
    }
    properties
}

/// The fragment schema of one `--input` document (spec 10.10): every key the command accepts with its accepted
/// encoded values, `additionalProperties: false`, no `required`, and no property for a flag-only option.
pub fn fragment_schema(title: &str, operands: &[OperandDoc], options: &[OptionDoc]) -> Value {
    json!({ "title": title, "type": "object", "properties": document_properties(operands, options), "additionalProperties": false })
}

/// The schema of the argument object a command validates after merging (spec 10.10 `argumentsRef`): the fragment's
/// properties, the keys that are required, and one `oneOf` per set of keys of which exactly one is given
/// (`one_of` holds document keys: `operands` or a camelCase option name). Informative: it is not stricter than the command.
pub fn arguments_schema(title: &str, operands: &[OperandDoc], options: &[OptionDoc], one_of: &[Vec<String>]) -> Value {
    let mut schema = fragment_schema(title, operands, options);
    let mut required: Vec<String> = Vec::new();
    if operands.first().is_some_and(|operand| operand.required) {
        required.push("operands".to_string());
    }
    required.extend(options.iter().filter(|option| option.required && !option.flag_only).map(|option| camel_case(&option.name)));
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    let groups: Vec<Value> = one_of.iter().filter(|group| !group.is_empty()).map(|group| json!({ "oneOf": group.iter().map(|key| json!({ "required": [key] })).collect::<Vec<_>>() })).collect();
    if !groups.is_empty() {
        schema["allOf"] = Value::Array(groups);
    }
    schema
}

// ------------------------------------------------------------- shared schemas

fn rewrite_refs(value: &mut Value, names: &BTreeMap<String, String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(target)) = map.get_mut("$ref") {
                if let Some(def) = target.strip_prefix("#/$defs/") {
                    let name = names.get(def).map_or(def, String::as_str);
                    *target = format!("#/schemas/{name}");
                }
            }
            for (key, child) in map.iter_mut() {
                if key != "$ref" {
                    rewrite_refs(child, names);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_refs(item, names);
            }
        }
        _ => {}
    }
}

/// Adds the JSON Schema `schema` to `schemas` under `name` the way a command document carries it (spec 10.9.2,
/// approaches/rust.md 5): its `$defs` become sibling members, every `#/$defs/<X>` becomes `#/schemas/<X>`, and no
/// member keeps its own `$id` or `$schema`, which would make `#/schemas/..` resolve against that URN. A definition
/// that another schema of the document already holds under the same name and content is shared; one with other content
/// is kept under the schema's name in front of its own (`BriefApproveManyReportSubject`).
pub fn add_schema(schemas: &mut BTreeMap<String, Value>, name: &str, mut schema: Value) -> Result<(), String> {
    let defs: Map<String, Value> = match schema.as_object_mut().and_then(|map| map.remove("$defs")) {
        Some(Value::Object(defs)) => defs,
        _ => Map::new(),
    };
    if let Some(map) = schema.as_object_mut() {
        map.remove("$id");
        map.remove("$schema");
    }
    let mut names: BTreeMap<String, String> = defs.keys().map(|def| (def.clone(), def.clone())).collect();
    // A definition is shared only when it is equal to the one already there once its own references are renamed; a
    // rename changes what other definitions refer to, so the decision repeats until nothing changes.
    for _ in 0..=defs.len() {
        let mut changed = false;
        for (def, body) in &defs {
            let mut renamed = body.clone();
            rewrite_refs(&mut renamed, &names);
            if let Value::Object(map) = &mut renamed {
                map.remove("$id");
                map.remove("$schema");
            }
            let current = names[def].clone();
            let clash = schemas.get(&current).is_some_and(|existing| *existing != renamed) || (current == name);
            if clash && current == *def {
                names.insert(def.clone(), format!("{name}{def}"));
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for (def, body) in defs {
        let mut renamed = body;
        rewrite_refs(&mut renamed, &names);
        let target = names[&def].clone();
        match schemas.get(&target) {
            Some(existing) if *existing == renamed => {}
            Some(_) => return Err(format!("the definition `{def}` of `{name}` clashes with a schema of the same name in the document")),
            None => {
                schemas.insert(target, renamed);
            }
        }
    }
    rewrite_refs(&mut schema, &names);
    if schemas.insert(name.to_string(), schema).is_some() {
        return Err(format!("the schema `{name}` is added twice"));
    }
    Ok(())
}

// -------------------------------------------------------------- relations

/// The relations of a command the way a tool's guide writes them (`a | b`, `[--a --b <V>]`), read from the rules the
/// command document lists as `constraints` (spec 10.8): a guide and `canon schema` say one thing. Only a rule that holds
/// whatever an argument's value is takes part; a rule that depends on a value is a document's alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Relations {
    /// Per name: the names it cannot be given with, in both directions.
    pub conflicts_with: BTreeMap<String, Vec<String>>,
    /// Per name: the names that must be given with it.
    pub requires: BTreeMap<String, Vec<String>>,
    /// Sets of names of which exactly one is required.
    pub required_one_of: Vec<Vec<String>>,
}

impl Relations {
    /// The relations the rules state.
    pub fn from_constraints(rules: &[crate::doc::ConstraintDoc]) -> Relations {
        Relations::from_json(&serde_json::to_value(rules).unwrap_or(Value::Null))
    }

    /// The relations the `constraints` array of a command document states.
    pub fn from_json(rules: &Value) -> Relations {
        let strings = |value: &Value| -> Vec<String> { value.as_array().map(|items| items.iter().filter_map(|item| item.as_str().map(str::to_string)).collect()).unwrap_or_default() };
        let mut found = Relations::default();
        for rule in rules.as_array().map(Vec::as_slice).unwrap_or_default() {
            let names = strings(&rule["names"]);
            let then = strings(&rule["then"]);
            let unconditional = rule["if"].is_object() && rule["if"].get("in").is_none();
            let when = rule["if"]["name"].as_str().unwrap_or_default().to_string();
            match rule["rule"].as_str().unwrap_or_default() {
                kind @ ("exactlyOne" | "atMostOne") => {
                    if kind == "exactlyOne" {
                        found.required_one_of.push(names.clone());
                    }
                    for a in &names {
                        for b in names.iter().filter(|b| *b != a) {
                            found.conflicts_with.entry(a.clone()).or_default().push(b.clone());
                        }
                    }
                }
                "requires" if unconditional => found.requires.entry(when).or_default().extend(then),
                "forbids" if unconditional => {
                    for b in &then {
                        found.conflicts_with.entry(when.clone()).or_default().push(b.clone());
                        found.conflicts_with.entry(b.clone()).or_default().push(when.clone());
                    }
                }
                _ => {}
            }
        }
        for names in found.conflicts_with.values_mut().chain(found.requires.values_mut()) {
            names.sort();
            names.dedup();
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use clap::value_parser;

    use super::*;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_string())
    }

    fn command(arg: Arg) -> Command {
        let mut cmd = Command::new("tool").subcommand(Command::new("run").about("Runs").arg(arg));
        cmd.build();
        cmd
    }

    fn only_arg(cmd: &Command) -> &Arg {
        cmd.find_subcommand("run").and_then(|run| run.get_arguments().find(|arg| !is_builtin(arg))).expect("the test command has one argument")
    }

    #[test]
    fn value_types_come_from_the_action_and_the_parser() {
        let cases: Vec<(Arg, ValueType)> = vec![
            (Arg::new("a").long("a").action(ArgAction::SetTrue).help("h"), ValueType::Switch),
            (Arg::new("a").long("a").action(ArgAction::Count).help("h"), ValueType::Count),
            (Arg::new("a").long("a").value_parser(value_parser!(u64).range(1..)).help("h"), ValueType::Integer),
            (Arg::new("a").long("a").value_parser(value_parser!(i32)).help("h"), ValueType::Integer),
            (Arg::new("a").long("a").value_parser(value_parser!(f64)).help("h"), ValueType::Number),
            (Arg::new("a").long("a").value_parser(value_parser!(std::path::PathBuf)).help("h"), ValueType::Path),
            (Arg::new("a").long("a").value_parser(["x", "y"]).help("h"), ValueType::Enum),
            (Arg::new("a").long("a").help("h"), ValueType::String),
        ];
        for (arg, expected) in cases {
            let cmd = command(arg);
            assert_eq!(value_type("run", only_arg(&cmd), &[]), Ok(expected));
        }
    }

    /// atip.schemadoc.06: an argument with a parser of the tool's own and no table entry fails the projection.
    #[test]
    fn atip_schemadoc_06_a_custom_parser_without_a_table_entry_fails_and_with_one_passes() {
        let cmd = command(Arg::new("who").long("who").value_name("WHO_ID").value_parser(parse_id).help("h"));
        let problem = value_type("run", only_arg(&cmd), &[]).expect_err("an unlisted custom parser is refused");
        assert_eq!(problem, Problem::UnknownParser { command: "run".into(), argument: "--who".into(), value_name: "WHO_ID".into() });
        assert!(problem.to_string().contains("WHO_ID"), "{problem}");
        assert_eq!(value_type("run", only_arg(&cmd), &[("WHO_ID", ValueType::String)]), Ok(ValueType::String));
        assert!(option_doc("run", only_arg(&cmd), &[], &[]).is_err(), "the option document fails the same way");
    }

    /// atip.schemadoc.06: a default that does not parse as its type fails; one that does is typed.
    #[test]
    fn atip_schemadoc_06_a_default_is_parsed_with_its_type_and_one_that_does_not_parse_fails() {
        let typed = command(Arg::new("limit").long("limit").value_parser(value_parser!(u64)).default_value("100").help("h"));
        assert_eq!(option_doc("run", only_arg(&typed), &[], &[]).expect("a typed default").default, json!(100));
        let broken = Arg::new("limit").long("limit").value_parser(value_parser!(u64)).default_value("many").help("h");
        assert!(matches!(typed_default("run", &broken, ValueType::Integer, false, false), Err(Problem::BadDefault { .. })));
        let huge = Arg::new("n").long("n").default_value("9007199254740993").help("h");
        assert!(matches!(typed_default("run", &huge, ValueType::Integer, false, false), Err(Problem::BadDefault { reason: "is outside ±(2^53-1)", .. })));
        let outside = Arg::new("e").long("e").value_parser(["a", "b"]).default_value("c").help("h");
        assert!(matches!(typed_default("run", &outside, ValueType::Enum, false, false), Err(Problem::BadDefault { .. })));
    }

    #[test]
    fn defaults_are_typed_and_a_required_argument_has_none() {
        let cmd = command(Arg::new("tags").long("tag").action(ArgAction::Append).default_value("a").help("h"));
        assert_eq!(option_doc("run", only_arg(&cmd), &[], &[]).expect("repeatable").default, json!(["a"]));
        let required = command(Arg::new("n").long("n").required(true).value_parser(value_parser!(u32)).default_value("3").help("h"));
        assert_eq!(option_doc("run", only_arg(&required), &[], &[]).expect("required").default, Value::Null);
        let flag = command(Arg::new("f").long("f").action(ArgAction::SetTrue).help("h"));
        let doc = option_doc("run", only_arg(&flag), &[], &["f"]).expect("flag");
        assert_eq!((doc.default, doc.value_name, doc.flag_only), (json!(false), None, true));
    }

    #[test]
    fn an_operand_name_is_the_id_without_its_id_suffix() {
        for (id, name) in [("change_id", "change"), ("change_ids", "change"), ("id", "id"), ("dir", "dir"), ("plugin_id", "plugin"), ("run_id", "run")] {
            assert_eq!(operand_name(&Arg::new(id)), name);
        }
    }

    #[test]
    fn help_is_required_of_an_option() {
        let cmd = command(Arg::new("quiet").long("quiet").action(ArgAction::SetTrue));
        assert!(matches!(option_doc("run", only_arg(&cmd), &[], &[]), Err(Problem::MissingText { .. })));
    }

    #[test]
    fn nodes_list_a_group_before_its_commands_in_a_fixed_order() {
        let mut root = Command::new("tool")
            .subcommand(Command::new("zed").about("z"))
            .subcommand(Command::new("finding").about("f").subcommand(Command::new("list").about("l")).subcommand(Command::new("add").about("a")))
            .subcommand(Command::new("alpha").about("a"));
        root.build();
        let listed: Vec<String> = nodes(&root).into_iter().map(|node| node.command).collect();
        assert_eq!(listed, ["alpha", "finding", "finding add", "finding list", "zed"]);
    }

    #[test]
    fn a_path_resolves_to_the_tree_a_group_or_a_command_and_an_unknown_word_names_what_is_valid() {
        let mut root = Command::new("tool").subcommand(Command::new("finding").about("f").subcommand(Command::new("close").about("c")).subcommand(Command::new("add").about("a")));
        root.build();
        let words = |text: &str| -> Vec<String> { text.split_whitespace().map(str::to_string).collect() };
        assert_eq!(resolve(&root, &[]), Ok(Resolved::Tree));
        assert_eq!(resolve(&root, &words("finding")), Ok(Resolved::Group(words("finding"))));
        assert_eq!(resolve(&root, &words("finding close")), Ok(Resolved::Command(words("finding close"))));
        let unknown = resolve(&root, &words("finding clos")).expect_err("a misspelt word is unknown");
        assert_eq!((unknown.expected.clone(), unknown.suggestions.clone()), (words("add close"), words("close")));
        assert!(unknown.message().contains("finding clos"), "{}", unknown.message());
        let past = resolve(&root, &words("finding close now")).expect_err("a command takes no sub-path");
        assert!(past.expected.is_empty() && past.message().contains("takes no sub-path"), "{}", past.message());
    }

    #[test]
    fn the_help_pointer_reaches_every_node() {
        let root = point_help_at_schema(Command::new("tool").about("Root").subcommand(Command::new("g").about("G").subcommand(Command::new("c").about("C"))), "Agents: \"tool schema --json\" describes every command as JSON.");
        for path in [&[][..], &["g"], &["g", "c"]] {
            let mut node = &root;
            for word in path {
                node = node.find_subcommand(word).expect("the node exists");
            }
            let help = node.clone().render_long_help().to_string();
            assert!(help.starts_with("Agents: \"tool schema --json\""), "{help}");
        }
    }

    #[test]
    fn a_fragment_has_no_required_no_flag_only_key_and_encodes_integers_as_numbers_or_digit_strings() {
        let mut root = Command::new("tool").subcommand(
            Command::new("run")
                .about("Runs")
                .arg(Arg::new("target").help("What").required(true))
                .arg(Arg::new("round").long("round").required(true).value_parser(value_parser!(u32)).help("Round"))
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue).help("JSON"))
                .arg(Arg::new("mode").long("mode").value_parser(["a", "b"]).help("Mode"))
                .arg(Arg::new("tag").long("tag").action(ArgAction::Append).help("Tag")),
        );
        root.build();
        let run = root.find_subcommand("run").expect("run");
        let operands: Vec<OperandDoc> = run.get_positionals().map(|arg| operand_doc("run", arg, &[]).expect("operand")).collect();
        let options: Vec<OptionDoc> = run.get_arguments().filter(|arg| !arg.is_positional() && !is_builtin(arg)).map(|arg| option_doc("run", arg, &[], &["json"]).expect("option")).collect();
        let fragment = fragment_schema("run input", &operands, &options);
        assert!(fragment.get("required").is_none(), "{fragment}");
        assert_eq!(fragment["additionalProperties"], json!(false));
        assert!(fragment["properties"].get("json").is_none(), "a flag-only option has no property");
        assert_eq!(fragment["properties"]["round"]["type"], json!(["integer", "string"]));
        assert_eq!(fragment["properties"]["round"]["pattern"], json!(INTEGER_PATTERN));
        assert_eq!(fragment["properties"]["mode"]["enum"], json!(["a", "b"]));
        assert_eq!(fragment["properties"]["tag"]["type"], json!("array"));
        assert_eq!((fragment["properties"]["operands"]["minItems"].clone(), fragment["properties"]["operands"]["maxItems"].clone()), (json!(1), json!(1)));
        let arguments = arguments_schema("run arguments", &operands, &options, &[vec!["mode".to_string(), "tag".to_string()]]);
        assert_eq!(arguments["required"], json!(["operands", "round"]));
        assert_eq!(arguments["allOf"][0]["oneOf"], json!([{ "required": ["mode"] }, { "required": ["tag"] }]));
    }

    #[test]
    fn lifting_a_schema_raises_its_defs_to_siblings_and_drops_its_own_id() {
        let mut schemas = BTreeMap::new();
        let report = json!({ "$schema": "x", "$id": "urn:x", "type": "object", "properties": { "a": { "$ref": "#/$defs/Item" } }, "$defs": { "Item": { "type": "string" } } });
        add_schema(&mut schemas, "ARep", report).expect("added");
        assert_eq!(schemas["ARep"]["properties"]["a"]["$ref"], json!("#/schemas/Item"));
        assert!(schemas["ARep"].get("$id").is_none() && schemas["ARep"].get("$schema").is_none() && schemas["ARep"].get("$defs").is_none());
        assert_eq!(schemas["Item"], json!({ "type": "string" }));
        // The same definition is shared; another one under the same name keeps the schema's name in front of its own.
        add_schema(&mut schemas, "BRep", json!({ "properties": { "b": { "$ref": "#/$defs/Item" } }, "$defs": { "Item": { "type": "string" } } })).expect("shared");
        assert_eq!(schemas["BRep"]["properties"]["b"]["$ref"], json!("#/schemas/Item"));
        add_schema(&mut schemas, "CRep", json!({ "properties": { "c": { "$ref": "#/$defs/Item" } }, "$defs": { "Item": { "type": "integer" } } })).expect("kept apart");
        assert_eq!(schemas["CRep"]["properties"]["c"]["$ref"], json!("#/schemas/CRepItem"));
        assert_eq!(schemas["CRepItem"], json!({ "type": "integer" }));
        assert_eq!(schemas["Item"], json!({ "type": "string" }), "the first definition is untouched");
        assert!(add_schema(&mut schemas, "ARep", json!({})).is_err(), "a schema is added once");
    }

    #[test]
    fn relations_are_read_from_the_rules() {
        let mut root = Command::new("tool").subcommand(
            Command::new("run")
                .about("Runs")
                .arg(Arg::new("a").long("a").action(ArgAction::SetTrue).help("a").conflicts_with("b"))
                .arg(Arg::new("b").long("b").action(ArgAction::SetTrue).help("b"))
                .arg(Arg::new("flow").long("flow").help("flow"))
                .arg(Arg::new("change").long("change").help("change").requires("flow"))
                .group(clap::ArgGroup::new("one").args(["flow", "change"]).required(true).multiple(false)),
        );
        root.build();
        let mut rules = crate::constraint::clap_rules(root.find_subcommand("run").expect("run"));
        rules.push(crate::constraint::Rule::requires("change", &["flow"]).doc());
        let found = Relations::from_constraints(&rules);
        assert_eq!(found.conflicts_with["a"], ["b"]);
        assert_eq!(found.conflicts_with["b"], ["a"]);
        assert_eq!(found.requires["change"], ["flow"]);
        assert_eq!(found.required_one_of, [vec!["flow".to_string(), "change".to_string()]]);
    }

    #[test]
    fn atip_crate12_06_the_flag_only_set_is_four_options_at_1_1_and_eight_at_1_2_and_option_doc_marks_limit() {
        assert_eq!(flag_only_options(Minor::V1_0), ["json", "input", "commit", "plan-digest"]);
        assert_eq!(flag_only_options(Minor::V1_1), ["json", "input", "commit", "plan-digest"]);
        assert_eq!(flag_only_options(Minor::V1_2), ["json", "input", "commit", "plan-digest", "limit", "cursor", "all", "fields"]);
        assert_eq!((flag_only_options(Minor::V1_1).len(), flag_only_options(Minor::V1_2).len()), (4, 8));
        // A tool adds its own flag-only options (canon's `repo` and `as`) to the crate's set; the crate knows none of them.
        let with_own: Vec<&str> = flag_only_options(Minor::V1_2).iter().copied().chain(["repo", "as"]).collect();
        let limit = command(Arg::new("limit").long("limit").value_parser(value_parser!(u32)).help("h"));
        assert!(option_doc("run", only_arg(&limit), &[], &with_own).expect("limit").flag_only, "at 1.2 `limit` is flagOnly: true");
        let repo = command(Arg::new("repo").long("repo").help("h"));
        assert!(option_doc("run", only_arg(&repo), &[], &with_own).expect("repo").flag_only, "the tool's own option is marked too");
        let at_11: Vec<&str> = flag_only_options(Minor::V1_1).to_vec();
        assert!(!option_doc("run", only_arg(&limit), &[], &at_11).expect("limit").flag_only, "at 1.1 `limit` is an ordinary option");
    }

    #[test]
    fn atip_crate12_07_an_option_of_the_set_that_a_document_does_not_mark_flag_only_is_a_problem() {
        let limit = command(Arg::new("limit").long("limit").value_parser(value_parser!(u32)).help("h"));
        let json_flag = command(Arg::new("json").long("json").action(ArgAction::SetTrue).help("h"));
        let name = command(Arg::new("name").long("name").help("h"));
        let unmarked = [
            option_doc("run", only_arg(&limit), &[], &[]).expect("limit"),
            option_doc("run", only_arg(&json_flag), &[], &["json"]).expect("json"),
            option_doc("run", only_arg(&name), &[], &[]).expect("name"),
        ];
        let at_12 = flag_only_problems(&unmarked, Minor::V1_2);
        assert_eq!(at_12.len(), 1, "{at_12:?}");
        assert!(at_12[0].contains("`--limit`") && at_12[0].contains("flagOnly: true"), "{at_12:?}");
        assert_eq!(flag_only_problems(&unmarked, Minor::V1_1), Vec::<String>::new(), "at 1.1 `limit` is an ordinary option");
        // Marked with the 1.2 set, no option is a problem.
        let marked = [option_doc("run", only_arg(&limit), &[], flag_only_options(Minor::V1_2)).expect("limit")];
        assert_eq!(flag_only_problems(&marked, Minor::V1_2), Vec::<String>::new());
    }
}
