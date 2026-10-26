//! `--input <JSON|FILE|->` (0.28 D7): a leaf's arguments as a JSON document.
//!
//! The document is turned into an ordinary command line before clap sees it,
//! so every rule of the flags (required, `requiredOneOf`, value sets, error
//! codes) applies unchanged; [`expand`] is that step and `main` runs it first.
//!
//! - **Source:** inline JSON when the value starts with `{` or `[` after
//!   whitespace, `-` for stdin (a terminal is refused), else a file path.
//! - **Framing:** the whole source is one JSON value first; only if that fails
//!   and the source is a file or stdin are its non-empty lines parsed as NDJSON
//!   (`#` lines skipped). An object is one invocation and prints the ordinary
//!   report; an array or NDJSON is a batch and prints a [`BatchReport`], even
//!   with one element.
//! - **Document:** keys are the leaf's long option names in camelCase,
//!   `operands` an array. Repeatable options are arrays, flags are booleans
//!   (`false` emits `--no-x` when it exists). Every value becomes one
//!   `--kebab=value` element, never shell-parsed. A flag on the command line
//!   and the same key with a different value is `INPUT_CONFLICT`.
//! - **Batch:** every element is validated (clap, then the leaf's own
//!   `--dry-run` where it has one) before the first write. Then the elements
//!   run in order, each as one `canon … --json` child process; the first
//!   failure stops the run (`failed`, the rest `skipped`, exit 1).
//!
//! The same [`Leaf`] description (keys, types, value sets, required) derives
//! the JSON Schema the spec publishes ([`document_schema`]).

use std::ffi::OsString;
use std::io::{IsTerminal, Read};
use std::process::{ExitCode, Stdio};
use std::sync::OnceLock;

use clap::builder::ValueParser;
use clap::{value_parser, Arg, ArgAction, Command};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::cli_error::{CliError, ErrorBody};
use crate::reports::Report;

/// The most bytes one document may hold; larger is refused before it is read further.
pub const MAX_BYTES: u64 = 8 * 1024 * 1024;
/// The most elements one batch may hold.
pub const MAX_ELEMENTS: usize = 10_000;

/// Options a document can never carry: the globals and the safety pair. A
/// document is a command invocation, not a way past a preview.
const FLAGS_ONLY: &[&str] = &["repo", "as", "json", "input", "commit", "plan-digest"];

