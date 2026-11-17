//! What `tests/arg_rules.rs` drives the declared constraints, domains and validators with (0.34 `canon-0-34-atip-constraints`).
//!
//! - [`search`] finds, for one rule, a set of arguments that breaks that rule and no other (or satisfies it while
//!   exercising it), by looking at every set of the arguments the rules name, and reads a command line against a document (spec 9.12);
//! - [`calls`] keeps what the search finds as data (0.42), which the sweeps read;
//! - [`Provision`] is a repo the command's example runs in (the story of `tests/harness`), copied for every call;
//! - [`run_call`] runs a call through the built binary and reads the envelope: the exit, the code, the field and whatever
//!   the call wrote.
//!
//! Every function takes the documents the generator builds as `serde_json::Value`, so a seeded wrong document is as
//! easy to test as the real one.

#![expect(dead_code, reason = "each test binary that includes this module uses a subset")]

use crate::hermetic;

use std::path::{Path, PathBuf};

use atip::constraint::Given;
use atip::doc::{ConstraintDoc, RuleKind};
use serde_json::{json, Value};
use tempfile::TempDir;

use super::harness::{self, canon_in, copy_dir, tree};
use search::{argument, arguments, constraints, normal, parse_call, sample, Assignment, Parsed};

pub mod calls;
pub mod search;
pub mod sites;
mod samples_authoring;
mod samples_evidence;
mod samples_reads;
mod sites_authoring;
mod sites_evidence;
mod sites_reads;

// ------------------------------------------------------------------------------------------------ the documents

/// Every command document of the spec, in tree order.
pub fn docs() -> Vec<&'static Value> {
    harness::leaves()
}

pub fn is_operand(doc: &Value, name: &str) -> bool {
    doc["operands"].as_array().into_iter().flatten().any(|operand| operand["name"] == name)
}

/// A readable name of a rule, for messages.
pub fn label(rule: &ConstraintDoc) -> String {
    serde_json::to_string(rule).unwrap_or_default()
}

/// What a call breaks of everything a document says without reading state: a required argument that is missing, a
/// value outside its domain, a rule. Empty when the call is one the document accepts.
pub fn document_problems(doc: &Value, words: &[String]) -> Vec<String> {
    let parsed = match parse_call(doc, words) {
        Ok(parsed) => parsed,
        Err(problem) => return vec![problem],
    };
    let mut problems = Vec::new();
    for argument in arguments(doc) {
        let name = argument["name"].as_str().unwrap_or_default();
        if argument["required"] == true && !parsed.given.contains_key(name) {
            problems.push(format!("`{name}` is required and missing"));
        }
        if let (Some(domain), Some(values)) = (argument.get("domain"), parsed.given.get(name)) {
            for value in values {
                if !atip::domain::admits(domain, value).unwrap_or(false) {
                    problems.push(format!("{value} is outside the domain of `{name}`: {domain}"));
                }
            }
        }
        if let Some(values) = parsed.given.get(name) {
            if argument["valueType"] == "enum" {
                let members: Vec<&str> = argument["possibleValues"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                for value in values {
                    if !value.as_str().is_some_and(|text| members.contains(&text)) {
                        problems.push(format!("{value} is not a possible value of `{name}`"));
                    }
                }
            }
        }
    }
    for (at, field) in atip::constraint::violations(&constraints(doc), &parsed.given) {
        problems.push(format!("breaks {} (field {field})", label(&constraints(doc)[at])));
    }
    problems
}

/// The words of a call (without `canon`, without `--json`): the command path, the operands in order, then the options.
/// An argument the assignment does not name keeps the value the example gives when it is required, else is left out.
pub fn words_of(doc: &Value, example: &[String], assignment: &Assignment) -> Vec<String> {
    let mut words: Vec<String> = doc["path"].as_array().into_iter().flatten().filter_map(|word| word.as_str().map(str::to_string)).collect();
    let mut given = assignment.clone();
    for argument in arguments(doc) {
        let name = argument["name"].as_str().unwrap_or_default().to_string();
        if argument["required"] == true && !given.contains_key(&name) {
            given.insert(name, vec![sample(doc, example, argument)]);
        }
    }
    let mut operand_words: Vec<String> = Vec::new();
    for operand in doc["operands"].as_array().into_iter().flatten() {
        if let Some(values) = given.get(operand["name"].as_str().unwrap_or_default()) {
            operand_words.extend(values.iter().cloned());
        }
    }
    let mut option_words: Vec<String> = Vec::new();
    for option in doc["options"].as_array().into_iter().flatten() {
        let name = option["name"].as_str().unwrap_or_default();
        let Some(values) = given.get(name) else { continue };
        for value in values {
            if matches!(option["valueType"].as_str(), Some("switch" | "count")) {
                option_words.push(format!("--{name}"));
            } else {
                option_words.push(format!("--{name}={value}"));
            }
        }
    }
    if operand_words.iter().any(|word| word.starts_with('-')) {
        // An operand that starts with a dash is one only after `--`.
        words.extend(option_words);
        words.push("--".to_string());
        words.extend(operand_words);
    } else {
        words.extend(operand_words);
        words.extend(option_words);
    }
    words
}

// ------------------------------------------------------------------------------------------------ running

/// A repo a command's example runs in (the story of `tests/harness`, with whatever state the command needs), kept
/// as a template that every call copies.
pub struct Provision {
    pub template: PathBuf,
    pub envs: Vec<(String, String)>,
    /// The example's command line (without `canon`, with `--json`), as the story writes it.
    pub example: Vec<String>,
    _case: harness::Case,
}

/// The provisioned repo of the command `path`, built from the fixture at `base`.
pub fn provision(path: &str, base: &Path, home: &Path) -> Provision {
    let case = harness::case(path, base, home);
    // A writer refreshes a stale scenario index before it reads its own arguments, and that is the writer's doing, not the
    // refusal's: the template starts with the index current, so a refused call is seen to write nothing.
    let envs: Vec<(&str, &str)> = case.envs.iter().map(|(key, value)| (key.as_str(), value.as_str())).chain(std::iter::once(("CANON_ROLE", "implementer"))).collect();
    let _ = canon_in(&case.dir, home, &["inventory".to_string(), "sync".to_string(), "--json".to_string()], &envs);
    Provision { template: case.dir.clone(), envs: case.envs.clone(), example: case.argv.clone(), _case: case }
}

/// What a call did.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub exit: i32,
    pub ok: bool,
    pub code: Option<String>,
    pub field: Option<String>,
    pub reason: Option<String>,
    pub message: Option<String>,
    /// The paths the call changed in the repo.
    pub changed: Vec<PathBuf>,
    pub stdout: String,
}

