//! `canon ingest` — the handlers of the group.

use canon_model::{RunId};
use crate::cli::ingest::{IngestCommand, IngestSessionsArgs, IngestRecordsArgs, IngestPlansArgs};
use crate::cli_error::{CliError, CliErrorExt as _};
use std::path::PathBuf;
use std::process::ExitCode;

pub fn run(cmd: IngestCommand) -> ExitCode {
    match cmd {
        IngestCommand::Sessions(IngestSessionsArgs { watch, interval_secs, home, repo, full, all_workspaces, dry_run, json, .. }) => run_ingest_sessions(&repo.join("canon.yaml"), home.as_deref(), SessionsOptions { watch, interval_secs, full, all_workspaces, dry_run: dry_run.dry_run, json }),
        IngestCommand::Records(IngestRecordsArgs { watch, interval_secs, repo, dry_run, json, run_id, .. }) => run_ingest_records(&repo, watch, interval_secs, dry_run.dry_run, json, run_id.as_ref()),
        IngestCommand::Plans(IngestPlansArgs { dialect, plan_source, repo, dry_run, json, .. }) => run_ingest_plans(&repo, dialect.as_deref(), plan_source.as_deref(), dry_run.dry_run, json),
    }
}

/// The flags of `canon ingest sessions` that shape one run.
#[derive(Debug, Clone, Copy)]
struct SessionsOptions {
    watch: bool,
    interval_secs: u64,
    full: bool,
    all_workspaces: bool,
    dry_run: bool,
    json: bool,
}

fn run_ingest_sessions(canon_yaml: &std::path::Path, home: Option<&std::path::Path>, options: SessionsOptions) -> ExitCode {
    let SessionsOptions { watch, interval_secs, full, all_workspaces, dry_run, json } = options;
    let home = match home {
        Some(h) => h.to_path_buf(),
        None => match std::env::var_os("HOME") {
            Some(h) => PathBuf::from(h),
            None => {
                return ExitCode::from(CliError::failed("INGEST_FAILED", "no `--home` given and $HOME is unset").hint("check that the source is readable and run the command again").emit("canon ingest sessions", json));
            }
        },
    };

    let root = canon_yaml.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
    loop {
        // The repository write lock for this pass alone: a polling run must not hold it while it sleeps.
        let pass = match crate::write_lock::hold_pass(root, dry_run) {
            Ok(pass) => pass,
            Err(error) => return ExitCode::from(error.emit("canon ingest sessions", json)),
        };
        match crate::ingest::run(canon_yaml, &home, true, full, all_workspaces, dry_run) {
            Ok(outcome) if json => {
                // One document per pass (one per poll under `--watch`); the human summary is for human mode.
                crate::json_out::data("ingest sessions", &crate::ingest::summary_json(&outcome, crate::hot_store::location(canon_yaml)));
            }
            Ok(outcome) => {
                print!("{}", crate::ingest::format_human(&outcome));
                if !outcome.dry_run {
                    if let Some(hot) = crate::hot_store::HotStore::of(canon_yaml) {
                        println!("{}", hot.line());
                    }
                }
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
                let error = match &err {
                    crate::ingest::IngestError::Config(message) => crate::read_cause::config_error(message.clone()).hint("fix the `ingest:` section of canon.yaml the message names, then run the command again"),
                    crate::ingest::IngestError::Unavailable(error) => error.clone(),
                    _ => CliError::failed("INGEST_FAILED", err.to_string()).hint("check that the source is readable and run the command again"),
                };
                return ExitCode::from(error.emit("canon ingest sessions", json));
            }
        }
        if !watch {
            return ExitCode::SUCCESS;
        }
        drop(pass);
        std::thread::sleep(std::time::Duration::from_secs(interval_secs));
    }
}

