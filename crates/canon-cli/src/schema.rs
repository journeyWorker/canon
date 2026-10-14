//! `canon schema [<command path>…] [--json]` — the CLI's own machine-readable
//! contract (0.20 D1).
//!
//! The command tree (names, aliases, arguments, options, enum values,
//! defaults) is read from clap, never restated, so it cannot drift from what
//! the parser accepts. What clap does not know — whether a command reads,
//! writes or destroys, and what its exit codes mean — lives in one table,
//! [`META`], with one row per leaf command. [`check_metadata`] reports every
//! leaf without a row and every row clap has no leaf for; the binary's test
//! suite fails on either, so the table cannot drift from clap.
//!
//! The command touches no repo and no network: it only walks the in-memory
//! clap tree, so it works outside a canon repo. Output order is
//! deterministic: leaves sort by canonical path, arguments and options keep
//! their declaration order.

use clap::{ArgAction, Command};
use serde::Serialize;

/// What running a command does to the repo and the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    /// Writes nothing (stdout/stderr only).
    Read,
    /// Writes or stages files or records; nothing existing is destroyed.
    Write,
    /// Permanently removes data.
    Destructive,
}

impl Effect {
    fn as_str(self) -> &'static str {
        match self {
            Effect::Read => "read",
            Effect::Write => "write",
            Effect::Destructive => "destructive",
        }
    }
}

/// One exit code a command can return, with its meaning.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ExitMeta {
    pub code: u8,
    pub meaning: &'static str,
}

/// The exit codes every canon command shares.
pub const STANDARD_EXITS: &[ExitMeta] = &[
    ExitMeta { code: 0, meaning: "success" },
    ExitMeta { code: 1, meaning: "operational failure, or a blocking finding the command reports" },
    ExitMeta { code: 2, meaning: "refused: the invocation or its inputs are wrong and fixable; nothing was written" },
];

/// Metadata clap cannot supply, for one leaf command.
pub struct CommandMeta {
    /// Canonical space-separated path, e.g. `evidence add`.
    pub path: &'static str,
    pub effect: Effect,
    pub exits: &'static [ExitMeta],
    /// Stable error codes the command can emit. Empty until the error-code
    /// registry lands; the registry fills this field per command.
    pub error_codes: &'static [&'static str],
}

const fn row(path: &'static str, effect: Effect) -> CommandMeta {
    CommandMeta { path, effect, exits: STANDARD_EXITS, error_codes: &[] }
}

use Effect::{Destructive, Read, Write};

/// One row per leaf command, canonical path first.
pub const META: &[CommandMeta] = &[
    row("adapter authorize", Read),
    row("adapter validate", Read),
    row("brief approve", Write),
    row("brief check", Read),
    row("change new", Write),
    row("context", Read),
    row("context-pack create", Write),
    row("context-pack show", Read),
    row("context-pack verify", Read),
    row("dashboard", Read),
    row("demo attest", Write),
    row("demo init", Write),
    row("dispatch begin", Write),
    row("dispatch diff", Read),
    row("dispatch end", Write),
    row("divergence defer", Write),
    row("divergence promote", Write),
    row("divergence resolve", Write),
    row("divergence stage", Write),
    row("divergence status", Read),
    row("evidence add", Write),
    row("evidence approval-payload", Read),
    row("evidence vault", Write),
    row("export", Write),
    row("feature new", Write),
    row("finding add", Write),
    row("finding close", Write),
    row("finding list", Read),
    row("format", Read),
    row("gate check", Read),
    row("gate install-hooks", Write),
    row("gate promote", Write),
    row("gate selftest", Read),
    row("gate task", Write),
    row("ingest artifacts", Write),
    row("ingest plans", Write),
    row("ingest sessions", Write),
    row("init", Write),
    row("inventory sync", Write),
    row("learn approve", Write),
    row("learn promote", Write),
    row("learn request", Write),
    row("learn rollback", Write),
    row("migrate", Write),
    row("plugin sync", Write),
    row("policy approve", Write),
    row("prompt register", Write),
    row("prompt show", Read),
    row("purge", Destructive),
    row("query", Read),
    row("regime-key", Read),
    row("report", Write),
    row("retrieve", Read),
    row("review add", Write),
    row("scenario new", Write),
    row("schema", Read),
    row("selftest", Read),
    row("skills check", Read),
    row("skills doctor", Read),
    row("skills install", Write),
    row("status", Read),
    row("subject adopt", Write),
    row("subject new", Write),
    row("subject status", Read),
    row("tier age", Write),
];

