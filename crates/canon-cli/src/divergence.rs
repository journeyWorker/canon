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
//!   status, including `Resolved`/`Deferred`. With `--promote` it
//!   then promotes that one candidate
//!   (`canon_gate::promote_staged_divergence`), never another staged
//!   one.
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

use canon_gate::{
    commit_divergence, divergence_staging_dir, promote_divergence, promote_staged_divergence, stage_divergence, DivergenceCandidate, GateCtx, PromoteReport,
};
use canon_model::{Actor, DivergenceStatus, Envelope, RecordKind, RoleId, Sha};
use canon_store::git_tier::GitTier;
use chrono::{DateTime, Utc};

pub use crate::review::{parse_project_id, parse_scenario_id};
use crate::cli_error::CliError;
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

/// `canon divergence add --status <status>`'s status-string ->
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
    let mut out = format_candidate_lines(report, dry_run);
    if report.promoted.is_empty() && report.refused.is_empty() {
        out.push_str("canon divergence promote: nothing staged\n");
    }
    out
}

/// One line per candidate in `report` — [`format_promote_report`]'s
/// body, and the whole of what `stage --promote` prints for its one
/// candidate.
fn format_candidate_lines(report: &PromoteReport, dry_run: bool) -> String {
    let verb = if dry_run { "would promote" } else { "promoted" };
    let mut out = String::new();
    for p in &report.promoted {
        out.push_str(&format!("{verb} {} -> {}\n", p.label(), p.target.display()));
    }
    for r in &report.refused {
        out.push_str(&format!("refused: {}\n", r.violation.line()));
    }
    out
}

/// `canon divergence add` (module doc): writes ONE unordered staging
/// candidate — `run_seq` is assigned later, at `promote` time. With
/// `--promote` or `write: { promote: auto }` (0.16 D2) the same run then
/// promotes that one candidate (`canon_gate::promote_staged_divergence`)
/// and leaves every other staged candidate for `canon divergence
/// promote` ([`crate::write_mode`]).
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
    promote: crate::write_mode::PromoteFlag,
    json: bool,
) -> i32 {
    let repo = resolve_repo_root(repo);
    let promote = match crate::write_mode::should_promote(&repo, promote) {
        Ok(promote) => promote,
        Err(error) => {
            return CliError::refused("CONFIG_UNREADABLE", error.to_string()).emit("canon divergence add", json);
        }
    };
    let gate_ctx = match GateCtx::from_repo(&repo) {
        Ok(ctx) => ctx,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).emit("canon divergence add", json);
        }
    };
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

    let path = match stage_divergence(&staging_dir, &candidate) {
        Ok(path) => path,
        Err(e) => {
            return CliError::refused("WRITE_FAILED", e.to_string()).emit("canon divergence add", json);
        }
    };
    if !promote {
        crate::say!(json, "canon divergence add: staged {} — {}", path.display(), crate::write_mode::STAGED_FOR_DIVERGENCE_PROMOTE);
        if json {
            crate::json_out::data("divergence add", serde_json::json!({ "staged": path.display().to_string(), "promoted": false }));
        }
        return 0;
    }
    // Only the file this run staged: another reviewer's staged candidate
    // is theirs to promote. Once it lands, the success line names the
    // committed record alone, never the staged file promotion moved.
    match promote_staged_divergence(&path, &GitTier::new(&gate_ctx.ledger_root)) {
        Ok(report) => match report.promoted.iter().chain(&report.recovered).next().filter(|_| report.is_clean()) {
            Some(landed) => {
                crate::say!(json, "canon divergence add: wrote {} — {}", landed.target.display(), crate::write_mode::PROMOTED);
                if json {
                    crate::json_out::data("divergence add", serde_json::json!({ "committed": landed.target.display().to_string(), "promoted": true }));
                }
                return 0;
            }
            None => {
                crate::say_raw!(json, "{}", format_candidate_lines(&report, false));
                crate::say!(json, "canon divergence add: staged {} — {}", path.display(), crate::write_mode::STAGED_FOR_DIVERGENCE_PROMOTE);
                let mut data = crate::gate::promote_json(&report, false);
                data["staged"] = serde_json::json!(path.display().to_string());
                return CliError::failed("PROMOTE_REFUSED", "the staged divergence was refused by promotion; it stays staged").emit_with_data("canon divergence add", json, data);
            }
        },
        Err(e) => {
            let code = CliError::failed("PROMOTE_FAILED", format!("staged, but promoting it failed: {e}")).emit("canon divergence add", json);
            crate::say!(json, "canon divergence add: staged {} — {}", path.display(), crate::write_mode::STAGED_FOR_DIVERGENCE_PROMOTE);
            return code;
        }
    }
}

