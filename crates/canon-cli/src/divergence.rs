//! `canon divergence {add,resolve,defer,accept,list}` (s15 P3b,
//! native-verdict-lifecycle spec, design D10): the ONLY producer of
//! `canon_model::Divergence` records — without it, `canon-gate`'s
//! divergence fold (`canon_model::fold_to_current_state`) has no
//! source of `Divergence` records at all, leaving the S9 divergence
//! burn-down permanently empty.
//!
//! # One staging shape (0.23)
//! Every subcommand stages the full `Divergence` record under
//! `_staging/kind=divergence/` with `run_seq` unassigned
//! ([`canon_gate::promote::UNASSIGNED_RUN_SEQ`]), like `evidence add` and
//! `finding add`; promotion assigns the monotonic `run_seq` within the
//! `(project_id, role, surface)` partition.
//! - `add` stages a candidate of ANY status. With `--promote` (or `write:
//!   { promote: auto }`) the same run promotes that one record;
//!   `--no-promote` keeps it staged.
//! - `canon gate promote --kind divergence` promotes every currently-staged
//!   divergence (`canon_gate::promote_only`).
//! - `resolve`/`defer`/`accept` stage their record and promote it in the
//!   same run: they have no staged-only mode, because the gate must see the
//!   resolution or acceptance the moment the command succeeds.

use std::path::Path;

use canon_gate::promote::UNASSIGNED_RUN_SEQ;
use canon_gate::GateCtx;
use canon_model::{Actor, Divergence, DivergenceStatus, Envelope, ProjectId, RecordKind, RoleId, ScenarioId, SessionId, Sha, TotalOrder};
use canon_store::tier::Tier;
use canon_policy::SchemaRegistry;
use canon_store::git_tier::GitTier;
use canon_model::ids::JoinKeyError;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Serialize;

pub use crate::review::{parse_project_id, parse_scenario_id};
use crate::cli_error::CliError;
use crate::context::resolve_repo_root;
use crate::timestamp::TimestampError;

/// `--sha`'s `clap` value parser.
pub fn parse_sha(s: &str) -> Result<Sha, JoinKeyError> {
    Sha::parse(s)
}

/// `--expiry`'s and `--as-of`'s `clap` value parser: the shared `<TIMESTAMP>`
/// grammar (RFC3339 or `YYYY-MM-DD`).
pub fn parse_timestamp(s: &str) -> Result<DateTime<Utc>, TimestampError> {
    crate::timestamp::parse(s)
}

/// `canon divergence add --state <state>` refused.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum DivergenceStateError {
    /// `--state deferred` without `--reason`.
    #[error("`--state deferred` requires `--reason`")]
    DeferredNeedsReason,
    /// `--state deferred` without `--expiry`.
    #[error("`--state deferred` requires `--expiry`")]
    DeferredNeedsExpiry,
    /// A state that is none of the four.
    #[error("unknown --state `{0}` (expected one of: open, still-divergent, resolved, deferred)")]
    Unknown(String),
}

/// `canon divergence add --state <state>`'s state-string ->
/// `DivergenceStatus` builder (module doc: `stage` accepts ANY
/// status, not only `Open`). `--reason`/`--expiry` are REQUIRED
/// together only when `status == "deferred"` — an `Err` here is a
/// usage refusal, never a synthesized reason/expiry.
pub fn parse_status(status: &str, reason: Option<&str>, expiry: Option<DateTime<Utc>>) -> Result<DivergenceStatus, DivergenceStateError> {
    match status {
        "open" => Ok(DivergenceStatus::Open),
        "still-divergent" => Ok(DivergenceStatus::StillDivergent),
        "resolved" => Ok(DivergenceStatus::Resolved),
        "deferred" => {
            let reason = reason.ok_or(DivergenceStateError::DeferredNeedsReason)?;
            let expiry = expiry.ok_or(DivergenceStateError::DeferredNeedsExpiry)?;
            Ok(DivergenceStatus::Deferred { reason: reason.to_string(), expiry })
        }
        other => Err(DivergenceStateError::Unknown(other.to_string())),
    }
}

/// `canon divergence add`: where the divergence landed.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DivergenceAddReport {
    #[serde(flatten)]
    pub landing: crate::retry::Landing,
}

impl crate::reports::Report for DivergenceAddReport {}

/// `canon divergence defer` and `canon divergence resolve`: the record committed.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DivergenceCommitReport {
    /// The ledger path of the committed record.
    pub committed: String,
}

impl crate::reports::Report for DivergenceCommitReport {}

/// One scenario `canon divergence accept` accepted.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedScenario {
    pub scenario_id: String,
    /// The ledger path of the acceptance record.
    pub location: String,
}

/// `canon divergence accept`: the acceptances written (all or none).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DivergenceAcceptReport<'a> {
    pub tracking: &'a str,
    /// The date the acceptance lapses (`YYYY-MM-DD`); `null` without `--expiry`.
    pub expiry: Option<String>,
    pub accepted: Vec<AcceptedScenario>,
}

