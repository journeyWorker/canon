//! The L0 checks: the 16 of approaches/checker.md 6 ("L0") and `L0.refusal-effects`, `L0.next` and `L0.page`.
//!
//! A run first makes every invocation the L0 table names, each in its own fresh copy of the fixture state, and
//! keeps what each one did in a log. Each check then reads the log.
//!
//! **One owner per pass condition.** A check is a list of *conditions*, each with an id (`version/exit-0`). A
//! condition is judged by exactly one check, so breaking one rule fails one check: the checks that would need
//! the same fact skip what is not theirs (an envelope that does not parse is `L0.unknown-command`'s, `L0.bare`'s
//! or `L0.json-mode-preparse`'s to report, and the others read only the documents that do). A condition whose
//! subject was not observed is not judged.
//!
//! **Needs.** A check declares the fixture entries it needs (`probe`, `registry`). A violation it observed is a
//! `fail` whatever the fixture lacks; a check that found nothing wrong but lacked an entry is `unprovided`.

use std::cell::RefCell;
use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use crate::exec::{Call, Exit, Outcome, RunError, Runner};
use crate::fixture::{Fixture, RegistryRow};
use crate::report::{Check, CheckResult, Report, Strength};
use crate::schema::schemas;

/// A fixture entry a check needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// The id of a command that runs without state.
    Probe,
    /// The tool's error registry.
    Registry,
}

/// One entry of the L0 table: the id, whether it is MUST or SHOULD, the contract rules it enforces and the fixture entries it needs.
struct Entry {
    id: &'static str,
    strength: Strength,
    rules: &'static [&'static str],
    needs: &'static [Need],
}

const fn must(id: &'static str, rules: &'static [&'static str], needs: &'static [Need]) -> Entry {
    Entry {
        id,
        strength: Strength::Must,
        rules,
        needs,
    }
}

const fn should(id: &'static str, rules: &'static [&'static str], needs: &'static [Need]) -> Entry {
    Entry {
        id,
        strength: Strength::Should,
        rules,
        needs,
    }
}

use Need::{Probe, Registry};

/// The L0 table, in the order of approaches/checker.md with the three added checks next to the checks they split off from.
const TABLE: [Entry; 19] = [
    must("L0.version", &["4.6"], &[]),
    should("L0.version-format", &["4.6"], &[]),
    must(
        "L0.unknown-command",
        &["4.4", "5.1", "5.2", "6.4", "6.6", "6.11", "7.2"],
        &[],
    ),
    must("L0.unknown-command-text", &["5.3"], &[]),
    should("L0.text-error-form", &["7.4"], &[]),
    must("L0.bare", &["7.2", "9.2"], &[]),
    must(
        "L0.unknown-option",
        &["4.4", "6.6", "9.9"],
        &[Probe, Registry],
    ),
    must("L0.json-mode-preparse", &["4.4"], &[Probe]),
    must("L0.after-dashdash", &["4.4"], &[Probe]),
    must(
        "L0.ok-exit",
        &["6.2", "6.4", "7.3.2", "7.3.3", "7.3.4", "8.1", "8.6"],
        &[Registry],
    ),
    must("L0.refusal-effects", &["8.1", "8.2"], &[]),
    must("L0.no-null", &["6.5"], &[]),
    should("L0.compact", &["5.4"], &[]),
    must("L0.one-document", &["5.1"], &[Probe]),
    must("L0.no-env-switch", &["4.3", "5.5"], &[Probe]),
    must("L0.no-env-switch-text", &["4.3"], &[Probe]),
    must("L0.next", &["6.8"], &[]),
    must("L0.page", &["6.9"], &[]),
    should("L0.contract", &["6.6"], &[]),
];

/// The ids of the L0 checks, in table order.
pub fn ids() -> [&'static str; 19] {
    TABLE.each_ref().map(|entry| entry.id)
}

/// The fixture entries the check `id` needs.
pub fn needs(id: &str) -> &'static [Need] {
    TABLE
        .iter()
        .find(|entry| entry.id == id)
        .map_or(&[], |entry| entry.needs)
}

/// The word no tool has as a command.
const NO_SUCH_COMMAND: &str = "nosuch-xyz";
/// The word no command has as an option.
const NO_SUCH_OPTION: &str = "--bogus-xyz";
/// The variations of the environment under which the output kind must not change (spec 4.3), each a variable and its value.
const ENV_VARIATIONS: [(&str, &str); 5] = [
    ("CI", "1"),
    ("CLAUDECODE", "1"),
    ("OUTPUT_FORMAT", "text"),
    ("NO_COLOR", "1"),
    ("TERM", "dumb"),
];
/// How long a call with stdin on an idle terminal may take before it is taken to wait for input.
const PROMPT_LIMIT: std::time::Duration = std::time::Duration::from_secs(5);
/// The reserved codes an L0 tool registers, with the exit the contract gives them (spec 7.2.1).
const RESERVED: [(&str, i64); 4] = [
    ("USAGE", 2),
    ("UNKNOWN_COMMAND", 2),
    ("COMMAND_REQUIRED", 2),
    ("INTERNAL", 1),
];

/// What a check concluded, and the log entries it rests on.
struct Finding {
    result: CheckResult,
    reason: Option<String>,
    advisories: Vec<String>,
    observed: Vec<usize>,
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
    let mut reason = problems
        .iter()
        .take(REASON_PROBLEMS)
        .map(cut)
        .collect::<Vec<_>>()
        .join("; ");
    if problems.len() > REASON_PROBLEMS {
        reason.push_str(&format!("; and {} more", problems.len() - REASON_PROBLEMS));
    }
    reason
}

/// The invocations of one run: where each named one sits in the log.
struct Invocations {
    version: usize,
    unknown_command: usize,
    unknown_command_text: usize,
    bare: usize,
    schema_errors: usize,
    probe: Option<ProbeCalls>,
}

/// The invocations that use the fixture's probe command.
struct ProbeCalls {
    /// `<probe> --bogus-xyz --json`.
    unknown_option: usize,
    /// `<probe> --json --bogus-xyz`.
    json_first: usize,
    after_dashdash: usize,
    /// The plain `<probe> --json`, then one per variation of the environment and one on a terminal.
    json_variations: Vec<(String, usize)>,
    /// The same for `<probe>` in text mode.
    text_variations: Vec<(String, usize)>,
    /// `<probe> --json` with stdin on a terminal nobody types on.
    prompt: usize,
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

    fn invoke_all(&mut self) -> Result<Invocations, RunError> {
        let version = self.invoke(Call::new(["--version"]))?;
        let unknown_command = self.invoke(Call::new([NO_SUCH_COMMAND, "--json"]))?;
        let unknown_command_text = self.invoke(Call::new([NO_SUCH_COMMAND]))?;
        let bare = self.invoke(Call::new(["--json"]))?;
        let schema_errors = self.invoke(Call::new(["schema", "--errors", "--json"]))?;
        let probe = match self.fixture.probe.clone() {
            None => None,
            Some(words) => Some(self.invoke_probe(&words)?),
        };
        Ok(Invocations {
            version,
            unknown_command,
            unknown_command_text,
            bare,
            schema_errors,
            probe,
        })
    }

    fn invoke_probe(&mut self, words: &[String]) -> Result<ProbeCalls, RunError> {
        let call = |extra: &[&str]| {
            Call::new(
                words
                    .iter()
                    .map(String::as_str)
                    .chain(extra.iter().copied()),
            )
        };
        let unknown_option = self.invoke(call(&[NO_SUCH_OPTION, "--json"]))?;
        let json_first = self.invoke(call(&["--json", NO_SUCH_OPTION]))?;
        let after_dashdash = self.invoke(call(&["--", "--json"]))?;
        let variations =
            |run: &mut Self, extra: &[&str]| -> Result<Vec<(String, usize)>, RunError> {
                let base = call(extra);
                let mut calls = vec![("without a variation".to_string(), base.clone())];
                for (name, value) in ENV_VARIATIONS {
                    calls.push((
                        format!("{name}={value}"),
                        base.clone().with_env(name, value),
                    ));
                }
                calls.push(("stdout on a terminal".to_string(), base.on_pty()));
                calls
                    .into_iter()
                    .map(|(label, call)| run.invoke(call).map(|index| (label, index)))
                    .collect()
            };
        let json_variations = variations(self, &["--json"])?;
        let text_variations = variations(self, &[])?;
        let prompt = self
            .invoke(call(&["--json"]).waiting_on_stdin(PROMPT_LIMIT.min(self.fixture.timeout)))?;
        Ok(ProbeCalls {
            unknown_option,
            json_first,
            after_dashdash,
            json_variations,
            text_variations,
            prompt,
        })
    }
}

/// How a test changes a run: checks reported as passed whatever they found, and conditions treated as holding.
/// A run for a tool uses [`Mutation::none`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct Mutation<'a> {
    pub(crate) forced_pass: &'a [&'a str],
    pub(crate) skipped_conditions: &'a [&'a str],
}

