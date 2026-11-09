//! `canon ingest` — the handlers of the group.

use canon_model::{RunId};
use crate::cli::ingest::{IngestCommand, IngestRecordsArgs, IngestPlansArgs};
use crate::cli_error::{CliError, CliErrorExt as _};
use std::process::ExitCode;

pub fn run(cmd: IngestCommand) -> ExitCode {
    match cmd {
        IngestCommand::Records(IngestRecordsArgs { watch, interval_secs, repo, dry_run, json, run_id, .. }) => run_ingest_records(&repo, watch, interval_secs, dry_run.dry_run, json, run_id.as_ref()),
        IngestCommand::Plans(IngestPlansArgs { dialect, plan_source, repo, dry_run, json, .. }) => run_ingest_plans(&repo, dialect.as_deref(), plan_source.as_deref(), dry_run.dry_run, json),
    }
}

/// `canon ingest records` (S14 `s14-artifact-ingest-cli`): see
/// `crate::artifact_ingest`'s module doc — the artifact/verdict
/// half of canon's join spine, `--repo`-resolved scan loop.
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
/// outcome), then ALWAYS also prints the documented `unwritten`
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
                // canon.yaml, where the sources are configured, cannot be read: nothing was persisted, and it is the refusal every plan-corpus
                // reader gives (`PLAN_CORPUS_UNAVAILABLE`, `CONFIG_DENIED` or `CONFIG_INVALID`).
                crate::plans::PlansError::CanonYamlRead { .. } => err.cli_error().expect("a canon.yaml read has its refusal"),
                // Several malformed constructs are `error.details`, one entry each: nothing was persisted.
                crate::plans::PlansError::SourcesInvalid(causes) => crate::gate_refusal::refusal(
                    causes.iter().map(|cause| crate::gate_refusal::Cause::new("INGEST_SOURCE_INVALID", cause.clone())).collect(),
                    "fix the plan source named (a `root:` that points one level too high is the usual cause), then run `canon ingest plans` again; nothing was persisted",
                ),
                _ => CliError::failed("INGEST_FAILED", err.to_string()).hint("check that the source is readable and run the command again"),
            };
            ExitCode::from(error.emit("canon ingest plans", json))
        }
    }
}
