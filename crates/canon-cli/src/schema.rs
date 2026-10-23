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
//! schema` lists every command, and `--as <ROLE>` lists only that role's
//! (`CANON_ROLE` never filters it). A command whose effect depends on its
//! arguments carries `effectNote`.

use clap::Command;
use serde::Serialize;

use crate::role::Role;
use crate::spec::{self, Node};

/// What running a command does to the repo and the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
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
    pub fn as_str(self) -> &'static str {
        match self {
            Effect::Read => "read",
            Effect::Write => "write",
            Effect::Destructive => "destructive",
        }
    }
}

/// One exit code a command can return, with its meaning.
#[derive(Debug, Clone, Copy, Serialize, schemars::JsonSchema)]
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
    /// decision); the unfiltered `canon schema` lists it.
    pub roles: &'static [Role],
}

/// One row: every row names its own exits, taken from the leaf's table row.
const fn row_exits(path: &'static str, effect: Effect, exits: &'static [ExitMeta], roles: &'static [Role]) -> CommandMeta {
    CommandMeta { path, effect, effect_note: None, exits, roles }
}

impl CommandMeta {
    const fn note(mut self, note: &'static str) -> Self {
        self.effect_note = Some(note);
        self
    }
}

/// Success, and a refusal where the leaf needs canon state: every failure of
/// the leaf is a refusal.
const REFUSABLE: &[ExitMeta] = &[exit(0, "success"), exit(2, "refused: CONFIG_MISSING, or an input that is missing, unreadable or invalid; nothing is written")];
/// The same for a leaf that reads or writes no canon state.
const REFUSABLE_NO_REPO: &[ExitMeta] = &[exit(0, "success"), exit(2, "refused: an argument or input that is missing, unreadable or invalid; nothing is written")];
/// A destructive leaf: the bare run previews, `--commit --plan-digest` acts.
const DESTRUCTIVE_EXITS: &[ExitMeta] = &[exit(0, "previewed, or committed, or nothing to do"), exit(2, "refused: CONFIG_MISSING, an invalid input, or PLAN_CHANGED; nothing is removed")];

use Effect::{Destructive, Read, Write};
use Role::{Implementer as I, Orchestrator as O, Planner as P, Reviewer as R};

const EVERYONE: &[Role] = &[P, I, R, O];
const HUMAN: &[Role] = &[];

