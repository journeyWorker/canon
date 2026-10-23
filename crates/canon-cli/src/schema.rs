//! `canon schema [<command path>…] [--json]` — the CLI's own machine-readable
//! contract (0.20 D1).
//!
//! The command tree (names, aliases, arguments, options, enum values,
//! defaults) is read from clap, never restated, so it cannot drift from what
//! the parser accepts. What clap does not know — whether a command reads,
//! writes or destroys, and what its exit codes mean — lives in one table,
//! [`META`], with one row per leaf command; [`crate::cli::meta::meta`] picks the
//! row of a parsed command by an exhaustive `match`. [`check_metadata`] reports every
//! leaf without a row and every row clap has no leaf for; the binary's test
//! suite fails on either, so the table cannot drift from clap.
//!
//! Each command's `error_codes` come from [`crate::schema_codes`], a table
//! generated from the sources (`tests/schema_codes.rs` derives it and fails on
//! any difference; `CANON_BLESS_SCHEMA_CODES=1` rewrites it). `canon schema
//! --errors` prints the whole registry of [`crate::cli_error::all_codes`].
//!
//! The command touches no repo and no network: it only walks the in-memory
//! clap tree, so it works outside a canon repo. Since 0.28 a command's schema
//! is its node of the generated spec ([`crate::spec`], checked in as
//! `docs/cli/canon.cli.json`), so the two cannot disagree; this module keeps
//! the tables the nodes are built from ([`META`]) and resolves a path to its
//! node. Output order is deterministic: leaves sort by canonical path,
//! operands and options keep their declaration order.
//!
//! 0.21 adds three things the table and clap cannot say alone. `roles` says
//! which roles' steps use a command ([`CommandMeta::roles`]); bare `canon
//! schema` lists only the declared role's commands (`CANON_ROLE` or `--as`),
//! and `--all` lists every command. A command whose effect depends on its
//! arguments carries `effectNote`.

use clap::Command;
use serde::Serialize;

use crate::role::Role;
use crate::spec::{self, Node};

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

const fn exit(code: u8, meaning: &'static str) -> ExitMeta {
    ExitMeta { code, meaning }
}

/// The default exits, for a command whose module documents nothing narrower.
pub const STANDARD_EXITS: &[ExitMeta] = &[
    ExitMeta { code: 0, meaning: "success" },
    ExitMeta { code: 1, meaning: "operational failure, or a blocking finding the command reports" },
    ExitMeta { code: 2, meaning: "refused: the invocation or its inputs are wrong and fixable; nothing was written" },
];

/// Metadata clap cannot supply, for one leaf command.
pub struct CommandMeta {
    /// Canonical space-separated path, e.g. `evidence add`.
    pub path: &'static str,
    /// The worst the command does on any invocation. A command whose effect
    /// depends on its arguments says so in `effect_note` (0.21 D1: `subject
    /// status` is `write`, and reads only without `--to`), rather than
    /// being listed as the weaker of the two.
    pub effect: Effect,
    /// The arguments under which the command does less than `effect`.
    pub effect_note: Option<&'static str>,
    pub exits: &'static [ExitMeta],
    /// The roles whose steps use the command (0.21 D3, the plan's step
    /// ownership). Empty: humans only (the command records a human's
    /// decision); `canon schema --all` still lists it.
    pub roles: &'static [Role],
}

const fn row(path: &'static str, effect: Effect, roles: &'static [Role]) -> CommandMeta {
    CommandMeta { path, effect, effect_note: None, exits: STANDARD_EXITS, roles }
}

/// A row whose exits differ from [`STANDARD_EXITS`], as its module documents.
const fn row_exits(path: &'static str, effect: Effect, exits: &'static [ExitMeta], roles: &'static [Role]) -> CommandMeta {
    CommandMeta { path, effect, effect_note: None, exits, roles }
}

impl CommandMeta {
    const fn note(mut self, note: &'static str) -> Self {
        self.effect_note = Some(note);
        self
    }
}

/// A command that writes only when it succeeds and has no operational lane:
/// every failure is a refusal.
const REFUSES: &[ExitMeta] = &[exit(0, "success"), exit(2, "refused or failed; nothing is left written")];

use Effect::{Destructive, Read, Write};
use Role::{Implementer as I, Orchestrator as O, Planner as P, Reviewer as R};

const EVERYONE: &[Role] = &[P, I, R, O];
const HUMAN: &[Role] = &[];

