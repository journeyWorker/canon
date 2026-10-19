//! Deprecated spellings (0.24 D2): one table, one mechanism.
//!
//! 0.24 gives each concept one name (`--change-id`, not `--change`; `canon
//! query --kind evidence`, not `evidence_record`). The old spelling keeps
//! running for one release, wired to the canonical argument as a hidden clap
//! alias ([`apply`]). [`scan`] reads argv against the clap tree to see which
//! deprecated spellings an invocation used; [`warn`] prints one stderr line per
//! spelling (`canon <command>: --<old> is deprecated — use --<new> (removed in
//! 0.25.0)`) and remembers them so [`crate::json_out::envelope`] adds
//! `meta.deprecated: [{ "used", "use", "removedIn" }]`. `canon schema` reads
//! [`FLAGS`] and [`KIND_VALUES`] to list each option's `deprecatedAliases` /
//! `deprecatedValues`; help shows neither. 0.25 deletes the table rows, the
//! aliases and the hidden `--kind` values, and the old spellings become usage
//! errors.
//!
//! The table is the only place a deprecated spelling is written down.

use std::sync::Mutex;

use clap::Command;
use serde_json::{json, Value};

/// The release that deletes everything in [`FLAGS`] and [`KIND_VALUES`].
pub const REMOVED_IN: &str = "0.25.0";

/// A renamed option: `old` still parses on every command in `commands`, `new`
/// is canonical. Both are long names without the leading `--`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagRename {
    /// Space-separated command paths (without `canon`) that define `new`. An
    /// argument a parent defines `global` is named by the parent's path and
    /// covers its subcommands.
    pub commands: &'static [&'static str],
    pub old: &'static str,
    pub new: &'static str,
}

/// Every deprecated option spelling.
pub const FLAGS: &[FlagRename] = &[
    FlagRename { commands: &["brief", "review add", "scenario new"], old: "change", new: "change-id" },
    FlagRename { commands: &["feature new", "scenario new"], old: "project", new: "project-id" },
    FlagRename { commands: &["change new", "retrieve", "scenario new", "subject adopt"], old: "subject", new: "subject-id" },
    FlagRename { commands: &["feature new", "learn rollback", "scenario new"], old: "actor", new: "actor-id" },
    FlagRename { commands: &["dispatch end", "ingest artifacts"], old: "run", new: "run-id" },
];

/// The commands whose `--kind` names a record kind.
pub const KIND_COMMANDS: &[&str] = &["export", "purge", "query"];

/// Deprecated `--kind` values of [`KIND_COMMANDS`] as `(old, new)`: every
/// record kind carried a `_record` suffix in the spelling agents guessed, and
/// `evidence_record` was the only one that was accepted. The record's own
/// `kind` field, ledger paths and `canon.yaml` routing keys keep the storage
/// names; only the CLI value is renamed.
pub const KIND_VALUES: &[(&str, &str)] = &[
    ("change_record", "change"),
    ("task_record", "task"),
    ("scenario_record", "scenario"),
    ("session_record", "session"),
    ("run_record", "run"),
    ("event_record", "event"),
    ("handoff_record", "handoff"),
    ("review_record", "review"),
    ("divergence_record", "divergence"),
    ("trajectory_record", "trajectory"),
    ("strategy_item_record", "strategy_item"),
    ("evidence_record", "evidence"),
    ("subject_record", "subject"),
    ("finding_record", "finding"),
];

/// The canonical `--kind` value a deprecated one stands for.
pub fn kind_replacement(old: &str) -> Option<&'static str> {
    KIND_VALUES.iter().find(|(o, _)| *o == old).map(|(_, n)| *n)
}

/// Whether `path` (a command path) is `row_path` or below it.
fn covers(row_path: &str, path: &str) -> bool {
    path == row_path || path.strip_prefix(row_path).is_some_and(|rest| rest.starts_with(' '))
}

/// The deprecated spellings (`--old`) of the option `--<long>` on `command`.
pub fn flag_aliases(command: &str, long: &str) -> Vec<String> {
    FLAGS.iter().filter(|f| f.new == long && f.commands.iter().any(|c| covers(c, command))).map(|f| format!("--{}", f.old)).collect()
}

/// The deprecated values of `--kind` on `command` as `(old, new)`.
pub fn kind_values(command: &str) -> &'static [(&'static str, &'static str)] {
    if KIND_COMMANDS.contains(&command) { KIND_VALUES } else { &[] }
}

