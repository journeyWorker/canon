//! `canon learn promote <strategy_id>` (S6 `role-strategy-memory` task
//! group 4): promote a distilled [`canon_learn::StrategyItem`] from the
//! operator-local parquet warm tier up into the git-tracked, PR-reviewed
//! tier (`<repo>/.canon/strategies/<role>/<id>.md`, `LearnConfig::
//! strategies_root`). The write path itself is `canon-learn`'s
//! ([`canon_learn::promote_strategy`]); this module only resolves the
//! repo's `canon.yaml`-configured store roots (mirroring
//! `canon_cli::artifact_ingest`'s own `learn.root`/`strategies_root`
//! resolution — never a second config-reading convention) and handles
//! the `--dry-run` preview + advisory-lint surfacing.
//!
//! Exit codes mirror the rest of `canon-cli`: `0` on a written (or
//! previewed) promotion, `2` on a usage error (an unparseable
//! `strategy_id` never reaches here — clap rejects it first), `1` on a
//! real failure (a malformed `learn:` config, an unknown `strategy_id`,
//! or a filesystem write error) OR on a promotion the S7 statistical
//! gate REFUSES. The advisory lint (content length, a literal
//! machine-specific absolute path) NEVER changes the exit code: its
//! warnings go to stderr and the promotion proceeds.
//!
//! # The statistical gate is the primary enforcement point
//! S6's own design says so verbatim: "[Mitigation] S7's
//! statistical-promotion gate is the primary enforcement point; this
//! change's `canon learn promote` adds a content-length +
//! literal-path-pattern lint as defense-in-depth (documented as
//! advisory)". S6 shipped this command BEFORE S7's gates existed, and
//! nothing came back to wire them (s37 `execution-graph-topology`) — so
//! for the whole interval between, the advisory lint was the only check
//! standing between a distilled item and the git-tracked, PR-reviewed
//! tier. Any item could graduate, including one whose every trajectory
//! was still `VerdictOutcome::Pending`.
//!
//! [`canon_learn::PromotionGate::evaluate`] is deliberately a PURE
//! function over already-resolved samples ("neither gate reads a store
//! OR a wall clock directly", `canon_learn::promotion`'s module doc),
//! so RESOLVING those samples is this integration layer's job, exactly
//! as `canon_learn::webhook`'s own doc assigns the mirror-image task
//! ("gathers candidates from a live store is `canon-cli`'s job"). This
//! module therefore: resolves the item to recover its `regime_key` +
//! `role`, reads that regime's trajectories, picks the per-role gate
//! [`canon_learn::PromotionMode`] from `canon.yaml`, and evaluates.
//!
//! FAILS CLOSED, with no `--force` escape: `canon.yaml`'s own parser
//! already rejects `promotion.<role>.n_min: 0` precisely because it
//! "would let `OccurrencePromotionGate` promote with zero corroborating
//! successes … defeating the n-occurrence gate entirely" (`config.rs`'s
//! own test). An override flag would reintroduce exactly the defeat that
//! validation exists to prevent. A rejected promotion is a statement
//! that the evidence is not there yet — the fix is to resolve the
//! trajectories (`canon ingest artifacts`), not to bypass the gate.

use std::path::Path;
use std::process::ExitCode;

use canon_learn::{
    CrnPromotionGate, DemotionPolicy, LearnConfig, LearnError, OccurrencePromotionGate, ParquetStrategyStore, ParquetTrajectoryStore, Promotion,
    PromotionApproval, PromotionDecision, PromotionEvaluation, PromotionMode, StrategyId, StrategyLifecycle, StrategyStore, TrajectoryStore,
    demote_strategy, evaluate_now, plan_promotion, promote_strategy, promote_strategy_approved, rollback_strategy_authenticated,
};
use chrono::Utc;
use serde_json::json;

use crate::context::resolve_repo_root;

/// clap `value_parser` for the positional `<strategy_id>` — a ULID
/// ([`canon_learn::StrategyId::parse`]); a malformed id is a clap usage
/// error (exit `2`), never reaching [`run_promote`].
pub fn parse_strategy_id(s: &str) -> Result<StrategyId, String> {
    StrategyId::parse(s).map_err(|e| e.to_string())
}

