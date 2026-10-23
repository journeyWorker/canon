//! `canon ingest` — the handlers of the group.

use canon_model::{RunId};
use crate::cli::ingest::*;
use crate::cli_error::CliError;
use std::path::PathBuf;
use std::process::ExitCode;

pub fn run(cmd: IngestCommand) -> ExitCode {
    match cmd {
        IngestCommand::Sessions(IngestSessionsArgs { watch, interval_secs, home, repo, full, all_workspaces, json }) => run_ingest_sessions(&repo.join("canon.yaml"), home.as_deref(), watch, interval_secs, full, all_workspaces, json),
        IngestCommand::Artifacts(IngestArtifactsArgs { watch, interval_secs, repo, json, run_id }) => run_ingest_artifacts(&repo, watch, interval_secs, json, run_id.as_ref()),
        IngestCommand::Plans(IngestPlansArgs { dialect, source, repo, json }) => run_ingest_plans(&repo, dialect.as_deref(), source.as_deref(), json),
    }
}

fn run_ingest_sessions(canon_yaml: &std::path::Path, home: Option<&std::path::Path>, watch: bool, interval_secs: u64, full: bool, all_workspaces: bool, json: bool) -> ExitCode {
    let home = match home {
        Some(h) => h.to_path_buf(),
        None => match std::env::var_os("HOME") {
            Some(h) => PathBuf::from(h),
            None => {
                return ExitCode::from(CliError::failed("INGEST_FAILED", "no `--home` given and $HOME is unset").hint("check that the source is readable and run the command again").emit("canon ingest sessions", json) as u8);
            }
        },
    };

    loop {
        match crate::ingest::run(canon_yaml, &home, true, full, all_workspaces) {
            Ok(outcome) if json => {
                // One document per pass (one per poll under `--watch`); the human summary is for human mode.
                crate::json_out::data("ingest sessions", &crate::json_out::untyped(crate::ingest::summary_json(&outcome)));
            }
            Ok(outcome) => {
                print!("{}", crate::ingest::format_human(&outcome));
                // The documented JSON fallback (`crate::ingest`'s
                // module doc: "the CLI prints it as JSON rather than
                // failing the whole ingest pass") must actually fire
                // whenever there's unwritten output — ReviewS3Full
                // finding 4: this used to be gated behind a `--json`
                // flag the human-readable summary line ABOVE already
                // unconditionally claims happened ("printing JSON
                // instead"), so a default (flagless) run whose tiers
                // were unreachable discarded the only normalized
                // output. `format_json` already returns `None` when
                // every record was persisted, so this is a no-op on
                // the common path.
                if let Some(body) = crate::ingest::format_json(&outcome) {
                    println!("{body}");
                }
            }
            Err(err) => {
                return ExitCode::from(CliError::failed("INGEST_FAILED", err.to_string()).hint("check that the source is readable and run the command again").emit("canon ingest sessions", json) as u8);
            }
        }
        if !watch {
            return ExitCode::SUCCESS;
        }
        std::thread::sleep(std::time::Duration::from_secs(interval_secs));
    }
}

/// `canon ingest artifacts` (S14 `s14-artifact-ingest-cli`): see
/// `crate::artifact_ingest`'s module doc — the artifact/verdict
/// half of canon's join spine, mirroring `run_ingest_sessions`'s
/// scan-loop shape one level up (`--repo`-resolved, never `--home`).
///
/// `--run-id` is the explicit attribution edge (s42
/// `close-the-open-loops`, task 3.2) — omitted, a trajectory records no
/// run rather than a guessed one.
fn run_ingest_artifacts(repo: &std::path::Path, watch: bool, interval_secs: u64, json: bool, run_id: Option<&RunId>) -> ExitCode {
    loop {
        match crate::artifact_ingest::run(repo, run_id) {
            Ok(outcome) => {
                if json {
                    crate::next::finish(repo, "ingest artifacts", &crate::next::Scope::Repo, false, true, &outcome);
                } else {
                    print!("{}", crate::artifact_ingest::format_human(&outcome));
                }
            }
            Err(err) => {
                return ExitCode::from(CliError::failed("INGEST_FAILED", err.to_string()).hint("check that the source is readable and run the command again").emit("canon ingest artifacts", json) as u8);
            }
        }
        if !watch {
            return ExitCode::SUCCESS;
        }
        std::thread::sleep(std::time::Duration::from_secs(interval_secs));
    }
}

/// `canon ingest plans [--dialect <id> --source <path>] [--repo <dir>]
/// [--json]` (s17 P3, extended s18 P2/B1): see `crate::plans`'s
/// module doc. Prints the human summary (or `--json`'s full structured
/// outcome), then -- mirroring `run_ingest_sessions`'s own ReviewS3Full
/// finding-4 fix -- ALWAYS also prints the documented `unwritten`
/// seam's JSON body when non-empty, regardless of `--json`, so a
/// routed-but-unreachable tier's candidates are never the one copy of
/// output silently discarded by a flagless default run.
///
/// s18 `loud-plan-import-diagnostics` spec's "A malformed-nonzero,
/// zero-persisted source makes canon ingest plans non-clean at the
/// process level": whenever `PlansOutcome::non_clean_sources` is
/// non-empty, an unconditional stderr WARN (regardless of `--json`) is
/// printed per flagged source, naming its dialect, root, and malformed
/// count, and the process exits non-zero -- never the unconditional
/// `ExitCode::SUCCESS` this condition produced before this change. A
/// pass with zero flagged sources keeps exiting `0` exactly as before.
/// Distinct from the `Err(err)` arm below (a malformed CONFIGURATION,
/// s17's own `PlansError` paths), which keeps failing loud with its own
/// exit code before any source is even scanned.
fn run_ingest_plans(repo: &std::path::Path, dialect: Option<&str>, source: Option<&std::path::Path>, json: bool) -> ExitCode {
    match crate::plans::run(repo, dialect, source) {
        Ok(outcome) => {
            if json {
                if outcome.non_clean_sources.is_empty() {
                    crate::next::finish(repo, "ingest plans", &crate::next::Scope::Repo, false, true, &outcome);
                } else {
                    // The failure keeps the outcome as `data` beside the shared `error`.
                    CliError::failed("INGEST_SOURCE_UNUSABLE", "a plan source was malformed and persisted nothing").hint("fix the malformed plan source named in the output, then run `canon ingest plans` again").emit_with_data("canon ingest plans", true, &outcome);
                }
            } else {
                print!("{}", crate::plans::format_human(&outcome));
                if let Some(body) = crate::plans::format_unwritten_json(&outcome) {
                    println!("{body}");
                }
            }
            for flagged in &outcome.non_clean_sources {
                crate::say_err!(
                    json,
                    "canon ingest plans: WARN {} ({}): {} malformed construct(s), 0 persisted -- this source's pass produced nothing usable; see the named malformed entries above (or in --json) for path + reason + hint, a `root:` misconfiguration is a likely cause",
                    flagged.dialect, flagged.root, flagged.malformed
                );
            }
            if outcome.non_clean_sources.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(err) => {
            ExitCode::from(CliError::failed("INGEST_FAILED", err.to_string()).hint("check that the source is readable and run the command again").emit("canon ingest plans", json) as u8)
        }
    }
}