/// Registers every [`FLAGS`] row's `old` as a hidden alias of its canonical
/// option, so the old spelling parses and help does not show it.
///
/// # Panics
/// When a row names a command or option that does not exist: the table and the
/// clap tree must agree (`tests/one_name.rs` runs this).
pub fn apply(root: Command) -> Command {
    apply_at(root, "")
}

fn apply_at(mut cmd: Command, path: &str) -> Command {
    for row in FLAGS.iter().filter(|f| f.commands.contains(&path)) {
        let id = cmd
            .get_arguments()
            .find(|a| a.get_long() == Some(row.new))
            .unwrap_or_else(|| panic!("deprecated `--{}` names `--{}`, which `canon {path}` does not define", row.old, row.new))
            .get_id()
            .clone();
        cmd = cmd.mut_arg(id, |a| a.alias(row.old));
    }
    let names: Vec<String> = cmd.get_subcommands().map(|s| s.get_name().to_string()).collect();
    for name in names {
        let child = if path.is_empty() { name.clone() } else { format!("{path} {name}") };
        cmd = cmd.mut_subcommand(name, |sub| apply_at(sub, &child));
    }
    cmd
}

/// One deprecated spelling an invocation used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Used {
    /// The canonical command path the invocation resolved to, e.g. `brief approve`.
    pub command: String,
    /// What was typed, e.g. `--change` or `--kind evidence_record`.
    pub used: String,
    /// What to type instead, e.g. `--change-id` or `--kind evidence`.
    pub replacement: String,
}