/// One row per leaf command, canonical path first.
pub const META: &[CommandMeta] = &[
    row("adapter authorize", Read, &[O]),
    row("adapter validate", Read, &[O]),
    row_exits("artifacts migrate", Write, &[exit(0, "migrated, already migrated, or a dry run: nothing to do is success"), exit(2, "refused: the target is not an s3 tier, a blob does not match its name, the store is unreachable, or a write failed; nothing is removed until every blob verified")], &[O]),
    row_exits("artifacts verify", Read, &[exit(0, "every bound file is stored (or, local store only, still proven by the working tree)"), exit(1, "a bound file is missing from the store or altered: data.problems names each"), exit(2, "refused: canon.yaml cannot be read or the store cannot be used or reached; an unreachable store is never a pass")], &[O]),
    row("brief approve", Write, HUMAN),
    row_exits("brief show", Read, &[exit(0, "shown: the state and the next one to three steps"), exit(1, "operational error, with a stable error code"), exit(2, "refused: no live change and none named (NO_BRIEF), no such change, or no canon.yaml at --repo (CONFIG_MISSING)")], &[P, R]),
    row_exits("brief check", Read, &[exit(0, "clean (advisories allowed)"), exit(1, "operational error, with a stable error code"), exit(2, "blocker findings, or no canon.yaml at --repo (CONFIG_MISSING)")], &[P, R]),
    row_exits("change new", Write, REFUSES, &[P]),
    row_exits("context", Read, &[exit(0, "always: it describes what can be authored and never gates; an unreadable canon.yaml or policy degrades to the built-in surface, with the reason in policy.diagnostics")], EVERYONE),
    row("context-pack new", Write, &[O, I]),
    row("context-pack show", Read, &[O, I]),
    row("context-pack verify", Read, &[O, I]),
    row("dashboard", Write, &[O]),
    row_exits("demo add", Write, REFUSES, &[O]),
    row_exits("demo init", Write, REFUSES, &[O]),
    row("dispatch begin", Write, &[O]),
    row_exits("dispatch diff", Read, &[exit(0, "always, including when the observed graph differs from the declared plan or a side cannot be read (a note on stderr and in `notes`): it reports, it never gates"), exit(2, "refused: no canon.yaml at --repo (CONFIG_MISSING)")], &[O]),
    row("dispatch end", Write, &[O]),
    row_exits("divergence accept", Write, REFUSES, &[R]),
    row("divergence add", Write, &[I, R]),
    row("divergence defer", Write, &[I, R]),
    row("divergence promote", Write, &[I, O]),
    row("divergence resolve", Write, &[I, R]),
    row_exits("divergence status", Read, &[exit(0, "always on a successful read: a capability query, never a gate"), exit(2, "refused: canon.yaml is missing or unreadable, or the ledger cannot be read")], EVERYONE),
    row_exits("evidence add", Write, &[exit(0, "staged"), exit(1, "gate-red: a fabrication marker, an invalid typed atom, a contradicting report, or an unapproved brief refuses the record; nothing is staged"), exit(2, "usage or infrastructure: a malformed flag combination, an unknown task, no plan corpus, a write failure; nothing is staged")], &[I]),
    row_exits("evidence audit", Write, &[exit(0, "staged"), exit(1, "gate-red: a fabrication marker, an invalid typed atom, a contradicting report, or an unapproved brief refuses the record; nothing is staged"), exit(2, "usage or infrastructure: a malformed flag combination, an unknown scenario or spec root, a write failure; nothing is staged")], &[I]),
    row("evidence approval-payload", Read, &[I, O]),
    row_exits("evidence vault", Write, &[exit(0, "the pass ran, whatever it could store: files it could not store are listed in `notStored`, not an error; their bytes are gone, so no retry could clear a failing exit (`canon gate check` reports the stale evidence)"), exit(2, "refused: the ledger cannot be read or a write failed")], &[O]),
    row("export", Write, &[O]),
    row_exits("feature new", Write, REFUSES, &[P, I]),
    row_exits("finding add", Write, REFUSES, &[R]),
    row_exits("finding close", Write, REFUSES, &[R]),
    row_exits("finding list", Read, &[exit(0, "whether or not any finding matched"), exit(2, "refused: canon.yaml is missing or unreadable, or the ledger cannot be read")], EVERYONE),
    row_exits("format", Read, &[exit(0, "clean"), exit(1, "the corpus has violations"), exit(2, "refused: the corpus root is not a directory, or --repo holds no canon.yaml")], EVERYONE),
    row_exits("gate check", Read, &[exit(0, "clean"), exit(1, "the gate found violations, including a policy that changed since its approval"), exit(2, "the gate could not run: bad --repo or an unreadable ledger or config")], EVERYONE),
    row("gate install-hooks", Write, &[O]),
    row("gate promote", Write, &[I]),
    row_exits("gate selftest", Read, &[exit(0, "every fixture is clean"), exit(1, "a fixture failed")], &[O]),
    row_exits("gate task", Write, &[exit(0, "the task row was flipped"), exit(1, "gate-red: the row is not ready to flip; nothing is written"), exit(2, "usage or infrastructure: an unknown task, unreadable plan or config; nothing is written")], &[I]),
    row("ingest artifacts", Write, &[O]),
    row("ingest plans", Write, &[O]),
    row("ingest sessions", Write, &[O]),
    row_exits("init", Write, REFUSES, &[O]),
    row("inventory sync", Write, &[I, O]),
    row("learn approve", Write, &[O]),
    row("learn promote", Write, &[O]),
    row("learn rollback", Write, &[O]),
    row_exits("ledger compact", Write, &[exit(0, "compacted, or nothing to move"), exit(1, "a move failed; what moved stays moved, the rest is untouched"), exit(2, "refused: the ledger has no archive, or canon.yaml is unreadable")], &[O]),
    row_exits("migrate", Write, &[exit(0, "migrated, or nothing to do"), exit(1, "migrated, but the index rebuild found the corpus unclean"), exit(2, "refused or failed; nothing is written")], &[O]),
    row("plugin sync", Write, &[O]),
    row_exits("policy approve", Write, REFUSES, HUMAN),
    row("policy show", Read, &[O]),
    row("prompt add", Write, &[O]),
    row("prompt show", Read, &[O]),
    row("purge", Destructive, &[O]),
    row("query", Read, EVERYONE).note("appends one line per run to the read-access audit log .canon/audit/access.jsonl, which `canon init` gitignores and which is never committed; changes nothing else"),
    row("regime-key", Read, &[O]),
    row("report", Write, &[O]).note("with --check it only compares the report on disk with a fresh render and writes nothing"),
    row("retrieve", Read, EVERYONE),
    row_exits("review change", Write, REFUSES, &[R]),
    row_exits("review scenario", Write, REFUSES, &[R]),
    row_exits("scenario new", Write, REFUSES, &[P, I]),
    row_exits("completion", Read, &[exit(0, "the script was written"), exit(2, "refused: the shell is not one of bash, zsh, fish")], EVERYONE),
    row_exits("schema", Read, &[exit(0, "success"), exit(2, "refused: the command path does not resolve")], EVERYONE),
    row_exits("selftest", Read, &[exit(0, "every suite is clean"), exit(1, "a suite failed")], &[O]),
    row("skills check", Read, &[O]),
    row("skills doctor", Read, &[O]),
    row("skills install", Write, &[O]),
    row_exits("status", Read, &[exit(0, "always once canon.yaml is found: a problem reading the repo is a warning"), exit(2, "refused: no canon.yaml at --repo (CONFIG_MISSING)"), exit(1, "the report could not be serialized (--json)")], EVERYONE),
    row_exits("subject adopt", Write, REFUSES, &[P, O]),
    row_exits("subject new", Write, REFUSES, &[P, O]),
    row_exits("subject status", Write, &[exit(0, "the transition was recorded, or the subject was shown (no --to)"), exit(1, "an evidence, brief, review or policy-approval guard blocked the transition; the record is unchanged"), exit(2, "refused: an off-chain step, a missing reason, or a store failure")], &[O]).note("reads without --to: `subject status <id>` shows the subject and writes nothing"),
    row_exits("subject unadopt", Write, REFUSES, &[P, O]),
    row("tier age", Write, &[O]),
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

/// Drift between clap and [`META`]: empty when they agree exactly.
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
struct ListEntry {
    command: String,
    effect: Effect,
    roles: Vec<Role>,
    about: Option<String>,
}

#[derive(Serialize)]
struct ListSchema {
    /// The role the listing was filtered to; absent when it lists every command.
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<Role>,
    commands: Vec<ListEntry>,
}

/// Why a command path did not resolve.
#[derive(Debug, PartialEq, Eq)]
pub struct Refusal(pub String);

enum Resolved {
    /// A path naming a group (or the root): list the leaves beneath it.
    Group(Vec<String>),
    Leaf(Vec<String>),
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

fn resolve(root: &Command, tokens: &[String]) -> Result<Resolved, Refusal> {
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
        Ok(Resolved::Leaf(canonical))
    }
}

/// The stable error codes `path` can emit, sorted. The table is derived from
/// the sources and kept in step by `tests/schema_codes.rs`; a path the table
/// does not know has none.
pub fn error_codes(path: &str) -> Vec<&'static str> {
    crate::schema_codes::ERROR_CODES.iter().find(|(p, _)| *p == path).map_or_else(Vec::new, |(_, codes)| codes.to_vec())
}