impl crate::reports::Report for DivergenceAcceptReport<'_> {}

/// Per-state counts of `canon divergence list`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BurndownSummary {
    pub open: usize,
    pub resolved: usize,
    pub still_divergent: usize,
    pub deferred: usize,
    pub resolved_invalid: usize,
    pub accepted: usize,
    pub total: usize,
}

/// One scenario's current state in `canon divergence list`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioRow<'a> {
    pub project_id: &'a str,
    pub scenario_id: &'a str,
    /// `open`, `resolved`, `still-divergent`, `deferred`, `resolved-invalid` or `accepted`.
    pub state: &'static str,
    /// Why it is deferred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'a str>,
    /// When a deferral or acceptance lapses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<String>,
    /// Where an accepted gap is tracked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracking: Option<&'a str>,
}

/// `canon divergence list`: the current state of every divergence; paging is in `meta`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DivergenceListReport<'a> {
    /// The instant deferred expiry was judged at (RFC 3339).
    pub as_of: String,
    pub summary: BurndownSummary,
    pub scenarios: Vec<ScenarioRow<'a>>,
}

impl crate::reports::Report for DivergenceListReport<'_> {}

/// `canon divergence list --fields`: each scenario keeps only the requested members.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DivergenceListFieldsReport {
    pub as_of: String,
    pub summary: BurndownSummary,
    pub scenarios: Vec<crate::json_out::Projected<ScenarioRow<'static>>>,
}

impl crate::reports::Report for DivergenceListFieldsReport {}

/// `canon divergence add` (module doc): writes ONE unordered staging
/// candidate — `run_seq` is assigned later, at `promote` time. With
/// `--promote` or `write: { promote: auto }` (0.16 D2) the same run then
/// promotes that one candidate (`canon_gate::promote_staged_divergence`)
/// and leaves every other staged candidate for `canon divergence
/// promote` ([`crate::write_mode`]).
#[expect(clippy::too_many_arguments, reason = "the signature is the `divergence add` flag set, passed one-to-one from `run/divergence.rs`")]
pub fn run_stage(
    repo: &Path,
    project_id: &canon_model::ProjectId,
    scenario_id: &canon_model::ScenarioId,
    commit: &crate::git_rev::Bound,
    status: DivergenceStatus,
    round: u32,
    reviewer: &str,
    detail: &str,
    actor_id: &str,
    role: &RoleId,
    session: Option<&SessionId>,
    promote: crate::write_mode::PromoteFlag,
    json: bool,
) -> u8 {
    let repo = resolve_repo_root(repo);
    if let Err(error) = crate::inventory::require_spec_root(&repo, project_id) {
        return error.emit("canon divergence add", json);
    }
    let promote = match crate::write_mode::should_promote(&repo, promote) {
        Ok(promote) => promote,
        Err(error) => {
            return CliError::refused("CONFIG_UNREADABLE", error.to_string()).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again").emit("canon divergence add", json);
        }
    };
    let gate_ctx = match GateCtx::from_repo(&repo) {
        Ok(ctx) => ctx,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again").emit("canon divergence add", json);
        }
    };
    let candidate = candidate(CandidateSpec { project_id, scenario_id, sha: &commit.sha, code: Some((&commit.code_tree, commit.worktree_dirty)), status, round, reviewer, detail, actor: actor_in(actor_id, role, session), at: Utc::now() });

    // The scenario's newest divergence already says exactly this: a repeat
    // writes nothing and reports the record that is there.
    let body = serde_json::to_value(&candidate).expect("a Divergence always serializes");
    let same_scenario = |stored: &serde_json::Value| ["project_id", "scenario_id"].iter().all(|key| stored.get(key) == body.get(key));
    if let Some(existing) = crate::retry::repeats_latest(&gate_ctx.ledger_root, Some(&crate::gate::evidence_staging_dir(&gate_ctx.ledger_root)), RecordKind::Divergence.as_str(), &body, same_scenario, None) {
        crate::say!(json, "canon divergence add: unchanged — an identical divergence for {} is already {}; nothing written", scenario_id.as_str(), existing.describe());
        crate::json_out::mark_unchanged();
        if json {
            crate::json_out::data("divergence add", &DivergenceAddReport { landing: existing.landing() });
        }
        return 0;
    }
    let location = match stage(&gate_ctx.ledger_root, &candidate) {
        Ok(location) => location,
        Err(e) => {
            return CliError::refused("WRITE_FAILED", e.to_string()).hint("check that the path is writable and the disk has room, then run the command again").emit("canon divergence add", json);
        }
    };
    let outcome = crate::gate::promote_after_stage("canon divergence add", &gate_ctx.ledger_root, &location, promote, json);
    crate::say!(json, "canon divergence add: {} — {}", outcome.landed(&location), outcome.suffix());
    if json {
        crate::json_out::data("divergence add", &DivergenceAddReport { landing: outcome.landing(&location) });
    }
    outcome.exit_code()
}