impl Mutation<'_> {
    pub(crate) const fn none() -> Mutation<'static> {
        Mutation {
            forced_pass: &[],
            skipped_conditions: &[],
        }
    }
}

/// What one run produced: the report, and the id of every condition it judged.
#[derive(Debug)]
pub(crate) struct Judged {
    pub(crate) report: Report,
    #[cfg_attr(
        not(feature = "test-support"),
        expect(dead_code, reason = "read only by app::check_binary_mutated")
    )]
    pub(crate) conditions: BTreeSet<String>,
}

/// Runs every L0 check against the fixture's state and reports each.
pub(crate) fn run_l0(
    runner: Runner,
    fixture: &Fixture,
    bin_name: &str,
    mutation: Mutation<'_>,
) -> Result<Judged, RunError> {
    let mut run = Run {
        runner,
        fixture,
        log: Vec::new(),
    };
    let calls = run.invoke_all()?;
    let judge = Judge::new(&run.log, fixture, bin_name, mutation);
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
        judge.ok_exit(&calls),
        judge.refusal_effects(),
        judge.no_null(),
        judge.compact(),
        judge.one_document(&calls),
        judge.no_env_switch(&calls),
        judge.no_env_switch_text(&calls),
        judge.next(),
        judge.page(),
        judge.contract(),
    ];
    let checks = TABLE
        .iter()
        .zip(findings)
        .map(|(entry, finding)| {
            let finding = if mutation.forced_pass.contains(&entry.id) {
                Finding {
                    result: CheckResult::Pass,
                    reason: None,
                    ..finding
                }
            } else {
                finding
            };
            Check {
                id: entry.id,
                strength: entry.strength,
                result: finding.result,
                rules: entry.rules.to_vec(),
                reason: finding.reason,
                advisories: finding.advisories,
                observed: finding
                    .observed
                    .iter()
                    .map(|index| summary(&run.log[*index]))
                    .collect(),
            }
        })
        .collect();
    let conditions = judge.seen.into_inner();
    Ok(Judged {
        report: Report::new("L0", checks),
        conditions,
    })
}

/// The conditions of one check being judged: what fails, what was looked at, what is only recommended.
struct Conditions<'j> {
    judge: &'j Judge<'j>,
    entry: &'static Entry,
    name: &'static str,
    problems: Vec<String>,
    advisories: Vec<String>,
    observed: Vec<usize>,
    /// The invocations this check made for itself: one that runs over the time limit is this check's failure.
    owned: Vec<usize>,
}

impl<'j> Conditions<'j> {
    /// Judges condition `condition`: `holds` says whether it did, `why` says what is wrong when it did not.
    fn holds(&mut self, condition: &'static str, holds: bool, why: impl FnOnce() -> String) {
        let id = format!("{}/{condition}", self.name);
        let skipped = self
            .judge
            .mutation
            .skipped_conditions
            .contains(&id.as_str());
        self.judge.seen.borrow_mut().insert(id);
        if !holds && !skipped {
            self.problems.push(why());
        }
    }

    fn observe(&mut self, index: usize) {
        if !self.observed.contains(&index) {
            self.observed.push(index);
        }
    }

    /// Like [`Conditions::observe`], for an invocation this check made for itself.
    fn own(&mut self, index: usize) {
        self.observe(index);
        if !self.owned.contains(&index) {
            self.owned.push(index);
        }
    }

    fn advise(&mut self, advisory: String) {
        if !self.advisories.contains(&advisory) {
            self.advisories.push(advisory);
        }
    }

