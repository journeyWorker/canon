//! The generated command spec (0.28 D2): everything an agent, a doc linter or
//! a completion script needs to know about `canon`, in one document, built
//! from the clap tree ([`crate::cli::command`]) and the tables beside it.
//!
//! - clap says each node's name, about, operands and options (help, value
//!   name, default, `possibleValues`, `env`, conflicts, requirements) and the
//!   argument groups of which one is required;
//! - [`crate::schema::META`] says what a leaf does (`effect`, `effectNote`,
//!   `roles`, `exits`) and [`crate::spec_facts::FACTS`] how a repeat behaves
//!   (`idempotency`) and what it prints (`outputs`);
//! - [`crate::schema_codes`] says which error codes a leaf can emit, and
//!   [`crate::cli_error::all_codes`] what each means;
//! - [`crate::reports`] holds the report and envelope JSON Schemas.
//!
//! [`generate`] is the whole document and [`render`] its checked-in text,
//! `docs/cli/canon.cli.json`; `tests/cli_spec.rs` regenerates it and
//! byte-diffs (`CANON_BLESS_CLI_SPEC=1` rewrites it). `canon schema`
//! renders its command nodes from [`nodes`], so what it prints and what the
//! spec says cannot differ.
//!
//! The output is deterministic: nodes sort by command, options keep their
//! declaration order, every other list is sorted or in a fixed order, and
//! object keys sort (the workspace's `serde_json` has no `preserve_order`).

use clap::{Arg, ArgAction, Command};
use serde::Serialize;
use serde_json::{json, Value};

use crate::role::Role;
use crate::schema::{error_codes, example_for, meta_for, Effect, ExitMeta, STANDARD_EXITS};
use crate::spec_facts::{facts_for, Idempotency, OutputKind};

/// The version of this document's own layout.
pub const SPEC_VERSION: &str = "1";

/// The repo-relative path of the checked-in spec.
pub const SPEC_PATH: &str = "docs/cli/canon.cli.json";

/// The exit table every leaf's own `exits` refines (0.28 D4).
const EXIT_TABLE: &[(u8, &str)] = &[
    (0, "ok"),
    (1, "the command's finding, a failed check, or an unverifiable result"),
    (2, "refused: usage, CONFIG_MISSING, an id or file that does not exist, unreadable input, or an invalid argument"),
];

/// An environment variable canon reads and a user may set.
struct EnvVar {
    name: &'static str,
    about: &'static str,
}

/// The user-facing variables, sorted by name. A variable read under
/// `crates/` that is neither here nor in [`INTERNAL_ENV`] fails
/// `tests/cli_spec.rs`.
const ENV_VARS: &[EnvVar] = &[
    EnvVar { name: "CANON_ACTOR", about: "Default actor id (`--actor-id`) of `feature new` and `scenario new`, and the actor of `query` and `export` audit lines; falls back to `USER`" },
    EnvVar { name: "CANON_INGEST_CLAUDE_SESSIONS_DIR", about: "An extra directory `ingest sessions` scans for Claude Code sessions, besides `~/.claude/projects`" },
    EnvVar { name: "CANON_INGEST_OMP_SESSIONS_DIR", about: "An extra directory `ingest sessions` scans for omp sessions, besides the default roots" },
    EnvVar { name: "CANON_PG_DSN", about: "Connection string of the Postgres tier (falls back to `DATABASE_URL`)" },
    EnvVar { name: "CANON_ROLE", about: "The role you act as: planner, implementer, reviewer or orchestrator; `--as` overrides it. Never filters or fails `schema`, `context` or `skills show`" },
    EnvVar { name: "CANON_S3_ACCESS_KEY", about: "Access key of an s3 tier's object store (required in a release build)" },
    EnvVar { name: "CANON_S3_ENDPOINT", about: "Endpoint of an s3 tier's object store (required in a release build)" },
    EnvVar { name: "CANON_S3_REGION", about: "Region of an s3 tier's object store (default `us-east-1`)" },
    EnvVar { name: "CANON_S3_SECRET_KEY", about: "Secret key of an s3 tier's object store (required in a release build)" },
    EnvVar { name: "CANON_SKILLS_SOURCE", about: "The directory `skills install|check|doctor` read the canon skills from, when `--source` is not given (default `canon/skills`)" },
];

