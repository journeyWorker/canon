//! The L0 checks: the 28 of approaches/checker.md 6 ("L0", contract 1.1).
//!
//! A run first makes every invocation the L0 table names, each in its own fresh copy of the fixture state, and
//! keeps what each one did in a log. Each check then reads the log.
//!
//! **One owner per pass condition.** A check is a list of *conditions*, each with an id (`version/exit-0`). A
//! condition is judged by exactly one check, so breaking one rule fails one check: the checks that would need
//! the same fact skip what is not theirs (an envelope that does not parse is `L0.unknown-command`'s, `L0.bare`'s
//! or `L0.json-mode-preparse`'s to report, and the others read only the documents that do). A condition whose
//! subject was not observed is not judged. The rules that read one document are in [`rules`].
//!
//! **Needs.** A check declares the fixture entries it needs (`probe`, `registry`, `commands`). A violation it
//! observed is a `fail` whatever the fixture lacks; a check that found nothing wrong but lacked an entry is
//! `unprovided`.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::{json, Map, Value};

use crate::claim::Claim;
use crate::cli::Level;
use crate::exec::{Call, Exit, Outcome, RunError, Runner};
use crate::fixture::{Fixture, RegistryRow};
use crate::report::{Check, CheckResult, Detail, Report, Strength};
use crate::schema::schemas;

pub mod documents;
pub mod rules;
mod l1;
mod l1_order;
mod l1_plan;
mod l2;

use l1_plan::L1Run;
use l2::L2Run;

/// A fixture entry a check needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// The id of a command that runs without state.
    Probe,
    /// The tool's error registry.
    Registry,
    /// The commands whose invocations the checker may run.
    Commands,
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

use Need::{Commands, Probe, Registry};

/// The L0 table, in the order of approaches/checker.md.
const TABLE: [Entry; 28] = [
    must("L0.version", &["4.6"], &[]),
    should("L0.version-format", &["4.6"], &[]),
    must("L0.unknown-command", &["4.4", "6.4", "6.6", "6.11", "7.2"], &[]),
    must("L0.unknown-command-text", &["5.3"], &[]),
    should("L0.text-error-form", &["7.4"], &[]),
    must("L0.bare", &["7.2", "9.2"], &[]),
    must("L0.unknown-option", &["4.4", "6.6", "9.9"], &[Probe]),
    must("L0.json-mode-preparse", &["4.4"], &[Probe]),
    must("L0.after-dashdash", &["4.4"], &[Probe]),
    must(
        "L0.ok-exit",
        &["6.2", "6.4", "7.3.2", "7.3.3", "8.1", "8.3.1", "8.4"],
        &[Registry],
    ),
    must("L0.no-null", &["6.5"], &[]),
    must("L0.envelope", &["6.1-6.6"], &[]),
    should("L0.compact", &["5.4"], &[]),
    must("L0.one-document", &["5.1"], &[Probe]),
    must("L0.no-env-switch", &["4.3"], &[Probe]),
    must("L0.no-env-switch-text", &["4.3"], &[Probe]),
    should("L0.contract", &["6.6"], &[]),
    must(
        "L0.contract-version",
        &["6.6", "10.1.4", "10.13 rule 16", "15.7", "15.8"],
        &[],
    ),
    must("L0.stderr-result", &["5.2"], &[]),
    should("L0.stderr-quiet", &["5.2"], &[]),
    must("L0.next", &["6.8"], &[]),
    must("L0.page", &["6.9"], &[Commands]),
    must("L0.refusal-effects", &["8.2", "12.2"], &[]),
    must("L0.no-prompt", &["5.5"], &[Commands]),
    must(
        "L0.registry",
        &["7.3.4", "7.5", "7.6.2", "17.3"],
        &[Registry],
    ),
    should("L0.usage-hint", &["7.1.3"], &[]),
    must("L0.hint-form", &["7.1.2"], &[]),
    should("L0.global-on-group", &["9.13"], &[Commands]),
];

/// The ids of the L0 checks, in table order.
pub fn ids() -> [&'static str; 28] {
    TABLE.each_ref().map(|entry| entry.id)
}

/// The ids of the L1 checks, in table order.
pub fn l1_ids() -> [&'static str; 31] {
    l1::ids()
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
/// The value no cursor has: the probe for a command that does not page.
const NO_SUCH_CURSOR: &str = "x";
/// The variations of the environment under which the output kind must not change (spec 4.3), each a variable and its value.
const ENV_VARIATIONS: [(&str, &str); 5] = [
    ("CI", "1"),
    ("CLAUDECODE", "1"),
    ("OUTPUT_FORMAT", "text"),
    ("NO_COLOR", "1"),
    ("TERM", "dumb"),
];

/// What a check concluded, and the log entries it rests on.
struct Finding {
    result: CheckResult,
    reason: Option<String>,
    /// Everything the check found, whole (`fail` and `unprovided`); `reason` is its first messages, cut.
    details: Vec<Detail>,
    /// Whether `reason` leaves part of `details` out.
    cut: bool,
    advisories: Vec<String>,
    observed: Vec<usize>,
    /// Facts the check found that do not change its result.
    notes: Vec<String>,
}

/// The most problems a reason lists, and the most characters of one.
const REASON_PROBLEMS: usize = 3;
const PROBLEM_CHARS: usize = 240;

/// The reason of a failed check: the first problems, each cut short, and how many more there are. Whether it left something out.
fn reason_and_cut(problems: &[String]) -> (String, bool) {
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
    let left_out = problems.len() > REASON_PROBLEMS || problems.iter().take(REASON_PROBLEMS).any(|problem| problem.chars().count() > PROBLEM_CHARS);
    (reason, left_out)
}

/// `argv` in JSON mode: with `--json` before any `--`.
fn in_json_mode(argv: &[String]) -> Vec<String> {
    if atip::wants_json(argv) {
        return argv.to_vec();
    }
    let mut words = argv.to_vec();
    match words.iter().position(|word| word == "--") {
        Some(at) => words.insert(at, "--json".to_string()),
        None => words.push("--json".to_string()),
    }
    words
}

/// `argv` with `--cursor <NO_SUCH_CURSOR>` in place of any cursor it has, in JSON mode.
fn with_cursor(argv: &[String]) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut skip = false;
    let mut after_dashdash = false;
    for word in argv {
        if skip {
            skip = false;
            continue;
        }
        if !after_dashdash && word == "--cursor" {
            skip = true;
            continue;
        }
        if word == "--" {
            after_dashdash = true;
        }
        words.push(word.clone());
    }
    let at = words.iter().position(|word| word == "--").unwrap_or(words.len());
    words.splice(at..at, ["--cursor".to_string(), NO_SUCH_CURSOR.to_string()]);
    in_json_mode(&words)
}

/// The invocations of one run: where each named one sits in the log.
struct Invocations {
    version: usize,
    unknown_command: usize,
    unknown_command_text: usize,
    bare: usize,
    /// `schema --errors --json`.
    schema_errors: usize,
    /// `schema --error <CODE> --json` for each code the tool's registry lists.
    error_entries: Vec<(String, usize)>,
    /// `schema --warnings --json`.
    schema_warnings: usize,
    /// `schema --warning <CODE> --json` for each code the tool's warning registry lists.
    warning_entries: Vec<(String, usize)>,
    /// `schema --error <CODE>` for each code only the warning registry lists.
    warning_only: Vec<(String, usize)>,
    probe: Option<ProbeCalls>,
    commands: Vec<CommandCalls>,
    group: Option<GroupCalls>,
    /// `schema --json`: the tool's tree of commands, when it offers one (the groups it has).
    schema_tree: usize,
    /// What the probe reports in an untouched copy, twice the same; `None` when it does not (or there is no probe).
    remote_baseline: Option<Value>,
    /// The one run of each `notRun` command, and the runs of those commands that were skipped.
    served: Vec<ServedRun>,
    skipped: Vec<Skip>,
}

/// The one run of a `notRun` command (approaches/checker.md 5): its first fixture `ok` invocation, else its first example, with `--json`, read up to the
/// end of its first JSON document and ended by a signal. `call` is `None` when the command has neither.
#[derive(Debug, Clone)]
pub(crate) struct ServedRun {
    pub(crate) command: String,
    pub(crate) call: Option<usize>,
}

/// A run of a `notRun` command that the checker did not make because it would succeed and so never end, reported by the check it would have fed
/// (`check` empty: by every check that runs commands).
#[derive(Debug, Clone)]
pub(crate) struct Skip {
    pub(crate) check: &'static str,
    pub(crate) text: String,
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
}

/// The invocations of one command of the fixture's `commands`.
struct CommandCalls {
    ok: Vec<usize>,
    refused: Vec<usize>,
    missing: Vec<MissingCalls>,
    /// The fixture's `paged` invocations: each is meant to return `meta.page.nextCursor`.
    paged: Vec<usize>,
    /// The command once more with `--cursor x --json`.
    cursor: usize,
}

/// One `missingValue` invocation: the control run with stdin at end-of-file, and the run on a terminal.
struct MissingCalls {
    control: usize,
    terminal: Option<usize>,
}

/// The group that `L0.global-on-group` runs: bare, and with each tool-wide option.
struct GroupCalls {
    bare: usize,
    with_option: Vec<(Vec<String>, usize)>,
}

struct Run<'a> {
    runner: Runner,
    fixture: &'a Fixture,
    log: Vec<Outcome>,
    served: Vec<ServedRun>,
    skipped: Vec<Skip>,
}

