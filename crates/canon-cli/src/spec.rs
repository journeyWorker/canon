//! The generated command spec (0.28 D2, 0.34 L1): everything an agent, a doc linter or
//! a completion script needs to know about `canon`, in one document, built
//! from the clap tree ([`crate::cli::command`]) and the tables beside it.
//!
//! The documents are the contract's (atip/1 spec 10): [`tree`] (the groups and commands with `level`), [`group`]
//! and [`command`] (one command with its options, outputs, input, examples and the JSON Schemas its refs name).
//! `canon schema` prints them and the checked-in `docs/cli/canon.cli.json` holds them, from the same functions, so
//! what the file says of a command and what `canon schema <COMMAND>` answers are one value.
//!
//! - clap says each node's name, summary, operands and options (help, value name, typed default,
//!   `possibleValues`, `env`); [`atip::project`] reads them and holds the parts every clap tool shares;
//! - [`crate::schema::META`] says what a leaf does (`effect`, `effectNote`, `roles`) and
//!   [`crate::spec_facts::FACTS`] how a repeat behaves (`idempotency`) and what it prints (`outputs`);
//! - [`crate::schema_codes`] says which error codes a leaf can emit, and
//!   [`crate::cli_error::all_codes`] what each means;
//! - [`crate::reports`] holds the report and envelope JSON Schemas, and [`crate::input`] the `--input` documents.
//!
//! [`generate`] is the whole file and [`render`] its checked-in text; `tests/cli_spec.rs` regenerates it and
//! byte-diffs it (`CANON_BLESS_CLI_SPEC=1` rewrites it).
//!
//! The output is deterministic: the tree is in command order, options keep their declaration order, every
//! other list is sorted or in a fixed order, and the file's object keys sort (the workspace's `serde_json` has
//! no `preserve_order`).

use std::collections::{BTreeMap, BTreeSet};

use atip::doc::{CommandDoc, EntryKind, GroupDoc, InputDoc, OperandDoc, OptionDoc, OutputDoc, OutputKind, TreeDoc, TreeEntry, ValidatorDoc, ValueType};
use atip::domain::Domain;
use atip::project::{self, Node};
use clap::Command;
use serde::Serialize;
use serde_json::{json, Value};

use crate::cli_error::feature_codes;
use crate::role::Role;
use crate::schema::{error_codes, example_for, meta_for};
use crate::spec_facts::facts_for;

/// The version of this document's own layout: `2` is the L1 shape (a tree and command documents in the contract's names).
pub const SPEC_VERSION: &str = "2";

/// The repo-relative path of the checked-in spec.
pub const SPEC_PATH: &str = "docs/cli/canon.cli.json";

/// What exit 0, 1 and 2 mean for every command (spec 8); the exit of a code is in the registry.
const EXIT_TABLE: &[(u8, &str)] = &[
    (0, "ok"),
    (1, "the command ran and its answer is negative or incomplete: a finding, a failed check, an unverifiable result, a failure after the work began, an internal error"),
    (2, "the command refused to run: usage, an unknown command, CONFIG_MISSING, an id or file that does not exist, unreadable input, an invalid argument, a target that cannot be written"),
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
    EnvVar { name: "CANON_ACTOR", about: "Default actor id (`--actor`) of `feature new` and `scenario new`, and the actor of `query` and `export` audit lines; falls back to `USER`" },
    EnvVar { name: "CANON_INGEST_CLAUDE_SESSIONS_DIR", about: "An extra directory `ingest sessions` scans for Claude Code sessions, besides `~/.claude/projects`" },
    EnvVar { name: "CANON_INGEST_OMP_SESSIONS_DIR", about: "An extra directory `ingest sessions` scans for omp sessions, besides the default roots" },
    EnvVar { name: "CANON_PG_DSN", about: "Connection string of the Postgres tier (falls back to `DATABASE_URL`)" },
    EnvVar { name: "CANON_ROLE", about: "The role you act as: planner, implementer, reviewer or orchestrator; `--as` overrides it. Never filters or fails `context` or `skills show`; `schema` does not read it" },
    EnvVar { name: "CANON_S3_ACCESS_KEY", about: "Access key of an s3 tier's object store (required in a release build)" },
    EnvVar { name: "CANON_S3_ENDPOINT", about: "Endpoint of an s3 tier's object store (required in a release build)" },
    EnvVar { name: "CANON_S3_REGION", about: "Region of an s3 tier's object store (default `us-east-1`)" },
    EnvVar { name: "CANON_S3_SECRET_KEY", about: "Secret key of an s3 tier's object store (required in a release build)" },
    EnvVar { name: "CANON_SKILLS_SOURCE", about: "Developer override: a directory `skills install|check|doctor` read `SKILL.src.md` and `scripts/` from, when `--skills-source` is not given (default: the text built into the binary)" },
];

