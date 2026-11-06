//! `canon schema [<command path>…] [--json]` — the CLI's own machine-readable
//! contract (0.20 D1; atip/1 spec 10 since 0.34).
//!
//! The answers are the contract's documents ([`atip::doc`]): the tree of every group and
//! command with the `level` canon claims, a group's children, and one command in full. The
//! command tree (names, operands, options, value types, typed defaults, enum values) is read
//! from clap by [`atip::project`], never restated, so it cannot drift from what the parser
//! accepts. What clap does not know — whether a command reads, writes or destroys, and which
//! roles use it — lives in one table, [`META`], with one row per leaf command;
//! [`crate::cli::meta::meta`] picks the row of a parsed command by an exhaustive `match`.
//! [`check_metadata`] reports every leaf without a row and every row clap has no leaf for;
//! the binary's test suite fails on either, so the table cannot drift from clap.
//!
//! Each command's `errors` come from [`crate::schema_codes`], a table generated from the
//! sources (`tests/schema_codes.rs` derives it and fails on any difference;
//! `CANON_BLESS_SCHEMA_CODES=1` rewrites it), with the reserved codes of the features the
//! command has. `canon schema --errors` prints the whole registry of
//! [`crate::cli_error::all_codes`].
//!
//! The command touches no repo and no network: it only walks the in-memory clap tree, so it
//! works outside a canon repo and from any environment (the documents are the one
//! [`crate::spec`] builds; the checked-in `docs/cli/canon.cli.json` holds the same ones).
//! Output order is deterministic: the tree is in command order, operands and options keep
//! their declaration order.
//!
//! The registries (0.34, contract 1.1): `--errors` and `--error <CODE>` print the
//! error registry with each code's reasons, `--warnings` and `--warning <CODE>`
//! the warning registry ([`crate::cli_error::all_warnings`]). Each answer names
//! itself in `data.kind` (`errors`, `error`, `warnings`, `warning`). A path and
//! the four options exclude one another; an unknown code, or a code of the other
//! registry, is `USAGE` with the option as `field`.
//!
//! `roles` says which roles' steps use a command ([`CommandMeta::roles`]); it travels in
//! `x-canon.roles`. Bare `canon schema` lists every command, and `--as <ROLE>` lists only
//! that role's (`CANON_ROLE` never filters it). A command whose effect depends on its
//! arguments carries `effectNote`.

use clap::Command;
use serde::Serialize;

use crate::cli_error::CliErrorExt as _;
use crate::role::Role;
use crate::spec;

pub use atip::doc::Effect;

/// Metadata clap cannot supply, for one leaf command.
#[derive(Debug)]
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
    /// The roles whose steps use the command (0.21 D3, the plan's step
    /// ownership). Empty: humans only (the command records a human's
    /// decision); the unfiltered `canon schema` lists it.
    pub roles: &'static [Role],
}

/// One row of [`META`].
const fn row(path: &'static str, effect: Effect, roles: &'static [Role]) -> CommandMeta {
    CommandMeta { path, effect, effect_note: None, roles }
}

impl CommandMeta {
    const fn note(mut self, note: &'static str) -> Self {
        self.effect_note = Some(note);
        self
    }
}

use Effect::{Destructive, Read, Write};
use Role::{Implementer as I, Orchestrator as O, Planner as P, Reviewer as R};

const EVERYONE: &[Role] = &[P, I, R, O];
const HUMAN: &[Role] = &[];

