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
//! JSON mode puts them in `meta.next` (`[{command, argv, why, actor, role}]`,
//! at most [`NEXT_LIMIT`]; `meta.nextOmitted` counts the rest; `argv` is
//! `command` split like a shell would, empty for a step that is not a canon
//! command line). Human
//! mode ends with one `next: <command> — <why>` line and `(+N more)`.
//! With a declared role ([`Role::current`]) only that role's steps are
//! kept and the rest are counted per role (`meta.waitingOn`,
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

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::json_out::Report;
use crate::role::Role;
use crate::status::{self, NextStep, StatusReport, StepActor, NEXT_LIMIT};

/// One `meta.next` entry: a step with its command split for a caller that
/// runs it without a shell.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct NextEntry {
    /// A canon command line when `actor` is `agent`; what to get from the human when it is `human`.
    pub command: String,
    /// `command` split into its words (`["canon", "gate", "check"]`); empty when `command` is not a canon command line.
    pub argv: Vec<String>,
    pub why: String,
    pub actor: StepActor,
    /// Who in the role set carries the step out; `null` for a step only the human takes.
    pub role: Option<Role>,
}

impl From<&NextStep> for NextEntry {
    fn from(step: &NextStep) -> Self {
        let argv = if step.actor == StepActor::Agent && step.command.starts_with("canon ") { split_argv(&step.command) } else { Vec::new() };
        Self { command: step.command.clone(), argv, why: step.why.clone(), actor: step.actor, role: step.role }
    }
}

/// `command` split into words the way a POSIX shell would: whitespace
/// separates, single quotes keep everything, double quotes keep everything but
/// `\"` and `\\`.
pub fn split_argv(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            '\'' => {
                started = true;
                for q in chars.by_ref() {
                    if q == '\'' {
                        break;
                    }
                    word.push(q);
                }
            }
            '"' => {
                started = true;
                while let Some(q) = chars.next() {
                    match q {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some(e @ ('"' | '\\')) => word.push(e),
                            Some(other) => {
                                word.push('\\');
                                word.push(other);
                            }
                            None => word.push('\\'),
                        },
                        other => word.push(other),
                    }
                }
            }
            other => {
                started = true;
                word.push(other);
            }
        }
    }
    if started {
        words.push(word);
    }
    words
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
    matches!(step.command.as_str(), "canon init" | "canon init --check-config" | "canon migrate" | "canon inventory sync" | "canon gate check")
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

    /// The steps as `meta.next` entries.
    pub fn entries(&self) -> Vec<NextEntry> {
        self.steps.iter().map(NextEntry::from).collect()
    }

    /// The steps `canon status` reports, as a write's `After` would: its
    /// own `next`, `nextOmitted` and `waitingOn`.
    pub fn of_status(report: &StatusReport) -> Self {
        Self { steps: report.next.clone(), omitted: report.next_omitted, waiting_on: report.waiting_on.clone() }
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
    finish_with_steps(repo, command, scope, staged, json, data, extra_meta, Vec::new());
}

/// [`finish_with_meta`] with `extra` steps ([`after_with`]).
#[allow(clippy::too_many_arguments)]
pub fn finish_with_steps<R: Report>(repo: &Path, command: &str, scope: &Scope, staged: bool, json: bool, data: &R, extra_meta: Value, extra: Vec<NextStep>) {
    let after = after_with(repo, scope, staged, extra);
    if json {
        crate::json_out::data_with_next_and_meta(command, data, &after, extra_meta);
    } else {
        for line in after.lines() {
            println!("{line}");
        }
    }
}
