//! The 15 `--input` and batch rows of approaches/checker.md 6 (L2): the framing of a document, how a document and argv are
//! merged, and the two phases of a batch.
//!
//! The invocations are made by [`super::input_plan`]. A row that has nothing to probe is `unsupported` when the documents
//! of the tool say so (no command takes `--input`, none takes a batch) and `unprovided` when only the fixture lacks an
//! `ok` invocation to build the probe from; a probe is never made of a write the fixture does not list.

use std::collections::BTreeSet;

use serde_json::Value;

use super::input_plan::{ConflictExpect, InputCommand, PointerExpect, SameRun};
use super::super::l1_plan::refusal_of;
use super::super::{
    exit_text, must, should, Conditions, End, Entry, Finding, Judge,
    Need::Commands,
};
use crate::claim::Claim;
use crate::exec::Exit;
use crate::l1::invocation;
use crate::schema::schemas;

/// The 15 input and batch rows, in the order of approaches/checker.md.
pub(crate) const TABLE: [Entry; 15] = [
    must("L2.input-present", &["11.1"], &[]),
    must("L2.input-example", &["10.10"], &[]),
    must("L2.input-refusals", &["11.2-11.8", "11.10"], &[Commands]),
    must("L2.input-tty", &["11.3"], &[]),
    must("L2.input-usage", &["11.2"], &[Commands]),
    should("L2.input-reasons", &["7.2.3"], &[Commands]),
    must("L2.input-fragment", &["11.7"], &[Commands]),
    must("L2.input-normalise", &["11.5"], &[Commands]),
    must("L2.input-conflict", &["11.7"], &[Commands]),
    must("L2.input-env", &["11.7"], &[Commands]),
    must("L2.input-pointer", &["11.8"], &[Commands]),
    must("L2.input-dash-values", &["11.9"], &[Commands]),
    must("L2.batch-preflight", &["11.11"], &[Commands]),
    must("L2.batch-incomplete", &["11.11"], &[Commands]),
    must("L2.batch-ok", &["11.11"], &[Commands]),
];

/// The codes of the framing of `--input`, which no state and no later step can answer.
const INPUT_CODES: [&str; 3] = ["INPUT_INVALID", "INPUT_CONFLICT", "INPUT_TOO_LARGE"];

/// Whether `field` is a JSON pointer, or `--input`: where a failure of a document points (spec 11.8).
fn is_pointer_or_input(field: &str) -> bool {
    field == "--input" || field.starts_with('/')
}

/// Whether `field` is `pointer` or lies under it.
fn at_pointer(field: &str, pointer: &str) -> bool {
    field == pointer || field.strip_prefix(pointer).is_some_and(|rest| rest.starts_with('/'))
}

/// The problems the batch-report schema of the contract finds in a batch report.
fn batch_report_problems(data: &Value) -> Vec<String> {
    use std::sync::LazyLock;
    static COMPILED: LazyLock<Result<jsonschema::Validator, String>> =
        LazyLock::new(|| schemas().validator(&serde_json::json!({"$ref": "https://atip.dev/1/batch-report.schema.json"})));
    match &*COMPILED {
        Ok(validator) => validator.iter_errors(data).map(|error| error.to_string()).collect(),
        Err(reason) => vec![format!("the batch-report schema does not compile: {reason}")],
    }
}

