//! `canon tier` — the handlers of the group.

use crate::cli::tier::*;
use crate::cli_error::CliError;
use std::process::ExitCode;

pub fn run(cmd: TierCommand) -> ExitCode {
    match cmd {
        TierCommand::Age(TierAgeArgs { dry_run, repo, canon_yaml, json }) => run_tier_age(&repo, canon_yaml.as_deref(), dry_run, json),
    }
}

fn run_tier_age(repo: &std::path::Path, canon_yaml: Option<&std::path::Path>, dry_run: bool, json: bool) -> ExitCode {
    let canon_yaml_path = crate::context::resolve_canon_yaml(repo, canon_yaml);
    match crate::tier::run(&canon_yaml_path, dry_run) {
        Ok(reports) => {
            crate::say_raw!(json, "{}", crate::tier::format_report(&reports, dry_run));
            if json {
                crate::json_out::data("tier age", &crate::json_out::untyped(crate::tier::format_json(&reports, dry_run)));
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            ExitCode::from(CliError::failed("TIER_FAILED", err.to_string()).hint("check the `tiers` settings in canon.yaml and that the destinations are reachable, then run `canon tier age` again").emit("canon tier age", json) as u8)
        }
    }
}