    /// The finding: a run over the time limit or a violated condition is a fail; else a missing fixture entry is
    /// unprovided; else a check that saw nothing of its subject (`exercised` false) does not apply.
    fn finish(self, exercised: bool) -> Finding {
        let over = self
            .owned
            .iter()
            .find(|index| self.judge.at(**index).exit == Exit::TimedOut);
        if let Some(index) = over {
            let reason = format!(
                "`{}` ran over the {} second limit and was killed",
                self.judge.at(*index).call.argv.join(" "),
                self.judge.limit(*index).as_secs_f64()
            );
            return Finding {
                result: CheckResult::Fail,
                reason: Some(reason),
                advisories: self.advisories,
                observed: self.observed,
            };
        }
        if !self.problems.is_empty() {
            return Finding {
                result: CheckResult::Fail,
                reason: Some(reason_of(&self.problems)),
                advisories: self.advisories,
                observed: self.observed,
            };
        }
        let missing: Vec<&str> = self
            .entry
            .needs
            .iter()
            .filter(|need| match need {
                Probe => self.judge.fixture.probe.is_none(),
                Registry => self.judge.fixture.registry.is_none(),
            })
            .map(|need| match need {
                Probe => "`probe` (the id of a command that runs without state)",
                Registry => "`registry` (the tool's error codes)",
            })
            .collect();
        if !missing.is_empty() {
            return Finding {
                result: CheckResult::Unprovided,
                reason: Some(format!("the fixture has no {}", missing.join(" and "))),
                advisories: self.advisories,
                observed: Vec::new(),
            };
        }
        let result = if exercised {
            CheckResult::Pass
        } else {
            CheckResult::Unsupported
        };
        let reason = (!exercised)
            .then(|| "no envelope the run saw carries what this check judges".to_string());
        Finding {
            result,
            reason,
            advisories: self.advisories,
            observed: self.observed,
        }
    }
}

struct Judge<'a> {
    log: &'a [Outcome],
    fixture: &'a Fixture,
    /// The first document of each invocation's stdout, when it parses.
    documents: Vec<Option<Value>>,
    tool: String,
    mutation: Mutation<'a>,
    seen: RefCell<BTreeSet<String>>,
}

impl<'a> Judge<'a> {
    fn new(
        log: &'a [Outcome],
        fixture: &'a Fixture,
        bin_name: &str,
        mutation: Mutation<'a>,
    ) -> Self {
        let documents: Vec<Option<Value>> = log
            .iter()
            .map(|outcome| first_document(&outcome.stdout))
            .collect();
        let tool = documents
            .iter()
            .flatten()
            .find_map(|document| document.pointer("/meta/tool").and_then(Value::as_str))
            .unwrap_or(bin_name)
            .to_string();
        Self {
            log,
            fixture,
            documents,
            tool,
            mutation,
            seen: RefCell::new(BTreeSet::new()),
        }
    }

    fn at(&self, index: usize) -> &Outcome {
        &self.log[index]
    }

    /// The envelope an invocation printed: its first document, when that is an envelope.
    fn envelope(&self, index: usize) -> Option<&Value> {
        self.documents[index]
            .as_ref()
            .filter(|document| is_envelope(document))
    }

    fn limit(&self, index: usize) -> std::time::Duration {
        self.at(index).call.limit.unwrap_or(self.fixture.timeout)
    }