/// Runs `words` (a command line without `canon`) with `--json` in a copy of the provisioned repo.
pub fn run_call(provision: &Provision, home: &Path, words: &[String], role: Option<&str>) -> Outcome {
    let scratch = TempDir::new().expect("a scratch directory");
    let repo = scratch.path().join("repo");
    copy_dir(&provision.template, &repo);
    let before = tree(&repo);
    let mut argv = words.to_vec();
    argv.push("--json".to_string());
    let mut envs: Vec<(&str, &str)> = provision.envs.iter().map(|(key, value)| (key.as_str(), value.as_str())).collect();
    if let Some(role) = role {
        envs.push(("CANON_ROLE", role));
    }
    let output = canon_in(&repo, home, &argv, &envs);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let document: Value = serde_json::from_str(&stdout).unwrap_or(Value::Null);
    Outcome {
        exit: output.status.code().unwrap_or(-1),
        ok: document["ok"] == true,
        code: document["error"]["code"].as_str().map(str::to_string),
        field: document["error"]["field"].as_str().map(str::to_string),
        reason: document["error"]["reason"].as_str().map(str::to_string),
        message: document["error"]["message"].as_str().map(str::to_string),
        changed: harness::changed(&before, &tree(&repo)),
        stdout,
    }
}

/// The role a call declares (`CANON_ROLE`): an agent role, except for the human's own acts, which refuse one
/// (`ACTOR_DENIED`) unless the call relays the human's answer.
pub fn declared_role(path: &str, assignment: &Assignment) -> Option<&'static str> {
    match path {
        "brief approve" | "policy approve" | "divergence accept" | "migrate" | "change defer" | "change drop" | "change reopen" if !assignment.contains_key("relayed-via") => None,
        _ => Some("implementer"),
    }
}

// ------------------------------------------------------------------------------------------- the parser, in process

/// Whether clap alone (no handler, no repo) refuses `words` (a command line without `canon`), and what it names:
/// `Err(text)` holds the argument clap blames and its message.
pub fn clap_refusal(words: &[String]) -> Option<(String, String)> {
    use clap::error::ContextKind;
    static ROOT: std::sync::LazyLock<clap::Command> = std::sync::LazyLock::new(|| {
        let mut root = canon_cli::cli::command();
        root.build();
        root
    });
    let argv = std::iter::once("canon".to_string()).chain(words.iter().cloned());
    match ROOT.clone().try_get_matches_from(argv) {
        Ok(_) => None,
        Err(error) => {
            let blamed = error.get(ContextKind::InvalidArg).map_or_default(ToString::to_string);
            Some((blamed, error.to_string()))
        }
    }
}

/// The arguments a document's call gives, as an assignment: what the call typed.
pub fn assignment_of(parsed: &Parsed) -> Assignment {
    parsed.text.clone()
}

/// The explicitly given arguments of an assignment, normalised (spec 11.5).
pub fn given_from(doc: &Value, assignment: &Assignment) -> Given {
    assignment
        .iter()
        .map(|(name, texts)| {
            let values = texts.iter().map(|text| argument(doc, name).map_or_else(|| json!(text), |argument| if argument["valueType"] == "switch" { json!(true) } else { normal(argument, text) })).collect();
            (name.clone(), values)
        })
        .collect()
}

