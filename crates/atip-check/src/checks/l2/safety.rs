//! The 10 safety rows of approaches/checker.md 6 (L2): the repeat of a write, the rules of a destructive command and its plan, work
//! that is accepted and not done, operation keys, and `L2.effects`.
//!
//! **A change is a change of the snapshot**: a preview, a refusal, `--dry-run`, `--help`, `--version` and a successful read
//! change nothing (spec 12.2), except the paths a `read` command with an `effectNote` lists in the fixture's `allowedEffects`. The
//! invocations are made by [`super::safety_plan`] and [`super::effects_plan`].

use serde_json::Value;

use super::effects_plan::EffectsRun;
use super::safety_plan::{digest_of, next_argv, DestructiveRun};
use super::super::l1::refusal_text;
use super::super::l1_plan::refusal_of;
use super::super::{exit_text, must, Conditions, End, Entry, Finding, Judge, Need::Commands};
use crate::exec::Exit;
use crate::l2::glob;

pub(crate) const EFFECTS_ID: &str = "L2.effects";

/// The safety rows, in the order of approaches/checker.md, `L2.effects` last (it is the row of the effects of every level).
pub(crate) const TABLE: [Entry; 9] = [
    must("L2.idempotency", &["12.3"], &[Commands]),
    must("L2.destructive-safety", &["12.4", "12.8"], &[]),
    must("L2.destructive-preview", &["12.2", "12.4", "12.5"], &[Commands]),
    must("L2.destructive-preview-stdin", &["12.5"], &[Commands]),
    must("L2.destructive-stale", &["12.7"], &[Commands]),
    must("L2.destructive-half", &["12.7"], &[Commands]),
    must("L2.destructive-commit", &["12.7"], &[Commands]),
    must("L2.async", &["13.2", "13.3"], &[Commands]),
    must("L2.operation-key", &["13.4"], &[Commands]),
];

/// `L2.effects`: a MUST at L2 and an advisory at L0 and L1 (the caller sets the strength).
pub(crate) const EFFECTS: Entry = must(EFFECTS_ID, &["12.2"], &[Commands]);

/// The words of `data`'s string members, depth first.
fn strings_of(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => {
            for item in items {
                strings_of(item, out);
            }
        }
        Value::Object(members) => {
            for item in members.values() {
                strings_of(item, out);
            }
        }
        _ => {}
    }
}

/// The identifier a start reports: a string of `data` that `meta.next[0].argv` carries, which is what the wait follows.
fn identifier_of(stdout: &[u8]) -> Option<String> {
    let document = super::super::first_document(stdout)?;
    let argv = next_argv(stdout)?;
    let mut strings = Vec::new();
    strings_of(document.get("data")?, &mut strings);
    strings.into_iter().find(|text| !text.is_empty() && argv.contains(text))
}

/// The words of `argv` sorted: two argvs are the same call when they have the same words, wherever an option stands.
fn words_of(argv: &[String]) -> Vec<String> {
    let mut words = argv.to_vec();
    words.sort();
    words
}

const COMPLETE_WORDS: [&str; 7] = ["done", "complete", "completed", "finished", "succeeded", "success", "ready"];

impl<'a> Judge<'a> {
    /// The findings of the nine rows, in table order.
    pub(crate) fn safety_findings(&'a self) -> Vec<Finding> {
        vec![
            self.l2_idempotency(),
            self.l2_destructive_safety(),
            self.l2_destructive_preview(),
            self.l2_destructive_preview_stdin(),
            self.l2_destructive_stale(),
            self.l2_destructive_half(),
            self.l2_destructive_commit(),
            self.l2_async(),
            self.l2_operation_key(),
        ]
    }

