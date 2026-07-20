//! `canon divergence {stage,promote,resolve,defer}` (s15 P3b,
//! native-verdict-lifecycle spec, design D10): the ONLY producer of
//! `canon_model::Divergence` records — without it, `canon-gate`'s
//! divergence fold (`canon_model::fold_to_current_state`) has no
//! source of `Divergence` records at all, leaving the S9 divergence
//! burn-down permanently empty.
//!
//! # Four subcommands, two mechanisms
//! - `stage` writes an unordered staging candidate carrying no
//!   `run_seq` (`canon_gate::stage_divergence`) — a candidate of ANY
//!   status, including `Resolved`/`Deferred`.
//! - `promote` batch-promotes every currently-staged candidate
//!   (`canon_gate::promote_divergence`), assigning each a monotonic
//!   `run_seq` within its `(project_id, role, surface)` partition.
//! - `resolve`/`defer` are convenience wrappers over
//!   `canon_gate::commit_divergence` — a SINGLE candidate, with status
//!   `Resolved`/`Deferred{reason, expiry}`, assigned a `run_seq` and
//!   committed DIRECTLY, without touching the batch staging directory
//!   at all (so a routine resolve/defer never risks promoting an
//!   UNRELATED candidate a reviewer is still mid-`stage`ing —
//!   `canon_gate::promote`'s own module doc).

use std::path::Path;

use canon_gate::{commit_divergence, divergence_staging_dir, promote_divergence, stage_divergence, DivergenceCandidate, GateCtx, PromoteReport};
use canon_model::{Actor, DivergenceStatus, Envelope, RecordKind, RoleId, Sha};
use canon_store::git_tier::GitTier;
use chrono::{DateTime, Utc};

pub use crate::review::{parse_project_id, parse_scenario_id};
use crate::context::resolve_repo_root;

/// `--sha`'s `clap` value parser.
pub fn parse_sha(s: &str) -> Result<Sha, String> {
    Sha::parse(s).map_err(|e| e.to_string())
}

/// `--expiry`'s `clap` value parser — an RFC3339/ISO-8601 timestamp
/// (`canon_cli::query::parse_since`'s identical grammar, a distinct
/// function so the error message names `--expiry`, not `--since`).
pub fn parse_timestamp(s: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)).map_err(|e| format!("invalid timestamp `{s}`: {e}"))
}

/// `canon divergence stage --status <status>`'s status-string ->
/// `DivergenceStatus` builder (module doc: `stage` accepts ANY
/// status, not only `Open`). `--reason`/`--expiry` are REQUIRED
/// together only when `status == "deferred"` — an `Err` here is a
/// usage refusal, never a synthesized reason/expiry.
pub fn parse_status(status: &str, reason: Option<&str>, expiry: Option<DateTime<Utc>>) -> Result<DivergenceStatus, String> {
    match status {
        "open" => Ok(DivergenceStatus::Open),
        "still-divergent" => Ok(DivergenceStatus::StillDivergent),
        "resolved" => Ok(DivergenceStatus::Resolved),
        "deferred" => {
            let reason = reason.ok_or("`--status deferred` requires `--reason`")?;
            let expiry = expiry.ok_or("`--status deferred` requires `--expiry`")?;
            Ok(DivergenceStatus::Deferred { reason: reason.to_string(), expiry })
        }
        other => Err(format!("unknown --status `{other}` (expected one of: open, still-divergent, resolved, deferred)")),
    }
}

fn format_promote_report(report: &PromoteReport, dry_run: bool) -> String {
    let verb = if dry_run { "would promote" } else { "promoted" };
    let mut out = String::new();
    for p in &report.promoted {
        out.push_str(&format!("{verb} {}/{} run_seq={} -> {}\n", p.role.as_str(), p.surface, p.run_seq, p.target.display()));
    }
    for r in &report.refused {
        out.push_str(&format!("refused: {}\n", r.violation.line()));
    }
    if report.promoted.is_empty() && report.refused.is_empty() {
        out.push_str("canon divergence promote: nothing staged\n");
    }
    out
}

