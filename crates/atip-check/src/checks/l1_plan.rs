//! The invocations of the L1 checks: what a run asks of the binary after the L0 ones (approaches/checker.md 6, L1).
//!
//! The run reads `schema --json`, then every document the tree names, and from those documents plans the rest: the
//! examples of the `read` commands, one invocation per rule group of each constraint set, the values at the edges of each
//! domain, the numbers beyond the safe range, the validators' `rejects`, the `next` entries of every answer, the paged
//! commands with `--limit 1` and the `--fields` selections of the fixture. Nothing is run that the fixture does not list
//! when it writes: `read` commands and the writes the fixture lists are the only ones a probe is made of.

use std::collections::BTreeSet;

use serde_json::Value;

use super::{first_document, in_json_mode, Run};
use crate::claim::Claim;
use crate::exec::{Call, RunError};
use crate::l1::invocation::{self, Invocation, Item};
use crate::l1::model::{Arg, CommandDoc, GroupDoc, Output, ToolDocs, Tree};
use crate::l1::witness::{self, DomainProbe, Expect, GroupOutcome, GroupPlan};

/// The word no tool has as a path word, for `schema nosuch-xyz`.
pub(crate) const NO_SUCH_PATH: &str = "nosuch-xyz";
/// The code no registry has, for `schema --error` and `schema --warning`.
pub(crate) const NO_SUCH_CODE: &str = "NOSUCH_XYZ";
/// The path no field selection of a tool declares.
pub(crate) const NO_SUCH_FIELD: &str = "nosuch-xyz";
/// The most `next` entries one run follows.
const MAX_NEXT_RUNS: usize = 300;

/// One document the tree names, and the invocations that read it.
#[derive(Debug, Clone)]
pub(crate) struct DocCall {
    pub id: String,
    pub is_group: bool,
    /// `schema <words> --json`.
    pub call: usize,
    /// The same again, for `L1.deterministic`.
    pub again: usize,
}

/// One example of a `read` command, run.
#[derive(Debug, Clone)]
pub(crate) struct ExampleRun {
    pub command: String,
    pub line: String,
    pub call: usize,
}

/// One rule group of a command and, when it has a witness, the invocation made for it.
#[derive(Debug, Clone)]
pub(crate) struct ConstraintRun {
    pub command: String,
    pub group: GroupPlan,
    pub call: Option<usize>,
    /// The other groups of the command, so an entailed group can name the rules that break with it.
    pub labels: Vec<String>,
}

/// One value tried for one argument.
#[derive(Debug, Clone)]
pub(crate) struct DomainRun {
    pub command: String,
    pub arg: String,
    pub probe: DomainProbe,
    pub call: usize,
}

/// What a domain needs and whether the run could probe it.
#[derive(Debug, Clone)]
pub(crate) struct DomainNote {
    pub command: String,
    pub arg: String,
    /// The domain has a `pattern` or a structural keyword the checker cannot generate a value outside of.
    pub needs_rejects: bool,
    pub has_rejects: bool,
    pub skipped: Vec<String>,
    /// No invocation of the fixture could carry the changed argument.
    pub no_base: bool,
}

/// One integer argument given a number one beyond the safe range.
#[derive(Debug, Clone)]
pub(crate) struct NumberRun {
    pub command: String,
    pub arg: String,
    pub word: String,
    pub call: usize,
}

/// One `rejects` invocation of a validator.
#[derive(Debug, Clone)]
pub(crate) struct ValidatorRun {
    pub command: String,
    pub validator: usize,
    pub line: String,
    pub call: usize,
}

/// One `next` entry followed: the answer that carried it, and the run of it in the same sequence.
#[derive(Debug, Clone)]
pub(crate) struct NextRun {
    pub source: usize,
    pub argv: Vec<String>,
    pub call: usize,
}

/// A paged command called with `--limit 1`, the next page, and the cursor reused with another argument.
#[derive(Debug, Clone)]
pub(crate) struct PagingRun {
    pub command: String,
    /// The invocation asked for one item (`--limit 1`); a command with no `--limit` is paged as the fixture or the run gave it.
    pub limited: bool,
    pub first: usize,
    pub next: Option<usize>,
    pub changed: Vec<(String, usize)>,
    /// Why the run could not go on: the first page has no next page, or no argument could be changed.
    pub why_not: Option<String>,
}