/// What [`candidate`] builds a staged divergence from.
struct CandidateSpec<'a> {
    project_id: &'a ProjectId,
    scenario_id: &'a ScenarioId,
    sha: &'a Sha,
    /// `(code tree, worktree dirty)` of the commit, when the verb binds one.
    code: Option<(&'a str, bool)>,
    status: DivergenceStatus,
    round: u32,
    reviewer: &'a str,
    detail: &'a str,
    actor: Actor,
    at: DateTime<Utc>,
}

/// The staged shape of one divergence: the full record, `run_seq` not yet
/// assigned.
fn candidate(spec: CandidateSpec<'_>) -> Divergence {
    let CandidateSpec { project_id, scenario_id, sha, code, status, round, reviewer, detail, actor, at } = spec;
    let record = Divergence::new(
        Envelope::new(1, RecordKind::Divergence, at, actor),
        project_id.clone(),
        scenario_id.clone(),
        sha.clone(),
        status,
        TotalOrder::new(UNASSIGNED_RUN_SEQ),
        round,
        reviewer,
        detail,
    );
    match code {
        Some((code_tree, dirty)) => record.with_code(code_tree, dirty),
        None => record,
    }
}

/// Stage `candidate` in the ledger's one staging directory; the location is
/// relative to it, as every staged write reports.
fn stage(ledger_root: &Path, candidate: &Divergence) -> Result<String, canon_store::tier::StoreError> {
    Ok(GitTier::new(crate::gate::evidence_staging_dir(ledger_root)).write(candidate)?.location)
}

/// Stage `candidate` and promote exactly that record: the path `resolve`,
/// `defer` and `accept` take. On refusal the staged file is removed, so a
/// refused command leaves nothing behind.
fn stage_and_promote(ledger_root: &Path, candidate: &Divergence) -> Result<std::path::PathBuf, String> {
    let location = stage(ledger_root, candidate).map_err(|e| e.to_string())?;
    match crate::gate::try_promote(ledger_root, &location) {
        Ok(target) => Ok(std::path::PathBuf::from(target)),
        Err(refusal) => {
            let _ = std::fs::remove_file(crate::gate::evidence_staging_dir(ledger_root).join(&location));
            Err(refusal.data.and_then(|data| data.refused.into_iter().next().map(|refused| refused.message)).unwrap_or_else(|| refusal.error.message.clone()))
        }
    }
}

/// The inputs of [`run_commit`].
struct CommitArgs<'a> {
    repo: &'a Path,
    project_id: &'a canon_model::ProjectId,
    scenario_id: &'a canon_model::ScenarioId,
    commit: &'a crate::git_rev::Bound,
    status: DivergenceStatus,
    round: u32,
    reviewer: &'a str,
    detail: &'a str,
    actor_id: &'a str,
    role: &'a RoleId,
    session: Option<&'a SessionId>,
    json: bool,
}

/// Shared `resolve`/`defer` path (module doc): build one
/// divergence with `status`, stage it and promote that one record.
fn run_commit(verb: &str, args: CommitArgs<'_>) -> u8 {
    let CommitArgs { repo, project_id, scenario_id, commit, status, round, reviewer, detail, actor_id, role, session, json } = args;
    let repo = resolve_repo_root(repo);
    if let Err(error) = crate::inventory::require_spec_root(&repo, project_id) {
        return error.emit(&format!("canon divergence {verb}"), json);
    }
    let gate_ctx = match GateCtx::from_repo(&repo) {
        Ok(ctx) => ctx,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again").emit(&format!("canon divergence {verb}"), json);
        }
    };
    let candidate = candidate(CandidateSpec { project_id, scenario_id, sha: &commit.sha, code: Some((&commit.code_tree, commit.worktree_dirty)), status, round, reviewer, detail, actor: actor_in(actor_id, role, session), at: Utc::now() });

    match stage_and_promote(&gate_ctx.ledger_root, &candidate) {
        Ok(target) => {
            crate::say!(json, "canon divergence {verb}: committed {} — {}", target.display(), crate::write_mode::PROMOTED);
            if json {
                crate::json_out::data(&format!("divergence {verb}"), &DivergenceCommitReport { committed: target.display().to_string() });
            }
            0
        }
        Err(message) => {
            CliError::failed("DIVERGENCE_REFUSED", message).hint("`canon divergence list` lists the scenario's divergences and their state; fix what the message names, then run the command again").emit(&format!("canon divergence {verb}"), json)
        }
    }
}

/// The actor of a divergence record: `actor_id` in `role`, in `session` when `--session-id` named one.
fn actor_in(actor_id: &str, role: &RoleId, session: Option<&SessionId>) -> Actor {
    let actor = Actor::new(actor_id, role.clone());
    match session {
        Some(session) => actor.with_session(session.clone()),
        None => actor,
    }
}