/// The minimal example invocation of the leaf command `path`.
pub fn example_for(path: &str) -> Option<&'static str> {
    crate::schema_examples::EXAMPLES.iter().find(|(p, _)| *p == path).map(|(_, example)| *example)
}

/// Drift between clap and the example table: empty when every leaf has one
/// example and every example names a leaf.
pub fn check_examples(root: &Command) -> Vec<String> {
    let leaves = leaf_paths(root);
    let mut problems = Vec::new();
    for leaf in &leaves {
        if example_for(leaf).is_none() {
            problems.push(format!("clap leaf `{leaf}` has no example"));
        }
    }
    let mut seen: Vec<&str> = Vec::new();
    for (path, example) in crate::schema_examples::EXAMPLES {
        if !leaves.iter().any(|l| l == path) {
            problems.push(format!("example row `{path}` names no clap leaf command"));
        }
        if seen.contains(path) {
            problems.push(format!("example row `{path}` appears twice"));
        }
        seen.push(path);
        if !example.starts_with(&format!("canon {path}")) {
            problems.push(format!("example for `{path}` does not start with `canon {path}`: {example}"));
        }
    }
    problems
}

pub fn meta_for(path: &str) -> Option<&'static CommandMeta> {
    META.iter().find(|m| m.path == path)
}

