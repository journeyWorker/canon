//! `canon tier` — the handlers of the group.

use crate::cli::tier::{TierCommand, TierAgeArgs};
use crate::cli_error::CliError;
use crate::safety::{Mode, Plan};
use std::process::ExitCode;

pub fn run(cmd: TierCommand) -> ExitCode {
    match cmd {
        TierCommand::Age(TierAgeArgs { safety, repo, json, .. }) => run_tier_age(&repo, &safety.mode(), json),
    }
}

/// `canon tier age`: the bare run previews the moves; `--commit --plan-digest`
/// applies exactly that plan.
fn run_tier_age(repo: &crate::cli::RepoArgs, mode: &Mode, json: bool) -> ExitCode {
    let base = crate::safety::command("tier age", &[], repo);
    let failed = |err: String| ExitCode::from(CliError::failed("TIER_FAILED", err).hint("check the `tiers` settings in canon.yaml and that the destinations are reachable, then run `canon tier age` again").emit("canon tier age", json));
    let canon_yaml_path = crate::context::resolve_canon_yaml(repo);
    let loaded = match crate::tiers::build_tiers_for_read(&canon_yaml_path) {
        Ok(loaded) => loaded,
        Err(err) => return failed(err.to_string()),
    };
    let (planned, effects) = match crate::tier::preview(&loaded) {
        Ok(planned) => planned,
        Err(err) => return failed(err.to_string()),
    };
    let plan = Plan::new(effects);
    let commit = match plan.gate(mode, &base) {
        Ok(commit) => commit,
        Err(err) => return ExitCode::from(err.emit("canon tier age", json)),
    };
    if !commit {
        if !json && planned.is_empty() {
            println!("{}", crate::tier::format_report(&planned).trim_end());
        }
        let report = crate::tier::report(&planned, plan);
        crate::safety::finish_preview(&base, "tier age", &report.plan, "the plan only lists what would move; moving takes its planDigest", json, &report);
        return ExitCode::SUCCESS;
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
        Err(err) => failed(err.to_string()),
    }
}
