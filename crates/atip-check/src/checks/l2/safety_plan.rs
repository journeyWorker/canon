//! The invocations of the safety rows (approaches/checker.md 6, L2): repeats of writes, the preview and the commit of a
//! destructive command, asynchronous work with its controllable worker, and the operation keys.
//!
//! Every probe is built from what the fixture lists: a write the fixture does not list is named and never run; a destructive
//! command is run only through the fixture's `preview` invocations. A preview and the commit that follows it run in one
//! sequence (one copy of the state); every other probe has a copy of its own.

use serde_json::Value;

use super::super::l1_plan::L1Run;
use super::super::{first_document, in_json_mode, Invocations, Run};
use crate::exec::{Call, RunError};
use crate::l1::invocation::{self, Invocation};
use crate::l1::model::CommandDoc;
use crate::l1::witness;
use crate::l2::document::{self, Spec, Which};

/// The invocations of the rows after the input ones.
#[derive(Debug, Default)]
pub(crate) struct SafetyRun {
    pub idempotency: Vec<IdempotencyRun>,
    /// The write commands (of the three idempotency values that are tested) that no fixture `ok` lists: named, never run.
    pub unlisted_writes: Vec<String>,
    pub destructive: Vec<DestructiveRun>,
    /// The destructive commands whose fixture entry has no `preview`.
    pub missing_preview: Vec<String>,
    pub asyncs: Vec<AsyncRun>,
    /// The commands whose document declares `async: true` and whose fixture entry has no `async`.
    pub missing_async: Vec<String>,
    /// The commands the fixture gives an `async` entry, and whether their document declares `async: true`.
    pub fixture_async: Vec<(String, bool)>,
    /// The async commands whose fixture entry leaves out some of `done`, `failed` and `timeout`, and which.
    pub missing_outcomes: Vec<(String, Vec<String>)>,
    pub operations: Vec<OperationRun>,
}

/// One `ok` write run again in its sequence.
#[derive(Debug, Clone)]
pub(crate) struct IdempotencyRun {
    pub command: String,
    pub idempotency: String,
    pub first: usize,
    pub second: usize,
    /// The command's fixture `allowedEffects`: named side effects (an audit line) that a repeat may change.
    pub allowed: Vec<String>,
    /// The command's `effectNote`: an allowed path is left out of the comparison only when the note names it.
    pub note: String,
}

/// What a command's preview gave: the plan, the refusal of a stale digest, the halves, the commit, the replay from stdin.
#[derive(Debug, Clone, Default)]
pub(crate) struct DestructiveRun {
    pub command: String,
    pub preview: usize,
    /// The digest altered by one character, run in the sequence of a second preview.
    pub stale: Option<StaleRun>,
    /// `--commit` alone, `--plan-digest x` alone.
    pub halves: Vec<(String, usize)>,
    pub commit: Option<CommitRun>,
    pub stdin: Option<StdinRun>,
}