/// One row per leaf command, canonical path first.
pub const META: &[CommandMeta] = &[
    row("adapter authorize", Read, &[O]),
    row("adapter validate", Read, &[O]),
    row("artifacts move", Destructive, &[O]),
    row("artifacts prune", Destructive, &[O]),
    row("artifacts verify", Read, &[O]),
    row("brief approve", Write, HUMAN),
    row("brief show", Read, &[P, R]),
    row("brief check", Read, &[P, R]),
    row("change new", Write, &[P]),
    row("context", Read, EVERYONE),
    row("context-pack add", Write, &[O, I]),
    row("context-pack show", Read, &[O, I]),
    row("context-pack verify", Read, &[O, I]),
    row("dashboard", Write, &[O]).note("prints one report, then serves until signalled; regenerates .canon/dashboard-snapshot; POST /api/briefs/approve rewrites a brief"),
    row("demo review", Write, &[O]),
    row("demo init", Write, &[O]),
    row("dispatch begin", Write, &[O]),
    row("dispatch diff", Read, &[O]),
    row("dispatch end", Write, &[O]),
    row("divergence accept", Write, HUMAN).note("refreshes stale scenario-index records once nothing refuses the call; a refused call writes nothing"),
    row("divergence add", Write, &[I, R]),
    row("divergence defer", Write, &[I, R]),
    row("divergence resolve", Write, &[I, R]),
    row("divergence list", Read, EVERYONE),
    row("evidence add", Write, &[I]).note("refreshes stale scenario-index records once nothing refuses the call; a refused call writes nothing"),
    row("evidence audit", Write, &[I]).note("refreshes stale scenario-index records once nothing refuses the call; a refused call writes nothing"),
    row("evidence approval-payload", Read, &[I, O]),
    row("artifacts store", Write, &[O]).note("refreshes stale scenario-index records once nothing refuses the call; a refused call writes nothing"),
    row("export", Write, &[O]).note("writes the --output file (mode 0600), replacing it if it exists, and appends one line to .canon/audit/access.jsonl on each run that succeeds; a refused run writes neither"),
    row("feature new", Write, &[P, I]),
    row("finding add", Write, &[R]),
    row("finding close", Write, &[R]),
    row("finding list", Read, EVERYONE),
    row("format", Read, EVERYONE),
    row("gate check", Read, EVERYONE),
    row("gate install-hooks", Write, &[O]),
    row("gate promote", Write, &[I]),
    row("task done", Write, &[I]),
    row("ingest records", Write, &[O]),
    row("ingest plans", Write, &[O]),
    row("ingest sessions", Write, &[O]),
    row("init", Write, &[O]).note("--check writes nothing"),
    row("inventory sync", Write, &[I, O]),
    row("strategy approval-payload", Read, &[O]),
    row("strategy promote", Write, &[O]),
    row("strategy rollback", Write, &[O]),
    row("ledger compact", Destructive, &[O]),
    row("migrate", Destructive, &[O]),
    row("plugin sync", Write, &[O]),
    row("policy approve", Write, HUMAN),
    row("policy show", Read, &[O]),
    row("prompt add", Write, &[O]),
    row("prompt show", Read, &[O]),
    row("purge", Destructive, &[O]).note("a committed run writes one audit line; a preview writes nothing"),
    row("query", Read, EVERYONE).note("appends one line to .canon/audit/access.jsonl; a refused run writes none"),
    row("regime show", Read, &[O]),
    row("search", Read, EVERYONE).note("first brings the derived index up to date: writes the gitignored .canon/index/search.db (and the -wal and -shm files SQLite may add) and .canon/index/.gitignore when an input changed or the index is missing; a current index is not written"),
    row("trace", Read, EVERYONE).note("first brings the derived index up to date: writes the gitignored .canon/index/search.db (and the -wal and -shm files SQLite may add) and .canon/index/.gitignore when an input changed or the index is missing; a current index is not written"),
    row("report build", Write, &[O]).note("scratch files go under the system temp dir"),
    row("report check", Read, &[O]).note("scratch files go under the system temp dir; writes no repo file"),
    row("report snapshot", Write, &[O]).note("scratch files go under the system temp dir"),
    row("retrieve", Read, EVERYONE),
    row("review change", Write, &[R]).note("refreshes stale scenario-index records once nothing refuses the call; a refused call writes nothing"),
    row("review scenario", Write, &[R]).note("refreshes stale scenario-index records once nothing refuses the call; a refused call writes nothing"),
    row("scenario new", Write, &[P, I]),
    row("completion", Read, EVERYONE),
    row("schema", Read, EVERYONE),
    row("selftest", Read, &[O]).note("scratch directories in the system temp dir, removed on exit"),
    row("skills check", Read, &[O]),
    row("skills doctor", Read, &[O]),
    row("skills install", Write, &[O]),
    row("skills show", Read, EVERYONE),
    row("status", Read, EVERYONE),
    row("subject adopt", Write, &[P, O]),
    row("subject new", Write, &[P, O]),
    row("subject show", Read, &[O]).note("refreshes the stale scenario-index cache once the subject is found; a refused call writes nothing"),
    row("subject move", Write, &[O]).note("refreshes stale scenario-index records once the transition is allowed; a refused call writes nothing"),
    row("subject unadopt", Write, &[P, O]),
    row("tier age", Destructive, &[O]),
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

/// `canon schema --errors`: every error code.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ErrorList {
    #[schemars(extend("const" = "errors"))]
    kind: &'static str,
    errors: Vec<spec::ErrorRow>,
}