/// Batch examples of the per-leaf table (D7), by leaf.
const BATCH_EXAMPLES: &[(&str, &str)] = &[
    ("scenario new", r#"[{"operands":["auth.login.01"],"title":"Logs in","case":"happy"},{"operands":["auth.login.02"],"title":"Signs out","case":"happy"}]"#),
    ("evidence add", r#"[{"scenarioId":"auth.login.01","class":"test-run","ref":"npm test","role":"implementer"},{"scenarioId":"auth.login.02","class":"test-run","ref":"npm test","role":"implementer"}]"#),
    ("finding add", r#"[{"changeId":"add-login","round":1,"seq":1,"severity":"note","actorId":"reviewer-1","summary":"Token is logged"},{"changeId":"add-login","round":1,"seq":2,"severity":"blocker","actorId":"reviewer-1","summary":"No rate limit"}]"#),
    ("divergence add", r#"[{"operands":["auth.login.01"],"actorId":"reviewer-1","role":"reviewer"},{"operands":["auth.login.02"],"actorId":"reviewer-1","role":"reviewer"}]"#),
    ("subject new", r#"[{"operands":["payments"],"domain":"dev","title":"Payments"},{"operands":["billing"],"domain":"dev","title":"Billing"}]"#),
];

// ---------------------------------------------------------------- batch report

/// What happened to one element of a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ItemStatus {
    /// The element ran and wrote.
    Written,
    /// The element ran and found its record already present with identical content.
    Unchanged,
    /// The element ran and failed; `error` says why.
    Failed,
    /// An earlier element failed, so this one never ran.
    Skipped,
}

/// One element of a batch, in input order.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchItem<R> {
    /// 0-based position in the input.
    pub index: usize,
    pub status: ItemStatus,
    /// The element's ordinary report (`written`, `unchanged`, and a failure that kept one).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<R>,
    /// The element's ordinary error, `field` a pointer into the batch (`/items/2/scenarioId`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<ErrorBody>")]
    pub error: Option<Value>,
}

/// `Batch<Name>Report`: the `data` of a leaf run with a JSON array or NDJSON as `--input`.
/// `R` is the leaf's ordinary report; the binary itself carries each element's report as JSON.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchReport<R> {
    pub items: Vec<BatchItem<R>>,
}

impl<R: Serialize + JsonSchema> Report for BatchReport<R> {}

/// Whether a leaf takes a batch: every writing leaf with an enveloped report.
/// The destructive leaves bind one plan (O16) and `dashboard` is a stream.
pub fn batch_supported(path: &str) -> bool {
    let writes = crate::schema::meta_for(path).is_some_and(|m| m.effect == crate::schema::Effect::Write);
    let streams = crate::spec_facts::facts_for(path).is_some_and(|f| f.outputs.iter().any(|o| o.kind == crate::spec_facts::OutputKind::Stream));
    writes && !streams
}

// ---------------------------------------------------------------- the leaf

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Flag,
    Count,
    Value,
}

/// One option of a leaf a document can set.
#[derive(Debug, Clone)]
struct Key {
    /// The camelCase document key.
    key: String,
    /// The long option name, without `--`.
    long: String,
    kind: Kind,
    repeatable: bool,
    integer: bool,
    possible: Vec<String>,
    /// `--no-<long>` exists.
    negatable: bool,
    required: bool,
    help: Option<String>,
    default: Vec<String>,
}

/// What a leaf's document can say, read from its clap definition.
#[derive(Debug, Clone)]
pub struct Leaf {
    path: Vec<String>,
    keys: Vec<Key>,
    /// `(min, max)` of the operands, when the leaf has any.
    operands: Option<(usize, Option<usize>)>,
    operand_help: Option<String>,
    operand_names: Vec<String>,
    /// Option groups of which exactly one is required, as document keys.
    one_of: Vec<Vec<String>>,
    has_dry_run: bool,
}

fn camel(long: &str) -> String {
    let mut out = String::with_capacity(long.len());
    let mut upper = false;
    for c in long.chars() {
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

fn is_integer(arg: &Arg) -> bool {
    let id = arg.get_value_parser().type_id();
    [ValueParser::from(value_parser!(u8)), ValueParser::from(value_parser!(u16)), ValueParser::from(value_parser!(u32)), ValueParser::from(value_parser!(u64)), ValueParser::from(value_parser!(usize)), ValueParser::from(value_parser!(i32)), ValueParser::from(value_parser!(i64))]
        .iter()
        .any(|parser| parser.type_id() == id)
}

fn takes_value(arg: &Arg) -> bool {
    !matches!(arg.get_action(), ArgAction::SetTrue | ArgAction::SetFalse | ArgAction::Count | ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong | ArgAction::Version)
}

impl Leaf {
    /// The document description of the leaf `path` whose (built) clap node is `cmd`.
    pub fn of(cmd: &Command, path: &[String]) -> Leaf {
        let args: Vec<&Arg> = cmd.get_arguments().filter(|a| !matches!(a.get_action(), ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong | ArgAction::Version)).collect();
        let keys: Vec<Key> = args
            .iter()
            .filter(|a| !a.is_positional())
            .filter_map(|a| {
                let long = a.get_long()?;
                if FLAGS_ONLY.contains(&long) {
                    return None;
                }
                let kind = match a.get_action() {
                    ArgAction::SetTrue | ArgAction::SetFalse => Kind::Flag,
                    ArgAction::Count => Kind::Count,
                    _ => Kind::Value,
                };
                Some(Key {
                    key: camel(long),
                    long: long.to_string(),
                    kind,
                    repeatable: matches!(a.get_action(), ArgAction::Append),
                    integer: kind == Kind::Value && is_integer(a),
                    possible: a.get_possible_values().iter().filter(|v| !v.is_hide_set()).map(|v| v.get_name().to_string()).collect(),
                    negatable: args.iter().any(|o| o.get_long().is_some_and(|l| l.strip_prefix("no-") == Some(long))),
                    required: a.is_required_set(),
                    help: a.get_help().map(std::string::ToString::to_string),
                    default: if kind == Kind::Value { a.get_default_values().iter().map(|v| v.to_string_lossy().into_owned()).collect() } else { Vec::new() },
                })
            })
            .collect();
        let positionals: Vec<&&Arg> = args.iter().filter(|a| a.is_positional()).collect();
        let operands = (!positionals.is_empty()).then(|| {
            let mut min = 0;
            let mut max = Some(0usize);
            for a in &positionals {
                let range = a.get_num_args();
                let (lo, hi) = range.map_or((1, 1), |r| (r.min_values(), r.max_values()));
                min += if a.is_required_set() { lo.max(1) } else { 0 };
                max = match (max, hi < usize::MAX / 2) {
                    (Some(total), true) => Some(total + hi),
                    _ => None,
                };
            }
            (min, max)
        });
        let by_id = |id: &str| -> Option<String> {
            match args.iter().find(|a| a.get_id() == id) {
                Some(a) if a.is_positional() => Some("operands".to_string()),
                Some(a) => a.get_long().map(camel),
                None => None,
            }
        };
        let one_of = cmd
            .get_groups()
            .filter(|g| g.is_required_set() && !(*g).clone().is_multiple())
            .map(|g| g.get_args().filter_map(|id| by_id(id.as_str())).collect::<Vec<String>>())
            .collect();
        Leaf {
            path: path.to_vec(),
            has_dry_run: keys.iter().any(|k| k.long == "dry-run"),
            keys,
            operands,
            operand_help: positionals.first().and_then(|a| a.get_help()).map(std::string::ToString::to_string),
            operand_names: positionals.iter().flat_map(|a| [a.get_id().to_string(), a.get_value_names().and_then(|v| v.first()).map(std::string::ToString::to_string).unwrap_or_default()]).collect(),
            one_of,
        }
    }

    fn command(&self) -> String {
        format!("canon {}", self.path.join(" "))
    }

    fn key(&self, name: &str) -> Option<&Key> {
        self.keys.iter().find(|k| k.key == name)
    }

    /// The document keys a caller may use, `operands` first.
    fn valid_keys(&self) -> Vec<String> {
        self.operands.iter().map(|_| "operands".to_string()).chain(self.keys.iter().map(|k| k.key.clone())).collect()
    }
}

// ---------------------------------------------------------------- field pointers

/// How the options of the leaf map to document keys, kept for the error
/// renderer: an error about `--scenario-id` that came from a document is
/// about `/scenarioId`.
#[derive(Debug)]
struct DocMap {
    /// `(long option, key)` for every key the command line did not supply.
    fields: Vec<(String, String)>,
    operand_names: Vec<String>,
    /// The command line supplied the operands.
    operands_on_cmdline: bool,
}

impl DocMap {
    fn new(leaf: &Leaf, lexed: &Lexed) -> DocMap {
        let on_cmdline = |long: &str| lexed.opts.iter().any(|o| o.long.as_deref() == Some(long));
        DocMap { fields: leaf.keys.iter().filter(|k| !on_cmdline(&k.long)).map(|k| (k.long.clone(), k.key.clone())).collect(), operand_names: leaf.operand_names.clone(), operands_on_cmdline: !lexed.positionals.is_empty() }
    }

    /// The pointer for an argv name or operand name `field`, under `prefix`.
    fn pointer(&self, prefix: &str, field: &str) -> Option<String> {
        if let Some(long) = field.strip_prefix("--") {
            return self.fields.iter().find(|(l, _)| l == long).map(|(_, key)| format!("{prefix}/{}", escape(key)));
        }
        let name = field.trim_matches(|c| c == '<' || c == '>' || c == '.');
        (!self.operands_on_cmdline && !name.is_empty() && self.operand_names.iter().any(|n| n == name)).then(|| format!("{prefix}/operands"))
    }
}

/// The document being run as one invocation, for [`field_for_body`].
static ACTIVE: OnceLock<DocMap> = OnceLock::new();

/// The `error.field` to print for `field`: an argv name becomes a pointer into
/// the document when the run came from `--input`.
pub fn field_for_body(field: &str) -> std::borrow::Cow<'_, str> {
    match ACTIVE.get().and_then(|map| map.pointer("", field)) {
        Some(pointer) => pointer.into(),
        None => field.into(),
    }
}

/// RFC 6901 escaping of one reference token.
fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

// ---------------------------------------------------------------- the command line

#[derive(Debug)]
struct Opt {
    /// The option's long name when canon's clap node knows it.
    long: Option<String>,
    value: Option<String>,
    /// The tokens this option occupies.
    span: std::ops::Range<usize>,
}

#[derive(Debug, Default)]
struct Lexed {
    opts: Vec<Opt>,
    positionals: Vec<String>,
    /// Index of a bare `--`.
    dashdash: Option<usize>,
}

fn find_long<'a>(cmd: &'a Command, name: &str) -> Option<&'a Arg> {
    cmd.get_arguments().find(|a| a.get_long() == Some(name) || a.get_all_aliases().is_some_and(|aliases| aliases.contains(&name)))
}