    fn check(&'a self, id: &'static str) -> Conditions<'a> {
        let entry = TABLE
            .iter()
            .find(|entry| entry.id == id)
            .unwrap_or_else(|| panic!("{id} is not in the L0 table"));
        Conditions {
            judge: self,
            entry,
            name: &id[3..],
            problems: Vec::new(),
            advisories: Vec::new(),
            observed: Vec::new(),
            owned: Vec::new(),
        }
    }

    /// The JSON-mode invocations that finished and printed an envelope.
    fn envelopes(&self) -> Vec<(usize, &Value)> {
        (0..self.log.len())
            .filter(|index| {
                self.at(*index).call.json_mode() && self.at(*index).exit != Exit::TimedOut
            })
            .filter_map(|index| self.envelope(index).map(|document| (index, document)))
            .collect()
    }

    fn stderr_conditions(&self, c: &mut Conditions<'_>, index: usize, document: &Value) {
        let carries = carries_error_or_result(&self.at(index).stderr, document);
        c.holds("stderr-carries-no-error", !carries, || {
            format!(
                "`{}` also printed its error or result on stderr",
                self.at(index).call.argv.join(" ")
            )
        });
        if !carries && !self.at(index).stderr.is_empty() {
            c.advise(format!(
                "`{}` printed on stderr in JSON mode; the contract recommends it stay empty (5.2)",
                self.at(index).call.argv.join(" ")
            ));
        }
    }

    fn version(&'a self, calls: &Invocations) -> Finding {
        let outcome = self.at(calls.version);
        let mut c = self.check("L0.version");
        c.own(calls.version);
        c.holds("exit-0", outcome.exit == Exit::Code(0), || {
            format!("exit is {}, expected 0", exit_text(outcome.exit))
        });
        c.holds("stdout-non-empty", !outcome.stdout.is_empty(), || {
            "stdout is empty".to_string()
        });
        if !outcome.stderr.is_empty() {
            c.advise(
                "--version printed on stderr; the contract recommends nothing there".to_string(),
            );
        }
        c.finish(true)
    }

    fn version_format(&'a self, calls: &Invocations) -> Finding {
        let outcome = self.at(calls.version);
        let mut c = self.check("L0.version-format");
        // Nothing on stdout is `L0.version`'s to report.
        if !outcome.stdout.is_empty() {
            c.observe(calls.version);
            let text = String::from_utf8_lossy(&outcome.stdout);
            let line = text.strip_suffix('\n').unwrap_or(&text);
            let ok = !line.contains('\n')
                && line
                    .strip_prefix(&format!("{} ", self.tool))
                    .is_some_and(|version| !version.trim().is_empty());
            c.holds("tool-version-line", ok, || {
                format!(
                    "stdout is {line:?}, expected one line `{} <version>`",
                    self.tool
                )
            });
        }
        c.finish(true)
    }

    fn unknown_command(&'a self, calls: &Invocations) -> Finding {
        let mut c = self.check("L0.unknown-command");
        c.own(calls.unknown_command);
        let document = self.envelope(calls.unknown_command);
        c.holds("envelope", document.is_some(), || {
            "stdout is not an envelope".to_string()
        });
        if let Some(document) = document {
            for (condition, holds, why) in unknown_command_conditions(document) {
                c.holds(condition, holds, || why);
            }
            self.stderr_conditions(&mut c, calls.unknown_command, document);
        }
        c.finish(true)
    }

    fn unknown_command_text(&'a self, calls: &Invocations) -> Finding {
        let outcome = self.at(calls.unknown_command_text);
        let mut c = self.check("L0.unknown-command-text");
        c.own(calls.unknown_command_text);
        c.holds("exit-2", outcome.exit == Exit::Code(2), || {
            format!("exit is {}, expected 2", exit_text(outcome.exit))
        });
        c.holds("stdout-empty", outcome.stdout.is_empty(), || {
            "stdout is not empty: a failure in text mode prints nothing on stdout".to_string()
        });
        c.holds("stderr-non-empty", !outcome.stderr.is_empty(), || {
            "stderr is empty".to_string()
        });
        c.finish(true)
    }

    fn text_error_form(&'a self, calls: &Invocations) -> Finding {
        let outcome = self.at(calls.unknown_command_text);
        let mut c = self.check("L0.text-error-form");
        // An empty stderr is `L0.unknown-command-text`'s to report.
        if !outcome.stderr.is_empty() {
            c.observe(calls.unknown_command_text);
            let prefix = format!("{}: UNKNOWN_COMMAND: ", self.tool);
            let stderr = String::from_utf8_lossy(&outcome.stderr);
            c.holds("prefix", stderr.starts_with(&prefix), || {
                format!(
                    "stderr starts {:?}, expected {prefix:?}",
                    first_line(&stderr)
                )
            });
        }
        c.finish(true)
    }

    fn bare(&'a self, calls: &Invocations) -> Finding {
        let mut c = self.check("L0.bare");
        c.own(calls.bare);
        let document = self.envelope(calls.bare);
        c.holds("envelope", document.is_some(), || {
            "stdout is not an envelope".to_string()
        });
        if let Some(document) = document {
            let code = document.pointer("/error/code").and_then(Value::as_str);
            c.holds("code", code == Some("COMMAND_REQUIRED"), || {
                format!(
                    "error.code is {}, expected COMMAND_REQUIRED",
                    code.unwrap_or("missing")
                )
            });
            self.stderr_conditions(&mut c, calls.bare, document);
        }
        c.finish(true)
    }

    /// `<probe> --bogus-xyz --json`: a refusal code, the field and the command. Whether it is an envelope is `L0.json-mode-preparse`'s.
    fn unknown_option(&'a self, calls: &Invocations) -> Finding {
        let mut c = self.check("L0.unknown-option");
        let Some(probe) = &calls.probe else {
            return c.finish(false);
        };
        c.own(probe.unknown_option);
        if let Some(document) = self.envelope(probe.unknown_option) {
            let code = document.pointer("/error/code").and_then(Value::as_str);
            // A code the registry lacks is `L0.ok-exit`'s; a registered one must be a refusal.
            let row = code.and_then(|code| self.fixture.row(code));
            c.holds(
                "code-is-a-refusal",
                row.is_none_or(|row| row.exit == 2),
                || {
                    format!(
                        "error.code {} is registered with exit {}, expected a refusal (2)",
                        code.unwrap_or_default(),
                        row.map_or(0, |row| row.exit)
                    )
                },
            );
            let field = document.pointer("/error/field").and_then(Value::as_str);
            c.holds("field", field == Some(NO_SUCH_OPTION), || {
                format!(
                    "error.field is {}, expected {NO_SUCH_OPTION:?}",
                    show(document.pointer("/error/field"))
                )
            });
            let id = self.fixture.probe_id().unwrap_or_default();
            let command = document.pointer("/meta/command").and_then(Value::as_str);
            c.holds("meta-command", command == Some(id.as_str()), || {
                format!(
                    "meta.command is {}, expected {id:?}",
                    show(document.pointer("/meta/command"))
                )
            });
            self.stderr_conditions(&mut c, probe.unknown_option, document);
        }
        c.finish(true)
    }

    fn json_mode_preparse(&'a self, calls: &Invocations) -> Finding {
        let mut c = self.check("L0.json-mode-preparse");
        let Some(probe) = &calls.probe else {
            return c.finish(false);
        };
        for (condition, index) in [
            ("json-last", probe.unknown_option),
            ("json-first", probe.json_first),
        ] {
            if index == probe.json_first {
                c.own(index);
            } else {
                c.observe(index);
            }
            let failure = self
                .envelope(index)
                .is_some_and(|document| document.get("ok") == Some(&Value::Bool(false)));
            c.holds(condition, failure, || {
                format!(
                    "`{}` did not print a failure envelope on stdout",
                    self.at(index).call.argv.join(" ")
                )
            });
        }
        c.finish(true)
    }

    fn after_dashdash(&'a self, calls: &Invocations) -> Finding {
        let mut c = self.check("L0.after-dashdash");
        let Some(probe) = &calls.probe else {
            return c.finish(false);
        };
        c.own(probe.after_dashdash);
        c.holds(
            "not-an-envelope",
            self.envelope(probe.after_dashdash).is_none(),
            || "`--json` after `--` still selected JSON mode".to_string(),
        );
        c.finish(true)
    }

    /// Every invocation: the exit is 0, 1, 2 or a signal; `ok` is true exactly when it is 0; a failure's code is
    /// registered with the exit and `retryable` the process shows; a refusal carries no `data`. Also the registry
    /// itself: the reserved codes have their reserved exits, and a registry the tool prints (`schema --errors
    /// --json`) is the fixture's.
    fn ok_exit(&'a self, calls: &Invocations) -> Finding {
        let mut c = self.check("L0.ok-exit");
        c.own(calls.schema_errors);
        for (index, outcome) in self.log.iter().enumerate() {
            if outcome.exit == Exit::TimedOut {
                continue;
            }
            let before = c.problems.len();
            let label = format!("`{}`", outcome.call.argv.join(" "));
            c.holds("exit-range", exit_is_allowed(outcome.exit), || {
                format!(
                    "{label} exited {}, outside 0, 1 and 2 and not a signal exit",
                    exit_text(outcome.exit)
                )
            });
            if let (true, Some(document)) = (outcome.call.json_mode(), self.envelope(index)) {
                self.envelope_exit(&mut c, &label, outcome.exit, document);
            }
            if c.problems.len() > before {
                c.observe(index);
            }
        }
        if let Some(registry) = &self.fixture.registry {
            let wrong: Vec<String> = RESERVED
                .iter()
                .filter_map(|(code, exit)| {
                    registry
                        .iter()
                        .find(|row| row.code == *code)
                        .filter(|row| row.exit != *exit)
                        .map(|row| {
                            format!(
                                "{code} is registered with exit {}, the contract says {exit}",
                                row.exit
                            )
                        })
                })
                .collect();
            c.holds("reserved-codes", wrong.is_empty(), || wrong.join(", "));
            if let Some(offered) = offered_registry(self.documents[calls.schema_errors].as_ref()) {
                let drift = registry_drift(registry, &offered);
                c.holds("schema-errors-match", drift.is_empty(), || {
                    format!(
                        "`schema --errors --json` and the fixture's registry differ: {}",
                        drift.join(", ")
                    )
                });
                c.observe(calls.schema_errors);
            }
        }
        c.finish(true)
    }

    fn envelope_exit(&self, c: &mut Conditions<'_>, label: &str, exit: Exit, document: &Value) {
        let Some(ok) = document.get("ok").and_then(Value::as_bool) else {
            return;
        };
        let code = match exit {
            Exit::Code(code) if (0..=2).contains(&code) => Some(code),
            _ => None,
        };
        if let Some(code) = code {
            c.holds("ok-iff-exit-0", ok == (code == 0), || {
                format!("{label} exited {code} with `ok` {ok}")
            });
        }
        if ok {
            return;
        }
        c.holds(
            "no-data-on-refusal",
            !(code == Some(2) && document.get("data").is_some()),
            || format!("{label} was refused (exit 2) and carries `data`"),
        );
        let Some(registry) = &self.fixture.registry else {
            return;
        };
        let Some(error_code) = document.pointer("/error/code").and_then(Value::as_str) else {
            return;
        };
        let row = registry.iter().find(|row| row.code == error_code);
        c.holds("code-registered", row.is_some(), || {
            format!("{label} failed with {error_code}, which is not in the fixture's registry")
        });
        let Some(row) = row else { return };
        if let Some(code) = code {
            c.holds("exit-is-registered", i64::from(code) == row.exit, || {
                format!(
                    "{label} failed with {error_code} and exited {code}, the registry says {}",
                    row.exit
                )
            });
        }
        if let Some(retryable) = document
            .pointer("/error/retryable")
            .and_then(Value::as_bool)
        {
            c.holds("retryable-is-registered", retryable == row.retryable, || format!("{label} failed with {error_code} and `retryable` {retryable}, the registry says {}", row.retryable));
        }
    }

    /// A refusal (exit 2) changes nothing in the working directory or in `HOME` (spec 8.1, 8.2).
    fn refusal_effects(&'a self) -> Finding {
        let mut c = self.check("L0.refusal-effects");
        for (index, outcome) in self.log.iter().enumerate() {
            if outcome.exit != Exit::Code(2) {
                continue;
            }
            c.holds(
                "refusal-changes-nothing",
                outcome.changes.is_empty(),
                || {
                    format!(
                        "`{}` was refused (exit 2) and changed {}",
                        outcome.call.argv.join(" "),
                        outcome.changes.join(", ")
                    )
                },
            );
            if !outcome.changes.is_empty() {
                c.observe(index);
            }
        }
        c.finish(true)
    }

    /// The envelopes validate against the contract's schema, `null` in `meta` and `error` included. What is under `meta.next` and `meta.page` is `L0.next`'s and `L0.page`'s.
    fn no_null(&'a self) -> Finding {
        let mut c = self.check("L0.no-null");
        for (index, document) in self.envelopes() {
            let found: Vec<String> = schemas()
                .envelope_problem_list(document)
                .into_iter()
                .filter(|problem| {
                    !problem.is_under("/meta/next") && !problem.is_under("/meta/page")
                })
                .map(|problem| problem.to_string())
                .collect();
            c.holds("valid-envelope", found.is_empty(), || {
                format!(
                    "`{}` printed an envelope the schema rejects: {}",
                    self.at(index).call.argv.join(" "),
                    found.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
                )
            });
            if !found.is_empty() {
                c.observe(index);
            }
        }
        c.finish(true)
    }

    fn compact(&'a self) -> Finding {
        let mut c = self.check("L0.compact");
        for (index, _) in self.envelopes() {
            let text = String::from_utf8_lossy(&self.at(index).stdout);
            let indented = first_document_text(&text).contains('\n');
            c.holds("no-indentation", !indented, || {
                format!(
                    "`{}` printed an indented document",
                    self.at(index).call.argv.join(" ")
                )
            });
            if indented {
                c.observe(index);
            }
        }
        c.finish(true)
    }

    fn contract(&'a self) -> Finding {
        let mut c = self.check("L0.contract");
        for (index, document) in self.envelopes() {
            let present = document.pointer("/meta/contract").is_some();
            c.holds("meta-contract", present, || {
                format!(
                    "`{}` printed an envelope without meta.contract",
                    self.at(index).call.argv.join(" ")
                )
            });
            if !present {
                c.observe(index);
            }
        }
        c.finish(true)
    }

    /// Spec 5.1: the whole stdout of the plain probe, the unknown command and the bare call is one document and one newline.
    fn one_document(&'a self, calls: &Invocations) -> Finding {
        let mut c = self.check("L0.one-document");
        let probe = calls.probe.as_ref().map(|probe| probe.json_variations[0].1);
        for index in [Some(calls.unknown_command), Some(calls.bare), probe]
            .into_iter()
            .flatten()
        {
            // Output that holds no document at all is the owner of that invocation's to report.
            if self.documents[index].is_none() {
                continue;
            }
            c.observe(index);
            let problem = one_document(&self.at(index).stdout).err();
            c.holds("one-document-one-newline", problem.is_none(), || {
                format!(
                    "`{}`: {}",
                    self.at(index).call.argv.join(" "),
                    problem.clone().unwrap_or_default()
                )
            });
        }
        c.finish(true)
    }

    /// The probe's JSON output does not depend on the environment or a terminal (spec 4.3), and the probe does not wait for input (spec 5.5).
    fn no_env_switch(&'a self, calls: &Invocations) -> Finding {
        let mut c = self.check("L0.no-env-switch");
        let Some(probe) = &calls.probe else {
            return c.finish(false);
        };
        let plain_index = &probe.json_variations[0].1;
        c.own(*plain_index);
        let plain = self.envelope(*plain_index);
        c.holds("baseline-is-envelope", plain.is_some(), || {
            format!(
                "`{}` without a variation did not print an envelope",
                self.at(*plain_index).call.argv.join(" ")
            )
        });
        if let Some(plain) = plain {
            for (label, index) in &probe.json_variations[1..] {
                c.own(*index);
                let Some(varied) = self.envelope(*index) else {
                    c.holds("same-envelope-kind", false, || {
                        format!(
                            "with {label} the output is not an envelope, without it an envelope"
                        )
                    });
                    continue;
                };
                c.holds("same-envelope-kind", true, String::new);
                let same = |pointer: &str| plain.pointer(pointer) == varied.pointer(pointer);
                c.holds("same-result", same("/ok") && same("/error/code"), || {
                    format!(
                        "with {label} ok is {} and error.code {}, without it {} and {}",
                        show(varied.get("ok")),
                        show(varied.pointer("/error/code")),
                        show(plain.get("ok")),
                        show(plain.pointer("/error/code"))
                    )
                });
                c.holds("same-command", same("/meta/command"), || {
                    format!(
                        "with {label} meta.command is {}, without it {}",
                        show(varied.pointer("/meta/command")),
                        show(plain.pointer("/meta/command"))
                    )
                });
                c.holds("same-schema-version", same("/meta/schemaVersion"), || {
                    format!(
                        "with {label} meta.schemaVersion is {}, without it {}",
                        show(varied.pointer("/meta/schemaVersion")),
                        show(plain.pointer("/meta/schemaVersion"))
                    )
                });
                c.holds(
                    "same-data-keys",
                    data_keys(plain) == data_keys(varied),
                    || {
                        format!(
                            "with {label} the data keys are {:?}, without it {:?}",
                            data_keys(varied),
                            data_keys(plain)
                        )
                    },
                );
            }
        }
        c.observe(probe.prompt);
        let waited = self.at(probe.prompt).exit == Exit::TimedOut;
        c.holds("no-prompt", !waited, || format!("`{}` was still running {} seconds after it started with stdin on a terminal nobody types on: it waits for input", self.at(probe.prompt).call.argv.join(" "), self.limit(probe.prompt).as_secs_f64()));
        c.finish(true)
    }

    fn no_env_switch_text(&'a self, calls: &Invocations) -> Finding {
        let mut c = self.check("L0.no-env-switch-text");
        let Some(probe) = &calls.probe else {
            return c.finish(false);
        };
        let kind = |index: usize| self.envelope(index).is_some();
        let plain = kind(probe.text_variations[0].1);
        c.own(probe.text_variations[0].1);
        for (label, index) in &probe.text_variations[1..] {
            c.own(*index);
            let seen = kind(*index);
            let name = |is_envelope: bool| {
                if is_envelope {
                    "an envelope"
                } else {
                    "not an envelope"
                }
            };
            c.holds("same-envelope-kind", seen == plain, || {
                format!(
                    "with {label} the output is {}, without it {}",
                    name(seen),
                    name(plain)
                )
            });
        }
        c.finish(true)
    }

    /// Every `meta.next` entry the run saw (spec 6.8).
    fn next(&'a self) -> Finding {
        let mut c = self.check("L0.next");
        let mut exercised = false;
        for (index, document) in self.envelopes() {
            let Some(next) = document.pointer("/meta/next") else {
                continue;
            };
            exercised = true;
            c.observe(index);
            let label = format!("`{}`", self.at(index).call.argv.join(" "));
            let found: Vec<String> = schemas()
                .envelope_problem_list(document)
                .into_iter()
                .filter(|problem| problem.is_under("/meta/next"))
                .map(|problem| problem.to_string())
                .collect();
            c.holds("entries-valid", found.is_empty(), || {
                format!(
                    "{label}: meta.next is not valid: {}",
                    found.iter().take(2).cloned().collect::<Vec<_>>().join(", ")
                )
            });
            let tool = document
                .pointer("/meta/tool")
                .and_then(Value::as_str)
                .unwrap_or(&self.tool);
            for entry in next.as_array().into_iter().flatten() {
                let (Some(command), Some(argv)) = (
                    entry.get("command").and_then(Value::as_str),
                    entry.get("argv").and_then(Value::as_array),
                ) else {
                    continue;
                };
                let words: Vec<&str> = argv.iter().filter_map(Value::as_str).collect();
                let placeholder = words
                    .iter()
                    .find(|word| atip::has_placeholder(word) || **word == "…" || **word == "...");
                c.holds("no-placeholder", placeholder.is_none(), || {
                    format!(
                        "{label}: next argv holds the placeholder {:?}",
                        placeholder.unwrap_or(&"")
                    )
                });
                if self.at(index).call.json_mode() && !words.is_empty() {
                    let before: Vec<&&str> =
                        words.iter().take_while(|word| **word != "--").collect();
                    c.holds(
                        "json-flag",
                        before.iter().any(|word| ***word == *"--json"),
                        || {
                            format!(
                                "{label}: a next step of a JSON-mode call has no --json: {command}"
                            )
                        },
                    );
                }
                let mut expected = vec![tool.to_string()];
                expected.extend(words.iter().map(ToString::to_string));
                let split = atip::split_argv(command);
                c.holds("command-matches-argv", split == expected, || {
                    format!(
                        "{label}: next command {command:?} splits to {split:?}, not {expected:?}"
                    )
                });
            }
        }
        c.finish(exercised)
    }

    /// Every `meta.page` the run saw (spec 6.9): more items mean a next entry for the next page.
    fn page(&'a self) -> Finding {
        let mut c = self.check("L0.page");
        let mut exercised = false;
        for (index, document) in self.envelopes() {
            let Some(page) = document.pointer("/meta/page") else {
                continue;
            };
            exercised = true;
            c.observe(index);
            let label = format!("`{}`", self.at(index).call.argv.join(" "));
            let found: Vec<String> = schemas()
                .envelope_problem_list(document)
                .into_iter()
                .filter(|problem| problem.is_under("/meta/page"))
                .map(|problem| problem.to_string())
                .collect();
            c.holds("page-valid", found.is_empty(), || {
                format!(
                    "{label}: meta.page is not valid: {}",
                    found.iter().take(2).cloned().collect::<Vec<_>>().join(", ")
                )
            });
            if let Some(cursor) = page.get("nextCursor").and_then(Value::as_str) {
                let has_entry = document
                    .pointer("/meta/next")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| entry.get("argv").and_then(Value::as_array))
                    .any(|argv| {
                        let words: Vec<&str> = argv.iter().filter_map(Value::as_str).collect();
                        words
                            .windows(2)
                            .any(|pair| pair[0] == "--cursor" && pair[1] == cursor)
                            || words.contains(&format!("--cursor={cursor}").as_str())
                    });
                c.holds("next-cursor-entry", has_entry, || format!("{label}: meta.page.nextCursor is {cursor:?} but meta.next has no entry with --cursor {cursor}"));
            }
        }
        c.finish(exercised)
    }
}

