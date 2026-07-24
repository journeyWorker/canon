//! `canon-report [--check] [--out <path>] [--repo <dir>] [--git-root
//! <dir>] [--r2-root <dir>] [--learn-root <dir>]` — a thin `[[bin]]`
//! entry over the library API (`lib fn + a thin [[bin]]`, S9
//! assignment #2). The FUTURE `canon-cli` `canon report` arm (part2,
//! deferred — canon-cli is mid-edit by a sibling change now) wires
//! `canon.yaml`-driven path resolution onto the exact same
//! [`canon_report::report`]/[`canon_report::write_report`]/
//! [`canon_report::check_report`] calls this binary makes directly;
//! this bin exists so the crate is independently runnable/testable
//! before that wiring lands, never as a competing entry point once it
//! does.
//!
//! No `clap`/argument-parsing dependency: five plain flags, hand-parsed
//! (mirrors `crates/canon-model/src/bin/xtask.rs`'s own no-framework
//! precedent for a small internal tool).

use std::path::PathBuf;
use std::process::ExitCode;

use canon_report::{check_report, write_report, ReportInputs, Roots};

struct Args {
    check: bool,
    out: PathBuf,
    repo_root: PathBuf,
    git_root: PathBuf,
    r2_root: PathBuf,
    learn_root: PathBuf,
}

fn parse_args() -> Args {
    let mut check = false;
    let mut repo_root = PathBuf::from(".");
    let mut out: Option<PathBuf> = None;
    let mut git_root: Option<PathBuf> = None;
    let mut r2_root: Option<PathBuf> = None;
    let mut learn_root: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--out" => out = args.next().map(PathBuf::from),
            "--repo" => repo_root = args.next().map(PathBuf::from).unwrap_or(repo_root),
            "--git-root" => git_root = args.next().map(PathBuf::from),
            "--r2-root" => r2_root = args.next().map(PathBuf::from),
            "--learn-root" => learn_root = args.next().map(PathBuf::from),
            other => {
                eprintln!("canon-report: unrecognized argument {other:?}");
                std::process::exit(2);
            }
        }
    }

    let git_root = git_root.unwrap_or_else(|| repo_root.join(canon_model::paths::LEDGER_DIR));
    let r2_root = r2_root.unwrap_or_else(|| repo_root.join(canon_model::paths::R2_LOCAL_DIR));
    let learn_root = learn_root.unwrap_or_else(|| repo_root.join(canon_model::paths::LEARN_DIR));
    let out = out.unwrap_or_else(|| repo_root.join(canon_report::render::DEFAULT_REPORT_PATH));

    Args { check, out, repo_root, git_root, r2_root, learn_root }
}

fn main() -> ExitCode {
    let args = parse_args();
    let inputs = ReportInputs::new(args.repo_root, Roots::new(args.git_root, args.r2_root, args.learn_root));

    if args.check {
        match check_report(&inputs, &args.out) {
            Ok(outcome) => {
                eprintln!("{}", outcome.message(&args.out));
                ExitCode::from(outcome.exit_code() as u8)
            }
            Err(e) => {
                eprintln!("canon-report --check: {e}");
                ExitCode::FAILURE
            }
        }
    } else {
        match write_report(&inputs, &args.out) {
            Ok(_content) => {
                eprintln!("canon report: wrote {}", args.out.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("canon-report: {e}");
                ExitCode::FAILURE
            }
        }
    }
}