fn find_short(cmd: &Command, short: char) -> Option<&Arg> {
    cmd.get_arguments().find(|a| a.get_short() == Some(short) || a.get_all_short_aliases().is_some_and(|aliases| aliases.contains(&short)))
}

/// Reads `tokens` (the command line without the program name) against the leaf `cmd` at `path`.
fn lex(cmd: &Command, path: &[String], tokens: &[String]) -> Lexed {
    let mut out = Lexed::default();
    let mut next_path = 0;
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        if token == "--" {
            out.dashdash = Some(i);
            out.positionals.extend(tokens[i + 1..].iter().cloned());
            break;
        }
        let start = i;
        if let Some(rest) = token.strip_prefix("--") {
            let (name, inline) = match rest.split_once('=') {
                Some((name, value)) => (name, Some(value.to_string())),
                None => (rest, None),
            };
            let arg = find_long(cmd, name);
            let mut value = inline;
            if arg.is_some_and(takes_value) && value.is_none() && i + 1 < tokens.len() {
                value = Some(tokens[i + 1].clone());
                i += 1;
            }
            out.opts.push(Opt { long: arg.and_then(|a| a.get_long()).map(str::to_string), value, span: start..i + 1 });
        } else if token.len() > 1 && token.starts_with('-') {
            let short = token[1..].chars().next().unwrap_or('-');
            let arg = find_short(cmd, short);
            let mut value = (token.len() > 2 && arg.is_some_and(takes_value)).then(|| token[1 + short.len_utf8()..].to_string());
            if arg.is_some_and(takes_value) && value.is_none() && i + 1 < tokens.len() {
                value = Some(tokens[i + 1].clone());
                i += 1;
            }
            out.opts.push(Opt { long: arg.and_then(|a| a.get_long()).map(str::to_string), value, span: start..i + 1 });
        } else if next_path < path.len() && *token == path[next_path] {
            next_path += 1;
        } else {
            out.positionals.push(token.clone());
        }
        i += 1;
    }
    out
}

/// The leaf the command line names and its path, found by walking the
/// subcommand words; options of the node reached so far skip their value.
fn find_leaf<'a>(root: &'a Command, tokens: &[String]) -> Option<(Vec<String>, &'a Command)> {
    let mut node = root;
    let mut path: Vec<String> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        if token == "--" {
            break;
        }
        if let Some(rest) = token.strip_prefix("--") {
            if !rest.contains('=') && find_long(node, rest).is_some_and(takes_value) {
                i += 1;
            }
        } else if token.len() > 1 && token.starts_with('-') {
            let short = token[1..].chars().next().unwrap_or('-');
            if token.len() == 2 && find_short(node, short).is_some_and(takes_value) {
                i += 1;
            }
        } else if let Some(sub) = node.find_subcommand(token) {
            path.push(sub.get_name().to_string());
            node = sub;
        }
        i += 1;
    }
    (node.get_subcommands().next().is_none() && !path.is_empty()).then_some((path, node))
}

// ---------------------------------------------------------------- errors

fn refused(code: &'static str, message: impl Into<String>, hint: impl Into<String>) -> CliError {
    CliError::refused(code, message).hint(hint)
}

const SCHEMA_HINT: &str = "`canon schema <command> --json` shows the document keys under `inputs` with an example";

// ---------------------------------------------------------------- the document

/// The source of a document.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Inline,
    File,
    Stdin,
}

/// A parsed `--input`: one invocation or a batch.
enum Document {
    One(Map<String, Value>),
    Batch(Vec<Value>),
}

fn too_large(what: &str) -> CliError {
    refused("INPUT_TOO_LARGE", format!("{what} exceeds the limit of {MAX_BYTES} bytes or {MAX_ELEMENTS} elements"), "split the batch into several runs, each under the limit")
}

/// Reads at most [`MAX_BYTES`] from `reader`; one byte more is `INPUT_TOO_LARGE`.
fn read_capped(reader: impl Read, what: &str) -> Result<String, CliError> {
    let mut bytes = Vec::new();
    reader.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| refused("INPUT_UNREADABLE", format!("{what} could not be read: {e}"), "check that the file is readable, or pass the document inline"))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(too_large(what));
    }
    String::from_utf8(bytes).map_err(|_| refused("INPUT_INVALID", format!("{what} is not UTF-8 text"), "write the document as UTF-8 JSON"))
}

