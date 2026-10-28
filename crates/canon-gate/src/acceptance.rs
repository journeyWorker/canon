//! Accepted divergence (0.22 D1): a known gap a reviewer accepted, which
//! the ship guard and `canon gate check` count as covered while it holds.
//!
//! The record is a [`Divergence`] whose status is
//! [`DivergenceStatus::Accepted`]: the same fact stream as `divergence
//! defer` and `resolve` (this scenario departs from its spec, and this is
//! what was decided), written by `canon divergence accept`. It pins the
//! evidence it saw, the way a review does, so it is read here rather than
//! through the divergence fold: the fold orders records by an integrator
//! `run_seq`, and an acceptance must not depend on another role's counter.
//!
//! An acceptance holds for a scenario while ALL of these are true:
//! - it is the scenario's latest acceptance (by record time);
//! - the scenario's latest evidence still says what it pinned (its verdict
//!   and the checks it bound, [`crate::freshness`]), so a different
//!   verdict or different checks end it ([`Held::Stale`]);
//! - it has not reached its `expiry` ([`Held::Expired`]).
//!
//! Anything else leaves the divergent verdict blocking, with a detail
//! naming why. A cell the acceptance covers is never silent:
//! [`accepted_divergences`] lists it for `status` and `gate check`.

use canon_model::records::{Divergence, DivergenceStatus};
use std::collections::BTreeSet;

use canon_model::{EvidenceVerdict, ProjectId, Scenario, ScenarioId, SessionId};
use chrono::{DateTime, Utc};

use crate::context::GateContext;
use crate::ledger::{latest_verdicts, verdicts_of};
use crate::freshness::{current_checks, evidence_current};
use crate::spec_coverage::latest_scenarios;

/// The text a divergent-verdict violation carries after its class and
/// subject, up to the actor: callers that need to recognise the refusal
/// (to attach [`hint`]) match on it.
pub const DIVERGENT_DETAIL: &str = "latest verdict is divergent";

/// One recorded acceptance of a divergent scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acceptance {
    pub project_id: ProjectId,
    pub scenario_id: ScenarioId,
    pub tracking: String,
    pub reason: String,
    pub expiry: Option<DateTime<Utc>>,
    /// Who accepted it (`Divergence.reviewer`).
    pub reviewer: String,
}

impl Acceptance {
    /// `accepted-divergence <id> — <tracking> (expires <date>)`, the line
    /// `canon status` and `canon gate check` list.
    pub fn line(&self) -> String {
        let expires = match self.expiry {
            Some(expiry) => format!("expires {}", expiry.format("%Y-%m-%d")),
            None => "no expiry".to_string(),
        };
        format!("accepted-divergence {} — {} ({expires})", self.scenario_id.as_str(), self.tracking)
    }
}

/// Whether the latest acceptance of a scenario still holds, and if not why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    Holds(Acceptance),
    /// The scenario's latest evidence says something else than it did at the acceptance.
    Stale(Acceptance),
    /// The acceptance's `expiry` has passed.
    Expired(Acceptance),
}

fn acceptance_of(record: &Divergence) -> Option<Acceptance> {
    let DivergenceStatus::Accepted { reason, tracking, expiry, .. } = &record.status else { return None };
    Some(Acceptance {
        project_id: record.project_id.clone(),
        scenario_id: record.scenario_id.clone(),
        tracking: tracking.clone(),
        reason: reason.clone(),
        expiry: *expiry,
        reviewer: record.reviewer.clone(),
    })
}

/// The latest acceptance of `scenario` and whether it holds now. `None`
/// when the scenario was never accepted.
pub fn held(ctx: &GateContext, scenario: &Scenario) -> Option<Held> {
    let latest = ctx.acceptance_fold().get(&(scenario.project_id.clone(), scenario.scenario_id.clone())).map(|&i| &ctx.divergences[i])?;
    let DivergenceStatus::Accepted { checks, .. } = &latest.status else { return None };
    let acceptance = acceptance_of(latest)?;
    if !evidence_current(checks.as_deref(), &current_checks(ctx, scenario)) {
        return Some(Held::Stale(acceptance));
    }
    if acceptance.expiry.is_some_and(|expiry| ctx.now >= expiry) {
        return Some(Held::Expired(acceptance));
    }
    Some(Held::Holds(acceptance))
}

/// Whether `scenario`'s divergent verdict is covered by a holding acceptance.
pub fn is_accepted(ctx: &GateContext, scenario: &Scenario) -> bool {
    matches!(held(ctx, scenario), Some(Held::Holds(_)))
}

/// The detail of a divergent-verdict violation for `scenario`, whose
/// divergent evidence was attested by `by`: [`DIVERGENT_DETAIL`], and when
/// an acceptance exists but does not hold, why.
pub fn divergent_detail(ctx: &GateContext, scenario: &Scenario, by: &str) -> String {
    let base = format!("{DIVERGENT_DETAIL} (by {by})");
    match held(ctx, scenario) {
        Some(Held::Stale(a)) => format!("{base}; the acceptance by {} (tracking {}) is stale — the latest evidence says a different verdict or names different checks than it did", a.reviewer, a.tracking),
        Some(Held::Expired(a)) => format!("{base}; the acceptance by {} (tracking {}) expired {}", a.reviewer, a.tracking, a.expiry.map_or_else(String::new, |e| e.format("%Y-%m-%d").to_string())),
        Some(Held::Holds(_)) | None => base,
    }
}

