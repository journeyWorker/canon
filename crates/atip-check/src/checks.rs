//! The L0 checks (approaches/checker.md 6, "L0").
//!
//! A run first makes every invocation the L0 table names, each in its own
//! fresh copy of the fixture state, and keeps what each one did in a log.
//! Each check then reads the log: the ones tied to one invocation read that
//! invocation, the ones the table scopes to "every invocation" or "every
//! envelope" read all of it. Which invocations a check reads is fixed here so
//! that one broken rule fails one check: a rule that two checks both state in
//! the table is judged by the check that owns it, and the other check reads
//! only what it needs to compare.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use crate::exec::{Call, Exit, Outcome, RunError, Runner};
use crate::fixture::{Fixture, RegistryRow};
use crate::report::{Check, CheckResult, Report, Strength};
use crate::schema::schemas;

/// One entry of the L0 table: the id, whether it is MUST or SHOULD, and the contract rules it enforces.
struct Entry {
    id: &'static str,
    strength: Strength,
    rules: &'static [&'static str],
}

const fn must(id: &'static str, rules: &'static [&'static str]) -> Entry {
    Entry { id, strength: Strength::Must, rules }
}

const fn should(id: &'static str, rules: &'static [&'static str]) -> Entry {
    Entry { id, strength: Strength::Should, rules }
}

/// The L0 table, in the order of approaches/checker.md. `L0.ok-exit` also names 8.2, the rule that a refusal changes nothing, which it judges.
const TABLE: [Entry; 16] = [
    must("L0.version", &["4.6"]),
    should("L0.version-format", &["4.6"]),
    must("L0.unknown-command", &["4.4", "5.1", "5.2", "6.4", "6.6", "6.11", "7.2"]),
    must("L0.unknown-command-text", &["5.3"]),
    should("L0.text-error-form", &["7.4"]),
    must("L0.bare", &["7.2", "9.2"]),
    must("L0.unknown-option", &["4.4", "6.6", "9.9"]),
    must("L0.json-mode-preparse", &["4.4"]),
    must("L0.after-dashdash", &["4.4"]),
    must("L0.ok-exit", &["6.2", "6.4", "7.3.2", "7.3.3", "8.1", "8.2"]),
    must("L0.no-null", &["6.5"]),
    should("L0.compact", &["5.4"]),
    must("L0.one-document", &["5.1"]),
    must("L0.no-env-switch", &["4.3"]),
    must("L0.no-env-switch-text", &["4.3"]),
    should("L0.contract", &["6.6"]),
];

/// The ids of the L0 checks, in table order.
pub fn ids() -> [&'static str; 16] {
    TABLE.each_ref().map(|entry| entry.id)
}

/// The word no tool has as a command.
const NO_SUCH_COMMAND: &str = "nosuch-xyz";
/// The word no command has as an option.
const NO_SUCH_OPTION: &str = "--bogus-xyz";
/// The variations of the environment under which the output kind must not change (spec 4.3), each a variable and its value.
const ENV_VARIATIONS: [(&str, &str); 5] = [("CI", "1"), ("CLAUDECODE", "1"), ("OUTPUT_FORMAT", "text"), ("NO_COLOR", "1"), ("TERM", "dumb")];

/// What a check concluded, and the log entries it rests on.
struct Finding {
    result: CheckResult,
    reason: Option<String>,
    observed: Vec<usize>,
}

impl Finding {
    fn from_problems(problems: Vec<String>, observed: Vec<usize>) -> Self {
        if problems.is_empty() {
            Self { result: CheckResult::Pass, reason: None, observed }
        } else {
            Self { result: CheckResult::Fail, reason: Some(reason_of(&problems)), observed }
        }
    }

    fn unprovided(reason: String) -> Self {
        Self { result: CheckResult::Unprovided, reason: Some(reason), observed: Vec::new() }
    }
}

/// The most problems a reason lists, and the most characters of one.
const REASON_PROBLEMS: usize = 3;
const PROBLEM_CHARS: usize = 240;

/// The reason of a failed check: the first problems, each cut short, and how many more there are.
fn reason_of(problems: &[String]) -> String {
    let cut = |problem: &String| {
        let mut kept: String = problem.chars().take(PROBLEM_CHARS).collect();
        if problem.chars().count() > PROBLEM_CHARS {
            kept.push('…');
        }
        kept
    };
    let mut reason = problems.iter().take(REASON_PROBLEMS).map(cut).collect::<Vec<_>>().join("; ");
    if problems.len() > REASON_PROBLEMS {
        reason.push_str(&format!("; and {} more", problems.len() - REASON_PROBLEMS));
    }
    reason
}

/// The invocations of one run: the log, and where each named one sits in it.
struct Invocations {
    version: usize,
    unknown_command: usize,
    unknown_command_text: usize,
    bare: usize,
    probe: Option<ProbeCalls>,
}

/// The invocations that use the fixture's probe command.
struct ProbeCalls {
    unknown_option: usize,
    option_before_json: usize,
    after_dashdash: usize,
    /// The plain `<probe> --json`, then one per variation of the environment and one on a terminal.
    json_variations: Vec<(String, usize)>,
    /// The same for `<probe>` in text mode.
    text_variations: Vec<(String, usize)>,
}

struct Run<'a> {
    runner: Runner,
    fixture: &'a Fixture,
    log: Vec<Outcome>,
}

impl Run<'_> {
    fn invoke(&mut self, call: Call) -> Result<usize, RunError> {
        let outcome = self.runner.run(&call)?;
        self.log.push(outcome);
        Ok(self.log.len() - 1)
    }

    fn probe_words(&self) -> Option<&[String]> {
        self.fixture.probe.as_deref()
    }

    fn probe_call(&self, extra: &[&str]) -> Option<Call> {
        self.probe_words().map(|words| Call::new(words.iter().map(String::as_str).chain(extra.iter().copied())))
    }

    fn invoke_all(&mut self) -> Result<Invocations, RunError> {
        let version = self.invoke(Call::new(["--version"]))?;
        let unknown_command = self.invoke(Call::new([NO_SUCH_COMMAND, "--json"]))?;
        let unknown_command_text = self.invoke(Call::new([NO_SUCH_COMMAND]))?;
        let bare = self.invoke(Call::new(["--json"]))?;
        let probe = match self.probe_words() {
            None => None,
            Some(_) => Some(self.invoke_probe()?),
        };
        Ok(Invocations { version, unknown_command, unknown_command_text, bare, probe })
    }

    fn invoke_probe(&mut self) -> Result<ProbeCalls, RunError> {
        let call = |run: &Self, extra: &[&str]| run.probe_call(extra).expect("the probe is set");
        let unknown_option = self.invoke(call(self, &[NO_SUCH_OPTION, "--json"]))?;
        let option_before_json = self.invoke(call(self, &["--json", NO_SUCH_OPTION]))?;
        let after_dashdash = self.invoke(call(self, &["--", "--json"]))?;
        let variations = |run: &mut Self, extra: &[&str]| -> Result<Vec<(String, usize)>, RunError> {
            let base = call(run, extra);
            let mut calls = vec![("without a variation".to_string(), base.clone())];
            for (name, value) in ENV_VARIATIONS {
                calls.push((format!("{name}={value}"), base.clone().with_env(name, value)));
            }
            calls.push(("stdout on a terminal".to_string(), base.on_pty()));
            calls.into_iter().map(|(label, call)| run.invoke(call).map(|index| (label, index))).collect()
        };
        let json_variations = variations(self, &["--json"])?;
        let text_variations = variations(self, &[])?;
        Ok(ProbeCalls { unknown_option, option_before_json, after_dashdash, json_variations, text_variations })
    }
}

