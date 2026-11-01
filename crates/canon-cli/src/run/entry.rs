//! What `main` needs around the parse: SIGPIPE, usage errors, the
//! missing-config refusal.

use std::path::PathBuf;
use std::process::ExitCode;

use crate::cli_error::{CliError, CliErrorExt as _};

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

/// A clap parse failure (0.20 D3, 0.33 F5). Help and version requests print as
/// clap prints them (stdout, exit 0). Any other parse failure is a shared
/// error with its own code: `UNKNOWN_COMMAND` for a command word the tree does
/// not have, `COMMAND_REQUIRED` for the bare tool or a group, `USAGE` for the
/// rest. It is a JSON failure envelope when `--json` is in argv (before `--`)
/// and, in text mode, `canon: <CODE>: <message>` on stderr; never clap's own text.
pub fn usage_error(error: &clap::Error) -> ExitCode {
    let json = atip::argv_wants_json();
    if !error.use_stderr() {
        error.exit();
    }
    // A removed spelling (the removed-spellings table): the refusal names its replacement.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let removed = crate::removed::find(&crate::cli::command(), &args);
    if let Some(removed) = removed.as_ref().filter(|removed| !removed.is_bare()) {
        let command = format!("canon {}", removed.command);
        return ExitCode::from(CliError::refused("USAGE", removed.message()).hint(removed.hint()).field(removed.field.clone()).expected([removed.expected()]).emit(command.trim(), json));
    }
    let root = crate::cli::command();
    let (path, mut usage) = usage_parts(error, &root);
    // A group that once ran bare (`canon brief`): still the contract's COMMAND_REQUIRED, and the hint names what replaced it.
    if let Some(removed) = removed {
        if usage.code == "COMMAND_REQUIRED" {
            usage = usage.hint(removed.hint());
        }
    }
    ExitCode::from(usage.emit(&format!("canon {path}"), json))
}