/// Variables canon reads or sets that are not a user's to set: a build input,
/// the roots canon points its own DuckDB views at, and debug-build test seams (`CANON_WITNESS_DIR`:
/// where a debug build records which (command, report) pairs it emitted, for the T9 witness; `CANON_TEST_PANIC`:
/// a debug build panics at start, for the test of the `INTERNAL` error; `CANON_TEST_REPO_LOCK_WAIT_MS`: how long a debug
/// build waits for the repository write lock, in milliseconds; `CANON_TEST_HOLD_AFTER_RUN_MS`: how long a debug build keeps
/// the lock after its run: the two seams of the test of the lock).
pub const INTERNAL_ENV: &[&str] = &[
    "CANON_ARCHIVE_ROOT",
    "CANON_DASHBOARD_DIST",
    "CANON_GIT_ROOT",
    "CANON_HOT_ROOT",
    "CANON_LEARN_ROOT",
    "CANON_R2_LOCAL_ROOT",
    "CANON_R2_ROOT",
    "CANON_TEST_HOLD_AFTER_RUN_MS",
    "CANON_TEST_PANIC",
    "CANON_TEST_REPO_LOCK_WAIT_MS",
    "CANON_WITNESS_DIR",
];

/// Every `CANON_*` variable the spec names.
pub fn env_var_names() -> impl Iterator<Item = &'static str> {
    ENV_VARS.iter().map(|v| v.name)
}


/// The conformance level the tree claims (spec 3.4, 10.3.3). `canon` claims it once `atip check --level L1` passes on the
/// build: the check runs in CI beside the tests that build these documents.
pub const LEVEL: &str = "L1";

/// The first line of every `--help` (spec 4.7, 4.8): it points at the self-description and at the guide, so an agent that read nothing else
/// knows both entrances exist.
pub const HELP_POINTER: &str = "Agents: \"canon schema --json\" describes every command; \"canon skills show\" prints the guide.";

/// The value type of every parser canon wrote itself, by the value name of the argument that uses it (approaches/rust.md
/// 4.1). clap cannot read a type out of a function, and a guessed `string` is how canon once published none: an argument
/// with a parser of its own and a value name that is not here fails the generation (`tests/schema_doc.rs`
/// `atip.schemadoc.06`). Every one of canon's today is text that the parser checks the form of, so `string`.
pub const CUSTOM_PARSERS: &[(&str, ValueType)] = &[
    ("CHANGE_ID", ValueType::String),
    ("FORMAT:PATH", ValueType::String),
    ("KEY", ValueType::String),
    ("NAME", ValueType::String),
    ("NAME@VERSION", ValueType::String),
    ("PATH_OR_EFFECT", ValueType::String),
    ("PROJECT_ID", ValueType::String),
    ("REV", ValueType::String),
    ("REVIEW_ID", ValueType::String),
    ("ROLE", ValueType::String),
    ("RUN_ID", ValueType::String),
    ("SCENARIO_ID", ValueType::String),
    ("SESSION_ID", ValueType::String),
    ("STRATEGY_ID", ValueType::String),
    ("SUBJECT_ID", ValueType::String),
    ("SURFACE", ValueType::String),
    ("TASK_ID", ValueType::String),
    ("TIMESTAMP", ValueType::String),
];

