//! What to do after a successful write (0.21 D2).
//!
//! A write command that succeeds names the next step itself, so the agent
//! does not call `canon status` only to learn it. The steps are
//! [`crate::status`]'s own: the same engine, run on the state after the
//! write, then narrowed to what the write touched ([`Scope`]). Nothing is
//! re-derived here, so a write and `canon status` cannot disagree about
//! the next step. canon never runs a step; `actor: human` marks the ones
//! only a human takes.
//!
//! JSON mode puts them in `meta.next` (`[{command, argv, reason, x-canon}]`,
//! at most [`NEXT_LIMIT`]; `data.nextOmitted` counts the rest). `argv` is the
//! command without the executable, every word literal, with `--json` when the
//! run is in JSON mode and the next command takes it; `x-canon` carries who
//! takes the step (`actor`, `role`). A step that still holds a `<placeholder>`
//! for a value only the agent knows is not an entry (the guidance is the
//! error's `hint` or the report's text), and neither is a step that is not a
//! canon command line. Human mode ends with one `next: <command> — <why>`
//! line and `(+N more)`.
//! With a declared role ([`Role::current`]) only that role's steps are
//! kept and the rest are counted per role (`data.waitingOn`,
//! `waiting on reviewer: 2`), as in `canon status`.
//!
//! # Scope
//! A write names its subject, change or scenario; the steps are those of
//! the subject(s) it joins to, plus the repo-wide steps that block every
//! subject (setup, a stale or unreadable index, an unusable policy). A
//! write that touched nothing subject-shaped uses [`Scope::Repo`]: every
//! step. A scenario no subject owns yet falls back to the repo-wide list.
//!
//! # Staged writes
//! A record staged for `canon gate promote` is not in the ledger yet, so
//! the engine cannot count it: the one step is the promote.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::json_out::Report;
use crate::role::Role;
use crate::status::{self, NextStep, StatusReport, StepActor, NEXT_LIMIT};

/// One `meta.next` entry: a runnable step (spec 6.8).
#[derive(Debug, Clone, Serialize)]
pub struct NextEntry {
    /// The whole invocation as one display string: `canon` and the words of `argv`, quoted as a shell would.
    pub command: String,
    /// The invocation without the executable; every word is literal.
    pub argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(rename = "x-canon", skip_serializing_if = "Option::is_none")]
    pub x_canon: Option<NextExt>,
}

/// What canon adds to a next step: who takes it.
#[derive(Debug, Clone, Serialize)]
pub struct NextExt {
    /// `agent`, or `human` for the one step only a human takes.
    pub actor: StepActor,
    /// Who in the role set carries the step out; absent for a step only the human takes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
}

/// The clap tree, built once: which commands take `--json`.
static TREE: std::sync::LazyLock<clap::Command> = std::sync::LazyLock::new(|| {
    let mut tree = crate::cli::command();
    tree.build();
    tree
});

/// Whether the command `words` names takes `--json`.
fn takes_json(words: &[String]) -> bool {
    let mut node: &clap::Command = &TREE;
    for word in words.iter().take_while(|word| *word != "--").filter(|word| !word.starts_with('-')) {
        match node.find_subcommand(word) {
            Some(child) => node = child,
            None => break,
        }
    }
    node.get_subcommands().next().is_none() && node.get_arguments().any(|arg| arg.get_id() == "json")
}

impl NextEntry {
    /// The entry for the invocation `argv` (without the executable), or none when `argv` is empty.
    /// In JSON mode `--json` is added where the command takes it, before a `--`.
    pub fn runnable(mut argv: Vec<String>, reason: &str, x_canon: Option<NextExt>) -> Option<Self> {
        if argv.is_empty() {
            return None;
        }
        if atip::json_mode() && takes_json(&argv) && !argv.iter().take_while(|word| *word != "--").any(|word| word == "--json") {
            let at = argv.iter().position(|word| word == "--").unwrap_or(argv.len());
            argv.insert(at, "--json".to_string());
        }
        let command = std::iter::once("canon").chain(argv.iter().map(String::as_str)).map(atip::quote).collect::<Vec<_>>().join(" ");
        Some(Self { command, argv, reason: (!reason.is_empty()).then(|| reason.to_string()), x_canon })
    }

    /// The entry for a status step, or none when the step is not a runnable canon command line.
    pub fn from_step(step: &NextStep) -> Option<Self> {
        if !step.command.starts_with("canon ") || atip::has_placeholder(&step.command) {
            return None;
        }
        let mut words = atip::split_argv(&step.command);
        words.remove(0);
        Self::runnable(words, &step.why, Some(NextExt { actor: step.actor, role: step.role }))
    }
}

/// What a write touched, to narrow the status steps to it.
#[derive(Debug, Clone)]
pub enum Scope {
    /// Nothing subject-shaped: every step.
    Repo,
    Subject(String),
    /// A change: the subjects joined to it.
    Change(String),
    /// Scenario ids: their owning subjects.
    Scenarios(Vec<String>),
}

/// The steps after one write.
#[derive(Debug, Clone, Default)]
pub struct After {
    pub steps: Vec<NextStep>,
    /// Steps past [`NEXT_LIMIT`].
    pub omitted: usize,
    /// With a declared role: the steps each other owner holds.
    pub waiting_on: Option<BTreeMap<String, usize>>,
}

/// Steps that block every subject, so a scoped list keeps them.
fn blocks_everything(step: &NextStep) -> bool {
    matches!(step.command.as_str(), "canon init" | "canon init --check" | "canon migrate" | "canon inventory sync" | "canon gate check")
}