/// `canon divergence resolve` (module doc): stages and promotes a
/// `Resolved` record.
#[expect(clippy::too_many_arguments, reason = "the signature is the `divergence resolve` flag set, passed one-to-one from `run/divergence.rs`")]
pub fn run_resolve(
    repo: &Path,
    project_id: &canon_model::ProjectId,
    scenario_id: &canon_model::ScenarioId,
    commit: &crate::git_rev::Bound,
    round: u32,
    reviewer: &str,
    detail: &str,
    actor_id: &str,
    role: &RoleId,
    session: Option<&SessionId>,
    json: bool,
) -> u8 {
    run_commit("resolve", CommitArgs { repo, project_id, scenario_id, commit, status: DivergenceStatus::Resolved, round, reviewer, detail, actor_id, role, session, json })
}

/// `canon divergence defer --reason <..> --expiry <..>` (module doc):
/// stages and promotes a `Deferred { reason, expiry }` record.
#[expect(clippy::too_many_arguments, reason = "the signature is the `divergence defer` flag set, passed one-to-one from `run/divergence.rs`")]
pub fn run_defer(
    repo: &Path,
    project_id: &canon_model::ProjectId,
    scenario_id: &canon_model::ScenarioId,
    commit: &crate::git_rev::Bound,
    round: u32,
    reviewer: &str,
    reason: &str,
    expiry: DateTime<Utc>,
    actor_id: &str,
    role: &RoleId,
    session: Option<&SessionId>,
    json: bool,
) -> u8 {
    run_commit("defer", CommitArgs { repo, project_id, scenario_id, commit, status: DivergenceStatus::Deferred { reason: reason.to_string(), expiry }, round, reviewer, detail: "", actor_id, role, session, json })
}

/// `canon divergence accept`'s arguments.
#[derive(Debug)]
pub struct AcceptArgs {
    pub project_id: ProjectId,
    /// `--scenario-id`, repeatable: one acceptance per scenario, all or nothing.
    pub scenario_ids: Vec<ScenarioId>,
    pub reason: String,
    /// Where the gap is tracked (an issue URL or reference).
    pub tracking: String,
    pub expiry: Option<DateTime<Utc>>,
    pub reviewer: String,
    pub actor_id: String,
    pub role: RoleId,
    pub session_id: Option<SessionId>,
    pub json: bool,
}