#[derive(Debug, Clone)]
pub(crate) struct StaleRun {
    pub preview: usize,
    pub call: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct CommitRun {
    pub preview: usize,
    pub call: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct StdinRun {
    pub preview: usize,
    /// The preview's `next[0]` run in its sequence, when it had one.
    pub replay: Option<usize>,
}

/// One outcome of the worker: the start, the wait that `next[0]` named, and the argv run.
#[derive(Debug, Clone)]
pub(crate) struct OutcomeRun {
    pub outcome: String,
    pub start: usize,
    pub wait: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct AsyncRun {
    pub command: String,
    pub outcomes: Vec<OutcomeRun>,
}

/// The operation key probes of one asynchronous start.
#[derive(Debug, Clone)]
pub(crate) struct OperationRun {
    pub command: String,
    /// The start with a key, and the same start again in its sequence.
    pub first: usize,
    pub repeat: usize,
    /// Two starts with a fresh key at once, and one start of that key alone, with the state each left.
    pub pair: [usize; 2],
    pub pair_state: String,
    pub lone: usize,
    pub lone_state: String,
    /// The key again with another payload (`None` when the start has no argument to change).
    pub other: Option<usize>,
}

/// `next[0].argv` of the envelope in `stdout`, when it is a list of strings.
pub(crate) fn next_argv(stdout: &[u8]) -> Option<Vec<String>> {
    let document = first_document(stdout)?;
    document
        .pointer("/meta/next/0/argv")?
        .as_array()?
        .iter()
        .map(|word| word.as_str().map(str::to_string))
        .collect()
}

/// The value of `--plan-digest` in `argv`, written as two words (`--plan-digest <v>`) or one (`--plan-digest=<v>`, the form of spec 9.12.1).
pub(crate) fn digest_of(argv: &[String]) -> Option<&str> {
    argv.iter().enumerate().find_map(|(at, word)| match word.strip_prefix("--plan-digest=") {
        Some(value) => Some(value),
        None if word == "--plan-digest" => argv.get(at + 1).map(String::as_str),
        None => None,
    })
}

/// The digest with one character changed: another character of the same kind, so that it is still well-formed.
fn altered(digest: &str) -> String {
    let mut chars: Vec<char> = digest.chars().collect();
    if let Some(last) = chars.last_mut() {
        *last = match *last {
            '0' => '1',
            'a' => 'b',
            '9' => '8',
            other if other.is_ascii_digit() => '0',
            other if other.is_ascii_alphabetic() => if other == 'z' { 'y' } else { 'a' },
            _ => 'x',
        };
    }
    chars.into_iter().collect()
}

impl Run<'_> {
    pub(crate) fn plan_safety(&mut self, calls: &Invocations, l1: &L1Run) -> Result<SafetyRun, RunError> {
        let mut out = SafetyRun::default();
        let docs: Vec<CommandDoc> = l1.docs.commands.iter().map(|(id, doc)| CommandDoc { command: id.clone(), ..doc.clone() }).collect();
        let with_timeout: Vec<Vec<String>> = docs.iter().filter(|doc| doc.option("timeout").is_some()).map(|doc| doc.path.clone()).collect();
        for doc in &docs {
            if self.not_run(&doc.command) {
                continue;
            }
            let raw_idempotency = doc.raw.get("idempotency").and_then(Value::as_str).unwrap_or_default().to_string();
            if doc.effect == "write" && matches!(raw_idempotency.as_str(), "replay-safe" | "once" | "content-keyed") {
                let bases = self.ok_bases(calls, doc);
                if bases.is_empty() {
                    out.unlisted_writes.push(doc.command.clone());
                }
                let allowed: Vec<String> = self.fixture.commands.as_ref().and_then(|commands| commands.get(&doc.command)).map(|entry| entry.allowed_effects.clone()).unwrap_or_default();
                let note = doc.raw.get("effectNote").and_then(Value::as_str).unwrap_or_default().to_string();
                for (_, first) in bases {
                    let sequence = self.log[first].sequence;
                    let argv = self.log[first].call.argv.clone();
                    let second = self.invoke_after(sequence, Call::new(argv).measured())?;
                    out.idempotency.push(IdempotencyRun { command: doc.command.clone(), idempotency: raw_idempotency.clone(), first, second, allowed: allowed.clone(), note: note.clone() });
                }
            }
            if doc.effect == "destructive" {
                self.plan_destructive(&mut out, doc)?;
            }
            let declared = doc.raw.get("async").and_then(Value::as_bool) == Some(true);
            if self.fixture.commands.as_ref().and_then(|commands| commands.get(&doc.command)).is_some_and(|entry| entry.async_entry.is_some()) {
                out.fixture_async.push((doc.command.clone(), declared));
            }
            if declared {
                self.plan_async(&mut out, doc, &with_timeout)?;
            }
        }
        Ok(out)
    }

    fn plan_destructive(&mut self, out: &mut SafetyRun, doc: &CommandDoc) -> Result<(), RunError> {
        let previews = self.fixture.commands.as_ref().and_then(|commands| commands.get(&doc.command)).map(|entry| entry.preview.clone()).unwrap_or_default();
        let Some(preview) = previews.first() else {
            out.missing_preview.push(doc.command.clone());
            return Ok(());
        };
        let argv = in_json_mode(preview);
        let preview_call = |this: &mut Self| this.invoke(Call::new(argv.clone()).measured());
        let mut run = DestructiveRun { command: doc.command.clone(), preview: preview_call(self)?, ..DestructiveRun::default() };
        // A stale digest, in the sequence of a preview of its own.
        let stale_preview = preview_call(self)?;
        if let Some(next) = next_argv(&self.log[stale_preview].stdout) {
            if let Some(digest) = digest_of(&next).map(altered) {
                let mut changed = next.clone();
                // The digest is altered in the form next[0] has it: one word, or the word after `--plan-digest`.
                if let Some(at) = changed.iter().position(|word| word.starts_with("--plan-digest=")) {
                    changed[at] = format!("--plan-digest={digest}");
                } else if let Some(at) = changed.iter().position(|word| word == "--plan-digest") {
                    changed[at + 1] = digest;
                }
                let sequence = self.log[stale_preview].sequence;
                let call = self.invoke_after(sequence, Call::new(changed).measured())?;
                run.stale = Some(StaleRun { preview: stale_preview, call });
            }
        }
        // Each half alone.
        for (label, extra) in [("--commit alone", vec!["--commit"]), ("--plan-digest x alone", vec!["--plan-digest", "x"])] {
            let mut half = argv.clone();
            half.extend(extra.into_iter().map(str::to_string));
            let call = self.invoke(Call::new(half).measured())?;
            run.halves.push((label.to_string(), call));
        }
        // The commit, in the sequence of its preview.
        let commit_preview = preview_call(self)?;
        if let Some(next) = next_argv(&self.log[commit_preview].stdout) {
            let sequence = self.log[commit_preview].sequence;
            let call = self.invoke_after(sequence, Call::new(next).measured())?;
            run.commit = Some(CommitRun { preview: commit_preview, call });
        }
        // The same preview with its arguments on stdin.
        if let Ok(base) = invocation::parse(doc, &argv) {
            let spec = Spec { operands: true, options: Which::All, ..Spec::default() };
            if let Some(split) = document::split(doc, &base, &spec) {
                if !split.document.is_empty() {
                    let stdin = split.text();
                    let call = Call::new(split.argv_with("-")).with_stdin(stdin.into_bytes()).measured();
                    let stdin_preview = self.invoke(call)?;
                    let replay = match next_argv(&self.log[stdin_preview].stdout) {
                        Some(next) => {
                            let sequence = self.log[stdin_preview].sequence;
                            Some(self.invoke_after(sequence, Call::new(next).measured())?)
                        }
                        None => None,
                    };
                    run.stdin = Some(StdinRun { preview: stdin_preview, replay });
                }
            }
        }
        out.destructive.push(run);
        Ok(())
    }

    fn plan_async(&mut self, out: &mut SafetyRun, doc: &CommandDoc, with_timeout: &[Vec<String>]) -> Result<(), RunError> {
        let Some(entry) = self.fixture.commands.as_ref().and_then(|commands| commands.get(&doc.command)).and_then(|entry| entry.async_entry.clone()) else {
            out.missing_async.push(doc.command.clone());
            return Ok(());
        };
        let start_argv = in_json_mode(&entry.start);
        let mut run = AsyncRun { command: doc.command.clone(), outcomes: Vec::new() };
        // The fixture lists what its fake worker supports: an outcome it leaves out is not driven, and the row cannot tell without it.
        let absent: Vec<String> = ["done", "failed", "timeout"].into_iter().filter(|outcome| !entry.outcomes.iter().any(|listed| listed == outcome)).map(str::to_string).collect();
        if !absent.is_empty() {
            out.missing_outcomes.push((doc.command.clone(), absent));
        }
        for outcome in &entry.outcomes {
            let start = self.invoke(Call::new(start_argv.clone()).with_env(&entry.worker, outcome))?;
            let mut wait = None;
            if let Some(mut argv) = next_argv(&self.log[start].stdout) {
                // A wait that hangs is given a short time to hang: the worker of `timeout` never ends.
                let has_timeout = argv.iter().any(|word| word == "--timeout" || word.starts_with("--timeout="));
                if outcome == "timeout" && !has_timeout && with_timeout.iter().any(|path| argv.starts_with(path)) {
                    argv.extend(["--timeout".to_string(), "1s".to_string()]);
                }
                let sequence = self.log[start].sequence;
                wait = Some(self.invoke_after(sequence, Call::new(argv).with_env(&entry.worker, outcome))?);
            }
            run.outcomes.push(OutcomeRun { outcome: outcome.clone(), start, wait });
        }
        out.asyncs.push(run);
        if let Some(operation) = self.plan_operation(doc, &start_argv)? {
            out.operations.push(operation);
        }
        Ok(())
    }

    fn plan_operation(&mut self, doc: &CommandDoc, start_argv: &[String]) -> Result<Option<OperationRun>, RunError> {
        if doc.option("operation").is_none() {
            return Ok(None);
        }
        let with_key = |key: &str, base: &[String]| -> Vec<String> {
            let mut argv = base.to_vec();
            argv.extend(["--operation".to_string(), key.to_string()]);
            argv
        };
        let first = self.invoke(Call::new(with_key("chk-one", start_argv)).measured())?;
        let sequence = self.log[first].sequence;
        let repeat = self.invoke_after(sequence, Call::new(with_key("chk-one", start_argv)).measured())?;
        // Another payload under the key the first request holds, run in the sequence where it was held.
        let other = match invocation::parse(doc, start_argv) {
            Ok(base) => self.other_payload(doc, &base).map(|argv| with_key("chk-one", &argv)),
            Err(_) => None,
        };
        let other = match other {
            Some(argv) => Some(self.invoke_after(sequence, Call::new(argv).measured())?),
            None => None,
        };
        // Two requests with a fresh key at once, and one alone: the worker sees one start in both.
        let calls = [Call::new(with_key("chk-two", start_argv)), Call::new(with_key("chk-two", start_argv))];
        let (pair_sequence, outcomes) = self.runner.run_concurrently(&calls)?;
        let pair_state = self.runner.fingerprint(pair_sequence)?;
        let mut pair = [0, 0];
        for (slot, outcome) in pair.iter_mut().zip(outcomes) {
            self.log.push(outcome);
            *slot = self.log.len() - 1;
        }
        let lone = self.invoke(Call::new(with_key("chk-two", start_argv)))?;
        let lone_state = self.runner.fingerprint(self.log[lone].sequence)?;
        Ok(Some(OperationRun { command: doc.command.clone(), first, repeat, pair, pair_state, lone, lone_state, other }))
    }

    /// The argv of `base` with one argument given another valid value: the payload the same key must refuse.
    fn other_payload(&self, doc: &CommandDoc, base: &Invocation) -> Option<Vec<String>> {
        let given = document::given_names(doc, base);
        let args = invocation::args(doc, base);
        for name in given {
            let Some(arg) = doc.arg(&name) else { continue };
            if arg.is_flag() || arg.name == "operation" || arg.flag_only {
                continue;
            }
            let current = if arg.is_option {
                args.options.get(&name).cloned()
            } else {
                doc.operands.iter().position(|operand| operand.name == name).and_then(|index| args.operands.get(index).cloned())
            }?;
            let avoid = match &current {
                Value::Array(values) => values.clone(),
                other => vec![other.clone()],
            };
            let Some(word) = witness::generate(arg, &avoid) else { continue };
            if let Some(changed) = Self::carrying(doc, std::slice::from_ref(base), arg, &word) {
                return Some(invocation::render(&changed));
            }
        }
        None
    }
}
