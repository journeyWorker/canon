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
#[expect(unsafe_code, reason = "libc::signal is an unsafe FFI call with no safe std equivalent for restoring SIGPIPE")]
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
    // A removed spelling (the "Removed in 0.28" table): the refusal names its replacement.
    if error.use_stderr() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if let Some(removed) = crate::removed::find(&crate::cli::command(), &args) {
            let command = format!("canon {}", removed.command);
            return ExitCode::from(CliError::refused("USAGE", removed.message()).hint(removed.hint()).field(removed.field.clone()).expected([removed.replacement.clone()]).emit(command.trim(), json));
        }
    }
    if !json || !error.use_stderr() {
        error.exit();
    }
    let root = crate::cli::command();
    let (path, usage) = usage_parts(error, &root);
    ExitCode::from(usage.emit(&format!("canon {path}"), true))
}

/// The command path and the `USAGE` error a clap parse failure becomes: the
/// refused argument (`field`, `received`), what would have parsed (`expected`),
/// the nearest valid words (`suggestions`), and a working invocation (`hint`),
/// all read from the clap tree, never clap's usage text.
pub(crate) fn usage_parts(error: &clap::Error, root: &clap::Command) -> (String, CliError) {
    use clap::error::{ContextKind, ContextValue, ErrorKind};
    let text = error.render().to_string();
    let message = text.lines().find(|line| !line.trim().is_empty()).unwrap_or_default().trim().trim_start_matches("error: ").to_string();
    let string = |kind: ContextKind| match error.get(kind) {
        Some(ContextValue::String(value)) => Some(value.clone()),
        Some(ContextValue::Strings(values)) => values.first().cloned(),
        _ => None,
    };
    let path = command_path(error, root);
    let node = crate::suggest::node_at(root, &path).unwrap_or(root);
    let first_word = |arg: String| arg.split_whitespace().next().unwrap_or_default().to_string();
    let children: Vec<String> = node.get_subcommands().filter(|c| !c.is_hide_set() && c.get_name() != "help").map(|c| c.get_name().to_string()).collect();
    let mut usage = CliError::refused("USAGE", message);
    let mut suggested: Vec<String> = Vec::new();
    // For an unknown option: what the leaf does take, and why an attribution flag cannot apply to a read leaf.
    let mut unknown_option: Option<String> = None;
    match error.kind() {
        ErrorKind::InvalidSubcommand => {
            let word = string(ContextKind::InvalidSubcommand).unwrap_or_default();
            suggested = crate::suggest::commands(root, &path, &word);
            usage = usage.received(word).expected(children);
        }
        ErrorKind::MissingSubcommand => usage = usage.expected(children),
        ErrorKind::UnknownArgument => {
            let arg = first_word(string(ContextKind::InvalidArg).unwrap_or_default());
            if arg.starts_with("--") {
                suggested = crate::suggest::options(root, &path, &arg);
                usage = usage.expected(crate::suggest::long_options(node));
                unknown_option = Some(arg.clone());
            }
            usage = usage.field(arg.clone()).received(arg);
        }
        _ => {
            if let Some(arg) = string(ContextKind::InvalidArg).map(first_word) {
                usage = usage.field(arg);
            }
            if let Some(value) = string(ContextKind::InvalidValue) {
                usage = usage.received(value);
            }
            let valid: Vec<String> = match error.get(ContextKind::ValidValue) {
                Some(ContextValue::Strings(values)) => values.clone(),
                _ => usage.field_name().map(|field| possible_values(node, field)).unwrap_or_default(),
            };
            if !valid.is_empty() {
                usage = usage.expected(valid);
            }
        }
    }
    if !suggested.is_empty() {
        usage = usage.suggestions(suggested.clone());
    }
    let joined = path.join(" ");
    let example = |path: &str| crate::schema::example_for(path);
    let mut hint = match (suggested.first().and_then(|s| s.strip_prefix("canon ")).and_then(example), example(&joined), first_leaf_example(root, &path)) {
        (Some(example), _, _) | (None, Some(example), _) | (None, None, Some(example)) => format!("a working call: {example}; `{}` lists every argument", format!("canon schema {joined}").trim_end()),
        _ => "run `canon schema` to list the commands".to_string(),
    };
    if let (Some(arg), true) = (&unknown_option, node.get_subcommands().next().is_none()) {
        let takes = crate::suggest::long_options(node).join(" ");
        hint.push_str(&format!("; `canon {joined}` takes only: {takes} (and the global --as)"));
        let read = crate::schema::META.iter().any(|row| row.path == joined && row.effect == crate::schema::Effect::Read);
        if read && matches!(arg.as_str(), "--actor-id" | "--session-id" | "--role") {
            hint.push_str(&format!("; `canon {joined}` is a read leaf: it records nothing, so the attribution flags (--actor-id, --session-id, --role) do not apply to it"));
        }
    }
    (joined, usage.hint(hint))
}