/// `canon divergence accept` (0.22 D1, [`canon_gate::acceptance`]): a
/// reviewer accepts each named scenario's `divergent` latest evidence as a
/// tracked known gap. Each acceptance pins the evidence it saw, so new
/// evidence ends it, and lapses at `--expiry`. Refused, with nothing
/// written for any scenario: a role other than `reviewer`; an empty
/// `--reason` or `--tracking`; a scenario with no record; evidence that is
/// not divergent; the evidence's own actor or session. Returns the exit
/// code: `0` on success, `2` refused.
pub fn run_accept(repo: &Path, args: &AcceptArgs) -> u8 {
    const CMD: &str = "canon divergence accept";
    let invalid = |message: String, field: &str| {
        CliError::refused("DIVERGENCE_INVALID_ARGUMENT", message)
            .field(field)
            .hint(format!("give {field} a value and run `canon divergence accept` again; `canon schema divergence accept --json` lists every flag"))
            .emit(CMD, args.json)
    };
    if args.role.as_str() != "reviewer" {
        return CliError::refused("DIVERGENCE_ACCEPT_REFUSED", format!("accepting a divergence is a reviewer step, not `{}`", args.role.as_str()))
            .field("--role")
            .hint("pass --role reviewer (or declare --as reviewer)")
            .emit(CMD, args.json);
    }
    if args.tracking.trim().is_empty() {
        return invalid("--tracking is empty: name where the gap is tracked (an issue URL or reference)".to_string(), "--tracking");
    }
    if args.reason.trim().is_empty() {
        return invalid("--reason is empty: say why the gap is accepted".to_string(), "--reason");
    }
    if args.scenario_ids.is_empty() {
        return invalid("no scenario to accept: give --scenario-id <id> (repeatable)".to_string(), "--scenario-id");
    }
    let mut scenario_ids: Vec<&ScenarioId> = Vec::new();
    for id in &args.scenario_ids {
        if !scenario_ids.contains(&id) {
            scenario_ids.push(id);
        }
    }

    let repo = resolve_repo_root(repo);
    if let Err(error) = crate::inventory::require_spec_root(&repo, &args.project_id) {
        return error.emit(CMD, args.json);
    }
    let context = match GateCtx::from_repo(&repo).map_err(|e| e.to_string()).and_then(|ctx| canon_gate::GateContext::load(ctx, &SchemaRegistry::load(), Utc::now()).map_err(|e| e.to_string())) {
        Ok(context) => context,
        Err(e) => return CliError::refused("CONFIG_UNREADABLE", e).hint("fix canon.yaml (`canon init --check` names the problem), then run `canon divergence accept` again").emit(CMD, args.json),
    };
    let indexed = canon_gate::spec_coverage::latest_scenarios(&context);
    let mut scenarios = Vec::new();
    let mut missing = Vec::new();
    for id in &scenario_ids {
        match indexed.iter().find(|s| s.project_id == args.project_id && &s.scenario_id == *id) {
            Some(scenario) => scenarios.push(*scenario),
            None => missing.push(format!("`{}`", id.as_str())),
        }
    }
    if !missing.is_empty() {
        return CliError::refused("DIVERGENCE_INVALID_ARGUMENT", format!("no scenario record for {} in spec root `{}`; an acceptance pins the evidence of a scenario the index knows", missing.join(", "), args.project_id.as_str()))
            .field("--scenario-id")
            .hint("run `canon inventory sync`, then accept again")
            .emit(CMD, args.json);
    }
    let refusals: Vec<String> = scenarios
        .iter()
        .filter_map(|scenario| canon_gate::acceptance::refusal(&context, scenario, &args.reviewer, &args.actor_id, args.session_id.as_ref()))
        .collect();
    if !refusals.is_empty() {
        let nothing = if scenarios.len() > 1 { " (nothing is written for any of the scenarios)" } else { "" };
        return CliError::refused("DIVERGENCE_ACCEPT_REFUSED", format!("{}{nothing}", refusals.join("; ")))
            .hint("an independent reviewer, in a session that did not attest the evidence, accepts a divergent verdict; otherwise fix the behavior and re-attest")
            .emit(CMD, args.json);
    }

    let mut actor = Actor::new(&args.actor_id, args.role.clone());
    if let Some(session) = &args.session_id {
        actor = actor.with_session(session.clone());
    }
    let at = Utc::now();
    let mut wrote: Vec<(String, std::path::PathBuf)> = Vec::new();
    for scenario in &scenarios {
        let record = candidate(CandidateSpec {
            project_id: &scenario.project_id,
            scenario_id: &scenario.scenario_id,
            // The commit the divergent evidence was attested against; an
            // acceptance binds to evidence, not to an app state, so a
            // scenario whose evidence carries no commit gets the null one.
            sha: &canon_gate::acceptance::evidence_sha(&context, scenario).unwrap_or_else(|| Sha::parse("0".repeat(40)).expect("forty hex digits")),
            code: None,
            status: DivergenceStatus::Accepted {
                reason: args.reason.clone(),
                tracking: args.tracking.clone(),
                expiry: args.expiry,
                checks: Some(canon_gate::freshness::current_checks(&context, scenario)),
                evidence: Vec::new(),
            },
            round: 1,
            reviewer: &args.reviewer,
            detail: &args.reason,
            actor: actor.clone(),
            at,
        });
        let failure = match stage_and_promote(&context.ctx.ledger_root, &record) {
            Ok(target) => {
                wrote.push((scenario.scenario_id.as_str().to_string(), target));
                continue;
            }
            Err(message) => message,
        };
        for (_, target) in &wrote {
            let _ = std::fs::remove_file(if target.is_absolute() { target.clone() } else { context.ctx.ledger_root.join(target) });
        }
        if !wrote.is_empty() {
            crate::ledger::restore_after_rollback(&context.ctx.ledger_root, RecordKind::Divergence);
        }
        let error = CliError::refused("WRITE_FAILED", failure).hint(if scenarios.len() > 1 {
            "no acceptance is written (every record this run wrote was removed): check that the ledger directory is writable, then run `canon divergence accept` again"
        } else {
            "check that the ledger directory is writable, then run `canon divergence accept` again"
        });
        return error.emit(CMD, args.json);
    }
    for (scenario, target) in &wrote {
        crate::say!(args.json, "canon divergence accept: {scenario} accepted as a known gap ({}) -> {} — {}", args.tracking, target.display(), crate::write_mode::PROMOTED);
    }
    let accepted: Vec<AcceptedScenario> = wrote.iter().map(|(scenario, target)| AcceptedScenario { scenario_id: scenario.clone(), location: target.display().to_string() }).collect();
    let scope = crate::next::Scope::Scenarios(wrote.iter().map(|(scenario, _)| scenario.clone()).collect());
    crate::next::finish(
        &repo,
        "divergence accept",
        &scope,
        false,
        args.json,
        &DivergenceAcceptReport { tracking: &args.tracking, expiry: args.expiry.map(|e| e.format("%Y-%m-%d").to_string()), accepted });
    0
}

/// `canon divergence list [--as-of <timestamp>]` (task 4.5): the S9
/// divergence burn-down's CURRENT-STATE view, over
/// `canon_report::divergence::{current_states, summarize}` — the
/// consumer of `canon_model::fold_to_current_state` this crate wires
/// up (`canon_report::divergence`'s own module doc contrasts this
/// with `mart_review_burndown`'s per-day TREND). `as_of` defaults to
/// "now"; ALWAYS exits `0` — a read-only capability query over
/// whatever is currently on the ledger, never a gate.
/// The fields of one `divergence list` scenario row, which `--fields` may name.
const ROW_FIELDS: [&str; 6] = ["projectId", "scenarioId", "state", "reason", "expiry", "tracking"];

