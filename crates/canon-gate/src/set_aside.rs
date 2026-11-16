//! What a closed change sets aside (0.42 `change-drop`).
//!
//! A human closes a change with `canon change defer` or `canon change drop`
//! (a [`ChangeState`] record); a closed change reads as if it were not in the
//! change list of any subject, and the scenarios only it made are set aside.
//! Every reader that asks "what does this subject still require" goes through
//! the folds of [`GateContext`], which consult [`SetAside`], so `canon gate
//! check`, `canon status`, `canon subject move` and the staleness, verdict and
//! divergence checks answer the same question the same way.
//!
//! # Two halves
//! - [`SetAside::closed`] comes from the `change_state` records alone
//!   ([`SetAside::from_states`], filled by [`GateContext::load`]): a change's
//!   current state is its latest record's, and it is closed when that is
//!   `deferred` or `dropped`. No git and no `.feature` file is read for it, so
//!   a change-level requirement (a chore's evidence after approval, its change
//!   review) goes away in every clone.
//! - [`SetAside::scenarios`] and [`SetAside::ignored`] need what the ledger
//!   does not hold: the flow tags (`@flow:<change>.<F-id>`) and the creation
//!   time of each scenario, both in the `.feature` files, and the time the
//!   change's brief was first committed. `canon-cli` reads those and fills the
//!   two fields (`set_aside::attach`) before any derived value is read; a
//!   context without them sets nothing aside at scenario level.
//!
//! The set is computed on every read and stored nowhere, so reopening a change
//! has nothing to delete.

use std::collections::BTreeMap;

use canon_model::{ApprovalRelay, ChangeDecision, ChangeId, ChangeState, ProjectId, ScenarioId};
use chrono::{DateTime, Utc};

/// A change whose latest decision closes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedChange {
    pub change_id: ChangeId,
    /// `Deferred` or `Dropped`, never `Reopened`.
    pub state: ChangeDecision,
    pub reason: String,
    pub refs: Vec<String>,
    pub approver: String,
    /// When the decision was recorded: the end of the change's window.
    pub at: DateTime<Utc>,
    pub relay: Option<ApprovalRelay>,
}

/// Why a closed change's tag on a scenario did not set the scenario aside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IgnoredReason {
    /// The scenario's `.feature` file carries no readable creation time.
    NoCreationTime,
    /// The creation time is outside the closed change's window.
    OutsideWindow,
    /// Git could not say when the change's brief was first committed (a shallow clone).
    HistoryUnavailable,
}

impl IgnoredReason {
    /// The spelling the reports use.
    pub const fn as_str(self) -> &'static str {
        match self {
            IgnoredReason::NoCreationTime => "no_creation_time",
            IgnoredReason::OutsideWindow => "outside_window",
            IgnoredReason::HistoryUnavailable => "history_unavailable",
        }
    }
}

/// A closed change's `@flow:` tag that left its scenario required.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IgnoredTag {
    pub project_id: ProjectId,
    pub scenario_id: ScenarioId,
    pub change_id: ChangeId,
    pub reason: IgnoredReason,
}

/// What the closed changes take out of the requirements (module doc).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetAside {
    /// The changes whose latest decision closes them.
    pub closed: BTreeMap<ChangeId, ClosedChange>,
    /// The scenarios only a closed change made, with the closed changes that made them.
    pub scenarios: BTreeMap<(ProjectId, ScenarioId), Vec<ChangeId>>,
    /// The closed-change tags that did not set their scenario aside.
    pub ignored: Vec<IgnoredTag>,
}

impl SetAside {
    /// The closed changes of `states`: the latest record of each change, when it is `deferred` or `dropped`.
    pub fn from_states(states: &[ChangeState]) -> Self {
        let closed = crate::spec_coverage::latest_by_key(states, |state| state.change_id.clone())
            .into_iter()
            .filter(|state| state.state.is_closed())
            .map(|state| {
                (
                    state.change_id.clone(),
                    ClosedChange {
                        change_id: state.change_id.clone(),
                        state: state.state,
                        reason: state.reason.clone(),
                        refs: state.refs.clone(),
                        approver: state.approver.clone(),
                        at: state.envelope.at,
                        relay: state.relay.clone(),
                    },
                )
            })
            .collect();
        Self { closed, scenarios: BTreeMap::new(), ignored: Vec::new() }
    }

    /// Whether `change` is deferred or dropped now.
    pub fn is_closed(&self, change: &ChangeId) -> bool {
        self.closed.contains_key(change)
    }

    /// Whether scenario `(project, scenario)` is set aside.
    pub fn is_set_aside(&self, project: &ProjectId, scenario: &ScenarioId) -> bool {
        !self.scenarios.is_empty() && self.scenarios.contains_key(&(project.clone(), scenario.clone()))
    }

    /// Whether nothing is closed: every read is as it was before `change_state`.
    pub fn is_empty(&self) -> bool {
        self.closed.is_empty()
    }

    /// Whether any closed change's window could not be read, so scenarios were left required for want of history.
    pub fn history_unavailable(&self) -> bool {
        self.ignored.iter().any(|tag| tag.reason == IgnoredReason::HistoryUnavailable)
    }
}

/// The latest [`ChangeState`] of each change, in change order: the current state of every change that was ever decided.
pub fn latest_states(states: &[ChangeState]) -> Vec<&ChangeState> {
    crate::spec_coverage::latest_by_key(states, |state| state.change_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use canon_model::{Actor, Envelope, RecordKind, RoleId};

    fn state(change: &str, state: ChangeDecision, offset: i64) -> ChangeState {
        let at = DateTime::UNIX_EPOCH + chrono::Duration::seconds(1_800_000_000 + offset);
        let envelope = Envelope::current(RecordKind::ChangeState, at, Actor::new("someone", RoleId::parse("planner").unwrap()));
        ChangeState::new(envelope, ChangeId::parse(change).unwrap(), state, "why", vec!["ref".to_string()], "journey")
    }

    #[test]
    fn the_latest_record_of_a_change_decides_whether_it_is_closed() {
        let states = vec![
            state("a", ChangeDecision::Deferred, 0),
            state("a", ChangeDecision::Reopened, 10),
            state("b", ChangeDecision::Deferred, 0),
            state("b", ChangeDecision::Dropped, 10),
            state("c", ChangeDecision::Reopened, 0),
        ];
        let set = SetAside::from_states(&states);
        assert!(!set.is_closed(&ChangeId::parse("a").unwrap()), "a reopened change is open");
        assert_eq!(set.closed.get(&ChangeId::parse("b").unwrap()).map(|c| c.state), Some(ChangeDecision::Dropped));
        assert!(!set.is_closed(&ChangeId::parse("c").unwrap()));
        assert!(!set.is_closed(&ChangeId::parse("never-decided").unwrap()));
        assert_eq!(set.closed.len(), 1);
    }

    #[test]
    fn nothing_is_set_aside_until_a_scenario_is_listed() {
        let set = SetAside::from_states(&[state("a", ChangeDecision::Deferred, 0)]);
        let (project, scenario) = (ProjectId::parse("root").unwrap(), ScenarioId::parse("x.y.01").unwrap());
        assert!(!set.is_set_aside(&project, &scenario));
    }
}