impl Run<'_> {
    fn invoke(&mut self, call: Call) -> Result<usize, RunError> {
        let outcome = self.runner.run(&call)?;
        self.log.push(outcome);
        Ok(self.log.len() - 1)
    }

    /// The first JSON document an invocation printed, when it is a success whose `data.<key>` is a list of objects with a `code`.
    fn listed_codes(&self, index: usize, key: &str) -> Vec<String> {
        first_document(&self.log[index].stdout)
            .filter(|document| document.get("ok") == Some(&Value::Bool(true)))
            .and_then(|document| document.pointer(&format!("/data/{key}")).cloned())
            .and_then(|list| list.as_array().cloned())
            .into_iter()
            .flatten()
            .filter_map(|row| row.get("code").and_then(Value::as_str).map(str::to_string))
            .collect()
    }

    fn invoke_all(&mut self) -> Result<Invocations, RunError> {
        let remote_baseline = self.runner.remote_baseline()?;
        let version = self.invoke(Call::new(["--version"]))?;
        let unknown_command = self.invoke(Call::new([NO_SUCH_COMMAND, "--json"]))?;
        let unknown_command_text = self.invoke(Call::new([NO_SUCH_COMMAND]))?;
        let bare = self.invoke(Call::new(["--json"]))?;
        let schema_errors = self.invoke(Call::new(["schema", "--errors", "--json"]))?;
        let error_codes = self.listed_codes(schema_errors, "errors");
        let mut error_entries = Vec::new();
        for code in &error_codes {
            let index = self.invoke(Call::new(["schema", "--error", code, "--json"]))?;
            error_entries.push((code.clone(), index));
        }
        let schema_warnings = self.invoke(Call::new(["schema", "--warnings", "--json"]))?;
        let warning_codes = self.listed_codes(schema_warnings, "warnings");
        let mut warning_entries = Vec::new();
        let mut warning_only = Vec::new();
        for code in &warning_codes {
            let index = self.invoke(Call::new(["schema", "--warning", code, "--json"]))?;
            warning_entries.push((code.clone(), index));
            if !error_codes.contains(code) {
                let index = self.invoke(Call::new(["schema", "--error", code, "--json"]))?;
                warning_only.push((code.clone(), index));
            }
        }
        let probe = match self.fixture.probe.clone() {
            None => None,
            Some(words) => Some(self.invoke_probe(&words)?),
        };
        let commands = self.invoke_commands()?;
        // A `notRun` command without an `ok` invocation has no run yet: at L1 its first example gives one.
        for id in self.fixture.not_run.clone() {
            if self.served.iter().all(|served| served.command != id) {
                self.served.push(ServedRun { command: id, call: None });
            }
        }
        let group = self.invoke_group()?;
        let schema_tree = self.invoke(Call::new(["schema", "--json"]))?;
        Ok(Invocations {
            version,
            unknown_command,
            unknown_command_text,
            bare,
            schema_errors,
            error_entries,
            schema_warnings,
            warning_entries,
            warning_only,
            probe,
            commands,
            group,
            schema_tree,
            remote_baseline,
            served: Vec::new(),
            skipped: Vec::new(),
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
        Ok(ProbeCalls {
            unknown_option,
            json_first,
            after_dashdash,
            json_variations,
            text_variations,
        })
    }

    fn invoke_commands(&mut self) -> Result<Vec<CommandCalls>, RunError> {
        let Some(commands) = self.fixture.commands.clone() else {
            return Ok(Vec::new());
        };
        let mut all = Vec::new();
        for (id, entry) in &commands {
            let mut calls = CommandCalls {
                ok: Vec::new(),
                refused: Vec::new(),
                missing: Vec::new(),
                paged: Vec::new(),
                cursor: 0,
            };
            if self.fixture.not_run.contains(id) {
                // A command that does not end on its own: its first `ok` invocation runs once, up to its first document; the rest would succeed and never end.
                if let Some(argv) = entry.ok.first() {
                    let call = self.invoke_served(id, argv, None)?;
                    self.served.push(ServedRun { command: id.clone(), call: Some(call) });
                }
                for argv in entry.ok.iter().skip(1) {
                    self.skip("", id, format!("`{}` (a further ok)", in_json_mode(argv).join(" ")));
                }
                for argv in &entry.paged {
                    self.skip("L0.page", id, format!("`{}` (paged)", in_json_mode(argv).join(" ")));
                    self.skip("L1.paging", id, format!("`{}` (paged)", in_json_mode(argv).join(" ")));
                }
            } else {
                for argv in &entry.ok {
                    calls.ok.push(self.invoke(Call::new(in_json_mode(argv)))?);
                }
                for argv in &entry.paged {
                    calls.paged.push(self.invoke(Call::new(in_json_mode(argv)))?);
                }
            }
            for argv in &entry.refused {
                calls.refused.push(self.invoke(Call::new(in_json_mode(argv)))?);
            }
            for argv in &entry.missing_value {
                let control = self.invoke(Call::new(in_json_mode(argv)))?;
                let terminal = if self.log[control].exit == Exit::Code(2) {
                    Some(self.invoke_on_terminal(argv, self.log[control].elapsed)?)
                } else {
                    None
                };
                calls.missing.push(MissingCalls { control, terminal });
            }
            let base: Vec<String> = entry
                .ok
                .first()
                .cloned()
                .unwrap_or_else(|| id.split(' ').map(str::to_string).collect());
            calls.cursor = self.invoke(Call::new(with_cursor(&base)))?;
            all.push(calls);
        }
        Ok(all)
    }

    /// Records a run of the `notRun` command `command` that the checker did not make.
    fn skip(&mut self, check: &'static str, command: &str, what: String) {
        self.skipped.push(Skip { check, text: format!("{command}: {what}: notRun") });
    }

    /// The one run of the `notRun` command `id`: `argv` with `--json`, until its first JSON document, then ended by a signal. A command that ends on its
    /// own, before or after that document, is a fixture error; with its document `doc`, so is a refusal with a code it declares on a constraint or a validator.
    fn invoke_served(&mut self, id: &str, argv: &[String], doc: Option<&crate::l1::model::CommandDoc>) -> Result<usize, RunError> {
        let index = self.invoke(Call::new(in_json_mode(argv)).until_first_document())?;
        if self.log[index].ended_alone && served_precondition(&self.log[index], self.fixture.registry.as_deref(), doc).is_none() {
            return Err(RunError::Fixture(format!(
                "`notRun` names {id}, and `{}` ended on its own ({}) within the time limit: a command that ends belongs in no `notRun`",
                self.log[index].call.argv.join(" "),
                exit_text(self.log[index].exit)
            )));
        }
        Ok(index)
    }

    /// The `missingValue` invocation on a terminal: it has `control_elapsed` x 2 + 2 seconds, at most the fixture's limit.
    fn invoke_on_terminal(&mut self, argv: &[String], control_elapsed: Duration) -> Result<usize, RunError> {
        let deadline = (control_elapsed * 2 + Duration::from_secs(2)).min(self.fixture.timeout);
        let mut call = Call::new(in_json_mode(argv)).on_terminal();
        call.limit = Some(deadline);
        let outcome = self.runner.run_on_terminal(&call, deadline)?;
        self.log.push(outcome);
        Ok(self.log.len() - 1)
    }

    fn invoke_group(&mut self) -> Result<Option<GroupCalls>, RunError> {
        let (Some(group), Some(options)) = (self.fixture.group(), self.fixture.global_options.clone()) else {
            return Ok(None);
        };
        let bare = self.invoke(Call::new([group.as_str(), "--json"]))?;
        let mut with_option = Vec::new();
        for option in options {
            let argv: Vec<String> = std::iter::once(group.clone())
                .chain(option.iter().cloned())
                .chain(std::iter::once("--json".to_string()))
                .collect();
            let index = self.invoke(Call::new(argv))?;
            with_option.push((option, index));
        }
        Ok(Some(GroupCalls { bare, with_option }))
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

/// Runs every check of `level` against the fixture's state and reports each.
pub(crate) fn run_checks(
    runner: Runner,
    fixture: &Fixture,
    claim: Claim,
    bin_name: &str,
    mutation: Mutation<'_>,
    level: Level,
) -> Result<Judged, RunError> {
    let mut run = Run {
        runner,
        fixture,
        log: Vec::new(),
        served: Vec::new(),
        skipped: Vec::new(),
    };
    let mut calls = run.invoke_all()?;
    let l1 = if level != Level::L0 {
        let warnings_offered = claim.at_least(Claim::V1_1)
            && first_document(&run.log[calls.schema_warnings].stdout)
                .is_some_and(|document| document.get("ok") == Some(&Value::Bool(true)) && document.pointer("/data/warnings").is_some_and(Value::is_array));
        Some(run.invoke_l1(warnings_offered, claim)?)
    } else {
        None
    };
    calls.served = std::mem::take(&mut run.served);
    calls.skipped = std::mem::take(&mut run.skipped);
    let l2 = if level == Level::L2 { Some(run.invoke_l2(&calls, l1.as_ref(), claim)?) } else { None };
    let effects = run.plan_effects(l1.as_ref())?;
    let judge = Judge::new(&run.log, fixture, claim, &calls, bin_name, mutation, Runs { l1: l1.as_ref(), l2: l2.as_ref(), effects: &effects });
    let mut findings = vec![
        judge.version(),
        judge.version_format(),
        judge.unknown_command(),
        judge.unknown_command_text(),
        judge.text_error_form(),
        judge.bare(),
        judge.unknown_option(),
        judge.json_mode_preparse(),
        judge.after_dashdash(),
        judge.ok_exit(),
        judge.no_null(),
        judge.envelope_check(),
        judge.compact(),
        judge.one_document(),
        judge.no_env_switch(),
        judge.no_env_switch_text(),
        judge.contract(),
        judge.contract_version(),
        judge.stderr_result(),
        judge.stderr_quiet(),
        judge.next(),
        judge.page(),
        judge.refusal_effects(),
        judge.no_prompt(),
        judge.registry(),
        judge.usage_hint(),
        judge.hint_form(),
        judge.global_on_group(),
    ];
    let mut table: Vec<&Entry> = TABLE.iter().collect();
    if l1.is_some() {
        findings.extend(judge.l1_findings());
        table.extend(l1::table(claim));
    }
    if l2.is_some() {
        findings.extend(judge.l2_findings());
        table.extend(l2::table());
    } else {
        // `L2.effects` is advisory at L0 and L1: no level gains a rule of a higher level (spec 3.2).
        findings.push(judge.l2_effects(&effects));
        table.push(l2::effects_entry());
    }
    let checks = table
        .into_iter()
        .zip(findings)
        .map(|(entry, finding)| {
            let finding = if mutation.forced_pass.contains(&entry.id) {
                Finding {
                    result: CheckResult::Pass,
                    reason: None,
                    details: Vec::new(),
                    cut: false,
                    ..finding
                }
            } else {
                finding
            };
            Check {
                id: entry.id,
                strength: if (entry.id == "L2.input-usage" && !claim.at_least(Claim::V1_1)) || (entry.id == l2::EFFECTS_ID && level != Level::L2) { Strength::Should } else { entry.strength },
                result: finding.result,
                rules: entry.rules.to_vec(),
                reason: finding.reason,
                advisories: finding.advisories,
                findings: finding.details,
                cut: finding.cut,
                notes: finding.notes,
                unlisted: if matches!(entry.id, "L1.constraints" | "L1.domains" | "L1.validators") {
                    l1.as_ref().map(|l1| l1.unprobed_writes.clone()).unwrap_or_default()
                } else {
                    l2.as_ref().map(|l2| l2::unlisted(entry.id, l2)).unwrap_or_default()
                },
                not_run: if entry.needs.contains(&Commands) || entry.id == "L1.next" { fixture.not_run.clone() } else { Vec::new() },
                skipped: calls.skipped.iter().filter(|skip| skip.check.is_empty() && entry.needs.contains(&Commands) || skip.check == entry.id).map(|skip| skip.text.clone()).collect(),
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
        report: Report::new(match level { Level::L0 => "L0", Level::L1 => "L1", Level::L2 => "L2" }, claim, checks).with_prepare(fixture.prepare.clone()),
        conditions,
    })
}

/// How a check ends when nothing it judged failed.
enum End {
    /// The check exercised its subject (`true`), or saw nothing of it (`false`: it does not apply). A missing fixture entry it needs makes it `unprovided`.
    Exercised(bool),
    /// The check cannot tell without something the fixture lacks: the reason (cut as `reason_and_cut` cuts, or whole), and the fixture entries that would give it.
    Unprovided { reason: String, cut: bool, entries: Vec<String> },
    /// The check does not apply, for this reason.
    Unsupported(String),
    /// The check observed its subject, so it needs no fixture entry to say so: nothing it judged failed.
    Observed,
    /// Every rule the check judges is entailed by others that passed: the rules, and what each is entailed by.
    Entailed(Vec<String>),
}

impl End {
    /// The check lacks one fixture entry, named `entry` as the fixture spells it (`commands`, `commands.<id>.paged`).
    fn missing(entry: impl Into<String>, why: impl Into<String>) -> Self {
        Self::Unprovided {
            reason: why.into(),
            cut: false,
            entries: vec![entry.into()],
        }
    }

    /// The check lacks several, each with its `why`: the reason is those, cut as every reason is.
    fn missing_all(whys: Vec<String>, entries: Vec<String>) -> Self {
        let (reason, cut) = reason_and_cut(&whys);
        Self::Unprovided { reason, cut, entries }
    }
}

/// The conditions of one check being judged: what fails, what was looked at, what is only recommended.
struct Conditions<'j> {
    judge: &'j Judge<'j>,
    entry: &'static Entry,
    name: &'static str,
    problems: Vec<Detail>,
    /// The invocations the fixture lists to succeed that a precondition refused: what the check could not judge for them.
    refused: Vec<Detail>,
    /// The fixture entries the check lacks to judge the one run of a `notRun` command: the entry, and what is missing.
    lacking: Vec<(String, String)>,
    advisories: Vec<String>,
    observed: Vec<usize>,
    /// Facts found that do not change the result.
    notes: Vec<String>,
    /// The invocations this check made for itself: one that runs over the time limit is this check's failure.
    owned: Vec<usize>,
}

impl<'j> Conditions<'j> {
    /// Judges condition `condition`: `holds` says whether it did, `why` says what is wrong when it did not. The finding has no `argv`:
    /// no one invocation decided it.
    fn holds(&mut self, condition: &'static str, holds: bool, why: impl FnOnce() -> String) {
        self.holds_in(None, condition, holds, why);
    }

    /// Like [`Conditions::holds`], for a condition the invocation `index` of the log decided: the finding carries its argv.
    fn holds_at(&mut self, index: usize, condition: &'static str, holds: bool, why: impl FnOnce() -> String) {
        self.holds_in(Some(index), condition, holds, why);
    }

    /// The invocation `index` of `command`, which the fixture lists to succeed, was refused by a precondition (not a stateless refusal): the check records
    /// it and the caller leaves the invocation unjudged. `true` when it was.
    fn skips_refused(&mut self, command: &str, index: usize) -> bool {
        match self.judge.fixture_refusal(command, index) {
            Some(Refusal::Precondition(code)) => {
                self.refused_by_precondition(index, &code);
                true
            }
            _ => false,
        }
    }

    fn refused_by_precondition(&mut self, index: usize, code: &str) {
        if self.refused.iter().any(|found| found.argv.as_deref() == Some(self.judge.at(index).call.argv.as_slice())) {
            return;
        }
        self.observe(index);
        let argv = self.judge.at(index).call.argv.clone();
        let message = format!(
            "`{}` was refused with {code} (exit 2), a precondition: the state or the environment the fixture gives did not give the command what it needs",
            argv.join(" ")
        );
        self.refused.push(Detail::new("refused-precondition", message).with_argv(&argv).with_code(code));
    }

    /// Judges condition `condition` that fails once for each of `problems` (none: it holds): one finding each, whole.
    /// `index` is the invocation that decided them, when one did.
    fn holds_each(&mut self, index: Option<usize>, condition: &'static str, problems: Vec<String>) {
        self.holds_each_at(condition, problems.into_iter().map(|problem| (index, problem)).collect());
    }

    /// Like [`Conditions::holds_each`], each problem with the invocation that decided it.
    fn holds_each_at(&mut self, condition: &'static str, problems: Vec<(Option<usize>, String)>) {
        let id = self.see(condition);
        if self.judge.mutation.skipped_conditions.contains(&id.as_str()) {
            return;
        }
        for (index, problem) in problems {
            let detail = self.detail(index, condition, problem);
            self.problems.push(detail);
        }
    }

    /// Like [`Conditions::holds`], for a condition decided by the invocation `index` of the log, when one decided it.
    fn holds_in(&mut self, index: Option<usize>, condition: &'static str, holds: bool, why: impl FnOnce() -> String) {
        let id = self.see(condition);
        let skipped = self.judge.mutation.skipped_conditions.contains(&id.as_str());
        if !holds && !skipped {
            let detail = self.detail(index, condition, why());
            self.problems.push(detail);
        }
    }

    /// Records that the check judged `condition`, and its id.
    fn see(&self, condition: &'static str) -> String {
        let id = format!("{}/{condition}", self.name);
        self.judge.seen.borrow_mut().insert(id.clone());
        id
    }

    fn detail(&self, index: Option<usize>, kind: &'static str, message: String) -> Detail {
        let detail = Detail::new(kind, message);
        match index {
            Some(index) => detail.with_argv(&self.judge.at(index).call.argv),
            None => detail,
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

    fn finish(self, exercised: bool) -> Finding {
        self.finish_as(End::Exercised(exercised))
    }

    /// The finding: a run over the time limit or a violated condition is a fail; else `end` decides: a missing
    /// fixture entry is unprovided, a check that saw nothing of its subject does not apply.
    fn finish_as(self, end: End) -> Finding {
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
            let (cut_reason, cut) = reason_and_cut(std::slice::from_ref(&reason));
            return Finding {
                result: CheckResult::Fail,
                details: vec![Detail::new("ran-over-time-limit", reason).with_argv(&self.judge.at(*index).call.argv)],
                reason: Some(cut_reason),
                cut,
                advisories: self.advisories,
                observed: self.observed,
                notes: self.notes,
            };
        }
        if !self.problems.is_empty() {
            let messages: Vec<String> = self.problems.iter().map(|problem| problem.message.clone()).collect();
            let (reason, cut) = reason_and_cut(&messages);
            return Finding {
                result: CheckResult::Fail,
                reason: Some(reason),
                details: self.problems,
                cut,
                advisories: self.advisories,
                observed: self.observed,
                notes: self.notes,
            };
        }
        if !self.refused.is_empty() {
            // Nothing it judged failed, and what it was to judge was refused for want of state or environment: it cannot tell, which fails a MUST level.
            let messages: Vec<String> = self.refused.iter().map(|found| found.message.clone()).collect();
            let (reason, cut) = reason_and_cut(&messages);
            return Finding {
                result: CheckResult::Unprovided,
                reason: Some(reason),
                details: self.refused,
                cut,
                advisories: self.advisories,
                observed: self.observed,
                notes: self.notes,
            };
        }
        let unprovided = |reason: String, cut: bool, entries: Vec<String>, advisories: Vec<String>, notes: Vec<String>| Finding {
            result: CheckResult::Unprovided,
            reason: Some(reason),
            details: entries.into_iter().map(|entry| Detail::new("missing-fixture-entry", entry)).collect(),
            cut,
            advisories,
            observed: Vec::new(),
            notes,
        };
        if !self.lacking.is_empty() {
            // A `notRun` command with neither an `ok` invocation nor (from L1) an example has no run to judge.
            let (reason, cut) = reason_and_cut(&self.lacking.iter().map(|(_, why)| why.clone()).collect::<Vec<_>>());
            let mut entries: Vec<String> = self.lacking.iter().map(|(entry, _)| entry.clone()).collect();
            entries.dedup();
            return unprovided(reason, cut, entries, self.advisories, self.notes);
        }
        let quiet = |result, reason, observed, advisories, notes| Finding {
            result,
            reason,
            details: Vec::new(),
            cut: false,
            advisories,
            observed,
            notes,
        };
        match end {
            End::Unprovided { reason, cut, entries } => return unprovided(reason, cut, entries, self.advisories, self.notes),
            End::Unsupported(reason) => return quiet(CheckResult::Unsupported, Some(reason), self.observed, self.advisories, self.notes),
            End::Observed => return quiet(CheckResult::Pass, None, self.observed, self.advisories, self.notes),
            End::Entailed(rules) => {
                let mut notes = self.notes;
                notes.extend(rules);
                return quiet(CheckResult::Entailed, None, self.observed, self.advisories, notes);
            }
            End::Exercised(_) => {}
        }
        let missing: Vec<(&str, &str)> = self
            .entry
            .needs
            .iter()
            .filter(|need| match need {
                Probe => self.judge.fixture.probe.is_none(),
                Registry => self.judge.rows.is_none(),
                Commands => self.judge.fixture.commands.is_none(),
            })
            .map(|need| match need {
                Probe => ("probe", "`probe` (the id of a command that runs without state)"),
                Registry => ("registry", "`registry` (the tool's error codes)"),
                Commands => ("commands", "`commands` (invocations of the tool's commands)"),
            })
            .collect();
        if !missing.is_empty() {
            let why = format!("the fixture has no {}", missing.iter().map(|(_, text)| *text).collect::<Vec<_>>().join(" and "));
            return unprovided(why, false, missing.iter().map(|(name, _)| (*name).to_string()).collect(), self.advisories, self.notes);
        }
        let exercised = matches!(end, End::Exercised(true));
        // A check whose every run was a `notRun` command's that the checker skipped has nothing to judge, and listing commands must not pass a level.
        let skipped: Vec<&Skip> = self.judge.calls.skipped.iter().filter(|skip| skip.check == self.entry.id).collect();
        if !exercised && !skipped.is_empty() {
            let reason = format!("every run this check would make is that of a command in `notRun`: {}", skipped.iter().take(3).map(|skip| skip.text.as_str()).collect::<Vec<_>>().join("; "));
            return Finding {
                result: CheckResult::Unprovided,
                reason: Some(reason),
                details: vec![Detail::new("all-runs-not-run", "notRun".to_string())],
                cut: skipped.len() > 3,
                advisories: self.advisories,
                observed: self.observed,
                notes: self.notes,
            };
        }
        let reason = (!exercised)
            .then(|| "no envelope the run saw carries what this check judges".to_string());
        quiet(
            if exercised {
                CheckResult::Pass
            } else {
                CheckResult::Unsupported
            },
            reason,
            self.observed,
            self.advisories,
            self.notes,
        )
    }
}

struct Judge<'a> {
    log: &'a [Outcome],
    fixture: &'a Fixture,
    claim: Claim,
    calls: &'a Invocations,
    /// The first document of each invocation's stdout, when it parses.
    documents: Vec<Option<Value>>,
    tool: String,
    mutation: Mutation<'a>,
    seen: RefCell<BTreeSet<String>>,
    /// The entries of `schema --errors --json`, when the tool offers it.
    tool_errors: Option<Vec<Value>>,
    /// The entries of `schema --warnings --json`, when the tool offers it and claims 1.1.
    tool_warnings: Option<Vec<Value>>,
    /// The registry a code is looked up in: the tool's own when it prints one, else the fixture's.
    rows: Option<Vec<RegistryRow>>,
    /// What the L1 invocations read, for an L1 run.
    l1: Option<&'a L1Run>,
    /// What the L2 invocations read, for an L2 run.
    l2: Option<&'a L2Run>,
    /// What `L2.effects` reads, at every level.
    effects: &'a l2::EffectsRun,
}

/// What the invocations of the levels above L0 read, handed to the judge.
struct Runs<'a> {
    l1: Option<&'a L1Run>,
    l2: Option<&'a L2Run>,
    effects: &'a l2::EffectsRun,
}

impl<'a> Judge<'a> {
    fn new(log: &'a [Outcome], fixture: &'a Fixture, claim: Claim, calls: &'a Invocations, bin_name: &str, mutation: Mutation<'a>, runs: Runs<'a>) -> Self {
        let Runs { l1, l2, effects } = runs;
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
        let listed = |index: usize, key: &str| -> Option<Vec<Value>> {
            let document = documents[index].as_ref()?;
            if document.get("ok") != Some(&Value::Bool(true)) || !is_envelope(document) {
                return None;
            }
            document
                .pointer(&format!("/data/{key}"))?
                .as_array()
                .cloned()
        };
        let tool_errors = listed(calls.schema_errors, "errors");
        let tool_warnings = listed(calls.schema_warnings, "warnings").filter(|_| claim.at_least(Claim::V1_1));
        let tool_rows = tool_errors.as_ref().map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    Some(RegistryRow {
                        code: entry.get("code")?.as_str()?.to_string(),
                        exit: entry.get("exit")?.as_i64()?,
                        retryable: entry.get("retryable")?.as_bool()?,
                        reasons: Some(rules::reason_names(entry)),
                    })
                })
                .collect::<Vec<_>>()
        });
        let rows = tool_rows.or_else(|| fixture.registry.clone());
        Self {
            log,
            fixture,
            claim,
            calls,
            documents,
            tool,
            mutation,
            seen: RefCell::new(BTreeSet::new()),
            tool_errors,
            tool_warnings,
            rows,
            l1,
            l2,
            effects,
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

    fn limit(&self, index: usize) -> Duration {
        self.at(index).call.limit.unwrap_or(self.fixture.timeout)
    }

    fn label(&self, index: usize) -> String {
        format!("`{}`", self.at(index).call.argv.join(" "))
    }

    fn check(&'a self, id: &'static str) -> Conditions<'a> {
        let entry = TABLE
            .iter()
            .chain(l1::TABLE.iter())
            .chain(std::iter::once(&l1::CHECK_ORDER))
            .chain(l2::table())
            .find(|entry| entry.id == id)
            .unwrap_or_else(|| panic!("{id} is not in a table"));
        Conditions {
            judge: self,
            entry,
            name: &id[3..],
            problems: Vec::new(),
            refused: Vec::new(),
            lacking: Vec::new(),
            advisories: Vec::new(),
            observed: Vec::new(),
            owned: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// The registry row of `code`.
    fn row(&self, code: &str) -> Option<&RegistryRow> {
        self.rows.as_ref()?.iter().find(|row| row.code == code)
    }

    /// A check that works from the probes of `command`, which the checker derives from the fixture's `ok` invocations of it: when the tool refused one of
    /// the fixture's `ok` or `paged` invocations of `command` as a precondition, the probes cannot be told from that refusal. The check records the refusal,
    /// leaves the command out and gets `true`.
    fn gate(&self, c: &mut Conditions<'_>, command: &str) -> bool {
        let Some(commands) = &self.fixture.commands else { return false };
        let Some((_, calls)) = commands.iter().zip(&self.calls.commands).find(|((id, _), _)| id.as_str() == command) else { return false };
        for index in calls.ok.iter().chain(&calls.paged) {
            if let Some(Refusal::Precondition(code)) = self.fixture_refusal(command, *index) {
                c.refused_by_precondition(*index, &code);
                return true;
            }
        }
        false
    }

    /// How the tool refused `index`, an invocation of `command` that the fixture lists to succeed: with exit 2 and an error code, by a step that reads no state
    /// (spec 11.7 steps 1-6) or by a precondition.
    fn fixture_refusal(&self, command: &str, index: usize) -> Option<Refusal> {
        if self.at(index).exit != Exit::Code(2) {
            return None;
        }
        let document = self.envelope(index).filter(|document| document.get("ok") == Some(&Value::Bool(false)))?;
        let code = document.pointer("/error/code").and_then(Value::as_str)?;
        let field = document.pointer("/error/field").and_then(Value::as_str);
        Some(if self.is_stateless(command, code, field) {
            Refusal::Stateless(code.to_string())
        } else {
            Refusal::Precondition(code.to_string())
        })
    }

    /// Whether a refusal with `code` of `command` is made before any state is read: `USAGE` and its registered finer replacements, `UNKNOWN_COMMAND`,
    /// `COMMAND_REQUIRED`, every `INPUT_*` code; from L1 also a code the command document declares on a constraint or a validator and a code the run
    /// saw refuse an argument's domain. At L0 there is no command document, so any other exit 2 code is a precondition, registered or not. The finer replacement
    /// of `USAGE` counts here when it names an option (`--name`): a registered code at a document pointer may be the lookup of a value, a precondition.
    fn is_stateless(&self, command: &str, code: &str, field: Option<&str>) -> bool {
        matches!(code, "USAGE" | "UNKNOWN_COMMAND" | "COMMAND_REQUIRED")
            || code.starts_with("INPUT_")
            || self.is_usage(Some(code), field.filter(|field| field.starts_with("--")))
            || (self.l1.is_some() && self.declared_before_state(command, code))
    }

    /// The JSON-mode invocations that finished: those whose stdout is read as one document.
    fn json_runs(&self) -> Vec<usize> {
        (0..self.log.len())
            .filter(|index| {
                self.at(*index).call.json_mode() && self.at(*index).exit != Exit::TimedOut
            })
            .collect()
    }

    /// The JSON-mode invocations that finished and printed a JSON document, an envelope or not.
    fn documents_of_json_runs(&self) -> Vec<(usize, &Value)> {
        self.json_runs()
            .into_iter()
            .filter_map(|index| self.documents[index].as_ref().map(|document| (index, document)))
            .collect()
    }

    /// The JSON-mode invocations that finished and printed an envelope.
    fn envelopes(&self) -> Vec<(usize, &Value)> {
        self.json_runs()
            .into_iter()
            .filter_map(|index| self.envelope(index).map(|document| (index, document)))
            .collect()
    }

    fn version(&'a self) -> Finding {
        let calls = self.calls;
        let outcome = self.at(calls.version);
        let mut c = self.check("L0.version");
        c.own(calls.version);
        c.holds_at(calls.version, "exit-0", outcome.exit == Exit::Code(0), || {
            format!("exit is {}, expected 0", exit_text(outcome.exit))
        });
        c.holds_at(calls.version, "stdout-non-empty", !outcome.stdout.is_empty(), || {
            "stdout is empty".to_string()
        });
        c.holds_at(calls.version, "stderr-empty", outcome.stderr.is_empty(), || {
            format!("stderr is {:?}, expected it empty", excerpt(&outcome.stderr))
        });
        c.finish(true)
    }

    fn version_format(&'a self) -> Finding {
        let calls = self.calls;
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
            c.holds_at(calls.version, "tool-version-line", ok, || {
                format!(
                    "stdout is {line:?}, expected one line `{} <version>`",
                    self.tool
                )
            });
        }
        c.finish(true)
    }

    fn unknown_command(&'a self) -> Finding {
        let calls = self.calls;
        let mut c = self.check("L0.unknown-command");
        c.own(calls.unknown_command);
        let document = self.envelope(calls.unknown_command);
        c.holds_at(calls.unknown_command, "envelope", document.is_some(), || {
            "stdout is not an envelope".to_string()
        });
        if let Some(document) = document {
            for (condition, holds, why) in unknown_command_conditions(document) {
                c.holds_at(calls.unknown_command, condition, holds, || why);
            }
        }
        c.finish(true)
    }

    fn unknown_command_text(&'a self) -> Finding {
        let calls = self.calls;
        let outcome = self.at(calls.unknown_command_text);
        let mut c = self.check("L0.unknown-command-text");
        c.own(calls.unknown_command_text);
        c.holds_at(calls.unknown_command_text, "exit-2", outcome.exit == Exit::Code(2), || {
            format!("exit is {}, expected 2", exit_text(outcome.exit))
        });
        c.holds_at(calls.unknown_command_text, "stdout-empty", outcome.stdout.is_empty(), || {
            "stdout is not empty: a refusal in text mode prints nothing on stdout".to_string()
        });
        c.holds_at(calls.unknown_command_text, "stderr-non-empty", !outcome.stderr.is_empty(), || {
            "stderr is empty".to_string()
        });
        c.finish(true)
    }

    fn text_error_form(&'a self) -> Finding {
        let calls = self.calls;
        let outcome = self.at(calls.unknown_command_text);
        let mut c = self.check("L0.text-error-form");
        // An empty stderr is `L0.unknown-command-text`'s to report.
        if !outcome.stderr.is_empty() {
            c.observe(calls.unknown_command_text);
            let prefix = format!("{}: UNKNOWN_COMMAND: ", self.tool);
            let stderr = String::from_utf8_lossy(&outcome.stderr);
            c.holds_at(calls.unknown_command_text, "prefix", stderr.starts_with(&prefix), || {
                format!(
                    "stderr starts {:?}, expected {prefix:?}",
                    first_line(&stderr)
                )
            });
        }
        c.finish(true)
    }

    fn bare(&'a self) -> Finding {
        let calls = self.calls;
        let mut c = self.check("L0.bare");
        c.own(calls.bare);
        let document = self.envelope(calls.bare);
        c.holds_at(calls.bare, "envelope", document.is_some(), || {
            "stdout is not an envelope".to_string()
        });
        if let Some(document) = document {
            let code = document.pointer("/error/code").and_then(Value::as_str);
            c.holds_at(calls.bare, "code", code == Some("COMMAND_REQUIRED"), || {
                format!(
                    "error.code is {}, expected COMMAND_REQUIRED",
                    code.unwrap_or("missing")
                )
            });
        }
        c.finish(true)
    }

    /// `<probe> --bogus-xyz --json`: a refusal code, the field and the command. Whether it is an envelope is `L0.json-mode-preparse`'s.
    fn unknown_option(&'a self) -> Finding {
        let mut c = self.check("L0.unknown-option");
        let Some(probe) = &self.calls.probe else {
            return c.finish(false);
        };
        c.own(probe.unknown_option);
        if let Some(document) = self.envelope(probe.unknown_option) {
            let code = document.pointer("/error/code").and_then(Value::as_str);
            // A code the registry lacks is `L0.registry`'s; a registered one must be a refusal.
            let row = code.and_then(|code| self.row(code));
            c.holds_at(
                probe.unknown_option,
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
            c.holds_at(probe.unknown_option, "field", field == Some(NO_SUCH_OPTION), || {
                format!(
                    "error.field is {}, expected {NO_SUCH_OPTION:?}",
                    show(document.pointer("/error/field"))
                )
            });
            let id = self.fixture.probe_id().unwrap_or_default();
            let command = document.pointer("/meta/command").and_then(Value::as_str);
            c.holds_at(probe.unknown_option, "meta-command", command == Some(id.as_str()), || {
                format!(
                    "meta.command is {}, expected {id:?}",
                    show(document.pointer("/meta/command"))
                )
            });
        }
        c.finish(true)
    }

    fn json_mode_preparse(&'a self) -> Finding {
        let mut c = self.check("L0.json-mode-preparse");
        let Some(probe) = &self.calls.probe else {
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
            c.holds_at(index, condition, failure, || {
                format!(
                    "{} did not print a failure envelope on stdout",
                    self.label(index)
                )
            });
        }
        c.finish(true)
    }

    fn after_dashdash(&'a self) -> Finding {
        let mut c = self.check("L0.after-dashdash");
        let Some(probe) = &self.calls.probe else {
            return c.finish(false);
        };
        c.own(probe.after_dashdash);
        c.holds_at(
            probe.after_dashdash,
            "not-an-envelope",
            self.envelope(probe.after_dashdash).is_none(),
            || "`--json` after `--` still selected JSON mode".to_string(),
        );
        c.finish(true)
    }

    /// Every invocation: the exit is 0, 1, 2 or a signal; `ok` is true exactly when it is 0; a failure's code is
    /// registered with the exit and `retryable` the process shows; a refusal carries no `data`. The reserved codes
    /// of the registry have their reserved exits. That a code is registered at all is `L0.registry`'s.
    fn ok_exit(&'a self) -> Finding {
        let mut c = self.check("L0.ok-exit");
        for (index, outcome) in self.log.iter().enumerate() {
            // The run of a `notRun` command ended by the checker's signal: its exit is the signal's (spec 8.4).
            if outcome.exit == Exit::TimedOut || outcome.served {
                continue;
            }
            let before = c.problems.len();
            let label = self.label(index);
            c.holds_at(index, "exit-range", exit_is_allowed(outcome.exit), || {
                format!(
                    "{label} exited {}, outside 0, 1 and 2 and not a signal exit",
                    exit_text(outcome.exit)
                )
            });
            if let (true, Some(document)) = (outcome.call.json_mode(), self.envelope(index)) {
                self.envelope_exit(&mut c, index, &label, outcome.exit, document);
            }
            if c.problems.len() > before {
                c.observe(index);
            }
        }
        if let Some(registry) = &self.rows {
            let wrong: Vec<String> = documents::L0_RESERVED
                .iter()
                .filter_map(|code| {
                    // The four L0 codes exist since 1.0, so the minor of the lookup does not matter.
                    let exit = i64::from(atip::reserved_code(code, atip::Minor::V1_0)?.exit);
                    registry
                        .iter()
                        .find(|row| row.code == *code)
                        .filter(|row| row.exit != exit)
                        .map(|row| {
                            format!(
                                "{code} is registered with exit {}, the contract says {exit}",
                                row.exit
                            )
                        })
                })
                .collect();
            c.holds_each(None, "reserved-codes", wrong);
        }
        c.finish(true)
    }

    fn envelope_exit(&self, c: &mut Conditions<'_>, index: usize, label: &str, exit: Exit, document: &Value) {
        let Some(ok) = document.get("ok").and_then(Value::as_bool) else {
            return;
        };
        let code = match exit {
            Exit::Code(code) if (0..=2).contains(&code) => Some(code),
            _ => None,
        };
        if let Some(code) = code {
            c.holds_at(index, "ok-iff-exit-0", ok == (code == 0), || {
                format!("{label} exited {code} with `ok` {ok}")
            });
        }
        if ok {
            return;
        }
        c.holds_at(
            index,
            "no-data-on-refusal",
            !(code == Some(2) && document.get("data").is_some()),
            || format!("{label} was refused (exit 2) and carries `data`"),
        );
        let Some(error_code) = document.pointer("/error/code").and_then(Value::as_str) else {
            return;
        };
        // A code that is not registered at all is `L0.registry`'s.
        let Some(row) = self.row(error_code) else {
            return;
        };
        if let Some(code) = code {
            c.holds_at(index, "exit-is-registered", i64::from(code) == row.exit, || {
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
            c.holds_at(index, "retryable-is-registered", retryable == row.retryable, || format!("{label} failed with {error_code} and `retryable` {retryable}, the registry says {}", row.retryable));
        }
    }

    /// `null` in `meta` or `error` (spec 6.5).
    fn no_null(&'a self) -> Finding {
        let mut c = self.check("L0.no-null");
        self.judge_served(&mut c, false);
        for (index, document) in self.documents_of_json_runs() {
            let found = rules::null_problems(document);
            c.holds_at(index, "no-null", found.is_empty(), || {
                format!(
                    "{} printed {}",
                    self.label(index),
                    found.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
                )
            });
            if !found.is_empty() {
                c.observe(index);
            }
        }
        c.finish(true)
    }

    /// Every JSON-mode document validates against the envelope schema, but for what other checks own.
    /// The one run of each `notRun` command: a command that printed no document within the time limit fails `L0.envelope`; one with no run to make leaves the check that judges it unprovided.
    fn judge_served(&self, c: &mut Conditions<'a>, no_document: bool) {
        for served in &self.calls.served {
            match served.call {
                Some(index) if self.at(index).ended_alone => {
                    // Refused at the start by a precondition (kept by `invoke_served`): like any fixture call, unprovided with its code.
                    if let Some(code) = served_precondition(self.at(index), self.fixture.registry.as_deref(), None) {
                        c.refused_by_precondition(index, &code);
                    }
                }
                Some(index) => {
                    c.observe(index);
                    if no_document && self.at(index).exit == Exit::TimedOut {
                        c.holds_at(index, "document-before-timeout", false, || {
                            format!("`{}` printed no JSON document within the {} second limit; it is a command of `notRun`, run once up to its first document", self.at(index).call.argv.join(" "), self.fixture.timeout.as_secs_f64())
                        });
                    }
                }
                None => c.lacking.push((
                    format!("commands.{}.ok", served.command),
                    format!("`notRun` names {}, which has no `ok` invocation in the fixture{}: there is no run to judge", served.command, if self.l1.is_some() { " and no example" } else { "" }),
                )),
            }
        }
    }

    fn envelope_check(&'a self) -> Finding {
        let mut c = self.check("L0.envelope");
        self.judge_served(&mut c, true);
        for (index, document) in self.documents_of_json_runs() {
            // A tool emits envelopes or stream lines (13.5): a stream is judged line by line by the stream-line schema.
            let found = if rules::is_stream_start(document) {
                stream_lines(&self.at(index).stdout).iter().flat_map(rules::stream_line_problems).collect()
            } else {
                rules::envelope_problems(document, self.claim)
            };
            c.holds_at(index, "valid-envelope", found.is_empty(), || {
                format!(
                    "{} printed a document the envelope schema rejects: {}",
                    self.label(index),
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
            c.holds_at(index, "no-indentation", !indented, || {
                format!("{} printed an indented document", self.label(index))
            });
            if indented {
                c.observe(index);
            }
        }
        c.finish(true)
    }

    /// Spec 5.1: the whole stdout of the plain probe, the unknown command, the bare call and every `ok` invocation is one document and one newline.
    fn one_document(&'a self) -> Finding {
        let calls = self.calls;
        let mut c = self.check("L0.one-document");
        let probe = calls.probe.as_ref().map(|probe| probe.json_variations[0].1);
        let ok_runs: Vec<(&str, usize)> = self
            .fixture
            .commands
            .iter()
            .flatten()
            .zip(&calls.commands)
            .flat_map(|((id, _), command)| command.ok.iter().map(move |index| (id.as_str(), *index)))
            .collect();
        for index in [Some(calls.unknown_command), Some(calls.bare), probe]
            .into_iter()
            .flatten()
            .chain(ok_runs.iter().map(|(_, index)| *index))
        {
            let ok_id = ok_runs.iter().find(|(_, ok)| *ok == index).map(|(id, _)| *id);
            let is_ok_run = ok_id.is_some();
            // Output that holds no document at all is the owner of that invocation's to report,
            // except for an `ok` invocation, which has no other owner.
            if self.documents[index].is_none() && !is_ok_run {
                continue;
            }
            if self.at(index).exit == Exit::TimedOut {
                continue;
            }
            // An `ok` invocation the tool refused: for want of state or environment the check cannot tell; before any state is read it is the fixture's or the tool's fault.
            if let Some(id) = ok_id {
                match self.fixture_refusal(id, index) {
                    Some(Refusal::Precondition(code)) => {
                        c.refused_by_precondition(index, &code);
                        continue;
                    }
                    Some(Refusal::Stateless(code)) => {
                        c.observe(index);
                        c.holds_at(index, "ok-is-not-refused", false, || {
                            format!(
                                "{} is listed under `ok` for {id} and was refused with {code} (exit 2) before any state is read: the fixture invocation is wrong, or the tool enforces a rule it does not declare",
                                self.label(index)
                            )
                        });
                        continue;
                    }
                    None => {}
                }
            }
            // A stream is NDJSON by design (13.5): its lines are `L0.envelope`'s.
            if self.documents[index].as_ref().is_some_and(rules::is_stream_start) {
                continue;
            }
            c.observe(index);
            let problem = one_document(&self.at(index).stdout).err();
            c.holds_at(index, "one-document-one-newline", problem.is_none(), || {
                format!(
                    "{}: {}",
                    self.label(index),
                    problem.clone().unwrap_or_default()
                )
            });
        }
        c.finish(true)
    }

    /// The probe's JSON output does not depend on the environment or a terminal (spec 4.3).
    fn no_env_switch(&'a self) -> Finding {
        let mut c = self.check("L0.no-env-switch");
        let Some(probe) = &self.calls.probe else {
            return c.finish(false);
        };
        let plain_index = &probe.json_variations[0].1;
        c.own(*plain_index);
        let plain = self.envelope(*plain_index);
        c.holds_at(*plain_index, "baseline-is-envelope", plain.is_some(), || {
            format!(
                "{} without a variation did not print an envelope",
                self.label(*plain_index)
            )
        });
        if let Some(plain) = plain {
            for (label, index) in &probe.json_variations[1..] {
                c.own(*index);
                let Some(varied) = self.envelope(*index) else {
                    c.holds_at(*index, "same-envelope-kind", false, || {
                        format!(
                            "with {label} the output is not an envelope, without it an envelope"
                        )
                    });
                    continue;
                };
                c.holds_at(*index, "same-envelope-kind", true, String::new);
                let same = |pointer: &str| plain.pointer(pointer) == varied.pointer(pointer);
                c.holds_at(*index, "same-result", same("/ok") && same("/error/code"), || {
                    format!(
                        "with {label} ok is {} and error.code {}, without it {} and {}",
                        show(varied.get("ok")),
                        show(varied.pointer("/error/code")),
                        show(plain.get("ok")),
                        show(plain.pointer("/error/code"))
                    )
                });
                c.holds_at(*index, "same-command", same("/meta/command"), || {
                    format!(
                        "with {label} meta.command is {}, without it {}",
                        show(varied.pointer("/meta/command")),
                        show(plain.pointer("/meta/command"))
                    )
                });
                c.holds_at(*index, "same-schema-version", same("/meta/schemaVersion"), || {
                    format!(
                        "with {label} meta.schemaVersion is {}, without it {}",
                        show(varied.pointer("/meta/schemaVersion")),
                        show(plain.pointer("/meta/schemaVersion"))
                    )
                });
                c.holds_at(
                    *index,
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
        c.finish(true)
    }

    fn no_env_switch_text(&'a self) -> Finding {
        let mut c = self.check("L0.no-env-switch-text");
        let Some(probe) = &self.calls.probe else {
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
            c.holds_at(*index, "same-envelope-kind", seen == plain, || {
                format!(
                    "with {label} the output is {}, without it {}",
                    name(seen),
                    name(plain)
                )
            });
        }
        c.finish(true)
    }

    /// `meta.contract` on every envelope: SHOULD for a 1.0 claim; for 1.1, `L0.contract-version` owns the presence.
    fn contract(&'a self) -> Finding {
        let mut c = self.check("L0.contract");
        if self.claim.at_least(Claim::V1_1) {
            return c.finish_as(End::Unsupported(
                format!("the claim is {}: L0.contract-version owns the presence of meta.contract", self.claim.as_str()),
            ));
        }
        for (index, document) in self.envelopes() {
            let present = document.pointer("/meta/contract").is_some();
            c.holds_at(index, "meta-contract", present, || {
                format!(
                    "{} printed an envelope without meta.contract",
                    self.label(index)
                )
            });
            if !present {
                c.observe(index);
            }
        }
        c.finish(true)
    }

    /// The document declares the claimed minor, and nothing of a later one (spec 15.7, 10.13 rule 16).
    fn contract_version(&'a self) -> Finding {
        let mut c = self.check("L0.contract-version");
        self.judge_served(&mut c, false);
        for (index, document) in self.envelopes() {
            for (condition, problem) in rules::contract_version_findings(document, self.claim) {
                c.holds_at(index, condition, problem.is_none(), || {
                    format!("{}: {}", self.label(index), problem.clone().unwrap_or_default())
                });
                if problem.is_some() {
                    c.observe(index);
                }
            }
        }
        // The start line of a stream carries the markers of an envelope's meta, and the end line holds an envelope.
        for (index, document) in self.documents_of_json_runs() {
            if !rules::is_stream_start(document) {
                continue;
            }
            let lines = stream_lines(&self.at(index).stdout);
            let end = lines.iter().rev().find(|line| line.get("type").and_then(Value::as_str) == Some("end")).and_then(|line| line.get("result"));
            let findings = rules::stream_start_findings(document, self.claim)
                .into_iter()
                .chain(end.filter(|result| is_envelope(result)).map(|result| rules::contract_version_findings(result, self.claim)).into_iter().flatten());
            for (condition, problem) in findings {
                c.holds_at(index, condition, problem.is_none(), || {
                    format!("{} (a stream): {}", self.label(index), problem.clone().unwrap_or_default())
                });
                if problem.is_some() {
                    c.observe(index);
                }
            }
        }
        c.finish(true)
    }

    /// Whether stderr of a JSON-mode run holds the error object or the result (spec 5.2).
    fn stderr_result(&'a self) -> Finding {
        let mut c = self.check("L0.stderr-result");
        for index in self.json_runs() {
            let outcome = self.at(index);
            let carries = carries_error_or_result(&outcome.stderr, self.documents[index].as_ref());
            c.holds_at(index, "stderr-carries-no-result", !carries, || {
                format!("{} also printed its error or result on stderr", self.label(index))
            });
            if carries {
                c.observe(index);
            }
        }
        c.finish(true)
    }

    fn stderr_quiet(&'a self) -> Finding {
        let mut c = self.check("L0.stderr-quiet");
        for index in self.json_runs() {
            let outcome = self.at(index);
            // A run that printed no envelope is not in JSON mode as far as the tool is concerned: its stderr is the text error
            // of a tool that dropped JSON, which the check that saw it drop owns. Stderr that carries the error or the result is
            // `L0.stderr-result`'s.
            if self.envelope(index).is_none()
                || carries_error_or_result(&outcome.stderr, self.documents[index].as_ref())
            {
                continue;
            }
            c.holds_at(index, "stderr-empty", outcome.stderr.is_empty(), || {
                format!(
                    "{} printed {:?} on stderr in JSON mode",
                    self.label(index),
                    excerpt(&outcome.stderr)
                )
            });
            if !outcome.stderr.is_empty() {
                c.observe(index);
            }
        }
        c.finish(true)
    }

    /// Every `meta.next` entry the run saw (spec 6.8).
    fn next(&'a self) -> Finding {
        let mut c = self.check("L0.next");
        self.judge_served(&mut c, false);
        let mut exercised = false;
        for (index, document) in self.envelopes() {
            if document.pointer("/meta/next").is_none() {
                continue;
            }
            exercised = true;
            c.observe(index);
            let json_mode = self.at(index).call.json_mode();
            for (condition, problem) in rules::next_findings(document, self.claim, json_mode, &self.tool) {
                c.holds_at(index, condition, problem.is_none(), || {
                    format!("{}: {}", self.label(index), problem.clone().unwrap_or_default())
                });
            }
        }
        c.finish(exercised)
    }

    /// Every `meta.page` the run saw (spec 6.9): more items mean a next entry for the next page. Whether the tool
    /// pages at all is decided by observation: a command that answers `--cursor x` with `CURSOR_INVALID` pages.
    fn page(&'a self) -> Finding {
        let mut c = self.check("L0.page");
        let mut exercised = false;
        for (index, document) in self.envelopes() {
            if document.pointer("/meta/page").is_none() {
                continue;
            }
            exercised = true;
            c.observe(index);
            for (condition, problem) in rules::page_findings(document, self.claim) {
                c.holds_at(index, condition, problem.is_none(), || {
                    format!("{}: {}", self.label(index), problem.clone().unwrap_or_default())
                });
            }
        }
        // A `paged` invocation is one the fixture says returns a next cursor in its state.
        for (id, command) in self.fixture.commands.iter().flatten().zip(&self.calls.commands).map(|((id, _), calls)| (id, calls)) {
            // An `ok` invocation refused for want of state or environment leaves the paging of the command unjudged too.
            for index in &command.ok {
                c.skips_refused(id, *index);
            }
            for index in &command.paged {
                c.own(*index);
                if c.skips_refused(id, *index) {
                    continue;
                }
                let carries = self
                    .envelope(*index)
                    .and_then(|document| document.pointer("/meta/page/nextCursor"))
                    .is_some_and(Value::is_string);
                c.holds_at(*index, "paged-next-cursor", carries, || {
                    format!("{} is listed as `paged` for {id} and its answer has no meta.page.nextCursor", self.label(*index))
                });
                exercised |= carries;
            }
        }
        if exercised {
            return c.finish_as(End::Observed);
        }
        // Nothing carried a page. Which commands page (spec 6.9): from L1 the command documents say, at L0 observation does.
        if let Some(l1) = self.l1 {
            let pages: Vec<&str> = l1.docs.commands.iter().filter(|(id, doc)| doc.has_option("cursor") && !self.fixture.not_run.contains(id)).map(|(id, _)| id.as_str()).collect();
            return c.finish_as(if pages.is_empty() {
                End::Unsupported("no command document declares --cursor".to_string())
            } else {
                End::Unprovided {
                    reason: format!(
                        "{} declare{} --cursor and no run saw a meta.page.nextCursor: list an invocation that pages under `paged` in the fixture",
                        pages.join(", "),
                        if pages.len() == 1 { "s" } else { "" }
                    ),
                    cut: false,
                    entries: pages.iter().map(|id| format!("commands.{id}.paged")).collect(),
                }
            });
        }
        let Some(commands) = &self.fixture.commands else {
            return c.finish_as(End::missing("commands", "the fixture has no `commands`"));
        };
        if commands.is_empty() {
            return c.finish_as(End::missing("commands", "the fixture's `commands` is empty"));
        }
        // At L0 a command that refuses `--cursor x` as an unknown option does not page; one that refuses it as a cursor does.
        let verdicts: Vec<CursorVerdict> = self
            .calls
            .commands
            .iter()
            .map(|command| self.cursor_verdict(command.cursor))
            .collect();
        if verdicts.is_empty() || verdicts.iter().any(|verdict| *verdict != CursorVerdict::NoPaging) {
            let entries: Vec<String> = commands.iter().zip(&verdicts).filter(|(_, verdict)| **verdict != CursorVerdict::NoPaging).map(|((id, _), _)| format!("commands.{id}.paged")).collect();
            return c.finish_as(End::Unprovided {
                reason: "no answer the run saw carried meta.page, and not every listed command definitely refuses --cursor as an option it does not have (CURSOR_INVALID shows a command pages; an inconclusive answer counts as one that might): list an invocation that pages under `paged`".to_string(),
                cut: false,
                entries: if entries.is_empty() { vec!["commands".to_string()] } else { entries },
            });
        }
        c.finish_as(End::Unsupported(
            "no command of the fixture pages: each refuses --cursor as an option it does not have".to_string(),
        ))
    }

    fn cursor_verdict(&self, index: usize) -> CursorVerdict {
        let Some(document) = self.envelope(index) else {
            return CursorVerdict::Unknown;
        };
        let refused = self.at(index).exit == Exit::Code(2)
            && document.pointer("/error/field").and_then(Value::as_str) == Some("--cursor");
        match document.pointer("/error/code").and_then(Value::as_str) {
            Some("CURSOR_INVALID") => CursorVerdict::Pages,
            Some(_) if refused => CursorVerdict::NoPaging,
            _ => CursorVerdict::Unknown,
        }
    }

    /// A refusal changes nothing in the working directory or in `HOME`, and nothing the probe reports (spec 8.2, 12.2).
    fn refusal_effects(&'a self) -> Finding {
        let mut c = self.check("L0.refusal-effects");
        for (index, outcome) in self.log.iter().enumerate() {
            if outcome.exit != Exit::Code(2) {
                continue;
            }
            c.holds_at(
                index,
                "refusal-changes-nothing",
                outcome.changes.is_empty(),
                || {
                    format!(
                        "{} was refused (exit 2) and changed {}",
                        self.label(index),
                        outcome.changes.join(", ")
                    )
                },
            );
            if !outcome.changes.is_empty() {
                c.observe(index);
            }
            if let (Some(baseline), Some(after)) = (&self.calls.remote_baseline, &outcome.probe_after) {
                let state = first_document(after)
                    .filter(|document| document.get("ok") == Some(&Value::Bool(true)))
                    .and_then(|document| document.get("data").cloned());
                if let Some(state) = state {
                    let same = &state == baseline;
                    c.holds_at(index, "refusal-changes-no-remote-state", same, || {
                        format!(
                            "{} was refused (exit 2) and the probe then reports {state}, not {baseline}",
                            self.label(index)
                        )
                    });
                    if !same {
                        c.observe(index);
                    }
                }
            }
        }
        c.finish(true)
    }

    /// `missingValue` on a terminal in canonical mode refuses as the control run with stdin at end-of-file does (spec 5.5).
    fn no_prompt(&'a self) -> Finding {
        let mut c = self.check("L0.no-prompt");
        if self.fixture.commands.is_none() {
            return c.finish_as(End::missing("commands", "the fixture has no `commands`"));
        }
        let missing: Vec<&MissingCalls> = self
            .calls
            .commands
            .iter()
            .flat_map(|command| command.missing.iter())
            .collect();
        if missing.is_empty() {
            return c.finish_as(End::missing("commands.<id>.missingValue", "no command of the fixture has a `missingValue` invocation"));
        }
        let mut unclear: Vec<String> = Vec::new();
        let mut judged = false;
        for call in missing {
            let control = self.at(call.control);
            let Some(terminal_index) = call.terminal else {
                unclear.push(format!(
                    "{} was not refused with exit 2 as a control run (it ended with {}), so it cannot show a prompt",
                    self.label(call.control),
                    exit_text(control.exit)
                ));
                continue;
            };
            let terminal = self.at(terminal_index);
            c.observe(terminal_index);
            if let Some(at) = &terminal.at_deadline {
                let printed = String::from_utf8_lossy(&at.output);
                let waited = format!(
                    "{} was still running {:.1} seconds after it started on a terminal (the control run took {:.2})",
                    self.label(terminal_index),
                    self.limit(terminal_index).as_secs_f64(),
                    control.elapsed.as_secs_f64()
                );
                if at.reacted_to_eof {
                    judged = true;
                    c.holds_at(terminal_index, "refuses-on-a-terminal", false, || {
                        format!("{waited}, and it ended or printed within two seconds of the end-of-file character: it waits for input")
                    });
                } else if !at.output.is_empty() {
                    judged = true;
                    c.holds_at(terminal_index, "refuses-on-a-terminal", false, || {
                        format!("{waited}, after printing {:?}: it asks for a value", excerpt(printed.as_bytes()))
                    });
                } else {
                    unclear.push(format!("{waited}, with no output and no reaction to end-of-file: slow, not shown to wait for input"));
                }
                continue;
            }
            judged = true;
            let envelope = self.envelope(terminal_index);
            let rest = envelope.map(|_| {
                let text = String::from_utf8_lossy(&terminal.stdout).into_owned();
                let body_length = first_document_text(&text).len();
                text.trim_start().get(body_length..).unwrap_or_default().trim().to_string()
            });
            // It refuses as the control run does and prints nothing besides the one document it prints (none at all is
            // `L0.no-env-switch`'s: a tool that drops JSON on a terminal).
            let printed_nothing_else = envelope.is_none() && terminal.stdout.iter().all(u8::is_ascii_whitespace)
                || rest.as_deref().is_some_and(str::is_empty);
            let refuses = terminal.exit == control.exit && printed_nothing_else;
            c.holds_at(terminal_index, "refuses-on-a-terminal", refuses, || {
                format!(
                    "{} on a terminal ended with {} and printed {:?}, but the control run refused with exit {} and nothing else should be on the terminal",
                    self.label(terminal_index),
                    exit_text(terminal.exit),
                    terminal_excerpt(&terminal.stdout),
                    exit_text(control.exit)
                )
            });
        }
        if judged {
            return c.finish(true);
        }
        let entries: Vec<String> = self
            .fixture
            .commands
            .iter()
            .flatten()
            .zip(&self.calls.commands)
            .filter(|(_, calls)| !calls.missing.is_empty())
            .map(|((id, _), _)| format!("commands.{id}.missingValue"))
            .collect();
        c.finish_as(End::Unprovided { reason: unclear.join("; "), cut: false, entries })
    }

    /// The registries the tool prints against the schemas and the fixture, and every code, reason and warning the run emitted against them (spec 7.3.4, 7.5, 7.6, 17.3).
    fn registry(&'a self) -> Finding {
        let calls = self.calls;
        let mut c = self.check("L0.registry");
        let offered = self.tool_errors.is_some();
        if let Some(entries) = &self.tool_errors {
            c.own(calls.schema_errors);
            let kind = self
                .envelope(calls.schema_errors)
                .and_then(|document| document.pointer("/data/kind"))
                .and_then(Value::as_str);
            c.holds_at(
                calls.schema_errors,
                "kind",
                if self.claim.at_least(Claim::V1_1) {
                    kind == Some("errors")
                } else {
                    kind.is_none_or(|kind| kind == "errors")
                },
                || format!("`schema --errors --json` has data.kind {}, expected \"errors\"", kind.map_or("missing".to_string(), |kind| format!("{kind:?}"))),
            );
            let invalid: Vec<String> = entries
                .iter()
                .flat_map(|entry| {
                    let prepared = strip_reasons_for_1_0(entry, self.claim);
                    schemas()
                        .registry_entry_problems(&prepared)
                        .into_iter()
                        // From L1 the reasons of an entry are `L1.reasons`' (form, no exit of their own, the same in the list and the entry).
                        .filter(|problem| self.l1.is_none() || !problem.starts_with("/reasons"))
                        .map(|problem| format!("the entry {} is invalid: {problem}", entry.get("code").map_or("?".to_string(), Value::to_string)))
                })
                .collect();
            c.holds_each(Some(calls.schema_errors), "entries-valid", invalid);
            let wrong: Vec<(Option<usize>, String)> = calls
                .error_entries
                .iter()
                .filter_map(|(code, index)| {
                    let listed = entries.iter().find(|entry| entry.get("code").and_then(Value::as_str) == Some(code));
                    self.entry_answer("error", self.envelope(*index), listed)
                        .map(|problem| (Some(*index), format!("`schema --error {code} --json` {problem}")))
                })
                .collect();
            c.holds_each_at("error-answers", wrong);
            if let Some(registry) = &self.fixture.registry {
                let tool_rows: Vec<RegistryRow> = self.rows.clone().unwrap_or_default();
                let drift = registry_drift(registry, &tool_rows);
                c.holds_each(
                    Some(calls.schema_errors),
                    "registry-is-the-fixtures",
                    drift.iter().map(|difference| format!("`schema --errors --json` and the fixture's registry differ: {difference}")).collect(),
                );
            }
        }
        if let Some(entries) = &self.tool_warnings {
            c.own(calls.schema_warnings);
            let kind = self
                .envelope(calls.schema_warnings)
                .and_then(|document| document.pointer("/data/kind"))
                .and_then(Value::as_str);
            c.holds_at(calls.schema_warnings, "warnings-kind", kind == Some("warnings"), || {
                format!("`schema --warnings --json` has data.kind {}, expected \"warnings\"", kind.map_or("missing".to_string(), |kind| format!("{kind:?}")))
            });
            let invalid: Vec<String> = entries
                .iter()
                .flat_map(|entry| {
                    schemas().warning_entry_problems(entry).into_iter().map(|problem| {
                        format!("the warning entry {} is invalid: {problem}", entry.get("code").map_or("?".to_string(), Value::to_string))
                    })
                })
                .collect();
            c.holds_each(Some(calls.schema_warnings), "warning-entries-valid", invalid);
            let wrong: Vec<(Option<usize>, String)> = calls
                .warning_entries
                .iter()
                .filter_map(|(code, index)| {
                    let listed = entries.iter().find(|entry| entry.get("code").and_then(Value::as_str) == Some(code));
                    self.entry_answer("warning", self.envelope(*index), listed)
                        .map(|problem| (Some(*index), format!("`schema --warning {code} --json` {problem}")))
                })
                .collect();
            c.holds_each_at("warning-answers", wrong);
            for (code, index) in &calls.warning_only {
                c.observe(*index);
                let Some(document) = self.documents[*index].as_ref().filter(|document| is_envelope(document)) else {
                    continue;
                };
                for (condition, problem) in rules::warning_only_findings(document, &self.tool, code) {
                    c.holds_at(*index, condition, problem.is_none(), || format!("`schema --error {code} --json`: {}", problem.clone().unwrap_or_default()));
                }
            }
            if let Some(listed) = &self.fixture.warnings {
                let tool: Vec<&str> = entries.iter().filter_map(|entry| entry.get("code").and_then(Value::as_str)).collect();
                let mut only_fixture: Vec<&str> = listed.iter().map(String::as_str).filter(|code| !tool.contains(code)).collect();
                let mut only_tool: Vec<&str> = tool.iter().copied().filter(|code| !listed.iter().any(|known| known == code)).collect();
                only_fixture.sort_unstable();
                only_tool.sort_unstable();
                c.holds_at(calls.schema_warnings, "warnings-are-the-fixtures", only_fixture.is_empty() && only_tool.is_empty(), || {
                    format!("`schema --warnings --json` and the fixture's warnings differ: only in the fixture {only_fixture:?}, only in the tool {only_tool:?}")
                });
            }
        }
        self.emitted_conditions(&mut c);
        let end = if !offered {
            End::Unsupported("the tool does not offer schema --errors".to_string())
        } else if self.fixture.registry_from_schema {
            End::Exercised(true)
        } else if self.fixture.registry.is_none() {
            End::missing("registry", "the fixture has no `registry` to compare the tool's with")
        } else if self.tool_warnings.is_some() && self.fixture.warnings.is_none() {
            End::missing("warnings", "the fixture has no `warnings` to compare the tool's warning registry with")
        } else {
            End::Exercised(true)
        };
        c.finish_as(end)
    }

    /// What is wrong with the answer of `schema --error <CODE>` or `--warning <CODE>`: it is the listed entry, under `data.kind` for a 1.1 claim.
    fn entry_answer(&self, kind: &str, document: Option<&Value>, listed: Option<&Value>) -> Option<String> {
        let Some(document) = document.filter(|document| document.get("ok") == Some(&Value::Bool(true))) else {
            return Some("did not answer with a success envelope".to_string());
        };
        if self.claim.at_least(Claim::V1_1) && document.pointer("/data/kind").and_then(Value::as_str) != Some(kind) {
            return Some(format!("has no data.kind {kind:?}"));
        }
        let without_reasons = |entry: Option<&Value>| -> Option<Value> {
            let mut entry = entry.cloned();
            if self.l1.is_some() {
                if let Some(object) = entry.as_mut().and_then(Value::as_object_mut) {
                    object.remove("reasons");
                }
            }
            entry
        };
        (without_reasons(document.pointer(&format!("/data/{kind}"))) != without_reasons(listed)).then(|| format!("answered an entry that is not the one the list holds ({})", show(document.pointer(&format!("/data/{kind}")))))
    }

    /// Every error code, reason and warning code any run emitted is registered (spec 7.3.1, 7.5.1, 7.6.2).
    fn emitted_conditions(&self, c: &mut Conditions<'_>) {
        let Some(rows) = &self.rows else {
            return;
        };
        let warning_codes: Vec<String> = match (&self.tool_warnings, &self.tool_errors) {
            (Some(entries), _) => entries.iter().filter_map(|entry| entry.get("code").and_then(Value::as_str).map(str::to_string)).collect(),
            (None, Some(_)) => Vec::new(),
            (None, None) => self.fixture.warnings.clone().unwrap_or_default(),
        };
        for (index, document) in self.envelopes() {
            for (condition, problem) in rules::registry_findings(document, rows, &warning_codes, self.claim) {
                c.holds_at(index, condition, problem.is_none(), || {
                    format!("{}: {}", self.label(index), problem.clone().unwrap_or_default())
                });
                if problem.is_some() {
                    c.observe(index);
                }
            }
        }
    }

    /// A `USAGE` without `field` names the command's schema (spec 7.1.3).
    fn usage_hint(&'a self) -> Finding {
        let mut c = self.check("L0.usage-hint");
        let mut exercised = false;
        let has_schema = self.tool_errors.is_some();
        for (index, document) in self.envelopes() {
            let error = document.get("error");
            let is_usage = error.and_then(|error| error.get("code")).and_then(Value::as_str) == Some("USAGE");
            if !is_usage || error.is_some_and(|error| error.get("field").is_some()) {
                continue;
            }
            exercised = true;
            c.observe(index);
            let command = document.pointer("/meta/command").and_then(Value::as_str);
            let hint = error.and_then(|error| error.get("hint")).and_then(Value::as_str);
            let mut wanted: Vec<Vec<String>> = Vec::new();
            let words: Vec<String> = command.map(|command| command.split(' ').map(str::to_string).collect()).unwrap_or_default();
            let tool = &self.tool;
            let schema_form: Vec<String> = [tool.clone(), "schema".to_string()].into_iter().chain(words.iter().cloned()).chain(["--json".to_string()]).collect();
            let help_form: Vec<String> = std::iter::once(tool.clone()).chain(words.iter().cloned()).chain(["--help".to_string()]).collect();
            wanted.push(schema_form.clone());
            if !has_schema {
                wanted.push(help_form);
            }
            let quoted: Vec<String> = if words.len() > 1 {
                vec![tool.clone(), "schema".to_string(), words.join(" "), "--json".to_string()]
            } else {
                schema_form
            };
            wanted.push(quoted);
            let given = hint.map(atip::split_argv);
            c.holds_at(index, "hint-names-the-schema", given.as_ref().is_some_and(|given| wanted.contains(given)), || {
                format!(
                    "{} refused with USAGE and no field, and the hint is {}, expected `{}`",
                    self.label(index),
                    hint.map_or("missing".to_string(), |hint| format!("{hint:?}")),
                    wanted[0].join(" ")
                )
            });
        }
        c.finish(exercised)
    }

    /// A hint that starts with the tool name and a space is one command line (spec 7.1.2); not run for a 1.0 claim.
    fn hint_form(&'a self) -> Finding {
        let mut c = self.check("L0.hint-form");
        if !self.claim.at_least(Claim::V1_1) {
            return c.finish_as(End::Unsupported(
                "the claim is 1.0: a prose hint may start with the tool name".to_string(),
            ));
        }
        for (index, document) in self.envelopes() {
            let found = rules::hint_problems(document, &self.tool);
            c.holds_at(index, "hint-is-one-command-line", found.is_empty(), || {
                format!("{}: {}", self.label(index), found.join("; "))
            });
            if !found.is_empty() {
                c.observe(index);
            }
        }
        c.finish(true)
    }

    /// A tool-wide option after a group word is no error: the group still answers `COMMAND_REQUIRED` (spec 9.13).
    fn global_on_group(&'a self) -> Finding {
        let mut c = self.check("L0.global-on-group");
        let Some(commands) = &self.fixture.commands else {
            return c.finish_as(End::missing("commands", "the fixture has no `commands`"));
        };
        if commands.is_empty() {
            return c.finish_as(End::missing("commands", "the fixture's `commands` is empty"));
        }
        let Some(options) = &self.fixture.global_options else {
            return c.finish_as(End::missing("globalOptions", "the fixture has no `globalOptions`"));
        };
        if options.is_empty() {
            return c.finish_as(End::Unsupported("the fixture lists no tool-wide option".to_string()));
        }
        let Some(group) = &self.calls.group else {
            // The fixture gives no group to run. Only the tool's own schema can show there are none; else it is unprovided.
            let tree = self.envelope(self.calls.schema_tree).filter(|document| {
                document.get("ok") == Some(&Value::Bool(true)) && document.pointer("/data/kind").and_then(Value::as_str) == Some("tree")
            });
            let groups = tree.map(|document| {
                document
                    .pointer("/data/commands")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|entry| entry.get("kind").and_then(Value::as_str) == Some("group"))
                    .count()
            });
            return c.finish_as(match groups {
                Some(0) => End::Unsupported("the tool's schema lists no group".to_string()),
                Some(_) => End::missing("commands", "no command of the fixture has a group word (a command id of two words), and the tool's schema lists groups"),
                None => End::missing("commands", "no command of the fixture has a group word (a command id of two words), and the tool's schema shows no list of groups to rule them out"),
            });
        };
        let answers = |index: usize| {
            self.envelope(index)
                .and_then(|document| document.pointer("/error/code"))
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        c.own(group.bare);
        let code = answers(group.bare);
        // From L1 a group that runs is `L1.groups-not-runnable`'s; this check then has nothing to say about the option.
        if self.l1.is_some() && code.as_deref() != Some("COMMAND_REQUIRED") {
            return c.finish_as(End::Unsupported("the group is not refused with COMMAND_REQUIRED: L1.groups-not-runnable reports it".to_string()));
        }
        c.holds_at(group.bare, "group-is-not-runnable", code.as_deref() == Some("COMMAND_REQUIRED"), || {
            format!("{} answered {}, expected COMMAND_REQUIRED", self.label(group.bare), code.clone().unwrap_or_else(|| "no failure envelope".to_string()))
        });
        for (option, index) in &group.with_option {
            c.own(*index);
            let code = answers(*index);
            c.holds_at(*index, "option-after-group-is-no-error", code.as_deref() == Some("COMMAND_REQUIRED"), || {
                format!(
                    "{} answered {} for `{}`, expected COMMAND_REQUIRED",
                    self.label(*index),
                    code.clone().unwrap_or_else(|| "no failure envelope".to_string()),
                    option.join(" ")
                )
            });
        }
        c.finish(true)
    }
}

/// How the tool refused an invocation the fixture lists to succeed (`ok`, `paged`, `fields`), with exit 2 and the code it names (approaches/checker.md 5).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Refusal {
    /// A step that reads no state refused it (spec 11.7 steps 1-6): the fixture invocation is wrong, or the tool enforces a rule it does not declare. The check that runs it fails.
    Stateless(String),
    /// Any other exit 2 code: the state or the environment the fixture gives did not give the command what it needs (a credential, a reachable backend). The checks that run it are `unprovided`, which still fails a MUST level.
    Precondition(String),
}

/// What a command that was run with `--cursor x` showed about paging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CursorVerdict {
    /// It refused `--cursor` as an option it does not have: the command does not page.
    NoPaging,
    /// It answered `CURSOR_INVALID`: the command pages.
    Pages,
    /// Anything else.
    Unknown,
}

/// A registry entry as the registry-entry schema sees it: for a 1.0 claim without `reasons` (`L0.contract-version`'s).
fn strip_reasons_for_1_0(entry: &Value, claim: Claim) -> Value {
    let mut copy = entry.clone();
    if claim == Claim::V1_0 {
        if let Some(object) = copy.as_object_mut() {
            object.remove("reasons");
        }
    }
    copy
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

/// The rules of `L0.ok-exit` that read a failure envelope against the registry: the process exit (when known) is the code's, `retryable` is the code's, and a refusal carries no `data`. Without a known exit the code's registered exit stands for it. A code the registry does not list is `L0.registry`'s, and reports nothing here.
pub fn failure_problems(
    document: &Value,
    registry: &[RegistryRow],
    exit: Option<i32>,
) -> Vec<String> {
    let Some(code) = document.pointer("/error/code").and_then(Value::as_str) else {
        return Vec::new();
    };
    let Some(row) = registry.iter().find(|row| row.code == code) else {
        return Vec::new();
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

/// Whether an exit status is one the contract allows: 0, 1, 2, a signal, or 128+n as a launcher reports a signal (spec 8.4).
pub fn exit_is_allowed(exit: Exit) -> bool {
    match exit {
        Exit::Code(code) => (0..=2).contains(&code) || (129..=192).contains(&code),
        Exit::Signal(_) => true,
        Exit::TimedOut => false,
    }
}

/// How the fixture's registry and the tool's own differ, by code: the exit, `retryable` and, where the fixture lists
/// them, the reasons.
pub fn registry_drift(fixture: &[RegistryRow], offered: &[RegistryRow]) -> Vec<String> {
    let mut drift = Vec::new();
    for row in fixture {
        match offered.iter().find(|other| other.code == row.code) {
            None => drift.push(format!("{} is only in the fixture", row.code)),
            Some(other) => {
                if other.exit != row.exit || other.retryable != row.retryable {
                    drift.push(format!(
                        "{} is exit {} retryable {} in the fixture and exit {} retryable {} in the tool",
                        row.code, row.exit, row.retryable, other.exit, other.retryable
                    ));
                }
                if let Some(listed) = &row.reasons {
                    let mut listed = listed.clone();
                    let mut printed = other.reasons.clone().unwrap_or_default();
                    listed.sort_unstable();
                    printed.sort_unstable();
                    if listed != printed {
                        drift.push(format!(
                            "{} has the reasons {listed:?} in the fixture and {printed:?} in the tool",
                            row.code
                        ));
                    }
                }
            }
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

/// Whether stderr holds the error or the result of a JSON-mode call (spec 5.2): a line that is an envelope, is the
/// document stdout holds, or is an error object.
fn carries_error_or_result(stderr: &[u8], stdout_document: Option<&Value>) -> bool {
    let text = String::from_utf8_lossy(stderr);
    text.lines().any(|line| {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            return false;
        };
        is_envelope(&value)
            || stdout_document == Some(&value)
            || (value.get("code").is_some()
                && value.get("message").is_some()
                && value.get("retryable").is_some())
    })
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

/// Every line of `stdout` that is JSON: the lines of a stream.
fn stream_lines(stdout: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
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

/// The code of a refusal at the start of a `notRun` command that the state or the environment of the fixture caused: exit 2 with an error
/// envelope whose code is registered (when the fixture lists a registry) and is not one a step that reads no state gives (spec 11.7 steps 1-6),
/// which is a fixture error, as any other way to end is. A step that reads no state gives `USAGE` and its finer replacements, `UNKNOWN_COMMAND`,
/// `COMMAND_REQUIRED`, every `INPUT_*` code, any code whose `field` names an argument, and, with the command's document, a code it declares on a
/// constraint or a validator.
fn served_precondition(outcome: &Outcome, registry: Option<&[crate::fixture::RegistryRow]>, doc: Option<&crate::l1::model::CommandDoc>) -> Option<String> {
    if outcome.exit != Exit::Code(2) {
        return None;
    }
    let document = first_document(&outcome.stdout).filter(|document| document.get("ok") == Some(&Value::Bool(false)))?;
    let code = document.pointer("/error/code").and_then(Value::as_str)?;
    let field = document.pointer("/error/field").and_then(Value::as_str);
    let names_argument = field.is_some_and(|field| field.starts_with("--") || field.starts_with('/'));
    let declared = doc.is_some_and(|doc| doc.constraints.iter().any(|constraint| constraint.code.as_deref() == Some(code)) || doc.validators.iter().any(|validator| validator.code == code));
    let stateless = matches!(code, "USAGE" | "UNKNOWN_COMMAND" | "COMMAND_REQUIRED" | "UNKNOWN_OPTION" | "OPTION_REQUIRED" | "OPTION_INVALID") || code.starts_with("INPUT_") || names_argument || declared;
    let registered = registry.is_none_or(|rows| rows.iter().any(|row| row.code == code));
    (!stateless && registered).then(|| code.to_string())
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

/// The output of a terminal run: the start and, when it is longer than [`EXCERPT`], the end too, so that the tail of a long
/// output stays in the report.
fn terminal_excerpt(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let characters: Vec<char> = text.chars().collect();
    if characters.len() <= EXCERPT {
        return text.into_owned();
    }
    let head: String = characters[..EXCERPT / 2].iter().collect();
    let tail: String = characters[characters.len() - EXCERPT / 2..].iter().collect();
    format!("{head}…{tail}")
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
    if outcome.call.terminal {
        entry.insert("terminal".to_string(), json!(true));
    }
    match outcome.exit {
        Exit::Code(code) => entry.insert("exit".to_string(), json!(code)),
        Exit::Signal(signal) => entry.insert("signal".to_string(), json!(signal)),
        Exit::TimedOut => entry.insert("timedOut".to_string(), json!(true)),
    };
    if outcome.served {
        // The run of a `notRun` command that the checker ended by a signal after its first document.
        entry.insert("served".to_string(), json!(true));
    }
    if let Some(at) = &outcome.at_deadline {
        entry.insert(
            "atDeadline".to_string(),
            json!({"output": excerpt(&at.output), "reactedToEof": at.reacted_to_eof}),
        );
    }
    if !outcome.stdout.is_empty() {
        let shown = if outcome.call.terminal { terminal_excerpt(&outcome.stdout) } else { excerpt(&outcome.stdout) };
        entry.insert("stdout".to_string(), json!(shown));
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
        exit_is_allowed, first_document, first_document_text, ids, in_json_mode, is_envelope, needs,
        one_document, with_cursor, Need, TABLE,
    };
    use crate::exec::Exit;
    use crate::report::Strength;

    fn words(items: &[&str]) -> Vec<String> {
        items.iter().map(|word| (*word).to_string()).collect()
    }

    #[test]
    fn atip_check_01_the_table_is_twenty_eight_checks_twenty_one_must_and_seven_should() {
        assert_eq!(ids().len(), 28);
        assert_eq!(
            TABLE
                .iter()
                .filter(|entry| entry.strength == Strength::Must)
                .count(),
            21
        );
        assert!(ids().iter().all(|id| id.starts_with("L0.")));
        let mut unique = ids().to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 28);
        assert_eq!(needs("L0.unknown-option"), [Need::Probe]);
        assert_eq!(needs("L0.ok-exit"), [Need::Registry]);
        assert_eq!(needs("L0.page"), [Need::Commands]);
        assert_eq!(needs("L0.no-prompt"), [Need::Commands]);
        assert_eq!(needs("L0.registry"), [Need::Registry]);
        assert_eq!(needs("L0.global-on-group"), [Need::Commands]);
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

    #[test]
    fn atip_check_60_an_invocation_is_put_in_json_mode_and_given_a_cursor_before_any_dashdash() {
        assert_eq!(in_json_mode(&words(&["a", "b"])), words(&["a", "b", "--json"]));
        assert_eq!(in_json_mode(&words(&["a", "--json"])), words(&["a", "--json"]));
        assert_eq!(in_json_mode(&words(&["a", "--", "x"])), words(&["a", "--json", "--", "x"]));
        assert_eq!(with_cursor(&words(&["list", "--json"])), words(&["list", "--json", "--cursor", "x"]).into_iter().collect::<Vec<_>>());
        assert_eq!(
            with_cursor(&words(&["list", "--cursor", "abc", "--limit", "1", "--json"])),
            words(&["list", "--limit", "1", "--json", "--cursor", "x"])
        );
        assert_eq!(with_cursor(&words(&["list", "--", "-v"])), words(&["list", "--cursor", "x", "--json", "--", "-v"]));
    }
}
