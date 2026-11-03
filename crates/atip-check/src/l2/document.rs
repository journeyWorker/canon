//! The `--input` documents the L2 checks build from a passing invocation (spec 11.5-11.7).
//!
//! An invocation of the fixture is split between argv and a document: the options and operands the document takes go
//! to it under their camelCase keys with normalised values (a numeric string is a number, spec 11.5), the flag-only
//! ones (`--json`, `--repo` when the tool marks it) stay on argv. The checks change one thing of the split to build
//! the variant they probe: a numeric string, an argument given both ways, a key left out.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::l1::invocation::{self, Invocation, Item};
use crate::l1::model::{camel, Arg, CommandDoc};

/// The options a split puts in the document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Which {
    /// Every option that is not flag-only.
    #[default]
    All,
    /// Only these (by option name).
    Only(Vec<String>),
    /// None: the operands, or nothing, are the document.
    Nothing,
}

/// How an invocation is split between argv and a document.
#[derive(Debug, Clone, Default)]
pub struct Spec {
    /// The operands go to the document (else they stay on argv).
    pub operands: bool,
    pub options: Which,
    /// An `integer` value is written as its numeric string (spec 11.5).
    pub numeric_strings: bool,
    /// Arguments (by name) left out of both argv and the document.
    pub omit: Vec<String>,
    /// Arguments (by name) that stay on argv although the rule would put them in the document.
    pub argv: Vec<String>,
    /// Document members set after the split, by document key (`operands` for the operands): a value of the document only.
    pub extra: Vec<(String, Value)>,
    /// Document members written as the raw JSON text, by document key: the spelling of a number or the order of a map.
    pub raw: Vec<(String, String)>,
}

/// What is left on argv, and the document.
#[derive(Debug, Clone)]
pub struct Split {
    pub invocation: Invocation,
    pub document: Map<String, Value>,
    raw: Vec<(String, String)>,
}

/// The operand of `doc` that the `index`-th operand word stands for: the last one takes the rest when it is variadic.
fn operand_arg(doc: &CommandDoc, index: usize) -> Option<&Arg> {
    doc.operands.get(index).or_else(|| doc.operands.last().filter(|operand| operand.variadic))
}

fn written(arg: Option<&Arg>, value: &Value, numeric_strings: bool) -> Value {
    let kind = arg.map_or("string", |arg| arg.value_type.as_str());
    let normal = invocation::normalise(kind, value);
    if numeric_strings && kind == "integer" {
        if let Value::Number(number) = &normal {
            return Value::String(number.to_string());
        }
    }
    normal
}

/// Splits `base` by `spec`. `None` when the base reads its arguments from `--input` already.
pub fn split(doc: &CommandDoc, base: &Invocation, spec: &Spec) -> Option<Split> {
    if base.items.iter().any(|item| matches!(item, Item::Option { name, .. } if name == "input")) {
        return None;
    }
    let args = invocation::args(doc, base);
    let omitted = |name: &str| spec.omit.iter().any(|omit| omit == name);
    let on_argv = |name: &str| spec.argv.iter().any(|keep| keep == name);
    let takes_option = |arg: &Arg| {
        !arg.flag_only
            && !on_argv(&arg.name)
            && match &spec.options {
                Which::All => true,
                Which::Only(names) => names.contains(&arg.name),
                Which::Nothing => false,
            }
    };
    let mut invocation = Invocation { before: base.before.clone(), path: base.path.clone(), items: Vec::new() };
    let mut document = Map::new();
    let mut operands: Vec<Value> = Vec::new();
    let mut seen_operands = 0;
    for item in &base.items {
        match item {
            Item::Operand(_) => {
                let index = seen_operands;
                seen_operands += 1;
                let arg = operand_arg(doc, index);
                let name = arg.map_or("", |arg| arg.name.as_str());
                if omitted(name) {
                    continue;
                }
                if spec.operands && !on_argv(name) {
                    let value = args.operands.get(index)?;
                    operands.push(written(arg, value, spec.numeric_strings));
                } else {
                    invocation.items.push(item.clone());
                }
            }
            Item::Option { name, .. } => {
                let arg = doc.option(name);
                if omitted(name) {
                    continue;
                }
                if !arg.is_some_and(takes_option) {
                    invocation.items.push(item.clone());
                }
            }
        }
    }
    if !operands.is_empty() {
        document.insert("operands".to_string(), Value::Array(operands));
    }
    for (name, value) in &args.options {
        let Some(arg) = doc.option(name) else { continue };
        if omitted(name) || !takes_option(arg) {
            continue;
        }
        let value = match value {
            Value::Array(values) if arg.repeatable && arg.value_type != "count" => {
                Value::Array(values.iter().map(|value| written(Some(arg), value, spec.numeric_strings)).collect())
            }
            other => written(Some(arg), other, spec.numeric_strings),
        };
        document.insert(camel(name), value);
    }
    for (key, value) in &spec.extra {
        document.insert(key.clone(), value.clone());
    }
    for (key, _) in &spec.raw {
        document.insert(key.clone(), Value::Null);
    }
    Some(Split { invocation, document, raw: spec.raw.clone() })
}

