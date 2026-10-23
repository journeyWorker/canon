//! Writes canon's man pages (`canon.1`, `canon-gate-check.1`, one per command
//! node) from the clap tree, into the directory given as the one argument
//! (default `man`).
//!
//! A packaging tool, never part of the shipped binary: it needs the `man`
//! feature, which no shipped build turns on.
//!
//! ```text
//! cargo run --locked -p canon-cli --features man --example canon-man -- packages/cli/man
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let dir = std::env::args_os().nth(1).map_or_else(|| PathBuf::from("man"), PathBuf::from);
    if let Err(error) = std::fs::create_dir_all(&dir).and_then(|()| clap_mangen::generate_to(canon_cli::cli::command(), &dir)) {
        eprintln!("canon-man: cannot write the man pages into {}: {error}", dir.display());
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
