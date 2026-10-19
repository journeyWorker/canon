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
//! Each command's `error_codes` come from [`crate::schema_codes`], a table
//! generated from the sources (`tests/schema_codes.rs` derives it and fails on
//! any difference; `CANON_BLESS_SCHEMA_CODES=1` rewrites it). `canon schema
//! --errors` prints the whole registry of [`crate::cli_error::all_codes`].
//!
//! The command touches no repo and no network: it only walks the in-memory
//! clap tree, so it works outside a canon repo. Output order is
//! deterministic: leaves sort by canonical path, arguments and options keep
//! their declaration order.
//!
//! 0.21 adds three things the table and clap cannot say alone. `roles` says
//! which roles' steps use a command ([`CommandMeta::roles`]); bare `canon
//! schema` lists only the declared role's commands (`CANON_ROLE` or `--as`),
//! and `--all` lists every command. A command whose effect depends on its
//! arguments carries `effectNote`.

use clap::{ArgAction, Command};
use serde::Serialize;

use crate::deprecation;
use crate::role::Role;

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
    row_exits("brief check", Read, &[exit(0, "clean (advisories allowed)"), exit(1, "operational error, with a stable error code"), exit(2, "blocker findings")], &[P, R]),
    row_exits("change new", Write, REFUSES, &[P]),
    row_exits("context", Read, &[exit(0, "always: it describes what can be authored and never gates; an unreadable canon.yaml or policy degrades to the built-in surface, with the reason in policy.diagnostics")], EVERYONE),
    row("context-pack new", Write, &[O, I]),
    row("context-pack show", Read, &[O, I]),
    row("context-pack verify", Read, &[O, I]),
    row("dashboard", Read, &[O]),
    row_exits("demo add", Write, REFUSES, &[O]),
    row_exits("demo init", Write, REFUSES, &[O]),
    row("dispatch begin", Write, &[O]),
    row_exits("dispatch diff", Read, &[exit(0, "always, including when the observed graph differs from the declared plan or a side cannot be read (a note on stderr and in `notes`): it reports, it never gates")], &[O]),
    row("dispatch end", Write, &[O]),
    row_exits("divergence accept", Write, REFUSES, &[R]),
    row("divergence add", Write, &[I, R]),
    row("divergence defer", Write, &[I, R]),
    row("divergence promote", Write, &[I, O]),
    row("divergence resolve", Write, &[I, R]),
    row_exits("divergence status", Read, &[exit(0, "always on a successful read: a capability query, never a gate"), exit(2, "refused: canon.yaml or the ledger cannot be read")], EVERYONE),
    row_exits("evidence add", Write, &[exit(0, "staged"), exit(1, "gate-red: a fabrication marker, an invalid typed atom, a contradicting report, or an unapproved brief refuses the record; nothing is staged"), exit(2, "usage or infrastructure: a malformed flag combination, an unknown task, no plan corpus, a write failure; nothing is staged")], &[I]),
    row("evidence approval-payload", Read, &[I, O]),
    row_exits("evidence vault", Write, &[exit(0, "the pass ran, whatever it could store: files it could not store are listed in `notStored`, not an error; their bytes are gone, so no retry could clear a failing exit (`canon gate check` reports the stale evidence)"), exit(2, "refused: the ledger cannot be read or a write failed")], &[O]),
    row("export", Write, &[O]),
    row_exits("feature new", Write, REFUSES, &[P, I]),
    row_exits("finding add", Write, REFUSES, &[R]),
    row_exits("finding close", Write, REFUSES, &[R]),
    row_exits("finding list", Read, &[exit(0, "whether or not any finding matched"), exit(2, "refused: the ledger cannot be read")], EVERYONE),
    row_exits("format", Read, &[exit(0, "clean"), exit(1, "the corpus has violations")], EVERYONE),
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
    row("query", Read, EVERYONE),
    row("regime-key", Read, &[O]),
    row("report", Write, &[O]),
    row("retrieve", Read, EVERYONE),
    row_exits("review add", Write, REFUSES, &[R]),
    row_exits("scenario new", Write, REFUSES, &[P, I]),
    row_exits("schema", Read, &[exit(0, "success"), exit(2, "refused: the command path does not resolve")], EVERYONE),
    row_exits("selftest", Read, &[exit(0, "every suite is clean"), exit(1, "a suite failed")], &[O]),
    row("skills check", Read, &[O]),
    row("skills doctor", Read, &[O]),
    row("skills install", Write, &[O]),
    row_exits("status", Read, &[exit(0, "always: a problem reading the repo is a warning"), exit(1, "the report could not be serialized (--json)")], EVERYONE),
    row_exits("subject adopt", Write, REFUSES, &[P, O]),
    row_exits("subject new", Write, REFUSES, &[P, O]),
    row_exits("subject status", Write, &[exit(0, "the transition was recorded, or the subject was shown (no --to)"), exit(1, "an evidence, brief, review or policy-approval guard blocked the transition; the record is unchanged"), exit(2, "refused: an off-chain step, a missing reason, or a store failure")], &[O]).note("reads without --to: `subject status <id>` shows the subject and writes nothing"),
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
#[serde(rename_all = "camelCase")]
struct ArgSchema {
    /// Positional name or option long name (without `--`).
    name: String,
    /// `positional`, `flag` (no value), `count`, or `value`.
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    short: Option<char>,
    aliases: Vec<String>,
    /// Old spellings of this option that still run and warn (0.24 D2,
    /// [`crate::deprecation::FLAGS`]); the next minor removes them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    deprecated_aliases: Vec<String>,
    value_name: Option<String>,
    required: bool,
    repeatable: bool,
    possible_values: Vec<String>,
    /// Old values of this option that still run and warn, with the value to
    /// use instead ([`crate::deprecation::KIND_VALUES`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    deprecated_values: Vec<DeprecatedValue>,
    default: Vec<String>,
    help: Option<String>,
}