impl Split {
    /// The document as compact JSON text, the raw members written as they were given.
    pub fn text(&self) -> String {
        let mut document = self.document.clone();
        let mut sentinels: Vec<(String, &str)> = Vec::new();
        for (index, (key, raw)) in self.raw.iter().enumerate() {
            let sentinel = format!("@@raw{index}@@");
            document.insert(key.clone(), Value::String(sentinel.clone()));
            sentinels.push((format!("\"{sentinel}\""), raw));
        }
        let mut text = serde_json::to_string(&Value::Object(document)).unwrap_or_default();
        for (sentinel, raw) in sentinels {
            text = text.replace(&sentinel, raw);
        }
        text
    }

    /// The argv of the call: what stayed on argv and `--input <document>`, the document last when `with_input` holds.
    pub fn argv(&self) -> Vec<String> {
        self.argv_with(&self.text())
    }

    /// The argv of the call with `input` as the value of `--input` (a batch, or a document of the check's own).
    pub fn argv_with(&self, input: &str) -> Vec<String> {
        let mut invocation = self.invocation.clone();
        invocation::set_option(&mut invocation, "input", Some(input.to_string()));
        invocation::render(&invocation)
    }

    /// The JSON pointer a document gives the argument `name` (`/operands` for an operand).
    pub fn pointer_of(doc: &CommandDoc, name: &str) -> String {
        if doc.operands.iter().any(|operand| operand.name == name) {
            "/operands".to_string()
        } else {
            format!("/{}", camel(name))
        }
    }
}

/// The argv of `base` with the single JSON text `input` as `--input`, and only the flag-only options and the words before the path kept.
pub fn argv_for_input(doc: &CommandDoc, base: &Invocation, input: &str) -> Vec<String> {
    let spec = Spec { options: Which::Nothing, omit: names_of(doc), ..Spec::default() };
    match split(doc, base, &spec) {
        Some(split) => split.argv_with(input),
        None => Vec::new(),
    }
}

/// Every operand and option name of `doc` that is not flag-only: the arguments a document can carry.
pub fn names_of(doc: &CommandDoc) -> Vec<String> {
    doc.args().filter(|arg| !arg.flag_only).map(|arg| arg.name.clone()).collect()
}

/// The names of the arguments that `base` gives (operands by their operand name, options by option name), flag-only ones left out.
pub fn given_names(doc: &CommandDoc, base: &Invocation) -> Vec<String> {
    let args = invocation::args(doc, base);
    let mut names: Vec<String> = Vec::new();
    for (index, _) in args.operands.iter().enumerate() {
        if let Some(arg) = operand_arg(doc, index) {
            if !names.contains(&arg.name) {
                names.push(arg.name.clone());
            }
        }
    }
    for name in args.options.keys() {
        if doc.option(name).is_some_and(|arg| !arg.flag_only) {
            names.push(name.clone());
        }
    }
    names
}

/// The set of keys a document of `doc` may carry: the camelCase option names that are not flag-only, and `operands`.
pub fn document_keys(doc: &CommandDoc) -> BTreeSet<String> {
    let mut keys: BTreeSet<String> = doc.options.iter().filter(|option| !option.flag_only).map(|option| camel(&option.name)).collect();
    if !doc.operands.is_empty() {
        keys.insert("operands".to_string());
    }
    keys
}