/// How a default that clap does not hold is found, for the options whose help promises one: `(command or "*", argument name, note)`.
/// Used only where the typed default is `null`; a command's own row wins over `*`. `tests/schema_doc.rs` fails on a help text that
/// promises a default and has neither a typed default nor a note here.
const DEFAULT_NOTES: &[(&str, &str, &str)] = &[
    ("*", "repo", "the nearest ancestor directory holding a canon.yaml"),
    ("*", "actor", "the CANON_ACTOR environment variable, with --relayed-via"),
    ("*", "role", "the role declared with --as or CANON_ROLE"),
    ("*", "promote", "on under canon.yaml `write: { promote: auto }`"),
    ("*", "kind", "every kind"),
    ("*", "change", "the repo's only live change"),
    ("*", "dir", "every configured specs.roots[] entry"),
    ("*", "home", "$HOME"),
    ("*", "suite", "every suite"),
    ("*", "as-of", "now"),
    ("dashboard", "snapshot", "regenerated fresh each run"),
    ("migrate", "backup", "the newest under .canon/migrate-backup/"),
    ("scenario new", "change", "the --subject's newest adopted change"),
    ("scenario new", "feature", "derived from the scenario tag"),
    ("finding add", "resolution-rev", "HEAD when --disposition is fixed"),
    ("finding close", "resolution-rev", "HEAD when --disposition is fixed"),
    ("evidence add", "class", "test-run when a --report is given"),
    ("evidence add", "ref", "the --report paths, comma-separated, when one is given"),
    ("evidence add", "verdict", "derived from the bound cases when a --report is given, else faithful"),
    ("skills check", "skills-source", "CANON_SKILLS_SOURCE, else canon/skills"),
    ("skills doctor", "skills-source", "CANON_SKILLS_SOURCE, else canon/skills"),
    ("skills install", "skills-source", "CANON_SKILLS_SOURCE, else the text built into the binary"),
    ("skills install", "providers", "canon.yaml `skills.providers`, else the install lock's providers, else the detected provider directories"),
    ("skills check", "providers", "the providers `skills install` would use"),
    ("skills doctor", "providers", "the providers `skills install` would use"),
    ("skills show", "name", "the default guide, `canon`"),
];

/// The environment variable a command reads to fill an option that clap does not give an `env` (spec 16.3: a variable may
/// fill only an option that declares it, and a tool declares it per command). `canon schema` reads none: its `--as` is the flag alone.
fn declared_env(command: &str, option: &str) -> Option<&'static str> {
    match (command, option) {
        ("schema", _) => None,
        (_, "as" | "role") => Some(crate::role::ENV),
        ("skills install" | "skills check" | "skills doctor", "skills-source") => Some(crate::skills::CANONICAL_SOURCE_ENV),
        // The relayed human acts (`--relayed-via`) take the agent's actor id from the variable when `--actor` is not given.
        ("brief approve" | "migrate" | "policy approve", "actor") => Some(crate::relay::ACTOR_ENV),
        _ => None,
    }
}

fn default_note(command: &str, name: &str) -> Option<&'static str> {
    DEFAULT_NOTES.iter().find(|(scope, known, _)| *scope == command && *known == name).or_else(|| DEFAULT_NOTES.iter().find(|(scope, known, _)| *scope == "*" && *known == name)).map(|(_, _, note)| *note)
}

/// What a failed projection says: a defect of a table or a declaration, found when a document is built.
pub type Built<T> = Result<T, String>;

fn built<T>(result: Result<T, atip::project::Problem>) -> Built<T> {
    result.map_err(|problem| problem.to_string())
}

/// `x-canon` of a command: the roles whose steps use it (empty: humans only).
fn x_canon(path: &str) -> BTreeMap<String, Value> {
    let roles: Vec<&str> = meta_for(path).map_or_else(Vec::new, |meta| meta.roles.iter().map(|role| role.as_str()).collect());
    BTreeMap::from([("x-canon".to_string(), json!({ "roles": roles }))])
}