pub fn run_list(repo: &Path, as_of: Option<DateTime<Utc>>, json: bool, page: &crate::paging::Page, fields: Option<&[String]>) -> u8 {
    if let Some(requested) = fields {
        if let Err(e) = crate::paging::check_known("canon divergence list", requested, &ROW_FIELDS) {
            return e.emit("canon divergence list", json);
        }
    }
    let repo = resolve_repo_root(repo);
    let gate_ctx = match GateCtx::from_repo(&repo) {
        Ok(ctx) => ctx,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again").emit("canon divergence list", json);
        }
    };
    let as_of = as_of.unwrap_or_else(Utc::now);

    let states = canon_report::divergence::current_states(&gate_ctx.ledger_root, as_of);
    let summary = canon_report::divergence::summarize(&states);
    let (rows, meta) = match crate::paging::paginate("divergence list", states.into_iter().collect::<Vec<_>>(), |((project_id, scenario_id), _)| format!("{project_id}/{scenario_id}"), page) {
        Ok(paged) => paged,
        Err(e) => return e.emit("canon divergence list", json),
    };

    if json {
        let scenarios = rows.iter().map(|((project_id, scenario_id), state)| {
            let mut row = ScenarioRow { project_id: project_id.as_str(), scenario_id: scenario_id.as_str(), state: state_slug(state), reason: None, expiry: None, tracking: None };
            match state {
                canon_model::FoldedState::Deferred { reason, expiry } => {
                    row.reason = Some(reason);
                    row.expiry = Some(expiry.to_string());
                }
                canon_model::FoldedState::Accepted { tracking, expiry } => {
                    row.tracking = Some(tracking);
                    row.expiry = expiry.map(|expiry| expiry.to_string());
                }
                _ => {}
            }
            row
        });
        let summary = BurndownSummary {
            open: summary.open,
            resolved: summary.resolved,
            still_divergent: summary.still_divergent,
            deferred: summary.deferred,
            resolved_invalid: summary.resolved_invalid,
            accepted: summary.accepted,
            total: summary.total(),
        };
        let as_of = as_of.to_rfc3339();
        let mut page = serde_json::Map::new();
        meta.insert_into(&mut page);
        let page = serde_json::Value::Object(page);
        match fields {
            Some(requested) => {
                let scenarios = scenarios
                    .map(|row| crate::json_out::Projected::new(crate::paging::project(&serde_json::to_value(row).expect("a scenario row serializes"), requested)))
                    .collect();
                crate::json_out::data_with_meta("divergence list", &DivergenceListFieldsReport { as_of, summary, scenarios }, page);
            }
            None => crate::json_out::data_with_meta("divergence list", &DivergenceListReport { as_of, summary, scenarios: scenarios.collect() }, page),
        }
        return 0;
    }

    println!(
        "canon divergence list (as of {as_of}): open={} resolved={} still-divergent={} deferred={} resolved-invalid={} accepted={} (total={})",
        summary.open,
        summary.resolved,
        summary.still_divergent,
        summary.deferred,
        summary.resolved_invalid,
        summary.accepted,
        summary.total()
    );
    for ((project_id, scenario_id), state) in &rows {
        let state_str = match state {
            canon_model::FoldedState::Deferred { reason, expiry } => format!("deferred({reason}, until {expiry})"),
            canon_model::FoldedState::Accepted { tracking, expiry: Some(expiry) } => format!("accepted({tracking}, until {expiry})"),
            canon_model::FoldedState::Accepted { tracking, expiry: None } => format!("accepted({tracking})"),
            other => state_slug(other).to_string(),
        };
        println!("  {project_id}/{scenario_id}: {state_str}");
    }
    if let Some(line) = meta.human_line() {
        println!("{line}");
    }
    0
}

fn state_slug(state: &canon_model::FoldedState) -> &'static str {
    match state {
        canon_model::FoldedState::Open => "open",
        canon_model::FoldedState::Resolved => "resolved",
        canon_model::FoldedState::StillDivergent => "still-divergent",
        canon_model::FoldedState::Deferred { .. } => "deferred",
        canon_model::FoldedState::ResolvedInvalid => "resolved-invalid",
        canon_model::FoldedState::Accepted { .. } => "accepted",
    }
}

#[cfg(test)]
mod tests {
    use canon_store::tier::TierQuery;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn the_value_parsers_refuse_in_their_old_words() {
        assert_eq!(parse_status("bogus", None, None).unwrap_err().to_string(), "unknown --state `bogus` (expected one of: open, still-divergent, resolved, deferred)");
        assert_eq!(parse_status("deferred", None, None).unwrap_err().to_string(), "`--state deferred` requires `--reason`");
        assert_eq!(parse_status("deferred", Some("why"), None).unwrap_err().to_string(), "`--state deferred` requires `--expiry`");
        assert_eq!(parse_timestamp("tomorrow").unwrap_err().to_string(), "invalid timestamp `tomorrow`: want RFC3339 or YYYY-MM-DD");
        assert!(parse_sha("nope").unwrap_err().to_string().starts_with("invalid Sha: \"nope\" does not match grammar"));
    }

