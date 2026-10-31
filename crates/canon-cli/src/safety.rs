//! Preview and commit for the five destructive leaves (0.28 D6): `purge`,
//! `artifacts move`, `migrate`, `ledger compact`, `tier age`.
//!
//! A bare run computes the plan and prints it: `effects[]`, the `planDigest`
//! over them, `committed: false`, exit 0, nothing written (no audit line
//! either). `--commit --plan-digest <DIGEST>` computes the plan again and acts
//! only when its digest is the one given; a plan that changed since is
//! `PLAN_CHANGED` (exit 2) and nothing is written. The flags are
//! [`SafetyArgs`](crate::cli::SafetyArgs), read as a [`Mode`].
//!
//! A handler builds its `Vec<Effect>`, wraps it in [`Plan::new`], asks
//! [`Plan::gate`] whether to act, does the work, and finishes with
//! [`Plan::committed`] in the report. The digest is the sha256 of the effects'
//! JSON serialization, whose field order is fixed by [`Effect`], so two plans
//! with the same effects in the same order have the same digest. A handler
//! lists its effects in a deterministic order.

use schemars::JsonSchema;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cli::SafetyArgs;
use crate::cli_error::{CliError, CliErrorExt as _};
use crate::json_out::Report;
use crate::next::After;
use crate::role::Role;
use crate::status::{NextStep, StepActor};

/// Effects a human preview lists before it says how many more there are.
const PREVIEW_LINES: usize = 50;

/// What a destructive leaf's run is: the bare preview, or the commit of the
/// plan whose digest is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Preview,
    Commit(String),
}

impl SafetyArgs {
    /// The mode the flags select (clap guarantees `--commit` and
    /// `--plan-digest` come together).
    pub fn mode(&self) -> Mode {
        match (self.commit, &self.plan_digest) {
            (true, Some(digest)) => Mode::Commit(digest.clone()),
            _ => Mode::Preview,
        }
    }
}

/// One thing a destructive run would do (or did): the `action`, and the file
/// (`path`, repo-relative) or record (`record`) it acts on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    /// What happens, e.g. `delete`, `move`, `upload`, `rewrite`, `remove`.
    pub action: &'static str,
    /// The file acted on, relative to the repo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The record acted on (an id, key or digest) when it is not a file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
    /// Where it goes, or why, when `action` alone does not say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Effect {
    pub fn path(action: &'static str, path: impl Into<String>) -> Self {
        Self { action, path: Some(path.into()), record: None, detail: None }
    }

    pub fn record(action: &'static str, record: impl Into<String>) -> Self {
        Self { action, path: None, record: Some(record.into()), detail: None }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// The three fields every destructive leaf's report carries, flattened into it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// sha256 (hex) of the plan's `effects`; `--commit --plan-digest` must repeat it.
    pub plan_digest: String,
    /// Everything the run would do, in the order it does it.
    pub effects: Vec<Effect>,
    /// `true` once the run acted, `false` for a preview.
    pub committed: bool,
}

/// The digest of `effects`.
pub fn digest(effects: &[Effect]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(effects).expect("effects serialize"));
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

impl Plan {
    /// A preview of `effects` with its digest.
    pub fn new(effects: Vec<Effect>) -> Self {
        Self { plan_digest: digest(&effects), effects, committed: false }
    }

    /// Whether the run acts: `Ok(false)` for a preview, `Ok(true)` when
    /// `mode` commits this very plan, `PLAN_CHANGED` when it commits another.
    /// `base` is the command line without the safety flags (see [`command`]),
    /// for the hint.
    pub fn gate(&self, mode: &Mode, base: &str) -> Result<bool, CliError> {
        match mode {
            Mode::Preview => Ok(false),
            Mode::Commit(given) if *given == self.plan_digest => Ok(true),
            Mode::Commit(given) => Err(CliError::refused("PLAN_CHANGED", format!("the plan digest {given} no longer matches the plan (now {}); nothing was written", self.plan_digest))
                .hint(format!("run `{base}` again, review the new plan, then commit it with the digest it prints"))),
        }
    }

    /// The same plan, marked as acted on.
    pub fn committed(mut self) -> Self {
        self.committed = true;
        self
    }

    /// The step after a preview: the exact command that commits this plan.
    pub fn next_step(&self, base: &str, why: &str) -> After {
        After::fixed(vec![NextStep { command: format!("{base} --commit --plan-digest {} --json", self.plan_digest), why: why.to_string(), actor: StepActor::Agent, role: Some(Role::Orchestrator) }])
    }

    /// The human lines of a preview: each effect, then the commit command.
    pub fn preview_lines(&self, base: &str) -> Vec<String> {
        let mut lines: Vec<String> = self.effects.iter().take(PREVIEW_LINES).map(effect_line).collect();
        if self.effects.len() > PREVIEW_LINES {
            lines.push(format!("  ... and {} more (`--json` lists every effect)", self.effects.len() - PREVIEW_LINES));
        }
        lines.push(self.commit_hint(base));
        lines
    }

    /// The closing line of a human preview: the digest and the commit command.
    pub fn commit_hint(&self, base: &str) -> String {
        format!("planDigest {}; nothing written. To act: {base} --commit --plan-digest {}", self.plan_digest, self.plan_digest)
    }
}

/// A preview's output: the report with the commit command as `meta.next[0]`
/// (JSON), or the effects and that command as lines (human). `command` is the
/// `data` command id (`purge`, `artifacts move`, ...).
pub fn finish_preview<R: Report>(base: &str, command: &str, plan: &Plan, why: &str, json: bool, report: &R) {
    if json {
        crate::json_out::data_with_next(command, report, &plan.next_step(base, why));
    } else {
        for line in plan.preview_lines(base) {
            println!("{line}");
        }
    }
}

/// One effect as a human line.
pub fn effect_line(effect: &Effect) -> String {
    let target = effect.path.as_deref().or(effect.record.as_deref()).unwrap_or("");
    match &effect.detail {
        Some(detail) => format!("  {} {target} ({detail})", effect.action),
        None => format!("  {} {target}", effect.action),
    }
}

/// The command line a destructive leaf was run as, without its safety flags:
/// `canon <leaf>` plus `parts`, plus `--repo <dir>` when one was typed.
pub fn command(leaf: &str, parts: &[(&str, String)], repo: &crate::cli::RepoArgs) -> String {
    let mut line = format!("canon {leaf}");
    for (flag, value) in parts {
        line.push_str(&format!(" {flag} {}", atip::quote(value)));
    }
    if let Some(dir) = repo.given() {
        line.push_str(&format!(" --repo {}", atip::quote(&dir.display().to_string())));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_follows_the_effects_and_their_order() {
        let a = vec![Effect::path("delete", "a"), Effect::record("delete", "b")];
        let b = vec![Effect::record("delete", "b"), Effect::path("delete", "a")];
        assert_eq!(digest(&a), digest(&a.clone()));
        assert_ne!(digest(&a), digest(&b));
        assert_ne!(digest(&a), digest(&a[..1]));
    }

    #[test]
    fn a_stale_digest_is_plan_changed_and_the_right_one_commits() {
        let plan = Plan::new(vec![Effect::path("delete", "a")]);
        assert!(!plan.gate(&Mode::Preview, "canon x").unwrap());
        assert!(plan.gate(&Mode::Commit(plan.plan_digest.clone()), "canon x").unwrap());
        assert_eq!(plan.gate(&Mode::Commit("0".into()), "canon x").unwrap_err().code, "PLAN_CHANGED");
    }
}