fn entry(node: &Node<'_>) -> Built<TreeEntry> {
    let summary = built(project::summary(node))?;
    Ok(if node.is_group {
        TreeEntry { command: node.command.clone(), path: node.path.clone(), kind: EntryKind::Group, summary, effect: None, extensions: BTreeMap::new() }
    } else {
        let effect = meta_for(&node.command).ok_or_else(|| format!("`{}` has no row in the metadata table", node.command))?.effect;
        TreeEntry { command: node.command.clone(), path: node.path.clone(), kind: EntryKind::Command, summary, effect: Some(effect), extensions: x_canon(&node.command) }
    })
}

/// Whether the tree of `role` lists `node`: a command of that role, or a group that holds one (`None`: everything).
fn listed_for(nodes: &[Node<'_>], node: &Node<'_>, role: Option<Role>) -> bool {
    match role {
        None => true,
        Some(role) if node.is_group => nodes.iter().any(|other| !other.is_group && other.command.starts_with(&format!("{} ", node.command)) && meta_for(&other.command).is_some_and(|meta| meta.roles.contains(&role))),
        Some(role) => meta_for(&node.command).is_some_and(|meta| meta.roles.contains(&role)),
    }
}

/// `canon schema --json`: every group and command, a group before its commands. `role` (`--as`) keeps the commands
/// of that role and the groups that hold one, and the tree names the role in `x-canon.as`; nothing else, the environment
/// included, filters it (spec 10.2.2).
pub fn tree(root: &Command, role: Option<Role>) -> Built<TreeDoc> {
    let nodes = project::nodes(root);
    let mut entries = Vec::with_capacity(nodes.len());
    for node in &nodes {
        if listed_for(&nodes, node, role) {
            entries.push(entry(node)?);
        }
    }
    // A filtered tree says so (spec 10.2.2, 10.12): the role is `x-canon.as`, and an unfiltered tree has no `x-canon`.
    let extensions = role.map(|role| ("x-canon".to_string(), json!({ "as": role.as_str() }))).into_iter().collect();
    Ok(TreeDoc { kind: TreeDoc::KIND, level: LEVEL.to_string(), commands: entries, extensions })
}

/// Whether the tree of `role` lists the group or command `path`.
pub fn is_listed(root: &Command, path: &[String], role: Role) -> bool {
    let nodes = project::nodes(root);
    let wanted = path.join(" ");
    nodes.iter().find(|node| node.command == wanted).is_some_and(|node| listed_for(&nodes, node, Some(role)))
}

/// The words that follow `path` in the tree of `role`: the valid next words of a path the role's tree does not list.
pub fn listed_children(root: &Command, path: &[String], role: Role) -> Vec<String> {
    let nodes = project::nodes(root);
    nodes.iter().filter(|node| node.path.len() == path.len() + 1 && node.path.starts_with(path) && listed_for(&nodes, node, Some(role))).filter_map(|node| node.path.last().cloned()).collect()
}

/// `canon schema <GROUP> --json`. With `role` (`--as`) the group lists only the children the role's tree lists, so the
/// group and the tree agree; a group the role's tree omits has none (the caller refuses it).
pub fn group(root: &Command, path: &[String], role: Option<Role>) -> Built<GroupDoc> {
    let nodes = project::nodes(root);
    let command = path.join(" ");
    let node = nodes.iter().find(|node| node.is_group && node.command == command).ok_or_else(|| format!("`{command}` is not a group"))?;
    let mut children = Vec::new();
    for child in &node.children {
        let child = nodes.iter().find(|other| other.command == *child).ok_or_else(|| format!("`{child}` is not in the tree"))?;
        if listed_for(&nodes, child, role) {
            children.push(entry(child)?);
        }
    }
    Ok(GroupDoc { kind: GroupDoc::KIND, command, path: node.path.clone(), summary: built(project::summary(node))?, commands: children, extensions: role.map(|role| ("x-canon".to_string(), json!({ "as": role.as_str() }))).into_iter().collect() })
}

/// The reserved codes of the features a command has (spec 7.2.1, 10.13 rule 4), with the codes it emits.
fn errors_of(path: &str, options: &[OptionDoc], operands: &[OperandDoc], batch: bool) -> Vec<String> {
    let mut codes: BTreeSet<String> = error_codes(path).into_iter().map(str::to_string).collect();
    codes.insert("USAGE".to_string());
    let has = |name: &str| options.iter().any(|option| option.name == name);
    if has("input") {
        codes.extend(feature_codes::INPUT.map(str::to_string));
    }
    if batch {
        codes.insert(feature_codes::BATCH.to_string());
    }
    if has("commit") {
        codes.insert(feature_codes::COMMIT.to_string());
    }
    if has("cursor") {
        codes.insert(feature_codes::CURSOR.to_string());
    }
    let arguments: Vec<(String, Option<String>)> = options.iter().map(|option| (option.name.clone(), option.value_name.clone())).chain(operands.iter().map(|operand| (operand.name.clone(), Some(operand.value_name.clone())))).collect();
    codes.extend(crate::arg_rules::codes_for(path, &arguments).into_iter().map(str::to_string));
    codes.into_iter().collect()
}

/// The outputs of a command and the schemas they name.
fn outputs_of(path: &str, batch: bool, schemas: &mut BTreeMap<String, Value>) -> Built<Vec<OutputDoc>> {
    let facts = facts_for(path).ok_or_else(|| format!("`{path}` has no row in the facts table"))?;
    let reports = crate::reports::schemas_of(path);
    let mut outputs = Vec::new();
    let mut add = |name: &str, kind: OutputKind, media_type: Option<&str>, batch: bool, when: Option<&str>, schemas: &mut BTreeMap<String, Value>| -> Built<()> {
        let report = reports.iter().find(|report| report.name == name).ok_or_else(|| format!("`{path}` names the report `{name}`, which has no schema"))?;
        project::add_schema(schemas, name, report.schema.clone())?;
        let fields = if batch { None } else { crate::fields::doc_for(path, name) };
        outputs.push(OutputDoc { kind, schema_ref: format!("#/schemas/{name}"), schema_version: report.schema_version.clone(), media_type: media_type.map(str::to_string), event_ref: None, batch, when: when.map(str::to_string), fields });
        Ok(())
    };
    for output in facts.outputs {
        add(output.report, output.kind, output.media_type, false, output.when, schemas)?;
    }
    if batch {
        let name = crate::reports::batch_output_name(path).ok_or_else(|| format!("`{path}` takes a batch and has no batch report"))?;
        add(&name, OutputKind::Report, None, true, Some("--input is an array or NDJSON"), schemas)?;
    }
    Ok(outputs)
}

/// The `--input` declaration of a command and the two schemas it names.
fn input_of(path: &str, cmd: &Command, operands: &[OperandDoc], options: &[OptionDoc], schemas: &mut BTreeMap<String, Value>) -> Option<InputDoc> {
    if !options.iter().any(|option| option.name == "input") {
        return None;
    }
    let words: Vec<String> = path.split(' ').map(str::to_string).collect();
    let leaf = crate::input::Leaf::of(cmd, &words);
    let name = crate::input::schema_name(path);
    let arguments = name.replacen("Input", "Arguments", 1);
    let (title, arguments_title) = (format!("`canon {path}` --input document"), format!("`canon {path}` arguments after the --input document is merged with the command line"));
    schemas.insert(name.clone(), project::fragment_schema(&title, operands, options));
    schemas.insert(arguments.clone(), project::arguments_schema(&arguments_title, operands, options, &leaf.required_one_of()));
    let batch = crate::input::batch_supported(path);
    Some(InputDoc {
        option: "--input",
        schema_ref: format!("#/schemas/{name}"),
        arguments_ref: Some(format!("#/schemas/{arguments}")),
        batch,
        max_bytes: crate::input::MAX_BYTES,
        max_items: batch.then_some(crate::input::MAX_ELEMENTS as u64),
        example: crate::input::example_document(cmd, &leaf).unwrap_or_else(|| json!({})),
    })
}

/// The examples of a command: the one table row, with `--json` after the path as the contract asks (spec 10.5).
fn examples_of(path: &str) -> Built<Vec<String>> {
    let example = example_for(path).ok_or_else(|| format!("`{path}` has no example"))?;
    Ok(vec![format!("{example} --json")])
}

/// The `domain` of the argument `name` of `path`: what its Rust integer type carries (`implicit`), merged with the row of
/// the tool's table, when there is one. A row for an `enum` or a `switch` argument is left out: those have no domain.
fn domain_of(path: &str, name: &str, value_name: Option<&str>, value_type: ValueType, implicit: Option<Value>) -> Built<Option<Value>> {
    let Some(row) = crate::arg_rules::domain_for(path, name, value_name) else { return Ok(implicit) };
    let fits = match (&row.domain, value_type) {
        (_, ValueType::Enum | ValueType::Switch) => return Ok(implicit),
        (Domain::Integer { .. }, ValueType::Integer | ValueType::Count) | (Domain::Text { .. }, ValueType::String | ValueType::Path) => true,
        _ => false,
    };
    if !fits {
        return Err(format!("`{path}`: the domain of `{name}` is {:?} and the argument is a {}", row.domain, value_type.as_str()));
    }
    atip::domain::merge(implicit, Some(row.domain.to_value())).map_err(|problem| format!("`{path}`: the domain of `{name}`: {problem}"))
}

fn command_of(node: &Node<'_>) -> Built<CommandDoc> {
    let path = node.command.as_str();
    let meta = meta_for(path).ok_or_else(|| format!("`{path}` has no row in the metadata table"))?;
    let facts = facts_for(path).ok_or_else(|| format!("`{path}` has no row in the facts table"))?;
    let mut operands = Vec::new();
    for arg in node.cmd.get_positionals().filter(|arg| !project::is_builtin_arg(arg)) {
        let mut operand = built(project::operand_doc(path, arg, CUSTOM_PARSERS))?;
        if let Some(about) = facts.operands.iter().find(|about| about.name == arg.get_id().as_str()) {
            operand.help = about.about.to_string();
        }
        if operand.default.is_null() && !operand.required {
            operand.default_note = default_note(path, &operand.name).map(str::to_string);
        }
        operand.domain = domain_of(path, &operand.name, Some(&operand.value_name), operand.value_type, operand.domain.take())?;
        operands.push(operand);
    }
    let mut options = Vec::new();
    for arg in node.cmd.get_arguments().filter(|arg| !arg.is_positional() && !project::is_builtin_arg(arg)) {
        let mut option = built(project::option_doc(path, arg, CUSTOM_PARSERS, crate::input::flags_only()))?;
        option.env = option.env.or_else(|| declared_env(path, &option.name).map(str::to_string));
        if (option.default.is_null() && !option.required) || option.name == "promote" {
            option.default_note = default_note(path, &option.name).map(str::to_string);
        }
        option.domain = domain_of(path, &option.name, option.value_name.as_deref(), option.value_type, option.domain.take())?;
        options.push(option);
    }
    let mut schemas = BTreeMap::new();
    let input = input_of(path, node.cmd, &operands, &options, &mut schemas);
    let batch = input.as_ref().is_some_and(|input| input.batch);
    let outputs = outputs_of(path, batch, &mut schemas)?;
    let constraints: Vec<_> = crate::arg_rules::rules_of(node.cmd, path).iter().map(atip::constraint::Rule::doc).collect();
    let validators: Vec<ValidatorDoc> = crate::arg_rules::validators_for(path).iter().map(atip::constraint::Validator::doc).collect();
    let examples = examples_of(path)?;
    let errors = errors_of(path, &options, &operands, batch);
    let problems = atip::constraint::document_problems(&options, &operands, &constraints, &validators, &errors, &examples);
    if !problems.is_empty() {
        return Err(format!("`{path}`: {}", problems.join("; ")));
    }
    Ok(CommandDoc {
        kind: CommandDoc::KIND,
        command: node.command.clone(),
        path: node.path.clone(),
        summary: built(project::summary(node))?,
        effect: meta.effect,
        idempotency: facts.idempotency,
        effect_note: meta.effect_note.map(str::to_string),
        errors,
        operands,
        options,
        constraints,
        validators,
        outputs,
        input,
        examples,
        schemas,
        extensions: x_canon(path),
    })
}

/// `canon schema <COMMAND> --json`.
pub fn command(root: &Command, path: &[String]) -> Built<CommandDoc> {
    let nodes = project::nodes(root);
    let wanted = path.join(" ");
    let node = nodes.iter().find(|node| !node.is_group && node.command == wanted).ok_or_else(|| format!("`{wanted}` is not a command"))?;
    command_of(node)
}

/// Every command document, in tree order: what the checked-in spec holds and what `canon schema <COMMAND>` answers.
pub fn commands(root: &Command) -> Built<Vec<CommandDoc>> {
    project::nodes(root).iter().filter(|node| !node.is_group).map(command_of).collect()
}

/// The clap tree of the whole CLI, built: the root every document is read from.
pub fn built_root() -> Command {
    let mut root = crate::cli::command();
    root.build();
    root
}

/// One reason registered under an error code (spec 7.6).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ReasonRow {
    pub reason: &'static str,
    pub meaning: &'static str,
    /// What the caller does next in this situation.
    pub fix: &'static str,
}