/// Runs every L0 check against the fixture's state and reports each.
///
/// `forced_pass` names checks to report as passed whatever they found. The checker's own tests use it
/// to show that every seeded-violation test notices a check that never fails (approaches/checker.md 7.2);
/// a run for a tool passes `&[]`.
pub fn run_l0(runner: Runner, fixture: &Fixture, bin_name: &str, forced_pass: &[&str]) -> Result<Report, RunError> {
    let mut run = Run { runner, fixture, log: Vec::new() };
    let calls = run.invoke_all()?;
    let judge = Judge { log: &run.log, fixture, tool: tool_name(&run.log[calls.unknown_command], bin_name) };
    let findings = [
        judge.version(&calls),
        judge.version_format(&calls),
        judge.unknown_command(&calls),
        judge.unknown_command_text(&calls),
        judge.text_error_form(&calls),
        judge.bare(&calls),
        judge.unknown_option(&calls),
        judge.json_mode_preparse(&calls),
        judge.after_dashdash(&calls),
        judge.ok_exit(),
        judge.no_null(),
        judge.compact(),
        judge.one_document(&calls),
        judge.no_env_switch(&calls),
        judge.no_env_switch_text(&calls),
        judge.contract(),
    ];
    let checks = TABLE
        .iter()
        .zip(findings)
        .map(|(entry, finding)| {
            let finding = if forced_pass.contains(&entry.id) { Finding { result: CheckResult::Pass, reason: None, observed: finding.observed } } else { finding };
            Check {
                id: entry.id,
                strength: entry.strength,
                result: finding.result,
                rules: entry.rules.to_vec(),
                reason: finding.reason,
                observed: finding.observed.iter().map(|index| summary(&run.log[*index])).collect(),
            }
        })
        .collect();
    Ok(Report::new("L0", checks))
}