/// `canon learn promote <strategy_id> [--repo <dir>] [--dry-run]`.
pub fn run_promote(repo: &Path, strategy_id: &StrategyId, dry_run: bool) -> ExitCode {
    run_promote_with_evidence(repo, strategy_id, dry_run, None, None, None)
}

pub fn run_promote_with_evidence(
    repo: &Path,
    strategy_id: &StrategyId,
    dry_run: bool,
    evaluation_path: Option<&Path>,
    approval_path: Option<&Path>,
    signature_path: Option<&Path>,
) -> ExitCode {
    let repo = resolve_repo_root(repo);
    let canon_yaml_text = std::fs::read_to_string(repo.join("canon.yaml")).unwrap_or_default();
    // A genuinely absent `learn:` section resolves to `LearnConfig::default()`
    // inside `from_manifest`; only a MALFORMED section reaches `Err`, and that
    // fails loud rather than silently promoting into the wrong store root
    // (same discipline `canon ingest artifacts` holds).
    let learn_config = match LearnConfig::from_manifest(&canon_yaml_text) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("canon learn promote: {err}");
            return ExitCode::from(1);
        }
    };

    let strategy_store = ParquetStrategyStore::open(repo.join(&learn_config.root).join("strategies"));

    let evidence = match (evaluation_path, approval_path) {
        (None, None) => None,
        (Some(eval), Some(approval)) => {
            let evaluation: PromotionEvaluation = match read_json(eval) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("canon learn promote: invalid evaluation: {error}");
                    return ExitCode::from(1);
                }
            };
            let mut approval: PromotionApproval = match read_json(approval) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("canon learn promote: invalid approval: {error}");
                    return ExitCode::from(1);
                }
            };
            if let Some(signature) = signature_path {
                match std::fs::read_to_string(signature) {
                    Ok(value) => {
                        approval.signature = Some(value);
                        approval.signer_key.get_or_insert_with(|| approval.approver_identity.clone());
                        approval.integrity_digest = approval.recomputed_integrity_digest();
                    }
                    Err(error) => {
                        eprintln!("canon learn promote: cannot read detached signature: {error}");
                        return ExitCode::from(1);
                    }
                }
            }
            Some((evaluation, approval))
        }
        _ => {
            eprintln!("canon learn promote: --evaluation and --approval must be supplied together");
            return ExitCode::from(1);
        }
    };
    let git_tier_root = repo.join(&learn_config.strategies_root);

    // The S7 gate runs BEFORE any write (module doc). `None` means the
    // id resolved to nothing — deliberately NOT reported here, so the
    // canonical `LearnError::UnknownStrategyId` from `plan_promotion`/
    // `promote_strategy` below stays the single source of that message.
    let decision = match promotion_decision(&repo, &learn_config, &strategy_store, strategy_id) {
        Ok(decision) => decision,
        Err(err) => {
            eprintln!("canon learn promote: {err}");
            return ExitCode::from(1);
        }
    };
    let blocked = decision.as_ref().is_some_and(|d| !d.is_promote());
    if blocked && !dry_run {
        if let Ok(Some(item)) = strategy_store.find_by_id(strategy_id) {
            if item.lifecycle == Some(StrategyLifecycle::Active) {
                if let Ok(samples) = ParquetTrajectoryStore::open(repo.join(&learn_config.root).join("trajectories")).query_by_regime_key(&item.regime_key) {
                    if let Some(contradiction) = samples.iter().find(|sample| {
                        matches!(sample.verdict_record.outcome, canon_learn::VerdictOutcome::Failure | canon_learn::VerdictOutcome::RolledBack)
                    }) {
                        let demotion_policy = if learn_config.demotion.hard_delete { DemotionPolicy::HARD_DELETE } else { DemotionPolicy::SOFT_FLAG };
                        if let Err(error) = demote_strategy(&strategy_store, *strategy_id, contradiction.id, &git_tier_root, demotion_policy) {
                            eprintln!("canon learn promote: contradiction detected but demotion failed: {error}");
                        } else {
                            eprintln!("canon learn promote: later contradictory evidence demoted {strategy_id}");
                        }
                    }
                }
            }
        }
    }

    // A blocked NON-dry-run stops here: nothing is rendered, nothing is
    // written. A blocked dry-run still renders its preview first — the
    // operator asked what WOULD be written, and seeing the content
    // alongside the refusal reason is the whole point of a preview — but
    // its exit code stays honest about the refusal.
    if blocked && !dry_run {
        report_block(decision.as_ref().expect("blocked implies a decision"));
        return ExitCode::from(1);
    }

    let outcome = if dry_run {
        plan_promotion(&strategy_store, strategy_id, &git_tier_root)
    } else if let Some((evaluation, approval)) = evidence.as_ref() {
        promote_strategy_approved(&strategy_store, strategy_id, &git_tier_root, evaluation, approval)
    } else {
        promote_strategy(&strategy_store, strategy_id, &git_tier_root)
    };

    match outcome {
        Ok(Promotion { path, warnings, .. }) => {
            for warning in &warnings {
                eprintln!("canon learn promote: advisory: {warning}");
            }
            if dry_run {
                println!("[dry-run] would promote {strategy_id} -> {}", path.display());
                if blocked {
                    report_block(decision.as_ref().expect("blocked implies a decision"));
                    return ExitCode::from(1);
                }
            } else {
                println!("promoted {strategy_id} -> {}", path.display());
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("canon learn promote: {err}");
            ExitCode::from(1)
        }
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

/// Emit the exact domain-separated bytes an external SSH signer must sign.
/// This command never reads a private key and never sets `verified`.
pub fn run_approve(repo: &Path, strategy_id: &StrategyId, evaluation_path: &Path, principal: &str, _json_output: bool) -> ExitCode {
    let repo = resolve_repo_root(repo);
    let config = match LearnConfig::from_manifest(&std::fs::read_to_string(repo.join("canon.yaml")).unwrap_or_default()) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("canon learn approve: {error}");
            return ExitCode::from(1);
        }
    };
    let store = ParquetStrategyStore::open(repo.join(&config.root).join("strategies"));
    let Some(candidate) = (match store.find_by_id(strategy_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("canon learn approve: {error}");
            return ExitCode::from(1);
        }
    }) else {
        eprintln!("canon learn approve: unknown strategy {strategy_id}");
        return ExitCode::from(1);
    };
    let evaluation: PromotionEvaluation = match read_json(evaluation_path) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("canon learn approve: invalid evaluation: {error}");
            return ExitCode::from(1);
        }
    };
    if let Err(error) = evaluation.validate_for(&candidate) {
        eprintln!("canon learn approve: evaluation rejected: {error}");
        return ExitCode::from(1);
    }
    let approved_at = Utc::now();
    let payload = canon_model::approval_payload_bytes(
        "canon-learning-approval-v1", &format!("strategy:{strategy_id}"), None,
        &evaluation.digest(), None, &[], &[], principal, &approved_at,
    );
    let response = json!({
        "schema_version": 1, "candidate_strategy_id": strategy_id.to_string(),
        "evaluation_digest": evaluation.digest(), "approver_identity": principal,
        "approver_role": "human", "approved_at": approved_at,
        "namespace": "canon-learning-approval-v1", "subject": format!("strategy:{strategy_id}"),
        "payload_hex": payload.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        "instructions": "Sign payload_hex externally; put the armored detached signature in approval.signature, set signer_key to approver_identity, and run canon learn promote with --evaluation and --approval.",
    });
    println!("{}", serde_json::to_string_pretty(&response).expect("approval payload serializes"));
    ExitCode::SUCCESS
}