    fn safety(&self) -> &'a super::safety_plan::SafetyRun {
        &self.l2().safety
    }

    /// How a row ends when it judged nothing: `unprovided` for the fixture entries it lacks, else `unsupported` for `why`.
    fn nothing_to_judge(&self, c: Conditions<'a>, missing: &[String], key: &str, why: &str) -> Finding {
        if self.fixture.commands.is_none() {
            return c.finish(false);
        }
        if missing.is_empty() {
            return c.finish_as(End::Unsupported(why.to_string()));
        }
        c.finish_as(End::missing_all(
            missing.iter().map(|command| format!("{command}: the fixture's entry for it has no `{key}`")).collect(),
            missing.iter().map(|command| format!("commands.{command}.{key}")).collect(),
        ))
    }

    fn l2_idempotency(&'a self) -> Finding {
        let mut c = self.check("L2.idempotency");
        let plan = self.safety();
        let mut judged = false;
        for run in &plan.idempotency {
            if self.gate(&mut c, &run.command) || c.skips_refused(&run.command, run.first) {
                continue;
            }
            judged = true;
            c.own(run.second);
            c.observe(run.first);
            let (first, second) = (self.at(run.first), self.at(run.second));
            let (first_doc, second_doc) = (self.envelope(run.first), self.envelope(run.second));
            let argv = second.call.argv.join(" ");
            match run.idempotency.as_str() {
                "once" => {
                    let refused = second.exit == Exit::Code(2) && second_doc.is_some_and(|document| document.get("ok") == Some(&Value::Bool(false)));
                    c.holds_at(run.second, "once-second-refused", refused, || {
                        format!("{}: `{argv}` is `once` and its second run was not refused with exit 2: {}", run.command, refusal_text(second_doc, second.exit))
                    });
                }
                kind => {
                    let succeeded = second.exit == Exit::Code(0) && second_doc.is_some_and(|document| document.get("ok") == Some(&Value::Bool(true)));
                    let same = |left: Option<&Value>, right: Option<&Value>| left.and_then(|document| document.get("data")) == right.and_then(|document| document.get("data"));
                    let unchanged = second_doc.and_then(|document| document.pointer("/data/status")).and_then(Value::as_str) == Some("unchanged");
                    c.holds_at(run.second, "second-run-result", succeeded && (unchanged || same(first_doc, second_doc)), || {
                        format!(
                            "{}: `{argv}` is `{kind}` and its second run did not report `unchanged` or the result of the first (exit {}): {}",
                            run.command,
                            exit_text(second.exit),
                            refusal_text(second_doc, second.exit)
                        )
                    });
                    // A side effect the fixture names in `allowedEffects` (an audit line) does not count against replay-safety.
                    // It must also be a path the command's `effectNote` names: a glob of the fixture alone cannot hide a new effect. The excluded paths are in the notes.
                    let excluded = |path: &str| run.allowed.iter().any(|pattern| glob::matches(pattern, path)) && run.note.contains(path.strip_prefix("./").unwrap_or(path));
                    let changed: Vec<&String> = second.changes.iter().filter(|path| !excluded(path)).collect();
                    let left_out: Vec<&String> = second.changes.iter().filter(|path| excluded(path)).collect();
                    if !left_out.is_empty() {
                        let note = format!("{}: the repeat of `{argv}` changed {}, which `allowedEffects` and the `effectNote` name: left out of the comparison", run.command, left_out.iter().map(|path| path.as_str()).collect::<Vec<_>>().join(", "));
                        if !c.notes.contains(&note) {
                            c.notes.push(note);
                        }
                    }
                    c.holds_at(run.second, "second-run-no-new-effect", changed.is_empty(), || {
                        format!("{}: `{argv}` is `{kind}` and its second run changed {}", run.command, changed.iter().map(|path| path.as_str()).collect::<Vec<_>>().join(", "))
                    });
                    let _ = first;
                }
            }
        }
        if !plan.unlisted_writes.is_empty() {
            c.notes.push(format!("writes the fixture does not list, never run: {}", plan.unlisted_writes.join(", ")));
        }
        if judged {
            c.finish(true)
        } else {
            self.nothing_to_judge(c, &plan.unlisted_writes, "ok", "no write command has the idempotency `replay-safe`, `once` or `content-keyed`")
        }
    }

    fn l2_destructive_safety(&'a self) -> Finding {
        let mut c = self.check("L2.destructive-safety");
        if !self.l1.is_some_and(|l1| l1.docs.tree.is_some()) {
            return self.no_tree(c);
        }
        let mut judged = false;
        for (id, doc) in &self.docs().commands {
            if doc.effect != "destructive" {
                continue;
            }
            judged = true;
            for name in ["commit", "plan-digest"] {
                let option = doc.option(name);
                c.holds("option-present", option.is_some(), || format!("{id} is destructive and has no --{name} option: spec 12.4 gives every destructive command --commit and --plan-digest"));
                if let Some(option) = option {
                    c.holds("option-flag-only", option.flag_only, || format!("{id}: --{name} is not `flagOnly`: a document must never carry it (spec 12.8)"));
                }
            }
            c.holds("no-batch", !doc.input.as_ref().is_some_and(|input| input.batch), || format!("{id} is destructive and takes a batch (`input.batch`): spec 12.8 gives a destructive command no batch"));
        }
        if judged {
            c.finish_as(End::Observed)
        } else {
            c.finish_as(End::Unsupported("no command is destructive".to_string()))
        }
    }

    /// The destructive runs, or the finding that says there are none to judge.
    fn destructive_runs(&self, c: &mut Conditions<'a>) -> Vec<&'a DestructiveRun> {
        let mut runs = Vec::new();
        for run in &self.safety().destructive {
            if self.gate(c, &run.command) || c.skips_refused(&run.command, run.preview) {
                continue;
            }
            runs.push(run);
        }
        runs
    }

    fn destructive_end(&'a self, c: Conditions<'a>, judged: bool) -> Finding {
        if judged {
            return c.finish(true);
        }
        if !self.docs().commands.values().any(|doc| doc.effect == "destructive") && self.l1.is_some() {
            return c.finish_as(End::Unsupported("no command is destructive".to_string()));
        }
        self.nothing_to_judge(c, &self.safety().missing_preview, "preview", "no command is destructive")
    }

    fn l2_destructive_preview(&'a self) -> Finding {
        let mut c = self.check("L2.destructive-preview");
        let mut judged = false;
        for run in self.destructive_runs(&mut c) {
            judged = true;
            c.own(run.preview);
            let outcome = self.at(run.preview);
            let document = self.envelope(run.preview);
            let ok = outcome.exit == Exit::Code(0) && document.is_some_and(|document| document.get("ok") == Some(&Value::Bool(true)));
            c.holds_at(run.preview, "preview-succeeds", ok, || format!("{}: the preview did not succeed (exit 0): {}", run.command, refusal_text(document, outcome.exit)));
            c.holds_at(run.preview, "preview-changes-nothing", outcome.changes.is_empty(), || {
                format!("{}: the preview changed {}: a preview writes nothing, whatever its `effectNote` says (spec 12.2)", run.command, outcome.changes.join(", "))
            });
            let data = document.and_then(|document| document.get("data"));
            let digest = data.and_then(|data| data.get("planDigest")).and_then(Value::as_str);
            c.holds_at(run.preview, "plan-digest", !ok || digest.is_some_and(|digest| !digest.is_empty()), || format!("{}: the preview has no `data.planDigest`", run.command));
            c.holds_at(run.preview, "effects-listed", !ok || data.and_then(|data| data.get("effects")).is_some_and(Value::is_array), || format!("{}: the preview has no `data.effects` list", run.command));
            c.holds_at(run.preview, "committed-false", !ok || data.and_then(|data| data.get("committed")) == Some(&Value::Bool(false)), || {
                format!("{}: the preview does not say `data.committed: false`", run.command)
            });
            if ok && digest.is_some() {
                let next = next_argv(&outcome.stdout);
                let carries = next.as_ref().is_some_and(|argv| argv.iter().any(|word| word == "--commit") && argv.iter().any(|word| word == "--json") && digest_of(argv) == digest);
                c.holds_at(run.preview, "next-commits-the-plan", carries, || {
                    format!(
                        "{}: `meta.next[0].argv` ({}) must contain --commit, --plan-digest {} and --json",
                        run.command,
                        next.as_ref().map_or("absent".to_string(), |argv| argv.join(" ")),
                        digest.unwrap_or("<digest>")
                    )
                });
            }
        }
        self.destructive_end(c, judged)
    }

    fn l2_destructive_preview_stdin(&'a self) -> Finding {
        let mut c = self.check("L2.destructive-preview-stdin");
        let mut judged = false;
        for run in self.destructive_runs(&mut c) {
            let Some(stdin) = &run.stdin else { continue };
            judged = true;
            c.own(stdin.preview);
            let outcome = self.at(stdin.preview);
            let document = self.envelope(stdin.preview);
            let ok = outcome.exit == Exit::Code(0) && document.is_some_and(|document| document.get("ok") == Some(&Value::Bool(true)));
            c.holds_at(stdin.preview, "stdin-preview-succeeds", ok, || format!("{}: the preview with its arguments on stdin (`--input -`) did not succeed: {}", run.command, refusal_text(document, outcome.exit)));
            if !ok {
                continue;
            }
            let next = next_argv(&outcome.stdout);
            let Some(argv) = &next else { continue };
            let has_dash = argv.windows(2).any(|pair| pair[0] == "--input" && pair[1] == "-") || argv.iter().any(|word| word == "--input=-");
            if has_dash {
                // The replay of a `next` that needs stdin is the condition above's.
                c.holds_at(stdin.preview, "next-without-stdin", false, || {
                    format!("{}: `meta.next[0].argv` ({}) carries `--input -`: a replay has no stdin to read it from (spec 12.5)", run.command, argv.join(" "))
                });
                continue;
            }
            c.holds_at(stdin.preview, "next-without-stdin", true, String::new);
            let planned = document.and_then(|document| document.pointer("/data/planDigest")).and_then(Value::as_str).is_some();
            if let Some(replay) = stdin.replay.filter(|_| planned) {
                c.own(replay);
                let replayed = self.at(replay);
                let replay_doc = self.envelope(replay);
                let digest = document.and_then(|document| document.pointer("/data/planDigest")).and_then(Value::as_str);
                let same_plan = replayed.exit == Exit::Code(0)
                    && replay_doc.is_some_and(|document| document.get("ok") == Some(&Value::Bool(true)) && document.pointer("/data/planDigest").and_then(Value::as_str) == digest);
                c.holds_at(replay, "next-replays-the-plan", same_plan, || {
                    format!("{}: running `meta.next[0]` of the preview with `--input -` did not replay the same plan: {}", run.command, refusal_text(replay_doc, replayed.exit))
                });
            }
        }
        self.destructive_end(c, judged)
    }

    fn l2_destructive_stale(&'a self) -> Finding {
        let mut c = self.check("L2.destructive-stale");
        let mut judged = false;
        for run in self.destructive_runs(&mut c) {
            let Some(stale) = &run.stale else { continue };
            judged = true;
            c.observe(stale.preview);
            c.own(stale.call);
            let outcome = self.at(stale.call);
            let document = self.envelope(stale.call);
            let failure = document.filter(|document| document.get("ok") == Some(&Value::Bool(false))).map(refusal_of);
            let (code, _) = failure.unwrap_or((None, None));
            let refused = outcome.exit == Exit::Code(2) && code == Some("PLAN_STALE");
            c.holds_at(stale.call, "stale-refused", refused, || {
                format!("{}: a plan digest that differs in one character was not refused with PLAN_STALE (exit 2): {}", run.command, refusal_text(document, outcome.exit))
            });
            if refused {
                let next = next_argv(&outcome.stdout);
                let is_preview = next.as_ref().is_some_and(|argv| !argv.iter().any(|word| word == "--commit" || word == "--plan-digest" || word.starts_with("--plan-digest=")) && argv.first() == self.at(stale.call).call.argv.first());
                c.holds_at(stale.call, "stale-next-is-the-preview", is_preview, || {
                    format!("{}: `meta.next[0]` of PLAN_STALE ({}) is not the preview: it must lead with the command that plans again, without --commit", run.command, next.as_ref().map_or("absent".to_string(), |argv| argv.join(" ")))
                });
            }
        }
        self.destructive_end(c, judged)
    }

    fn l2_destructive_half(&'a self) -> Finding {
        let mut c = self.check("L2.destructive-half");
        let mut judged = false;
        for run in self.destructive_runs(&mut c) {
            let declared: Vec<&str> = self
                .docs()
                .command(&run.command)
                .map(|doc| doc.constraints.iter().filter(|constraint| matches!(constraint.rule.as_str(), "requires" | "forbids" | "atLeastOne" | "exactlyOne")).filter_map(|constraint| constraint.code.as_deref()).collect())
                .unwrap_or_default();
            for (label, call) in &run.halves {
                judged = true;
                c.own(*call);
                let outcome = self.at(*call);
                let document = self.envelope(*call);
                let failure = document.filter(|document| document.get("ok") == Some(&Value::Bool(false))).map(refusal_of);
                let (code, field) = failure.unwrap_or((None, None));
                let answered = outcome.exit == Exit::Code(2) && code.is_some_and(|code| self.is_usage(Some(code), field) || declared.contains(&code));
                c.holds_at(*call, "half-refused", answered, || {
                    format!("{}: {label} was not refused with USAGE, its registered finer code or the code of the `requires` rule (exit 2): {}", run.command, refusal_text(document, outcome.exit))
                });
            }
        }
        self.destructive_end(c, judged)
    }

    fn l2_destructive_commit(&'a self) -> Finding {
        let mut c = self.check("L2.destructive-commit");
        let mut judged = false;
        for run in self.destructive_runs(&mut c) {
            let Some(commit) = &run.commit else { continue };
            judged = true;
            c.observe(commit.preview);
            c.own(commit.call);
            let outcome = self.at(commit.call);
            let document = self.envelope(commit.call);
            let ok = outcome.exit == Exit::Code(0) && document.is_some_and(|document| document.get("ok") == Some(&Value::Bool(true)));
            let committed = ok && document.and_then(|document| document.pointer("/data/committed")) == Some(&Value::Bool(true));
            c.holds_at(commit.call, "commit-committed", committed, || {
                if ok {
                    format!("{}: the commit does not say `data.committed: true`", run.command)
                } else {
                    format!("{}: the preview's `meta.next[0]` did not succeed (exit 0): {}", run.command, refusal_text(document, outcome.exit))
                }
            });
            if !committed {
                continue;
            }
            let planned = self.envelope(commit.preview).and_then(|document| document.pointer("/data/effects")).and_then(Value::as_array).is_some_and(|effects| !effects.is_empty());
            c.holds_at(commit.call, "commit-effects-happen", !planned || !outcome.changes.is_empty(), || {
                format!("{}: the commit planned {} effects and changed nothing", run.command, self.envelope(commit.preview).and_then(|document| document.pointer("/data/effects")).and_then(Value::as_array).map_or(0, Vec::len))
            });
        }
        self.destructive_end(c, judged)
    }

    fn l2_async(&'a self) -> Finding {
        let mut c = self.check("L2.async");
        let plan = self.safety();
        let mut judged = false;
        for (command, declared) in &plan.fixture_async {
            judged = true;
            c.holds("fixture-names-an-async-command", *declared, || format!("{command}: the fixture gives it an `async` entry and its document does not declare `async: true`"));
        }
        for run in &plan.asyncs {
            for outcome in &run.outcomes {
                if self.gate(&mut c, &run.command) || c.skips_refused(&run.command, outcome.start) {
                    continue;
                }
                judged = true;
                c.own(outcome.start);
                let start = self.at(outcome.start);
                let start_doc = self.envelope(outcome.start);
                let name = &outcome.outcome;
                let accepted = start.exit == Exit::Code(0) && start_doc.is_some_and(|document| document.get("ok") == Some(&Value::Bool(true)));
                c.holds_at(outcome.start, "start-accepted", accepted, || format!("{}: the start with the worker at {name} did not exit 0: {}", run.command, refusal_text(start_doc, start.exit)));
                if !accepted {
                    continue;
                }
                if next_argv(&start.stdout).is_some() {
                    c.holds_at(outcome.start, "start-reports-identifier", identifier_of(&start.stdout).is_some(), || {
                        format!("{}: the start reports no identifier that `meta.next[0].argv` carries: `data` must name what the wait follows (spec 13.2)", run.command)
                    });
                }
                let mut words = Vec::new();
                if let Some(data) = start_doc.and_then(|document| document.get("data")) {
                    strings_of(data, &mut words);
                }
                let claims_done = words.iter().any(|word| COMPLETE_WORDS.contains(&word.to_ascii_lowercase().as_str()));
                c.holds_at(outcome.start, "start-is-not-complete", !claims_done, || format!("{}: the start presents the work as complete ({}): accepted is not done (spec 13.2)", run.command, words.join(", ")));
                let next = next_argv(&start.stdout);
                // Always: next[0] is there, and it reads (the command that reads or waits for the outcome, spec 13.2).
                c.holds_at(outcome.start, "start-next-exists", next.is_some(), || format!("{}: the start has no `meta.next[0]`: it must lead with the command that reads or waits for the outcome (spec 13.2)", run.command));
                let Some(next) = next else { continue };
                let reads = self.docs().commands.values().filter(|doc| next.starts_with(&doc.path)).max_by_key(|doc| doc.path.len()).map(|doc| doc.effect == "read");
                c.holds_at(outcome.start, "start-next-reads", reads != Some(false), || format!("{}: `meta.next[0]` of the start ({}) is not a read command: it must read or wait for the outcome, not start more work (spec 13.2)", run.command, next.join(" ")));
                // The three outcomes are driven only when next[0] carries `--wait`, the reserved option of a command that waits (spec 13.3).
                if !next.iter().any(|word| word == "--wait" || word.starts_with("--wait=")) {
                    let note = format!("{}: `meta.next[0]` has no --wait, so the done, failed and timeout outcomes were not checked (unsupported)", run.command);
                    if !c.notes.contains(&note) {
                        c.notes.push(note);
                    }
                    continue;
                }
                let Some(wait) = outcome.wait else { continue };
                c.own(wait);
                let waited = self.at(wait);
                let wait_doc = self.envelope(wait);
                match name.as_str() {
                    "done" => {
                        let done = waited.exit == Exit::Code(0) && wait_doc.is_some_and(|document| document.get("ok") == Some(&Value::Bool(true)));
                        c.holds_at(wait, "done-exits-0", done, || format!("{}: the wait with the worker at done did not exit 0: {}", run.command, refusal_text(wait_doc, waited.exit)));
                    }
                    "failed" => {
                        c.holds_at(wait, "failed-exits-1", waited.exit == Exit::Code(1), || format!("{}: the wait with the worker at failed exited {}, not 1: {}", run.command, exit_text(waited.exit), refusal_text(wait_doc, waited.exit)));
                    }
                    _ => {
                        c.holds_at(wait, "timeout-exits-1", waited.exit == Exit::Code(1), || format!("{}: the wait with the worker at timeout exited {}, not 1: {}", run.command, exit_text(waited.exit), refusal_text(wait_doc, waited.exit)));
                        if waited.exit == Exit::Code(1) {
                            let retryable = wait_doc.and_then(|document| document.pointer("/error/retryable")).and_then(Value::as_bool) == Some(true);
                            c.holds_at(wait, "timeout-retryable", retryable, || {
                                format!("{}: the timed out wait answered retryable {}: spec 13.3 wants a retryable code", run.command, wait_doc.and_then(|document| document.pointer("/error/retryable")).map_or("absent".to_string(), ToString::to_string))
                            });
                            let again = next_argv(&waited.stdout);
                            c.holds_at(wait, "timeout-next-is-the-same-wait", again.as_ref().is_some_and(|argv| words_of(argv) == words_of(&waited.call.argv)), || {
                                format!("{}: `meta.next[0]` of the timeout ({}) is not the wait that ran ({})", run.command, again.as_ref().map_or("absent".to_string(), |argv| argv.join(" ")), waited.call.argv.join(" "))
                            });
                        }
                    }
                }
            }
        }
        if !plan.missing_outcomes.is_empty() {
            let whys: Vec<String> = plan.missing_outcomes.iter().map(|(command, outcomes)| format!("{command}: the fixture's async.outcomes leaves out {}, which the checker cannot drive", outcomes.join(", "))).collect();
            let entries: Vec<String> = plan.missing_outcomes.iter().map(|(command, _)| format!("commands.{command}.async.outcomes")).collect();
            return c.finish_as(End::missing_all(whys, entries));
        }
        if judged {
            c.finish(true)
        } else {
            self.nothing_to_judge(c, &plan.missing_async, "async", "no command declares `async: true`")
        }
    }

    fn l2_operation_key(&'a self) -> Finding {
        let mut c = self.check("L2.operation-key");
        let plan = self.safety();
        let mut judged = false;
        for run in &plan.operations {
            if self.gate(&mut c, &run.command) || c.skips_refused(&run.command, run.first) {
                continue;
            }
            judged = true;
            for call in [run.first, run.repeat, run.lone].into_iter().chain(run.pair).chain(run.other) {
                c.own(call);
            }
            let succeeds = |call: usize| self.at(call).exit == Exit::Code(0) && self.envelope(call).is_some_and(|document| document.get("ok") == Some(&Value::Bool(true)));
            let report = |call: usize| self.envelope(call).and_then(|document| document.get("data")).cloned();
            let repeated = succeeds(run.first) && succeeds(run.repeat);
            c.holds_at(run.repeat, "repeat-succeeds", repeated, || {
                format!("{}: the start with --operation, and its repeat, did not both succeed: {}", run.command, refusal_text(self.envelope(run.repeat), self.at(run.repeat).exit))
            });
            let mut starts_nothing = true;
            if repeated {
                c.holds_at(run.repeat, "repeat-same-report", report(run.first).is_some() && report(run.first) == report(run.repeat), || {
                    format!("{}: a repeat of the same --operation returned {}, the first request {}: a repeat returns the result of the first (spec 13.4)", run.command, report(run.repeat).map_or("nothing".to_string(), |data| data.to_string()), report(run.first).map_or("nothing".to_string(), |data| data.to_string()))
                });
                starts_nothing = self.at(run.repeat).changes.is_empty();
                c.holds_at(run.repeat, "repeat-starts-nothing", starts_nothing, || {
                    format!("{}: a repeat of the same --operation changed {}: it must start no new work (spec 13.4)", run.command, self.at(run.repeat).changes.join(", "))
                });
            }
            // A tool that starts again on a sequential repeat starts again under a concurrent pair too: that is not judged twice.
            if repeated && starts_nothing {
                let both = succeeds(run.pair[0]) && succeeds(run.pair[1]);
                c.holds_at(run.pair[0], "concurrent-one-start", both && run.pair_state == run.lone_state, || {
                    if both {
                        format!("{}: two concurrent starts with one --operation left another state than one start of it alone: the worker saw more than one start (spec 13.4, reservation)", run.command)
                    } else {
                        format!(
                            "{}: two concurrent starts with one fresh --operation did not both succeed: {} / {}",
                            run.command,
                            refusal_text(self.envelope(run.pair[0]), self.at(run.pair[0]).exit),
                            refusal_text(self.envelope(run.pair[1]), self.at(run.pair[1]).exit)
                        )
                    }
                });
            }
            if let Some(other) = run.other {
                let document = self.envelope(other);
                let failure = document.filter(|document| document.get("ok") == Some(&Value::Bool(false))).map(refusal_of);
                let (code, field) = failure.unwrap_or((None, None));
                let reused = self.at(other).exit == Exit::Code(2) && code == Some("OPERATION_REUSED");
                c.holds_at(other, "other-payload-reused", reused, || {
                    format!("{}: the same --operation with another payload was not refused with OPERATION_REUSED (exit 2): {}", run.command, refusal_text(document, self.at(other).exit))
                });
                if reused {
                    c.holds_at(other, "reused-field", field == Some("--operation"), || format!("{}: OPERATION_REUSED names {} as its field, not --operation", run.command, field.unwrap_or("no field")));
                }
            }
        }
        if judged {
            c.finish(true)
        } else if self.docs().commands.values().any(|doc| doc.raw.get("async").and_then(Value::as_bool) == Some(true) && doc.option("operation").is_some()) {
            self.nothing_to_judge(c, &plan.missing_async, "async", "no command takes --operation")
        } else {
            c.finish_as(End::Unsupported("no asynchronous command takes --operation".to_string()))
        }
    }

    /// `L2.effects`: what the invocations that must change nothing changed.
    pub(crate) fn l2_effects(&'a self, effects: &'a EffectsRun) -> Finding {
        let mut c = self.check(EFFECTS_ID);
        // A call that ran over the time limit is the limit's owner's (the L0 check of the same invocation), not a change.
        let timed_out = |call: usize| self.at(call).exit == Exit::TimedOut;
        if let Some(call) = effects.version.filter(|call| !timed_out(*call)) {
            c.own(call);
            let outcome = self.at(call);
            c.holds_at(call, "version-changes-nothing", outcome.changes.is_empty(), || format!("`--version` changed {}", outcome.changes.join(", ")));
        }
        for help in effects.helps.iter().filter(|help| !timed_out(help.call)) {
            c.own(help.call);
            let outcome = self.at(help.call);
            c.holds_at(help.call, "help-changes-nothing", outcome.changes.is_empty(), || format!("`{}` changed {}", help.label, outcome.changes.join(", ")));
        }
        for dry in effects.dry_runs.iter().filter(|dry| !timed_out(dry.call)) {
            c.own(dry.call);
            let outcome = self.at(dry.call);
            if outcome.exit == Exit::Code(2) {
                // A refused `--dry-run` is `L0.refusal-effects`'s; it ran and changed nothing or that row says so.
                c.observe(dry.call);
                continue;
            }
            c.holds_at(dry.call, "dry-run-changes-nothing", outcome.changes.is_empty(), || format!("{}: `--dry-run` changed {}", dry.label, outcome.changes.join(", ")));
        }
        for read in &effects.reads {
            if self.gate(&mut c, &read.command) || c.skips_refused(&read.command, read.call) || timed_out(read.call) {
                continue;
            }
            c.own(read.call);
            let outcome = self.at(read.call);
            // The snapshot counts files only (checker.md 1.3.1 section 4): a directory, made to hold an allowed file or left empty, is no change to judge,
            // so `.canon/audit/access.jsonl` is all a read that creates `.canon/audit` has to allow.
            let stray: Vec<&String> = outcome
                .changes
                .iter()
                .filter(|path| !(read.has_note && read.patterns.iter().any(|pattern| glob::matches(pattern, path))))
                .collect();
            c.holds_at(read.call, "read-changes-only-allowed", stray.is_empty(), || {
                format!(
                    "{}: a successful read changed {}{}",
                    read.command,
                    stray.iter().map(|path| path.as_str()).collect::<Vec<_>>().join(", "),
                    if read.has_note { " (outside its `allowedEffects`)" } else { " (the command has no `effectNote`, so nothing is allowed)" }
                )
            });
        }
        c.finish(self.fixture.commands.is_some())
    }
}