/// `canon divergence stage` (module doc): writes ONE unordered staging
/// candidate — `run_seq` is assigned later, at `promote` time.
#[allow(clippy::too_many_arguments)]
pub fn run_stage(
    repo: &Path,
    project_id: &canon_model::ProjectId,
    scenario_id: &canon_model::ScenarioId,
    sha: &Sha,
    status: DivergenceStatus,
    round: u32,
    reviewer: &str,
    detail: &str,
    actor_id: &str,
    role: &RoleId,
) -> i32 {
    let repo = resolve_repo_root(repo);
    let gate_ctx = GateCtx::from_repo(&repo);
    let staging_dir = divergence_staging_dir(&gate_ctx.ledger_root);

    let candidate = DivergenceCandidate {
        envelope: Envelope::new(1, RecordKind::Divergence, Utc::now(), Actor::new(actor_id, role.clone())),
        project_id: project_id.clone(),
        scenario_id: scenario_id.clone(),
        sha: sha.clone(),
        status,
        round,
        reviewer: reviewer.to_string(),
        detail: detail.to_string(),
    };

    match stage_divergence(&staging_dir, &candidate) {
        Ok(path) => {
            println!("canon divergence stage: wrote {}", path.display());
            0
        }
        Err(e) => {
            eprintln!("canon divergence stage: {e}");
            2
        }
    }
}

/// `canon divergence promote [--dry-run]` (module doc): batch-promotes
/// every currently-staged candidate.
pub fn run_promote(repo: &Path, dry_run: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let gate_ctx = GateCtx::from_repo(&repo);
    let staging_dir = divergence_staging_dir(&gate_ctx.ledger_root);
    let committed = GitTier::new(&gate_ctx.ledger_root);

    match promote_divergence(&staging_dir, &committed, dry_run) {
        Ok(report) => {
            print!("{}", format_promote_report(&report, dry_run));
            if report.is_clean() {
                0
            } else {
                1
            }
        }
        Err(e) => {
            eprintln!("canon divergence promote: {e}");
            2
        }
    }
}

/// Shared `resolve`/`defer` direct-commit path (module doc): build one
/// [`DivergenceCandidate`] with `status` and commit it immediately via
/// [`commit_divergence`], never through the batch staging directory.
#[allow(clippy::too_many_arguments)]
fn run_commit(
    verb: &str,
    repo: &Path,
    project_id: &canon_model::ProjectId,
    scenario_id: &canon_model::ScenarioId,
    sha: &Sha,
    status: DivergenceStatus,
    round: u32,
    reviewer: &str,
    detail: &str,
    actor_id: &str,
    role: &RoleId,
) -> i32 {
    let repo = resolve_repo_root(repo);
    let gate_ctx = GateCtx::from_repo(&repo);
    let committed = GitTier::new(&gate_ctx.ledger_root);

    let candidate = DivergenceCandidate {
        envelope: Envelope::new(1, RecordKind::Divergence, Utc::now(), Actor::new(actor_id, role.clone())),
        project_id: project_id.clone(),
        scenario_id: scenario_id.clone(),
        sha: sha.clone(),
        status,
        round,
        reviewer: reviewer.to_string(),
        detail: detail.to_string(),
    };

    match commit_divergence(&candidate, &committed) {
        Ok(Ok(promoted)) => {
            println!("canon divergence {verb}: committed {}/{} run_seq={} -> {}", promoted.role.as_str(), promoted.surface, promoted.run_seq, promoted.target.display());
            0
        }
        Ok(Err(refused)) => {
            eprintln!("canon divergence {verb}: refused: {}", refused.violation.line());
            1
        }
        Err(e) => {
            eprintln!("canon divergence {verb}: {e}");
            2
        }
    }
}

/// `canon divergence resolve` (module doc): direct-commits a
/// `Resolved` candidate.
#[allow(clippy::too_many_arguments)]
pub fn run_resolve(
    repo: &Path,
    project_id: &canon_model::ProjectId,
    scenario_id: &canon_model::ScenarioId,
    sha: &Sha,
    round: u32,
    reviewer: &str,
    detail: &str,
    actor_id: &str,
    role: &RoleId,
) -> i32 {
    run_commit("resolve", repo, project_id, scenario_id, sha, DivergenceStatus::Resolved, round, reviewer, detail, actor_id, role)
}