/// Variables canon reads or sets that are not a user's to set: a build input,
/// the roots canon points its own DuckDB views at, and a debug-build test seam.
pub const INTERNAL_ENV: &[&str] = &[
    "CANON_ARCHIVE_ROOT",
    "CANON_DASHBOARD_DIST",
    "CANON_GIT_ROOT",
    "CANON_HOT_ROOT",
    "CANON_LEARN_ROOT",
    "CANON_R2_LOCAL_ROOT",
    "CANON_R2_ROOT",
];

/// Every `CANON_*` variable the spec names.
pub fn env_var_names() -> impl Iterator<Item = &'static str> {
    ENV_VARS.iter().map(|v| v.name)
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Leaf,
    Group,
}

/// One operand of a leaf.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Operand {
    /// The clap id.
    pub name: String,
    /// The placeholder the operand is written as (`CHANGE_ID`).
    pub kind: String,
    pub arity: Arity,
    pub required: bool,
    pub repeatable: bool,
    pub possible_values: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub default: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// How many values an operand takes; `max` is absent when unbounded.
#[derive(Serialize)]
pub struct Arity {
    pub min: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<usize>,
}

/// One option of a leaf (or a global one).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionSpec {
    /// The long name, without `--`.
    pub name: String,
    /// `flag` (no value), `count`, or `value`.
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<char>,
    pub aliases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_name: Option<String>,
    pub required: bool,
    pub repeatable: bool,
    pub possible_values: Vec<String>,
    pub default: Vec<String>,
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    /// Options that cannot be given with this one, as `--name`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conflicts_with: Vec<String>,
    /// Options that must be given with this one, as `--name`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
}

/// One output of a leaf: what it prints and the schema of that.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub kind: OutputKind,
    /// The report's name; for an artifact the report that envelopes it under `--json`.
    pub report: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<&'static str>,
    /// `#/schemas/<report>`; absent while the report has no schema.
    pub schema_ref: Option<String>,
    pub schema_version: Option<&'static str>,
}

/// The part of a node only a leaf has.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leaf {
    pub effect: Effect,
    /// What the command does when invoked with fewer effects than `effect` says.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect_note: Option<&'static str>,
    /// Absent for a read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency: Option<Idempotency>,
    /// The roles whose steps use the command; empty means humans only.
    pub roles: Vec<Role>,
    pub supports_json: bool,
    pub operands: Vec<Operand>,
    pub options: Vec<OptionSpec>,
    /// Option sets of which exactly one is required.
    pub required_one_of: Vec<Vec<String>>,
    pub exits: Vec<ExitMeta>,
    pub error_codes: Vec<&'static str>,
    pub outputs: Vec<Output>,
    /// JSON input documents of the leaf (`--input`).
    pub inputs: Vec<Value>,
    pub examples: Vec<&'static str>,
}

/// One node of the command tree: a leaf, or a group that only holds children.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// The canonical space-separated path, e.g. `evidence add`.
    pub command: String,
    pub path: Vec<String>,
    pub name: String,
    pub node_kind: NodeKind,
    pub aliases: Vec<String>,
    pub about: Option<String>,
    /// A group's children, as commands.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
    #[serde(flatten)]
    pub leaf: Option<Leaf>,
}

/// What an option holds when it is not given. `--repo` (the shared `RepoArgs`)
/// has none: absent, it means the nearest ancestor directory holding a
/// `canon.yaml` (D5); `init` and `demo init` default to `.`, used as given.
fn default_values(arg: &Arg) -> Vec<String> {
    arg.get_default_values().iter().map(|v| v.to_string_lossy().into_owned()).collect()
}

/// Clap's own `--help` / `--version`: noise in a spec. A command's own
/// argument that happens to be called `version` is not.
fn is_builtin(arg: &Arg) -> bool {
    matches!(arg.get_action(), ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong | ArgAction::Version)
}

fn display_name(arg: &Arg) -> String {
    arg.get_long().map(|l| format!("--{l}")).unwrap_or_else(|| arg.get_id().to_string())
}

fn value_name(arg: &Arg) -> String {
    arg.get_value_names().and_then(|v| v.first()).map(|v| v.to_string()).unwrap_or_else(|| arg.get_id().as_str().to_uppercase())
}

fn is_repeatable(arg: &Arg) -> bool {
    matches!(arg.get_action(), ArgAction::Append | ArgAction::Count) || arg.get_num_args().is_some_and(|r| r.max_values() > 1)
}

