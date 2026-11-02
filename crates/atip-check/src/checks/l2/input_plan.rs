//! The invocations of the `--input` and batch rows (approaches/checker.md 6, L2: `L2.input-refusals` ... `L2.batch-ok`).
//!
//! Every probe is built from a passing invocation of the fixture, split between argv and a document ([`crate::l2::document`]),
//! and every command it runs is a `read` command or a command the fixture lists: a write the fixture does not list is named
//! and never run, so a tool that wrongly accepts a document cannot write through a probe of it.

use serde_json::{json, Value};

use super::super::l1_plan::L1Run;
use super::super::{in_json_mode, Invocations, Run};
use crate::claim::Claim;
use crate::exec::{Call, RunError};
use crate::l1::invocation::{self, Invocation};
use crate::l1::model::{camel, Arg, CommandDoc};
use crate::l1::witness::{self, Expect, GroupOutcome};
use crate::l2::document::{self, Spec, Split, Which};

/// One refusal document the framing probes make: what it is, its text, and what refuses it.
struct Probe {
    label: String,
    text: String,
    code: &'static str,
    reasons: Vec<&'static str>,
    details: Option<usize>,
}

/// The most bytes of padding the size probe sends on stdin.
const MAX_SIZE_PROBE: u64 = 64 * 1024 * 1024;
/// The values a document gives a string argument to show that nothing reads them as an option, an inline document or a file.
const DASH_VALUES: [&str; 4] = ["-x", "{x", "[x", "-"];

/// Everything the `--input` rows read besides the log.
#[derive(Debug, Default)]
pub(crate) struct InputRun {
    /// Each command with `input` that the checker may run.
    pub commands: Vec<InputCommand>,
    /// The write commands with `input` that no fixture entry lists: never run.
    pub unlisted: Vec<String>,
    /// `L1.numbers` at L2: one integer argument given a number beyond the safe range in a document.
    pub numbers: Vec<DocumentNumberRun>,
}

/// A document that must give the result of the argv form.
#[derive(Debug, Clone)]
pub(crate) struct SameRun {
    pub label: String,
    pub call: usize,
    pub reference: usize,
    /// The reference argv run again on a fresh copy of the same state: what differs from `reference` is varying (a timestamp), not compared.
    pub twin: usize,
}

/// A document refused at the framing: the code, the reasons 7.2.3 reserves for the situation, the number of `details` it has.
#[derive(Debug, Clone)]
pub(crate) struct RefusalRun {
    pub label: String,
    pub call: usize,
    pub code: &'static str,
    pub reasons: Vec<&'static str>,
    pub details: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) enum ConflictExpect {
    /// A value of the document differs from argv's: `INPUT_CONFLICT` with a pointer that starts with this one.
    Conflict { pointer: String },
    /// The same value written another way: it succeeds.
    Same,
}

#[derive(Debug, Clone)]
pub(crate) struct ConflictRun {
    pub label: String,
    pub call: usize,
    pub expect: ConflictExpect,
}

/// An option's key given in a document while its `env` variable holds a value the option refuses.
#[derive(Debug, Clone)]
pub(crate) struct EnvRun {
    pub option: String,
    pub variable: String,
    pub call: usize,
    pub reference: usize,
    pub twin: usize,
}

#[derive(Debug, Clone)]
pub(crate) enum PointerExpect {
    /// A constraint is broken: one of these codes and a pointer.
    Constraint { codes: Vec<String> },
    /// A required key is left out of document and argv: `USAGE` or a constraint's code and this pointer.
    Required { pointer: String },
}

#[derive(Debug, Clone)]
pub(crate) struct PointerRun {
    pub label: String,
    pub call: usize,
    pub expect: PointerExpect,
}

#[derive(Debug, Clone)]
pub(crate) struct DashRun {
    pub label: String,
    pub call: usize,
    /// The candidate string argument (by its position in the candidates, in declaration order) and its name, and the value it was given.
    pub group: usize,
    pub argument: String,
    pub value: String,
    /// The candidate is an operand (its pointer is `/operands`), not an option.
    pub operand: bool,
}

/// A batch whose last item breaks a rule that reads no state.
#[derive(Debug, Clone)]
pub(crate) struct PreflightRun {
    pub label: String,
    pub call: usize,
    pub index: usize,
    /// The codes that may answer: `INPUT_INVALID` for a type, a constraint's codes.
    pub codes: Vec<String>,
}

/// A batch of the fixture that stops at `index`.
#[derive(Debug, Clone)]
pub(crate) struct IncompleteRun {
    pub label: String,
    pub call: usize,
    pub items: usize,
    pub index: usize,
}

/// The fixture's `ok` invocations as one batch.
#[derive(Debug, Clone)]
pub(crate) struct OkBatchRun {
    pub call: usize,
    pub items: usize,
}