/// `canon schema --warnings`: every warning code.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct WarningList {
    #[schemars(extend("const" = "warnings"))]
    kind: &'static str,
    warnings: Vec<spec::WarningRow>,
}

/// `canon schema`'s report (`SchemaReport`): a document of the contract's self-description (the tree, a group, a command, the
/// schemas of a command: [`atip::Document`], written in the form of the minor canon declares), or a registry (atip/1 spec 9.1:
/// `data.kind` names which).
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum SchemaReport<'a> {
    Document(atip::Document<'a>),
    Errors(ErrorList),
    Error(ErrorEntry),
    Warnings(WarningList),
    Warning(WarningEntry),
}

/// The shape of the entries of a tree or a group (1.4: no `path`, `kind` only for a group).
#[derive(serde::Serialize, schemars::JsonSchema)]
struct EntryShape {
    command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    effect: Option<Effect>,
}

/// An option the document lists in full (spec 9.7): every member that is not its default.
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct OptionShape {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    short: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value_type: Option<atip::doc::ValueType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    possible_values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repeatable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<atip::doc::DefaultValue>")]
    default: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flag_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<atip::doc::DomainSchema>")]
    domain: Option<serde_json::Value>,
    help: String,
}

/// One entry of `options`: the name of a tree option or of a reserved option, or an option listed in full (spec 9.5).
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[expect(dead_code, reason = "the enum only gives the schema of an entry of `options`: the document itself is written by the atip crate")]
enum OptionEntry {
    Name(String),
    Own(Box<OptionShape>),
}

/// An operand of a 1.4 document: every member that is not its default.
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct OperandShape {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value_type: Option<atip::doc::ValueType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    possible_values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    variadic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<atip::doc::DefaultValue>")]
    default: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<atip::doc::DomainSchema>")]
    domain: Option<serde_json::Value>,
    help: String,
}

/// A validator of a 1.4 document: its `rejects` are in the `--schemas` answer.
#[derive(serde::Serialize, schemars::JsonSchema)]
struct ValidatorShape {
    arguments: Vec<String>,
    code: String,
    description: String,
}

/// An output of a 1.4 document: every member that is not its default.
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct OutputShape {
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<atip::doc::OutputKind>,
    schema_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    media_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    batch: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    when: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fields: Option<atip::doc::FieldsDoc>,
}

/// The `input` of a 1.4 document: no `option` and no `schemaRef`, the fragment schema is derived and is `Input` of the schemas answer.
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct InputShape {
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    batch: Option<bool>,
    max_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_items: Option<u64>,
    #[schemars(with = "atip::doc::InputExample")]
    example: serde_json::Value,
}

/// `canon schema --json`: the tree (spec 9.3), with `options` the options commands share.
#[derive(serde::Serialize, schemars::JsonSchema)]
struct TreeShape {
    #[schemars(extend("const" = "tree"))]
    kind: String,
    level: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<Vec<OptionShape>>,
    commands: Vec<EntryShape>,
}