    /// A repo whose `canon.yaml` configures the spec root `app-a`, the
    /// project id these tests write under (`--project-id` must name one).
    fn repo_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "specs:\n  roots:\n    - id: app-a\n      root: specs\n").unwrap();
        dir
    }

    fn gate_ctx_root(dir: &TempDir) -> std::path::PathBuf {
        GateCtx::from_repo(dir.path()).unwrap().ledger_root
    }

    fn ids() -> (ProjectId, ScenarioId, RoleId, crate::git_rev::Bound) {
        (
            ProjectId::parse("app-a").unwrap(),
            ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap(),
            RoleId::parse("reviewer").unwrap(),
            crate::git_rev::Bound { sha: Sha::parse("a".repeat(40)).unwrap(), code_tree: "t".repeat(40), worktree_dirty: false },
        )
    }

    #[test]
    fn stage_then_promote_assigns_a_monotonic_run_seq_and_a_refusal_consumes_none() {
        let dir = repo_dir();
        let (project_id, scenario_id_a, role, sha) = ids();
        let scenario_id_b = ScenarioId::parse("world.firstbuy-hotdeal.27").unwrap();

        assert_eq!(run_stage(dir.path(), &project_id, &scenario_id_a, &sha, DivergenceStatus::Open, 1, "reviewer-1", "", "agent-x", &role, None, Default::default(), false), 0);
        assert_eq!(run_stage(dir.path(), &project_id, &scenario_id_b, &sha, DivergenceStatus::Open, 1, "reviewer-1", "", "agent-x", &role, None, Default::default(), false), 0);

        let code = crate::gate::run_promote(dir.path(), Some(RecordKind::Divergence), false, false);
        assert_eq!(code, 0);

        let gate_ctx = GateCtx::from_repo(dir.path()).unwrap();
        let committed = GitTier::new(&gate_ctx.ledger_root);
        let landed = committed.read(&TierQuery::kind(RecordKind::Divergence)).unwrap();
        assert_eq!(landed.records.len(), 2);
        let mut seqs: Vec<u64> = landed.records.iter().filter_map(|r| r.0.get("run_seq").and_then(serde_json::Value::as_u64)).collect();
        seqs.sort_unstable();
        assert_eq!(seqs, vec![1, 2]);

        // Staging is empty afterward.
        let staging_dir = crate::gate::evidence_staging_dir(&gate_ctx.ledger_root);
        assert!(files_with_bytes(&staging_dir).is_empty(), "no staged file is left");
    }

    #[test]
    fn a_refused_candidate_consumes_no_run_seq() {
        let dir = repo_dir();
        let (project_id, scenario_id, role, sha) = ids();

        // Refused: no `actor.role` at all.
        let malformed = candidate(CandidateSpec {
            project_id: &project_id,
            scenario_id: &ScenarioId::parse("world.firstbuy-hotdeal.90").unwrap(),
            sha: &sha.sha,
            code: None,
            status: DivergenceStatus::Open,
            round: 1,
            reviewer: "reviewer-1",
            detail: "",
            actor: Actor::new_unattributed("legacy-writer"),
            at: Utc::now(),
        });
        stage(&gate_ctx_root(&dir), &malformed).unwrap();

        assert_eq!(run_stage(dir.path(), &project_id, &scenario_id, &sha, DivergenceStatus::Open, 1, "reviewer-1", "", "agent-x", &role, None, Default::default(), false), 0);

        let code = crate::gate::run_promote(dir.path(), Some(RecordKind::Divergence), false, false);
        assert_eq!(code, 1, "a refusal reports a non-zero (but not usage-error) exit code");

        let gate_ctx = GateCtx::from_repo(dir.path()).unwrap();
        let committed = GitTier::new(&gate_ctx.ledger_root);
        let landed = committed.read(&TierQuery::kind(RecordKind::Divergence)).unwrap();
        assert_eq!(landed.records.len(), 1);
        let run_seq = landed.records[0].0.get("run_seq").and_then(serde_json::Value::as_u64).unwrap();
        assert_eq!(run_seq, 1, "the refused candidate must not have consumed run_seq 1");
    }

    /// Every file under `dir`, with its bytes, in path order.
    fn files_with_bytes(dir: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
        fn walk(dir: &Path, out: &mut Vec<(std::path::PathBuf, Vec<u8>)>) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else {
                    let bytes = std::fs::read(&path).unwrap();
                    out.push((path, bytes));
                }
            }
        }
        let mut out = Vec::new();
        walk(dir, &mut out);
        out.sort();
        out
    }

    #[test]
    fn resolve_and_defer_promote_only_their_own_record_and_leave_other_staged_work() {
        let dir = repo_dir();
        let (project_id, scenario_id, role, sha) = ids();
        let deferred_id = ScenarioId::parse("world.firstbuy-hotdeal.51").unwrap();

        // Something else is mid-`stage`.
        assert_eq!(run_stage(dir.path(), &project_id, &ScenarioId::parse("world.firstbuy-hotdeal.50").unwrap(), &sha, DivergenceStatus::Open, 1, "reviewer-1", "", "agent-x", &role, None, Default::default(), false), 0);
        let gate_ctx = GateCtx::from_repo(dir.path()).unwrap();
        let staging_dir = crate::gate::evidence_staging_dir(&gate_ctx.ledger_root);
        let staged_before = files_with_bytes(&staging_dir);
        assert_eq!(staged_before.len(), 1, "exactly the one unrelated candidate is staged");

        assert_eq!(run_resolve(dir.path(), &project_id, &scenario_id, &sha, 1, "reviewer-1", "", "agent-x", &role, None, false), 0);
        // Whole seconds so the persisted expiry compares exactly.
        let expiry = chrono::DateTime::from_timestamp((Utc::now() + chrono::Duration::days(7)).timestamp(), 0).unwrap();
        assert_eq!(run_defer(dir.path(), &project_id, &deferred_id, &sha, 1, "reviewer-1", "needs another look", expiry, "agent-x", &role, None, false), 0);

        let committed = GitTier::new(&gate_ctx.ledger_root);
        let landed = committed.read(&TierQuery::kind(RecordKind::Divergence)).unwrap();
        let mut records: Vec<canon_model::records::Divergence> =
            landed.records.into_iter().map(|record| serde_json::from_value(record.0).expect("a committed divergence")).collect();
        records.sort_by(|a, b| a.scenario_id.as_str().cmp(b.scenario_id.as_str()));
        assert_eq!(records.len(), 2, "resolve + defer both committed in their own run");
        assert_eq!((&records[0].project_id, &records[0].scenario_id, &records[0].status), (&project_id, &scenario_id, &DivergenceStatus::Resolved));
        assert_eq!(
            (&records[1].project_id, &records[1].scenario_id, &records[1].status),
            (&project_id, &deferred_id, &DivergenceStatus::Deferred { reason: "needs another look".to_string(), expiry })
        );

        // The unrelated staged candidate is still sitting there, byte-identical.
        assert_eq!(files_with_bytes(&staging_dir), staged_before, "resolve and defer must never touch another record's staged work");
    }

    #[test]
    fn status_reflects_a_resolved_divergence() {
        let dir = repo_dir();
        let (project_id, scenario_id, role, sha) = ids();

        assert_eq!(run_resolve(dir.path(), &project_id, &scenario_id, &sha, 1, "reviewer-1", "", "agent-x", &role, None, false), 0);

        // Read-only capability query — always exits 0.
        assert_eq!(run_list(dir.path(), None, false, &crate::paging::Page::first_page(), None), 0);

        let gate_ctx = GateCtx::from_repo(dir.path()).unwrap();
        let states = canon_report::divergence::current_states(&gate_ctx.ledger_root, Utc::now());
        assert_eq!(states.get(&(project_id, scenario_id)), Some(&canon_model::FoldedState::Resolved));
    }

    /// divergence.lifecycle.08 against a real ledger: a committed
    /// resolution against sha `a` reads back as resolved-invalid once the
    /// scenario's live evidence binding names sha `b`, and neither
    /// `canon divergence list` nor the fold rewrites the persisted
    /// resolved record — the downgrade is derived at read time.
    #[test]
    fn a_moved_live_binding_reads_resolved_invalid_without_rewriting_the_resolution() {
        let dir = repo_dir();
        let (project_id, scenario_id, role, sha) = ids();
        assert_eq!(run_resolve(dir.path(), &project_id, &scenario_id, &sha, 1, "reviewer-1", "", "agent-x", &role, None, false), 0);

        let gate_ctx = GateCtx::from_repo(dir.path()).unwrap();
        let moved = Sha::parse("b".repeat(40)).unwrap();
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("agent-x", RoleId::parse("implementer").unwrap()));
        let binding = canon_model::EvidenceRecord::new(envelope, None, Some(scenario_id.clone()), None, canon_model::EvidenceVerdict::Faithful)
            .with_project_id(project_id.clone())
            .with_evidence_sha(moved);
        GitTier::new(&gate_ctx.ledger_root).write(&binding).expect("writing the live binding");

        let divergence_dir = gate_ctx.ledger_root.join("kind=divergence");
        let persisted_before = files_with_bytes(&divergence_dir);
        assert_eq!(persisted_before.len(), 1, "exactly the one committed resolution");

        assert_eq!(run_list(dir.path(), None, false, &crate::paging::Page::first_page(), None), 0);
        let states = canon_report::divergence::current_states(&gate_ctx.ledger_root, Utc::now());
        assert_eq!(states.get(&(project_id, scenario_id)), Some(&canon_model::FoldedState::ResolvedInvalid));

        assert_eq!(files_with_bytes(&divergence_dir), persisted_before, "reading must never rewrite the persisted resolution");
        let persisted: canon_model::records::Divergence = serde_json::from_slice(&persisted_before[0].1).unwrap();
        assert_eq!(persisted.status, DivergenceStatus::Resolved, "the record on disk still says resolved");
        assert_eq!(persisted.sha, sha.sha);
    }
}
