//! `canon` — the single entrypoint binary. The command tree is
//! `canon_cli::cli`; running a parsed command is `canon_cli::run`.

use std::ffi::OsString;
use std::process::ExitCode;

use canon_cli::cli::{self, Cli};
use canon_cli::cli_error::{CliError, CliErrorExt as _};
use canon_cli::input::Expanded;
use canon_cli::run::entry::{record_context, refuse_without_config, set_sigpipe, usage_error};
use clap::FromArgMatches;

/// Where to report a bug: the message of an `INTERNAL` error names it.
const ISSUES: &str = "https://github.com/journeyWorker/canon/issues";

fn main() -> ExitCode {
    set_sigpipe(true);
    install_panic_hook();
    // A bug must not end the process with Rust's exit 101 and a backtrace on stderr: it is the `INTERNAL`
    // error (exit 1), in the JSON envelope when `--json` is in argv.
    match std::panic::catch_unwind(run) {
        Ok(code) => code,
        Err(_) => {
            let message = PANIC.lock().map(|mut slot| slot.take()).ok().flatten().unwrap_or_else(|| "a panic with no message".to_string());
            let error = CliError::failed("INTERNAL", format!("canon hit a bug: {message}")).hint(format!("report it at {ISSUES} with the command line you ran and this message"));
            ExitCode::from(error.emit("canon", atip::argv_wants_json()))
        }
    }
}

/// What the panic hook saw, for the `INTERNAL` error `main` prints.
static PANIC: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Keeps a panic on the main thread off stderr (the error `main` prints carries it); a panic on
/// another thread (the dashboard's) is reported as Rust does.
fn install_panic_hook() {
    let default = std::panic::take_hook();
    let main_thread = std::thread::current().id();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().id() != main_thread {
            default(info);
            return;
        }
        let payload = info.payload();
        let text = payload.downcast_ref::<&str>().map(|s| (*s).to_string()).or_else(|| payload.downcast_ref::<String>().cloned()).unwrap_or_else(|| "a panic with no message".to_string());
        let at = info.location().map(|location| format!(" at {}:{}", location.file(), location.line())).unwrap_or_default();
        if let Ok(mut slot) = PANIC.lock() {
            *slot = Some(format!("{text}{at}"));
        }
    }));
}

/// `--json` anywhere before the first `--` is moved to the end of the words before it, so the leaf
/// that takes it sees it wherever the caller put it (`canon --json status`, `canon status --json`).
fn relocate_json(args: Vec<OsString>) -> Vec<OsString> {
    // `canon help <command>` explains a command in text; `--json` there is not a word to explain.
    if args.iter().skip(1).take_while(|arg| *arg != "--").find(|arg| !arg.to_string_lossy().starts_with('-')).is_some_and(|word| word == "help") {
        return args.into_iter().filter(|arg| arg != "--json").collect();
    }
    let end = args.iter().skip(1).position(|arg| arg == "--").map_or(args.len(), |at| at + 1);
    if !args[1..end].iter().any(|arg| arg == "--json") {
        return args;
    }
    let mut moved: Vec<OsString> = args[..end].iter().filter(|arg| *arg != "--json").cloned().collect();
    moved.push(OsString::from("--json"));
    moved.extend(args[end..].iter().cloned());
    moved
}

fn run() -> ExitCode {
    #[cfg(debug_assertions)]
    if std::env::var_os("CANON_TEST_PANIC").is_some() {
        panic!("CANON_TEST_PANIC asked for a panic");
    }
    let mut command = cli::command();
    command.build();
    // `--input` becomes ordinary arguments (or runs a batch) before clap parses.
    let args = match canon_cli::input::expand(&command, canon_cli::run::entry::hoist_tool_wide_options(&command, relocate_json(std::env::args_os().collect()))) {
        Expanded::Argv(args) => args,
        Expanded::Exit(code) => return code,
    };
    // The order of ATIP 11.7 step 6: a failure to parse, `required`, the domains, the constraints, (below) the validators.
    let mut matches = match canon_cli::validators::parse(&command, args) {
        Ok(matches) => matches,
        Err(code) => return code,
    };
    let parsed = matches.clone();
    record_context(&command, &parsed);
    canon_cli::paging::bind_arguments(&command, &parsed);
    let cli = match Cli::from_arg_matches_mut(&mut matches) {
        Ok(cli) => cli,
        Err(error) => return usage_error(&error.format(&mut command.clone())),
    };
    // The role is resolved before any command runs. An unknown `CANON_ROLE` is a check that depends on an environment value, so it is a
    // validator and answers after the others (spec 10.8.1).
    // `canon context` and `canon skills show` never fail on the role: a bad `CANON_ROLE` is a warning there.
    // `canon schema` does not read `CANON_ROLE` at all (spec 10.2.2, 16.3): only its own `--as` declares a role there.
    let unknown_role = if matches!(&cli.command, canon_cli::cli::Command::Schema(_)) {
        canon_cli::role::init_flag_only(cli.declared_role);
        None
    } else {
        canon_cli::role::declare(cli.declared_role)
    };
    let unknown_role = unknown_role.and_then(|error| {
        let role_free = matches!(&cli.command, canon_cli::cli::Command::Context(_) | canon_cli::cli::Command::Skills { action: canon_cli::cli::SkillsCommand::Show(_) });
        if role_free {
            canon_cli::json_out::warn("UNKNOWN_ROLE_IGNORED", error.message);
            return None;
        }
        Some(error)
    });
    // The validators (ATIP 10.8.3) answer before any precondition or state read.
    if let Some(code) = canon_cli::validators::check(&cli, &command, &parsed) {
        return code;
    }
    if let Some(error) = unknown_role {
        return ExitCode::from(error.emit("canon", atip::argv_wants_json()));
    }
    if let Some(code) = refuse_without_config(&command, &parsed) {
        return code;
    }
    if let Some(code) = canon_cli::old_layout::refuse_write(&command, &parsed) {
        return code;
    }
    canon_cli::run::run(cli)
}