/// What a command that takes a batch is asked.
#[derive(Debug, Default)]
pub(crate) struct BatchPlan {
    /// The name `meta.output` gives the batch output.
    pub output: Option<String>,
    pub preflights: Vec<PreflightRun>,
    pub incompletes: Vec<IncompleteRun>,
    pub ok: Option<OkBatchRun>,
    /// The fixture entries `L2.batch-incomplete` lacks (`commands.<id>.batchFailsAt` or `batchFailsFirst`).
    pub missing: Vec<String>,
    /// The rules on flag-only arguments alone: an item cannot break them, so no batch probe is built from them.
    pub unbuildable: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct InputCommand {
    pub command: String,
    /// The fixture lists an `ok` invocation of the command that parses against its document.
    pub has_ok: bool,
    pub batch: Option<BatchPlan>,
    pub refusals: Vec<RefusalRun>,
    pub tty: Option<usize>,
    pub usage: Vec<(String, usize)>,
    pub fragments: Vec<SameRun>,
    pub normalise: Vec<SameRun>,
    pub conflicts: Vec<ConflictRun>,
    pub envs: Vec<EnvRun>,
    pub pointers: Vec<PointerRun>,
    pub dashes: Vec<DashRun>,
}

/// One integer argument of a command given a number beyond the safe range in a document, as a JSON number or as a string.
#[derive(Debug, Clone)]
pub(crate) struct DocumentNumberRun {
    pub command: String,
    pub arg: String,
    pub word: String,
    pub form: &'static str,
    pub call: usize,
}

/// The value of a document key a command refuses by type (spec 11.5: no coercion), or `None` when every JSON value is of the type.
fn wrong_type(arg: &Arg) -> Option<Value> {
    let wrong = match arg.value_type.as_str() {
        "integer" | "number" | "count" => json!("not-a-number"),
        "enum" => json!("nosuch-xyz"),
        "string" | "path" => json!(5),
        "switch" => json!("yes"),
        _ => return None,
    };
    Some(if arg.repeatable && arg.value_type != "count" { Value::Array(vec![wrong]) } else { wrong })
}

/// A value of the type of `arg` for a document key: the argv word `word`, normalised.
fn document_value(arg: &Arg, word: &str) -> Value {
    let typed = witness::typed(arg, word);
    let normal = invocation::normalise(&arg.value_type, &typed);
    if arg.repeatable && arg.value_type != "count" {
        Value::Array(vec![normal])
    } else {
        normal
    }
}

/// The argv `before path --input <text> --json`.
fn framing_argv(before: &[String], path: &[String], input: &str) -> Vec<String> {
    before
        .iter()
        .chain(path)
        .cloned()
        .chain(["--input".to_string(), input.to_string(), "--json".to_string()])
        .collect()
}

/// How many of the arguments a rule names are flag-only.
#[derive(Debug, PartialEq, Eq)]
enum FlagOnly {
    None,
    Some,
    All,
}

fn flag_only_kind(doc: &CommandDoc, members: &[usize]) -> FlagOnly {
    let names: Vec<&String> = members.iter().filter_map(|index| doc.constraints.get(*index)).flat_map(|constraint| constraint.names.iter().chain(constraint.if_name.iter()).chain(constraint.then.iter())).collect();
    let flag_only = names.iter().filter(|name| doc.arg(name).is_some_and(|arg| arg.flag_only)).count();
    match flag_only {
        0 => FlagOnly::None,
        count if count == names.len() => FlagOnly::All,
        _ => FlagOnly::Some,
    }
}

/// How many options of the schema declare each environment variable: only an option that is the sole declarer of its variable is probed by
/// `L2.input-env` (`--as` and `--role` share `CANON_ROLE`, so neither is: the variable does not say which of them it fills).
pub(crate) fn env_declarers<'d>(docs: impl Iterator<Item = &'d CommandDoc>) -> std::collections::BTreeMap<String, usize> {
    let mut counts = std::collections::BTreeMap::new();
    for doc in docs {
        for option in &doc.options {
            if let Some(variable) = &option.env {
                *counts.entry(variable.clone()).or_insert(0) += 1;
            }
        }
    }
    counts
}

/// A rule on an argument that a document cannot carry (a flag-only one) is broken on argv, not in a document.
fn on_flag_only(doc: &CommandDoc, members: &[usize]) -> bool {
    members.iter().filter_map(|index| doc.constraints.get(*index)).any(|constraint| {
        constraint.names.iter().chain(constraint.if_name.iter()).chain(constraint.then.iter()).any(|name| doc.arg(name).is_some_and(|arg| arg.flag_only))
    })
}