/// One error code of the registry.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ErrorRow {
    pub code: &'static str,
    pub meaning: &'static str,
    /// The exit code a command that fails with it returns.
    pub exit: i32,
    /// Whether running the same command again, unchanged, can succeed.
    pub retryable: bool,
    /// One sentence telling the caller what to do about the code.
    pub fix: &'static str,
    /// The situations under the code that need another next action; absent when the code has none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<ReasonRow>,
}

/// Every registered error code, sorted by code.
pub fn error_rows() -> Vec<ErrorRow> {
    crate::cli_error::all_codes()
        .iter()
        .map(|info| ErrorRow {
            code: info.code,
            meaning: info.meaning,
            exit: i32::from(info.exit),
            retryable: info.retryable,
            fix: info.fix,
            reasons: info.reasons.iter().map(|reason| ReasonRow { reason: reason.reason, meaning: reason.meaning, fix: reason.fix }).collect(),
        })
        .collect()
}

/// One warning code of the warning registry (spec 7.5.2): no exit and no retryability.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct WarningRow {
    pub code: &'static str,
    pub meaning: &'static str,
    /// One sentence telling the caller what to do about the warning.
    pub fix: &'static str,
}

/// Every registered warning code, sorted by code.
pub fn warning_rows() -> Vec<WarningRow> {
    crate::cli_error::all_warnings().iter().map(|info| WarningRow { code: info.code, meaning: info.meaning, fix: info.fix }).collect()
}