/// `canon ingest records` (S14 `s14-artifact-ingest-cli`): see
/// `crate::artifact_ingest`'s module doc — the artifact/verdict
/// half of canon's join spine, mirroring `run_ingest_sessions`'s
/// scan-loop shape one level up (`--repo`-resolved, never `--home`).
///
/// `--run` is the explicit attribution edge (s42
/// `close-the-open-loops`, task 3.2) — omitted, a trajectory records no
/// run rather than a guessed one.
fn run_ingest_records(repo: &std::path::Path, watch: bool, interval_secs: u64, dry_run: bool, json: bool, run_id: Option<&RunId>) -> ExitCode {
    loop {
        // The repository write lock for this pass alone: a polling run must not hold it while it sleeps.
        let pass = match crate::write_lock::hold_pass(repo, dry_run) {
            Ok(pass) => pass,
            Err(error) => return ExitCode::from(error.emit("canon ingest records", json)),
        };
        match crate::artifact_ingest::run(repo, run_id, dry_run) {
            Ok(mut outcome) => {
                let hot = crate::hot_store::HotStore::of(&repo.join("canon.yaml")).filter(|_| !outcome.dry_run);
                outcome.hot_store = hot.as_ref().map(crate::hot_store::HotStore::location);
                if json {
                    crate::next::finish(repo, "ingest records", &crate::next::Scope::Repo, false, true, &outcome);
                } else {
                    print!("{}", crate::artifact_ingest::format_human(&outcome));
                    if let Some(hot) = &hot {
                        println!("{}", hot.line());
                    }
                }
            }
            Err(err) => {
                let error = match &err {
                    crate::artifact_ingest::ArtifactIngestError::Unavailable(error) => error.clone(),
                    _ => CliError::failed("INGEST_FAILED", err.to_string()).hint("check that the source is readable and run the command again"),
                };
                return ExitCode::from(error.emit("canon ingest records", json));
            }
        }
        if !watch {
            return ExitCode::SUCCESS;
        }
        drop(pass);
        std::thread::sleep(std::time::Duration::from_secs(interval_secs));
    }
}

/// `canon ingest plans [--dialect <id> --plan-source <path>] [--repo <dir>]
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
/// process level", as 0.36 answers it: a source with malformed
/// constructs and none that parses is `INGEST_SOURCE_INVALID`, a refusal
/// (exit 2, no `data`) judged before anything is persisted, with one
/// `error.details` entry for each malformed construct. A pass with no
/// such source exits `0`. Distinct from the `Err(err)` arm below for a
/// malformed CONFIGURATION, which keeps failing loud with its own code.
fn run_ingest_plans(repo: &std::path::Path, dialect: Option<&str>, source: Option<&std::path::Path>, dry_run: bool, json: bool) -> ExitCode {
    match crate::plans::run(repo, dialect, source, dry_run) {
        Ok(mut outcome) => {
            let hot = crate::hot_store::HotStore::of(&repo.join("canon.yaml")).filter(|_| !outcome.dry_run);
            outcome.hot_store = hot.as_ref().map(crate::hot_store::HotStore::location);
            if json {
                crate::next::finish(repo, "ingest plans", &crate::next::Scope::Repo, false, true, &outcome);
            } else {
                print!("{}", crate::plans::format_human(&outcome));
                if let Some(hot) = &hot {
                    println!("{}", hot.line());
                }
                if let Some(body) = crate::plans::format_unwritten_json(&outcome) {
                    println!("{body}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            let error = match &err {
                crate::plans::PlansError::Unavailable(error) => error.clone(),
                // Several malformed constructs are `error.details`, one entry each: nothing was persisted.
                crate::plans::PlansError::SourcesInvalid(causes) => crate::gate_refusal::refusal(
                    causes.iter().map(|cause| crate::gate_refusal::Cause { code: "INGEST_SOURCE_INVALID", message: cause.clone() }).collect(),
                    "fix the plan source named (a `root:` that points one level too high is the usual cause), then run `canon ingest plans` again; nothing was persisted",
                ),
                _ => CliError::failed("INGEST_FAILED", err.to_string()).hint("check that the source is readable and run the command again"),
            };
            ExitCode::from(error.emit("canon ingest plans", json))
        }
    }
}
