//! Removed spellings (0.24 D2 deprecated them, 0.25 D6 removed them).
//!
//! 0.24 gave each concept one name (`--change-id`, not `--change`; `canon
//! query --kind evidence`, not `evidence_record`) and kept the old spelling
//! running for one release. 0.25 stops parsing them: an old spelling is a
//! usage error like any unknown flag, but its message and hint name the
//! spelling that replaced it ([`find`]). The tables below exist only to write
//! that hint; nothing parses through them.

use clap::Command;

/// The release that renamed everything in [`FLAGS`] and the `_record` kind values.
const RENAMED_IN: &str = "0.24.0";

/// A renamed option: `old` no longer parses on any command in `commands`;
/// `new` is canonical. Both are long names without the leading `--`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagRename {
    /// Space-separated command paths (without `canon`) that define `new`. An
    /// argument a parent defines `global` is named by the parent's path and
    /// covers its subcommands.
    pub commands: &'static [&'static str],
    pub old: &'static str,
    pub new: &'static str,
}

/// Every removed option spelling.
pub const FLAGS: &[FlagRename] = &[
    FlagRename { commands: &["brief", "review add", "scenario new"], old: "change", new: "change-id" },
    FlagRename { commands: &["feature new", "scenario new"], old: "project", new: "project-id" },
    FlagRename { commands: &["change new", "retrieve", "scenario new", "subject adopt"], old: "subject", new: "subject-id" },
    FlagRename { commands: &["feature new", "learn rollback", "scenario new"], old: "actor", new: "actor-id" },
    FlagRename { commands: &["dispatch end", "ingest artifacts"], old: "run", new: "run-id" },
];

/// The commands whose `--kind` names a record kind. A `<kind>_record` value
/// of it was the 0.23 spelling.
pub const KIND_COMMANDS: &[&str] = &["export", "purge", "query"];

/// The commands whose `scenario new --from` lines carried the removed keys.
const BATCH_COMMAND: &str = "scenario new";

/// Whether `path` (a command path) is `row_path` or below it.
fn covers(row_path: &str, path: &str) -> bool {
    path == row_path || path.strip_prefix(row_path).is_some_and(|rest| rest.starts_with(' '))
}

/// The canonical `--kind` value a removed `<kind>_record` one stood for.
fn kind_replacement(old: &str) -> Option<&'static str> {
    let stem = old.strip_suffix("_record")?;
    crate::query::parse_kind(stem).ok().map(crate::query::kind_cli_name)
}

/// The key of a `scenario new --from` line that replaced the removed `old`.
pub fn batch_key_replacement(old: &str) -> Option<&'static str> {
    FLAGS.iter().find(|f| f.old == old && f.commands.contains(&BATCH_COMMAND)).map(|f| f.new)
}

/// The hint for a removed `scenario new --from` key.
pub fn batch_key_hint(old: &str, new: &str) -> String {
    format!("`{old}` was renamed `{new}` in {RENAMED_IN}; use `{new}`")
}

/// A removed spelling an invocation used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    /// The command path the invocation resolved to, e.g. `brief approve`.
    pub command: String,
    /// What was typed, e.g. `--change` or `--kind evidence_record`.
    pub used: String,
    /// What to type instead, e.g. `--change-id` or `--kind evidence`.
    pub replacement: String,
    /// The argument the usage error points at, e.g. `--change` or `--kind`.
    pub field: String,
}

impl Removed {
    /// The usage error's message.
    pub fn message(&self) -> String {
        format!("{} was removed in 0.25.0", self.used)
    }

    /// The usage error's hint: the old spelling, the release that renamed it, the new one.
    pub fn hint(&self) -> String {
        format!("{} was renamed {} in {RENAMED_IN}; use {}", self.used, self.replacement, self.replacement)
    }
}

/// The first removed spelling in `args` (argv without the program name), read
/// against the clap tree `root`; call it once clap refused the line.
///
/// A token is a flag only where clap would read it as one: the value of an
/// option that takes a value is skipped (`--title --change` uses no flag), and
/// nothing after `--` is read. Both `--old value` and `--old=value` count; a
/// flag counts only on the commands that renamed it.
pub fn find(root: &Command, args: &[String]) -> Option<Removed> {
    let mut root = root.clone();
    root.build();
    let mut cmd = &root;
    let mut path: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let token = args[i].as_str();
        if token == "--" {
            break;
        }
        if let Some(rest) = token.strip_prefix("--") {
            let (name, inline) = match rest.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (rest, None),
            };
            let here = path.join(" ");
            match cmd.get_arguments().find(|a| a.get_long() == Some(name)) {
                Some(arg) => {
                    let value = inline.or_else(|| args.get(i + 1).map(String::as_str));
                    if name == "kind" && KIND_COMMANDS.contains(&here.as_str()) {
                        if let Some((old, new)) = value.and_then(|v| kind_replacement(v).map(|n| (v, n))) {
                            return Some(Removed { command: here, used: format!("--kind {old}"), replacement: format!("--kind {new}"), field: "--kind".into() });
                        }
                    }
                    if arg.get_action().takes_values() && inline.is_none() {
                        i += 1;
                    }
                }
                None => {
                    if let Some(row) = FLAGS.iter().find(|f| f.old == name && f.commands.iter().any(|c| covers(c, &here))) {
                        return Some(Removed { command: here, used: format!("--{}", row.old), replacement: format!("--{}", row.new), field: format!("--{}", row.old) });
                    }
                }
            }
        } else if let Some(shorts) = token.strip_prefix('-').filter(|s| !s.is_empty()) {
            if let Some(arg) = shorts.chars().next().and_then(|c| cmd.get_arguments().find(|a| a.get_short() == Some(c))) {
                if arg.get_action().takes_values() && shorts.chars().count() == 1 {
                    i += 1;
                }
            }
        } else if let Some(sub) = cmd.find_subcommand(token) {
            path.push(sub.get_name().to_string());
            cmd = sub;
        }
        i += 1;
    }
    None
}