/// One `--fields` selection: the answer without it, with it, and with a path nothing declares.
#[derive(Debug, Clone)]
pub(crate) struct FieldsRun {
    pub command: String,
    pub selection: Vec<String>,
    pub plain: usize,
    pub projected: usize,
}

/// The refusal of a path no selection declares, once per command that offers `--fields`.
#[derive(Debug, Clone)]
pub(crate) struct UnknownFieldRun {
    pub command: String,
    pub call: usize,
}

/// Everything the L1 judgments read besides the log.
#[derive(Debug, Default)]
pub(crate) struct L1Run {
    pub docs: ToolDocs,
    pub tree: usize,
    pub tree_again: usize,
    pub empty: usize,
    pub doc_calls: Vec<DocCall>,
    /// The two-word command read as one quoted word, the split call it must equal, and the quoted call.
    pub quoted: Option<(String, usize, usize)>,
    pub unknown_path: usize,
    pub overlap: usize,
    pub group_errors: Option<usize>,
    pub unknown_error: usize,
    pub unknown_warning: Option<usize>,
    /// `<group> --json` for each group.
    pub groups: Vec<(String, usize)>,
    pub root_help: usize,
    /// `<path> --help` for each command with an option that has a default.
    pub helps: Vec<(String, usize)>,
    pub examples: Vec<ExampleRun>,
    pub constraints: Vec<ConstraintRun>,
    pub domains: Vec<DomainRun>,
    pub domain_notes: Vec<DomainNote>,
    pub numbers: Vec<NumberRun>,
    pub validators: Vec<ValidatorRun>,
    pub nexts: Vec<NextRun>,
    pub paging: Vec<PagingRun>,
    /// Paging commands (their documents declare `--cursor`) the run could not page: the command and why.
    pub paging_gaps: Vec<(String, String)>,
    pub fields: Vec<FieldsRun>,
    pub unknown_fields: Vec<UnknownFieldRun>,
}

/// `argv` as an envelope's `data` when it printed an ok envelope.
fn ok_data(stdout: &[u8]) -> Option<Value> {
    let document = first_document(stdout)?;
    (document.get("ok") == Some(&Value::Bool(true)))
        .then(|| document.get("data").cloned())
        .flatten()
}