/// What an `UNKNOWN_COMMAND` envelope must be (the rules of `L0.unknown-command` that read the document, not the streams): a failure with that code, a non-empty `expected`, no `meta.command`, schema version 1.0 and no `data`.
pub fn unknown_command_problems(document: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    if document.get("ok") != Some(&Value::Bool(false)) {
        problems.push("`ok` is not false".to_string());
    }
    let code = document.pointer("/error/code").and_then(Value::as_str);
    if code != Some("UNKNOWN_COMMAND") {
        problems.push(format!("error.code is {}, expected UNKNOWN_COMMAND", code.unwrap_or("missing")));
    }
    if !document.pointer("/error/expected").and_then(Value::as_array).is_some_and(|expected| !expected.is_empty()) {
        problems.push("error.expected is missing or empty".to_string());
    }
    if document.pointer("/meta/command").is_some() {
        problems.push("meta.command is present although no word resolved".to_string());
    }
    let version = document.pointer("/meta/schemaVersion").and_then(Value::as_str);
    if version != Some("1.0") {
        problems.push(format!("meta.schemaVersion is {}, expected \"1.0\"", version.unwrap_or("missing")));
    }
    if document.get("data").is_some() {
        problems.push("data is present on a refusal".to_string());
    }
    problems
}

/// The rules of `L0.ok-exit` that read a failure envelope against the registry: the code is registered, the process exit (when known) is the row's, `retryable` is the row's, and a refusal carries no `data`. Without a known exit the code's registered exit stands for it.
pub fn failure_problems(document: &Value, registry: &[RegistryRow], exit: Option<i32>) -> Vec<String> {
    let Some(code) = document.pointer("/error/code").and_then(Value::as_str) else { return Vec::new() };
    let Some(row) = registry.iter().find(|row| row.code == code) else {
        return vec![format!("failed with {code}, which is not in the registry")];
    };
    let mut problems = Vec::new();
    if let Some(exit) = exit {
        if i64::from(exit) != row.exit {
            problems.push(format!("failed with {code} and exited {exit}, the registry says {}", row.exit));
        }
    }
    if let Some(retryable) = document.pointer("/error/retryable").and_then(Value::as_bool) {
        if retryable != row.retryable {
            problems.push(format!("failed with {code} and `retryable` {retryable}, the registry says {}", row.retryable));
        }
    }
    let refused = exit.map_or(row.exit == 2, |exit| exit == 2);
    if refused && document.get("data").is_some() {
        problems.push(format!("was refused ({code}, exit 2) and carries `data`"));
    }
    problems
}