/// Every removed spelling with its replacement (the CHANGELOG's Upgrading tables
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

/// The whole spec.
///
/// # Panics
/// When a table or a declaration the documents are built from is incomplete (an unlisted custom parser, a default
/// that does not parse, an option without help): that is a defect of this build, which `tests/schema_doc.rs` finds.
pub fn generate() -> Value {
    let root = built_root();
    let tree = tree(&root, None).expect("the tree is built from tables the tests keep whole");
    let commands = commands(&root).expect("the command documents are built from tables the tests keep whole");
    json!({
        "_doNotEdit": "GENERATED from the clap tree and the tables beside it: do not edit. Regenerate with `CANON_BLESS_CLI_SPEC=1 cargo test -p canon-cli --test cli_spec`.",
        "specVersion": SPEC_VERSION,
        "cli": {
            "name": root.get_name(),
            "version": env!("CARGO_PKG_VERSION"),
            "about": root.get_about().map(std::string::ToString::to_string),
        },
        "envVars": ENV_VARS.iter().map(|v| json!({ "name": v.name, "about": v.about })).collect::<Vec<_>>(),
        "exitTable": EXIT_TABLE.iter().map(|(code, meaning)| json!({ "code": code, "meaning": meaning })).collect::<Vec<_>>(),
        "contract": atip::CONTRACT,
        "contractVersion": crate::json_out::DECLARED_MINOR,
        "errors": error_rows(),
        "warnings": warning_rows(),
        "removed": removed_rows(),
        "tree": tree,
        "commands": commands,
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