/// The command path and the error a clap parse failure becomes: the refused
/// argument (`field`, `received`), what would have parsed (`expected`), the
/// nearest valid words (`suggestions`), and a working invocation (`hint`), all
/// read from the clap tree, never clap's usage text. The code is
/// `UNKNOWN_COMMAND` for a command word the tree lacks, `COMMAND_REQUIRED` for
/// a group or the bare tool, `USAGE` otherwise.
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
    let joined = path.join(" ");
    let at = if joined.is_empty() { "canon".to_string() } else { format!("canon {joined}") };
    let mut usage = CliError::refused("USAGE", message.clone());
    let mut suggested: Vec<String> = Vec::new();
    // For an unknown option: what the leaf does take, and why an attribution flag cannot apply to a read leaf.
    let mut unknown_option: Option<String> = None;
    let missing_command = matches!(error.kind(), ErrorKind::MissingSubcommand | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand) && !children.is_empty();
    match error.kind() {
        ErrorKind::InvalidSubcommand => {
            let word = string(ContextKind::InvalidSubcommand).unwrap_or_default();
            suggested = crate::suggest::commands(root, &path, &word);
            usage = CliError::refused("UNKNOWN_COMMAND", format!("`{word}` is not a command of `{at}`")).field("command").received(word).expected(children);
        }
        _ if missing_command => usage = CliError::refused("COMMAND_REQUIRED", format!("`{at}` needs a command")).field("command").expected(children),
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => usage = CliError::refused("USAGE", format!("`{at}` needs arguments")),
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
                usage = usage.field(crate::arg_rules::argument_name(&arg));
            }
            if error.kind() == ErrorKind::ArgumentConflict {
                // The violation names the first argument of the rule at fault, in the rule's own order, not the one clap met last (spec 10.8.1).
                if let Some(field) = crate::arg_rules::first_fault(node, &joined, &std::env::args().skip(1).collect::<Vec<_>>(), &[]) {
                    usage = usage.field(field);
                }
            }
            if error.kind() == ErrorKind::MissingRequiredArgument {
                if let Some(ContextValue::Strings(names)) = error.get(ContextKind::InvalidArg) {
                    let names: Vec<String> = names.iter().flat_map(|name| crate::arg_rules::missing_names(name)).collect();
                    let field = crate::arg_rules::first_fault(node, &joined, &std::env::args().skip(1).collect::<Vec<_>>(), &names).or_else(|| names.first().cloned()).unwrap_or_default();
                    usage = CliError::refused("USAGE", format!("the following required arguments were not provided: {}", names.join(", "))).field(field);
                }
            }
            // clap's range sentence for an integer (`-1 is not in 0..=4294967295`) must not contradict a declared minimum: the declared domain speaks.
            if message.contains(" is not in ") {
                if let (Some(field), Some(value)) = (usage.field_name().map(str::to_string), string(ContextKind::InvalidValue)) {
                    if let Some(says) = crate::arg_rules::enforce::declared_integer_says(node, &joined, &field) {
                        usage.message = format!("invalid value '{value}' for '{field}': {says}");
                    }
                }
            }
            if let Some(value) = string(ContextKind::InvalidValue) {
                // clap reports an option with no value (`--change --json`) as an empty value too: only a value the
                // caller sent, empty or not, is `received` (spec 7.1.1); a value never sent leaves it out.
                if !value.is_empty() || usage.field_name().is_none_or(|field| sent_empty_value(field, &std::env::args().collect::<Vec<_>>())) {
                    usage = usage.received(value);
                }
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
        // `suggestions` are close matches from `expected`: a command word, not the whole path.
        let words: Vec<String> = suggested.iter().map(|s| s.rsplit(' ').next().unwrap_or(s).to_string()).collect();
        usage = usage.suggestions(words);
    }
    let example = |path: &str| crate::schema::example_for(path);
    let mut hint = match (suggested.first().and_then(|s| s.strip_prefix("canon ")).and_then(example), example(&joined), first_leaf_example(root, &path)) {
        (Some(example), _, _) | (None, Some(example), _) | (None, None, Some(example)) => format!("a working call: {example}; `{}` lists every argument", format!("canon schema {joined} --json").replace("schema  --json", "schema --json")),
        _ => "run `canon schema --json` to list the commands".to_string(),
    };
    if let (Some(arg), true) = (&unknown_option, node.get_subcommands().next().is_none()) {
        let takes = crate::suggest::long_options(node).join(" ");
        hint.push_str(&format!("; `canon {joined}` takes only: {takes} (and the global --as)"));
        let read = crate::schema::META.iter().any(|row| row.path == joined && row.effect == crate::schema::Effect::Read);
        if read && matches!(arg.as_str(), "--actor" | "--session" | "--role") {
            hint.push_str(&format!("; `canon {joined}` is a read leaf: it records nothing, so the attribution flags (--actor, --session, --role) do not apply to it"));
        }
    }
    (joined, usage.hint(hint))
}

/// Tool-wide options a group (or the bare tool) takes without using them (spec 9.13): `--repo` and `--as`, with their
/// value. `--json` is moved by `relocate_json`.
fn is_tool_wide(arg: &str) -> Option<bool> {
    match arg {
        "--repo" | "--as" => Some(true),
        _ if arg.starts_with("--repo=") || arg.starts_with("--as=") => Some(false),
        _ => None,
    }
}