/// The tool's name: what its own `unknown-command` envelope says, else the binary's file name.
fn tool_name(outcome: &Outcome, bin_name: &str) -> String {
    lenient(&outcome.stdout).and_then(|document| document.pointer("/meta/tool").and_then(Value::as_str).map(str::to_string)).unwrap_or_else(|| bin_name.to_string())
}

struct Judge<'a> {
    log: &'a [Outcome],
    fixture: &'a Fixture,
    tool: String,
}

impl Judge<'_> {
    fn at(&self, index: usize) -> &Outcome {
        &self.log[index]
    }

    /// The first of `indices` that ran over the time limit, as a failure reason.
    fn over_limit(&self, indices: &[usize]) -> Option<String> {
        indices.iter().find(|index| self.at(**index).exit == Exit::TimedOut).map(|index| format!("`{}` ran over the {} second limit and was killed", self.at(*index).call.argv.join(" "), self.fixture.timeout.as_secs_f64()))
    }

    /// A finding from `problems`, or the time-limit failure when one of `indices` ran over.
    fn judged(&self, indices: &[usize], problems: Vec<String>) -> Finding {
        match self.over_limit(indices) {
            Some(reason) => Finding { result: CheckResult::Fail, reason: Some(reason), observed: indices.to_vec() },
            None => Finding::from_problems(problems, indices.to_vec()),
        }
    }

    /// The probe's invocations, or the finding of a check that needs a probe the fixture does not give.
    fn probe_calls<'c>(&self, calls: &'c Invocations) -> Result<&'c ProbeCalls, Finding> {
        calls.probe.as_ref().ok_or_else(|| Finding::unprovided("the fixture has no `probe`: the id of a command that runs without state".to_string()))
    }

    fn version(&self, calls: &Invocations) -> Finding {
        let outcome = self.at(calls.version);
        let mut problems = Vec::new();
        if outcome.exit != Exit::Code(0) {
            problems.push(format!("exit is {}, expected 0", exit_text(outcome.exit)));
        }
        if outcome.stdout.is_empty() {
            problems.push("stdout is empty".to_string());
        }
        if !outcome.stderr.is_empty() {
            problems.push("stderr is not empty".to_string());
        }
        self.judged(&[calls.version], problems)
    }

    fn version_format(&self, calls: &Invocations) -> Finding {
        let outcome = self.at(calls.version);
        let text = String::from_utf8_lossy(&outcome.stdout);
        let line = text.strip_suffix('\n').unwrap_or(&text);
        let expected = format!("{} ", self.tool);
        let ok = !line.contains('\n') && line.strip_prefix(&expected).is_some_and(|version| !version.trim().is_empty());
        let problems = if ok { Vec::new() } else { vec![format!("stdout is {line:?}, expected one line `{} <version>`", self.tool)] };
        self.judged(&[calls.version], problems)
    }

    fn unknown_command(&self, calls: &Invocations) -> Finding {
        let outcome = self.at(calls.unknown_command);
        let mut problems = Vec::new();
        if outcome.exit != Exit::Code(2) {
            problems.push(format!("exit is {}, expected 2", exit_text(outcome.exit)));
        }
        match one_document(&outcome.stdout) {
            Err(reason) => problems.push(reason),
            Ok(document) => problems.extend(unknown_command_problems(&document)),
        }
        if !outcome.stderr.is_empty() {
            problems.push("stderr is not empty in JSON mode".to_string());
        }
        self.judged(&[calls.unknown_command], problems)
    }

    fn unknown_command_text(&self, calls: &Invocations) -> Finding {
        let outcome = self.at(calls.unknown_command_text);
        let mut problems = Vec::new();
        if outcome.exit != Exit::Code(2) {
            problems.push(format!("exit is {}, expected 2", exit_text(outcome.exit)));
        }
        if !outcome.stdout.is_empty() {
            problems.push("stdout is not empty: a failure in text mode prints nothing on stdout".to_string());
        }
        if outcome.stderr.is_empty() {
            problems.push("stderr is empty".to_string());
        }
        self.judged(&[calls.unknown_command_text], problems)
    }

    fn text_error_form(&self, calls: &Invocations) -> Finding {
        let outcome = self.at(calls.unknown_command_text);
        let prefix = format!("{}: UNKNOWN_COMMAND: ", self.tool);
        let stderr = String::from_utf8_lossy(&outcome.stderr);
        let problems = if stderr.starts_with(&prefix) { Vec::new() } else { vec![format!("stderr starts {:?}, expected {prefix:?}", first_line(&stderr))] };
        self.judged(&[calls.unknown_command_text], problems)
    }

    fn bare(&self, calls: &Invocations) -> Finding {
        let outcome = self.at(calls.bare);
        let mut problems = Vec::new();
        if outcome.exit != Exit::Code(2) {
            problems.push(format!("exit is {}, expected 2", exit_text(outcome.exit)));
        }
        match lenient(&outcome.stdout) {
            None => problems.push("stdout is not a JSON document".to_string()),
            Some(document) => {
                let code = document.pointer("/error/code").and_then(Value::as_str);
                if code != Some("COMMAND_REQUIRED") {
                    problems.push(format!("error.code is {}, expected COMMAND_REQUIRED", code.unwrap_or("missing")));
                }
            }
        }
        self.judged(&[calls.bare], problems)
    }

    /// Both `<probe> --bogus-xyz --json` and `<probe> --json --bogus-xyz` need the probe; the first also needs the registry.
    fn unknown_option(&self, calls: &Invocations) -> Finding {
        let missing: Vec<&str> = [(self.fixture.probe.is_none(), "`probe`"), (self.fixture.registry.is_none(), "`registry`")].into_iter().filter_map(|(absent, name)| absent.then_some(name)).collect();
        let Some(probe) = &calls.probe else {
            return Finding::unprovided(format!("the fixture has no {}", missing.join(" and ")));
        };
        if self.fixture.registry.is_none() {
            return Finding::unprovided("the fixture has no `registry`: the tool's error codes, to look the exit-2 code up in".to_string());
        }
        let outcome = self.at(probe.unknown_option);
        let mut problems = Vec::new();
        match lenient(&outcome.stdout) {
            None => problems.push("stdout is not a JSON document".to_string()),
            Some(document) => {
                match document.pointer("/error/code").and_then(Value::as_str) {
                    None => problems.push("error.code is missing".to_string()),
                    Some(code) => match self.fixture.row(code) {
                        None => problems.push(format!("error.code {code} is not in the fixture's registry")),
                        Some(row) if row.exit != 2 => problems.push(format!("error.code {code} is registered with exit {}, expected a refusal (2)", row.exit)),
                        Some(_) => {}
                    },
                }
                if document.pointer("/error/field").and_then(Value::as_str) != Some(NO_SUCH_OPTION) {
                    problems.push(format!("error.field is {}, expected {NO_SUCH_OPTION:?}", show(document.pointer("/error/field"))));
                }
                let id = self.fixture.probe_id().unwrap_or_default();
                if document.pointer("/meta/command").and_then(Value::as_str) != Some(id.as_str()) {
                    problems.push(format!("meta.command is {}, expected {id:?}", show(document.pointer("/meta/command"))));
                }
            }
        }
        self.judged(&[probe.unknown_option], problems)
    }

    fn json_mode_preparse(&self, calls: &Invocations) -> Finding {
        let probe = match self.probe_calls(calls) {
            Ok(probe) => probe,
            Err(finding) => return finding,
        };
        let indices = [probe.unknown_option, probe.option_before_json];
        let problems = indices
            .iter()
            .filter(|index| !lenient(&self.at(**index).stdout).is_some_and(|document| is_envelope(&document) && document.get("ok") == Some(&Value::Bool(false))))
            .map(|index| format!("`{}` did not print a failure envelope on stdout", self.at(*index).call.argv.join(" ")))
            .collect();
        self.judged(&indices, problems)
    }

    fn after_dashdash(&self, calls: &Invocations) -> Finding {
        let probe = match self.probe_calls(calls) {
            Ok(probe) => probe,
            Err(finding) => return finding,
        };
        let outcome = self.at(probe.after_dashdash);
        let problems = if lenient(&outcome.stdout).is_some_and(|document| is_envelope(&document)) { vec!["`--json` after `--` still selected JSON mode".to_string()] } else { Vec::new() };
        self.judged(&[probe.after_dashdash], problems)
    }

    /// Every invocation: the exit is 0, 1 or 2 (or a signal), `ok` is true exactly when it is 0, a failure's code is registered and its exit and `retryable` are the row's, an exit 2 carries no `data`, and an exit 2 changed nothing.
    fn ok_exit(&self) -> Finding {
        let mut problems = Vec::new();
        let mut observed = Vec::new();
        for (index, outcome) in self.log.iter().enumerate() {
            if outcome.exit == Exit::TimedOut {
                continue;
            }
            let before = problems.len();
            let label = format!("`{}`", outcome.call.argv.join(" "));
            if let Exit::Code(code) = outcome.exit {
                if !(0..=2).contains(&code) {
                    problems.push(format!("{label} exited {code}, outside 0, 1 and 2"));
                }
            }
            if outcome.exit == Exit::Code(2) && !outcome.changes.is_empty() {
                problems.push(format!("{label} was refused (exit 2) and changed {}", outcome.changes.join(", ")));
            }
            if outcome.call.json_mode() {
                if let Some(document) = lenient(&outcome.stdout) {
                    self.envelope_exit(&label, outcome.exit, &document, &mut problems);
                }
            }
            if problems.len() > before {
                observed.push(index);
            }
        }
        let missing_registry = self.fixture.registry.is_none();
        if problems.is_empty() && missing_registry {
            return Finding::unprovided("the fixture has no `registry`: the tool's error codes, to look every failure's exit and `retryable` up in".to_string());
        }
        if problems.is_empty() {
            observed.clear();
        }
        Finding::from_problems(problems, observed)
    }

    fn envelope_exit(&self, label: &str, exit: Exit, document: &Value, problems: &mut Vec<String>) {
        let Some(ok) = document.get("ok").and_then(Value::as_bool) else { return };
        let code = match exit {
            Exit::Code(code) => Some(code),
            _ => None,
        };
        if let Some(code) = code {
            if ok != (code == 0) {
                problems.push(format!("{label} exited {code} with `ok` {ok}"));
            }
        }
        if ok {
            return;
        }
        match &self.fixture.registry {
            Some(registry) => problems.extend(failure_problems(document, registry, code).into_iter().map(|problem| format!("{label} {problem}"))),
            None if code == Some(2) && document.get("data").is_some() => problems.push(format!("{label} was refused (exit 2) and carries `data`")),
            None => {}
        }
    }

    /// The JSON-mode invocations whose stdout parses as a document.
    fn envelopes(&self) -> impl Iterator<Item = (usize, Value)> + '_ {
        self.log.iter().enumerate().filter(|(_, outcome)| outcome.call.json_mode() && outcome.exit != Exit::TimedOut).filter_map(|(index, outcome)| lenient(&outcome.stdout).map(|document| (index, document)))
    }

    fn no_null(&self) -> Finding {
        let mut problems = Vec::new();
        let mut observed = Vec::new();
        for (index, document) in self.envelopes() {
            let found = schemas().envelope_problems(&document);
            if !found.is_empty() {
                problems.push(format!("`{}` printed an envelope the schema rejects: {}", self.at(index).call.argv.join(" "), found.iter().take(3).cloned().collect::<Vec<_>>().join(", ")));
                observed.push(index);
            }
        }
        Finding::from_problems(problems, observed)
    }

    fn compact(&self) -> Finding {
        let mut problems = Vec::new();
        let mut observed = Vec::new();
        for (index, _) in self.envelopes() {
            let stdout = &self.at(index).stdout;
            let body = stdout.strip_suffix(b"\n").unwrap_or(stdout);
            if body.contains(&b'\n') {
                problems.push(format!("`{}` printed an indented document", self.at(index).call.argv.join(" ")));
                observed.push(index);
            }
        }
        Finding::from_problems(problems, observed)
    }

    fn contract(&self) -> Finding {
        let mut problems = Vec::new();
        let mut observed = Vec::new();
        for (index, document) in self.envelopes().filter(|(_, document)| is_envelope(document)) {
            if document.pointer("/meta/contract").is_none() {
                problems.push(format!("`{}` printed an envelope without meta.contract", self.at(index).call.argv.join(" ")));
                observed.push(index);
            }
        }
        Finding::from_problems(problems, observed)
    }

    /// The probe's `--json` stdout is exactly one document and one newline.
    fn one_document(&self, calls: &Invocations) -> Finding {
        let probe = match self.probe_calls(calls) {
            Ok(probe) => probe,
            Err(finding) => return finding,
        };
        let index = probe.json_variations[0].1;
        let problems = one_document(&self.at(index).stdout).err().into_iter().collect();
        self.judged(&[index], problems)
    }

    fn no_env_switch(&self, calls: &Invocations) -> Finding {
        let probe = match self.probe_calls(calls) {
            Ok(probe) => probe,
            Err(finding) => return finding,
        };
        self.same_under_variations(&probe.json_variations, |outcome| envelope_signature(&outcome.stdout))
    }

    fn no_env_switch_text(&self, calls: &Invocations) -> Finding {
        let probe = match self.probe_calls(calls) {
            Ok(probe) => probe,
            Err(finding) => return finding,
        };
        self.same_under_variations(&probe.text_variations, |outcome| {
            let kind = if lenient(&outcome.stdout).is_some_and(|document| is_envelope(&document)) { "an envelope" } else { "not an envelope" };
            kind.to_string()
        })
    }

    /// `variations[0]` is the plain run; every other run must show what it shows.
    fn same_under_variations(&self, variations: &[(String, usize)], signature: impl Fn(&Outcome) -> String) -> Finding {
        let indices: Vec<usize> = variations.iter().map(|(_, index)| *index).collect();
        let plain = signature(self.at(variations[0].1));
        let problems = variations[1..].iter().filter_map(|(label, index)| {
            let seen = signature(self.at(*index));
            (seen != plain).then(|| format!("with {label} the output is {seen}, without it {plain}"))
        });
        self.judged(&indices, problems.collect())
    }
}

