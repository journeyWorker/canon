//! The 31 L1 checks of approaches/checker.md 6 (contract 1.1): self-description, arguments, data.
//!
//! The invocations are made by [`super::l1_plan`]; each check here reads the log and the documents the run read from
//! `schema`. **One owner per pass condition**, as at L0: a condition is judged by exactly one check, so breaking one
//! rule fails one check. Where the JSON Schema of the self-description also rejects a document for a rule another check
//! owns (a path word that is not kebab-case, an option named `--x-id`), the schema finding is dropped from `L1.tree` and
//! `L1.walk` and reported by the owner (`L1.grammar`).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::documents;
use crate::claim::Claim;
use super::l1_plan::{names_command, output_with_fields, refusal_of, L1Run};
use super::{
    excerpt, exit_text, must, should, show, Conditions, End, Entry, Finding, Judge,
    Need::Commands,
};
use crate::exec::Exit;
use crate::l1::invocation;
use crate::l1::model::{Arg, ToolDocs};
use crate::l1::project;
use crate::l1::regex::{self, Comparison};
use crate::l1::rules as doc_rules;
use crate::l1::witness::{Expect, GroupOutcome};

/// The L1 table, in the order of approaches/checker.md.
pub(crate) const TABLE: [Entry; 31] = [
    must("L1.tree", &["10.3", "10.13"], &[]),
    must("L1.no-state", &["10.2"], &[]),
    must("L1.walk", &["10.4", "10.5", "10.13"], &[]),
    must("L1.quoted-path", &["10.1.1"], &[]),
    must("L1.schema-errors", &["7.3.4", "10.1.2"], &[]),
    must("L1.registry", &["7.2.1", "7.3"], &[]),
    must("L1.reasons", &["7.6.1", "7.6.2"], &[]),
    should("L1.reasons-reserved", &["7.2.3"], &[]),
    must("L1.warnings", &["7.5.2", "7.5.3", "10.1"], &[]),
    must("L1.reserved-applicable", &["7.2.1"], &[]),
    must("L1.codes-known", &["10.5"], &[]),
    must("L1.groups-not-runnable", &["9.2"], &[]),
    must("L1.grammar", &["9.1", "9.5", "9.8"], &[]),
    should("L1.depth", &["9.1"], &[]),
    must("L1.one-meaning", &["9.6"], &[]),
    should("L1.one-meaning-values", &["9.6"], &[]),
    must("L1.examples", &["10.5", "10.10"], &[]),
    must("L1.example-runs", &["10.5"], &[Commands]),
    must("L1.constraints", &["10.8"], &[Commands]),
    must("L1.domains", &["10.7.4"], &[Commands]),
    must("L1.patterns", &["10.7.4"], &[Commands]),
    must("L1.numbers", &["10.7.5"], &[Commands]),
    must("L1.validators", &["10.8.3"], &[Commands]),
    must("L1.output-data", &["6.6", "6.11", "10.9"], &[Commands]),
    must("L1.next", &["6.8"], &[]),
    should("L1.hint-command", &["7.1.2"], &[]),
    must("L1.help-line", &["4.7"], &[]),
    should("L1.defaults", &["10.7.3"], &[]),
    should("L1.deterministic", &["10.2.4"], &[]),
    must("L1.paging", &["6.9", "7.2.1"], &[Commands]),
    must("L1.fields", &["6.10"], &[Commands]),
];

/// The ids of the L1 checks of a claim of 1.1 or 1.0, in table order.
pub(crate) fn ids() -> [&'static str; 31] {
    TABLE.each_ref().map(|entry| entry.id)
}

/// `L1.check-order`: the only check that only a claim of 1.2 runs (spec 11.7 step 6; approaches/checker.md 6).
pub(crate) const CHECK_ORDER: Entry = must("L1.check-order", &["11.7"], &[Commands]);

/// The L1 table of a claim: the 31 checks, and from 1.2 `L1.check-order` after `L1.validators`.
pub(crate) fn table(claim: Claim) -> Vec<&'static Entry> {
    let mut rows: Vec<&'static Entry> = TABLE.iter().collect();
    if claim.at_least(Claim::V1_2) {
        let at = rows.iter().position(|entry| entry.id == "L1.validators").map_or(rows.len(), |at| at + 1);
        rows.insert(at, &CHECK_ORDER);
    }
    rows
}


/// The reserved reasons of spec 7.2.3, by code.
const RESERVED_REASONS: [(&str, &[&str]); 3] = [
    (
        "INPUT_INVALID",
        &["UNREADABLE", "STDIN_TTY", "EMPTY", "NOT_JSON", "NOT_OBJECT", "BATCH_UNSUPPORTED", "UNKNOWN_KEY", "FLAG_ONLY_KEY", "INVALID_VALUE"],
    ),
    ("INPUT_TOO_LARGE", &["TOO_MANY_BYTES", "TOO_MANY_ITEMS"]),
    ("CURSOR_INVALID", &["FOREIGN", "ARGUMENTS_CHANGED", "EXPIRED"]),
];

/// Other spellings of a reserved reason that tools use for the same situation.
const REASON_SYNONYMS: [(&str, &str); 22] = [
    ("UNREADABLE_FILE", "UNREADABLE"),
    ("FILE_UNREADABLE", "UNREADABLE"),
    ("READ_FAILED", "UNREADABLE"),
    ("TTY", "STDIN_TTY"),
    ("STDIN_IS_TTY", "STDIN_TTY"),
    ("EMPTY_INPUT", "EMPTY"),
    ("INVALID_JSON", "NOT_JSON"),
    ("MALFORMED_JSON", "NOT_JSON"),
    ("BAD_JSON", "NOT_JSON"),
    ("NOT_AN_OBJECT", "NOT_OBJECT"),
    ("UNKNOWN_FIELD", "UNKNOWN_KEY"),
    ("UNKNOWN_OPTION", "UNKNOWN_KEY"),
    ("FLAG_ONLY", "FLAG_ONLY_KEY"),
    ("BAD_VALUE", "INVALID_VALUE"),
    ("WRONG_TYPE", "INVALID_VALUE"),
    ("TOO_LARGE", "TOO_MANY_BYTES"),
    ("TOO_MANY", "TOO_MANY_ITEMS"),
    ("FOREIGN_CURSOR", "FOREIGN"),
    ("NOT_A_CURSOR", "FOREIGN"),
    ("CHANGED_ARGUMENTS", "ARGUMENTS_CHANGED"),
    ("STALE", "ARGUMENTS_CHANGED"),
    ("CURSOR_EXPIRED", "EXPIRED"),
];

/// Whether `field` names the argument `arg`: its `--name`, its name, its value name, or a pointer ending in its document key.
pub(crate) fn names_argument(field: &str, arg: &Arg) -> bool {
    let squash = |text: &str| -> String { text.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_ascii_lowercase() };
    let field = squash(field.rsplit('/').next().unwrap_or(field));
    field == squash(&arg.name) || arg.value_name.as_ref().is_some_and(|value_name| field == squash(value_name))
}

/// What a failure envelope says, for a reason.
pub(crate) fn refusal_text(document: Option<&Value>, exit: Exit) -> String {
    match document {
        Some(document) if document.get("ok") == Some(&Value::Bool(false)) => {
            let (code, field) = refusal_of(document);
            format!("failed with {}{} (exit {})", code.unwrap_or("no code"), field.map(|field| format!(" on {field}")).unwrap_or_default(), exit_text(exit))
        }
        Some(_) => format!("answered ok (exit {})", exit_text(exit)),
        None => format!("printed no envelope (exit {})", exit_text(exit)),
    }
}

