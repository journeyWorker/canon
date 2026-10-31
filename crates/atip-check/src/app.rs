//! One run of `atip`: parse, check, print, exit.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::checks::{run_l0, Judged, Mutation};
use crate::cli::{self, CheckArgs, Command, Level, CHECK_HELP, ROOT_HELP};
use crate::exec::{RunError, Runner};
use crate::fixture;
use crate::output::{failure_line, failure_text, success_line, Code, Failure};
use crate::report::Report;

/// Runs `atip` with `args` (the words after the program name) and returns its exit status: 0, 1 or 2.
pub fn main(args: &[OsString]) -> ExitCode {
    restore_sigpipe();
    let json = atip::wants_json(args);
    // A panic prints nothing of its own: in JSON mode stderr stays empty, and the failure below says what happened.
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = std::panic::catch_unwind(|| execute(args))
        .unwrap_or_else(|payload| Err(internal(&payload)));
    match outcome {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(failure) => {
            if json {
                println!("{}", failure_line(&failure));
            } else {
                eprintln!("{}", failure_text(&failure));
            }
            ExitCode::from(failure.exit())
        }
    }
}

fn internal(payload: &Box<dyn std::any::Any + Send>) -> Failure {
    let text = payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic with no message".to_string());
    Failure::new(Code::Internal, format!("atip hit a bug: {text}"))
        .hint("Report it at https://github.com/journeyWorker/atip/issues with the command line.")
}

