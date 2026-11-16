//! The scenarios a closed change set aside (0.42 `change-drop`): the half of
//! [`canon_gate::set_aside::SetAside`] the ledger cannot answer.
//!
//! A scenario is set aside when it was made by closed changes only:
//! - it carries at least one `@flow:<change>.<F-id>` tag in its `.feature` file, and every tag names a closed
//!   change; and
//! - its creation time (the `at` of its `# canon: {...}` provenance line, which `canon scenario new` writes) falls
//!   inside the window of one of the closed changes its tags name: from the commit that first added the change's
//!   `.canon/changes/<slug>/brief.md` to the time of the change's closing record.
//!
//! The start comes from committed git history, never from the `change` record (the import rebuilds that record from the
//! brief's modification time in every clone). A scenario with no creation time, a tag outside the window and a
//! history git cannot answer (a shallow clone) all leave the scenario required, and each ignored tag is listed
//! ([`IgnoredTag`]) with its reason, so the reader sees why the tag did not count.
//!
//! One computation serves every reader: [`attach`] fills [`GateContext::set_aside`] and the gate's folds do the rest.

use std::collections::BTreeMap;
use std::path::Path;

use canon_gate::set_aside::{IgnoredReason, IgnoredTag};
use canon_gate::GateContext;
use canon_model::{ChangeId, ProjectId, ScenarioId};
use canon_store::git::GitCall;
use chrono::{DateTime, Utc};

/// When a change's window starts, or why that is not known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Start {
    At(DateTime<Utc>),
    /// The brief was never committed: the change has no window yet.
    Uncommitted,
    /// Git could not say (not a repository, a shallow clone whose history stops before the brief).
    Unavailable,
}

/// The commit time that first added `change`'s brief, from committed history.
fn brief_start(repo: &Path, change: &ChangeId) -> Start {
    let path = format!("{}/{}/brief.md", canon_model::paths::CHANGES_DIR, change.as_str());
    let Ok(out) = GitCall::new(repo).args(["log", "--diff-filter=A", "--format=%H %ct", "--", path.as_str()]).run() else {
        return Start::Unavailable;
    };
    let listed = out.stdout_trimmed();
    let earliest = listed.lines().last().and_then(|line| {
        let (sha, time) = line.split_once(' ')?;
        Some((sha.to_string(), DateTime::from_timestamp(time.trim().parse().ok()?, 0)?))
    });
    let shallow = GitCall::new(repo).args(["rev-parse", "--is-shallow-repository"]).run().map(|o| o.stdout_trimmed() == "true");
    let Ok(shallow) = shallow else { return Start::Unavailable };
    if shallow {
        // A commit at the shallow boundary shows every file as added: its time is not when the brief began.
        let boundary = GitCall::new(repo)
            .args(["rev-parse", "--path-format=absolute", "--git-path", "shallow"])
            .run()
            .ok()
            .and_then(|o| std::fs::read_to_string(o.stdout_trimmed()).ok())
            .unwrap_or_default();
        let at_boundary = |sha: &str| boundary.lines().any(|line| line.trim() == sha);
        return match earliest {
            Some((sha, at)) if !at_boundary(&sha) => Start::At(at),
            _ => Start::Unavailable,
        };
    }
    match earliest {
        Some((_, at)) => Start::At(at),
        None => Start::Uncommitted,
    }
}

/// What a set of closed changes sets aside: the scenarios (with the closed changes that made each) and the tags that did not count.
pub type Computed = (BTreeMap<(ProjectId, ScenarioId), Vec<ChangeId>>, Vec<IgnoredTag>);