fn load(value: &str) -> Result<(String, Source), CliError> {
    if value.trim_start().starts_with(['{', '[']) {
        if value.len() as u64 > MAX_BYTES {
            return Err(too_large("the inline document"));
        }
        return Ok((value.to_string(), Source::Inline));
    }
    if value == "-" {
        if std::io::stdin().is_terminal() {
            return Err(refused("INPUT_STDIN_TTY", "`--input -` reads stdin, and stdin is a terminal", "pipe the document in (`… | canon <command> --input -`), or pass it inline or as a file").field("--input"));
        }
        return Ok((read_capped(std::io::stdin().lock(), "stdin")?, Source::Stdin));
    }
    if value.is_empty() {
        return Err(refused("INPUT_INVALID", "`--input` is empty", "pass inline JSON, a file path, or `-` for stdin").field("--input"));
    }
    let file = std::fs::File::open(value).map_err(|e| refused("INPUT_UNREADABLE", format!("`--input {value}` could not be opened: {e}"), "give a readable file path, inline JSON starting with `{` or `[`, or `-` for stdin").field("--input"))?;
    if file.metadata().is_ok_and(|m| m.len() > MAX_BYTES) {
        return Err(too_large(&format!("`{value}`")));
    }
    Ok((read_capped(file, &format!("`{value}`"))?, Source::File))
}

/// Frames `text`: one JSON value first, then (file or stdin only) NDJSON.
fn frame(text: &str, source: Source) -> Result<Document, CliError> {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(object)) => Ok(Document::One(object)),
        Ok(Value::Array(items)) if items.len() > MAX_ELEMENTS => Err(too_large(&format!("the array ({} elements)", items.len()))),
        Ok(Value::Array(items)) => Ok(Document::Batch(items)),
        Ok(_) => Err(refused("INPUT_INVALID", "the document is JSON but not an object or an array of objects", SCHEMA_HINT).field("--input")),
        Err(error) if source == Source::Inline => Err(refused("INPUT_INVALID", format!("the inline document is not valid JSON: {error}"), "fix the JSON, or put one object per line in a file and pass its path (NDJSON)").field("--input")),
        Err(_) => {
            let mut items = Vec::new();
            for (n, line) in text.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if items.len() == MAX_ELEMENTS {
                    return Err(too_large(&format!("the NDJSON input (more than {MAX_ELEMENTS} lines)")));
                }
                match serde_json::from_str::<Value>(line) {
                    Ok(value) => items.push(value),
                    Err(error) => return Err(refused("INPUT_INVALID", format!("the input is neither one JSON value nor NDJSON: line {} is not JSON ({error})", n + 1), "fix the line, or pass a single JSON object or array").field("--input")),
                }
            }
            if items.is_empty() {
                return Err(refused("INPUT_INVALID", "the input holds no JSON value and no NDJSON line", SCHEMA_HINT).field("--input"));
            }
            Ok(Document::Batch(items))
        }
    }
}

// ---------------------------------------------------------------- document -> argv

/// One key of a document as the command line sees it.
struct Setting {
    key: String,
    /// The long option; `None` for `operands`.
    long: Option<String>,
    /// What the option's tokens carry (the flag tokens for a flag).
    values: Vec<String>,
    /// For a flag: the boolean asked for.
    flag: Option<bool>,
    negatable: bool,
    /// A count: `values` holds the total.
    count: bool,
}

fn scalar(value: &Value, pointer: &str, expected: &str) -> Result<String, CliError> {
    match value {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Null => Err(null_refusal(pointer)),
        other => Err(refused("INPUT_INVALID_TYPE", format!("`{pointer}` must be {expected}, not {}", type_name(other)), SCHEMA_HINT).field(pointer).received(other.to_string())),
    }
}