/// The conditions of `L0.unknown-command` that read the document: what an `UNKNOWN_COMMAND` envelope must be.
/// Each is the condition's id, whether it holds and what is wrong when it does not.
pub fn unknown_command_conditions(document: &Value) -> Vec<(&'static str, bool, String)> {
    let code = document.pointer("/error/code").and_then(Value::as_str);
    let version = document
        .pointer("/meta/schemaVersion")
        .and_then(Value::as_str);
    vec![
        (
            "code",
            code == Some("UNKNOWN_COMMAND") && document.get("ok") == Some(&Value::Bool(false)),
            format!(
                "error.code is {}, expected UNKNOWN_COMMAND in a failure",
                code.unwrap_or("missing")
            ),
        ),
        (
            "expected",
            document
                .pointer("/error/expected")
                .and_then(Value::as_array)
                .is_some_and(|expected| !expected.is_empty()),
            "error.expected is missing or empty".to_string(),
        ),
        (
            "no-meta-command",
            document.pointer("/meta/command").is_none(),
            "meta.command is present although no word resolved".to_string(),
        ),
        (
            "schema-version-1-0",
            version == Some("1.0"),
            format!(
                "meta.schemaVersion is {}, expected \"1.0\"",
                version.unwrap_or("missing")
            ),
        ),
    ]
}