/// The shape no-env-switch compares: `ok`, the error code, `meta.command`, `meta.schemaVersion` and the keys of `data`; or that there is no envelope.
fn envelope_signature(stdout: &[u8]) -> String {
    let Some(document) = lenient(stdout).filter(is_envelope) else { return "not an envelope".to_string() };
    let data_keys: BTreeSet<&str> = document.get("data").and_then(Value::as_object).map(|data| data.keys().map(String::as_str).collect()).unwrap_or_default();
    format!(
        "an envelope with ok {}, error.code {}, meta.command {}, meta.schemaVersion {}, data keys {data_keys:?}",
        show(document.get("ok")),
        show(document.pointer("/error/code")),
        show(document.pointer("/meta/command")),
        show(document.pointer("/meta/schemaVersion")),
    )
}

/// An envelope as far as the checks need to know: an object with a boolean `ok` and an object `meta`. The schema judges the rest.
fn is_envelope(document: &Value) -> bool {
    document.get("ok").is_some_and(Value::is_boolean) && document.get("meta").is_some_and(Value::is_object)
}

/// The stdout as one JSON value when it parses as one, indentation allowed; `None` otherwise.
fn lenient(stdout: &[u8]) -> Option<Value> {
    serde_json::from_str(std::str::from_utf8(stdout).ok()?.trim()).ok()
}