/// The commands under `under`, hidden from the bare listing when they are
/// outside the declared `role`'s steps (unless `all`).
fn list_entries(nodes: &[Node], under: &[String], role: Option<Role>, all: bool) -> Vec<ListEntry> {
    let prefix = under.join(" ");
    nodes
        .iter()
        .filter_map(|n| n.leaf.as_ref().map(|leaf| (n, leaf)))
        .filter(|(n, _)| prefix.is_empty() || n.command == prefix || n.command.starts_with(&format!("{prefix} ")))
        .filter(|(_, leaf)| all || !role.is_some_and(|r| !leaf.roles.contains(&r)))
        .map(|(n, leaf)| ListEntry { command: n.command.clone(), effect: leaf.effect, roles: leaf.roles.clone(), about: n.about.clone() })
        .collect()
}

/// Run `canon schema`: print to stdout and return the exit code. A path that
/// does not resolve is a refusal (exit 2) on stderr naming the nearest matches.
/// A bare listing shows the declared role's commands (`CANON_ROLE` / `--as`),
/// or every command without a declared role; `all` lists every command. A
/// command's own schema is its node of the generated spec ([`crate::spec`]).
pub fn run(root: &Command, tokens: &[String], json: bool, errors: bool, all: bool) -> u8 {
    if errors {
        print_errors(json);
        return 0;
    }
    // Building the tree first copies a parent's `global = true` flags (`brief --json`) down to its leaves.
    let mut built = root.clone();
    built.build();
    let root = &built;
    // `canon schema "evidence add"` (one quoted word, dogfood eval 8) names
    // the same command as `canon schema evidence add`.
    let tokens: Vec<String> = tokens.iter().flat_map(|t| t.split_whitespace()).map(str::to_string).collect();
    let resolved = match resolve(root, &tokens) {
        Ok(r) => r,
        Err(Refusal(msg)) => return crate::cli_error::CliError::refused("UNKNOWN_COMMAND", msg).hint("`canon schema` lists every command").emit("canon schema", json) as u8,
    };
    let nodes = spec::nodes(root);
    match resolved {
        Resolved::Group(under) => {
            let role = if all { None } else { Role::current(None) };
            let commands = list_entries(&nodes, &under, role, all);
            if json {
                crate::json_out::data("schema", &crate::json_out::untyped(serde_json::to_value(ListSchema { role, commands }).expect("schema serializes")));
            } else {
                let width = commands.iter().map(|c| c.command.len()).max().unwrap_or(0);
                for c in &commands {
                    let about = c.about.as_deref().unwrap_or("");
                    println!("{:<width$}  {:<11} {}", c.command, c.effect.as_str(), about);
                }
            }
        }
        Resolved::Leaf(path) => {
            let key = path.join(" ");
            let node = nodes.iter().find(|n| n.command == key).expect("a resolved leaf is a node of the tree it came from");
            if json {
                crate::json_out::data("schema", &crate::json_out::untyped(serde_json::to_value(node).expect("schema serializes")));
            } else {
                print_human(node);
            }
        }
    }
    0
}