/// The problems of an `UNKNOWN_COMMAND` envelope, empty when it is one.
pub fn unknown_command_problems(document: &Value) -> Vec<String> {
    unknown_command_conditions(document)
        .into_iter()
        .filter(|(_, holds, _)| !holds)
        .map(|(_, _, why)| why)
        .collect()
}

/// The rules of `L0.ok-exit` that read a failure envelope against the registry: the code is registered, the process exit (when known) is the row's, `retryable` is the row's, and a refusal carries no `data`. Without a known exit the code's registered exit stands for it.
pub fn failure_problems(
    document: &Value,
    registry: &[RegistryRow],
    exit: Option<i32>,
) -> Vec<String> {
    let Some(code) = document.pointer("/error/code").and_then(Value::as_str) else {
        return Vec::new();
    };
    let Some(row) = registry.iter().find(|row| row.code == code) else {
        return vec![format!("failed with {code}, which is not in the registry")];
    };
    let mut problems = Vec::new();
    if let Some(exit) = exit {
        if i64::from(exit) != row.exit {
            problems.push(format!(
                "failed with {code} and exited {exit}, the registry says {}",
                row.exit
            ));
        }
    }
    if let Some(retryable) = document
        .pointer("/error/retryable")
        .and_then(Value::as_bool)
    {
        if retryable != row.retryable {
            problems.push(format!(
                "failed with {code} and `retryable` {retryable}, the registry says {}",
                row.retryable
            ));
        }
    }
    let refused = exit.map_or(row.exit == 2, |exit| exit == 2);
    if refused && document.get("data").is_some() {
        problems.push(format!("was refused ({code}, exit 2) and carries `data`"));
    }
    problems
}