/// `canon schema <GROUP> --json`: a group and the entries of its children (spec 9.4).
#[derive(serde::Serialize, schemars::JsonSchema)]
struct GroupShape {
    #[schemars(extend("const" = "group"))]
    kind: String,
    command: String,
    summary: String,
    commands: Vec<EntryShape>,
}

/// `canon schema <COMMAND> --json`: the call surface of one command (spec 9.5); its members that equal their default are absent.
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CommandShape {
    #[schemars(extend("const" = "command"))]
    kind: String,
    command: String,
    summary: String,
    effect: Effect,
    #[serde(skip_serializing_if = "Option::is_none")]
    idempotency: Option<atip::doc::Idempotency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effect_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operands: Option<Vec<OperandShape>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<Vec<OptionEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    constraints: Option<Vec<atip::doc::ConstraintDoc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    validators: Option<Vec<ValidatorShape>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<Vec<String>>,
    outputs: Vec<OutputShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<InputShape>,
    examples: Vec<String>,
}

/// `canon schema <COMMAND> --schemas --json`: the JSON Schemas the command's refs reach, `Input` and the `rejects` of its validators (spec 9.9.3).
#[derive(serde::Serialize, schemars::JsonSchema)]
struct SchemasShape {
    #[schemars(extend("const" = "schemas"))]
    kind: String,
    command: String,
    schemas: std::collections::BTreeMap<String, atip::doc::EmbeddedSchema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rejects: Option<Vec<Vec<String>>>,
}

impl schemars::JsonSchema for SchemaReport<'_> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "SchemaReport".into()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        concat!(module_path!(), "::SchemaReport").into()
    }

    /// One of the answers, by `kind`. The members of a document are the contract's (`atip::SCHEMA_DOC_SCHEMA` holds each in full, and
    /// the tests hold every answer to it), so a list of options, operands, validators or outputs is typed no further here.
    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let alternatives = vec![
            generator.subschema_for::<TreeShape>(),
            generator.subschema_for::<GroupShape>(),
            generator.subschema_for::<CommandShape>(),
            generator.subschema_for::<SchemasShape>(),
            generator.subschema_for::<ErrorList>(),
            generator.subschema_for::<ErrorEntry>(),
            generator.subschema_for::<WarningList>(),
            generator.subschema_for::<WarningEntry>(),
        ];
        schemars::json_schema!({ "anyOf": alternatives })
    }
}

/// `canon schema --error <CODE>`: the one registry row.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ErrorEntry {
    #[schemars(extend("const" = "error"))]
    kind: &'static str,
    pub error: spec::ErrorRow,
}

/// `canon schema --warning <CODE>`: the one warning registry row.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct WarningEntry {
    #[schemars(extend("const" = "warning"))]
    kind: &'static str,
    pub warning: spec::WarningRow,
}

impl crate::json_out::Report for SchemaReport<'_> {
    /// The version of the self-description format is the contract version canon implements (spec 9.1.2).
    const SCHEMA_VERSION: &'static str = crate::json_out::DECLARED_MINOR;
}

/// The registry options of `canon schema`, which exclude a path and one another.
#[derive(Debug, Clone, Copy, Default)]
pub struct Registry<'a> {
    /// `--errors`
    pub errors: bool,
    /// `--error <CODE>`
    pub error: Option<&'a str>,
    /// `--warnings`
    pub warnings: bool,
    /// `--warning <CODE>`
    pub warning: Option<&'a str>,
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

