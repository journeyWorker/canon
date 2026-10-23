//! Removed spellings: an old command, option or bare group is a usage error
//! (exit 2) whose `error.hint` names the spelling that replaced it and whose
//! `error.expected` lists it.
//!
//! Nothing parses through the table, and no alias runs: clap refuses the line
//! first, then [`find`] reads the same argv against the clap tree and the one
//! data table, [`crate::removed_028::ROWS`], to write the refusal. The table is
//! also the spec's `removed` section.

use clap::Command;

use crate::removed_028::{Kind, Row, ROWS};

/// The commands whose `--kind` names a record kind. A `<kind>_record` value
/// of it was the 0.23 spelling.
pub const KIND_COMMANDS: &[&str] = &["export", "purge", "query"];

/// The canonical `--kind` value a removed `<kind>_record` one stood for.
fn kind_replacement(old: &str) -> Option<&'static str> {
    let stem = old.strip_suffix("_record")?;
    crate::query::parse_kind(stem).ok().map(crate::query::kind_cli_name)
}

/// A removed spelling an invocation used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    /// The command path the invocation resolved to, e.g. `brief approve`.
    pub command: String,
    /// What was typed, e.g. `--by`, `gate task` or `--kind evidence_record`.
    pub used: String,
    /// What to type instead: the spelling the table names, or `--kind <value>`.
    pub replacement: String,
    /// What else changed, one clause; empty when the replacement says it all.
    pub note: &'static str,
    /// The argument the usage error points at.
    pub field: String,
    /// The release that removed the spelling.
    pub since: &'static str,
}

impl Removed {
    /// The usage error's message.
    pub fn message(&self) -> String {
        format!("{} was removed in {}", self.used, self.since)
    }

    /// The usage error's hint: the old spelling, the release that removed it, the new one.
    pub fn hint(&self) -> String {
        let shown = if self.replacement.starts_with('-') { self.replacement.clone() } else { format!("canon {}", self.replacement) };
        let mut hint = format!("{} was removed in {}; use `{shown}`", self.used, self.since);
        if !self.note.is_empty() {
            hint.push_str(&format!(" ({})", self.note));
        }
        hint
    }

    fn from_row(row: &'static Row, command: String, used: String, field: String) -> Removed {
        Removed { command, used, replacement: row.new.to_string(), note: row.note, field, since: row.since }
    }

    /// A removed option whose value became an operand: the replacement is the line the caller typed, on the
    /// leaf the row names (the typed leaf when it is one of the row's own), with the value as the operand
    /// and every other token kept, so it runs as it stands.
    fn operand_row(row: &'static Row, command: String, used: String, args: &[String], path_at: &[usize]) -> Removed {
        let mut taken = vec![false; args.len()];
        for &i in path_at {
            taken[i] = true;
        }
        let leaf = if row.scope.contains(&row.new) { command.as_str() } else { row.new };
        let mut operand = take_option(args, &used, &mut taken);
        if row.scope.iter().any(|scope| scope.starts_with("prompt ")) {
            // `--name` and `--version` are one operand, name@version.
            let (other, joined) = if used == "--name" { ("--version", true) } else { ("--name", false) };
            let sibling = take_option(args, other, &mut taken);
            operand = match (operand, sibling) {
                (Some(a), Some(b)) if joined => Some(format!("{a}@{b}")),
                (Some(a), Some(b)) => Some(format!("{b}@{a}")),
                _ => None,
            };
        }
        // No value was typed, so there is no operand to fill in: the leaf says it, with the row's note.
        let Some(operand) = operand else {
            return Removed { command: command.clone(), used: used.clone(), replacement: leaf.to_string(), note: row.note, field: used, since: row.since };
        };
        let mut words: Vec<String> = leaf.split(' ').map(str::to_string).collect();
        words.push(shell_word(&operand));
        words.extend(args.iter().zip(&taken).filter(|(_, taken)| !**taken).map(|(arg, _)| shell_word(arg)));
        Removed { command, used: used.clone(), replacement: words.join(" "), note: row.note, field: used, since: row.since }
    }
}

/// The value of the option `name` (`--name value` or `--name=value`) in `args`,
/// marking the tokens it used in `taken`; nothing after `--` is read.
fn take_option(args: &[String], name: &str, taken: &mut [bool]) -> Option<String> {
    for (i, arg) in args.iter().enumerate() {
        if arg == "--" {
            break;
        }
        if arg == name {
            let value = args.get(i + 1).filter(|value| !value.starts_with("--"))?;
            taken[i] = true;
            taken[i + 1] = true;
            return Some(value.clone());
        }
        if let Some(value) = arg.strip_prefix(name).and_then(|rest| rest.strip_prefix('=')) {
            taken[i] = true;
            return Some(value.to_string());
        }
    }
    None
}

/// `word` as one shell word: as it is when it is plain, else in single quotes.
fn shell_word(word: &str) -> String {
    if !word.is_empty() && word.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_./:@=,+%".contains(&b)) {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// Whether `row` removed the option `old` on the command at `path`.
fn option_row(old: &str, path: &str) -> Option<&'static Row> {
    ROWS.iter().find(|row| row.kind == Kind::Option && row.old == old && row.scope.contains(&path))
}