/// Whether an exit status is one the contract allows: 0, 1, 2, a signal, or 128+n as a launcher reports a signal (spec 8.4, 8.6).
pub fn exit_is_allowed(exit: Exit) -> bool {
    match exit {
        Exit::Code(code) => (0..=2).contains(&code) || (129..=192).contains(&code),
        Exit::Signal(_) => true,
        Exit::TimedOut => false,
    }
}

/// The registry a tool printed with `schema --errors --json`: `Some(rows)` when the answer is a success whose `data.errors` is a list of rows, `None` when the tool does not offer it.
fn offered_registry(document: Option<&Value>) -> Option<Vec<RegistryRow>> {
    let document = document.filter(|document| document.get("ok") == Some(&Value::Bool(true)))?;
    let rows = document.pointer("/data/errors")?.as_array()?;
    Some(
        rows.iter()
            .filter_map(|row| {
                Some(RegistryRow {
                    code: row.get("code")?.as_str()?.to_string(),
                    exit: row.get("exit")?.as_i64()?,
                    retryable: row.get("retryable")?.as_bool()?,
                })
            })
            .collect(),
    )
}

/// How the fixture's registry and the tool's own differ, by code.
pub fn registry_drift(fixture: &[RegistryRow], offered: &[RegistryRow]) -> Vec<String> {
    let mut drift = Vec::new();
    for row in fixture {
        match offered.iter().find(|other| other.code == row.code) {
            None => drift.push(format!("{} is only in the fixture", row.code)),
            Some(other) if other != row => drift.push(format!(
                "{} is exit {} retryable {} in the fixture and exit {} retryable {} in the tool",
                row.code, row.exit, row.retryable, other.exit, other.retryable
            )),
            Some(_) => {}
        }
    }
    drift.extend(
        offered
            .iter()
            .filter(|row| !fixture.iter().any(|other| other.code == row.code))
            .map(|row| format!("{} is only in the tool", row.code)),
    );
    drift
}

