//! `canon` — the single entrypoint binary. The command tree is
//! `canon_cli::cli`; running a parsed command is `canon_cli::run`.

use std::process::ExitCode;

use canon_cli::cli::{self, Cli};
use canon_cli::input::Expanded;
use canon_cli::run::entry::{argv_wants_json, refuse_without_config, set_sigpipe, usage_error};
use clap::FromArgMatches;

fn main() -> ExitCode {
    set_sigpipe(true);
    let mut command = cli::command();
    command.build();
    // `--input` becomes ordinary arguments (or runs a batch) before clap parses.
    let args = match canon_cli::input::expand(&command, std::env::args_os().collect()) {
        Expanded::Argv(args) => args,
        Expanded::Exit(code) => return code,
    };
    let mut matches = match command.clone().try_get_matches_from(args) {
        Ok(matches) => matches,
        Err(error) => return usage_error(&error),
    };
    let parsed = matches.clone();
    canon_cli::run::entry::record_context(&parsed);
    let cli = match Cli::from_arg_matches_mut(&mut matches) {
        Ok(cli) => cli,
        Err(error) => return usage_error(&error.format(&mut command.clone())),
    };
    // The role is resolved before any command runs, so an unknown `CANON_ROLE` refuses the invocation.
    // `canon schema` and `canon context` never fail on the role: a bad `CANON_ROLE` is a warning there.
    if let Err(error) = canon_cli::role::init(cli.declared_role) {
        if !matches!(cli.command, canon_cli::cli::Command::Schema(_) | canon_cli::cli::Command::Context(_)) {
            return ExitCode::from(error.emit("canon", argv_wants_json()) as u8);
        }
        canon_cli::json_out::warn(error.message);
    }
    if let Some(code) = refuse_without_config(&parsed) {
        return code;
    }
    canon_cli::run::run(cli)
}
