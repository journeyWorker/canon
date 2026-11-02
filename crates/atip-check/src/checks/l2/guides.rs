//! The five rows of approaches/checker.md 6 (L2) that are the entry of an agent into the tool: `L2.stream`, `L2.skills`,
//! `L2.guide-commands`, `L2.search` and `L2.help-guide`.
//!
//! The invocations are made by [`super::guides_plan`]; a guide's command lines are read by the engine of `L1.examples`
//! (`l1::invocation::check_line`) after the placeholders are replaced ([`crate::l2::guide`]).

use std::sync::LazyLock;

use serde_json::{json, Value};

use super::guides_plan::StreamRun;
use super::super::l1::refusal_text;
use super::super::l1_plan::refusal_of;
use super::super::{exit_text, must, should, Conditions, End, Entry, Finding, Judge, Need::Commands};
use crate::exec::Exit;
use crate::l1::invocation;
use crate::l2::guide;
use crate::schema::schemas;

pub(crate) const TABLE: [Entry; 5] = [
    must("L2.stream", &["13.5"], &[Commands]),
    must("L2.skills", &["14"], &[]),
    must("L2.guide-commands", &["14.5"], &[]),
    should("L2.search", &["10.11"], &[Commands]),
    must("L2.help-guide", &["4.8"], &[]),
];

static GUIDE: LazyLock<Result<jsonschema::Validator, String>> = LazyLock::new(|| schemas().validator(&json!({"$ref": "https://atip.dev/1/guide.schema.json"})));

fn guide_problems(data: &Value) -> Vec<String> {
    match &*GUIDE {
        Ok(validator) => validator.iter_errors(data).map(|error| error.to_string()).collect(),
        Err(reason) => vec![format!("the guide schema does not compile: {reason}")],
    }
}