/// Spec 5.1 and 5.4: UTF-8, one JSON document, one newline, nothing else.
fn one_document(stdout: &[u8]) -> Result<Value, String> {
    let text = std::str::from_utf8(stdout).map_err(|error| format!("stdout is not UTF-8: {error}"))?;
    let Some(body) = text.strip_suffix('\n') else {
        return Err(if text.is_empty() { "stdout is empty".to_string() } else { "stdout does not end with a newline".to_string() });
    };
    if body.ends_with(char::is_whitespace) {
        return Err("stdout ends with more than one newline".to_string());
    }
    serde_json::from_str(body).map_err(|error| format!("stdout is not exactly one JSON document: {error}"))
}

fn exit_text(exit: Exit) -> String {
    match exit {
        Exit::Code(code) => code.to_string(),
        Exit::Signal(signal) => format!("signal {signal}"),
        Exit::TimedOut => "a timeout".to_string(),
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

fn show(value: Option<&Value>) -> String {
    value.map_or_else(|| "missing".to_string(), Value::to_string)
}

/// The most characters of a stream the report keeps.
const EXCERPT: usize = 300;

fn excerpt(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut kept: String = text.chars().take(EXCERPT).collect();
    if text.chars().count() > EXCERPT {
        kept.push('…');
    }
    kept
}

/// One invocation as the report shows it.
fn summary(outcome: &Outcome) -> Value {
    let mut entry = Map::new();
    entry.insert("argv".to_string(), json!(outcome.call.argv));
    if !outcome.call.env.is_empty() {
        entry.insert("env".to_string(), Value::Object(outcome.call.env.iter().map(|(name, value)| (name.clone(), json!(value))).collect()));
    }
    if outcome.call.pty {
        entry.insert("pty".to_string(), json!(true));
    }
    match outcome.exit {
        Exit::Code(code) => entry.insert("exit".to_string(), json!(code)),
        Exit::Signal(signal) => entry.insert("signal".to_string(), json!(signal)),
        Exit::TimedOut => entry.insert("timedOut".to_string(), json!(true)),
    };
    if !outcome.stdout.is_empty() {
        entry.insert("stdout".to_string(), json!(excerpt(&outcome.stdout)));
    }
    if !outcome.stderr.is_empty() {
        entry.insert("stderr".to_string(), json!(excerpt(&outcome.stderr)));
    }
    if !outcome.changes.is_empty() {
        entry.insert("changed".to_string(), json!(outcome.changes));
    }
    Value::Object(entry)
}

#[cfg(test)]
mod tests {
    use super::{ids, is_envelope, lenient, one_document, TABLE};
    use crate::report::Strength;

    #[test]
    fn atip_check_01_the_table_is_the_l0_table_twelve_must_and_four_should() {
        assert_eq!(ids().len(), 16);
        assert_eq!(TABLE.iter().filter(|entry| entry.strength == Strength::Must).count(), 12);
        assert!(ids().iter().all(|id| id.starts_with("L0.")));
        let mut unique = ids().to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 16);
    }

    #[test]
    fn atip_check_19_one_document_means_one_value_and_one_newline() {
        one_document(b"{\"a\":1}\n").expect("one document");
        one_document(b"{\n  \"a\": 1\n}\n").expect("indentation is allowed, L0.compact judges it");
        for bad in [&b""[..], b"{\"a\":1}", b"{\"a\":1}\n\n", b"{\"a\":1}\n{\"b\":2}\n", b"\xff\n", b"text\n"] {
            one_document(bad).expect_err(&format!("{:?} must be refused", String::from_utf8_lossy(bad)));
        }
    }

    #[test]
    fn atip_check_15_an_envelope_needs_a_boolean_ok_and_a_meta_object() {
        assert!(is_envelope(&lenient(b"{\"ok\":false,\"meta\":{}}").expect("json")));
        assert!(!is_envelope(&lenient(b"{\"ok\":\"yes\",\"meta\":{}}").expect("json")));
        assert!(!is_envelope(&lenient(b"[1]").expect("json")));
        assert!(lenient(b"not json").is_none());
    }
}