pub fn run_request(repo: &Path, strategy_id: &StrategyId, evaluation_path: &Path, principal: &str, json_output: bool) -> ExitCode {
    run_approve(repo, strategy_id, evaluation_path, principal, json_output)
}

pub fn run_rollback(repo: &Path, strategy_id: &StrategyId, reason: &str, actor: &str, contradicting: Option<&str>, signature_path: &Path, approved_at: &str) -> ExitCode {
    let repo = resolve_repo_root(repo);
    let config = match LearnConfig::from_manifest(&std::fs::read_to_string(repo.join("canon.yaml")).unwrap_or_default()) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("canon learn rollback: {error}");
            return ExitCode::from(1);
        }
    };
    let store = ParquetStrategyStore::open(repo.join(&config.root).join("strategies"));
    let contradicting = match contradicting.map(canon_learn::TrajectoryId::parse).transpose() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("canon learn rollback: invalid contradicting trajectory id: {error}");
            return ExitCode::from(1);
        }
    };
    let trajectory_store = ParquetTrajectoryStore::open(repo.join(&config.root).join("trajectories"));
    if let Some(id) = contradicting.as_ref() {
        match trajectory_store.find_by_id(id) {
            Ok(Some(_)) => {}
            Ok(None) => {
                eprintln!("canon learn rollback: contradicting trajectory does not exist");
                return ExitCode::from(1);
            }
            Err(error) => {
                eprintln!("canon learn rollback: cannot verify contradicting trajectory: {error}");
                return ExitCode::from(1);
            }
        }
    }
    let signature = match std::fs::read_to_string(signature_path) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("canon learn rollback: cannot read detached signature: {error}");
            return ExitCode::from(1);
        }
    };
    let approved_at = match chrono::DateTime::parse_from_rfc3339(approved_at) {
        Ok(value) => value.with_timezone(&Utc),
        Err(error) => {
            eprintln!("canon learn rollback: invalid --approved-at timestamp: {error}");
            return ExitCode::from(1);
        }
    };
    match rollback_strategy_authenticated(&store, *strategy_id, reason, actor, contradicting, &repo.join(&config.strategies_root), &signature, approved_at) {
        Ok(record) => {
            println!("{}", serde_json::to_string(&record).expect("rollback record serializes"));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("canon learn rollback: {error}");
            ExitCode::from(1)
        }
    }
}