/// One row per leaf command, canonical path first.
pub const META: &[CommandMeta] = &[
    row_exits("adapter authorize", Read, &[exit(0, "authorized"), exit(1, "not authorized: the report stays in data"), exit(2, "refused: the response file is missing or unreadable")], &[O]),
    row_exits("adapter validate", Read, &[exit(0, "valid envelope"), exit(1, "invalid envelope"), exit(2, "refused: the response file is missing or unreadable")], &[O]),
    row_exits("artifacts move", Destructive, DESTRUCTIVE_EXITS, &[O]),
    row_exits("artifacts verify", Read, &[exit(0, "every bound file is stored (or, local store only, still proven by the working tree)"), exit(1, "a bound file is missing from the store or altered: data.problems names each"), exit(2, "refused: canon.yaml cannot be read or the store cannot be used or reached")], &[O]),
    row_exits("brief approve", Write, REFUSABLE, HUMAN),
    row_exits("brief show", Read, REFUSABLE, &[P, R]),
    row_exits("brief check", Read, &[exit(0, "clean (advisories allowed)"), exit(1, "blocker findings"), exit(2, "refused: no such change, no live change (NO_BRIEF), or CONFIG_MISSING")], &[P, R]),
    row_exits("change new", Write, REFUSABLE, &[P]),
    row_exits("context", Read, &[exit(0, "success: it describes what can be authored and never gates"), exit(2, "refused: UNKNOWN_FIELD")], EVERYONE),
    row_exits("context-pack add", Write, REFUSABLE, &[O, I]),
    row_exits("context-pack show", Read, &[exit(0, "shown"), exit(1, "the pack is tampered (CONTEXT_PACK_TAMPERED)"), exit(2, "refused: CONFIG_MISSING, or no such pack")], &[O, I]),
    row_exits("context-pack verify", Read, &[exit(0, "verified"), exit(1, "the pack is tampered"), exit(2, "refused: CONFIG_MISSING, or no such pack")], &[O, I]),
    row_exits("dashboard", Write, REFUSABLE, &[O]).note("regenerates .canon/dashboard-snapshot; POST /api/briefs/approve rewrites a brief"),
    row_exits("demo review", Write, REFUSABLE, &[O]),
    row_exits("demo init", Write, REFUSABLE, &[O]),
    row_exits("dispatch begin", Write, REFUSABLE, &[O]),
    row_exits("dispatch diff", Read, REFUSABLE, &[O]),
    row_exits("dispatch end", Write, REFUSABLE, &[O]),
    row_exits("divergence accept", Write, REFUSABLE, HUMAN).note("refreshes stale scenario-index records first"),
    row_exits("divergence add", Write, REFUSABLE, &[I, R]),
    row_exits("divergence defer", Write, REFUSABLE, &[I, R]),
    row_exits("divergence resolve", Write, REFUSABLE, &[I, R]),
    row_exits("divergence list", Read, REFUSABLE, EVERYONE),
    row_exits("evidence add", Write, &[exit(0, "staged, committed or unchanged"), exit(1, "refused by the gate: a fabrication marker, an invalid typed atom, a contradicting report, an unapproved brief, or EVIDENCE_BATCH_REFUSED; nothing is staged"), exit(2, "refused: CONFIG_MISSING, a malformed flag combination, an unknown task, or no plan corpus; nothing is staged")], &[I]).note("refreshes stale scenario-index records first"),
    row_exits("evidence audit", Write, &[exit(0, "staged"), exit(1, "gate-red: a fabrication marker, an invalid typed atom, a contradicting report, or an unapproved brief refuses the record; nothing is staged"), exit(2, "usage or infrastructure: a malformed flag combination, an unknown scenario or spec root, a write failure; nothing is staged")], &[I]).note("refreshes stale scenario-index records first"),
    row_exits("evidence approval-payload", Read, REFUSABLE, &[I, O]),
    row_exits("artifacts store", Write, REFUSABLE, &[O]).note("refreshes stale scenario-index records first"),
    row_exits("export", Write, REFUSABLE, &[O]).note("writes the --output file (mode 0600) and one audit line"),
    row_exits("feature new", Write, REFUSABLE, &[P, I]),
    row_exits("finding add", Write, REFUSABLE, &[R]),
    row_exits("finding close", Write, REFUSABLE, &[R]),
    row_exits("finding list", Read, REFUSABLE, EVERYONE),
    row_exits("format", Read, &[exit(0, "clean"), exit(1, "the corpus has violations, or has 0 files (unverifiable)"), exit(2, "refused: the corpus root is not a directory, or CONFIG_MISSING")], EVERYONE),
    row_exits("gate check", Read, &[exit(0, "clean"), exit(1, "the gate found violations, or nothing was examined (unverifiable)"), exit(2, "refused: CONFIG_MISSING, or an unreadable ledger or config")], EVERYONE),
    row_exits("gate install-hooks", Write, REFUSABLE, &[O]),
    row_exits("gate promote", Write, &[exit(0, "promoted, recovered or nothing staged"), exit(1, "PROMOTE_REFUSED: the report stays in data"), exit(2, "refused: CONFIG_MISSING, or an unreadable ledger")], &[I]),
    row_exits("task done", Write, &[exit(0, "the task row was flipped, or already was"), exit(1, "GATE_TASK_BLOCKED: the row is not ready to flip; nothing is written"), exit(2, "refused: CONFIG_MISSING, an unknown task, or an unreadable plan")], &[I]),
    row_exits("ingest records", Write, REFUSABLE, &[O]),
    row_exits("ingest plans", Write, &[exit(0, "ingested"), exit(1, "INGEST_SOURCE_UNUSABLE: every source is unusable"), exit(2, "refused: CONFIG_MISSING, or an invalid argument")], &[O]),
    row_exits("ingest sessions", Write, REFUSABLE, &[O]),
    row_exits("init", Write, &[exit(0, "initialized, or (--check) every section passes"), exit(1, "--check: a section failed"), exit(2, "refused: an invalid argument or an unusable directory; nothing is written")], &[O]).note("--check writes nothing"),
    row_exits("inventory sync", Write, &[exit(0, "synced, or unchanged"), exit(1, "violations or duplicates in a root (INVENTORY_ABORTED); nothing is written for it"), exit(2, "refused: CONFIG_MISSING, or an invalid argument")], &[I, O]),
    row_exits("strategy approval-payload", Read, REFUSABLE, &[O]),
    row_exits("strategy promote", Write, &[exit(0, "promoted, or a dry run"), exit(1, "promotion blocked: the report names why"), exit(2, "refused: CONFIG_MISSING, or an argument error")], &[O]),
    row_exits("strategy rollback", Write, REFUSABLE, &[O]),
    row_exits("ledger compact", Destructive, DESTRUCTIVE_EXITS, &[O]),
    row_exits("migrate", Destructive, &[exit(0, "previewed, migrated, or nothing to do"), exit(1, "migrated, but the index rebuild found the corpus unclean"), exit(2, "refused: an invalid input, PLAN_CHANGED, or uncommitted changes under a moved path; nothing is written")], &[O]),
    row_exits("plugin sync", Write, &[exit(0, "synced, or deduped"), exit(1, "per-candidate write errors (PLUGIN_SYNC_ERRORS)"), exit(2, "refused: CONFIG_MISSING, or the plugin does not resolve")], &[O]),
    row_exits("policy approve", Write, REFUSABLE, HUMAN),
    row_exits("policy show", Read, REFUSABLE, &[O]),
    row_exits("prompt add", Write, &[exit(0, "added, or unchanged"), exit(1, "CONTEXT_OBJECT_CONFLICT: different content under one name@version"), exit(2, "refused: CONFIG_MISSING, or an invalid manifest or label")], &[O]),
    row_exits("prompt show", Read, REFUSABLE, &[O]),
    row_exits("purge", Destructive, DESTRUCTIVE_EXITS, &[O]).note("a committed run writes one audit line; a preview writes nothing"),
    row_exits("query", Read, REFUSABLE, EVERYONE).note("appends one line to .canon/audit/access.jsonl; a refused run writes none"),
    row_exits("regime show", Read, REFUSABLE_NO_REPO, &[O]),
    row_exits("report build", Write, &[exit(0, "built"), exit(1, "REPORT_FAILED: the ledger could not be read or duckdb is missing"), exit(2, "refused: CONFIG_MISSING, or an invalid argument")], &[O]).note("scratch files go under the system temp dir"),
    row_exits("report check", Read, &[exit(0, "fresh"), exit(1, "REPORT_STALE: the report is missing or has drifted, or REPORT_FAILED"), exit(2, "refused: CONFIG_MISSING, or an invalid argument")], &[O]).note("scratch files go under the system temp dir; writes no repo file"),
    row_exits("report snapshot", Write, &[exit(0, "written"), exit(1, "the ledger could not be read"), exit(2, "refused: CONFIG_MISSING, or an invalid argument")], &[O]).note("scratch files go under the system temp dir"),
    row_exits("retrieve", Read, REFUSABLE, EVERYONE),
    row_exits("review change", Write, REFUSABLE, &[R]).note("refreshes stale scenario-index records first"),
    row_exits("review scenario", Write, REFUSABLE, &[R]).note("refreshes stale scenario-index records first"),
    row_exits("scenario new", Write, REFUSABLE, &[P, I]),
    row_exits("completion", Read, &[exit(0, "the script was written"), exit(2, "refused: the shell is not one of bash, zsh, fish")], EVERYONE),
    row_exits("schema", Read, REFUSABLE_NO_REPO, EVERYONE),
    row_exits("selftest", Read, &[exit(0, "every suite is clean"), exit(1, "a suite failed"), exit(2, "refused: unknown suite")], &[O]).note("scratch directories in the system temp dir, removed on exit"),
    row_exits("skills check", Read, &[exit(0, "clean"), exit(1, "drift (SKILLS_DRIFT)"), exit(2, "refused: an invalid argument")], &[O]),
    row_exits("skills doctor", Read, &[exit(0, "always: diagnostics only")], &[O]),
    row_exits("skills install", Write, REFUSABLE, &[O]),
    row_exits("skills show", Read, REFUSABLE_NO_REPO, EVERYONE),
    row_exits("status", Read, &[exit(0, "success, whatever it finds"), exit(1, "unverifiable: nothing was examined"), exit(2, "refused: CONFIG_MISSING, or UNKNOWN_FIELD")], EVERYONE),
    row_exits("subject adopt", Write, REFUSABLE, &[P, O]),
    row_exits("subject new", Write, REFUSABLE, &[P, O]),
    row_exits("subject show", Read, &[exit(0, "shown"), exit(2, "refused: an unknown subject (SUBJECT_NOT_FOUND), or CONFIG_MISSING")], &[O]).note("refreshes stale scenario-index records first"),
    row_exits("subject move", Write, &[exit(0, "the transition was recorded"), exit(1, "SUBJECT_GATE_VIOLATION: an evidence, brief, review or policy-approval guard blocked the transition; the record is unchanged"), exit(2, "refused: an invalid transition, a missing reason, an unknown subject, or CONFIG_MISSING")], &[O]).note("reads without --to: `subject move <id>` shows the subject and writes nothing; with --to it refreshes stale scenario-index records first"),
    row_exits("subject unadopt", Write, REFUSABLE, &[P, O]),
    row_exits("tier age", Destructive, DESTRUCTIVE_EXITS, &[O]),
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

/// One command in `canon schema`'s listing.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ListEntry {
    command: String,
    effect: Effect,
    roles: Vec<Role>,
    about: Option<String>,
}

