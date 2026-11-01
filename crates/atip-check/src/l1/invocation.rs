//! A documented invocation read against a command document (spec 9.12, 10.13 rule 12, 11.5-11.7).
//!
//! OWNER: the invocation agent. The signatures below are the contract the checks are written against.

use std::collections::BTreeMap;

use serde_json::Value;

use super::model::{Arg, CommandDoc};

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

/// Splits a documented command line into argv words (approaches/checker.md 7.1): whitespace separates; `'…'` and `"…"` quote
/// (a backslash escapes `"` and `\` inside double quotes); a backslash outside quotes escapes the next character and a trailing
/// one joins the next line. An unquoted `|`, `>`, `<` that is not part of a `<PLACEHOLDER>`, `;`, `&`, `$(` or a backtick is an error.
pub fn split_line(line: &str) -> Result<Vec<String>, String> {
    let _ = line;
    Ok(Vec::new())
}

/// Reads `words` (an argv without the tool name: tool-wide options, the command path, then the arguments) against `doc` by the
/// grammar of spec 9.12. `Err` names the first word the grammar or the document does not allow.
pub fn parse(doc: &CommandDoc, words: &[String]) -> Result<Invocation, String> {
    let _ = (doc, words);
    Err(String::new())
}

/// The argv of `invocation`: `before`, the path, every option as `--name=value` (`--name` for a flag, once per occurrence), then the
/// operands (after one `--` when any of them starts with `-`).
pub fn render(invocation: &Invocation) -> Vec<String> {
    let _ = invocation;
    Vec::new()
}

/// The typed arguments of `invocation`.
pub fn args(doc: &CommandDoc, invocation: &Invocation) -> Args {
    let _ = (doc, invocation);
    Args::default()
}

/// A string value of an integer, count or number argument as the number it denotes (spec 11.5); anything else is returned as it is.
/// An integer outside the safe range stays a string, so that it is refused and never rounded (spec 10.7.5).
pub fn normalise(value_type: &str, value: &Value) -> Value {
    let _ = value_type;
    value.clone()
}

/// What is wrong with `value` as a value of `arg`: not valid for its valueType after normalisation (spec 10.7.2), or outside its domain.
pub fn value_problem(arg: &Arg, value: &Value) -> Option<String> {
    let _ = (arg, value);
    None
}

/// Missing required arguments and too many operands: the part of spec 11.7 step 6 that is not a constraint.
pub fn requirement_problems(doc: &CommandDoc, args: &Args) -> Vec<String> {
    let _ = (doc, args);
    Vec::new()
}

/// The indices into `doc.constraints` of the rules that `args` violate (spec 10.8.1: "given" means supplied explicitly; values compare after normalisation).
pub fn violated_constraints(doc: &CommandDoc, args: &Args) -> Vec<usize> {
    let _ = (doc, args);
    Vec::new()
}

/// The argument object of 11.7 step 6 that `input.argumentsRef` describes.
pub fn arguments_object(doc: &CommandDoc, args: &Args) -> Value {
    let _ = (doc, args);
    Value::Null
}

/// What `L1.examples` judges of one documented line, as `(condition, problem)` pairs: one pair per condition judged, `None` when it holds.
/// The conditions are `tool-name`, `path`, `grammar`, `no-placeholder`, `values`, `requirements`, `constraints`, `arguments-ref`, `input-inline`.
/// `with_arguments_ref` is `false` for a validator's `rejects` line (spec 10.13 rule 13).
pub fn check_line(
    doc: &CommandDoc,
    tool: &str,
    line: &str,
    with_arguments_ref: bool,
) -> Vec<(&'static str, Option<String>)> {
    let _ = (doc, tool, line, with_arguments_ref);
    Vec::new()
}