/// A group 0.27 ran without a subcommand, now a usage error that names the leaf
/// that took its place: `(group path, replacement path)`.
const BARE_GROUPS: &[(&str, &str)] = &[("brief", "brief show")];

/// A group invoked bare that used to run, and the leaf that replaced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BareGroup {
    /// The group path, e.g. `brief`.
    pub command: &'static str,
    /// The leaf to run instead, e.g. `brief show`.
    pub replacement: &'static str,
}

impl BareGroup {
    /// The usage error's message.
    pub fn message(&self) -> String {
        format!("`canon {}` is a group and takes a subcommand", self.command)
    }

    /// The usage error's hint: the leaf that does what the bare group did.
    pub fn hint(&self) -> String {
        format!("use `canon {}`", self.replacement)
    }
}

/// The bare group `args` (argv without the program name) stops at, when it is
/// one of [`BARE_GROUPS`]; read against the clap tree `root`. Options with a
/// value are skipped, and nothing after `--` is read.
pub fn bare_group(root: &Command, args: &[String]) -> Option<BareGroup> {
    let mut root = root.clone();
    root.build();
    let mut cmd = &root;
    let mut path: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let token = args[i].as_str();
        if token == "--" {
            break;
        }
        if let Some(rest) = token.strip_prefix("--") {
            if !rest.contains('=') && cmd.get_arguments().any(|a| a.get_long() == Some(rest) && a.get_action().takes_values()) {
                i += 1;
            }
        } else if token.starts_with('-') {
            // short flags carry no operand this walk needs
        } else if let Some(sub) = cmd.find_subcommand(token) {
            path.push(sub.get_name());
            cmd = sub;
        }
        i += 1;
    }
    let here = path.join(" ");
    BARE_GROUPS.iter().find(|(group, _)| *group == here).map(|(command, replacement)| BareGroup { command, replacement })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::kind_cli_name;
    use canon_model::envelope::RecordKind;

    #[test]
    fn every_record_kind_has_exactly_one_removed_spelling() {
        for kind in RecordKind::ALL {
            let new = kind_cli_name(kind);
            assert_eq!(kind_replacement(&format!("{new}_record")), Some(new), "{new}");
        }
        assert_eq!(kind_replacement("nonsense_record"), None);
        assert_eq!(kind_replacement("evidence"), None);
    }

    /// A tree shaped like the real one: `change new` renamed `--subject`, and
    /// `--title` takes a value that may look like a flag.
    fn tree() -> Command {
        use clap::Arg;
        Command::new("canon").subcommand(
            Command::new("change").subcommand(
                Command::new("new")
                    .arg(Arg::new("slug"))
                    .arg(Arg::new("subject-id").long("subject-id"))
                    .arg(Arg::new("title").long("title").allow_hyphen_values(true))
                    .arg(Arg::new("json").long("json").action(clap::ArgAction::SetTrue)),
            ),
        )
    }

    fn find_in(args: &[&str]) -> Option<Removed> {
        find(&tree(), &args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn a_removed_flag_is_found_in_both_value_forms() {
        let expected = Some(Removed { command: "change new".into(), used: "--subject".into(), replacement: "--subject-id".into(), field: "--subject".into() });
        assert_eq!(find_in(&["change", "new", "x", "--subject", "demo"]), expected);
        assert_eq!(find_in(&["change", "new", "x", "--subject=demo", "--json"]), expected);
        assert_eq!(find_in(&["change", "new", "x", "--subject-id", "demo"]), None);
    }

    #[test]
    fn a_value_or_a_token_after_the_separator_is_never_a_flag() {
        assert_eq!(find_in(&["change", "new", "x", "--title", "--subject", "--subject-id", "demo"]), None);
        assert_eq!(find_in(&["change", "new", "x", "--title=--subject"]), None);
        assert_eq!(find_in(&["change", "new", "x", "--", "--subject", "demo"]), None);
    }

    #[test]
    fn a_flag_is_removed_only_on_the_commands_that_renamed_it() {
        let tree = Command::new("canon").subcommand(Command::new("other"));
        assert_eq!(find(&tree, &["other".to_string(), "--subject".to_string(), "x".to_string()]), None);
    }

    #[test]
    fn the_hint_names_the_release_and_the_new_spelling() {
        let removed = find_in(&["change", "new", "x", "--subject", "demo"]).unwrap();
        assert_eq!(removed.hint(), "--subject was renamed --subject-id in 0.24.0; use --subject-id");
    }

    #[test]
    fn covers_matches_whole_path_segments() {
        assert!(covers("brief", "brief approve"));
        assert!(covers("brief", "brief"));
        assert!(!covers("brief", "briefing"));
        assert!(!covers("review add", "review"));
    }
}