/// `canon divergence defer --reason <..> --expiry <..>` (module doc):
/// direct-commits a `Deferred { reason, expiry }` candidate.
#[allow(clippy::too_many_arguments)]
pub fn run_defer(
    repo: &Path,
    project_id: &canon_model::ProjectId,
    scenario_id: &canon_model::ScenarioId,
    sha: &Sha,
    round: u32,
    reviewer: &str,
    reason: &str,
    expiry: DateTime<Utc>,
    actor_id: &str,
    role: &RoleId,
) -> i32 {
    run_commit(
        "defer",
        repo,
        project_id,
        scenario_id,
        sha,
        DivergenceStatus::Deferred { reason: reason.to_string(), expiry },
        round,
        reviewer,
        "",
        actor_id,
        role,
    )
}

/// `canon divergence status [--as-of <timestamp>]` (task 4.5): the S9
/// divergence burn-down's CURRENT-STATE view, over
/// `canon_report::divergence::{current_states, summarize}` — the
/// consumer of `canon_model::fold_to_current_state` this crate wires
/// up (`canon_report::divergence`'s own module doc contrasts this
/// with `mart_review_burndown`'s per-day TREND). `as_of` defaults to
/// "now"; ALWAYS exits `0` — a read-only capability query over
/// whatever is currently on the ledger, never a gate.
pub fn run_status(repo: &Path, as_of: Option<DateTime<Utc>>) -> i32 {
    let repo = resolve_repo_root(repo);
    let gate_ctx = GateCtx::from_repo(&repo);
    let as_of = as_of.unwrap_or_else(Utc::now);

    let states = canon_report::divergence::current_states(&gate_ctx.ledger_root, as_of);
    let summary = canon_report::divergence::summarize(&states);

    println!(
        "canon divergence status (as of {as_of}): open={} resolved={} still-divergent={} deferred={} resolved-invalid={} (total={})",
        summary.open,
        summary.resolved,
        summary.still_divergent,
        summary.deferred,
        summary.resolved_invalid,
        summary.total()
    );
    for ((project_id, scenario_id), state) in &states {
        let state_str = match state {
            canon_model::FoldedState::Open => "open".to_string(),
            canon_model::FoldedState::Resolved => "resolved".to_string(),
            canon_model::FoldedState::StillDivergent => "still-divergent".to_string(),
            canon_model::FoldedState::Deferred { reason, expiry } => format!("deferred({reason}, until {expiry})"),
            canon_model::FoldedState::ResolvedInvalid => "resolved-invalid".to_string(),
        };
        println!("  {project_id}/{scenario_id}: {state_str}");
    }
    0
}

#[cfg(test)]
mod tests {
    use canon_model::{ProjectId, ScenarioId};
    use canon_store::tier::{Tier, TierQuery};
    use tempfile::TempDir;

    use super::*;

    fn ids() -> (ProjectId, ScenarioId, RoleId, Sha) {
        (
            ProjectId::parse("app-a").unwrap(),
            ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap(),
            RoleId::parse("reviewer").unwrap(),
            Sha::parse("a".repeat(40)).unwrap(),
        )
    }

    #[test]
    fn stage_then_promote_assigns_a_monotonic_run_seq_and_a_refusal_consumes_none() {
        let dir = TempDir::new().unwrap();
        let (project_id, scenario_id_a, role, sha) = ids();
        let scenario_id_b = ScenarioId::parse("world.firstbuy-hotdeal.27").unwrap();

        assert_eq!(run_stage(dir.path(), &project_id, &scenario_id_a, &sha, DivergenceStatus::Open, 1, "reviewer-1", "", "agent-x", &role), 0);
        assert_eq!(run_stage(dir.path(), &project_id, &scenario_id_b, &sha, DivergenceStatus::Open, 1, "reviewer-1", "", "agent-x", &role), 0);

        let code = run_promote(dir.path(), false);
        assert_eq!(code, 0);

        let gate_ctx = GateCtx::from_repo(dir.path());
        let committed = GitTier::new(&gate_ctx.ledger_root);
        let landed = committed.read(&TierQuery::kind(RecordKind::Divergence)).unwrap();
        assert_eq!(landed.records.len(), 2);
        let mut seqs: Vec<u64> = landed.records.iter().filter_map(|r| r.0.get("run_seq").and_then(|v| v.as_u64())).collect();
        seqs.sort_unstable();
        assert_eq!(seqs, vec![1, 2]);

        // Staging is empty afterward.
        let staging_dir = divergence_staging_dir(&gate_ctx.ledger_root);
        assert!(std::fs::read_dir(&staging_dir).map(|mut e| e.next().is_none()).unwrap_or(true));
    }