/// Canonical paths of every leaf command under `cmd`, sorted.
pub fn leaf_paths(root: &Command) -> Vec<String> {
    fn walk(cmd: &Command, prefix: &mut Vec<String>, out: &mut Vec<String>) {
        let subs: Vec<&Command> = cmd.get_subcommands().filter(|c| c.get_name() != "help").collect();
        if subs.is_empty() {
            if !prefix.is_empty() {
                out.push(prefix.join(" "));
            }
            return;
        }
        for sub in subs {
            prefix.push(sub.get_name().to_string());
            walk(sub, prefix, out);
            prefix.pop();
        }
    }
    let mut out = Vec::new();
    walk(root, &mut Vec::new(), &mut out);
    out.sort();
    out
}

/// Drift between clap and [`META`]: empty when the two agree exactly.
pub fn check_metadata(root: &Command, meta: &[CommandMeta]) -> Vec<String> {
    let leaves = leaf_paths(root);
    let mut problems = Vec::new();
    for leaf in &leaves {
        if !meta.iter().any(|m| m.path == leaf) {
            problems.push(format!("clap leaf `{leaf}` has no metadata row"));
        }
    }
    for m in meta {
        if !leaves.iter().any(|l| l == m.path) {
            problems.push(format!("metadata row `{}` names no clap leaf command", m.path));
        }
    }
    let mut seen: Vec<&str> = Vec::new();
    for m in meta {
        if seen.contains(&m.path) {
            problems.push(format!("metadata row `{}` appears twice", m.path));
        }
        seen.push(m.path);
    }
    problems
}

#[derive(Serialize)]
struct ArgSchema {
    /// Positional name or option long name (without `--`).
    name: String,
    /// `positional`, `flag` (no value), `count`, or `value`.
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    short: Option<char>,
    aliases: Vec<String>,
    value_name: Option<String>,
    required: bool,
    repeatable: bool,
    possible_values: Vec<String>,
    default: Vec<String>,
    help: Option<String>,
}

#[derive(Serialize)]
struct CommandSchema {
    /// Canonical path, aliases resolved.
    command: String,
    path: Vec<String>,
    aliases: Vec<String>,
    about: Option<String>,
    effect: Effect,
    supports_json: bool,
    arguments: Vec<ArgSchema>,
    options: Vec<ArgSchema>,
    exits: Vec<ExitMeta>,
    error_codes: Vec<&'static str>,
}

#[derive(Serialize)]
struct ListEntry {
    command: String,
    effect: Effect,
    about: Option<String>,
}

#[derive(Serialize)]
struct ListSchema {
    commands: Vec<ListEntry>,
}

/// Why a command path did not resolve.
#[derive(Debug, PartialEq, Eq)]
pub struct Refusal(pub String);

enum Resolved<'a> {
    /// A path naming a group (or the root): list the leaves beneath it.
    Group(Vec<String>),
    Leaf(Vec<String>, &'a Command),
}

fn find_sub<'a>(cmd: &'a Command, token: &str) -> Option<&'a Command> {
    cmd.get_subcommands().filter(|c| c.get_name() != "help").find(|c| c.get_name() == token || c.get_all_aliases().any(|a| a == token))
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push((prev[j] + usize::from(ca != *cb)).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn resolve<'a>(root: &'a Command, tokens: &[String]) -> Result<Resolved<'a>, Refusal> {
    let mut cmd = root;
    let mut canonical: Vec<String> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        let is_group = cmd.get_subcommands().any(|c| c.get_name() != "help");
        if !is_group {
            return Err(Refusal(format!(
                "`{}` is a leaf command and takes no sub-path `{}`",
                canonical.join(" "),
                tokens[i..].join(" ")
            )));
        }
        match find_sub(cmd, token) {
            Some(next) => {
                canonical.push(next.get_name().to_string());
                cmd = next;
            }
            None => {
                let names: Vec<&str> = cmd.get_subcommands().filter(|c| c.get_name() != "help").flat_map(|c| std::iter::once(c.get_name()).chain(c.get_all_aliases())).collect();
                let limit = (token.chars().count() / 3).max(2);
                let mut near: Vec<(usize, &str)> = names.iter().map(|n| (distance(token, n), *n)).filter(|(d, _)| *d <= limit).collect();
                near.sort();
                let prefix = canonical.join(" ");
                let at = |n: &str| if prefix.is_empty() { n.to_string() } else { format!("{prefix} {n}") };
                let shown: Vec<String> = if near.is_empty() {
                    cmd.get_subcommands().filter(|c| c.get_name() != "help").map(|c| at(c.get_name())).collect()
                } else {
                    near.iter().take(3).map(|(_, n)| at(n)).collect()
                };
                let label = if near.is_empty() { "available" } else { "nearest matches" };
                return Err(Refusal(format!("unknown command `{}`; {label}: {}", at(token), shown.join(", "))));
            }
        }
    }
    if cmd.get_subcommands().any(|c| c.get_name() != "help") {
        Ok(Resolved::Group(canonical))
    } else if canonical.is_empty() {
        Ok(Resolved::Group(canonical))
    } else {
        Ok(Resolved::Leaf(canonical, cmd))
    }
}