impl<'a> Judge<'a> {
    pub(crate) fn l1(&self) -> &'a L1Run {
        self.l1.expect("the L1 checks run only with an L1 run")
    }

    pub(crate) fn docs(&self) -> &'a ToolDocs {
        &self.l1().docs
    }

    /// The invocation of `schema <id> --json` that printed the document of command `id`.
    fn doc_call(&self, id: &str) -> Option<usize> {
        self.l1().doc_calls.iter().find(|call| call.id == id).map(|call| call.call)
    }

    /// The envelope of an invocation when it is a success.
    pub(crate) fn ok_envelope(&self, index: usize) -> Option<&Value> {
        self.envelope(index).filter(|document| document.get("ok") == Some(&Value::Bool(true)))
    }

    /// Whether a refusal with `code` and `field` is `USAGE` or its registered finer replacement (checker.md 1.3.1 section 2, spec 7.3.2): a registered exit-2
    /// code whose `field` names an argument (`--name`, or a pointer into a document), whatever its name (`UNKNOWN_OPTION`, `OPTION_REQUIRED`, `OPTION_INVALID`).
    /// A reserved code (`UNKNOWN_COMMAND`, `PLAN_STALE`, `CURSOR_INVALID`, an `INPUT_*` code) and a code a command document declares on a constraint or a
    /// validator have a situation of their own and never replace `USAGE`.
    pub(crate) fn is_usage(&self, code: Option<&str>, field: Option<&str>) -> bool {
        code.is_some_and(|code| {
            code == "USAGE"
                || (!code.starts_with("INPUT_")
                    && atip::reserved_code(code, documents::minor_of(self.claim)).is_none()
                    && field.is_some_and(|field| field.starts_with("--") || field.starts_with('/'))
                    && self.row(code).is_some_and(|row| row.exit == 2)
                    && !self.declared_by_a_document(code))
        })
    }

    /// Whether some command document declares `code` on a constraint or a validator.
    fn declared_by_a_document(&self, code: &str) -> bool {
        self.l1.is_some_and(|l1| l1.docs.commands.values().any(|doc| doc.constraints.iter().any(|constraint| constraint.code.as_deref() == Some(code)) || doc.validators.iter().any(|validator| validator.code == code)))
    }

    /// The failure `(code, field)` of an invocation's envelope.
    pub(crate) fn failure(&self, index: usize) -> Option<(Option<&str>, Option<&str>)> {
        self.envelope(index)
            .filter(|document| document.get("ok") == Some(&Value::Bool(false)))
            .map(refusal_of)
    }

    /// Whether the invocation was refused as a usage problem: exit 2 and a failure envelope.
    pub(crate) fn refused(&self, index: usize) -> bool {
        self.at(index).exit == Exit::Code(2) && self.failure(index).is_some()
    }

    /// Whether `code` is one a step before the state is read gives for `command` (approaches/checker.md 5): a code its document declares on a constraint or on a
    /// validator (the fixture's `env` decides a validator that reads an `env` value), or one the run of `L1.domains` saw refuse an argument of it, since a domain declares none.
    pub(crate) fn declared_before_state(&self, command: &str, code: &str) -> bool {
        let Some(doc) = self.docs().command(command) else { return false };
        doc.constraints.iter().any(|constraint| constraint.code.as_deref() == Some(code))
            || doc.validators.iter().any(|validator| validator.code == code)
            || self.l1().domains.iter().any(|run| {
                run.command == command
                    && run.probe.expect == Expect::Refused
                    && self.refused(run.call)
                    && self.failure(run.call).is_some_and(|(found, _)| found == Some(code))
            })
    }

    /// A finding for a check that has nothing to read because the tool printed no tree.
    pub(crate) fn no_tree(&self, c: Conditions<'a>) -> Finding {
        c.finish_as(End::Unsupported("`schema --json` printed no tree: L1.tree reports it".to_string()))
    }

    fn registered_exit(&self, code: &str) -> Option<i64> {
        self.row(code)
            .map(|row| row.exit)
            .or_else(|| atip::reserved_code(code, documents::minor_of(self.claim)).map(|reserved| i64::from(reserved.exit)))
    }

    /// The problems of a `schema` answer against `schema-doc.schema.json`, but for what another check owns: the path words and
    /// option names (`L1.grammar`), a `null` in `meta` or `error` (`L0.no-null`) and members of a later minor (`L0.contract-version`).
    fn schema_doc_problems(&self, document: &Value) -> Vec<String> {
        documents::schema_doc_problems(document, self.claim)
    }

    /// The findings of the 30 checks, in table order.
    pub(crate) fn l1_findings(&'a self) -> Vec<Finding> {
        let mut findings = vec![
            self.l1_tree(),
            self.l1_no_state(),
            self.l1_walk(),
            self.l1_quoted_path(),
            self.l1_schema_errors(),
            self.l1_registry(),
            self.l1_reasons(),
            self.l1_reasons_reserved(),
            self.l1_warnings(),
            self.l1_reserved_applicable(),
            self.l1_codes_known(),
            self.l1_groups_not_runnable(),
            self.l1_grammar(),
            self.l1_depth(),
            self.l1_one_meaning(),
            self.l1_one_meaning_values(),
            self.l1_examples(),
            self.l1_example_runs(),
            self.l1_constraints(),
            self.l1_domains(),
            self.l1_patterns(),
            self.l1_numbers(),
            self.l1_validators(),
            self.l1_output_data(),
            self.l1_next(),
            self.l1_hint_command(),
            self.l1_help_line(),
            self.l1_defaults(),
            self.l1_deterministic(),
            self.l1_paging(),
            self.l1_fields(),
        ];
        if self.claim.at_least(Claim::V1_2) {
            let at = ids().iter().position(|id| *id == "L1.validators").map_or(findings.len(), |at| at + 1);
            findings.insert(at, self.l1_check_order());
        }
        findings
    }

    /// `L1.check-order` (spec 11.7 step 6, a claim of 1.2): a passing invocation changed to break two stages at once is answered by the earlier
    /// stage, with its code and its `field` (what the earlier fault alone is answered with).
    fn l1_check_order(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.check-order");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        let mut judged = 0;
        for run in &l1.orders {
            if self.gate(&mut c, &run.command) {
                continue;
            }
            // The earlier fault alone has to be refused for its answer to say what the stage answers: else `L1.constraints`, `L1.domains` and
            // `L1.validators` say so, and the pair is not judged.
            let Some((alone_code, alone_field)) = self.failure(run.alone).filter(|_| self.refused(run.alone)) else { continue };
            c.own(run.call);
            c.observe(run.alone);
            judged += 1;
            // A passing report says which pairs it judged and against which answer.
            c.notes.push(format!(
                "{}: {} (`{}` against `{}`)",
                run.command,
                run.pair,
                self.at(run.call).call.argv.join(" "),
                self.at(run.alone).call.argv.join(" ")
            ));
            let answer = self.failure(run.call).filter(|_| self.refused(run.call));
            let same = answer.is_some_and(|(code, field)| (code == alone_code || (self.is_usage(code, field) && self.is_usage(alone_code, alone_field))) && field == alone_field);
            c.holds_at(run.call, "earlier-stage-answers", same, || {
                let answered = match answer {
                    Some((code, field)) => format!("was answered {} on {}", code.unwrap_or("no code"), field.unwrap_or("no field")),
                    None => format!("was not refused: {}", refusal_text(self.envelope(run.call), self.at(run.call).exit)),
                };
                format!(
                    "{}: `{}` breaks {} and {} ({}) and {answered}; the earlier stage answers {} on {} (`{}`)",
                    run.command,
                    self.at(run.call).call.argv.join(" "),
                    run.earlier,
                    run.later,
                    run.pair,
                    alone_code.unwrap_or("no code"),
                    alone_field.unwrap_or("no field"),
                    self.at(run.alone).call.argv.join(" ")
                )
            });
        }
        if judged == 0 {
            if l1.order_gaps.is_empty() {
                return c.finish_as(End::Unsupported("no command has two stages that one invocation can break, and answers the earlier fault alone with a refusal".to_string()));
            }
            let entries: Vec<String> = l1.order_gaps.iter().map(|(command, _)| format!("commands.{command}.ok")).collect();
            return c.finish_as(End::missing_all(l1.order_gaps.iter().map(|(command, why)| format!("{command}: {why}")).collect(), entries));
        }
        c.notes.extend(l1.order_gaps.iter().map(|(command, why)| format!("{command}: {why}: not probed")));
        c.finish(true)
    }

    fn l1_tree(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.tree");
        c.own(l1.tree);
        let document = self.ok_envelope(l1.tree);
        c.holds_at(l1.tree, "answered", document.is_some(), || {
            format!("`schema --json` {}", refusal_text(self.envelope(l1.tree), self.at(l1.tree).exit))
        });
        if let Some(document) = document {
            let problems = self.schema_doc_problems(document);
            c.holds_each(
                Some(l1.tree),
                "valid",
                problems.into_iter().map(|problem| format!("`schema --json` does not validate against schema-doc.schema.json: {problem}")).collect(),
            );
            if let Some(tree) = &l1.docs.tree {
                let checked = if self.l2.is_some() { "L2" } else { "L1" };
                if tree.level == "L2" && checked == "L1" {
                    c.notes.push("the tree declares level L2 and this run checked L1: the higher claim was not checked (levels are cumulative, so the declaration meets the L1 claim)".to_string());
                }
                for (condition, problem) in doc_rules::tree_findings(document, tree, checked) {
                    c.holds_at(l1.tree, condition, problem.is_none(), || problem.clone().unwrap_or_default());
                }
            }
        }
        c.finish(true)
    }

    fn l1_no_state(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.no-state");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        c.own(l1.empty);
        let answered = self.ok_envelope(l1.empty).is_some_and(|document| document.pointer("/data/kind").and_then(Value::as_str) == Some("tree"));
        c.holds_at(l1.empty, "answers-in-an-empty-directory", answered, || {
            format!("`schema --json` in an empty directory with no variables but PATH, HOME and LANG {}", refusal_text(self.envelope(l1.empty), self.at(l1.empty).exit))
        });
        let mut measured: Vec<usize> = vec![l1.tree, l1.empty];
        measured.extend(l1.doc_calls.iter().map(|call| call.call));
        for index in measured {
            let changes = &self.at(index).changes;
            c.holds_at(index, "writes-nothing", changes.is_empty(), || {
                format!("{} changed {}", self.label(index), changes.join(", "))
            });
            if !changes.is_empty() {
                c.observe(index);
            }
        }
        c.finish(true)
    }

    fn l1_walk(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.walk");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        let exit_of = |code: &str| self.registered_exit(code);
        let tool = self.l1().docs.tool.clone();
        for call in &l1.doc_calls {
            c.own(call.call);
            let document = self.ok_envelope(call.call);
            c.holds_at(call.call, "answered", document.is_some(), || {
                format!("`schema {} --json` {}", call.id, refusal_text(self.envelope(call.call), self.at(call.call).exit))
            });
            let Some(document) = document else { continue };
            let problems = self.schema_doc_problems(document);
            c.holds_each(
                Some(call.call),
                "valid",
                problems.into_iter().map(|problem| format!("`schema {} --json` does not validate against schema-doc.schema.json: {problem}", call.id)).collect(),
            );
            let kind = document.pointer("/data/kind").and_then(Value::as_str);
            let wanted = if call.is_group { "group" } else { "command" };
            c.holds_at(call.call, "kind", kind == Some(wanted), || {
                format!("`schema {} --json` has data.kind {}, the tree lists it as a {wanted}", call.id, kind.map_or("missing".to_string(), |kind| format!("{kind:?}")))
            });
            let Some(tree) = &l1.docs.tree else { continue };
            if call.is_group {
                if let Some(group) = l1.docs.groups.get(&call.id) {
                    for (condition, problem) in doc_rules::group_findings(group, tree) {
                        c.holds_at(call.call, condition, problem.is_none(), || problem.clone().unwrap_or_default());
                    }
                }
            } else if let Some(doc) = l1.docs.commands.get(&call.id) {
                for (condition, problem) in doc_rules::command_findings(doc, &tool, self.claim, &exit_of) {
                    // The form of a `pattern` and the Rust limit are `L1.patterns`' (checker.md 6, 7.2).
                    if condition == "rule-3-domain-pattern" {
                        continue;
                    }
                    // At L2 `input.example` is `L2.input-example`'s: one owner for one pass condition.
                    if condition == "rule-7" && self.l2.is_some() {
                        continue;
                    }
                    c.holds_at(call.call, condition, problem.is_none(), || problem.clone().unwrap_or_default());
                }
            }
        }
        c.finish(!l1.doc_calls.is_empty())
    }

    fn l1_quoted_path(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.quoted-path");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        let Some((id, split, quoted)) = &l1.quoted else {
            return c.finish_as(End::Unsupported("the tool has no command of two words".to_string()));
        };
        c.own(*quoted);
        c.observe(*split);
        let data = |index: usize| self.ok_envelope(index).and_then(|document| document.get("data"));
        c.holds_at(*quoted, "same-answer", data(*quoted).is_some() && data(*quoted) == data(*split), || {
            format!("`schema \"{id}\" --json` {}, not the document the words give", refusal_text(self.envelope(*quoted), self.at(*quoted).exit))
        });
        c.finish(true)
    }

    fn l1_schema_errors(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.schema-errors");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        let expect = |c: &mut Conditions<'a>, condition: &'static str, index: usize, code: &str| {
            c.own(index);
            let failure = self.failure(index);
            c.holds_at(index, condition, failure.is_some_and(|(found, field)| found == Some(code) || (code == "USAGE" && self.is_usage(found, field))), || {
                format!("{} {}, expected {code}", self.label(index), refusal_text(self.envelope(index), self.at(index).exit))
            });
        };
        expect(&mut c, "unknown-path", l1.unknown_path, "UNKNOWN_COMMAND");
        // `field`, when the refusal has one, is `command` (spec 10.1.2; an empty `expected` is the envelope schema's: it requires at least one word).
        let refusal = self.envelope(l1.unknown_path).filter(|document| document.get("ok") == Some(&Value::Bool(false)));
        if let Some(refusal) = refusal.filter(|refusal| refusal.pointer("/error/code").and_then(Value::as_str) == Some("UNKNOWN_COMMAND")) {
            let field = refusal.pointer("/error/field").and_then(Value::as_str);
            c.holds_at(l1.unknown_path, "unknown-path-shape", field.is_none_or(|field| field == "command"), || {
                format!("{} has the field {}, which must be \"command\" when there is one", self.label(l1.unknown_path), field.unwrap_or_default())
            });
        }
        expect(&mut c, "overlap", l1.overlap, "USAGE");
        if let Some(index) = l1.group_errors {
            expect(&mut c, "group-with-errors", index, "USAGE");
        }
        expect(&mut c, "unknown-error-code", l1.unknown_error, "USAGE");
        let field = self.failure(l1.unknown_error).and_then(|(_, field)| field);
        c.holds_at(l1.unknown_error, "unknown-error-field", field == Some("--error"), || {
            format!("{} has error.field {}, expected \"--error\"", self.label(l1.unknown_error), field.map_or("missing".to_string(), |field| format!("{field:?}")))
        });
        c.finish(true)
    }

    fn l1_registry(&'a self) -> Finding {
        let mut c = self.check("L1.registry");
        let Some(entries) = &self.tool_errors else {
            c.holds_at(self.calls.schema_errors, "required-codes", false, || "the tool offers no `schema --errors --json`, so its codes cannot be looked up".to_string());
            return c.finish(true);
        };
        c.own(self.calls.schema_errors);
        for (condition, problem) in documents::registry_findings(entries, documents::minor_of(self.claim)) {
            c.holds_at(self.calls.schema_errors, condition, problem.is_none(), || problem.clone().unwrap_or_default());
        }
        let codes: Vec<String> = entries.iter().filter_map(|entry| entry.get("code").and_then(Value::as_str)).map(str::to_string).collect();
        let missing: Vec<&str> = documents::L0_RESERVED
            .iter()
            .copied()
            .filter(|code| !codes.iter().any(|known| known == code))
            .filter(|code| !(*code == "USAGE" && entries.iter().any(|entry| matches!(entry.get("code").and_then(Value::as_str), Some("UNKNOWN_OPTION" | "OPTION_REQUIRED" | "OPTION_INVALID")) && entry.get("exit").and_then(Value::as_u64) == Some(2))))
            .collect();
        c.holds_each(Some(self.calls.schema_errors), "required-codes", missing.iter().map(|code| format!("the registry lacks {code}")).collect());
        let mut unregistered: Vec<String> = Vec::new();
        for doc in self.docs().commands.values() {
            for code in &doc.errors {
                if !codes.contains(code) {
                    unregistered.push(format!("{} lists {code}", doc.command));
                }
            }
        }
        c.holds_each(Some(self.calls.schema_errors), "commands-registered", unregistered.into_iter().map(|why| format!("an error code of a command is not in the registry: {why}")).collect());
        c.finish(true)
    }

    fn l1_reasons(&'a self) -> Finding {
        let mut c = self.check("L1.reasons");
        let Some(entries) = &self.tool_errors else {
            return c.finish_as(End::Unsupported("the tool offers no `schema --errors --json`".to_string()));
        };
        let listing = entries.iter().any(|entry| entry.get("reasons").is_some());
        if !listing {
            return c.finish_as(End::Unsupported("no entry of the registry lists reasons".to_string()));
        }
        c.own(self.calls.schema_errors);
        for (condition, problem) in documents::reasons_findings(entries) {
            c.holds_at(self.calls.schema_errors, condition, problem.is_none(), || problem.clone().unwrap_or_default());
        }
        // The entries `schema --error <CODE>` answers are judged the same way: they are where a tool prints one code's reasons.
        let answered: Vec<Value> = self
            .calls
            .error_entries
            .iter()
            .filter_map(|(_, index)| self.ok_envelope(*index).and_then(|document| document.pointer("/data/error")).cloned())
            .collect();
        for (condition, problem) in documents::reasons_findings(&answered) {
            c.holds_in(None, condition, problem.is_none(), || format!("in `schema --error`: {}", problem.clone().unwrap_or_default()));
        }
        for (code, index) in &self.calls.error_entries {
            let Some(listed) = entries.iter().find(|entry| entry.get("code").and_then(Value::as_str) == Some(code)) else { continue };
            let Some(answered) = self.ok_envelope(*index).and_then(|document| document.pointer("/data/error")) else { continue };
            let names = |entry: &Value| -> Vec<String> {
                let mut names: Vec<String> = entry.get("reasons").and_then(Value::as_array).into_iter().flatten().filter_map(|reason| reason.get("reason").and_then(Value::as_str)).map(str::to_string).collect();
                names.sort_unstable();
                names
            };
            c.observe(*index);
            c.holds_at(*index, "entry-lists-same-reasons", names(listed) == names(answered), || {
                format!("`schema --error {code} --json` lists the reasons {:?}, `--errors` lists {:?}", names(answered), names(listed))
            });
        }
        c.finish(true)
    }

    fn l1_reasons_reserved(&'a self) -> Finding {
        let mut c = self.check("L1.reasons-reserved");
        let Some(entries) = &self.tool_errors else {
            return c.finish_as(End::Unsupported("the tool offers no `schema --errors --json`".to_string()));
        };
        let mut judged = false;
        for entry in entries {
            let code = entry.get("code").and_then(Value::as_str).unwrap_or_default();
            let Some((_, reserved)) = RESERVED_REASONS.iter().find(|(name, _)| *name == code) else { continue };
            for reason in entry.get("reasons").and_then(Value::as_array).into_iter().flatten().filter_map(|reason| reason.get("reason").and_then(Value::as_str)) {
                judged = true;
                let spelled_otherwise = (!reserved.contains(&reason)).then(|| {
                    REASON_SYNONYMS
                        .iter()
                        .find(|(other, name)| *other == reason && reserved.contains(name))
                        .map(|(_, name)| *name)
                        .or_else(|| {
                            let words: Vec<&str> = reason.split('_').collect();
                            reserved.iter().copied().find(|name| name.split('_').all(|word| words.contains(&word)))
                        })
                });
                let spelled_otherwise = spelled_otherwise.flatten();
                c.holds_at(self.calls.schema_errors, "reserved-names", spelled_otherwise.is_none(), || {
                    format!("under {code} the reason {reason} names a situation spec 7.2.3 names {}", spelled_otherwise.unwrap_or_default())
                });
            }
        }
        c.finish(judged)
    }

    fn l1_warnings(&'a self) -> Finding {
        let mut c = self.check("L1.warnings");
        let Some(entries) = &self.tool_warnings else {
            return c.finish_as(End::Unsupported("the tool offers no `schema --warnings`".to_string()));
        };
        c.own(self.calls.schema_warnings);
        for (condition, problem) in documents::warnings_findings(entries) {
            c.holds_at(self.calls.schema_warnings, condition, problem.is_none(), || problem.clone().unwrap_or_default());
        }
        if let Some(index) = self.l1().unknown_warning {
            c.own(index);
            let failure = self.failure(index);
            c.holds_at(index, "unknown-code-refused", failure.is_some_and(|(code, field)| self.is_usage(code, field) && field == Some("--warning")), || {
                format!("{} {}, expected USAGE on --warning", self.label(index), refusal_text(self.envelope(index), self.at(index).exit))
            });
        }
        if let Some(errors) = &self.tool_errors {
            for entry in entries {
                let code = entry.get("code").and_then(Value::as_str).unwrap_or_default();
                let Some(other) = errors.iter().find(|error| error.get("code").and_then(Value::as_str) == Some(code)) else { continue };
                let same = entry.get("meaning") == other.get("meaning");
                c.holds_at(self.calls.schema_warnings, "meaning-agrees", same, || format!("{code} has the meaning {} in the warning registry and {} in the error registry", show(entry.get("meaning")), show(other.get("meaning"))));
            }
        }
        c.finish(true)
    }

    fn l1_reserved_applicable(&'a self) -> Finding {
        let mut c = self.check("L1.reserved-applicable");
        if self.docs().tree.is_none() {
            return self.no_tree(c);
        }
        for doc in self.docs().commands.values() {
            for (condition, problem) in documents::reserved_applicable_findings(doc) {
                c.holds_in(self.doc_call(&doc.command), condition, problem.is_none(), || problem.clone().unwrap_or_default());
            }
        }
        c.finish(!self.docs().commands.is_empty())
    }

    fn l1_codes_known(&'a self) -> Finding {
        let mut c = self.check("L1.codes-known");
        if self.docs().tree.is_none() {
            return self.no_tree(c);
        }
        let mut judged = false;
        for (index, document) in self.envelopes() {
            if document.get("ok") != Some(&Value::Bool(false)) {
                continue;
            }
            let (Some(command), Some(code)) = (document.pointer("/meta/command").and_then(Value::as_str), document.pointer("/error/code").and_then(Value::as_str)) else { continue };
            let Some(doc) = self.docs().command(command) else { continue };
            judged = true;
            let known = doc.errors.iter().any(|listed| listed == code);
            c.holds_at(index, "code-in-errors", known, || format!("{} failed with {code}, which is not in the errors of {command}", self.label(index)));
            if !known {
                c.observe(index);
            }
        }
        c.finish(judged)
    }

    fn l1_groups_not_runnable(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.groups-not-runnable");
        let Some(tree) = &l1.docs.tree else { return self.no_tree(c) };
        for (id, index) in &l1.groups {
            c.own(*index);
            let failure = self.failure(*index);
            let code = failure.and_then(|(code, _)| code);
            c.holds_at(*index, "command-required", code == Some("COMMAND_REQUIRED"), || {
                format!("{} {}, expected COMMAND_REQUIRED", self.label(*index), refusal_text(self.envelope(*index), self.at(*index).exit))
            });
            if code != Some("COMMAND_REQUIRED") {
                continue;
            }
            let path = tree.commands.iter().find(|entry| entry.command == *id).map(|entry| entry.path.clone()).unwrap_or_default();
            let mut wanted: Vec<String> = tree.children(&path).iter().filter_map(|entry| entry.path.last().cloned()).collect();
            wanted.sort_unstable();
            let mut expected: Vec<String> = self
                .envelope(*index)
                .and_then(|document| document.pointer("/error/expected"))
                .and_then(Value::as_array)
                .map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default();
            expected.sort_unstable();
            c.holds_at(*index, "expected-is-the-groups-verbs", expected == wanted, || format!("{} has expected {expected:?}, the verbs of the group are {wanted:?}", self.label(*index)));
        }
        c.finish(!l1.groups.is_empty())
    }

    fn l1_grammar(&'a self) -> Finding {
        let mut c = self.check("L1.grammar");
        let Some(tree) = &self.docs().tree else { return self.no_tree(c) };
        let l1_tree_call = self.l1().tree;
        let paths: Vec<(&str, &[String])> = tree.commands.iter().map(|entry| (entry.command.as_str(), entry.path.as_slice())).collect();
        for (condition, problem) in documents::grammar_findings(&paths, None, documents::minor_of(self.claim)) {
            c.holds_in(Some(l1_tree_call), condition, problem.is_none(), || problem.clone().unwrap_or_default());
        }
        for doc in self.docs().commands.values() {
            // At L2 `commit` and `plan-digest` of a destructive command are flag-only by `L2.destructive-safety`'s judgment: one owner for one pass condition.
            let mut judged = doc.clone();
            if self.l2.is_some() && doc.effect == "destructive" {
                for option in judged.options.iter_mut().filter(|option| matches!(option.name.as_str(), "commit" | "plan-digest")) {
                    option.flag_only = true;
                }
            }
            for (condition, problem) in documents::grammar_findings(&[], Some(&judged), documents::minor_of(self.claim)) {
                c.holds_in(self.doc_call(&doc.command), condition, problem.is_none(), || problem.clone().unwrap_or_default());
            }
        }
        c.finish(true)
    }

    fn l1_depth(&'a self) -> Finding {
        let mut c = self.check("L1.depth");
        let Some(tree) = &self.docs().tree else { return self.no_tree(c) };
        let l1_tree_call = self.l1().tree;
        for entry in &tree.commands {
            c.holds_in(Some(l1_tree_call), "at-most-two-words", entry.path.len() <= 2, || format!("{} has {} words; a path should have at most two (spec 9.1)", entry.command, entry.path.len()));
        }
        c.finish(!tree.commands.is_empty())
    }

    fn l1_one_meaning(&'a self) -> Finding {
        let mut c = self.check("L1.one-meaning");
        if self.docs().tree.is_none() {
            return self.no_tree(c);
        }
        let mut names: BTreeMap<&str, Vec<(&str, &Arg)>> = BTreeMap::new();
        for doc in self.docs().commands.values() {
            for option in &doc.options {
                names.entry(option.name.as_str()).or_default().push((doc.command.as_str(), option));
            }
        }
        for (name, uses) in &names {
            let first = uses[0];
            let other_name = uses.iter().find(|(_, option)| option.value_name != first.1.value_name);
            c.holds("value-name", other_name.is_none(), || {
                let (command, option) = other_name.copied().unwrap_or(first);
                format!("--{name} has the value name {:?} in {} and {:?} in {command}", first.1.value_name, first.0, option.value_name)
            });
            let other_type = uses.iter().find(|(_, option)| option.value_type != first.1.value_type);
            c.holds("value-type", other_type.is_none(), || {
                let (command, option) = other_type.copied().unwrap_or(first);
                format!("--{name} is {} in {} and {} in {command}", first.1.value_type, first.0, option.value_type)
            });
        }
        c.finish(!names.is_empty())
    }

    fn l1_one_meaning_values(&'a self) -> Finding {
        let mut c = self.check("L1.one-meaning-values");
        if self.docs().tree.is_none() {
            return self.no_tree(c);
        }
        let mut names: BTreeMap<&str, Vec<(&str, BTreeSet<&str>)>> = BTreeMap::new();
        for doc in self.docs().commands.values() {
            for option in doc.options.iter().filter(|option| option.value_type == "enum") {
                names.entry(option.name.as_str()).or_default().push((doc.command.as_str(), option.possible_values.iter().map(String::as_str).collect()));
            }
        }
        let mut judged = false;
        for (name, uses) in &names {
            if uses.len() < 2 {
                continue;
            }
            judged = true;
            let disjoint = uses.iter().enumerate().find_map(|(at, (first, values))| {
                uses[at + 1..].iter().find(|(_, other)| values.is_disjoint(other)).map(|(second, _)| (*first, *second))
            });
            c.holds("overlapping", disjoint.is_none(), || {
                let (first, second) = disjoint.unwrap_or_default();
                format!("--{name} has no value in common between {first} and {second}: two meanings behind one name?")
            });
        }
        c.finish(judged)
    }

    fn l1_examples(&'a self) -> Finding {
        let mut c = self.check("L1.examples");
        if self.docs().tree.is_none() {
            return self.no_tree(c);
        }
        let tool = &self.docs().tool;
        let mut judged = false;
        for doc in self.docs().commands.values() {
            judged |= !doc.examples.is_empty();
            for (condition, problem) in documents::examples_findings(doc, tool) {
                c.holds_in(self.doc_call(&doc.command), condition, problem.is_none(), || problem.clone().unwrap_or_default());
            }
        }
        c.finish(judged)
    }

    fn l1_example_runs(&'a self) -> Finding {
        let mut c = self.check("L1.example-runs");
        if self.docs().tree.is_none() {
            return self.no_tree(c);
        }
        let l1 = self.l1();
        for run in &l1.examples {
            c.own(run.call);
            let failure = self.failure(run.call);
            c.holds_at(run.call, "not-usage", failure.is_none_or(|(code, field)| !self.is_usage(code, field)), || {
                format!("the example `{}` of {} was refused as a usage problem: {}", run.line, run.command, refusal_text(self.envelope(run.call), self.at(run.call).exit))
            });
            let ran = self.at(run.call).exit == Exit::Code(0) || failure.is_some();
            c.holds_at(run.call, "exit-or-registered-code", ran, || format!("the example `{}` of {} ended with exit {} and no failure envelope", run.line, run.command, exit_text(self.at(run.call).exit)));
        }
        c.finish(!l1.examples.is_empty())
    }

    fn l1_constraints(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.constraints");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        // First every group that has a witness; an entailed group rests on the groups of its command that passed.
        #[derive(Clone, PartialEq)]
        enum Verdict {
            Pass,
            Fail,
            Entailed,
            Unprovided(String, String),
        }
        let mut verdicts: Vec<Verdict> = Vec::new();
        for run in &l1.constraints {
            let labels = run.group.members.iter().filter_map(|index| self.docs().command(&run.command).and_then(|doc| doc.constraints.get(*index))).map(crate::l1::model::Constraint::label).collect::<Vec<_>>().join(" = ");
            let agree = run.group.codes.len() == 1;
            c.holds_in(None, "equivalent-codes-agree", agree, || {
                format!("{}: the equivalent rules {labels} declare different codes ({}): the expected refusal is ambiguous", run.command, run.group.codes.iter().cloned().collect::<Vec<_>>().join(", "))
            });
            let verdict = match &run.group.outcome {
                _ if !agree => Verdict::Fail,
                GroupOutcome::Witness { words } => {
                    let call = run.call.expect("a witness group has an invocation");
                    if self.gate(&mut c, &run.command) {
                        verdicts.push(Verdict::Fail);
                        continue;
                    }
                    c.own(call);
                    let refused = self.refused(call);
                    c.holds_at(call, "refused", refused, || {
                        format!("{}: `{}` violates only {labels} and was not refused: {}", run.command, words.join(" "), refusal_text(self.envelope(call), self.at(call).exit))
                    });
                    let (code, field) = self.failure(call).unwrap_or((None, None));
                    let declared = code.is_some_and(|code| run.group.codes.contains(code) || (run.group.codes.contains("USAGE") && self.is_usage(Some(code), field)));
                    if refused {
                        c.holds_at(call, "declared-code", declared, || {
                            format!("{}: `{}` violates only {labels} and was refused with {}, the rule declares {}", run.command, words.join(" "), code.unwrap_or("no code"), run.group.codes.iter().cloned().collect::<Vec<_>>().join(", "))
                        });
                    }
                    if refused && declared { Verdict::Pass } else { Verdict::Fail }
                }
                GroupOutcome::Entailed { .. } => Verdict::Entailed,
                GroupOutcome::Unprovided(why) => Verdict::Unprovided(run.command.clone(), format!("{}: {labels}: {why}", run.command)),
            };
            verdicts.push(verdict);
        }
        // The position of the first group of each command, to resolve the group indices an entailed group names.
        let first_of = |command: &str| l1.constraints.iter().position(|run| run.command == command).unwrap_or(0);
        let mut unprovided: Vec<(String, String)> = Vec::new();
        let mut entailed: Vec<String> = Vec::new();
        for (at, run) in l1.constraints.iter().enumerate() {
            let GroupOutcome::Entailed { broken_with } = &run.group.outcome else { continue };
            if verdicts[at] != Verdict::Entailed {
                continue;
            }
            let start = first_of(&run.command);
            let supported = broken_with.iter().all(|broken| broken.required || broken.groups.iter().any(|group| verdicts.get(start + group) == Some(&Verdict::Pass)));
            let label = run.labels.get(at - start).cloned().unwrap_or_default();
            if supported {
                let names: BTreeSet<String> = broken_with.iter().flat_map(|broken| broken.groups.iter().filter_map(|group| run.labels.get(*group).cloned())).collect();
                let required = broken_with.iter().any(|broken| broken.required);
                let mut by: Vec<String> = names.into_iter().collect();
                if required {
                    by.push("a required argument".to_string());
                }
                entailed.push(format!("{}: {label} is entailed by {}", run.command, by.join(", ")));
            } else {
                verdicts[at] = Verdict::Unprovided(run.command.clone(), format!("{}: {label} is entailed only by rules that have no isolated witness", run.command));
            }
        }
        for verdict in &verdicts {
            if let Verdict::Unprovided(command, why) = verdict {
                unprovided.push((command.clone(), why.clone()));
            }
        }
        c.notes.extend(entailed.clone());
        if !unprovided.is_empty() {
            let mut entries: Vec<String> = unprovided.iter().map(|(command, _)| format!("commands.{command}")).collect();
            entries.dedup();
            return c.finish_as(End::missing_all(unprovided.into_iter().map(|(_, why)| why).collect(), entries));
        }
        if l1.constraints.is_empty() {
            return c.finish_as(End::Unsupported("no command that can be probed declares a constraint".to_string()));
        }
        if verdicts.iter().all(|verdict| *verdict == Verdict::Entailed) {
            c.notes.clear();
            return c.finish_as(End::Entailed(entailed));
        }
        let _ = Verdict::Fail;
        c.finish(true)
    }

    fn l1_domains(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.domains");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        for run in &l1.domains {
            if self.gate(&mut c, &run.command) {
                continue;
            }
            let Some(doc) = self.docs().command(&run.command) else { continue };
            let Some(arg) = doc.arg(&run.arg) else { continue };
            c.own(run.call);
            let failure = self.failure(run.call);
            let names = failure.and_then(|(_, field)| field).is_some_and(|field| names_argument(field, arg));
            let label = format!("{}: {} with {:?} ({})", run.command, arg.spelling(), run.probe.word, run.probe.why);
            match run.probe.expect {
                Expect::Accepted => {
                    let refused_for_it = self.at(run.call).exit == Exit::Code(2) && names;
                    c.holds_at(run.call, "inside-accepted", !refused_for_it, || format!("{label} is inside the domain and was refused: {}", refusal_text(self.envelope(run.call), self.at(run.call).exit)));
                }
                Expect::Refused => {
                    let refused = self.refused(run.call);
                    c.holds_at(run.call, "outside-refused", refused, || format!("{label} is outside the domain and was not refused: {}", refusal_text(self.envelope(run.call), self.at(run.call).exit)));
                    if refused {
                        c.holds_at(run.call, "refusal-names-argument", names, || {
                            format!("{label} was refused with field {}, expected it to name {}", failure.and_then(|(_, field)| field).map_or("missing".to_string(), |field| format!("{field:?}")), arg.spelling())
                        });
                    }
                }
            }
        }
        let mut unprovided: Vec<(String, String)> = Vec::new();
        for note in &l1.domain_notes {
            if note.needs_rejects && !note.has_rejects {
                unprovided.push((format!("commands.{}.domainRejects", note.command), format!("{}: the domain of {} has a pattern or a structural keyword and the fixture has no `domainRejects` for it", note.command, note.arg)));
            }
            if note.no_base {
                unprovided.push((format!("commands.{}.ok", note.command), format!("{}: no `ok` invocation of the fixture can carry a changed {} and still satisfy the command's requirements and constraints", note.command, note.arg)));
            }
            c.notes.extend(note.skipped.iter().map(|why| format!("{}: {}: {why}: not probed", note.command, note.arg)));
        }
        if !unprovided.is_empty() {
            let mut entries: Vec<String> = unprovided.iter().map(|(entry, _)| entry.clone()).collect();
            entries.dedup();
            return c.finish_as(End::missing_all(unprovided.into_iter().map(|(_, why)| why).collect(), entries));
        }
        if l1.domain_notes.is_empty() {
            return c.finish_as(End::Unsupported("no argument declares a domain".to_string()));
        }
        c.finish(true)
    }

    fn l1_patterns(&'a self) -> Finding {
        let mut c = self.check("L1.patterns");
        if self.docs().tree.is_none() {
            return self.no_tree(c);
        }
        let mut judged = false;
        let mut unprovided: Vec<(String, String)> = Vec::new();
        for doc in self.docs().commands.values() {
            for (condition, problem) in documents::pattern_findings(doc) {
                judged = true;
                c.holds_in(self.doc_call(&doc.command), condition, problem.is_none(), || problem.clone().unwrap_or_default());
            }
            for arg in doc.args() {
                let Some(domain) = &arg.domain else { continue };
                let patterns: Vec<String> = documents::domain_patterns(doc)
                    .into_iter()
                    .filter(|(spelling, _)| *spelling == arg.spelling())
                    .filter_map(|(_, pattern)| pattern.ok())
                    .collect();
                let _ = domain;
                if patterns.is_empty() && documents::domain_patterns(doc).iter().all(|(spelling, _)| *spelling != arg.spelling()) {
                    continue;
                }
                // The values to compare: the fixture's rejects and the value each `ok` invocation gives the argument.
                let mut values: Vec<String> = self
                    .fixture
                    .commands
                    .as_ref()
                    .and_then(|commands| commands.get(&doc.command))
                    .and_then(|entry| entry.domain_rejects.get(&arg.name))
                    .cloned()
                    .unwrap_or_default();
                for argv in self.fixture.commands.as_ref().and_then(|commands| commands.get(&doc.command)).map(|entry| entry.ok.clone()).unwrap_or_default() {
                    if let Ok(parsed) = invocation::parse(doc, &super::in_json_mode(&argv)) {
                        if let Some(value) = super::l1_plan::given_value(doc, &parsed, arg) {
                            values.push(value);
                        }
                    }
                }
                // The values of the documented examples are accepted values too.
                for line in &doc.examples {
                    if let Some(parsed) = invocation::split_line(line).ok().and_then(|words| invocation::parse(doc, words.get(1..)?).ok()) {
                        if let Some(value) = super::l1_plan::given_value(doc, &parsed, arg) {
                            values.push(value);
                        }
                    }
                }
                for pattern in patterns {
                    judged = true;
                    if regex::form_problem(&pattern).is_some() || regex::rust_problem(&pattern).is_some() {
                        continue;
                    }
                    // Values are needed where the checker probes the command: a `read` one, or a write the fixture lists with an `ok` invocation.
                    let probed = doc.effect == "read"
                        || self.fixture.commands.as_ref().and_then(|commands| commands.get(&doc.command)).is_some_and(|entry| !entry.ok.is_empty());
                    if values.is_empty() && probed {
                        unprovided.push((format!("commands.{}.domainRejects", doc.command), format!("{}: no value of {} to compare the engines on for {pattern:?}", doc.command, arg.spelling())));
                    }
                    for value in &values {
                        let verdict = regex::compare(&pattern, value);
                        let problem = match &verdict {
                            Comparison::Differs { ecmascript, rust } => Some(format!(
                                "the engines disagree on {value:?}: ECMAScript {} it, Rust {} it",
                                if *ecmascript { "accepts" } else { "rejects" },
                                if *rust { "accepts" } else { "rejects" }
                            )),
                            Comparison::Uncompilable { ecmascript, rust } => Some(format!("one engine refuses the pattern (ECMAScript: {ecmascript:?}, Rust: {rust:?})")),
                            Comparison::Agree(_) | Comparison::UnicodeVersion { .. } => None,
                        };
                        c.holds_in(self.doc_call(&doc.command), "engines-agree", problem.is_none(), || format!("{}: the pattern {pattern:?} of {}: {}", doc.command, arg.spelling(), problem.clone().unwrap_or_default()));
                        if let Comparison::UnicodeVersion { difference, .. } = verdict {
                            c.notes.push(format!("{}: {pattern:?} on {value:?}: a verified Unicode-version difference: {}", doc.command, difference.describe()));
                        }
                    }
                }
            }
        }
        if !unprovided.is_empty() {
            let mut entries: Vec<String> = unprovided.iter().map(|(entry, _)| entry.clone()).collect();
            entries.dedup();
            return c.finish_as(End::missing_all(unprovided.into_iter().map(|(_, why)| why).collect(), entries));
        }
        c.finish(judged)
    }

    fn l1_numbers(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.numbers");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        for run in &l1.numbers {
            if self.gate(&mut c, &run.command) {
                continue;
            }
            c.own(run.call);
            c.holds_at(run.call, "beyond-safe-range-refused", self.refused(run.call), || {
                format!("{}: --{} {} is one beyond the safe integers and was not refused: {}", run.command, run.arg, run.word, refusal_text(self.envelope(run.call), self.at(run.call).exit))
            });
        }
        let in_documents = self.l2.is_some() && self.l2_numbers_in_documents(&mut c);
        c.finish(!l1.numbers.is_empty() || in_documents)
    }

    fn l1_validators(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.validators");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        for run in &l1.validators {
            if self.gate(&mut c, &run.command) {
                continue;
            }
            let Some(doc) = self.docs().command(&run.command) else { continue };
            let Some(validator) = doc.validators.get(run.validator) else { continue };
            c.own(run.call);
            let refused = self.refused(run.call);
            c.holds_at(run.call, "refused", refused, || format!("{}: the rejects `{}` was not refused: {}", run.command, run.line, refusal_text(self.envelope(run.call), self.at(run.call).exit)));
            if !refused {
                continue;
            }
            let (code, field) = self.failure(run.call).unwrap_or((None, None));
            c.holds_at(run.call, "declared-code", code == Some(validator.code.as_str()), || format!("{}: the rejects `{}` was refused with {}, the validator declares {}", run.command, run.line, code.unwrap_or("no code"), validator.code));
            let names = field.is_some_and(|field| validator.arguments.iter().filter_map(|name| doc.arg(name)).any(|arg| names_argument(field, arg)));
            c.holds_at(run.call, "field-is-an-argument", names, || format!("{}: the rejects `{}` was refused on {}, expected one of {}", run.command, run.line, field.unwrap_or("no field"), validator.arguments.join(", ")));
        }
        c.finish(!l1.validators.is_empty())
    }

    fn l1_output_data(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.output-data");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        let mut runs: Vec<usize> = Vec::new();
        for ((id, _), command) in self.fixture.commands.iter().flatten().zip(&self.calls.commands) {
            if !self.gate(&mut c, id) {
                runs.extend(command.ok.iter().copied());
            }
        }
        runs.extend(l1.examples.iter().map(|run| run.call));
        // The one run of each `notRun` command: its first document is judged against the output schema like any other.
        self.judge_served(&mut c, false);
        runs.extend(self.calls.served.iter().filter_map(|served| served.call));
        let mut judged = false;
        for index in runs {
            let Some(document) = self.ok_envelope(index) else { continue };
            let Some(doc) = document.pointer("/meta/command").and_then(Value::as_str).and_then(|command| self.docs().command(command)) else { continue };
            for (condition, problem) in documents::output_findings(doc, document) {
                judged = true;
                c.observe(index);
                c.holds_at(index, condition, problem.is_none(), || format!("{}: {}", self.label(index), problem.clone().unwrap_or_default()));
            }
        }
        c.finish(judged)
    }

    fn l1_next(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.next");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        let mut judged = false;
        for (index, document) in self.envelopes() {
            let Some(entries) = document.pointer("/meta/next").and_then(Value::as_array) else { continue };
            for entry in entries {
                let argv: Vec<String> = entry.get("argv").and_then(Value::as_array).map(|argv| argv.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                if argv.is_empty() {
                    continue;
                }
                judged = true;
                c.holds_at(index, "names-a-command", names_command(&l1.docs, &argv).is_some(), || format!("{} has a next entry {:?} that names no command of the tool", self.label(index), argv.join(" ")));
            }
        }
        if l1.nexts_dropped > 0 {
            c.notes.push(format!("{} next entries of read commands were not followed: the run follows at most 300", l1.nexts_dropped));
        }
        for run in &l1.nexts {
            c.own(run.call);
            let usage = self.failure(run.call).is_some_and(|(code, field)| self.is_usage(code, field));
            c.holds_at(run.call, "read-next-not-usage", !usage, || format!("{}: its next entry `{}` was refused as a usage problem: {}", self.label(run.source), run.argv.join(" "), refusal_text(self.envelope(run.call), self.at(run.call).exit)));
        }
        c.finish(judged)
    }

    /// A hint that starts with the tool name is a command line that parses against the `schema` document (spec 7.1.2): the command words, the
    /// option names and the grammar of 9.12. A value that stands for a `<PLACEHOLDER>` is not judged against a domain or a constraint.
    fn l1_hint_command(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.hint-command");
        if !self.claim.at_least(Claim::V1_1) {
            return c.finish_as(End::Unsupported("the claim is 1.0: a prose hint may start with the tool name (spec 7.1.2 binds 1.1 documents only)".to_string()));
        }
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        let tool = &l1.docs.tool;
        let mut judged = false;
        for (index, document) in self.envelopes() {
            let Some(hint) = document.pointer("/error/hint").and_then(Value::as_str) else { continue };
            if !hint.starts_with(&format!("{tool} ")) {
                continue;
            }
            let words = atip::split_argv(hint);
            let rest: Vec<String> = words.iter().skip(1).cloned().collect();
            if rest.is_empty() || rest.iter().all(|word| word == "--help" || word == "--version") {
                continue;
            }
            judged = true;
            c.observe(index);
            let problem = match names_command(&l1.docs, &rest) {
                None => Some("names no command of the tool".to_string()),
                Some(id) => match l1.docs.commands.get(&id) {
                    None => Some(format!("names {id}, a group, which is not run")),
                    Some(doc) => invocation::parse(doc, &rest).err().filter(|why| !why.is_empty()).or_else(|| {
                        // Too many operands is a grammar problem of the line; a missing value is not (a hint may leave a placeholder out).
                        invocation::parse(doc, &rest).ok().and_then(|parsed| {
                            let args = invocation::args(doc, &parsed);
                            invocation::requirement_problems(doc, &args).into_iter().find(|why| why.contains("too many"))
                        })
                    }),
                },
            };
            c.holds_at(index, "parses-as-a-command", problem.is_none(), || {
                format!("{}: the hint `{hint}` {}", self.label(index), problem.clone().unwrap_or_default())
            });
        }
        c.finish(judged)
    }

    fn l1_help_line(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.help-line");
        c.own(l1.root_help);
        let text = String::from_utf8_lossy(&self.at(l1.root_help).stdout).into_owned();
        let first = text.lines().next().unwrap_or_default();
        c.holds_at(l1.root_help, "mentions-schema", first.contains("schema"), || format!("the first line of `--help` is {:?}, it must point at `schema` (spec 4.7)", excerpt(first.as_bytes())));
        c.finish(true)
    }

    fn l1_defaults(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.defaults");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        let mut judged = false;
        for (id, index) in &l1.helps {
            let Some(doc) = self.docs().command(id) else { continue };
            let text = String::from_utf8_lossy(&self.at(*index).stdout).into_owned();
            for option in &doc.options {
                let Some(expected) = default_text(&option.default) else { continue };
                let Some(shown) = help_default(&text, &option.name) else { continue };
                judged = true;
                c.holds_at(*index, "help-agrees", shown == expected, || format!("`{id} --help` shows --{} with the default {shown:?}, the document says {expected:?}", option.name));
            }
        }
        c.finish(judged)
    }

    fn l1_deterministic(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.deterministic");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        c.holds_at(l1.tree, "tree-bytes", self.at(l1.tree).stdout == self.at(l1.tree_again).stdout, || "`schema --json` printed different bytes the second time".to_string());
        for call in &l1.doc_calls {
            c.holds_at(call.call, "doc-bytes", self.at(call.call).stdout == self.at(call.again).stdout, || format!("`schema {} --json` printed different bytes the second time", call.id));
        }
        c.finish(true)
    }

    fn l1_paging(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.paging");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        let offered = self.docs().commands.values().any(|doc| doc.has_option("cursor") && !self.fixture.not_run.contains(&doc.command));
        if !offered {
            return c.finish_as(End::Unsupported("no command document declares --cursor".to_string()));
        }
        let mut unprovided: Vec<(String, String)> = Vec::new();
        let mut proven: BTreeSet<String> = BTreeSet::new();
        for run in &l1.paging {
            if self.gate(&mut c, &run.command) {
                continue;
            }
            c.own(run.first);
            let returned = self.ok_envelope(run.first).and_then(|document| document.pointer("/meta/page/returned")).and_then(Value::as_u64);
            c.holds_at(run.first, "returned-at-most-limit", !run.limited || returned.is_some_and(|returned| returned <= 1), || {
                format!("{} has meta.page.returned {}, at most 1 for --limit 1", self.label(run.first), returned.map_or("missing".to_string(), |returned| returned.to_string()))
            });
            if let Some(next) = run.next {
                c.own(next);
                let first = self.ok_envelope(run.first).and_then(|document| document.get("data"));
                let second = self.ok_envelope(next).and_then(|document| document.get("data"));
                c.holds_at(next, "next-page-follows", second.is_some() && second != first, || {
                    format!("{} {}", self.label(next), if second.is_none() { refusal_text(self.envelope(next), self.at(next).exit) } else { "returned the first page again".to_string() })
                });
            }
            if run.next.is_some() {
                let mut good = false;
                let mut bad: Vec<(Option<usize>, String)> = Vec::new();
                for (label, call) in &run.changed {
                    c.own(*call);
                    let outcome = self.failure(*call);
                    match outcome {
                        None if self.at(*call).exit == Exit::Code(0) => bad.push((Some(*call), format!("{}: the cursor was accepted with {label}", run.command))),
                        Some((Some("CURSOR_INVALID"), Some("--cursor"))) => good = true,
                        Some((code, Some("--cursor"))) => bad.push((Some(*call), format!("{}: the cursor with {label} was refused with {} on --cursor, expected CURSOR_INVALID", run.command, code.unwrap_or("no code")))),
                        _ => {}
                    }
                }
                if !bad.is_empty() {
                    c.holds_each_at("changed-filter-refused", bad);
                } else if good {
                    c.holds_in(None, "changed-filter-refused", true, String::new);
                    proven.insert(run.command.clone());
                } else {
                    unprovided.push((run.command.clone(), format!("{}: no changed argument was refused as a reused cursor", run.command)));
                }
            }
            if self.claim.at_least(Claim::V1_2) {
                self.judge_paging_1_2(&mut c, run);
            }
            if let Some(why) = &run.why_not {
                unprovided.push((run.command.clone(), format!("{}: {why}", run.command)));
            }
        }
        for (command, why) in &l1.paging_gaps {
            unprovided.push((command.clone(), format!("{command}: {why}")));
        }
        // A command that one of its invocations proved is not left unprovided by another.
        let missing: Vec<(String, String)> = unprovided.into_iter().filter(|(command, _)| !proven.contains(command)).collect();
        if !missing.is_empty() {
            let mut entries: Vec<String> = missing.iter().map(|(command, _)| format!("commands.{command}.paged")).collect();
            entries.dedup();
            return c.finish_as(End::missing_all(missing.into_iter().map(|(_, why)| why).collect(), entries));
        }
        c.finish(true)
    }

    /// The 1.2 half of `L1.paging` for one paged command (spec 6.9, 10.7.2, 12.5): the cursor is bound to what the invocation gave.
    fn judge_paging_1_2(&'a self, c: &mut Conditions<'a>, run: &super::l1_plan::PagingRun) {
        let refused_as_cursor = |call: usize| self.failure(call).is_some_and(|(code, _)| code == Some("CURSOR_INVALID"));
        if let Some(call) = run.all_swap {
            c.own(call);
            c.holds_at(call, "cursor-survives-all", !refused_as_cursor(call), || {
                format!("{}: the cursor of a page asked with --limit 1 was refused with CURSOR_INVALID when --all replaced --limit ({})", run.command, self.label(call))
            });
        }
        for (label, call) in &run.defaults {
            c.own(*call);
            c.holds_at(*call, "cursor-bound-to-given-arguments", self.failure(*call).is_some_and(|(code, field)| code == Some("CURSOR_INVALID") && field == Some("--cursor")), || {
                format!("{}: the cursor was not refused with CURSOR_INVALID on --cursor with {label}: {}", run.command, refusal_text(self.envelope(*call), self.at(*call).exit))
            });
        }
        for (label, call) in &run.envs {
            c.own(*call);
            c.holds_at(*call, "cursor-bound-to-environment", self.failure(*call).is_some_and(|(code, field)| code == Some("CURSOR_INVALID") && field == Some("--cursor")), || {
                format!("{}: the cursor was not refused with CURSOR_INVALID on --cursor with {label}: {}", run.command, refusal_text(self.envelope(*call), self.at(*call).exit))
            });
        }
        if let Some(input) = &run.input {
            c.own(input.first);
            let first = self.ok_envelope(input.first).and_then(|document| document.get("data"));
            c.holds_at(input.first, "paged-through-input", first.is_some(), || {
                format!("{}: the paged invocation with its arguments in a document on stdin failed: {}", run.command, refusal_text(self.envelope(input.first), self.at(input.first).exit))
            });
            if let (Some(next_argv), Some(doc)) = (&input.next_argv, self.docs().command(&run.command)) {
                let mut problems: Vec<String> = Vec::new();
                if next_argv.iter().any(|word| word == "--input" || word.starts_with("--input=")) {
                    problems.push("it carries --input".to_string());
                }
                if let Ok(carried) = crate::l1::invocation::parse(doc, next_argv) {
                    for item in &carried.items {
                        let crate::l1::invocation::Item::Option { name, value } = item else { continue };
                        let Some(option) = doc.option(name) else { continue };
                        if input.gave.contains(name) || matches!(name.as_str(), "cursor" | "json") {
                            continue;
                        }
                        if value.as_ref().is_some_and(|value| !option.default.is_null() && *value == crate::l1::witness::word_of(&option.default)) {
                            problems.push(format!("it writes out the default of --{name}"));
                        } else if option.env.is_some() {
                            problems.push(format!("it carries --{name}, which the invocation did not give and the environment gave"));
                        }
                    }
                }
                // The next entry is run in the same environment; it has to go on from the page that the document gave.
                if let Some(next) = input.next {
                    c.own(next);
                    let second = self.ok_envelope(next).and_then(|document| document.get("data"));
                    if second.is_none() {
                        problems.push(format!("`{}` {}", self.at(next).call.argv.join(" "), refusal_text(self.envelope(next), self.at(next).exit)));
                    } else if second == first {
                        problems.push("it returns the first page again".to_string());
                    }
                }
                c.holds_at(input.first, "next-after-input", problems.is_empty(), || {
                    format!("{}: the next entry of the page given by a document on stdin (`{}`): {}", run.command, next_argv.join(" "), problems.join("; "))
                });
            }
        }
        if !run.unsupported.is_empty() {
            c.notes.push(format!("{}: the 1.2 half of the paging check has nothing to test for: {}", run.command, run.unsupported.join(", ")));
        }
    }

    fn l1_fields(&'a self) -> Finding {
        let l1 = self.l1();
        let mut c = self.check("L1.fields");
        if l1.docs.tree.is_none() {
            return self.no_tree(c);
        }
        let offered = self.docs().commands.values().any(|doc| doc.has_option("fields") && !self.fixture.not_run.contains(&doc.command));
        if !offered {
            return c.finish_as(End::Unsupported("no command offers --fields".to_string()));
        }
        let mut unprovided: Vec<(String, String)> = Vec::new();
        for run in &l1.fields {
            if self.gate(&mut c, &run.command) {
                continue;
            }
            let Some(doc) = self.docs().command(&run.command) else { continue };
            c.own(run.projected);
            let (Some(plain), Some(projected)) = (self.ok_envelope(run.plain), self.ok_envelope(run.projected)) else {
                c.holds_at(run.projected, "meta-fields", false, || format!("{} {}", self.label(run.projected), refusal_text(self.envelope(run.projected), self.at(run.projected).exit)));
                continue;
            };
            let name = projected.pointer("/meta/output").and_then(Value::as_str);
            let Some(output) = output_with_fields(doc, name).or_else(|| doc.outputs.iter().find(|output| output.fields.is_some())) else { continue };
            let Some(fields) = &output.fields else { continue };
            let mut kept: Vec<String> = run.selection.clone();
            kept.extend(fields.keep.iter().cloned());
            kept.sort();
            kept.dedup();
            let mut stated: Vec<String> = projected.pointer("/meta/fields").and_then(Value::as_array).map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
            stated.sort();
            c.holds_at(run.projected, "meta-fields", stated == kept, || format!("{} has meta.fields {stated:?}, the selection and `keep` make {kept:?}", self.label(run.projected)));
            for (condition, problem) in documents::fields_findings(doc, projected, &kept, Some(plain)) {
                // A selection the tool refused as USAGE is `unknown-path-refused`'s; a document that names an undeclared path is judged from its report alone.
                if condition == "selection-declared" {
                    continue;
                }
                c.holds_at(run.projected, condition, problem.is_none(), || format!("{}: {}", self.label(run.projected), problem.clone().unwrap_or_default()));
            }
            c.observe(run.projected);
            // Since 1.2 `--fields` alone is no reason to refuse (spec 12.5): the first selection with no `--json` is not refused; the text is not compared.
            if let Some(text) = run.text {
                c.own(text);
                let outcome = self.at(text);
                c.holds_at(text, "fields-without-json", outcome.exit == Exit::Code(0) && !outcome.stdout.is_empty(), || {
                    format!("{}: `{}` ended with {} and printed {}: --fields without --json is not a reason to refuse", run.command, outcome.call.argv.join(" "), exit_text(outcome.exit), if outcome.stdout.is_empty() { "nothing" } else { "its answer" })
                });
            }
        }
        for run in &l1.unknown_fields {
            if self.gate(&mut c, &run.command) {
                continue;
            }
            c.own(run.call);
            let failure = self.failure(run.call);
            c.holds_at(run.call, "unknown-path-refused", failure.is_some_and(|(code, field)| self.is_usage(code, field) && field == Some("--fields")), || {
                format!("{}: {} {}, expected USAGE on --fields", run.command, self.label(run.call), refusal_text(self.envelope(run.call), self.at(run.call).exit))
            });
            // Since 1.2 an unknown path is a USAGE refusal with no `--json` too: exit 2, an error on stderr and nothing on stdout.
            if let Some(text) = run.text {
                c.own(text);
                let outcome = self.at(text);
                c.holds_at(text, "unknown-path-refused-in-text", outcome.exit == Exit::Code(2) && outcome.stdout.is_empty() && !outcome.stderr.is_empty(), || {
                    format!("{}: `{}` ended with {} (stdout {:?}, stderr {:?}), expected exit 2 with the error on stderr", run.command, outcome.call.argv.join(" "), exit_text(outcome.exit), excerpt(&outcome.stdout), excerpt(&outcome.stderr))
                });
            }
        }
        // The selections must reach a nested path and an `[]` path when the output declares one.
        for doc in self.docs().commands.values() {
            let selections: Vec<&Vec<String>> = l1.fields.iter().filter(|run| run.command == doc.command).map(|run| &run.selection).collect();
            if selections.is_empty() {
                continue;
            }
            let Some(output) = doc.outputs.iter().find(|output| output.fields.is_some()) else { continue };
            let Some(fields) = &output.fields else { continue };
            let steps = |path: &String| project::parse_field_path(path).unwrap_or_default();
            let declared: Vec<&String> = fields.paths.iter().chain(fields.keep.iter()).collect();
            let nested = declared.iter().any(|path| steps(path).len() > 1);
            let array = declared.iter().any(|path| steps(path).iter().any(|step| step.array));
            let chosen = selections.iter().flat_map(|selection| selection.iter());
            if nested && !chosen.clone().any(|path| steps(path).len() > 1) {
                unprovided.push((doc.command.clone(), format!("{}: the output declares a nested path and no selection of the fixture goes through one", doc.command)));
            }
            if array && !chosen.clone().any(|path| steps(path).iter().any(|step| step.array)) {
                unprovided.push((doc.command.clone(), format!("{}: the output declares an `[]` path and no selection of the fixture goes through one", doc.command)));
            }
        }
        if !unprovided.is_empty() {
            let mut entries: Vec<String> = unprovided.iter().map(|(command, _)| format!("commands.{command}.fields")).collect();
            entries.dedup();
            return c.finish_as(End::missing_all(unprovided.into_iter().map(|(_, why)| why).collect(), entries));
        }
        if l1.fields.is_empty() {
            let entries: Vec<String> = self.docs().commands.values().filter(|doc| doc.has_option("fields") && !self.fixture.not_run.contains(&doc.command)).map(|doc| format!("commands.{}.fields", doc.command)).collect();
            return c.finish_as(End::Unprovided { reason: "the fixture lists no `fields` selection for a command that offers --fields".to_string(), cut: false, entries });
        }
        c.finish(true)
    }
}