impl Run<'_> {
    /// The fixture's `ok` invocations of `id`, as argv words in JSON mode, and a parse of each against `doc`.
    fn bases(&self, doc: &CommandDoc, tool: &str) -> Vec<Invocation> {
        let listed: Vec<Vec<String>> = self
            .fixture
            .commands
            .as_ref()
            .and_then(|commands| commands.get(&doc.command))
            .map(|entry| entry.ok.iter().map(|argv| in_json_mode(argv)).collect())
            .unwrap_or_default();
        let from_examples = || -> Vec<Vec<String>> {
            if doc.effect != "read" {
                return Vec::new();
            }
            doc.examples
                .iter()
                .filter_map(|line| invocation::split_line(line).ok())
                .filter(|words| words.first().is_some_and(|word| word == tool))
                .map(|words| in_json_mode(&words[1..]))
                .collect()
        };
        let chosen = if listed.is_empty() { from_examples() } else { listed };
        chosen
            .iter()
            .filter_map(|words| invocation::parse(doc, words).ok())
            .collect()
    }

    /// Whether the checker may run probes of `doc`: a `read` command, or a command the fixture lists with an `ok` invocation.
    fn runnable(&self, doc: &CommandDoc) -> bool {
        doc.effect == "read"
            || self
                .fixture
                .commands
                .as_ref()
                .and_then(|commands| commands.get(&doc.command))
                .is_some_and(|entry| !entry.ok.is_empty())
    }

    pub(crate) fn invoke_l1(&mut self, warnings_offered: bool, claim: Claim) -> Result<L1Run, RunError> {
        let _ = claim;
        let mut run = L1Run {
            tree: self.invoke(Call::new(["schema", "--json"]).measured())?,
            ..L1Run::default()
        };
        run.tree_again = self.invoke(Call::new(["schema", "--json"]))?;
        run.empty = self.invoke(Call::new(["schema", "--json"]).in_empty_state())?;
        let envelope = first_document(&self.log[run.tree].stdout).filter(|document| document.get("ok") == Some(&Value::Bool(true)));
        run.docs.tool = envelope
            .as_ref()
            .and_then(|document| document.pointer("/meta/tool"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        run.docs.tree = envelope
            .as_ref()
            .and_then(|document| document.get("data"))
            .and_then(Tree::parse);
        let entries = run.docs.tree.as_ref().map(|tree| tree.commands.clone()).unwrap_or_default();
        // The invocations that are the same whatever the fixture says.
        run.root_help = self.invoke(Call::new(["--help"]))?;
        run.unknown_path = self.invoke(Call::new(["schema", NO_SUCH_PATH, "--json"]))?;
        run.overlap = self.invoke(Call::new(["schema", "--errors", "--error", "USAGE", "--json"]))?;
        run.unknown_error = self.invoke(Call::new(["schema", "--error", NO_SUCH_CODE, "--json"]))?;
        if warnings_offered {
            run.unknown_warning = Some(self.invoke(Call::new(["schema", "--warning", NO_SUCH_CODE, "--json"]))?);
        }
        for entry in &entries {
            let words = entry.path.clone();
            let call = |extra: &[&str]| {
                Call::new(
                    ["schema"]
                        .into_iter()
                        .map(str::to_string)
                        .chain(words.iter().cloned())
                        .chain(extra.iter().map(|word| (*word).to_string())),
                )
            };
            let index = self.invoke(call(&["--json"]).measured())?;
            let again = self.invoke(call(&["--json"]))?;
            let data = ok_data(&self.log[index].stdout);
            if entry.is_group() {
                if let Some(group) = data.as_ref().and_then(GroupDoc::parse) {
                    run.docs.groups.insert(entry.command.clone(), group);
                }
                let bare = self.invoke(Call::new(words.iter().cloned().chain(["--json".to_string()])))?;
                run.groups.push((entry.command.clone(), bare));
            } else if let Some(doc) = data.as_ref().and_then(CommandDoc::parse) {
                run.docs.commands.insert(entry.command.clone(), doc);
            }
            run.doc_calls.push(DocCall {
                id: entry.command.clone(),
                is_group: entry.is_group(),
                call: index,
                again,
            });
        }
        if let Some((entry, split)) = entries
            .iter()
            .filter(|entry| !entry.is_group() && entry.path.len() == 2)
            .find_map(|entry| run.doc_calls.iter().find(|call| call.id == entry.command).map(|call| (entry, call.call)))
        {
            let quoted = self.invoke(Call::new(["schema".to_string(), entry.command.clone(), "--json".to_string()]))?;
            run.quoted = Some((entry.command.clone(), split, quoted));
        }
        if let Some(group) = entries.iter().find(|entry| entry.is_group()) {
            let words = ["schema".to_string()].into_iter().chain(group.path.iter().cloned()).chain(["--errors".to_string(), "--json".to_string()]);
            run.group_errors = Some(self.invoke(Call::new(words))?);
        }
        let tool = run.docs.tool.clone();
        // The plans are keyed by the id the tree gives a command, whatever `command` the document says of itself (`L1.walk` judges that).
        let docs: Vec<CommandDoc> = run
            .docs
            .commands
            .iter()
            .map(|(id, doc)| CommandDoc { command: id.clone(), ..doc.clone() })
            .collect();
        // `--help` of the commands that have a default to compare with.
        for doc in &docs {
            if doc.options.iter().any(|option| !option.default.is_null() && !matches!(option.name.as_str(), "json")) {
                let index = self.invoke(Call::new(doc.path.iter().cloned().chain(["--help".to_string()])))?;
                run.helps.push((doc.command.clone(), index));
            }
        }
        // Every example of a `read` command, in the fixture state.
        for doc in docs.iter().filter(|doc| doc.effect == "read") {
            for line in &doc.examples {
                let Some(words) = invocation::split_line(line).ok().filter(|words| words.first().is_some_and(|word| *word == tool)) else {
                    continue;
                };
                let call = self.invoke(Call::new(in_json_mode(&words[1..])))?;
                run.examples.push(ExampleRun { command: doc.command.clone(), line: line.clone(), call });
            }
        }
        for doc in &docs {
            if !self.runnable(doc) {
                continue;
            }
            let bases = self.bases(doc, &tool);
            self.plan_constraints(&mut run, doc, &bases)?;
            self.plan_domains(&mut run, doc, &bases)?;
            self.plan_validators(&mut run, doc, &tool)?;
        }
        for doc in docs.iter().filter(|doc| doc.effect == "read") {
            // A read command's validators are run whether or not the fixture lists it.
            if !self.runnable(doc) {
                continue;
            }
        }
        for doc in &docs {
            if self.runnable(doc) {
                let bases = self.bases(doc, &tool);
                self.plan_paging(&mut run, doc, &bases)?;
                self.plan_fields(&mut run, doc, &bases)?;
            }
        }
        self.plan_nexts(&mut run)?;
        Ok(run)
    }

    fn plan_constraints(&mut self, run: &mut L1Run, doc: &CommandDoc, bases: &[Invocation]) -> Result<(), RunError> {
        if doc.constraints.is_empty() {
            return Ok(());
        }
        let plans = match bases.first() {
            Some(base) => witness::plan_constraints(doc, base),
            None => doc
                .constraints
                .iter()
                .enumerate()
                .map(|(index, constraint)| GroupPlan {
                    members: vec![index],
                    codes: BTreeSet::from([constraint.code.clone().unwrap_or_else(|| "USAGE".to_string())]),
                    outcome: GroupOutcome::Unprovided("the fixture has no `ok` invocation of the command to build the invocation from".to_string()),
                })
                .collect(),
        };
        let labels: Vec<String> = plans
            .iter()
            .map(|plan| {
                plan.members
                    .iter()
                    .filter_map(|index| doc.constraints.get(*index))
                    .map(crate::l1::model::Constraint::label)
                    .collect::<Vec<_>>()
                    .join(" = ")
            })
            .collect();
        for plan in plans {
            let call = match &plan.outcome {
                GroupOutcome::Witness { words } => Some(self.invoke(Call::new(words.iter().cloned()))?),
                _ => None,
            };
            run.constraints.push(ConstraintRun {
                command: doc.command.clone(),
                group: plan,
                call,
                labels: labels.clone(),
            });
        }
        Ok(())
    }

    /// An invocation like one of `bases` with `arg` given as `word`, and the requirements and constraints of the command still satisfied, so that a refusal of the value is the value's: the
    /// base as it is, else with the options that the broken rules name left out or given another value of their enum (at most two changes).
    fn carrying(doc: &CommandDoc, bases: &[Invocation], arg: &Arg, word: &str) -> Option<Invocation> {
        let satisfied = |invocation: &Invocation| {
            let args = invocation::args(doc, invocation);
            invocation::requirement_problems(doc, &args).is_empty() && invocation::violated_constraints(doc, &args).is_empty()
        };
        let neighbours = |invocation: &Invocation| -> Vec<Invocation> {
            let args = invocation::args(doc, invocation);
            let mut names: Vec<&str> = Vec::new();
            for index in invocation::violated_constraints(doc, &args) {
                let Some(constraint) = doc.constraints.get(index) else { continue };
                for name in constraint.names.iter().chain(constraint.if_name.iter()).chain(constraint.then.iter()) {
                    if name != &arg.name && !names.contains(&name.as_str()) {
                        names.push(name);
                    }
                }
            }
            let mut next: Vec<Invocation> = Vec::new();
            for name in names {
                let Some(other) = doc.option(name) else { continue };
                let mut removed = invocation.clone();
                invocation::remove_option(&mut removed, name);
                next.push(removed);
                for value in &other.possible_values {
                    let mut given = invocation.clone();
                    invocation::set_option(&mut given, name, Some(value.clone()));
                    next.push(given);
                }
            }
            next
        };
        bases.iter().find_map(|base| {
            let changed = witness::with_argument(doc, base, arg, word)?;
            if satisfied(&changed) {
                return Some(changed);
            }
            let first = neighbours(&changed);
            if let Some(found) = first.iter().find(|candidate| satisfied(candidate)) {
                return Some(found.clone());
            }
            first.iter().flat_map(&neighbours).find(|candidate| satisfied(candidate))
        })
    }

    fn plan_domains(&mut self, run: &mut L1Run, doc: &CommandDoc, bases: &[Invocation]) -> Result<(), RunError> {
        for arg in doc.args().filter(|arg| arg.domain.is_some() && crate::l1::rules::domain_profile_problems(arg).is_empty()) {
            let plan = witness::domain_plan(arg);
            let rejects: Vec<String> = self
                .fixture
                .commands
                .as_ref()
                .and_then(|commands| commands.get(&doc.command))
                .and_then(|entry| entry.domain_rejects.get(&arg.name))
                .cloned()
                .unwrap_or_default();
            let mut note = DomainNote {
                command: doc.command.clone(),
                arg: arg.name.clone(),
                needs_rejects: plan.needs_rejects,
                has_rejects: !rejects.is_empty(),
                skipped: plan.skipped.clone(),
                no_base: false,
            };
            let mut probes = plan.probes;
            probes.extend(rejects.iter().map(|word| DomainProbe {
                word: word.clone(),
                expect: Expect::Refused,
                why: "a value outside the domain, from the fixture's domainRejects".to_string(),
            }));
            for probe in probes {
                let Some(changed) = Self::carrying(doc, bases, arg, &probe.word) else {
                    note.no_base = true;
                    continue;
                };
                let call = self.invoke(Call::new(invocation::render(&changed)))?;
                run.domains.push(DomainRun { command: doc.command.clone(), arg: arg.name.clone(), probe, call });
            }
            run.domain_notes.push(note);
            if arg.value_type == "integer" {
                for word in ["9007199254740992", "-9007199254740992"] {
                    let Some(changed) = Self::carrying(doc, bases, arg, word).or_else(|| {
                        bases.iter().find_map(|base| witness::with_argument(doc, base, arg, word))
                    }) else {
                        continue;
                    };
                    let call = self.invoke(Call::new(invocation::render(&changed)))?;
                    run.numbers.push(NumberRun { command: doc.command.clone(), arg: arg.name.clone(), word: word.to_string(), call });
                }
            }
        }
        // An integer argument without a domain is probed too.
        for arg in doc.args().filter(|arg| arg.domain.is_none() && arg.value_type == "integer") {
            for word in ["9007199254740992", "-9007199254740992"] {
                let Some(changed) = bases.iter().find_map(|base| witness::with_argument(doc, base, arg, word)) else {
                    continue;
                };
                let call = self.invoke(Call::new(invocation::render(&changed)))?;
                run.numbers.push(NumberRun { command: doc.command.clone(), arg: arg.name.clone(), word: word.to_string(), call });
            }
        }
        Ok(())
    }

    fn plan_validators(&mut self, run: &mut L1Run, doc: &CommandDoc, tool: &str) -> Result<(), RunError> {
        for (index, validator) in doc.validators.iter().enumerate() {
            for line in &validator.rejects {
                let Some(words) = invocation::split_line(line).ok().filter(|words| words.first().is_some_and(|word| word == tool)) else {
                    continue;
                };
                let call = self.invoke(Call::new(in_json_mode(&words[1..])).without_setup())?;
                run.validators.push(ValidatorRun { command: doc.command.clone(), validator: index, line: line.clone(), call });
            }
        }
        Ok(())
    }

    /// The `next` entry of `document` that carries `cursor`, as argv.
    fn next_page(document: &Value, cursor: &str) -> Option<Vec<String>> {
        document
            .pointer("/meta/next")?
            .as_array()?
            .iter()
            .filter_map(|entry| entry.get("argv")?.as_array())
            .map(|argv| argv.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>())
            .find(|argv| {
                argv.iter().enumerate().any(|(at, word)| {
                    (word == "--cursor" && argv.get(at + 1).is_some_and(|value| value == cursor)) || *word == format!("--cursor={cursor}")
                })
            })
    }

    /// The invocations to page: the fixture's `paged` ones, else each observed one whose answer carried `meta.page.nextCursor`
    /// (approaches/checker.md 6, L1.paging), at most three. A command whose document declares `--cursor` with neither is a gap.
    fn plan_paging(&mut self, run: &mut L1Run, doc: &CommandDoc, _bases: &[Invocation]) -> Result<(), RunError> {
        if !doc.has_option("cursor") {
            return Ok(());
        }
        let mut candidates: Vec<Invocation> = Vec::new();
        let listed: Vec<Vec<String>> = self
            .fixture
            .commands
            .as_ref()
            .and_then(|commands| commands.get(&doc.command))
            .map(|entry| entry.paged.iter().map(|argv| in_json_mode(argv)).collect())
            .unwrap_or_default();
        let observed = || -> Vec<Vec<String>> {
            self.log
                .iter()
                .filter(|outcome| {
                    first_document(&outcome.stdout).is_some_and(|document| {
                        document.get("ok") == Some(&Value::Bool(true))
                            && document.pointer("/meta/command").and_then(Value::as_str) == Some(doc.command.as_str())
                            && document.pointer("/meta/page/nextCursor").is_some_and(Value::is_string)
                    })
                })
                .map(|outcome| outcome.call.argv.clone())
                .collect()
        };
        let sources = if listed.is_empty() { observed() } else { listed };
        for argv in sources {
            if let Ok(parsed) = invocation::parse(doc, &argv) {
                if !candidates.iter().any(|known| invocation::render(known) == invocation::render(&parsed)) {
                    candidates.push(parsed);
                }
            }
        }
        candidates.truncate(3);
        if candidates.is_empty() {
            run.paging_gaps.push((
                doc.command.clone(),
                "its document declares --cursor and the fixture lists no `paged` invocation of it, and no run saw a meta.page.nextCursor from it".to_string(),
            ));
            return Ok(());
        }
        for base in candidates {
            self.plan_paging_of(run, doc, &base)?;
        }
        Ok(())
    }

    /// A value for `option`: one a documented example gives it (a value the author wrote is a value the parser takes), else a generated one; never one of `avoid`.
    fn choose(doc: &CommandDoc, option: &Arg, avoid: &[Value]) -> Option<String> {
        let avoided: Vec<String> = avoid.iter().map(witness::word_of).collect();
        let from_examples = doc.examples.iter().filter_map(|line| {
            let words = invocation::split_line(line).ok()?;
            let parsed = invocation::parse(doc, words.get(1..)?).ok()?;
            parsed.items.iter().find_map(|item| match item {
                Item::Option { name, value: Some(value) } if *name == option.name => Some(value.clone()),
                _ => None,
            })
        });
        from_examples
            .filter(|word| !avoided.contains(word))
            .find(|word| invocation::value_problem(option, &witness::typed(option, word)).is_none())
            .or_else(|| witness::generate(option, avoid))
    }

    fn plan_paging_of(&mut self, run: &mut L1Run, doc: &CommandDoc, base: &Invocation) -> Result<(), RunError> {
        let mut limited = base.clone();
        invocation::remove_option(&mut limited, "all");
        invocation::remove_option(&mut limited, "cursor");
        if doc.has_option("limit") {
            invocation::set_option(&mut limited, "limit", Some("1".to_string()));
        }
        let first = self.invoke(Call::new(invocation::render(&limited)))?;
        let sequence = self.log[first].sequence;
        let mut paging = PagingRun { command: doc.command.clone(), limited: doc.has_option("limit"), first, next: None, changed: Vec::new(), why_not: None };
        let document = first_document(&self.log[first].stdout).filter(|document| document.get("ok") == Some(&Value::Bool(true)));
        let cursor = document
            .as_ref()
            .and_then(|document| document.pointer("/meta/page/nextCursor"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let next_argv = cursor.as_deref().zip(document.as_ref()).and_then(|(cursor, document)| Self::next_page(document, cursor));
        let Some(next_argv) = next_argv else {
            paging.why_not = Some(match cursor {
                Some(_) => "the first page carries a cursor but no `next` entry with it (L0.page reports that)".to_string(),
                None => "with --limit 1 the answer has no next page: the fixture state needs at least two records for it".to_string(),
            });
            run.paging.push(paging);
            return Ok(());
        };
        paging.next = Some(self.invoke_after(sequence, Call::new(next_argv.clone()))?);
        // The cursor given with one argument of the filter changed.
        let reserved: BTreeSet<&str> = ["json", "limit", "cursor", "all", "fields", "input"].into_iter().collect();
        let global: BTreeSet<String> = self
            .fixture
            .global_options
            .iter()
            .flatten()
            .filter_map(|option| option.first())
            .map(|name| name.trim_start_matches('-').to_string())
            .collect();
        let mut variants: Vec<(String, Vec<String>)> = Vec::new();
        if let Ok(carried) = invocation::parse(doc, &next_argv) {
            let args = invocation::args(doc, &carried);
            let candidates: Vec<&Arg> = doc
                .options
                .iter()
                .filter(|option| !reserved.contains(option.name.as_str()) && !global.contains(&option.name) && !option.flag_only)
                .collect();
            // An option the call gives: its value changes. Then an option it does not give: it is added.
            for option in candidates.iter().filter(|option| args.options.contains_key(&option.name)) {
                let mut changed = carried.clone();
                if option.is_flag() {
                    invocation::remove_option(&mut changed, &option.name);
                } else {
                    let current = args.options.get(&option.name).cloned().into_iter().collect::<Vec<_>>();
                    let Some(word) = Self::choose(doc, option, &current) else { continue };
                    invocation::set_option(&mut changed, &option.name, Some(word));
                }
                variants.push((format!("--{} changed", option.name), invocation::render(&changed)));
            }
            for option in candidates.iter().filter(|option| !args.options.contains_key(&option.name)).take(4) {
                let mut changed = carried.clone();
                if option.is_flag() {
                    invocation::set_option(&mut changed, &option.name, None);
                } else {
                    let defaults = [option.default.clone()];
                    let Some(word) = Self::choose(doc, option, &defaults) else { continue };
                    invocation::set_option(&mut changed, &option.name, Some(word));
                }
                variants.push((format!("--{} added", option.name), invocation::render(&changed)));
            }
        }
        for (label, argv) in variants.into_iter().take(6) {
            let call = self.invoke_after(sequence, Call::new(argv))?;
            paging.changed.push((label, call));
        }
        if paging.changed.is_empty() {
            paging.why_not = Some("no argument of the command could be changed to test that the cursor is bound to them".to_string());
        }
        run.paging.push(paging);
        Ok(())
    }

    fn plan_fields(&mut self, run: &mut L1Run, doc: &CommandDoc, bases: &[Invocation]) -> Result<(), RunError> {
        if !doc.has_option("fields") {
            return Ok(());
        }
        let Some(base) = bases.first() else {
            return Ok(());
        };
        let selections: Vec<Vec<String>> = self
            .fixture
            .commands
            .as_ref()
            .and_then(|commands| commands.get(&doc.command))
            .map(|entry| entry.fields.clone())
            .unwrap_or_default();
        let mut unprojected = base.clone();
        invocation::remove_option(&mut unprojected, "fields");
        for selection in selections {
            let plain = self.invoke(Call::new(invocation::render(&unprojected)))?;
            let sequence = self.log[plain].sequence;
            let mut projected = unprojected.clone();
            invocation::set_option(&mut projected, "fields", Some(selection.join(",")));
            let projected = self.invoke_after(sequence, Call::new(invocation::render(&projected)))?;
            run.fields.push(FieldsRun { command: doc.command.clone(), selection, plain, projected });
        }
        let mut unknown = unprojected;
        invocation::set_option(&mut unknown, "fields", Some(NO_SUCH_FIELD.to_string()));
        let call = self.invoke(Call::new(invocation::render(&unknown)))?;
        run.unknown_fields.push(UnknownFieldRun { command: doc.command.clone(), call });
        Ok(())
    }

    /// Runs the `next` entries of every answer so far that name a `read` command, each in the sequence that produced it.
    fn plan_nexts(&mut self, run: &mut L1Run) -> Result<(), RunError> {
        let mut seen: BTreeSet<Vec<String>> = BTreeSet::new();
        let mut pending: Vec<(usize, Vec<String>)> = Vec::new();
        for (index, outcome) in self.log.iter().enumerate() {
            if !outcome.call.json_mode() {
                continue;
            }
            let Some(document) = first_document(&outcome.stdout) else { continue };
            let Some(entries) = document.pointer("/meta/next").and_then(Value::as_array) else { continue };
            for entry in entries {
                let argv: Vec<String> = entry
                    .get("argv")
                    .and_then(Value::as_array)
                    .map(|argv| argv.iter().filter_map(Value::as_str).map(str::to_string).collect())
                    .unwrap_or_default();
                if !argv.is_empty() && seen.insert(argv.clone()) {
                    pending.push((index, argv));
                }
            }
        }
        for (source, argv) in pending.into_iter().take(MAX_NEXT_RUNS) {
            let Some(doc) = names_command(&run.docs, &argv) else { continue };
            let Some(CommandDoc { effect, .. }) = run.docs.commands.get(&doc).cloned() else { continue };
            if effect != "read" {
                continue;
            }
            let sequence = self.log[source].sequence;
            if sequence == 0 {
                continue;
            }
            let call = self.invoke_after(sequence, Call::new(argv.clone()))?;
            run.nexts.push(NextRun { source, argv, call });
        }
        Ok(())
    }

    /// Like [`Run::invoke`], in the sequence that `sequence` names.
    pub(crate) fn invoke_after(&mut self, sequence: usize, call: Call) -> Result<usize, RunError> {
        let outcome = self.runner.run_after(sequence, &call)?;
        self.log.push(outcome);
        Ok(self.log.len() - 1)
    }
}

/// The id of the command `argv` names: its words, after any leading tool-wide options, begin with the path of a command or group of the tree.
pub(crate) fn names_command(docs: &ToolDocs, argv: &[String]) -> Option<String> {
    let tree = docs.tree.as_ref()?;
    (0..argv.len()).find_map(|start| {
        tree.commands
            .iter()
            .filter(|entry| argv[start..].starts_with(&entry.path))
            .max_by_key(|entry| entry.path.len())
            .map(|entry| entry.command.clone())
    })
    .filter(|_| argv.first().is_some_and(|first| !first.starts_with('-')) || argv.iter().any(|word| !word.starts_with('-')))
}

/// Whether `outcome`'s envelope, when it is a failure, has `code` and `field`.
pub(crate) fn refusal_of(document: &Value) -> (Option<&str>, Option<&str>) {
    (
        document.pointer("/error/code").and_then(Value::as_str),
        document.pointer("/error/field").and_then(Value::as_str),
    )
}

/// The `Output` of `doc` that carries `fields`, by the name `meta.output` gave or the only one.
pub(crate) fn output_with_fields<'a>(doc: &'a CommandDoc, name: Option<&str>) -> Option<&'a Output> {
    doc.output(name).filter(|output| output.fields.is_some())
}

/// The value a fixture `ok` invocation gives `arg` of `doc`, for the accepted values of `L1.patterns`.
pub(crate) fn given_value(doc: &CommandDoc, invocation: &Invocation, arg: &Arg) -> Option<String> {
    invocation.items.iter().find_map(|item| match item {
        Item::Option { name, value } if arg.is_option && *name == arg.name => value.clone(),
        _ => None,
    }).or_else(|| {
        let index = doc.operands.iter().position(|operand| operand.name == arg.name)?;
        invocation
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Operand(value) => Some(value.clone()),
                Item::Option { .. } => None,
            })
            .nth(index)
    })
}