fn meta_for(path: &str) -> Option<&'static CommandMeta> {
    META.iter().find(|m| m.path == path)
}

fn arg_schema(arg: &clap::Arg) -> ArgSchema {
    let positional = arg.is_positional();
    let action = arg.get_action();
    let (kind, repeatable) = match action {
        ArgAction::SetTrue | ArgAction::SetFalse => ("flag", false),
        ArgAction::Count => ("count", true),
        _ => (if positional { "positional" } else { "value" }, matches!(action, ArgAction::Append)),
    };
    let repeatable = repeatable || arg.get_num_args().is_some_and(|r| r.max_values() > 1);
    let name = if positional { arg.get_id().to_string() } else { arg.get_long().map(str::to_string).unwrap_or_else(|| arg.get_id().to_string()) };
    ArgSchema {
        name,
        kind,
        short: arg.get_short(),
        aliases: arg.get_all_aliases().map(|a| a.iter().map(|s| s.to_string()).collect()).unwrap_or_default(),
        value_name: if kind == "flag" || kind == "count" {
            None
        } else {
            Some(arg.get_value_names().and_then(|v| v.first()).map(|v| v.to_string()).unwrap_or_else(|| arg.get_id().as_str().to_uppercase()))
        },
        required: arg.is_required_set(),
        repeatable,
        possible_values: arg.get_possible_values().iter().filter(|v| !v.is_hide_set()).map(|v| v.get_name().to_string()).collect(),
        default: arg.get_default_values().iter().map(|v| v.to_string_lossy().into_owned()).collect(),
        help: arg.get_help().map(|h| h.to_string()),
    }
}

fn command_schema(canonical: &[String], cmd: &Command) -> CommandSchema {
    let key = canonical.join(" ");
    let meta = meta_for(&key);
    let mut cmd = cmd.clone();
    cmd.build();
    // clap's own `--help` / `--version` flags are noise; a command's own argument that happens to be
    // called `version` (`prompt register --version`) is not.
    let visible = |a: &&clap::Arg| {
        !a.is_hide_set() && !matches!(a.get_action(), clap::ArgAction::Help | clap::ArgAction::HelpShort | clap::ArgAction::HelpLong | clap::ArgAction::Version)
    };
    let arguments: Vec<ArgSchema> = cmd.get_positionals().filter(visible).map(arg_schema).collect();
    let options: Vec<ArgSchema> = cmd.get_arguments().filter(|a| !a.is_positional()).filter(visible).map(arg_schema).collect();
    let supports_json = options.iter().any(|o| o.name == "json");
    CommandSchema {
        command: key,
        path: canonical.to_vec(),
        aliases: cmd.get_all_aliases().map(str::to_string).collect(),
        about: cmd.get_about().map(|a| a.to_string()),
        effect: meta.map_or(Effect::Write, |m| m.effect),
        supports_json,
        arguments,
        options,
        exits: meta.map_or(STANDARD_EXITS, |m| m.exits).to_vec(),
        error_codes: meta.map_or(&[][..], |m| m.error_codes).to_vec(),
    }
}

fn list_entries(root: &Command, under: &[String]) -> Vec<ListEntry> {
    let prefix = under.join(" ");
    leaf_paths(root)
        .into_iter()
        .filter(|p| prefix.is_empty() || p == &prefix || p.starts_with(&format!("{prefix} ")))
        .map(|p| {
            let mut cmd = root;
            for t in p.split(' ') {
                cmd = find_sub(cmd, t).expect("leaf path came from the same tree");
            }
            ListEntry { effect: meta_for(&p).map_or(Effect::Write, |m| m.effect), about: cmd.get_about().map(|a| a.to_string()), command: p }
        })
        .collect()
}

