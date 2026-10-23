//! What `main` needs around the parse: SIGPIPE, usage errors, the
//! missing-config refusal.

use std::path::PathBuf;
use std::process::ExitCode;

use crate::cli_error::CliError;

/// Rust starts with SIGPIPE ignored, so every write to a stdout whose reader
/// has gone (`canon status | head`) fails and `println!` panics (exit 101).
/// canon restores the default action: the process ends quietly on SIGPIPE,
/// as other command-line tools do (0.20). `dashboard` serves sockets and
/// keeps it ignored, so a client that disconnects never kills the server.
#[cfg(unix)]
pub fn set_sigpipe(default: bool) {
    let action = if default { libc::SIG_DFL } else { libc::SIG_IGN };
    // SAFETY: called on the main thread before any other thread exists or
    // any output is written; it only changes the process's SIGPIPE action.
    unsafe {
        libc::signal(libc::SIGPIPE, action);
    }
}

#[cfg(not(unix))]
pub fn set_sigpipe(_default: bool) {}

/// A clap usage error (0.20 D3). Help and version requests print as clap
/// prints them. Any other parse failure keeps clap's own text and exit `2`,
/// unless `--json` is in argv: then it is the shared `USAGE` error, so a
/// caller that asked for JSON gets JSON on stdout.
pub fn usage_error(error: &clap::Error) -> ExitCode {
    let json = argv_wants_json();
    // A spelling 0.24 deprecated and 0.25 removed: the refusal names its replacement (0.25 D6).
    // A leaf whose primary object moved into operand position, or that split (0.28).
    if error.use_stderr() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if let Some(shape) = crate::removed::find_shape(&crate::cli::command(), &args) {
            let command = format!("canon {}", shape.command);
            return ExitCode::from(CliError::refused("USAGE", shape.message()).hint(shape.hint).field(shape.used.clone()).emit(command.trim(), json) as u8);
        }
    }
    if error.use_stderr() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if let Some(removed) = crate::removed::find(&crate::cli::command(), &args) {
            let command = format!("canon {}", removed.command);
            return ExitCode::from(CliError::refused("USAGE", removed.message()).hint(removed.hint()).field(removed.field.clone()).expected([removed.replacement.clone()]).emit(command.trim(), json) as u8);
        }
    }
    if error.use_stderr() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if let Some(bare) = crate::removed::bare_group(&crate::cli::command(), &args) {
            let command = format!("canon {}", bare.command);
            return ExitCode::from(CliError::refused("USAGE", bare.message()).hint(bare.hint()).field(bare.command).expected([format!("canon {}", bare.replacement)]).emit(&command, json) as u8);
        }
    }
    if !json || !error.use_stderr() {
        error.exit();
    }
    let text = error.render().to_string();
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let message = lines.next().unwrap_or_default().trim_start_matches("error: ").to_string();
    let hint = lines.map(str::trim).collect::<Vec<_>>().join(" ");
    let hint = if hint.is_empty() { "run `canon schema` to list the commands, or add --help for the command's arguments".to_string() } else { hint };
    let mut usage = CliError::refused("USAGE", message).hint(hint);
    // The offending argument, and its accepted values when clap knows them.
    use clap::error::{ContextKind, ContextValue};
    if let Some(ContextValue::String(arg)) = error.get(ContextKind::InvalidArg) {
        usage = usage.field(arg.split_whitespace().next().unwrap_or(arg));
    }
    if let Some(ContextValue::Strings(values)) = error.get(ContextKind::ValidValue) {
        usage = usage.expected(values.iter().cloned());
    }
    ExitCode::from(usage.emit("canon", true) as u8)
}

/// Whether the command line asked for `--json`, read from argv because the
/// parse may not have produced a command yet.
pub fn argv_wants_json() -> bool {
    std::env::args().skip(1).take_while(|arg| arg != "--").any(|arg| arg == "--json")
}

/// A command that needs canon state refuses, `CONFIG_MISSING`, when `--repo`
/// (absent or `.`: the nearest ancestor of cwd with a `canon.yaml`; anything
/// else: that directory as given) holds no `canon.yaml`. The exceptions are
/// [`crate::context::CONFIG_EXEMPT`]; `--canon-yaml`, where a command takes
/// one, names the file itself and the command reports a missing one; and
/// `canon format` without `--repo`, which validates the corpus it is handed.
pub fn refuse_without_config(parsed: &clap::ArgMatches) -> Option<ExitCode> {
    let mut names = Vec::new();
    let mut leaf = parsed;
    while let Some((name, sub)) = leaf.subcommand() {
        names.push(name);
        leaf = sub;
    }
    let command = names.join(" ");
    if !crate::context::needs_config(&command) {
        return None;
    }
    let repo = leaf.try_get_one::<PathBuf>("repo").ok().flatten()?;
    if leaf.try_get_one::<PathBuf>("canon_yaml").ok().flatten().is_some() {
        return None;
    }
    let json = leaf.try_get_one::<bool>("json").ok().flatten().copied().unwrap_or(false);
    let error = crate::context::require_config(repo).err()?;
    Some(ExitCode::from(error.emit(&format!("canon {command}"), json) as u8))
}