/// `canon schema` with no command: the commands, filtered to a role.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ListSchema {
    /// The role the listing was filtered to; absent when it lists every command.
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<Role>,
    commands: Vec<ListEntry>,
}

/// `canon schema --errors`: every error code.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ErrorList {
    errors: Vec<spec::ErrorRow>,
}

/// `canon schema`'s report (`SchemaReport`): the command listing, one command's
/// node of the generated spec, or the error registry.
#[derive(Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum SchemaReport<'a> {
    Commands(ListSchema),
    Command(&'a spec::Node),
    Errors(ErrorList),
    Error(ErrorEntry),
}

/// `canon schema --error <CODE>`: the one registry row.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ErrorEntry {
    pub error: spec::ErrorRow,
}

impl crate::json_out::Report for SchemaReport<'_> {}

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
/// outside `role`'s steps (`--as`); without a role every command is listed.
fn list_entries(nodes: &[Node], under: &[String], role: Option<Role>) -> Vec<ListEntry> {
    let prefix = under.join(" ");
    nodes
        .iter()
        .filter_map(|n| n.leaf.as_ref().map(|leaf| (n, leaf)))
        .filter(|(n, _)| prefix.is_empty() || n.command == prefix || n.command.starts_with(&format!("{prefix} ")))
        .filter(|(_, leaf)| !role.is_some_and(|r| !leaf.roles.contains(&r)))
        .map(|(n, leaf)| ListEntry { command: n.command.clone(), effect: leaf.effect, roles: leaf.roles.clone(), about: n.about.clone() })
        .collect()
}