/// The names under `guides` of a guide's data.
fn names_of(data: Option<&Value>) -> Vec<String> {
    data.and_then(|data| data.get("guides")).and_then(Value::as_array).into_iter().flatten().filter_map(|name| name.as_str().map(str::to_string)).collect()
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

impl<'a> Judge<'a> {
    fn guides(&self) -> &'a super::guides_plan::GuidesRun {
        &self.l2().guides
    }

    /// The findings of the five rows, in table order.
    pub(crate) fn guides_findings(&'a self) -> Vec<Finding> {
        vec![self.l2_stream(), self.l2_skills(), self.l2_guide_commands(), self.l2_search(), self.l2_help_guide()]
    }

    fn l2_help_guide(&'a self) -> Finding {
        let mut c = self.check("L2.help-guide");
        let Some(l1) = self.l1 else { return c.finish(false) };
        c.own(l1.root_help);
        let text = String::from_utf8_lossy(&self.at(l1.root_help).stdout).into_owned();
        let first = text.lines().next().unwrap_or_default();
        c.holds_at(l1.root_help, "mentions-skills-show", first.contains("skills show"), || {
            format!("the first line of `--help` is {:?}, it must point at `skills show` too (spec 4.8)", super::super::excerpt(first.as_bytes()))
        });
        c.finish(true)
    }

    fn l2_skills(&'a self) -> Finding {
        let mut c = self.check("L2.skills");
        let plan = &self.guides().skills;
        let Some(default) = plan.default else { return c.finish(false) };
        let mut calls: Vec<(String, usize)> = vec![("skills show".to_string(), default)];
        calls.extend(plan.guides.iter().map(|(name, call)| (format!("skills show {name}"), *call)));
        if let Some(unknown) = plan.unknown {
            calls.push(("skills show nosuch-xyz".to_string(), unknown));
        }
        calls.extend(plan.listed.iter().map(|(argv, call)| (format!("the fixture's `{argv}`"), *call)));
        for (label, call) in &calls {
            c.own(*call);
            c.holds_at(*call, "writes-nothing", self.at(*call).changes.is_empty(), || format!("`{label}` changed {}", self.at(*call).changes.join(", ")));
        }
        let succeeded = |call: usize| self.at(call).exit == Exit::Code(0) && self.envelope(call).is_some_and(|document| document.get("ok") == Some(&Value::Bool(true)));
        c.holds_at(default, "works-in-an-empty-directory", succeeded(default), || {
            format!("`skills show` in an empty directory did not succeed: {}", refusal_text(self.envelope(default), self.at(default).exit))
        });
        let data_of = |call: usize| self.envelope(call).and_then(|document| document.get("data"));
        let mut default_names: Vec<String> = Vec::new();
        if succeeded(default) {
            let problems = data_of(default).map(guide_problems).unwrap_or_default();
            c.holds_at(default, "default-guide-shape", problems.is_empty(), || format!("`skills show` does not validate against guide.schema.json: {}", problems.join("; ")));
            default_names = names_of(data_of(default));
            let named = data_of(default).and_then(|data| data.get("name")).and_then(Value::as_str).unwrap_or_default();
            c.holds_at(default, "default-lists-itself", default_names.iter().any(|name| name == named), || format!("`skills show` is the guide {named:?} and its `guides` ({}) does not list it", default_names.join(", ")));
        }
        for (name, call) in &plan.guides {
            if !succeeded(*call) {
                c.holds_at(*call, "works-in-an-empty-directory", false, || format!("`skills show {name}` in an empty directory did not succeed: {}", refusal_text(self.envelope(*call), self.at(*call).exit)));
                continue;
            }
            let problems = data_of(*call).map(guide_problems).unwrap_or_default();
            c.holds_at(*call, "guide-shape", problems.is_empty(), || format!("`skills show {name}` does not validate against guide.schema.json: {}", problems.join("; ")));
            if !problems.is_empty() {
                continue;
            }
            let shown = data_of(*call).and_then(|data| data.get("name")).and_then(Value::as_str).unwrap_or_default();
            c.holds_at(*call, "guide-name", shown == name, || format!("`skills show {name}` prints the guide {shown:?}: `name` is the name asked for"));
            c.holds_at(*call, "guides-agree", sorted(names_of(data_of(*call))) == sorted(default_names.clone()), || {
                format!("`skills show {name}` lists the guides {:?}, `skills show` lists {:?}", names_of(data_of(*call)), default_names)
            });
        }
        if let Some(unknown) = plan.unknown {
            let document = self.envelope(unknown).filter(|document| document.get("ok") == Some(&Value::Bool(false)));
            let (code, _) = document.map_or((None, None), refusal_of);
            let refused = self.at(unknown).exit == Exit::Code(2) && code.is_some_and(|code| self.row(code).is_none_or(|row| row.exit == 2));
            c.holds_at(unknown, "unknown-refused", refused, || format!("`skills show nosuch-xyz` was not refused with a registered code and exit 2: {}", refusal_text(self.envelope(unknown), self.at(unknown).exit)));
            if refused {
                let expected = document.and_then(|document| document.pointer("/error/expected")).and_then(Value::as_array).map(|names| names.iter().filter_map(|name| name.as_str().map(str::to_string)).collect::<Vec<_>>());
                c.holds_at(unknown, "unknown-lists-the-guides", expected.is_some(), || "the refusal of `skills show nosuch-xyz` has no `expected`: the guide names".to_string());
                if let Some(expected) = expected.filter(|_| succeeded(default)) {
                    c.holds_at(unknown, "unknown-expected-agrees", sorted(expected.clone()) == sorted(default_names.clone()), || {
                        format!("the refusal of `skills show nosuch-xyz` expects {expected:?}, `skills show` lists {default_names:?}")
                    });
                }
            }
        }
        c.finish(true)
    }

    fn l2_guide_commands(&'a self) -> Finding {
        let mut c = self.check("L2.guide-commands");
        let plan = &self.guides().skills;
        let Some(docs) = self.l1.map(|l1| &l1.docs) else { return c.finish(false) };
        if docs.tree.is_none() {
            return self.no_tree(c);
        }
        let tool = docs.tool.clone();
        let mut judged = false;
        for (name, call) in &plan.guides {
            let Some(text) = self.envelope(*call).and_then(|document| document.pointer("/data/text")).and_then(Value::as_str) else { continue };
            c.observe(*call);
            for line in guide::lines(text, &tool) {
                let place = format!("guide {name} line {}", line.line);
                let words = match invocation::split_line(&line.text) {
                    Ok(words) => words,
                    Err(why) => {
                        // Spec 14.5 leaves a line with a pipe, a redirection, `;`, `&` or a substitution out: it is reported and not judged.
                        c.notes.push(format!("{place}: not judged, {why} (unsupported)"));
                        continue;
                    }
                };
                judged = true;
                let rest = words.get(1..).unwrap_or_default().to_vec();
                let substituted = guide::substitute(docs, &rest);
                let Some(doc) = guide::command_of(docs, &substituted) else {
                    c.holds_at(*call, "line-parses", false, || format!("{place}: `{}` names no command of the tool", line.text.lines().next().unwrap_or_default()));
                    continue;
                };
                let mut all = vec![tool.clone()];
                all.extend(substituted.iter().cloned());
                let text = guide::quoted(&all);
                let problems: Vec<String> = invocation::check_line(doc, &tool, &text, true)
                    .into_iter()
                    .filter_map(|(condition, problem)| problem.map(|problem| format!("{condition}: {problem}")))
                    .collect();
                c.holds_at(*call, "line-parses", problems.is_empty(), || format!("{place}: {}", problems.join("; ")));
            }
        }
        if judged {
            c.finish_as(End::Observed)
        } else {
            c.finish_as(End::Unsupported("no guide has a fenced command line that starts with the tool name".to_string()))
        }
    }

    fn l2_search(&'a self) -> Finding {
        let mut c = self.check("L2.search");
        let plan = self.guides();
        if !plan.search_offered {
            return c.finish_as(End::Unsupported("the tool's `schema` has no `--search`".to_string()));
        }
        if plan.searches.is_empty() {
            return c.finish_as(End::missing("search", "the tool offers `schema --search` and the fixture lists no question for it"));
        }
        for (query, call) in &plan.searches {
            c.own(*call);
            let document = self.envelope(*call).filter(|document| document.get("ok") == Some(&Value::Bool(true)));
            let data = document.and_then(|document| document.get("data"));
            let tree = data.is_some_and(|data| data.get("kind").and_then(Value::as_str) == Some("tree"));
            c.holds_at(*call, "answers-a-tree", tree, || format!("`schema --search {:?}` did not answer a `tree`: {}", query.query, refusal_text(self.envelope(*call), self.at(*call).exit)));
            if !tree {
                continue;
            }
            c.holds_at(*call, "echoes-the-query", data.and_then(|data| data.get("query")).and_then(Value::as_str) == Some(query.query.as_str()), || format!("the tree answering {:?} has no `query` with that question", query.query));
            let first_five: Vec<&str> = data
                .and_then(|data| data.get("commands"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .take(5)
                .filter_map(|entry| entry.get("command").and_then(Value::as_str))
                .collect();
            c.holds_at(*call, "expected-in-the-first-five", first_five.contains(&query.expect.as_str()), || {
                format!("{:?}: {} is not among the first five entries ({}): ranking is advisory (spec 10.11)", query.query, query.expect, first_five.join(", "))
            });
        }
        c.finish(true)
    }

    /// The lines of an NDJSON stdout that are JSON values.
    fn stream_values(&self, call: usize) -> Vec<Value> {
        String::from_utf8_lossy(&self.at(call).stdout).lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()).collect()
    }

    fn l2_stream(&'a self) -> Finding {
        let mut c = self.check("L2.stream");
        let plan = self.guides();
        if self.l1.is_none_or(|l1| l1.docs.tree.is_none()) {
            return c.finish(false);
        }
        let mut judged = false;
        let mut unexercised: Vec<(String, String)> = Vec::new();
        for stream in &plan.streams {
            if self.gate(&mut c, &stream.command) {
                continue;
            }
            let mut seen: Vec<String> = Vec::new();
            judged |= self.judge_stream(&mut c, stream, &mut seen);
            // A command that declares several stream outputs is run through each of them by some `ok` invocation of the fixture.
            if stream.outputs.len() > 1 {
                unexercised.extend(stream.outputs.iter().filter(|output| !seen.contains(&output.name)).map(|output| (stream.command.clone(), output.name.clone())));
            }
        }
        if judged && plan.missing_stream.is_empty() && unexercised.is_empty() {
            return c.finish(true);
        }
        if judged {
            // What ran is judged; what no invocation of the fixture ran leaves the row unable to say more.
            let missing: Vec<String> = plan.missing_stream.clone();
            if !missing.is_empty() {
                return c.finish_as(End::missing_all(missing.iter().map(|command| format!("{command}: the fixture lists no `ok` invocation")).collect(), missing.iter().map(|command| format!("commands.{command}.ok")).collect()));
            }
            return c.finish_as(End::missing_all(
                unexercised.iter().map(|(command, output)| format!("{command}: no `ok` invocation runs the stream output {output}")).collect(),
                unexercised.iter().map(|(command, _)| format!("commands.{command}.ok")).collect(),
            ));
        }
        if !plan.missing_stream.is_empty() {
            let missing = &plan.missing_stream;
            return c.finish_as(End::missing_all(missing.iter().map(|command| format!("{command}: the fixture lists no `ok` invocation")).collect(), missing.iter().map(|command| format!("commands.{command}.ok")).collect()));
        }
        c.finish_as(End::Unsupported("no command has an output of kind `stream`".to_string()))
    }

    /// Judges the runs of one stream command; `true` when it judged a run.
    fn judge_stream(&'a self, c: &mut Conditions<'a>, stream: &StreamRun, seen: &mut Vec<String>) -> bool {
        let docs = &self.docs();
        let Some(doc) = docs.command(&stream.command) else { return false };
        let mut judged = false;
        for call in &stream.runs {
            if c.skips_refused(&stream.command, *call) {
                continue;
            }
            judged = true;
            c.own(*call);
            let lines = self.stream_values(*call);
            let label = format!("`{}`", self.at(*call).call.argv.join(" "));
            let first = lines.first();
            c.holds_at(*call, "starts-with-start", first.is_some_and(|line| line.get("type").and_then(Value::as_str) == Some("start")), || {
                format!("{label}: the first line is not a `start` line: {}", super::super::excerpt(&self.at(*call).stdout))
            });
            let Some(start) = first.filter(|line| line.get("type").and_then(Value::as_str) == Some("start")) else { continue };
            let declared = &stream.outputs;
            let named = start.get("output").and_then(Value::as_str);
            seen.extend(named.map(str::to_string));
            let output = named.and_then(|name| declared.iter().find(|output| output.name == name)).or_else(|| (declared.len() == 1 && named.is_none()).then(|| &declared[0]));
            c.holds_at(*call, "start-names-the-output", output.is_some(), || match named {
                Some(name) => format!("{label}: the start line names the output {name:?}, which the command does not declare as a stream output"),
                None => format!("{label}: the command declares {} stream outputs and the start line does not say which one runs (`output`)", declared.len()),
            });
            let last = lines.last();
            let ends = lines.iter().filter(|line| line.get("type").and_then(Value::as_str) == Some("end")).count() == 1 && last.is_some_and(|line| line.get("type").and_then(Value::as_str) == Some("end"));
            c.holds_at(*call, "ends-with-end", ends, || format!("{label}: the last line is not the one `end` line: the stream was cut off (spec 13.5.2)"));
            if let Some(output) = output {
                if let Some(validator) = output.event_ref.as_deref().and_then(|reference| doc.compile_ref(reference).ok()) {
                    let bad: Vec<String> = lines
                        .iter()
                        .filter(|line| line.get("type").and_then(Value::as_str) == Some("event"))
                        .filter_map(|line| line.get("data").map(|data| validator.iter_errors(data).map(|error| error.to_string()).collect::<Vec<_>>()))
                        .filter(|problems| !problems.is_empty())
                        .map(|problems| problems.join("; "))
                        .collect();
                    c.holds_at(*call, "events-match-the-event-schema", bad.is_empty(), || format!("{label}: an event fails the eventRef of {}: {}", output.name, bad.join(" / ")));
                }
            }
            let Some(end) = last.filter(|line| line.get("type").and_then(Value::as_str) == Some("end")) else { continue };
            let Some(result) = end.get("result") else { continue };
            let meta = |key: &str| result.pointer(&format!("/meta/{key}")).cloned();
            let agrees = ["tool", "command", "toolVersion", "schemaVersion", "contractVersion"].iter().all(|key| match (start.get(*key), meta(key)) {
                (Some(left), Some(right)) => *left == right,
                (None, None) => true,
                _ => *key == "contractVersion" && start.get(*key).is_none(),
            }) && meta("command").and_then(|command| command.as_str().map(str::to_string)).as_deref() == Some(stream.command.as_str());
            c.holds_at(*call, "start-and-end-agree", agrees, || format!("{label}: the start line and the `end` result disagree on the tool, the command, the versions or the command id"));
            if let (Some(output), Some(shown)) = (output, meta("output")) {
                c.holds_at(*call, "end-names-the-output", shown.as_str() == Some(output.name.as_str()), || format!("{label}: `meta.output` of the `end` result is {shown}, the start line names {}", output.name));
            } else if output.is_some() {
                c.holds_at(*call, "end-names-the-output", false, || format!("{label}: the `end` result has no `meta.output`"));
            }
            let ok = result.get("ok").and_then(Value::as_bool);
            if ok == Some(true) {
                if let Some(validator) = output.and_then(|output| doc.outputs.iter().find(|candidate| candidate.name() == output.name)).and_then(|output| doc.compile_ref(&output.schema_ref).ok()) {
                    let problems: Vec<String> = result.get("data").map_or_else(|| vec!["no data".to_string()], |data| validator.iter_errors(data).map(|error| error.to_string()).collect());
                    c.holds_at(*call, "end-data-matches-the-output", problems.is_empty(), || format!("{label}: the `end` result's data fails the output schema: {}", problems.join("; ")));
                }
            } else if ok == Some(false) {
                let code = result.pointer("/error/code").and_then(Value::as_str);
                let row = code.and_then(|code| self.row(code));
                c.holds_at(*call, "end-failure-is-registered", row.is_some_and(|row| result.pointer("/error/retryable").and_then(Value::as_bool) == Some(row.retryable)), || {
                    format!("{label}: the failed `end` result's code {} and retryable {} do not match the registry", code.unwrap_or("none"), result.pointer("/error/retryable").map_or("absent".to_string(), ToString::to_string))
                });
            }
            let expected = match ok {
                Some(true) => Some(0),
                _ => result.pointer("/error/code").and_then(Value::as_str).and_then(|code| self.row(code)).and_then(|row| i32::try_from(row.exit).ok()),
            };
            c.holds_at(*call, "exit-is-the-end-result", expected.is_some_and(|exit| self.at(*call).exit == Exit::Code(exit)), || {
                format!("{label}: the exit is {} and the `end` result says {}", exit_text(self.at(*call).exit), expected.map_or("an unregistered failure".to_string(), |exit| exit.to_string()))
            });
        }
        // The outputs the runs named, against the ones the command declares.
        // A usage error before the stream begins is one failure envelope and no start line.
        c.own(stream.refusal);
        let refusal = self.at(stream.refusal);
        let values = self.stream_values(stream.refusal);
        let one_envelope = values.len() == 1 && values[0].get("ok") == Some(&Value::Bool(false)) && values[0].get("type").is_none();
        c.holds_at(stream.refusal, "refusal-is-one-envelope", one_envelope && refusal.exit == Exit::Code(2), || {
            format!("`{}`: a usage error before the stream begins must be one failure envelope with exit 2 and no `start` line (spec 13.5.3): {}", refusal.call.argv.join(" "), super::super::excerpt(&refusal.stdout))
        });
        judged
    }
}