impl<'a> Judge<'a> {
    fn input_commands(&self) -> &'a [InputCommand] {
        &self.l2().input.commands
    }

    /// The command documents the checker read from `schema`, when it read a tree.
    fn has_tree(&self) -> bool {
        self.l1.is_some_and(|l1| l1.docs.tree.is_some())
    }

    /// What differs between two runs of the same invocation in two forms: exit, `ok`, the code of a failure, `data` and the members
    /// of `meta` that name the answer. A difference is a problem.
    /// `twin` is the reference run again on a fresh copy of the same state: the paths of `data` whose values differ between the two are varying
    /// (a timestamp, an address derived from one) and are left out of the comparison; exit, `ok`, the code and the rest of `data` are compared.
    fn differences(&self, call: usize, reference: usize, twin: usize) -> Vec<String> {
        let (a, b) = (self.at(call), self.at(reference));
        let mut problems = Vec::new();
        if a.exit != b.exit {
            problems.push(format!("exit {} against exit {} of the argv form", exit_text(a.exit), exit_text(b.exit)));
        }
        let (Some(a_doc), Some(b_doc)) = (self.envelope(call), self.envelope(reference)) else {
            if self.envelope(call).is_some() != self.envelope(reference).is_some() {
                problems.push("one of the two printed no envelope".to_string());
            }
            return problems;
        };
        let mut varying: Vec<String> = Vec::new();
        if let (Some(first), Some(second)) = (self.envelope(reference).and_then(|document| document.get("data")), self.envelope(twin).and_then(|document| document.get("data"))) {
            varying_paths(first, second, String::new(), &mut varying);
        }
        let masked = |document: &Value| {
            let mut data = document.get("data").cloned();
            if let Some(data) = data.as_mut() {
                for path in &varying {
                    if let Some(slot) = data.pointer_mut(path) {
                        *slot = Value::Null;
                    }
                }
            }
            data
        };
        for pointer in ["/ok", "/error/code", "/data", "/meta/command", "/meta/output", "/meta/warnings", "/meta/fields"] {
            let (left, right) = if pointer == "/data" { (masked(a_doc), masked(b_doc)) } else { (a_doc.pointer(pointer).cloned(), b_doc.pointer(pointer).cloned()) };
            if left != right {
                problems.push(format!("{pointer}: {} against {} of the argv form", show_value(left.as_ref()), show_value(right.as_ref())));
            }
        }
        problems
    }

    /// Judges the runs that must give the result of the argv form.
    fn judge_same(&self, c: &mut Conditions<'a>, command: &str, condition: &'static str, runs: &[SameRun]) {
        for run in runs {
            if c.skips_refused(command, run.reference) || c.skips_refused(command, run.call) {
                continue;
            }
            c.own(run.call);
            c.observe(run.reference);
            let problems = self.differences(run.call, run.reference, run.twin);
            c.holds_at(run.call, condition, problems.is_empty(), || {
                format!("{command}: {} did not give the result of the argv form: {}", run.label, problems.join("; "))
            });
        }
    }

    /// How a row with nothing exercised ends: `unprovided` when a command lacked the `ok` invocation the probes start from or a write command is
    /// not listed in the fixture (which is never run, and `unsupported` is never decided from the fixture), else `unsupported` for `why`.
    fn nothing_probed(&self, c: Conditions<'a>, why: &str) -> Finding {
        self.nothing_probed_in(c, why, false)
    }

    /// `nothing_probed`; with `batch`, only the unlisted writes that take a batch count.
    fn nothing_probed_in(&self, c: Conditions<'a>, why: &str, batch: bool) -> Finding {
        if self.fixture.commands.is_none() {
            return c.finish(false);
        }
        let missing: Vec<&InputCommand> = self.input_commands().iter().filter(|command| !command.has_ok).collect();
        let unlisted: Vec<&String> = self
            .l2()
            .input
            .unlisted
            .iter()
            .filter(|id| !batch || self.docs().command(id).is_some_and(|doc| doc.input.as_ref().is_some_and(|input| input.batch)))
            .collect();
        if missing.is_empty() && unlisted.is_empty() {
            return c.finish_as(End::Unsupported(why.to_string()));
        }
        let mut whys: Vec<String> = missing.iter().map(|command| format!("{}: the fixture lists no `ok` invocation of it to build the probes from", command.command)).collect();
        let mut entries: Vec<String> = missing.iter().map(|command| format!("commands.{}.ok", command.command)).collect();
        whys.extend(unlisted.iter().map(|id| format!("{id}: a write the fixture does not list, and the checker never runs one")));
        entries.extend(unlisted.iter().map(|id| format!("commands.{id}")));
        c.finish_as(End::missing_all(whys, entries))
    }

    /// The findings of the 15 rows, in table order.
    pub(crate) fn input_findings(&'a self) -> Vec<Finding> {
        vec![
            self.l2_input_present(),
            self.l2_input_example(),
            self.l2_input_refusals(),
            self.l2_input_tty(),
            self.l2_input_usage(),
            self.l2_input_reasons(),
            self.l2_input_fragment(),
            self.l2_input_normalise(),
            self.l2_input_conflict(),
            self.l2_input_env(),
            self.l2_input_pointer(),
            self.l2_input_dash_values(),
            self.l2_batch_preflight(),
            self.l2_batch_incomplete(),
            self.l2_batch_ok(),
        ]
    }

    fn l2_input_present(&'a self) -> Finding {
        let mut c = self.check("L2.input-present");
        if !self.has_tree() {
            return self.no_tree(c);
        }
        for (id, doc) in &self.docs().commands {
            if matches!(doc.effect.as_str(), "write" | "destructive") {
                c.holds("every-write-has-input", doc.input.is_some(), || format!("{id} is a {} command and its document has no `input`: spec 11.1 gives every write and destructive command --input", doc.effect));
            }
        }
        c.finish(true)
    }

    fn l2_input_example(&'a self) -> Finding {
        let mut c = self.check("L2.input-example");
        if !self.has_tree() {
            return self.no_tree(c);
        }
        let mut judged = false;
        for (id, doc) in &self.docs().commands {
            let Some(input) = &doc.input else { continue };
            judged = true;
            // That the example is a JSON object is `schema-doc.schema.json`'s: L1.walk owns it.
            let example = &input.example;
            if !example.is_object() {
                continue;
            }
            let fragment = doc.compile_ref(&input.schema_ref);
            if let Ok(validator) = &fragment {
                let problems: Vec<String> = validator.iter_errors(example).map(|error| error.to_string()).collect();
                c.holds("example-fragment", problems.is_empty(), || format!("{id}: input.example does not validate against {}: {}", input.schema_ref, problems.join("; ")));
            }
            let (args, value_problems) = invocation::document_arguments(doc, example);
            // A key no option has and a flag-only key are the fragment's; the values are judged here.
            let value_problems: Vec<String> = value_problems.into_iter().filter(|problem| !problem.ends_with("no such option") && !problem.ends_with("flag-only option in a document")).collect();
            c.holds("example-values", value_problems.is_empty(), || format!("{id}: input.example has a value outside its argument: {}", value_problems.join("; ")));
            let required = invocation::requirement_problems(doc, &args);
            c.holds("example-required", required.is_empty(), || format!("{id}: input.example leaves out what the command requires: {}", required.join("; ")));
            let broken: Vec<String> = invocation::violated_constraints(doc, &args).into_iter().filter_map(|index| doc.constraints.get(index)).map(crate::l1::model::Constraint::label).collect();
            c.holds("example-constraints", broken.is_empty(), || format!("{id}: input.example violates {}", broken.join(", ")));
            if let Some(reference) = input.arguments_ref.as_deref() {
                if let Ok(validator) = doc.compile_ref(reference) {
                    let object = invocation::arguments_object(doc, &args);
                    let problems: Vec<String> = validator.iter_errors(&object).map(|error| error.to_string()).collect();
                    c.holds("example-arguments-ref", problems.is_empty(), || format!("{id}: the argument object of input.example ({object}) does not validate against {reference}: {}", problems.join("; ")));
                }
            }
        }
        c.finish_as(if judged { End::Observed } else { End::Unsupported("no command document has an `input`".to_string()) })
    }

    fn l2_input_refusals(&'a self) -> Finding {
        let mut c = self.check("L2.input-refusals");
        let commands = self.input_commands();
        for command in commands {
            for run in &command.refusals {
                c.own(run.call);
                let document = self.envelope(run.call).filter(|document| document.get("ok") == Some(&Value::Bool(false)));
                let (code, field) = document.map_or((None, None), refusal_of);
                let refused = code == Some(run.code);
                c.holds_at(run.call, "refused", refused, || {
                    format!("{}: {} was not refused with {}: {}", command.command, run.label, run.code, super::super::l1::refusal_text(self.envelope(run.call), self.at(run.call).exit))
                });
                if !refused {
                    continue;
                }
                // The size of the document is checked before it is read: INPUT_TOO_LARGE points at `--input`, not into the document.
                let on_field = if run.code == "INPUT_TOO_LARGE" { field == Some("--input") } else { field.is_some_and(is_pointer_or_input) };
                c.holds_at(run.call, "field", on_field, || {
                    format!("{}: {} was refused on {}, not {}", command.command, run.label, field.unwrap_or("no field"), if run.code == "INPUT_TOO_LARGE" { "--input" } else { "a JSON pointer or --input" })
                });
                if let Some(count) = run.details {
                    let found = document.and_then(|document| document.pointer("/error/details")).and_then(Value::as_array).map_or(0, Vec::len);
                    c.holds_at(run.call, "details", found == count, || {
                        format!("{}: {} has {found} entries in details, one for each of the {count} problems of the document", command.command, run.label)
                    });
                }
            }
        }
        if commands.is_empty() {
            return self.nothing_probed(c, "no command takes --input");
        }
        c.finish(true)
    }

    fn l2_input_tty(&'a self) -> Finding {
        let mut c = self.check("L2.input-tty");
        let commands = self.input_commands();
        for command in commands {
            let Some(call) = command.tty else { continue };
            c.own(call);
            if self.at(call).exit == Exit::TimedOut {
                continue;
            }
            let document = self.envelope(call).filter(|document| document.get("ok") == Some(&Value::Bool(false)));
            let (code, field) = document.map_or((None, None), refusal_of);
            let refused = code == Some("INPUT_INVALID");
            c.holds_at(call, "refused", refused, || {
                format!("{}: `--input -` with a terminal on stdin was not refused with INPUT_INVALID: {}", command.command, super::super::l1::refusal_text(self.envelope(call), self.at(call).exit))
            });
            if refused {
                c.holds_at(call, "field", field == Some("--input"), || format!("{}: `--input -` on a terminal was refused on {}, not on --input", command.command, field.unwrap_or("no field")));
            }
        }
        if commands.is_empty() {
            return self.nothing_probed(c, "no command takes --input");
        }
        c.finish(true)
    }

    fn l2_input_usage(&'a self) -> Finding {
        let mut c = self.check("L2.input-usage");
        let mut judged = false;
        for command in self.input_commands() {
            if self.gate(&mut c, &command.command) {
                continue;
            }
            for (label, call) in &command.usage {
                judged = true;
                c.own(*call);
                let document = self.envelope(*call).filter(|document| document.get("ok") == Some(&Value::Bool(false)));
                let (code, field) = document.map_or((None, None), refusal_of);
                // A finer replacement of USAGE is any registered exit-2 code whose field names an argument (not a document's code): the shared rule.
                let is_usage = self.at(*call).exit == Exit::Code(2) && self.is_usage(code, field);
                c.holds_at(*call, "usage", is_usage, || {
                    format!("{}: {label} was not refused with USAGE or its registered finer replacement (exit 2): {}", command.command, super::super::l1::refusal_text(self.envelope(*call), self.at(*call).exit))
                });
                if is_usage {
                    c.holds_at(*call, "field", field == Some("--input"), || format!("{}: {label} was refused on {}, not on --input", command.command, field.unwrap_or("no field")));
                }
            }
        }
        if judged {
            c.finish(true)
        } else {
            self.nothing_probed(c, "no command takes --input")
        }
    }

    fn l2_input_reasons(&'a self) -> Finding {
        let mut c = self.check("L2.input-reasons");
        let listed = |code: &str| self.row(code).and_then(|row| row.reasons.as_ref()).is_some_and(|reasons| !reasons.is_empty());
        if !self.claim.at_least(Claim::V1_1) || !(listed("INPUT_INVALID") || listed("INPUT_TOO_LARGE")) {
            return c.finish_as(End::Unsupported("the registry lists no reasons under INPUT_INVALID or INPUT_TOO_LARGE".to_string()));
        }
        let mut judged = false;
        for command in self.input_commands() {
            let runs = command.refusals.iter().map(|run| (run.label.clone(), run.call, run.reasons.clone())).chain(command.tty.map(|call| ("`--input -` on a terminal".to_string(), call, vec!["STDIN_TTY"])));
            for (label, call, reasons) in runs {
                let document = self.envelope(call).filter(|document| document.get("ok") == Some(&Value::Bool(false)));
                let Some(document) = document.filter(|document| document.pointer("/error/code").and_then(Value::as_str).is_some_and(|code| INPUT_CODES.contains(&code))) else { continue };
                judged = true;
                c.observe(call);
                let reason = document.pointer("/error/reason").and_then(Value::as_str);
                c.holds_at(call, "reserved-reason", reason.is_some_and(|reason| reasons.contains(&reason)), || {
                    format!("{}: {label} was refused with the reason {}, spec 7.2.3 reserves {}", command.command, reason.unwrap_or("none"), reasons.join(" or "))
                });
            }
        }
        c.finish(judged)
    }

    fn l2_input_fragment(&'a self) -> Finding {
        let mut c = self.check("L2.input-fragment");
        let mut judged = false;
        for command in self.input_commands() {
            if self.gate(&mut c, &command.command) {
                continue;
            }
            judged |= !command.fragments.is_empty();
            self.judge_same(&mut c, &command.command, "same-as-argv", &command.fragments);
        }
        if judged {
            c.finish(true)
        } else {
            self.nothing_probed(c, "no fixture invocation of a command with `input` gives a document anything to carry")
        }
    }

    fn l2_input_normalise(&'a self) -> Finding {
        let mut c = self.check("L2.input-normalise");
        let mut judged = false;
        for command in self.input_commands() {
            if self.gate(&mut c, &command.command) {
                continue;
            }
            judged |= !command.normalise.is_empty();
            self.judge_same(&mut c, &command.command, "numeric-string-same-as-number", &command.normalise);
        }
        if judged {
            c.finish(true)
        } else {
            self.nothing_probed(c, "no fixture invocation of a command with `input` gives an integer argument")
        }
    }

    fn l2_input_conflict(&'a self) -> Finding {
        let mut c = self.check("L2.input-conflict");
        let mut judged = false;
        for command in self.input_commands() {
            if self.gate(&mut c, &command.command) {
                continue;
            }
            for run in &command.conflicts {
                judged = true;
                c.own(run.call);
                let document = self.envelope(run.call);
                match &run.expect {
                    ConflictExpect::Conflict { pointer } => {
                        let failure = document.filter(|document| document.get("ok") == Some(&Value::Bool(false))).map(refusal_of);
                        let (code, field) = failure.unwrap_or((None, None));
                        let refused = code == Some("INPUT_CONFLICT");
                        c.holds_at(run.call, "different-value-conflicts", refused, || {
                            format!("{}: {} was not refused with INPUT_CONFLICT: {}", command.command, run.label, super::super::l1::refusal_text(document, self.at(run.call).exit))
                        });
                        if refused {
                            c.holds_at(run.call, "conflict-pointer", field.is_some_and(|field| at_pointer(field, pointer)), || {
                                format!("{}: {} was refused on {}, the document pointer is {pointer}", command.command, run.label, field.unwrap_or("no field"))
                            });
                        }
                    }
                    ConflictExpect::Same => {
                        if c.skips_refused(&command.command, run.call) {
                            continue;
                        }
                        let ok = self.at(run.call).exit == Exit::Code(0) && document.is_some_and(|document| document.get("ok") == Some(&Value::Bool(true)));
                        c.holds_at(run.call, "same-value-succeeds", ok, || {
                            format!("{}: {} is the same value and must succeed (values are compared after normalisation, never as text): {}", command.command, run.label, super::super::l1::refusal_text(document, self.at(run.call).exit))
                        });
                    }
                }
            }
        }
        if judged {
            c.finish(true)
        } else {
            self.nothing_probed(c, "no fixture invocation of a command with `input` gives an argument a document could give again")
        }
    }

    fn l2_input_env(&'a self) -> Finding {
        let mut c = self.check("L2.input-env");
        let declarers = super::input_plan::env_declarers(self.docs().commands.values());
        let mut judged = false;
        for command in self.input_commands() {
            if self.gate(&mut c, &command.command) {
                continue;
            }
            for run in &command.envs {
                judged = true;
                if c.skips_refused(&command.command, run.reference) || c.skips_refused(&command.command, run.call) {
                    continue;
                }
                c.own(run.call);
                c.observe(run.reference);
                let problems = self.differences(run.call, run.reference, run.twin);
                c.holds_at(run.call, "env-applies-after-the-merge", problems.is_empty(), || {
                    format!(
                        "{}: --{} given in the document with {}={} in the environment did not give the result it has without the variable: {} (an env value applies only to an argument absent after the merge, spec 11.7 step 5)",
                        command.command, run.option, run.variable, "an invalid value", problems.join("; ")
                    )
                });
            }
        }
        if judged {
            c.finish(true)
        } else if self.has_tree() && !self.docs().commands.values().any(|doc| doc.input.is_some() && doc.options.iter().any(|option| !option.flag_only && option.env.as_ref().is_some_and(|variable| declarers.get(variable) == Some(&1)))) {
            c.finish_as(End::Unsupported("no option of a command with `input` is the sole declarer of its `env` variable".to_string()))
        } else {
            self.nothing_probed(c, "no option of a command with `input` declares `env` and has a value the checker knows to refuse")
        }
    }

    fn l2_input_pointer(&'a self) -> Finding {
        let mut c = self.check("L2.input-pointer");
        let mut judged = false;
        for command in self.input_commands() {
            if self.gate(&mut c, &command.command) {
                continue;
            }
            let doc = self.docs().command(&command.command);
            for run in &command.pointers {
                judged = true;
                c.own(run.call);
                let document = self.envelope(run.call);
                let failure = document.filter(|document| document.get("ok") == Some(&Value::Bool(false))).map(refusal_of);
                let (code, field) = failure.unwrap_or((None, None));
                match &run.expect {
                    PointerExpect::Constraint { codes } => {
                        let answered = code.is_some_and(|code| codes.iter().any(|expected| expected == code) || (codes.iter().any(|expected| expected == "USAGE") && self.is_usage(Some(code), field)));
                        c.holds_at(run.call, "constraint-code", answered, || {
                            format!("{}: {} was not refused with {}: {}", command.command, run.label, codes.join(" or "), super::super::l1::refusal_text(document, self.at(run.call).exit))
                        });
                        if answered {
                            c.holds_at(run.call, "document-pointer", field.is_some_and(|field| field.starts_with('/')), || {
                                format!("{}: {} was refused on {}, not on a pointer of the document (spec 11.8)", command.command, run.label, field.unwrap_or("no field"))
                            });
                        }
                    }
                    PointerExpect::Required { pointer } => {
                        let declared: Vec<&str> = doc.map(|doc| doc.constraints.iter().filter_map(|constraint| constraint.code.as_deref()).collect()).unwrap_or_default();
                        let answered = code.is_some_and(|code| self.is_usage(Some(code), field) || declared.contains(&code));
                        c.holds_at(run.call, "required-code", answered, || {
                            format!("{}: {} was not refused with USAGE: {}", command.command, run.label, super::super::l1::refusal_text(document, self.at(run.call).exit))
                        });
                        if answered {
                            c.holds_at(run.call, "required-pointer", field.is_some_and(|field| at_pointer(field, pointer)), || {
                                format!("{}: {} was refused on {}, the pointer the key would have is {pointer} (spec 11.8)", command.command, run.label, field.unwrap_or("no field"))
                            });
                        }
                    }
                }
            }
        }
        if judged {
            c.finish(true)
        } else {
            self.nothing_probed(c, "no command with `input` has a constraint or a required argument a document could break")
        }
    }

    /// A refusal whose `field` is the argument and whose `received` is the value given shows that the value was read as a value (a revision that
    /// does not exist, a name that is not there), whatever the code.
    fn read_as_value(&self, run: &super::input_plan::DashRun) -> bool {
        let Some(document) = self.envelope(run.call).filter(|document| document.get("ok") == Some(&Value::Bool(false))) else { return false };
        let named = document.pointer("/error/field").and_then(Value::as_str).is_some_and(|field| {
            let bare = field.trim_start_matches('/').trim_start_matches("--");
            if run.operand { field == "/operands" || field.starts_with("/operands/") || bare == run.argument } else { bare == run.argument || bare == crate::l1::model::camel(&run.argument) }
        });
        // Not a framing code: INPUT_INVALID with the value as `received` is how a tool that read the value as an option or a document refuses it.
        let framing = document.pointer("/error/code").and_then(Value::as_str).is_some_and(|code| code.starts_with("INPUT_") || code == "USAGE");
        named && !framing && document.pointer("/error/received").and_then(Value::as_str) == Some(run.value.as_str())
    }

    fn l2_input_dash_values(&'a self) -> Finding {
        let mut c = self.check("L2.input-dash-values");
        let mut judged = false;
        for command in self.input_commands() {
            if self.gate(&mut c, &command.command) {
                continue;
            }
            // The first candidate that gives a verdict is judged; a candidate whose probes are all refused by a precondition (the state, not the value)
            // cannot tell, and the next is tried. Unprovided only when no candidate decides.
            let groups: BTreeSet<usize> = command.dashes.iter().map(|run| run.group).collect();
            let precondition = |run: &super::input_plan::DashRun| match self.fixture_refusal(&command.command, run.call) {
                Some(super::super::Refusal::Precondition(_)) => !self.read_as_value(run),
                _ => false,
            };
            let decided = groups.iter().copied().find(|group| command.dashes.iter().any(|run| run.group == *group && !precondition(run)));
            judged |= !groups.is_empty();
            for run in command.dashes.iter().filter(|run| Some(run.group) == decided.or(groups.first().copied())) {
                if !self.read_as_value(run) && c.skips_refused(&command.command, run.call) {
                    continue;
                }
                c.own(run.call);
                let document = self.envelope(run.call);
                let ok = (self.at(run.call).exit == Exit::Code(0) && document.is_some_and(|document| document.get("ok") == Some(&Value::Bool(true)))) || self.read_as_value(run);
                c.holds_at(run.call, "value-is-a-value", ok, || {
                    format!("{}: {} in a document was not accepted as a value: {}", command.command, run.label, super::super::l1::refusal_text(document, self.at(run.call).exit))
                });
            }
        }
        if judged {
            c.finish(true)
        } else {
            self.nothing_probed(c, "no string argument of a command with `input` admits a value that starts with `-`, `{` or `[`")
        }
    }

    fn batch_commands(&self) -> impl Iterator<Item = &'a InputCommand> + '_ {
        self.input_commands().iter().filter(|command| command.batch.is_some())
    }

    fn l2_batch_preflight(&'a self) -> Finding {
        let mut c = self.check("L2.batch-preflight");
        let mut judged = false;
        for command in self.batch_commands() {
            if self.gate(&mut c, &command.command) {
                continue;
            }
            let Some(plan) = &command.batch else { continue };
            for run in &plan.preflights {
                judged = true;
                c.own(run.call);
                let document = self.envelope(run.call);
                let failure = document.filter(|document| document.get("ok") == Some(&Value::Bool(false))).map(refusal_of);
                let (code, field) = failure.unwrap_or((None, None));
                let answered = code.is_some_and(|code| run.codes.iter().any(|expected| expected == code) || self.is_usage(Some(code), field) && run.codes.iter().any(|expected| expected == "USAGE"));
                c.holds_at(run.call, "item-code", answered, || {
                    format!("{}: {} was not refused with {}: {}", command.command, run.label, run.codes.join(" or "), super::super::l1::refusal_text(document, self.at(run.call).exit))
                });
                if !answered {
                    continue;
                }
                c.holds_at(run.call, "item-pointer", field.is_some_and(|field| field.starts_with(&format!("/{}/", run.index))), || {
                    format!("{}: {} was refused on {}, the pointer starts with /{}/ (spec 11.8)", command.command, run.label, field.unwrap_or("no field"), run.index)
                });
            }
        }
        let unbuildable: Vec<String> = self.batch_commands().filter_map(|command| command.batch.as_ref()).flat_map(|plan| plan.unbuildable.iter().cloned()).collect();
        if !judged && !unbuildable.is_empty() && self.fixture.commands.is_some() {
            let why = format!("every rule an item could break is on flag-only arguments alone, which an item cannot carry: {}", unbuildable.join(", "));
            return self.nothing_probed_in(c, &why, true);
        }
        self.batch_end(c, judged)
    }

    /// How a batch row ends: judged, or nothing to probe because no command takes a batch (unsupported) or lacks the fixture entry.
    fn batch_end(&'a self, c: Conditions<'a>, judged: bool) -> Finding {
        if judged {
            return c.finish(true);
        }
        if self.fixture.commands.is_none() {
            return c.finish(false);
        }
        if self.batch_commands().next().is_none() {
            return self.nothing_probed_in(c, "no command takes a batch (`input.batch`)", true);
        }
        self.nothing_probed_in(c, "no batch could be built from the fixture's `ok` invocations", true)
    }

    fn l2_batch_incomplete(&'a self) -> Finding {
        let mut c = self.check("L2.batch-incomplete");
        let mut judged = false;
        let mut missing: Vec<String> = Vec::new();
        for command in self.batch_commands() {
            if self.gate(&mut c, &command.command) {
                continue;
            }
            let Some(plan) = &command.batch else { continue };
            missing.extend(plan.missing.iter().cloned());
            let Some(doc) = self.docs().command(&command.command) else { continue };
            for run in &plan.incompletes {
                judged = true;
                c.own(run.call);
                let document = self.envelope(run.call);
                let failure = document.filter(|document| document.get("ok") == Some(&Value::Bool(false)));
                let (code, field) = failure.map_or((None, None), refusal_of);
                let stopped = self.at(run.call).exit == Exit::Code(1) && code == Some("BATCH_INCOMPLETE");
                c.holds_at(run.call, "stopped", stopped, || {
                    format!("{}: {} did not stop with BATCH_INCOMPLETE (exit 1): {}", command.command, run.label, super::super::l1::refusal_text(document, self.at(run.call).exit))
                });
                if !stopped {
                    continue;
                }
                let pointer = format!("/{}", run.index);
                c.holds_at(run.call, "failed-index", field == Some(pointer.as_str()), || {
                    format!("{}: {} stopped on {}, the index of the failed item is {pointer}", command.command, run.label, field.unwrap_or("no field"))
                });
                let data = document.and_then(|document| document.get("data"));
                let items = data.and_then(|data| data.get("items")).and_then(Value::as_array);
                c.holds_at(run.call, "report-items", items.is_some(), || format!("{}: {} printed no batch report with `data.items`", command.command, run.label));
                let Some(items) = items else { continue };
                c.holds_at(run.call, "item-count", items.len() == run.items, || {
                    format!("{}: {} submitted {} items and the report has {}: one entry per submitted item", command.command, run.label, run.items, items.len())
                });
                for (at, item) in items.iter().enumerate() {
                    let status = item.get("status").and_then(Value::as_str).unwrap_or("");
                    let (expected, why) = match at.cmp(&run.index) {
                        std::cmp::Ordering::Less => ("written or unchanged", matches!(status, "written" | "unchanged")),
                        std::cmp::Ordering::Equal => ("failed", status == "failed"),
                        std::cmp::Ordering::Greater => ("skipped", status == "skipped"),
                    };
                    c.holds_at(run.call, "item-statuses", why && item.get("index").and_then(Value::as_u64) == u64::try_from(at).ok(), || {
                        format!("{}: {}: item {at} has status {status:?} and index {}, expected {expected} and {at}", command.command, run.label, item.get("index").map_or("none".to_string(), ToString::to_string))
                    });
                    if status == "failed" {
                        let code = item.pointer("/error/code").and_then(Value::as_str);
                        let retryable = item.pointer("/error/retryable").and_then(Value::as_bool);
                        let registered = code.and_then(|code| self.row(code)).map(|row| row.retryable);
                        c.holds_at(run.call, "failed-error-registered", registered.is_some() && retryable == registered, || {
                            format!("{}: {}: the error of the failed item is {} with retryable {}, the registry says {}", command.command, run.label, code.unwrap_or("no code"), retryable.map_or("absent".to_string(), |value| value.to_string()), registered.map_or("nothing".to_string(), |value| value.to_string()))
                        });
                    }
                }
                let problems = data.map(batch_report_problems).unwrap_or_default();
                c.holds_at(run.call, "batch-report-shape", problems.is_empty(), || {
                    format!("{}: {}: the report does not validate against batch-report.schema.json: {}", command.command, run.label, problems.join("; "))
                });
                if let Some(output) = &plan.output {
                    let named = document.and_then(|document| document.pointer("/meta/output")).and_then(Value::as_str);
                    c.holds_at(run.call, "batch-output", named == Some(output.as_str()), || {
                        format!("{}: {}: meta.output is {}, the batch output of {} is {output}", command.command, run.label, named.unwrap_or("absent"), doc.command)
                    });
                }
            }
        }
        if judged && missing.is_empty() {
            return c.finish(true);
        }
        if !missing.is_empty() && self.fixture.commands.is_some() {
            let whys: Vec<String> = missing.iter().map(|key| format!("the fixture has no `{key}`")).collect();
            return c.finish_as(End::missing_all(whys, missing));
        }
        self.batch_end(c, judged)
    }

    fn l2_batch_ok(&'a self) -> Finding {
        let mut c = self.check("L2.batch-ok");
        let mut judged = false;
        for command in self.batch_commands() {
            if self.gate(&mut c, &command.command) {
                continue;
            }
            let Some(plan) = &command.batch else { continue };
            let Some(run) = &plan.ok else { continue };
            judged = true;
            c.own(run.call);
            if c.skips_refused(&command.command, run.call) {
                continue;
            }
            let document = self.envelope(run.call);
            let ok = self.at(run.call).exit == Exit::Code(0) && document.is_some_and(|document| document.get("ok") == Some(&Value::Bool(true)));
            c.holds_at(run.call, "succeeded", ok, || {
                format!("{}: the batch of its {} ok invocations did not succeed: {}", command.command, run.items, super::super::l1::refusal_text(document, self.at(run.call).exit))
            });
            if !ok {
                continue;
            }
            let data = document.and_then(|document| document.get("data"));
            let problems = data.map(batch_report_problems).unwrap_or_default();
            c.holds_at(run.call, "batch-report-shape", problems.is_empty(), || format!("{}: the report does not validate against batch-report.schema.json: {}", command.command, problems.join("; ")));
            let items = data.and_then(|data| data.get("items")).and_then(Value::as_array);
            c.holds_at(run.call, "item-count", items.is_some_and(|items| items.len() == run.items), || {
                format!("{}: the batch holds {} items and the report has {}", command.command, run.items, items.map_or(0, Vec::len))
            });
            for (at, item) in items.into_iter().flatten().enumerate() {
                let status = item.get("status").and_then(Value::as_str).unwrap_or("");
                c.holds_at(run.call, "every-status-succeeded", matches!(status, "written" | "unchanged") && item.get("index").and_then(Value::as_u64) == u64::try_from(at).ok(), || {
                    format!("{}: item {at} of the batch of ok invocations is {status:?}, every item is written or unchanged", command.command)
                });
            }
            if let Some(output) = &plan.output {
                let named = document.and_then(|document| document.pointer("/meta/output")).and_then(Value::as_str);
                c.holds_at(run.call, "batch-output", named == Some(output.as_str()), || format!("{}: meta.output is {}, the batch output is {output}", command.command, named.unwrap_or("absent")));
            }
        }
        self.batch_end(c, judged)
    }

    /// `L1.numbers` at L2: a number beyond the safe range in a document is refused, never rounded.
    pub(crate) fn l2_numbers_in_documents(&'a self, c: &mut Conditions<'a>) -> bool {
        let runs = &self.l2().input.numbers;
        for run in runs {
            c.own(run.call);
            let document = self.envelope(run.call).filter(|document| document.get("ok") == Some(&Value::Bool(false)));
            let code = document.and_then(|document| document.pointer("/error/code")).and_then(Value::as_str);
            let refused = code == Some("INPUT_INVALID");
            c.holds_at(run.call, "beyond-safe-range-refused-in-a-document", refused, || {
                format!(
                    "{}: {} given as the {} {} in a document is one beyond the safe integers and was not refused with INPUT_INVALID: {}",
                    run.command, run.arg, run.form, run.word, super::super::l1::refusal_text(self.envelope(run.call), self.at(run.call).exit)
                )
            });
        }
        !runs.is_empty()
    }
}

/// The JSON pointers of the leaves (or whole values, when the shapes differ) of `a` and `b` that differ.
fn varying_paths(a: &Value, b: &Value, path: String, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(left), Value::Object(right)) if left.len() == right.len() && left.keys().all(|key| right.contains_key(key)) => {
            for (key, value) in left {
                varying_paths(value, &right[key], format!("{path}/{}", key.replace('~', "~0").replace('/', "~1")), out);
            }
        }
        (Value::Array(left), Value::Array(right)) if left.len() == right.len() => {
            for (at, (x, y)) in left.iter().zip(right).enumerate() {
                varying_paths(x, y, format!("{path}/{at}"), out);
            }
        }
        _ if a != b => out.push(path),
        _ => {}
    }
}

fn show_value(value: Option<&Value>) -> String {
    value.map_or_else(|| "absent".to_string(), ToString::to_string)
}