/// The accepted values of the option `--name` of `node`, when it takes a closed set.
fn possible_values(node: &clap::Command, field: &str) -> Vec<String> {
    let name = field.trim_start_matches('-');
    node.get_arguments().find(|arg| arg.get_long() == Some(name)).map(|arg| arg.get_possible_values().iter().filter(|v| !v.is_hide_set()).map(|v| v.get_name().to_string()).collect()).unwrap_or_default()
}

/// The example of the first leaf at or below the group at `path`.
fn first_leaf_example(root: &clap::Command, path: &[String]) -> Option<&'static str> {
    fn walk(node: &clap::Command, path: &mut Vec<String>) -> Option<&'static str> {
        let children: Vec<&clap::Command> = node.get_subcommands().filter(|c| c.get_name() != "help").collect();
        if children.is_empty() {
            return crate::schema::example_for(&path.join(" "));
        }
        children.into_iter().find_map(|child| {
            path.push(child.get_name().to_string());
            let found = walk(child, path);
            path.pop();
            found
        })
    }
    walk(crate::suggest::node_at(root, path)?, &mut path.to_vec())
}

/// The command a parse failure happened in: read from clap's `Usage` line,
/// else the longest run of argv words that name subcommands.
fn command_path(error: &clap::Error, root: &clap::Command) -> Vec<String> {
    use clap::error::{ContextKind, ContextValue};
    let mut path: Vec<String> = Vec::new();
    if let Some(ContextValue::StyledStr(usage)) = error.get(ContextKind::Usage) {
        let usage = usage.to_string();
        let words = usage.trim().trim_start_matches("Usage:").split_whitespace().skip(1);
        for word in words {
            let mut candidate = path.clone();
            candidate.push(word.to_string());
            if crate::suggest::node_at(root, &candidate).is_none() {
                break;
            }
            path = candidate;
        }
        return path;
    }
    for word in std::env::args().skip(1).take_while(|arg| arg != "--") {
        if word.starts_with('-') {
            continue;
        }
        let mut candidate = path.clone();
        candidate.push(word);
        if crate::suggest::node_at(root, &candidate).is_none() {
            break;
        }
        path = candidate;
    }
    path
}

/// Tells the JSON envelope which repo the command works in (`meta.context.repo`):
/// the leaf's `--repo`, or `.` when it has none.
pub fn record_context(parsed: &clap::ArgMatches) {
    let mut leaf = parsed;
    while let Some((_, sub)) = leaf.subcommand() {
        leaf = sub;
    }
    if let Some(repo) = leaf.try_get_one::<PathBuf>("repo").ok().flatten() {
        crate::json_out::set_repo(repo.clone());
    }
    crate::json_out::set_json_mode(leaf.try_get_one::<bool>("json").ok().flatten().copied().unwrap_or(false));
}

/// Whether the command line asked for `--json`, read from argv because the
/// parse may not have produced a command yet.
pub fn argv_wants_json() -> bool {
    std::env::args().skip(1).take_while(|arg| arg != "--").any(|arg| arg == "--json")
}

/// A command that needs canon state refuses, `CONFIG_MISSING`, when `--repo`
/// (absent: the nearest ancestor of cwd with a `canon.yaml`; present: that
/// directory as given) holds no `canon.yaml`. The exceptions are
/// [`crate::context::CONFIG_EXEMPT`], and `canon format <DIR>` without
/// `--repo`, which validates the corpus it is handed.
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
    // A leaf without `--repo` (`schema`, `selftest`, ...) names no repo to check.
    let repo = leaf.try_get_one::<PathBuf>("repo").ok()?;
    if command == "format" && leaf.try_get_one::<PathBuf>("dir").ok().flatten().is_some() && repo.is_none() {
        return None;
    }
    let json = leaf.try_get_one::<bool>("json").ok().flatten().copied().unwrap_or(false);
    let error = crate::context::require_config(repo.map(PathBuf::as_path)).err()?;
    Some(ExitCode::from(error.emit(&format!("canon {command}"), json)))
}