fn null_refusal(pointer: &str) -> CliError {
    refused("INPUT_INVALID_TYPE", format!("`{pointer}` is null: canon has no clear-to-default, so omit the key"), "drop the key to get the option's default").field(pointer)
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// The valid keys nearest to a refused one: the singular of a plural (`scenarioIds` for `scenarioId`) first,
/// then by edit distance.
fn key_suggestions(name: &str, valid: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for singular in [name.strip_suffix("es"), name.strip_suffix('s')].into_iter().flatten() {
        if let Some(key) = valid.iter().find(|k| k.eq_ignore_ascii_case(singular)) {
            out.push(key.clone());
        }
    }
    for near in crate::suggest::nearest(name, valid.iter().map(String::as_str)) {
        if !out.contains(&near) {
            out.push(near);
        }
    }
    out.truncate(3);
    out
}

fn settings(leaf: &Leaf, object: &Map<String, Value>, prefix: &str) -> Result<Vec<Setting>, CliError> {
    let mut out = Vec::with_capacity(object.len());
    for (name, value) in object {
        let pointer = format!("{prefix}/{}", escape(name));
        if name == "operands" && leaf.operands.is_some() {
            let items = match value {
                Value::Array(items) => items,
                Value::Null => return Err(null_refusal(&pointer)),
                other => return Err(refused("INPUT_INVALID_TYPE", format!("`{pointer}` must be an array of operands, not {}", type_name(other)), SCHEMA_HINT).field(pointer).received(other.to_string())),
            };
            let values = items.iter().enumerate().map(|(n, v)| scalar(v, &format!("{pointer}/{n}"), "a string")).collect::<Result<Vec<_>, _>>()?;
            out.push(Setting { key: name.clone(), long: None, values, flag: None, negatable: false, count: false });
            continue;
        }
        let Some(key) = leaf.key(name) else {
            let forbidden = FLAGS_ONLY.iter().any(|f| camel(f) == *name);
            let valid = leaf.valid_keys();
            let message = if forbidden { format!("`{name}` is a command-line flag and cannot be set in a document") } else { format!("`{name}` is not an option of `{}`", leaf.command()) };
            let suggestions = key_suggestions(name, &valid);
            let hint = match (forbidden, suggestions.first()) {
                (true, _) => "pass it as a flag on the command line".to_string(),
                (false, Some(best)) => {
                    let shape = if leaf.key(best).is_some_and(|k| k.repeatable) { "[...]" } else { "..." };
                    format!("use \"{best}\": {shape} (document keys are the option names in camelCase, singular even when the option repeats); {SCHEMA_HINT}")
                }
                (false, None) => SCHEMA_HINT.to_string(),
            };
            return Err(CliError::refused("INPUT_UNKNOWN_FIELD", message).hint(hint).field(pointer).expected(valid.clone()).received(name.clone()).suggestions(suggestions));
        };
        let setting = match (key.kind, value) {
            (_, Value::Null) => return Err(null_refusal(&pointer)),
            (Kind::Flag, Value::Bool(on)) => Setting { key: name.clone(), long: Some(key.long.clone()), values: Vec::new(), flag: Some(*on), negatable: key.negatable, count: false },
            (Kind::Flag, other) => return Err(refused("INPUT_INVALID_TYPE", format!("`{pointer}` is a flag and must be true or false, not {}", type_name(other)), SCHEMA_HINT).field(pointer).received(other.to_string())),
            (Kind::Count, Value::Number(n)) if n.as_u64().is_some() => Setting { key: name.clone(), long: Some(key.long.clone()), values: vec![n.to_string()], flag: None, negatable: false, count: true },
            (Kind::Count, other) => return Err(refused("INPUT_INVALID_TYPE", format!("`{pointer}` must be a count, not {}", type_name(other)), SCHEMA_HINT).field(pointer).received(other.to_string())),
            (Kind::Value, Value::Array(items)) if key.repeatable => {
                let values = items.iter().enumerate().map(|(n, v)| scalar(v, &format!("{pointer}/{n}"), "a string or number")).collect::<Result<Vec<_>, _>>()?;
                Setting { key: name.clone(), long: Some(key.long.clone()), values, flag: None, negatable: false, count: false }
            }
            (Kind::Value, other) if key.repeatable => return Err(refused("INPUT_INVALID_TYPE", format!("`{pointer}` is repeatable and must be an array, not {}", type_name(other)), SCHEMA_HINT).field(pointer).received(other.to_string())),
            (Kind::Value, other) => Setting { key: name.clone(), long: Some(key.long.clone()), values: vec![scalar(other, &pointer, if key.integer { "a number" } else { "a string or number" })?], flag: None, negatable: false, count: false },
        };
        out.push(setting);
    }
    Ok(out)
}

/// The command line a leaf run is built from: the flags given, minus `--input`.
struct Base<'a> {
    kept: Vec<String>,
    /// Where a bare `--` sits in `kept`.
    dashdash: Option<usize>,
    lexed: &'a Lexed,
}

fn conflict(pointer: String, flag: &str, given: &str, document: &str) -> CliError {
    refused("INPUT_CONFLICT", format!("`{flag}` was given as {given} and the document says {document}"), "give the value once: drop the flag or the key, or make them equal").field(pointer)
}

fn cmdline_values<'a>(lexed: &'a Lexed, long: &str) -> Vec<&'a str> {
    lexed.opts.iter().filter(|o| o.long.as_deref() == Some(long)).filter_map(|o| o.value.as_deref()).collect()
}

fn has_flag(lexed: &Lexed, long: &str) -> bool {
    lexed.opts.iter().any(|o| o.long.as_deref() == Some(long))
}

/// The full argument list (after the program name) for one document object.
fn assemble(base: &Base<'_>, settings: &[Setting], prefix: &str) -> Result<Vec<String>, CliError> {
    let mut options: Vec<String> = Vec::new();
    let mut operands: Vec<String> = Vec::new();
    for s in settings {
        let pointer = format!("{prefix}/{}", escape(&s.key));
        let Some(long) = &s.long else {
            if !base.lexed.positionals.is_empty() {
                if base.lexed.positionals != s.values {
                    return Err(conflict(pointer, "operands", &base.lexed.positionals.join(" "), &s.values.join(" ")));
                }
            } else {
                operands.extend(s.values.iter().cloned());
            }
            continue;
        };
        match s.flag {
            Some(on) => {
                let negation = format!("no-{long}");
                let (given_on, given_off) = (has_flag(base.lexed, long), has_flag(base.lexed, &negation));
                if on && given_off || !on && given_on {
                    return Err(conflict(pointer, &format!("--{}", if on { negation } else { long.clone() }), "set", if on { "true" } else { "false" }));
                }
                if on && !given_on {
                    options.push(format!("--{long}"));
                } else if !on && s.negatable && !given_off {
                    options.push(format!("--{negation}"));
                }
            }
            None if s.count => {
                // The command line repeats the flag; the document gives the total.
                let given = base.lexed.opts.iter().filter(|o| o.long.as_deref() == Some(long.as_str())).count();
                let total: usize = s.values[0].parse().unwrap_or(0);
                if given == 0 {
                    options.extend(std::iter::repeat_n(format!("--{long}"), total));
                } else if given != total {
                    return Err(conflict(pointer, &format!("--{long}"), &given.to_string(), &s.values[0]));
                }
            }
            None => {
                let given = cmdline_values(base.lexed, long);
                if !given.is_empty() {
                    if given != s.values.iter().map(String::as_str).collect::<Vec<_>>() {
                        return Err(conflict(pointer, &format!("--{long}"), &given.join(","), &s.values.join(",")));
                    }
                } else {
                    options.extend(s.values.iter().map(|v| format!("--{long}={v}")));
                }
            }
        }
    }
    let mut argv = base.kept.clone();
    let at = base.dashdash.unwrap_or(argv.len());
    argv.splice(at..at, options);
    if !operands.is_empty() {
        if base.dashdash.is_none() {
            argv.push("--".to_string());
        }
        argv.extend(operands);
    }
    Ok(argv)
}