/// Mends an assignment until the rules hold, never touching `keep` (the argument under test): a missing name that a
/// `requires` or an `atLeastOne` wants is added, one a `forbids` or an `atMostOne` refuses is dropped.
pub fn repair(doc: &Value, example: &[String], rules: &[ConstraintDoc], assignment: &mut Assignment, keep: &str) -> Result<(), String> {
    for _ in 0..12 {
        let given = given_from(doc, assignment);
        let broken = atip::constraint::violations(rules, &given);
        let Some((at, _)) = broken.first() else { return Ok(()) };
        let rule = &rules[*at];
        let names = rule.names.clone().unwrap_or_default();
        let then = rule.then.clone().unwrap_or_default();
        match rule.rule {
            RuleKind::Requires | RuleKind::AtLeastOne | RuleKind::ExactlyOne if given.is_empty() || rule.rule != RuleKind::ExactlyOne || names.iter().all(|name| !given.contains_key(name)) => {
                let wanted: Vec<String> = if rule.rule == RuleKind::Requires { then } else { names.iter().take(1).cloned().collect() };
                for name in wanted {
                    let argument = argument(doc, &name).ok_or_else(|| format!("`{name}` is not an argument"))?;
                    assignment.entry(name).or_insert_with(|| vec![sample(doc, example, argument)]);
                }
            }
            RuleKind::Forbids => {
                let victims: Vec<String> = then.iter().filter(|name| *name != keep).cloned().collect();
                if victims.is_empty() {
                    if let Some(when) = &rule.when {
                        // The argument that triggers the rule, when the command cannot do without it (`query --kind`), takes a value the rule does not list.
                        let trigger = argument(doc, &when.name);
                        let listed: Vec<String> = when.values.iter().flatten().map(|value| value.as_str().map_or_else(|| value.to_string(), str::to_string)).collect();
                        let outside = trigger.filter(|trigger| trigger["required"] == true).and_then(|trigger| trigger["possibleValues"].as_array()).and_then(|members| members.iter().filter_map(Value::as_str).find(|member| !listed.iter().any(|value| value == member)));
                        match outside {
                            Some(member) => {
                                assignment.insert(when.name.clone(), vec![member.to_string()]);
                            }
                            None => {
                                assignment.remove(&when.name);
                            }
                        }
                    }
                }
                for victim in victims {
                    assignment.remove(&victim);
                }
            }
            _ => {
                // atMostOne, or an exactlyOne with several given: keep the one under test, else the first.
                let mut kept = false;
                for name in &names {
                    if !given.contains_key(name) {
                        continue;
                    }
                    if (name == keep && !kept) || (!kept && !names.contains(&keep.to_string())) {
                        kept = true;
                    } else {
                        assignment.remove(name);
                    }
                }
            }
        }
    }
    Err(format!("the rules of `{}` do not settle for {assignment:?}", doc["command"]))
}

/// Runs `words` (a command line without `canon`) with `--json` in an EMPTY directory with an empty HOME and no `setup`: no repo,
/// no `canon.yaml`, nothing to read. A validator refuses here with its own code, before any precondition or state read
/// (spec 10.8.3, 11.7 step 6).
pub fn run_empty(words: &[String], role: Option<&str>) -> Outcome {
    let envs: Vec<(&str, &str)> = role.map(|role| ("CANON_ROLE", role)).into_iter().collect();
    run_empty_with(words, &envs)
}

/// [`run_empty`] with the variables of `envs` set (and no others).
pub fn run_empty_with(words: &[String], envs: &[(&str, &str)]) -> Outcome {
    let scratch = TempDir::new().expect("a scratch directory");
    let home = TempDir::new().expect("an empty home");
    let before = tree(scratch.path());
    let mut argv = words.to_vec();
    argv.push("--json".to_string());
    // No ambient `CANON_*` at all: the environment is an input of a validator (spec 11.7), and the checker's own is empty.
    let mut command = hermetic::canon();
    command.args(&argv).current_dir(scratch.path()).env_clear().env("HOME", home.path()).env("PATH", std::env::var_os("PATH").unwrap_or_default());
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().expect("spawn the canon binary");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let document: Value = serde_json::from_str(&stdout).unwrap_or(Value::Null);
    Outcome {
        exit: output.status.code().unwrap_or(-1),
        ok: document["ok"] == true,
        code: document["error"]["code"].as_str().map(str::to_string),
        field: document["error"]["field"].as_str().map(str::to_string),
        reason: document["error"]["reason"].as_str().map(str::to_string),
        message: document["error"]["message"].as_str().map(str::to_string),
        changed: harness::changed(&before, &tree(scratch.path())),
        stdout,
    }
}