#[derive(Serialize)]
struct DeprecatedValue {
    value: &'static str,
    #[serde(rename = "use")]
    replacement: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandSchema {
    /// Canonical path, aliases resolved.
    command: String,
    path: Vec<String>,
    aliases: Vec<String>,
    about: Option<String>,
    effect: Effect,
    /// What the command does when invoked with fewer effects than `effect`
    /// says (see [`CommandMeta::effect_note`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    effect_note: Option<&'static str>,
    /// The roles whose steps use the command; empty means humans only.
    roles: Vec<Role>,
    supports_json: bool,
    arguments: Vec<ArgSchema>,
    options: Vec<ArgSchema>,
    exits: Vec<ExitMeta>,
    /// Option sets of which exactly one is required (`review add`'s
    /// `--upstream-ref` / `--original-spec-ref`): no member is `required` on
    /// its own, so an agent reads the requirement here.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    required_one_of: Vec<Vec<String>>,
    /// One minimal invocation of the command (`tests/schema_examples.rs` runs it).
    example: Option<&'static str>,
    /// Every stable code the command can emit (module granularity; see
    /// `crate::schema_codes`).
    error_codes: Vec<&'static str>,
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

fn meta_for(path: &str) -> Option<&'static CommandMeta> {
    META.iter().find(|m| m.path == path)
}

fn arg_schema(command: &str, arg: &clap::Arg) -> ArgSchema {
    let positional = arg.is_positional();
    let action = arg.get_action();
    let (kind, repeatable) = match action {
        ArgAction::SetTrue | ArgAction::SetFalse => ("flag", false),
        ArgAction::Count => ("count", true),
        _ => (if positional { "positional" } else { "value" }, matches!(action, ArgAction::Append)),
    };
    let repeatable = repeatable || arg.get_num_args().is_some_and(|r| r.max_values() > 1);
    let name = if positional { arg.get_id().to_string() } else { arg.get_long().map(str::to_string).unwrap_or_else(|| arg.get_id().to_string()) };
    let deprecated_aliases = if positional { Vec::new() } else { deprecation::flag_aliases(command, &name) };
    let deprecated_values = if name == "kind" { deprecation::kind_values(command).iter().map(|(value, replacement)| DeprecatedValue { value, replacement }).collect() } else { Vec::new() };
    ArgSchema {
        aliases: arg.get_all_aliases().map(|a| a.iter().filter(|alias| !deprecated_aliases.contains(&format!("--{alias}"))).map(|s| s.to_string()).collect()).unwrap_or_default(),
        deprecated_aliases,
        deprecated_values,
        name,
        kind,
        short: arg.get_short(),
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
    let codes = error_codes(&key);
    let mut cmd = cmd.clone();
    cmd.build();
    // clap's own `--help` / `--version` flags are noise; a command's own argument that happens to be
    // called `version` (`prompt add --version`) is not.
    let visible = |a: &&clap::Arg| {
        !a.is_hide_set() && !matches!(a.get_action(), clap::ArgAction::Help | clap::ArgAction::HelpShort | clap::ArgAction::HelpLong | clap::ArgAction::Version)
    };
    let arguments: Vec<ArgSchema> = cmd.get_positionals().filter(visible).map(|a| arg_schema(&key, a)).collect();
    let options: Vec<ArgSchema> = cmd.get_arguments().filter(|a| !a.is_positional()).filter(visible).map(|a| arg_schema(&key, a)).collect();
    let supports_json = options.iter().any(|o| o.name == "json");
    CommandSchema {
        command: key.clone(),
        path: canonical.to_vec(),
        aliases: cmd.get_all_aliases().map(str::to_string).collect(),
        about: cmd.get_about().map(|a| a.to_string()),
        effect: meta.map_or(Effect::Write, |m| m.effect),
        effect_note: meta.and_then(|m| m.effect_note),
        roles: meta.map_or_else(Vec::new, |m| m.roles.to_vec()),
        supports_json,
        arguments,
        options,
        exits: meta.map_or(STANDARD_EXITS, |m| m.exits).to_vec(),
        required_one_of: cmd
            .get_groups()
            .filter(|g| g.is_required_set() && !(*g).clone().is_multiple())
            .map(|g| {
                g.get_args()
                    .filter_map(|id| cmd.get_arguments().find(|a| a.get_id() == id))
                    .map(|a| a.get_long().map(|l| format!("--{l}")).unwrap_or_else(|| a.get_id().to_string()))
                    .collect()
            })
            .collect(),
        example: example_for(&key),
        error_codes: codes,
    }
}

/// The commands under `under`, hidden from the bare listing when they are
/// outside the declared `role`'s steps (unless `all`).
fn list_entries(root: &Command, under: &[String], role: Option<Role>, all: bool) -> Vec<ListEntry> {
    let prefix = under.join(" ");
    leaf_paths(root)
        .into_iter()
        .filter(|p| prefix.is_empty() || p == &prefix || p.starts_with(&format!("{prefix} ")))
        .filter_map(|p| {
            let meta = meta_for(p.as_str());
            let roles = meta.map_or(&[][..], |m| m.roles);
            if !all && role.is_some_and(|r| !roles.contains(&r)) {
                return None;
            }
            let mut cmd = root;
            for t in p.split(' ') {
                cmd = find_sub(cmd, t).expect("leaf path came from the same tree");
            }
            Some(ListEntry {
                effect: meta.map_or(Effect::Write, |m| m.effect),
                roles: roles.to_vec(),
                about: cmd.get_about().map(|a| a.to_string()),
                command: p,
            })
        })
        .collect()
}

/// Run `canon schema`: print to stdout and return the exit code. A path that
/// does not resolve is a refusal (exit 2) on stderr naming the nearest matches.
/// A bare listing shows the declared role's commands (`CANON_ROLE` / `--as`),
/// or every command without a declared role; `all` lists every command.
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
    match resolved {
        Resolved::Group(under) => {
            let role = if all { None } else { Role::current(None) };
            let commands = list_entries(root, &under, role, all);
            if json {
                crate::json_out::data("schema", serde_json::to_value(ListSchema { role, commands }).expect("schema serializes"));
            } else {
                let width = commands.iter().map(|c| c.command.len()).max().unwrap_or(0);
                for c in &commands {
                    let about = c.about.as_deref().unwrap_or("");
                    println!("{:<width$}  {:<11} {}", c.command, c.effect.as_str(), about);
                }
            }
        }
        Resolved::Leaf(path, cmd) => {
            let schema = command_schema(&path, cmd);
            if json {
                crate::json_out::data("schema", serde_json::to_value(&schema).expect("schema serializes"));
            } else {
                print_human(&schema);
            }
        }
    }
    0
}

#[derive(Serialize)]
struct ErrorEntry {
    code: &'static str,
    meaning: &'static str,
}

/// `canon schema --errors`: the whole registry, sorted by code.
fn print_errors(json: bool) {
    let entries: Vec<ErrorEntry> = crate::cli_error::all_codes().iter().map(|(code, meaning)| ErrorEntry { code, meaning }).collect();
    if json {
        crate::json_out::data("schema", serde_json::json!({ "errors": entries }));
    } else {
        let width = entries.iter().map(|e| e.code.len()).max().unwrap_or(0);
        for e in &entries {
            println!("{:<width$}  {}", e.code, e.meaning);
        }
    }
}

fn print_human(s: &CommandSchema) {
    println!("canon {}", s.command);
    if let Some(about) = &s.about {
        println!("  {about}");
    }
    println!("effect: {}   supports --json: {}", s.effect.as_str(), if s.supports_json { "yes" } else { "no" });
    if let Some(note) = s.effect_note {
        println!("  {note}");
    }
    if s.roles.is_empty() {
        println!("roles: humans only");
    } else {
        println!("roles: {}", s.roles.iter().map(|r| r.as_str()).collect::<Vec<_>>().join(", "));
    }
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
    if let Some(example) = s.example {
        println!("example: {example}");
    }
    if !s.error_codes.is_empty() {
        println!("error codes: {}", s.error_codes.join(", "));
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
        let meta = [row("format", Read, EVERYONE), row("ghost", Read, EVERYONE)];
        let problems = check_metadata(&tree(), &meta);
        assert!(problems.iter().any(|p| p.contains("`evidence add`") && p.contains("no metadata row")));
        assert!(problems.iter().any(|p| p.contains("`ghost`")));
        assert!(check_metadata(&tree(), &[row("format", Read, EVERYONE), row("evidence add", Write, &[I])]).is_empty());
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