// ---------------------------------------------------------------- expand

/// What `main` does with the command line after [`expand`].
pub enum Expanded {
    /// Parse and run this command line.
    Argv(Vec<OsString>),
    /// The run is over (a refusal or a finished batch).
    Exit(ExitCode),
}

/// Turns `--input` into ordinary arguments, or runs the batch it describes.
/// A command line without `--input`, or naming no leaf that takes it, is
/// returned as it is.
pub fn expand(root: &Command, args: Vec<OsString>) -> Expanded {
    let json = crate::run::entry::argv_wants_json();
    let Some(tokens) = args.iter().skip(1).map(|a| a.to_str().map(str::to_string)).collect::<Option<Vec<String>>>() else {
        return Expanded::Argv(args);
    };
    let Some((path, cmd)) = find_leaf(root, &tokens) else { return Expanded::Argv(args) };
    if find_long(cmd, "input").is_none() {
        return Expanded::Argv(args);
    }
    let lexed = lex(cmd, &path, &tokens);
    let inputs: Vec<&Opt> = lexed.opts.iter().filter(|o| o.long.as_deref() == Some("input")).collect();
    if inputs.is_empty() {
        return Expanded::Argv(args);
    }
    // An option the leaf does not have is clap's to refuse (`USAGE` naming it), as it is without `--input`:
    // reading its value as an operand here would turn the refusal into a conflict with the document.
    let known_elsewhere = |token: &str| match token.strip_prefix("--") {
        Some(rest) => {
            let name = rest.split('=').next().unwrap_or_default();
            matches!(name, "help" | "version") || find_long(root, name).is_some()
        }
        None => matches!(token, "-h" | "-V") || token.chars().nth(1).is_some_and(|c| find_short(root, c).is_some()),
    };
    if lexed.opts.iter().any(|o| o.long.is_none() && !known_elsewhere(&tokens[o.span.start])) {
        return Expanded::Argv(args);
    }
    let leaf = Leaf::of(cmd, &path);
    let command_name = leaf.command();
    let fail = |error: CliError| Expanded::Exit(ExitCode::from(error.emit(&command_name, json) as u8));
    let [input] = inputs.as_slice() else {
        return fail(refused("INPUT_INVALID", "`--input` was given more than once", "give one `--input`; a batch is one array or NDJSON file").field("--input"));
    };
    let Some(value) = input.value.as_deref() else {
        return fail(refused("INPUT_INVALID", "`--input` needs a value", "pass inline JSON, a file path, or `-` for stdin").field("--input"));
    };
    if value == "-" {
        if let Some(other) = lexed.opts.iter().find(|o| o.long.as_deref() != Some("input") && o.value.as_deref().is_some_and(|v| v == "-" || v.ends_with(":-"))) {
            let field = other.long.as_deref().map(|l| format!("--{l}")).unwrap_or_default();
            return fail(refused("INPUT_STDIN_CONFLICT", format!("`--input -` and `{field}` both read stdin"), "read one of them from a file").field(field));
        }
    }
    let (text, source) = match load(value) {
        Ok(loaded) => loaded,
        Err(error) => return fail(error),
    };
    let document = match frame(&text, source) {
        Ok(document) => document,
        Err(error) => return fail(error),
    };
    drop(text);
    if matches!(document, Document::Batch(_)) && !batch_supported(&command_name["canon ".len()..]) {
        return fail(refused("INPUT_BATCH_UNSUPPORTED", format!("`{command_name}` takes one object as `--input`, not an array or NDJSON"), "pass one JSON object; run the command once per plan").field("--input"));
    }

    // The command line without `--input`.
    let mut kept: Vec<String> = Vec::with_capacity(tokens.len());
    let mut dashdash = None;
    for (i, token) in tokens.iter().enumerate() {
        if input.span.contains(&i) {
            continue;
        }
        if lexed.dashdash == Some(i) {
            dashdash = Some(kept.len());
        }
        kept.push(token.clone());
    }
    let base = Base { kept, dashdash, lexed: &lexed };
    let map = DocMap::new(&leaf, &lexed);
    let program = args[0].clone();
    match document {
        Document::One(object) => {
            let argv = match settings(&leaf, &object, "").and_then(|s| assemble(&base, &s, "")) {
                Ok(argv) => argv,
                Err(error) => return fail(error),
            };
            let _ = ACTIVE.set(map);
            Expanded::Argv(std::iter::once(program).chain(argv.into_iter().map(OsString::from)).collect())
        }
        Document::Batch(items) => run_batch(root, &leaf, &base, &map, &lexed, items, json),
    }
}

// ---------------------------------------------------------------- batch

/// A failure of the validation pass: the error with its field already a pointer into the batch.
fn pointed(mut error: CliError, map: &DocMap, prefix: &str) -> CliError {
    error.field = match error.field.as_deref() {
        Some(field) => map.pointer(prefix, field).or(error.field),
        // a refusal about the element as a whole still says which element
        None => Some(prefix.to_string()),
    };
    error
}