/// Run `canon schema`: print to stdout and return the exit code. A path that
/// does not resolve is a refusal (exit 2, `UNKNOWN_COMMAND`) whose `expected` is
/// the valid words where it went wrong. The tree is unfiltered; `as_role` (the
/// `--as` flag, never `CANON_ROLE`) keeps only that role's commands and the groups
/// that hold one. `--errors` lists every error code and `--error <CODE>` shows one,
/// `--warnings` and `--warning <CODE>` the same for the warning registry (an unknown
/// code is `USAGE`, exit 2). `--schemas` answers for a command path with the JSON Schemas its
/// refs reach, `Input` and the `rejects` of its validators; the tree, a group or no path is
/// `USAGE` with `field` `--schemas` (spec 9.1.1). A tree, group or command document is the one
/// [`crate::spec::described`] writes.
pub fn run(root: &Command, tokens: &[String], json: bool, registry: Registry<'_>, schemas: bool, as_role: Option<Role>) -> u8 {
    if registry.errors {
        print_errors(json);
        return 0;
    }
    if let Some(code) = registry.error {
        return print_error(code, json);
    }
    if registry.warnings {
        print_warnings(json);
        return 0;
    }
    if let Some(code) = registry.warning {
        return print_warning(code, json);
    }
    // Building the tree first copies a parent's `global = true` options (`--as`) down to its leaves.
    let mut built = root.clone();
    built.build();
    let root = &built;
    // `canon schema "evidence add"` (one quoted word, dogfood eval 8) names
    // the same command as `canon schema evidence add` (spec 9.1.1).
    let words: Vec<String> = tokens.iter().flat_map(|t| t.split_whitespace()).map(str::to_string).collect();
    let resolved = match atip::project::resolve(root, &words) {
        Ok(resolved) => resolved,
        Err(unknown) => {
            let hint = if unknown.resolved.is_empty() { "canon schema --json".to_string() } else { format!("canon schema {} --json", unknown.resolved.join(" ")) };
            return crate::cli_error::CliError::refused("UNKNOWN_COMMAND", unknown.message()).field("command").received(unknown.word.clone()).expected(unknown.expected.clone()).suggestions(unknown.suggestions.clone()).hint(hint).emit("canon schema", json);
        }
    };
    if let atip::project::Resolved::Group(path) = &resolved {
        // A role's tree omits a group that holds none of its commands: its group document would be empty (spec 9.4).
        if let Some(role) = as_role.filter(|role| !spec::is_listed(root, path, *role)) {
            let message = format!("`{}` holds no command for the role `{}`", path.join(" "), role.as_str());
            let expected = spec::listed_children(root, &path[..path.len() - 1], role);
            return crate::cli_error::CliError::refused("UNKNOWN_COMMAND", message).field("command").received(path[path.len() - 1].clone()).expected(expected).hint(format!("canon schema --as {} --json", role.as_str())).emit("canon schema", json);
        }
    }
    if schemas {
        let (what, hint) = match &resolved {
            atip::project::Resolved::Command(_) => ("", String::new()),
            atip::project::Resolved::Tree => ("no command path", "canon schema --json".to_string()),
            atip::project::Resolved::Group(path) => ("a group", format!("canon schema {} --json", path.join(" "))),
        };
        if !what.is_empty() {
            return crate::cli_error::CliError::refused("USAGE", format!("`--schemas` answers for one command, and the call names {what}")).field("--schemas").hint(hint).emit("canon schema", json);
        }
    }
    if !json && !schemas {
        print_human(root, &resolved, as_role);
        return 0;
    }
    let described = spec::described(root, as_role).expect("the command spec is built from the tables the tests keep whole");
    let name = |path: &[String]| path.join(" ");
    let document = match &resolved {
        atip::project::Resolved::Tree => described.tree(),
        atip::project::Resolved::Group(path) => described.group(&name(path)).expect("a resolved group is in the tree"),
        atip::project::Resolved::Command(path) if schemas => described.schemas(&name(path)).expect("a resolved command has its schemas answer"),
        atip::project::Resolved::Command(path) => described.command(&name(path)).expect("a resolved command has its document"),
    };
    if json {
        crate::json_out::data("schema", &SchemaReport::Document(document));
    } else {
        // Without `--json` the schemas answer is the document itself, on one line (spec 9.1.1).
        atip::print_line(&atip::line(&document));
    }
    0
}

/// `canon schema --errors`: the whole registry, sorted by code.
fn print_errors(json: bool) {
    let entries = spec::error_rows();
    if json {
        crate::json_out::data("schema", &SchemaReport::Errors(ErrorList { kind: "errors", errors: entries }));
    } else {
        let width = entries.iter().map(|e| e.code.len()).max().unwrap_or(0);
        for e in &entries {
            println!("{:<width$}  {}", e.code, e.meaning);
        }
    }
}