    #[test]
    fn a_refused_candidate_consumes_no_run_seq() {
        let dir = TempDir::new().unwrap();
        let (project_id, scenario_id, role, sha) = ids();

        // Refused: no `actor.role` at all.
        let staging_dir = divergence_staging_dir(&GateCtx::from_repo(dir.path()).ledger_root);
        let malformed = DivergenceCandidate {
            envelope: Envelope::new(1, RecordKind::Divergence, Utc::now(), Actor::new_unattributed("legacy-writer")),
            project_id: project_id.clone(),
            scenario_id: ScenarioId::parse("world.firstbuy-hotdeal.90").unwrap(),
            sha: sha.clone(),
            status: DivergenceStatus::Open,
            round: 1,
            reviewer: "reviewer-1".to_string(),
            detail: String::new(),
        };
        stage_divergence(&staging_dir, &malformed).unwrap();

        assert_eq!(run_stage(dir.path(), &project_id, &scenario_id, &sha, DivergenceStatus::Open, 1, "reviewer-1", "", "agent-x", &role), 0);

        let code = run_promote(dir.path(), false);
        assert_eq!(code, 1, "a refusal reports a non-zero (but not usage-error) exit code");

        let gate_ctx = GateCtx::from_repo(dir.path());
        let committed = GitTier::new(&gate_ctx.ledger_root);
        let landed = committed.read(&TierQuery::kind(RecordKind::Divergence)).unwrap();
        assert_eq!(landed.records.len(), 1);
        let run_seq = landed.records[0].0.get("run_seq").and_then(|v| v.as_u64()).unwrap();
        assert_eq!(run_seq, 1, "the refused candidate must not have consumed run_seq 1");
    }

    #[test]
    fn resolve_and_defer_commit_directly_without_touching_staging() {
        let dir = TempDir::new().unwrap();
        let (project_id, scenario_id, role, sha) = ids();

        // Something else is mid-`stage`.
        assert_eq!(run_stage(dir.path(), &project_id, &ScenarioId::parse("world.firstbuy-hotdeal.50").unwrap(), &sha, DivergenceStatus::Open, 1, "reviewer-1", "", "agent-x", &role), 0);

        assert_eq!(run_resolve(dir.path(), &project_id, &scenario_id, &sha, 1, "reviewer-1", "", "agent-x", &role), 0);
        let expiry = Utc::now() + chrono::Duration::days(7);
        assert_eq!(
            run_defer(dir.path(), &project_id, &ScenarioId::parse("world.firstbuy-hotdeal.51").unwrap(), &sha, 1, "reviewer-1", "needs another look", expiry, "agent-x", &role),
            0
        );

        let gate_ctx = GateCtx::from_repo(dir.path());
        let committed = GitTier::new(&gate_ctx.ledger_root);
        let landed = committed.read(&TierQuery::kind(RecordKind::Divergence)).unwrap();
        assert_eq!(landed.records.len(), 2, "resolve + defer both committed");

        // The unrelated staged candidate is still sitting there, untouched.
        let staging_dir = divergence_staging_dir(&gate_ctx.ledger_root);
        assert_eq!(std::fs::read_dir(&staging_dir).unwrap().count(), 1);
    }

    #[test]
    fn status_reflects_a_directly_committed_resolved_divergence() {
        let dir = TempDir::new().unwrap();
        let (project_id, scenario_id, role, sha) = ids();

        assert_eq!(run_resolve(dir.path(), &project_id, &scenario_id, &sha, 1, "reviewer-1", "", "agent-x", &role), 0);

        // Read-only capability query — always exits 0.
        assert_eq!(run_status(dir.path(), None), 0);

        let gate_ctx = GateCtx::from_repo(dir.path());
        let states = canon_report::divergence::current_states(&gate_ctx.ledger_root, Utc::now());
        assert_eq!(states.get(&(project_id, scenario_id)), Some(&canon_model::FoldedState::Resolved));
    }
}