/// Run `canon schema`: print to stdout and return the exit code. A path that
/// does not resolve is a refusal (exit 2) on stderr naming the nearest matches.
pub fn run(root: &Command, tokens: &[String], json: bool) -> u8 {
    // Building the tree first copies a parent's `global = true` flags (`brief --json`) down to its leaves.
    let mut built = root.clone();
    built.build();
    let root = &built;
    let resolved = match resolve(root, tokens) {
        Ok(r) => r,
        Err(Refusal(msg)) => {
            eprintln!("canon schema: {msg}");
            return 2;
        }
    };
    match resolved {
        Resolved::Group(under) => {
            let commands = list_entries(root, &under);
            if json {
                println!("{}", serde_json::to_string_pretty(&ListSchema { commands }).expect("schema serializes"));
            } else {
                let width = commands.iter().map(|c| c.command.len()).max().unwrap_or(0);
                for c in &commands {
                    println!("{:<width$}  {:<11} {}", c.command, c.effect.as_str(), c.about.as_deref().unwrap_or(""));
                }
            }
        }
        Resolved::Leaf(path, cmd) => {
            let schema = command_schema(&path, cmd);
            if json {
                println!("{}", serde_json::to_string_pretty(&schema).expect("schema serializes"));
            } else {
                print_human(&schema);
            }
        }
    }
    0
}

fn print_human(s: &CommandSchema) {
    println!("canon {}", s.command);
    if let Some(about) = &s.about {
        println!("  {about}");
    }
    println!("effect: {}   supports --json: {}", s.effect.as_str(), if s.supports_json { "yes" } else { "no" });
    if !s.aliases.is_empty() {
        println!("aliases: {}", s.aliases.join(", "));
    }
    let line = |a: &ArgSchema, dashed: bool| {
        let mut l = format!("  {}{}", if dashed { "--" } else { "" }, a.name);
        if let Some(v) = &a.value_name {
            l.push_str(&format!(" <{v}>"));
        }
        let mut notes = Vec::new();
        if a.required {
            notes.push("required".to_string());
        }
        if a.repeatable {
            notes.push("repeatable".to_string());
        }
        if !a.possible_values.is_empty() {
            notes.push(format!("one of: {}", a.possible_values.join("|")));
        }
        if !a.default.is_empty() {
            notes.push(format!("default: {}", a.default.join(",")));
        }
        if !a.aliases.is_empty() {
            notes.push(format!("aliases: {}", a.aliases.join(",")));
        }
        if !notes.is_empty() {
            l.push_str(&format!("  [{}]", notes.join("; ")));
        }
        println!("{l}");
    };
    if !s.arguments.is_empty() {
        println!("arguments:");
        s.arguments.iter().for_each(|a| line(a, false));
    }
    if !s.options.is_empty() {
        println!("options:");
        s.options.iter().for_each(|a| line(a, true));
    }
    println!("exits:");
    for e in &s.exits {
        println!("  {}  {}", e.code, e.meaning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, Command};

    fn tree() -> Command {
        Command::new("canon")
            .subcommand(Command::new("format").visible_alias("fmt").arg(Arg::new("root")).arg(Arg::new("json").long("json").action(ArgAction::SetTrue)))
            .subcommand(Command::new("evidence").subcommand(Command::new("add").arg(Arg::new("scenario").long("scenario-id").action(ArgAction::Append).required(true))))
    }

    #[test]
    fn drift_is_reported_both_ways() {
        let meta = [row("format", Read), row("ghost", Read)];
        let problems = check_metadata(&tree(), &meta);
        assert!(problems.iter().any(|p| p.contains("`evidence add`") && p.contains("no metadata row")));
        assert!(problems.iter().any(|p| p.contains("`ghost`")));
        assert!(check_metadata(&tree(), &[row("format", Read), row("evidence add", Write)]).is_empty());
    }

    #[test]
    fn alias_resolves_to_canonical_path() {
        match resolve(&tree(), &["fmt".into()]).unwrap() {
            Resolved::Leaf(path, _) => assert_eq!(path, vec!["format".to_string()]),
            _ => panic!("expected a leaf"),
        }
    }

    #[test]
    fn unknown_path_names_nearest_matches() {
        let Err(Refusal(msg)) = resolve(&tree(), &["evidence".into(), "ad".into()]) else { panic!("expected refusal") };
        assert!(msg.contains("evidence add"), "{msg}");
        let Err(Refusal(msg)) = resolve(&tree(), &["evidence".into(), "add".into(), "x".into()]) else { panic!("expected refusal") };
        assert!(msg.contains("leaf"), "{msg}");
    }

    #[test]
    fn leaf_schema_carries_options() {
        let t = tree();
        let Resolved::Leaf(path, cmd) = resolve(&t, &["evidence".into(), "add".into()]).unwrap() else { panic!() };
        let s = command_schema(&path, cmd);
        let o = &s.options[0];
        assert_eq!((o.name.as_str(), o.required, o.repeatable), ("scenario-id", true, true));
    }
}