/// Whether stderr holds the error or the result of a JSON-mode call: a line that is a JSON object with `ok`, `error` or `data`, or the error's code.
fn carries_error_or_result(stderr: &[u8], document: &Value) -> bool {
    let text = String::from_utf8_lossy(stderr);
    if text.trim().is_empty() {
        return false;
    }
    let as_document = text.lines().any(|line| {
        serde_json::from_str::<Value>(line.trim())
            .ok()
            .is_some_and(|value| {
                value.get("ok").is_some()
                    || value.get("error").is_some()
                    || value.get("data").is_some()
            })
    });
    let code = document
        .pointer("/error/code")
        .and_then(Value::as_str)
        .filter(|code| !code.is_empty());
    as_document || code.is_some_and(|code| text.contains(code))
}

/// The sorted top-level keys of an envelope's `data`.
fn data_keys(document: &Value) -> BTreeSet<&str> {
    document
        .get("data")
        .and_then(Value::as_object)
        .map(|data| data.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

/// An envelope as far as the checks need to know: an object with a boolean `ok` and an object `meta`. The schema judges the rest.
fn is_envelope(document: &Value) -> bool {
    document.get("ok").is_some_and(Value::is_boolean)
        && document.get("meta").is_some_and(Value::is_object)
}

/// The first JSON value of `stdout` (indentation allowed), when it starts with one.
fn first_document(stdout: &[u8]) -> Option<Value> {
    let text = std::str::from_utf8(stdout).ok()?;
    serde_json::Deserializer::from_str(text.trim_start())
        .into_iter::<Value>()
        .next()?
        .ok()
}

/// The text of the first JSON value of `text`; all of it when it holds none.
fn first_document_text(text: &str) -> &str {
    let trimmed = text.trim_start();
    let mut values = serde_json::Deserializer::from_str(trimmed).into_iter::<Value>();
    match values.next() {
        Some(Ok(_)) => &trimmed[..values.byte_offset()],
        _ => trimmed,
    }
}

/// Spec 5.1 and 5.4: UTF-8, one JSON document, one newline, nothing else.
fn one_document(stdout: &[u8]) -> Result<Value, String> {
    let text =
        std::str::from_utf8(stdout).map_err(|error| format!("stdout is not UTF-8: {error}"))?;
    let Some(body) = text.strip_suffix('\n') else {
        return Err(if text.is_empty() {
            "stdout is empty".to_string()
        } else {
            "stdout does not end with a newline".to_string()
        });
    };
    if body.ends_with(char::is_whitespace) {
        return Err("stdout ends with more than one newline".to_string());
    }
    serde_json::from_str(body)
        .map_err(|error| format!("stdout is not exactly one JSON document: {error}"))
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
        entry.insert(
            "env".to_string(),
            Value::Object(
                outcome
                    .call
                    .env
                    .iter()
                    .map(|(name, value)| (name.clone(), json!(value)))
                    .collect(),
            ),
        );
    }
    if outcome.call.pty {
        entry.insert("pty".to_string(), json!(true));
    }
    if outcome.call.stdin_pty {
        entry.insert("stdinPty".to_string(), json!(true));
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
    use super::{
        exit_is_allowed, first_document, first_document_text, ids, is_envelope, needs,
        one_document, Need, TABLE,
    };
    use crate::exec::Exit;
    use crate::report::Strength;

    #[test]
    fn atip_check_01_the_table_is_nineteen_checks_fifteen_must_and_four_should() {
        assert_eq!(ids().len(), 19);
        assert_eq!(
            TABLE
                .iter()
                .filter(|entry| entry.strength == Strength::Must)
                .count(),
            15
        );
        assert!(ids().iter().all(|id| id.starts_with("L0.")));
        let mut unique = ids().to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 19);
        assert_eq!(needs("L0.unknown-option"), [Need::Probe, Need::Registry]);
        assert_eq!(needs("L0.ok-exit"), [Need::Registry]);
        assert!(needs("L0.next").is_empty() && needs("L0.refusal-effects").is_empty());
    }

    #[test]
    fn atip_check_19_one_document_means_one_value_and_one_newline() {
        one_document(b"{\"a\":1}\n").expect("one document");
        one_document(b"{\n  \"a\": 1\n}\n").expect("indentation is allowed, L0.compact judges it");
        for bad in [
            &b""[..],
            b"{\"a\":1}",
            b"{\"a\":1}\n\n",
            b"{\"a\":1}\n{\"b\":2}\n",
            b"\xff\n",
            b"text\n",
        ] {
            one_document(bad).expect_err(&format!(
                "{:?} must be refused",
                String::from_utf8_lossy(bad)
            ));
        }
    }

    #[test]
    fn atip_check_15_an_envelope_needs_a_boolean_ok_and_a_meta_object() {
        assert!(is_envelope(
            &first_document(b"{\"ok\":false,\"meta\":{}}").expect("json")
        ));
        assert!(!is_envelope(
            &first_document(b"{\"ok\":\"yes\",\"meta\":{}}").expect("json")
        ));
        assert!(!is_envelope(&first_document(b"[1]").expect("json")));
        assert!(first_document(b"not json").is_none());
        let two = b"{\"ok\":true,\"meta\":{}}\n{\"ok\":false,\"meta\":{}}\n";
        assert!(
            is_envelope(&first_document(two).expect("the first of two documents")),
            "a second document is one-document's to report"
        );
        assert_eq!(
            first_document_text("{\n \"a\": 1\n}\n{\"b\":2}\n"),
            "{\n \"a\": 1\n}"
        );
    }

    #[test]
    fn atip_check_48_an_exit_is_allowed_when_it_is_0_1_2_a_signal_or_128_plus_a_signal() {
        for exit in [
            Exit::Code(0),
            Exit::Code(1),
            Exit::Code(2),
            Exit::Signal(15),
            Exit::Code(130),
            Exit::Code(143),
        ] {
            assert!(exit_is_allowed(exit), "{exit:?}");
        }
        for exit in [
            Exit::Code(3),
            Exit::Code(70),
            Exit::Code(101),
            Exit::Code(128),
            Exit::Code(255),
            Exit::Code(-1),
            Exit::TimedOut,
        ] {
            assert!(!exit_is_allowed(exit), "{exit:?}");
        }
    }
}