/// `canon divergence promote [--dry-run]` (module doc): batch-promotes
/// every currently-staged candidate.
pub fn run_promote(repo: &Path, dry_run: bool, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let gate_ctx = match GateCtx::from_repo(&repo) {
        Ok(ctx) => ctx,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).emit("canon divergence promote", json);
        }
    };
    let staging_dir = divergence_staging_dir(&gate_ctx.ledger_root);
    let committed = GitTier::new(&gate_ctx.ledger_root);

    match promote_divergence(&staging_dir, &committed, dry_run) {
        Ok(report) => {
            crate::say_raw!(json, "{}", format_promote_report(&report, dry_run));
            let data = crate::gate::promote_json(&report, dry_run);
            if report.is_clean() {
                if json {
                    crate::json_out::data("divergence promote", data);
                }
                0
            } else {
                CliError::failed("PROMOTE_REFUSED", format!("{} staged divergence(s) refused", report.refused.len())).emit_with_data("canon divergence promote", json, data)
            }
        }
        Err(e) => {
            CliError::refused("WRITE_FAILED", e.to_string()).emit("canon divergence promote", json)
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
    json: bool,
) -> i32 {
    let repo = resolve_repo_root(repo);
    let gate_ctx = match GateCtx::from_repo(&repo) {
        Ok(ctx) => ctx,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).emit(&format!("canon divergence {verb}"), json);
        }
    };
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
            crate::say!(json, "canon divergence {verb}: committed {} -> {} — {}", promoted.label(), promoted.target.display(), crate::write_mode::DIRECT);
            if json {
                crate::json_out::data(&format!("divergence {verb}"), serde_json::json!({ "committed": promoted.target.display().to_string(), "label": promoted.label() }));
            }
            0
        }
        Ok(Err(refused)) => {
            CliError::failed("DIVERGENCE_REFUSED", refused.violation.line()).emit(&format!("canon divergence {verb}"), json)
        }
        Err(e) => {
            CliError::refused("WRITE_FAILED", e.to_string()).emit(&format!("canon divergence {verb}"), json)
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
    json: bool,
) -> i32 {
    run_commit("resolve", repo, project_id, scenario_id, sha, DivergenceStatus::Resolved, round, reviewer, detail, actor_id, role, json)
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
    json: bool,
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
        json,
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
pub fn run_status(repo: &Path, as_of: Option<DateTime<Utc>>, json: bool, page: &crate::paging::Page) -> i32 {
    let repo = resolve_repo_root(repo);
    let gate_ctx = match GateCtx::from_repo(&repo) {
        Ok(ctx) => ctx,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).emit("canon divergence status", json);
        }
    };
    let as_of = as_of.unwrap_or_else(Utc::now);

    let states = canon_report::divergence::current_states(&gate_ctx.ledger_root, as_of);
    let summary = canon_report::divergence::summarize(&states);
    let (rows, meta) = match crate::paging::paginate("divergence status", states.into_iter().collect::<Vec<_>>(), |((project_id, scenario_id), _)| format!("{project_id}/{scenario_id}"), page) {
        Ok(paged) => paged,
        Err(e) => return e.emit("canon divergence status", json),
    };

    if json {
        let scenarios: Vec<serde_json::Value> = rows
            .iter()
            .map(|((project_id, scenario_id), state)| {
                let mut row = serde_json::json!({ "project_id": project_id.as_str(), "scenario_id": scenario_id.as_str(), "state": state_slug(state) });
                if let canon_model::FoldedState::Deferred { reason, expiry } = state {
                    row["reason"] = serde_json::json!(reason);
                    row["expiry"] = serde_json::json!(expiry.to_string());
                }
                row
            })
            .collect();
        let mut meta_json = serde_json::json!({ "command": "divergence status" });
        meta.insert_into(meta_json.as_object_mut().expect("meta is an object"));
        let out = serde_json::json!({
            "data": {
                "as_of": as_of.to_rfc3339(),
                "summary": {
                    "open": summary.open,
                    "resolved": summary.resolved,
                    "still_divergent": summary.still_divergent,
                    "deferred": summary.deferred,
                    "resolved_invalid": summary.resolved_invalid,
                    "total": summary.total(),
                },
                "scenarios": scenarios,
            },
            "meta": meta_json,
        });
        println!("{}", serde_json::to_string_pretty(&out).expect("a JSON value always serializes"));
        return 0;
    }

    println!(
        "canon divergence status (as of {as_of}): open={} resolved={} still-divergent={} deferred={} resolved-invalid={} (total={})",
        summary.open,
        summary.resolved,
        summary.still_divergent,
        summary.deferred,
        summary.resolved_invalid,
        summary.total()
    );
    for ((project_id, scenario_id), state) in &rows {
        let state_str = match state {
            canon_model::FoldedState::Deferred { reason, expiry } => format!("deferred({reason}, until {expiry})"),
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
    }
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

        assert_eq!(run_stage(dir.path(), &project_id, &scenario_id_a, &sha, DivergenceStatus::Open, 1, "reviewer-1", "", "agent-x", &role, Default::default(), false), 0);
        assert_eq!(run_stage(dir.path(), &project_id, &scenario_id_b, &sha, DivergenceStatus::Open, 1, "reviewer-1", "", "agent-x", &role, Default::default(), false), 0);

        let code = run_promote(dir.path(), false, false);
        assert_eq!(code, 0);

        let gate_ctx = GateCtx::from_repo(dir.path()).unwrap();
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
        let staging_dir = divergence_staging_dir(&GateCtx::from_repo(dir.path()).unwrap().ledger_root);
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

        assert_eq!(run_stage(dir.path(), &project_id, &scenario_id, &sha, DivergenceStatus::Open, 1, "reviewer-1", "", "agent-x", &role, Default::default(), false), 0);

        let code = run_promote(dir.path(), false, false);
        assert_eq!(code, 1, "a refusal reports a non-zero (but not usage-error) exit code");

        let gate_ctx = GateCtx::from_repo(dir.path()).unwrap();
        let committed = GitTier::new(&gate_ctx.ledger_root);
        let landed = committed.read(&TierQuery::kind(RecordKind::Divergence)).unwrap();
        assert_eq!(landed.records.len(), 1);
        let run_seq = landed.records[0].0.get("run_seq").and_then(|v| v.as_u64()).unwrap();
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
    fn resolve_and_defer_commit_directly_without_touching_staging() {
        let dir = TempDir::new().unwrap();
        let (project_id, scenario_id, role, sha) = ids();
        let deferred_id = ScenarioId::parse("world.firstbuy-hotdeal.51").unwrap();

        // Something else is mid-`stage`.
        assert_eq!(run_stage(dir.path(), &project_id, &ScenarioId::parse("world.firstbuy-hotdeal.50").unwrap(), &sha, DivergenceStatus::Open, 1, "reviewer-1", "", "agent-x", &role, Default::default(), false), 0);
        let gate_ctx = GateCtx::from_repo(dir.path()).unwrap();
        let staging_dir = divergence_staging_dir(&gate_ctx.ledger_root);
        let staged_before = files_with_bytes(&staging_dir);
        assert_eq!(staged_before.len(), 1, "exactly the one unrelated candidate is staged");

        assert_eq!(run_resolve(dir.path(), &project_id, &scenario_id, &sha, 1, "reviewer-1", "", "agent-x", &role, false), 0);
        // Whole seconds so the persisted expiry compares exactly.
        let expiry = chrono::DateTime::from_timestamp((Utc::now() + chrono::Duration::days(7)).timestamp(), 0).unwrap();
        assert_eq!(run_defer(dir.path(), &project_id, &deferred_id, &sha, 1, "reviewer-1", "needs another look", expiry, "agent-x", &role, false), 0);

        let committed = GitTier::new(&gate_ctx.ledger_root);
        let landed = committed.read(&TierQuery::kind(RecordKind::Divergence)).unwrap();
        let mut records: Vec<canon_model::records::Divergence> =
            landed.records.into_iter().map(|record| serde_json::from_value(record.0).expect("a committed divergence")).collect();
        records.sort_by(|a, b| a.scenario_id.as_str().cmp(b.scenario_id.as_str()));
        assert_eq!(records.len(), 2, "resolve + defer both committed without a promote step");
        assert_eq!((&records[0].project_id, &records[0].scenario_id, &records[0].status), (&project_id, &scenario_id, &DivergenceStatus::Resolved));
        assert_eq!(
            (&records[1].project_id, &records[1].scenario_id, &records[1].status),
            (&project_id, &deferred_id, &DivergenceStatus::Deferred { reason: "needs another look".to_string(), expiry })
        );

        // The unrelated staged candidate is still sitting there, byte-identical.
        assert_eq!(files_with_bytes(&staging_dir), staged_before, "a direct commit must never touch staged work");
    }

    #[test]
    fn status_reflects_a_directly_committed_resolved_divergence() {
        let dir = TempDir::new().unwrap();
        let (project_id, scenario_id, role, sha) = ids();

        assert_eq!(run_resolve(dir.path(), &project_id, &scenario_id, &sha, 1, "reviewer-1", "", "agent-x", &role, false), 0);

        // Read-only capability query — always exits 0.
        assert_eq!(run_status(dir.path(), None, false, &crate::paging::Page::first_page()), 0);

        let gate_ctx = GateCtx::from_repo(dir.path()).unwrap();
        let states = canon_report::divergence::current_states(&gate_ctx.ledger_root, Utc::now());
        assert_eq!(states.get(&(project_id, scenario_id)), Some(&canon_model::FoldedState::Resolved));
    }

    /// divergence.lifecycle.08 against a real ledger: a committed
    /// resolution against sha `a` reads back as resolved-invalid once the
    /// scenario's live evidence binding names sha `b`, and neither
    /// `canon divergence status` nor the fold rewrites the persisted
    /// resolved record — the downgrade is derived at read time.
    #[test]
    fn a_moved_live_binding_reads_resolved_invalid_without_rewriting_the_resolution() {
        let dir = TempDir::new().unwrap();
        let (project_id, scenario_id, role, sha) = ids();
        assert_eq!(run_resolve(dir.path(), &project_id, &scenario_id, &sha, 1, "reviewer-1", "", "agent-x", &role, false), 0);

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

        assert_eq!(run_status(dir.path(), None, false, &crate::paging::Page::first_page()), 0);
        let states = canon_report::divergence::current_states(&gate_ctx.ledger_root, Utc::now());
        assert_eq!(states.get(&(project_id, scenario_id)), Some(&canon_model::FoldedState::ResolvedInvalid));

        assert_eq!(files_with_bytes(&divergence_dir), persisted_before, "reading must never rewrite the persisted resolution");
        let persisted: canon_model::records::Divergence = serde_json::from_slice(&persisted_before[0].1).unwrap();
        assert_eq!(persisted.status, DivergenceStatus::Resolved, "the record on disk still says resolved");
        assert_eq!(persisted.sha, sha);
    }
}
