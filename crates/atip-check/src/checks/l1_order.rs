//! The invocations of `L1.check-order` (approaches/checker.md 6; spec 11.7 step 6): a passing invocation of the fixture changed to break two
//! stages at once, so that the answer says which stage answers first.
//!
//! The stages are, in the order that answers: a failure of argv parsing (among them the earliest position in argv), a missing required
//! argument, a broken domain (operands in order, then options in declaration order), a broken constraint and a failing validator (both in
//! declaration order). Each pair that the command can have is built from one passing invocation (the first `ok` of the fixture, or
//! the first example of a `read` command) by changes to single arguments; the checker's own reading of the document says which rules
//! the result breaks, and a pair that would break a third rule is not used. Each pair is run, and so is its earlier fault alone: the answer of
//! the pair must be the answer of that fault (its code and its `field`).

use std::collections::{BTreeMap, BTreeSet};

use super::l1_plan::{ConstraintRun, L1Run};
use super::Run;
use crate::exec::{Call, RunError};
use crate::l1::invocation::{self, Invocation, Item};
use crate::l1::model::{Arg, CommandDoc};
use crate::l1::witness::{self, Expect, GroupOutcome};

/// The options no command has, for the two unknown options of a pair.
const UNKNOWN_A: &str = "no-such-a";
const UNKNOWN_B: &str = "no-such-b";
/// A value of every type, wrong for an integer, a number and an enum.
const NOT_A_NUMBER: &str = "not-a-number";
const NOT_A_MEMBER: &str = "no-such-value";
/// The most pairs of one kind a command is run with.
const MAX_PER_KIND: usize = 2;

/// One pair of faults run: the invocation that breaks both, the one that breaks the earlier alone, and what each is called.
#[derive(Debug, Clone)]
pub(crate) struct OrderRun {
    pub command: String,
    /// What the pair is: `a broken domain and a broken constraint`.
    pub pair: &'static str,
    /// The stage and argument that must answer.
    pub earlier: String,
    /// The stage that must not.
    pub later: String,
    /// The invocation that breaks both.
    pub call: usize,
    /// The invocation that breaks the earlier alone: what the answer of the pair has to equal.
    pub alone: usize,
}

/// The stages in the order that answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stage {
    Parse,
    Required,
    Domain,
    Constraint,
    Validator,
}

/// A change to one argument of an invocation.
#[derive(Debug, Clone)]
enum Edit {
    /// An option the command does not have, at the front or at the end of the arguments.
    Unknown { name: &'static str, first: bool },
    /// `arg` given as `word`.
    Set { arg: Arg, word: String },
    /// `arg` left out.
    Remove { arg: Arg },
    /// The arguments in which `witness` differs from `base`, copied (a constraint's witness).
    Diff { witness: Invocation, base: Invocation },
    /// A validator's `rejects` invocation: what the pair starts from, not a change.
    Rejects { invocation: Invocation },
}

/// One thing a pair breaks.
#[derive(Debug, Clone)]
struct Fault {
    stage: Stage,
    /// Where it stands among the faults of its stage (the earliest answers).
    position: usize,
    label: String,
    edit: Edit,
    /// For a constraint: the rules (indices of `constraints`) it breaks, and the code it declares.
    members: Vec<usize>,
    code: Option<String>,
}

/// What the checker reads of an invocation: the required arguments it leaves out, the arguments whose value is outside their type or domain,
/// and the constraints it breaks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Broken {
    missing: BTreeSet<String>,
    values: BTreeSet<String>,
    constraints: BTreeSet<usize>,
}

fn broken(doc: &CommandDoc, invocation: &Invocation) -> Broken {
    let args = invocation::args(doc, invocation);
    let mut found = Broken::default();
    for arg in doc.args() {
        if arg.required && !invocation::arg_given(doc, &args, &arg.name) {
            found.missing.insert(arg.name.clone());
        }
    }
    found.values = invocation::broken_arguments(doc, invocation).into_iter().collect();
    found.constraints = invocation::violated_constraints(doc, &args).into_iter().collect();
    found
}

fn without_option(invocation: &mut Invocation, name: &str) {
    invocation::remove_option(invocation, name);
}