/// The deprecated spellings in `args` (argv without the program name), read
/// against the clap tree `root` after [`apply`].
///
/// A token is a flag only where clap would read it as one: the value of an
/// option that takes a value is skipped (`--title --change` uses no flag), and
/// nothing after `--` is read. Both `--old value` and `--old=value` count.
pub fn scan(root: &Command, args: &[String]) -> Vec<Used> {
    let mut root = root.clone();
    root.build();
    let mut cmd = &root;
    let mut path: Vec<String> = Vec::new();
    // (flag row index or kind value, command path at the time it was met)
    let mut hits: Vec<(Hit, String)> = Vec::new();
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
            if let Some(arg) = cmd.get_arguments().find(|a| a.get_long() == Some(name) || a.get_all_aliases().is_some_and(|aliases| aliases.contains(&name))) {
                let here = path.join(" ");
                let value = inline.or_else(|| args.get(i + 1).map(String::as_str));
                if arg.get_long() != Some(name) {
                    if let Some(row) = FLAGS.iter().find(|f| f.old == name && f.commands.iter().any(|c| covers(c, &here))) {
                        hits.push((Hit::Flag(row), here.clone()));
                    }
                }
                if name == "kind" && KIND_COMMANDS.contains(&here.as_str()) {
                    if let Some((old, new)) = value.and_then(|v| KIND_VALUES.iter().find(|(o, _)| *o == v)) {
                        hits.push((Hit::Kind(old, new), here));
                    }
                }
                if arg.get_action().takes_values() && inline.is_none() {
                    i += 1;
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
    let command = path.join(" ");
    let mut used: Vec<Used> = Vec::new();
    for (hit, _) in hits {
        let entry = match hit {
            Hit::Flag(row) => Used { command: command.clone(), used: format!("--{}", row.old), replacement: format!("--{}", row.new) },
            Hit::Kind(old, new) => Used { command: command.clone(), used: format!("--kind {old}"), replacement: format!("--kind {new}") },
        };
        if !used.contains(&entry) {
            used.push(entry);
        }
    }
    used
}

#[derive(Clone, Copy)]
enum Hit {
    Flag(&'static FlagRename),
    Kind(&'static str, &'static str),
}

/// What the running invocation used, filled by [`warn`].
static ACTIVE: Mutex<Vec<Used>> = Mutex::new(Vec::new());

/// The stderr line for one deprecated spelling.
pub fn line(used: &Used) -> String {
    format!("canon {}: {} is deprecated — use {} (removed in {REMOVED_IN})", used.command, used.used, used.replacement)
}

/// Announce the deprecated spellings the running invocation used: one stderr
/// line each (once per spelling), and `meta.deprecated` on the JSON envelope.
pub fn warn(used: Vec<Used>) {
    let mut active = ACTIVE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    for entry in used {
        if !active.contains(&entry) {
            eprintln!("{}", line(&entry));
            active.push(entry);
        }
    }
}

/// [`warn`] for a deprecated name read from input rather than argv (a
/// `scenario new --from` line's key, which is a flag's long name): `old` is
/// the long name without `--`, and nothing happens when `command` has no such
/// deprecated flag.
pub fn warn_input_name(command: &str, old: &str) {
    if let Some(row) = FLAGS.iter().find(|f| f.old == old && f.commands.iter().any(|c| covers(c, command))) {
        warn(vec![Used { command: command.to_string(), used: format!("--{}", row.old), replacement: format!("--{}", row.new) }]);
    }
}

/// `meta.deprecated` for the running command, absent when it used none.
pub fn meta() -> Option<Value> {
    let active = ACTIVE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if active.is_empty() {
        return None;
    }
    Some(Value::Array(active.iter().map(|u| json!({ "used": u.used, "use": u.replacement, "removedIn": REMOVED_IN })).collect()))
}

/// Adds `deprecated` to the top-level `meta` object of a JSON document that is
/// not an envelope (`brief`'s report, `query`'s page), creating `meta` when the
/// document has none. Nothing when the invocation used no deprecated spelling.
pub fn tag_document(document: &mut Value) {
    let (Some(deprecated), Some(object)) = (meta(), document.as_object_mut()) else { return };
    if let Some(meta) = object.entry("meta").or_insert_with(|| json!({})).as_object_mut() {
        meta.insert("deprecated".to_string(), deprecated);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::kind_cli_name;
    use canon_model::envelope::RecordKind;

    #[test]
    fn every_record_kind_has_exactly_one_deprecated_spelling() {
        for kind in RecordKind::ALL {
            let new = kind_cli_name(kind);
            let old = format!("{new}_record");
            assert_eq!(kind_replacement(&old), Some(new), "{old}");
        }
        assert_eq!(KIND_VALUES.len(), RecordKind::ALL.len());
    }

    /// A tree shaped like the real one: `change new` renamed `--subject`, and
    /// `--title` takes a value that may look like a flag.
    fn tree() -> Command {
        use clap::Arg;
        apply(
            Command::new("canon").subcommand(
                Command::new("change").subcommand(
                    Command::new("new")
                        .arg(Arg::new("slug"))
                        .arg(Arg::new("subject-id").long("subject-id"))
                        .arg(Arg::new("title").long("title").allow_hyphen_values(true))
                        .arg(Arg::new("json").long("json").action(clap::ArgAction::SetTrue)),
                ),
            ),
        )
    }

    fn scan_args(args: &[&str]) -> Vec<Used> {
        scan(&tree(), &args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn a_deprecated_flag_is_found_in_both_value_forms() {
        let expected = vec![Used { command: "change new".into(), used: "--subject".into(), replacement: "--subject-id".into() }];
        assert_eq!(scan_args(&["change", "new", "x", "--subject", "demo"]), expected);
        assert_eq!(scan_args(&["change", "new", "x", "--subject=demo", "--json"]), expected);
        assert!(scan_args(&["change", "new", "x", "--subject-id", "demo"]).is_empty());
    }

    #[test]
    fn a_value_or_a_token_after_the_separator_is_never_a_flag() {
        assert!(scan_args(&["change", "new", "x", "--title", "--subject", "--subject-id", "demo"]).is_empty());
        assert!(scan_args(&["change", "new", "x", "--title=--subject"]).is_empty());
        assert!(scan_args(&["change", "new", "x", "--", "--subject", "demo"]).is_empty());
    }

    #[test]
    fn a_flag_is_deprecated_only_on_the_commands_that_renamed_it() {
        // `canon other --subject` is not a row of the table, whatever clap makes of it.
        let tree = Command::new("canon").subcommand(Command::new("other").arg(clap::Arg::new("subject").long("subject")));
        assert!(scan(&tree, &["other".to_string(), "--subject".to_string(), "x".to_string()]).is_empty());
    }

    #[test]
    fn covers_matches_whole_path_segments() {
        assert!(covers("brief", "brief approve"));
        assert!(covers("brief", "brief"));
        assert!(!covers("brief", "briefing"));
        assert!(!covers("review add", "review"));
    }
}