/// The report's steps narrowed to `scope`.
fn scoped(report: &StatusReport, scope: &Scope) -> Vec<NextStep> {
    let subjects: Option<Vec<String>> = match scope {
        Scope::Repo => None,
        Scope::Subject(id) => Some(vec![id.clone()]),
        Scope::Change(id) => Some(report.change_subjects.get(id).cloned().unwrap_or_default()),
        Scope::Scenarios(ids) => {
            let mut owners: Vec<String> = ids.iter().filter_map(|id| report.scenario_owner.get(id).cloned()).collect();
            owners.dedup();
            (!owners.is_empty()).then_some(owners)
        }
    };
    let Some(subjects) = subjects else { return report.all_next.clone() };
    report
        .all_next
        .iter()
        .zip(&report.step_subjects)
        .filter(|(step, owner)| match owner {
            Some(owner) => subjects.contains(owner),
            None => blocks_everything(step),
        })
        .map(|(step, _)| step.clone())
        .collect()
}

/// [`scoped`] under `role`, limited to [`NEXT_LIMIT`].
fn present(steps: Vec<NextStep>, role: Option<Role>) -> After {
    let (steps, waiting_on) = match role {
        Some(role) => {
            let (mine, waiting) = status::split_by_role(steps, role);
            (mine, Some(waiting))
        }
        None => (steps, None),
    };
    // JSON mode lists only steps a caller can run as they are, so the limit counts those.
    let steps: Vec<NextStep> = if atip::json_mode() { steps.into_iter().filter(|step| NextEntry::from_step(step).is_some()).collect() } else { steps };
    let omitted = steps.len().saturating_sub(NEXT_LIMIT);
    After { steps: steps.into_iter().take(NEXT_LIMIT).collect(), omitted, waiting_on }
}

/// The steps after `command` wrote under `repo`, from the state now.
/// `staged`: the write waits for `canon gate promote` and is not in the
/// ledger.
pub fn after(repo: &Path, scope: &Scope, staged: bool) -> After {
    after_with(repo, scope, staged, Vec::new())
}

/// [`after`] with `extra` steps the command itself knows, listed after
/// the engine's: `canon change new` on a shipped subject names the reopen,
/// which status only offers once the subject's gate is red.
pub fn after_with(repo: &Path, scope: &Scope, staged: bool, extra: Vec<NextStep>) -> After {
    let role = Role::current(None);
    if staged {
        let promote = NextStep {
            command: "canon gate promote".into(),
            why: "the record is staged, not in the ledger yet; promote it so status and the gate count it".into(),
            actor: StepActor::Agent,
            role: Some(Role::Implementer),
        };
        return present(vec![promote], role);
    }
    let repo = crate::context::resolve_repo_root(repo);
    let mut steps = scoped(&status::resolve_with_pending_index(&repo), scope);
    steps.extend(extra);
    present(steps, role)
}

impl After {
    /// A command's own steps, filtered and limited like the engine's.
    pub fn fixed(steps: Vec<NextStep>) -> Self {
        present(steps, Role::current(None))
    }

    /// The steps as `meta.next` entries: the runnable ones.
    pub fn entries(&self) -> Vec<NextEntry> {
        self.steps.iter().filter_map(NextEntry::from_step).collect()
    }

    /// The steps `canon status` reports, as a write's `After` would list
    /// them: every step the rules produced, under the declared role, limited.
    pub fn of_status(report: &StatusReport) -> Self {
        present(report.all_next.clone(), Role::current(None))
    }

    /// The human form: one `next:` line, then a `waiting on` line per
    /// other role. Empty when there is nothing to say.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(step) = self.steps.first() {
            let more = if self.steps.len() + self.omitted > 1 { format!(" (+{} more)", self.steps.len() + self.omitted - 1) } else { String::new() };
            let command = match step.actor {
                StepActor::Agent => step.command.clone(),
                StepActor::Human => format!("human: {}", step.command),
            };
            lines.push(format!("next: {command} — {}{more}", step.why));
        }
        for (role, count) in self.waiting_on.iter().flatten() {
            lines.push(format!("waiting on {role}: {count}"));
        }
        lines
    }
}

/// A write's last output: its JSON result with `meta.next` (JSON mode), or
/// the human `next:` line.
pub fn finish<R: Report>(repo: &Path, command: &str, scope: &Scope, staged: bool, json: bool, data: &R) {
    finish_with_meta(repo, command, scope, staged, json, data, Value::Null);
}

/// [`finish`] with extra `meta` keys beside `next` (a page's cursor).
pub fn finish_with_meta<R: Report>(repo: &Path, command: &str, scope: &Scope, staged: bool, json: bool, data: &R, extra_meta: Value) {
    finish_with_steps(repo, command, scope, staged, json, data, Extras { meta: extra_meta, steps: Vec::new() });
}

/// What [`finish_with_steps`] adds beside the scope's own `next:`.
#[derive(Debug)]
pub struct Extras {
    /// Extra `meta` keys (a page's cursor, warnings).
    pub meta: Value,
    /// Extra steps ([`after_with`]).
    pub steps: Vec<NextStep>,
}

/// [`finish_with_meta`] with extra steps ([`after_with`]).
pub fn finish_with_steps<R: Report>(repo: &Path, command: &str, scope: &Scope, staged: bool, json: bool, data: &R, extras: Extras) {
    let Extras { meta, steps } = extras;
    let after = after_with(repo, scope, staged, steps);
    if json {
        crate::json_out::data_with_next_and_meta(command, data, &after, meta);
    } else {
        for line in after.lines() {
            println!("{line}");
        }
    }
}