/// A removed command: the command words the walk is at (`path`), the unresolved
/// word `token` and the positional words that follow it spell a row's `old`.
fn command_row(path: &str, token: &str, following: &[&str]) -> Option<&'static Row> {
    let mut spelled = if path.is_empty() { token.to_string() } else { format!("{path} {token}") };
    let mut found = ROWS.iter().find(|row| row.kind == Kind::Command && row.old == spelled);
    for word in following.iter().take(2) {
        if found.is_some() {
            break;
        }
        spelled = format!("{spelled} {word}");
        found = ROWS.iter().find(|row| row.kind == Kind::Command && row.old == spelled);
    }
    found
}

/// The first removed spelling in `args` (argv without the program name), read
/// against the clap tree `root`; call it once clap refused the line.
///
/// A token is an option only where clap would read it as one: the value of an
/// option that takes a value is skipped (`--title --by` uses no option), and
/// nothing after `--` is read. Both `--old value` and `--old=value` count; an
/// option counts only on the commands the table names. A command word counts
/// where its group has no such subcommand, and a group that ends the line
/// without a subcommand counts when it ran bare in 0.27.
pub fn find(root: &Command, args: &[String]) -> Option<Removed> {
    let mut root = root.clone();
    root.build();
    let mut cmd = &root;
    let mut path: Vec<String> = Vec::new();
    let mut path_at: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let token = args[i].as_str();
        if token == "--" {
            break;
        }
        let here = path.join(" ");
        if let Some(rest) = token.strip_prefix("--") {
            let (name, inline) = match rest.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (rest, None),
            };
            match cmd.get_arguments().find(|a| a.get_long() == Some(name)) {
                Some(arg) => {
                    let value = inline.or_else(|| args.get(i + 1).map(String::as_str));
                    if name == "kind" && KIND_COMMANDS.contains(&here.as_str()) {
                        if let Some((old, new)) = value.and_then(|v| kind_replacement(v).map(|n| (v, n))) {
                            return Some(Removed { command: here, used: format!("--kind {old}"), replacement: format!("--kind {new}"), note: "", field: "--kind".into(), since: "0.24.0" });
                        }
                    }
                    if arg.get_action().takes_values() && inline.is_none() {
                        i += 1;
                    }
                }
                None => {
                    if let Some(row) = option_row(&format!("--{name}"), &here) {
                        if row.operand {
                            return Some(Removed::operand_row(row, here, format!("--{name}"), args, &path_at));
                        }
                        return Some(Removed::from_row(row, here, format!("--{name}"), format!("--{name}")));
                    }
                }
            }
        } else if let Some(shorts) = token.strip_prefix('-').filter(|s| !s.is_empty()) {
            let first = shorts.chars().next();
            match first.and_then(|c| cmd.get_arguments().find(|a| a.get_short() == Some(c))) {
                Some(arg) => {
                    if arg.get_action().takes_values() && shorts.chars().count() == 1 {
                        i += 1;
                    }
                }
                None => {
                    if let Some(row) = first.and_then(|c| option_row(&format!("-{c}"), &here)) {
                        return Some(Removed::from_row(row, here, format!("-{}", first.unwrap_or_default()), format!("-{}", first.unwrap_or_default())));
                    }
                }
            }
        } else if let Some(sub) = cmd.find_subcommand(token) {
            path.push(sub.get_name().to_string());
            path_at.push(i);
            cmd = sub;
        } else if cmd.has_subcommands() {
            let following: Vec<&str> = args[i + 1..].iter().map(String::as_str).take_while(|a| !a.starts_with('-')).collect();
            if let Some(row) = command_row(&here, token, &following) {
                return Some(Removed::from_row(row, here, row.old.to_string(), row.old.to_string()));
            }
        }
        i += 1;
    }
    let here = path.join(" ");
    if cmd.has_subcommands() {
        if let Some(row) = ROWS.iter().find(|row| row.kind == Kind::Bare && row.old == here) {
            return Some(Removed::from_row(row, here.clone(), format!("canon {here} without a subcommand"), here));
        }
    }
    None
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
    /// `--title` takes a value that may look like an option.
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
    fn a_removed_option_is_found_in_both_value_forms() {
        let found = find_in(&["change", "new", "x", "--subject", "demo"]).expect("removed");
        assert_eq!((found.used.as_str(), found.replacement.as_str()), ("--subject", "--subject-id"));
        assert_eq!(find_in(&["change", "new", "x", "--subject=demo", "--json"]), Some(found));
        assert_eq!(find_in(&["change", "new", "x", "--subject-id", "demo"]), None);
    }

    #[test]
    fn a_value_or_a_token_after_the_separator_is_never_an_option() {
        assert_eq!(find_in(&["change", "new", "x", "--title", "--subject", "--subject-id", "demo"]), None);
        assert_eq!(find_in(&["change", "new", "x", "--title=--subject"]), None);
        assert_eq!(find_in(&["change", "new", "x", "--", "--subject", "demo"]), None);
    }

    #[test]
    fn an_option_is_removed_only_on_the_commands_the_table_names() {
        let tree = Command::new("canon").subcommand(Command::new("other"));
        assert_eq!(find(&tree, &["other".to_string(), "--subject".to_string(), "x".to_string()]), None);
    }

    #[test]
    fn the_hint_names_the_new_spelling() {
        let removed = find_in(&["change", "new", "x", "--subject", "demo"]).unwrap();
        assert!(removed.hint().contains("--subject-id"));
    }
}
