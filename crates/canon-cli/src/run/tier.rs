//! `canon tier` — the handlers of the group.

use crate::cli::tier::{TierCommand, TierAgeArgs};
use crate::cli_error::{CliError, CliErrorExt as _};
use crate::safety::{path_digest, Mode, Plan, Reads};
use std::process::ExitCode;

pub fn run(cmd: TierCommand) -> ExitCode {
    match cmd {
        TierCommand::Age(TierAgeArgs { safety, repo, json, .. }) => run_tier_age(&repo, &safety.mode(), json),
    }
}

/// `canon tier age`: the bare run previews the moves; `--commit --plan-digest`
/// applies exactly that plan.
fn run_tier_age(repo: &crate::cli::RepoArgs, mode: &Mode, json: bool) -> ExitCode {
    // A failure before a record moved that is a read the system failed, or a tier that did not answer, is a refusal a repeat can pass
    // (`TIER_UNAVAILABLE`); any other is `TIER_FAILED`.
    let failed = |err: String| {
        let error = if err.starts_with("policy: ") {
            // A tier the caller configured wrongly (a malformed or empty DSN) is a refusal of the configuration, which waiting does not fix.
            crate::read_cause::repo_error(err)
        } else if crate::read_cause::of_text(&err) == crate::read_cause::Cause::Unavailable || crate::read_cause::tier_did_not_answer(&err) {
            CliError::refused("TIER_UNAVAILABLE", err).hint("the tiers or the system failed in a way a repeat can pass: wait a moment, check that the tier backends are reachable, then run `canon tier age` again")
        } else {
            CliError::failed("TIER_FAILED", err).hint("check the `tiers` settings in canon.yaml and that the destinations are reachable, then run `canon tier age` again")
        };
        ExitCode::from(error.emit("canon tier age", json))
    };
    let canon_yaml_path = crate::context::resolve_canon_yaml(repo);
    let loaded = match crate::tiers::build_tiers_for_read(&canon_yaml_path) {
        Ok(loaded) => loaded,
        Err(err) => return failed(err.to_string()),
    };
    let (planned, effects, scanned) = match crate::tier::preview(&loaded) {
        Ok(planned) => planned,
        Err(err) => return failed(err.to_string()),
    };
    let root = canon_yaml_path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
    let reads = Reads::new().digest("canon.yaml", path_digest(root, &canon_yaml_path)).set("records", &scanned);
    let planned_moves = effects.len();
    let plan = match Plan::new("tier age", effects, reads) {
        Ok(plan) => plan,
        Err(err) => return ExitCode::from(err.emit("canon tier age", json)),
    };
    let commit = match plan.gate(mode) {
        Ok(commit) => commit,
        Err(err) => return ExitCode::from(err.emit("canon tier age", json)),
    };
    if !commit {
        if !json && planned.is_empty() {
            println!("{}", crate::tier::format_report(&planned).trim_end());
        }
        let report = crate::tier::report(&planned, plan.preview());
        plan.finish_preview("tier age", "the plan only lists what would move; moving takes its planDigest", json, &report);
        return ExitCode::SUCCESS;
    }
    // Records leave one rung and land in another: each local store a move touches must take it before the first record moves. A plan with nothing to move writes nothing and is not refused.
    let mut targets: Vec<std::path::PathBuf> = Vec::new();
    for rule in planned.iter().filter(|rule| rule.candidates > 0) {
        for target in crate::write_mode::move_targets(&canon_yaml_path, rule.kind, rule.source, rule.destination) {
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
    }
    let targets: Vec<&std::path::Path> = targets.iter().map(std::path::PathBuf::as_path).collect();
    if let Some(code) = crate::write_mode::refuse_unwritable(&targets, "canon tier age", json) {
        return ExitCode::from(code);
    }
    // Moving writes the hot db; the plan above only read it, so a preview or a refused digest leaves no db behind.
    let loaded = match crate::tiers::build_tiers(&canon_yaml_path) {
        Ok(loaded) => loaded,
        Err(err) => return failed(err.to_string()),
    };
    match crate::tier::apply(loaded, &planned) {
        Ok(reports) => {
            crate::say_raw!(json, "{}", crate::tier::format_report(&reports));
            if json {
                crate::json_out::data("tier age", &crate::tier::report(&reports, plan.committed()));
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            // Moving began: a record that left its tier stays moved. The plan's moves are counted again; fewer left means the run stopped part-way
            // (`TIER_INCOMPLETE`), as many means it failed before a record left (`TIER_FAILED`). A recount that cannot be made is read as part-way.
            let left = crate::tiers::build_tiers_for_read(&canon_yaml_path).ok().and_then(|loaded| crate::tier::preview(&loaded).ok()).map(|(_, effects, _)| effects.len());
            if left.is_some_and(|left| left >= planned_moves) {
                return failed(err.to_string());
            }
            crate::incomplete::read_status("read what the aging moved before it stopped: records that left their tier stay moved, the rest are still where they were");
            ExitCode::from(CliError::failed("TIER_INCOMPLETE", err.to_string()).reason("WRITE_FAILED").hint("what moved stays moved: check the `tiers` settings in canon.yaml and that the destinations are reachable, then preview with `canon tier age` and move the rest with `--commit --plan-digest <PLAN_DIGEST>`").emit("canon tier age", json))
        }
    }
}