/// The one place a refusal is reported, so the real and dry-run paths
/// can never drift in wording.
fn report_block(decision: &PromotionDecision) {
    eprintln!("canon learn promote: blocked by the promotion gate: {}", decision.reason());
    eprintln!(
        "canon learn promote: the gate reads each trajectory's RESOLVED `verdict_record.outcome`, not its raw verdict rows — \
         run `canon ingest artifacts` to resolve pending trajectories for this regime, then retry."
    );
}

/// Resolves `strategy_id`'s own regime, reads that regime's
/// trajectories, and evaluates the per-role gate `canon.yaml` selects.
///
/// `Ok(None)` means no strategy matched — the caller deliberately lets
/// the downstream `plan_promotion`/`promote_strategy` raise the
/// canonical [`LearnError::UnknownStrategyId`] instead of duplicating
/// that message here.
///
/// Both stores are opened under the SAME `learn.root` the rest of this
/// module (and `crate::artifact_ingest`) resolves, never a second
/// config-reading convention.
fn promotion_decision(
    repo: &Path,
    learn_config: &LearnConfig,
    strategy_store: &dyn StrategyStore,
    strategy_id: &StrategyId,
) -> Result<Option<PromotionDecision>, LearnError> {
    let Some(item) = strategy_store.find_by_id(strategy_id)? else {
        return Ok(None);
    };

    let trajectory_store = ParquetTrajectoryStore::open(repo.join(&learn_config.root).join("trajectories"));
    let samples = trajectory_store.query_by_regime_key(&item.regime_key)?;
    let promotion_config = learn_config.promotion_config_for(&item.role);

    // `evaluate_now` is the ONE place canon-learn reads the wall clock
    // for a promotion decision; the trailing-window semantics the
    // occurrence gate wants are exactly "ending now" for a live caller.
    let decision = match promotion_config.mode {
        PromotionMode::Crn => evaluate_now(&CrnPromotionGate, &item.regime_key, &samples),
        PromotionMode::Occurrence => evaluate_now(&OccurrencePromotionGate::from_config(promotion_config), &item.regime_key, &samples),
    };
    Ok(Some(decision))
}