/// Reads the `.feature` files of `repo` and answers [`Computed`] for `closed` (each change with the time its window ends).
pub fn compute(repo: &Path, closed: &BTreeMap<ChangeId, DateTime<Utc>>) -> Computed {
    let mut set: BTreeMap<(ProjectId, ScenarioId), Vec<ChangeId>> = BTreeMap::new();
    let mut ignored: Vec<IgnoredTag> = Vec::new();
    if closed.is_empty() {
        return (set, ignored);
    }
    let Ok(roots) = crate::inventory::load_spec_roots(&repo.join("canon.yaml")) else { return (set, ignored) };
    let mut starts: BTreeMap<ChangeId, Start> = BTreeMap::new();
    for root in roots {
        for path in canon_fmt::util::feature_files(&root.root) {
            let Some(file) = crate::inventory::feature_file_scan(&path) else { continue };
            for scenario in &file.scan.scenarios {
                let Ok(scenario_id) = ScenarioId::parse(&scenario.scenario_id) else { continue };
                let tagged: Vec<ChangeId> = scenario
                    .flow_tags
                    .iter()
                    .filter_map(|tag| tag.rsplit_once('.').and_then(|(change, _)| ChangeId::parse(change).ok()))
                    .fold(Vec::new(), |mut all, change| {
                        if !all.contains(&change) {
                            all.push(change);
                        }
                        all
                    });
                // Only a scenario whose every tag names a closed change can have been made by closed changes alone.
                if tagged.is_empty() || !tagged.iter().all(|change| closed.contains_key(change)) {
                    continue;
                }
                let created = file.scan.created_at.get(&scenario.scenario_id).copied();
                let mut within: Vec<ChangeId> = Vec::new();
                let mut reasons: Vec<(ChangeId, IgnoredReason)> = Vec::new();
                for change in &tagged {
                    let end = closed[change];
                    let reason = match created {
                        None => Some(IgnoredReason::NoCreationTime),
                        Some(created) => match *starts.entry(change.clone()).or_insert_with(|| brief_start(repo, change)) {
                            Start::Unavailable => Some(IgnoredReason::HistoryUnavailable),
                            Start::Uncommitted => Some(IgnoredReason::OutsideWindow),
                            Start::At(start) if created < start || created > end => Some(IgnoredReason::OutsideWindow),
                            Start::At(_) => None,
                        },
                    };
                    match reason {
                        None => within.push(change.clone()),
                        Some(reason) => reasons.push((change.clone(), reason)),
                    }
                }
                if within.is_empty() {
                    for (change, reason) in reasons {
                        ignored.push(IgnoredTag { project_id: root.id.clone(), scenario_id: scenario_id.clone(), change_id: change, reason });
                    }
                } else {
                    set.insert((root.id.clone(), scenario_id), within);
                }
            }
        }
    }
    ignored.sort();
    ignored.dedup();
    (set, ignored)
}

/// Fills the scenario half of `ctx.set_aside` from the `.feature` files and git (module doc) and forgets what
/// `ctx` derived before. Every command that grades requirements calls it once, right after loading the context.
pub fn attach(ctx: &mut GateContext) {
    if ctx.set_aside.closed.is_empty() {
        return;
    }
    let closed: BTreeMap<ChangeId, DateTime<Utc>> = ctx.set_aside.closed.iter().map(|(change, closed)| (change.clone(), closed.at)).collect();
    let (scenarios, ignored) = compute(&ctx.ctx.repo, &closed);
    ctx.set_aside.scenarios = scenarios;
    ctx.set_aside.ignored = ignored;
    ctx.invalidate_derived();
}

/// The scenarios closing `change` at `end` would set aside, besides the ones already set aside: sorted, for a report.
pub fn scenarios_made_by(repo: &Path, ctx: &GateContext, change: &ChangeId, end: DateTime<Utc>) -> Vec<(ProjectId, ScenarioId)> {
    let mut closed: BTreeMap<ChangeId, DateTime<Utc>> = ctx.set_aside.closed.iter().map(|(change, closed)| (change.clone(), closed.at)).collect();
    closed.insert(change.clone(), end);
    let (scenarios, _) = compute(repo, &closed);
    scenarios.into_iter().filter(|(_, made_by)| made_by.contains(change)).map(|(key, _)| key).collect()
}

/// The lines `canon gate check` prints under its advisory `closed_change` and `canon status` repeats: each closed
/// change with its state, reason and the scenarios it set aside, then each closed-change tag that did not count, with
/// the reason. Empty when no change is closed.
pub fn lines(ctx: &GateContext) -> Vec<String> {
    let mut out = Vec::new();
    for (change, closed) in &ctx.set_aside.closed {
        let made = ctx.set_aside.scenarios.values().filter(|by| by.contains(change)).count();
        out.push(format!(
            "`{}` is {} (approver {}, {}): {} — {made} scenario(s) set aside",
            change.as_str(),
            closed.state.as_str(),
            closed.approver,
            closed.at.format("%Y-%m-%dT%H:%M:%SZ"),
            closed.reason
        ));
    }
    for ((project, scenario), by) in &ctx.set_aside.scenarios {
        out.push(format!("scenario `{}` ({}) is set aside: made by closed change {}", scenario.as_str(), project.as_str(), by.iter().map(|c| format!("`{}`", c.as_str())).collect::<Vec<_>>().join(", ")));
    }
    for tag in &ctx.set_aside.ignored {
        out.push(format!(
            "scenario `{}` ({}): the `@flow:{}.*` tag is ignored and the scenario stays required ({}){}",
            tag.scenario_id.as_str(),
            tag.project_id.as_str(),
            tag.change_id.as_str(),
            tag.reason.as_str(),
            if tag.reason == IgnoredReason::HistoryUnavailable { "; this is a shallow clone, so git history is missing: run `git fetch --unshallow`" } else { "" }
        ));
    }
    out
}