/// `--repo` and `--as` given after a group word (`canon finding --repo . list`) or before the command (`canon --as
/// planner status`) are accepted wherever `--json` is (spec 9.13.1): they are moved to the end of the words before
/// the first `--`, where the leaf that takes them sees them. Where no leaf follows (`canon finding --repo . --json`)
/// they do nothing and are dropped, so the answer is `COMMAND_REQUIRED`, not `USAGE` for an option the group "does
/// not take". An option a group does not know is left where it is.
pub fn hoist_tool_wide_options(root: &clap::Command, args: Vec<std::ffi::OsString>) -> Vec<std::ffi::OsString> {
    let end = args.iter().skip(1).position(|arg| arg == "--").map_or(args.len(), |at| at + 1);
    let mut node = root;
    let mut kept: Vec<std::ffi::OsString> = vec![args[0].clone()];
    let mut hoisted: Vec<std::ffi::OsString> = Vec::new();
    let mut at = 1;
    while at < end {
        let word = args[at].to_string_lossy().into_owned();
        let is_group = node.get_subcommands().any(|sub| sub.get_name() != "help");
        if let (true, Some(takes_value)) = (is_group, is_tool_wide(&word)) {
            hoisted.push(args[at].clone());
            if takes_value && at + 1 < end {
                hoisted.push(args[at + 1].clone());
                at += 1;
            }
            at += 1;
            continue;
        }
        if word.starts_with('-') {
            // `--json` stays where `relocate_json` put it; any other option belongs to the leaf from here on.
            if word == "--json" {
                kept.push(args[at].clone());
                at += 1;
                continue;
            }
            break;
        }
        match node.get_subcommands().find(|sub| sub.get_name() == word || sub.get_all_aliases().any(|alias| alias == word)) {
            Some(sub) => {
                kept.push(args[at].clone());
                node = sub;
                at += 1;
            }
            None => break,
        }
    }
    if hoisted.is_empty() {
        return args;
    }
    let leaf_follows = !node.get_subcommands().any(|sub| sub.get_name() != "help");
    let mut out = kept;
    out.extend(args[at..end].iter().cloned());
    if leaf_follows {
        out.extend(hoisted);
    }
    out.extend(args[end..].iter().cloned());
    out
}

/// Whether `args` give the option `field` an empty value (`--change ""`, `--change=`). An operand (`field` not
/// starting with `--`) has no value to leave out: clap reports its absence as a missing argument, not an empty value.
fn sent_empty_value(field: &str, args: &[String]) -> bool {
    if !field.starts_with("--") {
        return true;
    }
    let end = args.iter().position(|arg| arg == "--").unwrap_or(args.len());
    let words = &args[..end];
    words.iter().enumerate().any(|(at, word)| (word == field && words.get(at + 1).is_some_and(String::is_empty)) || *word == format!("{field}="))
}