/// Run `canon schema`: print to stdout and return the exit code. A path that
/// does not resolve is a refusal (exit 2) on stderr naming the nearest matches.
/// The listing is unfiltered; `as_role` (the `--as` flag, never `CANON_ROLE`)
/// keeps only that role's commands. `--errors` lists every error code and
/// `--error <CODE>` shows one (unknown: exit 2). A command's own schema is its
/// node of the generated spec ([`crate::spec`]).
pub fn run(root: &Command, tokens: &[String], json: bool, errors: bool, error: Option<&str>, as_role: Option<Role>) -> u8 {
    if errors {
        print_errors(json);
        return 0;
    }
    if let Some(code) = error {
        return print_error(code, json);
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
            let role = as_role;
            let commands = list_entries(&nodes, &under, role);
            if json {
                crate::json_out::data("schema", &SchemaReport::Commands(ListSchema { role, commands }));
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
                crate::json_out::data("schema", &SchemaReport::Command(node));
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
        crate::json_out::data("schema", &SchemaReport::Errors(ErrorList { errors: entries }));
    } else {
        let width = entries.iter().map(|e| e.code.len()).max().unwrap_or(0);
        for e in &entries {
            println!("{:<width$}  {}", e.code, e.meaning);
        }
    }
}

/// `canon schema --error <CODE>`: one registry row; an unknown code is a refusal.
fn print_error(code: &str, json: bool) -> u8 {
    let Some(row) = spec::error_rows().into_iter().find(|row| row.code == code) else {
        let nearest: Vec<&str> = crate::cli_error::all_codes().iter().map(|info| info.code).filter(|known| known.contains(code) || code.contains(known)).take(5).collect();
        let mut error = crate::cli_error::CliError::refused("UNKNOWN_ERROR_CODE", format!("`{code}` is not an error code canon has")).field("--error").received(code).hint("`canon schema --errors` lists every code");
        if !nearest.is_empty() {
            error = error.suggestions(nearest.iter().map(|c| c.to_string()).collect::<Vec<_>>());
        }
        return error.emit("canon schema", json) as u8;
    };
    if json {
        crate::json_out::data("schema", &SchemaReport::Error(ErrorEntry { error: row }));
    } else {
        println!("{}  exit {}  retryable: {}", row.code, row.exit, row.retryable);
        println!("  {}", row.meaning);
        println!("  fix: {}", row.fix);
    }
    0
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
        let meta = [row_exits("format", Read, REFUSABLE, EVERYONE), row_exits("ghost", Read, REFUSABLE, EVERYONE)];
        let problems = check_metadata(&tree(), &meta);
        assert!(problems.iter().any(|p| p.contains("`evidence add`") && p.contains("no metadata row")));
        assert!(problems.iter().any(|p| p.contains("`ghost`")));
        assert!(check_metadata(&tree(), &[row_exits("format", Read, REFUSABLE, EVERYONE), row_exits("evidence add", Write, REFUSABLE, &[I])]).is_empty());
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