/// `fault` applied to `invocation`; `None` when it cannot be (the operand it names cannot be given).
fn apply(doc: &CommandDoc, fault: &Fault, invocation: &Invocation) -> Option<Invocation> {
    match &fault.edit {
        Edit::Unknown { name, first } => {
            let mut changed = invocation.clone();
            let item = Item::Option { name: (*name).to_string(), value: None };
            if *first {
                changed.items.insert(0, item);
            } else {
                changed.items.push(item);
            }
            Some(changed)
        }
        Edit::Set { arg, word } => witness::with_argument(doc, invocation, arg, word),
        Edit::Remove { arg } => {
            let mut changed = invocation.clone();
            if arg.is_option {
                without_option(&mut changed, &arg.name);
            } else {
                let index = doc.operands.iter().position(|operand| operand.name == arg.name)?;
                invocation::remove_operand(&mut changed, index);
            }
            Some(changed)
        }
        Edit::Rejects { invocation } => Some(invocation.clone()),
        Edit::Diff { witness, base } => {
            let mut changed = invocation.clone();
            let options = |invocation: &Invocation, name: &str| -> Vec<Item> {
                invocation
                    .items
                    .iter()
                    .filter(|item| matches!(item, Item::Option { name: found, .. } if found == name))
                    .cloned()
                    .collect()
            };
            let names: BTreeSet<&str> = witness
                .items
                .iter()
                .chain(base.items.iter())
                .filter_map(|item| match item {
                    Item::Option { name, .. } => Some(name.as_str()),
                    Item::Operand(_) => None,
                })
                .collect();
            for name in names {
                let (wanted, had) = (options(witness, name), options(base, name));
                if wanted != had {
                    without_option(&mut changed, name);
                    changed.items.extend(wanted);
                }
            }
            let operands = |invocation: &Invocation| -> Vec<String> {
                invocation
                    .items
                    .iter()
                    .filter_map(|item| match item {
                        Item::Operand(word) => Some(word.clone()),
                        Item::Option { .. } => None,
                    })
                    .collect()
            };
            let wanted = operands(witness);
            if wanted != operands(base) {
                invocation::remove_operand(&mut changed, 0);
                for (index, word) in wanted.into_iter().enumerate() {
                    invocation::set_operand(&mut changed, index, word);
                }
            }
            Some(changed)
        }
    }
}

/// What `fault` adds to the arguments `broken` reads.
fn add_to(expected: &mut Broken, doc: &CommandDoc, fault: &Fault) {
    match &fault.edit {
        Edit::Remove { arg } => {
            expected.missing.insert(arg.name.clone());
            // An operand cannot be left out without the ones after it.
            if !arg.is_option {
                if let Some(index) = doc.operands.iter().position(|operand| operand.name == arg.name) {
                    expected.missing.extend(doc.operands[index..].iter().filter(|operand| operand.required).map(|operand| operand.name.clone()));
                }
            }
        }
        Edit::Set { arg, .. } => {
            expected.values.insert(arg.name.clone());
        }
        Edit::Diff { .. } => expected.constraints.extend(fault.members.iter().copied()),
        Edit::Unknown { .. } | Edit::Rejects { .. } => {}
    }
}

/// The invocation that breaks `faults` and only them, when there is one: the faults are applied in the order they are given, then the checker's reading of the document
/// has to say exactly what they break.
fn build(doc: &CommandDoc, base: &Invocation, faults: &[&Fault]) -> Option<Invocation> {
    let mut invocation = base.clone();
    for fault in faults {
        invocation = apply(doc, fault, &invocation)?;
    }
    let mut expected = Broken::default();
    for fault in faults {
        add_to(&mut expected, doc, fault);
    }
    (broken(doc, &invocation) == expected).then_some(invocation)
}