/// The accepted values of the option `--name` of `node`, when it takes a closed set.
fn possible_values(node: &clap::Command, field: &str) -> Vec<String> {
    let name = field.trim_start_matches('-');
    // A switch has the values `true` and `false` only to clap: they are not a set the caller chooses from.
    node.get_arguments().find(|arg| arg.get_long() == Some(name) && arg.get_action().takes_values()).map(|arg| arg.get_possible_values().iter().filter(|v| !v.is_hide_set()).map(|v| v.get_name().to_string()).collect()).unwrap_or_default()
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
/// the leaf's `--repo` (or the discovered repo) when the leaf takes `--repo`,
/// nothing when it takes no context option; and whether the run is in JSON mode.
pub fn record_context(command_tree: &clap::Command, parsed: &clap::ArgMatches) {
    let mut leaf = parsed;
    let mut names = Vec::new();
    while let Some((name, sub)) = leaf.subcommand() {
        names.push(name);
        leaf = sub;
    }
    if leaf_takes_repo(command_tree, &names) {
        crate::json_out::set_repo(leaf.try_get_one::<PathBuf>("repo").ok().flatten().cloned());
    } else {
        crate::json_out::set_repo_unused();
    }
    atip::set_json_mode(atip::argv_wants_json() || leaf.try_get_one::<bool>("json").ok().flatten().copied().unwrap_or(false));
}

/// Whether the leaf at `names` (`["gate", "check"]`) declares a `--repo` option. Asked of the command tree, not
/// of the matches: `ArgMatches::try_get_one` reports an undeclared id as an error only in debug builds, and in a
/// release build answers `Ok(None)`, which read as "no `--repo` given" and refused `canon schema` outside a repo.
pub fn leaf_takes_repo(command: &clap::Command, names: &[&str]) -> bool {
    let mut leaf = command;
    for name in names {
        match leaf.find_subcommand(name) {
            Some(sub) => leaf = sub,
            None => return false,
        }
    }
    leaf.get_arguments().any(|arg| arg.get_id() == "repo")
}

/// A command that needs canon state refuses, `CONFIG_MISSING`, when `--repo`
/// (absent: the nearest ancestor of cwd with a `canon.yaml`; present: that
/// directory as given) holds no `canon.yaml`. The exceptions are
/// [`crate::context::CONFIG_EXEMPT`], a leaf without `--repo` (`schema`,
/// `selftest`, ...: it names no repo to check), and `canon format <DIR>`
/// without `--repo`, which validates the corpus it is handed.
pub fn refuse_without_config(command_tree: &clap::Command, parsed: &clap::ArgMatches) -> Option<ExitCode> {
    let mut names = Vec::new();
    let mut leaf = parsed;
    while let Some((name, sub)) = leaf.subcommand() {
        names.push(name);
        leaf = sub;
    }
    let command = names.join(" ");
    if !crate::context::needs_config(&command) || !leaf_takes_repo(command_tree, &names) {
        return None;
    }
    let repo = leaf.get_one::<PathBuf>("repo");
    if command == "format" && leaf.try_get_one::<PathBuf>("dir").ok().flatten().is_some() && repo.is_none() {
        return None;
    }
    let json = leaf.try_get_one::<bool>("json").ok().flatten().copied().unwrap_or(false);
    let error = crate::context::require_config(repo.map(PathBuf::as_path)).err()?;
    Some(ExitCode::from(error.emit(&format!("canon {command}"), json)))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{hoist_tool_wide_options, leaf_takes_repo, sent_empty_value};

    #[test]
    fn atip_contract11_08_an_option_with_no_value_is_not_an_empty_value_the_caller_sent() {
        let args = |words: &[&str]| words.iter().map(|w| (*w).to_string()).collect::<Vec<_>>();
        assert!(sent_empty_value("--change", &args(&["canon", "finding", "add", "--change", "", "--json"])));
        assert!(sent_empty_value("--change", &args(&["canon", "finding", "add", "--change=", "--json"])));
        assert!(!sent_empty_value("--change", &args(&["canon", "finding", "add", "--change", "--json"])));
        assert!(!sent_empty_value("--change", &args(&["canon", "finding", "add", "--json", "--change"])));
        assert!(!sent_empty_value("--change", &args(&["canon", "finding", "add", "--", "--change", ""])));
    }

    fn hoisted(args: &[&str]) -> Vec<String> {
        let tree = crate::cli::command();
        hoist_tool_wide_options(&tree, args.iter().map(OsString::from).collect()).into_iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn atip_contract11_11_tool_wide_options_after_a_group_word_move_to_the_leaf_or_are_dropped() {
        assert_eq!(hoisted(&["canon", "finding", "--repo", ".", "--json"]), ["canon", "finding", "--json"]);
        assert_eq!(hoisted(&["canon", "finding", "--as", "planner", "--json"]), ["canon", "finding", "--json"]);
        assert_eq!(hoisted(&["canon", "finding", "--repo=.", "list", "--change", "c"]), ["canon", "finding", "list", "--change", "c", "--repo=."]);
        assert_eq!(hoisted(&["canon", "--as", "planner", "finding", "list"]), ["canon", "finding", "list", "--as", "planner"]);
        assert_eq!(hoisted(&["canon", "finding", "--bogus", "--json"]), ["canon", "finding", "--bogus", "--json"], "an option the group does not know stays a USAGE");
        assert_eq!(hoisted(&["canon", "status", "--repo", "x"]), ["canon", "status", "--repo", "x"], "after a leaf nothing moves");
        assert_eq!(hoisted(&["canon", "finding", "list", "--repo", "x", "--", "--repo"]), ["canon", "finding", "list", "--repo", "x", "--", "--repo"]);
    }

    /// The context check reads `--repo` from the command tree: a leaf that declares none (`schema`) is never
    /// judged against a `canon.yaml`, in debug and release builds alike.
    #[test]
    fn only_leaves_that_declare_repo_are_judged_against_a_canon_yaml() {
        let tree = crate::cli::command();
        assert!(!leaf_takes_repo(&tree, &["schema"]));
        assert!(!leaf_takes_repo(&tree, &["skills", "show"]));
        assert!(leaf_takes_repo(&tree, &["gate", "check"]));
        assert!(leaf_takes_repo(&tree, &["evidence", "add"]));
        assert!(!leaf_takes_repo(&tree, &["no-such-command"]));
    }
}