/// `canon schema --warnings`: the whole warning registry, sorted by code.
fn print_warnings(json: bool) {
    let entries = spec::warning_rows();
    if json {
        crate::json_out::data("schema", &SchemaReport::Warnings(WarningList { kind: "warnings", warnings: entries }));
    } else {
        let width = entries.iter().map(|e| e.code.len()).max().unwrap_or(0);
        for e in &entries {
            println!("{:<width$}  {}", e.code, e.meaning);
        }
    }
}

/// The registered codes that contain `code` or are contained in it, for `suggestions`.
fn near_codes(code: &str, known: impl Iterator<Item = &'static str>) -> Vec<String> {
    known.filter(|known| known.contains(code) || code.contains(known)).take(5).map(str::to_string).collect()
}

/// `canon schema --error <CODE>`: one registry row. An unknown code, or a code that is only a
/// warning, is `USAGE` with `field` `--error` (spec 7.3.4).
fn print_error(code: &str, json: bool) -> u8 {
    let Some(row) = spec::error_rows().into_iter().find(|row| row.code == code) else {
        // A code that is only a warning points at the warning registry; any other unknown code at the list.
        let (hint, nearest) = if crate::cli_error::warning_info(code).is_some() {
            (format!("canon schema --warning {code} --json"), Vec::new())
        } else {
            ("canon schema --errors --json".to_string(), near_codes(code, crate::cli_error::all_codes().iter().map(|info| info.code)))
        };
        return crate::cli_error::CliError::refused("USAGE", format!("`{code}` is not an error code canon has")).field("--error").received(code).suggestions(nearest).hint(hint).emit("canon schema", json);
    };
    if json {
        crate::json_out::data("schema", &SchemaReport::Error(ErrorEntry { kind: "error", error: row }));
    } else {
        println!("{}  exit {}  retryable: {}", row.code, row.exit, row.retryable);
        println!("  {}", row.meaning);
        println!("  fix: {}", row.fix);
        for reason in &row.reasons {
            println!("  reason {}: {} ({})", reason.reason, reason.meaning, reason.fix);
        }
    }
    0
}

/// `canon schema --warning <CODE>`: one warning registry row; an unknown code is `USAGE` with `field` `--warning`.
fn print_warning(code: &str, json: bool) -> u8 {
    let Some(row) = spec::warning_rows().into_iter().find(|row| row.code == code) else {
        let nearest = near_codes(code, crate::cli_error::all_warnings().iter().map(|info| info.code));
        return crate::cli_error::CliError::refused("USAGE", format!("`{code}` is not a warning code canon has")).field("--warning").received(code).suggestions(nearest).hint("canon schema --warnings --json").emit("canon schema", json);
    };
    if json {
        crate::json_out::data("schema", &SchemaReport::Warning(WarningEntry { kind: "warning", warning: row }));
    } else {
        println!("{}", row.code);
        println!("  {}", row.meaning);
        println!("  fix: {}", row.fix);
    }
    0
}

/// The roles of a document's `x-canon`, as a line.
fn roles_line(extensions: &std::collections::BTreeMap<String, serde_json::Value>) -> Option<String> {
    let roles = extensions.get("x-canon")?.get("roles")?.as_array()?;
    Some(if roles.is_empty() { "humans only".to_string() } else { roles.iter().filter_map(serde_json::Value::as_str).collect::<Vec<_>>().join(", ") })
}

fn print_entries(entries: &[atip::doc::TreeEntry]) {
    let width = entries.iter().map(|entry| entry.command.len()).max().unwrap_or(0);
    for entry in entries {
        println!("{:<width$}  {:<11} {}", entry.command, entry.effect.map_or("group", atip::doc::Effect::as_str), entry.summary);
    }
}