/// `canon schema --errors`: the whole registry, sorted by code.
fn print_errors(json: bool) {
    let entries = spec::error_rows();
    if json {
        crate::json_out::data("schema", &crate::json_out::untyped(serde_json::json!({ "errors": entries })));
    } else {
        let width = entries.iter().map(|e| e.code.len()).max().unwrap_or(0);
        for e in &entries {
            println!("{:<width$}  {}", e.code, e.meaning);
        }
    }
}

fn print_human(node: &Node) {
    println!("canon {}", node.command);
    if let Some(about) = &node.about {
        println!("  {about}");
    }
    let Some(leaf) = &node.leaf else {
        println!("a group of commands: {}", node.children.join(", "));
        return;
    };
    println!("effect: {}   supports --json: {}", leaf.effect.as_str(), if leaf.supports_json { "yes" } else { "no" });
    if let Some(note) = leaf.effect_note {
        println!("  {note}");
    }
    if let Some(idempotency) = leaf.idempotency {
        println!("idempotency: {}", serde_json::to_value(idempotency).expect("idempotency serializes").as_str().unwrap_or_default());
    }
    if leaf.roles.is_empty() {
        println!("roles: humans only");
    } else {
        println!("roles: {}", leaf.roles.iter().map(|r| r.as_str()).collect::<Vec<_>>().join(", "));
    }
    if !node.aliases.is_empty() {
        println!("aliases: {}", node.aliases.join(", "));
    }
    let notes = |required: bool, repeatable: bool, values: &[String], default: &[String], aliases: &[String]| {
        let mut notes = Vec::new();
        if required {
            notes.push("required".to_string());
        }
        if repeatable {
            notes.push("repeatable".to_string());
        }
        if !values.is_empty() {
            notes.push(format!("one of: {}", values.join("|")));
        }
        if !default.is_empty() {
            notes.push(format!("default: {}", default.join(",")));
        }
        if !aliases.is_empty() {
            notes.push(format!("aliases: {}", aliases.join(",")));
        }
        if notes.is_empty() {
            String::new()
        } else {
            format!("  [{}]", notes.join("; "))
        }
    };
    if !leaf.operands.is_empty() {
        println!("operands:");
        for o in &leaf.operands {
            println!("  {} <{}>{}", o.name, o.kind, notes(o.required, o.repeatable, &o.possible_values, &o.default, &[]));
        }
    }
    if !leaf.options.is_empty() {
        println!("options:");
        for o in &leaf.options {
            let value = o.value_name.as_ref().map(|v| format!(" <{v}>")).unwrap_or_default();
            println!("  --{}{value}{}", o.name, notes(o.required, o.repeatable, &o.possible_values, &o.default, &o.aliases));
        }
    }
    if !leaf.required_one_of.is_empty() {
        println!("one of each required: {}", leaf.required_one_of.iter().map(|set| set.join(" | ")).collect::<Vec<_>>().join("; "));
    }
    println!("exits:");
    for e in &leaf.exits {
        println!("  {}  {}", e.code, e.meaning);
    }
    for example in &leaf.examples {
        println!("example: {example}");
    }
    if !leaf.error_codes.is_empty() {
        println!("error codes: {}", leaf.error_codes.join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, ArgAction, Command};

    fn tree() -> Command {
        Command::new("canon")
            .subcommand(Command::new("format").visible_alias("fmt").arg(Arg::new("root")).arg(Arg::new("json").long("json").action(ArgAction::SetTrue)))
            .subcommand(Command::new("evidence").subcommand(Command::new("add").arg(Arg::new("scenario").long("scenario-id").action(ArgAction::Append).required(true))))
    }

    #[test]
    fn drift_is_reported_both_ways() {
        let meta = [row("format", Read, EVERYONE), row("ghost", Read, EVERYONE)];
        let problems = check_metadata(&tree(), &meta);
        assert!(problems.iter().any(|p| p.contains("`evidence add`") && p.contains("no metadata row")));
        assert!(problems.iter().any(|p| p.contains("`ghost`")));
        assert!(check_metadata(&tree(), &[row("format", Read, EVERYONE), row("evidence add", Write, &[I])]).is_empty());
    }

    #[test]
    fn alias_resolves_to_canonical_path() {
        match resolve(&tree(), &["fmt".into()]).unwrap() {
            Resolved::Leaf(path) => assert_eq!(path, vec!["format".to_string()]),
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
    fn leaf_node_carries_options() {
        let mut t = tree();
        t.build();
        let nodes = spec::nodes(&t);
        let leaf = nodes.iter().find(|n| n.command == "evidence add").and_then(|n| n.leaf.as_ref()).expect("evidence add is a leaf");
        let o = &leaf.options[0];
        assert_eq!((o.name.as_str(), o.required, o.repeatable), ("scenario-id", true, true));
    }
}