/// Rust starts with SIGPIPE ignored, so `println!` to a closed stdout panics (exit 101). `atip` restores the default
/// action: a closed stdout ends it by the signal (spec 8.4), as `canon` does, and exit 101 never appears (spec 8.6).
#[expect(
    unsafe_code,
    reason = "libc::signal is an unsafe FFI call with no safe std equivalent for restoring SIGPIPE"
)]
fn restore_sigpipe() {
    // SAFETY: called first in `main`, before any other thread exists or any output is written; it only changes the
    // process's SIGPIPE action.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

/// The text a successful run prints on stdout: the version, a help text, the report, or the success envelope.
fn execute(args: &[OsString]) -> Result<String, Failure> {
    let json = atip::wants_json(args);
    match cli::parse(args)? {
        Command::Version => Ok(format!("atip {}", env!("CARGO_PKG_VERSION"))),
        Command::RootHelp => Ok(ROOT_HELP.to_string()),
        Command::CheckHelp => Ok(CHECK_HELP.to_string()),
        Command::Check(check) => {
            let report = check_binary(&check)?;
            if !report.passed {
                return Err(level_not_met(&report).unwrap_or_else(|failure| failure));
            }
            Ok(if json {
                success_line(&serde_json::to_value(&report).map_err(internal_error)?)
            } else {
                report.text()
            })
        }
    }
}

fn internal_error(error: impl std::fmt::Display) -> Failure {
    Failure::new(Code::Internal, format!("atip hit a bug: {error}"))
        .hint("Report it at https://github.com/journeyWorker/atip/issues with the command line.")
}

/// The failed level: exit 1 with the check list in `data`; in text mode the same list follows the error line on stderr.
fn level_not_met(report: &Report) -> Result<Failure, Failure> {
    let blocking: Vec<&str> = report
        .checks
        .iter()
        .filter(|check| check.blocks_level())
        .map(|check| check.id)
        .collect();
    let data = serde_json::to_value(report).map_err(internal_error)?;
    Ok(Failure::new(Code::LevelNotMet, format!("{} is not met: {} MUST checks fail or are unprovided ({})", report.level, blocking.len(), blocking.join(", ")))
        .in_command("check")
        .hint("Fix every check whose result is fail or unprovided in data.checks, then run atip check again.")
        .with_data(data)
        .with_detail(report.text()))
}

/// Runs the checks of the claimed level against the binary.
pub fn check_binary(args: &CheckArgs) -> Result<Report, Failure> {
    check_binary_with(args, Mutation::none()).map(|judged| judged.report)
}

/// What a test may do to a run: report checks as passed whatever they found, or treat single conditions as holding,
/// to show that the test of each notices the missing check or condition (approaches/checker.md 7.2). Returns the
/// report and the id of every condition the run judged. Not part of the checker's interface: only with the
/// `test-support` feature.
#[cfg(feature = "test-support")]
pub fn check_binary_mutated(
    args: &CheckArgs,
    forced_pass: &[&str],
    skipped_conditions: &[&str],
) -> Result<(Report, std::collections::BTreeSet<String>), Failure> {
    check_binary_with(
        args,
        Mutation {
            forced_pass,
            skipped_conditions,
        },
    )
    .map(|judged| (judged.report, judged.conditions))
}

fn check_binary_with(args: &CheckArgs, mutation: Mutation<'_>) -> Result<Judged, Failure> {
    let refuse = |code: Code, message: String, field: &str| {
        Failure::new(code, message).in_command("check").field(field)
    };
    match args.level {
        Level::L0 => {}
        Level::L1 => return Err(unsupported("L1", "0.34")),
        Level::L2 => return Err(unsupported("L2", "0.35")),
    }
    let fixture_path = Path::new(&args.fixture);
    let fixture = fixture::load(fixture_path)
        .map_err(|reason| refuse(Code::FixtureInvalid, reason, "--fixture"))?;
    let bin = resolve_binary(&args.bin)
        .map_err(|reason| refuse(Code::BinaryUnusable, reason, "--bin"))?;
    let scratch_failure = |reason: String| {
        Failure::new(
            Code::Internal,
            format!("the scratch space cannot be used: {reason}"),
        )
        .in_command("check")
        .hint("Point TMPDIR at a writable directory.")
    };
    let from_run = |error: RunError| match error {
        RunError::Spawn(reason) => refuse(
            Code::BinaryUnusable,
            format!("the binary cannot be started: {reason}"),
            "--bin",
        ),
        RunError::Setup(reason) => refuse(Code::FixtureInvalid, reason, "--fixture"),
        RunError::Scratch(reason) => scratch_failure(reason),
    };
    let runner = Runner::new(&bin, &fixture).map_err(from_run)?;
    let name = bin
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    run_l0(runner, &fixture, &name, mutation).map_err(from_run)
}

fn unsupported(level: &str, release: &str) -> Failure {
    Failure::new(
        Code::LevelUnsupported,
        format!("{level} is not supported yet: this release of atip has the L0 checks only"),
    )
    .in_command("check")
    .field("--level")
    .expected(["L0"])
    .received(level)
    .hint(format!(
        "{level} checks arrive in {release}; run atip check --level L0 until then."
    ))
}

/// The binary as an absolute path: a path as given, a bare name found on `PATH`; it must be an executable file.
fn resolve_binary(bin: &OsStr) -> Result<PathBuf, String> {
    let given = Path::new(bin);
    let path = if given.components().count() > 1 || bin.to_string_lossy().contains('/') {
        std::path::absolute(given).map_err(|error| format!("{}: {error}", given.display()))?
    } else {
        let search = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&search)
            .map(|dir| dir.join(given))
            .find(|candidate| candidate.is_file())
            .ok_or_else(|| format!("{} is not on PATH", given.display()))?
    };
    let metadata =
        std::fs::metadata(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a file", path.display()));
    }
    if std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o111 == 0 {
        return Err(format!("{} is not executable", path.display()));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::internal;
    use crate::output::{failure_line, Code};
    use crate::schema::schemas;

    #[test]
    fn atip_check_06_a_panic_is_an_internal_failure_with_exit_1_and_a_way_to_report_it() {
        let payload = std::panic::catch_unwind(|| panic!("boom")).expect_err("the closure panics");
        let failure = internal(&payload);
        assert_eq!((failure.code, failure.exit()), (Code::Internal, 1));
        assert!(
            failure.message.contains("boom")
                && failure
                    .hint
                    .as_deref()
                    .is_some_and(|hint| hint.contains("issues")),
            "{failure:?}"
        );
        let document: serde_json::Value =
            serde_json::from_str(&failure_line(&failure)).expect("json");
        assert_eq!(schemas().envelope_problems(&document), Vec::<String>::new());
        assert_eq!(document["error"]["code"], "INTERNAL");
    }
}