/// The text of a tree, a group or a command, read from the typed model (the full form), not from the document `--json` writes.
fn print_human(root: &Command, resolved: &atip::project::Resolved, as_role: Option<Role>) {
    const BUILT: &str = "the command spec is built from the tables the tests keep whole";
    match resolved {
        atip::project::Resolved::Tree => print_entries(&spec::tree(root, as_role).expect(BUILT).commands),
        atip::project::Resolved::Group(path) => {
            let group = spec::group(root, path, as_role).expect(BUILT);
            println!("canon {}", group.command);
            println!("  {}", group.summary);
            println!("a group of commands:");
            print_entries(&group.commands);
        }
        atip::project::Resolved::Command(path) => print_command(&spec::command(root, path).expect(BUILT)),
    }
}

fn print_command(doc: &atip::doc::CommandDoc) {
    use atip::doc::ValueType;
    println!("canon {}", doc.command);
    println!("  {}", doc.summary);
    println!("effect: {}", doc.effect.as_str());
    if let Some(note) = &doc.effect_note {
        println!("  {note}");
    }
    if let Some(idempotency) = doc.idempotency {
        println!("idempotency: {}", serde_json::to_value(idempotency).expect("idempotency serializes").as_str().unwrap_or_default());
    }
    if let Some(roles) = roles_line(&doc.extensions) {
        println!("roles: {roles}");
    }
    let notes = |required: bool, repeatable: bool, values: Option<&[String]>, default: &serde_json::Value, note: Option<&str>| {
        let mut notes = Vec::new();
        if required {
            notes.push("required".to_string());
        }
        if repeatable {
            notes.push("repeatable".to_string());
        }
        if let Some(values) = values {
            notes.push(format!("one of: {}", values.join("|")));
        }
        if !default.is_null() {
            notes.push(format!("default: {default}"));
        }
        if let Some(note) = note {
            notes.push(format!("default: {note}"));
        }
        if notes.is_empty() {
            String::new()
        } else {
            format!("  [{}]", notes.join("; "))
        }
    };
    if !doc.operands.is_empty() {
        println!("operands:");
        for o in &doc.operands {
            println!("  {} <{}> {}{}", o.name, o.value_name, o.value_type.as_str(), notes(o.required, o.variadic, o.possible_values.as_deref(), &o.default, o.default_note.as_deref()));
        }
    }
    if !doc.options.is_empty() {
        println!("options:");
        for o in &doc.options {
            let value = o.value_name.as_ref().map(|v| format!(" <{v}>")).unwrap_or_default();
            let flag_only = if o.flag_only { "; flag only" } else { "" };
            let shown = notes(o.required, o.repeatable && o.value_type != ValueType::Count, o.possible_values.as_deref(), &o.default, o.default_note.as_deref());
            println!("  --{}{value} {}{shown}{flag_only}", o.name, o.value_type.as_str());
        }
    }
    println!("errors: {}", doc.errors.join(", "));
    for output in &doc.outputs {
        println!("output: {}{}", output.schema_ref, output.when.as_ref().map(|when| format!(" (when {when})")).unwrap_or_default());
    }
    for example in &doc.examples {
        println!("example: {example}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, ArgAction};

    fn tree() -> Command {
        Command::new("canon")
            .subcommand(Command::new("format").arg(Arg::new("root")).arg(Arg::new("json").long("json").action(ArgAction::SetTrue)))
            .subcommand(Command::new("evidence").subcommand(Command::new("add").arg(Arg::new("scenario").long("scenario").action(ArgAction::Append).required(true))))
    }

    #[test]
    fn drift_is_reported_both_ways() {
        let meta = [row("format", Read, EVERYONE), row("ghost", Read, EVERYONE)];
        let problems = check_metadata(&tree(), &meta);
        assert!(problems.iter().any(|p| p.contains("`evidence add`") && p.contains("no metadata row")));
        assert!(problems.iter().any(|p| p.contains("`ghost`")));
        assert!(check_metadata(&tree(), &[row("format", Read, EVERYONE), row("evidence add", Write, &[I])]).is_empty());
    }
}