/// The value the option `name` is given in `base`, as a document value, when it is.
fn given_option(doc: &CommandDoc, base: &Invocation, name: &str) -> Option<Value> {
    invocation::args(doc, base).options.get(name).cloned()
}

impl Run<'_> {
    /// The fixture's `ok` invocations of `doc`, parsed, with the log index of the run the L0 part made of each.
    pub(crate) fn ok_bases(&self, calls: &Invocations, doc: &CommandDoc) -> Vec<(Invocation, usize)> {
        let Some(commands) = &self.fixture.commands else { return Vec::new() };
        commands
            .iter()
            .zip(&calls.commands)
            .find(|((id, _), _)| id.as_str() == doc.command)
            .map(|((_, entry), command)| {
                entry
                    .ok
                    .iter()
                    .zip(&command.ok)
                    .filter_map(|(argv, index)| invocation::parse(doc, &in_json_mode(argv)).ok().map(|parsed| (parsed, *index)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Runs `argv` on a terminal as stdin and stdout, at the fixture's time limit: `L2.input-tty`.
    fn invoke_terminal(&mut self, argv: Vec<String>) -> Result<usize, RunError> {
        let deadline = self.fixture.timeout;
        let mut call = Call::new(argv).on_terminal();
        call.limit = Some(deadline);
        let outcome = self.runner.run_on_terminal(&call, deadline)?;
        self.log.push(outcome);
        Ok(self.log.len() - 1)
    }

    pub(crate) fn plan_input(&mut self, calls: &Invocations, l1: &L1Run, claim: Claim) -> Result<InputRun, RunError> {
        let mut out = InputRun::default();
        let docs: Vec<CommandDoc> = l1.docs.commands.iter().map(|(id, doc)| CommandDoc { command: id.clone(), ..doc.clone() }).collect();
        let declarers = env_declarers(docs.iter());
        for doc in &docs {
            if doc.input.is_none() || self.not_run(&doc.command) {
                continue;
            }
            if !self.runnable(doc) {
                out.unlisted.push(doc.command.clone());
                continue;
            }
            let command = self.plan_input_command(calls, doc, claim, &declarers)?;
            self.plan_document_numbers(&mut out, doc, &command)?;
            out.commands.push(command);
        }
        Ok(out)
    }

    fn plan_input_command(&mut self, calls: &Invocations, doc: &CommandDoc, claim: Claim, declarers: &std::collections::BTreeMap<String, usize>) -> Result<InputCommand, RunError> {
        let bases = self.ok_bases(calls, doc);
        let mut command = InputCommand {
            command: doc.command.clone(),
            has_ok: !bases.is_empty(),
            batch: None,
            refusals: Vec::new(),
            tty: None,
            usage: Vec::new(),
            fragments: Vec::new(),
            normalise: Vec::new(),
            conflicts: Vec::new(),
            envs: Vec::new(),
            pointers: Vec::new(),
            dashes: Vec::new(),
        };
        let before: Vec<String> = bases.first().map(|(base, _)| base.before.clone()).unwrap_or_default();
        self.plan_refusals(&mut command, doc, &before, claim)?;
        let tty = framing_argv(&before, &doc.path, "-");
        command.tty = Some(self.invoke_terminal(tty)?);
        if let Some((base, _)) = bases.first() {
            self.plan_usage(&mut command, doc, base)?;
        }
        self.plan_fragments(&mut command, doc, &bases)?;
        if let Some((base, reference)) = bases.first() {
            self.plan_conflicts(&mut command, doc, base, *reference)?;
            self.plan_envs(&mut command, doc, base, declarers)?;
            self.plan_pointers(&mut command, doc, base)?;
            self.plan_dashes(&mut command, doc, base)?;
        }
        if doc.input.as_ref().is_some_and(|input| input.batch) && !bases.is_empty() {
            command.batch = Some(self.plan_batch(doc, &bases)?);
        }
        Ok(command)
    }

    fn plan_refusals(&mut self, command: &mut InputCommand, doc: &CommandDoc, before: &[String], claim: Claim) -> Result<(), RunError> {
        let Some(input) = doc.input.clone() else { return Ok(()) };
        let mut specs: Vec<Probe> = Vec::new();
        let mut push = |label: &str, text: String, code: &'static str, reasons: &[&'static str], details: Option<usize>| {
            specs.push(Probe { label: label.to_string(), text, code, reasons: reasons.to_vec(), details });
        };
        push("an unknown key", "{\"bogusKey\":1}".to_string(), "INPUT_INVALID", &["UNKNOWN_KEY"], None);
        for key in ["json", "commit"] {
            let flag_only = doc.option(key).is_some_and(|option| option.flag_only);
            let reasons: &[&'static str] = if flag_only { &["FLAG_ONLY_KEY"] } else { &["UNKNOWN_KEY", "FLAG_ONLY_KEY"] };
            push(&format!("the flag-only key {key}"), format!("{{\"{key}\":true}}"), "INPUT_INVALID", reasons, None);
        }
        if let Some(key) = input.example.as_object().and_then(|example| example.keys().next().cloned()) {
            let mut nulled = input.example.clone();
            nulled[&key] = Value::Null;
            push(&format!("the example with {key} null"), nulled.to_string(), "INPUT_INVALID", &["INVALID_VALUE"], None);
        }
        push("empty operands", "{\"operands\":[]}".to_string(), "INPUT_INVALID", &["INVALID_VALUE", "UNKNOWN_KEY"], None);
        if !input.batch {
            push("a batch for a command without batches", "[{}]".to_string(), "INPUT_INVALID", &["BATCH_UNSUPPORTED"], None);
        }
        push("a file that is not there", "./missing-xyz.json".to_string(), "INPUT_INVALID", &["UNREADABLE"], None);
        push("two unknown keys", "{\"bogusOne\":1,\"bogusTwo\":2}".to_string(), "INPUT_INVALID", &["UNKNOWN_KEY"], Some(2));
        if claim.at_least(Claim::V1_2) {
            for key in ["limit", "cursor", "all", "fields"] {
                let Some(option) = doc.option(key) else { continue };
                let value = match option.value_type.as_str() {
                    "switch" => json!(true),
                    "integer" | "count" => json!(1),
                    _ => json!("x"),
                };
                let value = if option.repeatable && option.value_type != "count" { Value::Array(vec![value]) } else { value };
                push(&format!("the key {key}, which 1.2 makes flag-only"), json!({ key: value }).to_string(), "INPUT_INVALID", &["FLAG_ONLY_KEY"], None);
            }
        }
        for Probe { label, text, code, reasons, details } in specs {
            let call = self.invoke(Call::new(framing_argv(before, &doc.path, &text)))?;
            command.refusals.push(RefusalRun { label, call, code, reasons, details });
        }
        if let Some(max) = input.max_bytes.filter(|max| (1..=MAX_SIZE_PROBE).contains(max)) {
            let padding = usize::try_from(max - 1).unwrap_or(0);
            let mut stdin = b"{}".to_vec();
            stdin.extend(std::iter::repeat_n(b' ', padding));
            let call = self.invoke(Call::new(framing_argv(before, &doc.path, "-")).with_stdin(stdin))?;
            command.refusals.push(RefusalRun {
                label: format!("a document one byte over maxBytes ({max})"),
                call,
                code: "INPUT_TOO_LARGE",
                reasons: vec!["TOO_MANY_BYTES"],
                details: None,
            });
        }
        Ok(())
    }

    fn plan_usage(&mut self, command: &mut InputCommand, doc: &CommandDoc, base: &Invocation) -> Result<(), RunError> {
        if base.items.iter().any(|item| matches!(item, invocation::Item::Option { name, .. } if name == "input")) {
            return Ok(());
        }
        let _ = doc;
        let argv = invocation::render(base);
        let twice: Vec<String> = argv.iter().cloned().chain(["--input", "{}", "--input", "{}"].map(String::from)).collect();
        let last: Vec<String> = argv.iter().cloned().chain(["--input".to_string()]).collect();
        let call = self.invoke(Call::new(twice))?;
        command.usage.push(("--input given twice".to_string(), call));
        let call = self.invoke(Call::new(last))?;
        command.usage.push(("--input as the last word".to_string(), call));
        Ok(())
    }

    fn plan_fragments(&mut self, command: &mut InputCommand, doc: &CommandDoc, bases: &[(Invocation, usize)]) -> Result<(), RunError> {
        for (base, reference) in bases.iter().take(2) {
            let twin = self.invoke(Call::new(self.log[*reference].call.argv.clone()))?;
            let splits = [
                ("the operands on argv and the rest in a document", Spec { operands: false, ..Spec::default() }),
                ("every argument in a document", Spec { operands: true, ..Spec::default() }),
                ("the operands in a document and the options on argv", Spec { operands: true, options: Which::Nothing, ..Spec::default() }),
            ];
            let mut seen: Vec<Vec<String>> = Vec::new();
            for (label, spec) in splits {
                let Some(split) = document::split(doc, base, &spec) else { continue };
                if split.document.is_empty() {
                    continue;
                }
                let argv = split.argv();
                if seen.contains(&argv) {
                    continue;
                }
                seen.push(argv.clone());
                let call = self.invoke(Call::new(argv).measured())?;
                command.fragments.push(SameRun { label: label.to_string(), call, reference: *reference, twin });
            }
            // The integers as numeric strings, apart from the complete document of L2.input-fragment: what stays on argv is
            // what the strings are not about.
            let given: Vec<&Arg> = document::given_names(doc, base).iter().filter_map(|name| doc.arg(name)).filter(|arg| arg.value_type == "integer" && !arg.repeatable).collect();
            let variants = [
                ("the integer options as numeric strings, the operands on argv", Spec { operands: false, numeric_strings: true, ..Spec::default() }, given.iter().any(|arg| arg.is_option)),
                ("the integer operands as numeric strings, the options on argv", Spec { operands: true, options: Which::Nothing, numeric_strings: true, ..Spec::default() }, given.iter().any(|arg| !arg.is_option)),
            ];
            for (label, spec, wanted) in variants {
                let Some(split) = wanted.then(|| document::split(doc, base, &spec)).flatten() else { continue };
                let on_argv = split.invocation.items.iter().any(|item| match item {
                    invocation::Item::Operand(_) => true,
                    invocation::Item::Option { name, .. } => doc.option(name).is_some_and(|option| !option.flag_only),
                });
                if !on_argv {
                    continue;
                }
                let call = self.invoke(Call::new(split.argv()).measured())?;
                command.normalise.push(SameRun { label: label.to_string(), call, reference: *reference, twin });
            }
        }
        Ok(())
    }

    /// An invocation like `base` that gives the argument `arg` the argv word `word`, the rest of it still valid.
    fn with_word(doc: &CommandDoc, base: &Invocation, arg: &Arg, word: &str) -> Option<Invocation> {
        if invocation::value_problem(arg, &witness::typed(arg, word)).is_some() {
            return None;
        }
        Self::carrying(doc, std::slice::from_ref(base), arg, word)
    }

    fn plan_conflicts(&mut self, command: &mut InputCommand, doc: &CommandDoc, base: &Invocation, _reference: usize) -> Result<(), RunError> {
        let args = invocation::args(doc, base);
        // A different value in the document for each argument the call gives, the argv form left as it is.
        for name in document::given_names(doc, base).into_iter().take(8) {
            let Some(arg) = doc.arg(&name) else { continue };
            if arg.is_flag() {
                continue;
            }
            let key = camel(&name);
            let (extra_key, value, pointer) = if arg.is_option {
                let current = args.options.get(&name).cloned().unwrap_or(Value::Null);
                let avoid: Vec<Value> = match &current {
                    Value::Array(values) => values.clone(),
                    other => vec![other.clone()],
                };
                let Some(word) = witness::generate(arg, &avoid) else { continue };
                (key.clone(), document_value(arg, &word), format!("/{key}"))
            } else {
                let Some(index) = doc.operands.iter().position(|operand| operand.name == name) else { continue };
                let Some(current) = args.operands.get(index) else { continue };
                let Some(word) = witness::generate(arg, std::slice::from_ref(current)) else { continue };
                let mut operands: Vec<Value> = args.operands.iter().enumerate().map(|(at, value)| {
                    let kind = doc.operands.get(at).or_else(|| doc.operands.last().filter(|operand| operand.variadic)).map_or("string", |operand| operand.value_type.as_str());
                    invocation::normalise(kind, value)
                }).collect();
                operands[index] = invocation::normalise(&arg.value_type, &witness::typed(arg, &word));
                ("operands".to_string(), Value::Array(operands), "/operands".to_string())
            };
            let spec = Spec { operands: false, options: Which::Nothing, extra: vec![(extra_key, value)], ..Spec::default() };
            let Some(split) = document::split(doc, base, &spec) else { continue };
            let call = self.invoke(Call::new(split.argv()))?;
            command.conflicts.push(ConflictRun { label: format!("{name} differs between argv and the document"), call, expect: ConflictExpect::Conflict { pointer } });
        }
        self.plan_spellings(command, doc, base)
    }

    /// The same value written another way: a map with its members in another order, `1.0` for `1`, `-0` for `0`.
    fn plan_spellings(&mut self, command: &mut InputCommand, doc: &CommandDoc, base: &Invocation) -> Result<(), RunError> {
        let mut variants: Vec<(String, Invocation, Spec)> = Vec::new();
        let others = |arg: &Arg| document::names_of(doc).into_iter().filter(|name| *name != arg.name).collect::<Vec<_>>();
        let _ = others;
        for arg in doc.options.iter().filter(|arg| !arg.flag_only && !arg.repeatable) {
            let key = camel(&arg.name);
            let argv_spec = |extra: Vec<(String, String)>| Spec { operands: false, options: Which::Nothing, raw: extra, ..Spec::default() };
            match arg.value_type.as_str() {
                "json" => {
                    let Some(Value::Object(members)) = given_option(doc, base, &arg.name) else { continue };
                    if members.len() < 2 {
                        continue;
                    }
                    let reversed: Vec<String> = members.iter().rev().map(|(name, value)| format!("{}:{}", Value::String(name.clone()), value)).collect();
                    variants.push((format!("--{} with its members in another order", arg.name), base.clone(), argv_spec(vec![(key, format!("{{{}}}", reversed.join(",")))])));
                }
                "number" => {
                    let candidate = Self::with_word(doc, base, arg, "1");
                    let Some(with_one) = candidate else { continue };
                    variants.push((format!("--{} written 1.0 against 1", arg.name), with_one, argv_spec(vec![(key, "1.0".to_string())])));
                }
                "integer" => {
                    let Some(with_zero) = Self::with_word(doc, base, arg, "0") else { continue };
                    variants.push((format!("--{} written -0 against 0", arg.name), with_zero, argv_spec(vec![(key.clone(), "-0".to_string())])));
                    if let Some(minus_zero) = Self::with_word(doc, base, arg, "-0") {
                        variants.push((format!("--{} written 0 against -0", arg.name), minus_zero, argv_spec(vec![(key, "0".to_string())])));
                    }
                }
                _ => {}
            }
        }
        for (label, invocation, spec) in variants {
            let Some(split) = document::split(doc, &invocation, &spec) else { continue };
            let call = self.invoke(Call::new(split.argv()))?;
            command.conflicts.push(ConflictRun { label, call, expect: ConflictExpect::Same });
        }
        Ok(())
    }

    fn plan_envs(&mut self, command: &mut InputCommand, doc: &CommandDoc, base: &Invocation, declarers: &std::collections::BTreeMap<String, usize>) -> Result<(), RunError> {
        for option in doc.options.iter().filter(|option| !option.flag_only && option.env.is_some() && !option.is_flag() && !option.repeatable) {
            let Some(variable) = option.env.clone() else { continue };
            if declarers.get(&variable).copied().unwrap_or(0) != 1 {
                continue;
            }
            let Some(invalid) = Self::invalid_word(option) else { continue };
            let carried = if given_option(doc, base, &option.name).is_some() {
                Some(base.clone())
            } else {
                Self::choose(doc, option, &[]).and_then(|word| Self::with_word(doc, base, option, &word))
            };
            let Some(carried) = carried else { continue };
            let spec = Spec { operands: true, options: Which::Only(vec![option.name.clone()]), ..Spec::default() };
            let Some(split) = document::split(doc, &carried, &spec) else { continue };
            let reference = self.invoke(Call::new(split.argv()))?;
            let twin = self.invoke(Call::new(split.argv()))?;
            let call = self.invoke(Call::new(split.argv()).with_env(&variable, &invalid))?;
            command.envs.push(EnvRun { option: option.name.clone(), variable, call, reference, twin });
        }
        Ok(())
    }

    /// A value the argument refuses, to put in its `env` variable: a wrong type, a value outside its domain.
    fn invalid_word(arg: &Arg) -> Option<String> {
        match arg.value_type.as_str() {
            "integer" | "number" | "count" => return Some("not-a-number".to_string()),
            "enum" => return Some("nosuch-xyz".to_string()),
            "switch" => return None,
            _ => {}
        }
        witness::domain_plan(arg).probes.into_iter().find(|probe| probe.expect == Expect::Refused).map(|probe| probe.word).or_else(|| (arg.value_type == "json").then(|| "{".to_string()))
    }

    fn plan_pointers(&mut self, command: &mut InputCommand, doc: &CommandDoc, base: &Invocation) -> Result<(), RunError> {
        // The operands stay on argv: what a document carries alone is L2.input-fragment's.
        let all = Spec { operands: false, ..Spec::default() };
        for plan in witness::plan_constraints(doc, base).into_iter().take(8) {
            let GroupOutcome::Witness { words } = &plan.outcome else { continue };
            if on_flag_only(doc, &plan.members) {
                continue;
            }
            let Ok(broken) = invocation::parse(doc, &in_json_mode(words)) else { continue };
            let Some(split) = document::split(doc, &broken, &all) else { continue };
            let label = plan.members.iter().filter_map(|index| doc.constraints.get(*index)).map(crate::l1::model::Constraint::label).collect::<Vec<_>>().join(" = ");
            let call = self.invoke(Call::new(split.argv()))?;
            command.pointers.push(PointerRun { label: format!("the document breaks {label}"), call, expect: PointerExpect::Constraint { codes: plan.codes.iter().cloned().collect() } });
        }
        for name in document::given_names(doc, base) {
            let Some(arg) = doc.arg(&name).filter(|arg| arg.required) else { continue };
            let spec = Spec { operands: false, omit: vec![name.clone()], ..Spec::default() };
            let Some(split) = document::split(doc, base, &spec) else { continue };
            let call = self.invoke(Call::new(split.argv()))?;
            let _ = arg;
            command.pointers.push(PointerRun { label: format!("{name} left out of the document and of argv"), call, expect: PointerExpect::Required { pointer: Split::pointer_of(doc, &name) } });
        }
        Ok(())
    }

    fn plan_dashes(&mut self, command: &mut InputCommand, doc: &CommandDoc, base: &Invocation) -> Result<(), RunError> {
        // A string argument that the command does not use to name a stored thing: an option before an operand, one that is not required before one that is.
        let mut candidates: Vec<&Arg> = doc.options.iter().filter(|arg| !arg.flag_only && !arg.repeatable && arg.value_type == "string").collect();
        candidates.sort_by_key(|arg| arg.required);
        candidates.extend(doc.operands.iter().filter(|arg| arg.value_type == "string" && !arg.variadic));
        // Every candidate that admits a dash value, in order (at most four): one whose probe is refused by a precondition cannot tell, and the next is tried.
        let candidates: Vec<&Arg> = candidates.into_iter().filter(|arg| DASH_VALUES.iter().any(|value| invocation::value_problem(arg, &Value::String((*value).to_string())).is_none())).take(4).collect();
        for (group, arg) in candidates.into_iter().enumerate() {
            for value in DASH_VALUES {
                if invocation::value_problem(arg, &Value::String(value.to_string())).is_some() {
                    continue;
                }
                let mut extra: Vec<(String, Value)> = Vec::new();
                let key = if arg.is_option { camel(&arg.name) } else { "operands".to_string() };
                if arg.is_option {
                    extra.push((key, Value::String(value.to_string())));
                } else {
                    let Some(index) = doc.operands.iter().position(|operand| operand.name == arg.name) else { continue };
                    let args = invocation::args(doc, base);
                    let mut operands: Vec<Value> = args.operands.clone();
                    while operands.len() <= index {
                        let Some(filler) = doc.operands.get(operands.len()).and_then(|operand| witness::generate(operand, &[])) else { break };
                        operands.push(Value::String(filler));
                    }
                    if operands.len() <= index {
                        continue;
                    }
                    operands[index] = Value::String(value.to_string());
                    extra.push((key, Value::Array(operands)));
                }
                // The other arguments stay on argv; the value under test is the only one the document carries.
                let spec = Spec { operands: !arg.is_option, options: Which::Nothing, omit: vec![arg.name.clone()], extra, ..Spec::default() };
                let Some(split) = document::split(doc, base, &spec) else { continue };
                let call = self.invoke(Call::new(split.argv()))?;
                command.dashes.push(DashRun { label: format!("{} given the value {value}", arg.name), call, group, argument: arg.name.clone(), value: value.to_string(), operand: !arg.is_option });
            }
        }
        Ok(())
    }

    fn plan_batch(&mut self, doc: &CommandDoc, bases: &[(Invocation, usize)]) -> Result<BatchPlan, RunError> {
        let Some(input) = doc.input.clone() else { return Ok(BatchPlan::default()) };
        let mut plan = BatchPlan { output: doc.outputs.iter().find(|output| output.batch).map(|output| output.name().to_string()), ..BatchPlan::default() };
        let entry = self.fixture.commands.as_ref().and_then(|commands| commands.get(&doc.command)).cloned().unwrap_or_default();
        let anchor_before: Vec<String> = bases.first().map(|(base, _)| base.before.clone()).unwrap_or_default();
        let whole = Spec { operands: true, ..Spec::default() };
        let documents: Vec<Value> = bases
            .iter()
            .filter_map(|(base, _)| document::split(doc, base, &whole).map(|split| Value::Object(split.document)))
            .take(usize::try_from(input.max_items.unwrap_or(8)).unwrap_or(8).max(1))
            .collect();
        let batch_argv = |text: &str, base: Option<&Invocation>| -> Vec<String> {
            match base.and_then(|base| document::split(doc, base, &Spec { operands: true, omit: document::names_of(doc), ..Spec::default() })) {
                Some(split) => split.argv_with(text),
                None => framing_argv(&anchor_before, &doc.path, text),
            }
        };
        let first_base = bases.first().map(|(base, _)| base);
        if !documents.is_empty() {
            let text = Value::Array(documents.clone()).to_string();
            let call = self.invoke(Call::new(batch_argv(&text, first_base)).measured())?;
            plan.ok = Some(OkBatchRun { call, items: documents.len() });
            // A last item that breaks a rule that reads no state: a value of the wrong type, then a broken constraint.
            let last = documents.len();
            let mut type_broken = documents[0].clone();
            let breaker = doc.options.iter().find_map(|arg| {
                (!arg.flag_only && type_broken.get(camel(&arg.name)).is_some()).then(|| wrong_type(arg).map(|value| (camel(&arg.name), value))).flatten()
            });
            if let Some((key, value)) = breaker {
                type_broken[&key] = value;
                let mut items = documents.clone();
                items.push(type_broken);
                let call = self.invoke(Call::new(batch_argv(&Value::Array(items).to_string(), first_base)).measured())?;
                plan.preflights.push(PreflightRun { label: format!("a last item whose {key} has the wrong type"), call, index: last, codes: vec!["INPUT_INVALID".to_string()] });
            }
            if let Some((base, _)) = bases.first() {
                let mut built = 0;
                for group in witness::plan_constraints(doc, base) {
                    let GroupOutcome::Witness { words } = &group.outcome else { continue };
                    let label = group.members.iter().filter_map(|index| doc.constraints.get(*index)).map(crate::l1::model::Constraint::label).collect::<Vec<_>>().join(" = ");
                    let kind = flag_only_kind(doc, &group.members);
                    if kind == FlagOnly::All {
                        // A rule on flag-only arguments alone is broken on argv: no item can break it.
                        plan.unbuildable.push(label);
                        continue;
                    }
                    if built == 2 {
                        continue;
                    }
                    let Ok(broken) = invocation::parse(doc, &in_json_mode(words)) else { continue };
                    let Some(split) = document::split(doc, &broken, &whole) else { continue };
                    built += 1;
                    if kind == FlagOnly::Some {
                        // The flag-only arguments of the rule go on argv, for every item, and the last item carries the rest: alone, so that no
                        // earlier item meets the rule first.
                        let Some(argv_split) = document::split(doc, &broken, &Spec { operands: true, omit: document::names_of(doc), ..Spec::default() }) else { continue };
                        let text = Value::Array(vec![Value::Object(split.document)]).to_string();
                        let call = self.invoke(Call::new(argv_split.argv_with(&text)).measured())?;
                        plan.preflights.push(PreflightRun { label: format!("an item that breaks {label}, with its flag-only arguments on argv"), call, index: 0, codes: group.codes.iter().cloned().collect() });
                        continue;
                    }
                    let mut items = documents.clone();
                    items.push(Value::Object(split.document));
                    let call = self.invoke(Call::new(batch_argv(&Value::Array(items).to_string(), first_base)).measured())?;
                    plan.preflights.push(PreflightRun { label: format!("a last item that breaks {label}"), call, index: last, codes: group.codes.iter().cloned().collect() });
                }
            }
        }
        for (key, cases) in [("batchFailsAt", &entry.batch_fails_at), ("batchFailsFirst", &entry.batch_fails_first)] {
            if cases.is_empty() {
                plan.missing.push(format!("commands.{}.{key}", doc.command));
            }
            for case in cases {
                let text = Value::Array(case.input.clone()).to_string();
                let call = self.invoke(Call::new(batch_argv(&text, first_base)).measured())?;
                plan.incompletes.push(IncompleteRun { label: format!("{key}: item {} of {} fails", case.index, case.input.len()), call, items: case.input.len(), index: case.index });
            }
        }
        Ok(plan)
    }

    /// `L1.numbers` at L2: each integer argument as `9007199254740992` and its negation in a document, as a JSON number and as a string.
    fn plan_document_numbers(&mut self, out: &mut InputRun, doc: &CommandDoc, command: &InputCommand) -> Result<(), RunError> {
        let _ = command;
        let before: Vec<String> = Vec::new();
        for arg in doc.args().filter(|arg| arg.value_type == "integer" && !arg.flag_only && !arg.repeatable && !arg.variadic) {
            for word in ["9007199254740992", "-9007199254740992"] {
                for (form, value) in [("number", word.parse::<serde_json::Number>().map_or(Value::Null, Value::Number)), ("string", Value::String(word.to_string()))] {
                    let document = if arg.is_option {
                        json!({ camel(&arg.name): value })
                    } else {
                        let Some(index) = doc.operands.iter().position(|operand| operand.name == arg.name) else { continue };
                        let mut operands: Vec<Value> = Vec::new();
                        for earlier in 0..index {
                            let Some(filler) = doc.operands.get(earlier).and_then(|operand| witness::generate(operand, &[])) else { break };
                            operands.push(document_value(&doc.operands[earlier], &filler));
                        }
                        if operands.len() != index {
                            continue;
                        }
                        operands.push(value);
                        json!({ "operands": operands })
                    };
                    let call = self.invoke(Call::new(framing_argv(&before, &doc.path, &document.to_string())))?;
                    out.numbers.push(DocumentNumberRun { command: doc.command.clone(), arg: arg.name.clone(), word: word.to_string(), form, call });
                }
            }
        }
        Ok(())
    }
}