/// The error of a child's failure envelope, `field` pointed into the batch.
fn error_of(envelope: &Value, map: &DocMap, prefix: &str) -> CliError {
    let body = &envelope["error"];
    let code = body["code"].as_str().and_then(crate::cli_error::code_info).map_or("INPUT_ELEMENT_FAILED", |info| info.code);
    let mut error = CliError::failed(code, body["message"].as_str().unwrap_or("the element failed without a message")).hint(body["hint"].as_str().unwrap_or("run the element on its own to see its output"));
    error.field = body["field"].as_str().map(str::to_string);
    error.received = body["received"].as_str().map(str::to_string);
    error.expected = body["expected"].as_array().map(|values| values.iter().filter_map(|v| v.as_str().map(str::to_string)).collect());
    error.suggestions = body["suggestions"].as_array().map(|values| values.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    pointed(error, map, prefix)
}

struct Child {
    exit: i32,
    envelope: Option<Value>,
    stderr: String,
}

/// One `canon … --json` child run with no stdin.
fn spawn(exe: &std::path::Path, argv: &[String], extra: &[&str]) -> Child {
    // The extra flags go before a bare `--`, which starts the operands.
    let at = argv.iter().position(|t| t == "--").unwrap_or(argv.len());
    let output = std::process::Command::new(exe).args(&argv[..at]).args(extra).args(&argv[at..]).stdin(Stdio::null()).output();
    match output {
        Ok(out) => Child { exit: out.status.code().unwrap_or(-1), envelope: serde_json::from_slice(&out.stdout).ok(), stderr: String::from_utf8_lossy(&out.stderr).trim().to_string() },
        Err(e) => Child { exit: -1, envelope: None, stderr: format!("could not start canon: {e}") },
    }
}

/// The error of a child that produced no failure document.
fn crashed(child: &Child) -> CliError {
    let message = if child.stderr.is_empty() { format!("the element's run ended with exit {} and no error document", child.exit) } else { format!("the element's run ended with exit {}: {}", child.exit, child.stderr) };
    CliError::failed("INPUT_ELEMENT_FAILED", message).hint("run the element on its own to see its output")
}

fn run_batch(root: &Command, leaf: &Leaf, base: &Base<'_>, map: &DocMap, lexed: &Lexed, items: Vec<Value>, json: bool) -> Expanded {
    let command_name = leaf.command();
    let exit_refused = |error: CliError| Expanded::Exit(ExitCode::from(error.emit(&command_name, json) as u8));
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => return exit_refused(CliError::failed("INPUT_ELEMENT_FAILED", format!("canon cannot find its own executable to run the batch: {e}")).hint("run the elements one by one instead")),
    };
    let wants_json = has_flag(lexed, "json");
    let dry_run_given = has_flag(lexed, "dry-run");

    // Validate every element before the first write.
    let mut argvs: Vec<Vec<String>> = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let prefix = format!("/items/{i}");
        let Some(object) = item.as_object() else {
            return exit_refused(refused("INPUT_INVALID_TYPE", format!("`{prefix}` must be an object, not {}", type_name(item)), SCHEMA_HINT).field(prefix).received(item.to_string()));
        };
        let argv = match settings(leaf, object, &prefix).and_then(|s| assemble(base, &s, &prefix)) {
            Ok(argv) => argv,
            Err(error) => return exit_refused(error),
        };
        let mut parse_argv = vec![root.get_name().to_string()];
        parse_argv.extend(argv.iter().cloned());
        if let Err(error) = root.clone().try_get_matches_from(&parse_argv) {
            let (_, usage) = crate::run::entry::usage_parts(&error, root);
            return exit_refused(pointed(usage, map, &prefix));
        }
        if leaf.has_dry_run && !dry_run_given {
            let child = spawn(&exe, &argv, if wants_json { &["--dry-run"] } else { &["--dry-run", "--json"] });
            let ok = child.exit == 0 && child.envelope.as_ref().is_some_and(|e| e["ok"] == true);
            if !ok {
                let error = match &child.envelope {
                    Some(envelope) if envelope["ok"] == false => error_of(envelope, map, &prefix),
                    _ => crashed(&child),
                };
                return exit_refused(error.with_exit(2));
            }
        }
        argvs.push(argv);
    }

    // Run in order; the first failure stops the run.
    let mut report: Vec<BatchItem<Value>> = Vec::with_capacity(argvs.len());
    let mut failure: Option<CliError> = None;
    for (i, argv) in argvs.iter().enumerate() {
        if failure.is_some() {
            report.push(BatchItem { index: i, status: ItemStatus::Skipped, data: None, error: None });
            continue;
        }
        let child = spawn(&exe, argv, if wants_json { &[] } else { &["--json"] });
        let prefix = format!("/items/{i}");
        match &child.envelope {
            Some(envelope) if envelope["ok"] == true && child.exit == 0 => {
                let status = if envelope["meta"]["unchanged"] == true { ItemStatus::Unchanged } else { ItemStatus::Written };
                report.push(BatchItem { index: i, status, data: Some(envelope["data"].clone()), error: None });
            }
            envelope => {
                let error = match envelope {
                    Some(envelope) if envelope["ok"] == false => error_of(envelope, map, &prefix),
                    _ => crashed(&child),
                };
                report.push(BatchItem { index: i, status: ItemStatus::Failed, data: envelope.as_ref().and_then(|e| e.get("data")).cloned(), error: Some(serde_json::to_value(error.body()).expect("an error body serializes")) });
                failure = Some(error.with_exit(1));
            }
        }
    }
    let all_unchanged = !report.is_empty() && report.iter().all(|item| item.status == ItemStatus::Unchanged);
    if json {
        let data = BatchReport { items: report };
        return Expanded::Exit(ExitCode::from(match &failure {
            Some(error) => error.emit_with_data(&command_name, true, &data) as u8,
            None => {
                if all_unchanged {
                    crate::json_out::mark_unchanged();
                }
                crate::json_out::data(&command_name, &data);
                0
            }
        }));
    }
    for item in &report {
        let status = serde_json::to_value(item.status).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        println!("{command_name}: [{}] {status}", item.index);
    }
    match failure {
        Some(error) => {
            eprintln!("{}", error.human(&command_name));
            Expanded::Exit(ExitCode::from(error.exit as u8))
        }
        None => Expanded::Exit(ExitCode::SUCCESS),
    }
}

// ---------------------------------------------------------------- spec

/// `<Name>Input` for the leaf `path` (`evidence add` → `EvidenceAddInput`).
pub fn schema_name(path: &str) -> String {
    let mut name = String::new();
    for word in path.split([' ', '-']) {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            name.extend(first.to_uppercase());
            name.push_str(chars.as_str());
        }
    }
    name + "Input"
}