impl Run<'_> {
    /// Plans the pairs of `doc` (spec 11.7 step 6), from the first of `bases`; a command with no passing invocation is a gap.
    pub(crate) fn plan_order(&mut self, run: &mut L1Run, doc: &CommandDoc, bases: &[Invocation], tool: &str) -> Result<(), RunError> {
        let Some(base) = bases.first().filter(|base| broken(doc, base) == Broken::default()) else {
            run.order_gaps.push((doc.command.clone(), "no passing `ok` invocation of the command to change into a pair".to_string()));
            return Ok(());
        };
        let faults = self.faults(run, doc, base, tool);
        let pick = |stage: Stage| faults.iter().filter(move |fault| fault.stage == stage);
        let unknown_a = Fault { stage: Stage::Parse, position: 0, label: format!("the unknown option --{UNKNOWN_A}"), edit: Edit::Unknown { name: UNKNOWN_A, first: true }, members: Vec::new(), code: None };
        let unknown_b = Fault { stage: Stage::Parse, position: 1, label: format!("the unknown option --{UNKNOWN_B}"), edit: Edit::Unknown { name: UNKNOWN_B, first: false }, members: Vec::new(), code: None };
        let wrong_type = doc
            .args()
            .filter(|arg| !matches!(arg.name.as_str(), "input" | "json") && matches!(arg.value_type.as_str(), "integer" | "number" | "enum"))
            .find_map(|arg| {
                let word = if arg.value_type == "enum" { NOT_A_MEMBER } else { NOT_A_NUMBER };
                let fault = Fault { stage: Stage::Parse, position: 2, label: format!("the wrong {} {word} of {}", arg.value_type, arg.spelling()), edit: Edit::Set { arg: arg.clone(), word: word.to_string() }, members: Vec::new(), code: None };
                build(doc, base, &[&fault]).map(|_| fault)
            });
        let required: Vec<&Fault> = pick(Stage::Required).collect();
        let domains: Vec<&Fault> = pick(Stage::Domain).collect();
        let constraints: Vec<&Fault> = pick(Stage::Constraint).collect();
        let validators: Vec<&Fault> = pick(Stage::Validator).collect();
        let mut pairs: Vec<(&'static str, Fault, Fault, Invocation)> = Vec::new();
        // At most `MAX_PER_KIND` pairs of a kind: the first ones the command can have, in the order of the faults.
        let mut add = |kind: &'static str, found: Vec<(&Fault, &Fault, Invocation)>| {
            for (earlier, later, invocation) in found.into_iter().take(MAX_PER_KIND) {
                pairs.push((kind, earlier.clone(), later.clone(), invocation));
            }
        };
        // Two parse failures: two unknown options; an unknown option before a value of the wrong type.
        add("two unknown options", build(doc, base, &[&unknown_b, &unknown_a]).map(|invocation| (&unknown_a, &unknown_b, invocation)).into_iter().collect());
        if let Some(wrong) = &wrong_type {
            add("an unknown option and a value of the wrong type", build(doc, base, &[wrong, &unknown_a]).map(|invocation| (&unknown_a, wrong, invocation)).into_iter().collect());
        }
        // An unknown option and a missing required argument.
        add(
            "an unknown option and a missing required argument",
            required.iter().filter_map(|required| build(doc, base, &[required, &unknown_a]).map(|invocation| (&unknown_a, *required, invocation))).collect(),
        );
        // A missing required argument and a broken domain, or a broken constraint.
        let mut found = Vec::new();
        for required in &required {
            for domain in &domains {
                if let Some(invocation) = build(doc, base, &[domain, required]) {
                    found.push((*required, *domain, invocation));
                }
            }
        }
        add("a missing required argument and a broken domain", found);
        let mut found = Vec::new();
        for required in &required {
            for constraint in &constraints {
                if let Some(invocation) = build(doc, base, &[constraint, required]) {
                    found.push((*required, *constraint, invocation));
                }
            }
        }
        add("a missing required argument and a broken constraint", found);
        // Two broken domains: the one that stands first answers.
        let mut found = Vec::new();
        for (at, first) in domains.iter().enumerate() {
            for second in &domains[at + 1..] {
                if let Some(invocation) = build(doc, base, &[second, first]) {
                    found.push((*first, *second, invocation));
                }
            }
        }
        add("two broken domains", found);
        // A broken domain and a broken constraint.
        let mut found = Vec::new();
        for domain in &domains {
            for constraint in &constraints {
                if let Some(invocation) = build(doc, base, &[constraint, domain]) {
                    found.push((*domain, *constraint, invocation));
                }
            }
        }
        add("a broken domain and a broken constraint", found);
        // Two constraints with different codes: the one declared first answers.
        let mut found = Vec::new();
        for (at, first) in constraints.iter().enumerate() {
            for second in &constraints[at + 1..] {
                if first.code == second.code {
                    continue;
                }
                if let Some(invocation) = build(doc, base, &[second, first]) {
                    found.push((*first, *second, invocation));
                }
            }
        }
        add("two constraints with different codes", found);
        // A constraint and a validator with different codes: the validator's `rejects` invocation, changed to break the constraint too.
        let mut found = Vec::new();
        for validator in &validators {
            let Edit::Rejects { invocation: rejects } = &validator.edit else { continue };
            for constraint in constraints.iter().filter(|constraint| constraint.code != validator.code) {
                // The constraint must leave the arguments the validators read as they are, and the pair breaks only it besides.
                let Some(changed) = apply(doc, constraint, rejects) else { continue };
                let mut expected = Broken::default();
                add_to(&mut expected, doc, constraint);
                if broken(doc, rejects) == Broken::default() && broken(doc, &changed) == expected && validator_args_kept(doc, rejects, &changed) {
                    found.push((*constraint, *validator, changed));
                }
            }
        }
        add("a constraint and a validator with different codes", found);
        let mut alone_runs: BTreeMap<Vec<String>, usize> = BTreeMap::new();
        for (pair, earlier, later, invocation) in pairs {
            let Some(alone) = apply(doc, &earlier, base) else { continue };
            let alone_words = invocation::render(&alone);
            let alone_call = match alone_runs.get(&alone_words) {
                Some(call) => *call,
                None => {
                    let call = self.invoke(Call::new(alone_words.clone()))?;
                    alone_runs.insert(alone_words, call);
                    call
                }
            };
            let call = self.invoke(Call::new(invocation::render(&invocation)))?;
            run.orders.push(OrderRun { command: doc.command.clone(), pair, earlier: earlier.label.clone(), later: later.label.clone(), call, alone: alone_call });
        }
        Ok(())
    }

    /// The faults one passing invocation can take, by stage.
    fn faults(&self, run: &L1Run, doc: &CommandDoc, base: &Invocation, tool: &str) -> Vec<Fault> {
        let mut faults: Vec<Fault> = Vec::new();
        for (position, arg) in doc.args().enumerate() {
            if arg.required {
                faults.push(Fault { stage: Stage::Required, position, label: format!("the missing required {}", arg.spelling()), edit: Edit::Remove { arg: arg.clone() }, members: Vec::new(), code: None });
            }
            if arg.domain.is_some() && !crate::l1::rules::domain_unusable(arg) {
                let rejects = self
                    .fixture
                    .commands
                    .as_ref()
                    .and_then(|commands| commands.get(&doc.command))
                    .and_then(|entry| entry.domain_rejects.get(&arg.name))
                    .cloned()
                    .unwrap_or_default();
                let words = witness::domain_plan(arg)
                    .probes
                    .into_iter()
                    .filter(|probe| probe.expect == Expect::Refused)
                    .map(|probe| probe.word)
                    .chain(rejects);
                for word in words {
                    let fault = Fault { stage: Stage::Domain, position, label: format!("the broken domain of {} ({word})", arg.spelling()), edit: Edit::Set { arg: arg.clone(), word }, members: Vec::new(), code: None };
                    if build(doc, base, &[&fault]).is_some() {
                        faults.push(fault);
                        break;
                    }
                }
            }
        }
        let constraints: Vec<&ConstraintRun> = run.constraints.iter().filter(|constraint| constraint.command == doc.command).collect();
        for constraint in constraints {
            let GroupOutcome::Witness { words } = &constraint.group.outcome else { continue };
            let Ok(witness) = invocation::parse(doc, words) else { continue };
            let code = constraint.group.codes.iter().next().cloned().filter(|_| constraint.group.codes.len() == 1);
            let members = constraint.group.members.clone();
            let label = format!(
                "the broken constraint {}",
                members.iter().filter_map(|index| doc.constraints.get(*index)).map(crate::l1::model::Constraint::label).collect::<Vec<_>>().join(" = ")
            );
            let fault = Fault { stage: Stage::Constraint, position: members.first().copied().unwrap_or(0), label, edit: Edit::Diff { witness, base: base.clone() }, members, code };
            if build(doc, base, &[&fault]).is_some() {
                faults.push(fault);
            }
        }
        for (position, validator) in doc.validators.iter().enumerate() {
            let Some(line) = validator.rejects.first() else { continue };
            let Some(words) = invocation::split_line(line).ok().filter(|words| words.first().is_some_and(|word| word == tool)) else { continue };
            let Ok(rejects) = invocation::parse(doc, &super::in_json_mode(&words[1..])) else { continue };
            faults.push(Fault { stage: Stage::Validator, position, label: format!("the validator {} ({})", validator.code, validator.rejects.first().map_or("", String::as_str)), edit: Edit::Rejects { invocation: rejects }, members: Vec::new(), code: Some(validator.code.clone()) });
        }
        faults.sort_by_key(|fault| (fault.stage, fault.position));
        faults
    }
}

/// Whether `changed` still has the values `rejects` gave the arguments the validators of `doc` read.
fn validator_args_kept(doc: &CommandDoc, rejects: &Invocation, changed: &Invocation) -> bool {
    let (before, after) = (invocation::args(doc, rejects), invocation::args(doc, changed));
    doc.validators.iter().flat_map(|validator| validator.arguments.iter()).all(|name| {
        before.options.get(name) == after.options.get(name)
            && doc.operands.iter().position(|operand| &operand.name == name).is_none_or(|index| before.operands.get(index) == after.operands.get(index))
    })
}