fn kind_of(arg: &Arg) -> &'static str {
    match arg.get_action() {
        ArgAction::SetTrue | ArgAction::SetFalse => "flag",
        ArgAction::Count => "count",
        _ => "value",
    }
}

/// The ids `arg` requires. clap keeps them private (`Arg::requires` has no
/// getter), but its `Debug` prints them: `requires: [(IsPresent, "flow")]`.
/// `tests/cli_spec.rs` pins one known requirement, so a clap that changes the
/// print fails the suite instead of emptying the spec.
fn required_ids(arg: &Arg) -> Vec<String> {
    let text = format!("{arg:?}");
    let Some(start) = text.find("requires: [") else { return Vec::new() };
    let rest = &text[start + "requires: [".len()..];
    let mut depth = 1usize;
    let mut end = rest.len();
    let mut in_str = false;
    for (i, c) in rest.char_indices() {
        match c {
            '"' => in_str = !in_str,
            '[' if !in_str => depth += 1,
            ']' if !in_str => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    // Each entry is `(<predicate>, "<id>")`; the id is the last quoted string.
    rest[..end]
        .split("), (")
        .filter_map(|entry| {
            let close = entry.rfind('"')?;
            let open = entry[..close].rfind('"')?;
            Some(entry[open + 1..close].to_string())
        })
        .collect()
}

/// The names a `requires` / conflict id stands for: an argument's own name, or
/// every member of the group it names.
fn names_of(cmd: &Command, id: &str) -> Vec<String> {
    if let Some(arg) = cmd.get_arguments().find(|a| a.get_id() == id) {
        return vec![display_name(arg)];
    }
    cmd.get_groups()
        .find(|g| g.get_id() == id)
        .map(|g| g.get_args().filter_map(|member| cmd.get_arguments().find(|a| a.get_id() == member)).map(display_name).collect())
        .unwrap_or_default()
}

fn option_spec(cmd: &Command, arg: &Arg, conflicts: &[(String, String)]) -> OptionSpec {
    let kind = kind_of(arg);
    let name = display_name(arg);
    let mut conflicts_with: Vec<String> = conflicts.iter().filter(|(a, _)| *a == name).map(|(_, b)| b.clone()).collect();
    conflicts_with.sort();
    conflicts_with.dedup();
    let mut requires: Vec<String> = required_ids(arg).iter().flat_map(|id| names_of(cmd, id)).collect();
    requires.sort();
    requires.dedup();
    OptionSpec {
        name: arg.get_long().map(str::to_string).unwrap_or_else(|| arg.get_id().to_string()),
        kind,
        short: arg.get_short(),
        aliases: arg.get_all_aliases().map(|a| a.iter().map(|s| s.to_string()).collect()).unwrap_or_default(),
        value_name: (kind == "value").then(|| value_name(arg)),
        required: arg.is_required_set(),
        repeatable: is_repeatable(arg),
        possible_values: arg.get_possible_values().iter().filter(|v| !v.is_hide_set()).map(|v| v.get_name().to_string()).collect(),
        // A flag is off unless given; clap's `false` is not a default worth listing.
        default: if kind == "value" { default_values(arg) } else { Vec::new() },
        help: arg.get_help().map(|h| h.to_string()),
        env: arg.get_env().map(|e| e.to_string_lossy().into_owned()),
        conflicts_with,
        requires,
    }
}

fn operand_spec(arg: &Arg, note: Option<&str>) -> Operand {
    let range = arg.get_num_args();
    let required = arg.is_required_set();
    let (min, max) = range.map_or((1, 1), |r| (r.min_values(), r.max_values()));
    Operand {
        name: arg.get_id().to_string(),
        kind: value_name(arg),
        arity: Arity { min: if required { min.max(1) } else { 0 }, max: (max < usize::MAX / 2).then_some(max) },
        required,
        repeatable: is_repeatable(arg),
        possible_values: arg.get_possible_values().iter().filter(|v| !v.is_hide_set()).map(|v| v.get_name().to_string()).collect(),
        default: default_values(arg),
        description: note.map(str::to_string).or_else(|| arg.get_help().map(|h| h.to_string())),
    }
}

/// Every pair `(a, b)` of options of `cmd` that cannot be given together, in
/// both directions: clap enforces a conflict whichever side declared it, and
/// the members of a group that allows one are in conflict with each other.
fn conflict_pairs(cmd: &Command) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for arg in cmd.get_arguments().filter(|a| !is_builtin(a)) {
        for other in cmd.get_arg_conflicts_with(arg) {
            pairs.push((display_name(arg), display_name(other)));
            pairs.push((display_name(other), display_name(arg)));
        }
    }
    for group in cmd.get_groups() {
        if (*group).clone().is_multiple() {
            continue;
        }
        let members: Vec<String> = group.get_args().filter_map(|id| cmd.get_arguments().find(|a| a.get_id() == id)).map(display_name).collect();
        for a in &members {
            for b in members.iter().filter(|b| *b != a) {
                pairs.push((a.clone(), b.clone()));
            }
        }
    }
    pairs
}

fn leaf_of(path: &str, cmd: &Command, reports: &[crate::reports::ReportSchema]) -> Leaf {
    let meta = meta_for(path);
    let facts = facts_for(path);
    let effect = meta.map_or(Effect::Write, |m| m.effect);
    let conflicts = conflict_pairs(cmd);
    let operands: Vec<Operand> = cmd
        .get_positionals()
        .filter(|a| !is_builtin(a))
        .map(|a| operand_spec(a, facts.and_then(|f| f.operands.iter().find(|o| o.name == a.get_id().as_str())).map(|o| o.about)))
        .collect();
    let options: Vec<OptionSpec> = cmd.get_arguments().filter(|a| !a.is_positional() && !a.is_global_set() && !is_builtin(a)).map(|a| option_spec(cmd, a, &conflicts)).collect();
    let supports_json = options.iter().any(|o| o.name == "json");
    let outputs = facts
        .map(|f| f.outputs)
        .unwrap_or_default()
        .iter()
        .map(|o| {
            let schema = reports.iter().find(|r| r.name == o.report);
            Output {
                kind: o.kind,
                report: o.report,
                media_type: o.media_type,
                schema_ref: schema.map(|r| format!("#/schemas/{}", r.name)),
                schema_version: schema.map(|r| r.schema_version),
            }
        })
        .collect();
    Leaf {
        effect,
        effect_note: meta.and_then(|m| m.effect_note),
        idempotency: facts.and_then(|f| f.idempotency),
        roles: meta.map_or_else(Vec::new, |m| m.roles.to_vec()),
        supports_json,
        operands,
        options,
        required_one_of: cmd
            .get_groups()
            .filter(|g| g.is_required_set() && !(*g).clone().is_multiple())
            .map(|g| g.get_args().filter_map(|id| cmd.get_arguments().find(|a| a.get_id() == id)).map(display_name).collect())
            .collect(),
        exits: meta.map_or(STANDARD_EXITS, |m| m.exits).to_vec(),
        error_codes: error_codes(path),
        outputs,
        inputs: Vec::new(),
        examples: example_for(path).into_iter().collect(),
    }
}

/// The subcommands of `cmd` a user can run: everything but clap's own `help`.
fn subcommands(cmd: &Command) -> impl Iterator<Item = &Command> {
    cmd.get_subcommands().filter(|c| c.get_name() != "help")
}

/// Every node of the tree under `root` (leaves and groups, not the root),
/// sorted by command. `root` must be built (`Command::build`) so a parent's
/// global flags have reached its leaves.
pub fn nodes(root: &Command) -> Vec<Node> {
    fn walk(cmd: &Command, prefix: &mut Vec<String>, reports: &[crate::reports::ReportSchema], out: &mut Vec<Node>) {
        for sub in subcommands(cmd) {
            prefix.push(sub.get_name().to_string());
            let command = prefix.join(" ");
            let is_group = subcommands(sub).next().is_some();
            out.push(Node {
                path: prefix.clone(),
                name: sub.get_name().to_string(),
                node_kind: if is_group { NodeKind::Group } else { NodeKind::Leaf },
                aliases: sub.get_all_aliases().map(str::to_string).collect(),
                about: sub.get_about().map(|a| a.to_string()),
                children: subcommands(sub).map(|c| format!("{command} {}", c.get_name())).collect(),
                leaf: (!is_group).then(|| leaf_of(&command, sub, reports)),
                command,
            });
            walk(sub, prefix, reports, out);
            prefix.pop();
        }
    }
    let reports = crate::reports::report_schemas();
    let mut out = Vec::new();
    walk(root, &mut Vec::new(), &reports, &mut out);
    out.sort_by(|a, b| a.command.cmp(&b.command));
    for node in &mut out {
        node.children.sort();
    }
    out
}

fn global_options(root: &Command) -> Vec<OptionSpec> {
    let no_conflicts = [];
    root.get_arguments().filter(|a| a.is_global_set() && !is_builtin(a)).map(|a| option_spec(root, a, &no_conflicts)).collect()
}

/// One error code of the registry.
#[derive(Serialize)]
pub struct ErrorRow {
    pub code: &'static str,
    pub meaning: &'static str,
    /// The exit code a command that fails with it returns.
    pub exit: i32,
    /// Whether running the same command again, unchanged, can succeed.
    pub retryable: bool,
    /// One sentence telling the caller what to do about the code.
    pub fix: &'static str,
}

/// Every registered error code, sorted by code.
pub fn error_rows() -> Vec<ErrorRow> {
    crate::cli_error::all_codes().iter().map(|info| ErrorRow { code: info.code, meaning: info.meaning, exit: info.exit, retryable: info.retryable, fix: info.fix }).collect()
}

/// Every removed spelling with its replacement (the README's Upgrading list
/// and the usage-error hints are generated from it).
pub fn removed_rows() -> Vec<Value> {
    crate::removed_028::ROWS
        .iter()
        .map(|row| json!({ "kind": row.kind, "old": row.old, "scope": row.scope, "new": row.new, "note": row.note, "rule": row.rule, "since": row.since }))
        .collect()
}

/// The record kinds and artifact families, linked to their JSON Schemas under
/// `crates/canon-model/schemas`.
fn records() -> Value {
    use canon_model::envelope::RecordKind;
    use canon_model::family::FamilyKind;
    let dir = "crates/canon-model/schemas";
    let kinds: Vec<Value> = RecordKind::ALL
        .iter()
        .map(|k| json!({ "kind": k.as_str(), "schemaVersion": k.schema_version(), "partition": k.partition_template(), "schema": format!("{dir}/{}.schema.json", k.as_str()) }))
        .collect();
    let families: Vec<Value> = FamilyKind::ALL.iter().map(|f| json!({ "family": f.as_str(), "schema": format!("{dir}/family-{}.schema.json", f.as_str()) })).collect();
    json!({ "kinds": kinds, "families": families })
}

/// Gives an embedded JSON Schema its own base URI, so that its `#/$defs/…`
/// references resolve inside it and not against the spec document.
fn with_id(mut schema: Value, id: String) -> Value {
    if let Some(object) = schema.as_object_mut() {
        object.insert("$id".to_string(), Value::String(id));
    }
    schema
}

/// The JSON Schema of every report, keyed by report name.
fn schemas() -> Value {
    let mut map = serde_json::Map::new();
    for report in crate::reports::report_schemas() {
        let id = format!("urn:canon:report:{}", report.name);
        map.insert(report.name, with_id(report.schema, id));
    }
    Value::Object(map)
}

/// The success and failure envelope schemas, each with its own base URI.
fn envelopes() -> Value {
    match crate::reports::envelope_schemas() {
        Value::Object(map) => Value::Object(map.into_iter().map(|(name, schema)| (name.clone(), with_id(schema, format!("urn:canon:envelope:{name}")))).collect()),
        other => other,
    }
}

/// The whole spec.
pub fn generate() -> Value {
    let mut root = crate::cli::command();
    root.build();
    json!({
        "_doNotEdit": "GENERATED from the clap tree and the tables beside it: do not edit. Regenerate with `CANON_BLESS_CLI_SPEC=1 cargo test -p canon-cli --test cli_spec`.",
        "specVersion": SPEC_VERSION,
        "cli": {
            "name": root.get_name(),
            "version": env!("CARGO_PKG_VERSION"),
            "about": root.get_about().map(|a| a.to_string()),
        },
        "globalOptions": global_options(&root),
        "envVars": ENV_VARS.iter().map(|v| json!({ "name": v.name, "about": v.about })).collect::<Vec<_>>(),
        "exitTable": EXIT_TABLE.iter().map(|(code, meaning)| json!({ "code": code, "meaning": meaning })).collect::<Vec<_>>(),
        "envelopes": envelopes(),
        "errors": error_rows(),
        "removed": removed_rows(),
        "commands": nodes(&root),
        "schemas": schemas(),
        "records": records(),
    })
}

/// [`generate`] as the text of `docs/cli/canon.cli.json`: pretty-printed, one
/// trailing newline.
pub fn render() -> String {
    let mut text = serde_json::to_string_pretty(&generate()).expect("the spec serializes");
    text.push('\n');
    text
}