fn schema_type(key: &Key) -> Value {
    let item = match key.kind {
        Kind::Flag => json!({ "type": "boolean" }),
        Kind::Count => json!({ "type": "integer", "minimum": 0 }),
        Kind::Value if key.integer => json!({ "type": ["integer", "string"] }),
        Kind::Value if !key.possible.is_empty() => json!({ "enum": key.possible }),
        Kind::Value => json!({ "type": ["string", "number"] }),
    };
    if key.repeatable {
        json!({ "type": "array", "items": item })
    } else {
        item
    }
}

/// The JSON Schema (2020-12) of one document object of the leaf, derived from its clap definition.
pub fn document_schema(leaf: &Leaf) -> Value {
    let mut properties = Map::new();
    if let Some((min, max)) = leaf.operands {
        let mut operands = json!({ "type": "array", "items": { "type": ["string", "number"] }, "minItems": min });
        if let Some(max) = max {
            operands["maxItems"] = json!(max);
        }
        if let Some(help) = &leaf.operand_help {
            operands["description"] = json!(help);
        }
        properties.insert("operands".to_string(), operands);
    }
    for key in &leaf.keys {
        let mut property = schema_type(key);
        if let Some(help) = &key.help {
            property["description"] = json!(help);
        }
        if !key.default.is_empty() && !key.repeatable {
            property["default"] = if key.integer { key.default[0].parse::<u64>().map_or_else(|_| json!(key.default[0]), |n| json!(n)) } else { json!(key.default[0]) };
        }
        properties.insert(key.key.clone(), property);
    }
    let mut required: Vec<&str> = leaf.keys.iter().filter(|k| k.required).map(|k| k.key.as_str()).collect();
    if leaf.operands.is_some_and(|(min, _)| min > 0) {
        required.insert(0, "operands");
    }
    let mut schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": format!("`{}` --input document", leaf.command()),
        "type": "object",
        "properties": properties,
        "additionalProperties": false,
    });
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    let one_of: Vec<Value> = leaf.one_of.iter().filter(|g| !g.is_empty()).map(|group| json!({ "oneOf": group.iter().map(|k| json!({ "required": [k] })).collect::<Vec<_>>() })).collect();
    if !one_of.is_empty() {
        schema["allOf"] = Value::Array(one_of);
    }
    schema
}

/// Splits an example line like a shell would for the examples canon ships:
/// whitespace separates, double quotes group.
fn words(line: &str) -> Vec<String> {
    let (mut out, mut current, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut current));
                    any = false;
                }
            }
            c => {
                current.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(current);
    }
    out
}

/// The document equivalent to the leaf's example command line: its options by
/// document key, its operands as `operands`; the globals are left out.
pub fn example_document(cmd: &Command, leaf: &Leaf) -> Option<Value> {
    let line = crate::schema::example_for(&leaf.path.join(" "))?;
    let tokens: Vec<String> = words(line).into_iter().skip(1).collect();
    let lexed = lex(cmd, &leaf.path, &tokens);
    let mut object = Map::new();
    if !lexed.positionals.is_empty() {
        object.insert("operands".to_string(), json!(lexed.positionals));
    }
    for opt in &lexed.opts {
        let Some(key) = opt.long.as_deref().and_then(|long| leaf.keys.iter().find(|k| k.long == long)) else { continue };
        let value = match (key.kind, &opt.value) {
            (Kind::Flag, _) => json!(true),
            (Kind::Count, _) => json!(lexed.opts.iter().filter(|o| o.long.as_deref() == Some(key.long.as_str())).count()),
            (Kind::Value, Some(v)) => match v.parse::<i64>() {
                Ok(n) if key.integer => json!(n),
                _ => json!(v),
            },
            (Kind::Value, None) => continue,
        };
        match (key.repeatable, object.get_mut(&key.key)) {
            (true, Some(Value::Array(values))) => values.push(value),
            (true, _) => {
                object.insert(key.key.clone(), json!([value]));
            }
            (false, _) => {
                object.insert(key.key.clone(), value);
            }
        }
    }
    Some(Value::Object(object))
}

/// One input of a leaf in the spec (`inputs[]`).
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InputDoc {
    /// The option that carries it.
    pub option: &'static str,
    /// `#/schemas/<Name>Input`: the JSON Schema of one document object.
    pub schema_ref: String,
    /// The most bytes one document may hold.
    pub max_bytes: u64,
    /// The most elements one batch may hold.
    pub max_elements: usize,
    /// An array or NDJSON runs the leaf once per element; false for a leaf that takes one object.
    pub batch: bool,
    /// A document that runs, equivalent to the leaf's example command line.
    pub example: Value,
    /// A batch document of the per-leaf table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_example: Option<Value>,
    /// The JSON Schema itself, published once under `schemas`.
    #[serde(skip)]
    pub schema: Value,
}

/// The inputs of the leaf `path` whose (built) clap node is `cmd`: one for a
/// leaf that takes `--input`, none otherwise.
pub fn inputs(path: &str, cmd: &Command) -> Vec<InputDoc> {
    if find_long(cmd, "input").is_none() {
        return Vec::new();
    }
    let words: Vec<String> = path.split(' ').map(str::to_string).collect();
    let leaf = Leaf::of(cmd, &words);
    vec![InputDoc {
        option: "--input",
        schema_ref: format!("#/schemas/{}", schema_name(path)),
        max_bytes: MAX_BYTES,
        max_elements: MAX_ELEMENTS,
        batch: batch_supported(path),
        example: example_document(cmd, &leaf).unwrap_or_else(|| json!({})),
        batch_example: BATCH_EXAMPLES.iter().find(|(p, _)| *p == path).map(|(_, text)| serde_json::from_str(text).expect("the batch example is JSON")),
        schema: document_schema(&leaf),
    }]
}