/// The hint every divergent-verdict refusal carries: fix and re-attest, or
/// accept the known gap. `cells` are `(project_id, scenario_id)` pairs.
pub fn hint(cells: &[(String, String)]) -> String {
    let (project, scenario) = cells.first().cloned().unwrap_or_else(|| ("<project-id>".to_string(), "<scenario-id>".to_string()));
    let more = if cells.len() > 1 { format!(" (name the other {} scenario ids as further operands)", cells.len() - 1) } else { String::new() };
    format!(
        "fix the behavior and re-attest (`canon evidence add --scenario {scenario} --project {project} … --verdict faithful`), or record a known gap a reviewer accepts: `canon divergence accept {scenario} --project {project} --reason <text> --tracking <issue-url-or-ref> [--expiry YYYY-MM-DD] --actor <id> --role reviewer`{more}; acceptance is a reviewer step from a session other than the evidence's, and a changed verdict or check, or its expiry, ends it"
    )
}

/// Every accepted divergence that holds right now: a live scenario whose
/// latest verdict (for some role) is divergent and whose acceptance holds,
/// sorted by scenario id. `status` and `gate check` list these.
pub fn accepted_divergences(ctx: &GateContext) -> Vec<Acceptance> {
    let verdicts = latest_verdicts(ctx);
    let mut out: Vec<Acceptance> = latest_scenarios(ctx)
        .into_iter()
        .filter(|s| verdicts_of(verdicts, s.scenario_id.as_str()).any(|(_, entry)| entry.verdict == EvidenceVerdict::Divergent))
        .filter_map(|s| match held(ctx, s)? {
            Held::Holds(acceptance) => Some(acceptance),
            Held::Stale(_) | Held::Expired(_) => None,
        })
        .collect();
    out.sort_by(|a, b| (&a.project_id, &a.scenario_id).cmp(&(&b.project_id, &b.scenario_id)));
    out
}

/// Why `reviewer`, acting as `actor_id` in `session`, cannot accept
/// `scenario`'s divergence now; `None` when they can. Acceptance needs the
/// scenario's latest evidence (for some role) to be divergent, and no
/// evidence record of the scenario by the same actor or from the same
/// session: the reviewer must be independent of whoever attested it.
pub fn refusal(ctx: &GateContext, scenario: &Scenario, reviewer: &str, actor_id: &str, session: Option<&SessionId>) -> Option<String> {
    let id = scenario.scenario_id.as_str();
    let verdicts = latest_verdicts(ctx);
    let latest: Vec<_> = verdicts_of(verdicts, id).map(|(_, entry)| entry).collect();
    if latest.is_empty() {
        return Some(format!("{id} has no evidence: only a divergent verdict can be accepted as a known gap"));
    }
    if !latest.iter().any(|e| e.verdict == EvidenceVerdict::Divergent) {
        let seen = latest.iter().map(|e| serde_json::to_value(e.verdict).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>().join(", ");
        return Some(format!("{id}'s latest evidence is not divergent ({seen}): there is no gap to accept"));
    }
    // Independence holds against EVERY attestation, superseded ones
    // included: the archive is read explicitly (`crate::history`).
    let evidence = crate::history::scenario_history(ctx, scenario);
    let actors: BTreeSet<&str> = evidence.iter().map(|e| e.envelope.actor.agent_id.as_str()).collect();
    if let Some(same) = [reviewer, actor_id].into_iter().find(|who| actors.contains(who)) {
        return Some(format!("`{same}` attested {id}'s evidence; acceptance is an independent reviewer's step, by an actor other than the evidence's"));
    }
    let sessions: BTreeSet<&str> = evidence.iter().filter_map(|e| e.envelope.actor.session_id.as_ref()).map(SessionId::as_str).collect();
    if let Some(same) = session.filter(|s| sessions.contains(s.as_str())) {
        return Some(format!("session `{}` attested {id}'s evidence; acceptance is an independent reviewer's step, from a session other than the evidence's", same.as_str()));
    }
    None
}

/// The `evidence_sha` of `scenario`'s newest evidence record that carries
/// one, for the acceptance's `sha` field. The newest record that carries
/// one need not be the latest record, so this reads the archive too
/// (`crate::history`).
pub fn evidence_sha(ctx: &GateContext, scenario: &Scenario) -> Option<canon_model::Sha> {
    crate::history::scenario_history(ctx, scenario)
        .into_iter()
        .filter(|e| e.evidence_sha.is_some())
        .max_by_key(|e| e.envelope.at)
        .and_then(|e| e.evidence_sha.clone())
}
