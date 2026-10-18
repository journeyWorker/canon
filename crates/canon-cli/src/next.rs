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
//! JSON mode puts them in `meta.next` (`[{command, why, actor, role}]`,
//! at most [`NEXT_LIMIT`]; `meta.nextOmitted` counts the rest). Human
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

use serde_json::{json, Value};

use crate::role::Role;
use crate::status::{self, NextStep, StatusReport, StepActor, NEXT_LIMIT};

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
    matches!(step.command.as_str(), "canon init" | "canon init --check-config" | "canon migrate" | "canon inventory sync" | "canon gate check" | crate::policy_approval::REVIEW_STEP)
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
    present(scoped(&status::resolve_with_pending_index(&repo), scope), role)
}

impl After {
    /// A command's own steps, filtered and limited like the engine's.
    pub fn fixed(steps: Vec<NextStep>) -> Self {
        present(steps, Role::current(None))
    }

    /// `meta`'s extra members: `next`, plus `nextOmitted` and `waitingOn`
    /// when they apply.
    pub fn meta(&self) -> Value {
        let mut meta = json!({ "next": self.steps });
        if self.omitted > 0 {
            meta["nextOmitted"] = json!(self.omitted);
        }
        if let Some(waiting) = &self.waiting_on {
            meta["waitingOn"] = json!(waiting);
        }
        meta
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
pub fn finish(repo: &Path, command: &str, scope: &Scope, staged: bool, json: bool, data: Value) {
    finish_with_meta(repo, command, scope, staged, json, data, Value::Null);
}

/// [`finish`] with extra `meta` keys beside `next` (a page's cursor).
pub fn finish_with_meta(repo: &Path, command: &str, scope: &Scope, staged: bool, json: bool, data: Value, extra_meta: Value) {
    emit(command, json, data, &after(repo, scope, staged), extra_meta);
}

/// [`finish`] with `first` ahead of the engine's steps: a write whose own
/// read-back comes next (`policy approve` → `canon policy show`, 0.22 D2).
pub fn finish_with_first(repo: &Path, command: &str, scope: &Scope, json: bool, data: Value, first: NextStep) {
    let repo = crate::context::resolve_repo_root(repo);
    let mut steps = vec![first];
    steps.extend(scoped(&status::resolve_with_pending_index(&repo), scope));
    emit(command, json, data, &present(steps, Role::current(None)), Value::Null);
}

fn emit(command: &str, json: bool, data: Value, after: &After, extra_meta: Value) {
    if json {
        crate::json_out::data_with_next_and_meta(command, data, after, extra_meta);
    } else {
        for line in after.lines() {
            println!("{line}");
        }
    }
}