/// A default as the text `--help` shows it: a string as it is, a number or boolean as written; `None` for no default or a list.
fn default_text(default: &Value) -> Option<String> {
    match default {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// The first long name of an option line: the line opens, behind blanks, with an option (`--name`, or short options `-l, --name`, each with a value `<V>` or `[V]`
/// of its own), and the first `--name` is the option the line describes. A usage line, a sentence, a bullet (`- see --limit`) and a description that names an
/// option open no option.
fn option_of_line(line: &str) -> Option<&str> {
    let mut rest = line.trim_start();
    loop {
        if let Some(long) = rest.strip_prefix("--") {
            let end = long.find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-')).unwrap_or(long.len());
            return (end > 0).then(|| &long[..end]);
        }
        let short = rest.strip_prefix('-')?;
        let mut chars = short.chars();
        if !chars.next().is_some_and(|ch| ch.is_ascii_alphanumeric()) {
            return None;
        }
        rest = chars.as_str().trim_start_matches([' ', '\t']);
        for (open, close) in [('<', '>'), ('[', ']')] {
            if rest.starts_with(open) {
                rest = rest.find(close).map_or("", |at| &rest[at + 1..]);
                break;
            }
        }
        // Only a comma goes on to the long name.
        rest = rest.trim_start().strip_prefix(',')?.trim_start();
    }
}

/// The default `text` shows for `--option` in a recognised form (`[default: X]`, `(default X)`), when it shows one: in the block of the line that opens with `--option` as its first long name.
fn help_default(text: &str, option: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|line| option_of_line(line) == Some(option))?;
    let mut block = String::from(lines[start]);
    for line in &lines[start + 1..] {
        if option_of_line(line).is_some() || line.trim().is_empty() {
            break;
        }
        block.push(' ');
        block.push_str(line.trim());
    }
    for (open, close) in [("[default: ", "]"), ("(default ", ")"), ("(default: ", ")")] {
        if let Some(at) = block.find(open) {
            let rest = &block[at + open.len()..];
            let end = rest.find(close)?;
            return Some(rest[..end].trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{help_default, ids, TABLE};
    use super::documents::is_kebab;
    use crate::report::Strength;

    #[test]
    fn atip_checkl1_table_is_thirty_one_checks_twenty_five_must_and_six_should() {
        assert_eq!(ids().len(), 31);
        assert_eq!(TABLE.iter().filter(|entry| entry.strength == Strength::Must).count(), 25);
        assert!(ids().iter().all(|id| id.starts_with("L1.")));
        let mut unique = ids().to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 31);
    }

    #[test]
    fn atip_checkl1_words_and_names() {
        assert!(is_kebab("plan-digest") && is_kebab("a1") && !is_kebab("Plan") && !is_kebab("a--b") && !is_kebab("a_b"));
       
    }

    #[test]
    fn atip_checkl1_help_defaults_are_read_in_the_two_recognised_forms() {
        let text = "Options:\n      --limit <N>   How many [default: 20]\n      --sort <KEY>  Order\n                    (default newest)\n  -h, --help\n";
        assert_eq!(help_default(text, "limit").as_deref(), Some("20"));
        assert_eq!(help_default(text, "sort").as_deref(), Some("newest"));
        assert_eq!(help_default(text, "other"), None);
    }

    #[test]
    fn atip_check_87_a_short_option_before_the_long_one_still_names_the_line_and_a_line_without_a_first_long_option_starts_no_block() {
        let text = concat!(
            "Usage: tool list [OPTIONS]\n",
            "\n",
            "Return members; --limit asks for fewer [default: 1]\n",
            "\n",
            "  - see --limit (the usual [default: 99])\n",
            "\n",
            "Options:\n",
            "  -l, --limit <N>      How many, like --json does [default: 20]\n",
            "  -o, --order <ORDER>  Sorted, see --limit [default: name]\n",
            "      --max=<N>        The most (default 7)\n",
            "      --json           Print JSON\n",
            "  -h, --help\n",
        );
        // The block of an option is the line that opens with its first long name, behind blanks and a short option.
        assert_eq!(help_default(text, "limit").as_deref(), Some("20"));
        assert_eq!(help_default(text, "order").as_deref(), Some("name"));
        assert_eq!(help_default(text, "max").as_deref(), Some("7"));
        // The usage line, the sentence and the bullet open no block, and a name in a description is no option line.
        assert_eq!(help_default(text, "json"), None);
        assert_eq!(help_default(text, "help"), None);
        assert_eq!(help_default("Usage: tool --limit <N> [default: 5]\n", "limit"), None);
        assert_eq!(help_default("  - --limit <N> [default: 5]\n", "limit"), None);
        // A longer name is another option.
        assert_eq!(help_default("  --limit-all <N>  all [default: 1]\n", "limit"), None);
    }
}
